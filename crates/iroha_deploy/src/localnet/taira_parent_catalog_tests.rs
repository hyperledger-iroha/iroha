//! Focused authored-baseline proofs for the optional Taira parent without physical IS.

use super::*;
use iroha_data_model::isi::{MintBox, TransferBox};

#[test]
fn taira_parent_catalog_removes_only_is_and_preserves_sparse_capacity() {
    let profile = Some(SoraProfile::Nexus);
    let with = TairaParentCatalog::WithIs;
    let without = TairaParentCatalog::WithoutIs;
    let (with_count, with_lanes) = localnet_lane_catalog(profile, true, with).unwrap();
    let (without_count, without_lanes) = localnet_lane_catalog(profile, true, without).unwrap();
    assert_eq!((with_count, without_count), (8, 8));
    assert_eq!(
        without_lanes,
        with_lanes
            .into_iter()
            .filter(|lane| { lane["index"].as_integer() != Some(i64::from(TAIRA_IS_LANE_INDEX)) })
            .collect::<Vec<_>>()
    );
    assert_eq!(
        without_lanes
            .iter()
            .map(|lane| lane["index"].as_integer().unwrap())
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4]
    );
    let dataspaces = localnet_dataspace_catalog(profile, 1, true, without);
    assert_eq!(
        dataspaces,
        localnet_dataspace_catalog(profile, 1, true, with)
            .into_iter()
            .filter(|entry| entry["alias"].as_str() != Some("is"))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        dataspaces
            .iter()
            .map(|entry| entry["alias"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["universal", "paynet", "nexus"]
    );
    let with_routes = localnet_routing_policy(profile, true, with).unwrap();
    let without_routes = localnet_routing_policy(profile, true, without).unwrap();
    assert_eq!(without_routes["default_lane"], with_routes["default_lane"]);
    assert_eq!(
        without_routes["default_dataspace"],
        with_routes["default_dataspace"]
    );
    assert_eq!(
        without_routes["rules"].as_array().unwrap(),
        &with_routes["rules"].as_array().unwrap()[..4]
    );
}

#[test]
fn taira_parent_catalog_selector_rejects_other_chains() {
    TairaParentCatalog::WithoutIs
        .validate(PUBLIC_TAIRA_CHAIN_ID)
        .unwrap();
    for chain in [DEFAULT_CHAIN_ID, PUBLIC_NEXUS_CHAIN_ID, "another-parent"] {
        assert!(TairaParentCatalog::WithoutIs.validate(chain).is_err());
        TairaParentCatalog::WithIs.validate(chain).unwrap();
    }
    assert_eq!(TairaParentCatalog::default(), TairaParentCatalog::WithIs);
}

#[test]
fn taira_parent_bootstrap_defers_only_the_builtin_is_binding() {
    let _guard = ChainDiscriminantGuard::enter(369);
    let seed = b"taira-parent-authored-bootstrap";
    let peers = build_peers(4, Some(seed), 8080, 1337).unwrap();
    let (genesis_public, _) = generate_genesis_key_pair(Some(seed), GENESIS_SEED).unwrap();
    let authority = AccountId::new(genesis_public.clone());
    let genesis = generate_raw_genesis(
        &genesis_public,
        SumeragiConsensusMode::Npos,
        PUBLIC_TAIRA_CHAIN_ID,
        &peers,
    )
    .unwrap();
    let client = localnet_client_account_id();
    let assets = vec![
        localnet_kagemusha_asset_spec_for_client(&client, true),
        AssetSpec {
            id: LOCALNET_KAGEMUSHA_ASSET_ID.into(),
            name: "Independent IS2 app asset".into(),
            alias: Some("app#demo.is2".into()),
            owned_by: client.clone(),
            mint_to: client.clone(),
            quantity: 7,
        },
    ];
    let with = extend_genesis(
        genesis.clone(),
        &authority,
        Some(seed),
        0,
        &assets,
        TairaParentCatalog::WithIs,
    )
    .unwrap();
    let without = extend_genesis(
        genesis,
        &authority,
        Some(seed),
        0,
        &assets,
        TairaParentCatalog::WithoutIs,
    )
    .unwrap();
    let definition =
        AssetDefinitionId::parse_address_literal(TAIRA_DIGITAL_SHEKEL_ASSET_ID).unwrap();
    let is_binding = |instruction: &&iroha_data_model::isi::InstructionBox| {
        instruction
            .as_any()
            .downcast_ref::<SetAssetDefinitionAlias>()
            .is_some_and(|bind| {
                bind.asset_definition_id == definition
                    && bind
                        .alias
                        .as_ref()
                        .is_some_and(|alias| alias.to_string() == TAIRA_DIGITAL_SHEKEL_ASSET_ALIAS)
            })
    };
    let before = with.instructions().collect::<Vec<_>>();
    let after = without.instructions().collect::<Vec<_>>();
    assert_eq!(
        before
            .iter()
            .copied()
            .filter(|instruction| is_binding(instruction))
            .count(),
        1
    );
    assert_eq!(
        after
            .iter()
            .copied()
            .filter(|instruction| is_binding(instruction))
            .count(),
        0
    );
    assert_eq!(
        after,
        before
            .into_iter()
            .filter(|instruction| !is_binding(instruction))
            .collect::<Vec<_>>(),
        "before deriving new consensus/context commitments, every other authored instruction and value stays exact"
    );
    assert_eq!(without.instructions().filter_map(|instruction| instruction.as_any().downcast_ref::<RegisterBox>()).filter(|register| matches!(register, RegisterBox::Domain(domain) if domain.object().id == DomainId::try_new("boi", "is").unwrap())).count(), 1);
    assert!(without.instructions().any(|instruction| matches!(instruction.as_any().downcast_ref::<RegisterBox>(), Some(RegisterBox::AssetDefinition(register)) if register.object().id == definition && register.object().owning_domain.is_none())));
    assert!(without.instructions().any(|instruction| matches!(instruction.as_any().downcast_ref::<MintBox>(), Some(MintBox::Asset(mint)) if mint.destination().definition == definition)));
    assert!(without.instructions().any(|instruction| matches!(instruction.as_any().downcast_ref::<TransferBox>(), Some(TransferBox::AssetDefinition(transfer)) if transfer.object() == &definition)));
    assert!(without.instructions().any(|instruction| {
        instruction
            .as_any()
            .downcast_ref::<SetAssetDefinitionAlias>()
            .is_some_and(|bind| {
                bind.alias
                    .as_ref()
                    .is_some_and(|alias| alias.to_string() == "app#demo.is2")
            })
    }));
}

fn peer_projection(selection: TairaParentCatalog) -> toml::Table {
    let _guard = ChainDiscriminantGuard::enter(369);
    let peers = build_peers(4, Some(b"taira-parent-render-fixture"), 8080, 1337).unwrap();
    let genesis_public = REAL_GENESIS_ACCOUNT_KEYPAIR.public_key();
    let operator = localnet_client_account_id();
    let operator_literal = operator.to_string();
    let operator_public = operator.expect_single_signatory();
    let bind = CanonicalHost::parse(DEFAULT_BIND_HOST, "bind").unwrap();
    let public = CanonicalHost::parse(DEFAULT_PUBLIC_HOST, "public").unwrap();
    let paths = LocalnetPeerStoragePaths::new(Path::new("/unit-fixture"), 0);
    let raw = render_peer_config(
        &peers[0],
        &[],
        &[],
        genesis_public,
        Path::new("/unit-fixture/genesis.signed.nrt"),
        LocalnetGenesisIdentitySource::BootstrapInline(HashOf::from_untyped_unchecked(Hash::new(
            b"authored test identity",
        ))),
        &[],
        &paths,
        None,
        PUBLIC_TAIRA_CHAIN_ID,
        Some(369),
        (&bind, &public),
        RenderPeerFeatures {
            mcp_enabled: true,
            npos_bootstrap: true,
            taira: true,
            taira_parent_catalog: selection,
            operator_account: &operator_literal,
            operator_public_key: operator_public,
            onboarding_account: &operator_literal,
            runtime: None,
        },
        Some(SoraProfile::Nexus),
        selection
            .includes_is()
            .then_some(Path::new("/unit-fixture/lane-manifests")),
        Some(1),
        &operator_literal,
        None,
        None,
        None,
        LOCALNET_QUEUE_CAPACITY,
    );
    toml::from_str(&raw).unwrap()
}

#[test]
fn taira_parent_renderer_preserves_other_runtime_and_parliament_settings() {
    let mut with = peer_projection(TairaParentCatalog::WithIs);
    let mut without = peer_projection(TairaParentCatalog::WithoutIs);
    let a = with.get_mut("nexus").unwrap().as_table_mut().unwrap();
    let b = without.get_mut("nexus").unwrap().as_table_mut().unwrap();
    assert_eq!(a["governance"], b["governance"]);
    assert!(a.contains_key("registry"));
    assert!(!b.contains_key("registry"));
    for field in [
        "lane_catalog",
        "dataspace_catalog",
        "routing_policy",
        "registry",
    ] {
        a.remove(field);
        b.remove(field);
    }
    assert_eq!(
        with, without,
        "all non-catalog settings and fixture identities stay exact"
    );
}

#[test]
fn taira_parent_without_is_writes_no_active_is_manifest() {
    let _guard = ChainDiscriminantGuard::enter(369);
    let temp = localnet_test_helpers::private_tempdir().unwrap();
    let peers = build_peers(4, Some(b"taira-parent-no-manifest"), 8080, 1337).unwrap();
    assert!(
        write_localnet_lane_manifests(
            temp.path(),
            Some(SoraProfile::Nexus),
            &peers,
            Some(369),
            true,
            TairaParentCatalog::WithoutIs
        )
        .unwrap()
        .is_none()
    );
    assert!(!temp.path().join("lane-manifests").exists());
}

#[test]
fn taira_parent_without_is_routes_retained_bootstrap_through_universal() {
    use iroha_core::{
        query::store::LiveQueryStore,
        queue::{ConfigLaneRouter, LaneRouter},
        state::{State, World},
    };
    use iroha_data_model::{
        nexus::{DataSpaceCatalog, DataSpaceMetadata, LaneCatalog, LaneConfig},
        transaction::{FeePaymentIntent, TransactionBuilder},
    };
    use std::num::NonZeroU32;
    let _guard = ChainDiscriminantGuard::enter(369);
    let profile = Some(SoraProfile::Nexus);
    let selection = TairaParentCatalog::WithoutIs;
    let entries = localnet_dataspace_catalog(profile, 1, true, selection);
    let dataspaces = entries
        .iter()
        .map(|entry| DataSpaceMetadata {
            id: DataSpaceId::new(u64::try_from(entry["id"].as_integer().unwrap()).unwrap()),
            alias: entry["alias"].as_str().unwrap().into(),
            description: entry["description"].as_str().map(str::to_owned),
            fault_tolerance: 1,
        })
        .collect::<Vec<_>>();
    let id_for = |alias: &str| {
        dataspaces
            .iter()
            .find(|entry| entry.alias == alias)
            .unwrap()
            .id
    };
    let (count, lanes) = localnet_lane_catalog(profile, true, selection).unwrap();
    let lanes = lanes
        .iter()
        .map(|entry| LaneConfig {
            id: LaneId::new(u32::try_from(entry["index"].as_integer().unwrap()).unwrap()),
            dataspace_id: id_for(entry["dataspace"].as_str().unwrap()),
            alias: entry["alias"].as_str().unwrap().into(),
            description: entry["description"].as_str().map(str::to_owned),
            ..LaneConfig::default()
        })
        .collect();
    let routes = localnet_routing_policy(profile, true, selection).unwrap();
    let mut nexus = actual::Nexus::default();
    nexus.dataspace_catalog = DataSpaceCatalog::new(dataspaces.clone()).unwrap();
    nexus.lane_catalog = LaneCatalog::new(
        NonZeroU32::new(u32::try_from(count).unwrap()).unwrap(),
        lanes,
    )
    .unwrap();
    nexus.routing_policy = actual::LaneRoutingPolicy {
        default_lane: LaneId::SINGLE,
        default_dataspace: DataSpaceId::UNIVERSAL,
        rules: routes["rules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|rule| {
                let matcher = rule["matcher"].as_table().unwrap();
                actual::LaneRoutingRule {
                    lane: LaneId::new(u32::try_from(rule["lane"].as_integer().unwrap()).unwrap()),
                    dataspace: Some(id_for(rule["dataspace"].as_str().unwrap())),
                    matcher: actual::LaneRoutingMatcher {
                        account: matcher
                            .get("account")
                            .and_then(toml::Value::as_str)
                            .map(str::to_owned),
                        instruction: matcher
                            .get("instruction")
                            .and_then(toml::Value::as_str)
                            .map(str::to_owned),
                        description: matcher
                            .get("description")
                            .and_then(toml::Value::as_str)
                            .map(str::to_owned),
                    },
                }
            })
            .collect(),
    };
    let router = ConfigLaneRouter::new(
        nexus.routing_policy.clone(),
        nexus.dataspace_catalog.clone(),
        nexus.lane_catalog.clone(),
    );
    let owner = localnet_client_account_id();
    let definition_id =
        AssetDefinitionId::parse_address_literal(TAIRA_DIGITAL_SHEKEL_ASSET_ID).unwrap();
    let definition = AssetDefinition::numeric(
        definition_id.clone(),
        "ds",
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    );
    let world = World::with(
        [],
        [Account::new(owner.clone()).build(&owner)],
        [definition.clone().build(&owner)],
    );
    let state =
        State::new_with_pre_genesis_nexus_for_testing(world, nexus, LiveQueryStore::start_test());
    let instructions: Vec<iroha_data_model::isi::InstructionBox> = vec![
        Register::domain(Domain::new(DomainId::try_new("boi", "is").unwrap())).into(),
        Register::asset_definition(definition).into(),
        Mint::asset_quantity(1_u32, AssetId::new(definition_id.clone(), owner.clone())).into(),
        Transfer::asset_definition(owner.clone(), definition_id.clone(), owner.clone()).into(),
    ];
    for instruction in instructions {
        let builder = TransactionBuilder::new_genesis(
            owner.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction]);
        let decision = router
            .try_route_with_view(builder.payload(), &state.view())
            .unwrap();
        assert_eq!(decision.lane_id, LaneId::SINGLE);
        assert_eq!(decision.dataspace_id, DataSpaceId::UNIVERSAL);
    }
}
