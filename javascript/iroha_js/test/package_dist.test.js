"use strict";

import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";

import {
  AccountAddress,
  SCCP_DOMAIN_BSC,
  SCCP_DOMAIN_ETH,
  SCCP_DOMAIN_SOL,
  SCCP_DOMAIN_SORA,
  SCCP_DOMAIN_SORA_KUSAMA,
  SCCP_DOMAIN_TON,
  SCCP_DOMAIN_TRON,
  SCCP_ETH_MAINNET_EVM_CHAIN_ID,
  SCCP_ETH_MAINNET_NETWORK_ID,
  SCCP_BSC_MAINNET_EVM_CHAIN_ID,
  SCCP_BSC_MAINNET_NETWORK_ID,
  SCCP_BSC_TESTNET_EVM_CHAIN_ID,
  SCCP_BSC_TESTNET_NETWORK_ID,
  SCCP_STARK_FRI_PROOF_FAMILY_V1,
  SCCP_SOURCE_STATE_MAX_PROOF_BYTES,
  SCCP_SOURCE_STATE_MAX_PROOF_LABEL_BYTES,
  SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES,
  SCCP_SOURCE_ADAPTER_FASTPQ_PARAMETER_SET_V1,
  SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1,
  SCCP_SUBSTRATE_RUNTIME_PROOF_BACKEND_V1,
  SCCP_SUBSTRATE_RUNTIME_CALL_SCALE_V1,
  KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1,
  KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1,
  KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
  KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1,
  KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT,
  KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT,
  KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_BRIDGE_ABI_VERSION,
  KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION,
  KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
  KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
  KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1,
  KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1,
  KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES,
  KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES,
  KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES,
  KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN,
  KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN,
  KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1,
  canAppendKagemushaRecursiveSpendWitnesslessLineage,
  canProveKagemushaRecursiveSpendAppendOutputProofCircuitId,
  canRedeemKagemushaRecursiveSpendWitnessless,
  canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId,
  isKagemushaCompactPaymentTokenNativeAvailable,
  isKagemushaRecursiveAggregationProofBundleNativeAvailable,
  isKagemushaRecursiveCompactPaymentTokenNativeAvailable,
  isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable,
  isKagemushaRecursiveCompactUnavailable,
  isSupportedKagemushaRecursiveSpendLineageKeyArtifactOpeningLen,
  isKagemushaRecursiveSpendLineageProofCircuitId,
  isKagemushaRecursiveSpendLineageAppendOutputCircuitId,
  isSupportedKagemushaRecursiveSpendAppendOutputProofCircuitId,
  isSupportedKagemushaRecursiveSpendAppendProofTransition,
  isSupportedKagemushaRecursiveSpendPreviousProofCircuitId,
  normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId,
  preferredKagemushaRecursiveSpendAppendOutputProofCircuitId,
  preferredKagemushaOfflineSpendModeForCapabilities,
  kagemushaRecursiveSpendLineageKeyArtifacts,
  kagemushaRecursiveSpendLineageKeyArtifactsForAppend,
  kagemushaRecursiveSpendLineageKeyArtifactsForInit,
  validateKagemushaRecursiveSpendLineageKeyArtifacts,
  requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput,
  requiresKagemushaRecursiveSpendLineageKeyArtifactsForInit,
  requiresKagemushaRecursiveSpendLineageWitnessForRedeem,
  requiresKagemushaRecursiveSpendPreviousLineageVerifierRecordForAppend,
  requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend,
  PRIVACY_FFI_ERROR_INVALID_REQUEST,
  PRIVACY_FFI_ERROR_MALFORMED_NORITO,
  PRIVACY_FFI_ERROR_NULL_POINTER,
  PRIVACY_FFI_ERROR_PRODUCTION_DISABLED,
  PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM,
  PRIVACY_FFI_STATUS_ERROR,
  PRIVACY_FFI_VERSION_V1,
  PRIVACY_REQUIRED_BRIDGE_ABI_VERSION,
  SCCP_MESSAGE_TRANSPARENT_PUBLIC_INPUTS_BYTES_V1_LEN,
  SCCP_TON_CURRENT_VALIDATOR_SET_CONFIG_PARAM,
  SCCP_TON_MESSAGE_BODY_BOC_V1,
  SCCP_TON_MAINNET_SHARD_STATE_VERIFIER_ID_V1,
  SCCP_TON_MAINNET_MASTERCHAIN_CONFIG_VERIFIER_ID_V1,
  SCCP_TON_MAINNET_VALIDATOR_SET_TRANSITION_VERIFIER_ID_V1,
  SCCP_TON_MAINNET_SHARD_ACCOUNTS_DICTIONARY_VERIFIER_ID_V1,
  SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1,
  SCCP_TON_MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1,
  SCCP_TON_VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1,
  SCCP_TON_SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1,
  SCCP_SOLANA_MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1,
  SCCP_SOLANA_UPGRADEABLE_LOADER_ID,
  SCCP_SOLANA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1,
  SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1,
  SCCP_SOLANA_TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1,
  SCCP_SOLANA_FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1,
  SCCP_SOLANA_BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1,
  SCCP_SOLANA_SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1,
  SCCP_SOLANA_STAKE_PROGRAM_ID,
  SCCP_SOLANA_STAKE_HISTORY_SYSVAR_ID,
  SCCP_SOLANA_SYSVAR_PROGRAM_ID,
  SCCP_SOLANA_TOWER_LOCKOUT_CONFIRMATION_DEPTH,
  SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH,
  SCCP_SOLANA_VOTE_PROGRAM_ID,
  bscCommitMessageHash,
  bscCommitSealHash,
  bscValidatorSetHashFromPayload,
  bscValidatorSetMetadataProofHash,
  bscValidatorSetPayloadFromHeaderRlp,
  bscValidatorSetPayloadFromParliaExtra,
  bscValidatorSetPayloadHash,
  bscValidatorSetStorageValueHash,
  bscValidatorSetTransitionMessageHash,
  buildEvmSccpProofRequest,
  buildEvmSccpSubmission,
  EthereumMainnetBeaconRestConsensusProvider,
  EthereumMainnetSccp,
  SCCP_ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
  SCCP_ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1,
  SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1,
  SCCP_ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1,
  SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
  SCCP_NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1,
  SCCP_NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1,
  ethereumMainnetSccpDestinationBinding,
  parseEthereumMainnetNativeEvmProverBundleManifest,
  parseEthereumMainnetNativeEvmProverParityFixture,
  parseEthereumMainnetNativeEvmProverSelfTestFixture,
  runEthereumMainnetNativeProverSelfTest,
  validateEthereumMainnetNativeEvmProverBundle,
  validateEthereumMainnetNativeEvmProverParityFixture,
  validateEthereumMainnetNativeEvmProverSelfTestFixture,
  verifyEthereumMainnetNativeEvmProverArtifacts,
  verifyEthereumMainnetNativeEvmProverArtifactsFromBundle,
  BscMainnetSccp,
  BscMainnetSccpProver,
  BscTestnetSccp,
  BscTestnetSccpProver,
  bscMainnetSccpDestinationBinding,
  bscTestnetSccpDestinationBinding,
  buildBscMainnetSccpDestinationProofRequest,
  buildBscMainnetSccpDestinationSubmission,
  buildBscTestnetSccpDestinationProofRequest,
  buildBscTestnetSccpDestinationSubmission,
  buildBscTestnetSccpLocalAdmissionSubmission,
  evmSccpDestinationBinding,
  wrapBscMainnetSccpDestinationProofResult,
  wrapBscTestnetSccpDestinationProofResult,
  wrapEvmSccpProofResult,
  buildSolanaSccpAccountsLtHashProofRequest,
  buildSolanaSccpFullLightClientAuditProofRequest,
  buildSolanaSccpTowerReplayProofRequest,
  buildSolanaSccpFullAccountsdbLatticeProofRequest,
  buildSolanaSccpBankForkChoiceProofRequest,
  buildSolanaSccpFullLightClientAuditProofRequests,
  buildSolanaSccpSubmission,
  wrapSolanaSccpSourceStateVerificationProof,
  buildSubstrateSccpProofRequest,
  buildSubstrateSccpSubmission,
  wrapSubstrateSccpProofResult,
  buildTonSccpProofRequest,
  buildTonSccpSubmission,
  buildTonShardStateProofRequest,
  buildTonSccpFullLightClientAuditProofRequest,
  buildTonSccpFullLightClientAuditProofRequests,
  buildTronSccpProofRequest,
  buildTronSccpSubmission,
  tronSccpDestinationBinding,
  wrapTonSccpProofResult,
  wrapTronSccpProofResult,
  wrapSolanaSccpProofResult,
  preferredKagemushaOfflineSpendMode,
  isKagemushaRecursiveSpendNativeAvailable,
  isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable,
  isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable,
  kagemushaProveVerifiedCompactPaymentTokenWithRecords,
  kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes,
  kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes,
  kagemushaVerifyRecursiveCompactPaymentToken,
  kagemushaRecursiveSpendCompactPaymentTokenFromBundle,
  kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection,
  kagemushaRecursiveSpendInit,
  kagemushaRecursiveSpendAppend,
  kagemushaRecursiveSpendTransitionProfileInit,
  kagemushaRecursiveSpendTransitionProfileAppend,
  kagemushaRecursiveSpendLineageAppendBoundary,
  kagemushaRecursiveSpendLineageWitnessFromInitResult,
  kagemushaRecursiveSpendLineageWitnessAppendResult,
  kagemushaRecursiveSpendVerify,
  kagemushaRecursiveSpendRedeem,
  PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
  isPrivacyNativeAvailable,
  privacyCapabilitiesV1,
  privacyBuildProofV1,
  privacyVerifyProofV1,
  getPrivacyCapabilities,
  buildPrivacyProofEnvelope,
  buildZkAtDevProofFixture,
  buildZkAmsAdmissionDevProofFixture,
  buildVegaCredentialDevProofFixture,
  buildSilentThresholdCredentialDevProofFixture,
  buildZkX509IdentityDevProofFixture,
  buildJindoLatticeDevProofFixture,
  buildSisHintsCredentialDevProofFixture,
  buildAnonymousPgcReceiverSet,
  buildAnonymousPgcDevProofFixture,
  buildVeRangeDevProofFixture,
  noritoDecodePrivacyProofEnvelope,
  canonicalBscCommitMessageBytes,
  canonicalBscCommitSealBytes,
  canonicalBscValidatorSetMetadataProofBytes,
  canonicalBscValidatorSetPayloadBytes,
  canonicalBscValidatorSetTransitionMessageBytes,
  canonicalEvmReceiptRootMptValue,
  canonicalSccpSourceAdapterEngineDeploymentBytes,
  canonicalSccpSourceVerifierMaterialBytes,
  canonicalEthSyncCommitteePayloadBytes,
  SCCP_ETH_MAINNET_SLOTS_PER_SYNC_COMMITTEE_PERIOD,
  ethMainnetSyncCommitteePeriodForSlot,
  ethSyncCommitteePayloadHash,
  ethSyncCommitteeHashFromPayload,
  buildSubstrateSccpRuntimeStorageProofRequest,
  canonicalSubstrateAuthoritySetPayloadBytes,
  canonicalSubstrateSccpRuntimeStorageVerificationStatementBytes,
  canonicalSubstrateSccpStorageProofBytes,
  substrateSccpRuntimeStorageProofPublicInputsHash,
  canonicalSubstrateAuthoritySetTransitionMessageBytes,
  canonicalSolanaSccpBankForkBytes,
  canonicalSolanaSccpRouteCanaryEvidenceBytes,
  canonicalSolanaSccpAccountsLtHashCommitmentBytes,
  canonicalSolanaSccpAccountsLtHashVerificationContextBytes,
  canonicalSolanaSccpSourceStateVerificationProofBytes,
  canonicalSolanaSccpFinalityContextBytes,
  canonicalSolanaSccpVoteMessageBytes,
  canonicalSolanaSccpFullLightClientAuditStatementBytes,
  canonicalSolanaSccpAccountInclusionLeafBytes,
  canonicalSolanaSccpAccountInclusionNodeBytes,
  canonicalSolanaSccpAccountOpeningBytes,
  canonicalSolanaSccpVoteAccountDataBytes,
  canonicalSolanaSccpStakeAccountDataBytes,
  canonicalSolanaSccpStakeActivationBytes,
  canonicalSolanaSccpStakeAccountStateBytes,
  canonicalSolanaSccpStakeHistorySysvarDataBytes,
  canonicalSolanaSccpStakeHistoryBytes,
  canonicalSolanaSccpTowerLockoutBytes,
  canonicalSolanaSccpTowerReplayBytes,
  canonicalTonShardStateProofPublicInputsBytes,
  canonicalTonShardStateVerificationContextBytes,
  canonicalTonShardStateWitnessCommitmentBytes,
  canonicalTonSccpSourceStateVerificationProofBytes,
  canonicalTonSccpRouteCanaryEvidenceBytes,
  canonicalTronSccpRouteCanaryEvidenceBytes,
  canonicalTonSccpFullLightClientAuditStatementBytes,
  canonicalTonMasterchainBlockMessageBytes,
  canonicalTonMasterchainConfigLeafBytes,
  canonicalTonMasterchainConfigProofBytes,
  canonicalTonMasterchainValidatorSignaturesBytes,
  canonicalTonValidatorSetPayloadBytes,
  canonicalTonValidatorSetTransitionMessageBytes,
  canonicalTronRawBlockHeaderBytes,
  canonicalTronReceiptRootMptValue,
  canonicalTronSccpReceiptStateProofBytes,
  canonicalTronSolidBlockMessageBytes,
  canonicalTronSolidBlockHeaderProofBytes,
  canonicalTronWitnessSealBytes,
  canonicalTronWitnessScheduleTransitionMessageBytes,
  canonicalTronWitnessScheduleTransitionSealBytes,
  canonicalTronWitnessSchedulePayloadBytes,
  ethBeaconBlockHeaderRoot,
  ethBeaconBodyRootFromExecutionPayloadBranch,
  ethExecutionPayloadHeaderRootFromRlp,
  sccpGroth16Bn254PublicSignalWords,
  sccpMessageTransparentPublicInputAbiWords,
  sccpSubmitMessageProofCallData,
  sccpDestinationBindingHash,
  sccpDestinationBindingKey,
  solanaSccpRouteCanaryEvidenceHash,
  tonSccpRouteCanaryEvidenceHash,
  tronSccpRouteCanaryEvidenceHash,
  sccpSourceAdapterEngineDeploymentHash,
  sccpSourceAdapterVerifierVkHash,
  sccpSolanaFullLightClientGateHash,
  sccpTonFullLightClientGateHash,
  sccpSourceVerifierMaterialHash,
  tonSccpShardStateVerificationProofHash,
  tonSccpFullLightClientAuditStatementHash,
  tonSccpFullLightClientAuditPublicInputColumns,
  tonSccpFullLightClientAuditOpenVerifySchemaDescriptor,
  solanaSccpAccountOpeningHash,
  solanaSccpAccountRawDataHash,
  solanaSccpAccountsLtHashProofHash,
  solanaSccpFinalityContextHash,
  solanaSccpVoteMessageHash,
  solanaSccpFullLightClientAuditStatementHash,
  solanaSccpFullLightClientAuditPublicInputColumns,
  solanaSccpFullLightClientAuditOpenVerifySchemaDescriptor,
  solanaSccpAccountInclusionLeafHash,
  solanaSccpAccountInclusionNodeHash,
  solanaSccpAccountInclusionRootFromBranch,
  solanaSccpAccountInclusionRootAndBranches,
  solanaSccpAccountsLtHashChecksum,
  solanaSccpOpenedAccountInclusionWitness,
  solanaSccpAccountsLtHashPublicInputColumns,
  solanaSccpAccountsLtHashOpenVerifySchemaDescriptor,
  solanaSccpAgaveBankHash,
  solanaSccpBankForkHash,
  solanaSccpVoteAccountDataHash,
  solanaSccpVoteAccountDataFromRawVoteState,
  solanaSccpVoteAccountDataHashFromRawVoteState,
  solanaSccpVoteAccountDataFromRawVoteStateV1OrV3,
  solanaSccpVoteAccountDataHashFromRawVoteStateV1OrV3,
  solanaSccpStakeAccountDataHash,
  solanaSccpStakeAccountDataFromRawStakeStateV2,
  solanaSccpStakeAccountDataHashFromRawStakeStateV2,
  solanaSccpStakeActivationHash,
  solanaSccpStakeAccountStateHash,
  solanaSccpStakeHistorySysvarDataHash,
  solanaSccpStakeHistorySysvarDataHashFromRawData,
  solanaSccpStakeHistoryHash,
  solanaSccpTowerLockoutHash,
  solanaSccpTowerReplayHash,
  substrateAuthoritySetHashFromPayload,
  substrateAuthoritySetPayloadHash,
  substrateAuthoritySetTransitionMessageHash,
  SubstrateSccpProver,
  tonMasterchainBlockMessageHash,
  tonMasterchainConfigLeafHash,
  tonMasterchainConfigProofHash,
  tonMasterchainValidatorSignaturesHash,
  tonConfigValidatorSetPayloadFromProofBoc,
  tonConfigValidatorSetPayloadHashFromProofBoc,
  tonHashmapEProofRootHash,
  tonHashmapECellRefValueHash,
  tonShardAccountsLastTransaction,
  tonShardAccountsLastTransactionHash,
  tonShardStateProofRootHash,
  tonShardStateAccountsRootHash,
  tonShardStateOpenVerifySchemaDescriptor,
  tonShardStateProofPublicInputsHash,
  tonShardStatePublicInputColumns,
  tonBocRootHashes,
  tonBocSingleRootHash,
  tonValidatorSetHashFromPayload,
  tonValidatorSetHash,
  tonValidatorSetPayloadHash,
  tonValidatorSetTransitionMessageHash,
  tronBlockIdFromRawDataHash,
  tronRawBlockHeaderHash,
  tronSccpReceiptStateProofHash,
  tronSolidBlockMessageHash,
  tronSolidBlockHeaderProofHash,
  tronWitnessSealHash,
  tronWitnessScheduleHashFromPayload,
  tronWitnessScheduleTransitionMessageHash,
  tronWitnessScheduleTransitionSealHash,
  tronWitnessSchedulePayloadHash,
} from "../dist/index.js";
import { compileKotodamaProgram as compileDistKotodamaProgram } from "../dist/kotodamaCompiler/index.js";
import { renderCanonicalAccountIdLiteralFromPublicKeyLiteral } from "../src/kotodamaCompiler/accountLiteral.js";
import { compileKotodamaProgram as compileSrcKotodamaProgram } from "../src/kotodamaCompiler/index.js";

function privacyNoritoFrame(schemaByte) {
  const frame = Buffer.alloc(40);
  frame.write("NRT0", 0, "ascii");
  frame.fill(schemaByte, 6, 22);
  return frame;
}

function privacyNoritoFrameWithPayload(schemaByte) {
  const frame = Buffer.concat([
    privacyNoritoFrame(schemaByte),
    Buffer.from([0x00, 0x00, 0xa5, 0x5a, 0x11]),
  ]);
  frame.writeBigUInt64LE(3n, 23);
  Buffer.from([0xb9, 0xd3, 0xa8, 0x0c, 0xcd, 0x5d, 0x13, 0x24]).copy(frame, 31);
  return frame;
}

const TEST_CRC64_MASK = 0xffff_ffff_ffff_ffffn;
const TEST_CRC64_REFLECTED_POLY = 0xc96c_5795_d787_0f42n;
const TEST_CRC64_TABLE = (() => {
  const table = new Array(256);
  for (let index = 0; index < 256; index += 1) {
    let crc = BigInt(index);
    for (let bit = 0; bit < 8; bit += 1) {
      crc =
        (crc & 1n) !== 0n
          ? (crc >> 1n) ^ TEST_CRC64_REFLECTED_POLY
          : crc >> 1n;
    }
    table[index] = crc;
  }
  return table;
})();

function testCrc64(payload) {
  let crc = TEST_CRC64_MASK;
  for (const byte of payload) {
    const index = Number((crc ^ BigInt(byte)) & 0xffn);
    crc = TEST_CRC64_TABLE[index] ^ (crc >> 8n);
  }
  return BigInt.asUintN(64, crc ^ TEST_CRC64_MASK);
}

function privacyNoritoFrameFromPayload(schemaByte, payload) {
  const payloadBuffer = Buffer.from(payload);
  const frame = Buffer.concat([privacyNoritoFrame(schemaByte), payloadBuffer]);
  frame.writeBigUInt64LE(BigInt(payloadBuffer.length), 23);
  frame.writeBigUInt64LE(testCrc64(payloadBuffer), 31);
  return frame;
}

const TEST_NORITO_COMPACT_LEN_FLAG = 0x02;
const KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH = Buffer.from(
  "119f4df38a98ef5848ad0aadb9715779",
  "hex",
);

function privacyNoritoFrameFromSchemaHash(schemaHash, payload, flags = 0) {
  const payloadBuffer = Buffer.from(payload);
  const frame = Buffer.alloc(40);
  frame.write("NRT0", 0, "ascii");
  Buffer.from(schemaHash).copy(frame, 6);
  frame[39] = flags;
  const archive = Buffer.concat([frame, payloadBuffer]);
  archive.writeBigUInt64LE(BigInt(payloadBuffer.length), 23);
  archive.writeBigUInt64LE(testCrc64(payloadBuffer), 31);
  return archive;
}

function kagemushaNoritoLength(value, flags = 0) {
  if ((flags & TEST_NORITO_COMPACT_LEN_FLAG) === 0) {
    const length = Buffer.alloc(8);
    length.writeBigUInt64LE(BigInt(value));
    return length;
  }
  let remaining = BigInt(value);
  const bytes = [];
  while (remaining >= 0x80n) {
    bytes.push(Number((remaining & 0x7fn) | 0x80n));
    remaining >>= 7n;
  }
  bytes.push(Number(remaining));
  return Buffer.from(bytes);
}

function kagemushaNoritoField(payload, flags = TEST_NORITO_COMPACT_LEN_FLAG) {
  const bytes = Buffer.from(payload);
  return Buffer.concat([kagemushaNoritoLength(bytes.length, flags), bytes]);
}

function kagemushaNoritoString(value, flags = TEST_NORITO_COMPACT_LEN_FLAG) {
  const bytes = Buffer.from(value, "utf8");
  return Buffer.concat([kagemushaNoritoLength(bytes.length, flags), bytes]);
}

function kagemushaNoritoByteVec(value) {
  const bytes = Buffer.from(value);
  const length = Buffer.alloc(8);
  length.writeBigUInt64LE(BigInt(bytes.length));
  return Buffer.concat([length, bytes]);
}

function kagemushaZk1Tlv(tag, payload) {
  const payloadBuffer = Buffer.from(payload);
  const length = Buffer.alloc(4);
  length.writeUInt32LE(payloadBuffer.length);
  return Buffer.concat([Buffer.from(tag, "ascii"), length, payloadBuffer]);
}

function kagemushaLineageVerifierKey(circuitId, seed) {
  return Buffer.concat([
    Buffer.from([0x5a, 0x4b, 0x31, 0x00]),
    kagemushaZk1Tlv("IPAK", Buffer.from([8, 0, 0, 0])),
    kagemushaZk1Tlv("CID1", Buffer.from(circuitId, "utf8")),
    kagemushaZk1Tlv("H2VK", Buffer.alloc(32, seed)),
  ]);
}

function kagemushaVerifierKeyCommitment(verifierKey) {
  const backend = Buffer.from(KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND, "utf8");
  const backendLength = Buffer.alloc(8);
  backendLength.writeBigUInt64BE(BigInt(backend.length));
  const verifierKeyLength = Buffer.alloc(8);
  verifierKeyLength.writeBigUInt64BE(BigInt(verifierKey.length));
  return createHash("sha256")
    .update("iroha:zk:v1:vk")
    .update(backendLength)
    .update(backend)
    .update(verifierKeyLength)
    .update(verifierKey)
    .digest();
}

function kagemushaLineageProvingKeyArchive(circuitId, verifierKey, seed) {
  const flags = TEST_NORITO_COMPACT_LEN_FLAG;
  const version = Buffer.alloc(2);
  version.writeUInt16LE(1);
  const payload = Buffer.concat([
    kagemushaNoritoField(version, flags),
    kagemushaNoritoField(kagemushaNoritoString(circuitId, flags), flags),
    kagemushaNoritoField(kagemushaVerifierKeyCommitment(verifierKey), flags),
    kagemushaNoritoField(kagemushaNoritoByteVec(Buffer.alloc(64, seed)), flags),
  ]);
  return privacyNoritoFrameFromSchemaHash(
    KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
    payload,
    flags,
  );
}

function privacyNoritoFrameWithPadding(schemaByte, paddingLength) {
  const frame = Buffer.concat([
    privacyNoritoFrame(schemaByte),
    Buffer.alloc(paddingLength),
    Buffer.from([0xa5, 0x5a, 0x11]),
  ]);
  frame.writeBigUInt64LE(3n, 23);
  Buffer.from([0xb9, 0xd3, 0xa8, 0x0c, 0xcd, 0x5d, 0x13, 0x24]).copy(frame, 31);
  return frame;
}

function privacyNoritoFrameWithSchemaOverride(schemaByte, offset, value) {
  const frame = Buffer.from(privacyNoritoFrameWithPayload(schemaByte));
  frame[offset] = value;
  return frame;
}

function privacyNoritoFrameWithDeclaredPayloadLength(schemaByte, payloadLength) {
  const frame = Buffer.from(privacyNoritoFrameWithPayload(schemaByte));
  frame.writeBigUInt64LE(BigInt(payloadLength), 23);
  return frame;
}

function privacyNoritoFrameWithFlags(schemaByte, flags) {
  const frame = Buffer.from(privacyNoritoFrameWithPayload(schemaByte));
  frame[39] = flags;
  return frame;
}

function slicedPrivacyView(archive, prefix = [0xff, 0x7f, 0x42], suffix = [0x24, 0x13]) {
  const backing = Uint8Array.from([
    ...prefix,
    ...archive,
    ...suffix,
  ]);
  return backing.subarray(prefix.length, prefix.length + archive.length);
}

function malformedPrivacyRequestArchives() {
  const badMagic = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badMagic[0] = 0x00;
  const badVersion = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badVersion[4] = 1;
  const badMinorVersion = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badMinorVersion[5] = 1;
  const badCompression = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badCompression[22] = 1;
  const badDeclaredPayloadLength = privacyNoritoFrameWithDeclaredPayloadLength(0x52, 6n);
  const badOversizedDeclaredPayloadLength = privacyNoritoFrameWithDeclaredPayloadLength(
    0x52,
    0x8000000000000000n,
  );
  const badPadding = Buffer.concat([PRIVACY_REQUEST_ARCHIVE, Buffer.from([0x7f])]);
  const badExcessivePadding = privacyNoritoFrameWithPadding(0x52, 65);
  const badFlags = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badFlags[39] = 0x08;
  const badFieldBitsetFlags = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badFieldBitsetFlags[39] = 0x20;
  const badChecksum = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badChecksum[31] ^= 0x01;
  const badPayload = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badPayload[44] ^= 0x7f;
  return [
    Buffer.from([1]),
    badMagic,
    badVersion,
    badMinorVersion,
    badCompression,
    badDeclaredPayloadLength,
    badOversizedDeclaredPayloadLength,
    badPadding,
    badExcessivePadding,
    badFlags,
    badFieldBitsetFlags,
    badChecksum,
    badPayload,
  ];
}

const PRIVACY_CAPABILITIES_ARCHIVE = privacyNoritoFrameWithPayload(0x50);
const PRIVACY_BUILD_ARCHIVE = privacyNoritoFrameWithPayload(0x42);
const PRIVACY_VERIFY_ARCHIVE = privacyNoritoFrameWithPayload(0x56);
const PRIVACY_REQUEST_ARCHIVE = privacyNoritoFrameWithPayload(0x52);

function malformedPrivacyNativeOutputArchives(schemaByte) {
  const archive = privacyNoritoFrameWithPayload(schemaByte);
  const badMagic = Buffer.from(archive);
  badMagic[0] = 0x00;
  const badVersion = Buffer.from(archive);
  badVersion[4] = 1;
  const badMinorVersion = Buffer.from(archive);
  badMinorVersion[5] = 1;
  const badCompression = Buffer.from(archive);
  badCompression[22] = 1;
  const badDeclaredPayloadLength = privacyNoritoFrameWithDeclaredPayloadLength(
    schemaByte,
    6n,
  );
  const badOversizedDeclaredPayloadLength = privacyNoritoFrameWithDeclaredPayloadLength(
    schemaByte,
    0x8000000000000000n,
  );
  const badPadding = Buffer.concat([archive, Buffer.from([0x7f])]);
  const badExcessivePadding = privacyNoritoFrameWithPadding(schemaByte, 65);
  const badFlags = Buffer.from(archive);
  badFlags[39] = 0x08;
  const badFieldBitsetFlags = Buffer.from(archive);
  badFieldBitsetFlags[39] = 0x20;
  const badChecksum = Buffer.from(archive);
  badChecksum[31] ^= 0x01;
  const badPayload = Buffer.from(archive);
  badPayload[44] ^= 0x7f;
  return [
    Buffer.from([1]),
    badMagic,
    badVersion,
    badMinorVersion,
    badCompression,
    badDeclaredPayloadLength,
    badOversizedDeclaredPayloadLength,
    badPadding,
    badExcessivePadding,
    badFlags,
    badFieldBitsetFlags,
    badChecksum,
    badPayload,
  ];
}

const LEGACY_FULLWIDTH_KANA = /[イロハニホヘトチリヌルヲワカヨタレソツネナラムウノオクヤマケフコエテアサキユメミシヒモセス]/u;
const HALFWIDTH_KANA = /[ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼﾋﾓｾｽ]/u;
const DECLARATIONS_TEXT = readFileSync(new URL("../index.d.ts", import.meta.url), "utf8");
const SCCP_SOURCE_TEXT = readFileSync(new URL("../src/sccp.js", import.meta.url), "utf8");
const INDEX_SOURCE_TEXT = readFileSync(new URL("../src/index.js", import.meta.url), "utf8");
const DIST_SCCP_TEXT = readFileSync(new URL("../dist/sccp.js", import.meta.url), "utf8");
const DIST_INDEX_TEXT = readFileSync(new URL("../dist/index.js", import.meta.url), "utf8");
const PACKAGE_JSON_TEXT = readFileSync(new URL("../package.json", import.meta.url), "utf8");
const sha256Hex = (bytes) =>
  `0x${createHash("sha256").update(Buffer.from(bytes)).digest("hex")}`;

function publicSccpSourceExports() {
  return [...SCCP_SOURCE_TEXT.matchAll(/export\s+(?:const|function|class)\s+([A-Za-z0-9_]+)/gu)]
    .map((match) => match[1])
    .filter((name) => name.startsWith("SCCP_") || /Sccp|sccp/u.test(name));
}

function sccpEntrypointExportNames(text) {
  const match = text.match(/export \{([\s\S]*?)\} from "\.\/sccp\.js";/u);
  assert.notEqual(match, null);
  return new Set([...match[1].matchAll(/\b([A-Za-z_][A-Za-z0-9_]*)\b/gu)].map((item) => item[1]));
}

function declarationExportNames() {
  return new Set(
    [...DECLARATIONS_TEXT.matchAll(/export\s+(?:const|function|class|interface|type)\s+([A-Za-z0-9_]+)/gu)]
      .map((match) => match[1]),
  );
}

function declarationInterface(name) {
  const match = DECLARATIONS_TEXT.match(
    new RegExp(`export interface ${name}(?:\\s+extends\\s+[^{]+)?\\s*\\{[\\s\\S]*?\\n\\}`),
  );
  assert.ok(match, `missing declaration interface ${name}`);
  return match[0];
}

function declarationClass(name) {
  const start = DECLARATIONS_TEXT.indexOf(`export class ${name} {`);
  assert.notEqual(start, -1, `missing declaration class ${name}`);
  const end = DECLARATIONS_TEXT.indexOf("\nexport ", start + 1);
  return end === -1 ? DECLARATIONS_TEXT.slice(start) : DECLARATIONS_TEXT.slice(start, end);
}

function abiWord(value) {
  let remaining = BigInt(value);
  const out = new Uint8Array(32);
  for (let index = out.length - 1; index >= 0; index -= 1) {
    out[index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  return out;
}

const BN254_G2_GENERATOR_WORDS = [
  abiWord(0x1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6edn),
  abiWord(0x198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2n),
  abiWord(0x12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daan),
  abiWord(0x090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975bn),
];

function sampleGroth16ProofBytes() {
  const out = new Uint8Array(384);
  [
    abiWord(1),
    Uint8Array.from({ length: 32 }, () => 0x11),
    abiWord(SCCP_DOMAIN_SORA),
    Uint8Array.from({ length: 32 }, () => 0x33),
    abiWord(1),
    abiWord(2),
    ...BN254_G2_GENERATOR_WORDS,
    abiWord(1),
    abiWord(2),
  ].forEach((word, index) => out.set(word, index * 32));
  return out;
}

function sampleEvmDestinationBinding() {
  return evmSccpDestinationBinding({
    targetDomain: SCCP_DOMAIN_ETH,
    networkId: `0x${"33".repeat(32)}`,
    verifierAddress: `0x${"11".repeat(20)}`,
    bridgeAddress: `0x${"22".repeat(20)}`,
    verifierCodeHash: `0x${"bb".repeat(32)}`,
    verifierKeyHash: `0x${"cc".repeat(32)}`,
  });
}

function sampleTronDestinationBinding() {
  return tronSccpDestinationBinding({
    networkId: `0x${"33".repeat(32)}`,
    verifierAddress: "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
    verifierCodeHash: `0x${"bb".repeat(32)}`,
    verifierKeyHash: `0x${"cc".repeat(32)}`,
  });
}

function sampleSolanaStakeStateV2StakeAccount() {
  const data = new Uint8Array(200);
  const view = new DataView(data.buffer);
  view.setUint32(0, 2, true);
  data.fill(0x81, 12, 44);
  data.fill(0x91, 44, 76);
  data.fill(0xa1, 124, 156);
  view.setBigUint64(156, 1_000n, true);
  view.setBigUint64(164, 2n, true);
  view.setBigUint64(172, 9n, true);
  data.set([0x0a, 0xd7, 0xa3, 0x70, 0x3d, 0x0a, 0xb7, 0x3f], 180);
  view.setBigUint64(188, 123n, true);
  data[196] = 1;
  return data;
}

function sampleSolanaVoteStateAccount() {
  const data = new Uint8Array(3_762);
  const view = new DataView(data.buffer);
  let offset = 0;
  const writeU8 = (value) => {
    data[offset] = value;
    offset += 1;
  };
  const writeU32 = (value) => {
    view.setUint32(offset, value, true);
    offset += 4;
  };
  const writeU64 = (value) => {
    view.setBigUint64(offset, BigInt(value), true);
    offset += 8;
  };
  const writeRepeated = (value, length) => {
    data.fill(value, offset, offset + length);
    offset += length;
  };

  writeU32(2);
  writeRepeated(0x51, 32);
  writeRepeated(0x71, 32);
  writeU8(7);
  writeU64(31n);
  for (let index = 0; index < 31; index += 1) {
    writeU8(0);
    writeU64(11n + BigInt(index));
    writeU32(31 - index);
  }
  writeU8(1);
  writeU64(10n);
  writeU64(2n);
  writeU64(1n);
  writeRepeated(0x60, 32);
  writeU64(3n);
  writeRepeated(0x61, 32);
  return data;
}

function sampleSolanaVoteStateV4Account() {
  const data = new Uint8Array(3_762);
  const view = new DataView(data.buffer);
  let offset = 0;
  const writeU8 = (value) => {
    data[offset] = value;
    offset += 1;
  };
  const writeU16 = (value) => {
    view.setUint16(offset, value, true);
    offset += 2;
  };
  const writeU32 = (value) => {
    view.setUint32(offset, value, true);
    offset += 4;
  };
  const writeU64 = (value) => {
    view.setBigUint64(offset, BigInt(value), true);
    offset += 8;
  };
  const writeRepeated = (value, length) => {
    data.fill(value, offset, offset + length);
    offset += length;
  };

  writeU32(3);
  writeRepeated(0x51, 32);
  writeRepeated(0x71, 32);
  writeRepeated(0x81, 32);
  writeRepeated(0x91, 32);
  writeU16(1_234);
  writeU16(9_876);
  writeU64(456n);
  writeU8(1);
  writeRepeated(0xa5, 48);
  writeU64(31n);
  for (let index = 0; index < 31; index += 1) {
    writeU8(0);
    writeU64(11n + BigInt(index));
    writeU32(31 - index);
  }
  writeU8(1);
  writeU64(10n);
  writeU64(2n);
  writeU64(1n);
  writeRepeated(0x60, 32);
  writeU64(3n);
  writeRepeated(0x61, 32);
  return data;
}

function rlpLengthPrefix(length, shortOffset, longOffset) {
  if (length < 56) return Uint8Array.from([shortOffset + length]);
  const bytes = [];
  let remaining = length;
  while (remaining > 0) {
    bytes.unshift(remaining & 0xff);
    remaining = Math.floor(remaining / 256);
  }
  return Uint8Array.from([longOffset + bytes.length, ...bytes]);
}

function concatBytes(...parts) {
  const out = new Uint8Array(parts.reduce((size, part) => size + part.length, 0));
  let offset = 0;
  for (const part of parts) {
    out.set(part, offset);
    offset += part.length;
  }
  return out;
}

function rlpString(bytes) {
  if (bytes.length === 1 && bytes[0] < 0x80) return bytes;
  return concatBytes(rlpLengthPrefix(bytes.length, 0x80, 0xb7), bytes);
}

function rlpList(fields) {
  const payload = concatBytes(...fields);
  return concatBytes(rlpLengthPrefix(payload.length, 0xc0, 0xf7), payload);
}

function sampleEthExecutionHeaderRlp(receiptsRoot = Uint8Array.from(Array(32).fill(0x15))) {
  return rlpList([
    rlpString(Uint8Array.from(Array(32).fill(0x10))),
    rlpString(Uint8Array.from(Array(32).fill(0x11))),
    rlpString(Uint8Array.from(Array(20).fill(0x12))),
    rlpString(Uint8Array.from(Array(32).fill(0x13))),
    rlpString(Uint8Array.from(Array(32).fill(0x14))),
    rlpString(receiptsRoot),
    rlpString(Uint8Array.from(Array(256).fill(0x00))),
    rlpString(new Uint8Array()),
    rlpString(Uint8Array.from([0x2a])),
    rlpString(Uint8Array.from([0x01, 0xc9, 0xc3, 0x80])),
    rlpString(Uint8Array.from([0x52, 0x08])),
    rlpString(Uint8Array.from([0x65, 0x53, 0xf1, 0x00])),
    rlpString(Uint8Array.from(Buffer.from("iroha-sccp-test"))),
    rlpString(Uint8Array.from(Array(32).fill(0x16))),
    rlpString(Uint8Array.from(Array(8).fill(0x00))),
    rlpString(Uint8Array.from([0x3b, 0x9a, 0xca, 0x00])),
    rlpString(Uint8Array.from(Array(32).fill(0x17))),
    rlpString(new Uint8Array()),
    rlpString(new Uint8Array()),
    rlpString(Uint8Array.from(Array(32).fill(0x18))),
  ]);
}

test("package dist entrypoint imports and emits halfwidth i105 literals", () => {
  const publicKey = Buffer.from(
    "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20",
    "hex",
  );
  const address = AccountAddress.fromAccount({ publicKey });
  const literal = address.toI105(0x02f1);

  assert.match(literal, /^sora/u);
  assert.equal(LEGACY_FULLWIDTH_KANA.test(literal), false);
  assert.equal(HALFWIDTH_KANA.test(literal), true);
});

test("package dist Kotodama compiler rejects AssetDefinitionId checksum mismatches", () => {
  const result = compileDistKotodamaProgram(`
seiyaku BadAssetDefinitionChecksum {
  kotoage fn run() permission(Admin) {
    mint_asset(authority(), asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjN"), 1);
  }
}
`);

  assert.equal(result.artifactBytes.length, 0);
  assert.equal(result.diagnostics.length, 1);
  assert.match(result.diagnostics[0].message, /invalid AssetDefinitionId literal `62Fk4FPcMuLvW5QjDGNF2a4jAmjN`.*checksum/is);
});

test("package dist Kotodama compiler matches src for direct account mint path", () => {
  const account = renderCanonicalAccountIdLiteralFromPublicKeyLiteral(`ed0120${"11".repeat(32)}`);
  const asset = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
  const source = `
seiyaku DirectAccountMint {
  kotoage fn run() permission(Admin) {
    mint_asset(account_id("${account}"), asset_definition("${asset}"), 1);
  }
}
`;

  const srcResult = compileSrcKotodamaProgram(source);
  const distResult = compileDistKotodamaProgram(source);

  assert.deepEqual(srcResult.diagnostics, []);
  assert.deepEqual(distResult.diagnostics, []);
  assert.deepEqual(Buffer.from(distResult.artifactBytes), Buffer.from(srcResult.artifactBytes));
});

test("package SCCP entrypoint and declarations cover public source exports", () => {
  const sourceExports = publicSccpSourceExports();
  const sourceEntrypointExports = sccpEntrypointExportNames(INDEX_SOURCE_TEXT);
  const distEntrypointExports = sccpEntrypointExportNames(DIST_INDEX_TEXT);
  const declarationExports = declarationExportNames();

  assert.deepEqual(
    sourceExports.filter((name) => !sourceEntrypointExports.has(name)),
    [],
  );
  assert.deepEqual(
    sourceExports.filter((name) => !distEntrypointExports.has(name)),
    [],
  );
  assert.deepEqual(
    sourceExports.filter((name) => !declarationExports.has(name)),
    [],
  );
});

test("package dist entrypoint exports Kagemusha recursive spend helpers", () => {
  const declarationExports = declarationExportNames();
  const expected = [
    "KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1",
    "KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1",
    "KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1",
    "KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_BRIDGE_ABI_VERSION",
    "KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1",
    "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT",
    "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT",
    "KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION",
    "KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND",
    "KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1",
    "KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1",
    "KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES",
    "KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES",
    "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
    "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN",
    "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN",
    "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1",
    "preferredKagemushaOfflineSpendMode",
    "preferredKagemushaOfflineSpendModeForCapabilities",
    "canRedeemKagemushaRecursiveSpendWitnessless",
    "requiresKagemushaRecursiveSpendLineageWitnessForRedeem",
    "canAppendKagemushaRecursiveSpendWitnesslessLineage",
    "isKagemushaRecursiveSpendLineageProofCircuitId",
    "isKagemushaRecursiveSpendLineageAppendOutputCircuitId",
    "isSupportedKagemushaRecursiveSpendLineageKeyArtifactOpeningLen",
    "kagemushaRecursiveSpendLineageKeyArtifactsForInit",
    "kagemushaRecursiveSpendLineageKeyArtifactsForAppend",
    "kagemushaRecursiveSpendLineageKeyArtifacts",
    "validateKagemushaRecursiveSpendLineageKeyArtifacts",
    "requiresKagemushaRecursiveSpendLineageKeyArtifactsForInit",
    "requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput",
    "normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId",
    "isSupportedKagemushaRecursiveSpendAppendOutputProofCircuitId",
    "isSupportedKagemushaRecursiveSpendAppendProofTransition",
    "isSupportedKagemushaRecursiveSpendPreviousProofCircuitId",
    "requiresKagemushaRecursiveSpendPreviousLineageVerifierRecordForAppend",
    "preferredKagemushaRecursiveSpendAppendOutputProofCircuitId",
    "canProveKagemushaRecursiveSpendAppendOutputProofCircuitId",
    "canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId",
    "requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend",
    "isKagemushaCompactPaymentTokenNativeAvailable",
    "isKagemushaRecursiveAggregationProofBundleNativeAvailable",
    "isKagemushaRecursiveCompactPaymentTokenNativeAvailable",
    "isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable",
    "isKagemushaRecursiveCompactUnavailable",
    "isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable",
    "isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable",
    "isKagemushaRecursiveSpendNativeAvailable",
    "kagemushaProveVerifiedCompactPaymentTokenWithRecords",
    "kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes",
    "kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes",
    "kagemushaVerifyRecursiveCompactPaymentToken",
    "kagemushaRecursiveSpendCompactPaymentTokenFromBundle",
    "kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection",
    "kagemushaRecursiveSpendInit",
    "kagemushaRecursiveSpendAppend",
    "kagemushaRecursiveSpendTransitionProfileInit",
    "kagemushaRecursiveSpendTransitionProfileAppend",
    "kagemushaRecursiveSpendLineageAppendBoundary",
    "kagemushaRecursiveSpendLineageWitnessFromInitResult",
    "kagemushaRecursiveSpendLineageWitnessAppendResult",
    "kagemushaRecursiveSpendVerify",
    "kagemushaRecursiveSpendRedeem",
  ];

  for (const name of expected) {
    assert.match(DIST_INDEX_TEXT, new RegExp(`\\b${name}\\b`, "u"));
    assert.ok(declarationExports.has(name), `missing declaration export ${name}`);
  }
  assert.equal(KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1, "recursive_compact_v1");
  assert.equal(KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1, "recursive_spend_v1");
  assert.equal(KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1, "checked_prefold_v1");
  assert.equal(KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_BRIDGE_ABI_VERSION, 7);
  assert.equal(KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1, "kagemusha-recursive-compact-v1");
  assert.equal(
    KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT,
    "recursive compact Kagemusha payment-token multi-hop proving requires the append verifier batch",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT,
    "recursive compact Kagemusha multi-hop payment-token proving requires the append verifier batch",
  );
  assert.equal(
    isKagemushaRecursiveCompactUnavailable(
      new Error(KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT),
    ),
    true,
  );
  assert.equal(
    isKagemushaRecursiveCompactUnavailable("recursive compact proof composition unavailable"),
    false,
  );
  assert.equal(KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION, 6);
  assert.equal(KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND, "halo2/ipa");
  assert.equal(
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    "kagemusha-recursive-aggregation-v1",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    "kagemusha-recursive-spend-lineage-v1",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    "kagemusha-recursive-spend-lineage-onehop-v1",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    "kagemusha-recursive-spend-lineage-append-v1",
  );
  assert.equal(KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS, 64);
  assert.equal(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1, 64);
  assert.equal(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1, true);
  assert.equal(KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1, 1);
  assert.equal(KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES, 8 * 1024 * 1024);
  assert.equal(KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES, 128);
  assert.equal(KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES, 64 * 1024 * 1024);
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN,
    "iroha:kagemusha:v1:recursive-spend-transition-profile",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN,
    "iroha:kagemusha:v1:recursive-spend-transition-profile-digest",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN,
    "iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1,
    "iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1,
    "iroha:kagemusha:recursive-spend-lineage-append-boundary:v1",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1,
    "iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1,
    "iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1",
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendAppendProofTransition(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendAppendProofTransition(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendAppendProofTransition(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendAppendProofTransition(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    ),
    false,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendAppendProofTransition(
      "unknown-kagemusha-recursive-spend-circuit",
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    ),
    false,
  );
  assert.equal(
    normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(),
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(
    normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(null),
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(
    normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(""),
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(
    normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    ),
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(
    normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(
      "unknown-kagemusha-recursive-spend-circuit",
    ),
    "unknown-kagemusha-recursive-spend-circuit",
  );
  for (const circuitId of [
    undefined,
    null,
    "",
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
  ]) {
    assert.equal(
      isSupportedKagemushaRecursiveSpendAppendOutputProofCircuitId(circuitId),
      true,
    );
  }
  assert.equal(
    isSupportedKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    ),
    false,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendAppendOutputProofCircuitId(
      "unknown-kagemusha-recursive-spend-circuit",
    ),
    false,
  );
  for (const circuitId of [
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
  ]) {
    assert.equal(isKagemushaRecursiveSpendLineageProofCircuitId(circuitId), true);
  }
  assert.equal(
    isKagemushaRecursiveSpendLineageAppendOutputCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    ),
    false,
  );
  assert.equal(
    isKagemushaRecursiveSpendLineageAppendOutputCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(requiresKagemushaRecursiveSpendLineageKeyArtifactsForInit(), true);
  for (const openingLen of [2, 4, 8, 16, 32, 64, 128]) {
    assert.equal(
      isSupportedKagemushaRecursiveSpendLineageKeyArtifactOpeningLen(openingLen),
      true,
    );
  }
  for (const openingLen of [0, 1, 3, 65, 129, -2, 2.5, Number.NaN, "2", true]) {
    assert.equal(
      isSupportedKagemushaRecursiveSpendLineageKeyArtifactOpeningLen(openingLen),
      false,
    );
  }
  const verifierKey = kagemushaLineageVerifierKey(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    0xe7,
  );
  const provingKey = kagemushaLineageProvingKeyArchive(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    verifierKey,
    0xe8,
  );
  const expectedVerifierKey = Buffer.from(verifierKey);
  const expectedProvingKey = Buffer.from(provingKey);
  const initArtifacts = kagemushaRecursiveSpendLineageKeyArtifactsForInit(
    128,
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
    verifierKey,
    provingKey,
  );
  verifierKey.fill(0);
  provingKey.fill(0);
  assert.equal(
    initArtifacts.proofCircuitId,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(initArtifacts.verifierOpeningLen, 128);
  assert.equal(initArtifacts.lineageVerifierKeyBackend, "halo2/ipa");
  assert.deepEqual(initArtifacts.lineageVerifierKey, expectedVerifierKey);
  assert.deepEqual(initArtifacts.lineageProvingKeyArchive, expectedProvingKey);
  assert.equal(initArtifacts.isInitArtifact, true);
  assert.equal(initArtifacts.isAppendArtifact, false);
  const appendVerifierKey = kagemushaLineageVerifierKey(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    0xa7,
  );
  const appendProvingKey = kagemushaLineageProvingKeyArchive(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    appendVerifierKey,
    0xa8,
  );
  const appendArtifacts = kagemushaRecursiveSpendLineageKeyArtifactsForAppend(
    64,
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
    appendVerifierKey,
    appendProvingKey,
  );
  assert.equal(
    appendArtifacts.proofCircuitId,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(appendArtifacts.isInitArtifact, false);
  assert.equal(appendArtifacts.isAppendArtifact, true);
  const genericArtifacts = kagemushaRecursiveSpendLineageKeyArtifacts(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    2,
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
    appendVerifierKey,
    appendProvingKey,
  );
  assert.equal(genericArtifacts.verifierOpeningLen, 2);
  assert.deepEqual(
    validateKagemushaRecursiveSpendLineageKeyArtifacts(genericArtifacts),
    genericArtifacts,
  );
  const directVerifierKey = kagemushaLineageVerifierKey(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    0x11,
  );
  const directProvingKey = kagemushaLineageProvingKeyArchive(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    directVerifierKey,
    0x12,
  );
  const directArtifacts = validateKagemushaRecursiveSpendLineageKeyArtifacts({
    ...initArtifacts,
    lineageVerifierKey: directVerifierKey,
    lineageProvingKeyArchive: directProvingKey,
  });
  directVerifierKey.fill(0);
  directProvingKey.fill(0);
  assert.deepEqual(
    directArtifacts.lineageVerifierKey,
    kagemushaLineageVerifierKey(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
      0x11,
    ),
  );
  assert.deepEqual(
    directArtifacts.lineageProvingKeyArchive,
    kagemushaLineageProvingKeyArchive(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
      kagemushaLineageVerifierKey(
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        0x11,
      ),
      0x12,
    ),
  );
  const exposedVerifierKey = directArtifacts.lineageVerifierKey;
  const exposedProvingKey = directArtifacts.lineageProvingKeyArchive;
  exposedVerifierKey[0] = 0;
  exposedProvingKey[0] = 0;
  assert.equal(directArtifacts.lineageVerifierKey[0], 0x5a);
  assert.equal(directArtifacts.lineageProvingKeyArchive[0], 0x4e);
  assert.notStrictEqual(
    directArtifacts.lineageVerifierKey,
    directArtifacts.lineageVerifierKey,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        appendVerifierKey,
        expectedProvingKey,
      ),
    /lineage_verifier_key/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        expectedVerifierKey,
        appendProvingKey,
      ),
    /lineage_proving_key_archive/,
  );
  for (const malformed of [
    [null, /lineage_key_artifacts/],
    [{ ...initArtifacts, proofCircuitId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1 }, /proof_circuit_id/],
    [{ ...initArtifacts, proofCircuitId: "unknown-kagemusha-recursive-spend-circuit" }, /proof_circuit_id/],
    [{ ...initArtifacts, verifierOpeningLen: 3 }, /verifier_opening_len/],
    [{ ...initArtifacts, verifierOpeningLen: true }, /verifier_opening_len/],
    [{ ...initArtifacts, lineageVerifierKeyBackend: "halo2/kzg" }, /lineage_verifier_key/],
    [{ ...initArtifacts, lineageVerifierKey: Buffer.alloc(0) }, /lineage_verifier_key/],
    [{ ...initArtifacts, lineageProvingKeyArchive: Buffer.alloc(0) }, /lineage_proving_key_archive/],
    [{ ...initArtifacts, lineageVerifierKey: "not-bytes" }, /lineage_verifier_key/],
    [{ ...initArtifacts, lineageProvingKeyArchive: "not-bytes" }, /lineage_proving_key_archive/],
  ]) {
    assert.throws(
      () => validateKagemushaRecursiveSpendLineageKeyArtifacts(malformed[0]),
      malformed[1],
    );
  }
  assert.equal(
    requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  for (const outputCircuitId of [
    undefined,
    null,
    "",
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    "unknown-kagemusha-recursive-spend-circuit",
  ]) {
    assert.equal(
      requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput(outputCircuitId),
      false,
    );
  }
  assert.equal(
    isSupportedKagemushaRecursiveSpendPreviousProofCircuitId(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendPreviousProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendPreviousProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendPreviousProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendPreviousProofCircuitId(
      "unknown-kagemusha-recursive-spend-circuit",
    ),
    false,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousLineageVerifierRecordForAppend(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    ),
    false,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousLineageVerifierRecordForAppend(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousLineageVerifierRecordForAppend(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousLineageVerifierRecordForAppend(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousLineageVerifierRecordForAppend(
      "unknown-kagemusha-recursive-spend-circuit",
    ),
    false,
  );
  assert.equal(
    canRedeemKagemushaRecursiveSpendWitnessless(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    true,
  );
  assert.equal(
    canRedeemKagemushaRecursiveSpendWitnessless(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    true,
  );
  assert.equal(
    canRedeemKagemushaRecursiveSpendWitnessless(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
      2,
    ),
    true,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendLineageWitnessForRedeem(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    false,
  );
  assert.equal(
    canRedeemKagemushaRecursiveSpendWitnessless(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    false,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendLineageWitnessForRedeem(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      2,
    ),
    false,
  );
  for (const [circuitId, hopCount] of [
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, -1],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, Number.MAX_SAFE_INTEGER],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, Number.NaN],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, Number.POSITIVE_INFINITY],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 1n],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, new Number(1)],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, true],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, "1"],
    [undefined, 1],
    [null, 1],
    ["", 1],
    ["unknown-kagemusha-recursive-spend-circuit", Number.MAX_SAFE_INTEGER],
  ]) {
    assert.equal(canRedeemKagemushaRecursiveSpendWitnessless(circuitId, hopCount), false);
    assert.equal(
      requiresKagemushaRecursiveSpendLineageWitnessForRedeem(circuitId, hopCount),
      true,
    );
  }
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(0), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(1), true);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(63), true);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(64), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(1.5), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(-1), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(Number.MAX_SAFE_INTEGER), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(Number.NaN), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(Number.POSITIVE_INFINITY), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(1n), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(new Number(1)), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(true), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage("1"), false);
  assert.equal(
    preferredKagemushaRecursiveSpendAppendOutputProofCircuitId(1),
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(
    preferredKagemushaRecursiveSpendAppendOutputProofCircuitId(63),
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(
    preferredKagemushaRecursiveSpendAppendOutputProofCircuitId(64),
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    "preferred append selector falls back at the witnessless hop cap",
  );
  assert.equal(
    preferredKagemushaRecursiveSpendAppendOutputProofCircuitId(0),
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    true,
  );
  assert.equal(canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(null, 1), true);
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS - 1,
    ),
    true,
  );
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      0,
    ),
    false,
  );
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
    ),
    false,
  );
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    true,
  );
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    true,
  );
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    false,
  );
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      63,
    ),
    true,
  );
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      64,
    ),
    false,
  );
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      "unknown-kagemusha-recursive-spend-circuit",
      1,
    ),
    false,
  );
  for (const previousHopCount of [
    1.5,
    Number.NaN,
    Number.POSITIVE_INFINITY,
    1n,
    new Number(1),
    true,
    "1",
  ]) {
    assert.equal(
      canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        previousHopCount,
      ),
      false,
    );
  }
  assert.equal(
    canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    true,
  );
  assert.equal(
    canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    true,
  );
  assert.equal(
    canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
      "unknown-kagemusha-recursive-spend-circuit",
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    false,
  );
  assert.equal(
    canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    false,
    "semantic previous proofs cannot select Reserved-lineage output",
  );
  assert.equal(
    canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    true,
  );
  assert.equal(
    canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    true,
  );
  assert.equal(
    canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      "unknown-kagemusha-recursive-spend-circuit",
      1,
    ),
    false,
  );
  assert.equal(
    canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      0,
    ),
    false,
  );
  for (const previousHopCount of [
    Number.NaN,
    1n,
    new Number(1),
  ]) {
    assert.equal(
      canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        previousHopCount,
      ),
      false,
    );
  }
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    true,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    true,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      64,
    ),
    true,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      0,
    ),
    false,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    false,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend("", 1),
    false,
  );
  for (const previousHopCount of [
    1.5,
    Number.NaN,
    Number.POSITIVE_INFINITY,
    1n,
    new Number(1),
    "1",
  ]) {
    assert.equal(
      requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend(
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        previousHopCount,
      ),
      false,
    );
  }
  assert.equal(
    preferredKagemushaOfflineSpendModeForCapabilities(true, true),
    KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1,
  );
  assert.equal(
    preferredKagemushaOfflineSpendModeForCapabilities(false, true),
    KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1,
  );
  assert.equal(
    preferredKagemushaOfflineSpendMode(true),
    KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1,
  );
  assert.equal(
    preferredKagemushaOfflineSpendMode(false),
    KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
  );
  assert.equal(typeof isKagemushaRecursiveCompactPaymentTokenNativeAvailable(), "boolean");
  assert.equal(typeof isKagemushaCompactPaymentTokenNativeAvailable(), "boolean");
  assert.equal(
    typeof isKagemushaRecursiveAggregationProofBundleNativeAvailable(),
    "boolean",
  );
  assert.equal(typeof isKagemushaRecursiveSpendNativeAvailable(), "boolean");
  assert.equal(
    typeof isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable(),
    "boolean",
  );
  assert.equal(
    typeof isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable(),
    "boolean",
  );
  assert.equal(
    typeof isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable(),
    "boolean",
  );
  assert.equal(typeof kagemushaVerifyRecursiveCompactPaymentToken, "function");
  assert.equal(typeof kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection, "function");
  assert.throws(
    () => kagemushaProveVerifiedCompactPaymentTokenWithRecords(
      privacyNoritoFrameWithPayload(0x4d),
    ),
    /Kagemusha compact payment-token prover|unavailable in browser-only crypto builds|Native binding required/,
  );
  assert.throws(
    () =>
      kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
        privacyNoritoFrameWithPayload(0x4e),
        privacyNoritoFrameWithPayload(0x4f),
      ),
    /Kagemusha recursive aggregation proof-bundle prover|unavailable in browser-only crypto builds|Native binding required/,
  );
  assert.throws(
    () =>
      kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        privacyNoritoFrameWithPayload(0x4a),
        privacyNoritoFrameWithPayload(0x4c),
    ),
    /recursive compact Kagemusha payment-token prover|unavailable in browser-only crypto builds|Native binding required/,
  );
  assert.throws(
    () => kagemushaVerifyRecursiveCompactPaymentToken(privacyNoritoFrameWithPayload(0x4b)),
    /recursive compact Kagemusha payment-token verifier|unavailable in browser-only crypto builds|Native binding required/,
  );
  assert.throws(
    () => kagemushaRecursiveSpendCompactPaymentTokenFromBundle(privacyNoritoFrameWithPayload(0x4c)),
    /recursive spend compact Kagemusha payment-token projection|unavailable in browser-only crypto builds|Native binding required/,
  );
  assert.throws(
    () =>
      kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
        privacyNoritoFrameWithPayload(0x4c),
        privacyNoritoFrameWithPayload(0x4d),
      ),
    /recursive spend compact Kagemusha payment-token projection verifier|unavailable in browser-only crypto builds|Native binding required/,
  );
  for (const helper of [
    kagemushaRecursiveSpendInit,
    kagemushaRecursiveSpendAppend,
    kagemushaRecursiveSpendTransitionProfileInit,
    kagemushaRecursiveSpendTransitionProfileAppend,
    kagemushaRecursiveSpendLineageAppendBoundary,
    kagemushaRecursiveSpendLineageWitnessFromInitResult,
    kagemushaRecursiveSpendLineageWitnessAppendResult,
    kagemushaRecursiveSpendVerify,
    kagemushaRecursiveSpendRedeem,
  ]) {
    assert.equal(typeof helper, "function");
  }
});

test("package dist Kagemusha recursive spend availability rejects coerced ABI versions", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  try {
    for (const abiVersion of [
      "6",
      true,
      -1,
      6.5,
      Number.NaN,
      Number.POSITIVE_INFINITY,
      Number.MAX_SAFE_INTEGER + 1,
      0x1_0000_0000,
    ]) {
      globalThis.__IROHA_NATIVE_BINDING__ = {
        connectNoritoBridgeAbiVersion() {
          return abiVersion;
        },
        kagemushaRecursiveSpendInit() {
          return Uint8Array.from([1]);
        },
        kagemushaRecursiveSpendAppend() {
          return Uint8Array.from([2]);
        },
        kagemushaRecursiveSpendLineageWitnessFromInitResult() {
          return Uint8Array.from([3]);
        },
        kagemushaRecursiveSpendLineageWitnessAppendResult() {
          return Uint8Array.from([4]);
        },
        kagemushaRecursiveSpendVerify() {
          return Uint8Array.from([5]);
        },
        kagemushaRecursiveSpendRedeem() {
          return Uint8Array.from([6]);
        },
        kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes() {
          return Uint8Array.from([10]);
        },
        kagemushaVerifyRecursiveCompactPaymentToken() {
          return true;
        },
      };

      assert.equal(isKagemushaRecursiveSpendNativeAvailable(), false);
      assert.equal(isKagemushaRecursiveCompactPaymentTokenNativeAvailable(), false);
      assert.equal(
        isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable(),
        false,
      );
    }
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist entrypoint exports privacy native archive helpers", () => {
  const declarationExports = declarationExportNames();
  const expected = [
    "PRIVACY_FFI_VERSION_V1",
    "PRIVACY_REQUIRED_BRIDGE_ABI_VERSION",
    "PRIVACY_FFI_STATUS_ERROR",
    "PRIVACY_FFI_ERROR_NULL_POINTER",
    "PRIVACY_FFI_ERROR_MALFORMED_NORITO",
    "PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM",
    "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED",
    "PRIVACY_FFI_ERROR_INVALID_REQUEST",
    "isPrivacyNativeAvailable",
    "privacyCapabilitiesV1",
    "privacyBuildProofV1",
    "privacyVerifyProofV1",
    "getPrivacyCapabilities",
  ];

  for (const name of expected) {
    assert.match(DIST_INDEX_TEXT, new RegExp(`\\b${name}\\b`, "u"));
    assert.ok(declarationExports.has(name), `missing declaration export ${name}`);
  }
  assert.equal(PRIVACY_FFI_VERSION_V1, 1);
  assert.equal(PRIVACY_REQUIRED_BRIDGE_ABI_VERSION, 6);
  assert.equal(PRIVACY_FFI_STATUS_ERROR, 1);
  assert.equal(PRIVACY_FFI_ERROR_NULL_POINTER, 1);
  assert.equal(PRIVACY_FFI_ERROR_MALFORMED_NORITO, 2);
  assert.equal(PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM, 3);
  assert.equal(PRIVACY_FFI_ERROR_PRODUCTION_DISABLED, 4);
  assert.equal(PRIVACY_FFI_ERROR_INVALID_REQUEST, 5);
  assert.equal(typeof isPrivacyNativeAvailable(), "boolean");
  for (const helper of [
    privacyCapabilitiesV1,
    privacyBuildProofV1,
    privacyVerifyProofV1,
  ]) {
    assert.equal(typeof helper, "function");
  }
  const capabilities = getPrivacyCapabilities();
  assert.equal(capabilities.javascriptSdkAvailable, true);
  assert.equal(typeof capabilities.bridgeAvailable, "boolean");
  assert.deepEqual(Object.keys(capabilities).sort(), [
    "bridgeAvailable",
    "javascriptSdkAvailable",
    "privacyAlgorithms",
    "privacyCriteria",
  ]);
  assert.equal(
    capabilities.privacyAlgorithms.every((descriptor) => descriptor.productionReady === false),
    true,
  );
  assert.equal(
    capabilities.privacyAlgorithms.every((descriptor) => descriptor.productionGate.ready === false),
    true,
  );
  assert.equal(Object.isFrozen(capabilities), true);
  assert.equal(Object.isFrozen(capabilities.privacyAlgorithms), true);
  assert.equal(Object.isFrozen(capabilities.privacyAlgorithms[0]), true);
  assert.equal(Object.isFrozen(capabilities.privacyAlgorithms[0].productionGate), true);
  assert.equal(Object.isFrozen(capabilities.privacyAlgorithms[0].productionGate.gates), true);
  assert.equal(Object.isFrozen(capabilities.privacyAlgorithms[0].productionGate.missing), true);
  assert.equal(Object.isFrozen(capabilities.privacyCriteria), true);
  assert.throws(() => {
    capabilities.privacyAlgorithms[0].productionReady = true;
  });
  assert.throws(() => {
    capabilities.privacyAlgorithms[0].productionGate.ready = true;
  });
  assert.throws(() => {
    capabilities.privacyAlgorithms[0].productionGate.gates.external_audit = true;
  });
  assert.throws(() => {
    capabilities.privacyAlgorithms[0].productionGate.missing.length = 0;
  });
  assert.throws(() => {
    capabilities.privacyCriteria.push("tampered");
  });
  const fresh = getPrivacyCapabilities();
  assert.equal(fresh.privacyAlgorithms[0].productionReady, false);
  assert.equal(fresh.privacyAlgorithms[0].productionGate.ready, false);
  assert.equal(fresh.privacyAlgorithms[0].productionGate.gates.external_audit, false);
  assert.ok(
    fresh.privacyAlgorithms[0].productionGate.missing.includes(
      "external audit signoff is missing",
    ),
  );
  assert.deepEqual(fresh.privacyCriteria, capabilities.privacyCriteria);
});

test("package dist privacy proof envelopes preserve pending production backend tags", () => {
  const vkHash = Buffer.alloc(32, 0x66);
  const cases = [
    ["halo2-ipa-orchard", "Halo2IpaOrchard"],
    ["halo2/ipa/orchard", "Halo2IpaOrchard"],
    ["orchard", "Halo2IpaOrchard"],
    ["zcash-orchard", "Halo2IpaOrchard"],
    ["groth16-bls12-377", "Groth16Bls12377"],
    ["groth16/bls12-377", "Groth16Bls12377"],
    ["bls12-377", "Groth16Bls12377"],
    ["decaf377", "Groth16Bls12377"],
    ["masp", "Groth16Bls12377"],
    ["penumbra-masp", "Groth16Bls12377"],
    ["halo2/ipa/penumbra", "Groth16Bls12377"],
    ["halo2/ipa/masp", "Groth16Bls12377"],
    ["fcmp-plus-plus-curve-tree", "FcmpPlusPlusCurveTree"],
    ["fcmp++", "FcmpPlusPlusCurveTree"],
    ["monero-fcmp++", "FcmpPlusPlusCurveTree"],
    ["halo2/ipa/monero", "FcmpPlusPlusCurveTree"],
    ["halo2/ipa/curve-tree", "FcmpPlusPlusCurveTree"],
    ["lattice-pcs-sis", "LatticePcsSis"],
    ["jindo-lattice-pcs-zk", "LatticePcsSis"],
    ["jindo-lattice-pcs-zk-v0", "LatticePcsSis"],
    ["stark/fri", "Stark"],
    ["stark/fri/sha256-goldilocks", "Stark"],
    ["stark/fri/poseidon2-goldilocks", "Stark"],
    ["stark/fri/sha256_goldilocks.v1", "Stark"],
    ["miden-stark", "MidenStark"],
    ["stark/fri/miden", "MidenStark"],
    ["aztec-plonkish-private-kernel", "AztecPlonkishPrivateKernel"],
    ["aztec/private-kernel", "AztecPlonkishPrivateKernel"],
    ["pq-masp-stark-fri", "PqMaspStarkFri"],
    ["stark/fri/pq-masp-stark-fri", "PqMaspStarkFri"],
    ["post-quantum-masp", "PqMaspStarkFri"],
    ["anonymous-pgc", "AnonymousPgc"],
    ["anonymous-pgc-k-out-of-n", "AnonymousPgc"],
    ["anonymous-pgc-k-out-of-n-v1", "AnonymousPgc"],
    ["verange", "VeRange"],
    ["verange-transparent-range", "VeRange"],
    ["verange-transparent-range-v1", "VeRange"],
    ["zkat", "ZkAt"],
    ["zkAt policy-private authenticator", "ZkAt"],
    ["zkat-policy-private-auth-v1", "ZkAt"],
    ["recursive-anonymous-admission", "RecursiveAnonymousAdmission"],
    ["recursive-anonymous-admission-v0", "RecursiveAnonymousAdmission"],
    ["zk-ams-recursive-admission-v0", "RecursiveAnonymousAdmission"],
    ["vega-existing-credential-zk", "VegaExistingCredentialZk"],
    ["vega-existing-credential-zk-v0", "VegaExistingCredentialZk"],
    ["silent-threshold-anoncred", "SilentThresholdAnoncred"],
    ["silent-threshold-anoncred-v0", "SilentThresholdAnoncred"],
    ["threshold-anonymous-credentials", "SilentThresholdAnoncred"],
    ["zk-x509", "ZkX509"],
    ["zkvm-x509-identity", "ZkX509"],
    ["zk-x509-onchain-identity-v0", "ZkX509"],
    ["sis-with-hints", "SisWithHints"],
    ["sis-hints-anoncred-pq-v0", "SisWithHints"],
    ["lattice-anonymous-credentials", "SisWithHints"],
  ];

  for (const [backend, expected] of cases) {
    const encoded = buildPrivacyProofEnvelope({
      backend,
      circuitId: `${backend}:dist-pending-production-shape-v0`,
      vkHash,
      publicInputs: Buffer.from([0x01]),
      proofBytes: Buffer.from([0x02]),
      maxProofBytes: 16,
      maxPublicInputBytes: 16,
    });
    const decoded = noritoDecodePrivacyProofEnvelope(encoded);
    assert.equal(decoded.backend, expected);
  }
});

test("package dist privacy proof envelopes reject production metadata claims", () => {
  const base = {
    backend: "stark/fri/sha256-goldilocks",
    circuitId: "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
    vkHash: Buffer.alloc(32, 0x55),
    publicInputs: Buffer.from([1, 2]),
    proofBytes: Buffer.from("proof"),
  };
  for (const payload of [
    { ...base, backend: "unsupported" },
    { ...base, backend: "mock/dev" },
    { ...base, backend: " unsupported" },
    { ...base, backend: "unsupported " },
    { ...base, backend: " stark/fri/sha256-goldilocks" },
    { ...base, backend: "stark/fri/sha256-goldilocks " },
    { ...base, backend: "stark/fri/sha256 goldilocks" },
    { ...base, backend: "stark/fri/sha256+goldilocks" },
    { ...base, backend: "halo2/ipa+mock" },
    { ...base, backend: "stark/fri/dev-fixture" },
    { ...base, backend: "stark/fri/d-e-v-f-i-x-t-u-r-e" },
    { ...base, backend: "stark/fri/sha512-goldilocks" },
    { ...base, backend: "stark/fri/audit-proof-v1" },
    { ...base, backend: "halo2\uFF0Fipa" },
    { ...base, backend: "halo2/\u200Bipa" },
    { ...base, backend: "h\u0430lo2/ipa" },
    { ...base, backend: "stark\uFF0Ffri/sha256-goldilocks" },
    { ...base, backend: "stark/fri/\u200Bsha256-goldilocks" },
    { ...base, backend: "st\u0430rk/fri/sha256-goldilocks" },
    { ...base, backend: "halo2/ipa/orchard/dev-fixture" },
    { ...base, backend: "halo2/ipa/orchard:production-ready" },
    { ...base, backend: "orchard:mainnet-ready" },
    { ...base, backend: "penumbra-masp:external-security-review" },
    { ...base, backend: "jindo-lattice-pcs-zk:release-ready" },
    { ...base, backend: "stark/fri/miden/claimed-production" },
    { ...base, backend: "miden-stark:dev-fixture" },
    { ...base, backend: "anonymous-pgc-k-out-of-n-v1-production" },
    { ...base, backend: "sis-hints-anoncred-pq-v0-devfixture" },
    { ...base, backend: "sis-with-hints:s-e-c-u-r-i-t-y-a-u-d-i-t-e-d" },
    { ...base, backend: "halo2/ipa/orchard:kzg" },
    { ...base, backend: "orchard:universal-srs" },
    { ...base, backend: "penumbra-masp:kzg" },
    { ...base, backend: "jindo-lattice-pcs-zk:trusted-setup" },
    { ...base, backend: "miden-stark:ptau" },
    { ...base, backend: "sis-with-hints:groth16" },
    { ...base, backend: "pq-masp-stark-fri:kzg" },
    { ...base, backend: "groth16/bls12-377/../../prod" },
    { ...base, backend: "post-quantum-masp/audit-claimed" },
    { ...base, production: true },
    { ...base, productionReady: true },
    { ...base, production_ready: true },
    { ...base, productionGate: { ready: true } },
    { ...base, production_gate: { ready: true } },
  ]) {
    assert.throws(
      () => buildPrivacyProofEnvelope(payload),
      /privacyProofEnvelope/,
    );
  }
});

test("package dist privacy dev proof fixtures reject production metadata claims", () => {
  const accountId = AccountAddress.fromAccount({
    publicKey: Buffer.alloc(32, 0x10),
  }).toI105(0x02f1);
  const anonymousPgcReceiverSet = buildAnonymousPgcReceiverSet({
    threshold: 1,
    receivers: [
      {
        accountCommitment: Buffer.alloc(32, 0x21),
        ciphertextCommitment: Buffer.alloc(32, 0x31),
      },
      {
        accountCommitment: Buffer.alloc(32, 0x22),
        ciphertextCommitment: Buffer.alloc(32, 0x32),
      },
    ],
  });
  const devFixtureCases = [
    [
      "zkAt",
      buildZkAtDevProofFixture,
      {
        policyJson: { threshold: 2, roles: ["ops", "risk", "treasury"] },
        policyEpoch: 7,
        policySchema: "boi-hidden-threshold-v1",
        payload: Buffer.from("zkat:transparent-transfer:42"),
        accountId,
        actionClass: "transparent_transfer",
        domainSeparator: "boi:zkat:v1",
        vkHash: Buffer.alloc(32, 0x55),
      },
    ],
    [
      "ZK-AMS",
      buildZkAmsAdmissionDevProofFixture,
      {
        issuerRoot: Buffer.alloc(32, 0x91),
        admissionNullifiers: [Buffer.alloc(32, 0xa1), Buffer.alloc(32, 0xa2)],
        anonymousAccountCommitments: [
          Buffer.alloc(32, 0xb1),
          Buffer.alloc(32, 0xb2),
        ],
        recursiveProof: Buffer.from("zk-ams:recursive-proof:batch-7"),
        domainSeparator: "boi:zk-ams:pilot:v0",
        vkHash: Buffer.alloc(32, 0x66),
      },
    ],
    [
      "Vega",
      buildVegaCredentialDevProofFixture,
      {
        issuerJson: { did: "did:example:issuer:boi", key: "issuer-key-1" },
        predicateJson: { kind: "age_over", attribute: "age", threshold: 18 },
        credentialSchema: "boi-age-credential-v1",
        accountId,
        expirationEpoch: 42,
        domainSeparator: "boi:vega:pilot:v0",
        vkHash: Buffer.alloc(32, 0x77),
      },
    ],
    [
      "Silent Threshold",
      buildSilentThresholdCredentialDevProofFixture,
      {
        issuerSetJson: { threshold: 2, issuers: ["a", "b", "c"] },
        thresholdPolicyJson: { threshold: 2, purpose: "wallet" },
        credentialShowingJson: { credential_type: "wallet", nonce: "n-1" },
        verifierPolicyJson: { verifier: "boi", purpose: "wallet" },
        domainSeparator: "boi:silent-threshold:pilot:v0",
        vkHash: Buffer.alloc(32, 0x88),
      },
    ],
    [
      "ZK-X.509",
      buildZkX509IdentityDevProofFixture,
      {
        caRootJson: { root: "boi-root-ca", version: 1 },
        certificatePolicyJson: { eku: ["clientAuth"], policy: "wallet" },
        revocationJson: { epoch: 7, root: "revocation-root" },
        subjectJson: { cn: "Bank A", lei: "5493001KJTIIGC8Y1R12" },
        accountId,
        domainSeparator: "boi:zk-x509:pilot:v0",
        vkHash: Buffer.alloc(32, 0x99),
      },
    ],
    [
      "Jindo",
      buildJindoLatticeDevProofFixture,
      {
        polynomialJson: { ring: "Rq", degree: 1024, digest: "poly" },
        openingClaimJson: { point: "x=42", value_digest: "value" },
        querySetJson: { queries: [0, 7, 42] },
        parametersJson: { scheme: "jindo-pcs-v0", q_bits: 64 },
        domainSeparator: "boi:jindo:pcs:pilot:v0",
        vkHash: Buffer.alloc(32, 0xaa),
      },
    ],
    [
      "SIS-with-hints",
      buildSisHintsCredentialDevProofFixture,
      {
        issuerJson: { issuer: "boi", scheme: "sis-hints-v0" },
        credentialJson: { credential_type: "wallet", nonce: "n-1" },
        showingPolicyJson: { verifier: "boi", purpose: "wallet" },
        parametersJson: { scheme: "sis-hints-anoncred-v0", q_bits: 64 },
        domainSeparator: "boi:sis-hints:pilot:v0",
        vkHash: Buffer.alloc(32, 0xbb),
      },
    ],
    [
      "Anonymous PGC",
      buildAnonymousPgcDevProofFixture,
      {
        receiverSet: anonymousPgcReceiverSet,
        anonymitySetRoot: Buffer.alloc(32, 0x41),
        payload: Buffer.from("anonymous-pgc:alice:bob:42"),
        balanceCommitments: [Buffer.alloc(32, 0x51), Buffer.alloc(32, 0x52)],
        linkTag: Buffer.alloc(32, 0x61),
        rangeCommitments: [Buffer.alloc(32, 0x71)],
        chainId: "boi-localnet",
        domainSeparator: "boi:anonymous-pgc:v1",
        vkHash: Buffer.alloc(32, 0x55),
      },
    ],
    [
      "VeRange",
      buildVeRangeDevProofFixture,
      {
        commitments: [Buffer.alloc(32, 0x44), Buffer.alloc(32, 0x45)],
        bitLength: 64,
        commitmentScheme: "pedersen-v1",
        domainSeparator: "boi:amount-range:v1",
        payload: Buffer.from("transfer:alice@wonderland:bob@wonderland:42"),
        vkHash: Buffer.alloc(32, 0x55),
      },
    ],
  ];

  for (const [name, builder, input] of devFixtureCases) {
    const fixture = builder(input);
    assert.equal(fixture.production, false, `${name} fixture must stay dev-only`);
    for (const [field, value] of [
      ["production", true],
      ["productionReady", true],
      ["production_ready", true],
      ["productionGate", { ready: true }],
      ["production_gate", { ready: true }],
    ]) {
      assert.throws(
        () => builder({ ...input, [field]: value }),
        new RegExp(field),
        `${name} fixture builder accepted ${field}`,
      );
    }
  }
});

test("package dist privacy native availability rejects coerced ABI versions", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  try {
    for (const abiVersion of ["6", true]) {
      globalThis.__IROHA_NATIVE_BINDING__ = {
        connectNoritoBridgeAbiVersion() {
          return abiVersion;
        },
        privacyCapabilitiesV1() {
          return Uint8Array.from(PRIVACY_CAPABILITIES_ARCHIVE);
        },
        privacyBuildProofV1() {
          return Uint8Array.from(PRIVACY_BUILD_ARCHIVE);
        },
        privacyVerifyProofV1() {
          return Uint8Array.from(PRIVACY_VERIFY_ARCHIVE);
        },
      };

      assert.equal(isPrivacyNativeAvailable(), false);
    }
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist privacy native availability clears request copies after failures", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const completeBinding = (overrides = {}) => ({
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return Uint8Array.from(PRIVACY_CAPABILITIES_ARCHIVE);
    },
    privacyBuildProofV1() {
      return Uint8Array.from(PRIVACY_BUILD_ARCHIVE);
    },
    privacyVerifyProofV1() {
      return Uint8Array.from(PRIVACY_VERIFY_ARCHIVE);
    },
    ...overrides,
  });
  let throwingProbe;
  let badOutputProbe;

  try {
    globalThis.__IROHA_NATIVE_BINDING__ = completeBinding({
      privacyBuildProofV1(request) {
        throwingProbe = request;
        throw new Error("probe failure after request copy");
      },
    });
    assert.equal(isPrivacyNativeAvailable(), false);

    globalThis.__IROHA_NATIVE_BINDING__ = completeBinding({
      privacyVerifyProofV1(request) {
        badOutputProbe = request;
        return Buffer.from([0x56]);
      },
    });
    assert.equal(isPrivacyNativeAvailable(), false);
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }

  assert.deepEqual(Buffer.from(throwingProbe), Buffer.alloc(privacyNoritoFrame(0x52).length));
  assert.deepEqual(Buffer.from(badOutputProbe), Buffer.alloc(privacyNoritoFrame(0x52).length));
});

test("package dist privacy native availability probes reject unsafe raw output", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const completeBinding = (overrides = {}) => ({
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return Uint8Array.from(PRIVACY_CAPABILITIES_ARCHIVE);
    },
    privacyBuildProofV1() {
      return Uint8Array.from(PRIVACY_BUILD_ARCHIVE);
    },
    privacyVerifyProofV1() {
      return Uint8Array.from(PRIVACY_VERIFY_ARCHIVE);
    },
    ...overrides,
  });
  try {
    const overrides = [
      {
        privacyCapabilitiesV1() {
          return "json is not Norito";
        },
      },
      {
        privacyBuildProofV1() {
          return Uint8Array.from([]);
        },
      },
      {
        privacyVerifyProofV1() {
          return undefined;
        },
      },
      {
        privacyBuildProofV1() {
          return [0x42];
        },
      },
      {
        privacyBuildProofV1() {
          return Buffer.from([0x42]);
        },
      },
      {
        privacyCapabilitiesV1() {
          return Buffer.from(PRIVACY_BUILD_ARCHIVE);
        },
      },
      {
        privacyBuildProofV1() {
          return Buffer.from(PRIVACY_VERIFY_ARCHIVE);
        },
      },
      {
        privacyVerifyProofV1() {
          return Buffer.from(PRIVACY_CAPABILITIES_ARCHIVE);
        },
      },
      {
        privacyCapabilitiesV1() {
          const bad = Buffer.from(PRIVACY_CAPABILITIES_ARCHIVE);
          bad[0] = 0x00;
          return bad;
        },
      },
      {
        privacyBuildProofV1() {
          const bad = Buffer.from(PRIVACY_BUILD_ARCHIVE);
          bad[39] = 0x08;
          return bad;
        },
      },
      {
        privacyVerifyProofV1() {
          return Buffer.concat([PRIVACY_VERIFY_ARCHIVE, Buffer.from([0x01])]);
        },
      },
      {
        privacyVerifyProofV1() {
          const bad = Buffer.concat([PRIVACY_VERIFY_ARCHIVE, Buffer.alloc(1)]);
          bad[31] = 0x01;
          return bad;
        },
      },
      {
        privacyVerifyProofV1() {
          throw new Error("native probe failed");
        },
      },
      {
        privacyCapabilitiesV1() {
          return Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f);
        },
      },
      {
        privacyBuildProofV1() {
          return Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f);
        },
      },
      {
        privacyVerifyProofV1() {
          return Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f);
        },
      },
    ];

    for (const archive of malformedPrivacyNativeOutputArchives(0x50)) {
      overrides.push({
        privacyCapabilitiesV1() {
          return Buffer.from(archive);
        },
      });
    }
    for (const archive of malformedPrivacyNativeOutputArchives(0x42)) {
      overrides.push({
        privacyBuildProofV1() {
          return Buffer.from(archive);
        },
      });
    }
    for (const archive of malformedPrivacyNativeOutputArchives(0x56)) {
      overrides.push({
        privacyVerifyProofV1() {
          return Buffer.from(archive);
        },
      });
    }

    for (const override of overrides) {
      globalThis.__IROHA_NATIVE_BINDING__ = completeBinding(override);
      assert.equal(isPrivacyNativeAvailable(), false);
    }
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist privacy native wrappers reject wrong-operation result schemas", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const completeBinding = (overrides = {}) => ({
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return Uint8Array.from(PRIVACY_CAPABILITIES_ARCHIVE);
    },
    privacyBuildProofV1() {
      return Uint8Array.from(PRIVACY_BUILD_ARCHIVE);
    },
    privacyVerifyProofV1() {
      return Uint8Array.from(PRIVACY_VERIFY_ARCHIVE);
    },
    ...overrides,
  });
  try {
    globalThis.__IROHA_NATIVE_BINDING__ = completeBinding({
      privacyCapabilitiesV1() {
        return privacyNoritoFrameWithSchemaOverride(0x50, 21, 0x42);
      },
    });
    assert.equal(isPrivacyNativeAvailable(), false);
    assert.throws(
      () => privacyCapabilitiesV1(),
      /native privacyCapabilitiesV1 returned unexpected privacy result schema/,
    );

    globalThis.__IROHA_NATIVE_BINDING__ = completeBinding({
      privacyBuildProofV1() {
        return privacyNoritoFrameWithSchemaOverride(0x42, 6, 0x56);
      },
    });
    assert.equal(isPrivacyNativeAvailable(), false);
    assert.throws(
      () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
      /native privacyBuildProofV1 returned unexpected privacy result schema/,
    );

    globalThis.__IROHA_NATIVE_BINDING__ = completeBinding({
      privacyVerifyProofV1() {
        return privacyNoritoFrameWithSchemaOverride(0x56, 21, 0x50);
      },
    });
    assert.equal(isPrivacyNativeAvailable(), false);
    assert.throws(
      () => privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE),
      /native privacyVerifyProofV1 returned unexpected privacy result schema/,
    );

    globalThis.__IROHA_NATIVE_BINDING__ = completeBinding({
      privacyBuildProofV1() {
        assert.fail("wrong-schema build request must not reach native dispatch");
      },
      privacyVerifyProofV1() {
        assert.fail("wrong-schema verify request must not reach native dispatch");
      },
    });
    for (const wrongSchemaArchive of [
      PRIVACY_CAPABILITIES_ARCHIVE,
      PRIVACY_BUILD_ARCHIVE,
      PRIVACY_VERIFY_ARCHIVE,
      privacyNoritoFrameWithSchemaOverride(0x52, 6, 0x42),
      privacyNoritoFrameWithSchemaOverride(0x52, 21, 0x56),
    ]) {
      assert.throws(
        () => privacyBuildProofV1(Buffer.from(wrongSchemaArchive)),
        /requestArchive must use the privacy request schema/,
      );
      assert.throws(
        () => privacyVerifyProofV1(Uint8Array.from(wrongSchemaArchive)),
        /requestArchive must use the privacy request schema/,
      );
    }
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist privacy native wrappers reject oversized output archives", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const oversized = Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f);
  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return oversized;
    },
    privacyBuildProofV1() {
      return oversized;
    },
    privacyVerifyProofV1() {
      return oversized;
    },
  };
  try {
    assert.throws(
      () => privacyCapabilitiesV1(),
      /native privacyCapabilitiesV1 returned oversized output/,
    );
    assert.throws(
      () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
      /native privacyBuildProofV1 returned oversized output/,
    );
    assert.throws(
      () => privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE),
      /native privacyVerifyProofV1 returned oversized output/,
    );
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist privacy native wrappers reject invalid Norito-framed output archives", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const badMagic = Buffer.from(PRIVACY_CAPABILITIES_ARCHIVE);
  badMagic[0] = 0x00;
  const badVersion = Buffer.from(PRIVACY_BUILD_ARCHIVE);
  badVersion[4] = 1;
  const badMinorVersion = Buffer.from(PRIVACY_BUILD_ARCHIVE);
  badMinorVersion[5] = 1;
  const badDeclaredPayloadLength = privacyNoritoFrameWithDeclaredPayloadLength(0x42, 6n);
  const badOversizedDeclaredPayloadLength = privacyNoritoFrameWithDeclaredPayloadLength(
    0x42,
    0x8000000000000000n,
  );
  const badPadding = Buffer.concat([PRIVACY_VERIFY_ARCHIVE, Buffer.from([0x7f])]);
  const badFlags = Buffer.from(PRIVACY_BUILD_ARCHIVE);
  badFlags[39] = 0x08;
  const badFieldBitsetFlags = Buffer.from(PRIVACY_BUILD_ARCHIVE);
  badFieldBitsetFlags[39] = 0x20;
  const badChecksum = Buffer.concat([PRIVACY_VERIFY_ARCHIVE, Buffer.alloc(1)]);
  badChecksum[31] = 0x01;
  const badPayload = Buffer.from(privacyNoritoFrameWithPayload(0x57));
  badPayload[44] ^= 0x7f;
  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return badMagic;
    },
    privacyBuildProofV1() {
      return badVersion;
    },
    privacyVerifyProofV1() {
      return badPadding;
    },
  };
  try {
    assert.throws(
      () => privacyCapabilitiesV1(),
      /native privacyCapabilitiesV1 returned invalid Norito V1 archive/,
    );
    assert.throws(
      () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
      /native privacyBuildProofV1 returned invalid Norito V1 archive/,
    );
    assert.throws(
      () => privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE),
      /native privacyVerifyProofV1 returned invalid Norito V1 archive/,
    );
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }

  for (const invalidBuildOutput of [
    badMinorVersion,
    badDeclaredPayloadLength,
    badOversizedDeclaredPayloadLength,
  ]) {
    globalThis.__IROHA_NATIVE_BINDING__ = {
      connectNoritoBridgeAbiVersion() {
        return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
      },
      privacyCapabilitiesV1() {
        return PRIVACY_CAPABILITIES_ARCHIVE;
      },
      privacyBuildProofV1() {
        return invalidBuildOutput;
      },
      privacyVerifyProofV1() {
        return PRIVACY_VERIFY_ARCHIVE;
      },
    };
    try {
      assert.throws(
        () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
        /native privacyBuildProofV1 returned invalid Norito V1 archive/,
      );
    } finally {
      if (previous === undefined) {
        delete globalThis.__IROHA_NATIVE_BINDING__;
      } else {
        globalThis.__IROHA_NATIVE_BINDING__ = previous;
      }
    }
  }

  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return PRIVACY_CAPABILITIES_ARCHIVE;
    },
    privacyBuildProofV1() {
      return badFieldBitsetFlags;
    },
    privacyVerifyProofV1() {
      return PRIVACY_VERIFY_ARCHIVE;
    },
  };
  try {
    assert.throws(
      () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
      /native privacyBuildProofV1 returned invalid Norito V1 archive/,
    );
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }

  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return badPayload;
    },
    privacyBuildProofV1() {
      return badFlags;
    },
    privacyVerifyProofV1() {
      return badChecksum;
    },
  };
  try {
    assert.throws(
      () => privacyCapabilitiesV1(),
      /native privacyCapabilitiesV1 returned invalid Norito V1 archive/,
    );
    assert.throws(
      () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
      /native privacyBuildProofV1 returned invalid Norito V1 archive/,
    );
    assert.throws(
      () => privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE),
      /native privacyVerifyProofV1 returned invalid Norito V1 archive/,
    );
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist privacy native wrappers reject oversized request archives", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const oversized = Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f);
  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return Uint8Array.from(PRIVACY_CAPABILITIES_ARCHIVE);
    },
    privacyBuildProofV1() {
      assert.fail("oversized build request must not reach native dispatch");
    },
    privacyVerifyProofV1() {
      assert.fail("oversized verify request must not reach native dispatch");
    },
  };
  try {
    assert.throws(
      () => privacyBuildProofV1(oversized),
      /requestArchive must not exceed/,
    );
    assert.throws(
      () => privacyVerifyProofV1(oversized),
      /requestArchive must not exceed/,
    );
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist privacy native wrappers reject invalid request archives", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return Uint8Array.from(PRIVACY_CAPABILITIES_ARCHIVE);
    },
    privacyBuildProofV1() {
      assert.fail("invalid build request must not reach native dispatch");
    },
    privacyVerifyProofV1() {
      assert.fail("invalid verify request must not reach native dispatch");
    },
  };
  try {
    for (const malformedArchive of malformedPrivacyRequestArchives()) {
      assert.throws(
        () => privacyBuildProofV1(Buffer.from(malformedArchive)),
        /requestArchive must be a valid Norito V1 archive/,
      );
      assert.throws(
        () => privacyVerifyProofV1(Uint8Array.from(malformedArchive)),
        /requestArchive must be a valid Norito V1 archive/,
      );
    }
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist privacy native wrappers accept complete field-bitset flags", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const requestArchive = privacyNoritoFrameWithFlags(0x52, 0x26);
  const buildArchive = privacyNoritoFrameWithFlags(0x42, 0x26);
  const verifyArchive = privacyNoritoFrameWithFlags(0x56, 0x26);
  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return Uint8Array.from(PRIVACY_CAPABILITIES_ARCHIVE);
    },
    privacyBuildProofV1(request) {
      assert.deepEqual(Buffer.from(request), requestArchive);
      return buildArchive;
    },
    privacyVerifyProofV1(request) {
      assert.deepEqual(Buffer.from(request), requestArchive);
      return verifyArchive;
    },
  };
  try {
    assert.deepEqual(privacyBuildProofV1(requestArchive), buildArchive);
    assert.deepEqual(privacyVerifyProofV1(requestArchive), verifyArchive);
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist privacy native wrappers sanitize native exceptions", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const witness = Buffer.from("dist-private-witness-never-echo-21f0", "utf8");
  const requestArchive = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  const capturedRequests = [];
  const throwLeakingNativeError = (request) => {
    if (request !== undefined) {
      capturedRequests.push(request);
      assert.notEqual(request, requestArchive);
      assert.deepEqual(Buffer.from(request), PRIVACY_REQUEST_ARCHIVE);
    }
    throw new Error(`native panic included ${witness.toString("utf8")}`);
  };
  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1: throwLeakingNativeError,
    privacyBuildProofV1: throwLeakingNativeError,
    privacyVerifyProofV1: throwLeakingNativeError,
  };
  try {
    for (const [operation, invoke] of [
      ["privacyCapabilitiesV1", () => privacyCapabilitiesV1()],
      ["privacyBuildProofV1", () => privacyBuildProofV1(requestArchive)],
      ["privacyVerifyProofV1", () => privacyVerifyProofV1(requestArchive)],
    ]) {
      let error;
      try {
        invoke();
      } catch (caught) {
        error = caught;
      }
      assert.ok(error, `${operation} should throw`);
      assert.match(error.message, new RegExp(`native ${operation} failed`, "u"));
      assert.equal(error.cause, undefined);
      assert.equal(String(error).includes(witness.toString("utf8")), false);
      assert.equal(String(error.stack).includes(witness.toString("utf8")), false);
    }
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }

  assert.equal(capturedRequests.length, 2);
  for (const request of capturedRequests) {
    assert.equal(request.every((value) => value === 0), true);
  }
  assert.deepEqual(requestArchive, Buffer.from(PRIVACY_REQUEST_ARCHIVE));
});

test("package dist privacy native wrappers clear temporary request copies", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const requestArchive = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  const originalArchive = Buffer.from(requestArchive);
  let buildRequest;
  let verifyRequest;
  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return Uint8Array.from(PRIVACY_CAPABILITIES_ARCHIVE);
    },
    privacyBuildProofV1(request) {
      buildRequest = request;
      assert.notEqual(request, requestArchive);
      assert.deepEqual(Buffer.from(request), originalArchive);
      return Uint8Array.from(PRIVACY_BUILD_ARCHIVE);
    },
    privacyVerifyProofV1(request) {
      verifyRequest = request;
      assert.notEqual(request, requestArchive);
      assert.deepEqual(Buffer.from(request), originalArchive);
      return Uint8Array.from(PRIVACY_VERIFY_ARCHIVE);
    },
  };
  try {
    assert.deepEqual(privacyBuildProofV1(requestArchive), PRIVACY_BUILD_ARCHIVE);
    assert.deepEqual(privacyVerifyProofV1(requestArchive), PRIVACY_VERIFY_ARCHIVE);
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }

  assert.ok(buildRequest, "build request should be captured");
  assert.ok(verifyRequest, "verify request should be captured");
  assert.equal(buildRequest.every((value) => value === 0), true);
  assert.equal(verifyRequest.every((value) => value === 0), true);
  assert.deepEqual(requestArchive, originalArchive);
});

test("package dist privacy native wrappers respect sliced request archive views", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const buildView = slicedPrivacyView(PRIVACY_REQUEST_ARCHIVE);
  const verifyBacking = Uint8Array.from([
    0x99,
    0x88,
    ...PRIVACY_REQUEST_ARCHIVE,
    0x77,
  ]);
  const verifyView = new DataView(
    verifyBacking.buffer,
    2,
    PRIVACY_REQUEST_ARCHIVE.length,
  );
  let buildRequest;
  let verifyRequest;
  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return Uint8Array.from(PRIVACY_CAPABILITIES_ARCHIVE);
    },
    privacyBuildProofV1(request) {
      buildRequest = request;
      assert.deepEqual(Buffer.from(request), PRIVACY_REQUEST_ARCHIVE);
      return slicedPrivacyView(PRIVACY_BUILD_ARCHIVE);
    },
    privacyVerifyProofV1(request) {
      verifyRequest = request;
      assert.deepEqual(Buffer.from(request), PRIVACY_REQUEST_ARCHIVE);
      return new DataView(
        slicedPrivacyView(PRIVACY_VERIFY_ARCHIVE).buffer,
        3,
        PRIVACY_VERIFY_ARCHIVE.length,
      );
    },
  };
  try {
    assert.deepEqual(privacyBuildProofV1(buildView), PRIVACY_BUILD_ARCHIVE);
    assert.deepEqual(privacyVerifyProofV1(verifyView), PRIVACY_VERIFY_ARCHIVE);
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }

  assert.deepEqual(Buffer.from(buildView), PRIVACY_REQUEST_ARCHIVE);
  assert.deepEqual(
    Buffer.from(verifyBacking.subarray(2, 2 + PRIVACY_REQUEST_ARCHIVE.length)),
    PRIVACY_REQUEST_ARCHIVE,
  );
  assert.equal(buildRequest.every((value) => value === 0), true);
  assert.equal(verifyRequest.every((value) => value === 0), true);
});

test("package dist privacy native wrappers respect sliced native output archive views", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const prefixLength = 3;
  const capabilitiesBacking = Uint8Array.from([
    0xff,
    0x7f,
    0x50,
    ...PRIVACY_CAPABILITIES_ARCHIVE,
    0x24,
  ]);
  const buildBacking = Uint8Array.from([
    0xff,
    0x7f,
    0x42,
    ...PRIVACY_BUILD_ARCHIVE,
    0x13,
  ]);
  const verifyBacking = Uint8Array.from([
    0xff,
    0x7f,
    0x56,
    ...PRIVACY_VERIFY_ARCHIVE,
    0x37,
  ]);

  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return capabilitiesBacking.subarray(
        prefixLength,
        prefixLength + PRIVACY_CAPABILITIES_ARCHIVE.length,
      );
    },
    privacyBuildProofV1() {
      return new DataView(
        buildBacking.buffer,
        prefixLength,
        PRIVACY_BUILD_ARCHIVE.length,
      );
    },
    privacyVerifyProofV1() {
      return verifyBacking.subarray(
        prefixLength,
        prefixLength + PRIVACY_VERIFY_ARCHIVE.length,
      );
    },
  };
  try {
    const capabilitiesArchive = privacyCapabilitiesV1();
    const buildArchive = privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE);
    const verifyArchive = privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE);

    assert.deepEqual(capabilitiesArchive, PRIVACY_CAPABILITIES_ARCHIVE);
    assert.deepEqual(buildArchive, PRIVACY_BUILD_ARCHIVE);
    assert.deepEqual(verifyArchive, PRIVACY_VERIFY_ARCHIVE);

    capabilitiesBacking[prefixLength] = 0x00;
    buildBacking[prefixLength] = 0x00;
    verifyBacking[prefixLength] = 0x00;

    assert.deepEqual(capabilitiesArchive, PRIVACY_CAPABILITIES_ARCHIVE);
    assert.deepEqual(buildArchive, PRIVACY_BUILD_ARCHIVE);
    assert.deepEqual(verifyArchive, PRIVACY_VERIFY_ARCHIVE);
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist privacy native wrappers defensively copy native output archives", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const capabilitiesOutput = Buffer.from(PRIVACY_CAPABILITIES_ARCHIVE);
  const buildOutput = Buffer.from(PRIVACY_BUILD_ARCHIVE);
  const verifyBacking = Uint8Array.from(
    Buffer.concat([Buffer.from([0x00]), PRIVACY_VERIFY_ARCHIVE, Buffer.from([0x00])]),
  );
  const verifyOutput = verifyBacking.subarray(1, 1 + PRIVACY_VERIFY_ARCHIVE.length);
  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return capabilitiesOutput;
    },
    privacyBuildProofV1() {
      return buildOutput;
    },
    privacyVerifyProofV1() {
      return verifyOutput;
    },
  };
  try {
    const capabilitiesArchive = privacyCapabilitiesV1();
    assert.notEqual(capabilitiesArchive, capabilitiesOutput);
    assert.deepEqual(capabilitiesArchive, PRIVACY_CAPABILITIES_ARCHIVE);
    capabilitiesArchive[0] = 0x7f;
    assert.deepEqual(capabilitiesOutput, PRIVACY_CAPABILITIES_ARCHIVE);

    const buildArchive = privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE);
    assert.notEqual(buildArchive, buildOutput);
    assert.deepEqual(buildArchive, PRIVACY_BUILD_ARCHIVE);
    buildArchive[0] = 0x7f;
    assert.deepEqual(buildOutput, PRIVACY_BUILD_ARCHIVE);

    const verifyArchive = privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE);
    assert.deepEqual(verifyArchive, PRIVACY_VERIFY_ARCHIVE);
    verifyBacking[1] = 0x7f;
    assert.deepEqual(verifyArchive, PRIVACY_VERIFY_ARCHIVE);
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package declarations mark privacy capability metadata readonly", () => {
  const pqLayers = declarationInterface("PrivacyPqLayers");
  assert.match(pqLayers, /readonly proof: boolean;/);
  assert.match(pqLayers, /readonly authorization: boolean;/);
  assert.match(pqLayers, /readonly noteEncryption: boolean;/);

  const productionGate = declarationInterface("PrivacyProductionGate");
  assert.match(productionGate, /readonly ready: boolean;/);
  assert.match(productionGate, /readonly gates: Readonly<Record<string, boolean>>;/);
  assert.match(productionGate, /readonly missing: readonly string\[\];/);
  assert.match(
    productionGate,
    /readonly auditReferences: readonly Readonly<\{ label: string; url: string \}>\[\];/,
  );

  const descriptor = declarationInterface("PrivacyAlgorithmDescriptor");
  assert.match(descriptor, /readonly coveredCriteria: readonly PrivacyCriterionKey\[\];/);
  assert.match(descriptor, /readonly backendFamily: string;/);
  assert.match(descriptor, /readonly pqLayers: PrivacyPqLayers;/);
  assert.match(descriptor, /readonly sdkEntrypoints: readonly string\[\];/);
  assert.match(descriptor, /readonly chainRequirements: readonly string\[\];/);
  assert.match(descriptor, /readonly productionReady: boolean;/);
  assert.match(descriptor, /readonly productionGate: PrivacyProductionGate;/);

  const capabilities = declarationInterface("PrivacyCapabilities");
  assert.match(capabilities, /readonly javascriptSdkAvailable: boolean;/);
  assert.match(capabilities, /readonly bridgeAvailable: boolean;/);
  assert.match(
    capabilities,
    /readonly privacyAlgorithms: readonly PrivacyAlgorithmDescriptor\[\];/,
  );
  assert.match(capabilities, /readonly privacyCriteria: readonly PrivacyCriterionKey\[\];/);

  assert.match(
    DECLARATIONS_TEXT,
    /export function getPrivacyCriteria\(\): readonly PrivacyCriterionKey\[\];/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export function getPrivacyAlgorithmDescriptors\(\): readonly PrivacyAlgorithmDescriptor\[\];/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export const PRIVACY_NATIVE_ARCHIVE_MAX_BYTES: number;/,
  );
  for (const [name, value] of [
    ["PRIVACY_FFI_STATUS_ERROR", 1],
    ["PRIVACY_FFI_ERROR_NULL_POINTER", 1],
    ["PRIVACY_FFI_ERROR_MALFORMED_NORITO", 2],
    ["PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM", 3],
    ["PRIVACY_FFI_ERROR_PRODUCTION_DISABLED", 4],
    ["PRIVACY_FFI_ERROR_INVALID_REQUEST", 5],
  ]) {
    assert.match(
      DECLARATIONS_TEXT,
      new RegExp(`export const ${name}: ${value};`),
    );
  }
});

test("package declarations mark Kagemusha lineage key artifacts readonly", () => {
  const artifacts = declarationInterface(
    "KagemushaRecursiveSpendLineageKeyArtifacts",
  );
  assert.match(artifacts, /readonly proofCircuitId:/);
  assert.match(
    artifacts,
    /readonly verifierOpeningLen: KagemushaRecursiveSpendLineageKeyArtifactOpeningLen;/,
  );
  assert.match(
    artifacts,
    /readonly lineageVerifierKeyBackend: typeof KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND;/,
  );
  assert.match(artifacts, /readonly lineageVerifierKey: Buffer;/);
  assert.match(artifacts, /readonly lineageProvingKeyArchive: Buffer;/);
  assert.match(artifacts, /readonly isInitArtifact: boolean;/);
  assert.match(artifacts, /readonly isAppendArtifact: boolean;/);
});

test("package declarations do not advertise privacy production metadata inputs", () => {
  for (const name of [
    "PrivacyProofEnvelopeInput",
    "ZkAtAuthenticatorEnvelopeInput",
    "ZkAmsAdmissionProofEnvelopeInput",
    "VegaCredentialProofEnvelopeInput",
    "SilentThresholdCredentialEnvelopeInput",
    "ZkX509IdentityEnvelopeInput",
    "JindoLatticeProofEnvelopeInput",
    "SisHintsCredentialEnvelopeInput",
    "AnonymousPgcDevProofFixtureInput",
    "VeRangeProofEnvelopeInput",
    "VeRangeDevProofFixtureInput",
  ]) {
    const declaration = declarationInterface(name);
    assert.doesNotMatch(declaration, /\bproduction\b/u, `${name} exposes production`);
    assert.doesNotMatch(
      declaration,
      /\bproductionReady\b/u,
      `${name} exposes productionReady`,
    );
    assert.doesNotMatch(
      declaration,
      /\bproductionGate\b/u,
      `${name} exposes productionGate`,
    );
  }
});

test("package declarations mark SCCP FastPQ proof requests readonly", () => {
  assert.match(
    DECLARATIONS_TEXT,
    /export interface SolanaSccpAccountsLtHashProofRequest[\s\S]*readonly publicInputColumns: ReadonlyArray<ReadonlyArray<string>>;[\s\S]*readonly fastpqTransitions: ReadonlyArray<[\s\S]*Readonly<SolanaSccpAccountsLtHashFastpqTransition>[\s\S]*>;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export function buildSolanaSccpFullLightClientAuditProofRequests\([\s\S]*\): Readonly<\{/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface SolanaSccpFullLightClientAuditProofRequestBaseInput[\s\S]*sourceTrustAnchorHash\?: string;[\s\S]*sourceAdapterDeploymentHash\?: string;[\s\S]*sourceAdapterDeploymentReceiptHash\?: string;[\s\S]*fullLightClientGateHash\?: string;[\s\S]*accountsLtHashProofHash\?: string;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type SolanaSccpFullLightClientAuditProofRequestInput =[\s\S]*accountsLtHashProof: SolanaSccpSourceStateVerificationProof;[\s\S]*accounts_lt_hash_proof\?: never;[\s\S]*accountsLtHashProof\?: never;[\s\S]*accounts_lt_hash_proof: SolanaSccpSourceStateVerificationProof;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface WrappedSolanaSccpSourceStateVerificationProof[\s\S]*readonly proofBase64: string;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type SccpSourceStateProofBytesResultMetadata[\s\S]*proofBytes: BinaryLike \| number\[\];[\s\S]*proof_bytes: BinaryLike \| number\[\];[\s\S]*proof: BinaryLike \| number\[\];[\s\S]*export type SccpSourceStateProofCapsuleMetadata[\s\S]*SccpSourceStateResultAlias<"circuitId", "circuit_id", CircuitId>[\s\S]*export type SolanaSccpSourceStateVerificationProof =[\s\S]*SccpSourceStateProofCapsuleMetadata/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type SccpSourceStateResultAlias<[\s\S]*\{ \[Key in Snake\]\?: never \}[\s\S]*export type SccpSourceStateFastpqPublicInputsResultMetadata = \{[\s\S]*slot: string \| number \| bigint;[\s\S]*SccpSourceStateResultAlias<"txSetHash", "tx_set_hash", string>;[\s\S]*export type SccpSourceStateFastpqTransitionResultMetadata = \{[\s\S]*SccpSourceStateResultAlias<"oldValue", "old_value", BinaryLike>[\s\S]*SccpSourceStateResultAlias<"newValue", "new_value", BinaryLike>;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type SolanaSccpSourceStateProverResultObject =[\s\S]*SccpSourceStateProverResultProofMetadata[\s\S]*sourceStateVerifierId\?: string;[\s\S]*sourceStateVerifierHash\?: string;[\s\S]*publicInputColumns\?: ReadonlyArray<ReadonlyArray<string>>;[\s\S]*fastpqPublicInputs\?: SccpSourceStateFastpqPublicInputsResultMetadata;[\s\S]*fastpqTransitions\?: ReadonlyArray<SccpSourceStateFastpqTransitionResultMetadata>;[\s\S]*statementBytes\?: BinaryLike \| number\[\];[\s\S]*accountCommitmentBytes\?: BinaryLike \| number\[\];[\s\S]*verificationContextBytes\?: BinaryLike \| number\[\];[\s\S]*schemaDescriptor\?: BinaryLike \| number\[\];/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export function wrapSolanaSccpSourceStateVerificationProof\([\s\S]*\): WrappedSolanaSccpSourceStateVerificationProof;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface TonShardStateProofRequest[\s\S]*readonly publicInputColumns: ReadonlyArray<ReadonlyArray<string>>;[\s\S]*readonly fastpqTransitions: ReadonlyArray<[\s\S]*Readonly<TonShardStateFastpqTransition>[\s\S]*>;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface TonSccpFullLightClientAuditProofRequestBaseInput[\s\S]*tonMasterchainConfigVerifierHash\?: string;[\s\S]*tonShardAccountsDictionaryVerifierHash\?: string;[\s\S]*shardStateVerificationProofHash\?: string;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type TonSccpFullLightClientAuditProofRequestInput =[\s\S]*shardStateVerificationProof: TonSccpSourceStateVerificationProof;[\s\S]*shard_state_verification_proof\?: never;[\s\S]*shardStateVerificationProof\?: never;[\s\S]*shard_state_verification_proof: TonSccpSourceStateVerificationProof;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type TonSccpSourceStateProverResultObject =[\s\S]*SccpSourceStateProverResultProofMetadata[\s\S]*masterchainSeqno\?: string \| number \| bigint;[\s\S]*shardSeqno\?: string \| number \| bigint;[\s\S]*sourceStateVerifierId\?: string;[\s\S]*shardStateProofPublicInputsHash\?: string;[\s\S]*shardStateVerificationProofHash\?: string;[\s\S]*role\?: TonSccpFullLightClientAuditRole;[\s\S]*fastpqPublicInputs\?: SccpSourceStateFastpqPublicInputsResultMetadata;[\s\S]*fastpqTransitions\?: ReadonlyArray<SccpSourceStateFastpqTransitionResultMetadata>;[\s\S]*statementBytes\?: BinaryLike \| number\[\];[\s\S]*witnessCommitmentBytes\?: BinaryLike \| number\[\];[\s\S]*verificationContextBytes\?: BinaryLike \| number\[\];[\s\S]*schemaDescriptor\?: BinaryLike \| number\[\];/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export function buildTonSccpFullLightClientAuditProofRequests\([\s\S]*\): Readonly<\{/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface SubstrateSccpRuntimeStorageProofRequest[\s\S]*readonly publicInputColumns: ReadonlyArray<ReadonlyArray<string>>;[\s\S]*readonly fastpqTransitions: ReadonlyArray<[\s\S]*Readonly<SubstrateSccpRuntimeStorageFastpqTransition>[\s\S]*>;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface EvmSccpBridgeProofSubmitPayloadInput[\s\S]*submission\?: EvmSccpSubmission;[\s\S]*destinationBinding\?: EvmSccpDestinationBindingInput;[\s\S]*export function buildEvmSccpBridgeProofSubmitPayload\([\s\S]*\): ToriiBridgeProofSubmitPayload;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface TronSccpBridgeProofSubmitPayloadInput[\s\S]*submission\?: TronSccpSubmission;[\s\S]*destinationBinding\?: TronSccpDestinationBindingInput;[\s\S]*export function buildTronSccpBridgeProofSubmitPayload\([\s\S]*\): ToriiBridgeProofSubmitPayload;/,
  );
});

test("package declarations expose SCCP proof-result request bytes", () => {
  for (const resultType of [
    "TonSccpProofResult",
    "EvmSccpProofResult",
    "TronSccpProofResult",
    "SubstrateSccpProofResult",
  ]) {
    assert.match(
      DECLARATIONS_TEXT,
      new RegExp(
        `export interface ${resultType}[\\s\\S]*bundleBytes: Uint8Array;[\\s\\S]*sourceProofBytes: Uint8Array;`,
      ),
      `${resultType} must expose the retained request bytes`,
    );
  }
  for (const inputType of ["EvmSccpSubmissionInput", "TronSccpSubmissionInput"]) {
    assert.match(
      DECLARATIONS_TEXT,
      new RegExp(
        `export interface ${inputType}[\\s\\S]*bundleBytes\\?: BinaryLike;[\\s\\S]*sourceProofBytes\\?: BinaryLike;`,
      ),
      `${inputType} must accept explicit request bytes for proof-result replay checks`,
    );
  }
});

test("package declarations keep EVM and TRON proof envelopes readonly", () => {
  for (const typeName of [
    "EvmSccpProofRequest",
    "EvmSccpProofResult",
    "TronSccpProofRequest",
    "TronSccpProofResult",
  ]) {
    const declaration = declarationInterface(typeName);
    assert.match(declaration, /readonly version: 1;/);
    assert.match(declaration, /readonly publicInputs: /);
    assert.match(declaration, /readonly publicSignalWords: readonly string\[\];/);
    assert.match(declaration, /readonly bundleBytes: Uint8Array;/);
    assert.match(declaration, /readonly sourceProofBytes: Uint8Array;/);
    assert.match(declaration, /readonly proofContext: Readonly<SolanaSccpProofContext>;/);
  }
});

test("package declarations expose SCCP local-prover result metadata", () => {
  for (const [resultType, proveFnType] of [
    ["TonSccpProveResult", "TonSccpProveFn"],
    ["EvmSccpProveResult", "EvmSccpProveFn"],
    ["TronSccpProveResult", "TronSccpProveFn"],
    ["SubstrateSccpProveResult", "SubstrateSccpProveFn"],
    ["SolanaSccpProveResult", "SolanaSccpProveFn"],
  ]) {
    assert.match(
      DECLARATIONS_TEXT,
      new RegExp(`export interface ${resultType}[\\s\\S]*proofBytes\\?: BinaryLike;`),
      `${resultType} must expose callback proof bytes`,
    );
    assert.match(
      DECLARATIONS_TEXT,
      new RegExp(
        `export type ${proveFnType} = \\([\\s\\S]*\\) => ${resultType} \\| Promise<${resultType}>;`,
      ),
      `${proveFnType} must return the named metadata result type`,
    );
  }

  for (const resultType of [
    "EvmSccpProveResult",
    "TronSccpProveResult",
    "SubstrateSccpProveResult",
  ]) {
    assert.match(
      DECLARATIONS_TEXT,
      new RegExp(
        `export interface ${resultType}[\\s\\S]*proofBase64\\?: string;[\\s\\S]*proof_base64\\?: string;`,
      ),
      `${resultType} must expose optional callback proof base64 metadata`,
    );
  }

  assert.match(
    DECLARATIONS_TEXT,
    /export interface EvmSccpProveResult[\s\S]*publicInputs\?: SccpMessageTransparentPublicInputsInput;[\s\S]*proofContext\?: SolanaSccpProofContextInput;[\s\S]*publicSignalWords\?: readonly string\[\];/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface EvmSccpProofRequestInput[\s\S]*proofArtifactHash\?: string;[\s\S]*provingKeyHash\?: string;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface EvmSccpProveResult[\s\S]*proofArtifactHash\?: string;[\s\S]*provingKeyHash\?: string;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface TronSccpProveResult[\s\S]*publicInputs\?: SccpMessageTransparentPublicInputsInput;[\s\S]*proofContext\?: SolanaSccpProofContextInput;[\s\S]*publicSignalWords\?: readonly string\[\];/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface SolanaSccpProofPublicInputs[\s\S]*readonly sourceStateVerifierId: string;[\s\S]*readonly sourceStateVerifierHash: string;[\s\S]*readonly sourceAdapterDeploymentBindingHash: string;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type SolanaSccpProofPublicInputsInput =[\s\S]*SolanaSccpProofPublicInputs[\s\S]*SolanaSccpProofPublicInputsSnakeCase;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface SolanaSccpProveResult[\s\S]*proofBase64\?: string;[\s\S]*proof_base64\?: string;[\s\S]*publicInputs\?: SolanaSccpProofPublicInputsInput;[\s\S]*sourceStateVerifierId\?: string;[\s\S]*sourceStateVerifierHash\?: string;[\s\S]*proofContext\?: SolanaSccpProofContextInput;[\s\S]*sourceAdapterDeploymentBinding\?: SccpSourceAdapterDeploymentBindingInput;[\s\S]*proofContextHash\?: string;[\s\S]*sourceAdapterDeploymentBindingHash\?: string;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface TonSccpProveResult[\s\S]*requestHash\?: string;[\s\S]*sourceAdapterDeploymentBindingHash\?: string;[\s\S]*envelopeHash\?: string;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface SubstrateSccpProveResult[\s\S]*requestHash\?: string;[\s\S]*envelopeHash\?: string;/,
  );
});

test("package declarations expose SCCP witness-provider hooks for portal provers", () => {
  assert.match(
    DECLARATIONS_TEXT,
    /export type SccpWitnessProviderFn<Input> = \([\s\S]*\) => Input \| Promise<Input>;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type SccpWitnessProviderResolverOption<Input> =[\s\S]*resolveWitness: SccpWitnessProviderFn<Input>;[\s\S]*resolve_witness\?: never;[\s\S]*resolveWitness\?: never;[\s\S]*resolve_witness: SccpWitnessProviderFn<Input>;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type SccpProverWitnessProviderOption<Provider, Input> =[\s\S]*witnessProvider\?: Provider \| SccpWitnessProviderFn<Input>;[\s\S]*witness_provider\?: never;[\s\S]*witnessProvider\?: never;[\s\S]*witness_provider\?: Provider \| SccpWitnessProviderFn<Input>;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type SccpProverProveOption<ProveFn> =[\s\S]*prove\?: ProveFn;[\s\S]*proveFn\?: never;[\s\S]*prove_fn\?: never;[\s\S]*prove\?: never;[\s\S]*proveFn\?: ProveFn;[\s\S]*prove_fn\?: never;[\s\S]*prove\?: never;[\s\S]*proveFn\?: never;[\s\S]*prove_fn\?: ProveFn;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type SolanaSccpSourceStateProverOptions =\s+SccpProverProveOption<SolanaSccpSourceStateProveFn>;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type TonSccpSourceStateProverOptions =\s+SccpProverProveOption<TonSccpSourceStateProveFn>;/,
  );
  for (const [lane, inputType] of [
    ["Ton", "TonSccpProofRequestInput"],
    ["Evm", "EvmSccpProofRequestInput"],
    ["Tron", "TronSccpProofRequestInput"],
    ["Substrate", "SubstrateSccpProofRequestInput"],
    ["Solana", "SolanaSccpWitnessInput"],
  ]) {
    assert.match(
      DECLARATIONS_TEXT,
      new RegExp(
        `export type ${lane}SccpWitnessProvider =[\\s\\S]*` +
          `SccpWitnessProviderResolverOption<${inputType}>;`,
      ),
      `${lane} provider declarations must use the exclusive resolver alias helper`,
    );
    assert.match(
      DECLARATIONS_TEXT,
      new RegExp(
        `export type ${lane}SccpProverOptions =[\\s\\S]*` +
          `SccpProverWitnessProviderOption<[\\s\\S]*${lane}SccpWitnessProvider,[\\s\\S]*${inputType}[\\s\\S]*>[\\s\\S]*` +
          `SccpProverProveOption<${lane}SccpProveFn>;`,
      ),
      `${lane} prover options must reject duplicate witness/prove aliases`,
    );
  }
});

test("package declarations expose Ethereum mainnet finality evidence hooks", () => {
  assert.match(
    DECLARATIONS_TEXT,
    /export interface EthereumMainnetBeaconFinalityEvidenceInput[\s\S]*executionBlockNumber\?: string \| number \| bigint;[\s\S]*executionBlockHash\?: string;[\s\S]*executionReceiptsRoot\?: string;[\s\S]*finalizedHeaderRoot\?: string;[\s\S]*syncCommitteeRoot\?: string;[\s\S]*beaconSlot\?: string \| number \| bigint;[\s\S]*finalizedSlot\?: string \| number \| bigint;[\s\S]*slot\?: string \| number \| bigint;[\s\S]*finalityBranch\?: readonly string\[\];[\s\S]*finality_branch\?: readonly string\[\];[\s\S]*syncCommitteeBits\?: string;[\s\S]*syncCommitteeSignature\?: string;[\s\S]*syncSignatureSlot\?: string \| number \| bigint;[\s\S]*signatureSlot\?: string \| number \| bigint;[\s\S]*syncCommitteeParticipation\?: string \| number \| bigint;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface EthereumMainnetBeaconFinalityEvidence[\s\S]*readonly executionBlockNumber: string;[\s\S]*readonly executionBlockHash: string;[\s\S]*readonly executionReceiptsRoot: string;[\s\S]*readonly finalizedHeaderRoot\?: string;[\s\S]*readonly syncCommitteeRoot\?: string;[\s\S]*readonly beaconSlot\?: string;[\s\S]*readonly finalityBranch\?: readonly string\[\];[\s\S]*readonly syncCommitteeBits\?: string;[\s\S]*readonly syncCommitteeSignature\?: string;[\s\S]*readonly syncSignatureSlot\?: string;[\s\S]*readonly syncCommitteeParticipation\?: string;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface EthereumMainnetConsensusProviderInput[\s\S]*readonly receipt\?: Record<string, unknown>;[\s\S]*readonly block\?: Record<string, unknown>;[\s\S]*readonly transactionHash\?: string;[\s\S]*readonly beaconBlockId\?: string \| number \| bigint;[\s\S]*readonly targetBeaconBlockRoot\?: string;[\s\S]*readonly beaconSlot\?: string \| number \| bigint;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type EthereumMainnetConsensusProvider = \{[\s\S]*collectFinalityEvidence\([\s\S]*input: EthereumMainnetConsensusProviderInput,[\s\S]*\):[\s\S]*EthereumMainnetBeaconFinalityEvidenceInput[\s\S]*Promise<EthereumMainnetBeaconFinalityEvidenceInput>;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface EthereumMainnetBeaconRestConsensusProviderOptions[\s\S]*endpoint\?: string \| URL;[\s\S]*fetch\?: EthereumMainnetBeaconRestFetch;[\s\S]*syncCommitteeRoot\?: string;[\s\S]*syncCommitteePayload\?: EthSyncCommitteePayloadInput \| BinaryLike;[\s\S]*verifyFinalityCheckpoint\?: boolean;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export class EthereumMainnetBeaconRestConsensusProvider[\s\S]*constructor\([\s\S]*options: EthereumMainnetBeaconRestConsensusProviderOptions \| string \| URL,[\s\S]*\);[\s\S]*collectFinalityEvidence\([\s\S]*input: EthereumMainnetConsensusProviderInput,[\s\S]*beaconBlockId\?: string \| number \| bigint;[\s\S]*targetBeaconBlockRoot\?: string;[\s\S]*beaconSlot\?: string \| number \| bigint;[\s\S]*\): Promise<EthereumMainnetBeaconFinalityEvidence>;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface EthereumMainnetInboundEvidenceInput[\s\S]*beaconFinality\?: EthereumMainnetBeaconFinalityEvidenceInput;[\s\S]*finalityEvidence\?: EthereumMainnetBeaconFinalityEvidenceInput;[\s\S]*receiptProofHash\?: string;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /proveInboundToSora\([\s\S]*executionProvider\?: EthereumMainnetExecutionProvider;[\s\S]*consensusProvider\?: EthereumMainnetConsensusProvider;[\s\S]*proveInbound\?: EthereumMainnetInboundProveFn;/,
  );
});

test("package declarations expose Ethereum mainnet SCCP facade methods", () => {
  assert.equal(
    new EthereumMainnetBeaconRestConsensusProvider({
      endpoint: "https://beacon.example",
      fetch: async () => ({ ok: true, json: async () => ({ data: {} }) }),
      syncCommitteeRoot: "0x".padEnd(66, "1"),
    }) instanceof EthereumMainnetBeaconRestConsensusProvider,
    true,
  );
  const declaration = declarationClass("EthereumMainnetSccp");
  assert.match(
    declaration,
    /static fromNativeProverBundle\([\s\S]*options: EthereumMainnetSccpNativeProverBundleOptions,[\s\S]*\): Promise<EthereumMainnetSccp>;/u,
  );
  assert.match(declaration, /validateExecutionProviderMainnet\([\s\S]*\): Promise<unknown>;/u);
  assert.match(
    declaration,
    /collectInboundEvidenceFromReceipt\([\s\S]*input\?: EthereumMainnetInboundEvidenceInput,[\s\S]*\): Promise<EthereumMainnetInboundEvidence>;/u,
  );
  assert.match(
    declaration,
    /proveInboundToSora\([\s\S]*input: EthereumMainnetInboundEvidenceInput,[\s\S]*\): Promise<Uint8Array>;/u,
  );
  assert.match(
    declaration,
    /submitInboundToIroha\([\s\S]*input: BinaryLike,[\s\S]*\): Promise<unknown>;/u,
  );
  assert.match(
    declaration,
    /buildOutboundProofRequest\([\s\S]*input: EvmSccpProofRequestInput,[\s\S]*\): EvmSccpProofRequest;/u,
  );
  assert.match(
    declaration,
    /runNativeProverSelfTest\([\s\S]*\): Promise<EthereumMainnetNativeEvmProverSelfTestSdkResult>;/u,
  );
  assert.match(
    declaration,
    /proveOutboundToEthereum\([\s\S]*input: EvmSccpProofRequestInput,[\s\S]*\): Promise<EvmSccpProofResult>;/u,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type EthereumMainnetNativeProverSelfTestFn = \([\s\S]*context: Readonly<EthereumMainnetNativeProverSelfTestContext>[\s\S]*EthereumMainnetNativeEvmProverSelfTestSdkResultInput/u,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export function runEthereumMainnetNativeProverSelfTest\([\s\S]*input: EthereumMainnetNativeProverSelfTestRunInput,[\s\S]*\): Promise<EthereumMainnetNativeEvmProverSelfTestSdkResult>;/u,
  );
  assert.match(
    declaration,
    /buildEthereumCalldata\([\s\S]*input: EthereumMainnetSccpSubmissionInput,[\s\S]*\): EvmSccpSubmission;/u,
  );
  assert.match(
    declaration,
    /submitOutboundToEthereum\([\s\S]*input: EthereumMainnetSccpSubmissionInput & \{[\s\S]*\): Promise<unknown>;/u,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type EthereumMainnetInboundProveFn = \([\s\S]*\) => BinaryLike \| Promise<BinaryLike>;/u,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type EthereumMainnetSccpNativeProverBundleOptions = Omit<[\s\S]*EthereumMainnetNativeEvmProverArtifactBundleInput;/u,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type EthereumMainnetSubmitInboundFn = \([\s\S]*proofBytes: Uint8Array,[\s\S]*\) => unknown \| Promise<unknown>;/u,
  );
});

function assertBrowserMainnetSccpArtifactsStayJsOnlyAndLocalProverOwned() {
  const artifacts = {
    "package.json": PACKAGE_JSON_TEXT,
    "src/sccp.js": SCCP_SOURCE_TEXT,
    "src/index.js": INDEX_SOURCE_TEXT,
    "dist/sccp.js": DIST_SCCP_TEXT,
    "dist/index.js": DIST_INDEX_TEXT,
    "index.d.ts": DECLARATIONS_TEXT,
  };
  const forbidden = [
    /\bWebAssembly\b/u,
    /\bwasm\b/iu,
    /\bsnarkjs\b/iu,
    /\bremoteProver\b/u,
    /\bremote prover\b/iu,
    /\bremote_prover\b/iu,
    /\bremote-prover\b/iu,
    /\bproverUrl\b/u,
    /\bproverURL\b/u,
    /\bprover_url\b/iu,
    /\bproverEndpoint\b/u,
    /\bprover_endpoint\b/iu,
  ];
  for (const [artifact, source] of Object.entries(artifacts)) {
    for (const pattern of forbidden) {
      assert.doesNotMatch(source, pattern, `${artifact} must not depend on ${pattern}`);
    }
  }
}

test("browser SCCP no-WASM guard catches remote-prover identifier variants", () => {
  const samples = [
    "WebAssembly.compile(bytes)",
    "import './proof.wasm'",
    "import snarkjs from 'snarkjs'",
    "const remoteProver = endpoint",
    "fallback remote prover",
    "const remote_prover = endpoint",
    "remote-prover endpoint",
    "const proverUrl = endpoint",
    "const proverURL = endpoint",
    "const prover_url = endpoint",
    "const proverEndpoint = endpoint",
    "const prover_endpoint = endpoint",
  ];
  const forbidden = [
    /\bWebAssembly\b/u,
    /\bwasm\b/iu,
    /\bsnarkjs\b/iu,
    /\bremoteProver\b/u,
    /\bremote prover\b/iu,
    /\bremote_prover\b/iu,
    /\bremote-prover\b/iu,
    /\bproverUrl\b/u,
    /\bproverURL\b/u,
    /\bprover_url\b/iu,
    /\bproverEndpoint\b/u,
    /\bprover_endpoint\b/iu,
  ];
  for (const sample of samples) {
    assert(
      forbidden.some((pattern) => pattern.test(sample)),
      `${sample} must match a browser SCCP no-WASM guard`,
    );
  }
});

test("browser Ethereum mainnet SCCP artifacts stay JS-only and local-prover owned", () => {
  assertBrowserMainnetSccpArtifactsStayJsOnlyAndLocalProverOwned();
});

test("browser BSC mainnet SCCP artifacts stay JS-only and local-prover owned", () => {
  assertBrowserMainnetSccpArtifactsStayJsOnlyAndLocalProverOwned();
});

test("package declarations expose BSC mainnet Parlia finality evidence hooks", () => {
  assert.match(
    DECLARATIONS_TEXT,
    /export interface BscMainnetParliaFinalityEvidenceInput[\s\S]*executionBlockNumber\?: string \| number \| bigint;[\s\S]*executionBlockHash\?: string;[\s\S]*executionReceiptsRoot\?: string;[\s\S]*validatorEpoch\?: string \| number \| bigint;[\s\S]*commitSealHash\?: string;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface BscMainnetParliaFinalityEvidence[\s\S]*readonly executionBlockNumber: string;[\s\S]*readonly executionBlockHash: string;[\s\S]*readonly executionReceiptsRoot: string;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface BscMainnetConsensusProviderInput[\s\S]*readonly receipt\?: Record<string, unknown>;[\s\S]*readonly block\?: Record<string, unknown>;[\s\S]*readonly transactionHash\?: string;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type BscMainnetConsensusProvider = \{[\s\S]*collectFinalityEvidence\([\s\S]*input: BscMainnetConsensusProviderInput,[\s\S]*\):[\s\S]*BscMainnetParliaFinalityEvidenceInput[\s\S]*Promise<BscMainnetParliaFinalityEvidenceInput>;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface BscMainnetInboundEvidenceInput[\s\S]*parliaFinality\?: BscMainnetParliaFinalityEvidenceInput;[\s\S]*finalityEvidence\?: BscMainnetParliaFinalityEvidenceInput;[\s\S]*receiptProof\?: BscSccpReceiptProofInput;[\s\S]*receiptProofHash\?: string;[\s\S]*receipt_proof_hash\?: string;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /proveInboundToSora\([\s\S]*executionProvider\?: BscMainnetExecutionProvider;[\s\S]*consensusProvider\?: BscMainnetConsensusProvider;[\s\S]*proveInbound\?: BscMainnetInboundProveFn;/,
  );
});

test("package declarations separate TON proof-request and submission inputs", () => {
  const manifestInput = declarationInterface("TonSccpManifestInput");
  const proofRequestInput = declarationInterface("TonSccpProofRequestInput");
  const messageBodyInputBase = declarationInterface("TonSccpMessageBodyInputBase");

  assert.match(manifestInput, /version: SccpVersionInput;/);
  assert.doesNotMatch(manifestInput, /version\?:/);
  assert.match(manifestInput, /proofFamily\?: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;/);
  assert.match(manifestInput, /proof_family\?: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;/);
  assert.match(
    manifestInput,
    /verifierBackendKey\?: typeof SCCP_TON_CONTRACT_PROOF_BACKEND_V1;/,
  );
  assert.match(
    manifestInput,
    /verifier_backend_key\?: typeof SCCP_TON_CONTRACT_PROOF_BACKEND_V1;/,
  );
  assert.match(proofRequestInput, /publicInputs\?: SccpMessageTransparentPublicInputsInput;/);
  assert.match(proofRequestInput, /bundleBytes\?: BinaryLike;/);
  assert.match(proofRequestInput, /sourceStateVerifierHash\?: string;/);
  assert.match(
    proofRequestInput,
    /sourceAdapterDeploymentBinding\?: SccpSourceAdapterDeploymentBindingInput;/,
  );
  assert.doesNotMatch(proofRequestInput, /proofResult\?:/);
  assert.doesNotMatch(proofRequestInput, /proofBytes\?:/);
  assert.doesNotMatch(proofRequestInput, /metadataBytes\?:/);
  assert.doesNotMatch(proofRequestInput, /manifest\?:/);
  assert.doesNotMatch(proofRequestInput, /queryId\?:/);

  assert.match(messageBodyInputBase, /extends TonSccpProofRequestInput/);
  assert.doesNotMatch(messageBodyInputBase, /proofResult\?:/);
  assert.match(messageBodyInputBase, /proofBytes\?: BinaryLike;/);
  assert.match(messageBodyInputBase, /metadataBytes\?: BinaryLike;/);
  assert.match(messageBodyInputBase, /manifest\?: TonSccpManifestInput;/);
  assert.match(messageBodyInputBase, /queryId\?: string \| number \| bigint;/);
  assert.match(
    DECLARATIONS_TEXT,
    /export type TonSccpMessageBodyInput =\s+TonSccpMessageBodyInputBase &[\s\S]*proofResult: TonSccpProofResult;[\s\S]*proof_result: TonSccpProofResult;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export function buildTonSccpProofRequest\([\s\S]*input: TonSccpProofRequestInput,[\s\S]*\): TonSccpProofRequest;/,
  );
});

test("package declarations require wrapped Solana submission proof results", () => {
  const submissionInputBase = declarationInterface("SolanaSccpSubmissionInputBase");

  assert.match(submissionInputBase, /publicInputs\?: SccpMessageTransparentPublicInputsInput;/);
  assert.match(submissionInputBase, /proofBytes\?: BinaryLike;/);
  assert.doesNotMatch(submissionInputBase, /proofResult\?:/);
  assert.match(
    DECLARATIONS_TEXT,
    /export type SolanaSccpSubmissionInput =\s+SolanaSccpSubmissionInputBase &[\s\S]*proofResult: SolanaSccpProofResult;[\s\S]*proof_result\?: never;[\s\S]*proofResult\?: never;[\s\S]*proof_result: SolanaSccpProofResult;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface SolanaSccpSubmissionInputWithProofResult[\s\S]*proofResult: SolanaSccpProofResult;[\s\S]*proof_result\?: never;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export function buildSolanaSccpSubmission\([\s\S]*input: SolanaSccpSubmissionInput,[\s\S]*\): SolanaSccpSubmission;/,
  );
});

test("package dist Solana submission rejects inert bundle bytes", () => {
  const publicInputs = {
    version: 1,
    message_id: `0x${"11".repeat(32)}`,
    payload_hash: `0x${"22".repeat(32)}`,
    target_domain: SCCP_DOMAIN_SOL,
    commitment_root: `0x${"33".repeat(32)}`,
    finality_height: "42",
    finality_block_hash: `0x${"44".repeat(32)}`,
  };
  const baseSubmission = {
    publicInputs,
    proofResult: {},
    proofBytes: new Uint8Array([1]),
  };

  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        ...baseSubmission,
        bundleBytes: new Uint8Array([0, 0]),
      }),
    /bundleBytes must not be all zero/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        ...baseSubmission,
        bundleBytes: new Uint8Array(SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1).fill(1),
      }),
    /bundleBytes must be at most/,
  );
});

test("package dist entrypoint exports SCCP portal constants", () => {
  assert.equal(SCCP_MESSAGE_TRANSPARENT_PUBLIC_INPUTS_BYTES_V1_LEN, 141);
  assert.equal(SCCP_SOLANA_SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1, "submit_sccp_message_proof");
  assert.match(SCCP_TON_MAINNET_MASTERCHAIN_CONFIG_VERIFIER_ID_V1, /^sccp:ton:/u);
  assert.match(SCCP_TON_MAINNET_VALIDATOR_SET_TRANSITION_VERIFIER_ID_V1, /^sccp:ton:/u);
  assert.match(SCCP_TON_MAINNET_SHARD_ACCOUNTS_DICTIONARY_VERIFIER_ID_V1, /^sccp:ton:/u);
  assert.match(
    DECLARATIONS_TEXT,
    /export const SCCP_MESSAGE_TRANSPARENT_PUBLIC_INPUTS_BYTES_V1_LEN: 141;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export const SCCP_SOLANA_SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1: "submit_sccp_message_proof";/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export const SCCP_TON_MAINNET_MASTERCHAIN_CONFIG_VERIFIER_ID_V1: string;/,
  );
});

test("package dist entrypoint exports Solana source-state helpers", () => {
  assert.equal(SCCP_SOURCE_STATE_MAX_PROOF_BYTES, 2 * 1024 * 1024);
  assert.equal(SCCP_SOURCE_STATE_MAX_PROOF_LABEL_BYTES, 128);
  assert.equal(SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES, 2 * 1024 * 1024);
  assert.equal(
    SCCP_SOLANA_TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1,
    "sccp-solana-tower-replay-v1",
  );
  assert.equal(
    SCCP_SOLANA_FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1,
    "sccp-solana-full-accountsdb-lattice-v1",
  );
  assert.equal(
    SCCP_SOLANA_BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1,
    "sccp-solana-bank-fork-choice-v1",
  );
  for (const helper of [
    canonicalSolanaSccpSourceStateVerificationProofBytes,
    solanaSccpAccountsLtHashProofHash,
    canonicalSolanaSccpFinalityContextBytes,
    solanaSccpFinalityContextHash,
    canonicalSolanaSccpVoteMessageBytes,
    solanaSccpVoteMessageHash,
    canonicalSolanaSccpFullLightClientAuditStatementBytes,
    solanaSccpFullLightClientAuditStatementHash,
    solanaSccpFullLightClientAuditPublicInputColumns,
    solanaSccpFullLightClientAuditOpenVerifySchemaDescriptor,
    buildSolanaSccpFullLightClientAuditProofRequest,
    buildSolanaSccpTowerReplayProofRequest,
    buildSolanaSccpFullAccountsdbLatticeProofRequest,
    buildSolanaSccpBankForkChoiceProofRequest,
    buildSolanaSccpFullLightClientAuditProofRequests,
    canonicalSolanaSccpRouteCanaryEvidenceBytes,
    solanaSccpRouteCanaryEvidenceHash,
    canonicalTonSccpRouteCanaryEvidenceBytes,
    tonSccpRouteCanaryEvidenceHash,
    canonicalTronSccpRouteCanaryEvidenceBytes,
    tronSccpRouteCanaryEvidenceHash,
  ]) {
    assert.equal(typeof helper, "function");
  }
  assert.equal(
    SCCP_SOLANA_UPGRADEABLE_LOADER_ID,
    "BPFLoaderUpgradeab1e11111111111111111111111",
  );
  assert.equal(
    solanaSccpRouteCanaryEvidenceHash({
      routeAllowlistHash: `0x${"31".repeat(32)}`,
      destinationBindingHash: sccpDestinationBindingHash(SCCP_DOMAIN_SOL),
      sourceVerifierMaterialHash: `0x${"33".repeat(32)}`,
      sourceAdapterEngineDeploymentHash: `0x${"34".repeat(32)}`,
      verifierIdentity: "3JF3sEqM796hk5WFqA6EtmEwJQ9quALszsfJyvXNQKy3",
      verifierCodeHash: "0xc81178d11a4de525782fe7ac6f5accc2056fa15d1b8c2bfd819eb2ef179c3411",
      solanaRpcCommitment: "finalized",
      solanaProgramOwner: SCCP_SOLANA_UPGRADEABLE_LOADER_ID,
      solanaProgramdataOwner: SCCP_SOLANA_UPGRADEABLE_LOADER_ID,
      solanaProgramImmutable: true,
      solanaProgramAccountDataBase64: "AgAAABERERERERERERERERERERERERERERERERERERERERER",
      solanaProgramdataAddress: "29d2S7vB453rNYFdR5Ycwt7y9haRT5fwVwL9zTmBhfV2",
      solanaProgramdataSlot: "4321",
      solanaExpectedProgramdataSlot: "4321",
      solanaProgramAccountContextSlot: "5000",
      solanaProgramdataAccountContextSlot: "5001",
      solanaProgramdataMetadataBlake2b256:
        "0x2b5f26278ea949463e97c1dc5e53a821b82515b405454a1b0e3cd652c3b00209",
      solanaProgramdataMetadataBase64:
        "AwAAAOEQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
      solanaProgramdataExecutableBlake2b256:
        "0xc81178d11a4de525782fe7ac6f5accc2056fa15d1b8c2bfd819eb2ef179c3411",
      solanaProgramdataExecutableBase64: "f0VMRgECAwQF",
    }),
    "0x77296e47d5681f97136dc79d66dbda4478c3c5ec80271bfd4f1f3b3dbb8e15ca",
  );
  assert.equal(
    tonSccpRouteCanaryEvidenceHash({
      routeAllowlistHash: `0x${"31".repeat(32)}`,
      destinationBindingHash: sccpDestinationBindingHash(SCCP_DOMAIN_TON),
      sourceVerifierMaterialHash: `0x${"33".repeat(32)}`,
      sourceAdapterEngineDeploymentHash: `0x${"34".repeat(32)}`,
      verifierContractAddress: `0:${"11".repeat(32)}`,
      verifierCodeHash: `0x${"44".repeat(32)}`,
      accountStatus: "active",
      accountStateHash: `0x${"55".repeat(32)}`,
      lastTransactionLt: "123456789",
      lastTransactionHash: `0x${"66".repeat(32)}`,
      verifierCodeBocRootHash: `0x${"44".repeat(32)}`,
    }),
    "0xf128e8405017b9ca7733bb10d43eeaf783e38d39740a3455aa353c76655c6942",
  );
  assert.equal(
    canonicalTronSccpRouteCanaryEvidenceBytes({
      routeAllowlistHash: "0xfea8effb3cddfa458ea79a5a9af6f2d2c33a460b3a66d9305963908c2a3ea67a",
      destinationBindingHash:
        "0x17c953ad5b8c9a2b6f7102aca993fa7c427d018505cf4f58fac35ea454caba7f",
      sourceVerifierMaterialHash:
        "0x68c20262e44676bd5f3c4ec428f063373147a1ca14c5885648a9c651b3bcd8d8",
      sourceAdapterEngineDeploymentHash:
        "0x94dbe28a2fb16e043b83639b6dea8ec62f53679599ef1dd220fd13c71c7bdcb8",
      networkId: `0x${"33".repeat(32)}`,
      verifierAddress: "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
      verifierCodeHash: `0x${"bb".repeat(32)}`,
      verifierKeyHash: `0x${"cc".repeat(32)}`,
      transactionId: `0x${"fa".repeat(32)}`,
      transactionOwnerAddress: "0x417e5f4552091a69125d5dfcb7b8c2659029395bdf",
      blockNumber: 234n,
      blockTimestamp: 567000n,
      logIndex: 0,
      messageId: `0x${"dd".repeat(32)}`,
      callDataSha256:
        "0xf96dfb36d47a61e7e80df4f19e00b78c12f9a3f3c542e8dac06a7422e1d5f951",
      payloadHash: `0x${"ab".repeat(32)}`,
      commitmentRoot: `0x${"ee".repeat(32)}`,
      finalityHeight: `0x${"00".repeat(31)}7b`,
      finalityBlockHash: `0x${"cd".repeat(32)}`,
      statementHash: `0x${"f1".repeat(32)}`,
      usedMessageProof: true,
      rawDataOwnerMatchesTransaction: true,
      signatureSha256: `0x${"c4".repeat(32)}`,
      signatureRecoveredAddress: "0x417e5f4552091a69125d5dfcb7b8c2659029395bdf",
      signatureRecoversToOwner: true,
    }).length,
    551,
  );
  assert.equal(
    tronSccpRouteCanaryEvidenceHash({
      routeAllowlistHash: "0xfea8effb3cddfa458ea79a5a9af6f2d2c33a460b3a66d9305963908c2a3ea67a",
      destinationBindingHash:
        "0x17c953ad5b8c9a2b6f7102aca993fa7c427d018505cf4f58fac35ea454caba7f",
      sourceVerifierMaterialHash:
        "0x68c20262e44676bd5f3c4ec428f063373147a1ca14c5885648a9c651b3bcd8d8",
      sourceAdapterEngineDeploymentHash:
        "0x94dbe28a2fb16e043b83639b6dea8ec62f53679599ef1dd220fd13c71c7bdcb8",
      networkId: `0x${"33".repeat(32)}`,
      verifierAddress: "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
      verifierCodeHash: `0x${"bb".repeat(32)}`,
      verifierKeyHash: `0x${"cc".repeat(32)}`,
      transactionId: `0x${"fa".repeat(32)}`,
      transactionOwnerAddress: "0x417e5f4552091a69125d5dfcb7b8c2659029395bdf",
      blockNumber: 234n,
      blockTimestamp: 567000n,
      logIndex: 0,
      messageId: `0x${"dd".repeat(32)}`,
      callDataSha256:
        "0xf96dfb36d47a61e7e80df4f19e00b78c12f9a3f3c542e8dac06a7422e1d5f951",
      payloadHash: `0x${"ab".repeat(32)}`,
      commitmentRoot: `0x${"ee".repeat(32)}`,
      finalityHeight: `0x${"00".repeat(31)}7b`,
      finalityBlockHash: `0x${"cd".repeat(32)}`,
      statementHash: `0x${"f1".repeat(32)}`,
      usedMessageProof: true,
      rawDataOwnerMatchesTransaction: true,
      signatureSha256: `0x${"c4".repeat(32)}`,
      signatureRecoveredAddress: "0x417e5f4552091a69125d5dfcb7b8c2659029395bdf",
      signatureRecoversToOwner: true,
      routeCanaryEvidenceHash:
        "0xe0a96ff7e8f523599fd60fffe8bb3b9fda9519126b7ba00c89c922b323b64e56",
    }),
    "0xe0a96ff7e8f523599fd60fffe8bb3b9fda9519126b7ba00c89c922b323b64e56",
  );
  assert.match(
    DECLARATIONS_TEXT,
    /wrapSolanaSccpSourceStateVerificationProof\(\s+proofBytes: BinaryLike \| number\[\],[\s\S]*request:[\s\S]*SolanaSccpAccountsLtHashProofRequest[\s\S]*SolanaSccpFullLightClientAuditProofRequest,/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export function solanaSccpRouteCanaryEvidenceHash\([\s\S]*SolanaSccpRouteCanaryEvidenceInput/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export function tonSccpRouteCanaryEvidenceHash\([\s\S]*TonSccpRouteCanaryEvidenceInput/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export function tronSccpRouteCanaryEvidenceHash\([\s\S]*TronSccpRouteCanaryEvidenceInput/,
  );

  const rawDataHash = solanaSccpAccountRawDataHash(new Uint8Array([1, 2, 3, 4]));
  assert.match(rawDataHash, /^0x[0-9a-f]{64}$/u);
  const opening = {
    address: `0x${"41".repeat(32)}`,
    owner: SCCP_SOLANA_VOTE_PROGRAM_ID,
    lamports: 1n,
    rentEpoch: 0n,
    executable: false,
    dataHash: rawDataHash,
  };
  const leafInput = {
    finalizedSlot: 42n,
    opening,
    rawDataHash,
  };
  assert.equal(canonicalSolanaSccpAccountInclusionLeafBytes(leafInput).length, 109);
  const leafHash = solanaSccpAccountInclusionLeafHash(leafInput);
  assert.match(leafHash, /^0x[0-9a-f]{64}$/u);
  assert.equal(
    canonicalSolanaSccpAccountInclusionNodeBytes(leafHash, `0x${"88".repeat(32)}`).length,
    65,
  );
  assert.match(
    solanaSccpAccountInclusionNodeHash(leafHash, `0x${"88".repeat(32)}`),
    /^0x[0-9a-f]{64}$/u,
  );
  const tree = solanaSccpAccountInclusionRootAndBranches([leafHash, `0x${"88".repeat(32)}`]);
  assert.match(tree.root, /^0x[0-9a-f]{64}$/u);
  assert.equal(tree.branches.length, 2);
  assert.equal(tree.branches[0].length, 1);
  assert.ok(Object.isFrozen(tree));
  assert.ok(Object.isFrozen(tree.branches));
  assert.ok(Object.isFrozen(tree.branches[0]));
  assert.equal(solanaSccpAccountInclusionRootFromBranch(leafHash, tree.branches[0]), tree.root);

  assert.match(
    DECLARATIONS_TEXT,
    /export interface SolanaSccpAccountInclusionLeafInput/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export function solanaSccpAccountRawDataHash\(rawData: BinaryLike\): string;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export function canonicalSolanaSccpAccountInclusionLeafBytes\([\s\S]*input: SolanaSccpAccountInclusionLeafInput[\s\S]*\): Uint8Array;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface SolanaSccpAccountInclusionRootAndBranches[\s\S]*readonly branches: ReadonlyArray<ReadonlyArray<string>>;[\s\S]*export function solanaSccpAccountInclusionRootAndBranches\([\s\S]*leaves: readonly BinaryLike\[\][\s\S]*\): SolanaSccpAccountInclusionRootAndBranches;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface SolanaSccpOpenedAccountInclusionWitness[\s\S]*readonly branches: ReadonlyArray<ReadonlyArray<string>>;[\s\S]*readonly stakeHistorySysvarBranch: ReadonlyArray<string>;/,
  );
});

test("package dist entrypoint exports Solana tower lockout helpers", () => {
  assert.equal(SCCP_DOMAIN_SOL, 3);
  assert.equal(SCCP_SOLANA_TOWER_LOCKOUT_CONFIRMATION_DEPTH, 32n);
  assert.equal(SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH, 31n);
  const input = {
    finalizedSlot: 1_296_096n,
    rootedSlot: 1_296_065n,
    parentSlot: 1_296_095n,
    parentBankHash: `0x${"33".repeat(32)}`,
  };
  assert.equal(canonicalSolanaSccpTowerLockoutBytes(input).length, 73);
  assert.match(solanaSccpTowerLockoutHash(input), /^0x[0-9a-f]{64}$/u);
});

test("package dist entrypoint exports Solana stake activation helpers", () => {
  const input = {
    epoch: 3n,
    validatorPublicKeys: [`0x${"11".repeat(32)}`, `0x${"22".repeat(32)}`],
    validatorStakes: [1n, 2n],
    validatorActivationEpochs: [0n, 2n],
    validatorDeactivationEpochs: [(1n << 64n) - 1n, 9n],
  };
  assert.equal(canonicalSolanaSccpStakeActivationBytes(input).length, 165);
  assert.equal(
    solanaSccpStakeActivationHash(input),
    "0xdb418c62a1aeb8ae15cb26e3a198d46890cefa3545df8e1921be2e83f57dabf3",
  );
});

test("package dist entrypoint exports Solana account opening helpers", () => {
  const input = {
    address: `0x${"31".repeat(32)}`,
    owner: SCCP_SOLANA_VOTE_PROGRAM_ID,
    lamports: 1_000_000n,
    rentEpoch: 0n,
    executable: false,
    dataHash: `0x${"71".repeat(32)}`,
  };
  assert.equal(canonicalSolanaSccpAccountOpeningBytes(input).length, 122);
  const accountHash = solanaSccpAccountOpeningHash(input);
  assert.match(accountHash, /^0x[0-9a-f]{64}$/u);
  assert.notEqual(
    accountHash,
    solanaSccpAccountOpeningHash({ ...input, owner: SCCP_SOLANA_STAKE_PROGRAM_ID }),
  );
  assert.equal(typeof solanaSccpOpenedAccountInclusionWitness, "function");
  assert.equal(
    SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1,
    "sccp-solana-accounts-lt-hash-v1",
  );
  assert.equal(typeof buildSolanaSccpAccountsLtHashProofRequest, "function");
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(new Uint8Array([1, 2, 3]), {
        version: 1,
        proofFamily: SCCP_STARK_FRI_PROOF_FAMILY_V1,
        circuitId: SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1,
        sourceDomain: SCCP_DOMAIN_SOL,
      }),
    /request\.parameterSet/,
  );
  assert.equal(typeof canonicalSolanaSccpAccountsLtHashCommitmentBytes, "function");
  assert.equal(typeof canonicalSolanaSccpAccountsLtHashVerificationContextBytes, "function");
  assert.equal(typeof solanaSccpAccountsLtHashPublicInputColumns, "function");
  assert.equal(typeof solanaSccpAccountsLtHashOpenVerifySchemaDescriptor, "function");
  assert.equal(typeof buildTonShardStateProofRequest, "function");
  assert.equal(typeof canonicalTonShardStateProofPublicInputsBytes, "function");
  assert.equal(typeof canonicalTonShardStateWitnessCommitmentBytes, "function");
  assert.equal(typeof canonicalTonShardStateVerificationContextBytes, "function");
  assert.equal(typeof tonShardStateProofPublicInputsHash, "function");
  assert.equal(typeof tonShardStatePublicInputColumns, "function");
  assert.equal(typeof tonShardStateOpenVerifySchemaDescriptor, "function");
  assert.equal(
    SCCP_TON_MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1,
    "sccp-ton-masterchain-config-v1",
  );
  assert.equal(
    SCCP_TON_VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1,
    "sccp-ton-validator-set-transition-v1",
  );
  assert.equal(
    SCCP_TON_SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1,
    "sccp-ton-shard-accounts-dictionary-v1",
  );
  assert.equal(typeof tonSccpShardStateVerificationProofHash, "function");
  assert.equal(typeof canonicalTonSccpFullLightClientAuditStatementBytes, "function");
  assert.equal(typeof tonSccpFullLightClientAuditStatementHash, "function");
  assert.equal(typeof tonSccpFullLightClientAuditPublicInputColumns, "function");
  assert.equal(typeof tonSccpFullLightClientAuditOpenVerifySchemaDescriptor, "function");
  assert.equal(typeof buildTonSccpFullLightClientAuditProofRequest, "function");
  assert.equal(typeof buildTonSccpFullLightClientAuditProofRequests, "function");
});

test("package dist entrypoint exports Solana account data helpers", () => {
  const towerVoteSlots = Array.from({ length: Number(SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH) }, (_, index) =>
    11n + BigInt(index),
  );
  const voteInput = {
    nodePubkey: `0x${"51".repeat(32)}`,
    authorizedVoter: `0x${"61".repeat(32)}`,
    authorizedWithdrawer: `0x${"71".repeat(32)}`,
    inflationRewardsCollector: `0x${"81".repeat(32)}`,
    blockRevenueCollector: `0x${"51".repeat(32)}`,
    inflationRewardsCommissionBps: 700n,
    blockRevenueCommissionBps: 10_000n,
    pendingDelegatorRewards: 123n,
    blsPubkeyCompressed: new Uint8Array(),
    rootSlot: 10n,
    towerVoteSlots,
  };
  assert.equal(canonicalSolanaSccpVoteAccountDataBytes(voteInput).length, 457);
  const voteHash = solanaSccpVoteAccountDataHash(voteInput);
  assert.match(voteHash, /^0x[0-9a-f]{64}$/u);
  assert.notEqual(
    voteHash,
    solanaSccpVoteAccountDataHash({ ...voteInput, authorizedVoter: `0x${"62".repeat(32)}` }),
  );
  assert.throws(
    () => solanaSccpVoteAccountDataHash({ ...voteInput, towerVoteSlots: [10n, ...towerVoteSlots.slice(1)] }),
    /towerVoteSlots\[0\]/u,
  );

  const stakeInput = {
    staker: `0x${"81".repeat(32)}`,
    withdrawer: `0x${"91".repeat(32)}`,
    voterPubkey: `0x${"a1".repeat(32)}`,
    delegatedStake: 1_000n,
    activationEpoch: 2n,
    deactivationEpoch: 9n,
    creditsObserved: 123n,
  };
  assert.equal(canonicalSolanaSccpStakeAccountDataBytes(stakeInput).length, 154);
  const stakeHash = solanaSccpStakeAccountDataHash(stakeInput);
  assert.match(stakeHash, /^0x[0-9a-f]{64}$/u);
  assert.notEqual(
    stakeHash,
    solanaSccpStakeAccountDataHash({ ...stakeInput, voterPubkey: `0x${"a2".repeat(32)}` }),
  );
  assert.throws(
    () => solanaSccpStakeAccountDataHash({ ...stakeInput, deactivationEpoch: 2n }),
    /deactivationEpoch/u,
  );
});

test("package dist entrypoint exports Solana raw vote account parser helpers", () => {
  const raw = sampleSolanaVoteStateAccount();
  const voteAccountAddress = `0x${"81".repeat(32)}`;
  const parsed = solanaSccpVoteAccountDataFromRawVoteState(raw, 3n, voteAccountAddress);
  assert.deepEqual(Array.from(parsed.authorizedVoter), Array(32).fill(0x61));
  assert.deepEqual(Array.from(parsed.inflationRewardsCollector), Array(32).fill(0x81));
  assert.equal(parsed.inflationRewardsCommissionBps, 700n);
  assert.equal(parsed.rootSlot, 10n);
  assert.equal(
    solanaSccpVoteAccountDataHashFromRawVoteState(raw, 3n, voteAccountAddress),
    solanaSccpVoteAccountDataHash(parsed),
  );
  assert.equal(
    solanaSccpVoteAccountDataHashFromRawVoteStateV1OrV3(raw, 3n, voteAccountAddress),
    solanaSccpVoteAccountDataHash(parsed),
  );
  const parsedV4 = solanaSccpVoteAccountDataFromRawVoteState(
    sampleSolanaVoteStateV4Account(),
    3n,
    voteAccountAddress,
  );
  assert.deepEqual(Array.from(parsedV4.blockRevenueCollector), Array(32).fill(0x91));
  assert.equal(parsedV4.inflationRewardsCommissionBps, 1_234n);
  assert.equal(parsedV4.blockRevenueCommissionBps, 9_876n);
  assert.equal(parsedV4.pendingDelegatorRewards, 456n);
  assert.deepEqual(Array.from(parsedV4.blsPubkeyCompressed), Array(48).fill(0xa5));
});

test("package dist entrypoint exports Solana raw stake account parser helpers", () => {
  const raw = sampleSolanaStakeStateV2StakeAccount();
  const parsed = solanaSccpStakeAccountDataFromRawStakeStateV2(raw);
  assert.deepEqual(Array.from(parsed.voterPubkey), Array(32).fill(0xa1));
  assert.equal(parsed.delegatedStake, 1_000n);
  assert.equal(
    solanaSccpStakeAccountDataHashFromRawStakeStateV2(raw),
    solanaSccpStakeAccountDataHash(parsed),
  );
  const hiddenPadding = raw.slice();
  hiddenPadding[197] = 1;
  assert.throws(
    () => solanaSccpStakeAccountDataFromRawStakeStateV2(hiddenPadding),
    /padding/u,
  );
});

test("package dist entrypoint exports Solana stake account state helpers", () => {
  const input = {
    epoch: 3n,
    validatorPublicKeys: [`0x${"11".repeat(32)}`, `0x${"22".repeat(32)}`],
    validatorStakes: [1n, 2n],
    validatorActivationEpochs: [0n, 2n],
    validatorDeactivationEpochs: [(1n << 64n) - 1n, 9n],
    validatorVoteAccountAddresses: [`0x${"33".repeat(32)}`, `0x${"44".repeat(32)}`],
    validatorStakeAccountAddresses: [`0x${"55".repeat(32)}`, `0x${"66".repeat(32)}`],
    validatorVoteAccountHashes: [`0x${"77".repeat(32)}`, `0x${"88".repeat(32)}`],
    validatorStakeAccountHashes: [`0x${"99".repeat(32)}`, `0x${"aa".repeat(32)}`],
  };
  assert.equal(canonicalSolanaSccpStakeAccountStateBytes(input).length, 437);
  assert.equal(
    solanaSccpStakeAccountStateHash(input),
    "0x34f6086dd8c1770770802be17b833ed7c973fdaa002c866c0462c33d6938f5b5",
  );
});

test("package dist entrypoint exports Solana stake history helpers", () => {
  const input = {
    epoch: 3n,
    validatorPublicKeys: [`0x${"11".repeat(32)}`, `0x${"22".repeat(32)}`],
    validatorStakes: [1n, 2n],
    validatorDelegatedStakes: [1n, 3n],
    validatorActivationEpochs: [0n, 2n],
    validatorDeactivationEpochs: [(1n << 64n) - 1n, 9n],
    validatorVoteAccountAddresses: [`0x${"33".repeat(32)}`, `0x${"44".repeat(32)}`],
    validatorStakeAccountAddresses: [`0x${"55".repeat(32)}`, `0x${"66".repeat(32)}`],
    validatorVoteAccountHashes: [`0x${"77".repeat(32)}`, `0x${"88".repeat(32)}`],
    validatorStakeAccountHashes: [`0x${"99".repeat(32)}`, `0x${"aa".repeat(32)}`],
    stakeHistoryEntries: [
      { epoch: 2n, effective: 23n, activating: 3n, deactivating: 0n },
      { epoch: 3n, effective: 3n, activating: 1n, deactivating: 0n },
    ],
  };
  assert.equal(canonicalSolanaSccpStakeHistoryBytes(input).length, 249);
  assert.equal(
    solanaSccpStakeHistoryHash(input),
    "0xd75957eec3cf9f5b88076c8dc18e81c5debd627adfbed7e03e35443bcc4d14b6",
  );
});

test("package dist entrypoint exports Solana StakeHistory sysvar helpers", () => {
  assert.equal(
    SCCP_SOLANA_SYSVAR_PROGRAM_ID,
    "0x06a7d5171875f729c73d93408f216120067ed88c76e08c287fc1946000000000",
  );
  assert.equal(
    SCCP_SOLANA_STAKE_HISTORY_SYSVAR_ID,
    "0x06a7d517193584d0feed9bb3431d13206be544281b57b8566cc5375ff4000000",
  );
  const input = {
    stakeHistoryEntries: [
      { epoch: 2n, effective: 10n, activating: 3n, deactivating: 1n },
      { epoch: 3n, effective: 12n, activating: 0n, deactivating: 0n },
    ],
  };
  const canonical = canonicalSolanaSccpStakeHistorySysvarDataBytes(input);
  const hash = solanaSccpStakeHistorySysvarDataHash(input);
  assert.equal(canonical.length, 72);
  assert.match(hash, /^0x[0-9a-f]{64}$/u);
  assert.equal(solanaSccpStakeHistorySysvarDataHashFromRawData(canonical), hash);
});

test("package dist entrypoint exports Solana tower replay helpers", () => {
  const input = {
    finalizedSlot: 1_296_096n,
    rootedSlot: 1_296_065n,
    parentSlot: 1_296_095n,
    bankForkHash: `0x${"aa".repeat(32)}`,
    towerVoteSlots: Array.from({ length: Number(SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH) }, (_, index) =>
      1_296_066n + BigInt(index),
    ),
  };
  assert.equal(canonicalSolanaSccpTowerReplayBytes(input).length, 573);
  assert.match(solanaSccpTowerReplayHash(input), /^0x[0-9a-f]{64}$/u);
});

test("package dist entrypoint exports Solana bank-fork helpers", () => {
  assert.equal(
    SCCP_SOLANA_MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1,
    "sccp:sol:accounts-db-verifier:accounts-lt-hash-mainnet-beta:v1",
  );
  assert.equal(
    SCCP_SOLANA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1,
    "0x6b4e4106bbb6b343ae1a4a36c9c68756d4454d2167c9b8b2ee3225e39fb0a48b",
  );
  assert.equal(typeof wrapSolanaSccpProofResult, "function");
  const accountsLtHash = `0x${"99".repeat(2048)}`;
  const bankSignatureCount = 8n;
  const parentBankHash = `0x${"33".repeat(32)}`;
  const blockhash = `0x${"55".repeat(32)}`;
  const bankHash = solanaSccpAgaveBankHash({
    parentBankHash,
    bankSignatureCount,
    blockhash,
    accountsLtHash,
  });
  const input = {
    finalizedSlot: 1_296_096n,
    parentSlot: 1_296_095n,
    bankSignatureCount,
    parentBankHash,
    bankHash,
    blockhash,
    accountsLtHash,
    transactionStatusRoot: `0x${"66".repeat(32)}`,
    accountInclusionRoot: `0x${"77".repeat(32)}`,
    accountsLtHashChecksum: solanaSccpAccountsLtHashChecksum(accountsLtHash),
  };
  assert.equal(canonicalSolanaSccpBankForkBytes(input).length, 229);
  assert.equal(
    solanaSccpBankForkHash(input),
    "0x8c496fb25a4499947e454a84f638211a84445748bc5242fbb6fb511edd82e531",
  );
});

test("package dist entrypoint exports TON BoC root helper", () => {
  assert.equal(
    SCCP_TON_MAINNET_SHARD_STATE_VERIFIER_ID_V1,
    "sccp:ton:source-state-verifier:shard-state-light-client-mainnet:v1",
  );
  const boc = Buffer.from("b5ee9c720101020100070001020101000202", "hex");
  const checkedBoc = Buffer.from("b5ee9c724101020100070001020101000202be1c1df5", "hex");
  const prunedBoc = Buffer.from(
    "b5ee9c72010101010026002848010149725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe0001",
    "hex",
  );
  const legacyPrunedProofBoc = Buffer.from(
    "b5ee9c7201010601005f0022012001052201620203284801010bd445eea7213bd88307c204a267aa798c1bacb2ad2d781f6106a8296bc12b6500010103a0c0040004006f284801011d894d2390dbd75b607f99091580d9f1652f34c525e35ba648d4325cc7495d3e0001",
    "hex",
  );
  const merkleProofBoc = Buffer.from(
    "b5ee9c7201010301002d0009460349725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe00010101020102000202",
    "hex",
  );
  const hashmapBoc = Buffer.from(
    "b5ee9c72010109010028000101c001020120020702016203050103a0c004000403090103a0c0060004006f0101de08000403e7",
    "hex",
  );
  const shardAccountsBoc = Buffer.from(
    "b5ee9c72010103010073000101c00101d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e41900000000000000078020000",
    "hex",
  );
  const shardStateProofBoc = Buffer.from(
    "b5ee9c720101060100aa00035b9023afe2ffffff11000000000000000000000000000000000700000001000000000000000000000000000000002001020500000101c00301d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419000000000000000780400000000",
    "hex",
  );
  assert.deepEqual(tonBocRootHashes(boc), [
    "0x49725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe",
  ]);
  assert.equal(
    tonBocSingleRootHash(boc),
    "0x49725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe",
  );
  assert.equal(
    tonBocSingleRootHash(checkedBoc),
    "0x49725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe",
  );
  assert.equal(
    tonBocSingleRootHash(prunedBoc),
    "0xcc9095f882fb62a27bb19ad4aa84e19571a3283988ae40b75e238ad240cf1a96",
  );
  assert.equal(
    tonBocSingleRootHash(legacyPrunedProofBoc),
    "0x9c769b035b601b0ddc098e9b148d9bdab0761c14bfe310ac090962ba1f39739a",
  );
  assert.equal(
    tonBocSingleRootHash(merkleProofBoc),
    "0xe749bc5225cabbe3fa78fc12d74a734c365379bc0d302123dcf7bfa2ee3fbd21",
  );
  assert.equal(
    tonHashmapECellRefValueHash(hashmapBoc, Uint8Array.from([17]), 8),
    "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
  );
  assert.equal(
    tonShardAccountsLastTransactionHash(shardAccountsBoc, Uint8Array.from([17, ...Array(31).fill(0)]), 256),
    "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
  );
  const selectedShardAccount = tonShardAccountsLastTransaction(
    shardAccountsBoc,
    Uint8Array.from([17, ...Array(31).fill(0)]),
    256,
  );
  assert.deepEqual(
    {
      hash: selectedShardAccount?.hash,
      lt: selectedShardAccount?.lt.toString(),
    },
    {
      hash: "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
      lt: "7",
    },
  );
  assert.equal(
    tonShardStateProofRootHash(shardStateProofBoc),
    "0xb77955f1a48e68cb56b9e910603a0acddf1c78d45125d65e272b821faa6fce55",
  );
  assert.equal(
    tonShardStateAccountsRootHash(shardStateProofBoc),
    "0x049a63ecefc78dc0cd468ebf47e0385807d790a2ca8e0dca5cbbeb0714567fd3",
  );
  const shardStateTagOffset = shardStateProofBoc.indexOf(Buffer.from("9023afe2", "hex"));
  assert.notEqual(shardStateTagOffset, -1);
  const basechainCustom = Buffer.from(shardStateProofBoc);
  basechainCustom[shardStateTagOffset + 45] |= 0x40;
  assert.throws(() => tonShardStateAccountsRootHash(basechainCustom), /custom/);
});

test("package dist entrypoint exports SCCP TRON Groth16 helpers", () => {
  const proofBytes = sampleGroth16ProofBytes();
  const destinationBinding = sampleTronDestinationBinding();
  const publicInputs = {
    version: 1,
    message_id: `0x${"11".repeat(32)}`,
    payload_hash: `0x${"22".repeat(32)}`,
    target_domain: SCCP_DOMAIN_TRON,
    commitment_root: `0x${"33".repeat(32)}`,
    finality_height: "19",
    finality_block_hash: `0x${"44".repeat(32)}`,
  };
  const request = buildTronSccpProofRequest({
    public_inputs: publicInputs,
    bundle_bytes: new Uint8Array([5, 6, 7]),
    source_proof_bytes: new Uint8Array([9, 10]),
    source_domain: SCCP_DOMAIN_SORA,
    statement_hash: `0x${"55".repeat(32)}`,
    destination_binding: destinationBinding,
  });

  assert.equal(
    request.requestHash,
    "0x53d48d1d2005df00f1a4060ef9396b4ca2aa8ecc405dee439729c061693a44e5",
  );
  const proofResult = wrapTronSccpProofResult(proofBytes, request);
  assert.equal(proofResult.requestHash, request.requestHash);
  assert.throws(
    () =>
      buildTronSccpSubmission({
        proofResult: null,
        proofBytes,
        publicInputs,
        statementHash: `0x${"55".repeat(32)}`,
        destinationBindingHash: destinationBinding.bindingHash,
      }),
    /proofResult must be a wrapped Groth16 SCCP proof result/,
  );
  assert.throws(
    () =>
      buildTronSccpSubmission({
        proofBytes,
        publicInputs,
        statementHash: `0x${"55".repeat(32)}`,
        destinationBindingHash: destinationBinding.bindingHash,
        sourceProofBytes: new Uint8Array([9, 10]),
      }),
    /sourceProofBytes requires proofResult for request-bound submission/,
  );
  const publicInputWords = sccpMessageTransparentPublicInputAbiWords(publicInputs);
  assert.equal(publicInputWords.length, 6);
  assert.equal(Buffer.from(publicInputWords[2]).toString("hex"), `${"00".repeat(31)}05`);
  const callData = sccpSubmitMessageProofCallData(
    proofBytes,
    publicInputs,
    `0x${"55".repeat(32)}`,
  );
  assert.equal(callData.length, 676);
  assert.equal(Buffer.from(callData.subarray(0, 4)).toString("hex"), "bd57826c");
  const mismatchedProof = Uint8Array.from(proofBytes);
  mismatchedProof.fill(0x44, 3 * 32, 4 * 32);
  assert.throws(
    () =>
      sccpSubmitMessageProofCallData(
        mismatchedProof,
        publicInputs,
        `0x${"55".repeat(32)}`,
      ),
    /proofBytes\.commitmentRoot must match publicInputs\.commitmentRoot/,
  );
  const wrongSourceDomainProof = Uint8Array.from(proofBytes);
  wrongSourceDomainProof.set(abiWord(SCCP_DOMAIN_TRON), 2 * 32);
  assert.throws(
    () =>
      sccpSubmitMessageProofCallData(
        wrongSourceDomainProof,
        publicInputs,
        `0x${"55".repeat(32)}`,
      ),
    /proofBytes\.sourceDomain must match sourceDomain/,
  );
  assert.throws(
    () =>
      sccpSubmitMessageProofCallData(
        proofBytes,
        publicInputs,
        `0x${"55".repeat(32)}`,
        SCCP_DOMAIN_TRON,
      ),
    /sourceDomain must be SORA/,
  );
  assert.deepEqual(
    request.publicSignalWords,
    sccpGroth16Bn254PublicSignalWords({
      publicInputs,
      sourceDomain: SCCP_DOMAIN_SORA,
      statementHash: `0x${"55".repeat(32)}`,
      destinationBindingHash: destinationBinding.bindingHash,
    }),
  );
  assert.notEqual(
    request.requestHash,
    buildTronSccpProofRequest({
      public_inputs: publicInputs,
      bundle_bytes: new Uint8Array([5, 6, 7, 9]),
      source_proof_bytes: new Uint8Array([10]),
      source_domain: SCCP_DOMAIN_SORA,
      statement_hash: `0x${"55".repeat(32)}`,
      destination_binding: destinationBinding,
    }).requestHash,
  );
  assert.throws(
    () =>
      buildTronSccpProofRequest({
        public_inputs: { ...publicInputs, target_domain: SCCP_DOMAIN_BSC },
        bundle_bytes: new Uint8Array([5, 6, 7]),
        source_domain: SCCP_DOMAIN_SORA,
        statement_hash: `0x${"55".repeat(32)}`,
        destination_binding: destinationBinding,
    }),
    /publicInputs\.targetDomain must be TRON/,
  );
  assert.throws(
    () =>
      buildTronSccpProofRequest({
        public_inputs: publicInputs,
        bundle_bytes: new Uint8Array(),
        source_domain: SCCP_DOMAIN_SORA,
        statement_hash: `0x${"55".repeat(32)}`,
        destination_binding: destinationBinding,
      }),
    /bundleBytes must not be empty/,
  );
});

test("package dist entrypoint exports SCCP EVM-family Groth16 helpers", async () => {
  const proofBytes = sampleGroth16ProofBytes();
  const destinationBinding = sampleEvmDestinationBinding();
  const publicInputs = {
    version: 1,
    message_id: `0x${"11".repeat(32)}`,
    payload_hash: `0x${"22".repeat(32)}`,
    target_domain: SCCP_DOMAIN_ETH,
    commitment_root: `0x${"33".repeat(32)}`,
    finality_height: "19",
    finality_block_hash: `0x${"44".repeat(32)}`,
  };
  assert.equal(SCCP_ETH_MAINNET_EVM_CHAIN_ID, 1);
  const nativeArtifactPayloadBytes = (label) => {
    const seed = Buffer.from(`${label}\n`, "utf8");
    const out = Buffer.alloc(256);
    for (let index = 0; index < out.length; index += 1) {
      out[index] = seed[index % seed.length];
    }
    return out;
  };
  const proofArtifactBytes = nativeArtifactPayloadBytes("sccp package proof artifact v1");
  const provingKeyBytes = nativeArtifactPayloadBytes("sccp package proving key v1");
  const verifierKeyBytes = nativeArtifactPayloadBytes("sccp package verifier key v1");
  const implementationBytes = nativeArtifactPayloadBytes(
    "sccp package pure typescript prover artifact v1",
  );
  const proofArtifactHash = sha256Hex(proofArtifactBytes);
  const provingKeyHash = sha256Hex(provingKeyBytes);
  const verifierKeyHash = sha256Hex(verifierKeyBytes);
  const implementationHash = sha256Hex(implementationBytes);
  const ethereumMainnetBinding = ethereumMainnetSccpDestinationBinding({
    verifierAddress: `0x${"11".repeat(20)}`,
    bridgeAddress: `0x${"22".repeat(20)}`,
    verifierCodeHash: `0x${"bb".repeat(32)}`,
    verifierKeyHash,
  });
  assert.equal(SCCP_ETH_MAINNET_NETWORK_ID, ethereumMainnetBinding.networkId);
  const nativeProverBundle = {
    schema: SCCP_NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1,
    bundle_id: SCCP_ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
    domain: SCCP_DOMAIN_ETH,
    chain: "eth",
    proof_backend: SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
    proof_artifact: "artifacts/eth-mainnet/proof-artifact.bin",
    proof_artifact_hash: proofArtifactHash,
    proving_key: "artifacts/eth-mainnet/proving-key.bin",
    proving_key_hash: provingKeyHash,
    verifier_key: "artifacts/eth-mainnet/verifier-key.bin",
    verifier_key_hash: verifierKeyHash,
    destination_binding_hash: ethereumMainnetBinding.bindingHash,
    no_wasm: true,
    remote_prover_required: false,
    browser_implementation: "pure-typescript",
    cross_sdk_fixture_parity_artifact: "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
    native_prover_self_test_artifact: "artifacts/eth-mainnet/native-prover-self-test.json",
    native_sdk_artifacts: Object.entries(
      SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1,
    ).map(([sdk, implementation], index) => ({
      sdk,
      implementation,
      prover_artifact_hash: proofArtifactHash,
      proving_key_hash: provingKeyHash,
      implementation_artifact: `artifacts/eth-mainnet/${sdk}-implementation.bin`,
      implementation_hash: sdk === "javascript"
        ? implementationHash
        : `0x${(index + 1).toString(16).padStart(2, "0").repeat(32)}`,
    })),
    audit_hashes: {
      circuit_security_audit: `0x${"a1".repeat(32)}`,
      native_implementation_audit: `0x${"a2".repeat(32)}`,
      reproducible_build_attestation: `0x${"a3".repeat(32)}`,
      cross_sdk_fixture_parity: `0x${"a4".repeat(32)}`,
      native_prover_self_test: `0x${"a5".repeat(32)}`,
      no_wasm_no_remote_scan: `0x${"a6".repeat(32)}`,
    },
  };
  const publicSignalWords = Array.from(
    { length: 9 },
    (_, index) => `0x${(index + 0x10).toString(16).padStart(2, "0").repeat(32)}`,
  );
  const paritySdkResult = {
    receipt_proof_hash: `0x${"d1".repeat(32)}`,
    source_proof_hash: `0x${"d2".repeat(32)}`,
    destination_binding_hash: ethereumMainnetBinding.bindingHash,
    public_signal_words: publicSignalWords,
    calldata_hash: `0x${"d3".repeat(32)}`,
    torii_submit_payload_hash: `0x${"d4".repeat(32)}`,
  };
  const parityFixture = {
    schema: SCCP_ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1,
    domain: SCCP_DOMAIN_ETH,
    chain: "eth",
    proof_backend: SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
    proof_artifact_hash: proofArtifactHash,
    proving_key_hash: provingKeyHash,
    verifier_key_hash: verifierKeyHash,
    destination_binding_hash: ethereumMainnetBinding.bindingHash,
    ...paritySdkResult,
    sdk_results: Object.fromEntries(
      Object.keys(SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1).map((sdk) => [
        sdk,
        { ...paritySdkResult },
      ]),
    ),
  };
  const parityFixtureBytes = Buffer.from(JSON.stringify(parityFixture), "utf8");
  const parityFixtureHash = sha256Hex(parityFixtureBytes);
  nativeProverBundle.audit_hashes.cross_sdk_fixture_parity = parityFixtureHash;
  const selfTestPublicSignalWords = Array.from(
    { length: 9 },
    (_, index) => `0x${(index + 0x30).toString(16).padStart(2, "0").repeat(32)}`,
  );
  const selfTestSdkResult = {
    request_hash: `0x${"e1".repeat(32)}`,
    witness_hash: `0x${"e2".repeat(32)}`,
    source_proof_hash: `0x${"e3".repeat(32)}`,
    proof_hash: `0x${"e4".repeat(32)}`,
    public_signal_words: selfTestPublicSignalWords,
    calldata_hash: `0x${"e5".repeat(32)}`,
    torii_submit_payload_hash: `0x${"e6".repeat(32)}`,
  };
  const selfTestFixture = {
    schema: SCCP_ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1,
    domain: SCCP_DOMAIN_ETH,
    chain: "eth",
    proof_backend: SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
    proof_artifact_hash: proofArtifactHash,
    proving_key_hash: provingKeyHash,
    verifier_key_hash: verifierKeyHash,
    destination_binding_hash: ethereumMainnetBinding.bindingHash,
    ...selfTestSdkResult,
    sdk_results: Object.fromEntries(
      Object.keys(SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1).map((sdk) => [
        sdk,
        { ...selfTestSdkResult },
      ]),
    ),
  };
  const selfTestFixtureBytes = Buffer.from(JSON.stringify(selfTestFixture), "utf8");
  const selfTestFixtureHash = sha256Hex(selfTestFixtureBytes);
  nativeProverBundle.audit_hashes.native_prover_self_test = selfTestFixtureHash;
  assert.equal(
    validateEthereumMainnetNativeEvmProverBundle(nativeProverBundle, {
      destinationBinding: ethereumMainnetBinding,
    }).browserImplementation,
    "pure-typescript",
  );
  assert.equal(
    parseEthereumMainnetNativeEvmProverBundleManifest(JSON.stringify(nativeProverBundle), {
      destinationBinding: ethereumMainnetBinding,
    }).proofArtifactHash,
    proofArtifactHash,
  );
  assert.equal(
    validateEthereumMainnetNativeEvmProverParityFixture(
      parityFixture,
      nativeProverBundle,
    ).sdkResults.javascript.calldataHash,
    parityFixture.calldata_hash,
  );
  assert.equal(
    parseEthereumMainnetNativeEvmProverParityFixture(
      JSON.stringify(parityFixture),
      nativeProverBundle,
    ).publicSignalWords.length,
    9,
  );
  assert.equal(
    validateEthereumMainnetNativeEvmProverSelfTestFixture(
      selfTestFixture,
      nativeProverBundle,
    ).sdkResults.javascript.proofHash,
    selfTestFixture.proof_hash,
  );
  assert.equal(
    parseEthereumMainnetNativeEvmProverSelfTestFixture(
      JSON.stringify(selfTestFixture),
      nativeProverBundle,
    ).publicSignalWords.length,
    9,
  );
  const verifiedNativeArtifacts = verifyEthereumMainnetNativeEvmProverArtifacts(
    {
      nativeProverBundle: nativeProverBundle,
      proofArtifactBytes,
      provingKeyBytes,
      verifierKeyBytes,
      crossSdkFixtureParityBytes: parityFixtureBytes,
      nativeProverSelfTestBytes: selfTestFixtureBytes,
      sdk: "javascript",
      implementationBytes,
    },
    { destinationBinding: ethereumMainnetBinding },
  );
  assert.equal(
    verifiedNativeArtifacts.hashAlgorithm,
    SCCP_NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1,
  );
  assert.equal(verifiedNativeArtifacts.implementation, "pure-typescript");
  assert.equal(verifiedNativeArtifacts.implementationHash, implementationHash);
  assert.equal(verifiedNativeArtifacts.crossSdkFixtureParityHash, parityFixtureHash);
  assert.equal(verifiedNativeArtifacts.nativeProverSelfTestHash, selfTestFixtureHash);
  assert.equal(
    (await runEthereumMainnetNativeProverSelfTest({
      nativeProverArtifacts: verifiedNativeArtifacts,
      nativeProverSelfTest(context) {
        return context.expectedResult;
      },
    })).proofHash,
    selfTestFixture.proof_hash,
  );
  const nativeArtifactBytes = new Map([
    [nativeProverBundle.proof_artifact, proofArtifactBytes],
    [nativeProverBundle.proving_key, provingKeyBytes],
    [nativeProverBundle.verifier_key, verifierKeyBytes],
    [nativeProverBundle.cross_sdk_fixture_parity_artifact, parityFixtureBytes],
    [nativeProverBundle.native_prover_self_test_artifact, selfTestFixtureBytes],
    [
      nativeProverBundle.native_sdk_artifacts.find((row) => row.sdk === "javascript")
        .implementation_artifact,
      implementationBytes,
    ],
  ]);
  assert.equal(
    (await verifyEthereumMainnetNativeEvmProverArtifactsFromBundle(
      {
        nativeProverBundle,
        sdk: "javascript",
        artifactResolver(path) {
          return nativeArtifactBytes.get(path);
        },
      },
      { destinationBinding: ethereumMainnetBinding },
    )).implementationHash,
    implementationHash,
  );
  let factoryRequest;
  const factorySdk = await EthereumMainnetSccp.fromNativeProverBundle({
    destinationBinding: ethereumMainnetBinding,
    manifest: JSON.stringify(nativeProverBundle),
    sdk: "javascript",
    artifactResolver(path) {
      return nativeArtifactBytes.get(path);
    },
    nativeProverSelfTest(context) {
      return context.expectedResult;
    },
    outboundProver: {
      async prove(request) {
        factoryRequest = request;
        return wrapEvmSccpProofResult(proofBytes, request);
      },
    },
  });
  assert.equal((await factorySdk.runNativeProverSelfTest()).calldataHash, selfTestFixture.calldata_hash);
  const factoryResult = await factorySdk.proveOutboundToEthereum({
    public_inputs: publicInputs,
    bundle_bytes: new Uint8Array([5, 6, 7]),
    source_domain: SCCP_DOMAIN_SORA,
    statement_hash: `0x${"55".repeat(32)}`,
    destination_binding: ethereumMainnetBinding,
  });
  assert.equal(factoryRequest.proofArtifactHash, proofArtifactHash);
  assert.equal(factoryRequest.provingKeyHash, provingKeyHash);
  assert.equal(factoryResult.destinationBindingHash, ethereumMainnetBinding.bindingHash);
  const tinyProofArtifactBytes = Buffer.from("tiny native proof artifact\n", "utf8");
  const tinyProofArtifactHash = sha256Hex(tinyProofArtifactBytes);
  const tinyNativeProverBundle = {
    ...nativeProverBundle,
    proof_artifact_hash: tinyProofArtifactHash,
    native_sdk_artifacts: nativeProverBundle.native_sdk_artifacts.map((artifact) => ({
      ...artifact,
      prover_artifact_hash: tinyProofArtifactHash,
    })),
  };
  const tinyParityFixture = {
    ...parityFixture,
    proof_artifact_hash: tinyProofArtifactHash,
  };
  const tinyParityFixtureBytes = Buffer.from(JSON.stringify(tinyParityFixture), "utf8");
  const tinySelfTestFixture = {
    ...selfTestFixture,
    proof_artifact_hash: tinyProofArtifactHash,
  };
  const tinySelfTestFixtureBytes = Buffer.from(JSON.stringify(tinySelfTestFixture), "utf8");
  tinyNativeProverBundle.audit_hashes = {
    ...nativeProverBundle.audit_hashes,
    cross_sdk_fixture_parity: sha256Hex(tinyParityFixtureBytes),
    native_prover_self_test: sha256Hex(tinySelfTestFixtureBytes),
  };
  assert.throws(
    () =>
      verifyEthereumMainnetNativeEvmProverArtifacts(
        {
          nativeProverBundle: tinyNativeProverBundle,
          proofArtifactBytes: tinyProofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          crossSdkFixtureParityBytes: tinyParityFixtureBytes,
          nativeProverSelfTestBytes: tinySelfTestFixtureBytes,
          sdk: "javascript",
          implementationBytes,
        },
        { destinationBinding: ethereumMainnetBinding },
      ),
    /proofArtifactBytes must be at least 256 bytes/u,
  );
  assert.throws(
    () =>
      verifyEthereumMainnetNativeEvmProverArtifacts(
        {
          nativeProverBundle: nativeProverBundle,
          proofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          crossSdkFixtureParityBytes: parityFixtureBytes,
          nativeProverSelfTestBytes: selfTestFixtureBytes,
          sdk: "javascript",
          implementationBytes: Buffer.from("tampered", "utf8"),
        },
        { destinationBinding: ethereumMainnetBinding },
      ),
    /implementationBytes sha256/u,
  );
  assert.equal(new EthereumMainnetSccp().buildOutboundProofRequest({
    public_inputs: publicInputs,
    bundle_bytes: new Uint8Array([5, 6, 7]),
    source_domain: SCCP_DOMAIN_SORA,
    statement_hash: `0x${"55".repeat(32)}`,
    destination_binding: ethereumMainnetBinding,
    native_prover_bundle: nativeProverBundle,
  }).proofArtifactHash, proofArtifactHash);
  assert.equal(new EthereumMainnetSccp().buildOutboundProofRequest({
    public_inputs: publicInputs,
    bundle_bytes: new Uint8Array([5, 6, 7]),
    source_domain: SCCP_DOMAIN_SORA,
    statement_hash: `0x${"55".repeat(32)}`,
    destination_binding: ethereumMainnetBinding,
  }).targetDomain, SCCP_DOMAIN_ETH);
  assert.throws(
    () =>
      new EthereumMainnetSccp().buildOutboundProofRequest({
        public_inputs: { ...publicInputs, target_domain: SCCP_DOMAIN_BSC },
        bundle_bytes: new Uint8Array([5, 6, 7]),
        source_domain: SCCP_DOMAIN_SORA,
        statement_hash: `0x${"55".repeat(32)}`,
        destination_binding: ethereumMainnetBinding,
      }),
    /destinationBinding|targetDomain|Ethereum mainnet/u,
  );
  assert.match(DECLARATIONS_TEXT, /export class EthereumMainnetSccp/u);
  const bscMainnetBinding = bscMainnetSccpDestinationBinding({
    verifierAddress: `0x${"11".repeat(20)}`,
    bridgeAddress: `0x${"22".repeat(20)}`,
    verifierCodeHash: `0x${"bb".repeat(32)}`,
    verifierKeyHash: `0x${"cc".repeat(32)}`,
  });
  const bscPublicInputs = { ...publicInputs, target_domain: SCCP_DOMAIN_BSC };
  assert.equal(SCCP_BSC_MAINNET_EVM_CHAIN_ID, 56);
  assert.equal(SCCP_BSC_MAINNET_NETWORK_ID, bscMainnetBinding.networkId);
  const bscRequest = buildBscMainnetSccpDestinationProofRequest({
    public_inputs: bscPublicInputs,
    bundle_bytes: new Uint8Array([5, 6, 7]),
    source_proof_bytes: new Uint8Array([9, 10]),
    source_domain: SCCP_DOMAIN_SORA,
    statement_hash: `0x${"55".repeat(32)}`,
    destination_binding: bscMainnetBinding,
  });
  const bscProofResult = wrapBscMainnetSccpDestinationProofResult(proofBytes, bscRequest);
  assert.equal(
    buildBscMainnetSccpDestinationSubmission({ proofResult: bscProofResult }).targetDomain,
    SCCP_DOMAIN_BSC,
  );
  assert.equal(new BscMainnetSccp().buildBscCalldata({ proofResult: bscProofResult }).targetDomain, SCCP_DOMAIN_BSC);
  assert.equal((await new BscMainnetSccpProver().buildRequest({
    public_inputs: bscPublicInputs,
    bundle_bytes: new Uint8Array([5, 6, 7]),
    source_domain: SCCP_DOMAIN_SORA,
    statement_hash: `0x${"55".repeat(32)}`,
    destination_binding: bscMainnetBinding,
  })).targetDomain, SCCP_DOMAIN_BSC);
  assert.match(DECLARATIONS_TEXT, /export class BscMainnetSccp/u);
  assert.match(DECLARATIONS_TEXT, /export class BscMainnetSccpProver/u);
  const bscTestnetBinding = bscTestnetSccpDestinationBinding({
    verifierAddress: `0x${"33".repeat(20)}`,
    bridgeAddress: `0x${"44".repeat(20)}`,
    verifierCodeHash: `0x${"dd".repeat(32)}`,
    verifierKeyHash: `0x${"ee".repeat(32)}`,
  });
  assert.equal(SCCP_BSC_TESTNET_EVM_CHAIN_ID, 97);
  assert.equal(SCCP_BSC_TESTNET_NETWORK_ID, bscTestnetBinding.networkId);
  const bscTestnetRequest = buildBscTestnetSccpDestinationProofRequest({
    public_inputs: bscPublicInputs,
    bundle_bytes: new Uint8Array([5, 6, 7]),
    source_proof_bytes: new Uint8Array([9, 10]),
    source_domain: SCCP_DOMAIN_SORA,
    statement_hash: `0x${"55".repeat(32)}`,
    destination_binding: bscTestnetBinding,
  });
  const bscTestnetProofResult = wrapBscTestnetSccpDestinationProofResult(proofBytes, bscTestnetRequest);
  assert.equal(
    buildBscTestnetSccpDestinationSubmission({ proofResult: bscTestnetProofResult }).targetDomain,
    SCCP_DOMAIN_BSC,
  );
  assert.equal(new BscTestnetSccp().buildBscCalldata({
    proofResult: bscTestnetProofResult,
  }).destinationBindingHash, bscTestnetRequest.destinationBindingHash);
  assert.equal((await new BscTestnetSccpProver().buildRequest({
    public_inputs: bscPublicInputs,
    bundle_bytes: new Uint8Array([5, 6, 7]),
    source_domain: SCCP_DOMAIN_SORA,
    statement_hash: `0x${"55".repeat(32)}`,
    destination_binding: bscTestnetBinding,
  })).destinationBinding.networkId, SCCP_BSC_TESTNET_NETWORK_ID);
  assert.equal(buildBscTestnetSccpLocalAdmissionSubmission({
    source_domain: SCCP_DOMAIN_BSC,
    target_domain: SCCP_DOMAIN_SORA,
    proof_bytes: new Uint8Array([1, 2, 3]),
    public_inputs_bytes: new Uint8Array([4, 5, 6]),
    bundle_bytes: new Uint8Array([7, 8, 9]),
    envelope_bytes: new Uint8Array([10, 11, 12]),
    statement_hash: `0x${"66".repeat(32)}`,
    source_verifier_material_hash: `0x${"77".repeat(32)}`,
    source_adapter_engine_deployment_hash: `0x${"88".repeat(32)}`,
  }).sourceDomain, SCCP_DOMAIN_BSC);
  assert.match(DECLARATIONS_TEXT, /export type BscTestnetSccpProofRequest/u);
  assert.match(DECLARATIONS_TEXT, /export class BscTestnetSccp/u);
  assert.match(DECLARATIONS_TEXT, /export class BscTestnetSccpProver/u);
  assert.match(DECLARATIONS_TEXT, /buildBscTestnetSccpDestinationSubmission/u);
  assert.match(DECLARATIONS_TEXT, /buildBscTestnetSccpLocalAdmissionSubmission/u);
  const request = buildEvmSccpProofRequest({
    public_inputs: publicInputs,
    bundle_bytes: new Uint8Array([5, 6, 7]),
    source_proof_bytes: new Uint8Array([9, 10]),
    source_domain: SCCP_DOMAIN_SORA,
    statement_hash: `0x${"55".repeat(32)}`,
    destination_binding: destinationBinding,
  });

  assert.equal(
    request.requestHash,
    "0x4a7c71c3c1838f5d30e1641a32984999a71f9c6cfdff9151ac7d77ca60b64d5e",
  );
  const artifactRequest = buildEvmSccpProofRequest({
    public_inputs: publicInputs,
    bundle_bytes: new Uint8Array([5, 6, 7]),
    source_proof_bytes: new Uint8Array([9, 10]),
    source_domain: SCCP_DOMAIN_SORA,
    statement_hash: `0x${"55".repeat(32)}`,
    destination_binding: destinationBinding,
    proof_artifact_hash: `0x${"91".repeat(32)}`,
    proving_key_hash: `0x${"92".repeat(32)}`,
  });
  assert.equal(artifactRequest.proofArtifactHash, `0x${"91".repeat(32)}`);
  assert.equal(artifactRequest.provingKeyHash, `0x${"92".repeat(32)}`);
  assert.notEqual(artifactRequest.requestHash, request.requestHash);
  const proofResult = wrapEvmSccpProofResult(proofBytes, request);
  assert.equal(proofResult.requestHash, request.requestHash);
  const artifactProofResult = wrapEvmSccpProofResult(proofBytes, artifactRequest);
  assert.equal(artifactProofResult.proofArtifactHash, artifactRequest.proofArtifactHash);
  assert.equal(artifactProofResult.provingKeyHash, artifactRequest.provingKeyHash);
  assert.throws(
    () =>
      buildEvmSccpSubmission({
        proofResult: null,
        proofBytes,
        publicInputs,
        statementHash: `0x${"55".repeat(32)}`,
        destinationBindingHash: destinationBinding.bindingHash,
      }),
    /proofResult must be a wrapped Groth16 SCCP proof result/,
  );
  assert.throws(
    () =>
      buildEvmSccpSubmission({
        proofBytes,
        publicInputs,
        statementHash: `0x${"55".repeat(32)}`,
        destinationBindingHash: destinationBinding.bindingHash,
        bundleBytes: new Uint8Array([5, 6, 7]),
      }),
    /bundleBytes requires proofResult for request-bound submission/,
  );
  const publicInputWords = sccpMessageTransparentPublicInputAbiWords(publicInputs);
  assert.equal(publicInputWords.length, 6);
  assert.equal(Buffer.from(publicInputWords[2]).toString("hex"), `${"00".repeat(31)}01`);
  const callData = sccpSubmitMessageProofCallData(
    proofBytes,
    publicInputs,
    `0x${"55".repeat(32)}`,
  );
  assert.equal(callData.length, 676);
  assert.equal(Buffer.from(callData.subarray(0, 4)).toString("hex"), "bd57826c");
  const mismatchedProof = Uint8Array.from(proofBytes);
  mismatchedProof.fill(0x22, 32, 64);
  assert.throws(
    () =>
      sccpSubmitMessageProofCallData(
        mismatchedProof,
        publicInputs,
        `0x${"55".repeat(32)}`,
      ),
    /proofBytes\.messageId must match publicInputs\.messageId/,
  );
  const wrongSourceDomainProof = Uint8Array.from(proofBytes);
  wrongSourceDomainProof.set(abiWord(SCCP_DOMAIN_ETH), 2 * 32);
  assert.throws(
    () =>
      sccpSubmitMessageProofCallData(
        wrongSourceDomainProof,
        publicInputs,
        `0x${"55".repeat(32)}`,
      ),
    /proofBytes\.sourceDomain must match sourceDomain/,
  );
  assert.throws(
    () =>
      sccpSubmitMessageProofCallData(
        proofBytes,
        publicInputs,
        `0x${"55".repeat(32)}`,
        SCCP_DOMAIN_ETH,
      ),
    /sourceDomain must be SORA/,
  );
  assert.notEqual(
    request.requestHash,
    buildEvmSccpProofRequest({
      public_inputs: publicInputs,
      bundle_bytes: new Uint8Array([5, 6, 7, 9]),
      source_proof_bytes: new Uint8Array([10]),
      source_domain: SCCP_DOMAIN_SORA,
      statement_hash: `0x${"55".repeat(32)}`,
      destination_binding: destinationBinding,
    }).requestHash,
  );
  assert.deepEqual(
    request.publicSignalWords,
    sccpGroth16Bn254PublicSignalWords({
      publicInputs,
      sourceDomain: SCCP_DOMAIN_SORA,
      statementHash: `0x${"55".repeat(32)}`,
      destinationBindingHash: destinationBinding.bindingHash,
    }),
  );
  assert.throws(
    () =>
      buildEvmSccpProofRequest({
        public_inputs: publicInputs,
        bundle_bytes: new Uint8Array(),
        source_domain: SCCP_DOMAIN_SORA,
        statement_hash: `0x${"55".repeat(32)}`,
        destination_binding: destinationBinding,
      }),
    /bundleBytes must not be empty/,
  );
});

test("package dist entrypoint exports SCCP Substrate runtime proof helpers", async () => {
  const publicInputs = {
    version: 1,
    message_id: `0x${"11".repeat(32)}`,
    payload_hash: `0x${"22".repeat(32)}`,
    target_domain: SCCP_DOMAIN_SORA_KUSAMA,
    commitment_root: `0x${"33".repeat(32)}`,
    finality_height: "19",
    finality_block_hash: `0x${"44".repeat(32)}`,
  };
  const input = {
    public_inputs: publicInputs,
    bundle_bytes: new Uint8Array([5, 6, 7]),
    source_proof_bytes: new Uint8Array([9, 10]),
    source_domain: SCCP_DOMAIN_SORA,
    statement_hash: `0x${"55".repeat(32)}`,
    destination_binding_hash: `0x${"66".repeat(32)}`,
  };
  const request = buildSubstrateSccpProofRequest(input);

  assert.equal(request.backend, SCCP_SUBSTRATE_RUNTIME_PROOF_BACKEND_V1);
  assert.equal(request.targetDomain, SCCP_DOMAIN_SORA_KUSAMA);
  assert.match(request.requestHash, /^0x[0-9a-f]{64}$/u);
  assert.equal(Object.isFrozen(request), true);
  const exposedBundleBytes = request.bundleBytes;
  exposedBundleBytes[0] = 99;
  assert.equal(request.bundleBytes[0], 5);
  assert.notEqual(
    request.requestHash,
    buildSubstrateSccpProofRequest({
      ...input,
      bundle_bytes: new Uint8Array([5, 6, 7, 9]),
    }).requestHash,
  );
  assert.throws(
    () =>
      buildSubstrateSccpProofRequest({
        ...input,
        bundle_bytes: new Uint8Array([0, 0]),
      }),
    /bundleBytes must not be all zero/,
  );
  assert.throws(
    () =>
      buildSubstrateSccpProofRequest({
        ...input,
        bundle_bytes: new Uint8Array(SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1).fill(1),
      }),
    /bundleBytes must be at most/,
  );

  const prover = new SubstrateSccpProver({
    prove: (callbackRequest) => {
      assert.equal(callbackRequest.requestHash, request.requestHash);
      return { proofBytes: new Uint8Array([1, 2, 3]) };
    },
  });
  const result = await prover.prove(input);
  assert.equal(result.requestHash, request.requestHash);
  const submission = buildSubstrateSccpSubmission({ proofResult: result });
  assert.equal(submission.requestHash, request.requestHash);
  assert.equal(submission.envelopeEncoding, SCCP_SUBSTRATE_RUNTIME_CALL_SCALE_V1);
  assert.equal(wrapSubstrateSccpProofResult([1, 2, 3], request).requestHash, request.requestHash);
  assert.throws(
    () =>
      buildSubstrateSccpSubmission({
        proofResult: null,
        proofBytes: new Uint8Array([1, 2, 3]),
        publicInputs,
        bundleBytes: new Uint8Array([5, 6, 7]),
        sourceProofBytes: new Uint8Array([9, 10]),
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: `0x${"55".repeat(32)}`,
        destinationBindingHash: `0x${"66".repeat(32)}`,
      }),
    /proofResult must be a wrapped Substrate SCCP proof result/,
  );
  assert.throws(
    () =>
      buildSubstrateSccpSubmission({
        proofBytes: new Uint8Array([1, 2, 3]),
        publicInputs,
        bundleBytes: new Uint8Array([5, 6, 7]),
        sourceProofBytes: new Uint8Array([9, 10]),
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: `0x${"55".repeat(32)}`,
        destinationBindingHash: `0x${"66".repeat(32)}`,
      }),
    /sourceProofBytes requires proofResult for request-bound submission/,
  );
  assert.throws(
    () =>
      wrapSubstrateSccpProofResult(
        new Uint8Array(SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1).fill(1),
        request,
    ),
    /at most/,
  );
  assert.throws(
    () =>
      buildSubstrateSccpSubmission({
        publicInputs,
        proofBytes: new Uint8Array(SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1).fill(1),
        bundleBytes: new Uint8Array([5, 6, 7]),
        sourceProofBytes: new Uint8Array(),
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: `0x${"55".repeat(32)}`,
        destinationBindingHash: `0x${"66".repeat(32)}`,
      }),
    /at most/,
  );
  assert.throws(
    () =>
      buildSubstrateSccpSubmission({
        publicInputs,
        proofBytes: new Uint8Array([1]),
        bundleBytes: new Uint8Array([0, 0]),
        sourceProofBytes: new Uint8Array([9, 10]),
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: `0x${"55".repeat(32)}`,
        destinationBindingHash: `0x${"66".repeat(32)}`,
      }),
    /bundleBytes must not be all zero/,
  );
  assert.match(result.envelopeHash, /^0x[0-9a-f]{64}$/u);
  const exposedProofBytes = result.proofBytes;
  exposedProofBytes[0] = 99;
  assert.equal(result.proofBytes[0], 1);
});

test("package dist entrypoint exports SCCP TON proof wrapper", () => {
  const publicInputs = {
    version: 1,
    message_id: `0x${"11".repeat(32)}`,
    payload_hash: `0x${"22".repeat(32)}`,
    target_domain: SCCP_DOMAIN_TON,
    commitment_root: `0x${"33".repeat(32)}`,
    finality_height: "19",
    finality_block_hash: `0x${"44".repeat(32)}`,
  };
  const request = buildTonSccpProofRequest({
    public_inputs: publicInputs,
    bundle_bytes: new Uint8Array([5, 6, 7]),
    source_proof_bytes: new Uint8Array([9, 10]),
    statement_hash: `0x${"55".repeat(32)}`,
    destination_binding_hash: `0x${"66".repeat(32)}`,
    source_state_verifier_hash: `0x${"77".repeat(32)}`,
    source_adapter_deployment_hash: `0x${"88".repeat(32)}`,
    source_adapter_deployment_receipt_hash: `0x${"99".repeat(32)}`,
  });
  const result = wrapTonSccpProofResult([1, 2, 3], request);
  const submission = buildTonSccpSubmission({
    proofResult: result,
    bundleBytes: new Uint8Array([5, 6, 7]),
  });

  assert.equal(request.targetDomain, SCCP_DOMAIN_TON);
  assert.equal(result.requestHash, request.requestHash);
  assert.equal(submission.envelopeEncoding, SCCP_TON_MESSAGE_BODY_BOC_V1);
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult: result,
        bundleBytes: new Uint8Array([0, 0]),
      }),
    /bundleBytes must not be all zero/,
  );
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult: result,
        bundleBytes: new Uint8Array(SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1).fill(1),
      }),
    /bundleBytes must be at most/,
  );
  assert.throws(
    () =>
      wrapTonSccpProofResult(
        new Uint8Array(SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1).fill(1),
        request,
    ),
    /at most/,
  );
  assert.throws(
    () =>
      buildTonSccpSubmission({
        publicInputs,
        proofBytes: new Uint8Array(SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1).fill(1),
        bundleBytes: new Uint8Array([5, 6, 7]),
        statementHash: `0x${"55".repeat(32)}`,
        destinationBindingHash: `0x${"66".repeat(32)}`,
      }),
    /proofResult must be a wrapped TON SCCP proof result/,
  );
  assert.match(result.envelopeHash, /^0x[0-9a-f]{64}$/u);
  assert.ok(
    canonicalTonSccpSourceStateVerificationProofBytes({
      circuitId: SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1,
      proofBytes: new Uint8Array([1, 2, 3]),
    }).length > 0,
  );
  assert.throws(
    () =>
      canonicalTonSccpSourceStateVerificationProofBytes({
        circuitId: SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1,
        proofBytes: new Uint8Array([1, 2, 3]),
      }),
    /TON source-state/,
  );
});

test("package dist entrypoint exports SCCP source record helpers", () => {
  const material = {
    sourceDomain: SCCP_DOMAIN_ETH,
    sourceTrustAnchorHash: `0x${"44".repeat(32)}`,
    consensusVerifierHash: `0x${"55".repeat(32)}`,
    messageInclusionVerifierHash: `0x${"66".repeat(32)}`,
    finalityPolicyHash: `0x${"88".repeat(32)}`,
    bridgeAddress: `0x${"11".repeat(20)}`,
    sourceBridgeEmitterCodeHash: `0x${"77".repeat(32)}`,
    networkId: SCCP_ETH_MAINNET_NETWORK_ID,
    configHash: "0x871a910500648c68576f7d8fb044de1c494ae24c74f435c87dd451e6ae169c6b",
  };
  assert.ok(canonicalSccpSourceVerifierMaterialBytes(material).length > 0);
  assert.equal(
    sccpSourceVerifierMaterialHash(material),
    "0x4d1e9d15bc59c0a2157aa967eb033f5778c805aea4707785a31ef6b60f694d77",
  );
  assert.equal(SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1, "sccp-source-adapter-v1");
  assert.equal(SCCP_SOURCE_ADAPTER_FASTPQ_PARAMETER_SET_V1, "fastpq-lane-balanced");
  const sourceAdapterVerifierVkHash = sccpSourceAdapterVerifierVkHash(SCCP_DOMAIN_ETH);
  assert.match(sourceAdapterVerifierVkHash, /^0x[0-9a-f]{64}$/u);
  assert.notEqual(sourceAdapterVerifierVkHash, sccpSourceAdapterVerifierVkHash(SCCP_DOMAIN_BSC));
  assert.throws(
    () =>
      sccpSourceAdapterVerifierVkHash({
        sourceDomain: SCCP_DOMAIN_ETH,
        targetDomain: SCCP_DOMAIN_SOL,
      }),
    /targetDomain must be SORA/,
  );
  const deployment = {
    ...material,
    deploymentReceiptHash: `0x${"aa".repeat(32)}`,
  };
  assert.ok(canonicalSccpSourceAdapterEngineDeploymentBytes(deployment).length > 0);
  assert.equal(
    sccpSourceAdapterEngineDeploymentHash(deployment),
    "0xfeb62925410b1376a2cd3704c3822e335da96c3dcc283b041a559d7b08ab1cc4",
  );
  assert.equal(sccpDestinationBindingKey(SCCP_DOMAIN_SOL), "sccp:0:3:sol:solana-program-v1:2");
  assert.equal(
    sccpDestinationBindingHash(SCCP_DOMAIN_SOL),
    "0x078578f0aa27daa2972d6c19d1d26dbb6bf6ba1e8df84e283d7ef101fc46abf6",
  );
  assert.equal(
    sccpSolanaFullLightClientGateHash({
      sourceDomain: SCCP_DOMAIN_SOL,
      sourceTrustAnchorHash: `0x${"44".repeat(32)}`,
      consensusVerifierHash: `0x${"55".repeat(32)}`,
      messageInclusionVerifierHash: `0x${"66".repeat(32)}`,
      finalityPolicyHash: `0x${"88".repeat(32)}`,
      sourceStateVerifierHash: `0x${"77".repeat(32)}`,
      deploymentReceiptHash: `0x${"aa".repeat(32)}`,
      solanaTowerReplayVerifierHash: `0x${"bb".repeat(32)}`,
      solanaFullAccountsdbLatticeVerifierHash: `0x${"cc".repeat(32)}`,
      solanaBankForkChoiceVerifierHash: `0x${"dd".repeat(32)}`,
    }),
    "0x2c94b86a665bb68708b762c678661f5e9879bd588627e93a640796eeaef970f9",
  );
  assert.throws(
    () =>
      sccpSolanaFullLightClientGateHash({
        sourceDomain: SCCP_DOMAIN_SOL,
        sourceTrustAnchorHash: `0x${"44".repeat(32)}`,
        consensusVerifierHash: `0x${"55".repeat(32)}`,
        messageInclusionVerifierHash: `0x${"66".repeat(32)}`,
        finalityPolicyHash: `0x${"88".repeat(32)}`,
        sourceStateVerifierHash: `0x${"77".repeat(32)}`,
        deploymentReceiptHash: `0x${"aa".repeat(32)}`,
        solanaTowerReplayVerifierHash: `0x${"bb".repeat(32)}`,
        solanaFullAccountsdbLatticeVerifierHash: `0x${"bb".repeat(32)}`,
        solanaBankForkChoiceVerifierHash: `0x${"dd".repeat(32)}`,
      }),
    /role-separated/,
  );
  assert.equal(
    sccpTonFullLightClientGateHash({
      sourceDomain: SCCP_DOMAIN_TON,
      sourceTrustAnchorHash: `0x${"44".repeat(32)}`,
      consensusVerifierHash: `0x${"55".repeat(32)}`,
      messageInclusionVerifierHash: `0x${"66".repeat(32)}`,
      finalityPolicyHash: `0x${"88".repeat(32)}`,
      sourceStateVerifierHash: `0x${"77".repeat(32)}`,
      deploymentReceiptHash: `0x${"aa".repeat(32)}`,
      tonMasterchainConfigVerifierHash: `0x${"bb".repeat(32)}`,
      tonValidatorSetTransitionVerifierHash: `0x${"cc".repeat(32)}`,
      tonShardAccountsDictionaryVerifierHash: `0x${"dd".repeat(32)}`,
    }),
    "0xc32d8cfc2e273646abb00911b9a15e7ee0ab1721b04a6e89a060422dd3cc4596",
  );
});

test("package dist entrypoint exports BSC validator-set payload helpers", () => {
  const payload = canonicalBscValidatorSetPayloadBytes({
    validatorAddresses: [`0x${"11".repeat(20)}`, `0x${"22".repeat(20)}`],
    validatorPowers: [1n, 2n],
  });
  const metadataProof = {
    version: 1,
    stateRoot: `0x${"aa".repeat(32)}`,
    nextValidatorSetPayloadHash: bscValidatorSetPayloadHash(payload),
    validatorContractAddress: `0x${"00".repeat(18)}1000`,
    accountProofNodes: [`0xf842a0${"11".repeat(32)}`],
    storageRoot: `0x${"bb".repeat(32)}`,
    validatorSetLengthSlot: `0x${"00".repeat(31)}01`,
    validatorSetLengthValue: "0x02",
    validatorSetLengthValueHash: bscValidatorSetStorageValueHash("0x02"),
    validatorSetLengthProofNodes: [`0xe4822080a0${"22".repeat(32)}`],
    validatorStorageProofs: [
      {
        version: 1,
        validatorIndex: 0,
        storageSlot: `0x${"33".repeat(32)}`,
        storageValue: `0x94${"11".repeat(20)}`,
        storageValueHash: bscValidatorSetStorageValueHash(`0x94${"11".repeat(20)}`),
        storageProofNodes: [`0xe4822080a0${"44".repeat(32)}`],
      },
      {
        version: 1,
        validatorIndex: 1,
        storageSlot: `0x${"55".repeat(32)}`,
        storageValue: `0x94${"22".repeat(20)}`,
        storageValueHash: bscValidatorSetStorageValueHash(`0x94${"22".repeat(20)}`),
        storageProofNodes: [`0xe4822080a0${"66".repeat(32)}`],
      },
    ],
  };

  assert.equal(
    Buffer.from(payload).toString("hex"),
    `0102000000${"11".repeat(20)}0100000000000000${"22".repeat(20)}0200000000000000`,
  );
  assert.equal(
    bscValidatorSetPayloadHash(payload),
    "0xdc6190956bc147c9a0a2fbf1384d40a1deb4b211a709f229275d1ea5ac3f8370",
  );
  assert.equal(
    bscValidatorSetHashFromPayload(payload),
    "0x3ef5ecfb6dc4f5fc9e970cc18cd72164495c827e96f77851813973a286f5c762",
  );
  const commitValidatorPublicKeys = [
    "0x0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
    "0x02c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5",
    "0x02f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
    "0x02e493dbf1c10d80f3581e4904930b1404cc6c13900ee0758474fa94abe8c4cd13",
  ];
  const commitMessage = {
    validatorEpoch: 2n,
    blockNumber: 401n,
    blockHash: `0x${"22".repeat(32)}`,
    receiptsRoot: `0x${"33".repeat(32)}`,
    validatorSetHash: "0xc5152802f6ca9ec72a4249646aca7476496f00b71ab5b1482c881a31fb42dd8c",
  };
  assert.equal(canonicalBscCommitMessageBytes(commitMessage).length, 117);
  assert.equal(
    bscCommitMessageHash(commitMessage),
    "0x5832165d1a87ed49a323f2ecaecbef973489aed1a42e7eab369244e7abec43c7",
  );
  const commitSeal = {
    totalPower: 4n,
    signedPower: 3n,
    commitMessageHash: bscCommitMessageHash(commitMessage),
    validatorPublicKeys: commitValidatorPublicKeys,
    validatorPowers: [1n, 1n, 1n, 1n],
    signersBitmap: "0x07",
    signatures: [
      "0x1b8802069b82c3d4cb6d7bec82323853f36d965c1e71647560084e7c7a0de9c17c85fcc3c6222f905cbbc4ba5b5f3f005f07d144304184181be67b3d02d1ba9f00",
      "0x921d39c29fb793c496f96cf647128232d228024ed2f3e68cc6a52aa4cf64facf6bbd9dfcf7d703165f7880e7e1310f34d1b0fb8ca6dd8f506bf289ba012387f001",
      "0xcfa11aa1ec214278afdb4ef7f3c40af97a2784e0336afb5ebef345c0d2eaa9ef629ad2d25cf9709eb9b842fb2fb3f749ce365af97af6e7064771614312d3619600",
    ],
    validatorSetHash: commitMessage.validatorSetHash,
  };
  assert.equal(canonicalBscCommitSealBytes(commitSeal).length, 297);
  assert.equal(
    bscCommitSealHash(commitSeal),
    "0xcd9d87b24d8c1cf7615cb4267cde5a3fc24bbb770807134ee75d4ddaba992172",
  );
  assert.equal(typeof bscValidatorSetPayloadFromParliaExtra, "function");
  assert.equal(typeof bscValidatorSetPayloadFromHeaderRlp, "function");
  assert.equal(canonicalBscValidatorSetMetadataProofBytes(metadataProof).length, 560);
  assert.match(bscValidatorSetMetadataProofHash(metadataProof), /^0x[0-9a-f]{64}$/);
  assert.equal(
    canonicalBscValidatorSetTransitionMessageBytes({
      sourceDomain: SCCP_DOMAIN_BSC,
      fromValidatorEpoch: 1n,
      toValidatorEpoch: 2n,
      transitionBlockNumber: 400n,
      transitionBlockHash: `0x${"77".repeat(32)}`,
      parentValidatorSetHash: `0x${"88".repeat(32)}`,
      nextValidatorSetHash: bscValidatorSetHashFromPayload(payload),
      nextValidatorSetPayloadHash: bscValidatorSetPayloadHash(payload),
      validatorSetMetadataProofHash: bscValidatorSetMetadataProofHash(metadataProof),
    }).length,
    189,
  );
  assert.match(
    bscValidatorSetTransitionMessageHash({
      sourceDomain: SCCP_DOMAIN_BSC,
      fromValidatorEpoch: 1n,
      toValidatorEpoch: 2n,
      transitionBlockNumber: 400n,
      transitionBlockHash: `0x${"77".repeat(32)}`,
      parentValidatorSetHash: `0x${"88".repeat(32)}`,
      nextValidatorSetHash: bscValidatorSetHashFromPayload(payload),
      nextValidatorSetPayloadHash: bscValidatorSetPayloadHash(payload),
      validatorSetMetadataProofHash: bscValidatorSetMetadataProofHash(metadataProof),
    }),
    /^0x[0-9a-f]{64}$/,
  );
});

test("package dist entrypoint exports ETH sync-committee payload helpers", () => {
  const syncCommitteePublicKeys = Array.from({ length: 512 }, (_, index) => {
    const publicKey = new Uint8Array(48).fill(0x33);
    publicKey[46] = (index >> 8) & 0xff;
    publicKey[47] = index & 0xff;
    return publicKey;
  });
  const syncCommitteePops = Array.from({ length: 512 }, (_, index) => {
    const pop = new Uint8Array(96).fill(0xcc);
    pop[94] = (index >> 8) & 0xff;
    pop[95] = index & 0xff;
    return pop;
  });
  const payload = canonicalEthSyncCommitteePayloadBytes({
    syncCommitteePublicKeys,
    syncCommitteeWeights: Array.from({ length: 512 }, () => 1n),
    syncCommitteePops,
  });

  assert.equal(payload.length, 81925);
  assert.match(ethSyncCommitteeHashFromPayload(payload), /^0x[0-9a-f]{64}$/u);
  assert.match(ethSyncCommitteePayloadHash(payload), /^0x[0-9a-f]{64}$/u);
  assert.equal(SCCP_ETH_MAINNET_SLOTS_PER_SYNC_COMMITTEE_PERIOD, 8192);
  assert.equal(ethMainnetSyncCommitteePeriodForSlot(19n), 0n);
  assert.equal(ethMainnetSyncCommitteePeriodForSlot(8192n), 1n);
});

test("package dist entrypoint exports ETH beacon execution-payload SSZ helpers", () => {
  const headerRlp = sampleEthExecutionHeaderRlp();
  const executionPayloadRoot = ethExecutionPayloadHeaderRootFromRlp(headerRlp);
  const beaconBodyRoot = ethBeaconBodyRootFromExecutionPayloadBranch(executionPayloadRoot, [
    `0x${"ee".repeat(32)}`,
    `0x${"ff".repeat(32)}`,
    `0x${"11".repeat(32)}`,
    `0x${"22".repeat(32)}`,
  ]);

  assert.equal(
    executionPayloadRoot,
    "0xc029dda492d2e41ad72bd83f1727a67e5331f413ec29d5c31de955d0bea24624",
  );
  assert.equal(
    beaconBodyRoot,
    "0x431e6bef5e759e8fdf32d8e8ed1ff761933ddb4de24ec9ae8e2aa0d25fe861ba",
  );
  assert.equal(
    ethBeaconBlockHeaderRoot({
      beaconSlot: 320n,
      beaconProposerIndex: 17n,
      beaconParentRoot: `0x${"aa".repeat(32)}`,
      beaconStateRoot: `0x${"bb".repeat(32)}`,
      beaconBodyRoot,
    }),
    "0xd54b406debae26e6ebaef512cc4f9e6bc12cf02af0d4476895383b37f682a179",
  );
});

test("package dist entrypoint exports TON validator-set transition helpers", () => {
  const validatorSet = {
    validatorPublicKeys: [`0x${"11".repeat(32)}`, `0x${"22".repeat(32)}`],
    validatorWeights: [1n, 2n],
  };
  const nextValidatorSetPayload = canonicalTonValidatorSetPayloadBytes({
    validatorPublicKeys: [`0x${"33".repeat(32)}`, `0x${"44".repeat(32)}`],
    validatorWeights: [3n, 4n],
  });
  const parentValidatorSetHash = tonValidatorSetHash(validatorSet);
  const message = {
    sourceDomain: 4,
    fromValidatorSetSeqno: 7n,
    toValidatorSetSeqno: 8n,
    masterchainSeqno: 19n,
    masterchainWorkchainId: -1,
    masterchainShard: 0x8000000000000000n,
    masterchainBlockHash: `0x${"aa".repeat(32)}`,
    masterchainFileHash: `0x${"a5".repeat(32)}`,
    parentValidatorSetHash,
    nextValidatorSetHash: tonValidatorSetHashFromPayload(nextValidatorSetPayload),
    nextValidatorSetPayload,
    nextValidatorSetPayloadHash: tonValidatorSetPayloadHash(nextValidatorSetPayload),
    nextValidatorSetConfigHash: `0x${"cc".repeat(32)}`,
  };

  assert.equal(
    parentValidatorSetHash,
    "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938",
  );
  assert.equal(
    Buffer.from(nextValidatorSetPayload).toString("hex"),
    `0102000000${"33".repeat(32)}0300000000000000${"44".repeat(32)}0400000000000000`,
  );
  assert.equal(canonicalTonValidatorSetTransitionMessageBytes(message).length, 233);
  assert.equal(
    tonValidatorSetTransitionMessageHash(message),
    "0x91eda926884eb1ae700e7b398c46f6d47fbb973efa322564894936140ccd2a19",
  );
});

test("package dist entrypoint exports TON masterchain config proof helpers", () => {
  const validatorSetPayload = canonicalTonValidatorSetPayloadBytes({
    validatorPublicKeys: [`0x${"11".repeat(32)}`, `0x${"22".repeat(32)}`],
    validatorWeights: [1n, 2n],
  });
  const validatorSetPayloadHash = tonValidatorSetPayloadHash(validatorSetPayload);
  const configDictionaryProofBoc = Buffer.from(
    "b5ee9c72010106010091000101c00101117fffffff80000008a002012b120000000100000002000200020000000000000003c00302087fff00000405005b14e3a049e28444444444444444444444444444444444444444444444444444444444444444400000000000000060005b14e3a049e288888888888888888888888888888888888888888888888888888888888888888000000000000000a0",
    "hex",
  );
  const leaf = {
    sourceDomain: 4,
    masterchainSeqno: 19n,
    masterchainWorkchainId: -1,
    masterchainShard: 0x8000000000000000n,
    masterchainBlockHash: `0x${"aa".repeat(32)}`,
    masterchainFileHash: `0x${"a5".repeat(32)}`,
    shardStateRoot: `0x${"cc".repeat(32)}`,
    validatorSetHash: tonValidatorSetHashFromPayload(validatorSetPayload),
    validatorSetPayloadHash,
  };
  const configLeafHash = tonMasterchainConfigLeafHash(leaf);
  const proof = {
    ...leaf,
    configRoot: "0x5bf87008e0e76085d6db977b53a89329de49a4eed8fd1ff90d8c78f096ef05af",
    configLeafHash,
    configLeafIndex: SCCP_TON_CURRENT_VALIDATOR_SET_CONFIG_PARAM,
    configValueHash: "0x1aa64eb5ca0b3cb254dfada709904ce81f8b327eed0d83f2522122a0a9dddd50",
    configDictionaryProofBoc,
    configInclusionBranch: [],
  };

  assert.equal(canonicalTonMasterchainConfigLeafBytes(leaf).length, 141);
  assert.equal(tonHashmapEProofRootHash(configDictionaryProofBoc), proof.configRoot);
  assert.deepEqual(
    tonConfigValidatorSetPayloadFromProofBoc(configDictionaryProofBoc),
    validatorSetPayload,
  );
  assert.equal(
    tonConfigValidatorSetPayloadHashFromProofBoc(configDictionaryProofBoc),
    validatorSetPayloadHash,
  );
  assert.equal(
    configLeafHash,
    "0xed92ba8082850092da7cc296a2184cc4576877aaee08c72748d96ea449b16e39",
  );
  assert.equal(canonicalTonMasterchainConfigProofBytes(proof).length, 411);
  assert.equal(
    tonMasterchainConfigProofHash(proof),
    "0x9949285613a9e9dfb4ed3728bbede7ddea36fd82ac3d7eff3955dd75e9c4941c",
  );
});

test("package dist entrypoint exports TON masterchain signature helpers", () => {
  const validatorSet = {
    validatorPublicKeys: [`0x${"11".repeat(32)}`, `0x${"22".repeat(32)}`],
    validatorWeights: [1n, 2n],
  };
  const blockMessage = {
    sourceDomain: 4,
    masterchainSeqno: 19n,
    masterchainWorkchainId: -1,
    masterchainShard: 0x8000000000000000n,
    masterchainBlockHash: `0x${"aa".repeat(32)}`,
    masterchainFileHash: `0x${"a5".repeat(32)}`,
    validatorSetHash: tonValidatorSetHash(validatorSet),
    masterchainConfigRoot: "0x5bf87008e0e76085d6db977b53a89329de49a4eed8fd1ff90d8c78f096ef05af",
    masterchainConfigProofHash: "0x9949285613a9e9dfb4ed3728bbede7ddea36fd82ac3d7eff3955dd75e9c4941c",
    shardWorkchainId: 0,
    shardShard: 0x8000000000000000n,
    shardSeqno: 7n,
    shardBlockHash: `0x${"bb".repeat(32)}`,
    shardFileHash: `0x${"bc".repeat(32)}`,
    shardStateRoot: `0x${"cc".repeat(32)}`,
    transactionRoot: `0x${"dd".repeat(32)}`,
    shardProofHash: `0x${"ee".repeat(32)}`,
  };
  const blockMessageHash = tonMasterchainBlockMessageHash(blockMessage);
  const signatures = {
    version: 1,
    totalWeight: 3n,
    signedWeight: 3n,
    blockMessageHash,
    ...validatorSet,
    validatorSetHash: blockMessage.validatorSetHash,
    signersBitmap: [0x03],
    signatures: [new Uint8Array(64).fill(0xab), new Uint8Array(64).fill(0xcd)],
  };

  assert.equal(canonicalTonMasterchainBlockMessageBytes(blockMessage).length, 365);
  assert.equal(
    blockMessageHash,
    "0x0ca07d5072adb7db3d6a0f831294c7e119c451884aaa1afcbb23e0df0911d8bd",
  );
  assert.equal(canonicalTonMasterchainValidatorSignaturesBytes(signatures).length, 322);
  assert.equal(
    tonMasterchainValidatorSignaturesHash(signatures),
    "0x7a927ad3e689e4f3679fe1d1b8ea1088b914523b0c2da0d6dc0938e5e5cf8d15",
  );
});

test("package dist entrypoint exports Substrate authority-set payload helpers", () => {
  const payload = canonicalSubstrateAuthoritySetPayloadBytes({
    authorityPublicKeys: [`0x${"11".repeat(32)}`, `0x${"22".repeat(32)}`],
    authorityWeights: [1n, 2n],
  });

  assert.equal(
    Buffer.from(payload).toString("hex"),
    `0102000000${"11".repeat(32)}0100000000000000${"22".repeat(32)}0200000000000000`,
  );
  assert.equal(
    substrateAuthoritySetPayloadHash(payload),
    "0xdedc4ebe5f91162a5029cb67f88cdbbf94c2bf2b9d0d373bd3e670321565cc16",
  );
  assert.equal(
    substrateAuthoritySetHashFromPayload(payload),
    "0xde84b8b7a5409c0f2cff1191173d6caa681d902b35e42669106ec6ea3193a117",
  );

  const nextPayload = canonicalSubstrateAuthoritySetPayloadBytes({
    authorityPublicKeys: [`0x${"aa".repeat(32)}`, `0x${"bb".repeat(32)}`, `0x${"cc".repeat(32)}`],
    authorityWeights: [13n, 17n, 19n],
  });
  const message = {
    sourceDomain: 6,
    fromGrandpaSetId: 41n,
    toGrandpaSetId: 42n,
    transitionBlockNumber: 9001n,
    transitionBlockHash: `0x${"44".repeat(32)}`,
    parentAuthoritySetHash: "0xb2efd5d86304ea728a8a9ed4013aab8f3e10c0cf862e859c9cade55e660934ef",
    nextAuthoritySetHash: substrateAuthoritySetHashFromPayload(nextPayload),
    nextAuthoritySetPayloadHash: substrateAuthoritySetPayloadHash(nextPayload),
  };
  assert.equal(canonicalSubstrateAuthoritySetTransitionMessageBytes(message).length, 157);
  assert.equal(
    substrateAuthoritySetTransitionMessageHash(message),
    "0x60589333bf798bf592b2642d0fbac39b4e9305576cd2ebe9dd1f448a97a0596b",
  );
});

test("package dist entrypoint exports Substrate runtime-storage proof request helpers", () => {
  const input = {
    sourceDomain: 6,
    sourceEventDigest: `0x${"34".repeat(32)}`,
    sourceEventLeafIndex: 0n,
    finalizedBlockNumber: 31n,
    grandpaSetId: 32n,
    blockHash: `0x${"aa".repeat(32)}`,
    authoritySetHash: `0x${"cc".repeat(32)}`,
    eventsRoot: `0x${"bb".repeat(32)}`,
    inclusionBranch: [`0x${"ee".repeat(32)}`],
    sourceTrustAnchorHash: `0x${"aa".repeat(32)}`,
    consensusVerifierHash: `0x${"bb".repeat(32)}`,
    messageInclusionVerifierHash: `0x${"cc".repeat(32)}`,
    finalityPolicyHash: `0x${"dd".repeat(32)}`,
    sourceStateVerifierHash: `0x${"12".repeat(32)}`,
  };

  assert.equal(
    Buffer.from(canonicalSubstrateSccpRuntimeStorageVerificationStatementBytes(input)).toString("hex"),
    Buffer.from(canonicalSubstrateSccpStorageProofBytes(input)).toString("hex"),
  );
  const publicInputsHash = substrateSccpRuntimeStorageProofPublicInputsHash(input);
  const request = buildSubstrateSccpRuntimeStorageProofRequest(input);
  assert.equal(Object.isFrozen(request), true);
  assert.equal(Object.isFrozen(request.publicInputColumns), true);
  assert.equal(Object.isFrozen(request.fastpqPublicInputs), true);
  assert.equal(Object.isFrozen(request.fastpqTransitions), true);
  assert.equal(request.runtimeStorageProofPublicInputsHash, publicInputsHash);
  assert.equal(request.fastpqPublicInputs.slot, "31");
  assert.equal(request.fastpqTransitions[0].key, "sccp:substrate:runtime-storage:v1:context");
});

test("package dist entrypoint exports TRON witness-schedule payload helpers", () => {
  const payload = canonicalTronWitnessSchedulePayloadBytes({
    witnessAddresses: [`0x41${"11".repeat(20)}`, `0x41${"22".repeat(20)}`],
    witnessWeights: [1n, 2n],
  });

  assert.equal(
    Buffer.from(payload).toString("hex"),
    `010200000041${"11".repeat(20)}010000000000000041${"22".repeat(20)}0200000000000000`,
  );
  assert.equal(
    tronWitnessSchedulePayloadHash(payload),
    "0xd6087d6ea6a1b58b17523587f28e457d84d5d2214298f93a09dbb509ea2cf429",
  );
  assert.equal(
    tronWitnessScheduleHashFromPayload(payload),
    "0x0c5eca6f96572fe939e640d8951abd126d2e966ffc4e3d0d087dbff6052577be",
  );
  const solidMessage = {
    sourceDomain: SCCP_DOMAIN_TRON,
    solidBlockNumber: 12345n,
    blockHash: "0x0000000000003039b6bc08fb34f737c093d9dd2adefccb04344715e2619c8286",
    witnessScheduleHash: "0x0c5eca6f96572fe939e640d8951abd126d2e966ffc4e3d0d087dbff6052577be",
    receiptRoot: `0x${"bb".repeat(32)}`,
    transactionRoot: `0x${"dd".repeat(32)}`,
    receiptProofHash: `0x${"cc".repeat(32)}`,
  };
  assert.equal(canonicalTronSolidBlockMessageBytes(solidMessage).length, 173);
  assert.equal(
    tronSolidBlockMessageHash(solidMessage),
    "0x065173d89272a549b504258936729c5226dfdb866ccb9422757d95ec9fa6d688",
  );
  const sourceEventHash =
    "0xbe9223cdfd6728fd2512f270a44f928fbd58df98f8e9e5fe13c4dc73503192e4";
  const ownerAddress = "0x417e5f4552091a69125d5dfcb7b8c2659029395bdf";
  const ownerSignature =
    "0x79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798" +
    "38508a4cf743e4a97ab3550672d69d980545ff8d776f6e9bade4ff4196f3693b00";
  const witnessSeal = {
    totalWeight: 1n,
    signedWeight: 1n,
    solidBlockMessageHash: sourceEventHash,
    witnessAddresses: [ownerAddress],
    witnessWeights: [1n],
    signersBitmap: "0x01",
    signatures: [ownerSignature],
  };
  assert.equal(canonicalTronWitnessSealBytes(witnessSeal).length, 200);
  assert.equal(
    tronWitnessSealHash(witnessSeal),
    "0x4266cf4de71c96e4fde925b686abbd50e67026f63ad90e0cf4899d4925d45849",
  );
  const parentScheduleHash =
    "0x87174bbfde1c4b8473a6be18df37b60979c7609ebf1788ce8cf97604311474b6";
  const transitionMessage = {
    sourceDomain: SCCP_DOMAIN_TRON,
    fromWitnessScheduleEpoch: 7n,
    toWitnessScheduleEpoch: 8n,
    transitionBlockNumber: 12345n,
    transitionBlockHash: solidMessage.blockHash,
    parentWitnessScheduleHash: parentScheduleHash,
    nextWitnessScheduleHash: solidMessage.witnessScheduleHash,
    nextWitnessSchedulePayload: payload,
  };
  assert.equal(canonicalTronWitnessScheduleTransitionMessageBytes(transitionMessage).length, 157);
  assert.equal(
    tronWitnessScheduleTransitionMessageHash(transitionMessage),
    "0x6e53d3f7d1253223a70a163a02544a8df27b74171cb0c76c8f42d71419fabd43",
  );
  const transitionSeal = {
    ...transitionMessage,
    nextWitnessSchedulePayloadHash:
      "0xd6087d6ea6a1b58b17523587f28e457d84d5d2214298f93a09dbb509ea2cf429",
    transitionMessageHash:
      "0x6e53d3f7d1253223a70a163a02544a8df27b74171cb0c76c8f42d71419fabd43",
    sealProof: {
      totalWeight: 1n,
      signedWeight: 1n,
      solidBlockMessageHash:
        "0x6e53d3f7d1253223a70a163a02544a8df27b74171cb0c76c8f42d71419fabd43",
      witnessAddresses: [ownerAddress],
      witnessWeights: [1n],
      signersBitmap: "0x01",
      signatures: [
        "0xc6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5" +
          "65d3d639f676a837945854abb3f59c4b93355bb55a789e31a25aee261500932d01",
      ],
    },
  };
  assert.equal(canonicalTronWitnessScheduleTransitionSealBytes(transitionSeal).length, 456);
  assert.equal(
    tronWitnessScheduleTransitionSealHash(transitionSeal),
    "0xbb3b7ef87bd3efb77d9b7f0a4dba8e7398827621d59039c694c285a7e2deacce",
  );
});

test("package dist entrypoint exports TRON receipt-state transcript helpers", () => {
  const input = {
    sourceEventDigest: `0x${"34".repeat(32)}`,
    receiptRoot: `0x${"bb".repeat(32)}`,
    transactionRoot: "0x21789ae4e9fb0f13a9d7ef876ccbc90ee2fe1d1eddeec5c35e33e0a09c768079",
    receiptRootIndex: 0n,
    receiptTrieProofNodes: [`0xe4822080a0${"bb".repeat(32)}`],
    inclusionBranch: [`0x${"ee".repeat(32)}`],
  };

  assert.equal(
    Buffer.from(canonicalEvmReceiptRootMptValue(input.receiptRoot)).toString("hex"),
    `f8409e736363703a65766d3a726563656970742d726f6f742d76616c75653a7631a0${"bb".repeat(32)}`,
  );
  assert.throws(() => canonicalEvmReceiptRootMptValue(`0x${"00".repeat(32)}`), /must not be zero/u);
  assert.equal(
    Buffer.from(canonicalTronReceiptRootMptValue(input.receiptRoot)).toString("hex"),
    `f8419f736363703a74726f6e3a726563656970742d726f6f742d76616c75653a7631a0${"bb".repeat(32)}`,
  );
  assert.equal(canonicalTronSccpReceiptStateProofBytes(input).length, 186);
  assert.equal(
    tronSccpReceiptStateProofHash(input),
    "0x847c5ee3e6f4f83fef4d754a9aed93fae38c6677011cae03b10228c17c60b13b",
  );
});

test("package dist entrypoint exports TRON solid-block header transcript helpers", () => {
  const parentRawHeader = canonicalTronRawBlockHeaderBytes({
    number: 12344n,
    txTrieRoot: `0x${"cc".repeat(32)}`,
    accountStateRoot: `0x${"aa".repeat(32)}`,
    parentBlockId: `0x${"bb".repeat(32)}`,
    witnessAddress: `0x41${"11".repeat(20)}`,
    headerVersion: 1,
    timestampMs: 1700000012344n,
  });
  const parentRawHeaderHash =
    "0x5647d462e78851c6701e5a1cd89912e6118f8aa18222c8b90867fedcca84c4d4";
  const parentBlockId =
    "0x0000000000003038701e5a1cd89912e6118f8aa18222c8b90867fedcca84c4d4";
  const rawHeader = canonicalTronRawBlockHeaderBytes({
    number: 12345n,
    txTrieRoot: `0x${"dd".repeat(32)}`,
    accountStateRoot: `0x${"ee".repeat(32)}`,
    parentBlockId,
    witnessAddress: `0x41${"11".repeat(20)}`,
    headerVersion: 1,
    timestampMs: 1700000012345n,
  });
  const rawHeaderHash =
    "0x614a09275b6d0fffb6bc08fb34f737c093d9dd2adefccb04344715e2619c8286";
  const blockId =
    "0x0000000000003039b6bc08fb34f737c093d9dd2adefccb04344715e2619c8286";

  assert.equal(tronRawBlockHeaderHash(parentRawHeader), parentRawHeaderHash);
  assert.equal(tronRawBlockHeaderHash(rawHeader), rawHeaderHash);
  assert.equal(tronBlockIdFromRawDataHash(12344n, parentRawHeaderHash), parentBlockId);
  assert.equal(tronBlockIdFromRawDataHash(12345n, rawHeaderHash), blockId);
  const tronHeaderSignature = (recoveryId) => {
    const signature = new Uint8Array(65).fill(0xaa);
    signature.fill(0x01, 32, 64);
    signature[64] = recoveryId;
    return signature;
  };
  const proof = {
    rawData: rawHeader,
    witnessSignature: tronHeaderSignature(0),
    parentRawData: parentRawHeader,
    parentWitnessSignature: tronHeaderSignature(27),
    rawDataHash: rawHeaderHash,
    parentRawDataHash: parentRawHeaderHash,
    blockId,
    txTrieRoot: `0x${"dd".repeat(32)}`,
    accountStateRoot: `0x${"ee".repeat(32)}`,
    parentBlockId,
    witnessAddress: `0x41${"11".repeat(20)}`,
    timestampMs: 1700000012345n,
    headerVersion: 1,
  };

  assert.equal(canonicalTronSolidBlockHeaderProofBytes(proof).length, 650);
  assert.equal(
    tronSolidBlockHeaderProofHash(proof),
    "0x25416bda5734ecef1ab9920d15f1011e962f6ff90e9c6247ff6b2ce34a5ab49f",
  );
  assert.throws(
    () =>
      canonicalTronSolidBlockHeaderProofBytes({
        ...proof,
        witnessSignature: new Uint8Array(65).fill(0xaa),
      }),
    /TRON header signatures must be canonical low-S/,
  );
});
