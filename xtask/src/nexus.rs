use std::{
    collections::{BTreeSet, HashMap},
    error::Error,
    fmt::Write,
    fs,
    io::{self, Write as _},
    num::NonZeroU32,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use arrow_array::{
    ArrayRef, BooleanArray, Float64Array, RecordBatch, StringArray, UInt32Array, UInt64Array,
};
use arrow_schema::{DataType, Field, Schema};
use hex::decode;
use iroha::nexus_app::{
    NexusAppClient, NexusAppConfig, NexusAppError, NexusFinalizeOptions, NexusSignatureAlgorithm,
    NexusToriiSubmitter, NexusTransferInput, NexusTransferReceipt, NexusWalletSignature,
    UnsupportedConnectTransport,
};
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::{
    account::{AccountId, address::ChainDiscriminantGuard},
    asset::{AssetDefinitionId, AssetId},
    block::consensus::{
        LaneBlockCommitment, LaneLiquidityProfile, LaneSettlementReceipt, LaneSwapMetadata,
        LaneVolatilityClass,
    },
    name::Name,
    nexus::{DataSpaceId, LaneCompliancePolicy, LaneId},
    prelude::{Metadata, NetworkId},
    transaction::{FeePaymentIntent, SignedTransaction, TransactionPayload},
};
use iroha_primitives::{json::Json, numeric::Quantity};
use iroha_telemetry::metrics::Status;
use norito::{
    core::NoritoDeserialize as _,
    derive::{JsonDeserialize, JsonSerialize},
    json,
    json::{self as serde_json, Map as JsonMap, Value as JsonValue},
};
use parquet::{arrow::ArrowWriter, basic::Compression, file::properties::WriterProperties};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const NEXUS_CONNECT_FIXTURE_OUTPUT: &str = "fixtures/sdk/nexus_connect_transfer_v1.json";
const NEXUS_CONNECT_FIXTURE_NETWORK_ID: &str =
    "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";
const NEXUS_CONNECT_FIXTURE_CHAIN_ID: &str = "test-chain";
const NEXUS_CONNECT_FIXTURE_CHAIN_DISCRIMINANT: u16 = 369;
const NEXUS_CONNECT_FIXTURE_CREATION_TIME_MS: u64 = 1_700_000_000_000;
const NEXUS_CONNECT_FIXTURE_TTL_MS: u64 = 30_000;
const NEXUS_CONNECT_FIXTURE_NONCE: u32 = 7;
const NEXUS_CONNECT_FIXTURE_AUTHORITY_SEED: [u8; 32] = [0x51; 32];
const NEXUS_CONNECT_FIXTURE_DESTINATION_SEED: [u8; 32] = [0x52; 32];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Requested operation for the Rust-owned Nexus Connect fixture.
pub enum NexusConnectFixtureMode {
    /// Render into an external, non-Git staging directory.
    Write,
    /// Compare the rendered bytes with an existing output tree.
    Check,
}

#[derive(Debug, PartialEq, Eq)]
/// Closed command-line options for the Nexus Connect fixture owner.
pub struct NexusConnectFixtureOptions {
    /// Exactly one requested operation.
    pub mode: NexusConnectFixtureMode,
    /// Absolute root containing `fixtures/sdk/nexus_connect_transfer_v1.json`.
    pub output_root: PathBuf,
}

#[derive(Debug, Clone, Copy)]
struct NexusConnectFixtureSubmitter;

impl NexusToriiSubmitter for NexusConnectFixtureSubmitter {
    fn quote_fee_payment(
        &self,
        payload: &TransactionPayload,
    ) -> Result<FeePaymentIntent, NexusAppError> {
        Ok(payload.fee_payment.clone())
    }

    fn submit_and_wait(
        &self,
        transaction: &SignedTransaction,
        _options: NexusFinalizeOptions,
    ) -> Result<NexusTransferReceipt, NexusAppError> {
        Ok(NexusTransferReceipt {
            signed_transaction: transaction.clone(),
            signed_transaction_hash_hex: hex::encode(transaction.hash().as_ref()),
            status: None,
        })
    }
}

/// Parse the exact `--write|--check --output-root <absolute>` surface.
pub fn parse_nexus_connect_fixture_options(
    arguments: impl IntoIterator<Item = String>,
) -> Result<NexusConnectFixtureOptions, Box<dyn Error>> {
    let mut mode = None;
    let mut output_root = None;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--write" | "--check" => {
                let requested = if argument == "--write" {
                    NexusConnectFixtureMode::Write
                } else {
                    NexusConnectFixtureMode::Check
                };
                if mode.replace(requested).is_some() {
                    return Err("expected exactly one of --write or --check".into());
                }
            }
            "--output-root" if output_root.is_none() => {
                let value = arguments
                    .next()
                    .ok_or("--output-root requires one absolute directory path")?;
                let path = PathBuf::from(value);
                if !path.is_absolute()
                    || path.components().any(|component| {
                        matches!(component, Component::CurDir | Component::ParentDir)
                    })
                {
                    return Err(
                        "--output-root must be one normalized absolute directory path".into(),
                    );
                }
                output_root = Some(path);
            }
            "--output-root" => return Err("--output-root was supplied more than once".into()),
            _ => {
                return Err(format!(
                    "unknown argument `{argument}`; usage: --write|--check --output-root <absolute-directory>"
                )
                .into());
            }
        }
    }

    Ok(NexusConnectFixtureOptions {
        mode: mode.ok_or("expected exactly one of --write or --check")?,
        output_root: output_root.ok_or("--output-root is required")?,
    })
}

/// Build and either stage or verify the Rust-owned Nexus Connect fixture.
pub fn run_nexus_connect_fixture(
    options: &NexusConnectFixtureOptions,
) -> Result<(), Box<dyn Error>> {
    let output_root = match options.mode {
        NexusConnectFixtureMode::Write => nexus_connect_staging_root(&options.output_root)?,
        NexusConnectFixtureMode::Check => options.output_root.clone(),
    };
    let output = output_root.join(NEXUS_CONNECT_FIXTURE_OUTPUT);
    let rendered = build_nexus_connect_fixture()?;
    sync_nexus_connect_fixture(&output, &rendered, options.mode)
}

fn nexus_connect_workspace_root() -> Result<&'static Path, Box<dyn Error>> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "xtask manifest directory has no workspace parent".into())
}

fn nexus_connect_staging_root(path: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "--write requires an existing staging directory at {}: {error}",
            path.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "--write output root must be an existing non-symlink staging directory: {}",
            path.display()
        )
        .into());
    }

    let canonical = fs::canonicalize(path)?;
    let workspace = fs::canonicalize(nexus_connect_workspace_root()?)?;
    if canonical.starts_with(&workspace) {
        return Err(format!(
            "--write refuses the live workspace; use an external staging directory: {}",
            canonical.display()
        )
        .into());
    }
    if canonical
        .ancestors()
        .any(|ancestor| fs::symlink_metadata(ancestor.join(".git")).is_ok())
    {
        return Err(format!(
            "--write output root must not be inside a Git checkout: {}",
            canonical.display()
        )
        .into());
    }
    Ok(canonical)
}

fn nexus_connect_json_object(
    fields: impl IntoIterator<Item = (&'static str, JsonValue)>,
) -> JsonValue {
    let mut map = JsonMap::new();
    for (key, value) in fields {
        map.insert(key.to_owned(), value);
    }
    JsonValue::Object(map)
}

fn nexus_connect_network_id() -> Result<NetworkId, Box<dyn Error>> {
    let network_id: NetworkId = NEXUS_CONNECT_FIXTURE_NETWORK_ID.parse()?;
    if network_id.to_string() != NEXUS_CONNECT_FIXTURE_NETWORK_ID {
        return Err("canonical Nexus fixture NetworkId did not round-trip byte-for-byte".into());
    }
    Ok(network_id)
}

fn nexus_connect_fixture_account(seed: [u8; 32]) -> Result<(KeyPair, AccountId), Box<dyn Error>> {
    let key_pair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::Ed25519)?;
    let account = AccountId::new(key_pair.public_key().clone());
    Ok((key_pair, account))
}

fn build_nexus_connect_fixture() -> Result<Vec<u8>, Box<dyn Error>> {
    let _chain_discriminant =
        ChainDiscriminantGuard::enter(NEXUS_CONNECT_FIXTURE_CHAIN_DISCRIMINANT);
    let network_id = nexus_connect_network_id()?;
    let (authority_key_pair, authority) =
        nexus_connect_fixture_account(NEXUS_CONNECT_FIXTURE_AUTHORITY_SEED)?;
    let (_, destination) = nexus_connect_fixture_account(NEXUS_CONNECT_FIXTURE_DESTINATION_SEED)?;
    let asset_definition = AssetDefinitionId::from_uuid_bytes([
        0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x42, 0x22, 0x82, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22,
        0x22,
    ])?;
    let source_asset = AssetId::new(asset_definition, authority.clone());
    let quantity: Quantity = "12.34".parse()?;
    let mut metadata = Metadata::default();
    metadata.insert("purpose".parse::<Name>()?, Json::from("nexus-app-fixture"));
    let fee_payment = FeePaymentIntent::authority(Vec::new(), None);

    let config = NexusAppConfig {
        signing_public_key: Some(authority_key_pair.public_key().clone()),
        ..NexusAppConfig::new(NEXUS_CONNECT_FIXTURE_CHAIN_ID.into(), network_id)
    };
    let client = NexusAppClient::new(
        config,
        UnsupportedConnectTransport,
        NexusConnectFixtureSubmitter,
    );
    let draft = client.build_transfer_draft(NexusTransferInput {
        source_asset_id: source_asset.clone(),
        quantity: quantity.clone(),
        destination_account_id: destination.clone(),
        authority: Some(authority.clone()),
        metadata,
        fee_payment,
        creation_time_ms: Some(NEXUS_CONNECT_FIXTURE_CREATION_TIME_MS),
        ttl: Some(Duration::from_millis(NEXUS_CONNECT_FIXTURE_TTL_MS)),
        nonce: Some(
            NonZeroU32::new(NEXUS_CONNECT_FIXTURE_NONCE).expect("Nexus fixture nonce is non-zero"),
        ),
    })?;
    let payload_bytes_hex = hex::encode(&draft.signable.payload_bytes);
    let payload_hash_hex = draft.signable.payload_hash_hex.clone();
    let payload_hash = hex::decode(&payload_hash_hex)?;
    let wallet_signature = Signature::try_new(authority_key_pair.private_key(), &payload_hash)?;
    let wallet_signature_hex = hex::encode(wallet_signature.payload());
    let receipt = client.finalize_and_submit(
        draft.signable,
        NexusWalletSignature {
            algorithm: NexusSignatureAlgorithm::Ed25519,
            signature: wallet_signature.payload().to_vec(),
        },
        NexusFinalizeOptions::default(),
    )?;

    let (_, public_key_bytes) = authority_key_pair.public_key().to_bytes();
    let authority_text = authority.to_string();
    let destination_text = destination.to_string();

    let approval_frame = nexus_connect_json_object([
        ("account_id", authority_text.clone().into()),
        (
            "signing_public_key_hex",
            hex::encode(public_key_bytes).into(),
        ),
        ("signature_algorithm", "ed25519".into()),
    ]);
    let connect = nexus_connect_json_object([
        ("app_id", "fixture-app".into()),
        ("sid", "sid-fixture-1".into()),
        (
            "wallet_launch_uri",
            "iroha://connect?sid=sid-fixture-1&role=wallet".into(),
        ),
        ("chain_id", NEXUS_CONNECT_FIXTURE_CHAIN_ID.into()),
        ("approval_frame", approval_frame),
    ]);
    let fee_value = nexus_connect_json_object([
        ("charge_limits", JsonValue::Array(Vec::new())),
        ("gas_limit", JsonValue::Null),
    ]);
    let fee_payment =
        nexus_connect_json_object([("payer", "authority".into()), ("value", fee_value)]);
    let metadata = nexus_connect_json_object([("purpose", "nexus-app-fixture".into())]);
    let transfer_input = nexus_connect_json_object([
        ("network_id", NEXUS_CONNECT_FIXTURE_NETWORK_ID.into()),
        ("source_asset_id", source_asset.to_string().into()),
        ("quantity", quantity.to_string().into()),
        ("destination_account_id", destination_text.clone().into()),
        ("authority", authority_text.clone().into()),
        (
            "creation_time_ms",
            NEXUS_CONNECT_FIXTURE_CREATION_TIME_MS.into(),
        ),
        ("ttl_ms", NEXUS_CONNECT_FIXTURE_TTL_MS.into()),
        ("nonce", NEXUS_CONNECT_FIXTURE_NONCE.into()),
        ("fee_payment", fee_payment),
        ("metadata", metadata),
    ]);
    let expected = nexus_connect_json_object([
        ("payload_bytes_hex", payload_bytes_hex.into()),
        ("payload_hash_hex", payload_hash_hex.into()),
        ("wallet_signature_hex", wallet_signature_hex.into()),
        (
            "signed_transaction_hash_hex",
            receipt.signed_transaction_hash_hex.into(),
        ),
        (
            "status_sequence",
            JsonValue::Array(vec!["Submitted".into(), "Applied".into()]),
        ),
    ]);
    let error_cases = JsonValue::Array(vec![
        nexus_connect_json_object([
            ("name", "unsupported signature algorithm".into()),
            ("signature_algorithm", "secp256k1".into()),
            ("expected_code", "unsupported_signature_algorithm".into()),
        ]),
        nexus_connect_json_object([
            ("name", "approval without signing key".into()),
            (
                "approval_frame",
                nexus_connect_json_object([("account_id", authority_text.clone().into())]),
            ),
            ("expected_code", "missing_signing_public_key".into()),
        ]),
        nexus_connect_json_object([
            ("name", "authority mismatch".into()),
            ("transfer_authority", destination_text.into()),
            ("expected_code", "approval_account_mismatch".into()),
        ]),
        nexus_connect_json_object([
            ("name", "connect transport unavailable".into()),
            ("expected_code", "connect_transport_unavailable".into()),
        ]),
        nexus_connect_json_object([
            ("name", "missing authority".into()),
            ("expected_code", "missing_authority".into()),
        ]),
        nexus_connect_json_object([
            ("name", "invalid signing public key".into()),
            ("expected_code", "invalid_signing_public_key".into()),
        ]),
        nexus_connect_json_object([
            ("name", "invalid signature length".into()),
            ("signature_bytes_hex", "07".repeat(63).into()),
            ("expected_code", "invalid_signature".into()),
        ]),
        nexus_connect_json_object([
            ("name", "torii client unavailable".into()),
            ("expected_code", "torii_client_unavailable".into()),
        ]),
        nexus_connect_json_object([
            ("name", "transaction hash mismatch".into()),
            ("submitted_transaction_hash_hex", "ff".repeat(32).into()),
            ("expected_code", "transaction_hash_mismatch".into()),
        ]),
        nexus_connect_json_object([
            ("name", "submit failure".into()),
            ("expected_code", "submit_failed".into()),
        ]),
        nexus_connect_json_object([
            ("name", "status wait failure".into()),
            ("expected_code", "status_wait_failed".into()),
        ]),
    ]);

    let root = nexus_connect_json_object([
        ("fixture", "nexus_connect_transfer_v1".into()),
        ("version", 1_u64.into()),
        (
            "description",
            "Deterministic SORA Nexus app facade transfer fixture for SDK parity tests.".into(),
        ),
        ("connect", connect),
        ("transfer_input", transfer_input),
        ("expected", expected),
        ("error_cases", error_cases),
    ]);
    let rendered = json::to_json_pretty(&root)?;
    Ok(format!("{rendered}\n").into_bytes())
}

fn sync_nexus_connect_fixture(
    path: &Path,
    expected: &[u8],
    mode: NexusConnectFixtureMode,
) -> Result<(), Box<dyn Error>> {
    match mode {
        NexusConnectFixtureMode::Check => {
            let actual = fs::read(path).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "failed to read generated Nexus fixture {}: {error}",
                        path.display()
                    ),
                )
            })?;
            if actual != expected {
                return Err(format!(
                    "generated Nexus fixture {} is stale; rerun nexus-connect-fixture --write against an external staging root",
                    path.display()
                )
                .into());
            }
        }
        NexusConnectFixtureMode::Write => {
            if fs::read(path).is_ok_and(|actual| actual == expected) {
                return Ok(());
            }
            let parent = path
                .parent()
                .ok_or("generated Nexus fixture has no parent")?;
            fs::create_dir_all(parent)?;
            let temporary = parent.join(format!(
                ".{}.{}.tmp",
                path.file_name()
                    .and_then(|name| name.to_str())
                    .ok_or("generated Nexus fixture name is not UTF-8")?,
                std::process::id()
            ));
            let write_result = (|| -> Result<(), Box<dyn Error>> {
                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&temporary)?;
                file.write_all(expected)?;
                file.sync_all()?;
                drop(file);
                fs::rename(&temporary, path)?;
                Ok(())
            })();
            if write_result.is_err() {
                let _ = fs::remove_file(&temporary);
            }
            write_result?;
        }
    }
    Ok(())
}

pub fn write_lane_commitment_fixtures(output: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(output)?;

    for fixture in sample_commitments() {
        let json_value = json::to_value(&fixture.payload)?;
        let rendered = canonical_json(&json_value)?;
        let json_path = output.join(format!("{}.json", fixture.file_stem));
        fs::write(&json_path, rendered)?;

        let to_path = output.join(format!("{}.to", fixture.file_stem));
        let bytes = norito::to_bytes(&fixture.payload)?;
        fs::write(&to_path, bytes)?;
    }

    Ok(())
}

pub fn verify_lane_commitment_fixtures(dir: &Path) -> Result<(), Box<dyn Error>> {
    let fixtures = sample_commitments();
    let mut expected_entries = BTreeSet::new();
    for fixture in &fixtures {
        expected_entries.insert(format!("{}.json", fixture.file_stem));
        expected_entries.insert(format!("{}.to", fixture.file_stem));
    }

    for fixture in fixtures {
        let json_path = dir.join(format!("{}.json", fixture.file_stem));
        if !json_path.is_file() {
            return Err(format!("missing lane commitment JSON {:?}", json_path).into());
        }
        let raw = fs::read_to_string(&json_path)?;
        let parsed: LaneBlockCommitment = json::from_str(&raw)?;
        if parsed != fixture.payload {
            return Err(format!(
                "lane commitment JSON {:?} does not match the generated payload",
                json_path
            )
            .into());
        }
        let json_value = json::to_value(&fixture.payload)?;
        let canonical = canonical_json(&json_value)?;
        if raw != canonical {
            return Err(format!(
                "lane commitment JSON {:?} is not canonical; run `cargo xtask nexus-fixtures`",
                json_path
            )
            .into());
        }

        let to_path = dir.join(format!("{}.to", fixture.file_stem));
        if !to_path.is_file() {
            return Err(format!("missing lane commitment Norito bytes {:?}", to_path).into());
        }
        let bytes = fs::read(&to_path)?;
        let decoded = norito::from_bytes::<LaneBlockCommitment>(&bytes)
            .and_then(LaneBlockCommitment::try_deserialize)
            .map_err(|err| format!("failed to deserialize {:?}: {err}", to_path))?;
        if decoded != fixture.payload {
            return Err(format!(
                "lane commitment Norito bytes {:?} do not match the generated payload",
                to_path
            )
            .into());
        }
        let expected_bytes = norito::to_bytes(&fixture.payload)?;
        if bytes != expected_bytes {
            return Err(format!(
                "lane commitment Norito bytes {:?} are not canonical; rerun generator",
                to_path
            )
            .into());
        }
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Some(file_name) = path.file_name().and_then(|name| name.to_str())
            && expected_entries.contains(file_name)
        {
            continue;
        }
        return Err(format!(
            "unexpected lane commitment artefact {:?}; delete it or extend the generator",
            path
        )
        .into());
    }

    Ok(())
}

fn load_lane_compliance_map(
    path: &Path,
) -> Result<HashMap<u32, LaneComplianceEvidence>, Box<dyn Error>> {
    let raw = fs::read_to_string(path).map_err(|err| {
        format!(
            "failed to read lane compliance evidence {}: {err}",
            path.display()
        )
    })?;
    let file: LaneComplianceEvidenceFile = serde_json::from_str(&raw).map_err(|err| {
        format!(
            "failed to parse lane compliance evidence {}: {err}",
            path.display()
        )
    })?;
    let mut map = HashMap::new();
    for record in file.lanes {
        let serialized_policy = serde_json::to_string(&record.policy).map_err(|err| {
            format!(
                "failed to serialize lane compliance policy for lane {}: {err}",
                record.lane_id
            )
        })?;
        let policy: LaneCompliancePolicy = json::from_str(&serialized_policy).map_err(|err| {
            format!(
                "failed to decode lane compliance policy for lane {}: {err}",
                record.lane_id
            )
        })?;
        let policy_lane = policy.lane_id.as_u32();
        if policy_lane != record.lane_id {
            return Err(format!(
                "lane compliance record lists lane_id {} but policy targets lane_id {}",
                record.lane_id, policy_lane
            )
            .into());
        }
        if map
            .insert(
                record.lane_id,
                LaneComplianceEvidence {
                    policy: record.policy,
                    reviewer_signatures: record.reviewer_signatures,
                    metrics_snapshot: record.metrics_snapshot,
                    audit_log: record.audit_log,
                },
            )
            .is_some()
        {
            return Err(format!(
                "lane compliance evidence contains duplicate entry for lane {policy_lane}",
            )
            .into());
        }
    }
    Ok(map)
}

fn canonical_json(value: &JsonValue) -> Result<String, Box<dyn Error>> {
    Ok(format!("{}\n", json::to_string_pretty(value)?))
}

struct CommitmentFixture {
    file_stem: &'static str,
    payload: LaneBlockCommitment,
}

fn sample_commitments() -> Vec<CommitmentFixture> {
    vec![
        CommitmentFixture {
            file_stem: "default_public_lane_commitment",
            payload: LaneBlockCommitment {
                block_height: 8_642,
                lane_id: LaneId::new(1),
                lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
                dataspace_id: DataSpaceId::new(7),
                tx_count: 2,
                total_local_amount: quantity("7.5"),
                total_xor_due: quantity("3.05"),
                total_xor_after_haircut: quantity("3"),
                total_xor_variance: quantity("0.05"),
                swap_metadata: Some(LaneSwapMetadata {
                    epsilon_bps: 25,
                    twap_window_seconds: 60,
                    liquidity_profile: LaneLiquidityProfile::Tier1,
                    twap_local_per_xor: "8123.4455".parse().expect("canonical TWAP"),
                    volatility_class: LaneVolatilityClass::Stable,
                }),
                receipts: vec![
                    receipt(
                        "4f25818e98f7b549a21ceda9a1f3812d95d64c83f7d02c361e13caf113e53344",
                        "4",
                        "1.62",
                        "1.6",
                        1_726_296_400_000,
                    ),
                    receipt(
                        "ab56be456758d5be8d3d24ae7ef44c6a0ca1cf4a788ad18cf3b987fe9954f0d2",
                        "3.5",
                        "1.43",
                        "1.4",
                        1_726_296_401_200,
                    ),
                ],
                nexus_fee_receipts: Vec::new(),
                native_amx_receipts: Vec::new(),
            },
        },
        CommitmentFixture {
            file_stem: "cbdc_private_lane_commitment",
            payload: LaneBlockCommitment {
                block_height: 91_234,
                lane_id: LaneId::new(12),
                lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
                dataspace_id: DataSpaceId::new(24),
                tx_count: 3,
                total_local_amount: quantity("9.3"),
                total_xor_due: quantity("4.2"),
                total_xor_after_haircut: quantity("4.05"),
                total_xor_variance: quantity("0.15"),
                swap_metadata: Some(LaneSwapMetadata {
                    epsilon_bps: 120,
                    twap_window_seconds: 300,
                    liquidity_profile: LaneLiquidityProfile::Tier3,
                    twap_local_per_xor: "1.2456".parse().expect("canonical TWAP"),
                    volatility_class: LaneVolatilityClass::Dislocated,
                }),
                receipts: vec![
                    receipt(
                        "beadf1f4a09fd303cc6971f2f58d7f2c1eca1aa1a5d2cda7088fcfd97994cb8a",
                        "3.3",
                        "1.6",
                        "1.55",
                        1_726_297_000_500,
                    ),
                    receipt(
                        "d74fefc1c3f216e8844141493dfd9e4fb3c947ff9b35331a37c6ae16a5f97028",
                        "2.8",
                        "1.3",
                        "1.25",
                        1_726_297_001_250,
                    ),
                    receipt(
                        "cedf9cb93f1b8f52a08ff19793b0ce6049db0a89c9a6ec7fd65dd8f5ecd0f92b",
                        "3.2",
                        "1.3",
                        "1.25",
                        1_726_297_001_900,
                    ),
                ],
                nexus_fee_receipts: Vec::new(),
                native_amx_receipts: Vec::new(),
            },
        },
    ]
}

#[derive(Debug)]
pub struct LaneAuditOptions {
    pub status_path: PathBuf,
    pub json_output: PathBuf,
    pub parquet_output: PathBuf,
    pub markdown_output: PathBuf,
    pub captured_at: Option<String>,
    pub lane_compliance: Option<PathBuf>,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct LaneComplianceEvidence {
    pub policy: JsonValue,
    #[norito(default)]
    pub reviewer_signatures: Vec<LaneComplianceReviewerSignature>,
    #[norito(default = "json_null")]
    pub metrics_snapshot: JsonValue,
    #[norito(default)]
    pub audit_log: Vec<JsonValue>,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct LaneComplianceReviewerSignature {
    pub reviewer: String,
    pub signature_hex: String,
    #[norito(default)]
    pub signed_at: Option<String>,
    #[norito(default)]
    pub digest_hex: Option<String>,
    #[norito(default)]
    pub notes: Option<String>,
}

#[derive(Debug, JsonSerialize, JsonDeserialize)]
struct LaneComplianceEvidenceFile {
    lanes: Vec<LaneComplianceEvidenceRecord>,
}

#[derive(Debug, JsonSerialize, JsonDeserialize)]
struct LaneComplianceEvidenceRecord {
    lane_id: u32,
    policy: JsonValue,
    #[norito(default)]
    reviewer_signatures: Vec<LaneComplianceReviewerSignature>,
    #[norito(default = "json_null")]
    metrics_snapshot: JsonValue,
    #[norito(default)]
    audit_log: Vec<JsonValue>,
}

fn json_null() -> JsonValue {
    JsonValue::Null
}

#[derive(Clone, JsonSerialize)]
struct LaneAuditRow {
    lane_id: u32,
    lane_alias: String,
    dataspace_id: u64,
    dataspace_alias: Option<String>,
    block_height: u64,
    finality_lag_slots: u64,
    teu_capacity: u64,
    teu_committed: u64,
    teu_utilization_pct: f64,
    trigger_level: u64,
    must_serve_truncations: u64,
    scheduler_utilization_pct: u64,
    tx_vertices: u64,
    tx_edges: u64,
    rbc_chunks: u64,
    rbc_bytes_total: u64,
    settlement_backlog_xor_micro: String,
    settlement_backlog_xor: f64,
    governance: Option<String>,
    settlement: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    lane_compliance: Option<LaneComplianceEvidence>,
    manifest_required: bool,
    manifest_ready: bool,
    captured_at: String,
    status_height: u64,
}

impl LaneAuditRow {
    fn compliance_json_string(&self) -> Result<Option<String>, serde_json::Error> {
        self.lane_compliance
            .as_ref()
            .map(serde_json::to_json)
            .transpose()
    }
}

pub fn run_lane_audit(options: &LaneAuditOptions) -> Result<(), Box<dyn Error>> {
    let raw = fs::read_to_string(&options.status_path).map_err(|err| {
        format!(
            "failed to read status blob {}: {err}",
            options.status_path.display()
        )
    })?;
    let status: Status = json::from_str(&raw)?;
    let mut compliance_map = if let Some(path) = &options.lane_compliance {
        load_lane_compliance_map(path)?
    } else {
        HashMap::new()
    };
    let captured_at = options.captured_at.clone().unwrap_or_else(|| {
        OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
    });
    let Status {
        teu_lane_commit,
        blocks: status_height,
        ..
    } = status;
    let mut rows: Vec<LaneAuditRow> = Vec::with_capacity(teu_lane_commit.len());
    for lane in teu_lane_commit {
        let compliance = compliance_map.remove(&lane.lane_id);
        rows.push(LaneAuditRow {
            lane_id: lane.lane_id,
            lane_alias: lane.alias,
            dataspace_id: lane.dataspace_id,
            dataspace_alias: lane.dataspace_alias,
            block_height: lane.block_height,
            finality_lag_slots: lane.finality_lag_slots,
            teu_capacity: lane.capacity,
            teu_committed: lane.committed,
            teu_utilization_pct: compute_teu_utilization_pct(lane.capacity, lane.committed),
            trigger_level: lane.trigger_level,
            must_serve_truncations: lane.must_serve_truncations,
            scheduler_utilization_pct: lane.scheduler_utilization_pct,
            tx_vertices: lane.tx_vertices,
            tx_edges: lane.tx_edges,
            rbc_chunks: lane.rbc_chunks,
            rbc_bytes_total: lane.rbc_bytes_total,
            settlement_backlog_xor_micro: lane.settlement_backlog_xor_micro.to_string(),
            settlement_backlog_xor: micro_xor_to_units(lane.settlement_backlog_xor_micro),
            governance: lane.governance,
            settlement: lane.settlement,
            lane_compliance: compliance,
            manifest_required: lane.manifest_required,
            manifest_ready: lane.manifest_ready,
            captured_at: captured_at.clone(),
            status_height,
        });
    }
    if !compliance_map.is_empty() {
        let mut unknown: Vec<String> = compliance_map.keys().map(|id| id.to_string()).collect();
        unknown.sort();
        return Err(format!(
            "lane compliance evidence includes lanes not present in the status blob: {}",
            unknown.join(", ")
        )
        .into());
    }
    rows.sort_by(|a, b| {
        a.lane_id
            .cmp(&b.lane_id)
            .then(a.dataspace_id.cmp(&b.dataspace_id))
    });
    write_json_rows(&options.json_output, &rows)?;
    write_parquet_rows(&options.parquet_output, &rows)?;
    write_markdown_summary(&options.markdown_output, &rows)?;
    println!(
        "lane audit exported to {}, {}, and {}",
        options.json_output.display(),
        options.parquet_output.display(),
        options.markdown_output.display()
    );
    Ok(())
}

fn write_json_rows(path: &Path, rows: &[LaneAuditRow]) -> Result<(), Box<dyn Error>> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let rendered = serde_json::to_json_pretty(&rows.to_vec())?;
    fs::write(path, rendered)?;
    Ok(())
}

fn write_parquet_rows(path: &Path, rows: &[LaneAuditRow]) -> Result<(), Box<dyn Error>> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let schema = Arc::new(arrow_schema());
    let compliance_serialized: Vec<Option<String>> = rows
        .iter()
        .map(|row| row.compliance_json_string())
        .collect::<Result<_, _>>()
        .map_err(|err| format!("failed to serialize lane compliance evidence: {err}"))?;
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            make_u32_array(rows.iter().map(|row| row.lane_id)),
            make_string_array(rows.iter().map(|row| Some(row.lane_alias.as_str()))),
            make_u64_array(rows.iter().map(|row| row.dataspace_id)),
            make_string_array(rows.iter().map(|row| row.dataspace_alias.as_deref())),
            make_u64_array(rows.iter().map(|row| row.block_height)),
            make_u64_array(rows.iter().map(|row| row.finality_lag_slots)),
            make_u64_array(rows.iter().map(|row| row.teu_capacity)),
            make_u64_array(rows.iter().map(|row| row.teu_committed)),
            make_f64_array(rows.iter().map(|row| row.teu_utilization_pct)),
            make_u64_array(rows.iter().map(|row| row.trigger_level)),
            make_u64_array(rows.iter().map(|row| row.must_serve_truncations)),
            make_u64_array(rows.iter().map(|row| row.scheduler_utilization_pct)),
            make_u64_array(rows.iter().map(|row| row.tx_vertices)),
            make_u64_array(rows.iter().map(|row| row.tx_edges)),
            make_u64_array(rows.iter().map(|row| row.rbc_chunks)),
            make_u64_array(rows.iter().map(|row| row.rbc_bytes_total)),
            make_string_array(
                rows.iter()
                    .map(|row| Some(row.settlement_backlog_xor_micro.as_str())),
            ),
            make_f64_array(rows.iter().map(|row| row.settlement_backlog_xor)),
            make_string_array(rows.iter().map(|row| row.governance.as_deref())),
            make_string_array(rows.iter().map(|row| row.settlement.as_deref())),
            make_string_array(compliance_serialized.iter().map(|value| value.as_deref())),
            make_bool_array(rows.iter().map(|row| row.manifest_required)),
            make_bool_array(rows.iter().map(|row| row.manifest_ready)),
            make_string_array(rows.iter().map(|row| Some(row.captured_at.as_str()))),
            make_u64_array(rows.iter().map(|row| row.status_height)),
        ],
    )?;
    let file = fs::File::create(path)?;
    let props = WriterProperties::builder()
        .set_compression(Compression::UNCOMPRESSED)
        .build();
    let mut writer = ArrowWriter::try_new(file, schema, Some(props))?;
    writer.write(&batch)?;
    writer.close()?;
    Ok(())
}

fn write_markdown_summary(path: &Path, rows: &[LaneAuditRow]) -> Result<(), Box<dyn Error>> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut rendered = String::new();
    writeln!(rendered, "# Nexus Lane Audit")?;
    if rows.is_empty() {
        writeln!(
            rendered,
            "\nNo lane telemetry records were found in the provided status snapshot."
        )?;
        fs::write(path, rendered)?;
        return Ok(());
    }

    let captured_at = &rows[0].captured_at;
    let status_height = rows[0].status_height;
    let total = rows.len();
    let lagging = rows.iter().filter(|row| row.finality_lag_slots > 0).count();
    let backlog_lanes = rows
        .iter()
        .filter(|row| row.settlement_backlog_xor > 0.0)
        .count();
    let pending_manifests = rows
        .iter()
        .filter(|row| row.manifest_required && !row.manifest_ready)
        .count();
    let missing_compliance = rows
        .iter()
        .filter(|row| row.manifest_required && row.lane_compliance.is_none())
        .count();
    let max_backlog = rows
        .iter()
        .map(|row| row.settlement_backlog_xor)
        .fold(0.0_f64, f64::max);
    let max_lag = rows
        .iter()
        .map(|row| row.finality_lag_slots)
        .max()
        .unwrap_or(0);

    writeln!(rendered)?;
    writeln!(rendered, "- Captured at: {captured_at}")?;
    writeln!(rendered, "- Status height: {status_height}")?;
    writeln!(
        rendered,
        "- Lanes: {total} (lagging: {lagging}; backlog>0: {backlog_lanes}; pending manifests: {pending_manifests}; missing compliance: {missing_compliance})"
    )?;
    writeln!(rendered, "- Max finality lag (slots): {max_lag}")?;
    writeln!(rendered, "- Peak backlog (XOR): {:.6}", max_backlog)?;
    writeln!(rendered)?;
    writeln!(
        rendered,
        "| Lane | Dataspace | Height | Finality Lag | Backlog (XOR) | TEU Util (%) | Flags |"
    )?;
    writeln!(rendered, "| --- | --- | --- | --- | --- | --- | --- |")?;

    for row in rows {
        let lane_label = format!("{} (#{})", row.lane_alias, row.lane_id);
        let dataspace_label = match row.dataspace_alias.as_deref() {
            Some(alias) => format!("{alias} (#{})", row.dataspace_id),
            None => format!("#{}", row.dataspace_id),
        };
        let flags = {
            let mut entries: Vec<&str> = Vec::new();
            if row.finality_lag_slots > 0 {
                entries.push("lag");
            }
            if row.settlement_backlog_xor > 0.0 {
                entries.push("backlog");
            }
            if row.manifest_required && !row.manifest_ready {
                entries.push("manifest");
            }
            if row.manifest_required && row.lane_compliance.is_none() {
                entries.push("compliance");
            }
            if row.teu_utilization_pct >= 95.0 {
                entries.push("teu-high");
            }
            if entries.is_empty() {
                "ok".to_string()
            } else {
                entries.join(",")
            }
        };
        writeln!(
            rendered,
            "| {lane_label} | {dataspace_label} | {} | {} | {:.6} | {:.1} | {flags} |",
            row.block_height,
            row.finality_lag_slots,
            row.settlement_backlog_xor,
            row.teu_utilization_pct
        )?;
    }

    fs::write(path, rendered)?;
    Ok(())
}

fn arrow_schema() -> Schema {
    Schema::new(vec![
        Field::new("lane_id", DataType::UInt32, false),
        Field::new("lane_alias", DataType::Utf8, false),
        Field::new("dataspace_id", DataType::UInt64, false),
        Field::new("dataspace_alias", DataType::Utf8, true),
        Field::new("block_height", DataType::UInt64, false),
        Field::new("finality_lag_slots", DataType::UInt64, false),
        Field::new("teu_capacity", DataType::UInt64, false),
        Field::new("teu_committed", DataType::UInt64, false),
        Field::new("teu_utilization_pct", DataType::Float64, false),
        Field::new("trigger_level", DataType::UInt64, false),
        Field::new("must_serve_truncations", DataType::UInt64, false),
        Field::new("scheduler_utilization_pct", DataType::UInt64, false),
        Field::new("tx_vertices", DataType::UInt64, false),
        Field::new("tx_edges", DataType::UInt64, false),
        Field::new("rbc_chunks", DataType::UInt64, false),
        Field::new("rbc_bytes_total", DataType::UInt64, false),
        Field::new("settlement_backlog_xor_micro", DataType::Utf8, false),
        Field::new("settlement_backlog_xor", DataType::Float64, false),
        Field::new("governance", DataType::Utf8, true),
        Field::new("settlement", DataType::Utf8, true),
        Field::new("lane_compliance", DataType::Utf8, true),
        Field::new("manifest_required", DataType::Boolean, false),
        Field::new("manifest_ready", DataType::Boolean, false),
        Field::new("captured_at", DataType::Utf8, false),
        Field::new("status_height", DataType::UInt64, false),
    ])
}

fn make_u32_array<I>(values: I) -> ArrayRef
where
    I: IntoIterator<Item = u32>,
{
    Arc::new(UInt32Array::from_iter_values(values))
}

fn make_u64_array<I>(values: I) -> ArrayRef
where
    I: IntoIterator<Item = u64>,
{
    Arc::new(UInt64Array::from_iter_values(values))
}

fn make_f64_array<I>(values: I) -> ArrayRef
where
    I: IntoIterator<Item = f64>,
{
    Arc::new(Float64Array::from_iter_values(values))
}

fn make_bool_array<I>(values: I) -> ArrayRef
where
    I: IntoIterator<Item = bool>,
{
    Arc::new(BooleanArray::from_iter(values.into_iter().map(Some)))
}

fn make_string_array<'a, I>(values: I) -> ArrayRef
where
    I: IntoIterator<Item = Option<&'a str>>,
{
    Arc::new(StringArray::from_iter(values))
}

fn compute_teu_utilization_pct(capacity: u64, committed: u64) -> f64 {
    if capacity == 0 {
        return 0.0;
    }
    (committed as f64 / capacity as f64) * 100.0
}

fn micro_xor_to_units(value: u128) -> f64 {
    (value as f64) / 1_000_000.0
}

fn quantity(value: &str) -> Quantity {
    value.parse().expect("canonical quantity fixture")
}

fn receipt(
    source_hex: &str,
    local_amount: &str,
    xor_due: &str,
    xor_after_haircut: &str,
    timestamp_ms: u64,
) -> LaneSettlementReceipt {
    let local_amount = quantity(local_amount);
    let xor_due = quantity(xor_due);
    let xor_after_haircut = quantity(xor_after_haircut);
    let variance = xor_due
        .checked_sub(&xor_after_haircut)
        .expect("fixture haircut cannot exceed XOR due");
    LaneSettlementReceipt {
        source_id: hex32(source_hex),
        local_amount,
        xor_due,
        xor_after_haircut,
        xor_variance: variance,
        timestamp_ms,
    }
}

fn hex32(input: &str) -> [u8; 32] {
    let bytes = decode(input).expect("fixture hex payload");
    assert_eq!(
        bytes.len(),
        32,
        "lane commitment fixture ids must be 32 bytes"
    );
    let mut out = [0_u8; 32];
    out.copy_from_slice(&bytes);
    out
}

#[cfg(test)]
mod tests {
    use std::fs;

    use arrow_array::{Array, BooleanArray, Float64Array, StringArray, UInt32Array, UInt64Array};
    use iroha_data_model::{
        metadata::Metadata,
        nexus::{AuditControls, JurisdictionSet, LaneCompliancePolicyId},
    };
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use tempfile::{NamedTempFile, tempdir};

    use super::*;

    #[test]
    fn teu_utilization_helper_handles_zero_capacity() {
        assert_eq!(compute_teu_utilization_pct(0, 10), 0.0);
        let pct = compute_teu_utilization_pct(200, 100);
        assert!((pct - 50.0).abs() < 1e-6);
    }

    #[test]
    fn parquet_writer_round_trips_rows() {
        let dir = tempdir().expect("tempdir");
        let parquet_path = dir.path().join("lane.parquet");
        let rows = vec![
            LaneAuditRow {
                lane_id: 4,
                lane_alias: "lane-4".to_string(),
                dataspace_id: 7,
                dataspace_alias: Some("payments".to_string()),
                block_height: 42,
                finality_lag_slots: 3,
                teu_capacity: 1200,
                teu_committed: 640,
                teu_utilization_pct: compute_teu_utilization_pct(1200, 640),
                trigger_level: 0,
                must_serve_truncations: 2,
                scheduler_utilization_pct: 87,
                tx_vertices: 48,
                tx_edges: 12,
                rbc_chunks: 1_024,
                rbc_bytes_total: 65_536,
                settlement_backlog_xor_micro: "1250000".to_string(),
                settlement_backlog_xor: 1.25,
                governance: Some("council".to_string()),
                settlement: None,
                lane_compliance: Some(sample_compliance(4, 7)),
                manifest_required: true,
                manifest_ready: false,
                captured_at: "2026-02-12T09:00:00Z".to_string(),
                status_height: 81,
            },
            LaneAuditRow {
                lane_id: 9,
                lane_alias: "lane-9".to_string(),
                dataspace_id: 11,
                dataspace_alias: None,
                block_height: 7,
                finality_lag_slots: 1,
                teu_capacity: 250,
                teu_committed: 250,
                teu_utilization_pct: compute_teu_utilization_pct(250, 250),
                trigger_level: 1,
                must_serve_truncations: 0,
                scheduler_utilization_pct: 64,
                tx_vertices: 12,
                tx_edges: 2,
                rbc_chunks: 512,
                rbc_bytes_total: 16_384,
                settlement_backlog_xor_micro: "0".to_string(),
                settlement_backlog_xor: 0.0,
                governance: None,
                settlement: Some("settlement-ok".to_string()),
                lane_compliance: None,
                manifest_required: false,
                manifest_ready: true,
                captured_at: "2026-02-12T10:00:00Z".to_string(),
                status_height: 99,
            },
        ];

        write_parquet_rows(&parquet_path, &rows).expect("parquet writer");

        let file = fs::File::open(&parquet_path).expect("parquet exists");
        let mut reader = ParquetRecordBatchReaderBuilder::try_new(file)
            .expect("reader builder")
            .with_batch_size(64)
            .build()
            .expect("reader");
        let batch = reader.next().expect("one batch").expect("batch ok");

        assert_eq!(batch.num_rows(), rows.len());

        let lane_ids = batch
            .column_by_name("lane_id")
            .and_then(|array| array.as_any().downcast_ref::<UInt32Array>())
            .expect("lane_id column");
        assert_eq!(lane_ids.value(0), rows[0].lane_id);
        assert_eq!(lane_ids.value(1), rows[1].lane_id);

        let dataspace_alias = batch
            .column_by_name("dataspace_alias")
            .and_then(|array| array.as_any().downcast_ref::<StringArray>())
            .expect("dataspace alias");
        assert_eq!(dataspace_alias.value(0), "payments");
        assert!(dataspace_alias.is_null(1));

        let utilization = batch
            .column_by_name("teu_utilization_pct")
            .and_then(|array| array.as_any().downcast_ref::<Float64Array>())
            .expect("utilization column");
        assert!((utilization.value(0) - rows[0].teu_utilization_pct).abs() < 1e-6);

        let backlog = batch
            .column_by_name("settlement_backlog_xor")
            .and_then(|array| array.as_any().downcast_ref::<Float64Array>())
            .expect("backlog column");
        assert!((backlog.value(0) - rows[0].settlement_backlog_xor).abs() < 1e-6);

        let manifest_required = batch
            .column_by_name("manifest_required")
            .and_then(|array| array.as_any().downcast_ref::<BooleanArray>())
            .expect("manifest_required column");
        assert!(manifest_required.value(0));
        assert!(!manifest_required.value(1));

        let status_height = batch
            .column_by_name("status_height")
            .and_then(|array| array.as_any().downcast_ref::<UInt64Array>())
            .expect("status height");
        assert_eq!(status_height.value(1), rows[1].status_height);

        let trigger_level = batch
            .column_by_name("trigger_level")
            .and_then(|array| array.as_any().downcast_ref::<UInt64Array>())
            .expect("trigger level");
        assert_eq!(trigger_level.value(1), rows[1].trigger_level);

        let compliance = batch
            .column_by_name("lane_compliance")
            .and_then(|array| array.as_any().downcast_ref::<StringArray>())
            .expect("lane_compliance column");
        assert!(compliance.is_valid(0));
        assert!(compliance.is_null(1));
        let parsed: JsonValue =
            serde_json::from_str(compliance.value(0)).expect("compliance json parses");
        assert_eq!(
            parsed
                .get("reviewer_signatures")
                .and_then(|value| value.as_array())
                .map(|arr| arr.len()),
            Some(1)
        );

        assert!(
            reader.next().is_none(),
            "parquet reader should yield a single batch"
        );
    }

    #[test]
    fn lane_compliance_loader_rejects_mismatched_lane_ids() {
        let file = NamedTempFile::new().expect("temp file");
        let mut record = record_from_evidence(sample_compliance(4, 7), 4);
        if let JsonValue::Object(ref mut map) = record.policy {
            map.insert("lane_id".to_string(), JsonValue::from(3_u64));
        } else {
            panic!("policy should be an object");
        }
        write_compliance_file(file.path(), vec![record]);
        let err = load_lane_compliance_map(file.path()).expect_err("loader must fail");
        assert!(
            err.to_string().contains("policy targets lane_id"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn lane_compliance_loader_rejects_duplicate_entries() {
        let file = NamedTempFile::new().expect("temp file");
        let record_a = record_from_evidence(sample_compliance(4, 7), 4);
        let record_b = record_from_evidence(sample_compliance(4, 7), 4);
        write_compliance_file(file.path(), vec![record_a, record_b]);
        let err = load_lane_compliance_map(file.path()).expect_err("loader must fail");
        assert!(
            err.to_string().contains("duplicate entry for lane"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn lane_compliance_loader_accepts_valid_entries() {
        let file = NamedTempFile::new().expect("temp file");
        let record_a = record_from_evidence(sample_compliance(4, 7), 4);
        let record_b = record_from_evidence(sample_compliance(9, 11), 9);
        write_compliance_file(file.path(), vec![record_a, record_b]);
        let map = load_lane_compliance_map(file.path()).expect("loader ok");
        assert_eq!(map.len(), 2);
        assert!(map.contains_key(&4));
        assert!(map.contains_key(&9));
    }

    #[test]
    fn markdown_summary_includes_flags_and_counts() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("lane.md");
        let rows = vec![
            LaneAuditRow {
                lane_id: 4,
                lane_alias: "lane-4".to_string(),
                dataspace_id: 7,
                dataspace_alias: Some("payments".to_string()),
                block_height: 42,
                finality_lag_slots: 3,
                teu_capacity: 1200,
                teu_committed: 1188,
                teu_utilization_pct: compute_teu_utilization_pct(1200, 1188),
                trigger_level: 0,
                must_serve_truncations: 2,
                scheduler_utilization_pct: 87,
                tx_vertices: 48,
                tx_edges: 12,
                rbc_chunks: 1_024,
                rbc_bytes_total: 65_536,
                settlement_backlog_xor_micro: "1250000".to_string(),
                settlement_backlog_xor: 1.25,
                governance: Some("council".to_string()),
                settlement: None,
                lane_compliance: None,
                manifest_required: true,
                manifest_ready: false,
                captured_at: "2026-02-12T09:00:00Z".to_string(),
                status_height: 81,
            },
            LaneAuditRow {
                lane_id: 9,
                lane_alias: "lane-9".to_string(),
                dataspace_id: 11,
                dataspace_alias: None,
                block_height: 7,
                finality_lag_slots: 0,
                teu_capacity: 250,
                teu_committed: 200,
                teu_utilization_pct: compute_teu_utilization_pct(250, 200),
                trigger_level: 1,
                must_serve_truncations: 0,
                scheduler_utilization_pct: 64,
                tx_vertices: 12,
                tx_edges: 2,
                rbc_chunks: 512,
                rbc_bytes_total: 16_384,
                settlement_backlog_xor_micro: "0".to_string(),
                settlement_backlog_xor: 0.0,
                governance: None,
                settlement: Some("settlement-ok".to_string()),
                lane_compliance: Some(sample_compliance(9, 11)),
                manifest_required: false,
                manifest_ready: true,
                captured_at: "2026-02-12T09:00:00Z".to_string(),
                status_height: 81,
            },
        ];

        write_markdown_summary(&path, &rows).expect("markdown writer ok");
        let rendered = fs::read_to_string(&path).expect("markdown exists");
        assert!(rendered.contains("# Nexus Lane Audit"));
        assert!(rendered.contains(
            "Lanes: 2 (lagging: 1; backlog>0: 1; pending manifests: 1; missing compliance: 1)"
        ));
        assert!(rendered.contains(
            "| lane-4 (#4) | payments (#7) | 42 | 3 | 1.250000 | 99.0 | lag,backlog,manifest,compliance,teu-high |"
        ));
        assert!(rendered.contains("| lane-9 (#9) | #11 | 7 | 0 | 0.000000 | 80.0 | ok |"));
    }

    #[test]
    fn canonical_json_renders_lane_commitment_value() {
        let fixture = sample_commitments()
            .into_iter()
            .next()
            .expect("lane commitment fixture");
        let value = json::to_value(&fixture.payload).expect("lane commitment json value");

        let rendered = canonical_json(&value).expect("canonical JSON");

        assert!(
            rendered.ends_with('\n'),
            "canonical lane commitment JSON should end with a newline"
        );
        let parsed: LaneBlockCommitment = json::from_str(&rendered).expect("parse canonical JSON");
        assert_eq!(parsed, fixture.payload);
    }

    #[test]
    fn nexus_connect_fixture_options_require_one_mode_and_absolute_root() {
        let staging = tempdir().expect("temporary staging root");
        let root = staging.path().to_string_lossy().into_owned();
        assert_eq!(
            parse_nexus_connect_fixture_options([
                "--write".to_owned(),
                "--output-root".to_owned(),
                root.clone(),
            ])
            .expect("valid Nexus fixture options"),
            NexusConnectFixtureOptions {
                mode: NexusConnectFixtureMode::Write,
                output_root: PathBuf::from(&root),
            }
        );

        for invalid in [
            vec![],
            vec!["--write".to_owned()],
            vec![
                "--write".to_owned(),
                "--check".to_owned(),
                "--output-root".to_owned(),
                root.clone(),
            ],
            vec![
                "--check".to_owned(),
                "--output-root".to_owned(),
                "relative".to_owned(),
            ],
            vec![
                "--check".to_owned(),
                "--output-root".to_owned(),
                root.clone(),
                "unexpected".to_owned(),
            ],
        ] {
            assert!(parse_nexus_connect_fixture_options(invalid).is_err());
        }
    }

    #[test]
    fn nexus_connect_fixture_is_deterministic_and_has_closed_domain_fields() {
        let first = build_nexus_connect_fixture().expect("build Nexus fixture once");
        let second = build_nexus_connect_fixture().expect("build Nexus fixture twice");
        assert_eq!(first, second);

        let parsed: JsonValue = json::from_slice(&first).expect("parse generated Nexus fixture");
        let root = parsed.as_object().expect("Nexus fixture root object");
        assert_eq!(
            root.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "connect",
                "description",
                "error_cases",
                "expected",
                "fixture",
                "transfer_input",
                "version",
            ])
        );

        let connect = root
            .get("connect")
            .and_then(JsonValue::as_object)
            .expect("closed Connect descriptor");
        assert_eq!(
            connect.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "app_id",
                "approval_frame",
                "chain_id",
                "sid",
                "wallet_launch_uri",
            ])
        );
        assert_eq!(
            connect.get("chain_id").and_then(JsonValue::as_str),
            Some(NEXUS_CONNECT_FIXTURE_CHAIN_ID)
        );

        let transfer = root
            .get("transfer_input")
            .and_then(JsonValue::as_object)
            .expect("closed transfer input");
        assert_eq!(
            transfer.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "authority",
                "creation_time_ms",
                "destination_account_id",
                "fee_payment",
                "metadata",
                "network_id",
                "nonce",
                "quantity",
                "source_asset_id",
                "ttl_ms",
            ])
        );
        assert_eq!(
            transfer.get("network_id").and_then(JsonValue::as_str),
            Some(NEXUS_CONNECT_FIXTURE_NETWORK_ID)
        );
        for retired in ["chain", "chainId", "chain_id"] {
            assert!(!transfer.contains_key(retired));
        }

        let expected = root
            .get("expected")
            .and_then(JsonValue::as_object)
            .expect("closed expected vectors");
        assert_eq!(
            expected.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "payload_bytes_hex",
                "payload_hash_hex",
                "signed_transaction_hash_hex",
                "status_sequence",
                "wallet_signature_hex",
            ])
        );
        assert_eq!(
            root.get("error_cases")
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(11)
        );
    }

    #[test]
    fn nexus_connect_fixture_writes_only_to_external_staging_root() {
        let staging = tempdir().expect("temporary staging root");
        let write = NexusConnectFixtureOptions {
            mode: NexusConnectFixtureMode::Write,
            output_root: staging.path().to_path_buf(),
        };
        run_nexus_connect_fixture(&write).expect("write staged Nexus fixture");
        let output = staging.path().join(NEXUS_CONNECT_FIXTURE_OUTPUT);
        assert!(output.is_file());

        run_nexus_connect_fixture(&NexusConnectFixtureOptions {
            mode: NexusConnectFixtureMode::Check,
            output_root: staging.path().to_path_buf(),
        })
        .expect("check staged Nexus fixture");

        let error = run_nexus_connect_fixture(&NexusConnectFixtureOptions {
            mode: NexusConnectFixtureMode::Write,
            output_root: nexus_connect_workspace_root()
                .expect("workspace root")
                .to_path_buf(),
        })
        .expect_err("write mode must refuse the live workspace");
        assert!(error.to_string().contains("refuses the live workspace"));
    }

    fn sample_compliance(lane_id: u32, dataspace_id: u64) -> LaneComplianceEvidence {
        let policy = LaneCompliancePolicy {
            id: LaneCompliancePolicyId::default(),
            version: 1,
            lane_id: LaneId::new(lane_id),
            dataspace_id: DataSpaceId::new(dataspace_id),
            jurisdiction: JurisdictionSet::default(),
            deny: Vec::new(),
            allow: Vec::new(),
            transfer_limits: Vec::new(),
            audit_controls: AuditControls::default(),
            metadata: Metadata::default(),
        };
        LaneComplianceEvidence {
            policy: policy_to_json_value(&policy),
            reviewer_signatures: vec![LaneComplianceReviewerSignature {
                reviewer: "auditor@example.com".to_string(),
                signature_hex: "deadbeef".to_string(),
                signed_at: Some("2026-02-01T00:00:00Z".to_string()),
                digest_hex: None,
                notes: Some("baseline".to_string()),
            }],
            metrics_snapshot: json!({
                "nexus_lane_policy_decisions_total": {
                    "allow": 42,
                    "deny": 1
                }
            }),
            audit_log: vec![json!({
                "decision": "allow",
                "policy_id": "lane-4-policy",
                "recorded_at": "2026-02-12T09:00:00Z"
            })],
        }
    }

    fn record_from_evidence(
        evidence: LaneComplianceEvidence,
        lane_id: u32,
    ) -> LaneComplianceEvidenceRecord {
        LaneComplianceEvidenceRecord {
            lane_id,
            policy: evidence.policy,
            reviewer_signatures: evidence.reviewer_signatures,
            metrics_snapshot: evidence.metrics_snapshot,
            audit_log: evidence.audit_log,
        }
    }

    fn write_compliance_file(path: &Path, records: Vec<LaneComplianceEvidenceRecord>) {
        let file = LaneComplianceEvidenceFile { lanes: records };
        let value = serde_json::to_value(&file).expect("lane compliance value");
        let mut rendered =
            serde_json::to_string_pretty(&value).expect("lane compliance serialization");
        rendered.push('\n');
        fs::write(path, rendered).expect("write compliance file");
    }

    fn policy_to_json_value(policy: &LaneCompliancePolicy) -> JsonValue {
        let norito_value = json::to_value(policy).expect("policy json value");
        let rendered = json::to_string(&norito_value).expect("policy json encode");
        serde_json::from_str(&rendered).expect("serde policy json")
    }
}
