//! Norito-typed VRF request envelopes used by the `VRF_VERIFY` and
//! `VRF_VERIFY_BATCH` syscalls.
//!
//! Keeping these types in a shared module ensures the Norito schema hash baked
//! into encoded envelopes matches what the host expects to decode. Tests should
//! import and use these exact types for encoding requests.

/// Single VRF verification request.
#[derive(norito::Encode, norito::Decode)]
pub struct VrfVerifyRequest {
    /// Variant: 1 = SigInG2 (pk in G1, proof in G2); 2 = SigInG1 (pk in G2, proof in G1).
    pub variant: u8,
    /// Public key bytes (48 bytes for G1, 96 bytes for G2).
    pub pk: Vec<u8>,
    /// Proof/signature bytes (96 bytes for G2, 48 bytes for G1).
    pub proof: Vec<u8>,
    /// Chain identifier; host may enforce a configured value.
    pub chain_id: Vec<u8>,
    /// Input message bytes.
    pub input: Vec<u8>,
}

/// Batch verification request for multiple VRF items.
#[derive(norito::Encode, norito::Decode)]
pub struct VrfVerifyBatchRequest {
    /// Items to verify in order; outputs preserve input order.
    pub items: Vec<VrfVerifyRequest>,
}

/// Request envelope for reading a persisted VRF epoch seed snapshot.
#[derive(norito::Encode, norito::Decode)]
pub struct VrfEpochSeedRequest {
    /// Epoch to fetch from world snapshot storage.
    pub epoch: u64,
    /// If true and `epoch` is missing, return the latest known epoch seed.
    #[norito(default)]
    pub fallback_to_latest: bool,
}

/// Response envelope for `VrfEpochSeedRequest`.
#[derive(norito::Encode, norito::Decode)]
pub struct VrfEpochSeedResponse {
    /// Whether a seed snapshot was found.
    pub found: bool,
    /// Epoch associated with `seed` (requested or latest fallback epoch).
    pub epoch: u64,
    /// Seed bytes (all-zero when `found == false`).
    pub seed: [u8; 32],
}
