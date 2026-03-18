use std::{
    collections::BTreeSet,
    fs, io,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use hex::FromHexError;
use iroha_crypto::soranet::token::{self, AdmissionToken, MintError, compute_issuer_fingerprint};
use norito::json::{self, Value};
use rand::{CryptoRng, RngCore};
use soranet_pq::MlDsaSuite;
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

/// Request parameters for minting an admission token.
#[derive(Debug, Clone)]
pub struct MintRequest<'a> {
    /// ML-DSA suite used for signing.
    pub suite: MlDsaSuite,
    /// Issuer public key bytes.
    pub issuer_public_key: &'a [u8],
    /// Issuer secret key bytes.
    pub issuer_secret_key: &'a [u8],
    /// Relay identifier bound to the token.
    pub relay_id: [u8; 32],
    /// Resume transcript hash bound to the token.
    pub transcript_hash: [u8; 32],
    /// Token activation time.
    pub issued_at: SystemTime,
    /// Token expiry time.
    pub expires_at: SystemTime,
    /// Token flags (reserved for future use).
    pub flags: u8,
}

/// Metadata derived from a decoded admission token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenMetadata {
    pub token_id: [u8; 32],
    pub issuer_fingerprint: [u8; 32],
    pub relay_id: [u8; 32],
    pub transcript_hash: [u8; 32],
    pub issued_at: SystemTime,
    pub expires_at: SystemTime,
    pub flags: u8,
    pub signature_len: usize,
}

impl TokenMetadata {
    /// Compute the configured TTL for this token.
    #[must_use]
    pub fn ttl(&self) -> Duration {
        match self.expires_at.duration_since(self.issued_at) {
            Ok(ttl) => ttl,
            Err(_) => Duration::ZERO,
        }
    }

    fn issued_at_iso(&self) -> String {
        OffsetDateTime::from(self.issued_at)
            .format(&Rfc3339)
            .expect("RFC3339 format")
    }

    fn expires_at_iso(&self) -> String {
        OffsetDateTime::from(self.expires_at)
            .format(&Rfc3339)
            .expect("RFC3339 format")
    }
}

/// Minted token bundle containing the raw token and derived metadata.
/// Minted token bundle containing the raw token and derived metadata.
#[derive(Debug, Clone)]
pub struct TokenBundle {
    pub token: AdmissionToken,
    pub metadata: TokenMetadata,
}

impl TokenBundle {
    /// Create a bundle from a freshly minted or decoded token.
    fn new(token: AdmissionToken) -> Self {
        let issued_at = UNIX_EPOCH + Duration::from_secs(token.issued_at());
        let expires_at = UNIX_EPOCH + Duration::from_secs(token.expires_at());
        let metadata = TokenMetadata {
            token_id: token.token_id(),
            issuer_fingerprint: *token.issuer_fingerprint(),
            relay_id: *token.relay_id(),
            transcript_hash: *token.transcript_hash(),
            issued_at,
            expires_at,
            flags: token.flags(),
            signature_len: token.signature().len(),
        };
        Self { token, metadata }
    }

    /// Serialise bundle details into a JSON value using Norito helpers.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let encoded = self.token.encode();
        let base64 = BASE64.encode(&encoded);
        let hex = hex::encode(&encoded);
        let ttl = self.metadata.ttl().as_secs();
        let mut object = json::Map::new();
        object.insert("token_base64".into(), Value::from(base64));
        object.insert("token_hex".into(), Value::from(hex));
        object.insert(
            "token_id_hex".into(),
            Value::from(hex::encode(self.metadata.token_id)),
        );
        object.insert(
            "issuer_fingerprint_hex".into(),
            Value::from(hex::encode(self.metadata.issuer_fingerprint)),
        );
        object.insert(
            "relay_id_hex".into(),
            Value::from(hex::encode(self.metadata.relay_id)),
        );
        object.insert(
            "transcript_hash_hex".into(),
            Value::from(hex::encode(self.metadata.transcript_hash)),
        );
        object.insert(
            "issued_at".into(),
            Value::from(self.metadata.issued_at_iso()),
        );
        object.insert(
            "expires_at".into(),
            Value::from(self.metadata.expires_at_iso()),
        );
        object.insert("ttl_secs".into(), Value::from(ttl));
        object.insert("flags".into(), Value::from(self.metadata.flags));
        object.insert(
            "signature_len".into(),
            Value::from(self.metadata.signature_len as u64),
        );
        Value::Object(object)
    }
}

/// Persistent revocation list backed by a JSON document.
#[derive(Debug, Clone, Default)]
pub struct RevocationList {
    entries: BTreeSet<[u8; 32]>,
}

impl RevocationList {
    /// Load a revocation list from disk. Missing files return an empty set.
    pub fn load_or_default(path: &Path) -> Result<Self, TokenToolError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = fs::read(path)?;
        if bytes.is_empty() {
            return Ok(Self::default());
        }
        let values: Vec<String> = json::from_slice(&bytes)?;
        let mut entries = BTreeSet::new();
        for (idx, value) in values.iter().enumerate() {
            let id = parse_hex_array::<32>(value, "revocation_list")?;
            if !entries.insert(id) {
                return Err(TokenToolError::DuplicateRevocation {
                    index: idx,
                    token_id_hex: value.clone(),
                });
            }
        }
        Ok(Self { entries })
    }

    /// Persist the revocation list to disk, creating parent directories if needed.
    pub fn write(&self, path: &Path) -> Result<(), TokenToolError> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        let strings: Vec<String> = self.entries.iter().map(hex::encode).collect();
        let array = norito::json::Value::Array(
            strings
                .into_iter()
                .map(norito::json::Value::String)
                .collect(),
        );
        let mut bytes = json::to_vec_pretty(&array)?;
        bytes.push(b'\n');
        fs::write(path, bytes)?;
        Ok(())
    }

    /// Insert a token identifier into the revocation list.
    pub fn insert(&mut self, token_id: [u8; 32]) -> bool {
        self.entries.insert(token_id)
    }

    /// Return the current entries sorted lexicographically.
    pub fn entries(&self) -> impl Iterator<Item = &[u8; 32]> {
        self.entries.iter()
    }
}

impl FromIterator<[u8; 32]> for RevocationList {
    fn from_iter<T: IntoIterator<Item = [u8; 32]>>(iter: T) -> Self {
        Self {
            entries: iter.into_iter().collect(),
        }
    }
}

/// Errors surfaced by token tooling helpers.
#[derive(Debug, Error)]
pub enum TokenToolError {
    #[error("invalid hex for {field}: {error}")]
    Hex {
        field: &'static str,
        #[source]
        error: FromHexError,
    },
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] json::Error),
    #[error("base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("RFC3339 parse error for {field}: {error}")]
    TimeParse {
        field: &'static str,
        #[source]
        error: time::error::Parse,
    },
    #[error("token mint error: {0}")]
    Mint(#[from] MintError),
    #[error("token decode error: {0}")]
    Decode(#[from] token::DecodeError),
    #[error("issued_at must be earlier than expires_at")]
    InvalidTemporalBounds,
    #[error("expected {expected} bytes for {field}, got {actual}")]
    InvalidLength {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("duplicate token id in revocation list at index {index}: {token_id_hex}")]
    DuplicateRevocation { index: usize, token_id_hex: String },
}

/// Mint a token bundle using the provided RNG.
pub fn mint_token<R: RngCore + CryptoRng>(
    request: &MintRequest<'_>,
    rng: &mut R,
) -> Result<TokenBundle, TokenToolError> {
    ensure_temporal_bounds(request.issued_at, request.expires_at)?;

    let fingerprint = compute_issuer_fingerprint(request.issuer_public_key);
    let token = AdmissionToken::mint(
        request.suite,
        request.issuer_secret_key,
        fingerprint,
        request.relay_id,
        request.transcript_hash,
        request.issued_at,
        request.expires_at,
        request.flags,
        rng,
    )?;
    Ok(TokenBundle::new(token))
}

/// Decode a token frame and collect metadata.
pub fn inspect_token(bytes: &[u8]) -> Result<TokenBundle, TokenToolError> {
    let token = AdmissionToken::decode(bytes)?;
    Ok(TokenBundle::new(token))
}

/// Decode a base64 or hexadecimal token string.
pub fn decode_token_string(input: &str) -> Result<Vec<u8>, TokenToolError> {
    let trimmed = input.trim();
    let is_hex_candidate = trimmed.len().is_multiple_of(2)
        && !trimmed.is_empty()
        && trimmed.chars().all(|c| c.is_ascii_hexdigit());
    if is_hex_candidate {
        let bytes = parse_hex_bytes(trimmed, "token_hex")?;
        return Ok(bytes);
    }
    let decoded = BASE64.decode(trimmed).map_err(TokenToolError::Base64)?;
    Ok(decoded)
}

/// Parse an RFC3339 timestamp into `SystemTime`.
pub fn parse_rfc3339(value: &str, field: &'static str) -> Result<SystemTime, TokenToolError> {
    let dt = OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|error| TokenToolError::TimeParse { field, error })?;
    Ok(SystemTime::from(dt))
}

/// Encode a token frame as base64.
#[must_use]
pub fn encode_token_base64(token: &AdmissionToken) -> String {
    BASE64.encode(token.encode())
}

/// Encode a token frame as hexadecimal.
#[must_use]
pub fn encode_token_hex(token: &AdmissionToken) -> String {
    hex::encode(token.encode())
}

/// Helper used by configuration parsing to load revocation IDs from disk.
pub fn read_revocation_file(path: &Path) -> Result<Vec<[u8; 32]>, TokenToolError> {
    let list = RevocationList::load_or_default(path)?;
    Ok(list.entries().cloned().collect())
}

fn ensure_temporal_bounds(start: SystemTime, end: SystemTime) -> Result<(), TokenToolError> {
    if end <= start {
        return Err(TokenToolError::InvalidTemporalBounds);
    }
    Ok(())
}

pub fn parse_hex_array<const N: usize>(
    value: &str,
    field: &'static str,
) -> Result<[u8; N], TokenToolError> {
    let bytes = hex::decode(value).map_err(|error| TokenToolError::Hex { field, error })?;
    if bytes.len() != N {
        return Err(TokenToolError::InvalidLength {
            field,
            expected: N,
            actual: bytes.len(),
        });
    }
    let mut array = [0u8; N];
    array.copy_from_slice(&bytes);
    Ok(array)
}

pub fn parse_hex_bytes(value: &str, field: &'static str) -> Result<Vec<u8>, TokenToolError> {
    hex::decode(value).map_err(|error| TokenToolError::Hex { field, error })
}

#[cfg(test)]
mod tests {
    use rand::{SeedableRng, rngs::StdRng};
    use soranet_pq::generate_mldsa_keypair;
    use tempfile::tempdir;

    use super::*;

    const RELAY_ID: [u8; 32] = [0x45; 32];
    const TRANSCRIPT: [u8; 32] = [0xAB; 32];

    #[test]
    fn mint_and_inspect_round_trip() {
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keypair");
        let issued_at = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        let expires_at = issued_at + Duration::from_secs(600);
        let mut rng = StdRng::seed_from_u64(0xDEADBEEF);
        let request = MintRequest {
            suite: MlDsaSuite::MlDsa44,
            issuer_public_key: keypair.public_key(),
            issuer_secret_key: keypair.secret_key(),
            relay_id: RELAY_ID,
            transcript_hash: TRANSCRIPT,
            issued_at,
            expires_at,
            flags: 0,
        };
        let bundle = mint_token(&request, &mut rng).expect("mint");
        assert_eq!(bundle.metadata.relay_id, RELAY_ID);
        assert_eq!(bundle.metadata.transcript_hash, TRANSCRIPT);
        assert_eq!(bundle.metadata.flags, 0);
        assert_eq!(bundle.metadata.ttl(), Duration::from_secs(600));

        let encoded = bundle.token.encode();
        let decoded = inspect_token(&encoded).expect("inspect");
        assert_eq!(bundle.metadata, decoded.metadata);
    }

    #[test]
    fn revocation_list_round_trip() {
        let dir = tempdir().expect("tmp");
        let path = dir.path().join("revocations.json");
        let mut list = RevocationList::default();
        list.insert([0x11; 32]);
        list.insert([0x22; 32]);
        list.write(&path).expect("write");

        let loaded = RevocationList::load_or_default(&path).expect("load");
        assert_eq!(
            loaded.entries().map(hex::encode).collect::<Vec<_>>(),
            vec![hex::encode([0x11; 32]), hex::encode([0x22; 32])]
        );
    }

    #[test]
    fn decode_token_string_accepts_hex() {
        let bytes = vec![0xAA, 0xBB, 0xCC];
        let hex = hex::encode(&bytes);
        let decoded = decode_token_string(&hex).expect("decode");
        assert_eq!(decoded, bytes);
    }

    #[test]
    fn decode_token_string_accepts_base64() {
        let bytes = vec![1u8, 2, 3, 4];
        let b64 = BASE64.encode(&bytes);
        let decoded = decode_token_string(&b64).expect("decode");
        assert_eq!(decoded, bytes);
    }
}
