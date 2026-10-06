//! C ABI, with initialized outputs and panic containment at each boundary.

use super::*;

/// Typed call result. `bytes` uses the existing `connect_norito_free` allocator contract.
#[repr(C)]
#[derive(Debug)]
pub struct WalletResult {
    /// 0 unknown, 1 complete, 2 pending, 3 not performed, 4 archived, 5 delivery loss,
    /// 6 idle, 7 caught up, 8 checkpoint, 9 folded, 10 CreditStatus; negative is failure.
    pub status: i32,
    /// Failure platform reason or -1. Never conflate `UNAVAILABLE` with unknown/absent.
    pub reason: i32,
    /// OS/platform code for a failure.
    pub platform_code: i32,
    /// Low 64 bits of a checkpoint/fold sequence.
    pub sequence_low: u64,
    /// High 64 bits of the sequence.
    pub sequence_high: u64,
    /// Checkpoint ordinal; for not-performed: 0 stale head, 1 capacity, 2 invalid.
    pub detail: u32,
    /// Canonical result bytes; null for an empty result. Release with `connect_norito_free`.
    pub bytes: *mut u8,
    /// Exact initialized bytes length.
    pub length: usize,
}
impl Default for WalletResult {
    fn default() -> Self {
        Self {
            status: INTERNAL,
            reason: -1,
            platform_code: 0,
            sequence_low: 0,
            sequence_high: 0,
            detail: 0,
            bytes: std::ptr::null_mut(),
            length: 0,
        }
    }
}
unsafe fn output(out: *mut WalletResult, action: impl FnOnce() -> Result<Response>) -> i32 {
    if out.is_null() {
        return INVALID;
    }
    // SAFETY: caller provides writable aligned result memory.
    unsafe { out.write(WalletResult::default()) };
    let result = run(action);
    let value = match result {
        Ok(response) => {
            let mut value = WalletResult {
                status: response.kind,
                sequence_low: response.sequence as u64,
                sequence_high: (response.sequence >> 64) as u64,
                detail: response.detail,
                ..WalletResult::default()
            };
            if !response.bytes.is_empty() {
                let mut length = 0;
                // SAFETY: valid local outputs; allocator is paired with bridge free.
                if unsafe { crate::write_bytes(&mut value.bytes, &mut length, &response.bytes) }
                    .is_err()
                {
                    return RESOURCE;
                }
                value.length = length as usize;
            }
            value
        }
        Err(error) => WalletResult {
            status: error.status,
            reason: error.reason,
            platform_code: error.platform_code,
            ..WalletResult::default()
        },
    };
    let status = if value.status < 0 { value.status } else { 0 };
    // SAFETY: admitted writable result memory; no previous owned output exists here.
    unsafe { out.write(value) };
    status
}
unsafe fn input<'a>(pointer: *const u8, length: usize, bound: usize) -> Result<&'a [u8]> {
    if length > bound || (pointer.is_null() && length != 0) {
        return Err(Failure::code(INVALID));
    }
    if length == 0 {
        return Ok(&[]);
    }
    // SAFETY: caller supplies initialized input of length; bound checked before constructing slice.
    Ok(unsafe { std::slice::from_raw_parts(pointer, length) })
}
/// Wallet ABI revision. This is not evidence that the native artifacts are available.
#[unsafe(no_mangle)]
pub extern "C" fn connect_norito_kagemusha_wallet_revision_v1() -> u32 {
    1
}
/// Open an Apple wallet with an authenticated artifact identity.
///
/// Currently returns `ARTIFACTS_UNAVAILABLE`, initializes handle to zero, and performs no
/// callback or custody operation. This explicit unfinished dependency prevents a proof bypass.
/// # Safety
/// Inputs must be initialized for their stated lengths and output must be writable. The
/// callback context satisfies `PlatformCallbacks` if a later authenticated loader retains it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_open_v1(
    callbacks: *const PlatformCallbacks,
    slot: *const u8,
    scheme: *const u8,
    wallet: *const u8,
    artifact: *const u8,
    handle: *mut u64,
) -> i32 {
    if handle.is_null() {
        return INVALID;
    }
    // SAFETY: caller supplies writable handle.
    unsafe { handle.write(0) };
    run(|| {
        if callbacks.is_null() {
            return Err(Failure::code(INVALID));
        }
        // SAFETY: caller supplies initialized complete table and four exact 32-byte identities.
        unsafe {
            CallbackPlatform::validate(&*callbacks)?;
            for value in [slot, scheme, wallet, artifact] {
                if input(value, 32, 32)? == [0; 32] {
                    return Err(Failure::code(INVALID));
                }
            }
        }
        Err::<(), _>(Failure::code(ARTIFACTS_UNAVAILABLE))
    })
    .err()
    .map_or(0, |error| error.status)
}
/// Close an owner, cooperatively join a running fold, and release exclusive custody.
#[unsafe(no_mangle)]
pub extern "C" fn connect_norito_kagemusha_wallet_close_v1(handle: u64) -> i32 {
    run(|| close(handle)).err().map_or(0, |error| error.status)
}
/// Update foreground/charging eligibility; inactive cancels a running sub-proof.
#[unsafe(no_mangle)]
pub extern "C" fn connect_norito_kagemusha_wallet_activity_v1(
    handle: u64,
    foreground: u8,
    charging: u8,
) -> i32 {
    run(|| {
        if foreground > 1 || charging > 1 {
            return Err(Failure::code(INVALID));
        }
        activity(handle, foreground != 0, charging != 0)
    })
    .err()
    .map_or(0, |error| error.status)
}
/// Commit canonical FrozenTransition bytes after native verification and durable custody.
/// # Safety
/// Input is initialized for length; out is writable and its previous result already freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_commit_v1(
    handle: u64,
    bytes: *const u8,
    length: usize,
    out: *mut WalletResult,
) -> i32 {
    unsafe { output(out, || commit(handle, input(bytes, length, FROZEN_MAX)?)) }
}
/// Return the exact source-retained output, without signing or proving again.
/// # Safety
/// Operation is exactly32 readable bytes; out is writable with no unfreed previous result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_retry_v1(
    handle: u64,
    operation: *const u8,
    out: *mut WalletResult,
) -> i32 {
    unsafe { output(out, || retry(handle, input(operation, 32, 32)?)) }
}
/// Reconcile/resume the selected operation, preserving any committed debit and output.
/// # Safety
/// Out is writable with no unfreed previous result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_resume_v1(
    handle: u64,
    out: *mut WalletResult,
) -> i32 {
    unsafe { output(out, || resume(handle)) }
}
/// Compute and durably retain at most one native folding checkpoint or final Ω.
/// # Safety
/// Out is writable with no unfreed previous result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_fold_v1(
    handle: u64,
    out: *mut WalletResult,
) -> i32 {
    unsafe { output(out, || fold(handle)) }
}
/// Construct canonical CreditStatus from the authenticated persistent first-credit index.
/// # Safety
/// Both identities are exactly32 readable bytes; out is writable with no unfreed old result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_credit_status_v1(
    handle: u64,
    credit_id: *const u8,
    payment_digest: *const u8,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        output(out, || {
            credit(
                handle,
                input(credit_id, 32, 32)?,
                input(payment_digest, 32, 32)?,
            )
        })
    }
}
