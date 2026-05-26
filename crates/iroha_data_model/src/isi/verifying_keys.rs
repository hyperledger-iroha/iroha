use super::*;
use crate::proof::{VerifyingKeyId, VerifyingKeyRecord};

isi! {
    /// Register a new verifying key record into the WSV.
    pub struct RegisterVerifyingKey {
        /// Identifier of the verifying key (backend + name).
        pub id: VerifyingKeyId,
        /// Verifying key record with version, commitment, and optional stored key bytes.
        pub record: VerifyingKeyRecord,
    }
}

isi! {
    /// Update an existing verifying key record (monotonic version).
    pub struct UpdateVerifyingKey {
        /// Identifier of the verifying key to update.
        pub id: VerifyingKeyId,
        /// New record with strictly greater version.
        pub record: VerifyingKeyRecord,
    }
}

// Seal implementations so these types participate as instructions
impl crate::seal::Instruction for RegisterVerifyingKey {}
impl crate::seal::Instruction for UpdateVerifyingKey {}

fn verifying_key_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

macro_rules! impl_decode_verifying_key_instruction {
    ($ty:ident) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = verifying_key_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }

                let mut offset = 0usize;
                let id = super::decode_aos_canonical_field::<VerifyingKeyId>(
                    super::read_aos_field(bytes, &mut offset, flags)?,
                    flags,
                )?;
                let record = super::decode_aos_canonical_field::<VerifyingKeyRecord>(
                    super::read_aos_field(bytes, &mut offset, flags)?,
                    flags,
                )?;
                if offset != bytes.len() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                norito::core::note_payload_access(bytes, offset);
                Ok((Self { id, record }, offset))
            }
        }
    };
}

impl_decode_verifying_key_instruction!(RegisterVerifyingKey);
impl_decode_verifying_key_instruction!(UpdateVerifyingKey);

#[cfg(test)]
mod tests {
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::{proof::VerifyingKeyRecord, zk::BackendTag};

    fn key_id(name: &str) -> VerifyingKeyId {
        VerifyingKeyId::new("halo2/ipa", name)
    }

    fn record(version: u32) -> VerifyingKeyRecord {
        VerifyingKeyRecord::new(
            version,
            "offline-note",
            BackendTag::Halo2IpaPasta,
            "pasta",
            [0x11; 32],
            [0x22; 32],
        )
    }

    fn assert_slice_roundtrip<T>(value: T)
    where
        T: Clone + PartialEq + core::fmt::Debug + norito::codec::Encode,
        for<'a> T: DecodeFromSlice<'a>,
    {
        let bytes = value.encode();
        let (decoded, used) = T::decode_from_slice(&bytes).expect("decode from slice");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, value);
    }

    fn assert_registry_decodes<T>(registry: &crate::isi::InstructionRegistry, value: T)
    where
        T: crate::isi::Instruction
            + norito::codec::Encode
            + 'static
            + norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let wire_id = std::any::type_name::<T>();
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed =
            norito::core::frame_bare_with_header_flags::<T>(&payload, flags).expect("frame");
        let decoded = crate::isi::InstructionRegistry::decode(registry, wire_id, &framed)
            .expect("registered")
            .expect("decode");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }

    #[test]
    fn verifying_key_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(RegisterVerifyingKey {
            id: key_id("vk_a"),
            record: record(1),
        });
        assert_slice_roundtrip(UpdateVerifyingKey {
            id: key_id("vk_a"),
            record: record(2),
        });
    }

    #[test]
    fn verifying_key_registry_decodes_type_names() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_slice::<RegisterVerifyingKey>()
            .register_slice::<UpdateVerifyingKey>();

        assert_registry_decodes(
            &registry,
            RegisterVerifyingKey {
                id: key_id("vk_b"),
                record: record(1),
            },
        );
        assert_registry_decodes(
            &registry,
            UpdateVerifyingKey {
                id: key_id("vk_b"),
                record: record(2),
            },
        );
    }
}
