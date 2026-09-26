//! Adversarial checks for independently pinned, authority-signed mobile bootstrap.

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, SignatureOf};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ASSET_SCALE_MAX_V1, KagemushaMobileBootstrapApprovalV1,
};

use super::*;

fn network(byte: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([byte; 32])))
}

fn context(byte: u8) -> HeightContextId {
    HeightContextId(HashOf::from_untyped_unchecked(Hash::prehashed([byte; 32])))
}

fn fixture() -> (
    Vec<KeyPair>,
    KagemushaReleaseAuthorityPolicyV1,
    KagemushaMobileBootstrapPackageV1,
) {
    let mut keys = [41, 42, 43]
        .map(|byte| KeyPair::from_seed(vec![byte; 32], Algorithm::Ed25519))
        .to_vec();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let policy = KagemushaReleaseAuthorityPolicyV1 {
        version: 1,
        authority_set_id: [40; 32],
        threshold: 2,
        authorized_signers: keys.iter().map(|key| key.public_key().clone()).collect(),
    };
    let checkpoint = KagemushaMobileBootstrapCheckpointV1 {
        version: 1,
        authority_policy_digest: policy.canonical_digest().expect("policy digest"),
        network_id: network(3),
        scope: KagemushaMobileBootstrapScopeV1 {
            asset_identity_digest: [4; 32],
            asset_incarnation: [5; 32],
            asset_scale: 2,
            liability_pool_id: [6; 32],
        },
        release_id: [7; 32],
        release_attestation_digest: [8; 32],
        first_context_id: context(5),
        sequence: 10,
        issued_at_ms: 1_000,
        expires_at_ms: 300_000,
    };
    let package = signed(checkpoint, &keys[..2]);
    (keys, policy, package)
}

fn signed(
    checkpoint: KagemushaMobileBootstrapCheckpointV1,
    keys: &[KeyPair],
) -> KagemushaMobileBootstrapPackageV1 {
    KagemushaMobileBootstrapPackageV1 {
        checkpoint,
        approvals: keys
            .iter()
            .map(|key| KagemushaMobileBootstrapApprovalV1 {
                public_key: key.public_key().clone(),
                signature: SignatureOf::try_new(key.private_key(), &checkpoint.approval_payload())
                    .expect("checkpoint signature"),
            })
            .collect(),
    }
}

fn pins(policy: &KagemushaReleaseAuthorityPolicyV1) -> KagemushaMobileBootstrapPinsV1<'_> {
    KagemushaMobileBootstrapPinsV1 {
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
        trusted_now_ms: 1_500,
    }
}

fn archive(package: &KagemushaMobileBootstrapPackageV1) -> Vec<u8> {
    norito::encode_canonical(package).expect("canonical bootstrap")
}

pub(super) fn verified_fixture() -> KagemushaVerifiedMobileBootstrapV1 {
    let (_, policy, package) = fixture();
    verify_kagemusha_mobile_bootstrap_v1(&archive(&package), || Ok(pins(&policy)))
        .expect("authenticated native test bootstrap")
}

#[test]
fn accepts_exact_threshold_package_and_exposes_only_verified_pins() {
    let (_, policy, package) = fixture();
    let verified = verified_fixture();
    assert_eq!(*verified.checkpoint(), package.checkpoint);
    assert_eq!(verified.network_id(), network(3));
    assert_eq!(verified.first_context_id(), context(5));
    assert_eq!(verified.scope(), package.checkpoint.scope);
    assert_eq!(verified.release_id(), [7; 32]);
    assert_eq!(verified.release_attestation_digest(), [8; 32]);
    assert_eq!(verified.trusted_authority_policy(), &policy);
    assert_eq!(verified.replay_pin().sequence, 10);
    assert_ne!(verified.replay_pin().checkpoint_digest, [0; 32]);
    verified
        .require_unexpired()
        .expect("fresh installation lease");
}

#[test]
fn expired_installation_token_requires_fresh_verification() {
    let expired = expired_test_bootstrap_v1();
    assert!(expired.require_unexpired().is_err());
    let current = verified_fixture();
    current
        .require_unexpired()
        .expect("independently reverified token");
    // Inspecting or retaining the old token does not renew its deadline.
    assert_eq!(expired.checkpoint(), current.checkpoint());
    assert!(expired.require_unexpired().is_err());
}

#[test]
fn freshness_read_delay_consumes_the_packages_remaining_lifetime() {
    let (_, policy, package) = fixture();
    let bytes = archive(&package);
    let mut current_pins = pins(&policy);
    current_pins.trusted_now_ms = package.checkpoint.expires_at_ms - 500;
    verify_kagemusha_mobile_bootstrap_v1(&bytes, || Ok(current_pins))
        .expect("the signed package is still valid at the trusted UTC snapshot");

    // Simulate a one-second suspension across the synchronous freshness read. The UTC
    // snapshot is otherwise valid, but its remaining half-second must not restart afterward.
    let started = NativeContinuousInstantV1::before_for_test(Duration::from_secs(1));
    let read = std::cell::Cell::new(false);
    let result = verify_from_reading(&bytes, started, || {
        read.set(true);
        Ok(current_pins)
    });
    assert!(read.get());
    assert_eq!(
        result.err(),
        Some("KAGEMUSHA mobile bootstrap installation lease expired or invalid".to_owned())
    );
}

#[test]
fn freshness_read_failure_is_returned_and_oversized_input_never_reads_pins() {
    let (_, policy, package) = fixture();
    let bytes = archive(&package);
    assert_eq!(
        verify_kagemusha_mobile_bootstrap_v1(&bytes, || Err("freshness unavailable".to_owned()))
            .err(),
        Some("freshness unavailable".to_owned())
    );
    assert!(
        verify_kagemusha_mobile_bootstrap_v1(
            &vec![0; KAGEMUSHA_MOBILE_BOOTSTRAP_MAX_BYTES_V1 + 1],
            || {
                let _ = pins(&policy);
                panic!("oversized archive must not invoke the freshness authority")
            }
        )
        .is_err()
    );
}

#[test]
fn rejects_noncanonical_truncated_trailing_and_oversized_archives() {
    let (_, policy, package) = fixture();
    let valid = archive(&package);
    let mut trailing = valid.clone();
    trailing.push(0);
    for bytes in [
        Vec::new(),
        valid[..valid.len() - 1].to_vec(),
        trailing,
        vec![0; KAGEMUSHA_MOBILE_BOOTSTRAP_MAX_BYTES_V1 + 1],
        norito::encode_canonical(&package.checkpoint).expect("wrong archive type"),
    ] {
        assert!(verify_kagemusha_mobile_bootstrap_v1(&bytes, || Ok(pins(&policy))).is_err());
    }
}

#[test]
fn rejects_native_network_scope_and_release_substitution() {
    let (_, policy, package) = fixture();
    let bytes = archive(&package);
    for index in 0..7 {
        let mut changed = pins(&policy);
        match index {
            0 => changed.network_id = network(9),
            1 => changed.scope.asset_identity_digest = [9; 32],
            2 => changed.scope.asset_incarnation = [9; 32],
            3 => changed.scope.asset_scale = 3,
            4 => changed.scope.liability_pool_id = [9; 32],
            5 => changed.release_id = [9; 32],
            _ => changed.release_attestation_digest = [9; 32],
        }
        assert!(verify_kagemusha_mobile_bootstrap_v1(&bytes, || Ok(changed)).is_err());
    }
}

#[test]
fn authenticates_first_context_and_freshness_fields_in_signature() {
    let (_, policy, package) = fixture();
    for index in 0..4 {
        let mut changed = package.clone();
        match index {
            0 => changed.checkpoint.first_context_id = context(9),
            1 => changed.checkpoint.sequence += 1,
            2 => changed.checkpoint.issued_at_ms += 1,
            _ => changed.checkpoint.expires_at_ms += 1,
        }
        assert!(
            verify_kagemusha_mobile_bootstrap_v1(&archive(&changed), || Ok(pins(&policy))).is_err()
        );
    }
}

#[test]
fn rejects_even_threshold_signed_invalid_context_scope_version_and_freshness() {
    let (keys, policy, package) = fixture();
    for index in 0..8 {
        let mut checkpoint = package.checkpoint;
        match index {
            0 => checkpoint.first_context_id = context(0),
            1 => checkpoint.scope.asset_identity_digest = [0; 32],
            2 => checkpoint.scope.asset_scale = KAGEMUSHA_ASSET_SCALE_MAX_V1 + 1,
            3 => checkpoint.scope.liability_pool_id = checkpoint.scope.asset_identity_digest,
            4 => checkpoint.version = 2,
            5 => checkpoint.sequence = 0,
            6 => checkpoint.issued_at_ms = 0,
            _ => checkpoint.expires_at_ms = checkpoint.issued_at_ms,
        }
        let changed = signed(checkpoint, &keys[..2]);
        let mut changed_pins = pins(&policy);
        changed_pins.scope = checkpoint.scope;
        assert!(
            verify_kagemusha_mobile_bootstrap_v1(&archive(&changed), || Ok(changed_pins)).is_err()
        );
    }
}

#[test]
fn rejects_package_selected_key_and_changed_authority_policy() {
    let (_, policy, package) = fixture();
    let foreign = KeyPair::from_seed(vec![99; 32], Algorithm::Ed25519);
    let mut changed = package.clone();
    changed.approvals[0] = signed(changed.checkpoint, &[foreign]).approvals.remove(0);
    changed
        .approvals
        .sort_by(|a, b| a.public_key.cmp(&b.public_key));
    assert!(
        verify_kagemusha_mobile_bootstrap_v1(&archive(&changed), || Ok(pins(&policy))).is_err()
    );
    let mut changed_policy = policy.clone();
    changed_policy.authority_set_id = [99; 32];
    assert!(
        verify_kagemusha_mobile_bootstrap_v1(&archive(&package), || Ok(pins(&changed_policy)))
            .is_err()
    );
    changed_policy.threshold = 0;
    assert!(
        verify_kagemusha_mobile_bootstrap_v1(&archive(&package), || Ok(pins(&changed_policy)))
            .is_err()
    );
}

#[test]
fn rejects_duplicate_unordered_insufficient_and_invalid_approvals() {
    let (keys, policy, package) = fixture();
    for index in 0..4 {
        let mut changed = package.clone();
        match index {
            0 => changed.approvals[1] = changed.approvals[0].clone(),
            1 => changed.approvals.reverse(),
            2 => changed.approvals.truncate(1),
            _ => {
                changed.approvals[0].signature = SignatureOf::try_new(
                    keys[2].private_key(),
                    &changed.checkpoint.approval_payload(),
                )
                .expect("wrong signing key");
            }
        }
        assert!(
            verify_kagemusha_mobile_bootstrap_v1(&archive(&changed), || Ok(pins(&policy))).is_err()
        );
    }
}

#[test]
fn rejects_cross_domain_signature_replay() {
    let (keys, policy, mut package) = fixture();
    let mut payload = package.checkpoint.approval_payload();
    payload.domain = "iroha:kagemusha:v1:release-approval".to_owned();
    for (approval, key) in package.approvals.iter_mut().zip(&keys) {
        approval.signature =
            SignatureOf::try_new(key.private_key(), &payload).expect("other domain");
    }
    assert!(
        verify_kagemusha_mobile_bootstrap_v1(&archive(&package), || Ok(pins(&policy))).is_err()
    );
}

#[test]
fn enforces_inclusive_issuance_exclusive_expiry_and_native_sequence_floor() {
    let (_, policy, package) = fixture();
    let bytes = archive(&package);
    for (time, accepted) in [
        (999, false),
        (1_000, true),
        (299_000, true),
        (300_000, false),
    ] {
        let mut changed = pins(&policy);
        changed.trusted_now_ms = time;
        assert_eq!(
            verify_kagemusha_mobile_bootstrap_v1(&bytes, || Ok(changed)).is_ok(),
            accepted
        );
    }
    for (floor, accepted) in [(0, false), (10, true), (11, false)] {
        let mut changed = pins(&policy);
        changed.minimum_sequence = floor;
        assert_eq!(
            verify_kagemusha_mobile_bootstrap_v1(&bytes, || Ok(changed)).is_ok(),
            accepted
        );
    }
}

#[test]
fn allows_exact_retry_but_rejects_rollback_and_same_sequence_equivocation() {
    let (keys, policy, package) = fixture();
    let accepted = verify_kagemusha_mobile_bootstrap_v1(&archive(&package), || Ok(pins(&policy)))
        .expect("first accepted package")
        .replay_pin();
    for (sequence, context_byte, succeeds) in
        [(10, 5, true), (9, 5, false), (10, 9, false), (11, 9, true)]
    {
        let mut checkpoint = package.checkpoint;
        checkpoint.sequence = sequence;
        checkpoint.first_context_id = context(context_byte);
        let changed = signed(checkpoint, &keys[..2]);
        let mut changed_pins = pins(&policy);
        changed_pins.previous = Some(accepted);
        assert_eq!(
            verify_kagemusha_mobile_bootstrap_v1(&archive(&changed), || Ok(changed_pins)).is_ok(),
            succeeds
        );
    }
    for previous in [
        KagemushaMobileBootstrapReplayPinV1 {
            sequence: 0,
            ..accepted
        },
        KagemushaMobileBootstrapReplayPinV1 {
            checkpoint_digest: [0; 32],
            ..accepted
        },
    ] {
        let mut changed_pins = pins(&policy);
        changed_pins.previous = Some(previous);
        assert!(
            verify_kagemusha_mobile_bootstrap_v1(&archive(&package), || Ok(changed_pins)).is_err()
        );
    }
}
