//! Deterministic exact SCCP fixtures for crate and downstream integration tests.

use core::num::NonZeroU64;

use halo2curves::{
    Coordinates, CurveAffine,
    bn256::{Fq, Fr, G1Affine},
    ff::PrimeField as _,
    group::Curve,
};
use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
use iroha_data_model::{
    account::AccountId,
    block::consensus_v2::{
        BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
        GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding, QuorumCertificate,
        ValidatorPower, finality::V2FinalityArtifact,
    },
    bridge::{
        BRIDGE_FINALITY_PROOF_VERSION_V1, BridgeSccpDestinationProofV1,
        SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER, SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE, SccpBn254G1PointV1,
        SccpBn254G2PointV1, SccpDestinationDeploymentV1, SccpEvmDestinationDeploymentV1,
        SccpEvmSourceEmitterV1, SccpGovernedRouteV1, SccpGroth16Bn254IcV1,
        SccpGroth16Bn254SemanticCircuitV1, SccpGroth16Bn254VerifyingKeyV1, SccpLaneIdV1,
        SccpNetworkV1, SccpOutboundMessageContextV1, SccpOutboundProofPolicyV1,
        SccpRouteActivationV1, SccpSemanticProofProfileV1, SccpSoraFinalityAnchorV1,
        SccpSoraSettlementV1, SccpSourceEmitterV1, SccpSourceIdentityV1,
        sccp_groth16_bn254_public_signal_schema_hash_v1, sccp_sora_taira_chain_id_hash_v1,
        sccp_v1_taira_xor_asset_definition_id,
    },
    peer::PeerId,
};
use norito::to_bytes;

use crate::*;

/// Complete exact EVM outbound fixture for downstream SCCP integration tests.
///
/// Every field is reconstructed from the governed route and the returned proof
/// passes the same canonical decoding, BN254 subgroup, pairing, and
/// request-binding path as a submitted destination proof.
#[derive(Clone, Debug)]
pub struct SccpExactOutboundTestFixtureV1 {
    /// Complete exact governed route used by the proof.
    pub route: SccpGovernedRouteV1,
    /// Canonical SORA-origin message bundle.
    pub bundle: TairaSccpMessageProofV1,
    /// Query-free request derived from the route and bundle.
    pub request: SccpGroth16Bn254ProofRequestV1,
    /// Pairing-valid Groth16 artifact bound to the request.
    pub artifact: SccpGroth16Bn254ProofArtifactV1,
    /// Closed bridge-proof container for Core and Torii admission tests.
    pub bridge_proof: BridgeSccpDestinationProofV1,
}

fn word_u64(value: u64) -> H256 {
    let mut word = [0; 32];
    word[24..].copy_from_slice(&value.to_be_bytes());
    word
}

fn hex32(value: &str) -> H256 {
    decode_fixed_hex_bytes(value).expect("static exact SCCP test vector is lowercase hex")
}

fn fq_word(value: Fq) -> H256 {
    let repr = value.to_repr();
    let mut word = [0; 32];
    for (output, input) in word.iter_mut().zip(repr.as_ref().iter().rev()) {
        *output = *input;
    }
    word
}

fn g1_words(point: G1Affine) -> [H256; 2] {
    let coordinates: Coordinates<G1Affine> =
        Option::from(point.coordinates()).expect("exact SCCP fixture point is not infinity");
    [fq_word(*coordinates.x()), fq_word(*coordinates.y())]
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

fn outbound_policy() -> SccpOutboundProofPolicyV1 {
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
            protocol_version: iroha_data_model::block::consensus_v2::PROTOCOL_VERSION,
            chain_id_hash: sccp_sora_taira_chain_id_hash_v1(),
            checkpoint_height: 5,
            checkpoint_block_hash: [0x73; 32],
            checkpoint_context_id: [0x74; 32],
            checkpoint_finality_artifact_hash: [0x75; 32],
        },
    }
}

/// Build one complete exact EVM-family governed route for downstream tests.
///
/// # Panics
///
/// Panics when `network` is not an Ethereum or BSC profile, or when a static
/// fixture invariant no longer matches the production route schema.
#[must_use]
pub fn sccp_exact_evm_governed_route_test_fixture_v1(
    network: SccpNetworkV1,
    activation: SccpRouteActivationV1,
) -> SccpGovernedRouteV1 {
    let route_id = match network {
        SccpNetworkV1::EthereumMainnet | SccpNetworkV1::EthereumSepolia => {
            SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1
        }
        SccpNetworkV1::BscMainnet | SccpNetworkV1::BscTestnet => SCCP_TAIRA_BSC_XOR_ROUTE_ID_V1,
        _ => panic!("exact EVM SCCP fixture requires an Ethereum or BSC profile"),
    };
    let lane_id = SccpLaneIdV1 {
        source: network,
        target: SccpNetworkV1::SoraTaira,
    };
    let verifying_key = verifying_key();
    let deployment = SccpEvmDestinationDeploymentV1 {
        token_address: [0x11; 20],
        token_code_hash: [0x21; 32],
        verifier_address: [0x31; 20],
        verifier_code_hash: [0x41; 32],
        verifying_key,
        verifier_key_hash: sccp_groth16_bn254_verifying_key_hash_v1(&verifying_key)
            .expect("exact SCCP test verification key is curve-valid"),
        outbound_proof_policy: outbound_policy(),
        route_address: [0x51; 20],
        route_code_hash: [0x61; 32],
        taira_to_token_multiplier: SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER,
    };
    let destination = SccpDestinationDeploymentV1::Evm(deployment);
    let route_configuration_hash = destination
        .route_configuration_hash(
            lane_id,
            route_id,
            SCCP_TAIRA_XOR_ASSET_KEY_V1,
            1,
            SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
        )
        .expect("exact SCCP fixture route configuration is valid");
    let custody = KeyPair::try_from_seed(vec![0x81; 32], Algorithm::Ed25519)
        .expect("exact SCCP custody fixture key")
        .public_key()
        .clone();
    let route = SccpGovernedRouteV1 {
        lane_id,
        route_id: route_id.to_owned(),
        asset_key: SCCP_TAIRA_XOR_ASSET_KEY_V1.to_owned(),
        revision: 1,
        activation,
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
        .validate()
        .expect("exact SCCP governed route fixture remains valid");
    assert_eq!(
        route
            .route_configuration_hash()
            .expect("exact SCCP fixture route configuration"),
        route_configuration_hash
    );
    assert!(sccp_governed_route_groth16_key_is_valid_v1(&route));
    route
}

fn transfer_payload(route: &SccpGovernedRouteV1, nonce: u64) -> SccpPayloadV1 {
    SccpPayloadV1::Transfer(TransferPayloadV1 {
        version: 1,
        source_domain: SCCP_DOMAIN_SORA,
        dest_domain: route.lane_id.source.domain_id(),
        nonce,
        route_revision: route.revision,
        asset_home_domain: SCCP_DOMAIN_SORA,
        asset_id_codec: SCCP_CODEC_CANONICAL_TEXT,
        asset_id: route.asset_key.as_bytes().to_vec(),
        amount: 123,
        sender_codec: SCCP_CODEC_CANONICAL_TEXT,
        sender: b"sccp-test-sender".to_vec(),
        recipient_codec: SCCP_CODEC_EVM_ADDRESS20,
        recipient: vec![0x91; 20],
        route_id_codec: SCCP_CODEC_CANONICAL_TEXT,
        route_id: route.route_id.as_bytes().to_vec(),
    })
}

/// Build an exact cryptographically valid Sumeragi-v2 Taira finality proof.
pub(crate) fn signed_finality_proof(commitment_root: H256) -> Vec<u8> {
    let height = 1;
    let mut block_header = BlockHeader::new(
        NonZeroU64::new(height).expect("exact SCCP fixture height is nonzero"),
        None,
        None,
        None,
        0,
        0,
    );
    block_header.set_sccp_commitment_root(Some(commitment_root));
    let mut keypairs = vec![
        KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal).expect("BLS fixture key 1"),
        KeyPair::try_from_seed(vec![2; 32], Algorithm::BlsNormal).expect("BLS fixture key 2"),
        KeyPair::try_from_seed(vec![3; 32], Algorithm::BlsNormal).expect("BLS fixture key 3"),
        KeyPair::try_from_seed(vec![4; 32], Algorithm::BlsNormal).expect("BLS fixture key 4"),
    ];
    keypairs.sort_by(|left, right| {
        PeerId::new(left.public_key().clone()).cmp(&PeerId::new(right.public_key().clone()))
    });
    let roster = keypairs
        .iter()
        .zip([40_u64, 30, 20, 10])
        .map(|(keypair, power)| ValidatorPower {
            validator: PeerId::new(keypair.public_key().clone()),
            power,
        })
        .collect::<Vec<_>>();
    let context = HeightContext {
        chain_id: SCCP_TAIRA_FINALITY_CHAIN_ID_V1.into(),
        protocol_version: PROTOCOL_VERSION,
        height,
        epoch: 0,
        epoch_end_height: 10,
        next_epoch_snapshot: None,
        mode: ConsensusMode::Npos,
        parent_commit_qc: None,
        next_epoch_snapshot: None,
        quorum: DualQuorum::from_roster(&roster).expect("valid powered SCCP fixture roster"),
        roster,
        nexus_amx_context_hash: Hash::new(b"exact SCCP fixture Nexus/AMX context"),
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::Plain,
            chunk_size_bytes: 1024,
            data_shards: 0,
            parity_shards: 0,
            max_payload_size_bytes: 4096,
            max_chunk_count: 4,
        },
        leader_seed: [0x5a; 32],
    };
    let subject = BlockSubject {
        parent_block_hash: None,
        block_hash: block_header.hash(),
        payload_hash: Hash::new(b"exact SCCP fixture payload"),
    };
    let mut commit_qc = QuorumCertificate {
        round: ConsensusRound {
            context_id: context.id(),
            height,
            view: 1,
        },
        phase: GlobalPhase::Commit,
        subject,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![1],
    };
    let message = commit_qc
        .signer_preimage(&context, 0)
        .expect("valid exact Sumeragi-v2 commit certificate");
    let signatures = commit_qc
        .signers
        .iter()
        .map(|index| {
            let index = usize::try_from(*index).expect("fixture signer index fits usize");
            Signature::try_new(keypairs[index].private_key(), &message)
                .expect("BLS fixture commit vote")
        })
        .collect::<Vec<_>>();
    let signature_refs = signatures
        .iter()
        .map(|signature| signature.payload().as_ref())
        .collect::<Vec<_>>();
    commit_qc.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
        .expect("aggregate BLS fixture votes");
    let validator_set_pops = keypairs
        .iter()
        .map(|keypair| {
            iroha_crypto::bls_normal_pop_prove(keypair.private_key()).expect("BLS fixture PoP")
        })
        .collect();
    let finality_artifact =
        V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops);
    to_bytes(&TairaBridgeFinalityProofV1 {
        version: BRIDGE_FINALITY_PROOF_VERSION_V1,
        block_header,
        finality_artifact,
    })
    .expect("canonical exact SCCP finality fixture")
}

fn message_bundle(route: &SccpGovernedRouteV1, nonce: u64) -> TairaSccpMessageProofV1 {
    let payload = transfer_payload(route, nonce);
    let context = SccpOutboundMessageContextV1::new(
        SccpLaneIdV1 {
            source: route.lane_id.target,
            target: route.lane_id.source,
        },
        route
            .destination_binding_hash()
            .expect("exact SCCP destination binding"),
        route
            .route_configuration_hash()
            .expect("exact SCCP route configuration"),
    )
    .expect("exact SCCP outbound context");
    let commitment =
        hub_commitment_from_sccp_payload(context, &payload).expect("exact SCCP commitment");
    let merkle_proof = SccpMerkleProofV1 { steps: Vec::new() };
    let commitment_root = merkle_root_from_commitment(&commitment, &merkle_proof);
    let bundle = TairaSccpMessageProofV1 {
        version: 1,
        commitment_root,
        commitment,
        merkle_proof,
        payload,
        finality_proof: signed_finality_proof(commitment_root),
    };
    assert!(verify_message_bundle_structure(&bundle));
    assert!(
        verified_sccp_message_taira_finality_proof_cryptographically_self_consistent(&bundle)
            .is_some()
    );
    bundle
}

fn valid_proof(request: &SccpGroth16Bn254ProofRequestV1) -> Vec<u8> {
    let signals = sccp_groth16_bn254_public_signal_words(
        &request.public_inputs,
        request.source_network.domain_id(),
        request.statement_hash,
        request.destination_binding_hash,
        request.route_configuration_hash,
        request.sora_finality_anchor_hash,
    );
    let mut scalar = Fr::from(3_u64);
    for signal in &signals {
        scalar += bn254_fr_from_abi_word(signal).expect("canonical SCCP scalar signal");
    }
    let a = (G1Affine::generator() * scalar).to_affine();
    let proof = SccpEvmGroth16Bn254ProofV1 {
        version: 1,
        message_id: request.public_inputs.message_id,
        source_domain: request.source_network.domain_id(),
        commitment_root: request.public_inputs.commitment_root,
        a: g1_words(a),
        b: [
            request.verifying_key.beta2.x_c0,
            request.verifying_key.beta2.x_c1,
            request.verifying_key.beta2.y_c0,
            request.verifying_key.beta2.y_c1,
        ],
        c: [
            request.verifying_key.alpha1.x,
            request.verifying_key.alpha1.y,
        ],
    };
    encode_sccp_evm_groth16_bn254_proof_bytes(&proof)
}

/// Build an owned, end-to-end exact Ethereum-mainnet outbound test fixture.
///
/// # Panics
///
/// Panics if any fixed vector stops satisfying the production SCCP
/// canonicalization, governed-route, finality, or BN254 verification rules.
#[must_use]
pub fn sccp_exact_outbound_test_fixture_v1() -> SccpExactOutboundTestFixtureV1 {
    sccp_exact_outbound_test_fixture_for_nonce_v1(7)
}

/// Build an owned exact Ethereum-mainnet outbound fixture for `nonce`.
///
/// # Panics
///
/// Panics if any fixed vector stops satisfying the production SCCP
/// canonicalization, governed-route, finality, or BN254 verification rules.
#[must_use]
pub fn sccp_exact_outbound_test_fixture_for_nonce_v1(nonce: u64) -> SccpExactOutboundTestFixtureV1 {
    let route = sccp_exact_evm_governed_route_test_fixture_v1(
        SccpNetworkV1::EthereumMainnet,
        SccpRouteActivationV1::Bidirectional,
    );
    let bundle = message_bundle(&route, nonce);
    let request = build_sccp_groth16_bn254_proof_request_from_governed_route_v1(&bundle, &route)
        .expect("exact SCCP governed proof request");
    let proof_bytes = valid_proof(&request);
    assert!(verify_sccp_groth16_bn254_proof_v1(&request, &proof_bytes,));
    let artifact = wrap_sccp_evm_groth16_bn254_proof_result(&proof_bytes, &request)
        .expect("exact SCCP Groth16 artifact");
    let bridge_proof =
        bridge_sccp_destination_proof_v1(&artifact).expect("closed exact SCCP bridge proof");
    assert!(verify_sccp_destination_proof_v1(&bridge_proof, &bundle, &route).is_some());
    SccpExactOutboundTestFixtureV1 {
        route,
        bundle,
        request,
        artifact,
        bridge_proof,
    }
}
