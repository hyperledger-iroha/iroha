use norito::json::{self, Error as JsonError, Value as NoritoJsonValue};
use sha2::{Digest, Sha256};

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
