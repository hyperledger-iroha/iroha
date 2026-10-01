//! The ordinary State seam accepts one exact paired helper family and rejects role mixing.

use super::{KagemushaRecursionArtifactsV1, KagemushaRecursionErrorV1};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1, KagemushaArtifactRoleV1 as Role,
};

fn ordinary() -> KagemushaRecursionArtifactsV1 {
    let mut artifacts = super::tests::artifacts();
    artifacts.guard_bundle_verifying_key_eq.role = Role::OrdinaryAppGuardVkEq;
    artifacts.guard_bundle_verifying_key_ep.role = Role::OrdinaryAppGuardVkEp;
    artifacts
}

#[test]
fn ordinary_state_artifact_seam_rejects_mixed_or_reversed_helper_families() {
    let selected = ordinary();
    assert!(selected.validate().is_ok());
    // The experimental OEM family has its own seam; neither half can replace an
    // ordinary half merely because the public columns have the same width.
    for (eq, ep) in [
        (Role::OrdinaryAppGuardVkEq, Role::GuardBundleVkEp),
        (Role::GuardBundleVkEq, Role::OrdinaryAppGuardVkEp),
        (Role::OrdinaryAppGuardVkEp, Role::OrdinaryAppGuardVkEq),
        (Role::OrdinaryAppGuardPkEq, Role::OrdinaryAppGuardVkEp),
        (Role::OrdinaryAppGuardVkEq, Role::OrdinaryAppGuardPkEp),
    ] {
        let mut substituted = selected;
        substituted.guard_bundle_verifying_key_eq.role = eq;
        substituted.guard_bundle_verifying_key_ep.role = ep;
        assert_eq!(
            substituted.validate(),
            Err(KagemushaRecursionErrorV1::InvalidArtifacts)
        );
    }
}

#[test]
fn ordinary_state_artifact_seam_rejects_unbounded_or_missing_originals() {
    for parity in [0, 1] {
        for length in [0, KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1 + 1] {
            let mut substituted = ordinary();
            let binding = if parity == 0 {
                &mut substituted.guard_bundle_verifying_key_eq
            } else {
                &mut substituted.guard_bundle_verifying_key_ep
            };
            binding.byte_len = length;
            assert_eq!(
                substituted.validate(),
                Err(KagemushaRecursionErrorV1::InvalidArtifacts)
            );
        }
        let mut substituted = ordinary();
        let binding = if parity == 0 {
            &mut substituted.guard_bundle_verifying_key_eq
        } else {
            &mut substituted.guard_bundle_verifying_key_ep
        };
        binding.sha256 = [0; 32];
        assert_eq!(
            substituted.validate(),
            Err(KagemushaRecursionErrorV1::InvalidArtifacts)
        );
    }
}

#[test]
fn ordinary_state_artifact_seam_rejects_cross_role_or_protocol_aliases() {
    let selected = ordinary();
    for digest in [
        selected.guard_bundle_verifying_key_ep.sha256,
        selected.terminal_authorization_verifying_key_eq.sha256,
        selected.terminal_authorization_verifying_key_ep.sha256,
        selected.commit_wrapper_verifying_key_eq.sha256,
        selected.commit_wrapper_verifying_key_ep.sha256,
    ] {
        let mut substituted = selected;
        substituted.guard_bundle_verifying_key_eq.sha256 = digest;
        assert_eq!(
            substituted.validate(),
            Err(KagemushaRecursionErrorV1::InvalidArtifacts)
        );
    }
    for digest in [
        selected.eq_protocol_digest,
        selected.ep_protocol_digest,
        selected.terminal_authorization_eq_protocol_digest,
        selected.commit_wrapper_ep_protocol_digest,
        selected.mint_hash_claim_eq_protocol_digest,
    ] {
        let mut substituted = selected;
        substituted.guard_bundle_eq_protocol_digest = digest;
        assert_eq!(
            substituted.validate(),
            Err(KagemushaRecursionErrorV1::InvalidArtifacts)
        );
    }
}
