//! Compact publication evidence: signer-only inclusion, exact floor and body-free certificates.

use super::publication_evidence;
use axum::http::StatusCode;
use iroha_allocation::AllocationBudget;
use iroha_core::{
    state::{StateReadOnly, World},
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    account::AccountId,
    isi::{InstructionBox, Log},
    level::Level,
    sorafs::publication::SorafsPublicationProofRequestV1,
};
use std::{
    num::NonZeroU64,
    time::{Duration, Instant},
};

/// Seed of the test chain's genesis-registered clock account.
const CLOCK_SEED: u8 = 0xCC;

#[test]
fn publication_evidence_serves_only_the_signer_with_certificates_and_one_inclusion() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 10_000)).unwrap();
    let signer = KeyPair::from_seed(vec![CLOCK_SEED; 32], Algorithm::Ed25519);
    let account = AccountId::new(signer.public_key().clone());
    let log: InstructionBox = Log::new(Level::INFO, "publication evidence fixture".into()).into();
    let signed = chain.sign(&signer, [log], 10_001);
    assert_eq!(chain.commit(vec![signed.clone()]), vec![true]);
    chain.commit(Vec::new());
    let entry_hash = signed.hash_as_entrypoint();
    let height = NonZeroU64::new(chain.height() - 1).unwrap();
    let (genesis, executed) = {
        let view = chain.state().view();
        let hashes = view.block_hashes();
        (
            *hashes.first().unwrap().as_ref(),
            *hashes
                .get(usize::try_from(height.get()).unwrap() - 1)
                .unwrap()
                .as_ref(),
        )
    };
    let request = SorafsPublicationProofRequestV1 {
        entry_hash: *entry_hash.as_ref(),
        floor_height: 1,
        floor_block_hash: genesis,
    };
    let budget = AllocationBudget::new(64 * 1024 * 1024);
    let deadline = Instant::now() + Duration::from_secs(60);
    let evidence = |requester: &AccountId, request: &SorafsPublicationProofRequestV1| {
        publication_evidence(
            chain.state(),
            requester,
            request,
            entry_hash,
            height,
            &budget,
            deadline,
        )
    };

    let (proof, charges) = evidence(&account, &request).unwrap();
    assert!(
        proof.epoch_boundaries.is_empty(),
        "one epoch needs no boundary"
    );
    assert_eq!(proof.certificate.height().unwrap(), height.get());
    assert!(proof.inclusion.joined_network_output().is_ok());
    assert!(!charges.is_empty(), "certificate reads stay charged");
    // A header, a quorum certificate, a result and two Merkle paths: no block body.
    assert!(norito::canonical_frame_len(&proof).unwrap() < 256 * 1024);
    drop(charges);

    let stranger = AccountId::new(
        KeyPair::from_seed(vec![0x5A; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    assert_eq!(
        evidence(&stranger, &request).unwrap_err(),
        StatusCode::FORBIDDEN,
        "only the assertion's signer receives its inclusion"
    );
    let substituted_floor = SorafsPublicationProofRequestV1 {
        floor_block_hash: [0x55; 32],
        ..request
    };
    assert_eq!(
        evidence(&account, &substituted_floor).unwrap_err(),
        StatusCode::CONFLICT
    );
    let floor_at_execution = SorafsPublicationProofRequestV1 {
        floor_height: height.get(),
        floor_block_hash: executed,
        ..request
    };
    assert_eq!(
        evidence(&account, &floor_at_execution).unwrap_err(),
        StatusCode::CONFLICT,
        "the assertion must execute after the client's floor"
    );
}
