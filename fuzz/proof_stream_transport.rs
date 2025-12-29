#![no_main]

use arbitrary::Arbitrary;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use flate2::{
    Compression,
    write::{DeflateEncoder, GzEncoder},
};
use libfuzzer_sys::fuzz_target;
use sorafs_car::proof_stream_transport::decode_transport_items;
use std::io::Write;
use zstd::stream::encode_all as encode_zstd;

#[derive(Debug, Arbitrary)]
struct FuzzLine {
    /// When true we emit a well-formed JSON object, otherwise raw bytes.
    well_formed: bool,
    /// Raw payload used either as JSON fragment or arbitrary bytes.
    bytes: Vec<u8>,
}

#[derive(Debug, Arbitrary)]
struct FuzzCase {
    /// Encoding selector (0 = identity, 1 = gzip, 2 = deflate, 3 = zstd, others = mixed).
    encoding: u8,
    lines: Vec<FuzzLine>,
}

fn build_line(line: &FuzzLine, index: usize) -> Vec<u8> {
    if line.well_formed {
        // Construct a deterministic JSON object embedding the fuzz bytes as base64 so we always
        // produce syntactically valid NDJSON when requested.
        let encoded = BASE64_STANDARD.encode(&line.bytes);
        format!(
            "{{\"verification_status\":\"success\",\"latency_ms\":{},\"payload\":\"{}\"}}",
            index, encoded
        )
        .into_bytes()
    } else {
        // Feed raw bytes (including potential invalid UTF-8) to exercise error paths.
        line.bytes.clone()
    }
}

fn build_payload(case: &FuzzCase) -> (Option<&'static str>, Vec<u8>) {
    let mut joined: Vec<u8> = Vec::new();
    if case.lines.is_empty() {
        joined.extend_from_slice(b"{\"verification_status\":\"success\"}\n");
    } else {
        for (idx, line) in case.lines.iter().enumerate() {
            let mut rendered = build_line(line, idx);
            joined.append(&mut rendered);
            joined.push(b'\n');
        }
    }

    match case.encoding % 5 {
        0 => (None, joined),
        1 => {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            let _ = encoder.write_all(&joined);
            (
                Some("gzip"),
                encoder.finish().unwrap_or_else(|_| Vec::new()),
            )
        }
        2 => {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
            let _ = encoder.write_all(&joined);
            (
                Some("deflate"),
                encoder.finish().unwrap_or_else(|_| Vec::new()),
            )
        }
        3 => (
            Some("zstd"),
            encode_zstd(joined.as_slice(), 3).unwrap_or_else(|_| Vec::new()),
        ),
        _ => {
            // Construct a mixed payload by concatenating identity + gzip blocks to ensure
            // the decoder rejects unsupported combinations gracefully.
            let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
            let _ = encoder.write_all(&joined);
            let gzip = encoder.finish().unwrap_or_default();

            let mut composite = joined.clone();
            composite.extend_from_slice(&gzip);
            (Some("identity"), composite)
        }
    }
}

fuzz_target!(|case: FuzzCase| {
    let (encoding, payload) = build_payload(&case);
    // Ignore the result – we only care that decoding does not panic.
    let _ = decode_transport_items(encoding, &payload);
});
