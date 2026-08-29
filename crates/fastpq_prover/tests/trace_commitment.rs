//! Canonical ordering and trace-commitment regression tests.
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
use norito::{json, to_bytes};
use std::{fmt::Write as _, path::PathBuf};

const FIXTURE_NAMES: [&str; 2] = ["transfer", "metadata"];

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
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
        "metadata" => {
            batch.push(StateTransition::new(
                b"metadata/reserve".to_vec(),
                b"old".to_vec(),
                b"new".to_vec(),
                OperationKind::MetaSet,
            ));
            batch.push(StateTransition::new(
                b"metadata/treasury".to_vec(),
                b"pending".to_vec(),
                b"active".to_vec(),
                OperationKind::MetaSet,
            ));
        }
        other => panic!("unknown fixture {other}"),
    }
    batch.sort();
    batch
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
fn update_hash_fixture(file_name: &str, hash_batch: impl Fn(&TransitionBatch) -> Hash) {
    let mut map = json::native::Map::new();
    for name in FIXTURE_NAMES {
        let digest: [u8; Hash::LENGTH] = hash_batch(&build_fixture(name)).into();
        map.insert(name.to_owned(), json::Value::from(bytes_to_hex(&digest)));
    }
    let value = json::Value::Object(map);
    let json_text = json::to_json_pretty(&value).expect("serialize hash fixture");
    std::fs::write(fixtures_dir().join(file_name), json_text).expect("write hash fixture");
}

fn load_hash_expectations(file_name: &str) -> Vec<(&'static str, String)> {
    let path = fixtures_dir().join(file_name);
    let bytes = std::fs::read(&path).unwrap_or_else(|err| panic!("read {file_name}: {err}"));
    let value: json::Value = json::from_slice(&bytes).expect("parse hash fixture");
    let object = value
        .as_object()
        .unwrap_or_else(|| panic!("{file_name} must contain an object"));
    assert_eq!(
        object.len(),
        FIXTURE_NAMES.len(),
        "{file_name} must contain exactly the current release fixtures"
    );
    FIXTURE_NAMES
        .into_iter()
        .map(|name| {
            let value = object
                .get(name)
                .unwrap_or_else(|| panic!("{file_name} is missing {name}"));
            let hex = value
                .as_str()
                .unwrap_or_else(|| panic!("{file_name} fixture {name} must be a string"));
            (name, hex.to_owned())
        })
        .collect()
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
fn ordering_hash_matches_golden_vectors() {
    if fixture_update_requested() {
        update_hash_fixture("ordering_hash.json", |batch| {
            ordering_hash(batch).expect("ordering hash")
        });
    }
    for (name, expected_hex) in load_hash_expectations("ordering_hash.json") {
        let batch = build_fixture(name);
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
    if fixture_update_requested() {
        update_hash_fixture("trace_commitment.json", |batch| {
            trace_commitment(&params, batch).expect("trace commitment")
        });
    }
    for (name, expected_hex) in load_hash_expectations("trace_commitment.json") {
        let batch = build_fixture(name);
        let commitment = trace_commitment(&params, &batch).expect("trace commitment");
        let actual: [u8; Hash::LENGTH] = commitment.into();
        let actual_hex = bytes_to_hex(&actual);
        assert!(
            !expected_hex.is_empty(),
            "missing golden commitment for {name}"
        );
        assert_eq!(actual_hex, expected_hex, "commitment {name}");
    }
}
