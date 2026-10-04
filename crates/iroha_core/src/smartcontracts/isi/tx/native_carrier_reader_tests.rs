//! Native execution provenance and complete source-budget controls for offchain consumers.

use super::*;
use crate::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};

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
