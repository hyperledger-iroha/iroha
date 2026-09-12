//! Canonical shared FFI operations for base-owned opaque handles.

use crate::metadata::Metadata;

// Preserve Metadata's assigned ID while moving its implementation/export owner.
// IDs are scoped to these dispatchers; no aggregate handle is accepted here.
iroha_ffi::handles! { 3, Metadata }
iroha_ffi::def_ffi_fns! { link_prefix = "iroha_model_base"
    Drop: { Metadata },
    Clone: { Metadata },
    Eq: { Metadata },
    Ord: { Metadata },
}
