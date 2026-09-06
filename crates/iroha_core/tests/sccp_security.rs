//! Isolated regressions for SCCP governance and verifier-work security boundaries.
#![cfg(feature = "iroha-core-tests")]

use std::num::NonZeroU64;

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::isi::world::isi::{
        remove_sccp_route_for_testing, sccp_bsc_native_verifier_work_fields_for_testing,
    },
    state::{State, ValidatedSccpRegistryV1, World},
};
use iroha_data_model::{
    ChainId,
    block::BlockHeader,
    bridge::{
        SccpDestinationDeploymentV1, SccpGovernedLaneV1, SccpGovernedRouteV1, SccpLaneIdV1,
        SccpNetworkV1, SccpRegistryV1, SccpRouteActivationV1, SccpSourceEmitterV1,
        SccpSourceIdentityV1, SccpTronDestinationDeploymentV1, SccpTronSourceEmitterV1,
    },
};

fn test_state() -> State {
    State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    )
}

fn test_header() -> BlockHeader {
    BlockHeader::new(
        NonZeroU64::new(1).expect("one is nonzero"),
        None,
        None,
        None,
        0,
        0,
    )
}

fn tron_lane() -> SccpLaneIdV1 {
    SccpLaneIdV1 {
        source: SccpNetworkV1::TronMainnet,
        target: SccpNetworkV1::SoraTaira,
    }
}

fn staged_tron_route() -> SccpGovernedRouteV1 {
    let mut route = iroha_sccp::sccp_exact_evm_governed_route_test_fixture_v1(
        SccpNetworkV1::EthereumMainnet,
        SccpRouteActivationV1::Staged,
    );
    let SccpDestinationDeploymentV1::Evm(evm) = route.destination else {
        panic!("exact EVM fixture must carry an EVM deployment")
    };
    let deployment = SccpTronDestinationDeploymentV1 {
        token_address: evm.token_address,
        token_code_hash: evm.token_code_hash,
        verifier_address: evm.verifier_address,
        verifier_code_hash: evm.verifier_code_hash,
        verifying_key: evm.verifying_key,
        verifier_key_hash: evm.verifier_key_hash,
        outbound_proof_policy: evm.outbound_proof_policy,
        route_address: evm.route_address,
        route_code_hash: evm.route_code_hash,
        replay_verifier_address: evm.replay_verifier_address,
        replay_verifier_code_hash: evm.replay_verifier_code_hash,
        mint_breaker_address: evm.mint_breaker_address,
        mint_breaker_code_hash: evm.mint_breaker_code_hash,
        taira_to_token_multiplier: evm.taira_to_token_multiplier,
        max_wrapped_supply: evm.max_wrapped_supply,
    };
    let lane = tron_lane();
    route.lane_id = lane;
    route.route_id = iroha_sccp::SCCP_TAIRA_TRON_XOR_ROUTE_ID_V1.to_owned();
    route.destination = SccpDestinationDeploymentV1::Tron(deployment);
    let route_config_hash = route
        .destination
        .route_configuration_hash(
            route.lane_id,
            &route.route_id,
            &route.asset_key,
            route.revision,
            route.settlement.payload_amount_scale,
        )
        .expect("exact staged TRON route configuration");
    route.source_identity = SccpSourceIdentityV1 {
        lane,
        emitter: SccpSourceEmitterV1::Tron(SccpTronSourceEmitterV1 {
            address: deployment.route_address,
            runtime_code_hash: deployment.route_code_hash,
            route_config_hash,
        }),
    };
    route
        .validate_with_anchor(None)
        .expect("exact staged TRON route must validate without an active anchor");
    route
}

fn registry_with(route: SccpGovernedRouteV1) -> SccpRegistryV1 {
    SccpRegistryV1 {
        version: 1,
        lanes: vec![SccpGovernedLaneV1 {
            lane_id: route.lane_id,
            native_trust_anchors: Vec::new(),
            current_native_trust_anchor_hash: None,
            routes: vec![route],
        }],
    }
}

#[test]
fn registered_staged_tron_route_cannot_be_removed() {
    let state = test_state();
    let mut block = state.block(test_header());
    let mut transaction = block.transaction();
    transaction.chain_id = ChainId::from(iroha_sccp::SCCP_TAIRA_CHAIN_ID_V1);
    let registry = registry_with(staged_tron_route());
    let key = registry.lanes[0].routes[0].key();
    transaction.sccp_registry =
        ValidatedSccpRegistryV1::try_from_wire(registry).expect("exact staged TRON registry");
    let before = transaction.sccp_registry.canonical_wire().to_vec();

    let error = remove_sccp_route_for_testing(key.clone(), &mut transaction)
        .expect_err("a registered TRON address must remain a permanent replay boundary");

    assert!(
        format!("{error:?}").contains("immutable addresses remain replay boundaries"),
        "unexpected TRON removal rejection: {error:?}"
    );
    assert_eq!(transaction.sccp_registry.canonical_wire(), before);
    assert!(transaction.sccp_registry.route(&key).is_some());
}

#[test]
fn never_used_staged_evm_route_remains_removable() {
    let state = test_state();
    let mut block = state.block(test_header());
    let mut transaction = block.transaction();
    transaction.chain_id = ChainId::from(iroha_sccp::SCCP_TAIRA_CHAIN_ID_V1);
    let route = iroha_sccp::sccp_exact_evm_governed_route_test_fixture_v1(
        SccpNetworkV1::EthereumMainnet,
        SccpRouteActivationV1::Staged,
    );
    let key = route.key();
    transaction.sccp_registry = ValidatedSccpRegistryV1::try_from_wire(registry_with(route))
        .expect("exact staged EVM registry");

    remove_sccp_route_for_testing(key, &mut transaction)
        .expect("a never-used staged EVM route remains removable");

    assert!(transaction.sccp_registry.lanes().is_empty());
}

#[test]
fn bsc_native_verifier_work_uses_complete_sccp_estimate() {
    let estimate = iroha_sccp::BscNativeFinalityWorkEstimateV1 {
        continuation_headers: 11,
        framed_header_bytes: 12,
        secp256k1_recoveries: 13,
        bls_aggregate_checks_upper_bound: 14,
        bls_signer_contributions_upper_bound: 15,
    };

    assert_eq!(
        sccp_bsc_native_verifier_work_fields_for_testing(estimate),
        [11, 12, 13, 14, 15]
    );
}
