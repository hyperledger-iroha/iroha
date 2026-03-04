use eyre::{Result, WrapErr};
use hex::encode as hex_encode;
use iroha_data_model::soradns::ResolverDirectoryRecordV1;
use norito::json::{self, value};
use norito_derive::{JsonDeserialize, JsonSerialize};

use crate::canonical::{canonicalize_json_bytes, sha256_digest};

/// Parsed representation of `directory.json` emitted by the release tooling.
#[derive(Debug, Clone, JsonDeserialize)]
pub struct DirectoryListing {
    pub version: u32,
    pub created_at_ms: u64,
    pub rad_count: usize,
    pub merkle_root: String,
    #[norito(default)]
    pub previous_root: Option<String>,
    pub rad: Vec<DirectoryRadEntry>,
}

impl DirectoryListing {
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.rad.len()
    }
}

/// Single RAD entry inside `directory.json`.
#[derive(Debug, Clone, JsonDeserialize)]
pub struct DirectoryRadEntry {
    pub resolver_id: String,
    pub rad_sha256: String,
    pub leaf_hash: String,
    pub file: String,
}

/// Decode and canonicalise a directory listing JSON blob.
pub fn parse_directory_listing(bytes: &[u8]) -> Result<(DirectoryListing, [u8; 32])> {
    let (canonical_bytes, canonical_value) =
        canonicalize_json_bytes(bytes).wrap_err("failed to canonicalize directory.json")?;
    let listing: DirectoryListing = value::from_value(canonical_value)
        .wrap_err("failed to parse canonical directory.json via Norito")?;
    let digest = sha256_digest(&canonical_bytes);
    Ok((listing, digest))
}

/// Build the canonical signing payload used for directory record signatures.
pub fn signing_payload_bytes(record: &ResolverDirectoryRecordV1) -> Result<Vec<u8>, json::Error> {
    let (_, public_key_bytes) = record.builder_public_key.to_bytes();
    let payload = SigningPayload {
        record_version: record.record_version,
        created_at_ms: record.created_at_ms,
        rad_count: record.rad_count,
        root_hash_hex: hex_encode(record.root_hash),
        directory_json_sha256_hex: hex_encode(record.directory_json_sha256),
        previous_root_hex: record.previous_root.map(hex_encode),
        proof_manifest_cid: record.proof_manifest_cid.to_string(),
        builder_public_key_hex: hex_encode(public_key_bytes),
    };
    json::to_vec(&payload)
}

#[derive(Debug, JsonSerialize)]
struct SigningPayload {
    record_version: u16,
    created_at_ms: u64,
    rad_count: u32,
    root_hash_hex: String,
    directory_json_sha256_hex: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    previous_root_hex: Option<String>,
    proof_manifest_cid: String,
    builder_public_key_hex: String,
}
