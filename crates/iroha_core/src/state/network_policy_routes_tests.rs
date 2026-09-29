//! Native/physical separation and original pre-effect source-custody controls.

use super::*;
use crate::{governance::manifest::LaneManifestRegistry, query::store::LiveQueryStore};
use iroha_data_model::{
    Registrable,
    account::Account,
    block::{BlockExecutionContextBundle, ExternalExecutionContext, builder::BlockBuilder},
    isi::Log,
    parameter::Parameter,
    sumeragi_lanes::{
        SumeragiFixedLane, SumeragiLaneFrontier, SumeragiLaneMergeSection, SumeragiLanePolicy,
        SumeragiLaneRecord, SumeragiLaneRoute,
    },
    transaction::{FeePaymentIntent, SealedTransactionReveal, TransactionBuilder},
};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
use std::sync::Arc;

fn state() -> State {
    let mut world = World::with([], [Account::new(ALICE_ID.clone()).build(&ALICE_ID)], []);
    let mut committee = crate::sumeragi::test_chain::fixture_validators().into_iter()
        .map(|(peer, pop)| iroha_data_model::sumeragi_lanes::SumeragiLaneMember { peer, pop })
        .collect::<Vec<_>>();
    committee.sort();
    let policy = SumeragiLanePolicy {
        anchor_freshness: 16,
        max_merge_blocks: 32,
        stall_window: 256,
        lane_params: Default::default(),
        fixed: vec![SumeragiFixedLane {
            lane: LaneId::new(1),
            dataspace: DataSpaceId::UNIVERSAL,
            committee: committee.clone(),
        }],
        routes: vec![SumeragiLaneRoute {
            lane: LaneId::new(1),
            account: Some(ALICE_ID.to_string()),
            instruction: None,
        }],
        autoscale: None,
    };
    policy.validate().expect("valid native policy fixture");
    // Component setup supplies committed native policy/state, not a certificate substitute.
    world
        .parameters
        .get_mut()
        .set_parameter(Parameter::Custom(policy.into_custom_parameter()));
    world.sumeragi_lanes.get_mut().upsert(SumeragiLaneRecord {
        lane: LaneId::new(1),
        dataspace: DataSpaceId::UNIVERSAL,
        incarnation: Hash::new(b"native ordering identity").into(),
        params: Default::default(),
        committee,
        created_at: 1,
        active_from: 3,
        closing: None,
        anchor_freshness: 16,
        merged: SumeragiLaneFrontier::default(),
        merged_at: 3,
        rescued: 0,
    });
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let nexus = state.nexus_snapshot();
    let registry =
        LaneManifestRegistry::from_config(&nexus.lane_catalog, &nexus.governance, &nexus.registry);
    registry
        .validate_materialized_authority_for_catalog(&nexus.lane_catalog, &nexus.governance)
        .unwrap();
    state.install_lane_manifests_for_testing(&Arc::new(registry));
    state
}

fn input(state: &State, label: &str) -> TransactionEntrypoint {
    TransactionBuilder::new(
        state.network_id,
        ALICE_ID.clone(),
        FeePaymentIntent::authority(vec![], None),
    )
    .with_instructions([Log::new(iroha_data_model::Level::INFO, label.to_owned())])
    .sign(ALICE_KEYPAIR.private_key())
    .into()
}

fn source(
    inputs: Vec<TransactionEntrypoint>,
    height: u64,
    contexts: Option<Vec<ExternalExecutionContext>>,
) -> SignedBlock {
    let mut builder = BlockBuilder::new(BlockHeader::new(
        height.try_into().unwrap(),
        None,
        None,
        1000,
        0,
    ));
    for input in inputs {
        match input {
            TransactionEntrypoint::External(signed) => {
                builder.push_transaction(signed);
            }
            TransactionEntrypoint::SealedReveal(reveal) => {
                builder.push_sealed_transaction_reveal(reveal);
            }
            TransactionEntrypoint::SealedCommitment(commitment) => {
                builder.push_sealed_transaction_commitment(commitment);
            }
        }
    }
    builder.set_execution_context(contexts.map(BlockExecutionContextBundle::new));
    builder.build_with_signature(0, ALICE_KEYPAIR.private_key())
}

fn carrier(inputs: Vec<TransactionEntrypoint>) -> SignedBlock {
    let contexts = inputs
        .iter()
        .map(|input| {
            ExternalExecutionContext::new(input.hash(), LaneId::new(1), DataSpaceId::UNIVERSAL)
        })
        .collect();
    source(inputs, 4, Some(contexts))
}

fn before_effects(
    state: &State,
    source: &SignedBlock,
    replacement: bool,
    check: impl FnOnce(&mut StateBlock<'_>),
) {
    let stage = |block: &mut StateBlock<'_>| {
        assert!(!block.start_of_block_effects_applied);
        check(block);
        Err::<(), _>("examined pristine owner")
    };
    let result = if replacement {
        state
            .block_and_revert_with_pristine_carrier_stage(source, stage)
            .map(|_| ())
    } else {
        state
            .block_with_owned_start_stages_with_carrier(
                source.header(),
                Some(source),
                stage,
                |_, ()| Ok(()),
            )
            .map(|_| ())
    };
    assert!(matches!(
        result,
        Err(StateBlockStartError::Stage("examined pristine owner"))
    ));
}

#[test]
fn native_ordering_lane_uses_independently_resolved_physical_policy() {
    let state = state();
    let source = carrier(vec![input(&state, "independent namespaces")]);
    before_effects(&state, &source, false, |block| {
        let (native, token) = block
            .network_policy_routes
            .as_ref()
            .unwrap()
            .get(&source, 0)
            .unwrap();
        assert_eq!(
            native,
            RoutingDecision::new(LaneId::new(1), DataSpaceId::UNIVERSAL)
        );
        assert_eq!(
            token.physical().unwrap().decision(),
            RoutingDecision::default()
        );
        let TransactionEntrypoint::External(signed) = source.network_entrypoint_at(0).unwrap()
        else {
            unreachable!()
        };
        let mut tx = block.transaction();
        StateBlock::validate_stateful_admission(signed, &mut tx, native, token).unwrap();
        assert_eq!(tx.current_lane_id, Some(LaneId::new(1)));
        assert_eq!(tx.current_dataspace_id, Some(DataSpaceId::UNIVERSAL));
    });
}

#[test]
fn normal_and_replacement_routes_are_frozen_before_every_prefix_hook() {
    for replacement in [false, true] {
        let state = state();
        let source = carrier(vec![input(&state, "first"), input(&state, "second")]);
        before_effects(&state, &source, replacement, |block| {
            let first = block
                .network_policy_routes
                .as_ref()
                .unwrap()
                .get(&source, 1)
                .unwrap();
            block.nexus.routing_policy.default_lane = LaneId::new(99);
            block
                .world
                .sumeragi_lanes
                .get_mut()
                .lane_mut(LaneId::new(1))
                .unwrap()
                .closing = Some(3);
            let second = block
                .network_policy_routes
                .as_ref()
                .unwrap()
                .get(&source, 1)
                .unwrap();
            assert_eq!(first.0, second.0);
            assert_eq!(first.1.physical(), second.1.physical());
            assert_eq!(
                second.1.physical().unwrap().decision().lane_id,
                LaneId::SINGLE
            );
            let accepted = AcceptedTransaction::new_unchecked_entrypoint(Cow::Borrowed(
                source.network_entrypoint_at(1).unwrap(),
            ));
            assert!(
                PhysicalExecutionPolicyRoute::resolve(&block.nexus, &block.world, &accepted, 3, 0)
                    .is_err()
            );
        });
    }
}

#[test]
fn source_owner_rejects_missing_context_and_every_noncanonical_native_field() {
    let state = state();
    let entry = input(&state, "context");
    let valid = ExternalExecutionContext::new(entry.hash(), LaneId::new(1), DataSpaceId::UNIVERSAL);
    for mutation in 0..7 {
        let mut changed = valid.clone();
        match mutation {
            0 => {}
            1 => changed.entrypoint_hash = HashOf::from_untyped_unchecked(Hash::new(b"foreign")),
            2 => changed.lane_id = LaneId::new(7),
            3 => changed.dataspace_id = DataSpaceId::new(9),
            4 => changed.routing_plan_digest = Hash::new(b"foreign plan"),
            5 => changed.routing_plan_legs.clear(),
            6 => changed.routing_plan_legs[0].role = ExternalExecutionRouteRole::Participant,
            _ => unreachable!(),
        }
        let source = source(
            vec![entry.clone()],
            4,
            if mutation == 0 {
                None
            } else {
                Some(vec![changed])
            },
        );
        before_effects(&state, &source, false, |block| {
            assert!(
                block
                    .network_policy_routes
                    .as_ref()
                    .unwrap()
                    .get(&source, 0)
                    .is_err()
            );
        });
    }
    let source = source(vec![entry], 4, Some(vec![valid]));
    before_effects(&state, &source, false, |block| {
        assert!(
            block
                .network_policy_routes
                .as_ref()
                .unwrap()
                .get(&source, 0)
                .is_ok()
        )
    });
}

#[test]
fn native_activation_and_closure_are_checked_at_the_exact_predecessor_height() {
    for (height, closing, valid) in [
        (3, None, false),
        (4, None, true),
        (4, Some(3), false),
        (4, Some(4), true),
    ] {
        let mut state = state();
        state
            .world
            .sumeragi_lanes
            .get_mut()
            .lane_mut(LaneId::new(1))
            .unwrap()
            .closing = closing;
        let entry = input(&state, "native activation");
        let source = source(
            vec![entry.clone()],
            height,
            Some(vec![ExternalExecutionContext::new(
                entry.hash(),
                LaneId::new(1),
                DataSpaceId::UNIVERSAL,
            )]),
        );
        before_effects(&state, &source, false, |block| {
            assert_eq!(
                block
                    .network_policy_routes
                    .as_ref()
                    .unwrap()
                    .get(&source, 0)
                    .is_ok(),
                valid
            )
        });
    }
}

#[test]
fn physical_dataspace_mismatch_is_a_transaction_refusal_not_a_carrier_failure() {
    let mut state = state();
    state
        .world
        .sumeragi_lanes
        .get_mut()
        .lane_mut(LaneId::new(1))
        .unwrap()
        .dataspace = DataSpaceId::new(7);
    let entry = input(&state, "dataspace mismatch");
    let source = source(
        vec![entry.clone()],
        4,
        Some(vec![ExternalExecutionContext::new(
            entry.hash(),
            LaneId::new(1),
            DataSpaceId::new(7),
        )]),
    );
    before_effects(&state, &source, false, |block| {
        let (native, token) = block
            .network_policy_routes
            .as_ref()
            .unwrap()
            .get(&source, 0)
            .unwrap();
        assert!(matches!(
            token.projection,
            PolicyProjection::Signed(Err(PhysicalPolicyRouteRejection::DataspaceMismatch))
        ));
        let TransactionEntrypoint::External(signed) = &entry else {
            unreachable!()
        };
        let tx = block.transaction();
        assert!(matches!(
            token.for_signed(signed, &tx, native),
            Err(TransactionRejectionReason::Validation(_))
        ));
    });
}

#[test]
fn missing_physical_route_is_retained_as_bounded_ordinary_rejection() {
    let mut state = state();
    state.nexus.get_mut().routing_policy.default_lane = LaneId::new(99);
    let source = carrier(vec![input(&state, "missing physical lane")]);
    before_effects(&state, &source, false, |block| {
        let (_, token) = block
            .network_policy_routes
            .as_ref()
            .unwrap()
            .get(&source, 0)
            .unwrap();
        assert!(matches!(token.projection, PolicyProjection::Signed(Err(_))));
        assert!(!std::mem::needs_drop::<SourceRow>());
    });
}

#[test]
fn original_source_hash_order_and_carrier_are_required() {
    let state = state();
    let a = input(&state, "a");
    let b = input(&state, "b");
    let source = carrier(vec![a.clone(), b.clone()]);
    let foreign = carrier(vec![b, a]);
    before_effects(&state, &source, false, |block| {
        let owner = block.network_policy_routes.as_ref().unwrap();
        assert!(owner.get(&source, 0).is_ok());
        assert!(owner.get(&source, 1).is_ok());
        assert!(owner.get(&source, 2).is_err());
        assert!(owner.get(&foreign, 0).is_err());
        assert!(owner.get(&foreign, 1).is_err());
    });
}

#[test]
fn actual_merged_suffix_receives_original_ordinal_and_policy_rows() {
    let state = state();
    let a = input(&state, "ordinary");
    let b = input(&state, "merged");
    let mut builder =
        BlockBuilder::new(BlockHeader::new(4_u64.try_into().unwrap(), None, None, 1000, 0));
    let TransactionEntrypoint::External(signed) = a.clone() else {
        unreachable!()
    };
    builder.push_transaction(signed);
    let mut context = BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
        a.hash(),
        LaneId::new(1),
        DataSpaceId::UNIVERSAL,
    )]);
    context.lane_merge = Some(SumeragiLaneMergeSection {
        merges: vec![],
        time_floor_ms: 0,
        merged_count: 0,
    });
    builder.set_execution_context(Some(context));
    let source = builder
        .build_with_signature(0, ALICE_KEYPAIR.private_key())
        .with_merged_entrypoints(
            vec![b.clone()],
            vec![ExternalExecutionContext::new(
                b.hash(),
                LaneId::new(1),
                DataSpaceId::UNIVERSAL,
            )],
        )
        .unwrap();
    assert_eq!(source.merged_entrypoint_count(), 1);
    before_effects(&state, &source, false, |block| {
        let owner = block.network_policy_routes.as_ref().unwrap();
        assert_eq!(owner.rows.as_slice().len(), 2);
        assert_eq!(owner.rows.as_slice()[0].entrypoint, a.hash());
        assert_eq!(owner.rows.as_slice()[1].entrypoint, b.hash());
        assert_eq!(
            owner
                .get(&source, 1)
                .unwrap()
                .1
                .physical()
                .unwrap()
                .decision(),
            RoutingDecision::default()
        );
    });
}

#[test]
fn sealed_reveal_policy_uses_inner_fields_but_retains_outer_source_binding() {
    let state = state();
    let TransactionEntrypoint::External(signed) = input(&state, "sealed inner") else {
        unreachable!()
    };
    let reveal = TransactionEntrypoint::SealedReveal(SealedTransactionReveal::new(
        Hash::new(b"commitment"),
        signed.clone(),
        [7; 32],
    ));
    let source = carrier(vec![reveal.clone()]);
    before_effects(&state, &source, false, |block| {
        let owner = block.network_policy_routes.as_ref().unwrap();
        assert_eq!(owner.rows.as_slice()[0].entrypoint, reveal.hash());
        let (native, token) = owner.get(&source, 0).unwrap();
        assert_eq!(token.signed_hash, Some(signed.hash()));
        block.nexus.routing_policy.default_lane = LaneId::new(99);
        let tx = block.transaction();
        assert_eq!(
            token.for_signed(&signed, &tx, native).unwrap().decision(),
            RoutingDecision::default()
        );
        let TransactionEntrypoint::External(foreign) = input(&state, "foreign") else {
            unreachable!()
        };
        assert!(token.for_signed(&foreign, &tx, native).is_err());
        assert!(
            token
                .for_signed(&signed, &tx, RoutingDecision::default())
                .is_err()
        );
    });
}

#[test]
fn policy_rows_remain_charged_until_the_original_block_owner_drops() {
    let state = state();
    let source = carrier(vec![input(&state, "charge a"), input(&state, "charge b")]);
    let bytes = std::alloc::Layout::array::<SourceRow>(2).unwrap().size();
    let budget = AllocationBudget::new(bytes);
    let owner = CapturedNetworkPolicyRoutes::reserve(&source, &budget).unwrap();
    assert_eq!(budget.reserved_bytes(), bytes);
    assert!(CapturedNetworkPolicyRoutes::reserve(&source, &budget).is_err());
    drop(owner);
    assert_eq!(budget.reserved_bytes(), 0);
    let refused = CapturedNetworkPolicyRoutes::reserve(&source, &AllocationBudget::new(bytes - 1));
    assert!(refused.is_err());
}

#[test]
fn only_genesis_carries_explicit_bootstrap_policy_and_normal_scope_has_no_fallback() {
    let state = state();
    let entry = input(&state, "bootstrap");
    let genesis = source(vec![entry.clone()], 1, None);
    before_effects(&state, &genesis, false, |block| {
        let (native, token) = block
            .network_policy_routes
            .as_ref()
            .unwrap()
            .get(&genesis, 0)
            .unwrap();
        assert_eq!(native, RoutingDecision::default());
        assert!(matches!(token.projection, PolicyProjection::Genesis));
    });
    let absent = state.block(carrier(vec![entry]).header());
    assert!(absent.network_policy_routes.is_none());
}
