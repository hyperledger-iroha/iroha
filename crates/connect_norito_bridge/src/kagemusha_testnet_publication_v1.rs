//! Exclusive publication and dispatch ownership for the native testnet installation.
//!
//! This standard-library-only boundary keeps partially installed globals inaccessible and
//! permanently revokes dispatch after an uncertain callback or caught FFI panic.

use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::{
        Mutex, MutexGuard, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

/// Whether the complete native installation may dispatch calls into retained global owners.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TestnetPublicationStateV1 {
    /// Rust-only diagnostic installers may publish an owner without a startup context.
    Standalone,
    /// A signed-checkpoint context exists, but its complete host is not published.
    StartupPending,
    /// Every required installation and final freshness check has succeeded.
    Active,
    /// Installation or publication was uncertain; only process restart may recover.
    Poisoned,
}

/// One outer ownership gate for installation and all C/JNI testnet dispatch.
///
/// Lock order is publication, startup session, value ledger, observation owner. Native host
/// callbacks already hold publication ownership and must not reacquire it. All dispatch is
/// exclusive: a caught panic cannot revoke one operation while another admitted operation
/// continues. Installation retains the same lock until every journal and final lease check
/// succeeds; errors leave dispatch shut.
pub(crate) struct TestnetPublicationGateV1 {
    process_id: u32,
    state: Mutex<TestnetPublicationStateV1>,
    revoked: AtomicBool,
}

/// Dispatch ownership that permanently revokes the installation if its callback unwinds.
pub(crate) struct TestnetPublicationDispatchGuardV1<'a> {
    gate: &'a TestnetPublicationGateV1,
    _state: MutexGuard<'a, TestnetPublicationStateV1>,
}

/// Borrowed proof that the outer publication mutex is still exclusively held.
/// Its private constructor and guard-bound lifetime prevent ambient or retained bypasses.
pub(crate) struct TestnetPublicationPermitV1<'a> {
    gate: &'a TestnetPublicationGateV1,
    // Ownership cannot be handed to worker threads while its mutex guard stays here.
    not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'a> TestnetPublicationPermitV1<'a> {
    fn new(gate: &'a TestnetPublicationGateV1) -> Self {
        Self {
            gate,
            not_send_or_sync: PhantomData,
        }
    }

    /// Catch a scoped method's panic before an enclosing Rust callback can hide it.
    pub(crate) fn run<T>(
        &self,
        operation: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        self.require_valid()?;
        match catch_unwind(AssertUnwindSafe(operation)) {
            Ok(result) => {
                self.require_valid()?;
                result
            }
            Err(_) => {
                self.gate.revoked.store(true, Ordering::Release);
                Err("KAGEMUSHA testnet native mutation panicked; publication revoked".to_owned())
            }
        }
    }

    pub(crate) fn require_valid(&self) -> Result<(), String> {
        self.gate.require_process_owner()?;
        self.gate.require_not_revoked()
    }
}

/// Installation ownership; inner installers receive only a borrowed permit.
pub(crate) struct TestnetPublicationExclusiveGuardV1<'a> {
    gate: &'a TestnetPublicationGateV1,
    state: MutexGuard<'a, TestnetPublicationStateV1>,
}

impl TestnetPublicationExclusiveGuardV1<'_> {
    pub(crate) fn with_permit<T>(
        &mut self,
        use_owned: impl FnOnce(&mut TestnetPublicationStateV1, &TestnetPublicationPermitV1<'_>) -> T,
    ) -> T {
        use_owned(&mut self.state, &TestnetPublicationPermitV1::new(self.gate))
    }

    pub(crate) fn permit(&self) -> TestnetPublicationPermitV1<'_> {
        TestnetPublicationPermitV1::new(self.gate)
    }
}

impl Deref for TestnetPublicationExclusiveGuardV1<'_> {
    type Target = TestnetPublicationStateV1;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl DerefMut for TestnetPublicationExclusiveGuardV1<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

impl TestnetPublicationDispatchGuardV1<'_> {
    pub(crate) fn permit(&self) -> TestnetPublicationPermitV1<'_> {
        TestnetPublicationPermitV1::new(self.gate)
    }

    fn revoke(&self) {
        self.gate.revoked.store(true, Ordering::Release);
    }
}

impl Drop for TestnetPublicationDispatchGuardV1<'_> {
    fn drop(&mut self) {
        // Preserve revocation independently of the ownership mutex's poisoning state.
        if std::thread::panicking() {
            self.revoke();
        }
    }
}

/// Catch an FFI dispatch panic without hiding it from the shared publication owner.
pub(crate) fn catch_testnet_dispatch_panic_v1<T>(
    publication: &TestnetPublicationDispatchGuardV1<'_>,
    dispatch: impl FnOnce() -> T,
) -> std::thread::Result<T> {
    let result = catch_unwind(AssertUnwindSafe(dispatch));
    if result.is_err() {
        publication.revoke();
    }
    result
}

impl TestnetPublicationGateV1 {
    fn new() -> Self {
        Self {
            process_id: std::process::id(),
            state: Mutex::new(TestnetPublicationStateV1::Standalone),
            revoked: AtomicBool::new(false),
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        Self::new()
    }

    #[cfg(test)]
    pub(crate) fn inherited_for_test() -> Self {
        let mut gate = Self::new();
        gate.process_id = std::process::id().wrapping_add(1);
        gate
    }

    pub(crate) fn require_process_owner(&self) -> Result<(), String> {
        // Never touch a possibly locked mutex inherited from another process.
        if self.process_id != std::process::id() {
            return Err("KAGEMUSHA testnet publication belongs to another process".to_owned());
        }
        Ok(())
    }

    fn require_not_revoked(&self) -> Result<(), String> {
        if self.revoked.load(Ordering::Acquire) {
            return Err("KAGEMUSHA testnet publication was permanently revoked".to_owned());
        }
        Ok(())
    }

    pub(crate) fn dispatch(&self) -> Result<TestnetPublicationDispatchGuardV1<'_>, String> {
        self.require_process_owner()?;
        self.require_not_revoked()?;
        let state = self
            .state
            .lock()
            .map_err(|_| "KAGEMUSHA testnet publication lock is poisoned".to_owned())?;
        self.require_not_revoked()?;
        match *state {
            TestnetPublicationStateV1::Standalone | TestnetPublicationStateV1::Active => {
                Ok(TestnetPublicationDispatchGuardV1 {
                    gate: self,
                    _state: state,
                })
            }
            TestnetPublicationStateV1::StartupPending | TestnetPublicationStateV1::Poisoned => {
                Err("KAGEMUSHA complete testnet host is not active".to_owned())
            }
        }
    }

    pub(crate) fn exclusive(&self) -> Result<TestnetPublicationExclusiveGuardV1<'_>, String> {
        self.require_process_owner()?;
        self.require_not_revoked()?;
        let state = self
            .state
            .lock()
            .map_err(|_| "KAGEMUSHA testnet publication lock is poisoned".to_owned())?;
        self.require_not_revoked()?;
        Ok(TestnetPublicationExclusiveGuardV1 { gate: self, state })
    }

    /// Execute a Rust callback under the same panic/revocation boundary as C dispatch.
    pub(crate) fn with_dispatch<T>(
        &self,
        dispatch: impl FnOnce(&TestnetPublicationPermitV1<'_>) -> Result<T, String>,
    ) -> Result<T, String> {
        let guard = self.dispatch()?;
        let result = catch_testnet_dispatch_panic_v1(&guard, || dispatch(&guard.permit()))
            .map_err(|_| {
                "KAGEMUSHA testnet native callback panicked; publication revoked".to_owned()
            })?;
        guard.permit().require_valid()?;
        result
    }
}

pub(crate) fn testnet_publication_gate_v1() -> &'static TestnetPublicationGateV1 {
    static GATE: OnceLock<TestnetPublicationGateV1> = OnceLock::new();
    GATE.get_or_init(TestnetPublicationGateV1::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publication_dispatch_panic_revokes_every_later_dispatch_and_publication() {
        let gate = TestnetPublicationGateV1::new();
        *gate.exclusive().unwrap() = TestnetPublicationStateV1::Active;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let _publication = gate.dispatch().unwrap();
            panic!("uncertain native host callback");
        }));
        assert!(outcome.is_err());
        assert!(gate.state.is_poisoned());
        assert!(gate.dispatch().is_err());
        assert!(
            gate.exclusive().is_err(),
            "a revoked installation cannot be resurrected"
        );
    }

    #[test]
    fn caught_ffi_panic_revokes_dispatch_but_ordinary_rejection_does_not() {
        let gate = TestnetPublicationGateV1::new();
        let publication = gate.dispatch().unwrap();
        assert!(matches!(
            catch_testnet_dispatch_panic_v1(&publication, || Err::<(), _>("invalid proof")),
            Ok(Err("invalid proof")),
        ));
        drop(publication);
        let publication = gate.dispatch().unwrap();
        let outcome = catch_testnet_dispatch_panic_v1(&publication, || {
            panic!("uncertain C dispatch");
        });
        assert!(outcome.is_err());
        drop(publication);
        assert!(!gate.state.is_poisoned());
        assert!(gate.dispatch().is_err());
        assert!(gate.exclusive().is_err());
    }

    #[test]
    fn concurrent_dispatch_cannot_outlive_another_dispatch_revocation() {
        use std::{sync::mpsc, thread, time::Duration};

        let gate = TestnetPublicationGateV1::new();
        let (started_tx, started_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        thread::scope(|threads| {
            let first = gate.dispatch().unwrap();
            let pending = threads.spawn(|| {
                started_tx.send(()).unwrap();
                result_tx.send(gate.dispatch().is_ok()).unwrap();
            });
            let started = started_rx.recv_timeout(Duration::from_secs(5));
            let blocked = result_rx.recv_timeout(Duration::from_millis(50));
            let revoked = catch_testnet_dispatch_panic_v1(&first, || panic!("dispatch failed"));
            // Release ownership and join before asserting, even if a timed observation failed.
            drop(first);
            let completed = result_rx.recv_timeout(Duration::from_secs(5));
            let joined = pending.join();
            assert_eq!(started, Ok(()));
            assert_eq!(blocked, Err(mpsc::RecvTimeoutError::Timeout),);
            assert!(revoked.is_err());
            assert_eq!(completed, Ok(false));
            joined.unwrap();
        });
        assert!(gate.dispatch().is_err());
        assert!(gate.exclusive().is_err());
    }

    #[test]
    fn borrowed_permit_is_revoked_even_when_caller_catches_panic_before_guard_drop() {
        let gate = TestnetPublicationGateV1::new();
        let guard = gate.dispatch().unwrap();
        let permit = guard.permit();
        permit.require_valid().unwrap();
        assert!(
            catch_testnet_dispatch_panic_v1(&guard, || panic!("caught native failure")).is_err()
        );
        assert!(permit.require_valid().is_err());
        drop(guard);
        assert!(!gate.state.is_poisoned());
        assert!(gate.with_dispatch(|_| Ok(())).is_err());
    }

    #[test]
    fn rust_callbacks_catch_panics_and_never_reenter_after_revocation() {
        let gate = TestnetPublicationGateV1::new();
        let entered = std::cell::Cell::new(0);
        assert!(
            gate.with_dispatch::<()>(|permit| {
                permit.require_valid()?;
                entered.set(entered.get() + 1);
                panic!("Rust host mutation became uncertain");
            })
            .is_err()
        );
        assert_eq!(entered.get(), 1);
        assert!(
            gate.with_dispatch(|_| {
                entered.set(2);
                Ok(())
            })
            .is_err()
        );
        assert_eq!(entered.get(), 1);
        assert!(
            !gate.state.is_poisoned(),
            "caught panic still permanently revokes"
        );
    }

    #[test]
    fn nested_method_cannot_hide_revocation_from_any_enclosing_success() {
        let gate = TestnetPublicationGateV1::new();
        let result = gate.with_dispatch(|permit| {
            let outer = permit.run(|| {
                assert!(permit.run::<()>(|| panic!("nested owner failure")).is_err());
                Ok(7_u8)
            });
            assert!(
                outer.is_err(),
                "a scoped method cannot return stale success"
            );
            Ok(9_u8)
        });
        assert!(
            result.is_err(),
            "dispatch cannot return stale success either"
        );
        assert!(!gate.state.is_poisoned());
        assert!(gate.dispatch().is_err());
    }

    #[test]
    fn publication_rejects_inherited_process_before_touching_ownership_lock() {
        use std::{sync::mpsc, thread, time::Duration};

        let mut gate = TestnetPublicationGateV1::new();
        gate.process_id = std::process::id().wrapping_add(1);
        let (send, receive) = mpsc::channel();
        thread::scope(|threads| {
            let locked = gate.state.lock().unwrap();
            let worker = threads.spawn(|| {
                send.send((gate.dispatch().err(), gate.exclusive().err()))
                    .unwrap();
            });
            let observed = receive.recv_timeout(Duration::from_secs(5));
            // A lock-before-PID regression must fail promptly rather than strand the worker.
            drop(locked);
            let joined = worker.join();
            let expected =
                Some("KAGEMUSHA testnet publication belongs to another process".to_owned());
            assert_eq!(observed, Ok((expected.clone(), expected)));
            joined.unwrap();
        });
    }
}
