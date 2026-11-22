//! Joined fresh catalog promotion with serialized full renderer validation between HTTP calls.

use super::*;
use crate::localnet::service_authorities::RetainedGatewayCompliancePlan;
use std::sync::{Mutex, MutexGuard, TryLockError, atomic::AtomicU64};

pub(super) fn promote(
    prepared: &PreparedLocalnet,
    live: &mut [OwnedGateway; 3],
    budget: &Budget,
    parallel: bool,
) -> std::result::Result<[PromotedGeneratedCatalog; 3], Failure> {
    budget.check()?;
    // Every full live check reopens the same original Bootstrap and custody selections. Keep
    // that exact recipe exclusive within this round, but hold no guard during catalog HTTP.
    let validation = Mutex::new(());
    let round = (parallel
        && !norito::core::decode_limits_active()
        && live[0].read_renderer().is_some()
        && live.iter().all(|gateway| gateway.same_launch(&live[0])))
    .then(LaunchRound::default);
    let result = crate::managed::provider_round::run(
        live.each_mut(),
        parallel,
        |gateway| {
            let publisher = budget.call(|_| gateway.open_catalog_writer(prepared))?;
            let mut gateway = PublishingGateway {
                original: GuardedGateway {
                    gateway,
                    validation: &validation,
                    budget,
                },
                round: round.as_ref(),
            };
            loop {
                gateway.validate_original(prepared)?;
                let result = publisher.advance(&mut gateway, budget.deadline()?);
                budget.check()?;
                gateway.validate_original(prepared)?;
                if let Ok(catalog) = result {
                    return Ok(catalog);
                }
                budget.wait()?;
            }
        },
        || budget.progress.unconfirmed(),
    );
    // All publishers, live borrows and workers have closed on both success and failure.
    budget.check()?;
    result
}

/// Verify all three retained promoted catalogs after the selected owner's effects finish.
/// Each HTTP call uses that provider's own live child and original catalog authority. The shared launch is
/// freshly validated on each side of the joined reads; signing and publication stay separate.
pub(in crate::managed::runtime) fn observe(
    prepared: &PreparedLocalnet,
    live: &mut [OwnedGateway; 3],
    expected: [PromotedGeneratedCatalog; 3],
    budget: &Budget,
    mut reads: Option<&mut CatalogLaunchReads<'_>>,
) -> std::result::Result<[PromotedGeneratedCatalog; 3], Failure> {
    budget.check()?;
    let Some(renderer) = live[0].read_renderer() else {
        return observe_independently(prepared, live, expected, budget);
    };
    let owners = crate::managed::provider_round::run(
        live.each_ref(),
        true,
        |gateway| budget.call(|_| renderer.open_catalog_reader(prepared, gateway.provider())),
        || budget.progress.unconfirmed(),
    )?;
    budget.call(|deadline| {
        ManagedGatewayCompliance::recover_promoted_set(&owners, expected, deadline, |plans| {
            budget
                .check()
                .map_err(|f| crate::managed::Error::Invalid(f.message()))?;
            let result =
                OwnedGateway::validate_catalog_read(prepared, live, plans, reads.as_deref_mut());
            budget
                .check()
                .map_err(|f| crate::managed::Error::Invalid(f.message()))?;
            result
        })
    })
}

fn observe_independently(
    prepared: &PreparedLocalnet,
    live: &mut [OwnedGateway; 3],
    expected: [PromotedGeneratedCatalog; 3],
    budget: &Budget,
) -> std::result::Result<[PromotedGeneratedCatalog; 3], Failure> {
    budget.check()?;
    let validation = Mutex::new(());
    let [first, second, third] = live;
    let [first_catalog, second_catalog, third_catalog] = expected;
    let result = crate::managed::provider_round::run(
        [
            (first, first_catalog),
            (second, second_catalog),
            (third, third_catalog),
        ],
        true,
        |(gateway, catalog)| {
            let publisher =
                budget.call(|_| ManagedGatewayCompliance::open(prepared, gateway.provider()))?;
            let mut gateway = GuardedGateway {
                gateway,
                validation: &validation,
                budget,
            };
            budget.call(|deadline| publisher.recover_promoted(&mut gateway, catalog, deadline))
        },
        || budget.progress.unconfirmed(),
    );
    budget.check()?;
    result
}

struct GuardedGateway<'a, G> {
    gateway: &'a mut G,
    validation: &'a Mutex<()>,
    budget: &'a Budget,
}
impl<G: LiveGatewayProcess> LiveGatewayProcess for GuardedGateway<'_, G> {
    fn validate(
        &mut self,
        prepared: &PreparedLocalnet,
        plan: &RetainedGatewayCompliancePlan,
    ) -> Result<()> {
        let _guard = acquire_validation(self.validation, self.budget)?;
        let result = self.gateway.validate(prepared, plan);
        // Cancellation or expiry during the complete native renderer check wins even over
        // an ordinary validation error. Never retain the mutex across the next HTTP effect.
        self.budget
            .check()
            .map_err(|failure| crate::managed::Error::Invalid(failure.message()))?;
        result
    }
}
/// Only requests already waiting before a successful native read begins can share it.
/// Neither a completed read nor a read already in flight admits a later request.
#[derive(Default)]
struct LaunchRound {
    requested: AtomicU64,
    completed: Mutex<u64>,
}
impl LaunchRound {
    fn validate(&self, budget: &Budget, read: impl FnOnce() -> Result<()>) -> Result<()> {
        let ticket = self
            .requested
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
            .map_err(|_| invalid("catalog validation request count overflowed"))?
            + 1;
        let mut completed = acquire_validation(&self.completed, budget)?;
        if *completed < ticket {
            let pending = self.requested.load(Ordering::Acquire);
            let result = read();
            budget
                .check()
                .map_err(|failure| crate::managed::Error::Invalid(failure.message()))?;
            result?;
            *completed = pending;
        }
        Ok(())
    }
}

struct PublishingGateway<'a> {
    original: GuardedGateway<'a, OwnedGateway>,
    round: Option<&'a LaunchRound>,
}

impl LiveGatewayProcess for PublishingGateway<'_> {
    fn validate(
        &mut self,
        prepared: &PreparedLocalnet,
        plan: &RetainedGatewayCompliancePlan,
    ) -> Result<()> {
        let Some(round) = self.round else {
            return self.original.validate(prepared, plan);
        };
        let gateway = &*self.original.gateway;
        let budget = self.original.budget;
        budget
            .check()
            .map_err(|failure| crate::managed::Error::Invalid(failure.message()))?;
        let result = gateway.validate_with_launch(prepared, plan, || {
            round.validate(budget, || gateway.validate_launch())
        });
        budget
            .check()
            .map_err(|failure| crate::managed::Error::Invalid(failure.message()))?;
        result
    }
}

impl PublishingGateway<'_> {
    fn validate_original(
        &mut self,
        prepared: &PreparedLocalnet,
    ) -> std::result::Result<(), Failure> {
        let budget = self.original.budget;
        let plan = budget.call(|_| {
            self.original
                .gateway
                .original_gateway_compliance_plan(prepared)?
                .ok_or_else(|| invalid("generated gateway plan absent"))
        })?;
        budget.call(|_| self.validate(prepared, &plan))
    }
}

fn acquire_validation<'a, T>(
    validation: &'a Mutex<T>,
    budget: &Budget,
) -> Result<MutexGuard<'a, T>> {
    loop {
        budget
            .check()
            .map_err(|failure| crate::managed::Error::Invalid(failure.message()))?;
        match validation.try_lock() {
            Ok(guard) => {
                budget
                    .check()
                    .map_err(|failure| crate::managed::Error::Invalid(failure.message()))?;
                return Ok(guard);
            }
            Err(TryLockError::WouldBlock) => {
                // The existing poll and unchanged outer timers bound contention. This is
                // scheduling only; no failed native check is turned into an admission.
                budget
                    .wait()
                    .map_err(|failure| crate::managed::Error::Invalid(failure.message()))?;
            }
            Err(TryLockError::Poisoned(_)) => {
                return Err(invalid("catalog validation worker panicked"));
            }
        }
    }
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
