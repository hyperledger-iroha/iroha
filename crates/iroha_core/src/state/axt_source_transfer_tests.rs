//! Finalized source-coordinate checks; these fixtures do not enable AXT admission.

use super::axt_source_transfer::FinalizedAxtSourceTransferErrorV1 as Error;
use super::{State, World};
use crate::kura::tests::CommittedNetworkProofFixture;
use iroha_crypto::{Hash, KeyPair};
use iroha_data_model::{
    account::AccountId,
    asset::id::AssetDefinitionId,
    block::{
        BlockHeader, SignedBlock,
        builder::BlockBuilder,
        execution_output::{ExecutionOutputV1, NetworkExecutionOutputV1},
        output_budget::ExecutionOutputLimits,
    },
    fastpq::{
        FastpqPublicTransferDeltaV1, TransferDeltaTranscript, TransferSmtWitness,
        TransferTranscript,
    },
    nexus::{AxtSourceTransferOccurrenceV1, axt_source_transfer_digest_v1},
    transaction::{FeePaymentIntent, TransactionBuilder, TransactionEntrypoint, TransactionResult},
};
use iroha_model_base::domain::DomainId;
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use nonzero_ext::nonzero;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

const MAX_WORK: u64 = 8;
const MAX_BYTES: u64 = 4 * 1024 * 1024;

fn test_delta(from_before: u32, to_before: u32) -> TransferDeltaTranscript {
    TransferDeltaTranscript {
        from_account: (*ALICE_ID).clone(),
        to_account: (*BOB_ID).clone(),
        asset_definition: AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "rose".parse().expect("asset name"),
        ),
        amount: Quantity::one(),
        from_balance_before: Quantity::from(from_before),
        from_balance_after: Quantity::from(from_before - 1),
        to_balance_before: Quantity::from(to_before),
        to_balance_after: Quantity::from(to_before + 1),
        from_smt_witness: TransferSmtWitness::default(),
        to_smt_witness: TransferSmtWitness::default(),
    }
}

fn source_target(parent: &SignedBlock, rejected: bool, retain_transcripts: bool) -> SignedBlock {
    let keypair: KeyPair = super::checked_keypair();
    let authority = AccountId::new(keypair.public_key().clone());
    let mut builder = BlockBuilder::new(BlockHeader::new(
        nonzero!(2_u64),
        Some(parent.hash()),
        None,
        parent.header().creation_time_ms + 1,
        0,
    ));
    let mut tx = TransactionBuilder::new(
        *super::DEFAULT_TEST_NETWORK_ID,
        authority,
        FeePaymentIntent::authority(Vec::new(), None),
    );
    tx.set_creation_time(Duration::from_millis(7));
    let tx = tx.sign(keypair.private_key());
    let entrypoint = TransactionEntrypoint::External(tx.clone());
    let source_hash = Hash::from(entrypoint.execution_call_hash());
    builder.push_transaction(tx);
    let mut block = builder.build_with_signature(0, keypair.private_key());
    let result = if rejected {
        TransactionResult::new(Err(
            iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                iroha_data_model::ValidationFail::NotPermitted("source rejected".into()),
            ),
        ))
    } else {
        TransactionResult::new(Ok(Vec::new()))
    };
    let transcripts = if retain_transcripts {
        BTreeMap::from([(
            source_hash,
            vec![TransferTranscript {
                batch_hash: source_hash,
                deltas: vec![test_delta(10, 0), test_delta(9, 1)],
                authority_digest: Hash::new(b"source test authority"),
                poseidon_preimage_digest: None,
            }],
        )])
    } else {
        BTreeMap::new()
    };
    block
        .set_execution_outputs(
            vec![ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
                input_index: 0,
                result,
                completions: Vec::new(),
            })],
            u64::from(!rejected),
            transcripts,
            Vec::new(),
            Default::default(),
            BTreeSet::new(),
            &ExecutionOutputLimits {
                max_outputs: 8,
                max_output_bytes: 1024 * 1024,
                max_total_output_bytes: 4 * 1024 * 1024,
                max_executed_wire_bytes: 4 * 1024 * 1024,
            },
        )
        .expect("bounded synthetic source result");
    block
}

fn finalized_fixture(rejected: bool, retain_transcripts: bool) -> CommittedNetworkProofFixture {
    CommittedNetworkProofFixture::new(
        |parent| source_target(parent, rejected, retain_transcripts),
        true,
    )
}

fn source_state(fixture: &CommittedNetworkProofFixture) -> Box<State> {
    let state = Box::new(
        State::try_new_with_chain_and_network_id(
            crate::state::AllocationBudget::new(
                iroha_config::parameters::defaults::pipeline::IVM_EXECUTION_MAX_BYTES,
            ),
            World::default(),
            Arc::clone(&fixture.kura),
            crate::query::store::LiveQueryStore::start_test(),
            (*super::DEFAULT_TEST_CHAIN_ID).clone(),
            fixture.artifacts[0].height_context.network_id,
            #[cfg(feature = "telemetry")]
            Default::default(),
        )
        .expect("State startup over authenticated test Kura"),
    );
    let mut hashes = state.block_hashes.block();
    for block in &fixture.blocks {
        hashes.push(block.hash());
    }
    hashes.commit();
    state
}

fn source_claim(block: &SignedBlock) -> AxtSourceTransferOccurrenceV1 {
    let source = block.network_entrypoint_at(0).expect("source input");
    let source_hash = Hash::from(source.execution_call_hash());
    let delta = test_delta(9, 1);
    AxtSourceTransferOccurrenceV1 {
        source_tx_commitment: source_hash.into(),
        source_success_receipt_digest: [0x71; 32],
        source_tx_index: 0,
        transcript_index: 0,
        delta_index: 1,
        pair_ordinal: 1,
        transfer_digest: axt_source_transfer_digest_v1(&FastpqPublicTransferDeltaV1::from(&delta)),
        remote_spend_claim_commitment: [0x72; 32],
    }
}

#[test]
fn finalized_source_resolver_confirms_success_and_one_exact_transfer() {
    let fixture = finalized_fixture(false, true);
    let state = source_state(&fixture);
    let claim = source_claim(fixture.target());
    let fact = state
        .resolve_finalized_axt_source_transfer_v1(2, &claim, MAX_WORK, MAX_BYTES)
        .expect("QC-bound successful source and exact second delta");
    assert_eq!(fact.height, 2);
    assert_eq!(fact.block_header_hash, fixture.target().hash());
    assert_eq!(fact.source_tx_commitment, claim.source_tx_commitment);
    assert_eq!(fact.source_tx_index, 0);
    assert_eq!(
        (fact.transcript_index, fact.delta_index, fact.pair_ordinal),
        (0, 1, 1)
    );
    assert_eq!(fact.transfer_digest, claim.transfer_digest);
    let unverified = AxtSourceTransferOccurrenceV1 {
        source_success_receipt_digest: [0x75; 32],
        remote_spend_claim_commitment: [0x76; 32],
        ..claim
    };
    assert_eq!(
        state.resolve_finalized_axt_source_transfer_v1(2, &unverified, MAX_WORK, MAX_BYTES),
        Ok(fact),
        "receipt and handle claims require separate authority checks",
    );

    for (changed, expected) in [
        (
            AxtSourceTransferOccurrenceV1 {
                source_tx_commitment: [0x73; 32],
                ..claim
            },
            Error::SourceExecution,
        ),
        (
            AxtSourceTransferOccurrenceV1 {
                source_tx_index: 1,
                ..claim
            },
            Error::SourceExecution,
        ),
        (
            AxtSourceTransferOccurrenceV1 {
                transcript_index: 1,
                ..claim
            },
            Error::TransferCoordinate,
        ),
        (
            AxtSourceTransferOccurrenceV1 {
                delta_index: 0,
                ..claim
            },
            Error::TransferCoordinate,
        ),
        (
            AxtSourceTransferOccurrenceV1 {
                pair_ordinal: 0,
                ..claim
            },
            Error::TransferCoordinate,
        ),
        (
            AxtSourceTransferOccurrenceV1 {
                transfer_digest: [0x74; 32],
                ..claim
            },
            Error::TransferFacts,
        ),
    ] {
        assert_eq!(
            state.resolve_finalized_axt_source_transfer_v1(2, &changed, MAX_WORK, MAX_BYTES),
            Err(expected),
        );
    }
    assert_eq!(
        state.resolve_finalized_axt_source_transfer_v1(2, &claim, 0, MAX_BYTES),
        Err(Error::FinalizedCarrier),
    );
    assert_eq!(
        state.resolve_finalized_axt_source_transfer_v1(0, &claim, MAX_WORK, MAX_BYTES),
        Err(Error::InvalidClaim),
    );
    assert_eq!(
        state.resolve_finalized_axt_source_transfer_v1(3, &claim, MAX_WORK, MAX_BYTES),
        Err(Error::FinalizedCarrier),
    );
}

#[test]
fn finalized_source_resolver_rejects_rejected_or_missing_source_evidence() {
    let rejected = finalized_fixture(true, false);
    let rejected_state = source_state(&rejected);
    let claim = source_claim(rejected.target());
    assert_eq!(
        rejected_state.resolve_finalized_axt_source_transfer_v1(2, &claim, MAX_WORK, MAX_BYTES),
        Err(Error::RejectedExecution),
    );

    let missing = finalized_fixture(false, false);
    let missing_state = source_state(&missing);
    let claim = source_claim(missing.target());
    assert_eq!(
        missing_state.resolve_finalized_axt_source_transfer_v1(2, &claim, MAX_WORK, MAX_BYTES),
        Err(Error::MissingTranscripts),
    );
}

#[test]
fn finalized_source_resolver_requires_canonical_qc_bound_body() {
    let fixture = finalized_fixture(false, true);
    let state = source_state(&fixture);
    let claim = source_claim(fixture.target());
    fixture.make_target_cold();
    let mut wire = fixture.target_disk_bytes();
    let last = wire.len() - 1;
    wire[last] ^= 1;
    fixture.overwrite_target_wire(&wire);
    assert_eq!(
        state.resolve_finalized_axt_source_transfer_v1(2, &claim, MAX_WORK, MAX_BYTES),
        Err(Error::FinalizedCarrier),
    );
}
