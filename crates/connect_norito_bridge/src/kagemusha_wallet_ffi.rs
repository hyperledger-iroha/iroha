//! Exclusive mobile ownership of the shared KAGEMUSHA state machine.
//!
//! The C/JNI boundary carries canonical Norito objects and exact retained output bytes.
//! Only the Rust artifact owner can install a coordinator; a caller cannot supply a proof
//! verdict or an arbitrary-sign request. Foreign open currently reports `ARTIFACTS_UNAVAILABLE`
//! because the authenticated operation/Λ/Ω loader is not yet available.
// TODO(G3/G4): connect the authenticated artifact loader to foreign open. Do not replace it
// with structural verification, a caller-provided verdict, or a software payment key.

use iroha_core_zk::kagemusha_wallet_artifacts_v1::producer_inventory::OriginalSourceV1;
use iroha_core_zk::{kagemusha_wallet_advance_v1 as advance, kagemusha_wallet_state_v1 as state};
use iroha_data_model::kagemusha::*;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock},
};

mod platform;
pub use platform::{CallbackPlatform, PlatformCallbacks, PlatformReply};
#[cfg(any(target_os = "android", target_os = "linux", target_os = "macos"))]
mod android;
#[cfg(any(target_os = "android", target_os = "linux", target_os = "macos"))]
pub use android::AndroidPlatform;
mod exports;
pub(crate) mod requests;
pub(crate) mod setup;
pub use exports::*;
#[cfg(test)]
mod tests;

/// Invalid input or malformed callback result.
pub const INVALID: i32 = -1;
/// Closed, closing, or unknown opaque handle.
pub const CLOSED: i32 = -2;
/// Owner/handle capacity exhausted.
pub const RESOURCE: i32 = -3;
/// Required authenticated native proof artifacts are unavailable.
pub const ARTIFACTS_UNAVAILABLE: i32 = -4;
/// Storage or the key store gave no definitive answer.
pub const UNAVAILABLE: i32 = -5;
/// A publication may have committed; reconciliation is required.
pub const UNCERTAIN: i32 = -6;
/// Source-bound custody or required fold witnesses were lost.
pub const CUSTODY_LOST: i32 = -7;
/// Current head must be folded before this operation.
pub const FOLD_REQUIRED: i32 = -8;
/// Native proof verification failed.
pub const PROOF_REJECTED: i32 = -9;
/// Cooperative cancellation completed, with durable state retained.
pub const CANCELLED: i32 = -10;
/// An existing operation/credit identity has different inputs.
pub const CONFLICT: i32 = -11;
/// Insufficient durable storage.
pub const NO_SPACE: i32 = -12;
/// Custody is terminal or the payment key has been lost.
pub const TERMINAL: i32 = -13;
/// Internal panic or poisoned owner; no outcome should be inferred.
pub const INTERNAL: i32 = -100;

const MAX_OWNERS: usize = 32;

/// Bridge failure. `reason` preserves platform tri-state errors instead of calling them absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Failure {
    /// One negative bridge status above.
    pub status: i32,
    /// Platform reason: 0 locked, 1 before first unlock, 2 busy, 3 I/O, 4 platform,
    /// 5 unusable key, 6 permanently invalidated, or -1 when inapplicable.
    pub reason: i32,
    /// Original OS/platform error code; zero when inapplicable.
    pub platform_code: i32,
}
impl Failure {
    pub(crate) fn code(status: i32) -> Self {
        Self {
            status,
            reason: -1,
            platform_code: 0,
        }
    }
    fn unavailable(status: i32, value: advance::KagemushaWalletUnavailableV1) -> Self {
        use advance::KagemushaWalletUnavailableV1 as U;
        let (reason, platform_code) = match value {
            U::Locked => (0, 0),
            U::BeforeFirstUnlock => (1, 0),
            U::Busy => (2, 0),
            U::Io(code) => (3, code),
            U::Platform(code) => (4, code),
            U::KeyUnusable => (5, 0),
            U::PermanentlyInvalidated => (6, 0),
        };
        Self {
            status,
            reason,
            platform_code,
        }
    }
}
impl From<advance::KagemushaWalletProviderErrorV1> for Failure {
    fn from(value: advance::KagemushaWalletProviderErrorV1) -> Self {
        use advance::KagemushaWalletProviderErrorV1 as E;
        match value {
            E::Unavailable(reason) => Self::unavailable(UNAVAILABLE, reason),
            E::Uncertain(reason) => Self::unavailable(UNCERTAIN, reason),
            E::NoSpace => Self::code(NO_SPACE),
            E::NoReplaceUnsupported | E::Invalid { .. } => Self::code(INVALID),
            E::UnavailableCustodyData { .. } | E::LostCustody(_) | E::UnexpectedEntry { .. } => {
                Self::code(CUSTODY_LOST)
            }
            E::KeyLost | E::Terminal => Self::code(TERMINAL),
            E::OperationIdConflict { .. } => Self::code(CONFLICT),
        }
    }
}
impl From<state::Error> for Failure {
    fn from(value: state::Error) -> Self {
        use state::Error as E;
        match value {
            E::Provider(error) => error.into(),
            E::Storage(error) => Self::unavailable(
                UNAVAILABLE,
                advance::KagemushaWalletUnavailableV1::from_io(&error),
            ),
            E::Invalid(_) | E::Collected => Self::code(INVALID),
            E::WitnessLost(_) => Self::code(CUSTODY_LOST),
            E::CreditConflict | E::OperationConflict => Self::code(CONFLICT),
            E::FoldRequired => Self::code(FOLD_REQUIRED),
            E::Pending => Self::code(UNCERTAIN),
            E::NoHead => Self::code(INVALID),
            E::Cancelled => Self::code(CANCELLED),
            E::ArtifactsUnavailable(_) => Self::code(ARTIFACTS_UNAVAILABLE),
            E::Proof(_) => Self::code(PROOF_REJECTED),
        }
    }
}
pub(crate) type Result<T> = std::result::Result<T, Failure>;

#[derive(Debug, Default)]
pub(crate) struct Response {
    // 0 unknown, 1 complete, 2 pending, 3 not performed, 4 archived, 5 delivery loss;
    // 6 idle, 7 caught up, 8 checkpoint, 9 folded, 10 CreditStatus, 11 preparing,
    // 12 exact setup original, 13 time challenge (sequence=token, bytes=nonce32), 14 time retained.
    pub(crate) kind: i32,
    pub(crate) sequence: u128,
    pub(crate) detail: u32,
    pub(crate) bytes: Vec<u8>,
}
fn completion(value: Option<state::Completion>) -> Response {
    use advance::KagemushaWalletNotPerformedV1 as N;
    use state::Completion as C;
    match value {
        None => Response::default(),
        Some(C::Complete(bytes)) => Response {
            kind: 1,
            bytes,
            ..Response::default()
        },
        Some(C::CreditStatus(bytes)) => Response {
            kind: 10,
            bytes,
            ..Response::default()
        },
        Some(C::Pending) => Response {
            kind: 2,
            ..Response::default()
        },
        Some(C::NotPerformed(reason)) => Response {
            kind: 3,
            detail: match reason {
                N::StaleHead => 0,
                N::CapacityWait => 1,
                N::Invalid { .. } => 2,
            },
            ..Response::default()
        },
        Some(C::Archived) => Response {
            kind: 4,
            ..Response::default()
        },
        Some(C::DeliveryDataLoss) => Response {
            kind: 5,
            ..Response::default()
        },
    }
}
trait Wallet: Send {
    fn snapshot(&mut self) -> Result<state::Snapshot>;
    fn setup(&mut self, input: setup::Setup) -> Result<Response>;
    fn execute(&mut self, request: state::OperationRequestV1) -> Result<Response>;
    fn request_status(&mut self, request: &[u8; 32]) -> Result<Response>;
    fn retry(&mut self, operation: &[u8; 32]) -> Result<Response>;
    fn resume(&mut self) -> Result<Response>;
    fn fold(&mut self) -> Result<Response>;
    fn credit(&mut self, credit: &[u8; 32], payment: &[u8; 32]) -> Result<Response>;
}
struct NativeWallet<P: advance::KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send> {
    wallet: state::Coordinator<
        state::AdvanceHandle<advance::KagemushaWalletStdFsV1, P>,
        state::ProviderArchive<advance::KagemushaWalletStdFsV1, P>,
        state::NativeWalletProofsV1<advance::KagemushaWalletStdFsV1, P, S>,
    >,
    times: BTreeMap<u64, state::DirectTimeExchangeV1>,
    next_time: u64,
}
impl<P: advance::KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send> Wallet
    for NativeWallet<P, S>
{
    fn setup(&mut self, input: setup::Setup) -> Result<Response> {
        self.setup_inner(input)
    }
    fn snapshot(&mut self) -> Result<state::Snapshot> {
        Ok(self.wallet.snapshot()?)
    }
    fn execute(&mut self, request: state::OperationRequestV1) -> Result<Response> {
        Ok(completion(Some(self.wallet.execute(request)?)))
    }
    fn request_status(&mut self, request: &[u8; 32]) -> Result<Response> {
        Ok(match self.wallet.retry_request(request)? {
            state::RequestStatusV1::Unknown => Response::default(),
            state::RequestStatusV1::Preparing => Response {
                kind: 11,
                ..Response::default()
            },
            state::RequestStatusV1::Outcome(value) => completion(Some(value)),
        })
    }
    fn retry(&mut self, operation: &[u8; 32]) -> Result<Response> {
        Ok(completion(self.wallet.retry(operation)?))
    }
    fn resume(&mut self) -> Result<Response> {
        Ok(completion(self.wallet.resume()?))
    }
    fn fold(&mut self) -> Result<Response> {
        use state::FoldStatus as F;
        Ok(match self.wallet.fold_once()? {
            F::Idle => Response {
                kind: 6,
                ..Response::default()
            },
            F::CaughtUp => Response {
                kind: 7,
                ..Response::default()
            },
            F::Checkpoint { sequence, ordinal } => Response {
                kind: 8,
                sequence,
                detail: ordinal,
                ..Response::default()
            },
            F::Folded(sequence) => Response {
                kind: 9,
                sequence,
                ..Response::default()
            },
        })
    }
    fn credit(&mut self, credit: &[u8; 32], payment: &[u8; 32]) -> Result<Response> {
        let value = self.wallet.credit_status(credit, payment)?;
        let bytes = norito::encode_canonical(&value).map_err(|_| Failure::code(INTERNAL))?;
        Ok(Response {
            kind: 10,
            bytes,
            ..Response::default()
        })
    }
}
struct Owner {
    scheduler: state::Scheduler,
    // None is the closing linearization point. Lookups captured before close cannot perform
    // another operation once the current call returns. Drop releases exclusive filesystem custody.
    wallet: Mutex<Option<Box<dyn Wallet>>>,
}
#[derive(Default)]
struct Registry {
    next: u64,
    owners: BTreeMap<u64, Arc<Owner>>,
}
fn registry() -> &'static Mutex<Registry> {
    static VALUE: OnceLock<Mutex<Registry>> = OnceLock::new();
    VALUE.get_or_init(|| Mutex::new(Registry::default()))
}
fn install(wallet: Box<dyn Wallet>, scheduler: state::Scheduler) -> Result<u64> {
    let mut registry = registry().lock().map_err(|_| Failure::code(INTERNAL))?;
    if registry.owners.len() >= MAX_OWNERS {
        return Err(Failure::code(RESOURCE));
    }
    let id = registry
        .next
        .checked_add(1)
        .filter(|id| *id <= i64::MAX as u64)
        .ok_or(Failure::code(RESOURCE))?;
    registry.next = id;
    registry.owners.insert(
        id,
        Arc::new(Owner {
            scheduler,
            wallet: Mutex::new(Some(wallet)),
        }),
    );
    Ok(id)
}
/// Transfer a fully constructed, authenticated native coordinator into one opaque handle.
///
/// Only the Rust artifact loader uses this entry. Construction requires the real `NativeProofs`
/// implementation and source-bound `AdvanceHandle`/`ProviderArchive`; foreign callbacks cannot
/// implement or replace proof verification. The bridge never constructs a software custody path.
///
/// # Errors
/// Returns `RESOURCE` for exhausted handles/capacity or `INTERNAL` for a poisoned registry.
pub fn retain_native_owner<P, S>(
    wallet: state::Coordinator<
        state::AdvanceHandle<advance::KagemushaWalletStdFsV1, P>,
        state::ProviderArchive<advance::KagemushaWalletStdFsV1, P>,
        state::NativeWalletProofsV1<advance::KagemushaWalletStdFsV1, P, S>,
    >,
) -> Result<u64>
where
    P: advance::KagemushaWalletPlatformV1 + 'static,
    S: OriginalSourceV1 + Send + 'static,
{
    let scheduler = wallet.scheduler();
    install(
        Box::new(NativeWallet {
            wallet,
            times: BTreeMap::new(),
            next_time: 0,
        }),
        scheduler,
    )
}
fn owner(id: u64) -> Result<Arc<Owner>> {
    registry()
        .lock()
        .map_err(|_| Failure::code(INTERNAL))?
        .owners
        .get(&id)
        .cloned()
        .ok_or(Failure::code(CLOSED))
}
pub(crate) fn close(id: u64) -> Result<()> {
    let owner = registry()
        .lock()
        .map_err(|_| Failure::code(INTERNAL))?
        .owners
        .remove(&id)
        .ok_or(Failure::code(CLOSED))?;
    owner.scheduler.set_activity(false, false);
    let _priority = owner.scheduler.payment();
    let wallet = owner
        .wallet
        .lock()
        .map_err(|_| Failure::code(INTERNAL))?
        .take();
    drop(wallet);
    Ok(())
}
pub(crate) fn activity(id: u64, foreground: bool, charging: bool) -> Result<()> {
    let owner = owner(id)?;
    // Do not acquire the wallet mutex: a running fold needs this cancellation signal.
    owner.scheduler.set_activity(foreground, charging);
    Ok(())
}
fn with_wallet<T>(
    id: u64,
    payment: bool,
    action: impl FnOnce(&mut dyn Wallet) -> Result<T>,
) -> Result<T> {
    let owner = owner(id)?;
    // Signal and join before the owner lock. Reversing these locks deadlocks against proving.
    let _priority = payment.then(|| owner.scheduler.payment());
    let mut guard = owner.wallet.lock().map_err(|_| Failure::code(INTERNAL))?;
    action(guard.as_deref_mut().ok_or(Failure::code(CLOSED))?)
}
pub(crate) fn snapshot(id: u64) -> Result<state::Snapshot> {
    // A view does not cancel a useful background fold. Call off the UI thread.
    with_wallet(id, false, |wallet| wallet.snapshot())
}
pub(crate) fn setup(id: u64, input: setup::Setup) -> Result<Response> {
    with_wallet(id, true, |wallet| wallet.setup(input))
}
pub(crate) fn execute(id: u64, request: state::OperationRequestV1) -> Result<Response> {
    with_wallet(id, true, |wallet| wallet.execute(request))
}
pub(crate) fn request_status(id: u64, request: &[u8]) -> Result<Response> {
    let request: &[u8; 32] = request.try_into().map_err(|_| Failure::code(INVALID))?;
    if *request == [0; 32] {
        return Err(Failure::code(INVALID));
    }
    with_wallet(id, true, |wallet| wallet.request_status(request))
}
pub(crate) fn retry(id: u64, operation: &[u8]) -> Result<Response> {
    let operation = operation.try_into().map_err(|_| Failure::code(INVALID))?;
    with_wallet(id, true, |wallet| wallet.retry(operation))
}
pub(crate) fn resume(id: u64) -> Result<Response> {
    with_wallet(id, true, |wallet| wallet.resume())
}
pub(crate) fn fold(id: u64) -> Result<Response> {
    with_wallet(id, false, |wallet| wallet.fold())
}
pub(crate) fn credit(id: u64, credit: &[u8], payment: &[u8]) -> Result<Response> {
    let credit = credit.try_into().map_err(|_| Failure::code(INVALID))?;
    let payment = payment.try_into().map_err(|_| Failure::code(INVALID))?;
    with_wallet(id, true, |wallet| wallet.credit(credit, payment))
}
pub(crate) fn run<T>(action: impl FnOnce() -> Result<T>) -> Result<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(action))
        .unwrap_or(Err(Failure::code(INTERNAL)))
}
