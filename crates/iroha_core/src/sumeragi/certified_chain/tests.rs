//! The certified-chain reader over a real certified chain: committed and certified reads,
//! per-node certificates, and the frames and certificates it must refuse.

use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    isi::{InstructionBox, Log},
    transaction::TransactionEntrypoint,
};
use iroha_logger::Level;
use iroha_sumeragi::crypto::NoAttestation;

use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
    sumeragi::{
        block_store::commit_certificate,
        test_chain::{CertifiedTestChain, Signers, TestChainConfig},
    },
};

/// Genesis and four certified blocks; block 3 carries a transaction.
fn chain() -> (CertifiedTestChain, HashOf<TransactionEntrypoint>) {
    let mut chain =
        CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).expect("chain");
    chain.commit_at(2_000, Vec::new());
    let author = KeyPair::from_seed(vec![0x51; 32], Algorithm::Ed25519);
    let transaction = chain.sign(
        &author,
        [InstructionBox::from(Log::new(
            Level::INFO,
            "certified".into(),
        ))],
        2_500,
    );
    let entry = transaction.hash_as_entrypoint();
    chain.commit_at(3_000, vec![transaction]);
    chain.commit_with(Some(4_000), Vec::new(), Signers::All);
    chain.commit_at(5_000, Vec::new());
    (chain, entry)
}

fn frame(chain: &CertifiedTestChain, height: u64) -> Arc<SignedBlock> {
    chain
        .kura()
        .get_block(NonZeroUsize::new(usize::try_from(height).unwrap()).unwrap())
        .expect("stored frame")
}

/// `frame` with its certificate's parts replaced.
fn with_parts(
    frame: &SignedBlock,
    edit: impl FnOnce(&mut BlockHeader, &mut Qc, &mut Vec<u8>),
) -> Arc<SignedBlock> {
    let certificate = frame.commit_certificate().expect("certificate");
    let (mut header, mut qc) = decode_certificate(certificate).expect("parts");
    let mut preimage = certificate.result_preimage.clone();
    edit(&mut header, &mut qc, &mut preimage);
    Arc::new(
        frame
            .clone()
            .with_commit_certificate(Some(commit_certificate(&header, &qc, preimage).unwrap())),
    )
}

#[test]
fn committed_and_certified_reads_of_a_real_chain() {
    let (chain, entry) = chain();
    let view = chain.state().view();
    let reader = CertifiedChain::new(&view).expect("reader");
    assert_eq!(reader.instance(), chain.instance());
    let genesis = reader.certified(1).expect("genesis");
    assert_eq!(genesis.verification(), QcVerification::Genesis);
    assert!(genesis.header().is_none() && genesis.commit_qc().is_none());
    assert_eq!(genesis.block_hash(), chain.genesis().hash());
    let blocks = reader
        .walk(1, 5)
        .collect::<Result<Vec<_>, _>>()
        .expect("walk");
    assert_eq!(blocks.len(), 5);
    for (block, height) in blocks.iter().zip(1..) {
        assert_eq!(block.height(), height);
        assert_eq!(
            block.block_hash(),
            *view
                .block_hashes()
                .get(usize::try_from(height - 1).unwrap())
                .unwrap()
        );
        let committed = committed_block(&view, height).expect("committed read");
        assert_eq!(committed.id(), block.id());
        assert_eq!(
            committed.id(),
            certified_block_id(&block.core_hash(), &block.result())
        );
        if height > 1 {
            assert_eq!(block.verification(), QcVerification::Verified);
            assert_eq!(block.commit_qc().unwrap().block_hash, block.core_hash());
            assert!(block.extends(&blocks[usize::try_from(height - 2).unwrap()]));
            assert_eq!(
                block.certificate_len(),
                norito::canonical_frame_len(block.certificate().expect("certificate")).unwrap()
            );
        }
    }
    assert!(
        !blocks[3].extends(&blocks[1]),
        "block 4 does not extend block 2"
    );
    // R_g commits the genesis committee as C_{g+2}, the committee that certified g + 1.
    assert_eq!(
        genesis.commitment().next_committee_digest,
        reader.candidates().expect("candidates").genesis
    );
    // The transaction of block 3 is anchored by the executed wire R commits.
    let anchor = blocks[2].entry_anchor(&entry).expect("anchor");
    assert!(
        blocks[2]
            .block()
            .network_execution_proof(&entry)
            .expect("proof")
            .verify(&anchor)
    );
    assert!(blocks[1].entry_anchor(&entry).is_err());
    let (output_index, _) = blocks[2].block().network_output_at(0).expect("output");
    assert_eq!(
        blocks[2]
            .output_anchor(output_index)
            .expect("output anchor")
            .output_index(),
        output_index
    );
    // The full check with an attestation verifier accepts unflagged certificates too.
    let full = CertifiedChain::new(&view)
        .expect("reader")
        .with_attestation_verifier(&NoAttestation);
    assert_eq!(
        full.certified(4).expect("full check").verification(),
        QcVerification::Verified
    );
}

#[test]
fn uncommitted_heights_are_not_read() {
    let (chain, _) = chain();
    let view = chain.state().view();
    let reader = CertifiedChain::new(&view).expect("reader");
    for height in [0, 6, u64::MAX] {
        assert_eq!(
            committed_block(&view, height).err(),
            Some(ChainReadError::NotCommitted { height })
        );
        assert_eq!(
            reader.certified(height).err(),
            Some(ChainReadError::NotCommitted { height })
        );
    }
    assert_eq!(
        reader
            .walk(4, 6)
            .map(|read| read.is_ok())
            .collect::<Vec<_>>(),
        [true, true, false]
    );
}

#[test]
fn frames_without_a_matching_header_preimage_or_certificate_are_refused() {
    let (chain, _) = chain();
    let original = frame(&chain, 3);
    // No certificate at all.
    let bare = Arc::new(original.as_ref().clone().with_commit_certificate(None));
    assert_eq!(
        read_frame(bare, 3).err(),
        Some(ChainReadError::MissingCertificate { height: 3 })
    );
    // A header for another payload.
    let wrong_payload = with_parts(&original, |header, _, _| {
        header.payload_hash = Hash32([7; 32])
    });
    assert_eq!(
        read_frame(wrong_payload, 3).err(),
        Some(ChainReadError::HeaderMismatch { height: 3 })
    );
    // A header of another height.
    let wrong_height = with_parts(&original, |header, _, _| header.height = 4);
    assert_eq!(
        read_frame(wrong_height, 3).err(),
        Some(ChainReadError::HeaderMismatch { height: 3 })
    );
    // A result preimage that commits another executed block.
    let wrong_wire = with_parts(&original, |_, _, preimage| {
        let mut commitment = ExecutionResultCommitment::decode(preimage).unwrap();
        commitment.execution.executed_block_wire_len += 1;
        *preimage = commitment.preimage().unwrap();
    });
    assert_eq!(
        read_frame(wrong_wire, 3).err(),
        Some(ChainReadError::ExecutionMismatch { height: 3 })
    );
    // Garbage in the preimage or header.
    let garbage = with_parts(&original, |_, _, preimage| *preimage = vec![1, 2, 3]);
    assert!(matches!(
        read_frame(garbage, 3),
        Err(ChainReadError::Malformed { height: 3, .. })
    ));
    // Genesis must carry the result-only certificate.
    let genesis = frame(&chain, 1);
    let certificate = genesis.commit_certificate().unwrap().clone();
    let headed = Arc::new(genesis.as_ref().clone().with_commit_certificate(Some(
        CommitCertificate::new(vec![1], Vec::new(), certificate.result_preimage),
    )));
    assert!(matches!(
        read_frame(headed, 1),
        Err(ChainReadError::Malformed { height: 1, .. })
    ));
}

#[test]
fn certificates_that_do_not_certify_the_stored_block_are_refused() {
    let (chain, _) = chain();
    let view = chain.state().view();
    let reader = CertifiedChain::new(&view).expect("reader");
    let original = frame(&chain, 4);
    let certify = |frame: Arc<SignedBlock>| reader.check_certificate(read_frame(frame, 4).unwrap());
    certify(Arc::clone(&original)).expect("the stored certificate verifies");
    // A preimage whose `R` is not the certified one (its executed wire is unchanged).
    let other_result = with_parts(&original, |_, _, preimage| {
        let mut commitment = ExecutionResultCommitment::decode(preimage).unwrap();
        commitment.next_committee_digest = [9; 32];
        *preimage = commitment.preimage().unwrap();
    });
    assert_eq!(
        certify(other_result).err(),
        Some(ChainReadError::ResultMismatch { height: 4 })
    );
    // A header whose hash is not the certified block hash.
    let other_header = with_parts(&original, |header, _, _| header.origin_view = 3);
    assert_eq!(
        certify(other_header).err(),
        Some(ChainReadError::HeaderMismatch { height: 4 })
    );
    // A Prepare certificate, or a flag that differs from the header's.
    let edits: [fn(&mut BlockHeader, &mut Qc, &mut Vec<u8>); 2] = [
        |_, qc, _| qc.kind = VoteKind::Prepare,
        |_, qc, _| qc.attest = !qc.attest,
    ];
    for edit in edits {
        assert_eq!(
            certify(with_parts(&original, edit)).err(),
            Some(ChainReadError::HeaderMismatch { height: 4 })
        );
    }
    // Another instance.
    let other_instance = with_parts(&original, |_, qc, _| qc.instance = Hash32([5; 32]));
    assert_eq!(
        certify(other_instance).err(),
        Some(ChainReadError::WrongInstance { height: 4 })
    );
    // Below the quorum, or a forged aggregate.
    let (header, qc) = decode_certificate(original.commit_certificate().unwrap()).unwrap();
    let below = chain.commit_qc(4, qc.block_hash, qc.result, qc.attest, Signers::BelowQuorum);
    let below = with_parts(&original, |_, qc, _| *qc = below);
    assert_eq!(
        certify(below).err(),
        Some(ChainReadError::Certificate {
            height: 4,
            error: CertError::TooFewSigners
        })
    );
    let forged = with_parts(&original, |_, qc, _| qc.agg_sig.0[5] ^= 1);
    assert!(matches!(
        certify(forged),
        Err(ChainReadError::Certificate { height: 4, .. })
    ));
    assert_eq!(header.height, 4);
}

/// Certificates are per node: another valid `CommitQC` of the same block (other signers) gives
/// the same consensus-visible receipt, and both certificates verify.
#[test]
fn two_valid_certificates_of_one_block_give_one_consensus_receipt() {
    let (chain, entry) = chain();
    let view = chain.state().view();
    let reader = CertifiedChain::new(&view).expect("reader");
    let original = frame(&chain, 3);
    let (_, qc) = decode_certificate(original.commit_certificate().unwrap()).unwrap();
    let other_qc = chain.commit_qc(3, qc.block_hash, qc.result, qc.attest, Signers::LastThree);
    assert_ne!(other_qc.signers, qc.signers);
    let other = with_parts(&original, |_, qc, _| *qc = other_qc.clone());
    assert_ne!(other.commit_certificate(), original.commit_certificate());
    let left = read_frame(Arc::clone(&original), 3).expect("committed read");
    let right = read_frame(Arc::clone(&other), 3).expect("committed read");
    assert_eq!(left.id(), right.id());
    assert_eq!(left.header(), right.header());
    assert_eq!(
        (left.core_hash(), left.result(), left.block_hash()),
        (right.core_hash(), right.result(), right.block_hash())
    );
    assert_eq!(left.commitment(), right.commitment());
    assert_eq!(left.entry_anchor(&entry), right.entry_anchor(&entry));
    assert_eq!(left.block_time_ms(), right.block_time_ms());
    for committed in [left, right] {
        assert_eq!(
            reader
                .check_certificate(committed)
                .expect("both certificates verify")
                .verification(),
            QcVerification::Verified
        );
    }
}

#[test]
fn an_unknown_historical_committee_relies_on_the_commit_time_check() {
    let (chain, _) = chain();
    let view = chain.state().view();
    let reader = CertifiedChain::new(&view).expect("reader");
    assert!(
        reader
            .candidates
            .set(Ok(Candidates {
                by_digest: BTreeMap::new(),
                genesis: [0; 32],
            }))
            .is_ok()
    );
    assert_eq!(
        reader.certified(4).expect("read").verification(),
        QcVerification::CommittedLocally
    );
    // The genesis committee is known again: the certificate is verified.
    let reader = CertifiedChain::new(&view).expect("reader");
    assert_eq!(
        reader.certified(4).expect("read").verification(),
        QcVerification::Verified
    );
}

/// A State of the chain's network over a fresh Kura that holds the chain's frames, each passed
/// through `edit` first (local tampering of the certificates: the iroha blocks, and so the view's
/// block-hash journal, stay the chain's).
fn tampered_state(
    chain: &CertifiedTestChain,
    edit: impl Fn(u64, Arc<SignedBlock>) -> Arc<SignedBlock>,
) -> State {
    let kura = Kura::blank_kura_for_testing();
    let mut state = State::new_with_chain_and_network_id_for_testing(
        World::new(),
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
        "sumeragi-certified-test-chain".parse().unwrap(),
        chain.network_id(),
    );
    for height in 1..=chain.height() {
        let block = edit(height, frame(chain, height));
        state.push_block_hash_for_testing(block.hash());
        kura.store_block(block).unwrap();
    }
    state
}

/// The committee of a height is authenticated by `R_{h-2}` through the headers its `CommitQC`
/// certifies: a stored `R_{h-2}` preimage that names another committee is refused, and so is a
/// header of `h - 1` rewritten to agree with it (the certified header of `h` binds the original).
#[test]
fn a_tampered_committee_digest_does_not_select_the_committee() {
    let (chain, _) = chain();
    let untouched = tampered_state(&chain, |_, frame| frame);
    let view = untouched.view();
    assert_eq!(
        CertifiedChain::new(&view)
            .expect("reader")
            .certified(4)
            .expect("read")
            .verification(),
        QcVerification::Verified
    );
    let tampered_preimage = |preimage: &mut Vec<u8>| {
        let mut commitment = ExecutionResultCommitment::decode(preimage).unwrap();
        commitment.next_committee_digest = [9; 32];
        *preimage = commitment.preimage().unwrap();
    };
    let mut other_r2 = frame(&chain, 2)
        .commit_certificate()
        .unwrap()
        .result_preimage
        .clone();
    tampered_preimage(&mut other_r2);
    let other_r2 = result_of_preimage(&other_r2);
    // `R_2` names another committee for height 4; the header of 3 still binds the original `R_2`.
    let state = tampered_state(&chain, |height, frame| match height {
        2 => with_parts(&frame, |_, _, preimage| tampered_preimage(preimage)),
        _ => frame,
    });
    let view = state.view();
    let reader = CertifiedChain::new(&view).expect("reader");
    assert_eq!(
        reader.certified(4).err(),
        Some(ChainReadError::ResultMismatch { height: 2 })
    );
    assert!(reader.proof_committee(4).is_err());
    // The header of 3 rewritten to bind the tampered `R_2`: the certified header of 4 does not
    // extend it.
    let state = tampered_state(&chain, |height, frame| match height {
        2 => with_parts(&frame, |_, _, preimage| tampered_preimage(preimage)),
        3 => with_parts(&frame, |header, _, _| header.parent_result = other_r2),
        _ => frame,
    });
    let view = state.view();
    let reader = CertifiedChain::new(&view).expect("reader");
    assert_eq!(
        reader.certified(4).err(),
        Some(ChainReadError::Discontinuous { height: 4 })
    );
    assert_eq!(
        reader.proof_committee(4).err(),
        Some(ChainReadError::Discontinuous { height: 4 })
    );
    // The consensus-visible receipt of 4 does not depend on either certificate.
    assert_eq!(
        committed_block(&view, 4).expect("committed").id(),
        chain.committed(4).id()
    );
}

#[test]
fn a_view_of_another_network_is_refused() {
    let (chain, _) = chain();
    // The chain's genesis frame in the Kura of a State of another network.
    let kura = Kura::blank_kura_for_testing();
    let mut state = State::new_with_chain_and_network_id_for_testing(
        World::new(),
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
        "sumeragi-certified-test-chain".parse().unwrap(),
        iroha_data_model::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"another genesis",
        ))),
    );
    kura.store_block(frame(&chain, 1)).unwrap();
    state.push_block_hash_for_testing(chain.genesis().hash());
    let view = state.view();
    assert!(committed_block(&view, 1).is_ok());
    assert_eq!(
        CertifiedChain::new(&view).err(),
        Some(ChainReadError::ForeignGenesis)
    );
    // A view whose journal names another block at a height does not read Kura's.
    let mut other = State::new_with_chain_and_network_id_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        "sumeragi-certified-test-chain".parse().unwrap(),
        chain.network_id(),
    );
    other.push_block_hash_for_testing(chain.genesis().hash());
    assert_eq!(
        committed_block(&other.view(), 1).err(),
        Some(ChainReadError::NotInView { height: 1 })
    );
}
