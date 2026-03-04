use std::sync::OnceLock;

use blake2::{
    Blake2bVar,
    digest::{Update, VariableOutput},
};
use curve25519_dalek::{
    constants::RISTRETTO_BASEPOINT_POINT,
    ristretto::{CompressedRistretto, RistrettoPoint},
    scalar::Scalar,
    traits::VartimeMultiscalarMul,
};
use iroha_crypto::Hash;
use iroha_data_model::{
    ChainId,
    isi::error::{InstructionExecutionError, InvalidParameterError},
    offline::OfflineBalanceProof,
};
use iroha_primitives::numeric::Numeric;
use rand_core_06::{OsRng, RngCore};
use sha2::Sha512;
use thiserror::Error;

const BALANCE_PROOF_VERSION: u8 = 1;
const DELTA_PROOF_BYTES: usize = 96;
const RANGE_PROOF_BITS: usize = 64;
const RANGE_PROOF_PER_BIT_BYTES: usize = 192;
const RANGE_PROOF_BYTES: usize = RANGE_PROOF_BITS * RANGE_PROOF_PER_BIT_BYTES;
const BALANCE_PROOF_BYTES: usize = 1 + DELTA_PROOF_BYTES + RANGE_PROOF_BYTES;
const PROOF_TRANSCRIPT_LABEL: &[u8] = b"iroha.offline.balance.v1";
const RANGE_PROOF_TRANSCRIPT_LABEL: &[u8] = b"iroha.offline.balance.range.v1";
const H_GENERATOR_LABEL: &[u8] = b"iroha.offline.balance.generator.H.v1";

pub(super) struct VerificationInputs<'a> {
    pub balance_proof: &'a OfflineBalanceProof,
    pub chain_id: &'a ChainId,
    pub expected_scale: u32,
}

pub(super) fn verify_balance_proof(
    inputs: &VerificationInputs<'_>,
) -> Result<(), InstructionExecutionError> {
    let Some(proof_bytes) = inputs.balance_proof.zk_proof.as_deref() else {
        return Err(BalanceProofError::MissingProof.into());
    };

    if proof_bytes.is_empty() {
        return Err(BalanceProofError::EmptyProof.into());
    }
    if proof_bytes[0] != BALANCE_PROOF_VERSION {
        return Err(BalanceProofError::UnsupportedProofVersion(proof_bytes[0]).into());
    }
    if proof_bytes.len() != BALANCE_PROOF_BYTES {
        return Err(BalanceProofError::InvalidProofLength {
            expected: BALANCE_PROOF_BYTES,
            actual: proof_bytes.len(),
        }
        .into());
    }

    let c_init = decode_commitment(&inputs.balance_proof.initial_commitment.commitment)?;
    let c_res = decode_commitment(&inputs.balance_proof.resulting_commitment)?;
    let delta_bytes =
        numeric_delta_bytes(&inputs.balance_proof.claimed_delta, inputs.expected_scale)?;
    let delta_proof = &proof_bytes[1..=DELTA_PROOF_BYTES];
    let range_proof = &proof_bytes[1 + DELTA_PROOF_BYTES..];
    let (r_point, s_g, s_h) = parse_delta_proof(delta_proof)?;
    let context = derive_context(inputs.chain_id);

    let u = c_res - c_init;
    let challenge = transcript_challenge(&c_init, &c_res, &delta_bytes, &context, &u, &r_point);

    let lhs = RistrettoPoint::vartime_multiscalar_mul(
        [s_g, s_h],
        [RISTRETTO_BASEPOINT_POINT, pedersen_generator_h()],
    );
    let rhs = r_point + (u * challenge);
    if lhs != rhs {
        return Err(BalanceProofError::InvalidProof.into());
    }

    verify_range_proof(range_proof, &c_res, &context)?;
    Ok(())
}

/// Build a balance proof blob for an offline bundle.
///
/// `expected_scale` defines the canonical scale for numeric values; both
/// `claimed_delta` and `resulting_value` must use it.
///
/// # Errors
///
/// Returns an error when commitments or scalars are malformed, or when the
/// resulting commitment does not match the provided value/blinding.
#[allow(clippy::too_many_arguments)]
pub fn build_balance_proof(
    chain_id: &ChainId,
    expected_scale: u32,
    claimed_delta: &Numeric,
    resulting_value: &Numeric,
    initial_commitment: &[u8],
    resulting_commitment: &[u8],
    initial_blinding: &[u8],
    resulting_blinding: &[u8],
) -> Result<Vec<u8>, InstructionExecutionError> {
    let claimed_delta_scale = claimed_delta.scale();
    if claimed_delta_scale != expected_scale {
        return Err(BalanceProofError::DeltaScaleMismatch {
            expected: expected_scale,
            actual: claimed_delta_scale,
        }
        .into());
    }
    let resulting_value_scale = resulting_value.scale();
    if resulting_value_scale != expected_scale {
        return Err(BalanceProofError::ValueScaleMismatch {
            expected: expected_scale,
            actual: resulting_value_scale,
        }
        .into());
    }
    let c_init = decode_commitment(initial_commitment)?;
    let c_res = decode_commitment(resulting_commitment)?;
    let delta_scalar = numeric_to_scalar_delta(claimed_delta, expected_scale)?;
    let delta_bytes = numeric_delta_bytes(claimed_delta, expected_scale)?;
    let resulting_value_u64 = numeric_to_u64(resulting_value, expected_scale)?;
    let blind_init = decode_scalar(initial_blinding)?;
    let blind_res = decode_scalar(resulting_blinding)?;
    let blind_delta = blind_res - blind_init;
    let context = derive_context(chain_id);
    let u = c_res - c_init;

    let mut rng = OsRng;
    let alpha = random_scalar(&mut rng);
    let beta = random_scalar(&mut rng);
    let r_point = RISTRETTO_BASEPOINT_POINT * alpha + pedersen_generator_h() * beta;
    let challenge = transcript_challenge(&c_init, &c_res, &delta_bytes, &context, &u, &r_point);
    let s_g = alpha + challenge * delta_scalar;
    let s_h = beta + challenge * blind_delta;

    let expected_commitment = RISTRETTO_BASEPOINT_POINT * Scalar::from(resulting_value_u64)
        + pedersen_generator_h() * blind_res;
    if expected_commitment != c_res {
        return Err(BalanceProofError::InvalidProof.into());
    }

    let range_proof = build_range_proof(&context, resulting_value_u64, blind_res);
    let mut proof = Vec::with_capacity(BALANCE_PROOF_BYTES);
    proof.push(BALANCE_PROOF_VERSION);
    proof.extend_from_slice(r_point.compress().as_bytes());
    proof.extend_from_slice(s_g.to_bytes().as_ref());
    proof.extend_from_slice(s_h.to_bytes().as_ref());
    proof.extend_from_slice(&range_proof);
    Ok(proof)
}

/// Compute a Pedersen commitment for the provided value and blinding scalar.
///
/// `expected_scale` defines the canonical scale for `value`.
///
/// # Errors
///
/// Returns an error when the value or blinding is out of range or malformed.
pub fn compute_commitment(
    value: &Numeric,
    expected_scale: u32,
    blinding: &[u8],
) -> Result<Vec<u8>, InstructionExecutionError> {
    let value_scalar = numeric_to_scalar_value(value, expected_scale)?;
    let blind = decode_scalar(blinding)?;
    let commitment = RISTRETTO_BASEPOINT_POINT * value_scalar + pedersen_generator_h() * blind;
    Ok(commitment.compress().as_bytes().to_vec())
}

#[derive(Debug, Error)]
enum BalanceProofError {
    #[error("balance proof bytes are empty")]
    EmptyProof,
    #[error("balance proof is required")]
    MissingProof,
    #[error("balance proof version {0} is unsupported")]
    UnsupportedProofVersion(u8),
    #[error("balance proof must be {expected} bytes (got {actual})")]
    InvalidProofLength { expected: usize, actual: usize },
    #[error("balance range proof bytes must be {expected} bytes (got {actual})")]
    InvalidRangeProofLength { expected: usize, actual: usize },
    #[error("commitment must be 32 bytes (got {0})")]
    CommitmentLength(usize),
    #[error("scalar must be 32 bytes (got {0})")]
    ScalarLength(usize),
    #[error("commitment encoding is invalid")]
    CommitmentEncoding,
    #[error("proof point encoding is invalid")]
    ProofPointEncoding,
    #[error("scalar encoding is not canonical")]
    NonCanonicalScalar,
    #[error("claimed delta must use scale {expected} (got {actual})")]
    DeltaScaleMismatch { expected: u32, actual: u32 },
    #[error("value must use scale {expected} (got {actual})")]
    ValueScaleMismatch { expected: u32, actual: u32 },
    #[error("claimed delta exceeds supported range")]
    DeltaOutOfRange,
    #[error("resulting value exceeds supported range")]
    ResultingValueOutOfRange,
    #[error("balance proof equation does not hold")]
    InvalidProof,
    #[error("balance range proof is invalid")]
    InvalidRangeProof,
    #[error("balance range proof commitment mismatch")]
    RangeProofCommitmentMismatch,
}

impl From<BalanceProofError> for InstructionExecutionError {
    fn from(err: BalanceProofError) -> Self {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            err.to_string(),
        ))
    }
}

fn decode_commitment(bytes: &[u8]) -> Result<RistrettoPoint, BalanceProofError> {
    if bytes.len() != 32 {
        return Err(BalanceProofError::CommitmentLength(bytes.len()));
    }
    let compressed = CompressedRistretto::from_slice(bytes)
        .map_err(|_| BalanceProofError::CommitmentLength(bytes.len()))?;
    compressed
        .decompress()
        .ok_or(BalanceProofError::CommitmentEncoding)
}

fn decode_point(bytes: &[u8]) -> Result<RistrettoPoint, BalanceProofError> {
    if bytes.len() != 32 {
        return Err(BalanceProofError::CommitmentLength(bytes.len()));
    }
    let compressed = CompressedRistretto::from_slice(bytes)
        .map_err(|_| BalanceProofError::ProofPointEncoding)?;
    compressed
        .decompress()
        .ok_or(BalanceProofError::ProofPointEncoding)
}

fn decode_scalar(bytes: &[u8]) -> Result<Scalar, BalanceProofError> {
    if bytes.len() != 32 {
        return Err(BalanceProofError::ScalarLength(bytes.len()));
    }
    let scalar = Scalar::from_canonical_bytes(
        bytes
            .try_into()
            .map_err(|_| BalanceProofError::ScalarLength(bytes.len()))?,
    );
    Option::from(scalar).ok_or(BalanceProofError::NonCanonicalScalar)
}

fn parse_delta_proof(bytes: &[u8]) -> Result<(RistrettoPoint, Scalar, Scalar), BalanceProofError> {
    let r_compressed = CompressedRistretto::from_slice(&bytes[0..32])
        .map_err(|_| BalanceProofError::ProofPointEncoding)?;
    let r_point = r_compressed
        .decompress()
        .ok_or(BalanceProofError::ProofPointEncoding)?;

    let s_g = decode_scalar(&bytes[32..64])?;
    let s_h = decode_scalar(&bytes[64..96])?;

    Ok((r_point, s_g, s_h))
}

fn numeric_delta_bytes(
    value: &Numeric,
    expected_scale: u32,
) -> Result<[u8; 16], BalanceProofError> {
    ensure_expected_scale(
        value,
        expected_scale,
        BalanceProofError::DeltaScaleMismatch {
            expected: expected_scale,
            actual: value.scale(),
        },
    )?;
    let mantissa = value
        .try_mantissa_u128()
        .ok_or(BalanceProofError::DeltaOutOfRange)?;
    let delta_i128 = i128::try_from(mantissa).map_err(|_| BalanceProofError::DeltaOutOfRange)?;
    Ok(delta_i128.to_le_bytes())
}

fn numeric_to_scalar_delta(
    value: &Numeric,
    expected_scale: u32,
) -> Result<Scalar, BalanceProofError> {
    ensure_expected_scale(
        value,
        expected_scale,
        BalanceProofError::DeltaScaleMismatch {
            expected: expected_scale,
            actual: value.scale(),
        },
    )?;
    let mantissa = value
        .try_mantissa_u128()
        .ok_or(BalanceProofError::DeltaOutOfRange)?;
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(&mantissa.to_le_bytes());
    Ok(Scalar::from_bytes_mod_order(bytes))
}

fn numeric_to_scalar_value(
    value: &Numeric,
    expected_scale: u32,
) -> Result<Scalar, BalanceProofError> {
    ensure_expected_scale(
        value,
        expected_scale,
        BalanceProofError::ValueScaleMismatch {
            expected: expected_scale,
            actual: value.scale(),
        },
    )?;
    let mantissa = value
        .try_mantissa_u128()
        .ok_or(BalanceProofError::ResultingValueOutOfRange)?;
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(&mantissa.to_le_bytes());
    Ok(Scalar::from_bytes_mod_order(bytes))
}

fn numeric_to_u64(value: &Numeric, expected_scale: u32) -> Result<u64, BalanceProofError> {
    ensure_expected_scale(
        value,
        expected_scale,
        BalanceProofError::ValueScaleMismatch {
            expected: expected_scale,
            actual: value.scale(),
        },
    )?;
    let mantissa = value
        .try_mantissa_u128()
        .ok_or(BalanceProofError::ResultingValueOutOfRange)?;
    u64::try_from(mantissa).map_err(|_| BalanceProofError::ResultingValueOutOfRange)
}

fn ensure_expected_scale(
    value: &Numeric,
    expected_scale: u32,
    err: BalanceProofError,
) -> Result<(), BalanceProofError> {
    if value.scale() != expected_scale {
        return Err(err);
    }
    Ok(())
}

fn transcript_challenge(
    c_init: &RistrettoPoint,
    c_res: &RistrettoPoint,
    delta_le: &[u8; 16],
    context: &[u8; 32],
    u: &RistrettoPoint,
    r_point: &RistrettoPoint,
) -> Scalar {
    let mut hasher = Blake2bVar::new(64).expect("Blake2b length is valid");
    hasher.update(PROOF_TRANSCRIPT_LABEL);
    hasher.update(c_init.compress().as_bytes());
    hasher.update(c_res.compress().as_bytes());
    hasher.update(delta_le);
    hasher.update(context);
    hasher.update(u.compress().as_bytes());
    hasher.update(r_point.compress().as_bytes());
    let mut output = [0u8; 64];
    hasher
        .finalize_variable(&mut output)
        .expect("output size matches");
    Scalar::from_bytes_mod_order_wide(&output)
}

fn random_scalar(rng: &mut impl RngCore) -> Scalar {
    let mut bytes = [0u8; 64];
    rng.fill_bytes(&mut bytes);
    Scalar::from_bytes_mod_order_wide(&bytes)
}

fn derive_context(chain_id: &ChainId) -> [u8; 32] {
    Hash::new(chain_id.as_str().as_bytes()).into()
}

fn pedersen_generator_h() -> RistrettoPoint {
    static GENERATOR: OnceLock<RistrettoPoint> = OnceLock::new();
    *GENERATOR.get_or_init(|| RistrettoPoint::hash_from_bytes::<Sha512>(H_GENERATOR_LABEL))
}

fn range_proof_challenge(
    context: &[u8; 32],
    bit_index: u8,
    commitment: &RistrettoPoint,
    a0: &RistrettoPoint,
    a1: &RistrettoPoint,
) -> Scalar {
    let mut hasher = Blake2bVar::new(64).expect("Blake2b length is valid");
    hasher.update(RANGE_PROOF_TRANSCRIPT_LABEL);
    hasher.update(context);
    hasher.update(&[bit_index]);
    hasher.update(commitment.compress().as_bytes());
    hasher.update(a0.compress().as_bytes());
    hasher.update(a1.compress().as_bytes());
    let mut output = [0u8; 64];
    hasher
        .finalize_variable(&mut output)
        .expect("output size matches");
    Scalar::from_bytes_mod_order_wide(&output)
}

fn verify_range_proof(
    proof_bytes: &[u8],
    resulting_commitment: &RistrettoPoint,
    context: &[u8; 32],
) -> Result<(), BalanceProofError> {
    if proof_bytes.len() != RANGE_PROOF_BYTES {
        return Err(BalanceProofError::InvalidRangeProofLength {
            expected: RANGE_PROOF_BYTES,
            actual: proof_bytes.len(),
        });
    }

    let mut commitments = Vec::with_capacity(RANGE_PROOF_BITS);
    for bit_index in 0..RANGE_PROOF_BITS {
        let offset = bit_index * RANGE_PROOF_PER_BIT_BYTES;
        let commitment = decode_point(&proof_bytes[offset..offset + 32])?;
        let a0 = decode_point(&proof_bytes[offset + 32..offset + 64])?;
        let a1 = decode_point(&proof_bytes[offset + 64..offset + 96])?;
        let e0 = decode_scalar(&proof_bytes[offset + 96..offset + 128])?;
        let s0 = decode_scalar(&proof_bytes[offset + 128..offset + 160])?;
        let s1 = decode_scalar(&proof_bytes[offset + 160..offset + 192])?;

        let challenge = range_proof_challenge(
            context,
            u8::try_from(bit_index).expect("range proof bits fit in u8"),
            &commitment,
            &a0,
            &a1,
        );
        let e1 = challenge - e0;
        let lhs0 = RistrettoPoint::vartime_multiscalar_mul(
            [s0, -e0],
            [pedersen_generator_h(), commitment],
        );
        if lhs0 != a0 {
            return Err(BalanceProofError::InvalidRangeProof);
        }
        let commitment_minus_g = commitment - RISTRETTO_BASEPOINT_POINT;
        let lhs1 = RistrettoPoint::vartime_multiscalar_mul(
            [s1, -e1],
            [pedersen_generator_h(), commitment_minus_g],
        );
        if lhs1 != a1 {
            return Err(BalanceProofError::InvalidRangeProof);
        }
        commitments.push(commitment);
    }

    let scalars: Vec<Scalar> = (0..RANGE_PROOF_BITS)
        .map(|index| Scalar::from(1u64 << index))
        .collect();
    let sum = RistrettoPoint::vartime_multiscalar_mul(scalars, commitments.iter());
    if &sum != resulting_commitment {
        return Err(BalanceProofError::RangeProofCommitmentMismatch);
    }
    Ok(())
}

fn build_range_proof(context: &[u8; 32], value: u64, resulting_blinding: Scalar) -> Vec<u8> {
    let mut rng = OsRng;
    let mut blindings = Vec::with_capacity(RANGE_PROOF_BITS);
    let mut sum = Scalar::ZERO;
    for bit_index in 0..(RANGE_PROOF_BITS - 1) {
        let blinding = random_scalar(&mut rng);
        sum += Scalar::from(1u64 << bit_index) * blinding;
        blindings.push(blinding);
    }
    let last_weight = Scalar::from(1u64 << (RANGE_PROOF_BITS - 1));
    let last_blinding = (resulting_blinding - sum) * last_weight.invert();
    blindings.push(last_blinding);

    let mut proof = Vec::with_capacity(RANGE_PROOF_BYTES);
    for (bit_index, blinding) in blindings.iter().enumerate() {
        let bit = ((value >> bit_index) & 1) == 1;
        let bit_scalar = Scalar::from(u64::from(bit));
        let commitment = RISTRETTO_BASEPOINT_POINT * bit_scalar + pedersen_generator_h() * blinding;
        let (a0, a1, e0, s0, s1) = if bit {
            let alpha = random_scalar(&mut rng);
            let e0 = random_scalar(&mut rng);
            let s0 = random_scalar(&mut rng);
            let a0 = pedersen_generator_h() * s0 - commitment * e0;
            let a1 = pedersen_generator_h() * alpha;
            let challenge = range_proof_challenge(
                context,
                u8::try_from(bit_index).expect("range proof bits fit in u8"),
                &commitment,
                &a0,
                &a1,
            );
            let e1 = challenge - e0;
            let s1 = alpha + e1 * blinding;
            (a0, a1, e0, s0, s1)
        } else {
            let alpha = random_scalar(&mut rng);
            let e1 = random_scalar(&mut rng);
            let s1 = random_scalar(&mut rng);
            let a0 = pedersen_generator_h() * alpha;
            let commitment_minus_g = commitment - RISTRETTO_BASEPOINT_POINT;
            let a1 = pedersen_generator_h() * s1 - commitment_minus_g * e1;
            let challenge = range_proof_challenge(
                context,
                u8::try_from(bit_index).expect("range proof bits fit in u8"),
                &commitment,
                &a0,
                &a1,
            );
            let e0 = challenge - e1;
            let s0 = alpha + e0 * blinding;
            (a0, a1, e0, s0, s1)
        };
        proof.extend_from_slice(commitment.compress().as_bytes());
        proof.extend_from_slice(a0.compress().as_bytes());
        proof.extend_from_slice(a1.compress().as_bytes());
        proof.extend_from_slice(e0.to_bytes().as_ref());
        proof.extend_from_slice(s0.to_bytes().as_ref());
        proof.extend_from_slice(s1.to_bytes().as_ref());
    }
    proof
}

#[cfg(test)]
mod tests {
    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        account::AccountId,
        asset::{AssetDefinitionId, AssetId},
        domain::DomainId,
        offline::OfflineAllowanceCommitment,
    };
    use iroha_primitives::numeric::Numeric;

    use super::*;

    fn scalar_from_i128(value: i128) -> Scalar {
        let magnitude = value.unsigned_abs();
        let mut bytes = [0u8; 32];
        bytes[..16].copy_from_slice(&magnitude.to_le_bytes());
        let scalar = Scalar::from_bytes_mod_order(bytes);
        if value.is_negative() { -scalar } else { scalar }
    }

    fn sample_account() -> AccountId {
        let domain: DomainId = "wonderland".parse().unwrap();
        let key_pair = KeyPair::random();
        AccountId::new(domain, key_pair.public_key().clone())
    }

    fn sample_asset() -> AssetId {
        let definition: AssetDefinitionId = "usd#wonderland".parse().unwrap();
        let owner = sample_account();
        AssetId::new(definition, owner)
    }

    fn pedersen_commit(value: u64, blind: Scalar) -> RistrettoPoint {
        let scalar = scalar_from_i128(i128::from(value));
        RISTRETTO_BASEPOINT_POINT * scalar + pedersen_generator_h() * blind
    }

    fn make_delta_proof(
        c_init: &RistrettoPoint,
        c_res: &RistrettoPoint,
        delta: u64,
        blind_delta: Scalar,
        context: [u8; 32],
    ) -> Vec<u8> {
        let u = *c_res - *c_init;
        let alpha = Scalar::from(7u64);
        let beta = Scalar::from(11u64);
        let r_point = RISTRETTO_BASEPOINT_POINT * alpha + pedersen_generator_h() * beta;
        let challenge = transcript_challenge(
            c_init,
            c_res,
            &i128::from(delta).to_le_bytes(),
            &context,
            &u,
            &r_point,
        );
        let delta_scalar = Scalar::from(delta);
        let s_g = alpha + challenge * delta_scalar;
        let s_h = beta + challenge * blind_delta;

        let mut proof = Vec::with_capacity(DELTA_PROOF_BYTES);
        proof.extend_from_slice(r_point.compress().as_bytes());
        proof.extend_from_slice(s_g.to_bytes().as_ref());
        proof.extend_from_slice(s_h.to_bytes().as_ref());
        proof
    }

    fn make_range_proof(value: u64, blinding: Scalar, context: [u8; 32]) -> Vec<u8> {
        let mut blindings = Vec::with_capacity(RANGE_PROOF_BITS);
        let mut sum = Scalar::ZERO;
        for bit_index in 0..(RANGE_PROOF_BITS - 1) {
            let scalar = Scalar::from(10 + bit_index as u64);
            sum += Scalar::from(1u64 << bit_index) * scalar;
            blindings.push(scalar);
        }
        let two_pow = Scalar::from(1u64 << (RANGE_PROOF_BITS - 1));
        let last = (blinding - sum) * two_pow.invert();
        blindings.push(last);

        let mut proof = Vec::with_capacity(RANGE_PROOF_BYTES);
        for (bit_index, blinding) in blindings.iter().enumerate() {
            let bit = ((value >> bit_index) & 1) == 1;
            let bit_scalar = Scalar::from(u64::from(bit));
            let commitment =
                RISTRETTO_BASEPOINT_POINT * bit_scalar + pedersen_generator_h() * blinding;
            let bit_index_u8 = u8::try_from(bit_index).expect("range proof bit index fits in u8");
            let (a0, a1, e0, s0, s1) =
                make_range_bit_proof(bit, bit_index_u8, &commitment, blinding, &context);
            proof.extend_from_slice(commitment.compress().as_bytes());
            proof.extend_from_slice(a0.compress().as_bytes());
            proof.extend_from_slice(a1.compress().as_bytes());
            proof.extend_from_slice(e0.to_bytes().as_ref());
            proof.extend_from_slice(s0.to_bytes().as_ref());
            proof.extend_from_slice(s1.to_bytes().as_ref());
        }
        proof
    }

    fn make_range_bit_proof(
        bit: bool,
        bit_index: u8,
        commitment: &RistrettoPoint,
        blinding: &Scalar,
        context: &[u8; 32],
    ) -> (RistrettoPoint, RistrettoPoint, Scalar, Scalar, Scalar) {
        if bit {
            let alpha = Scalar::from(40 + u64::from(bit_index));
            let e0 = Scalar::from(50 + u64::from(bit_index));
            let s0 = Scalar::from(60 + u64::from(bit_index));
            let a0 = pedersen_generator_h() * s0 - commitment * e0;
            let a1 = pedersen_generator_h() * alpha;
            let challenge = range_proof_challenge(context, bit_index, commitment, &a0, &a1);
            let e1 = challenge - e0;
            let s1 = alpha + e1 * blinding;
            (a0, a1, e0, s0, s1)
        } else {
            let alpha = Scalar::from(70 + u64::from(bit_index));
            let e1 = Scalar::from(80 + u64::from(bit_index));
            let s1 = Scalar::from(90 + u64::from(bit_index));
            let a0 = pedersen_generator_h() * alpha;
            let commitment_minus_g = commitment - RISTRETTO_BASEPOINT_POINT;
            let a1 = pedersen_generator_h() * s1 - commitment_minus_g * e1;
            let challenge = range_proof_challenge(context, bit_index, commitment, &a0, &a1);
            let e0 = challenge - e1;
            let s0 = alpha + e0 * blinding;
            (a0, a1, e0, s0, s1)
        }
    }

    fn assemble_balance_proof(delta_proof: &[u8], range_proof: &[u8]) -> Vec<u8> {
        let mut proof = Vec::with_capacity(BALANCE_PROOF_BYTES);
        proof.push(BALANCE_PROOF_VERSION);
        proof.extend_from_slice(delta_proof);
        proof.extend_from_slice(range_proof);
        proof
    }

    fn assert_smart_contract_error(err: &InstructionExecutionError, expected: &str) {
        let InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            message,
        )) = err
        else {
            panic!("unexpected error: {err}");
        };
        assert!(message.contains(expected), "unexpected error: {message}");
    }

    #[test]
    fn verify_accepts_valid_proof() {
        let value = 42;
        let delta = 10;
        let expected_scale = 0;
        let blind_start = Scalar::from(5u64);
        let blind_delta = Scalar::from(3u64);
        let c_init = pedersen_commit(value, blind_start);
        let c_res = pedersen_commit(value + delta, blind_start + blind_delta);
        let mut balance_proof = OfflineBalanceProof {
            initial_commitment: OfflineAllowanceCommitment {
                asset: sample_asset(),
                amount: Numeric::new(0, 0),
                commitment: c_init.compress().as_bytes().to_vec(),
            },
            resulting_commitment: c_res.compress().as_bytes().to_vec(),
            claimed_delta: Numeric::new(u128::from(delta), 0),
            zk_proof: None,
        };
        let chain_id: ChainId = "testnet".parse().unwrap();
        let context = derive_context(&chain_id);
        let delta_proof = make_delta_proof(&c_init, &c_res, delta, blind_delta, context);
        let range_proof = make_range_proof(value + delta, blind_start + blind_delta, context);
        balance_proof.zk_proof = Some(assemble_balance_proof(&delta_proof, &range_proof));
        let inputs = VerificationInputs {
            balance_proof: &balance_proof,
            chain_id: &chain_id,
            expected_scale,
        };
        assert!(verify_balance_proof(&inputs).is_ok());
    }

    #[test]
    fn build_balance_proof_roundtrip() {
        let initial_value = 25u64;
        let delta = 7u64;
        let resulting_value = initial_value + delta;
        let expected_scale = 2;
        let initial_blind = Scalar::from(5u64);
        let resulting_blind = Scalar::from(11u64);
        let initial_commitment = pedersen_commit(initial_value, initial_blind)
            .compress()
            .as_bytes()
            .to_vec();
        let resulting_commitment = pedersen_commit(resulting_value, resulting_blind)
            .compress()
            .as_bytes()
            .to_vec();
        let chain_id: ChainId = "testnet".parse().unwrap();
        let claimed_delta = Numeric::new(u128::from(delta), expected_scale);
        let resulting_value_numeric = Numeric::new(u128::from(resulting_value), expected_scale);
        let initial_blinding = initial_blind.to_bytes();
        let resulting_blinding = resulting_blind.to_bytes();

        let proof = build_balance_proof(
            &chain_id,
            expected_scale,
            &claimed_delta,
            &resulting_value_numeric,
            &initial_commitment,
            &resulting_commitment,
            &initial_blinding,
            &resulting_blinding,
        )
        .expect("balance proof builds");

        let balance_proof = OfflineBalanceProof {
            initial_commitment: OfflineAllowanceCommitment {
                asset: sample_asset(),
                amount: Numeric::new(u128::from(initial_value), expected_scale),
                commitment: initial_commitment,
            },
            resulting_commitment,
            claimed_delta,
            zk_proof: Some(proof),
        };
        let inputs = VerificationInputs {
            balance_proof: &balance_proof,
            chain_id: &chain_id,
            expected_scale,
        };
        assert!(verify_balance_proof(&inputs).is_ok());
    }

    #[test]
    fn compute_commitment_rejects_invalid_blinding_length() {
        let value = Numeric::new(1, 0);
        let err = compute_commitment(&value, 0, &[0u8; 31])
            .expect_err("invalid blinding length should be rejected");
        assert_smart_contract_error(&err, "scalar must be 32 bytes");
    }

    #[test]
    fn build_balance_proof_rejects_invalid_blinding_length() {
        let chain_id: ChainId = "testnet".parse().unwrap();
        let initial_value = 10u64;
        let delta = 5u64;
        let resulting_value = initial_value + delta;
        let expected_scale = 0;
        let initial_blind = Scalar::from(3u64);
        let resulting_blind = Scalar::from(7u64);
        let initial_commitment = pedersen_commit(initial_value, initial_blind)
            .compress()
            .as_bytes()
            .to_vec();
        let resulting_commitment = pedersen_commit(resulting_value, resulting_blind)
            .compress()
            .as_bytes()
            .to_vec();
        let claimed_delta = Numeric::new(u128::from(delta), expected_scale);
        let resulting_value_numeric = Numeric::new(u128::from(resulting_value), expected_scale);
        let invalid_blinding = [0u8; 31];
        let resulting_blinding = resulting_blind.to_bytes();

        let err = build_balance_proof(
            &chain_id,
            expected_scale,
            &claimed_delta,
            &resulting_value_numeric,
            &initial_commitment,
            &resulting_commitment,
            &invalid_blinding,
            &resulting_blinding,
        )
        .expect_err("invalid blinding length should be rejected");
        assert_smart_contract_error(&err, "scalar must be 32 bytes");
    }

    #[test]
    fn build_balance_proof_rejects_fractional_delta() {
        let chain_id: ChainId = "testnet".parse().unwrap();
        let initial_value = 10u64;
        let expected_scale = 0;
        let delta = Numeric::new(15, 1);
        let resulting_value = Numeric::new(25, 0);
        let initial_blind = Scalar::from(5u64);
        let resulting_blind = Scalar::from(9u64);
        let initial_commitment = pedersen_commit(initial_value, initial_blind)
            .compress()
            .as_bytes()
            .to_vec();
        let resulting_commitment = pedersen_commit(25, resulting_blind)
            .compress()
            .as_bytes()
            .to_vec();
        let initial_blinding = initial_blind.to_bytes();
        let resulting_blinding = resulting_blind.to_bytes();

        let err = build_balance_proof(
            &chain_id,
            expected_scale,
            &delta,
            &resulting_value,
            &initial_commitment,
            &resulting_commitment,
            &initial_blinding,
            &resulting_blinding,
        )
        .expect_err("fractional delta should be rejected");
        assert_smart_contract_error(&err, "claimed delta must use scale 0");
    }

    #[test]
    fn build_balance_proof_rejects_fractional_value() {
        let chain_id: ChainId = "testnet".parse().unwrap();
        let initial_value = 10u64;
        let expected_scale = 0;
        let delta = Numeric::new(5, 0);
        let resulting_value = Numeric::new(125, 1);
        let initial_blind = Scalar::from(5u64);
        let resulting_blind = Scalar::from(9u64);
        let initial_commitment = pedersen_commit(initial_value, initial_blind)
            .compress()
            .as_bytes()
            .to_vec();
        let resulting_commitment = pedersen_commit(12, resulting_blind)
            .compress()
            .as_bytes()
            .to_vec();
        let initial_blinding = initial_blind.to_bytes();
        let resulting_blinding = resulting_blind.to_bytes();

        let err = build_balance_proof(
            &chain_id,
            expected_scale,
            &delta,
            &resulting_value,
            &initial_commitment,
            &resulting_commitment,
            &initial_blinding,
            &resulting_blinding,
        )
        .expect_err("fractional value should be rejected");
        assert_smart_contract_error(&err, "value must use scale 0");
    }

    #[test]
    fn verify_rejects_fractional_delta() {
        let value = 42;
        let delta = 10;
        let expected_scale = 0;
        let blind_start = Scalar::from(5u64);
        let blind_delta = Scalar::from(3u64);
        let c_init = pedersen_commit(value, blind_start);
        let c_res = pedersen_commit(value + delta, blind_start + blind_delta);
        let chain_id: ChainId = "testnet".parse().unwrap();
        let context = derive_context(&chain_id);
        let delta_proof = make_delta_proof(&c_init, &c_res, delta, blind_delta, context);
        let range_proof = make_range_proof(value + delta, blind_start + blind_delta, context);
        let balance_proof = OfflineBalanceProof {
            initial_commitment: OfflineAllowanceCommitment {
                asset: sample_asset(),
                amount: Numeric::new(0, 0),
                commitment: c_init.compress().as_bytes().to_vec(),
            },
            resulting_commitment: c_res.compress().as_bytes().to_vec(),
            claimed_delta: Numeric::new(100, 1),
            zk_proof: Some(assemble_balance_proof(&delta_proof, &range_proof)),
        };
        let inputs = VerificationInputs {
            balance_proof: &balance_proof,
            chain_id: &chain_id,
            expected_scale,
        };
        let err = verify_balance_proof(&inputs).expect_err("fractional delta should be rejected");
        assert_smart_contract_error(&err, "claimed delta must use scale 0");
    }

    #[test]
    fn verify_rejects_missing_proof() {
        let value = 5;
        let blind_start = Scalar::from(2u64);
        let c_init = pedersen_commit(value, blind_start);
        let c_res = pedersen_commit(value, blind_start);
        let balance_proof = OfflineBalanceProof {
            initial_commitment: OfflineAllowanceCommitment {
                asset: sample_asset(),
                amount: Numeric::new(u128::from(value), 0),
                commitment: c_init.compress().as_bytes().to_vec(),
            },
            resulting_commitment: c_res.compress().as_bytes().to_vec(),
            claimed_delta: Numeric::new(0, 0),
            zk_proof: None,
        };
        let chain_id: ChainId = "testnet".parse().unwrap();
        let inputs = VerificationInputs {
            balance_proof: &balance_proof,
            chain_id: &chain_id,
            expected_scale: 0,
        };
        assert!(verify_balance_proof(&inputs).is_err());
    }

    #[test]
    fn verify_rejects_invalid_length() {
        let chain_id: ChainId = "testnet".parse().unwrap();
        let mut balance_proof = OfflineBalanceProof {
            initial_commitment: OfflineAllowanceCommitment {
                asset: sample_asset(),
                amount: Numeric::new(0, 0),
                commitment: vec![0; 32],
            },
            resulting_commitment: vec![0; 32],
            claimed_delta: Numeric::new(0, 0),
            zk_proof: Some(vec![0u8; 10]),
        };
        let inputs = VerificationInputs {
            balance_proof: &balance_proof,
            chain_id: &chain_id,
            expected_scale: 0,
        };
        assert!(verify_balance_proof(&inputs).is_err());
        balance_proof.zk_proof = Some(vec![0u8; BALANCE_PROOF_BYTES + 1]);
        let inputs = VerificationInputs {
            balance_proof: &balance_proof,
            chain_id: &chain_id,
            expected_scale: 0,
        };
        assert!(verify_balance_proof(&inputs).is_err());
    }

    #[test]
    fn verify_rejects_invalid_commitment_encoding() {
        let chain_id: ChainId = "testnet".parse().unwrap();
        let balance_proof = OfflineBalanceProof {
            initial_commitment: OfflineAllowanceCommitment {
                asset: sample_asset(),
                amount: Numeric::new(0, 0),
                commitment: vec![1u8; 31],
            },
            resulting_commitment: vec![0; 32],
            claimed_delta: Numeric::new(0, 0),
            zk_proof: Some(vec![0u8; BALANCE_PROOF_BYTES]),
        };
        let inputs = VerificationInputs {
            balance_proof: &balance_proof,
            chain_id: &chain_id,
            expected_scale: 0,
        };
        assert!(verify_balance_proof(&inputs).is_err());
    }

    #[test]
    fn verify_rejects_non_canonical_scalar() {
        let value = 5;
        let delta = 2;
        let blind_start = Scalar::from(3u64);
        let blind_delta = Scalar::from(7u64);
        let c_init = pedersen_commit(value, blind_start);
        let c_res = pedersen_commit(value + delta, blind_start + blind_delta);
        let chain_id: ChainId = "testnet".parse().unwrap();
        let context = derive_context(&chain_id);
        let delta_proof = make_delta_proof(&c_init, &c_res, delta, blind_delta, context);
        let range_proof = make_range_proof(value + delta, blind_start + blind_delta, context);
        let mut proof = assemble_balance_proof(&delta_proof, &range_proof);
        // Force s_G bytes to be non-canonical (all 0xff).
        let start = 1 + 32;
        let end = 1 + 64;
        for byte in &mut proof[start..end] {
            *byte = 0xFF;
        }
        let balance_proof = OfflineBalanceProof {
            initial_commitment: OfflineAllowanceCommitment {
                asset: sample_asset(),
                amount: Numeric::new(u128::from(value), 0),
                commitment: c_init.compress().as_bytes().to_vec(),
            },
            resulting_commitment: c_res.compress().as_bytes().to_vec(),
            claimed_delta: Numeric::new(u128::from(delta), 0),
            zk_proof: Some(proof),
        };
        let inputs = VerificationInputs {
            balance_proof: &balance_proof,
            chain_id: &chain_id,
            expected_scale: 0,
        };
        assert!(verify_balance_proof(&inputs).is_err());
    }
}
