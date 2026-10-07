//! C ABI, with initialized outputs and panic containment at each boundary.

use super::*;

/// Typed call result. `bytes` uses the existing `connect_norito_free` allocator contract.
#[repr(C)]
#[derive(Debug)]
pub struct WalletResult {
    /// 0 unknown, 1 complete, 2 pending, 3 not performed, 4 archived, 5 delivery loss,
    /// 6 idle, 7 caught up, 8 checkpoint, 9 folded, 10 CreditStatus, 11 preparing; negative is failure.
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
/// Original owner frames. Native selects slot, scheme, wallet and complete proof owners.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WalletOpenRequest {
    /// Original issuer-authenticated credential.
    pub credential: *const u8,
    /// Exact credential frame length.
    pub credential_length: usize,
    /// Original Enrollment CertificateSet.
    pub certificates: *const u8,
    /// Exact certificate-set frame length.
    pub certificates_length: usize,
    /// Existing canonical AccountId frame.
    pub account: *const u8,
    /// Exact account frame length.
    pub account_length: usize,
    /// Original issuer-bound asset scope and scale.
    pub asset: *const u8,
    /// Exact asset-scope frame length.
    pub asset_length: usize,
}
/// Reconcile a native-provisioned runtime and return one fresh account challenge (kind15).
/// # Safety
/// Request/input memory must be readable for its declared lengths and output writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_open_begin_v1(
    runtime: u64,
    request: *const WalletOpenRequest,
    result: *mut WalletResult,
) -> i32 {
    // SAFETY: caller supplies writable output; individual original bounds precede slices.
    unsafe {
        output(result, || {
            let request = request.as_ref().ok_or(Failure::code(INVALID))?;
            open::begin(
                runtime,
                [
                    input(
                        request.credential,
                        request.credential_length,
                        open::BOUNDS[0],
                    )?,
                    input(
                        request.certificates,
                        request.certificates_length,
                        open::BOUNDS[1],
                    )?,
                    input(request.account, request.account_length, open::BOUNDS[2])?,
                    input(request.asset, request.asset_length, open::BOUNDS[3])?,
                ],
            )
        })
    }
}
/// Consume one account challenge and return the admitted wallet handle (kind16).
/// Wrong signatures consume the challenge and retain unadmitted custody for a fresh begin.
/// # Safety
/// Signature must be readable for length and output writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_open_finish_v1(
    runtime: u64,
    signature: *const u8,
    length: usize,
    result: *mut WalletResult,
) -> i32 {
    // SAFETY: caller supplies writable result and readable signature. An invalid byte length
    // reaches the native finish as an empty signature so its one-use challenge is consumed.
    unsafe {
        output(result, || {
            let signature = if length != 64 || signature.is_null() {
                &[]
            } else {
                input(signature, length, 64)?
            };
            open::finish(runtime, signature)
        })
    }
}
/// Abandon an account challenge without discarding native custody or accepting authority.
#[unsafe(no_mangle)]
pub extern "C" fn connect_norito_kagemusha_wallet_open_cancel_v1(runtime: u64) -> i32 {
    run(|| open::cancel(runtime))
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
/// Typed lifecycle input. Unused original slots and amount limbs must be zero/empty.
/// Selectors: Load0, Send1, Receive2, Credential3, SchemePolicy4, Blacklist5,
/// TimeAnchor6, QuotaShare7, Unload8, Retire9. These never select proof keys.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WalletOperationRequest {
    /// Exactly32 readable bytes; nonzero local retry identity, not a protocol operation ID.
    pub request_id: *const u8,
    /// One fixed lifecycle selector documented above.
    pub selector: u32,
    /// Requested Unload amount only; zero for all other operations.
    pub amount: WalletU128,
    /// First original: receipt/Request/Payment/update/charge quote, depending on selector.
    pub first: *const u8,
    /// Exact first-original length.
    pub first_length: usize,
    /// Second original: finality/payer credential/certificate set, depending on selector.
    pub second: *const u8,
    /// Exact second-original length.
    pub second_length: usize,
    /// Third original: Receive certificate set only.
    pub third: *const u8,
    /// Exact third-original length.
    pub third_length: usize,
}
/// Execute typed intent under the exclusive native preparation/proof owner.
/// # Safety
/// Request and its pointers are initialized for their lengths; out is writable with no old result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_execute_v1(
    handle: u64,
    request: *const WalletOperationRequest,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        output(out, || {
            if request.is_null() {
                return Err(Failure::code(INVALID));
            }
            let request = &*request;
            let bounds = requests::bounds(request.selector)?;
            let value = requests::request(
                input(request.request_id, 32, 32)?,
                request.selector,
                u128::from(request.amount.low) | (u128::from(request.amount.high) << 64),
                [
                    input(request.first, request.first_length, bounds[0])?,
                    input(request.second, request.second_length, bounds[1])?,
                    input(request.third, request.third_length, bounds[2])?,
                ],
            )?;
            execute(handle, value)
        })
    }
}
/// Resolve exact request custody; preparing11 is not irreversible pending2 or completion1.
/// # Safety
/// Request identity is exactly32 readable bytes; out is writable with no old result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_request_status_v1(
    handle: u64,
    request_id: *const u8,
    out: *mut WalletResult,
) -> i32 {
    unsafe { output(out, || request_status(handle, input(request_id, 32, 32)?)) }
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

/// Lossless scalar representation shared by the typed snapshot C ABI.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WalletU128 {
    /// Least significant 64 bits.
    pub low: u64,
    /// Most significant 64 bits.
    pub high: u64,
}
impl From<u128> for WalletU128 {
    fn from(value: u128) -> Self {
        Self {
            low: value as u64,
            high: (value >> 64) as u64,
        }
    }
}

/// Fixed typed Native projection; no caller identities, financial codec, or allocated bytes.
/// Zeroed optional storage is meaningful only under its explicit flags after status success.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WalletSnapshot {
    /// Zero success; otherwise an exact negative failure and no snapshot values.
    pub status: i32,
    /// Platform failure reason, or -1.
    pub reason: i32,
    /// Platform failure code, or zero.
    pub platform_code: i32,
    /// Actual lifecycle tag: Active1 or Retiring2; never operation readiness.
    pub lifecycle: u32,
    /// Bit0: verified_fold exists; bit1: current head is folded. Other bits zero.
    pub flags: u32,
    /// Current Native scheme identity.
    pub scheme: [u8; 32],
    /// Current Native wallet incarnation identity.
    pub wallet: [u8; 32],
    /// Exact current source-selected head.
    pub head: [u8; 32],
    /// Exact current credential digest.
    pub credential: [u8; 32],
    /// Exact source-indexed verified folded head, iff bit0.
    pub folded_head: [u8; 32],
    /// Credential of that folded head, iff bit0; renewal may differ from current.
    pub folded_credential: [u8; 32],
    /// Current retained sequence.
    pub sequence: WalletU128,
    /// Current retained gross balance.
    pub balance: WalletU128,
    /// Burns in current retained core.
    pub core_burned_total: WalletU128,
    /// Last verified burns, or actual retained core before any fold.
    pub known_burned_total: WalletU128,
    /// Gross minus known burns, computed by Native only; unfinished folds remain subject to P4.
    pub owned_balance: WalletU128,
    /// Balance with Ω of current head, iff bit1; grants no operation readiness.
    pub folded_balance: WalletU128,
    /// Released heads still requiring local proof.
    pub fold_backlog: WalletU128,
    /// Exact folded head sequence, iff bit0.
    pub folded_sequence: WalletU128,
    /// Exact verified cumulative burns, iff bit0.
    pub folded_burned_total: WalletU128,
}
impl Default for WalletSnapshot {
    fn default() -> Self {
        Self {
            status: INTERNAL,
            reason: -1,
            platform_code: 0,
            lifecycle: 0,
            flags: 0,
            scheme: [0; 32],
            wallet: [0; 32],
            head: [0; 32],
            credential: [0; 32],
            folded_head: [0; 32],
            folded_credential: [0; 32],
            sequence: WalletU128::default(),
            balance: WalletU128::default(),
            core_burned_total: WalletU128::default(),
            known_burned_total: WalletU128::default(),
            owned_balance: WalletU128::default(),
            folded_balance: WalletU128::default(),
            fold_backlog: WalletU128::default(),
            folded_sequence: WalletU128::default(),
            folded_burned_total: WalletU128::default(),
        }
    }
}
impl From<state::Snapshot> for WalletSnapshot {
    fn from(value: state::Snapshot) -> Self {
        let mut out = Self {
            status: 0,
            lifecycle: u32::from(value.lifecycle.tag()),
            scheme: value.scheme_id,
            wallet: value.wallet_id,
            head: value.head,
            credential: value.credential_digest,
            sequence: value.sequence.into(),
            balance: value.balance.into(),
            core_burned_total: value.core_burned_total.into(),
            known_burned_total: value.known_burned_total.into(),
            owned_balance: value.owned_balance.into(),
            fold_backlog: value.fold_backlog.into(),
            ..Self::default()
        };
        if let Some(fold) = value.verified_fold {
            out.flags |= 1;
            out.folded_head = fold.head;
            out.folded_credential = fold.credential_digest;
            out.folded_sequence = fold.sequence.into();
            out.folded_burned_total = fold.burned_total.into();
        }
        if let Some(balance) = value.folded_balance {
            out.flags |= 2;
            out.folded_balance = balance.into();
        }
        out
    }
}
impl From<Failure> for WalletSnapshot {
    fn from(error: Failure) -> Self {
        Self {
            status: error.status,
            reason: error.reason,
            platform_code: error.platform_code,
            ..Self::default()
        }
    }
}

/// Read a source-selected current ownership/fold snapshot. Run off the UI thread.
/// Pending, uncertain, missing witness and proof failures return no monetary projection.
/// # Safety
/// Out must be writable aligned complete WalletSnapshot storage. It owns no allocated memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_snapshot_v1(
    handle: u64,
    out: *mut WalletSnapshot,
) -> i32 {
    if out.is_null() {
        return INVALID;
    }
    // SAFETY: caller supplies writable aligned snapshot memory.
    unsafe { out.write(WalletSnapshot::default()) };
    let value = match run(|| snapshot(handle)) {
        Ok(value) => WalletSnapshot::from(value),
        Err(error) => WalletSnapshot::from(error),
    };
    let status = value.status;
    // SAFETY: admitted complete output storage; all fields are initialized, no heap ownership.
    unsafe { out.write(value) };
    status
}

/// Typed native setup: Bootstrap0, Offer1, Request2, Credited3, begin-time4, finish-time5,
/// cancel-time6; wrap7..10/unwrap11..14 in Offer, Request, Payment, Credited order.
/// Unused identities, amount, token and original slots are exactly zero/empty.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WalletSetupRequest {
    /// Exactly32 readable bytes: nonzero retry identity only for Offer/Request.
    pub setup_id: *const u8,
    /// Fixed setup action, never an Archive relation or proof-key selector.
    pub selector: u32,
    /// Positive Offer amount only.
    pub amount: WalletU128,
    /// Native single-use time token only for finish-time; cannot contain a clock reading.
    pub token: u64,
    /// Offer, Credited or TimeAnchor original, according to action.
    pub first: *const u8,
    /// Exact first original length.
    pub first_length: usize,
    /// Optional Request fee schedule or required finish-time signer certificate.
    pub second: *const u8,
    /// Exact second original length.
    pub second_length: usize,
    /// Optional Request fee-schedule signer certificate only.
    pub third: *const u8,
    /// Exact third original length.
    pub third_length: usize,
}
/// Complete bootstrap or a typed native session action. Status12 carries exact setup bytes;
/// status13 carries nonce32 and sequence_low native token; status14 confirms retained time.
/// # Safety
/// Request and pointed-to inputs are initialized for their lengths; out is writable and empty.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_setup_v1(
    handle: u64,
    request: *const WalletSetupRequest,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        output(out, || {
            if request.is_null() {
                return Err(Failure::code(INVALID));
            }
            let request = &*request;
            let limits = setup::bounds(request.selector)?;
            let action = setup::request(
                input(request.setup_id, 32, 32)?,
                request.selector,
                u128::from(request.amount.low) | (u128::from(request.amount.high) << 64),
                request.token,
                [
                    input(request.first, request.first_length, limits[0])?,
                    input(request.second, request.second_length, limits[1])?,
                    input(request.third, request.third_length, limits[2])?,
                ],
            )?;
            setup(handle, action)
        })
    }
}
