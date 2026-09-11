//! Governed SCCP route, policy, destination binding and codec rejection tests.

use super::*;

const TEST_MAX_OUTSTANDING_LIABILITY: u128 = 1_000_000_000_000;

const fn test_max_wrapped_supply(multiplier: u64) -> u128 {
    TEST_MAX_OUTSTANDING_LIABILITY * multiplier as u128
}
use norito::codec::DecodeAll as _;
fn word_u64(value: u64) -> [u8; 32] {
    let mut word = [0; 32];
    word[24..].copy_from_slice(&value.to_be_bytes());
    word
}
fn hex32(value: &str) -> [u8; 32] {
    assert_eq!(value.len(), 64);
    let mut output = [0; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let nibble = |byte: u8| match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => panic!("non-lowercase hexadecimal test vector"),
        };
        output[index] = (nibble(pair[0]) << 4) | nibble(pair[1]);
    }
    output
}
fn verifying_key() -> SccpGroth16Bn254VerifyingKeyV1 {
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
fn bls12381_verifying_key() -> SccpGroth16Bls12381VerifyingKeyV1 {
    let mut g1 = [0_u8; 48];
    g1[0] = 0x80;
    let mut g2 = [0_u8; 96];
    g2[0] = 0x80;
    SccpGroth16Bls12381VerifyingKeyV1 {
        version: 1,
        alpha1: g1,
        beta2: g2,
        gamma2: g2,
        delta2: g2,
        ic: SccpGroth16Bls12381IcV1 {
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
fn outbound_proof_policy() -> SccpOutboundProofPolicyV1 {
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
            protocol_version: SCCP_V1_SUMERAGI_PROTOCOL_VERSION,
            chain_id_hash: sccp_sora_taira_chain_id_hash_v1(),
            epoch: 1,
            epoch_end_height: 10,
            roster_commitment: [0x78; 32],
            checkpoint_height: 5,
            checkpoint_block_hash: [0x73; 32],
            checkpoint_context_id: [0x74; 32],
            checkpoint_finality_artifact_hash: [0x75; 32],
        },
    }
}
fn ton_outbound_proof_policy() -> SccpOutboundProofPolicyV1 {
    SccpOutboundProofPolicyV1 {
        version: 1,
        semantic_profile: SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bls12381(
            SccpGroth16Bls12381SemanticCircuitV1 {
                version: 1,
                circuit_commitment: [0x76; 32],
                witness_generator_commitment: [0x77; 32],
                public_signal_schema_hash: sccp_groth16_bls12381_public_signal_schema_hash_v1(),
            },
        ),
        sora_finality_anchor: outbound_proof_policy().sora_finality_anchor,
    }
}
fn sora_outbound_execution_policy() -> SccpSoraOutboundExecutionPolicyV1 {
    SccpSoraOutboundExecutionPolicyV1 {
        version: 1,
        semantics: SCCP_V1_SORA_OUTBOUND_EXECUTION_SEMANTICS.to_owned(),
        contract_artifact_sha256: [0xb1; 32],
        vk_ref: SccpPortableVerifyingKeyRefV1 {
            backend: "stark/fri/v1".to_owned(),
            name: "ivm-execution-v1".to_owned(),
            version: 1,
            commitment: [0xb2; 32],
        },
        gas_limit: 50_000_000,
    }
}
fn lane() -> SccpLaneIdV1 {
    SccpLaneIdV1 {
        source: SccpNetworkV1::EthereumMainnet,
        target: SccpNetworkV1::SoraTaira,
    }
}
fn deployment(revision: u32) -> SccpEvmDestinationDeploymentV1 {
    let key = verifying_key();
    let key_hash =
        sccp_groth16_bn254_verifying_key_hash_v1(key).expect("valid structural verification key");
    let revision_byte = u8::try_from(revision).expect("test revision fits u8");
    SccpEvmDestinationDeploymentV1 {
        token_address: [0x10_u8.wrapping_add(revision_byte); 20],
        token_code_hash: [0x20_u8.wrapping_add(revision_byte); 32],
        verifier_address: [0x30_u8.wrapping_add(revision_byte); 20],
        verifier_code_hash: [0x40_u8.wrapping_add(revision_byte); 32],
        verifying_key: key,
        verifier_key_hash: key_hash,
        outbound_proof_policy: outbound_proof_policy(),
        route_address: [0x50_u8.wrapping_add(revision_byte); 20],
        route_code_hash: [0x60_u8.wrapping_add(revision_byte); 32],
        replay_verifier_address: [0x70_u8.wrapping_add(revision_byte); 20],
        replay_verifier_code_hash: [0x80_u8.wrapping_add(revision_byte); 32],
        mint_breaker_address: [0x90_u8.wrapping_add(revision_byte); 20],
        mint_breaker_code_hash: [0xa0_u8.wrapping_add(revision_byte); 32],
        taira_to_token_multiplier: SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER,
        max_wrapped_supply: test_max_wrapped_supply(SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER),
    }
}
fn tron_deployment() -> SccpTronDestinationDeploymentV1 {
    let key = verifying_key();
    SccpTronDestinationDeploymentV1 {
        token_address: [0x11; 20],
        token_code_hash: [0x21; 32],
        verifier_address: [0x31; 20],
        verifier_code_hash: [0x41; 32],
        verifying_key: key,
        verifier_key_hash: sccp_groth16_bn254_verifying_key_hash_v1(key)
            .expect("valid structural verification key"),
        outbound_proof_policy: outbound_proof_policy(),
        route_address: [0x51; 20],
        route_code_hash: [0x61; 32],
        replay_verifier_address: [0x71; 20],
        replay_verifier_code_hash: [0x81; 32],
        mint_breaker_address: [0x91; 20],
        mint_breaker_code_hash: [0xa1; 32],
        taira_to_token_multiplier: SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER,
        max_wrapped_supply: test_max_wrapped_supply(SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER),
    }
}
fn tron_lane(source: SccpNetworkV1) -> SccpLaneIdV1 {
    SccpLaneIdV1 {
        source,
        target: SccpNetworkV1::SoraTaira,
    }
}
fn tron_route(
    source: SccpNetworkV1,
    revision: u32,
    activation: SccpRouteActivationV1,
    deployment: &SccpTronDestinationDeploymentV1,
) -> SccpGovernedRouteV1 {
    let lane = tron_lane(source);
    let destination = SccpDestinationDeploymentV1::Tron(*deployment);
    let route_config_hash = destination
        .route_configuration_hash(
            lane,
            "taira_tron_xor",
            "xor",
            revision,
            SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
        )
        .expect("valid exact TRON route configuration");
    SccpGovernedRouteV1 {
        lane_id: lane,
        route_id: "taira_tron_xor".to_owned(),
        asset_key: "xor".to_owned(),
        revision,
        activation,
        inbound_finality_cutoff: activation
            .is_terminal()
            .then_some(SccpInboundFinalityCutoffV1 {
                trust_anchor_hash: [0xd1; 32],
                max_anchor_interval_height: 100,
            }),
        source_identity: SccpSourceIdentityV1 {
            lane,
            emitter: SccpSourceEmitterV1::Tron(SccpTronSourceEmitterV1 {
                address: deployment.route_address,
                runtime_code_hash: deployment.route_code_hash,
                route_config_hash,
            }),
        },
        destination,
        sora_outbound_execution_policy: sora_outbound_execution_policy(),
        settlement: SccpSoraSettlementV1 {
            asset_definition_id: sccp_v1_taira_xor_asset_definition_id(),
            payload_amount_scale: SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            max_outstanding_liability: TEST_MAX_OUTSTANDING_LIABILITY,
        },
    }
}
fn tron_anchor(height: u64) -> SccpNativeTrustAnchorV1 {
    SccpNativeTrustAnchorV1 {
        backend: BridgeNativeProofBackendV1::TronDpos,
        anchor_hash: [0xd1; 32],
        checkpoint_height: height,
    }
}
fn ton_address(byte: u8) -> SccpTonAddressV1 {
    SccpTonAddressV1 {
        workchain: 0,
        account: [byte; 32],
    }
}
fn ton_lane(source: SccpNetworkV1) -> SccpLaneIdV1 {
    SccpLaneIdV1 {
        source,
        target: SccpNetworkV1::SoraTaira,
    }
}
fn ton_deployment() -> SccpTonDestinationDeploymentV1 {
    let verifying_key = bls12381_verifying_key();
    SccpTonDestinationDeploymentV1 {
        jetton_master_address: ton_address(0x81),
        jetton_master_code_hash: [0x91; 32],
        jetton_master_initial_data_hash: [0x89; 32],
        jetton_wallet_code_hash: [0x92; 32],
        route_address: ton_address(0x82),
        route_code_hash: [0x93; 32],
        route_initial_data_hash: [0x8a; 32],
        embedded_verifier_code_hash: [0x94; 32],
        verifier_circuit_hash: [0x76; 32],
        verifying_key,
        verifier_key_hash: sccp_groth16_bls12381_verifying_key_hash_v1(verifying_key)
            .expect("structurally valid BLS12-381 key"),
        proof_profile_commitment: sccp_ton_groth16_bls12381_proof_profile_commitment_v1(),
        mint_breaker_guardian_keys: [[0x01; 32], [0x02; 32], [0x03; 32], [0x04; 32], [0x05; 32]]
            .into(),
        outbound_proof_policy: ton_outbound_proof_policy(),
        taira_to_token_multiplier: SCCP_V1_TAIRA_TO_TON_TOKEN_MULTIPLIER,
        max_wrapped_supply: test_max_wrapped_supply(SCCP_V1_TAIRA_TO_TON_TOKEN_MULTIPLIER),
    }
}
fn ton_route(
    source: SccpNetworkV1,
    revision: u32,
    activation: SccpRouteActivationV1,
    deployment: &SccpTonDestinationDeploymentV1,
) -> SccpGovernedRouteV1 {
    let lane = ton_lane(source);
    let destination = SccpDestinationDeploymentV1::Ton(*deployment);
    let route_config_hash = destination
        .route_configuration_hash(
            lane,
            "taira_ton_xor",
            "xor",
            revision,
            SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
        )
        .expect("valid exact TON route configuration");
    SccpGovernedRouteV1 {
        lane_id: lane,
        route_id: "taira_ton_xor".to_owned(),
        asset_key: "xor".to_owned(),
        revision,
        activation,
        inbound_finality_cutoff: activation
            .is_terminal()
            .then_some(SccpInboundFinalityCutoffV1 {
                trust_anchor_hash: [0xe1; 32],
                max_anchor_interval_height: 100,
            }),
        source_identity: SccpSourceIdentityV1 {
            lane,
            emitter: SccpSourceEmitterV1::Ton(SccpTonSourceEmitterV1 {
                address: deployment.route_address,
                code_hash: deployment.route_code_hash,
                route_config_hash,
            }),
        },
        destination,
        sora_outbound_execution_policy: sora_outbound_execution_policy(),
        settlement: SccpSoraSettlementV1 {
            asset_definition_id: sccp_v1_taira_xor_asset_definition_id(),
            payload_amount_scale: SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            max_outstanding_liability: TEST_MAX_OUTSTANDING_LIABILITY,
        },
    }
}
fn ton_anchor(height: u64) -> SccpNativeTrustAnchorV1 {
    SccpNativeTrustAnchorV1 {
        backend: BridgeNativeProofBackendV1::TonMasterchain,
        anchor_hash: [0xe1; 32],
        checkpoint_height: height,
    }
}
fn route(revision: u32, activation: SccpRouteActivationV1) -> SccpGovernedRouteV1 {
    let lane = lane();
    let deployment = deployment(revision);
    let destination = SccpDestinationDeploymentV1::Evm(deployment);
    let route_config_hash = destination
        .route_configuration_hash(
            lane,
            "taira_eth_xor",
            "xor",
            revision,
            SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
        )
        .expect("valid exact route configuration");
    SccpGovernedRouteV1 {
        lane_id: lane,
        route_id: "taira_eth_xor".to_owned(),
        asset_key: "xor".to_owned(),
        revision,
        activation,
        inbound_finality_cutoff: activation
            .is_terminal()
            .then_some(SccpInboundFinalityCutoffV1 {
                trust_anchor_hash: [0x91; 32],
                max_anchor_interval_height: 100,
            }),
        source_identity: SccpSourceIdentityV1 {
            lane,
            emitter: SccpSourceEmitterV1::Evm(SccpEvmSourceEmitterV1 {
                address: deployment.route_address,
                runtime_code_hash: deployment.route_code_hash,
                route_config_hash,
            }),
        },
        destination,
        sora_outbound_execution_policy: sora_outbound_execution_policy(),
        settlement: SccpSoraSettlementV1 {
            asset_definition_id: sccp_v1_taira_xor_asset_definition_id(),
            payload_amount_scale: SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            max_outstanding_liability: TEST_MAX_OUTSTANDING_LIABILITY,
        },
    }
}
fn network_id(byte: u8) -> NetworkId {
    NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([byte; iroha_crypto::Hash::LENGTH]),
        ),
    )
}
#[test]
fn governed_route_checks_every_number_encoded_u64_role() {
    let maximum = crate::parliament_types::FIRST_RELEASE_MAX_EXACT_JSON_U64;
    let hostile = maximum + 1;
    assert_eq!(
        route(1, SccpRouteActivationV1::Staged)
            .first_release_exact_json_u64_invariant_error(maximum),
        None
    );

    let mut cases = Vec::new();
    let mut cutoff = route(1, SccpRouteActivationV1::Retired);
    cutoff
        .inbound_finality_cutoff
        .as_mut()
        .expect("retired route cutoff")
        .max_anchor_interval_height = hostile;
    cases.push(cutoff);

    let mut gas = route(1, SccpRouteActivationV1::Staged);
    gas.sora_outbound_execution_policy.gas_limit = hostile;
    cases.push(gas);

    let mut finality_epoch = route(1, SccpRouteActivationV1::Staged);
    let SccpDestinationDeploymentV1::Evm(deployment) = &mut finality_epoch.destination else {
        unreachable!("fixture is EVM")
    };
    deployment.outbound_proof_policy.sora_finality_anchor.epoch = hostile;
    cases.push(finality_epoch);

    let mut finality_epoch_end = route(1, SccpRouteActivationV1::Staged);
    let SccpDestinationDeploymentV1::Evm(deployment) = &mut finality_epoch_end.destination else {
        unreachable!("fixture is EVM")
    };
    deployment
        .outbound_proof_policy
        .sora_finality_anchor
        .epoch_end_height = hostile;
    cases.push(finality_epoch_end);

    let mut finality_checkpoint = route(1, SccpRouteActivationV1::Staged);
    let SccpDestinationDeploymentV1::Evm(deployment) = &mut finality_checkpoint.destination else {
        unreachable!("fixture is EVM")
    };
    deployment
        .outbound_proof_policy
        .sora_finality_anchor
        .checkpoint_height = hostile;
    deployment
        .outbound_proof_policy
        .sora_finality_anchor
        .epoch_end_height = hostile;
    cases.push(finality_checkpoint);

    let mut evm_multiplier = route(1, SccpRouteActivationV1::Staged);
    let SccpDestinationDeploymentV1::Evm(deployment) = &mut evm_multiplier.destination else {
        unreachable!("fixture is EVM")
    };
    deployment.taira_to_token_multiplier = hostile;
    cases.push(evm_multiplier);

    let mut tron_multiplier = route(1, SccpRouteActivationV1::Staged);
    let mut tron = tron_deployment();
    tron.taira_to_token_multiplier = hostile;
    tron_multiplier.destination = SccpDestinationDeploymentV1::Tron(tron);
    cases.push(tron_multiplier);

    for candidate in cases {
        assert!(
            candidate
                .first_release_exact_json_u64_invariant_error(maximum)
                .is_some()
        );
    }

    let mut registration = crate::isi::bridge::SccpRouteGovernanceActionV1::Register(
        crate::isi::bridge::SccpRegisterRouteV1 {
            route: route(1, SccpRouteActivationV1::Staged),
            native_trust_anchor: Some(SccpNativeTrustAnchorV1 {
                backend: BridgeNativeProofBackendV1::EthereumBeacon,
                anchor_hash: [0x91; 32],
                checkpoint_height: maximum,
            }),
        },
    );
    assert_eq!(
        registration.first_release_exact_json_u64_invariant_error(maximum),
        None
    );
    let crate::isi::bridge::SccpRouteGovernanceActionV1::Register(payload) = &mut registration
    else {
        unreachable!()
    };
    payload
        .native_trust_anchor
        .as_mut()
        .expect("registration anchor")
        .checkpoint_height = hostile;
    assert!(
        registration
            .first_release_exact_json_u64_invariant_error(maximum)
            .is_some()
    );
}
#[test]
fn route_escrow_derivation_is_stable_and_exact_network_bound() {
    let first_route = route(1, SccpRouteActivationV1::Staged);
    let second_route = route(2, SccpRouteActivationV1::Staged);
    let asset = first_route.settlement.asset_definition_id.clone();
    let first = sccp_route_escrow_account_id_v1(&network_id(0x31), &first_route.key(), &asset);
    assert_eq!(
        first,
        sccp_route_escrow_account_id_v1(&network_id(0x31), &first_route.key(), &asset)
    );
    assert_ne!(
        first,
        sccp_route_escrow_account_id_v1(&network_id(0x32), &first_route.key(), &asset),
        "the same display route on another genesis lineage needs distinct escrow"
    );
    assert_ne!(
        first,
        sccp_route_escrow_account_id_v1(&network_id(0x31), &second_route.key(), &asset),
        "immutable route revisions must not share custody"
    );
}
fn anchor(height: u64) -> SccpNativeTrustAnchorV1 {
    SccpNativeTrustAnchorV1 {
        backend: BridgeNativeProofBackendV1::EthereumBeacon,
        anchor_hash: [0x91; 32],
        checkpoint_height: height,
    }
}
fn registry(
    routes: Vec<SccpGovernedRouteV1>,
    native_trust_anchor: Option<SccpNativeTrustAnchorV1>,
) -> SccpRegistryV1 {
    registry_for_lane(lane(), routes, native_trust_anchor)
}
fn registry_for_lane(
    lane_id: SccpLaneIdV1,
    routes: Vec<SccpGovernedRouteV1>,
    native_trust_anchor: Option<SccpNativeTrustAnchorV1>,
) -> SccpRegistryV1 {
    let native_trust_anchors = native_trust_anchor.into_iter().collect();
    SccpRegistryV1 {
        version: 1,
        lanes: vec![SccpGovernedLaneV1 {
            lane_id,
            native_trust_anchors,
            current_native_trust_anchor_hash: native_trust_anchor.map(|anchor| anchor.anchor_hash),
            routes,
        }],
    }
}

fn insert_unknown_json_field(value: &mut norito::json::Value, path: &[&str]) {
    let mut current = value;
    for field in path {
        let norito::json::Value::Object(object) = current else {
            panic!("JSON path component `{field}` is not an object")
        };
        current = object
            .get_mut(*field)
            .unwrap_or_else(|| panic!("JSON path component `{field}` is absent"));
    }
    let norito::json::Value::Object(object) = current else {
        panic!("JSON target at {path:?} is not an object")
    };
    object.insert(
        "adversarial_extension".to_owned(),
        norito::json::Value::Null,
    );
}
#[test]
fn solidity_verifying_key_hash_vector_is_exact() {
    let key = verifying_key();
    assert_eq!(
        canonical_sccp_groth16_bn254_verifying_key_bytes_v1(key)
            .expect("canonical key")
            .len(),
        38 * 32
    );
    assert_eq!(
        sccp_groth16_bn254_verifying_key_hash_v1(key).expect("canonical key hash"),
        hex32("6923e63427820ab42cc16c3c2bc0eb4097577919bb3911ea50cbb4f20cebfddb")
    );
}
#[test]
fn network_tags_and_tron_route_hash_match_exact_contract_vectors() {
    assert_eq!(sccp_network_tag_v1(SccpNetworkV1::SoraTaira), 0x40);
    assert_eq!(sccp_network_tag_v1(SccpNetworkV1::EthereumMainnet), 0x41);
    assert_eq!(sccp_network_tag_v1(SccpNetworkV1::BscMainnet), 0x42);
    assert_eq!(sccp_network_tag_v1(SccpNetworkV1::TronMainnet), 0x43);
    assert_eq!(sccp_network_tag_v1(SccpNetworkV1::TonMainnet), 0x44);
    assert_eq!(
        canonical_sccp_network_bytes_v1(SccpNetworkV1::TronMainnet),
        vec![0x01, 0x43, 0x05, 0x00, 0x00, 0x00, 0xdc, 0x53, 0x66, 0x2b]
    );
    let inbound_lane = SccpLaneIdV1 {
        source: SccpNetworkV1::TronMainnet,
        target: SccpNetworkV1::SoraTaira,
    };
    let outbound_lane = SccpLaneIdV1 {
        source: inbound_lane.target,
        target: inbound_lane.source,
    };
    let source_lane_hash = sccp_lane_id_hash_v1(inbound_lane).expect("valid inbound lane");
    let destination_lane_hash = sccp_lane_id_hash_v1(outbound_lane).expect("valid outbound lane");
    assert_eq!(
        source_lane_hash,
        hex32("d60bbf61a0b7476e8686e28ef0f4e085b904362cb2e7e0807100dfa8b14d243a")
    );
    assert_eq!(
        destination_lane_hash,
        hex32("d5f7a0caf114d9af169ecf0871fcc6683301ffda7276cbf3944e7a6a12056b72")
    );
    let deployment = tron_deployment();
    let route_hash = sccp_exact_tron_xor_route_config_hash_v1(
        SccpNetworkV1::TronMainnet,
        source_lane_hash,
        destination_lane_hash,
        &deployment,
        7,
    )
    .expect("valid exact TRON route");
    assert_eq!(
        route_hash,
        hex32("6b237ceca900d81735f4c4cb72257d0d8b225c5d39ee2ff6a40414b9077479cf")
    );
    let changed_cap = SccpTronDestinationDeploymentV1 {
        max_wrapped_supply: deployment.max_wrapped_supply - 1,
        ..deployment
    };
    assert_ne!(
        route_hash,
        sccp_exact_tron_xor_route_config_hash_v1(
            SccpNetworkV1::TronMainnet,
            source_lane_hash,
            destination_lane_hash,
            &changed_cap,
            7,
        )
        .expect("positive changed cap remains hashable")
    );
}
#[test]
fn ton_network_and_raw_address_encodings_bind_exact_zero_states() {
    let mut expected = vec![1, sccp_network_tag_v1(SccpNetworkV1::TonMainnet)];
    expected.extend_from_slice(&SCCP_DOMAIN_TON.to_le_bytes());
    expected.extend_from_slice(&SCCP_TON_MAINNET_GLOBAL_ID_V1.to_le_bytes());
    expected.extend_from_slice(&SCCP_TON_MASTERCHAIN_WORKCHAIN_V1.to_le_bytes());
    expected.extend_from_slice(&SCCP_TON_MASTERCHAIN_SHARD_V1.to_le_bytes());
    expected.extend_from_slice(&SCCP_TON_ZERO_STATE_SEQNO_V1.to_le_bytes());
    expected.extend_from_slice(&SCCP_TON_MAINNET_ZERO_STATE_ROOT_HASH_V1);
    expected.extend_from_slice(&SCCP_TON_MAINNET_ZERO_STATE_FILE_HASH_V1);
    assert_eq!(expected.len(), 90);
    assert_eq!(
        canonical_sccp_network_bytes_v1(SccpNetworkV1::TonMainnet),
        expected
    );
    let address = SccpTonAddressV1 {
        workchain: -1,
        account: [0xa5; 32],
    };
    let raw =
        canonical_sccp_ton_raw_address_bytes_v1(address).expect("nonzero TON raw address encodes");
    assert_eq!(&raw[..4], &(-1_i32).to_be_bytes());
    assert_eq!(&raw[4..], &[0xa5; 32]);
    assert!(
        canonical_sccp_ton_raw_address_bytes_v1(SccpTonAddressV1 {
            account: [0; 32],
            ..address
        })
        .is_none()
    );
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one adversarial matrix covers every TON deployment and source role"
)]
fn ton_deployment_route_and_source_bindings_are_closed_and_fail_safe() {
    let lane = ton_lane(SccpNetworkV1::TonMainnet);
    let deployment = ton_deployment();
    let destination = SccpDestinationDeploymentV1::Ton(deployment);
    destination
        .validate_for_lane(lane)
        .expect("complete TON deployment validates");
    assert_eq!(
        destination.verifier_key_hash(),
        deployment.verifier_key_hash
    );
    assert_eq!(
        destination.verifier_key_hash(),
        deployment.verifier_key_hash
    );
    assert_eq!(
        destination.outbound_proof_policy(),
        ton_outbound_proof_policy()
    );
    assert_eq!(
        destination.validate_for_lane(tron_lane(SccpNetworkV1::TronMainnet)),
        Err(SccpRouteValidationError::DestinationFamilyMismatch)
    );

    let binding = sccp_ton_destination_binding_hash_v1(lane.source, &deployment)
        .expect("TON destination binding");
    let governed_deployment_bytes =
        norito::to_bytes(&deployment).expect("governed TON deployment must encode canonically");
    let mut changed_master_data = deployment;
    changed_master_data.jetton_master_initial_data_hash[0] ^= 1;
    assert_eq!(
        binding,
        sccp_ton_destination_binding_hash_v1(lane.source, &changed_master_data)
            .expect("post-deployment TON master data must not form a binding fixed point")
    );
    assert_ne!(
        governed_deployment_bytes,
        norito::to_bytes(&changed_master_data).expect("changed TON master data must encode")
    );
    let mut changed_route_data = deployment;
    changed_route_data.route_initial_data_hash[0] ^= 1;
    assert_eq!(
        binding,
        sccp_ton_destination_binding_hash_v1(lane.source, &changed_route_data)
            .expect("post-deployment TON route data must not form a binding fixed point")
    );
    assert_ne!(
        governed_deployment_bytes,
        norito::to_bytes(&changed_route_data).expect("changed TON route data must encode")
    );
    let changed_addresses = SccpTonDestinationDeploymentV1 {
        jetton_master_address: ton_address(0x85),
        route_address: ton_address(0x83),
        ..deployment
    };
    assert_eq!(
        binding,
        sccp_ton_destination_binding_hash_v1(lane.source, &changed_addresses)
            .expect("post-StateInit TON addresses must not form a binding fixed point")
    );
    assert_ne!(
        governed_deployment_bytes,
        norito::to_bytes(&changed_addresses).expect("changed TON addresses must encode")
    );
    let predeployment = SccpTonDestinationDeploymentV1 {
        jetton_master_address: SccpTonAddressV1 {
            workchain: 0,
            account: [0; 32],
        },
        jetton_master_initial_data_hash: [0; 32],
        route_address: SccpTonAddressV1 {
            workchain: 0,
            account: [0; 32],
        },
        route_initial_data_hash: [0; 32],
        ..deployment
    };
    assert_eq!(
        binding,
        sccp_ton_destination_binding_hash_v1(lane.source, &predeployment)
            .expect("TON primitive binding must be derivable before StateInit")
    );
    assert_eq!(
        SccpDestinationDeploymentV1::Ton(predeployment).validate_for_lane(lane),
        Err(SccpRouteValidationError::InvalidTonAddress)
    );
    let source_lane_hash = sccp_lane_id_hash_v1(lane).expect("TON source lane");
    let destination_lane_hash = sccp_lane_id_hash_v1(SccpLaneIdV1 {
        source: lane.target,
        target: lane.source,
    })
    .expect("TON destination lane");
    let route_hash = sccp_exact_ton_xor_route_config_hash_v1(
        lane.source,
        source_lane_hash,
        destination_lane_hash,
        &deployment,
        1,
    )
    .expect("TON route config");
    assert_ne!(binding, route_hash);
    assert_eq!(
        route_hash,
        sccp_exact_ton_xor_route_config_hash_v1(
            lane.source,
            source_lane_hash,
            destination_lane_hash,
            &changed_master_data,
            1,
        )
        .expect("post-deployment TON master data must not form a route-config fixed point")
    );
    assert_eq!(
        route_hash,
        sccp_exact_ton_xor_route_config_hash_v1(
            lane.source,
            source_lane_hash,
            destination_lane_hash,
            &changed_route_data,
            1,
        )
        .expect("post-deployment TON route data must not form a route-config fixed point")
    );
    assert_eq!(
        route_hash,
        sccp_exact_ton_xor_route_config_hash_v1(
            lane.source,
            source_lane_hash,
            destination_lane_hash,
            &changed_addresses,
            1,
        )
        .expect("post-StateInit TON addresses must not form a route-config fixed point")
    );
    assert_eq!(
        route_hash,
        sccp_exact_ton_xor_route_config_hash_v1(
            lane.source,
            source_lane_hash,
            destination_lane_hash,
            &predeployment,
            1,
        )
        .expect("TON route config must be derivable before StateInit")
    );
    assert_eq!(
        sccp_exact_ton_xor_route_config_hash_v1(
            lane.source,
            source_lane_hash,
            destination_lane_hash,
            &deployment,
            0,
        ),
        Err(SccpRouteValidationError::InvalidRouteRevision)
    );
    assert_eq!(
        sccp_exact_ton_xor_route_config_hash_v1(
            SccpNetworkV1::EthereumMainnet,
            source_lane_hash,
            destination_lane_hash,
            &deployment,
            1,
        ),
        Err(SccpRouteValidationError::DestinationFamilyMismatch)
    );

    for invalid in [
        SccpTonDestinationDeploymentV1 {
            jetton_master_address: SccpTonAddressV1 {
                workchain: -1,
                ..deployment.jetton_master_address
            },
            ..deployment
        },
        SccpTonDestinationDeploymentV1 {
            route_address: deployment.jetton_master_address,
            ..deployment
        },
        SccpTonDestinationDeploymentV1 {
            jetton_master_code_hash: [0; 32],
            ..deployment
        },
        SccpTonDestinationDeploymentV1 {
            jetton_master_initial_data_hash: [0; 32],
            ..deployment
        },
        SccpTonDestinationDeploymentV1 {
            jetton_master_initial_data_hash: deployment.route_initial_data_hash,
            ..deployment
        },
        SccpTonDestinationDeploymentV1 {
            jetton_wallet_code_hash: deployment.jetton_master_code_hash,
            ..deployment
        },
        SccpTonDestinationDeploymentV1 {
            route_initial_data_hash: deployment.jetton_master_initial_data_hash,
            ..deployment
        },
        SccpTonDestinationDeploymentV1 {
            route_code_hash: deployment.embedded_verifier_code_hash,
            ..deployment
        },
        SccpTonDestinationDeploymentV1 {
            verifier_circuit_hash: deployment.verifier_key_hash,
            ..deployment
        },
        SccpTonDestinationDeploymentV1 {
            verifier_key_hash: [0x96; 32],
            ..deployment
        },
        SccpTonDestinationDeploymentV1 {
            verifying_key: SccpGroth16Bls12381VerifyingKeyV1 {
                alpha1: [0; 48],
                ..deployment.verifying_key
            },
            ..deployment
        },
        SccpTonDestinationDeploymentV1 {
            proof_profile_commitment: deployment.verifier_key_hash,
            ..deployment
        },
        SccpTonDestinationDeploymentV1 {
            outbound_proof_policy: outbound_proof_policy(),
            ..deployment
        },
        SccpTonDestinationDeploymentV1 {
            taira_to_token_multiplier: 2,
            ..deployment
        },
    ] {
        assert!(
            SccpDestinationDeploymentV1::Ton(invalid)
                .validate_for_lane(lane)
                .is_err(),
            "invalid TON role unexpectedly validated: {invalid:?}"
        );
    }

    let route_with_derived_hash_alias = ton_route(
        SccpNetworkV1::TonMainnet,
        1,
        SccpRouteActivationV1::Bidirectional,
        &SccpTonDestinationDeploymentV1 {
            route_initial_data_hash: binding,
            ..deployment
        },
    );
    assert_eq!(
        route_with_derived_hash_alias.validate(),
        Err(SccpRouteValidationError::RoleAlias)
    );

    let route = ton_route(
        SccpNetworkV1::TonMainnet,
        1,
        SccpRouteActivationV1::Bidirectional,
        &deployment,
    );
    route
        .validate_with_anchor(Some(ton_anchor(1)))
        .expect("production TON route and masterchain anchor validate");
    assert_eq!(
        route.destination.route_configuration_hash(
            lane,
            "taira_ton_xor",
            "xor",
            1,
            SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
        ),
        Ok(route_hash)
    );
    let mut mismatched_source = route.clone();
    let SccpSourceEmitterV1::Ton(ref mut source) = mismatched_source.source_identity.emitter else {
        unreachable!("TON fixture uses TON source emitter")
    };
    source.address = ton_address(0x84);
    assert_eq!(
        mismatched_source.validate(),
        Err(SccpRouteValidationError::SourceDestinationMismatch)
    );
    let mut mismatched_source_code = route.clone();
    let SccpSourceEmitterV1::Ton(ref mut source) = mismatched_source_code.source_identity.emitter
    else {
        unreachable!("TON fixture uses TON source emitter")
    };
    source.code_hash = [0x98; 32];
    assert_eq!(
        mismatched_source_code.validate(),
        Err(SccpRouteValidationError::SourceDestinationMismatch)
    );

    let encoded = norito::to_bytes(&destination).expect("TON destination encodes");
    assert_eq!(
        norito::decode_from_bytes::<SccpDestinationDeploymentV1>(&encoded)
            .expect("TON destination decodes"),
        destination
    );
    let source = route.source_identity.emitter;
    let source_bytes =
        canonical_sccp_source_emitter_bytes_v1(&source).expect("TON source canonical bytes");
    assert_eq!(source_bytes.len(), 102);
    assert_eq!(&source_bytes[..2], &[1, 2]);
    assert_eq!(&source_bytes[2..6], &0_i32.to_le_bytes());
    assert_eq!(&source_bytes[6..38], &[0x82; 32]);
    let source_encoded = source.encode();
    assert_eq!(
        SccpSourceEmitterV1::decode_all(&mut source_encoded.as_slice())
            .expect("TON source emitter decodes"),
        source
    );
}
#[test]
fn verifying_key_structure_and_embedded_hash_fail_closed() {
    let mut invalid_version = verifying_key();
    invalid_version.version = 2;
    assert_eq!(
        invalid_version.validate_structure(),
        Err(SccpRouteValidationError::InvalidGroth16VerifyingKey)
    );
    let mut infinity = verifying_key();
    infinity.alpha1 = SccpBn254G1PointV1 {
        x: [0; 32],
        y: [0; 32],
    };
    assert_eq!(
        infinity.validate_structure(),
        Err(SccpRouteValidationError::InvalidGroth16VerifyingKey)
    );
    let mut out_of_field = verifying_key();
    out_of_field.alpha1.x = BN254_BASE_FIELD_MODULUS_BE;
    assert_eq!(
        out_of_field.validate_structure(),
        Err(SccpRouteValidationError::InvalidGroth16VerifyingKey)
    );
    let mut mismatch = route(1, SccpRouteActivationV1::Staged);
    let SccpDestinationDeploymentV1::Evm(deployment) = &mut mismatch.destination else {
        unreachable!("fixture uses EVM")
    };
    deployment.verifier_key_hash[0] ^= 1;
    assert_eq!(
        mismatch.validate(),
        Err(SccpRouteValidationError::Groth16VerifyingKeyHashMismatch)
    );
}
#[test]
fn route_revision_is_explicit_in_config_and_source_identity() {
    let first = route(1, SccpRouteActivationV1::Staged);
    let second = route(2, SccpRouteActivationV1::Staged);
    assert_eq!(
        first
            .route_configuration_hash()
            .expect("governed route hash"),
        first
            .destination
            .route_configuration_hash(
                first.lane_id,
                &first.route_id,
                &first.asset_key,
                first.revision,
                first.settlement.payload_amount_scale,
            )
            .expect("destination contract route config")
    );
    assert_ne!(
        first.route_configuration_hash().expect("revision one hash"),
        second
            .route_configuration_hash()
            .expect("revision two hash")
    );
    assert_ne!(
        first
            .destination
            .route_configuration_hash(
                first.lane_id,
                &first.route_id,
                &first.asset_key,
                1,
                first.settlement.payload_amount_scale,
            )
            .expect("revision one route config"),
        first
            .destination
            .route_configuration_hash(
                first.lane_id,
                &first.route_id,
                &first.asset_key,
                2,
                first.settlement.payload_amount_scale,
            )
            .expect("revision two route config")
    );
    assert!(first.validate().is_ok());
    assert!(second.validate().is_ok());
    let mut invalid_source = first.clone();
    let SccpSourceEmitterV1::Evm(mut emitter) = invalid_source.source_identity.emitter else {
        panic!("fixture route must use an EVM source emitter");
    };
    emitter.runtime_code_hash[0] ^= 1;
    invalid_source.source_identity.emitter = SccpSourceEmitterV1::Evm(emitter);
    assert_eq!(
        invalid_source.route_configuration_hash(),
        Err(SccpRouteValidationError::SourceDestinationMismatch)
    );
}
#[test]
fn optional_anchor_and_draining_lifecycle_preserve_redemption() {
    assert!(
        registry(vec![route(1, SccpRouteActivationV1::Staged)], None)
            .validate()
            .is_ok()
    );
    assert_eq!(
        registry(vec![route(1, SccpRouteActivationV1::Bidirectional)], None).validate(),
        Err(SccpRouteValidationError::UnsupportedInboundActivation)
    );
    let draining = route(1, SccpRouteActivationV1::InboundOnly);
    let live = route(2, SccpRouteActivationV1::Bidirectional);
    assert!(draining.activation.allows_inbound());
    assert!(!draining.activation.allows_outbound());
    assert!(
        registry(vec![draining, live], Some(anchor(100)))
            .validate()
            .is_ok()
    );
}
#[test]
fn retained_tron_revisions_require_distinct_route_addresses_per_lane() {
    let first_deployment = tron_deployment();
    let mut second_deployment = first_deployment;
    second_deployment.verifier_address = [0x32; 20];
    second_deployment.verifier_code_hash = [0x42; 32];
    let first = tron_route(
        SccpNetworkV1::TronMainnet,
        1,
        SccpRouteActivationV1::InboundOnly,
        &first_deployment,
    );
    let second = tron_route(
        SccpNetworkV1::TronMainnet,
        2,
        SccpRouteActivationV1::Bidirectional,
        &second_deployment,
    );
    assert_ne!(
        first.destination_binding_hash().unwrap(),
        second.destination_binding_hash().unwrap(),
        "existing destination-binding uniqueness must not mask route-address reuse"
    );
    assert_ne!(
        first.route_configuration_hash().unwrap(),
        second.route_configuration_hash().unwrap(),
        "contiguous revisions must retain distinct configuration commitments"
    );
    assert_eq!(
        registry_for_lane(
            tron_lane(SccpNetworkV1::TronMainnet),
            vec![first.clone(), second],
            Some(tron_anchor(100)),
        )
        .validate(),
        Err(SccpRouteValidationError::DuplicateTronSourceAddress)
    );

    second_deployment.route_address = [0x52; 20];
    second_deployment.route_code_hash = [0x62; 32];
    let fresh_address = tron_route(
        SccpNetworkV1::TronMainnet,
        2,
        SccpRouteActivationV1::Bidirectional,
        &second_deployment,
    );
    registry_for_lane(
        tron_lane(SccpNetworkV1::TronMainnet),
        vec![first, fresh_address],
        Some(tron_anchor(100)),
    )
    .validate()
    .expect("a fresh immutable TRON deployment address is a valid successor");
}
#[test]
fn trust_anchor_history_is_append_only_and_current_selects_highest_checkpoint() {
    let first = anchor(100);
    let second = SccpNativeTrustAnchorV1 {
        anchor_hash: [0x92; 32],
        checkpoint_height: 200,
        ..first
    };
    let mut governed = registry(vec![route(1, SccpRouteActivationV1::Staged)], Some(first));
    governed.lanes[0].native_trust_anchors.push(second);
    governed.lanes[0].current_native_trust_anchor_hash = Some(second.anchor_hash);
    governed.validate().expect("append-only anchor history");
    let lane = &governed.lanes[0];
    assert_eq!(lane.current_native_trust_anchor(), Some(second));
    assert_eq!(
        lane.native_trust_anchor_by_hash(first.anchor_hash),
        Some(first)
    );
    assert_eq!(
        lane.native_trust_anchor_interval(first.anchor_hash),
        Some((first, Some(second.checkpoint_height)))
    );
    assert_eq!(
        lane.native_trust_anchor_interval(second.anchor_hash),
        Some((second, None))
    );
    let mut stale_pointer = governed.clone();
    stale_pointer.lanes[0].current_native_trust_anchor_hash = Some(first.anchor_hash);
    assert_eq!(
        stale_pointer.validate(),
        Err(SccpRouteValidationError::InvalidCurrentTrustAnchor)
    );
    let mut duplicate_hash = governed.clone();
    duplicate_hash.lanes[0]
        .native_trust_anchors
        .push(SccpNativeTrustAnchorV1 {
            anchor_hash: first.anchor_hash,
            checkpoint_height: 300,
            ..first
        });
    duplicate_hash.lanes[0].current_native_trust_anchor_hash = Some(first.anchor_hash);
    assert_eq!(
        duplicate_hash.validate(),
        Err(SccpRouteValidationError::InvalidTrustAnchorHistory)
    );
    let mut rollback = governed;
    rollback.lanes[0]
        .native_trust_anchors
        .push(SccpNativeTrustAnchorV1 {
            anchor_hash: [0x93; 32],
            checkpoint_height: 150,
            ..first
        });
    rollback.lanes[0].current_native_trust_anchor_hash = Some([0x93; 32]);
    assert_eq!(
        rollback.validate(),
        Err(SccpRouteValidationError::InvalidTrustAnchorHistory)
    );
}
#[test]
fn terminal_history_does_not_exhaust_live_route_capacity() {
    let routes = (1..=12)
        .map(|revision| {
            route(
                revision,
                if revision == 12 {
                    SccpRouteActivationV1::Staged
                } else {
                    SccpRouteActivationV1::Retired
                },
            )
        })
        .collect::<Vec<_>>();
    let first = anchor(100);
    let second = SccpNativeTrustAnchorV1 {
        anchor_hash: [0x92; 32],
        checkpoint_height: 101,
        ..first
    };
    let mut governed = registry(routes, Some(first));
    governed.lanes[0].native_trust_anchors.push(second);
    governed.lanes[0].current_native_trust_anchor_hash = Some(second.anchor_hash);
    for route in governed.lanes[0]
        .routes
        .iter_mut()
        .filter(|route| route.activation.is_terminal())
    {
        route.inbound_finality_cutoff = Some(SccpInboundFinalityCutoffV1 {
            trust_anchor_hash: first.anchor_hash,
            max_anchor_interval_height: second.checkpoint_height,
        });
    }
    governed
        .validate()
        .expect("retained terminal history below the generous cap remains valid");
    assert_eq!(governed.lanes[0].routes.len(), 12);
    let live_routes = (1..=SCCP_V1_MAX_LIVE_ROUTES_PER_LANE + 1)
        .map(|revision| {
            route(
                u32::try_from(revision).expect("test revision fits u32"),
                SccpRouteActivationV1::Staged,
            )
        })
        .collect();
    assert_eq!(
        registry(live_routes, Some(anchor(100))).validate(),
        Err(SccpRouteValidationError::InvalidLaneLiveRouteCount)
    );
}
#[test]
fn retained_history_caps_accept_exact_bounds_and_reject_one_more() {
    let first = anchor(99);
    let second = SccpNativeTrustAnchorV1 {
        anchor_hash: [0x92; 32],
        checkpoint_height: 100,
        ..first
    };
    let routes = (1..=SCCP_V1_MAX_RETAINED_ROUTES_PER_LANE)
        .map(|revision| {
            route(
                u32::try_from(revision).expect("retained route bound fits u32"),
                if revision == SCCP_V1_MAX_RETAINED_ROUTES_PER_LANE {
                    SccpRouteActivationV1::Staged
                } else {
                    SccpRouteActivationV1::Retired
                },
            )
        })
        .collect::<Vec<_>>();
    let mut exact_routes = registry(routes, Some(first));
    exact_routes.lanes[0].native_trust_anchors.push(second);
    exact_routes.lanes[0].current_native_trust_anchor_hash = Some(second.anchor_hash);
    exact_routes
        .validate()
        .expect("exact retained-route bound remains admissible");
    let mut excess_routes = exact_routes;
    excess_routes.lanes[0].routes.push(route(
        u32::try_from(SCCP_V1_MAX_RETAINED_ROUTES_PER_LANE + 1)
            .expect("retained route overflow fixture fits u32"),
        SccpRouteActivationV1::Staged,
    ));
    assert_eq!(
        excess_routes.validate(),
        Err(SccpRouteValidationError::TooManyRetainedRoutes)
    );
    let retained_anchor = |height: usize| {
        let height = u64::try_from(height).expect("retained anchor bound fits u64");
        let mut anchor_hash = [0_u8; 32];
        anchor_hash[24..].copy_from_slice(&height.to_be_bytes());
        SccpNativeTrustAnchorV1 {
            backend: BridgeNativeProofBackendV1::EthereumBeacon,
            anchor_hash,
            checkpoint_height: height,
        }
    };
    let anchors = (1..=SCCP_V1_MAX_RETAINED_NATIVE_TRUST_ANCHORS_PER_LANE)
        .map(retained_anchor)
        .collect::<Vec<_>>();
    let last_anchor_hash = anchors
        .last()
        .expect("retained-anchor bound is nonzero")
        .anchor_hash;
    let exact_anchors = SccpRegistryV1 {
        version: 1,
        lanes: vec![SccpGovernedLaneV1 {
            lane_id: lane(),
            native_trust_anchors: anchors,
            current_native_trust_anchor_hash: Some(last_anchor_hash),
            routes: vec![route(1, SccpRouteActivationV1::Staged)],
        }],
    };
    exact_anchors
        .validate()
        .expect("exact retained-anchor bound remains admissible");
    let mut excess_anchors = exact_anchors;
    let excess_anchor = retained_anchor(SCCP_V1_MAX_RETAINED_NATIVE_TRUST_ANCHORS_PER_LANE + 1);
    excess_anchors.lanes[0]
        .native_trust_anchors
        .push(excess_anchor);
    excess_anchors.lanes[0].current_native_trust_anchor_hash = Some(excess_anchor.anchor_hash);
    assert_eq!(
        excess_anchors.validate(),
        Err(SccpRouteValidationError::TooManyRetainedTrustAnchors)
    );
    let maximum_route = route(1, SccpRouteActivationV1::Retired);
    maximum_route
        .validate()
        .expect("fixed-shape V1 route remains valid");
    assert!(
        maximum_route.encode().len() <= 4_096,
        "retained-route envelope exceeded the cap-sizing assumption"
    );
    assert!(
        retained_anchor(1).encode().len() <= 64,
        "retained-anchor envelope exceeded the cap-sizing assumption"
    );
    assert_eq!(
        SCCP_V1_MAX_GOVERNED_LANES
            * (SCCP_V1_MAX_RETAINED_ROUTES_PER_LANE * 4_096
                + SCCP_V1_MAX_RETAINED_NATIVE_TRUST_ANCHORS_PER_LANE * 64),
        8 * 1024 * 1024,
        "conservative retained-entry envelope must remain eight MiB"
    );
}
#[test]
fn retired_route_cutoff_must_belong_to_one_retained_anchor_interval() {
    let first = anchor(100);
    let second = SccpNativeTrustAnchorV1 {
        anchor_hash: [0x92; 32],
        checkpoint_height: 200,
        ..first
    };
    for cutoff in [
        SccpInboundFinalityCutoffV1 {
            trust_anchor_hash: [0; 32],
            max_anchor_interval_height: 0,
        },
        SccpInboundFinalityCutoffV1 {
            trust_anchor_hash: first.anchor_hash,
            max_anchor_interval_height: second.checkpoint_height,
        },
    ] {
        let mut live = route(1, SccpRouteActivationV1::Staged);
        live.inbound_finality_cutoff = Some(cutoff);
        assert_eq!(
            live.validate(),
            Err(SccpRouteValidationError::InvalidInboundFinalityCutoff),
            "nonterminal route carried cutoff {cutoff:?}"
        );
    }
    let mut governed = registry(vec![route(1, SccpRouteActivationV1::Retired)], Some(first));
    governed.lanes[0].native_trust_anchors.push(second);
    governed.lanes[0].current_native_trust_anchor_hash = Some(second.anchor_hash);
    governed.lanes[0].routes[0].inbound_finality_cutoff = Some(SccpInboundFinalityCutoffV1 {
        trust_anchor_hash: first.anchor_hash,
        max_anchor_interval_height: second.checkpoint_height,
    });
    governed
        .validate()
        .expect("cutoff at the inclusive successor checkpoint is valid");
    for cutoff in [
        None,
        Some(SccpInboundFinalityCutoffV1 {
            trust_anchor_hash: [0xFF; 32],
            max_anchor_interval_height: 150,
        }),
        Some(SccpInboundFinalityCutoffV1 {
            trust_anchor_hash: first.anchor_hash,
            max_anchor_interval_height: 99,
        }),
        Some(SccpInboundFinalityCutoffV1 {
            trust_anchor_hash: first.anchor_hash,
            max_anchor_interval_height: second.checkpoint_height - 1,
        }),
        Some(SccpInboundFinalityCutoffV1 {
            trust_anchor_hash: first.anchor_hash,
            max_anchor_interval_height: second.checkpoint_height + 1,
        }),
        Some(SccpInboundFinalityCutoffV1 {
            trust_anchor_hash: second.anchor_hash,
            max_anchor_interval_height: second.checkpoint_height + 1,
        }),
    ] {
        let mut hostile = governed.clone();
        hostile.lanes[0].routes[0].inbound_finality_cutoff = cutoff;
        assert_eq!(
            hostile.validate(),
            Err(SccpRouteValidationError::InvalidInboundFinalityCutoff)
        );
    }
}
#[test]
fn multiple_outbound_revisions_and_revision_gaps_are_rejected() {
    assert_eq!(
        registry(
            vec![
                route(1, SccpRouteActivationV1::Bidirectional),
                route(2, SccpRouteActivationV1::Bidirectional),
            ],
            Some(anchor(100)),
        )
        .validate(),
        Err(SccpRouteValidationError::MultipleEnabledRevisions)
    );
    assert_eq!(
        registry(
            vec![route(2, SccpRouteActivationV1::Staged)],
            Some(anchor(100))
        )
        .validate(),
        Err(SccpRouteValidationError::InvalidRouteRevision)
    );
}
#[test]
fn activation_transitions_enforce_drain_before_retirement() {
    use SccpRouteActivationV1 as A;
    assert!(A::Staged.can_transition_to(A::Bidirectional));
    assert!(A::Staged.can_transition_to(A::InboundOnly));
    assert!(A::Bidirectional.can_transition_to(A::InboundOnly));
    assert!(A::InboundOnly.can_transition_to(A::Retired));
    assert!(!A::Bidirectional.can_transition_to(A::Retired));
    assert!(!A::Retired.can_transition_to(A::InboundOnly));
    assert!(!A::Paused.can_transition_to(A::Paused));
}
#[test]
fn settlement_asset_scale_and_canonical_keys_are_exact() {
    let mut wrong_scale = route(1, SccpRouteActivationV1::Staged);
    wrong_scale.settlement.payload_amount_scale = 8;
    assert_eq!(
        wrong_scale.validate(),
        Err(SccpRouteValidationError::InvalidSettlementScale)
    );
    let mut wrong_asset = route(1, SccpRouteActivationV1::Staged);
    wrong_asset.settlement.asset_definition_id = AssetDefinitionId::derive_from_components(
        iroha_model_base::domain::DomainId::try_new("wrong", "universal").expect("valid domain"),
        "xor".parse().expect("valid name"),
    );
    assert_eq!(
        wrong_asset.validate(),
        Err(SccpRouteValidationError::SettlementAssetMismatch)
    );
    let mut uppercase = route(1, SccpRouteActivationV1::Staged);
    uppercase.asset_key = "XOR".to_owned();
    assert!(matches!(
        uppercase.validate(),
        Err(SccpRouteValidationError::NonCanonicalKey("asset_key"))
    ));
}
#[test]
fn source_destination_and_role_aliasing_fail_closed() {
    let mut source_drift = route(1, SccpRouteActivationV1::Staged);
    let SccpSourceEmitterV1::Evm(emitter) = &mut source_drift.source_identity.emitter else {
        unreachable!("fixture uses EVM")
    };
    emitter.address[0] ^= 1;
    assert_eq!(
        source_drift.validate(),
        Err(SccpRouteValidationError::SourceDestinationMismatch)
    );
    let mut alias = route(1, SccpRouteActivationV1::Staged);
    let SccpDestinationDeploymentV1::Evm(deployment) = &mut alias.destination else {
        unreachable!("fixture uses EVM")
    };
    deployment.route_address = deployment.token_address;
    assert_eq!(alias.validate(), Err(SccpRouteValidationError::RoleAlias));
}
#[test]
fn outbound_proof_policy_rejects_every_malformed_or_aliased_role() {
    let policy = outbound_proof_policy();
    policy.validate().expect("exact fixture policy");
    let profile_hash = policy.semantic_profile_hash().expect("profile hash");
    let anchor_hash = policy.sora_finality_anchor_hash().expect("anchor hash");
    assert_eq!(
        sccp_groth16_bn254_public_signal_schema_hash_v1(),
        hex32("7567439f41173d6745a3d51923cb70371acc7d66f23cefb4100d6d5d7a432cbb")
    );
    assert_eq!(
        sccp_sora_taira_chain_id_hash_v1(),
        hex32("cf1cfc0f57b0bfa4c21882a9870317a1f4812f86533897095e3944be34c5bba7")
    );
    assert_eq!(
        profile_hash,
        hex32("ce5a1e17aca3cafe47a403fd66479f0a36339eb56092dafa67c8d97bdeeb60ef")
    );
    assert_eq!(
        anchor_hash,
        hex32("e9b9a7ff38cde8cb071d473cf0c6570270118df418ad790959474daedea7a365")
    );
    let anchor_bytes = canonical_sccp_sora_finality_anchor_bytes_v1(policy.sora_finality_anchor)
        .expect("canonical epoch-aware anchor bytes");
    assert_eq!(anchor_bytes.len(), SCCP_V1_SORA_FINALITY_ANCHOR_BYTES);
    assert_eq!(&anchor_bytes[0..4], &[1, 0x40, 4, 0]);
    assert_eq!(&anchor_bytes[36..44], &1_u64.to_le_bytes());
    assert_eq!(&anchor_bytes[44..52], &10_u64.to_le_bytes());
    assert_eq!(&anchor_bytes[52..84], &[0x78; 32]);
    assert_eq!(&anchor_bytes[84..92], &5_u64.to_le_bytes());
    assert_ne!(profile_hash, [0; 32]);
    assert_ne!(anchor_hash, [0; 32]);
    assert_ne!(profile_hash, anchor_hash);
    let mut invalid = policy;
    invalid.version = 0;
    assert_eq!(
        invalid.validate(),
        Err(SccpRouteValidationError::InvalidOutboundProofPolicy)
    );
    let SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(mut circuit) =
        policy.semantic_profile
    else {
        panic!("test policy must use BN254")
    };
    circuit.circuit_commitment = [0; 32];
    invalid = policy;
    invalid.semantic_profile =
        SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(circuit);
    assert_eq!(
        invalid.validate(),
        Err(SccpRouteValidationError::InvalidSemanticProofProfile)
    );
    let SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(mut circuit) =
        policy.semantic_profile
    else {
        panic!("test policy must use BN254")
    };
    circuit.witness_generator_commitment = circuit.circuit_commitment;
    invalid = policy;
    invalid.semantic_profile =
        SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(circuit);
    assert_eq!(
        invalid.validate(),
        Err(SccpRouteValidationError::InvalidSemanticProofProfile)
    );
    let SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(mut circuit) =
        policy.semantic_profile
    else {
        panic!("test policy must use BN254")
    };
    circuit.public_signal_schema_hash[0] ^= 1;
    invalid = policy;
    invalid.semantic_profile =
        SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(circuit);
    assert_eq!(
        invalid.validate(),
        Err(SccpRouteValidationError::InvalidSemanticProofProfile)
    );
    assert_invalid_finality_anchors(&policy);
}

fn assert_invalid_finality_anchors(policy: &SccpOutboundProofPolicyV1) {
    let anchor_mutations: [fn(&mut SccpSoraFinalityAnchorV1); 14] = [
        |anchor: &mut SccpSoraFinalityAnchorV1| anchor.version = 0,
        |anchor: &mut SccpSoraFinalityAnchorV1| {
            anchor.source_network = SccpNetworkV1::EthereumMainnet;
        },
        |anchor: &mut SccpSoraFinalityAnchorV1| {
            anchor.protocol_version = SCCP_V1_SUMERAGI_PROTOCOL_VERSION - 1;
        },
        |anchor: &mut SccpSoraFinalityAnchorV1| {
            anchor.protocol_version = SCCP_V1_SUMERAGI_PROTOCOL_VERSION + 1;
        },
        |anchor: &mut SccpSoraFinalityAnchorV1| anchor.chain_id_hash[0] ^= 1,
        |anchor: &mut SccpSoraFinalityAnchorV1| anchor.epoch = 0,
        |anchor: &mut SccpSoraFinalityAnchorV1| {
            anchor.epoch_end_height = anchor.checkpoint_height - 1;
        },
        |anchor: &mut SccpSoraFinalityAnchorV1| anchor.roster_commitment = [0; 32],
        |anchor: &mut SccpSoraFinalityAnchorV1| anchor.checkpoint_height = 0,
        |anchor: &mut SccpSoraFinalityAnchorV1| anchor.checkpoint_block_hash = [0; 32],
        |anchor: &mut SccpSoraFinalityAnchorV1| {
            anchor.checkpoint_context_id = [0; 32];
        },
        |anchor: &mut SccpSoraFinalityAnchorV1| {
            anchor.checkpoint_finality_artifact_hash = [0; 32];
        },
        |anchor: &mut SccpSoraFinalityAnchorV1| {
            anchor.checkpoint_context_id = anchor.checkpoint_block_hash;
        },
        |anchor: &mut SccpSoraFinalityAnchorV1| {
            anchor.checkpoint_finality_artifact_hash = anchor.checkpoint_context_id;
        },
    ];
    for mutate in anchor_mutations {
        let mut invalid = *policy;
        mutate(&mut invalid.sora_finality_anchor);
        assert_eq!(
            invalid.validate(),
            Err(SccpRouteValidationError::InvalidSoraFinalityAnchor)
        );
    }
}

#[test]
fn ton_bls12381_profile_and_key_are_curve_separated_and_canonical() {
    let policy = ton_outbound_proof_policy();
    policy.validate().expect("TON BLS12-381 policy validates");
    assert!(policy.semantic_profile.is_bls12381());
    assert!(!policy.semantic_profile.is_bn254());
    assert_ne!(
        sccp_groth16_bls12381_public_signal_schema_hash_v1(),
        sccp_groth16_bn254_public_signal_schema_hash_v1()
    );
    let key = bls12381_verifying_key();
    key.validate_structure()
        .expect("compressed BLS12-381 fixture is structurally canonical");
    let canonical =
        canonical_sccp_groth16_bls12381_verifying_key_bytes_v1(key).expect("canonical key bytes");
    assert_eq!(canonical.len(), 1 + 48 + 3 * 96 + 12 * 48);
    assert_eq!(
        sccp_groth16_bls12381_verifying_key_hash_v1(key).expect("canonical key hash"),
        sha256_bytes(&canonical)
    );
    let mut uncompressed = key;
    uncompressed.alpha1[0] &= !0x80;
    assert_eq!(
        uncompressed.validate_structure(),
        Err(SccpRouteValidationError::InvalidGroth16VerifyingKey)
    );
    let mut infinity = key;
    infinity.alpha1[0] |= 0x40;
    assert_eq!(
        infinity.validate_structure(),
        Err(SccpRouteValidationError::InvalidGroth16VerifyingKey)
    );
    let mut wrong_schema = policy;
    let SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bls12381(ref mut circuit) =
        wrong_schema.semantic_profile
    else {
        unreachable!("TON policy uses BLS12-381")
    };
    circuit.public_signal_schema_hash = sccp_groth16_bn254_public_signal_schema_hash_v1();
    assert_eq!(
        wrong_schema.validate(),
        Err(SccpRouteValidationError::InvalidSemanticProofProfile)
    );
}
#[test]
fn canonical_destination_deployments_roundtrip_with_typed_outbound_policy() {
    let evm = deployment(1);
    let evm_bytes = norito::to_bytes(&evm).expect("canonical EVM deployment encodes");
    let decoded_evm = norito::decode_from_bytes::<SccpEvmDestinationDeploymentV1>(&evm_bytes)
        .expect("canonical EVM deployment decodes");
    assert_eq!(decoded_evm, evm);
    assert_eq!(
        norito::to_bytes(&decoded_evm).expect("decoded EVM deployment re-encodes"),
        evm_bytes
    );
    decoded_evm
        .outbound_proof_policy
        .validate()
        .expect("roundtripped EVM policy remains valid");
    let tron = tron_deployment();
    let tron_bytes = norito::to_bytes(&tron).expect("canonical TRON deployment encodes");
    let decoded_tron = norito::decode_from_bytes::<SccpTronDestinationDeploymentV1>(&tron_bytes)
        .expect("canonical TRON deployment decodes");
    assert_eq!(decoded_tron, tron);
    assert_eq!(
        norito::to_bytes(&decoded_tron).expect("decoded TRON deployment re-encodes"),
        tron_bytes
    );
    decoded_tron
        .outbound_proof_policy
        .validate()
        .expect("roundtripped TRON policy remains valid");
    assert_ton_deployment_roundtrip();
}
fn assert_ton_deployment_roundtrip() {
    let ton = ton_deployment();
    let ton_bytes = norito::to_bytes(&ton).expect("canonical TON deployment encodes");
    let decoded_ton = norito::decode_from_bytes::<SccpTonDestinationDeploymentV1>(&ton_bytes)
        .expect("canonical TON deployment decodes");
    assert_eq!(decoded_ton, ton);
    assert_eq!(
        norito::to_bytes(&decoded_ton).expect("decoded TON deployment re-encodes"),
        ton_bytes
    );
    decoded_ton
        .outbound_proof_policy
        .validate()
        .expect("roundtripped TON policy remains valid");
}
#[test]
fn policyless_norito_destination_deployments_are_rejected() {
    #[derive(norito::derive::NoritoSerialize, norito::NoritoSchema)]
    #[norito_schema(
        name = "test::iroha_data_model::bridge::sccp_registry::PolicylessEvmDestinationDeploymentV1",
        frame = "iroha_data_model::bridge::sccp_registry::SccpEvmDestinationDeploymentV1"
    )]
    struct PolicylessEvmDestinationDeploymentV1 {
        token_address: [u8; 20],
        token_code_hash: [u8; 32],
        verifier_address: [u8; 20],
        verifier_code_hash: [u8; 32],
        verifying_key: SccpGroth16Bn254VerifyingKeyV1,
        verifier_key_hash: [u8; 32],
        route_address: [u8; 20],
        route_code_hash: [u8; 32],
        taira_to_token_multiplier: u64,
    }
    #[derive(norito::derive::NoritoSerialize, norito::NoritoSchema)]
    #[norito_schema(
        name = "test::iroha_data_model::bridge::sccp_registry::PolicylessTronDestinationDeploymentV1",
        frame = "iroha_data_model::bridge::sccp_registry::SccpTronDestinationDeploymentV1"
    )]
    struct PolicylessTronDestinationDeploymentV1 {
        token_address: [u8; 20],
        token_code_hash: [u8; 32],
        verifier_address: [u8; 20],
        verifier_code_hash: [u8; 32],
        verifying_key: SccpGroth16Bn254VerifyingKeyV1,
        verifier_key_hash: [u8; 32],
        route_address: [u8; 20],
        route_code_hash: [u8; 32],
        taira_to_token_multiplier: u64,
    }
    let evm = deployment(1);
    let evm_bytes = norito::to_bytes(&PolicylessEvmDestinationDeploymentV1 {
        token_address: evm.token_address,
        token_code_hash: evm.token_code_hash,
        verifier_address: evm.verifier_address,
        verifier_code_hash: evm.verifier_code_hash,
        verifying_key: evm.verifying_key,
        verifier_key_hash: evm.verifier_key_hash,
        route_address: evm.route_address,
        route_code_hash: evm.route_code_hash,
        taira_to_token_multiplier: evm.taira_to_token_multiplier,
    })
    .expect("policy-less EVM deployment encodes");
    assert_eq!(
        norito::schema::identity::frame_hash::<PolicylessEvmDestinationDeploymentV1>(),
        norito::schema::identity::frame_hash::<SccpEvmDestinationDeploymentV1>(),
        "malformed fixture must use the production frame identity"
    );
    assert_eq!(
        norito::core::Header::read(evm_bytes.as_slice())
            .expect("read malformed fixture header")
            .schema,
        norito::schema::identity::frame_hash::<SccpEvmDestinationDeploymentV1>(),
        "malformed fixture header must reach the production decoder"
    );
    assert!(
        norito::decode_from_bytes::<SccpEvmDestinationDeploymentV1>(&evm_bytes).is_err(),
        "policy-less EVM deployment must not decode as the canonical V1 shape"
    );
    let tron = tron_deployment();
    let tron_bytes = norito::to_bytes(&PolicylessTronDestinationDeploymentV1 {
        token_address: tron.token_address,
        token_code_hash: tron.token_code_hash,
        verifier_address: tron.verifier_address,
        verifier_code_hash: tron.verifier_code_hash,
        verifying_key: tron.verifying_key,
        verifier_key_hash: tron.verifier_key_hash,
        route_address: tron.route_address,
        route_code_hash: tron.route_code_hash,
        taira_to_token_multiplier: tron.taira_to_token_multiplier,
    })
    .expect("policy-less TRON deployment encodes");
    assert_eq!(
        norito::schema::identity::frame_hash::<PolicylessTronDestinationDeploymentV1>(),
        norito::schema::identity::frame_hash::<SccpTronDestinationDeploymentV1>(),
        "malformed fixture must use the production frame identity"
    );
    assert_eq!(
        norito::core::Header::read(tron_bytes.as_slice())
            .expect("read malformed fixture header")
            .schema,
        norito::schema::identity::frame_hash::<SccpTronDestinationDeploymentV1>(),
        "malformed fixture header must reach the production decoder"
    );
    assert!(
        norito::decode_from_bytes::<SccpTronDestinationDeploymentV1>(&tron_bytes).is_err(),
        "policy-less TRON deployment must not decode as the canonical V1 shape"
    );
}

#[test]
fn policyless_json_destination_deployments_are_rejected() {
    let mut evm = norito::json::to_value(&deployment(1)).expect("serialize EVM deployment");
    let norito::json::Value::Object(evm_object) = &mut evm else {
        panic!("EVM deployment JSON is an object")
    };
    assert!(evm_object.remove("outbound_proof_policy").is_some());
    let evm_json = norito::json::to_json(&evm).expect("serialize policy-less EVM deployment");
    assert!(norito::json::from_json::<SccpEvmDestinationDeploymentV1>(&evm_json).is_err());
    let mut tron = norito::json::to_value(&tron_deployment()).expect("serialize TRON deployment");
    let norito::json::Value::Object(tron_object) = &mut tron else {
        panic!("TRON deployment JSON is an object")
    };
    assert!(tron_object.remove("outbound_proof_policy").is_some());
    let tron_json = norito::json::to_json(&tron).expect("serialize policy-less TRON deployment");
    assert!(norito::json::from_json::<SccpTronDestinationDeploymentV1>(&tron_json).is_err());
}

#[test]
fn governed_route_json_requires_exact_sora_execution_policy_and_vk_pin() {
    let route = route(1, SccpRouteActivationV1::Staged);
    let mut policyless = norito::json::to_value(&route).expect("serialize governed route");
    let norito::json::Value::Object(route_object) = &mut policyless else {
        panic!("governed route must serialize as an object")
    };
    route_object.remove("sora_outbound_execution_policy");
    let json = norito::json::to_json(&policyless).expect("serialize policy-less route");
    assert!(norito::json::from_json::<SccpGovernedRouteV1>(&json).is_err());
    for missing in ["version", "commitment"] {
        let mut hostile = norito::json::to_value(&route).expect("serialize governed route");
        let norito::json::Value::Object(route_object) = &mut hostile else {
            panic!("governed route must serialize as an object")
        };
        let norito::json::Value::Object(policy_object) = route_object
            .get_mut("sora_outbound_execution_policy")
            .expect("execution policy")
        else {
            panic!("execution policy must serialize as an object")
        };
        let norito::json::Value::Object(vk_ref) = policy_object
            .get_mut("vk_ref")
            .expect("verification-key reference")
        else {
            panic!("verification-key reference must serialize as an object")
        };
        vk_ref.remove(missing);
        let json = norito::json::to_json(&hostile).expect("serialize hostile route");
        assert!(
            norito::json::from_json::<SccpGovernedRouteV1>(&json).is_err(),
            "missing governed verification-key {missing} must reject"
        );
    }
    let exact = &route.sora_outbound_execution_policy.vk_ref;
    let id = crate::proof::VerifyingKeyId::new(exact.backend.clone(), exact.name.clone());
    assert!(exact.matches(&id, exact.version, exact.commitment));
    assert!(!exact.matches(&id, exact.version.saturating_add(1), exact.commitment));
    assert!(!exact.matches(&id, exact.version, [0x7f; 32]));
    let mut zero_version = route.clone();
    zero_version.sora_outbound_execution_policy.vk_ref.version = 0;
    assert_eq!(
        zero_version.validate(),
        Err(SccpRouteValidationError::InvalidSoraOutboundExecutionPolicy),
        "governance must not pin the unversioned registry sentinel"
    );
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one scenario verifies every semantic-profile and anchor commitment role"
)]
fn semantic_profile_and_anchor_are_committed_by_binding_and_route_hash() {
    let baseline = deployment(1);
    let baseline_binding =
        sccp_evm_destination_binding_hash_v1(lane().source, &baseline).expect("binding");
    assert_eq!(
        baseline_binding,
        hex32("0822a77973c618db69c82e50668670823f7b39eba240103833dc9e500f3811fc")
    );
    let baseline_route = SccpDestinationDeploymentV1::Evm(baseline)
        .route_configuration_hash(
            lane(),
            "taira_eth_xor",
            "xor",
            1,
            SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
        )
        .expect("route hash");
    let mut changed_profile = baseline;
    let SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(ref mut circuit) =
        changed_profile.outbound_proof_policy.semantic_profile
    else {
        panic!("test policy must use BN254")
    };
    circuit.circuit_commitment = [0x76; 32];
    changed_profile
        .outbound_proof_policy
        .validate()
        .expect("changed profile remains valid");
    assert_ne!(
        baseline_binding,
        sccp_evm_destination_binding_hash_v1(lane().source, &changed_profile)
            .expect("changed binding")
    );
    assert_ne!(
        baseline_route,
        SccpDestinationDeploymentV1::Evm(changed_profile)
            .route_configuration_hash(
                lane(),
                "taira_eth_xor",
                "xor",
                1,
                SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            )
            .expect("changed route hash")
    );
    let mut changed_anchor = baseline;
    changed_anchor
        .outbound_proof_policy
        .sora_finality_anchor
        .checkpoint_height += 1;
    changed_anchor
        .outbound_proof_policy
        .validate()
        .expect("changed anchor remains valid");
    assert_ne!(
        baseline_binding,
        sccp_evm_destination_binding_hash_v1(lane().source, &changed_anchor)
            .expect("changed anchor binding")
    );
    assert_ne!(
        baseline_route,
        SccpDestinationDeploymentV1::Evm(changed_anchor)
            .route_configuration_hash(
                lane(),
                "taira_eth_xor",
                "xor",
                1,
                SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            )
            .expect("changed anchor route hash")
    );
    let tron_lane = SccpLaneIdV1 {
        source: SccpNetworkV1::TronMainnet,
        target: SccpNetworkV1::SoraTaira,
    };
    let baseline_tron = tron_deployment();
    let baseline_tron_binding =
        sccp_tron_destination_binding_hash_v1(tron_lane.source, &baseline_tron)
            .expect("TRON binding");
    assert_eq!(
        baseline_tron_binding,
        hex32("e3a973b02a7e233698bd146af095dfb83f00438ee35d2a32d686fdc75ce8e5ec")
    );
    let baseline_tron_route = SccpDestinationDeploymentV1::Tron(baseline_tron)
        .route_configuration_hash(
            tron_lane,
            "taira_tron_xor",
            "xor",
            1,
            SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
        )
        .expect("TRON route hash");
    let mut changed_tron_profile = baseline_tron;
    let SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(ref mut circuit) =
        changed_tron_profile.outbound_proof_policy.semantic_profile
    else {
        panic!("test policy must use BN254")
    };
    circuit.circuit_commitment = [0x76; 32];
    changed_tron_profile
        .outbound_proof_policy
        .validate()
        .expect("changed TRON profile remains valid");
    assert_ne!(
        baseline_tron_binding,
        sccp_tron_destination_binding_hash_v1(tron_lane.source, &changed_tron_profile)
            .expect("changed TRON binding")
    );
    assert_ne!(
        baseline_tron_route,
        SccpDestinationDeploymentV1::Tron(changed_tron_profile)
            .route_configuration_hash(
                tron_lane,
                "taira_tron_xor",
                "xor",
                1,
                SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            )
            .expect("changed TRON route hash")
    );
    let mut changed_tron_anchor = baseline_tron;
    changed_tron_anchor
        .outbound_proof_policy
        .sora_finality_anchor
        .checkpoint_height += 1;
    changed_tron_anchor
        .outbound_proof_policy
        .validate()
        .expect("changed TRON anchor remains valid");
    assert_ne!(
        baseline_tron_binding,
        sccp_tron_destination_binding_hash_v1(tron_lane.source, &changed_tron_anchor)
            .expect("changed TRON anchor binding")
    );
    assert_ne!(
        baseline_tron_route,
        SccpDestinationDeploymentV1::Tron(changed_tron_anchor)
            .route_configuration_hash(
                tron_lane,
                "taira_tron_xor",
                "xor",
                1,
                SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            )
            .expect("changed TRON anchor route hash")
    );
}
#[test]
fn destination_bindings_commit_replay_and_breaker_roles_independently() {
    assert_evm_replay_and_breaker_bindings();
    assert_tron_replay_and_breaker_bindings();
}

fn assert_evm_replay_and_breaker_bindings() {
    let evm = deployment(1);
    let evm_binding = sccp_evm_destination_binding_hash_v1(lane().source, &evm)
        .expect("valid EVM destination binding");
    for substituted in [
        SccpEvmDestinationDeploymentV1 {
            replay_verifier_address: [0x72; 20],
            ..evm
        },
        SccpEvmDestinationDeploymentV1 {
            replay_verifier_code_hash: [0x82; 32],
            ..evm
        },
        SccpEvmDestinationDeploymentV1 {
            mint_breaker_address: [0x92; 20],
            ..evm
        },
        SccpEvmDestinationDeploymentV1 {
            mint_breaker_code_hash: [0xa2; 32],
            ..evm
        },
    ] {
        assert_ne!(
            evm_binding,
            sccp_evm_destination_binding_hash_v1(lane().source, &substituted)
                .expect("role substitution remains structurally valid")
        );
    }
    let mut swapped_evm = evm;
    (
        swapped_evm.replay_verifier_address,
        swapped_evm.mint_breaker_address,
    ) = (
        swapped_evm.mint_breaker_address,
        swapped_evm.replay_verifier_address,
    );
    (
        swapped_evm.replay_verifier_code_hash,
        swapped_evm.mint_breaker_code_hash,
    ) = (
        swapped_evm.mint_breaker_code_hash,
        swapped_evm.replay_verifier_code_hash,
    );
    assert_ne!(
        evm_binding,
        sccp_evm_destination_binding_hash_v1(lane().source, &swapped_evm)
            .expect("swapped EVM roles remain structurally valid")
    );
    assert_eq!(
        sccp_evm_destination_binding_hash_v1(
            lane().source,
            &SccpEvmDestinationDeploymentV1 {
                mint_breaker_address: evm.replay_verifier_address,
                ..evm
            },
        ),
        Err(SccpRouteValidationError::RoleAlias)
    );
    assert_eq!(
        sccp_evm_destination_binding_hash_v1(
            lane().source,
            &SccpEvmDestinationDeploymentV1 {
                mint_breaker_code_hash: evm.replay_verifier_code_hash,
                ..evm
            },
        ),
        Err(SccpRouteValidationError::RoleAlias)
    );
}

fn assert_tron_replay_and_breaker_bindings() {
    let tron = tron_deployment();
    let tron_lane = tron_lane(SccpNetworkV1::TronMainnet);
    let tron_binding = sccp_tron_destination_binding_hash_v1(tron_lane.source, &tron)
        .expect("valid TRON destination binding");
    for substituted in [
        SccpTronDestinationDeploymentV1 {
            replay_verifier_address: [0x72; 20],
            ..tron
        },
        SccpTronDestinationDeploymentV1 {
            replay_verifier_code_hash: [0x82; 32],
            ..tron
        },
        SccpTronDestinationDeploymentV1 {
            mint_breaker_address: [0x92; 20],
            ..tron
        },
        SccpTronDestinationDeploymentV1 {
            mint_breaker_code_hash: [0xa2; 32],
            ..tron
        },
    ] {
        assert_ne!(
            tron_binding,
            sccp_tron_destination_binding_hash_v1(tron_lane.source, &substituted)
                .expect("role substitution remains structurally valid")
        );
    }
    let mut swapped_tron = tron;
    (
        swapped_tron.replay_verifier_address,
        swapped_tron.mint_breaker_address,
    ) = (
        swapped_tron.mint_breaker_address,
        swapped_tron.replay_verifier_address,
    );
    (
        swapped_tron.replay_verifier_code_hash,
        swapped_tron.mint_breaker_code_hash,
    ) = (
        swapped_tron.mint_breaker_code_hash,
        swapped_tron.replay_verifier_code_hash,
    );
    assert_ne!(
        tron_binding,
        sccp_tron_destination_binding_hash_v1(tron_lane.source, &swapped_tron)
            .expect("swapped TRON roles remain structurally valid")
    );
    assert_eq!(
        sccp_tron_destination_binding_hash_v1(
            tron_lane.source,
            &SccpTronDestinationDeploymentV1 {
                mint_breaker_address: tron.replay_verifier_address,
                ..tron
            },
        ),
        Err(SccpRouteValidationError::RoleAlias)
    );
    assert_eq!(
        sccp_tron_destination_binding_hash_v1(
            tron_lane.source,
            &SccpTronDestinationDeploymentV1 {
                mint_breaker_code_hash: tron.replay_verifier_code_hash,
                ..tron
            },
        ),
        Err(SccpRouteValidationError::RoleAlias)
    );
}

#[test]
fn registry_json_rejects_unknown_fields_at_every_consensus_boundary() {
    let route = route(1, SccpRouteActivationV1::Staged);
    let valid_json = norito::json::to_json(&route).expect("route serializes");
    assert_eq!(
        norito::json::from_json::<SccpGovernedRouteV1>(&valid_json).expect("valid route decodes"),
        route
    );
    for path in [
        &[][..],
        &["destination"][..],
        &["destination", "deployment"][..],
        &["destination", "deployment", "verifying_key"][..],
        &[
            "destination",
            "deployment",
            "verifying_key",
            "ic",
            "signal_10",
        ][..],
        &["source_identity"][..],
        &["source_identity", "lane"][..],
        &["source_identity", "emitter"][..],
        &["source_identity", "emitter", "identity"][..],
        &["destination", "deployment", "outbound_proof_policy"][..],
        &[
            "destination",
            "deployment",
            "outbound_proof_policy",
            "semantic_profile",
        ][..],
        &[
            "destination",
            "deployment",
            "outbound_proof_policy",
            "semantic_profile",
            "commitments",
        ][..],
        &[
            "destination",
            "deployment",
            "outbound_proof_policy",
            "sora_finality_anchor",
        ][..],
    ] {
        let mut hostile = norito::json::to_value(&route).expect("serialize route");
        insert_unknown_json_field(&mut hostile, path);
        let hostile_json = norito::json::to_json(&hostile).expect("serialize hostile route");
        let error = norito::json::from_json::<SccpGovernedRouteV1>(&hostile_json)
            .expect_err("unknown route field must fail");
        assert!(
            error.to_string().contains("adversarial_extension"),
            "unexpected error for path {path:?}: {error}"
        );
    }
    let registry = registry(vec![route], None);
    let registry_json = norito::json::to_json(&registry).expect("registry serializes");
    assert_eq!(
        norito::json::from_json::<SccpRegistryV1>(&registry_json).expect("valid registry decodes"),
        registry
    );
    for path in [&[][..], &["lanes"][..]] {
        let mut hostile = norito::json::to_value(&registry).expect("serialize registry");
        if path == ["lanes"] {
            let norito::json::Value::Object(root) = &mut hostile else {
                panic!("registry JSON is an object")
            };
            let norito::json::Value::Array(lanes) = root
                .get_mut("lanes")
                .expect("registry JSON carries governed lanes")
            else {
                panic!("registry lanes JSON is an array")
            };
            insert_unknown_json_field(&mut lanes[0], &[]);
        } else {
            insert_unknown_json_field(&mut hostile, path);
        }
        let hostile_json = norito::json::to_json(&hostile).expect("serialize hostile registry");
        assert!(
            norito::json::from_json::<SccpRegistryV1>(&hostile_json).is_err(),
            "unknown registry field at {path:?} must fail"
        );
    }
}
