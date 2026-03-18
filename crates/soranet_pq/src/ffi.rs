//! C-friendly bindings for the `soranet_pq` primitives.

use core::{
    convert::TryFrom,
    ffi::{c_int, c_uchar, c_uint, c_ulong},
    slice,
};

use crate::{
    MlDsaSuite, MlKemSuite, decapsulate_mlkem, encapsulate_mlkem, generate_mldsa_keypair,
    generate_mlkem_keypair, mldsa::MlDsaError, mlkem::MlKemError, sign_mldsa, verify_mldsa,
};

const ERR_INVALID_SUITE: c_int = -1;
const ERR_NULL_POINTER: c_int = -2;
const ERR_LENGTH_MISMATCH: c_int = -3;
const ERR_ENCODING: c_int = -4;
const ERR_KEYGEN: c_int = -5;
const ERR_VERIFICATION_FAILED: c_int = -6;

fn mlkem_suite_from_id(id: c_uint) -> Result<MlKemSuite, c_int> {
    let id = u8::try_from(id).map_err(|_| ERR_INVALID_SUITE)?;
    MlKemSuite::from_kem_id(id).ok_or(ERR_INVALID_SUITE)
}

fn mldsa_suite_from_id(id: c_uint) -> Result<MlDsaSuite, c_int> {
    let id = u8::try_from(id).map_err(|_| ERR_INVALID_SUITE)?;
    MlDsaSuite::from_suite_id(id).ok_or(ERR_INVALID_SUITE)
}

fn map_mldsa_error(err: &MlDsaError) -> c_int {
    match err {
        MlDsaError::BadEncoding(_) => ERR_ENCODING,
        MlDsaError::VerificationFailed(_) => ERR_VERIFICATION_FAILED,
        MlDsaError::KeyGenerationFailed { .. } => ERR_KEYGEN,
    }
}

fn usize_from_c_ulong(value: c_ulong) -> Result<usize, c_int> {
    usize::try_from(value).map_err(|_| ERR_LENGTH_MISMATCH)
}

fn read_input<'a>(ptr: *const c_uchar, len: c_ulong) -> Result<&'a [u8], c_int> {
    let len = usize_from_c_ulong(len)?;
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(ERR_NULL_POINTER);
    }
    // SAFETY: the caller guarantees that the pointer is valid for `len` bytes.
    Ok(unsafe { slice::from_raw_parts(ptr, len) })
}

fn read_input_exact<'a>(
    ptr: *const c_uchar,
    len: c_ulong,
    expected: usize,
) -> Result<&'a [u8], c_int> {
    let bytes = read_input(ptr, len)?;
    if bytes.len() != expected {
        return Err(ERR_LENGTH_MISMATCH);
    }
    Ok(bytes)
}

fn write_output_exact<'a>(
    ptr: *mut c_uchar,
    len: c_ulong,
    expected: usize,
) -> Result<&'a mut [u8], c_int> {
    if ptr.is_null() {
        return Err(ERR_NULL_POINTER);
    }
    let len = usize_from_c_ulong(len)?;
    if len != expected {
        return Err(ERR_LENGTH_MISMATCH);
    }
    // SAFETY: the caller ensures the pointer references `len` writable bytes.
    Ok(unsafe { slice::from_raw_parts_mut(ptr, len) })
}

fn write_len(out: *mut c_uint, value: usize) -> Result<(), c_int> {
    let converted = c_uint::try_from(value).map_err(|_| ERR_LENGTH_MISMATCH)?;
    // SAFETY: callers ensure the pointer is valid and non-null.
    unsafe {
        *out = converted;
    }
    Ok(())
}

/// Return ML-KEM parameter lengths for the requested suite.
///
/// # Safety
/// The caller must provide valid writable pointers for all output parameters.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soranet_mlkem_parameters(
    suite_id: c_uint,
    public_key_len_out: *mut c_uint,
    secret_key_len_out: *mut c_uint,
    ciphertext_len_out: *mut c_uint,
    shared_secret_len_out: *mut c_uint,
) -> c_int {
    if public_key_len_out.is_null()
        || secret_key_len_out.is_null()
        || ciphertext_len_out.is_null()
        || shared_secret_len_out.is_null()
    {
        return ERR_NULL_POINTER;
    }

    let suite = match mlkem_suite_from_id(suite_id) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let params = suite.parameters();
    for (out, value) in [
        (public_key_len_out, params.public_key),
        (secret_key_len_out, params.secret_key),
        (ciphertext_len_out, params.ciphertext),
        (shared_secret_len_out, params.shared_secret),
    ] {
        if let Err(code) = write_len(out, value) {
            return code;
        }
    }
    0
}

/// Generate an ML-KEM keypair and write it into the provided buffers.
///
/// # Safety
/// Buffers must be valid for writes and match the suite's expected byte lengths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soranet_mlkem_generate_keypair(
    suite_id: c_uint,
    public_key_out: *mut c_uchar,
    public_key_len: c_ulong,
    secret_key_out: *mut c_uchar,
    secret_key_len: c_ulong,
) -> c_int {
    let suite = match mlkem_suite_from_id(suite_id) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let public_buf =
        match write_output_exact(public_key_out, public_key_len, suite.public_key_len()) {
            Ok(buf) => buf,
            Err(code) => return code,
        };
    let secret_buf =
        match write_output_exact(secret_key_out, secret_key_len, suite.secret_key_len()) {
            Ok(buf) => buf,
            Err(code) => return code,
        };
    let pair = generate_mlkem_keypair(suite);
    public_buf.copy_from_slice(pair.public_key());
    secret_buf.copy_from_slice(pair.secret_key());
    0
}

/// Encapsulate against an ML-KEM public key.
///
/// # Safety
/// Input and output buffers must be valid, non-null, and match the suite's byte lengths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soranet_mlkem_encapsulate(
    suite_id: c_uint,
    public_key: *const c_uchar,
    public_key_len: c_ulong,
    ciphertext_out: *mut c_uchar,
    ciphertext_len: c_ulong,
    shared_secret_out: *mut c_uchar,
    shared_secret_len: c_ulong,
) -> c_int {
    let suite = match mlkem_suite_from_id(suite_id) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let pk = match read_input_exact(public_key, public_key_len, suite.public_key_len()) {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    let ciphertext_buf =
        match write_output_exact(ciphertext_out, ciphertext_len, suite.ciphertext_len()) {
            Ok(buf) => buf,
            Err(code) => return code,
        };
    let shared_buf = match write_output_exact(
        shared_secret_out,
        shared_secret_len,
        suite.shared_secret_len(),
    ) {
        Ok(buf) => buf,
        Err(code) => return code,
    };
    match encapsulate_mlkem(suite, pk) {
        Ok((shared, ciphertext)) => {
            ciphertext_buf.copy_from_slice(ciphertext.as_bytes());
            shared_buf.copy_from_slice(shared.as_bytes());
            0
        }
        Err(MlKemError::BadEncoding { .. }) => ERR_ENCODING,
    }
}

/// Decapsulate an ML-KEM ciphertext.
///
/// # Safety
/// Input pointers must reference valid encodings and output buffers must match the suite's lengths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soranet_mlkem_decapsulate(
    suite_id: c_uint,
    secret_key: *const c_uchar,
    secret_key_len: c_ulong,
    ciphertext: *const c_uchar,
    ciphertext_len: c_ulong,
    shared_secret_out: *mut c_uchar,
    shared_secret_len: c_ulong,
) -> c_int {
    let suite = match mlkem_suite_from_id(suite_id) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let sk = match read_input_exact(secret_key, secret_key_len, suite.secret_key_len()) {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    let ct = match read_input_exact(ciphertext, ciphertext_len, suite.ciphertext_len()) {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    let shared_buf = match write_output_exact(
        shared_secret_out,
        shared_secret_len,
        suite.shared_secret_len(),
    ) {
        Ok(buf) => buf,
        Err(code) => return code,
    };
    match decapsulate_mlkem(suite, sk, ct) {
        Ok(shared) => {
            shared_buf.copy_from_slice(shared.as_bytes());
            0
        }
        Err(MlKemError::BadEncoding { .. }) => ERR_ENCODING,
    }
}

/// Return ML-DSA parameter lengths for the requested suite.
///
/// # Safety
/// The caller must provide valid writable pointers for all outputs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soranet_mldsa_parameters(
    suite_id: c_uint,
    public_key_len_out: *mut c_uint,
    secret_key_len_out: *mut c_uint,
    signature_len_out: *mut c_uint,
) -> c_int {
    if public_key_len_out.is_null() || secret_key_len_out.is_null() || signature_len_out.is_null() {
        return ERR_NULL_POINTER;
    }

    let suite = match mldsa_suite_from_id(suite_id) {
        Ok(value) => value,
        Err(code) => return code,
    };
    for (out, value) in [
        (public_key_len_out, suite.public_key_len()),
        (secret_key_len_out, suite.secret_key_len()),
        (signature_len_out, suite.signature_len()),
    ] {
        if let Err(code) = write_len(out, value) {
            return code;
        }
    }
    0
}

/// Generate an ML-DSA keypair and store it in the supplied buffers.
///
/// # Safety
/// Buffers must be valid for writes and match the suite's expected lengths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soranet_mldsa_generate_keypair(
    suite_id: c_uint,
    public_key_out: *mut c_uchar,
    public_key_len: c_ulong,
    secret_key_out: *mut c_uchar,
    secret_key_len: c_ulong,
) -> c_int {
    let suite = match mldsa_suite_from_id(suite_id) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let public_buf =
        match write_output_exact(public_key_out, public_key_len, suite.public_key_len()) {
            Ok(buf) => buf,
            Err(code) => return code,
        };
    let secret_buf =
        match write_output_exact(secret_key_out, secret_key_len, suite.secret_key_len()) {
            Ok(buf) => buf,
            Err(code) => return code,
        };
    let pair = match generate_mldsa_keypair(suite) {
        Ok(kp) => kp,
        Err(err) => return map_mldsa_error(&err),
    };
    public_buf.copy_from_slice(pair.public_key());
    secret_buf.copy_from_slice(pair.secret_key());
    0
}

/// Produce an ML-DSA signature for the supplied message.
///
/// # Safety
/// Input pointers must reference valid buffers and the signature buffer must match the suite length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soranet_mldsa_sign(
    suite_id: c_uint,
    secret_key: *const c_uchar,
    secret_key_len: c_ulong,
    message: *const c_uchar,
    message_len: c_ulong,
    signature_out: *mut c_uchar,
    signature_len: c_ulong,
) -> c_int {
    let suite = match mldsa_suite_from_id(suite_id) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let secret = match read_input_exact(secret_key, secret_key_len, suite.secret_key_len()) {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    let message = match read_input(message, message_len) {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    let signature_buf =
        match write_output_exact(signature_out, signature_len, suite.signature_len()) {
            Ok(buf) => buf,
            Err(code) => return code,
        };
    match sign_mldsa(suite, secret, message) {
        Ok(signature) => {
            signature_buf.copy_from_slice(signature.as_bytes());
            0
        }
        Err(err) => map_mldsa_error(&err),
    }
}

/// Verify an ML-DSA signature.
///
/// # Safety
/// Inputs must reference valid encodings with lengths matching the selected suite.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soranet_mldsa_verify(
    suite_id: c_uint,
    public_key: *const c_uchar,
    public_key_len: c_ulong,
    message: *const c_uchar,
    message_len: c_ulong,
    signature: *const c_uchar,
    signature_len: c_ulong,
) -> c_int {
    let suite = match mldsa_suite_from_id(suite_id) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let pk = match read_input_exact(public_key, public_key_len, suite.public_key_len()) {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    let message = match read_input(message, message_len) {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    let sig = match read_input_exact(signature, signature_len, suite.signature_len()) {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    match verify_mldsa(suite, pk, message, sig) {
        Ok(()) => 0,
        Err(err) => map_mldsa_error(&err),
    }
}

#[cfg(test)]
mod tests {
    use pqcrypto_traits::sign::VerificationError;

    use super::*;

    #[test]
    fn ffi_mlkem_roundtrip() {
        let suite = MlKemSuite::MlKem512;
        let suite_id = c_uint::from(suite.kem_id());
        let params = suite.parameters();
        let mut public_key = vec![0u8; params.public_key];
        let mut secret_key = vec![0u8; params.secret_key];
        let rc = unsafe {
            soranet_mlkem_generate_keypair(
                suite_id,
                public_key.as_mut_ptr(),
                public_key.len() as c_ulong,
                secret_key.as_mut_ptr(),
                secret_key.len() as c_ulong,
            )
        };
        assert_eq!(rc, 0);

        let mut ciphertext = vec![0u8; params.ciphertext];
        let mut shared_sender = vec![0u8; params.shared_secret];
        let rc = unsafe {
            soranet_mlkem_encapsulate(
                suite_id,
                public_key.as_ptr(),
                public_key.len() as c_ulong,
                ciphertext.as_mut_ptr(),
                ciphertext.len() as c_ulong,
                shared_sender.as_mut_ptr(),
                shared_sender.len() as c_ulong,
            )
        };
        assert_eq!(rc, 0);

        let mut shared_receiver = vec![0u8; params.shared_secret];
        let rc = unsafe {
            soranet_mlkem_decapsulate(
                suite_id,
                secret_key.as_ptr(),
                secret_key.len() as c_ulong,
                ciphertext.as_ptr(),
                ciphertext.len() as c_ulong,
                shared_receiver.as_mut_ptr(),
                shared_receiver.len() as c_ulong,
            )
        };
        assert_eq!(rc, 0);
        assert_eq!(shared_sender, shared_receiver);
    }

    #[test]
    fn ffi_mldsa_sign_and_verify() {
        let suite = MlDsaSuite::MlDsa44;
        let suite_id = c_uint::from(suite.suite_id());
        let mut public_key = vec![0u8; suite.public_key_len()];
        let mut secret_key = vec![0u8; suite.secret_key_len()];
        let rc = unsafe {
            soranet_mldsa_generate_keypair(
                suite_id,
                public_key.as_mut_ptr(),
                public_key.len() as c_ulong,
                secret_key.as_mut_ptr(),
                secret_key.len() as c_ulong,
            )
        };
        assert_eq!(rc, 0);

        let message = b"pq ffi smoke";
        let mut signature = vec![0u8; suite.signature_len()];
        let rc = unsafe {
            soranet_mldsa_sign(
                suite_id,
                secret_key.as_ptr(),
                secret_key.len() as c_ulong,
                message.as_ptr(),
                message.len() as c_ulong,
                signature.as_mut_ptr(),
                signature.len() as c_ulong,
            )
        };
        assert_eq!(rc, 0);

        let rc = unsafe {
            soranet_mldsa_verify(
                suite_id,
                public_key.as_ptr(),
                public_key.len() as c_ulong,
                message.as_ptr(),
                message.len() as c_ulong,
                signature.as_ptr(),
                signature.len() as c_ulong,
            )
        };
        assert_eq!(rc, 0);
    }

    #[test]
    fn write_len_validates_bounds() {
        let mut slot: c_uint = 0;
        let ptr = core::ptr::addr_of_mut!(slot);
        assert!(write_len(ptr, c_uint::MAX as usize).is_ok());
        assert_eq!(slot, c_uint::MAX);
        if usize::BITS > c_uint::BITS {
            let too_large = (c_uint::MAX as usize).saturating_add(1);
            assert_eq!(write_len(ptr, too_large), Err(ERR_LENGTH_MISMATCH));
        }
    }

    #[test]
    fn map_mldsa_error_maps_variants() {
        let verification = MlDsaError::VerificationFailed(VerificationError::InvalidSignature);
        assert_eq!(map_mldsa_error(&verification), ERR_VERIFICATION_FAILED);

        let keygen = MlDsaError::KeyGenerationFailed {
            suite: MlDsaSuite::MlDsa44,
            status: -1,
        };
        assert_eq!(map_mldsa_error(&keygen), ERR_KEYGEN);
    }
}
