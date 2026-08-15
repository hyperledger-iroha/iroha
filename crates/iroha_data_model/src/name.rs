//! This module contains [`Name`](`crate::name::Name`) structure
//! and related implementations and trait implementations.
use icu_normalizer::{ComposingNormalizer, ComposingNormalizerBorrowed, provider::Baked};
use idna::{
    AsciiDenyList,
    uts46::{DnsLength, Hyphens, Uts46},
};
use iroha_data_model_derive::model;
use iroha_primitives::conststr::ConstString;
use norito::core::{DecodeFromSlice, Error as NoritoError};
use sha2::{Digest as _, Sha256};
use std::{
    borrow::{Borrow, Cow},
    format,
    str::FromStr,
    string::String,
    sync::OnceLock,
    vec::Vec,
};
const ERR_DOMAIN_NORMALISATION: &str = "domain name failed UTS-46 STD3 normalization requirements";
const ERR_NFC_PROFILE: &str =
    "compiled NFC normalization data does not match the consensus profile";
pub use self::model::*;
use crate::error::ParseError;
/// Maximum UTF-8 byte length of a canonical [`Name`].
///
/// The bound matches the DNS-style full-name ceiling already used by domain
/// normalization. Counting bytes, rather than Unicode scalar values, also
/// bounds the canonical Norito representation independently of platform.
pub const MAX_NAME_BYTES: usize = 255;
const EXPECTED_NFC_DATA_SHA256: [u8; 32] = [
    0xbe, 0x71, 0xcd, 0xd0, 0x40, 0x2b, 0x3d, 0xf5, 0x8c, 0x10, 0xe7, 0xbd, 0x33, 0x3d, 0xb0, 0x59,
    0x95, 0x13, 0x5c, 0xf3, 0x70, 0x4f, 0xa3, 0x7b, 0x1e, 0xb9, 0x2f, 0xa0, 0x4a, 0x18, 0x7d, 0x2e,
];
type NormalizerCell = OnceLock<Result<ComposingNormalizerBorrowed<'static>, ()>>;
/// Lazily initialized, profile-checked NFC normalizer shared across [`Name`] parsing.
static NFC_NORMALIZER: NormalizerCell = NormalizerCell::new();
fn hash_nfc_data_field(hasher: &mut Sha256, label: &[u8], bytes: &[u8]) {
    hasher.update(
        u64::try_from(label.len())
            .expect("field label length fits u64")
            .to_le_bytes(),
    );
    hasher.update(label);
    hasher.update(
        u64::try_from(bytes.len())
            .expect("field length fits u64")
            .to_le_bytes(),
    );
    hasher.update(bytes);
}
/// Fingerprint every baked table used by ICU4X NFC normalization.
fn nfc_data_sha256() -> [u8; 32] {
    let mut hasher = Sha256::new();
    let nfd = Baked::SINGLETON_NORMALIZER_NFD_DATA_V1;
    hasher.update(b"nfd-ranges-v1");
    for range in nfd.trie.iter_ranges() {
        hasher.update(range.range.start().to_le_bytes());
        hasher.update(range.range.end().to_le_bytes());
        hasher.update(range.value.to_le_bytes());
    }
    hasher.update(nfd.passthrough_cap.to_le_bytes());
    let tables = Baked::SINGLETON_NORMALIZER_NFD_TABLES_V1;
    hash_nfc_data_field(
        &mut hasher,
        b"nfd-scalars16-v1",
        tables.scalars16.as_bytes(),
    );
    hash_nfc_data_field(
        &mut hasher,
        b"nfd-scalars24-v1",
        tables.scalars24.as_bytes(),
    );
    let nfc = Baked::SINGLETON_NORMALIZER_NFC_V1;
    hash_nfc_data_field(
        &mut hasher,
        b"nfc-compositions-v1",
        nfc.canonical_compositions.data.as_bytes(),
    );
    hasher.finalize().into()
}
fn checked_nfc_normalizer(
    expected_data_sha256: &[u8; 32],
) -> Result<ComposingNormalizerBorrowed<'static>, ()> {
    if nfc_data_sha256() == *expected_data_sha256 {
        Ok(ComposingNormalizer::new_nfc())
    } else {
        Err(())
    }
}
fn nfc_normalizer() -> Result<&'static ComposingNormalizerBorrowed<'static>, ParseError> {
    NFC_NORMALIZER
        .get_or_init(|| checked_nfc_normalizer(&EXPECTED_NFC_DATA_SHA256))
        .as_ref()
        .map_err(|()| ParseError::new(ERR_NFC_PROFILE))
}
#[model]
mod model {
    use super::*;
    use derive_more::{Debug, Display};
    use iroha_schema::IntoSchema;
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
        if candidate.len() > MAX_NAME_BYTES {
            return Err(ParseError {
                reason: "`Name` exceeds the 255-byte UTF-8 limit",
            });
        }
        if candidate.chars().any(char::is_control) {
            return Err(ParseError {
                reason: "Unicode control characters are not allowed in `Name` constructs",
            });
        }
        if candidate.chars().any(is_bidi_control) {
            return Err(ParseError {
                reason: "Unicode bidirectional control characters are not allowed in `Name` constructs",
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
                reason: "The `@` character is reserved for scoped alias/public-key constructs, \
                        `#` for alias separators (for example `name#domain.dataspace`), and `$` — for `nft$domain`.",
            });
        }
        Ok(())
    }
    /// Return a canonical form of the input string according to the Name normalization policy.
    ///
    /// Applies ICU-backed NFC composition so canonically equivalent sequences share the same
    /// representation (for example, `e\u{0301}` becomes `é`) on every platform.
    ///
    /// The manifest pins the normalization algorithm exactly and the baked tables are checked
    /// against a reviewed semantic fingerprint because this output is consensus-visible. Any
    /// intentional data upgrade must also update the fingerprint and regression corpus below.
    pub(crate) fn normalize(candidate: &str) -> Result<Cow<'_, str>, ParseError> {
        // Use ICU compiled data to apply NFC normalization deterministically
        // across platforms. This preserves compatibility forms but composes
        // canonically equivalent sequences (e.g., "e\u{0301}" -> "é"). The
        // normalizer construction is relatively expensive, so cache a single
        // instance and reuse it for every invocation.
        Ok(nfc_normalizer()?.normalize(candidate))
    }
    fn parse(candidate: &str) -> Result<Self, ParseError> {
        Self::validate_str(candidate)?;
        let normalized = Self::normalize(candidate)?;
        Self::validate_str(normalized.as_ref())?;
        Ok(Self(ConstString::from(normalized.as_ref())))
    }
    fn decode_wire(bytes: &[u8]) -> Result<(Self, usize), NoritoError> {
        let (len, header_len) = norito::core::inspect_len_from_slice(bytes)?;
        if len > MAX_NAME_BYTES {
            return Err(NoritoError::Message(
                "`Name` exceeds the 255-byte UTF-8 limit".into(),
            ));
        }
        let end = header_len
            .checked_add(len)
            .ok_or(NoritoError::LengthMismatch)?;
        let raw = bytes
            .get(header_len..end)
            .ok_or(NoritoError::LengthMismatch)?;
        let value = core::str::from_utf8(raw).map_err(|_| NoritoError::InvalidUtf8)?;
        norito::core::reserve_decode_allocation(len)?;
        let name = Self::parse(value).map_err(|error| NoritoError::Message(error.reason.into()))?;
        norito::core::note_payload_access(bytes, end);
        Ok((name, end))
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
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
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
            return Self::decode_wire(payload).map(|(name, _)| name);
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
        Self::parse(candidate)
    }
}
impl TryFrom<String> for Name {
    type Error = ParseError;
    fn try_from(candidate: String) -> Result<Self, Self::Error> {
        Self::parse(&candidate)
    }
}
impl<'a> DecodeFromSlice<'a> for Name {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), NoritoError> {
        Self::decode_wire(bytes)
    }
}
/// Canonicalise a domain label using UTS-46 STD3 rules and ASCII folding.
///
/// # Errors
/// Returns [`ParseError`] when the label cannot be normalised or violates
/// the allowed character set for domain identifiers.
pub fn canonicalize_domain_label(raw: &str) -> Result<String, ParseError> {
    Name::validate_str(raw)?;
    let normalized = Name::normalize(raw)?;
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
pub(crate) fn is_bidi_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061C}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
    )
}
#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for Name {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(self.as_ref(), out);
    }
    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::write_json_string_to(self.as_ref(), out)
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
    use super::*;
    use norito::codec::{Decode, Encode};
    use std::borrow::ToOwned as _;
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
        let err = Name::from_str("alice@banka")
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
    fn name_utf8_byte_limit_is_enforced_by_both_constructors() {
        let ascii_boundary = "a".repeat(MAX_NAME_BYTES);
        let ascii_over_limit = "a".repeat(MAX_NAME_BYTES + 1);
        let unicode_boundary = "é".repeat(MAX_NAME_BYTES / "é".len());
        let unicode_over_limit = format!("{unicode_boundary}é");
        for valid in [&ascii_boundary, &unicode_boundary] {
            let parsed = Name::from_str(valid).expect("boundary name must be accepted");
            let converted =
                Name::try_from(valid.clone()).expect("owned boundary name must be accepted");
            assert_eq!(parsed, converted);
            assert!(parsed.as_ref().len() <= MAX_NAME_BYTES);
        }
        for invalid in [&ascii_over_limit, &unicode_over_limit] {
            assert!(
                Name::from_str(invalid).is_err(),
                "borrowed constructor accepted {} UTF-8 bytes",
                invalid.len()
            );
            assert!(
                Name::try_from(invalid.clone()).is_err(),
                "owned constructor accepted {} UTF-8 bytes",
                invalid.len()
            );
        }
    }
    #[test]
    fn controls_and_bidirectional_controls_are_rejected() {
        for invalid in [
            "nul\0suffix",
            "unit\u{001F}separator",
            "delete\u{007F}",
            "c1\u{0080}control",
            "arabic\u{061C}mark",
            "left\u{200E}mark",
            "right\u{200F}mark",
            "embed\u{202A}text",
            "override\u{202E}text",
            "isolate\u{2066}text",
            "pop\u{2069}text",
        ] {
            assert!(
                Name::from_str(invalid).is_err(),
                "unsafe identifier text was accepted: {invalid:?}"
            );
        }
    }
    #[test]
    fn norito_decoders_cannot_bypass_name_validation() {
        for invalid in [
            "nul\0suffix".to_owned(),
            format!("bidi\u{202E}suffix"),
            "x".repeat(MAX_NAME_BYTES + 1),
        ] {
            let forged = Name(ConstString::from(invalid.as_str()));
            let encoded = forged.encode();
            let mut cursor = encoded.as_slice();
            assert!(
                Name::decode(&mut cursor).is_err(),
                "Norito Decode accepted invalid Name: {invalid:?}"
            );
            assert!(
                <Name as DecodeFromSlice>::decode_from_slice(&encoded).is_err(),
                "slice decoder accepted invalid Name: {invalid:?}"
            );
            let framed = norito::to_bytes(&forged).expect("encode forged Name fixture");
            assert!(
                norito::decode_from_bytes::<Name>(&framed).is_err(),
                "framed decoder accepted invalid Name: {invalid:?}"
            );
        }
    }
    #[test]
    fn slice_decoder_rejects_declared_oversize_before_body_access() {
        let mut declared_oversize = Vec::new();
        norito::core::write_len_to_vec(
            &mut declared_oversize,
            u64::try_from(MAX_NAME_BYTES + 1).expect("name limit fits u64"),
        );
        let error = <Name as DecodeFromSlice>::decode_from_slice(&declared_oversize)
            .expect_err("oversized declared Name must fail before its missing body is read");
        assert!(
            error.to_string().contains("255-byte"),
            "decoder reached a generic truncation error before the Name limit: {error}"
        );
    }
    #[cfg(feature = "json")]
    #[test]
    fn json_decoder_cannot_bypass_name_validation() {
        for invalid in [
            "\"nul\\u0000suffix\"".to_owned(),
            "\"bidi\\u202Esuffix\"".to_owned(),
            format!("\"{}\"", "x".repeat(MAX_NAME_BYTES + 1)),
        ] {
            assert!(
                norito::json::from_str::<Name>(&invalid).is_err(),
                "JSON decoder accepted invalid Name: {invalid}"
            );
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
    fn nfc_data_matches_consensus_profile() {
        assert_eq!(nfc_data_sha256(), EXPECTED_NFC_DATA_SHA256);
    }
    #[test]
    fn nfc_normalizer_rejects_unreviewed_data_profile() {
        let mut unreviewed_profile = EXPECTED_NFC_DATA_SHA256;
        unreviewed_profile[0] ^= 0x01;
        assert!(checked_nfc_normalizer(&unreviewed_profile).is_err());
    }
    #[test]
    fn nfc_normalization_matches_pinned_regression_corpus() {
        // This corpus exercises canonical decomposition, composition, combining
        // mark ordering, Hangul composition, and canonical singleton mappings.
        // Changes are protocol changes and must be reviewed with the exact ICU
        // algorithm pin and baked-data fingerprint.
        let cases = [
            ("e\u{0301}", "\u{00E9}"),
            ("\u{212B}", "\u{00C5}"),
            ("\u{2126}", "\u{03A9}"),
            ("\u{212A}", "K"),
            ("\u{1100}\u{1161}\u{11A8}", "\u{AC01}"),
            ("a\u{0315}\u{0300}", "\u{00E0}\u{0315}"),
            ("\u{1E0A}\u{0323}", "\u{1E0C}\u{0307}"),
        ];
        for (input, expected) in cases {
            let normalized = Name::from_str(input).expect("normalization corpus input is valid");
            let canonical = Name::from_str(expected).expect("normalization corpus output is valid");
            assert_eq!(normalized, canonical, "NFC mismatch for {input:?}");
            assert_eq!(normalized.as_ref(), expected, "NFC output for {input:?}");
        }
    }
}
