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
use sha2::Sha512;
use thiserror::Error;

const PROOF_BYTES: usize = 96;
const PROOF_TRANSCRIPT_LABEL: &[u8] = b"iroha.offline.balance.v1";
const H_GENERATOR_LABEL: &[u8] = b"iroha.offline.balance.generator.H.v1";

pub(super) struct VerificationInputs<'a> {
    pub balance_proof: &'a OfflineBalanceProof,
    pub chain_id: &'a ChainId,
}

pub(super) fn verify_balance_proof(
    inputs: &VerificationInputs<'_>,
) -> Result<(), InstructionExecutionError> {
    let Some(proof_bytes) = inputs.balance_proof.zk_proof.as_deref() else {
        return Ok(());
    };

    if proof_bytes.is_empty() {
        return Err(BalanceProofError::EmptyProof.into());
    }
    if proof_bytes.len() != PROOF_BYTES {
        return Err(BalanceProofError::InvalidProofLength {
            expected: PROOF_BYTES,
            actual: proof_bytes.len(),
        }
        .into());
    }

    let c_init = decode_commitment(&inputs.balance_proof.initial_commitment.commitment)?;
    let c_res = decode_commitment(&inputs.balance_proof.resulting_commitment)?;
    let delta_bytes = numeric_delta_bytes(&inputs.balance_proof.claimed_delta)?;
    let (r_point, s_g, s_h) = parse_proof(proof_bytes)?;
    let context = derive_context(inputs.chain_id);

    let u = c_res - c_init;
    let challenge = transcript_challenge(&c_init, &c_res, &delta_bytes, &context, &u, &r_point);

    let lhs = RistrettoPoint::vartime_multiscalar_mul(
        [s_g, s_h],
        [RISTRETTO_BASEPOINT_POINT, pedersen_generator_h()],
    );
    let rhs = r_point + (u * challenge);
    if lhs == rhs {
        Ok(())
    } else {
        Err(BalanceProofError::InvalidProof.into())
    }
}

#[derive(Debug, Error)]
enum BalanceProofError {
    #[error("balance proof bytes are empty")]
    EmptyProof,
    #[error("balance proof must be {expected} bytes (got {actual})")]
    InvalidProofLength { expected: usize, actual: usize },
    #[error("commitment must be 32 bytes (got {0})")]
    CommitmentLength(usize),
    #[error("commitment encoding is invalid")]
    CommitmentEncoding,
    #[error("proof point encoding is invalid")]
    ProofPointEncoding,
    #[error("scalar encoding is not canonical")]
    NonCanonicalScalar,
    #[error("claimed delta exceeds supported range")]
    DeltaOutOfRange,
    #[error("balance proof equation does not hold")]
    InvalidProof,
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

fn parse_proof(bytes: &[u8]) -> Result<(RistrettoPoint, Scalar, Scalar), BalanceProofError> {
    let r_compressed = CompressedRistretto::from_slice(&bytes[0..32])
        .map_err(|_| BalanceProofError::ProofPointEncoding)?;
    let r_point = r_compressed
        .decompress()
        .ok_or(BalanceProofError::ProofPointEncoding)?;

    let s_g = Scalar::from_canonical_bytes(bytes[32..64].try_into().unwrap());
    let Some(s_g) = Option::from(s_g) else {
        return Err(BalanceProofError::NonCanonicalScalar);
    };
    let s_h = Scalar::from_canonical_bytes(bytes[64..96].try_into().unwrap());
    let Some(s_h) = Option::from(s_h) else {
        return Err(BalanceProofError::NonCanonicalScalar);
    };

    Ok((r_point, s_g, s_h))
}

fn numeric_delta_bytes(value: &Numeric) -> Result<[u8; 16], BalanceProofError> {
    let mantissa = value
        .try_mantissa_u128()
        .ok_or(BalanceProofError::DeltaOutOfRange)?;
    let delta_i128 = i128::try_from(mantissa).map_err(|_| BalanceProofError::DeltaOutOfRange)?;
    Ok(delta_i128.to_le_bytes())
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

fn derive_context(chain_id: &ChainId) -> [u8; 32] {
    Hash::new(chain_id.as_str().as_bytes()).into()
}

fn pedersen_generator_h() -> RistrettoPoint {
    static GENERATOR: OnceLock<RistrettoPoint> = OnceLock::new();
    *GENERATOR.get_or_init(|| RistrettoPoint::hash_from_bytes::<Sha512>(H_GENERATOR_LABEL))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        account::AccountId,
        asset::{AssetDefinitionId, AssetId},
        domain::DomainId,
        offline::OfflineAllowanceCommitment,
    };
    use iroha_primitives::numeric::Numeric;

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

    fn make_proof(
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

        let mut proof = Vec::with_capacity(PROOF_BYTES);
        proof.extend_from_slice(r_point.compress().as_bytes());
        proof.extend_from_slice(s_g.to_bytes().as_ref());
        proof.extend_from_slice(s_h.to_bytes().as_ref());
        proof
    }

    #[test]
    fn verify_accepts_valid_proof() {
        let value = 42;
        let delta = 10;
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
        let proof = make_proof(&c_init, &c_res, delta, blind_delta, context);
        balance_proof.zk_proof = Some(proof);
        let inputs = VerificationInputs {
            balance_proof: &balance_proof,
            chain_id: &chain_id,
        };
        assert!(verify_balance_proof(&inputs).is_ok());
    }

    #[test]
    fn verify_accepts_missing_proof() {
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
        };
        assert!(verify_balance_proof(&inputs).is_ok());
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
        };
        assert!(verify_balance_proof(&inputs).is_err());
        balance_proof.zk_proof = Some(vec![0u8; PROOF_BYTES + 1]);
        let inputs = VerificationInputs {
            balance_proof: &balance_proof,
            chain_id: &chain_id,
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
            zk_proof: Some(vec![0u8; PROOF_BYTES]),
        };
        let inputs = VerificationInputs {
            balance_proof: &balance_proof,
            chain_id: &chain_id,
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
        let mut proof = make_proof(&c_init, &c_res, delta, blind_delta, context);
        // Force s_G bytes to be non-canonical (all 0xff).
        for byte in &mut proof[32..64] {
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
        };
        assert!(verify_balance_proof(&inputs).is_err());
    }
}
