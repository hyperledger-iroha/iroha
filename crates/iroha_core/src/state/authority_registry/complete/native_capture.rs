//! Scoped native capture with explicit original State allocation custody.
//!
//! Sharing State's execution pool is local resource policy, never proof of
//! authoritative publication. Capacity failure retains the exact pool refusal.

use super::*;

/// Capture the actual `world.domains` table without claiming State finality.
///
/// `None` means a coordinated State publication overlapped this local read.
/// Direct MV writes can bypass that generation, so this remains diagnostic.
pub(crate) fn capture_domains_table_once(
    state: &State,
    limits: LeafLimits,
) -> Result<Option<CanonicalTablePairedSnapshot>, LeafError> {
    let generation = state.state_view_generation();
    if generation & 1 != 0 {
        return Ok(None);
    }
    let budget = state.ivm_execution_budget();
    let domains = state.world.domains.view();
    let snapshot = CanonicalTableLeafSet::paired_table_from_rows(
        "world.domains",
        limits,
        &budget,
        domains.iter(),
    )?;
    drop(domains);
    if !is_stable_state_view_generation(generation, state.state_view_generation()) {
        return Ok(None);
    }
    Ok(Some(snapshot))
}

/// Capture the actual `world.accounts` table without claiming State finality.
///
/// `None` means a coordinated State publication overlapped this local read.
/// Direct MV writes can bypass that generation, so this remains diagnostic.
pub(crate) fn capture_accounts_table_once(
    state: &State,
    limits: LeafLimits,
) -> Result<Option<CanonicalTablePairedSnapshot>, LeafError> {
    let generation = state.state_view_generation();
    if generation & 1 != 0 {
        return Ok(None);
    }
    let budget = state.ivm_execution_budget();
    let accounts = state.world.accounts.view();
    let snapshot = CanonicalTableLeafSet::paired_table_from_rows(
        "world.accounts",
        limits,
        &budget,
        accounts.iter(),
    )?;
    drop(accounts);
    if !is_stable_state_view_generation(generation, state.state_view_generation()) {
        return Ok(None);
    }
    Ok(Some(snapshot))
}

/// Capture one actual authoritative World table from a stable State generation.
///
/// The returned pair authenticates only `world.account_aliases`. The caller
/// must still check every other authority, publish one complete root through
/// State/Kura, and retain its nodes before exposing any finalized witness.
/// `None` means a concurrent State publication won the generation race and
/// the caller must retry from a new view. This method never supplies a
/// finalized anchor or permission to admit private or remote transactions.
pub(crate) fn capture_account_alias_table_once(
    state: &State,
    limits: LeafLimits,
) -> Result<Option<CanonicalTablePairedSnapshot>, LeafError> {
    let generation = state.state_view_generation();
    if generation & 1 != 0 {
        return Ok(None);
    }
    let budget = state.ivm_execution_budget();
    let aliases = state.world.account_aliases.view();
    let snapshot = CanonicalTableLeafSet::paired_table_from_rows(
        "world.account_aliases",
        limits,
        &budget,
        aliases.iter(),
    )?;
    drop(aliases);
    if !is_stable_state_view_generation(generation, state.state_view_generation()) {
        return Ok(None);
    }
    Ok(Some(snapshot))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kura::Kura, query::store::LiveQueryStore, state::World};
    use iroha_allocation::{AllocationBudget, AllocationRefusal};
    use iroha_crypto::NoritoKeyRangeError;
    use std::{
        future::Future,
        pin::pin,
        task::{Context, Poll, Waker},
    };

    fn limits() -> LeafLimits {
        LeafLimits {
            max_tables: 1,
            max_rows: 4,
            max_payload_bytes: 4096,
            max_ordered_table_bytes: 4096,
            max_streamed_value_bytes: 8192,
        }
    }

    #[test]
    fn state_capture_defers_original_pool_refusal_without_losing_release_custody() {
        // Other tests can retain the process-global EBR epoch for arbitrary work.
        // The child keeps the original pin and exact refund assertions independent.
        if crate::unit_test_support::run_in_isolated_harness(
            "state::authority_registry::complete::native_capture::tests::state_capture_defers_original_pool_refusal_without_losing_release_custody",
        ) {
            return;
        }
        let state = State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let budget = state
            .pipeline_ivm_prepared_cache
            .read()
            .execution_budget()
            .clone();
        // Fresh State already owns the native execution tip's current/undo EBR
        // cells, publication identity and release controls in this same pool.
        let initial_bytes = mv::cell::CellInitialization::<
            Option<crate::state::NativeExecutionTip>,
        >::allocation_layouts()
        .iter()
        .map(std::alloc::Layout::size)
        .sum::<usize>();
        assert_eq!(budget.reserved_bytes(), initial_bytes);
        budget.set_limit_bytes(0);
        assert!(matches!(
            capture_accounts_table_once(&state, limits()),
            Err(LeafError::OrderedRange(NoritoKeyRangeError::Admission(
                AllocationRefusal::ExceedsLimit { .. }
            )))
        ));
        assert_eq!(budget.reserved_bytes(), initial_bytes);
        budget.set_limit_bytes(initial_bytes.checked_add(4096).unwrap());
        let occupied = budget.try_reserve_bytes(4096).unwrap();
        let Err(LeafError::OrderedRange(NoritoKeyRangeError::Admission(
            AllocationRefusal::Capacity { release, .. },
        ))) = capture_accounts_table_once(&state, limits())
        else {
            panic!("State capture must preserve the original allocation refusal")
        };
        let mut wait = pin!(release.wait_for_release());
        let mut context = Context::from_waker(Waker::noop());
        assert_eq!(wait.as_mut().poll(&mut context), Poll::Pending);
        let unrelated = AllocationBudget::new(1);
        drop(unrelated.try_reserve_bytes(1).unwrap());
        assert_eq!(wait.as_mut().poll(&mut context), Poll::Pending);
        drop(occupied);
        assert_eq!(wait.as_mut().poll(&mut context), Poll::Ready(()));
        let captured = capture_accounts_table_once(&state, limits())
            .unwrap()
            .unwrap();
        assert_eq!(captured.row_count(), 0);
        assert!(budget.reserved_bytes() > initial_bytes);
        let captured_bytes = budget.reserved_bytes() - initial_bytes;
        let root = captured.root();
        budget.set_limit_bytes(0);
        let retired_generations = crate::state::native_execution_tip::TipCell::allocation_layouts()
            .iter()
            .map(std::alloc::Layout::size)
            .sum::<usize>();
        let retirement_pin = crossbeam_epoch::pin();
        drop(state);
        assert_eq!(captured.root(), root);
        retirement_pin.flush();
        assert_eq!(
            budget.reserved_bytes(),
            captured_bytes + retired_generations,
            "the live epoch retains both original native-tip generations after State drops",
        );
        drop(retirement_pin);
        collect_original_ebr_until(&budget, captured_bytes);
        assert_eq!(budget.reserved_bytes(), captured_bytes);
        drop(captured);
        assert_eq!(budget.reserved_bytes(), 0);
    }
    fn collect_original_ebr_until(budget: &iroha_allocation::AllocationBudget, expected: usize) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while budget.reserved_bytes() != expected {
            assert!(
                std::time::Instant::now() < deadline,
                "original EBR custody {} != {expected}",
                budget.reserved_bytes(),
            );
            crossbeam_epoch::pin().flush();
            std::thread::yield_now();
        }
    }
}
