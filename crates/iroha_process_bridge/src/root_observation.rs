//! Separate bounded lifecycle observer: Python retains the original process.
//!
//! This observes only the caller before creation and selects a child through its
//! already recorded direct FORK. No operation attaches to a supplied PID, signals
//! a task, or reaps it. A terminal certificate covers lifecycle only; the embedding
//! harness must independently join its retained executable and live initial image.

use super::INVALID;
#[cfg(target_os = "linux")]
use super::boundary;

/// This host has no certified Linux root-thread-group observation implementation.
pub const ROOT_OBSERVER_UNSUPPORTED: i32 = -5;
/// Observation tokens are disjoint from original process-session tokens.
pub const OBSERVER_TAG: u64 = 1 << 63;
const MAX_OUTPUT: usize = 64 * 1024;

#[cfg(target_os = "linux")]
mod kernel;
#[cfg(target_os = "linux")]
mod ledger;

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, norito::json::JsonSerialize)]
struct Failure {
    reason: &'static str,
    errno: Option<i32>,
}
#[cfg(target_os = "linux")]
type Result<T> = std::result::Result<T, Failure>;
#[cfg(target_os = "linux")]
impl Failure {
    const fn new(reason: &'static str) -> Self {
        Self {
            reason,
            errno: None,
        }
    }
    fn io(error: std::io::Error) -> Self {
        let errno = error.raw_os_error();
        drop(error);
        Self {
            reason: "native_io",
            errno,
        }
    }
    fn last_io() -> Self {
        Self::io(std::io::Error::last_os_error())
    }
}

#[cfg(target_os = "linux")]
mod implementation {
    use super::{Failure, OBSERVER_TAG, Result, kernel, ledger};
    use crate::{INVALID, IO, OK, PANIC, PENDING, UNKNOWN};
    use std::{
        collections::BTreeMap,
        sync::{
            Arc, Mutex, OnceLock,
            atomic::{AtomicU64, Ordering},
        },
    };

    // Norito supplies canonical Vec serialization but its generic reference
    // implementation requires FastJsonWrite. This borrowed projection delegates
    // both methods to the existing Vec implementation, without another encoder.
    struct BorrowedVector<'a, T>(&'a Vec<T>);
    impl<T: norito::json::JsonSerialize> norito::json::JsonSerialize for BorrowedVector<'_, T> {
        fn json_serialize(&self, output: &mut String) {
            self.0.json_serialize(output);
        }
        fn json_serialize_to(
            &self,
            output: &mut dyn norito::json::JsonWriteSink,
        ) -> std::result::Result<(), norito::json::BoundedJsonError> {
            self.0.json_serialize_to(output)
        }
    }

    #[derive(norito::json::JsonSerialize)]
    struct Report<'a> {
        schema: &'static str,
        scope: &'static str,
        supervisor_pid: u32,
        root_pid: Option<u32>,
        prepared_ns: u64,
        observed_through_ns: u64,
        initial_exec_ns: Option<u64>,
        possible_cpus: BorrowedVector<'a, u32>,
        online_cpus: BorrowedVector<'a, u32>,
        records: usize,
        target_tasks: usize,
        counters: BorrowedVector<'a, kernel::Counter>,
        first_error: Option<Failure>,
        terminal: Option<kernel::Terminal>,
        finalized: bool,
        lifecycle_certified: bool,
        image_authenticated: bool,
        descendant_cleanup: &'static str,
    }

    struct Observation {
        supervisor: u32,
        topology: Option<kernel::Topology>,
        rings: Vec<kernel::Ring>,
        records: Vec<ledger::Record>,
        counters: Vec<kernel::Counter>,
        admitted_ns: u64,
        observed_ns: u64,
        prepared: bool,
        root: Option<u32>,
        first: Option<Failure>,
        facts: ledger::Facts,
        terminal: Option<kernel::Terminal>,
        finished: bool,
        frozen: Option<Vec<u8>>,
        closing: bool,
        close_error: Option<Failure>,
    }

    impl Observation {
        fn reserved() -> Self {
            Self {
                supervisor: std::process::id(),
                topology: None,
                rings: Vec::with_capacity(kernel::MAX_CPUS),
                records: Vec::with_capacity(ledger::MAX_RECORDS),
                counters: Vec::with_capacity(kernel::MAX_CPUS),
                admitted_ns: 0,
                observed_ns: 0,
                prepared: false,
                root: None,
                first: None,
                facts: ledger::Facts::default(),
                terminal: None,
                finished: false,
                frozen: None,
                closing: false,
                close_error: None,
            }
        }

        fn retain(&mut self, error: Failure) {
            self.first.get_or_insert(error);
        }

        fn initialize(&mut self) -> Result<()> {
            if kernel::single_main()? != self.supervisor {
                return Err(Failure::new("caller_identity"));
            }
            let topology = kernel::topology()?;
            for cpu in &topology.possible {
                self.rings.push(kernel::Ring::open_current(*cpu)?);
            }
            if kernel::topology()? != topology {
                return Err(Failure::new("topology_drift"));
            }
            self.topology = Some(topology);
            self.admitted_ns = kernel::monotonic_ns()?;
            self.observed_ns = self.admitted_ns;
            self.prepared = true;
            Ok(())
        }

        fn same_caller(&self) -> Result<()> {
            if kernel::caller()? != self.supervisor || std::process::id() != self.supervisor {
                return Err(Failure::new("caller_identity"));
            }
            Ok(())
        }

        fn collect(&mut self) {
            self.counters.clear();
            for index in 0..self.rings.len() {
                if let Err(error) = self.rings[index].drain(&mut self.records) {
                    self.retain(error);
                }
                match self.rings[index].counter() {
                    Ok(counter) => {
                        if counter.lost != 0 {
                            self.retain(Failure::new("event_loss"));
                        }
                        self.counters.push(counter);
                    }
                    Err(error) => self.retain(error),
                }
            }
            match kernel::topology() {
                Ok(current) if self.topology.as_ref() == Some(&current) => {}
                Ok(_) => self.retain(Failure::new("topology_drift")),
                Err(error) => self.retain(error),
            }
            match kernel::monotonic_ns() {
                Ok(time) => self.observed_ns = time,
                Err(error) => self.retain(error),
            }
        }

        fn report(&self) -> Result<Vec<u8>> {
            if let Some(frozen) = &self.frozen {
                return Ok(frozen.clone());
            }
            let initial_exec_ns = self.root.and_then(|root| {
                let mut events = self
                    .records
                    .iter()
                    .filter(|record| record.exec && record.pid == root);
                let first = events.next()?;
                (events.next().is_none()).then_some(first.time)
            });
            let empty_cpus = Vec::new();
            let report = Report {
                schema: "iroha.root-thread-group-observation.v1",
                scope: "original_root_thread_group_lifecycle",
                supervisor_pid: self.supervisor,
                root_pid: self.root,
                prepared_ns: self.admitted_ns,
                observed_through_ns: self.observed_ns,
                initial_exec_ns,
                possible_cpus: BorrowedVector(
                    self.topology
                        .as_ref()
                        .map_or(&empty_cpus, |value| &value.possible),
                ),
                online_cpus: BorrowedVector(
                    self.topology
                        .as_ref()
                        .map_or(&empty_cpus, |value| &value.online),
                ),
                records: self.records.len(),
                target_tasks: self.facts.target_tasks,
                counters: BorrowedVector(&self.counters),
                first_error: self.first,
                terminal: self.terminal,
                finalized: self.finished,
                lifecycle_certified: self.prepared
                    && self.finished
                    && self.first.is_none()
                    && self.terminal.is_some()
                    && self.root.is_some()
                    && self.facts.initial_exec_ns.is_some(),
                image_authenticated: false,
                descendant_cleanup: if self.first.is_some() {
                    "unsupported_or_unknown"
                } else {
                    "root_thread_group_only"
                },
            };
            let bytes =
                norito::json::to_vec(&report).map_err(|_| Failure::new("report_encoding"))?;
            if bytes.len() > super::MAX_OUTPUT {
                return Err(Failure::new("report_extent"));
            }
            Ok(bytes)
        }

        fn finish(&mut self) -> std::result::Result<i32, i32> {
            if self.finished {
                return Ok(OK);
            }
            let root = self.root.ok_or(INVALID)?;
            let terminal = match kernel::terminal(root) {
                Ok(Some(terminal)) => terminal,
                Ok(None) => return Ok(PENDING),
                Err(error) => {
                    self.retain(error);
                    return Err(IO);
                }
            };
            // The Popen original remains unreaped. All root threads have reached
            // terminal before any event is disabled or any ring/module is closed.
            self.terminal = Some(terminal);
            self.collect();
            match ledger::terminal(&self.records, self.supervisor, Some(root), self.admitted_ns) {
                Ok(facts) => self.facts = facts,
                Err(error) => self.retain(error),
            }
            for index in 0..self.rings.len() {
                if let Err(error) = self.rings[index].disable() {
                    self.retain(error);
                }
            }
            self.collect();
            let previous_records = self.records.len();
            let previous_counters = self.counters.clone();
            self.collect();
            if self.records.len() != previous_records || self.counters != previous_counters {
                self.retain(Failure::new("unstable_final_drain"));
            }
            match ledger::terminal(&self.records, self.supervisor, Some(root), self.admitted_ns) {
                Ok(facts) => self.facts = facts,
                Err(error) => self.retain(error),
            }
            self.finished = true;
            match self.report() {
                Ok(report) => {
                    self.frozen = Some(report);
                    Ok(OK)
                }
                Err(error) => {
                    self.retain(error);
                    Err(IO)
                }
            }
        }

        fn close_preparation(&mut self) -> std::result::Result<i32, i32> {
            // prepare returned an error before conferring permission to Popen.
            if !self.prepared {
                return Ok(OK);
            }
            if self.root.is_some() {
                return Ok(if self.finished { OK } else { PENDING });
            }
            if kernel::single_main().is_err() {
                return Ok(PENDING);
            }
            self.collect();
            if self.first.is_some() {
                return Err(IO);
            }
            // Popen may have internally forked/reaped an exec-refusal child before
            // raising. Only an explicit complete no-EXEC ledger can close here;
            // absent/lost initial evidence never becomes a no-child assertion.
            if let Err(error) =
                ledger::terminal(&self.records, self.supervisor, None, self.admitted_ns)
            {
                self.retain(error);
                return Ok(PENDING);
            }
            for index in 0..self.rings.len() {
                if let Err(error) = self.rings[index].disable() {
                    self.retain(error);
                }
            }
            self.collect();
            if let Err(error) =
                ledger::terminal(&self.records, self.supervisor, None, self.admitted_ns)
            {
                self.retain(error);
            }
            if self.first.is_some() {
                return Err(IO);
            }
            Ok(OK)
        }
    }

    fn observations() -> &'static Mutex<BTreeMap<u64, Arc<Mutex<Observation>>>> {
        static VALUE: OnceLock<Mutex<BTreeMap<u64, Arc<Mutex<Observation>>>>> = OnceLock::new();
        VALUE.get_or_init(|| Mutex::new(BTreeMap::new()))
    }

    fn original(token: u64) -> std::result::Result<Arc<Mutex<Observation>>, i32> {
        if token & OBSERVER_TAG == 0 {
            return Err(UNKNOWN);
        }
        observations()
            .lock()
            .map_err(|_| PANIC)?
            .get(&token)
            .cloned()
            .ok_or(UNKNOWN)
    }

    fn next_observer_token(value: u64) -> Option<u64> {
        (value & OBSERVER_TAG != 0)
            .then(|| value.checked_add(1))
            .flatten()
    }

    pub(super) fn reserve() -> std::result::Result<u64, i32> {
        static NEXT: AtomicU64 = AtomicU64::new(OBSERVER_TAG);
        let mut owners = observations().lock().map_err(|_| PANIC)?;
        if !owners.is_empty() {
            return Err(PENDING);
        }
        let token = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, next_observer_token)
            .map_err(|_| PANIC)?;
        owners.insert(token, Arc::new(Mutex::new(Observation::reserved())));
        Ok(token)
    }

    pub(super) fn initialize(token: u64) -> std::result::Result<i32, i32> {
        let original = original(token)?;
        let mut state = original.lock().map_err(|_| PANIC)?;
        match state.initialize() {
            Ok(()) => Ok(OK),
            Err(error) => {
                state.retain(error);
                Err(IO)
            }
        }
    }

    pub(super) fn bind(token: u64, root: u32) -> std::result::Result<i32, i32> {
        let original = original(token)?;
        let mut state = original.lock().map_err(|_| PANIC)?;
        state.same_caller().map_err(|_| INVALID)?;
        if state.closing
            || !state.prepared
            || state.root.is_some()
            || root == 0
            || root == state.supervisor
        {
            return Err(INVALID);
        }
        state.collect();
        match ledger::birth(&state.records, state.supervisor, root, state.admitted_ns) {
            Ok(true) => {
                state.root = Some(root);
                Ok(if state.first.is_none() { OK } else { IO })
            }
            Ok(false) => Ok(if state.first.is_none() { PENDING } else { IO }),
            Err(error) => {
                state.retain(error);
                Err(IO)
            }
        }
    }

    pub(super) fn report(token: u64, finish: bool) -> std::result::Result<(i32, Vec<u8>), i32> {
        let original = original(token)?;
        let mut state = original.lock().map_err(|_| PANIC)?;
        state.same_caller().map_err(|_| INVALID)?;
        if state.closing && finish {
            return Err(INVALID);
        }
        let status = if finish {
            state.finish()?
        } else {
            if !state.finished && !state.closing {
                state.collect();
            }
            OK
        };
        let bytes = state.report().map_err(|error| {
            state.retain(error);
            IO
        })?;
        Ok((status, bytes))
    }

    pub(super) fn close(token: u64) -> std::result::Result<i32, i32> {
        let original = original(token)?;
        let mut state = original.lock().map_err(|_| PANIC)?;
        state.same_caller().map_err(|_| INVALID)?;
        if state.close_error.is_some() {
            return Err(IO);
        }
        if !state.closing {
            let status = state.close_preparation()?;
            if status != OK {
                return Ok(status);
            }
            state.closing = true;
        }
        for index in 0..state.rings.len() {
            if let Err(error) = state.rings[index].close() {
                state.retain(error);
                state.close_error.get_or_insert(error);
            }
        }
        if state.close_error.is_some() {
            return Err(IO);
        }
        // No process owner is dropped here. This removes only observation FDs,
        // mappings and records after the explicit preparation/terminal boundary.
        observations()
            .lock()
            .map_err(|_| PANIC)?
            .remove(&token)
            .ok_or(UNKNOWN)?;
        Ok(OK)
    }
    #[cfg(test)]
    mod token_tests {
        use super::*;
        #[test]
        fn borrowed_report_vectors_use_the_canonical_norito_encoding() {
            for values in [vec![], vec![0_u32], vec![1, 7, 255]] {
                assert_eq!(
                    norito::json::to_vec(&BorrowedVector(&values)).unwrap(),
                    norito::json::to_vec(&values).unwrap()
                );
            }
            let counters = vec![kernel::Counter { cpu: 1, lost: 3 }];
            assert_eq!(
                norito::json::to_vec(&BorrowedVector(&counters)).unwrap(),
                norito::json::to_vec(&counters).unwrap()
            );
        }

        #[test]
        fn original_error_is_not_replaced_by_later_observation_or_cleanup_errors() {
            let mut observation = Observation::reserved();
            let first = Failure::io(std::io::Error::from_raw_os_error(libc::EACCES));
            observation.retain(first);
            observation.retain(Failure::new("event_loss"));
            observation.retain(Failure::io(std::io::Error::from_raw_os_error(libc::EBADF)));
            assert_eq!(observation.first, Some(first));
        }

        #[test]
        fn nonzero_terminal_status_is_separate_from_lifecycle_certificate() {
            let mut observation = Observation::reserved();
            observation.prepared = true;
            observation.root = Some(200);
            observation.facts.initial_exec_ns = Some(20);
            observation.terminal = Some(kernel::Terminal {
                pid: 200,
                code: libc::CLD_EXITED,
                status: 101,
            });
            observation.finished = true;
            let report = String::from_utf8(observation.report().unwrap()).unwrap();
            assert!(report.contains("\"status\":101"));
            assert!(report.contains("\"lifecycle_certified\":true"));
            assert!(report.contains("\"image_authenticated\":false"));
            observation.retain(Failure::new("event_loss"));
            let report = String::from_utf8(observation.report().unwrap()).unwrap();
            assert!(report.contains("\"lifecycle_certified\":false"));
        }

        #[test]
        fn observer_sequence_never_wraps_or_accepts_process_tokens() {
            assert_eq!(next_observer_token(0), None);
            assert_eq!(next_observer_token(OBSERVER_TAG - 1), None);
            assert_eq!(next_observer_token(OBSERVER_TAG), Some(OBSERVER_TAG + 1));
            assert_eq!(next_observer_token(u64::MAX - 1), Some(u64::MAX));
            assert_eq!(next_observer_token(u64::MAX), None);
            assert!(matches!(original(1), Err(UNKNOWN)));
        }
    }
}

/// Prepare original caller-thread observation before Popen or reader-thread creation.
/// The nonzero high-bit token is installed before native resource acquisition.
/// A failed preparation must be explicitly closed; it never permits child spawn.
///
/// # Safety
/// `out_token` is an aligned writable u64 retained for this entire call.
#[allow(
    unsafe_code,
    reason = "bounded C output exposes its token before native resource acquisition"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iroha_process_root_observer_prepare_v1(out_token: *mut u64) -> i32 {
    if out_token.is_null() {
        return INVALID;
    }
    // SAFETY: the caller supplies the documented writable aligned output.
    unsafe {
        out_token.write(0);
    }
    #[cfg(target_os = "linux")]
    {
        boundary(|| {
            let token = implementation::reserve()?;
            // SAFETY: token output remains valid for the call, before any native open.
            unsafe {
                out_token.write(token);
            }
            implementation::initialize(token)
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        ROOT_OBSERVER_UNSUPPORTED
    }
}

/// Bind only the unique direct FORK already observed for a returned original Popen.
/// A PID is a selection join, never an attachment, lookup or signaling authority.
#[unsafe(no_mangle)]
#[allow(unsafe_code, reason = "export the bounded no-attachment C ABI symbol")]
pub extern "C" fn iroha_process_root_observer_bind_v1(token: u64, returned_child_pid: u32) -> i32 {
    #[cfg(target_os = "linux")]
    {
        boundary(|| implementation::bind(token, returned_child_pid))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (token, returned_child_pid);
        ROOT_OBSERVER_UNSUPPORTED
    }
}

#[allow(
    unsafe_code,
    reason = "copy bounded canonical report to caller-owned output extent"
)]
unsafe fn projection(
    token: u64,
    finish: bool,
    output: *mut u8,
    capacity: usize,
    written: *mut usize,
) -> i32 {
    if output.is_null() || written.is_null() || capacity == 0 || capacity > MAX_OUTPUT {
        return INVALID;
    }
    // SAFETY: the caller supplies independent aligned writable count storage.
    unsafe {
        written.write(0);
    }
    #[cfg(target_os = "linux")]
    {
        boundary(|| {
            let (status, bytes) = implementation::report(token, finish)?;
            if bytes.len() > capacity {
                return Err(INVALID);
            }
            // SAFETY: output/count are documented disjoint writable regions and the
            // bounded serialized report is private, initialized, and independent.
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), output, bytes.len());
                written.write(bytes.len());
            }
            Ok(status)
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (token, finish);
        ROOT_OBSERVER_UNSUPPORTED
    }
}

/// Drain original rings and return bounded lifecycle facts, never image authority.
///
/// # Safety
/// `output` references `capacity` writable bytes and `written` an aligned writable
/// usize, disjoint from each other, both retained for this call.
#[allow(unsafe_code, reason = "documented bounded C report projection")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iroha_process_root_observer_poll_v1(
    token: u64,
    output: *mut u8,
    capacity: usize,
    written: *mut usize,
) -> i32 {
    // SAFETY: this function's caller contract is exactly the shared projection's.
    unsafe { projection(token, false, output, capacity, written) }
}

/// Freeze lifecycle facts while the selected original is terminal but unreaped.
/// The caller must join its live initial image and then independently wait4/reap.
/// A certified status101 is lifecycle evidence, never successful workload evidence.
///
/// # Safety
/// `output` references `capacity` writable bytes and `written` an aligned writable
/// usize, disjoint from each other, both retained for this call.
#[allow(
    unsafe_code,
    reason = "documented bounded nonreaping C terminal projection"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iroha_process_root_observer_finish_v1(
    token: u64,
    output: *mut u8,
    capacity: usize,
    written: *mut usize,
) -> i32 {
    // SAFETY: this function's caller contract is exactly the shared projection's.
    unsafe { projection(token, true, output, capacity, written) }
}

/// Close only observation resources after terminal or proven closed preparation.
/// This never reaps, signals, authenticates an image, or returns command success.
#[allow(
    unsafe_code,
    reason = "export the bounded observer-only C close symbol"
)]
#[unsafe(no_mangle)]
pub extern "C" fn iroha_process_root_observer_close_v1(token: u64) -> i32 {
    #[cfg(target_os = "linux")]
    {
        boundary(|| implementation::close(token))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = token;
        ROOT_OBSERVER_UNSUPPORTED
    }
}
