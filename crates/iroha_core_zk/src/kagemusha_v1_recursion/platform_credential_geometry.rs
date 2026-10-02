//! Full PlatformCredential graph geometry without SHA proving or proof authority.
//!
//! The parser material below only fills the exact authenticated wire shape. It is not a valid
//! Claim or fold proof and is never synthesized, proven, accepted or exposed to a consumer.
//! Every original verifier, hash-binding, history and reciprocal constraint is nevertheless
//! built by the same circuit constructors used after the genuine typed SHA claim is produced.
//! The only returned values describe the complete Base layouts that passed the unchanged key
//! resource guard. Actual synthesis, serialized-key guards and genuine proof verification remain
//! mandatory. TODO: measure graph construction memory/time as well as the configured layout.

use super::super::guard_bundle::{
    KagemushaPlatformCredentialHashClaimPairWitnessV1,
    KagemushaPlatformCredentialHashClaimParityWitnessV1,
    KagemushaPlatformCredentialRelationCircuitV1, build_kagemusha_platform_credential_ep_v1,
    build_kagemusha_platform_credential_eq_v1, discover_kagemusha_platform_credential_audits_v1,
};
use super::super::mint_hash_claim_fold::{
    KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
    KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
};
use super::*;

fn unproven_instance_shape_v1<F: ff::Field>() -> Vec<Vec<F>> {
    vec![
        // A nonzero semantic column retains its actual instance-commitment namespace.
        // Zero would create an identity point and refuse before measuring the verifier.
        vec![F::ONE; KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1],
        vec![F::ZERO; KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1],
        vec![F::ZERO; KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1],
    ]
}

pub(super) fn preflight_complete_graph_v1(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    eq_hash: &KagemushaLoadedEqMintHashArtifactsV1,
    ep_hash: &KagemushaLoadedEpMintHashArtifactsV1,
    relation: &super::super::KagemushaPlatformCredentialRelationWitnessV1,
    provider_policy_root: [u8; 32],
) -> Result<(BaseCircuitParams, BaseCircuitParams), KagemushaArtifactGenerationErrorV1> {
    let build_error = KagemushaArtifactGenerationErrorV1::CircuitBuild;
    if eq_params.k() != KAGEMUSHA_HALO2_K_V1
        || ep_params.k() != KAGEMUSHA_HALO2_K_V1
        || provider_policy_root == [0; 32]
        || relation.statement.hardware_policy_id != provider_policy_root
    {
        return Err(build_error(
            "PlatformCredential full geometry requires the actual k16 parameters and provider root"
                .to_owned(),
        ));
    }
    relation.validate().map_err(build_error)?;
    let eq_instances = unproven_instance_shape_v1::<Fp>();
    let ep_instances = unproven_instance_shape_v1::<Fq>();
    let eq_point = EqAffine::generator().to_bytes();
    let ep_point = EpAffine::generator().to_bytes();
    // These are canonical parser items, deliberately not proved or accepted witnesses.
    let eq_proof = dummy_two_carrier_hybrid_ordinary_proof_bytes(
        &eq_hash.claim_protocol,
        eq_point.as_ref(),
        KagemushaPastaParityV1::Eq,
    )?;
    let ep_proof = dummy_two_carrier_hybrid_ordinary_proof_bytes(
        &ep_hash.claim_protocol,
        ep_point.as_ref(),
        KagemushaPastaParityV1::Ep,
    )?;
    let eq_fold = dummy_fold_proof_bytes(eq_point.as_ref());
    let ep_fold = dummy_fold_proof_bytes(ep_point.as_ref());
    let eq_history = super::super::initial_kagemusha_eq_accumulator_v1(eq_params)
        .map_err(|error| build_error(error.to_string()))?;
    let ep_history = super::super::initial_kagemusha_ep_accumulator_v1(ep_params)
        .map_err(|error| build_error(error.to_string()))?;
    let eq_native_history = eq_history
        .to_native()
        .map_err(|error| build_error(error.to_string()))?;
    let ep_native_history = ep_history
        .to_native()
        .map_err(|error| build_error(error.to_string()))?;
    let witness = KagemushaPlatformCredentialHashClaimPairWitnessV1 {
        relation: relation.clone(),
        eq_claim_protocol_digest: eq_hash.claim_protocol_digest,
        ep_claim_protocol_digest: ep_hash.claim_protocol_digest,
        eq_shard_protocol_digest: eq_hash.shard_protocol_digest,
        ep_shard_protocol_digest: ep_hash.shard_protocol_digest,
        eq: KagemushaPlatformCredentialHashClaimParityWitnessV1 {
            claim_protocol: &eq_hash.claim_protocol,
            claim_instances: &eq_instances,
            claim_proof: &eq_proof,
            claim_history: &eq_native_history,
            claim_history_fold_proof: &eq_fold,
            successor_history: eq_history.as_bytes(),
        },
        ep: KagemushaPlatformCredentialHashClaimParityWitnessV1 {
            claim_protocol: &ep_hash.claim_protocol,
            claim_instances: &ep_instances,
            claim_proof: &ep_proof,
            claim_history: &ep_native_history,
            claim_history_fold_proof: &ep_fold,
            successor_history: ep_history.as_bytes(),
        },
    };
    let discovery = discover_kagemusha_platform_credential_audits_v1(
        eq_params,
        ep_params,
        &witness,
        provider_policy_root,
    )
    .map_err(build_error)?;
    let eq = build_kagemusha_platform_credential_eq_v1(
        eq_params,
        &witness,
        &discovery,
        provider_policy_root,
    )
    .map_err(build_error)?;
    let eq_layout = eq.params();
    drop(eq);
    halo2_proofs::release_allocator_slack();
    preflight_helper_key_configuration_v1::<
        EqAffine,
        KagemushaPlatformCredentialRelationCircuitV1<Fp>,
    >(
        KAGEMUSHA_HALO2_K_V1 as usize,
        eq_layout.clone(),
        KagemushaPastaParityV1::Eq,
        "PlatformCredential complete graph before SHA proving",
    )?;
    let ep = build_kagemusha_platform_credential_ep_v1(
        ep_params,
        &witness,
        &discovery,
        provider_policy_root,
    )
    .map_err(build_error)?;
    let ep_layout = ep.params();
    drop(ep);
    halo2_proofs::release_allocator_slack();
    preflight_helper_key_configuration_v1::<
        EpAffine,
        KagemushaPlatformCredentialRelationCircuitV1<Fq>,
    >(
        KAGEMUSHA_HALO2_K_V1 as usize,
        ep_layout.clone(),
        KagemushaPastaParityV1::Ep,
        "PlatformCredential complete graph before SHA proving",
    )?;
    Ok((eq_layout.base, ep_layout.base))
}
