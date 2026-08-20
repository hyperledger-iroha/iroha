//! Parsers for the concise report emitted by `kagami verify`.

use super::{GenesisProfile, KagamiVerifyReport};

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
    profile: GenesisProfile,
    vrf_seed_hex: Option<&str>,
    output: &str,
) -> KagamiVerifyReport {
    let mut chain_id = None;
    let mut peers_with_pop = None;
    let mut fingerprint = None;
    let mut vrf_seed = vrf_seed_hex.map(|value| value.to_owned());
    for line in output.lines() {
        if chain_id.is_none() {
            chain_id = parse_keyed_value(line, &["chain_id", "chain id", "chain"]);
        }
        if peers_with_pop.is_none() {
            peers_with_pop = parse_keyed_value(
                line,
                &["peers_with_pop", "peers-with-pop", "pop_peers", "pop"],
            )
            .and_then(|value| value.parse().ok());
        }
        if fingerprint.is_none() {
            fingerprint = parse_keyed_value(line, &["fingerprint", "hash", "fingerprint_hex"]);
        }
        if vrf_seed.is_none() {
            vrf_seed = parse_keyed_value(line, &["vrf_seed_hex", "vrf-seed", "vrf_seed"]);
        }
    }
    KagamiVerifyReport {
        profile,
        chain_id,
        vrf_seed_hex: vrf_seed,
        peers_with_pop,
        fingerprint,
        raw_output: output.to_owned(),
    }
}
