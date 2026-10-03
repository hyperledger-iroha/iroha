//! Concrete ordinary Node credit producer/readback under actual archived source custody.
//! The source loan is constructed only by the committed World/Kura publication owner. Historical
//! source verification never reuses a live transaction token or renews the old debit decision.
use super::ordinary_mint_publication::KagemushaAuthenticatedOrdinaryNodeFinalizedMintSourceV1;
use super::*;
use crate::zk::kagemusha_v1_recursion::{
    KagemushaGeneratedOrdinaryFinalizedMintCreditV1,
    prove_ordinary_finalized_mint_from_checkpoint_v1, verify_ordinary_finalized_mint_credit_v1,
};

fn certificate(
    source: &KagemushaAuthenticatedOrdinaryNodeFinalizedMintSourceV1<'_>,
) -> Result<KagemushaMintCertificateWitnessV1, String> {
    source.recheck_retained_custody()?;
    let finalized = source.finalized()?;
    let authorization = source.authorization()?;
    let anchor = source.trust_anchor()?;
    finalized.validate_against(anchor)?;
    if finalized.request_original.as_slice() != authorization.request_original()
        || <[u8; 32]>::from(Sha256::digest(&finalized.request_original))
            != authorization.request_original_sha256()
    {
        return Err("ordinary Node source changes its genuine historical Mint113 request".into());
    }
    let statement = authorization.authorization().finalized_credit_statement(
        finalized
            .finality
            .reserve_receipt_witness
            .receipt
            .committed_at_ms,
    )?;
    let membership = finalized
        .finality
        .top_up_membership_witness
        .clone()
        .ok_or("ordinary finalized Node source lacks its complete top-up membership")?;
    let (seal_bundle, authority_generation) =
        crate::sumeragi::attestation::verify_native_mint_finality_bundle(
            &finalized.finality.finality_proof,
            anchor,
        )?;
    let certificate = KagemushaMintCertificateWitnessV1 {
        statement,
        membership,
        seal_bundle,
        authority_generation,
    };
    certificate.validate_shape()?;
    source.recheck_retained_custody()?;
    Ok(certificate)
}

/// Prove from the actual retained Node source and separately reverified durable roster checkpoint.
/// Caller persists the exact returned full original before any publication or recovery response.
pub(super) fn prove(
    registry: &AuthenticatedKagemushaV1RuntimeVerifier,
    source: &KagemushaAuthenticatedOrdinaryNodeFinalizedMintSourceV1<'_>,
    checkpoint: &KagemushaMintAuthorityCheckpointV1,
) -> Result<KagemushaGeneratedOrdinaryFinalizedMintCreditV1, String> {
    source.recheck_runtime(registry)?;
    let certificate = certificate(source)?;
    let release_id = certificate.statement.lifecycle.release_id;
    let runtime = registry.runtime_for_terminal_verification(release_id)?;
    require_production_release_purpose_v1(runtime.purpose)?;
    require_release_mint_scope_v1(
        runtime.purpose,
        runtime.network_id,
        runtime.release_id,
        KagemushaMintScopeSubjectV1::from_statement(&certificate.statement),
    )?;
    if source.trust_anchor()?.network_id != runtime.network_id {
        return Err("ordinary Node source anchor changes its installed release network".into());
    }
    registry.verify_mint_authority_checkpoint(
        release_id,
        &certificate.seal_bundle.message.epoch_authorization,
        checkpoint,
    )?;
    source.recheck_retained_custody()?;
    let generated = prove_ordinary_finalized_mint_from_checkpoint_v1(
        &runtime.eq_mint_prover,
        &runtime.ep_mint_prover,
        &runtime.eq_mint_hash_prover,
        &runtime.ep_mint_hash_prover,
        &runtime.verifier,
        source.authorization()?,
        source.finalized()?,
        certificate,
        checkpoint,
    )
    .map_err(|e| format!("genuine ordinary finalized-mint proof rejected: {e}"))?;
    source.recheck_retained_custody()?;
    source.recheck_runtime(registry)?;
    Ok(generated)
}

/// Reverify the exact immutable full retained credit without generating replacement proof bytes.
/// This checks historical source/certificate/roster and both current proofs plus full histories.
pub(super) fn verify(
    registry: &AuthenticatedKagemushaV1RuntimeVerifier,
    source: &KagemushaAuthenticatedOrdinaryNodeFinalizedMintSourceV1<'_>,
    credit: &KagemushaMintCreditV1,
    checkpoint: &KagemushaMintAuthorityCheckpointV1,
) -> Result<(), String> {
    source.recheck_runtime(registry)?;
    let certificate = certificate(source)?;
    let release_id = certificate.statement.lifecycle.release_id;
    let runtime = registry.runtime_for_terminal_verification(release_id)?;
    require_production_release_purpose_v1(runtime.purpose)?;
    require_release_mint_scope_v1(
        runtime.purpose,
        runtime.network_id,
        runtime.release_id,
        KagemushaMintScopeSubjectV1::from_statement(&certificate.statement),
    )?;
    if source.trust_anchor()?.network_id != runtime.network_id {
        return Err(
            "retained ordinary Node source anchor changes its installed release network".into(),
        );
    }
    registry.verify_mint_authority_checkpoint(
        release_id,
        &certificate.seal_bundle.message.epoch_authorization,
        checkpoint,
    )?;
    source.recheck_retained_custody()?;
    verify_ordinary_finalized_mint_credit_v1(
        &runtime.verifier,
        source.authorization()?,
        source.finalized()?,
        &certificate,
        checkpoint,
        credit,
    )
    .map_err(|e| format!("retained ordinary finalized-mint proof rejected: {e}"))?;
    source.recheck_retained_custody()?;
    source.recheck_runtime(registry)
}
