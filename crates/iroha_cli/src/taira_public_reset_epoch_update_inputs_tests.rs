//! Native source controls; execution belongs to the owner's Cargo qualification lane.
use super::*;
use norito::codec::Encode as _;

fn fixture() -> (
    PrepareEpochUpdate,
    DeploymentV1,
    PreparedV1,
    DeploymentTrustV1,
) {
    let (block, genesis) = crate::taira_public_reset::deployment_genesis_fixture();
    let network = NetworkId::from_genesis_hash(block.hash());
    let trust = DeploymentTrustV1 {
        genesis_public_key: genesis.public_key().clone(),
        genesis_signed_wire_hex: hex::encode(block.encode_wire().unwrap()),
        peers: iroha_genesis::signed_genesis_validator_pops(&block)
            .unwrap()
            .into_keys()
            .enumerate()
            .map(|(index, key)| {
                let peer_id = PeerId::new(key);
                crate::taira_dataspace_deploy::DeploymentPeerV1 {
                    torii_origin: format!("https://validator-{index}.example/"),
                    node_fingerprint: Hash::new(peer_id.encode()),
                    peer_id,
                    build_fingerprint: Hash::new([1]),
                    config_fingerprint: Hash::new([2]),
                }
            })
            .collect(),
    };
    let args = PrepareEpochUpdate {
        deployment: "/public/deployment.json".into(),
        prepared_result: "/public/result.json".into(),
        operation: format!("update-{}", "1".repeat(32)),
        original_service_state: "absent".into(),
        successor_service_state: "running".into(),
        before_binding: None,
        installed_state: "absent".into(),
        installed_binding: None,
        trust: "/public/trust.json".into(),
        authorization: "until-stopped".into(),
        administrator: AccountId::new(genesis.public_key().clone()).to_string(),
        payment_asset: crate::taira::DEFAULT_GAS_ASSET_ID.into(),
        transaction_fee_maximum: "100".into(),
        first_epoch: 1,
        batch_epochs: 2,
        operation_timeout_ms: 180_000,
        provision_timeout_ms: 180_000,
        worker_timeout_ms: 6_000_000,
        original_seed_sources: (0..4)
            .map(|i| PathBuf::from(format!("/unopened-originals/peer{i}.seed")))
            .collect(),
        output: "/public/update-inputs".into(),
    };
    let deployment = DeploymentV1 {
        schema: "taira.runtime-deployment.v1".into(),
        guest_ssh: SshV1 {
            argv: vec!["ssh".into()],
            pins: vec![ReferenceV1 {
                path: "/public/known_hosts".into(),
                sha256: "1".repeat(64),
            }],
        },
        runtime_root: "/srv/taira".into(),
        state_root: "/var/lib/taira".into(),
        config_root: "/etc/taira".into(),
        config_release: "a".repeat(40),
        genesis_manifest: "/srv/taira/genesis.json".into(),
        network_id: network,
        public_origin: "https://taira.example".into(),
        roles: (1..=4).map(|i| format!("taira-validator-{i}")).collect(),
        ports: vec![18081, 18082, 18083, 18084],
        replay_floor: 1,
        renderer_sha256: "1".repeat(64),
        current: CurrentV1 {
            commit: "a".repeat(40),
            daemon: "/srv/taira/old/bin/iroha3d_taira".into(),
            attempt_name: "old".into(),
            plan_schema: "taira.daemon-update.plan.v1".into(),
            result_schema: "taira.daemon-update.result.v1".into(),
            local_plan: "/public/old-plan.json".into(),
            local_plan_sha256: "1".repeat(64),
        },
    };
    let prepared = PreparedV1 {
        commit: "b".repeat(40),
        signer_fingerprint: "A".repeat(40),
        native_check_scope: "basic".into(),
        native_incremental: true,
        environment_sha256: "1".repeat(64),
        native_environment_sha256: "2".repeat(64),
        tree: "c".repeat(40),
        target: "aarch64-unknown-linux-gnu".into(),
        profile: "release".into(),
        jobs: 6,
        source_unchanged: true,
        toolchain_unchanged: true,
        source_snapshot_sha256: "3".repeat(64),
        source_root: "/public/source".into(),
        source_output_target: "/public/target".into(),
        compiler_tools: vec![],
        tools: vec![],
        command: vec![],
        release_qualified: false,
        deployed: false,
        timings_seconds: json::Value::Null,
        attempt: "attempts/01".into(),
        artifacts: [
            ("iroha3d_taira", "irohad"),
            ("iroha", "iroha_cli"),
            ("sorafs-node", "sorafs_node"),
            ("kagami", "iroha_kagami"),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (name, package))| ArtifactV1 {
            name: name.into(),
            package: package.into(),
            path: format!("/public/artifacts/{name}"),
            sha256: (i + 1).to_string().repeat(64),
            size: 2_000_000,
        })
        .collect(),
    };
    (args, deployment, prepared, trust)
}

#[test]
fn epoch_update_inputs_first_install_derives_exact_native_closure_without_private_files() {
    let _guard = ChainDiscriminantGuard::enter(super::super::super::CHAIN_DISCRIMINANT);
    let (args, deployment, prepared, trust) = fixture();
    let raw = json::to_vec(&trust).unwrap();
    let output = args
        .build(&deployment, &prepared, &trust, &raw, None, None)
        .unwrap();
    assert_eq!(output.original_service_state, "absent");
    assert_eq!(output.successor_service_state, "running");
    assert!(output.before.is_none() && output.installed.is_none());
    assert_eq!(
        output.after.unit_spec.cli,
        format!(
            "/srv/taira/release-{}-{}/bin/iroha",
            prepared.commit, args.operation
        )
    );
    assert_eq!(output.after.iroha_sha256, prepared.artifacts[1].sha256);
    assert_eq!(output.after.kagami_sha256, prepared.artifacts[3].sha256);
    assert_eq!(output.after.observation_trust_bytes.as_bytes(), raw);
    assert!(
        output
            .after
            .unit_spec
            .policy
            .contains(&output.after.policy_sha256)
    );
    let plan = epoch_generation::as_public_plan(&output.after, "absent").unwrap();
    assert_eq!(
        epoch_supervisor::render_unit(&plan, &output.after.unit_spec.cli).unwrap(),
        plan.unit_bytes
    );
    assert!(epoch_supervisor::validate_generation(&plan).is_err());
    assert!(plan.admin_config_sha256.is_empty() && plan.http_operator_key_sha256.is_empty());
    assert_eq!(output.original_seed_sources.len(), 4);
    assert!(
        output
            .original_seed_sources
            .windows(2)
            .all(|p| p[0].validator < p[1].validator)
    );
    let decoded: PreparationV1 = json::from_slice(&json::to_vec(&output).unwrap()).unwrap();
    assert_eq!(decoded.after.policy_sha256, output.after.policy_sha256);
}

#[test]
fn epoch_update_inputs_rejects_incomplete_or_changed_build_and_operation() {
    let _guard = ChainDiscriminantGuard::enter(super::super::super::CHAIN_DISCRIMINANT);
    let (_, deployment, prepared, _) = fixture();
    validate_prepared(&prepared, &deployment.current.commit).unwrap();
    for kind in 0..7 {
        let mut bad = prepared.clone();
        match kind {
            0 => bad.jobs = 1,
            1 => bad.artifacts[3].package = "iroha_cli".into(),
            2 => bad.artifacts[1].sha256 = "0".repeat(64),
            3 => {
                bad.artifacts.pop();
            }
            4 => bad.source_unchanged = false,
            5 => bad.commit = deployment.current.commit.clone(),
            _ => bad.artifacts.swap(2, 3),
        }
        assert!(validate_prepared(&bad, &deployment.current.commit).is_err());
    }
    for bad in [
        "update",
        "update-../escape",
        "update-0000",
        "update-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    ] {
        assert!(update_operation(bad, "old").is_err());
    }
    let same = format!("update-{}", "1".repeat(32));
    assert!(update_operation(&same, &same).is_err());
}

#[test]
fn epoch_update_inputs_preserves_original_intent_separately_from_installed_state() {
    let _guard = ChainDiscriminantGuard::enter(super::super::super::CHAIN_DISCRIMINANT);
    let (mut args, mut deployment, mut prepared, trust) = fixture();
    let raw = json::to_vec(&trust).unwrap();
    let first = args
        .build(&deployment, &prepared, &trust, &raw, None, None)
        .unwrap()
        .after;
    deployment.current.commit = first.release_source_commit.clone();
    deployment.current.daemon = Path::new(&first.unit_spec.cli)
        .with_file_name("iroha3d_taira")
        .to_str()
        .unwrap()
        .into();
    prepared.commit = "c".repeat(40);
    for state in ["running", "stopped"] {
        args.original_service_state = state.into();
        args.successor_service_state = state.into();
        // Recovery may observe absence while immutable original intent remains present.
        let result = args
            .build(
                &deployment,
                &prepared,
                &trust,
                &raw,
                Some(first.clone()),
                None,
            )
            .unwrap();
        assert_eq!(result.original_service_state, state);
        assert_eq!(result.successor_service_state, state);
        assert!(result.before.is_some() && result.installed.is_none());
        args.successor_service_state = if state == "running" {
            "stopped"
        } else {
            "running"
        }
        .into();
        assert!(
            args.build(
                &deployment,
                &prepared,
                &trust,
                &raw,
                Some(first.clone()),
                None
            )
            .is_err()
        );
    }
    args.successor_service_state = args.original_service_state.clone();
    args.installed_state = "present".into();
    assert!(
        args.build(
            &deployment,
            &prepared,
            &trust,
            &raw,
            Some(first.clone()),
            None
        )
        .is_err()
    );
    args.installed_state = "absent".into();
    assert!(
        args.build(
            &deployment,
            &prepared,
            &trust,
            &raw,
            Some(first.clone()),
            Some(first)
        )
        .is_err()
    );
}

#[test]
fn epoch_update_inputs_rejects_rebased_authority_trust_and_seed_sources() {
    let _guard = ChainDiscriminantGuard::enter(super::super::super::CHAIN_DISCRIMINANT);
    let (mut args, mut deployment, mut prepared, mut trust) = fixture();
    let raw = json::to_vec(&trust).unwrap();
    let first = args
        .build(&deployment, &prepared, &trust, &raw, None, None)
        .unwrap()
        .after;
    deployment.current.commit = first.release_source_commit.clone();
    deployment.current.daemon = Path::new(&first.unit_spec.cli)
        .with_file_name("iroha3d_taira")
        .to_str()
        .unwrap()
        .into();
    prepared.commit = "c".repeat(40);
    args.original_service_state = "running".into();
    args.first_epoch += 1;
    assert!(
        args.build(
            &deployment,
            &prepared,
            &trust,
            &raw,
            Some(first.clone()),
            None
        )
        .is_err()
    );
    args.first_epoch -= 1;
    trust.peers[0].build_fingerprint = Hash::new([7]);
    let current = json::to_vec(&trust).unwrap();
    args.build(
        &deployment,
        &prepared,
        &trust,
        &current,
        Some(first.clone()),
        None,
    )
    .unwrap();
    trust.peers[0].torii_origin = "https://different.example/".into();
    let changed = json::to_vec(&trust).unwrap();
    assert!(
        args.build(&deployment, &prepared, &trust, &changed, Some(first), None)
            .is_err()
    );
    args.original_seed_sources[1] = args.original_seed_sources[0].clone();
    assert!(seed_references(&trust, &args.original_seed_sources, deployment.network_id).is_err());
    args.original_seed_sources[1] = "/original/../alias.seed".into();
    assert!(seed_references(&trust, &args.original_seed_sources, deployment.network_id).is_err());
    trust.genesis_signed_wire_hex = "00".into();
    assert!(validate_deployment_trust(&trust, deployment.network_id).is_err());
}

#[test]
fn epoch_update_inputs_closed_preparation_has_no_implicit_state_or_receipt() {
    let _guard = ChainDiscriminantGuard::enter(super::super::super::CHAIN_DISCRIMINANT);
    let (args, deployment, prepared, trust) = fixture();
    let value = args
        .build(
            &deployment,
            &prepared,
            &trust,
            &json::to_vec(&trust).unwrap(),
            None,
            None,
        )
        .unwrap();
    let mut json: json::Value = json::from_slice(&json::to_vec(&value).unwrap()).unwrap();
    let object = json.as_object_mut().unwrap();
    object.insert("native_provisioning_receipt".into(), json::Value::Null);
    assert!(json::from_slice::<PreparationV1>(&json::to_vec(&json).unwrap()).is_err());
    json.as_object_mut()
        .unwrap()
        .remove("native_provisioning_receipt");
    json.as_object_mut().unwrap().remove("installed");
    assert!(json::from_slice::<PreparationV1>(&json::to_vec(&json).unwrap()).is_err());
}

#[cfg(unix)]
#[test]
fn epoch_update_inputs_output_is_atomic_private_and_never_replaced() {
    let parent = tempfile::Builder::new()
        .prefix(".epoch-update-public-")
        .tempdir_in(std::env::var_os("HOME").unwrap())
        .unwrap();
    fs::set_permissions(parent.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let output = parent.path().join("bundle");
    publish_bundle(&output, &[("preparation.json", b"public")]).unwrap();
    assert_eq!(fs::metadata(&output).unwrap().mode() & 0o777, 0o700);
    assert_eq!(
        fs::metadata(output.join("preparation.json"))
            .unwrap()
            .mode()
            & 0o777,
        0o600
    );
    assert!(publish_bundle(&output, &[("preparation.json", b"replacement")]).is_err());
    assert_eq!(
        fs::read(output.join("preparation.json")).unwrap(),
        b"public"
    );
}
