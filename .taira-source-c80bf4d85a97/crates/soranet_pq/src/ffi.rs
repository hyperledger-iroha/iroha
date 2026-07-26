//! C-friendly bindings for the `soranet_pq` primitives.

use core::{
    convert::TryFrom,
    ffi::{c_int, c_uchar, c_uint, c_ulong},
    slice,
};

use crate::{
    MlDsaSuite, MlKemSuite, decapsulate_mlkem, encapsulate_mlkem_from_os,
    generate_mldsa_keypair_from_os, generate_mlkem_keypair_from_os, mldsa::MlDsaError,
    mlkem::MlKemError, sign_mldsa_from_os, verify_mldsa,
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
        MlDsaError::BadEncoding(_)
        | MlDsaError::SecretKeyMismatch { .. }
        | MlDsaError::InertKeyMaterial { .. } => ERR_ENCODING,
        MlDsaError::ContextTooLong { .. } => ERR_LENGTH_MISMATCH,
        MlDsaError::VerificationFailed(_) => ERR_VERIFICATION_FAILED,
        MlDsaError::KeyGenerationFailed { .. } | MlDsaError::Rng(_) => ERR_KEYGEN,
    }
}

fn map_mlkem_error(err: &MlKemError) -> c_int {
    match err {
        MlKemError::BadEncoding { .. }
        | MlKemError::KeyPairMismatch { .. }
        | MlKemError::KeyPairPublicHashMismatch { .. }
        | MlKemError::InertKeyMaterial { .. }
        | MlKemError::NonCanonicalEncoding { .. } => ERR_ENCODING,
        MlKemError::BackendFailure { .. } | MlKemError::Rng(_) => ERR_KEYGEN,
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
    match generate_mlkem_keypair_from_os(suite) {
        Ok(pair) => {
            public_buf.copy_from_slice(pair.public_key());
            secret_buf.copy_from_slice(pair.secret_key());
            0
        }
        Err(err) => map_mlkem_error(&err),
    }
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
    match encapsulate_mlkem_from_os(suite, pk) {
        Ok((shared, ciphertext)) => {
            ciphertext_buf.copy_from_slice(ciphertext.as_bytes());
            shared_buf.copy_from_slice(shared.as_bytes());
            0
        }
        Err(err) => map_mlkem_error(&err),
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
        Err(err) => map_mlkem_error(&err),
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
    let pair = match generate_mldsa_keypair_from_os(suite) {
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
    match sign_mldsa_from_os(suite, secret, &[], message) {
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
    match verify_mldsa(suite, pk, &[], message, sig) {
        Ok(()) => 0,
        Err(err) => map_mldsa_error(&err),
    }
}

#[cfg(test)]
mod tests {
    use core::ptr;

    use pqcrypto_traits::sign::VerificationError;
    use sha3::{Digest, Sha3_256};

    use crate::{HedgedRngSeed, deterministic_chacha20_rng, sign_mldsa};

    use super::*;

    fn len_as_c_uint(len: usize) -> c_uint {
        c_uint::try_from(len).expect("test vector length fits in c_uint")
    }

    fn ffi_mldsa_keypair(suite: MlDsaSuite) -> (Vec<u8>, Vec<u8>) {
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
        (public_key, secret_key)
    }

    fn ffi_mlkem_keypair(suite: MlKemSuite) -> (Vec<u8>, Vec<u8>) {
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
        (public_key, secret_key)
    }

    fn ffi_mlkem_encapsulate(suite: MlKemSuite, public_key: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let suite_id = c_uint::from(suite.kem_id());
        let params = suite.parameters();
        let mut ciphertext = vec![0u8; params.ciphertext];
        let mut shared_secret = vec![0u8; params.shared_secret];
        let rc = unsafe {
            soranet_mlkem_encapsulate(
                suite_id,
                public_key.as_ptr(),
                public_key.len() as c_ulong,
                ciphertext.as_mut_ptr(),
                ciphertext.len() as c_ulong,
                shared_secret.as_mut_ptr(),
                shared_secret.len() as c_ulong,
            )
        };
        assert_eq!(rc, 0);
        (ciphertext, shared_secret)
    }

    fn mlkem_secret_embedded_public_range(suite: MlKemSuite) -> core::ops::Range<usize> {
        const PUBLIC_HASH_AND_REJECTION_SEED_BYTES: usize = 64;

        let start =
            suite.secret_key_len() - suite.public_key_len() - PUBLIC_HASH_AND_REJECTION_SEED_BYTES;
        start..start + suite.public_key_len()
    }

    fn mlkem_secret_embedded_public_hash_range(suite: MlKemSuite) -> core::ops::Range<usize> {
        const PUBLIC_KEY_HASH_BYTES: usize = 32;
        const PUBLIC_HASH_AND_REJECTION_SEED_BYTES: usize = 64;

        let start = suite.secret_key_len() - PUBLIC_HASH_AND_REJECTION_SEED_BYTES;
        start..start + PUBLIC_KEY_HASH_BYTES
    }

    fn mlkem_public_key_hash(public_key: &[u8]) -> [u8; 32] {
        let digest = Sha3_256::digest(public_key);
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        out
    }

    fn set_first_mlkem_12_bit_coefficient_noncanonical(bytes: &mut [u8]) {
        bytes[0] = 0xFF;
        bytes[1] = (bytes[1] & 0xF0) | 0x0F;
    }

    fn set_first_mlkem_public_key_coefficient_noncanonical(public_key: &mut [u8]) {
        set_first_mlkem_12_bit_coefficient_noncanonical(public_key);
    }

    #[test]
    fn ffi_suite_converters_reject_overflow_identifiers() {
        let overflow = c_uint::from(u8::MAX) + 1;

        assert_eq!(mlkem_suite_from_id(overflow).err(), Some(ERR_INVALID_SUITE));
        assert_eq!(mldsa_suite_from_id(overflow).err(), Some(ERR_INVALID_SUITE));
    }

    #[test]
    fn ffi_buffer_helpers_accept_empty_input_and_reject_bad_lengths() {
        let empty = read_input(ptr::null(), 0).expect("empty null input is allowed");
        assert!(empty.is_empty());

        let input = [0x11, 0x22];
        let exact =
            read_input_exact(input.as_ptr(), input.len() as c_ulong, input.len()).expect("exact");
        assert_eq!(exact, input);
        assert_eq!(
            read_input_exact(input.as_ptr(), (input.len() - 1) as c_ulong, input.len()).err(),
            Some(ERR_LENGTH_MISMATCH)
        );

        let mut output = [0u8; 2];
        assert!(write_output_exact(output.as_mut_ptr(), 2, 2).is_ok());
        assert_eq!(
            write_output_exact(output.as_mut_ptr(), 1, 2).err(),
            Some(ERR_LENGTH_MISMATCH)
        );
        assert_eq!(
            write_output_exact(ptr::null_mut(), 0, 0).err(),
            Some(ERR_NULL_POINTER)
        );
    }

    #[test]
    fn ffi_exported_operations_reject_invalid_suite_before_buffers() {
        let invalid_suite = c_uint::from(u8::MAX) + 1;

        let rc = unsafe {
            soranet_mlkem_generate_keypair(invalid_suite, ptr::null_mut(), 0, ptr::null_mut(), 0)
        };
        assert_eq!(rc, ERR_INVALID_SUITE);

        let rc = unsafe {
            soranet_mlkem_encapsulate(
                invalid_suite,
                ptr::null(),
                0,
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                0,
            )
        };
        assert_eq!(rc, ERR_INVALID_SUITE);

        let rc = unsafe {
            soranet_mlkem_decapsulate(
                invalid_suite,
                ptr::null(),
                0,
                ptr::null(),
                0,
                ptr::null_mut(),
                0,
            )
        };
        assert_eq!(rc, ERR_INVALID_SUITE);

        let rc = unsafe {
            soranet_mldsa_generate_keypair(invalid_suite, ptr::null_mut(), 0, ptr::null_mut(), 0)
        };
        assert_eq!(rc, ERR_INVALID_SUITE);

        let rc = unsafe {
            soranet_mldsa_sign(
                invalid_suite,
                ptr::null(),
                0,
                ptr::null(),
                0,
                ptr::null_mut(),
                0,
            )
        };
        assert_eq!(rc, ERR_INVALID_SUITE);

        let rc = unsafe {
            soranet_mldsa_verify(
                invalid_suite,
                ptr::null(),
                0,
                ptr::null(),
                0,
                ptr::null(),
                0,
            )
        };
        assert_eq!(rc, ERR_INVALID_SUITE);
    }

    #[test]
    fn ffi_mlkem_roundtrip() {
        let suite = MlKemSuite::MlKem512;
        let suite_id = c_uint::from(suite.kem_id());
        let params = suite.parameters();
        let (public_key, secret_key) = ffi_mlkem_keypair(suite);
        let (ciphertext, shared_sender) = ffi_mlkem_encapsulate(suite, &public_key);

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
    fn ffi_mlkem_encapsulate_rejects_noncanonical_public_key() {
        let suite = MlKemSuite::MlKem768;
        let suite_id = c_uint::from(suite.kem_id());
        let params = suite.parameters();
        let (mut public_key, _) = ffi_mlkem_keypair(suite);
        set_first_mlkem_public_key_coefficient_noncanonical(&mut public_key);
        let mut ciphertext = vec![0u8; params.ciphertext];
        let mut shared_secret = vec![0u8; params.shared_secret];

        let rc = unsafe {
            soranet_mlkem_encapsulate(
                suite_id,
                public_key.as_ptr(),
                public_key.len() as c_ulong,
                ciphertext.as_mut_ptr(),
                ciphertext.len() as c_ulong,
                shared_secret.as_mut_ptr(),
                shared_secret.len() as c_ulong,
            )
        };

        assert_eq!(rc, ERR_ENCODING);
    }

    #[test]
    fn ffi_mlkem_encapsulate_rejects_all_zero_public_key() {
        let suite = MlKemSuite::MlKem512;
        let suite_id = c_uint::from(suite.kem_id());
        let params = suite.parameters();
        let public_key = vec![0u8; params.public_key];
        let mut ciphertext = vec![0u8; params.ciphertext];
        let mut shared_secret = vec![0u8; params.shared_secret];

        let rc = unsafe {
            soranet_mlkem_encapsulate(
                suite_id,
                public_key.as_ptr(),
                public_key.len() as c_ulong,
                ciphertext.as_mut_ptr(),
                ciphertext.len() as c_ulong,
                shared_secret.as_mut_ptr(),
                shared_secret.len() as c_ulong,
            )
        };

        assert_eq!(rc, ERR_ENCODING);
    }

    #[test]
    fn ffi_mlkem_decapsulate_rejects_noncanonical_secret_key_private_component() {
        let suite = MlKemSuite::MlKem768;
        let suite_id = c_uint::from(suite.kem_id());
        let params = suite.parameters();
        let (public_key, mut secret_key) = ffi_mlkem_keypair(suite);
        let (ciphertext, _) = ffi_mlkem_encapsulate(suite, &public_key);
        set_first_mlkem_12_bit_coefficient_noncanonical(&mut secret_key);
        let mut shared_secret = vec![0u8; params.shared_secret];

        let rc = unsafe {
            soranet_mlkem_decapsulate(
                suite_id,
                secret_key.as_ptr(),
                secret_key.len() as c_ulong,
                ciphertext.as_ptr(),
                ciphertext.len() as c_ulong,
                shared_secret.as_mut_ptr(),
                shared_secret.len() as c_ulong,
            )
        };

        assert_eq!(rc, ERR_ENCODING);
    }

    #[test]
    fn ffi_mlkem_decapsulate_rejects_all_zero_secret_key() {
        let suite = MlKemSuite::MlKem512;
        let suite_id = c_uint::from(suite.kem_id());
        let params = suite.parameters();
        let (public_key, _) = ffi_mlkem_keypair(suite);
        let (ciphertext, _) = ffi_mlkem_encapsulate(suite, &public_key);
        let secret_key = vec![0u8; params.secret_key];
        let mut shared_secret = vec![0u8; params.shared_secret];

        let rc = unsafe {
            soranet_mlkem_decapsulate(
                suite_id,
                secret_key.as_ptr(),
                secret_key.len() as c_ulong,
                ciphertext.as_ptr(),
                ciphertext.len() as c_ulong,
                shared_secret.as_mut_ptr(),
                shared_secret.len() as c_ulong,
            )
        };

        assert_eq!(rc, ERR_ENCODING);
    }

    #[test]
    fn ffi_mlkem_decapsulate_rejects_all_zero_embedded_public_key() {
        let suite = MlKemSuite::MlKem512;
        let suite_id = c_uint::from(suite.kem_id());
        let params = suite.parameters();
        let (public_key, mut secret_key) = ffi_mlkem_keypair(suite);
        let (ciphertext, _) = ffi_mlkem_encapsulate(suite, &public_key);
        let public_range = mlkem_secret_embedded_public_range(suite);
        secret_key[public_range.clone()].fill(0);
        let public_hash = mlkem_public_key_hash(&secret_key[public_range]);
        secret_key[mlkem_secret_embedded_public_hash_range(suite)].copy_from_slice(&public_hash);
        let mut shared_secret = vec![0u8; params.shared_secret];

        let rc = unsafe {
            soranet_mlkem_decapsulate(
                suite_id,
                secret_key.as_ptr(),
                secret_key.len() as c_ulong,
                ciphertext.as_ptr(),
                ciphertext.len() as c_ulong,
                shared_secret.as_mut_ptr(),
                shared_secret.len() as c_ulong,
            )
        };

        assert_eq!(rc, ERR_ENCODING);
    }

    #[test]
    fn ffi_mlkem_decapsulate_rejects_all_zero_ciphertext() {
        let suite = MlKemSuite::MlKem512;
        let suite_id = c_uint::from(suite.kem_id());
        let params = suite.parameters();
        let (_, secret_key) = ffi_mlkem_keypair(suite);
        let ciphertext = vec![0u8; params.ciphertext];
        let mut shared_secret = vec![0u8; params.shared_secret];

        let rc = unsafe {
            soranet_mlkem_decapsulate(
                suite_id,
                secret_key.as_ptr(),
                secret_key.len() as c_ulong,
                ciphertext.as_ptr(),
                ciphertext.len() as c_ulong,
                shared_secret.as_mut_ptr(),
                shared_secret.len() as c_ulong,
            )
        };

        assert_eq!(rc, ERR_ENCODING);
    }

    #[test]
    fn ffi_mlkem_parameters_writes_lengths_and_rejects_null_output() {
        let suite = MlKemSuite::MlKem768;
        let params = suite.parameters();
        let mut public_key_len = 0;
        let mut secret_key_len = 0;
        let mut ciphertext_len = 0;
        let mut shared_secret_len = 0;
        let rc = unsafe {
            soranet_mlkem_parameters(
                c_uint::from(suite.kem_id()),
                &raw mut public_key_len,
                &raw mut secret_key_len,
                &raw mut ciphertext_len,
                &raw mut shared_secret_len,
            )
        };
        assert_eq!(rc, 0);
        assert_eq!(public_key_len, len_as_c_uint(params.public_key));
        assert_eq!(secret_key_len, len_as_c_uint(params.secret_key));
        assert_eq!(ciphertext_len, len_as_c_uint(params.ciphertext));
        assert_eq!(shared_secret_len, len_as_c_uint(params.shared_secret));

        let rc = unsafe {
            soranet_mlkem_parameters(
                c_uint::from(suite.kem_id()),
                &raw mut public_key_len,
                ptr::null_mut(),
                &raw mut ciphertext_len,
                &raw mut shared_secret_len,
            )
        };
        assert_eq!(rc, ERR_NULL_POINTER);
    }

    #[test]
    fn ffi_mlkem_rejects_invalid_suite_and_lengths() {
        let mut public_key_len = 0;
        let mut secret_key_len = 0;
        let mut ciphertext_len = 0;
        let mut shared_secret_len = 0;
        let rc = unsafe {
            soranet_mlkem_parameters(
                0xFF,
                &raw mut public_key_len,
                &raw mut secret_key_len,
                &raw mut ciphertext_len,
                &raw mut shared_secret_len,
            )
        };
        assert_eq!(rc, ERR_INVALID_SUITE);

        let suite = MlKemSuite::MlKem512;
        let params = suite.parameters();
        let mut public_key = vec![0u8; params.public_key - 1];
        let mut secret_key = vec![0u8; params.secret_key];
        let rc = unsafe {
            soranet_mlkem_generate_keypair(
                c_uint::from(suite.kem_id()),
                public_key.as_mut_ptr(),
                public_key.len() as c_ulong,
                secret_key.as_mut_ptr(),
                secret_key.len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_LENGTH_MISMATCH);
    }

    #[test]
    fn ffi_mlkem_generate_keypair_rejects_null_outputs() {
        let suite = MlKemSuite::MlKem512;
        let params = suite.parameters();
        let mut public_key = vec![0u8; params.public_key];
        let mut secret_key = vec![0u8; params.secret_key];

        let rc = unsafe {
            soranet_mlkem_generate_keypair(
                c_uint::from(suite.kem_id()),
                ptr::null_mut(),
                params.public_key as c_ulong,
                secret_key.as_mut_ptr(),
                secret_key.len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_NULL_POINTER);

        let rc = unsafe {
            soranet_mlkem_generate_keypair(
                c_uint::from(suite.kem_id()),
                public_key.as_mut_ptr(),
                public_key.len() as c_ulong,
                ptr::null_mut(),
                params.secret_key as c_ulong,
            )
        };
        assert_eq!(rc, ERR_NULL_POINTER);
    }

    #[test]
    fn ffi_mlkem_rejects_null_nonempty_input() {
        let suite = MlKemSuite::MlKem512;
        let params = suite.parameters();
        let mut ciphertext = vec![0u8; params.ciphertext];
        let mut shared_secret = vec![0u8; params.shared_secret];

        let rc = unsafe {
            soranet_mlkem_encapsulate(
                c_uint::from(suite.kem_id()),
                ptr::null(),
                params.public_key as c_ulong,
                ciphertext.as_mut_ptr(),
                ciphertext.len() as c_ulong,
                shared_secret.as_mut_ptr(),
                shared_secret.len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_NULL_POINTER);
    }

    #[test]
    fn ffi_mlkem_encapsulate_rejects_short_output_buffers() {
        let suite = MlKemSuite::MlKem512;
        let params = suite.parameters();
        let (public_key, _) = ffi_mlkem_keypair(suite);
        let mut ciphertext = vec![0u8; params.ciphertext - 1];
        let mut shared_secret = vec![0u8; params.shared_secret];

        let rc = unsafe {
            soranet_mlkem_encapsulate(
                c_uint::from(suite.kem_id()),
                public_key.as_ptr(),
                public_key.len() as c_ulong,
                ciphertext.as_mut_ptr(),
                ciphertext.len() as c_ulong,
                shared_secret.as_mut_ptr(),
                shared_secret.len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_LENGTH_MISMATCH);

        let mut ciphertext = vec![0u8; params.ciphertext];
        let mut shared_secret = vec![0u8; params.shared_secret - 1];
        let rc = unsafe {
            soranet_mlkem_encapsulate(
                c_uint::from(suite.kem_id()),
                public_key.as_ptr(),
                public_key.len() as c_ulong,
                ciphertext.as_mut_ptr(),
                ciphertext.len() as c_ulong,
                shared_secret.as_mut_ptr(),
                shared_secret.len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_LENGTH_MISMATCH);
    }

    #[test]
    fn ffi_mlkem_encapsulate_rejects_null_output_buffers() {
        let suite = MlKemSuite::MlKem512;
        let params = suite.parameters();
        let (public_key, _) = ffi_mlkem_keypair(suite);
        let mut ciphertext = vec![0u8; params.ciphertext];
        let mut shared_secret = vec![0u8; params.shared_secret];

        let rc = unsafe {
            soranet_mlkem_encapsulate(
                c_uint::from(suite.kem_id()),
                public_key.as_ptr(),
                public_key.len() as c_ulong,
                ptr::null_mut(),
                params.ciphertext as c_ulong,
                shared_secret.as_mut_ptr(),
                shared_secret.len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_NULL_POINTER);

        let rc = unsafe {
            soranet_mlkem_encapsulate(
                c_uint::from(suite.kem_id()),
                public_key.as_ptr(),
                public_key.len() as c_ulong,
                ciphertext.as_mut_ptr(),
                ciphertext.len() as c_ulong,
                ptr::null_mut(),
                params.shared_secret as c_ulong,
            )
        };
        assert_eq!(rc, ERR_NULL_POINTER);
    }

    #[test]
    fn ffi_mlkem_decapsulate_rejects_bad_output_and_input_lengths() {
        let suite = MlKemSuite::MlKem512;
        let suite_id = c_uint::from(suite.kem_id());
        let params = suite.parameters();
        let (public_key, secret_key) = ffi_mlkem_keypair(suite);
        let (ciphertext, _) = ffi_mlkem_encapsulate(suite, &public_key);
        let mut shared_secret = vec![0u8; params.shared_secret - 1];

        let rc = unsafe {
            soranet_mlkem_decapsulate(
                suite_id,
                secret_key.as_ptr(),
                secret_key.len() as c_ulong,
                ciphertext.as_ptr(),
                ciphertext.len() as c_ulong,
                shared_secret.as_mut_ptr(),
                shared_secret.len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_LENGTH_MISMATCH);

        let mut shared_secret = vec![0u8; params.shared_secret];
        let rc = unsafe {
            soranet_mlkem_decapsulate(
                suite_id,
                secret_key.as_ptr(),
                (secret_key.len() - 1) as c_ulong,
                ciphertext.as_ptr(),
                ciphertext.len() as c_ulong,
                shared_secret.as_mut_ptr(),
                shared_secret.len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_LENGTH_MISMATCH);
    }

    #[test]
    fn ffi_mlkem_decapsulate_rejects_null_nonempty_inputs() {
        let suite = MlKemSuite::MlKem512;
        let suite_id = c_uint::from(suite.kem_id());
        let params = suite.parameters();
        let (public_key, secret_key) = ffi_mlkem_keypair(suite);
        let (ciphertext, _) = ffi_mlkem_encapsulate(suite, &public_key);
        let mut shared_secret = vec![0u8; params.shared_secret];

        let rc = unsafe {
            soranet_mlkem_decapsulate(
                suite_id,
                ptr::null(),
                secret_key.len() as c_ulong,
                ciphertext.as_ptr(),
                ciphertext.len() as c_ulong,
                shared_secret.as_mut_ptr(),
                shared_secret.len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_NULL_POINTER);

        let rc = unsafe {
            soranet_mlkem_decapsulate(
                suite_id,
                secret_key.as_ptr(),
                secret_key.len() as c_ulong,
                ptr::null(),
                ciphertext.len() as c_ulong,
                shared_secret.as_mut_ptr(),
                shared_secret.len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_NULL_POINTER);
    }

    #[test]
    fn ffi_mlkem_decapsulate_rejects_null_output_buffer() {
        let suite = MlKemSuite::MlKem512;
        let suite_id = c_uint::from(suite.kem_id());
        let params = suite.parameters();
        let (public_key, secret_key) = ffi_mlkem_keypair(suite);
        let (ciphertext, _) = ffi_mlkem_encapsulate(suite, &public_key);

        let rc = unsafe {
            soranet_mlkem_decapsulate(
                suite_id,
                secret_key.as_ptr(),
                secret_key.len() as c_ulong,
                ciphertext.as_ptr(),
                ciphertext.len() as c_ulong,
                ptr::null_mut(),
                params.shared_secret as c_ulong,
            )
        };
        assert_eq!(rc, ERR_NULL_POINTER);
    }

    #[test]
    fn ffi_mldsa_sign_and_verify() {
        let suite = MlDsaSuite::MlDsa44;
        let suite_id = c_uint::from(suite.suite_id());
        let (public_key, secret_key) = ffi_mldsa_keypair(suite);

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
    fn ffi_mldsa_parameters_writes_expected_lengths() {
        let suite = MlDsaSuite::MlDsa87;
        let mut public_key_len = 0;
        let mut secret_key_len = 0;
        let mut signature_len = 0;

        let rc = unsafe {
            soranet_mldsa_parameters(
                c_uint::from(suite.suite_id()),
                &raw mut public_key_len,
                &raw mut secret_key_len,
                &raw mut signature_len,
            )
        };
        assert_eq!(rc, 0);
        assert_eq!(public_key_len, len_as_c_uint(suite.public_key_len()));
        assert_eq!(secret_key_len, len_as_c_uint(suite.secret_key_len()));
        assert_eq!(signature_len, len_as_c_uint(suite.signature_len()));
    }

    #[test]
    fn ffi_mldsa_rejects_invalid_suite_and_null_outputs() {
        let mut public_key_len = 0;
        let mut secret_key_len = 0;
        let mut signature_len = 0;
        let rc = unsafe {
            soranet_mldsa_parameters(
                0xFF,
                &raw mut public_key_len,
                &raw mut secret_key_len,
                &raw mut signature_len,
            )
        };
        assert_eq!(rc, ERR_INVALID_SUITE);

        let rc = unsafe {
            soranet_mldsa_parameters(
                c_uint::from(MlDsaSuite::MlDsa44.suite_id()),
                ptr::null_mut(),
                &raw mut secret_key_len,
                &raw mut signature_len,
            )
        };
        assert_eq!(rc, ERR_NULL_POINTER);
    }

    #[test]
    fn ffi_mldsa_generate_keypair_rejects_short_buffers() {
        let suite = MlDsaSuite::MlDsa65;
        let mut public_key = vec![0u8; suite.public_key_len()];
        let mut secret_key = vec![0u8; suite.secret_key_len() - 1];

        let rc = unsafe {
            soranet_mldsa_generate_keypair(
                c_uint::from(suite.suite_id()),
                public_key.as_mut_ptr(),
                public_key.len() as c_ulong,
                secret_key.as_mut_ptr(),
                secret_key.len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_LENGTH_MISMATCH);
    }

    #[test]
    fn ffi_mldsa_generate_keypair_rejects_null_outputs() {
        let suite = MlDsaSuite::MlDsa65;
        let mut public_key = vec![0u8; suite.public_key_len()];
        let mut secret_key = vec![0u8; suite.secret_key_len()];

        let rc = unsafe {
            soranet_mldsa_generate_keypair(
                c_uint::from(suite.suite_id()),
                ptr::null_mut(),
                suite.public_key_len() as c_ulong,
                secret_key.as_mut_ptr(),
                secret_key.len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_NULL_POINTER);

        let rc = unsafe {
            soranet_mldsa_generate_keypair(
                c_uint::from(suite.suite_id()),
                public_key.as_mut_ptr(),
                public_key.len() as c_ulong,
                ptr::null_mut(),
                suite.secret_key_len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_NULL_POINTER);
    }

    #[test]
    fn ffi_mldsa_sign_accepts_null_zero_length_message() {
        let suite = MlDsaSuite::MlDsa44;
        let (_, secret_key) = ffi_mldsa_keypair(suite);
        let mut signature = vec![0u8; suite.signature_len()];

        let rc = unsafe {
            soranet_mldsa_sign(
                c_uint::from(suite.suite_id()),
                secret_key.as_ptr(),
                secret_key.len() as c_ulong,
                ptr::null(),
                0,
                signature.as_mut_ptr(),
                signature.len() as c_ulong,
            )
        };
        assert_eq!(rc, 0);
    }

    #[test]
    fn ffi_mldsa_sign_rejects_null_nonempty_message() {
        let suite = MlDsaSuite::MlDsa44;
        let (_, secret_key) = ffi_mldsa_keypair(suite);
        let mut signature = vec![0u8; suite.signature_len()];

        let rc = unsafe {
            soranet_mldsa_sign(
                c_uint::from(suite.suite_id()),
                secret_key.as_ptr(),
                secret_key.len() as c_ulong,
                ptr::null(),
                1,
                signature.as_mut_ptr(),
                signature.len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_NULL_POINTER);
    }

    #[test]
    fn ffi_mldsa_sign_rejects_null_signature_output() {
        let suite = MlDsaSuite::MlDsa44;
        let (_, secret_key) = ffi_mldsa_keypair(suite);
        let message = b"null signature output";

        let rc = unsafe {
            soranet_mldsa_sign(
                c_uint::from(suite.suite_id()),
                secret_key.as_ptr(),
                secret_key.len() as c_ulong,
                message.as_ptr(),
                message.len() as c_ulong,
                ptr::null_mut(),
                suite.signature_len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_NULL_POINTER);
    }

    #[test]
    fn ffi_mldsa_sign_rejects_short_secret_and_signature_buffers() {
        let suite = MlDsaSuite::MlDsa44;
        let (_, secret_key) = ffi_mldsa_keypair(suite);
        let mut signature = vec![0u8; suite.signature_len()];
        let message = b"short secret";

        let rc = unsafe {
            soranet_mldsa_sign(
                c_uint::from(suite.suite_id()),
                secret_key.as_ptr(),
                (secret_key.len() - 1) as c_ulong,
                message.as_ptr(),
                message.len() as c_ulong,
                signature.as_mut_ptr(),
                signature.len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_LENGTH_MISMATCH);

        let rc = unsafe {
            soranet_mldsa_sign(
                c_uint::from(suite.suite_id()),
                secret_key.as_ptr(),
                secret_key.len() as c_ulong,
                message.as_ptr(),
                message.len() as c_ulong,
                signature.as_mut_ptr(),
                (signature.len() - 1) as c_ulong,
            )
        };
        assert_eq!(rc, ERR_LENGTH_MISMATCH);
    }

    #[test]
    fn ffi_mldsa_sign_rejects_all_zero_secret_key() {
        let suite = MlDsaSuite::MlDsa44;
        let secret_key = vec![0u8; suite.secret_key_len()];
        let mut signature = vec![0u8; suite.signature_len()];
        let message = b"inert secret";

        let rc = unsafe {
            soranet_mldsa_sign(
                c_uint::from(suite.suite_id()),
                secret_key.as_ptr(),
                secret_key.len() as c_ulong,
                message.as_ptr(),
                message.len() as c_ulong,
                signature.as_mut_ptr(),
                signature.len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_ENCODING);
    }

    #[test]
    fn ffi_mldsa_verify_rejects_null_public_key_and_signature() {
        let suite = MlDsaSuite::MlDsa44;
        let suite_id = c_uint::from(suite.suite_id());
        let (public_key, secret_key) = ffi_mldsa_keypair(suite);
        let message = b"verify rejects null public key and signature";
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
                ptr::null(),
                public_key.len() as c_ulong,
                message.as_ptr(),
                message.len() as c_ulong,
                signature.as_ptr(),
                signature.len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_NULL_POINTER);

        let rc = unsafe {
            soranet_mldsa_verify(
                suite_id,
                public_key.as_ptr(),
                public_key.len() as c_ulong,
                message.as_ptr(),
                message.len() as c_ulong,
                ptr::null(),
                signature.len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_NULL_POINTER);
    }

    #[test]
    fn ffi_mldsa_verify_rejects_null_nonempty_message() {
        let suite = MlDsaSuite::MlDsa44;
        let suite_id = c_uint::from(suite.suite_id());
        let (public_key, secret_key) = ffi_mldsa_keypair(suite);
        let message = b"verify rejects null message";
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
                ptr::null(),
                message.len() as c_ulong,
                signature.as_ptr(),
                signature.len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_NULL_POINTER);
    }

    #[test]
    fn ffi_mldsa_verify_accepts_null_zero_length_message() {
        let suite = MlDsaSuite::MlDsa44;
        let suite_id = c_uint::from(suite.suite_id());
        let (public_key, secret_key) = ffi_mldsa_keypair(suite);
        let mut signature = vec![0u8; suite.signature_len()];
        let rc = unsafe {
            soranet_mldsa_sign(
                suite_id,
                secret_key.as_ptr(),
                secret_key.len() as c_ulong,
                ptr::null(),
                0,
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
                ptr::null(),
                0,
                signature.as_ptr(),
                signature.len() as c_ulong,
            )
        };
        assert_eq!(rc, 0);
    }

    #[test]
    fn ffi_mldsa_verify_rejects_short_public_key_and_signature() {
        let suite = MlDsaSuite::MlDsa44;
        let suite_id = c_uint::from(suite.suite_id());
        let (public_key, secret_key) = ffi_mldsa_keypair(suite);
        let message = b"ffi verify length";
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
                (public_key.len() - 1) as c_ulong,
                message.as_ptr(),
                message.len() as c_ulong,
                signature.as_ptr(),
                signature.len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_LENGTH_MISMATCH);

        let rc = unsafe {
            soranet_mldsa_verify(
                suite_id,
                public_key.as_ptr(),
                public_key.len() as c_ulong,
                message.as_ptr(),
                message.len() as c_ulong,
                signature.as_ptr(),
                (signature.len() - 1) as c_ulong,
            )
        };
        assert_eq!(rc, ERR_LENGTH_MISMATCH);
    }

    #[test]
    fn ffi_mldsa_verify_rejects_all_zero_public_key_and_signature() {
        let suite = MlDsaSuite::MlDsa44;
        let suite_id = c_uint::from(suite.suite_id());
        let (public_key, secret_key) = ffi_mldsa_keypair(suite);
        let message = b"inert verify material";
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

        let public_key_zero = vec![0u8; suite.public_key_len()];
        let rc = unsafe {
            soranet_mldsa_verify(
                suite_id,
                public_key_zero.as_ptr(),
                public_key_zero.len() as c_ulong,
                message.as_ptr(),
                message.len() as c_ulong,
                signature.as_ptr(),
                signature.len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_ENCODING);

        let signature_zero = vec![0u8; suite.signature_len()];
        let rc = unsafe {
            soranet_mldsa_verify(
                suite_id,
                public_key.as_ptr(),
                public_key.len() as c_ulong,
                message.as_ptr(),
                message.len() as c_ulong,
                signature_zero.as_ptr(),
                signature_zero.len() as c_ulong,
            )
        };
        assert_eq!(rc, ERR_ENCODING);
    }

    #[test]
    fn ffi_mldsa_verify_rejects_tampered_signature() {
        let suite = MlDsaSuite::MlDsa44;
        let suite_id = c_uint::from(suite.suite_id());
        let (public_key, secret_key) = ffi_mldsa_keypair(suite);
        let message = b"pq ffi tamper";
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

        signature[0] ^= 0x01;
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
        assert_eq!(rc, ERR_VERIFICATION_FAILED);
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

        let mut rng = deterministic_chacha20_rng(
            HedgedRngSeed::from_entropy([0xBD; 32]),
            b"ffi-map-bad-encoding",
        );
        let bad_encoding = sign_mldsa(MlDsaSuite::MlDsa44, &[], &[], b"message", &mut rng)
            .expect_err("empty secret key must fail");
        assert_eq!(map_mldsa_error(&bad_encoding), ERR_ENCODING);

        let secret_mismatch = MlDsaError::SecretKeyMismatch {
            suite: MlDsaSuite::MlDsa44,
            kind: "test",
        };
        assert_eq!(map_mldsa_error(&secret_mismatch), ERR_ENCODING);

        let inert = MlDsaError::InertKeyMaterial {
            suite: MlDsaSuite::MlDsa44,
            kind: "test",
        };
        assert_eq!(map_mldsa_error(&inert), ERR_ENCODING);

        let keygen = MlDsaError::KeyGenerationFailed {
            suite: MlDsaSuite::MlDsa44,
            status: -1,
        };
        assert_eq!(map_mldsa_error(&keygen), ERR_KEYGEN);

        let rng = MlDsaError::Rng(crate::RngError);
        assert_eq!(map_mldsa_error(&rng), ERR_KEYGEN);

        let context = MlDsaError::ContextTooLong { len: 256 };
        assert_eq!(map_mldsa_error(&context), ERR_LENGTH_MISMATCH);
    }
}
