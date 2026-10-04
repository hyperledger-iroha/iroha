//! Actual native custody, operation and Check execution on a certified test chain.
//!
//! The shared fixture executes signed transactions through the node's block path; every block is
//! certified by a real three-of-four BLS `CommitQC` (height 1 is the signed genesis). It is not a
//! four-process consensus or production application-state-root qualification.

use super::*;
use crate::{query::signer_check::fixture, state::World, sumeragi::test_chain::CertifiedTestChain};
use iroha_crypto::Signature;
use iroha_data_model::{
    IntoKeyValue, Registrable,
    account::Account,
    isi::{InstructionBox, Revoke, sorafs::MutateSorafsStreamTokenCustody},
    permission::{Permission, Permissions},
    sorafs::{
        stream_token_authority::{StreamTokenCompleteRequestV1, StreamTokenOutcomeV1},
        stream_token_custody::SorafsStreamTokenCustodyActionV1,
    },
};
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsStreamToken, CanManageSorafsStreamTokenCustody, CanOperateSorafsStreamToken,
};
use sorafs_manifest::signer::{
    custody::{
        SIGNER_CUSTODY_MAGIC_V1, SIGNER_CUSTODY_VERSION_V1, SignerCustodyAuthorityV1,
        SignerCustodyRecordV1, SignerCustodyStatementV1,
    },
    custody_control::SignerCustodyPolicyV1,
    protocol::{
        SignerKeyAlgorithmV1, SignerOperationActionV1, SignerOperationAuditHeadV1,
        SignerOperationCommitmentV1, SignerOperationCustodyV1, SignerOperationIntentV1,
        SignerRoleV1,
    },
    stream_token::SignerStreamTokenRequestV1,
};

const NOW: u64 = 4_000;

fn account(seed: u8) -> AccountId {
    AccountId::new(fixture::key(seed).public_key().clone())
}

struct Fixture {
    chain: CertifiedTestChain,
    state: Arc<State>,
    provider: ProviderId,
    policy: SignerCustodyPolicyV1,
}
impl Fixture {
    fn new() -> Self {
        let provider = ProviderId::new([19; 32]);
        let mut world = World::new();
        for seed in 1..=3 {
            let (id, value) = Account::new(account(seed))
                .build(&account(1))
                .into_key_value();
            world.accounts.insert(id, value);
        }
        world.provider_owners.insert(provider, account(2));
        for (seed, permission) in [
            (
                1,
                Permission::from(CanManageSorafsStreamTokenCustody {
                    provider_id: provider,
                }),
            ),
            (
                2,
                Permission::from(CanOperateSorafsStreamToken {
                    provider_id: provider,
                }),
            ),
            (
                3,
                Permission::from(CanCheckSorafsStreamToken {
                    provider_id: provider,
                }),
            ),
        ] {
            let mut permissions = Permissions::new();
            permissions.insert(permission);
            world.account_permissions.insert(account(seed), permissions);
        }
        let chain = fixture::chain(world);
        let state = Arc::clone(chain.state());
        let policy = SignerCustodyPolicyV1 {
            binding: SignerCustodyBindingV1 {
                chain_id: state.view().chain_id().to_string(),
                network_id: *state.network_id_ref().as_bytes(),
                runtime_handle: "software://sorafs/stream-token/primary".into(),
                key_handle: "software://sorafs/stream-token/key-1".into(),
                service_id: "stream-token-service".into(),
                administrator_id: "stream-token-admin".into(),
                role: SignerRoleV1::StreamToken,
                purpose: SignerPurposeBindingV1::StreamToken {
                    provider_id: *provider.as_bytes(),
                },
                algorithm: SignerKeyAlgorithmV1::Ed25519,
                public_key: fixture::key(4).public_key().clone(),
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
            attester_public_key: fixture::key(7).public_key().clone(),
            active_from_unix_ms: 100,
            active_until_unix_ms: 500_000,
            max_validity_ms: 120_000,
            max_anchor_age_ms: 60_000,
        };
        let mut fixture = Self {
            chain,
            state,
            provider,
            policy,
        };
        let configure = MutateSorafsStreamTokenCustody {
            provider_id: provider,
            expected_revision: 0,
            expected_digest: [0; 32],
            action: SorafsStreamTokenCustodyActionV1::Configure(
                norito::encode_canonical(&fixture.policy).unwrap(),
            ),
        };
        fixture.execute(configure.into(), 1, 1_000);
        let current = read_stream_token_custody_control_at_v1(
            &fixture.state.view(),
            &fixture.policy.binding,
            2,
        )
        .unwrap()
        .unwrap();
        let statement = SignerCustodyStatementV1 {
            magic: SIGNER_CUSTODY_MAGIC_V1,
            version: SIGNER_CUSTODY_VERSION_V1,
            binding: fixture.policy.binding.clone(),
            authority: fixture.policy.attester_authority.clone(),
            anchor: current.anchor,
            sequence: current.state.next_sequence,
            predecessor_digest: current.state.predecessor_digest,
            issued_at_unix_ms: 1_500,
            expires_at_unix_ms: 100_000,
            evidence_digest: [8; 32],
            revoked: false,
        };
        let signature = Signature::try_new(
            fixture::key(7).private_key(),
            &statement.signing_payload().unwrap(),
        )
        .unwrap();
        let enrollment = SignerCustodyRecordV1 {
            statement,
            attestation: signature.payload().try_into().unwrap(),
        };
        let enroll = MutateSorafsStreamTokenCustody {
            provider_id: provider,
            expected_revision: 1,
            expected_digest: current.anchor.state_digest,
            action: SorafsStreamTokenCustodyActionV1::Enroll(
                norito::encode_canonical(&enrollment).unwrap(),
            ),
        };
        fixture.execute(enroll.into(), 1, 1_500);
        fixture
    }
    fn execute(&mut self, instruction: InstructionBox, seed: u8, now: u64) {
        let signed = fixture::sign(&self.state, instruction, seed, now);
        assert_eq!(fixture::commit(&mut self.chain, now, vec![signed]), [true]);
    }
    fn expected(&self) -> StreamTokenCheckExpectedV1 {
        let control = read_active(self.state.view().world(), self.provider)
            .unwrap()
            .unwrap();
        let request = SignerStreamTokenRequestV1 {
            operation_id: [31; 32],
            binding_digest: stream_token_binding_digest_v1(&self.policy.binding).unwrap(),
            original_custody: SignerOperationCustodyV1 {
                record_digest: control.state.active_head.unwrap().record_digest,
                control_state_digest: control.index.digest,
            },
            signing_payload_digest: [32; 32],
            signing_payload_size: 256,
            issued_at_unix_ms: 1_500,
            expires_at_unix_ms: 90_000,
        };
        let audit = SignerOperationAuditHeadV1 {
            sequence: 0,
            digest: [0; 32],
        };
        let reviewed = StreamTokenReviewedV1 {
            request,
            intent: SignerOperationIntentV1 {
                action: SignerOperationActionV1::Sign,
                operation_id: request.operation_id,
                request_digest: request.digest().unwrap(),
                previous_audit: audit,
            },
        };
        let tip = self.chain.committed(self.chain.height());
        StreamTokenCheckExpectedV1 {
            binding: self.policy.binding.clone(),
            observer: account(3),
            expected_operator: account(2),
            control_revision: control.index.revision,
            control_digest: control.index.digest,
            reviewed,
            phase: Phase::Current(audit),
            floor: StreamTokenFinalityFloorV1 {
                height: tip.height(),
                block_hash: *tip.block_hash().as_ref(),
                context_id: tip.id(),
            },
        }
    }
    fn pending(&self, expected: StreamTokenCheckExpectedV1) -> PendingStreamTokenCheckV1 {
        let prepared =
            begin_stream_token_check_v1(self.state.clone(), expected, Duration::from_secs(60))
                .unwrap();
        let signed = fixture::sign(&self.state, prepared.instruction().clone().into(), 3, NOW);
        prepared.bind_signed_transaction(signed).unwrap()
    }
    /// Apply the Check in a block whose local `CommitQC` verifies (`finality`) or does not.
    fn apply(&mut self, pending: &PendingStreamTokenCheckV1, finality: bool) {
        let transactions = vec![pending.signed_transaction().clone()];
        let outcomes = if finality {
            fixture::commit(&mut self.chain, NOW, transactions)
        } else {
            fixture::commit_uncertified(&mut self.chain, NOW, transactions)
        };
        assert_eq!(outcomes, [true]);
    }
    fn complete(&mut self) -> StreamTokenCheckExpectedV1 {
        let initial = self.expected();
        let request = StreamTokenAuthorityRequestV1 {
            network_id: self.policy.binding.network_id,
            provider_id: self.provider,
            expected_control_revision: initial.control_revision,
            expected_control_digest: initial.control_digest,
            action: Action::Reserve(initial.reviewed),
        };
        self.execute(
            MutateSorafsStreamTokenAuthority { request }.into(),
            2,
            2_000,
        );
        let row = read_slot(
            self.state.view().world(),
            self.provider,
            initial.reviewed.request.operation_id,
        )
        .unwrap()
        .unwrap();
        let request = StreamTokenAuthorityRequestV1 {
            network_id: self.policy.binding.network_id,
            provider_id: self.provider,
            expected_control_revision: initial.control_revision,
            expected_control_digest: initial.control_digest,
            action: Action::Complete(StreamTokenCompleteRequestV1 {
                reviewed: initial.reviewed,
                reservation: row.operation.operation.reservation,
                commitment: SignerOperationCommitmentV1 {
                    audit: SignerOperationAuditHeadV1 {
                        sequence: 1,
                        digest: [33; 32],
                    },
                    response_digest: [34; 32],
                },
                signatures_digest: [35; 32],
            }),
        };
        self.execute(
            MutateSorafsStreamTokenAuthority { request }.into(),
            2,
            3_000,
        );
        let row = read_slot(
            self.state.view().world(),
            self.provider,
            initial.reviewed.request.operation_id,
        )
        .unwrap()
        .unwrap();
        assert!(matches!(
            row.operation.operation.outcome,
            StreamTokenOutcomeV1::Completed(_)
        ));
        let mut expected = self.expected();
        expected.phase = Phase::BeforeRelease(row.operation);
        expected
    }
}

fn now() -> Result<StreamTokenEligibilityTimeIntervalV1, Error> {
    Ok(StreamTokenEligibilityTimeIntervalV1 {
        earliest_unix_ms: NOW,
        latest_unix_ms: NOW + 1,
    })
}

#[test]
fn current_check_requires_actual_application_and_accepts_exact_native_finality() {
    let fixture = Fixture::new();
    let pending = fixture.pending(fixture.expected());
    assert_eq!(
        pending
            .verify_finalized(now)
            .err()
            .and_then(|failure| failure.rejection()),
        Some(Error::NotApplied)
    );
    let mut fixture = Fixture::new();
    let pending = fixture.pending(fixture.expected());
    fixture.apply(&pending, true);
    let verified = pending.verify_finalized(now).unwrap();
    assert!(verified.snapshot().operation().is_none());
    assert_eq!(verified.snapshot().anchor().height, 4);
    assert_eq!(verified.applied_floor().height, 4);
    assert!(!verified.canonical_external().is_empty());
    assert_eq!(verified.time_interval(), now().unwrap());
    verified.ensure_live().unwrap();
}

#[test]
fn completed_check_authenticates_original_reserve_complete_and_current_authority() {
    let mut fixture = Fixture::new();
    let expected = fixture.complete();
    let captured = capture_stream_token_authority_v1(
        &fixture.state.view(),
        &fixture.policy.binding,
        expected.reviewed.request.operation_id,
    )
    .unwrap();
    assert_eq!(captured.floor, expected.floor);
    assert_eq!(captured.operator, account(2));
    assert_eq!(
        captured
            .operation
            .as_ref()
            .unwrap()
            .operation
            .operation
            .reviewed,
        expected.reviewed
    );
    let pending = fixture.pending(expected);
    fixture.apply(&pending, true);
    let verified = pending.verify_finalized(now).unwrap();
    let operation = &verified.snapshot().operation().unwrap().operation;
    assert_eq!(operation.reserved_execution.height, 4);
    assert_eq!(operation.terminal_execution.as_ref().unwrap().height, 5);
    assert_eq!(verified.snapshot().anchor().height, 6);
    let completed = verified.snapshot().completed_operation().unwrap();
    assert_eq!(completed.anchor.height, 5);
    assert_eq!(
        completed.anchor.block_hash,
        *fixture.state.view().block_hashes().get(4).unwrap().as_ref()
    );
    assert_eq!(
        completed.anchor.operation_state_digest,
        super::super::record_digest(verified.snapshot().operation().unwrap()).unwrap()
    );
    assert_eq!(
        completed.completed_at_unix_ms,
        operation
            .terminal_execution
            .as_ref()
            .unwrap()
            .recorded_at_unix_ms
    );
}

#[test]
fn missing_check_finality_cannot_be_replaced_by_successful_application() {
    let mut fixture = Fixture::new();
    let expected = fixture.complete();
    let pending = fixture.pending(expected);
    fixture.apply(&pending, false);
    assert_eq!(
        pending
            .verify_finalized(now)
            .err()
            .and_then(|failure| failure.rejection()),
        Some(Error::Finality)
    );
}

#[test]
fn current_operator_permission_is_rechecked_after_successful_completed_check() {
    let mut fixture = Fixture::new();
    let expected = fixture.complete();
    let pending = fixture.pending(expected);
    fixture.apply(&pending, true);
    fixture.execute(
        Revoke::account_permission(
            Permission::from(CanOperateSorafsStreamToken {
                provider_id: fixture.provider,
            }),
            account(2),
        )
        .into(),
        2,
        NOW + 1,
    );
    assert_eq!(
        pending
            .verify_finalized(now)
            .err()
            .and_then(|failure| failure.rejection()),
        Some(Error::Authority)
    );
}

#[test]
fn both_utc_endpoints_and_original_monotonic_deadline_bound_the_capability() {
    let mut fixture = Fixture::new();
    let pending = fixture.pending(fixture.expected());
    fixture.apply(&pending, true);
    assert_eq!(
        pending
            .verify_finalized(|| Ok(StreamTokenEligibilityTimeIntervalV1 {
                earliest_unix_ms: NOW,
                latest_unix_ms: 90_000,
            }))
            .err()
            .and_then(|failure| failure.rejection()),
        Some(Error::Authority)
    );
    let mut fixture = Fixture::new();
    let mut pending = fixture.pending(fixture.expected());
    fixture.apply(&pending, true);
    pending.prepared.round.expire_for_test();
    assert_eq!(
        pending
            .verify_finalized(now)
            .err()
            .and_then(|failure| failure.rejection()),
        Some(Error::Expired)
    );
}

#[test]
fn preparation_rejects_wrong_scope_and_binding_consumes_substituted_transaction() {
    let fixture = Fixture::new();
    let mut expected = fixture.expected();
    expected.observer = expected.expected_operator.clone();
    assert_eq!(
        begin_stream_token_check_v1(fixture.state.clone(), expected, Duration::from_secs(60)).err(),
        Some(Error::Invalid)
    );
    let prepared = begin_stream_token_check_v1(
        fixture.state.clone(),
        fixture.expected(),
        Duration::from_secs(60),
    )
    .unwrap();
    prepared.ensure_live().unwrap();
    let mut wrong = prepared.instruction().clone();
    wrong.request.expected_control_digest = [99; 32];
    let signed = fixture::sign(&fixture.state, wrong.into(), 3, NOW);
    assert_eq!(
        prepared
            .bind_signed_transaction(signed)
            .err()
            .and_then(|failure| failure.rejection()),
        Some(Error::Transaction)
    );
}

#[path = "tests/single_walk.rs"]
mod single_walk;

#[test]
fn binding_local_refusal_retries_only_original_signed_custody() {
    let fixture = Fixture::new();
    let prepared = begin_stream_token_check_v1(
        fixture.state.clone(),
        fixture.expected(),
        Duration::from_secs(60),
    )
    .unwrap();
    let signed = fixture::sign(
        &fixture.state,
        prepared.instruction().clone().into(),
        3,
        NOW,
    );
    let expected_wire = signed.encode_wire_v1().unwrap();
    let deadline = prepared.deadline();
    let zero = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 128);
    let failure =
        match norito::with_decode_limits_scope(zero, || prepared.bind_signed_transaction(signed)) {
            Err(failure) => failure,
            Ok(_) => panic!("original caller allocation ceiling must survive"),
        };
    assert!(failure.error().is_retryable());
    assert!(failure.rejection().is_none());
    assert_eq!(failure.deadline(), deadline);
    let pending = failure.retry().unwrap();
    assert_eq!(pending.deadline(), deadline);
    assert_eq!(
        pending.signed_transaction().encode_wire_v1().unwrap(),
        expected_wire
    );
    assert!(Arc::ptr_eq(&pending.prepared.state, &fixture.state));
}
