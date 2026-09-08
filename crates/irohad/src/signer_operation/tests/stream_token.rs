//! Stream-token producer simulations with real custody signatures and private journal persistence.

use super::super::{
    journal::{SignerReceiptJournalV1, SignerReceiptPurposeV1},
    stream_token::{
        SignerStreamTokenErrorV1, SignerStreamTokenReceiptBytesV1, SignerStreamTokenServiceV1,
    },
};
use super::*;
use sorafs_manifest::{
    StreamTokenBodyV1, StreamTokenV1,
    signer::{
        protocol::signer_operation_signatures_digest_v1,
        stream_token::{
            SignerStreamTokenExpectedV1, SignerStreamTokenReceiptErrorV1,
            SignerStreamTokenReceiptV1,
        },
    },
};
use std::{fs, os::unix::fs::PermissionsExt as _};

#[path = "stream_token_support.rs"]
mod support;
use support::*;

#[test]
fn stream_receipt_persists_four_exact_signatures_and_restart_recovery_never_uses_key() {
    let mut harness = Harness::new();
    assert_eq!(
        harness.source.counts().current,
        1,
        "service construction freshly observes custody"
    );
    let body = body(1);
    let receipt = harness
        .sign(&body)
        .expect("complete canonical stream receipt");
    let bytes = receipt.bytes().to_vec();
    let decoded: SignerStreamTokenReceiptV1 = norito::decode_canonical(&bytes).unwrap();
    assert_eq!(decoded.encode_canonical().unwrap(), bytes);
    assert_eq!(decoded.signatures.len(), 4);
    assert_eq!(
        fs::read(harness.source.path(decoded.intent.operation_id)).unwrap(),
        bytes
    );
    assert_eq!(harness.source.staged_checked.load(Ordering::SeqCst), 1);
    assert_eq!(harness.calls(), 4);
    assert_eq!(harness.source.base.state.lock().unwrap().commits, 1);
    let debug = format!("{receipt:?}");
    assert!(!debug.contains(&body.token_id));
    assert!(!debug.contains(&hex::encode(&decoded.signatures[0].signature)));
    assert!(!format!("{:?}", harness.service()).contains(&body.token_id));
    harness.restart();
    let before = harness.source.counts();
    assert_eq!(
        harness.service().recover(&payload(&body)).unwrap().bytes(),
        bytes
    );
    let after = harness.source.counts();
    assert!(after.current > before.current);
    assert_eq!(
        (after.signing, after.reserves, after.commits),
        (before.signing, before.reserves, before.commits)
    );
    assert_eq!(after.completed - before.completed, 3);
    assert_eq!(harness.calls(), 4);
    assert!(harness.sign(&body).is_err());
    assert_eq!(
        harness.calls(),
        4,
        "duplicate signing never retries a completed key operation"
    );
}

#[test]
fn stream_receipt_bytes_are_identical_under_all_ten_ambient_layouts() {
    let mut canonical: Option<Vec<u8>> = None;
    for flags in layouts() {
        let _guard = norito::core::DecodeFlagsGuard::enter(flags);
        let harness = Harness::new();
        let body = body(2);
        let exact_payload = payload(&body);
        assert_eq!(body.signing_payload_bytes().unwrap(), exact_payload);
        let receipt = harness.sign(&body).unwrap();
        if let Some(expected) = &canonical {
            assert_eq!(receipt.bytes(), expected);
        } else {
            canonical = Some(receipt.bytes().to_vec());
        }
        assert_eq!(
            harness.service().recover(&exact_payload).unwrap().bytes(),
            receipt.bytes()
        );
        assert_eq!(harness.calls(), 4);
    }
}

#[test]
fn long_lived_stream_service_reads_successive_heads_and_recovers_historical_completion() {
    let harness = Harness::new();
    harness.source.base.state.lock().unwrap().audit = SignerOperationAuditHeadV1 {
        sequence: 0,
        digest: [0; 32],
    };
    let first_body = body(3);
    let second_body = body(4);
    let first = harness.sign(&first_body).unwrap();
    let first_receipt: SignerStreamTokenReceiptV1 =
        norito::decode_canonical(first.bytes()).unwrap();
    let second = harness.sign(&second_body).unwrap();
    let second_receipt: SignerStreamTokenReceiptV1 =
        norito::decode_canonical(second.bytes()).unwrap();
    assert_eq!(first_receipt.intent.previous_audit.sequence, 0);
    assert_eq!(first_receipt.intent.previous_audit.digest, [0; 32]);
    assert_eq!(first_receipt.commitment.audit.sequence, 1);
    assert_eq!(second_receipt.commitment.audit.sequence, 2);
    assert_ne!(
        first_receipt.intent.operation_id,
        second_receipt.intent.operation_id
    );
    assert_eq!(
        second_receipt.intent.previous_audit,
        first_receipt.commitment.audit
    );
    assert_eq!(
        second_receipt.commitment.audit.sequence,
        first_receipt.commitment.audit.sequence + 1
    );
    assert_eq!(
        second_receipt.reservation.fence,
        first_receipt.reservation.fence + 1
    );
    assert_eq!(harness.source.history.lock().unwrap().len(), 2);
    assert_eq!(harness.source.counts().signing, 2);
    let before = harness.source.counts();
    // The original reservation expired at 1800, but its actual completion was committed at 1500.
    // This is fresh read-only recovery before both token and original custody expire at 2000.
    {
        let mut state = harness.source.base.state.lock().unwrap();
        state.context.now_unix_ms = 1850;
        state.context.anchor_observed_at_unix_ms = 1850;
    }
    assert_eq!(
        harness
            .service()
            .recover(&payload(&first_body))
            .unwrap()
            .bytes(),
        first.bytes()
    );
    let after = harness.source.counts();
    assert_eq!(
        (after.signing, after.reserves, after.commits),
        (before.signing, before.reserves, before.commits)
    );
    assert_eq!(harness.calls(), 8);
    assert_eq!(
        harness.source.base.state.lock().unwrap().audit,
        second_receipt.commitment.audit
    );
}

#[test]
fn stream_snapshot_head_race_fails_cas_before_any_provider_operation() {
    for sequence in [0, u64::MAX] {
        let harness = Harness::new();
        harness.source.base.state.lock().unwrap().audit = SignerOperationAuditHeadV1 {
            sequence,
            digest: [0x65; 32],
        };
        assert_eq!(
            harness.sign(&body(5)).unwrap_err(),
            SignerStreamTokenErrorV1::Operation(SignerOperationErrorV1::InvalidOperation)
        );
        assert_eq!(harness.source.counts().signing, 1);
        assert_eq!(harness.source.counts().reserves, 0);
        assert_eq!(harness.calls(), 0);
        let state = harness.source.base.state.lock().unwrap();
        assert!(state.used_ids.is_empty());
        assert_eq!(state.commits, 0);
        assert_eq!(fs::read_dir(&harness.source.directory).unwrap().count(), 0);
    }
    // A structurally valid but concurrently superseded head instead reaches authoritative CAS.
    let harness = Harness::new();
    let body = body(5);
    *harness.source.advance_head_after_snapshot.lock().unwrap() = true;
    assert!(matches!(
        harness.sign(&body),
        Err(SignerStreamTokenErrorV1::Operation(
            SignerOperationErrorV1::ReservationConflict
        ))
    ));
    assert_eq!(harness.source.counts().signing, 1);
    assert_eq!(harness.source.counts().reserves, 1);
    assert_eq!(harness.calls(), 0);
    assert_eq!(harness.source.base.state.lock().unwrap().commits, 0);
    assert_eq!(fs::read_dir(&harness.source.directory).unwrap().count(), 0);
}

#[test]
fn two_stream_operations_with_one_snapshot_have_exactly_one_cas_winner() {
    let harness = Harness::new();
    let first = body(6);
    let second = body(7);
    harness.source.register(&first);
    harness.source.register(&second);
    *harness.source.snapshot_gate.lock().unwrap() = Some(Arc::new(SnapshotGate::new()));
    let service = harness.service();
    let outcomes = std::thread::scope(|scope| {
        let left = scope.spawn(|| service.sign(&payload(&first)));
        let right = scope.spawn(|| service.sign(&payload(&second)));
        [left.join().unwrap(), right.join().unwrap()]
    });
    assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|result| matches!(
                result,
                Err(SignerStreamTokenErrorV1::Operation(
                    SignerOperationErrorV1::ReservationConflict
                ))
            ))
            .count(),
        1
    );
    assert_eq!(harness.source.counts().signing, 2);
    assert_eq!(harness.source.counts().reserves, 2);
    assert_eq!(harness.calls(), 4);
    assert_eq!(harness.source.base.state.lock().unwrap().commits, 1);
    assert_eq!(harness.source.staged_checked.load(Ordering::SeqCst), 1);
}

#[test]
fn stream_payload_admission_rejects_domain_body_provider_and_key_before_source_reads() {
    let harness = Harness::new();
    let body = body(8);
    let good = payload(&body);
    let before = harness.source.counts();
    let mut cases = vec![
        Vec::new(),
        b"not a stream token".to_vec(),
        norito::encode_canonical(&body).unwrap(),
    ];
    let mut wrong_domain = good.clone();
    wrong_domain[0] ^= 1;
    cases.push(wrong_domain);
    let mut suffix = good.clone();
    suffix.push(0);
    cases.push(suffix);
    let mut truncated = good.clone();
    truncated.pop();
    cases.push(truncated);
    for index in 0..5 {
        let mut changed = body.clone();
        match index {
            0 => changed.provider_id = [0x63; 32],
            1 => changed.token_pk_version = 8,
            2 => changed.token_id = "invalid-token-id".into(),
            3 => changed.max_streams = 0,
            _ => changed.profile_handle = "x".repeat(129),
        }
        cases.push(payload(&changed));
    }
    for bytes in cases {
        assert!(matches!(
            harness.service().sign(&bytes),
            Err(SignerStreamTokenErrorV1::Receipt(_))
        ));
        assert!(matches!(
            harness.service().recover(&bytes),
            Err(SignerStreamTokenErrorV1::Receipt(_))
        ));
        assert_eq!(harness.source.counts(), before);
        assert_eq!(harness.calls(), 0);
    }
    let mut alternates = 0;
    for flags in layouts() {
        let encoded = {
            let _guard = norito::core::DecodeFlagsGuard::enter(flags);
            norito::core::to_bytes(&body).unwrap()
        };
        assert_eq!(
            norito::decode_from_bytes::<StreamTokenBodyV1>(&encoded).unwrap(),
            body
        );
        if encoded == norito::encode_canonical(&body).unwrap() {
            continue;
        }
        alternates += 1;
        let mut bytes = b"sorafs.stream-token.signature.v1\0".to_vec();
        bytes.extend(encoded);
        assert!(matches!(
            harness.service().sign(&bytes),
            Err(SignerStreamTokenErrorV1::Receipt(_))
        ));
        assert_eq!(harness.source.counts(), before);
    }
    assert!(
        alternates > 0,
        "a genuinely different ordinary-decodable frame was rejected"
    );
    assert_eq!(fs::read_dir(&harness.source.directory).unwrap().count(), 0);
    harness
        .sign(&body)
        .expect("positive control reaches source and four provider calls");
    assert_eq!(harness.calls(), 4);
}

#[test]
fn stream_phase_and_provider_failures_keep_exact_tombstones_and_never_release() {
    for fault in 0..7 {
        let harness = Harness::new();
        let body = body(9);
        let expected = harness.source.register(&body);
        {
            let mut state = harness.source.base.state.lock().unwrap();
            match fault {
                0 => {
                    state.fail_reserved_phase =
                        Some(SignerReservedObservationPhaseV1::BeforeProvider)
                }
                1 => *harness.provider.fault.lock().unwrap() = Some(ProviderFault::Unavailable),
                2 => {
                    state.fail_reserved_phase =
                        Some(SignerReservedObservationPhaseV1::AfterProvider)
                }
                3 => {
                    state.fail_reserved_phase = Some(SignerReservedObservationPhaseV1::BeforeCommit)
                }
                4 => state.fail_commit = true,
                5 => {
                    state.fail_completed_phase =
                        Some(SignerCommittedObservationPhaseV1::AfterCommit)
                }
                _ => {
                    state.fail_completed_phase =
                        Some(SignerCommittedObservationPhaseV1::BeforeRelease)
                }
            }
        }
        let error = harness.sign(&body).unwrap_err();
        let expected_error = match fault {
            1 => SignerOperationErrorV1::ProviderUnavailable,
            4 => SignerOperationErrorV1::ReservationConflict,
            _ => SignerOperationErrorV1::StateUnavailable,
        };
        assert_eq!(error, SignerStreamTokenErrorV1::Operation(expected_error));
        let calls = [0, 1, 1, 4, 4, 4, 4][fault];
        assert_eq!(harness.calls(), calls);
        let state = harness.source.base.state.lock().unwrap();
        assert_eq!(state.used_ids.len(), 1);
        assert!(state.used_ids.contains(&expected.operation_id()));
        assert_eq!(state.commits, usize::from(fault >= 5));
        if fault < 5 {
            assert!(state.reservation.is_some());
        } else {
            assert!(state.reservation.is_none());
        }
        drop(state);
        assert_eq!(
            harness.source.path(expected.operation_id()).exists(),
            fault >= 3
        );
        assert!(harness.service().recover(&payload(&body)).is_err());
        assert!(harness.sign(&body).is_err());
        assert_eq!(
            harness.calls(),
            calls,
            "neither failure path can retry the key"
        );
    }
}

#[test]
fn committed_stream_receipt_substitution_or_fresh_revocation_blocks_release_and_recovery() {
    for mutation in 0..4 {
        let harness = Harness::new();
        let body = body(10);
        match mutation {
            0 => *harness.source.substitute_after_commit.lock().unwrap() = true,
            1 => {
                harness
                    .source
                    .base
                    .state
                    .lock()
                    .unwrap()
                    .mutate_after_commit = Some(Mutation::SignerRevoked)
            }
            2 => {
                harness.source.base.state.lock().unwrap().mutate_at_release =
                    Some(Mutation::AttesterRevoked)
            }
            _ => {
                harness.source.base.state.lock().unwrap().mutate_at_release = Some(Mutation::Record)
            }
        }
        assert!(harness.sign(&body).is_err());
        assert_eq!(harness.source.base.state.lock().unwrap().commits, 1);
        assert_eq!(harness.source.staged_checked.load(Ordering::SeqCst), 1);
        assert!(harness.service().recover(&payload(&body)).is_err());
        assert_eq!(harness.calls(), 4);
    }
}

#[test]
fn stream_recovery_requires_exact_body_receipt_permissions_and_immutable_completion() {
    for mutation in 0..4 {
        let harness = Harness::new();
        let body = body(11);
        let receipt = harness.sign(&body).unwrap();
        let decoded: SignerStreamTokenReceiptV1 =
            norito::decode_canonical(receipt.bytes()).unwrap();
        assert_eq!(
            harness.service().recover(&payload(&body)).unwrap().bytes(),
            receipt.bytes()
        );
        let before = harness.source.counts();
        match mutation {
            0 => {
                let mut other = body.clone();
                other.rate_limit_bytes += 1;
                assert!(harness.service().recover(&payload(&other)).is_err());
            }
            1 => fs::set_permissions(
                harness.source.path(decoded.intent.operation_id),
                fs::Permissions::from_mode(0o444),
            )
            .unwrap(),
            2 => {
                harness
                    .source
                    .history
                    .lock()
                    .unwrap()
                    .get_mut(&decoded.intent.operation_id)
                    .unwrap()
                    .3[0] ^= 1;
            }
            _ => {
                harness
                    .source
                    .history
                    .lock()
                    .unwrap()
                    .remove(&decoded.intent.operation_id);
            }
        }
        if mutation != 0 {
            assert!(harness.service().recover(&payload(&body)).is_err());
        }
        let after = harness.source.counts();
        assert_eq!(
            (after.signing, after.reserves, after.commits),
            (before.signing, before.reserves, before.commits)
        );
        assert_eq!(harness.calls(), 4);
    }
}

#[test]
fn stream_constructor_rejects_wrong_journal_role_and_unavailable_fresh_custody() {
    for failure in 0..3 {
        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.path().canonicalize().unwrap();
        let fixture = if failure == 1 {
            fixture()
        } else {
            fixture_for(
                SignerRoleV1::StreamToken,
                SignerPurposeBindingV1::StreamToken {
                    provider_id: [0x62; 32],
                },
            )
        };
        if failure == 2 {
            fixture.source.state.lock().unwrap().fail_observe = true;
        }
        let purpose = if failure == 0 {
            SignerReceiptPurposeV1::ReleaseManifest
        } else {
            SignerReceiptPurposeV1::StreamToken
        };
        let error = SignerStreamTokenServiceV1::new(
            fixture.coordinator,
            SignerReceiptJournalV1::open(&path, purpose).unwrap(),
        )
        .unwrap_err();
        let expected = match failure {
            0 => SignerStreamTokenErrorV1::Journal,
            1 => SignerStreamTokenErrorV1::Receipt(SignerStreamTokenReceiptErrorV1::WrongPurpose),
            _ => SignerStreamTokenErrorV1::Operation(SignerOperationErrorV1::StateUnavailable),
        };
        assert_eq!(error, expected);
        assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 0);
        assert!(fixture.source.state.lock().unwrap().used_ids.is_empty());
        assert_eq!(fs::read_dir(path).unwrap().count(), 0);
    }
}

#[test]
fn stream_time_checks_use_fresh_custody_before_reserve_and_read_only_recovery() {
    let harness = Harness::with_custody_expiry(3000);
    let body = body(12);
    let receipt = harness.sign(&body).unwrap();
    assert_eq!(
        harness.service().recover(&payload(&body)).unwrap().bytes(),
        receipt.bytes()
    );
    let mut future = body.clone();
    future.token_id = hex::encode([13; 16]);
    future.issued_at = 2;
    future.ttl_epoch = 3;
    let before = harness.source.counts();
    assert_eq!(
        harness.sign(&future).unwrap_err(),
        SignerStreamTokenErrorV1::Receipt(SignerStreamTokenReceiptErrorV1::InvalidTime)
    );
    assert_eq!(harness.source.counts().reserves, before.reserves);
    assert_eq!(harness.calls(), 4);
    {
        let mut state = harness.source.base.state.lock().unwrap();
        state.context.now_unix_ms = 2000;
        state.context.anchor_observed_at_unix_ms = 2000;
        verify_signer_custody_use_v1(
            &harness.source.record,
            &harness.source.base.binding,
            &harness.source.trust,
            &state.context,
        )
        .expect("current independently signed custody remains valid at exact token expiry");
    }
    let before = harness.source.counts();
    assert_eq!(
        harness.service().recover(&payload(&body)).unwrap_err(),
        SignerStreamTokenErrorV1::Receipt(SignerStreamTokenReceiptErrorV1::TokenExpired)
    );
    let after = harness.source.counts();
    assert_eq!(
        (after.signing, after.reserves, after.commits),
        (before.signing, before.reserves, before.commits)
    );
    assert_eq!(harness.calls(), 4);
}

#[test]
fn same_key_custody_renewal_cannot_replay_or_relabel_a_stream_receipt() {
    let mut harness = Harness::new();
    let body = body(14);
    let receipt = harness.sign(&body).unwrap();
    let mut decoded: SignerStreamTokenReceiptV1 =
        norito::decode_canonical(receipt.bytes()).unwrap();
    let original_id = decoded.intent.operation_id;
    assert_eq!(
        harness.service().recover(&payload(&body)).unwrap().bytes(),
        receipt.bytes()
    );
    let (renewed_record, renewed_custody) = harness.renew_same_key();
    assert_ne!(renewed_custody, decoded.request.original_custody);
    let renewed: SignerCustodyRecordV1 = norito::decode_canonical(&renewed_record).unwrap();
    assert_eq!(renewed.statement.binding, harness.source.base.binding);
    assert_eq!(
        SignerStreamTokenExpectedV1::new(&body, &renewed.statement.binding)
            .unwrap()
            .operation_id(),
        original_id
    );
    harness.restart_with_record(renewed_record.clone());
    assert_eq!(
        harness.service().recover(&payload(&body)).unwrap_err(),
        SignerStreamTokenErrorV1::Receipt(SignerStreamTokenReceiptErrorV1::TokenMismatch)
    );
    assert!(matches!(
        harness.sign(&body),
        Err(SignerStreamTokenErrorV1::Operation(
            SignerOperationErrorV1::ReservationConflict
        ))
    ));
    assert_eq!(
        harness.calls(),
        5,
        "only the four original signatures and genuine terminal renewal audit were signed"
    );
    assert_eq!(
        harness.source.base.state.lock().unwrap().transition_commits,
        1
    );
    // Reopen an ordinary private canonical file after an adversary changes only public metadata.
    // The original immutable source row remains unchanged and no signature is regenerated.
    drop(harness.service.take());
    decoded.custody_record = renewed_record.clone();
    decoded.request.original_custody = renewed_custody;
    decoded.intent.request_digest = decoded.request.digest().unwrap();
    decoded.provenance.original_custody = renewed_custody;
    decoded.provenance.intent_digest = decoded.intent.digest().unwrap();
    let path = harness.source.path(original_id);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(&path, decoded.encode_canonical().unwrap()).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
    harness.restart_with_record(renewed_record);
    let before = harness.source.counts();
    assert!(matches!(
        harness.service().recover(&payload(&body)),
        Err(SignerStreamTokenErrorV1::Receipt(_))
    ));
    let after = harness.source.counts();
    assert_eq!(
        (after.signing, after.reserves, after.commits),
        (before.signing, before.reserves, before.commits)
    );
    assert_eq!(harness.calls(), 5);
}

#[path = "stream_token_transport.rs"]
mod transport;
