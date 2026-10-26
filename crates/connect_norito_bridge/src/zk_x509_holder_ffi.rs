//! Canonical public X.509 ownership-signing preparation for foreign-language wallets.
//!
//! This boundary accepts no subject key or private witness. Canonical preparation
//! does not authenticate State, establish readiness, prove, pair a host or submit.

use iroha_core_privacy::privacy_engines::zk_x509::holder;
use iroha_data_model::privacy::{
    PrivacyStatementV1, TAIRA_PRIVACY_MAX_STATEMENT_AND_ENCRYPTED_OUTPUT_BYTES_PER_TRANSACTION_V1,
};
use libc::{c_int, c_uchar, c_ulong};
use std::ptr;

pub(super) const MAX_STATEMENT_BYTES: usize =
    TAIRA_PRIVACY_MAX_STATEMENT_AND_ENCRYPTED_OUTPUT_BYTES_PER_TRANSACTION_V1 as usize;
pub(super) const NULL: c_int = -1;
pub(super) const EMPTY: c_int = -2;
pub(super) const TOO_LARGE: c_int = -3;
pub(super) const NON_CANONICAL: c_int = -4;
pub(super) const WRONG_PROTOCOL: c_int = -5;
pub(super) const PREPARATION: c_int = -6;
pub(super) const INTERNAL: c_int = -100;

#[derive(Debug)]
pub(super) struct Prepared {
    pub(super) message: Vec<u8>,
    pub(super) digest: [u8; 32],
}

pub(super) fn prepare(archive: &[u8]) -> Result<Prepared, c_int> {
    if archive.is_empty() {
        return Err(EMPTY);
    }
    if archive.len() > MAX_STATEMENT_BYTES {
        return Err(TOO_LARGE);
    }
    let base = norito::canonical_decode_limits(archive.len());
    let limits = norito::DecodeLimits::new(
        base.max_sequence_elements(),
        base.max_field_bytes(),
        base.max_total_elements(),
        base.max_total_allocated_bytes(),
        64,
    );
    let statement = norito::decode_canonical_with_limits::<PrivacyStatementV1>(archive, limits)
        .map_err(|_| NON_CANONICAL)?;
    let PrivacyStatementV1::IrohaZkX509StarkP256V1(statement) = statement else {
        return Err(WRONG_PROTOCOL);
    };
    let message = holder::ownership_signing_message_v1(&statement).map_err(|_| PREPARATION)?;
    let digest = holder::ownership_challenge_v1(&statement).map_err(|_| PREPARATION)?;
    if message.is_empty() || message.len() > MAX_STATEMENT_BYTES {
        return Err(PREPARATION);
    }
    Ok(Prepared { message, digest })
}

#[derive(Default)]
struct Output {
    pointer: *mut c_uchar,
    length: c_ulong,
}
impl Output {
    fn new(bytes: &[u8]) -> Result<Self, c_int> {
        let mut output = Self::default();
        // Both destinations belong to this local owner until both allocations succeed.
        unsafe { super::write_privacy_bytes(&mut output.pointer, &mut output.length, bytes)? };
        Ok(output)
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        super::iroha_privacy_free_buffer(self.pointer);
    }
}

/// Prepare the canonical message and its SHA-256 digest for one exact X.509 statement.
///
/// Pass the message to a P-256/SHA-256 message signer, or the digest to a prehash
/// signer. Normalize the signature to low-s fixed 64-byte `r || s` before witness
/// construction. Never message-sign the digest: that would apply SHA-256 twice.
/// This function accepts no signing key and performs no State authentication.
/// Both successful outputs must be released with [`super::iroha_privacy_free_buffer`].
/// Every failure leaves both outputs null with zero length.
///
/// # Safety
///
/// For a nonempty length at most 262144, input must be readable for that length.
/// Oversized inputs are rejected before reading their storage. Each output destination
/// must be a valid, writable, distinct location that does not overlap the input
/// or any other output. The caller owns input storage for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_zk_x509_prepare_ownership_signing_v1(
    statement_ptr: *const c_uchar,
    statement_len: c_ulong,
    out_message: *mut *mut c_uchar,
    out_message_len: *mut c_ulong,
    out_digest: *mut *mut c_uchar,
    out_digest_len: *mut c_ulong,
) -> c_int {
    super::clear_privacy_output(out_message, out_message_len);
    super::clear_privacy_output(out_digest, out_digest_len);
    if statement_ptr.is_null()
        || out_message.is_null()
        || out_message_len.is_null()
        || out_digest.is_null()
        || out_digest_len.is_null()
    {
        return NULL;
    }
    let Ok(length) = usize::try_from(statement_len) else {
        return TOO_LARGE;
    };
    if length > MAX_STATEMENT_BYTES {
        return TOO_LARGE;
    }
    std::panic::catch_unwind(|| -> Result<(), c_int> {
        let archive = unsafe { std::slice::from_raw_parts(statement_ptr, length) };
        let prepared = prepare(archive)?;
        let mut message = Output::new(&prepared.message)?;
        let mut digest = Output::new(&prepared.digest)?;
        // Publish only after both local allocation owners are ready.
        unsafe {
            *out_message = message.pointer;
            *out_message_len = message.length;
            *out_digest = digest.pointer;
            *out_digest_len = digest.length;
        }
        message.pointer = ptr::null_mut();
        digest.pointer = ptr::null_mut();
        Ok(())
    })
    .unwrap_or(Err(INTERNAL))
    .err()
    .unwrap_or(0)
}

#[cfg(test)]
mod tests;
