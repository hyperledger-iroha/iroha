//! Exact borrowed proposal-wire equivalence across result and feature layouts.

use super::*;
use iroha_crypto::{Algorithm, KeyPair};
use std::num::NonZeroU64;

fn plain_signed_block() -> SignedBlock {
    let header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, 1_000, 0);
    let key = KeyPair::try_from_seed(vec![0x49; 32], Algorithm::Ed25519).unwrap();
    let signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(key.private_key(), header.hash()).unwrap(),
    );
    SignedBlock {
        signatures: BTreeSet::from([signature]),
        payload: BlockPayload {
            header,
            external_entrypoints: Vec::new(),
            execution_context: None,
            da_commitments: None,
            da_proof_policies: None,
            da_pin_intents: None,
            npos_consensus_effects: None,
        },
        result: None,
    }
}

fn assert_exact_borrowed_proposal_wire(block: &SignedBlock) {
    let reference = block.canonical_resultless_proposal().encode_wire().unwrap();
    let borrowed = block.borrowed_resultless_wire().unwrap();
    let candidate = SignedBlockOutputCandidate {
        signatures: OutputFieldRef(&block.signatures),
        payload: OutputFieldRef(&block.payload),
        result: None,
    };
    assert_eq!(
        borrowed, reference,
        "version, SignedBlock header and payload"
    );
    assert_eq!(borrowed[0], block.version());
    let payload_len = {
        norito::core::reset_decode_state();
        let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        norito::core::encoded_payload_len(&candidate).unwrap()
    };
    assert_eq!(
        payload_len + norito::core::Header::SIZE + 1,
        borrowed.len(),
        "version + fixed header + counted payload define the custom wire size",
    );
    assert_eq!(
        block.canonical_proposal_wire_hash().unwrap(),
        Hash::new(&reference),
    );
}

#[test]
fn borrowed_proposal_hash_matches_exact_resultless_and_executed_layouts() {
    let proposal = plain_signed_block();
    assert_exact_borrowed_proposal_wire(&proposal);
    assert_eq!(
        proposal.canonical_proposal_wire_hash().unwrap(),
        proposal.executed_block_wire_hash().unwrap(),
    );

    let mut executed = proposal.clone();
    executed.result = Some(BlockResult::default());
    assert_exact_borrowed_proposal_wire(&executed);
    assert_eq!(
        executed.canonical_proposal_wire_hash().unwrap(),
        proposal.canonical_proposal_wire_hash().unwrap(),
    );
    assert_ne!(
        executed.executed_block_wire_hash().unwrap(),
        proposal.executed_block_wire_hash().unwrap(),
    );
}

#[test]
fn borrowed_proposal_hash_includes_changed_signatures() {
    let mut proposal = plain_signed_block();
    let before = proposal.canonical_proposal_wire_hash().unwrap();
    let key = KeyPair::try_from_seed(vec![0x4a; 32], Algorithm::Ed25519).unwrap();
    proposal.signatures.insert(BlockSignature::new(
        1,
        SignatureOf::try_from_hash(key.private_key(), proposal.hash()).unwrap(),
    ));
    assert_exact_borrowed_proposal_wire(&proposal);
    assert_ne!(proposal.canonical_proposal_wire_hash().unwrap(), before);

    proposal.result = Some(BlockResult::default());
    assert_exact_borrowed_proposal_wire(&proposal);
    assert_ne!(proposal.canonical_proposal_wire_hash().unwrap(), before);
}

#[cfg(feature = "transparent_api")]
#[test]
fn borrowed_proposal_hash_matches_real_attached_outputs_and_post_attachment_signature() {
    let mut block = output_test_support::proposal(2);
    let before = block.canonical_proposal_wire_hash().unwrap();
    let rows = vec![
        output_test_support::network(0, Ok(Vec::default())),
        output_test_support::network(1, Ok(Vec::default())),
        output_test_support::simple_time(&block, 0),
    ];
    output_test_support::install(&mut block, rows, 3).unwrap();
    block
        .validate_execution_outputs(&output_test_support::limits())
        .unwrap();
    assert_exact_borrowed_proposal_wire(&block);
    assert_eq!(block.canonical_proposal_wire_hash().unwrap(), before);

    let key = KeyPair::try_from_seed(vec![0x4b; 32], Algorithm::Ed25519).unwrap();
    block
        .add_signature(BlockSignature::new(
            1,
            SignatureOf::try_from_hash(key.private_key(), block.hash()).unwrap(),
        ))
        .unwrap();
    assert_exact_borrowed_proposal_wire(&block);
    assert_ne!(block.canonical_proposal_wire_hash().unwrap(), before);
}
