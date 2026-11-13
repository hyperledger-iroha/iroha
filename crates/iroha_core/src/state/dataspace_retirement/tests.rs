//! Owner-signed retirement, closure prerequisites, reopen guards, routing and inertness.

use super::*;
use crate::{
    query::store::LiveQueryStore,
    smartcontracts::{
        Execute,
        isi::sccp::test_support::{authority, header},
    },
    state::{State, StateBlock, World},
    sumeragi::lanes::routing::{RoutingInputs, test_support},
    tx::AcceptedTransaction,
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    NetworkId, Registrable,
    account::{Account, AccountAddress},
    asset::{AssetBalancePolicy, AssetDefinition, AssetDefinitionId},
    domain::Domain,
    isi::{Register, SetParameter},
    nexus::{DataSpaceCatalog, DataSpaceMetadata, LaneCatalog, LaneConfig},
    parameter::system::SumeragiParameters,
    smart_contract::ContractAddress,
    sns::{NameControllerV1, NameRecordV1},
    sumeragi_lanes::{
        SumeragiFixedLane, SumeragiLaneAutoscale, SumeragiLaneFrontier, SumeragiLaneMember,
        SumeragiLaneRecord, SumeragiLaneRoute, SumeragiLaneState,
    },
    transaction::{
        Executable, FeePaymentIntent, TransactionBuilder, executable::ContractInvocation,
    },
};
use iroha_model_base::{domain::DomainId, metadata::Metadata, peer::PeerId};
use norito::codec::Encode;
use std::num::NonZeroU32;

const ALIAS: &str = "acme";
const LANE: LaneId = LaneId::new(1);

fn acme() -> DataSpaceId {
    crate::sns::dataspace_id_for_sns_alias(ALIAS).expect("hash-derived fixture identity")
}

fn lease(owner: &AccountId, generation: u64) -> NameRecordV1 {
    NameRecordV1 {
        ownership_generation: generation,
        ..NameRecordV1::new(
            crate::sns::selector_for_dataspace_alias(ALIAS).unwrap(),
            owner.clone(),
            vec![NameControllerV1::account(
                &AccountAddress::from_account_id(owner).unwrap(),
            )],
            0,
            0,
            100_000,
            200_000,
            300_000,
            Metadata::default(),
        )
    }
}

/// Parent catalog: universal lane 0 plus the catalogued restricted `acme` dataspace on lane 1.
pub(crate) fn nexus() -> iroha_config::parameters::actual::Nexus {
    nexus_with(LaneVisibility::Restricted)
}

fn nexus_with(visibility: LaneVisibility) -> iroha_config::parameters::actual::Nexus {
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.autoscale.enabled = false;
    nexus.dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: acme(),
            alias: ALIAS.into(),
            description: None,
            fault_tolerance: 1,
        },
    ])
    .unwrap();
    nexus.lane_catalog = LaneCatalog::new(
        NonZeroU32::new(2).unwrap(),
        vec![
            LaneConfig::default(),
            LaneConfig {
                id: LANE,
                alias: "acme-lane".into(),
                dataspace_id: acme(),
                visibility,
                ..LaneConfig::default()
            },
        ],
    )
    .unwrap();
    nexus
}

fn committee() -> Vec<SumeragiLaneMember> {
    let mut committee: Vec<_> = (1_u8..=4)
        .map(|seed| {
            let pair = KeyPair::from_seed(vec![seed + 40; 32], Algorithm::BlsNormal);
            SumeragiLaneMember {
                peer: PeerId::new(pair.public_key().clone()),
                pop: iroha_crypto::bls_normal_pop_prove(pair.private_key()).expect("pop"),
            }
        })
        .collect();
    committee.sort_by(|left, right| left.peer.cmp(&right.peer));
    committee
}

fn lane_policy(fixed: Vec<SumeragiFixedLane>) -> SumeragiLanePolicy {
    SumeragiLanePolicy {
        da_layout: iroha_sumeragi::availability::recommended_data_availability_layout(),
        anchor_freshness: 16,
        max_merge_blocks: 16,
        stall_window: 64,
        lane_params: SumeragiParameters::default(),
        fixed,
        routes: Vec::new(),
        autoscale: None,
    }
}

fn acme_fixed_lane(committee: Vec<SumeragiLaneMember>) -> SumeragiFixedLane {
    SumeragiFixedLane {
        lane: LANE,
        dataspace: acme(),
        committee,
    }
}

fn record(retirement_height: u64) -> DataSpaceRetirementsV1 {
    DataSpaceRetirementsV1::default()
        .with_retirement(RetiredDataSpaceV1 {
            dataspace_id: acme(),
            alias: ALIAS.into(),
            owner: authority(1),
            ownership_generation: 1,
            lanes: vec![LANE],
            retirement_height,
        })
        .unwrap()
}

/// Parent world: Global root, the owner's active `acme` lease and optional extra parameters.
fn world(parameters: Vec<Parameter>) -> World {
    let owner = authority(1);
    let other = authority(2);
    let accounts = [
        Account::new(owner.clone()).build(&owner),
        Account::new(other).build(&owner),
    ];
    let mut world = World::with([], accounts, []);
    let lease = lease(&owner, 1);
    world.smart_contract_state_mut_for_testing().insert(
        crate::sns::record_storage_key(&lease.selector),
        lease.encode(),
    );
    {
        let mut cell = crate::state::parameter_cell_block_for_testing(&world)
            .expect("original parameter-only fixture block");
        for parameter in
            std::iter::once(test_support::metadata(SumeragiRootScope::Global)).chain(parameters)
        {
            crate::state::set_parameter_cell_for_testing(
                &mut cell,
                parameter,
                world.sccp_committee_generations.allocation_budget(),
            )
            .expect("valid fixture parameters");
        }
        cell.commit();
    }
    world
}

fn state(world: World) -> State {
    State::new_with_nexus_for_testing(world, nexus(), LiveQueryStore::start_test())
}

fn request(owner: &AccountId) -> DataSpaceRetirementRequestV1 {
    DataSpaceRetirementRequestV1 {
        version: 1,
        dataspace_id: acme(),
        alias: ALIAS.into(),
        owner: owner.clone(),
        expected_ownership_generation: 1,
        lanes: vec![LANE],
    }
}

fn set(request: &DataSpaceRetirementRequestV1) -> SetParameter {
    SetParameter::new(Parameter::Custom(
        request.clone().into_custom_parameter().unwrap(),
    ))
}

fn execute(
    block: &mut StateBlock<'_>,
    signer: &AccountId,
    instruction: impl Execute,
) -> Result<(), InstructionExecutionError> {
    let mut transaction = block.transaction();
    instruction.execute(signer, &mut transaction)?;
    transaction.apply();
    Ok(())
}

/// The full refusal, including the `InvalidParameter` message that `Display` omits.
fn refused(block: &mut StateBlock<'_>, signer: &AccountId, instruction: impl Execute) -> String {
    format!(
        "{:?}",
        execute(block, signer, instruction).expect_err("retirement must be refused")
    )
}

#[test]
fn only_the_exact_sns_owner_retires_and_the_record_is_append_only() {
    let state = state(world(Vec::new()));
    let mut block = state.block(header(2));
    let owner = authority(1);
    for (signer, candidate, message) in [
        (
            authority(2),
            request(&authority(2)),
            "exact current SNS owner",
        ),
        (
            authority(2),
            request(&owner),
            "signed by its exact SNS owner",
        ),
        (
            owner.clone(),
            DataSpaceRetirementRequestV1 {
                expected_ownership_generation: 2,
                ..request(&owner)
            },
            "ownership generation",
        ),
        (
            owner.clone(),
            DataSpaceRetirementRequestV1 {
                lanes: Vec::new(),
                ..request(&owner)
            },
            "every physical lane",
        ),
        (
            owner.clone(),
            DataSpaceRetirementRequestV1 {
                lanes: vec![LaneId::new(2)],
                ..request(&owner)
            },
            "every physical lane",
        ),
        (
            owner.clone(),
            DataSpaceRetirementRequestV1 {
                alias: "other".into(),
                ..request(&owner)
            },
            "catalog descriptor",
        ),
        (
            owner.clone(),
            DataSpaceRetirementRequestV1 {
                dataspace_id: DataSpaceId::new(77),
                ..request(&owner)
            },
            "unknown physical dataspace",
        ),
    ] {
        let error = refused(&mut block, &signer, set(&candidate));
        assert!(error.contains(message), "{message}: {error}");
        assert!(!has_retirements(&block.world));
    }
    // A structurally invalid payload is refused before any ownership read.
    let universal = DataSpaceRetirementRequestV1 {
        dataspace_id: DataSpaceId::UNIVERSAL,
        ..request(&owner)
    };
    let forged = CustomParameter::new(
        DataSpaceRetirementRequestV1::parameter_id(),
        iroha_primitives::json::Json::from_norito_value_ref(
            &norito::json::from_str::<norito::json::Value>(
                &norito::json::to_json(&universal).unwrap(),
            )
            .unwrap(),
        )
        .unwrap(),
    );
    let error = refused(
        &mut block,
        &owner,
        SetParameter::new(Parameter::Custom(forged)),
    );
    assert!(
        error.contains("invalid dataspace retirement payload"),
        "{error}"
    );

    execute(&mut block, &owner, set(&request(&owner))).unwrap();
    assert_eq!(
        retirements(block.world.parameters()).unwrap(),
        Some(record(2))
    );
    assert!(
        !block
            .world
            .parameters()
            .custom()
            .contains_key(&DataSpaceRetirementRequestV1::parameter_id()),
        "the request is consumed, never stored"
    );
    let error = refused(&mut block, &owner, set(&request(&owner)));
    assert!(error.contains("already retired"), "{error}");
    let erase = DataSpaceRetirementsV1::default()
        .into_custom_parameter()
        .unwrap();
    let error = refused(
        &mut block,
        &owner,
        SetParameter::new(Parameter::Custom(erase)),
    );
    assert!(error.contains("typed retirement request"), "{error}");
    assert_eq!(
        retirements(block.world.parameters()).unwrap(),
        Some(record(2))
    );
}

#[test]
fn retirement_waits_for_native_closure_and_drained_value() {
    let owner = authority(1);
    // The native lane policy still binds the dataspace's fixed lane.
    let bound = state(world(vec![Parameter::Custom(
        lane_policy(vec![acme_fixed_lane(committee())]).into_custom_parameter(),
    )]));
    let mut block = bound.block(header(2));
    let error = refused(&mut block, &owner, set(&request(&owner)));
    assert!(error.contains("native lane policy still binds"), "{error}");

    // The policy dropped the lane, but its native record is still closing (c + A + 1).
    let closing = state(world(Vec::new()));
    {
        let mut world = closing.world.block();
        world.sumeragi_lanes.get_mut().upsert(SumeragiLaneRecord {
            da_layout: iroha_sumeragi::availability::recommended_data_availability_layout(),
            lane: LANE,
            dataspace: acme(),
            incarnation: Hash::new(b"closing acme incarnation").into(),
            params: SumeragiParameters::default(),
            committee: committee(),
            created_at: 1,
            active_from: 3,
            closing: Some(5),
            anchor_freshness: 16,
            merged: SumeragiLaneFrontier::default(),
            merged_at: 3,
            rescued: 0,
        });
        world.commit();
    }
    let mut block = closing.block(header(6));
    let error = refused(&mut block, &owner, set(&request(&owner)));
    assert!(error.contains("still active or closing"), "{error}");

    // A public dataspace keeps its parent-replicated execution promise.
    let public = State::new_with_nexus_for_testing(
        world(Vec::new()),
        nexus_with(LaneVisibility::Public),
        LiveQueryStore::start_test(),
    );
    let mut block = public.block(header(2));
    let error = refused(&mut block, &owner, set(&request(&owner)));
    assert!(
        error.contains("only a restricted physical dataspace"),
        "{error}"
    );

    // A restricted asset definition homed in the dataspace would be stranded.
    let mut homed = world(Vec::new());
    let definition = AssetDefinition::numeric(
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("homes", "universal").unwrap(),
            "sbd".parse().unwrap(),
        ),
        "sbd",
        AssetBalancePolicy::DataspaceRestricted,
        None,
    )
    .build(&owner);
    homed
        .insert_direct_asset_definition_with_assets_for_testing(definition, acme(), [])
        .unwrap();
    let homed = state(homed);
    let mut block = homed.block(header(2));
    let error = refused(&mut block, &owner, set(&request(&owner)));
    assert!(error.contains("homes asset definition"), "{error}");

    // An active contract instance addressed in the dataspace would be stranded.
    let contracts = state(world(Vec::new()));
    let mut block = contracts.block(header(2));
    {
        let mut transaction = block.transaction();
        let address = ContractAddress::derive(&transaction.network_id, &owner, 0, acme()).unwrap();
        transaction
            .world
            .contract_instances
            .insert(address, Hash::new(b"acme contract code"));
        transaction.apply();
    }
    let error = refused(&mut block, &owner, set(&request(&owner)));
    assert!(error.contains("active contract instance"), "{error}");
}

#[test]
fn retired_dataspace_cannot_regain_native_physical_or_lifecycle_execution() {
    let open = state(world(Vec::new()));
    let retired = state(world(vec![Parameter::Custom(
        record(2).into_custom_parameter().unwrap(),
    )]));
    let fixed = lane_policy(vec![acme_fixed_lane(committee())]);
    let reused_lane = lane_policy(vec![SumeragiFixedLane {
        lane: LANE,
        dataspace: DataSpaceId::UNIVERSAL,
        committee: committee(),
    }]);
    let mut routed = lane_policy(vec![SumeragiFixedLane {
        lane: LaneId::new(2),
        dataspace: DataSpaceId::UNIVERSAL,
        committee: committee(),
    }]);
    routed.routes.push(SumeragiLaneRoute {
        lane: LANE,
        account: None,
        instruction: Some("Log".into()),
    });
    let mut autoscale = lane_policy(Vec::new());
    autoscale.autoscale = Some(SumeragiLaneAutoscale {
        min_lane: LaneId::new(16),
        max_lane_exclusive: LaneId::new(18),
        dataspace: acme(),
        committee_size: 4,
        per_lane_target_tps: 10,
        window: 4,
        scale_out_permille: 800,
        scale_in_permille: 200,
        cooldown: 3,
    });
    let unrelated = lane_policy(vec![SumeragiFixedLane {
        lane: LaneId::new(2),
        dataspace: DataSpaceId::UNIVERSAL,
        committee: committee(),
    }]);
    let add = |id: u32, dataspace: DataSpaceId| LaneLifecyclePlan {
        additions: vec![LaneConfig {
            id: LaneId::new(id),
            alias: format!("added-{id}"),
            dataspace_id: dataspace,
            ..LaneConfig::default()
        }],
        retire: Vec::new(),
    };
    {
        // Inert before any retirement: no guard reads or refuses anything.
        let view = open.world_view();
        assert!(!has_retirements(&view));
        assert_eq!(retired_dataspace(&view, acme()).unwrap(), None);
        assert!(!routing_retired(&view, acme()).unwrap());
        for policy in [&fixed, &reused_lane, &routed, &autoscale] {
            ensure_native_policy_preserves_retirements(&view, policy).unwrap();
        }
        crate::sumeragi::private_dataspace::ensure_parent_execution_separate(&view, [acme()])
            .unwrap();
        ensure_lane_lifecycle_preserves_retirements(&view, &add(2, acme())).unwrap();
    }
    let view = retired.world_view();
    assert!(routing_retired(&view, acme()).unwrap());
    assert!(!routing_retired(&view, DataSpaceId::UNIVERSAL).unwrap());
    for (policy, message) in [
        (&fixed, "cannot execute retired dataspace"),
        (&autoscale, "cannot execute retired dataspace"),
        (&reused_lane, "cannot reuse lane 1"),
        (&routed, "cannot reuse lane 1"),
    ] {
        let error = ensure_native_policy_preserves_retirements(&view, policy).unwrap_err();
        assert!(error.contains(message), "{message}: {error}");
    }
    ensure_native_policy_preserves_retirements(&view, &unrelated).unwrap();
    let error =
        crate::sumeragi::private_dataspace::ensure_parent_execution_separate(&view, [acme()])
            .unwrap_err();
    assert!(error.contains("retired dataspace"), "{error}");
    crate::sumeragi::private_dataspace::ensure_parent_execution_separate(
        &view,
        [DataSpaceId::UNIVERSAL],
    )
    .unwrap();
    for plan in [add(2, acme()), add(1, DataSpaceId::UNIVERSAL)] {
        assert!(ensure_lane_lifecycle_preserves_retirements(&view, &plan).is_err());
    }
    ensure_lane_lifecycle_preserves_retirements(&view, &add(3, DataSpaceId::UNIVERSAL)).unwrap();
    // The SetParameter path applies the same guard to a submitted lane policy.
    drop(view);
    let mut block = retired.block(header(3));
    let error = refused(
        &mut block,
        &authority(1),
        SetParameter::new(Parameter::Custom(fixed.into_custom_parameter())),
    );
    assert!(error.contains("retired dataspace"), "{error}");
}

#[test]
fn retired_dataspace_has_no_route_and_global_execution_refuses_its_scoped_work() {
    let pair = KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519);
    let signer = AccountId::new(pair.public_key().clone());
    let network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
        Hash::new(b"retirement-routing"),
    ));
    let call = AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(
        TransactionBuilder::new(
            network,
            signer.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::ContractCall(ContractInvocation {
            contract_address: ContractAddress::derive(&network, &signer, 0, acme()).unwrap(),
            expected_code_hash: Hash::new(b"acme contract"),
            entrypoint: "call".into(),
            arguments: None,
        }))
        .sign(pair.private_key()),
    ));
    // Even an (artificially) admitted fixed lane cannot route retired work.
    let mut lanes = SumeragiLaneState::default();
    lanes.upsert(SumeragiLaneRecord {
        da_layout: iroha_sumeragi::availability::recommended_data_availability_layout(),
        lane: LANE,
        dataspace: acme(),
        incarnation: [9; 32],
        params: SumeragiParameters::default(),
        committee: Vec::new(),
        created_at: 1,
        active_from: 3,
        closing: None,
        anchor_freshness: 16,
        merged: SumeragiLaneFrontier::default(),
        merged_at: 3,
        rescued: 0,
    });
    let policy = lane_policy(vec![acme_fixed_lane(Vec::new())]);
    let dataspaces = nexus().dataspace_catalog;
    for (parameters, expected) in [
        (Vec::new(), Some(LANE)),
        (
            vec![Parameter::Custom(
                record(2).into_custom_parameter().unwrap(),
            )],
            None,
        ),
    ] {
        let world = world(parameters);
        let view = world.view();
        let inputs = RoutingInputs {
            root_scope: Some(SumeragiRootScope::Global),
            policy: Some(&policy),
            lanes: &lanes,
            dataspaces: &dataspaces,
            world: &view,
            ledger_time_ms: 0,
        };
        assert_eq!(inputs.route(&call, 5).unwrap(), expected);
    }

    let retired = state(world(vec![Parameter::Custom(
        record(2).into_custom_parameter().unwrap(),
    )]));
    let open = state(world(Vec::new()));
    let scoped: InstructionBox =
        Register::domain(Domain::new(DomainId::try_new("pob", ALIAS).unwrap())).into();
    let universal: InstructionBox =
        Register::domain(Domain::new(DomainId::try_new("pob", "universal").unwrap())).into();
    let control: InstructionBox = SetParameter::new(Parameter::Custom(
        request(&authority(1)).into_custom_parameter().unwrap(),
    ))
    .into();
    {
        let mut block = open.block(header(3));
        let mut transaction = block.transaction();
        for instruction in [&scoped, &universal, &control] {
            ensure_instruction_outside_retired_dataspaces(instruction, &mut transaction).unwrap();
        }
    }
    let mut block = retired.block(header(3));
    let mut transaction = block.transaction();
    let error = format!(
        "{:?}",
        ensure_instruction_outside_retired_dataspaces(&scoped, &mut transaction).unwrap_err()
    );
    assert!(error.contains("retired from parent execution"), "{error}");
    // The full Global instruction gate applies the same refusal.
    assert!(
        crate::executor::root_scope::ensure_instruction_scope(&scoped, &mut transaction).is_err()
    );
    for instruction in [&universal, &control] {
        ensure_instruction_outside_retired_dataspaces(instruction, &mut transaction).unwrap();
        crate::executor::root_scope::ensure_instruction_scope(instruction, &mut transaction)
            .unwrap();
    }
}

#[test]
fn inert_record_checks_make_no_native_allocation_request() {
    let state = state(world(Vec::new()));
    let view = state.world_view();
    let dataspace = acme();
    let policy = lane_policy(vec![acme_fixed_lane(Vec::new())]);
    let plan = LaneLifecyclePlan::default();
    assert_eq!(
        crate::test_allocations::allocations_during(|| {
            assert!(!has_retirements(&view));
            assert!(!routing_retired(&view, dataspace).unwrap());
            assert!(ensure_not_retired(&view, [dataspace]).is_ok());
            assert!(ensure_native_policy_preserves_retirements(&view, &policy).is_ok());
            assert!(ensure_lane_lifecycle_preserves_retirements(&view, &plan).is_ok());
        }),
        0
    );
}
