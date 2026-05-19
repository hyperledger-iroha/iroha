//! Hidden-function-backed identifier policy instructions.

use super::*;
use crate::{
    account::AccountId,
    identifier::{IdentifierPolicy, IdentifierPolicyId, IdentifierResolutionReceipt},
};

isi! {
    /// Register a new identifier policy namespace in the world state.
    pub struct RegisterIdentifierPolicy {
        /// Identifier policy record to register.
        pub policy: IdentifierPolicy,
    }
}

impl crate::seal::Instruction for RegisterIdentifierPolicy {}

isi! {
    /// Activate an existing identifier policy namespace.
    pub struct ActivateIdentifierPolicy {
        /// Policy namespace to activate.
        pub policy_id: IdentifierPolicyId,
    }
}

impl crate::seal::Instruction for ActivateIdentifierPolicy {}

isi! {
    /// Bind an attested opaque identifier receipt to the UAID attached to an account.
    pub struct ClaimIdentifier {
        /// Account whose UAID should own the receipt-bound opaque identifier.
        pub account: AccountId,
        /// Receipt emitted by the configured identifier resolver.
        pub receipt: IdentifierResolutionReceipt,
    }
}

impl crate::seal::Instruction for ClaimIdentifier {}

isi! {
    /// Revoke a previously claimed opaque identifier.
    pub struct RevokeIdentifier {
        /// Policy namespace under which the opaque identifier was claimed.
        pub policy_id: IdentifierPolicyId,
        /// Opaque identifier to revoke.
        pub opaque_id: OpaqueAccountId,
    }
}

impl crate::seal::Instruction for RevokeIdentifier {}

fn identifier_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

macro_rules! impl_decode_one_field {
    ($ty:ident { $field:ident: $field_ty:ty }) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = identifier_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }

                let mut offset = 0usize;
                let $field = super::decode_aos_canonical_field::<$field_ty>(
                    super::read_aos_field(bytes, &mut offset, flags)?,
                    flags,
                )?;
                if offset != bytes.len() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                norito::core::note_payload_access(bytes, offset);
                Ok((Self { $field }, offset))
            }
        }
    };
}

macro_rules! impl_decode_two_fields {
    ($ty:ident { $first:ident: $first_ty:ty, $second:ident: $second_ty:ty }) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = identifier_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }

                let mut offset = 0usize;
                let $first = super::decode_aos_canonical_field::<$first_ty>(
                    super::read_aos_field(bytes, &mut offset, flags)?,
                    flags,
                )?;
                let $second = super::decode_aos_canonical_field::<$second_ty>(
                    super::read_aos_field(bytes, &mut offset, flags)?,
                    flags,
                )?;
                if offset != bytes.len() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                norito::core::note_payload_access(bytes, offset);
                Ok((Self { $first, $second }, offset))
            }
        }
    };
}

impl_decode_one_field!(RegisterIdentifierPolicy {
    policy: IdentifierPolicy
});
impl_decode_one_field!(ActivateIdentifierPolicy {
    policy_id: IdentifierPolicyId
});
impl_decode_two_fields!(ClaimIdentifier {
    account: AccountId,
    receipt: IdentifierResolutionReceipt
});
impl_decode_two_fields!(RevokeIdentifier {
    policy_id: IdentifierPolicyId,
    opaque_id: OpaqueAccountId
});

#[cfg(test)]
mod tests {
    use iroha_crypto::{
        Algorithm, Hash, KeyPair, PublicKey, RamLfeBackend, RamLfeVerificationMode, Signature,
        SignatureOf,
    };
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::{
        nexus::UniversalAccountId,
        ram_lfe::{
            RamLfeExecutionReceiptPayload, RamLfeOutputOpening, RamLfeOutputOpeningPayload,
            RamLfeProgramId, RamLfeReceiptAttestation,
        },
    };

    fn public_key(seed: u8) -> PublicKey {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        key_pair.public_key().clone()
    }

    fn account(seed: u8) -> AccountId {
        AccountId::new(public_key(seed))
    }

    fn policy_id() -> IdentifierPolicyId {
        "email#retail".parse().expect("policy id")
    }

    fn program_id() -> RamLfeProgramId {
        "email_retail".parse().expect("program id")
    }

    fn policy() -> IdentifierPolicy {
        IdentifierPolicy::new(
            policy_id(),
            account(0xF1),
            crate::identifier::IdentifierNormalization::EmailAddress,
            program_id(),
        )
        .with_note("retail email")
    }

    fn receipt() -> IdentifierResolutionReceipt {
        let account_id = account(0xF2);
        let opening_payload = RamLfeOutputOpeningPayload {
            program_id: program_id(),
            input_ciphertext_hash: Hash::new(b"input-ciphertext"),
            output_ciphertext_hash: Hash::new(b"output-ciphertext"),
            parameter_digest: Hash::new(b"parameters"),
            evaluation_key_digest: Hash::new(b"evaluation-keys"),
            opened_output_hash: Hash::new(b"opened-output"),
            opened_at_ms: 1_777_777_777_001,
            expires_at_ms: Some(1_777_777_877_000),
        };
        let opening_signer = KeyPair::from_seed(vec![0xF4; 32], Algorithm::Ed25519);
        let opening = RamLfeOutputOpening {
            signature: Signature::from_bytes(
                SignatureOf::new(opening_signer.private_key(), &opening_payload).payload(),
            ),
            payload: opening_payload,
        };
        let payload = crate::identifier::IdentifierResolutionReceiptPayload {
            policy_id: policy_id(),
            execution: RamLfeExecutionReceiptPayload {
                program_id: program_id(),
                program_digest: Hash::new(b"program"),
                backend: RamLfeBackend::BfvProgrammedSha3_256V1,
                verification_mode: RamLfeVerificationMode::Signed,
                input_ciphertext_hash: Hash::new(b"input-ciphertext"),
                output_ciphertext_hash: Hash::new(b"output-ciphertext"),
                parameter_digest: Hash::new(b"parameters"),
                evaluation_key_digest: Hash::new(b"evaluation-keys"),
                output_hash: Hash::new(b"output"),
                associated_data_hash: Hash::new(b"associated-data"),
                executed_at_ms: 1_777_777_777_000,
                expires_at_ms: Some(1_777_777_877_000),
            },
            opening,
            opaque_id: OpaqueAccountId::from_hash(Hash::new(b"opaque")),
            receipt_hash: Hash::new(b"receipt"),
            uaid: UniversalAccountId::from_hash(Hash::new(b"uaid")),
            account_id,
        };
        let signer = KeyPair::from_seed(vec![0xF3; 32], Algorithm::Ed25519);
        let signature = SignatureOf::new(signer.private_key(), &payload);
        IdentifierResolutionReceipt {
            payload,
            attestation: RamLfeReceiptAttestation::Signed(Signature::from_bytes(
                signature.payload(),
            )),
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
    fn identifier_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(RegisterIdentifierPolicy { policy: policy() });
        assert_slice_roundtrip(ActivateIdentifierPolicy {
            policy_id: policy_id(),
        });
        assert_slice_roundtrip(ClaimIdentifier {
            account: account(0xF4),
            receipt: receipt(),
        });
        assert_slice_roundtrip(RevokeIdentifier {
            policy_id: policy_id(),
            opaque_id: OpaqueAccountId::from_hash(Hash::new(b"opaque")),
        });
    }

    #[test]
    fn identifier_registry_decodes_stable_ids() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id_slice::<RegisterIdentifierPolicy>(
                "identity::RegisterIdentifierPolicy",
            )
            .register_with_id_slice::<ActivateIdentifierPolicy>(
                "identity::ActivateIdentifierPolicy",
            )
            .register_with_id_slice::<ClaimIdentifier>("identity::ClaimIdentifier")
            .register_with_id_slice::<RevokeIdentifier>("identity::RevokeIdentifier");

        assert_registry_decodes(
            &registry,
            "identity::RegisterIdentifierPolicy",
            RegisterIdentifierPolicy { policy: policy() },
        );
        assert_registry_decodes(
            &registry,
            "identity::ActivateIdentifierPolicy",
            ActivateIdentifierPolicy {
                policy_id: policy_id(),
            },
        );
        assert_registry_decodes(
            &registry,
            "identity::ClaimIdentifier",
            ClaimIdentifier {
                account: account(0xF5),
                receipt: receipt(),
            },
        );
        assert_registry_decodes(
            &registry,
            "identity::RevokeIdentifier",
            RevokeIdentifier {
                policy_id: policy_id(),
                opaque_id: OpaqueAccountId::from_hash(Hash::new(b"opaque")),
            },
        );
    }
}
