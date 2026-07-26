//! Canonical first-release privacy governance and proof-admission instructions.
//!
//! These instructions expose only typed V1 privacy records. They intentionally
//! have no string protocol selectors, compatibility aliases, or opaque proof
//! bodies.

use super::*;
use crate::privacy::{
    PrivacyPgcAccountBootstrapV1, PrivacyProofEnvelopeV1, PrivacyProtocolActivationRecordV1,
    PrivacyProtocolIdV1, PrivacyProtocolLifecycleV1, PrivacyRootPublicationV1,
};

isi! {
    /// Register one immutable, future privacy-protocol activation.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RegisterPrivacyProtocolActivationV1 {
        /// Exact protocol, artifacts, lifecycle, and admission limits to register.
        pub activation: PrivacyProtocolActivationRecordV1,
    }
}

impl crate::seal::Instruction for RegisterPrivacyProtocolActivationV1 {}

impl RegisterPrivacyProtocolActivationV1 {
    /// Canonical first-release Norito instruction identifier.
    pub const WIRE_ID: &'static str = "iroha.privacy.register_protocol_activation.v1";

    /// Construct an activation-registration instruction.
    #[must_use]
    pub fn new(activation: PrivacyProtocolActivationRecordV1) -> Self {
        Self { activation }
    }
}

isi! {
    /// Apply a forward-only lifecycle transition to a registered privacy protocol.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct TransitionPrivacyProtocolLifecycleV1 {
        /// Exact protocol whose lifecycle is changing.
        pub protocol_id: PrivacyProtocolIdV1,
        /// Complete next lifecycle state, including its effective height.
        pub next_lifecycle: PrivacyProtocolLifecycleV1,
    }
}

impl crate::seal::Instruction for TransitionPrivacyProtocolLifecycleV1 {}

impl TransitionPrivacyProtocolLifecycleV1 {
    /// Canonical first-release Norito instruction identifier.
    pub const WIRE_ID: &'static str = "iroha.privacy.transition_protocol_lifecycle.v1";

    /// Construct a lifecycle-transition instruction.
    #[must_use]
    pub fn new(
        protocol_id: PrivacyProtocolIdV1,
        next_lifecycle: PrivacyProtocolLifecycleV1,
    ) -> Self {
        Self {
            protocol_id,
            next_lifecycle,
        }
    }
}

isi! {
    /// Publish or initialize one governance-authorized canonical privacy root.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct PublishPrivacyRootV1 {
        /// Exact namespace, role, epoch, and root publication.
        pub publication: PrivacyRootPublicationV1,
    }
}

impl crate::seal::Instruction for PublishPrivacyRootV1 {}

impl PublishPrivacyRootV1 {
    /// Canonical first-release Norito instruction identifier.
    pub const WIRE_ID: &'static str = "iroha.privacy.publish_root.v1";

    /// Construct a governed root-publication instruction.
    #[must_use]
    pub fn new(publication: PrivacyRootPublicationV1) -> Self {
        Self { publication }
    }
}

isi! {
    /// Bootstrap one complete governed Anonymous PGC encrypted-account table.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct BootstrapPrivacyPgcAccountsV1 {
        /// Complete canonical pool namespace, root, epoch, and ordered accounts.
        pub bootstrap: PrivacyPgcAccountBootstrapV1,
    }
}

impl crate::seal::Instruction for BootstrapPrivacyPgcAccountsV1 {}

impl BootstrapPrivacyPgcAccountsV1 {
    /// Canonical first-release Norito instruction identifier.
    pub const WIRE_ID: &'static str = "iroha.privacy.bootstrap_pgc_accounts.v1";

    /// Construct a governed Anonymous PGC account bootstrap.
    #[must_use]
    pub fn new(bootstrap: PrivacyPgcAccountBootstrapV1) -> Self {
        Self { bootstrap }
    }
}

isi! {
    /// Verify and atomically apply one protocol-typed privacy proof action.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct SubmitPrivacyProofV1 {
        /// Complete governed-artifact-bound statement and native proof.
        pub envelope: PrivacyProofEnvelopeV1,
    }
}

impl crate::seal::Instruction for SubmitPrivacyProofV1 {}

impl SubmitPrivacyProofV1 {
    /// Canonical first-release Norito instruction identifier.
    pub const WIRE_ID: &'static str = "iroha.privacy.submit_proof.v1";

    /// Construct a privacy-proof submission instruction.
    #[must_use]
    pub fn new(envelope: PrivacyProofEnvelopeV1) -> Self {
        Self { envelope }
    }
}

fn privacy_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

macro_rules! impl_privacy_decode_from_slice {
    ($ty:ident { $($field:ident : $field_ty:ty),+ $(,)? }) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = privacy_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }

                let mut offset = 0usize;
                $(
                    let $field = super::decode_aos_canonical_field::<$field_ty>(
                        super::read_aos_field(bytes, &mut offset, flags)?,
                        flags,
                    )?;
                )+
                if offset != bytes.len() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                norito::core::note_payload_access(bytes, offset);
                Ok((Self { $($field),+ }, offset))
            }
        }
    };
}

impl_privacy_decode_from_slice!(RegisterPrivacyProtocolActivationV1 {
    activation: PrivacyProtocolActivationRecordV1,
});
impl_privacy_decode_from_slice!(TransitionPrivacyProtocolLifecycleV1 {
    protocol_id: PrivacyProtocolIdV1,
    next_lifecycle: PrivacyProtocolLifecycleV1,
});
impl_privacy_decode_from_slice!(PublishPrivacyRootV1 {
    publication: PrivacyRootPublicationV1,
});
impl_privacy_decode_from_slice!(BootstrapPrivacyPgcAccountsV1 {
    bootstrap: PrivacyPgcAccountBootstrapV1,
});
impl_privacy_decode_from_slice!(SubmitPrivacyProofV1 {
    envelope: PrivacyProofEnvelopeV1,
});

#[cfg(test)]
mod tests {
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::{
        ChainId,
        privacy::{
            IrohaJindoPolynomialCommitmentStatementV1, JindoActivationLimitsV1,
            PrivacyActiveLifecycleV1, PrivacyAssuranceV1, PrivacyCommitmentV1,
            PrivacyConsensusLimitsV1, PrivacyEngineManifestDigestV1, PrivacyJindoOpeningV1,
            PrivacyJindoScalarV1, PrivacyNamespaceScopeV1, PrivacyNamespaceV1,
            PrivacyP256CiphertextV1, PrivacyP256PointV1, PrivacyParameterDigestV1,
            PrivacyParameterIdV1, PrivacyPgcAccountBootstrapV1, PrivacyPgcAccountV1,
            PrivacyPoolIdV1, PrivacyPoolNamespaceV1, PrivacyProofBytesV1, PrivacyProofV1,
            PrivacyProposedLifecycleV1, PrivacyProtocolActivationLimitsV1, PrivacyRootRoleV1,
            PrivacyRootV1, PrivacyStatementContextV1, PrivacyStatementSchemaDigestV1,
            PrivacyStatementV1, PrivacyVerifierDigestV1,
        },
    };

    fn digest(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn activation() -> PrivacyProtocolActivationRecordV1 {
        let protocol_id = PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0;
        PrivacyProtocolActivationRecordV1 {
            protocol_id,
            proof_system_id: protocol_id.expected_proof_system(),
            engine_id: protocol_id.expected_engine(),
            parameter_id: PrivacyParameterIdV1::new(digest(1)),
            parameter_digest: PrivacyParameterDigestV1::new(digest(2)),
            verifier_digest: PrivacyVerifierDigestV1::new(digest(3)),
            statement_schema_digest: PrivacyStatementSchemaDigestV1::new(digest(4)),
            engine_manifest_digest: PrivacyEngineManifestDigestV1::new(digest(5)),
            lifecycle: PrivacyProtocolLifecycleV1::Proposed(PrivacyProposedLifecycleV1 {
                proposed_at_height: 100,
                activate_at_height: 400,
            }),
            limits: PrivacyConsensusLimitsV1::taira_default(),
            protocol_limits: PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
                JindoActivationLimitsV1 {
                    max_polynomial_count: 4,
                    max_evaluation_point_count: 8,
                },
            ),
            assurance: PrivacyAssuranceV1::Experimental,
        }
    }

    fn envelope() -> PrivacyProofEnvelopeV1 {
        let activation = activation();
        let context = PrivacyStatementContextV1 {
            chain_id: ChainId::from("privacy-isi-test"),
            action_index: 0,
            parameter_id: activation.parameter_id,
            parameter_digest: activation.parameter_digest,
            verifier_digest: activation.verifier_digest,
            statement_schema_digest: activation.statement_schema_digest,
            engine_manifest_digest: activation.engine_manifest_digest,
        };
        let statement = PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(
            IrohaJindoPolynomialCommitmentStatementV1::new(
                context,
                vec![PrivacyCommitmentV1::new(digest(6))],
                vec![PrivacyJindoOpeningV1 {
                    evaluation_point: PrivacyJindoScalarV1::new(digest(7)),
                    evaluations: vec![PrivacyJindoScalarV1::new(digest(8))],
                }],
            )
            .expect("fixture dimensions fit u32"),
        );
        let statement_digest = statement.digest().expect("fixture statement encodes");
        PrivacyProofEnvelopeV1 {
            protocol_id: activation.protocol_id,
            proof_system_id: activation.proof_system_id,
            engine_id: activation.engine_id,
            parameter_id: activation.parameter_id,
            parameter_digest: activation.parameter_digest,
            verifier_digest: activation.verifier_digest,
            statement_schema_digest: activation.statement_schema_digest,
            engine_manifest_digest: activation.engine_manifest_digest,
            statement_digest,
            statement,
            proof: PrivacyProofV1::IrohaJindoPolynomialCommitmentV0(PrivacyProofBytesV1::new(
                vec![9],
            )),
        }
    }

    fn publication() -> PrivacyRootPublicationV1 {
        PrivacyRootPublicationV1::new(
            PrivacyNamespaceV1::new(
                PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
                PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                    pool_id: PrivacyPoolIdV1::new(digest(20)),
                }),
            ),
            PrivacyRootRoleV1::PgcAccountState,
            1,
            PrivacyRootV1::new(digest(21)),
        )
        .expect("valid root publication")
    }

    fn p256_point(prefix: u8, final_byte: u8) -> PrivacyP256PointV1 {
        let mut bytes = [0; 33];
        bytes[0] = prefix;
        bytes[32] = final_byte;
        PrivacyP256PointV1::new(bytes)
    }

    fn pgc_bootstrap() -> PrivacyPgcAccountBootstrapV1 {
        PrivacyPgcAccountBootstrapV1 {
            namespace: publication().namespace,
            initial_root: PrivacyRootV1::new(digest(22)),
            initial_epoch: 1,
            accounts: (1..=16)
                .map(|index| PrivacyPgcAccountV1 {
                    public_key: p256_point(2, index),
                    encrypted_balance: PrivacyP256CiphertextV1 {
                        left: p256_point(2, index.wrapping_add(32)),
                        right: p256_point(3, index.wrapping_add(64)),
                    },
                })
                .collect(),
        }
    }

    fn assert_slice_roundtrip<T>(value: T)
    where
        T: Clone
            + core::fmt::Debug
            + PartialEq
            + norito::codec::Encode
            + for<'a> DecodeFromSlice<'a>,
    {
        let bytes = value.encode();
        let (decoded, used) = T::decode_from_slice(&bytes).expect("decode");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, value);
    }

    #[test]
    fn privacy_isis_roundtrip_through_direct_slice_decoders() {
        assert_slice_roundtrip(RegisterPrivacyProtocolActivationV1::new(activation()));
        assert_slice_roundtrip(TransitionPrivacyProtocolLifecycleV1::new(
            PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
            PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
                proposed_at_height: 100,
                activated_at_height: 400,
                state_since_height: 400,
            }),
        ));
        assert_slice_roundtrip(PublishPrivacyRootV1::new(publication()));
        assert_slice_roundtrip(BootstrapPrivacyPgcAccountsV1::new(pgc_bootstrap()));
        assert_slice_roundtrip(SubmitPrivacyProofV1::new(envelope()));
    }

    #[test]
    fn privacy_isi_decoders_reject_trailing_and_truncated_payloads() {
        let mut trailing = RegisterPrivacyProtocolActivationV1::new(activation()).encode();
        trailing.push(0xA5);
        assert!(matches!(
            RegisterPrivacyProtocolActivationV1::decode_from_slice(&trailing),
            Err(norito::core::Error::LengthMismatch)
        ));

        let submit = SubmitPrivacyProofV1::new(envelope()).encode();
        for truncated_len in [0, 1, submit.len() / 2, submit.len() - 1] {
            assert!(
                SubmitPrivacyProofV1::decode_from_slice(&submit[..truncated_len]).is_err(),
                "truncation at {truncated_len} bytes must fail closed"
            );
        }
    }

    #[test]
    fn stable_wire_ids_have_no_retired_compatibility_names() {
        for wire_id in [
            RegisterPrivacyProtocolActivationV1::WIRE_ID,
            TransitionPrivacyProtocolLifecycleV1::WIRE_ID,
            PublishPrivacyRootV1::WIRE_ID,
            BootstrapPrivacyPgcAccountsV1::WIRE_ID,
            SubmitPrivacyProofV1::WIRE_ID,
        ] {
            assert!(wire_id.starts_with("iroha.privacy."));
            assert!(!wire_id.contains("zkAt"));
            assert!(!wire_id.contains("silent"));
            assert!(!wire_id.contains("penumbra"));
            assert!(!wire_id.contains("aztec"));
        }
    }
}
