//! C-friendly bindings for the `soranet_pq` primitives.
use crate::{
    MlDsaSuite, MlKemSuite, decapsulate_mlkem, encapsulate_mlkem_from_os,
    generate_mldsa_keypair_from_os, generate_mlkem_keypair_from_os, mldsa::MlDsaError,
    mlkem::MlKemError, sign_mldsa_from_os, verify_mldsa,
};
use core::{
    convert::TryFrom,
    ffi::{c_int, c_uchar, c_uint},
    mem, slice,
};
const ERR_INVALID_SUITE: c_int = -1;
const ERR_NULL_POINTER: c_int = -2;
const ERR_LENGTH_MISMATCH: c_int = -3;
const ERR_ENCODING: c_int = -4;
const ERR_KEYGEN: c_int = -5;
const ERR_VERIFICATION_FAILED: c_int = -6;
const ERR_BUFFER_OVERLAP: c_int = -7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AddressRange {
    start: usize,
    end: usize,
}

impl AddressRange {
    fn overlaps(self, other: Self) -> bool {
        self.start < other.end && other.start < self.end
    }
}
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
        MlDsaError::Rng(_) => ERR_KEYGEN,
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
fn checked_nonempty_address_range(ptr: *const c_uchar, len: usize) -> Result<AddressRange, c_int> {
    debug_assert!(len != 0);
    if len > isize::MAX as usize {
        return Err(ERR_LENGTH_MISMATCH);
    }
    let start = ptr.addr();
    let end = start.checked_add(len).ok_or(ERR_LENGTH_MISMATCH)?;
    Ok(AddressRange { start, end })
}

fn checked_input_range(ptr: *const c_uchar, len: usize) -> Result<Option<AddressRange>, c_int> {
    if len == 0 {
        return Ok(None);
    }
    if ptr.is_null() {
        return Err(ERR_NULL_POINTER);
    }
    checked_nonempty_address_range(ptr, len).map(Some)
}

fn checked_input_range_exact(
    ptr: *const c_uchar,
    len: usize,
    expected: usize,
) -> Result<Option<AddressRange>, c_int> {
    let range = checked_input_range(ptr, len)?;
    if len != expected {
        return Err(ERR_LENGTH_MISMATCH);
    }
    Ok(range)
}

fn checked_output_range_exact(
    ptr: *mut c_uchar,
    len: usize,
    expected: usize,
) -> Result<Option<AddressRange>, c_int> {
    if ptr.is_null() {
        return Err(ERR_NULL_POINTER);
    }
    if len != expected {
        return Err(ERR_LENGTH_MISMATCH);
    }
    if len == 0 {
        return Ok(None);
    }
    checked_nonempty_address_range(ptr.cast_const(), len).map(Some)
}

fn checked_scalar_output_range<T>(ptr: *mut T) -> Result<AddressRange, c_int> {
    if ptr.is_null() {
        return Err(ERR_NULL_POINTER);
    }
    checked_nonempty_address_range(ptr.cast::<c_uchar>().cast_const(), mem::size_of::<T>())
}

fn ensure_disjoint(left: Option<AddressRange>, right: Option<AddressRange>) -> Result<(), c_int> {
    if left
        .zip(right)
        .is_some_and(|(left, right)| left.overlaps(right))
    {
        return Err(ERR_BUFFER_OVERLAP);
    }
    Ok(())
}

fn ensure_pairwise_disjoint(ranges: &[Option<AddressRange>]) -> Result<(), c_int> {
    for (index, left) in ranges.iter().enumerate() {
        for right in &ranges[index + 1..] {
            ensure_disjoint(*left, *right)?;
        }
    }
    Ok(())
}

fn validate_distinct_scalar_outputs(outputs: &[*mut c_uint]) -> Result<(), c_int> {
    for &output in outputs {
        checked_scalar_output_range(output)?;
    }
    for (index, &left) in outputs.iter().enumerate() {
        let left = Some(checked_scalar_output_range(left)?);
        for &right in &outputs[index + 1..] {
            ensure_disjoint(left, Some(checked_scalar_output_range(right)?))?;
        }
    }
    Ok(())
}

/// Borrow an input buffer after its numeric range has been checked.
///
/// # Safety
/// For nonzero `len`, `ptr` must remain valid and readable for `len` bytes for
/// the returned lifetime. Any writable range borrowed at the same time must be
/// disjoint from this range.
unsafe fn read_input<'a>(ptr: *const c_uchar, len: usize) -> Result<&'a [u8], c_int> {
    checked_input_range(ptr, len)?;
    if len == 0 {
        return Ok(&[]);
    }
    // SAFETY: the caller guarantees that the pointer is valid for `len` bytes.
    Ok(unsafe { slice::from_raw_parts(ptr, len) })
}
/// Borrow an exact-length input buffer after its numeric range has been checked.
///
/// # Safety
/// The requirements of [`read_input`] apply.
unsafe fn read_input_exact<'a>(
    ptr: *const c_uchar,
    len: usize,
    expected: usize,
) -> Result<&'a [u8], c_int> {
    checked_input_range_exact(ptr, len, expected)?;
    // SAFETY: the caller accepts `read_input`'s pointer-validity and aliasing
    // requirements; the exact-length preflight above adds no weaker path.
    unsafe { read_input(ptr, len) }
}
/// Borrow an exact-length writable output buffer after range preflight.
///
/// # Safety
/// `ptr` must remain valid and writable for `len` bytes for the returned
/// lifetime, and the range must be disjoint from every other live borrow.
unsafe fn write_output_exact<'a>(
    ptr: *mut c_uchar,
    len: usize,
    expected: usize,
) -> Result<&'a mut [u8], c_int> {
    checked_output_range_exact(ptr, len, expected)?;
    // SAFETY: the caller ensures the pointer references `len` writable bytes.
    Ok(unsafe { slice::from_raw_parts_mut(ptr, len) })
}
/// Write one scalar length after pointer-range preflight.
///
/// # Safety
/// `out` must be non-null, properly aligned, valid for one `c_uint`, and
/// exclusively writable for the duration of the write.
unsafe fn write_len(out: *mut c_uint, value: usize) -> Result<(), c_int> {
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
/// Every output pointer that would be written on success must be aligned and
/// valid for one `c_uint`. Pointer ranges must be representable without
/// wrapping the address space. Overlapping outputs are permitted as input to
/// the preflight and return [`ERR_BUFFER_OVERLAP`] before any write occurs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soranet_mlkem_parameters(
    suite_id: c_uint,
    public_key_len_out: *mut c_uint,
    secret_key_len_out: *mut c_uint,
    ciphertext_len_out: *mut c_uint,
    shared_secret_len_out: *mut c_uint,
) -> c_int {
    if let Err(code) = validate_distinct_scalar_outputs(&[
        public_key_len_out,
        secret_key_len_out,
        ciphertext_len_out,
        shared_secret_len_out,
    ]) {
        return code;
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
        // SAFETY: `validate_distinct_scalar_outputs` checked non-null,
        // non-wrapping, mutually disjoint ranges; the extern caller supplies
        // alignment and underlying writability.
        if let Err(code) = unsafe { write_len(out, value) } {
            return code;
        }
    }
    0
}
/// Generate an ML-KEM keypair and write it into the provided buffers.
///
/// # Safety
/// Each buffer must be valid for its declared length and occupy a non-wrapping
/// address range. Overlapping buffers are permitted as input to the preflight
/// and return [`ERR_BUFFER_OVERLAP`] before Rust references are formed;
/// disjointness is required only for a successful operation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soranet_mlkem_generate_keypair(
    suite_id: c_uint,
    public_key_out: *mut c_uchar,
    public_key_len: usize,
    secret_key_out: *mut c_uchar,
    secret_key_len: usize,
) -> c_int {
    let suite = match mlkem_suite_from_id(suite_id) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let public_range =
        match checked_output_range_exact(public_key_out, public_key_len, suite.public_key_len()) {
            Ok(range) => range,
            Err(code) => return code,
        };
    let secret_range =
        match checked_output_range_exact(secret_key_out, secret_key_len, suite.secret_key_len()) {
            Ok(range) => range,
            Err(code) => return code,
        };
    if let Err(code) = ensure_disjoint(public_range, secret_range) {
        return code;
    }
    // SAFETY: both exact writable ranges were preflighted and shown disjoint;
    // the extern caller supplies their underlying validity.
    let public_buf =
        match unsafe { write_output_exact(public_key_out, public_key_len, suite.public_key_len()) }
        {
            Ok(buf) => buf,
            Err(code) => return code,
        };
    // SAFETY: same preflight and caller-validity argument as `public_buf`.
    let secret_buf =
        match unsafe { write_output_exact(secret_key_out, secret_key_len, suite.secret_key_len()) }
        {
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
/// Each buffer must be valid for its declared length and occupy a non-wrapping
/// address range. Overlapping ranges are permitted as input to the preflight
/// and return [`ERR_BUFFER_OVERLAP`] before Rust references are formed;
/// disjointness is required only for a successful operation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soranet_mlkem_encapsulate(
    suite_id: c_uint,
    public_key: *const c_uchar,
    public_key_len: usize,
    ciphertext_out: *mut c_uchar,
    ciphertext_len: usize,
    shared_secret_out: *mut c_uchar,
    shared_secret_len: usize,
) -> c_int {
    let suite = match mlkem_suite_from_id(suite_id) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let public_range =
        match checked_input_range_exact(public_key, public_key_len, suite.public_key_len()) {
            Ok(range) => range,
            Err(code) => return code,
        };
    let ciphertext_range =
        match checked_output_range_exact(ciphertext_out, ciphertext_len, suite.ciphertext_len()) {
            Ok(range) => range,
            Err(code) => return code,
        };
    let shared_range = match checked_output_range_exact(
        shared_secret_out,
        shared_secret_len,
        suite.shared_secret_len(),
    ) {
        Ok(range) => range,
        Err(code) => return code,
    };
    if let Err(code) = ensure_pairwise_disjoint(&[public_range, ciphertext_range, shared_range]) {
        return code;
    }
    // SAFETY: all three exact ranges were preflighted as pairwise disjoint;
    // the extern caller supplies their underlying validity.
    let pk = match unsafe { read_input_exact(public_key, public_key_len, suite.public_key_len()) } {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    // SAFETY: the output range was preflighted and is disjoint from every
    // other live range in this call.
    let ciphertext_buf =
        match unsafe { write_output_exact(ciphertext_out, ciphertext_len, suite.ciphertext_len()) }
        {
            Ok(buf) => buf,
            Err(code) => return code,
        };
    // SAFETY: the output range was preflighted and is disjoint from every
    // other live range in this call.
    let shared_buf = match unsafe {
        write_output_exact(
            shared_secret_out,
            shared_secret_len,
            suite.shared_secret_len(),
        )
    } {
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
/// Input pointers must reference valid encodings and output buffers must match
/// the suite's lengths. Every range must be non-wrapping. An output that
/// overlaps either input is permitted as input to the preflight and returns
/// [`ERR_BUFFER_OVERLAP`] before Rust references are formed. The two read-only
/// inputs may overlap each other.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soranet_mlkem_decapsulate(
    suite_id: c_uint,
    secret_key: *const c_uchar,
    secret_key_len: usize,
    ciphertext: *const c_uchar,
    ciphertext_len: usize,
    shared_secret_out: *mut c_uchar,
    shared_secret_len: usize,
) -> c_int {
    let suite = match mlkem_suite_from_id(suite_id) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let secret_range =
        match checked_input_range_exact(secret_key, secret_key_len, suite.secret_key_len()) {
            Ok(range) => range,
            Err(code) => return code,
        };
    let ciphertext_range =
        match checked_input_range_exact(ciphertext, ciphertext_len, suite.ciphertext_len()) {
            Ok(range) => range,
            Err(code) => return code,
        };
    let shared_range = match checked_output_range_exact(
        shared_secret_out,
        shared_secret_len,
        suite.shared_secret_len(),
    ) {
        Ok(range) => range,
        Err(code) => return code,
    };
    if let Err(code) = ensure_disjoint(secret_range, shared_range)
        .and_then(|()| ensure_disjoint(ciphertext_range, shared_range))
    {
        return code;
    }
    // SAFETY: both input ranges and the writable output were preflighted; the
    // output is disjoint and the two read-only inputs may overlap safely.
    let sk = match unsafe { read_input_exact(secret_key, secret_key_len, suite.secret_key_len()) } {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    // SAFETY: same preflight and caller-validity argument as `sk`.
    let ct = match unsafe { read_input_exact(ciphertext, ciphertext_len, suite.ciphertext_len()) } {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    // SAFETY: the output was preflighted as disjoint from both live inputs.
    let shared_buf = match unsafe {
        write_output_exact(
            shared_secret_out,
            shared_secret_len,
            suite.shared_secret_len(),
        )
    } {
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
/// Every output pointer that would be written on success must be aligned and
/// valid for one `c_uint`. Pointer ranges must be representable without
/// wrapping the address space. Overlapping outputs are permitted as input to
/// the preflight and return [`ERR_BUFFER_OVERLAP`] before any write occurs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soranet_mldsa_parameters(
    suite_id: c_uint,
    public_key_len_out: *mut c_uint,
    secret_key_len_out: *mut c_uint,
    signature_len_out: *mut c_uint,
) -> c_int {
    if let Err(code) = validate_distinct_scalar_outputs(&[
        public_key_len_out,
        secret_key_len_out,
        signature_len_out,
    ]) {
        return code;
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
        // SAFETY: `validate_distinct_scalar_outputs` checked non-null,
        // non-wrapping, mutually disjoint ranges; the extern caller supplies
        // alignment and underlying writability.
        if let Err(code) = unsafe { write_len(out, value) } {
            return code;
        }
    }
    0
}
/// Generate an ML-DSA keypair and store it in the supplied buffers.
///
/// # Safety
/// Each buffer must be valid for its declared length and occupy a non-wrapping
/// address range. Overlapping buffers are permitted as input to the preflight
/// and return [`ERR_BUFFER_OVERLAP`] before Rust references are formed;
/// disjointness is required only for a successful operation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soranet_mldsa_generate_keypair(
    suite_id: c_uint,
    public_key_out: *mut c_uchar,
    public_key_len: usize,
    secret_key_out: *mut c_uchar,
    secret_key_len: usize,
) -> c_int {
    let suite = match mldsa_suite_from_id(suite_id) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let public_range =
        match checked_output_range_exact(public_key_out, public_key_len, suite.public_key_len()) {
            Ok(range) => range,
            Err(code) => return code,
        };
    let secret_range =
        match checked_output_range_exact(secret_key_out, secret_key_len, suite.secret_key_len()) {
            Ok(range) => range,
            Err(code) => return code,
        };
    if let Err(code) = ensure_disjoint(public_range, secret_range) {
        return code;
    }
    // SAFETY: both exact writable ranges were preflighted and shown disjoint;
    // the extern caller supplies their underlying validity.
    let public_buf =
        match unsafe { write_output_exact(public_key_out, public_key_len, suite.public_key_len()) }
        {
            Ok(buf) => buf,
            Err(code) => return code,
        };
    // SAFETY: same preflight and caller-validity argument as `public_buf`.
    let secret_buf =
        match unsafe { write_output_exact(secret_key_out, secret_key_len, suite.secret_key_len()) }
        {
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
/// Input pointers must reference valid buffers and the signature buffer must
/// match the suite length. Every range must be non-wrapping. A signature output
/// that overlaps either input is permitted as input to the preflight and
/// returns [`ERR_BUFFER_OVERLAP`] before Rust references are formed. The two
/// read-only inputs may overlap each other; a null message pointer is permitted
/// only for length zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soranet_mldsa_sign(
    suite_id: c_uint,
    secret_key: *const c_uchar,
    secret_key_len: usize,
    message: *const c_uchar,
    message_len: usize,
    signature_out: *mut c_uchar,
    signature_len: usize,
) -> c_int {
    let suite = match mldsa_suite_from_id(suite_id) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let secret_range =
        match checked_input_range_exact(secret_key, secret_key_len, suite.secret_key_len()) {
            Ok(range) => range,
            Err(code) => return code,
        };
    let message_range = match checked_input_range(message, message_len) {
        Ok(range) => range,
        Err(code) => return code,
    };
    let signature_range =
        match checked_output_range_exact(signature_out, signature_len, suite.signature_len()) {
            Ok(range) => range,
            Err(code) => return code,
        };
    if let Err(code) = ensure_disjoint(secret_range, signature_range)
        .and_then(|()| ensure_disjoint(message_range, signature_range))
    {
        return code;
    }
    // SAFETY: the input ranges and exact output were preflighted, and the
    // writable signature is disjoint from both live inputs.
    let secret =
        match unsafe { read_input_exact(secret_key, secret_key_len, suite.secret_key_len()) } {
            Ok(bytes) => bytes,
            Err(code) => return code,
        };
    // SAFETY: the message range was preflighted and is disjoint from the
    // writable signature output.
    let message = match unsafe { read_input(message, message_len) } {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    // SAFETY: the exact output was preflighted as disjoint from both inputs.
    let signature_buf =
        match unsafe { write_output_exact(signature_out, signature_len, suite.signature_len()) } {
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
/// Inputs must reference valid encodings with lengths matching the selected
/// suite, and each range must be representable without wrapping the address
/// space. Read-only input ranges may overlap each other; a null message pointer
/// is permitted only for length zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soranet_mldsa_verify(
    suite_id: c_uint,
    public_key: *const c_uchar,
    public_key_len: usize,
    message: *const c_uchar,
    message_len: usize,
    signature: *const c_uchar,
    signature_len: usize,
) -> c_int {
    let suite = match mldsa_suite_from_id(suite_id) {
        Ok(value) => value,
        Err(code) => return code,
    };
    if let Err(code) = checked_input_range_exact(public_key, public_key_len, suite.public_key_len())
        .and_then(|_| checked_input_range(message, message_len))
        .and_then(|_| checked_input_range_exact(signature, signature_len, suite.signature_len()))
    {
        return code;
    }
    // SAFETY: every input range was preflighted; all borrows are read-only and
    // may overlap, while the extern caller supplies their underlying validity.
    let pk = match unsafe { read_input_exact(public_key, public_key_len, suite.public_key_len()) } {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    // SAFETY: same read-only preflight and caller-validity argument as `pk`.
    let message = match unsafe { read_input(message, message_len) } {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    // SAFETY: same read-only preflight and caller-validity argument as `pk`.
    let sig = match unsafe { read_input_exact(signature, signature_len, suite.signature_len()) } {
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
    use super::*;
    use crate::{HedgedRngSeed, deterministic_chacha20_rng, sign_mldsa};
    use core::ptr;
    use pqcrypto_traits::sign::VerificationError;
    use sha3::{Digest, Sha3_256};
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
                public_key.len(),
                secret_key.as_mut_ptr(),
                secret_key.len(),
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
                public_key.len(),
                secret_key.as_mut_ptr(),
                secret_key.len(),
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
                public_key.len(),
                ciphertext.as_mut_ptr(),
                ciphertext.len(),
                shared_secret.as_mut_ptr(),
                shared_secret.len(),
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
        // SAFETY: every nonempty pointer below names its backing array for the
        // duration of the borrow; the zero-length cases return before dereference.
        let empty = unsafe { read_input(ptr::null(), 0) }.expect("empty null input is allowed");
        assert!(empty.is_empty());
        let input = [0x11, 0x22];
        // SAFETY: `input` is readable for its exact length.
        let exact =
            unsafe { read_input_exact(input.as_ptr(), input.len(), input.len()) }.expect("exact");
        assert_eq!(exact, input);
        assert_eq!(
            // SAFETY: the shorter requested range remains inside `input`.
            unsafe { read_input_exact(input.as_ptr(), input.len() - 1, input.len()) }.err(),
            Some(ERR_LENGTH_MISMATCH)
        );
        let mut output = [0u8; 2];
        // SAFETY: `output` is exclusively writable for two bytes.
        assert!(unsafe { write_output_exact(output.as_mut_ptr(), 2, 2) }.is_ok());
        assert_eq!(
            // SAFETY: the shorter requested range remains inside `output`.
            unsafe { write_output_exact(output.as_mut_ptr(), 1, 2) }.err(),
            Some(ERR_LENGTH_MISMATCH)
        );
        assert_eq!(
            // SAFETY: the helper rejects the null pointer before constructing a slice.
            unsafe { write_output_exact(ptr::null_mut(), 0, 0) }.err(),
            Some(ERR_NULL_POINTER)
        );
    }

    #[test]
    fn ffi_pointer_preflight_rejects_overlap_and_wrapping_ranges() {
        let bytes = [0_u8; 8];
        let first = checked_input_range(bytes.as_ptr(), 4).expect("first range");
        let overlapping =
            checked_input_range(bytes.as_ptr().wrapping_add(2), 4).expect("overlap range");
        let adjacent =
            checked_input_range(bytes.as_ptr().wrapping_add(4), 4).expect("adjacent range");
        assert_eq!(ensure_disjoint(first, overlapping), Err(ERR_BUFFER_OVERLAP));
        assert_eq!(ensure_disjoint(first, adjacent), Ok(()));

        let wrapping = usize::MAX.wrapping_sub(1) as *const c_uchar;
        assert_eq!(checked_input_range(wrapping, 4), Err(ERR_LENGTH_MISMATCH));
        assert_eq!(
            checked_input_range(bytes.as_ptr(), isize::MAX as usize + 1),
            Err(ERR_LENGTH_MISMATCH)
        );
    }

    #[test]
    fn ffi_mlkem_operations_reject_overlapping_buffers_before_crypto() {
        let kem = MlKemSuite::MlKem512;
        let kem_id = c_uint::from(kem.kem_id());
        let kem_params = kem.parameters();
        let mut kem_key_storage = vec![0_u8; kem_params.secret_key];
        let rc = unsafe {
            soranet_mlkem_generate_keypair(
                kem_id,
                kem_key_storage.as_mut_ptr(),
                kem_params.public_key,
                kem_key_storage.as_mut_ptr(),
                kem_params.secret_key,
            )
        };
        assert_eq!(rc, ERR_BUFFER_OVERLAP);

        let mut kem_public_and_ciphertext =
            vec![0_u8; kem_params.public_key.max(kem_params.ciphertext)];
        let mut kem_shared = vec![0_u8; kem_params.shared_secret];
        let rc = unsafe {
            soranet_mlkem_encapsulate(
                kem_id,
                kem_public_and_ciphertext.as_ptr(),
                kem_params.public_key,
                kem_public_and_ciphertext.as_mut_ptr(),
                kem_params.ciphertext,
                kem_shared.as_mut_ptr(),
                kem_params.shared_secret,
            )
        };
        assert_eq!(rc, ERR_BUFFER_OVERLAP);

        let kem_public = vec![0_u8; kem_params.public_key];
        let mut kem_output_storage =
            vec![0_u8; kem_params.ciphertext.max(kem_params.shared_secret)];
        let rc = unsafe {
            soranet_mlkem_encapsulate(
                kem_id,
                kem_public.as_ptr(),
                kem_params.public_key,
                kem_output_storage.as_mut_ptr(),
                kem_params.ciphertext,
                kem_output_storage.as_mut_ptr(),
                kem_params.shared_secret,
            )
        };
        assert_eq!(rc, ERR_BUFFER_OVERLAP);

        let mut kem_secret_and_shared = vec![0_u8; kem_params.secret_key];
        let kem_ciphertext = vec![0_u8; kem_params.ciphertext];
        let rc = unsafe {
            soranet_mlkem_decapsulate(
                kem_id,
                kem_secret_and_shared.as_ptr(),
                kem_params.secret_key,
                kem_ciphertext.as_ptr(),
                kem_params.ciphertext,
                kem_secret_and_shared.as_mut_ptr(),
                kem_params.shared_secret,
            )
        };
        assert_eq!(rc, ERR_BUFFER_OVERLAP);
    }

    #[test]
    fn ffi_mldsa_operations_reject_overlapping_buffers_before_crypto() {
        let dsa = MlDsaSuite::MlDsa44;
        let dsa_id = c_uint::from(dsa.suite_id());
        let mut dsa_key_storage = vec![0_u8; dsa.secret_key_len()];
        let rc = unsafe {
            soranet_mldsa_generate_keypair(
                dsa_id,
                dsa_key_storage.as_mut_ptr(),
                dsa.public_key_len(),
                dsa_key_storage.as_mut_ptr(),
                dsa.secret_key_len(),
            )
        };
        assert_eq!(rc, ERR_BUFFER_OVERLAP);

        let mut dsa_secret_and_signature = vec![0_u8; dsa.secret_key_len()];
        let message = b"overlap must fail before key validation";
        let rc = unsafe {
            soranet_mldsa_sign(
                dsa_id,
                dsa_secret_and_signature.as_ptr(),
                dsa.secret_key_len(),
                message.as_ptr(),
                message.len(),
                dsa_secret_and_signature.as_mut_ptr(),
                dsa.signature_len(),
            )
        };
        assert_eq!(rc, ERR_BUFFER_OVERLAP);

        let dsa_secret = vec![0_u8; dsa.secret_key_len()];
        let mut dsa_message_and_signature = vec![0_u8; dsa.signature_len()];
        let rc = unsafe {
            soranet_mldsa_sign(
                dsa_id,
                dsa_secret.as_ptr(),
                dsa.secret_key_len(),
                dsa_message_and_signature.as_ptr(),
                32,
                dsa_message_and_signature.as_mut_ptr(),
                dsa.signature_len(),
            )
        };
        assert_eq!(rc, ERR_BUFFER_OVERLAP);
    }

    #[test]
    fn ffi_parameter_queries_reject_overlapping_outputs() {
        let kem_id = c_uint::from(MlKemSuite::MlKem512.kem_id());

        let mut parameter = 0_u32;
        let mut ciphertext_len = 0_u32;
        let mut shared_secret_len = 0_u32;
        let rc = unsafe {
            soranet_mlkem_parameters(
                kem_id,
                &raw mut parameter,
                &raw mut parameter,
                &raw mut ciphertext_len,
                &raw mut shared_secret_len,
            )
        };
        assert_eq!(rc, ERR_BUFFER_OVERLAP);

        let dsa_id = c_uint::from(MlDsaSuite::MlDsa44.suite_id());
        let mut dsa_parameter = 0_u32;
        let mut dsa_signature_len = 0_u32;
        let rc = unsafe {
            soranet_mldsa_parameters(
                dsa_id,
                &raw mut dsa_parameter,
                &raw mut dsa_parameter,
                &raw mut dsa_signature_len,
            )
        };
        assert_eq!(rc, ERR_BUFFER_OVERLAP);
    }

    #[test]
    fn ffi_exported_operation_rejects_wrapping_range_before_dereference() {
        let suite = MlDsaSuite::MlDsa44;
        let wrapping = usize::MAX.wrapping_sub(8) as *const c_uchar;
        let rc = unsafe {
            soranet_mldsa_sign(
                c_uint::from(suite.suite_id()),
                wrapping,
                suite.secret_key_len(),
                ptr::null(),
                0,
                ptr::null_mut(),
                suite.signature_len(),
            )
        };
        assert_eq!(rc, ERR_LENGTH_MISMATCH);

        let rc = unsafe {
            soranet_mldsa_verify(
                c_uint::from(suite.suite_id()),
                wrapping,
                suite.public_key_len(),
                ptr::null(),
                0,
                ptr::null(),
                suite.signature_len(),
            )
        };
        assert_eq!(rc, ERR_LENGTH_MISMATCH);
    }

    #[test]
    fn ffi_length_abi_matches_c_size_t_and_swift_uint() {
        type MlKemGenerateKeypair =
            unsafe extern "C" fn(c_uint, *mut c_uchar, usize, *mut c_uchar, usize) -> c_int;
        type MlKemEncapsulate = unsafe extern "C" fn(
            c_uint,
            *const c_uchar,
            usize,
            *mut c_uchar,
            usize,
            *mut c_uchar,
            usize,
        ) -> c_int;
        type MlKemDecapsulate = unsafe extern "C" fn(
            c_uint,
            *const c_uchar,
            usize,
            *const c_uchar,
            usize,
            *mut c_uchar,
            usize,
        ) -> c_int;
        type MlDsaGenerateKeypair = MlKemGenerateKeypair;
        type MlDsaSign = unsafe extern "C" fn(
            c_uint,
            *const c_uchar,
            usize,
            *const c_uchar,
            usize,
            *mut c_uchar,
            usize,
        ) -> c_int;
        type MlDsaVerify = unsafe extern "C" fn(
            c_uint,
            *const c_uchar,
            usize,
            *const c_uchar,
            usize,
            *const c_uchar,
            usize,
        ) -> c_int;

        let _: MlKemGenerateKeypair = soranet_mlkem_generate_keypair;
        let _: MlKemEncapsulate = soranet_mlkem_encapsulate;
        let _: MlKemDecapsulate = soranet_mlkem_decapsulate;
        let _: MlDsaGenerateKeypair = soranet_mldsa_generate_keypair;
        let _: MlDsaSign = soranet_mldsa_sign;
        let _: MlDsaVerify = soranet_mldsa_verify;

        assert_eq!(
            mem::size_of::<usize>(),
            mem::size_of::<*const ()>(),
            "Rust FFI lengths must track the target pointer width"
        );
        #[cfg(all(target_os = "windows", target_pointer_width = "64"))]
        assert_ne!(
            mem::size_of::<usize>(),
            mem::size_of::<core::ffi::c_ulong>(),
            "LLP64 is the platform where c_ulong cannot represent C size_t"
        );

        let header = include_str!("../include/soranet_pq.h");
        assert!(header.contains("size_t public_key_len"));
        assert!(header.contains("size_t message_len"));
        assert!(!header.contains("unsigned long public_key_len"));
        assert!(!header.contains("unsigned long message_len"));
        assert!(header.contains("SORANET_PQ_ERR_BUFFER_OVERLAP -7"));

        let swift = include_str!("../../../IrohaSwift/Sources/IrohaSwift/NativeBridge.swift");
        let start = swift
            .find("private typealias MldsaGenerateKeypairFn")
            .expect("Swift ML-DSA FFI aliases");
        let end = swift[start..]
            .find("private typealias ConnectGenerateKeypairFn")
            .map(|offset| start + offset)
            .expect("end of Swift ML-DSA FFI aliases");
        let aliases = &swift[start..end];
        assert!(aliases.contains("UnsafeMutablePointer<UInt8>?, UInt"));
        assert!(!aliases.contains("CUnsignedLong"));
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
                secret_key.len(),
                ciphertext.as_ptr(),
                ciphertext.len(),
                shared_receiver.as_mut_ptr(),
                shared_receiver.len(),
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
                public_key.len(),
                ciphertext.as_mut_ptr(),
                ciphertext.len(),
                shared_secret.as_mut_ptr(),
                shared_secret.len(),
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
                public_key.len(),
                ciphertext.as_mut_ptr(),
                ciphertext.len(),
                shared_secret.as_mut_ptr(),
                shared_secret.len(),
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
                secret_key.len(),
                ciphertext.as_ptr(),
                ciphertext.len(),
                shared_secret.as_mut_ptr(),
                shared_secret.len(),
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
                secret_key.len(),
                ciphertext.as_ptr(),
                ciphertext.len(),
                shared_secret.as_mut_ptr(),
                shared_secret.len(),
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
                secret_key.len(),
                ciphertext.as_ptr(),
                ciphertext.len(),
                shared_secret.as_mut_ptr(),
                shared_secret.len(),
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
                secret_key.len(),
                ciphertext.as_ptr(),
                ciphertext.len(),
                shared_secret.as_mut_ptr(),
                shared_secret.len(),
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
                public_key.len(),
                secret_key.as_mut_ptr(),
                secret_key.len(),
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
                params.public_key,
                secret_key.as_mut_ptr(),
                secret_key.len(),
            )
        };
        assert_eq!(rc, ERR_NULL_POINTER);
        let rc = unsafe {
            soranet_mlkem_generate_keypair(
                c_uint::from(suite.kem_id()),
                public_key.as_mut_ptr(),
                public_key.len(),
                ptr::null_mut(),
                params.secret_key,
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
                params.public_key,
                ciphertext.as_mut_ptr(),
                ciphertext.len(),
                shared_secret.as_mut_ptr(),
                shared_secret.len(),
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
                public_key.len(),
                ciphertext.as_mut_ptr(),
                ciphertext.len(),
                shared_secret.as_mut_ptr(),
                shared_secret.len(),
            )
        };
        assert_eq!(rc, ERR_LENGTH_MISMATCH);
        let mut ciphertext = vec![0u8; params.ciphertext];
        let mut shared_secret = vec![0u8; params.shared_secret - 1];
        let rc = unsafe {
            soranet_mlkem_encapsulate(
                c_uint::from(suite.kem_id()),
                public_key.as_ptr(),
                public_key.len(),
                ciphertext.as_mut_ptr(),
                ciphertext.len(),
                shared_secret.as_mut_ptr(),
                shared_secret.len(),
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
                public_key.len(),
                ptr::null_mut(),
                params.ciphertext,
                shared_secret.as_mut_ptr(),
                shared_secret.len(),
            )
        };
        assert_eq!(rc, ERR_NULL_POINTER);
        let rc = unsafe {
            soranet_mlkem_encapsulate(
                c_uint::from(suite.kem_id()),
                public_key.as_ptr(),
                public_key.len(),
                ciphertext.as_mut_ptr(),
                ciphertext.len(),
                ptr::null_mut(),
                params.shared_secret,
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
                secret_key.len(),
                ciphertext.as_ptr(),
                ciphertext.len(),
                shared_secret.as_mut_ptr(),
                shared_secret.len(),
            )
        };
        assert_eq!(rc, ERR_LENGTH_MISMATCH);
        let mut shared_secret = vec![0u8; params.shared_secret];
        let rc = unsafe {
            soranet_mlkem_decapsulate(
                suite_id,
                secret_key.as_ptr(),
                secret_key.len() - 1,
                ciphertext.as_ptr(),
                ciphertext.len(),
                shared_secret.as_mut_ptr(),
                shared_secret.len(),
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
                secret_key.len(),
                ciphertext.as_ptr(),
                ciphertext.len(),
                shared_secret.as_mut_ptr(),
                shared_secret.len(),
            )
        };
        assert_eq!(rc, ERR_NULL_POINTER);
        let rc = unsafe {
            soranet_mlkem_decapsulate(
                suite_id,
                secret_key.as_ptr(),
                secret_key.len(),
                ptr::null(),
                ciphertext.len(),
                shared_secret.as_mut_ptr(),
                shared_secret.len(),
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
                secret_key.len(),
                ciphertext.as_ptr(),
                ciphertext.len(),
                ptr::null_mut(),
                params.shared_secret,
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
                secret_key.len(),
                message.as_ptr(),
                message.len(),
                signature.as_mut_ptr(),
                signature.len(),
            )
        };
        assert_eq!(rc, 0);
        let rc = unsafe {
            soranet_mldsa_verify(
                suite_id,
                public_key.as_ptr(),
                public_key.len(),
                message.as_ptr(),
                message.len(),
                signature.as_ptr(),
                signature.len(),
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
                public_key.len(),
                secret_key.as_mut_ptr(),
                secret_key.len(),
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
                suite.public_key_len(),
                secret_key.as_mut_ptr(),
                secret_key.len(),
            )
        };
        assert_eq!(rc, ERR_NULL_POINTER);
        let rc = unsafe {
            soranet_mldsa_generate_keypair(
                c_uint::from(suite.suite_id()),
                public_key.as_mut_ptr(),
                public_key.len(),
                ptr::null_mut(),
                suite.secret_key_len(),
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
                secret_key.len(),
                ptr::null(),
                0,
                signature.as_mut_ptr(),
                signature.len(),
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
                secret_key.len(),
                ptr::null(),
                1,
                signature.as_mut_ptr(),
                signature.len(),
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
                secret_key.len(),
                message.as_ptr(),
                message.len(),
                ptr::null_mut(),
                suite.signature_len(),
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
                secret_key.len() - 1,
                message.as_ptr(),
                message.len(),
                signature.as_mut_ptr(),
                signature.len(),
            )
        };
        assert_eq!(rc, ERR_LENGTH_MISMATCH);
        let rc = unsafe {
            soranet_mldsa_sign(
                c_uint::from(suite.suite_id()),
                secret_key.as_ptr(),
                secret_key.len(),
                message.as_ptr(),
                message.len(),
                signature.as_mut_ptr(),
                signature.len() - 1,
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
                secret_key.len(),
                message.as_ptr(),
                message.len(),
                signature.as_mut_ptr(),
                signature.len(),
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
                secret_key.len(),
                message.as_ptr(),
                message.len(),
                signature.as_mut_ptr(),
                signature.len(),
            )
        };
        assert_eq!(rc, 0);
        let rc = unsafe {
            soranet_mldsa_verify(
                suite_id,
                ptr::null(),
                public_key.len(),
                message.as_ptr(),
                message.len(),
                signature.as_ptr(),
                signature.len(),
            )
        };
        assert_eq!(rc, ERR_NULL_POINTER);
        let rc = unsafe {
            soranet_mldsa_verify(
                suite_id,
                public_key.as_ptr(),
                public_key.len(),
                message.as_ptr(),
                message.len(),
                ptr::null(),
                signature.len(),
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
                secret_key.len(),
                message.as_ptr(),
                message.len(),
                signature.as_mut_ptr(),
                signature.len(),
            )
        };
        assert_eq!(rc, 0);
        let rc = unsafe {
            soranet_mldsa_verify(
                suite_id,
                public_key.as_ptr(),
                public_key.len(),
                ptr::null(),
                message.len(),
                signature.as_ptr(),
                signature.len(),
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
                secret_key.len(),
                ptr::null(),
                0,
                signature.as_mut_ptr(),
                signature.len(),
            )
        };
        assert_eq!(rc, 0);
        let rc = unsafe {
            soranet_mldsa_verify(
                suite_id,
                public_key.as_ptr(),
                public_key.len(),
                ptr::null(),
                0,
                signature.as_ptr(),
                signature.len(),
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
                secret_key.len(),
                message.as_ptr(),
                message.len(),
                signature.as_mut_ptr(),
                signature.len(),
            )
        };
        assert_eq!(rc, 0);
        let rc = unsafe {
            soranet_mldsa_verify(
                suite_id,
                public_key.as_ptr(),
                public_key.len() - 1,
                message.as_ptr(),
                message.len(),
                signature.as_ptr(),
                signature.len(),
            )
        };
        assert_eq!(rc, ERR_LENGTH_MISMATCH);
        let rc = unsafe {
            soranet_mldsa_verify(
                suite_id,
                public_key.as_ptr(),
                public_key.len(),
                message.as_ptr(),
                message.len(),
                signature.as_ptr(),
                signature.len() - 1,
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
                secret_key.len(),
                message.as_ptr(),
                message.len(),
                signature.as_mut_ptr(),
                signature.len(),
            )
        };
        assert_eq!(rc, 0);
        let public_key_zero = vec![0u8; suite.public_key_len()];
        let rc = unsafe {
            soranet_mldsa_verify(
                suite_id,
                public_key_zero.as_ptr(),
                public_key_zero.len(),
                message.as_ptr(),
                message.len(),
                signature.as_ptr(),
                signature.len(),
            )
        };
        assert_eq!(rc, ERR_ENCODING);
        let signature_zero = vec![0u8; suite.signature_len()];
        let rc = unsafe {
            soranet_mldsa_verify(
                suite_id,
                public_key.as_ptr(),
                public_key.len(),
                message.as_ptr(),
                message.len(),
                signature_zero.as_ptr(),
                signature_zero.len(),
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
                secret_key.len(),
                message.as_ptr(),
                message.len(),
                signature.as_mut_ptr(),
                signature.len(),
            )
        };
        assert_eq!(rc, 0);
        signature[0] ^= 0x01;
        let rc = unsafe {
            soranet_mldsa_verify(
                suite_id,
                public_key.as_ptr(),
                public_key.len(),
                message.as_ptr(),
                message.len(),
                signature.as_ptr(),
                signature.len(),
            )
        };
        assert_eq!(rc, ERR_VERIFICATION_FAILED);
    }
    #[test]
    fn write_len_validates_bounds() {
        let mut slot: c_uint = 0;
        let ptr = core::ptr::addr_of_mut!(slot);
        // SAFETY: `ptr` points to the live, aligned `slot` object for one
        // `c_uint` write.
        assert!(unsafe { write_len(ptr, c_uint::MAX as usize) }.is_ok());
        assert_eq!(slot, c_uint::MAX);
        if usize::BITS > c_uint::BITS {
            let too_large = (c_uint::MAX as usize).saturating_add(1);
            // SAFETY: `ptr` still points to the live, aligned `slot` object.
            assert_eq!(
                unsafe { write_len(ptr, too_large) },
                Err(ERR_LENGTH_MISMATCH)
            );
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
        let rng = MlDsaError::Rng(crate::RngError);
        assert_eq!(map_mldsa_error(&rng), ERR_KEYGEN);
        let context = MlDsaError::ContextTooLong { len: 256 };
        assert_eq!(map_mldsa_error(&context), ERR_LENGTH_MISMATCH);
    }
}
