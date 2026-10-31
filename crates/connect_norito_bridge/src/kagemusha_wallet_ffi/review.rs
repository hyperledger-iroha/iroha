//! Owner-local one-use financial review. Projection bytes are DATA, never authority.
use super::*;
use exports::{input, output};

const FINANCIAL_BYTES: usize = 491;
pub(crate) const PROJECTION_FIXED_BYTES: usize = FINANCIAL_BYTES + 4;
const ACCOUNT_MAX: usize = iroha_core_zk::kagemusha_wallet_intake_v1::ACCOUNT_ORIGINAL_MAX_BYTES_V1;
pub(crate) const PROJECTION_MAX_BYTES: usize = PROJECTION_FIXED_BYTES + ACCOUNT_MAX;
const MAX_REVIEWS: usize = 8;
const MAGIC: &[u8; 8] = b"KWORV1\0\0";
const CHARGE_MAGIC: &[u8; 8] = b"KWUCV1\0\0";
const CHARGE_CARRIER_MAX: usize = 16 + KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1 + ACCOUNT_MAX;

/// One fixed request for Native to authenticate; no foreign state or financial verdict.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WalletReviewRequest {
    /// Send1 or Unload8 only.
    pub selector: u32,
    /// Fresh0 or exact-original unstarted1; every other mode is rejected.
    pub expectation: u32,
    /// Unload face amount only; zero for Send.
    pub amount: WalletU128,
    /// Exactly32 nonzero bytes, bound to the actual review at issuance.
    pub request_id32: *const u8,
    /// Exact prior projection comparison DATA; empty only for Fresh.
    pub expected_projection: *const u8,
    /// Bounded exact projection length; no retired frame is accepted.
    pub expected_projection_length: usize,
    /// Complete Request for Send, optional original charge quote for Unload.
    pub first: *const u8,
    /// Exact first-original length.
    pub first_length: usize,
    /// Canonical destination AccountId for Send; closed certificate/beneficiary DATA for Unload.
    pub second: *const u8,
    /// Exact second-original length.
    pub second_length: usize,
}
pub(crate) enum Input {
    Send {
        request_id: [u8; 32],
        expectation: state::ReviewExpectationV1,
        request: Vec<u8>,
        destination_account: Vec<u8>,
    },
    Unload {
        request_id: [u8; 32],
        expectation: state::ReviewExpectationV1,
        amount: u128,
        charge: Option<state::ChargeOriginalsV1>,
        beneficiary: Option<Vec<u8>>,
    },
}
pub(crate) fn bounds(selector: u32) -> Result<[usize; 2]> {
    Ok(match selector {
        1 => [KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1, ACCOUNT_MAX],
        8 => [
            KAGEMUSHA_WALLET_CHARGE_QUOTE_MAX_BYTES_V1,
            CHARGE_CARRIER_MAX,
        ],
        _ => return Err(Failure::code(INVALID)),
    })
}
fn unload_charge_carrier(second: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    let invalid = || Failure::code(INVALID);
    if second.len() < 18 || second.len() > CHARGE_CARRIER_MAX || &second[..8] != CHARGE_MAGIC {
        return Err(invalid());
    }
    let certificates =
        u32::from_le_bytes(second[8..12].try_into().map_err(|_| invalid())?) as usize;
    if certificates == 0 || certificates > KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1 {
        return Err(invalid());
    }
    let beneficiary_offset = 12 + certificates;
    let length = second
        .get(beneficiary_offset..beneficiary_offset + 4)
        .ok_or_else(invalid)?;
    let beneficiary = u32::from_le_bytes(length.try_into().map_err(|_| invalid())?) as usize;
    if beneficiary == 0
        || beneficiary > ACCOUNT_MAX
        || second.len() != beneficiary_offset + 4 + beneficiary
    {
        return Err(invalid());
    }
    Ok((
        second[12..beneficiary_offset].to_vec(),
        second[beneficiary_offset + 4..].to_vec(),
    ))
}
pub(crate) fn request(
    request_id: &[u8],
    selector: u32,
    amount: u128,
    expectation: u32,
    expected_projection: &[u8],
    first: &[u8],
    second: &[u8],
) -> Result<Input> {
    let request_id: [u8; 32] = request_id.try_into().map_err(|_| Failure::code(INVALID))?;
    if request_id == [0; 32] {
        return Err(Failure::code(INVALID));
    }
    let expectation = match expectation {
        0 if expected_projection.is_empty() => state::ReviewExpectationV1::Fresh,
        1 => state::ReviewExpectationV1::OriginalUnstarted(parse_projection(
            selector,
            expected_projection,
        )?),
        _ => return Err(Failure::code(INVALID)),
    };
    let b = bounds(selector)?;
    if first.len() > b[0] || second.len() > b[1] {
        return Err(Failure::code(INVALID));
    }
    Ok(match selector {
        1 if amount == 0 && !first.is_empty() && !second.is_empty() => Input::Send {
            request_id,
            expectation,
            request: first.to_vec(),
            destination_account: second.to_vec(),
        },
        8 if amount != 0 && first.is_empty() && second.is_empty() => Input::Unload {
            request_id,
            expectation,
            amount,
            charge: None,
            beneficiary: None,
        },
        8 if amount != 0 && !first.is_empty() => {
            let (certificates, beneficiary) = unload_charge_carrier(second)?;
            Input::Unload {
                request_id,
                expectation,
                amount,
                charge: Some(state::ChargeOriginalsV1 {
                    quote: first.to_vec(),
                    certificates,
                }),
                beneficiary: Some(beneficiary),
            }
        }
        _ => return Err(Failure::code(INVALID)),
    })
}
// Financial review, deletion review and direct-time capabilities share one process sequence.
// A numeric token from another owner or purpose cannot alias any outstanding capability.
static NEXT_CAPABILITY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub(super) fn allocate_token() -> Result<u64> {
    allocate_from(&NEXT_CAPABILITY)
}
fn allocate_from(next: &std::sync::atomic::AtomicU64) -> Result<u64> {
    use std::sync::atomic::Ordering;
    next.try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
        value
            .checked_add(1)
            .filter(|value| *value <= i64::MAX as u64)
    })
    .map(|value| value + 1)
    .map_err(|_| Failure::code(RESOURCE))
}

/// This private owner-local table holds actual non-Clone Native capabilities.
/// It never imports a capability from its copyable projection or token.
pub(super) struct Tokens<T> {
    values: BTreeMap<u64, T>,
}

/// A genuine Native capability bound to the exact requested exchange that issued it.
pub(super) struct OriginBound<T> {
    origin: u64,
    pub(super) value: T,
}
impl<T> OriginBound<T> {
    pub(super) fn new(origin: u64, value: T) -> Self {
        Self { origin, value }
    }
    pub(super) fn require_origin(&self, origin: u64) -> Result<()> {
        if origin == 0 || self.origin != origin {
            Err(Failure::code(INVALID))
        } else {
            Ok(())
        }
    }
}
impl<T> Tokens<OriginBound<T>> {
    pub(super) fn take_bound(&mut self, token: u64, origin: u64) -> Result<T> {
        self.get(token)
            .ok_or(Failure::code(INVALID))?
            .require_origin(origin)?;
        self.take(token).map(|bound| bound.value)
    }
}
impl<T> Default for Tokens<T> {
    fn default() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }
}
impl<T> Tokens<T> {
    pub(super) fn capacity(&self) -> Result<()> {
        if self.values.len() >= MAX_REVIEWS {
            Err(Failure::code(RESOURCE))
        } else {
            Ok(())
        }
    }
    pub(super) fn put(&mut self, value: T) -> Result<u64> {
        self.capacity()?;
        let id = allocate_token()?;
        self.values.insert(id, value);
        Ok(id)
    }
    pub(super) fn clear(&mut self) {
        self.values.clear();
    }
    pub(super) fn take(&mut self, token: u64) -> Result<T> {
        self.values.remove(&token).ok_or(Failure::code(INVALID))
    }
    // Only the caller that just took this table's token may restore it, under the same
    // exclusive wallet ownership. No foreign route accepts a token/value pair to restore.
    fn restore_exact(&mut self, token: u64, value: T) -> Result<()> {
        if token == 0 || token > i64::MAX as u64 {
            return Err(Failure::code(INTERNAL));
        }
        match self.values.entry(token) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(value);
                Ok(())
            }
            std::collections::btree_map::Entry::Occupied(_) => Err(Failure::code(INTERNAL)),
        }
    }
    pub(super) fn get(&self, token: u64) -> Option<&T> {
        self.values.get(&token)
    }
}
pub(super) fn retained_response<T>(
    tokens: &Tokens<OriginBound<T>>,
    origin: u64,
    token: u64,
    detail: u32,
) -> Result<Response> {
    tokens
        .get(token)
        .ok_or(Failure::code(INVALID))?
        .require_origin(origin)?;
    if !matches!(detail, 1 | 2 | 6) {
        return Err(Failure::code(INTERNAL));
    }
    Ok(Response {
        kind: 65,
        sequence: u128::from(token),
        detail,
        ..Response::default()
    })
}

fn wait_detail(wait: state::ReviewMaintenanceWaitV1) -> u32 {
    match wait {
        state::ReviewMaintenanceWaitV1::SelectedPending => 1,
        state::ReviewMaintenanceWaitV1::ArchiveFold => 2,
        state::ReviewMaintenanceWaitV1::Busy => 6,
    }
}

fn waiting_response(wait: state::ReviewMaintenanceWaitV1) -> Response {
    Response {
        kind: 67,
        detail: wait_detail(wait),
        ..Response::default()
    }
}

fn existing_response(value: state::BoundRequestStatusV1) -> Result<Response> {
    let inner = match value {
        state::BoundRequestStatusV1::Preparing => Response {
            kind: 11,
            ..Response::default()
        },
        state::BoundRequestStatusV1::Outcome(value) => completion(Some(value))?,
    };
    if inner.bytes.len() > KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1 {
        return Err(Failure::code(INTERNAL));
    }
    let mut bytes = Vec::with_capacity(28 + inner.bytes.len());
    bytes.extend_from_slice(&(inner.kind as u32).to_le_bytes());
    bytes.extend_from_slice(&inner.detail.to_le_bytes());
    bytes.extend_from_slice(&inner.sequence.to_le_bytes());
    bytes.extend_from_slice(&(inner.bytes.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&inner.bytes);
    Ok(Response {
        kind: 66,
        bytes,
        ..Response::default()
    })
}

fn projection(value: &state::NativeOperationReviewV1) -> Result<Vec<u8>> {
    let selector = match value.kind {
        KagemushaWalletOperationKindV1::Send => 1,
        KagemushaWalletOperationKindV1::Unload => 8,
        _ => return Err(Failure::code(INTERNAL)),
    };
    let account = match (selector, value.destination_account_original.as_deref()) {
        (1, Some(original)) if !original.is_empty() && original.len() <= ACCOUNT_MAX => original,
        (8, None) => &[],
        _ => return Err(Failure::code(INTERNAL)),
    };
    let mut out = Vec::with_capacity(PROJECTION_FIXED_BYTES + account.len());
    out.extend_from_slice(MAGIC);
    out.push(selector);
    for amount in [
        value.amount,
        value.fee,
        value.gross_debit,
        value.net_destination_amount,
    ] {
        out.extend_from_slice(&amount.to_le_bytes());
    }
    out.push(u8::from(value.receiver_wallet_id.is_some()));
    out.extend_from_slice(&value.receiver_wallet_id.unwrap_or([0; 32]));
    for digest in [
        value.destination_account_digest,
        value.request_digest,
        value.charge_quote_digest,
        value.scheme_id,
        value.wallet_id,
        value.current_head,
        value.source_state_commitment,
        value.source_capsule_digest,
        value.credential_digest,
        value.artifact_manifest_digest,
    ] {
        out.extend_from_slice(&digest);
    }
    out.extend_from_slice(value.payment_key.as_sec1_bytes());
    if out.len() != FINANCIAL_BYTES {
        return Err(Failure::code(INTERNAL));
    }
    out.extend_from_slice(&(account.len() as u32).to_le_bytes());
    out.extend_from_slice(account);
    Ok(out)
}

// Decode comparison DATA only. Core reauthenticates the complete current source/action;
// this frame never reconstructs a capability or establishes request nonmembership.
fn parse_projection(selector: u32, bytes: &[u8]) -> Result<state::NativeOperationReviewV1> {
    let invalid = || Failure::code(INVALID);
    if !(PROJECTION_FIXED_BYTES..=PROJECTION_MAX_BYTES).contains(&bytes.len())
        || &bytes[..8] != MAGIC
        || u32::from(bytes[8]) != selector
    {
        return Err(invalid());
    }
    let length = u32::from_le_bytes(
        bytes[FINANCIAL_BYTES..PROJECTION_FIXED_BYTES]
            .try_into()
            .map_err(|_| invalid())?,
    ) as usize;
    if length > ACCOUNT_MAX || bytes.len() != PROJECTION_FIXED_BYTES + length {
        return Err(invalid());
    }
    let word = |offset: usize| -> Result<[u8; 32]> {
        bytes
            .get(offset..offset + 32)
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|_| invalid())
    };
    let scalar = |offset: usize| -> Result<u128> {
        Ok(u128::from_le_bytes(
            bytes
                .get(offset..offset + 16)
                .ok_or_else(invalid)?
                .try_into()
                .map_err(|_| invalid())?,
        ))
    };
    let (kind, receiver_wallet_id, destination_account_original) = match selector {
        1 if length != 0
            && bytes[73] == 1
            && word(74)? != [0; 32]
            && word(138)? != [0; 32]
            && word(170)? == [0; 32] =>
        {
            (
                KagemushaWalletOperationKindV1::Send,
                Some(word(74)?),
                Some(bytes[PROJECTION_FIXED_BYTES..].to_vec()),
            )
        }
        8 if length == 0 && bytes[73] == 0 && word(74)? == [0; 32] && word(138)? == [0; 32] => {
            (KagemushaWalletOperationKindV1::Unload, None, None)
        }
        _ => return Err(invalid()),
    };
    if scalar(9)? == 0
        || [106, 202, 234, 266, 298, 330, 362, 394]
            .into_iter()
            .any(|offset| bytes[offset..offset + 32] == [0; 32])
    {
        return Err(invalid());
    }
    Ok(state::NativeOperationReviewV1 {
        kind,
        amount: scalar(9)?,
        fee: scalar(25)?,
        gross_debit: scalar(41)?,
        net_destination_amount: scalar(57)?,
        receiver_wallet_id,
        destination_account_digest: word(106)?,
        destination_account_original,
        request_digest: word(138)?,
        charge_quote_digest: word(170)?,
        scheme_id: word(202)?,
        wallet_id: word(234)?,
        current_head: word(266)?,
        source_state_commitment: word(298)?,
        source_capsule_digest: word(330)?,
        credential_digest: word(362)?,
        artifact_manifest_digest: word(394)?,
        payment_key: KagemushaDevicePublicKeyV1::from_sec1_bytes(&bytes[426..491])
            .map_err(|_| invalid())?,
    })
}
impl<P: advance::KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send> NativeWallet<P, S> {
    pub(super) fn review_inner(&mut self, exchange_id: u64, input: Input) -> Result<Response> {
        let decision = match input {
            Input::Send {
                request_id,
                expectation,
                request,
                destination_account,
            } => {
                self.wallet
                    .review_send(request_id, &request, &destination_account, expectation)?
            }
            Input::Unload {
                request_id,
                expectation,
                amount,
                charge,
                beneficiary,
            } => self.wallet.review_unload(
                request_id,
                amount,
                charge,
                beneficiary.as_deref(),
                expectation,
            )?,
        };
        let review = match decision {
            state::ReviewDecisionV1::Ready(review) => review,
            state::ReviewDecisionV1::Existing(value) => return existing_response(value),
            state::ReviewDecisionV1::Waiting(wait) => return Ok(waiting_response(wait)),
        };
        let bytes = projection(review.projection())?;
        let token = self.reviews.put(OriginBound::new(exchange_id, review))?;
        Ok(Response {
            kind: 18,
            sequence: u128::from(token),
            bytes,
            ..Response::default()
        })
    }
    pub(super) fn execute_reviewed_inner(
        &mut self,
        exchange_id: u64,
        token: u64,
    ) -> Result<Response> {
        // Actual execution consumes even on an invalid identity or failed source recheck.
        // Only Native's pre-execution maintenance refusal can return this same genuine review.
        let reviewed = self.reviews.take_bound(token, exchange_id)?;
        match self.wallet.execute_reviewed(reviewed) {
            Ok(state::ReviewedExecutionV1::Executed(value)) => completion(Some(value)),
            Ok(state::ReviewedExecutionV1::Existing(value)) => existing_response(value),
            Err(failure) => {
                let wait = failure.maintenance_wait();
                let (error, retained) = failure.into_parts();
                match (wait, retained) {
                    (Some(wait), Some(reviewed)) => {
                        self.reviews
                            .restore_exact(token, OriginBound::new(exchange_id, reviewed))?;
                        retained_response(&self.reviews, exchange_id, token, wait_detail(wait))
                    }
                    (None, None) => Err(error.into()),
                    _ => Err(Failure::code(INTERNAL)),
                }
            }
        }
    }
}
pub(crate) fn review(
    invocation: &exchange::Invocation,
    input: Input,
    delivery: &mut CapabilityDelivery,
) -> Result<Response> {
    invocation.validate()?;
    let owner = invocation.owner();
    let (response, speculation) =
        with_wallet_owner(Arc::clone(&owner), Priority::Ingest, |wallet| {
            let response = wallet.review(invocation.origin(), input)?;
            delivery.issued(invocation, &response);
            let speculation = (response.kind == 18)
                .then_some(response.sequence)
                .and_then(|token| u64::try_from(token).ok())
                .and_then(|token| wallet.review_speculation(token));
            Ok((response, speculation))
        })?;
    if let Some(job) = speculation {
        speculate(&owner, job);
    }
    if response.kind == 67 {
        owner.background.retry(&owner)?;
    } else {
        owner.background.notify();
    }
    Ok(response)
}
/// Prove one Send review's σ_send speculatively on its own thread, the way a fold turn
/// proves: it prepares under the wallet owner, then waits for the scheduler's speculation slot
/// (payment priority over folds; payments wait for it) and proves without the owner. A
/// refusal, a failure or a thread that cannot start only leaves execution to prove; nothing
/// reaches the review's caller.
fn speculate(owner: &Arc<Owner>, job: state::SpeculativeSendJobV1) {
    let owner = Arc::downgrade(owner);
    let _ = std::thread::Builder::new()
        .name("kagemusha-speculate".into())
        .spawn(move || {
            let Some(owner) = owner.upgrade() else {
                return;
            };
            // The owner is used only before admission: a payment waiting for the admitted
            // proof may hold the owner.
            let turn = state::NativeWorkTurnV1::start(state::NativeWorkKindV1::SpeculativeSend);
            let prepared = run(|| {
                with_wallet_owner(Arc::clone(&owner), Priority::None, |wallet| {
                    wallet.prepare_speculation(&job)
                })
            });
            drop(job);
            let returned = match prepared {
                Ok(Some(proof)) => {
                    let scheduler = owner.scheduler.clone();
                    let result = run(move || Ok(proof.prove(&scheduler)?));
                    // Folds waited while the speculation held its slot.
                    owner.background.notify();
                    result.is_ok()
                }
                Ok(None) => true,
                Err(_) => false,
            };
            owner.background.record_native_work(&turn.finish(returned));
        });
}
pub(crate) fn execute_reviewed(invocation: &exchange::Invocation, token: u64) -> Result<Response> {
    invocation.validate()?;
    let owner = invocation.owner();
    let turn = state::NativeWorkTurnV1::start(state::NativeWorkKindV1::ReviewedExecute);
    // Core classifies the privately bound request before reserving Payment. An outer
    // maintenance wait or Payment guard would hide existing work and preempt needlessly.
    let result = with_wallet_owner(Arc::clone(&owner), Priority::None, |wallet| {
        wallet.execute_reviewed(invocation.origin(), token)
    });
    owner
        .background
        .record_native_work(&turn.finish(result.is_ok()));
    let response = result?;
    if response.kind == 65 {
        owner.background.retry(&owner)?;
    } else {
        owner.background.notify();
    }
    Ok(response)
}
pub(crate) fn discard_review(invocation: &exchange::Invocation, token: u64) -> Result<()> {
    with_wallet_owner(invocation.owner(), Priority::Ingest, |wallet| {
        wallet.discard_review(invocation.origin(), token)
    })
}

/// Closed kinds of owner-local, non-durable capabilities. No monetary operation is undone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CapabilityKind {
    Financial,
    Time,
    Deletion,
}

/// Owns in-memory capability cleanup before inner publication through foreign delivery.
#[derive(Default)]
pub(crate) struct CapabilityDelivery {
    pending: Option<(Arc<Owner>, CapabilityKind, u64, u64)>,
}
impl CapabilityDelivery {
    pub(crate) fn financial(invocation: &exchange::Invocation, token: u64) -> Self {
        Self {
            pending: Some((
                invocation.owner(),
                CapabilityKind::Financial,
                invocation.origin(),
                token,
            )),
        }
    }
    /// Called inside the wallet action immediately after insertion, before any wrapper fence.
    pub(super) fn issued(&mut self, invocation: &exchange::Invocation, response: &Response) {
        let kind = match response.kind {
            13 => CapabilityKind::Time,
            18 => CapabilityKind::Financial,
            53 => CapabilityKind::Deletion,
            _ => return,
        };
        self.pending = Some((
            invocation.owner(),
            kind,
            invocation.origin(),
            response.sequence as u64,
        ));
    }
    pub(crate) fn complete(&mut self, delivered: bool) {
        if delivered {
            self.pending = None;
        }
    }
}
impl Drop for CapabilityDelivery {
    fn drop(&mut self) {
        let Some((owner, kind, origin, token)) = self.pending.take() else {
            return;
        };
        // No custody admission, archive publication, durability fence, proof priority or
        // fresh registry lookup can block removal of this exact in-memory capability.
        let cleanup = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
            let mut guard = owner.wallet.lock().map_err(|_| Failure::code(INTERNAL))?;
            let Some(wallet) = guard.as_deref_mut() else {
                return Ok(());
            };
            match wallet.discard_capability(kind, origin, token) {
                Err(error) if error.status == INVALID => Ok(()), // consumed or foreign; preserve other capabilities
                result => result,
            }
        }));
        if !matches!(&cleanup, Ok(Ok(()))) {
            // Keep exact custody registered for explicit close retry. Never join while
            // this failed foreign handoff still owns an accepted invocation child.
            owner.closing.begin();
            owner.background.stop();
            owner.scheduler.close();
        }
    }
}
/// A non-null foreign error object does not deliver a capability issued before a failed fence.
pub(crate) fn deliver<T>(
    mut delivery: CapabilityDelivery,
    result: Result<Response>,
    emit: impl FnOnce(Result<Response>) -> (T, bool),
) -> T {
    let succeeded = result.is_ok();
    let (value, delivered) = emit(result);
    delivery.complete(succeeded && delivered);
    value
}

/// Authenticate Send/Unload and retain the genuine one-use review. No monetary operation.
/// Result18 contains the491-byte financial prefix, LE u32 account length and exact
/// authenticated AccountId DATA (Send1..4096; Unload0), with an owner-local one-use token.
/// # Safety
/// Caller supplies initialized inputs and writable result memory for stated lengths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_review_v1(
    handle: u64,
    exchange_id: u64,
    request: *const WalletReviewRequest,
    out: *mut WalletResult,
) -> i32 {
    let mut delivery = CapabilityDelivery::default();
    let mut admitted = None;
    let status = unsafe {
        output(out, || {
            if request.is_null() {
                return Err(Failure::code(INVALID));
            }
            let req = &*request;
            let b = bounds(req.selector)?;
            let request_id = input(req.request_id32, 32, 32)?;
            let expected_projection = input(
                req.expected_projection,
                req.expected_projection_length,
                PROJECTION_MAX_BYTES,
            )?;
            let first = input(req.first, req.first_length, b[0])?;
            let second = input(req.second, req.second_length, b[1])?;
            let amount = u128::from(req.amount.low) | (u128::from(req.amount.high) << 64);
            let input = self::request(
                request_id,
                req.selector,
                amount,
                req.expectation,
                expected_projection,
                first,
                second,
            )?;
            admitted = Some(exchange::Invocation::admit(handle, exchange_id)?);
            review(
                admitted.as_ref().ok_or(Failure::code(INTERNAL))?,
                input,
                &mut delivery,
            )
        })
    };
    delivery.complete(status == 0);
    status
}
/// Consume the actual retained review after fresh hardware UI approval; source is rechecked.
/// No amount, destination, signature body or financial verdict is accepted from foreign code.
/// # Safety
/// The result points to writable memory. The actual capability privately owns its request ID.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_execute_reviewed_v1(
    handle: u64,
    exchange_id: u64,
    token: u64,
    out: *mut WalletResult,
) -> i32 {
    let mut admitted = None;
    let mut delivery = None;
    let mut retained = false;
    let status = unsafe {
        output(out, || {
            admitted = Some(exchange::Invocation::admit(handle, exchange_id)?);
            delivery = Some(CapabilityDelivery::financial(
                admitted.as_ref().ok_or(Failure::code(INTERNAL))?,
                token,
            ));
            let response =
                execute_reviewed(admitted.as_ref().ok_or(Failure::code(INTERNAL))?, token)?;
            retained = response.kind == 65;
            Ok(response)
        })
    };
    if let Some(delivery) = delivery.as_mut() {
        delivery.complete(status == 0 && retained);
    }
    status
}
/// Drop one owner-local review on cancellation. Never affects durable monetary custody.
#[unsafe(no_mangle)]
pub extern "C" fn connect_norito_kagemusha_wallet_discard_review_v1(
    handle: u64,
    exchange_id: u64,
    token: u64,
) -> i32 {
    run(|| discard_review(&exchange::Invocation::cleanup(handle, exchange_id)?, token))
        .err()
        .map_or(0, |error| error.status)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fresh_request(selector: u32, amount: u128, first: &[u8], second: &[u8]) -> Result<Input> {
        request(&[1; 32], selector, amount, 0, &[], first, second)
    }
    fn charge_carrier(certificates: &[u8], beneficiary: &[u8]) -> Vec<u8> {
        let mut bytes = CHARGE_MAGIC.to_vec();
        bytes.extend_from_slice(&(certificates.len() as u32).to_le_bytes());
        bytes.extend_from_slice(certificates);
        bytes.extend_from_slice(&(beneficiary.len() as u32).to_le_bytes());
        bytes.extend_from_slice(beneficiary);
        bytes
    }
    #[test]
    fn charged_review_carrier_is_closed_bounded_and_keeps_exact_originals() {
        let certificates = vec![7; 10_034];
        let beneficiary = vec![8; ACCOUNT_MAX];
        let bytes = charge_carrier(&certificates, &beneficiary);
        assert_eq!(bytes.len(), CHARGE_CARRIER_MAX);
        assert_eq!(bounds(8).unwrap(), [1_024, 14_146]);
        let Input::Unload {
            charge,
            beneficiary: actual,
            ..
        } = fresh_request(8, 1, &[9], &bytes).unwrap()
        else {
            panic!("Unload input");
        };
        assert_eq!(charge.unwrap().certificates, certificates);
        assert_eq!(actual, Some(beneficiary));
        for invalid in [
            vec![7; 10_034],
            charge_carrier(&[], &[8]),
            charge_carrier(&[7], &[]),
            charge_carrier(&vec![7; 10_035], &[8]),
            charge_carrier(&[7], &vec![8; ACCOUNT_MAX + 1]),
            bytes[..bytes.len() - 1].to_vec(),
            [bytes.as_slice(), &[0]].concat(),
        ] {
            assert!(fresh_request(8, 1, &[9], &invalid).is_err());
        }
        for offset in [0, 8, 12 + certificates.len()] {
            let mut invalid = bytes.clone();
            invalid[offset] ^= 0xff;
            assert!(fresh_request(8, 1, &[9], &invalid).is_err());
        }
    }
    #[test]
    fn intake_refuses_other_operations_and_unused_authority() {
        assert!(fresh_request(1, 0, &[7], &[8]).is_ok());
        assert!(fresh_request(1, 1, &[7], &[8]).is_err());
        assert!(fresh_request(1, 0, &[7], &[]).is_err());
        assert!(fresh_request(1, 0, &[], &[8]).is_err());
        assert_eq!(bounds(1).unwrap(), [10_034, ACCOUNT_MAX]);
        assert!(fresh_request(1, 0, &[7], &vec![8; ACCOUNT_MAX + 1]).is_err());
        assert!(fresh_request(8, u128::MAX, &[], &[]).is_ok());
        assert!(fresh_request(8, 1, &[7], &charge_carrier(&[8], &[9])).is_ok());
        assert!(fresh_request(8, 1, &[7], &[8]).is_err());
        assert!(fresh_request(8, 1, &[7], &[]).is_err());
        assert!(fresh_request(8, 1, &[], &[8]).is_err());
        assert!(fresh_request(8, 0, &[], &[]).is_err());
        for selector in [0, 2, 3, 4, 5, 6, 7, 9, u32::MAX] {
            assert!(bounds(selector).is_err());
        }
        assert!(
            fresh_request(
                1,
                0,
                &vec![7; KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1 + 1],
                &[8]
            )
            .is_err()
        );
    }
    fn unadmitted_projection_fixture() -> state::NativeOperationReviewV1 {
        use iroha_crypto::{Algorithm, KeyPair};
        use iroha_data_model::account::AccountId;
        let account = AccountId::new(
            KeyPair::from_seed(vec![7; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        let payment = p256::ecdsa::SigningKey::from_bytes((&[9; 32]).into()).unwrap();
        state::NativeOperationReviewV1 {
            kind: KagemushaWalletOperationKindV1::Send,
            amount: 1,
            fee: 2,
            gross_debit: 3,
            net_destination_amount: 1,
            receiver_wallet_id: Some([4; 32]),
            destination_account_digest: kagemusha_wallet_account_digest_v1(&account).unwrap(),
            destination_account_original: Some(norito::encode_canonical(&account).unwrap()),
            request_digest: [5; 32],
            charge_quote_digest: [0; 32],
            scheme_id: [6; 32],
            wallet_id: [7; 32],
            current_head: [8; 32],
            source_state_commitment: [9; 32],
            source_capsule_digest: [10; 32],
            credential_digest: [11; 32],
            payment_key: KagemushaDevicePublicKeyV1::from_sec1_bytes(
                payment.verifying_key().to_sec1_point(false).as_bytes(),
            )
            .unwrap(),
            artifact_manifest_digest: [12; 32],
        }
    }
    #[test]
    fn projection_transports_exact_bounded_original_with_explicit_length() {
        // Public DATA framing only; this fixture cannot construct a genuine reviewed token.
        let value = unadmitted_projection_fixture();
        let original = value.destination_account_original.as_ref().unwrap();
        let bytes = projection(&value).unwrap();
        assert_eq!(bytes.len(), PROJECTION_FIXED_BYTES + original.len());
        assert_eq!(
            &bytes[FINANCIAL_BYTES..PROJECTION_FIXED_BYTES],
            &(original.len() as u32).to_le_bytes()
        );
        assert_eq!(&bytes[PROJECTION_FIXED_BYTES..], original);
        assert_eq!(&bytes[106..138], &value.destination_account_digest);
        let mut copied = bytes;
        copied[PROJECTION_FIXED_BYTES] ^= 1;
        assert_ne!(&copied[PROJECTION_FIXED_BYTES..], original);
        assert_eq!(
            &projection(&value).unwrap()[PROJECTION_FIXED_BYTES..],
            original.as_slice()
        );
    }
    #[test]
    fn projection_rejects_missing_oversized_and_foreign_unload_original_shapes() {
        for original in [None, Some(vec![]), Some(vec![0; ACCOUNT_MAX + 1])] {
            let mut value = unadmitted_projection_fixture();
            value.destination_account_original = original;
            assert_eq!(projection(&value).unwrap_err().status, INTERNAL);
        }
        let mut unload = unadmitted_projection_fixture();
        unload.kind = KagemushaWalletOperationKindV1::Unload;
        unload.receiver_wallet_id = None;
        unload.request_digest = [0; 32];
        assert_eq!(projection(&unload).unwrap_err().status, INTERNAL);
        unload.destination_account_original = None;
        let bytes = projection(&unload).unwrap();
        assert_eq!(bytes.len(), PROJECTION_FIXED_BYTES);
        assert_eq!(&bytes[FINANCIAL_BYTES..], &[0; 4]);
    }
    #[test]
    fn review_projection_constraints_roundtrip_complete_send_and_unload_originals() {
        let send = unadmitted_projection_fixture();
        assert_eq!(
            parse_projection(1, &projection(&send).unwrap()).unwrap(),
            send
        );
        let mut unload = send.clone();
        unload.kind = KagemushaWalletOperationKindV1::Unload;
        unload.receiver_wallet_id = None;
        unload.destination_account_original = None;
        unload.request_digest = [0; 32];
        unload.charge_quote_digest = [13; 32];
        assert_eq!(
            parse_projection(8, &projection(&unload).unwrap()).unwrap(),
            unload
        );
        let mut largest = send;
        largest.destination_account_original = Some(vec![7; ACCOUNT_MAX]);
        let bytes = projection(&largest).unwrap();
        assert_eq!(bytes.len(), PROJECTION_MAX_BYTES);
        // Framing DATA only: Core still authenticates the actual canonical action account.
        assert_eq!(parse_projection(1, &bytes).unwrap(), largest);
    }
    #[test]
    fn review_projection_constraints_refuse_retired_or_ambiguous_frames() {
        let bytes = projection(&unadmitted_projection_fixture()).unwrap();
        for invalid in [
            vec![],
            vec![7; 64],
            bytes[..491].to_vec(),
            bytes[..494].to_vec(),
            bytes[..bytes.len() - 1].to_vec(),
            [bytes.as_slice(), &[0]].concat(),
            vec![7; PROJECTION_MAX_BYTES + 1],
        ] {
            assert_eq!(parse_projection(1, &invalid).unwrap_err().status, INVALID);
        }
        for selector in [0, 8, u32::MAX] {
            assert_eq!(
                parse_projection(selector, &bytes).unwrap_err().status,
                INVALID
            );
        }
        for (offset, count) in [
            (0, 1),
            (8, 1),
            (9, 16),
            (73, 1),
            (74, 32),
            (106, 32),
            (138, 32),
            (202, 32),
            (234, 32),
            (266, 32),
            (298, 32),
            (330, 32),
            (362, 32),
            (394, 32),
            (426, 65),
            (491, 4),
        ] {
            let mut invalid = bytes.clone();
            invalid[offset..offset + count].fill(0);
            assert_eq!(
                parse_projection(1, &invalid).unwrap_err().status,
                INVALID,
                "offset {offset}"
            );
        }
        let mut invalid = bytes.clone();
        invalid[170] = 1; // Send has no Unload quote.
        assert_eq!(parse_projection(1, &invalid).unwrap_err().status, INVALID);
        let mut invalid = bytes;
        invalid[491..495].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(parse_projection(1, &invalid).unwrap_err().status, INVALID);
    }
    #[test]
    fn review_request_requires_original_identity_and_closed_expectation() {
        let expected = unadmitted_projection_fixture();
        let bytes = projection(&expected).unwrap();
        for id in [vec![], vec![1; 31], vec![0; 32], vec![1; 33]] {
            assert_eq!(
                request(&id, 1, 0, 0, &[], &[7], &[8]).err().unwrap().status,
                INVALID
            );
        }
        for (mode, old) in [
            (0, bytes.as_slice()),
            (1, &[][..]),
            (2, &[][..]),
            (u32::MAX, bytes.as_slice()),
        ] {
            assert_eq!(
                request(&[9; 32], 1, 0, mode, old, &[7], &[8])
                    .err()
                    .unwrap()
                    .status,
                INVALID
            );
        }
        let Input::Send {
            request_id,
            expectation,
            request: original,
            destination_account,
        } = request(&[9; 32], 1, 0, 1, &bytes, &[7], &[8]).unwrap()
        else {
            panic!("Send constraints");
        };
        assert_eq!(request_id, [9; 32]);
        assert_eq!(original, [7]);
        assert_eq!(destination_account, [8]);
        let state::ReviewExpectationV1::OriginalUnstarted(actual) = expectation else {
            panic!("exact original projection");
        };
        assert_eq!(actual, expected);
    }
    #[test]
    fn existing_review_result_preserves_only_bound_request_outcomes() {
        use state::{BoundRequestStatusV1 as B, Completion as C};
        let cases = [
            (B::Preparing, 11, 0, vec![]),
            (
                B::Outcome(C::Complete(vec![0, 255, 7])),
                1,
                0,
                vec![0, 255, 7],
            ),
            (B::Outcome(C::Pending), 2, 0, vec![]),
            (
                B::Outcome(C::NotPerformed(
                    advance::KagemushaWalletNotPerformedV1::StaleHead,
                )),
                3,
                0,
                vec![],
            ),
            (B::Outcome(C::Archived), 4, 0, vec![]),
            (B::Outcome(C::DeliveryDataLoss), 5, 0, vec![]),
            (B::Outcome(C::CreditStatus(vec![8])), 10, 0, vec![8]),
        ];
        for (value, kind, detail, original) in cases {
            let reply = existing_response(value).unwrap();
            assert_eq!((reply.kind, reply.sequence, reply.detail), (66, 0, 0));
            assert_eq!(reply.bytes.len(), 28 + original.len());
            assert_eq!(&reply.bytes[..4], &(kind as u32).to_le_bytes());
            assert_eq!(&reply.bytes[4..8], &(detail as u32).to_le_bytes());
            assert_eq!(&reply.bytes[8..24], &[0; 16]);
            assert_eq!(&reply.bytes[24..28], &(original.len() as u32).to_le_bytes());
            assert_eq!(&reply.bytes[28..], original);
        }
        assert_eq!(
            existing_response(B::Outcome(C::Complete(vec![1; 10_034])))
                .unwrap()
                .bytes
                .len(),
            10_062
        );
        assert_eq!(
            existing_response(B::Outcome(C::Complete(vec![1; 10_035])))
                .unwrap_err()
                .status,
            INTERNAL
        );
        assert_eq!(
            existing_response(B::Outcome(C::NotPerformed(
                advance::KagemushaWalletNotPerformedV1::CapacityWait
            )))
            .unwrap_err()
            .status,
            NOT_ENOUGH_STORAGE
        );
    }
    #[test]
    fn issuance_wait_is_tokenless_and_cannot_retain_a_review() {
        use state::ReviewMaintenanceWaitV1 as W;
        for (wait, detail) in [(W::SelectedPending, 1), (W::ArchiveFold, 2), (W::Busy, 6)] {
            let reply = waiting_response(wait);
            assert_eq!(
                (reply.kind, reply.sequence, reply.detail, reply.bytes.len()),
                (67, 0, detail, 0)
            );
        }
    }
    #[test]
    fn review_c_layout_binds_identity_and_expectation_before_originals() {
        use std::mem::{align_of, offset_of, size_of};
        let align = |n: usize, a: usize| (n + a - 1) & !(a - 1);
        let pointer = size_of::<*const u8>();
        let amount = align(8, align_of::<WalletU128>());
        let id = align(amount + size_of::<WalletU128>(), align_of::<*const u8>());
        assert_eq!(offset_of!(WalletReviewRequest, selector), 0);
        assert_eq!(offset_of!(WalletReviewRequest, expectation), 4);
        assert_eq!(offset_of!(WalletReviewRequest, amount), amount);
        assert_eq!(offset_of!(WalletReviewRequest, request_id32), id);
        assert_eq!(
            offset_of!(WalletReviewRequest, expected_projection),
            id + pointer
        );
        assert_eq!(
            offset_of!(WalletReviewRequest, expected_projection_length),
            id + 2 * pointer
        );
        assert_eq!(offset_of!(WalletReviewRequest, first), id + 3 * pointer);
        assert_eq!(
            offset_of!(WalletReviewRequest, first_length),
            id + 4 * pointer
        );
        assert_eq!(offset_of!(WalletReviewRequest, second), id + 5 * pointer);
        assert_eq!(
            offset_of!(WalletReviewRequest, second_length),
            id + 6 * pointer
        );
        assert_eq!(
            size_of::<WalletReviewRequest>(),
            align(id + 7 * pointer, align_of::<WalletReviewRequest>())
        );
    }
    #[test]
    fn consumed_tokens_never_replay_or_alias_another_review() {
        let mut first = Tokens::default();
        let one = first.put(String::from("opaque-one")).unwrap();
        let two = first.put(String::from("opaque-two")).unwrap();
        assert_eq!(first.take(one).unwrap(), "opaque-one");
        // Swift's post-execution discard must recognize this consumed-token
        // result without treating the still-live wallet owner as closed.
        assert_eq!(first.take(one).unwrap_err().status, INVALID);
        assert_eq!(first.take(two).unwrap(), "opaque-two");
        assert!(first.put(String::from("later")).unwrap() > two);
        assert!(Tokens::<String>::default().take(one).is_err());
    }
    #[test]
    fn outstanding_reviews_are_bounded_without_reusing_tokens() {
        let mut tokens = Tokens::default();
        let ids: Vec<_> = (0..MAX_REVIEWS).map(|i| tokens.put(i).unwrap()).collect();
        assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(tokens.put(9).unwrap_err().status, RESOURCE);
        assert_eq!(tokens.take(ids[2]).unwrap(), 2);
        assert!(tokens.put(9).unwrap() > *ids.last().unwrap());
        tokens.clear();
        for id in ids {
            assert!(tokens.take(id).is_err());
        }
        assert!(tokens.put(10).is_ok());
        let exhausted = std::sync::atomic::AtomicU64::new(i64::MAX as u64 - 1);
        assert_eq!(allocate_from(&exhausted).unwrap(), i64::MAX as u64);
        assert_eq!(allocate_from(&exhausted).unwrap_err().status, RESOURCE);
        assert_eq!(allocate_from(&exhausted).unwrap_err().status, RESOURCE);
    }
    #[test]
    fn maintenance_refusal_restores_the_same_taken_token_at_full_capacity() {
        let mut tokens = Tokens::default();
        let ids: Vec<_> = (0..MAX_REVIEWS)
            .map(|value| tokens.put(value).unwrap())
            .collect();
        let token = ids[2];
        let returned_review = tokens.take(token).unwrap();
        assert_eq!(tokens.values.len(), MAX_REVIEWS - 1);
        tokens.restore_exact(token, returned_review).unwrap();
        assert_eq!(tokens.values.len(), MAX_REVIEWS);
        assert_eq!(tokens.put(99).unwrap_err().status, RESOURCE);
        assert_eq!(tokens.take(token).unwrap(), 2);
        assert_eq!(tokens.take(token).unwrap_err().status, INVALID);
        for (index, id) in ids.into_iter().enumerate().filter(|(index, _)| *index != 2) {
            assert_eq!(tokens.take(id).unwrap(), index);
        }
    }
    #[test]
    fn exact_restore_refuses_collision_and_invalid_identity_without_replacing_a_review() {
        let mut tokens = Tokens::default();
        let original = tokens.put("original").unwrap();
        assert_eq!(
            tokens
                .restore_exact(original, "replacement")
                .unwrap_err()
                .status,
            INTERNAL
        );
        for invalid in [0, i64::MAX as u64 + 1, u64::MAX] {
            assert_eq!(
                tokens.restore_exact(invalid, "foreign").unwrap_err().status,
                INTERNAL
            );
        }
        assert_eq!(tokens.values.len(), 1);
        assert_eq!(tokens.take(original).unwrap(), "original");
    }
    #[test]
    fn retained_review_response_preserves_exact_origin_token_and_closed_reason() {
        use state::ReviewMaintenanceWaitV1 as W;
        let mut tokens = Tokens::default();
        let token = tokens.put(OriginBound::new(7, "actual review")).unwrap();
        for (wait, detail) in [(W::SelectedPending, 1), (W::ArchiveFold, 2), (W::Busy, 6)] {
            let response = retained_response(&tokens, 7, token, wait_detail(wait)).unwrap();
            assert_eq!(
                (
                    response.kind,
                    response.sequence,
                    response.detail,
                    response.bytes.len()
                ),
                (65, u128::from(token), detail, 0)
            );
        }
        for wait in [3, 4, 5] {
            assert_eq!(
                retained_response(&tokens, 7, token, wait)
                    .unwrap_err()
                    .status,
                INTERNAL
            );
        }
        for origin in [0, 8] {
            assert_eq!(
                retained_response(&tokens, origin, token, wait_detail(W::SelectedPending))
                    .unwrap_err()
                    .status,
                INVALID
            );
        }
        assert_eq!(tokens.take_bound(token, 7).unwrap(), "actual review");
        assert_eq!(
            retained_response(&tokens, 7, token, wait_detail(W::SelectedPending))
                .unwrap_err()
                .status,
            INVALID
        );
    }

    #[derive(Default)]
    struct HandoffFaults {
        admission: std::sync::atomic::AtomicBool,
        publication: std::sync::atomic::AtomicBool,
        discard: std::sync::atomic::AtomicUsize,
        fail_after_issue: std::sync::atomic::AtomicBool,
        last_issued: std::sync::atomic::AtomicU64,
        existing: std::sync::atomic::AtomicBool,
        maintenance_calls: std::sync::atomic::AtomicUsize,
    }
    struct HandoffWallet {
        base: crate::kagemusha_wallet_ffi::tests::TestWallet,
        reviews: Arc<Mutex<Tokens<OriginBound<&'static str>>>>,
        faults: Arc<HandoffFaults>,
        kind: CapabilityKind,
    }
    impl HandoffWallet {
        fn issue(&mut self, origin: u64) -> Result<Response> {
            use std::sync::atomic::Ordering;
            let token = self
                .reviews
                .lock()
                .unwrap()
                .put(OriginBound::new(origin, "new capability"))?;
            self.faults.last_issued.store(token, Ordering::SeqCst);
            if self.faults.fail_after_issue.load(Ordering::SeqCst) {
                self.faults.publication.store(true, Ordering::SeqCst);
                self.faults.admission.store(true, Ordering::SeqCst);
            }
            Ok(Response {
                kind: match self.kind {
                    CapabilityKind::Financial => 18,
                    CapabilityKind::Time => 13,
                    CapabilityKind::Deletion => 53,
                },
                sequence: u128::from(token),
                bytes: vec![7; 32],
                ..Response::default()
            })
        }
    }
    impl Wallet for HandoffWallet {
        fn deferred_receipt(&mut self) -> Result<maintenance::ReceiptStep> {
            Ok(maintenance::ReceiptStep::Empty)
        }
        fn deferred_wake_hint(&mut self) -> Result<maintenance::WakeHint> {
            Ok(maintenance::WakeHint::Age(
                state::DeferredArchiveWakeHintV1::NoAgeWake,
            ))
        }
        fn drain_plan(&mut self) -> Result<maintenance::DrainPlan> {
            Ok(maintenance::DrainPlan::Finished(
                state::DeferredArchiveDrainV1::NoWork,
            ))
        }

        fn maintenance(&mut self) -> Result<maintenance::View> {
            self.faults
                .maintenance_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(maintenance::View::Released)
        }
        fn require_custody_operations(&self) -> Result<()> {
            if self
                .faults
                .admission
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                Err(Failure::code(UNAVAILABLE))
            } else {
                Ok(())
            }
        }
        fn close_archive_call(&mut self, _succeeded: bool) -> Result<()> {
            if self
                .faults
                .publication
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                Err(Failure::code(UNCERTAIN))
            } else {
                Ok(())
            }
        }
        fn discard_capability(
            &mut self,
            kind: CapabilityKind,
            origin: u64,
            token: u64,
        ) -> Result<()> {
            if kind != self.kind {
                return Err(Failure::code(INVALID));
            }
            self.discard_review(origin, token)
        }
        fn review(&mut self, origin: u64, _input: Input) -> Result<Response> {
            assert_eq!(self.kind, CapabilityKind::Financial);
            if self
                .faults
                .existing
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                return existing_response(state::BoundRequestStatusV1::Preparing);
            }
            self.issue(origin)
        }
        fn execute_reviewed(&mut self, origin: u64, token: u64) -> Result<Response> {
            self.reviews.lock().unwrap().take_bound(token, origin)?;
            existing_response(state::BoundRequestStatusV1::Preparing)
        }
        fn discard_review(&mut self, origin: u64, token: u64) -> Result<()> {
            match self
                .faults
                .discard
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                1 => return Err(Failure::code(UNCERTAIN)),
                2 => panic!("injected in-memory review cleanup panic"),
                _ => {}
            }
            self.reviews
                .lock()
                .unwrap()
                .take_bound(token, origin)
                .map(drop)
        }
        fn setup(&mut self, origin: u64, input: setup::Setup) -> Result<Response> {
            match input {
                setup::Setup::BeginTime => {
                    assert_eq!(self.kind, CapabilityKind::Time);
                    self.issue(origin)
                }
                setup::Setup::ReviewCustodyDeletion => {
                    assert_eq!(self.kind, CapabilityKind::Deletion);
                    self.issue(origin)
                }
                input => self.base.setup(origin, input),
            }
        }
        fn snapshot(&mut self) -> Result<state::Snapshot> {
            self.base.snapshot()
        }
        fn execute(&mut self, request: state::OperationRequestV1) -> Result<Response> {
            self.base.execute(request)
        }
        fn request_status(&mut self, request: &[u8; 32]) -> Result<Response> {
            self.base.request_status(request)
        }
        fn retry(&mut self, operation: &[u8; 32]) -> Result<Response> {
            self.base.retry(operation)
        }
        fn has_pending_operation(&mut self) -> Result<bool> {
            self.base.has_pending_operation()
        }
        fn resume(&mut self) -> Result<Response> {
            self.base.resume()
        }
        fn fold(&mut self) -> Result<ControlFlow<Response, FoldProof>> {
            self.base.fold()
        }
        fn credit(&mut self, credit: &[u8; 32], payment: &[u8; 32]) -> Result<Response> {
            self.base.credit(credit, payment)
        }
    }
    fn handoff_wallet() -> (
        u64,
        u64,
        u64,
        u64,
        Arc<Mutex<Tokens<OriginBound<&'static str>>>>,
        Arc<HandoffFaults>,
    ) {
        handoff_wallet_kind(CapabilityKind::Financial)
    }
    fn handoff_wallet_kind(
        kind: CapabilityKind,
    ) -> (
        u64,
        u64,
        u64,
        u64,
        Arc<Mutex<Tokens<OriginBound<&'static str>>>>,
        Arc<HandoffFaults>,
    ) {
        use std::sync::atomic::AtomicUsize;
        let faults = Arc::new(HandoffFaults::default());
        let reviews = Arc::new(Mutex::new(Tokens::default()));
        let handle = install(
            Box::new(HandoffWallet {
                base: crate::kagemusha_wallet_ffi::tests::TestWallet {
                    calls: Arc::new(AtomicUsize::new(0)),
                    drops: Arc::new(AtomicUsize::new(0)),
                    expected_request: None,
                },
                reviews: Arc::clone(&reviews),
                faults: Arc::clone(&faults),
                kind,
            }),
            state::Scheduler::new(),
        )
        .unwrap();
        let origin = exchange::begin(handle).unwrap().delivered();
        let selected = reviews
            .lock()
            .unwrap()
            .put(OriginBound::new(origin, "returned review"))
            .unwrap();
        let other = reviews
            .lock()
            .unwrap()
            .put(OriginBound::new(origin, "unrelated review"))
            .unwrap();
        (handle, origin, selected, other, reviews, faults)
    }
    #[test]
    fn retained_c_review_fence_failure_discards_exact_review_after_exchange_end() {
        let (handle, origin, token, other, reviews, _faults) = handoff_wallet();
        let invocation = exchange::Invocation::admit(handle, origin).unwrap();
        assert_eq!(
            exchange::end(handle, origin).unwrap(),
            state::ExchangeEndV1::Ending
        );
        let delivery = CapabilityDelivery::financial(&invocation, token);
        // The C output path runs this exact fence after obtaining its Native response.
        let result = run_with_fence(
            || {
                let response = retained_response(&reviews.lock().unwrap(), origin, token, 1)?;
                Ok(response)
            },
            || Err(std::io::Error::other("injected late durability refusal")),
        );
        assert_eq!(result.unwrap_err().status, UNAVAILABLE);
        drop(delivery);
        assert!(reviews.lock().unwrap().get(token).is_none());
        assert_eq!(
            reviews.lock().unwrap().get(other).unwrap().value,
            "unrelated review"
        );
        invocation.validate().unwrap();
        drop(invocation);
        assert_eq!(
            exchange::end(handle, origin).unwrap(),
            state::ExchangeEndV1::AlreadyEnded
        );
        close(handle).unwrap();
    }
    #[test]
    fn existing_review_and_execution_classify_without_outer_maintenance_or_new_token() {
        use std::sync::atomic::Ordering;
        let (handle, origin, token, other, reviews, faults) = handoff_wallet();
        faults.existing.store(true, Ordering::SeqCst);
        let invocation = exchange::Invocation::admit(handle, origin).unwrap();
        let mut delivery = CapabilityDelivery::default();
        let existing = review(
            &invocation,
            fresh_request(8, 1, &[], &[]).unwrap(),
            &mut delivery,
        )
        .unwrap();
        assert_eq!(existing.kind, 66);
        assert_eq!(existing.sequence, 0);
        assert!(delivery.pending.is_none());
        assert_eq!(faults.last_issued.load(Ordering::SeqCst), 0);
        assert_eq!(reviews.lock().unwrap().values.len(), 2);
        let guard = CapabilityDelivery::financial(&invocation, token);
        let existing = execute_reviewed(&invocation, token).unwrap();
        assert_eq!(existing.kind, 66);
        assert_eq!(&existing.bytes[..4], &11_u32.to_le_bytes());
        drop(guard); // Already consumed: exact cleanup cannot touch another token.
        assert!(reviews.lock().unwrap().get(token).is_none());
        assert!(reviews.lock().unwrap().get(other).is_some());
        assert_eq!(faults.maintenance_calls.load(Ordering::SeqCst), 0);
        drop(invocation);
        close(handle).unwrap();
    }
    #[test]
    fn c_review_binds_id_and_mode_before_owner_and_execution_accepts_only_token() {
        use std::sync::atomic::Ordering;
        let (handle, origin, token, other, reviews, faults) = handoff_wallet();
        faults.existing.store(true, Ordering::SeqCst);
        let identity = [9; 32];
        let zero = [0; 32];
        let mut request = WalletReviewRequest {
            selector: 8,
            expectation: 0,
            amount: WalletU128 { low: 1, high: 0 },
            request_id32: identity.as_ptr(),
            expected_projection: std::ptr::null(),
            expected_projection_length: 0,
            first: std::ptr::null(),
            first_length: 0,
            second: std::ptr::null(),
            second_length: 0,
        };
        for (id, mode) in [
            (std::ptr::null(), 0),
            (zero.as_ptr(), 0),
            (identity.as_ptr(), 1),
            (identity.as_ptr(), 2),
        ] {
            request.request_id32 = id;
            request.expectation = mode;
            let mut out = WalletResult::default();
            assert_eq!(
                unsafe {
                    connect_norito_kagemusha_wallet_review_v1(handle, origin, &request, &mut out)
                },
                INVALID
            );
            assert!(out.bytes.is_null());
            assert_eq!(faults.last_issued.load(Ordering::SeqCst), 0);
            assert_eq!(reviews.lock().unwrap().values.len(), 2);
        }
        request.request_id32 = identity.as_ptr();
        request.expectation = 0;
        let mut out = WalletResult::default();
        assert_eq!(
            unsafe {
                connect_norito_kagemusha_wallet_review_v1(handle, origin, &request, &mut out)
            },
            0
        );
        assert_eq!(out.status, 66);
        crate::connect_norito_free(out.bytes);
        let mut out = WalletResult::default();
        assert_eq!(
            unsafe {
                connect_norito_kagemusha_wallet_execute_reviewed_v1(handle, origin, token, &mut out)
            },
            0
        );
        assert_eq!(out.status, 66);
        crate::connect_norito_free(out.bytes);
        assert!(reviews.lock().unwrap().get(token).is_none());
        assert!(reviews.lock().unwrap().get(other).is_some());
        assert_eq!(faults.maintenance_calls.load(Ordering::SeqCst), 0);
        close(handle).unwrap();
    }
    #[test]
    fn retained_jni_review_requires_successful_fence_and_response_object_handoff() {
        for (fence_succeeds, object_delivered) in [(false, true), (true, false), (true, true)] {
            let (handle, origin, token, other, reviews, _faults) = handoff_wallet();
            let invocation = exchange::Invocation::admit(handle, origin).unwrap();
            let mut delivery = CapabilityDelivery::financial(&invocation, token);
            let result = run_with_fence(
                || {
                    let response = retained_response(&reviews.lock().unwrap(), origin, token, 6)?;
                    Ok(response)
                },
                || {
                    if fence_succeeds {
                        Ok(())
                    } else {
                        Err(std::io::Error::other("injected late durability refusal"))
                    }
                },
            );
            // JNI may construct a non-null error object after fence failure. A null object
            // after native success likewise cannot tell the SDK to retain its original token.
            delivery.complete(result.is_ok() && object_delivered);
            drop(delivery);
            assert_eq!(
                reviews.lock().unwrap().get(token).is_some(),
                fence_succeeds && object_delivered
            );
            assert_eq!(
                reviews.lock().unwrap().get(other).unwrap().value,
                "unrelated review"
            );
            close(handle).unwrap();
        }
    }

    #[test]
    fn hidden_inner_review_publication_failure_cleans_without_custody_admission() {
        use std::sync::atomic::Ordering;
        let (handle, origin, token, other, reviews, faults) = handoff_wallet();
        let invocation = exchange::Invocation::admit(handle, origin).unwrap();
        let delivery = CapabilityDelivery::financial(&invocation, token);
        faults.publication.store(true, Ordering::SeqCst);
        // The actual owner/archive wrapper hides the genuine65 before an outer delivery
        // observer could see it. Cleanup is already armed from the accepted invocation.
        let hidden = with_wallet_owner(invocation.owner(), Priority::None, |_| {
            retained_response(&reviews.lock().unwrap(), origin, token, 6)
        });
        assert_eq!(hidden.unwrap_err().status, UNCERTAIN);
        faults.admission.store(true, Ordering::SeqCst);
        drop(delivery);
        assert!(reviews.lock().unwrap().get(token).is_none());
        assert_eq!(
            reviews.lock().unwrap().get(other).unwrap().value,
            "unrelated review"
        );
        invocation.validate().unwrap();
        // A foreign origin never removes the still-genuine review, even on failure cleanup.
        let foreign = exchange::begin(handle).unwrap().delivered();
        let foreign_call = exchange::Invocation::admit(handle, foreign).unwrap();
        drop(CapabilityDelivery::financial(&foreign_call, other));
        assert_eq!(
            reviews.lock().unwrap().get(other).unwrap().value,
            "unrelated review"
        );
        foreign_call.validate().unwrap();
        close(handle).unwrap();
    }
    #[test]
    fn failed_local_review_cleanup_fences_exact_owner_without_joining_accepted_child() {
        use std::sync::atomic::Ordering;
        for fault in [1, 2] {
            let (handle, origin, token, _other, _reviews, faults) = handoff_wallet();
            let invocation = exchange::Invocation::admit(handle, origin).unwrap();
            let selected = invocation.owner();
            faults.discard.store(fault, Ordering::SeqCst);
            drop(CapabilityDelivery::financial(&invocation, token));
            assert_eq!(invocation.validate().unwrap_err().status, CLOSED);
            assert_eq!(exchange::begin(handle).err().unwrap().status, CLOSED);
            assert!(
                registry()
                    .lock()
                    .unwrap()
                    .owners
                    .get(&handle)
                    .is_some_and(|held| Arc::ptr_eq(held, &selected))
            );
            assert!(
                selected
                    .wallet
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .is_some()
            );
            if fault == 2 {
                selected.wallet.clear_poison();
            } // Test-only repair of injected panic.
            close(handle).unwrap();
            assert!(selected.wallet.lock().unwrap().is_none());
        }
    }

    #[test]
    fn every_issued_capability_is_owned_before_inner_fence_and_foreign_delivery() {
        use std::sync::atomic::Ordering;
        for kind in [
            CapabilityKind::Financial,
            CapabilityKind::Time,
            CapabilityKind::Deletion,
        ] {
            for fault in 0..=3 {
                let (handle, origin, prior, other, tokens, faults) = handoff_wallet_kind(kind);
                let invocation = exchange::Invocation::admit(handle, origin).unwrap();
                let mut delivery = CapabilityDelivery::default();
                faults.fail_after_issue.store(fault == 0, Ordering::SeqCst);
                let result = run_with_fence(
                    || match kind {
                        CapabilityKind::Financial => review(
                            &invocation,
                            Input::Unload {
                                request_id: [1; 32],
                                expectation: state::ReviewExpectationV1::Fresh,
                                amount: 1,
                                charge: None,
                                beneficiary: None,
                            },
                            &mut delivery,
                        ),
                        CapabilityKind::Time => {
                            setup(&invocation, setup::Setup::BeginTime, &mut delivery)
                        }
                        CapabilityKind::Deletion => setup(
                            &invocation,
                            setup::Setup::ReviewCustodyDeletion,
                            &mut delivery,
                        ),
                    },
                    || {
                        if fault == 1 {
                            Err(std::io::Error::other(
                                "late issued-capability fence refusal",
                            ))
                        } else {
                            Ok(())
                        }
                    },
                );
                let issued = faults.last_issued.load(Ordering::SeqCst);
                assert!(issued > 0 && issued != prior && issued != other);
                let result = deliver(delivery, result, |result| (result, fault != 2));
                match fault {
                    0 => assert_eq!(result.unwrap_err().status, UNCERTAIN),
                    1 => assert_eq!(result.unwrap_err().status, UNAVAILABLE),
                    _ => assert!(result.is_ok()),
                }
                assert_eq!(tokens.lock().unwrap().get(issued).is_some(), fault == 3);
                assert_eq!(
                    tokens.lock().unwrap().get(prior).unwrap().value,
                    "returned review"
                );
                assert_eq!(
                    tokens.lock().unwrap().get(other).unwrap().value,
                    "unrelated review"
                );
                invocation.validate().unwrap();
                close(handle).unwrap();
            }
        }
    }
    #[test]
    fn c_time_challenge_allocation_refusal_discards_only_its_new_owned_token() {
        use std::sync::atomic::Ordering;
        let (handle, origin, prior, other, tokens, faults) =
            handoff_wallet_kind(CapabilityKind::Time);
        let zero = [0; 32];
        let request = exports::WalletSetupRequest {
            setup_id: zero.as_ptr(),
            selector: 4,
            amount: WalletU128 { low: 0, high: 0 },
            token: 0,
            first: std::ptr::null(),
            first_length: 0,
            second: std::ptr::null(),
            second_length: 0,
            third: std::ptr::null(),
            third_length: 0,
        };
        let mut output = WalletResult::default();
        let rc = unsafe {
            exports::setup_output_with(handle, origin, &request, &mut output, |value, bytes| {
                assert_eq!(value.status, 13);
                assert_eq!(bytes, [7; 32]);
                Err(())
            })
        };
        assert_eq!(rc, RESOURCE);
        assert_eq!(output.status, RESOURCE);
        assert!(output.bytes.is_null());
        let issued = faults.last_issued.load(Ordering::SeqCst);
        assert!(issued > 0);
        assert!(tokens.lock().unwrap().get(issued).is_none());
        assert!(tokens.lock().unwrap().get(prior).is_some());
        assert!(tokens.lock().unwrap().get(other).is_some());
        close(handle).unwrap();
    }

    #[test]
    fn exchange_origin_refuses_foreign_use_without_consuming_the_genuine_review() {
        let mut reviews = Tokens::default();
        let token = reviews
            .put(OriginBound::new(7, "exact Native review"))
            .unwrap();
        for foreign in [0, 8, u64::MAX] {
            assert_eq!(
                reviews.take_bound(token, foreign).unwrap_err().status,
                INVALID
            );
            assert_eq!(reviews.get(token).unwrap().value, "exact Native review");
        }
        let genuine = reviews.take_bound(token, 7).unwrap();
        reviews
            .restore_exact(token, OriginBound::new(7, genuine))
            .unwrap();
        assert_eq!(reviews.take_bound(token, 8).unwrap_err().status, INVALID);
        assert_eq!(reviews.take_bound(token, 7).unwrap(), "exact Native review");
        assert_eq!(reviews.take_bound(token, 7).unwrap_err().status, INVALID);
    }
    #[test]
    fn simultaneously_outstanding_owner_and_purpose_tokens_never_alias() {
        let mut a = Tokens::default();
        let mut b = Tokens::default();
        let a_token = a.put("deletion A").unwrap();
        let b_token = b.put("deletion B").unwrap();
        let time = allocate_token().unwrap();
        let mut financial = Tokens::default();
        let financial_token = financial.put("financial").unwrap();
        assert!(b.take(a_token).is_err());
        assert!(a.take(b_token).is_err());
        assert!(a.take(time).is_err());
        assert!(a.take(financial_token).is_err());
        assert_eq!(a.take(a_token).unwrap(), "deletion A");
        assert_eq!(b.take(b_token).unwrap(), "deletion B");
        assert_eq!(financial.take(financial_token).unwrap(), "financial");
    }
    #[test]
    fn raw_send_and_unload_cannot_reach_an_owner_without_review() {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let id = install(
            Box::new(super::super::tests::TestWallet {
                calls: Arc::clone(&calls),
                drops: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                expected_request: None,
            }),
            state::Scheduler::new(),
        )
        .unwrap();
        for action in [
            state::OperationActionV1::Send { request: vec![1] },
            state::OperationActionV1::Unload {
                amount: 1,
                charge: None,
            },
        ] {
            // A valid exchange still cannot bypass the genuine Native review.
            let error = execute(
                &test_invocation(id).unwrap(),
                requests::Call::Operation(state::OperationRequestV1 {
                    request_id: [1; 32],
                    action,
                }),
            )
            .unwrap_err();
            assert_eq!(error.status, INVALID);
        }
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        close(id).unwrap();
    }
    #[test]
    fn invalid_c_review_initializes_every_output_before_refusing() {
        let mut out = WalletResult::default();
        let status = unsafe {
            connect_norito_kagemusha_wallet_review_v1(
                0,
                test_exchange_id(0),
                std::ptr::null(),
                &mut out,
            )
        };
        assert_eq!(status, INVALID);
        assert_eq!(out.status, INVALID);
        assert_eq!(out.reason, -1);
        assert_eq!(out.sequence_low, 0);
        assert_eq!(out.sequence_high, 0);
        assert_eq!(out.detail, 0);
        assert!(out.bytes.is_null());
        assert_eq!(out.length, 0);
    }
}
