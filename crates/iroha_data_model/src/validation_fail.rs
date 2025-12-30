//! Legacy re-exports for validation failure data structures.
//!
//! These types now live in the executor module; this file remains to avoid churn
//! for call sites that still import them via `iroha_data_model::validation_fail`.

#[cfg(any(feature = "transparent_api", feature = "ffi_import"))]
pub use crate::executor::{
    DecodedCodeSizeLimitInfo, DecodedInstructionLimitInfo, ManifestAbiHashMismatchInfo,
    ManifestCodeHashMismatchInfo, MaxCyclesExceedsFuelInfo, MaxCyclesExceedsUpperBoundInfo,
    VectorLengthTooLargeInfo,
};
pub use crate::executor::{IvmAdmissionError, UnsupportedVersionInfo, ValidationFail};
