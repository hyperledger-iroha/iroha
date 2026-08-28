//! SCCP registry security-boundary regressions.
#![cfg(feature = "test-fixtures")]

use iroha_data_model::bridge::{
    BridgeNativeProofBackendV1, SccpDestinationDeploymentV1, SccpGovernedLaneV1,
    SccpGovernedRouteV1, SccpLaneIdV1, SccpNativeTrustAnchorV1, SccpNetworkV1, SccpRegistryV1,
    SccpRouteActivationV1, SccpRouteValidationError, SccpSourceEmitterV1, SccpSourceIdentityV1,
    SccpTronDestinationDeploymentV1, SccpTronSourceEmitterV1,
};

fn tron_lane() -> SccpLaneIdV1 {
    SccpLaneIdV1 {
        source: SccpNetworkV1::TronMainnet,
        target: SccpNetworkV1::SoraTaira,
    }
}

fn tron_deployment_fixture() -> SccpTronDestinationDeploymentV1 {
    let route = iroha_sccp::sccp_exact_evm_governed_route_test_fixture_v1(
        SccpNetworkV1::EthereumMainnet,
        SccpRouteActivationV1::Staged,
    );
    let SccpDestinationDeploymentV1::Evm(deployment) = route.destination else {
        panic!("exact EVM fixture must carry an EVM deployment")
    };
    SccpTronDestinationDeploymentV1 {
        token_address: deployment.token_address,
        token_code_hash: deployment.token_code_hash,
        verifier_address: deployment.verifier_address,
        verifier_code_hash: deployment.verifier_code_hash,
        verifying_key: deployment.verifying_key,
        verifier_key_hash: deployment.verifier_key_hash,
        outbound_proof_policy: deployment.outbound_proof_policy,
        route_address: deployment.route_address,
        route_code_hash: deployment.route_code_hash,
        replay_verifier_address: deployment.replay_verifier_address,
        replay_verifier_code_hash: deployment.replay_verifier_code_hash,
        mint_breaker_address: deployment.mint_breaker_address,
        mint_breaker_code_hash: deployment.mint_breaker_code_hash,
        taira_to_token_multiplier: deployment.taira_to_token_multiplier,
        max_wrapped_supply: deployment.max_wrapped_supply,
    }
}

fn tron_route(
    revision: u32,
    activation: SccpRouteActivationV1,
    deployment: SccpTronDestinationDeploymentV1,
) -> SccpGovernedRouteV1 {
    let mut route = iroha_sccp::sccp_exact_evm_governed_route_test_fixture_v1(
        SccpNetworkV1::EthereumMainnet,
        activation,
    );
    route.lane_id = tron_lane();
    route.route_id = iroha_sccp::SCCP_TAIRA_TRON_XOR_ROUTE_ID_V1.to_owned();
    route.revision = revision;
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
        .expect("exact TRON route configuration");
    route.source_identity = SccpSourceIdentityV1 {
        lane: route.lane_id,
        emitter: SccpSourceEmitterV1::Tron(SccpTronSourceEmitterV1 {
            address: deployment.route_address,
            runtime_code_hash: deployment.route_code_hash,
            route_config_hash,
        }),
    };
    route
}

fn tron_registry(routes: Vec<SccpGovernedRouteV1>) -> SccpRegistryV1 {
    let anchor = SccpNativeTrustAnchorV1 {
        backend: BridgeNativeProofBackendV1::TronDpos,
        checkpoint_height: 100,
        anchor_hash: [0xd1; 32],
    };
    SccpRegistryV1 {
        version: 1,
        lanes: vec![SccpGovernedLaneV1 {
            lane_id: tron_lane(),
            native_trust_anchors: vec![anchor],
            current_native_trust_anchor_hash: Some(anchor.anchor_hash),
            routes,
        }],
    }
}

#[test]
fn retained_tron_revisions_reject_address_reuse_and_accept_a_fresh_successor() {
    let first_deployment = tron_deployment_fixture();
    let mut successor_deployment = first_deployment;
    successor_deployment.verifier_address = [0x32; 20];
    successor_deployment.verifier_code_hash = [0x42; 32];
    let first = tron_route(1, SccpRouteActivationV1::InboundOnly, first_deployment);
    let reused_address = tron_route(
        2,
        SccpRouteActivationV1::Bidirectional,
        successor_deployment,
    );
    assert_eq!(
        tron_registry(vec![first.clone(), reused_address]).validate(),
        Err(SccpRouteValidationError::DuplicateTronSourceAddress)
    );

    successor_deployment.route_address = [0x52; 20];
    successor_deployment.route_code_hash = [0x62; 32];
    tron_registry(vec![
        first,
        tron_route(
            2,
            SccpRouteActivationV1::Bidirectional,
            successor_deployment,
        ),
    ])
    .validate()
    .expect("a fresh immutable TRON address remains a valid successor");
}
