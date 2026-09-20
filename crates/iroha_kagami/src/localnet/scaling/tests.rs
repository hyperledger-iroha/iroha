//! Fixed geometry, bootstrap, custody, and actual command boundary tests.

use super::*;
use clap::Parser as _;
use iroha_data_model::{
    isi::MintBox,
    nexus::{LaneStorageProfile, LaneVisibility},
    transaction::{Executable, FeePaymentIntent, TransactionBuilder},
};
#[cfg(unix)]
use std::os::fd::{AsRawFd as _, IntoRawFd as _};

const SEED: &str = "abababababababababababababababababababababababababababababababab";

/// Create the ready original seed pipe shared by actual fixed-layout CLI fixtures.
#[cfg(unix)]
#[allow(
    unsafe_code,
    reason = "test-only anonymous seed pipe transfers its original read end"
)]
pub(super) fn seed_pipe(seed: &[u8]) -> fs::File {
    use std::os::fd::FromRawFd as _;
    let mut descriptors = [-1; 2];
    // SAFETY: the array has space for both descriptors; success transfers both valid FDs.
    assert_eq!(unsafe { libc::pipe(descriptors.as_mut_ptr()) }, 0);
    // SAFETY: each freshly created descriptor is uniquely owned exactly once.
    let (read, mut write) = unsafe {
        (
            std::fs::File::from_raw_fd(descriptors[0]),
            std::fs::File::from_raw_fd(descriptors[1]),
        )
    };
    let flags = rustix::fs::fcntl_getfl(&read).unwrap();
    rustix::fs::fcntl_setfl(&read, flags | rustix::fs::OFlags::NONBLOCK).unwrap();
    write.write_all(seed).unwrap();
    drop(write);
    read
}

fn layout(lanes: ScalingLanes, accounts: u16) -> ScalingLayout {
    ScalingLayout::from_args(Some(lanes), Some(accounts))
        .unwrap()
        .unwrap()
}

fn options(path: &Path) -> LocalnetOptions {
    LocalnetOptions {
        sora_profile: None,
        perf_profile: None,
        peers: NonZeroU16::new(4).unwrap(),
        seed: Some(SEED.to_owned()),
        bind_host: DEFAULT_BIND_HOST.to_owned(),
        public_host: DEFAULT_PUBLIC_HOST.to_owned(),
        base_api_port: 28080,
        base_p2p_port: 31337,
        out_dir: path.to_owned(),
        extra_accounts: 0,
        assets: vec![],
        consensus_mode: SumeragiConsensusMode::Npos,
        block_cadence_ms: None,
    }
}

#[test]
fn fixed_layout_arguments_require_exact_lanes_and_complete_account_groups() {
    assert!(ScalingLayout::from_args(None, None).unwrap().is_none());
    assert!(ScalingLayout::from_args(None, Some(4)).is_err());
    for lanes in [ScalingLanes::One, ScalingLanes::Four] {
        assert_eq!(
            ScalingLayout::from_args(Some(lanes), None)
                .unwrap()
                .unwrap()
                .accounts,
            4
        );
        for accounts in 0..=68 {
            assert_eq!(
                ScalingLayout::from_args(Some(lanes), Some(accounts)).is_ok(),
                (4..=64).contains(&accounts) && accounts % 4 == 0
            );
        }
    }
    for bad in ["0", "2", "3", "5", "01", "one", "four"] {
        assert!(
            crate::Cli::try_parse_from([
                "kagami",
                "localnet",
                "--out-dir",
                "/unused",
                "--seed-fd",
                "3",
                "--scaling-lanes",
                bad
            ])
            .is_err()
        );
    }
    assert!(
        crate::Cli::try_parse_from([
            "kagami",
            "localnet",
            "--out-dir",
            "/unused",
            "--scaling-lanes",
            "4"
        ])
        .is_err()
    );
    assert!(
        crate::Cli::try_parse_from([
            "kagami",
            "localnet",
            "--out-dir",
            "/unused",
            "--scaling-accounts",
            "4"
        ])
        .is_err()
    );
}

#[test]
fn fixed_layout_rejects_incompatible_options_before_creating_output() {
    let temp = tempfile::tempdir().unwrap();
    let temp_root = fs::canonicalize(temp.path()).unwrap();
    let dest = temp_root.as_path().join("uncreated");
    for invalid in 0..7 {
        let mut opts = options(&dest);
        match invalid {
            0 => opts.peers = NonZeroU16::new(7).unwrap(),
            1 => opts.consensus_mode = SumeragiConsensusMode::Permissioned,
            2 => opts.sora_profile = Some(SoraProfile::Nexus),
            3 => opts.extra_accounts = 1,
            4 => opts.seed = None,
            5 => opts.seed = Some(String::new()),
            6 => opts
                .assets
                .push(requested_localnet_asset_spec(&localnet_sample_asset_literal()).unwrap()),
            _ => unreachable!(),
        }
        assert!(
            generate_localnet_for_layout(
                &opts,
                &mut BufWriter::new(Vec::new()),
                None,
                Some(layout(ScalingLanes::Four, 4))
            )
            .is_err()
        );
        assert!(!dest.exists());
    }
    for extra in [
        vec!["--consensus-mode", "permissioned"],
        vec!["--scaling-accounts", "5"],
        vec!["--perf-profile", "10k-npos"],
    ] {
        #[cfg(unix)]
        let seed_input = seed_pipe(SEED.as_bytes());
        #[cfg(unix)]
        let seed_fd = seed_input.as_raw_fd().to_string();
        #[cfg(not(unix))]
        let seed_fd = "3".to_owned();
        let mut args = vec![
            "kagami",
            "localnet",
            "--out-dir",
            dest.to_str().unwrap(),
            "--seed-fd",
            &seed_fd,
            "--scaling-lanes",
            "4",
        ];
        args.extend(extra);
        let parsed = crate::Cli::try_parse_from(args).unwrap();
        #[cfg(unix)]
        let _transferred_seed = seed_input.into_raw_fd();
        assert!(parsed.command.run(&mut BufWriter::new(Vec::new())).is_err());
        assert!(!dest.exists());
    }
}

#[test]
fn fixed_account_keys_are_ordered_unique_and_independent_of_lane_count() {
    let one = layout(ScalingLanes::One, 64)
        .identities(Some(SEED.as_bytes()))
        .unwrap();
    let four = layout(ScalingLanes::Four, 64)
        .identities(Some(SEED.as_bytes()))
        .unwrap();
    let short = layout(ScalingLanes::Four, 4)
        .identities(Some(SEED.as_bytes()))
        .unwrap();
    assert_eq!(
        one.iter()
            .map(|a| &a.account_id)
            .collect::<BTreeSet<_>>()
            .len(),
        64
    );
    for index in 0..64 {
        assert_eq!(one[index].account_id, four[index].account_id);
        assert_eq!(one[index].public_key, four[index].public_key);
        assert_eq!(
            one[index].private_key.as_str(),
            four[index].private_key.as_str()
        );
        if index < 4 {
            assert_eq!(one[index].account_id, short[index].account_id);
        }
    }
    assert!(layout(ScalingLanes::One, 4).identities(None).is_err());
    assert!(layout(ScalingLanes::One, 4).identities(Some(b"")).is_err());
    let other = layout(ScalingLanes::One, 4)
        .identities(Some(b"different-private-fixture"))
        .unwrap();
    assert!(
        short
            .iter()
            .zip(other.iter())
            .all(|(a, b)| a.account_id != b.account_id)
    );
}

#[test]
fn fixed_config_projection_changes_only_catalog_routing_and_autoscale_fields() {
    let original = "chain = 'preserve-chain'\n[sumeragi]\nbody = 12345\n[nexus]\n[nexus.storage]\nlocal_budget_bytes = 1234567\n[nexus.fees]\nper_instruction_fee = '0.001'\n";
    let before = original.parse::<Table>().unwrap();
    let mut variants = vec![];
    for (lanes, count) in [(ScalingLanes::One, 1), (ScalingLanes::Four, 4)] {
        let layout = layout(lanes, 8);
        let accounts = layout.identities(Some(SEED.as_bytes())).unwrap();
        let rendered = layout
            .render_config(Zeroizing::new(original.to_owned()), &accounts)
            .unwrap();
        let mut table = rendered.parse::<Table>().unwrap();
        let nexus = table.get_mut("nexus").unwrap().as_table_mut().unwrap();
        assert_eq!(nexus["lane_count"].as_integer(), Some(count));
        assert_eq!(nexus["dataspace_catalog"].as_array().unwrap().len(), 1);
        assert_eq!(nexus["autoscale"]["enabled"].as_bool(), Some(false));
        let rules = nexus["routing_policy"]["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 8);
        for (index, rule) in rules.iter().enumerate() {
            assert_eq!(rule["lane"].as_integer(), Some(index as i64 % count));
            assert_eq!(
                rule["matcher"]["account"].as_str(),
                Some(accounts[index].account_id.to_string().as_str())
            );
            assert_eq!(rule["matcher"].as_table().unwrap().len(), 1);
        }
        for key in [
            "lane_count",
            "lane_catalog",
            "dataspace_catalog",
            "routing_policy",
            "autoscale",
        ] {
            assert!(nexus.remove(key).is_some());
        }
        assert_eq!(table, before);
        variants.push(rendered);
    }
    let mut one = variants[0].parse::<Table>().unwrap();
    let mut four = variants[1].parse::<Table>().unwrap();
    for table in [&mut one, &mut four] {
        let nexus = table.get_mut("nexus").unwrap().as_table_mut().unwrap();
        for key in ["lane_count", "lane_catalog", "routing_policy"] {
            nexus.remove(key).unwrap();
        }
    }
    assert_eq!(one, four, "all non-topology parameters must be identical");
}

#[test]
fn fixed_account_consumers_reject_missing_or_duplicate_pool_members() {
    let layout = layout(ScalingLanes::Four, 4);
    let mut accounts = layout.identities(Some(SEED.as_bytes())).unwrap();
    assert!(layout.validate_accounts(&accounts[..3]).is_err());
    accounts[1] =
        localnet_ephemeral_identity(Some(SEED.as_bytes()), b"scaling-account-00").unwrap();
    assert!(layout.validate_accounts(&accounts).is_err());
    assert!(
        layout
            .render_config(Zeroizing::new("[nexus]\n".to_owned()), &accounts)
            .is_err()
    );
    let accounts = layout.identities(Some(SEED.as_bytes())).unwrap();
    assert!(
        layout
            .render_config(Zeroizing::new("[".to_owned()), &accounts)
            .is_err()
    );
    assert!(
        layout
            .render_config(Zeroizing::new("chain = 'x'".to_owned()), &accounts)
            .is_err()
    );
}

#[test]
fn fixed_client_files_preserve_explicit_network_scope_and_owner_only_key_custody() {
    let temp = tempfile::tempdir().unwrap();
    let temp_root = crate::secure_fs::prepare_empty_private_directory(temp.path()).unwrap();
    let layout = layout(ScalingLanes::Four, 4);
    let accounts = layout.identities(Some(SEED.as_bytes())).unwrap();
    let host = CanonicalHost::parse("127.0.0.1", "host").unwrap();
    layout
        .write_accounts(
            temp_root.as_path(),
            8080,
            &host,
            DEFAULT_CHAIN_ID,
            777,
            &accounts,
        )
        .unwrap();
    assert_eq!(fs::read_dir(temp_root.as_path()).unwrap().count(), 4);
    for (index, account) in accounts.iter().enumerate() {
        let path = temp_root.as_path().join(account_config_name(index));
        let contents = Zeroizing::new(fs::read_to_string(&path).unwrap());
        let parsed = contents.parse::<Table>().unwrap();
        assert_eq!(
            parsed["network_id_file"].as_str(),
            Some(GENESIS_EXPECTED_HASH_FILE)
        );
        assert_eq!(parsed["chain"].as_str(), Some(DEFAULT_CHAIN_ID));
        assert_eq!(
            parsed["account"]["chain_discriminant"].as_integer(),
            Some(777)
        );
        assert_eq!(
            parsed["account"]["private_key"].as_str(),
            Some(account.private_key.as_str())
        );
        assert_eq!(
            parsed["account"]["public_key"]
                .as_str()
                .unwrap()
                .parse::<iroha_crypto::PublicKey>()
                .unwrap(),
            account.public_key
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(metadata.is_file());
            assert_eq!(metadata.permissions().mode() & 0o7777, 0o600);
            assert_eq!(metadata.nlink(), 1);
        }
    }
    assert!(!temp_root.as_path().join("client.toml").exists());
    let originals = (0..4)
        .map(|index| fs::read(temp_root.as_path().join(account_config_name(index))).unwrap())
        .collect::<Vec<_>>();
    assert!(
        layout
            .write_accounts(
                temp_root.as_path(),
                8080,
                &host,
                DEFAULT_CHAIN_ID,
                777,
                &accounts
            )
            .is_err()
    );
    for (index, original) in originals.iter().enumerate() {
        assert_eq!(
            &fs::read(temp_root.as_path().join(account_config_name(index))).unwrap(),
            original
        );
    }
    assert!(
        layout
            .write_accounts(
                &temp_root.as_path().join("missing-parent"),
                8080,
                &host,
                DEFAULT_CHAIN_ID,
                777,
                &accounts
            )
            .is_err()
    );
}

#[test]
#[cfg(unix)]
#[expect(
    clippy::too_many_lines,
    reason = "authenticate the actual signed deployment and all fixed geometry in one end-to-end boundary test"
)]
fn fixed_command_signed_genesis_authenticates_every_lane_and_funded_account() {
    let mut previous_accounts = None;
    for (lanes, count) in [("1", 1_usize), ("4", 4_usize)] {
        let temp = tempfile::tempdir().unwrap();
        let temp_root = fs::canonicalize(temp.path()).unwrap();
        let dest = temp_root.as_path().join("fixed");
        let seed_input = seed_pipe(SEED.as_bytes());
        let seed_fd = seed_input.as_raw_fd().to_string();
        let command = crate::Cli::try_parse_from([
            "kagami",
            "localnet",
            "--out-dir",
            dest.to_str().unwrap(),
            "--seed-fd",
            &seed_fd,
            "--scaling-lanes",
            lanes,
            "--scaling-accounts",
            "8",
        ])
        .unwrap();
        let mut output = BufWriter::new(Vec::new());
        let _transferred_seed = seed_input.into_raw_fd();
        command.command.run(&mut output).unwrap();
        output.flush().unwrap();
        let reply = String::from_utf8(output.into_inner().unwrap()).unwrap();
        let receipt: norito::json::Value = norito::json::from_str(&reply).unwrap();
        assert_eq!(receipt["consensus_mode"].as_str(), Some("npos"));
        assert_eq!(receipt["accounts"].as_array().unwrap().len(), 8);
        assert_eq!(
            reply.as_bytes(),
            fs::read(dest.join(anchors::RECEIPT_FILE)).unwrap()
        );
        assert!(!reply.contains(SEED));
        let manifest = RawGenesisTransaction::from_path(dest.join("genesis.json")).unwrap();
        assert_eq!(manifest.consensus_mode(), SumeragiConsensusMode::Npos);
        let signed = fs::read(dest.join("genesis.signed.nrt")).unwrap();
        let config =
            actual::Root::from_toml_source(TomlSource::from_file(dest.join("peer0.toml")).unwrap())
                .unwrap();
        let authority =
            crate::genesis::staged_signed_genesis_merge_authority(&manifest, &signed, &config)
                .unwrap();
        let mut expected_peers = build_peers(4, Some(SEED.as_bytes()), 8080, 1337)
            .unwrap()
            .into_iter()
            .map(|peer| PeerId::new(peer.public_key))
            .collect::<Vec<_>>();
        expected_peers.sort();
        assert_eq!(authority.active_lanes().len(), count);
        assert_eq!(authority.proofs_of_possession().len(), 4);
        assert_eq!(authority.lane_authority_catalog().rosters.len(), 1);
        assert_eq!(
            authority.lane_authority_catalog().lane_roster_indices,
            vec![0; count]
        );
        assert_eq!(authority.context().height, 1);
        assert_eq!(
            authority.catalog_hash(),
            iroha_data_model::nexus::LaneLifecycleParameterV1::catalog_hash(
                &config.nexus.configured_lane_catalog
            )
        );
        assert_eq!(
            config.nexus.lane_catalog,
            config.nexus.configured_lane_catalog
        );
        let registrations = manifest
            .instructions()
            .filter_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<RegisterPublicLaneValidator>()
            })
            .collect::<Vec<_>>();
        let activations = manifest
            .instructions()
            .filter_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<ActivatePublicLaneValidator>()
            })
            .collect::<Vec<_>>();
        assert_eq!(registrations.len(), 4);
        assert_eq!(activations.len(), 4);
        assert!(
            registrations
                .iter()
                .all(|registration| registration.lane_id == LaneId::SINGLE)
        );
        assert!(
            activations
                .iter()
                .all(|activation| activation.lane_id == LaneId::SINGLE)
        );
        assert_eq!(
            authority.context().network_id,
            NetworkId::from_genesis_hash(config.genesis.expected_hash)
        );
        for (index, binding) in authority.active_lanes().iter().enumerate() {
            assert_eq!(binding.lane_id, LaneId::new(index as u32));
            assert_eq!(binding.dataspace_id, DataSpaceId::UNIVERSAL);
            assert_eq!(binding.activation_height, 1);
            assert_eq!(
                authority
                    .lane_authority_catalog()
                    .roster_for_lane(index)
                    .unwrap()
                    .validators,
                expected_peers
            );
        }
        let selected = layout(
            if count == 1 {
                ScalingLanes::One
            } else {
                ScalingLanes::Four
            },
            8,
        );
        let accounts = selected.identities(Some(SEED.as_bytes())).unwrap();
        let ids = accounts
            .iter()
            .map(|account| account.account_id.clone())
            .collect::<Vec<_>>();
        if let Some(previous) = previous_accounts {
            assert_eq!(ids, previous);
        }
        previous_accounts = Some(ids);
        for (index, account) in accounts.iter().enumerate() {
            let registrations = manifest
                .instructions()
                .filter_map(|instruction| {
                    let RegisterBox::Account(register) =
                        instruction.as_any().downcast_ref::<RegisterBox>()?
                    else {
                        return None;
                    };
                    (register.object.id == account.account_id).then_some(register)
                })
                .collect::<Vec<_>>();
            assert_eq!(registrations.len(), 1);
            assert!(registrations[0].object.metadata.is_empty());
            let expected_asset = AssetId::new(
                localnet_fee_asset_definition_id(),
                account.account_id.clone(),
            );
            let mints = manifest
                .instructions()
                .filter_map(|instruction| {
                    let MintBox::Asset(mint) = instruction.as_any().downcast_ref::<MintBox>()?
                    else {
                        return None;
                    };
                    (mint.destination() == &expected_asset).then_some(mint)
                })
                .collect::<Vec<_>>();
            assert_eq!(mints.len(), 1);
            assert_eq!(mints[0].object(), &Quantity::from(100_u64));
            assert!(!manifest.instructions().any(|instruction| matches!(instruction.as_any().downcast_ref::<GrantBox>(), Some(GrantBox::Permission(grant)) if grant.destination() == &account.account_id)));
            let transaction = TransactionBuilder::new(
                authority.context().network_id,
                account.account_id.clone(),
                FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_executable(Executable::Instructions(
                vec![InstructionBox::from(SetKeyValue::account(
                    account.account_id.clone(),
                    format!("gscale_{index:064x}").parse().unwrap(),
                    Json::try_new(format!("{index:064x}")).unwrap(),
                ))]
                .into(),
            ))
            .sign(
                &account
                    .private_key
                    .parse::<iroha_crypto::PrivateKey>()
                    .unwrap(),
            );
            let decision = iroha_core::queue::evaluate_policy_with_catalog(
                &config.nexus.routing_policy,
                &config.nexus.lane_catalog,
                &config.nexus.dataspace_catalog,
                transaction.payload(),
            )
            .unwrap();
            assert_eq!(decision.lane_id, LaneId::new((index % count) as u32));
            assert_eq!(decision.dataspace_id, DataSpaceId::UNIVERSAL);
            assert_eq!(
                config.nexus.routing_policy.rules[index].lane,
                LaneId::new((index % count) as u32)
            );
            assert_eq!(
                config.nexus.routing_policy.rules[index].matcher.account,
                Some(account.account_id.to_string())
            );
            assert_eq!(
                config.nexus.routing_policy.rules[index].dataspace,
                Some(DataSpaceId::UNIVERSAL)
            );
            assert!(!reply.contains(account.private_key.as_str()));
        }
        assert!(
            selected
                .append_accounts(manifest.clone(), &accounts)
                .is_err(),
            "cannot reuse already registered workload accounts"
        );
        assert!(!config.nexus.autoscale.enabled);
        assert_eq!(config.nexus.fees.base_fee, Quantity::from(0_u64));
        assert_eq!(config.nexus.fees.per_byte_fee, Quantity::from(0_u64));
        assert_eq!(
            config.nexus.fees.per_instruction_fee,
            "0.001".parse::<Quantity>().unwrap()
        );
        assert_eq!(
            config.nexus.fees.per_gas_unit_fee,
            "0.00005".parse::<Quantity>().unwrap()
        );
        assert_eq!(config.nexus.fees.fee_asset_id, localnet_fee_asset_literal());
        assert_eq!(
            config.nexus.fees.settlement_mode,
            actual::NexusFeeSettlementMode::Direct
        );
        assert_eq!(config.nexus.dataspace_catalog.entries().len(), 1);
        assert_eq!(
            config.nexus.dataspace_catalog.entries()[0].fault_tolerance,
            1
        );
        for lane in config.nexus.lane_catalog.lanes() {
            assert_eq!(lane.storage, LaneStorageProfile::FullReplica);
            assert_eq!(lane.visibility, LaneVisibility::Public);
            assert!(lane.governance.is_none());
            assert!(lane.metadata.is_empty());
        }
        for peer in 1..4 {
            let peer_config = actual::Root::from_toml_source(
                TomlSource::from_file(dest.join(format!("peer{peer}.toml"))).unwrap(),
            )
            .unwrap();
            assert_eq!(
                peer_config.genesis.expected_hash,
                config.genesis.expected_hash
            );
            assert_eq!(peer_config.nexus.lane_catalog, config.nexus.lane_catalog);
            assert_eq!(
                peer_config.nexus.routing_policy,
                config.nexus.routing_policy
            );
            assert!(!peer_config.nexus.autoscale.enabled);
        }
    }
}

#[test]
fn fixed_config_secret_custody_preserves_valid_keys_and_sanitizes_failures() {
    let layout = layout(ScalingLanes::Four, 4);
    let accounts = layout.identities(Some(SEED.as_bytes())).unwrap();
    let secret = "fixed-layout-secret-must-not-appear-in-diagnostics";
    let valid = Zeroizing::new(format!("private_key = \"{secret}\"\n[nexus]\n"));
    let projected = layout.render_config(valid, &accounts).unwrap();
    let retained = crate::secret_toml::Table::new(
        crate::secret_toml::parse_table(&projected, "private test config").unwrap(),
    );
    assert_eq!(retained["private_key"].as_str(), Some(secret));
    for input in [
        format!("private_key = \"{secret}\"\ninvalid = ["),
        format!("private_key = \"{secret}\"\n"),
        format!("private_key = \"{secret}\"\nnexus = \"{secret}\"\n"),
    ] {
        let error = layout
            .render_config(Zeroizing::new(input), &accounts)
            .unwrap_err();
        for diagnostic in [format!("{error}"), format!("{error:?}")] {
            assert!(!diagnostic.contains(secret));
            assert!(!diagnostic.contains("private_key"));
        }
    }
}

#[test]
fn fixed_config_removes_file_loading_services_and_codec_but_keeps_other_fields() {
    let selected = layout(ScalingLanes::Four, 4);
    let accounts = selected.identities(Some(SEED.as_bytes())).unwrap();
    let original = Zeroizing::new(
        "private_key = 'retained-validator-secret'\n[nexus]\n\
         [torii]\naddress = '127.0.0.1:8080'\n\
         [torii.account_onboarding]\nprivate_key_file = '/unread/onboarding-secret'\n\
         [torii.faucet]\nenabled = true\nprivate_key_file = '/unread/faucet-secret'\n\
         [streaming]\nidentity_private_key = 'retained-streaming-secret'\n\
         session_store_dir = '/retained/sessions'\n\
         [streaming.codec]\nrans_tables_path = '/unread/rans-secret'\n"
            .to_owned(),
    );
    let rendered = selected.render_config(original, &accounts).unwrap();
    let table = crate::secret_toml::Table::new(
        crate::secret_toml::parse_table(&rendered, "fixed removal test").unwrap(),
    );
    assert_eq!(
        table["private_key"].as_str(),
        Some("retained-validator-secret")
    );
    assert_eq!(table["torii"]["address"].as_str(), Some("127.0.0.1:8080"));
    assert!(
        !table["torii"]
            .as_table()
            .unwrap()
            .contains_key("account_onboarding")
    );
    assert!(!table["torii"].as_table().unwrap().contains_key("faucet"));
    assert!(!table["streaming"].as_table().unwrap().contains_key("codec"));
    assert_eq!(
        table["streaming"]["identity_private_key"].as_str(),
        Some("retained-streaming-secret")
    );
    assert_eq!(
        table["streaming"]["session_store_dir"].as_str(),
        Some("/retained/sessions")
    );
    for removed in [
        "/unread/onboarding-secret",
        "/unread/faucet-secret",
        "/unread/rans-secret",
    ] {
        assert!(!rendered.contains(removed));
    }
}

#[test]
fn fixed_generated_peer_configs_parse_from_retained_bytes_after_side_files_move() {
    for lanes in [ScalingLanes::One, ScalingLanes::Four] {
        let temp = tempfile::tempdir().unwrap();
        let temp_root = fs::canonicalize(temp.path()).unwrap();
        let out = temp_root.as_path().join("generated");
        generate_localnet_for_layout(
            &options(&out),
            &mut BufWriter::new(Vec::new()),
            None,
            Some(layout(lanes, 4)),
        )
        .unwrap();
        // The internal fixed producer has no human scaffold or unused private sidecars.
        for absent in [
            "README.md",
            "start.sh",
            "stop.sh",
            ".gitignore",
            "codec",
            LOCALNET_RUNTIME_DIRECTORY,
            GENESIS_PRIVATE_KEY_FILE,
            GENESIS_PUBLIC_KEY_FILE,
        ] {
            assert!(!out.join(absent).exists(), "{absent}");
        }
        let signed = fs::read(out.join("genesis.signed.nrt")).unwrap();
        let block = iroha_data_model::block::decode_framed_signed_block(&signed).unwrap();
        let expected = NetworkId::from_genesis_hash(block.hash());
        let manifest = RawGenesisTransaction::from_path(out.join("genesis.json")).unwrap();
        let retained = (0..4)
            .map(|index| {
                let path = out.join(format!("peer{index}.toml"));
                let bytes = Zeroizing::new(fs::read_to_string(&path).unwrap());
                let table = crate::secret_toml::Table::new(
                    crate::secret_toml::parse_table(&bytes, "generated fixed peer").unwrap(),
                );
                assert_eq!(
                    table["genesis"]["expected_hash"].as_str(),
                    Some(expected.to_string().as_str())
                );
                assert!(
                    !table["genesis"]
                        .as_table()
                        .unwrap()
                        .contains_key("expected_hash_file")
                );
                assert!(
                    !table["torii"]
                        .as_table()
                        .unwrap()
                        .contains_key("account_onboarding")
                );
                assert!(!table["torii"].as_table().unwrap().contains_key("faucet"));
                assert!(!table["streaming"].as_table().unwrap().contains_key("codec"));
                (path, bytes)
            })
            .collect::<Vec<_>>();
        let identity = out.join(GENESIS_EXPECTED_HASH_FILE);
        assert!(identity.is_file());
        fs::rename(&identity, temp_root.as_path().join("held-network-record")).unwrap();
        assert!(!identity.exists());
        for absent in [
            out.join(LOCALNET_RUNTIME_DIRECTORY)
                .join(LOCALNET_OPERATOR_SIGNER_KEY_FILE),
            out.join(LOCALNET_RUNTIME_DIRECTORY)
                .join(LOCALNET_ONBOARDING_SIGNER_KEY_FILE),
            out.join(LOCALNET_RANS_TABLE_RELATIVE_PATH),
        ] {
            assert!(!absent.exists());
        }
        let mut first = None;
        for (path, bytes) in &retained {
            let config = parse_localnet_peer_config(bytes, Some(path)).unwrap();
            assert_eq!(config.genesis.expected_hash, block.hash());
            assert!(config.torii.account_onboarding.is_none());
            assert!(config.torii.faucet.is_none());
            let defaults = actual::StreamingCodec::from_defaults();
            assert_eq!(config.streaming.codec.cabac_mode, defaults.cabac_mode);
            assert_eq!(
                config.streaming.codec.trellis_block_sizes,
                defaults.trellis_block_sizes
            );
            assert_eq!(config.streaming.codec.entropy_mode, defaults.entropy_mode);
            assert_eq!(config.streaming.codec.bundle_width, defaults.bundle_width);
            assert_eq!(config.streaming.codec.bundle_accel, defaults.bundle_accel);
            assert_eq!(
                config.streaming.codec.rans_tables_path,
                defaults.rans_tables_path
            );
            let authority =
                crate::genesis::staged_signed_genesis_merge_authority(&manifest, &signed, &config)
                    .unwrap();
            assert_eq!(authority.context().network_id, expected);
            assert_eq!(authority.context().height, 1);
            assert_eq!(authority.context().roster.len(), 4);
            assert_eq!(
                authority.active_lanes().len(),
                usize::from(layout(lanes, 4).lane_count())
            );
            let projection = (
                authority.context().clone(),
                authority.catalog_hash(),
                authority.active_lanes().to_vec(),
                authority.lane_authority_catalog().clone(),
                authority.proofs_of_possession().to_vec(),
            );
            if let Some(first) = first.as_ref() {
                assert_eq!(&projection, first);
            } else {
                first = Some(projection);
            }
            // A caller cannot replace the independently selected final genesis identity.
            let mut changed = config.clone();
            changed.genesis.expected_hash =
                HashOf::from_untyped_unchecked(Hash::new(b"wrong-final-genesis"));
            assert!(crate::genesis::staged_signed_genesis_merge_authority(
                &manifest, &signed, &changed,
            ).is_err());
        }
    }
}

#[test]
fn ordinary_generated_configs_keep_published_identity_and_service_dependencies() {
    let temp = tempfile::tempdir().unwrap();
    let temp_root = fs::canonicalize(temp.path()).unwrap();
    let out = temp_root.as_path().join("ordinary");
    generate_localnet(&options(&out), &mut BufWriter::new(Vec::new())).unwrap();
    let readme = fs::read_to_string(out.join("README.md")).unwrap();
    assert!(readme.contains("enable structural `torii.account_onboarding`"));
    assert!(readme.contains("calling sponsored onboarding"));
    assert!(readme.contains("`kagami docker` without `--seed`"));
    assert!(!readme.contains("exact final genesis hash inline"));
    assert!(!readme.contains("genesis-funded workload accounts"));
    let mut retained = Vec::new();
    for index in 0..4 {
        let path = out.join(format!("peer{index}.toml"));
        let bytes = Zeroizing::new(fs::read_to_string(&path).unwrap());
        let table = crate::secret_toml::Table::new(
            crate::secret_toml::parse_table(&bytes, "ordinary peer").unwrap(),
        );
        assert_eq!(
            table["genesis"]["expected_hash_file"].as_str(),
            Some(GENESIS_EXPECTED_HASH_FILE)
        );
        assert!(
            !table["genesis"]
                .as_table()
                .unwrap()
                .contains_key("expected_hash")
        );
        assert!(
            table["torii"]["account_onboarding"]["private_key_file"]
                .as_str()
                .is_some()
        );
        assert!(
            table["torii"]["faucet"]["private_key_file"]
                .as_str()
                .is_some()
        );
        assert_eq!(table["torii"]["faucet"]["enabled"].as_bool(), Some(true));
        assert!(
            table["streaming"]["codec"]["rans_tables_path"]
                .as_str()
                .is_some()
        );
        let parsed = parse_localnet_peer_config(&bytes, Some(&path)).unwrap();
        assert!(parsed.torii.account_onboarding.is_some());
        assert!(parsed.torii.faucet.is_some());
        retained.push((path, bytes));
    }
    fs::rename(
        out.join(GENESIS_EXPECTED_HASH_FILE),
        temp_root.as_path().join("held-ordinary-identity"),
    )
    .unwrap();
    for (path, bytes) in retained {
        assert!(parse_localnet_peer_config(&bytes, Some(&path)).is_err());
    }
}

#[test]
fn fixed_generated_context_is_canonical_and_matches_every_final_peer_stage() {
    use iroha_data_model::block::consensus_v2::HeightContext;

    for lanes in [ScalingLanes::One, ScalingLanes::Four] {
        let temp = tempfile::tempdir().unwrap();
        let temp_root = fs::canonicalize(temp.path()).unwrap();
        let out = temp_root.as_path().join("generated");
        let mut output = BufWriter::new(Vec::new());
        generate_localnet_for_layout(&options(&out), &mut output, None, Some(layout(lanes, 4)))
            .unwrap();
        let output = String::from_utf8(output.into_inner().unwrap()).unwrap();
        let path = out.join(GENESIS_CONTEXT_FILE);
        let receipt: norito::json::Value = norito::json::from_str(&output).unwrap();
        assert!(
            receipt["artifacts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry["path"].as_str() == Some(GENESIS_CONTEXT_FILE))
        );
        assert!(!out.join("README.md").exists());
        let context_bytes = crate::secure_fs::read_private_file(&path).unwrap();
        assert!(!context_bytes.is_empty());
        assert!(context_bytes.len() <= GENESIS_CONTEXT_MAX_BYTES);
        let context: HeightContext = norito::decode_from_bytes(&context_bytes).unwrap();
        assert_eq!(context.height, 1);
        assert_eq!(context.roster.len(), 4);
        assert_eq!(
            norito::canonical_frame_len(&context).unwrap(),
            context_bytes.len()
        );
        let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        assert_eq!(
            norito::core::to_bytes_bounded(&context, GENESIS_CONTEXT_MAX_BYTES).unwrap(),
            context_bytes
        );
        let manifest = RawGenesisTransaction::from_path(out.join("genesis.json")).unwrap();
        let signed =
            iroha_genesis::read_signed_genesis_bytes(&out.join("genesis.signed.nrt")).unwrap();
        let block = iroha_genesis::decode_signed_genesis(&signed).unwrap();
        assert_eq!(
            context.network_id,
            NetworkId::from_genesis_hash(block.hash())
        );
        for index in 0..4 {
            let peer = out.join(format!("peer{index}.toml"));
            let rendered = Zeroizing::new(fs::read_to_string(&peer).unwrap());
            let config = parse_localnet_peer_config(&rendered, Some(&peer)).unwrap();
            assert_eq!(config.genesis.expected_hash, block.hash());
            let authority =
                crate::genesis::staged_signed_genesis_merge_authority(&manifest, &signed, &config)
                    .unwrap();
            assert_eq!(&context, authority.context());
            assert_eq!(
                authority.active_lanes().len(),
                usize::from(layout(lanes, 4).lane_count())
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(metadata.is_file());
            assert_eq!(metadata.mode() & 0o777, 0o600);
            assert_eq!(metadata.nlink(), 1);
        }
        assert!(write_owner_only_localnet_file(&path, b"replacement context").is_err());
        assert_eq!(
            crate::secure_fs::read_private_file(&path).unwrap(),
            context_bytes
        );
        let mut trailing = context_bytes.clone();
        trailing.push(0);
        assert!(norito::decode_from_bytes::<HeightContext>(&trailing).is_err());
    }
}

#[test]
fn fixed_context_derivation_rejects_final_wire_manifest_config_and_bound_tampering() {
    for lanes in [ScalingLanes::One, ScalingLanes::Four] {
        let temp = tempfile::tempdir().unwrap();
        let temp_root = fs::canonicalize(temp.path()).unwrap();
        let out = temp_root.as_path().join("generated");
        generate_localnet_for_layout(
            &options(&out),
            &mut BufWriter::new(Vec::new()),
            None,
            Some(layout(lanes, 4)),
        )
        .unwrap();
        let manifest_path = out.join("genesis.json");
        let signed_path = out.join("genesis.signed.nrt");
        let peer_path = out.join("peer0.toml");
        let rendered = Zeroizing::new(fs::read_to_string(&peer_path).unwrap());
        let config = parse_localnet_peer_config(&rendered, Some(&peer_path)).unwrap();
        let context = crate::secure_fs::read_private_file(&out.join(GENESIS_CONTEXT_FILE)).unwrap();
        assert_eq!(
            genesis_context_bytes(&manifest_path, &signed_path, &config).unwrap(),
            context
        );
        let mut wrong_hash = config.clone();
        wrong_hash.genesis.expected_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"wrong-final-context-anchor"));
        let error = genesis_context_bytes(&manifest_path, &signed_path, &wrong_hash).unwrap_err();
        assert!(format!("{error:#}").contains("signed genesis body hashes to"));
        let mut wrong_policy = config.clone();
        wrong_policy.pipeline.amx_group_budget_ms += 1;
        let error = genesis_context_bytes(&manifest_path, &signed_path, &wrong_policy).unwrap_err();
        assert!(matches!(
            error.downcast_ref::<iroha_core::sumeragi::GenesisMergeAuthorityError>(),
            Some(iroha_core::sumeragi::GenesisMergeAuthorityError::Bootstrap(
                iroha_core::sumeragi::V2GenesisBootstrapError::NexusAmxContextHashMismatch { .. }
            ))
        ));
        let manifest_bytes = fs::read(&manifest_path).unwrap();
        let changed = RawGenesisTransaction::from_path(&manifest_path)
            .unwrap()
            .with_consensus_mode(SumeragiConsensusMode::Permissioned);
        fs::write(
            &manifest_path,
            norito::json::to_json_pretty(&changed).unwrap(),
        )
        .unwrap();
        let error = genesis_context_bytes(&manifest_path, &signed_path, &config).unwrap_err();
        assert!(format!("{error:#}").contains("genesis manifest consensus mode"));
        fs::write(&manifest_path, &manifest_bytes).unwrap();
        let signed_bytes = fs::read(&signed_path).unwrap();
        let mut trailing = signed_bytes.clone();
        trailing.push(0);
        fs::write(&signed_path, &trailing).unwrap();
        assert!(genesis_context_bytes(&manifest_path, &signed_path, &config).is_err());
        // Sparse oversized input must fail the existing reader before Norito admission.
        fs::OpenOptions::new()
            .write(true)
            .open(&signed_path)
            .unwrap()
            .set_len((SIGNED_GENESIS_MAX_BYTES_V1 as u64) + 1)
            .unwrap();
        let error = genesis_context_bytes(&manifest_path, &signed_path, &config).unwrap_err();
        assert!(format!("{error:#}").contains("read bounded final scaling signed genesis"));
        fs::write(&signed_path, &signed_bytes).unwrap();
        assert_eq!(
            genesis_context_bytes(&manifest_path, &signed_path, &config).unwrap(),
            context
        );
        assert_eq!(
            crate::secure_fs::read_private_file(&out.join(GENESIS_CONTEXT_FILE)).unwrap(),
            context
        );
    }
}

#[test]
fn ordinary_generation_does_not_publish_or_advertise_fixed_genesis_context() {
    let temp = tempfile::tempdir().unwrap();
    let temp_root = fs::canonicalize(temp.path()).unwrap();
    let out = temp_root.as_path().join("ordinary");
    let mut output = BufWriter::new(Vec::new());
    generate_localnet(&options(&out), &mut output).unwrap();
    assert!(!out.join(GENESIS_CONTEXT_FILE).exists());
    assert!(
        !String::from_utf8(output.into_inner().unwrap())
            .unwrap()
            .contains("genesis_context:")
    );
    let readme = fs::read_to_string(out.join("README.md")).unwrap();
    assert!(!readme.contains("genesis-context.nrt"));
    assert!(readme.contains("enable structural `torii.account_onboarding`"));
    assert!(readme.contains("calling sponsored onboarding"));
}

#[test]
fn fixed_storage_roots_and_rendered_runtime_paths_are_exact() {
    for lanes in [ScalingLanes::One, ScalingLanes::Four] {
        let temp = tempfile::tempdir().unwrap();
        let temp_root = fs::canonicalize(temp.path()).unwrap();
        let out = temp_root.join("fixed");
        let mut output = BufWriter::new(Vec::new());
        generate_localnet_for_layout(&options(&out), &mut output, None, Some(layout(lanes, 4)))
            .unwrap();
        let receipt: norito::json::Value =
            norito::json::from_slice(&output.into_inner().unwrap()).unwrap();
        let mut expected = receipt["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["path"].as_str().unwrap().to_owned())
            .collect::<BTreeSet<_>>();
        expected.insert("genesis-anchors.json".to_owned());
        expected.insert("storage".to_owned());
        assert_eq!(
            fs::read_dir(&out)
                .unwrap()
                .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                .collect::<BTreeSet<_>>(),
            expected
        );
        assert_eq!(fs::read_dir(out.join("storage")).unwrap().count(), 4);
        for index in 0..4 {
            let role = out.join("storage").join(format!("peer{index}"));
            assert_eq!(
                fs::read_dir(&role)
                    .unwrap()
                    .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from(["kura".to_owned(), "state".to_owned()])
            );
            assert_eq!(fs::read_dir(role.join("kura")).unwrap().count(), 0);
            assert_eq!(fs::read_dir(role.join("state")).unwrap().count(), 0);
            let path = out.join(format!("peer{index}.toml"));
            let text = Zeroizing::new(fs::read_to_string(&path).unwrap());
            let config = parse_localnet_peer_config(&text, Some(&path)).unwrap();
            let paths = LocalnetPeerStoragePaths::scaling(&out, index);
            assert_eq!(paths.kura, role.join("kura"));
            assert_eq!(paths.state, role.join("state"));
            runtime_paths::validate(&config, &paths).unwrap();
            let mut foreign = config.clone();
            foreign.torii.sorafs_discovery.replay_checkpoint_path =
                temp_root.as_path().join("outside");
            assert!(runtime_paths::validate(&foreign, &paths).is_err());
            let mut foreign = config.clone();
            foreign.soracloud_runtime.state_dir = paths.kura.join("state");
            assert!(runtime_paths::validate(&foreign, &paths).is_err());
            let mut foreign = config.clone();
            foreign.torii.iso_bridge.enabled = true;
            assert!(runtime_paths::validate(&foreign, &paths).is_err());
            assert!(
                runtime_paths::validate(
                    &config,
                    &LocalnetPeerStoragePaths::scaling(&out, (index + 1) % 4)
                )
                .is_err()
            );
        }
        let ordinary = LocalnetPeerStoragePaths::new(&out, 0);
        assert_eq!(ordinary.kura, out.join("storage/peer0"));
        assert_eq!(ordinary.state, out.join("state/peer0"));
    }
}

#[test]
fn fixed_runtime_binding_requires_structure_and_redacts_parse_failures() {
    let paths = LocalnetPeerStoragePaths::scaling(Path::new("/fixed"), 0);
    let secret = "do-not-disclose-runtime-secret";
    for text in [
        format!("private_key = '{secret}'\nbad = ["),
        format!("private_key = '{secret}'"),
    ] {
        let error = runtime_paths::bind(Zeroizing::new(text), &paths).unwrap_err();
        assert!(!format!("{error:?}").contains(secret));
    }
    let text = Zeroizing::new(format!("private_key = '{secret}'\n[sorafs]\n"));
    let result = runtime_paths::bind(text, &paths).unwrap();
    let table = crate::secret_toml::Table::new(
        crate::secret_toml::parse_table(&result, "runtime test").unwrap(),
    );
    assert_eq!(table["private_key"].as_str(), Some(secret));
    assert_eq!(
        table["snapshot"]["store_dir"].as_str(),
        Some("/fixed/storage/peer0/state/snapshot")
    );
    assert_eq!(
        table["sorafs"]["discovery"]["replay_checkpoint_path"].as_str(),
        Some("/fixed/storage/peer0/state/torii/sorafs_discovery_replay.nrt")
    );
}

#[test]
fn fixed_profile_rejects_inline_seed_and_requires_only_bounded_descriptor_transport() {
    let base = ["kagami", "localnet", "--out-dir", "/unused"];
    for extra in [
        vec!["--scaling-lanes", "4", "--seed", SEED],
        vec!["--scaling-lanes", "4", "--seed-fd", "3", "--seed", SEED],
        vec!["--seed-fd", "3"],
        vec!["--scaling-lanes", "4", "--seed-fd", "2"],
        vec!["--scaling-lanes", "4", "--seed-fd", "65536"],
    ] {
        assert!(crate::Cli::try_parse_from(base.into_iter().chain(extra)).is_err());
    }
    assert!(
        crate::Cli::try_parse_from(
            base.into_iter()
                .chain(["--seed", "generic-development-fixture"])
        )
        .is_ok()
    );
    assert!(
        crate::Cli::try_parse_from(base.into_iter().chain([
            "--scaling-lanes",
            "4",
            "--seed-fd",
            "3"
        ]))
        .is_ok()
    );
}
