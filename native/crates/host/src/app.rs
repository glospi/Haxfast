//! CEF app: window (Views framework), navigation handlers and host wiring.

use std::cell::RefCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use cef::*;

use crate::ads;
use crate::bridge::BridgeInfo;
use crate::engine::{self, Engine};
use crate::log;
use crate::paths;
use haxfast_core::prefs::{GovernorMode, VideoPrefs};

/// Official game page (or `$HAXFAST_URL` for tests).
pub const GAME_URL: &str = "https://www.haxball.com/play";

/// Desktop Chrome UA: the official site expects a modern browser.
pub const USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/154.0.0.0 Safari/537.36";

/// Host state visible to the CEF callbacks.
pub struct HostState {
    pub bridge: BridgeInfo,
    pub engine: Option<Engine>,
    pub page_url: String,
    /// F2 startup choices (flags cannot change while running).
    pub video: VideoPrefs,
    /// Live browsers: the app closes when the last one closes.
    pub browsers: AtomicUsize,
}

static STATE: OnceLock<Arc<HostState>> = OnceLock::new();

pub fn init_state(state: HostState) {
    let _ = STATE.set(Arc::new(state));
}

fn state() -> Arc<HostState> {
    STATE.get().expect("HostState not initialized").clone()
}

// ---------------------------------------------------------------------------
// App / startup
// ---------------------------------------------------------------------------

wrap_app! {
    pub struct HaxFastApp;

    impl App {
        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(HaxFastBrowserProcessHandler::new(RefCell::new(None)))
        }

        fn on_before_command_line_processing(
            &self,
            process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            let is_browser_process = process_type.map_or(true, |t| t.to_string().is_empty());
            let Some(command_line) = command_line else {
                return;
            };
            if !is_browser_process {
                return;
            }
            // F2: gsync keeps Chromium's defaults; locked/unlimited disable the
            // frame-rate limit and vsync. The draw cap itself is applied live by
            // the page (D21/D22).
            let video = state().video.clone();
            let mode = video.mode();
            if mode.needs_frame_rate_flags() {
                command_line.append_switch(Some(&CefString::from("disable-frame-rate-limit")));
                command_line.append_switch(Some(&CefString::from("disable-gpu-vsync")));
            }
            if let Some(angle) = video.backend().angle_value() {
                command_line.append_switch_with_value(
                    Some(&CefString::from("use-angle")),
                    Some(&CefString::from(angle)),
                );
            }
            let limit = match mode {
                GovernorMode::Locked => format!("{} fps", video.draw_cap()),
                _ => "no cap".to_string(),
            };
            log::info(&format!(
                "cef: startup flags (governor {}, {}, gpu {})",
                mode.as_str(),
                limit,
                video.backend().as_str()
            ));
        }
    }
}

wrap_browser_process_handler! {
    struct HaxFastBrowserProcessHandler {
        client: RefCell<Option<Client>>,
    }

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            // Chromium ≥138 (Local Network Access) asks permission for a public page
            // to talk to 127.0.0.1. Without a permission UI the request hangs, so it
            // is granted by default: the host bridge lives on loopback (D3).
            if let Some(context) = request_context_get_global_context() {
                context.set_content_setting(
                    None,
                    None,
                    ContentSettingTypes::LOOPBACK_NETWORK,
                    ContentSettingValues::ALLOW,
                );
            }

            let client = HaxFastClient::new();
            *self.client.borrow_mut() = Some(client.clone());

            let url = CefString::from(state().page_url.as_str());
            let browser_settings = BrowserSettings::default();

            // Alloy style: the app draws the window (a single one, no tabs or
            // omnibox). CEF's default "Chrome" style restores profile sessions and
            // opens browser UI we do not want.
            let mut client = self.client.borrow().clone();
            let mut view_delegate = HaxFastBrowserViewDelegate::new(RuntimeStyle::ALLOY);
            let browser_view = browser_view_create(
                client.as_mut(),
                Some(&url),
                Some(&browser_settings),
                None,
                None,
                Some(&mut view_delegate),
            );

            let mut window_delegate = HaxFastWindowDelegate::new(
                RefCell::new(browser_view),
                RuntimeStyle::ALLOY,
                ShowState::NORMAL,
            );
            window_create_top_level(Some(&mut window_delegate));
            log::info("cef: window created");
        }
    }
}

// ---------------------------------------------------------------------------
// Window and view
// ---------------------------------------------------------------------------

wrap_browser_view_delegate! {
    struct HaxFastBrowserViewDelegate {
        runtime_style: RuntimeStyle,
    }

    impl ViewDelegate {}

    impl BrowserViewDelegate {
        fn browser_runtime_style(&self) -> RuntimeStyle {
            self.runtime_style
        }
    }
}

wrap_window_delegate! {
    struct HaxFastWindowDelegate {
        browser_view: RefCell<Option<BrowserView>>,
        runtime_style: RuntimeStyle,
        initial_show_state: ShowState,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            Size { width: 1280, height: 720 }
        }
    }

    impl PanelDelegate {}

    impl WindowDelegate {
        fn on_window_created(&self, window: Option<&mut Window>) {
            let browser_view = self.browser_view.borrow();
            let (Some(window), Some(browser_view)) = (window, browser_view.as_ref()) else {
                return;
            };
            let mut view = View::from(browser_view);
            window.add_child_view(Some(&mut view));
            if self.initial_show_state != ShowState::HIDDEN {
                window.show();
            }
        }

        fn on_window_destroyed(&self, _window: Option<&mut Window>) {
            *self.browser_view.borrow_mut() = None;
        }

        fn can_close(&self, _window: Option<&mut Window>) -> i32 {
            // The close is decided by the browser (same as cefsimple).
            let browser_view = self.browser_view.borrow();
            let Some(browser_view) = browser_view.as_ref() else {
                return 1;
            };
            match browser_view.browser() {
                Some(browser) => match browser.host() {
                    Some(host) => host.try_close_browser(),
                    None => 1,
                },
                None => 1,
            }
        }

        fn initial_show_state(&self, _window: Option<&mut Window>) -> ShowState {
            self.initial_show_state
        }

        fn window_runtime_style(&self) -> RuntimeStyle {
            self.runtime_style
        }
    }
}

// ---------------------------------------------------------------------------
// Client and handlers
// ---------------------------------------------------------------------------

wrap_client! {
    pub struct HaxFastClient;

    impl Client {
        /// F2: the only browser UI kept is a Copy/Paste context menu.
        fn context_menu_handler(&self) -> Option<ContextMenuHandler> {
            Some(HaxFastContextMenuHandler::new())
        }

        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(HaxFastDisplayHandler::new())
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(HaxFastLifeSpanHandler::new())
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(HaxFastLoadHandler::new())
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(HaxFastRequestHandler::new())
        }
    }
}

wrap_life_span_handler! {
    struct HaxFastLifeSpanHandler;

    impl LifeSpanHandler {
        fn on_after_created(&self, _browser: Option<&mut Browser>) {
            state().browsers.fetch_add(1, Ordering::SeqCst);
            log::info("browser: created");
        }

        fn on_before_close(&self, _browser: Option<&mut Browser>) {
            let remaining = state()
                .browsers
                .fetch_sub(1, Ordering::SeqCst)
                .saturating_sub(1);
            log::info(&format!("browser: closed ({remaining} left)"));
            if remaining == 0 {
                quit_message_loop();
            }
        }
    }
}

wrap_load_handler! {
    struct HaxFastLoadHandler;

    impl LoadHandler {
        fn on_load_end(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            http_status_code: i32,
        ) {
            let Some(frame) = frame else {
                return;
            };
            if frame.is_main() == 0 {
                return; // the game's inner frames are handled in later phases
            }
            let url = CefString::from(&frame.url()).to_string();
            log::info(&format!("page: loaded {url} (HTTP {http_status_code})"));
            inject_page(frame);
        }

        fn on_load_error(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            error_code: Errorcode,
            error_text: Option<&CefString>,
            failed_url: Option<&CefString>,
        ) {
            let code = sys::cef_errorcode_t::from(error_code) as i32;
            let text = error_text.map(CefString::to_string).unwrap_or_default();
            let url = failed_url.map(CefString::to_string).unwrap_or_default();
            let where_ = match frame {
                Some(frame) if frame.is_main() != 0 => "main",
                _ => "inner",
            };
            log::warn(&format!("page: load error ({where_}) {url} [{code}] {text}"));
        }
    }
}

wrap_display_handler! {
    struct HaxFastDisplayHandler;

    impl DisplayHandler {
        fn on_title_change(&self, browser: Option<&mut Browser>, title: Option<&CefString>) {
            let mut browser = browser.cloned();
            if let Some(browser_view) = browser_view_get_for_browser(browser.as_mut()) {
                if let Some(window) = browser_view.window() {
                    window.set_title(title);
                }
            }
        }

        fn on_console_message(
            &self,
            _browser: Option<&mut Browser>,
            level: LogSeverity,
            message: Option<&CefString>,
            source: Option<&CefString>,
            line: i32,
        ) -> i32 {
            let message = message.map(CefString::to_string).unwrap_or_default();
            let source = source.map(CefString::to_string).unwrap_or_default();
            log::info(&format!(
                "console[{}] {source}:{line} {message}",
                level.get_raw()
            ));
            0
        }
    }
}

wrap_context_menu_handler! {
    // F2: keeps only Copy and Paste from the browser's default menu. The menu
    // steals DOM focus; the page layer tracks the right-clicked element so the
    // edit command runs against it.
    struct HaxFastContextMenuHandler;

    impl ContextMenuHandler {
        fn on_before_context_menu(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _params: Option<&mut ContextMenuParams>,
            model: Option<&mut MenuModel>,
        ) {
            let Some(model) = model else {
                return;
            };
            model.clear();
            model.add_item(MenuId::COPY.get_raw() as i32, Some(&CefString::from("Copy")));
            model.add_item(MenuId::PASTE.get_raw() as i32, Some(&CefString::from("Paste")));
        }

        fn on_context_menu_command(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _params: Option<&mut ContextMenuParams>,
            command_id: i32,
            _event_flags: EventFlags,
        ) -> i32 {
            let Some(frame) = frame else {
                return 0;
            };
            // The native menu steals DOM focus and Copy/Paste are no-ops without
            // it; the page layer tracks the element that was right-clicked.
            let focus = CefString::from(
                "window.__HF_CTX_TARGET__ && window.__HF_CTX_TARGET__.focus && window.__HF_CTX_TARGET__.focus()",
            );
            frame.execute_java_script(Some(&focus), None, 0);
            if command_id == MenuId::COPY.get_raw() as i32 {
                frame.copy();
                return 1;
            }
            if command_id == MenuId::PASTE.get_raw() as i32 {
                frame.paste();
                return 1;
            }
            0
        }
    }
}

wrap_request_handler! {
    struct HaxFastRequestHandler;

    impl RequestHandler {
        fn resource_request_handler(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _is_navigation: i32,
            _is_download: i32,
            _request_initiator: Option<&CefString>,
            _disable_default_handling: Option<&mut i32>,
        ) -> Option<ResourceRequestHandler> {
            let Some(request) = request else {
                return None;
            };
            let url = CefString::from(&request.url()).to_string();
            if let Some(domain) = ads::ad_domain(&url) {
                ads::log_blocked(domain);
                return Some(HaxFastBlockedResourceRequestHandler::new());
            }
            if engine::is_game_bundle(&url) {
                if state().engine.is_some() {
                    log::info(&format!("engine: intercepted {url}"));
                    return Some(HaxFastResourceRequestHandler::new());
                }
                log::warn(&format!(
                    "engine: request for {url} with no patch available (fail-open: serving the original)"
                ));
            }
            None
        }
    }
}

wrap_resource_request_handler! {
    // Cancels ad/sync requests before they reach the network.
    struct HaxFastBlockedResourceRequestHandler;

    impl ResourceRequestHandler {
        fn on_before_resource_load(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _request: Option<&mut Request>,
            _callback: Option<&mut Callback>,
        ) -> ReturnValue {
            ReturnValue::CANCEL
        }
    }
}

wrap_resource_request_handler! {
    struct HaxFastResourceRequestHandler;

    impl ResourceRequestHandler {
        fn on_before_resource_load(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _request: Option<&mut Request>,
            _callback: Option<&mut Callback>,
        ) -> ReturnValue {
            // Careful: cef-rs returns RV_CANCEL by default here. Without CONTINUE, CEF
            // cancels the request and never gets to query `resource_handler`.
            ReturnValue::CONTINUE
        }

        fn resource_handler(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _request: Option<&mut Request>,
        ) -> Option<ResourceHandler> {
            let engine = state().engine.as_ref()?.payload();
            Some(HaxFastResourceHandler::with_payload(engine))
        }
    }
}

wrap_resource_handler! {
    struct HaxFastResourceHandler {
        payload: Arc<Vec<u8>>,
        offset: Arc<AtomicUsize>,
    }

    impl ResourceHandler {
        fn open(
            &self,
            _request: Option<&mut Request>,
            handle_request: Option<&mut i32>,
            _callback: Option<&mut Callback>,
        ) -> i32 {
            if let Some(handle_request) = handle_request {
                *handle_request = 1;
            }
            1
        }

        fn response_headers(
            &self,
            response: Option<&mut Response>,
            response_length: Option<&mut i64>,
            _redirect_url: Option<&mut CefString>,
        ) {
            let Some(response) = response else {
                return;
            };
            response.set_status(200);
            response.set_status_text(Some(&CefString::from("OK")));
            response.set_mime_type(Some(&CefString::from("application/javascript")));
            if let Some(response_length) = response_length {
                *response_length = self.payload.len() as i64;
            }
        }

        fn read(
            &self,
            data_out: *mut u8,
            bytes_to_read: i32,
            bytes_read: Option<&mut i32>,
            _callback: Option<&mut ResourceReadCallback>,
        ) -> i32 {
            let Some(bytes_read) = bytes_read else {
                return 0;
            };
            let offset = self.offset.load(Ordering::SeqCst);
            let remaining = self.payload.len().saturating_sub(offset);
            if remaining == 0 {
                *bytes_read = 0;
                return 0; // end of response (cef_resource_handler_t semantics)
            }
            let count = remaining.min(bytes_to_read.max(0) as usize);
            unsafe {
                std::ptr::copy_nonoverlapping(self.payload[offset..].as_ptr(), data_out, count);
            }
            self.offset.store(offset + count, Ordering::SeqCst);
            *bytes_read = count as i32;
            1
        }

        fn skip(
            &self,
            bytes_to_skip: i64,
            bytes_skipped: Option<&mut i64>,
            _callback: Option<&mut ResourceSkipCallback>,
        ) -> i32 {
            let offset = self.offset.load(Ordering::SeqCst);
            let remaining = self.payload.len().saturating_sub(offset) as i64;
            let count = remaining.min(bytes_to_skip.max(0));
            self.offset.store(offset + count as usize, Ordering::SeqCst);
            if let Some(bytes_skipped) = bytes_skipped {
                *bytes_skipped = count;
            }
            1
        }

        fn cancel(&self) {}
    }
}

impl HaxFastResourceHandler {
    /// The macro-generated `new()` requires every field; this shortcut starts the cursor at 0.
    fn with_payload(payload: Arc<Vec<u8>>) -> ResourceHandler {
        Self::new(payload, Arc::new(AtomicUsize::new(0)))
    }
}

// ---------------------------------------------------------------------------
// Page layer injection
// ---------------------------------------------------------------------------

/// Injects the bridge address and the page bundle into the main frame.
/// The bundle is re-read on every load: rebuilding + reloading is enough to develop.
fn inject_page(frame: &mut Frame) {
    let state = state();
    let boot = format!(
        "window.__HAXFAST_BOOT={{wsUrl:{},token:{}}};",
        serde_json::to_string(&state.bridge.ws_url()).unwrap_or_else(|_| "\"\"".into()),
        serde_json::to_string(&state.bridge.token).unwrap_or_else(|_| "\"\"".into()),
    );
    frame.execute_java_script(
        Some(&CefString::from(boot.as_str())),
        Some(&CefString::from("haxfast://boot.js")),
        0,
    );

    let Some(bundle) = paths::page_bundle() else {
        log::warn("page: dist/page.js not found (missing `npm run build:page`?)");
        return;
    };
    match std::fs::read_to_string(&bundle) {
        Ok(source) => {
            frame.execute_java_script(
                Some(&CefString::from(source.as_str())),
                Some(&CefString::from("haxfast://page.js")),
                0,
            );
            log::info(&format!("page: bundle injected ({})", bundle.display()));
        }
        Err(error) => log::warn(&format!(
            "page: could not read {}: {error}",
            bundle.display()
        )),
    }
}
