//! Authenticated experimental backend on the installed runtime's exclusive provider.
use super::*;
use iroha_core_zk::kagemusha_spartan_native::{FutureRuntime, native};

mod session_state;
#[cfg(feature = "spartan-device-fixture")]
pub mod device_fixture;
use session_state::{InstalledOriginals, ReviewCache};

pub(crate) enum Action<'a> {
    Install {
        bundle: &'a [u8],
        manifest: &'a [u8],
        signature: &'a [u8; 64],
    },
    Prepare {
        slot: [u8; 32],
        witness: &'a [u8],
        head: &'a [u8],
    },
    BeginAccount([&'a [u8]; 4]),
    AuthorizeAccount(&'a [u8]),
    Execute(u64),
    Output {
        slot: [u8; 32],
        operation: [u8; 32],
        part: u32,
    },
    VerifyPayment(&'a [u8]),
    Discard(u64),
}
impl From<native::Error> for Failure {
    fn from(error: native::Error) -> Self {
        Self::code(match error {
            native::Error::ResourceLimit => RESOURCE,
            native::Error::NotEnoughMemory { .. } => NOT_ENOUGH_MEMORY,
            native::Error::Unavailable(_) => UNAVAILABLE,
            native::Error::Rejected(_) => PROOF_REJECTED,
            native::Error::AuthorityMismatch(_) | native::Error::Malformed(_) => INVALID,
        })
    }
}
pub(super) trait FutureSession: Send {
    fn call(&mut self, action: Action<'_>) -> Result<Response>;
}
struct Review {
    slot: advance::KagemushaWalletSlotIdV1,
    capsule: native::RecoveryCapsule,
    digest: [u8; 32],
    bytes: Vec<u8>,
}
pub(super) struct Session<P, S> {
    runtime: FutureRuntime<advance::KagemushaWalletStdFsV1, P, S>,
    installed: InstalledOriginals,
    reviews: ReviewCache<Review>,
}
impl<P: advance::KagemushaWalletPlatformV1 + 'static, S> Session<P, S> {
    pub(super) fn new(
        runtime: FutureRuntime<advance::KagemushaWalletStdFsV1, P, S>,
        bundle: &[u8],
        manifest: &[u8],
        signature: &[u8; 64],
    ) -> Self {
        let installed =
            InstalledOriginals::authenticated(*runtime.parameter_id(), bundle, manifest, signature);
        Self {
            runtime,
            installed,
            reviews: ReviewCache::default(),
        }
    }
}
fn response(kind: i32, bytes: Vec<u8>) -> Response {
    Response {
        kind,
        bytes,
        ..Response::default()
    }
}
impl<P: advance::KagemushaWalletPlatformV1 + 'static, S: Send> FutureSession for Session<P, S> {
    fn call(&mut self, action: Action<'_>) -> Result<Response> {
        match action {
            Action::Install {
                bundle,
                manifest,
                signature,
            } => self.installed.retry(bundle, manifest, signature),
            Action::Prepare {
                slot,
                witness,
                head,
            } => {
                let slot = advance::KagemushaWalletSlotIdV1(slot);
                let prepared = self.runtime.prepare(&slot, witness, head)?;
                if let Some((token, old)) = self
                    .reviews
                    .find(|r| r.slot == slot && &r.digest == prepared.capsule_digest())
                {
                    return Ok(Response {
                        kind: 82,
                        sequence: u128::from(token),
                        bytes: old.bytes.clone(),
                        ..Response::default()
                    });
                }
                let bytes = prepared.review_bytes()?;
                let token = self.reviews.insert(Review {
                    slot,
                    capsule: prepared.capsule().clone(),
                    digest: *prepared.capsule_digest(),
                    bytes: bytes.clone(),
                })?;
                Ok(Response {
                    kind: 82,
                    sequence: u128::from(token),
                    bytes,
                    ..Response::default()
                })
            }
            Action::BeginAccount([credential, certificate, account, asset]) => {
                let challenge =
                    self.runtime
                        .begin_account(credential, certificate, account, asset)?;
                Ok(response(15, challenge.to_vec()))
            }
            Action::AuthorizeAccount(signature) => {
                let slot = self.runtime.authorize_account(signature)?;
                let mut bytes = self.runtime.parameter_id().to_vec();
                bytes.extend_from_slice(&slot);
                Ok(response(85, bytes))
            }
            Action::Execute(token) => {
                let runtime = &mut self.runtime;
                self.reviews.execute(token, |review| {
                    let prepared = runtime.resume(&review.capsule)?;
                    let result = runtime.advance(&review.slot, prepared)?;
                    use advance::KagemushaWalletAdvanceOutcomeV1 as O;
                    Ok(match result {
                        O::Released { retained, .. } => response(1, retained.record.encode()?),
                        O::Pending { .. } => response(2, Vec::new()),
                        O::NotPerformed(reason) => Response {
                            kind: 3,
                            detail: match reason {
                                advance::KagemushaWalletNotPerformedV1::StaleHead => 0,
                                advance::KagemushaWalletNotPerformedV1::CapacityWait => 1,
                                advance::KagemushaWalletNotPerformedV1::Invalid { .. } => 2,
                            },
                            ..Response::default()
                        },
                        O::Archived(_) => response(4, Vec::new()),
                        O::DeliveryDataLoss { .. } => response(5, Vec::new()),
                    })
                })
            }
            Action::Output {
                slot,
                operation,
                part,
            } => {
                if part > 2 {
                    return Err(Failure::code(INVALID));
                }
                use advance::KagemushaWalletLookupV1 as L;
                Ok(
                    match self
                        .runtime
                        .lookup(&advance::KagemushaWalletSlotIdV1(slot), &operation)?
                    {
                        L::Retained(retained) => response(1, retained.record.output_part(part)?),
                        L::SelectedUnsigned { .. } => response(2, Vec::new()),
                        L::Unknown => response(0, Vec::new()),
                        L::Archived(_) => response(4, Vec::new()),
                        L::DeliveryDataLoss => response(5, Vec::new()),
                    },
                )
            }
            Action::VerifyPayment(bytes) => {
                let payment = self.runtime.verify_payment(bytes)?;
                let mut projection = Vec::with_capacity(514);
                projection.extend_from_slice(&payment.digest_bytes());
                projection.extend_from_slice(&payment.request().body);
                projection.extend_from_slice(&payment.request().signature);
                Ok(response(83, projection))
            }
            Action::Discard(token) => {
                // Release volatile preparation only; durable custody stays with the provider.
                self.reviews.discard(token);
                Ok(response(84, Vec::new()))
            }
        }
    }
}
pub(crate) fn call(handle: u64, action: Action<'_>) -> Result<Response> {
    let owner = open::runtime(handle)?;
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
    let admission = guard.as_deref_mut().ok_or(Failure::code(CLOSED))?;
    let mut result = custody_transaction(|| admission.spartan(action))?;
    if matches!(result.kind, 15 | 81 | 85) {
        result.sequence = u128::from(handle);
    }
    Ok(result)
}
unsafe fn fixed32(pointer: *const u8) -> Result<[u8; 32]> {
    unsafe { exports::input(pointer, 32, 32) }?
        .try_into()
        .map_err(|_| Failure::code(INVALID))
}
/// Install the eight root-authenticated public verifier programs on this runtime.
/// # Safety
/// Inputs address their stated lengths; `out` is writable and aligned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_install_v1(
    handle: u64,
    bundle: *const u8,
    bundle_len: usize,
    manifest: *const u8,
    manifest_len: usize,
    signature: *const u8,
    signature_len: usize,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        exports::output(out, || {
            call(
                handle,
                Action::Install {
                    bundle: exports::input(bundle, bundle_len, native::MAX_PROGRAM_BUNDLE_BYTES)?,
                    manifest: exports::input(manifest, manifest_len, 16384)?,
                    signature: exports::input(signature, signature_len, 64)?
                        .try_into()
                        .map_err(|_| Failure::code(INVALID))?,
                },
            )
        })
    }
}
/// Freeze a checked operation and return its native review token and projection.
/// # Safety
/// `slot` addresses 32 bytes; other inputs address stated lengths; `out` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_prepare_v1(
    handle: u64,
    slot: *const u8,
    witness: *const u8,
    witness_len: usize,
    head: *const u8,
    head_len: usize,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        exports::output(out, || {
            call(
                handle,
                Action::Prepare {
                    slot: fixed32(slot)?,
                    witness: exports::input(witness, witness_len, native::MAX_RECOVERY_BYTES)?,
                    head: exports::input(head, head_len, native::MAX_OUTPUT_BYTES)?,
                },
            )
        })
    }
}
/// Execute or resume the exact reviewed capsule through hardware custody.
/// # Safety
/// `out` is writable and aligned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_execute_v1(
    handle: u64,
    token: u64,
    out: *mut WalletResult,
) -> i32 {
    unsafe { exports::output(out, || call(handle, Action::Execute(token))) }
}
/// Reconcile and return exact retained head (0), Payment (1), or completion (2).
/// # Safety
/// `slot` and `operation` address 32 bytes each; `out` is writable and aligned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_output_v1(
    handle: u64,
    slot: *const u8,
    operation: *const u8,
    part: u32,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        exports::output(out, || {
            call(
                handle,
                Action::Output {
                    slot: fixed32(slot)?,
                    operation: fixed32(operation)?,
                    part,
                },
            )
        })
    }
}
/// Verify complete Payment and return its authenticated request projection.
/// # Safety
/// Input addresses its stated length; `out` is writable and aligned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_verify_payment_v1(
    handle: u64,
    bytes: *const u8,
    length: usize,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        exports::output(out, || {
            call(
                handle,
                Action::VerifyPayment(exports::input(bytes, length, native::MAX_OUTPUT_BYTES)?),
            )
        })
    }
}
/// Discard volatile preparation without deleting selected or released custody.
/// # Safety
/// `out` is writable and aligned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_discard_v1(
    handle: u64,
    token: u64,
    out: *mut WalletResult,
) -> i32 {
    unsafe { exports::output(out, || call(handle, Action::Discard(token))) }
}

/// Begin actual account admission from exact original credential, certificate,
/// canonical account and asset scope. Native derives the custody slot and head.
/// # Safety
/// Inputs address stated lengths and `out` is writable and aligned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_begin_account_v1(
    handle: u64,
    request: *const WalletOpenRequest,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        exports::output(out, || {
            let request = request.as_ref().ok_or(Failure::code(INVALID))?;
            call(
                handle,
                Action::BeginAccount([
                    exports::input(request.credential, request.credential_length, 596)?,
                    exports::input(request.certificates, request.certificates_length, 172)?,
                    exports::input(request.account, request.account_length, open::BOUNDS[2])?,
                    exports::input(request.asset, request.asset_length, open::BOUNDS[3])?,
                ]),
            )
        })
    }
}

/// Complete the same retained account challenge; no caller acceptance flag.
/// # Safety
/// Signature addresses its stated length; `out` is writable and aligned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_authorize_account_v1(
    handle: u64,
    signature: *const u8,
    length: usize,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        exports::output(out, || {
            call(
                handle,
                Action::AuthorizeAccount(exports::input(signature, length, 4096)?),
            )
        })
    }
}
