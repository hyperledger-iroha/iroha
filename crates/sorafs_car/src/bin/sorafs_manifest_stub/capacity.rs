//! CLI helpers for authoring capacity marketplace artefacts.

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STD};
use hex::FromHex;
use norito::json::{self, Map, Value};
use sorafs_manifest::{
    capacity::{
        CAPACITY_DECLARATION_VERSION_V1, CAPACITY_DISPUTE_VERSION_V1,
        CAPACITY_TELEMETRY_VERSION_V1, CapacityDeclarationV1, CapacityDisputeEvidenceV1,
        CapacityDisputeKind, CapacityDisputeV1, CapacityMetadataEntry, CapacityTelemetryV1,
        ChunkerCommitmentV1, LaneCommitmentV1, PricingScheduleV1, REPLICATION_ORDER_VERSION_V1,
        ReplicationAssignmentV1, ReplicationOrderSlaV1, ReplicationOrderV1,
    },
    chunker_registry,
    deal::XorQuantity,
    provider_advert::{CapabilityType, StakePointer},
};

/// Entry point for the `capacity` sub-commands.
pub fn run<I>(mut args: I) -> Result<(), String>
where
    I: Iterator<Item = String>,
{
    let Some(subcommand) = args.next() else {
        return Err(capacity_usage());
    };

    match subcommand.as_str() {
        "declaration" => run_declaration(args),
        "telemetry" => run_telemetry(args),
        "replication-order" => run_replication_order(args),
        "complete" => run_complete(args),
        "dispute" => run_dispute(args),
        "help" => {
            println!("{usage}", usage = capacity_usage());
            Ok(())
        }
        other => Err(format!(
            "unknown capacity subcommand `{other}`; try `capacity help`"
        )),
    }
}

fn capacity_usage() -> String {
    r#"usage: sorafs_manifest_stub capacity <subcommand> [options]

Available subcommands:
  declaration        Generate a CapacityDeclarationV1 payload from a JSON spec.
  telemetry          Generate a CapacityTelemetryV1 payload from a JSON spec.
  replication-order  Generate a ReplicationOrderV1 payload from a JSON spec.
  complete           Emit a completion request body referencing an order id.
  dispute            Generate a CapacityDisputeV1 payload from a JSON spec.

Run `sorafs_manifest_stub capacity <subcommand> --help` for detailed usage.
"#
    .to_owned()
}

fn declaration_usage() -> &'static str {
    r#"usage: sorafs_manifest_stub capacity declaration --spec=<file> [options]

Options:
  --spec=<file>                Path to the declaration spec JSON (required).
  --registered-epoch=<u64>     Override the registered epoch recorded in the summary.
  --valid-from-epoch=<u64>     Override the declaration activation epoch in the summary.
  --valid-until-epoch=<u64>    Override the declaration expiry epoch in the summary.
  --json-out=<file>            Write a Norito JSON summary to <file>.
  --norito-out=<file>          Write the canonical Norito bytes to <file>.
  --base64-out=<file>          Write the canonical Norito payload (base64) to <file>.
  --request-out=<file>         Write Torii request JSON (`/v1/sorafs/capacity/declare`) to <file>.
  --authority=<account_id>     Account identifier used when building the request JSON.
  --private-key=<key>          Exposed private key (multihash hex) for the request JSON.
  --private-key-file=<file>    Read the private key from <file> instead of exposing it in argv.
  --quiet                      Suppress stdout (otherwise prints the base64 payload).
  --help                       Show this message.

The spec schema is documented in `docs/source/sorafs/storage_capacity_marketplace.md`.
"#
}

fn telemetry_usage() -> &'static str {
    r#"usage: sorafs_manifest_stub capacity telemetry --spec=<file> [options]

Options:
  --spec=<file>                Path to the telemetry spec JSON (required).
  --json-out=<file>            Write a Norito JSON summary to <file>.
  --norito-out=<file>          Write the canonical Norito bytes to <file>.
  --base64-out=<file>          Write the canonical Norito payload (base64) to <file>.
  --request-out=<file>         Write Torii request JSON (`/v1/sorafs/capacity/telemetry`) to <file>.
  --authority=<account_id>     Account identifier used when building the request JSON.
  --private-key=<key>          Exposed private key (multihash hex) for the request JSON.
  --private-key-file=<file>    Read the private key from <file> instead of exposing it in argv.
  --quiet                      Suppress stdout (otherwise prints the base64 payload).
  --help                       Show this message.

The spec schema is documented in `docs/source/sorafs/storage_capacity_marketplace.md`.
When present, `effective_capacity_gib` (or `effective_gib`) overrides the default effective GiB
derived from `utilised_capacity_gib` in the generated request payload.
"#
}

fn replication_usage() -> &'static str {
    r#"usage: sorafs_manifest_stub capacity replication-order --spec=<file> [options]

Options:
  --spec=<file>                Path to the replication order spec JSON (required).
  --json-out=<file>            Write a Norito JSON summary to <file>.
  --norito-out=<file>          Write the canonical Norito bytes to <file>.
  --base64-out=<file>          Write the canonical Norito payload (base64) to <file>.
  --request-out=<file>         Write Torii request JSON (`/v1/sorafs/capacity/schedule`) to <file>.
  --quiet                      Suppress stdout (otherwise prints the base64 payload).
  --help                       Show this message.

The spec schema is documented in `docs/source/sorafs/storage_capacity_marketplace.md`.
"#
}

fn complete_usage() -> &'static str {
    r#"usage: sorafs_manifest_stub capacity complete --order-id=<hex> [options]

Options:
  --order-id=<hex>             Replication order identifier (BLAKE3-256, hex) (required).
  --request-out=<file>         Write Torii request JSON (`/v1/sorafs/capacity/complete`) to <file>.
  --quiet                      Suppress stdout (otherwise prints the JSON payload).
  --help                       Show this message.
"#
}

fn dispute_usage() -> &'static str {
    r#"usage: sorafs_manifest_stub capacity dispute --spec=<file> [options]

Options:
  --spec=<file>                Path to the dispute spec JSON (required).
  --json-out=<file>            Write a Norito JSON summary to <file>.
  --norito-out=<file>          Write the canonical Norito bytes to <file>.
  --base64-out=<file>          Write the canonical Norito payload (base64) to <file>.
  --request-out=<file>         Write Torii request JSON (`/v1/sorafs/capacity/dispute`) to <file>.
  --authority=<account_id>     Account identifier used when building the request JSON.
  --private-key=<key>          Exposed private key (multihash hex) for the request JSON.
  --private-key-file=<file>    Read the private key from <file> instead of exposing it in argv.
  --quiet                      Suppress stdout (otherwise prints the base64 payload).
  --help                       Show this message.

The spec schema is documented in `docs/source/sorafs/storage_capacity_marketplace.md`.
"#
}

fn run_declaration<I>(args: I) -> Result<(), String>
where
    I: Iterator<Item = String>,
{
    let mut opts = DeclarationOptions::default();
    for arg in args {
        if arg == "--quiet" {
            opts.quiet = true;
            continue;
        }
        if arg == "--help" {
            println!("{usage}", usage = declaration_usage());
            return Ok(());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value option, got `{arg}`"))?;
        match key {
            "--spec" => opts.spec = Some(PathBuf::from(value)),
            "--registered-epoch" => opts.registered_epoch = Some(parse_u64(value, key)?),
            "--valid-from-epoch" => opts.valid_from_epoch = Some(parse_u64(value, key)?),
            "--valid-until-epoch" => opts.valid_until_epoch = Some(parse_u64(value, key)?),
            "--json-out" => opts.json_out = Some(PathBuf::from(value)),
            "--norito-out" => opts.norito_out = Some(PathBuf::from(value)),
            "--base64-out" => opts.base64_out = Some(PathBuf::from(value)),
            "--request-out" => opts.request_out = Some(PathBuf::from(value)),
            "--authority" => opts.authority = Some(value.to_string()),
            "--private-key" => opts.private_key = Some(value.to_string()),
            "--private-key-file" => opts.private_key_file = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown capacity option `{key}`")),
        }
    }

    let spec_path = opts
        .spec
        .clone()
        .ok_or_else(|| "missing required --spec=<file> option".to_string())?;

    let spec_bytes = fs::read(&spec_path)
        .map_err(|err| format!("failed to read spec `{}`: {err}", spec_path.display()))?;
    let spec_value: Value = norito::json::from_slice(&spec_bytes)
        .map_err(|err| format!("failed to parse spec JSON `{}`: {err}", spec_path.display()))?;

    let artefacts = build_artefacts(spec_value, &opts)?;

    let canonical_bytes = norito::to_bytes(&artefacts.declaration)
        .map_err(|err| format!("failed to encode capacity declaration: {err}"))?;
    let declaration_b64 = BASE64_STD.encode(&canonical_bytes);

    if let Some(path) = opts.norito_out.as_ref() {
        write_binary(path, &canonical_bytes)?;
    }
    if let Some(path) = opts.base64_out.as_ref() {
        write_text(path, &declaration_b64)?;
    }

    let summary = build_declaration_summary(&artefacts, &declaration_b64)?;
    if let Some(path) = opts.json_out.as_ref() {
        let json_text = json::to_string_pretty(&summary)
            .map_err(|err| format!("failed to serialize summary JSON: {err}"))?
            + "\n";
        write_text(path, &json_text)?;
    }

    if let Some(path) = opts.request_out.as_ref() {
        let authority = opts
            .authority
            .as_deref()
            .ok_or_else(|| "--authority is required when using --request-out".to_string())?;
        if authority.is_empty() {
            return Err("--authority must not be empty".to_string());
        }
        let private_key = resolve_private_key(
            opts.private_key.as_deref(),
            opts.private_key_file.as_deref(),
        )?;

        let request_value =
            build_declaration_request_value(&artefacts, &declaration_b64, authority, &private_key)?;
        write_json_file(path, &request_value)?;
    }

    if !opts.quiet && opts.base64_out.is_none() {
        println!("{declaration_b64}");
    }

    Ok(())
}

fn run_telemetry<I>(args: I) -> Result<(), String>
where
    I: Iterator<Item = String>,
{
    let mut opts = TelemetryOptions::default();
    for arg in args {
        if arg == "--quiet" {
            opts.quiet = true;
            continue;
        }
        if arg == "--help" {
            println!("{usage}", usage = telemetry_usage());
            return Ok(());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value option, got `{arg}`"))?;
        match key {
            "--spec" => opts.spec = Some(PathBuf::from(value)),
            "--json-out" => opts.json_out = Some(PathBuf::from(value)),
            "--norito-out" => opts.norito_out = Some(PathBuf::from(value)),
            "--base64-out" => opts.base64_out = Some(PathBuf::from(value)),
            "--request-out" => opts.request_out = Some(PathBuf::from(value)),
            "--authority" => opts.authority = Some(value.to_string()),
            "--private-key" => opts.private_key = Some(value.to_string()),
            "--private-key-file" => opts.private_key_file = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown capacity option `{key}`")),
        }
    }

    let spec_path = opts
        .spec
        .clone()
        .ok_or_else(|| "missing required --spec=<file> option".to_string())?;

    let spec_bytes = fs::read(&spec_path)
        .map_err(|err| format!("failed to read spec `{}`: {err}", spec_path.display()))?;
    let spec_value: Value = norito::json::from_slice(&spec_bytes)
        .map_err(|err| format!("failed to parse spec JSON `{}`: {err}", spec_path.display()))?;

    let telemetry_artefacts = build_telemetry(spec_value)?;

    let canonical_bytes = norito::to_bytes(&telemetry_artefacts.telemetry)
        .map_err(|err| format!("failed to encode capacity telemetry: {err}"))?;
    let telemetry_b64 = BASE64_STD.encode(&canonical_bytes);

    if let Some(path) = opts.norito_out.as_ref() {
        write_binary(path, &canonical_bytes)?;
    }
    if let Some(path) = opts.base64_out.as_ref() {
        write_text(path, &telemetry_b64)?;
    }

    if let Some(path) = opts.json_out.as_ref() {
        let summary = build_telemetry_summary(&telemetry_artefacts, &telemetry_b64)?;
        let json_text = json::to_string_pretty(&summary)
            .map_err(|err| format!("failed to serialize summary JSON: {err}"))?
            + "\n";
        write_text(path, &json_text)?;
    }

    if let Some(path) = opts.request_out.as_ref() {
        let authority = opts
            .authority
            .as_deref()
            .ok_or_else(|| "--authority is required when using --request-out".to_string())?;
        if authority.is_empty() {
            return Err("--authority must not be empty".to_string());
        }
        let private_key = resolve_private_key(
            opts.private_key.as_deref(),
            opts.private_key_file.as_deref(),
        )?;
        let request_value =
            build_telemetry_request_value(&telemetry_artefacts, authority, &private_key)?;
        write_json_file(path, &request_value)?;
    }

    if !opts.quiet && opts.base64_out.is_none() {
        println!("{telemetry_b64}");
    }

    Ok(())
}

fn run_replication_order<I>(args: I) -> Result<(), String>
where
    I: Iterator<Item = String>,
{
    let mut opts = ReplicationOrderOptions::default();
    for arg in args {
        if arg == "--quiet" {
            opts.quiet = true;
            continue;
        }
        if arg == "--help" {
            println!("{usage}", usage = replication_usage());
            return Ok(());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value option, got `{arg}`"))?;
        match key {
            "--spec" => opts.spec = Some(PathBuf::from(value)),
            "--json-out" => opts.json_out = Some(PathBuf::from(value)),
            "--norito-out" => opts.norito_out = Some(PathBuf::from(value)),
            "--base64-out" => opts.base64_out = Some(PathBuf::from(value)),
            "--request-out" => opts.request_out = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown capacity option `{key}`")),
        }
    }

    let spec_path = opts
        .spec
        .clone()
        .ok_or_else(|| "missing required --spec=<file> option".to_string())?;

    let spec_bytes = fs::read(&spec_path)
        .map_err(|err| format!("failed to read spec `{}`: {err}", spec_path.display()))?;
    let spec_value: Value = norito::json::from_slice(&spec_bytes)
        .map_err(|err| format!("failed to parse spec JSON `{}`: {err}", spec_path.display()))?;

    let order = build_replication_order(spec_value)?;

    let canonical_bytes = norito::to_bytes(&order)
        .map_err(|err| format!("failed to encode replication order: {err}"))?;
    let order_b64 = BASE64_STD.encode(&canonical_bytes);

    if let Some(path) = opts.norito_out.as_ref() {
        write_binary(path, &canonical_bytes)?;
    }
    if let Some(path) = opts.base64_out.as_ref() {
        write_text(path, &order_b64)?;
    }

    if let Some(path) = opts.json_out.as_ref() {
        let summary = build_replication_order_summary(&order, &order_b64)?;
        let json_text = json::to_string_pretty(&summary)
            .map_err(|err| format!("failed to serialize summary JSON: {err}"))?
            + "\n";
        write_text(path, &json_text)?;
    }

    if let Some(path) = opts.request_out.as_ref() {
        let request_value = build_replication_order_request_value(&order_b64);
        write_json_file(path, &request_value)?;
    }

    if !opts.quiet && opts.base64_out.is_none() {
        println!("{order_b64}");
    }

    Ok(())
}

fn run_dispute<I>(args: I) -> Result<(), String>
where
    I: Iterator<Item = String>,
{
    let mut opts = DisputeOptions::default();
    for arg in args {
        if arg == "--quiet" {
            opts.quiet = true;
            continue;
        }
        if arg == "--help" {
            println!("{usage}", usage = dispute_usage());
            return Ok(());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value option, got `{arg}`"))?;
        match key {
            "--spec" => opts.spec = Some(PathBuf::from(value)),
            "--json-out" => opts.json_out = Some(PathBuf::from(value)),
            "--norito-out" => opts.norito_out = Some(PathBuf::from(value)),
            "--base64-out" => opts.base64_out = Some(PathBuf::from(value)),
            "--request-out" => opts.request_out = Some(PathBuf::from(value)),
            "--authority" => opts.authority = Some(value.to_string()),
            "--private-key" => opts.private_key = Some(value.to_string()),
            "--private-key-file" => opts.private_key_file = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown capacity option `{key}`")),
        }
    }

    let spec_path = opts
        .spec
        .clone()
        .ok_or_else(|| "missing required --spec=<file> option".to_string())?;

    let spec_bytes = fs::read(&spec_path)
        .map_err(|err| format!("failed to read spec `{}`: {err}", spec_path.display()))?;
    let spec_value: Value = norito::json::from_slice(&spec_bytes)
        .map_err(|err| format!("failed to parse spec JSON `{}`: {err}", spec_path.display()))?;

    let dispute = build_dispute(spec_value)?;

    let canonical_bytes = norito::to_bytes(&dispute)
        .map_err(|err| format!("failed to encode capacity dispute: {err}"))?;
    let dispute_b64 = BASE64_STD.encode(&canonical_bytes);

    if let Some(path) = opts.norito_out.as_ref() {
        write_binary(path, &canonical_bytes)?;
    }
    if let Some(path) = opts.base64_out.as_ref() {
        write_text(path, &dispute_b64)?;
    }

    if let Some(path) = opts.json_out.as_ref() {
        let summary = build_dispute_summary(&dispute, &dispute_b64)?;
        let json_text = json::to_string_pretty(&summary)
            .map_err(|err| format!("failed to serialize summary JSON: {err}"))?
            + "\n";
        write_text(path, &json_text)?;
    }

    if let Some(path) = opts.request_out.as_ref() {
        let authority = opts
            .authority
            .as_deref()
            .ok_or_else(|| "--authority is required when using --request-out".to_string())?;
        if authority.is_empty() {
            return Err("--authority must not be empty".to_string());
        }
        let private_key = resolve_private_key(
            opts.private_key.as_deref(),
            opts.private_key_file.as_deref(),
        )?;
        let request_value =
            build_dispute_request_value(&dispute, &dispute_b64, authority, &private_key)?;
        write_json_file(path, &request_value)?;
    }

    if !opts.quiet && opts.base64_out.is_none() {
        println!("{dispute_b64}");
    }

    Ok(())
}

fn run_complete<I>(args: I) -> Result<(), String>
where
    I: Iterator<Item = String>,
{
    let mut opts = CompleteOptions::default();
    for arg in args {
        if arg == "--quiet" {
            opts.quiet = true;
            continue;
        }
        if arg == "--help" {
            println!("{usage}", usage = complete_usage());
            return Ok(());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value option, got `{arg}`"))?;
        match key {
            "--order-id" => {
                let bytes = parse_fixed_hex::<32>(value, "order_id_hex")?;
                opts.order_id_hex = Some(hex::encode(bytes));
            }
            "--request-out" => opts.request_out = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown capacity option `{key}`")),
        }
    }

    let order_id_hex = opts
        .order_id_hex
        .clone()
        .ok_or_else(|| "missing required --order-id=<hex> option".to_string())?;

    let request_value = build_complete_request_value(&order_id_hex);
    if let Some(path) = opts.request_out.as_ref() {
        write_json_file(path, &request_value)?;
    }
    if !opts.quiet && opts.request_out.is_none() {
        write_json_file(Path::new("-"), &request_value)?;
    }

    Ok(())
}

#[derive(Debug, Default)]
struct DeclarationOptions {
    spec: Option<PathBuf>,
    registered_epoch: Option<u64>,
    valid_from_epoch: Option<u64>,
    valid_until_epoch: Option<u64>,
    json_out: Option<PathBuf>,
    norito_out: Option<PathBuf>,
    base64_out: Option<PathBuf>,
    request_out: Option<PathBuf>,
    authority: Option<String>,
    private_key: Option<String>,
    private_key_file: Option<PathBuf>,
    quiet: bool,
}

struct DeclarationArtefacts {
    declaration: CapacityDeclarationV1,
    metadata: Vec<CapacityMetadataEntry>,
    registered_epoch: u64,
    valid_from_epoch: u64,
    valid_until_epoch: u64,
}

#[derive(Debug, Default)]
struct TelemetryOptions {
    spec: Option<PathBuf>,
    json_out: Option<PathBuf>,
    norito_out: Option<PathBuf>,
    base64_out: Option<PathBuf>,
    request_out: Option<PathBuf>,
    authority: Option<String>,
    private_key: Option<String>,
    private_key_file: Option<PathBuf>,
    quiet: bool,
}

struct TelemetryArtefacts {
    telemetry: CapacityTelemetryV1,
    effective_gib: u64,
}

#[derive(Debug, Default)]
struct ReplicationOrderOptions {
    spec: Option<PathBuf>,
    json_out: Option<PathBuf>,
    norito_out: Option<PathBuf>,
    base64_out: Option<PathBuf>,
    request_out: Option<PathBuf>,
    quiet: bool,
}

#[derive(Debug, Default)]
struct CompleteOptions {
    order_id_hex: Option<String>,
    request_out: Option<PathBuf>,
    quiet: bool,
}

#[derive(Debug, Default)]
struct DisputeOptions {
    spec: Option<PathBuf>,
    json_out: Option<PathBuf>,
    norito_out: Option<PathBuf>,
    base64_out: Option<PathBuf>,
    request_out: Option<PathBuf>,
    authority: Option<String>,
    private_key: Option<String>,
    private_key_file: Option<PathBuf>,
    quiet: bool,
}

fn build_artefacts(
    value: Value,
    opts: &DeclarationOptions,
) -> Result<DeclarationArtefacts, String> {
    let map = value
        .as_object()
        .ok_or_else(|| "capacity declaration spec must be a JSON object".to_string())?;

    let provider_id_hex = require_string(map, "provider_id_hex")?;
    let provider_id = parse_fixed_hex::<32>(provider_id_hex, "provider_id_hex")?;

    let stake_value = require_object(map, "stake")?;
    let pool_id_hex = require_string(stake_value, "pool_id_hex")?;
    let pool_id = parse_fixed_hex::<32>(pool_id_hex, "stake.pool_id_hex")?;
    let stake_amount_value = stake_value
        .get("stake_amount")
        .ok_or_else(|| "missing `stake.stake_amount` field".to_string())?;
    let stake_amount = parse_xor_quantity_value(stake_amount_value, "stake.stake_amount")?;

    let committed_capacity =
        parse_u64_value(map.get("committed_capacity_gib"), "committed_capacity_gib")?;

    let chunker_commitments_value = require_array(map, "chunker_commitments")?;
    if chunker_commitments_value.is_empty() {
        return Err("chunker_commitments must contain at least one entry".into());
    }
    let mut chunker_commitments = Vec::with_capacity(chunker_commitments_value.len());
    for (idx, entry) in chunker_commitments_value.iter().enumerate() {
        let entry_obj = entry
            .as_object()
            .ok_or_else(|| format!("chunker_commitments[{idx}] must be an object"))?;
        let handle = require_string(entry_obj, "profile_handle")?;
        let descriptor = chunker_registry::lookup_by_handle(handle).ok_or_else(|| {
            format!(
                "unknown chunker profile handle `{handle}`; see docs/source/sorafs/chunker_registry.md"
            )
        })?;
        let canonical_handle = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        let profile_id = canonical_handle;
        let profile_aliases = entry_obj
            .get("profile_aliases")
            .map(|value| parse_string_array(value, "chunker_commitments.profile_aliases"))
            .transpose()?;
        let committed_gib = parse_u64_value(
            entry_obj.get("committed_gib"),
            "chunker_commitments.committed_gib",
        )?;
        let capability_refs = match entry_obj.get("capability_refs") {
            Some(value) => parse_capability_array(value)?,
            None => Vec::new(),
        };
        chunker_commitments.push(ChunkerCommitmentV1 {
            profile_id,
            profile_aliases,
            committed_gib,
            capability_refs,
        });
    }

    let lane_commitments_value = map.get("lane_commitments");
    let lane_commitments = if let Some(value) = lane_commitments_value {
        let array = value
            .as_array()
            .ok_or_else(|| "lane_commitments must be an array when provided".to_string())?;
        let mut lanes = Vec::with_capacity(array.len());
        for (idx, lane) in array.iter().enumerate() {
            let lane_obj = lane
                .as_object()
                .ok_or_else(|| format!("lane_commitments[{idx}] must be an object"))?;
            let lane_id = require_string(lane_obj, "lane_id")?.to_owned();
            let max_gib = parse_u64_value(lane_obj.get("max_gib"), "lane_commitments.max_gib")?;
            lanes.push(LaneCommitmentV1 { lane_id, max_gib });
        }
        lanes
    } else {
        Vec::new()
    };

    let pricing = map.get("pricing").map(parse_pricing).transpose()?;

    let valid_from = parse_u64_value(map.get("valid_from"), "valid_from")?;
    let valid_until = parse_u64_value(map.get("valid_until"), "valid_until")?;

    if valid_from > valid_until {
        return Err("valid_from must not exceed valid_until".into());
    }

    let metadata_entries = map
        .get("metadata")
        .map(parse_metadata_entries)
        .transpose()?
        .unwrap_or_default();

    let declaration = CapacityDeclarationV1 {
        version: CAPACITY_DECLARATION_VERSION_V1,
        provider_id,
        stake: StakePointer {
            pool_id,
            stake_amount,
        },
        committed_capacity_gib: committed_capacity,
        chunker_commitments,
        lane_commitments,
        pricing,
        valid_from,
        valid_until,
        metadata: metadata_entries.clone(),
    };

    declaration
        .validate()
        .map_err(|err| format!("capacity declaration validation failed: {err}"))?;

    let (registered_epoch, valid_from_epoch, valid_until_epoch) =
        parse_record_window(map, opts, valid_from, valid_until)?;

    Ok(DeclarationArtefacts {
        declaration,
        metadata: metadata_entries,
        registered_epoch,
        valid_from_epoch,
        valid_until_epoch,
    })
}

fn parse_record_window(
    map: &Map,
    opts: &DeclarationOptions,
    default_from: u64,
    default_until: u64,
) -> Result<(u64, u64, u64), String> {
    if let Some(window_value) = map.get("record_window") {
        let window = window_value
            .as_object()
            .ok_or_else(|| "`record_window` must be an object when provided".to_string())?;
        let registered = parse_u64_value(
            window.get("registered_epoch"),
            "record_window.registered_epoch",
        )?;
        let valid_from = parse_u64_value(
            window.get("valid_from_epoch"),
            "record_window.valid_from_epoch",
        )?;
        let valid_until = parse_u64_value(
            window.get("valid_until_epoch"),
            "record_window.valid_until_epoch",
        )?;
        return Ok((
            opts.registered_epoch.unwrap_or(registered),
            opts.valid_from_epoch.unwrap_or(valid_from),
            opts.valid_until_epoch.unwrap_or(valid_until),
        ));
    }

    let registered = opts.registered_epoch.unwrap_or(default_from);
    let valid_from_epoch = opts.valid_from_epoch.unwrap_or(default_from);
    let valid_until_epoch = opts.valid_until_epoch.unwrap_or(default_until);
    Ok((registered, valid_from_epoch, valid_until_epoch))
}

fn parse_capability_array(value: &Value) -> Result<Vec<CapabilityType>, String> {
    let array = value
        .as_array()
        .ok_or_else(|| "`capability_refs` must be an array of strings".to_string())?;
    let mut out = Vec::with_capacity(array.len());
    for item in array {
        let name = item
            .as_str()
            .ok_or_else(|| "capability_refs entries must be strings".to_string())?;
        let capability = parse_capability_name(name).ok_or_else(|| {
            format!(
                "unknown capability name `{name}`; expected one of \
                 torii_gateway, quic_noise, soranet, soranet_pq, chunk_range_fetch, vendor_reserved"
            )
        })?;
        out.push(capability);
    }
    Ok(out)
}

fn parse_capability_name(name: &str) -> Option<CapabilityType> {
    match name {
        "torii" | "torii_gateway" => Some(CapabilityType::ToriiGateway),
        "quic" | "quic_noise" => Some(CapabilityType::QuicNoise),
        "soranet" | "soranet_pq" | "soranet-pq" | "soranet_hybrid_pq" => {
            Some(CapabilityType::SoraNetHybridPq)
        }
        "range" | "chunk_range_fetch" => Some(CapabilityType::ChunkRangeFetch),
        "vendor" | "vendor_reserved" => Some(CapabilityType::VendorReserved),
        _ => None,
    }
}

fn parse_pricing(value: &Value) -> Result<PricingScheduleV1, String> {
    let obj = value
        .as_object()
        .ok_or_else(|| "`pricing` must be an object".to_string())?;
    let currency = require_string(obj, "currency")?.to_owned();
    let rate = parse_u64_value(
        obj.get("rate_per_gib_hour_milliu"),
        "pricing.rate_per_gib_hour_milliu",
    )?;
    let min_commitment_hours = match obj.get("min_commitment_hours") {
        Some(v) => Some(parse_u32_value(v, "pricing.min_commitment_hours")?),
        None => None,
    };
    let notes = obj
        .get("notes")
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| "pricing.notes must be a string".to_string())
                .map(|s| s.to_owned())
        })
        .transpose()?;
    let pricing = PricingScheduleV1 {
        currency,
        rate_per_gib_hour_milliu: rate,
        min_commitment_hours,
        notes,
    };
    Ok(pricing)
}

fn parse_metadata_entries(value: &Value) -> Result<Vec<CapacityMetadataEntry>, String> {
    let obj = value
        .as_object()
        .ok_or_else(|| "`metadata` must be an object of string values".to_string())?;
    let mut entries = Vec::with_capacity(obj.len());
    for (key, val) in obj {
        let value = val
            .as_str()
            .ok_or_else(|| format!("metadata entry `{key}` must be a string value"))?;
        entries.push(CapacityMetadataEntry {
            key: key.clone(),
            value: value.to_owned(),
        });
    }
    Ok(entries)
}

fn build_declaration_summary(
    artefacts: &DeclarationArtefacts,
    declaration_b64: &str,
) -> Result<Value, String> {
    let mut root = Map::new();
    root.insert(
        "provider_id_hex".into(),
        Value::String(hex::encode(artefacts.declaration.provider_id)),
    );
    root.insert(
        "stake_pool_hex".into(),
        Value::String(hex::encode(artefacts.declaration.stake.pool_id)),
    );
    root.insert(
        "stake_amount".into(),
        Value::String(artefacts.declaration.stake.stake_amount.to_string()),
    );
    root.insert(
        "committed_capacity_gib".into(),
        json::to_value(&artefacts.declaration.committed_capacity_gib)
            .map_err(|err| format!("failed to serialize committed_capacity_gib: {err}"))?,
    );
    root.insert(
        "valid_from".into(),
        json::to_value(&artefacts.declaration.valid_from)
            .map_err(|err| format!("failed to serialize valid_from: {err}"))?,
    );
    root.insert(
        "valid_until".into(),
        json::to_value(&artefacts.declaration.valid_until)
            .map_err(|err| format!("failed to serialize valid_until: {err}"))?,
    );
    root.insert(
        "registered_epoch".into(),
        json::to_value(&artefacts.registered_epoch)
            .map_err(|err| format!("failed to serialize registered_epoch: {err}"))?,
    );
    root.insert(
        "valid_from_epoch".into(),
        json::to_value(&artefacts.valid_from_epoch)
            .map_err(|err| format!("failed to serialize valid_from_epoch: {err}"))?,
    );
    root.insert(
        "valid_until_epoch".into(),
        json::to_value(&artefacts.valid_until_epoch)
            .map_err(|err| format!("failed to serialize valid_until_epoch: {err}"))?,
    );
    root.insert(
        "declaration_b64".into(),
        Value::String(declaration_b64.to_owned()),
    );

    let mut commitments = Vec::with_capacity(artefacts.declaration.chunker_commitments.len());
    for commitment in &artefacts.declaration.chunker_commitments {
        let mut item = Map::new();
        item.insert(
            "profile_handle".into(),
            Value::String(commitment.profile_id.clone()),
        );
        item.insert(
            "committed_gib".into(),
            json::to_value(&commitment.committed_gib)
                .map_err(|err| format!("failed to serialize committed_gib: {err}"))?,
        );
        if let Some(aliases) = &commitment.profile_aliases {
            item.insert(
                "profile_aliases".into(),
                json::to_value(aliases)
                    .map_err(|err| format!("failed to serialize profile_aliases: {err}"))?,
            );
        }
        if !commitment.capability_refs.is_empty() {
            let caps = commitment
                .capability_refs
                .iter()
                .map(capability_label)
                .map(|label| Value::String(label.to_string()))
                .collect::<Vec<_>>();
            item.insert("capability_refs".into(), Value::Array(caps));
        }
        commitments.push(Value::Object(item));
    }
    root.insert("chunker_commitments".into(), Value::Array(commitments));

    if !artefacts.declaration.lane_commitments.is_empty() {
        let lanes = artefacts
            .declaration
            .lane_commitments
            .iter()
            .map(|lane| {
                let mut entry = Map::new();
                entry.insert("lane_id".into(), Value::String(lane.lane_id.clone()));
                entry.insert(
                    "max_gib".into(),
                    json::to_value(&lane.max_gib)
                        .map_err(|err| format!("failed to serialize lane max_gib: {err}"))?,
                );
                Ok(Value::Object(entry))
            })
            .collect::<Result<Vec<_>, String>>()?;
        root.insert("lane_commitments".into(), Value::Array(lanes));
    }

    if let Some(pricing) = &artefacts.declaration.pricing {
        let mut pricing_map = Map::new();
        pricing_map.insert("currency".into(), Value::String(pricing.currency.clone()));
        pricing_map.insert(
            "rate_per_gib_hour_milliu".into(),
            json::to_value(&pricing.rate_per_gib_hour_milliu)
                .map_err(|err| format!("failed to serialize pricing rate: {err}"))?,
        );
        pricing_map.insert(
            "min_commitment_hours".into(),
            json::to_value(&pricing.min_commitment_hours)
                .map_err(|err| format!("failed to serialize pricing min commitment: {err}"))?,
        );
        pricing_map.insert(
            "notes".into(),
            json::to_value(&pricing.notes)
                .map_err(|err| format!("failed to serialize pricing notes: {err}"))?,
        );
        root.insert("pricing".into(), Value::Object(pricing_map));
    }

    if !artefacts.metadata.is_empty() {
        let mut metadata_map = Map::new();
        for entry in &artefacts.metadata {
            metadata_map.insert(entry.key.clone(), Value::String(entry.value.clone()));
        }
        root.insert("metadata".into(), Value::Object(metadata_map));
    }

    Ok(Value::Object(root))
}

fn build_declaration_request_value(
    artefacts: &DeclarationArtefacts,
    declaration_b64: &str,
    authority: &str,
    private_key: &str,
) -> Result<Value, String> {
    let mut root = Map::new();
    root.insert("authority".into(), Value::String(authority.to_owned()));
    root.insert("private_key".into(), Value::String(private_key.to_owned()));
    root.insert(
        "declaration_b64".into(),
        Value::String(declaration_b64.to_owned()),
    );
    root.insert(
        "registered_epoch".into(),
        json::to_value(&artefacts.registered_epoch)
            .map_err(|err| format!("failed to serialize registered_epoch: {err}"))?,
    );
    root.insert(
        "valid_from_epoch".into(),
        json::to_value(&artefacts.valid_from_epoch)
            .map_err(|err| format!("failed to serialize valid_from_epoch: {err}"))?,
    );
    root.insert(
        "valid_until_epoch".into(),
        json::to_value(&artefacts.valid_until_epoch)
            .map_err(|err| format!("failed to serialize valid_until_epoch: {err}"))?,
    );

    let metadata = artefacts
        .metadata
        .iter()
        .map(|entry| {
            let mut item = Map::new();
            item.insert("key".into(), Value::String(entry.key.clone()));
            item.insert("value".into(), Value::String(entry.value.clone()));
            Value::Object(item)
        })
        .collect::<Vec<_>>();
    root.insert("metadata".into(), Value::Array(metadata));

    Ok(Value::Object(root))
}

fn build_telemetry(telemetry_value: Value) -> Result<TelemetryArtefacts, String> {
    let map = telemetry_value
        .as_object()
        .ok_or_else(|| "capacity telemetry spec must be a JSON object".to_string())?;

    let provider_id_hex = require_string(map, "provider_id_hex")?;
    let provider_id = parse_fixed_hex::<32>(provider_id_hex, "provider_id_hex")?;
    let epoch_start = parse_u64_value(map.get("epoch_start"), "epoch_start")?;
    let epoch_end = parse_u64_value(map.get("epoch_end"), "epoch_end")?;
    let declared_capacity =
        parse_u64_value(map.get("declared_capacity_gib"), "declared_capacity_gib")?;
    let utilised_capacity =
        parse_u64_value(map.get("utilised_capacity_gib"), "utilised_capacity_gib")?;

    let successful_replications = parse_u32_value(
        require_value(map, "successful_replications")?,
        "successful_replications",
    )?;
    let failed_replications = parse_u32_value(
        require_value(map, "failed_replications")?,
        "failed_replications",
    )?;
    let uptime_percent = parse_u32_value(
        require_value(map, "uptime_percent_milli")?,
        "uptime_percent_milli",
    )?;
    let por_percent = parse_u32_value(
        require_value(map, "por_success_percent_milli")?,
        "por_success_percent_milli",
    )?;

    let effective_override = match (map.get("effective_capacity_gib"), map.get("effective_gib")) {
        (Some(_), Some(_)) => {
            return Err(
                "spec must not set both `effective_capacity_gib` and `effective_gib`".to_string(),
            );
        }
        (Some(value), None) => Some(parse_u64_value(Some(value), "effective_capacity_gib")?),
        (None, Some(value)) => Some(parse_u64_value(Some(value), "effective_gib")?),
        (None, None) => None,
    };

    let notes = match map.get("notes") {
        Some(value) => Some(
            value
                .as_str()
                .ok_or_else(|| "notes must be a string".to_string())?
                .to_owned(),
        ),
        None => None,
    };

    let effective_capacity = effective_override.unwrap_or(utilised_capacity);
    if effective_capacity > declared_capacity {
        return Err(format!(
            "`effective_capacity_gib` ({effective_capacity}) must not exceed `declared_capacity_gib` ({declared_capacity})"
        ));
    }
    if utilised_capacity > effective_capacity {
        return Err(format!(
            "`utilised_capacity_gib` ({utilised_capacity}) must not exceed effective capacity ({effective_capacity})"
        ));
    }

    let telemetry = CapacityTelemetryV1 {
        version: CAPACITY_TELEMETRY_VERSION_V1,
        provider_id,
        epoch_start,
        epoch_end,
        declared_capacity_gib: declared_capacity,
        utilised_capacity_gib: utilised_capacity,
        successful_replications,
        failed_replications,
        uptime_percent_milli: uptime_percent,
        por_success_percent_milli: por_percent,
        notes,
    };

    telemetry
        .validate()
        .map_err(|err| format!("capacity telemetry validation failed: {err}"))?;
    Ok(TelemetryArtefacts {
        telemetry,
        effective_gib: effective_capacity,
    })
}

fn build_telemetry_summary(
    artefacts: &TelemetryArtefacts,
    telemetry_b64: &str,
) -> Result<Value, String> {
    let telemetry = &artefacts.telemetry;
    let mut root = Map::new();
    root.insert(
        "provider_id_hex".into(),
        Value::String(hex::encode(telemetry.provider_id)),
    );
    root.insert(
        "epoch_start".into(),
        json::to_value(&telemetry.epoch_start)
            .map_err(|err| format!("failed to serialize epoch_start: {err}"))?,
    );
    root.insert(
        "epoch_end".into(),
        json::to_value(&telemetry.epoch_end)
            .map_err(|err| format!("failed to serialize epoch_end: {err}"))?,
    );
    root.insert(
        "declared_capacity_gib".into(),
        json::to_value(&telemetry.declared_capacity_gib)
            .map_err(|err| format!("failed to serialize declared capacity: {err}"))?,
    );
    root.insert(
        "effective_capacity_gib".into(),
        json::to_value(&artefacts.effective_gib)
            .map_err(|err| format!("failed to serialize effective capacity: {err}"))?,
    );
    root.insert(
        "utilised_capacity_gib".into(),
        json::to_value(&telemetry.utilised_capacity_gib)
            .map_err(|err| format!("failed to serialize utilised capacity: {err}"))?,
    );
    root.insert(
        "successful_replications".into(),
        json::to_value(&telemetry.successful_replications)
            .map_err(|err| format!("failed to serialize successful_replications: {err}"))?,
    );
    root.insert(
        "failed_replications".into(),
        json::to_value(&telemetry.failed_replications)
            .map_err(|err| format!("failed to serialize failed_replications: {err}"))?,
    );
    root.insert(
        "uptime_percent_milli".into(),
        json::to_value(&telemetry.uptime_percent_milli)
            .map_err(|err| format!("failed to serialize uptime_percent_milli: {err}"))?,
    );
    root.insert(
        "por_success_percent_milli".into(),
        json::to_value(&telemetry.por_success_percent_milli)
            .map_err(|err| format!("failed to serialize por_success_percent_milli: {err}"))?,
    );
    root.insert(
        "notes".into(),
        json::to_value(&telemetry.notes)
            .map_err(|err| format!("failed to serialize notes: {err}"))?,
    );
    root.insert(
        "telemetry_b64".into(),
        Value::String(telemetry_b64.to_owned()),
    );
    Ok(Value::Object(root))
}

fn build_telemetry_request_value(
    artefacts: &TelemetryArtefacts,
    authority: &str,
    private_key: &str,
) -> Result<Value, String> {
    let telemetry = &artefacts.telemetry;
    let mut root = Map::new();
    root.insert("authority".into(), Value::String(authority.to_owned()));
    root.insert("private_key".into(), Value::String(private_key.to_owned()));
    root.insert(
        "provider_id_hex".into(),
        Value::String(hex::encode(telemetry.provider_id)),
    );
    root.insert(
        "window_start_epoch".into(),
        json::to_value(&telemetry.epoch_start)
            .map_err(|err| format!("failed to serialize window_start_epoch: {err}"))?,
    );
    root.insert(
        "window_end_epoch".into(),
        json::to_value(&telemetry.epoch_end)
            .map_err(|err| format!("failed to serialize window_end_epoch: {err}"))?,
    );

    let declared_gib = telemetry.declared_capacity_gib;
    let effective_gib = artefacts.effective_gib;
    let utilised_gib = telemetry.utilised_capacity_gib;
    let orders_completed = u64::from(telemetry.successful_replications);
    let orders_issued = orders_completed + u64::from(telemetry.failed_replications);
    let uptime_bps = (telemetry.uptime_percent_milli + 5) / 10;
    let por_success_bps = (telemetry.por_success_percent_milli + 5) / 10;

    root.insert(
        "declared_gib".into(),
        json::to_value(&declared_gib)
            .map_err(|err| format!("failed to serialize declared_gib: {err}"))?,
    );
    root.insert(
        "effective_gib".into(),
        json::to_value(&effective_gib)
            .map_err(|err| format!("failed to serialize effective_gib: {err}"))?,
    );
    root.insert(
        "utilised_gib".into(),
        json::to_value(&utilised_gib)
            .map_err(|err| format!("failed to serialize utilised_gib: {err}"))?,
    );
    root.insert(
        "orders_issued".into(),
        json::to_value(&orders_issued)
            .map_err(|err| format!("failed to serialize orders_issued: {err}"))?,
    );
    root.insert(
        "orders_completed".into(),
        json::to_value(&orders_completed)
            .map_err(|err| format!("failed to serialize orders_completed: {err}"))?,
    );
    root.insert(
        "uptime_bps".into(),
        json::to_value(&uptime_bps)
            .map_err(|err| format!("failed to serialize uptime_bps: {err}"))?,
    );
    root.insert(
        "por_success_bps".into(),
        json::to_value(&por_success_bps)
            .map_err(|err| format!("failed to serialize por_success_bps: {err}"))?,
    );

    Ok(Value::Object(root))
}

fn build_replication_order(order_value: Value) -> Result<ReplicationOrderV1, String> {
    let map = order_value
        .as_object()
        .ok_or_else(|| "replication order spec must be a JSON object".to_string())?;

    let order_id_hex = require_string(map, "order_id_hex")?;
    let order_id = parse_fixed_hex::<32>(order_id_hex, "order_id_hex")?;

    let manifest_cid_hex = require_string(map, "manifest_cid_hex")?;
    let manifest_cid = parse_vec_hex(manifest_cid_hex, "manifest_cid_hex")?;

    let manifest_digest_hex = require_string(map, "manifest_digest_hex")?;
    let manifest_digest = parse_fixed_hex::<32>(manifest_digest_hex, "manifest_digest_hex")?;

    let chunking_profile = require_string(map, "chunking_profile")?.to_owned();
    let target_replicas =
        parse_u16_value(require_value(map, "target_replicas")?, "target_replicas")?;

    let assignments_value = require_array(map, "assignments")?;
    if assignments_value.is_empty() {
        return Err("assignments must contain at least one entry".into());
    }
    let mut assignments = Vec::with_capacity(assignments_value.len());
    for (idx, value) in assignments_value.iter().enumerate() {
        let obj = value
            .as_object()
            .ok_or_else(|| format!("assignments[{idx}] must be an object"))?;
        let provider_hex = require_string(obj, "provider_id_hex")?;
        let provider_id = parse_fixed_hex::<32>(provider_hex, "assignments.provider_id_hex")?;
        let slice_gib = parse_u64_value(obj.get("slice_gib"), "assignments.slice_gib")?;
        let lane = obj
            .get("lane")
            .map(|lane_value| {
                lane_value
                    .as_str()
                    .ok_or_else(|| "assignments.lane must be a string".to_string())
                    .map(|s| s.to_owned())
            })
            .transpose()?;
        assignments.push(ReplicationAssignmentV1 {
            provider_id,
            slice_gib,
            lane,
        });
    }

    let issued_at = parse_u64_value(map.get("issued_at"), "issued_at")?;
    let deadline_at = parse_u64_value(map.get("deadline_at"), "deadline_at")?;

    let sla_obj = require_object(map, "sla")?;
    let ingest_deadline = parse_u32_value(
        require_value(sla_obj, "ingest_deadline_secs")?,
        "sla.ingest_deadline_secs",
    )?;
    let min_availability = parse_u32_value(
        require_value(sla_obj, "min_availability_percent_milli")?,
        "sla.min_availability_percent_milli",
    )?;
    let min_por = parse_u32_value(
        require_value(sla_obj, "min_por_success_percent_milli")?,
        "sla.min_por_success_percent_milli",
    )?;
    let sla = ReplicationOrderSlaV1 {
        ingest_deadline_secs: ingest_deadline,
        min_availability_percent_milli: min_availability,
        min_por_success_percent_milli: min_por,
    };

    let metadata_entries = map
        .get("metadata")
        .map(parse_metadata_entries)
        .transpose()?
        .unwrap_or_default();

    let order = ReplicationOrderV1 {
        version: REPLICATION_ORDER_VERSION_V1,
        order_id,
        manifest_cid,
        manifest_digest,
        chunking_profile,
        target_replicas,
        assignments,
        issued_at,
        deadline_at,
        sla,
        metadata: metadata_entries,
    };

    order
        .validate()
        .map_err(|err| format!("replication order validation failed: {err}"))?;
    Ok(order)
}

fn build_replication_order_summary(
    order: &ReplicationOrderV1,
    order_b64: &str,
) -> Result<Value, String> {
    let mut root = Map::new();
    root.insert(
        "order_id_hex".into(),
        Value::String(hex::encode(order.order_id)),
    );
    root.insert(
        "manifest_cid_hex".into(),
        Value::String(hex::encode(&order.manifest_cid)),
    );
    root.insert(
        "manifest_digest_hex".into(),
        Value::String(hex::encode(order.manifest_digest)),
    );
    root.insert(
        "chunking_profile".into(),
        Value::String(order.chunking_profile.clone()),
    );
    root.insert(
        "target_replicas".into(),
        json::to_value(&order.target_replicas)
            .map_err(|err| format!("failed to serialize target_replicas: {err}"))?,
    );

    let assignments = order
        .assignments
        .iter()
        .map(|assignment| {
            let mut entry = Map::new();
            entry.insert(
                "provider_id_hex".into(),
                Value::String(hex::encode(assignment.provider_id)),
            );
            entry.insert(
                "slice_gib".into(),
                json::to_value(&assignment.slice_gib)
                    .map_err(|err| format!("failed to serialize slice_gib: {err}"))?,
            );
            entry.insert(
                "lane".into(),
                json::to_value(&assignment.lane)
                    .map_err(|err| format!("failed to serialize lane: {err}"))?,
            );
            Ok(Value::Object(entry))
        })
        .collect::<Result<Vec<_>, String>>()?;
    root.insert("assignments".into(), Value::Array(assignments));

    root.insert(
        "issued_at".into(),
        json::to_value(&order.issued_at)
            .map_err(|err| format!("failed to serialize issued_at: {err}"))?,
    );
    root.insert(
        "deadline_at".into(),
        json::to_value(&order.deadline_at)
            .map_err(|err| format!("failed to serialize deadline_at: {err}"))?,
    );

    let mut sla_map = Map::new();
    sla_map.insert(
        "ingest_deadline_secs".into(),
        json::to_value(&order.sla.ingest_deadline_secs)
            .map_err(|err| format!("failed to serialize ingest_deadline_secs: {err}"))?,
    );
    sla_map.insert(
        "min_availability_percent_milli".into(),
        json::to_value(&order.sla.min_availability_percent_milli)
            .map_err(|err| format!("failed to serialize min_availability_percent_milli: {err}"))?,
    );
    sla_map.insert(
        "min_por_success_percent_milli".into(),
        json::to_value(&order.sla.min_por_success_percent_milli)
            .map_err(|err| format!("failed to serialize min_por_success_percent_milli: {err}"))?,
    );
    root.insert("sla".into(), Value::Object(sla_map));

    if !order.metadata.is_empty() {
        let mut metadata_map = Map::new();
        for entry in &order.metadata {
            metadata_map.insert(entry.key.clone(), Value::String(entry.value.clone()));
        }
        root.insert("metadata".into(), Value::Object(metadata_map));
    } else {
        root.insert("metadata".into(), Value::Null);
    }

    root.insert(
        "replication_order_b64".into(),
        Value::String(order_b64.to_owned()),
    );
    Ok(Value::Object(root))
}

fn build_replication_order_request_value(order_b64: &str) -> Value {
    let mut root = Map::new();
    root.insert("order_b64".into(), Value::String(order_b64.to_owned()));
    Value::Object(root)
}

fn build_dispute(dispute_value: Value) -> Result<CapacityDisputeV1, String> {
    let map = dispute_value
        .as_object()
        .ok_or_else(|| "capacity dispute spec must be a JSON object".to_string())?;

    let provider_id_hex = require_string(map, "provider_id_hex")?;
    let provider_id = parse_fixed_hex::<32>(provider_id_hex, "provider_id_hex")?;

    let complainant_id_hex = require_string(map, "complainant_id_hex")?;
    let complainant_id = parse_fixed_hex::<32>(complainant_id_hex, "complainant_id_hex")?;

    let replication_order_id = match map.get("replication_order_id_hex") {
        Some(value) => {
            let text = value
                .as_str()
                .ok_or_else(|| "`replication_order_id_hex` must be a string".to_string())?;
            Some(parse_fixed_hex::<32>(text, "replication_order_id_hex")?)
        }
        None => None,
    };

    let kind_value = require_string(map, "kind")?;
    let kind = parse_dispute_kind(kind_value)?;

    let submitted_epoch = parse_u64_value(map.get("submitted_epoch"), "submitted_epoch")?;

    let description = require_string(map, "description")?.to_owned();

    let requested_remedy = match map.get("requested_remedy") {
        Some(value) => {
            let text = value
                .as_str()
                .ok_or_else(|| "`requested_remedy` must be a string when provided".to_string())?;
            Some(text.to_owned())
        }
        None => None,
    };

    let evidence_value = require_object(map, "evidence")?;
    let digest_hex = require_string(evidence_value, "digest_hex")?;
    let evidence_digest = parse_fixed_hex::<32>(digest_hex, "evidence.digest_hex")?;
    let media_type = evidence_value
        .get("media_type")
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| "`evidence.media_type` must be a string".to_string())
                .map(ToOwned::to_owned)
        })
        .transpose()?;
    let uri = evidence_value
        .get("uri")
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| "`evidence.uri` must be a string".to_string())
                .map(ToOwned::to_owned)
        })
        .transpose()?;
    let size_bytes = match evidence_value.get("size_bytes") {
        Some(value) => Some(parse_u64_value(Some(value), "evidence.size_bytes")?),
        None => None,
    };

    let evidence = CapacityDisputeEvidenceV1 {
        evidence_digest,
        media_type,
        uri,
        size_bytes,
    };

    let dispute = CapacityDisputeV1 {
        version: CAPACITY_DISPUTE_VERSION_V1,
        provider_id,
        complainant_id,
        replication_order_id,
        kind,
        evidence,
        submitted_epoch,
        description,
        requested_remedy,
    };

    dispute
        .validate()
        .map_err(|err| format!("capacity dispute validation failed: {err}"))?;
    Ok(dispute)
}

fn build_dispute_summary(dispute: &CapacityDisputeV1, dispute_b64: &str) -> Result<Value, String> {
    let mut root = Map::new();
    root.insert(
        "provider_id_hex".into(),
        Value::String(hex::encode(dispute.provider_id)),
    );
    root.insert(
        "complainant_id_hex".into(),
        Value::String(hex::encode(dispute.complainant_id)),
    );
    root.insert(
        "kind".into(),
        Value::String(dispute_kind_to_str(dispute.kind).to_owned()),
    );
    if let Some(order_id) = dispute.replication_order_id {
        root.insert(
            "replication_order_id_hex".into(),
            Value::String(hex::encode(order_id)),
        );
    } else {
        root.insert("replication_order_id_hex".into(), Value::Null);
    }
    root.insert(
        "submitted_epoch".into(),
        json::to_value(&dispute.submitted_epoch)
            .map_err(|err| format!("failed to serialize submitted_epoch: {err}"))?,
    );
    root.insert(
        "description".into(),
        Value::String(dispute.description.clone()),
    );
    if let Some(remedy) = &dispute.requested_remedy {
        root.insert("requested_remedy".into(), Value::String(remedy.clone()));
    } else {
        root.insert("requested_remedy".into(), Value::Null);
    }

    let mut evidence_map = Map::new();
    evidence_map.insert(
        "digest_hex".into(),
        Value::String(hex::encode(dispute.evidence.evidence_digest)),
    );
    evidence_map.insert(
        "media_type".into(),
        dispute
            .evidence
            .media_type
            .as_ref()
            .map(|value| Value::String(value.clone()))
            .unwrap_or(Value::Null),
    );
    evidence_map.insert(
        "uri".into(),
        dispute
            .evidence
            .uri
            .as_ref()
            .map(|value| Value::String(value.clone()))
            .unwrap_or(Value::Null),
    );
    let size_value = if let Some(value) = dispute.evidence.size_bytes {
        json::to_value(&value)
            .map_err(|err| format!("failed to serialize evidence.size_bytes: {err}"))?
    } else {
        Value::Null
    };
    evidence_map.insert("size_bytes".into(), size_value);
    root.insert("evidence".into(), Value::Object(evidence_map));

    root.insert("dispute_b64".into(), Value::String(dispute_b64.to_owned()));

    Ok(Value::Object(root))
}

fn build_dispute_request_value(
    dispute: &CapacityDisputeV1,
    dispute_b64: &str,
    authority: &str,
    private_key: &str,
) -> Result<Value, String> {
    let mut root = Map::new();
    root.insert("authority".into(), Value::String(authority.to_owned()));
    root.insert("private_key".into(), Value::String(private_key.to_owned()));
    root.insert("dispute_b64".into(), Value::String(dispute_b64.to_owned()));
    root.insert(
        "submitted_epoch".into(),
        json::to_value(&dispute.submitted_epoch)
            .map_err(|err| format!("failed to serialize submitted_epoch: {err}"))?,
    );
    root.insert(
        "provider_id_hex".into(),
        Value::String(hex::encode(dispute.provider_id)),
    );
    root.insert(
        "complainant_id_hex".into(),
        Value::String(hex::encode(dispute.complainant_id)),
    );
    if let Some(order_id) = dispute.replication_order_id {
        root.insert(
            "replication_order_id_hex".into(),
            Value::String(hex::encode(order_id)),
        );
    }
    root.insert(
        "kind".into(),
        Value::String(dispute_kind_to_str(dispute.kind).to_owned()),
    );
    Ok(Value::Object(root))
}

fn parse_dispute_kind(kind: &str) -> Result<CapacityDisputeKind, String> {
    match kind {
        "replication_shortfall" => Ok(CapacityDisputeKind::ReplicationShortfall),
        "uptime_breach" => Ok(CapacityDisputeKind::UptimeBreach),
        "proof_failure" => Ok(CapacityDisputeKind::ProofFailure),
        "fee_dispute" => Ok(CapacityDisputeKind::FeeDispute),
        "other" => Ok(CapacityDisputeKind::Other),
        other if other.to_ascii_lowercase() == other => Err(format!(
            "unknown dispute kind `{other}` (expected replication_shortfall|uptime_breach|proof_failure|fee_dispute|other)"
        )),
        other => Err(format!(
            "dispute kind `{other}` must be canonical lowercase (expected replication_shortfall|uptime_breach|proof_failure|fee_dispute|other)"
        )),
    }
}

fn dispute_kind_to_str(kind: CapacityDisputeKind) -> &'static str {
    match kind {
        CapacityDisputeKind::ReplicationShortfall => "replication_shortfall",
        CapacityDisputeKind::UptimeBreach => "uptime_breach",
        CapacityDisputeKind::ProofFailure => "proof_failure",
        CapacityDisputeKind::FeeDispute => "fee_dispute",
        CapacityDisputeKind::Other => "other",
    }
}

fn build_complete_request_value(order_id_hex: &str) -> Value {
    let mut root = Map::new();
    root.insert(
        "order_id_hex".into(),
        Value::String(order_id_hex.to_owned()),
    );
    Value::Object(root)
}

fn capability_label(cap: &CapabilityType) -> &'static str {
    match cap {
        CapabilityType::ToriiGateway => "torii_gateway",
        CapabilityType::QuicNoise => "quic_noise",
        CapabilityType::SoraNetHybridPq => "soranet_hybrid_pq",
        CapabilityType::ChunkRangeFetch => "chunk_range_fetch",
        CapabilityType::VendorReserved => "vendor_reserved",
    }
}

fn parse_u64(value: &str, context: &str) -> Result<u64, String> {
    require_canonical_unsigned_decimal(context, value)?;
    value
        .parse::<u64>()
        .map_err(|err| format!("invalid {context}: {err}"))
}

fn parse_u64_value(value: Option<&Value>, context: &str) -> Result<u64, String> {
    let val = value.ok_or_else(|| format!("missing `{context}` field"))?;
    if let Some(num) = val.as_u64() {
        return Ok(num);
    }
    if let Some(text) = val.as_str() {
        require_canonical_unsigned_decimal(context, text)?;
        return text
            .parse::<u64>()
            .map_err(|err| format!("invalid `{context}`: {err}"));
    }
    Err(format!("`{context}` must be a number or string"))
}

fn parse_u32_value(value: &Value, context: &str) -> Result<u32, String> {
    if let Some(num) = value.as_u64() {
        return u32::try_from(num)
            .map_err(|_| format!("`{context}` does not fit into u32 (value: {num})"));
    }
    if let Some(text) = value.as_str() {
        require_canonical_unsigned_decimal(context, text)?;
        let parsed: u64 = text
            .parse()
            .map_err(|err| format!("invalid `{context}`: {err}"))?;
        return u32::try_from(parsed)
            .map_err(|_| format!("`{context}` does not fit into u32 (value: {parsed})"));
    }
    Err(format!("`{context}` must be a number or string"))
}

fn parse_xor_quantity_value(value: &Value, context: &str) -> Result<XorQuantity, String> {
    let text = value
        .as_str()
        .ok_or_else(|| format!("`{context}` must be a canonical decimal string"))?;
    let quantity = text
        .parse::<XorQuantity>()
        .map_err(|err| format!("invalid `{context}`: {err}"))?;
    if quantity.to_string() != text {
        return Err(format!(
            "`{context}` must use the canonical decimal spelling"
        ));
    }
    Ok(quantity)
}

fn parse_u16_value(value: &Value, context: &str) -> Result<u16, String> {
    if let Some(num) = value.as_u64() {
        return u16::try_from(num)
            .map_err(|_| format!("`{context}` does not fit into u16 (value: {num})"));
    }
    if let Some(text) = value.as_str() {
        require_canonical_unsigned_decimal(context, text)?;
        let parsed: u64 = text
            .parse()
            .map_err(|err| format!("invalid `{context}`: {err}"))?;
        return u16::try_from(parsed)
            .map_err(|_| format!("`{context}` does not fit into u16 (value: {parsed})"));
    }
    Err(format!("`{context}` must be a number or string"))
}

fn require_canonical_unsigned_decimal(context: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("`{context}` must not be empty"));
    }
    if value.as_bytes().iter().any(u8::is_ascii_whitespace) {
        return Err(format!("`{context}` must not contain ASCII whitespace"));
    }
    if value.starts_with('+') || value.starts_with('-') {
        return Err(format!(
            "`{context}` must be a canonical unsigned decimal token"
        ));
    }
    if !value.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("`{context}` must contain only decimal digits"));
    }
    if value.len() > 1 && value.starts_with('0') {
        return Err(format!(
            "`{context}` must use canonical decimal without leading zeros"
        ));
    }
    Ok(())
}

fn require_value<'a>(map: &'a Map, key: &str) -> Result<&'a Value, String> {
    map.get(key).ok_or_else(|| format!("missing `{key}` field"))
}

fn parse_vec_hex(value: &str, context: &str) -> Result<Vec<u8>, String> {
    require_canonical_hex(context, value, None)?;
    let bytes = Vec::from_hex(value).map_err(|err| format!("invalid hex in `{context}`: {err}"))?;
    if bytes.iter().all(|&byte| byte == 0) {
        return Err(format!("`{context}` must not be all zero"));
    }
    Ok(bytes)
}

fn require_string<'a>(map: &'a Map, key: &str) -> Result<&'a str, String> {
    map.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing string field `{key}`"))
}

fn require_object<'a>(map: &'a Map, key: &str) -> Result<&'a Map, String> {
    map.get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("missing object field `{key}`"))
}

fn require_array<'a>(map: &'a Map, key: &str) -> Result<&'a Vec<Value>, String> {
    map.get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("missing array field `{key}`"))
}

fn parse_string_array(value: &Value, context: &str) -> Result<Vec<String>, String> {
    let array = value
        .as_array()
        .ok_or_else(|| format!("`{context}` must be an array of strings"))?;
    let mut out = Vec::with_capacity(array.len());
    for item in array {
        let text = item
            .as_str()
            .ok_or_else(|| format!("`{context}` entries must be strings"))?;
        out.push(text.to_owned());
    }
    Ok(out)
}

fn parse_fixed_hex<const N: usize>(value: &str, context: &str) -> Result<[u8; N], String> {
    require_canonical_hex(context, value, Some(N * 2))?;
    let bytes = Vec::from_hex(value).map_err(|err| format!("invalid hex in `{context}`: {err}"))?;
    if bytes.len() != N {
        return Err(format!(
            "`{context}` must decode to {N} bytes, got {} bytes",
            bytes.len()
        ));
    }
    if bytes.iter().all(|&byte| byte == 0) {
        return Err(format!("`{context}` must not be all zero"));
    }
    let mut out = [0u8; N];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn require_canonical_hex(
    context: &str,
    value: &str,
    expected_chars: Option<usize>,
) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("`{context}` must not be empty"));
    }
    if value.as_bytes().iter().any(u8::is_ascii_whitespace) {
        return Err(format!("`{context}` must not contain ASCII whitespace"));
    }
    if value.starts_with("0x") || value.starts_with("0X") {
        return Err(format!("`{context}` must not use a hex prefix"));
    }
    if !value.len().is_multiple_of(2) {
        return Err(format!(
            "`{context}` must contain an even number of hex characters"
        ));
    }
    if let Some(expected) = expected_chars
        && value.len() != expected
    {
        return Err(format!(
            "`{context}` must contain exactly {expected} lowercase hex characters"
        ));
    }
    if !value
        .chars()
        .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    {
        return Err(format!(
            "`{context}` must contain only lowercase hex characters"
        ));
    }
    Ok(())
}

fn resolve_private_key(
    private_key: Option<&str>,
    private_key_file: Option<&Path>,
) -> Result<String, String> {
    if private_key.is_some() && private_key_file.is_some() {
        return Err("--private-key and --private-key-file are mutually exclusive".to_string());
    }

    if let Some(value) = private_key {
        if value.is_empty() {
            return Err("--private-key must not be empty".to_string());
        }
        return Ok(value.to_string());
    }

    let Some(path) = private_key_file else {
        return Err(
            "--private-key or --private-key-file is required when using --request-out".to_string(),
        );
    };

    let value = fs::read_to_string(path).map_err(|err| {
        format!(
            "failed to read private key file `{}`: {err}",
            path.display()
        )
    })?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!(
            "private key file `{}` must not be empty",
            path.display()
        ));
    }
    Ok(trimmed.to_string())
}

fn write_binary(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if path == Path::new("-") {
        io::stdout()
            .write_all(bytes)
            .map_err(|err| format!("failed to write binary payload to stdout: {err}"))?;
        return Ok(());
    }
    let mut file = super::open_output_file(path, "capacity binary payload")?;
    file.write_all(bytes)
        .map_err(|err| format!("failed to write `{}`: {err}", path.display()))
}

fn write_text(path: &Path, text: &str) -> Result<(), String> {
    if path == Path::new("-") {
        io::stdout()
            .write_all(text.as_bytes())
            .map_err(|err| format!("failed to write to stdout: {err}"))?;
        if !text.ends_with('\n') {
            io::stdout()
                .write_all(b"\n")
                .map_err(|err| format!("failed to write newline to stdout: {err}"))?;
        }
        return Ok(());
    }

    let mut file = super::open_output_file(path, "capacity text output")?;
    file.write_all(text.as_bytes())
        .map_err(|err| format!("failed to write `{}`: {err}", path.display()))?;
    if !text.ends_with('\n') {
        file.write_all(b"\n")
            .map_err(|err| format!("failed to write newline to `{}`: {err}", path.display()))?;
    }
    Ok(())
}

fn write_json_file(path: &Path, value: &Value) -> Result<(), String> {
    let json_text = json::to_string_pretty(value)
        .map_err(|err| format!("failed to serialize JSON: {err}"))?
        + "\n";
    write_text(path, &json_text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{TempDir, tempdir};

    fn canonical_tempdir() -> (TempDir, PathBuf) {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().canonicalize().expect("canonical tempdir");
        (temp, path)
    }

    #[test]
    fn numeric_string_parsers_reject_noncanonical_tokens() {
        assert_eq!(parse_u64("0", "--registered-epoch").expect("zero"), 0);
        assert_eq!(parse_u64("42", "--registered-epoch").expect("u64"), 42);
        assert_eq!(
            parse_u64_value(Some(&Value::String("42".into())), "valid_from").expect("string u64"),
            42
        );
        assert_eq!(
            parse_u32_value(&Value::String("42".into()), "target_replicas").expect("string u32"),
            42
        );
        assert_eq!(
            parse_u16_value(&Value::String("42".into()), "target_replicas").expect("string u16"),
            42
        );
        assert_eq!(
            parse_xor_quantity_value(&Value::String("0.000000001".into()), "stake.stake_amount")
                .expect("sub-micro quantity")
                .to_string(),
            "0.000000001"
        );
        assert_eq!(
            parse_xor_quantity_value(
                &Value::String("340282366920938463463374607431768211456".into()),
                "stake.stake_amount"
            )
            .expect("quantity wider than u128")
            .to_string(),
            "340282366920938463463374607431768211456"
        );
        for value in [
            Value::from(5_u64),
            Value::String("01".into()),
            Value::String("1.0".into()),
            Value::String("+1".into()),
            Value::String("-1".into()),
            Value::String(" 1".into()),
        ] {
            parse_xor_quantity_value(&value, "stake.stake_amount")
                .expect_err("noncanonical or non-string XOR quantity must fail");
        }

        for value in ["", "01", "+1", "-1", "1 ", "1_000", "18446744073709551616"] {
            let err =
                parse_u64(value, "--registered-epoch").expect_err("noncanonical decimal must fail");
            assert!(
                err.contains("--registered-epoch"),
                "error should name context for {value:?}: {err}"
            );
        }

        let err = parse_u16_value(&Value::String("65536".into()), "target_replicas")
            .expect_err("u16 overflow must fail");
        assert!(err.contains("target_replicas"), "unexpected error: {err}");
    }

    #[test]
    fn hex_parsers_reject_noncanonical_tokens() {
        let fixed = "11".repeat(32);
        assert_eq!(
            parse_fixed_hex::<32>(&fixed, "provider_id_hex").expect("fixed hex"),
            [0x11; 32]
        );
        assert_eq!(
            parse_vec_hex("aabbccdd", "manifest_cid_hex").expect("vec hex"),
            vec![0xaa, 0xbb, 0xcc, 0xdd]
        );

        for (value, expected) in [
            ("", "must not be empty"),
            ("11", "exactly 64"),
            (
                "0x1111111111111111111111111111111111111111111111111111111111111111",
                "prefix",
            ),
            (
                "111111111111111111111111111111111111111111111111111111111111111A",
                "lowercase",
            ),
            (
                "111111111111111111111111111111111111111111111111111111111111111 ",
                "whitespace",
            ),
            ("111", "even number"),
            (
                "0000000000000000000000000000000000000000000000000000000000000000",
                "all zero",
            ),
        ] {
            let err = parse_fixed_hex::<32>(value, "provider_id_hex")
                .expect_err("noncanonical fixed hex must fail");
            assert!(
                err.contains(expected),
                "error for {value:?} should contain {expected:?}, got: {err}"
            );
        }

        for (value, expected) in [
            ("AABB", "lowercase"),
            ("aabb ", "whitespace"),
            ("0xaabb", "prefix"),
            ("aaa", "even number"),
            ("0000", "all zero"),
        ] {
            let err = parse_vec_hex(value, "manifest_cid_hex")
                .expect_err("noncanonical vec hex must fail");
            assert!(
                err.contains(expected),
                "error for {value:?} should contain {expected:?}, got: {err}"
            );
        }
    }

    #[test]
    fn dispute_kind_requires_canonical_lowercase() {
        assert_eq!(
            parse_dispute_kind("replication_shortfall").expect("kind"),
            CapacityDisputeKind::ReplicationShortfall
        );

        for value in ["Replication_Shortfall", "proof-failure", " proof_failure"] {
            let err = parse_dispute_kind(value).expect_err("noncanonical kind must fail");
            assert!(
                err.contains("dispute kind") || err.contains("unknown dispute kind"),
                "unexpected error for {value:?}: {err}"
            );
        }
    }

    #[test]
    fn write_binary_creates_parent_and_writes_all_bytes() {
        let (_temp, temp_path) = canonical_tempdir();
        let output_path = temp_path.join("nested").join("payload.to");

        write_binary(&output_path, b"sorafs-capacity").expect("write binary");

        assert_eq!(
            fs::read(&output_path).expect("read output"),
            b"sorafs-capacity"
        );
    }

    #[cfg(unix)]
    #[test]
    fn write_text_rejects_symlink_output() {
        let (_temp, temp_path) = canonical_tempdir();
        let target_path = temp_path.join("target.txt");
        fs::write(&target_path, b"unchanged\n").expect("write target");
        let output_path = temp_path.join("payload.txt");
        std::os::unix::fs::symlink(&target_path, &output_path).expect("create symlink");

        let err = write_text(&output_path, "changed\n").expect_err("reject symlink output");

        assert!(
            err.contains("must not be a symlink"),
            "unexpected error: {err}"
        );
        assert_eq!(fs::read(&target_path).expect("read target"), b"unchanged\n");
    }

    #[cfg(unix)]
    #[test]
    fn write_json_file_rejects_symlink_parent() {
        let (_temp, temp_path) = canonical_tempdir();
        let real_dir = temp_path.join("real");
        fs::create_dir(&real_dir).expect("create real dir");
        let linked_dir = temp_path.join("linked");
        std::os::unix::fs::symlink(&real_dir, &linked_dir).expect("create symlink");
        let output_path = linked_dir.join("request.json");

        let err = write_json_file(&output_path, &Value::Object(Map::new()))
            .expect_err("reject symlink parent");

        assert!(
            err.contains("parent") && err.contains("must not be a symlink"),
            "unexpected error: {err}"
        );
        assert!(
            !real_dir.join("request.json").exists(),
            "symlink parent should not receive output"
        );
    }
}
