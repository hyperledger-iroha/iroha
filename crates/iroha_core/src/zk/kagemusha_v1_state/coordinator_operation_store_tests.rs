//! Real WAL/process-restart and Core-index ordering tests for coordinator operation admission.
//! Test-only Core fixture verifiers exercise persistence and semantic state transitions; they are
//! not qualified provider evidence and are never linked into the production coordinator.

use super::super::private_journal::TestPersistenceFailure;
use super::*;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

type Machine =
    KagemushaStateMachineV1<AcceptSnapshotRecursiveVerifierV1, AcceptSnapshotGuardVerifierV1>;
type StoreError = KagemushaCoordinatorOperationStoreErrorV1;
type Recovery = KagemushaCoordinatorSenderIntentRecoveryV1;
const FILE: &str = "operations.norito.wal";
const CAPACITY: u64 = 8 * 1024 * 1024;

fn machine() -> (Machine, DigestV1, AccountId) {
    let artifacts = crate::zk::kagemusha_v1_recursion::tests::artifacts();
    let suite_id = snapshot_digest(b"snapshot-suite", 1);
    let vk_digest = snapshot_digest(b"snapshot-verifier-set", 2);
    let governance_key = SigningKey::from_bytes((&[8; 32]).into()).expect("governance key");
    let profile = snapshot_hardware_profile(suite_id, &governance_key);
    let enabled_profile = KagemushaEnabledProfileV1 {
        hardware_profile: profile,
        hardware_profile_id: profile.hardware_profile_id,
        suite_id,
        vk_digest,
        qualification_digest: snapshot_digest(b"snapshot-qualification-matrix", 3),
        policy_epoch: profile.policy_epoch,
        qualification_report: KagemushaEvidenceFileV1 {
            sha256: profile.qualification_report_digest,
            byte_len: 1,
        },
    };
    let proof_release =
        KagemushaStateProofReleaseV1::from_test_artifacts(artifacts, vec![enabled_profile])
            .expect("snapshot-test proof release");
    let payment_context =
        crate::zk::kagemusha_v1_recursion::tests::incoming_payment_fixture(1, 2, 3, 5, 32, 32)
            .request;
    let lane = KagemushaLaneIdV1 {
        network_id: payment_context.network_id,
        device_lane_id: snapshot_digest(b"snapshot-lane", 4),
        asset: payment_context.asset.clone(),
        scale: payment_context.scale,
    };
    let old_epoch = HardwareEpochV1 {
        generation: 7,
        epoch_id: snapshot_digest(b"snapshot-old-epoch", 5),
    };
    let old_device_key = SigningKey::from_bytes((&[17; 32]).into()).expect("old device key");
    let old_credential = snapshot_hardware_credential(
        lane.network_id,
        lane.device_lane_id,
        old_epoch,
        &profile,
        suite_id,
        &old_device_key,
        &governance_key,
    );
    let old_policy = DevicePolicyBindingV1 {
        device_key_reference: old_credential.device_key_reference,
        hardware_policy_id: snapshot_digest(b"snapshot-old-policy", 7),
    };
    let context = KagemushaStateContextV1 {
        protocol_version: KAGEMUSHA_STATE_VERSION_V1,
        suite_id,
        vk_digest,
        release_id: artifacts.release_id,
        asset_incarnation: payment_context.asset_incarnation,
        hardware_profile_id: profile.hardware_profile_id,
        policy_epoch: profile.policy_epoch,
    };
    let liability_pool_id = derive_liability_pool_id(&lane, payment_context.asset_incarnation)
        .expect("snapshot-test liability pool");
    let state = KagemushaStateV1::build(
        context,
        liability_pool_id,
        lane.clone(),
        1000,
        0,
        old_epoch,
        old_policy,
        snapshot_digest(b"snapshot-old-state-nonce", 8),
        ExactConsumedCreditIndex::empty().root(),
    )
    .expect("snapshot-test old-epoch state");
    let authenticated_history = KagemushaStateAuthenticatedHistoryV1::open(
        KagemushaMemoryAuthenticatedHistoryStoreV1::new(8 * 1024 * 1024),
    )
    .expect("empty authenticated history");
    let machine = KagemushaStateMachineV1 {
        state,
        journal_revision: 0,
        inbox_revision: 0,
        pending_credits: BTreeMap::new(),
        accepted_recipient_bindings: BTreeSet::from([old_policy]),
        accepted_payment_receipts: BTreeMap::new(),
        mint_inbox: KagemushaMintInboxV1::default(),
        consumed_credits: ExactConsumedCreditIndex::empty(),
        authenticated_history,
        receiver_inbox_capacity: KagemushaReceiverInboxCapacityV1::new(32 * 1024 * 1024),
        sender_outbox_capacity: KagemushaSenderOutboxCapacityV1::new(8 * 1024 * 1024),
        outgoing_candidate_journal: KagemushaOutgoingCandidateJournalV1::default(),
        proof_release: proof_release.clone(),
        recursive_verifier: AcceptSnapshotRecursiveVerifierV1,
        guard_verifier: AcceptSnapshotGuardVerifierV1,
    };

    (
        machine,
        old_credential.credential_id,
        payment_context.recipient,
    )
}

fn location() -> (tempfile::TempDir, PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().canonicalize().unwrap().join("operations");
    (root, path)
}
fn id(tag: u8) -> DigestV1 {
    [tag; 32]
}
fn intent(
    machine: &Machine,
    credential_id: DigestV1,
    beneficiary: AccountId,
    operation_id: DigestV1,
) -> KagemushaOutgoingPublicInputPreimageV1 {
    KagemushaOutgoingPublicInputPreimageV1 {
        version: KAGEMUSHA_STATE_VERSION_V1,
        operation_id,
        context: KagemushaOutgoingOperationContextV1 {
            lane: machine.state.lane.clone(),
            release: machine.state.context(),
            credential_id,
            hardware_epoch: machine.state.hardware_epoch,
            device_policy_binding: machine.state.device_policy_binding,
        },
        inputs: KagemushaOutgoingPublicInputsV1::RedeemSplit {
            amount: 20,
            beneficiary,
        },
    }
}
fn binding(intent: &KagemushaOutgoingPublicInputPreimageV1) -> Vec<u8> {
    norito::encode_canonical(&intent.inputs).unwrap()
}
fn prepare(machine: &mut Machine, intent: &KagemushaOutgoingPublicInputPreimageV1) {
    let KagemushaOutgoingPublicInputsV1::RedeemSplit {
        amount,
        beneficiary,
    } = &intent.inputs
    else {
        panic!("test redemption")
    };
    let candidate = machine
        .prepare_redeem_split(RedeemSplitPreparationV1 {
            amount: *amount,
            beneficiary: beneficiary.clone(),
            terminal_nullifier: id(61),
            redemption_commitment: id(62),
            successor_state_nonce_commitment: id(63),
            commit_evidence: KagemushaCommitEvidenceV1::TrustedTime(KagemushaTrustedCommitTimeV1 {
                time_evidence_commitment: id(64),
            }),
            commit_authorization_reference_ms: 500,
            outbox_reservation: KagemushaOutboxReservationV1 {
                reservation_id: id(65),
                operation_kind: KagemushaOperationKindV1::RedeemSplit,
                reserved_outbox_bytes: aggregate_outbox_reservation_bytes(
                    KagemushaOperationKindV1::RedeemSplit,
                ),
                issued_at_ms: 100,
                expires_at_ms: 10000,
            },
            prepared_one_use_authorization_digest: id(66),
            sealed_transition_inputs: vec![67],
            sealed_recovery_seeds: vec![68],
        })
        .unwrap();
    machine
        .prepare_indexed_outgoing_candidate(
            intent.operation_id,
            intent.context.credential_id,
            candidate,
        )
        .unwrap();
}
fn restored(machine: &Machine) -> Machine {
    let snapshot = machine.snapshot().unwrap();
    let anchor = machine.seal_durability_anchor(vec![69]).unwrap();
    Machine::restore(
        snapshot,
        &anchor,
        machine.proof_release.clone(),
        machine.authenticated_history.clone().into_store(),
        AcceptSnapshotRecursiveVerifierV1,
        AcceptSnapshotGuardVerifierV1,
    )
    .unwrap()
}

#[test]
fn operation_store_exact_reservation_retry_is_stable_and_new_id_is_distinct() {
    let (machine, credential, account) = machine();
    let (_root, path) = location();
    let mut store = machine
        .create_coordinator_operation_store(&path, CAPACITY)
        .unwrap();
    let intent = intent(&machine, credential, account, id(1));
    let binding = binding(&intent);
    assert_eq!(
        machine.reserve_coordinator_operation(&mut store, id(1), 5, &binding),
        Ok(id(1))
    );
    let size = fs::metadata(path.join(FILE)).unwrap().len();
    assert_eq!(
        machine.reserve_coordinator_operation(&mut store, id(1), 5, &binding),
        Ok(id(1))
    );
    assert_eq!(fs::metadata(path.join(FILE)).unwrap().len(), size);
    assert_eq!(
        machine.reserve_coordinator_operation(&mut store, id(1), 1, &binding),
        Err(StoreError::Conflict)
    );
    assert_eq!(
        machine.reserve_coordinator_operation(&mut store, id(1), 5, b"other"),
        Err(StoreError::Conflict)
    );
    assert_eq!(
        machine.reserve_coordinator_operation(&mut store, id(2), 5, &binding),
        Ok(id(2))
    );
    drop(store);
    let mut store = machine.open_coordinator_operation_store(&path, 0).unwrap();
    assert_eq!(
        machine.reserve_coordinator_operation(&mut store, id(1), 5, &binding),
        Ok(id(1))
    );
    assert_eq!(
        machine.reserve_coordinator_operation(&mut store, id(2), 5, &binding),
        Ok(id(2))
    );
    assert_eq!(
        machine.reserve_coordinator_operation(&mut store, id(3), 5, &binding),
        Err(StoreError::Capacity)
    );
}

#[test]
fn operation_store_bounds_and_retired_sender_binding_reject_before_append() {
    let (machine, _, _) = machine();
    let (_root, path) = location();
    let mut store = machine
        .create_coordinator_operation_store(&path, CAPACITY)
        .unwrap();
    let size = fs::metadata(path.join(FILE)).unwrap().len();
    for (operation_id, operation, binding) in [
        (id(0), 1, vec![1]),
        (id(1), 0, vec![1]),
        (id(1), 23, vec![1]),
        (id(1), 1, vec![]),
        (
            id(1),
            1,
            vec![1; KAGEMUSHA_COORDINATOR_PUBLIC_BINDING_MAX_BYTES_V1 + 1],
        ),
        (id(1), 5, b"retired untagged sender inputs".to_vec()),
    ] {
        assert_eq!(
            machine.reserve_coordinator_operation(&mut store, operation_id, operation, &binding),
            Err(StoreError::InvalidBinding)
        );
    }
    assert_eq!(fs::metadata(path.join(FILE)).unwrap().len(), size);
}

#[test]
fn operation_store_validates_nested_send_request_before_reserving_capacity() {
    let (machine, _, _) = machine();
    let (_root, path) = location();
    let mut store = machine
        .create_coordinator_operation_store(&path, CAPACITY)
        .unwrap();
    let request =
        crate::zk::kagemusha_v1_recursion::tests::incoming_payment_fixture(1, 2, 3, 5, 32, 32)
            .request;
    let canonical_request = norito::encode_canonical(&request).unwrap();
    let mut zero_amount = request.clone();
    zero_amount.amount = 0;
    let mut trailing = canonical_request.clone();
    trailing.push(0);
    let size = fs::metadata(path.join(FILE)).unwrap().len();
    for invalid_request in [
        b"noncanonical nested request".to_vec(),
        canonical_request[..canonical_request.len() - 1].to_vec(),
        trailing,
        norito::encode_canonical(&zero_amount).unwrap(),
    ] {
        let inputs = KagemushaOutgoingPublicInputsV1::SendSplit {
            request: invalid_request,
        };
        let binding = norito::encode_canonical(&inputs).unwrap();
        assert_eq!(
            machine.reserve_coordinator_operation(&mut store, id(1), 5, &binding),
            Err(StoreError::InvalidBinding)
        );
        assert_eq!(fs::metadata(path.join(FILE)).unwrap().len(), size);
    }
    let inputs = KagemushaOutgoingPublicInputsV1::SendSplit {
        request: canonical_request,
    };
    let binding = norito::encode_canonical(&inputs).unwrap();
    assert_eq!(
        machine.reserve_coordinator_operation(&mut store, id(1), 5, &binding),
        Ok(id(1))
    );
    drop(store);
    let mut store = machine.open_coordinator_operation_store(&path, 0).unwrap();
    assert_eq!(
        machine.reserve_coordinator_operation(&mut store, id(1), 5, &binding),
        Ok(id(1))
    );
}

#[test]
fn operation_store_cross_sdk_sender_reservations_match_canonical_core_types() {
    use norito::codec::Encode as _;

    let fixture: norito::json::Value = norito::json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/offline/kagemusha_sender_reservation_v1.json"
    )))
    .unwrap();
    let string = |name: &str| fixture.get(name).unwrap().as_str().unwrap();
    let bytes = |name: &str| hex::decode(string(name)).unwrap();
    let send_binding = bytes("send_binding_hex");
    let redeem_binding = bytes("redeem_binding_hex");
    let send: KagemushaOutgoingPublicInputsV1 = norito::decode_canonical(&send_binding).unwrap();
    assert_eq!(
        send,
        KagemushaOutgoingPublicInputsV1::SendSplit {
            request: bytes("send_request_hex"),
        }
    );
    send.decode_send_parts().unwrap();
    assert_eq!(norito::encode_canonical(&send).unwrap(), send_binding);
    let redeem: KagemushaOutgoingPublicInputsV1 =
        norito::decode_canonical(&redeem_binding).unwrap();
    let KagemushaOutgoingPublicInputsV1::RedeemSplit {
        amount,
        beneficiary,
    } = &redeem
    else {
        panic!("shared fixture must use the Core redemption variant")
    };
    assert_eq!(
        *amount,
        string("redeem_amount_decimal").parse::<u128>().unwrap()
    );
    {
        let _canonical =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        assert_eq!(
            beneficiary.encode(),
            bytes("redeem_beneficiary_payload_hex")
        );
    }
    assert_eq!(norito::encode_canonical(&redeem).unwrap(), redeem_binding);

    let (machine, _, _) = machine();
    let (_root, path) = location();
    let mut store = machine
        .create_coordinator_operation_store(&path, CAPACITY)
        .unwrap();
    for (operation_id, binding) in [(id(1), &send_binding), (id(2), &redeem_binding)] {
        assert_eq!(
            machine.reserve_coordinator_operation(&mut store, operation_id, 5, binding),
            Ok(operation_id)
        );
    }
    drop(store);
    let mut store = machine.open_coordinator_operation_store(&path, 0).unwrap();
    for (operation_id, binding) in [(id(1), &send_binding), (id(2), &redeem_binding)] {
        assert_eq!(
            machine.reserve_coordinator_operation(&mut store, operation_id, 5, binding),
            Ok(operation_id)
        );
    }
}

#[test]
fn operation_store_intent_crash_recovery_never_becomes_prepared_or_absent() {
    let (machine, credential, account) = machine();
    let (_root, path) = location();
    let mut store = machine
        .create_coordinator_operation_store(&path, CAPACITY)
        .unwrap();
    let intent = intent(&machine, credential, account, id(3));
    machine
        .reserve_coordinator_operation(&mut store, id(3), 5, &binding(&intent))
        .unwrap();
    assert_eq!(
        machine.recover_coordinator_sender_intent(&store, id(3)),
        Ok(Recovery::Reserved)
    );
    machine
        .begin_coordinator_sender_intent(&mut store, &intent)
        .unwrap();
    let size = fs::metadata(path.join(FILE)).unwrap().len();
    machine
        .begin_coordinator_sender_intent(&mut store, &intent)
        .unwrap();
    assert_eq!(fs::metadata(path.join(FILE)).unwrap().len(), size);
    assert!(machine.outgoing_operation_index().is_empty());
    drop(store);
    let restored = restored(&machine);
    let mut store = restored.open_coordinator_operation_store(&path, 0).unwrap();
    assert_eq!(
        restored.recover_coordinator_sender_intent(&store, id(3)),
        Ok(Recovery::Intent(intent.clone()))
    );
    assert_eq!(
        restored.recover_coordinator_sender_intent(&store, id(4)),
        Err(StoreError::Conflict)
    );
    let mut conflict = intent.clone();
    conflict.context.credential_id = id(99);
    assert_eq!(
        restored.begin_coordinator_sender_intent(&mut store, &conflict),
        Err(StoreError::Conflict)
    );
}

#[test]
fn operation_store_reconciles_actual_prepared_core_index_across_restore() {
    let (mut machine, credential, account) = machine();
    let (_root, path) = location();
    let mut store = machine
        .create_coordinator_operation_store(&path, CAPACITY)
        .unwrap();
    let intent = intent(&machine, credential, account, id(5));
    machine
        .reserve_coordinator_operation(&mut store, id(5), 5, &binding(&intent))
        .unwrap();
    machine
        .begin_coordinator_sender_intent(&mut store, &intent)
        .unwrap();
    prepare(&mut machine, &intent);
    let record = machine
        .outgoing_operation_index()
        .lookup(id(5))
        .unwrap()
        .clone();
    assert_eq!(record.phase, KagemushaOutgoingOperationPhaseV1::Prepared);
    drop(store);
    let restored = restored(&machine);
    let mut store = restored
        .open_coordinator_operation_store(&path, CAPACITY)
        .unwrap();
    assert_eq!(
        restored.recover_coordinator_sender_intent(&store, id(5)),
        Ok(Recovery::Indexed(record))
    );
    restored
        .begin_coordinator_sender_intent(&mut store, &intent)
        .unwrap();
}

#[test]
fn operation_store_old_prefix_cannot_hide_unrelated_core_operation() {
    let (mut machine, credential, account) = machine();
    let (_root, path) = location();
    let mut store = machine
        .create_coordinator_operation_store(&path, CAPACITY)
        .unwrap();
    let old_prefix = fs::read(path.join(FILE)).unwrap();
    let intent = intent(&machine, credential, account, id(6));
    machine
        .reserve_coordinator_operation(&mut store, id(6), 5, &binding(&intent))
        .unwrap();
    machine
        .begin_coordinator_sender_intent(&mut store, &intent)
        .unwrap();
    prepare(&mut machine, &intent);
    drop(store);
    fs::write(path.join(FILE), old_prefix).unwrap();
    let restored = restored(&machine);
    assert!(matches!(
        restored.open_coordinator_operation_store(&path, CAPACITY),
        Err(StoreError::CoreMismatch)
    ));
    let (_other_root, other) = location();
    assert!(matches!(
        restored.create_coordinator_operation_store(&other, CAPACITY),
        Err(StoreError::CoreMismatch)
    ));
}

#[test]
fn operation_store_must_reconcile_again_when_core_advances_after_open() {
    let (mut machine, credential, account) = machine();
    let (_root, path) = location();
    let mut store = machine
        .create_coordinator_operation_store(&path, CAPACITY)
        .unwrap();
    let missing = intent(&machine, credential, account, id(7));
    prepare(&mut machine, &missing);
    assert_eq!(
        machine.reserve_coordinator_operation(&mut store, id(8), 1, b"read"),
        Err(StoreError::CoreMismatch)
    );
    assert_eq!(
        machine.begin_coordinator_sender_intent(&mut store, &missing),
        Err(StoreError::CoreMismatch)
    );
    assert_eq!(
        machine.recover_coordinator_sender_intent(&store, id(8)),
        Err(StoreError::CoreMismatch)
    );
}

#[test]
fn operation_store_foreign_wallet_context_and_changed_epoch_fail_closed() {
    let (machine, credential, account) = machine();
    let (_root, path) = location();
    let mut store = machine
        .create_coordinator_operation_store(&path, CAPACITY)
        .unwrap();
    let mut wrong = intent(&machine, credential, account, id(9));
    machine
        .reserve_coordinator_operation(&mut store, id(9), 5, &binding(&wrong))
        .unwrap();
    wrong.context.hardware_epoch.epoch_id = id(99);
    assert_eq!(
        machine.begin_coordinator_sender_intent(&mut store, &wrong),
        Err(StoreError::InvalidBinding)
    );
    let mut foreign = restored(&machine);
    foreign.state.lane.device_lane_id = id(98);
    assert_eq!(
        foreign.reserve_coordinator_operation(&mut store, id(10), 1, b"read"),
        Err(StoreError::CoreMismatch)
    );
}

#[test]
fn operation_store_process_child() {
    let Some(path) = std::env::var_os("KAGEMUSHA_OPERATION_STORE_TEST_PATH") else {
        return;
    };
    let (machine, credential, account) = machine();
    let path = Path::new(&path);
    let mut store = machine
        .create_coordinator_operation_store(path, CAPACITY)
        .unwrap();
    let intent = intent(&machine, credential, account, id(11));
    if std::env::var_os("KAGEMUSHA_OPERATION_STORE_TEST_LOST_REPLY").is_some() {
        store
            .wal
            .failure
            .set(Some(TestPersistenceFailure::AfterSync));
        assert_eq!(
            machine.reserve_coordinator_operation(&mut store, id(11), 5, &binding(&intent)),
            Err(StoreError::DurabilityUncertain)
        );
    } else {
        machine
            .reserve_coordinator_operation(&mut store, id(11), 5, &binding(&intent))
            .unwrap();
        machine
            .begin_coordinator_sender_intent(&mut store, &intent)
            .unwrap();
    }
    // Real process exit without Drop simulates losing the host reply after durable admission.
    std::process::exit(0);
}

#[test]
fn operation_store_process_restart_recovers_intent_and_lost_reservation_reply() {
    for lost in [false, true] {
        let (_root, path) = location();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args(["--exact", "zk::kagemusha_v1_state::tests::coordinator_operation_store_tests::operation_store_process_child", "--nocapture"])
            .env("KAGEMUSHA_OPERATION_STORE_TEST_PATH", &path);
        if lost {
            command.env("KAGEMUSHA_OPERATION_STORE_TEST_LOST_REPLY", "1");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let (machine, credential, account) = machine();
        let intent = intent(&machine, credential, account, id(11));
        let mut store = machine.open_coordinator_operation_store(&path, 0).unwrap();
        let size = fs::metadata(path.join(FILE)).unwrap().len();
        assert_eq!(
            machine.reserve_coordinator_operation(&mut store, id(11), 5, &binding(&intent)),
            Ok(id(11))
        );
        assert_eq!(fs::metadata(path.join(FILE)).unwrap().len(), size);
        let expected = if lost {
            Recovery::Reserved
        } else {
            Recovery::Intent(intent)
        };
        assert_eq!(
            machine.recover_coordinator_sender_intent(&store, id(11)),
            Ok(expected)
        );
    }
}

#[test]
fn operation_store_write_uncertainty_poison_never_acknowledges() {
    for failure in [
        TestPersistenceFailure::PartialWrite,
        TestPersistenceFailure::BeforeSync,
        TestPersistenceFailure::AfterSync,
        TestPersistenceFailure::ReplaceAfterSync,
        TestPersistenceFailure::TruncateAfterSync,
    ] {
        let (machine, _, _) = machine();
        let (_root, path) = location();
        let mut store = machine
            .create_coordinator_operation_store(&path, CAPACITY)
            .unwrap();
        store.wal.failure.set(Some(failure));
        assert_eq!(
            machine.reserve_coordinator_operation(&mut store, id(12), 1, b"exact read"),
            Err(StoreError::DurabilityUncertain)
        );
        assert_eq!(
            machine.reserve_coordinator_operation(&mut store, id(12), 1, b"exact read"),
            Err(StoreError::DurabilityUncertain)
        );
    }
}

#[test]
fn operation_store_same_length_tamper_and_concurrent_writer_reject() {
    let (machine, _, _) = machine();
    let (_root, path) = location();
    let mut store = machine
        .create_coordinator_operation_store(&path, CAPACITY)
        .unwrap();
    assert!(matches!(
        machine.open_coordinator_operation_store(&path, CAPACITY),
        Err(StoreError::AlreadyOpen)
    ));
    let file = path.join(FILE);
    let mut bytes = fs::read(&file).unwrap();
    bytes[90] ^= 1;
    fs::write(&file, &bytes).unwrap();
    assert_eq!(
        machine.reserve_coordinator_operation(&mut store, id(13), 1, b"read"),
        Err(StoreError::JournalCorrupt)
    );
    drop(store);
    assert!(matches!(
        machine.open_coordinator_operation_store(&path, CAPACITY),
        Err(StoreError::JournalCorrupt)
    ));
}

#[test]
fn operation_store_corrupt_empty_partial_and_replaced_frames_never_reset() {
    let (machine, _, _) = machine();
    let (_root, path) = location();
    let store = machine
        .create_coordinator_operation_store(&path, CAPACITY)
        .unwrap();
    drop(store);
    let file = path.join(FILE);
    let bytes = fs::read(&file).unwrap();
    for length in [0, 1, 87, bytes.len() - 1] {
        fs::write(&file, &bytes[..length]).unwrap();
        assert!(
            machine
                .open_coordinator_operation_store(&path, CAPACITY)
                .is_err()
        );
        assert_eq!(fs::read(&file).unwrap(), bytes[..length]);
    }
    fs::write(&file, &bytes).unwrap();
    let (_other_root, other) = location();
    assert!(
        machine
            .open_coordinator_operation_store(&other, CAPACITY)
            .is_err()
    );
    assert!(!other.exists());
}
