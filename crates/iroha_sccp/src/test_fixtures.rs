//! Deterministic exact SCCP fixtures for crate and downstream integration tests.
use crate::*;
use core::{num::NonZeroU64, time::Duration};
use halo2curves::{
    Coordinates, CurveAffine,
    bls12381::{Fr as Bls12381Fr, G1Affine as Bls12381G1Affine, G2Affine as Bls12381G2Affine},
    bn256::{Fq, Fr, G1Affine},
    ff::PrimeField as _,
    group::{Curve, GroupEncoding},
    pasta::{Fp as PastaFp, Fq as PastaFq, PallasAffine, VestaAffine},
};
use iroha_crypto::{Algorithm, Hash, KeyPair, MerkleTree, Signature, SignatureOf};
use iroha_data_model::{
    account::AccountId,
    block::consensus_v2::{
        BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
        ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
        QuorumCertificate, ValidatorPower, finality::V2FinalityArtifact,
    },
    block::{BlockHeader, BlockSignature, SignedBlock},
    bridge::{
        BRIDGE_FINALITY_PROOF_VERSION_V2, BridgeSccpDestinationProofV1,
        SCCP_V1_SORA_OUTBOUND_EXECUTION_SEMANTICS, SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER,
        SCCP_V1_TAIRA_TO_TON_TOKEN_MULTIPLIER, SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
        SccpBn254G1PointV1, SccpBn254G2PointV1, SccpDestinationDeploymentV1,
        SccpEvmDestinationDeploymentV1, SccpEvmSourceEmitterV1, SccpGovernedRouteV1,
        SccpGroth16Bls12381IcV1, SccpGroth16Bls12381SemanticCircuitV1,
        SccpGroth16Bls12381VerifyingKeyV1, SccpGroth16Bn254IcV1, SccpGroth16Bn254SemanticCircuitV1,
        SccpGroth16Bn254VerifyingKeyV1, SccpLaneIdV1, SccpNetworkV1, SccpOutboundMessageContextV1,
        SccpOutboundProofPolicyV1, SccpPortableVerifyingKeyRefV1, SccpRouteActivationV1,
        SccpSemanticProofProfileV1, SccpSoraFinalityAnchorV1, SccpSoraOutboundExecutionPolicyV1,
        SccpSoraSettlementV1, SccpSourceEmitterV1, SccpSourceIdentityV1, SccpTonAddressV1,
        SccpTonDestinationDeploymentV1, SccpTonSourceEmitterV1,
        sccp_groth16_bls12381_public_signal_schema_hash_v1,
        sccp_groth16_bn254_public_signal_schema_hash_v1, sccp_sora_taira_chain_id_hash_v1,
        sccp_v1_taira_xor_asset_definition_id,
    },
    consensus::{NposConsensusEffects, NposMarkConsensusEvidenceAppliedAction, NposPenaltyAction},
    isi::{InstructionBox, bridge::RecordSccpMessage},
    peer::PeerId,
    transaction::{
        DataTriggerSequence, Executable, IvmBytecode, IvmProved, TransactionBuilder,
        TransactionEntrypoint, TransactionResult, TransactionResultInner,
    },
};
use norito::to_bytes;
use std::collections::BTreeSet;

const TEST_MAX_OUTSTANDING_LIABILITY: u128 = 1_000_000_000_000;
const fn test_max_wrapped_supply(multiplier: u64) -> u128 {
    TEST_MAX_OUTSTANDING_LIABILITY * multiplier as u128
}

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
    /// Complete signed block and exact Sumeragi-v2 finality used by the bundle.
    pub finalized_block: SccpFinalizedBlockTestFixtureV1,
}
/// Complete exact TON outbound fixture for downstream SCCP integration tests.
///
/// Every field is reconstructed from the governed TON route and the returned
/// proof passes the canonical BLS12-381 and route-binding verification path.
#[derive(Clone, Debug)]
pub struct SccpExactTonOutboundTestFixtureV1 {
    /// Complete exact governed TON route used by the proof.
    pub route: SccpGovernedRouteV1,
    /// Canonical SORA-origin message bundle with a TON account recipient.
    pub bundle: TairaSccpMessageProofV1,
    /// Query-free request derived from the route and bundle.
    pub request: SccpTonGroth16Bls12381ProofRequestV1,
    /// Pairing-valid Groth16 artifact bound to the request.
    pub artifact: SccpTonGroth16Bls12381ProofArtifactV1,
    /// Closed bridge-proof container for Core and Torii admission tests.
    pub bridge_proof: BridgeSccpDestinationProofV1,
    /// Complete signed block and exact Sumeragi-v2 finality used by the bundle.
    pub finalized_block: SccpFinalizedBlockTestFixtureV1,
}
/// Complete block plus finality produced only through the exact test signer.
///
/// Private fields keep the parent invariant closed: a height-two fixture can
/// inherit only a parent `CommitQC` already bound to both its canonical resultless
/// proposal and complete result-bearing block wire images by this module.
#[derive(Clone, Debug)]
pub struct SccpFinalizedBlockTestFixtureV1 {
    block: SignedBlock,
    proof: TairaBridgeFinalityProofV1,
}

fn sccp_mint_finality_roster_test_fixture_v1(
    network_id: iroha_data_model::NetworkId,
    epoch: u64,
    roster: &[ValidatorPower],
) -> iroha_data_model::isi::offline_cash_v1::OfflineCashMintFinalityEpochRosterV1 {
    use iroha_data_model::isi::offline_cash_v1::{
        OFFLINE_CASH_CHAIN_VERSION_V1, OfflineCashMintFinalityEpochRosterV1,
        OfflineCashMintFinalityValidatorKeysV1,
    };

    OfflineCashMintFinalityEpochRosterV1 {
        version: OFFLINE_CASH_CHAIN_VERSION_V1,
        network_id,
        epoch,
        validators: roster
            .iter()
            .enumerate()
            .map(|(index, validator)| {
                let scalar = u64::try_from(index + 1).expect("small SCCP fixture roster");
                let eq_encoded = (<PallasAffine as CurveAffine>::CurveExt::generator()
                    * PastaFq::from(scalar))
                .to_affine()
                .to_bytes();
                let ep_encoded = (<VestaAffine as CurveAffine>::CurveExt::generator()
                    * PastaFp::from(scalar))
                .to_affine()
                .to_bytes();
                let mut eq_proof_public_key = [0_u8; 32];
                eq_proof_public_key.copy_from_slice(eq_encoded.as_ref());
                let mut ep_proof_public_key = [0_u8; 32];
                ep_proof_public_key.copy_from_slice(ep_encoded.as_ref());
                OfflineCashMintFinalityValidatorKeysV1 {
                    validator: validator.validator.clone(),
                    eq_proof_public_key,
                    ep_proof_public_key,
                }
            })
            .collect(),
    }
}

impl SccpFinalizedBlockTestFixtureV1 {
    /// Return the complete signed block authenticated by this fixture.
    #[must_use]
    pub const fn block(&self) -> &SignedBlock {
        &self.block
    }
    /// Return the exact finality proof bound to the proposal and executed wire images.
    #[must_use]
    pub const fn proof(&self) -> &TairaBridgeFinalityProofV1 {
        &self.proof
    }
}
impl SccpExactOutboundTestFixtureV1 {
    /// Rebuild this exact fixture around one complete finalized signed block.
    ///
    /// The caller must first attach the block's transactions and results so
    /// both Merkle roots are present. A height-two block must also receive the
    /// typed complete parent fixture whose exact `CommitQC` becomes the frozen
    /// successor context. This method then signs the canonical block wire with
    /// the deterministic test-only Taira roster and regenerates every
    /// downstream request, Groth16 artifact, and bridge-proof binding.
    ///
    /// # Panics
    ///
    /// Panics if the block is incomplete, commits another SCCP root, has an
    /// invalid parent, or the
    /// regenerated exact proof stops satisfying production verification.
    #[must_use]
    pub fn with_finalized_block(
        &self,
        block: &SignedBlock,
        parent: Option<&SccpFinalizedBlockTestFixtureV1>,
    ) -> Self {
        let block_header = block.header();
        assert!(
            block_header.merkle_root().is_some(),
            "an SCCP finalized-header fixture must commit its external entrypoints"
        );
        assert!(
            block_header.result_merkle_root().is_some(),
            "an SCCP finalized-header fixture must commit its transaction results"
        );
        assert_eq!(
            block_header.sccp_commitment_root(),
            Some(self.bundle.commitment_root),
            "an SCCP finalized-header fixture must commit the exact bundle root"
        );
        let finalized_block = sccp_finalize_taira_block_test_fixture_v1(block, parent);
        let finality = finalized_block.proof();
        assert_eq!(finality.block_header, block_header);
        assert_eq!(finality.finality_artifact.block_hash, block_header.hash());
        assert_eq!(finalized_block.block(), block);
        finality
            .finality_artifact
            .validate_for_header(&block_header)
            .expect("exact SCCP finality artifact binds the complete block header");
        finality
            .finality_artifact
            .verify()
            .expect("exact SCCP finalized-header fixture is cryptographically valid");
        let mut bundle = self.bundle.clone();
        bundle.finality_proof =
            to_bytes(finality).expect("canonical exact SCCP finalized-header finality proof");
        assert!(verify_message_bundle_structure(&bundle));
        assert!(
            verified_sccp_message_taira_finality_proof_cryptographically_self_consistent(&bundle)
                .is_some()
        );
        let route = self.route.clone();
        let request =
            build_sccp_groth16_bn254_proof_request_from_governed_route_v1(&bundle, &route)
                .expect("exact finalized-header SCCP governed proof request");
        let proof_bytes = valid_proof(&request);
        assert!(verify_sccp_groth16_bn254_proof_v1(&request, &proof_bytes,));
        let artifact = wrap_sccp_evm_groth16_bn254_proof_result(&proof_bytes, &request)
            .expect("exact finalized-header SCCP Groth16 artifact");
        let bridge_proof = bridge_sccp_destination_proof_v1(&artifact)
            .expect("closed exact finalized-header SCCP bridge proof");
        assert!(
            verify_sccp_destination_proof_v1(&bridge_proof, &bundle, &route, finality).is_some()
        );
        Self {
            route,
            bundle,
            request,
            artifact,
            bridge_proof,
            finalized_block,
        }
    }
}
impl SccpExactTonOutboundTestFixtureV1 {
    /// Rebuild this exact TON fixture around one complete finalized signed block.
    ///
    /// # Panics
    ///
    /// Panics if the block is incomplete, commits another SCCP root, has an
    /// invalid parent, or the regenerated BLS12-381 proof stops satisfying
    /// production verification.
    #[must_use]
    pub fn with_finalized_block(
        &self,
        block: &SignedBlock,
        parent: Option<&SccpFinalizedBlockTestFixtureV1>,
    ) -> Self {
        let block_header = block.header();
        assert!(block_header.merkle_root().is_some());
        assert!(block_header.result_merkle_root().is_some());
        assert_eq!(
            block_header.sccp_commitment_root(),
            Some(self.bundle.commitment_root)
        );
        let finalized_block = sccp_finalize_taira_block_test_fixture_v1(block, parent);
        let finality = finalized_block.proof();
        assert_eq!(finality.block_header, block_header);
        assert_eq!(finality.finality_artifact.block_hash, block_header.hash());
        finality
            .finality_artifact
            .validate_for_header(&block_header)
            .expect("exact TON fixture finality binds the complete block header");
        finality
            .finality_artifact
            .verify()
            .expect("exact TON fixture finality is cryptographically valid");
        let mut bundle = self.bundle.clone();
        bundle.finality_proof =
            to_bytes(finality).expect("canonical exact TON finalized-header finality proof");
        assert!(verify_message_bundle_structure(&bundle));
        let route = self.route.clone();
        let request =
            build_sccp_ton_groth16_bls12381_proof_request_from_governed_route_v1(&bundle, &route)
                .expect("exact finalized-header SCCP governed TON proof request");
        let proof_bytes = valid_bls12381_proof_bytes(&request);
        let artifact = wrap_sccp_ton_groth16_bls12381_proof_result_v1(&proof_bytes, &request)
            .expect("exact finalized-header SCCP TON Groth16 artifact");
        let bridge_proof = bridge_sccp_ton_destination_proof_v1(&artifact)
            .expect("closed exact finalized-header SCCP TON bridge proof");
        assert!(
            verify_sccp_destination_proof_v1(&bridge_proof, &bundle, &route, finality).is_some()
        );
        Self {
            route,
            bundle,
            request,
            artifact,
            bridge_proof,
            finalized_block,
        }
    }
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
fn bls12381_g1_bytes(point: Bls12381G1Affine) -> [u8; 48] {
    let encoded = point.to_bytes();
    let mut bytes = [0; 48];
    bytes.copy_from_slice(encoded.as_ref());
    bytes
}
fn bls12381_g2_bytes(point: Bls12381G2Affine) -> [u8; 96] {
    let encoded = point.to_bytes();
    let mut bytes = [0; 96];
    bytes.copy_from_slice(encoded.as_ref());
    bytes
}
fn bls12381_verifying_key() -> SccpGroth16Bls12381VerifyingKeyV1 {
    let g1 = bls12381_g1_bytes(Bls12381G1Affine::generator());
    let g2 = bls12381_g2_bytes(Bls12381G2Affine::generator());
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
fn ton_outbound_policy() -> SccpOutboundProofPolicyV1 {
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
        sora_finality_anchor: outbound_policy().sora_finality_anchor,
    }
}
/// Build the deterministic contract burn-and-record policy used by SCCP tests.
#[must_use]
pub fn sccp_sora_outbound_execution_policy_test_fixture_v1() -> SccpSoraOutboundExecutionPolicyV1 {
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
        SccpNetworkV1::EthereumMainnet => SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1,
        SccpNetworkV1::BscMainnet => SCCP_TAIRA_BSC_XOR_ROUTE_ID_V1,
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
        replay_verifier_address: [0x71; 20],
        replay_verifier_code_hash: [0x72; 32],
        mint_breaker_address: [0x81; 20],
        mint_breaker_code_hash: [0x82; 32],
        taira_to_token_multiplier: SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER,
        max_wrapped_supply: test_max_wrapped_supply(SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER),
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
        sora_outbound_execution_policy: sccp_sora_outbound_execution_policy_test_fixture_v1(),
        settlement: SccpSoraSettlementV1 {
            asset_definition_id: sccp_v1_taira_xor_asset_definition_id(),
            payload_amount_scale: SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            max_outstanding_liability: TEST_MAX_OUTSTANDING_LIABILITY,
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
/// Build one complete exact TON governed route for downstream tests.
///
/// # Panics
///
/// Panics when `network` is not a TON profile or a static fixture invariant
/// no longer matches the production route schema.
#[must_use]
pub fn sccp_exact_ton_governed_route_test_fixture_v1(
    network: SccpNetworkV1,
    activation: SccpRouteActivationV1,
) -> SccpGovernedRouteV1 {
    assert!(
        matches!(network, SccpNetworkV1::TonMainnet),
        "exact TON SCCP fixture requires a TON profile"
    );
    let lane_id = SccpLaneIdV1 {
        source: network,
        target: SccpNetworkV1::SoraTaira,
    };
    let address = |byte| SccpTonAddressV1 {
        workchain: 0,
        account: [byte; 32],
    };
    let verifying_key = bls12381_verifying_key();
    let deployment = SccpTonDestinationDeploymentV1 {
        jetton_master_address: address(0x81),
        jetton_master_code_hash: [0x91; 32],
        jetton_master_initial_data_hash: [0x89; 32],
        jetton_wallet_code_hash: [0x92; 32],
        route_address: address(0x82),
        route_code_hash: [0x93; 32],
        route_initial_data_hash: [0x8a; 32],
        embedded_verifier_code_hash: [0x94; 32],
        verifier_circuit_hash: [0x76; 32],
        verifying_key,
        verifier_key_hash: sccp_groth16_bls12381_verifying_key_hash_v1(&verifying_key)
            .expect("exact SCCP TON verification key is curve-valid"),
        proof_profile_commitment: sccp_ton_groth16_bls12381_proof_profile_commitment_v1(),
        mint_breaker_guardian_keys: [[0xa1; 32], [0xa2; 32], [0xa3; 32], [0xa4; 32], [0xa5; 32]]
            .into(),
        outbound_proof_policy: ton_outbound_policy(),
        taira_to_token_multiplier: SCCP_V1_TAIRA_TO_TON_TOKEN_MULTIPLIER,
        max_wrapped_supply: test_max_wrapped_supply(SCCP_V1_TAIRA_TO_TON_TOKEN_MULTIPLIER),
    };
    let destination = SccpDestinationDeploymentV1::Ton(deployment);
    let route_configuration_hash = destination
        .route_configuration_hash(
            lane_id,
            SCCP_TAIRA_TON_XOR_ROUTE_ID_V1,
            SCCP_TAIRA_XOR_ASSET_KEY_V1,
            1,
            SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
        )
        .expect("exact SCCP TON route configuration is valid");
    let route = SccpGovernedRouteV1 {
        lane_id,
        route_id: SCCP_TAIRA_TON_XOR_ROUTE_ID_V1.to_owned(),
        asset_key: SCCP_TAIRA_XOR_ASSET_KEY_V1.to_owned(),
        revision: 1,
        activation,
        inbound_finality_cutoff: None,
        source_identity: SccpSourceIdentityV1 {
            lane: lane_id,
            emitter: SccpSourceEmitterV1::Ton(SccpTonSourceEmitterV1 {
                address: deployment.route_address,
                code_hash: deployment.route_code_hash,
                route_config_hash: route_configuration_hash,
            }),
        },
        destination,
        sora_outbound_execution_policy: sccp_sora_outbound_execution_policy_test_fixture_v1(),
        settlement: SccpSoraSettlementV1 {
            asset_definition_id: sccp_v1_taira_xor_asset_definition_id(),
            payload_amount_scale: SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            max_outstanding_liability: TEST_MAX_OUTSTANDING_LIABILITY,
        },
    };
    route
        .validate()
        .expect("exact SCCP governed TON route fixture remains valid");
    assert_eq!(
        route
            .route_configuration_hash()
            .expect("exact SCCP TON fixture route configuration"),
        route_configuration_hash
    );
    assert!(sccp_ton_deployment_matches_verifying_key_v1(&deployment));
    route
}
fn transfer_payload(route: &SccpGovernedRouteV1, nonce: u64) -> SccpPayloadV1 {
    let sender = AccountId::new(
        KeyPair::try_from_seed(vec![0x90; 32], Algorithm::Ed25519)
            .expect("exact outbound SCCP sender fixture key")
            .public_key()
            .clone(),
    )
    .to_i105_for_discriminant(SCCP_TAIRA_I105_DISCRIMINANT_V1)
    .expect("exact outbound SCCP sender fixture has canonical Taira I105");
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
        sender: sender.into_bytes(),
        recipient_codec: SCCP_CODEC_EVM_ADDRESS20,
        recipient: vec![0x91; 20],
        route_id_codec: SCCP_CODEC_CANONICAL_TEXT,
        route_id: route.route_id.as_bytes().to_vec(),
    })
}
fn ton_transfer_payload(route: &SccpGovernedRouteV1, nonce: u64) -> SccpPayloadV1 {
    let sender = AccountId::new(
        KeyPair::try_from_seed(vec![0x90; 32], Algorithm::Ed25519)
            .expect("exact outbound SCCP sender fixture key")
            .public_key()
            .clone(),
    )
    .to_i105_for_discriminant(SCCP_TAIRA_I105_DISCRIMINANT_V1)
    .expect("exact outbound SCCP sender fixture has canonical Taira I105");
    let recipient = canonical_sccp_ton_account36_bytes_v1(SccpTonAddressV1 {
        workchain: 0,
        account: [0xa6; 32],
    })
    .expect("exact TON recipient is canonical");
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
        sender: sender.into_bytes(),
        recipient_codec: SCCP_CODEC_TON_ACCOUNT36,
        recipient: recipient.to_vec(),
        route_id_codec: SCCP_CODEC_CANONICAL_TEXT,
        route_id: route.route_id.as_bytes().to_vec(),
    })
}
fn exact_fixture_proposal_wire_hash(block: &SignedBlock) -> Hash {
    block
        .canonical_proposal_wire_hash()
        .expect("exact SCCP fixture proposal has canonical wire bytes")
}
fn exact_fixture_executed_wire_hash(block: &SignedBlock) -> Hash {
    block
        .executed_block_wire_hash()
        .expect("exact SCCP fixture executed block has canonical wire bytes")
}
fn exact_fixture_executed_wire_len(block: &SignedBlock) -> u64 {
    u64::try_from(
        block
            .encode_wire()
            .expect("exact SCCP fixture executed block has canonical wire bytes")
            .len(),
    )
    .expect("exact SCCP fixture executed block wire length fits u64")
}
fn assert_exact_finalized_block_fixture(fixture: &SccpFinalizedBlockTestFixtureV1) {
    assert_eq!(fixture.proof.block_header, fixture.block.header());
    assert_eq!(
        fixture.proof.finality_artifact.block_hash,
        fixture.block.hash()
    );
    assert_eq!(
        fixture.proof.finality_artifact.subject.payload_hash,
        exact_fixture_proposal_wire_hash(&fixture.block),
        "the finality subject must bind the canonical resultless proposal wire image"
    );
    assert_eq!(
        fixture
            .proof
            .finality_artifact
            .commit_qc
            .execution_commitment
            .executed_block_wire_len,
        exact_fixture_executed_wire_len(&fixture.block),
        "the execution commitment must bind the complete result-bearing block wire length"
    );
    assert_eq!(
        fixture
            .proof
            .finality_artifact
            .commit_qc
            .execution_commitment
            .executed_block_wire_hash,
        exact_fixture_executed_wire_hash(&fixture.block),
        "the execution commitment must bind the complete result-bearing block wire image"
    );
    fixture
        .proof
        .finality_artifact
        .validate_for_header(&fixture.block.header())
        .expect("exact SCCP fixture finality binds its complete block header");
    fixture
        .proof
        .finality_artifact
        .verify()
        .expect("exact SCCP fixture finality is cryptographically valid");
}
#[expect(
    clippy::too_many_lines,
    reason = "the canonical fixture matrix keeps entrypoint, result, body-root, and exact SCCP ordering assertions together"
)]
fn assert_exact_fixture_block_body(block: &SignedBlock) {
    assert!(
        block.has_results(),
        "an exact finalized block fixture must carry its complete execution results"
    );
    let external_entrypoints = block.external_entrypoints_cloned().collect::<Vec<_>>();
    let external_root = external_entrypoints
        .iter()
        .map(TransactionEntrypoint::hash)
        .collect::<MerkleTree<TransactionEntrypoint>>()
        .root();
    assert_eq!(
        block.header().merkle_root(),
        external_root,
        "the finalized header must commit the exact external entrypoint order"
    );
    let entrypoint_hashes = block
        .entrypoints_cloned()
        .map(|entrypoint| entrypoint.hash())
        .collect::<Vec<_>>();
    assert_eq!(
        block.entrypoint_hashes().collect::<Vec<_>>(),
        entrypoint_hashes,
        "the attached full-entrypoint Merkle tree must match the canonical entrypoints"
    );
    let results = block.results().collect::<Vec<_>>();
    assert_eq!(
        results.len(),
        entrypoint_hashes.len(),
        "every canonical entrypoint must have exactly one committed result"
    );
    let result_hashes = results
        .iter()
        .map(|result| result.hash())
        .collect::<Vec<_>>();
    assert_eq!(
        block.result_hashes().collect::<Vec<_>>(),
        result_hashes,
        "the attached result Merkle tree must match the exact result vector"
    );
    let result_root = result_hashes
        .iter()
        .copied()
        .collect::<MerkleTree<TransactionResult>>()
        .root();
    assert_eq!(
        block.header().result_merkle_root(),
        result_root,
        "the finalized header must commit the exact transaction-result vector"
    );
    let mut commitments = Vec::new();
    let mut seen = BTreeSet::new();
    for (entrypoint_index, entrypoint) in external_entrypoints.iter().enumerate() {
        if results
            .get(entrypoint_index)
            .is_none_or(|result| result.as_ref().is_err())
        {
            continue;
        }
        let transaction = match entrypoint {
            TransactionEntrypoint::External(transaction) => transaction,
            TransactionEntrypoint::SealedReveal(reveal) => reveal.signed_transaction(),
            TransactionEntrypoint::SealedCommitment(_) | TransactionEntrypoint::Time(_) => {
                continue;
            }
        };
        let instructions: Vec<&InstructionBox> = match transaction.instructions() {
            Executable::Instructions(instructions) => instructions.iter().collect(),
            Executable::IvmProved(proved) => proved.overlay.iter().collect(),
            Executable::Batch(items) => items
                .iter()
                .filter_map(|item| match item {
                    iroha_data_model::transaction::ExecutableBatchItem::Instruction(
                        instruction,
                    ) => Some(instruction),
                    iroha_data_model::transaction::ExecutableBatchItem::ContractCall(_) => None,
                })
                .collect(),
            Executable::ContractCall(_) | Executable::Ivm(_) => continue,
        };
        for instruction in instructions {
            let Some(record) = instruction.as_any().downcast_ref::<RecordSccpMessage>() else {
                continue;
            };
            let payload = decode_canonical_sccp_payload_bytes(&record.payload_bytes)
                .expect("successful exact SCCP record must use canonical payload bytes");
            assert_eq!(
                canonical_sccp_payload_bytes(&payload)
                    .expect("exact SCCP record payload re-encodes canonically"),
                record.payload_bytes
            );
            let commitment = hub_commitment_from_sccp_payload(record.context, &payload)
                .expect("successful exact SCCP record must have a well-formed outbound context");
            assert!(
                seen.insert((record.context.lane, commitment.message_id)),
                "an exact finalized block must not repeat an outbound SCCP replay key"
            );
            commitments.push(commitment);
        }
    }
    let maximum =
        usize::try_from(iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1)
            .expect("SCCP block bound fits usize");
    assert!(commitments.len() <= maximum);
    assert_eq!(
        block.header().sccp_commitment_root(),
        commitment_merkle_root(&commitments),
        "the finalized header SCCP root must match successful record instructions exactly"
    );
}
/// Finalize one complete height-one or height-two block with the test-only Taira roster.
///
/// This helper is available only to crate tests or consumers of the existing
/// `test-fixtures` feature. It provides no caller-selected signing material and
/// must not be used by production release tooling. The returned opaque parent
/// type proves that a successor reuses the exact `CommitQC` of a proof already
/// bound to both canonical proposal and result-bearing signed-block wire images.
///
/// # Panics
///
/// Panics if `block` is outside the exact two-height fixture corridor, has an
/// invalid or non-exact parent, carries an SCCP root without both transaction
/// Merkle roots, or cannot be bound to a cryptographically valid artifact.
#[must_use]
#[expect(
    clippy::too_many_lines,
    reason = "the test-only signer keeps the ordered block, roster, context, QC, and aggregate-binding checks cohesive"
)]
pub fn sccp_finalize_taira_block_test_fixture_v1(
    block: &SignedBlock,
    parent: Option<&SccpFinalizedBlockTestFixtureV1>,
) -> SccpFinalizedBlockTestFixtureV1 {
    let block_header = block.header();
    let height = block_header.height().get();
    assert!(
        (1..=2).contains(&height),
        "the exact SCCP finality signer supports fixture heights one and two only"
    );
    if block_header.sccp_commitment_root().is_some() {
        assert!(
            block_header.merkle_root().is_some(),
            "an SCCP-finalized block must commit its external entrypoints"
        );
        assert!(
            block_header.result_merkle_root().is_some(),
            "an SCCP-finalized block must commit its transaction results"
        );
    }
    assert_exact_fixture_block_body(block);
    let mut keypairs = [
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
        .zip([1_u64; 4])
        .map(|(keypair, power)| ValidatorPower {
            validator: PeerId::new(keypair.public_key().clone()),
            power,
        })
        .collect::<Vec<_>>();
    let quorum = DualQuorum::from_roster(&roster).expect("valid SCCP fixture roster");
    let validator_set_pops = keypairs
        .iter()
        .map(|keypair| {
            iroha_crypto::bls_normal_pop_prove(keypair.private_key()).expect("BLS fixture PoP")
        })
        .collect::<Vec<_>>();
    let da_layout = DataAvailabilityLayout {
        encoding: PayloadEncoding::ReedSolomon16,
        chunk_size_bytes: 1024,
        data_shards: 1,
        parity_shards: 1,
        max_payload_size_bytes: 4096,
        max_chunk_count: 8,
    };
    let network_id = sccp_taira_finality_network_id_v1();
    let offline_cash_mint_finality_epoch_roster =
        sccp_mint_finality_roster_test_fixture_v1(network_id, 0, &roster);
    let offline_cash_mint_finality_epoch_id = offline_cash_mint_finality_epoch_roster
        .finality_epoch_id()
        .expect("valid deterministic SCCP mint-finality roster");
    let context = match (height, block_header.prev_block_hash(), parent) {
        (1, None, None) => HeightContext {
            network_id,
            protocol_version: PROTOCOL_VERSION,
            height,
            epoch: 0,
            epoch_end_height: 10,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Npos,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum,
            roster,
            nexus_amx_context_hash: Hash::new(b"exact SCCP fixture Nexus/AMX context"),
            execution_policy_hash: Hash::new(b"exact SCCP fixture execution policy"),
            da_layout,
            leader_seed: [0x5a; 32],
            offline_cash_mint_finality_epoch_id,
            offline_cash_mint_finality_epoch_roster,
        },
        (2, Some(parent_hash), Some(parent)) => {
            assert_exact_finalized_block_fixture(parent);
            assert_eq!(parent.block().header().height().get(), 1);
            assert_eq!(parent_hash, parent.block().hash());
            assert_eq!(parent.proof().finality_artifact.height, 1);
            assert!(
                parent
                    .proof()
                    .finality_artifact
                    .height_context
                    .next_epoch_snapshot
                    .is_none(),
                "the two-height exact corridor does not synthesize an epoch transition"
            );
            let parent_context = &parent.proof().finality_artifact.height_context;
            HeightContext {
                network_id: parent_context.network_id,
                protocol_version: PROTOCOL_VERSION,
                height,
                epoch: parent_context.epoch,
                epoch_end_height: parent_context.epoch_end_height,
                next_epoch_snapshot: None,
                mode: parent_context.mode,
                parent_commit_qc: Some(parent.proof().finality_artifact.commit_qc.clone()),
                snapshot_bootstrap: None,
                quorum: parent_context.quorum,
                roster: parent_context.roster.clone(),
                nexus_amx_context_hash: parent_context.nexus_amx_context_hash,
                execution_policy_hash: parent_context.execution_policy_hash,
                da_layout: parent_context.da_layout,
                leader_seed: parent_context.leader_seed,
                offline_cash_mint_finality_epoch_id: parent_context
                    .offline_cash_mint_finality_epoch_id,
                offline_cash_mint_finality_epoch_roster: parent_context
                    .offline_cash_mint_finality_epoch_roster
                    .clone(),
            }
        }
        _ => panic!(
            "height one requires no parent and height two requires the exact complete parent fixture"
        ),
    };
    let subject = BlockSubject {
        parent_block_hash: block_header.prev_block_hash(),
        block_hash: block_header.hash(),
        payload_hash: exact_fixture_proposal_wire_hash(block),
    };
    let round = ConsensusRound {
        context_id: context.id(),
        height,
        // The finality artifact duplicates the finalized header's
        // view-change index and must bind it exactly.
        view: block_header.view_change_index(),
    };
    let mut commit_qc = QuorumCertificate {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment: ExecutionCommitment::without_offline_cash_top_ups_or_merge_carrier(
            Hash::new(b"exact SCCP fixture parent state"),
            Hash::new(b"exact SCCP fixture post state"),
            Hash::new(b"exact SCCP fixture ordinary writes"),
            exact_fixture_executed_wire_len(block),
            exact_fixture_executed_wire_hash(block),
        ),
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
        .map(Signature::payload)
        .collect::<Vec<_>>();
    commit_qc.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
        .expect("aggregate BLS fixture votes");
    let finality_artifact =
        V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops);
    finality_artifact
        .validate_for_header(&block_header)
        .expect("exact SCCP finality artifact binds the supplied block header");
    finality_artifact
        .verify()
        .expect("exact SCCP finality artifact is cryptographically valid");
    let finalized = SccpFinalizedBlockTestFixtureV1 {
        block: block.clone(),
        proof: TairaBridgeFinalityProofV1 {
            version: BRIDGE_FINALITY_PROOF_VERSION_V2,
            block_header,
            finality_artifact,
        },
    };
    assert_exact_finalized_block_fixture(&finalized);
    finalized
}
fn exact_sccp_fixture_block(
    context: SccpOutboundMessageContextV1,
    payload: &SccpPayloadV1,
    commitment_root: Option<H256>,
    height: u64,
    previous: Option<iroha_crypto::HashOf<BlockHeader>>,
) -> SignedBlock {
    let payload_bytes =
        canonical_sccp_payload_bytes(payload).expect("exact SCCP fixture payload is canonical");
    let transaction_key = KeyPair::try_from_seed(vec![0x31; 32], Algorithm::Ed25519)
        .expect("exact SCCP fixture transaction key");
    let authority = AccountId::new(transaction_key.public_key().clone());
    let mut transaction_builder = TransactionBuilder::new(
        sccp_taira_finality_network_id_v1(),
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    transaction_builder.set_creation_time(Duration::from_millis(1_700_000_000_001));
    let transaction = transaction_builder
        .with_executable(Executable::IvmProved(IvmProved {
            bytecode: IvmBytecode::from_compiled(vec![0x01, 0x02, 0x03]),
            overlay: vec![InstructionBox::from(RecordSccpMessage::new(
                context,
                payload_bytes,
                iroha_data_model::bridge::SccpSparseMerkleWitnessV1::empty_shard(),
            ))]
            .into(),
            events_commitment: Hash::new(b"exact SCCP fixture events"),
            gas_policy_commitment: Hash::new(b"exact SCCP fixture gas policy"),
        }))
        .sign(transaction_key.private_key());
    let entrypoint_hash = transaction.hash_as_entrypoint();
    let mut header = BlockHeader::new(
        NonZeroU64::new(height).expect("exact SCCP fixture height is nonzero"),
        previous,
        None,
        None,
        1_700_000_000_002,
        0,
    );
    header.set_sccp_commitment_root(commitment_root);
    let block_key = KeyPair::try_from_seed(vec![0x32; 32], Algorithm::Ed25519)
        .expect("exact SCCP fixture block key");
    let provisional_signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(block_key.private_key(), header.hash())
            .expect("sign exact SCCP provisional block header"),
    );
    let mut block = SignedBlock::presigned(provisional_signature, header, vec![transaction]);
    // Exercise the active NPoS-effects header commitment in the exact SCCP fixture.
    block.set_npos_consensus_effects(Some(NposConsensusEffects {
        finalized_global_beacon_pulse: None,
        v2_evidence_admissions: Vec::new(),
        penalty_actions: vec![NposPenaltyAction::MarkConsensusEvidenceApplied(
            NposMarkConsensusEvidenceAppliedAction {
                evidence_key: Hash::new(b"exact-sccp-fixture"),
                height,
            },
        )],
    }));
    block
        .set_transaction_results(
            Vec::new(),
            &[entrypoint_hash],
            vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
        )
        .expect("exact SCCP fixture block results match its transaction");
    let final_signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(block_key.private_key(), block.hash())
            .expect("sign exact SCCP complete block header"),
    );
    block
        .replace_signatures([final_signature].into_iter().collect())
        .expect("replace exact SCCP provisional block signature");
    block
        .signatures()
        .next()
        .expect("exact SCCP block signature")
        .signature()
        .verify_hash(block_key.public_key(), block.hash())
        .expect("exact SCCP complete block signature verifies");
    block
}
#[cfg(test)]
pub fn signed_finality_proof_for_message_test_fixture_v1(
    context: SccpOutboundMessageContextV1,
    payload: &SccpPayloadV1,
    commitment_root: H256,
) -> Vec<u8> {
    let block = exact_sccp_fixture_block(context, payload, Some(commitment_root), 1, None);
    let finalized = sccp_finalize_taira_block_test_fixture_v1(&block, None);
    to_bytes(finalized.proof()).expect("canonical complete-block SCCP finality fixture")
}
fn message_bundle(
    route: &SccpGovernedRouteV1,
    nonce: u64,
) -> (TairaSccpMessageProofV1, SccpFinalizedBlockTestFixtureV1) {
    message_bundle_with_payload(route, transfer_payload(route, nonce))
}
fn message_bundle_with_payload(
    route: &SccpGovernedRouteV1,
    payload: SccpPayloadV1,
) -> (TairaSccpMessageProofV1, SccpFinalizedBlockTestFixtureV1) {
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
    let block = exact_sccp_fixture_block(context, &payload, Some(commitment_root), 1, None);
    let finalized_block = sccp_finalize_taira_block_test_fixture_v1(&block, None);
    let bundle = TairaSccpMessageProofV1 {
        version: 1,
        commitment_root,
        commitment,
        merkle_proof,
        payload,
        finality_proof: to_bytes(finalized_block.proof())
            .expect("canonical exact SCCP finality fixture"),
    };
    assert!(verify_message_bundle_structure(&bundle));
    assert!(
        verified_sccp_message_taira_finality_proof_cryptographically_self_consistent(&bundle)
            .is_some()
    );
    (bundle, finalized_block)
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
fn valid_bls12381_proof_bytes(request: &SccpTonGroth16Bls12381ProofRequestV1) -> Vec<u8> {
    let mut a_scalar = Bls12381Fr::from(3_u64);
    for signal in request.public_signals.words() {
        a_scalar += bls12381_fr_from_be_word(signal).expect("canonical exact TON scalar signal");
    }
    let proof = SccpGroth16Bls12381ProofV1 {
        a: SccpBls12381G1CompressedV1 {
            bytes: bls12381_g1_bytes((Bls12381G1Affine::generator() * a_scalar).to_affine())
                .to_vec(),
        },
        b: SccpBls12381G2CompressedV1 {
            bytes: bls12381_g2_bytes(Bls12381G2Affine::generator()).to_vec(),
        },
        c: SccpBls12381G1CompressedV1 {
            bytes: bls12381_g1_bytes(Bls12381G1Affine::generator()).to_vec(),
        },
    };
    canonical_sccp_groth16_bls12381_proof_bytes_v1(&proof)
        .expect("exact TON proof has canonical compressed bytes")
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
    let (bundle, finalized_block) = message_bundle(&route, nonce);
    let request = build_sccp_groth16_bn254_proof_request_from_governed_route_v1(&bundle, &route)
        .expect("exact SCCP governed proof request");
    let proof_bytes = valid_proof(&request);
    assert!(verify_sccp_groth16_bn254_proof_v1(&request, &proof_bytes,));
    let artifact = wrap_sccp_evm_groth16_bn254_proof_result(&proof_bytes, &request)
        .expect("exact SCCP Groth16 artifact");
    let bridge_proof =
        bridge_sccp_destination_proof_v1(&artifact).expect("closed exact SCCP bridge proof");
    assert!(
        verify_sccp_destination_proof_v1(&bridge_proof, &bundle, &route, finalized_block.proof(),)
            .is_some()
    );
    SccpExactOutboundTestFixtureV1 {
        route,
        bundle,
        request,
        artifact,
        bridge_proof,
        finalized_block,
    }
}
/// Build an owned, end-to-end exact TON-mainnet outbound test fixture.
///
/// # Panics
///
/// Panics if any fixed vector stops satisfying the production SCCP
/// canonicalization, governed-route, finality, or BLS12-381 verification rules.
#[must_use]
pub fn sccp_exact_ton_outbound_test_fixture_v1() -> SccpExactTonOutboundTestFixtureV1 {
    let route = sccp_exact_ton_governed_route_test_fixture_v1(
        SccpNetworkV1::TonMainnet,
        SccpRouteActivationV1::Bidirectional,
    );
    let (bundle, finalized_block) =
        message_bundle_with_payload(&route, ton_transfer_payload(&route, 19));
    assert!(
        build_sccp_groth16_bn254_proof_request_from_governed_route_v1(&bundle, &route).is_none(),
        "TON proving material must never enter the BN254 path"
    );
    let request =
        build_sccp_ton_groth16_bls12381_proof_request_from_governed_route_v1(&bundle, &route)
            .expect("exact SCCP governed TON proof request");
    let proof_bytes = valid_bls12381_proof_bytes(&request);
    let artifact = wrap_sccp_ton_groth16_bls12381_proof_result_v1(&proof_bytes, &request)
        .expect("exact SCCP TON Groth16 artifact");
    let bridge_proof = bridge_sccp_ton_destination_proof_v1(&artifact)
        .expect("closed exact SCCP TON bridge proof");
    assert!(
        verify_sccp_destination_proof_v1(&bridge_proof, &bundle, &route, finalized_block.proof(),)
            .is_some()
    );
    SccpExactTonOutboundTestFixtureV1 {
        route,
        bundle,
        request,
        artifact,
        bridge_proof,
        finalized_block,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ton_fixture_exercises_closed_bls12381_destination_path() {
        let fixture = sccp_exact_ton_outbound_test_fixture_v1();
        assert_eq!(fixture.route.lane_id.source, SccpNetworkV1::TonMainnet);
        assert_eq!(
            fixture.request.backend,
            BridgeSccpDestinationProofBackendV1::TonGroth16Bls12381
        );
        assert_eq!(
            decode_and_parse_canonical_sccp_destination_proof_v1(
                &to_bytes(&fixture.bridge_proof).expect("canonical outer TON fixture envelope")
            )
            .expect("closed outer TON fixture decodes")
            .1
            .backend(),
            BridgeSccpDestinationProofBackendV1::TonGroth16Bls12381
        );
    }
    #[test]
    fn finalized_block_rebinding_authenticates_every_exact_hash_role() {
        let fixture = sccp_exact_outbound_test_fixture_v1();
        let default_finality = decode_taira_bridge_finality_proof(&fixture.bundle.finality_proof)
            .expect("default exact finality decodes");
        assert_eq!(default_finality.finality_artifact.height, 1);
        assert_eq!(
            default_finality
                .finality_artifact
                .height_context
                .epoch_end_height,
            10
        );
        assert!(
            default_finality
                .finality_artifact
                .height_context
                .parent_commit_qc
                .is_none()
        );
        let context = &default_finality.finality_artifact.height_context;
        assert_eq!(
            context
                .offline_cash_mint_finality_epoch_roster
                .finality_epoch_id(),
            Ok(context.offline_cash_mint_finality_epoch_id),
            "the SCCP fixture must carry a self-authenticating Offline Cash mint-finality roster"
        );
        assert!(
            context
                .roster
                .iter()
                .map(|validator| &validator.validator)
                .eq(context
                    .offline_cash_mint_finality_epoch_roster
                    .validators
                    .iter()
                    .map(|validator| &validator.validator)),
            "the Pasta fixture authority must exactly match consensus roster order"
        );
        let block = fixture.finalized_block.block().clone();
        let header = block.header();
        let rebound = fixture.with_finalized_block(&block, None);
        let finality = decode_taira_bridge_finality_proof(&rebound.bundle.finality_proof)
            .expect("rebound exact finality decodes");
        assert_eq!(finality.block_header, header);
        assert_eq!(finality.finality_artifact.block_hash, header.hash());
        assert_eq!(
            rebound.request.public_inputs.finality_block_hash,
            <[u8; 32]>::from(Hash::from(header.hash()))
        );
        finality
            .finality_artifact
            .validate_for_header(&header)
            .expect("rebound artifact authenticates the exact complete header");
        finality
            .finality_artifact
            .verify()
            .expect("rebound artifact remains cryptographically valid");
        assert!(
            verify_sccp_destination_proof_v1(
                &rebound.bridge_proof,
                &rebound.bundle,
                &rebound.route,
                rebound.finalized_block.proof(),
            )
            .is_some()
        );
    }
    #[test]
    fn finalized_height_two_rebinding_authenticates_parent_and_request() {
        let fixture = sccp_exact_outbound_test_fixture_v1();
        let parent = fixture.finalized_block.clone();
        let block = exact_sccp_fixture_block(
            fixture.bundle.commitment.context,
            &fixture.bundle.payload,
            Some(fixture.bundle.commitment_root),
            2,
            Some(parent.block().hash()),
        );
        let header = block.header();
        let rebound = fixture.with_finalized_block(&block, Some(&parent));
        let finality = decode_taira_bridge_finality_proof(&rebound.bundle.finality_proof)
            .expect("height-two exact finality decodes");
        assert_eq!(finality.block_header, header);
        assert_eq!(finality.finality_artifact.height, 2);
        let inherited_parent_qc = finality
            .finality_artifact
            .height_context
            .parent_commit_qc
            .as_ref()
            .expect("height-two fixture parent CommitQC");
        assert_eq!(
            inherited_parent_qc.round.view, 0,
            "the exact height-one fixture finalizes in view zero"
        );
        assert!(
            inherited_parent_qc
                .as_ref()
                .same_commit_decision(parent.proof().finality_artifact.commit_qc.as_ref()),
            "the successor must freeze the exact finalized parent decision"
        );
        assert_eq!(
            inherited_parent_qc,
            &parent.proof().finality_artifact.commit_qc,
            "the fixture successor must carry the exact parent CommitQC, not a reconstructed adjacent decision"
        );
        assert_eq!(
            finality.finality_artifact.height_context.epoch,
            parent.proof().finality_artifact.height_context.epoch,
            "an in-epoch successor must inherit its parent's epoch"
        );
        assert_eq!(
            finality
                .finality_artifact
                .height_context
                .offline_cash_mint_finality_epoch_id,
            parent
                .proof()
                .finality_artifact
                .height_context
                .offline_cash_mint_finality_epoch_id,
            "an in-epoch successor must inherit the exact Pasta authority identifier"
        );
        assert_eq!(
            finality
                .finality_artifact
                .height_context
                .offline_cash_mint_finality_epoch_roster,
            parent
                .proof()
                .finality_artifact
                .height_context
                .offline_cash_mint_finality_epoch_roster,
            "an in-epoch successor must inherit the exact Pasta authority roster"
        );
        assert_eq!(
            finality.finality_artifact.height_context.epoch_end_height,
            parent
                .proof()
                .finality_artifact
                .height_context
                .epoch_end_height,
            "an in-epoch successor must inherit its parent's epoch boundary"
        );
        assert!(
            parent
                .proof()
                .finality_artifact
                .height_context
                .next_epoch_snapshot
                .is_none()
        );
        assert!(
            finality
                .finality_artifact
                .height_context
                .next_epoch_snapshot
                .is_none()
        );
        assert_eq!(
            finality
                .finality_artifact
                .height_context
                .parent_commit_qc
                .as_ref()
                .expect("height-two fixture parent CommitQC")
                .subject
                .block_hash,
            parent.block().hash()
        );
        assert_eq!(rebound.request.public_inputs.finality_height, 2);
        assert_eq!(
            rebound.request.public_inputs.finality_block_hash,
            <[u8; 32]>::from(Hash::from(header.hash()))
        );
        finality
            .finality_artifact
            .validate_for_header(&header)
            .expect("height-two artifact binds its full header lineage");
        finality
            .finality_artifact
            .verify()
            .expect("height-two artifact is cryptographically valid");
    }
    #[test]
    fn finalized_block_rebinding_rejects_rootless_and_mismatched_blocks() {
        let fixture = sccp_exact_outbound_test_fixture_v1();
        for block in [
            exact_sccp_fixture_block(
                fixture.bundle.commitment.context,
                &fixture.bundle.payload,
                None,
                1,
                None,
            ),
            exact_sccp_fixture_block(
                fixture.bundle.commitment.context,
                &fixture.bundle.payload,
                Some([0xA5; 32]),
                1,
                None,
            ),
        ] {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    fixture.with_finalized_block(&block, None)
                }))
                .is_err(),
                "rootless or mismatched finalized block must fail closed"
            );
        }
    }
    #[test]
    fn successor_rejects_header_matching_parent_wire_substitution() {
        let fixture = sccp_exact_outbound_test_fixture_v1();
        let parent = fixture.finalized_block.clone();
        let mut substituted_block = parent.block().clone();
        let substitute_key = KeyPair::try_from_seed(vec![0x7C; 32], Algorithm::Ed25519)
            .expect("substitute block key");
        let substitute_signature = BlockSignature::new(
            0,
            SignatureOf::try_from_hash(substitute_key.private_key(), substituted_block.hash())
                .expect("sign substituted block envelope"),
        );
        substituted_block
            .replace_signatures([substitute_signature].into_iter().collect())
            .expect("replace substituted block signature");
        assert_eq!(substituted_block.header(), parent.block().header());
        assert_ne!(
            exact_fixture_proposal_wire_hash(&substituted_block),
            parent.proof().finality_artifact.subject.payload_hash
        );
        assert_ne!(
            exact_fixture_executed_wire_hash(&substituted_block),
            parent
                .proof()
                .finality_artifact
                .commit_qc
                .execution_commitment
                .executed_block_wire_hash
        );
        parent
            .proof()
            .finality_artifact
            .validate_for_header(&substituted_block.header())
            .expect("header-only validation cannot detect a signed-body substitution");
        let forged_parent = SccpFinalizedBlockTestFixtureV1 {
            block: substituted_block,
            proof: parent.proof().clone(),
        };
        let child = exact_sccp_fixture_block(
            fixture.bundle.commitment.context,
            &fixture.bundle.payload,
            Some(fixture.bundle.commitment_root),
            2,
            Some(parent.block().hash()),
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                sccp_finalize_taira_block_test_fixture_v1(&child, Some(&forged_parent))
            }))
            .is_err(),
            "a successor must reject a parent whose header matches but canonical wire does not"
        );
    }
    #[test]
    fn finality_signer_rejects_fixed_header_external_entrypoint_tamper() {
        let fixture = sccp_exact_outbound_test_fixture_v1();
        let mut tampered = fixture.finalized_block.block().clone();
        let canonical_header = tampered.header();
        tampered.set_external_entrypoints(Vec::new());
        tampered.replace_header_for_testing(canonical_header);
        assert_eq!(tampered.header(), canonical_header);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                sccp_finalize_taira_block_test_fixture_v1(&tampered, None)
            }))
            .is_err(),
            "the test signer must reject a body whose external entrypoints do not match its fixed header"
        );
    }
    #[test]
    fn finality_signer_rejects_fixed_header_result_envelope_tamper() {
        let fixture = sccp_exact_outbound_test_fixture_v1();
        let mut tampered = fixture.finalized_block.block().clone();
        let canonical_header = tampered.header();
        assert!(tampered.update_transaction_result(
            0,
            &TransactionResultInner::Err(
                iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                    iroha_data_model::ValidationFail::NotPermitted(
                        "adversarial SCCP result envelope".to_owned(),
                    ),
                ),
            ),
        ));
        tampered.replace_header_for_testing(canonical_header);
        assert_eq!(tampered.header(), canonical_header);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                sccp_finalize_taira_block_test_fixture_v1(&tampered, None)
            }))
            .is_err(),
            "the test signer must reject a result envelope whose Merkle root does not match its fixed header"
        );
    }
    #[test]
    fn finality_signer_rejects_internally_consistent_body_with_stale_sccp_root() {
        let fixture = sccp_exact_outbound_test_fixture_v1();
        let alternate = sccp_exact_outbound_test_fixture_for_nonce_v1(8);
        let mut tampered = alternate.finalized_block.block().clone();
        let alternate_header = tampered.header();
        assert_eq!(
            alternate_header.sccp_commitment_root(),
            Some(alternate.bundle.commitment_root)
        );
        assert_ne!(
            fixture.bundle.commitment_root, alternate.bundle.commitment_root,
            "the substituted instruction body must have a distinct SCCP commitment"
        );
        let mut stale_header = alternate_header;
        stale_header.set_sccp_commitment_root(Some(fixture.bundle.commitment_root));
        tampered.replace_header_for_testing(stale_header);
        let block_key = KeyPair::try_from_seed(vec![0x32; 32], Algorithm::Ed25519)
            .expect("exact SCCP fixture block key");
        let final_signature = BlockSignature::new(
            0,
            SignatureOf::try_from_hash(block_key.private_key(), tampered.hash())
                .expect("sign adversarial complete block header"),
        );
        tampered
            .replace_signatures([final_signature].into_iter().collect())
            .expect("replace adversarial block signature");
        tampered
            .signatures()
            .next()
            .expect("adversarial block signature")
            .signature()
            .verify_hash(block_key.public_key(), tampered.hash())
            .expect("adversarial block signature verifies");
        assert_eq!(
            tampered.header().merkle_root(),
            alternate_header.merkle_root()
        );
        assert_eq!(
            tampered.header().result_merkle_root(),
            alternate_header.result_merkle_root()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                sccp_finalize_taira_block_test_fixture_v1(&tampered, None)
            }))
            .is_err(),
            "the test signer must reject a complete, internally consistent body whose SCCP root was replaced by a stale root"
        );
    }
}
