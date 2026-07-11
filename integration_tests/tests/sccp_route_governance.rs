//! Four-peer consensus coverage for exact SCCP route governance.

use std::time::{Duration, Instant};

use eyre::{Result, eyre};
use integration_tests::sandbox;
use iroha::data_model::{
    account::{Account, AccountId},
    asset::{AssetDefinition, AssetId},
    block::consensus_v2::PROTOCOL_VERSION,
    bridge::{
        BridgeNativeProofBackendV1, SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER,
        SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE, SccpBn254G1PointV1, SccpBn254G2PointV1,
        SccpDestinationDeploymentV1, SccpEvmDestinationDeploymentV1, SccpEvmSourceEmitterV1,
        SccpGovernedRouteV1, SccpGroth16Bn254IcV1, SccpGroth16Bn254SemanticCircuitV1,
        SccpGroth16Bn254VerifyingKeyV1, SccpLaneIdV1, SccpNativeTrustAnchorV1, SccpNetworkV1,
        SccpOutboundProofPolicyV1, SccpRegistryV1, SccpRouteActivationV1, SccpRouteKeyV1,
        SccpSemanticProofProfileV1, SccpSoraFinalityAnchorV1, SccpSoraSettlementV1,
        SccpSourceEmitterV1, SccpSourceIdentityV1, sccp_groth16_bn254_public_signal_schema_hash_v1,
        sccp_groth16_bn254_verifying_key_hash_v1, sccp_sora_taira_chain_id_hash_v1,
        sccp_v1_taira_xor_asset_definition_id,
    },
    domain::Domain,
    isi::{
        Grant, Mint, Register,
        bridge::{
            ApplySccpRouteGovernance, SccpRegisterRouteV1, SccpRouteGovernanceActionV1,
            SccpSetRouteActivationV1, SccpSwitchRouteRevisionV1,
        },
    },
    permission::Permission,
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_executor_data_model::permission::sccp::CanManageSccpGovernance;
use iroha_test_network::{Network, NetworkBuilder, init_instruction_registry};
use iroha_test_samples::{ALICE_ID, BOB_ID, BOB_KEYPAIR};
use tokio::time::sleep;

const REGISTRY_CONVERGENCE_TIMEOUT: Duration = Duration::from_secs(120);
const TAIRA_CHAIN_ID: &str = "fc56984b-2be7-431d-840e-21514d1883f0";

fn word_u64(value: u64) -> [u8; 32] {
    let mut word = [0; 32];
    word[24..].copy_from_slice(&value.to_be_bytes());
    word
}

fn hex32(value: &str) -> [u8; 32] {
    hex::decode(value)
        .expect("static SCCP integration vector must be hexadecimal")
        .try_into()
        .expect("static SCCP integration vector must contain 32 bytes")
}

fn error_chain_text(error: &eyre::Report) -> String {
    let mut text = format!("{error:?}");
    for cause in error.chain() {
        text.push_str(" | ");
        text.push_str(&cause.to_string());
    }
    text
}

fn integration_verifying_key() -> SccpGroth16Bn254VerifyingKeyV1 {
    let g1 = SccpBn254G1PointV1 {
        x: word_u64(1),
        y: word_u64(2),
    };
    let g2 = SccpBn254G2PointV1 {
        x_c0: hex32("1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed"),
        x_c1: hex32("198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2"),
        y_c0: hex32("12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa"),
        y_c1: hex32("090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b"),
    };
    SccpGroth16Bn254VerifyingKeyV1 {
        version: 1,
        alpha1: g1,
        beta2: g2,
        gamma2: g2,
        delta2: g2,
        ic: SccpGroth16Bn254IcV1 {
            constant: g1,
            signal_0: g1,
            signal_1: g1,
            signal_2: g1,
            signal_3: g1,
            signal_4: g1,
            signal_5: g1,
            signal_6: g1,
            signal_7: g1,
            signal_8: g1,
            signal_9: g1,
            signal_10: g1,
        },
    }
}

fn integration_outbound_policy() -> SccpOutboundProofPolicyV1 {
    SccpOutboundProofPolicyV1 {
        version: 1,
        semantic_profile: SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(
            SccpGroth16Bn254SemanticCircuitV1 {
                version: 1,
                circuit_commitment: [0x71; 32],
                witness_generator_commitment: [0x72; 32],
                public_signal_schema_hash: sccp_groth16_bn254_public_signal_schema_hash_v1(),
            },
        ),
        sora_finality_anchor: SccpSoraFinalityAnchorV1 {
            version: 1,
            source_network: SccpNetworkV1::SoraTaira,
            protocol_version: PROTOCOL_VERSION,
            chain_id_hash: sccp_sora_taira_chain_id_hash_v1(),
            checkpoint_height: 5,
            checkpoint_block_hash: [0x73; 32],
            checkpoint_context_id: [0x74; 32],
            checkpoint_finality_artifact_hash: [0x75; 32],
        },
    }
}

fn integration_route() -> SccpGovernedRouteV1 {
    let lane_id = SccpLaneIdV1 {
        source: SccpNetworkV1::EthereumMainnet,
        target: SccpNetworkV1::SoraTaira,
    };
    let verifying_key = integration_verifying_key();
    let deployment = SccpEvmDestinationDeploymentV1 {
        token_address: [0x11; 20],
        token_code_hash: [0x21; 32],
        verifier_address: [0x31; 20],
        verifier_code_hash: [0x41; 32],
        verifying_key,
        verifier_key_hash: sccp_groth16_bn254_verifying_key_hash_v1(&verifying_key)
            .expect("integration verification key must be curve-valid"),
        outbound_proof_policy: integration_outbound_policy(),
        route_address: [0x51; 20],
        route_code_hash: [0x61; 32],
        taira_to_token_multiplier: SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER,
    };
    let destination = SccpDestinationDeploymentV1::Evm(deployment);
    let route_configuration_hash = destination
        .route_configuration_hash(
            lane_id,
            "taira_eth_xor",
            "xor",
            1,
            SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
        )
        .expect("integration route configuration must be canonical");
    let custody = KeyPair::try_from_seed(vec![0x81; 32], Algorithm::Ed25519)
        .expect("integration custody key must be valid")
        .public_key()
        .clone();
    let route = SccpGovernedRouteV1 {
        lane_id,
        route_id: "taira_eth_xor".to_owned(),
        asset_key: "xor".to_owned(),
        revision: 1,
        activation: SccpRouteActivationV1::Staged,
        inbound_finality_cutoff: None,
        source_identity: SccpSourceIdentityV1 {
            lane: lane_id,
            emitter: SccpSourceEmitterV1::Evm(SccpEvmSourceEmitterV1 {
                address: deployment.route_address,
                runtime_code_hash: deployment.route_code_hash,
                route_config_hash: route_configuration_hash,
            }),
        },
        destination,
        settlement: SccpSoraSettlementV1 {
            asset_definition_id: sccp_v1_taira_xor_asset_definition_id(),
            custody_account_id: AccountId::new(custody),
            payload_amount_scale: SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
        },
    };
    route
        .validate_registration()
        .expect("integration route must satisfy registration invariants");
    route
}

fn successor_route(mut route: SccpGovernedRouteV1) -> SccpGovernedRouteV1 {
    route.revision = 2;
    let (route_address, route_code_hash) = match &mut route.destination {
        SccpDestinationDeploymentV1::Evm(destination) => {
            destination.route_address[0] ^= 1;
            destination.route_code_hash[0] ^= 1;
            (destination.route_address, destination.route_code_hash)
        }
        SccpDestinationDeploymentV1::Tron(_) => {
            unreachable!("Ethereum integration route uses an EVM destination")
        }
    };
    let route_configuration_hash = route
        .destination
        .route_configuration_hash(
            route.lane_id,
            &route.route_id,
            &route.asset_key,
            route.revision,
            route.settlement.payload_amount_scale,
        )
        .expect("successor route configuration must be canonical");
    let SccpSourceEmitterV1::Evm(source) = &mut route.source_identity.emitter else {
        unreachable!("Ethereum integration route uses an EVM source emitter");
    };
    source.address = route_address;
    source.runtime_code_hash = route_code_hash;
    source.route_config_hash = route_configuration_hash;
    route
        .validate_registration()
        .expect("successor integration route must satisfy registration invariants");
    route
}

fn integration_native_anchor() -> SccpNativeTrustAnchorV1 {
    SccpNativeTrustAnchorV1 {
        backend: BridgeNativeProofBackendV1::EthereumBeacon,
        anchor_hash: [0x91; 32],
        checkpoint_height: 1,
    }
}

fn route_in_registry<'a>(
    registry: &'a iroha::data_model::bridge::SccpRegistryV1,
    key: &SccpRouteKeyV1,
) -> Option<&'a SccpGovernedRouteV1> {
    registry
        .lanes
        .iter()
        .flat_map(|lane| lane.routes.iter())
        .find(|route| route.key() == *key)
}

async fn wait_for_route_states(
    network: &Network,
    expected: &[(&SccpRouteKeyV1, Option<SccpRouteActivationV1>)],
) -> Result<SccpRegistryV1> {
    let deadline = Instant::now() + REGISTRY_CONVERGENCE_TIMEOUT;
    loop {
        let mut observed = Vec::with_capacity(network.peers().len());
        let mut converged = true;
        let mut reference = None;
        for (peer_index, peer) in network.peers().iter().enumerate() {
            match peer.client().get_sccp_registry() {
                Ok(registry) => {
                    let activations = expected
                        .iter()
                        .map(|(key, _)| {
                            route_in_registry(&registry, key).map(|route| route.activation)
                        })
                        .collect::<Vec<_>>();
                    let expected_activations = expected
                        .iter()
                        .map(|(_, activation)| *activation)
                        .collect::<Vec<_>>();
                    observed.push(format!("peer {peer_index}: {activations:?}"));
                    converged &= activations == expected_activations;
                    if let Some(reference) = &reference {
                        converged &= reference == &registry;
                    } else {
                        reference = Some(registry);
                    }
                }
                Err(error) => {
                    observed.push(format!("peer {peer_index}: query-error:{error}"));
                    converged = false;
                }
            }
        }
        if converged && let Some(registry) = reference {
            return Ok(registry);
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "SCCP registry did not converge to {expected:?}; peer observations: {observed:?}"
            ));
        }
        sleep(Duration::from_millis(250)).await;
    }
}

async fn wait_for_atomic_revision_switch(
    network: &Network,
    previous_key: &SccpRouteKeyV1,
    successor_key: &SccpRouteKeyV1,
) -> Result<SccpRegistryV1> {
    let before = (
        Some(SccpRouteActivationV1::Bidirectional),
        Some(SccpRouteActivationV1::Staged),
    );
    let after = (
        Some(SccpRouteActivationV1::InboundOnly),
        Some(SccpRouteActivationV1::Bidirectional),
    );
    let deadline = Instant::now() + REGISTRY_CONVERGENCE_TIMEOUT;
    loop {
        let mut observed = Vec::with_capacity(network.peers().len());
        let mut converged = true;
        let mut reference = None;
        for (peer_index, peer) in network.peers().iter().enumerate() {
            match peer.client().get_sccp_registry() {
                Ok(registry) => {
                    let state = (
                        route_in_registry(&registry, previous_key).map(|route| route.activation),
                        route_in_registry(&registry, successor_key).map(|route| route.activation),
                    );
                    observed.push(format!("peer {peer_index}: {state:?}"));
                    if state != before && state != after {
                        return Err(eyre!(
                            "peer {peer_index} exposed a non-atomic SCCP revision switch: \
                             {state:?}; expected an old {before:?} or new {after:?} snapshot"
                        ));
                    }
                    converged &= state == after;
                    if let Some(reference) = &reference {
                        converged &= reference == &registry;
                    } else {
                        reference = Some(registry);
                    }
                }
                Err(error) => {
                    observed.push(format!("peer {peer_index}: query-error:{error}"));
                    converged = false;
                }
            }
        }
        if converged && let Some(registry) = reference {
            return Ok(registry);
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "atomic SCCP revision switch did not converge; peer observations: {observed:?}"
            ));
        }
        sleep(Duration::from_millis(250)).await;
    }
}

fn register_action(route: SccpGovernedRouteV1) -> ApplySccpRouteGovernance {
    ApplySccpRouteGovernance::new(SccpRouteGovernanceActionV1::Register(SccpRegisterRouteV1 {
        route,
        native_trust_anchor: Some(integration_native_anchor()),
    }))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn exact_sccp_route_governance_converges_and_rejects_adversarial_updates() -> Result<()> {
    init_instruction_registry();

    let route = integration_route();
    let key = route.key();
    let successor = successor_route(route.clone());
    let successor_key = successor.key();
    let custody_asset = AssetId::new(
        route.settlement.asset_definition_id.clone(),
        route.settlement.custody_account_id.clone(),
    );
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| layer.write("chain", TAIRA_CHAIN_ID))
        .with_genesis_instruction(Register::domain(Domain::new(
            route.settlement.asset_definition_id.domain().clone(),
        )))
        .with_genesis_instruction(Register::account(Account::new(
            route.settlement.custody_account_id.clone(),
        )))
        .with_genesis_instruction(Register::asset_definition(
            AssetDefinition::numeric(route.settlement.asset_definition_id.clone())
                .with_name("xor".to_owned()),
        ))
        .with_genesis_instruction(Mint::asset_numeric(1_u64, custody_asset))
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanManageSccpGovernance),
            ALICE_ID.clone(),
        ));

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(exact_sccp_route_governance_converges_and_rejects_adversarial_updates),
    )
    .await?
    else {
        return Ok(());
    };

    let alice = network.client();
    let bob = network
        .peer()
        .client_for(&BOB_ID, BOB_KEYPAIR.private_key().clone());

    let initial_registry =
        wait_for_route_states(&network, &[(&key, None), (&successor_key, None)]).await?;
    assert!(initial_registry.lanes.is_empty());

    let unauthorized = bob
        .submit_blocking(register_action(route.clone()))
        .expect_err("an account without CanManageSccpGovernance must be rejected");
    let unauthorized_text = error_chain_text(&unauthorized);
    assert!(
        unauthorized_text.contains("CanManageSccpGovernance")
            || unauthorized_text.contains("permission"),
        "unexpected unauthorized error: {unauthorized_text}"
    );
    let after_unauthorized =
        wait_for_route_states(&network, &[(&key, None), (&successor_key, None)]).await?;
    assert_eq!(after_unauthorized, initial_registry);

    alice.submit_blocking(register_action(route.clone()))?;
    let registered_registry = wait_for_route_states(
        &network,
        &[
            (&key, Some(SccpRouteActivationV1::Staged)),
            (&successor_key, None),
        ],
    )
    .await?;

    let stale = ApplySccpRouteGovernance::new(SccpRouteGovernanceActionV1::SetActivation(
        SccpSetRouteActivationV1 {
            key: key.clone(),
            expected_current: SccpRouteActivationV1::Paused,
            next: SccpRouteActivationV1::Bidirectional,
            inbound_finality_cutoff: None,
        },
    ));
    let stale_error = alice
        .submit_blocking(stale)
        .expect_err("a stale activation compare-and-swap must be rejected");
    let stale_error_text = error_chain_text(&stale_error);
    assert!(
        stale_error_text.contains("compare-and-swap") || stale_error_text.contains("activation"),
        "unexpected stale-CAS error: {stale_error_text}"
    );
    let after_stale_activation = wait_for_route_states(
        &network,
        &[
            (&key, Some(SccpRouteActivationV1::Staged)),
            (&successor_key, None),
        ],
    )
    .await?;
    assert_eq!(after_stale_activation, registered_registry);

    alice.submit_blocking(ApplySccpRouteGovernance::new(
        SccpRouteGovernanceActionV1::SetActivation(SccpSetRouteActivationV1 {
            key: key.clone(),
            expected_current: SccpRouteActivationV1::Staged,
            next: SccpRouteActivationV1::Bidirectional,
            inbound_finality_cutoff: None,
        }),
    ))?;
    wait_for_route_states(
        &network,
        &[
            (&key, Some(SccpRouteActivationV1::Bidirectional)),
            (&successor_key, None),
        ],
    )
    .await?;

    alice.submit_blocking(register_action(successor))?;
    let before_switch = wait_for_route_states(
        &network,
        &[
            (&key, Some(SccpRouteActivationV1::Bidirectional)),
            (&successor_key, Some(SccpRouteActivationV1::Staged)),
        ],
    )
    .await?;

    let stale_switch = ApplySccpRouteGovernance::new(SccpRouteGovernanceActionV1::SwitchRevision(
        SccpSwitchRouteRevisionV1 {
            previous_key: key.clone(),
            expected_previous: SccpRouteActivationV1::Paused,
            previous_next: SccpRouteActivationV1::InboundOnly,
            previous_inbound_finality_cutoff: None,
            successor_key: successor_key.clone(),
            successor_next: SccpRouteActivationV1::Bidirectional,
        },
    ));
    let stale_switch_error = alice
        .submit_blocking(stale_switch)
        .expect_err("a stale revision switch must reject without changing either route");
    let stale_switch_error_text = error_chain_text(&stale_switch_error);
    assert!(
        stale_switch_error_text.contains("compare-and-swap")
            || stale_switch_error_text.contains("revision-switch"),
        "unexpected stale-switch error: {stale_switch_error_text}"
    );
    let after_stale_switch = wait_for_route_states(
        &network,
        &[
            (&key, Some(SccpRouteActivationV1::Bidirectional)),
            (&successor_key, Some(SccpRouteActivationV1::Staged)),
        ],
    )
    .await?;
    assert_eq!(after_stale_switch, before_switch);

    alice.submit_blocking(ApplySccpRouteGovernance::new(
        SccpRouteGovernanceActionV1::SwitchRevision(SccpSwitchRouteRevisionV1 {
            previous_key: key.clone(),
            expected_previous: SccpRouteActivationV1::Bidirectional,
            previous_next: SccpRouteActivationV1::InboundOnly,
            previous_inbound_finality_cutoff: None,
            successor_key: successor_key.clone(),
            successor_next: SccpRouteActivationV1::Bidirectional,
        }),
    ))?;
    wait_for_atomic_revision_switch(&network, &key, &successor_key).await?;

    Ok(())
}
