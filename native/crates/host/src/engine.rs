//! Game engine: downloads `game-min.js`, patches it (F1), injects the HaxFast
//! runtime into it and serves it to the browser instead of the original.
//!
//! Fail-open at every level: an anchor that is not found leaves its patch out,
//! a disk-cache error falls back to the network, and any problem returns `None`
//! so the client plays with the official file untouched.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::app::USER_AGENT;
use crate::log;
use haxfast_core::prefs::EngineToggles;

/// Runtime source, injected after `'use strict'` with the placeholders filled.
const RUNTIME: &str = include_str!("runtime.js");

/// Content ready to serve + summary for logs and status.
pub struct Engine {
    payload: Arc<Vec<u8>>,
    pub status: String,
}

impl Engine {
    /// Downloads (or reuses the disk cache of) the bundle and patches it.
    /// `None` = no patches (fail-open).
    ///
    /// The game path changes on every deploy (`/<key>/__cache_static__/g/game.html`),
    /// so it is resolved from the official page first.
    pub fn prepare(page_url: &str, toggles: &EngineToggles, cache_dir: &Path) -> Option<Self> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(20)))
            .build()
            .into();

        let page = fetch_text(&agent, page_url)?;
        let Some(game_url) = find_game_page_url(&page, page_url) else {
            log::warn(&format!(
                "engine: game page not found at {page_url} (did the site change?)"
            ));
            return None;
        };
        log::info(&format!("engine: game page at {game_url}"));

        let bundle_url = sibling_url(&game_url, "game-min.js");

        // Always resolve the deploy live (the page above is the resolution
        // already); the bundle itself may come from disk. Never serve a stale
        // entry for a different deploy: the cache key is the deploy path.
        let (original, source) = match disk_cache_load(cache_dir, &bundle_url) {
            Some(original) => (original, "disk cache"),
            None => {
                let original = fetch_text(&agent, &bundle_url)?;
                disk_cache_save(cache_dir, &bundle_url, &original);
                (original, "download")
            }
        };

        let (patched, patches) = patch(&original, toggles);
        let status = format!(
            "{} bytes ({source}), patches: {}",
            original.len(),
            patches.join(", ")
        );
        log::info(&format!("engine: game-min.js ready ({status})"));

        Some(Self {
            payload: Arc::new(patched.into_bytes()),
            status,
        })
    }

    /// Bytes served when the browser requests `game-min.js`.
    pub fn payload(&self) -> Arc<Vec<u8>> {
        self.payload.clone()
    }
}

fn fetch_text(agent: &ureq::Agent, url: &str) -> Option<String> {
    let mut response = match agent.get(url).header("User-Agent", USER_AGENT).call() {
        Ok(response) => response,
        Err(error) => {
            log::warn(&format!("engine: could not download {url}: {error}"));
            return None;
        }
    };
    if response.status() != 200 {
        log::warn(&format!("engine: {url} responded {}", response.status()));
        return None;
    }
    match response.body_mut().read_to_string() {
        Ok(text) => Some(text),
        Err(error) => {
            log::warn(&format!("engine: invalid body from {url}: {error}"));
            None
        }
    }
}

/// Is this URL the engine bundle?
pub fn is_game_bundle(url: &str) -> bool {
    url.split(['?', '#'])
        .next()
        .is_some_and(|path| path.ends_with("/game-min.js"))
}

/// Looks for the game page `src` (the one ending in `game.html`) in the `/play` HTML.
fn find_game_page_url(html: &str, page_url: &str) -> Option<String> {
    let mut rest = html;
    while let Some(index) = rest.find("src=\"") {
        let value_start = index + 5;
        let value_end = value_start + rest[value_start..].find('"')?;
        let value = &rest[value_start..value_end];
        if value.ends_with("game.html") {
            return Some(resolve_url(page_url, value));
        }
        rest = &rest[value_end..];
    }
    None
}

/// Turns a relative URL into an absolute one against the origin and directory of
/// `base`. No URL crate: the site uses simple paths and the gate checks this.
fn resolve_url(base: &str, relative: &str) -> String {
    if relative.contains("://") {
        return relative.to_string();
    }
    let origin = origin_of(base);
    if let Some(absolute) = relative.strip_prefix('/') {
        return format!("{origin}/{absolute}");
    }
    let base_path = &base[origin.len()..];
    let base_dir = match base_path.rfind('/') {
        Some(cut) => &base_path[..=cut],
        None => "/",
    };
    format!("{origin}{base_dir}{relative}")
}

/// `https://host[:port]/x/y.png` + `name` → `https://host[:port]/x/name`.
fn sibling_url(url: &str, name: &str) -> String {
    match url.rfind('/') {
        Some(cut) => format!("{}/{}", &url[..cut], name),
        None => format!("{url}/{name}"),
    }
}

fn origin_of(url: &str) -> &str {
    let scheme_end = url.find("://").map(|i| i + 3).unwrap_or(0);
    match url[scheme_end..].find('/') {
        Some(cut) => &url[..scheme_end + cut],
        None => url,
    }
}

/// Runtime + patches. Every patch is one unique anchor verified against the
/// live bundle and one tiny text substitution; an anchor not found simply
/// leaves out its patch (fail-open). The runtime is injected first (it must be
/// the first statement) but built last, so `patches` can list what got applied.
fn patch(original: &str, toggles: &EngineToggles) -> (String, Vec<&'static str>) {
    let mut patches = vec!["runtime"];
    let mut text = original.to_string();

    // zero-input-tolerance: input tolerance 2 → 0 (60 Hz ticks without the
    // ~33 ms of dead time). `this.vo=600;super.Za(` right after is context and
    // stays untouched.
    try_patch(
        &mut text,
        toggles.zero_input_tolerance,
        "zero-input-tolerance",
        "this.Od=2",
        "this.Od=0",
        &mut patches,
    );

    // canvas-resize-cache: only the per-frame rect read is delegated to the
    // runtime (ResizeObserver cache); the backing-store write stays guarded and
    // devicePixelRatio stays live at the call site.
    try_patch(
        &mut text,
        toggles.canvas_resize_cache,
        "canvas-resize-cache",
        "this.oa.getBoundingClientRect()",
        "__HAXFAST_ENGINE__.rect(this.oa)",
        &mut patches,
    );

    // stadium-layer-cache: the per-frame Dr+Cr passes become a cached texture
    // blit managed by the runtime. The zoom `h` local at the call site is exact.
    try_patch(
        &mut text,
        toggles.stadium_layer_cache,
        "stadium-layer-cache",
        "this.Dr(c.U);this.Cr(c.U);",
        "__HAXFAST_ENGINE__.stadium(this,h,c.U);",
        &mut patches,
    );

    // hidden-frame-skip: the live-view loop keeps scheduling rAF but returns
    // before drawing while the page is hidden. The needle carries the unique
    // `this.W.A()` so the other loop class (replay/menu) is never touched.
    // NOTE: the guard is negated — `shouldRender()` is true when drawing.
    try_patch(
        &mut text,
        toggles.hidden_frame_skip,
        "hidden-frame-skip",
        "this.Re=window.requestAnimationFrame(M(this,this.sf));this.W.A();",
        "this.Re=window.requestAnimationFrame(M(this,this.sf));\
         if(!__HAXFAST_ENGINE__.shouldRender())return;this.W.A();",
        &mut patches,
    );

    let toggles_json = serde_json::json!({
        "canvas-resize-cache": toggles.canvas_resize_cache,
        "stadium-layer-cache": toggles.stadium_layer_cache,
        "zero-input-tolerance": toggles.zero_input_tolerance,
        "hidden-frame-skip": toggles.hidden_frame_skip,
    })
    .to_string();
    let patches_json = serde_json::to_string(&patches).unwrap_or_else(|_| "[]".into());
    let runtime = RUNTIME
        .replace("__HF_VERSION__", haxfast_core::version())
        .replace("__HF_TOGGLES__", &toggles_json)
        .replace("__HF_PATCHES__", &patches_json);

    (insert_after_directive(&text, &runtime), patches)
}

/// Applies `replacement` in place of `anchor`, exactly once. Logged outcomes:
/// off (preference), applied, or skipped when the anchor is missing (fail-open).
fn try_patch(
    text: &mut String,
    enabled: bool,
    name: &'static str,
    anchor: &str,
    replacement: &str,
    patches: &mut Vec<&'static str>,
) {
    if !enabled {
        log::info(&format!("engine: {name} off (preference)"));
        return;
    }
    if replace_once(text, anchor, replacement) {
        patches.push(name);
    } else {
        log::warn(&format!("engine: {name} skipped: anchor not found (fail-open)"));
    }
}

/// Replaces the first occurrence of `anchor` with `replacement`.
fn replace_once(text: &mut String, anchor: &str, replacement: &str) -> bool {
    match text.find(anchor) {
        Some(index) => {
            text.replace_range(index..index + anchor.len(), replacement);
            true
        }
        None => false,
    }
}

/// Inserts `code` right after the `'use strict'` directive (looked up only in the
/// header, to avoid confusing it with a random string in the body).
fn insert_after_directive(source: &str, code: &str) -> String {
    let head_end = source.len().min(256);
    let head = &source[..head_end];

    for directive in ["'use strict';", "\"use strict\";"] {
        if let Some(index) = head.find(directive) {
            let cut = index + directive.len();
            let mut out = String::with_capacity(source.len() + code.len());
            out.push_str(&source[..cut]);
            out.push_str(code);
            out.push_str(&source[cut..]);
            return out;
        }
    }

    format!("{code}{source}")
}

/// Digest for cache integrity (std `DefaultHasher`: no new dependency; stable
/// across runs for the same input).
fn content_hash(bytes: &[u8]) -> u64 {
    use std::hash::Hasher;

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hasher.write(bytes);
    hasher.finish()
}

/// Cache id of a bundle: hash of its URL (the path rotates per deploy).
fn bundle_id(bundle_url: &str) -> String {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bundle_url.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Loads the ORIGINAL bundle from `$HAXFAST_HOME/engine/`. Hit only when the
/// stored URL matches and the stored content hash still matches; anything else
/// is a miss (logged when surprising) and the network step takes over.
fn disk_cache_load(dir: &Path, bundle_url: &str) -> Option<String> {
    if !dir.is_dir() {
        return None;
    }
    let id = bundle_id(bundle_url);
    let meta_text = std::fs::read_to_string(dir.join(format!("{id}.json"))).ok()?;
    let meta: serde_json::Value = serde_json::from_str(&meta_text).ok()?;
    if meta.get("url").and_then(|value| value.as_str()) != Some(bundle_url) {
        return None;
    }
    let raw = std::fs::read(dir.join(format!("{id}.js"))).ok()?;
    if meta.get("hash").and_then(|value| value.as_u64()) != Some(content_hash(&raw)) {
        log::warn("engine: disk cache entry is corrupt (hash mismatch): re-downloading");
        return None;
    }
    match String::from_utf8(raw) {
        Ok(original) => Some(original),
        Err(_) => {
            log::warn("engine: disk cache entry is not UTF-8: re-downloading");
            None
        }
    }
}

/// Best-effort save of the ORIGINAL bundle (never fatal). `.js` first: a partial
/// pair without meta can only miss, never mislead.
fn disk_cache_save(dir: &Path, bundle_url: &str, original: &str) {
    if let Err(error) = std::fs::create_dir_all(dir) {
        log::warn(&format!("engine: disk cache disabled ({error})"));
        return;
    }
    let id = bundle_id(bundle_url);
    let meta = serde_json::json!({
        "url": bundle_url,
        "hash": content_hash(original.as_bytes()),
    });
    let entry = dir.join(format!("{id}.js"));
    let error = std::fs::write(&entry, original).and_then(|_| {
        std::fs::write(dir.join(format!("{id}.json")), meta.to_string())
    });
    if let Err(error) = error {
        let _ = std::fs::remove_file(&entry);
        log::warn(&format!("engine: disk cache save failed ({error})"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("haxfast-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A tiny fake of the interesting fragments of the live bundle (all four
    /// F1 anchors in order). Text-only: the transforms are string surgery.
    fn fixture() -> String {
        [
            "'use strict';",
            "(function(){",
            "Z(){this.Od=2;this.vo=600;Za(x)};",
            "sf(){this.Re=window.requestAnimationFrame(M(this,this.sf));this.W.A();",
            "\nthis.za.A();this.Rc()};",
            "Ds(){var b=this.oa.getBoundingClientRect()};",
            "Rc(){h=2;this.Dr(c.U);this.Cr(c.U);}",
            "})()",
        ]
        .concat()
    }

    #[test]
    fn inserts_after_use_strict() {
        let out = insert_after_directive("'use strict';(function(){})();", "M;");
        assert_eq!(out, "'use strict';M;(function(){})();");
    }

    #[test]
    fn without_directive_inserts_at_the_top() {
        assert_eq!(insert_after_directive("var x=1;", ";M;"), ";M;var x=1;");
    }

    #[test]
    fn ignores_late_directives() {
        let late = format!("var a=1;{}'use strict';", " ".repeat(400));
        assert!(insert_after_directive(&late, ";M;").starts_with(";M;"));
    }

    #[test]
    fn recognizes_the_engine_bundle() {
        assert!(is_game_bundle("https://www.haxball.com/game-min.js"));
        assert!(is_game_bundle("https://www.haxball.com/game-min.js?v=2#x"));
        assert!(!is_game_bundle("https://www.haxball.com/vendor/pako.min.js"));
    }

    #[test]
    fn resolves_the_game_page_from_the_html() {
        let html = r#"<iframe src="H4mQWPXF/__cache_static__/g/game.html"></iframe>"#;
        assert_eq!(
            find_game_page_url(html, "https://www.haxball.com/play").as_deref(),
            Some("https://www.haxball.com/H4mQWPXF/__cache_static__/g/game.html")
        );
        assert_eq!(find_game_page_url("<iframe src=\"x.html\">", "https://x/a"), None);
    }

    #[test]
    fn absolute_and_sibling() {
        assert_eq!(
            resolve_url("https://x/a/b.html", "sub/c.html"),
            "https://x/a/sub/c.html"
        );
        assert_eq!(resolve_url("https://x/a/b.html", "/root.html"), "https://x/root.html");
        assert_eq!(
            sibling_url("https://x/a/g/game.html", "game-min.js"),
            "https://x/a/g/game-min.js"
        );
    }

    #[test]
    fn applies_every_patch_when_all_anchors_exist() {
        let (out, patches) = patch(&fixture(), &EngineToggles::default());
        assert_eq!(
            patches,
            vec![
                "runtime",
                "zero-input-tolerance",
                "canvas-resize-cache",
                "stadium-layer-cache",
                "hidden-frame-skip",
            ]
        );
        assert!(out.contains("this.Od=0;this.vo=600;"), "{out}");
        assert!(out.contains("__HAXFAST_ENGINE__.rect(this.oa)"), "{out}");
        assert!(out.contains("__HAXFAST_ENGINE__.stadium(this,h,c.U);"), "{out}");
        assert!(
            out.contains(
                "window.requestAnimationFrame(M(this,this.sf));\
                 if(!__HAXFAST_ENGINE__.shouldRender())return;this.W.A();"
            ),
            "{out}"
        );
        assert!(out.starts_with("'use strict';"));
        assert!(!out.contains("this.Od=2;"));
    }

    #[test]
    fn missing_anchor_leaves_its_patch_out_and_the_bundle_working() {
        let broken = fixture().replacen("this.Od=2", "this.Od=9", 1);
        let (out, patches) = patch(&broken, &EngineToggles::default());
        assert!(!patches.contains(&"zero-input-tolerance"));
        assert!(out.contains("this.Od=9;"));
        assert!(patches.contains(&"stadium-layer-cache"));
        assert!(out.starts_with("'use strict';"));
    }

    #[test]
    fn disabled_toggles_keep_the_vanilla_bytes() {
        let toggles = EngineToggles {
            canvas_resize_cache: false,
            stadium_layer_cache: false,
            zero_input_tolerance: false,
            hidden_frame_skip: false,
        };
        let (out, patches) = patch(&fixture(), &toggles);
        assert_eq!(patches, vec!["runtime"]);
        assert!(out.contains("this.Od=2"));
        assert!(out.contains("this.oa.getBoundingClientRect()"));
        assert!(out.contains("this.Dr(c.U);this.Cr(c.U);"));
        assert!(out.contains("window.requestAnimationFrame(M(this,this.sf));this.W.A();"));
    }

    #[test]
    fn runtime_carries_version_toggles_and_the_patch_list() {
        let (out, patches) = patch(&fixture(), &EngineToggles::default());
        assert!(out.contains(haxfast_core::version()), "{out}");
        assert!(out.contains("\"canvas-resize-cache\":true"), "{out}");
        assert!(out.contains("[\"runtime\",\"zero-input-tolerance\""), "{out}");
        assert_eq!(patches.first(), Some(&"runtime"));
    }

    #[test]
    fn disk_cache_round_trip_and_corruption() {
        let dir = temp_dir("engine-cache");
        let url = "https://www.haxball.com/H4mQWPXF/__cache_static__/g/game-min.js";

        assert_eq!(disk_cache_load(&dir, url), None);

        disk_cache_save(&dir, url, "'use strict';BODY;");
        assert_eq!(disk_cache_load(&dir, url), Some("'use strict';BODY;".to_string()));

        // A different deploy path is a different key: miss.
        let other = "https://www.haxball.com/OTHER/__cache_static__/g/game-min.js";
        assert_eq!(disk_cache_load(&dir, other), None);

        // Corrupt bytes (hash mismatch) are a miss: the network takes over.
        let id = bundle_id(url);
        std::fs::write(dir.join(format!("{id}.js")), b"corrupt").unwrap();
        assert_eq!(disk_cache_load(&dir, url), None);
    }
}
