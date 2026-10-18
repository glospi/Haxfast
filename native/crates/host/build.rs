// cef-dll-sys copies `libcef.so` (plus locales/resources) next to the binary, but
// the Linux loader does not look there: with `$ORIGIN` the executable finds
// itself and loads libcef from its own folder (dev and future AppImage).
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN");
    }
}
