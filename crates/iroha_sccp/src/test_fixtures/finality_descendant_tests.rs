//! Exact test-only finality descendants retain the opaque parent, fixed epoch and real BLS votes.

use super::*;
use iroha_data_model::block::builder::BlockBuilder;

fn block(height: u64, parent: Option<&SccpFinalizedBlockTestFixtureV1>) -> SignedBlock {
    let key = KeyPair::try_from_seed(vec![0x73; 32], Algorithm::Ed25519).unwrap();
    BlockBuilder::new(BlockHeader::new(
        height.try_into().unwrap(),
        parent.map(|p| p.block().hash()),
        None,
        1_700_000_000_000 + height,
        0,
    ))
    .try_build_with_signature(0, key.private_key())
    .unwrap()
}

#[test]
fn exact_same_epoch_descendants_authenticate_through_the_last_nonboundary_height() {
    let mut parent = sccp_finalize_taira_block_test_fixture_v1(&block(1, None), None);
    for height in 2..=9 {
        let child =
            sccp_finalize_taira_block_test_fixture_v1(&block(height, Some(&parent)), Some(&parent));
        let artifact = &child.proof().finality_artifact;
        artifact.verify().unwrap();
        assert_eq!(
            artifact.height_context.epoch,
            parent.proof().finality_artifact.height_context.epoch
        );
        assert_eq!(
            artifact.height_context.parent_commit_qc.as_ref(),
            Some(&parent.proof().finality_artifact.commit_qc)
        );
        assert_eq!(artifact.height_context.roster.len(), 4);
        assert_eq!(artifact.commit_qc.signers.len(), 3);
        assert_eq!(
            artifact.height_context.da_layout.encoding,
            PayloadEncoding::ReedSolomon16
        );
        assert_exact_finalized_block_fixture(&child);
        parent = child;
    }
    for height in [10, 11] {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                sccp_finalize_taira_block_test_fixture_v1(
                    &block(height, Some(&parent)),
                    Some(&parent),
                )
            }))
            .is_err()
        );
    }
}

#[test]
fn descendant_signer_rejects_missing_skipped_and_substituted_parents() {
    let parent = sccp_finalize_taira_block_test_fixture_v1(&block(1, None), None);
    let child = sccp_finalize_taira_block_test_fixture_v1(&block(2, Some(&parent)), Some(&parent));
    for (candidate, supplied) in [
        (block(3, Some(&child)), None),
        (block(3, Some(&parent)), Some(&parent)),
        (block(3, Some(&parent)), Some(&child)),
    ] {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                sccp_finalize_taira_block_test_fixture_v1(&candidate, supplied)
            }))
            .is_err()
        );
    }
}
