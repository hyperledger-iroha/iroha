//! Scoped native capture with explicit original State allocation custody.
//!
//! Sharing State's execution pool is local resource policy, never proof of
//! authoritative publication. Capacity failure retains the exact pool refusal.

use super::super::account_alias_ownership::{AliasOwnershipError, CheckedAccountAliases};
use super::super::account_identity_ownership::{CheckedAccountIdentities, IdentityOwnershipError};
use super::super::domain_ownership::{CheckedDomainOwnership, DomainOwnershipError};
use super::*;
use mv::PublicationPreparationError;

/// Capture the actual `world.domains` table without claiming State finality.
///
/// `None` means State or either native table changed during this local read.
/// Current and predecessor owner-index images are checked against the retained
/// canonical rows before encoding; native identity checks also detect direct MV
/// publication of either table. The complete State owner is still absent, so
/// this remains diagnostic and cannot establish finality.
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
    // Eight inspected rows/members per retained-row allowance is local capture
    // policy, not gas or validity. A larger prior image can require a larger
    // admitted bound even when the current table would fit the retention limit.
    let checked = CheckedDomainOwnership::capture(&state.world, limits.max_rows.saturating_mul(8));
    // A partially published World may expose individually stable maps that do
    // not yet belong to one State cut. Do not label that observation corruption.
    if !is_stable_state_view_generation(generation, state.state_view_generation()) {
        return Ok(None);
    }
    let domains = match checked {
        Ok(domains) => domains,
        Err(DomainOwnershipError::Publication(PublicationPreparationError::Changed)) => {
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
    };
    let snapshot = CanonicalTableLeafSet::paired_table_from_rows(
        "world.domains",
        limits,
        &budget,
        domains.domains().iter(),
    )?;
    if !domains.matches_current()? {
        return Ok(None);
    }
    drop(domains);
    if !is_stable_state_view_generation(generation, state.state_view_generation()) {
        return Ok(None);
    }
    Ok(Some(snapshot))
}

/// Capture the actual `world.accounts` table without claiming State finality.
///
/// Retain the original account and identity-index readers, checking their exact
/// current and predecessor relations before encoding the same account rows.
/// Every physical row and inspected opaque member consumes local work, bounded
/// to eight times the admitted row allowance. `None` means State or native
/// publication overlapped this read. Neither result supplies finalized authority.
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
    let checked =
        CheckedAccountIdentities::capture(&state.world, limits.max_rows.saturating_mul(8));
    if !is_stable_state_view_generation(generation, state.state_view_generation()) {
        return Ok(None);
    }
    let accounts = match checked {
        Ok(accounts) => accounts,
        Err(IdentityOwnershipError::Publication(PublicationPreparationError::Changed)) => {
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
    };
    let snapshot = CanonicalTableLeafSet::paired_table_from_rows(
        "world.accounts",
        limits,
        &budget,
        accounts.accounts().iter(),
    )?;
    if !accounts.matches_current()? {
        return Ok(None);
    }
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
/// Both current and predecessor aliases, account primary labels and reverse
/// membership are checked through retained original native readers. The local
/// work allowance is eight inspections per admitted table-row allowance;
/// accounts without aliases still consume work and can require a larger bound.
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
    let checked = CheckedAccountAliases::capture(&state.world, limits.max_rows.saturating_mul(8));
    if !is_stable_state_view_generation(generation, state.state_view_generation()) {
        return Ok(None);
    }
    let aliases = match checked {
        Ok(aliases) => aliases,
        Err(AliasOwnershipError::Publication(PublicationPreparationError::Changed)) => {
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
    };
    let snapshot = CanonicalTableLeafSet::paired_table_from_rows(
        "world.account_aliases",
        limits,
        &budget,
        aliases.aliases().iter(),
    )?;
    if !aliases.matches_current()? {
        return Ok(None);
    }
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
        // Fresh State owns the native tip's original generations and controls,
        // plus each separately funded physical reader/writer release source.
        let native_tip_bytes = mv::cell::CellInitialization::<
            Option<crate::state::NativeExecutionTip>,
        >::allocation_layouts()
        .iter()
        .map(std::alloc::Layout::size)
        .sum::<usize>();
        let lock_releases = [
            state.latest_block_header.observe_release(),
            state.da_commitments.observe_release(),
            state.da_confidential_compute.observe_release(),
            state.da_receipt_cursors.observe_release(),
            state.da_shard_cursors.observe_release(),
            state.da_pin_intents.observe_release(),
            state.lane_manifests.observe_release(),
            state.lane_privacy_registry.observe_release(),
            state.da_indexes_hydrated.observe_release(),
            state.pipeline_ivm_prepared_cache.observe_release(),
            state.nexus.observe_release(),
            state.crypto.observe_release(),
            state.kagemusha_v1_runtime_verifier.observe_release(),
            state.state_write_lock.observe_release(),
        ];
        let initial_bytes = native_tip_bytes
            + lock_releases.len()
                * iroha_allocation::release::ReleaseNotification::allocation_layout::<
                    iroha_allocation::AllocationCharge,
                >()
                .size();
        // These observations must not keep controls alive after State drops.
        drop(lock_releases);
        assert_eq!(budget.reserved_bytes(), initial_bytes);
        budget.set_limit_bytes(0);
        assert!(matches!(
            capture_accounts_table_once(&state, limits()),
            Err(LeafError::OrderedRange(NoritoKeyRangeError::Admission(
                AllocationRefusal::ExceedsLimit { .. }
            )))
        ));
        assert_eq!(budget.reserved_bytes(), initial_bytes);
        budget.set_limit_bytes(
            initial_bytes.checked_add(4096).unwrap()
                + iroha_allocation::release::ReleaseRegistration::allocation_layout().size(),
        );
        let mut registration = crate::unit_test_support::release_registration(&budget);
        let occupied = budget.try_reserve_bytes(4096).unwrap();
        let Err(LeafError::OrderedRange(NoritoKeyRangeError::Admission(
            AllocationRefusal::Capacity { release, .. },
        ))) = capture_accounts_table_once(&state, limits())
        else {
            panic!("State capture must preserve the original allocation refusal")
        };
        let mut wait = Box::pin(release.wait_for_release(&mut registration));
        let mut context = Context::from_waker(Waker::noop());
        assert_eq!(wait.as_mut().poll(&mut context), Poll::Pending);
        let unrelated = AllocationBudget::new(1);
        drop(unrelated.try_reserve_bytes(1).unwrap());
        assert_eq!(wait.as_mut().poll(&mut context), Poll::Pending);
        drop(occupied);
        assert_eq!(wait.as_mut().poll(&mut context), Poll::Ready(()));
        drop(wait);
        drop(registration);
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
