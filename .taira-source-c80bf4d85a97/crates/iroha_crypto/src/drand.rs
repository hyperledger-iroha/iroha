//! Verification of drand BLS12-381 randomness beacons.
//!
//! The first-release SoraFS PoR coordinator consumes the production drand
//! `bls-unchained-g1-rfc9380` scheme. That scheme signs the SHA-256 digest of
//! the eight-byte big-endian round number with a signature in G1 and a public
//! key in G2. The published randomness is SHA-256 of the canonical compressed
//! signature. This module deliberately performs no network I/O and accepts no
//! remotely supplied chain metadata; callers must pin the chain identity and
//! public key in configuration before invoking the verifier.

use blstrs::{G1Affine, G1Projective, G2Affine, G2Prepared};
use group::{Curve as _, Group as _, prime::PrimeCurveAffine as _};
use pairing::{MillerLoopResult as _, MultiMillerLoop as _};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

/// drand scheme supported by the first-release verifier.
pub const UNCHAINED_G1_RFC9380_SCHEME: &str = "bls-unchained-g1-rfc9380";

/// RFC 9380 BLS signature domain used by drand's G1 signature scheme.
const DRAND_G1_SIGNATURE_DST: &[u8] = b"BLS_SIG_BLS12381G1_XMD:SHA-256_SSWU_RO_NUL_";

/// Length of a canonical compressed drand G2 public key.
pub const DRAND_PUBLIC_KEY_BYTES: usize = 96;
/// Length of a canonical compressed drand G1 signature.
pub const DRAND_SIGNATURE_BYTES: usize = 48;

/// Errors returned while verifying a drand beacon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DrandVerificationError {
    /// Round zero is not a valid published beacon round.
    #[error("drand beacon round must be non-zero")]
    ZeroRound,
    /// Public-key bytes do not encode one canonical, non-identity G2 point.
    #[error("drand public key must be a canonical non-identity G2 point")]
    InvalidPublicKey,
    /// Signature bytes do not encode one canonical, non-identity G1 point.
    #[error("drand signature must be a canonical non-identity G1 point")]
    InvalidSignature,
    /// The signature does not verify for the pinned public key and round.
    #[error("drand signature verification failed")]
    SignatureMismatch,
    /// The advertised randomness is not SHA-256 of the verified signature.
    #[error("drand advertised randomness does not match the verified signature")]
    RandomnessMismatch,
}

/// Return whether bytes encode a canonical, non-identity drand G2 public key.
#[must_use]
pub fn is_valid_unchained_g1_rfc9380_public_key(public_key: &[u8]) -> bool {
    decode_public_key(public_key).is_ok()
}

/// Verify one `bls-unchained-g1-rfc9380` beacon against a pinned public key.
///
/// The returned bytes are the canonical randomness derived as SHA-256 of the
/// verified compressed signature. When `advertised_randomness` is supplied,
/// it must be exactly 32 bytes and equal the derived value.
///
/// # Errors
///
/// Returns [`DrandVerificationError`] for a zero round, malformed or
/// non-canonical curve encodings, an invalid pairing, or mismatched advertised
/// randomness.
pub fn verify_unchained_g1_rfc9380(
    public_key: &[u8],
    round: u64,
    signature: &[u8],
    advertised_randomness: Option<&[u8]>,
) -> Result<[u8; 32], DrandVerificationError> {
    if round == 0 {
        return Err(DrandVerificationError::ZeroRound);
    }
    let public_key = decode_public_key(public_key)?;
    let signature_point = decode_signature(signature)?;

    // drand hashes the fixed-width big-endian round before applying the BLS
    // hash-to-curve operation for the unchained scheme.
    let message_digest = Sha256::digest(round.to_be_bytes());
    let message_point =
        G1Projective::hash_to_curve(&message_digest, DRAND_G1_SIGNATURE_DST, &[]).to_affine();

    // e(signature, G2 generator) == e(H(message), public key).
    let terms: [(&G1Affine, &G2Prepared); 2] = [
        (&signature_point, &G2Prepared::from(G2Affine::generator())),
        (
            &(-G1Projective::from(message_point)).to_affine(),
            &G2Prepared::from(public_key),
        ),
    ];
    let pairing = blstrs::Bls12::multi_miller_loop(&terms).final_exponentiation();
    if !bool::from(pairing.is_identity()) {
        return Err(DrandVerificationError::SignatureMismatch);
    }

    let randomness: [u8; 32] = Sha256::digest(signature).into();
    if let Some(advertised) = advertised_randomness
        && advertised != &randomness[..]
    {
        return Err(DrandVerificationError::RandomnessMismatch);
    }
    Ok(randomness)
}

fn decode_public_key(bytes: &[u8]) -> Result<G2Affine, DrandVerificationError> {
    let encoded: [u8; DRAND_PUBLIC_KEY_BYTES] = bytes
        .try_into()
        .map_err(|_| DrandVerificationError::InvalidPublicKey)?;
    let point = G2Affine::from_compressed(&encoded)
        .into_option()
        .ok_or(DrandVerificationError::InvalidPublicKey)?;
    if bool::from(point.is_identity()) || point.to_compressed() != encoded {
        return Err(DrandVerificationError::InvalidPublicKey);
    }
    Ok(point)
}

fn decode_signature(bytes: &[u8]) -> Result<G1Affine, DrandVerificationError> {
    let encoded: [u8; DRAND_SIGNATURE_BYTES] = bytes
        .try_into()
        .map_err(|_| DrandVerificationError::InvalidSignature)?;
    let point = G1Affine::from_compressed(&encoded)
        .into_option()
        .ok_or(DrandVerificationError::InvalidSignature)?;
    if bool::from(point.is_identity()) || point.to_compressed() != encoded {
        return Err(DrandVerificationError::InvalidSignature);
    }
    Ok(point)
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;

    use super::{DrandVerificationError, verify_unchained_g1_rfc9380};

    // Official Quicknet public key and round 1000 beacon. The vector is fixed
    // so verification tests never depend on network access or wall-clock time.
    const QUICKNET_PUBLIC_KEY: [u8; 96] = hex!(
        "83cf0f2896adee7eb8b5f01fcad3912212c437e0073e911fb90022d3e760183c"
        "8c4b450b6a0a6c3ac6a5776a2d1064510d1fec758c921cc22b0e17e63aaf4bcb"
        "5ed66304de9cf809bd274ca73bab4af5a6e9c76a4bc09e76eae8991ef5ece45a"
    );
    const QUICKNET_ROUND_1000_SIGNATURE: [u8; 48] = hex!(
        "b44679b9a59af2ec876b1a6b1ad52ea9b1615fc3982b19576350f93447cb1125"
        "e342b73a8dd2bacbe47e4b6b63ed5e39"
    );
    const QUICKNET_ROUND_1000_RANDOMNESS: [u8; 32] =
        hex!("fe290beca10872ef2fb164d2aa4442de4566183ec51c56ff3cd603d930e54fdd");

    #[test]
    fn official_quicknet_round_verifies() {
        assert_eq!(
            verify_unchained_g1_rfc9380(
                &QUICKNET_PUBLIC_KEY,
                1_000,
                &QUICKNET_ROUND_1000_SIGNATURE,
                Some(&QUICKNET_ROUND_1000_RANDOMNESS),
            ),
            Ok(QUICKNET_ROUND_1000_RANDOMNESS)
        );
    }

    #[test]
    fn wrong_round_and_randomness_fail_closed() {
        assert_eq!(
            verify_unchained_g1_rfc9380(
                &QUICKNET_PUBLIC_KEY,
                999,
                &QUICKNET_ROUND_1000_SIGNATURE,
                None,
            ),
            Err(DrandVerificationError::SignatureMismatch)
        );
        let mut forged_randomness = QUICKNET_ROUND_1000_RANDOMNESS;
        forged_randomness[0] ^= 1;
        assert_eq!(
            verify_unchained_g1_rfc9380(
                &QUICKNET_PUBLIC_KEY,
                1_000,
                &QUICKNET_ROUND_1000_SIGNATURE,
                Some(&forged_randomness),
            ),
            Err(DrandVerificationError::RandomnessMismatch)
        );
        assert_eq!(
            verify_unchained_g1_rfc9380(
                &QUICKNET_PUBLIC_KEY,
                1_000,
                &QUICKNET_ROUND_1000_SIGNATURE,
                Some(&QUICKNET_ROUND_1000_RANDOMNESS[..31]),
            ),
            Err(DrandVerificationError::RandomnessMismatch)
        );
    }

    #[test]
    fn malformed_key_and_signature_encodings_are_rejected() {
        assert_eq!(
            verify_unchained_g1_rfc9380(
                &QUICKNET_PUBLIC_KEY[..95],
                1_000,
                &QUICKNET_ROUND_1000_SIGNATURE,
                None,
            ),
            Err(DrandVerificationError::InvalidPublicKey)
        );
        assert_eq!(
            verify_unchained_g1_rfc9380(&[0; 96], 1_000, &QUICKNET_ROUND_1000_SIGNATURE, None,),
            Err(DrandVerificationError::InvalidPublicKey)
        );
        let mut identity_public_key = [0; 96];
        identity_public_key[0] = 0xc0;
        assert_eq!(
            verify_unchained_g1_rfc9380(
                &identity_public_key,
                1_000,
                &QUICKNET_ROUND_1000_SIGNATURE,
                None,
            ),
            Err(DrandVerificationError::InvalidPublicKey)
        );
        assert_eq!(
            verify_unchained_g1_rfc9380(
                &QUICKNET_PUBLIC_KEY,
                1_000,
                &QUICKNET_ROUND_1000_SIGNATURE[..47],
                None,
            ),
            Err(DrandVerificationError::InvalidSignature)
        );
        assert_eq!(
            verify_unchained_g1_rfc9380(&QUICKNET_PUBLIC_KEY, 1_000, &[0; 48], None),
            Err(DrandVerificationError::InvalidSignature)
        );
        let mut identity_signature = [0; 48];
        identity_signature[0] = 0xc0;
        assert_eq!(
            verify_unchained_g1_rfc9380(&QUICKNET_PUBLIC_KEY, 1_000, &identity_signature, None,),
            Err(DrandVerificationError::InvalidSignature)
        );
    }

    #[test]
    fn tampering_and_zero_round_are_rejected() {
        assert_eq!(
            verify_unchained_g1_rfc9380(
                &QUICKNET_PUBLIC_KEY,
                0,
                &QUICKNET_ROUND_1000_SIGNATURE,
                None,
            ),
            Err(DrandVerificationError::ZeroRound)
        );

        let mut tampered_signature = QUICKNET_ROUND_1000_SIGNATURE;
        tampered_signature[17] ^= 1;
        assert!(matches!(
            verify_unchained_g1_rfc9380(&QUICKNET_PUBLIC_KEY, 1_000, &tampered_signature, None,),
            Err(DrandVerificationError::InvalidSignature
                | DrandVerificationError::SignatureMismatch)
        ));

        let mut wrong_public_key = QUICKNET_PUBLIC_KEY;
        wrong_public_key[31] ^= 1;
        assert!(matches!(
            verify_unchained_g1_rfc9380(
                &wrong_public_key,
                1_000,
                &QUICKNET_ROUND_1000_SIGNATURE,
                None,
            ),
            Err(DrandVerificationError::InvalidPublicKey
                | DrandVerificationError::SignatureMismatch)
        ));
    }
}
