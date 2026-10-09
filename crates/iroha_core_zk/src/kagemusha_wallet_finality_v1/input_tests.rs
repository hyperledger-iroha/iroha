//! Genuine native QC/event conversions. Fixture success rows are synthetic;
//! these tests do not claim World execution or compact proof production.

use super::*;
use crate::kagemusha_wallet_finality_v1::derive_history_anchor;
use iroha_crypto::{HashOf, KeyPair, MerkleTree};
use iroha_data_model::{
    account::AccountId,
    block::{BlockSignatures, builder::BlockBuilder},
    events::data::{DataEvent, kagemusha::KagemushaLoadCommittedV1},
    isi::kagemusha_wallet::{KagemushaWalletLedgerActionV1, KagemushaWalletLedgerV1},
    kagemusha::kagemusha_wallet_account_digest_v1,
    sumeragi_finality::test_fixtures::NativeFinalityFixture,
    transaction::{FeePaymentIntent, TransactionBuilder},
};
use std::time::Duration;

fn fixture() -> (
    NativeFinalityFixture,
    HistoryAnchor,
    VerifiedSumeragiBlock,
    KagemushaWalletLoadReceiptV1,
    MerkleTree<EventBox>,
) {
    let mut fixture = NativeFinalityFixture::new_with_explicit_parameters();
    let anchor = derive_history_anchor(&fixture.verifier()).unwrap();
    let signer = KeyPair::from_seed(vec![41; 32], Algorithm::Ed25519);
    let payer = AccountId::new(signer.public_key().clone());
    let instruction = KagemushaWalletLedgerV1::new(
        [1; 32],
        KagemushaWalletLedgerActionV1::IssueLoad {
            wallet: [2; 32],
            asset: [3; 32],
            ordinal: 7,
            request_id: [4; 32],
            amount: 123,
            charge: None,
        },
    );
    let header = fixture.next_header();
    let mut transaction = TransactionBuilder::new(
        fixture.network_id(),
        payer.clone(),
        FeePaymentIntent::authority(vec![], None),
    );
    transaction.set_creation_time(Duration::from_millis(header.creation_time_ms - 1));
    let transaction = transaction
        .with_instructions([instruction])
        .sign(signer.private_key());
    let receipt = KagemushaWalletLoadReceiptV1 {
        version: 1,
        scheme_id: [1; 32],
        asset_digest: [3; 32],
        wallet_id: [2; 32],
        request_id: [4; 32],
        ordinal: 7,
        amount: 123,
        online_charge: 0,
        charge_quote: [0; 32],
        transaction_hash: *transaction.hash().as_ref(),
        block_height: header.height().get(),
        payer_account_digest: kagemusha_wallet_account_digest_v1(&payer).unwrap(),
    };
    let mut builder = BlockBuilder::new(header);
    builder.push_transaction(transaction);
    let mut block = builder.build(BlockSignatures::default());
    NativeFinalityFixture::install_network_results(&mut block, vec![Ok(vec![])]);
    let events = [11, 17, receipt.amount].map(|amount| {
        let mut selected = receipt;
        selected.amount = amount;
        EventBox::Data(
            DataEvent::from(KagemushaLoadCommittedV1::from_receipt(&selected).unwrap()).into(),
        )
    });
    let tree = events.iter().map(HashOf::new).collect();
    let proof = fixture.certify_with_events(block, &events);
    let verified = fixture.verifier().verify_retained_decision(&proof).unwrap();
    (fixture, anchor, verified, receipt, tree)
}

#[test]
fn block_proposal_preserves_original_qc_result_and_ordered_native_roster() {
    let (fixture, anchor, block, _, _) = fixture();
    let input = block_witness(&anchor, fixture.chain_id(), &block).unwrap();
    let original = block.block().commit_certificate().unwrap();
    let qc: Qc = norito::decode_canonical(original.commit_qc()).unwrap();
    assert_eq!(input.result_frame, original.result_preimage());
    assert_eq!(input.message.as_slice(), qc.preimage());
    assert_eq!(input.signature, qc.agg_sig.0);
    assert_eq!(input.bitmap, qc.signers.as_bytes());
    assert_eq!(input.current_context, qc.epoch.context.0);
    assert_eq!(input.authorized_context, input.current_context);
    assert_eq!(
        input.roster.len(),
        block.commitment().schedule.current.committee.len()
    );
    for (key, member) in input
        .roster
        .iter()
        .zip(&block.commitment().schedule.current.committee)
    {
        let (algorithm, original) = member.validator.public_key().try_to_bytes().unwrap();
        assert_eq!(algorithm, Algorithm::BlsNormal);
        assert_eq!(key.as_slice(), original);
    }
    let mut foreign = anchor;
    foreign.instance[0] ^= 1;
    assert!(block_witness(&foreign, fixture.chain_id(), &block).is_err());
    foreign = anchor;
    foreign.network[0] ^= 1;
    assert!(block_witness(&foreign, fixture.chain_id(), &block).is_err());
    assert!(block_witness(&anchor, "different native chain", &block).is_err());
}

#[test]
fn npos_boundary_preserves_current_certificate_and_authorized_successor() {
    use iroha_data_model::block::{CommitCertificate, decode_framed_signed_block};

    for seats in [4, 31] {
        let (fixture, chain) =
            NativeFinalityFixture::short_npos_boundary_chain_with_explicit_parameters(seats);
        let verifier = fixture.verifier();
        let anchor = derive_history_anchor(&verifier).unwrap();
        let proof = chain.iter().find(|proof| proof.height() == 3).unwrap();
        let block = verifier.verify_retained_decision(proof).unwrap();
        let schedule = &block.commitment().schedule;
        let boundary = schedule.boundary.as_ref().unwrap();
        let current_context = schedule.current.context_id().unwrap();
        let successor_context = boundary.next.context_id().unwrap();
        assert_ne!(current_context, successor_context);
        assert_eq!(current_context, anchor.initial_context);
        assert_eq!(boundary.predecessor_context_id, current_context);

        let input = block_witness(&anchor, fixture.chain_id(), &block).unwrap();
        let certificate = block.block().commit_certificate().unwrap();
        let qc: Qc = norito::decode_canonical(certificate.commit_qc()).unwrap();
        assert_eq!(input.result_frame, certificate.result_preimage());
        assert_eq!(input.current_context, current_context);
        assert_eq!(input.authorized_context, successor_context);
        assert_eq!(input.current_context, qc.epoch.context.0);
        assert_eq!(qc.epoch.epoch, schedule.current.authorization.epoch);
        assert_ne!(qc.epoch.epoch, boundary.next.authorization.epoch);
        assert_eq!(input.message.as_slice(), qc.preimage());
        assert_eq!(input.signature, qc.agg_sig.0);
        assert_eq!(input.bitmap, qc.signers.as_bytes());
        assert_eq!(input.roster.len(), seats);
        for (actual, member) in input.roster.iter().zip(&schedule.current.committee) {
            let (algorithm, original) = member.validator.public_key().try_to_bytes().unwrap();
            assert_eq!(algorithm, Algorithm::BlsNormal);
            assert_eq!(actual.as_slice(), original);
        }

        let mut wrong_anchor = anchor;
        wrong_anchor.instance[0] ^= 1;
        assert!(block_witness(&wrong_anchor, fixture.chain_id(), &block).is_err());
        assert!(block_witness(&anchor, "foreign NPoS chain", &block).is_err());

        // A successor-context QC is not current authority at the boundary.
        // Rejection happens at the native owner; no forged capability enters the adapter.
        let mut changed_qc = qc;
        changed_qc.epoch.epoch = boundary.next.authorization.epoch;
        changed_qc.epoch.context.0 = successor_context;
        let mut changed_block = decode_framed_signed_block(&proof.block_wire).unwrap();
        changed_block.set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
            certificate.consensus_header().to_vec(),
            norito::encode_canonical(&changed_qc).unwrap(),
            certificate.result_preimage().to_vec(),
            certificate.availability().to_vec(),
        )));
        let mut changed_proof = proof.clone();
        changed_proof.block_wire = changed_block.encode_wire().unwrap();
        assert!(verifier.verify_retained_decision(&changed_proof).is_err());

        let next = chain.iter().find(|proof| proof.height() == 4).unwrap();
        let next = verifier.verify_retained_decision(next).unwrap();
        let next_input = block_witness(&anchor, fixture.chain_id(), &next).unwrap();
        assert_eq!(next_input.current_context, successor_context);
        assert_eq!(next_input.authorized_context, successor_context);
    }
}

#[test]
fn load_proposal_binds_original_receipt_and_counted_promotion_path() {
    let (fixture, anchor, block, receipt, tree) = fixture();
    let path = tree.get_proof(2).unwrap();
    assert_eq!(path.audit_path()[0], None, "odd final leaf is promoted");
    let input = load_witness(&anchor, fixture.chain_id(), &block, &receipt, &path).unwrap();
    assert_eq!(input.receipt, receipt.transcript().unwrap());
    assert_eq!(input.event_count, 3);
    assert_eq!(input.event_index, 2);
    assert_eq!(input.event_root, *tree.root().unwrap().as_ref());
    assert_eq!(input.siblings[0], [0; 32]);
    assert_eq!(input.siblings[1], *path.audit_path()[1].unwrap().as_ref());
    assert_eq!(input.siblings[2..], [[0; 32]; 30]);
    assert_eq!(
        input.result_frame,
        block
            .block()
            .commit_certificate()
            .unwrap()
            .result_preimage()
    );
    let mut wrong_receipt = receipt;
    wrong_receipt.amount += 1;
    assert!(load_witness(&anchor, fixture.chain_id(), &block, &wrong_receipt, &path).is_err());
    assert!(
        load_witness(
            &anchor,
            fixture.chain_id(),
            &block,
            &receipt,
            &tree.get_proof(0).unwrap()
        )
        .is_err()
    );
    let mut changed = path.audit_path().to_vec();
    changed[1] = None;
    let changed = MerkleProof::from_audit_path(2, changed);
    assert!(load_witness(&anchor, fixture.chain_id(), &block, &receipt, &changed).is_err());
}
