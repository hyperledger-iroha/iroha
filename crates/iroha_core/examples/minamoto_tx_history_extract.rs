//! Extract Minamoto transaction history JSONL from a Kura block store.

use std::{
    collections::BTreeSet,
    env,
    error::Error,
    fs::File,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use iroha_core::kura::{BlockIndex, BlockStore};
use iroha_data_model::{
    block::{ExternalExecutionContext, SignedBlock, decode_framed_signed_block},
    isi::{
        BurnBox, GrantBox, InstructionBox, MintBox, RegisterBox, RemoveKeyValueBox, RevokeBox,
        SetKeyValueBox, TransferBox, UnregisterBox,
    },
    nexus::{DataSpaceId, LaneId},
    transaction::{Executable, SignedTransaction, TransactionEntrypoint},
};
use iroha_version::codec::DecodeVersioned;
use norito::core::{Header, MAGIC, reset_decode_state, set_decode_flags};
use norito::json::{JsonSerialize, to_json};

const DA_BLOCKS_DIR_NAME: &str = "da_blocks";
const EVICTED_BLOCK_START: u64 = u64::MAX;

#[derive(Debug)]
struct Args {
    network: String,
    storage: PathBuf,
    output: PathBuf,
}

#[derive(Default)]
struct TxSummary {
    accounts: BTreeSet<String>,
    asset_ids: BTreeSet<String>,
    from: Option<String>,
    to: Option<String>,
    amount: Option<String>,
    asset_id: Option<String>,
    operation_type: Option<String>,
}

struct InstructionRow {
    index: usize,
    kind: String,
    json: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args()?;
    let block_store_path = resolve_block_store_dir(&args.storage)?;
    let mut block_store = BlockStore::new(&block_store_path);
    let index_count = block_store.read_index_count()?;

    let file = File::create(&args.output)?;
    let mut out = BufWriter::new(file);

    let mut block_indices = vec![
        BlockIndex {
            start: 0,
            length: 0,
        };
        usize::try_from(index_count)?
    ];
    block_store.read_block_indices(0, &mut block_indices)?;

    let mut skipped_private_rows = 0_usize;
    for (block_offset, block_index) in block_indices.iter().enumerate() {
        let sidecar_height = u64::try_from(block_offset)?.saturating_add(1);
        let block_buf = read_block_payload(
            &block_store_path,
            &mut block_store,
            block_index,
            sidecar_height,
        )?;
        let block = decode_block(&block_buf)?;
        let header = block.header();
        let block_height = header.height().get();
        let block_time_ms = u64::try_from(header.creation_time().as_millis())?;
        let block_hash = format!("{}", block.hash());
        let txs = block.transactions_vec();

        for (tx_index, tx) in txs.iter().enumerate() {
            let has_route_bundle = block.execution_context().is_some();
            let route_context = route_context_for_tx(&block, tx_index, tx);
            if (has_route_bundle && route_context.is_none())
                || !route_context_is_public(route_context)
            {
                skipped_private_rows = skipped_private_rows.saturating_add(1);
                continue;
            }
            let (lane_id, dataspace_id) = route_fields(route_context);
            let mut summary = TxSummary::default();
            let authority = tx.authority().to_string();
            summary.accounts.insert(authority.clone());

            let instruction_rows = match tx.instructions() {
                Executable::Instructions(instructions) => instructions
                    .iter()
                    .enumerate()
                    .map(|(index, instruction)| {
                        summarize_instruction(instruction, &mut summary);
                        InstructionRow {
                            index,
                            kind: instruction_kind(instruction).to_string(),
                            json: instruction_json(instruction),
                        }
                    })
                    .collect::<Vec<_>>(),
                Executable::ContractCall(call) => {
                    summary.operation_type = Some("ContractCall".to_string());
                    vec![InstructionRow {
                        index: 0,
                        kind: "ContractCall".to_string(),
                        json: format!(
                            "{{\"ContractCall\":{{\"address\":{},\"entrypoint\":{},\"payload\":{}}}}}",
                            json_string(&call.contract_address.to_string()),
                            json_string(&call.entrypoint),
                            json_string(&format!("{:?}", call.payload))
                        ),
                    }]
                }
                Executable::Ivm(bytecode) => {
                    summary.operation_type = Some("Ivm".to_string());
                    vec![InstructionRow {
                        index: 0,
                        kind: "Ivm".to_string(),
                        json: format!(
                            "{{\"Ivm\":{{\"bytecode_size\":{}}}}}",
                            bytecode.size_bytes()
                        ),
                    }]
                }
                Executable::IvmProved(proved) => {
                    summary.operation_type = Some("IvmProved".to_string());
                    vec![InstructionRow {
                        index: 0,
                        kind: "IvmProved".to_string(),
                        json: format!(
                            "{{\"IvmProved\":{{\"bytecode_size\":{},\"overlay_instructions\":{}}}}}",
                            proved.bytecode.size_bytes(),
                            proved.overlay.len()
                        ),
                    }]
                }
            };

            if summary.operation_type.is_none() {
                summary.operation_type = instruction_rows.first().map(|row| row.kind.clone());
            }

            let tx_time_ms = u64::try_from(tx.creation_time().as_millis())?;
            let result_ok = block.error(tx_index).is_none();
            let rejection = block
                .error(tx_index)
                .map(|err| json_string(&format!("{err:?}")))
                .unwrap_or_else(|| "null".to_string());
            let metadata_json = to_json_value(tx.metadata());
            let executable_json = to_json_value(tx.instructions());

            write!(out, "{{")?;
            write_field(&mut out, "network", &args.network, false)?;
            write_field(&mut out, "tx_hash", &tx.hash().to_string(), true)?;
            write_field(&mut out, "hash", &tx.hash().to_string(), true)?;
            write_number(&mut out, "block", block_height, true)?;
            write_number(&mut out, "block_height", block_height, true)?;
            write_field(&mut out, "block_hash_hex", &block_hash, true)?;
            write_number(&mut out, "block_index", u64::try_from(block_offset)?, true)?;
            write_number(&mut out, "lane_id", lane_id, true)?;
            write_number(&mut out, "dataspace_id", dataspace_id, true)?;
            write_number(&mut out, "timestamp_ms", tx_time_ms, true)?;
            write_number(&mut out, "block_timestamp_ms", block_time_ms, true)?;
            write_field(&mut out, "authority", &authority, true)?;
            write_bool(&mut out, "result_ok", result_ok, true)?;
            write_field(
                &mut out,
                "status",
                if result_ok { "SUCCESS" } else { "FAILED" },
                true,
            )?;
            write_field(
                &mut out,
                "transaction_status",
                if result_ok { "Committed" } else { "Rejected" },
                true,
            )?;
            write_optional_field(
                &mut out,
                "operation_type",
                summary.operation_type.as_deref(),
                true,
            )?;
            write_optional_field(&mut out, "from_account_id", summary.from.as_deref(), true)?;
            write_optional_field(&mut out, "to_account_id", summary.to.as_deref(), true)?;
            write_optional_field(&mut out, "amount", summary.amount.as_deref(), true)?;
            write_optional_field(&mut out, "asset_id", summary.asset_id.as_deref(), true)?;
            write_string_array(&mut out, "accounts", &summary.accounts, true)?;
            write_string_array(&mut out, "asset_ids", &summary.asset_ids, true)?;
            write!(out, ",\"metadata\":{metadata_json}")?;
            write!(out, ",\"executable_payload\":{executable_json}")?;
            write!(out, ",\"rejection_reason\":{rejection}")?;
            write!(out, ",\"instructions\":[")?;
            for (idx, row) in instruction_rows.iter().enumerate() {
                if idx > 0 {
                    write!(out, ",")?;
                }
                write!(
                    out,
                    "{{\"index\":{},\"kind\":{},\"authority\":{},\"transaction_hash\":{},\"transaction_status\":{},\"block\":{},\"json\":{}}}",
                    row.index,
                    json_string(&row.kind),
                    json_string(&authority),
                    json_string(&tx.hash().to_string()),
                    json_string(if result_ok { "Committed" } else { "Rejected" }),
                    block_height,
                    row.json
                )?;
            }
            writeln!(out, "]}}")?;
        }
    }

    out.flush()?;
    if skipped_private_rows > 0 {
        eprintln!("skipped {skipped_private_rows} private dataspace transaction row(s)");
    }
    Ok(())
}

fn parse_args() -> Result<Args, String> {
    let mut network = None;
    let mut storage = None;
    let mut output = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--network" => network = args.next(),
            "--storage" => storage = args.next().map(PathBuf::from),
            "--output" => output = args.next().map(PathBuf::from),
            "-h" | "--help" => {
                return Err(
                    "usage: minamoto_tx_history_extract --network <name> --storage <kura-peer-dir> --output <jsonl>".to_string(),
                );
            }
            other => return Err(format!("unexpected argument: {other}")),
        }
    }
    Ok(Args {
        network: network.ok_or("missing --network")?,
        storage: storage.ok_or("missing --storage")?,
        output: output.ok_or("missing --output")?,
    })
}

fn decode_block(bytes: &[u8]) -> Result<SignedBlock, Box<dyn std::error::Error>> {
    match decode_framed_signed_block(bytes) {
        Ok(block) => Ok(block),
        Err(framed_err) => {
            if bytes.len() > 1 + Header::SIZE && bytes[1..].starts_with(MAGIC.as_slice()) {
                let mut bare_versioned = Vec::with_capacity(1 + bytes.len() - 1 - Header::SIZE);
                bare_versioned.push(bytes[0]);
                bare_versioned.extend_from_slice(&bytes[1 + Header::SIZE..]);
                let flags = bytes[1 + Header::SIZE - 1];
                set_decode_flags(flags);
                let legacy_result = SignedBlock::decode_all_versioned(&bare_versioned);
                reset_decode_state();
                match legacy_result {
                    Ok(block) => return Ok(block),
                    Err(legacy_err) => {
                        return Err(format!(
                            "failed to decode block as framed ({framed_err}) or deframed legacy payload with flags {flags:#04x} ({legacy_err})"
                        )
                        .into());
                    }
                }
            }
            match norito::decode_from_bytes::<SignedBlock>(bytes) {
                Ok(block) => Ok(block),
                Err(raw_err) => Err(format!(
                    "failed to decode block as framed ({framed_err}) or raw Norito ({raw_err})"
                )
                .into()),
            }
        }
    }
}

fn read_block_payload(
    block_store_path: &Path,
    block_store: &mut BlockStore,
    block_index: &BlockIndex,
    height: u64,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let expected_len = usize::try_from(block_index.length)?;
    if block_index.start == EVICTED_BLOCK_START {
        let sidecar_path = block_store_path
            .join(DA_BLOCKS_DIR_NAME)
            .join(format!("{height:020}.norito"));
        let bytes = std::fs::read(&sidecar_path)?;
        if block_index.length > 0 && bytes.len() != expected_len {
            return Err(format!(
                "DA sidecar {} length {} does not match Kura index length {}",
                sidecar_path.display(),
                bytes.len(),
                block_index.length
            )
            .into());
        }
        return Ok(bytes);
    }

    let mut block_buf = vec![0_u8; expected_len];
    block_store.read_block_data(block_index.start, &mut block_buf)?;
    Ok(block_buf)
}

fn route_context_for_tx<'block>(
    block: &'block SignedBlock,
    tx_index: usize,
    tx: &SignedTransaction,
) -> Option<&'block ExternalExecutionContext> {
    let bundle = block.execution_context()?;
    let entrypoint_hash = TransactionEntrypoint::from(tx.clone()).hash();
    match bundle.external.get(tx_index) {
        Some(context) if context.entrypoint_hash == entrypoint_hash => Some(context),
        _ => bundle
            .external
            .iter()
            .find(|context| context.entrypoint_hash == entrypoint_hash),
    }
}

fn route_context_is_public(context: Option<&ExternalExecutionContext>) -> bool {
    let Some(context) = context else {
        return true;
    };
    context.dataspace_id == DataSpaceId::UNIVERSAL
        && context
            .routing_plan_legs
            .iter()
            .all(|leg| leg.dataspace_id == DataSpaceId::UNIVERSAL)
}

fn route_fields(context: Option<&ExternalExecutionContext>) -> (u64, u64) {
    match context {
        Some(context) => (
            u64::from(context.lane_id.as_u32()),
            context.dataspace_id.as_u64(),
        ),
        None => (
            u64::from(LaneId::SINGLE.as_u32()),
            DataSpaceId::UNIVERSAL.as_u64(),
        ),
    }
}

fn resolve_block_store_dir(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if path.join("blocks.index").exists() {
        return Ok(path.to_path_buf());
    }
    let blocks_dir = path.join("blocks");
    if blocks_dir.is_dir() {
        let mut entries = std::fs::read_dir(&blocks_dir)?
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().map(|ty| ty.is_dir()).unwrap_or(false))
            .collect::<Vec<_>>();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let candidate = entry.path();
            if candidate.join("blocks.index").exists() {
                return Ok(candidate);
            }
        }
    }
    Err(format!("failed to locate block store under {}", path.display()).into())
}

fn summarize_instruction(instruction: &InstructionBox, summary: &mut TxSummary) {
    let kind = instruction_kind(instruction);
    if summary.operation_type.is_none() {
        summary.operation_type = Some(kind.to_string());
    }

    if let Some(transfer) = instruction.as_any().downcast_ref::<TransferBox>() {
        if let TransferBox::Asset(asset) = transfer {
            let from = asset.source.account.to_string();
            let to = asset.destination.to_string();
            let asset_id = asset.source.definition.to_string();
            summary.accounts.insert(from.clone());
            summary.accounts.insert(to.clone());
            summary.asset_ids.insert(asset_id.clone());
            summary.from.get_or_insert(from);
            summary.to.get_or_insert(to);
            summary.asset_id.get_or_insert(asset_id);
            summary.amount.get_or_insert(asset.object.to_string());
        }
        return;
    }

    if let Some(mint) = instruction.as_any().downcast_ref::<MintBox>() {
        if let MintBox::Asset(asset) = mint {
            let to = asset.destination.account.to_string();
            let asset_id = asset.destination.definition.to_string();
            summary.accounts.insert(to.clone());
            summary.asset_ids.insert(asset_id.clone());
            summary.to.get_or_insert(to);
            summary.asset_id.get_or_insert(asset_id);
            summary.amount.get_or_insert(asset.object.to_string());
        }
        return;
    }

    if let Some(burn) = instruction.as_any().downcast_ref::<BurnBox>() {
        if let BurnBox::Asset(asset) = burn {
            let from = asset.destination.account.to_string();
            let asset_id = asset.destination.definition.to_string();
            summary.accounts.insert(from.clone());
            summary.asset_ids.insert(asset_id.clone());
            summary.from.get_or_insert(from);
            summary.asset_id.get_or_insert(asset_id);
            summary.amount.get_or_insert(asset.object.to_string());
        }
        return;
    }

    if let Some(register) = instruction.as_any().downcast_ref::<RegisterBox>() {
        if let RegisterBox::Account(account) = register {
            summary.accounts.insert(account.object.id.to_string());
        }
        return;
    }

    if let Some(unregister) = instruction.as_any().downcast_ref::<UnregisterBox>() {
        if let UnregisterBox::Account(account) = unregister {
            summary.accounts.insert(account.object.to_string());
        }
    }
}

fn instruction_kind(instruction: &InstructionBox) -> &'static str {
    if instruction.as_any().downcast_ref::<TransferBox>().is_some() {
        "Transfer"
    } else if instruction.as_any().downcast_ref::<MintBox>().is_some() {
        "Mint"
    } else if instruction.as_any().downcast_ref::<BurnBox>().is_some() {
        "Burn"
    } else if instruction.as_any().downcast_ref::<RegisterBox>().is_some() {
        "Register"
    } else if instruction
        .as_any()
        .downcast_ref::<UnregisterBox>()
        .is_some()
    {
        "Unregister"
    } else if instruction
        .as_any()
        .downcast_ref::<SetKeyValueBox>()
        .is_some()
    {
        "SetKeyValue"
    } else if instruction
        .as_any()
        .downcast_ref::<RemoveKeyValueBox>()
        .is_some()
    {
        "RemoveKeyValue"
    } else if instruction.as_any().downcast_ref::<GrantBox>().is_some() {
        "Grant"
    } else if instruction.as_any().downcast_ref::<RevokeBox>().is_some() {
        "Revoke"
    } else {
        "Custom"
    }
}

fn instruction_json(instruction: &InstructionBox) -> String {
    to_json(instruction)
        .unwrap_or_else(|_| format!("{{\"Debug\":{}}}", json_string(&format!("{instruction:?}"))))
}

fn to_json_value<T: JsonSerialize>(value: &T) -> String {
    to_json(value).unwrap_or_else(|_| "null".to_string())
}

fn write_field(
    out: &mut dyn Write,
    key: &str,
    value: &str,
    comma: bool,
) -> Result<(), std::io::Error> {
    if comma {
        write!(out, ",")?;
    }
    write!(out, "{}:{}", json_string(key), json_string(value))
}

fn write_optional_field(
    out: &mut dyn Write,
    key: &str,
    value: Option<&str>,
    comma: bool,
) -> Result<(), std::io::Error> {
    if comma {
        write!(out, ",")?;
    }
    write!(out, "{}:", json_string(key))?;
    match value {
        Some(value) => write!(out, "{}", json_string(value)),
        None => write!(out, "null"),
    }
}

fn write_number(
    out: &mut dyn Write,
    key: &str,
    value: u64,
    comma: bool,
) -> Result<(), std::io::Error> {
    if comma {
        write!(out, ",")?;
    }
    write!(out, "{}:{value}", json_string(key))
}

fn write_bool(
    out: &mut dyn Write,
    key: &str,
    value: bool,
    comma: bool,
) -> Result<(), std::io::Error> {
    if comma {
        write!(out, ",")?;
    }
    write!(out, "{}:{value}", json_string(key))
}

fn write_string_array(
    out: &mut dyn Write,
    key: &str,
    values: &BTreeSet<String>,
    comma: bool,
) -> Result<(), std::io::Error> {
    if comma {
        write!(out, ",")?;
    }
    write!(out, "{}:[", json_string(key))?;
    for (idx, value) in values.iter().enumerate() {
        if idx > 0 {
            write!(out, ",")?;
        }
        write!(out, "{}", json_string(value))?;
    }
    write!(out, "]")
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            ch if ch < ' ' => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use iroha_core::kura::BlockStore;
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        block::ExternalExecutionContext,
        nexus::{DataSpaceId, LaneId},
        transaction::TransactionEntrypoint,
    };
    use tempfile::TempDir;

    use super::{EVICTED_BLOCK_START, read_block_payload, route_context_is_public};

    fn entrypoint_hash(label: &[u8]) -> HashOf<TransactionEntrypoint> {
        HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(label))
    }

    #[test]
    fn read_block_payload_reads_evicted_sidecar() {
        let dir = TempDir::new().expect("tempdir");
        let mut store = BlockStore::new(dir.path());
        store.create_files_if_they_do_not_exist().expect("files");
        let payload = b"evicted block payload";
        store
            .write_block_index(
                0,
                EVICTED_BLOCK_START,
                u64::try_from(payload.len()).expect("payload length fits"),
            )
            .expect("index");
        let sidecar_dir = dir.path().join("da_blocks");
        std::fs::create_dir_all(&sidecar_dir).expect("sidecar dir");
        std::fs::write(sidecar_dir.join("00000000000000000001.norito"), payload).expect("sidecar");
        let index = store.read_block_index(0).expect("index read");

        let observed =
            read_block_payload(dir.path(), &mut store, &index, 1).expect("sidecar payload");

        assert_eq!(observed, payload);
    }

    #[test]
    fn route_context_public_filter_excludes_private_dataspaces() {
        assert!(route_context_is_public(None));

        let public = ExternalExecutionContext::new(
            entrypoint_hash(b"public"),
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
        );
        assert!(route_context_is_public(Some(&public)));

        let private = ExternalExecutionContext::new(
            entrypoint_hash(b"private"),
            LaneId::new(1),
            DataSpaceId::new(1),
        );
        assert!(!route_context_is_public(Some(&private)));
    }
}
