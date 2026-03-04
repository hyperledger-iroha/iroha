//! Norito payloads and helpers for ZK verify syscalls using Halo2 (transparent IPA).
#![allow(unexpected_cfgs)]

#[cfg(feature = "goldilocks_backend")]
use iroha_zkp_halo2::backend::goldilocks;
use iroha_zkp_halo2::{
    OpenVerifyEnvelope, Transcript,
    backend::{bn254, pallas},
    norito_helpers::{self as nh, DecodedEnvelope},
};

/// Verify a Halo2 IPA polynomial opening encoded as a Norito envelope (outer TLV payload).
pub fn verify_open_envelope(raw: &[u8]) -> Result<bool, iroha_zkp_halo2::Error> {
    let env: OpenVerifyEnvelope =
        norito::decode_from_bytes(raw).map_err(|_| iroha_zkp_halo2::Error::VerificationFailed)?;
    let decoded = nh::decode_envelope(&env)?;
    let mut tr = Transcript::new(&env.transcript_label);
    let result = match decoded {
        DecodedEnvelope::Pallas {
            params,
            proof,
            z,
            t,
            p_g,
        } => pallas::Polynomial::verify_open(params.as_ref(), &mut tr, z, p_g, t, proof.as_ref()),
        DecodedEnvelope::Bn254 {
            params,
            proof,
            z,
            t,
            p_g,
        } => bn254::Polynomial::verify_open(params.as_ref(), &mut tr, z, p_g, t, proof.as_ref()),
        #[cfg(feature = "goldilocks_backend")]
        DecodedEnvelope::Goldilocks {
            params,
            proof,
            z,
            t,
            p_g,
        } => {
            goldilocks::Polynomial::verify_open(params.as_ref(), &mut tr, z, p_g, t, proof.as_ref())
        }
        #[cfg(not(feature = "goldilocks_backend"))]
        DecodedEnvelope::Goldilocks => {
            return Err(iroha_zkp_halo2::Error::UnsupportedBackend {
                backend: iroha_zkp_halo2::ZkCurveId::Goldilocks,
            });
        }
    };
    match result {
        Ok(()) => Ok(true),
        Err(e) => {
            // On verification failure signal false but keep error to allow callers to map to VMError.
            if matches!(e, iroha_zkp_halo2::Error::VerificationFailed) {
                Ok(false)
            } else {
                Err(e)
            }
        }
    }
}

/// Build an `OpenVerifyEnvelope` from wire-compatible structs with a fixed transcript label.
pub fn build_open_verify_envelope(
    params: iroha_zkp_halo2::IpaParams,
    public: iroha_zkp_halo2::PolyOpenPublic,
    proof: iroha_zkp_halo2::IpaProofData,
    transcript_label: &str,
) -> iroha_zkp_halo2::OpenVerifyEnvelope {
    iroha_zkp_halo2::OpenVerifyEnvelope {
        params,
        public,
        proof,
        transcript_label: transcript_label.to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    }
}

/// Verify multiple envelopes and return per-envelope results.
pub fn batch_verify_open_envelopes(
    envs: &[iroha_zkp_halo2::OpenVerifyEnvelope],
) -> Vec<Result<bool, iroha_zkp_halo2::Error>> {
    iroha_zkp_halo2::batch::verify_open_batch(envs)
}

// Request/response structs for state-read syscalls (roots/tally)
#[derive(Debug, Clone, norito::NoritoSerialize, norito::NoritoDeserialize)]
pub struct RootsGetRequest {
    pub asset_id: String,
    pub max: u32,
}

#[derive(Debug, Clone, norito::NoritoSerialize, norito::NoritoDeserialize)]
pub struct RootsGetResponse {
    pub latest: [u8; 32],
    pub roots: Vec<[u8; 32]>,
    pub height: u32,
}

#[derive(Debug, Clone, norito::NoritoSerialize, norito::NoritoDeserialize)]
pub struct VoteGetTallyRequest {
    pub election_id: String,
}

#[derive(Debug, Clone, norito::NoritoSerialize, norito::NoritoDeserialize)]
pub struct VoteGetTallyResponse {
    pub finalized: bool,
    pub tally: Vec<u64>,
}
