//! Named suffix APIs and opaque one-use reviews over the same native wallet owner.
use super::*;
use exports::{input, output};
use iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::{
    SUFFIX_UNLOAD_INSTRUCTION_MAX_BYTES_V1, SUFFIX_UNLOAD_SIGNED_WIRE_MAX_BYTES_V1,
    SuffixUnloadStatusV1,
};
pub(crate) const ACCOUNT_MAX: usize = 4096;
mod lifetime;
pub(crate) use lifetime::{ExchangeLifetime, SuffixReviewCell};

/// Additive C request. All three charge originals are absent together or present together.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WalletSuffixUnloadRequest {
    /// Stable nonzero request identity, pointing to exactly 32 bytes.
    pub request_id32: *const u8,
    /// Positive amount removed from the offline wallet, as low/high limbs.
    pub amount: WalletU128,
    /// Optional canonical signed charge quote; absent when its length is zero.
    pub charge_quote: *const u8,
    /// Length of the borrowed charge quote in bytes.
    pub charge_quote_length: usize,
    /// Optional canonical certificates authenticating the charge quote.
    pub charge_certificates: *const u8,
    /// Length of the borrowed charge certificate set in bytes.
    pub charge_certificates_length: usize,
    /// Optional canonical beneficiary account original bound to the charge.
    pub charge_beneficiary: *const u8,
    /// Length of the borrowed beneficiary account original in bytes.
    pub charge_beneficiary_length: usize,
}

pub(crate) enum Input {
    Prepare(state::SuffixUnloadRequestV1),
    Resume([u8; 32]),
}
pub(crate) enum Read {
    Status,
    Instruction {
        request: [u8; 32],
        ordinal: u128,
    },
    SignedWire {
        request: [u8; 32],
        ordinal: u128,
        attempt: u64,
    },
    Attempt {
        request: [u8; 32],
        ordinal: u128,
        wire: Vec<u8>,
    },
}
pub(crate) fn identity(bytes: &[u8]) -> Result<[u8; 32]> {
    let value: [u8; 32] = bytes.try_into().map_err(|_| Failure::code(INVALID))?;
    if value == [0; 32] {
        return Err(Failure::code(INVALID));
    }
    Ok(value)
}
pub(crate) fn request(
    id: &[u8],
    amount: u128,
    quote: &[u8],
    certificates: &[u8],
    beneficiary: &[u8],
) -> Result<Input> {
    if amount == 0
        || quote.len() > KAGEMUSHA_WALLET_CHARGE_QUOTE_MAX_BYTES_V1
        || certificates.len() > KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1
        || beneficiary.len() > ACCOUNT_MAX
    {
        return Err(Failure::code(INVALID));
    }
    let (charge, beneficiary) = match (
        quote.is_empty(),
        certificates.is_empty(),
        beneficiary.is_empty(),
    ) {
        (true, true, true) => (None, None),
        (false, false, false) => (
            Some(state::ChargeOriginalsV1 {
                quote: quote.to_vec(),
                certificates: certificates.to_vec(),
            }),
            Some(beneficiary.to_vec()),
        ),
        _ => return Err(Failure::code(INVALID)),
    };
    Ok(Input::Prepare(state::SuffixUnloadRequestV1 {
        request_id: identity(id)?,
        amount,
        charge,
        beneficiary,
    }))
}
pub(crate) fn ordinal(low: u64, high: u64) -> Result<u128> {
    let value = u128::from(low) | (u128::from(high) << 64);
    if value > 513 {
        return Err(Failure::code(INVALID));
    }
    Ok(value)
}
fn stage_outcome(value: state::SuffixStageOutcomeV1) -> Response {
    use state::SuffixStageOutcomeV1 as O;
    let (kind, height, bytes) = match value {
        O::Awaiting { entrypoint } => (46, 0, entrypoint),
        O::Rejected(commit) => (69, commit.height, commit.commit_hash),
        O::StageConfirmed(commit) => (76, commit.height, commit.commit_hash),
        O::UnloadConfirmed(commit) => (70, commit.height, commit.commit_hash),
    };
    Response {
        kind,
        sequence: u128::from(height),
        bytes: bytes.to_vec(),
        ..Response::default()
    }
}
fn wait_detail(wait: state::ReviewMaintenanceWaitV1) -> Result<u32> {
    match wait {
        state::ReviewMaintenanceWaitV1::SelectedPending => Ok(1),
        state::ReviewMaintenanceWaitV1::Busy => Ok(6),
        _ => Err(Failure::code(INTERNAL)),
    }
}
fn absent_response(status: SuffixUnloadStatusV1, requested: Option<[u8; 32]>) -> Result<Response> {
    if status.state != 7 || requested.is_none() || status.request_id != requested {
        return Err(Failure::code(INTERNAL));
    }
    Ok(Response {
        kind: 77,
        bytes: status
            .to_canonical_bytes()
            .map_err(|_| Failure::code(INTERNAL))?,
        ..Response::default()
    })
}

impl<P: advance::KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send> NativeWallet<P, S> {
    fn suffix_decision(
        &mut self,
        origin: u64,
        value: state::SuffixUnloadDecisionV1,
        lifetime: &ExchangeLifetime,
        absent_request: Option<[u8; 32]>,
    ) -> Result<Response> {
        use state::SuffixUnloadDecisionV1 as D;
        lifetime.require_live()?;
        // Empty cells retain no authority after exchange end; reclaim only suffix entries.
        self.reviews.prune_ended_suffix();
        Ok(match value {
            D::NotStarted(status) => absent_response(status, absent_request)?,
            D::NotApplicable(reason) => Response {
                kind: 71,
                detail: reason as u32,
                ..Response::default()
            },
            D::Verifying(progress) => Response {
                kind: 72,
                bytes: progress
                    .to_canonical_bytes()
                    .map_err(|_| Failure::code(INTERNAL))?,
                ..Response::default()
            },
            D::Ready(review) => {
                let bytes = review
                    .projection()
                    .to_canonical_bytes()
                    .map_err(|_| Failure::code(INTERNAL))?;
                let token = self.reviews.put(review::OriginBound::new(
                    origin,
                    review::FinancialReview::Suffix(lifetime.review(review)?),
                ))?;
                Response {
                    kind: 73,
                    sequence: u128::from(token),
                    bytes,
                    ..Response::default()
                }
            }
            D::Released(value) => Response {
                kind: 74,
                bytes: value
                    .to_canonical_bytes()
                    .map_err(|_| Failure::code(INTERNAL))?,
                ..Response::default()
            },
            D::Pending => Response {
                kind: 2,
                ..Response::default()
            },
            D::Waiting(wait) => Response {
                kind: 67,
                detail: wait_detail(wait)?,
                ..Response::default()
            },
        })
    }
    pub(super) fn suffix_prepare_inner(
        &mut self,
        origin: u64,
        lifetime: ExchangeLifetime,
        input: Input,
    ) -> Result<ControlFlow<Response, FoldProof>> {
        self.require_custody_operations()?;
        lifetime.require_live()?;
        let absent_request = match &input {
            Input::Resume(request) => Some(*request),
            Input::Prepare(_) => None,
        };
        let prepared = match input {
            Input::Prepare(request) => self.wallet.suffix_unload_prepare(request)?,
            Input::Resume(request) => self.wallet.suffix_unload_resume(&request)?,
        };
        Ok(match prepared {
            ControlFlow::Break(value) => ControlFlow::Break(self.suffix_decision(
                origin,
                value,
                &lifetime,
                absent_request,
            )?),
            ControlFlow::Continue(job) => {
                let registration = lifetime.register_job(job.cancellation())?;
                // The bounded job owns pinned originals and a private verification cursor.
                // It holds no wallet, archive or original-store mutex while proving.
                let proof: FoldProof = Box::new(move || {
                    registration.check()?;
                    let result = job.verify()?;
                    registration.check()?;
                    Ok(Box::new(move |wallet: &mut dyn Wallet| {
                        registration.check()?;
                        wallet.suffix_publish(origin, lifetime, result)
                    }))
                });
                ControlFlow::Continue(proof)
            }
        })
    }
    pub(super) fn suffix_publish_inner(
        &mut self,
        origin: u64,
        lifetime: ExchangeLifetime,
        proved: state::ProvedSuffixPreparationV1,
    ) -> Result<Response> {
        self.require_custody_operations()?;
        lifetime.require_live()?;
        let value = self.wallet.suffix_unload_publish(proved)?;
        self.suffix_decision(origin, value, &lifetime, None)
    }
    pub(super) fn suffix_execute_inner(&mut self, origin: u64, token: u64) -> Result<Response> {
        // Wrong-kind execution consumes this exact origin-bound token and rejects it.
        let review::FinancialReview::Suffix(reviewed) = self.reviews.take_bound(token, origin)?
        else {
            return Err(Failure::code(INVALID));
        };
        let lifetime = reviewed.lifetime();
        let reviewed = reviewed.take()?;
        match self.wallet.execute_reviewed_suffix_unload(reviewed) {
            Ok(state::SuffixUnloadExecutionV1::Released(value)) => Ok(Response {
                kind: 74,
                bytes: value
                    .to_canonical_bytes()
                    .map_err(|_| Failure::code(INTERNAL))?,
                ..Response::default()
            }),
            Ok(state::SuffixUnloadExecutionV1::Pending) => Ok(Response {
                kind: 2,
                ..Response::default()
            }),
            Err(failure) => {
                let wait = failure.maintenance_wait();
                let (error, retained) = failure.into_parts();
                match (wait, retained) {
                    (Some(wait), Some(reviewed)) => {
                        let detail = wait_detail(wait)?;
                        self.reviews.restore_exact(
                            token,
                            review::OriginBound::new(
                                origin,
                                review::FinancialReview::Suffix(lifetime.review(reviewed)?),
                            ),
                        )?;
                        review::retained_response(&self.reviews, origin, token, detail)
                    }
                    (None, None) => Err(error.into()),
                    _ => Err(Failure::code(INTERNAL)),
                }
            }
        }
    }
    pub(super) fn suffix_read_inner(&mut self, input: Read) -> Result<Response> {
        self.require_custody_operations()?;
        Ok(match input {
            Read::Status => {
                let status = self.wallet.suffix_unload_status()?;
                if status.state > 6 {
                    return Err(Failure::code(INTERNAL));
                }
                Response {
                    kind: 77,
                    bytes: status
                        .to_canonical_bytes()
                        .map_err(|_| Failure::code(INTERNAL))?,
                    ..Response::default()
                }
            }
            Read::Instruction { request, ordinal } => {
                let bytes = self.wallet.suffix_unload_instruction(request, ordinal)?;
                if bytes.is_empty() || bytes.len() > SUFFIX_UNLOAD_INSTRUCTION_MAX_BYTES_V1 {
                    return Err(Failure::code(INTERNAL));
                }
                Response {
                    kind: 75,
                    bytes,
                    ..Response::default()
                }
            }
            Read::SignedWire {
                request,
                ordinal,
                attempt,
            } => {
                let bytes = self
                    .wallet
                    .suffix_unload_signed_wire(request, ordinal, attempt)?
                    .unwrap_or_default();
                if bytes.len() > SUFFIX_UNLOAD_SIGNED_WIRE_MAX_BYTES_V1 {
                    return Err(Failure::code(INTERNAL));
                }
                Response {
                    kind: 78,
                    bytes,
                    ..Response::default()
                }
            }
            Read::Attempt {
                request,
                ordinal,
                wire,
            } => stage_outcome(
                self.wallet
                    .suffix_unload_attempt_outcome(request, ordinal, &wire)?,
            ),
        })
    }
    pub(super) fn suffix_finality_inner(
        &mut self,
        request: [u8; 32],
        ordinal: u128,
        wire: &[u8],
        finality: &[u8],
    ) -> Result<Response> {
        Ok(
            match self
                .wallet
                .suffix_unload_ingest_finality(request, ordinal, wire, finality)?
            {
                state::SuffixStageIngestV1::Outcome(value) => stage_outcome(value),
                state::SuffixStageIngestV1::EpochRequired { target_height } => Response {
                    kind: 68,
                    sequence: u128::from(target_height),
                    ..Response::default()
                },
            },
        )
    }
}

pub(crate) fn prepare(
    invocation: &exchange::Invocation,
    input: Input,
    delivery: &mut review::CapabilityDelivery,
) -> Result<Response> {
    invocation.validate()?;
    let owner = invocation.owner();
    let lifetime = invocation.suffix_lifetime()?;
    let prepared = with_wallet_owner(Arc::clone(&owner), Priority::Ingest, |wallet| {
        let value = wallet.suffix_prepare(invocation.origin(), lifetime, input)?;
        if let ControlFlow::Break(response) = &value {
            delivery.issued(invocation, response);
        }
        Ok(value)
    })?;
    let response = match prepared {
        ControlFlow::Break(response) => response,
        ControlFlow::Continue(proof) => {
            let publish = proof()?;
            with_wallet_owner(Arc::clone(&owner), Priority::Ingest, |wallet| {
                let response = publish(wallet)?;
                delivery.issued(invocation, &response);
                Ok(response)
            })?
        }
    };
    if response.kind == 67 {
        owner.background.retry(&owner)?;
    } else {
        owner.background.notify();
    }
    Ok(response)
}
pub(crate) fn execute(invocation: &exchange::Invocation, token: u64) -> Result<Response> {
    invocation.validate()?;
    let owner = invocation.owner();
    let response = with_wallet_owner(Arc::clone(&owner), Priority::None, |wallet| {
        wallet.suffix_execute(invocation.origin(), token)
    })?;
    if response.kind == 65 {
        owner.background.retry(&owner)?;
    } else {
        owner.background.notify();
    }
    Ok(response)
}
pub(crate) fn read(handle: u64, input: Read) -> Result<Response> {
    with_wallet_owner(owner(handle)?, Priority::Ingest, |wallet| {
        wallet.suffix_read(input)
    })
}
pub(crate) fn ingest(
    invocation: &exchange::Invocation,
    request: [u8; 32],
    ordinal: u128,
    wire: &[u8],
    finality: &[u8],
) -> Result<Response> {
    invocation.validate()?;
    if wire.is_empty()
        || wire.len() > SUFFIX_UNLOAD_SIGNED_WIRE_MAX_BYTES_V1
        || finality.is_empty()
        || finality.len() > KAGEMUSHA_WALLET_LEDGER_FINALITY_MAX_BYTES_V1
    {
        return Err(Failure::code(INVALID));
    }
    with_wallet_owner(invocation.owner(), Priority::Ingest, |wallet| {
        wallet.suffix_finality(request, ordinal, wire, finality)
    })
}

/// Read bounded authenticated scheduling DATA. No proof, scan or network operation.
/// # Safety
/// `out` must point to writable initialized result storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_suffix_unload_status_v1(
    handle: u64,
    out: *mut WalletResult,
) -> i32 {
    unsafe { output(out, || read(handle, Read::Status)) }
}
/// Prepare one bounded pass. All request pointers are borrowed only during this call.
/// # Safety
/// All nonempty inputs and writable `out` must satisfy their stated lengths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_suffix_unload_prepare_v1(
    handle: u64,
    exchange_id: u64,
    req: *const WalletSuffixUnloadRequest,
    out: *mut WalletResult,
) -> i32 {
    let mut delivery = review::CapabilityDelivery::default();
    let mut admitted = None;
    let status = unsafe {
        output(out, || {
            if req.is_null() {
                return Err(Failure::code(INVALID));
            }
            let req = &*req;
            let value = request(
                input(req.request_id32, 32, 32)?,
                u128::from(req.amount.low) | (u128::from(req.amount.high) << 64),
                input(
                    req.charge_quote,
                    req.charge_quote_length,
                    KAGEMUSHA_WALLET_CHARGE_QUOTE_MAX_BYTES_V1,
                )?,
                input(
                    req.charge_certificates,
                    req.charge_certificates_length,
                    KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1,
                )?,
                input(
                    req.charge_beneficiary,
                    req.charge_beneficiary_length,
                    ACCOUNT_MAX,
                )?,
            )?;
            admitted = Some(exchange::Invocation::admit(handle, exchange_id)?);
            prepare(
                admitted.as_ref().ok_or(Failure::code(INTERNAL))?,
                value,
                &mut delivery,
            )
        })
    };
    delivery.complete(status == 0);
    status
}
/// Resume retained preparation or selected Advance; never performs a fresh unreviewed debit.
/// # Safety
/// Request addresses32 bytes and `out` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_suffix_unload_resume_v1(
    handle: u64,
    exchange_id: u64,
    request_id32: *const u8,
    out: *mut WalletResult,
) -> i32 {
    let mut delivery = review::CapabilityDelivery::default();
    let mut admitted = None;
    let status = unsafe {
        output(out, || {
            let request = identity(input(request_id32, 32, 32)?)?;
            admitted = Some(exchange::Invocation::admit(handle, exchange_id)?);
            prepare(
                admitted.as_ref().ok_or(Failure::code(INTERNAL))?,
                Input::Resume(request),
                &mut delivery,
            )
        })
    };
    delivery.complete(status == 0);
    status
}
/// Consume an exact Native token after fresh hardware confirmation.
/// # Safety
/// `out` is writable; no caller financial terms are accepted here.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_suffix_unload_execute_reviewed_v1(
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
            let invocation = admitted.as_ref().ok_or(Failure::code(INTERNAL))?;
            delivery = Some(review::CapabilityDelivery::financial(invocation, token));
            let response = execute(invocation, token)?;
            retained = response.kind == 65;
            Ok(response)
        })
    };
    if let Some(delivery) = delivery.as_mut() {
        delivery.complete(status == 0 && retained);
    }
    status
}
/// Return one exact retained ledger instruction.
/// # Safety
/// Request addresses32 bytes and `out` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_suffix_unload_instruction_v1(
    handle: u64,
    request_id32: *const u8,
    ordinal_low: u64,
    ordinal_high: u64,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        output(out, || {
            read(
                handle,
                Read::Instruction {
                    request: identity(input(request_id32, 32, 32)?)?,
                    ordinal: ordinal(ordinal_low, ordinal_high)?,
                },
            )
        })
    }
}
/// Read back an exact retained signed attempt; only an absent index produces empty bytes.
/// # Safety
/// Request addresses32 bytes and `out` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_suffix_unload_signed_wire_v1(
    handle: u64,
    request_id32: *const u8,
    ordinal_low: u64,
    ordinal_high: u64,
    attempt_index: u64,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        output(out, || {
            read(
                handle,
                Read::SignedWire {
                    request: identity(input(request_id32, 32, 32)?)?,
                    ordinal: ordinal(ordinal_low, ordinal_high)?,
                    attempt: attempt_index,
                },
            )
        })
    }
}
/// Retain the exact signed wire before the application dispatches it.
/// # Safety
/// All nonempty inputs and writable `out` satisfy their stated lengths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_suffix_unload_attempt_outcome_v1(
    handle: u64,
    request_id32: *const u8,
    ordinal_low: u64,
    ordinal_high: u64,
    signed_wire: *const u8,
    signed_wire_length: usize,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        output(out, || {
            let wire = input(
                signed_wire,
                signed_wire_length,
                SUFFIX_UNLOAD_SIGNED_WIRE_MAX_BYTES_V1,
            )?;
            if wire.is_empty() {
                return Err(Failure::code(INVALID));
            }
            read(
                handle,
                Read::Attempt {
                    request: identity(input(request_id32, 32, 32)?)?,
                    ordinal: ordinal(ordinal_low, ordinal_high)?,
                    wire: wire.to_vec(),
                },
            )
        })
    }
}
/// Verify finality with the signed validators; Begin/Append success is never payout.
/// # Safety
/// All nonempty inputs and writable `out` satisfy their stated lengths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_suffix_unload_ingest_finality_v1(
    handle: u64,
    exchange_id: u64,
    request_id32: *const u8,
    ordinal_low: u64,
    ordinal_high: u64,
    signed_wire: *const u8,
    signed_wire_length: usize,
    finality: *const u8,
    finality_length: usize,
    out: *mut WalletResult,
) -> i32 {
    let mut admitted = None;
    unsafe {
        output(out, || {
            let request = identity(input(request_id32, 32, 32)?)?;
            let ordinal = ordinal(ordinal_low, ordinal_high)?;
            let wire = input(
                signed_wire,
                signed_wire_length,
                SUFFIX_UNLOAD_SIGNED_WIRE_MAX_BYTES_V1,
            )?;
            let finality = input(
                finality,
                finality_length,
                KAGEMUSHA_WALLET_LEDGER_FINALITY_MAX_BYTES_V1,
            )?;
            admitted = Some(exchange::Invocation::admit(handle, exchange_id)?);
            ingest(
                admitted.as_ref().ok_or(Failure::code(INTERNAL))?,
                request,
                ordinal,
                wire,
                finality,
            )
        })
    }
}

#[cfg(test)]
mod tests;
