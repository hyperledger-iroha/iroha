//! Exact successful-source occurrence binding in the sole compact AXT context.

use super::*;

fn receipt_fixture() -> (
    TransitionBatch,
    AxtFastpqBinding,
    AxtFinalizedSpendAnchorV1,
    Vec<TransactionEntrypoint>,
    AxtSourceSuccessReceiptV1,
    AxtSourceTransferOccurrenceV1,
) {
    let transactions = vec![finalized_transaction(81), finalized_transaction(82)];
    let mut binding = remote_transfer_binding();
    binding.source_tx_commitment = hex::encode(transactions[0].execution_call_hash().as_ref());
    let mut batch = real_transfer_claim_batch(&binding);
    let anchor = finalized_test_anchor(batch.public_inputs, &transactions);
    batch.public_inputs.tx_set_hash = anchor.transaction_set_digest.into();
    let receipt = AxtSourceSuccessReceiptV1 {
        finalized_anchor_digest: anchor.digest_v1(),
        source_tx_commitment: *transactions[0].execution_call_hash().as_ref(),
        source_tx_index: 0,
        post_transaction_state_root: batch.public_inputs.new_root,
        effect_set_digest: Hash::new(b"test source effect set").into(),
    };
    let mut occurrences: Vec<AxtSourceTransferOccurrenceV1> = norito::decode_canonical(
        &batch.metadata[AXT_FASTPQ_SOURCE_TRANSFER_OCCURRENCES_METADATA_KEY],
    )
    .unwrap();
    occurrences[0].source_success_receipt_digest = receipt.digest_v1();
    batch.metadata.remove(AXT_FASTPQ_BATCH_SEAL_METADATA_KEY);
    set_axt_source_transfer_occurrences(&mut batch, &binding, &occurrences).unwrap();
    bind_axt_batch_with_proof_metadata(
        &mut batch,
        &binding,
        [0x42; 32],
        Some(anchor.da_manifest_digest.into()),
        None,
        Some(100),
    )
    .unwrap();
    (
        batch,
        binding,
        anchor,
        transactions,
        receipt,
        occurrences[0],
    )
}

#[test]
fn source_occurrences_require_every_exact_public_field_and_complete_coverage() {
    let (batch, binding, _, _, receipt, occurrence) = receipt_fixture();
    assert_eq!(
        source_occurrence::validate_bound_occurrences(&batch, &binding, Some(0), Some(&occurrence))
            .unwrap(),
        Some(receipt.digest_v1())
    );
    let artifact = compact::prepare(&batch, &binding).unwrap();
    let claims = artifact.remote_spend_claims.as_deref().unwrap();
    let validate = |occurrences: &[AxtSourceTransferOccurrenceV1], claimed| {
        source_occurrence::validate_public_occurrences(
            &binding,
            &artifact.statement.transcripts,
            claims,
            occurrences,
            Some(0),
            claimed,
        )
    };
    assert!(validate(&[], None).is_err());
    assert!(validate(&[occurrence, occurrence], None).is_err());
    for field in 0..7 {
        let mut changed = occurrence;
        match field {
            0 => changed.source_tx_index += 1,
            1 => changed.transcript_index += 1,
            2 => changed.delta_index += 1,
            3 => changed.pair_ordinal += 1,
            4 => changed.transfer_digest[0] ^= 1,
            5 => changed.source_tx_commitment[0] ^= 1,
            6 => changed.remote_spend_claim_commitment[0] ^= 1,
            _ => unreachable!(),
        }
        assert!(validate(&[changed], None).is_err(), "field {field}");
    }
    let mut different_receipt = occurrence;
    different_receipt.source_success_receipt_digest[0] ^= 1;
    assert!(validate(&[different_receipt], Some(&occurrence)).is_err());
    let mut missing = batch.clone();
    missing
        .metadata
        .remove(AXT_FASTPQ_SOURCE_TRANSFER_OCCURRENCES_METADATA_KEY);
    assert!(source_occurrence::validate_bound_occurrences(&missing, &binding, None, None).is_err());
    let mut sealed = batch.clone();
    assert!(set_axt_source_transfer_occurrences(&mut sealed, &binding, &[occurrence]).is_err());
}

#[test]
fn compact_metadata_roundtrips_exact_receipt_and_occurrence_claims() {
    let (batch, binding, _, _, receipt, occurrence) = receipt_fixture();
    let artifact = compact::prepare(&batch, &binding).unwrap();
    assert_eq!(
        artifact.metadata.source_transfer_occurrences,
        vec![occurrence]
    );
    assert_eq!(
        artifact.metadata.source_transfer_occurrences[0].source_success_receipt_digest,
        receipt.digest_v1()
    );
    let bytes = encode_canonical_norito(&artifact).unwrap();
    assert_eq!(compact::decode(&bytes).unwrap(), artifact);
    let mut altered = artifact.clone();
    altered.metadata.source_transfer_occurrences[0].source_success_receipt_digest[0] ^= 1;
    assert_ne!(encode_canonical_norito(&altered).unwrap(), bytes);
}

#[test]
fn claimed_source_verifier_rejects_receipt_substitution_before_child_verification() {
    let (batch, binding, anchor, transactions, receipt, occurrence) = receipt_fixture();
    let artifact = compact::prepare(&batch, &binding).unwrap();
    let mut envelope = envelope_with_payload(binding, encode_canonical_norito(&artifact).unwrap());
    envelope.da_commitment = Some(anchor.da_manifest_digest.into());
    for field in 0..5 {
        let mut changed = receipt;
        match field {
            0 => changed.finalized_anchor_digest[0] ^= 1,
            1 => changed.source_tx_commitment[0] ^= 1,
            2 => changed.source_tx_index += 1,
            3 => changed.post_transaction_state_root[0] ^= 1,
            4 => changed.effect_set_digest[0] ^= 1,
            _ => unreachable!(),
        }
        let error = verify_axt_proof_envelope_against_anchor_and_claimed_source_v1(
            &envelope,
            Some(100),
            &anchor,
            &transactions,
            &changed,
            &occurrence,
        )
        .unwrap_err();
        assert!(
            matches!(error, Error::InvalidAxtBinding { details } if details.contains("claimed AXT source success receipt"))
        );
    }
    // A complete source claim still cannot turn an empty child into an execution proof.
    assert!(
        verify_axt_proof_envelope_against_anchor_and_claimed_source_v1(
            &envelope,
            Some(100),
            &anchor,
            &transactions,
            &receipt,
            &occurrence
        )
        .is_err()
    );
}
