//! Process-exclusive, serialized adapter around a separately qualified native coordinator.
//!
//! This adapter supplies no hardware authority and must never be installed around a software
//! monetary mock. The inner backend must own the authenticated durable store, operation journal,
//! current governed release, and non-forking hardware session. The adapter only prevents a second
//! process-local open or concurrent dispatch through the generic C/JNI coordinator boundary.

use std::sync::{Arc, Mutex};

use super::{
    KagemushaCoreCoordinatorBackendErrorV1, KagemushaCoreCoordinatorBackendV1,
    KagemushaCoreCoordinatorMethodV1,
};

#[derive(Default)]
struct ExclusiveState {
    attempted_open: bool,
    selected_handle: Option<u64>,
}

/// One process-lifetime native owner over a prequalified platform backend.
///
/// Close revokes the current handle but cannot reopen this process: a new wallet or account
/// selection requires a fresh process and a fresh hardware-backed open. The delegate's handle is
/// checked on every invocation, and one mutex serializes all hardware calls for this owner. The delegate
/// remains responsible for crash-safe idempotency, response authentication and exact recovery.
pub struct KagemushaExclusiveCoordinatorBackendV1 {
    inner: Arc<dyn KagemushaCoreCoordinatorBackendV1>,
    state: Mutex<ExclusiveState>,
}

impl KagemushaExclusiveCoordinatorBackendV1 {
    /// Construct without installing; the caller must supply an independently qualified backend.
    #[must_use]
    pub fn new(inner: Arc<dyn KagemushaCoreCoordinatorBackendV1>) -> Self {
        Self {
            inner,
            state: Mutex::new(ExclusiveState::default()),
        }
    }
}

impl KagemushaCoreCoordinatorBackendV1 for KagemushaExclusiveCoordinatorBackendV1 {
    fn open(&self, storage_path: &str) -> Result<u64, KagemushaCoreCoordinatorBackendErrorV1> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        if state.attempted_open {
            // A repeated open may be an account switch. Never leave the previous UI handle
            // usable after rejecting that selection; this process cannot safely reopen.
            if let Some(handle) = state.selected_handle.take() {
                let _ = self.inner.close(handle);
            }
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        // Even an unavailable or panicking delegated open can have an uncertain hardware effect.
        // Consume this process's sole attempt before crossing that boundary.
        state.attempted_open = true;
        let handle = self.inner.open(storage_path)?;
        if handle == 0 {
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        state.selected_handle = Some(handle);
        Ok(handle)
    }

    fn invoke(
        &self,
        handle: u64,
        method: KagemushaCoreCoordinatorMethodV1,
        request_frame: &[u8],
    ) -> Result<Vec<u8>, KagemushaCoreCoordinatorBackendErrorV1> {
        let state = self
            .state
            .lock()
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        if handle == 0 || state.selected_handle != Some(handle) {
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        if matches!(
            method,
            KagemushaCoreCoordinatorMethodV1::InitialEnrollment
                | KagemushaCoreCoordinatorMethodV1::AcknowledgeCommittedAppAttest
                | KagemushaCoreCoordinatorMethodV1::ExportOutgoingStateProof
        ) {
            // These operations must use their dedicated native-owner hooks. A caller using
            // the public Rust adapter directly cannot bypass the opaque enrollment attempt
            // the committed App Attest journal check, or the retained proof archive through
            // generic dispatch.
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        // Keep the lock through device I/O and delegate result handling. The C/JNI entry point
        // independently validates the exact request and complete response archive before exposure.
        self.inner.invoke(handle, method, request_frame)
    }

    fn invoke_initial_enrollment(
        &self,
        handle: u64,
        request_frame: &[u8],
    ) -> Result<Vec<u8>, KagemushaCoreCoordinatorBackendErrorV1> {
        let state = self
            .state
            .lock()
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        if handle == 0 || state.selected_handle != Some(handle) {
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        self.inner.invoke_initial_enrollment(handle, request_frame)
    }

    fn acknowledge_committed_app_attest(
        &self,
        handle: u64,
        request_frame: &[u8],
    ) -> Result<Vec<u8>, KagemushaCoreCoordinatorBackendErrorV1> {
        let state = self
            .state
            .lock()
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        if handle == 0 || state.selected_handle != Some(handle) {
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        self.inner
            .acknowledge_committed_app_attest(handle, request_frame)
    }

    fn export_outgoing_state_proof(
        &self,
        handle: u64,
        operation_id: [u8; 32],
    ) -> Result<
        iroha_core::zk::kagemusha_v1_state::KagemushaOutgoingStateProofArchivePairV1,
        KagemushaCoreCoordinatorBackendErrorV1,
    > {
        let state = self
            .state
            .lock()
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        if handle == 0 || state.selected_handle != Some(handle) {
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        self.inner.export_outgoing_state_proof(handle, operation_id)
    }

    fn close(&self, handle: u64) -> Result<(), KagemushaCoreCoordinatorBackendErrorV1> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        if handle == 0 || state.selected_handle != Some(handle) {
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        // Revoke before delegated teardown. Even an unavailable hardware close cannot
        // restore this process's authority or permit a second open.
        state.selected_handle = None;
        self.inner.close(handle)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc, Barrier,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        thread,
        time::Duration,
    };

    use super::*;

    #[test]
    fn outgoing_proof_export_cannot_fall_back_to_generic_dispatch() {
        let backend = Arc::new(Backend::new(false));
        let owner = KagemushaExclusiveCoordinatorBackendV1::new(backend.clone());
        assert_eq!(owner.open("/private/wallet"), Ok(7));
        let operation_id = [0x5a; 32];
        assert_eq!(
            owner.export_outgoing_state_proof(7, operation_id),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Unavailable)
        );
        assert_eq!(
            owner.invoke(
                7,
                KagemushaCoreCoordinatorMethodV1::ExportOutgoingStateProof,
                b"forged-generic-request",
            ),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
        );
        assert_eq!(
            owner.export_outgoing_state_proof(8, operation_id),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
        );
        assert_eq!(backend.invoke_calls.load(Ordering::SeqCst), 0);
    }

    struct Backend {
        open_calls: AtomicUsize,
        invoke_calls: AtomicUsize,
        close_calls: AtomicUsize,
        active_calls: AtomicUsize,
        maximum_active: AtomicUsize,
        fail_open: bool,
        fail_close: AtomicBool,
    }

    impl Backend {
        fn new(fail_open: bool) -> Self {
            Self {
                open_calls: AtomicUsize::new(0),
                invoke_calls: AtomicUsize::new(0),
                close_calls: AtomicUsize::new(0),
                active_calls: AtomicUsize::new(0),
                maximum_active: AtomicUsize::new(0),
                fail_open,
                fail_close: AtomicBool::new(false),
            }
        }
    }

    impl KagemushaCoreCoordinatorBackendV1 for Backend {
        fn open(&self, _storage_path: &str) -> Result<u64, KagemushaCoreCoordinatorBackendErrorV1> {
            self.open_calls.fetch_add(1, Ordering::SeqCst);
            if self.fail_open {
                Err(KagemushaCoreCoordinatorBackendErrorV1::Unavailable)
            } else {
                Ok(7)
            }
        }

        fn invoke(
            &self,
            _handle: u64,
            _method: KagemushaCoreCoordinatorMethodV1,
            request_frame: &[u8],
        ) -> Result<Vec<u8>, KagemushaCoreCoordinatorBackendErrorV1> {
            self.invoke_calls.fetch_add(1, Ordering::SeqCst);
            let active = self.active_calls.fetch_add(1, Ordering::SeqCst) + 1;
            self.maximum_active.fetch_max(active, Ordering::SeqCst);
            thread::sleep(Duration::from_millis(2));
            self.active_calls.fetch_sub(1, Ordering::SeqCst);
            Ok(request_frame.to_vec())
        }

        fn close(&self, _handle: u64) -> Result<(), KagemushaCoreCoordinatorBackendErrorV1> {
            self.close_calls.fetch_add(1, Ordering::SeqCst);
            if self.fail_close.load(Ordering::SeqCst) {
                Err(KagemushaCoreCoordinatorBackendErrorV1::Unavailable)
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn app_attest_ack_does_not_fall_back_to_generic_invoke() {
        let backend = Arc::new(Backend::new(false));
        let owner = KagemushaExclusiveCoordinatorBackendV1::new(backend.clone());
        assert_eq!(owner.open("/private/wallet"), Ok(7));
        assert_eq!(
            owner.acknowledge_committed_app_attest(7, b"original-request"),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Unavailable)
        );
        assert_eq!(backend.invoke_calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            owner.invoke(
                7,
                KagemushaCoreCoordinatorMethodV1::AcknowledgeCommittedAppAttest,
                b"original-request",
            ),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
        );
        assert_eq!(backend.invoke_calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            owner.acknowledge_committed_app_attest(8, b"original-request"),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
        );
    }

    #[test]
    fn second_open_revokes_original_and_forged_handles_before_delegate() {
        let backend = Arc::new(Backend::new(false));
        let owner = KagemushaExclusiveCoordinatorBackendV1::new(backend.clone());
        assert_eq!(owner.open("/private/wallet"), Ok(7));
        assert_eq!(
            owner.open("/private/other"),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
        );
        assert_eq!(
            owner.invoke(
                8,
                KagemushaCoreCoordinatorMethodV1::RecoverSender,
                b"request"
            ),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
        );
        assert_eq!(
            owner.invoke(
                7,
                KagemushaCoreCoordinatorMethodV1::RecoverSender,
                b"request"
            ),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
        );
        assert_eq!(backend.open_calls.load(Ordering::SeqCst), 1);
        assert_eq!(backend.invoke_calls.load(Ordering::SeqCst), 0);
        assert_eq!(backend.close_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn unavailable_open_consumes_process_attempt() {
        let backend = Arc::new(Backend::new(true));
        let owner = KagemushaExclusiveCoordinatorBackendV1::new(backend.clone());
        assert_eq!(
            owner.open("/private/wallet"),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Unavailable)
        );
        assert_eq!(
            owner.open("/private/wallet"),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
        );
        assert_eq!(backend.open_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn concurrent_open_attempts_never_create_two_delegate_handles() {
        let backend = Arc::new(Backend::new(false));
        let owner = Arc::new(KagemushaExclusiveCoordinatorBackendV1::new(backend.clone()));
        let start = Arc::new(Barrier::new(3));
        let threads: Vec<_> = ["/private/wallet-a", "/private/wallet-b"]
            .into_iter()
            .map(|path| {
                let owner = owner.clone();
                let start = start.clone();
                thread::spawn(move || {
                    start.wait();
                    owner.open(path)
                })
            })
            .collect();
        start.wait();
        let results: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(backend.open_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            owner.invoke(
                7,
                KagemushaCoreCoordinatorMethodV1::RecoverSender,
                b"request"
            ),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
        );
        assert_eq!(backend.close_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn explicit_close_revokes_before_any_delegate_reuse() {
        let backend = Arc::new(Backend::new(false));
        let owner = KagemushaExclusiveCoordinatorBackendV1::new(backend.clone());
        assert_eq!(owner.open("/private/wallet"), Ok(7));
        assert_eq!(owner.close(7), Ok(()));
        assert_eq!(
            owner.invoke(
                7,
                KagemushaCoreCoordinatorMethodV1::RecoverSender,
                b"request"
            ),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
        );
        assert_eq!(
            owner.close(7),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
        );
        assert_eq!(
            owner.open("/private/wallet"),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
        );
        assert_eq!(backend.close_calls.load(Ordering::SeqCst), 1);
        assert_eq!(backend.invoke_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn enrollment_dispatch_requires_explicit_backend_implementation() {
        let backend = Arc::new(Backend::new(false));
        let owner = KagemushaExclusiveCoordinatorBackendV1::new(backend.clone());
        assert_eq!(owner.open("/private/wallet"), Ok(7));
        assert_eq!(
            owner.invoke_initial_enrollment(7, b"phase-one"),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Unavailable)
        );
        assert_eq!(backend.invoke_calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            owner.invoke(
                7,
                KagemushaCoreCoordinatorMethodV1::InitialEnrollment,
                b"phase-one",
            ),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
        );
        assert_eq!(backend.invoke_calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            owner.invoke_initial_enrollment(8, b"phase-one"),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
        );
        assert_eq!(owner.close(7), Ok(()));
        assert_eq!(
            owner.invoke_initial_enrollment(7, b"phase-one"),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
        );
    }

    #[test]
    fn failed_delegate_close_still_revokes_process_handle() {
        let backend = Arc::new(Backend::new(false));
        backend.fail_close.store(true, Ordering::SeqCst);
        let owner = KagemushaExclusiveCoordinatorBackendV1::new(backend.clone());
        assert_eq!(owner.open("/private/wallet"), Ok(7));
        assert_eq!(
            owner.close(7),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Unavailable)
        );
        assert_eq!(
            owner.invoke(
                7,
                KagemushaCoreCoordinatorMethodV1::RecoverSender,
                b"request"
            ),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
        );
        assert_eq!(
            owner.open("/private/wallet"),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
        );
        assert_eq!(backend.close_calls.load(Ordering::SeqCst), 1);
        assert_eq!(backend.invoke_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn concurrent_invocations_serialize_and_preserve_exact_delegate_bytes() {
        let backend = Arc::new(Backend::new(false));
        let owner = Arc::new(KagemushaExclusiveCoordinatorBackendV1::new(backend.clone()));
        assert_eq!(owner.open("/private/wallet"), Ok(7));
        let threads: Vec<_> = (0_u8..8)
            .map(|value| {
                let owner = owner.clone();
                thread::spawn(move || {
                    owner.invoke(7, KagemushaCoreCoordinatorMethodV1::RecoverSender, &[value])
                })
            })
            .collect();
        for (value, thread) in threads.into_iter().enumerate() {
            assert_eq!(thread.join().unwrap(), Ok(vec![value as u8]));
        }
        assert_eq!(backend.invoke_calls.load(Ordering::SeqCst), 8);
        assert_eq!(backend.maximum_active.load(Ordering::SeqCst), 1);
    }
}
