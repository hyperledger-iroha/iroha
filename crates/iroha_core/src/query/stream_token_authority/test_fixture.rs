//! Signed native role-11 execution with retained four-validator test finality.
//!
//! Account/permission setup is synthetic. Custody configuration, enrollment and every later
//! operation execute their original signed instructions through the initial executor. Real
//! three-of-four BLS votes and RS16 layout authenticate this fixture's blocks; this does not
//! qualify ordinary admission, fee settlement, production custody or a deployed State root.

use crate::{
    kura::Kura,
    query::{
        signer_check::fixture as execution,
        store::LiveQueryStore,
        stream_token_custody::{encode, read_active, read_stream_token_custody_control_at_v1},
    },
    state::{State, World},
};
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::{
    IntoKeyValue, Registrable,
    account::{Account, AccountId},
    isi::{InstructionBox, Revoke, sorafs::MutateSorafsStreamTokenCustody},
    permission::{Permission, Permissions},
    sorafs::{capacity::ProviderId, stream_token_custody::SorafsStreamTokenCustodyActionV1},
    transaction::{FeePaymentIntent, SignedTransaction, TransactionBuilder},
};
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsStreamToken, CanManageSorafsStreamTokenCustody, CanOperateSorafsStreamToken,
};
use iroha_sccp::{
    SCCP_TAIRA_CHAIN_ID_V1, SccpFinalizedBlockTestFixtureV1, sccp_taira_finality_network_id_v1,
};
use sorafs_manifest::signer::{
    custody::{
        SIGNER_CUSTODY_MAGIC_V1, SIGNER_CUSTODY_VERSION_V1, SignerCustodyAuthorityV1,
        SignerCustodyBindingV1, SignerCustodyRecordV1, SignerCustodyStatementV1,
    },
    custody_control::SignerCustodyPolicyV1,
    protocol::{SignerKeyAlgorithmV1, SignerPurposeBindingV1, SignerRoleV1},
};
use std::{sync::Arc, time::Duration};

/// Owns exact signed native custody history and its real fixed-roster test proof chain.
pub struct StreamTokenRuntimeTestFixtureV1 {
    /// State shared by the runtime under test and native executor.
    pub state: Arc<State>,
    /// Policy actually installed by the signed Configure instruction.
    pub policy: SignerCustodyPolicyV1,
    /// Actual attestation bytes admitted by the signed Enroll instruction.
    pub record: Vec<u8>,
    /// Exact provider scope of the installed custody and runtime permissions.
    pub provider: ProviderId,
    finalized: Vec<SccpFinalizedBlockTestFixtureV1>,
}

impl StreamTokenRuntimeTestFixtureV1 {
    /// Derive a reproducible test-only Ed25519 key.
    #[must_use]
    pub fn key(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("test key")
    }

    /// Execute configuration and anchored enrollment immediately before `now` milliseconds.
    ///
    /// # Panics
    /// Panics on invalid time, fixture storage failure or rejected actual native execution.
    #[must_use]
    pub fn new_at(now: u64) -> Self {
        assert!(now > 2 && now < u64::MAX - 3_600_000);
        let manager = AccountId::new(Self::key(1).public_key().clone());
        let operator = AccountId::new(Self::key(2).public_key().clone());
        let observer = AccountId::new(Self::key(3).public_key().clone());
        let provider = ProviderId::new([3; 32]);
        let mut world = World::new();
        for account in [&manager, &operator, &observer] {
            let (id, value) = Account::new(account.clone())
                .build(&manager)
                .into_key_value();
            world.accounts.insert(id, value);
        }
        for (account, permission) in [
            (
                &manager,
                Permission::from(CanManageSorafsStreamTokenCustody {
                    provider_id: provider,
                }),
            ),
            (
                &operator,
                Permission::from(CanOperateSorafsStreamToken {
                    provider_id: provider,
                }),
            ),
            (
                &observer,
                Permission::from(CanCheckSorafsStreamToken {
                    provider_id: provider,
                }),
            ),
        ] {
            let mut permissions = Permissions::new();
            permissions.insert(permission);
            world
                .account_permissions
                .insert(account.clone(), permissions);
        }
        world.provider_owners.insert(provider, operator);
        let state = Arc::new(State::new_with_chain_and_network_id_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            SCCP_TAIRA_CHAIN_ID_V1.parse().expect("fixed test chain"),
            sccp_taira_finality_network_id_v1(),
        ));
        let policy = SignerCustodyPolicyV1 {
            binding: SignerCustodyBindingV1 {
                chain_id: SCCP_TAIRA_CHAIN_ID_V1.into(),
                network_id: *state.network_id_ref().as_bytes(),
                runtime_handle: "software://sorafs/stream-token/primary".into(),
                key_handle: "software://sorafs/stream-token/key-1".into(),
                service_id: "stream-service".into(),
                administrator_id: "stream-admin".into(),
                role: SignerRoleV1::StreamToken,
                purpose: SignerPurposeBindingV1::StreamToken {
                    provider_id: *provider.as_bytes(),
                },
                algorithm: SignerKeyAlgorithmV1::Ed25519,
                public_key: Self::key(4).public_key().clone(),
                key_revision: 1,
                policy_revision: 1,
                policy_digest: [5; 32],
            },
            attester_authority: SignerCustodyAuthorityV1 {
                service_id: "custody-service".into(),
                administrator_id: "custody-admin".into(),
                key_revision: 1,
                policy_revision: 1,
                policy_digest: [6; 32],
            },
            attester_public_key: Self::key(7).public_key().clone(),
            active_from_unix_ms: now - 2,
            active_until_unix_ms: now + 3_600_000,
            max_validity_ms: 3_600_000,
            max_anchor_age_ms: 120_000,
        };
        let mut fixture = Self {
            state,
            policy,
            record: Vec::new(),
            provider,
            finalized: Vec::new(),
        };
        assert!(
            fixture.commit_instruction(
                MutateSorafsStreamTokenCustody {
                    provider_id: provider,
                    expected_revision: 0,
                    expected_digest: [0; 32],
                    action: SorafsStreamTokenCustodyActionV1::Configure(
                        encode(&fixture.policy).unwrap()
                    ),
                }
                .into(),
                1,
                now - 2
            )
        );
        let snapshot = read_stream_token_custody_control_at_v1(
            &fixture.state.view(),
            &fixture.policy.binding,
            1,
        )
        .unwrap()
        .unwrap();
        let statement = SignerCustodyStatementV1 {
            magic: SIGNER_CUSTODY_MAGIC_V1,
            version: SIGNER_CUSTODY_VERSION_V1,
            binding: fixture.policy.binding.clone(),
            authority: fixture.policy.attester_authority.clone(),
            anchor: snapshot.anchor,
            sequence: snapshot.state.next_sequence,
            predecessor_digest: snapshot.state.predecessor_digest,
            issued_at_unix_ms: now - 1,
            expires_at_unix_ms: now + 3_599_999,
            evidence_digest: [9; 32],
            revoked: false,
        };
        let attestation = Signature::try_new(
            Self::key(7).private_key(),
            &statement.signing_payload().unwrap(),
        )
        .unwrap();
        fixture.record = encode(&SignerCustodyRecordV1 {
            statement,
            attestation: attestation.payload().try_into().unwrap(),
        })
        .unwrap();
        let current = read_active(fixture.state.view().world(), provider)
            .unwrap()
            .unwrap();
        assert!(
            fixture.commit_instruction(
                MutateSorafsStreamTokenCustody {
                    provider_id: provider,
                    expected_revision: current.index.revision,
                    expected_digest: current.index.digest,
                    action: SorafsStreamTokenCustodyActionV1::Enroll(fixture.record.clone()),
                }
                .into(),
                1,
                now - 1
            )
        );
        fixture
    }

    /// Sign one exact native instruction with the selected test actor and execute it.
    /// # Panics
    /// Panics on malformed signing input or the same fixture admission errors as `commit_signed`.
    pub fn commit_instruction(&mut self, instruction: InstructionBox, seed: u8, now: u64) -> bool {
        let key = Self::key(seed);
        let mut builder = TransactionBuilder::new(
            *self.state.network_id_ref(),
            AccountId::new(key.public_key().clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        );
        builder.set_creation_time(Duration::from_millis(now));
        let signed = builder
            .with_instructions([instruction])
            .try_sign(key.private_key())
            .unwrap();
        self.commit_signed(signed, now)
    }

    /// Execute the original signed envelope and retain its actual result and durable finality.
    /// # Panics
    /// Rejects foreign networks, invalid signatures, changed parent/frontier, exhausted test
    /// schedule and unsupported instructions before execution. No arbitrary callback is accepted.
    pub fn commit_signed(&mut self, signed: SignedTransaction, now: u64) -> bool {
        assert!(self.finalized.len() < 255);
        let view = self.state.view();
        assert_eq!(
            view.height(),
            self.finalized.len(),
            "fixture frontier changed"
        );
        assert_eq!(
            view.latest_block_hash(),
            self.finalized.last().map(|p| p.block().hash()),
            "fixture parent changed"
        );
        assert_eq!(
            signed.network_id(),
            Some(self.state.network_id_ref()),
            "foreign fixture network"
        );
        signed.verify_signature().expect("original signed envelope");
        drop(view);
        execution::commit_native_operation(
            &self.state,
            &mut self.finalized,
            now,
            vec![signed],
            true,
            true,
        )[0]
    }

    /// Remove the durable finality record of one committed fixture height.
    ///
    /// The block stays committed; finality readers then report that height as unfinalized.
    ///
    /// # Errors
    /// Returns the filesystem error when the isolated fixture record cannot be removed.
    pub fn remove_finality_for_test(&self, height: u64) -> std::io::Result<()> {
        std::fs::remove_file(
            self.state
                .kura()
                .v2_finality_artifact_path_for_testing(height),
        )
    }

    /// Execute revocation of one scoped operator or observer permission under the manager.
    /// # Panics
    /// Panics on fixture construction or signed-execution errors.
    pub fn revoke_runtime_permission(&mut self, observer: bool, now: u64) -> bool {
        let (seed, permission): (u8, Permission) = if observer {
            (
                3,
                CanCheckSorafsStreamToken {
                    provider_id: self.provider,
                }
                .into(),
            )
        } else {
            (
                2,
                CanOperateSorafsStreamToken {
                    provider_id: self.provider,
                }
                .into(),
            )
        };
        self.commit_instruction(
            Revoke::account_permission(
                permission,
                AccountId::new(Self::key(seed).public_key().clone()),
            )
            .into(),
            1,
            now,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::stream_token_custody::read_current_stream_token_custody_block_finality_v1;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    fn restored_stream_fixture_executes_anchored_enrollment_with_exact_durable_quorum() {
        let fixture = StreamTokenRuntimeTestFixtureV1::new_at(1_700_000_000_000);
        let actual = read_current_stream_token_custody_block_finality_v1(
            &fixture.state.view(),
            &fixture.policy.binding,
            2,
        )
        .unwrap()
        .unwrap();
        assert!(actual.custody().state.active_head.is_some());
        assert_eq!(fixture.state.view().height(), 2);
        for block in &fixture.finalized {
            let proof = &block.proof().finality_artifact;
            assert_eq!(proof.height_context.roster.len(), 4);
            assert_eq!(proof.commit_qc.signers.len(), 3);
            proof.verify().unwrap();
        }
    }

    #[test]
    fn restored_stream_fixture_rejects_unsupported_execution_before_frontier_mutation() {
        use iroha_data_model::{domain::Domain, isi::Register};
        use iroha_model_base::domain::DomainId;
        let mut fixture = StreamTokenRuntimeTestFixtureV1::new_at(1_700_000_000_000);
        let before = fixture.state.view().latest_block_hash();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                fixture.commit_instruction(
                    Register::domain(Domain::new(
                        DomainId::try_new("unsupported", "universal").unwrap(),
                    ))
                    .into(),
                    1,
                    1_700_000_000_001,
                )
            }))
            .is_err()
        );
        assert_eq!(fixture.state.view().latest_block_hash(), before);
        assert_eq!(fixture.state.view().height(), 2);
    }
    #[test]
    fn restored_stream_fixture_authenticates_actual_signed_reserve_position() {
        use crate::query::stream_token_authority::{
            observation::capture_stream_token_authority_v1, read_slot,
        };
        use iroha_data_model::{
            isi::sorafs::MutateSorafsStreamTokenAuthority,
            sorafs::stream_token_authority::{
                StreamTokenAuthorityActionV1, StreamTokenAuthorityRequestV1, StreamTokenReviewedV1,
            },
        };
        use sorafs_manifest::signer::{
            protocol::{
                SignerOperationActionV1, SignerOperationCustodyV1, SignerOperationIntentV1,
            },
            stream_token::{SignerStreamTokenRequestV1, stream_token_binding_digest_v1},
        };
        let now = 1_700_000_000_000;
        let mut fixture = StreamTokenRuntimeTestFixtureV1::new_at(now);
        let current = capture_stream_token_authority_v1(
            &fixture.state.view(),
            &fixture.policy.binding,
            [0; 32],
        )
        .unwrap();
        let request = SignerStreamTokenRequestV1 {
            operation_id: [31; 32],
            binding_digest: stream_token_binding_digest_v1(&fixture.policy.binding).unwrap(),
            original_custody: SignerOperationCustodyV1 {
                record_digest: current.control.active_head.unwrap().record_digest,
                control_state_digest: current.anchor.state_digest,
            },
            signing_payload_digest: [32; 32],
            signing_payload_size: 256,
            issued_at_unix_ms: now,
            expires_at_unix_ms: now + 60_000,
        };
        let reviewed = StreamTokenReviewedV1 {
            request,
            intent: SignerOperationIntentV1 {
                action: SignerOperationActionV1::Sign,
                operation_id: request.operation_id,
                request_digest: request.digest().unwrap(),
                previous_audit: current.head.audit,
            },
        };
        assert!(
            fixture.commit_instruction(
                MutateSorafsStreamTokenAuthority {
                    request: StreamTokenAuthorityRequestV1 {
                        network_id: *fixture.state.network_id_ref().as_bytes(),
                        provider_id: fixture.provider,
                        expected_control_revision: current.control_revision,
                        expected_control_digest: current.anchor.state_digest,
                        action: StreamTokenAuthorityActionV1::Reserve(reviewed),
                    }
                }
                .into(),
                2,
                now
            )
        );
        let record = read_slot(
            fixture.state.view().world(),
            fixture.provider,
            request.operation_id,
        )
        .unwrap()
        .unwrap();
        let execution = record.operation.reserved_execution;
        assert_eq!(execution.height, 3);
        assert_eq!(execution.entry_index, 0);
        assert_eq!(execution.instruction_index, 0);
        assert_eq!(
            execution.authority,
            AccountId::new(StreamTokenRuntimeTestFixtureV1::key(2).public_key().clone())
        );
        assert_eq!(
            fixture
                .finalized
                .last()
                .unwrap()
                .proof()
                .finality_artifact
                .height,
            3
        );
        assert!(fixture.revoke_runtime_permission(false, now + 1));
    }

    #[test]
    fn restored_stream_fixture_rejects_networkless_signed_input_before_mutation() {
        use iroha_data_model::isi::Log;
        let mut fixture = StreamTokenRuntimeTestFixtureV1::new_at(1_700_000_000_000);
        let key = StreamTokenRuntimeTestFixtureV1::key(1);
        let signed = TransactionBuilder::new_genesis(
            AccountId::new(key.public_key().clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            iroha_data_model::Level::INFO,
            "foreign genesis".into(),
        )])
        .try_sign(key.private_key())
        .unwrap();
        let before = fixture.state.view().latest_block_hash();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || fixture.commit_signed(signed, 1_700_000_000_001)
            ))
            .is_err()
        );
        assert_eq!(fixture.state.view().latest_block_hash(), before);
        assert_eq!(fixture.state.view().height(), 2);
    }
}
