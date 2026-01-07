//! Internal helpers shared across governance CLI submodules.

use core::mem;

use crate::RunContext;
use eyre::{Result, eyre};
use iroha_crypto::blake2::{Blake2b512, digest::Digest};
use norito::json::Value;

/// Print a JSON payload, optionally prefixed with a summary line.
pub fn print_with_summary<C: RunContext>(
    context: &mut C,
    summary: Option<String>,
    json: &Value,
    summary_only: bool,
    no_summary: bool,
) -> Result<()> {
    validate_summary_flags(summary_only, no_summary)?;
    if !no_summary && let Some(line) = summary {
        let _ = context.println(line);
    }
    if !summary_only {
        context.print_data(json)?;
    }
    Ok(())
}

/// Normalize a 32-byte hex string.
pub(super) fn canonicalize_hex32(input: &str) -> Result<String> {
    let trimmed = input.trim();
    let without_scheme = if let Some((scheme, rest)) = trimmed.split_once(':') {
        if scheme.is_empty() || scheme.eq_ignore_ascii_case("blake2b32") {
            rest
        } else {
            return Err(eyre!("expected 32-byte hex string"));
        }
    } else {
        trimmed
    };
    let body = without_scheme.trim();
    let body = body
        .strip_prefix("0x")
        .or_else(|| body.strip_prefix("0X"))
        .unwrap_or(body)
        .trim();
    if body.len() != 64 || !body.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(eyre!("expected 32-byte hex string"));
    }
    Ok(body.to_ascii_lowercase())
}

/// Decode a canonicalized 32-byte hex string into bytes.
pub(super) fn decode_hex32(hex_str: &str) -> Result<[u8; 32]> {
    let canonical = canonicalize_hex32(hex_str)?;
    let mut out = [0u8; 32];
    hex::decode_to_slice(&canonical, &mut out)?;
    Ok(out)
}

/// Validate that mutually-exclusive summary flags are not both set.
pub(super) fn validate_summary_flags(summary_only: bool, no_summary: bool) -> Result<()> {
    if summary_only && no_summary {
        return Err(eyre!(
            "--summary-only and --no-summary cannot be used together"
        ));
    }
    Ok(())
}

/// Compute the governance proposal id using the stable hash recipe.
pub(super) fn compute_proposal_id(
    namespace: &str,
    contract_id: &str,
    code_hash: &[u8; 32],
    abi_hash: &[u8; 32],
) -> [u8; 32] {
    let namespace_len =
        u32::try_from(namespace.len()).expect("namespace length must fit in u32 for hashing");
    let contract_len =
        u32::try_from(contract_id.len()).expect("contract id length must fit in u32 for hashing");
    let mut input = Vec::with_capacity(
        b"iroha:gov:proposal:v1|".len()
            + mem::size_of::<u32>() * 2
            + namespace.len()
            + contract_id.len()
            + code_hash.len()
            + abi_hash.len(),
    );
    input.extend_from_slice(b"iroha:gov:proposal:v1|");
    input.extend_from_slice(&namespace_len.to_le_bytes());
    input.extend_from_slice(namespace.as_bytes());
    input.extend_from_slice(&contract_len.to_le_bytes());
    input.extend_from_slice(contract_id.as_bytes());
    input.extend_from_slice(code_hash);
    input.extend_from_slice(abi_hash);
    let digest = Blake2b512::digest(&input);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalize_hex32_strips_prefixes() {
        let raw = "0xAaBb".to_string() + &"Cc".repeat(30);
        let canon = canonicalize_hex32(&raw).expect("canonicalize");
        assert_eq!(canon.len(), 64);
        assert!(
            canon
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        );

        let scheme = "BLAKE2B32:11".to_string() + &"22".repeat(31);
        let canon2 = canonicalize_hex32(&scheme).expect("canonicalize with scheme");
        assert_eq!(canon2.len(), 64);
    }

    #[test]
    fn decode_hex32_roundtrip() {
        let hex_str = "0x".to_string() + &"ab".repeat(32);
        let bytes = decode_hex32(&hex_str).expect("decode");
        assert_eq!(hex::encode(bytes), "ab".repeat(32));
    }

    #[test]
    fn compute_proposal_id_matches_reference_logic() {
        use iroha_crypto::blake2::{Blake2b512, digest::Digest as _};

        let ns = "apps";
        let cid = "calc.v1";
        let code = [0x11u8; 32];
        let abi = [0x22u8; 32];

        let mut input = Vec::new();
        input.extend_from_slice(b"iroha:gov:proposal:v1|");
        let ns_len = u32::try_from(ns.len()).expect("namespace length must fit in u32 for hashing");
        input.extend_from_slice(&ns_len.to_le_bytes());
        input.extend_from_slice(ns.as_bytes());
        let cid_len =
            u32::try_from(cid.len()).expect("contract id length must fit in u32 for hashing");
        input.extend_from_slice(&cid_len.to_le_bytes());
        input.extend_from_slice(cid.as_bytes());
        input.extend_from_slice(&code);
        input.extend_from_slice(&abi);
        let digest = Blake2b512::digest(&input);
        let mut expected = [0u8; 32];
        expected.copy_from_slice(&digest[..32]);

        let candidate = compute_proposal_id(ns, cid, &code, &abi);
        assert_eq!(candidate, expected);
    }

    #[test]
    fn validate_summary_flags_detects_conflict() {
        let err = validate_summary_flags(true, true).unwrap_err();
        assert!(
            err.to_string()
                .contains("--summary-only and --no-summary cannot be used together"),
            "unexpected error message: {err}"
        );
        assert!(validate_summary_flags(true, false).is_ok());
        assert!(validate_summary_flags(false, true).is_ok());
    }
}
