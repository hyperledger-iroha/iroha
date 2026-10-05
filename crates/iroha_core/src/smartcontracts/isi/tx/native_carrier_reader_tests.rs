//! Native execution provenance and complete source-budget controls for offchain consumers.

use super::*;
use crate::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};

fn history_read_budget() -> crate::state::CanonicalHistoryReadBudget {
    crate::state::CanonicalHistoryReadBudget::new(
        iroha_allocation::AllocationBudget::new(48 * 1024 * 1024),
        norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
            1_000_000,
            48 * 1024 * 1024,
            1_000_000,
            48 * 1024 * 1024,
            64,
        )),
    )
}

pub(super) fn chain() -> CertifiedTestChain {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    chain.commit(Vec::new());
    chain
}

pub(super) fn bounds(chain: &CertifiedTestChain, height: u64) -> (u64, u64) {
    let block = chain.committed(height);
    let validation = block
        .block()
        .network_entrypoint_count()
        .max(block.block().execution_outputs().len())
        .max(1) as u64;
    let source_bytes = (1..=height.max(2))
        .map(|height| chain.committed(height).block().encode_wire().unwrap().len() as u64)
        .sum();
    (height.max(2) + validation, source_bytes)
}

#[test]
fn native_carrier_charges_the_complete_source_and_preserves_exact_outputs() {
    let chain = chain();
    for height in 1..=2 {
        let (work, bytes) = bounds(&chain, height);
        let carrier = chain
            .state()
            .read_finalized_execution_carrier(
                NonZeroUsize::new(height as usize).unwrap(),
                work,
                bytes,
            )
            .unwrap();
        assert_eq!(carrier.work_items(), work);
        assert_eq!(carrier.wire_bytes(), bytes);
        assert_eq!(
            carrier.block().encode_wire().unwrap(),
            chain.committed(height).block().encode_wire().unwrap()
        );
        assert!(!chain.kura().store_root().join("v2_finality").exists());
    }
}

#[test]
fn cold_checkpoint_reader_uses_original_frame_pool_and_refunds_after_last_block() {
    let chain = chain();
    let frames = iroha_allocation::AllocationBudget::new(0);
    let context = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
        1_000_000, 48_000_000, 1_000_000, 48_000_000, 64,
    ));
    let budget = crate::state::CanonicalHistoryReadBudget::new(frames.clone(), context.clone());
    let height = NonZeroUsize::new(2).unwrap();
    assert!(
        chain
            .state()
            .read_executed_carrier_from_checkpoints(height, 1_000, 1 << 40, &budget)
            .is_err()
    );
    assert_eq!(frames.reserved_bytes(), 0);
    frames.set_limit_bytes(48_000_000);
    let carrier = chain
        .state()
        .read_executed_carrier_from_checkpoints(height, 1_000, 1 << 40, &budget)
        .unwrap();
    assert!(carrier.block().belongs_to(&frames));
    assert!(context.consumed_allocated_bytes() > carrier.wire_bytes() as u64);
    let held = carrier.block().clone();
    let charge = frames.reserved_bytes();
    assert!(charge > 0);
    drop(carrier);
    assert_eq!(frames.reserved_bytes(), charge);
    drop(held);
    assert_eq!(frames.reserved_bytes(), 0);
}

#[test]
fn cold_checkpoint_reader_refuses_native_allocation_expansion_before_graph_publication() {
    let chain = chain();
    let frames = iroha_allocation::AllocationBudget::new(48_000_000);
    let context = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
        1_000_000, 48_000_000, 1_000_000, 1, 64,
    ));
    let budget = crate::state::CanonicalHistoryReadBudget::new(frames.clone(), context.clone());
    assert!(
        chain
            .state()
            .read_executed_carrier_from_checkpoints(
                NonZeroUsize::new(2).unwrap(),
                1_000,
                1 << 40,
                &budget,
            )
            .is_err()
    );
    assert_eq!(frames.reserved_bytes(), 0);
    assert_eq!(context.consumed_allocated_bytes(), 0);
}

#[test]
fn checkpointed_carrier_reads_start_near_their_target() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    for _ in 0..5 {
        chain.commit(Vec::new());
    }
    let tip = u64::try_from(chain.state().view().height()).unwrap();
    assert!(tip >= 4, "the chain spans several blocks");
    let validation = |height: u64| {
        let block = chain.committed(height);
        block
            .block()
            .network_entrypoint_count()
            .max(block.block().execution_outputs().len())
            .max(1) as u64
    };
    let read = |height: u64| {
        chain
            .state()
            .read_executed_carrier_from_checkpoints(
                NonZeroUsize::new(usize::try_from(height).unwrap()).unwrap(),
                1_000,
                1 << 40,
                &history_read_budget(),
            )
            .unwrap()
    };
    // Cold: the walk starts at the tip and pays for every newer block.
    chain.kura().history_checkpoints().clear();
    let cold = read(2);
    assert_eq!(cold.work_items(), tip - 1 + validation(2));
    assert_eq!(
        cold.block().encode_wire().unwrap(),
        chain.committed(2).block().encode_wire().unwrap()
    );
    let finalized = chain
        .state()
        .read_finalized_execution_carrier(NonZeroUsize::new(2).unwrap(), 1_000, 1 << 40)
        .unwrap();
    assert_eq!(
        cold.block().encode_wire().unwrap(),
        finalized.block().encode_wire().unwrap(),
        "both readers authenticate the same execution"
    );
    // Warm: the cold walk remembered every identity it verified.
    let warm = read(3);
    assert_eq!(warm.work_items(), 1 + validation(3));
    assert_eq!(
        warm.block().encode_wire().unwrap(),
        chain.committed(3).block().encode_wire().unwrap()
    );
    assert!(
        matches!(
            chain.state().read_executed_carrier_from_checkpoints(
                NonZeroUsize::new(3).unwrap(),
                validation(3),
                1 << 40,
                &history_read_budget()
            ),
            Err(crate::execution_attempt::ExecutionAttemptError::Rejected(
                QueryExecutionFail::GasBudgetExceeded
            ))
        ),
        "the source block counts against the work bound"
    );
}

#[test]
fn checkpointed_reads_discard_foreign_journal_entries_and_recheck_execution() {
    use crate::kura::history_checkpoints::HistoryCheckpoint;
    let mut chain = chain();
    chain.commit(Vec::new());
    let height = NonZeroUsize::new(2).unwrap();
    let source = chain.committed(2);
    let checkpoints = chain.kura().history_checkpoints();
    let authentic = HistoryCheckpoint {
        iroha_hash: source.block_hash(),
        core_hash: source.core_hash(),
        result: source.result(),
    };
    checkpoints.record(
        2,
        HistoryCheckpoint {
            iroha_hash: HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(b"foreign history")),
            ..authentic
        },
    );
    let read = || {
        chain.state().read_executed_carrier_from_checkpoints(
            height,
            1_000,
            1 << 40,
            &history_read_budget(),
        )
    };
    let repaired =
        read().expect("foreign checkpoint is discarded and the tip authenticates history");
    assert_eq!(
        repaired.block().encode_wire().unwrap(),
        source.block().encode_wire().unwrap()
    );
    assert!(
        checkpoints
            .candidates(2, 2)
            .iter()
            .copied()
            .eq([(2, authentic)])
    );

    checkpoints.record(
        2,
        HistoryCheckpoint {
            result: iroha_sumeragi::types::Hash32([0xD7; 32]),
            ..authentic
        },
    );
    assert!(
        read().is_err(),
        "a matching journal hash never bypasses native result validation"
    );
}

#[test]
fn checkpoint_warming_does_not_change_consensus_history_metering() {
    let mut chain = chain();
    chain.commit(Vec::new());
    let height = NonZeroUsize::new(2).unwrap();
    chain.kura().history_checkpoints().clear();
    let read = || {
        chain
            .state()
            .read_finalized_execution_carrier(height, 1_000, 1 << 40)
            .unwrap()
    };
    let cold = read();
    let warm = read();
    assert_eq!(cold.work_items(), warm.work_items());
    assert_eq!(cold.wire_bytes(), warm.wire_bytes());
    assert_eq!(
        cold.block().encode_wire().unwrap(),
        warm.block().encode_wire().unwrap()
    );
    let offchain_cold = chain
        .state()
        .read_executed_carrier_from_checkpoints(height, 1_000, 1 << 40, &history_read_budget())
        .unwrap();
    let offchain = chain
        .state()
        .read_executed_carrier_from_checkpoints(height, 1_000, 1 << 40, &history_read_budget())
        .unwrap();
    assert!(offchain.work_items() < offchain_cold.work_items());
    assert!(offchain.work_items() < warm.work_items());
    let after_offchain = read();
    assert_eq!(after_offchain.work_items(), cold.work_items());
    assert_eq!(after_offchain.wire_bytes(), cold.wire_bytes());
}

#[test]
fn native_carrier_refusal_never_projects_partial_transaction_results() {
    let chain = chain();
    let height = NonZeroUsize::new(2).unwrap();
    let block = chain.committed(2);
    let (work, bytes) = bounds(&chain, 2);
    for (max_work, max_bytes) in [(0, bytes), (work, 0), (work - 1, bytes), (work, bytes - 1)] {
        let mut visits = 0;
        assert!(matches!(
            visit_finalized_network_transactions(
                chain.state(),
                height,
                block.block_hash(),
                max_work,
                max_bytes,
                |_, _| visits += 1,
            ),
            Err(crate::execution_attempt::ExecutionAttemptError::Rejected(
                QueryExecutionFail::GasBudgetExceeded
            ))
        ));
        assert_eq!(visits, 0);
    }
    let foreign = HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(b"foreign carrier"));
    let mut visits = 0;
    assert!(
        visit_finalized_network_transactions(
            chain.state(),
            height,
            foreign,
            work,
            bytes,
            |_, _| visits += 1,
        )
        .is_err()
    );
    assert_eq!(visits, 0);
    visit_finalized_network_transactions(
        chain.state(),
        height,
        block.block_hash(),
        work,
        bytes,
        |_, _| visits += 1,
    )
    .unwrap();
    assert_eq!(visits, block.block().network_entrypoint_count());
}

#[test]
fn native_carrier_requires_actual_genesis_successor_and_configured_instance() {
    let chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    assert!(
        chain
            .state()
            .read_finalized_execution_carrier(NonZeroUsize::MIN, 128, 16 * 1024 * 1024,)
            .is_err()
    );
    let chain = self::chain();
    let view = chain.state().view();
    let block = chain.committed(2);
    let (work, bytes) = bounds(&chain, 2);
    assert!(
        read_finalized_execution_carrier(
            chain.kura(),
            &iroha_model_base::chain::ChainId::from("foreign-instance"),
            *view.network_id(),
            view.block_hashes(),
            NonZeroUsize::new(2).unwrap(),
            block.block_hash(),
            work,
            bytes,
            &chain.state().ivm_execution_budget(),
        )
        .is_err()
    );
}
