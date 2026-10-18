//! Game engine: downloads `game-min.js`, injects the HaxFast runtime into it and
//! serves it to the browser instead of the original.
//!
//! Fail-open: on any problem it returns `None` and the client plays with the
//! official file untouched (project rule: never break the ability to play).

use std::sync::Arc;
use std::time::Duration;

use crate::app::USER_AGENT;
use crate::log;

/// Content ready to serve + summary for logs and status.
pub struct Engine {
    payload: Arc<Vec<u8>>,
    pub status: String,
}

impl Engine {
    /// Downloads and patches the bundle. `None` = no patches (fail-open).
    ///
    /// The game path changes on every deploy (`/<key>/__cache_static__/g/game.html`),
    /// so it is resolved from the official page first.
    pub fn prepare(page_url: &str) -> Option<Self> {
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
        let original = fetch_text(&agent, &bundle_url)?;

        let (patched, patches) = patch(&original);
        let status = format!("{} bytes, patches: {}", original.len(), patches.join(", "));
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

/// Runtime + patches. Returns the final bundle and what was applied.
fn patch(original: &str) -> (String, Vec<&'static str>) {
    let mut patches = Vec::new();

    // The runtime goes after `'use strict'` so the file does not leave strict mode.
    let runtime = format!(
        ";globalThis.__HAXFAST_ENGINE__={{version:{:?},patches:[]}};",
        haxfast_core::version()
    );
    let mut patched = insert_after_directive(original, &runtime);
    patches.push("runtime");

    // F0: marker inside the anchor of the future zero-input-tolerance patch. It does
    // not change behavior: it proves the pipeline intercepts and replaces bytes.
    if let Some(index) = patched.find("this.Od=2") {
        patched.insert_str(index + "this.Od=2".len(), "/*haxfast:f0*/");
        patches.push("f0-marker");
    } else {
        patches.push("no-anchor"); // fail-open: only the runtime is served
    }

    (patched, patches)
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn patch_marks_the_anchor_without_changing_its_value() {
        let (out, patches) = patch("'use strict';x();this.Od=2;y();");
        assert!(out.contains("this.Od=2/*haxfast:f0*/"));
        assert!(patches.contains(&"f0-marker"));
        assert!(out.starts_with("'use strict';"));
    }
}
