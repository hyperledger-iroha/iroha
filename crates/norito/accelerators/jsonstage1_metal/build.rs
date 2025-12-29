fn main() {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        println!("cargo:rustc-link-lib=framework=Metal");
        println!("cargo:rustc-link-lib=framework=Foundation");
        let mut build = cc::Build::new();
        build.file("src/metal.m");
        build.flag("-fobjc-arc");
        build.compile("jsonstage1_metal_objc");
    }
}
