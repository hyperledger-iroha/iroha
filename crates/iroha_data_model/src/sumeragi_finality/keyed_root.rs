//! Opaque 32-byte complete-State roots. The governed commitment profile determines
//! their construction. No digest bits are changed on the wire.

use iroha_crypto::Hash;

/// The raw bytes certified in the parent and post-execution complete-State slots.
/// An LtHash profile supplies its original marked digest. A keyed profile supplies
/// all 256 bits of its schema-bound digest.
#[repr(transparent)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::KeyedStateRoot")]
pub struct KeyedStateRoot(pub [u8; 32]);

impl KeyedStateRoot {
    /// Exact wire payload length.
    pub const BYTES: usize = 32;
    /// All certified digest bits, without any marker or field reduction.
    pub const fn payload(&self) -> &[u8; 32] {
        &self.0
    }
    /// Only a complete raw 32-byte payload is a root.
    pub fn from_payload(payload: &[u8]) -> Option<Self> {
        payload.try_into().ok().map(Self)
    }
    /// Extract the exact marked representation required by an LtHash consumer.
    /// A marked digest alone does not establish which profile certified it.
    pub fn as_legacy_hash(&self) -> Option<Hash> {
        Hash::from_marked_bytes(self.0)
    }
}
impl AsRef<[u8; 32]> for KeyedStateRoot {
    fn as_ref(&self) -> &[u8; 32] {
        self.payload()
    }
}
impl From<Hash> for KeyedStateRoot {
    fn from(hash: Hash) -> Self {
        Self(hash.into())
    }
}
impl From<[u8; 32]> for KeyedStateRoot {
    fn from(raw: [u8; 32]) -> Self {
        Self(raw)
    }
}
impl PartialEq<Hash> for KeyedStateRoot {
    fn eq(&self, hash: &Hash) -> bool {
        self.payload() == hash.as_ref()
    }
}
impl std::fmt::Display for KeyedStateRoot {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut bytes = [0; 64];
        hex::encode_to_slice(self.payload(), &mut bytes).expect("exact hex length");
        out.pad(std::str::from_utf8(&bytes).expect("hex is ASCII"))
    }
}
impl norito::core::SerializePayload for KeyedStateRoot {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        writer.write_all(&self.0)?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(Self::BYTES)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(Self::BYTES)
    }
}
impl<'de> norito::core::DeserializePayload<'de> for KeyedStateRoot {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("canonical State commitment decode")
    }
    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        let mut offset = 0;
        let bytes = norito::core::decode_context_byte_array::<{ Self::BYTES }>(ptr, &mut offset)?;
        norito::core::finish_context_fields(ptr, offset)?;
        Ok(Self(bytes))
    }
}
impl<'de> norito::core::DecodeFromSlice<'de> for KeyedStateRoot {
    fn decode_from_slice(bytes: &'de [u8]) -> Result<(Self, usize), norito::core::Error> {
        let raw = bytes
            .get(..Self::BYTES)
            .ok_or(norito::core::Error::LengthMismatch)?;
        let mut digest = [0; Self::BYTES];
        digest.copy_from_slice(raw);
        norito::core::note_payload_access(bytes, Self::BYTES);
        Ok((Self(digest), Self::BYTES))
    }
}
impl norito::json::FastJsonWrite for KeyedStateRoot {
    fn write_json(&self, out: &mut String) {
        crate::json_helpers::fixed_bytes::serialize(&self.0, out);
    }
    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        crate::json_helpers::fixed_bytes::serialize_bounded(&self.0, out)
    }
}
impl norito::json::JsonDeserialize for KeyedStateRoot {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        crate::json_helpers::fixed_bytes::deserialize(parser).map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_field_roundtrips_every_bit_and_is_wire_identical_to_marked_lthash() {
        for &tail in &[0, 1, 0xfe, 0xff] {
            let mut bytes = [0x38; 32];
            bytes[31] = tail;
            let root = KeyedStateRoot(bytes);
            for flags in
                (u8::MIN..=u8::MAX).filter(|f| norito::core::validate_header_flags(*f).is_ok())
            {
                let _flags = norito::core::DecodeFlagsGuard::enter(flags);
                let raw = norito::codec::encode_with_header_flags(&root).0;
                assert_eq!(raw, bytes);
                let (decoded, n) =
                    norito::core::decode_field_canonical::<KeyedStateRoot>(&raw).unwrap();
                assert_eq!(n, 32);
                assert_eq!(decoded, root);
                assert_eq!(KeyedStateRoot::from_payload(&raw), Some(root));
                assert_eq!(root.as_legacy_hash().is_some(), tail & 1 != 0);
                if let Some(legacy) = root.as_legacy_hash() {
                    assert_eq!(norito::codec::encode_with_header_flags(&legacy).0, raw);
                    assert_eq!(KeyedStateRoot::from(legacy), root);
                }
            }
            assert_eq!(
                norito::json::from_str::<KeyedStateRoot>(&norito::json::to_json(&root).unwrap())
                    .unwrap(),
                root
            );
        }
        for len in [0, 31, 33, 48] {
            assert!(norito::core::decode_field_canonical::<KeyedStateRoot>(&vec![0; len]).is_err());
        }
    }
}
