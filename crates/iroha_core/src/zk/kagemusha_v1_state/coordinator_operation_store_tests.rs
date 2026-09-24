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

pub(in super::super) fn machine() -> (Machine, DigestV1, AccountId) {
    machine_for_payment_scope(None)
}

fn machine_for_payment_scope(
    payment_context: Option<iroha_data_model::kagemusha::KagemushaPaymentRequestV1>,
) -> (Machine, DigestV1, AccountId) {
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
    let payment_context = payment_context.unwrap_or_else(|| {
        crate::zk::kagemusha_v1_recursion::tests::incoming_payment_fixture(1, 2, 3, 5, 32, 32)
            .request
    });
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
    // Operation-store pairing now checks the actual selected prefix, so the structural
    // machine fixture derives its initializer from a real private WAL instead of a fake head.
    let (_initial_root, initial_path) = location();
    let initial_store = KagemushaCoordinatorOperationStoreV1::create_new(
        &initial_path,
        state.lane.clone(),
        state.asset_incarnation,
        CAPACITY,
    )
    .unwrap();
    let mut recovery_metadata =
        snapshot_initial_metadata(&state, &proof_release, old_credential.clone());
    recovery_metadata.journals.coordinator = initial_store.recovery_prefix().unwrap();
    drop(initial_store);
    let machine = KagemushaStateMachineV1 {
        recovery_metadata,
        published_checkpoint: None,
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
    let machine = snapshot_initial_publish(machine);

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
            core_authorization_key_reference: id(70),
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
fn candidate(
    machine: &Machine,
    intent: &KagemushaOutgoingPublicInputPreimageV1,
) -> PreparedOutgoingCandidateV1 {
    let KagemushaOutgoingPublicInputsV1::RedeemSplit {
        amount,
        beneficiary,
    } = &intent.inputs
    else {
        panic!("test redemption")
    };
    machine
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
        .unwrap()
}

#[test]
fn committed_redemption_exports_exact_native_terminal_opening_messages() {
    use sha2::{Digest as _, Sha256};

    let (machine, credential, beneficiary) = machine();
    let intent = intent(&machine, credential, beneficiary, id(90));
    let prepared = candidate(&machine, &intent);
    let artifacts = machine.proof_release.artifacts;
    // This accepting snapshot verifier tests Core's retained preimages, not proof validity.
    // A genuine paired redemption proof belongs in the funded State/Terminal corridor.
    let proof = snapshot_paired_proof(
        prepared.semantic_digest().unwrap(),
        artifacts.eq_protocol_digest,
        artifacts.ep_protocol_digest,
        91,
    );
    let persisted = PersistedOutgoingCandidateV1::verify_and_persist_redemption(
        prepared.clone(),
        proof,
        artifacts,
        &AcceptSnapshotRecursiveVerifierV1,
    )
    .expect("verify the native redemption candidate before commitment");
    let body = persisted.hardware_terminal_body().unwrap();
    let certificate = KagemushaCommitCertificateV1 {
        version: body.version,
        certificate_id: [0; 32],
        candidate_envelope_digest: body.candidate_envelope_digest,
        lifecycle_binding_digest: body.lifecycle_binding_digest,
        transition_nullifier: body.transition_nullifier,
        outbox_reservation_commitment: body.outbox_reservation_commitment,
        commit_evidence: body.commit_evidence,
        hardware_profile_id: body.hardware_profile_id,
        policy_epoch: body.policy_epoch,
        hardware_terminal_commitment: [0; 32],
    }
    .seal_with_terminal_body(&body)
    .unwrap();
    let committed = CommittedOutgoingCandidateV1::from_hardware_commit(persisted, certificate)
        .expect("native certificate matches the retained redemption candidate");
    let messages = committed
        .canonical_outgoing_opening_sha_messages_v1()
        .expect("export exact native redemption SHA preimages");
    let digest = |message: &[u8]| -> DigestV1 { Sha256::digest(message).into() };
    let carriers = prepared.prepared_intent_commitments();
    let operation_offset = b"iroha:kagemusha:v1:outgoing-preparation\0".len() + 2;
    assert_eq!(messages[2][operation_offset], 4);
    assert_eq!(
        digest(&messages[0]),
        carriers.sealed_transition_inputs_digest
    );
    assert_eq!(digest(&messages[1]), carriers.sealed_recovery_seeds_digest);
    assert_eq!(digest(&messages[2]), carriers.preparation_id);
    assert_eq!(digest(&messages[3]), body.private_journal_commitment);
    assert_eq!(digest(&messages[4]), body.private_recovery_commitment);
    assert_eq!(
        digest(&messages[5]),
        committed.commit_certificate.hardware_terminal_commitment
    );
    let mut changed = committed;
    changed.candidate.prepared.sealed_recovery_seeds[0] ^= 1;
    assert!(
        changed
            .canonical_outgoing_opening_sha_messages_v1()
            .is_err()
    );
}

fn prepare(machine: &mut Machine, intent: &KagemushaOutgoingPublicInputPreimageV1) {
    let candidate = candidate(machine, intent);
    machine
        .prepare_indexed_outgoing_candidate(
            intent.operation_id,
            intent.context.credential_id,
            intent.context.core_authorization_key_reference,
            candidate,
        )
        .unwrap();
}

#[test]
fn outgoing_state_proof_archive_export_uses_only_retained_core_pair() {
    // The accepting snapshot verifier and structural terminal proof test retained-byte identity
    // across Core stages; they do not establish cryptographic validity or monetary admission.
    let (mut machine, credential, beneficiary) = machine();
    let operation_id = id(93);
    let intent = intent(&machine, credential, beneficiary, operation_id);
    let prepared = candidate(&machine, &intent);
    let artifacts = machine.proof_release.artifacts;
    let proof = snapshot_paired_proof(
        prepared.semantic_digest().unwrap(),
        artifacts.eq_protocol_digest,
        artifacts.ep_protocol_digest,
        94,
    );
    let expected_public = prepared.candidate_public_inputs(artifacts, &proof).unwrap();
    assert_eq!(
        machine.export_outgoing_state_proof_archives(operation_id),
        Err(KagemushaStateErrorV1::InvalidCandidateStage)
    );
    let (_, _, capability) = machine
        .prepare_indexed_outgoing_candidate(
            operation_id,
            credential,
            intent.context.core_authorization_key_reference,
            prepared,
        )
        .unwrap();
    assert_eq!(
        machine.export_outgoing_state_proof_archives(operation_id),
        Err(KagemushaStateErrorV1::InvalidCandidateStage)
    );
    machine
        .persist_outgoing_redemption_candidate(&capability, proof.clone())
        .unwrap();
    let archives = machine
        .export_outgoing_state_proof_archives(operation_id)
        .unwrap();
    assert_eq!(archives.operation_id, operation_id);
    assert_eq!(
        archives.public_inputs_archive,
        norito::encode_canonical(&expected_public).unwrap()
    );
    assert_eq!(
        archives.paired_proof_archive,
        norito::encode_canonical(&proof).unwrap()
    );
    assert!(
        archives.public_inputs_archive.len()
            <= KAGEMUSHA_OUTGOING_STATE_PUBLIC_INPUT_ARCHIVE_MAX_BYTES_V1
    );
    assert!(archives.paired_proof_archive.len() <= KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1);
    assert_eq!(
        machine.export_outgoing_state_proof_archives(id(95)),
        Err(KagemushaStateErrorV1::InvalidCandidateStage)
    );
    let mut recovered = restored(machine);
    assert_eq!(
        recovered
            .export_outgoing_state_proof_archives(operation_id)
            .unwrap(),
        archives,
        "authenticated snapshot recovery preserves the original observer bytes"
    );
    let original_lane_id = recovered.state.lane.device_lane_id;
    recovered.state.lane.device_lane_id = id(96);
    assert_eq!(
        recovered.export_outgoing_state_proof_archives(operation_id),
        Err(KagemushaStateErrorV1::SnapshotIntegrity)
    );
    recovered.state.lane.device_lane_id = original_lane_id;

    let candidate = match recovered.outgoing_candidate_journal.stage() {
        KagemushaOutgoingJournalStageV1::Candidate(candidate) => candidate,
        _ => panic!("restored Core must retain its verified candidate"),
    };
    let body = candidate.hardware_terminal_body().unwrap();
    let certificate = KagemushaCommitCertificateV1 {
        version: body.version,
        certificate_id: [0; 32],
        candidate_envelope_digest: body.candidate_envelope_digest,
        lifecycle_binding_digest: body.lifecycle_binding_digest,
        transition_nullifier: body.transition_nullifier,
        outbox_reservation_commitment: body.outbox_reservation_commitment,
        commit_evidence: body.commit_evidence,
        hardware_profile_id: body.hardware_profile_id,
        policy_epoch: body.policy_epoch,
        hardware_terminal_commitment: [0; 32],
    }
    .seal_with_terminal_body(&body)
    .unwrap();
    let recovered_capability = recovered
        .recover_indexed_outgoing_commit_capability(operation_id)
        .unwrap();
    let committed = recovered
        .commit_outgoing_candidate(recovered_capability, certificate)
        .unwrap();
    assert_eq!(
        recovered
            .export_outgoing_state_proof_archives(operation_id)
            .unwrap(),
        archives,
        "hardware commit cannot substitute the earlier State proof pair"
    );

    let output = committed.public_output().unwrap();
    let terminal_proof = KagemushaRedemptionProofV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        eq_protocol_digest: artifacts.commit_wrapper_eq_protocol_digest,
        ep_protocol_digest: artifacts.commit_wrapper_ep_protocol_digest,
        semantic_digest: output.semantic_digest,
        candidate_envelope_digest: output.candidate_envelope_digest,
        commit_certificate_digest: output.commit_certificate_digest,
        eq_deferred_audit: artifacts.commit_wrapper_eq_protocol_digest,
        ep_deferred_audit: artifacts.commit_wrapper_ep_protocol_digest,
        eq_proof: vec![98; 32],
        ep_proof: vec![99; 32],
        eq_history: proof.eq_history,
        ep_history: proof.ep_history,
    };
    recovered
        .finalize_outgoing_redemption(terminal_proof, Vec::new())
        .unwrap();
    assert_eq!(
        recovered
            .export_outgoing_state_proof_archives(operation_id)
            .unwrap(),
        archives,
        "installed envelope keeps the original observer pair until release"
    );
    recovered.state.logical_sequence -= 1;
    assert_eq!(
        recovered.export_outgoing_state_proof_archives(operation_id),
        Err(KagemushaStateErrorV1::SnapshotIntegrity),
        "an installed candidate ahead of the current State is stale"
    );
}

#[test]
fn released_sender_index_cannot_export_prepared_state_proof() {
    let (mut machine, credential, beneficiary) = machine();
    let operation_id = id(97);
    let intent = intent(&machine, credential, beneficiary, operation_id);
    prepare(&mut machine, &intent);
    release_index_fixture(&mut machine, operation_id);
    assert_eq!(
        machine.export_outgoing_state_proof_archives(operation_id),
        Err(KagemushaStateErrorV1::InvalidCandidateStage)
    );
}

fn restored(machine: Machine) -> Machine {
    let machine = snapshot_publish_checkpoint(machine);
    let anchor = machine.recovery_checkpoint().clone();
    let snapshot = machine.snapshot().unwrap();
    Machine::restore(
        snapshot,
        &anchor,
        machine.proof_release.clone(),
        machine.proof_release.clone(),
        machine.enrollment_binding(),
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
        machine.reserve_coordinator_operation(&mut store, id(1), 4, &binding),
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
        (id(0), 4, vec![1]),
        (id(1), 0, vec![1]),
        (id(1), 23, vec![1]),
        (id(1), 4, vec![]),
        (
            id(1),
            4,
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

    let shared: norito::json::Value = norito::json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/offline/kagemusha_v1.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let canonical_request =
        hex::decode(shared["payment_request"]["norito_hex"].as_str().unwrap()).unwrap();

    let fixture: norito::json::Value = norito::json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/offline/kagemusha_sender_reservation_v1.json"
    )))
    .unwrap();
    let string = |name: &str| fixture.get(name).unwrap().as_str().unwrap();
    let bytes = |name: &str| hex::decode(string(name)).unwrap();
    assert_eq!(bytes("send_request_hex"), canonical_request);
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

    // Build the test Core wallet and credential in the shared request's exact public scope.
    let (machine, _, _) = machine_for_payment_scope(Some(send.decode_send_parts().unwrap()));
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
    let restored = restored(machine);
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
fn operation_store_authorization_key_substitution_conflicts_across_reopen() {
    let (mut machine, credential, account) = machine();
    let (_root, path) = location();
    let mut store = machine
        .create_coordinator_operation_store(&path, CAPACITY)
        .unwrap();
    let intent = intent(&machine, credential, account, id(71));
    machine
        .reserve_coordinator_operation(&mut store, intent.operation_id, 5, &binding(&intent))
        .unwrap();
    let mut substituted = intent.clone();
    substituted.context.core_authorization_key_reference = [0; 32];
    assert_eq!(
        machine.begin_coordinator_sender_intent(&mut store, &substituted),
        Err(StoreError::InvalidBinding),
    );
    machine
        .begin_coordinator_sender_intent(&mut store, &intent)
        .unwrap();
    substituted.context.core_authorization_key_reference = id(72);
    assert_ne!(
        intent.canonical_digest().unwrap(),
        substituted.canonical_digest().unwrap(),
    );
    assert_eq!(
        machine.begin_coordinator_sender_intent(&mut store, &substituted),
        Err(StoreError::Conflict),
    );
    let prepared = candidate(&machine, &intent);
    assert!(
        machine
            .prepare_indexed_outgoing_candidate(
                intent.operation_id,
                credential,
                [0; 32],
                prepared.clone(),
            )
            .is_err()
    );
    assert!(machine.outgoing_operation_index().is_empty());
    machine
        .prepare_indexed_outgoing_candidate(
            intent.operation_id,
            credential,
            intent.context.core_authorization_key_reference,
            prepared.clone(),
        )
        .unwrap();
    assert!(
        machine
            .prepare_indexed_outgoing_candidate(
                intent.operation_id,
                credential,
                substituted.context.core_authorization_key_reference,
                prepared,
            )
            .is_err()
    );
    drop(store);
    let restored = restored(machine);
    let mut store = restored.open_coordinator_operation_store(&path, 0).unwrap();
    restored
        .begin_coordinator_sender_intent(&mut store, &intent)
        .unwrap();
    assert_eq!(
        restored.begin_coordinator_sender_intent(&mut store, &substituted),
        Err(StoreError::Conflict),
    );
    let Recovery::Indexed(record) = restored
        .recover_coordinator_sender_intent(&store, intent.operation_id)
        .unwrap()
    else {
        panic!("restored Core preparation must retain its exact index record")
    };
    assert_eq!(record.context, intent.context);
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
    let restored = restored(machine);
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
    let restored = restored(machine);
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
        machine.reserve_coordinator_operation(&mut store, id(8), 4, b"durable command"),
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
    let mut foreign = restored(machine);
    foreign.state.lane.device_lane_id = id(98);
    assert_eq!(
        foreign.reserve_coordinator_operation(&mut store, id(10), 4, b"durable command"),
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
            machine.reserve_coordinator_operation(&mut store, id(12), 4, b"exact durable command"),
            Err(StoreError::DurabilityUncertain)
        );
        assert_eq!(
            machine.reserve_coordinator_operation(&mut store, id(12), 4, b"exact durable command"),
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
        machine.reserve_coordinator_operation(&mut store, id(13), 4, b"durable command"),
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

// This fixture advances only the already Core-owned public index so journal accounting can be
// tested without generating a recursive monetary proof. It is test-only state, not a provider
// completion or hardware qualification. Receipt verification has its own Core release tests.
fn release_index_fixture(machine: &mut Machine, operation_id: DigestV1) {
    let index = machine
        .outgoing_candidate_journal
        .operation_index_mut_for_test();
    let record = index.records.get_mut(&operation_id).unwrap();
    assert_eq!(record.phase, KagemushaOutgoingOperationPhaseV1::Prepared);
    record.phase = KagemushaOutgoingOperationPhaseV1::Installed;
    record.candidate_digest = Some(id(81));
    record.commit_certificate_digest = Some(id(82));
    record.envelope_digest = Some(id(83));
    record.record_revision = 4;
    index.revision = 4;
    let reservation = record.outbox_reservation_id;
    *index = index
        .release_successor(reservation, id(83), id(84))
        .unwrap();
}

#[test]
fn operation_store_retires_only_core_released_sender_capacity_and_keeps_exact_tombstone() {
    let (mut machine, credential, account) = machine();
    let (_root, path) = location();
    let mut store = machine
        .create_coordinator_operation_store(&path, CAPACITY)
        .unwrap();
    let first = intent(&machine, credential, account.clone(), id(85));
    let second = intent(&machine, credential, account, id(86));
    machine
        .reserve_coordinator_operation(&mut store, first.operation_id, 5, &binding(&first))
        .unwrap();
    let charge = store.live_reserved_bytes();
    assert!(charge > KAGEMUSHA_COORDINATOR_INTENT_MAX_BYTES_V1 as u64);
    machine
        .begin_coordinator_sender_intent(&mut store, &first)
        .unwrap();
    prepare(&mut machine, &first);
    let prepared_journal = machine.outgoing_candidate_journal.clone();
    drop(store);
    let mut store = machine
        .open_coordinator_operation_store(&path, charge)
        .unwrap();
    assert_eq!(
        machine.retire_released_coordinator_sender_operations(&mut store),
        Ok(0)
    );
    assert_eq!(store.live_reserved_bytes(), charge);
    assert_eq!(
        machine.reserve_coordinator_operation(
            &mut store,
            second.operation_id,
            5,
            &binding(&second)
        ),
        Err(StoreError::Capacity),
    );
    release_index_fixture(&mut machine, first.operation_id);
    assert_eq!(machine.outgoing_operation_index().reserved_bytes(), 0);
    assert_eq!(
        machine.retire_released_coordinator_sender_operations(&mut store),
        Ok(1)
    );
    assert_eq!(store.live_reserved_bytes(), 0);
    let size = fs::metadata(path.join(FILE)).unwrap().len();
    assert_eq!(
        machine.retire_released_coordinator_sender_operations(&mut store),
        Ok(0)
    );
    machine
        .begin_coordinator_sender_intent(&mut store, &first)
        .unwrap();
    assert_eq!(fs::metadata(path.join(FILE)).unwrap().len(), size);
    machine
        .reserve_coordinator_operation(&mut store, second.operation_id, 5, &binding(&second))
        .unwrap();
    assert_eq!(store.live_reserved_bytes(), charge);
    drop(store);
    let mut store = machine
        .open_coordinator_operation_store(&path, charge)
        .unwrap();
    assert_eq!(store.live_reserved_bytes(), charge);
    assert_eq!(
        machine.reserve_coordinator_operation(&mut store, first.operation_id, 5, &binding(&first)),
        Ok(first.operation_id),
    );
    assert_eq!(
        machine.reserve_coordinator_operation(&mut store, first.operation_id, 5, b"changed"),
        Err(StoreError::Conflict),
    );
    let Recovery::Indexed(record) = machine
        .recover_coordinator_sender_intent(&store, first.operation_id)
        .unwrap()
    else {
        panic!("terminal history must remain Indexed, never Missing or a fresh Intent");
    };
    assert_eq!(record.phase, KagemushaOutgoingOperationPhaseV1::Released);
    drop(store);
    machine.outgoing_candidate_journal = prepared_journal;
    assert!(matches!(
        machine.open_coordinator_operation_store(&path, charge),
        Err(StoreError::CoreMismatch)
    ));
}

#[test]
fn operation_store_lost_retirement_reply_reopens_exactly_without_double_credit() {
    let (mut machine, credential, account) = machine();
    let (_root, path) = location();
    let mut store = machine
        .create_coordinator_operation_store(&path, CAPACITY)
        .unwrap();
    let first = intent(&machine, credential, account.clone(), id(87));
    machine
        .reserve_coordinator_operation(&mut store, first.operation_id, 5, &binding(&first))
        .unwrap();
    let charge = store.live_reserved_bytes();
    machine
        .begin_coordinator_sender_intent(&mut store, &first)
        .unwrap();
    prepare(&mut machine, &first);
    release_index_fixture(&mut machine, first.operation_id);
    store
        .wal
        .failure
        .set(Some(TestPersistenceFailure::AfterSync));
    assert_eq!(
        machine.retire_released_coordinator_sender_operations(&mut store),
        Err(StoreError::DurabilityUncertain)
    );
    assert_eq!(
        store.live_reserved_bytes(),
        charge,
        "the uncertain writer cannot reuse capacity"
    );
    assert_eq!(
        machine.retire_released_coordinator_sender_operations(&mut store),
        Err(StoreError::DurabilityUncertain)
    );
    let length = fs::metadata(path.join(FILE)).unwrap().len();
    drop(store);
    let mut store = machine
        .open_coordinator_operation_store(&path, charge)
        .unwrap();
    assert_eq!(store.live_reserved_bytes(), 0);
    assert_eq!(fs::metadata(path.join(FILE)).unwrap().len(), length);
    let second = intent(&machine, credential, account, id(88));
    machine
        .reserve_coordinator_operation(&mut store, second.operation_id, 5, &binding(&second))
        .unwrap();
    assert_eq!(store.live_reserved_bytes(), charge);
}

#[test]
fn operation_store_core_released_journal_prefix_is_retired_but_changed_receipt_is_rejected() {
    let (mut machine, credential, account) = machine();
    let (_root, path) = location();
    let mut store = machine
        .create_coordinator_operation_store(&path, CAPACITY)
        .unwrap();
    let first = intent(&machine, credential, account, id(89));
    machine
        .reserve_coordinator_operation(&mut store, first.operation_id, 5, &binding(&first))
        .unwrap();
    machine
        .begin_coordinator_sender_intent(&mut store, &first)
        .unwrap();
    prepare(&mut machine, &first);
    let before_release = fs::read(path.join(FILE)).unwrap();
    release_index_fixture(&mut machine, first.operation_id);
    machine
        .retire_released_coordinator_sender_operations(&mut store)
        .unwrap();
    drop(store);
    fs::write(path.join(FILE), before_release).unwrap();
    let store = machine.open_coordinator_operation_store(&path, 0).unwrap();
    assert_eq!(store.live_reserved_bytes(), 0);
    drop(store);
    machine
        .outgoing_candidate_journal
        .operation_index_mut_for_test()
        .records
        .get_mut(&first.operation_id)
        .unwrap()
        .terminal_receipt_digest = Some(id(90));
    assert!(matches!(
        machine.open_coordinator_operation_store(&path, 0),
        Err(StoreError::CoreMismatch)
    ));
}

#[test]
fn operation_store_observations_never_append_or_consume_durable_capacity() {
    let (machine, _, _) = machine();
    let (_root, path) = location();
    let mut store = machine
        .create_coordinator_operation_store(&path, 0)
        .unwrap();
    let initialized = fs::read(path.join(FILE)).unwrap();
    for tag in 1..=96 {
        for operation in [1, 13, 18, 21] {
            assert_eq!(
                machine.reserve_coordinator_operation(&mut store, id(tag), operation, b"read body"),
                Err(StoreError::InvalidBinding)
            );
        }
    }
    assert_eq!(store.live_reserved_bytes(), 0);
    assert_eq!(fs::read(path.join(FILE)).unwrap(), initialized);
    drop(store);
    let restored = machine.open_coordinator_operation_store(&path, 0).unwrap();
    assert_eq!(restored.live_reserved_bytes(), 0);
    assert_eq!(fs::read(path.join(FILE)).unwrap(), initialized);
}

#[test]
fn operation_store_asset_incarnation_is_part_of_immutable_wallet_scope() {
    let (mut machine, _, _) = machine();
    let (_root, path) = location();
    let mut store = machine
        .create_coordinator_operation_store(&path, CAPACITY)
        .unwrap();
    machine
        .reserve_coordinator_operation(&mut store, id(93), 4, b"exact durable command")
        .unwrap();
    machine.state.asset_incarnation =
        iroha_data_model::nexus::AxtAssetIncarnationV1::try_from_bytes(
            *iroha_crypto::Hash::new(b"a different issued asset incarnation").as_ref(),
        )
        .unwrap();
    assert_eq!(
        machine.reserve_coordinator_operation(&mut store, id(94), 4, b"exact durable command"),
        Err(StoreError::CoreMismatch)
    );
    drop(store);
    assert!(
        machine
            .open_coordinator_operation_store(&path, CAPACITY)
            .is_err()
    );
}

#[test]
fn operation_store_checkpoint_prefix_tracks_exact_durable_frames_and_rejects_replaced_bytes() {
    let (machine, _, _) = machine();
    let (_root, path) = location();
    let mut store = machine
        .create_coordinator_operation_store(&path, CAPACITY)
        .unwrap();
    let initial = store.recovery_prefix().unwrap();
    assert_eq!(initial.sequence, 1);
    assert_eq!(
        initial.byte_len,
        fs::metadata(path.join(FILE)).unwrap().len()
    );
    machine
        .reserve_coordinator_operation(&mut store, id(91), 4, b"durable command")
        .unwrap();
    let selected = store.recovery_prefix().unwrap();
    assert_eq!(selected.sequence, initial.sequence + 1);
    assert!(selected.byte_len > initial.byte_len);
    assert_ne!(selected.head, initial.head);
    drop(store);
    let store = machine
        .open_coordinator_operation_store(&path, CAPACITY)
        .unwrap();
    assert_eq!(store.recovery_prefix().unwrap(), selected);
    let mut bytes = fs::read(path.join(FILE)).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    fs::write(path.join(FILE), bytes).unwrap();
    assert_eq!(store.recovery_prefix(), Err(StoreError::JournalCorrupt));
}

#[path = "recovery_journal_bundle_tests.rs"]
mod recovery_bundle;
