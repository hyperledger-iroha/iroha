use assert_cmd::cargo::cargo_bin_cmd;
use norito::json::Value;
use sorafs_car::ChunkStore;
use sorafs_chunker::fixtures::FixtureProfile;
use tempfile::{NamedTempFile, tempdir};

fn write_payload(path: &std::path::PathBuf, size: usize) -> Vec<u8> {
    let mut buf = vec![0u8; size];
    for (idx, byte) in buf.iter_mut().enumerate() {
        *byte = (idx as u8).wrapping_mul(31).wrapping_add(7);
    }
    std::fs::write(path, &buf).expect("write payload");
    buf
}

#[test]
fn cli_emits_chunk_metadata_for_fixture() {
    let fixture = FixtureProfile::SF1_V1.generate_vectors();
    let mut file = NamedTempFile::new().expect("tempfile");
    std::io::Write::write_all(&mut file, &fixture.input).expect("write fixture");

    let output = cargo_bin_cmd!("sorafs_chunk_store")
        .arg(file.path())
        .output()
        .expect("run chunk store CLI");
    assert!(output.status.success(), "cli exited with failure");

    let json: Value =
        norito::json::from_slice(&output.stdout).expect("parse chunk store JSON output");
    assert_eq!(
        json.get("chunk_count")
            .and_then(Value::as_u64)
            .expect("chunk_count"),
        fixture.chunk_lengths.len() as u64
    );

    let payload_digest = json
        .get("payload_digest_blake3")
        .and_then(Value::as_str)
        .expect("payload digest");
    assert_eq!(
        payload_digest,
        to_hex(blake3::hash(&fixture.input).as_bytes())
    );

    let input_bytes = json
        .get("input_bytes")
        .and_then(Value::as_u64)
        .expect("input_bytes field");
    assert_eq!(input_bytes as usize, fixture.input.len());

    let por_root = json
        .get("por_root_hex")
        .and_then(Value::as_str)
        .expect("por_root_hex");
    let mut store = ChunkStore::new();
    store.ingest_bytes(&fixture.input);
    assert_eq!(por_root, to_hex(store.por_tree().root()));

    let specs = json
        .get("chunk_fetch_specs")
        .and_then(Value::as_array)
        .expect("chunk_fetch_specs array");
    assert_eq!(specs.len(), fixture.chunk_lengths.len());
    let first_spec = specs[0].as_object().expect("chunk fetch spec object");
    assert_eq!(
        first_spec
            .get("chunk_index")
            .and_then(Value::as_u64)
            .expect("chunk index"),
        0
    );
    assert_eq!(
        first_spec
            .get("offset")
            .and_then(Value::as_u64)
            .expect("chunk offset"),
        0
    );
    assert_eq!(
        first_spec
            .get("length")
            .and_then(Value::as_u64)
            .expect("chunk length"),
        fixture.chunk_lengths[0] as u64
    );
}

#[test]
fn cli_lists_registered_profiles() {
    let output = cargo_bin_cmd!("sorafs_chunk_store")
        .arg("--list-profiles")
        .output()
        .expect("run list-profiles");
    assert!(output.status.success(), "cli exited with failure");

    let json: Value = norito::json::from_slice(&output.stdout).expect("parse profile list JSON");
    let array = json.as_array().expect("profile list array");
    assert!(!array.is_empty(), "expected at least one profile");
    let first = array[0].as_object().expect("profile descriptor object");
    assert_eq!(
        first
            .get("profile_id")
            .and_then(Value::as_u64)
            .expect("profile_id"),
        1
    );
    assert_eq!(
        first
            .get("name")
            .and_then(Value::as_str)
            .expect("profile name"),
        "sf1"
    );
    assert_eq!(
        first
            .get("handle")
            .and_then(Value::as_str)
            .expect("profile handle"),
        "sorafs.sf1@1.0.0"
    );
}

#[test]
fn cli_accepts_profile_handle() {
    let fixture = FixtureProfile::SF1_V1.generate_vectors();
    let mut file = NamedTempFile::new().expect("tempfile");
    std::io::Write::write_all(&mut file, &fixture.input).expect("write fixture");

    let output = cargo_bin_cmd!("sorafs_chunk_store")
        .arg(file.path())
        .arg("--profile=sorafs.sf1@1.0.0")
        .output()
        .expect("run chunk store CLI");
    assert!(output.status.success(), "cli exited with failure");

    let json: Value = norito::json::from_slice(&output.stdout).expect("parse chunk store JSON");
    let profile = json
        .get("profile")
        .and_then(Value::as_object)
        .expect("profile object");
    assert_eq!(
        profile
            .get("profile_id")
            .and_then(Value::as_u64)
            .expect("profile id"),
        1
    );
    assert_eq!(
        profile
            .get("name")
            .and_then(Value::as_str)
            .expect("profile name"),
        "sf1"
    );
    assert_eq!(
        profile
            .get("handle")
            .and_then(Value::as_str)
            .expect("profile handle"),
        "sorafs.sf1@1.0.0"
    );
}

#[test]
fn cli_rejects_conflicting_profile_flags() {
    let fixture = FixtureProfile::SF1_V1.generate_vectors();
    let mut file = NamedTempFile::new().expect("tempfile");
    std::io::Write::write_all(&mut file, &fixture.input).expect("write fixture");

    let output = cargo_bin_cmd!("sorafs_chunk_store")
        .arg(file.path())
        .arg("--profile-id=1")
        .arg("--profile=sorafs.sf1@1.0.0")
        .output()
        .expect("run chunk store CLI");
    assert!(
        !output.status.success(),
        "cli should fail on conflicting flags"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("use either --profile-id or --profile"),
        "expected conflict message, got {stderr}"
    );
}

#[test]
fn cli_writes_por_json() {
    let fixture = FixtureProfile::SF1_V1.generate_vectors();
    let mut file = NamedTempFile::new().expect("tempfile");
    std::io::Write::write_all(&mut file, &fixture.input).expect("write fixture");
    let por_path = NamedTempFile::new().expect("por file");
    let por_path = por_path.into_temp_path();

    let output = cargo_bin_cmd!("sorafs_chunk_store")
        .arg(file.path())
        .arg(format!("--por-json-out={}", por_path.display()))
        .output()
        .expect("run chunk store CLI");
    assert!(output.status.success(), "cli exited with failure");

    let por_json: Value = norito::json::from_slice(&std::fs::read(&por_path).expect("read por"))
        .expect("parse por json");
    let root_hex = por_json
        .get("root_hex")
        .and_then(Value::as_str)
        .expect("root hex");
    let mut store = ChunkStore::new();
    store.ingest_bytes(&fixture.input);
    assert_eq!(root_hex, to_hex(store.por_tree().root()));
}

#[test]
fn cli_writes_por_proof() {
    let fixture = FixtureProfile::SF1_V1.generate_vectors();
    let mut file = NamedTempFile::new().expect("tempfile");
    std::io::Write::write_all(&mut file, &fixture.input).expect("write fixture");
    let proof_path = NamedTempFile::new().expect("proof file").into_temp_path();

    let output = cargo_bin_cmd!("sorafs_chunk_store")
        .arg(file.path())
        .arg("--por-proof=0:0:0")
        .arg(format!("--por-proof-out={}", proof_path.display()))
        .output()
        .expect("run chunk store CLI");
    assert!(output.status.success(), "cli exited with failure");

    let stdout_json: Value = norito::json::from_slice(&output.stdout).expect("parse stdout json");
    let proof_value = stdout_json
        .get("por_proof")
        .and_then(Value::as_object)
        .expect("por proof object");
    assert_eq!(
        proof_value
            .get("chunk_index")
            .and_then(Value::as_u64)
            .expect("chunk index"),
        0
    );

    let proof_bytes = std::fs::read::<&std::path::Path>(proof_path.as_ref()).expect("read proof");
    let file_json: Value = norito::json::from_slice(&proof_bytes).expect("parse proof file");
    assert_eq!(
        &file_json,
        stdout_json.get("por_proof").expect("proof in stdout")
    );

    let mut store = ChunkStore::new();
    store.ingest_bytes(&fixture.input);
    let tree = store.por_tree();
    let proof = tree
        .prove_leaf(0, 0, 0, &fixture.input)
        .expect("generate proof");
    let expected_leaf_hex = to_hex(&proof.leaf_bytes);
    let leaf_hex = proof_value
        .get("leaf_bytes_hex")
        .and_then(Value::as_str)
        .expect("leaf hex");
    assert_eq!(leaf_hex, expected_leaf_hex);
    assert!(proof.verify(tree.root()));

    let verify = cargo_bin_cmd!("sorafs_chunk_store")
        .arg(file.path())
        .arg(format!("--por-proof-verify={}", proof_path.display()))
        .output()
        .expect("run verification CLI");
    assert!(verify.status.success(), "verification run failed");
    let verify_json: Value = norito::json::from_slice(&verify.stdout).expect("parse verify json");
    assert!(
        verify_json
            .get("por_proof_verified")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "expected por_proof_verified field"
    );
}

#[test]
fn cli_writes_chunk_fetch_plan_json() {
    let fixture = FixtureProfile::SF1_V1.generate_vectors();
    let mut file = NamedTempFile::new().expect("tempfile");
    std::io::Write::write_all(&mut file, &fixture.input).expect("write fixture");
    let plan_path = NamedTempFile::new().expect("plan file").into_temp_path();

    let output = cargo_bin_cmd!("sorafs_chunk_store")
        .arg(file.path())
        .arg(format!("--chunk-fetch-plan-out={}", plan_path.display()))
        .output()
        .expect("run chunk store CLI");
    assert!(output.status.success(), "cli exited with failure");

    let stdout_json: Value = norito::json::from_slice(&output.stdout).expect("parse stdout json");
    let stdout_specs = stdout_json
        .get("chunk_fetch_specs")
        .cloned()
        .expect("chunk_fetch_specs in stdout");
    let file_json: Value = norito::json::from_slice(&std::fs::read(&plan_path).expect("read plan"))
        .expect("parse plan json");
    assert_eq!(stdout_specs, file_json);
}

#[test]
fn cli_writes_report_to_stdout_when_json_dash() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("payload.bin");
    write_payload(&payload_path, 4096);

    let assert = cargo_bin_cmd!("sorafs_chunk_store")
        .arg(&payload_path)
        .arg("--json-out=-")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let report: Value = norito::json::from_str(&stdout).expect("parse stdout json");
    assert_eq!(
        report
            .get("chunk_count")
            .and_then(Value::as_u64)
            .expect("chunk_count"),
        1
    );
}

#[test]
fn cli_writes_por_json_to_stdout_when_dash() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("input.bin");
    write_payload(&payload_path, 8 * 1024);

    let assert = cargo_bin_cmd!("sorafs_chunk_store")
        .arg(&payload_path)
        .arg("--por-json-out=-")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let por_tree_value: Value = norito::json::from_str(&stdout).expect("parse stdout json");
    assert!(
        por_tree_value.is_object(),
        "expected PoR JSON object when streaming to stdout"
    );
}

#[test]
fn cli_writes_chunk_fetch_plan_to_stdout_when_dash() {
    let tempdir = tempdir().expect("tempdir");
    let payload_path = tempdir.path().join("input.bin");
    write_payload(&payload_path, 8 * 1024);

    let assert = cargo_bin_cmd!("sorafs_chunk_store")
        .arg(&payload_path)
        .arg("--chunk-fetch-plan-out=-")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let plan_value: Value = norito::json::from_str(&stdout).expect("parse chunk fetch specs");
    let specs = plan_value
        .as_array()
        .expect("chunk fetch specs array from stdout");
    assert!(!specs.is_empty(), "expected chunk fetch specs entries");
}

#[test]
fn cli_samples_por_leaves() {
    let fixture = FixtureProfile::SF1_V1.generate_vectors();
    let mut file = NamedTempFile::new().expect("tempfile");
    std::io::Write::write_all(&mut file, &fixture.input).expect("write fixture");
    let sample_path = NamedTempFile::new().expect("sample file").into_temp_path();

    let output = cargo_bin_cmd!("sorafs_chunk_store")
        .arg(file.path())
        .arg("--por-sample=3")
        .arg("--por-sample-seed=42")
        .arg(format!("--por-sample-out={}", sample_path.display()))
        .output()
        .expect("run sampling CLI");
    assert!(output.status.success(), "cli exited with failure");

    let stdout_json: Value = norito::json::from_slice(&output.stdout).expect("parse stdout json");
    let samples = stdout_json
        .get("por_samples")
        .and_then(Value::as_array)
        .expect("por_samples array");
    assert_eq!(samples.len(), 3, "expected three samples");

    let mut seen = std::collections::HashSet::new();
    let mut store = ChunkStore::new();
    store.ingest_bytes(&fixture.input);
    let tree = store.por_tree();

    for sample in samples {
        let sample = sample.as_object().expect("sample object");
        let flat = sample
            .get("leaf_index_flat")
            .and_then(Value::as_u64)
            .expect("flat index") as usize;
        assert!(seen.insert(flat), "duplicate flat index");
        let chunk_idx = sample
            .get("chunk_index")
            .and_then(Value::as_u64)
            .expect("chunk index") as usize;
        let segment_idx = sample
            .get("segment_index")
            .and_then(Value::as_u64)
            .expect("segment index") as usize;
        let leaf_idx = sample
            .get("leaf_index")
            .and_then(Value::as_u64)
            .expect("leaf index") as usize;
        let proof_value = sample
            .get("proof")
            .and_then(Value::as_object)
            .expect("proof object");
        let proof = tree
            .prove_leaf(chunk_idx, segment_idx, leaf_idx, &fixture.input)
            .expect("generate proof");
        let expected_leaf_hex = to_hex(&proof.leaf_bytes);
        assert_eq!(
            proof_value
                .get("leaf_bytes_hex")
                .and_then(Value::as_str)
                .expect("leaf bytes hex"),
            expected_leaf_hex
        );
    }

    let sample_path_ref: &std::path::Path = sample_path.as_ref();
    let file_samples: Value =
        norito::json::from_slice(&std::fs::read(sample_path_ref).expect("read sample file"))
            .expect("parse sample file");
    assert_eq!(
        &file_samples,
        stdout_json.get("por_samples").expect("samples in stdout")
    );
}

#[test]
fn car_cli_truncates_samples_when_request_exceeds_leaves() {
    let fixture = FixtureProfile::SF1_V1.generate_vectors();
    let mut file = NamedTempFile::new().expect("tempfile");
    std::io::Write::write_all(&mut file, &fixture.input).expect("write fixture");

    let total_leaves = {
        let mut store = ChunkStore::new();
        store.ingest_bytes(&fixture.input);
        store.por_tree().leaf_count()
    };
    let request = total_leaves + 5;

    let output = cargo_bin_cmd!("sorafs_chunk_store")
        .arg(file.path())
        .arg(format!("--por-sample={request}"))
        .arg("--por-sample-seed=1234")
        .output()
        .expect("run sampling CLI");
    assert!(output.status.success(), "cli exited with failure");

    let stdout_json: Value = norito::json::from_slice(&output.stdout).expect("parse stdout json");
    let samples = stdout_json
        .get("por_samples")
        .and_then(Value::as_array)
        .expect("por_samples array");
    assert!(!samples.is_empty());
    assert!(
        stdout_json
            .get("por_samples_truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "expected truncation flag when requesting more leaves than available"
    );
}

fn to_hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(TABLE[(byte >> 4) as usize] as char);
        out.push(TABLE[(byte & 0x0f) as usize] as char);
    }
    out
}
