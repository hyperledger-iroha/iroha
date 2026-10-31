//! Exact committed proposal ceiling with independently paid native payment originals.

use super::*;
use crate::state::WorldReadOnly as _;
use crate::sumeragi::{
    payload,
    test_chain::native_payment::{
        PaymentKind, assemble, asset, exact_block, fee_asset, payment_chain, recipient,
    },
};
use iroha_allocation::ChargedBuffer;
use iroha_crypto::HashOf;
use iroha_data_model::{
    asset::AssetId,
    block::{SignedBlock, consensus::NexusFeeSettlementV1},
    isi::SettleAtomic,
    isi::settlement::{SettlementInstructionBox, project_atomic_payment_receipt_unverified},
    nexus::FeeDebitSource,
    query::CommittedTransaction,
    sumeragi_finality::{FinalityValidator, SumeragiFinalityVerifier},
    transaction::{Executable, SignedTransaction, TransactionEntrypoint},
};
use iroha_sumeragi::{availability::PayloadBytes, types::ControlWitness};

fn intent(transaction: &SignedTransaction) -> &SettleAtomic {
    let Executable::Instructions(instructions) = transaction.instructions() else {
        panic!("original native instruction carrier");
    };
    let [instruction] = instructions.as_ref() else {
        panic!("one exact payment instruction");
    };
    let instruction = instruction.as_any();
    instruction
        .downcast_ref::<SettleAtomic>()
        .or_else(
            || match instruction.downcast_ref::<SettlementInstructionBox>() {
                Some(SettlementInstructionBox::Atomic(payment)) => Some(payment),
                _ => None,
            },
        )
        .expect("direct atomic payment")
}

fn selected(block: &SignedBlock, index: u32) -> CommittedTransaction {
    let entrypoint = block.network_entrypoint_at(index as usize).unwrap().clone();
    let (output_index, _) = block.network_output_at(index).unwrap();
    let output = block.execution_outputs()[output_index as usize].clone();
    CommittedTransaction {
        block_hash: block.hash(),
        entrypoint_hash: entrypoint.hash(),
        entrypoint_proof: block.network_input_proof(index).unwrap(),
        entrypoint,
        output_hash: HashOf::new(&output),
        output_proof: block.output_proof(output_index).unwrap(),
        output,
    }
}

#[test]
fn legal_full_payment_block_settles_and_first_over_refuses_before_effects() {
    let (mut chain, signer) = payment_chain(PaymentKind::Applied, 4);
    let legal = exact_block(&chain, &signer);
    let first_over = assemble(
        &chain,
        &signer,
        &legal.originals[0],
        legal.second_text_bytes + 1,
    );
    let maximum = chain
        .state()
        .view()
        .world()
        .parameters()
        .sumeragi()
        .max_block_bytes
        .get() as usize;
    assert_eq!(
        legal.proposal.resultless_proposal_wire_len().unwrap(),
        maximum
    );
    assert_eq!(
        first_over.proposal.resultless_proposal_wire_len().unwrap(),
        maximum + 1
    );

    let original = chain.author_proposal(legal.proposal, ControlWitness::default());
    assert_eq!(original.payload().as_slice().len(), maximum);
    assert_eq!(
        original.source().config().params.max_block_bytes as usize,
        maximum
    );
    let over_payload = payload::encode(&first_over.proposal).unwrap();
    assert_eq!(over_payload.len(), maximum + 1);
    // The production payload domain refuses the first-over original before an
    // authoring source can exist. Its typed refusal preserves the exact charged
    // bytes and original State pool; no signing or execution is entered.
    let budget = chain.state().ivm_execution_budget();
    let before = budget.reserved_bytes();
    let mut backing = ChargedBuffer::new(over_payload.len(), &budget).unwrap();
    backing.append(&over_payload).unwrap();
    let pointer = backing.as_slice().as_ptr();
    let charged = budget.reserved_bytes();
    let (refused, error) = PayloadBytes::from_charged(backing, &budget)
        .expect_err("first-over bytes cannot enter the production payload domain");
    assert_eq!(
        error.to_string(),
        format!("invalid shared byte sequence length {}", maximum + 1)
    );
    assert_eq!(refused.as_slice(), over_payload.as_slice());
    assert_eq!(refused.as_slice().as_ptr(), pointer);
    assert!(refused.belongs_to(&budget));
    assert_eq!(budget.reserved_bytes(), charged);
    drop(refused);
    assert_eq!(budget.reserved_bytes(), before);
    drop(over_payload);
    assert_eq!(chain.height(), 1);
    {
        let view = chain.state().view();
        assert!(
            view.world()
                .assets
                .get(&AssetId::of(asset("cash"), recipient()))
                .is_none()
        );
        assert_eq!(
            view.world()
                .assets
                .get(&AssetId::of(fee_asset(), chain.genesis_account().clone()))
                .unwrap()
                .0,
            10_u32.into()
        );
        for transaction in &first_over.originals {
            assert!(
                view.world()
                    .settlement_receipts
                    .get(&intent(transaction).settlement_id)
                    .is_none()
            );
        }
    }
    drop(first_over);
    let committed = chain
        .begin_available_proposal(original)
        .expect("exact admitted original executes")
        .publish(Signers::Quorum)
        .expect("actual native quorum publication");
    assert_eq!(chain.height(), 2);
    assert_eq!(committed.block().network_entrypoint_count(), 2);
    assert!(committed.block().encode_wire().unwrap().len() <= MAX_FINALITY_BLOCK_BYTES);
    check_settled(&chain, &legal.originals);
    check_independent_selected_receipts(&chain, &legal.originals);
    drop(committed);
    let later = exact_block(&chain, &signer);
    assert_eq!(
        later.proposal.resultless_proposal_wire_len().unwrap(),
        maximum
    );
    chain.commit_proposal(later.proposal, Signers::Quorum, ControlWitness::default());
    assert_eq!(chain.height(), 3);
    check_settled(&chain, &later.originals);
    check_independent_selected_receipts(&chain, &later.originals);
    assert_eq!(
        chain.height(),
        3,
        "the final payment requires no unrelated successor traffic"
    );
}

#[inline(never)]
fn check_settled(chain: &CertifiedTestChain, originals: &[SignedTransaction; 2]) {
    let view = chain.state().view();
    let paid = 2 * u32::try_from(chain.height() - 1).unwrap();
    assert_eq!(
        view.world()
            .assets
            .get(&AssetId::of(asset("cash"), recipient()))
            .unwrap()
            .0,
        paid.into()
    );
    assert_eq!(
        view.world()
            .assets
            .get(&AssetId::of(fee_asset(), chain.genesis_account().clone()))
            .unwrap()
            .0,
        (10 - paid).into()
    );
    assert_eq!(
        view.world()
            .asset_definitions
            .get(&fee_asset())
            .unwrap()
            .total_quantity(),
        &(10 - paid).into()
    );
    assert!(
        view.world()
            .assets
            .get(&AssetId::of(fee_asset(), recipient()))
            .is_none()
    );
    let committed = chain.committed(chain.height());
    for (index, original) in originals.iter().enumerate() {
        let input = committed.block().network_entrypoint_at(index).unwrap();
        assert_eq!(input, &TransactionEntrypoint::External(original.clone()));
        let output = &committed
            .block()
            .network_output_at(index as u32)
            .unwrap()
            .1
            .result;
        assert!(output.is_ok());
        let fee = output
            .nexus_fee_receipt()
            .expect("actual paid source outcome");
        fee.validate_for_network_input(input, chain.height())
            .unwrap();
        assert_eq!(fee.fee_amount, 1_u32.into());
        assert_eq!(fee.fee_asset_id, fee_asset());
        assert_eq!(
            fee.debit_source,
            FeeDebitSource::Account(chain.genesis_account().clone())
        );
        assert_eq!(fee.settlement, NexusFeeSettlementV1::Burn);
        assert!(
            view.world()
                .settlement_receipts
                .get(&intent(original).settlement_id)
                .is_some()
        );
    }
}

#[inline(never)]
fn check_independent_selected_receipts(
    chain: &CertifiedTestChain,
    originals: &[SignedTransaction; 2],
) {
    let view = chain.state().view();
    let height = chain.height();
    let source = NativeFinalityProofSource::new(
        view.chain_id(),
        view.network_id(),
        view.block_hashes(),
        view.kura(),
    );
    let owner = NativeFinalityProofBudget::new(
        AllocationBudget::new(default_frame_bytes()),
        // Core's standalone proof control measures the genuine full-block path.
        // Actual default admission is qualified by Torii's original HTTP owner,
        // whose concrete response controls Core cannot own across this boundary.
        AllocationBudget::new(GENEROUS_TEST_GRAPH_BYTES),
        norito::DecodeLimits::new(
            MAX_FINALITY_BLOCK_BYTES,
            MAX_FINALITY_BLOCK_BYTES,
            usize::MAX,
            usize::MAX,
            128,
        ),
    );
    let ((interval, live), costs) =
        super::super::super::super::interval_budget::cost_observation::measure(|| {
            live_allocations_during(65_536, || {
                build_proof_interval(
                    &source,
                    &owner,
                    &limits(height, height),
                    &AtomicBool::new(false),
                )
            })
        });
    eprintln!("full16MiB paid native finality H{height} decoder admissions: {costs:?}");
    report_live_cost(live);
    let interval =
        interval.expect("explicit diagnostic custody verifies the legal full payment block");
    let proof = interval.proof(0).unwrap();
    eprintln!(
        "full16MiB paid native finality H{height} wire={} proof_frame={}",
        proof.block_wire.len(),
        norito::canonical_frame_len(proof).unwrap()
    );
    let validators = chain
        .validated_genesis()
        .validator_pops()
        .iter()
        .map(|(key, pop)| FinalityValidator {
            public_key: key.clone(),
            proof_of_possession: pop.clone(),
        })
        .collect();
    let mut verifier = SumeragiFinalityVerifier::new(
        chain.validated_genesis().block(),
        view.chain_id().as_ref(),
        validators,
    )
    .unwrap();
    for prefix_height in 1..height {
        verifier
            .verify(&super::super::super::super::build_proof(&view, prefix_height).unwrap())
            .unwrap();
    }
    let verified = verifier.verify(proof).unwrap();
    for (index, original) in originals.iter().enumerate() {
        let evidence = selected(verified.block(), index as u32);
        verified
            .verify_committed_transaction(&chain.network_id(), &evidence)
            .unwrap();
        assert_eq!(
            evidence.entrypoint(),
            &TransactionEntrypoint::External(original.clone())
        );
        let projected = project_atomic_payment_receipt_unverified(
            chain.network_id(),
            intent(original),
            &verified.header(),
            evidence.entrypoint(),
            evidence.result(),
        )
        .unwrap();
        assert_eq!(
            &projected,
            view.world()
                .settlement_receipts
                .get(&intent(original).settlement_id)
                .unwrap()
        );
        assert!(
            project_atomic_payment_receipt_unverified(
                chain.network_id(),
                intent(&originals[1 - index]),
                &verified.header(),
                evidence.entrypoint(),
                evidence.result()
            )
            .is_err()
        );
    }
}
