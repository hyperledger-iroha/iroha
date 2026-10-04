//! Canonical masked AXT artifacts with independently checked outer context.

use iroha_allocation::{AllocationBudget, AllocationReservation};

use super::*;
use crate::{
    gadgets::public_transfer_statement::{
        prepare_quantity_public_transfers, public_claims_from_transcripts,
    },
    offline_compact::{
        self, ExpectedAxtContext, ExpectedStatement, ProvingError, ProvingLimits,
        VerificationError, VerificationLimits,
    },
};
use iroha_data_model::fastpq::{
    FastpqAxtCompactArtifactV1, FastpqAxtPreProofMirrorsV1, FastpqAxtPublicMetadataV1,
    FastpqPublicTransferStatementV1,
};

pub(super) fn prepare(
    batch: &TransitionBatch,
    binding: &AxtFastpqBinding,
) -> Result<FastpqAxtCompactArtifactV1> {
    enforce_default_verify_batch_limits(batch)?;
    if batch.parameter != DEFAULT_PARAMETER {
        return Err(Error::ParameterMismatch {
            expected: DEFAULT_PARAMETER.to_owned(),
            actual: batch.parameter.clone(),
        });
    }
    let binding = require_canonical_binding(binding)?;
    validate_axt_transfer_claim_binding(&binding)?;
    verify_batch_matches_canonical_binding(batch, &binding, None, None)?;
    let limits = VerificationLimits::default();
    let transcripts =
        decode_transcripts(&batch.metadata)?.ok_or_else(|| Error::MissingMetadata {
            key: TRANSFER_TRANSCRIPTS_METADATA_KEY.into(),
        })?;
    let claims = public_claims_from_transcripts(&transcripts, limits.public_statement)?;
    let prepared = prepare_quantity_public_transfers(
        &batch.transitions,
        &claims,
        batch.public_inputs,
        ProofSemantics::AxtTransferClaim,
        limits.public_statement,
    )?;
    let model = transition_batch_to_model(batch);
    let statement = FastpqPublicTransferStatementV1 {
        public_inputs: model.public_inputs,
        ordering_hash: prepared.ordering_hash().into(),
        transitions: model.transitions,
        transcripts: transcripts.iter().map(Into::into).collect(),
    };
    let mirrors = FastpqAxtPreProofMirrorsV1 {
        dsid: DataSpaceId::new(binding.source_dsid),
        manifest_root: proof_bound_manifest_root(batch)?,
        da_commitment: proof_bound_da_commitment(batch)?,
        committed_amount: proof_bound_committed_amount(batch)?,
        expiry_slot: proof_bound_expiry_slot(batch)?,
    };
    let mut metadata = metadata(&binding, mirrors)?;
    if let Some(encoded) = batch
        .metadata
        .get(AXT_FASTPQ_SOURCE_TRANSFER_OCCURRENCES_METADATA_KEY)
    {
        metadata.source_transfer_occurrences =
            norito::decode_canonical(encoded).map_err(|_| Error::InvalidAxtBinding {
                details: "source transfer occurrences must use canonical Norito".into(),
            })?;
    }
    let remote_spend_claims = batch
        .metadata
        .get(AXT_FASTPQ_REMOTE_SPEND_CLAIMS_METADATA_KEY)
        .map(|encoded| decode_bound_remote_spend_claims(encoded, &binding))
        .transpose()?;
    Ok(FastpqAxtCompactArtifactV1 {
        profile_id: offline_compact::quantity_profile_id(),
        statement,
        binding,
        metadata,
        mirrors,
        remote_spend_claims,
        bundle_frame: Vec::new(),
    })
}

fn metadata(
    binding: &AxtFastpqBinding,
    mirrors: FastpqAxtPreProofMirrorsV1,
) -> Result<FastpqAxtPublicMetadataV1> {
    Ok(FastpqAxtPublicMetadataV1 {
        source_transfer_occurrences: Vec::new(),
        parameter: binding.parameter.clone(),
        entry_hash: decode_hex_digest(&binding.source_tx_commitment, "source_tx_commitment")?,
        committed_amount: mirrors.committed_amount.map(u128::to_le_bytes),
        expiry_slot: mirrors.expiry_slot.unwrap_or(0).to_le_bytes(),
        manifest_root: mirrors.manifest_root,
        da_commitment: encode_optional_da_commitment(mirrors.da_commitment)
            .try_into()
            .expect("fixed option encoding"),
    })
}

fn expected(artifact: &FastpqAxtCompactArtifactV1) -> Result<ExpectedStatement> {
    Ok(ExpectedStatement {
        inputs: artifact.statement.public_inputs,
        ordering_hash: artifact.statement.ordering_hash,
        public_statement_digest: Hash::new(norito::encode_canonical(&artifact.statement)?).into(),
    })
}

fn context(artifact: &FastpqAxtCompactArtifactV1) -> ExpectedAxtContext<'_> {
    ExpectedAxtContext {
        binding: &artifact.binding,
        metadata: &artifact.metadata,
        mirrors: artifact.mirrors,
        remote_spend_claims: artifact.remote_spend_claims.as_deref(),
    }
}

pub(super) fn prove(
    batch: &TransitionBatch,
    binding: &AxtFastpqBinding,
    budget: &AllocationBudget,
    reservation: &mut AllocationReservation,
) -> Result<Vec<u8>> {
    if !reservation.belongs_to(budget) {
        return Err(Error::AllocationForeignPool);
    }
    let artifact = prepare(batch, binding)?;
    offline_compact::prove_quantity_axt_artifact(
        &artifact.statement,
        expected(&artifact)?,
        context(&artifact),
        ProvingLimits::default(),
        VerificationLimits::default(),
        budget,
        reservation,
    )
    .map_err(|error| match error {
        ProvingError::Prove(error) => error,
        ProvingError::Verify(error) => verification_error(error),
        ProvingError::Busy => Error::ProducerBusy,
    })
}

pub(super) fn verify_bound(
    batch: &TransitionBatch,
    bytes: &[u8],
    binding: &AxtFastpqBinding,
) -> Result<()> {
    enforce_axt_fastpq_payload_limit(bytes)?;
    let artifact = prepare(batch, binding)?;
    offline_compact::verify_quantity_axt_artifact(
        bytes,
        expected(&artifact)?,
        context(&artifact),
        VerificationLimits::default(),
    )
    .map(|_| ())
    .map_err(verification_error)
}

pub(super) fn decode(bytes: &[u8]) -> Result<FastpqAxtCompactArtifactV1> {
    enforce_axt_fastpq_payload_limit(bytes)?;
    FastpqAxtCompactArtifactV1::decode_canonical_with_limits(
        bytes,
        offline_compact::quantity_profile_id(),
        VerificationLimits::default().transport,
    )
    .map_err(|error| Error::InvalidAxtBinding {
        details: format!("invalid canonical AXT artifact: {error}"),
    })
}

fn verification_error(error: VerificationError) -> Error {
    match error {
        VerificationError::Verify(error) => error,
        VerificationError::Transport(error) => Error::InvalidAxtBinding {
            details: format!("invalid canonical AXT artifact: {error}"),
        },
    }
}

pub(super) fn verify_envelope(
    envelope: &AxtProofEnvelope,
    finalized: Option<(&AxtFinalizedSpendAnchorV1, Option<u64>, u32)>,
    claimed_occurrence: Option<&AxtSourceTransferOccurrenceV1>,
) -> Result<AxtVerifiedProof> {
    enforce_axt_fastpq_payload_limit(&envelope.proof)?;
    let binding = envelope
        .fastpq_binding
        .as_ref()
        .ok_or_else(|| Error::InvalidAxtBinding {
            details: "AXT proof envelope is missing fastpq_binding".into(),
        })?;
    let binding = require_canonical_binding(binding)?;
    validate_axt_transfer_claim_binding(&binding)?;
    if binding.source_dsid != envelope.dsid.as_u64() {
        return Err(Error::InvalidAxtBinding {
            details: "AXT proof envelope source_dsid does not match dsid".into(),
        });
    }
    let artifact = decode(&envelope.proof)?;
    let inputs = artifact.statement.public_inputs;
    if let Some((anchor, _, _)) = finalized {
        require_finalized_public_inputs_v1(
            &PublicInputs {
                dsid: inputs.dsid,
                slot: inputs.slot,
                old_root: inputs.old_root,
                new_root: inputs.new_root,
                perm_root: inputs.perm_root,
                tx_set_hash: inputs.tx_set_hash,
            },
            anchor,
        )?;
    }
    let expiry_slot = finalized.map_or(artifact.mirrors.expiry_slot, |(_, expiry, _)| expiry);
    let mirrors = FastpqAxtPreProofMirrorsV1 {
        dsid: envelope.dsid,
        manifest_root: envelope.manifest_root,
        da_commitment: envelope.da_commitment,
        committed_amount: envelope.committed_amount,
        expiry_slot,
    };
    let mut metadata = metadata(&binding, mirrors)?;
    metadata
        .source_transfer_occurrences
        .clone_from(&artifact.metadata.source_transfer_occurrences);
    let source_success_receipt_digest = source_occurrence::validate_public_occurrences(
        &binding,
        &artifact.statement.transcripts,
        artifact.remote_spend_claims.as_deref().unwrap_or(&[]),
        &metadata.source_transfer_occurrences,
        finalized.map(|(_, _, index)| index),
        claimed_occurrence,
    )?;
    offline_compact::verify_quantity_axt_artifact(
        &envelope.proof,
        expected(&artifact)?,
        ExpectedAxtContext {
            binding: &binding,
            metadata: &metadata,
            mirrors,
            remote_spend_claims: artifact.remote_spend_claims.as_deref(),
        },
        VerificationLimits::default(),
    )
    .map_err(verification_error)?;
    Ok(AxtVerifiedProof {
        statement_digest: axt_statement_digest(envelope, &binding, &artifact.statement)?,
        proof_digest: Hash::new(&envelope.proof),
        old_root: inputs.old_root,
        new_root: inputs.new_root,
        tx_set_hash: inputs.tx_set_hash,
        expiry_slot,
        source_success_receipt_digest,
    })
}
