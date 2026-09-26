//! Authenticated, fail-closed boundary for eight-party BFV decryption.
//!
//! This module checks the public transcript and eight independent signatures. It handles one
//! registered `R_q` ciphertext, not the full source/RNS-limb bootstrap relation. The signed
//! contribution contains only an opaque commitment: publishing raw `c1*s_i` can reveal `s_i`
//! when `c1` is invertible. No result is released until a privacy-preserving decryption-share
//! construction and its public zero-knowledge verifier are reviewed and implemented.

use super::{
    BfvCiphertext, BfvError, BfvParameters, BfvPublicKey, bfv_ciphertext_digest,
    bfv_public_key_digest, poly_add_mod, validate_poly, validate_registered_bfv_parameters,
};
use crate::{Algorithm, Hash, PublicKey, SignatureOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
use norito::derive::{JsonDeserialize, JsonSerialize};

/// Number of actual, independently authenticated protocol participants.
///
/// The eight registered RNS modulus limbs are a different dimension. The present policy requires
/// all eight participants; it makes no threshold or dropout claim.
pub const BFV_EIGHT_PARTY_DECRYPTION_PARTICIPANTS_V1: usize = 8;
const BFV_EIGHT_PARTY_REGISTERED_COEFFICIENTS_V1: usize = 64;
const BFV_EIGHT_PARTY_STATEMENT_MAX_BYTES_V1: usize = 16 * 1024;
const BFV_EIGHT_PARTY_CONTRIBUTIONS_MAX_BYTES_V1: usize = 8 * 1024;
const BFV_EIGHT_PARTY_DECRYPTION_VERSION_V1: u16 = 1;
const BFV_EIGHT_PARTY_DECRYPTION_STATEMENT_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.crypto.fhe.bfv.eight_party_decryption.statement.v1";

/// One member of a frozen eight-party decryption roster.
///
/// For a common public polynomial `a`, the claimed key share is `b_i = -a*s_i + e_i (mod q)`.
/// The aggregate key's `b` is the sum of these public `b_i` values. This public arithmetic does
/// not prove that a bounded ternary `s_i` and bounded error `e_i` exist.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, norito::NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_crypto::fhe_bfv::BfvEightPartyDecryptionMemberV1")]
pub struct BfvEightPartyDecryptionMemberV1 {
    /// Key authorizing this participant's decryption contribution.
    pub signing_public_key: PublicKey,
    /// Claimed share of the aggregate public key's `b` polynomial.
    pub public_key_share_b: [u64; BFV_EIGHT_PARTY_REGISTERED_COEFFICIENTS_V1],
}

/// Frozen statement for eight-of-eight additive-secret-share BFV decryption.
///
/// The caller must persist a unique session identifier and bind this statement to finalized job
/// authority. The roster is canonically ordered by distinct signing public keys. A transcript
/// cannot authorize an output until every party supplies a verified algebraic share proof.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, norito::NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_crypto::fhe_bfv::BfvEightPartyDecryptionStatementV1")]
pub struct BfvEightPartyDecryptionStatementV1 {
    /// Canonical layout version.
    pub version: u16,
    /// Durable, nonzero, unique identifier for this decryption attempt.
    pub session_id: [u8; 32],
    /// Exact registered parameter profile.
    pub parameters: BfvParameters,
    /// Aggregate public key with `b = sum_i b_i (mod q)`.
    pub aggregate_public_key: BfvPublicKey,
    /// Exact ciphertext being decrypted.
    pub ciphertext: BfvCiphertext,
    /// Exactly eight distinct, canonically ordered participants.
    pub members: [BfvEightPartyDecryptionMemberV1; BFV_EIGHT_PARTY_DECRYPTION_PARTICIPANTS_V1],
}

/// Signed participant declaration of a private BFV decryption contribution.
///
/// The opaque commitment binds a future private/masked share protocol. A signature proves who
/// declared the commitment, not that any BFV relation holds. Raw `c1*s_i` is never a public field.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, norito::NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_crypto::fhe_bfv::BfvEightPartyDecryptionContributionPayloadV1")]
pub struct BfvEightPartyDecryptionContributionPayloadV1 {
    /// Canonical layout version.
    pub version: u16,
    /// Session identifier from the frozen statement.
    pub session_id: [u8; 32],
    /// Digest of the complete frozen statement.
    pub statement_digest: Hash,
    /// Index into the statement's canonical roster.
    pub participant_index: u8,
    /// Opaque commitment to a future private/masked contribution and proof transcript.
    pub contribution_commitment: Hash,
}

/// Authenticated decryption declaration from one roster member.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, norito::NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_crypto::fhe_bfv::BfvEightPartySignedDecryptionContributionV1")]
pub struct BfvEightPartySignedDecryptionContributionV1 {
    /// Exact participant payload signed under its roster key.
    pub payload: BfvEightPartyDecryptionContributionPayloadV1,
    /// Signature authenticating the payload, without certifying its BFV arithmetic.
    pub signature: SignatureOf<BfvEightPartyDecryptionContributionPayloadV1>,
}

fn validate_bfv_eight_party_signing_key_v1(key: &PublicKey) -> Result<(), BfvError> {
    let (algorithm, payload) = key.try_to_bytes().map_err(|err| {
        BfvError::InvalidParameters(format!(
            "BFV eight-party signing public key is malformed: {err}"
        ))
    })?;
    if algorithm != Algorithm::Ed25519 {
        return Err(BfvError::InvalidParameters(
            "BFV eight-party signing public key must use Ed25519".into(),
        ));
    }
    if payload.len() != 32 || payload.iter().all(|byte| *byte == 0) {
        return Err(BfvError::InvalidParameters(
            "BFV eight-party signing public key must have nonzero canonical Ed25519 bytes".into(),
        ));
    }
    let canonical = PublicKey::from_bytes(Algorithm::Ed25519, payload).map_err(|err| {
        BfvError::InvalidParameters(format!(
            "BFV eight-party signing public key is invalid: {err}"
        ))
    })?;
    if &canonical != key {
        return Err(BfvError::InvalidParameters(
            "BFV eight-party signing public key is not canonical".into(),
        ));
    }
    Ok(())
}

/// Validate public roster, key-sum, ciphertext, and registered-parameter bounds.
///
/// # Errors
/// Rejects malformed or unfrozen context, non-Ed25519 or out-of-order roster keys, invalid
/// polynomial shapes, or a claimed aggregate key that does not sum from the roster's public shares.
pub fn validate_bfv_eight_party_decryption_statement_v1(
    statement: &BfvEightPartyDecryptionStatementV1,
) -> Result<(), BfvError> {
    if statement.version != BFV_EIGHT_PARTY_DECRYPTION_VERSION_V1 {
        return Err(BfvError::InvalidParameters(
            "BFV eight-party decryption statement version is not canonical".into(),
        ));
    }
    if statement.session_id == [0; 32] {
        return Err(BfvError::InvalidParameters(
            "BFV eight-party decryption session id must be nonzero".into(),
        ));
    }
    let params = &statement.parameters;
    validate_registered_bfv_parameters(params)?;
    bfv_public_key_digest(params, &statement.aggregate_public_key)?;
    bfv_ciphertext_digest(params, &statement.ciphertext)?;
    if statement
        .aggregate_public_key
        .a
        .iter()
        .all(|&value| value == 0)
    {
        return Err(BfvError::InvalidParameters(
            "BFV eight-party aggregate public-key a must be nonzero".into(),
        ));
    }
    let mut sum = vec![0; usize::from(params.polynomial_degree)];
    let mut previous_key: Option<&PublicKey> = None;
    for member in &statement.members {
        validate_bfv_eight_party_signing_key_v1(&member.signing_public_key)?;
        if previous_key.is_some_and(|previous| previous >= &member.signing_public_key) {
            return Err(BfvError::InvalidParameters(
                "BFV eight-party roster keys must be distinct and strictly ordered".into(),
            ));
        }
        previous_key = Some(&member.signing_public_key);
        validate_poly(params, &member.public_key_share_b, "BFV public key b share")?;
        if member.public_key_share_b.iter().all(|&value| value == 0) {
            return Err(BfvError::InvalidParameters(
                "BFV eight-party public-key b share must be nonzero".into(),
            ));
        }
        sum = poly_add_mod(params, &sum, &member.public_key_share_b);
    }
    if sum != statement.aggregate_public_key.b {
        return Err(BfvError::InvalidParameters(
            "BFV eight-party aggregate public-key b does not sum from roster shares".into(),
        ));
    }
    Ok(())
}

/// Decode and validate one canonical eight-party statement under explicit allocation limits.
///
/// The existing aggregate key and ciphertext contain variable-length polynomial vectors. The
/// 64-element per-sequence limit is enforced by Norito before those vectors allocate, while the
/// frame, cumulative element, allocation, and depth ceilings bound the complete decode.
///
/// # Errors
/// Returns [`BfvError`] for malformed, oversized, noncanonical, or invalid statements.
pub fn decode_bfv_eight_party_decryption_statement_bytes_v1(
    bytes: &[u8],
) -> Result<BfvEightPartyDecryptionStatementV1, BfvError> {
    if bytes.is_empty() || bytes.len() > BFV_EIGHT_PARTY_STATEMENT_MAX_BYTES_V1 {
        return Err(BfvError::InvalidParameters(
            "BFV eight-party statement exceeds its canonical byte bound".into(),
        ));
    }
    let limits = norito::DecodeLimits::new(
        BFV_EIGHT_PARTY_REGISTERED_COEFFICIENTS_V1,
        BFV_EIGHT_PARTY_STATEMENT_MAX_BYTES_V1,
        2_048,
        64 * 1024,
        16,
    );
    let statement =
        norito::decode_canonical_with_limits::<BfvEightPartyDecryptionStatementV1>(bytes, limits)
            .map_err(|err| {
            BfvError::InvalidParameters(format!(
                "BFV eight-party statement canonical decode failed: {err}"
            ))
        })?;
    validate_bfv_eight_party_decryption_statement_v1(&statement)?;
    Ok(statement)
}

/// Decode exactly eight canonical signed contributions under explicit allocation limits.
///
/// Context and signatures must still be checked with
/// [`validate_bfv_eight_party_decryption_authentication_v1`].
///
/// # Errors
/// Returns [`BfvError`] for malformed, oversized, noncanonical, or wrong-count inputs.
pub fn decode_bfv_eight_party_decryption_contributions_bytes_v1(
    bytes: &[u8],
) -> Result<
    [BfvEightPartySignedDecryptionContributionV1; BFV_EIGHT_PARTY_DECRYPTION_PARTICIPANTS_V1],
    BfvError,
> {
    if bytes.is_empty() || bytes.len() > BFV_EIGHT_PARTY_CONTRIBUTIONS_MAX_BYTES_V1 {
        return Err(BfvError::InvalidParameters(
            "BFV eight-party contributions exceed their canonical byte bound".into(),
        ));
    }
    norito::decode_canonical_with_limits::<
        [BfvEightPartySignedDecryptionContributionV1; BFV_EIGHT_PARTY_DECRYPTION_PARTICIPANTS_V1],
    >(
        bytes,
        norito::DecodeLimits::new(
            128,
            BFV_EIGHT_PARTY_CONTRIBUTIONS_MAX_BYTES_V1,
            1_024,
            32 * 1024,
            16,
        ),
    )
    .map_err(|err| {
        BfvError::InvalidParameters(format!(
            "BFV eight-party contributions canonical decode failed: {err}"
        ))
    })
}

/// Digest the exact frozen decryption statement under a domain-separated Norito encoding.
///
/// # Errors
/// Returns [`BfvError`] when the statement is invalid or canonical encoding fails.
pub fn bfv_eight_party_decryption_statement_digest_v1(
    statement: &BfvEightPartyDecryptionStatementV1,
) -> Result<Hash, BfvError> {
    validate_bfv_eight_party_decryption_statement_v1(statement)?;
    let encoded = norito::encode_canonical(statement).map_err(|err| {
        BfvError::InvalidParameters(format!(
            "BFV eight-party statement canonical encoding failed: {err}"
        ))
    })?;
    Ok(Hash::new_from_chunks(&[
        BFV_EIGHT_PARTY_DECRYPTION_STATEMENT_DIGEST_DOMAIN_V1,
        encoded.as_slice(),
    ]))
}

/// Authenticate all eight declarations against the frozen roster and statement.
///
/// This verifies identity, order, signature, session, and ciphertext context.
/// It is only an authentication check; it does not verify the secret-share or decryption-share
/// equations and cannot authorize release of a plaintext or scaled coefficient.
///
/// # Errors
/// Returns [`BfvError`] for missing, duplicate, reordered, replayed, forged, or malformed input.
pub fn validate_bfv_eight_party_decryption_authentication_v1(
    statement: &BfvEightPartyDecryptionStatementV1,
    contributions: &[BfvEightPartySignedDecryptionContributionV1],
) -> Result<(), BfvError> {
    let statement_digest = bfv_eight_party_decryption_statement_digest_v1(statement)?;
    if contributions.len() != BFV_EIGHT_PARTY_DECRYPTION_PARTICIPANTS_V1 {
        return Err(BfvError::InvalidParameters(format!(
            "BFV eight-party decryption requires exactly {BFV_EIGHT_PARTY_DECRYPTION_PARTICIPANTS_V1} signed contributions",
        )));
    }
    for (index, contribution) in contributions.iter().enumerate() {
        let payload = &contribution.payload;
        if payload.version != BFV_EIGHT_PARTY_DECRYPTION_VERSION_V1
            || payload.session_id != statement.session_id
            || payload.statement_digest != statement_digest
            || usize::from(payload.participant_index) != index
        {
            return Err(BfvError::InvalidParameters(format!(
                "BFV eight-party contribution {index} does not bind its canonical participant and session",
            )));
        }
        contribution
            .signature
            .verify(&statement.members[index].signing_public_key, payload)
            .map_err(|err| {
                BfvError::InvalidParameters(format!(
                    "BFV eight-party contribution {index} signature verification failed: {err}"
                ))
            })?;
    }
    Ok(())
}

/// Require sound verification before an eight-party result can be released.
///
/// The missing construction must protect each partial decryption from exposing `s_i`, then prove
/// that `b_i + a*s_i = e_i (mod q)` with registered secret/error bounds and that its committed,
/// privacy-preserving contribution is correctly derived from `c1*s_i`. It must account for
/// masking/noise in deterministic combination and rounding, cover every full source limb, and bind
/// the full roster, session, ciphertext, and parameters.
///
/// # Errors
/// Returns [`BfvError::EightPartyShareRelationProofUnavailable`] after authentication until a
/// sound contribution proof verifier and its audited production profile are implemented.
pub fn verify_bfv_eight_party_decryption_v1(
    statement: &BfvEightPartyDecryptionStatementV1,
    contributions: &[BfvEightPartySignedDecryptionContributionV1],
) -> Result<Vec<u64>, BfvError> {
    validate_bfv_eight_party_decryption_authentication_v1(statement, contributions)?;
    // TODO: Design a private partial-decryption protocol and verify its bounded ternary secret,
    // error, public-key-share, masking, and c1*s_i relations across the full source limbs.
    Err(BfvError::EightPartyShareRelationProofUnavailable)
}

#[cfg(test)]
mod tests;
