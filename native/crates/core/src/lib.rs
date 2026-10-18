//! haxfast-core — pure logic of the HaxFast client.
//!
//! No CEF, no DOM, no networking of its own: everything platform-independent
//! lives here, because it is the seed of the future native client (track R / E3).

pub mod prefs;

/// Core version (used in logs and in the bridge handshake).
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_not_empty() {
        assert!(!super::version().is_empty());
    }
}
