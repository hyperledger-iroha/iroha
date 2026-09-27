//! Staged composition of the Apple assertion with authenticated State and Guard cells.
//!
//! The terminal circuit copy-binds the SHA-derived body and the signed release, profile,
//! candidate, credential and app-policy fields to authenticated sources. This assertion stage
//! and issuer enrollment still must use those same assigned subject cells inside both live
//! recursive parities. It is deliberately not a monetary gate.
// TODO: Fold this assertion and issuer enrollment into the terminal relation using the exact
// assigned signed subject returned by the terminal-body link.

use halo2_base::{AssignedValue, gates::circuit::builder::BaseCircuitBuilder};
use halo2_ecc::{bigint::ProperCrtUint, ecc::EcPoint};
use iroha_data_model::kagemusha::{
    KagemushaHardwareCredentialV1, KagemushaHardwareProfileV1,
    KagemushaHardwareSelectionSigningLayoutV1,
    kagemusha_app_enrollment_v1::KagemushaAppAttestationAuthorityPolicyV1,
};

use crate::zk::{kagemusha_v1_poseidon::KagemushaPoseidonFieldV1, pasta_sha256::PastaSha256JobsV1};

use super::{
    KagemushaAssignedGuardBundleV1, KagemushaStateRelationWitnessV1,
    app_attest_assertion_fold::constrain_original_apple_assertion_ecdsa_37_v1,
    apple_compact_credential_id::{
        AppleCompactCredentialIdCellsV1, constrain_apple_compact_credential_id_v1,
    },
    apple_governed_policy_opening::{
        apple_policy_cells_from_state_guard_v1, constrain_apple_governed_signed_identity_v1,
    },
    constrain_apple_signed_subject_state_fields_v1, state_relation,
};

/// P-256 witness cells consumed by the canonical DER, SHA and signature relation.
pub(super) struct AppleAssertionSignatureCellsV1<'a, F: KagemushaPoseidonFieldV1> {
    pub(super) signature_public_key: &'a EcPoint<F, ProperCrtUint<F>>,
    pub(super) enrolled_public_key: &'a EcPoint<F, ProperCrtUint<F>>,
    pub(super) r: &'a ProperCrtUint<F>,
    pub(super) s: &'a ProperCrtUint<F>,
    pub(super) z: &'a ProperCrtUint<F>,
    pub(super) digest_reduction_quotient: AssignedValue<F>,
}

/// Copy-bind one original assertion to the same signed subject, policy, State
/// and Guard cells in a single parity. Invoke identically in Eq and Ep only
/// after the remaining terminal and issuer relations have been completed.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "staged monetary assertion fold remains closed")
)]
#[allow(clippy::too_many_arguments)]
pub(super) fn constrain_apple_state_guard_assertion_stage_v1<
    F: KagemushaPoseidonFieldV1,
    const N: usize,
>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    state_witness: &KagemushaStateRelationWitnessV1,
    state: &state_relation::KagemushaAssignedStateRelationV1<F>,
    guard: &KagemushaAssignedGuardBundleV1<F>,
    public: &[AssignedValue<F>],
    canonical_s: &[AssignedValue<F>; KagemushaHardwareSelectionSigningLayoutV1::TOTAL_BYTES],
    authenticator_data: &[AssignedValue<F>; 37],
    original_raw_assertion: &[u8],
    credential: &KagemushaHardwareCredentialV1,
    profile: &KagemushaHardwareProfileV1,
    policy: &KagemushaAppAttestationAuthorityPolicyV1,
    signature: AppleAssertionSignatureCellsV1<'_, F>,
) -> Result<(), String> {
    let enrolled_sec1 = constrain_apple_signed_subject_state_fields_v1(
        builder,
        jobs,
        state_witness,
        state,
        guard,
        public,
        canonical_s,
        authenticator_data,
        credential,
    )?;
    let governed = constrain_apple_governed_signed_identity_v1(
        builder,
        jobs,
        profile,
        policy,
        apple_policy_cells_from_state_guard_v1(state, guard),
        canonical_s,
        authenticator_data,
    )?;
    // Open the exact compact credential ID that Guard authenticated. Its
    // firmware field and profile lifetime come from the same governed profile
    // opening as the assertion's App ID, while network, key and epoch come
    // from the recursively authenticated predecessor State/Guard cells.
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let compact_credential = AppleCompactCredentialIdCellsV1 {
        network_id: state.predecessor.network_id,
        hardware_profile_id: state.predecessor.hardware_profile_id,
        suite_id: state.predecessor.suite_id,
        firmware_policy_digest: governed.firmware_policy_digest,
        policy_epoch: state.predecessor.policy_epoch,
        lane_id: state.predecessor.lane_id,
        epoch_id: state.predecessor.epoch_id,
        epoch_generation: state.predecessor.epoch_generation,
        device_public_key: guard.credential_device_public_keys[0].clone(),
        key_reference: state.predecessor.key_reference,
        profile_valid_from_ms: governed.valid_from_ms,
        profile_expires_at_ms: governed.expires_at_ms,
        issued_at_ms: ctx.load_witness(F::from(credential.issued_at_ms)),
        expires_at_ms: ctx.load_witness(F::from(credential.expires_at_ms)),
        app_policy_binding_digest: guard.credential_app_policy_binding_digests[0],
        guard_issuance_digest: guard.credential_issuance_digests[0],
    };
    constrain_apple_compact_credential_id_v1(ctx, &range, jobs, &compact_credential)?;
    constrain_original_apple_assertion_ecdsa_37_v1::<F, N>(
        builder,
        jobs,
        original_raw_assertion,
        canonical_s,
        authenticator_data,
        &governed.rp_id_hash,
        state.predecessor.secure_index,
        state.successor.secure_index,
        signature.signature_public_key,
        signature.enrolled_public_key,
        &enrolled_sec1,
        signature.r,
        signature.s,
        signature.z,
        signature.digest_reduction_quotient,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_proofs::halo2curves::pasta::{Fp, Fq};

    #[test]
    fn staged_composition_has_the_same_typed_entry_point_in_both_parities() {
        // This checks that both recursive fields instantiate the full composition.
        // TODO: Add a positive and mutation MockProver fixture built from one
        // non-bootstrap State, Apple credential, Guard, and exact signed subject.
        let _ = constrain_apple_state_guard_assertion_stage_v1::<Fp, 256>;
        let _ = constrain_apple_state_guard_assertion_stage_v1::<Fq, 256>;
    }
}
