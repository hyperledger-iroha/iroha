//! Public cryptographic/cache specimens only: no authenticated Core or monetary owner.

use super::*;
use crate::kagemusha_core_coordinator_v1::startup_qualification::tests::{qualification, reseal};
use crate::kagemusha_device_bridge_v1::{
    QualificationProjectionV1,
    sender_payload::{
        SenderCommandBodyV1, SenderCommandV1, SenderPhaseV1, SenderRecordV1, SenderRecoveryItemV1,
        SenderRecoverySelectorV1, SenderReplyBodyV1, SenderReplyV1, SenderWalletContextV1,
        canonical_command_body_for_tests,
    },
};
use iroha_data_model::kagemusha::{
    KagemushaOperationKindV1, kagemusha_device_response_signing_bytes_v1,
};
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

fn context(qualification: &QualificationProjectionV1) -> SenderWalletContextV1 {
    let command = canonical_command_body_for_tests(6).unwrap();
    let mut command: SenderCommandV1 = norito::decode_canonical(&command).unwrap();
    command.context.lane.network_id = qualification.credential.network_id;
    command.context.lane.device_lane_id = qualification.credential.lane_commitment;
    command.context.release.release_id = qualification.release_id;
    command.context.release.hardware_profile_id = qualification.profile.hardware_profile_id;
    command.context.release.suite_id = qualification.credential.suite_id;
    command.context.credential_id = qualification.credential.credential_id;
    command.context.hardware_epoch.generation =
        u128::from(qualification.credential.hardware_epoch_generation);
    command.context.hardware_epoch.epoch_id = qualification.credential.hardware_epoch_id;
    command.context.device_policy_binding.device_key_reference =
        qualification.credential.device_key_reference;
    command.context.device_policy_binding.hardware_policy_id = [0x91; 32];
    command.context.core_authorization_key_reference =
        qualification.core_authorization_key_reference;
    command.context
}

fn command(operation: u8, request: [u8; 32], inputs_digest: [u8; 32]) -> SenderCommandV1 {
    SenderCommandV1 {
        version: 1,
        operation,
        operation_id: request,
        context: context(&qualification(1)),
        body: match operation {
            6 => SenderCommandBodyV1::RecoverPrepared { inputs_digest },
            8 => SenderCommandBodyV1::RecoverTerminal { inputs_digest },
            10 => SenderCommandBodyV1::RecoverInstalled {
                selector: SenderRecoverySelectorV1::Lookup { inputs_digest },
            },
            _ => panic!("read fixture operation"),
        },
    }
}

fn signed_read(
    command: &SenderCommandV1,
    qualification: &QualificationProjectionV1,
    revision: u128,
    body: SenderReplyBodyV1,
) -> AuthenticatedSenderReplyV1 {
    let current = context(qualification);
    let reply = SenderReplyV1 {
        version: 1,
        operation: command.operation,
        request_id: command.operation_id,
        context: current.clone(),
        index_revision: revision,
        body,
    }
    .encode_canonical(command, &current)
    .unwrap();
    let encoded_command = command.encode_canonical().unwrap();
    let transcript = kagemusha_device_response_signing_bytes_v1(
        command.operation,
        command.operation_id,
        &encoded_command,
        &reply,
        qualification.hardware_policy_digest,
        qualification.profile.qualification_report_digest,
    )
    .unwrap();
    let seed = 10 + qualification.credential.hardware_epoch_generation as u8;
    let signature: Signature = SigningKey::from_bytes((&[seed; 32]).into())
        .unwrap()
        .sign(&transcript);
    let signature = signature.normalize_s().unwrap_or(signature).to_bytes();
    AuthenticatedSenderReplyV1::authenticate(
        command.operation,
        command.operation_id,
        &encoded_command,
        &reply,
        &signature,
        &current,
        qualification,
        current.device_policy_binding.hardware_policy_id,
    )
    .unwrap()
}

#[test]
fn signed_current_read_replaces_older_index_observation_for_all_lookup_operations() {
    for operation in [6, 8, 10] {
        let command = command(operation, [7; 32], [5; 32]);
        let qualification = qualification(1);
        let old = signed_read(&command, &qualification, 1, SenderReplyBodyV1::Lookup(None));
        let next = signed_read(&command, &qualification, 2, SenderReplyBodyV1::Lookup(None));
        assert!(old != next);
        let mut cache = BTreeMap::new();
        admit_authenticated(&mut cache, old).unwrap();
        admit_authenticated(&mut cache, next.clone()).unwrap();
        assert!(cache.get(&(operation, [7; 32])).unwrap() == &next);
        assert_eq!(cache.len(), 1);
        let rollback = signed_read(&command, &qualification, 1, SenderReplyBodyV1::Lookup(None));
        assert!(admit_authenticated(&mut cache, rollback).is_err());
        assert!(cache.get(&(operation, [7; 32])).unwrap() == &next);
    }
}

#[test]
fn signed_credential_renewal_and_epoch_rotation_refresh_only_the_current_read() {
    for operation in [6, 8, 10] {
        let command = command(operation, [7; 32], [5; 32]);
        let original = qualification(1);
        let mut renewed = original.clone();
        renewed.credential.issued_at_ms += 1;
        let renewed = reseal(renewed);
        let rotated = qualification(2);
        let mut cache = BTreeMap::new();
        let old = signed_read(&command, &original, 2, SenderReplyBodyV1::Lookup(None));
        admit_authenticated(&mut cache, old).unwrap();
        for current in [&renewed, &rotated] {
            let next = signed_read(&command, current, 2, SenderReplyBodyV1::Lookup(None));
            assert_ne!(
                next.reply().context,
                cache.get(&(operation, [7; 32])).unwrap().reply().context
            );
            assert_eq!(next.command().context, command.context);
            admit_authenticated(&mut cache, next.clone()).unwrap();
            assert!(cache.get(&(operation, [7; 32])).unwrap() == &next);
        }
        let old_epoch_rollback =
            signed_read(&command, &rotated, 1, SenderReplyBodyV1::Lookup(None));
        assert!(admit_authenticated(&mut cache, old_epoch_rollback).is_err());
    }
}

#[test]
fn signed_substituted_command_and_conflicting_same_revision_body_leave_cache_unchanged() {
    let command = command(6, [7; 32], [5; 32]);
    let qualification = qualification(1);
    let old = signed_read(&command, &qualification, 2, SenderReplyBodyV1::Lookup(None));
    let mut cache = BTreeMap::new();
    admit_authenticated(&mut cache, old.clone()).unwrap();
    let mut changed_command = command.clone();
    changed_command.body = SenderCommandBodyV1::RecoverPrepared {
        inputs_digest: [6; 32],
    };
    let changed = signed_read(
        &changed_command,
        &qualification,
        3,
        SenderReplyBodyV1::Lookup(None),
    );
    assert!(admit_authenticated(&mut cache, changed).is_err());

    // This is a signed public tombstone shape, never a genuine native Released record.
    let tombstone = SenderRecoveryItemV1 {
        record: SenderRecordV1 {
            operation_id: command.operation_id,
            context: command.context.clone(),
            inputs_digest: [5; 32],
            operation_kind: KagemushaOperationKindV1::SendSplit,
            preparation_id: [1; 32],
            outbox_reservation_id: [2; 32],
            outcome_id: [3; 32],
            phase: SenderPhaseV1::Released,
            record_revision: 2,
            inputs: None,
            candidate_digest: Some([4; 32]),
            commit_certificate_digest: Some([5; 32]),
            envelope_digest: Some([6; 32]),
            terminal_receipt_digest: Some([7; 32]),
        },
        canonical_envelope: Vec::new(),
    };
    let conflicting = signed_read(
        &command,
        &qualification,
        2,
        SenderReplyBodyV1::Lookup(Some(tombstone)),
    );
    assert!(admit_authenticated(&mut cache, conflicting).is_err());
    assert!(cache.get(&(6, [7; 32])).unwrap() == &old);
}

#[test]
fn mutation_decision_retains_every_original_byte_and_refuses_changed_retries() {
    // Byte markers exercise only whole-value equality. They are not signed operation frames.
    // The production caller passes the entire authenticated token, including op12 originals.
    let original = (vec![1, 2], vec![3, 4], vec![5, 6], Some(vec![7, 8]));
    for operation in [5, 7, 9, 12] {
        assert_eq!(
            admission(operation, &original, &original).unwrap(),
            Admission::Keep
        );
        let mut changes = Vec::new();
        let mut changed = original.clone();
        changed.0.push(9);
        changes.push(changed);
        let mut changed = original.clone();
        changed.1.push(9);
        changes.push(changed);
        let mut changed = original.clone();
        changed.2.push(9);
        changes.push(changed);
        let mut changed = original.clone();
        changed.3.as_mut().unwrap().push(9);
        changes.push(changed);
        let mut changed = original.clone();
        changed.3 = None;
        changes.push(changed);
        for changed in changes {
            assert!(admission(operation, &original, &changed).is_err());
        }
    }
    assert!(admission(11, &original, &original).is_err());
}

#[test]
fn more_than_sixteen_authenticated_reads_retire_only_read_slots() {
    let qualification = qualification(1);
    let mut cache = BTreeMap::new();
    for id in 1..=40 {
        let command = command([6, 8, 10][usize::from(id - 1) % 3], [id; 32], [5; 32]);
        let token = signed_read(&command, &qualification, 1, SenderReplyBodyV1::Lookup(None));
        let key = (command.operation, command.operation_id);
        admit_authenticated(&mut cache, token.clone()).unwrap();
        assert!(cache.get(&key).unwrap() == &token);
        assert!(cache.len() <= MAX_SENDER_REPLIES);
    }
    assert_eq!(cache.len(), 16);
    let (operation, id) = *cache.keys().next().unwrap();
    let command = command(operation, id, [5; 32]);
    let refreshed = signed_read(&command, &qualification, 2, SenderReplyBodyV1::Lookup(None));
    admit_authenticated(&mut cache, refreshed.clone()).unwrap();
    assert!(cache.get(&(operation, id)).unwrap() == &refreshed);
}

#[test]
fn victim_selection_preserves_every_uncertain_mutation_and_retirement_is_operation_exact() {
    // Structural storage markers only: only the production native proof caller may invoke
    // completion retirement. These values manufacture no authenticated token or authority.
    let mut cache = BTreeMap::new();
    for operation in [5, 7, 9, 12] {
        cache.insert((operation, [1; 32]), operation);
    }
    assert!(read_victim(&cache).is_none());
    for operation in [6, 8, 10] {
        cache.insert((operation, [2; 32]), operation);
    }
    while let Some(victim) = read_victim(&cache) {
        assert!(matches!(victim.0, 6 | 8 | 10));
        cache.remove(&victim);
    }
    assert_eq!(cache.len(), 4);
    cache.insert((12, [3; 32]), 12);
    retire_completed_operation(&mut cache, [1; 32]);
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.get(&(12, [3; 32])), Some(&12));
    retire_completed_operation(&mut cache, [4; 32]);
    assert_eq!(cache.len(), 1);
}

#[test]
fn resolving_release_requires_no_extra_transient_slot_when_uncertain_mutations_fill_cache() {
    let mut occupied = BTreeMap::new();
    for id in 1..=16 {
        occupied.insert((5, [id; 32]), id);
    }
    assert_eq!(occupied.len(), MAX_SENDER_REPLIES);
    assert!(read_victim(&occupied).is_none());
    assert!(!needs_transient_slot(12));
    for operation in [5, 6, 7, 8, 9, 10] {
        assert!(needs_transient_slot(operation));
    }
    // Slot exemption is not authentication/completion; only genuine native completion proof
    // allows the resolving caller to invoke exact-operation storage retirement.
    retire_completed_operation(&mut occupied, [1; 32]);
    assert_eq!(occupied.len(), 15);
    assert!(!occupied.contains_key(&(5, [1; 32])));
    for id in 2..=16 {
        assert_eq!(occupied.get(&(5, [id; 32])), Some(&id));
    }
}
