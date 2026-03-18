//! Norito-serializable wire types for IPA-based Halo2 transparent backend.
#![allow(unexpected_cfgs)]
//!
//! These types define stable, versioned layouts for parameters and proofs so
//! they can be archived, transmitted, and persisted across components.

use norito::{NoritoDeserialize, NoritoSerialize};

use crate::hash::sha3_256;

/// Curve identifier for wire payloads.
///
/// These codes disambiguate encodings across backends. The current crate uses
/// the Halo2 Pasta Pallas backend by default. Future backends will introduce new
/// IDs while preserving the existing ones.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum ZkCurveId {
    /// Unknown/unspecified curve (reserved).
    Unknown = 0,
    /// Halo2 Pasta/Pallas backend (transparent IPA over Pasta).
    Pallas = 1,
    /// Goldilocks (64-bit field) backend with multiplicative commitments.
    Goldilocks = 2,
    /// Pasta (Vesta) placeholder for future backends.
    Pasta = 10,
    /// BN254 IPA backend (transparent, G1/Fr commitment scheme).
    Bn254 = 20,
}

impl ZkCurveId {
    /// Convert raw u16 to `ZkCurveId`, mapping unknown codes to `Unknown`.
    pub fn from_u16(v: u16) -> Self {
        match v {
            1 => ZkCurveId::Pallas,
            2 => ZkCurveId::Goldilocks,
            10 => ZkCurveId::Pasta,
            20 => ZkCurveId::Bn254,
            _ => ZkCurveId::Unknown,
        }
    }

    /// Return the u16 code.
    pub const fn as_u16(self) -> u16 {
        self as u16
    }
}

/// IPA parameters (transparent, deterministic generators).
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct IpaParams {
    /// Reserved for format evolution; set to 1.
    pub version: u16,
    /// Curve identifier.
    pub curve_id: u16,
    /// Vector length `n` (power of two).
    pub n: u32,
    /// G generators, encoded as compressed curve points.
    pub g: Vec<[u8; 32]>,
    /// H generators, encoded as compressed curve points.
    pub h: Vec<[u8; 32]>,
    /// U generator, encoded as a compressed curve point.
    pub u: [u8; 32],
}

impl IpaParams {
    /// Encode to bare Norito payload bytes (no outer header). Panics on IO error.
    pub fn encode_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let flags = norito::core::default_encode_flags();
        let _guard = norito::core::DecodeFlagsGuard::enter(flags);
        norito::core::NoritoSerialize::serialize(self, &mut out)
            .expect("IpaParams Norito serialization failed");
        out
    }

    /// Decode from a bare Norito payload.
    pub fn decode_bytes(bytes: &[u8]) -> Result<Self, norito::Error> {
        let flags = norito::core::default_encode_flags();
        let _flags = norito::core::DecodeFlagsGuard::enter(flags);
        let archived = norito::core::archived_from_slice_unchecked::<Self>(bytes);
        let _ctx = norito::core::PayloadCtxGuard::enter(archived.bytes());
        <Self as norito::NoritoDeserialize>::try_deserialize(archived.as_ref())
    }

    /// Compute a deterministic fingerprint of the parameter set.
    pub fn fingerprint(&self) -> [u8; 32] {
        fingerprint_bytes(self.curve_id, self.n, &self.g, &self.h, &self.u)
    }
}

/// IPA inner-product proof (encoded form for transport).
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct IpaProofData {
    /// Reserved for format evolution; set to 1.
    pub version: u16,
    /// L commitments per round, encoded as compressed curve points.
    pub l: Vec<[u8; 32]>,
    /// R commitments per round, encoded as compressed curve points.
    pub r: Vec<[u8; 32]>,
    /// Final reduced scalar for vector `a`.
    pub a_final: [u8; 32],
    /// Final reduced scalar for vector `b`.
    pub b_final: [u8; 32],
}

impl IpaProofData {
    /// Encode to bare Norito payload bytes (no outer header). Panics on IO error.
    pub fn encode_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let flags = norito::core::default_encode_flags();
        let _guard = norito::core::DecodeFlagsGuard::enter(flags);
        norito::core::NoritoSerialize::serialize(self, &mut out)
            .expect("IpaProofData Norito serialization failed");
        out
    }

    /// Decode from a bare Norito payload.
    pub fn decode_bytes(bytes: &[u8]) -> Result<Self, norito::Error> {
        let flags = norito::core::default_encode_flags();
        let _flags = norito::core::DecodeFlagsGuard::enter(flags);
        let archived = norito::core::archived_from_slice_unchecked::<Self>(bytes);
        let _ctx = norito::core::PayloadCtxGuard::enter(archived.bytes());
        <Self as norito::NoritoDeserialize>::try_deserialize(archived.as_ref())
    }
}

/// Public inputs and commitment for a single-point polynomial opening.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct PolyOpenPublic {
    /// Reserved for format evolution; set to 1.
    pub version: u16,
    /// Curve identifier.
    pub curve_id: u16,
    /// Vector length `n` (power of two).
    pub n: u32,
    /// Evaluation point z.
    pub z: [u8; 32],
    /// Claimed evaluation t = f(z).
    pub t: [u8; 32],
    /// Commitment to coefficients using G, compressed curve point bytes.
    pub p_g: [u8; 32],
}

impl PolyOpenPublic {
    /// Encode to bare Norito payload bytes (no outer header). Panics on IO error.
    pub fn encode_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let flags = norito::core::default_encode_flags();
        let _guard = norito::core::DecodeFlagsGuard::enter(flags);
        norito::core::NoritoSerialize::serialize(self, &mut out)
            .expect("PolyOpenPublic Norito serialization failed");
        out
    }

    /// Decode from a bare Norito payload.
    pub fn decode_bytes(bytes: &[u8]) -> Result<Self, norito::Error> {
        let flags = norito::core::default_encode_flags();
        let _flags = norito::core::DecodeFlagsGuard::enter(flags);
        let archived = norito::core::archived_from_slice_unchecked::<Self>(bytes);
        let _ctx = norito::core::PayloadCtxGuard::enter(archived.bytes());
        <Self as norito::NoritoDeserialize>::try_deserialize(archived.as_ref())
    }
}

fn decode_from_slice_via_cursor<T>(bytes: &[u8]) -> Result<(T, usize), norito::core::Error>
where
    T: norito::codec::Decode,
{
    let mut cursor = std::io::Cursor::new(bytes);
    let value = T::decode(&mut cursor)?;
    let used = cursor.position() as usize;
    Ok((value, used))
}

pub(crate) fn fingerprint_bytes(
    curve_id: u16,
    n: u32,
    g: &[[u8; 32]],
    h: &[[u8; 32]],
    u: &[u8; 32],
) -> [u8; 32] {
    let mut buf = Vec::with_capacity(4 + 4 + (g.len() + h.len() + 1) * 32);
    buf.extend_from_slice(&curve_id.to_le_bytes());
    buf.extend_from_slice(&n.to_le_bytes());
    for elem in g {
        buf.extend_from_slice(elem);
    }
    for elem in h {
        buf.extend_from_slice(elem);
    }
    buf.extend_from_slice(u);
    sha3_256(&buf)
}

impl<'a> norito::core::DecodeFromSlice<'a> for IpaParams {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        decode_from_slice_via_cursor(bytes)
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for PolyOpenPublic {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        decode_from_slice_via_cursor(bytes)
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for IpaProofData {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        decode_from_slice_via_cursor(bytes)
    }
}

/// Norito envelope holding all inputs for verifying a Halo2 IPA polynomial opening.
///
/// This is intended for use as the outer payload inside an IVM TLV of type
/// `NoritoBytes`. Nested fields use the stable wire types from this crate.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct OpenVerifyEnvelope {
    /// Transparent IPA parameters (deterministic generator vectors).
    pub params: IpaParams,
    /// Public inputs (evaluation point, value, and commitment).
    pub public: PolyOpenPublic,
    /// Inner-product opening proof.
    pub proof: IpaProofData,
    /// Transcript label (must match between prover and verifier).
    pub transcript_label: String,
    /// Optional verifying-key commitment used by the circuit.
    #[norito(default)]
    pub vk_commitment: Option<[u8; 32]>,
    /// Optional public-input schema hash for this circuit.
    #[norito(default)]
    pub public_inputs_schema_hash: Option<[u8; 32]>,
    /// Optional domain tag binding the proof to chain/backend/VK/manifest/namespace/syscall.
    #[norito(default)]
    pub domain_tag: Option<[u8; 32]>,
}
