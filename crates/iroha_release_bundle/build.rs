//! Record the compiler-selected native target for manifest admission.

fn main() {
    let target = std::env::var("TARGET").expect("Cargo supplies the compilation target");
    println!("cargo:rustc-env=IROHA_RELEASE_NATIVE_TARGET={target}");
    println!("cargo:rerun-if-changed=build.rs");
}
