//! End-to-end checks for the sorafs-node CLI helpers.

use std::{fs, path::Path};

use assert_cmd::cargo::cargo_bin_cmd;
use blake3::hash;
use sorafs_car::CarBuildPlan;
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{
    BLAKE3_256_MULTIHASH_CODE, DagCodecId, ManifestBuilder, PinPolicy,
    por::{
        POR_CHALLENGE_VERSION_V1, POR_PROOF_VERSION_V1, PorChallengeV1, PorProofSampleV1,
        PorProofV1, derive_challenge_id, derive_challenge_seed,
    },
    provider_advert::{AdvertSignature, SignatureAlgorithm},
};
use tempfile::TempDir;

fn ingest_tests_enabled() -> bool {
    std::env::var("SORAFS_NODE_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")
}

#[test]
fn sorafs_node_cli_ingest_and_export_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    if !ingest_tests_enabled() {
        eprintln!("skipping ingest roundtrip (SORAFS_NODE_SKIP_INGEST_TESTS=1)");
        return Ok(());
    }

    let temp_dir = TempDir::new()?;
    let storage_dir = temp_dir.path().join("storage");

    let payload = b"sorafs-node CLI integration payload";
    let plan = CarBuildPlan::single_file(payload)?;
    let digest = hash(payload);

    let manifest = ManifestBuilder::new()
        .root_cid(digest.as_bytes().to_vec())
        .dag_codec(DagCodecId(0x71))
        .chunking_from_profile(ChunkProfile::DEFAULT, BLAKE3_256_MULTIHASH_CODE)
        .content_length(plan.content_length)
        .car_digest(digest.into())
        .car_size(plan.content_length)
        .pin_policy(PinPolicy::default())
        .build()
        .expect("manifest build");
    let manifest_bytes = norito::to_bytes(&manifest)?;

    let manifest_path = temp_dir.path().join("manifest.to");
    fs::write(&manifest_path, &manifest_bytes)?;
    let payload_path = temp_dir.path().join("payload.bin");
    fs::write(&payload_path, payload)?;

    let ingest_plan_path = temp_dir.path().join("ingest_plan.json");
    let mut ingest = cargo_bin_cmd!("sorafs-node");
    let ingest_assert = ingest
        .arg("ingest")
        .arg(format!("--data-dir={}", storage_dir.display()))
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--payload={}", payload_path.display()))
        .arg(format!("--plan-json-out={}", ingest_plan_path.display()))
        .assert()
        .success();

    let ingest_stdout = String::from_utf8(ingest_assert.get_output().stdout.clone())?;
    let ingest_json: norito::json::Value =
        norito::json::from_str(ingest_stdout.trim()).expect("ingest JSON");
    let manifest_id = ingest_json
        .get("manifest_id_hex")
        .and_then(norito::json::Value::as_str)
        .expect("manifest_id_hex present")
        .to_string();

    assert!(Path::new(&ingest_plan_path).exists());
    let ingest_plan_value: norito::json::Value =
        norito::json::from_slice(&fs::read(&ingest_plan_path)?)?;

    let export_manifest_path = temp_dir.path().join("export_manifest.to");
    let export_payload_path = temp_dir.path().join("export_payload.bin");
    let export_plan_path = temp_dir.path().join("export_plan.json");
    let mut export = cargo_bin_cmd!("sorafs-node");
    let export_assert = export
        .arg("export")
        .arg(format!("--data-dir={}", storage_dir.display()))
        .arg(format!("--manifest-id={manifest_id}"))
        .arg(format!("--manifest-out={}", export_manifest_path.display()))
        .arg(format!("--payload-out={}", export_payload_path.display()))
        .arg(format!("--plan-json-out={}", export_plan_path.display()))
        .assert()
        .success();

    let export_stdout = String::from_utf8(export_assert.get_output().stdout.clone())?;
    let export_json: norito::json::Value =
        norito::json::from_str(export_stdout.trim()).expect("export JSON");
    assert_eq!(
        export_json
            .get("manifest_id_hex")
            .and_then(norito::json::Value::as_str),
        Some(manifest_id.as_str())
    );

    let exported_manifest = fs::read(&export_manifest_path)?;
    let exported_payload = fs::read(&export_payload_path)?;
    assert_eq!(manifest_bytes, exported_manifest);
    assert_eq!(payload.to_vec(), exported_payload);

    let export_plan_value: norito::json::Value =
        norito::json::from_slice(&fs::read(&export_plan_path)?)?;
    assert_eq!(ingest_plan_value, export_plan_value);

    Ok(())
}

#[test]
fn sorafs_node_cli_ingest_por_flow() -> Result<(), Box<dyn std::error::Error>> {
    if !ingest_tests_enabled() {
        eprintln!("skipping PoR ingest flow (SORAFS_NODE_SKIP_INGEST_TESTS=1)");
        return Ok(());
    }

    let temp_dir = TempDir::new()?;
    let storage_dir = temp_dir.path().join("storage");
    fs::create_dir_all(&storage_dir)?;

    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("failed to resolve workspace root")?
        .join("fixtures/sorafs_manifest/por");
    let challenge_path = base.join("challenge_v1.to");
    let proof_path = base.join("proof_v1.to");
    let verdict_path = base.join("verdict_v1.to");

    let mut cmd = cargo_bin_cmd!("sorafs-node");
    let assert = cmd
        .arg("ingest")
        .arg("por")
        .arg(format!("--data-dir={}", storage_dir.display()))
        .arg(format!("--challenge={}", challenge_path.display()))
        .arg(format!("--proof={}", proof_path.display()))
        .arg(format!("--verdict={}", verdict_path.display()))
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone())?;
    let value: norito::json::Value =
        norito::json::from_str(stdout.trim()).expect("por ingest JSON output");

    assert_eq!(
        value
            .get("manifest_digest_hex")
            .and_then(norito::json::Value::as_str),
        Some("4242424242424242424242424242424242424242424242424242424242424242")
    );
    assert_eq!(
        value
            .get("proof_digest_hex")
            .and_then(norito::json::Value::as_str),
        Some("b0fa66e8df64a6ea922344919aeb5732b152618d5b4bc30cb8f0313b55f40025")
    );
    let verdict = value
        .get("verdict")
        .and_then(norito::json::Value::as_object)
        .expect("verdict summary present");
    assert_eq!(
        verdict.get("outcome").and_then(norito::json::Value::as_str),
        Some("success")
    );
    assert_eq!(
        verdict
            .get("success_samples")
            .and_then(norito::json::Value::as_u64),
        Some(3)
    );

    Ok(())
}

#[test]
fn sorafs_node_cli_ingest_por_replays_proof() -> Result<(), Box<dyn std::error::Error>> {
    if !ingest_tests_enabled() {
        eprintln!("skipping PoR replay (SORAFS_NODE_SKIP_INGEST_TESTS=1)");
        return Ok(());
    }

    let temp_dir = TempDir::new()?;
    let storage_dir = temp_dir.path().join("storage");

    let payload = b"sorafs-node PoR replay payload";
    let plan = CarBuildPlan::single_file(payload)?;
    let digest = hash(payload);

    let manifest = ManifestBuilder::new()
        .root_cid(digest.as_bytes().to_vec())
        .dag_codec(DagCodecId(0x71))
        .chunking_from_profile(ChunkProfile::DEFAULT, BLAKE3_256_MULTIHASH_CODE)
        .content_length(plan.content_length)
        .car_digest(digest.into())
        .car_size(plan.content_length)
        .pin_policy(PinPolicy::default())
        .build()
        .expect("manifest build");
    let manifest_bytes = norito::to_bytes(&manifest)?;

    let manifest_path = temp_dir.path().join("manifest_por.to");
    fs::write(&manifest_path, &manifest_bytes)?;
    let payload_path = temp_dir.path().join("payload_por.bin");
    fs::write(&payload_path, payload)?;

    let mut ingest = cargo_bin_cmd!("sorafs-node");
    let ingest_assert = ingest
        .arg("ingest")
        .arg(format!("--data-dir={}", storage_dir.display()))
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--payload={}", payload_path.display()))
        .assert()
        .success();

    let ingest_stdout = String::from_utf8(ingest_assert.get_output().stdout.clone())?;
    let ingest_json: norito::json::Value =
        norito::json::from_str(ingest_stdout.trim()).expect("ingest JSON");
    let manifest_id = ingest_json
        .get("manifest_id_hex")
        .and_then(norito::json::Value::as_str)
        .expect("manifest_id present")
        .to_string();

    let mut challenge = fixture_challenge();
    let manifest_digest: [u8; 32] = manifest.digest()?.into();
    challenge.manifest_digest = manifest_digest;
    challenge.seed = derive_challenge_seed(
        &challenge.drand_randomness,
        challenge.vrf_output.as_ref(),
        &challenge.manifest_digest,
        challenge.epoch_id,
    );
    challenge.challenge_id = derive_challenge_id(
        &challenge.seed,
        &challenge.manifest_digest,
        &challenge.provider_id,
        challenge.epoch_id,
        challenge.drand_round,
    );
    let proof = fixture_proof(&challenge);

    let challenge_path = temp_dir.path().join("challenge.to");
    let proof_path = temp_dir.path().join("proof.to");
    fs::write(&challenge_path, norito::to_bytes(&challenge)?)?;
    fs::write(&proof_path, norito::to_bytes(&proof)?)?;

    let mut por = cargo_bin_cmd!("sorafs-node");
    let por_assert = por
        .arg("ingest")
        .arg("por")
        .arg(format!("--data-dir={}", storage_dir.display()))
        .arg(format!("--manifest-id={manifest_id}"))
        .arg(format!("--challenge={}", challenge_path.display()))
        .arg(format!("--proof={}", proof_path.display()))
        .assert()
        .success();

    let por_stdout = String::from_utf8(por_assert.get_output().stdout.clone())?;
    let por_json: norito::json::Value =
        norito::json::from_str(por_stdout.trim()).expect("por JSON");
    assert_eq!(
        por_json.get("status").and_then(norito::json::Value::as_str),
        Some("accepted")
    );
    let digest_hex = hex::encode(manifest_digest);
    assert_eq!(
        por_json
            .get("manifest_digest_hex")
            .and_then(norito::json::Value::as_str),
        Some(digest_hex.as_str())
    );

    Ok(())
}

fn fixture_challenge() -> PorChallengeV1 {
    let manifest_digest = [2; 32];
    let provider_id = [3; 32];
    let epoch_id = 123;
    let drand_round = 456;
    let drand_randomness = [0x41; 32];
    let vrf_output = [0x51; 32];
    let seed = derive_challenge_seed(
        &drand_randomness,
        Some(&vrf_output),
        &manifest_digest,
        epoch_id,
    );
    let challenge_id =
        derive_challenge_id(&seed, &manifest_digest, &provider_id, epoch_id, drand_round);
    PorChallengeV1 {
        version: POR_CHALLENGE_VERSION_V1,
        challenge_id,
        manifest_digest,
        provider_id,
        epoch_id,
        drand_round,
        drand_randomness,
        drand_signature: vec![0x61; 96],
        vrf_output: Some(vrf_output),
        vrf_proof: Some(vec![0x71; 80]),
        forced: false,
        chunking_profile: "sorafs.sf1@1.0.0".to_string(),
        seed,
        sample_tier: 1,
        sample_count: 2,
        sample_indices: vec![0, 64],
        issued_at: 1_700_000_000,
        deadline_at: 1_700_000_600,
    }
}

fn fixture_proof(challenge: &PorChallengeV1) -> PorProofV1 {
    PorProofV1 {
        version: POR_PROOF_VERSION_V1,
        challenge_id: challenge.challenge_id,
        manifest_digest: challenge.manifest_digest,
        provider_id: challenge.provider_id,
        samples: vec![
            PorProofSampleV1 {
                sample_index: 0,
                chunk_offset: 0,
                chunk_size: 65_536,
                chunk_digest: [5; 32],
                leaf_digest: [6; 32],
            },
            PorProofSampleV1 {
                sample_index: 64,
                chunk_offset: 4_194_304,
                chunk_size: 65_536,
                chunk_digest: [7; 32],
                leaf_digest: [8; 32],
            },
        ],
        auth_path: vec![[9; 32], [10; 32]],
        signature: AdvertSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: vec![11; 32],
            signature: vec![12; 64],
        },
        submitted_at: 1_700_000_100,
    }
}
