//! Canonical first-release privacy governance and proof-admission instructions.
//!
//! These instructions expose only typed V1 privacy records. They intentionally
//! have no string protocol selectors, compatibility aliases, or opaque proof
//! bodies.

use super::*;
use crate::privacy::{
    PrivacyConsensusLimitsV1, PrivacyPgcAccountBootstrapV1, PrivacyPgcBootstrapProofBytesV1,
    PrivacyProofEnvelopeV1, PrivacyProtocolActivationLimitsV1, PrivacyProtocolActivationRecordV1,
    PrivacyProtocolIdV1, PrivacyProtocolLifecycleV1, PrivacyRootPublicationV1,
    PrivacyZkAcePolicyRecordDigestV1, PrivacyZkAcePolicyRecordV1, PrivacyZkAmsRegistryBootstrapV1,
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
    /// Schedule a delayed component-wise tightening of the chain-wide privacy policy.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct SchedulePrivacyConsensusPolicyTighteningV1 {
        /// Exact incoming height at which the successor becomes effective.
        pub effective_at_height: u64,
        /// Complete component-wise-lower successor limits.
        pub next_limits: PrivacyConsensusLimitsV1,
    }
}

impl crate::seal::Instruction for SchedulePrivacyConsensusPolicyTighteningV1 {}

impl SchedulePrivacyConsensusPolicyTighteningV1 {
    /// Canonical first-release Norito instruction identifier.
    pub const WIRE_ID: &'static str = "iroha.privacy.schedule_consensus_policy_tightening.v1";

    /// Construct a chain-wide privacy-policy schedule.
    #[must_use]
    pub const fn new(effective_at_height: u64, next_limits: PrivacyConsensusLimitsV1) -> Self {
        Self {
            effective_at_height,
            next_limits,
        }
    }
}

isi! {
    /// Schedule a delayed component-wise tightening for one privacy protocol.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct SchedulePrivacyProtocolLimitsTighteningV1 {
        /// Exact registered protocol whose limits will be tightened.
        pub protocol_id: PrivacyProtocolIdV1,
        /// Exact incoming height at which the successor becomes effective.
        pub effective_at_height: u64,
        /// Complete protocol-tagged successor limits.
        pub next_limits: PrivacyProtocolActivationLimitsV1,
    }
}

impl crate::seal::Instruction for SchedulePrivacyProtocolLimitsTighteningV1 {}

impl SchedulePrivacyProtocolLimitsTighteningV1 {
    /// Canonical first-release Norito instruction identifier.
    pub const WIRE_ID: &'static str = "iroha.privacy.schedule_protocol_limits_tightening.v1";

    /// Construct a protocol-specific limit schedule.
    #[must_use]
    pub const fn new(
        protocol_id: PrivacyProtocolIdV1,
        effective_at_height: u64,
        next_limits: PrivacyProtocolActivationLimitsV1,
    ) -> Self {
        Self {
            protocol_id,
            effective_at_height,
            next_limits,
        }
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
        /// Exact canonical native proof of account well-formedness, range, and supply.
        pub proof: PrivacyPgcBootstrapProofBytesV1,
    }
}

impl crate::seal::Instruction for BootstrapPrivacyPgcAccountsV1 {}

impl BootstrapPrivacyPgcAccountsV1 {
    /// Canonical first-release Norito instruction identifier.
    pub const WIRE_ID: &'static str = "iroha.privacy.bootstrap_pgc_accounts.v1";

    /// Construct a governed Anonymous PGC account bootstrap.
    #[must_use]
    pub fn new(
        bootstrap: PrivacyPgcAccountBootstrapV1,
        proof: PrivacyPgcBootstrapProofBytesV1,
    ) -> Self {
        Self { bootstrap, proof }
    }
}

isi! {
    /// Atomically initialize one governed ZK-AMS issuer, policy, and admitted-identity registry.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct BootstrapPrivacyZkAmsRegistryV1 {
        /// Exact issuer key, policy digest, namespace, root, and origin epoch.
        pub bootstrap: PrivacyZkAmsRegistryBootstrapV1,
    }
}

impl crate::seal::Instruction for BootstrapPrivacyZkAmsRegistryV1 {}

impl BootstrapPrivacyZkAmsRegistryV1 {
    /// Canonical first-release Norito instruction identifier.
    pub const WIRE_ID: &'static str = "iroha.privacy.bootstrap_zk_ams_registry.v1";

    /// Construct a governed ZK-AMS registry bootstrap.
    #[must_use]
    pub const fn new(bootstrap: PrivacyZkAmsRegistryBootstrapV1) -> Self {
        Self { bootstrap }
    }
}

isi! {
    /// Register one canonical authoritative ZK-ACE policy lineage.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RegisterPrivacyZkAcePolicyV1 {
        /// Complete active origin record, including its canonical self-digest.
        pub policy: PrivacyZkAcePolicyRecordV1,
    }
}

impl crate::seal::Instruction for RegisterPrivacyZkAcePolicyV1 {}

impl RegisterPrivacyZkAcePolicyV1 {
    /// Canonical first-release Norito instruction identifier.
    pub const WIRE_ID: &'static str = "iroha.privacy.register_zk_ace_policy.v1";

    /// Construct an authoritative policy registration.
    #[must_use]
    pub fn new(policy: PrivacyZkAcePolicyRecordV1) -> Self {
        Self { policy }
    }
}

isi! {
    /// Rotate one active authoritative ZK-ACE policy by exactly one epoch.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RotatePrivacyZkAcePolicyV1 {
        /// Exact self-digest of the active record being replaced.
        pub expected_current_record_digest: PrivacyZkAcePolicyRecordDigestV1,
        /// Complete active successor record.
        pub successor: PrivacyZkAcePolicyRecordV1,
    }
}

impl crate::seal::Instruction for RotatePrivacyZkAcePolicyV1 {}

impl RotatePrivacyZkAcePolicyV1 {
    /// Canonical first-release Norito instruction identifier.
    pub const WIRE_ID: &'static str = "iroha.privacy.rotate_zk_ace_policy.v1";

    /// Construct an exact policy rotation.
    #[must_use]
    pub fn new(
        expected_current_record_digest: PrivacyZkAcePolicyRecordDigestV1,
        successor: PrivacyZkAcePolicyRecordV1,
    ) -> Self {
        Self {
            expected_current_record_digest,
            successor,
        }
    }
}

isi! {
    /// Irreversibly revoke one active authoritative ZK-ACE policy.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RevokePrivacyZkAcePolicyV1 {
        /// Exact self-digest of the active record being revoked.
        pub expected_current_record_digest: PrivacyZkAcePolicyRecordDigestV1,
        /// Complete revoked successor record at the next epoch.
        pub successor: PrivacyZkAcePolicyRecordV1,
    }
}

impl crate::seal::Instruction for RevokePrivacyZkAcePolicyV1 {}

impl RevokePrivacyZkAcePolicyV1 {
    /// Canonical first-release Norito instruction identifier.
    pub const WIRE_ID: &'static str = "iroha.privacy.revoke_zk_ace_policy.v1";

    /// Construct an exact irreversible policy revocation.
    #[must_use]
    pub fn new(
        expected_current_record_digest: PrivacyZkAcePolicyRecordDigestV1,
        successor: PrivacyZkAcePolicyRecordV1,
    ) -> Self {
        Self {
            expected_current_record_digest,
            successor,
        }
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
impl_privacy_decode_from_slice!(SchedulePrivacyConsensusPolicyTighteningV1 {
    effective_at_height: u64,
    next_limits: PrivacyConsensusLimitsV1,
});
impl_privacy_decode_from_slice!(SchedulePrivacyProtocolLimitsTighteningV1 {
    protocol_id: PrivacyProtocolIdV1,
    effective_at_height: u64,
    next_limits: PrivacyProtocolActivationLimitsV1,
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
    proof: PrivacyPgcBootstrapProofBytesV1,
});
impl_privacy_decode_from_slice!(BootstrapPrivacyZkAmsRegistryV1 {
    bootstrap: PrivacyZkAmsRegistryBootstrapV1,
});
impl_privacy_decode_from_slice!(RegisterPrivacyZkAcePolicyV1 {
    policy: PrivacyZkAcePolicyRecordV1,
});
impl_privacy_decode_from_slice!(RotatePrivacyZkAcePolicyV1 {
    expected_current_record_digest: PrivacyZkAcePolicyRecordDigestV1,
    successor: PrivacyZkAcePolicyRecordV1,
});
impl_privacy_decode_from_slice!(RevokePrivacyZkAcePolicyV1 {
    expected_current_record_digest: PrivacyZkAcePolicyRecordDigestV1,
    successor: PrivacyZkAcePolicyRecordV1,
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
            IROHA_JINDO_LATTICE_COMMITMENT_BYTES_V1, IrohaJindoPolynomialCommitmentStatementV1,
            JindoActivationLimitsV1, PrivacyActiveLifecycleV1, PrivacyAssuranceV1,
            PrivacyConsensusLimitsV1, PrivacyEngineManifestDigestV1, PrivacyJindoFieldElementV1,
            PrivacyJindoLatticeCommitmentV1, PrivacyNamespaceScopeV1, PrivacyNamespaceV1,
            PrivacyP256CiphertextV1, PrivacyP256PointV1, PrivacyParameterDigestV1,
            PrivacyParameterIdV1, PrivacyPgcAccountBootstrapV1, PrivacyPgcAccountV1,
            PrivacyPoolIdV1, PrivacyPoolNamespaceV1, PrivacyProofBytesV1, PrivacyProofV1,
            PrivacyProposedLifecycleV1, PrivacyProtocolActivationLimitsV1, PrivacyRootRoleV1,
            PrivacyRootV1, PrivacyStatementContextV1, PrivacyStatementSchemaDigestV1,
            PrivacyStatementV1, PrivacyTransactionIntentDigestV1, PrivacyVerifierDigestV1,
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
            protocol_limits: PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
                JindoActivationLimitsV1 {
                    max_polynomial_count: 4,
                },
            ),
            pending_protocol_limits_tightening: None,
            assurance: PrivacyAssuranceV1::Experimental,
        }
    }

    fn envelope() -> PrivacyProofEnvelopeV1 {
        let activation = activation();
        let context = PrivacyStatementContextV1 {
            chain_id: ChainId::from("privacy-isi-test"),
            action_index: 0,
            transaction_intent_digest: PrivacyTransactionIntentDigestV1::new(digest(6)),
            parameter_id: activation.parameter_id,
            parameter_digest: activation.parameter_digest,
            verifier_digest: activation.verifier_digest,
            statement_schema_digest: activation.statement_schema_digest,
            engine_manifest_digest: activation.engine_manifest_digest,
        };
        let mut commitment = vec![0; IROHA_JINDO_LATTICE_COMMITMENT_BYTES_V1];
        commitment[..4].copy_from_slice(&6_i32.to_le_bytes());
        let mut evaluation_point = [0; 32];
        evaluation_point[0] = 7;
        let mut claimed_evaluation = [0; 32];
        claimed_evaluation[0] = 8;
        let statement = PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(
            IrohaJindoPolynomialCommitmentStatementV1 {
                context,
                polynomial_commitments: vec![PrivacyJindoLatticeCommitmentV1::new(commitment)],
                evaluation_point: PrivacyJindoFieldElementV1::new(evaluation_point),
                claimed_evaluations: vec![PrivacyJindoFieldElementV1::new(claimed_evaluation)],
            },
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
            total_supply: 160,
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
        let mut next_consensus_limits = PrivacyConsensusLimitsV1::taira_default();
        next_consensus_limits.max_actions_per_block = 1;
        assert_slice_roundtrip(SchedulePrivacyConsensusPolicyTighteningV1::new(
            700,
            next_consensus_limits,
        ));
        let activation = activation();
        let mut next_protocol_limits = activation.protocol_limits;
        let PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(ref mut limits) =
            next_protocol_limits
        else {
            unreachable!("Jindo fixture")
        };
        limits.max_polynomial_count -= 1;
        assert_slice_roundtrip(SchedulePrivacyProtocolLimitsTighteningV1::new(
            activation.protocol_id,
            700,
            next_protocol_limits,
        ));
        assert_slice_roundtrip(TransitionPrivacyProtocolLifecycleV1::new(
            PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
            PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
                proposed_at_height: 100,
                activated_at_height: 400,
                state_since_height: 400,
            }),
        ));
        assert_slice_roundtrip(PublishPrivacyRootV1::new(publication()));
        assert_slice_roundtrip(BootstrapPrivacyPgcAccountsV1::new(
            pgc_bootstrap(),
            PrivacyPgcBootstrapProofBytesV1::new(vec![0xA5, 0x5A, 1]),
        ));
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

        let bootstrap = BootstrapPrivacyPgcAccountsV1::new(
            pgc_bootstrap(),
            PrivacyPgcBootstrapProofBytesV1::new(vec![0xA5, 0x5A, 1]),
        )
        .encode();
        for truncated_len in [0, 1, bootstrap.len() / 2, bootstrap.len() - 1] {
            assert!(
                BootstrapPrivacyPgcAccountsV1::decode_from_slice(&bootstrap[..truncated_len])
                    .is_err(),
                "bootstrap truncation at {truncated_len} bytes must fail closed"
            );
        }
        let mut trailing_bootstrap = bootstrap;
        trailing_bootstrap.push(0x5A);
        assert!(
            BootstrapPrivacyPgcAccountsV1::decode_from_slice(&trailing_bootstrap).is_err(),
            "bootstrap trailing bytes must fail closed"
        );
        assert!(
            BootstrapPrivacyPgcAccountsV1::decode_from_slice(&pgc_bootstrap().encode()).is_err(),
            "the unreleased proofless bootstrap layout has no legacy decoder"
        );
    }

    #[test]
    fn stable_wire_ids_have_no_retired_compatibility_names() {
        for wire_id in [
            RegisterPrivacyProtocolActivationV1::WIRE_ID,
            SchedulePrivacyConsensusPolicyTighteningV1::WIRE_ID,
            SchedulePrivacyProtocolLimitsTighteningV1::WIRE_ID,
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
