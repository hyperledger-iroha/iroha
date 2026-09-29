//! Bounded consecutive native proof pages extending an independently selected checkpoint.
use super::*;

/// Exact native decision identity `H(tag || core_hash || result)`.
/// This digest alone does not authenticate its inputs or create a trust root.
#[must_use]
pub fn certified_block_context_id(core_hash: &Hash32, result: &Hash32) -> Hash {
    const TAG: &[u8] = b"iroha/sumeragi/certified-block/v1";
    let mut bytes = Vec::with_capacity(TAG.len() + 64);
    bytes.extend_from_slice(TAG);
    bytes.extend_from_slice(&core_hash.0);
    bytes.extend_from_slice(&result.0);
    Hash::new(bytes)
}

/// Verified page tip and the restart checkpoint derived from that same authenticated prefix.
/// Neither field can be supplied by an unverified response.
#[derive(Debug, Clone)]
pub struct VerifiedFinalityPage {
    tip: VerifiedSumeragiBlock,
    checkpoint: SumeragiFinalityCheckpoint,
}
impl VerifiedFinalityPage {
    /// Last block authenticated from the caller's exact checkpoint.
    #[must_use]
    pub fn tip(&self) -> &VerifiedSumeragiBlock {
        &self.tip
    }
    /// Bounded checkpoint suitable for durable promotion after all application proofs succeed.
    #[must_use]
    pub fn checkpoint(&self) -> &SumeragiFinalityCheckpoint {
        &self.checkpoint
    }
    /// Consume this verified page to retain its authenticated checkpoint.
    #[must_use]
    pub fn into_checkpoint(self) -> SumeragiFinalityCheckpoint {
        self.checkpoint
    }
}

/// Verify a page beginning with the caller's exact independently authenticated checkpoint.
///
/// The first proof reauthenticates that decision, including any alternate quorum witness;
/// every following proof extends it consecutively. No supplied response field can select
/// the trust root, instance, authority generation, or previous result. Nothing mutates the
/// caller's checkpoint on failure. Application callers must verify their own witnesses
/// before persisting the returned checkpoint.
///
/// # Errors
/// Invalid bounds, wrong network, absent checkpoint, gaps, altered decisions or failed signatures.
pub fn verify_checkpoint_page(
    network_id: NetworkId,
    checkpoint: &SumeragiFinalityCheckpoint,
    proofs: &[SumeragiFinalityProof],
    max_proofs: usize,
    max_bytes: usize,
) -> Result<VerifiedFinalityPage, FinalityError> {
    need(
        max_proofs > 0
            && max_proofs <= crate::sumeragi::finality::NATIVE_FINALITY_MAX_BLOCK_COUNT
            && max_bytes > 0
            && max_bytes <= crate::sumeragi::finality::NATIVE_FINALITY_MAX_JOURNAL_BYTES,
        "native proof page limits are invalid",
    )?;
    need(
        !proofs.is_empty() && proofs.len() <= max_proofs,
        "native proof page is empty or exceeds its count bound",
    )?;
    check_page_byte_bound(proofs, max_bytes)?;
    norito::core::with_decode_limits_scope(norito::canonical_decode_limits(max_bytes), || {
        let mut verifier = SumeragiFinalityVerifier::from_trusted_checkpoint(
            checkpoint,
            &network_id,
            checkpoint.chain_id(),
        )?;
        let first = &proofs[0];
        need(
            first.height() == checkpoint.height(),
            "native proof page does not start at the pinned checkpoint",
        )?;
        let mut tip = verifier.verify_same_decision(checkpoint.tip(), first)?;
        for proof in &proofs[1..] {
            tip = verifier.verify(proof)?;
        }
        let checkpoint =
            verifier.export_checkpoint(proofs.last().expect("non-empty native page"))?;
        Ok(VerifiedFinalityPage { tip, checkpoint })
    })
}

// Measure the borrowed generic sequence with the codec's exact Vec framing. No
// payload buffer or proof clones are needed, and each prefix is checked before
// measuring another proof.
fn check_page_byte_bound(
    proofs: &[SumeragiFinalityProof],
    max_bytes: usize,
) -> Result<(), FinalityError> {
    let mut measured =
        norito::core::SequencePayloadLength::new(norito::core::default_encode_flags())
            .map_err(malformed)?;
    let max_payload = max_bytes
        .checked_sub(norito::core::Header::SIZE)
        .ok_or_else(|| FinalityError("native proof page exceeds its byte bound".into()))?;
    need(
        measured.len() <= max_payload,
        "native proof page exceeds its byte bound",
    )?;
    for proof in proofs {
        measured.push(proof).map_err(malformed)?;
        need(
            measured.len() <= max_payload,
            "native proof page exceeds its byte bound",
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::tests::Fixture;
    use super::*;
    const CAP: usize = 3 * 1024 * 1024;

    fn checkpoint(f: &Fixture) -> SumeragiFinalityCheckpoint {
        let mut verifier = f.verifier();
        verifier.verify(&f.first).unwrap();
        verifier.export_checkpoint(&f.first).unwrap()
    }
    #[test]
    fn native_page_preserves_exact_context_and_promotes_authenticated_tip() {
        let f = Fixture::new();
        let checkpoint = checkpoint(&f);
        let page = verify_checkpoint_page(
            f.network,
            &checkpoint,
            &[f.first.clone(), f.second.clone()],
            64,
            CAP,
        )
        .unwrap();
        assert_eq!(page.tip().height(), 2);
        assert_eq!(
            page.tip().context_id(),
            certified_block_context_id(&page.tip().core_hash(), &page.tip().result())
        );
        assert_eq!(page.checkpoint().height(), 2);
        assert_eq!(checkpoint.height(), 1);
        let promoted = page.into_checkpoint();
        verify_checkpoint_page(f.network, &promoted, &[f.second.clone()], 64, CAP).unwrap();
    }
    #[test]
    fn native_page_rejects_missing_anchor_wrong_network_gaps_and_tampering() {
        let f = Fixture::new();
        let checkpoint = checkpoint(&f);
        for proofs in [
            vec![],
            vec![f.second.clone()],
            vec![f.first.clone(), f.first.clone()],
            vec![f.second.clone(), f.first.clone()],
        ] {
            assert!(verify_checkpoint_page(f.network, &checkpoint, &proofs, 64, CAP).is_err());
        }
        let foreign = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"foreign native network",
        )));
        assert!(verify_checkpoint_page(foreign, &checkpoint, &[f.first.clone()], 64, CAP).is_err());
        let mut changed = f.second.clone();
        let last = changed.block_wire.len() - 1;
        changed.block_wire[last] ^= 1;
        assert!(
            verify_checkpoint_page(f.network, &checkpoint, &[f.first.clone(), changed], 64, CAP)
                .is_err()
        );
        assert!(
            verify_checkpoint_page(f.network, &checkpoint, &[f.first.clone()], 0, CAP).is_err()
        );
        assert!(verify_checkpoint_page(f.network, &checkpoint, &[f.first.clone()], 64, 1).is_err());
        assert_eq!(checkpoint.height(), 1);
    }
    #[test]
    fn borrowed_page_measurement_matches_canonical_vec_frame_exactly() {
        let f = Fixture::new();
        for proofs in [vec![], vec![f.first.clone()], vec![f.first, f.second]] {
            let frame = norito::to_bytes(&proofs).unwrap();
            check_page_byte_bound(&proofs, frame.len()).unwrap();
            assert!(check_page_byte_bound(&proofs, frame.len() - 1).is_err());
        }
    }
    #[test]
    fn native_context_identity_binds_both_original_commitments() {
        let hash = Hash32([1; 32]);
        let result = Hash32([2; 32]);
        let id = certified_block_context_id(&hash, &result);
        assert_ne!(id, certified_block_context_id(&Hash32([3; 32]), &result));
        assert_ne!(id, certified_block_context_id(&hash, &Hash32([3; 32])));
    }
}
