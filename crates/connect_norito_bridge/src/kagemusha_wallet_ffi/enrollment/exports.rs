//! Bounded original E1 C input; native slot custody never crosses the ABI.
use super::*;

/// Original-only enrollment request; dates are retained Core DATA, never clock authority.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WalletEnrollmentRequest {
    /// 0 original begin/retry, 1 existing-intent resume, 2 request retention, 3 credential storage.
    pub selector: u32,
    /// Canonical Norito E1, at most1024 bytes.
    pub challenge: *const u8,
    /// Exact original extent.
    pub challenge_length: usize,
    /// Canonical signed-selection policy preimage, at most1024 bytes.
    pub policy: *const u8,
    /// Exact original extent.
    pub policy_length: usize,
    /// Existing canonical AccountId, at most4096 bytes.
    pub account: *const u8,
    /// Exact original extent.
    pub account_length: usize,
    /// Exact whole complete-request JSON (2) or initial credential (3); otherwise empty.
    pub original: *const u8,
    /// At most524288 for2 or1024 for3.
    pub original_length: usize,
    /// Rooted enrollment CertificateSet, only selector3; at most10000 bytes.
    pub certificates: *const u8,
    /// Exact original extent.
    pub certificates_length: usize,
    /// Original issuance time; never refreshed on retry.
    pub issued_at_ms: u64,
    /// Original expiration; issued < expires <= issued+600000.
    pub expires_at_ms: u64,
}
/// Enrollment DATA result; none of these statuses admits account or monetary use.
#[repr(C)]
#[derive(Debug)]
pub struct WalletEnrollmentResult {
    /// 0 Enrolled,1 Pending,2 Abandoned,3 RequestRetained,4 CredentialStored; negative failure.
    pub status: i32,
    /// Original platform reason or -1.
    pub reason: i32,
    /// Original platform code.
    pub platform_code: i32,
    /// SEC1 public payment key, only status0.
    pub payment_key: [u8; 65],
    /// Exactly65 for status0, otherwise0.
    pub payment_key_length: usize,
    /// Exact original Core account-signing frame, only status0.
    pub account_frame: [u8; 385],
    /// Exactly385 for status0, otherwise0.
    pub account_frame_length: usize,
    /// Model enrollment-key-binding digest, only status0.
    pub binding: [u8; 32],
    /// Exactly32 for status0, otherwise0.
    pub binding_length: usize,
    /// Canonical Norito Vec<Vec<u8>> original Android leaf-first DER chain; free with connect_norito_free.
    /// This is evidence DATA, not an attestation verification result. Empty on Apple/other statuses.
    pub chain: *mut u8,
    /// Exact initialized chain extent.
    pub chain_length: usize,
    /// Exact marker/request/credential original for0/3/4; free with connect_norito_free.
    pub bytes: *mut u8,
    /// Exact initialized original extent.
    pub length: usize,
}
impl Default for WalletEnrollmentResult {
    fn default() -> Self {
        Self {
            status: INTERNAL,
            reason: -1,
            platform_code: 0,
            payment_key: [0; 65],
            payment_key_length: 0,
            account_frame: [0; 385],
            account_frame_length: 0,
            binding: [0; 32],
            binding_length: 0,
            chain: std::ptr::null_mut(),
            chain_length: 0,
            bytes: std::ptr::null_mut(),
            length: 0,
        }
    }
}
fn output(response: Response) -> Result<WalletEnrollmentResult> {
    let mut value = WalletEnrollmentResult {
        status: response.kind,
        ..WalletEnrollmentResult::default()
    };
    if response.kind == 0 {
        value.payment_key = response
            .payment_key
            .as_slice()
            .try_into()
            .map_err(|_| Failure::code(INTERNAL))?;
        value.account_frame = response
            .account_frame
            .as_slice()
            .try_into()
            .map_err(|_| Failure::code(INTERNAL))?;
        value.binding = response
            .binding
            .as_slice()
            .try_into()
            .map_err(|_| Failure::code(INTERNAL))?;
        value.payment_key_length = 65;
        value.account_frame_length = 385;
        value.binding_length = 32;
    }
    let chain = if response.chain.is_empty() {
        vec![]
    } else {
        norito::encode_canonical(&response.chain).map_err(|_| Failure::code(INTERNAL))?
    };
    let mut chain_length = 0;
    // SAFETY: local initialized allocator outputs, released on any subsequent failure.
    unsafe { crate::write_bytes(&mut value.chain, &mut chain_length, &chain) }
        .map_err(|_| Failure::code(RESOURCE))?;
    value.chain_length = chain_length as usize;
    let mut length = 0;
    // SAFETY: local initialized allocator outputs, transferred only by a successful return.
    if unsafe { crate::write_bytes(&mut value.bytes, &mut length, &response.bytes) }.is_err() {
        crate::connect_norito_free(value.chain);
        return Err(Failure::code(RESOURCE));
    }
    value.length = length as usize;
    Ok(value)
}
/// Progress the sole installed Native provider; later steps resolve its existing intent.
/// # Safety
/// All buffers must be readable for declared lengths and result aligned/writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_enrollment_v1(
    runtime: u64,
    request: *const WalletEnrollmentRequest,
    result: *mut WalletEnrollmentResult,
) -> i32 {
    if result.is_null() {
        return INVALID;
    }
    // SAFETY: caller supplies writable aligned output; initialize before fallible work.
    unsafe {
        result.write(WalletEnrollmentResult::default());
    }
    let answer = run(|| {
        // SAFETY: the request and buffers satisfy the readable caller contract.
        let req = unsafe { request.as_ref() }.ok_or(Failure::code(INVALID))?;
        let input = unsafe {
            Input {
                selector: req.selector,
                challenge: super::super::exports::input(
                    req.challenge,
                    req.challenge_length,
                    BOUNDS[0],
                )?,
                policy: super::super::exports::input(req.policy, req.policy_length, BOUNDS[1])?,
                account: super::super::exports::input(req.account, req.account_length, BOUNDS[2])?,
                original: super::super::exports::input(
                    req.original,
                    req.original_length,
                    BOUNDS[3],
                )?,
                certificates: super::super::exports::input(
                    req.certificates,
                    req.certificates_length,
                    BOUNDS[4],
                )?,
                issued_at_ms: req.issued_at_ms,
                expires_at_ms: req.expires_at_ms,
            }
        };
        input.validate()?;
        output(open::enroll(runtime, input)?)
    });
    let value = match answer {
        Ok(value) => value,
        Err(error) => WalletEnrollmentResult {
            status: error.status,
            reason: error.reason,
            platform_code: error.platform_code,
            ..WalletEnrollmentResult::default()
        },
    };
    let status = if value.status < 0 { value.status } else { 0 };
    // SAFETY: writable result is still initialized without any previous owned output.
    unsafe {
        result.write(value);
    }
    status
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn null_input_initializes_every_owned_output_before_failure() {
        let mut value = WalletEnrollmentResult::default();
        value.payment_key.fill(7);
        value.account_frame.fill(7);
        value.binding.fill(7);
        // SAFETY: aligned writable result and deliberately null request.
        assert_eq!(
            unsafe {
                connect_norito_kagemusha_wallet_enrollment_v1(0, std::ptr::null(), &mut value)
            },
            INVALID
        );
        assert_eq!(value.status, INVALID);
        assert_eq!(value.payment_key, [0; 65]);
        assert_eq!(value.account_frame, [0; 385]);
        assert_eq!(value.binding, [0; 32]);
        assert!(value.chain.is_null() && value.bytes.is_null());
        assert_eq!((value.chain_length, value.length), (0, 0));
    }
}
