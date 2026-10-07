//! Native startup and original account admission; no foreign authority selections.
use super::*;
use iroha_core_zk::kagemusha_wallet_intake_v1 as intake;

pub(crate) const BOUNDS: [usize; 4] = [
    KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1,
    KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1,
    intake::ACCOUNT_ORIGINAL_MAX_BYTES_V1,
    intake::ASSET_SCOPE_ORIGINAL_MAX_BYTES_V1,
];

impl From<state::NativeOpenErrorV1> for Failure {
    fn from(error: state::NativeOpenErrorV1) -> Self {
        match error {
            state::NativeOpenErrorV1::State(error) => error.into(),
            state::NativeOpenErrorV1::Intake(intake::Error::Provider(error)) => error.into(),
            state::NativeOpenErrorV1::Intake(intake::Error::SourceChanged) => Self::code(CONFLICT),
            state::NativeOpenErrorV1::Intake(_) => Self::code(INVALID),
        }
    }
}
trait Admission: Send {
    fn enrollment(&mut self, _input: enrollment::Input<'_>) -> Result<enrollment::Response> {
        Err(Failure::code(ARTIFACTS_UNAVAILABLE))
    }
    fn begin(&mut self, originals: [&[u8]; 4]) -> Result<Vec<u8>>;
    fn finish(&mut self, signature: &[u8]) -> Result<(Box<dyn Wallet>, state::Scheduler)>;
    fn cancel(&mut self) -> Result<()>;
}
enum Phase<P: advance::KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send> {
    Ready(state::NativeWalletRuntimeV1<advance::KagemushaWalletStdFsV1, P, S>),
    Pending(state::PendingNativeWalletOpenV1<advance::KagemushaWalletStdFsV1, P, S>),
}
struct Runtime<P: advance::KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send> {
    phase: Option<Phase<P, S>>,
    binding: Option<super::installed::BoundOriginals>,
}
impl<P: advance::KagemushaWalletPlatformV1 + 'static, S: OriginalSourceV1 + Send + 'static>
    Admission for Runtime<P, S>
{
    fn enrollment(&mut self, input: enrollment::Input<'_>) -> Result<enrollment::Response> {
        let binding = self
            .binding
            .as_ref()
            .ok_or(Failure::code(ARTIFACTS_UNAVAILABLE))?;
        let scope = binding.enrollment(&input)?;
        let Some(Phase::Ready(runtime)) = self.phase.as_mut() else {
            return Err(Failure::code(CONFLICT));
        };
        enrollment::execute(runtime, input, scope)
    }
    fn begin(&mut self, originals: [&[u8]; 4]) -> Result<Vec<u8>> {
        validate(originals)?;
        if let Some(binding) = &self.binding {
            binding.require(originals)?;
        }
        let phase = self.phase.take().ok_or(Failure::code(CLOSED))?;
        let runtime = match phase {
            Phase::Pending(pending) => {
                // Lost transport replies do not replace an actual issued challenge.
                let same = pending.matches_originals(originals);
                let challenge = same.then(|| pending.challenge().to_vec());
                self.phase = Some(Phase::Pending(pending));
                return challenge.ok_or(Failure::code(CONFLICT));
            }
            Phase::Ready(runtime) => runtime,
        };
        let [credential, certificates, account, asset] = originals;
        match runtime.begin(credential, certificates, account, asset) {
            Ok(pending) => {
                let challenge = pending.challenge().to_vec();
                self.phase = Some(Phase::Pending(pending));
                Ok(challenge)
            }
            Err(failure) => {
                let (runtime, error) = failure.into_parts();
                self.phase = Some(Phase::Ready(runtime));
                Err(error.into())
            }
        }
    }
    fn finish(&mut self, signature: &[u8]) -> Result<(Box<dyn Wallet>, state::Scheduler)> {
        let phase = self.phase.take().ok_or(Failure::code(CLOSED))?;
        let Phase::Pending(pending) = phase else {
            self.phase = Some(phase);
            return Err(Failure::code(INVALID));
        };
        // Every ordinary intake/signature refusal retains this SAME actual challenge.
        // Explicit cancellation alone selects the separate abandonment path.
        match pending.finish(signature) {
            Ok(wallet) => {
                let scheduler = wallet.scheduler();
                Ok((
                    Box::new(NativeWallet {
                        wallet,
                        times: BTreeMap::new(),
                        next_time: 0,
                        reviews: review::Tokens::default(),
                    }),
                    scheduler,
                ))
            }
            Err(failure) => {
                let (runtime, error) = failure.into_parts();
                self.phase = Some(match runtime.recover_pending() {
                    Ok(pending) => Phase::Pending(pending),
                    // Current later native coordinator failure remains an owned runtime;
                    // it cannot be fabricated into an account Pending here.
                    Err(runtime) => Phase::Ready(runtime),
                });
                Err(error.into())
            }
        }
    }
    fn cancel(&mut self) -> Result<()> {
        let phase = self.phase.take().ok_or(Failure::code(CLOSED))?;
        match phase {
            Phase::Pending(pending) => {
                self.phase = Some(Phase::Ready(pending.abandon()));
                Ok(())
            }
            other => {
                self.phase = Some(other);
                Err(Failure::code(INVALID))
            }
        }
    }
}
pub(super) struct RuntimeOwner {
    pub(super) closing: Arc<closing::CloseState>,
    admission: Mutex<Option<Box<dyn Admission>>>,
    finished: Mutex<Option<(Box<dyn Wallet>, state::Scheduler)>>,
}
fn validate(originals: [&[u8]; 4]) -> Result<()> {
    if originals
        .iter()
        .zip(BOUNDS)
        .any(|(original, bound)| original.is_empty() || original.len() > bound)
    {
        return Err(Failure::code(INVALID));
    }
    Ok(())
}
fn runtime(id: u64) -> Result<Arc<RuntimeOwner>> {
    let owner = registry()
        .lock()
        .map_err(|_| Failure::code(INTERNAL))?
        .runtimes
        .get(&id)
        .cloned()
        .ok_or(Failure::code(ARTIFACTS_UNAVAILABLE))?;
    owner.closing.require_open()?;
    Ok(owner)
}
pub(crate) fn begin(id: u64, originals: [&[u8]; 4]) -> Result<Response> {
    validate(originals)?;
    let owner = runtime(id)?;
    let mut guard = owner
        .admission
        .lock()
        .map_err(|_| Failure::code(INTERNAL))?;
    owner.closing.require_open()?;
    if owner
        .finished
        .lock()
        .map_err(|_| Failure::code(INTERNAL))?
        .is_some()
    {
        return Err(Failure::code(CONFLICT));
    }
    let bytes = guard
        .as_deref_mut()
        .ok_or(Failure::code(CLOSED))?
        .begin(originals)?;
    Ok(Response {
        kind: 15,
        sequence: u128::from(id),
        bytes,
        ..Response::default()
    })
}
pub(crate) fn finish(id: u64, signature: &[u8]) -> Result<Response> {
    let owner = {
        let selected = registry().lock().map_err(|_| Failure::code(INTERNAL))?;
        if let Some(owner) = selected.owners.get(&id) {
            owner.closing.require_open()?;
            return Ok(opened(id));
        }
        selected.runtimes.get(&id).cloned().ok_or(Failure::code(
            if id > 0 && id <= selected.next {
                CLOSED
            } else {
                ARTIFACTS_UNAVAILABLE
            },
        ))?
    };
    finish_with(registry(), owner, id, signature)
}
fn opened(id: u64) -> Response {
    Response {
        kind: 16,
        sequence: u128::from(id),
        ..Response::default()
    }
}
fn finish_with(
    registry: &Mutex<Registry>,
    owner: Arc<RuntimeOwner>,
    id: u64,
    signature: &[u8],
) -> Result<Response> {
    let mut admission = owner
        .admission
        .lock()
        .map_err(|_| Failure::code(INTERNAL))?;
    owner.closing.require_open()?;
    let mut retained = owner.finished.lock().map_err(|_| Failure::code(INTERNAL))?;
    if retained.is_none() && admission.is_none() {
        // Another finish may have promoted this same runtime while this caller waited.
        let selected = registry.lock().map_err(|_| Failure::code(INTERNAL))?;
        return match selected.owners.get(&id) {
            Some(active) => {
                active.closing.require_open()?;
                Ok(opened(id))
            }
            None => Err(Failure::code(CLOSED)),
        };
    }
    if retained.is_none() {
        let completed = admission
            .as_deref_mut()
            .ok_or(Failure::code(CLOSED))?
            .finish(signature)?;
        *retained = Some(completed);
        admission.take();
    }
    // Preserve an initialized owner before touching the registry. A registration error
    // cannot drop custody or repeat account authorization on a later promotion attempt.
    let mut selected = registry.lock().map_err(|_| Failure::code(INTERNAL))?;
    owner.closing.require_open()?;
    if !selected
        .runtimes
        .get(&id)
        .is_some_and(|active| Arc::ptr_eq(active, &owner))
    {
        return Err(Failure::code(CLOSED));
    }
    let (wallet, scheduler) = retained.take().ok_or(Failure::code(INTERNAL))?;
    selected.runtimes.remove(&id);
    selected.owners.insert(
        id,
        Arc::new(Owner {
            closing: Arc::clone(&owner.closing),
            scheduler,
            wallet: Mutex::new(Some(wallet)),
        }),
    );
    Ok(opened(id))
}
pub(crate) fn cancel(id: u64) -> Result<()> {
    let owner = runtime(id)?;
    let mut admission = owner
        .admission
        .lock()
        .map_err(|_| Failure::code(INTERNAL))?;
    owner.closing.require_open()?;
    admission
        .as_deref_mut()
        .ok_or(Failure::code(CLOSED))?
        .cancel()
}
pub(super) fn enroll(id: u64, input: enrollment::Input<'_>) -> Result<enrollment::Response> {
    enroll_with(runtime(id)?, input)
}
fn enroll_with(
    owner: Arc<RuntimeOwner>,
    input: enrollment::Input<'_>,
) -> Result<enrollment::Response> {
    let mut admission = owner
        .admission
        .lock()
        .map_err(|_| Failure::code(INTERNAL))?;
    owner.closing.require_open()?;
    if owner
        .finished
        .lock()
        .map_err(|_| Failure::code(INTERNAL))?
        .is_some()
    {
        return Err(Failure::code(CONFLICT));
    }
    admission
        .as_deref_mut()
        .ok_or(Failure::code(CLOSED))?
        .enrollment(input)
}
pub(super) fn close(owner: Arc<RuntimeOwner>) -> Result<()> {
    owner.closing.join(|| {
        // Acquire both fallible locks before dropping either actual custody owner.
        let mut admission = owner
            .admission
            .lock()
            .map_err(|_| Failure::code(INTERNAL))?;
        let mut finished = owner.finished.lock().map_err(|_| Failure::code(INTERNAL))?;
        let _priority = finished.as_ref().map(|(_, scheduler)| {
            scheduler.set_activity(false, false);
            scheduler.payment()
        });
        drop(admission.take());
        drop(finished.take());
        Ok(())
    })
}

/// Native deployment startup failure; custody and original-store ownership remain recoverable.
pub enum NativeStartupFailure<P: advance::KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send> {
    /// Signed artifact/source qualification failed before registration.
    Load(state::NativeStartupFailureV1<advance::KagemushaWalletStdFsV1, P, S>),
    /// The loaded runtime could not be registered; neither custody nor originals were dropped.
    Registration {
        /// Exact loaded native runtime, ready for registration retry.
        runtime: state::NativeWalletRuntimeV1<advance::KagemushaWalletStdFsV1, P, S>,
        /// Registry failure.
        failure: Failure,
    },
}
/// Load and register the genuine native installation from trusted embedding-app initialization.
///
/// Native deployment supplies independent configuration/genesis and the actual exclusive
/// provider root. The phone-facing original-open APIs accept only the returned opaque handle,
/// original owner frames and an existing account signature. They cannot select trust pins.
/// # Errors
/// Invalid/unavailable complete artifacts, configuration mismatch or registry capacity.
/// Every error retains the exclusive provider and sole original source in its typed failure.
pub fn start_native_wallet<P, S>(
    config: state::NativeInstallationConfigV1,
    provider: advance::KagemushaWalletProviderV1<advance::KagemushaWalletStdFsV1, P>,
    verifier_pack: &[u8],
    producer_inventory: &[u8],
    originals: S,
) -> std::result::Result<u64, NativeStartupFailure<P, S>>
where
    P: advance::KagemushaWalletPlatformV1 + 'static,
    S: OriginalSourceV1 + Send + 'static,
{
    let runtime = state::NativeWalletRuntimeV1::load(
        config,
        provider,
        verifier_pack,
        producer_inventory,
        originals,
    )
    .map_err(NativeStartupFailure::Load)?;
    retain_native_runtime(runtime)
}
/// Register one already loaded native runtime, including retry after registry unavailability.
/// # Errors
/// Capacity or poisoned registry; the exact runtime is returned and remains unadmitted.
pub fn retain_native_runtime<P, S>(
    runtime: state::NativeWalletRuntimeV1<advance::KagemushaWalletStdFsV1, P, S>,
) -> std::result::Result<u64, NativeStartupFailure<P, S>>
where
    P: advance::KagemushaWalletPlatformV1 + 'static,
    S: OriginalSourceV1 + Send + 'static,
{
    retain_runtime(runtime, None)
}
/// Retain the genuine native installation with its independently authenticated app scope.
/// This function is private to the installed-original intake; foreign IDs cannot create it.
pub(super) fn retain_native_runtime_bound<P, S>(
    runtime: state::NativeWalletRuntimeV1<advance::KagemushaWalletStdFsV1, P, S>,
    binding: super::installed::BoundOriginals,
) -> std::result::Result<u64, NativeStartupFailure<P, S>>
where
    P: advance::KagemushaWalletPlatformV1 + 'static,
    S: OriginalSourceV1 + Send + 'static,
{
    retain_runtime(runtime, Some(binding))
}
fn retain_runtime<P, S>(
    runtime: state::NativeWalletRuntimeV1<advance::KagemushaWalletStdFsV1, P, S>,
    binding: Option<super::installed::BoundOriginals>,
) -> std::result::Result<u64, NativeStartupFailure<P, S>>
where
    P: advance::KagemushaWalletPlatformV1 + 'static,
    S: OriginalSourceV1 + Send + 'static,
{
    let mut registry = match registry().lock() {
        Ok(registry) => registry,
        Err(_) => {
            return Err(NativeStartupFailure::Registration {
                runtime,
                failure: Failure::code(INTERNAL),
            });
        }
    };
    if registry.owners.len() + registry.runtimes.len() >= MAX_OWNERS {
        return Err(NativeStartupFailure::Registration {
            runtime,
            failure: Failure::code(RESOURCE),
        });
    }
    let Some(id) = registry
        .next
        .checked_add(1)
        .filter(|id| *id <= i64::MAX as u64)
    else {
        return Err(NativeStartupFailure::Registration {
            runtime,
            failure: Failure::code(RESOURCE),
        });
    };
    registry.next = id;
    registry.runtimes.insert(
        id,
        Arc::new(RuntimeOwner {
            closing: Arc::new(closing::CloseState::default()),
            admission: Mutex::new(Some(Box::new(Runtime {
                phase: Some(Phase::Ready(runtime)),
                binding,
            }))),
            finished: Mutex::new(None),
        }),
    );
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_intake_is_bounded_and_never_accepts_foreign_identity_digests_as_authority() {
        assert!(validate([&[1], &[2], &[3], &[4]]).is_ok());
        for role in 0..4 {
            let oversized = vec![0; BOUNDS[role] + 1];
            let mut originals: [&[u8]; 4] = [&[1]; 4];
            originals[role] = &oversized;
            assert!(validate(originals).is_err());
            originals[role] = &[];
            assert!(validate(originals).is_err());
        }
        assert_eq!(
            begin(u64::MAX, [&[1]; 4]).unwrap_err().status,
            ARTIFACTS_UNAVAILABLE
        );
    }
    use std::sync::{
        Barrier,
        atomic::{AtomicUsize, Ordering},
    };
    // Boundary ownership fixture only: this has no installed artifact/source capability.
    struct TestAdmission {
        wallet: Option<Box<dyn Wallet>>,
        calls: Arc<AtomicUsize>,
        barriers: Option<(Arc<Barrier>, Arc<Barrier>)>,
    }
    impl Admission for TestAdmission {
        fn enrollment(&mut self, _: enrollment::Input<'_>) -> Result<enrollment::Response> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if let Some((entered, resume)) = &self.barriers {
                entered.wait();
                resume.wait();
            }
            Ok(enrollment::Response {
                kind: 1,
                ..enrollment::Response::default()
            })
        }
        fn begin(&mut self, _: [&[u8]; 4]) -> Result<Vec<u8>> {
            Err(Failure::code(INVALID))
        }
        fn cancel(&mut self) -> Result<()> {
            Err(Failure::code(INVALID))
        }
        fn finish(&mut self, _: &[u8]) -> Result<(Box<dyn Wallet>, state::Scheduler)> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if let Some((entered, resume)) = &self.barriers {
                entered.wait();
                resume.wait();
            }
            Ok((
                self.wallet.take().ok_or(Failure::code(INVALID))?,
                state::Scheduler::new(),
            ))
        }
    }
    fn local_runtime(
        barriers: Option<(Arc<Barrier>, Arc<Barrier>)>,
    ) -> (
        Mutex<Registry>,
        Arc<RuntimeOwner>,
        Arc<AtomicUsize>,
        Arc<AtomicUsize>,
    ) {
        let calls = Arc::new(AtomicUsize::new(0));
        let drops = Arc::new(AtomicUsize::new(0));
        let owner = Arc::new(RuntimeOwner {
            closing: Arc::new(closing::CloseState::default()),
            admission: Mutex::new(Some(Box::new(TestAdmission {
                wallet: Some(Box::new(super::super::tests::TestWallet {
                    calls: Arc::new(AtomicUsize::new(0)),
                    drops: drops.clone(),
                    expected_request: None,
                })),
                calls: calls.clone(),
                barriers,
            }))),
            finished: Mutex::new(None),
        });
        let mut registry = Registry::default();
        registry.runtimes.insert(1, Arc::clone(&owner));
        (Mutex::new(registry), owner, calls, drops)
    }
    #[test]
    fn registry_failure_retains_admitted_custody_and_retry_never_reauthorizes() {
        let (registry, owner, calls, drops) = local_runtime(None);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = registry.lock().unwrap();
            panic!("injected local registry poison");
        }));
        assert_eq!(
            finish_with(&registry, owner.clone(), 1, &[1; 64])
                .unwrap_err()
                .status,
            INTERNAL
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert!(owner.finished.lock().unwrap().is_some());
        registry.clear_poison();
        let result = finish_with(&registry, owner.clone(), 1, &[]).unwrap();
        assert_eq!((result.kind, result.sequence), (16, 1));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        // Lose the successful response and retry the retained runtime capability.
        let replay = finish_with(&registry, owner.clone(), 1, &[]).unwrap();
        assert_eq!((replay.kind, replay.sequence), (16, 1));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let selected = registry.lock().unwrap().owners.remove(&1).unwrap();
        drop(selected.wallet.lock().unwrap().take());
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(
            finish_with(&registry, owner, 1, &[]).unwrap_err().status,
            CLOSED
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn concurrent_close_cannot_resurrect_owner_after_account_finish() {
        let entered = Arc::new(Barrier::new(2));
        let resume = Arc::new(Barrier::new(2));
        let (registry, owner, calls, drops) =
            local_runtime(Some((entered.clone(), resume.clone())));
        let registry = Arc::new(registry);
        let finishing = {
            let registry = registry.clone();
            let owner = owner.clone();
            std::thread::spawn(move || finish_with(&registry, owner, 1, &[1; 64]))
        };
        entered.wait();
        let close_registry = Arc::clone(&registry);
        let closing = std::thread::spawn(move || closing::close_with(&close_registry, 1));
        let timeout = std::time::Instant::now();
        while owner.closing.require_open().is_ok() {
            assert!(timeout.elapsed() < std::time::Duration::from_secs(5));
            std::thread::yield_now();
        }
        resume.wait();
        assert_eq!(finishing.join().unwrap().unwrap_err().status, CLOSED);
        closing.join().unwrap().unwrap();
        assert!(registry.lock().unwrap().owners.is_empty());
        assert!(owner.finished.lock().unwrap().is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn close_refusal_retains_same_runtime_and_both_custody_owners_for_retry() {
        let (registry, owner, calls, drops) = local_runtime(None);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = owner.finished.lock().unwrap();
            panic!("injected retained-result lock refusal");
        }));
        for _ in 0..2 {
            assert_eq!(
                closing::close_with(&registry, 1).unwrap_err().status,
                INTERNAL
            );
            assert!(
                registry
                    .lock()
                    .unwrap()
                    .runtimes
                    .get(&1)
                    .is_some_and(|actual| Arc::ptr_eq(actual, &owner))
            );
            assert!(owner.admission.lock().unwrap().is_some());
            assert_eq!(drops.load(Ordering::SeqCst), 0);
            assert_eq!(
                finish_with(&registry, Arc::clone(&owner), 1, &[1; 64])
                    .unwrap_err()
                    .status,
                CLOSED
            );
        }
        owner.finished.clear_poison(); // Test-only repair: production never invents cleanup success.
        closing::close_with(&registry, 1).unwrap();
        assert!(registry.lock().unwrap().runtimes.is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
    fn enrollment_input() -> enrollment::Input<'static> {
        enrollment::Input {
            selector: 0,
            slot: &[],
            challenge: &[1],
            policy: &[1],
            account: &[1],
            original: &[],
            certificates: &[],
        }
    }
    #[test]
    fn enrollment_captured_before_close_refuses_after_the_actual_admission_lock() {
        let (registry, owner, calls, drops) = local_runtime(None);
        let registry = Arc::new(registry);
        let held = owner.admission.lock().unwrap();
        let (captured_send, captured) = std::sync::mpsc::channel();
        let enrolling = {
            let owner = Arc::clone(&owner);
            std::thread::spawn(move || {
                captured_send.send(()).unwrap();
                enroll_with(owner, enrollment_input())
            })
        };
        captured.recv().unwrap();
        let closing = {
            let registry = Arc::clone(&registry);
            std::thread::spawn(move || closing::close_with(&registry, 1))
        };
        let timeout = std::time::Instant::now();
        while owner.closing.require_open().is_ok() {
            assert!(timeout.elapsed() < std::time::Duration::from_secs(5));
            std::thread::yield_now();
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(held);
        assert_eq!(enrolling.join().unwrap().err().unwrap().status, CLOSED);
        closing.join().unwrap().unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(registry.lock().unwrap().runtimes.is_empty());
    }
    #[test]
    fn close_joins_an_accepted_enrollment_before_releasing_the_same_actual_owner() {
        let entered = Arc::new(Barrier::new(2));
        let resume = Arc::new(Barrier::new(2));
        let (registry, owner, calls, drops) =
            local_runtime(Some((entered.clone(), resume.clone())));
        let registry = Arc::new(registry);
        let enrolling = {
            let owner = Arc::clone(&owner);
            std::thread::spawn(move || enroll_with(owner, enrollment_input()))
        };
        entered.wait();
        let closing = {
            let registry = Arc::clone(&registry);
            std::thread::spawn(move || closing::close_with(&registry, 1))
        };
        let timeout = std::time::Instant::now();
        while owner.closing.require_open().is_ok() {
            assert!(timeout.elapsed() < std::time::Duration::from_secs(5));
            std::thread::yield_now();
        }
        assert!(
            registry
                .lock()
                .unwrap()
                .runtimes
                .get(&1)
                .is_some_and(|active| Arc::ptr_eq(active, &owner))
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        resume.wait();
        assert_eq!(enrolling.join().unwrap().unwrap().kind, 1);
        closing.join().unwrap().unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(registry.lock().unwrap().runtimes.is_empty());
    }
}
