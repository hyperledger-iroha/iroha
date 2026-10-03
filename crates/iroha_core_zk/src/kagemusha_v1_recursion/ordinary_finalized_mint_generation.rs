//! Genuine ordinary finalized-credit adapter for the unchanged neutral MintAuthority relation.
//! The exact admitted Mint113 original and complete finalized receipt are bound to the neutral
//! statement, certificate and both terminally decided histories. This result supplies proof
//! originals only; Node custody, current effects and one-use incoming State remain separate.
use super::super::{
    KagemushaAuthenticatedRecursiveVerifierV1, KagemushaVerifiedOrdinaryMintAuthorizationV1,
    verify_kagemusha_mint_finality_helper_v1,
};
use super::*;
use iroha_data_model::kagemusha::{
    KagemushaOrdinaryTopUpFinalizedOriginalV1, KagemushaOrdinaryTopUpRequestV1,
};

/// Owned exact generated neutral credit for durable ordinary Node publication.
/// No decoder, clone or caller-field constructor produces this value. The complete paired
/// proof and histories have passed the same actual installed recursive verifier. It conveys no
/// current account debit, finalized-source custody, user key or incoming State authority.
#[derive(Debug)]
pub struct KagemushaGeneratedOrdinaryFinalizedMintCreditV1 {
    credit: KagemushaMintCreditV1,
    credit_original: Vec<u8>,
    credit_original_sha256: [u8; 32],
    finalized_source_original_sha256: [u8; 32],
    request_original_sha256: [u8; 32],
    authority_checkpoint_original_sha256: [u8; 32],
}
impl KagemushaGeneratedOrdinaryFinalizedMintCreditV1 {
    /// Same neutral statement, actual generated pair and exact ciphertext original.
    #[must_use]
    pub fn credit(&self) -> &KagemushaMintCreditV1 {
        &self.credit
    }
    /// Full bounded sole canonical original to persist before publication or retry.
    #[must_use]
    pub fn credit_original(&self) -> &[u8] {
        &self.credit_original
    }
    /// Raw SHA256 of the complete credit original, including randomized current proofs/history.
    #[must_use]
    pub const fn credit_original_sha256(&self) -> [u8; 32] {
        self.credit_original_sha256
    }
    /// Raw SHA256 of the complete original request, signed debit decision and native finality.
    #[must_use]
    pub const fn finalized_source_original_sha256(&self) -> [u8; 32] {
        self.finalized_source_original_sha256
    }
    /// Same complete unsigned request actually admitted by Mint113 before debit.
    #[must_use]
    pub const fn request_original_sha256(&self) -> [u8; 32] {
        self.request_original_sha256
    }
    /// Exact original bootstrap/rotation checkpoint independently reverified for this proof.
    #[must_use]
    pub const fn authority_checkpoint_original_sha256(&self) -> [u8; 32] {
        self.authority_checkpoint_original_sha256
    }
    /// Actual release constrained by the accepted neutral proof pair.
    #[must_use]
    pub fn release_id(&self) -> [u8; 32] {
        self.credit.statement.lifecycle.release_id
    }
}
fn fail(error: impl core::fmt::Display) -> KagemushaArtifactGenerationErrorV1 {
    KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string())
}

fn require_request_and_certificate(
    authorization: &KagemushaVerifiedOrdinaryMintAuthorizationV1,
    finalized: &KagemushaOrdinaryTopUpFinalizedOriginalV1,
    certificate: &KagemushaMintCertificateWitnessV1,
) -> Result<KagemushaOrdinaryTopUpRequestV1, KagemushaArtifactGenerationErrorV1> {
    finalized.validate_originals().map_err(fail)?;
    if finalized.request_original.as_slice() != authorization.request_original()
        || <[u8; 32]>::from(Sha256::digest(&finalized.request_original))
            != authorization.request_original_sha256()
    {
        return Err(fail(
            "ordinary finalized source changes the complete admitted Mint113 request",
        ));
    }
    let request =
        KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(&finalized.request_original)
            .map_err(fail)?;
    let receipt = &finalized.finality.reserve_receipt_witness.receipt;
    let expected = authorization
        .authorization()
        .finalized_credit_statement(receipt.committed_at_ms)
        .map_err(fail)?;
    if &request.authorization != authorization.authorization()
        || certificate.statement != expected
        || finalized.finality.top_up_membership_witness.as_ref() != Some(&certificate.membership)
    {
        return Err(fail(
            "ordinary certificate changes the full admitted authorization, receipt or top-up membership",
        ));
    }
    certificate.validate_shape().map_err(fail)?;
    Ok(request)
}

/// Reverify an exact retained ordinary neutral credit without re-proving randomized originals.
/// The caller must hold actual finalized Node/source custody. This checks proof identity and
/// historical mathematical authority only; no current debit window, FI grant or State advance
/// is derived from the retained frame.
/// # Errors
/// Refuses any changed full Mint113/source/certificate/credit/checkpoint original, incompatible
/// ordinary family/release, another roster/genesis or either invalid proof/history.
pub fn verify_ordinary_finalized_mint_credit_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    authorization: &KagemushaVerifiedOrdinaryMintAuthorizationV1,
    finalized: &KagemushaOrdinaryTopUpFinalizedOriginalV1,
    certificate: &KagemushaMintCertificateWitnessV1,
    checkpoint: &KagemushaMintAuthorityCheckpointV1,
    credit: &KagemushaMintCreditV1,
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    let request = require_request_and_certificate(authorization, finalized, certificate)?;
    let context = &request.authorization.statement.context;
    let material = verifier.ordinary_mint_material().map_err(fail)?;
    let artifacts = verifier.state_checkpoint_material().artifacts;
    if context.release_id != material.release_id
        || context.suite_id != material.suite_id
        || context.vk_digest != material.vk_set_digest
        || context.artifact_manifest_digest != material.artifact_manifest_digest
        || request.authorization.proof.eq_protocol_digest != material.eq_protocol_digest
        || request.authorization.proof.ep_protocol_digest != material.ep_protocol_digest
        || context.release_id != artifacts.release_id
        || context.artifact_manifest_digest != artifacts.artifact_manifest_digest
    {
        return Err(fail(
            "ordinary finalized Mint selects another installed family or release",
        ));
    }
    let _accepted_checkpoint = verifier
        .verify_mint_authority_checkpoint(checkpoint)
        .map_err(fail)?;
    let head = certificate
        .seal_bundle
        .message
        .epoch_authorization
        .authorization_id()
        .map_err(fail)?;
    let binding = certificate
        .certificate_binding_digest(KagemushaMintAuthorityStepV1::FinalizedMint)
        .map_err(fail)?;
    if checkpoint.authority_head != head
        || checkpoint.release_id != context.release_id
        || checkpoint.genesis_authorization_id != verifier.mint_genesis_authorization_id()
        || credit.finality_certificate_binding != binding
        || credit.finality_authority_head != head
        || credit.finality_genesis_authorization_id != checkpoint.genesis_authorization_id
    {
        return Err(fail(
            "ordinary finalized Mint changes the original certificate or authenticated checkpoint head",
        ));
    }
    request
        .validate_finalized_credit(
            credit,
            finalized
                .finality
                .reserve_receipt_witness
                .receipt
                .committed_at_ms,
        )
        .map_err(fail)?;
    let _accepted_credit =
        verify_kagemusha_mint_finality_helper_v1(verifier, artifacts, credit).map_err(fail)?;
    Ok(())
}

/// Generate the exact ordinary finalized credit under the actual held neutral authority keys.
/// This uses the unchanged full quorum/certificate/MintHash/authority checkpoint relation and
/// terminally re-admits its result before exposing owned originals. The actual Node source owner
/// must authenticate full native finality/World history before this call and persist the returned
/// exact original before serving it; retries must reverify that original rather than re-prove.
/// # Errors
/// Refuses altered original scope, missing actual ordinary family, wrong checkpoint/release,
/// unsupported original size or any genuine circuit/proof/history rejection.
pub fn prove_ordinary_finalized_mint_from_checkpoint_v1(
    eq: &KagemushaLoadedEqMintAuthorityArtifactsV1,
    ep: &KagemushaLoadedEpMintAuthorityArtifactsV1,
    hash_eq: &KagemushaLoadedEqMintHashArtifactsV1,
    hash_ep: &KagemushaLoadedEpMintHashArtifactsV1,
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    authorization: &KagemushaVerifiedOrdinaryMintAuthorizationV1,
    finalized: &KagemushaOrdinaryTopUpFinalizedOriginalV1,
    certificate: KagemushaMintCertificateWitnessV1,
    checkpoint: &KagemushaMintAuthorityCheckpointV1,
) -> Result<KagemushaGeneratedOrdinaryFinalizedMintCreditV1, KagemushaArtifactGenerationErrorV1> {
    let request = require_request_and_certificate(authorization, finalized, &certificate)?;
    let material = verifier.ordinary_mint_material().map_err(fail)?;
    let context = &request.authorization.statement.context;
    if context.release_id != material.release_id
        || context.suite_id != material.suite_id
        || context.vk_digest != material.vk_set_digest
        || context.artifact_manifest_digest != material.artifact_manifest_digest
        || request.authorization.proof.eq_protocol_digest != material.eq_protocol_digest
        || request.authorization.proof.ep_protocol_digest != material.ep_protocol_digest
    {
        return Err(fail(
            "ordinary finalized Mint differs from the actual installed Mint113 family",
        ));
    }
    let generated = prove_kagemusha_finalized_mint_from_checkpoint_v1(
        eq,
        ep,
        hash_eq,
        hash_ep,
        verifier,
        certificate.clone(),
        checkpoint,
    )?;
    let credit = KagemushaMintCreditV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        statement: certificate.statement.clone(),
        proof: generated.proof,
        finality_certificate_binding: generated.certificate_binding,
        finality_authority_head: generated.authority_head,
        finality_genesis_authorization_id: generated.genesis_authorization_id,
        finality_proof_binding_digest: generated.proof_binding_digest,
        encrypted_credit: request.encrypted_credit.clone(),
        artifact_manifest_digest: context.artifact_manifest_digest,
    };
    verify_ordinary_finalized_mint_credit_v1(
        verifier,
        authorization,
        finalized,
        &certificate,
        checkpoint,
        &credit,
    )?;
    let credit_original = norito::encode_canonical(&credit).map_err(fail)?;
    // Recheck complete supported framing, including both proof histories, before retaining it.
    KagemushaMintCreditV1::decode_canonical_shape_exact(&credit_original).map_err(fail)?;
    let finalized_original = finalized.canonical_bytes().map_err(fail)?;
    let checkpoint_original = norito::encode_canonical(checkpoint).map_err(fail)?;
    Ok(KagemushaGeneratedOrdinaryFinalizedMintCreditV1 {
        credit_original_sha256: Sha256::digest(&credit_original).into(),
        finalized_source_original_sha256: Sha256::digest(&finalized_original).into(),
        request_original_sha256: authorization.request_original_sha256(),
        authority_checkpoint_original_sha256: Sha256::digest(&checkpoint_original).into(),
        credit,
        credit_original,
    })
}
