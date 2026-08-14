use norito::json::{self, Error as JsonError, Value as NoritoJsonValue};
use sha2::{Digest, Sha256};
use crate::limits::preflight_json;
/// Convert a Norito JSON value into a canonicalised byte representation.
pub fn canonicalize_norito_bytes(value: &NoritoJsonValue) -> Result<Vec<u8>, JsonError> {
    let mut bytes = Vec::new();
    json::to_writer(&mut bytes, value)?;
    bytes.push(b'\n');
    Ok(bytes)
}
/// Canonicalise arbitrary JSON bytes and return both the canonical encoding and parsed value.
pub fn canonicalize_json_bytes(bytes: &[u8]) -> Result<(Vec<u8>, NoritoJsonValue), JsonError> {
    let value: NoritoJsonValue = json::from_slice(bytes)?;
    let canonical_bytes = canonicalize_norito_bytes(&value)?;
    Ok((canonical_bytes, value))
}
/// Canonicalise untrusted JSON inside explicit lexical, decode, and output-byte ceilings.
pub fn canonicalize_json_bytes_bounded(
    bytes: &[u8],
    max_bytes: usize,
    decode_limits: norito::DecodeLimits,
    label: &str,
) -> eyre::Result<(Vec<u8>, NoritoJsonValue)> {
    preflight_json(bytes, max_bytes, decode_limits, label)?;
    let value: NoritoJsonValue =
        norito::with_decode_limits_scope(decode_limits, || json::from_slice(bytes))?;
    let json_bytes = max_bytes
        .checked_sub(1)
        .ok_or_else(|| eyre::eyre!("{label} canonical byte limit must include a newline"))?;
    let text = json::to_json_bounded(&value, json_bytes)
        .map_err(|error| eyre::eyre!("failed to canonicalize {label} within bounds: {error}"))?;
    let mut canonical = text.into_bytes();
    canonical
        .try_reserve_exact(1)
        .map_err(|error| eyre::eyre!("failed to reserve {label} newline: {error}"))?;
    canonical.push(b'\n');
    Ok((canonical, value))
}
/// Compute a SHA-256 digest over the supplied payload.
pub fn sha256_digest(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}
/// Compute a SHA-256 digest with a domain separator prefix.
pub fn sha256_domain_digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    hasher.finalize().into()
}
