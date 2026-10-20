//! HaxFast host: embeds Chromium (CEF), serves the patched engine and the prefs
//! bridge, and opens the official game page.

mod app;
mod bridge;
mod engine;
mod log;
mod paths;

use cef::*;
use haxfast_core::prefs::{GpuBackend, GovernorMode};

fn main() {
    ensure_libcef_preloaded();

    // Checks the CEF API version expected by the crate against libcef.so.
    // This must come first: the rest of the API calls assume this check.
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);

    // This same binary is re-executed as a Chromium subprocess (renderer, GPU,
    // utility…): CEF services it inside execute_process and the process ends there.
    let args = cef::args::Args::new();
    let Some(command_line) = args.as_cmd_line() else {
        eprintln!("haxfast-host: could not parse the command line");
        std::process::exit(1);
    };
    let is_browser_process = command_line.has_switch(Some(&CefString::from("type"))) != 1;

    let ret = execute_process(Some(args.as_main_args()), None, std::ptr::null_mut());
    if is_browser_process {
        if ret != -1 {
            eprintln!("haxfast-host: execute_process failed in the main process ({ret})");
            std::process::exit(1);
        }
    } else {
        return;
    }

    let state = init_runtime();

    let user_data = paths::user_data_dir();
    let cache_path = paths::cache_dir();
    let log_file = paths::cef_log_file();
    let settings = Settings {
        // F0: sandbox disabled on purpose (decision in docs/GATE-F0.md and
        // docs/DECISIONS.md); it is re-evaluated when packaging (F9).
        no_sandbox: 1,
        user_agent: CefString::from(app::USER_AGENT),
        cache_path: CefString::from(cache_path.to_string_lossy().as_ref()),
        persist_session_cookies: 1,
        log_file: CefString::from(log_file.to_string_lossy().as_ref()),
        log_severity: LogSeverity::INFO,
        background_color: 0xFF17_171B, // dark gray: avoids the white flash on open
        remote_debugging_port: paths::remote_debug_port(),
        ..Default::default()
    };

    app::init_state(state);

    let mut cef_app = app::HaxFastApp::new();
    if initialize(
        Some(args.as_main_args()),
        Some(&settings),
        Some(&mut cef_app),
        std::ptr::null_mut(),
    ) != 1
    {
        log::error("host: CEF initialize failed");
        std::process::exit(1);
    }

    run_message_loop();
    shutdown();
    log::info(&format!("host: shut down (data in {})", user_data.display()));
}

/// libcef.so exports an interposed `close` that resolves the real one with
/// `dlsym(RTLD_NEXT, "close")`, that is, it searches *after* libcef in load order.
/// In this binary libc comes before libcef (rustc fixes that), so the search fails
/// and Chromium aborts on initialization. With LD_PRELOAD, libcef goes first and
/// the next `close` is libc's (same soname: it is not loaded twice).
/// Fixed by re-executing once with the variables set.
///
/// shortcut: costs one extra exec at startup; revisit when adopting the CEF sandbox (F9).
fn ensure_libcef_preloaded() {
    if std::env::var_os("HAXFAST_PRELOADED").is_some() {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(dir) = exe.parent() else {
        return;
    };

    let mut library_path = dir.as_os_str().to_os_string();
    if let Some(previous) = std::env::var_os("LD_LIBRARY_PATH").filter(|value| !value.is_empty()) {
        library_path.push(":");
        library_path.push(previous);
    }
    let preload = match std::env::var_os("LD_PRELOAD").filter(|value| !value.is_empty()) {
        Some(previous) => {
            let mut value = std::ffi::OsString::from("libcef.so:");
            value.push(previous);
            value
        }
        None => std::ffi::OsString::from("libcef.so"),
    };

    use std::os::unix::process::CommandExt as _;
    let error = std::process::Command::new(&exe)
        .env("LD_PRELOAD", preload)
        .env("LD_LIBRARY_PATH", library_path)
        .env("HAXFAST_PRELOADED", "1")
        .exec();
    eprintln!("haxfast-host: could not re-exec with libcef preload: {error}");
}

/// Starts logs, bridge and patched engine; returns the shared state.
fn init_runtime() -> app::HostState {
    let user_data = paths::user_data_dir();
    if let Err(error) = log::init(&paths::log_file()) {
        eprintln!("haxfast-host: could not open the log at {user_data:?}: {error}");
    }
    log::info(&format!(
        "host: haxfast {} starting (data in {})",
        haxfast_core::version(),
        user_data.display()
    ));

    let bridge = match bridge::start() {
        Ok(bridge) => bridge,
        Err(error) => {
            log::error(&format!("bridge: could not open the local socket: {error}"));
            std::process::exit(1);
        }
    };
    log::info(&format!("bridge: listening on {}", bridge.ws_url()));

    let requested_url = std::env::var("HAXFAST_URL").unwrap_or_else(|_| app::GAME_URL.to_string());

    // F1: the engine patches read their toggles from preferences.json.
    let (prefs, prefs_warning) = haxfast_core::prefs::Preferences::load(&paths::prefs_file());
    if let Some(warning) = prefs_warning {
        log::warn(&format!("prefs: {warning}"));
    }
    if GovernorMode::parse(&prefs.video.governor).is_none() {
        log::warn(&format!(
            "prefs: unknown governor '{}' (using gsync)",
            prefs.video.governor
        ));
    }
    if GpuBackend::parse(&prefs.video.gpu_backend).is_none() {
        log::warn(&format!(
            "prefs: unknown gpu_backend '{}' (using auto)",
            prefs.video.gpu_backend
        ));
    }
    if prefs.video.fps_limit != prefs.video.draw_cap() {
        log::warn(&format!(
            "prefs: fps_limit {} out of range (using {})",
            prefs.video.fps_limit,
            prefs.video.draw_cap()
        ));
    }
    let video = prefs.video.clone();

    let engine = match engine::Engine::prepare(&requested_url, &prefs.engine, &paths::engine_dir())
    {
        Some(engine) => {
            log::info(&format!("engine: ready ({})", engine.status));
            Some(engine)
        }
        None => {
            log::warn("engine: no patches (fail-open): the page will load the original engine");
            None
        }
    };
    // The official page (`/play`) embeds the game in an iframe: loading it as is makes
    // the engine start. A bare `game.html` stays in "Loading" (it needs its parent
    // page). From the bundle only the URL matters, and that was resolved before.
    let page_url = requested_url;

    app::HostState {
        bridge,
        engine,
        page_url,
        video,
        browsers: std::sync::atomic::AtomicUsize::new(0),
    }
}
