//! SCCP v1 signatures and signature sets (spec §3.8).
//!
//! - A signature is 65 bytes `r ‖ s ‖ v` with `v ∈ {27, 28}`, `1 ≤ r < N`, `1 ≤ s ≤ HALF_N`
//!   (low-S); recovery that yields the zero address is invalid.
//! - Signers use RFC 6979 deterministic nonces over the 32-byte digest (prehash). If the
//!   recovery id is 2 or 3 (`v` would be 29 or 30, probability about `2^-127`), the signer
//!   discards the signature and re-signs with fresh RFC 6979 extra entropy.
//! - A signature set is `(signer_bitmap: u32, signatures)`: bit `i` names roster member `i`,
//!   bits `≥ n` are zero, and `signatures` concatenates one signature per set bit in ascending
//!   bit order. Each set bit's member is nonzero and equals the recovered address.
//! - The address of a key is `keccak256(X ‖ Y)[12..32]`; verifiers mask a recovered address
//!   word to its low 160 bits before any comparison.

use iroha_crypto::{EcdsaSecp256k1Sha256, KeyGenOption};
use sha2::{Digest as _, Sha256};

use super::constants::{SECP256K1_HALF_N, SECP256K1_N, SIGNATURE_BYTES};

/// Upper bound on signing attempts before [`SignatureError::SigningExhausted`].
pub const MAX_SIGNING_ATTEMPTS: u32 = 8;

unit_error! {
    /// Signature, signature-set and signing errors.
    pub enum SignatureError {
        /// `v` is not 27 or 28.
        BadRecoveryByte => "signature v must be 27 or 28",
        /// `r` is zero or at least `N`.
        BadR => "signature r must satisfy 1 <= r < N",
        /// `s` is zero or above `HALF_N`.
        BadS => "signature s must satisfy 1 <= s <= HALF_N",
        /// Public-key recovery failed.
        RecoveryFailed => "signature public-key recovery failed",
        /// Recovery yielded the zero address.
        ZeroAddress => "signature recovers the zero address",
        /// The recovered address differs from the expected member.
        WrongSigner => "signature recovers a different address",
        /// A set bit at or above `n`.
        BitmapOutOfRange => "signer bitmap has a bit at or above n",
        /// A set bit addresses a zero (keyless) member.
        ZeroMember => "signer bitmap addresses a zero member",
        /// `signatures` is not exactly 65 bytes per set bit.
        BadLength => "signatures must be 65 bytes per set bitmap bit",
        /// Fewer than `t` signatures.
        TooFewSignatures => "signature set has fewer than t signatures",
        /// The roster has more than 32 members.
        RosterTooLarge => "a signer bitmap addresses at most 32 members",
        /// A signer index is repeated or out of range while building a set.
        BadSignerIndex => "signer indexes must be distinct and below n",
        /// The secret key is not a valid secp256k1 scalar.
        InvalidSecret => "bridge key secret is not a valid secp256k1 scalar",
        /// The public key is not a canonical compressed secp256k1 point.
        InvalidPublicKey => "bridge public key is not a canonical compressed secp256k1 point",
        /// The signing backend failed.
        SigningFailed => "secp256k1 signing failed",
        /// Fresh entropy could not be drawn from the OS.
        EntropyUnavailable => "OS randomness is unavailable for re-signing",
        /// Every attempt produced a recovery id of 2 or 3.
        SigningExhausted => "every signing attempt produced a recovery id of 2 or 3",
    }
}

// ---------------------------------------------------------------------------------------------
// 256-bit big-endian helpers
// ---------------------------------------------------------------------------------------------

fn be_sub(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut out = [0_u8; 32];
    let mut borrow = 0_i16;
    for index in (0..32).rev() {
        let mut value = i16::from(a[index]) - i16::from(b[index]) - borrow;
        borrow = 0;
        if value < 0 {
            value += 256;
            borrow = 1;
        }
        out[index] = u8::try_from(value).expect("a byte after borrow");
    }
    out
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

/// Best-effort wipe of secret bytes without `unsafe`: overwrite, then hide the buffer behind an
/// optimization barrier so the stores are not elided.
pub(crate) fn wipe(bytes: &mut [u8]) {
    bytes.fill(0);
    std::hint::black_box(&mut *bytes);
}

// ---------------------------------------------------------------------------------------------
// Addresses
// ---------------------------------------------------------------------------------------------

/// The 20-byte address of a compressed secp256k1 public key (`keccak256(X ‖ Y)[12..32]`).
///
/// # Errors
///
/// Returns [`SignatureError::InvalidPublicKey`] for a non-canonical or invalid encoding.
pub fn address_of(public_key: &[u8; 33]) -> Result<[u8; 20], SignatureError> {
    let key = EcdsaSecp256k1Sha256::parse_public_key(public_key)
        .map_err(|_| SignatureError::InvalidPublicKey)?;
    Ok(EcdsaSecp256k1Sha256::evm_address(&key))
}

/// The compressed public key of a secret scalar.
///
/// # Errors
///
/// Returns [`SignatureError::InvalidSecret`] unless `1 ≤ secret < N`.
pub fn public_key_of(secret: &[u8; 32]) -> Result<[u8; 33], SignatureError> {
    let key = EcdsaSecp256k1Sha256::parse_private_key(secret)
        .map_err(|_| SignatureError::InvalidSecret)?;
    let encoded = key.public_key().to_sec1_bytes();
    <[u8; 33]>::try_from(&encoded[..]).map_err(|_| SignatureError::InvalidSecret)
}

/// The address of a secret scalar's public key.
///
/// # Errors
///
/// Returns [`SignatureError::InvalidSecret`] unless `1 ≤ secret < N`.
pub fn address_of_secret(secret: &[u8; 32]) -> Result<[u8; 20], SignatureError> {
    let key = EcdsaSecp256k1Sha256::parse_private_key(secret)
        .map_err(|_| SignatureError::InvalidSecret)?;
    Ok(EcdsaSecp256k1Sha256::evm_address(&key.public_key()))
}

/// Mask a 32-byte address word to its low 160 bits (TVM may set padding bytes).
#[must_use]
pub fn mask_address_word(word: &[u8; 32]) -> [u8; 20] {
    let mut address = [0_u8; 20];
    address.copy_from_slice(&word[12..]);
    address
}

// ---------------------------------------------------------------------------------------------
// Single signatures
// ---------------------------------------------------------------------------------------------

/// Check the §3.8 form of a signature: `v ∈ {27, 28}`, `1 ≤ r < N`, `1 ≤ s ≤ HALF_N`.
///
/// # Errors
///
/// Returns [`SignatureError::BadRecoveryByte`], [`SignatureError::BadR`] or
/// [`SignatureError::BadS`].
pub fn check_signature_form(signature: &[u8; 65]) -> Result<(), SignatureError> {
    let (r, s) = split_rs(signature);
    if !matches!(signature[64], 27 | 28) {
        return Err(SignatureError::BadRecoveryByte);
    }
    if is_zero(&r) || r >= SECP256K1_N {
        return Err(SignatureError::BadR);
    }
    if is_zero(&s) || s > SECP256K1_HALF_N {
        return Err(SignatureError::BadS);
    }
    Ok(())
}

fn split_rs(signature: &[u8; 65]) -> ([u8; 32], [u8; 32]) {
    let mut r = [0_u8; 32];
    let mut s = [0_u8; 32];
    r.copy_from_slice(&signature[..32]);
    s.copy_from_slice(&signature[32..64]);
    (r, s)
}

/// Recover the signer address of a §3.8 signature over `digest`.
///
/// # Errors
///
/// Returns a form error, [`SignatureError::RecoveryFailed`] or [`SignatureError::ZeroAddress`].
pub fn recover_address(digest: &[u8; 32], signature: &[u8; 65]) -> Result<[u8; 20], SignatureError> {
    check_signature_form(signature)?;
    let key = EcdsaSecp256k1Sha256::recover_public_key_from_prehash(digest, signature)
        .map_err(|_| SignatureError::RecoveryFailed)?;
    let address = EcdsaSecp256k1Sha256::evm_address(&key);
    if address == [0; 20] {
        return Err(SignatureError::ZeroAddress);
    }
    Ok(address)
}

/// Verify one signature against an expected nonzero address.
///
/// # Errors
///
/// Returns [`SignatureError::ZeroMember`] for a zero expected address, any
/// [`recover_address`] error, or [`SignatureError::WrongSigner`].
pub fn verify_signature(
    digest: &[u8; 32],
    signature: &[u8; 65],
    expected: &[u8; 20],
) -> Result<(), SignatureError> {
    if *expected == [0; 20] {
        return Err(SignatureError::ZeroMember);
    }
    if recover_address(digest, signature)? == *expected {
        Ok(())
    } else {
        Err(SignatureError::WrongSigner)
    }
}

/// Normalize a signature to low-S: if `s > HALF_N`, replace it with `N − s` and flip the
/// y-parity bit of `v`. Other components are returned unchanged.
#[must_use]
pub fn normalize_low_s(signature: &[u8; 65]) -> [u8; 65] {
    let (_, s) = split_rs(signature);
    if s <= SECP256K1_HALF_N || s >= SECP256K1_N {
        return *signature;
    }
    let mut out = *signature;
    out[32..64].copy_from_slice(&be_sub(&SECP256K1_N, &s));
    out[64] = match signature[64] {
        27 => 28,
        28 => 27,
        other => other ^ 1,
    };
    out
}

// ---------------------------------------------------------------------------------------------
// Signature sets
// ---------------------------------------------------------------------------------------------

/// A §3.8 signature set (`SignaturesV1` of §5.2.2).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct SignatureSetV1 {
    /// Bit `i` set iff roster member `i` signed.
    pub signer_bitmap: u32,
    /// One 65-byte signature per set bit, in ascending bit order.
    pub signatures: Vec<u8>,
}

impl SignatureSetV1 {
    /// Assemble a set from `(member index, signature)` pairs for a roster of `n` members.
    ///
    /// # Errors
    ///
    /// Returns [`SignatureError::BadSignerIndex`] for a repeated index or one `≥ n`, and
    /// [`SignatureError::RosterTooLarge`] for `n > 32`.
    pub fn from_signers(
        n: usize,
        signers: &[(usize, [u8; 65])],
    ) -> Result<Self, SignatureError> {
        if n > 32 {
            return Err(SignatureError::RosterTooLarge);
        }
        let mut sorted = signers.to_vec();
        sorted.sort_by_key(|(index, _)| *index);
        let mut bitmap = 0_u32;
        let mut signatures = Vec::with_capacity(sorted.len() * SIGNATURE_BYTES);
        for (index, signature) in &sorted {
            if *index >= n || bitmap & (1 << index) != 0 {
                return Err(SignatureError::BadSignerIndex);
            }
            bitmap |= 1 << index;
            signatures.extend_from_slice(signature);
        }
        Ok(Self {
            signer_bitmap: bitmap,
            signatures,
        })
    }

    /// Number of set bits.
    #[must_use]
    pub fn popcount(&self) -> u32 {
        self.signer_bitmap.count_ones()
    }

    /// Verify the set against the roster `members` and return its popcount.
    ///
    /// # Errors
    ///
    /// See [`verify_signature_set`].
    pub fn verify(&self, digest: &[u8; 32], members: &[[u8; 20]]) -> Result<u32, SignatureError> {
        verify_signature_set(digest, members, self.signer_bitmap, &self.signatures)
    }

    /// Verify the set and require at least `threshold` signatures.
    ///
    /// # Errors
    ///
    /// Returns [`SignatureError::TooFewSignatures`] or any [`verify_signature_set`] error.
    pub fn verify_quorum(
        &self,
        digest: &[u8; 32],
        members: &[[u8; 20]],
        threshold: usize,
    ) -> Result<u32, SignatureError> {
        let count = self.verify(digest, members)?;
        if (count as usize) < threshold {
            return Err(SignatureError::TooFewSignatures);
        }
        Ok(count)
    }
}

/// Verify a signature set: bits `≥ n` zero, exactly 65 bytes per set bit, and each set bit's
/// member nonzero and equal to the address recovered from its signature. Returns the popcount.
///
/// # Errors
///
/// Returns the first violated [`SignatureError`].
pub fn verify_signature_set(
    digest: &[u8; 32],
    members: &[[u8; 20]],
    signer_bitmap: u32,
    signatures: &[u8],
) -> Result<u32, SignatureError> {
    let n = members.len();
    if n > 32 {
        return Err(SignatureError::RosterTooLarge);
    }
    if n < 32 && signer_bitmap >> n != 0 {
        return Err(SignatureError::BitmapOutOfRange);
    }
    let count = signer_bitmap.count_ones();
    if signatures.len() != count as usize * SIGNATURE_BYTES {
        return Err(SignatureError::BadLength);
    }
    let mut chunks = signatures.chunks_exact(SIGNATURE_BYTES);
    for (index, member) in members.iter().enumerate() {
        if signer_bitmap & (1 << index) == 0 {
            continue;
        }
        let chunk = chunks.next().ok_or(SignatureError::BadLength)?;
        let signature = <[u8; 65]>::try_from(chunk).map_err(|_| SignatureError::BadLength)?;
        verify_signature(digest, &signature, member)?;
    }
    Ok(count)
}

// ---------------------------------------------------------------------------------------------
// Signing
// ---------------------------------------------------------------------------------------------

/// Sign `digest` with `secret` (§3.8): RFC 6979 deterministic first, and while the recovery id
/// is 2 or 3, re-sign with fresh RFC 6979 extra entropy from the OS.
///
/// # Errors
///
/// Returns [`SignatureError::InvalidSecret`], [`SignatureError::SigningFailed`],
/// [`SignatureError::EntropyUnavailable`] or [`SignatureError::SigningExhausted`].
pub fn sign_digest(secret: &[u8; 32], digest: &[u8; 32]) -> Result<[u8; 65], SignatureError> {
    sign_digest_with(secret, digest, rfc6979_sign, fresh_entropy)
}

/// [`sign_digest`] with injectable hooks: `attempt(secret, digest, extra_entropy)` produces one
/// candidate signature (`None` entropy on the first attempt), and `entropy()` draws the extra
/// entropy of every later attempt. Candidates are low-S normalized; a candidate whose `v` is
/// not 27 or 28 is discarded. The accepted signature is checked to recover the key's address.
///
/// # Errors
///
/// See [`sign_digest`]; errors of the hooks are propagated.
pub fn sign_digest_with<A, E>(
    secret: &[u8; 32],
    digest: &[u8; 32],
    mut attempt: A,
    mut entropy: E,
) -> Result<[u8; 65], SignatureError>
where
    A: FnMut(&[u8; 32], &[u8; 32], Option<&[u8; 32]>) -> Result<[u8; 65], SignatureError>,
    E: FnMut() -> Result<[u8; 32], SignatureError>,
{
    let address = address_of_secret(secret)?;
    for round in 0..MAX_SIGNING_ATTEMPTS {
        let mut extra = if round == 0 { None } else { Some(entropy()?) };
        let candidate = attempt(secret, digest, extra.as_ref());
        if let Some(extra) = extra.as_mut() {
            wipe(extra);
        }
        let candidate = normalize_low_s(&candidate?);
        if !matches!(candidate[64], 27 | 28) {
            continue;
        }
        if recover_address(digest, &candidate)? != address {
            return Err(SignatureError::SigningFailed);
        }
        return Ok(candidate);
    }
    Err(SignatureError::SigningExhausted)
}

/// One RFC 6979 signing attempt. Without extra entropy this is exactly
/// `iroha_crypto::EcdsaSecp256k1Sha256::sign_prehash_recoverable`; with extra entropy the
/// RFC 6979 §3.6 additional data `k'` is set to it.
///
/// The returned `v` is `27 + recovery_id` and may be 29 or 30.
///
/// # Errors
///
/// Returns [`SignatureError::InvalidSecret`] or [`SignatureError::SigningFailed`].
pub fn rfc6979_sign(
    secret: &[u8; 32],
    digest: &[u8; 32],
    extra_entropy: Option<&[u8; 32]>,
) -> Result<[u8; 65], SignatureError> {
    match extra_entropy {
        None => {
            let key = EcdsaSecp256k1Sha256::parse_private_key(secret)
                .map_err(|_| SignatureError::InvalidSecret)?;
            EcdsaSecp256k1Sha256::sign_prehash_recoverable(digest, &key)
                .map_err(|_| SignatureError::SigningFailed)
        }
        Some(extra) => sign_with_additional_data(secret, digest, extra),
    }
}

/// Draw 32 bytes of fresh OS entropy (a uniformly random nonzero scalar).
///
/// # Errors
///
/// Returns [`SignatureError::EntropyUnavailable`] when the OS RNG fails.
pub fn fresh_entropy() -> Result<[u8; 32], SignatureError> {
    let (_, key) = EcdsaSecp256k1Sha256::try_keypair(KeyGenOption::Random)
        .map_err(|_| SignatureError::EntropyUnavailable)?;
    let bytes = key.to_bytes();
    let mut out = [0_u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

/// HMAC-SHA256 with a 32-byte key.
fn hmac_sha256(key: &[u8; 32], parts: &[&[u8]]) -> [u8; 32] {
    let mut inner_pad = [0x36_u8; 64];
    let mut outer_pad = [0x5c_u8; 64];
    for (index, byte) in key.iter().enumerate() {
        inner_pad[index] ^= byte;
        outer_pad[index] ^= byte;
    }
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    for part in parts {
        inner.update(part);
    }
    let inner = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner);
    wipe(&mut inner_pad);
    wipe(&mut outer_pad);
    outer.finalize().into()
}

/// RFC 6979 HMAC-DRBG over SHA-256, seeded like the `rfc6979` crate that k256 uses:
/// entropy input `x` (the secret), nonce `h` (the unreduced 32-byte prehash) and additional
/// data `k'`.
struct HmacDrbg {
    k: [u8; 32],
    v: [u8; 32],
}

impl HmacDrbg {
    fn new(secret: &[u8; 32], digest: &[u8; 32], additional: &[u8]) -> Self {
        let mut drbg = Self {
            k: [0; 32],
            v: [1; 32],
        };
        for round in 0_u8..=1 {
            drbg.k = hmac_sha256(&drbg.k, &[&drbg.v, &[round], secret, digest, additional]);
            drbg.v = hmac_sha256(&drbg.k, &[&drbg.v]);
        }
        drbg
    }

    fn generate(&mut self) -> [u8; 32] {
        self.v = hmac_sha256(&self.k, &[&self.v]);
        let out = self.v;
        self.k = hmac_sha256(&self.k, &[&self.v, &[0]]);
        self.v = hmac_sha256(&self.k, &[&self.v]);
        out
    }
}

impl Drop for HmacDrbg {
    fn drop(&mut self) {
        wipe(&mut self.k);
        wipe(&mut self.v);
    }
}

/// ECDSA over secp256k1 with an RFC 6979 nonce that includes `additional` data (§3.6 of the
/// RFC), producing the same `r ‖ s ‖ 27 + recovery_id` form (low-S) as
/// `EcdsaSecp256k1Sha256::sign_prehash_recoverable`, which it equals for empty additional data.
fn sign_with_additional_data(
    secret: &[u8; 32],
    digest: &[u8; 32],
    additional: &[u8],
) -> Result<[u8; 65], SignatureError> {
    let signing_key = EcdsaSecp256k1Sha256::parse_private_key(secret)
        .map_err(|_| SignatureError::InvalidSecret)?;
    let d = *signing_key.to_nonzero_scalar();
    // z = digest mod N (at most one subtraction since digest < 2^256 < 2N).
    let z = if *digest >= SECP256K1_N {
        be_sub(digest, &SECP256K1_N)
    } else {
        *digest
    };
    let mut drbg = HmacDrbg::new(secret, digest, additional);
    for _ in 0..64 {
        let mut k_bytes = drbg.generate();
        if is_zero(&k_bytes) || k_bytes >= SECP256K1_N {
            continue;
        }
        let nonce_key = EcdsaSecp256k1Sha256::parse_private_key(&k_bytes)
            .map_err(|_| SignatureError::SigningFailed)?;
        wipe(&mut k_bytes);
        // R = k·G in compressed SEC1 form: parity byte, then x.
        let point = nonce_key.public_key().to_sec1_bytes();
        let y_odd = point[0] == 0x03;
        let mut x = [0_u8; 32];
        x.copy_from_slice(&point[1..33]);
        let x_reduced = x >= SECP256K1_N;
        let r = if x_reduced { be_sub(&x, &SECP256K1_N) } else { x };
        if is_zero(&r) {
            continue;
        }
        let r_scalar = *EcdsaSecp256k1Sha256::parse_private_key(&r)
            .map_err(|_| SignatureError::SigningFailed)?
            .to_nonzero_scalar();
        let mut sum = r_scalar * d;
        if !is_zero(&z) {
            sum = sum
                + *EcdsaSecp256k1Sha256::parse_private_key(&z)
                    .map_err(|_| SignatureError::SigningFailed)?
                    .to_nonzero_scalar();
        }
        let k_inverse = nonce_key.to_nonzero_scalar().invert();
        if !bool::from(k_inverse.is_some()) {
            continue;
        }
        let s_scalar = k_inverse.unwrap() * sum;
        let mut s = [0_u8; 32];
        s.copy_from_slice(&s_scalar.to_bytes());
        if is_zero(&s) {
            continue;
        }
        let s_high = s > SECP256K1_HALF_N;
        if s_high {
            s = be_sub(&SECP256K1_N, &s);
        }
        let recovery_id = u8::from(y_odd ^ s_high) | (u8::from(x_reduced) << 1);
        let mut out = [0_u8; 65];
        out[..32].copy_from_slice(&r);
        out[32..64].copy_from_slice(&s);
        out[64] = 27 + recovery_id;
        return Ok(out);
    }
    Err(SignatureError::SigningFailed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::hashes::keccak256;

    fn secret(seed: u8) -> [u8; 32] {
        keccak256(&[b"sccp-test-key", &[seed]])
    }

    fn digest(seed: u8) -> [u8; 32] {
        keccak256(&[b"sccp-test-digest", &[seed]])
    }

    #[test]
    fn deterministic_signature_recovers_the_signer() {
        let key = secret(1);
        let message = digest(1);
        let signature = sign_digest(&key, &message).unwrap();
        assert!(matches!(signature[64], 27 | 28));
        check_signature_form(&signature).unwrap();
        let address = address_of_secret(&key).unwrap();
        assert_eq!(recover_address(&message, &signature), Ok(address));
        assert_eq!(address_of(&public_key_of(&key).unwrap()), Ok(address));
        assert_eq!(sign_digest(&key, &message).unwrap(), signature, "RFC 6979");
        verify_signature(&message, &signature, &address).unwrap();
        assert_eq!(
            verify_signature(&message, &signature, &[1; 20]),
            Err(SignatureError::WrongSigner)
        );
        assert_eq!(
            verify_signature(&message, &signature, &[0; 20]),
            Err(SignatureError::ZeroMember)
        );
    }

    #[test]
    fn additional_data_signing_matches_the_backend_without_entropy() {
        for seed in 0..16 {
            let key = secret(seed);
            let message = digest(seed);
            let backend = rfc6979_sign(&key, &message, None).unwrap();
            let own = sign_with_additional_data(&key, &message, &[]).unwrap();
            assert_eq!(own, backend, "seed {seed}");
        }
        // A digest above N is reduced for s but used unreduced as the RFC 6979 nonce input.
        let key = secret(99);
        let high = [0xff; 32];
        assert_eq!(
            sign_with_additional_data(&key, &high, &[]).unwrap(),
            rfc6979_sign(&key, &high, None).unwrap()
        );
    }

    #[test]
    fn extra_entropy_changes_the_nonce_and_still_verifies() {
        let key = secret(2);
        let message = digest(2);
        let address = address_of_secret(&key).unwrap();
        let plain = rfc6979_sign(&key, &message, None).unwrap();
        let salted = rfc6979_sign(&key, &message, Some(&[7; 32])).unwrap();
        assert_ne!(plain[..32], salted[..32]);
        let salted = normalize_low_s(&salted);
        if matches!(salted[64], 27 | 28) {
            assert_eq!(recover_address(&message, &salted), Ok(address));
        }
        assert_eq!(
            rfc6979_sign(&key, &message, Some(&[7; 32])).unwrap(),
            rfc6979_sign(&key, &message, Some(&[7; 32])).unwrap()
        );
    }

    #[test]
    fn recovery_id_two_forces_a_resign_with_fresh_entropy() {
        let key = secret(3);
        let message = digest(3);
        let address = address_of_secret(&key).unwrap();
        let mut calls = Vec::new();
        let mut entropy_draws = 0;
        let signature = sign_digest_with(
            &key,
            &message,
            |secret, digest, extra| {
                calls.push(extra.copied());
                let mut signature = rfc6979_sign(secret, digest, extra)?;
                if extra.is_none() {
                    // Simulate R.x >= N: recovery id 2 (v = 29).
                    signature[64] = 29;
                }
                Ok(signature)
            },
            || {
                entropy_draws += 1;
                Ok([0x5a; 32])
            },
        )
        .unwrap();
        assert_eq!(calls, vec![None, Some([0x5a; 32])]);
        assert_eq!(entropy_draws, 1);
        assert!(matches!(signature[64], 27 | 28));
        assert_eq!(recover_address(&message, &signature), Ok(address));
        assert_ne!(signature, rfc6979_sign(&key, &message, None).unwrap());
    }

    #[test]
    fn signing_gives_up_after_bounded_attempts() {
        let key = secret(4);
        let result = sign_digest_with(
            &key,
            &digest(4),
            |_, _, _| {
                let mut out = [1_u8; 65];
                out[64] = 30;
                Ok(out)
            },
            || Ok([1; 32]),
        );
        assert_eq!(result, Err(SignatureError::SigningExhausted));
        assert_eq!(
            sign_digest(&[0; 32], &digest(4)),
            Err(SignatureError::InvalidSecret)
        );
        assert_eq!(
            sign_digest(&SECP256K1_N, &digest(4)),
            Err(SignatureError::InvalidSecret)
        );
        // A hook that returns someone else's signature is caught.
        let other = sign_digest(&secret(5), &digest(4)).unwrap();
        assert_eq!(
            sign_digest_with(&key, &digest(4), |_, _, _| Ok(other), || Ok([1; 32])),
            Err(SignatureError::SigningFailed)
        );
    }

    #[test]
    fn fresh_entropy_is_random_and_nonzero() {
        let a = fresh_entropy().unwrap();
        let b = fresh_entropy().unwrap();
        assert_ne!(a, b);
        assert!(!is_zero(&a));
    }

    #[test]
    fn form_checks() {
        let key = secret(6);
        let message = digest(6);
        let good = sign_digest(&key, &message).unwrap();
        let mut bad_v = good;
        for v in [0_u8, 1, 26, 29, 30, 255] {
            bad_v[64] = v;
            assert_eq!(check_signature_form(&bad_v), Err(SignatureError::BadRecoveryByte));
        }
        let mut zero_r = good;
        zero_r[..32].copy_from_slice(&[0; 32]);
        assert_eq!(check_signature_form(&zero_r), Err(SignatureError::BadR));
        let mut big_r = good;
        big_r[..32].copy_from_slice(&SECP256K1_N);
        assert_eq!(check_signature_form(&big_r), Err(SignatureError::BadR));
        let mut zero_s = good;
        zero_s[32..64].copy_from_slice(&[0; 32]);
        assert_eq!(check_signature_form(&zero_s), Err(SignatureError::BadS));
        // The high-S twin of a valid signature is rejected, and normalizes back.
        let (_, s) = split_rs(&good);
        let mut high = good;
        high[32..64].copy_from_slice(&be_sub(&SECP256K1_N, &s));
        high[64] = if good[64] == 27 { 28 } else { 27 };
        assert_eq!(check_signature_form(&high), Err(SignatureError::BadS));
        assert_eq!(recover_address(&message, &high), Err(SignatureError::BadS));
        assert_eq!(normalize_low_s(&high), good);
        assert_eq!(normalize_low_s(&good), good);
        // HALF_N itself is admitted as s.
        let mut half = good;
        half[32..64].copy_from_slice(&SECP256K1_HALF_N);
        assert!(check_signature_form(&half).is_ok());
    }

    #[test]
    fn signature_sets() {
        let message = digest(7);
        let keys: Vec<[u8; 32]> = (10..14).map(secret).collect();
        let mut members: Vec<[u8; 20]> =
            keys.iter().map(|key| address_of_secret(key).unwrap()).collect();
        members.sort_unstable();
        members[0] = [0; 20];
        let key_of = |address: &[u8; 20]| {
            keys.iter()
                .find(|key| address_of_secret(key).unwrap() == *address)
                .copied()
                .unwrap()
        };
        let signers: Vec<(usize, [u8; 65])> = [3_usize, 1, 2]
            .iter()
            .map(|index| (*index, sign_digest(&key_of(&members[*index]), &message).unwrap()))
            .collect();
        let set = SignatureSetV1::from_signers(4, &signers).unwrap();
        assert_eq!(set.signer_bitmap, 0b1110);
        assert_eq!(set.signatures.len(), 3 * 65);
        assert_eq!(&set.signatures[..65], &signers[1].1[..]);
        assert_eq!(set.popcount(), 3);
        assert_eq!(set.verify(&message, &members), Ok(3));
        assert_eq!(set.verify_quorum(&message, &members, 3), Ok(3));
        assert_eq!(
            set.verify_quorum(&message, &members, 4),
            Err(SignatureError::TooFewSignatures)
        );
        // Bit >= n.
        assert_eq!(
            verify_signature_set(&message, &members, set.signer_bitmap | 0b1_0000, &set.signatures),
            Err(SignatureError::BitmapOutOfRange)
        );
        // Wrong length.
        assert_eq!(
            verify_signature_set(&message, &members, set.signer_bitmap, &set.signatures[..130]),
            Err(SignatureError::BadLength)
        );
        // A bit addressing the zero member.
        let mut padded = signers[1].1.to_vec();
        padded.extend_from_slice(&set.signatures);
        assert_eq!(
            verify_signature_set(&message, &members, 0b1111, &padded),
            Err(SignatureError::ZeroMember)
        );
        // Swapped order fails.
        let mut swapped = set.signatures.clone();
        swapped[..65].copy_from_slice(&set.signatures[65..130]);
        swapped[65..130].copy_from_slice(&set.signatures[..65]);
        assert_eq!(
            verify_signature_set(&message, &members, set.signer_bitmap, &swapped),
            Err(SignatureError::WrongSigner)
        );
        assert_eq!(
            SignatureSetV1::from_signers(4, &[(1, [0; 65]), (1, [0; 65])]),
            Err(SignatureError::BadSignerIndex)
        );
        assert_eq!(
            SignatureSetV1::from_signers(4, &[(4, [0; 65])]),
            Err(SignatureError::BadSignerIndex)
        );
        assert_eq!(
            verify_signature_set(&message, &[[1; 20]; 33], 0, &[]),
            Err(SignatureError::RosterTooLarge)
        );
    }

    #[test]
    fn address_helpers() {
        let mut word = [0x41_u8; 32];
        word[12..].copy_from_slice(&[0x22; 20]);
        assert_eq!(mask_address_word(&word), [0x22; 20]);
        assert_eq!(address_of(&[0; 33]), Err(SignatureError::InvalidPublicKey));
        assert_eq!(public_key_of(&[0; 32]), Err(SignatureError::InvalidSecret));
        // Known vector: secret 1 is the generator; its address is well known.
        let mut one = [0_u8; 32];
        one[31] = 1;
        let hex: String = address_of_secret(&one)
            .unwrap()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(hex, "7e5f4552091a69125d5dfcb7b8c2659029395bdf");
    }

    #[test]
    fn big_endian_subtraction_and_wipe() {
        let mut one = [0_u8; 32];
        one[31] = 1;
        let mut expected = SECP256K1_N;
        expected[31] -= 1;
        assert_eq!(be_sub(&SECP256K1_N, &one), expected);
        let mut borrow = [0_u8; 32];
        borrow[30] = 1;
        let mut result = [0_u8; 32];
        result[31] = 0xff;
        assert_eq!(be_sub(&borrow, &one), result);
        let mut secret = [7_u8; 32];
        wipe(&mut secret);
        assert_eq!(secret, [0; 32]);
    }
}
