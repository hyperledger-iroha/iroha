//! Emit the strict line protocol consumed by the final-V1 StateInit golden generator.

import "@acton/build"
import "@acton/io"
import "@contracts/constants"
import "@tests/stateinit-golden-fixture"

fun emitHash(name: string, value: int) {
    println("{}={:x}", name, value);
}

fun main() {
    val configCell = stateInitGoldenBridgeConfig();
    val config = configCell.load();
    val routeHashes = config.routeHashes.load();
    val routeTail = routeHashes.tail.load();
    val codeHashes = routeTail.deploymentCodeHashes.load();
    val finality = config.finalityHashes.load();
    val finalityTail = finality.tail.load();
    val guardians = finalityTail.mintBreakerGuardians.load();
    val guardianTail = guardians.tail.load();
    val bridgeState = stateInitGoldenBridgeState();
    val bridgeAddress = stateInitGoldenBridgeAddress();
    val (_, bridgeAddressHash) = bridgeAddress.getWorkchainAndHash();
    val masterState = stateInitGoldenMasterState();
    val masterAddress = stateInitGoldenMasterAddress();
    val (_, masterAddressHash) = masterAddress.getWorkchainAndHash();

    println("schema=iroha.sccp.ton-stateinit-golden.final-v1");
    println("expected_global_id={}", config.expectedGlobalId);
    println("workchain={}", SCCP_WORKCHAIN);
    println("storage_version={}", SCCP_STORAGE_VERSION);
    println("route_revision={}", config.routeRevision);
    println("taira_to_ton_multiplier={}", config.tairaToTonMultiplier);
    println("max_wrapped_supply={}", config.maxWrappedSupply);
    emitHash("source_lane_cell_hash", config.sourceLaneBytes.hash());
    emitHash("source_lane_hash", routeHashes.sourceLaneHash);
    emitHash("destination_lane_cell_hash", config.destinationLaneBytes.hash());
    emitHash("destination_lane_hash", routeHashes.destinationLaneHash);
    emitHash("jetton_master_code_hash", codeHashes.jettonMasterCodeHash);
    emitHash("jetton_wallet_code_hash", codeHashes.jettonWalletCodeHash);
    emitHash("route_code_hash", codeHashes.routeCodeHash);
    emitHash("semantic_proof_profile_hash", routeTail.semanticProofProfileHash);
    emitHash("sora_finality_anchor_hash", finality.soraFinalityAnchorHash);
    emitHash("verifier_circuit_hash", finality.verifierCircuitHash);
    emitHash("verifying_key_hash", finality.verifyingKeyHash);
    emitHash("proof_profile_commitment", finalityTail.proofProfileCommitment);
    emitHash("embedded_verifier_code_hash", finalityTail.embeddedVerifierCodeHash);
    emitHash("verifying_key_cell_hash", finalityTail.verifyingKeyCellHash);
    emitHash("master_metadata_cell_hash", finalityTail.masterMetadata.hash());
    emitHash("guardian_0", guardians.guardian0);
    emitHash("guardian_1", guardians.guardian1);
    emitHash("guardian_2", guardians.guardian2);
    emitHash("guardian_3", guardianTail.guardian3);
    emitHash("guardian_4", guardianTail.guardian4);
    emitHash("destination_binding_hash", routeTail.destinationBindingHash);
    emitHash("route_configuration_hash", routeHashes.routeConfigurationHash);
    emitHash("bridge_config_cell_hash", configCell.hash());
    emitHash("bridge_replay_cell_hash", emptyBridgeReplayState().hash());
    emitHash("bridge_pending_cell_hash", emptyBridgePendingState().hash());
    emitHash("master_replay_cell_hash", emptyMasterReplayState().hash());
    emitHash("bridge_initial_data_cell_hash", bridgeState.data.hash());
    emitHash("bridge_state_init_account_hash", bridgeAddressHash);
    emitHash("master_initial_data_cell_hash", masterState.data.hash());
    emitHash("master_state_init_account_hash", masterAddressHash);
}
