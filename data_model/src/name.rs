//! This module contains [`Name`](`crate::name::Name`) structure
//! and related implementations and trait implementations.
#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec::Vec};
use core::{ops::RangeInclusive, str::FromStr};

use derive_more::{DebugCustom, Display};
use iroha_ffi::{IntoFfi, TryFromFfi};
use iroha_primitives::conststr::ConstString;
use iroha_schema::IntoSchema;
use parity_scale_codec::{Decode, Encode, Input};
use serde::{Deserialize, Serialize};

use crate::{ParseError, ValidationError};

/// `Name` struct represents type for Iroha Entities names, like
/// [`Domain`](`crate::domain::Domain`)'s name or
/// [`Account`](`crate::account::Account`)'s name.
#[derive(
    DebugCustom,
    Display,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Serialize,
    IntoSchema,
    IntoFfi,
    TryFromFfi,
)]
#[repr(transparent)]
// TODO: This struct doesn't have to be opaque
pub struct Name(ConstString);

impl Name {
    /// Check if `range` contains the number of chars in the inner `ConstString` of this [`Name`].
    ///
    /// # Errors
    /// Fails if `range` does not
    pub fn validate_len(
        &self,
        range: impl Into<RangeInclusive<usize>>,
    ) -> Result<(), ValidationError> {
        let range = range.into();
        if range.contains(&self.as_ref().chars().count()) {
            Ok(())
        } else {
            Err(ValidationError::new(&format!(
                "Name must be between {} and {} characters in length.",
                &range.start(),
                &range.end()
            )))
        }
    }

    /// Check if `candidate` string would be valid [`Name`].
    ///
    /// # Errors
    /// Fails if not valid [`Name`].
    fn validate_str(candidate: &str) -> Result<(), ParseError> {
        if candidate.is_empty() {
            return Err(ParseError {
                reason: "`Name` cannot be empty",
            });
        }
        if candidate.chars().any(char::is_whitespace) {
            return Err(ParseError {
                reason: "White space not allowed in `Name` constructs",
            });
        }
        if candidate.chars().any(|ch| ch == '@' || ch == '#') {
            #[allow(clippy::non_ascii_literal)]
            return Err(ParseError {
                reason: "The `@` character is reserved for `account@domain` constructs, `#` — for `asset#domain`",
            });
        }
        Ok(())
    }
}

impl AsRef<str> for Name {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl FromStr for Name {
    type Err = ParseError;

    fn from_str(candidate: &str) -> Result<Self, Self::Err> {
        Self::validate_str(candidate).map(|_| Self(ConstString::from(candidate)))
    }
}

/// FFI function equivalent of [`Name::from_str`]
///
/// # Safety
///
/// All of the given pointers must be valid
#[no_mangle]
#[allow(non_snake_case, unsafe_code)]
pub unsafe extern "C" fn Name__from_str(
    candidate: *const u8,
    candidate_len: usize,
    output: *mut *mut Name,
) -> iroha_ffi::FfiResult {
    let res = std::panic::catch_unwind(|| {
        let candidate = core::slice::from_raw_parts(candidate, candidate_len);

        let method_res = match core::str::from_utf8(candidate) {
            // TODO: Implement error handling (https://github.com/hyperledger/iroha/issues/2252)
            Err(_error) => return iroha_ffi::FfiResult::Utf8Error,
            Ok(candidate) => Name::from_str(candidate),
        };
        let method_res = Box::into_raw(Box::new(match method_res {
            Err(_error) => return iroha_ffi::FfiResult::ExecutionFail,
            Ok(method_res) => method_res,
        }));

        output.write(method_res);
        iroha_ffi::FfiResult::Ok
    });

    match res {
        Ok(res) => res,
        Err(_) => {
            // TODO: Implement error handling (https://github.com/hyperledger/iroha/issues/2252)
            iroha_ffi::FfiResult::UnrecoverableError
        }
    }
}

impl<'de> Deserialize<'de> for Name {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;

        let name = ConstString::deserialize(deserializer)?;
        Self::validate_str(&name)
            .map(|_| Self(name))
            .map_err(D::Error::custom)
    }
}
impl Decode for Name {
    fn decode<I: Input>(input: &mut I) -> Result<Self, parity_scale_codec::Error> {
        let name = ConstString::decode(input)?;
        Self::validate_str(&name)
            .map(|_| Self(name))
            .map_err(|error| error.reason.into())
    }
}

/// The prelude re-exports most commonly used traits, structs and macros from this crate.
pub mod prelude {
    pub use super::Name;
}

#[cfg(test)]
mod tests {
    #![allow(clippy::restriction)]

    use super::*;

    const INVALID_NAMES: [&str; 4] = ["", " ", "@", "#"];

    #[test]
    fn deserialize_name() {
        for invalid_name in INVALID_NAMES {
            let invalid_name = Name(invalid_name.to_owned().into());
            let serialized = serde_json::to_string(&invalid_name).expect("Valid");
            let name = serde_json::from_str::<Name>(serialized.as_str());

            assert!(name.is_err());
        }
    }

    #[test]
    fn decode_name() {
        for invalid_name in INVALID_NAMES {
            let invalid_name = Name(invalid_name.to_owned().into());
            let bytes = invalid_name.encode();
            let name = Name::decode(&mut &bytes[..]);

            assert!(name.is_err());
        }
    }

    #[test]
    #[allow(unsafe_code)]
    fn ffi_name_from_str() -> Result<(), ParseError> {
        use crate::ffi::{Handle, __drop};

        let candidate = "Name";
        let candidate_bytes = candidate.as_bytes();
        let candidate_bytes_len = candidate_bytes.len();

        unsafe {
            let mut name = core::mem::MaybeUninit::new(core::ptr::null_mut());

            assert_eq!(
                iroha_ffi::FfiResult::Ok,
                Name__from_str(
                    candidate_bytes.as_ptr(),
                    candidate_bytes_len,
                    name.as_mut_ptr()
                )
            );

            let name = name.assume_init();
            assert_ne!(core::ptr::null_mut(), name);
            assert_eq!(Name::from_str(candidate)?, *name);

            assert_eq!(iroha_ffi::FfiResult::Ok, __drop(Name::ID, name.cast()));
        }

        Ok(())
    }
}
