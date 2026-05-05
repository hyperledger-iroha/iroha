//! Internal helpers shared across governance CLI submodules.

use core::mem;

use crate::{CliOutputFormat, RunContext};
use eyre::{Result, eyre};
use iroha::client::Client;
use iroha_crypto::blake2::{Blake2b512, digest::Digest};
use norito::json::Value;

/// Print a JSON payload or a summary line, depending on CLI output mode.
pub fn print_with_summary<C: RunContext>(
    context: &mut C,
    summary: Option<String>,
    json: &Value,
) -> Result<()> {
    match context.output_format() {
        CliOutputFormat::Json => context.print_data(json)?,
        CliOutputFormat::Text => {
            if let Some(line) = summary {
                context.println(line)?;
            } else {
                context.print_data(json)?;
            }
        }
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

/// Compute the governance proposal id using the stable hash recipe.
pub(super) fn compute_proposal_id(
    contract_address: &iroha::data_model::smart_contract::ContractAddress,
    code_hash: &[u8; 32],
    abi_hash: &[u8; 32],
) -> [u8; 32] {
    let contract_address_literal = contract_address.as_ref();
    let contract_address_len = u32::try_from(contract_address_literal.len())
        .expect("contract address length must fit in u32 for hashing");
    let mut input = Vec::with_capacity(
        b"iroha:gov:proposal:v1|".len()
            + mem::size_of::<u32>()
            + contract_address_literal.len()
            + code_hash.len()
            + abi_hash.len(),
    );
    input.extend_from_slice(b"iroha:gov:proposal:v1|");
    input.extend_from_slice(&contract_address_len.to_le_bytes());
    input.extend_from_slice(contract_address_literal.as_bytes());
    input.extend_from_slice(code_hash);
    input.extend_from_slice(abi_hash);
    let digest = Blake2b512::digest(&input);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

pub(super) fn resolve_contract_address_target(
    client: &Client,
    contract_address: Option<&str>,
    contract_alias: Option<&str>,
) -> Result<iroha::data_model::smart_contract::ContractAddress> {
    match (contract_address, contract_alias) {
        (Some(_), Some(_)) => Err(eyre!(
            "exactly one of --contract-address or --contract-alias must be provided"
        )),
        (Some(contract_address), None) => contract_address
            .parse()
            .map_err(|err| eyre!("invalid --contract-address: {err}")),
        (None, Some(contract_alias)) => {
            let contract_alias: iroha::data_model::smart_contract::ContractAlias = contract_alias
                .parse()
                .map_err(|err| eyre!("invalid --contract-alias: {err}"))?;
            let response = client
                .post_contract_alias_resolve(&contract_alias)
                .map_err(|err| eyre!("failed to resolve contract alias `{contract_alias}`: {err}"))?;
            if response.status() != reqwest::StatusCode::OK {
                return Err(eyre!(
                    "contract alias resolve request failed with HTTP {}: {}",
                    response.status(),
                    std::str::from_utf8(response.body()).unwrap_or("")
                ));
            }
            let value: Value = norito::json::from_slice(response.body())
                .map_err(|err| eyre!("failed to decode contract alias response: {err}"))?;
            let resolved = value
                .get("contract_address")
                .and_then(Value::as_str)
                .ok_or_else(|| eyre!("contract alias response missing `contract_address`"))?;
            resolved
                .parse()
                .map_err(|err| eyre!("resolved contract address is invalid: {err}"))
        }
        (None, None) => Err(eyre!(
            "provide exactly one contract target via --contract-address or --contract-alias"
        )),
    }
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

        let contract_address: iroha::data_model::smart_contract::ContractAddress =
            "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7"
                .parse()
                .expect("contract address");
        let code = [0x11u8; 32];
        let abi = [0x22u8; 32];

        let mut input = Vec::new();
        input.extend_from_slice(b"iroha:gov:proposal:v1|");
        let contract_address_len = u32::try_from(contract_address.as_ref().len())
            .expect("contract address length must fit in u32 for hashing");
        input.extend_from_slice(&contract_address_len.to_le_bytes());
        input.extend_from_slice(contract_address.as_ref().as_bytes());
        input.extend_from_slice(&code);
        input.extend_from_slice(&abi);
        let digest = Blake2b512::digest(&input);
        let mut expected = [0u8; 32];
        expected.copy_from_slice(&digest[..32]);

        let candidate = compute_proposal_id(&contract_address, &code, &abi);
        assert_eq!(candidate, expected);
    }
}
