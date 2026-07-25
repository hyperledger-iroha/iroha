#![no_main]

use arbitrary::Arbitrary;
use flate2::{
    Compression,
    write::{DeflateEncoder, GzEncoder},
};
use libfuzzer_sys::fuzz_target;
use norito::json::{Value, to_vec};
use sorafs_car::{
    ChunkStore, PorProof,
    por_json::sample_to_map,
    proof_stream_transport::{MAX_PROOF_STREAM_ITEMS, decode_transport_items},
};
use std::{io::Write, sync::OnceLock};
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

fn canonical_por_sample() -> &'static (usize, PorProof) {
    static SAMPLE: OnceLock<(usize, PorProof)> = OnceLock::new();
    SAMPLE.get_or_init(|| {
        let payload = (0_u16..512)
            .map(|value| u8::try_from(value % 251).expect("fixture byte"))
            .collect::<Vec<_>>();
        let mut store = ChunkStore::new();
        store
            .ingest_bytes(&payload)
            .expect("ingest canonical fuzz PoR fixture");
        store
            .sample_leaves(1, 7, &payload)
            .expect("sample canonical fuzz PoR fixture")
            .into_iter()
            .next()
            .expect("one canonical fuzz PoR sample")
    })
}

fn canonical_line(bytes: &[u8], index: usize) -> Vec<u8> {
    let (flat_index, proof) = canonical_por_sample();
    let manifest_tag = bytes
        .first()
        .copied()
        .filter(|byte| *byte != 0)
        .unwrap_or(1);
    let provider_tag = bytes.get(1).copied().filter(|byte| *byte != 0).unwrap_or(2);
    let latency_ms = u32::try_from(index).unwrap_or(u32::MAX);

    let mut map = sample_to_map(*flat_index, proof);
    map.insert(
        "manifest_digest_hex".into(),
        Value::from(format!("{manifest_tag:02x}").repeat(32)),
    );
    map.insert(
        "provider_id_hex".into(),
        Value::from(format!("{provider_tag:02x}").repeat(32)),
    );
    map.insert("proof_kind".into(), Value::from("por"));
    map.insert("result".into(), Value::from("success"));
    map.insert("latency_ms".into(), Value::from(u64::from(latency_ms)));
    to_vec(&Value::Object(map)).expect("encode canonical fuzz PoR row")
}

fn build_line(line: &FuzzLine, index: usize) -> Vec<u8> {
    if line.well_formed {
        // Exercise the complete successful decoder path with a bounded canonical PoR witness.
        // Only two fuzz bytes select non-zero identities; arbitrary payload bytes never inflate
        // the proof row.
        canonical_line(&line.bytes, index)
    } else {
        // Feed raw bytes (including potential invalid UTF-8) to exercise error paths.
        line.bytes.clone()
    }
}

fn build_payload(case: &FuzzCase) -> (Option<&'static str>, Vec<u8>) {
    let mut joined: Vec<u8> = Vec::new();
    if case.lines.is_empty() {
        joined.extend_from_slice(&canonical_line(&[], 0));
        joined.push(b'\n');
    } else {
        for (idx, line) in case
            .lines
            .iter()
            .take(MAX_PROOF_STREAM_ITEMS.saturating_add(1))
            .enumerate()
        {
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
