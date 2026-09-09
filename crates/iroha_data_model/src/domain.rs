//! This module contains [`Domain`](`crate::domain::Domain`) structure
//! and related implementations and trait implementations.
pub use self::model::*;

use crate::{
    DeriveFastJson as DeriveFast, DeriveJsonDeserialize as DeriveJsonDe,
    DeriveJsonSerialize as DeriveJsonSer,
};
use crate::{
    HasMetadata, Identifiable, Name, Registered, Registrable, error::ParseError,
    metadata::Metadata, name, prelude::*, sorafs_uri::SorafsUri,
};
use derive_more::Display;
use iroha_data_model_derive::{IdEqOrdHash, model};
use iroha_schema::IntoSchema;
use norito::{
    codec::{Decode, Encode},
    core as ncore,
};
use std::{format, str::FromStr, string::String, vec::Vec};
#[model]
mod model {
    use super::*;
    use getset::Getters;
    /// Identification of a [`Domain`].
    ///
    /// Components can only be constructed through the validating public API,
    /// including when the `transparent_api` feature is enabled.
    ///
    /// ```compile_fail,E0451
    /// use iroha_data_model::domain::DomainId;
    /// let id = DomainId {
    ///     name: "a.b".parse().unwrap(),
    ///     dataspace: "c".parse().unwrap(),
    /// };
    /// ```
    ///
    /// ```compile_fail,E0616
    /// use iroha_data_model::domain::DomainId;
    /// let mut id = DomainId::try_new("a", "b").unwrap();
    /// id.name = "a.c".parse().unwrap();
    /// ```
    ///
    /// ```compile_fail,E0616
    /// use iroha_data_model::domain::DomainId;
    /// let mut id = DomainId::try_new("a", "b").unwrap();
    /// id.dataspace = "b.c".parse().unwrap();
    /// ```
    #[derive(
        Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Getters, Encode, IntoSchema,
    )]
    #[display("{name}.{dataspace}")]
    #[getset(get = "pub")]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    #[derive(norito::NoritoSchema)]
    #[norito_schema(name = "iroha_data_model::domain::model::DomainId")]
    pub struct DomainId {
        /// Domain label unique only within its parent dataspace.
        pub(super) name: Name,
        /// Dataspace alias that owns the domain namespace.
        pub(super) dataspace: Name,
    }
    /// Named group of [`Account`] and [`Asset`](`crate::asset::value::Asset`) entities.
    #[derive(Debug, Display, Clone, IdEqOrdHash, Getters, Decode, Encode, IntoSchema)]
    #[allow(clippy::multiple_inherent_impl)]
    #[display("[{id}]")]
    #[derive(DeriveJsonSer, DeriveJsonDe, DeriveFast)]
    #[norito(no_fast_from_json)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    #[derive(norito::NoritoSchema)]
    #[norito_schema(name = "iroha_data_model::domain::model::Domain")]
    pub struct Domain {
        /// Identification of this [`Domain`].
        pub id: DomainId,
        /// `SoraFS` URI to the [`Domain`] logo.
        #[getset(get = "pub")]
        pub logo: Option<SorafsUri>,
        /// [`Metadata`] of this `Domain` as a key-value store.
        pub metadata: Metadata,
        /// The account that owns this domain. Usually the [`Account`] that registered it.
        #[getset(get = "pub")]
        pub owned_by: AccountId,
    }
    /// Builder which can be submitted in a transaction to create a new [`Domain`]
    #[derive(
        Debug,
        Display,
        Clone,
        IdEqOrdHash,
        Decode,
        Encode,
        IntoSchema,
        DeriveJsonSer,
        DeriveJsonDe,
        DeriveFast,
    )]
    #[norito(no_fast_from_json)]
    #[display("[{id}]")]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    #[derive(norito::NoritoSchema)]
    #[norito_schema(name = "iroha_data_model::domain::model::NewDomain")]
    pub struct NewDomain {
        /// The identification associated with the domain builder.
        pub id: DomainId,
        /// The (`SoraFS`) link to the logo of this domain.
        pub logo: Option<SorafsUri>,
        /// Metadata associated with the domain builder.
        pub metadata: Metadata,
    }
}
impl DomainId {
    fn parse_label(raw: &str) -> Result<Name, ParseError> {
        let canonical = name::canonicalize_domain_label(raw)?;
        if canonical.contains('.') {
            return Err(ParseError::new(
                "each domain id component must contain exactly one DNS label",
            ));
        }
        Name::from_str(&canonical)
    }
    fn from_canonical_parts(name: Name, dataspace: Name) -> Self {
        Self { name, dataspace }
    }
    /// Build a dataspace-qualified domain identifier from explicit parts.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] when either segment is invalid under the domain-label
    /// canonicalisation rules or contains more than one DNS label.
    pub fn try_new(name: impl AsRef<str>, dataspace: impl AsRef<str>) -> Result<Self, ParseError> {
        Ok(Self::from_canonical_parts(
            Self::parse_label(name.as_ref())?,
            Self::parse_label(dataspace.as_ref())?,
        ))
    }
    /// Parse a fully qualified `domain.dataspace` literal.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] when the literal is empty, contains surrounding whitespace,
    /// or does not use the exact `domain.dataspace` format.
    pub fn parse_fully_qualified(candidate: &str) -> Result<Self, ParseError> {
        if candidate.trim().is_empty() {
            return Err(ParseError::new("domain id must not be empty"));
        }
        if candidate.trim() != candidate {
            return Err(ParseError::new(
                "domain id must not contain leading or trailing whitespace",
            ));
        }
        let dot_count = candidate.bytes().filter(|byte| *byte == b'.').count();
        if dot_count != 1 {
            return Err(ParseError::new(
                "domain id must use `domain.dataspace` format",
            ));
        }
        let (name, dataspace) = candidate
            .split_once('.')
            .expect("validated domain literal must contain exactly one dot");
        if name.is_empty() || dataspace.is_empty() {
            return Err(ParseError::new("domain id segments must not be empty"));
        }
        Self::try_new(name, dataspace)
    }

    fn preflight_canonical_label(label: &str) -> Result<(), ncore::Error> {
        if label.is_empty()
            || label.len() > 63
            || !label.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
            })
        {
            return Err(ncore::Error::NonCanonicalEncoding);
        }
        Ok(())
    }

    fn a_label_scratch_bytes(label: &str) -> Result<usize, ncore::Error> {
        fn smallvec_growth_bytes(
            max_scalars: usize,
            inline_scalars: usize,
            first_heap_capacity: usize,
        ) -> Result<usize, ncore::Error> {
            if max_scalars <= inline_scalars {
                return Ok(0);
            }
            let max_capacity = max_scalars
                .checked_next_power_of_two()
                .ok_or(ncore::Error::LengthMismatch)?;
            max_capacity
                .checked_mul(2)
                .and_then(|value| value.checked_sub(first_heap_capacity))
                .and_then(|value| value.checked_mul(core::mem::size_of::<u32>()))
                .ok_or(ncore::Error::LengthMismatch)
        }

        let Some(punycode) = label.strip_prefix("xn--") else {
            return Ok(0);
        };
        // `idna` decodes at most one scalar per Punycode input byte into inline
        // 59-scalar buffers for a DNS-sized A-label. ICU's pinned UTS-46
        // normalizer can expand one scalar to at most 18 scalars. Charge the
        // cumulative SmallVec growth for both ICU's 17-scalar normalization
        // buffer and idna's 253-scalar output buffer before either runs.
        let max_normalized_scalars = punycode
            .len()
            .checked_mul(18)
            .ok_or(ncore::Error::LengthMismatch)?;
        let normalizer = smallvec_growth_bytes(max_normalized_scalars, 17, 32)?;
        let output = smallvec_growth_bytes(max_normalized_scalars, 253, 256)?;
        normalizer
            .checked_add(output)
            .ok_or(ncore::Error::LengthMismatch)
    }

    fn from_canonical_wire_labels(name: &str, dataspace: &str) -> Result<Self, ncore::Error> {
        Self::preflight_canonical_label(name)?;
        Self::preflight_canonical_label(dataspace)?;

        // Both binary fields and JSON keys are still borrowed here. Reserve the
        // two retained Names, the two owned UTS-46 results, and audited A-label
        // scratch before constructing either component. Non-ASCII wire aliases
        // never reach Name's NFC normalizer.
        let component_bytes = name
            .len()
            .checked_add(dataspace.len())
            .ok_or(ncore::Error::LengthMismatch)?;
        let a_label_scratch = Self::a_label_scratch_bytes(name)?
            .checked_add(Self::a_label_scratch_bytes(dataspace)?)
            .ok_or(ncore::Error::LengthMismatch)?;
        let requested_bytes = component_bytes
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(a_label_scratch))
            .ok_or(ncore::Error::LengthMismatch)?;
        ncore::reserve_decode_allocation(requested_bytes)?;
        for label in [name, dataspace] {
            let canonical = name::canonicalize_domain_label(label)
                .map_err(|_| ncore::Error::NonCanonicalEncoding)?;
            if canonical != label {
                return Err(ncore::Error::NonCanonicalEncoding);
            }
        }
        Ok(Self::from_canonical_parts(
            Name::from_str(name).map_err(|_| ncore::Error::NonCanonicalEncoding)?,
            Name::from_str(dataspace).map_err(|_| ncore::Error::NonCanonicalEncoding)?,
        ))
    }

    fn decode_wire_labels(bytes: &[u8]) -> Result<(&str, &str, usize), ncore::Error> {
        fn field<'a>(
            bytes: &'a [u8],
            offset: &mut usize,
            len: usize,
        ) -> Result<&'a str, ncore::Error> {
            let end = offset
                .checked_add(len)
                .ok_or(ncore::Error::LengthMismatch)?;
            let payload = bytes
                .get(*offset..end)
                .ok_or(ncore::Error::LengthMismatch)?;
            // Name encodes exactly one length-prefixed UTF-8 string. Borrow that
            // payload until both domain labels pass the shared canonical check.
            let (label_len, header_len) = ncore::inspect_len_from_slice(payload)?;
            if label_len > 63 {
                return Err(ncore::Error::NonCanonicalEncoding);
            }
            let used = header_len
                .checked_add(label_len)
                .ok_or(ncore::Error::LengthMismatch)?;
            if used != payload.len() {
                return Err(ncore::Error::LengthMismatch);
            }
            let label = core::str::from_utf8(
                payload
                    .get(header_len..used)
                    .ok_or(ncore::Error::LengthMismatch)?,
            )
            .map_err(|_| ncore::Error::InvalidUtf8)?;
            *offset = end;
            Ok(label)
        }
        fn field_length(bytes: &[u8], offset: &mut usize) -> Result<usize, ncore::Error> {
            let tail = bytes.get(*offset..).ok_or(ncore::Error::LengthMismatch)?;
            let (len, header_len) = ncore::inspect_len_from_slice(tail)?;
            *offset = offset
                .checked_add(header_len)
                .ok_or(ncore::Error::LengthMismatch)?;
            Ok(len)
        }
        fn framed_field<'a>(bytes: &'a [u8], offset: &mut usize) -> Result<&'a str, ncore::Error> {
            let len = field_length(bytes, offset)?;
            field(bytes, offset, len)
        }
        if !ncore::use_packed_struct() {
            let mut offset = 0;
            let name = framed_field(bytes, &mut offset)?;
            let dataspace = framed_field(bytes, &mut offset)?;
            return Ok((name, dataspace, offset));
        }
        if ncore::use_field_bitset() {
            // Both opaque Name fields have explicit sizes in the derived encoder.
            // This fixed schema needs no heap-backed size table.
            if bytes.first() != Some(&0b0000_0011) {
                return Err(ncore::Error::NonCanonicalEncoding);
            }
            let mut offset = 1;
            let name_len = field_length(bytes, &mut offset)?;
            let dataspace_len = field_length(bytes, &mut offset)?;
            let name = field(bytes, &mut offset, name_len)?;
            let dataspace = field(bytes, &mut offset, dataspace_len)?;
            return Ok((name, dataspace, offset));
        }
        let (offsets, header_len, data_len, tail_len) =
            ncore::decode_packed_offsets_slice(bytes, 2)?;
        let data_end = header_len
            .checked_add(data_len)
            .ok_or(ncore::Error::LengthMismatch)?;
        let data = bytes
            .get(header_len..data_end)
            .ok_or(ncore::Error::LengthMismatch)?;
        let [start, middle, end] = offsets.as_slice() else {
            return Err(ncore::Error::LengthMismatch);
        };
        let mut offset = 0;
        let name = field(data, &mut offset, middle - start)?;
        let dataspace = field(data, &mut offset, end - middle)?;
        let used = data_end
            .checked_add(tail_len)
            .ok_or(ncore::Error::LengthMismatch)?;
        Ok((name, dataspace, used))
    }

    /// Parse one canonical JSON object-key spelling with bounded decode accounting.

    pub(crate) fn parse_json_object_key(candidate: &str) -> Result<Self, norito::json::Error> {
        let (name, dataspace) = candidate.split_once('.').ok_or_else(|| {
            norito::json::Error::Message("domain key must use `domain.dataspace` format".to_owned())
        })?;
        Self::from_canonical_wire_labels(name, dataspace).map_err(|error| {
            if error.is_decode_resource_limit() {
                norito::json::Error::from_decode_resource(error)
            } else {
                norito::json::Error::Message(
                    "domain key must use two canonical DNS labels".to_owned(),
                )
            }
        })
    }
}

impl<'de> ncore::DeserializePayload<'de> for DomainId {
    fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("DomainId deserialization requires a valid canonical archive")
    }

    fn try_deserialize(archived: &'de ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        let bytes = ncore::payload_slice_from_ptr(ptr)?;
        let (name, dataspace, used) = Self::decode_wire_labels(bytes)?;
        ncore::finish_context_fields(ptr, used)?;
        Self::from_canonical_wire_labels(name, dataspace)
    }
}

impl norito::json::FastJsonWrite for DomainId {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.to_string(), out);
    }
    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::write_json_string_to(&self.to_string(), out)
    }
}

impl norito::json::JsonDeserialize for DomainId {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        Self::parse_fully_qualified(&value)
            .map_err(|err| norito::json::Error::Message(err.reason().into()))
    }
}
impl HasMetadata for NewDomain {
    #[inline]
    fn metadata(&self) -> &crate::metadata::Metadata {
        &self.metadata
    }
}
impl NewDomain {
    /// Create a [`NewDomain`], reserved for internal use.
    #[must_use]
    fn new(id: DomainId) -> Self {
        Self {
            id,
            logo: None,
            metadata: Metadata::default(),
        }
    }
    /// Add [`logo`](SorafsUri) to the domain replacing previously defined value.
    #[must_use]
    pub fn with_logo(mut self, logo: SorafsUri) -> Self {
        self.logo = Some(logo);
        self
    }
    /// Add [`Metadata`] to the domain replacing previously defined value
    #[must_use]
    pub fn with_metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = metadata;
        self
    }
}
impl HasMetadata for Domain {
    #[inline]
    fn metadata(&self) -> &crate::metadata::Metadata {
        &self.metadata
    }
}
impl Registered for Domain {
    type With = NewDomain;
}
impl Registrable for NewDomain {
    type Target = Domain;
    #[inline]
    fn build(self, authority: &AccountId) -> Self::Target {
        Self::Target {
            id: self.id,
            metadata: self.metadata,
            logo: self.logo,
            owned_by: authority.clone(),
        }
    }
}
impl Domain {
    /// Construct builder for [`Domain`] identifiable by [`DomainId`].
    #[inline]
    pub fn new(id: DomainId) -> <Self as Registered>::With {
        <Self as Registered>::With::new(id)
    }
    /// Mutable access to domain metadata for in-place updates.
    pub fn metadata_mut(&mut self) -> &mut Metadata {
        &mut self.metadata
    }
    /// Set the domain owner.
    pub fn set_owned_by(&mut self, owner: AccountId) {
        self.owned_by = owner;
    }
}
#[cfg(test)]
#[path = "domain/identity_tests.rs"]
mod identity_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::dsl::{HasProjection, PredicateMarker, SelectorMarker};
    fn assert_predicate<T: HasProjection<PredicateMarker>>() {}
    fn assert_selector<T: HasProjection<SelectorMarker>>() {}
    #[test]
    fn domain_has_projection_impls() {
        assert_predicate::<Domain>();
        assert_selector::<Domain>();
    }
    #[test]
    fn domain_id_try_new_canonicalizes_both_segments() {
        let domain_id = DomainId::try_new("Treasury", "CentralBank").expect("domain id");
        assert_eq!(domain_id.to_string(), "treasury.centralbank");
    }

    #[test]
    fn domain_id_json_key_constructor_rejects_ambiguous_component_boundaries() {
        // These component pairs would otherwise both display as `a.b.c`.
        for (name, dataspace) in [("a.b", "c"), ("a", "b.c"), ("a。b", "c"), ("a", "b．c")] {
            assert!(DomainId::try_new(name, dataspace).is_err());
        }
        let id = DomainId::try_new("例え", "テスト").expect("one label per component");
        assert_eq!(
            DomainId::parse_fully_qualified(&id.to_string()).expect("unique component boundary"),
            id
        );
    }
    #[test]
    fn domain_id_parse_fully_qualified_requires_both_segments() {
        let domain_id = DomainId::parse_fully_qualified("treasury.centralbank").expect("domain id");
        assert_eq!(domain_id.to_string(), "treasury.centralbank");
        assert!(DomainId::parse_fully_qualified("treasury").is_err());
    }

    #[test]
    fn domain_json_key_accounts_punycode_normalization_before_idna() {
        use norito::json::JsonObjectKeyOwned;

        let key = "xn--r8jz45g.centralbank";
        let component_bytes = key.len() - 1;
        // Seven Punycode bytes can normalize to at most 126 scalars. ICU's
        // 17-element inline buffer therefore grows through capacities 32, 64,
        // and 128: (32 + 64 + 128) * four bytes.
        let a_label_scratch = (32 + 64 + 128) * core::mem::size_of::<u32>();
        let exact = component_bytes * 2 + a_label_scratch;
        let limits = |bytes| {
            norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes, usize::MAX)
        };

        let (decoded, usage) = norito::core::with_decode_limits_measured(limits(exact), || {
            <DomainId as JsonObjectKeyOwned>::from_json_key_text(key)
        });
        assert_eq!(decoded.expect("canonical A-label key").to_string(), key);
        assert_eq!(usage.total_allocated_bytes(), exact);

        let (rejected, usage) =
            norito::core::with_decode_limits_measured(limits(exact - 1), || {
                <DomainId as JsonObjectKeyOwned>::from_json_key_text(key)
            });
        assert!(matches!(
            rejected,
            Err(norito::json::Error::DecodeResourceLimit)
        ));
        assert_eq!(usage.total_allocated_bytes(), 0);
    }
}
/// The prelude re-exports most commonly used traits, structs and macros from this crate.
pub mod prelude {
    pub use super::{Domain, DomainId};
}
