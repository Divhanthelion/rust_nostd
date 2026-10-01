// Link flags for a freestanding Linux executable. Using `rustc-link-arg-bins`
// (instead of global RUSTFLAGS) keeps build scripts and proc-macros, which are
// normal host programs, linking normally.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        // No crt1.o/crti.o: our `_start` (in nostd_rt) is the entry point.
        println!("cargo:rustc-link-arg-bins=-nostartfiles");
        // A static, non-PIE executable: no dynamic loader is involved at all.
        println!("cargo:rustc-link-arg-bins=-static");
        println!("cargo:rustc-link-arg-bins=-no-pie");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
