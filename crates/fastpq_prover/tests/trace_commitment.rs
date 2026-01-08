//! Trace commitment regression tests built from Norito fixtures.

use std::{
    fmt::Write as _,
    path::{Path, PathBuf},
};

use fastpq_isi::CANONICAL_PARAMETER_SETS;
use fastpq_prover::{
    OperationKind, PublicInputs, StateTransition, TransitionBatch, ordering_hash, trace_commitment,
};
use iroha_crypto::Hash;
use norito::{decode_from_bytes, json};

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn fixture_path(name: &str) -> PathBuf {
    fixtures_dir().join(format!("{name}.norito"))
}

fn load_fixture(name: &str) -> TransitionBatch {
    let path = fixture_path(name);
    let update = std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok();
    let mut decoded = std::fs::read(&path)
        .ok()
        .and_then(|bytes| decode_from_bytes(&bytes).ok());

    if update || decoded.is_none() {
        let fresh = build_fixture(name);
        let encoded = norito::core::to_bytes(&fresh).expect("encode fixture");
        std::fs::write(&path, &encoded).expect("write fixture");
        decoded = Some(fresh);
    }

    decoded.expect("fixture available")
}

fn build_fixture(name: &str) -> TransitionBatch {
    let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
    batch.public_inputs.dsid = [0xAA; 16];
    batch.public_inputs.slot = 42;
    batch.public_inputs.old_root = [0x11; 32];
    batch.public_inputs.new_root = [0x22; 32];
    batch.public_inputs.perm_root = [0x33; 32];
    batch.public_inputs.tx_set_hash = [0x44; 32];

    match name {
        "transfer" => {
            batch.push(StateTransition::new(
                b"asset/xor/alice".to_vec(),
                u64_bytes(1_000),
                u64_bytes(925),
                OperationKind::Transfer,
            ));
            batch.push(StateTransition::new(
                b"asset/xor/bob".to_vec(),
                u64_bytes(75),
                u64_bytes(150),
                OperationKind::Transfer,
            ));
        }
        "mint" => {
            batch.push(StateTransition::new(
                b"asset/xor/reserve".to_vec(),
                u64_bytes(4_096),
                u64_bytes(5_120),
                OperationKind::Mint,
            ));
            batch.push(StateTransition::new(
                b"asset/xor/treasury".to_vec(),
                u64_bytes(64),
                u64_bytes(1_024),
                OperationKind::Mint,
            ));
        }
        "burn" => {
            batch.push(StateTransition::new(
                b"asset/xor/liability".to_vec(),
                u64_bytes(8_192),
                u64_bytes(6_656),
                OperationKind::Burn,
            ));
            batch.push(StateTransition::new(
                b"asset/xor/supply".to_vec(),
                u64_bytes(16_384),
                u64_bytes(14_848),
                OperationKind::Burn,
            ));
        }
        other => panic!("unknown fixture {other}"),
    }

    batch.sort();
    batch
}

fn u64_bytes(value: u64) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut result, "{byte:02x}").expect("write to string");
    }
    result
}

fn load_ordering_expectations() -> Vec<(String, String)> {
    let path = fixtures_dir().join("ordering_hash.json");
    let update = std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok();

    if update || !path.exists() {
        let mut map = json::native::Map::new();
        for name in ["transfer", "mint", "burn"] {
            let batch = load_fixture(name);
            let hash = ordering_hash(&batch).expect("ordering hash");
            let digest: [u8; Hash::LENGTH] = hash.into();
            map.insert(name.to_string(), json::Value::from(bytes_to_hex(&digest)));
        }
        let value = json::Value::Object(map);
        let json_text = json::to_json_pretty(&value).expect("serialize ordering_hash.json");
        std::fs::write(&path, json_text).expect("write ordering_hash.json");
    }

    let bytes = std::fs::read(&path).expect("read ordering_hash.json");
    let value: json::Value = json::from_slice(&bytes).expect("parse ordering_hash.json");
    let object = value
        .as_object()
        .expect("ordering_hash.json must contain an object");
    let mut entries: Vec<(String, String)> = object
        .iter()
        .map(|(name, value)| {
            let hex = value
                .as_str()
                .unwrap_or_else(|| panic!("ordering hash fixture {name} must be a string"));
            (name.clone(), hex.to_owned())
        })
        .collect();
    entries.sort_by(|(a, _), (b, _)| a.cmp(b));
    entries
}

#[test]
fn fixtures_roundtrip_via_norito() {
    for name in ["transfer", "mint", "burn"] {
        let batch = load_fixture(name);
        let bytes = std::fs::read(fixture_path(name)).expect("read fixture");
        let encoded = norito::core::to_bytes(&batch).expect("encode");
        assert_eq!(bytes, encoded, "{name} roundtrip");
    }
}

#[test]
fn ordering_hash_matches_golden_vectors() {
    for (name, expected_hex) in load_ordering_expectations() {
        let batch = load_fixture(&name);
        let hash = ordering_hash(&batch).expect("ordering hash");
        let actual: [u8; Hash::LENGTH] = hash.into();
        let actual_hex = bytes_to_hex(&actual);
        assert!(
            !expected_hex.is_empty(),
            "missing golden ordering hash for {name}"
        );
        assert_eq!(actual_hex, expected_hex, "ordering hash {name}");
    }
}

#[test]
fn trace_commitment_matches_golden_vectors() {
    let params = CANONICAL_PARAMETER_SETS
        .iter()
        .find(|set| set.name == "fastpq-lane-balanced")
        .copied()
        .expect("canonical parameter set");
    let expectations: [(&str, &str); 3] = [
        (
            "transfer",
            "8f0e6c4962bc212d9a95d6f81c21e07c352211f2a9ed137b95d17cc2ce8a7e23",
        ),
        (
            "mint",
            "88ca4107c150219a054f8ad5bcd42832ee2aa57522073e50225770ec9b5f472d",
        ),
        (
            "burn",
            "18c0718a22cb72581de085dfac51bff7e5cc33bfbc6a2da94281ee3787a9bb11",
        ),
    ];
    for (name, expected_hex) in expectations {
        let batch = load_fixture(name);
        let commitment = trace_commitment(&params, &batch).expect("trace commitment");
        let actual: [u8; Hash::LENGTH] = commitment.into();
        let actual_hex = bytes_to_hex(&actual);
        println!("{name} => {actual_hex}");
        assert!(
            !expected_hex.is_empty(),
            "missing golden commitment for {name}"
        );
        assert_eq!(actual_hex, expected_hex, "commitment {name}");
    }
}
