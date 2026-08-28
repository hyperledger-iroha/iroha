//! Trace commitment regression tests built from Norito fixtures.
use crate::common::{fixture_update_requested, fixture_update_requested_from};
use fastpq_isi::CANONICAL_PARAMETER_SETS;
use fastpq_prover::{
    OperationKind, PublicInputs, StateTransition, TransitionBatch,
    gadgets::transfer::attach_transfer_smt_witnesses, ordering_hash, trace_commitment,
};
use iroha_crypto::Hash;
use iroha_data_model::{
    asset::id::AssetDefinitionId,
    domain::DomainId,
    fastpq::{TRANSFER_TRANSCRIPTS_METADATA_KEY, TransferDeltaTranscript, TransferTranscript},
};
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use norito::{decode_from_bytes, json, to_bytes};
use std::{
    fmt::Write as _,
    path::{Path, PathBuf},
};
fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}
fn fixture_path(name: &str) -> PathBuf {
    fixtures_dir().join(format!("{name}.norito"))
}
fn load_fixture(name: &str) -> TransitionBatch {
    let path = fixture_path(name);
    if fixture_update_requested() {
        let fresh = build_fixture(name);
        let encoded = norito::core::to_bytes(&fresh).expect("encode fixture");
        std::fs::write(&path, &encoded).expect("write fixture");
        return fresh;
    }
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("read FASTPQ fixture {}: {error}", path.display()));
    decode_from_bytes(&bytes).unwrap_or_else(|error| {
        panic!(
            "decode FASTPQ fixture {}: {error}; fixture regeneration is explicit and requires \
             FASTPQ_UPDATE_FIXTURES=1",
            path.display()
        )
    })
}
fn build_fixture(name: &str) -> TransitionBatch {
    let mut batch =
        TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
    batch.public_inputs.dsid = [0xAA; 16];
    batch.public_inputs.slot = 42;
    batch.public_inputs.old_root = [0x11; 32];
    batch.public_inputs.new_root = [0x22; 32];
    batch.public_inputs.perm_root = [0x33; 32];
    batch.public_inputs.tx_set_hash = [0x44; 32];
    match name {
        "transfer" => {
            let mut transcripts = vec![sample_transfer_transcript()];
            let (old_root, new_root) =
                attach_transfer_smt_witnesses(&mut transcripts).expect("attach witnesses");
            batch.public_inputs.old_root = old_root;
            batch.public_inputs.new_root = new_root;
            let transcript = transcripts.pop().expect("transcript");
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
    let asset_definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("fixture", "universal").unwrap(),
        "xor".parse().unwrap(),
    );
    let delta = TransferDeltaTranscript {
        from_account: (*ALICE_ID).clone(),
        to_account: (*BOB_ID).clone(),
        asset_definition,
        amount: Quantity::from(75u32),
        from_balance_before: Quantity::from(1_000u32),
        from_balance_after: Quantity::from(925u32),
        to_balance_before: Quantity::from(75u32),
        to_balance_after: Quantity::from(150u32),
        from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
        to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
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
fn numeric_to_bytes(value: &Quantity) -> Vec<u8> {
    let amount: u64 = value
        .as_numeric()
        .clone()
        .try_into()
        .expect("quantity fits u64");
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
    let update = fixture_update_requested();
    if update {
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
fn assert_fixture_semantics_eq(name: &str, actual: &TransitionBatch, expected: &TransitionBatch) {
    assert_eq!(actual.parameter, expected.parameter, "{name} parameter");
    assert_eq!(
        actual.public_inputs, expected.public_inputs,
        "{name} public inputs"
    );
    assert_eq!(actual.metadata, expected.metadata, "{name} metadata");
    assert_eq!(
        actual.transitions.len(),
        expected.transitions.len(),
        "{name} transition count"
    );
    for (index, (actual, expected)) in actual
        .transitions
        .iter()
        .zip(&expected.transitions)
        .enumerate()
    {
        assert_eq!(actual.key, expected.key, "{name} transition {index} key");
        assert_eq!(
            actual.pre_value, expected.pre_value,
            "{name} transition {index} pre-value"
        );
        assert_eq!(
            actual.post_value, expected.post_value,
            "{name} transition {index} post-value"
        );
        assert_eq!(
            actual.operation, expected.operation,
            "{name} transition {index} operation"
        );
    }
}
#[test]
fn fixture_update_gate_requires_exact_one() {
    use std::ffi::OsStr;
    assert_eq!(fixture_update_requested_from(None), Ok(false));
    assert_eq!(
        fixture_update_requested_from(Some(OsStr::new("1"))),
        Ok(true)
    );
    for invalid in ["", "0", "true", " 1", "1 ", "01", "1\n"] {
        assert!(
            fixture_update_requested_from(Some(OsStr::new(invalid))).is_err(),
            "unexpectedly accepted FASTPQ_UPDATE_FIXTURES={invalid:?}"
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt as _;
        assert!(fixture_update_requested_from(Some(OsStr::from_bytes(b"1\xff"))).is_err());
    }
}
#[test]
fn fixtures_roundtrip_via_norito() {
    for name in ["transfer", "mint", "burn"] {
        let batch = load_fixture(name);
        assert_fixture_semantics_eq(name, &batch, &build_fixture(name));
        let bytes = std::fs::read(fixture_path(name)).expect("read fixture");
        let encoded = norito::core::to_bytes(&batch).expect("encode");
        assert_eq!(
            bytes, encoded,
            "{name} fixture is not the canonical Norito encoding of its decoded semantics"
        );
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
fn trace_commitment_is_canonical_deterministic_and_fixture_separated() {
    let params = CANONICAL_PARAMETER_SETS
        .iter()
        .find(|set| set.name == "fastpq-state-transition-stark-v1")
        .copied()
        .expect("canonical parameter set");
    let mut commitments = std::collections::BTreeSet::new();
    for name in ["transfer", "mint", "burn"] {
        let batch = load_fixture(name);
        let commitment = trace_commitment(&params, &batch).expect("trace commitment");
        let encoded = commitment.to_le_bytes();
        assert_eq!(
            iroha_data_model::privacy::GoldilocksDigest384V1::from_le_bytes(encoded),
            Some(commitment),
            "{name} commitment must remain canonically encoded"
        );
        assert_eq!(
            trace_commitment(&params, &batch).expect("repeat trace commitment"),
            commitment,
            "{name} commitment must be deterministic"
        );
        assert!(
            commitments.insert(encoded),
            "{name} must not collide with another canonical fixture"
        );
    }
}
