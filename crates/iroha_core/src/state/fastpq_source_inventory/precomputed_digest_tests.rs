//! Supplied digest validation fails before owned inventory publication or digest repair.

use super::{
    tests::{apply_source, cache_canonical_test_transaction_set, delta, header, state},
    *,
};
use crate::fastpq::{
    poseidon_preimage_digest, validate_precomputed_transfer_transcript_digests_in_map,
};
use iroha_data_model::fastpq::TransferTranscript;
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::ALICE_ID;

fn precomputed_transcript(hash: Hash) -> TransferTranscript {
    let delta = delta();
    let digest = poseidon_preimage_digest(&delta, &hash);
    TransferTranscript {
        batch_hash: hash,
        deltas: vec![delta],
        authority_digest: crate::fastpq::authority_digest(&ALICE_ID),
        poseidon_preimage_digest: Some(digest),
    }
}

#[test]
fn precomputed_validation_preserves_values_and_canonical_layout_for_valid_inputs() {
    let hash = Hash::new(b"canonical precomputed digest validation");
    let original = BTreeMap::from([(hash, vec![precomputed_transcript(hash)])]);
    assert!(validate_precomputed_transfer_transcript_digests_in_map(&BTreeMap::new()).is_ok());
    for flags in
        (u8::MIN..=u8::MAX).filter(|&flags| norito::core::validate_header_flags(flags).is_ok())
    {
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        assert!(validate_precomputed_transfer_transcript_digests_in_map(&original).is_ok());
        assert_eq!(norito::core::effective_decode_flags(), Some(flags));
        assert_eq!(original[&hash][0], precomputed_transcript(hash));
    }
}

#[test]
fn invalid_supplied_digest_is_preserved_and_failure_latches_before_publication() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    let state = state();
    crate::sumeragi::witness::start_block();
    let mut block = state.block(header());
    cache_canonical_test_transaction_set(&mut block, &[]);
    let bad_hash = Hash::new(b"invalid supplied digest");
    let missing_hash = Hash::new(b"must remain missing on rejection");
    apply_source(&mut block, bad_hash, false, None);
    apply_source(&mut block, missing_hash, false, None);
    let invalid = Hash::prehashed([0xF1; 32]);
    block.fastpq_transcripts.get_mut(&bad_hash).unwrap()[0].poseidon_preimage_digest =
        Some(invalid);
    block.fastpq_transcripts.get_mut(&missing_hash).unwrap()[0].poseidon_preimage_digest = None;
    let before = block.fastpq_transcripts.clone();
    let before_tx_set = block.fastpq_tx_set_hash;
    let before_dataspaces = block.fastpq_entry_dataspaces.clone();
    let error = block
        .finalize_fastpq_source_inventory(&[], &[], &[])
        .unwrap_err();
    assert!(error.contains("differs from its canonical preimage"));
    assert_eq!(block.fastpq_transcripts, before);
    assert_eq!(
        block.fastpq_transcripts[&bad_hash][0].poseidon_preimage_digest,
        Some(invalid)
    );
    assert!(
        block.fastpq_transcripts[&missing_hash][0]
            .poseidon_preimage_digest
            .is_none()
    );
    assert_eq!(block.fastpq_tx_set_hash, before_tx_set);
    assert_eq!(block.fastpq_entry_dataspaces, before_dataspaces);
    assert!(block.fastpq_source_captures.sealed_sources().is_err());
    assert_eq!(block.fastpq_source_inventory(), Err(error.as_str()));
    assert_eq!(
        block.verified_fastpq_source_inventory_for_capture(),
        Err(error.clone())
    );
    assert!(block.exec_witness.is_none());
    assert!(block.fastpq_witness_context.is_none());
    assert!(block.parliament_timed_ovn_casting_bindings.is_none());

    // A caller cannot repair the original failure by retrying a smaller archive.
    block.fastpq_transcripts.clear();
    assert!(
        block
            .finalize_fastpq_source_inventory(&[], &[], &[])
            .unwrap_err()
            .contains("already been finalized")
    );
    assert_eq!(block.fastpq_source_inventory(), Err(error.as_str()));
    assert_eq!(block.capture_exec_witness(), Err(error));
}

#[test]
fn supplied_valid_and_missing_digests_seal_while_multi_delta_none_stays_unchanged() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    let state = state();
    crate::sumeragi::witness::start_block();
    let mut block = state.block(header());
    cache_canonical_test_transaction_set(&mut block, &[]);
    let supplied_hash = Hash::new(b"valid supplied digest");
    let missing_hash = Hash::new(b"missing digest finalized after validation");
    let multi_hash = Hash::new(b"multi delta none policy unchanged");
    for hash in [supplied_hash, missing_hash, multi_hash] {
        apply_source(&mut block, hash, false, None);
    }
    let supplied = precomputed_transcript(supplied_hash).poseidon_preimage_digest;
    block.fastpq_transcripts.get_mut(&supplied_hash).unwrap()[0].poseidon_preimage_digest =
        supplied;
    block.fastpq_transcripts.get_mut(&missing_hash).unwrap()[0].poseidon_preimage_digest = None;
    let multi = &mut block.fastpq_transcripts.get_mut(&multi_hash).unwrap()[0];
    let mut second = delta();
    second.from_balance_before = Quantity::from(9_u32);
    second.from_balance_after = Quantity::from(8_u32);
    second.to_balance_before = Quantity::from(1_u32);
    second.to_balance_after = Quantity::from(2_u32);
    multi.deltas.push(second);
    multi.poseidon_preimage_digest = None;
    let multi_before = multi.clone();
    block
        .finalize_fastpq_source_inventory(&[], &[], &[])
        .unwrap();
    assert_eq!(
        block.fastpq_transcripts[&supplied_hash][0].poseidon_preimage_digest,
        supplied
    );
    let missing = &block.fastpq_transcripts[&missing_hash][0];
    assert_eq!(
        missing.poseidon_preimage_digest,
        Some(poseidon_preimage_digest(
            &missing.deltas[0],
            &missing.batch_hash
        ))
    );
    assert_eq!(block.fastpq_transcripts[&multi_hash][0], multi_before);
    let inventory = block.fastpq_source_inventory().unwrap().unwrap();
    assert_eq!(inventory.transcript_entry_hashes().len(), 3);
    assert_eq!(inventory.transcript_seal.transcript_count, 3);
    assert_eq!(inventory.transcript_seal.delta_count, 4);
    assert!(block.fastpq_source_captures.sealed_sources().is_ok());
}

#[test]
fn shape_failure_precedes_precomputed_digest_validation() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    let state = state();
    crate::sumeragi::witness::start_block();
    let mut block = state.block(header());
    cache_canonical_test_transaction_set(&mut block, &[]);
    let hash = Hash::new(b"shape wins over invalid supplied digest");
    apply_source(&mut block, hash, false, None);
    let transcript = &mut block.fastpq_transcripts.get_mut(&hash).unwrap()[0];
    transcript.batch_hash = Hash::new(b"misidentified transcript");
    transcript.poseidon_preimage_digest = Some(Hash::prehashed([0xF1; 32]));
    let before = block.fastpq_transcripts.clone();
    let error = block
        .finalize_fastpq_source_inventory(&[], &[], &[])
        .unwrap_err();
    assert!(error.contains("empty or misidentified transcript"));
    assert_eq!(block.fastpq_transcripts, before);
    assert_eq!(block.fastpq_source_inventory(), Err(error.as_str()));
}

#[test]
fn missing_or_zero_wire_commitment_latches_before_any_digest_finalization() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    let state = state();
    for invalid_commitment in [None, Some([0; 32])] {
        for pending_entrypoint in [false, true] {
            crate::sumeragi::witness::start_block();
            let mut block = state.block(header());
            let hash = Hash::new(b"digest must remain absent without canonical wire authority");
            apply_source(&mut block, hash, false, None);
            block.fastpq_transcripts.get_mut(&hash).unwrap()[0].poseidon_preimage_digest = None;
            block.fastpq_tx_set_hash = invalid_commitment;
            let before = block.fastpq_transcripts.clone();
            let before_sources = block.captured_fastpq_transcript_sources().unwrap().clone();
            let error = if pending_entrypoint {
                block.finalize_fastpq_source_inventory_with_pending(&[], &[], &[], None)
            } else {
                block.finalize_fastpq_source_inventory(&[], &[], &[])
            }
            .unwrap_err();
            assert!(error.contains("ordered transaction-wire commitment"));
            assert_eq!(block.fastpq_transcripts, before);
            assert_eq!(block.fastpq_tx_set_hash, invalid_commitment);
            assert_eq!(
                block.captured_fastpq_transcript_sources().unwrap(),
                &before_sources
            );
            assert!(block.fastpq_source_captures.sealed_sources().is_err());
            assert_eq!(block.fastpq_source_inventory(), Err(error.as_str()));
            assert!(block.fastpq_entry_dataspaces.is_empty());

            // Supplying the missing authority later cannot repair a latched failure.
            cache_canonical_test_transaction_set(&mut block, &[]);
            assert!(
                block
                    .finalize_fastpq_source_inventory_with_pending(&[], &[], &[], None)
                    .unwrap_err()
                    .contains("already been finalized")
            );
            assert_eq!(block.fastpq_transcripts, before);
            assert_eq!(block.fastpq_source_inventory(), Err(error.as_str()));
            assert_eq!(block.capture_exec_witness(), Err(error));
            let _ = crate::sumeragi::witness::drain_exec_witness();
        }
    }
}
