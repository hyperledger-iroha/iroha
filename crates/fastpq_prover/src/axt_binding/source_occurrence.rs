//! Proof-trace binding for claimed successful source receipts and exact transfers.

use iroha_data_model::{
    fastpq::{
        FastpqPublicTransferDeltaV1, FastpqPublicTransferTranscriptV1,
        TRANSFER_TRANSCRIPTS_METADATA_KEY,
    },
    nexus::{
        AxtFastpqBinding, AxtRemoteSpendClaimV1, AxtSourceTransferOccurrenceV1,
        MAX_REMOTE_SPEND_INTENT_COMMITMENTS_V1, axt_source_transfer_digest_v1,
    },
};

use super::{
    AXT_FASTPQ_BATCH_SEAL_METADATA_KEY, AXT_FASTPQ_REMOTE_SPEND_CLAIMS_METADATA_KEY,
    AXT_FASTPQ_SOURCE_TRANSFER_OCCURRENCES_METADATA_KEY, Error, Result, TransitionBatch,
    canonical_remote_account, decode_bound_remote_spend_claims, decode_hex_digest,
    decode_transcripts, encode_canonical_norito, require_canonical_binding,
};

fn invalid(details: impl Into<String>) -> Error {
    Error::InvalidAxtBinding {
        details: details.into(),
    }
}

/// Attach exact source transfer occurrences before the AXT batch is sealed.
///
/// Each occurrence carries a claimed successful-execution receipt digest and
/// selects one ordered transfer pair. The canonical public occurrence list is
/// committed by the compact FASTPQ relation; the issuer's existing fresh signature covers
/// the resulting canonical proof blob digest. This helper does not authenticate
/// the claimed receipt against finalized State.
///
/// # Errors
/// Rejects a sealed batch, missing remote-spend claim preimages, malformed
/// coordinates, duplicate occurrences, a mismatch with public transcripts,
/// or an unavailable local allocation after bounded size preflight.
pub fn set_axt_source_transfer_occurrences(
    batch: &mut TransitionBatch,
    binding: &AxtFastpqBinding,
    occurrences: &[AxtSourceTransferOccurrenceV1],
) -> Result<()> {
    if batch
        .metadata
        .contains_key(AXT_FASTPQ_BATCH_SEAL_METADATA_KEY)
    {
        return Err(invalid(
            "source transfer occurrences must be attached before the AXT batch is sealed",
        ));
    }
    let canonical = require_canonical_binding(binding)?;
    if canonical.remote_spend_intent_commitments.is_empty() {
        return Err(invalid(
            "source transfer occurrences require remote-spend commitments",
        ));
    }
    validate_occurrences(batch, &canonical, occurrences, None, None)?;
    let mut owned_occurrences = Vec::new();
    owned_occurrences
        .try_reserve_exact(occurrences.len())
        .map_err(|_| Error::LocalAllocationUnavailable {
            context: "source transfer occurrence metadata",
        })?;
    owned_occurrences.extend_from_slice(occurrences);
    batch.metadata.insert(
        AXT_FASTPQ_SOURCE_TRANSFER_OCCURRENCES_METADATA_KEY.into(),
        encode_canonical_norito(&owned_occurrences)?,
    );
    Ok(())
}

/// Validate proof-bound metadata, requiring its presence for anchored spends.
pub(super) fn validate_bound_occurrences(
    batch: &TransitionBatch,
    binding: &AxtFastpqBinding,
    source_tx_index: Option<u32>,
    claimed_occurrence: Option<&AxtSourceTransferOccurrenceV1>,
) -> Result<Option<[u8; 32]>> {
    let encoded = batch
        .metadata
        .get(AXT_FASTPQ_SOURCE_TRANSFER_OCCURRENCES_METADATA_KEY);
    if binding.remote_spend_intent_commitments.is_empty() {
        if encoded.is_some() {
            return Err(invalid(
                "source transfer occurrences are forbidden without remote-spend commitments",
            ));
        }
        return Ok(None);
    }
    let encoded = encoded.ok_or_else(|| Error::MissingMetadata {
        key: AXT_FASTPQ_SOURCE_TRANSFER_OCCURRENCES_METADATA_KEY.to_owned(),
    })?;
    let occurrences: Vec<AxtSourceTransferOccurrenceV1> = norito::decode_canonical(encoded)
        .map_err(|_| invalid("source transfer occurrence metadata must be canonical Norito"))?;
    validate_occurrences(
        batch,
        binding,
        &occurrences,
        source_tx_index,
        claimed_occurrence,
    )
}

fn validate_occurrences(
    batch: &TransitionBatch,
    binding: &AxtFastpqBinding,
    occurrences: &[AxtSourceTransferOccurrenceV1],
    source_tx_index: Option<u32>,
    claimed_occurrence: Option<&AxtSourceTransferOccurrenceV1>,
) -> Result<Option<[u8; 32]>> {
    let encoded_claims = batch
        .metadata
        .get(AXT_FASTPQ_REMOTE_SPEND_CLAIMS_METADATA_KEY)
        .ok_or_else(|| Error::MissingMetadata {
            key: AXT_FASTPQ_REMOTE_SPEND_CLAIMS_METADATA_KEY.to_owned(),
        })?;
    let claims = decode_bound_remote_spend_claims(encoded_claims, binding)?;
    let transcripts =
        decode_transcripts(&batch.metadata)?.ok_or_else(|| Error::MissingMetadata {
            key: TRANSFER_TRANSCRIPTS_METADATA_KEY.to_owned(),
        })?;
    let public_transcripts =
        crate::gadgets::public_transfer_statement::public_claims_from_transcripts(
            &transcripts,
            crate::gadgets::public_transfer_statement::PublicTransferLimits::default(),
        )?;
    validate_public_occurrences(
        binding,
        &public_transcripts,
        &claims,
        occurrences,
        source_tx_index,
        claimed_occurrence,
    )
}

/// Validate exact claimed occurrences against the complete public transfer statement.
pub(crate) fn validate_public_occurrences(
    binding: &AxtFastpqBinding,
    transcripts: &[FastpqPublicTransferTranscriptV1],
    claims: &[AxtRemoteSpendClaimV1],
    occurrences: &[AxtSourceTransferOccurrenceV1],
    source_tx_index: Option<u32>,
    claimed_occurrence: Option<&AxtSourceTransferOccurrenceV1>,
) -> Result<Option<[u8; 32]>> {
    if binding.remote_spend_intent_commitments.is_empty() {
        if !occurrences.is_empty() || !claims.is_empty() || claimed_occurrence.is_some() {
            return Err(invalid(
                "source transfer occurrences are forbidden without remote-spend commitments",
            ));
        }
        return Ok(None);
    }
    if claims.len() != binding.remote_spend_intent_commitments.len() {
        return Err(invalid("source transfer claim count differs from binding"));
    }
    if occurrences.len() != claims.len() || occurrences.is_empty() {
        return Err(invalid(
            "source transfer occurrences must cover every remote-spend claim exactly once",
        ));
    }
    validate_claimed_source_tx_index(occurrences, source_tx_index)?;
    let source_execution =
        decode_hex_digest(&binding.source_tx_commitment, "source_tx_commitment")?;
    let pair_count = transcripts.iter().try_fold(0_usize, |count, transcript| {
        count
            .checked_add(transcript.deltas.len())
            .ok_or_else(|| invalid("source transfer count overflows host size"))
    })?;
    if pair_count > MAX_REMOTE_SPEND_INTENT_COMMITMENTS_V1 {
        return Err(Error::VerifierLimitExceeded {
            limit: "max_source_transfer_occurrences_v1",
            actual: pair_count,
            max: MAX_REMOTE_SPEND_INTENT_COMMITMENTS_V1,
        });
    }
    if pair_count != occurrences.len() {
        return Err(invalid(
            "source transfer occurrences do not cover every transcript delta",
        ));
    }
    let mut ordered_pairs = Vec::new();
    ordered_pairs
        .try_reserve_exact(pair_count)
        .map_err(|_| Error::LocalAllocationUnavailable {
            context: "source transfer occurrence index",
        })?;
    for (transcript_index, transcript) in transcripts.iter().enumerate() {
        if transcript.batch_hash.as_ref() != &source_execution {
            return Err(invalid(
                "source transfer transcript belongs to another execution",
            ));
        }
        let transcript_index = u32::try_from(transcript_index)
            .map_err(|_| invalid("source transcript index exceeds u32"))?;
        for (delta_index, delta) in transcript.deltas.iter().enumerate() {
            ordered_pairs.push((
                transcript_index,
                u32::try_from(delta_index)
                    .map_err(|_| invalid("source delta index exceeds u32"))?,
                delta,
            ));
        }
    }
    let mut seen = Vec::new();
    seen.try_reserve_exact(pair_count)
        .map_err(|_| Error::LocalAllocationUnavailable {
            context: "source transfer occurrence occupancy",
        })?;
    seen.resize(pair_count, false);
    let mut receipt_digest = None;
    let mut found_claimed_occurrence = false;
    for ((claim, occurrence), expected_claim_commitment) in claims
        .iter()
        .zip(occurrences)
        .zip(&binding.remote_spend_intent_commitments)
    {
        validate_one(
            claim,
            occurrence,
            expected_claim_commitment,
            &source_execution,
            &ordered_pairs,
            &mut seen,
        )?;
        if claimed_occurrence.is_some_and(|claimed| claimed == occurrence) {
            found_claimed_occurrence = true;
        }
        if receipt_digest.is_some_and(|digest| digest != occurrence.source_success_receipt_digest) {
            return Err(invalid(
                "source transfer occurrences disagree on successful-execution receipt",
            ));
        }
        receipt_digest = Some(occurrence.source_success_receipt_digest);
    }
    if claimed_occurrence.is_some() && !found_claimed_occurrence {
        return Err(invalid(
            "claimed source transfer occurrence is absent from proof-bound metadata",
        ));
    }
    Ok(receipt_digest)
}

fn validate_claimed_source_tx_index(
    occurrences: &[AxtSourceTransferOccurrenceV1],
    authoritative_index: Option<u32>,
) -> Result<()> {
    let first = occurrences
        .first()
        .ok_or_else(|| invalid("source transfer occurrences cannot be empty"))?;
    if occurrences
        .iter()
        .any(|occurrence| occurrence.source_tx_index != first.source_tx_index)
    {
        return Err(invalid(
            "source transfer occurrences disagree on source transaction index",
        ));
    }
    if authoritative_index.is_some_and(|index| first.source_tx_index != index) {
        return Err(invalid(
            "source transfer occurrence transaction index differs from finalized wires",
        ));
    }
    Ok(())
}

fn validate_one(
    claim: &AxtRemoteSpendClaimV1,
    occurrence: &AxtSourceTransferOccurrenceV1,
    expected_claim_commitment: &[u8; 32],
    source_execution: &[u8; 32],
    ordered_pairs: &[(u32, u32, &FastpqPublicTransferDeltaV1)],
    seen: &mut [bool],
) -> Result<()> {
    occurrence
        .validate()
        .map_err(|error| invalid(format!("invalid source transfer occurrence: {error}")))?;
    if &occurrence.source_tx_commitment != source_execution {
        return Err(invalid(
            "source transfer occurrence execution commitment differs from proof binding",
        ));
    }
    if &occurrence.remote_spend_claim_commitment != expected_claim_commitment {
        return Err(invalid(
            "source transfer occurrence belongs to another handle/intent claim",
        ));
    }
    let pair_index = usize::try_from(occurrence.pair_ordinal)
        .map_err(|_| invalid("source transfer pair ordinal exceeds host index"))?;
    let occupied = seen
        .get_mut(pair_index)
        .ok_or_else(|| invalid("source transfer pair ordinal is out of range"))?;
    if std::mem::replace(occupied, true) {
        return Err(invalid("source transfer occurrence is duplicated"));
    }
    let (transcript_index, delta_index, delta) = ordered_pairs
        .get(pair_index)
        .ok_or_else(|| invalid("source transfer pair ordinal is out of range"))?;
    if (*transcript_index, *delta_index) != (occurrence.transcript_index, occurrence.delta_index) {
        return Err(invalid(
            "source transfer coordinate differs from its ordered pair ordinal",
        ));
    }
    if occurrence.transfer_digest != axt_source_transfer_digest_v1(delta) {
        return Err(invalid(
            "source transfer digest differs from its exact public transcript delta",
        ));
    }
    if claim.asset_definition_id != delta.asset_definition
        || canonical_remote_account(&claim.from, "from")? != delta.from_account
        || canonical_remote_account(&claim.to, "to")? != delta.to_account
        || claim.effective_amount != delta.amount
    {
        return Err(invalid(
            "source transfer occurrence facts differ from its handle/intent claim",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn occurrence(source_tx_index: u32) -> AxtSourceTransferOccurrenceV1 {
        AxtSourceTransferOccurrenceV1 {
            source_tx_commitment: [1; 32],
            source_success_receipt_digest: [2; 32],
            source_tx_index,
            transcript_index: 0,
            delta_index: 0,
            pair_ordinal: 0,
            transfer_digest: [3; 32],
            remote_spend_claim_commitment: [4; 32],
        }
    }

    #[test]
    fn claimed_source_index_is_uniform_and_matches_finalized_wires_when_available() {
        let first = occurrence(2);
        assert!(validate_claimed_source_tx_index(&[first, first], None).is_ok());
        assert!(validate_claimed_source_tx_index(&[first, first], Some(2)).is_ok());
        assert!(matches!(
            validate_claimed_source_tx_index(&[first, occurrence(3)], None),
            Err(Error::InvalidAxtBinding { details })
                if details.contains("disagree on source transaction index")
        ));
        assert!(matches!(
            validate_claimed_source_tx_index(&[first], Some(3)),
            Err(Error::InvalidAxtBinding { details })
                if details.contains("differs from finalized wires")
        ));
        assert!(validate_claimed_source_tx_index(&[], None).is_err());
    }
}

#[cfg(test)]
pub(crate) fn test_occurrences(
    transcripts: &[FastpqPublicTransferTranscriptV1],
    claims: &[AxtRemoteSpendClaimV1],
) -> Vec<AxtSourceTransferOccurrenceV1> {
    let pairs: Vec<_> = transcripts
        .iter()
        .enumerate()
        .flat_map(|(ti, transcript)| {
            transcript
                .deltas
                .iter()
                .enumerate()
                .map(move |(di, delta)| (ti, di, transcript.batch_hash, delta))
        })
        .collect();
    let mut used = vec![false; pairs.len()];
    claims
        .iter()
        .map(|claim| {
            let (ordinal, (ti, di, hash, delta)) = pairs
                .iter()
                .enumerate()
                .find(|(i, (_, _, _, delta))| {
                    !used[*i]
                        && claim.asset_definition_id == delta.asset_definition
                        && claim.from == delta.from_account.to_string()
                        && claim.to == delta.to_account.to_string()
                        && claim.effective_amount == delta.amount
                })
                .expect("one matching fixture occurrence");
            used[ordinal] = true;
            AxtSourceTransferOccurrenceV1 {
                source_success_receipt_digest: [0x51; 32],
                source_tx_commitment: (*hash).into(),
                source_tx_index: 0,
                transcript_index: *ti as u32,
                delta_index: *di as u32,
                pair_ordinal: ordinal as u32,
                transfer_digest: axt_source_transfer_digest_v1(delta),
                remote_spend_claim_commitment:
                    iroha_data_model::nexus::compute_remote_spend_claim_commitment_v1(claim),
            }
        })
        .collect()
}
