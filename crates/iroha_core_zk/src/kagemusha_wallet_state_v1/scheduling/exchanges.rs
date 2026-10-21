//! Requested exchange lifetimes in the shared scheduler, independent of proof priority.
//!
//! A lease is registered before caller-side queues or UI work. Ending it fences new calls
//! immediately; already admitted children retain occupancy until they finish. These
//! process-local capabilities grant neither monetary authority nor Archive dispatch permission.
//! TODO: connect this lifetime to the final Native Archive gate and mandatory bridge routes.

use std::collections::BTreeMap;

use super::{Arc, Scheduler};

const MAX_EXCHANGES: usize = 64;

#[derive(Default)]
struct Exchange {
    ending: bool,
    calls: usize,
}

#[derive(Default)]
pub(super) struct Exchanges {
    next: u64,
    live: BTreeMap<u64, Exchange>,
}

impl Exchanges {
    pub(super) fn is_empty(&self) -> bool {
        self.live.is_empty()
    }

    pub(super) fn close(&mut self) {
        // Only primitive counters live here. Closing never joins user-held capabilities.
        self.live.clear();
    }
}

/// Local exchange-control failure; it is not a proof or monetary rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ExchangeErrorV1 {
    /// The exact scheduler has closed; reopening creates a different owner.
    #[error("exchange owner closed")]
    Closed,
    /// End was requested for this lease; it cannot admit more executable calls.
    #[error("exchange ended")]
    Ended,
    /// A capability belongs to a different scheduler.
    #[error("foreign exchange owner")]
    Foreign,
    /// Live exchange or accepted-call accounting has reached its bound.
    #[error("exchange capacity exhausted")]
    Capacity,
    /// This scheduler has exhausted its nonreused local identities.
    #[error("exchange identities exhausted")]
    Exhausted,
}

/// Result of ending the exact exchange, without waiting for accepted children.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExchangeEndV1 {
    /// No accepted children remain; the occupancy was released by this call.
    Ended,
    /// New children are fenced, but accepted children still retain occupancy.
    Ending,
    /// This same capability's occupancy was already released.
    AlreadyEnded,
}

/// One non-cloneable logical exchange registered synchronously before caller-side waits.
///
/// Keep this value through Request export, confirmation and carrier teardown. Dropping it
/// requests end; children already accepted by [`Self::enter_call`] keep their occupancy.
/// It is never serialized and its identity cannot be constructed from caller DATA.
pub struct ExchangeLeaseV1 {
    scheduler: Scheduler,
    id: u64,
}

/// One accepted call, kept alive through queueing and execution after its lease ends.
///
/// This value does not hold Payment priority, prevent folding or prove mutation readiness.
/// A mutation must separately satisfy the current Native maintenance and operation checks.
pub struct ExchangeCallV1 {
    scheduler: Scheduler,
    id: u64,
}

impl Scheduler {
    /// Register requested exchange occupancy without waiting for wallet, proof or UI work.
    /// No Payment priority is taken and useful folding is left running.
    ///
    /// # Errors
    /// Closed owner, 64 live/ending exchanges, or exhausted nonreused local identities.
    pub fn begin_exchange(&self) -> Result<ExchangeLeaseV1, ExchangeErrorV1> {
        let mut state = self.0.lock();
        if state.closed {
            return Err(ExchangeErrorV1::Closed);
        }
        let exchanges = &mut state.exchanges;
        if exchanges.live.len() >= MAX_EXCHANGES {
            return Err(ExchangeErrorV1::Capacity);
        }
        let id = exchanges
            .next
            .checked_add(1)
            .ok_or(ExchangeErrorV1::Exhausted)?;
        exchanges.live.insert(id, Exchange::default());
        exchanges.next = id;
        drop(state);
        self.0.idle.notify_all();
        Ok(ExchangeLeaseV1 {
            scheduler: self.clone(),
            id,
        })
    }
}

impl ExchangeLeaseV1 {
    /// Admit a child synchronously before its executor or worker queue is entered.
    ///
    /// # Errors
    /// Closed owner, an ended/ending lease, or accepted-call counter exhaustion.
    pub fn enter_call(&self) -> Result<ExchangeCallV1, ExchangeErrorV1> {
        let mut state = self.scheduler.0.lock();
        if state.closed {
            return Err(ExchangeErrorV1::Closed);
        }
        let exchange = state
            .exchanges
            .live
            .get_mut(&self.id)
            .ok_or(ExchangeErrorV1::Ended)?;
        if exchange.ending {
            return Err(ExchangeErrorV1::Ended);
        }
        exchange.calls = exchange
            .calls
            .checked_add(1)
            .ok_or(ExchangeErrorV1::Capacity)?;
        Ok(ExchangeCallV1 {
            scheduler: self.scheduler.clone(),
            id: self.id,
        })
    }

    /// Fence new executable children and release occupancy once accepted children finish.
    /// Repeated calls with this actual capability are harmless; no identity is reused.
    ///
    /// # Errors
    /// The exact scheduler has closed. This never ends an exchange on a reopened owner.
    pub fn end(&self) -> Result<ExchangeEndV1, ExchangeErrorV1> {
        let mut state = self.scheduler.0.lock();
        if state.closed {
            return Err(ExchangeErrorV1::Closed);
        }
        let Some(exchange) = state.exchanges.live.get_mut(&self.id) else {
            return Ok(ExchangeEndV1::AlreadyEnded);
        };
        exchange.ending = true;
        let result = if exchange.calls == 0 {
            state.exchanges.live.remove(&self.id);
            ExchangeEndV1::Ended
        } else {
            ExchangeEndV1::Ending
        };
        drop(state);
        self.scheduler.0.idle.notify_all();
        Ok(result)
    }
}

impl ExchangeCallV1 {
    /// Check the exact live owner before using this already accepted call.
    /// An end request leaves admitted children valid; scheduler close invalidates them.
    /// This check is not a reservation for any subsequent operation.
    ///
    /// # Errors
    /// Foreign scheduler, closed owner, or missing accepted-call ownership.
    pub fn validate_for(&self, scheduler: &Scheduler) -> Result<(), ExchangeErrorV1> {
        if !Arc::ptr_eq(&self.scheduler.0, &scheduler.0) {
            return Err(ExchangeErrorV1::Foreign);
        }
        let state = scheduler.0.lock();
        if state.closed {
            return Err(ExchangeErrorV1::Closed);
        }
        match state.exchanges.live.get(&self.id) {
            Some(exchange) if exchange.calls != 0 => Ok(()),
            _ => Err(ExchangeErrorV1::Ended),
        }
    }
}

impl Drop for ExchangeLeaseV1 {
    fn drop(&mut self) {
        let _ = self.end();
    }
}

impl Drop for ExchangeCallV1 {
    fn drop(&mut self) {
        let mut state = self.scheduler.0.lock();
        if let Some(exchange) = state.exchanges.live.get_mut(&self.id) {
            // A call can only be constructed after this exact increment; it is not Clone.
            exchange.calls -= 1;
            if exchange.ending && exchange.calls == 0 {
                state.exchanges.live.remove(&self.id);
            }
        }
        drop(state);
        self.scheduler.0.idle.notify_all();
    }
}

#[cfg(test)]
mod tests;
