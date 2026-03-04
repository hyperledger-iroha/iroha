//! Moderation token helpers used by gateway clients.
//!
//! Tokens are opaque base64 blobs wrapping a Norito payload plus a keyed
//! BLAKE3 MAC. The payload binds the denylist/cache versions to the manifest
//! and optional chunk digest so callers can verify that block responses were
//! produced by the expected policy snapshot.

use std::fmt;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use blake3::Hasher;
use hex::{FromHex, FromHexError};
use norito::{NoritoDeserialize, NoritoSerialize, decode_from_bytes, to_bytes};

const MODERATION_TOKEN_VERSION_V1: u8 = 1;

/// Payload bound into a moderation proof token.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationTokenBodyV1 {
    /// Manifest identifier associated with the blocked request.
    pub manifest_id: [u8; 32],
    /// Optional chunk digest when the block applies to a specific chunk.
    #[norito(default)]
    pub chunk_digest: Option<[u8; 32]>,
    /// Denylist version advertised by the gateway.
    pub denylist_version: String,
    /// Cache version advertised by the gateway.
    pub cache_version: String,
}

impl ModerationTokenBodyV1 {
    /// Construct a payload from hex inputs.
    pub fn new(
        manifest_hex: &str,
        chunk_digest_hex: Option<&str>,
        denylist_version: &str,
        cache_version: &str,
    ) -> Result<Self, ModerationTokenError> {
        let manifest_id = decode_hex_array(manifest_hex, "manifest id")?;
        let chunk_digest = match chunk_digest_hex {
            Some(hex) => Some(decode_hex_array(hex, "chunk digest")?),
            None => None,
        };
        let denylist_version = trimmed_non_empty(denylist_version, "denylist version")?;
        let cache_version = trimmed_non_empty(cache_version, "cache version")?;
        Ok(Self {
            manifest_id,
            chunk_digest,
            denylist_version,
            cache_version,
        })
    }
}

/// Full moderation proof (payload + MAC).
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationTokenProofV1 {
    /// Explicit format version.
    pub version: u8,
    /// Bound payload.
    pub body: ModerationTokenBodyV1,
    /// Keyed MAC over the payload bytes.
    pub mac: [u8; 32],
}

impl ModerationTokenProofV1 {
    /// Verify the MAC using the supplied key material.
    pub fn verify(
        &self,
        key: &ModerationTokenKey,
    ) -> Result<ValidatedModerationProof, ModerationTokenError> {
        if self.version != MODERATION_TOKEN_VERSION_V1 {
            return Err(ModerationTokenError::UnsupportedVersion(self.version));
        }
        let expected = mac_for_body(key, &self.body)?;
        if constant_time_eq(&expected, &self.mac) {
            Ok(ValidatedModerationProof {
                body: self.body.clone(),
            })
        } else {
            Err(ModerationTokenError::MacMismatch)
        }
    }
}

/// Validated proof returned after MAC verification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedModerationProof {
    /// Payload that was signed by the gateway.
    pub body: ModerationTokenBodyV1,
}

/// Key material used to sign or verify moderation tokens.
#[derive(Clone)]
pub struct ModerationTokenKey([u8; 32]);

impl ModerationTokenKey {
    /// Build a key from raw bytes.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Parse a base64-encoded key.
    pub fn from_base64(value: &str) -> Result<Self, ModerationTokenError> {
        let trimmed = value.trim();
        let bytes = BASE64_STANDARD
            .decode(trimmed.as_bytes())
            .map_err(ModerationTokenError::InvalidBase64)?;
        let array = bytes
            .as_slice()
            .try_into()
            .map_err(|_| ModerationTokenError::InvalidKeyLength(bytes.len()))?;
        Ok(Self(array))
    }
}

impl fmt::Debug for ModerationTokenKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ModerationTokenKey(**hidden**)") // avoid leaking key material
    }
}

impl AsRef<[u8; 32]> for ModerationTokenKey {
    fn as_ref(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Encode a moderation token for the provided context and key.
pub fn encode_token(
    key: &ModerationTokenKey,
    manifest_hex: &str,
    chunk_digest_hex: Option<&str>,
    denylist_version: &str,
    cache_version: &str,
) -> Result<String, ModerationTokenError> {
    let body = ModerationTokenBodyV1::new(
        manifest_hex,
        chunk_digest_hex,
        denylist_version,
        cache_version,
    )?;
    let proof = ModerationTokenProofV1 {
        version: MODERATION_TOKEN_VERSION_V1,
        mac: mac_for_body(key, &body)?,
        body,
    };
    let bytes = to_bytes(&proof).map_err(ModerationTokenError::Encode)?;
    Ok(BASE64_STANDARD.encode(bytes))
}

/// Decode and verify a token observed on the wire.
pub fn decode_and_verify_token(
    token_b64: &str,
    key: &ModerationTokenKey,
) -> Result<ValidatedModerationProof, ModerationTokenError> {
    let raw = BASE64_STANDARD
        .decode(token_b64.trim().as_bytes())
        .map_err(ModerationTokenError::InvalidBase64)?;
    let proof: ModerationTokenProofV1 =
        decode_from_bytes(&raw).map_err(ModerationTokenError::Decode)?;
    proof.verify(key)
}

/// Verify a token against the expected manifest, chunk, and policy versions.
pub fn verify_token_for_context(
    token_b64: &str,
    key: &ModerationTokenKey,
    manifest_id: &[u8; 32],
    chunk_digest: Option<&[u8; 32]>,
    denylist_version: &str,
    cache_version: &str,
) -> Result<ValidatedModerationProof, ModerationTokenError> {
    let proof = decode_and_verify_token(token_b64, key)?;
    if &proof.body.manifest_id != manifest_id {
        return Err(ModerationTokenError::ContextMismatch("manifest id"));
    }
    if chunk_digest.is_some() && proof.body.chunk_digest.as_ref() != chunk_digest {
        return Err(ModerationTokenError::ContextMismatch("chunk digest"));
    }
    if proof.body.denylist_version != denylist_version.trim() {
        return Err(ModerationTokenError::ContextMismatch("denylist version"));
    }
    if proof.body.cache_version != cache_version.trim() {
        return Err(ModerationTokenError::ContextMismatch("cache version"));
    }
    Ok(proof)
}

fn mac_for_body(
    key: &ModerationTokenKey,
    body: &ModerationTokenBodyV1,
) -> Result<[u8; 32], ModerationTokenError> {
    let bytes = to_bytes(body).map_err(ModerationTokenError::Encode)?;
    let mut hasher = Hasher::new_keyed(key.as_ref());
    hasher.update(&bytes);
    Ok(*hasher.finalize().as_bytes())
}

fn decode_hex_array<const N: usize>(
    value: &str,
    label: &'static str,
) -> Result<[u8; N], ModerationTokenError> {
    let mut buf = [0u8; N];
    let trimmed = value.trim();
    let bytes: Vec<u8> =
        Vec::from_hex(trimmed).map_err(|err| ModerationTokenError::InvalidHex(label, err))?;
    if bytes.len() != N {
        return Err(ModerationTokenError::InvalidHexLength {
            label,
            expected: N,
            actual: bytes.len(),
        });
    }
    buf.copy_from_slice(&bytes);
    Ok(buf)
}

fn constant_time_eq(lhs: &[u8], rhs: &[u8]) -> bool {
    if lhs.len() != rhs.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (a, b) in lhs.iter().zip(rhs.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

fn trimmed_non_empty(value: &str, label: &'static str) -> Result<String, ModerationTokenError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ModerationTokenError::EmptyField(label));
    }
    Ok(trimmed.to_string())
}

/// Errors surfaced when decoding or verifying moderation proof tokens.
#[derive(Debug)]
pub enum ModerationTokenError {
    /// Token payload failed to decode from base64.
    InvalidBase64(base64::DecodeError),
    /// Token payload failed to decode from Norito.
    Decode(norito::Error),
    /// Token payload failed to encode during signing.
    Encode(norito::Error),
    /// Hex input does not represent valid bytes.
    InvalidHex(&'static str, FromHexError),
    /// Hex input has the wrong length.
    InvalidHexLength {
        label: &'static str,
        expected: usize,
        actual: usize,
    },
    /// Unsupported proof version.
    UnsupportedVersion(u8),
    /// MAC does not match the supplied key.
    MacMismatch,
    /// Field was unexpectedly empty.
    EmptyField(&'static str),
    /// Key did not have the expected length.
    InvalidKeyLength(usize),
    /// Token payload did not match the expected context.
    ContextMismatch(&'static str),
}

impl std::fmt::Display for ModerationTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBase64(err) => write!(f, "token is not valid base64: {err}"),
            Self::Decode(err) => write!(f, "token payload failed to decode: {err}"),
            Self::Encode(err) => write!(f, "token payload failed to encode: {err}"),
            Self::InvalidHex(label, err) => {
                write!(f, "{label} must be valid hex: {err}")
            }
            Self::InvalidHexLength {
                label,
                expected,
                actual,
            } => write!(f, "{label} must decode to {expected} bytes (got {actual})"),
            Self::UnsupportedVersion(version) => write!(
                f,
                "unsupported moderation token version {version}; expected {}",
                MODERATION_TOKEN_VERSION_V1
            ),
            Self::MacMismatch => write!(f, "token MAC did not verify"),
            Self::EmptyField(label) => write!(f, "{label} must not be empty"),
            Self::InvalidKeyLength(len) => {
                write!(f, "moderation token key must be 32 bytes (got {len})")
            }
            Self::ContextMismatch(label) => {
                write!(f, "token payload mismatch for expected {label}")
            }
        }
    }
}

impl std::error::Error for ModerationTokenError {}
