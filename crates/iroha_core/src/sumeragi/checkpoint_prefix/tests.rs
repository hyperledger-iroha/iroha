//! Genuine signed-history controls for the body-independent Core checkpoint join.

use super::*;
use crate::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_crypto::Hash;
use iroha_data_model::sumeragi_finality::SumeragiCommitCertificateV1;

/// Original executed chain and independently verifiable minimal transfer frames.
pub(crate) struct Fixture {
    pub(crate) chain: CertifiedTestChain,
    pub(crate) genesis: iroha_data_model::block::SharedSignedBlock,
    pub(crate) hashes: Vec<HashOf<BlockHeader>>,
    pub(crate) frames: Vec<PortablePrefixFrameV1>,
}

/// Build P_2 ancestry through actual H3, extracting rows only from original native custody.
pub(crate) fn fixture() -> Fixture {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000))
        .expect("original signed genesis");
    chain.commit(Vec::new());
    chain.commit(Vec::new());
    let genesis = chain.committed(1).block().clone();
    let mut verifier =
        verifier(&genesis, chain.state().chain_id_ref(), chain.network_id()).unwrap();
    let hashes = (1..=2).map(|h| chain.committed(h).block_hash()).collect();
    let mut frames = Vec::new();
    for height in 2..=3 {
        let block = chain.committed(height);
        let certificate = block.block().commit_certificate().unwrap();
        let mut acquisition = verifier
            .acquisition(
                SumeragiCommitCertificateV1 {
                    consensus_header: certificate.consensus_header().to_vec(),
                    commit_qc: certificate.commit_qc().to_vec(),
                    result_preimage: certificate.result_preimage().to_vec(),
                },
                certificate.availability().to_vec(),
                chain.committed(height - 1).block().header(),
            )
            .unwrap();
        let (body, _) = chain.committed_body(height).unwrap().unwrap();
        acquisition.acquire_from_body(&body).unwrap();
        let proof = acquisition
            .into_proof()
            .unwrap_or_else(|_| panic!("original body must cover its minimal prefix"))
            .with_parent_da_bundles(
                chain
                    .committed(height - 1)
                    .block()
                    .da_commitments()
                    .cloned(),
                chain
                    .committed(height - 1)
                    .block()
                    .da_pin_intents()
                    .cloned(),
            );
        verifier.verify(&proof).unwrap();
        frames.push(proof);
    }
    Fixture {
        chain,
        genesis,
        hashes,
        frames,
    }
}

#[test]
fn original_portable_prefix_binds_every_hash_and_published_root() {
    let fixture = fixture();
    let mut observed = Vec::new();
    let verified = verify(
        &fixture.genesis,
        fixture.chain.state().chain_id_ref(),
        fixture.chain.network_id(),
        &fixture.hashes,
        &mut |height| Ok(fixture.frames[(height - 2) as usize].clone()),
        &mut |block| {
            observed.push((block.header().height().get(), block.header().hash()));
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(
        observed,
        vec![(1, fixture.hashes[0]), (2, fixture.hashes[1])]
    );
    assert_eq!(
        verified.checkpoint().core_hash(),
        fixture.chain.committed(2).core_hash()
    );
    assert_eq!(
        verified.predecessor().unwrap().header().hash(),
        fixture.hashes[0]
    );
    assert_eq!(
        verified.published_root(),
        fixture
            .chain
            .committed(3)
            .commitment()
            .execution
            .parent_world_state_root,
    );
}

#[test]
fn portable_prefix_refuses_foreign_journal_and_missing_successor() {
    let fixture = fixture();
    let mut changed = fixture.hashes.clone();
    changed[1] = HashOf::from_untyped_unchecked(Hash::new(b"substituted checkpoint"));
    let mut observed = Vec::new();
    let error = verify(
        &fixture.genesis,
        fixture.chain.state().chain_id_ref(),
        fixture.chain.network_id(),
        &changed,
        &mut |height| Ok(fixture.frames[(height - 2) as usize].clone()),
        &mut |block| {
            observed.push(block.header().height().get());
            Ok(())
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ExecutionAttemptError::Rejected(Error::Invalid(_))
    ));
    assert_eq!(
        observed,
        [1],
        "a substituted parent never reaches its observer"
    );
    let error = verify(
        &fixture.genesis,
        fixture.chain.state().chain_id_ref(),
        fixture.chain.network_id(),
        &fixture.hashes,
        &mut |height| {
            if height == 2 {
                Ok(fixture.frames[0].clone())
            } else {
                Err(invalid("original successor unavailable"))
            }
        },
        &mut |_| Ok(()),
    )
    .unwrap_err();
    assert!(
        matches!(error, ExecutionAttemptError::Rejected(Error::Invalid(message))
        if message == "original successor unavailable")
    );
}

#[test]
fn portable_prefix_checks_pinned_network_before_reading_frames() {
    let fixture = fixture();
    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"another independently selected network",
    )));
    let error = verify(
        &fixture.genesis,
        fixture.chain.state().chain_id_ref(),
        network,
        &fixture.hashes,
        &mut |_| panic!("foreign trust root must fail before transfer reads"),
        &mut |_| panic!("foreign trust root must not produce an observer receipt"),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ExecutionAttemptError::Rejected(Error::Invalid(_))
    ));
}

#[test]
fn portable_committee_observer_checks_the_same_original_world_cut() {
    let fixture = fixture();
    let budget = fixture.chain.state().ivm_execution_budget();
    let world = fixture
        .chain
        .state()
        .world
        .try_block_and_revert(&budget)
        .unwrap();
    let mut progress = crate::state::validator_committee::CommittedProgress::start(
        &world,
        fixture.chain.network_id(),
        2,
    )
    .unwrap();
    verify(
        &fixture.genesis,
        fixture.chain.state().chain_id_ref(),
        fixture.chain.network_id(),
        &fixture.hashes,
        &mut |height| Ok(fixture.frames[(height - 2) as usize].clone()),
        &mut |block| {
            progress
                .observe_portable(block)
                .map_err(|error| error.map_rejection(Error::Invalid))
        },
    )
    .unwrap();
    progress
        .finish()
        .expect("exact original H2 schedule and beacon progress");

    let empty = World::new();
    let empty_view = empty.view();
    let mut omitted = crate::state::validator_committee::CommittedProgress::start(
        &empty_view,
        fixture.chain.network_id(),
        2,
    )
    .unwrap();
    let result = verify(
        &fixture.genesis,
        fixture.chain.state().chain_id_ref(),
        fixture.chain.network_id(),
        &fixture.hashes,
        &mut |height| Ok(fixture.frames[(height - 2) as usize].clone()),
        &mut |block| {
            omitted
                .observe_portable(block)
                .map_err(|error| error.map_rejection(Error::Invalid))
        },
    );
    assert!(
        result.is_err() || omitted.finish().is_err(),
        "portable finality never excuses omitted committee progress"
    );
}

#[test]
fn portable_prefix_preserves_original_decode_refusal_and_storage_cause() {
    let fixture = fixture();
    let bytes = fixture.frames[0].to_canonical_bytes().unwrap();
    let refused = norito::core::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, 0, usize::MAX, usize::MAX, 128),
        || PortablePrefixFrameV1::decode_canonical(&bytes).map_err(finality_error),
    );
    assert!(matches!(refused, Err(ExecutionAttemptError::Deferred(_))));
    let path = std::path::PathBuf::from("original-prefix-proof.norito");
    let original = crate::kura::Error::IO(
        std::io::Error::new(std::io::ErrorKind::PermissionDenied, "original refusal"),
        path.clone(),
    );
    match storage_error(original) {
        ExecutionAttemptError::Rejected(Error::Storage(original)) => match *original {
            crate::kura::Error::IO(error, actual_path) => {
                assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
                assert_eq!(error.to_string(), "original refusal");
                assert_eq!(actual_path, path);
            }
            other => panic!("lost original storage error: {other}"),
        },
        other => panic!("lost original source: {other}"),
    }
}
