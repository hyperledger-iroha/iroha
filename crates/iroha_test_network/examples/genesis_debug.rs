use std::process::{Command, Stdio};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use iroha_data_model::{block::SignedBlock, isi::InstructionBox, transaction::Executable};
use iroha_primitives::const_vec::ConstVec;
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use norito::{
    codec::{decode_adaptive, encode_adaptive},
    json::{self, Value as JsonValue},
};

fn main() {
    init_instruction_registry();

    if let Ok(mode) = std::env::var("GENESIS_DEBUG_MODE") {
        match mode.as_str() {
            "decode_step" => decode_step_mode(),
            other => panic!("unsupported GENESIS_DEBUG_MODE={other}"),
        }
        return;
    }

    let network = NetworkBuilder::new().with_peers(1).build();
    let genesis = network.genesis();

    inspect_transactions(&genesis.0);
}

fn inspect_transactions(block: &SignedBlock) {
    for (tx_idx, tx) in block.external_transactions().enumerate() {
        match tx.instructions() {
            Executable::Instructions(step) => {
                let encoded_step = encode_adaptive(step);
                let step_flags = format_flags(norito::codec::take_last_encode_flags());
                println!(
                    "tx#{tx_idx}: instructions len={} bytes={} flags={step_flags}",
                    step.len(),
                    encoded_step.len()
                );

                match decode_step_in_child(&encoded_step) {
                    Ok(info) => {
                        let len_matches = info.len == step.len();
                        println!("  child decode len={} len_matches={len_matches}", info.len);
                        if !len_matches {
                            println!("  expected len={}", step.len());
                        }
                        if let Some(sample) = info.sample_fields {
                            println!("  sample fields:");
                            for entry in sample {
                                println!("    {entry}");
                            }
                        }
                    }
                    Err(err) => {
                        println!("  child decode failed: {err}");
                    }
                }

                // Per-instruction decoding is skipped for now to avoid aborting on malformed items.
            }
            Executable::ContractCall(call) => {
                println!(
                    "tx#{tx_idx}: executable carries contract call {}::{} (skipping instruction probe)",
                    call.contract_address, call.entrypoint
                );
            }
            Executable::Ivm(_) => {
                println!(
                    "tx#{tx_idx}: executable carries IVM bytecode (skipping instruction probe)"
                );
            }
            Executable::IvmProved(proved) => {
                println!(
                    "tx#{tx_idx}: executable carries proved IVM bytecode (overlay_len={}, skipping instruction probe)",
                    proved.overlay.len()
                );
            }
        }
    }
}

fn format_flags(flags: Option<u8>) -> String {
    flags
        .map(|f| format!("0x{f:02x}"))
        .unwrap_or_else(|| "None".to_string())
}

struct ChildDecodeInfo {
    len: usize,
    sample_fields: Option<Vec<String>>,
}

fn decode_step_in_child(bytes: &[u8]) -> Result<ChildDecodeInfo, String> {
    let payload = BASE64.encode(bytes);
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let output = Command::new(exe)
        .env("GENESIS_DEBUG_MODE", "decode_step")
        .env("GENESIS_DEBUG_PAYLOAD", payload)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| err.to_string())?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_child_payload(stdout.trim())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            Err(format!("child exited with status {}", output.status))
        } else {
            Err(stderr)
        }
    }
}

fn parse_child_payload(line: &str) -> Result<ChildDecodeInfo, String> {
    let value: JsonValue =
        json::from_json(line).map_err(|err| format!("failed to parse child JSON: {err}"))?;
    let len = value
        .get("len")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| "missing 'len' field".to_string())? as usize;
    let sample_fields = value.get("sample").and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|item| item.as_str().map(ToOwned::to_owned))
            .collect::<Vec<_>>()
    });
    Ok(ChildDecodeInfo { len, sample_fields })
}

fn decode_step_mode() {
    let payload = std::env::var("GENESIS_DEBUG_PAYLOAD")
        .unwrap_or_else(|_| panic!("GENESIS_DEBUG_PAYLOAD must be set"));
    let bytes = BASE64
        .decode(payload.as_bytes())
        .unwrap_or_else(|err| panic!("base64 decode failed: {err}"));

    match decode_adaptive::<ConstVec<InstructionBox>>(&bytes) {
        Ok(decoded) => {
            let len = decoded.len();
            let sample = decoded
                .iter()
                .take(3)
                .map(format_instruction_path)
                .collect::<Vec<_>>();
            let rendered = json::to_json(&norito::json!({
                "len": len,
                "sample": sample,
            }))
            .expect("serialize child result JSON");
            println!("{rendered}");
            std::process::exit(0);
        }
        Err(err) => {
            eprintln!("ERR {err:?}");
            std::process::exit(1);
        }
    }
}

fn format_instruction_path(instruction: &InstructionBox) -> String {
    format!("{instruction}")
}
