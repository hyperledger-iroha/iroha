//! Zero-knowledge envelope types (Norito TLV payloads).

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Backend tag for zero-knowledge verifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
pub enum BackendTag {
    /// Halo2 IPA over Pasta curves
    Halo2IpaPasta,
    /// Halo2 over BN254 (optional)
    Halo2Bn254,
    /// Groth16 (stub)
    Groth16,
    /// STARK/FRI (transparent, no trusted setup)
    Stark,
    /// Unknown/unsupported backend
    Unsupported,
}

#[cfg(feature = "json")]
impl norito::json::JsonSerialize for BackendTag {
    fn json_serialize(&self, out: &mut String) {
        let label = match self {
            BackendTag::Halo2IpaPasta => "halo2-ipa-pasta",
            BackendTag::Halo2Bn254 => "halo2-bn254",
            BackendTag::Groth16 => "groth16",
            BackendTag::Stark => "stark",
            BackendTag::Unsupported => "unsupported",
        };
        norito::json::write_json_string(label, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for BackendTag {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        let tag = match value.as_str() {
            "halo2-ipa-pasta" => BackendTag::Halo2IpaPasta,
            "halo2-bn254" => BackendTag::Halo2Bn254,
            "groth16" => BackendTag::Groth16,
            "stark" => BackendTag::Stark,
            _ => BackendTag::Unsupported,
        };
        Ok(tag)
    }
}

/// Envelope for open-verify operations (canonical `SignedQuery` layout).
///
/// This structure is serialized with Norito and used as the TLV payload for
/// `&NoritoBytes` pointer-ABI types passed to IVM verify syscalls or host vendor bridges.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OpenVerifyEnvelope {
    /// Backend tag string (e.g., `halo2-ipa-pasta`).
    pub backend: BackendTag,
    /// Circuit identifier string (backend-specific; opaque to host).
    pub circuit_id: String,
    /// Verifying-key hash (Blake2b-32) if known; all zeros if inline.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub vk_hash: [u8; 32],
    /// Public inputs (opaque bytes; backend-specific canonical encoding).
    pub public_inputs: Vec<u8>,
    /// Proof bytes (opaque, backend-specific canonical encoding).
    pub proof_bytes: Vec<u8>,
    /// Opaque aux map encoded as JSON bytes (for small structured extras).
    pub aux: Vec<u8>,
}

impl OpenVerifyEnvelope {
    /// Create a new envelope with required fields; `aux` defaults to empty.
    pub fn new(
        backend: BackendTag,
        circuit_id: impl Into<String>,
        vk_hash: [u8; 32],
        public_inputs: Vec<u8>,
        proof_bytes: Vec<u8>,
    ) -> Self {
        Self {
            backend,
            circuit_id: circuit_id.into(),
            vk_hash,
            public_inputs,
            proof_bytes,
            aux: Vec::new(),
        }
    }
}

// Note: Norito serialization is derived via `Encode`/`Decode` (packed structs compatible)

/// STARK/FRI proof payload embedded inside [`OpenVerifyEnvelope::proof_bytes`] when
/// [`OpenVerifyEnvelope::backend`] is [`BackendTag::Stark`].
///
/// This wrapper carries:
/// - `public_inputs`: public inputs expressed as 32-byte words, column-major (matching
///   the instance-column layout used by Halo2 envelopes), and
/// - `envelope_bytes`: backend-native proof bytes (typically a Norito-encoded STARK/FRI
///   envelope such as `StarkVerifyEnvelopeV1`).
///
/// Higher-level flows (governance voting, `Executable::IvmProved`, etc.) interpret the public
/// inputs according to the circuit/policy definitions and must validate their semantics.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct StarkFriOpenProofV1 {
    /// Version tag for format evolution.
    pub version: u16,
    /// Public inputs encoded as 32-byte words, column-major.
    pub public_inputs: Vec<Vec<[u8; 32]>>,
    /// Backend-native proof envelope bytes.
    pub envelope_bytes: Vec<u8>,
}
