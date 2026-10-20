//! Ad network blocklist for the shell page.
//!
//! `haxball.com/play` is the official shell: outside the game iframe it wraps a
//! `div.rightbar` that injects cpmstar, which in turn pulls a long tail of ad
//! and cookie-sync networks. Blocking them here is the part of the F2 cleanup
//! that actually removes work (network, CPU) instead of just hiding pixels;
//! the page layer also hides the empty bar (fail-open either way).
//!
//! Only third-party ad/sync domains are listed. Never add: haxball.com,
//! p2p.haxball.com, google.com/gstatic.com (reCAPTCHA), fonts.googleapis.com,
//! or loopback (the bridge).

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use crate::log;

/// Suffix-matched against the request host. Observed live on 2026-10-20 plus the
/// ad networks REA saw in the sidebar; extend as new ones appear.
const AD_DOMAINS: &[&str] = &[
    // cpmstar: the sidebar script itself and its CDN/proxy hosts
    "cpmstar.com",
    // observed live
    "googlesyndication.com",
    "doubleclick.net",
    "id5-sync.com",
    "onetag-sys.com",
    "rubiconproject.com",
    "a-mo.net",
    "richaudience.com",
    "bidswitch.net",
    "3lift.com",
    "crwdcntrl.net",
    "360yield.com",
    "everesttech.net",
    "lijit.com",
    "2mdn.net",
    "yieldmo.com",
    "adroll.com",
    "smartadserver.com",
    "kargo.com",
    // seen in previous sessions / REA
    "doubleverify.com",
    "bannerflow.net",
    "googletagservices.com",
    "casalemedia.com",
    "script.ac",
    "ad-score.com",
    "delivrdsp.ai",
    "openx.net",
    "sportradar.com",
];

/// Ad domain matching `url`, if any.
pub fn ad_domain(url: &str) -> Option<&'static str> {
    let host = host_of(url)?;
    AD_DOMAINS
        .iter()
        .copied()
        .find(|domain| matches_host(&host, domain))
}

/// Logs the first blocked request per domain (the tail is long and noisy).
pub fn log_blocked(domain: &'static str) {
    static SEEN: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let first = SEEN
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .map(|mut seen| seen.insert(domain))
        .unwrap_or(false);
    if first {
        log::info(&format!("ads: blocking {domain}"));
    }
}

/// Lowercased host of an absolute URL, without userinfo or port.
fn host_of(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    let host = rest.split(['/', '?', '#']).next()?;
    let host = host.rsplit_once('@').map_or(host, |(_, after)| after);
    let host = host.split_once(':').map_or(host, |(before, _)| before);
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// True for `domain` itself or any subdomain of it, with a dot boundary.
fn matches_host(host: &str, domain: &str) -> bool {
    host == domain
        || host
            .strip_suffix(domain)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_ads_and_subdomains() {
        assert!(ad_domain("https://server.cpmstar.com/view.aspx?poolid=26640").is_some());
        assert!(ad_domain("https://ssl.cdne.cpmstar.com/x").is_some());
        assert!(ad_domain("https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js").is_some());
        assert!(ad_domain("https://googleads.g.doubleclick.net/pagead/id").is_some());
        assert!(ad_domain("HTTPS://SERVER.CPMSTAR.COM/x").is_some());
    }

    #[test]
    fn keeps_the_game_and_its_dependencies() {
        assert!(ad_domain("https://www.haxball.com/play").is_none());
        assert!(ad_domain("https://www.haxball.com/H4mQWPXF/__cache_static__/g/game.html").is_none());
        assert!(ad_domain("https://p2p.haxball.com/").is_none());
        assert!(ad_domain("https://www.google.com/recaptcha/api.js").is_none());
        assert!(ad_domain("https://www.gstatic.com/recaptcha/releases/x.js").is_none());
        assert!(ad_domain("https://fonts.googleapis.com/css?family=Open+Sans").is_none());
        assert!(ad_domain("ws://127.0.0.1:8765/").is_none());
    }

    #[test]
    fn suffix_needs_a_dot_boundary() {
        assert!(ad_domain("https://notcpmstar.com/").is_none());
        assert!(ad_domain("https://cpmstar.com.evil.test/").is_none());
        assert!(ad_domain("https://cpmstar.com/").is_some());
    }
}
