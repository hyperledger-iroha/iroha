//! Fixed Android shared-library alignment for all maintained JNI ABIs.

fn main() {
    if matches!(
        std::env::var("CARGO_CFG_TARGET_OS").as_deref(),
        Ok("android")
    ) {
        println!("cargo:rustc-link-arg-cdylib=-Wl,-z,max-page-size=16384");
    }
}
