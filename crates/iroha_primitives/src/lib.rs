//! Data primitives used inside Iroha, but not related directly to the
//! blockchain-specific data model.
//!
//! If you need a thin wrapper around a third-party library, so that
//! it can be used in `IntoSchema`, as well as [`norito::codec`]'s
//! `Encode` and `Decode` trait implementations, you should add the
//! wrapper as a submodule to this crate, rather than into
//! `iroha_data_model` directly.
#![deny(warnings)]
pub mod addr;
pub mod big_numeric;
pub mod bigint;
pub mod calendar;
pub mod cmpext;
#[cfg(not(feature = "ffi_import"))]
pub mod const_vec;
#[cfg(not(feature = "ffi_import"))]
pub mod conststr;
pub mod erasure;
pub mod json;
pub mod must_use;
pub mod numeric;
pub mod small;
pub mod soradns;
pub mod time;
pub mod unique_vec;

#[cfg(feature = "json")]
/// Re-export Norito's JSON derive macros for use within the crate.
pub mod json_macros {
    pub use norito::derive::{JsonDeserialize, JsonSerialize};
}

#[cfg(not(feature = "json"))]
/// Placeholder when the `json` feature is disabled.
pub mod json_macros {}

pub use crate::{
    big_numeric::{BigNumeric, BigNumericError},
    bigint::{BigInt, BigIntError, MAX_BITS as BIGINT_MAX_BITS},
    numeric::{
        Numeric, NumericError, NumericSpec, NumericSpecError, NumericSpecParseError,
        TryFromNumericError, numeric,
    },
};

mod ffi {
    //! Definitions and implementations of FFI related functionalities

    macro_rules! ffi_item {
        ($it: item $($attr: meta)?) => {
            #[cfg(all(not(feature = "ffi_export"), not(feature = "ffi_import")))]
            $it

            #[cfg(all(feature = "ffi_export", not(feature = "ffi_import")))]
            #[derive(iroha_ffi::FfiType)]
            #[iroha_ffi::ffi_export]
            $(#[$attr])?
            $it

            #[cfg(feature = "ffi_import")]
            iroha_ffi::ffi! {
                #[iroha_ffi::ffi_import]
                $(#[$attr])?
                $it
            }
        };
    }

    pub(crate) use ffi_item;
}
