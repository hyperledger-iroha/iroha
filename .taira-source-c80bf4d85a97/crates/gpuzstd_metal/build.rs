fn main() {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        println!("cargo:rerun-if-changed=src/metal.m");
        println!("cargo:rustc-link-lib=framework=Metal");
        println!("cargo:rustc-link-lib=framework=Foundation");
        let mut build = cc::Build::new();
        build.file("src/metal.m");
        build.flag("-fobjc-arc");
        build.compile("gpuzstd_metal_objc");
    }
}
