//! End-to-end checks for the sorafs-node CLI helpers.
use assert_cmd::cargo::cargo_bin_cmd;
use ed25519_dalek::{Signer as _, SigningKey};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    account::{AccountId, address::AccountAddress},
    peer::PeerId,
};
use sorafs_car::{CarBuildPlan, CarWriter, compute_chunk_plan_digest_sha3, compute_por_root};
use sorafs_manifest::{
    BLAKE3_256_MULTIHASH_CODE, CouncilSignature, DagCodecId, GovernanceProofs, ManifestBuilder,
    PinPolicy, StorageClass,
    operator_preseed::{OPERATOR_PRESEED_SESSION_RELEASE_ACK_V1, OperatorPreseedSessionReceiptV1},
    por::{
        POR_CHALLENGE_VERSION_V1, POR_PROOF_VERSION_V1, PorChallengeV1, PorProofSampleV1,
        PorProofV1, derive_challenge_id, derive_challenge_seed,
    },
    provider_advert::{AdvertSignature, SignatureAlgorithm},
};
use std::{
    fs, io,
    io::{BufRead as _, BufReader, Read as _},
    path::Path,
    process::Stdio,
};
use tempfile::TempDir;
fn ingest_tests_enabled() -> bool {
    std::env::var("SORAFS_NODE_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")
}
fn build_manifest(
    payload: &[u8],
) -> Result<(CarBuildPlan, sorafs_manifest::ManifestV1), Box<dyn std::error::Error>> {
    let plan = CarBuildPlan::single_file(payload)?;
    let stats = CarWriter::new(&plan, payload)?.write_to(io::sink())?;
    let mut car_digest = [0u8; 32];
    car_digest.copy_from_slice(stats.car_archive_digest.as_bytes());
    let mut manifest = ManifestBuilder::new()
        .root_cid(stats.root_cids[0].clone())
        .dag_codec(DagCodecId(stats.dag_codec))
        .chunking_from_profile(plan.chunk_profile, BLAKE3_256_MULTIHASH_CODE)
        .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(compute_por_root(payload, &plan)?)
        .content_length(plan.content_length)
        .car_digest(car_digest)
        .car_size(stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 1,
        })
        .build()?;
    let signing_key = SigningKey::from_bytes(&[0x22; 32]);
    let digest = manifest.digest()?;
    manifest.governance = GovernanceProofs {
        council_signatures: vec![CouncilSignature {
            signer: signing_key.verifying_key().to_bytes(),
            signature: signing_key.sign(digest.as_bytes()).to_bytes().to_vec(),
        }],
    };
    Ok((plan, manifest))
}
#[test]
fn sorafs_node_cli_help_documents_only_canonical_ingest_spellings()
-> Result<(), Box<dyn std::error::Error>> {
    let mut command = cargo_bin_cmd!("sorafs-node");
    let assertion = command.arg("--help").assert().success();
    let stderr = String::from_utf8(assertion.get_output().stderr.clone())?;
    assert!(stderr.contains(
        "ingest --data-dir=<dir> --max-capacity-bytes=<bytes> --manifest=<path> (--payload=<path>|--payload-dir=<dir>) [--plan-json-out=<path>]"
    ));
    assert!(stderr.contains(
        "ingest por --data-dir=<dir> --challenge=<path> --proof=<path> [--verdict=<path>] [--manifest-id=<hex>] [--json-out=<path>]"
    ));
    assert!(stderr.contains(
        "preseed-session --target=<validator-account-id>,<peer-id>,<data-dir>... --max-capacity-bytes=<bytes> [--verify-only] (--manifest=<path> (--payload=<path>|--payload-dir=<dir>))..."
    ));
    assert!(!stderr.contains("ingest [manifest]"));
    Ok(())
}
fn preseed_target_arg(seed: u8, data_dir: &Path) -> String {
    let validator_key_pair =
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("fixture key pair");
    let peer_key_pair = KeyPair::try_from_seed(vec![seed ^ 0x80; 32], Algorithm::BlsNormal)
        .expect("fixture peer key pair");
    let validator_account_id = AccountId::new(validator_key_pair.public_key().clone());
    let validator_account_literal = AccountAddress::from_account_id(&validator_account_id)
        .expect("fixture account address")
        .to_i105_for_discriminant(369)
        .expect("canonical Taira fixture account literal");
    let peer_id = PeerId::from(peer_key_pair.public_key().clone());
    format!(
        "--target={validator_account_literal},{peer_id},{}",
        data_dir.display()
    )
}

fn completed_preseed_receipt(
    output: &[u8],
) -> Result<OperatorPreseedSessionReceiptV1, Box<dyn std::error::Error>> {
    let ready_end = output
        .iter()
        .position(|byte| *byte == b'\n')
        .ok_or("preseed output is missing its ready-receipt newline")?;
    let (ready_with_newline, release) = output.split_at(ready_end + 1);
    if release != OPERATOR_PRESEED_SESSION_RELEASE_ACK_V1 {
        return Err("preseed output is missing its exact EOF release acknowledgment".into());
    }
    let ready = ready_with_newline
        .strip_suffix(b"\n")
        .ok_or("preseed ready receipt is missing its newline")?;
    if ready.is_empty() || ready.contains(&b'\r') || ready.contains(&b'\n') {
        return Err("preseed ready receipt is not one canonical line".into());
    }
    Ok(norito::json::from_slice(ready)?)
}
#[test]
fn sorafs_node_cli_rejects_manifest_subcommand_alias() -> Result<(), Box<dyn std::error::Error>> {
    let mut command = cargo_bin_cmd!("sorafs-node");
    let assertion = command.arg("ingest").arg("manifest").assert().failure();
    let stderr = String::from_utf8(assertion.get_output().stderr.clone())?;
    assert_eq!(stderr, "error: unknown option: manifest\n");
    Ok(())
}
#[test]
fn sorafs_node_cli_ingest_requires_canonical_explicit_capacity()
-> Result<(), Box<dyn std::error::Error>> {
    for (capacity, expected) in [
        (None, "missing required option --max-capacity-bytes"),
        (
            Some("0"),
            "--max-capacity-bytes must be a nonzero canonical unsigned decimal",
        ),
        (
            Some("01"),
            "--max-capacity-bytes must be a nonzero canonical unsigned decimal",
        ),
        (
            Some("18446744073709551616"),
            "--max-capacity-bytes must fit in an unsigned 64-bit integer",
        ),
    ] {
        let mut command = cargo_bin_cmd!("sorafs-node");
        command
            .arg("ingest")
            .arg("--data-dir=storage")
            .arg("--manifest=manifest.to")
            .arg("--payload=payload.bin");
        if let Some(capacity) = capacity {
            command.arg(format!("--max-capacity-bytes={capacity}"));
        }
        let assertion = command.assert().failure();
        let stderr = String::from_utf8(assertion.get_output().stderr.clone())?;
        assert_eq!(stderr, format!("error: {expected}\n"));
    }
    Ok(())
}
#[test]
fn sorafs_node_cli_rejects_file_payload_over_explicit_capacity_before_storage_open()
-> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().canonicalize()?;
    let storage_dir = temp_path.join("storage");
    let payload = b"payload larger than its explicit storage ceiling";
    let (_plan, manifest) = build_manifest(payload)?;
    let manifest_path = temp_path.join("manifest.to");
    fs::write(&manifest_path, norito::to_bytes(&manifest)?)?;
    let payload_path = temp_path.join("payload.bin");
    fs::write(&payload_path, payload)?;
    let capacity = payload.len() as u64 - 1;

    let mut command = cargo_bin_cmd!("sorafs-node");
    let assertion = command
        .arg("ingest")
        .arg(format!("--data-dir={}", storage_dir.display()))
        .arg(format!("--max-capacity-bytes={capacity}"))
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--payload={}", payload_path.display()))
        .assert()
        .failure();
    let stderr = String::from_utf8(assertion.get_output().stderr.clone())?;
    assert_eq!(
        stderr,
        format!(
            "error: manifest payload length {} exceeds --max-capacity-bytes={capacity}\n",
            payload.len()
        )
    );
    assert!(!storage_dir.exists());
    Ok(())
}
#[test]
fn sorafs_node_cli_rejects_por_manifest_option_alias() -> Result<(), Box<dyn std::error::Error>> {
    let mut command = cargo_bin_cmd!("sorafs-node");
    let assertion = command
        .arg("ingest")
        .arg("por")
        .arg("--manifest=00")
        .assert()
        .failure();
    let stderr = String::from_utf8(assertion.get_output().stderr.clone())?;
    assert_eq!(stderr, "error: unknown option: --manifest=00\n");
    Ok(())
}
#[test]
fn sorafs_node_cli_ingest_and_export_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    if !ingest_tests_enabled() {
        eprintln!("skipping ingest roundtrip (SORAFS_NODE_SKIP_INGEST_TESTS=1)");
        return Ok(());
    }
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().canonicalize()?;
    let storage_dir = temp_path.join("storage");
    let payload = b"sorafs-node CLI integration payload";
    let (_plan, manifest) = build_manifest(payload)?;
    let manifest_bytes = norito::to_bytes(&manifest)?;
    let manifest_path = temp_path.join("manifest.to");
    fs::write(&manifest_path, &manifest_bytes)?;
    let payload_path = temp_path.join("payload.bin");
    fs::write(&payload_path, payload)?;
    let ingest_plan_path = temp_path.join("ingest_plan.json");
    let mut ingest = cargo_bin_cmd!("sorafs-node");
    let ingest_assert = ingest
        .arg("ingest")
        .arg(format!("--data-dir={}", storage_dir.display()))
        .arg("--max-capacity-bytes=1048576")
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
    let export_manifest_path = temp_path.join("export_manifest.to");
    let export_payload_path = temp_path.join("export_payload.bin");
    let export_plan_path = temp_path.join("export_plan.json");
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
fn preseed_session_holds_all_locks_and_exact_rerun_is_idempotent()
-> Result<(), Box<dyn std::error::Error>> {
    if !ingest_tests_enabled() {
        eprintln!("skipping preseed session (SORAFS_NODE_SKIP_INGEST_TESTS=1)");
        return Ok(());
    }
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().canonicalize()?;
    let store_a = temp_path.join("store-a");
    let store_b = temp_path.join("store-b");
    fs::create_dir(&store_a)?;
    fs::create_dir(&store_b)?;
    let payload = b"exact first-release Inrou preseed payload";
    let (_plan, manifest) = build_manifest(payload)?;
    let manifest_path = temp_path.join("manifest.to");
    fs::write(&manifest_path, norito::to_bytes(&manifest)?)?;
    let payload_path = temp_path.join("payload.bin");
    fs::write(&payload_path, payload)?;
    let second_payload = b"second exact guest/discovery preseed payload";
    let (_second_plan, second_manifest) = build_manifest(second_payload)?;
    let second_manifest_path = temp_path.join("second-manifest.to");
    fs::write(&second_manifest_path, norito::to_bytes(&second_manifest)?)?;
    let second_payload_path = temp_path.join("second-payload.bin");
    fs::write(&second_payload_path, second_payload)?;

    let session_args = [
        "preseed-session".to_owned(),
        preseed_target_arg(0x41, &store_a),
        preseed_target_arg(0x42, &store_b),
        "--max-capacity-bytes=1048576".to_owned(),
        format!("--manifest={}", manifest_path.display()),
        format!("--payload={}", payload_path.display()),
        format!("--manifest={}", second_manifest_path.display()),
        format!("--payload={}", second_payload_path.display()),
    ];
    let mut session = std::process::Command::new(assert_cmd::cargo::cargo_bin!("sorafs-node"));
    let mut child = session
        .args(&session_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let stdin = child.stdin.take().ok_or("missing session stdin")?;
    let mut stdout = BufReader::new(child.stdout.take().ok_or("missing session stdout")?);
    let mut receipt_line = String::new();
    stdout.read_line(&mut receipt_line)?;
    let receipt: OperatorPreseedSessionReceiptV1 = norito::json::from_str(receipt_line.trim_end())?;
    receipt.validate()?;
    assert_eq!(receipt.status, "ready");
    assert_eq!(receipt.mode, "ingest");
    assert_eq!(receipt.targets.len(), 2);
    let mut expected_validator_literals = session_args
        .iter()
        .filter_map(|arg| arg.strip_prefix("--target="))
        .filter_map(|target| target.split(',').next())
        .collect::<Vec<_>>();
    expected_validator_literals.sort_unstable();
    assert_eq!(
        receipt
            .targets
            .iter()
            .map(|target| target.validator_account_id.as_str())
            .collect::<Vec<_>>(),
        expected_validator_literals,
        "standalone preseed must preserve every exact embedded Taira discriminant in canonical order"
    );
    assert_eq!(receipt.artifacts.len(), 2);
    assert!(
        receipt
            .artifacts
            .iter()
            .all(|artifact| artifact.store_count == 2)
    );
    assert!(
        child.try_wait()?.is_none(),
        "ready receipt must arrive while every store lock remains held"
    );
    let canonical_receipt_bytes = norito::json::to_vec(&receipt)?;
    for store in [&store_a, &store_b] {
        let retained = sorafs_node::operator_preseed::read_operator_preseed_store_receipts(store)?;
        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0].0, receipt);
        assert_eq!(retained[0].1, canonical_receipt_bytes);
    }

    let mut contending = cargo_bin_cmd!("sorafs-node");
    let assertion = contending.args(&session_args).assert().failure();
    let stderr = String::from_utf8(assertion.get_output().stderr.clone())?;
    assert!(stderr.contains("already in use"), "{stderr}");

    drop(stdin);
    let mut release_ack = Vec::new();
    stdout.read_to_end(&mut release_ack)?;
    assert_eq!(release_ack, OPERATOR_PRESEED_SESSION_RELEASE_ACK_V1);
    assert!(child.wait()?.success());
    let mut rerun = cargo_bin_cmd!("sorafs-node");
    let rerun_assertion = rerun.args(&session_args).assert().success();
    let rerun_receipt = completed_preseed_receipt(&rerun_assertion.get_output().stdout)?;
    assert_eq!(rerun_receipt, receipt);

    let reversed_args = [
        "preseed-session".to_owned(),
        preseed_target_arg(0x42, &store_b),
        preseed_target_arg(0x41, &store_a),
        "--max-capacity-bytes=1048576".to_owned(),
        format!("--manifest={}", second_manifest_path.display()),
        format!("--payload={}", second_payload_path.display()),
        format!("--manifest={}", manifest_path.display()),
        format!("--payload={}", payload_path.display()),
    ];
    let mut reversed = cargo_bin_cmd!("sorafs-node");
    let reversed_assertion = reversed.args(&reversed_args).assert().success();
    let reversed_receipt = completed_preseed_receipt(&reversed_assertion.get_output().stdout)?;
    assert_eq!(
        reversed_receipt, receipt,
        "target and artifact permutations must resolve to one canonical qualification"
    );

    let mut duplicate = cargo_bin_cmd!("sorafs-node");
    let duplicate_assertion = duplicate
        .arg("preseed-session")
        .arg(preseed_target_arg(0x41, &store_a))
        .arg("--max-capacity-bytes=1048576")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--payload={}", payload_path.display()))
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--payload={}", payload_path.display()))
        .assert()
        .failure();
    let duplicate_stderr = String::from_utf8(duplicate_assertion.get_output().stderr.clone())?;
    assert!(
        duplicate_stderr.contains("artifact manifest digests must be distinct"),
        "{duplicate_stderr}"
    );

    let mut duplicate_identity = cargo_bin_cmd!("sorafs-node");
    let duplicate_identity_assertion = duplicate_identity
        .arg("preseed-session")
        .arg(preseed_target_arg(0x41, &store_a))
        .arg(preseed_target_arg(0x41, &store_b))
        .arg("--max-capacity-bytes=1048576")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--payload={}", payload_path.display()))
        .assert()
        .failure();
    let duplicate_identity_stderr =
        String::from_utf8(duplicate_identity_assertion.get_output().stderr.clone())?;
    assert!(
        duplicate_identity_stderr.contains("identities must each be distinct"),
        "{duplicate_identity_stderr}"
    );

    let nested_store = store_a.join("nested-store");
    fs::create_dir(&nested_store)?;
    let mut overlapping_roots = cargo_bin_cmd!("sorafs-node");
    let overlapping_roots_assertion = overlapping_roots
        .arg("preseed-session")
        .arg(preseed_target_arg(0x41, &store_a))
        .arg(preseed_target_arg(0x42, &nested_store))
        .arg("--max-capacity-bytes=1048576")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--payload={}", payload_path.display()))
        .assert()
        .failure();
    let overlapping_roots_stderr =
        String::from_utf8(overlapping_roots_assertion.get_output().stderr.clone())?;
    assert!(
        overlapping_roots_stderr.contains("roots must not overlap"),
        "{overlapping_roots_stderr}"
    );

    let mut verify_args = session_args.to_vec();
    verify_args.push("--verify-only".to_owned());
    let mut verify = cargo_bin_cmd!("sorafs-node");
    let verify_assertion = verify.args(&verify_args).assert().success();
    let verify_receipt = completed_preseed_receipt(&verify_assertion.get_output().stdout)?;
    let mut expected_verify_receipt = receipt.clone();
    expected_verify_receipt.mode = "verify_only".to_owned();
    assert_eq!(verify_receipt, expected_verify_receipt);
    for store in [&store_a, &store_b] {
        let retained = sorafs_node::operator_preseed::read_operator_preseed_store_receipts(store)?;
        assert_eq!(
            retained.len(),
            1,
            "verify-only replay must not replace or duplicate the durable ingest qualification"
        );
        assert_eq!(retained[0].0, receipt);
    }

    let empty_store = temp_path.join("empty-store");
    fs::create_dir(&empty_store)?;
    let mut missing = cargo_bin_cmd!("sorafs-node");
    let missing_assertion = missing
        .arg("preseed-session")
        .arg(preseed_target_arg(0x43, &empty_store))
        .arg("--max-capacity-bytes=1048576")
        .arg("--verify-only")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--payload={}", payload_path.display()))
        .assert()
        .failure();
    let missing_stderr = String::from_utf8(missing_assertion.get_output().stderr.clone())?;
    assert!(
        missing_stderr.contains("verify-only preseed store")
            && missing_stderr.contains("has no exact durable ingest qualification"),
        "{missing_stderr}"
    );
    assert_eq!(
        fs::read_dir(empty_store.join("manifests"))?.count(),
        0,
        "verify-only replay must never repair a missing artifact"
    );
    Ok(())
}
#[test]
fn sorafs_node_cli_ingest_por_flow() -> Result<(), Box<dyn std::error::Error>> {
    if !ingest_tests_enabled() {
        eprintln!("skipping PoR ingest flow (SORAFS_NODE_SKIP_INGEST_TESTS=1)");
        return Ok(());
    }
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().canonicalize()?;
    let storage_dir = temp_path.join("storage");
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
        Some("e725de3d9e31f4d5150cb9f26122f7e4ca1c21b177c1c27a3e7047ae7832a9da")
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
    let temp_path = temp_dir.path().canonicalize()?;
    let storage_dir = temp_path.join("storage");
    let payload = b"sorafs-node PoR replay payload";
    let (_plan, manifest) = build_manifest(payload)?;
    let manifest_bytes = norito::to_bytes(&manifest)?;
    let manifest_path = temp_path.join("manifest_por.to");
    fs::write(&manifest_path, &manifest_bytes)?;
    let payload_path = temp_path.join("payload_por.bin");
    fs::write(&payload_path, payload)?;
    let mut ingest = cargo_bin_cmd!("sorafs-node");
    let ingest_assert = ingest
        .arg("ingest")
        .arg(format!("--data-dir={}", storage_dir.display()))
        .arg("--max-capacity-bytes=1048576")
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
    let challenge_path = temp_path.join("challenge.to");
    let proof_path = temp_path.join("proof.to");
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
        drand_signature: [0x61; 48],
        vrf_output: Some(vrf_output),
        vrf_proof: Some(iroha_crypto::vrf::VrfProof::SigInG1([0x71; 48])),
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
    let mut proof = PorProofV1 {
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
            public_key: Vec::new(),
            signature: Vec::new(),
        },
        submitted_at: 1_700_000_100,
    };
    let signing_key = SigningKey::from_bytes(&[0x11; 32]);
    proof.signature.public_key = signing_key.verifying_key().to_bytes().to_vec();
    let payload = proof
        .signature_payload_bytes()
        .expect("encode deterministic PoR proof signing payload");
    proof.signature.signature = signing_key.sign(&payload).to_bytes().to_vec();
    proof
}
