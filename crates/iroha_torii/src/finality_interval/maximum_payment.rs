//! Genuine maximum paid originals through the original native HTTP memory owner.

use super::*;
use iroha_core::{
    state::{StateReadOnly as _, WorldReadOnly as _},
    sumeragi::test_chain::native_payment::{self, PaymentKind, asset, fee_asset, recipient},
};
use iroha_crypto::HashOf;
use iroha_data_model::{
    asset::AssetId,
    block::{SignedBlock, consensus::NexusFeeSettlementV1},
    isi::{
        SettleAtomic,
        settlement::{SettlementInstructionBox, project_atomic_payment_receipt_unverified},
    },
    nexus::FeeDebitSource,
    query::CommittedTransaction,
    sumeragi_finality::{FinalityValidator, SumeragiFinalityVerifier},
    transaction::{Executable, SignedTransaction, TransactionEntrypoint},
};
use mv::storage::StorageReadOnly as _;

pub(crate) fn paid_chain() -> CertifiedTestChain {
    native_payment::maximum_payment_chain(PaymentKind::Applied)
}

fn assert_paid_effects(chain: &CertifiedTestChain, payments: u32) {
    assert_eq!(
        chain.height(),
        3,
        "no successor traffic after the final payment"
    );
    let view = chain.state().view();
    assert_eq!(
        view.world()
            .assets()
            .get(&AssetId::of(asset("cash"), recipient()))
            .unwrap()
            .0,
        payments.into()
    );
    assert_eq!(
        view.world()
            .assets()
            .get(&AssetId::of(fee_asset(), chain.genesis_account().clone()))
            .unwrap()
            .0,
        (10 - payments).into()
    );
    assert_eq!(
        view.world()
            .asset_definitions()
            .get(&fee_asset())
            .unwrap()
            .total_quantity(),
        &(10 - payments).into()
    );
    assert!(
        view.world()
            .assets()
            .get(&AssetId::of(fee_asset(), recipient()))
            .is_none()
    );
}

fn selected(block: &SignedBlock, input_index: u32) -> CommittedTransaction {
    let entrypoint = block
        .network_entrypoint_at(input_index as usize)
        .unwrap()
        .clone();
    let (index, _) = block.network_output_at(input_index).unwrap();
    let output = block.execution_outputs()[index as usize].clone();
    CommittedTransaction {
        block_hash: block.hash(),
        entrypoint_hash: entrypoint.hash(),
        entrypoint_proof: block.network_input_proof(input_index).unwrap(),
        entrypoint,
        output_hash: HashOf::new(&output),
        output_proof: block.output_proof(index).unwrap(),
        output,
    }
}

fn intent(original: &SignedTransaction) -> &SettleAtomic {
    let Executable::Instructions(instructions) = original.instructions() else {
        panic!("native original");
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
        .unwrap()
}

fn verify(
    chain: &CertifiedTestChain,
    proofs: &[SumeragiFinalityProof],
    originals: Option<&[[SignedTransaction; 2]; 2]>,
) {
    let view = chain.state().view();
    let validators = chain
        .validated_genesis()
        .validator_pops()
        .iter()
        .map(|(public_key, pop)| FinalityValidator {
            public_key: public_key.clone(),
            proof_of_possession: pop.clone(),
        })
        .collect();
    let mut verifier = SumeragiFinalityVerifier::new(
        chain.validated_genesis().block(),
        view.chain_id().as_ref(),
        validators,
    )
    .unwrap();
    for height in 1..proofs[0].block_header.height().get() {
        verifier
            .verify(&build_proof(&view, height).unwrap())
            .unwrap();
    }
    for proof in proofs {
        let verified = verifier.verify(proof).unwrap();
        let expected = originals
            .map(|blocks| &blocks[usize::try_from(verified.header().height().get() - 2).unwrap()]);
        if expected.is_some() {
            assert_eq!(verified.block().network_entrypoint_count(), 2);
            assert_eq!(verified.block().external_transactions().count(), 2);
        }
        for (index, original) in verified.block().external_transactions().enumerate() {
            let evidence = selected(verified.block(), u32::try_from(index).unwrap());
            verified
                .verify_committed_transaction(&chain.network_id(), &evidence)
                .unwrap();
            if let Some(expected) = expected {
                assert_eq!(
                    evidence.entrypoint(),
                    &TransactionEntrypoint::External(expected[index].clone())
                );
            }
            assert!(evidence.result().is_ok());
            let fee = evidence.result().nexus_fee_receipt().unwrap();
            fee.validate_for_network_input(evidence.entrypoint(), verified.header().height().get())
                .unwrap();
            assert_eq!(fee.fee_amount, 1_u32.into());
            assert_eq!(fee.fee_asset_id, fee_asset());
            assert_eq!(
                fee.debit_source,
                FeeDebitSource::Account(chain.genesis_account().clone())
            );
            assert_eq!(fee.settlement, NexusFeeSettlementV1::Burn);
            let payment = intent(original);
            let receipt = project_atomic_payment_receipt_unverified(
                chain.network_id(),
                payment,
                &verified.header(),
                evidence.entrypoint(),
                evidence.result(),
            )
            .unwrap();
            assert_eq!(
                &receipt,
                view.world()
                    .settlement_receipts()
                    .get(&payment.settlement_id)
                    .unwrap()
            );
            if let Some(expected) = expected {
                assert!(
                    project_atomic_payment_receipt_unverified(
                        chain.network_id(),
                        intent(&expected[1 - index]),
                        &verified.header(),
                        evidence.entrypoint(),
                        evidence.result(),
                    )
                    .is_err()
                );
            }
        }
    }
}

fn check_original_http_owner(
    chain: &CertifiedTestChain,
    originals: Option<&[[SignedTransaction; 2]; 2]>,
    minimum_response: usize,
) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let working = usize::try_from(
        iroha_config::parameters::defaults::torii::FINALITY_MAX_WORKING_SET_BYTES.0,
    )
    .unwrap();
    let limit =
        usize::try_from(iroha_config::parameters::defaults::torii::MAX_CONTENT_LEN.0).unwrap();
    assert_eq!(working, 384_000_000);
    for selection in [Selection::Single(3), Selection::Interval { from: 2, to: 3 }] {
        let pool = ByteWeightedMemoryPool::new(working).unwrap();
        let envelope = QueryFanoutMemoryEnvelope::for_native_finality(working, limit).unwrap();
        let fixed = crate::query_fanout_fixed_overhead_bytes().unwrap();
        let graphs = working - fixed - envelope.route_body_bytes - envelope.candidate_encoded_bytes;
        eprintln!(
            "actual native finality owner: W={working} fixed={fixed} frames={} metadata={} graphs={graphs}",
            envelope.route_body_bytes, envelope.candidate_encoded_bytes,
        );
        let reservation = QueryFanoutMemoryReservation::from_admitted_fanout(
            pool.try_acquire_parts([working as u64]).unwrap(),
            envelope,
            pool.generation(),
        )
        .unwrap();
        let query = Arc::new(tokio::sync::Semaphore::new(1));
        let admission = QueryAdmissionPermit {
            _query: Arc::clone(&query).try_acquire_owned().unwrap(),
            heavy: None,
            body: None,
            fanout_memory: Some(reservation),
        };
        let owner = history_producer::HistoryProducerOwner::from_admission(&admission).unwrap();
        let frames = owner.cold_frames().clone();
        drop(owner);
        let prepared = runtime
            .block_on(prepare(
                Arc::clone(chain.state()),
                selection,
                ResponseFormat::Norito,
                admission,
                limit,
                deadline(),
            ))
            .unwrap();
        assert_eq!(prepared.response.status(), StatusCode::OK);
        assert!(prepared.bytes > minimum_response);
        assert_eq!(frames.reserved_bytes(), prepared.bytes);
        assert_eq!(query.available_permits(), 1);
        let bytes = runtime
            .block_on(axum::body::to_bytes(prepared.response.into_body(), limit))
            .unwrap();
        let proofs = match selection {
            Selection::Single(_) => {
                vec![norito::decode_canonical::<SumeragiFinalityProof>(&bytes).unwrap()]
            }
            Selection::Interval { .. } => {
                norito::decode_canonical::<Vec<SumeragiFinalityProof>>(&bytes).unwrap()
            }
        };
        let (from, to) = selection.range();
        assert_eq!(proofs.len(), usize::try_from(to - from + 1).unwrap());
        for (proof, height) in proofs.iter().zip(from..=to) {
            assert_eq!(proof.block_header.height().get(), height);
        }
        verify(chain, &proofs, originals);
        drop(proofs);
        let last = bytes.slice(..1);
        drop(bytes);
        assert_eq!(pool.available_bytes(), 0);
        assert!(pool.try_acquire_parts([1]).is_none());
        assert_eq!(frames.reserved_bytes(), prepared.bytes);
        drop(last);
        assert_eq!(frames.reserved_bytes(), 0);
        assert_eq!(pool.available_bytes(), u64::try_from(working).unwrap());
    }
}

#[test]
fn maximum_paid_native_single_and_interval_keep_original_pool_until_last_http_slice() {
    let chain = paid_chain();
    assert_paid_effects(&chain, 2);
    check_original_http_owner(&chain, None, 10 * 1024 * 1024);
}

#[test]
fn full_payment_blocks_use_actual_native_owner_for_every_receipt_and_last_http_slice() {
    let (chain, originals) = native_payment::full_payment_chain();
    assert_paid_effects(&chain, 4);
    check_original_http_owner(&chain, Some(&originals), 16 * 1024 * 1024);
    assert_eq!(
        chain.height(),
        3,
        "final paid block needs no unrelated successor"
    );
}
