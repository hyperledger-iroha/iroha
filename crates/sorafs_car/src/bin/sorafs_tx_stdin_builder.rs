use std::{env, fs, process, str::FromStr};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STD};
use iroha_data_model::{
    isi::sorafs::{CompleteReplicationOrder, IssueReplicationOrder, RegisterCapacityDeclaration},
    metadata::Metadata,
    prelude::{InstructionBox, Name},
    sorafs::{
        capacity::{CapacityDeclarationRecord, ProviderId},
        pin_registry::ReplicationOrderId,
    },
};
use iroha_primitives::json::Json;
use norito::{
    decode_from_bytes,
    json::{self, Map, Value},
    to_bytes,
};
use sorafs_manifest::capacity::{CapacityDeclarationV1, ReplicationOrderV1};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        return Err(usage());
    };

    match command.as_str() {
        "capacity-declaration-request" => run_capacity_declaration_request(args),
        "replication-order-request" => run_replication_order_request(args),
        "complete-order" => run_complete_order(args),
        "help" | "--help" | "-h" => {
            println!("{}", usage());
            Ok(())
        }
        other => Err(format!("unknown subcommand `{other}`\n\n{}", usage())),
    }
}

fn usage() -> String {
    r#"usage: sorafs_tx_stdin_builder <subcommand> [options]

Subcommands:
  capacity-declaration-request  Convert a declaration request JSON into `iroha ledger transaction stdin` JSON.
  replication-order-request     Convert a replication-order request JSON into `iroha ledger transaction stdin` JSON.
  complete-order                Emit a completion instruction for an existing replication order.

Options:
  capacity-declaration-request --request=<path>
  replication-order-request --request=<path> --issued-epoch=<u64> --deadline-epoch=<u64>
  complete-order --order-id-hex=<64-hex> --completion-epoch=<u64>
"#
    .to_owned()
}

fn run_capacity_declaration_request(args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut request_path = None;
    for arg in args {
        let (key, value) = split_option(&arg)?;
        match key {
            "--request" => request_path = Some(value.to_owned()),
            _ => return Err(format!("unknown option `{key}`")),
        }
    }

    let request = read_json_map(request_path.as_deref(), "declaration request")?;
    let declaration_b64 = require_string(&request, "declaration_b64")?;
    let declaration_bytes = BASE64_STD
        .decode(declaration_b64.as_bytes())
        .map_err(|err| format!("invalid base64 in `declaration_b64`: {err}"))?;
    let declaration: CapacityDeclarationV1 = decode_from_bytes(&declaration_bytes)
        .map_err(|err| format!("failed to decode `CapacityDeclarationV1`: {err}"))?;
    declaration
        .validate()
        .map_err(|err| format!("capacity declaration validation failed: {err}"))?;

    let canonical_bytes = to_bytes(&declaration)
        .map_err(|err| format!("failed to re-encode capacity declaration: {err}"))?;
    let metadata = metadata_from_request(&request)?;
    let record = CapacityDeclarationRecord::new(
        ProviderId::new(declaration.provider_id),
        canonical_bytes,
        declaration.committed_capacity_gib,
        require_u64(&request, "registered_epoch")?,
        require_u64(&request, "valid_from_epoch")?,
        require_u64(&request, "valid_until_epoch")?,
        metadata,
    );

    print_instruction_json(InstructionBox::from(RegisterCapacityDeclaration::new(
        record,
    )))
}

fn run_replication_order_request(args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut request_path = None;
    let mut issued_epoch = None;
    let mut deadline_epoch = None;

    for arg in args {
        let (key, value) = split_option(&arg)?;
        match key {
            "--request" => request_path = Some(value.to_owned()),
            "--issued-epoch" => issued_epoch = Some(parse_u64(value, key)?),
            "--deadline-epoch" => deadline_epoch = Some(parse_u64(value, key)?),
            _ => return Err(format!("unknown option `{key}`")),
        }
    }

    let request = read_json_map(request_path.as_deref(), "replication order request")?;
    let order_b64 = require_string(&request, "order_b64")?;
    let order_bytes = BASE64_STD
        .decode(order_b64.as_bytes())
        .map_err(|err| format!("invalid base64 in `order_b64`: {err}"))?;
    let order: ReplicationOrderV1 = decode_from_bytes(&order_bytes)
        .map_err(|err| format!("failed to decode `ReplicationOrderV1`: {err}"))?;
    order
        .validate()
        .map_err(|err| format!("replication order validation failed: {err}"))?;

    let issued_epoch = issued_epoch.ok_or_else(|| "missing `--issued-epoch=<u64>`".to_owned())?;
    let deadline_epoch =
        deadline_epoch.ok_or_else(|| "missing `--deadline-epoch=<u64>`".to_owned())?;

    let instruction = IssueReplicationOrder::new(
        ReplicationOrderId::new(order.order_id),
        to_bytes(&order).map_err(|err| format!("failed to re-encode replication order: {err}"))?,
        issued_epoch,
        deadline_epoch,
    );

    print_instruction_json(InstructionBox::from(instruction))
}

fn run_complete_order(args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut order_id_hex = None;
    let mut completion_epoch = None;

    for arg in args {
        let (key, value) = split_option(&arg)?;
        match key {
            "--order-id-hex" => order_id_hex = Some(value.to_owned()),
            "--completion-epoch" => completion_epoch = Some(parse_u64(value, key)?),
            _ => return Err(format!("unknown option `{key}`")),
        }
    }

    let order_id_hex = order_id_hex.ok_or_else(|| "missing `--order-id-hex=<hex>`".to_owned())?;
    let order_id = parse_hex_32(&order_id_hex, "order_id_hex")?;
    let completion_epoch =
        completion_epoch.ok_or_else(|| "missing `--completion-epoch=<u64>`".to_owned())?;

    let instruction =
        CompleteReplicationOrder::new(ReplicationOrderId::new(order_id), completion_epoch);

    print_instruction_json(InstructionBox::from(instruction))
}

fn split_option(arg: &str) -> Result<(&str, &str), String> {
    arg.split_once('=')
        .ok_or_else(|| format!("expected `--key=value`, got `{arg}`"))
}

fn read_json_map(path: Option<&str>, label: &str) -> Result<Map, String> {
    let path = path.ok_or_else(|| format!("missing `--request=<path>` for {label}"))?;
    let bytes =
        fs::read(path).map_err(|err| format!("failed to read `{path}` for {label}: {err}"))?;
    let value: Value = json::from_slice(&bytes)
        .map_err(|err| format!("failed to parse JSON `{path}` for {label}: {err}"))?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| format!("{label} `{path}` must be a JSON object"))
}

fn require_string<'a>(map: &'a Map, key: &str) -> Result<&'a str, String> {
    map.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing or invalid string field `{key}`"))
}

fn require_u64(map: &Map, key: &str) -> Result<u64, String> {
    map.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("missing or invalid integer field `{key}`"))
}

fn parse_u64(value: &str, label: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|err| format!("invalid `{label}` value `{value}`: {err}"))
}

fn parse_hex_32(value: &str, label: &str) -> Result<[u8; 32], String> {
    let decoded = hex::decode(value).map_err(|err| format!("invalid `{label}` hex: {err}"))?;
    decoded
        .try_into()
        .map_err(|_| format!("`{label}` must be exactly 32 bytes (64 hex chars)"))
}

fn metadata_from_request(request: &Map) -> Result<Metadata, String> {
    let mut metadata = Metadata::default();
    let Some(entries) = request.get("metadata") else {
        return Ok(metadata);
    };
    let array = entries
        .as_array()
        .ok_or_else(|| "`metadata` must be an array".to_owned())?;

    for (index, entry) in array.iter().enumerate() {
        let object = entry
            .as_object()
            .ok_or_else(|| format!("metadata[{index}] must be an object"))?;
        let key = require_string(object, "key")?;
        let value = object
            .get("value")
            .cloned()
            .ok_or_else(|| format!("metadata[{index}] is missing `value`"))?;
        let name = Name::from_str(key)
            .map_err(|err| format!("metadata[{index}].key `{key}` is invalid: {err}"))?;
        metadata.insert(name, Json::new(value));
    }

    Ok(metadata)
}

fn print_instruction_json(instruction: InstructionBox) -> Result<(), String> {
    let encoded = to_bytes(&instruction)
        .map_err(|err| format!("failed to encode instruction payload: {err}"))?;
    let payload = Value::Array(vec![Value::String(BASE64_STD.encode(encoded))]);
    let rendered = json::to_string(&payload)
        .map_err(|err| format!("failed to serialize tx-stdin JSON: {err}"))?;
    println!("{rendered}");
    Ok(())
}
