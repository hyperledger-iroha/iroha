use crate::{
    canonical::{canonicalize_json_bytes_bounded, sha256_digest},
    limits::{
        MAX_DIRECTORY_JSON_BYTES, MAX_DIRECTORY_RECORD_BYTES, MAX_IDENTIFIER_BYTES,
        MAX_RAD_ENTRIES, directory_json_decode_limits,
    },
};
use eyre::{Result, WrapErr};
use hex::encode as hex_encode;
use iroha_data_model::soradns::ResolverDirectoryRecordV1;
use norito::json::{self, value};
use norito_derive::{JsonDeserialize, JsonSerialize};
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
    let decode_limits = directory_json_decode_limits();
    let (canonical_bytes, canonical_value) = canonicalize_json_bytes_bounded(
        bytes,
        MAX_DIRECTORY_JSON_BYTES,
        decode_limits,
        "directory.json",
    )
    .wrap_err("failed to canonicalize directory.json")?;
    let listing: DirectoryListing =
        norito::with_decode_limits_scope(decode_limits, || value::from_value(canonical_value))
            .wrap_err("failed to parse canonical directory.json via Norito")?;
    validate_listing_bounds(&listing)?;
    let digest = sha256_digest(&canonical_bytes);
    Ok((listing, digest))
}
fn validate_listing_bounds(listing: &DirectoryListing) -> Result<()> {
    validate_directory_entry_count(listing.rad.len())?;
    if listing.rad_count != listing.rad.len() {
        eyre::bail!(
            "directory.json rad_count declares {} entries but contains {}",
            listing.rad_count,
            listing.rad.len()
        );
    }
    check_hash("directory.json merkle_root", &listing.merkle_root)?;
    if let Some(previous) = &listing.previous_root {
        check_hash("directory.json previous_root", previous)?;
    }
    let mut retained = std::mem::size_of::<DirectoryListing>()
        .checked_add(
            listing
                .rad
                .capacity()
                .saturating_mul(std::mem::size_of::<DirectoryRadEntry>()),
        )
        .and_then(|bytes| bytes.checked_add(listing.merkle_root.capacity()))
        .and_then(|bytes| {
            bytes.checked_add(
                listing
                    .previous_root
                    .as_ref()
                    .map_or(0, |value| value.capacity()),
            )
        })
        .ok_or_else(|| eyre::eyre!("directory listing retained-byte accounting overflow"))?;
    for entry in &listing.rad {
        check_hash("directory RAD resolver_id", &entry.resolver_id)?;
        check_hash("directory RAD sha256", &entry.rad_sha256)?;
        check_hash("directory RAD leaf_hash", &entry.leaf_hash)?;
        if entry.file.len() > MAX_IDENTIFIER_BYTES {
            eyre::bail!(
                "directory RAD file contains {} bytes; the limit is {MAX_IDENTIFIER_BYTES}",
                entry.file.len()
            );
        }
        validate_rad_entry_file(entry)?;
        retained = retained
            .checked_add(entry.resolver_id.capacity())
            .and_then(|bytes| bytes.checked_add(entry.rad_sha256.capacity()))
            .and_then(|bytes| bytes.checked_add(entry.leaf_hash.capacity()))
            .and_then(|bytes| bytes.checked_add(entry.file.capacity()))
            .ok_or_else(|| eyre::eyre!("directory entry retained-byte accounting overflow"))?;
    }
    if retained > MAX_DIRECTORY_JSON_BYTES {
        eyre::bail!(
            "directory listing retains {retained} bytes; the limit is {MAX_DIRECTORY_JSON_BYTES}"
        );
    }
    Ok(())
}
fn validate_rad_entry_file(entry: &DirectoryRadEntry) -> Result<()> {
    if !entry
        .resolver_id
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        eyre::bail!("directory RAD resolver_id must use canonical lowercase hexadecimal");
    }
    let expected = format!("rad/{}.norito", entry.resolver_id);
    if entry.file != expected {
        eyre::bail!(
            "directory RAD file `{}` must equal the canonical path `{expected}`",
            entry.file
        );
    }
    Ok(())
}
fn validate_directory_entry_count(count: usize) -> Result<()> {
    if count > MAX_RAD_ENTRIES {
        eyre::bail!(
            "directory.json contains {} RAD entries; the limit is {MAX_RAD_ENTRIES}",
            count
        );
    }
    Ok(())
}
fn check_hash(label: &str, value: &str) -> Result<()> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        eyre::bail!("{label} must be exactly 64 hexadecimal characters");
    }
    Ok(())
}
/// Build the canonical signing payload used for directory record signatures.
pub fn signing_payload_bytes(record: &ResolverDirectoryRecordV1) -> Result<Vec<u8>> {
    let (_, public_key_bytes) = record
        .builder_public_key
        .try_to_bytes()
        .wrap_err("directory builder public key is malformed")?;
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
    json::to_json_bounded(&payload, MAX_DIRECTORY_RECORD_BYTES)
        .map(String::into_bytes)
        .wrap_err("failed to encode bounded directory signing payload")
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directory_entry_count_accepts_exact_and_rejects_plus_one() {
        validate_directory_entry_count(MAX_RAD_ENTRIES).expect("exact directory count");
        assert!(validate_directory_entry_count(MAX_RAD_ENTRIES + 1).is_err());
    }
    #[test]
    fn directory_rad_file_rejects_traversal_and_noncanonical_identity() {
        let resolver_id = "ab".repeat(32);
        let canonical = DirectoryRadEntry {
            resolver_id: resolver_id.clone(),
            rad_sha256: "cd".repeat(32),
            leaf_hash: "ef".repeat(32),
            file: format!("rad/{resolver_id}.norito"),
        };
        validate_rad_entry_file(&canonical).expect("canonical RAD path");
        let traversal = DirectoryRadEntry {
            file: "rad/../../etc/passwd".to_owned(),
            ..canonical.clone()
        };
        assert!(validate_rad_entry_file(&traversal).is_err());
        let uppercase = DirectoryRadEntry {
            resolver_id: "AB".repeat(32),
            file: format!("rad/{}.norito", "AB".repeat(32)),
            ..canonical
        };
        assert!(validate_rad_entry_file(&uppercase).is_err());
    }
}
