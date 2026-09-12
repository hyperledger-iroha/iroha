//! Canonical domain identities, normalization, bounded codecs and storage keys.

use derive_more::Display;
use iroha_data_model_derive::model;
use iroha_schema::IntoSchema;
use mv::json::JsonKeyCodec;
#[cfg(test)]
use norito::codec::Decode;
use norito::{
    codec::Encode,
    core as ncore,
    json::{self, JsonObjectKey, JsonObjectKeyOwned},
};
use std::str::FromStr;

pub use self::model::DomainId;
use crate::{error::ParseError, name, name::Name};

#[model]
mod model {
    use super::*;
    use getset::Getters;
    /// Canonical dataspace-qualified domain identity.
    ///
    /// Components can only be constructed through the validating public API,
    /// including when the `transparent_api` feature is enabled.
    ///
    /// ```compile_fail,E0451
    /// use iroha_model_base::domain::DomainId;
    /// let id = DomainId {
    ///     name: "a.b".parse().unwrap(),
    ///     dataspace: "c".parse().unwrap(),
    /// };
    /// ```
    ///
    /// ```compile_fail,E0616
    /// use iroha_model_base::domain::DomainId;
    /// let mut id = DomainId::try_new("a", "b").unwrap();
    /// id.name = "a.c".parse().unwrap();
    /// ```
    ///
    /// ```compile_fail,E0616
    /// use iroha_model_base::domain::DomainId;
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
    fn parse_json_object_key(candidate: &str) -> Result<Self, norito::json::Error> {
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

impl<'a> ncore::DecodeFromSlice<'a> for DomainId {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        ncore::decode_field_canonical(bytes)
    }
}

impl JsonKeyCodec for crate::domain::DomainId {
    fn encode_json_key(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        crate::domain::DomainId::parse_fully_qualified(encoded)
            .map_err(|err| json::Error::Message(err.to_string()))
    }
}

impl JsonObjectKey for DomainId {
    fn visit_json_key_text<E>(
        &self,
        mut visitor: impl FnMut(&str) -> Result<(), E>,
    ) -> Result<(), E> {
        let canonical = self.to_string();
        visitor(&canonical)
    }

    fn visit_json_key_text_checked(
        &self,
        visitor: impl FnMut(&str) -> Result<(), json::BoundedJsonError>,
    ) -> Result<(), json::BoundedJsonError> {
        json::visit_json_display_text(self, visitor)
    }
}

impl JsonObjectKeyOwned for crate::domain::DomainId {
    fn from_json_key_text(key: &str) -> Result<Self, json::Error> {
        crate::domain::DomainId::parse_json_object_key(key)
    }
}

#[cfg(test)]
mod identity_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod wire_identity_tests;
