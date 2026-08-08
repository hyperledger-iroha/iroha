//! One-shot, non-networked Exact12 action-construction driver.
//!
//! This boundary is designed for a sealed qualification controller to own
//! validator lifecycle, submission, direct peer queries, replay attempts, and
//! outcome validation.  This binary deliberately has no endpoint or credential
//! input.  Its sole v1 operation accepts one bounded public network context on
//! stdin and returns one genuine proof-bearing VeRange transaction on stdout.
//! Witness and signing material are derived and consumed inside this process
//! and never cross the IPC boundary.  Receipt issuance remains closed until
//! the controller has adopted this split for every Exact12 case.

use std::{
    env,
    io::{Read as _, Write as _},
    num::NonZeroU32,
    process::ExitCode,
    time::Duration,
};

use iroha_core::privacy_release_evidence::{
    PrivacyReleaseTransactionContextV1, build_privacy_release_verange_network_action_v1,
};
use iroha_crypto::{Algorithm, Hash, HashOf, PrivateKey, PublicKey};
use iroha_data_model::{
    block::BlockHeader,
    metadata::Metadata,
    prelude::{AccountId, AssetDefinitionId, ChainId, NetworkId},
    privacy::{PrivacyPolicyIdV1, TAIRA_PRIVACY_MAX_ACTION_BYTES_V1},
    transaction::FeePaymentIntent,
};
use sha2::{Digest as _, Sha256};
use zeroize::Zeroizing;

const REQUEST_SCHEMA: &str = "iroha.taira.privacy_action_driver_request";
const RESPONSE_SCHEMA: &str = "iroha.taira.privacy_action_driver_response";
const SCHEMA_VERSION: u8 = 1;
const OPERATION: &str = "build-verange-action-v1";
const REQUEST_ID_DOMAIN: &[u8] = b"iroha.taira.privacy_action_driver_request.v1\0";
const SEED_DOMAIN: &[u8] = b"iroha.taira.privacy_action_driver_seed.v1\0";
const MAX_REQUEST_BYTES: u64 = 16 * 1024;
const MAX_TRANSACTION_BYTES: usize = TAIRA_PRIVACY_MAX_ACTION_BYTES_V1 as usize;
const MAX_TTL_MILLIS: u64 = 2 * 60 * 60 * 1_000;
const MAX_CREATION_TIME_MILLIS: u64 = 9_223_372_036_854_775_807;
const MAX_ASSET_DEFINITION_ID_BYTES: usize = 1024;
const MAX_CHAIN_ID_BYTES: usize = 128;

#[derive(Debug, Clone, norito::JsonDeserialize, norito::JsonSerialize)]
#[norito(deny_unknown_fields)]
struct BuildVeRangeRequestV1 {
    asset_definition_id: String,
    candidate_binding_sha256: String,
    chain_id: String,
    creation_time_millis: u64,
    genesis_hash_hex: String,
    nonce: u32,
    operation: String,
    request_id: String,
    schema: String,
    schema_version: u8,
    ttl_millis: u64,
    values: Vec<u64>,
}

#[derive(Debug, Clone, norito::JsonSerialize)]
struct RequestIdBodyV1 {
    asset_definition_id: String,
    candidate_binding_sha256: String,
    chain_id: String,
    creation_time_millis: u64,
    genesis_hash_hex: String,
    nonce: u32,
    operation: String,
    schema: String,
    schema_version: u8,
    ttl_millis: u64,
    values: Vec<u64>,
}

#[derive(Debug, norito::JsonSerialize)]
struct BuildVeRangeResponseV1 {
    candidate_binding_sha256: String,
    operation: String,
    protocol: String,
    request_id: String,
    schema: String,
    schema_version: u8,
    transaction_hash_hex: String,
    transaction_norito_hex: String,
    transaction_sha256: String,
}

fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(sha256_bytes(bytes))
}

fn decode_hex_32(value: &str, label: &str, reject_zero: bool) -> Result<[u8; 32], String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{label} must be exactly 64 lowercase hexadecimal characters"
        ));
    }
    let decoded = hex::decode(value).map_err(|_| format!("{label} is not hexadecimal"))?;
    let bytes: [u8; 32] = decoded
        .try_into()
        .map_err(|_| format!("{label} does not decode to 32 bytes"))?;
    if reject_zero && bytes == [0; 32] {
        return Err(format!("{label} must be nonzero"));
    }
    Ok(bytes)
}

fn request_id_body(request: &BuildVeRangeRequestV1) -> RequestIdBodyV1 {
    RequestIdBodyV1 {
        asset_definition_id: request.asset_definition_id.clone(),
        candidate_binding_sha256: request.candidate_binding_sha256.clone(),
        chain_id: request.chain_id.clone(),
        creation_time_millis: request.creation_time_millis,
        genesis_hash_hex: request.genesis_hash_hex.clone(),
        nonce: request.nonce,
        operation: request.operation.clone(),
        schema: request.schema.clone(),
        schema_version: request.schema_version,
        ttl_millis: request.ttl_millis,
        values: request.values.clone(),
    }
}

fn compute_request_id(request: &BuildVeRangeRequestV1) -> Result<String, String> {
    let body = norito::json::to_string(&request_id_body(request))
        .map_err(|error| format!("cannot encode request ID body: {error}"))?;
    let mut hash = Sha256::new();
    hash.update(REQUEST_ID_DOMAIN);
    hash.update(body.as_bytes());
    Ok(hex::encode(hash.finalize()))
}

fn derive_nonzero_seed(candidate: &[u8; 32], request_id: &[u8; 32], purpose: u8) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(SEED_DOMAIN);
    hash.update(candidate);
    hash.update(request_id);
    hash.update([purpose]);
    let mut seed: [u8; 32] = hash.finalize().into();
    if seed == [0; 32] {
        seed[0] = 1;
    }
    seed
}

fn read_request() -> Result<(BuildVeRangeRequestV1, Vec<u8>), String> {
    if env::args_os().count() != 1 {
        return Err("the action driver accepts no command-line arguments".to_owned());
    }
    let mut input = Vec::new();
    std::io::stdin()
        .lock()
        .take(MAX_REQUEST_BYTES + 1)
        .read_to_end(&mut input)
        .map_err(|error| format!("cannot read action-driver request: {error}"))?;
    if input.is_empty() || input.len() as u64 > MAX_REQUEST_BYTES {
        return Err("action-driver request is empty or exceeds 16384 bytes".to_owned());
    }
    let request: BuildVeRangeRequestV1 = norito::json::from_slice(&input)
        .map_err(|error| format!("cannot decode action-driver request: {error}"))?;
    let canonical = norito::json::to_string(&request)
        .map_err(|error| format!("cannot re-encode action-driver request: {error}"))?
        + "\n";
    if canonical.as_bytes() != input {
        return Err("action-driver request is not the one canonical JSON encoding".to_owned());
    }
    Ok((request, input))
}

fn build_response(request: BuildVeRangeRequestV1) -> Result<BuildVeRangeResponseV1, String> {
    if request.schema != REQUEST_SCHEMA
        || request.schema_version != SCHEMA_VERSION
        || request.operation != OPERATION
    {
        return Err("action-driver request selects an unsupported contract".to_owned());
    }
    if request.creation_time_millis == 0
        || request.creation_time_millis > MAX_CREATION_TIME_MILLIS
        || request.ttl_millis == 0
        || request.ttl_millis > MAX_TTL_MILLIS
    {
        return Err("action-driver time fields are outside the v1 bounds".to_owned());
    }
    if request.asset_definition_id.is_empty()
        || !request.asset_definition_id.is_ascii()
        || request.asset_definition_id.len() > MAX_ASSET_DEFINITION_ID_BYTES
    {
        return Err("action-driver asset definition ID is not bounded ASCII".to_owned());
    }
    if request.chain_id.is_empty()
        || !request.chain_id.is_ascii()
        || request.chain_id.len() > MAX_CHAIN_ID_BYTES
    {
        return Err("action-driver chain ID is not bounded ASCII".to_owned());
    }
    let nonce = NonZeroU32::new(request.nonce)
        .ok_or_else(|| "action-driver nonce must be nonzero".to_owned())?;
    if request.values.is_empty()
        || request.values.len() > 8
        || request
            .values
            .iter()
            .any(|value| *value > u64::from(u32::MAX))
    {
        return Err("action-driver VeRange values violate the 1..=8 Bits32 bound".to_owned());
    }
    let candidate = decode_hex_32(&request.candidate_binding_sha256, "candidate binding", true)?;
    let genesis = decode_hex_32(&request.genesis_hash_hex, "genesis hash", true)?;
    let request_id = decode_hex_32(&request.request_id, "request ID", true)?;
    if compute_request_id(&request)? != request.request_id {
        return Err("action-driver request ID is not derived from the canonical body".to_owned());
    }

    let chain_id: ChainId = request
        .chain_id
        .parse()
        .map_err(|error| format!("invalid action-driver chain ID: {error}"))?;
    let asset_definition_id: AssetDefinitionId = request
        .asset_definition_id
        .parse()
        .map_err(|error| format!("invalid action-driver asset definition: {error}"))?;
    let signing_seed = Zeroizing::new(derive_nonzero_seed(&candidate, &request_id, 0));
    let fixture_seed = Zeroizing::new(derive_nonzero_seed(&candidate, &request_id, 1));
    let policy_seed = derive_nonzero_seed(&candidate, &request_id, 2);
    let private_key = PrivateKey::from_bytes(Algorithm::Ed25519, signing_seed.as_ref())
        .map_err(|error| format!("cannot derive action-driver signing key: {error}"))?;
    let authority = AccountId::new(PublicKey::from(private_key.clone()));
    let network_id = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed(genesis),
    ));
    let context = PrivacyReleaseTransactionContextV1 {
        network_id,
        chain_id,
        authority: authority.clone(),
        creation_time: Duration::from_millis(request.creation_time_millis),
        time_to_live: Some(Duration::from_millis(request.ttl_millis)),
        nonce: Some(nonce),
        fee_payment: FeePaymentIntent::authority(Vec::new(), None),
        metadata: Metadata::default(),
        genesis_hash: genesis,
    };
    let policy_id = PrivacyPolicyIdV1::new(policy_seed);
    let action = build_privacy_release_verange_network_action_v1(
        context,
        asset_definition_id,
        policy_id,
        request.values,
        *fixture_seed,
        &private_key,
    );
    let action =
        action.map_err(|error| format!("native VeRange action construction failed: {error:?}"))?;

    let transaction = norito::to_bytes(&action.transaction)
        .map_err(|error| format!("cannot encode proof-bearing transaction: {error}"))?;
    if transaction.is_empty() || transaction.len() > MAX_TRANSACTION_BYTES {
        return Err("encoded action-driver transaction violates its byte bound".to_owned());
    }
    let transaction_hash_hex = hex::encode(action.transaction.hash().as_ref());

    Ok(BuildVeRangeResponseV1 {
        candidate_binding_sha256: request.candidate_binding_sha256,
        operation: OPERATION.to_owned(),
        protocol: "verange-transparent-range-v1".to_owned(),
        request_id: request.request_id,
        schema: RESPONSE_SCHEMA.to_owned(),
        schema_version: SCHEMA_VERSION,
        transaction_hash_hex,
        transaction_norito_hex: hex::encode(&transaction),
        transaction_sha256: sha256_hex(&transaction),
    })
}

fn run() -> Result<(), String> {
    let (request, mut request_bytes) = read_request()?;
    let response = build_response(request);
    request_bytes.fill(0);
    let response = response?;
    let mut encoded = norito::json::to_string(&response)
        .map_err(|error| format!("cannot encode action-driver response: {error}"))?;
    encoded.push('\n');
    std::io::stdout()
        .lock()
        .write_all(encoded.as_bytes())
        .map_err(|error| format!("cannot write action-driver response: {error}"))?;
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("privacy Exact12 action driver refused: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REQUEST_ID_GOLDEN: &str =
        include_str!("../../../../fixtures/privacy_exact12_action_driver_request_id_v1.json");

    #[derive(Debug, norito::JsonDeserialize)]
    struct RequestIdGoldenV1 {
        canonical_request: String,
        canonical_request_id_body: String,
        request: BuildVeRangeRequestV1,
        request_id: String,
        schema: String,
        schema_version: u8,
    }

    #[test]
    fn python_and_rust_share_one_request_id_golden() {
        let golden: RequestIdGoldenV1 =
            norito::json::from_str(REQUEST_ID_GOLDEN).expect("decode request-ID golden");
        assert_eq!(
            golden.schema,
            "iroha.taira.privacy_action_driver_request_id_golden"
        );
        assert_eq!(golden.schema_version, 1);
        assert_eq!(golden.request.request_id, golden.request_id);
        assert_eq!(
            norito::json::to_string(&request_id_body(&golden.request))
                .expect("encode request-ID body"),
            golden.canonical_request_id_body
        );
        assert_eq!(
            norito::json::to_string(&golden.request).expect("encode full request") + "\n",
            golden.canonical_request
        );
        assert_eq!(
            compute_request_id(&golden.request).expect("derive request ID"),
            golden.request_id
        );
    }
}
