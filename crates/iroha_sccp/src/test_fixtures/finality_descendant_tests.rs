//! Exact test-only finality descendants retain the opaque parent, fixed epoch and real BLS votes.

use super::*;
use iroha_data_model::block::builder::BlockBuilder;

fn block(height: u64, parent: Option<&SccpFinalizedBlockTestFixtureV1>) -> SignedBlock {
    let key = KeyPair::try_from_seed(vec![0x73; 32], Algorithm::Ed25519).unwrap();
    let mut block = BlockBuilder::new(BlockHeader::new(
        height.try_into().unwrap(),
        parent.map(|p| p.block().hash()),
        None,
        1_700_000_000_000 + height,
        0,
    ))
    .try_build_with_signature(0, key.private_key())
    .unwrap();
    let proposal = block.canonical_resultless_proposal();
    block
        .set_execution_outputs(
            Vec::new(),
            0,
            Default::default(),
            Vec::new(),
            Default::default(),
            Default::default(),
            Vec::new(),
            &exact_fixture_output_limits(),
        )
        .expect("complete empty execution matches the fixture's zero inputs");
    assert!(block.has_results());
    assert_eq!(block.canonical_resultless_proposal(), proposal);
    block
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
        assert_eq!(
            artifact
                .height_context
                .kagemusha_mint_finality_authorization,
            parent
                .proof()
                .finality_artifact
                .height_context
                .kagemusha_mint_finality_authorization,
            "same-epoch descendants must inherit the exact certified schedule"
        );
        assert_eq!(
            artifact.height_context.kagemusha_mint_finality_authority,
            parent
                .proof()
                .finality_artifact
                .height_context
                .kagemusha_mint_finality_authority,
            "same-epoch descendants must inherit the exact immutable key generation"
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
fn exact_epoch_one_successor_inherits_certified_boundary_and_parent_commit() {
    let parent = sccp_finalize_taira_epoch_boundary_test_fixture_v1(&block(1, None));
    let parent_context = &parent.proof().finality_artifact.height_context;
    let snapshot = parent_context
        .next_epoch_snapshot
        .as_ref()
        .expect("height-one boundary certifies the next epoch");
    assert_eq!(parent_context.epoch, 0);
    assert_eq!(snapshot.epoch, 1);
    assert_eq!(snapshot.epoch_end_height, 10);

    let child = sccp_finalize_taira_block_test_fixture_v1(&block(2, Some(&parent)), Some(&parent));
    let context = &child.proof().finality_artifact.height_context;
    assert_eq!(context.epoch, snapshot.epoch);
    assert_eq!(context.epoch_end_height, snapshot.epoch_end_height);
    assert_eq!(
        context.kagemusha_mint_finality_authorization,
        snapshot.kagemusha_mint_finality_authorization
    );
    assert_eq!(
        context.parent_commit_qc.as_ref(),
        Some(&parent.proof().finality_artifact.commit_qc)
    );
    assert_exact_finalized_block_fixture(&child);
}

#[test]
fn genesis_mint_authority_is_network_bound_and_rejects_schedule_or_key_substitution() {
    use iroha_data_model::isi::kagemusha_v1::{
        BeaconEpochBindingV1, KagemushaMintFinalityEpochDecisionV1,
    };
    let fixture = sccp_finalize_taira_block_test_fixture_v1(&block(1, None), None);
    let context = &fixture.proof().finality_artifact.height_context;
    context
        .validate()
        .expect("valid canonical SCCP genesis context");
    let authority = &context.kagemusha_mint_finality_authority;
    let authorization = &context.kagemusha_mint_finality_authorization;
    assert_eq!(authority.generation, 0);
    assert_eq!(authorization.epoch, 0);
    assert_eq!(
        (authorization.first_height, authorization.last_height),
        (1, 10)
    );
    assert_eq!(authorization.beacon, BeaconEpochBindingV1::Bootstrap);
    assert_eq!(
        authorization.decision,
        KagemushaMintFinalityEpochDecisionV1::Genesis
    );
    assert_eq!(authorization.previous_authorization_id, [0; 32]);
    assert_eq!(authorization.transition_id, [0; 32]);
    assert_eq!(authority.network_id, context.network_id);
    authorization.validate_against_authority(authority).unwrap();
    assert_eq!(authority.validators.len(), 4);
    for (mint, consensus) in authority.validators.iter().zip(&context.roster) {
        assert_eq!(mint.validator, consensus.validator);
        assert!(bool::from(
            PallasAffine::from_bytes(&mint.eq_proof_public_key.into()).is_some()
        ));
        assert!(bool::from(
            VestaAffine::from_bytes(&mint.ep_proof_public_key.into()).is_some()
        ));
    }
    for mutation in 0..7 {
        let mut invalid = context.clone();
        match mutation {
            0 => invalid.kagemusha_mint_finality_authority.generation += 1,
            1 => invalid.kagemusha_mint_finality_authorization.authority_id[0] ^= 1,
            2 => invalid.kagemusha_mint_finality_authorization.epoch += 1,
            3 => invalid.kagemusha_mint_finality_authorization.last_height += 1,
            4 => invalid.kagemusha_mint_finality_authorization.first_height += 1,
            5 => {
                invalid.kagemusha_mint_finality_authority.network_id =
                    iroha_data_model::NetworkId::from_genesis_hash(
                        iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
                            b"foreign SCCP genesis",
                        )),
                    )
            }
            _ => invalid
                .kagemusha_mint_finality_authority
                .validators
                .swap(0, 1),
        }
        assert!(
            invalid.validate().is_err(),
            "substitution {mutation} must fail closed"
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
