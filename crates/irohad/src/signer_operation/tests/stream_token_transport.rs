//! Real producer/journal simulations through the complete-receipt transport trait.
use super::*;
use iroha_torii::sorafs::{
    StreamTokenHardwareCallErrorV1 as Error, StreamTokenHardwareClientV1 as Client,
};

#[test]
fn hardware_transport_uses_exact_body_and_recovers_identical_bytes_without_key_use() {
    let harness = Harness::new();
    let body = body(30);
    let expected = harness.source.register(&body);
    assert_eq!(
        Client::handle(harness.service()),
        harness.source.base.binding.runtime_handle
    );
    let receipt = Client::sign(harness.service(), &expected, &body).unwrap();
    let decoded = SignerStreamTokenReceiptV1::decode_canonical(receipt.bytes()).unwrap();
    assert_eq!(decoded.intent.operation_id, expected.operation_id());
    assert_eq!(decoded.signatures.len(), 4);
    assert_eq!(harness.calls(), 4);
    assert_eq!(harness.source.staged_checked.load(Ordering::SeqCst), 1);
    let before = harness.source.counts();
    let recovered = Client::recover(harness.service(), &expected, &body).unwrap();
    let after = harness.source.counts();
    assert_eq!(recovered.bytes(), receipt.bytes());
    assert_eq!(
        (after.signing, after.reserves, after.commits),
        (before.signing, before.reserves, before.commits)
    );
    assert_eq!(harness.calls(), 4);
}

#[test]
fn transport_rejects_changed_expected_body_provider_and_key_before_source_or_hardware() {
    let harness = Harness::new();
    let body = body(31);
    let expected = harness.source.register(&body);
    let before = harness.source.counts();
    for fault in 0..5 {
        let mut changed = body.clone();
        match fault {
            0 => changed.token_id = hex::encode([32; 16]),
            1 => changed.provider_id[0] ^= 1,
            2 => changed.token_pk_version += 1,
            3 => changed.profile_handle = "x".repeat(129),
            _ => changed.max_streams = 0,
        }
        assert_eq!(
            Client::sign(harness.service(), &expected, &changed).unwrap_err(),
            Error::Refused
        );
        assert_eq!(
            Client::recover(harness.service(), &expected, &changed).unwrap_err(),
            Error::Refused
        );
        assert_eq!(harness.source.counts(), before);
        assert_eq!(harness.calls(), 0);
    }
}

#[test]
fn signing_errors_after_invocation_are_ambiguous_and_read_only_recovery_keeps_original_commit() {
    for fault in 0..4 {
        let harness = Harness::new();
        let body = body(33);
        let expected = harness.source.register(&body);
        {
            let mut state = harness.source.base.state.lock().unwrap();
            match fault {
                0 => state.fail_observe = true,
                1 => *harness.provider.fault.lock().unwrap() = Some(ProviderFault::Unavailable),
                2 => {
                    state.fail_completed_phase =
                        Some(SignerCommittedObservationPhaseV1::AfterCommit)
                }
                _ => *harness.source.substitute_after_commit.lock().unwrap() = true,
            }
        }
        assert_eq!(
            Client::sign(harness.service(), &expected, &body).unwrap_err(),
            Error::AmbiguousCompletion
        );
        let before = harness.source.counts();
        let calls = harness.calls();
        {
            let mut state = harness.source.base.state.lock().unwrap();
            state.fail_observe = false;
            state.fail_completed_phase = None;
        }
        let result = Client::recover(harness.service(), &expected, &body);
        if fault == 2 {
            let recovered = result.unwrap();
            assert_eq!(
                SignerStreamTokenReceiptV1::decode_canonical(recovered.bytes())
                    .unwrap()
                    .intent
                    .operation_id,
                expected.operation_id()
            );
        } else {
            assert!(result.is_err());
        }
        let after = harness.source.counts();
        assert_eq!(
            (after.signing, after.reserves, after.commits),
            (before.signing, before.reserves, before.commits)
        );
        assert_eq!(harness.calls(), calls);
    }
}

include!("stream_token_window.rs");
