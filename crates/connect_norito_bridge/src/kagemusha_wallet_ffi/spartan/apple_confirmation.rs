//! iOS prompts are owned by Native, outside the exclusive wallet lock. The only public
//! request selects its already-frozen review. Foreign prompt text and DER are ineligible.
use super::*;

#[cfg(target_os = "ios")]
pub(super) struct Prompt {
    pub(super) token: u64,
    message: [u8; 32],
    generation: u64,
    signature: Option<Vec<u8>>,
}
#[cfg(target_os = "ios")]
impl Prompt {
    fn open(token: u64, message: &[u8; 32]) -> Result<Self> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let generation = NEXT
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| Failure::code(RESOURCE))?;
        unsafe extern "C" {
            fn kagemusha_apple_debit_open_prompt(generation: u64) -> i32;
        }
        // SAFETY: the function creates the OS context under this process-unique generation.
        let status = unsafe { kagemusha_apple_debit_open_prompt(generation) };
        if status != 0 {
            return Err(platform_error(status));
        }
        Ok(Self {
            token,
            message: *message,
            generation,
            signature: None,
        })
    }
    pub(super) fn matches(&self, token: u64, message: &[u8; 32]) -> bool {
        self.token == token && &self.message == message
    }
}
#[cfg(target_os = "ios")]
impl Drop for Prompt {
    fn drop(&mut self) {
        unsafe extern "C" {
            fn kagemusha_apple_debit_close_prompt(generation: u64);
        }
        // SAFETY: closing an exact generation is idempotent.
        unsafe { kagemusha_apple_debit_close_prompt(self.generation) };
    }
}
#[cfg(target_os = "ios")]
fn platform_error(status: i32) -> Failure {
    Failure::unavailable(
        UNAVAILABLE,
        advance::KagemushaWalletUnavailableV1::Platform(status),
    )
}
#[cfg(target_os = "ios")]
pub(crate) enum Update<'a> {
    Start,
    Signed(u64, &'a [u8]),
    Aborted(u64),
}

fn next_address<'a>(input: &mut &'a [u8]) -> Result<&'a [u8]> {
    let malformed = || Failure::code(INVALID);
    let (n, rest) = input.split_at_checked(2).ok_or_else(malformed)?;
    let length = u16::from_le_bytes(n.try_into().map_err(|_| malformed())?);
    let (address, remaining) = rest
        .split_at_checked(usize::from(length))
        .ok_or_else(malformed)?;
    *input = remaining;
    Ok(address)
}

fn terms(review: &[u8], asset: &[u8; 32]) -> Result<String> {
    let malformed = || Failure::code(INVALID);
    let amount = |offset: usize| -> Result<u128> {
        Ok(u128::from_le_bytes(
            review
                .get(offset..offset + 16)
                .ok_or_else(malformed)?
                .try_into()
                .map_err(|_| malformed())?,
        ))
    };
    match review.get(..3) {
        Some([1, 0, 3]) if review.len() == 291 => Ok(format!(
            "Send {} base units plus a fee of {} base units. Asset {}. Receiver account digest {}. Wallet {}.",
            amount(131)?,
            amount(147)?,
            hex::encode(asset),
            hex::encode(&review[195..227]),
            hex::encode(&review[163..195])
        )),
        Some([3, 0, 6])
            if (423..=native::MAX_UNLOAD_CLAIM_BYTES).contains(&review.len())
                && &review[131..163] == asset =>
        {
            let mut addresses = &review[419..];
            let account = next_address(&mut addresses)?;
            let payee = next_address(&mut addresses)?;
            let fee = amount(227)?;
            if account.is_empty() || !addresses.is_empty() || (fee > 0) != !payee.is_empty() {
                return Err(malformed());
            }
            let mut text = format!(
                "Unload {} base units. Net payout {} base units. Fee {fee} base units. Asset {}. Online account {}.",
                amount(211)?,
                amount(243)?,
                hex::encode(asset),
                hex::encode(account)
            );
            if fee != 0 {
                use std::fmt::Write as _;
                write!(text, " Fee recipient {}.", hex::encode(payee)).map_err(|_| malformed())?;
            }
            Ok(text)
        }
        _ => Err(malformed()),
    }
}

#[cfg(target_os = "ios")]
impl<P: advance::KagemushaWalletPlatformV1 + 'static, S> Session<P, S> {
    pub(super) fn apple_prompt(
        &mut self,
        token: u64,
        message: &[u8; 32],
        update: Update<'_>,
    ) -> Result<Response> {
        if let Update::Aborted(generation) = update {
            if self.apple_prompt.as_ref().is_some_and(|p| {
                p.matches(token, message) && p.generation == generation && p.signature.is_none()
            }) {
                self.apple_prompt = None;
            }
            return Ok(response(84, Vec::new()));
        }
        let review = self.reviews.active(token)?;
        if review
            .last_confirmation
            .as_ref()
            .is_some_and(|(held, _)| held == message)
        {
            return Ok(response(89, Vec::new()));
        }
        self.runtime
            .require_pending_debit_confirmation(&review.slot, &review.capsule, message)?;
        if self.apple_prompt.is_none() && matches!(update, Update::Start) {
            let (tag, point) =
                self.runtime
                    .apple_debit_prompt_key(&review.slot, &review.capsule, message)?;
            let mut input = review.slot.0.to_vec();
            input.extend_from_slice(&tag);
            input.extend_from_slice(point.as_ref());
            input.extend_from_slice(terms(&review.bytes, review.capsule.asset())?.as_bytes());
            let prompt = Prompt::open(token, message)?;
            let generation = prompt.generation;
            self.apple_prompt = Some(prompt);
            return Ok(Response {
                kind: 91,
                sequence: u128::from(generation),
                bytes: input,
                ..Response::default()
            });
        }
        let pending = self
            .apple_prompt
            .as_mut()
            .filter(|p| p.matches(token, message))
            .ok_or_else(|| Failure::code(CONFLICT))?;
        if let Update::Signed(generation, der) = update {
            if generation != pending.generation || pending.signature.is_some() {
                return Err(Failure::code(CONFLICT));
            }
            pending.signature = Some(der.to_vec());
        }
        let der = pending.signature.as_ref().ok_or_else(|| {
            Failure::unavailable(UNAVAILABLE, advance::KagemushaWalletUnavailableV1::Busy)
        })?;
        // Keep the same signed original across unavailable/uncertain publication. Successful
        // retention, its replay cache, and release of this prompt share the session lock.
        if let Err(error) = self.runtime.retain_signed_debit_confirmation(
            &review.slot,
            &review.capsule,
            message,
            der,
        ) {
            let failure = Failure::from(error);
            if matches!(failure.status, INVALID | PROOF_REJECTED | CUSTODY_LOST) {
                self.apple_prompt = None;
            }
            return Err(failure);
        }
        review.last_confirmation = Some((*message, der.clone()));
        self.apple_prompt = None;
        Ok(response(89, Vec::new()))
    }
}

#[cfg(target_os = "ios")]
struct InFlight<'a> {
    handle: u64,
    token: u64,
    message: &'a [u8; 32],
    generation: u64,
}
#[cfg(target_os = "ios")]
impl Drop for InFlight<'_> {
    fn drop(&mut self) {
        let _ = call(
            self.handle,
            Action::ApplePrompt {
                token: self.token,
                message: self.message,
                update: Update::Aborted(self.generation),
            },
        );
    }
}

#[cfg(target_os = "ios")]
pub(super) fn confirm(handle: u64, token: u64, message: &[u8; 32]) -> Result<Response> {
    let result = call(
        handle,
        Action::ApplePrompt {
            token,
            message,
            update: Update::Start,
        },
    )?;
    if result.kind == 89 {
        return Ok(result);
    }
    if result.kind != 91 {
        return Err(Failure::code(INTERNAL));
    }
    let generation = u64::try_from(result.sequence).map_err(|_| Failure::code(INTERNAL))?;
    let _cleanup = InFlight {
        handle,
        token,
        message,
        generation,
    };
    let (scope, terms) = result
        .bytes
        .split_at_checked(129)
        .ok_or_else(|| Failure::code(INTERNAL))?;
    let mut der = [0u8; 72];
    let mut written = 0usize;
    unsafe extern "C" {
        fn kagemusha_apple_debit_sign(
            generation: u64,
            slot: *const u8,
            tag: *const u8,
            pin: *const u8,
            message: *const u8,
            terms: *const u8,
            terms_len: usize,
            output: *mut u8,
            written: *mut usize,
        ) -> i32;
    }
    // SAFETY: Native just emitted slot32, tag32, pin65 and the exact terms. No wallet lock
    // is held during modality, and the caller-owned challenge has already been copied.
    let status = unsafe {
        kagemusha_apple_debit_sign(
            generation,
            scope.as_ptr(),
            scope[32..].as_ptr(),
            scope[64..].as_ptr(),
            message.as_ptr(),
            terms.as_ptr(),
            terms.len(),
            der.as_mut_ptr(),
            &mut written,
        )
    };
    if status != 0 {
        return Err(platform_error(status));
    }
    if !(8..=72).contains(&written) {
        return Err(Failure::code(INTERNAL));
    }
    call(
        handle,
        Action::ApplePrompt {
            token,
            message,
            update: Update::Signed(generation, &der[..written]),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_prompt_binds_full_amount_fee_asset_and_both_destination_digests() {
        let mut review = vec![0; 291];
        review[..3].copy_from_slice(&[1, 0, 3]);
        review[131..147].copy_from_slice(&u128::MAX.to_le_bytes());
        review[147..163].copy_from_slice(&19u128.to_le_bytes());
        review[163..195].fill(0xa1);
        review[195..227].fill(0xa2);
        let text = terms(&review, &[0xb3; 32]).unwrap();
        assert_eq!(
            text,
            format!(
                "Send {} base units plus a fee of 19 base units. Asset {}. Receiver account digest {}. Wallet {}.",
                u128::MAX,
                "b3".repeat(32),
                "a2".repeat(32),
                "a1".repeat(32)
            )
        );
        review.pop();
        assert!(terms(&review, &[0xb3; 32]).is_err());
    }

    fn unload(beneficiary: &[u8]) -> Vec<u8> {
        let mut review = vec![0; 419];
        review[..3].copy_from_slice(&[3, 0, 6]);
        review[131..163].fill(0xb3);
        review[211..227].copy_from_slice(&100u128.to_le_bytes());
        let fee = if beneficiary.is_empty() {
            0u128
        } else {
            15u128
        };
        review[227..243].copy_from_slice(&fee.to_le_bytes());
        review[243..259].copy_from_slice(&(100 - fee).to_le_bytes());
        for account in [&[0x41; 40][..], beneficiary] {
            review.extend_from_slice(&(account.len() as u16).to_le_bytes());
            review.extend_from_slice(account);
        }
        review
    }

    #[test]
    fn unload_prompt_binds_both_destinations_and_rejects_ambiguous_trailing_bytes() {
        let mut review = unload(&[0x42; 38]);
        assert_eq!(
            terms(&review, &[0xb3; 32]).unwrap(),
            format!(
                "Unload 100 base units. Net payout 85 base units. Fee 15 base units. Asset {}. Online account {}. Fee recipient {}.",
                "b3".repeat(32),
                "41".repeat(40),
                "42".repeat(38)
            )
        );
        assert!(terms(&review, &[0; 32]).is_err());
        review.push(0);
        assert!(terms(&review, &[0xb3; 32]).is_err());
        review.truncate(423);
        assert!(terms(&review, &[0xb3; 32]).is_err());
        let mut free = unload(&[]);
        assert_eq!(
            terms(&free, &[0xb3; 32]).unwrap(),
            format!(
                "Unload 100 base units. Net payout 100 base units. Fee 0 base units. Asset {}. Online account {}.",
                "b3".repeat(32),
                "41".repeat(40)
            )
        );
        free[227] = 1;
        assert!(terms(&free, &[0xb3; 32]).is_err());
    }
}
