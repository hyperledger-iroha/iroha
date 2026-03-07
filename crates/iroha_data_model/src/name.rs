//! This module contains [`Name`](`crate::name::Name`) structure
//! and related implementations and trait implementations.
use std::{
    borrow::{Borrow, Cow},
    format,
    str::FromStr,
    string::String,
    sync::OnceLock,
    vec::Vec,
};

use idna::{
    AsciiDenyList,
    uts46::{DnsLength, Hyphens, Uts46},
};
use iroha_data_model_derive::model;
use iroha_primitives::conststr::ConstString;
use norito::core::{DecodeFromSlice, Error as NoritoError};

const ERR_DOMAIN_NORMALISATION: &str = "domain name failed UTS-46 STD3 normalization requirements";

use icu_normalizer::{ComposingNormalizer, ComposingNormalizerBorrowed};

pub use self::model::*;
use crate::error::ParseError;

type NormalizerCell = OnceLock<ComposingNormalizerBorrowed<'static>>;

/// Lazily initialized NFC normalizer shared across [`Name`] parsing.
static NFC_NORMALIZER: NormalizerCell = NormalizerCell::new();

#[model]
mod model {
    use derive_more::{Debug, Display};
    use iroha_schema::IntoSchema;

    use super::*;

    /// `Name` struct represents the type of Iroha Entities names, such as
    /// [`Domain`](`crate::domain::Domain`) name or
    /// [`Account`](`crate::account::Account`) name.
    #[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, IntoSchema)]
    #[repr(transparent)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub struct Name(pub(super) ConstString);
}

impl Name {
    /// Check if `candidate` string would be valid [`Name`].
    ///
    /// # Errors
    /// Fails if not valid [`Name`].
    fn validate_str(candidate: &str) -> Result<(), ParseError> {
        const FORBIDDEN_CHARS: [char; 3] = ['@', '#', '$'];

        if candidate.is_empty() {
            return Err(ParseError {
                reason: "Empty `Name`",
            });
        }
        if candidate.chars().any(char::is_whitespace) {
            return Err(ParseError {
                reason: "White space not allowed in `Name` constructs",
            });
        }
        if candidate.chars().any(|ch| FORBIDDEN_CHARS.contains(&ch)) {
            #[allow(clippy::non_ascii_literal)]
            return Err(ParseError {
                reason: "The `@` character is reserved for `account@domain` constructs, \
                        `#` for `asset#domain`, and `$` — for `nft$domain`.",
            });
        }
        Ok(())
    }

    /// Return a canonical form of the input string according to the Name normalization policy.
    ///
    /// Applies ICU-backed NFC composition so canonically equivalent sequences share the same
    /// representation (for example, `e\u{0301}` becomes `é`) on every platform. See
    /// `roadmap.md` ("Names/IDs").
    fn normalize(candidate: &str) -> Cow<'_, str> {
        // Use ICU compiled data to apply NFC normalization deterministically
        // across platforms. This preserves compatibility forms but composes
        // canonically equivalent sequences (e.g., "e\u{0301}" -> "é"). The
        // normalizer construction is relatively expensive, so cache a single
        // instance and reuse it for every invocation.
        NFC_NORMALIZER
            .get_or_init(ComposingNormalizer::new_nfc)
            .normalize(candidate)
    }

    /// Returns true if this name is reserved for internal use.
    ///
    /// Currently reserves `"genesis"` (case-insensitive) to help prevent
    /// accidental misuse; enforcement is context-dependent.
    pub fn is_reserved(&self) -> bool {
        self.0.as_ref().eq_ignore_ascii_case("genesis")
    }
}

impl norito::core::NoritoSerialize for Name {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        <&str as norito::core::NoritoSerialize>::serialize(&self.as_ref(), writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        <&str as norito::core::NoritoSerialize>::encoded_len_hint(&self.as_ref())
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        <&str as norito::core::NoritoSerialize>::encoded_len_exact(&self.as_ref())
    }
}

impl<'a> norito::core::NoritoDeserialize<'a> for Name {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("Name deserialization must succeed for valid archives")
    }

    fn try_deserialize(
        archived: &'a norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        if let Ok(payload) = norito::core::payload_slice_from_ptr(ptr) {
            #[cfg(debug_assertions)]
            if norito::debug_trace_enabled() {
                let preview_len = core::cmp::min(payload.len(), 32);
                eprintln!(
                    "Name::try_deserialize payload len={} preview={:?}",
                    payload.len(),
                    &payload[..preview_len]
                );
            }
            match norito::core::decode_field_canonical::<String>(payload) {
                Ok((value, _used)) => {
                    return Name::from_str(&value)
                        .map_err(|err| norito::core::Error::Message(err.reason.into()));
                }
                Err(norito::core::Error::LengthMismatch) => {
                    if let Ok(raw) = core::str::from_utf8(payload) {
                        #[cfg(debug_assertions)]
                        if norito::debug_trace_enabled() {
                            eprintln!("Name::try_deserialize fallback raw={raw}");
                        }
                        return Name::from_str(raw)
                            .map_err(|err| norito::core::Error::Message(err.reason.into()));
                    }
                }
                Err(err) => return Err(err),
            }
        }

        let string = norito::core::NoritoDeserialize::deserialize(archived.cast::<String>());
        Name::from_str(string.as_str())
            .map_err(|err| norito::core::Error::Message(err.reason.into()))
    }
}

impl AsRef<str> for Name {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl Borrow<str> for Name {
    fn borrow(&self) -> &str {
        self.0.as_ref()
    }
}

impl FromStr for Name {
    type Err = ParseError;

    fn from_str(candidate: &str) -> Result<Self, Self::Err> {
        Self::validate_str(candidate)?;
        let normalized = Self::normalize(candidate);
        Ok(Self(ConstString::from(&*normalized)))
    }
}

impl TryFrom<String> for Name {
    type Error = ParseError;

    fn try_from(candidate: String) -> Result<Self, Self::Error> {
        Self::validate_str(&candidate)?;
        let normalized = Self::normalize(&candidate);
        Ok(Self(ConstString::from(&*normalized)))
    }
}

impl<'a> DecodeFromSlice<'a> for Name {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), NoritoError> {
        let (value, used) = norito::core::decode_field_canonical::<String>(bytes)?;
        let name = Name::from_str(&value).map_err(|err| NoritoError::Message(err.reason.into()))?;
        Ok((name, used))
    }
}

/// Canonicalise a domain label using UTS-46 STD3 rules and ASCII folding.
///
/// # Errors
/// Returns [`ParseError`] when the label cannot be normalised or violates
/// the allowed character set for domain identifiers.
pub fn canonicalize_domain_label(raw: &str) -> Result<String, ParseError> {
    Name::validate_str(raw)?;
    let normalized = Name::normalize(raw);
    reject_disallowed_unicode(normalized.as_ref())?;
    let ascii = Uts46::new()
        .to_ascii(
            normalized.as_ref().as_bytes(),
            AsciiDenyList::EMPTY,
            Hyphens::Check,
            DnsLength::Verify,
        )
        .map_err(|_| ParseError::new(ERR_DOMAIN_NORMALISATION))?
        .into_owned();
    let mut label = ascii;
    label.make_ascii_lowercase();
    if label.is_empty()
        || label.bytes().any(|byte| {
            !byte.is_ascii_alphanumeric() && byte != b'-' && byte != b'_' && byte != b'.'
        })
    {
        return Err(ParseError::new(ERR_DOMAIN_NORMALISATION));
    }
    Ok(label)
}

fn reject_disallowed_unicode(label: &str) -> Result<(), ParseError> {
    if label
        .chars()
        .any(|ch| matches!(ch, '\u{1E00}'..='\u{1EFF}'))
    {
        return Err(ParseError::new(ERR_DOMAIN_NORMALISATION));
    }
    Ok(())
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for Name {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(self.as_ref(), out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for Name {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        Name::from_str(&value).map_err(|err| norito::json::Error::Message(err.reason.into()))
    }
}

// Norito deserialization is derived via `Decode` above.
// DecodeFromSlice is provided via a crate-level shim in `norito_slice_decode.rs`.

/// The prelude re-exports most commonly used traits, structs and macros from this crate.
pub mod prelude {
    pub use super::Name;
}

#[cfg(test)]
mod tests {
    use std::borrow::ToOwned as _;

    use norito::codec::{Decode, Encode};

    use super::*;
    // Trait import not required; tests roundtrip via header-framed helpers.

    const INVALID_NAMES: [&str; 4] = ["", " ", "@", "#"];

    #[cfg(feature = "json")]
    #[test]
    fn deserialize_name() {
        for invalid_name in INVALID_NAMES {
            let invalid_name = Name(invalid_name.to_owned().into());
            let serialized = norito::json::to_json(&invalid_name).expect("Valid");
            let name = norito::json::from_str::<Name>(serialized.as_str());

            assert!(name.is_err());
        }
    }

    #[test]
    fn decode_name() {
        // Limit to valid strings for roundtrip via codec
        let valid = ["valid", "hello", "abc123", "é" /* NFC composed */];
        for s in valid {
            let name = Name::from_str(s).expect("valid");
            // Use stable header-framed Norito over String, then parse back to Name
            let bytes = norito::to_bytes(&s.to_string()).expect("encode str");
            let archived = norito::from_bytes::<String>(&bytes).expect("archived str");
            let decoded_s = norito::core::NoritoDeserialize::deserialize(archived);
            assert_eq!(decoded_s, s);
            let reparsed = Name::from_str(&decoded_s).expect("parse back");
            assert_eq!(reparsed, name);
        }
    }

    #[test]
    fn name_rejects_account_literal_text() {
        let err = Name::from_str("alice@hbl")
            .expect_err("account-style literal must not be accepted as Name");
        assert!(
            err.to_string().contains("`@` character"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn norito_roundtrip_preserves_value() {
        let samples = ["wonderland", "éclair", "genesis-domain"];
        for raw in samples {
            let name = Name::from_str(raw).expect("valid name");
            let bytes = Encode::encode(&name);
            let mut cursor = bytes.as_slice();
            let decoded = Name::decode(&mut cursor).expect("decode name");
            assert_eq!(decoded, name);
            assert!(cursor.is_empty(), "decoder must consume entire buffer");
        }
    }

    #[test]
    fn invalid_names_rejected() {
        for s in INVALID_NAMES {
            assert!(Name::from_str(s).is_err(), "should reject: {s:?}");
        }
    }

    #[test]
    fn reserved_name_detection_is_case_insensitive() {
        assert!(Name::from_str("genesis").unwrap().is_reserved());
        assert!(Name::from_str("Genesis").unwrap().is_reserved());
        assert!(Name::from_str("GENESIS").unwrap().is_reserved());
        assert!(!Name::from_str("genesisx").unwrap().is_reserved());
    }

    #[test]
    fn whitespace_is_rejected_anywhere() {
        for s in [" leading", "trailing ", "in side", " "] {
            assert!(Name::from_str(s).is_err(), "should reject: {s:?}");
        }
    }

    #[test]
    fn canonicalize_domain_label_lowercases_ascii() {
        let canonical = canonicalize_domain_label("Treasury").expect("ASCII domains canonicalize");
        assert_eq!(canonical, "treasury");
    }

    #[test]
    fn canonicalize_domain_label_produces_punycode() {
        let canonical = canonicalize_domain_label("例え").expect("punycode conversion");
        assert_eq!(canonical, "xn--r8jz45g");
    }

    #[test]
    fn canonicalize_domain_label_rejects_invalid_chars() {
        assert!(canonicalize_domain_label("bad label").is_err());
    }

    #[test]
    fn canonicalize_domain_label_accepts_multilabel_idn() {
        let canonical =
            canonicalize_domain_label("例え.テスト").expect("multilabel IDNs canonicalize");
        assert_eq!(canonical, "xn--r8jz45g.xn--zckzah");
    }

    #[test]
    fn canonicalize_domain_label_rejects_extended_latin_letters() {
        assert!(canonicalize_domain_label("wÍḷd-card").is_err());
    }

    #[test]
    fn canonicalize_domain_label_allows_latin1_supplement_letters() {
        let canonical =
            canonicalize_domain_label("bücher.example").expect("latin-1 diacritics allowed");
        assert_eq!(canonical, "xn--bcher-kva.example");
    }

    #[test]
    fn nfc_normalization_applies() {
        // "e" + combining acute accent
        let decomposed = "e\u{0301}";
        let composed = "é";
        let name1 = Name::from_str(decomposed).expect("valid");
        let name2 = Name::from_str(composed).expect("valid");
        assert_eq!(name1, name2);
        assert_eq!(name1.as_ref(), composed);
    }
}
