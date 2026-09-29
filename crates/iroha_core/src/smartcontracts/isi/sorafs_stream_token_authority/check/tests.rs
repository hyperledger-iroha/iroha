//! Check predicate regressions. Synthetic rows below are shape tests, never execution evidence.

use super::*;
use crate::{
    kura::Kura,
    query::{store::LiveQueryStore, stream_token_custody::ControlIndexV1},
    state::{State, World},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    IntoKeyValue, Registrable,
    account::Account,
    block::{BlockHeader, consensus_v2::HeightContextId},
    permission::Permissions,
    sorafs::{
        stream_token_authority::{StreamTokenAuthorityRequestV1, StreamTokenOperationV1},
        stream_token_custody::StreamTokenCustodyControlRecordV1,
    },
};
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsStreamToken, CanOperateSorafsStreamToken,
};
use sorafs_manifest::signer::{
    custody::{
        SignerCustodyActiveHeadV1, SignerCustodyAnchorV1, SignerCustodyAuthorityV1,
        SignerCustodyBindingV1,
    },
    custody_control::{SignerCustodyControlStateV1, SignerCustodyPolicyV1},
    protocol::{
        SignerKeyAlgorithmV1, SignerOperationActionV1, SignerOperationAuditHeadV1,
        SignerOperationCustodyV1, SignerOperationIntentV1, SignerOperationReservationV1,
        SignerPurposeBindingV1, SignerRoleV1,
    },
    stream_token::SignerStreamTokenRequestV1,
};

fn key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("fixture key")
}
fn account(seed: u8) -> AccountId {
    AccountId::new(key(seed).public_key().clone())
}
fn state(world: World) -> State {
    State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    )
}
fn floor() -> StreamTokenFinalityFloorV1 {
    StreamTokenFinalityFloorV1 {
        height: 1,
        block_hash: [0x51; 32],
        context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(b"role11 floor"))),
    }
}
fn control(state: &State, provider: ProviderId, operator: &AccountId) -> NativeControl {
    let binding = SignerCustodyBindingV1 {
        chain_id: state.view().chain_id().to_string(),
        network_id: *state.view().network_id().as_bytes(),
        runtime_handle: "software://stream-token/primary".into(),
        key_handle: "software://stream-token/key-1".into(),
        service_id: "stream-token-service".into(),
        administrator_id: "stream-token-admin".into(),
        role: SignerRoleV1::StreamToken,
        purpose: SignerPurposeBindingV1::StreamToken {
            provider_id: *provider.as_bytes(),
        },
        algorithm: SignerKeyAlgorithmV1::Ed25519,
        public_key: key(3).public_key().clone(),
        key_revision: 1,
        policy_revision: 1,
        policy_digest: [0x52; 32],
    };
    let policy = SignerCustodyPolicyV1 {
        binding,
        attester_authority: SignerCustodyAuthorityV1 {
            service_id: "custody-service".into(),
            administrator_id: "custody-admin".into(),
            key_revision: 1,
            policy_revision: 1,
            policy_digest: [0x53; 32],
        },
        attester_public_key: key(4).public_key().clone(),
        active_from_unix_ms: 100,
        active_until_unix_ms: 10_000,
        max_validity_ms: 5_000,
        max_anchor_age_ms: 1_000,
    };
    let active_head = SignerCustodyActiveHeadV1 {
        record_digest: [0x54; 32],
        sequence: 1,
        approved_anchor: SignerCustodyAnchorV1 {
            height: 1,
            block_hash: [0x55; 32],
            state_digest: [0x56; 32],
        },
        key_revision: 1,
        policy_revision: 1,
        policy_digest: [0x52; 32],
    };
    NativeControl {
        record: StreamTokenCustodyControlRecordV1 {
            provider_id: provider,
            revision: 1,
            predecessor_digest: [0; 32],
            request_digest: [0x57; 32],
            execution_height: 1,
            ordinal: 0,
            recorded_at_unix_ms: 1_000,
            authority: operator.clone(),
            control_state: Vec::new(),
        },
        state: SignerCustodyControlStateV1 {
            policy,
            next_sequence: 2,
            predecessor_digest: active_head.record_digest,
            active_head: Some(active_head),
            signer_revoked: false,
            attester_revoked: false,
        },
        index: ControlIndexV1 {
            revision: 1,
            digest: [0x58; 32],
            height: 1,
            ordinal: 0,
        },
    }
}
fn instruction(
    state: &State,
    provider: ProviderId,
    current: &NativeControl,
    operator: AccountId,
    observer: AccountId,
) -> MutateSorafsStreamTokenAuthority {
    let request = SignerStreamTokenRequestV1 {
        operation_id: [0x59; 32],
        binding_digest: stream_token_binding_digest_v1(&current.state.policy.binding).unwrap(),
        original_custody: SignerOperationCustodyV1 {
            record_digest: current.state.active_head.unwrap().record_digest,
            control_state_digest: current.index.digest,
        },
        signing_payload_digest: [0x5a; 32],
        signing_payload_size: 256,
        issued_at_unix_ms: 1_000,
        expires_at_unix_ms: 5_000,
    };
    let reviewed = StreamTokenReviewedV1 {
        request,
        intent: SignerOperationIntentV1 {
            action: SignerOperationActionV1::Sign,
            operation_id: request.operation_id,
            request_digest: request.digest().unwrap(),
            previous_audit: SignerOperationAuditHeadV1 {
                sequence: 0,
                digest: [0; 32],
            },
        },
    };
    MutateSorafsStreamTokenAuthority {
        request: StreamTokenAuthorityRequestV1 {
            network_id: *state.view().network_id().as_bytes(),
            provider_id: provider,
            expected_control_revision: current.index.revision,
            expected_control_digest: current.index.digest,
            action: Action::Check(StreamTokenCheckV1 {
                challenge: [0x5b; 32],
                expected_operator: operator,
                expected_observer: observer,
                floor: floor(),
                reviewed,
                phase: Phase::Current(reviewed.intent.previous_audit),
            }),
        },
    }
}
fn check(instruction: &MutateSorafsStreamTokenAuthority) -> &StreamTokenCheckV1 {
    let Action::Check(check) = &instruction.request.action else {
        panic!("test Check instruction")
    };
    check
}
fn check_mut(instruction: &mut MutateSorafsStreamTokenAuthority) -> &mut StreamTokenCheckV1 {
    let Action::Check(check) = &mut instruction.request.action else {
        panic!("test Check instruction")
    };
    check
}

/// A certified chain with one block after genesis, committed with the local `CommitQC` of
/// `signers`, and the floor of that block.
fn certified_floor(
    signers: crate::sumeragi::test_chain::Signers,
) -> (
    crate::sumeragi::test_chain::CertifiedTestChain,
    StreamTokenFinalityFloorV1,
) {
    let mut chain = crate::query::signer_check::fixture::chain(World::new());
    chain.commit_with(Some(1_500), Vec::new(), signers);
    let committed = chain.committed(2);
    let floor = StreamTokenFinalityFloorV1 {
        height: 2,
        block_hash: *committed.block_hash().as_ref(),
        context_id: committed.id(),
    };
    (chain, floor)
}

/// The check of `floor` inside the next block's transaction.
fn check_next(
    chain: &crate::sumeragi::test_chain::CertifiedTestChain,
    floor: StreamTokenFinalityFloorV1,
    challenge: [u8; 32],
    execution_height: u64,
) -> Result<(), Error> {
    let state = chain.state();
    let mut block = state.block(BlockHeader::new(
        3_u64.try_into().unwrap(),
        state.view().latest_block_hash(),
        None,
        2_000,
        0,
    ));
    let tx = block.transaction();
    check_floor(&tx, floor, challenge, execution_height)
}

#[test]
fn signed_floor_requires_exact_state_hash_and_committed_block_id() {
    use crate::sumeragi::test_chain::Signers;
    let (chain, actual) = certified_floor(Signers::Quorum);
    assert_eq!(check_next(&chain, actual, [1; 32], 3), Ok(()));
    let mut changed = actual;
    changed.block_hash = [0x99; 32];
    assert_eq!(
        check_next(&chain, changed, [1; 32], 3),
        Err(Error::Finality)
    );
    changed = actual;
    changed.context_id = floor().context_id;
    assert_eq!(
        check_next(&chain, changed, [1; 32], 3),
        Err(Error::Finality)
    );
    // The height of another committed block.
    changed = actual;
    changed.height = 1;
    assert_eq!(
        check_next(&chain, changed, [1; 32], 3),
        Err(Error::Finality)
    );
    assert_eq!(
        check_next(&chain, actual, [0; 32], 3),
        Err(Error::BindingMismatch)
    );
    assert_eq!(
        check_next(&chain, actual, [1; 32], 2),
        Err(Error::BindingMismatch)
    );
    // Without the committed frame the floor is not a committed block.
    chain
        .kura()
        .corrupt_canonical_body_for_testing(std::num::NonZeroUsize::new(2).unwrap())
        .unwrap();
    assert_eq!(check_next(&chain, actual, [1; 32], 3), Err(Error::Finality));
}

/// Certificates are per node: nodes that stored different valid `CommitQC`s for the floor
/// block (other signers), or even a local certificate that does not verify, execute the Check
/// identically. The instruction reads the committed header and result preimage only.
#[test]
fn floor_check_is_independent_of_the_local_commit_certificate() {
    use crate::sumeragi::test_chain::Signers;
    let (quorum, floor) = certified_floor(Signers::Quorum);
    for signers in [Signers::LastThree, Signers::All, Signers::BelowQuorum] {
        let (other, other_floor) = certified_floor(signers);
        assert_eq!(
            other_floor, floor,
            "the same block and certified id on every node"
        );
        assert_ne!(
            other.committed(2).block().commit_certificate(),
            quorum.committed(2).block().commit_certificate(),
            "the local certificates differ"
        );
        for (challenge, height) in [([1; 32], 3), ([0; 32], 3), ([1; 32], 2)] {
            assert_eq!(
                check_next(&other, floor, challenge, height),
                check_next(&quorum, floor, challenge, height)
            );
        }
    }
}

#[test]
fn current_custody_claim_rejects_observer_signer_revocation_and_binding_changes() {
    let state = state(World::new());
    let provider = ProviderId::new([0x61; 32]);
    let operator = account(1);
    let observer = account(2);
    let mut current = control(&state, provider, &operator);
    let mut instruction = instruction(
        &state,
        provider,
        &current,
        operator.clone(),
        observer.clone(),
    );
    assert_eq!(
        check_live_custody(&current, check(&instruction), &observer, 2_000),
        Ok(())
    );
    assert_eq!(
        check_live_custody(&current, check(&instruction), &operator, 2_000),
        Err(Error::Custody)
    );
    assert_eq!(
        check_live_custody(&current, check(&instruction), &account(3), 2_000),
        Err(Error::Custody)
    );
    current.state.signer_revoked = true;
    assert_eq!(
        check_live_custody(&current, check(&instruction), &observer, 2_000),
        Err(Error::Custody)
    );
    current.state.signer_revoked = false;
    current.state.attester_revoked = true;
    assert_eq!(
        check_live_custody(&current, check(&instruction), &observer, 2_000),
        Err(Error::Custody)
    );
    current.state.attester_revoked = false;
    check_mut(&mut instruction).reviewed.request.binding_digest = [0x99; 32];
    assert_eq!(
        check_live_custody(&current, check(&instruction), &observer, 2_000),
        Err(Error::Custody)
    );
    check_mut(&mut instruction).reviewed.request.binding_digest =
        stream_token_binding_digest_v1(&current.state.policy.binding).unwrap();
    check_mut(&mut instruction)
        .reviewed
        .request
        .original_custody
        .control_state_digest = [0x98; 32];
    assert_eq!(
        check_live_custody(&current, check(&instruction), &observer, 2_000),
        Err(Error::Custody)
    );
}

#[test]
fn scoped_observer_grant_does_not_make_operator_or_protected_signer_an_observer() {
    let provider = ProviderId::new([0x62; 32]);
    let operator = account(1);
    let observer = account(2);
    let signer = account(3);
    let foreign = account(5);
    let mut world = World::new();
    for id in [&operator, &observer, &signer, &foreign] {
        let (key, value) = Account::new(id.clone()).build(&operator).into_key_value();
        world.accounts.insert(key, value);
    }
    world.provider_owners.insert(provider, operator.clone());
    for (id, permission) in [
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
        (
            &signer,
            Permission::from(CanCheckSorafsStreamToken {
                provider_id: provider,
            }),
        ),
    ] {
        let mut permissions = Permissions::new();
        permissions.insert(permission);
        world.account_permissions.insert(id.clone(), permissions);
    }
    let state = state(world);
    let current = control(&state, provider, &operator);
    let instruction = instruction(
        &state,
        provider,
        &current,
        operator.clone(),
        observer.clone(),
    );
    let mut block = state.block(BlockHeader::new(
        1_u64.try_into().unwrap(),
        None,
        None,
        2_000,
        0,
    ));
    let tx = block.transaction();
    assert!(authorized(
        &tx,
        &observer,
        provider,
        &instruction.request.action
    ));
    assert!(!authorized(
        &tx,
        &operator,
        provider,
        &instruction.request.action
    ));
    assert!(!authorized(
        &tx,
        &signer,
        provider,
        &instruction.request.action
    ));
    assert!(!authorized(
        &tx,
        &foreign,
        provider,
        &instruction.request.action
    ));
    let mut changed = instruction;
    check_mut(&mut changed).expected_observer = signer.clone();
    assert!(authorized(&tx, &signer, provider, &changed.request.action));
    assert_eq!(
        check_live_custody(&current, check(&changed), &signer, 2_000),
        Err(Error::Custody)
    );
}

#[test]
fn retained_phase_rejects_row_and_phase_substitution_and_never_writes() {
    let provider = ProviderId::new([0x63; 32]);
    let operator = account(1);
    let observer = account(2);
    let base = state(World::new());
    let current = control(&base, provider, &operator);
    let mut instruction = instruction(
        &base,
        provider,
        &current,
        operator.clone(),
        observer.clone(),
    );
    let reviewed = check(&instruction).reviewed;
    let row = StreamTokenNativeOperationV1 {
        provider_id: provider,
        custody_control_revision: current.index.revision,
        custody_control_digest: current.index.digest,
        operation: StreamTokenOperationV1 {
            reviewed,
            reservation: SignerOperationReservationV1 {
                reservation_id: [0x64; 32],
                fence: 1,
                expires_at_unix_ms: 4_000,
            },
            outcome: StreamTokenOutcomeV1::Reserved,
        },
        reserved_execution: StreamTokenExecutionV1 {
            height: 1,
            transaction_hash: [0x65; 32],
            entry_index: 0,
            instruction_index: 0,
            recorded_at_unix_ms: 1_500,
            authority: operator,
        },
        terminal_execution: None,
    };
    let record = OperationRecordV1 {
        revision: 1,
        predecessor_digest: [0; 32],
        request_digest: [0x66; 32],
        operation: row.clone(),
    };
    let mut world = World::new();
    world.smart_contract_state.insert(
        journal::record_key(provider, 1),
        journal::encode(&record).unwrap(),
    );
    world.smart_contract_state.insert(
        journal::admission_key(provider, reviewed.request.operation_id),
        journal::encode(&1_u64).unwrap(),
    );
    world.smart_contract_state.insert(
        journal::slot_key(provider, reviewed.request.operation_id),
        journal::encode(&1_u64).unwrap(),
    );
    world.smart_contract_state.insert(
        journal::head_key(provider),
        journal::encode(&OperationHeadV1 {
            revision: 1,
            digest: journal::record_digest(&record).unwrap(),
            fence: 1,
            audit: reviewed.intent.previous_audit,
            active_operation: Some(reviewed.request.operation_id),
            total_admissions: 1,
        })
        .unwrap(),
    );
    let state = state(world);
    check_mut(&mut instruction).phase = Phase::BeforeProvider(row.clone());
    let mut block = state.block(BlockHeader::new(
        1_u64.try_into().unwrap(),
        None,
        None,
        2_000,
        0,
    ));
    let mut tx = block.transaction();
    assert_eq!(
        check_retained_phase(
            &instruction,
            &observer,
            &tx,
            check(&instruction),
            &current,
            2_000
        ),
        Ok(())
    );
    assert_eq!(
        check_retained_phase(
            &instruction,
            &observer,
            &tx,
            check(&instruction),
            &current,
            1_499
        ),
        Err(Error::Conflict)
    );
    let mut changed = instruction.clone();
    check_mut(&mut changed).phase = Phase::AfterCommit(row.clone());
    assert_eq!(
        check_retained_phase(&changed, &observer, &tx, check(&changed), &current, 2_000),
        Err(Error::Conflict)
    );
    check_mut(&mut changed).phase = Phase::Current(reviewed.intent.previous_audit);
    assert_eq!(
        check_retained_phase(&changed, &observer, &tx, check(&changed), &current, 2_000),
        Err(Error::Conflict)
    );
    let mut changed_row = row;
    changed_row.operation.reservation.fence = 2;
    check_mut(&mut changed).phase = Phase::BeforeProvider(changed_row);
    assert_eq!(
        check_retained_phase(&changed, &observer, &tx, check(&changed), &current, 2_000),
        Err(Error::Conflict)
    );
    assert_eq!(
        tx.world
            .smart_contract_state
            .get(&journal::head_key(provider)),
        state
            .view()
            .world
            .smart_contract_state
            .get(&journal::head_key(provider))
    );
    let result = apply(
        instruction,
        &observer,
        &mut tx,
        StreamTokenExecutionV1 {
            height: 1,
            transaction_hash: [0x67; 32],
            entry_index: 0,
            instruction_index: 0,
            recorded_at_unix_ms: 2_000,
            authority: observer.clone(),
        },
    );
    assert_eq!(result, Err(Error::BindingMismatch));
    assert_eq!(
        tx.world
            .smart_contract_state
            .get(&journal::head_key(provider)),
        state
            .view()
            .world
            .smart_contract_state
            .get(&journal::head_key(provider))
    );
}
