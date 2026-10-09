//! Cheap exact-native continuity checks before any recursive generator work.
use super::*;

pub(super) fn verify_history(
    genesis: &SumeragiFinalityVerifier,
    signed_original: &[u8],
    proofs: &[SumeragiFinalityProof],
) -> Result<Vec<VerifiedSumeragiBlock>, String> {
    if proofs.len() != usize::try_from(REGISTER_HEIGHT + 1).unwrap() {
        return Err("exact H1-H6 capture required".into());
    }
    let signed = decode_framed_signed_block(signed_original).map_err(|e| e.to_string())?;
    if !signed.header().is_genesis()
        || !signed.is_resultless_proposal()
        || signed.encode_wire().map_err(|e| e.to_string())? != signed_original
    {
        return Err("noncanonical selected signed genesis".into());
    }
    let mut verifier = genesis.clone();
    let mut blocks = Vec::with_capacity(proofs.len());
    for (index, proof) in proofs.iter().enumerate() {
        if proof.height() != u64::try_from(index).unwrap() + 1 {
            return Err("capture proof height differs".into());
        }
        // The native owner checks the independently selected root, exact result-bearing
        // wire commitments, committees, quorums, availability and each parent/result join.
        let block = verifier.verify(proof).map_err(|e| e.to_string())?;
        if index == 0 {
            // H1 execution attaches a result-only certificate and output material. Bind
            // its unchanged signed proposal to the separately pinned genesis original,
            // using the same canonical projection as the native finality owner. The
            // complete executed wire is still verified above; it is never substituted.
            if block.block().hash() != signed.hash()
                || block
                    .block()
                    .canonical_resultless_proposal()
                    .map_err(|e| e.to_string())?
                    .encode_wire()
                    .map_err(|e| e.to_string())?
                    != signed_original
            {
                return Err("captured genesis differs from authenticated signed root".into());
            }
        }
        blocks.push(block);
    }
    Ok(blocks)
}

#[test]
fn executed_genesis_and_signed_original_have_the_same_authenticated_proposal() {
    use iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture;
    let mut fixture = NativeFinalityFixture::start("registration-preflight");
    let genesis = SumeragiFinalityVerifier::new(
        fixture.genesis(),
        fixture.chain_id(),
        fixture.genesis_proof().committee.clone(),
    )
    .unwrap();
    let signed = fixture.genesis().encode_wire().unwrap();
    let mut proofs = vec![fixture.genesis_proof().clone()];
    for _ in 2..=REGISTER_HEIGHT + 1 {
        let block = fixture.block_with_submitted_work(fixture.next_header());
        proofs.push(fixture.certify(block));
    }
    assert_ne!(
        proofs[0].block_wire, signed,
        "executed result attachment differs from signed original"
    );
    let blocks = verify_history(&genesis, &signed, &proofs).unwrap();
    assert_eq!(
        blocks
            .iter()
            .map(VerifiedSumeragiBlock::height)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5, 6]
    );
    for (block, original) in blocks.iter().zip(&proofs) {
        assert_eq!(
            block.block().encode_wire().unwrap(),
            original.block_wire,
            "native execution originals remain exact after preflight"
        );
    }
    // Native certification here is a structural fixture, not actual World/Register
    // execution. The actual capture path separately verifies the H5 Register join.
    let mut missing = proofs.clone();
    missing.pop();
    assert!(verify_history(&genesis, &signed, &missing).is_err());
    let mut reordered = proofs.clone();
    reordered.swap(4, 5);
    assert!(verify_history(&genesis, &signed, &reordered).is_err());
    let mut malformed = proofs.clone();
    malformed[5].block_wire[0] ^= 1;
    assert!(verify_history(&genesis, &signed, &malformed).is_err());
    let mut forged = proofs.clone();
    let mut block = decode_framed_signed_block(&forged[0].block_wire).unwrap();
    block.set_commit_certificate(None);
    forged[0].block_wire = block.encode_wire().unwrap();
    assert!(verify_history(&genesis, &signed, &forged).is_err());
    assert!(
        verify_history(&genesis, &proofs[0].block_wire, &proofs).is_err(),
        "executed H1 cannot be relabeled as the selected signed original"
    );
    let mut trailing = signed.clone();
    trailing.push(0);
    assert!(verify_history(&genesis, &trailing, &proofs).is_err());
    assert!(verify_history(&genesis, &proofs[1].block_wire, &proofs).is_err());
    let foreign =
        NativeFinalityFixture::start_with_explicit_parameters("foreign-registration-preflight");
    let foreign_genesis = SumeragiFinalityVerifier::new(
        foreign.genesis(),
        foreign.chain_id(),
        foreign.genesis_proof().committee.clone(),
    )
    .unwrap();
    assert!(verify_history(&foreign_genesis, &signed, &proofs).is_err());
    assert!(verify_history(&genesis, &foreign.genesis().encode_wire().unwrap(), &proofs).is_err());
}
