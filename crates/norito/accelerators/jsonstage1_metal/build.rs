fn main() {
    // Declare an explicit input set so non-Metal targets do not rerun this
    // script on every package file change.
    println!("cargo:rerun-if-changed=build.rs");
    // Build scripts run on the host; gate on the compilation target so cross
    // builds match the `cfg(all(target_os = "macos", target_arch = "aarch64"))`
    // gates in `src/`.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if target_os == "macos" && target_arch == "aarch64" {
        println!("cargo:rerun-if-changed=src/metal.m");
        println!("cargo:rustc-link-lib=framework=Metal");
        println!("cargo:rustc-link-lib=framework=Foundation");
        let mut build = cc::Build::new();
        build.file("src/metal.m");
        build.flag("-fobjc-arc");
        build.compile("jsonstage1_metal_objc");
    }
}
