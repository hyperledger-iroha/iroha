//! Canonical threshold freshness, native nonce, interval, deployment and retention checks.

use super::*;
use crate::block::consensus_v2::HeightContextId;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};

fn network(byte: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([byte; 32])))
}

fn context(byte: u8) -> HeightContextId {
    HeightContextId(HashOf::from_untyped_unchecked(Hash::prehashed([byte; 32])))
}

fn fixture() -> (
    Vec<KeyPair>,
    KagemushaReleaseAuthorityPolicyV1,
    KagemushaMobileBootstrapCheckpointV1,
    KagemushaMobileBootstrapFreshnessPackageV1,
) {
    let mut keys = [51, 52, 53]
        .map(|byte| KeyPair::from_seed(vec![byte; 32], Algorithm::Ed25519))
        .to_vec();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let policy = KagemushaReleaseAuthorityPolicyV1 {
        version: 1,
        authority_set_id: [50; 32],
        threshold: 2,
        authorized_signers: keys.iter().map(|key| key.public_key().clone()).collect(),
    };
    let checkpoint = KagemushaMobileBootstrapCheckpointV1 {
        version: 1,
        authority_policy_digest: policy.canonical_digest().unwrap(),
        network_id: network(3),
        scope: KagemushaMobileBootstrapScopeV1 {
            asset_identity_digest: [4; 32],
            asset_incarnation: [5; 32],
            asset_scale: 2,
            liability_pool_id: [6; 32],
        },
        release_id: [7; 32],
        release_attestation_digest: [8; 32],
        first_context_id: context(9),
        sequence: 10,
        issued_at_ms: 1_000,
        expires_at_ms: 300_000,
    };
    let checkpoint_digest = checkpoint
        .validate_pins(&KagemushaMobileBootstrapPinsV1 {
            authority_policy: &policy,
            network_id: checkpoint.network_id,
            scope: checkpoint.scope,
            release_id: checkpoint.release_id,
            release_attestation_digest: checkpoint.release_attestation_digest,
            minimum_sequence: 1,
            previous: None,
            trusted_now_ms: 1_500,
        })
        .unwrap();
    let statement = KagemushaMobileBootstrapFreshnessStatementV1 {
        version: 1,
        request_nonce: [10; 32],
        authority_policy_digest: policy.canonical_digest().unwrap(),
        network_id: checkpoint.network_id,
        scope: checkpoint.scope,
        checkpoint_digest,
        retained_sequence: checkpoint.sequence,
        authority_time_lower_ms: 1_500,
        authority_time_upper_ms: 1_600,
    };
    let package = signed(statement, &keys[..2]);
    (keys, policy, checkpoint, package)
}

fn signed(
    statement: KagemushaMobileBootstrapFreshnessStatementV1,
    keys: &[KeyPair],
) -> KagemushaMobileBootstrapFreshnessPackageV1 {
    KagemushaMobileBootstrapFreshnessPackageV1 {
        statement,
        approvals: keys
            .iter()
            .map(|key| KagemushaMobileBootstrapFreshnessApprovalV1 {
                public_key: key.public_key().clone(),
                signature: SignatureOf::try_new(key.private_key(), &statement.approval_payload())
                    .unwrap(),
            })
            .collect(),
    }
}

fn pins<'a>(
    policy: &'a KagemushaReleaseAuthorityPolicyV1,
    checkpoint: &'a KagemushaMobileBootstrapCheckpointV1,
) -> KagemushaMobileBootstrapFreshnessPinsV1<'a> {
    KagemushaMobileBootstrapFreshnessPinsV1 {
        authority_policy: policy,
        network_id: network(3),
        scope: KagemushaMobileBootstrapScopeV1 {
            asset_identity_digest: [4; 32],
            asset_incarnation: [5; 32],
            asset_scale: 2,
            liability_pool_id: [6; 32],
        },
        release_id: [7; 32],
        release_attestation_digest: [8; 32],
        minimum_sequence: 1,
        previous: None,
        checkpoint,
        request_nonce: [10; 32],
        native_elapsed_ms: 250,
    }
}

#[test]
fn canonical_freshness_roundtrips_and_authenticates_explicit_interval_and_retention() {
    let (_, policy, checkpoint, package) = fixture();
    let archive = norito::encode_canonical(&package).unwrap();
    let decoded = KagemushaMobileBootstrapFreshnessPackageV1::decode_canonical_exact(&archive)
        .expect("canonical freshness");
    assert_eq!(decoded, package);
    let observed = decoded.authenticate(&pins(&policy, &checkpoint)).unwrap();
    assert_eq!(observed.trusted_time_lower_ms, 1_500);
    assert_eq!(observed.trusted_time_upper_ms, 1_850);
    assert_eq!(observed.replay_pin.sequence, checkpoint.sequence);
    assert_eq!(
        observed.replay_pin.checkpoint_digest,
        package.statement.checkpoint_digest
    );
    let statement: KagemushaMobileBootstrapFreshnessStatementV1 =
        norito::decode_canonical(&norito::encode_canonical(&package.statement).unwrap()).unwrap();
    assert_eq!(statement, package.statement);
    let approval: KagemushaMobileBootstrapFreshnessApprovalV1 =
        norito::decode_canonical(&norito::encode_canonical(&package.approvals[0]).unwrap())
            .unwrap();
    assert_eq!(approval, package.approvals[0]);
    approval.verify(&statement, &policy).unwrap();
}

#[test]
fn partial_freshness_approval_does_not_establish_threshold() {
    let (_, policy, checkpoint, mut package) = fixture();
    package.approvals.truncate(1);
    package.approvals[0]
        .verify(&package.statement, &policy)
        .unwrap();
    assert!(package.authenticate(&pins(&policy, &checkpoint)).is_err());
    let mut changed = package.statement;
    changed.authority_time_upper_ms += 1;
    assert!(package.approvals[0].verify(&changed, &policy).is_err());
}

#[test]
fn rejects_noncanonical_truncated_trailing_wrong_type_and_oversized_freshness() {
    let (_, _, _, package) = fixture();
    let valid = norito::encode_canonical(&package).unwrap();
    let mut trailing = valid.clone();
    trailing.push(0);
    for bytes in [
        Vec::new(),
        valid[..valid.len() - 1].to_vec(),
        trailing,
        vec![0; KAGEMUSHA_MOBILE_BOOTSTRAP_FRESHNESS_MAX_BYTES_V1 + 1],
        norito::encode_canonical(&package.statement).unwrap(),
    ] {
        assert!(
            KagemushaMobileBootstrapFreshnessPackageV1::decode_canonical_exact(&bytes).is_err()
        );
    }
}

#[test]
fn authenticates_every_field_of_freshness_statement() {
    let (_, policy, checkpoint, package) = fixture();
    for index in 0..13 {
        let mut changed = package.clone();
        match index {
            0 => changed.statement.version = 2,
            1 => changed.statement.request_nonce[0] ^= 1,
            2 => changed.statement.authority_policy_digest[0] ^= 1,
            3 => changed.statement.network_id = network(13),
            4 => changed.statement.scope.asset_identity_digest[0] ^= 1,
            5 => changed.statement.scope.asset_incarnation[0] ^= 1,
            6 => changed.statement.scope.asset_scale += 1,
            7 => changed.statement.scope.liability_pool_id[0] ^= 1,
            8 => changed.statement.checkpoint_digest[0] ^= 1,
            9 => changed.statement.retained_sequence += 1,
            10 => changed.statement.authority_time_lower_ms += 1,
            11 => changed.statement.authority_time_upper_ms += 1,
            _ => changed.statement.request_nonce = [0; 32],
        }
        assert!(
            changed.authenticate(&pins(&policy, &checkpoint)).is_err(),
            "field {index}"
        );
    }
}

#[test]
fn rejects_even_threshold_signed_wrong_nonce_and_retained_checkpoint() {
    let (keys, policy, checkpoint, package) = fixture();
    for index in 0..5 {
        let mut statement = package.statement;
        match index {
            0 => statement.version = 2,
            1 => statement.request_nonce = [11; 32],
            2 => statement.retained_sequence += 1,
            3 => statement.checkpoint_digest[0] ^= 1,
            _ => statement.retained_sequence = 0,
        }
        assert!(
            signed(statement, &keys[..2])
                .authenticate(&pins(&policy, &checkpoint))
                .is_err()
        );
    }
    let mut statement = package.statement;
    statement.request_nonce = [0; 32];
    let mut native_pins = pins(&policy, &checkpoint);
    native_pins.request_nonce = [0; 32];
    assert!(
        signed(statement, &keys[..2])
            .authenticate(&native_pins)
            .is_err()
    );
}

#[test]
fn rejects_substitution_of_every_independent_deployment_pin() {
    let (_, policy, checkpoint, package) = fixture();
    for index in 0..7 {
        let mut changed = pins(&policy, &checkpoint);
        match index {
            0 => changed.network_id = network(13),
            1 => changed.scope.asset_identity_digest = [13; 32],
            2 => changed.scope.asset_incarnation = [13; 32],
            3 => changed.scope.asset_scale += 1,
            4 => changed.scope.liability_pool_id = [13; 32],
            5 => changed.release_id = [13; 32],
            _ => changed.release_attestation_digest = [13; 32],
        }
        assert!(package.authenticate(&changed).is_err(), "pin {index}");
    }
}

#[test]
fn freshness_digest_binds_every_checkpoint_field() {
    let (_, policy, checkpoint, package) = fixture();
    for index in 0..14 {
        let mut changed = checkpoint;
        match index {
            0 => changed.version = 2,
            1 => changed.authority_policy_digest[0] ^= 1,
            2 => changed.network_id = network(13),
            3 => changed.scope.asset_identity_digest[0] ^= 1,
            4 => changed.scope.asset_incarnation[0] ^= 1,
            5 => changed.scope.asset_scale += 1,
            6 => changed.scope.liability_pool_id[0] ^= 1,
            7 => changed.release_id[0] ^= 1,
            8 => changed.release_attestation_digest[0] ^= 1,
            9 => changed.first_context_id = context(13),
            10 => changed.sequence += 1,
            11 => changed.issued_at_ms += 1,
            12 => changed.expires_at_ms += 1,
            _ => changed.first_context_id = context(0),
        }
        assert!(
            package.authenticate(&pins(&policy, &changed)).is_err(),
            "checkpoint field {index}"
        );
    }
}

#[test]
fn rejects_package_selected_keys_and_changed_or_malformed_policy() {
    let (_, policy, checkpoint, package) = fixture();
    let foreign = KeyPair::from_seed(vec![99; 32], Algorithm::Ed25519);
    let mut changed = package.clone();
    changed.approvals[0] = signed(changed.statement, &[foreign]).approvals.remove(0);
    changed
        .approvals
        .sort_by(|a, b| a.public_key.cmp(&b.public_key));
    assert!(changed.authenticate(&pins(&policy, &checkpoint)).is_err());
    for index in 0..3 {
        let mut changed_policy = policy.clone();
        match index {
            0 => changed_policy.authority_set_id[0] ^= 1,
            1 => changed_policy.threshold = 0,
            _ => changed_policy.authorized_signers.reverse(),
        }
        assert!(
            package
                .authenticate(&pins(&changed_policy, &checkpoint))
                .is_err()
        );
    }
    let mut malformed_policy = policy;
    malformed_policy.threshold = 0;
    let mut no_approvals = package;
    no_approvals.approvals.clear();
    assert!(
        no_approvals
            .authenticate(&pins(&malformed_policy, &checkpoint))
            .is_err()
    );
}

#[test]
fn rejects_duplicate_unordered_insufficient_excessive_and_invalid_freshness_approvals() {
    let (keys, policy, checkpoint, package) = fixture();
    for index in 0..5 {
        let mut changed = package.clone();
        match index {
            0 => changed.approvals[1] = changed.approvals[0].clone(),
            1 => changed.approvals.reverse(),
            2 => changed.approvals.truncate(1),
            3 => changed.approvals.extend(package.approvals.clone()),
            _ => {
                changed.approvals[0].signature = SignatureOf::try_new(
                    keys[2].private_key(),
                    &changed.statement.approval_payload(),
                )
                .unwrap();
            }
        }
        assert!(changed.authenticate(&pins(&policy, &checkpoint)).is_err());
    }
}

#[test]
fn rejects_freshness_signature_replay_from_another_domain() {
    let (keys, policy, checkpoint, mut package) = fixture();
    let mut payload = package.statement.approval_payload();
    payload.domain = "iroha:kagemusha:v1:mobile-bootstrap-approval".to_owned();
    for (approval, key) in package.approvals.iter_mut().zip(&keys) {
        approval.signature = SignatureOf::try_new(key.private_key(), &payload).unwrap();
    }
    assert!(package.authenticate(&pins(&policy, &checkpoint)).is_err());
}

#[test]
fn issuer_uncertainty_and_full_native_elapsed_must_fit_issuance_and_expiry() {
    let (keys, policy, checkpoint, package) = fixture();
    for (lower, upper, elapsed, accepted) in [
        (999, 1_500, 1, false),
        (1_000, 1_000, 0, true),
        (1_000, 299_999, 0, true),
        (1_000, 299_999, 1, false),
        (1_500, 299_749, 250, true),
        (1_500, 299_750, 250, false),
        (1_500, 300_000, 0, false),
        (1_500, 1_499, 0, false),
        (0, 1_500, 0, false),
    ] {
        let mut statement = package.statement;
        statement.authority_time_lower_ms = lower;
        statement.authority_time_upper_ms = upper;
        let mut native_pins = pins(&policy, &checkpoint);
        native_pins.native_elapsed_ms = elapsed;
        assert_eq!(
            signed(statement, &keys[..2])
                .authenticate(&native_pins)
                .is_ok(),
            accepted,
            "issuer interval [{lower},{upper}], native elapsed {elapsed}",
        );
    }
}

#[test]
fn rejects_time_overflow_and_bounds_native_attempt_duration() {
    let (keys, policy, checkpoint, package) = fixture();
    for elapsed in [0, KAGEMUSHA_MOBILE_BOOTSTRAP_FRESHNESS_MAX_ELAPSED_MS_V1] {
        let mut native_pins = pins(&policy, &checkpoint);
        native_pins.native_elapsed_ms = elapsed;
        assert_eq!(
            package
                .authenticate(&native_pins)
                .unwrap()
                .trusted_time_upper_ms,
            package.statement.authority_time_upper_ms + elapsed,
        );
    }
    for elapsed in [
        KAGEMUSHA_MOBILE_BOOTSTRAP_FRESHNESS_MAX_ELAPSED_MS_V1 + 1,
        u64::MAX,
    ] {
        let mut native_pins = pins(&policy, &checkpoint);
        native_pins.native_elapsed_ms = elapsed;
        assert!(package.authenticate(&native_pins).is_err());
    }
    let mut statement = package.statement;
    statement.authority_time_upper_ms = u64::MAX;
    assert!(
        signed(statement, &keys[..2])
            .authenticate(&pins(&policy, &checkpoint))
            .is_err()
    );
}

#[test]
fn retains_exact_retry_but_rejects_sequence_regression_and_equivocation() {
    let (_, policy, checkpoint, package) = fixture();
    let accepted = package
        .authenticate(&pins(&policy, &checkpoint))
        .unwrap()
        .replay_pin;
    let mut native_pins = pins(&policy, &checkpoint);
    native_pins.previous = Some(accepted);
    assert_eq!(
        package.authenticate(&native_pins).unwrap().replay_pin,
        accepted
    );
    for previous in [
        KagemushaMobileBootstrapReplayPinV1 {
            sequence: accepted.sequence + 1,
            ..accepted
        },
        KagemushaMobileBootstrapReplayPinV1 {
            checkpoint_digest: [19; 32],
            ..accepted
        },
        KagemushaMobileBootstrapReplayPinV1 {
            sequence: 0,
            ..accepted
        },
        KagemushaMobileBootstrapReplayPinV1 {
            checkpoint_digest: [0; 32],
            ..accepted
        },
    ] {
        native_pins.previous = Some(previous);
        assert!(package.authenticate(&native_pins).is_err());
    }
    native_pins.previous = None;
    for (floor, succeeds) in [
        (0, false),
        (checkpoint.sequence, true),
        (checkpoint.sequence + 1, false),
    ] {
        native_pins.minimum_sequence = floor;
        assert_eq!(package.authenticate(&native_pins).is_ok(), succeeds);
    }
}
