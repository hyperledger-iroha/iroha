//! Native nonce, time and exact-authority-retention controls for online startup freshness.

use std::time::Duration;

use iroha_crypto::{Algorithm, KeyPair, SignatureOf};
use iroha_data_model::kagemusha::{
    KagemushaMobileBootstrapApprovalV1, KagemushaMobileBootstrapFreshnessApprovalV1,
    KagemushaMobileBootstrapFreshnessStatementV1, KagemushaMobileBootstrapPinsV1,
};

use super::*;

fn keys() -> Vec<KeyPair> {
    let mut keys: Vec<_> = [41, 42, 43]
        .into_iter()
        .map(|byte| KeyPair::from_seed(vec![byte; 32], Algorithm::Ed25519))
        .collect();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    keys
}

fn bootstrap() -> (KagemushaTestnetNativeStartupContextV1, Vec<u8>) {
    let context = crate::kagemusha_testnet_native_startup_v1::startup_test_context_v1();
    let checkpoint =
        *crate::kagemusha_mobile_bootstrap_v1::verified_test_bootstrap_v1().checkpoint();
    let package = KagemushaMobileBootstrapPackageV1 {
        checkpoint,
        approvals: keys()[..2]
            .iter()
            .map(|key| KagemushaMobileBootstrapApprovalV1 {
                public_key: key.public_key().clone(),
                signature: SignatureOf::try_new(key.private_key(), &checkpoint.approval_payload())
                    .unwrap(),
            })
            .collect(),
    };
    (context, norito::encode_canonical(&package).unwrap())
}

fn attempt() -> KagemushaNativeBootstrapFreshnessAttemptV1 {
    let (context, archive) = bootstrap();
    KagemushaNativeBootstrapFreshnessAttemptV1::begin(&context, &archive, 1, None).unwrap()
}

fn reply(
    attempt: &KagemushaNativeBootstrapFreshnessAttemptV1,
) -> KagemushaMobileBootstrapFreshnessPackageV1 {
    let checkpoint = attempt.checkpoint();
    let digest = checkpoint
        .validate_pins(&KagemushaMobileBootstrapPinsV1 {
            authority_policy: &attempt.pins.policy,
            network_id: attempt.pins.network_id,
            scope: attempt.pins.scope,
            release_id: attempt.pins.release_id,
            release_attestation_digest: attempt.pins.release_attestation_digest,
            minimum_sequence: 1,
            previous: None,
            trusted_now_ms: 1_500,
        })
        .unwrap();
    let statement = KagemushaMobileBootstrapFreshnessStatementV1 {
        version: 1,
        request_nonce: attempt.request_nonce(),
        authority_policy_digest: attempt.pins.policy.canonical_digest().unwrap(),
        network_id: attempt.pins.network_id,
        scope: attempt.pins.scope,
        checkpoint_digest: digest,
        retained_sequence: checkpoint.sequence,
        authority_time_lower_ms: 1_500,
        authority_time_upper_ms: 1_501,
    };
    sign(statement)
}

fn sign(
    statement: KagemushaMobileBootstrapFreshnessStatementV1,
) -> KagemushaMobileBootstrapFreshnessPackageV1 {
    KagemushaMobileBootstrapFreshnessPackageV1 {
        approvals: keys()[..2]
            .iter()
            .map(|key| KagemushaMobileBootstrapFreshnessApprovalV1 {
                public_key: key.public_key().clone(),
                signature: SignatureOf::try_new(key.private_key(), &statement.approval_payload())
                    .unwrap(),
            })
            .collect(),
        statement,
    }
}

fn encode(reply: &KagemushaMobileBootstrapFreshnessPackageV1) -> Vec<u8> {
    norito::encode_canonical(reply).unwrap()
}

#[test]
fn mobile_bootstrap_online_nonce_is_native_unique_and_reply_replay_is_rejected() {
    let first = attempt();
    let second = attempt();
    assert_ne!(first.request_nonce(), [0; 32]);
    assert_ne!(first.request_nonce(), second.request_nonce());
    assert_eq!(first.checkpoint(), second.checkpoint());
    let bytes = encode(&reply(&first));
    assert!(second.complete(&bytes).is_err());
    assert!(first.complete(&bytes).is_ok());
}

#[test]
fn mobile_bootstrap_online_retains_only_exact_authority_signed_checkpoint() {
    let attempt = attempt();
    let reply = reply(&attempt);
    let expected = KagemushaMobileBootstrapReplayPinV1 {
        sequence: reply.statement.retained_sequence,
        checkpoint_digest: reply.statement.checkpoint_digest,
    };
    let provider = attempt.complete(&encode(&reply)).unwrap();
    let observed = provider.read_freshness().unwrap();
    assert_eq!(observed.previous, Some(expected));
    assert_eq!(observed.minimum_sequence, expected.sequence);
    assert!(observed.trusted_now_ms >= reply.statement.authority_time_upper_ms);
    assert!(observed.trusted_now_ms < provider.expires_at_ms);
    provider.retain_verified_bootstrap(expected).unwrap();
    for pin in [
        KagemushaMobileBootstrapReplayPinV1 {
            sequence: expected.sequence - 1,
            ..expected
        },
        KagemushaMobileBootstrapReplayPinV1 {
            sequence: expected.sequence + 1,
            ..expected
        },
        KagemushaMobileBootstrapReplayPinV1 {
            checkpoint_digest: [91; 32],
            ..expected
        },
    ] {
        assert!(provider.retain_verified_bootstrap(pin).is_err());
    }
    assert_eq!(provider.read_freshness().unwrap().previous, Some(expected));
}

#[test]
fn mobile_bootstrap_online_requires_original_bootstrap_approvals() {
    let mut attempt = attempt();
    let bytes = encode(&reply(&attempt));
    attempt.package.approvals.clear();
    assert!(attempt.complete(&bytes).is_err());
}

#[test]
fn mobile_bootstrap_online_rejects_invalid_reply_and_signed_checkpoint_substitution() {
    for mutation in 0..4 {
        let attempt = attempt();
        let mut response = reply(&attempt);
        match mutation {
            0 => response.approvals.clear(),
            1 => response.statement.authority_time_upper_ms += 1,
            2 => {
                // Even the trusted threshold cannot substitute another checkpoint for the
                // one retained by this particular native attempt and its unchanged nonce.
                response.statement.checkpoint_digest[0] ^= 1;
                response = sign(response.statement);
            }
            _ => {
                response.statement.retained_sequence += 1;
                response = sign(response.statement);
            }
        }
        assert!(
            attempt.complete(&encode(&response)).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn mobile_bootstrap_online_preserves_independent_native_release_and_sequence_floors() {
    for mutation in 0..3 {
        let mut attempt = attempt();
        let response = reply(&attempt);
        match mutation {
            0 => attempt.pins.minimum_sequence = attempt.checkpoint().sequence + 1,
            1 => {
                attempt.pins.previous = Some(KagemushaMobileBootstrapReplayPinV1 {
                    sequence: attempt.checkpoint().sequence + 1,
                    checkpoint_digest: [93; 32],
                });
            }
            _ => attempt.pins.release_id = [92; 32],
        }
        assert!(
            attempt.complete(&encode(&response)).is_err(),
            "native pin {mutation}"
        );
    }
}

#[test]
fn mobile_bootstrap_online_rejects_expired_attempt_before_accepting_reply() {
    let mut attempt = attempt();
    let bytes = encode(&reply(&attempt));
    attempt.deadline = NativeDeadlineV1::expired_for_test();
    assert!(attempt.complete(&bytes).is_err());
}

#[test]
fn mobile_bootstrap_online_charges_time_before_response_validation() {
    let mut attempt = attempt();
    let mut statement = reply(&attempt).statement;
    statement.authority_time_upper_ms = attempt.checkpoint().expires_at_ms - 500;
    let bytes = encode(&sign(statement));
    attempt.started = NativeContinuousInstantV1::before_for_test(Duration::from_secs(1));
    attempt.deadline = NativeDeadlineV1::from_reading(attempt.started, MAX_LIFETIME).unwrap();
    assert!(attempt.complete(&bytes).is_err());
}

#[test]
fn mobile_bootstrap_online_read_and_retention_recheck_original_clock_and_expiry() {
    let attempt = attempt();
    let bytes = encode(&reply(&attempt));
    let mut provider = attempt.complete(&bytes).unwrap();
    let expected = provider.pin;
    provider.expires_at_ms = provider.authority_time_upper_ms;
    assert!(provider.read_freshness().is_err());
    assert!(provider.retain_verified_bootstrap(expected).is_err());
    provider.expires_at_ms = u64::MAX;
    provider.authority_time_upper_ms = u64::MAX;
    assert!(provider.read_freshness().is_err());
    provider.authority_time_upper_ms = 1_501;
    provider.deadline = NativeDeadlineV1::expired_for_test();
    assert!(provider.read_freshness().is_err());
    assert!(provider.retain_verified_bootstrap(expected).is_err());
}

#[test]
fn mobile_bootstrap_online_rejects_bad_native_floor_and_malformed_archive() {
    let (context, archive) = bootstrap();
    assert!(
        KagemushaNativeBootstrapFreshnessAttemptV1::begin(&context, &archive, 0, None).is_err()
    );
    for pin in [
        KagemushaMobileBootstrapReplayPinV1 {
            sequence: 0,
            checkpoint_digest: [1; 32],
        },
        KagemushaMobileBootstrapReplayPinV1 {
            sequence: 1,
            checkpoint_digest: [0; 32],
        },
    ] {
        assert!(
            KagemushaNativeBootstrapFreshnessAttemptV1::begin(&context, &archive, 1, Some(pin))
                .is_err()
        );
    }
    assert!(KagemushaNativeBootstrapFreshnessAttemptV1::begin(&context, &[], 1, None).is_err());
    let mut trailing = archive;
    trailing.push(0);
    assert!(
        KagemushaNativeBootstrapFreshnessAttemptV1::begin(&context, &trailing, 1, None).is_err()
    );
}
