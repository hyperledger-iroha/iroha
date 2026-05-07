//! Consensus key lifecycle instructions.

use super::*;

/// Register a consensus/committee key with lifecycle metadata.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, getset::Getters, Decode, Encode, IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[getset(get = "pub")]
pub struct RegisterConsensusKey {
    /// Identifier of the key being registered.
    pub id: crate::consensus::ConsensusKeyId,
    /// Key record to register.
    pub record: crate::consensus::ConsensusKeyRecord,
}

/// Rotate an existing consensus key by registering a successor and marking the old one retiring.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, getset::Getters, Decode, Encode, IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[getset(get = "pub")]
pub struct RotateConsensusKey {
    /// Identifier of the key being rotated out.
    pub id: crate::consensus::ConsensusKeyId,
    /// Replacement key record (must target the same role).
    pub record: crate::consensus::ConsensusKeyRecord,
}

/// Disable an existing consensus key.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, getset::Getters, Decode, Encode, IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[getset(get = "pub")]
pub struct DisableConsensusKey {
    /// Identifier of the key being disabled.
    pub id: crate::consensus::ConsensusKeyId,
}

impl crate::seal::Instruction for RegisterConsensusKey {}
impl crate::seal::Instruction for RotateConsensusKey {}
impl crate::seal::Instruction for DisableConsensusKey {}

fn consensus_key_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

macro_rules! impl_decode_key_record_instruction {
    ($ty:ident) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = consensus_key_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }

                let mut offset = 0usize;
                let id = super::decode_aos_canonical_field::<crate::consensus::ConsensusKeyId>(
                    super::read_aos_field(bytes, &mut offset, flags)?,
                    flags,
                )?;
                let record = super::decode_aos_canonical_field::<
                    crate::consensus::ConsensusKeyRecord,
                >(super::read_aos_field(bytes, &mut offset, flags)?, flags)?;
                if offset != bytes.len() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                norito::core::note_payload_access(bytes, offset);
                Ok((Self { id, record }, offset))
            }
        }
    };
}

impl_decode_key_record_instruction!(RegisterConsensusKey);
impl_decode_key_record_instruction!(RotateConsensusKey);

impl<'a> norito::core::DecodeFromSlice<'a> for DisableConsensusKey {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = consensus_key_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let id = super::decode_aos_canonical_field::<crate::consensus::ConsensusKeyId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { id }, offset))
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair, PublicKey};
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::consensus::{
        ConsensusKeyId, ConsensusKeyRecord, ConsensusKeyRole, ConsensusKeyStatus,
    };

    fn public_key(seed: u8) -> PublicKey {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        key_pair.public_key().clone()
    }

    fn key_id(name: &str) -> ConsensusKeyId {
        ConsensusKeyId::new(ConsensusKeyRole::Validator, name)
    }

    fn record(name: &str, seed: u8) -> ConsensusKeyRecord {
        ConsensusKeyRecord {
            id: key_id(name),
            public_key: public_key(seed),
            pop: Some(vec![0x01, 0x02]),
            activation_height: 10,
            expiry_height: Some(100),
            hsm: None,
            replaces: None,
            status: ConsensusKeyStatus::Pending,
        }
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

    fn assert_registry_decodes<T>(
        registry: &crate::isi::InstructionRegistry,
        wire_id: &'static str,
        value: T,
    ) where
        T: crate::isi::Instruction
            + norito::codec::Encode
            + 'static
            + norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed =
            norito::core::frame_bare_with_header_flags::<T>(&payload, flags).expect("frame");
        let decoded = crate::isi::InstructionRegistry::decode(registry, wire_id, &framed)
            .expect("registered")
            .expect("decode");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }

    #[test]
    fn consensus_key_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(RegisterConsensusKey {
            id: key_id("validator_a"),
            record: record("validator_a", 0x71),
        });
        assert_slice_roundtrip(RotateConsensusKey {
            id: key_id("validator_a"),
            record: ConsensusKeyRecord {
                replaces: Some(key_id("validator_a")),
                ..record("validator_b", 0x72)
            },
        });
        assert_slice_roundtrip(DisableConsensusKey {
            id: key_id("validator_a"),
        });
    }

    #[test]
    fn consensus_key_registry_decodes_stable_ids() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id_slice::<RegisterConsensusKey>("consensus::RegisterConsensusKey")
            .register_with_id_slice::<RotateConsensusKey>("consensus::RotateConsensusKey")
            .register_with_id_slice::<DisableConsensusKey>("consensus::DisableConsensusKey");

        assert_registry_decodes(
            &registry,
            "consensus::RegisterConsensusKey",
            RegisterConsensusKey {
                id: key_id("validator_a"),
                record: record("validator_a", 0x73),
            },
        );
        assert_registry_decodes(
            &registry,
            "consensus::RotateConsensusKey",
            RotateConsensusKey {
                id: key_id("validator_a"),
                record: ConsensusKeyRecord {
                    replaces: Some(key_id("validator_a")),
                    ..record("validator_c", 0x74)
                },
            },
        );
        assert_registry_decodes(
            &registry,
            "consensus::DisableConsensusKey",
            DisableConsensusKey {
                id: key_id("validator_a"),
            },
        );
    }
}
