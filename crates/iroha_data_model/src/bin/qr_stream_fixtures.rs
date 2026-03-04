//! Regenerate QR stream fixtures shared across SDKs.
//!
//! Run with `cargo run -p iroha_data_model --features test-fixtures --bin qr_stream_fixtures`
//! to refresh `fixtures/qr_stream/*.json`. Use `--check` to verify fixtures are up to date.

use std::{env, error::Error, fs, path::Path};

use hex::encode;
use iroha_data_model::qr_stream::{QrPayloadKind, QrStreamEncoder, QrStreamOptions};
use norito::json::{self, Value};

const BASIC_FIXTURE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/qr_stream/qr_stream_basic.json"
);
const PARITY_FIXTURE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/qr_stream/qr_stream_parity.json"
);

fn main() -> Result<(), Box<dyn Error>> {
    let check_only = env::args().any(|arg| arg == "--check");
    let basic_payload = b"iroha-qr-stream-basic-v1".to_vec();
    let parity_payload = b"Iroha QR stream parity fixture payload v1. ".repeat(6);

    let basic = build_fixture(
        &basic_payload,
        200,
        0,
        QrPayloadKind::OfflineToOnlineTransfer,
    )?;
    let parity = build_fixture(&parity_payload, 180, 3, QrPayloadKind::OfflineSpendReceipt)?;

    write_fixture(BASIC_FIXTURE_PATH, &basic, check_only)?;
    write_fixture(PARITY_FIXTURE_PATH, &parity, check_only)?;
    Ok(())
}

fn build_fixture(
    payload: &[u8],
    chunk_size: u16,
    parity_group: u8,
    payload_kind: QrPayloadKind,
) -> Result<Value, Box<dyn Error>> {
    let options = QrStreamOptions {
        chunk_size,
        parity_group,
        payload_kind,
        ..QrStreamOptions::default()
    };
    let (envelope, frames) = QrStreamEncoder::encode_frames(payload, options)?;
    let frames_value = frames
        .into_iter()
        .map(|frame| {
            let kind = match frame.kind {
                iroha_data_model::qr_stream::QrStreamFrameKind::Header => "header",
                iroha_data_model::qr_stream::QrStreamFrameKind::Data => "data",
                iroha_data_model::qr_stream::QrStreamFrameKind::Parity => "parity",
            };
            json::json!({
                "kind": kind,
                "bytes_hex": encode(frame.encode()),
            })
        })
        .collect::<Vec<_>>();

    let payload_kind_label = match payload_kind {
        QrPayloadKind::OfflineToOnlineTransfer => "offline_to_online_transfer",
        QrPayloadKind::OfflineSpendReceipt => "offline_spend_receipt",
        QrPayloadKind::OfflineEnvelope => "offline_envelope",
        QrPayloadKind::Unspecified => "unspecified",
    };

    Ok(json::json!({
        "fixture_version": 1,
        "payload_hex": encode(payload),
        "options": {
            "chunk_size": chunk_size as u64,
            "parity_group": parity_group as u64,
            "payload_kind": payload_kind_label,
        },
        "envelope_hex": encode(envelope.encode()),
        "frames": frames_value,
    }))
}

fn write_fixture(path: &str, value: &Value, check_only: bool) -> Result<(), Box<dyn Error>> {
    let rendered = json::to_string_pretty(value)?;
    if check_only {
        let existing = fs::read_to_string(path)?;
        if existing.trim() != rendered.trim() {
            return Err(format!(
                "fixture {} is stale; run cargo run -p iroha_data_model --features test-fixtures --bin qr_stream_fixtures",
                path
            )
            .into());
        }
        return Ok(());
    }
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, format!("{rendered}\n"))?;
    Ok(())
}
