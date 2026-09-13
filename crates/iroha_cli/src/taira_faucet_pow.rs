//! Shared exact-network, deadline-bounded native Taira faucet proof-of-work solver.
use super::*;

pub(super) fn solve_account_faucet_claim(
    public_root: &str,
    account_id: &AccountId,
    expected_network_id: &NetworkId,
    deadline: Instant,
) -> Result<AccountFaucetClaimV1> {
    let http = http_client()?;
    let puzzle_url = join_url(public_root, "/v1/accounts/faucet/puzzle")?;
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
        puzzle,
        deadline,
    )?;
    json::from_value(claim).wrap_err("decode solved faucet claim into its closed V1 schema")
}

pub(super) fn solve_faucet_puzzle(
    account_id: &str,
    expected_network_id: &NetworkId,
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
    let network_id = validate_taira_puzzle_identity(puzzle, expected_network_id)?;
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
pub(super) const FAUCET_PUZZLE_V1_FIELDS: [&str; 11] = [
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
pub(super) fn validate_exact_faucet_puzzle_shape(puzzle: &Value) -> Result<()> {
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
pub(super) fn validate_taira_puzzle_identity(
    puzzle: &Value,
    expected_network_id: &NetworkId,
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
    if chain_discriminant != DEFAULT_CHAIN_DISCRIMINANT {
        eyre::bail!(
            "faucet puzzle chain_discriminant `{chain_discriminant}` does not match Taira `{DEFAULT_CHAIN_DISCRIMINANT}`"
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
pub(super) fn required_nullable_str<'a>(value: &'a Value, key: &str) -> Result<Option<&'a str>> {
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
pub(super) fn build_faucet_challenge(
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
pub(super) fn solve_faucet_pow(
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
pub(super) fn leading_zero_bits(bytes: &[u8]) -> u32 {
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
