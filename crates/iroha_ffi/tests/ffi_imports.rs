//! Consolidated integration-test harness for non-conflicting FFI imports.

use iroha_ffi::decl_ffi_fns;

// `ffi_import_opaque` expands opaque-handle methods against these crate-root
// declarations. Keep the declarations at the harness root while the concrete
// exports remain owned by that module's fixture implementation.
decl_ffi_fns! { Drop, Clone, Eq }

#[path = "ffi_import.rs"]
mod ffi_import;
#[path = "ffi_import_opaque.rs"]
mod ffi_import_opaque;
