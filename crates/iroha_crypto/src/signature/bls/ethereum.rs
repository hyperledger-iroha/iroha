//! Ethereum consensus BLS12-381 proof-of-possession verification.
//!
//! Ethereum uses the IETF BLS min-pk proof-of-possession ciphersuite directly. This is deliberately
//! separate from Iroha's contextual BLS signatures: the message is hashed to G2 with Ethereum's
//! standard ciphersuite DST and is not wrapped in an Iroha message context.
use crate::Error;
use blstrs::{G1Affine, G1Projective, G2Affine, G2Projective, pairing};
use group::{Curve as _, Group as _, prime::PrimeCurveAffine as _};
/// IETF BLS min-pk proof-of-possession signature ciphersuite DST used by Ethereum consensus.
pub const ETHEREUM_BLS_POP_DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_";
/// Validate one compressed Ethereum BLS min-pk public key.
///
/// Validation checks the canonical 48-byte encoding, curve membership,
/// subgroup membership, and rejects the identity point as required by
/// `KeyValidate` in the BLS proof-of-possession ciphersuite.
///
/// # Errors
///
/// Returns [`Error::BadSignature`] when the compressed key is non-canonical,
/// is not a valid subgroup point, or encodes the identity.
pub fn ethereum_bls_pop_validate_public_key(public_key: &[u8; 48]) -> Result<(), Error> {
    parse_public_key(public_key).map(|_| ())
}
/// Verify an Ethereum BLS fast aggregate signature over one message.
///
/// `public_keys` contains one entry for every participating sync-committee position. Repeated keys
/// are intentionally retained: Ethereum sync committees are sampled with replacement, so one
/// validator can occupy multiple positions and contributes once per set participation bit.
///
/// This function implements the standard min-pk proof-of-possession `FastAggregateVerify`
/// operation. It rejects an empty participant set, malformed/non-canonical points, subgroup-invalid
/// points, identity inputs, and public-key aggregates that cancel to the identity.
///
/// # Errors
///
/// Returns [`Error::BadSignature`] when there are no participants, any key or
/// the signature is invalid, the aggregate key is the identity, or the pairing
/// equation does not verify for `message`.
pub fn ethereum_bls_pop_fast_aggregate_verify(
    public_keys: &[[u8; 48]],
    message: &[u8],
    signature: &[u8; 96],
) -> Result<(), Error> {
    if public_keys.is_empty() {
        return Err(Error::BadSignature);
    }
    let mut aggregate_public_key = G1Projective::identity();
    for public_key in public_keys {
        aggregate_public_key += parse_public_key(public_key)?;
    }
    if bool::from(aggregate_public_key.is_identity()) {
        return Err(Error::BadSignature);
    }
    let signature = parse_signature(signature)?;
    let message_point = G2Projective::hash_to_curve(message, ETHEREUM_BLS_POP_DST, &[]).to_affine();
    let aggregate_public_key = aggregate_public_key.to_affine();
    let generator = G1Affine::generator();
    if pairing(&aggregate_public_key, &message_point) == pairing(&generator, &signature) {
        Ok(())
    } else {
        Err(Error::BadSignature)
    }
}
fn parse_public_key(bytes: &[u8; 48]) -> Result<G1Affine, Error> {
    let Some(point) = G1Affine::from_compressed(bytes).into_option() else {
        return Err(Error::BadSignature);
    };
    if bool::from(point.is_identity()) || point.to_compressed() != *bytes {
        return Err(Error::BadSignature);
    }
    Ok(point)
}
fn parse_signature(bytes: &[u8; 96]) -> Result<G2Affine, Error> {
    let Some(point) = G2Affine::from_compressed(bytes).into_option() else {
        return Err(Error::BadSignature);
    };
    if bool::from(point.is_identity()) || point.to_compressed() != *bytes {
        return Err(Error::BadSignature);
    }
    Ok(point)
}
#[cfg(test)]
mod tests {
    use super::*;
    use blstrs::{G1Projective, G2Projective, Scalar};
    fn key_and_signature(secret: u64, message: &[u8]) -> ([u8; 48], G2Projective) {
        let scalar = Scalar::from(secret);
        let public_key = (G1Projective::generator() * scalar)
            .to_affine()
            .to_compressed();
        let signature = G2Projective::hash_to_curve(message, ETHEREUM_BLS_POP_DST, &[]) * scalar;
        (public_key, signature)
    }
    #[test]
    fn verifies_standard_pop_fast_aggregate() {
        let message = b"ethereum-consensus-signing-root";
        let (public_key_a, signature_a) = key_and_signature(7, message);
        let (public_key_b, signature_b) = key_and_signature(11, message);
        let signature = (signature_a + signature_b).to_affine().to_compressed();
        ethereum_bls_pop_fast_aggregate_verify(&[public_key_a, public_key_b], message, &signature)
            .expect("valid standard-DST aggregate");
    }
    #[test]
    fn repeated_committee_positions_are_not_deduplicated() {
        let message = b"duplicate sync committee position";
        let (public_key, signature) = key_and_signature(19, message);
        let repeated_signature = (signature + signature).to_affine().to_compressed();
        ethereum_bls_pop_fast_aggregate_verify(
            &[public_key, public_key],
            message,
            &repeated_signature,
        )
        .expect("the same validator may occupy two committee positions");
        assert!(
            ethereum_bls_pop_fast_aggregate_verify(&[public_key], message, &repeated_signature)
                .is_err()
        );
    }
    #[test]
    fn rejects_empty_malformed_identity_and_wrong_message_inputs() {
        let message = b"message";
        let (public_key, signature) = key_and_signature(23, message);
        let signature = signature.to_affine().to_compressed();
        assert!(ethereum_bls_pop_fast_aggregate_verify(&[], message, &signature).is_err());
        assert!(
            ethereum_bls_pop_fast_aggregate_verify(&[public_key], b"other", &signature).is_err()
        );
        let identity_public_key = G1Projective::identity().to_affine().to_compressed();
        assert!(ethereum_bls_pop_validate_public_key(&identity_public_key).is_err());
        assert!(
            ethereum_bls_pop_fast_aggregate_verify(&[identity_public_key], message, &signature)
                .is_err()
        );
        let identity_signature = G2Projective::identity().to_affine().to_compressed();
        assert!(
            ethereum_bls_pop_fast_aggregate_verify(&[public_key], message, &identity_signature)
                .is_err()
        );
        let malformed_public_key = [0xff; 48];
        let malformed_signature = [0xff; 96];
        assert!(ethereum_bls_pop_validate_public_key(&malformed_public_key).is_err());
        assert!(
            ethereum_bls_pop_fast_aggregate_verify(&[public_key], message, &malformed_signature)
                .is_err()
        );
    }
    #[test]
    fn rejects_cancelling_keys_and_non_standard_dst() {
        let message = b"domain separation is consensus critical";
        let scalar = Scalar::from(29_u64);
        let public_key = G1Projective::generator() * scalar;
        let public_key_bytes = public_key.to_affine().to_compressed();
        let negated_public_key_bytes = (-public_key).to_affine().to_compressed();
        let valid_signature = (G2Projective::hash_to_curve(message, ETHEREUM_BLS_POP_DST, &[])
            * scalar)
            .to_affine()
            .to_compressed();
        assert!(
            ethereum_bls_pop_fast_aggregate_verify(
                &[public_key_bytes, negated_public_key_bytes],
                message,
                &valid_signature,
            )
            .is_err()
        );
        let wrong_dst_signature = (G2Projective::hash_to_curve(
            message,
            b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_",
            &[],
        ) * scalar)
            .to_affine()
            .to_compressed();
        assert!(
            ethereum_bls_pop_fast_aggregate_verify(
                &[public_key_bytes],
                message,
                &wrong_dst_signature,
            )
            .is_err()
        );
    }
}
