//! Exact SCCP route-governance ISI execution tests.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use std::num::NonZeroU64;

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, StateTransaction, WorldReadOnly},
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    account::Account,
    asset::{AssetDefinition, AssetId},
    block::BlockHeader,
    bridge::{
        BridgeNativeProofBackendV1, SCCP_V1_MAX_LIVE_ROUTES_PER_LANE,
        SCCP_V1_MAX_RETAINED_ROUTES_PER_LANE, SccpGovernedRouteV1, SccpNativeTrustAnchorV1,
        SccpNetworkV1, SccpRouteActivationV1, SccpSourceEmitterV1,
    },
    isi::{
        Grant, Mint, Register,
        bridge::{
            ApplySccpRouteGovernance, SccpAdvanceLaneTrustAnchorV1,
            SccpInitializeLaneTrustAnchorV1, SccpRegisterRouteV1, SccpRouteGovernanceActionV1,
            SccpRouteGovernanceAnchorV1, SccpSetRouteActivationV1, SccpSwitchRouteRevisionV1,
        },
        governance::{EnactSccpRouteGovernance, ProposeSccpRouteGovernance, VotingMode},
    },
    permission::Permission,
};
use iroha_executor_data_model::permission::{
    governance::CanEnactGovernance, sccp::CanProposeSccpRouteGovernance,
};
use iroha_primitives::numeric::NumericSpec;
use iroha_test_samples::ALICE_ID;
use mv::storage::StorageReadOnly;

#[path = "common/world_fixture.rs"]
mod test_world;

fn test_state() -> State {
    State::new_for_testing(
        test_world::world_with_test_accounts(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    )
}

fn test_header() -> BlockHeader {
    BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0)
}

fn staged_route() -> SccpGovernedRouteV1 {
    iroha_sccp::sccp_exact_evm_governed_route_test_fixture_v1(
        SccpNetworkV1::EthereumMainnet,
        SccpRouteActivationV1::Staged,
    )
}

fn staged_solana_route() -> SccpGovernedRouteV1 {
    let mut route = staged_route();
    let iroha_data_model::bridge::SccpDestinationDeploymentV1::Evm(evm_deployment) =
        route.destination
    else {
        unreachable!("exact EVM fixture uses an EVM destination")
    };
    let lane = iroha_data_model::bridge::SccpLaneIdV1 {
        source: SccpNetworkV1::SolanaTestnet,
        target: SccpNetworkV1::SoraTaira,
    };
    let route_id = iroha_sccp::SCCP_TAIRA_SOL_XOR_ROUTE_ID_V1;
    let mut deployment = iroha_data_model::bridge::SccpSolanaDestinationDeploymentV1 {
        token_mint_address: [0x11; 32],
        route_program_id: [0x12; 32],
        route_program_data_address: [0x13; 32],
        route_program_data_slot: 17,
        route_state_account: [0x14; 32],
        route_program_code_hash: [0x15; 32],
        native_verifier_program_id: [0x16; 32],
        native_verifier_program_data_address: [0x17; 32],
        native_verifier_program_data_slot: 18,
        native_verifier_material_account: [0x18; 32],
        native_verifier_program_code_hash: [0x19; 32],
        native_verifier_config_hash: [0x1a; 32],
        verifying_key: evm_deployment.verifying_key,
        verifier_key_hash: evm_deployment.verifier_key_hash,
        outbound_proof_policy: evm_deployment.outbound_proof_policy,
        taira_to_token_multiplier:
            iroha_data_model::bridge::SCCP_V1_TAIRA_TO_SOLANA_TOKEN_MULTIPLIER,
    };
    deployment.native_verifier_config_hash =
        iroha_data_model::bridge::sccp_solana_native_verifier_config_hash_v1(
            lane,
            route_id,
            &route.asset_key,
            route.revision,
            [0x31; 32],
            &deployment,
        )
        .expect("derive exact Solana native-verifier config");
    let destination = iroha_data_model::bridge::SccpDestinationDeploymentV1::Solana(deployment);
    let route_configuration_hash = destination
        .route_configuration_hash(
            lane,
            route_id,
            &route.asset_key,
            route.revision,
            route.settlement.payload_amount_scale,
        )
        .expect("derive exact Solana route configuration");
    route.lane_id = lane;
    route.route_id = route_id.to_owned();
    route.source_identity = iroha_data_model::bridge::SccpSourceIdentityV1 {
        lane,
        emitter: iroha_data_model::bridge::SccpSourceEmitterV1::Solana(
            iroha_data_model::bridge::SccpSolanaSourceEmitterV1 {
                program_id: [0x31; 32],
                program_data_address: [0x32; 32],
                program_data_slot: 19,
                state_account: [0x33; 32],
                program_code_hash: [0x34; 32],
                route_config_hash: route_configuration_hash,
            },
        ),
    };
    route.destination = destination;
    route
        .validate_registration()
        .expect("Solana fixture must remain an exact staged governed route");
    route
}

fn native_anchor() -> SccpNativeTrustAnchorV1 {
    SccpNativeTrustAnchorV1 {
        backend: BridgeNativeProofBackendV1::EthereumBeacon,
        anchor_hash: [0x91; 32],
        checkpoint_height: 1,
    }
}

fn grant_governance_permission(stx: &mut StateTransaction<'_, '_>) {
    for permission in [
        Permission::from(CanProposeSccpRouteGovernance),
        Permission::from(CanEnactGovernance),
    ] {
        Grant::account_permission(permission, ALICE_ID.clone())
            .execute(&ALICE_ID, stx)
            .expect("grant exact SCCP referendum permission");
    }
}

fn configure_taira(stx: &mut StateTransaction<'_, '_>) {
    stx.chain_id = iroha_data_model::ChainId::from(iroha_sccp::SCCP_TAIRA_CHAIN_ID_V1);
}

fn register_settlement_definition(stx: &mut StateTransaction<'_, '_>, route: &SccpGovernedRouteV1) {
    Register::asset_definition(AssetDefinition::numeric(
        route.settlement.asset_definition_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    ))
    .execute(&ALICE_ID, stx)
    .expect("register exact SCCP settlement definition");
}

fn register_custody_account(stx: &mut StateTransaction<'_, '_>, route: &SccpGovernedRouteV1) {
    Register::account(Account::new(route.settlement.custody_owner.clone()))
        .execute(&ALICE_ID, stx)
        .expect("register exact SCCP custody account");
}

fn materialize_custody_asset(stx: &mut StateTransaction<'_, '_>, route: &SccpGovernedRouteV1) {
    Mint::asset_quantity(
        1_u64,
        AssetId::new(
            route.settlement.asset_definition_id.clone(),
            route.settlement.custody_owner.clone(),
        ),
    )
    .execute(&ALICE_ID, stx)
    .expect("materialize exact SCCP custody asset");
}

fn register_route_resources(stx: &mut StateTransaction<'_, '_>, route: &SccpGovernedRouteV1) {
    register_settlement_definition(stx, route);
    register_custody_account(stx, route);
    materialize_custody_asset(stx, route);
}

fn execute_governance(
    stx: &mut StateTransaction<'_, '_>,
    action: SccpRouteGovernanceActionV1,
) -> Result<EnactSccpRouteGovernance, iroha_data_model::isi::error::InstructionExecutionError> {
    let anchor = SccpRouteGovernanceAnchorV1 {
        network_id: stx.network_id,
        action,
    };
    ProposeSccpRouteGovernance {
        anchor: anchor.clone(),
        window: None,
        mode: Some(VotingMode::Plain),
    }
    .execute(&ALICE_ID, stx)?;
    let proposal_id = stx
        .world
        .governance_proposals()
        .iter()
        .find_map(|(id, proposal)| match &proposal.kind {
            iroha_data_model::governance::types::ProposalKind::SccpRouteGovernance(payload)
                if payload.anchor.as_ref() == &anchor =>
            {
                Some(*id)
            }
            _ => None,
        })
        .expect("proposed SCCP anchor must be retained");
    let referendum_id = hex::encode(proposal_id);
    let referendum = *stx
        .world
        .governance_referenda()
        .get(&referendum_id)
        .expect("SCCP proposal must create a referendum");
    {
        let mut proposals = stx.world.governance_proposals_mut();
        let proposal = proposals
            .get_mut(&proposal_id)
            .expect("SCCP proposal record must exist");
        proposal.status = iroha_core::state::GovernanceProposalStatus::Approved;
        proposal.finalization_evidence = Some(
            iroha_data_model::governance::types::GovernanceFinalizationEvidence {
                proposal_id,
                referendum_id: proposal_id,
                finalized_at_height: 0,
                mode: VotingMode::Plain,
                approve: 1,
                reject: 0,
                abstain: 0,
                min_turnout: 1,
                approval_threshold_numerator: 1,
                approval_threshold_denominator: 2,
                approved: true,
            },
        );
    }
    stx.world.governance_referenda_mut().insert(
        referendum_id,
        iroha_core::state::GovernanceReferendumRecord {
            status: iroha_core::state::GovernanceReferendumStatus::Closed,
            ..referendum
        },
    );
    let enactment = EnactSccpRouteGovernance {
        referendum_id: proposal_id,
        anchor,
        at_window: iroha_data_model::governance::types::AtWindow {
            lower: referendum.h_start,
            upper: referendum.h_end,
        },
    };
    enactment.clone().execute(&ALICE_ID, stx)?;
    Ok(enactment)
}

fn register_action(
    route: SccpGovernedRouteV1,
    native_trust_anchor: Option<SccpNativeTrustAnchorV1>,
) -> SccpRouteGovernanceActionV1 {
    SccpRouteGovernanceActionV1::Register(SccpRegisterRouteV1 {
        route,
        native_trust_anchor,
    })
}

fn successor_route(mut route: SccpGovernedRouteV1) -> SccpGovernedRouteV1 {
    route.revision = route
        .revision
        .checked_add(1)
        .expect("test route revision has a successor");
    route.activation = SccpRouteActivationV1::Staged;
    let (route_address, route_code_hash) = match &mut route.destination {
        iroha_data_model::bridge::SccpDestinationDeploymentV1::Evm(destination) => {
            destination.route_address[0] = destination.route_address[0].wrapping_add(1);
            destination.route_code_hash[0] = destination.route_code_hash[0].wrapping_add(1);
            (destination.route_address, destination.route_code_hash)
        }
        iroha_data_model::bridge::SccpDestinationDeploymentV1::Tron(_) => {
            unreachable!("Ethereum fixture uses an EVM destination")
        }
        iroha_data_model::bridge::SccpDestinationDeploymentV1::Solana(_) => {
            unreachable!("Ethereum fixture uses an EVM destination")
        }
    };
    let route_config_hash = route
        .destination
        .route_configuration_hash(
            route.lane_id,
            &route.route_id,
            &route.asset_key,
            route.revision,
            route.settlement.payload_amount_scale,
        )
        .expect("derive successor route configuration");
    let SccpSourceEmitterV1::Evm(source) = &mut route.source_identity.emitter else {
        unreachable!("Ethereum fixture uses an EVM source emitter");
    };
    source.address = route_address;
    source.runtime_code_hash = route_code_hash;
    source.route_config_hash = route_config_hash;
    route
        .validate_registration()
        .expect("successor fixture must remain an exact governed route");
    route
}

#[test]
fn direct_sccp_route_governance_is_always_rejected() {
    let state = test_state();
    let mut block = state.block(test_header());
    let mut stx = block.transaction();
    let error =
        ApplySccpRouteGovernance::new(register_action(staged_route(), Some(native_anchor())))
            .execute(&ALICE_ID, &mut stx)
            .expect_err("direct SCCP route mutation must remain closed");
    assert!(format!("{error:?}").contains("finalized threshold referendum"));
}

#[test]
fn typed_sccp_enactment_rejects_wrong_preimage_network_and_replay() {
    let state = test_state();
    let mut block = state.block(test_header());
    let mut stx = block.transaction();
    configure_taira(&mut stx);
    grant_governance_permission(&mut stx);
    let route = staged_route();
    let key = route.key();
    register_route_resources(&mut stx, &route);

    let enactment = execute_governance(&mut stx, register_action(route, Some(native_anchor())))
        .expect("exact typed SCCP enactment must apply once");

    let replay = enactment
        .clone()
        .execute(&ALICE_ID, &mut stx)
        .expect_err("SCCP enactment must be one-shot");
    assert!(format!("{replay:?}").contains("cannot be replayed"));

    let mut wrong_action = enactment.clone();
    wrong_action.anchor.action = SccpRouteGovernanceActionV1::Remove(key);
    let wrong_action = wrong_action
        .execute(&ALICE_ID, &mut stx)
        .expect_err("an approved referendum id must not authorize another action preimage");
    assert!(format!("{wrong_action:?}").contains("does not derive"));

    let mut wrong_network = enactment;
    wrong_network.anchor.network_id = iroha_data_model::NetworkId::from_genesis_hash(
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA5; 32])),
    );
    assert_ne!(wrong_network.anchor.network_id, stx.network_id);
    let wrong_network = wrong_network
        .execute(&ALICE_ID, &mut stx)
        .expect_err("an SCCP approval must not replay on another exact network");
    assert!(format!("{wrong_network:?}").contains("different exact NetworkId"));
}

#[test]
fn route_registration_requires_permission_and_complete_resources() {
    let state = test_state();
    let mut block = state.block(test_header());
    let route = staged_route();
    let key = route.key();
    let action = register_action(route.clone(), Some(native_anchor()));

    {
        let mut denied = block.transaction();
        configure_taira(&mut denied);
        let before = denied.sccp_registry.revision();
        let error = execute_governance(&mut denied, action.clone())
            .expect_err("an unprivileged account must not register an SCCP route");
        assert!(format!("{error:?}").contains("CanProposeSccpRouteGovernance"));
        assert_eq!(denied.sccp_registry.revision(), before);
        assert!(denied.sccp_registry.route(&key).is_none());
    }

    let mut stx = block.transaction();
    configure_taira(&mut stx);
    grant_governance_permission(&mut stx);
    let before = stx.sccp_registry.revision();
    let error = execute_governance(&mut stx, action.clone())
        .expect_err("a route must not register before its settlement resources exist");
    assert!(format!("{error:?}").contains("asset definition is not registered"));
    assert_eq!(stx.sccp_registry.revision(), before);
    assert!(stx.sccp_registry.route(&key).is_none());

    register_settlement_definition(&mut stx, &route);
    let error = execute_governance(&mut stx, action.clone())
        .expect_err("a route must not register before its custody owner exists");
    assert!(format!("{error:?}").contains("custody owner is not registered"));
    assert_eq!(stx.sccp_registry.revision(), before);
    assert!(stx.sccp_registry.route(&key).is_none());

    register_custody_account(&mut stx, &route);
    execute_governance(&mut stx, action).expect("complete exact route registration");
    assert_ne!(stx.sccp_registry.revision(), before);
    assert_eq!(
        stx.sccp_registry
            .route(&key)
            .expect("registered route")
            .activation,
        SccpRouteActivationV1::Staged
    );
}

#[test]
fn solana_route_registration_validates_destination_key_and_commits() {
    let state = test_state();
    let mut block = state.block(test_header());
    let mut stx = block.transaction();
    configure_taira(&mut stx);
    grant_governance_permission(&mut stx);
    let route = staged_solana_route();
    let key = route.key();
    register_route_resources(&mut stx, &route);

    execute_governance(&mut stx, register_action(route, None))
        .expect("complete Solana route registration must validate its governed key");

    assert!(matches!(
        stx.sccp_registry
            .route(&key)
            .expect("registered Solana route")
            .destination,
        iroha_data_model::bridge::SccpDestinationDeploymentV1::Solana(_)
    ));
}

#[test]
fn route_registration_is_bound_to_the_exact_local_taira_profile() {
    let state = test_state();
    let mut block = state.block(test_header());
    let mut stx = block.transaction();
    grant_governance_permission(&mut stx);
    let route = staged_route();
    let key = route.key();
    let action = register_action(route.clone(), Some(native_anchor()));
    register_route_resources(&mut stx, &route);

    stx.chain_id = iroha_data_model::ChainId::from("sora-taira");
    let before = stx.sccp_registry.revision();
    let error = execute_governance(&mut stx, action.clone())
        .expect_err("a display alias must not authorize Taira route governance");
    assert!(
        format!("{error:?}").contains("not a canonical public SORA chain id"),
        "{error:?}"
    );
    assert_eq!(stx.sccp_registry.revision(), before);
    assert!(stx.sccp_registry.route(&key).is_none());

    configure_taira(&mut stx);
    execute_governance(&mut stx, action)
        .expect("the canonical Taira chain id must authorize its exact route");
    assert!(stx.sccp_registry.route(&key).is_some());
}

#[test]
fn route_registration_rejects_insufficient_asset_precision_without_mutation() {
    let state = test_state();
    let mut block = state.block(test_header());
    let mut stx = block.transaction();
    configure_taira(&mut stx);
    grant_governance_permission(&mut stx);
    let route = staged_route();
    let key = route.key();

    Register::asset_definition(AssetDefinition::new(
        route.settlement.asset_definition_id.clone(),
        "xor".to_owned(),
        NumericSpec::fractional(route.settlement.payload_amount_scale - 1),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    ))
    .execute(&ALICE_ID, &mut stx)
    .expect("register insufficient-precision settlement definition");
    register_custody_account(&mut stx, &route);
    materialize_custody_asset(&mut stx, &route);

    let before = stx.sccp_registry.revision();
    let error = execute_governance(&mut stx, register_action(route, Some(native_anchor())))
        .expect_err("settlement precision below the SCCP payload scale must reject");
    assert!(
        format!("{error:?}").contains("numeric precision cannot represent payload amounts"),
        "{error:?}"
    );
    assert_eq!(stx.sccp_registry.revision(), before);
    assert!(stx.sccp_registry.route(&key).is_none());
}

#[test]
fn activation_updates_are_strict_compare_and_swap() {
    let state = test_state();
    let mut block = state.block(test_header());
    let mut stx = block.transaction();
    configure_taira(&mut stx);
    grant_governance_permission(&mut stx);
    let route = staged_route();
    let key = route.key();
    register_route_resources(&mut stx, &route);
    execute_governance(&mut stx, register_action(route, Some(native_anchor())))
        .expect("register staged route");

    let before = stx.sccp_registry.revision();
    let stale = SccpRouteGovernanceActionV1::SetActivation(SccpSetRouteActivationV1 {
        key: key.clone(),
        expected_current: SccpRouteActivationV1::Paused,
        next: SccpRouteActivationV1::InboundOnly,
        inbound_finality_cutoff: None,
    });
    let error = execute_governance(&mut stx, stale)
        .expect_err("a stale activation compare-and-swap must reject");
    assert!(format!("{error:?}").contains("compare-and-swap"));
    assert_eq!(stx.sccp_registry.revision(), before);

    let illegal = SccpRouteGovernanceActionV1::SetActivation(SccpSetRouteActivationV1 {
        key: key.clone(),
        expected_current: SccpRouteActivationV1::Staged,
        next: SccpRouteActivationV1::Staged,
        inbound_finality_cutoff: None,
    });
    assert!(execute_governance(&mut stx, illegal).is_err());
    assert_eq!(stx.sccp_registry.revision(), before);

    execute_governance(
        &mut stx,
        SccpRouteGovernanceActionV1::SetActivation(SccpSetRouteActivationV1 {
            key: key.clone(),
            expected_current: SccpRouteActivationV1::Staged,
            next: SccpRouteActivationV1::Bidirectional,
            inbound_finality_cutoff: None,
        }),
    )
    .expect("activate exact route");
    assert_ne!(stx.sccp_registry.revision(), before);
    assert_eq!(
        stx.sccp_registry
            .route(&key)
            .expect("active route")
            .activation,
        SccpRouteActivationV1::Bidirectional
    );
}

#[test]
fn revision_switch_is_atomic_and_rejects_stale_state() {
    let state = test_state();
    let mut block = state.block(test_header());
    let mut stx = block.transaction();
    configure_taira(&mut stx);
    grant_governance_permission(&mut stx);
    let first = staged_route();
    let first_key = first.key();
    let successor = successor_route(first.clone());
    let successor_key = successor.key();
    let anchor = native_anchor();
    register_route_resources(&mut stx, &first);
    execute_governance(&mut stx, register_action(first, Some(anchor)))
        .expect("register first revision");
    execute_governance(
        &mut stx,
        SccpRouteGovernanceActionV1::SetActivation(SccpSetRouteActivationV1 {
            key: first_key.clone(),
            expected_current: SccpRouteActivationV1::Staged,
            next: SccpRouteActivationV1::Bidirectional,
            inbound_finality_cutoff: None,
        }),
    )
    .expect("activate first revision");
    execute_governance(&mut stx, register_action(successor, Some(anchor)))
        .expect("register immutable successor");

    let before = stx.sccp_registry.revision();
    let stale = SccpRouteGovernanceActionV1::SwitchRevision(SccpSwitchRouteRevisionV1 {
        previous_key: first_key.clone(),
        expected_previous: SccpRouteActivationV1::Paused,
        previous_next: SccpRouteActivationV1::InboundOnly,
        previous_inbound_finality_cutoff: None,
        successor_key: successor_key.clone(),
        successor_next: SccpRouteActivationV1::Bidirectional,
    });
    let error = execute_governance(&mut stx, stale)
        .expect_err("a stale revision cutover must reject atomically");
    assert!(format!("{error:?}").contains("compare-and-swap"));
    assert_eq!(stx.sccp_registry.revision(), before);
    assert_eq!(
        stx.sccp_registry
            .route(&first_key)
            .expect("first revision")
            .activation,
        SccpRouteActivationV1::Bidirectional
    );
    assert_eq!(
        stx.sccp_registry
            .route(&successor_key)
            .expect("successor revision")
            .activation,
        SccpRouteActivationV1::Staged
    );

    execute_governance(
        &mut stx,
        SccpRouteGovernanceActionV1::SwitchRevision(SccpSwitchRouteRevisionV1 {
            previous_key: first_key.clone(),
            expected_previous: SccpRouteActivationV1::Bidirectional,
            previous_next: SccpRouteActivationV1::InboundOnly,
            previous_inbound_finality_cutoff: None,
            successor_key: successor_key.clone(),
            successor_next: SccpRouteActivationV1::Bidirectional,
        }),
    )
    .expect("atomically switch exact route revisions");
    assert_ne!(stx.sccp_registry.revision(), before);
    assert_eq!(
        stx.sccp_registry
            .route(&first_key)
            .expect("draining first revision")
            .activation,
        SccpRouteActivationV1::InboundOnly
    );
    assert_eq!(
        stx.sccp_registry
            .route(&successor_key)
            .expect("active successor revision")
            .activation,
        SccpRouteActivationV1::Bidirectional
    );
}

#[test]
fn sequential_revision_lifecycles_reach_retained_cap_then_reject_without_mutation() {
    let state = test_state();
    let mut block = state.block(test_header());
    let mut stx = block.transaction();
    configure_taira(&mut stx);
    grant_governance_permission(&mut stx);
    let mut current = staged_route();
    let mut current_anchor = native_anchor();
    register_route_resources(&mut stx, &current);
    execute_governance(
        &mut stx,
        register_action(current.clone(), Some(current_anchor)),
    )
    .expect("register first revision");
    execute_governance(
        &mut stx,
        SccpRouteGovernanceActionV1::SetActivation(SccpSetRouteActivationV1 {
            key: current.key(),
            expected_current: SccpRouteActivationV1::Staged,
            next: SccpRouteActivationV1::Bidirectional,
            inbound_finality_cutoff: None,
        }),
    )
    .expect("activate first revision");
    current.activation = SccpRouteActivationV1::Bidirectional;

    for expected_revision in 2..=u32::try_from(SCCP_V1_MAX_RETAINED_ROUTES_PER_LANE)
        .expect("retained route bound fits u32")
    {
        let mut successor = successor_route(current.clone());
        assert_eq!(successor.revision, expected_revision);
        let next_anchor = SccpNativeTrustAnchorV1 {
            anchor_hash: [0x90_u8
                .wrapping_add(u8::try_from(expected_revision).expect("test revision fits u8"));
                32],
            checkpoint_height: current_anchor
                .checkpoint_height
                .checked_add(1)
                .expect("test checkpoint has a successor"),
            ..current_anchor
        };
        execute_governance(
            &mut stx,
            SccpRouteGovernanceActionV1::AdvanceTrustAnchor(SccpAdvanceLaneTrustAnchorV1 {
                lane_id: current.lane_id,
                expected_current: current_anchor,
                next: next_anchor,
            }),
        )
        .expect("append the next authenticated source checkpoint boundary");
        execute_governance(
            &mut stx,
            register_action(successor.clone(), Some(next_anchor)),
        )
        .expect("append one staged successor");
        execute_governance(
            &mut stx,
            SccpRouteGovernanceActionV1::SwitchRevision(SccpSwitchRouteRevisionV1 {
                previous_key: current.key(),
                expected_previous: SccpRouteActivationV1::Bidirectional,
                previous_next: SccpRouteActivationV1::Retired,
                previous_inbound_finality_cutoff: Some(
                    iroha_data_model::bridge::SccpInboundFinalityCutoffV1 {
                        trust_anchor_hash: current_anchor.anchor_hash,
                        max_anchor_interval_height: next_anchor.checkpoint_height,
                    },
                ),
                successor_key: successor.key(),
                successor_next: SccpRouteActivationV1::Bidirectional,
            }),
        )
        .expect("atomically cut over to successor");
        successor.activation = SccpRouteActivationV1::Bidirectional;
        current = successor;
        current_anchor = next_anchor;
    }

    let lane = stx
        .sccp_registry
        .lane(current.lane_id)
        .expect("governed lane survives repeated rotations");
    assert_eq!(lane.routes.len(), SCCP_V1_MAX_RETAINED_ROUTES_PER_LANE);
    assert_eq!(
        lane.native_trust_anchors.len(),
        SCCP_V1_MAX_RETAINED_ROUTES_PER_LANE
    );
    assert_eq!(lane.current_native_trust_anchor(), Some(current_anchor));
    assert_eq!(
        lane.routes
            .iter()
            .filter(|route| route.activation.consumes_live_capacity())
            .count(),
        1
    );
    for revision in 1..u32::try_from(SCCP_V1_MAX_RETAINED_ROUTES_PER_LANE)
        .expect("retained route bound fits u32")
    {
        assert_eq!(
            lane.routes
                .iter()
                .find(|route| route.revision == revision)
                .expect("historical revision retained")
                .activation,
            SccpRouteActivationV1::Retired
        );
    }
    assert_eq!(
        lane.routes
            .iter()
            .find(|route| {
                route.revision
                    == u32::try_from(SCCP_V1_MAX_RETAINED_ROUTES_PER_LANE)
                        .expect("retained route bound fits u32")
            })
            .expect("current revision retained")
            .activation,
        SccpRouteActivationV1::Bidirectional
    );

    let overflow = successor_route(current);
    let before = stx.sccp_registry.revision();
    let error = execute_governance(
        &mut stx,
        register_action(overflow.clone(), Some(current_anchor)),
    )
    .expect_err("one route beyond the retained-history cap must fail closed");
    assert!(format!("{error:?}").contains("retained route"), "{error:?}");
    assert_eq!(stx.sccp_registry.revision(), before);
    assert!(stx.sccp_registry.route(&overflow.key()).is_none());
}

#[test]
fn live_route_capacity_rejects_staged_accumulation_without_mutation() {
    let state = test_state();
    let mut block = state.block(test_header());
    let mut stx = block.transaction();
    configure_taira(&mut stx);
    grant_governance_permission(&mut stx);
    let anchor = native_anchor();
    let mut previous = staged_route();
    register_route_resources(&mut stx, &previous);
    execute_governance(&mut stx, register_action(previous.clone(), Some(anchor)))
        .expect("register first staged revision");

    for _ in 1..SCCP_V1_MAX_LIVE_ROUTES_PER_LANE {
        let successor = successor_route(previous);
        execute_governance(&mut stx, register_action(successor.clone(), Some(anchor)))
            .expect("live capacity admits the exact boundary");
        previous = successor;
    }
    let overflow = successor_route(previous);
    let before = stx.sccp_registry.revision();
    let error = execute_governance(&mut stx, register_action(overflow.clone(), Some(anchor)))
        .expect_err("ninth nonterminal route must exceed the lane mutation bound");
    assert!(
        format!("{error:?}").contains("nonterminal routes"),
        "{error:?}"
    );
    assert_eq!(stx.sccp_registry.revision(), before);
    assert!(stx.sccp_registry.route(&overflow.key()).is_none());
}

#[test]
fn trust_anchor_initialization_and_advance_are_strict_cas() {
    let state = test_state();
    let mut block = state.block(test_header());
    let mut stx = block.transaction();
    configure_taira(&mut stx);
    grant_governance_permission(&mut stx);
    let route = staged_route();
    let lane_id = route.lane_id;
    register_route_resources(&mut stx, &route);
    execute_governance(&mut stx, register_action(route, None))
        .expect("register an anchorless staged route");

    let initial = native_anchor();
    let initialize =
        SccpRouteGovernanceActionV1::InitializeTrustAnchor(SccpInitializeLaneTrustAnchorV1 {
            lane_id,
            expected_current: None,
            initial,
        });
    execute_governance(&mut stx, initialize.clone()).expect("initialize lane anchor");
    assert_eq!(
        stx.sccp_registry
            .lane(lane_id)
            .expect("governed lane")
            .current_native_trust_anchor(),
        Some(initial)
    );

    let before = stx.sccp_registry.revision();
    let error = execute_governance(&mut stx, initialize)
        .expect_err("replaying a None-to-Some initialization must reject");
    assert!(format!("{error:?}").contains("compare-and-swap"));
    assert_eq!(stx.sccp_registry.revision(), before);

    let stale_expected = SccpNativeTrustAnchorV1 {
        anchor_hash: [0xD1; 32],
        checkpoint_height: initial.checkpoint_height + 1,
        ..initial
    };
    let stale_next = SccpNativeTrustAnchorV1 {
        anchor_hash: [0xD2; 32],
        checkpoint_height: initial.checkpoint_height + 2,
        ..initial
    };
    let stale = SccpRouteGovernanceActionV1::AdvanceTrustAnchor(SccpAdvanceLaneTrustAnchorV1 {
        lane_id,
        expected_current: stale_expected,
        next: stale_next,
    });
    assert!(execute_governance(&mut stx, stale).is_err());
    assert_eq!(stx.sccp_registry.revision(), before);

    let next = SccpNativeTrustAnchorV1 {
        anchor_hash: [0xD3; 32],
        checkpoint_height: initial.checkpoint_height + 1,
        ..initial
    };
    execute_governance(
        &mut stx,
        SccpRouteGovernanceActionV1::AdvanceTrustAnchor(SccpAdvanceLaneTrustAnchorV1 {
            lane_id,
            expected_current: initial,
            next,
        }),
    )
    .expect("advance exact native checkpoint");
    assert_ne!(stx.sccp_registry.revision(), before);
    assert_eq!(
        stx.sccp_registry
            .lane(lane_id)
            .expect("governed lane")
            .current_native_trust_anchor(),
        Some(next)
    );
    let lane = stx.sccp_registry.lane(lane_id).expect("governed lane");
    assert_eq!(lane.native_trust_anchors, vec![initial, next]);

    let before_rollback = stx.sccp_registry.revision();
    let rollback = SccpRouteGovernanceActionV1::AdvanceTrustAnchor(SccpAdvanceLaneTrustAnchorV1 {
        lane_id,
        expected_current: next,
        next: initial,
    });
    assert!(execute_governance(&mut stx, rollback).is_err());
    assert_eq!(stx.sccp_registry.revision(), before_rollback);
}

#[test]
fn remove_accepts_only_never_used_staged_routes() {
    let state = test_state();
    let mut block = state.block(test_header());
    let mut stx = block.transaction();
    configure_taira(&mut stx);
    grant_governance_permission(&mut stx);
    let route = staged_route();
    let key = route.key();
    let anchor = native_anchor();
    register_route_resources(&mut stx, &route);
    execute_governance(&mut stx, register_action(route.clone(), Some(anchor)))
        .expect("register removable staged route");
    let before_remove = stx.sccp_registry.revision();
    execute_governance(&mut stx, SccpRouteGovernanceActionV1::Remove(key.clone()))
        .expect("remove a never-used staged route");
    assert_ne!(stx.sccp_registry.revision(), before_remove);
    assert!(stx.sccp_registry.route(&key).is_none());

    execute_governance(&mut stx, register_action(route, Some(anchor)))
        .expect("re-register clean route");
    execute_governance(
        &mut stx,
        SccpRouteGovernanceActionV1::SetActivation(SccpSetRouteActivationV1 {
            key: key.clone(),
            expected_current: SccpRouteActivationV1::Staged,
            next: SccpRouteActivationV1::Bidirectional,
            inbound_finality_cutoff: None,
        }),
    )
    .expect("activate re-registered route");
    let before = stx.sccp_registry.revision();
    let error = execute_governance(&mut stx, SccpRouteGovernanceActionV1::Remove(key.clone()))
        .expect_err("an active route must not be removable");
    assert!(format!("{error:?}").contains("never-used staged"));
    assert_eq!(stx.sccp_registry.revision(), before);
    assert_eq!(
        stx.sccp_registry
            .route(&key)
            .expect("active route remains")
            .activation,
        SccpRouteActivationV1::Bidirectional
    );
}
