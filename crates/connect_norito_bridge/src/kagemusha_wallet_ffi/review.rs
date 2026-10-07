//! Owner-local one-use financial review. Projection bytes are DATA, never authority.
use super::*;
use exports::{input, output};

pub(crate) const PROJECTION_BYTES: usize = 491;
const MAX_REVIEWS: usize = 8;
const MAGIC: &[u8; 8] = b"KWORV1\0\0";

/// One fixed request for Native to authenticate; no foreign state or financial verdict.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WalletReviewRequest {
    /// Send1 or Unload8 only.
    pub selector: u32,
    /// Unload face amount only; zero for Send.
    pub amount: WalletU128,
    /// Complete Request for Send, optional original charge quote for Unload.
    pub first: *const u8,
    /// Exact first-original length.
    pub first_length: usize,
    /// Original certificate set for a nonempty Unload charge; empty for Send.
    pub second: *const u8,
    /// Exact second-original length.
    pub second_length: usize,
}
pub(crate) enum Input {
    Send(Vec<u8>),
    Unload {
        amount: u128,
        charge: Option<state::ChargeOriginalsV1>,
    },
}
pub(crate) fn bounds(selector: u32) -> Result<[usize; 2]> {
    Ok(match selector {
        1 | 8 => {
            let limits = requests::bounds(selector)?;
            [limits[0], limits[1]]
        }
        _ => return Err(Failure::code(INVALID)),
    })
}
pub(crate) fn request(selector: u32, amount: u128, first: &[u8], second: &[u8]) -> Result<Input> {
    let b = bounds(selector)?;
    if first.len() > b[0] || second.len() > b[1] {
        return Err(Failure::code(INVALID));
    }
    Ok(match selector {
        1 if amount == 0 && !first.is_empty() && second.is_empty() => Input::Send(first.to_vec()),
        8 if amount != 0 && first.is_empty() == second.is_empty() => Input::Unload {
            amount,
            charge: (!first.is_empty()).then(|| state::ChargeOriginalsV1 {
                quote: first.to_vec(),
                certificates: second.to_vec(),
            }),
        },
        _ => return Err(Failure::code(INVALID)),
    })
}
/// This private owner-local table holds actual non-Clone Native capabilities.
/// It never imports a capability from its copyable projection or token.
pub(super) struct Tokens<T> {
    next: u64,
    values: BTreeMap<u64, T>,
}
impl<T> Default for Tokens<T> {
    fn default() -> Self {
        Self {
            next: 0,
            values: BTreeMap::new(),
        }
    }
}
impl<T> Tokens<T> {
    fn capacity(&self) -> Result<()> {
        if self.values.len() >= MAX_REVIEWS || self.next >= i64::MAX as u64 {
            Err(Failure::code(RESOURCE))
        } else {
            Ok(())
        }
    }
    fn put(&mut self, value: T) -> Result<u64> {
        self.capacity()?;
        let id = self.next.checked_add(1).ok_or(Failure::code(RESOURCE))?;
        self.next = id;
        self.values.insert(id, value);
        Ok(id)
    }
    pub(super) fn take(&mut self, token: u64) -> Result<T> {
        self.values.remove(&token).ok_or(Failure::code(INVALID))
    }
}
fn projection(value: &state::NativeOperationReviewV1) -> Result<Vec<u8>> {
    let selector = match value.kind {
        KagemushaWalletOperationKindV1::Send => 1,
        KagemushaWalletOperationKindV1::Unload => 8,
        _ => return Err(Failure::code(INTERNAL)),
    };
    let mut out = Vec::with_capacity(PROJECTION_BYTES);
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
    if out.len() != PROJECTION_BYTES {
        return Err(Failure::code(INTERNAL));
    }
    Ok(out)
}
impl<P: advance::KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send> NativeWallet<P, S> {
    pub(super) fn review_inner(&mut self, input: Input) -> Result<Response> {
        self.reviews.capacity()?;
        let review = match input {
            Input::Send(request) => self.wallet.review_send(&request)?,
            Input::Unload { amount, charge } => self.wallet.review_unload(amount, charge)?,
        };
        let bytes = projection(review.projection())?;
        let token = self.reviews.put(review)?;
        Ok(Response {
            kind: 18,
            sequence: u128::from(token),
            bytes,
            ..Response::default()
        })
    }
    pub(super) fn execute_reviewed_inner(
        &mut self,
        token: u64,
        request: [u8; 32],
    ) -> Result<Response> {
        // Consume even on an invalid identity, failed source recheck or failed execution.
        let reviewed = self.reviews.take(token)?;
        Ok(completion(Some(
            self.wallet.execute_reviewed(reviewed, request)?,
        )))
    }
}
pub(crate) fn review(id: u64, input: Input) -> Result<Response> {
    with_wallet(id, true, |wallet| wallet.review(input))
}
pub(crate) fn execute_reviewed(id: u64, token: u64, request: &[u8]) -> Result<Response> {
    let request = request.try_into().map_err(|_| Failure::code(INVALID))?;
    with_wallet(id, true, |wallet| wallet.execute_reviewed(token, request))
}
pub(crate) fn discard_review(id: u64, token: u64) -> Result<()> {
    with_wallet(id, true, |wallet| wallet.discard_review(token))
}

/// Authenticate Send/Unload and retain the genuine one-use review. No monetary operation.
/// Result18 contains exact491 DATA bytes and owner-local token in sequence_low.
/// # Safety
/// Caller supplies initialized inputs and writable result memory for stated lengths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_review_v1(
    handle: u64,
    request: *const WalletReviewRequest,
    out: *mut WalletResult,
) -> i32 {
    let mut retained = None;
    let status = unsafe {
        output(out, || {
            if request.is_null() {
                return Err(Failure::code(INVALID));
            }
            let req = &*request;
            let b = bounds(req.selector)?;
            let first = input(req.first, req.first_length, b[0])?;
            let second = input(req.second, req.second_length, b[1])?;
            let amount = u128::from(req.amount.low) | (u128::from(req.amount.high) << 64);
            let result = review(handle, self::request(req.selector, amount, first, second)?)?;
            retained = Some(result.sequence as u64);
            Ok(result)
        })
    };
    if status < 0 {
        if let Some(token) = retained {
            let _ = run(|| discard_review(handle, token));
        }
    }
    status
}
/// Consume the actual retained review after fresh hardware UI approval; source is rechecked.
/// No amount, destination, signature body or financial verdict is accepted from foreign code.
/// # Safety
/// request_id points to exactly32 initialized bytes; result is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_execute_reviewed_v1(
    handle: u64,
    token: u64,
    request_id: *const u8,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        output(out, || {
            let request = match input(request_id, 32, 32) {
                Ok(value) => value,
                Err(error) => {
                    let _ = discard_review(handle, token);
                    return Err(error);
                }
            };
            execute_reviewed(handle, token, request)
        })
    }
}
/// Drop one owner-local review on cancellation. Never affects durable monetary custody.
#[unsafe(no_mangle)]
pub extern "C" fn connect_norito_kagemusha_wallet_discard_review_v1(
    handle: u64,
    token: u64,
) -> i32 {
    run(|| discard_review(handle, token))
        .err()
        .map_or(0, |error| error.status)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn intake_refuses_other_operations_and_unused_authority() {
        assert!(request(1, 0, &[7], &[]).is_ok());
        assert!(request(1, 1, &[7], &[]).is_err());
        assert!(request(1, 0, &[7], &[8]).is_err());
        assert!(request(8, u128::MAX, &[], &[]).is_ok());
        assert!(request(8, 1, &[7], &[8]).is_ok());
        assert!(request(8, 1, &[7], &[]).is_err());
        assert!(request(8, 1, &[], &[8]).is_err());
        assert!(request(8, 0, &[], &[]).is_err());
        for selector in [0, 2, 3, 4, 5, 6, 7, 9, u32::MAX] {
            assert!(bounds(selector).is_err());
        }
        assert!(
            request(
                1,
                0,
                &vec![7; KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1 + 1],
                &[]
            )
            .is_err()
        );
    }
    #[test]
    fn consumed_tokens_never_replay_or_alias_another_review() {
        let mut first = Tokens::default();
        let one = first.put(String::from("opaque-one")).unwrap();
        let two = first.put(String::from("opaque-two")).unwrap();
        assert_eq!(first.take(one).unwrap(), "opaque-one");
        assert!(first.take(one).is_err());
        assert_eq!(first.take(two).unwrap(), "opaque-two");
        assert!(first.put(String::from("later")).unwrap() > two);
        assert!(Tokens::<String>::default().take(one).is_err());
    }
    #[test]
    fn outstanding_reviews_are_bounded_without_reusing_tokens() {
        let mut tokens = Tokens::default();
        for i in 1..=MAX_REVIEWS {
            assert_eq!(tokens.put(i).unwrap(), i as u64);
        }
        assert_eq!(tokens.put(9).unwrap_err().status, RESOURCE);
        assert_eq!(tokens.take(3).unwrap(), 3);
        assert_eq!(tokens.put(9).unwrap(), 9);
        tokens.next = i64::MAX as u64;
        assert_eq!(tokens.put(10).unwrap_err().status, RESOURCE);
    }
    #[test]
    fn raw_send_and_unload_cannot_reach_an_owner_without_review() {
        for action in [
            state::OperationActionV1::Send { request: vec![1] },
            state::OperationActionV1::Unload {
                amount: 1,
                charge: None,
            },
        ] {
            // INVALID is emitted before unknown-handle CLOSED, proving the ordinary
            // foreign execute route cannot reach a Native monetary owner for these actions.
            let error = execute(
                u64::MAX,
                state::OperationRequestV1 {
                    request_id: [1; 32],
                    action,
                },
            )
            .unwrap_err();
            assert_eq!(error.status, INVALID);
        }
    }
    #[test]
    fn invalid_c_review_initializes_every_output_before_refusing() {
        let mut out = WalletResult::default();
        let status =
            unsafe { connect_norito_kagemusha_wallet_review_v1(0, std::ptr::null(), &mut out) };
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
