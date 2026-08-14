//! Consolidated integration-test harness for non-conflicting FFI exports.
#[path = "export_getset.rs"]
mod export_getset;
#[path = "export_shared_fns.rs"]
mod export_shared_fns;
#[path = "ffi_export.rs"]
pub mod ffi_export;
#[path = "ffi_export_import_u128_i128.rs"]
mod ffi_export_import_u128_i128;
#[path = "generics.rs"]
mod generics;
#[path = "transparent.rs"]
mod transparent;
#[path = "unambiguous.rs"]
mod unambiguous;
