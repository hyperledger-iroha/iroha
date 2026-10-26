//! Numeric exchange capabilities for the exact Core scheduler, without host admission policy.
//!
//! Controls take only the registry, capability table and Core's short control lock, never
//! wallet ownership or Payment priority. Core owns all exchange/child occupancy.

use std::collections::VecDeque;

use super::*;

const MAX_LIVE: usize = 64;
const MAX_ENDED: usize = 64;

struct Entry {
    lease: state::ExchangeLeaseV1,
    // Cleanup bookkeeping only; Core decides whether any accepted children remain.
    ending: bool,
}

#[derive(Default)]
pub(super) struct Tokens {
    live: BTreeMap<u64, Entry>,
    ended: VecDeque<u64>,
}

fn failure(error: state::ExchangeErrorV1) -> Failure {
    use state::ExchangeErrorV1 as E;
    Failure::code(match error {
        E::Closed | E::Ended => CLOSED,
        E::Foreign => INVALID,
        E::Capacity | E::Exhausted => RESOURCE,
    })
}

impl Tokens {
    #[cfg(test)]
    pub(super) fn first_live(&self) -> Option<u64> {
        self.live
            .iter()
            .find_map(|(id, entry)| (!entry.ending).then_some(*id))
    }
    fn begin(&mut self, scheduler: &state::Scheduler) -> Result<u64> {
        if self.live.len() >= MAX_LIVE {
            return Err(Failure::code(RESOURCE));
        }
        let token = review::allocate_token()?;
        let lease = scheduler.begin_exchange().map_err(failure)?;
        self.live.insert(
            token,
            Entry {
                lease,
                ending: false,
            },
        );
        Ok(token)
    }

    fn enter(&self, token: u64) -> Result<state::ExchangeCallV1> {
        let entry = self.live.get(&token).ok_or_else(|| {
            Failure::code(if self.ended.contains(&token) {
                CLOSED
            } else {
                INVALID
            })
        })?;
        entry.lease.enter_call().map_err(failure)
    }

    fn end(&mut self, token: u64) -> Result<state::ExchangeEndV1> {
        let Some(entry) = self.live.get_mut(&token) else {
            return if self.ended.contains(&token) {
                Ok(state::ExchangeEndV1::AlreadyEnded)
            } else {
                Err(Failure::code(INVALID))
            };
        };
        let result = entry.lease.end().map_err(failure)?;
        match result {
            state::ExchangeEndV1::Ending => entry.ending = true,
            state::ExchangeEndV1::Ended | state::ExchangeEndV1::AlreadyEnded => {
                self.retire(token);
            }
        }
        Ok(result)
    }

    fn retire(&mut self, token: u64) {
        self.live.remove(&token);
        if self.ended.len() == MAX_ENDED {
            self.ended.pop_front();
        }
        self.ended.push_back(token);
    }

    fn after_call(&mut self, token: u64) {
        if self.live.get(&token).is_some_and(|entry| entry.ending) {
            // The Core child has already dropped. Its own state decides whether this was
            // the last child; a closed scheduler needs no foreign tombstone publication.
            let _ = self.end(token);
        }
    }
}

/// A newly issued numeric identity, retained until its control response is delivered.
pub(crate) struct Issued {
    owner: Arc<Owner>,
    token: u64,
    retained: bool,
}

impl Issued {
    /// Allocation-free projection for an undelivered control response.
    pub(crate) fn response(&self) -> Response {
        Response {
            kind: 60,
            sequence: u128::from(self.token),
            ..Response::default()
        }
    }
    /// Complete an allocation-free control handoff. No Native queue locator is exposed.
    pub(crate) fn delivered(mut self) -> u64 {
        self.retained = false;
        self.token
    }
}

impl Drop for Issued {
    fn drop(&mut self) {
        if self.retained {
            // Cleanup may recover the table lock only to retire this undisclosed capability.
            // Dropping its Core lease fences fresh children; admitted children remain owned.
            let removed = self
                .owner
                .exchanges
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .live
                .remove(&self.token);
            // Release the real Core lease and table lock before discovery can run.
            drop(removed);
            self.owner.background.notify();
        }
    }
}

/// Retain a new exchange only after both the enclosing fence and foreign handoff succeed.
pub(crate) fn deliver_issued<T>(
    issued: Option<Issued>,
    result: Result<Response>,
    emit: impl FnOnce(Result<Response>) -> (T, bool),
) -> T {
    let succeeded = result.is_ok();
    let (value, delivered) = emit(result);
    if succeeded
        && delivered
        && let Some(issued) = issued
    {
        issued.delivered();
    }
    value
}

/// One actual accepted Core child, retained through queueing, execution and response delivery.
pub(crate) struct Call {
    owner: Arc<Owner>,
    token: u64,
    admitted: Option<state::ExchangeCallV1>,
}

/// One closed route's exact owner, retained until the foreign response has been delivered.
pub(crate) struct Invocation {
    owner: Arc<Owner>,
    exchange_id: u64,
    child: Option<Call>,
}
pub(super) fn setup_exempt(selector: u32) -> bool {
    matches!(selector, 8..=10 | 12..=14 | 16..=18 | 20..=22 | 24 | 26 | 29 | 33 | 37 | 38 | 40
        | 42..=45 | 47 | 50 | 52 | 55 | 57 | 58)
}
impl Invocation {
    /// Every executable exchange route admits its actual Core child before owner waits.
    pub(crate) fn admit(id: u64, exchange_id: u64) -> Result<Self> {
        let child = enter(id, exchange_id)?;
        Ok(Self {
            owner: child.owner(),
            exchange_id,
            child: Some(child),
        })
    }
    /// Cleanup checks its genuine capability's origin under wallet ownership. It remains
    /// callable after end, even after that exchange's bounded tombstone has been evicted.
    pub(crate) fn cleanup(id: u64, exchange_id: u64) -> Result<Self> {
        if exchange_id == 0 {
            return Err(Failure::code(INVALID));
        }
        Ok(Self {
            owner: owner(id)?,
            exchange_id,
            child: None,
        })
    }
    /// Zero is accepted only for this explicit closed set of observation/recovery routes.
    pub(crate) fn setup(id: u64, exchange_id: u64, selector: u32) -> Result<Self> {
        setup::bounds(selector)?;
        if matches!(selector, 6 | 51) {
            return Self::cleanup(id, exchange_id);
        }
        if setup_exempt(selector) {
            if exchange_id != 0 {
                return Err(Failure::code(INVALID));
            }
            return Ok(Self {
                owner: owner(id)?,
                exchange_id,
                child: None,
            });
        }
        Self::admit(id, exchange_id)
    }
    pub(super) fn owner(&self) -> Arc<Owner> {
        Arc::clone(&self.owner)
    }
    pub(super) fn origin(&self) -> u64 {
        self.exchange_id
    }
    pub(super) fn validate(&self) -> Result<()> {
        match &self.child {
            Some(child) => child.validate_for(&self.owner),
            None => self.owner.closing.require_open(),
        }
    }
}

impl Call {
    /// The exact registered owner captured before wallet or Payment waits.
    pub(super) fn owner(&self) -> Arc<Owner> {
        Arc::clone(&self.owner)
    }

    /// End does not invalidate admitted children; close or a foreign owner does.
    /// This is capability validation, not permission to mutate a head or dispatch Archive.
    pub(super) fn validate_for(&self, owner: &Arc<Owner>) -> Result<()> {
        if !Arc::ptr_eq(&self.owner, owner) {
            return Err(Failure::code(INVALID));
        }
        owner.closing.require_open()?;
        self.admitted
            .as_ref()
            .ok_or(Failure::code(CLOSED))?
            .validate_for(&owner.scheduler)
            .map_err(failure)
    }
}

impl Drop for Call {
    fn drop(&mut self) {
        // Never acquire the table while holding Core's state lock. Core releases actual
        // occupancy first; the table only retires an ending foreign identity afterward.
        drop(self.admitted.take());
        self.owner
            .exchanges
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .after_call(self.token);
        // End may have returned Ending; the last actual child is the release event.
        self.owner.background.notify();
    }
}

fn control<T>(
    registry: &Mutex<Registry>,
    id: u64,
    action: impl FnOnce(Arc<Owner>) -> Result<T>,
) -> Result<T> {
    // Closing marks this same generation while holding this registry lock. No control
    // can register a new child between that mark and Scheduler::close.
    let selected = registry.lock().map_err(|_| Failure::code(INTERNAL))?;
    let owner = selected
        .owners
        .get(&id)
        .cloned()
        .ok_or(Failure::code(CLOSED))?;
    owner.closing.require_open()?;
    action(owner)
}

fn begin_with(registry: &Mutex<Registry>, id: u64) -> Result<Issued> {
    control(registry, id, |owner| {
        let token = owner
            .exchanges
            .lock()
            .map_err(|_| Failure::code(INTERNAL))?
            .begin(&owner.scheduler)?;
        Ok(Issued {
            owner,
            token,
            retained: true,
        })
    })
}

/// Register the requested exchange synchronously before any wallet, worker or UI wait.
pub(crate) fn begin(id: u64) -> Result<Issued> {
    begin_with(registry(), id)
}

fn enter_with(registry: &Mutex<Registry>, id: u64, token: u64) -> Result<Call> {
    control(registry, id, |owner| {
        let admitted = owner
            .exchanges
            .lock()
            .map_err(|_| Failure::code(INTERNAL))?
            .enter(token)?;
        Ok(Call {
            owner,
            token,
            admitted: Some(admitted),
        })
    })
}

/// Admit a child before wallet/Payment waits; keep it through the whole accepted call.
pub(crate) fn enter(id: u64, token: u64) -> Result<Call> {
    enter_with(registry(), id, token)
}

fn end_with(registry: &Mutex<Registry>, id: u64, token: u64) -> Result<state::ExchangeEndV1> {
    let (owner, result) = control(registry, id, |owner| {
        let result = owner
            .exchanges
            .lock()
            .map_err(|_| Failure::code(INTERNAL))?
            .end(token)?;
        Ok((owner, result))
    })?;
    // The registry, table and Core control locks are all released before notification.
    owner.background.notify();
    Ok(result)
}

/// Fence new children without joining UI holders or invalidating already accepted calls.
pub(crate) fn end(id: u64, token: u64) -> Result<state::ExchangeEndV1> {
    end_with(registry(), id, token)
}

/// Closed result geometry shared by C and JNI; no financial completion is implied.
pub(crate) fn end_response(value: state::ExchangeEndV1) -> Response {
    Response {
        kind: 61,
        detail: match value {
            state::ExchangeEndV1::Ended => 0,
            state::ExchangeEndV1::Ending => 1,
            state::ExchangeEndV1::AlreadyEnded => 2,
        },
        ..Response::default()
    }
}

/// Reserve the requested exchange synchronously, without wallet/proof/UI admission waits.
/// Kind60 contains only the nonzero exchange identity in sequence.
/// # Safety
/// `out` must point to writable initialized result storage with no unreleased bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_begin_exchange_v1(
    handle: u64,
    out: *mut WalletResult,
) -> i32 {
    let mut retained = None;
    let status = unsafe {
        exports::output(out, || {
            let issued = begin(handle)?;
            let response = issued.response();
            retained = Some(issued);
            Ok(response)
        })
    };
    if status == 0
        && let Some(issued) = retained.take()
    {
        issued.delivered();
    }
    status
}

/// Fence later calls; admitted children may finish. Does not join a user-held UI lifetime.
/// Kind61 detail0=Ended,1=Ending,2=AlreadyEnded; all other fields are empty.
/// # Safety
/// `out` must point to writable initialized result storage with no unreleased bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_end_exchange_v1(
    handle: u64,
    exchange_id: u64,
    out: *mut WalletResult,
) -> i32 {
    unsafe { exports::output(out, || Ok(end_response(end(handle, exchange_id)?))) }
}

#[cfg(test)]
mod tests;
