//! Build script
//!
//! Warn if `ffi_import` and `ffi_export` features are active at the same time

fn main() {
    let ffi_import = std::env::var_os("CARGO_FEATURE_FFI_IMPORT").is_some();
    let ffi_export = std::env::var_os("CARGO_FEATURE_FFI_EXPORT").is_some();

    if ffi_import && ffi_export {
        println!("cargo:warning=Features `ffi_export` and `ffi_import` are mutually exclusive");
        println!("cargo:warning=When both active, `ffi_import` feature takes precedence");
    }
}
