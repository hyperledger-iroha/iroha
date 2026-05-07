//! Hidden-program RAM-LFE program-policy instructions.

use super::*;
use crate::ram_lfe::{RamLfeProgramId, RamLfeProgramPolicy};

isi! {
    /// Register a new generic RAM-LFE program policy.
    pub struct RegisterRamLfeProgramPolicy {
        /// Program policy record to register.
        pub policy: RamLfeProgramPolicy,
    }
}

impl crate::seal::Instruction for RegisterRamLfeProgramPolicy {}

isi! {
    /// Activate an existing RAM-LFE program policy.
    pub struct ActivateRamLfeProgramPolicy {
        /// Program policy identifier to activate.
        pub program_id: RamLfeProgramId,
    }
}

impl crate::seal::Instruction for ActivateRamLfeProgramPolicy {}

isi! {
    /// Deactivate an existing RAM-LFE program policy.
    pub struct DeactivateRamLfeProgramPolicy {
        /// Program policy identifier to deactivate.
        pub program_id: RamLfeProgramId,
    }
}

impl crate::seal::Instruction for DeactivateRamLfeProgramPolicy {}

impl<'a> norito::core::DecodeFromSlice<'a> for RegisterRamLfeProgramPolicy {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let policy = super::decode_aos_canonical_field::<RamLfeProgramPolicy>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { policy }, offset))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for ActivateRamLfeProgramPolicy {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let program_id = super::decode_aos_canonical_field::<RamLfeProgramId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { program_id }, offset))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for DeactivateRamLfeProgramPolicy {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let program_id = super::decode_aos_canonical_field::<RamLfeProgramId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { program_id }, offset))
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{
        Algorithm, Hash, KeyPair, PolicyCommitment, PublicKey, RamLfeBackend,
        RamLfeVerificationMode,
    };
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::account::AccountId;

    fn public_key(seed: u8) -> PublicKey {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        key_pair.public_key().clone()
    }

    fn account(seed: u8) -> AccountId {
        AccountId::new(public_key(seed))
    }

    fn program_id() -> RamLfeProgramId {
        "email_retail".parse().expect("program id")
    }

    fn policy() -> RamLfeProgramPolicy {
        RamLfeProgramPolicy::new(
            program_id(),
            account(0xE1),
            RamLfeBackend::BfvProgrammedSha3_256V1,
            RamLfeVerificationMode::Signed,
            PolicyCommitment {
                backend: RamLfeBackend::BfvProgrammedSha3_256V1,
                policy_hash: Hash::new(b"policy"),
                public_parameters: vec![0x01, 0x02],
            },
            public_key(0xE2),
        )
        .with_note("identifier resolver")
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
    fn ram_lfe_policy_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(RegisterRamLfeProgramPolicy { policy: policy() });
        assert_slice_roundtrip(ActivateRamLfeProgramPolicy {
            program_id: program_id(),
        });
        assert_slice_roundtrip(DeactivateRamLfeProgramPolicy {
            program_id: program_id(),
        });
    }

    #[test]
    fn ram_lfe_policy_registry_decodes_stable_ids() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id_slice::<RegisterRamLfeProgramPolicy>(
                "identity::RegisterRamLfeProgramPolicy",
            )
            .register_with_id_slice::<ActivateRamLfeProgramPolicy>(
                "identity::ActivateRamLfeProgramPolicy",
            )
            .register_with_id_slice::<DeactivateRamLfeProgramPolicy>(
                "identity::DeactivateRamLfeProgramPolicy",
            );

        assert_registry_decodes(
            &registry,
            "identity::RegisterRamLfeProgramPolicy",
            RegisterRamLfeProgramPolicy { policy: policy() },
        );
        assert_registry_decodes(
            &registry,
            "identity::ActivateRamLfeProgramPolicy",
            ActivateRamLfeProgramPolicy {
                program_id: program_id(),
            },
        );
        assert_registry_decodes(
            &registry,
            "identity::DeactivateRamLfeProgramPolicy",
            DeactivateRamLfeProgramPolicy {
                program_id: program_id(),
            },
        );
    }
}
