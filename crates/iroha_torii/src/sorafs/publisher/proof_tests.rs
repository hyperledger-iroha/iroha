//! Exact native lineage, range and refusal controls for publication proof production.

use super::publication_lineage;
use axum::http::StatusCode;
use iroha_core::{
    state::{StateReadOnly, World},
    sumeragi::{
        finality::build_proof,
        test_chain::{CertifiedTestChain, Signers, TestChainConfig},
    },
};
use iroha_data_model::sorafs::publication::PUBLICATION_PROOF_MAX_BLOCKS_V1;

#[test]
fn publication_lineage_preserves_every_original_proof_and_releases_native_owners() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 10_000)).unwrap();
    while chain.height() < 5 {
        chain.commit(Vec::new());
    }
    let view = chain.state().view();
    let floor = *view.block_hashes().get(1).unwrap().as_ref();
    let budget = view.execution_budget();
    let reserved = budget.reserved_bytes();
    let proof = publication_lineage(&view, 2, floor, 4).unwrap();
    assert_eq!(budget.reserved_bytes(), reserved);
    assert_eq!(proof.lineage.len(), 3);
    for (height, actual) in (2..=4).zip(&proof.lineage) {
        let expected = build_proof(&view, height).unwrap();
        assert_eq!(actual, &expected);
        assert_eq!(
            norito::encode_canonical(actual).unwrap(),
            norito::encode_canonical(&expected).unwrap()
        );
        actual.decode_checked(chain.root_scope().into()).unwrap();
    }
    assert_eq!(budget.reserved_bytes(), reserved);
    assert_eq!(proof.lineage.last().unwrap().block_header.height().get(), 4);
    assert_eq!(
        view.height(),
        5,
        "the requested target must not expand to the tip"
    );

    assert_eq!(
        publication_lineage(&view, 2, [0x55; 32], 4).unwrap_err(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        publication_lineage(&view, 4, floor, 3).unwrap_err(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        publication_lineage(&view, 2, floor, 2).unwrap_err(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(
        publication_lineage(&view, 2, floor, 2 + PUBLICATION_PROOF_MAX_BLOCKS_V1 as u64)
            .unwrap_err(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(
        publication_lineage(&view, 2, floor, 6).unwrap_err(),
        StatusCode::SERVICE_UNAVAILABLE,
        "a missing target must not produce a successful partial lineage"
    );
    assert_eq!(budget.reserved_bytes(), reserved);
}

#[test]
fn publication_lineage_rejects_an_invalid_native_quorum_with_no_partial_response() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 10_000)).unwrap();
    while chain.height() < 4 {
        chain.commit(Vec::new());
    }
    chain.corrupt_local_quorum_for_test(3, Signers::BelowQuorum);
    let view = chain.state().view();
    let floor = *view.block_hashes().get(1).unwrap().as_ref();
    assert!(build_proof(&view, 3).is_err());
    let budget = view.execution_budget();
    let reserved = budget.reserved_bytes();
    assert_eq!(
        publication_lineage(&view, 2, floor, 4).unwrap_err(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(budget.reserved_bytes(), reserved);
}
