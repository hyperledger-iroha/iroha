//! Public-boundary regression tests for state-backed native FX routing plans.

use std::{
    collections::BTreeSet,
    num::{NonZeroU32, NonZeroU64},
    time::Duration,
};

use iroha_config::parameters::actual::{LaneRoutingMatcher, LaneRoutingPolicy, LaneRoutingRule};
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    queue::{
        ConfigLaneRouter, LaneRouter, RoutingPlan, evaluate_policy_plan_with_nexus_and_world_at,
    },
    smartcontracts::Execute,
    state::{State, StateReadOnly, World, WorldReadOnly},
    tx::AcceptedTransaction,
};
use iroha_crypto::Hash;
use iroha_data_model::{
    Encode,
    account::AccountAddress,
    isi::{
        settlement::{
            FxCorridorPolicy, FxCorridorSource, SetFxCorridorPolicy, SettleFxCorridor,
            SettlementInstructionBox,
        },
        smart_contract_code::{ActivateContractInstance, RegisterSmartContractBytes},
    },
    nexus::{DataSpaceCatalog, DataSpaceMetadata, LaneCatalog, LaneConfig},
    prelude::*,
    sns::{NameControllerV1, NameRecordV1},
};
use iroha_executor_data_model::permission::settlement::CanManageFxCorridors;
use iroha_primitives::time::TimeSource;
use iroha_test_samples::{
    ALICE_ID, ALICE_KEYPAIR, BOB_ID, CARPENTER_ID, SAMPLE_GENESIS_ACCOUNT_ID,
};

const LEDGER_TIME_MS: u64 = 0;
const SOURCE_DATASPACE: DataSpaceId = DataSpaceId::new(10);
const DESTINATION_DATASPACE: DataSpaceId = DataSpaceId::new(12);
const CONTRACT_DATASPACE: DataSpaceId = DataSpaceId::new(14);
const DEPLOY_POLICY_DATASPACE: DataSpaceId = DataSpaceId::new(16);
const SOURCE_LANE: LaneId = LaneId::new(3);
const DESTINATION_LANE: LaneId = LaneId::new(4);
const CONTRACT_LANE: LaneId = LaneId::new(5);
const DEPLOY_POLICY_LANE: LaneId = LaneId::new(6);

struct Fixture {
    state: State,
    router: ConfigLaneRouter,
    corridor: FxCorridorPolicy,
}

fn lane_catalog() -> LaneCatalog {
    LaneCatalog::new(
        NonZeroU32::new(7).expect("nonzero lane bound"),
        vec![
            LaneConfig::default(),
            LaneConfig {
                id: SOURCE_LANE,
                dataspace_id: SOURCE_DATASPACE,
                alias: "fx-source".to_owned(),
                ..LaneConfig::default()
            },
            LaneConfig {
                id: DESTINATION_LANE,
                dataspace_id: DESTINATION_DATASPACE,
                alias: "fx-destination".to_owned(),
                ..LaneConfig::default()
            },
            LaneConfig {
                id: CONTRACT_LANE,
                dataspace_id: CONTRACT_DATASPACE,
                alias: "contract-instances".to_owned(),
                ..LaneConfig::default()
            },
            LaneConfig {
                id: DEPLOY_POLICY_LANE,
                dataspace_id: DEPLOY_POLICY_DATASPACE,
                alias: "private-deploy-policy".to_owned(),
                ..LaneConfig::default()
            },
        ],
    )
    .expect("valid deterministic lane catalog")
}

fn dataspace_catalog() -> DataSpaceCatalog {
    DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: SOURCE_DATASPACE,
            alias: "cbuae".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
        DataSpaceMetadata {
            id: DESTINATION_DATASPACE,
            alias: "sbp".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
        DataSpaceMetadata {
            id: CONTRACT_DATASPACE,
            alias: "contracts".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
        DataSpaceMetadata {
            id: DEPLOY_POLICY_DATASPACE,
            alias: "private_deploy".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
    ])
    .expect("valid deterministic dataspace catalog")
}

fn routing_policy() -> LaneRoutingPolicy {
    LaneRoutingPolicy {
        default_lane: LaneId::SINGLE,
        default_dataspace: DataSpaceId::UNIVERSAL,
        rules: vec![LaneRoutingRule {
            lane: DEPLOY_POLICY_LANE,
            dataspace: Some(DEPLOY_POLICY_DATASPACE),
            matcher: LaneRoutingMatcher {
                account: None,
                instruction: Some("smartcontract::deploy".to_owned()),
                description: Some("private smart-contract deployment policy".to_owned()),
            },
        }],
    }
}

fn source_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::new(
        DomainId::try_new("cbuae", "universal").expect("source asset domain"),
        "aed".parse().expect("source asset name"),
    )
}

fn destination_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::new(
        DomainId::try_new("sbp", "universal").expect("destination asset domain"),
        "pkr".parse().expect("destination asset name"),
    )
}

fn corridor() -> FxCorridorPolicy {
    FxCorridorPolicy {
        policy_id: "mobile_aed_pkr".parse().expect("FX policy id"),
        revision: 1,
        source_dataspace: SOURCE_DATASPACE,
        source: FxCorridorSource::TransactionAuthority,
        source_asset_definition_id: source_asset_definition_id(),
        source_sink: CARPENTER_ID.clone(),
        destination_dataspace: DESTINATION_DATASPACE,
        destination_reserve: SAMPLE_GENESIS_ACCOUNT_ID.clone(),
        destination_asset_definition_id: destination_asset_definition_id(),
        allowed_destination_alias_domains: BTreeSet::from([
            DomainId::try_new("hbl", "sbp").expect("HBL alias domain"),
            DomainId::try_new("ubl", "sbp").expect("UBL alias domain"),
        ]),
        rate_numerator: 76,
        rate_denominator: 1,
        enabled: true,
    }
}

fn seed_active_sns_dataspace(world: &mut World, alias: &str) {
    let selector =
        iroha_core::sns::selector_for_dataspace_alias(alias).expect("valid SNS dataspace selector");
    let owner_address =
        AccountAddress::from_account_id(&ALICE_ID).expect("canonical account address");
    let record = NameRecordV1::new(
        selector.clone(),
        ALICE_ID.clone(),
        vec![NameControllerV1::account(&owner_address)],
        0,
        LEDGER_TIME_MS,
        10_000,
        20_000,
        30_000,
        Metadata::default(),
    );
    world.smart_contract_state_mut_for_testing().insert(
        iroha_core::sns::record_storage_key(&selector),
        record.encode(),
    );
}

fn fixture(active_sns_alias: Option<&str>) -> Fixture {
    let source_domain = DomainId::try_new("cbuae", "universal").expect("source domain");
    let destination_domain = DomainId::try_new("sbp", "universal").expect("destination domain");
    let mut world = World::with_assets(
        [
            Domain::new(source_domain).build(&ALICE_ID),
            Domain::new(destination_domain).build(&ALICE_ID),
        ],
        [
            Account::new(ALICE_ID.clone()).build(&ALICE_ID),
            Account::new(BOB_ID.clone()).build(&ALICE_ID),
            Account::new(CARPENTER_ID.clone()).build(&ALICE_ID),
            Account::new(SAMPLE_GENESIS_ACCOUNT_ID.clone()).build(&ALICE_ID),
        ],
        [
            AssetDefinition::numeric(source_asset_definition_id())
                .with_balance_scope_policy(AssetBalancePolicy::DataspaceRestricted)
                .build(&ALICE_ID),
            AssetDefinition::numeric(destination_asset_definition_id())
                .with_balance_scope_policy(AssetBalancePolicy::DataspaceRestricted)
                .build(&ALICE_ID),
        ],
        std::iter::empty::<Asset>(),
        std::iter::empty::<Nft>(),
    );
    world.account_permissions_mut_for_testing().insert(
        ALICE_ID.clone(),
        BTreeSet::from([Permission::from(CanManageFxCorridors)]),
    );
    if let Some(alias) = active_sns_alias {
        seed_active_sns_dataspace(&mut world, alias);
    }

    let mut state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let lanes = lane_catalog();
    let dataspaces = dataspace_catalog();
    let policy = routing_policy();
    let mut nexus = state.nexus_snapshot();
    nexus.enabled = true;
    nexus.lane_catalog = lanes.clone();
    nexus.lane_config = iroha_config::parameters::actual::LaneConfig::from_catalog(&lanes);
    nexus.dataspace_catalog = dataspaces.clone();
    nexus.routing_policy = policy.clone();
    state
        .set_nexus(nexus)
        .expect("pre-genesis Nexus configuration must be valid");

    let corridor = corridor();
    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("nonzero block height"),
        None,
        None,
        None,
        LEDGER_TIME_MS,
        0,
    );
    let mut block = state.block(header);
    let mut transaction = block.transaction();
    SetFxCorridorPolicy {
        policy: corridor.clone(),
    }
    .execute(&ALICE_ID, &mut transaction)
    .expect("exact CanManageFxCorridors grant must install the valid policy");
    transaction.apply();
    block.commit().expect("policy setup block must commit");

    Fixture {
        state,
        router: ConfigLaneRouter::new(policy, dataspaces, lanes),
        corridor,
    }
}

fn accepted_transaction(
    state: &State,
    instructions: Vec<InstructionBox>,
) -> AcceptedTransaction<'static> {
    let mut builder = TransactionBuilder::new(
        state.chain_id.clone(),
        ALICE_ID.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(instructions);
    builder.set_creation_time(Duration::from_millis(1));
    let signed = builder.sign(ALICE_KEYPAIR.private_key());
    let view = state.view();
    let max_clock_drift = view.world().parameters().sumeragi().max_clock_drift();
    let transaction_parameters = view.world().parameters().transaction();
    drop(view);
    let crypto = state.crypto.read().clone();
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(2));
    AcceptedTransaction::accept_with_time_source(
        signed,
        &state.chain_id,
        max_clock_drift,
        transaction_parameters,
        crypto.as_ref(),
        &time_source,
    )
    .expect("deterministic transaction must pass stateless admission")
}

fn settlement_instruction(corridor: &FxCorridorPolicy, settlement_id: &str) -> InstructionBox {
    InstructionBox::from(SettlementInstructionBox::SettleFxCorridor(
        SettleFxCorridor {
            policy_id: corridor.policy_id.clone(),
            expected_policy_revision: corridor.revision,
            source_asset_definition_id: corridor.source_asset_definition_id.clone(),
            destination_asset_definition_id: corridor.destination_asset_definition_id.clone(),
            settlement_id: settlement_id.parse().expect("settlement id"),
            recipient: BOB_ID.clone(),
            source_amount: Quantity::from(10_u32),
        },
    ))
}

fn queue_and_block_plans(
    fixture: &Fixture,
    transaction: &AcceptedTransaction<'_>,
) -> (RoutingPlan, RoutingPlan) {
    let view = fixture.state.view();
    let queue_plan = fixture
        .router
        .try_route_plan_with_view(transaction, &view)
        .expect("queue-time FX plan must resolve");
    let block_plan = evaluate_policy_plan_with_nexus_and_world_at(
        view.nexus(),
        transaction,
        view.world(),
        LEDGER_TIME_MS,
    )
    .expect("block-time FX plan must resolve");
    (queue_plan, block_plan)
}

fn participant_routes(plan: &RoutingPlan) -> BTreeSet<(LaneId, DataSpaceId)> {
    let RoutingPlan::NativeAmx(native) = plan else {
        panic!("FX transaction must produce a native AMX plan");
    };
    native
        .participants
        .iter()
        .map(|leg| (leg.route.lane_id, leg.route.dataspace_id))
        .collect()
}

#[test]
fn fx_deployment_preserves_intrinsic_and_private_policy_participants() {
    let fixture = fixture(None);
    let code = vec![0xCA, 0xFE, 0xBA, 0xBE];
    let contract_address = ContractAddress::derive(0, &ALICE_ID, 0, CONTRACT_DATASPACE)
        .expect("deterministic contract address");
    let transaction = accepted_transaction(
        &fixture.state,
        vec![
            InstructionBox::from(RegisterSmartContractBytes {
                code_hash: Hash::new(&code),
                code,
            }),
            InstructionBox::from(ActivateContractInstance {
                contract_address,
                code_hash: Hash::new(b"contract-code"),
            }),
            settlement_instruction(&fixture.corridor, "fx_deploy_boundary"),
        ],
    );

    let (queue_plan, block_plan) = queue_and_block_plans(&fixture, &transaction);
    assert_eq!(queue_plan, block_plan);
    assert_eq!(queue_plan.digest(), block_plan.digest());
    assert_eq!(
        participant_routes(&queue_plan),
        BTreeSet::from([
            (SOURCE_LANE, SOURCE_DATASPACE),
            (DESTINATION_LANE, DESTINATION_DATASPACE),
            (CONTRACT_LANE, CONTRACT_DATASPACE),
            (DEPLOY_POLICY_LANE, DEPLOY_POLICY_DATASPACE),
        ])
    );
}

#[test]
fn fx_state_view_uses_ledger_time_for_active_sns_only_dataspace() {
    const SNS_ALIAS: &str = "alpha";
    let fixture = fixture(Some(SNS_ALIAS));
    let dynamic_dataspace =
        iroha_core::sns::dataspace_id_for_sns_alias(SNS_ALIAS).expect("SNS-only dataspace id");
    let transaction = accepted_transaction(
        &fixture.state,
        vec![
            InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("merchant", SNS_ALIAS).expect("SNS-scoped domain"),
            ))),
            settlement_instruction(&fixture.corridor, "fx_sns_boundary"),
        ],
    );

    let (queue_plan, block_plan) = queue_and_block_plans(&fixture, &transaction);
    assert_eq!(queue_plan, block_plan);
    assert_eq!(queue_plan.digest(), block_plan.digest());
    assert_eq!(
        participant_routes(&queue_plan),
        BTreeSet::from([
            (SOURCE_LANE, SOURCE_DATASPACE),
            (DESTINATION_LANE, DESTINATION_DATASPACE),
            (LaneId::SINGLE, dynamic_dataspace),
        ])
    );
}
