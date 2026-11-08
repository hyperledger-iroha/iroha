//! Original-State binding and local package custody, without network or signer inputs.

use super::*;
use crate::replay_check::tests::{store_config, tree};
use iroha_core::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
use iroha_data_model::{
    isi::{Grant, Register},
    permission::Permission,
};
use iroha_executor_data_model::permission::sorafs::{
    CanSetSorafsReservePolicy, CanUpsertSorafsProviderCredit,
};

fn request_for(state: &State) -> InspectRequestV1 {
    let view = state.view();
    InspectRequestV1 {
        schema: REQUEST_SCHEMA.into(),
        mode: "inspect".into(),
        module: MODULE.into(),
        expected_cut: ExpectedCutV1 {
            network_id: view.network_id,
            genesis_hash: *view.network_id.as_genesis_hash(),
            height: view.height() as u64,
            tip_hash: view.latest_block_hash().unwrap(),
            selection_id: view.world().runtime_selection().id().unwrap(),
        },
    }
}

fn native_replay(chain: &CertifiedTestChain) -> ReplayCheckReport {
    replay_check::check_replay(
        ReplayCheckInputs {
            state: Arc::clone(chain.state()),
            source_lanes: chain.kura().store_root().join("lanes"),
            genesis_account: chain.genesis_account().clone(),
            consensus_mode: iroha_data_model::parameter::system::ConsensusMode::Permissioned,
        },
        &mut |_, _| {},
    )
    .expect("actual native replay/cold capture")
}

#[test]
fn inspect_exports_the_original_matched_state_and_keeps_inventory_refusal() {
    for protected in [false, true] {
        let mut config = TestChainConfig::new(World::new(), 1_000);
        if protected {
            let holder = AccountId::new(config.genesis_key.public_key().clone());
            config.genesis_instructions.push(
                Grant::account_permission(Permission::from(CanSetSorafsReservePolicy), holder)
                    .into(),
            );
        }
        let mut chain = CertifiedTestChain::start(config).unwrap();
        chain.commit(Vec::new());
        let state = chain.state();
        let before = tree(chain.kura().store_root().as_path());
        let core = native_replay(&chain);
        assert!(matches!(core.outcome, ReplayCheckOutcome::Matched));
        let request = request_for(state);
        let request_bytes = norito::json::to_vec(&request).unwrap();
        let owner = MatchedReplay {
            state,
            report: &core,
            tip_height: chain.height(),
            genesis_hash: request.expected_cut.genesis_hash,
        };
        let prepared = prepare_inspection(&owner, &request, request_bytes.len()).unwrap();
        let material: norito::json::Value = norito::json::from_slice(&prepared.material).unwrap();
        let inventory: ReplaySorafsAuthorityInventoryV1 =
            norito::json::from_value(material.get("activation_inventory").unwrap().clone())
                .unwrap();
        assert_eq!(inventory.inventory_clear, !protected);
        let observed = material.get("authority_observation").unwrap();
        assert_eq!(observed.get("scan_complete").unwrap().as_bool(), Some(true));
        assert_eq!(
            observed
                .get("executor")
                .unwrap()
                .get("kind")
                .unwrap()
                .as_str(),
            Some("Initial")
        );
        assert_eq!(
            observed
                .get("direct_grants")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .any(|row| {
                    row.get("permission").unwrap().get("name").unwrap().as_str()
                        == Some("CanSetSorafsReservePolicy")
                }),
            protected
        );
        assert_eq!(
            inventory.failure_reason,
            iroha_core::runtime::validate_sorafs_authority_activation_inventory(
                state.view().world()
            )
            .err()
        );
        let selection: RootSelectionV1 = norito::decode_canonical(&prepared.selection).unwrap();
        assert_eq!(selection.id().unwrap(), request.expected_cut.selection_id);
        assert_eq!(&selection, state.view().world().runtime_selection());
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("review");
        let publication = write_package(&output, &request_bytes, &prepared).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                std::fs::metadata(&output).unwrap().permissions().mode() & 0o077,
                0
            );
            assert_eq!(
                std::fs::metadata(output.join("inspection.json"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o077,
                0
            );
        }
        let manifest_bytes = std::fs::read(output.join("manifest.json")).unwrap();
        assert_eq!(
            publication.manifest_artifact_sha256,
            hex::encode(iroha_crypto::sha256(&manifest_bytes))
        );
        assert_eq!(
            std::fs::read(output.join("request.json")).unwrap(),
            request_bytes
        );
        let manifest: norito::json::Value = norito::json::from_slice(&manifest_bytes).unwrap();
        for entry in manifest.get("files").unwrap().as_array().unwrap() {
            let path = entry.get("path").unwrap().as_str().unwrap();
            let contents = std::fs::read(output.join(path)).unwrap();
            assert_eq!(
                entry.get("artifact_sha256").unwrap().as_str().unwrap(),
                hex::encode(iroha_crypto::sha256(contents))
            );
        }
        assert!(write_package(&output, &request_bytes, &prepared).is_err());
        assert!(material.get("change_set").is_none());
        assert!(material.get("qualification").is_none());
        assert_eq!(tree(chain.kura().store_root().as_path()), before);
        assert_eq!(state.committed_height() as u64, chain.height());
    }
}

#[test]
fn inspection_captures_a_genuinely_replayed_suffix() {
    // Both independent fixtures execute the same signed genesis. Only the source has H2/H3;
    // the replay State receives their original certified frames, never their World rows.
    let mut source = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    source.commit(Vec::new());
    source.commit(Vec::new());
    let destination = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    assert_eq!(source.state().network_id, destination.state().network_id);
    for height in 2..=source.height() {
        destination
            .kura()
            .store_block(source.committed(height).block().clone())
            .unwrap();
    }
    let before = tree(source.kura().store_root().as_path());
    let mut replayed = Vec::new();
    let core = replay_check::check_replay(
        ReplayCheckInputs {
            state: Arc::clone(destination.state()),
            source_lanes: source.kura().store_root().join("lanes"),
            genesis_account: source.genesis_account().clone(),
            consensus_mode: iroha_data_model::parameter::system::ConsensusMode::Permissioned,
        },
        &mut |height, _| replayed.push(height),
    )
    .unwrap();
    assert!(matches!(core.outcome, ReplayCheckOutcome::Matched));
    assert_eq!(core.start.height(), 1);
    assert_eq!(replayed, vec![2, 3]);
    let request = request_for(source.state());
    let owner = MatchedReplay {
        state: destination.state(),
        report: &core,
        tip_height: source.height(),
        genesis_hash: request.expected_cut.genesis_hash,
    };
    let prepared = prepare_inspection(&owner, &request, 0).unwrap();
    let selection: RootSelectionV1 = norito::decode_canonical(&prepared.selection).unwrap();
    assert_eq!(selection.id().unwrap(), request.expected_cut.selection_id);
    assert_eq!(tree(source.kura().store_root().as_path()), before);
}

#[test]
fn inspection_refuses_every_foreign_cut_and_nonmatched_report() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    chain.commit(Vec::new());
    let core = native_replay(&chain);
    let request = request_for(chain.state());
    let owner = MatchedReplay {
        state: chain.state(),
        report: &core,
        tip_height: chain.height(),
        genesis_hash: request.expected_cut.genesis_hash,
    };
    for field in 0..5 {
        let mut wrong = request.clone();
        match field {
            0 => wrong.expected_cut.height += 1,
            1 => wrong.expected_cut.tip_hash = request.expected_cut.genesis_hash,
            2 => wrong.expected_cut.selection_id.0[0] ^= 1,
            3 => {
                wrong.expected_cut.network_id =
                    NetworkId::from_genesis_hash(request.expected_cut.tip_hash)
            }
            4 => wrong.expected_cut.genesis_hash = request.expected_cut.tip_hash,
            _ => unreachable!(),
        }
        assert!(prepare_inspection(&owner, &wrong, 0).is_err());
    }
    let missing = ReplayCheckReport {
        outcome: ReplayCheckOutcome::MissingHistory { height: 3 },
        ..core.clone()
    };
    let owner = MatchedReplay {
        report: &missing,
        ..owner
    };
    assert!(prepare_inspection(&owner, &request, 0).is_err());
    let empty = ReplayCheckReport {
        replayed_to: 0,
        ..core
    };
    let owner = MatchedReplay {
        report: &empty,
        tip_height: 0,
        ..owner
    };
    assert!(prepare_inspection(&owner, &request, 0).is_err());
}

#[test]
fn request_is_closed_bounded_and_inspect_only() {
    let chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    let request = request_for(chain.state());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("request.json");
    let bytes = norito::json::to_vec(&request).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(read_request(&path).unwrap().1, bytes);
    let mut draft = request.clone();
    draft.mode = "draft".into();
    std::fs::write(&path, norito::json::to_vec(&draft).unwrap()).unwrap();
    assert!(read_request(&path).is_err());
    let mut unknown = norito::json::to_value(&request).unwrap();
    unknown.as_object_mut().unwrap().insert(
        "qualification".into(),
        norito::json::Value::String("00".repeat(32)),
    );
    std::fs::write(&path, norito::json::to_vec(&unknown).unwrap()).unwrap();
    assert!(read_request(&path).is_err());
    std::fs::write(&path, vec![b' '; MAX_REQUEST_BYTES + 1]).unwrap();
    assert!(read_request(&path).is_err());
    #[cfg(unix)]
    {
        let link = directory.path().join("linked-request");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(read_request(&link).is_err());
    }
}

#[test]
fn output_refuses_replacement_source_overlap_and_partial_publication() {
    let directory = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(directory.path()).unwrap();
    let config = store_config(&root);
    let output = root.join("review");
    assert!(output_destination(&config, &output, None).is_ok());
    for path in [
        root.join("kura/review"),
        root.join("snapshot/review"),
        root.join("secrets/review"),
    ] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        assert!(output_destination(&config, &path, None).is_err());
    }
    let config_path = root.join("config.toml");
    std::fs::write(&config_path, "# fixture").unwrap();
    assert!(output_destination(&config, &output, Some(&config_path)).is_err());
    std::fs::create_dir(&output).unwrap();
    assert!(output_destination(&config, &output, None).is_err());
    publish_file(&output, "inspection.json", b"partial").unwrap();
    assert!(publish_file(&output, "inspection.json", b"replacement").is_err());
    assert!(!output.join("manifest.json").exists());
    assert_eq!(
        std::fs::read(output.join("inspection.json")).unwrap(),
        b"partial"
    );
    #[cfg(unix)]
    {
        let alias = root.join("alias");
        std::os::unix::fs::symlink(&output, &alias).unwrap();
        assert!(output_destination(&config, &alias.join("review"), None).is_err());
    }
}

#[test]
fn inspection_flags_require_replay_and_a_pair_and_exclude_repair() {
    let pair = [
        "--runtime-addition-review-request",
        "/request.json",
        "--runtime-addition-review-output",
        "/output",
    ];
    assert!(
        crate::Args::try_parse_from(["iroha3d", "--check-replay"].into_iter().chain(pair)).is_ok()
    );
    assert!(crate::Args::try_parse_from(["iroha3d"].into_iter().chain(pair)).is_err());
    for flag in [
        "--runtime-addition-review-request",
        "--runtime-addition-review-output",
    ] {
        assert!(
            crate::Args::try_parse_from(["iroha3d", "--check-replay", flag, "/value"]).is_err()
        );
    }
    for other in ["--check-config", "--check-storage"] {
        assert!(
            crate::Args::try_parse_from(
                ["iroha3d", "--check-replay", other].into_iter().chain(pair)
            )
            .is_err()
        );
    }
    assert!(
        crate::Args::try_parse_from(
            [
                "iroha3d",
                "--check-replay",
                "--rehearse-sorafs-repair",
                "/signed.norito",
                "--repair-execution-time-ms",
                "1234",
            ]
            .into_iter()
            .chain(pair)
        )
        .is_err()
    );
}

#[test]
fn inspection_entrypoint_never_publishes_an_unstarted_replay() {
    let chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    let request = request_for(chain.state());
    let directory = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(directory.path()).unwrap();
    let path = root.join("request.json");
    std::fs::write(&path, norito::json::to_vec(&request).unwrap()).unwrap();
    let before = tree(&root);
    let output = root.join("review");
    assert!(run_sorafs_authority_inspection(&store_config(&root), &path, &output, None).is_err());
    assert!(!output.exists());
    assert_eq!(tree(&root), before);
}

#[test]
fn authority_observation_retains_native_direct_role_and_membership_origins() {
    let mut config = TestChainConfig::new(World::new(), 1_000);
    let account = AccountId::new(config.genesis_key.public_key().clone());
    let management_role: RoleId = "inspection_management".parse().unwrap();
    let unrelated_role: RoleId = "inspection_unrelated".parse().unwrap();
    config.genesis_instructions.extend([
        Grant::account_permission(
            Permission::from(CanProposeRuntimeChangeSet),
            account.clone(),
        )
        .into(),
        Grant::account_permission(Permission::from(CanSetSorafsReservePolicy), account.clone())
            .into(),
        Register::role(
            Role::new(management_role.clone(), account.clone())
                .add_permission(CanManageParliament)
                .add_permission(CanSetParameters)
                .add_permission(CanUpsertSorafsProviderCredit),
        )
        .into(),
        Register::role(Role::new(COMPLIANCE_ROLE.parse().unwrap(), account.clone())).into(),
        Register::role(Role::new(unrelated_role.clone(), account.clone())).into(),
    ]);
    let mut chain = CertifiedTestChain::start(config).unwrap();
    chain.commit(Vec::new());
    let before = tree(chain.kura().store_root().as_path());
    let view = chain.state().view();
    let world = view.world();
    let observation = inspect_authority(world, MAX_INVENTORY_ROWS).unwrap();
    assert!(observation.scan_complete);
    assert_eq!(observation.executor.kind, "Initial");
    assert_eq!(
        observation.executor.semantic_hash,
        iroha_core::executor::executor_norito::net_state_hash(world.executor()).unwrap()
    );
    let proposer = observation
        .direct_grants
        .iter()
        .find(|row| row.permission.0.name() == "CanProposeRuntimeChangeSet")
        .unwrap();
    assert_eq!(proposer.account.0, &account);
    assert!(proposer.account_exists);
    assert_eq!(proposer.exact_administrative_token, Some(true));
    assert!(
        observation
            .direct_grants
            .iter()
            .all(|row| { row.permission.0.name() != "CanManageParliament" })
    );
    let manager = observation
        .roles
        .iter()
        .find(|row| row.id.0 == &management_role)
        .unwrap();
    assert_eq!(
        manager.exact_administrative_tokens,
        vec!["CanManageParliament", "CanSetParameters"]
    );
    assert_eq!(
        norito::json::to_vec(&manager.definition).unwrap(),
        norito::json::to_vec(world.roles().get(&management_role).unwrap()).unwrap()
    );
    assert!(
        manager
            .definition
            .0
            .permission_epoch(&Permission::from(CanManageParliament))
            .is_some()
    );
    let rendered: norito::json::Value =
        norito::json::from_slice(&norito::json::to_vec(&observation).unwrap()).unwrap();
    let rendered_role = rendered
        .get("roles")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            row.get("exact_administrative_tokens")
                .unwrap()
                .as_array()
                .unwrap()
                .len()
                == 2
        })
        .unwrap();
    let rendered_epochs = rendered_role
        .get("permission_epochs")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(rendered_epochs.len(), 3);
    let manager_epoch = rendered_epochs
        .iter()
        .find(|row| {
            row.get("permission").unwrap().get("name").unwrap().as_str()
                == Some("CanManageParliament")
        })
        .unwrap();
    assert_eq!(manager_epoch.get("epoch").unwrap().as_u64(), Some(1));
    assert!(
        observation
            .roles
            .iter()
            .any(|row| row.id.0.to_string() == COMPLIANCE_ROLE)
    );
    assert!(
        !observation
            .roles
            .iter()
            .any(|row| row.id.0 == &unrelated_role)
    );
    assert!(observation.memberships.iter().any(|row| {
        row.membership.0.id == management_role
            && row.membership.0.account == account
            && row.account_exists
            && row.role_exists
    }));
    assert!(
        observation
            .memberships
            .iter()
            .any(|row| row.membership.0.id.to_string() == COMPLIANCE_ROLE)
    );
    assert!(
        !observation
            .memberships
            .iter()
            .any(|row| row.membership.0.id == unrelated_role)
    );
    assert_eq!(observation.roles_scanned, 3);
    assert_eq!(observation.role_permissions_scanned, 3);
    assert_eq!(observation.memberships_scanned, 3);
    assert!(observation.accounts_scanned >= 1);
    // Local resource refusal cannot publish a truncated inventory with scan_complete=true.
    let row_count = observation.direct_grants.len()
        + observation.roles.len()
        + observation.memberships.len()
        + observation
            .roles
            .iter()
            .map(|role| role.permission_epochs.len())
            .sum::<usize>();
    assert!(inspect_authority(world, row_count - 1).is_err());
    assert!(inspect_authority(world, row_count).is_ok());
    assert!(norito::json::to_json_bounded_boxed(&observation, 16).is_err());
    assert_eq!(tree(chain.kura().store_root().as_path()), before);
}

#[test]
fn authority_observation_keeps_malformed_named_grants_without_inventing_authority() {
    let mut world = World::new();
    let account = AccountId::new(
        TestChainConfig::new(World::new(), 1_000)
            .genesis_key
            .public_key()
            .clone(),
    );
    let malformed = Permission::new(
        "CanManageParliament".into(),
        iroha_primitives::json::Json::new("not the canonical unit token"),
    );
    let scoped = Permission::new(
        "CanCompleteSorafsReplicationOrder".into(),
        iroha_primitives::json::Json::new("malformed provider scope"),
    );
    let unrelated = Permission::new(
        "UnrelatedInspectionToken".into(),
        iroha_primitives::json::Json::new(()),
    );
    world.account_permissions_mut_for_testing().insert(
        account.clone(),
        [malformed.clone(), scoped.clone(), unrelated]
            .into_iter()
            .collect(),
    );
    let view = world.view();
    let observation = inspect_authority(&view, MAX_INVENTORY_ROWS).unwrap();
    assert_eq!(observation.direct_permissions_scanned, 3);
    assert_eq!(observation.direct_permission_sets_scanned, 1);
    assert_eq!(observation.direct_grants.len(), 2);
    assert!(
        observation
            .direct_grants
            .iter()
            .all(|row| !row.account_exists)
    );
    let manager = observation
        .direct_grants
        .iter()
        .find(|row| row.permission.0 == &malformed)
        .unwrap();
    assert_eq!(manager.exact_administrative_token, Some(false));
    let completion = observation
        .direct_grants
        .iter()
        .find(|row| row.permission.0 == &scoped)
        .unwrap();
    assert_eq!(completion.exact_administrative_token, None);
    assert_eq!(
        norito::json::to_vec(&manager.permission).unwrap(),
        norito::json::to_vec(&malformed).unwrap()
    );
    assert!(inspect_authority(&view, 1).is_err());
}
