//! Parsers for the concise report emitted by `kagami verify`.

use super::KagamiVerifyReport;

pub(super) fn parse_keyed_value(line: &str, keys: &[&str]) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    for key in keys {
        if let Some(start) = lower.find(key) {
            let remainder = &line[start + key.len()..];
            if let Some((_, value)) = remainder
                .split_once(':')
                .or_else(|| remainder.split_once('='))
            {
                let trimmed = value.trim().trim_matches([',', ';']);
                if !trimmed.is_empty() {
                    return Some(trimmed.to_owned());
                }
            }
        }
    }
    for token in line.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        if let Some((key, value)) = token.split_once(['=', ':'])
            && keys
                .iter()
                .any(|candidate| key.eq_ignore_ascii_case(candidate))
        {
            let trimmed = value.trim().trim_matches([',', ';']);
            if !trimmed.is_empty() {
                return Some(trimmed.to_owned());
            }
        }
    }
    None
}
pub(super) fn parse_kagami_verify_output(
    vrf_seed_hex: Option<&str>,
    output: &str,
) -> KagamiVerifyReport {
    let mut chain_id = None;
    let mut fingerprint = None;
    let mut vrf_seed = vrf_seed_hex.map(|value| value.to_owned());
    for line in output.lines() {
        if chain_id.is_none() {
            chain_id = parse_keyed_value(line, &["chain_id", "chain id", "chain"]);
        }
        if fingerprint.is_none() {
            fingerprint = parse_keyed_value(line, &["fingerprint", "hash", "fingerprint_hex"]);
        }
        if vrf_seed.is_none() {
            vrf_seed = parse_keyed_value(line, &["vrf_seed_hex", "vrf-seed", "vrf_seed"]);
        }
    }
    KagamiVerifyReport {
        chain_id,
        vrf_seed_hex: vrf_seed,
        fingerprint,
    }
}
