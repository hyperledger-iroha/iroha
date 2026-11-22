//! Bounded maintenance replaces an observation only after exact catalog and fresh native checks.
//! It never starts/stops children, renews initial custody, submits a transaction or extends a timer.

use super::{
    activation::{
        Budget, ReadinessExpiry, catalog, observe_native_set, validate_gateways, validate_live,
    },
    owned::OwnedGateway,
    progress::{Failure, Phase, Progress},
    renewal,
};
use crate::localnet::service_authorities::RetainedProviderServicePlan;
use crate::managed::{
    PreparedLocalnet, Result,
    build_registry::{DISCOVERY_FRESHNESS, GeneratedServiceObservation},
    gateway_compliance::{ManagedGatewayCompliance, PromotedGeneratedCatalog},
    native_operation::{ManagedTransactionFinality, invalid, now_ms},
};
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

const TURN_MAXIMUM: Duration = Duration::from_secs(15);
const RETRY_DELAY: Duration = Duration::from_secs(1);

#[derive(Clone, Copy)]
struct Interval {
    issued: u64,
    expires: u64,
}
impl Interval {
    fn new(issued: u64, expires: u64) -> Result<Self> {
        if issued >= expires || expires == u64::MAX {
            return Err(invalid("invalid signed maintenance interval"));
        }
        Ok(Self { issued, expires })
    }
    fn midpoint(self) -> u64 {
        // The difference is bounded by expires; adding it to issued cannot overflow.
        self.issued + (self.expires - self.issued) / 2
    }
}

#[derive(Clone)]
struct Selection {
    continuation: renewal::Continuation,
    terminal: ManagedTransactionFinality,
    catalog: PromotedGeneratedCatalog,
    advert_interval: Interval,
    catalog_interval: Interval,
}

/// A finite observation and its exact original selections. Neither a timer nor a transport ACK
/// can construct a replacement; native discovery remains the only constructor input below.
#[derive(Clone)]
pub(super) struct ProviderObservation {
    selection: Selection,
    timing: Timing,
}
#[derive(Clone, Copy)]
struct Timing {
    expiry: ReadinessExpiry,
    next_monotonic: Instant,
    next_utc: u64,
}
impl Timing {
    fn select(
        valid_until: u64,
        advert: Interval,
        catalog: Interval,
        observed_at: Instant,
        observed_utc: u64,
    ) -> Result<Self> {
        let freshness_ms = u64::try_from(DISCOVERY_FRESHNESS.as_millis())
            .map_err(|_| invalid("discovery freshness interval overflow"))?;
        let freshness_end = observed_utc
            .checked_add(freshness_ms)
            .ok_or_else(|| invalid("discovery freshness interval overflow"))?;
        let expiry =
            ReadinessExpiry::select(valid_until.min(freshness_end), observed_at, observed_utc)?;
        let valid_ms = expiry.utc_ceiling().saturating_sub(observed_utc);
        let next_utc = observed_utc
            .checked_add(valid_ms / 2)
            .ok_or_else(|| invalid("discovery refresh interval overflow"))?
            .min(advert.midpoint())
            .min(catalog.midpoint());
        let mut timing = Self {
            expiry,
            next_monotonic: observed_at,
            next_utc: observed_utc,
        };
        // A stale but still valid signed interval can request an immediate refresh. Bound it
        // by one retry delay to avoid a busy loop if a server keeps returning the same bytes.
        timing.schedule(next_utc, observed_at, observed_utc)?;
        Ok(timing)
    }
    // Within the original observation, allow one turn of the measured duration and a full
    // bounded retry. A previously fast read can race a newly advancing certified tip.
    fn reserve_refresh(
        &mut self,
        elapsed: Duration,
        observed_at: Instant,
        observed_utc: u64,
    ) -> Result<()> {
        let lead = elapsed
            .min(TURN_MAXIMUM)
            .saturating_add(RETRY_DELAY)
            .saturating_add(TURN_MAXIMUM);
        let lead = u64::try_from(lead.as_millis()).unwrap_or(u64::MAX);
        let latest_start = self.expiry.utc_ceiling().saturating_sub(lead);
        if latest_start < self.next_utc {
            self.schedule(latest_start, observed_at, observed_utc)?;
        }
        Ok(())
    }
    fn schedule(&mut self, requested: u64, now: Instant, utc: u64) -> Result<()> {
        let minimum = utc
            .checked_add(
                u64::try_from(RETRY_DELAY.as_millis())
                    .map_err(|_| invalid("maintenance retry interval overflow"))?,
            )
            .ok_or_else(|| invalid("maintenance retry interval overflow"))?;
        let next_utc = requested.max(minimum);
        let delay = Duration::from_millis(next_utc.saturating_sub(utc));
        // A pending retry may never postpone the original expiry or create a new lease.
        self.next_monotonic = now
            .checked_add(delay)
            .ok_or_else(|| invalid("maintenance schedule overflow"))?
            .min(self.expiry.deadline(Duration::MAX)?);
        self.next_utc = next_utc;
        Ok(())
    }
    /// A fresh discovery may replace discovery timing for every provider. An already-due
    /// signed advertisement or catalog still needs its own owner's turn.
    fn retain_due_publication(&mut self, previous: &Self, signed_midpoint: u64, observed_utc: u64) {
        if signed_midpoint <= observed_utc {
            self.next_monotonic = self.next_monotonic.min(previous.next_monotonic);
            self.next_utc = self.next_utc.min(previous.next_utc);
        }
    }
    fn current_at(&self, enrollment: ReadinessExpiry, now: Instant, utc: u64) -> bool {
        self.expiry.current_at(now, utc) && enrollment.current_at(now, utc)
    }
    fn due_at(&self, now: Instant, utc: u64) -> bool {
        now >= self.next_monotonic || utc >= self.next_utc
    }
}

/// Overdue observations keep their priority across another provider's turn. Either clock
/// can make a slot due. Ties retain the original provider order.
fn oldest_due_slot<'a>(
    timings: impl Iterator<Item = &'a Timing>,
    now: Instant,
    utc: u64,
) -> Option<usize> {
    timings
        .enumerate()
        .filter(|(_, timing)| timing.due_at(now, utc))
        .min_by_key(|(_, timing)| {
            std::cmp::Reverse(
                now.saturating_duration_since(timing.next_monotonic)
                    .max(Duration::from_millis(utc.saturating_sub(timing.next_utc))),
            )
        })
        .map(|(slot, _)| slot)
}
/// A failed native check defers only its own slot. Other observations retain their original
/// schedules even if they came due while the physical turn was running.
fn defer_failed_slot<'a>(
    mut timings: impl Iterator<Item = &'a mut Timing>,
    slot: usize,
    now: Instant,
    utc: u64,
) -> Result<()> {
    timings
        .nth(slot)
        .ok_or_else(|| invalid("unknown original provider slot"))?
        .schedule(utc, now, utc)
}

impl ProviderObservation {
    pub(super) fn new(
        continuation: renewal::Continuation,
        terminal: ManagedTransactionFinality,
        catalog: PromotedGeneratedCatalog,
        current: GeneratedServiceObservation,
        observed_at: Instant,
        observed_utc: u64,
        elapsed: Duration,
    ) -> Result<Self> {
        let (issued, expires) = current.advert_interval();
        let advert_interval = Interval::new(issued, expires)?;
        let catalog_interval = Interval::new(
            catalog
                .generated_at_unix
                .checked_mul(1_000)
                .ok_or_else(|| invalid("catalog interval overflow"))?,
            catalog
                .valid_until_unix
                .checked_mul(1_000)
                .ok_or_else(|| invalid("catalog interval overflow"))?,
        )?;
        let mut timing = Timing::select(
            current.valid_until_unix_ms().min(catalog_interval.expires),
            advert_interval,
            catalog_interval,
            observed_at,
            observed_utc,
        )?;
        timing.reserve_refresh(elapsed, observed_at, observed_utc)?;
        Ok(Self {
            selection: Selection {
                continuation,
                terminal,
                catalog,
                advert_interval,
                catalog_interval,
            },
            timing,
        })
    }
    fn begin(&self, cancelled: Arc<AtomicBool>) -> Result<Budget> {
        let started = Instant::now();
        let deadline = self.timing.expiry.deadline(TURN_MAXIMUM)?;
        let timeout = deadline
            .checked_duration_since(started)
            .filter(|value| !value.is_zero())
            .ok_or_else(|| invalid("maintenance observation expired"))?;
        Ok(Budget {
            started,
            timeout,
            startup_deadline_ns: None,
            utc_ceiling_unix_ms: Some(self.timing.expiry.utc_ceiling()),
            cancelled,
            progress: Arc::new(Progress::default()),
        })
    }
}

/// Only the selected original may publish its advertisement or advance its catalog.
fn update_selected(
    prepared: &PreparedLocalnet,
    selected: &Selection,
    live: &mut OwnedGateway,
    budget: &Budget,
) -> std::result::Result<Option<PromotedGeneratedCatalog>, Failure> {
    let now = budget.call(|_| now_ms())?;
    if now >= selected.advert_interval.midpoint() {
        budget.progress.enter(Phase::ProviderAdvertisement);
        validate_live(prepared, live, budget)?;
        budget.call(|deadline| live.open_provider_advert(prepared)?.publish(deadline))?;
        validate_live(prepared, live, budget)?;
    }
    if now >= selected.catalog_interval.midpoint() {
        budget.progress.enter(Phase::Catalog);
        return budget
            .call(|deadline| {
                ManagedGatewayCompliance::open(prepared, live.provider())?.advance(live, deadline)
            })
            .map(Some);
    }
    Ok(None)
}

/// All three finite observations, in the original profile's validated provider-slot order.
#[derive(Clone)]
pub(super) struct Observation {
    providers: [ProviderObservation; 3],
}

pub(super) struct RefreshSelection {
    original: Observation,
    slot: usize,
}
impl Observation {
    /// Combine observations only after the caller freshly selects the original provider order.
    /// The plans carry no current provider eligibility or replacement observation clocks.
    pub(super) fn new(
        plans: &[RetainedProviderServicePlan; 3],
        providers: [ProviderObservation; 3],
    ) -> Result<Self> {
        for (plan, observed) in plans.iter().zip(&providers) {
            if plan.provider_id() != observed.selection.continuation.provider() {
                return Err(invalid(
                    "aggregate observation changed original provider order",
                ));
            }
        }
        let value = Self { providers };
        if !value.current()? {
            return Err(invalid("aggregate provider observation expired"));
        }
        Ok(value)
    }
    pub(super) fn current(&self) -> Result<bool> {
        let mono = Instant::now();
        let utc = now_ms()?;
        Ok(self.providers.iter().all(|value| {
            value
                .timing
                .current_at(value.selection.continuation.expiry(), mono, utc)
        }))
    }
    pub(super) fn due_slot(&self) -> Result<Option<usize>> {
        Ok(oldest_due_slot(
            self.providers.iter().map(|value| &value.timing),
            Instant::now(),
            now_ms()?,
        ))
    }
    pub(super) fn begin(
        &self,
        slot: usize,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(RefreshSelection, Budget)> {
        if !self.current()? {
            return Err(invalid("aggregate refresh observation expired"));
        }
        let mut budget = self.providers[slot].begin(cancelled)?;
        budget.cap_to_expiries(
            self.providers
                .iter()
                .flat_map(|value| [value.timing.expiry, value.selection.continuation.expiry()]),
            TURN_MAXIMUM,
        )?;
        Ok((
            RefreshSelection {
                original: self.clone(),
                slot,
            },
            budget,
        ))
    }
    pub(super) fn retry(&mut self, slot: usize) -> Result<()> {
        let utc = now_ms()?;
        defer_failed_slot(
            self.providers.iter_mut().map(|value| &mut value.timing),
            slot,
            Instant::now(),
            utc,
        )?;
        if !self.current()? {
            return Err(invalid("aggregate retry observation expired"));
        }
        Ok(())
    }
    pub(super) fn renewal(
        &self,
        cancelled: &Arc<AtomicBool>,
        progress: &Arc<Progress>,
    ) -> Result<Option<renewal::Turn>> {
        if !self.current()? {
            return Err(invalid("aggregate renewal observation expired"));
        }
        let continuations =
            std::array::from_fn(|index| self.providers[index].selection.continuation.clone());
        let catalogs = std::array::from_fn(|index| self.providers[index].selection.catalog);
        for value in &self.providers {
            if let Some(turn) = value.selection.continuation.begin(
                value.selection.terminal,
                Arc::clone(cancelled),
                Arc::clone(progress),
                continuations.clone(),
                catalogs,
            )? {
                return Ok(Some(turn));
            }
        }
        Ok(None)
    }
}

/// One bounded physical turn refreshes each provider only after all three independently
/// authenticate their exact catalogs, native rows and custody at the fresh Global cut.
pub(super) fn refresh(
    prepared: &PreparedLocalnet,
    selected: RefreshSelection,
    mut live: [OwnedGateway; 3],
    budget: &Budget,
) -> std::result::Result<Observation, Failure> {
    let RefreshSelection { original, slot } = selected;
    budget.progress.enter(Phase::GatewayValidation);
    budget.check()?;
    for (gateway, provider) in live.iter().zip(&original.providers) {
        if gateway.provider() != provider.selection.continuation.provider() {
            return Err(budget.progress.unconfirmed());
        }
    }
    validate_gateways(prepared, &mut live, budget)?;
    let written = update_selected(
        prepared,
        &original.providers[slot].selection,
        &mut live[slot],
        budget,
    )?;
    budget.progress.enter(Phase::PromotedCatalog);
    let catalogs = original
        .providers
        .each_ref()
        .map(|provider| provider.selection.catalog);
    let promoted = catalog::observe(prepared, &mut live, catalogs, budget)?;
    // The old lease supplies every anchor, including the writer's. A durably committed
    // catalog from a failed earlier turn is recoverable. Any effect just returned by this
    // turn must also agree exactly with the independently authenticated serving head.
    budget.call(|_| promoted[slot].require_writer_receipt(written))?;
    let floors = original
        .providers
        .each_ref()
        .map(|provider| provider.selection.terminal);
    let (currents, observed_at, observed_utc) =
        observe_native_set(prepared, &live, floors, budget)?;
    budget.progress.enter(Phase::GatewayValidation);
    validate_gateways(prepared, &mut live, budget)?;
    drop(live);
    budget.progress.enter(Phase::Selection);
    // The old aggregate remains current without interruption even after all fresh rows return.
    if !original.current().unwrap_or(false) {
        return Err(Failure::ObservationExpired);
    }
    budget.call(|_| {
        let replace = |index: usize| {
            let old = &original.providers[index].selection;
            let mut next = ProviderObservation::new(
                old.continuation.clone(),
                old.terminal,
                promoted[index],
                currents[index],
                observed_at,
                observed_utc,
                budget.started.elapsed(),
            )?;
            if index != slot {
                let midpoint = next
                    .selection
                    .advert_interval
                    .midpoint()
                    .min(next.selection.catalog_interval.midpoint());
                next.timing.retain_due_publication(
                    &original.providers[index].timing,
                    midpoint,
                    observed_utc,
                );
            }
            Ok::<_, crate::managed::Error>(next)
        };
        let providers = [replace(0)?, replace(1)?, replace(2)?];
        // Release all owned gateway references before the original standalone profile capture.
        let plans = prepared
            .provider_service_plans()?
            .ok_or_else(|| invalid("original provider plans absent"))?;
        Observation::new(&plans, providers)
    })
}

#[cfg(test)]
mod tests;
