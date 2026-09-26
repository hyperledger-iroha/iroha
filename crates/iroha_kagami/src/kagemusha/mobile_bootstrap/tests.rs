//! Exercised operator custody, exact pinning, partial signing and threshold assembly.

use super::*;
use iroha_crypto::Algorithm;

fn fixture() -> (
    tempfile::TempDir,
    Vec<KeyPair>,
    KagemushaReleaseAuthorityPolicyV1,
    KagemushaMobileBootstrapCheckpointV1,
    CheckpointInputs,
) {
    let parent = fs::canonicalize(std::env::temp_dir()).unwrap();
    let directory = tempfile::Builder::new()
        .prefix(".mobile-bootstrap-")
        .tempdir_in(parent)
        .unwrap();
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let mut keys: Vec<_> = [31, 32, 33]
        .into_iter()
        .map(|byte| KeyPair::from_seed(vec![byte; 32], Algorithm::Ed25519))
        .collect();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let policy = KagemushaReleaseAuthorityPolicyV1 {
        version: 1,
        authority_set_id: [30; 32],
        threshold: 2,
        authorized_signers: keys.iter().map(|key| key.public_key().clone()).collect(),
    };
    let checkpoint = KagemushaMobileBootstrapCheckpointV1 {
        version: 1,
        authority_policy_digest: policy.canonical_digest().unwrap(),
        network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed(
            [3; 32],
        ))),
        scope: KagemushaMobileBootstrapScopeV1 {
            asset_identity_digest: [4; 32],
            asset_incarnation: [5; 32],
            asset_scale: 2,
            liability_pool_id: [6; 32],
        },
        release_id: [7; 32],
        release_attestation_digest: [8; 32],
        first_context_id: height_context(&"09".repeat(32)).unwrap(),
        sequence: 10,
        issued_at_ms: 1000,
        expires_at_ms: 2000,
    };
    let input = CheckpointInputs {
        checkpoint: directory.path().join("checkpoint.norito"),
        authority_policy: directory.path().join("policy.norito"),
        deployment: ExperimentalOperatorPinsV1 {
            expected_network_id: hex::encode(checkpoint.network_id.as_bytes()),
            expected_release_id: hex::encode(checkpoint.release_id),
            expected_asset_identity_digest: hex::encode(checkpoint.scope.asset_identity_digest),
            expected_asset_incarnation: hex::encode(checkpoint.scope.asset_incarnation),
            expected_asset_scale: checkpoint.scope.asset_scale,
            expected_liability_pool_id: hex::encode(checkpoint.scope.liability_pool_id),
        },
        expected_release_attestation_digest: hex::encode(checkpoint.release_attestation_digest),
        expected_first_context_id: hex::encode(checkpoint.first_context_id.0.as_ref()),
        expected_sequence: 10,
        expected_issued_at_ms: 1000,
        expected_expires_at_ms: 2000,
        trusted_now_ms: 1500,
    };
    crate::secure_fs::write_private_file_atomic(
        &input.checkpoint,
        &norito::encode_canonical(&checkpoint).unwrap(),
    )
    .unwrap();
    crate::secure_fs::write_private_file_atomic(
        &input.authority_policy,
        &norito::encode_canonical(&policy).unwrap(),
    )
    .unwrap();
    (directory, keys, policy, checkpoint, input)
}

fn sign_one(input: &CheckpointInputs, key: &KeyPair, index: usize) -> PathBuf {
    let parent = input.checkpoint.parent().unwrap();
    let signer_private_key = parent.join(format!("signer-{index}.key"));
    let bytes =
        Zeroizing::new(format!("{}\n", ExposedPrivateKey(key.private_key().clone())).into_bytes());
    crate::secure_fs::write_private_file_atomic(&signer_private_key, bytes.as_slice()).unwrap();
    let approval_output = parent.join(format!("approval-{index}.norito"));
    let args = SignArgs {
        input: input.clone(),
        signer_private_key,
        approval_output: approval_output.clone(),
    };
    let mut writer = std::io::BufWriter::new(Vec::new());
    sign(&args, &mut writer).unwrap();
    let report: JsonValue = norito::json::from_slice(&writer.into_inner().unwrap()).unwrap();
    assert_eq!(
        report["status"].as_str(),
        Some("signed_one_mobile_bootstrap_approval")
    );
    assert_eq!(report["sequence"].as_u64(), Some(10));
    approval_output
}

#[test]
fn mobile_bootstrap_sign_and_assemble_roundtrip_uses_the_native_model_verifier() {
    let (_directory, keys, policy, checkpoint, input) = fixture();
    let a = sign_one(&input, &keys[0], 0);
    let b = sign_one(&input, &keys[1], 1);
    let args = AssembleArgs {
        input: input.clone(),
        approval: vec![b, a],
        package_output: input.checkpoint.parent().unwrap().join("bootstrap.norito"),
    };
    let mut writer = std::io::BufWriter::new(Vec::new());
    assemble(&args, &mut writer).unwrap();
    let bytes = fs::read(&args.package_output).unwrap();
    let package = KagemushaMobileBootstrapPackageV1::decode_canonical_exact(&bytes).unwrap();
    assert_eq!(package.checkpoint, checkpoint);
    assert_eq!(package.approvals.len(), 2);
    let digest = package.authenticate(&input.pins(&policy).unwrap()).unwrap();
    let report: JsonValue = norito::json::from_slice(&writer.into_inner().unwrap()).unwrap();
    assert_eq!(
        report["checkpoint_digest"].as_str(),
        Some(hex::encode(digest).as_str())
    );
    assert_eq!(
        report["status"].as_str(),
        Some("assembled_mobile_bootstrap")
    );
    assert!(
        assemble(&args, &mut std::io::BufWriter::new(Vec::new())).is_err(),
        "never overwrite an earlier issued package"
    );
    assert_eq!(fs::read(&args.package_output).unwrap(), bytes);
}

#[test]
fn mobile_bootstrap_assembly_rejects_missing_duplicate_and_changed_approvals_without_output() {
    let (_directory, keys, _policy, _checkpoint, input) = fixture();
    let a = sign_one(&input, &keys[0], 0);
    let b = sign_one(&input, &keys[1], 1);
    let path = input.checkpoint.parent().unwrap().join("bootstrap.norito");
    for approvals in [vec![a.clone()], vec![a.clone(), a.clone()]] {
        let args = AssembleArgs {
            input: input.clone(),
            approval: approvals,
            package_output: path.clone(),
        };
        assert!(assemble(&args, &mut std::io::BufWriter::new(Vec::new())).is_err());
        assert!(!path.exists());
    }
    let mut approval: KagemushaMobileBootstrapApprovalV1 =
        norito::decode_canonical(&fs::read(&b).unwrap()).unwrap();
    let (mut checkpoint, _) = input.read().unwrap();
    checkpoint.sequence += 1;
    approval.signature =
        SignatureOf::try_new(keys[1].private_key(), &checkpoint.approval_payload()).unwrap();
    fs::write(&b, norito::encode_canonical(&approval).unwrap()).unwrap();
    let args = AssembleArgs {
        input,
        approval: vec![a, b],
        package_output: path.clone(),
    };
    assert!(assemble(&args, &mut std::io::BufWriter::new(Vec::new())).is_err());
    assert!(!path.exists());
}

#[test]
fn mobile_bootstrap_exact_operator_pins_reject_each_changed_subject_field() {
    let (_directory, _keys, policy, checkpoint, input) = fixture();
    input.validate(&checkpoint, &policy).unwrap();
    for index in 0..13 {
        let mut changed = input.clone();
        match index {
            0 => changed.deployment.expected_network_id = "11".repeat(32),
            1 => changed.deployment.expected_release_id = "12".repeat(32),
            2 => changed.deployment.expected_asset_identity_digest = "13".repeat(32),
            3 => changed.deployment.expected_asset_incarnation = "14".repeat(32),
            4 => changed.deployment.expected_asset_scale += 1,
            5 => changed.deployment.expected_liability_pool_id = "15".repeat(32),
            6 => changed.expected_release_attestation_digest = "16".repeat(32),
            7 => changed.expected_first_context_id = "17".repeat(32),
            8 => changed.expected_sequence -= 1,
            9 => changed.expected_issued_at_ms -= 1,
            10 => changed.expected_expires_at_ms += 1,
            11 => changed.trusted_now_ms = 999,
            _ => changed.trusted_now_ms = 2000,
        }
        assert!(
            changed.validate(&checkpoint, &policy).is_err(),
            "changed independent field {index}"
        );
        assert!(changed.read().is_err());
    }
}

#[test]
fn mobile_bootstrap_signer_rejects_foreign_key_and_noncanonical_subject_before_publication() {
    let (_directory, _keys, _policy, _checkpoint, input) = fixture();
    let foreign = KeyPair::from_seed(vec![99; 32], Algorithm::Ed25519);
    let parent = input.checkpoint.parent().unwrap();
    let key_path = parent.join("foreign.key");
    let key_bytes = Zeroizing::new(
        format!("{}\n", ExposedPrivateKey(foreign.private_key().clone())).into_bytes(),
    );
    crate::secure_fs::write_private_file_atomic(&key_path, key_bytes.as_slice()).unwrap();
    let args = SignArgs {
        input: input.clone(),
        signer_private_key: key_path,
        approval_output: parent.join("refused.norito"),
    };
    assert!(sign(&args, &mut std::io::BufWriter::new(Vec::new())).is_err());
    assert!(!args.approval_output.exists());
    let mut bytes = fs::read(&input.checkpoint).unwrap();
    bytes.push(0);
    fs::write(&input.checkpoint, bytes).unwrap();
    assert!(input.read().is_err());
}

#[test]
fn mobile_bootstrap_context_parser_never_normalizes_a_supplied_hash() {
    assert!(height_context(&"09".repeat(32)).is_ok());
    for value in [
        "08".repeat(32),
        "00".repeat(32),
        "AB".repeat(32),
        "ab".repeat(31),
        hex::encode(Hash::prehashed([0; 32]).as_ref()),
    ] {
        assert!(height_context(&value).is_err(), "rejected {value}");
    }
}

#[test]
fn mobile_bootstrap_preparation_requires_authenticated_release_before_creating_subject() {
    let (_directory, _keys, _policy, _checkpoint, input) = fixture();
    let parent = input.checkpoint.parent().unwrap();
    let args = PrepareArgs {
        release: AuthenticateExperimentalReleaseV1Args {
            manifest: input.checkpoint.clone(),
            validation_receipt: parent.join("receipt.norito"),
            authority_policy: input.authority_policy.clone(),
            attestation: parent.join("attestation.norito"),
            artifact_root: parent.to_path_buf(),
            pins: input.deployment.clone(),
        },
        first_context_id: input.expected_first_context_id,
        sequence: input.expected_sequence,
        issued_at_ms: input.expected_issued_at_ms,
        expires_at_ms: input.expected_expires_at_ms,
        trusted_now_ms: input.trusted_now_ms,
        checkpoint_output: parent.join("never.norito"),
    };
    assert!(prepare(&args, &mut std::io::BufWriter::new(Vec::new())).is_err());
    assert!(!args.checkpoint_output.exists());
}

#[test]
fn mobile_bootstrap_preparation_authenticates_threshold_release_and_all_fifty_files() {
    use iroha_data_model::testing::kagemusha_release::KagemushaExperimentalReleaseFixtureV1;

    let (directory, _keys, _policy, seed, _input) = fixture();
    let artifact_root = directory.path().join("artifacts");
    fs::create_dir(&artifact_root).unwrap();
    let bindings = crate::kagemusha::tests::write_experimental_artifact_fixture(&artifact_root);
    assert_eq!(bindings.len(), 50);
    let scope = KagemushaTestnetExperimentScopeV1 {
        asset_identity_digest: seed.scope.asset_identity_digest,
        asset_incarnation: seed.scope.asset_incarnation,
        asset_scale: seed.scope.asset_scale,
        liability_pool_id: seed.scope.liability_pool_id,
    };
    let release =
        KagemushaExperimentalReleaseFixtureV1::new(bindings.clone(), seed.network_id, scope);
    let authenticated = release
        .manifest
        .authenticate_experimental(
            &release.receipt,
            &release.authority_policy,
            &release.attestation,
        )
        .unwrap();
    let manifest = directory.path().join("release-manifest.norito");
    let validation_receipt = directory.path().join("release-receipt.norito");
    let authority_policy = directory.path().join("release-policy.norito");
    let attestation = directory.path().join("release-attestation.norito");
    for (path, bytes) in [
        (
            &manifest,
            norito::encode_canonical(&release.manifest).unwrap(),
        ),
        (
            &validation_receipt,
            norito::encode_canonical(&release.receipt).unwrap(),
        ),
        (
            &authority_policy,
            norito::encode_canonical(&release.authority_policy).unwrap(),
        ),
        (
            &attestation,
            norito::encode_canonical(&release.attestation).unwrap(),
        ),
    ] {
        crate::secure_fs::write_private_file_atomic(path, &bytes).unwrap();
    }
    let mut args = PrepareArgs {
        release: AuthenticateExperimentalReleaseV1Args {
            manifest,
            validation_receipt,
            authority_policy,
            attestation,
            artifact_root: artifact_root.clone(),
            pins: ExperimentalOperatorPinsV1 {
                expected_network_id: hex::encode(seed.network_id.as_bytes()),
                expected_release_id: hex::encode(authenticated.release_id()),
                expected_asset_identity_digest: hex::encode(scope.asset_identity_digest),
                expected_asset_incarnation: hex::encode(scope.asset_incarnation),
                expected_asset_scale: scope.asset_scale,
                expected_liability_pool_id: hex::encode(scope.liability_pool_id),
            },
        },
        first_context_id: hex::encode(seed.first_context_id.0.as_ref()),
        sequence: seed.sequence,
        issued_at_ms: seed.issued_at_ms,
        expires_at_ms: seed.expires_at_ms,
        trusted_now_ms: 1500,
        checkpoint_output: directory.path().join("prepared-checkpoint.norito"),
    };
    let mut writer = std::io::BufWriter::new(Vec::new());
    prepare(&args, &mut writer).unwrap();
    let bytes = fs::read(&args.checkpoint_output).unwrap();
    let checkpoint: KagemushaMobileBootstrapCheckpointV1 =
        norito::decode_canonical(&bytes).unwrap();
    assert_eq!(checkpoint.network_id, seed.network_id);
    assert_eq!(checkpoint.scope, seed.scope);
    assert_eq!(checkpoint.release_id, authenticated.release_id());
    assert_eq!(
        checkpoint.release_attestation_digest,
        authenticated.attestation_digest()
    );
    assert_eq!(
        checkpoint.authority_policy_digest,
        authenticated.authority_policy_digest()
    );
    assert_eq!(checkpoint.first_context_id, seed.first_context_id);
    assert_eq!(checkpoint.sequence, seed.sequence);
    assert_eq!(checkpoint.issued_at_ms, seed.issued_at_ms);
    assert_eq!(checkpoint.expires_at_ms, seed.expires_at_ms);
    let report: JsonValue = norito::json::from_slice(&writer.into_inner().unwrap()).unwrap();
    assert_eq!(
        report["status"].as_str(),
        Some("prepared_unsigned_mobile_checkpoint")
    );

    // A valid release signature does not excuse different bytes in any content-addressed file.
    let resolver = KagemushaDirectoryArtifactResolverV1::new(&artifact_root).unwrap();
    fs::write(resolver.path_for_digest(bindings[49].sha256), [0xff]).unwrap();
    args.checkpoint_output = directory.path().join("refused-checkpoint.norito");
    assert!(prepare(&args, &mut std::io::BufWriter::new(Vec::new())).is_err());
    assert!(!args.checkpoint_output.exists());
    assert_eq!(
        fs::read(directory.path().join("prepared-checkpoint.norito")).unwrap(),
        bytes
    );
}

#[test]
fn mobile_bootstrap_commands_require_all_independent_pins_and_one_signer() {
    use clap::Parser as _;
    let pins = format!(
        "--checkpoint checkpoint.norito --authority-policy policy.norito --expected-network-id {} --expected-release-id {} --expected-asset-identity-digest {} --expected-asset-incarnation {} --expected-asset-scale 2 --expected-liability-pool-id {} --expected-release-attestation-digest {} --expected-first-context-id {} --expected-sequence 10 --expected-issued-at-ms 1000 --expected-expires-at-ms 2000 --trusted-now-ms 1500",
        "03".repeat(32),
        "07".repeat(32),
        "04".repeat(32),
        "05".repeat(32),
        "06".repeat(32),
        "08".repeat(32),
        "09".repeat(32)
    );
    for command in [
        format!(
            "kagami kagemusha sign-mobile-bootstrap-approval-v1 {pins} --signer-private-key signer.key --approval-output approval.norito"
        ),
        format!(
            "kagami kagemusha assemble-mobile-bootstrap-v1 {pins} --approval a.norito --approval b.norito --package-output bootstrap.norito"
        ),
    ] {
        assert!(crate::Cli::try_parse_from(command.split_whitespace()).is_ok());
        for omitted in [
            "--trusted-now-ms 1500",
            "--expected-sequence 10",
            "--expected-expires-at-ms 2000",
        ] {
            assert!(
                crate::Cli::try_parse_from(command.replace(omitted, "").split_whitespace())
                    .is_err()
            );
        }
    }
}
