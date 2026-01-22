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
use iroha_data_model::{
    asset::id::AssetDefinitionId,
    fastpq::{TRANSFER_TRANSCRIPTS_METADATA_KEY, TransferDeltaTranscript, TransferTranscript},
};
use iroha_primitives::numeric::Numeric;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use norito::{decode_from_bytes, json, to_bytes};
use std::str::FromStr;

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
            let transcript = sample_transfer_transcript();
            for transition in sample_transfer_transitions(&transcript) {
                batch.push(transition);
            }
            batch.metadata.insert(
                TRANSFER_TRANSCRIPTS_METADATA_KEY.into(),
                to_bytes(&vec![transcript]).expect("encode transcripts"),
            );
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

fn sample_transfer_transcript() -> TransferTranscript {
    let delta = TransferDeltaTranscript {
        from_account: (*ALICE_ID).clone(),
        to_account: (*BOB_ID).clone(),
        asset_definition: AssetDefinitionId::from_str("xor#fixture").expect("asset definition"),
        amount: Numeric::from(75u32),
        from_balance_before: Numeric::from(1_000u32),
        from_balance_after: Numeric::from(925u32),
        to_balance_before: Numeric::from(75u32),
        to_balance_after: Numeric::from(150u32),
        from_merkle_proof: None,
        to_merkle_proof: None,
    };
    let batch_hash = Hash::prehashed([0x11; 32]);
    let digest = fastpq_prover::gadgets::transfer::compute_poseidon_digest(&delta, &batch_hash);
    TransferTranscript {
        batch_hash,
        deltas: vec![delta],
        authority_digest: Hash::new(b"authority"),
        poseidon_preimage_digest: Some(digest),
    }
}

fn sample_transfer_transitions(transcript: &TransferTranscript) -> Vec<StateTransition> {
    transcript
        .deltas
        .iter()
        .flat_map(|delta| {
            let sender = StateTransition::new(
                format!("asset/{}/{}", delta.asset_definition, delta.from_account).into_bytes(),
                numeric_to_bytes(&delta.from_balance_before),
                numeric_to_bytes(&delta.from_balance_after),
                OperationKind::Transfer,
            );
            let receiver = StateTransition::new(
                format!("asset/{}/{}", delta.asset_definition, delta.to_account).into_bytes(),
                numeric_to_bytes(&delta.to_balance_before),
                numeric_to_bytes(&delta.to_balance_after),
                OperationKind::Transfer,
            );
            [sender, receiver]
        })
        .collect()
}

fn numeric_to_bytes(value: &Numeric) -> Vec<u8> {
    let amount: u64 = value.clone().try_into().expect("numeric fits u64");
    amount.to_le_bytes().to_vec()
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
            "1bc3c36c257eff44f2a3ad56c9a49291d893aaf8373164ed664542602ba3409f",
        ),
        (
            "mint",
            "04bed7722e55216cc980063b6ed9393e964bba3ef76c5892240f3a09274caa0b",
        ),
        (
            "burn",
            "9ba538400a7d03c24b62a60fb7c4e2dc0681d6e14477052e3968f76cb793fc81",
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
