//! Client paths (XDG) and page bundle lookup.

use std::path::PathBuf;

/// `~/.config/haxfast`. `$HAXFAST_HOME` overrides it (tests with isolated data).
pub fn user_data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("HAXFAST_HOME") {
        return PathBuf::from(dir);
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        });
    base.join("haxfast")
}

pub fn prefs_file() -> PathBuf {
    user_data_dir().join("preferences.json")
}

pub fn logs_dir() -> PathBuf {
    user_data_dir().join("logs")
}

pub fn log_file() -> PathBuf {
    logs_dir().join("haxfast.log")
}

pub fn cef_log_file() -> PathBuf {
    logs_dir().join("cef.log")
}

pub fn cache_dir() -> PathBuf {
    user_data_dir().join("cef-cache")
}

/// `dist/page.js`: tries `$HAXFAST_PAGE_BUNDLE`, the current directory and paths
/// relative to the executable (in dev: `native/target/debug` → `<repo>/dist`).
pub fn page_bundle() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("HAXFAST_PAGE_BUNDLE") {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }

    let mut candidates = vec![PathBuf::from("dist/page.js")];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("dist/page.js"));
            candidates.push(dir.join("../../../dist/page.js"));
        }
    }
    candidates.into_iter().find(|path| path.is_file())
}

/// Chromium remote debugging port (0 = disabled): `HAXFAST_REMOTE_DEBUG=9222`.
pub fn remote_debug_port() -> i32 {
    std::env::var("HAXFAST_REMOTE_DEBUG")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}
