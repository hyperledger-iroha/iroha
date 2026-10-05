//! Native owner, funded MV custody, deterministic history and restore assertions.
use super::*;
use crate::sumeragi::test_chain::{CertifiedTestChain, Signers, TestChainConfig};

fn chain() -> CertifiedTestChain {
    CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap()
}

#[test]
fn original_genesis_and_worker_publish_exact_tip_with_undo() {
    let mut chain = chain();
    let old_view = chain.state().view();
    let genesis = old_view.native_execution_tip().unwrap();
    let genesis_receipt = crate::sumeragi::certified_chain::committed_block(&old_view, 1).unwrap();
    assert_eq!(genesis.core_hash(), genesis_receipt.core_hash());
    assert_eq!(genesis.result(), genesis_receipt.result());
    assert_eq!(*old_view.native_execution_tip_predecessor.get(), Some(None));
    let state = Arc::clone(chain.state());
    // Release the chain borrow while retaining the actual old EBR cut through State.
    drop(old_view);
    let old_view = state.view();
    chain.commit(Vec::new());
    let view = state.view();
    let current = view.native_execution_tip().unwrap();
    let receipt = chain.committed(2);
    assert_eq!(current.height(), 2);
    assert_eq!(current.iroha_hash(), receipt.block_hash());
    assert_eq!(current.core_hash(), receipt.core_hash());
    assert_eq!(current.result(), receipt.result());
    assert_eq!(
        *view.native_execution_tip_predecessor.get(),
        Some(Some(genesis))
    );
    assert_eq!(old_view.native_execution_tip(), Some(genesis));
    assert_eq!(old_view.height(), 1);
}

#[test]
fn rejected_original_preparation_and_discard_preserve_committed_tip() {
    let mut chain = chain();
    let state = Arc::clone(chain.state());
    let before = state.view().native_execution_tip();
    let proposal = chain.proposal(None, Vec::new());
    let mut original = chain
        .begin_proposal(proposal, iroha_sumeragi::types::ControlWitness::default())
        .unwrap();
    assert_eq!(
        original
            .inspect(|owner| owner.state.native_execution_tip())
            .unwrap(),
        before
    );
    assert!(original.prepare(Signers::BelowQuorum).is_err());
    assert_eq!(state.view().native_execution_tip(), before);
    drop(original);
    assert_eq!(state.view().native_execution_tip(), before);
    chain.commit(Vec::new());
    assert_eq!(state.view().native_execution_tip().unwrap().height(), 2);
}

#[test]
fn replacement_reads_exact_tip_undo_and_abandonment_restores_original_cut() {
    let mut chain = chain();
    let previous = chain.state().view().native_execution_tip();
    chain.commit(Vec::new());
    let current = chain.state().view().native_execution_tip();
    let header = chain.committed(2).block().header();
    let mut replacement = chain.state().block_and_revert(header);
    assert_eq!(replacement.native_execution_tip(), previous);
    assert_eq!(replacement.transaction().native_execution_tip(), previous);
    drop(replacement);
    assert_eq!(chain.state().view().native_execution_tip(), current);
}

#[test]
fn execution_history_admits_each_actual_source_before_read() {
    let mut chain = chain();
    chain.commit(Vec::new());
    chain.commit(Vec::new());
    let view = chain.state().view();
    let mut admitted = Vec::new();
    let genesis = view
        .canonical_history()
        .executed_block(NonZeroUsize::MIN, |count, bytes| {
            admitted.push((count, bytes));
            Ok(())
        })
        .unwrap();
    assert_eq!(genesis.hash(), chain.genesis().hash());
    assert_eq!(admitted.len(), 3);
    assert!(
        admitted
            .iter()
            .all(|&(count, bytes)| count == 1 && bytes > 0)
    );
    let mut attempts = 0;
    assert!(
        view.canonical_history()
            .executed_block(NonZeroUsize::MIN, |_, _| {
                attempts += 1;
                if attempts == 2 {
                    Err(crate::execution_attempt::ExecutionAttemptError::Deferred(
                        ivm::error::ExecutionDeferral::CanonicalHistoryCapacity.into(),
                    ))
                } else {
                    Ok(())
                }
            })
            .is_err()
    );
    assert_eq!(attempts, 2);
    chain
        .kura()
        .corrupt_native_frame_for_test(NonZeroUsize::new(2).unwrap());
    assert!(
        view.canonical_history()
            .executed_block(NonZeroUsize::MIN, |_, _| Ok(()))
            .is_err()
    );
}

fn snapshot_claim(state: &State) -> NativeExecutionTipSnapshot {
    let view = state.view();
    let claim = NativeExecutionTipSnapshot::from_original(
        *view.native_execution_tip.get(),
        *view.native_execution_tip_predecessor.get(),
    );
    norito::json::from_str(&norito::json::to_json(&claim).unwrap()).unwrap()
}

#[test]
fn restore_reauthenticates_native_current_and_undo_and_rejects_claim_substitution() {
    let mut chain = chain();
    chain.commit(Vec::new());
    let view = chain.state().view();
    let hashes: Vec<_> = view.block_hashes().iter().copied().collect();
    let restore = |claim: NativeExecutionTipSnapshot| {
        claim.restore(
            &chain.state().ivm_execution_budget(),
            view.chain_id(),
            view.network_id(),
            &hashes,
            chain.kura(),
        )
    };
    let restored = restore(snapshot_claim(chain.state())).unwrap();
    assert_eq!(*restored.view().get(), view.native_execution_tip());
    assert_eq!(
        *restored.predecessor_view().get(),
        *view.native_execution_tip_predecessor.get()
    );
    let mut wrong_result = snapshot_claim(chain.state());
    wrong_result.blocks.as_mut().unwrap().result[0] ^= 1;
    assert!(restore(wrong_result).is_err());
    let mut wrong_undo = snapshot_claim(chain.state());
    wrong_undo
        .revert
        .as_mut()
        .unwrap()
        .value
        .as_mut()
        .unwrap()
        .core_hash[0] ^= 1;
    assert!(restore(wrong_undo).is_err());
    assert!(
        snapshot_claim(chain.state())
            .restore(
                &chain.state().ivm_execution_budget(),
                &"another-native-instance".parse().unwrap(),
                view.network_id(),
                &hashes,
                chain.kura(),
            )
            .is_err()
    );
}

#[test]
fn restore_rebuilds_sparse_history_checkpoints_from_verified_snapshot_prefix() {
    use crate::kura::history_checkpoints::HISTORY_CHECKPOINT_INTERVAL;

    let mut chain = chain();
    let checkpoint_height = HISTORY_CHECKPOINT_INTERVAL;
    let tip_height = checkpoint_height + 2;
    for _ in 1..tip_height {
        chain.commit(Vec::new());
    }
    let checkpoint = checkpoint_of(record(&chain.committed(checkpoint_height)));
    let target_height = checkpoint_height - 1;
    let target_hash = chain.committed(target_height).block_hash();
    let claim = snapshot_claim(chain.state());
    let view = chain.state().view();
    let hashes: Vec<_> = view.block_hashes().iter().copied().collect();
    let checkpoints = chain.kura().history_checkpoints();
    // A restart has the authenticated journal and decoded snapshot, but no
    // node-local checkpoints retained from the original executions.
    checkpoints.clear();
    assert!(checkpoints.candidates(1, tip_height).is_empty());
    let restored = claim
        .restore(
            &chain.state().ivm_execution_budget(),
            view.chain_id(),
            view.network_id(),
            &hashes,
            chain.kura(),
        )
        .unwrap();
    assert_eq!(*restored.view().get(), view.native_execution_tip());
    assert_eq!(
        *restored.predecessor_view().get(),
        *view.native_execution_tip_predecessor.get()
    );
    assert_eq!(checkpoints.sparse_len(), 1);
    assert_eq!(
        checkpoints.recent_len(),
        0,
        "restore retains only sparse identities"
    );
    assert!(
        checkpoints
            .candidates(1, tip_height)
            .iter()
            .copied()
            .eq([(checkpoint_height, checkpoint)])
    );

    let target = NonZeroUsize::new(usize::try_from(target_height).unwrap()).unwrap();
    let mut source_reads = 0;
    let mut visited = Vec::new();
    view.canonical_history()
        .visit_executed_backwards_from_checkpoints(
            target,
            target,
            |count, _| {
                source_reads += count;
                Ok(())
            },
            |block| {
                visited.push(block.block_hash());
                Ok(core::ops::ControlFlow::Continue(()))
            },
        )
        .unwrap();
    assert_eq!(
        source_reads, 2,
        "the restored checkpoint bounds the cold read"
    );
    assert_eq!(visited, [target_hash]);
}

#[test]
fn genesis_only_snapshot_cannot_decode_its_unsigned_result_into_authority() {
    let chain = chain();
    let view = chain.state().view();
    let hashes: Vec<_> = view.block_hashes().iter().copied().collect();
    assert!(
        snapshot_claim(chain.state())
            .restore(
                &chain.state().ivm_execution_budget(),
                view.chain_id(),
                view.network_id(),
                &hashes,
                chain.kura(),
            )
            .is_err()
    );
}

#[test]
fn funded_tip_admission_is_atomic_and_original_pool_bound() {
    use mv::BlockAcquisition as _;
    let initial_bytes =
        mv::cell::CellInitialization::<Option<NativeExecutionTip>>::allocation_layouts()
            .iter()
            .map(std::alloc::Layout::size)
            .sum::<usize>();
    let too_small = AllocationBudget::new(initial_bytes - 1);
    assert!(empty_cell(&too_small).is_err());
    assert_eq!(too_small.reserved_bytes(), 0);
    let budget = AllocationBudget::new(64 * 1024);
    let cell = empty_cell(&budget).unwrap();
    let before = budget.reserved_bytes();
    let successor = mv::cell::CellPublicationSuccessor::allocation_layout();
    let foreign = AllocationBudget::new(64 * 1024);
    let mut wrong_parent = foreign.try_reserve_layouts([successor]).unwrap();
    assert!(original_cell(&cell, &budget, &mut wrong_parent).is_err());
    assert_eq!(budget.reserved_bytes(), before);
    let mut parent = budget.try_reserve_layouts([successor]).unwrap();
    let mut original = original_cell(&cell, &budget, &mut parent).unwrap();
    assert_eq!(parent.remaining_bytes(), 0);
    original.initialize(mv::BlockMode::Ordinary);
    drop(original);
    assert_eq!(*cell.view().get(), None);
}

#[test]
fn snapshot_json_preserves_genesis_undo_absence_distinction() {
    let chain = chain();
    let view = chain.state().view();
    let claim = snapshot_claim(chain.state());
    assert!(claim.matches_original(view.native_execution_tip(), Some(None)));
    assert!(!claim.matches_original(view.native_execution_tip(), None));
    let empty = NativeExecutionTipSnapshot::from_original(None, None);
    let empty: NativeExecutionTipSnapshot =
        norito::json::from_str(&norito::json::to_json(&empty).unwrap()).unwrap();
    assert!(empty.matches_original(None, None));
    assert!(!empty.matches_original(None, Some(None)));
}
