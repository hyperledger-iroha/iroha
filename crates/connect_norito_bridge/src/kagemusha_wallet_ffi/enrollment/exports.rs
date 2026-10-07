//! Bounded enrollment C originals and initialized ownership-free result DATA.
use super::*;

/// Original-only enrollment request; no supplied field grants freshness or custody.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WalletEnrollmentRequest {
    /// 0 begin/retry exact E1, 1 resume exact slot, 2 retain request, 3 store credential.
    pub selector: u32,
    /// Empty for begin; exact returned slot for all later steps.
    pub slot: *const u8,
    /// Slot extent: zero or exactly 32.
    pub slot_length: usize,
    /// Canonical E1 original acquired through the authenticated issuer endpoint.
    pub challenge: *const u8,
    /// Exact E1 extent, at most 1024.
    pub challenge_length: usize,
    /// Canonical enrollment-policy preimage selected by the signed installation digest.
    pub policy: *const u8,
    /// Exact policy extent, at most 1024.
    pub policy_length: usize,
    /// Existing canonical single-Ed25519 AccountId original.
    pub account: *const u8,
    /// Exact account extent, at most 4096.
    pub account_length: usize,
    /// Exact request (selector2), initial credential (selector3), otherwise empty.
    pub original: *const u8,
    /// Exact original extent: request at most 131072, credential at most 1024.
    pub original_length: usize,
    /// Initial credential's original Enrollment CertificateSet, only selector3.
    pub certificates: *const u8,
    /// Exact certificate-set extent, at most 10000.
    pub certificates_length: usize,
}
/// Enrollment result DATA; no status means account admission or monetary completion.
#[repr(C)]
#[derive(Debug)]
pub struct WalletEnrollmentResult {
    /// 0 Enrolled, 1 Pending, 2 SlotAbandoned, 3 RequestRetained, 4 CredentialStored; negative failure.
    pub status: i32,
    /// Original platform reason, or -1.
    pub reason: i32,
    /// Original platform error code.
    pub platform_code: i32,
    /// Native-selected exact slot, all zero on failure.
    pub slot: [u8; 32],
    /// Enrolled payment public key as SEC1; zero-filled for other outcomes.
    pub payment_key: [u8; 65],
    /// Exactly65 for Enrolled, otherwise zero.
    pub payment_key_length: usize,
    /// Exact marker/request/credential DATA for statuses0/3/4; free with connect_norito_free.
    pub bytes: *mut u8,
    /// Exact initialized output extent; zero for all other statuses.
    pub length: usize,
}
impl Default for WalletEnrollmentResult {
    fn default() -> Self {
        Self {
            status: INTERNAL,
            reason: -1,
            platform_code: 0,
            slot: [0; 32],
            payment_key: [0; 65],
            payment_key_length: 0,
            bytes: std::ptr::null_mut(),
            length: 0,
        }
    }
}
/// Progress the same installed Native runtime's actual enrollment custody.
/// # Safety
/// All original buffers must remain readable for declared lengths; result must be aligned/writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_enrollment_v1(
    runtime: u64,
    request: *const WalletEnrollmentRequest,
    result: *mut WalletEnrollmentResult,
) -> i32 {
    if result.is_null() {
        return INVALID;
    }
    // SAFETY: caller supplies writable output, initialized before any fallible work.
    unsafe {
        result.write(WalletEnrollmentResult::default());
    }
    let answer = run(|| {
        // SAFETY: request and declared input buffers satisfy the export's readable contract.
        let request = unsafe { request.as_ref() }.ok_or(Failure::code(INVALID))?;
        let input = unsafe {
            Input {
                selector: request.selector,
                slot: super::super::exports::input(request.slot, request.slot_length, BOUNDS[0])?,
                challenge: super::super::exports::input(
                    request.challenge,
                    request.challenge_length,
                    BOUNDS[1],
                )?,
                policy: super::super::exports::input(
                    request.policy,
                    request.policy_length,
                    BOUNDS[2],
                )?,
                account: super::super::exports::input(
                    request.account,
                    request.account_length,
                    BOUNDS[3],
                )?,
                original: super::super::exports::input(
                    request.original,
                    request.original_length,
                    BOUNDS[4],
                )?,
                certificates: super::super::exports::input(
                    request.certificates,
                    request.certificates_length,
                    BOUNDS[5],
                )?,
            }
        };
        input.validate()?;
        open::enroll(runtime, input)
    });
    let mut value = WalletEnrollmentResult::default();
    match answer {
        Ok(response) => {
            value.status = response.kind;
            value.slot = response.slot;
            if !response.payment_key.is_empty() {
                let Ok(key) = response.payment_key.as_slice().try_into() else {
                    return INTERNAL;
                };
                value.payment_key = key;
                value.payment_key_length = 65;
            }
            if !response.bytes.is_empty() {
                let mut length = 0;
                // SAFETY: initialized local allocator outputs, released by the documented bridge free.
                if unsafe { crate::write_bytes(&mut value.bytes, &mut length, &response.bytes) }
                    .is_err()
                {
                    return RESOURCE;
                }
                value.length = length as usize;
            }
        }
        Err(error) => {
            value.status = error.status;
            value.reason = error.reason;
            value.platform_code = error.platform_code;
        }
    }
    let status = if value.status < 0 { value.status } else { 0 };
    // SAFETY: admitted writable result, not previously populated with owned output.
    unsafe {
        result.write(value);
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_request_initializes_every_output_without_a_runtime() {
        let mut result = WalletEnrollmentResult::default();
        result.slot = [9; 32];
        result.payment_key_length = 123;
        // SAFETY: null request is an explicitly rejected input; output is writable.
        assert_eq!(
            unsafe {
                connect_norito_kagemusha_wallet_enrollment_v1(0, std::ptr::null(), &mut result)
            },
            INVALID
        );
        assert_eq!(result.status, INVALID);
        assert_eq!(result.slot, [0; 32]);
        assert_eq!(result.payment_key, [0; 65]);
        assert_eq!(result.payment_key_length, 0);
        assert_eq!(result.length, 0);
        assert!(result.bytes.is_null());
    }
}
