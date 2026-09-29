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
    let budget = state
        .pipeline_ivm_prepared_cache
        .read()
        .execution_budget()
        .clone();
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
    let budget = state
        .pipeline_ivm_prepared_cache
        .read()
        .execution_budget()
        .clone();
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
    let budget = state
        .pipeline_ivm_prepared_cache
        .read()
        .execution_budget()
        .clone();
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
    use iroha_crypto::NoritoKeyRangeError;
    use mv::allocation::{AllocationBudget, AllocationRefusal};
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
        assert_eq!(budget.reserved_bytes(), 0);
        budget.set_limit_bytes(0);
        assert!(matches!(
            capture_accounts_table_once(&state, limits()),
            Err(LeafError::OrderedRange(NoritoKeyRangeError::Admission(
                AllocationRefusal::ExceedsLimit { .. }
            )))
        ));
        budget.set_limit_bytes(4096);
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
        assert!(budget.reserved_bytes() > 0);
        let root = captured.root();
        budget.set_limit_bytes(0);
        drop(state);
        assert_eq!(captured.root(), root);
        assert!(budget.reserved_bytes() > 0);
        drop(captured);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}
