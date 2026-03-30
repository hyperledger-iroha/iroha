#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Telemetry scheduler TEU integration tests.
#![cfg(feature = "telemetry")]

use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroUsize,
    sync::Arc,
    time::Duration,
};

use eyre::Result;
use iroha_config::parameters::actual::{
    LaneRoutingMatcher, LaneRoutingPolicy, LaneRoutingRule, Nexus, Queue as QueueConfig,
};
use iroha_core::{
    gas,
    queue::{ConfigLaneRouter, LaneRouter, Queue, QueueLimits, SingleLaneRouter},
    state::{State, World},
    telemetry::StateTelemetry,
    tx::AcceptedTransaction,
};
use iroha_crypto::KeyPair;
use iroha_data_model::{
    isi::{
        InstructionBox,
        prelude::{Mint, SetKeyValue, Transfer},
    },
    metadata::Metadata,
    nexus::{
        DataSpaceCatalog, DataSpaceId, DataSpaceMetadata, LaneCatalog, LaneConfig, LaneId,
        LaneVisibility,
    },
    prelude::*,
};
use iroha_primitives::{json::Json, time::TimeSource};
use iroha_telemetry::metrics::Metrics;
use iroha_test_samples::gen_account_in;
use nonzero_ext::nonzero;
use tokio::sync::broadcast;

const TEST_CHAIN_ID: &str = "00000000-0000-0000-0000-000000000000";

fn build_world(authority: &AccountId, domain_id: &DomainId) -> World {
    let domain = Domain::new(domain_id.clone()).build(authority);
    let account = Account::new(authority.clone()).build(authority);
    let asset_definition_id = AssetDefinitionId::new(domain_id.clone(), "xor".parse().unwrap());
    let asset_definition = {
        let __asset_definition_id = asset_definition_id;
        AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::default())
            .with_name(__asset_definition_id.name().to_string())
    }
    .build(authority);

    World::with([domain], [account], [asset_definition])
}

fn build_transaction(
    chain_id: &ChainId,
    authority: &AccountId,
    keypair: &KeyPair,
    time_source: &TimeSource,
    instructions: Vec<InstructionBox>,
) -> AcceptedTransaction<'static> {
    let tx =
        TransactionBuilder::new_with_time_source(chain_id.clone(), authority.clone(), time_source)
            .with_instructions(instructions)
            .with_metadata(Metadata::default())
            .sign(keypair.private_key());
    let default_limits = TransactionParameters::default();
    let params = TransactionParameters::with_max_signatures(
        nonzero!(16_u64),
        nonzero!(4096_u64),
        nonzero!(4096_u64),
        default_limits.max_tx_bytes(),
        default_limits.max_decompressed_bytes(),
        default_limits.max_metadata_depth(),
    );
    let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
    AcceptedTransaction::accept(tx, chain_id, Duration::from_secs(30), params, &crypto_cfg)
        .expect("transaction should be accepted")
}

#[test]
fn queue_teu_backlog_matches_metering() -> Result<()> {
    let chain_id = ChainId::from(TEST_CHAIN_ID);
    let (account_id, keypair) = gen_account_in("wonderland");
    let wonderland_domain: DomainId = "wonderland".parse().unwrap();
    let world = build_world(&account_id, &wonderland_domain);
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let query_store = iroha_core::query::store::LiveQueryStore::start_test();

    let metrics = Arc::new(Metrics::default());
    let telemetry = StateTelemetry::new(Arc::clone(&metrics), true);

    let mut nexus = Nexus::default();
    nexus.fusion.exit_teu = 12_345;
    let queue_limits = QueueLimits::from_nexus(&nexus);

    let mut state_inner = State::with_telemetry(world, kura, query_store, telemetry);
    state_inner
        .set_nexus(nexus)
        .expect("apply Nexus config for TEU scheduler test");
    let state = Arc::new(state_inner);
    let (events_sender, _) = broadcast::channel(16);
    let queue_cfg = QueueConfig::default();
    let router: Arc<dyn LaneRouter> = Arc::new(SingleLaneRouter::new());
    let queue = Arc::new(Queue::from_config_with_router_and_limits(
        queue_cfg,
        events_sender,
        router,
        queue_limits.clone(),
        None,
    ));

    let time_source = TimeSource::new_system();

    let asset_definition_id =
        AssetDefinitionId::new(wonderland_domain.clone(), "xor".parse().unwrap());
    let asset_id = AssetId::of(asset_definition_id, account_id.clone());

    let mint = Mint::asset_numeric(10_u32, asset_id.clone());
    let transfer = Transfer::asset_numeric(asset_id.clone(), 5_u32, account_id.clone());
    let metadata_instruction = SetKeyValue::account(
        account_id.clone(),
        "teu_key".parse().unwrap(),
        Json::new("value"),
    );

    let txs = vec![
        build_transaction(
            &chain_id,
            &account_id,
            &keypair,
            &time_source,
            vec![InstructionBox::from(mint.clone())],
        ),
        build_transaction(
            &chain_id,
            &account_id,
            &keypair,
            &time_source,
            vec![InstructionBox::from(transfer.clone())],
        ),
        build_transaction(
            &chain_id,
            &account_id,
            &keypair,
            &time_source,
            vec![InstructionBox::from(metadata_instruction.clone())],
        ),
    ];

    let expected_teu: u64 = [
        gas::meter_instructions(&[InstructionBox::from(mint.clone())]),
        gas::meter_instructions(&[InstructionBox::from(transfer.clone())]),
        gas::meter_instructions(&[InstructionBox::from(metadata_instruction.clone())]),
    ]
    .into_iter()
    .sum();

    for tx in txs.clone() {
        if let Err(failure) = queue.push(tx, state.view()) {
            return Err(eyre::eyre!(failure.err.to_string()));
        }
    }

    let lane_label = LaneId::SINGLE.as_u32().to_string();
    let dataspace_label = DataSpaceId::GLOBAL.as_u64().to_string();

    let backlog = metrics
        .nexus_scheduler_dataspace_teu_backlog
        .with_label_values(&[lane_label.as_str(), dataspace_label.as_str()])
        .get();
    assert_eq!(backlog, expected_teu);

    let capacity = metrics
        .nexus_scheduler_lane_teu_capacity
        .with_label_values(&[lane_label.as_str()])
        .get();
    assert_eq!(capacity, queue_limits.for_lane(LaneId::SINGLE).teu_capacity);

    // Drain the queue and ensure backlog resets to zero.
    let mut guards = Vec::new();
    let max = NonZeroUsize::new(txs.len()).expect("non-zero");
    queue.get_transactions_for_block(&state.view(), max, &mut guards);
    drop(guards);

    let backlog_after = metrics
        .nexus_scheduler_dataspace_teu_backlog
        .with_label_values(&[lane_label.as_str(), dataspace_label.as_str()])
        .get();
    assert_eq!(backlog_after, 0);

    Ok(())
}

#[test]
#[allow(clippy::too_many_lines, clippy::unnecessary_wraps)]
fn queue_routes_transactions_across_configured_lanes() -> Result<()> {
    let chain_id = ChainId::from(TEST_CHAIN_ID);
    let (lane0_account, lane0_keypair) = gen_account_in("nexus");
    let (lane1_account, lane1_keypair) = gen_account_in("nexus_alt");
    let lane0_domain_id: DomainId = "nexus".parse().unwrap();
    let lane1_domain_id: DomainId = "nexus_alt".parse().unwrap();

    // Assemble world with both authorities registered.
    let domain0: Domain = Domain::new(lane0_domain_id.clone()).build(&lane0_account);
    let domain1: Domain = Domain::new(lane1_domain_id.clone()).build(&lane1_account);
    let account0 = Account::new(lane0_account.clone())
        .build(&lane0_account);
    let account1 = Account::new(lane1_account.clone())
        .build(&lane1_account);
    let world = World::with(
        [domain0, domain1],
        [account0, account1],
        Vec::<AssetDefinition>::new(),
    );

    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let query_store = iroha_core::query::store::LiveQueryStore::start_test();
    let metrics = Arc::new(Metrics::default());
    let telemetry = StateTelemetry::new(Arc::clone(&metrics), true);

    // Nexus catalog with two lanes and distinct dataspace assignments.
    let mut lane0_metadata = BTreeMap::new();
    lane0_metadata.insert("scheduler.teu_capacity".to_string(), "900".to_string());
    lane0_metadata.insert(
        "scheduler.starvation_bound_slots".to_string(),
        "4".to_string(),
    );
    let lane0 = LaneConfig {
        id: LaneId::new(0),
        dataspace_id: DataSpaceId::GLOBAL,
        alias: "public".to_string(),
        description: Some("Public execution lane".to_string()),
        visibility: LaneVisibility::Public,
        lane_type: Some("core".to_string()),
        governance: None,
        settlement: Some("xor".to_string()),
        metadata: lane0_metadata,
        ..LaneConfig::default()
    };

    let mut lane1_metadata = BTreeMap::new();
    lane1_metadata.insert("scheduler.teu_capacity".to_string(), "1500".to_string());
    lane1_metadata.insert(
        "scheduler.starvation_bound_slots".to_string(),
        "6".to_string(),
    );
    let lane1 = LaneConfig {
        id: LaneId::new(1),
        dataspace_id: DataSpaceId::new(1),
        alias: "private".to_string(),
        description: Some("Privacy-preserving lane".to_string()),
        visibility: LaneVisibility::Restricted,
        lane_type: Some("privacy".to_string()),
        governance: None,
        settlement: Some("xor".to_string()),
        metadata: lane1_metadata,
        ..LaneConfig::default()
    };

    let lane_catalog = LaneCatalog::new(nonzero!(2_u32), vec![lane0, lane1]).expect("catalog");

    let dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: DataSpaceId::new(1),
            alias: "alpha".to_string(),
            description: Some("Dedicated dataspace".to_string()),
            fault_tolerance: 1,
        },
    ])
    .expect("dataspace catalog");

    let routing_policy = LaneRoutingPolicy {
        default_lane: LaneId::new(0),
        default_dataspace: DataSpaceId::GLOBAL,
        rules: vec![LaneRoutingRule {
            lane: LaneId::new(1),
            dataspace: Some(DataSpaceId::new(1)),
            matcher: LaneRoutingMatcher {
                account: Some(lane1_account.to_string()),
                instruction: None,
                description: Some("route lane1 account to private lane".to_string()),
            },
        }],
    };

    let nexus = Nexus {
        enabled: true,
        lane_catalog: lane_catalog.clone(),
        dataspace_catalog: dataspace_catalog.clone(),
        routing_policy: routing_policy.clone(),
        ..Nexus::default()
    };

    let queue_limits = QueueLimits::from_nexus(&nexus);

    let mut state_inner = State::with_telemetry(world, kura, query_store, telemetry);
    state_inner
        .set_nexus(nexus.clone())
        .expect("apply Nexus config for router TEU test");
    let state = Arc::new(state_inner);

    let (events_sender, _) = broadcast::channel(16);
    let queue_cfg = QueueConfig::default();
    let router: Arc<dyn LaneRouter> = Arc::new(ConfigLaneRouter::new(
        routing_policy,
        dataspace_catalog.clone(),
        lane_catalog.clone(),
    ));
    let lane_catalog_arc = Arc::new(lane_catalog);
    let dataspace_catalog_arc = Arc::new(dataspace_catalog);
    let queue = Arc::new(Queue::from_config_with_router_limits_and_catalogs(
        queue_cfg,
        events_sender,
        router,
        queue_limits,
        &lane_catalog_arc,
        &dataspace_catalog_arc,
        None,
    ));

    let time_source = TimeSource::new_system();

    let instr_lane0 = InstructionBox::from(SetKeyValue::account(
        lane0_account.clone(),
        "k0".parse().unwrap(),
        Json::new("v0"),
    ));
    let instr_lane1_a = InstructionBox::from(SetKeyValue::account(
        lane1_account.clone(),
        "k1a".parse().unwrap(),
        Json::new("v1"),
    ));
    let instr_lane1_b = InstructionBox::from(SetKeyValue::account(
        lane1_account.clone(),
        "k1b".parse().unwrap(),
        Json::new("v2"),
    ));

    let teu_lane0 = gas::meter_instructions(std::slice::from_ref(&instr_lane0));
    let teu_lane1 = gas::meter_instructions(&[instr_lane1_a.clone(), instr_lane1_b.clone()]);

    let tx_lane0 = build_transaction(
        &chain_id,
        &lane0_account,
        &lane0_keypair,
        &time_source,
        vec![instr_lane0.clone()],
    );
    let tx_lane1 = build_transaction(
        &chain_id,
        &lane1_account,
        &lane1_keypair,
        &time_source,
        vec![instr_lane1_a.clone(), instr_lane1_b.clone()],
    );

    queue
        .push(tx_lane0, state.view())
        .expect("lane0 transaction accepted");
    queue
        .push(tx_lane1, state.view())
        .expect("lane1 transaction accepted");

    assert_eq!(queue.queued_len(), 2, "both transactions should be queued");

    let lane0_label = LaneId::new(0).as_u32().to_string();
    let lane1_label = LaneId::new(1).as_u32().to_string();
    let dataspace0_label = DataSpaceId::GLOBAL.as_u64().to_string();
    let dataspace1_label = DataSpaceId::new(1).as_u64().to_string();

    let backlog_lane0 = metrics
        .nexus_scheduler_dataspace_teu_backlog
        .with_label_values(&[lane0_label.as_str(), dataspace0_label.as_str()])
        .get();
    let backlog_lane1 = metrics
        .nexus_scheduler_dataspace_teu_backlog
        .with_label_values(&[lane1_label.as_str(), dataspace1_label.as_str()])
        .get();

    assert_eq!(backlog_lane0, teu_lane0);
    assert_eq!(backlog_lane1, teu_lane1);

    let capacity_lane0 = metrics
        .nexus_scheduler_lane_teu_capacity
        .with_label_values(&[lane0_label.as_str()])
        .get();
    let capacity_lane1 = metrics
        .nexus_scheduler_lane_teu_capacity
        .with_label_values(&[lane1_label.as_str()])
        .get();
    assert_eq!(capacity_lane0, 900);
    assert_eq!(capacity_lane1, 1_500);

    let mut guards = Vec::new();
    let max = NonZeroUsize::new(2).expect("non-zero");
    queue.get_transactions_for_block(&state.view(), max, &mut guards);

    let mut lanes_seen = BTreeSet::new();
    let mut dataspaces_seen = BTreeSet::new();
    for guard in &guards {
        let decision = guard.routing();
        lanes_seen.insert(decision.lane_id);
        dataspaces_seen.insert(decision.dataspace_id);
    }
    assert!(lanes_seen.contains(&LaneId::new(0)));
    assert!(lanes_seen.contains(&LaneId::new(1)));
    assert!(dataspaces_seen.contains(&DataSpaceId::GLOBAL));
    assert!(dataspaces_seen.contains(&DataSpaceId::new(1)));

    drop(guards);

    assert_eq!(
        queue.queued_len(),
        0,
        "queue should drain after guards drop"
    );

    let backlog_lane0_after = metrics
        .nexus_scheduler_dataspace_teu_backlog
        .with_label_values(&[lane0_label.as_str(), dataspace0_label.as_str()])
        .get();
    let backlog_lane1_after = metrics
        .nexus_scheduler_dataspace_teu_backlog
        .with_label_values(&[lane1_label.as_str(), dataspace1_label.as_str()])
        .get();
    assert_eq!(backlog_lane0_after, 0);
    assert_eq!(backlog_lane1_after, 0);

    Ok(())
}

#[test]
#[allow(clippy::too_many_lines, clippy::unnecessary_wraps)]
fn queue_uses_default_lane_when_no_rule_matches() -> Result<()> {
    let chain_id = ChainId::from(TEST_CHAIN_ID);
    let (fallback_account, fallback_keypair) = gen_account_in("fallback");
    let (routed_account, routed_keypair) = gen_account_in("routed");
    let fallback_domain_id: DomainId = "fallback".parse().unwrap();
    let routed_domain_id: DomainId = "routed".parse().unwrap();

    let domain_fallback: Domain = Domain::new(fallback_domain_id.clone()).build(&fallback_account);
    let domain_routed: Domain = Domain::new(routed_domain_id.clone()).build(&routed_account);
    let account_fallback =
        Account::new(fallback_account.clone())
            .build(&fallback_account);
    let account_routed = Account::new(routed_account.clone())
        .build(&routed_account);
    let world = World::with(
        [domain_fallback, domain_routed],
        [account_fallback, account_routed],
        Vec::<AssetDefinition>::new(),
    );

    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let query_store = iroha_core::query::store::LiveQueryStore::start_test();
    let metrics = Arc::new(Metrics::default());
    let telemetry = StateTelemetry::new(Arc::clone(&metrics), true);

    // Lane 0 has a dedicated dataspace and explicit routing rule, lane 1 acts as the default.
    let mut lane0_metadata = BTreeMap::new();
    lane0_metadata.insert("scheduler.teu_capacity".to_string(), "1200".to_string());
    lane0_metadata.insert(
        "scheduler.starvation_bound_slots".to_string(),
        "5".to_string(),
    );
    let lane0 = LaneConfig {
        id: LaneId::new(0),
        dataspace_id: DataSpaceId::GLOBAL,
        alias: "rule-lane".to_string(),
        description: Some("Routed lane".to_string()),
        visibility: LaneVisibility::Public,
        lane_type: Some("governance".to_string()),
        governance: None,
        settlement: Some("xor".to_string()),
        metadata: lane0_metadata,
        ..LaneConfig::default()
    };

    let mut lane1_metadata = BTreeMap::new();
    lane1_metadata.insert("scheduler.teu_capacity".to_string(), "900".to_string());
    lane1_metadata.insert(
        "scheduler.starvation_bound_slots".to_string(),
        "4".to_string(),
    );
    let lane1 = LaneConfig {
        id: LaneId::new(1),
        dataspace_id: DataSpaceId::new(1),
        alias: "default-lane".to_string(),
        description: Some("Fallback lane".to_string()),
        visibility: LaneVisibility::Public,
        lane_type: Some("core".to_string()),
        governance: None,
        settlement: Some("xor".to_string()),
        metadata: lane1_metadata,
        ..LaneConfig::default()
    };

    let lane_catalog = LaneCatalog::new(nonzero!(2_u32), vec![lane0, lane1]).expect("catalog");
    let dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: DataSpaceId::new(1),
            alias: "fallback".to_string(),
            description: Some("Default dataspace".to_string()),
            fault_tolerance: 1,
        },
    ])
    .expect("dataspace catalog");

    let routing_policy = LaneRoutingPolicy {
        default_lane: LaneId::new(1),
        default_dataspace: DataSpaceId::new(1),
        rules: vec![LaneRoutingRule {
            lane: LaneId::new(0),
            dataspace: Some(DataSpaceId::GLOBAL),
            matcher: LaneRoutingMatcher {
                account: Some(routed_account.to_string()),
                instruction: None,
                description: Some("route routed_account to explicit lane".to_string()),
            },
        }],
    };

    let nexus = Nexus {
        enabled: true,
        lane_catalog: lane_catalog.clone(),
        dataspace_catalog: dataspace_catalog.clone(),
        routing_policy: routing_policy.clone(),
        ..Nexus::default()
    };

    let queue_limits = QueueLimits::from_nexus(&nexus);

    let mut state_inner = State::with_telemetry(world, kura, query_store, telemetry);
    state_inner
        .set_nexus(nexus.clone())
        .expect("apply Nexus config for per-account routing test");
    let state = Arc::new(state_inner);

    let (events_sender, _) = broadcast::channel(16);
    let queue_cfg = QueueConfig::default();
    let router: Arc<dyn LaneRouter> = Arc::new(ConfigLaneRouter::new(
        routing_policy,
        dataspace_catalog.clone(),
        lane_catalog.clone(),
    ));
    let lane_catalog_arc = Arc::new(lane_catalog);
    let dataspace_catalog_arc = Arc::new(dataspace_catalog);
    let queue = Arc::new(Queue::from_config_with_router_limits_and_catalogs(
        queue_cfg,
        events_sender,
        router,
        queue_limits,
        &lane_catalog_arc,
        &dataspace_catalog_arc,
        None,
    ));

    let time_source = TimeSource::new_system();
    let routed_instr = InstructionBox::from(SetKeyValue::account(
        routed_account.clone(),
        "key_routed".parse().unwrap(),
        Json::new("value_routed"),
    ));
    let fallback_instr = InstructionBox::from(SetKeyValue::account(
        fallback_account.clone(),
        "key_fallback".parse().unwrap(),
        Json::new("value_fallback"),
    ));

    let routed_teu = gas::meter_instructions(std::slice::from_ref(&routed_instr));
    let fallback_teu = gas::meter_instructions(std::slice::from_ref(&fallback_instr));

    let tx_routed = build_transaction(
        &chain_id,
        &routed_account,
        &routed_keypair,
        &time_source,
        vec![routed_instr.clone()],
    );
    let tx_fallback = build_transaction(
        &chain_id,
        &fallback_account,
        &fallback_keypair,
        &time_source,
        vec![fallback_instr.clone()],
    );

    queue
        .push(tx_routed, state.view())
        .expect("routed transaction accepted");
    queue
        .push(tx_fallback, state.view())
        .expect("fallback transaction accepted");
    assert_eq!(queue.queued_len(), 2, "both transactions should be queued");

    let lane0_label = LaneId::new(0).as_u32().to_string();
    let lane1_label = LaneId::new(1).as_u32().to_string();
    let dataspace0_label = DataSpaceId::GLOBAL.as_u64().to_string();
    let dataspace1_label = DataSpaceId::new(1).as_u64().to_string();

    let backlog_lane0 = metrics
        .nexus_scheduler_dataspace_teu_backlog
        .with_label_values(&[lane0_label.as_str(), dataspace0_label.as_str()])
        .get();
    let backlog_lane1 = metrics
        .nexus_scheduler_dataspace_teu_backlog
        .with_label_values(&[lane1_label.as_str(), dataspace1_label.as_str()])
        .get();
    assert_eq!(backlog_lane0, routed_teu);
    assert_eq!(backlog_lane1, fallback_teu);

    let mut guards = Vec::new();
    let max = NonZeroUsize::new(2).expect("non-zero");
    queue.get_transactions_for_block(&state.view(), max, &mut guards);

    let mut lanes_seen = BTreeSet::new();
    let mut dataspaces_seen = BTreeSet::new();
    for guard in &guards {
        let decision = guard.routing();
        lanes_seen.insert(decision.lane_id);
        dataspaces_seen.insert(decision.dataspace_id);
    }
    assert!(
        lanes_seen.contains(&LaneId::new(1)),
        "unmatched transaction should use default lane"
    );
    assert!(
        lanes_seen.contains(&LaneId::new(0)),
        "explicit rule should still route to configured lane"
    );
    assert!(
        dataspaces_seen.contains(&DataSpaceId::new(1)),
        "default dataspace should be used for fallback"
    );
    assert!(
        dataspaces_seen.contains(&DataSpaceId::GLOBAL),
        "explicit rule should keep its dataspace"
    );

    drop(guards);
    assert_eq!(queue.queued_len(), 0, "queue should drain after processing");

    let backlog_lane0_after = metrics
        .nexus_scheduler_dataspace_teu_backlog
        .with_label_values(&[lane0_label.as_str(), dataspace0_label.as_str()])
        .get();
    let backlog_lane1_after = metrics
        .nexus_scheduler_dataspace_teu_backlog
        .with_label_values(&[lane1_label.as_str(), dataspace1_label.as_str()])
        .get();
    assert_eq!(backlog_lane0_after, 0);
    assert_eq!(backlog_lane1_after, 0);

    Ok(())
}
