//! Transition and bounded replay controls over explicit claims; these fixtures do not execute Core.
use super::*;
mod borrowed;
mod fixture;
use fixture::{Fixture, actor, key};
use sorafs_manifest::signer::protocol::SignerOperationActionV1;

#[test]
fn complete_prefix_roundtrips_and_restores_ids_fences_audit_and_original_owner() {
    let mut f = Fixture::new();
    let first = f.reserve(10, 3, 120_000);
    assert_eq!(first.reservation.fence, 1);
    let completion = f.completion(&first);
    let completed = f
        .apply(
            TopologyActionV1::Complete(Box::new(completion)),
            4,
            125_000,
            32,
        )
        .unwrap()
        .operation
        .unwrap();
    assert_eq!(completed.reserved, first.reserved);
    assert_eq!(f.model.audit(), completion.commitment.audit);
    let second = f.reserve(11, 5, 130_000);
    assert_eq!(second.reservation.fence, 2);
    let expiry = TopologyExpireV1 {
        operation_id: [11; 32],
        reservation: second.reservation,
    };
    f.apply(
        TopologyActionV1::Expire(expiry),
        6,
        second.reservation.expires_at_unix_ms,
        33,
    )
    .unwrap();
    let mut restored = f.restore(f.frames.clone()).unwrap();
    assert_eq!(restored.history_head(), f.model.history_head());
    assert_eq!(restored.audit(), completion.commitment.audit);
    assert_eq!(restored.root.fence, 2);
    assert_eq!(restored.operation_inventory().count(), 2);
    assert_eq!(restored.signer_key_inventory().count(), 1);
    assert_eq!(restored.attester_key_inventory().count(), 1);
    assert_eq!(restored.operation(&[10; 32]), Some(&completed));
    assert_eq!(
        restored.operation(&[11; 32]).unwrap().outcome,
        TopologyOutcomeV1::Expired
    );
    let request = f.reviewed(10, 7, 195_000);
    let transition = f.transition(TopologyActionV1::Reserve(Box::new(request)));
    assert_eq!(
        restored.apply_claimed(&transition, &f.context(7, 195_000, 32)),
        Err(Error::Conflict)
    );
    for bytes in &f.frames {
        let decoded: TopologyHistoryEntryV1 = decode(bytes, TOPOLOGY_HISTORY_MAX_BYTES_V1).unwrap();
        assert_eq!(
            encode(&decoded, TOPOLOGY_HISTORY_MAX_BYTES_V1).unwrap(),
            *bytes
        );
    }
    let bytes = encode(&completed, TOPOLOGY_RECORD_MAX_BYTES_V1).unwrap();
    assert_eq!(
        decode::<TopologyOperationRecordV1>(&bytes, TOPOLOGY_RECORD_MAX_BYTES_V1).unwrap(),
        completed
    );
    let control = f.model.control().unwrap();
    let bytes = encode(control, TOPOLOGY_RECORD_MAX_BYTES_V1).unwrap();
    assert_eq!(
        decode::<TopologyControlRecordV1>(&bytes, TOPOLOGY_RECORD_MAX_BYTES_V1).unwrap(),
        *control
    );
}

#[test]
fn every_reviewed_reservation_coordinate_and_cas_is_bound_without_mutation() {
    for mutate in [
        |r: &mut TopologyReserveV1| r.subject.release_manifest_sha256[0] ^= 1,
        |r: &mut TopologyReserveV1| r.subject.manifest_sha256[0] ^= 1,
        |r: &mut TopologyReserveV1| r.subject.deployment_id = "production-other".into(),
        |r: &mut TopologyReserveV1| r.request.binding_digest[0] ^= 1,
        |r: &mut TopologyReserveV1| r.request.original_custody.record_digest[0] ^= 1,
        |r: &mut TopologyReserveV1| r.request.original_custody.control_state_digest[0] ^= 1,
        |r: &mut TopologyReserveV1| r.intent.operation_id[0] ^= 1,
        |r: &mut TopologyReserveV1| r.intent.request_digest[0] ^= 1,
        |r: &mut TopologyReserveV1| r.intent.action = SignerOperationActionV1::Status,
        |r: &mut TopologyReserveV1| r.intent.previous_audit.sequence = 1,
    ] {
        let mut f = Fixture::new();
        let before = f.model.history_head();
        let mut request = f.reviewed(10, 3, 120_000);
        mutate(&mut request);
        assert!(
            f.apply(TopologyActionV1::Reserve(Box::new(request)), 3, 120_000, 32)
                .is_err()
        );
        assert_eq!(f.model.history_head(), before);
        assert!(f.model.operations.is_empty());
    }
    let mut f = Fixture::new();
    let before = f.model.history_head();
    let mut transition = f.transition(TopologyActionV1::Reserve(Box::new(
        f.reviewed(10, 3, 120_000),
    )));
    transition.control.digest[0] ^= 1;
    assert_eq!(
        f.model
            .apply_claimed(&transition, &f.context(3, 120_000, 32)),
        Err(Error::Conflict)
    );
    assert_eq!(f.model.history_head(), before);
}

#[test]
fn reservation_is_exclusive_and_exact_duplicate_never_allocates_another_fence() {
    let mut f = Fixture::new();
    let first = f.reserve(10, 3, 120_000);
    let before = f.model.history_head();
    let duplicate = f
        .apply(
            TopologyActionV1::Reserve(Box::new(first.reviewed.clone())),
            4,
            121_000,
            32,
        )
        .unwrap();
    assert_eq!(duplicate, TopologyTransitionDeltaV1::unchanged());
    assert_eq!(f.model.history_head(), before);
    assert_eq!(f.model.root.fence, 1);
    assert!(
        f.apply(
            TopologyActionV1::Reserve(Box::new(first.reviewed.clone())),
            4,
            121_000,
            33
        )
        .is_err()
    );
    let other = f.reviewed(11, 4, 121_000);
    assert!(
        f.apply(TopologyActionV1::Reserve(Box::new(other)), 4, 121_000, 32)
            .is_err()
    );
    assert_eq!(f.model.history_head(), before);
    assert_eq!(f.model.operation(&[10; 32]), Some(&first));
}

#[test]
fn completion_requires_original_intent_owner_fence_time_and_next_audit() {
    for mutate in [
        |r: &mut TopologyCompleteV1| r.request.subject_digest[0] ^= 1,
        |r: &mut TopologyCompleteV1| r.intent.request_digest[0] ^= 1,
        |r: &mut TopologyCompleteV1| r.reservation.fence += 1,
        |r: &mut TopologyCompleteV1| r.reservation.reservation_id[0] ^= 1,
        |r: &mut TopologyCompleteV1| r.commitment.audit.sequence += 1,
        |r: &mut TopologyCompleteV1| r.commitment.audit.digest = [0; 32],
        |r: &mut TopologyCompleteV1| r.commitment.response_digest = [0; 32],
        |r: &mut TopologyCompleteV1| r.signatures_digest = [0; 32],
    ] {
        let mut f = Fixture::new();
        let row = f.reserve(10, 3, 120_000);
        let before = f.model.history_head();
        let mut request = f.completion(&row);
        mutate(&mut request);
        assert!(
            f.apply(
                TopologyActionV1::Complete(Box::new(request)),
                4,
                125_000,
                32
            )
            .is_err()
        );
        assert_eq!(f.model.history_head(), before);
        assert_eq!(f.model.operation(&[10; 32]), Some(&row));
    }
    let mut f = Fixture::new();
    let row = f.reserve(10, 3, 120_000);
    let request = f.completion(&row);
    assert!(
        f.apply(
            TopologyActionV1::Complete(Box::new(request)),
            4,
            125_000,
            33
        )
        .is_err()
    );
    assert!(
        f.apply(
            TopologyActionV1::Complete(Box::new(request)),
            4,
            row.reservation.expires_at_unix_ms,
            32
        )
        .is_err()
    );
    let transition = f.transition(TopologyActionV1::Complete(Box::new(request)));
    let mut same_block = f.context(3, 121_000, 32);
    same_block.execution.ordinal = 1;
    assert_eq!(
        f.model.apply_claimed(&transition, &same_block),
        Err(Error::Time)
    );
    f.apply(
        TopologyActionV1::Complete(Box::new(request)),
        4,
        125_000,
        32,
    )
    .unwrap();
    let head = f.model.history_head();
    assert_eq!(
        f.apply(
            TopologyActionV1::Complete(Box::new(request)),
            5,
            190_000,
            32
        )
        .unwrap(),
        TopologyTransitionDeltaV1::unchanged()
    );
    assert_eq!(f.model.history_head(), head);
    assert_eq!(f.model.audit().sequence, 1);
}

#[test]
fn custody_changes_atomically_invalidate_active_operations_and_survive_restart() {
    let mut f = Fixture::new();
    let row = f.reserve(10, 3, 120_000);
    let previous_control = f.model.control_head();
    let delta = f
        .apply(
            TopologyActionV1::Revoke {
                signer: true,
                attester: false,
            },
            4,
            125_000,
            31,
        )
        .unwrap();
    assert!(delta.control.is_some());
    assert_eq!(
        delta.operation.as_ref().unwrap().outcome,
        TopologyOutcomeV1::Invalidated
    );
    assert_eq!(delta.operation.unwrap().reserved, row.reserved);
    assert_ne!(f.model.control_head(), previous_control);
    assert_eq!(f.model.audit().sequence, 0);
    assert_eq!(f.model.root.fence, 1);
    let mut restored = f.restore(f.frames.clone()).unwrap();
    let transition = f.transition(TopologyActionV1::Complete(Box::new(f.completion(&row))));
    assert!(
        restored
            .apply_claimed(&transition, &f.context(5, 130_000, 32))
            .is_err()
    );
    assert!(
        f.apply(
            TopologyActionV1::Revoke {
                signer: true,
                attester: false
            },
            5,
            130_000,
            31
        )
        .is_err()
    );
    f.apply(
        TopologyActionV1::Revoke {
            signer: false,
            attester: true,
        },
        5,
        130_000,
        31,
    )
    .unwrap();
    assert!(
        f.restore(f.frames.clone())
            .unwrap()
            .state
            .unwrap()
            .attester_revoked
    );
}

#[test]
fn rotation_preserves_old_key_tombstones_and_renewal_cannot_relabel_an_operation() {
    let mut f = Fixture::new();
    let original = f.policy.clone();
    let reserved = f.reserve(10, 3, 120_000);
    let mut rotated = f.policy.clone();
    rotated.binding.public_key = key(23).public_key().clone();
    rotated.binding.key_revision += 1;
    f.apply(
        TopologyActionV1::Configure(norito::encode_canonical(&rotated).unwrap()),
        4,
        125_000,
        31,
    )
    .unwrap();
    assert_eq!(
        f.model.operation(&[10; 32]).unwrap().outcome,
        TopologyOutcomeV1::Invalidated
    );
    assert!(f.model.state.as_ref().unwrap().active_head.is_none());
    f.model = f.restore(f.frames.clone()).unwrap();
    let mut reused = rotated;
    reused.binding.public_key = original.binding.public_key;
    reused.binding.key_revision += 1;
    let before = f.model.history_head();
    assert_eq!(
        f.apply(
            TopologyActionV1::Configure(norito::encode_canonical(&reused).unwrap()),
            5,
            130_000,
            31
        ),
        Err(Error::Generation)
    );
    assert_eq!(f.model.history_head(), before);
    assert_eq!(f.model.signer_keys.len(), 2);
    let mut fresh = Fixture::new();
    let row = fresh.reserve(10, 3, 120_000);
    let enrollment = fresh.enrollment(4, 125_000);
    fresh
        .apply(TopologyActionV1::Enroll(enrollment), 4, 125_000, 31)
        .unwrap();
    assert_eq!(
        fresh.model.operation(&[10; 32]).unwrap().outcome,
        TopologyOutcomeV1::Invalidated
    );
    assert!(
        fresh
            .apply(
                TopologyActionV1::Complete(Box::new(fresh.completion(&row))),
                5,
                130_000,
                32
            )
            .is_err()
    );
    assert_eq!(reserved.reservation.fence, row.reservation.fence);
}

#[test]
fn replay_requires_every_canonical_row_and_the_independently_pinned_terminal_head() {
    let mut f = Fixture::new();
    let row = f.reserve(10, 3, 120_000);
    f.apply(
        TopologyActionV1::Complete(Box::new(f.completion(&row))),
        4,
        125_000,
        32,
    )
    .unwrap();
    for frames in [
        Vec::new(),
        f.frames[1..].to_vec(),
        f.frames[..3].to_vec(),
        vec![
            f.frames[0].clone(),
            f.frames[2].clone(),
            f.frames[3].clone(),
        ],
        vec![
            f.frames[0].clone(),
            f.frames[1].clone(),
            f.frames[1].clone(),
            f.frames[3].clone(),
        ],
    ] {
        assert!(f.restore(frames).is_err());
    }
    let mut extra = f.frames.clone();
    extra.push(f.frames[3].clone());
    assert!(f.restore(extra).is_err());
    let mut changed = f.frames.clone();
    let mut entry: TopologyHistoryEntryV1 = norito::decode_canonical(&changed[2]).unwrap();
    entry.context.execution.authority = actor(99);
    changed[2] = norito::encode_canonical(&entry).unwrap();
    assert!(f.restore(changed).is_err());
    let mut corrupt = f.frames.clone();
    corrupt[1].pop();
    assert!(f.restore(corrupt).is_err());
    assert!(
        f.restore(vec![vec![0; TOPOLOGY_HISTORY_MAX_BYTES_V1 + 1]])
            .is_err()
    );
    let mut wrong = f.model.retained().clone();
    wrong.history_head.digest = [99; 32];
    assert!(
        TopologyTransitionModelV1::restore_claimed(
            "production-primary".into(),
            [1; 32],
            "topology-chain".into(),
            369,
            &wrong,
            f.frames
        )
        .is_err()
    );
}

#[test]
fn challenged_checks_bind_floor_operator_candidate_phase_and_do_not_mutate() {
    let mut f = Fixture::new();
    let row = f.reserve(10, 3, 120_000);
    let floor = TopologyFloorClaimV1 {
        height: 2,
        block_hash: [42; 32],
    };
    let request = TopologyCheckV1 {
        challenge: [18; 32],
        network_id: [1; 32],
        floor,
        expected_operator: actor(32),
        reviewed: row.reviewed.clone(),
        phase: TopologyCheckPhaseV1::BeforeProvider(Box::new(row.clone())),
    };
    let mut context = f.context(4, 125_000, 33);
    context.floor = Some(floor);
    let before = f.model.history_head();
    let mut check = request.clone();
    for phase in [
        TopologyCheckPhaseV1::Current(Box::new(f.model.audit())),
        TopologyCheckPhaseV1::BeforeProvider(Box::new(row.clone())),
        TopologyCheckPhaseV1::AfterProvider(Box::new(row.clone())),
        TopologyCheckPhaseV1::BeforeCommit(Box::new(row.clone())),
    ] {
        check.phase = phase;
        assert_eq!(
            f.model
                .apply_claimed(
                    &f.transition(TopologyActionV1::Check(Box::new(check.clone()))),
                    &context
                )
                .unwrap(),
            TopologyTransitionDeltaV1::unchanged()
        );
    }
    for mutate in [
        |r: &mut TopologyCheckV1| r.challenge = [0; 32],
        |r: &mut TopologyCheckV1| r.network_id[0] ^= 1,
        |r: &mut TopologyCheckV1| r.floor.block_hash[0] ^= 1,
        |r: &mut TopologyCheckV1| r.expected_operator = actor(33),
        |r: &mut TopologyCheckV1| r.reviewed.subject.release_manifest_sha256[0] ^= 1,
    ] {
        let mut altered = request.clone();
        mutate(&mut altered);
        assert!(
            f.model
                .apply_claimed(
                    &f.transition(TopologyActionV1::Check(Box::new(altered))),
                    &context
                )
                .is_err()
        );
        assert_eq!(f.model.history_head(), before);
    }
    let mut swapped = request;
    if let TopologyCheckPhaseV1::BeforeProvider(row) = &mut swapped.phase {
        row.reservation.fence += 1;
    }
    assert!(
        f.model
            .apply_claimed(
                &f.transition(TopologyActionV1::Check(Box::new(swapped))),
                &context
            )
            .is_err()
    );
    assert_eq!(f.model.history_head(), before);
}

#[test]
fn completed_checks_recover_after_original_reservation_expiry_without_rewriting_audit() {
    let mut f = Fixture::new();
    let row = f.reserve(10, 3, 120_000);
    let floor = TopologyFloorClaimV1 {
        height: 2,
        block_hash: [42; 32],
    };
    let mut check = TopologyCheckV1 {
        challenge: [20; 32],
        network_id: [1; 32],
        floor,
        expected_operator: actor(32),
        reviewed: row.reviewed.clone(),
        phase: TopologyCheckPhaseV1::Current(Box::new(f.model.audit())),
    };
    let completion = f.completion(&row);
    let done = f
        .apply(
            TopologyActionV1::Complete(Box::new(completion)),
            4,
            125_000,
            32,
        )
        .unwrap()
        .operation
        .unwrap();
    for phase in [
        TopologyCheckPhaseV1::AfterCommit(Box::new(done.clone())),
        TopologyCheckPhaseV1::BeforeRelease(Box::new(done)),
    ] {
        check.phase = phase;
        let mut late = f.context(5, 190_000, 33);
        late.floor = Some(floor);
        f.model
            .apply_claimed(
                &f.transition(TopologyActionV1::Check(Box::new(check.clone()))),
                &late,
            )
            .unwrap();
    }
    assert_eq!(f.model.audit().sequence, 1);
}

#[test]
fn finite_limits_preserve_emergency_revocation_and_terminal_capacity() {
    let mut f = Fixture::new();
    // Counter-edge injection is confined to the pure reducer test. It is not accepted native history.
    f.model.root.control_head.revision = TOPOLOGY_CONTROL_NORMAL_LIMIT_V1;
    f.model.root.history_head.revision = TOPOLOGY_CONTROL_NORMAL_LIMIT_V1;
    let control = f.model.control.as_mut().unwrap();
    control.revision = TOPOLOGY_CONTROL_NORMAL_LIMIT_V1;
    f.model.root.control_head.digest =
        digest(b"iroha.sorafs.topology.control.v1\0", control).unwrap();
    let enrollment = f.enrollment(3, 120_000);
    assert_eq!(
        f.apply(TopologyActionV1::Enroll(enrollment), 3, 120_000, 31),
        Err(Error::Capacity)
    );
    f.apply(
        TopologyActionV1::Revoke {
            signer: true,
            attester: false,
        },
        3,
        120_000,
        31,
    )
    .unwrap();
    f.apply(
        TopologyActionV1::Revoke {
            signer: false,
            attester: true,
        },
        4,
        125_000,
        31,
    )
    .unwrap();
    assert_eq!(f.model.control_head().revision, TOPOLOGY_CONTROL_LIMIT_V1);
    assert_eq!(
        f.apply(
            TopologyActionV1::Revoke {
                signer: true,
                attester: true
            },
            5,
            130_000,
            31
        ),
        Err(Error::Capacity)
    );
    let mut f = Fixture::new();
    f.model.root.operation_count = TOPOLOGY_OPERATION_LIMIT_V1;
    f.model.root.fence = TOPOLOGY_OPERATION_LIMIT_V1;
    f.model.root.operation_head.revision = TOPOLOGY_OPERATION_LIMIT_V1 * 2;
    f.model.root.operation_head.digest = [99; 32];
    f.model.root.history_head.revision =
        f.model.root.operation_head.revision + f.model.root.control_head.revision;
    let request = f.reviewed(10, 3, 120_000);
    let before = f.model.history_head();
    assert_eq!(
        f.apply(TopologyActionV1::Reserve(Box::new(request)), 3, 120_000, 32),
        Err(Error::Capacity)
    );
    assert_eq!(f.model.history_head(), before);
    assert!(f.model.operations.is_empty());
    let mut f = Fixture::new();
    f.model.root.history_head.revision = TOPOLOGY_HISTORY_LIMIT_V1;
    let before = f.model.control_head();
    let transition = f.transition(TopologyActionV1::Revoke {
        signer: true,
        attester: false,
    });
    let context = f.context(3, 120_000, 31);
    // Public preparation rejects this inconsistent synthetic summary before using any row.
    assert_eq!(
        f.model.view().prepare_claimed(&transition, &context),
        Err(TopologyPreparationErrorV1::Transition(Error::History))
    );
    // Exercise the private final capacity arithmetic separately; this is not a valid native cut.
    assert_eq!(
        f.model
            .view()
            .prepare_publication(&transition, &context, None, None),
        Err(TopologyPreparationErrorV1::Transition(Error::Capacity))
    );
    assert_eq!(f.model.control_head(), before);
    assert!(!f.model.state.as_ref().unwrap().signer_revoked);
}

#[test]
fn failed_control_invalidation_does_not_partially_revoke_or_advance_any_head() {
    let mut f = Fixture::new();
    let row = f.reserve(10, 3, 120_000);
    // Deliberately corrupt the local head; borrowed preparation must refuse it without a write.
    f.model.root.operation_head.digest[0] ^= 1;
    let control = f.model.control_head();
    let history = f.model.history_head();
    assert!(
        f.apply(
            TopologyActionV1::Revoke {
                signer: true,
                attester: false
            },
            4,
            125_000,
            31
        )
        .is_err()
    );
    assert_eq!(f.model.control_head(), control);
    assert_eq!(f.model.history_head(), history);
    assert!(!f.model.state.as_ref().unwrap().signer_revoked);
    assert_eq!(f.model.operation(&[10; 32]), Some(&row));
}

#[test]
fn every_action_has_one_canonical_frame_and_oversized_inputs_fail_before_publication() {
    let mut f = Fixture::new();
    let row = f.reserve(10, 3, 120_000);
    let enrolled: TopologyHistoryEntryV1 = norito::decode_canonical(&f.frames[1]).unwrap();
    let check = TopologyCheckV1 {
        challenge: [19; 32],
        network_id: [1; 32],
        floor: TopologyFloorClaimV1 {
            height: 2,
            block_hash: [42; 32],
        },
        expected_operator: actor(32),
        reviewed: row.reviewed.clone(),
        phase: TopologyCheckPhaseV1::Current(Box::new(f.model.audit())),
    };
    for action in [
        TopologyActionV1::Configure(norito::encode_canonical(&f.policy).unwrap()),
        enrolled.transition.action,
        TopologyActionV1::Revoke {
            signer: true,
            attester: false,
        },
        TopologyActionV1::Reserve(Box::new(row.reviewed.clone())),
        TopologyActionV1::Complete(Box::new(f.completion(&row))),
        TopologyActionV1::Expire(TopologyExpireV1 {
            operation_id: [10; 32],
            reservation: row.reservation,
        }),
        TopologyActionV1::Check(Box::new(check)),
    ] {
        let bytes = encode(&action, TOPOLOGY_RECORD_MAX_BYTES_V1).unwrap();
        assert_eq!(
            decode::<TopologyActionV1>(&bytes, TOPOLOGY_RECORD_MAX_BYTES_V1).unwrap(),
            action
        );
        assert!(
            decode::<TopologyActionV1>(&bytes[..bytes.len() - 1], TOPOLOGY_RECORD_MAX_BYTES_V1)
                .is_err()
        );
        let mut suffix = bytes;
        suffix.push(0);
        assert!(decode::<TopologyActionV1>(&suffix, TOPOLOGY_RECORD_MAX_BYTES_V1).is_err());
    }
    let before = f.model.history_head();
    assert_eq!(
        f.apply(
            TopologyActionV1::Enroll(vec![0; 16 * 1024 + 1]),
            4,
            125_000,
            31
        ),
        Err(Error::Invalid)
    );
    assert_eq!(f.model.history_head(), before);
}

#[test]
fn role_scope_enrollment_signature_predecessor_and_parent_anchor_are_mandatory() {
    use sorafs_manifest::signer::{
        custody::SignerCustodyRecordV1,
        protocol::{SignerPurposeBindingV1, SignerRoleV1},
    };
    let mut f = Fixture::new();
    let before = f.model.history_head();
    let mut other = f.policy.clone();
    other.binding.role = SignerRoleV1::Promotion;
    other.binding.purpose = SignerPurposeBindingV1::NativeOrPromotion;
    assert_eq!(
        f.apply(
            TopologyActionV1::Configure(norito::encode_canonical(&other).unwrap()),
            3,
            120_000,
            31
        ),
        Err(Error::Binding)
    );
    let mut record: SignerCustodyRecordV1 =
        norito::decode_canonical(&f.enrollment(3, 120_000)).unwrap();
    record.attestation[0] ^= 1;
    assert_eq!(
        f.apply(
            TopologyActionV1::Enroll(norito::encode_canonical(&record).unwrap()),
            3,
            120_000,
            31
        ),
        Err(Error::Custody)
    );
    let old: TopologyHistoryEntryV1 = norito::decode_canonical(&f.frames[1]).unwrap();
    assert_eq!(
        f.apply(old.transition.action, 3, 120_000, 31),
        Err(Error::Custody)
    );
    let request = f.reviewed(10, 3, 120_000);
    let transition = f.transition(TopologyActionV1::Reserve(Box::new(request)));
    let mut context = f.context(3, 120_000, 32);
    context.custody_anchor.as_mut().unwrap().state_digest[0] ^= 1;
    assert_eq!(
        f.model.apply_claimed(&transition, &context),
        Err(Error::Custody)
    );
    assert_eq!(f.model.history_head(), before);
    assert!(f.model.operations.is_empty());
}

#[test]
fn reviewed_expiry_caps_reservation_even_while_custody_remains_eligible() {
    use sorafs_manifest::signer::topology::subject::prepare_topology_approval_v1;
    let mut f = Fixture::new();
    let mut request = f.reviewed(10, 3, 120_000);
    request.subject.expires_at_unix_ms = 130_000;
    let context = f.context(3, 120_000, 32);
    let custody = f.model.view().use_current(&context).unwrap();
    let prepared =
        prepare_topology_approval_v1(&request.subject, &custody.statement().binding).unwrap();
    request.request = SignerTopologyRequestV1::new(&custody, [10; 32], &prepared).unwrap();
    request.intent.request_digest = request.request.digest().unwrap();
    let row = f
        .apply(TopologyActionV1::Reserve(Box::new(request)), 3, 120_000, 32)
        .unwrap()
        .operation
        .unwrap();
    assert_eq!(row.reservation.expires_at_unix_ms, 130_000);
    assert!(
        f.model
            .view()
            .use_current(&f.context(4, 130_000, 32))
            .is_ok()
    );
    let before = f.model.history_head();
    assert_eq!(
        f.apply(
            TopologyActionV1::Complete(Box::new(f.completion(&row))),
            4,
            130_000,
            32
        ),
        Err(Error::Time)
    );
    assert_eq!(f.model.history_head(), before);
}
