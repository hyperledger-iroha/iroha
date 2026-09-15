//! Single exact-network, bounded native faucet proof-of-work implementation.
use eyre::{Result, WrapErr as _, eyre};
use iroha::{
    client::AccountFaucetClaimV1,
    data_model::{NetworkId, account::AccountId},
};
use norito::json::{self, Map, Value};
use scrypt::{Params as ScryptParams, scrypt as derive_scrypt};
use sha2::{Digest as _, Sha256};
use std::{
    io::Read as _,
    time::{Duration, Instant},
};
use url::Url;
const FAUCET_POW_ALGORITHM: &str = "scrypt-leading-zero-bits-v1";
const FAUCET_POW_DOMAIN_SEPARATOR: &[u8] = b"iroha:accounts:faucet:pow:v1";
struct HttpJson {
    status: u16,
    body: Option<Value>,
}
fn remaining_prepared_budget(deadline: Instant) -> Result<Duration> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "prepared action exhausted its original deadline",
        )
        .into());
    }
    Ok(remaining)
}

/// Fetch and solve the canonical faucet puzzle under an exact network, profile and deadline.
///
/// # Errors
/// Rejects malformed, foreign, oversized or excessive-work puzzles and transport/deadline failures.
pub fn solve_account_faucet_claim(
    public_root: &str,
    account_id: &AccountId,
    expected_network_id: &NetworkId,
    expected_chain_discriminant: u16,
    deadline: Instant,
) -> Result<AccountFaucetClaimV1> {
    let root = Url::parse(public_root)?;
    iroha::account_bootstrap::validate_endpoint(&root)?;
    let http = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let puzzle_url = root.join("/v1/accounts/faucet/puzzle")?;
    let response = http
        .get(puzzle_url)
        .header(reqwest::header::ACCEPT, "application/json")
        .timeout(Duration::from_secs(30).min(remaining_prepared_budget(deadline)?))
        .send()
        .wrap_err("faucet puzzle request failed")?;
    let status = response.status().as_u16();
    let puzzle = decode_puzzle_response(status, response)?;
    if puzzle.status != 200 {
        eyre::bail!(
            "faucet puzzle request failed with HTTP {}; no transaction was prepared",
            puzzle.status
        );
    }
    let puzzle = puzzle
        .body
        .as_ref()
        .ok_or_else(|| eyre!("faucet puzzle response was not canonical JSON"))?;
    let claim = solve_faucet_puzzle(
        &account_id.to_string(),
        expected_network_id,
        expected_chain_discriminant,
        puzzle,
        deadline,
    )?;
    json::from_value(claim).wrap_err("decode solved faucet claim into its closed V1 schema")
}

pub(crate) fn solve_faucet_puzzle(
    account_id: &str,
    expected_network_id: &NetworkId,
    expected_chain_discriminant: u16,
    puzzle: &Value,
    deadline: Instant,
) -> Result<Value> {
    validate_exact_faucet_puzzle_shape(puzzle)?;
    let algorithm = required_str(puzzle, "algorithm")?;
    if algorithm != FAUCET_POW_ALGORITHM {
        eyre::bail!(
            "unsupported faucet puzzle algorithm `{algorithm}`; expected `{FAUCET_POW_ALGORITHM}`"
        );
    }
    let network_id =
        validate_puzzle_identity(puzzle, expected_network_id, expected_chain_discriminant)?;
    let difficulty_bits = required_u64(puzzle, "difficulty_bits")?;
    if difficulty_bits == 0 {
        eyre::bail!("faucet puzzle difficulty_bits must be positive");
    }
    let mut body = Map::new();
    body.insert("account_id".into(), Value::String(account_id.to_owned()));
    let anchor_height = required_u64(puzzle, "anchor_height")?;
    if anchor_height == 0 {
        eyre::bail!("faucet puzzle anchor_height must be positive");
    }
    let anchor_hash_hex = required_str(puzzle, "anchor_block_hash_hex")?;
    let challenge_salt_hex = required_nullable_str(puzzle, "challenge_salt_hex")?;
    let log_n = u8::try_from(required_u64(puzzle, "scrypt_log_n")?)
        .map_err(|_| eyre!("faucet puzzle scrypt_log_n is too large"))?;
    let r = u32::try_from(required_u64(puzzle, "scrypt_r")?)
        .map_err(|_| eyre!("faucet puzzle scrypt_r is too large"))?;
    let p = u32::try_from(required_u64(puzzle, "scrypt_p")?)
        .map_err(|_| eyre!("faucet puzzle scrypt_p is too large"))?;
    if required_u64(puzzle, "max_anchor_age_blocks")? == 0 {
        eyre::bail!("faucet puzzle max_anchor_age_blocks must be positive");
    }
    let challenge = build_faucet_challenge(
        account_id,
        &network_id,
        anchor_height,
        anchor_hash_hex,
        challenge_salt_hex,
    )?;
    validate_work_budget(log_n, r, p, difficulty_bits)?;
    let params = ScryptParams::new(log_n, r, p, 32)
        .map_err(|err| eyre!("invalid faucet scrypt parameters: {err}"))?;
    let difficulty_bits =
        u32::try_from(difficulty_bits).map_err(|_| eyre!("faucet difficulty is too large"))?;
    let nonce = solve_faucet_pow(&challenge, &params, difficulty_bits, deadline)?;
    body.insert("pow_anchor_height".into(), Value::from(anchor_height));
    body.insert("pow_nonce_hex".into(), Value::String(hex::encode(nonce)));
    Ok(Value::Object(body))
}
pub(crate) const FAUCET_PUZZLE_V1_FIELDS: [&str; 11] = [
    "algorithm",
    "network_id",
    "chain_discriminant",
    "difficulty_bits",
    "anchor_height",
    "anchor_block_hash_hex",
    "challenge_salt_hex",
    "scrypt_log_n",
    "scrypt_r",
    "scrypt_p",
    "max_anchor_age_blocks",
];
pub(crate) fn validate_exact_faucet_puzzle_shape(puzzle: &Value) -> Result<()> {
    let object = puzzle
        .as_object()
        .ok_or_else(|| eyre!("faucet puzzle response must be an exact V1 object"))?;
    if object.len() != FAUCET_PUZZLE_V1_FIELDS.len()
        || FAUCET_PUZZLE_V1_FIELDS
            .iter()
            .any(|field| !object.contains_key(*field))
    {
        eyre::bail!("faucet puzzle response violates the exact V1 field set");
    }
    Ok(())
}
/// Validate a public puzzle's exact network and address profile before performing work.
///
/// # Errors
/// Rejects missing, noncanonical or mismatched identity fields.
pub fn validate_puzzle_identity(
    puzzle: &Value,
    expected_network_id: &NetworkId,
    expected_chain_discriminant: u16,
) -> Result<NetworkId> {
    let network_id_literal = required_str(puzzle, "network_id")?;
    let network_id = network_id_literal
        .parse::<NetworkId>()
        .wrap_err("faucet puzzle network_id is not a canonical NetworkId")?;
    if network_id.to_string() != network_id_literal {
        eyre::bail!("faucet puzzle network_id is not canonically encoded");
    }
    if &network_id != expected_network_id {
        eyre::bail!(
            "faucet puzzle network_id `{network_id}` does not match configured network `{expected_network_id}`"
        );
    }
    let chain_discriminant = u16::try_from(required_u64(puzzle, "chain_discriminant")?)
        .map_err(|_| eyre!("faucet puzzle chain_discriminant is too large"))?;
    if chain_discriminant != expected_chain_discriminant {
        eyre::bail!(
            "faucet puzzle chain_discriminant `{chain_discriminant}` does not match configured profile `{expected_chain_discriminant}`"
        );
    }
    Ok(network_id)
}
fn required_u64(value: &Value, key: &str) -> Result<u64> {
    value
        .as_object()
        .and_then(|obj| obj.get(key))
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre!("faucet puzzle missing numeric `{key}`"))
}
fn required_str<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .as_object()
        .and_then(|obj| obj.get(key))
        .and_then(Value::as_str)
        .ok_or_else(|| eyre!("faucet puzzle missing string `{key}`"))
}
pub(crate) fn required_nullable_str<'a>(value: &'a Value, key: &str) -> Result<Option<&'a str>> {
    let field = value
        .as_object()
        .and_then(|obj| obj.get(key))
        .ok_or_else(|| eyre!("faucet puzzle missing nullable string `{key}`"))?;
    if field.is_null() {
        return Ok(None);
    }
    field
        .as_str()
        .map(Some)
        .ok_or_else(|| eyre!("faucet puzzle `{key}` must be a string or null"))
}
pub(crate) fn build_faucet_challenge(
    account_id: &str,
    network_id: &NetworkId,
    anchor_height: u64,
    anchor_hash_hex: &str,
    challenge_salt_hex: Option<&str>,
) -> Result<[u8; 32]> {
    // This Torii field is explicitly raw lowercase hex, not the marked `Hash` display form.
    let anchor_hash = decode_exact_lower_hex(anchor_hash_hex, "anchor_block_hash_hex", 32)?;
    let mut hasher = Sha256::new();
    hasher.update(FAUCET_POW_DOMAIN_SEPARATOR);
    hasher.update(network_id.as_bytes());
    hasher.update(account_id.as_bytes());
    hasher.update(anchor_height.to_be_bytes());
    hasher.update(anchor_hash);
    if let Some(salt) = challenge_salt_hex {
        let salt = decode_exact_lower_hex(salt, "challenge_salt_hex", 32)?;
        hasher.update(salt);
    }
    Ok(hasher.finalize().into())
}
fn decode_exact_lower_hex(value: &str, field: &str, byte_length: usize) -> Result<Vec<u8>> {
    if value.len() != byte_length.saturating_mul(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        eyre::bail!(
            "faucet puzzle {field} must be an exact lowercase {byte_length}-byte hex string"
        );
    }
    hex::decode(value).wrap_err_with(|| format!("invalid faucet puzzle {field}"))
}
pub(crate) fn solve_faucet_pow(
    challenge: &[u8; 32],
    params: &ScryptParams,
    difficulty_bits: u32,
    deadline: Instant,
) -> Result<[u8; 8]> {
    for nonce in 0_u64..(1_u64 << 63) {
        remaining_prepared_budget(deadline)?;
        let nonce_bytes = nonce.to_be_bytes();
        let mut digest = [0_u8; 32];
        derive_scrypt(&nonce_bytes, challenge, params, &mut digest)
            .map_err(|err| eyre!("failed faucet scrypt derivation: {err}"))?;
        remaining_prepared_budget(deadline)?;
        if leading_zero_bits(&digest) >= difficulty_bits {
            return Ok(nonce_bytes);
        }
    }
    eyre::bail!("faucet PoW nonce space exhausted")
}
pub(crate) fn leading_zero_bits(bytes: &[u8]) -> u32 {
    let mut total = 0_u32;
    for byte in bytes {
        if *byte == 0 {
            total += 8;
            continue;
        }
        total += byte.leading_zeros();
        break;
    }
    total
}

// These are defensive client resource ceilings, independent of operator-selected puzzle difficulty.
const MAX_PUZZLE_RESPONSE_BYTES: usize = 4096;
const MAX_SCRYPT_MEMORY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_SCRYPT_WORK_UNITS: u64 = 4 * 1024 * 1024;

fn decode_puzzle_response(status: u16, reader: impl std::io::Read) -> Result<HttpJson> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_PUZZLE_RESPONSE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_PUZZLE_RESPONSE_BYTES {
        eyre::bail!("faucet puzzle response exceeds its bounded size");
    }
    let body = json::from_slice(&bytes).wrap_err("faucet puzzle response is not canonical JSON")?;
    Ok(HttpJson {
        status,
        body: Some(body),
    })
}

fn validate_work_budget(log_n: u8, r: u32, p: u32, difficulty: u64) -> Result<()> {
    let n = 1_u64.checked_shl(u32::from(log_n));
    let memory = n
        .and_then(|n| n.checked_add(u64::from(p) + 2))
        .and_then(|blocks| blocks.checked_mul(128 * u64::from(r)));
    let work = n
        .and_then(|n| n.checked_mul(u64::from(r)))
        .and_then(|work| work.checked_mul(u64::from(p)));
    if log_n == 0
        || r == 0
        || p == 0
        || !(1..=256).contains(&difficulty)
        || memory.is_none_or(|bytes| bytes > MAX_SCRYPT_MEMORY_BYTES)
        || work.is_none_or(|units| units > MAX_SCRYPT_WORK_UNITS)
    {
        eyre::bail!("faucet puzzle exceeds the native client's bounded proof-of-work budget");
    }
    Ok(())
}

#[cfg(test)]
mod resource_tests {
    use super::*;

    #[test]
    fn faucet_preparation_deadline_stops_cpu_work_before_dispatch() {
        let expired = Instant::now();
        let params = ScryptParams::new(1, 1, 1, 32).unwrap();
        let error = solve_faucet_pow(&[0; 32], &params, 1, expired).unwrap_err();
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::TimedOut
        );
    }

    #[test]
    fn faucet_puzzle_response_has_a_fixed_read_bound() {
        assert!(decode_puzzle_response(200, b"{}".as_slice()).is_ok());
        assert!(
            decode_puzzle_response(200, vec![b' '; MAX_PUZZLE_RESPONSE_BYTES + 1].as_slice())
                .is_err()
        );
        assert!(decode_puzzle_response(200, b"not json".as_slice()).is_err());
    }

    #[test]
    fn faucet_work_budget_accepts_normal_cost_and_rejects_hostile_parameters() {
        validate_work_budget(13, 8, 1, 4).unwrap();
        for (log_n, r, p, difficulty) in [
            (63, 8, 1, 4),
            (13, u32::MAX, 1, 4),
            (13, 8, u32::MAX, 4),
            (13, 8, 1, 257),
            (0, 1, 1, 1),
            (13, 0, 1, 4),
            (13, 8, 0, 4),
        ] {
            assert!(validate_work_budget(log_n, r, p, difficulty).is_err());
        }
    }
}

#[cfg(test)]
mod migrated_tests {
    use super::*;
    use iroha_crypto::Hash;
    const DEFAULT_CHAIN_DISCRIMINANT: u16 = 369;
    #[test]
    fn leading_zero_bits_counts_prefix() {
        assert_eq!(leading_zero_bits(&[0x00, 0x0f]), 12);
        assert_eq!(leading_zero_bits(&[0x80]), 0);
        assert_eq!(leading_zero_bits(&[0x40]), 1);
    }
    fn faucet_puzzle_fixture(network_id: &NetworkId) -> Value {
        norito::json!({
            "algorithm": FAUCET_POW_ALGORITHM,
            "network_id": (network_id.to_string()),
            "chain_discriminant": DEFAULT_CHAIN_DISCRIMINANT,
            "difficulty_bits": 1,
            "anchor_height": 7,
            "anchor_block_hash_hex": ("11".repeat(32)),
            "challenge_salt_hex": null,
            "scrypt_log_n": 1,
            "scrypt_r": 1,
            "scrypt_p": 1,
            "max_anchor_age_blocks": 16
        })
    }
    #[test]
    fn faucet_challenge_matches_python_fixture_shape() {
        let network_id = crate::operations::tests::fixture_config().network_id;
        let challenge = build_faucet_challenge(
            "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            &network_id,
            7,
            &"11".repeat(32),
            Some(&"22".repeat(32)),
        )
        .expect("challenge");
        assert_eq!(challenge.len(), 32);
        assert_ne!(challenge, [0_u8; 32]);
    }

    #[test]
    fn faucet_challenge_rejects_noncanonical_anchor_hash_hex() {
        let network_id = crate::operations::tests::fixture_config().network_id;
        let _error = build_faucet_challenge(
            "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            &network_id,
            7,
            &"AA".repeat(32),
            Some(&"22".repeat(32)),
        )
        .expect_err("uppercase anchor hash must fail before proof-of-work");
    }
    #[test]
    fn faucet_challenge_matches_v1_preimage_vector() {
        let genesis_hash =
            hex::decode("32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149")
                .expect("decode fixture genesis hash")
                .try_into()
                .expect("fixture genesis hash is exactly 32 bytes");
        let network_id = NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(Hash::prehashed(genesis_hash)),
        );
        let challenge = build_faucet_challenge(
            "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53",
            &network_id,
            68,
            "d5c0016a6345e8ea379da42aab1fdc16ba82756e19e0b63c48c14735e8caf7ef",
            None,
        )
        .expect("V1 faucet challenge");
        assert_eq!(
            hex::encode(challenge),
            "21e547302359214b28f0d1e0b04b6aeaf62a0e597dbad018d93ab0ce6af81a05"
        );
    }
    #[test]
    fn solve_faucet_puzzle_rejects_pre_release_algorithm_label() {
        let network_id = crate::operations::tests::fixture_config().network_id;
        let mut puzzle = faucet_puzzle_fixture(&network_id);
        puzzle.as_object_mut().expect("puzzle object").insert(
            "algorithm".to_owned(),
            Value::from("scrypt-leading-zero-bits-v2"),
        );
        let error = solve_faucet_puzzle(
            "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            &network_id,
            DEFAULT_CHAIN_DISCRIMINANT,
            &puzzle,
            Instant::now() + Duration::from_secs(5),
        )
        .expect_err("pre-release faucet algorithm must fail closed");
        let message = format!("{error:#}");
        assert!(message.contains("scrypt-leading-zero-bits-v2"));
        assert!(message.contains(FAUCET_POW_ALGORITHM));
    }
    #[test]
    fn faucet_challenge_rejects_same_label_different_genesis_replay() {
        let first_network = crate::operations::tests::fixture_config().network_id;
        let second_network =
            NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
                iroha_crypto::Hash::new(b"foreign-faucet-genesis"),
            ));
        let account_id = "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
        let first = build_faucet_challenge(
            account_id,
            &first_network,
            7,
            &"11".repeat(32),
            Some(&"22".repeat(32)),
        )
        .expect("first challenge");
        let second = build_faucet_challenge(
            account_id,
            &second_network,
            7,
            &"11".repeat(32),
            Some(&"22".repeat(32)),
        )
        .expect("second challenge");
        assert_ne!(first, second);
    }
    #[test]
    fn solve_faucet_puzzle_rejects_zero_difficulty() {
        let network_id = crate::operations::tests::fixture_config().network_id;
        let mut puzzle = faucet_puzzle_fixture(&network_id);
        puzzle
            .as_object_mut()
            .expect("puzzle object")
            .insert("difficulty_bits".to_owned(), Value::from(0_u64));
        let error = solve_faucet_puzzle(
            "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            &network_id,
            DEFAULT_CHAIN_DISCRIMINANT,
            &puzzle,
            Instant::now() + Duration::from_secs(5),
        )
        .expect_err("zero-difficulty faucet puzzle must fail closed");
        assert!(format!("{error:#}").contains("difficulty_bits must be positive"));
    }
    #[test]
    fn solve_faucet_puzzle_requires_the_exact_v1_field_set() {
        let network_id = crate::operations::tests::fixture_config().network_id;
        let canonical = faucet_puzzle_fixture(&network_id);
        validate_exact_faucet_puzzle_shape(&canonical).expect("exact V1 puzzle field set");
        assert_eq!(
            required_nullable_str(&canonical, "challenge_salt_hex")
                .expect("explicit nullable salt"),
            None
        );

        for field in FAUCET_PUZZLE_V1_FIELDS {
            let mut missing = canonical.clone();
            missing
                .as_object_mut()
                .expect("puzzle object")
                .remove(field);
            let error = solve_faucet_puzzle(
                "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                &network_id,
                DEFAULT_CHAIN_DISCRIMINANT,
                &missing,
                Instant::now() + Duration::from_secs(5),
            )
            .expect_err("omitted exact puzzle field must fail closed");
            assert!(format!("{error:#}").contains("exact V1 field set"));
        }

        let mut unknown = canonical.clone();
        unknown
            .as_object_mut()
            .expect("puzzle object")
            .insert("legacy_salt".to_owned(), Value::Null);
        assert!(
            solve_faucet_puzzle(
                "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                &network_id,
                DEFAULT_CHAIN_DISCRIMINANT,
                &unknown,
                Instant::now() + Duration::from_secs(5),
            )
            .is_err(),
            "unknown puzzle fields must fail closed"
        );

        let mut malformed = canonical;
        malformed
            .as_object_mut()
            .expect("puzzle object")
            .insert("challenge_salt_hex".to_owned(), Value::from(false));
        assert!(required_nullable_str(&malformed, "challenge_salt_hex").is_err());
    }
    #[test]
    fn taira_puzzle_identity_requires_the_exact_network_and_discriminant() {
        let network_id = crate::operations::tests::fixture_config().network_id;
        let canonical = norito::json!({
            "network_id": (network_id.to_string()),
            "chain_discriminant": DEFAULT_CHAIN_DISCRIMINANT,
        });
        assert_eq!(
            validate_puzzle_identity(&canonical, &network_id, DEFAULT_CHAIN_DISCRIMINANT)
                .expect("canonical Taira puzzle identity"),
            network_id
        );

        let foreign_network =
            NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
                iroha_crypto::Hash::new(b"foreign-taira-genesis"),
            ));
        let foreign = norito::json!({
            "network_id": (foreign_network.to_string()),
            "chain_discriminant": DEFAULT_CHAIN_DISCRIMINANT,
        });
        let network_error =
            validate_puzzle_identity(&foreign, &network_id, DEFAULT_CHAIN_DISCRIMINANT)
                .expect_err("a foreign network identity must fail before publication");
        assert!(format!("{network_error:#}").contains("does not match configured network"));

        let wrong_discriminant = norito::json!({
            "network_id": (network_id.to_string()),
            "chain_discriminant": (DEFAULT_CHAIN_DISCRIMINANT + 1),
        });
        let discriminant_error =
            validate_puzzle_identity(&wrong_discriminant, &network_id, DEFAULT_CHAIN_DISCRIMINANT)
                .expect_err("a foreign chain discriminant must fail before publication");
        assert!(format!("{discriminant_error:#}").contains("does not match configured profile"));
    }
}
