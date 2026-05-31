import { test } from "node:test";
import assert from "node:assert/strict";
import {
  SCCP_DOMAIN_SOL,
  SCCP_DOMAIN_SORA,
  SCCP_DOMAIN_TON,
  SCCP_DOMAIN_TRON,
  SCCP_DOMAIN_ETH,
  SCCP_DOMAIN_BSC,
  SCCP_DOMAIN_SORA_KUSAMA,
  SCCP_DOMAIN_SORA_POLKADOT,
  SCCP_DOMAIN_SORA2,
  SCCP_STARK_FRI_PROOF_FAMILY_V1,
  SCCP_SOURCE_STATE_MAX_PROOF_BYTES,
  SCCP_SOURCE_STATE_MAX_PROOF_LABEL_BYTES,
  SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES,
  SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
  SCCP_GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1,
  SCCP_EVM_CONTRACT_CALL_ABI_TUPLE_V1,
  SCCP_SUBMIT_MESSAGE_PROOF_SELECTOR_V1,
  SCCP_TRON_CONTRACT_CALL_ABI_TUPLE_V1,
  SCCP_SOLANA_BORSH_INSTRUCTION_V1,
  SCCP_SOLANA_MAINNET_GENESIS_HASH,
  SCCP_SOLANA_MAINNET_SLOTS_PER_EPOCH,
  SCCP_SOLANA_TOWER_LOCKOUT_CONFIRMATION_DEPTH,
  SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH,
  SCCP_SOLANA_MAX_VALIDATORS,
  SCCP_SOLANA_VOTE_PROGRAM_ID,
  SCCP_SOLANA_STAKE_PROGRAM_ID,
  SCCP_SOLANA_SYSVAR_PROGRAM_ID,
  SCCP_SOLANA_STAKE_HISTORY_SYSVAR_ID,
  SCCP_SOLANA_RECURSIVE_PROOF_BACKEND_V1,
  SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1,
  SCCP_SOLANA_TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1,
  SCCP_SOLANA_FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1,
  SCCP_SOLANA_BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1,
  SCCP_SOLANA_MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1,
  SCCP_SOLANA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1,
  SCCP_TON_CONTRACT_PROOF_BACKEND_V1,
  SCCP_TON_MESSAGE_BODY_BOC_V1,
  SCCP_TON_CURRENT_VALIDATOR_SET_CONFIG_PARAM,
  SCCP_TON_MAINNET_SHARD_STATE_VERIFIER_ID_V1,
  SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1,
  SCCP_TON_MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1,
  SCCP_TON_VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1,
  SCCP_TON_SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1,
  SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1,
  SCCP_SUBSTRATE_RUNTIME_PROOF_BACKEND_V1,
  SCCP_SUBSTRATE_RUNTIME_CALL_SCALE_V1,
  SCCP_SUBSTRATE_SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1,
  SCCP_SUBSTRATE_RUNTIME_STORAGE_OPEN_VERIFY_CIRCUIT_ID_V1,
  SCCP_ZERO_HASH_V1,
  EvmSccpProver,
  SolanaSccpSourceStateProver,
  SolanaSccpProver,
  SubstrateSccpProver,
  TonSccpSourceStateProver,
  TonSccpProver,
  TronSccpProver,
  bscCommitMessageHash,
  bscCommitSealHash,
  bscSccpReceiptProofHash,
  bscValidatorSetHashFromPayload,
  bscValidatorSetMetadataProofHash,
  bscValidatorSetPayloadFromHeaderRlp,
  bscValidatorSetPayloadFromParliaExtra,
  bscValidatorSetPayloadHash,
  bscValidatorSetStorageValueHash,
  bscValidatorSetTransitionMessageHash,
  buildSccpTonMessageBodyBoc,
  tonConfigValidatorSetPayloadFromProofBoc,
  tonConfigValidatorSetPayloadHashFromProofBoc,
  tonHashmapEProofRootHash,
  tonHashmapECellRefValueHash,
  tonShardAccountsLastTransaction,
  tonShardAccountsLastTransactionHash,
  tonShardStateProofRootHash,
  tonShardStateAccountsRootHash,
  tonBocRootHashes,
  tonBocSingleRootHash,
  buildEvmSccpProofRequest,
  buildEvmSccpBridgeProofSubmitPayload,
  buildEvmSccpSubmission,
  wrapEvmSccpProofResult,
  buildSolanaSccpSubmission,
  buildSolanaSccpProofRequest,
  buildSolanaSccpAccountsLtHashProofRequest,
  buildSolanaSccpFullLightClientAuditProofRequests,
  buildSolanaSccpTowerReplayProofRequest,
  wrapSolanaSccpSourceStateVerificationProof,
  buildSubstrateSccpProofRequest,
  wrapSubstrateSccpProofResult,
  buildSubstrateSccpSubmission,
  buildTonSccpFullLightClientAuditProofRequests,
  buildTonSccpProofRequest,
  wrapTonSccpProofResult,
  wrapTonSccpSourceStateVerificationProof,
  buildTonSccpSubmission,
  buildTronSccpProofRequest,
  buildTronSccpBridgeProofSubmitPayload,
  buildTronSccpSubmission,
  wrapTronSccpProofResult,
  sccpSubmitMessageProofCallData,
  canonicalBscCommitMessageBytes,
  canonicalBscCommitSealBytes,
  canonicalBscValidatorSetMetadataProofBytes,
  canonicalBscValidatorSetPayloadBytes,
  canonicalBscValidatorSetTransitionMessageBytes,
  canonicalBscSccpReceiptProofBytes,
  canonicalEvmReceiptRootMptValue,
  canonicalEvmSccpReceiptProofBytes,
  canonicalEthSyncCommitteePayloadBytes,
  canonicalEthSyncCommitteeTransitionMessageBytes,
  canonicalEthSyncCommitteeTransitionSignatureBytes,
  canonicalTronRawBlockHeaderBytes,
  canonicalTronSolidBlockMessageBytes,
  canonicalTronSolidBlockHeaderProofBytes,
  canonicalTronWitnessSealBytes,
  canonicalTronWitnessScheduleTransitionMessageBytes,
  canonicalTronWitnessScheduleTransitionSealBytes,
  canonicalSubstrateAuthoritySetPayloadBytes,
  canonicalSubstrateAuthoritySetTransitionJustificationBytes,
  canonicalSubstrateAuthoritySetTransitionMessageBytes,
  canonicalTronWitnessSchedulePayloadBytes,
  canonicalSolanaSccpProofContextBytes,
  canonicalSolanaSccpEpochStakeRootBytes,
  canonicalSolanaSccpStakeActivationBytes,
  canonicalSolanaSccpAccountOpeningBytes,
  canonicalSolanaSccpAccountInclusionLeafBytes,
  canonicalSolanaSccpAccountInclusionNodeBytes,
  canonicalSolanaSccpVoteAccountDataBytes,
  canonicalSolanaSccpStakeAccountDataBytes,
  canonicalSolanaSccpStakeAccountStateBytes,
  canonicalSolanaSccpStakeHistorySysvarDataBytes,
  canonicalSolanaSccpStakeHistoryBytes,
  canonicalSolanaSccpBankForkBytes,
  canonicalSolanaSccpRouteCanaryEvidenceBytes,
  canonicalTonSccpRouteCanaryEvidenceBytes,
  canonicalSolanaSccpAccountsLtHashProofPublicInputsBytes,
  canonicalSolanaSccpAccountsLtHashCommitmentBytes,
  canonicalSolanaSccpAccountsLtHashVerificationContextBytes,
  canonicalSolanaSccpSourceStateVerificationProofBytes,
  canonicalSolanaSccpFinalityContextBytes,
  canonicalSolanaSccpFullLightClientAuditStatementBytes,
  canonicalSolanaSccpMessageProofBytes,
  canonicalSolanaSccpTowerLockoutBytes,
  canonicalSolanaSccpTowerReplayBytes,
  canonicalSolanaSccpWitnessBytes,
  canonicalSccpSourceAdapterDeploymentBindingBytes,
  canonicalSccpSourceVerifierMaterialBytes,
  canonicalSccpSourceAdapterEngineDeploymentBytes,
  canonicalSccpMessageTransparentPublicInputsBytes,
  canonicalSccpTonSubmissionMetadataBytes,
  canonicalSccpBurnPayloadBytes,
  canonicalSccpTokenAddPayloadBytes,
  canonicalSccpTokenControlPayloadBytes,
  canonicalSccpCommitmentBytes,
  canonicalSubstrateSccpStorageProofBytes,
  canonicalSubstrateSccpRuntimeStorageVerificationStatementBytes,
  canonicalSubstrateSccpRuntimeStorageVerificationContextBytes,
  canonicalTonSccpShardProofBytes,
  canonicalTonSccpFullLightClientAuditStatementBytes,
  canonicalTonShardStateProofPublicInputsBytes,
  canonicalTonShardStateVerificationContextBytes,
  canonicalTonShardStateWitnessCommitmentBytes,
  canonicalTonSccpSourceStateVerificationProofBytes,
  canonicalTonMasterchainBlockMessageBytes,
  canonicalTonMasterchainConfigLeafBytes,
  canonicalTonMasterchainConfigProofBytes,
  canonicalTonMasterchainValidatorSignaturesBytes,
  canonicalTonValidatorSetBytes,
  canonicalTonValidatorSetPayloadBytes,
  canonicalTonValidatorSetTransitionMessageBytes,
  canonicalTonValidatorSetTransitionSignatureBytes,
  canonicalTronReceiptRootMptValue,
  canonicalTronSccpReceiptProofBytes,
  canonicalTronSccpReceiptStateProofBytes,
  canonicalTronSccpTransactionSourceProofBytes,
  ethSyncCommitteeHash,
  ethSyncCommitteeHashFromPayload,
  ethSyncCommitteePayloadHash,
  ethBeaconBlockHeaderRoot,
  ethBeaconBodyRootFromExecutionPayloadBranch,
  ethExecutionPayloadHeaderRootFromRlp,
  ethSyncCommitteeTransitionMessageHash,
  ethSyncCommitteeTransitionSignatureHash,
  evmSccpDestinationBinding,
  evmSccpDestinationBindingHash,
  evmSccpReceiptProofHash,
  normalizeSccpSourceAdapterDeploymentBinding,
  normalizeSccpSourceVerifierMaterial,
  normalizeSccpSourceAdapterEngineDeployment,
  isSupportedSccpDomain,
  normalizeSolanaSccpWitness,
  normalizeSolanaSccpProofContext,
  sccpSourceAdapterDeploymentBindingHash,
  sccpSourceVerifierMaterialHash,
  sccpSourceAdapterEngineDeploymentHash,
  sccpSolanaFullLightClientGateHash,
  sccpTonFullLightClientGateHash,
  sccpSourceAdapterVerifierVkHash,
  sccpTokenMessageTargetDomain,
  sccpDestinationBindingKey,
  sccpDestinationBindingHash,
  sccpGroth16Bn254PublicSignalWords,
  solanaSccpMessageProofHash,
  solanaSccpTransactionStatusLeafHash,
  solanaSccpTransactionStatusRootFromBranch,
  solanaSccpAgaveBankHash,
  solanaSccpBankForkHash,
  solanaSccpEpochStakeRoot,
  solanaSccpMainnetEpochForSlot,
  solanaSccpStakeActivationHash,
  solanaSccpAccountOpeningHash,
  solanaSccpAccountRawDataHash,
  solanaSccpAccountLtHash,
  solanaSccpAccountsLtHashChecksum,
  solanaSccpAccountsLtHashFromOpenings,
  solanaSccpAccountsLtHashOpenedContributionsHash,
  solanaSccpAccountsLtHashOpenedResidual,
  solanaSccpAccountsLtHashOpenedResidualChecksum,
  canonicalSolanaSccpAccountsLtHashOpenedContributionsBytes,
  solanaSccpAccountsLtHashPublicInputColumns,
  solanaSccpAccountsLtHashOpenVerifySchemaDescriptor,
  solanaSccpAccountInclusionLeafHash,
  solanaSccpAccountInclusionNodeHash,
  solanaSccpAccountInclusionRootFromBranch,
  solanaSccpAccountInclusionRootAndBranches,
  solanaSccpOpenedAccountInclusionWitness,
  solanaSccpVoteAccountDataHash,
  solanaSccpVoteAccountDataFromRawVoteState,
  solanaSccpVoteAccountDataHashFromRawVoteState,
  solanaSccpVoteAccountDataFromRawVoteStateV1OrV3,
  solanaSccpVoteAccountDataHashFromRawVoteStateV1OrV3,
  solanaSccpStakeAccountDataHash,
  solanaSccpStakeAccountDataFromRawStakeStateV2,
  solanaSccpStakeAccountDataHashFromRawStakeStateV2,
  solanaSccpStakeAccountStateHash,
  solanaSccpStakeHistorySysvarDataHash,
  solanaSccpStakeHistorySysvarDataHashFromRawData,
  solanaSccpStakeHistoryHash,
  solanaSccpTowerLockoutHash,
  solanaSccpTowerReplayHash,
  solanaSccpAccountsLtHashProofPublicInputsHash,
  solanaSccpAccountsLtHashProofHash,
  solanaSccpFinalityContextHash,
  solanaSccpVoteMessageHash,
  solanaSccpFullLightClientAuditStatementHash,
  solanaSccpFullLightClientAuditPublicInputColumns,
  solanaSccpFullLightClientAuditOpenVerifySchemaDescriptor,
  solanaSccpProofContextHash,
  solanaSccpRouteCanaryEvidenceHash,
  tonSccpRouteCanaryEvidenceHash,
  substrateAuthoritySetHashFromPayload,
  substrateAuthoritySetPayloadHash,
  substrateAuthoritySetTransitionJustificationHash,
  substrateAuthoritySetTransitionMessageHash,
  substrateSccpStorageProofHash,
  substrateSccpRuntimeStorageProofPublicInputsHash,
  substrateSccpRuntimeStoragePublicInputColumns,
  substrateSccpRuntimeStorageOpenVerifySchemaDescriptor,
  buildSubstrateSccpRuntimeStorageProofRequest,
  buildTonShardStateProofRequest,
  wrapSolanaSccpProofResult,
  tonSccpShardProofHash,
  tonShardStateOpenVerifySchemaDescriptor,
  tonShardStateProofPublicInputsHash,
  tonShardStatePublicInputColumns,
  tonSccpShardStateVerificationProofHash,
  tonSccpFullLightClientAuditStatementHash,
  tonSccpFullLightClientAuditPublicInputColumns,
  tonSccpFullLightClientAuditOpenVerifySchemaDescriptor,
  tonMasterchainBlockMessageHash,
  tonMasterchainConfigLeafHash,
  tonMasterchainConfigProofHash,
  tonMasterchainValidatorSignaturesHash,
  tonValidatorSetHash,
  tonValidatorSetHashFromPayload,
  tonValidatorSetPayloadHash,
  tonValidatorSetTransitionMessageHash,
  tonValidatorSetTransitionSignatureHash,
  tronSccpDestinationBinding,
  tronSccpDestinationBindingHash,
  tronSccpReceiptProofHash,
  tronSccpReceiptStateProofHash,
  tronSccpSourceMessageCallData,
  tronSccpTransactionSourceProofHash,
  tronBlockIdFromRawDataHash,
  tronRawBlockHeaderHash,
  tronSolidBlockMessageHash,
  tronSolidBlockHeaderProofHash,
  tronWitnessSealHash,
  tronWitnessScheduleTransitionMessageHash,
  tronWitnessScheduleTransitionSealHash,
  tronWitnessScheduleHashFromPayload,
  tronWitnessSchedulePayloadHash,
} from "../src/sccp.js";

const HEX32_A = `0x${"aa".repeat(32)}`;
const HEX32_B = `0x${"bb".repeat(32)}`;
const HEX32_C = `0x${"cc".repeat(32)}`;
const HEX32_D = `0x${"dd".repeat(32)}`;
const HEX32_E = `0x${"ee".repeat(32)}`;
const HEX32_F = `0x${"12".repeat(32)}`;
const HEX32_G = `0x${"56".repeat(32)}`;
const HEX32_H = `0x${"78".repeat(32)}`;
const SOLANA_MAINNET_GENESIS_PUBLIC_INPUT =
  "0x8dbaadfbc441ded0257a4700cd26d814b5a196be44b963454cff8dd9543f13b5";

const sampleEvmDestinationBindingInput = (overrides = {}) => ({
  targetDomain: SCCP_DOMAIN_ETH,
  networkId: `0x${"33".repeat(32)}`,
  verifierAddress: `0x${"11".repeat(20)}`,
  bridgeAddress: `0x${"22".repeat(20)}`,
  verifierCodeHash: `0x${"bb".repeat(32)}`,
  verifierKeyHash: `0x${"cc".repeat(32)}`,
  ...overrides,
});

const sampleEvmDestinationBinding = (overrides = {}) =>
  evmSccpDestinationBinding(sampleEvmDestinationBindingInput(overrides));

const sampleTronDestinationBindingInput = (overrides = {}) => ({
  networkId: `0x${"33".repeat(32)}`,
  verifierAddress: "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
  verifierCodeHash: `0x${"bb".repeat(32)}`,
  verifierKeyHash: `0x${"cc".repeat(32)}`,
  ...overrides,
});

const sampleTronDestinationBinding = (overrides = {}) =>
  tronSccpDestinationBinding(sampleTronDestinationBindingInput(overrides));

const sampleSolanaRouteCanaryEvidence = (overrides = {}) => ({
  routeAllowlistHash: `0x${"31".repeat(32)}`,
  destinationBindingHash: sccpDestinationBindingHash(SCCP_DOMAIN_SOL),
  sourceVerifierMaterialHash: `0x${"33".repeat(32)}`,
  sourceAdapterEngineDeploymentHash: `0x${"34".repeat(32)}`,
  verifierIdentity: "3JF3sEqM796hk5WFqA6EtmEwJQ9quALszsfJyvXNQKy3",
  verifierCodeHash: "0xc81178d11a4de525782fe7ac6f5accc2056fa15d1b8c2bfd819eb2ef179c3411",
  solanaRpcCommitment: "finalized",
  solanaProgramOwner: "BPFLoaderUpgradeab1e11111111111111111111111",
  solanaProgramdataOwner: "BPFLoaderUpgradeab1e11111111111111111111111",
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
  ...overrides,
});

const sampleTonRouteCanaryEvidence = (overrides = {}) => ({
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
  ...overrides,
});

const assertImmutableFastpqProofRequest = (request, byteFields) => {
  assert.equal(Object.isFrozen(request), true);
  assert.equal(Object.isFrozen(request.publicInputColumns), true);
  assert.equal(Object.isFrozen(request.publicInputColumns[0]), true);
  assert.equal(Object.isFrozen(request.fastpqPublicInputs), true);
  assert.equal(Object.isFrozen(request.fastpqTransitions), true);
  assert.equal(Object.isFrozen(request.fastpqTransitions[0]), true);

  for (const field of byteFields) {
    const exposedBytes = request[field];
    const freshBytes = request[field];
    assert.notStrictEqual(exposedBytes, freshBytes);
    assert.ok(exposedBytes.length > 0);
    const originalByte = freshBytes[0];
    exposedBytes[0] ^= 0xff;
    assert.equal(request[field][0], originalByte);
  }
};

const mutableFastpqProofRequest = (request) => {
  const mutable = { ...request };
  if (request.publicInputColumns) {
    mutable.publicInputColumns = request.publicInputColumns.map((column) => [...column]);
  }
  if (request.fastpqPublicInputs) {
    mutable.fastpqPublicInputs = { ...request.fastpqPublicInputs };
  }
  if (request.fastpqTransitions) {
    mutable.fastpqTransitions = request.fastpqTransitions.map((transition) => ({ ...transition }));
  }
  return mutable;
};

test("derives Solana ProgramData route canary evidence hash", () => {
  const evidence = sampleSolanaRouteCanaryEvidence();
  assert.equal(canonicalSolanaSccpRouteCanaryEvidenceBytes(evidence).length, 475);
  assert.equal(
    solanaSccpRouteCanaryEvidenceHash(evidence),
    "0x77296e47d5681f97136dc79d66dbda4478c3c5ec80271bfd4f1f3b3dbb8e15ca",
  );
  assert.throws(
    () =>
      solanaSccpRouteCanaryEvidenceHash(
        sampleSolanaRouteCanaryEvidence({ solanaProgramdataSlot: "4322" }),
      ),
    /solanaExpectedProgramdataSlot must match solanaProgramdataSlot/,
  );
  assert.throws(
    () =>
      solanaSccpRouteCanaryEvidenceHash(
        sampleSolanaRouteCanaryEvidence({ solanaProgramdataExecutableBase64: "AQIDBA==" }),
      ),
    /BPF ELF/,
  );
  assert.throws(
    () =>
      solanaSccpRouteCanaryEvidenceHash(
        sampleSolanaRouteCanaryEvidence({ destinationBindingHash: HEX32_H }),
      ),
    /destinationBindingHash must match canonical Solana destination binding/,
  );
  assert.throws(
    () =>
      solanaSccpRouteCanaryEvidenceHash(
        sampleSolanaRouteCanaryEvidence({ expectedDestinationBindingHash: HEX32_H }),
      ),
    /expectedDestinationBindingHash must match canonical Solana destination binding/,
  );
});

test("derives TON live-account route canary evidence hash", () => {
  const evidence = sampleTonRouteCanaryEvidence();
  assert.equal(canonicalTonSccpRouteCanaryEvidenceBytes(evidence).length, 358);
  assert.equal(
    tonSccpRouteCanaryEvidenceHash(evidence),
    "0xf128e8405017b9ca7733bb10d43eeaf783e38d39740a3455aa353c76655c6942",
  );
  assert.throws(
    () =>
      tonSccpRouteCanaryEvidenceHash(
        sampleTonRouteCanaryEvidence({ destinationBindingHash: HEX32_H }),
      ),
    /destinationBindingHash must match canonical TON destination binding/,
  );
  assert.throws(
    () =>
      tonSccpRouteCanaryEvidenceHash(
        sampleTonRouteCanaryEvidence({ verifierContractAddress: `1:${"11".repeat(32)}` }),
      ),
    /verifierContractAddress workchain must be basechain 0/,
  );
  assert.throws(
    () => tonSccpRouteCanaryEvidenceHash(sampleTonRouteCanaryEvidence({ accountStatus: "uninit" })),
    /accountStatus must be active/,
  );
  assert.throws(
    () =>
      tonSccpRouteCanaryEvidenceHash(
        sampleTonRouteCanaryEvidence({ lastTransactionLt: "0123" }),
      ),
    /lastTransactionLt must be a positive decimal/,
  );
  assert.throws(
    () =>
      tonSccpRouteCanaryEvidenceHash(
        sampleTonRouteCanaryEvidence({ verifierCodeBocRootHash: `0x${"45".repeat(32)}` }),
      ),
    /verifierCodeBocRootHash must match verifierCodeHash/,
  );
  assert.throws(
    () =>
      tonSccpRouteCanaryEvidenceHash(
        sampleTonRouteCanaryEvidence({ accountStatus: "active", account_status: "active" }),
      ),
    /accountStatus must not use multiple aliases/,
  );
});

const abiWord = (value) => {
  let remaining = BigInt(value);
  const out = new Uint8Array(32);
  for (let index = out.length - 1; index >= 0; index -= 1) {
    out[index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  return out;
};
const BN254_G2_GENERATOR_WORDS = [
  abiWord(0x1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6edn),
  abiWord(0x198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2n),
  abiWord(0x12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daan),
  abiWord(0x090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975bn),
];
const groth16ProofBytes = (words = []) => {
  const out = new Uint8Array(SCCP_GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1);
  const defaults = [
    abiWord(1),
    Uint8Array.from({ length: 32 }, () => 0x11),
    abiWord(SCCP_DOMAIN_SORA),
    Uint8Array.from({ length: 32 }, () => 0x33),
    abiWord(1),
    abiWord(2),
    ...BN254_G2_GENERATOR_WORDS,
    abiWord(1),
    abiWord(2),
  ];
  words.forEach((word, index) => {
    defaults[index] = word;
  });
  defaults.forEach((word, index) => out.set(word, index * 32));
  return out;
};
const GROTH16_PROOF_BYTES = groth16ProofBytes();
const SOLANA_SIGNATURE_55 =
  "2hxGyn4y9Mjkii76BqmxVoNYbTs3tw97bmtZRXnDoZPAw7VZTWhhk1aV11DtFgYGVibPaty4PQLHVLaKrT24NxGU";
const SOLANA_ZERO_SIGNATURE = "1".repeat(64);
const SOLANA_PROGRAM_42 = "5TeWSsjg2gbxCyWVniXeCmwM7UtHTCK7svzJr5xYJzHf";
const SOLANA_ZERO_PROGRAM = "1".repeat(32);
const BSC_VALIDATOR_SET_PAYLOAD_HEX = `0102000000${"11".repeat(20)}0100000000000000${"22".repeat(20)}0200000000000000`;
const BSC_VALIDATOR_SET_PAYLOAD_HASH =
  "0xdc6190956bc147c9a0a2fbf1384d40a1deb4b211a709f229275d1ea5ac3f8370";
const BSC_VALIDATOR_SET_HASH =
  "0x3ef5ecfb6dc4f5fc9e970cc18cd72164495c827e96f77851813973a286f5c762";
const BSC_COMMIT_VALIDATOR_PUBLIC_KEYS = [
  "0x0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
  "0x02c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5",
  "0x02f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
  "0x02e493dbf1c10d80f3581e4904930b1404cc6c13900ee0758474fa94abe8c4cd13",
];
const BSC_COMMIT_VALIDATOR_POWERS = [1n, 1n, 1n, 1n];
const BSC_COMMIT_VALIDATOR_SET_HASH =
  "0xc5152802f6ca9ec72a4249646aca7476496f00b71ab5b1482c881a31fb42dd8c";
const BSC_COMMIT_MESSAGE_HASH =
  "0x5832165d1a87ed49a323f2ecaecbef973489aed1a42e7eab369244e7abec43c7";
const BSC_COMMIT_SIGNATURES = [
  "0x1b8802069b82c3d4cb6d7bec82323853f36d965c1e71647560084e7c7a0de9c17c85fcc3c6222f905cbbc4ba5b5f3f005f07d144304184181be67b3d02d1ba9f00",
  "0x921d39c29fb793c496f96cf647128232d228024ed2f3e68cc6a52aa4cf64facf6bbd9dfcf7d703165f7880e7e1310f34d1b0fb8ca6dd8f506bf289ba012387f001",
  "0xcfa11aa1ec214278afdb4ef7f3c40af97a2784e0336afb5ebef345c0d2eaa9ef629ad2d25cf9709eb9b842fb2fb3f749ce365af97af6e7064771614312d3619600",
];
const BSC_COMMIT_SEAL_HASH =
  "0xcd9d87b24d8c1cf7615cb4267cde5a3fc24bbb770807134ee75d4ddaba992172";
const ETH_SYNC_COMMITTEE_HASH =
  "0xa95be780d50a9f42f4b1871e29798dbee0352d08027f0c4c6f4fc6466b4bd536";
const ETH_NEXT_SYNC_COMMITTEE_PAYLOAD_HEX =
  `010200000030000000${"33".repeat(48)}030000000000000060000000${"cc".repeat(96)}` +
  `30000000${"44".repeat(48)}040000000000000060000000${"dd".repeat(96)}`;
const ETH_NEXT_SYNC_COMMITTEE_HASH =
  "0xb3343685e8ab63a2d66bccebb6c03a149a53330389473b4a495598065c17b445";
const ETH_NEXT_SYNC_COMMITTEE_PAYLOAD_HASH =
  "0xfdba6ad2ff9acca564b1042eec01c2d6356d5e2ade5e653c9d47360e55d53e17";
const ETH_SYNC_COMMITTEE_TRANSITION_MESSAGE_HASH =
  "0xc5cbfaf915a63e59bc142277814f13fab1e8012a0bd56db7033b18bc02637bec";
const ETH_SYNC_COMMITTEE_TRANSITION_SIGNATURE_HASH =
  "0x2d03886e7ea307f7b5a77af00075b32536cbf016d0d8554bec2b1e424252f858";
const TRON_WITNESS_SCHEDULE_PAYLOAD_HEX = `010200000041${"11".repeat(20)}010000000000000041${"22".repeat(20)}0200000000000000`;
const TRON_WITNESS_SCHEDULE_PAYLOAD_HASH =
  "0xd6087d6ea6a1b58b17523587f28e457d84d5d2214298f93a09dbb509ea2cf429";
const TRON_WITNESS_SCHEDULE_HASH =
  "0x0c5eca6f96572fe939e640d8951abd126d2e966ffc4e3d0d087dbff6052577be";
const TRON_PARENT_RAW_HEADER_HEX =
  "08b8b096ffbc311220cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc1a20bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb38b8604a1541111111111111111111111111111111111111111150015a20aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const TRON_RAW_HEADER_HEX =
  "08b9b096ffbc311220dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd1a200000000000003038701e5a1cd89912e6118f8aa18222c8b90867fedcca84c4d438b9604a1541111111111111111111111111111111111111111150015a20eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const TRON_PARENT_RAW_HEADER_HASH =
  "0x5647d462e78851c6701e5a1cd89912e6118f8aa18222c8b90867fedcca84c4d4";

const sampleSolanaStakeStateV2StakeAccount = () => {
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
};

const sampleSolanaVoteStateAccount = (hasLatency = true) => {
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

  writeU32(hasLatency ? 2 : 1);
  writeRepeated(0x51, 32);
  writeRepeated(0x71, 32);
  writeU8(7);
  writeU64(SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH);
  for (let index = 0; index < Number(SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH); index += 1) {
    if (hasLatency) writeU8(0);
    writeU64(11n + BigInt(index));
    writeU32(Number(SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH) - index);
  }
  writeU8(1);
  writeU64(10n);
  writeU64(2n);
  writeU64(1n);
  writeRepeated(0x60, 32);
  writeU64(3n);
  writeRepeated(0x61, 32);
  return data;
};

const sampleSolanaVoteStateV4Account = (withBls = true, authorizedVoterCount = 2) => {
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
  writeU8(withBls ? 1 : 0);
  if (withBls) {
    writeRepeated(0xa5, 48);
  }
  writeU64(SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH);
  for (let index = 0; index < Number(SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH); index += 1) {
    writeU8(0);
    writeU64(11n + BigInt(index));
    writeU32(Number(SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH) - index);
  }
  writeU8(1);
  writeU64(10n);
  writeU64(BigInt(authorizedVoterCount));
  for (let index = 0; index < authorizedVoterCount; index += 1) {
    writeU64(BigInt(index + 1));
    writeRepeated(0x60 + index, 32);
  }
  return data;
};
const TRON_RAW_HEADER_HASH =
  "0x614a09275b6d0fffb6bc08fb34f737c093d9dd2adefccb04344715e2619c8286";
const TRON_PARENT_BLOCK_ID =
  "0x0000000000003038701e5a1cd89912e6118f8aa18222c8b90867fedcca84c4d4";
const TRON_BLOCK_ID =
  "0x0000000000003039b6bc08fb34f737c093d9dd2adefccb04344715e2619c8286";
const TRON_SOLID_BLOCK_HEADER_PROOF_HASH =
  "0x25416bda5734ecef1ab9920d15f1011e962f6ff90e9c6247ff6b2ce34a5ab49f";
const TRON_SOLID_BLOCK_MESSAGE_HASH =
  "0x065173d89272a549b504258936729c5226dfdb866ccb9422757d95ec9fa6d688";
const TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR =
  "be9223cdfd6728fd2512f270a44f928fbd58df98f8e9e5fe13c4dc73503192e4";
const TRON_SOURCE_EVENT_SIGNATURE_VECTOR =
  "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798" +
  "38508a4cf743e4a97ab3550672d69d980545ff8d776f6e9bade4ff4196f3693b" +
  "00";
const TRON_TEST_OWNER_ADDRESS = "0x417e5f4552091a69125d5dfcb7b8c2659029395bdf";
const TRON_WITNESS_SEAL_HASH =
  "0x4266cf4de71c96e4fde925b686abbd50e67026f63ad90e0cf4899d4925d45849";
const TRON_PARENT_WITNESS_SCHEDULE_PAYLOAD_HEX =
  "0101000000417e5f4552091a69125d5dfcb7b8c2659029395bdf0100000000000000";
const TRON_PARENT_WITNESS_SCHEDULE_HASH =
  "0x87174bbfde1c4b8473a6be18df37b60979c7609ebf1788ce8cf97604311474b6";
const TRON_WITNESS_SCHEDULE_TRANSITION_MESSAGE_HASH =
  "0x6e53d3f7d1253223a70a163a02544a8df27b74171cb0c76c8f42d71419fabd43";
const TRON_WITNESS_SCHEDULE_TRANSITION_SIGNATURE =
  "0xc6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5" +
  "65d3d639f676a837945854abb3f59c4b93355bb55a789e31a25aee261500932d01";
const TRON_WITNESS_SCHEDULE_TRANSITION_SEAL_HASH =
  "0xbb3b7ef87bd3efb77d9b7f0a4dba8e7398827621d59039c694c285a7e2deacce";
const tronHeaderSignature = (recoveryId) => {
  const signature = new Uint8Array(65).fill(0xaa);
  signature.fill(0x01, 32, 64);
  signature[64] = recoveryId;
  return signature;
};
const TRON_RECEIPT_STATE_MPT_NODE_HEX = `0xe4822080a0${"bb".repeat(32)}`;
const EVM_RECEIPT_ROOT_MPT_VALUE_HEX =
  `0xf8409e736363703a65766d3a726563656970742d726f6f742d76616c75653a7631a0${"bb".repeat(32)}`;
const EVM_RECEIPT_STATE_MPT_NODE_HEX = `0xf847822080b842${EVM_RECEIPT_ROOT_MPT_VALUE_HEX.slice(2)}`;
const EVM_RECEIPT_STATE_TRANSACTION_ROOT =
  "0x6438aaabb78989f2803c6b0f227ee0f94beecde07cdd9c737e258e4faf581b68";
const TRON_RECEIPT_ROOT_MPT_VALUE_HEX =
  `0xf8419f736363703a74726f6e3a726563656970742d726f6f742d76616c75653a7631a0${"bb".repeat(32)}`;
const TRON_RECEIPT_STATE_TRANSACTION_ROOT =
  "0x21789ae4e9fb0f13a9d7ef876ccbc90ee2fe1d1eddeec5c35e33e0a09c768079";
const TRON_RECEIPT_STATE_PROOF_HASH =
  "0x847c5ee3e6f4f83fef4d754a9aed93fae38c6677011cae03b10228c17c60b13b";
const TRON_SOURCE_MESSAGE_CALL_DATA_HEX =
  `06841e30${"0".repeat(63)}5${"0".repeat(64)}${"34".repeat(32)}`;
const TRON_TRANSACTION_SOURCE_BYTES_HEX =
  "0x0af3010a02123418b9602208565656565656565640959aef3a5acf01081f12ca" +
  "010a31747970652e676f6f676c65617069732e636f6d2f70726f746f636f6c2e" +
  "54726967676572536d617274436f6e74726163741294010a15417e5f4552091a" +
  "69125d5dfcb7b8c2659029395bdf121541454545454545454545454545454545" +
  "4545454545226406841e30000000000000000000000000000000000000000000" +
  "0000000000000000000005000000000000000000000000000000000000000000" +
  "0000000000000000000000343434343434343434343434343434343434343434" +
  "34343434343434343434347090e5ee3a900180e1eb171241cc58d7ac52c91117" +
  "92495fee682b53cab96ff4229043c5b8b90c31447f5934553d8854ab35de3437" +
  "2c13331bf3ef5cefd8f2cc5ad026faf223da83969fe8973c012a0410001801";
const TRON_TRANSACTION_SOURCE_ROOT =
  "0x1751c62dce36d5d642e48480b45d48ed16dd1b9b40ce216bc2f15c1b1ccf300b";
const TRON_TRANSACTION_SOURCE_PROOF_HASH =
  "0xfc98a09ae9e7f63ccd383b2f3e104efce0d2c291dc7900ffd49e4f391e6016b6";
const SUBSTRATE_AUTHORITY_SET_PAYLOAD_HEX = `0102000000${"11".repeat(32)}0100000000000000${"22".repeat(32)}0200000000000000`;
const SUBSTRATE_AUTHORITY_SET_PAYLOAD_HASH =
  "0xdedc4ebe5f91162a5029cb67f88cdbbf94c2bf2b9d0d373bd3e670321565cc16";
const SUBSTRATE_AUTHORITY_SET_HASH =
  "0xde84b8b7a5409c0f2cff1191173d6caa681d902b35e42669106ec6ea3193a117";
const SUBSTRATE_PARENT_AUTHORITY_SET_PAYLOAD_HEX =
  `0103000000${"11".repeat(32)}0500000000000000${"22".repeat(32)}0700000000000000` +
  `${"33".repeat(32)}0b00000000000000`;
const SUBSTRATE_NEXT_AUTHORITY_SET_PAYLOAD_HEX =
  `0103000000${"aa".repeat(32)}0d00000000000000${"bb".repeat(32)}1100000000000000` +
  `${"cc".repeat(32)}1300000000000000`;
const SUBSTRATE_PARENT_AUTHORITY_SET_HASH =
  "0xb2efd5d86304ea728a8a9ed4013aab8f3e10c0cf862e859c9cade55e660934ef";
const SUBSTRATE_NEXT_AUTHORITY_SET_HASH =
  "0x07cdbba0d61fdd4324b571dd793965e52acbf7f4c163af328e26c92c047501b3";
const SUBSTRATE_NEXT_AUTHORITY_SET_PAYLOAD_HASH =
  "0x12ce972498ba5cd8a760aee0429fdc30d8b6447890e1bf77d8dde46f86b40d85";
const SUBSTRATE_AUTHORITY_SET_TRANSITION_MESSAGE_HASH =
  "0x60589333bf798bf592b2642d0fbac39b4e9305576cd2ebe9dd1f448a97a0596b";
const SUBSTRATE_AUTHORITY_SET_TRANSITION_JUSTIFICATION_HASH =
  "0x9528bad2f181eb20a86a9c106cf529e60abf5db81f05cce9c9c3027b78c6cf01";
const TON_VALIDATOR_SET_HASH =
  "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938";
const TON_NEXT_VALIDATOR_SET_PAYLOAD_HEX =
  `0102000000${"33".repeat(32)}0300000000000000${"44".repeat(32)}0400000000000000`;
const TON_NEXT_VALIDATOR_SET_HASH =
  "0x26bfcffe8913e5e4f09e56076d5a237cbc5b890d31b8912bd7eacc5d3805691f";
const TON_NEXT_VALIDATOR_SET_PAYLOAD_HASH =
  "0xb76b843e99596a049425653e9921e4227af23a5b70331940fa057f1f58314983";
const TON_VALIDATOR_SET_TRANSITION_MESSAGE_HASH =
  "0x91eda926884eb1ae700e7b398c46f6d47fbb973efa322564894936140ccd2a19";
const TON_VALIDATOR_SET_TRANSITION_SIGNATURE_HASH =
  "0xd784461f68495981c2c00e60316dc9353ea4b5be3bc261b26feadc7c83c4f6a7";
const TON_VALIDATOR_SET_PAYLOAD_HASH =
  "0xb322afe2faa070a2ed88a922c5ac5d27e5f9fecc41a11ffbed37cca293c4aeb0";
const TON_TEMPLATE_SOURCE_STATE_VERIFIER_HASH =
  "0x540205f876591604ccf39f72a051ac5e82647c9e48dbd48cb129d2543971a34f";
const TON_TEMPLATE_SOURCE_MATERIAL_COMPONENT_HASHES = new Map([
  ["sourceTrustAnchorHash", "0xd83b3a3eb920ac8338533535cf0d6c69c69d507e84aef8ec2094564b8427c56c"],
  ["consensusVerifierHash", "0xb0225e16477ea3420f7d0de76b87b6e99a43ab97f445d8565a384d4b655bc473"],
  ["messageInclusionVerifierHash", "0x89254256421c15da8c92842c7d6f448ef6c1d5ca1e2a173754643425fcee6353"],
  ["sourceStateVerifierHash", TON_TEMPLATE_SOURCE_STATE_VERIFIER_HASH],
  ["finalityPolicyHash", "0x50044ee6db0eb0cdef097e69406b6c30d3406d8f784e8ba34e9b923b38bd0c43"],
]);
const SOLANA_TEMPLATE_SOURCE_MATERIAL_COMPONENT_HASHES = new Map([
  ["sourceTrustAnchorHash", "0x113bdb7601d84f2098daec386346a7123857d181b3ac5bd23df50fa9e1b2cbe3"],
  ["consensusVerifierHash", "0x97ea89019e6c79305d06dfc27640ee14a6b42ba6eaf86e1835ee9b433dba48ba"],
  ["messageInclusionVerifierHash", "0xb8358bfef1e428a6a7e9115687cb2b88d9c21dad4021bea3e11d43489eb3dcb0"],
  ["sourceStateVerifierHash", SCCP_SOLANA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1],
  ["finalityPolicyHash", "0x9df7ea90cf1bbba036788b14804f63f4be1e908390be89524fd4486f74344f56"],
]);
const TRON_TEMPLATE_SOURCE_MATERIAL_COMPONENT_HASHES = new Map([
  ["sourceTrustAnchorHash", "0x3550934cbdfe49449ec4aa383dcea7674541fedf66ab6159b1ed2f2c0be4755c"],
  ["consensusVerifierHash", "0x8a1de96a869b2f28f197a7835597f17cf77ff45f7cbb77da2f7c48e87df8c5ea"],
  ["messageInclusionVerifierHash", "0xf39db56474b288680ad9561389cca7a841bd1fd223719255324705e1038fcacc"],
  ["finalityPolicyHash", "0xad5a6a4f200e070400b5aaa1b7976c639e67571eb711eb6f69d01e3615423864"],
]);
const TON_MASTERCHAIN_CONFIG_LEAF_HASH =
  "0xed92ba8082850092da7cc296a2184cc4576877aaee08c72748d96ea449b16e39";
const TON_MASTERCHAIN_CONFIG_PROOF_BOC_HEX =
  "b5ee9c72010106010091000101c00101117fffffff80000008a002012b120000000100000002000200020000000000000003c00302087fff00000405005b14e3a049e28444444444444444444444444444444444444444444444444444444444444444400000000000000060005b14e3a049e288888888888888888888888888888888888888888888888888888888888888888000000000000000a0";
const TON_MASTERCHAIN_CONFIG_ROOT =
  "0x5bf87008e0e76085d6db977b53a89329de49a4eed8fd1ff90d8c78f096ef05af";
const TON_MASTERCHAIN_CONFIG_VALUE_HASH =
  "0x1aa64eb5ca0b3cb254dfada709904ce81f8b327eed0d83f2522122a0a9dddd50";
const TON_MASTERCHAIN_CONFIG_PROOF_HASH =
  "0x9949285613a9e9dfb4ed3728bbede7ddea36fd82ac3d7eff3955dd75e9c4941c";
const TON_SHARD_STATE_MASTERCHAIN_CONFIG_PROOF_HASH =
  "0x235c1f0946e38bc210a6a8e193fbe52399ccc4d82693ef3f123be20e27697fc3";
const TON_MASTERCHAIN_BLOCK_MESSAGE_HASH =
  "0x0ca07d5072adb7db3d6a0f831294c7e119c451884aaa1afcbb23e0df0911d8bd";
const TON_MASTERCHAIN_SIGNATURES_HASH =
  "0x7a927ad3e689e4f3679fe1d1b8ea1088b914523b0c2da0d6dc0938e5e5cf8d15";
const TON_ORDINARY_BOC_HEX = "b5ee9c720101020100070001020101000202";
const TON_ORDINARY_BOC_CRC_HEX = "b5ee9c724101020100070001020101000202be1c1df5";
const TON_ORDINARY_BOC_ROOT_HASH =
  "0x49725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe";
const TON_PRUNED_BRANCH_BOC_HEX =
  "b5ee9c72010101010026002848010149725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe0001";
const TON_PRUNED_BRANCH_ROOT_HASH =
  "0xcc9095f882fb62a27bb19ad4aa84e19571a3283988ae40b75e238ad240cf1a96";
const TON_LEGACY_PRUNED_PROOF_BOC_HEX =
  "b5ee9c7201010601005f0022012001052201620203284801010bd445eea7213bd88307c204a267aa798c1bacb2ad2d781f6106a8296bc12b6500010103a0c0040004006f284801011d894d2390dbd75b607f99091580d9f1652f34c525e35ba648d4325cc7495d3e0001";
const TON_LEGACY_PRUNED_PROOF_ROOT_HASH =
  "0x9c769b035b601b0ddc098e9b148d9bdab0761c14bfe310ac090962ba1f39739a";
const TON_MERKLE_PROOF_BOC_HEX =
  "b5ee9c7201010301002d0009460349725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe00010101020102000202";
const TON_MERKLE_PROOF_ROOT_HASH =
  "0xe749bc5225cabbe3fa78fc12d74a734c365379bc0d302123dcf7bfa2ee3fbd21";
const TON_HASHMAP_E_CELL_REF_BOC_HEX =
  "b5ee9c72010109010028000101c001020120020702016203050103a0c004000403090103a0c0060004006f0101de08000403e7";
const TON_HASHMAP_E_DIRECT_PROOF_BOC_HEX =
  "b5ee9c72010107010063002101c00122012002062201620304284801010bd445eea7213bd88307c204a267aa798c1bacb2ad2d781f6106a8296bc12b6500010103a0c0050004006f284801011d894d2390dbd75b607f99091580d9f1652f34c525e35ba648d4325cc7495d3e0001";
const TON_HASHMAP_E_MERKLE_PROOF_BOC_HEX =
  "b5ee9c72010108010089000101c001094603e714f85374c2c336ed499a5a35e6c4f87441184532e7c23be795ce71b457f1bf00030222012003072201620405284801010bd445eea7213bd88307c204a267aa798c1bacb2ad2d781f6106a8296bc12b6500010103a0c0060004006f284801011d894d2390dbd75b607f99091580d9f1652f34c525e35ba648d4325cc7495d3e0001";
const TON_HASHMAP_E_VALUE_HASH =
  "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419";
const TON_HASHMAP_E_ROOT_HASH =
  "0x767fcde38f7a8e9eb21d75271ed20e2b92c30e9f1726ee0247c98829b900199d";
const TON_SHARD_ACCOUNTS_BOC_HEX =
  "b5ee9c72010103010073000101c00101d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e41900000000000000078020000";
const TON_SHARD_ACCOUNTS_ROOT_HASH =
  "0x049a63ecefc78dc0cd468ebf47e0385807d790a2ca8e0dca5cbbeb0714567fd3";
const TON_SHARD_STATE_PROOF_BOC_HEX =
  "b5ee9c720101060100aa00035b9023afe2ffffff110000000000000000000000000000000007000000010000000b000000000000000c000000122001020500000101c00301d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419000000000000000780400000000";
const TON_SHARD_STATE_ROOT_HASH =
  "0x12a960855fea2f529c336d7325b1cca784f0f0b1a52ae149d02d046a2499e270";
const TON_SHARD_ACCOUNT_KEY = Uint8Array.from([17, ...Array(31).fill(0)]);

function minimalBeLengthBytes(length) {
  const hex = length.toString(16);
  return Array.from(Buffer.from(hex.length % 2 === 0 ? hex : `0${hex}`, "hex"));
}

function rlpString(bytes) {
  if (bytes.length === 1 && bytes[0] < 0x80) return Uint8Array.from(bytes);
  if (bytes.length < 56) return Uint8Array.from([0x80 + bytes.length, ...bytes]);
  const lengthBytes = minimalBeLengthBytes(bytes.length);
  return Uint8Array.from([0xb7 + lengthBytes.length, ...lengthBytes, ...bytes]);
}

function rlpList(fields) {
  const payload = Buffer.concat(fields.map((field) => Buffer.from(field)));
  if (payload.length < 56) return Uint8Array.from([0xc0 + payload.length, ...payload]);
  const lengthBytes = minimalBeLengthBytes(payload.length);
  return Uint8Array.from([0xf7 + lengthBytes.length, ...lengthBytes, ...payload]);
}

function sampleBscParliaExtra() {
  return Uint8Array.from([
    ...Array(32).fill(0x11),
    2,
    ...Array(20).fill(0x11),
    ...Array(48).fill(0x01),
    ...Array(20).fill(0x22),
    ...Array(48).fill(0x02),
    ...Array(65).fill(0x99),
  ]);
}

function sampleBscParliaHeaderRlp(extraData) {
  return rlpList([
    rlpString(Uint8Array.from(Array(32).fill(0x10))),
    rlpString(Uint8Array.from(Array(32).fill(0x11))),
    rlpString(Uint8Array.from(Array(20).fill(0x12))),
    rlpString(Uint8Array.from(Array(32).fill(0x13))),
    rlpString(Uint8Array.from(Array(32).fill(0x14))),
    rlpString(Uint8Array.from(Array(32).fill(0x15))),
    rlpString(Uint8Array.from(Array(256).fill(0x00))),
    rlpString(Uint8Array.from([2])),
    rlpString(Uint8Array.from([1])),
    rlpString(Uint8Array.from([1])),
    rlpString(Uint8Array.from([1])),
    rlpString(Uint8Array.from([1])),
    rlpString(extraData),
    rlpString(Uint8Array.from(Array(32).fill(0x00))),
    rlpString(Uint8Array.from(Array(8).fill(0x00))),
  ]);
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
    rlpString(Uint8Array.from([])),
    rlpString(Uint8Array.from([0x2a])),
    rlpString(Uint8Array.from([0x01, 0xc9, 0xc3, 0x80])),
    rlpString(Uint8Array.from([0x52, 0x08])),
    rlpString(Uint8Array.from([0x65, 0x53, 0xf1, 0x00])),
    rlpString(Uint8Array.from(Buffer.from("iroha-sccp-test"))),
    rlpString(Uint8Array.from(Array(32).fill(0x16))),
    rlpString(Uint8Array.from(Array(8).fill(0x00))),
    rlpString(Uint8Array.from([0x3b, 0x9a, 0xca, 0x00])),
    rlpString(Uint8Array.from(Array(32).fill(0x17))),
    rlpString(Uint8Array.from([])),
    rlpString(Uint8Array.from([])),
    rlpString(Uint8Array.from(Array(32).fill(0x18))),
  ]);
}

const sampleTonPublicInputs = {
  version: 1,
  messageId: HEX32_D,
  payloadHash: HEX32_E,
  targetDomain: SCCP_DOMAIN_TON,
  commitmentRoot: HEX32_F,
  finalityHeight: 19n,
  finalityBlockHash: HEX32_A,
};

const sampleTronPublicInputs = {
  version: 1,
  messageId: `0x${"11".repeat(32)}`,
  payloadHash: `0x${"22".repeat(32)}`,
  targetDomain: SCCP_DOMAIN_TRON,
  commitmentRoot: `0x${"33".repeat(32)}`,
  finalityHeight: 19n,
  finalityBlockHash: `0x${"44".repeat(32)}`,
};

const sampleEvmPublicInputs = {
  version: 1,
  messageId: `0x${"11".repeat(32)}`,
  payloadHash: `0x${"22".repeat(32)}`,
  targetDomain: SCCP_DOMAIN_ETH,
  commitmentRoot: `0x${"33".repeat(32)}`,
  finalityHeight: 19n,
  finalityBlockHash: `0x${"44".repeat(32)}`,
};

const sampleSubstratePublicInputs = {
  version: 1,
  messageId: `0x${"21".repeat(32)}`,
  payloadHash: `0x${"22".repeat(32)}`,
  targetDomain: SCCP_DOMAIN_SORA2,
  commitmentRoot: `0x${"23".repeat(32)}`,
  finalityHeight: 42n,
  finalityBlockHash: `0x${"24".repeat(32)}`,
};

test("rejects boolean SCCP domains in web portal payload helpers", () => {
  const assetId = `0x${"11".repeat(32)}`;
  const messageId = `0x${"22".repeat(32)}`;
  const payloadHash = `0x${"33".repeat(32)}`;
  const burnPayload = {
    version: 1,
    source_domain: SCCP_DOMAIN_SORA,
    dest_domain: SCCP_DOMAIN_ETH,
    nonce: 1n,
    sora_asset_id: assetId,
    amount: 7n,
    recipient: `0x${"44".repeat(32)}`,
  };
  const tokenPayload = {
    version: 1,
    target_domain: SCCP_DOMAIN_ETH,
    nonce: 2n,
    sora_asset_id: assetId,
    decimals: 18,
    name: `0x${"55".repeat(32)}`,
    symbol: `0x${"66".repeat(32)}`,
  };
  const commitment = {
    version: 1,
    kind: "TokenAdd",
    target_domain: SCCP_DOMAIN_ETH,
    message_id: messageId,
    payload_hash: payloadHash,
  };

  assert.equal(isSupportedSccpDomain(true), false);
  assert.equal(isSupportedSccpDomain(false), false);
  assert.throws(
    () => canonicalSccpBurnPayloadBytes({ ...burnPayload, source_domain: true }),
    /payload\.source_domain must be a u32 domain id/,
  );
  assert.throws(
    () => canonicalSccpBurnPayloadBytes({ ...burnPayload, dest_domain: false }),
    /payload\.dest_domain must be a u32 domain id/,
  );
  assert.throws(
    () => canonicalSccpTokenAddPayloadBytes({ ...tokenPayload, target_domain: true }),
    /payload\.target_domain must be a u32 domain id/,
  );
  assert.throws(
    () => canonicalSccpTokenControlPayloadBytes({ ...tokenPayload, target_domain: false }),
    /payload\.target_domain must be a u32 domain id/,
  );
  assert.throws(
    () => sccpTokenMessageTargetDomain({ kind: "TokenPause", value: { ...tokenPayload, target_domain: true } }),
    /payload\.target_domain must be a u32 domain id/,
  );
  assert.throws(
    () => canonicalSccpCommitmentBytes({ ...commitment, target_domain: true }),
    /commitment\.target_domain must be a u32 domain id/,
  );
  assert.throws(
    () =>
      canonicalSccpTonSubmissionMetadataBytes({
        manifest: {
          localDomain: true,
          counterpartyDomain: SCCP_DOMAIN_TON,
        },
        destinationBinding: { key: "sora:ton", bindingHash: HEX32_H },
        publicInputs: sampleTonPublicInputs,
        statementHash: HEX32_G,
      }),
    /manifest\.localDomain must be a u32 domain id/,
  );
  assert.throws(
    () =>
      canonicalSccpTonSubmissionMetadataBytes({
        manifest: {
          localDomain: SCCP_DOMAIN_SORA,
          counterpartyDomain: false,
        },
        destinationBinding: { key: "sora:ton", bindingHash: HEX32_H },
        publicInputs: sampleTonPublicInputs,
        statementHash: HEX32_G,
    }),
    /manifest\.counterpartyDomain must be a u32 domain id/,
  );
  const tonSubmissionManifest = {
    version: 1,
    localDomain: SCCP_DOMAIN_SORA,
    counterpartyDomain: SCCP_DOMAIN_TON,
    securityModel: "RecursiveZk",
    anchorGovernance: "CryptographicProof",
    verifierTarget: "TonContract",
    verifierBackendFamily: "TonContract",
    proofFamily: SCCP_STARK_FRI_PROOF_FAMILY_V1,
    verifierBackendKey: SCCP_TON_CONTRACT_PROOF_BACKEND_V1,
    messageBackend: "sccp-message-v1",
    registryBackend: "sccp-registry-v1",
    manifestSeed: "iroha:sccp:bridge-proof:message:stark-fri:v1:ton",
    destinationBinding: { key: "sora:ton", bindingHash: HEX32_H },
  };
  assert.ok(
    canonicalSccpTonSubmissionMetadataBytes({
      manifest: tonSubmissionManifest,
      destinationBinding: tonSubmissionManifest.destinationBinding,
      destinationBindingHash: HEX32_H,
      publicInputs: sampleTonPublicInputs,
      statementHash: HEX32_G,
    }).length > 0,
  );
  assert.throws(
    () =>
      canonicalSccpTonSubmissionMetadataBytes({
        manifest: tonSubmissionManifest,
        destinationBinding: tonSubmissionManifest.destinationBinding,
        destination_binding: tonSubmissionManifest.destinationBinding,
        destinationBindingHash: HEX32_H,
        publicInputs: sampleTonPublicInputs,
        statementHash: HEX32_G,
      }),
    /destinationBinding must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalSccpTonSubmissionMetadataBytes({
        manifest: tonSubmissionManifest,
        destinationBinding: {
          ...tonSubmissionManifest.destinationBinding,
          binding_hash: HEX32_H,
        },
        destinationBindingHash: HEX32_H,
        publicInputs: sampleTonPublicInputs,
        statementHash: HEX32_G,
      }),
    /destinationBinding\.bindingHash must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalSccpTonSubmissionMetadataBytes({
        manifest: tonSubmissionManifest,
        destinationBinding: tonSubmissionManifest.destinationBinding,
        destinationBindingHash: HEX32_H,
        destination_binding_hash: HEX32_H,
        publicInputs: sampleTonPublicInputs,
        statementHash: HEX32_G,
      }),
    /destinationBindingHash must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalSccpTonSubmissionMetadataBytes({
        manifest: tonSubmissionManifest,
        destinationBinding: tonSubmissionManifest.destinationBinding,
        destinationBindingHash: HEX32_H,
        publicInputs: sampleTonPublicInputs,
        public_inputs: sampleTonPublicInputs,
        statementHash: HEX32_G,
      }),
    /publicInputs must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalSccpTonSubmissionMetadataBytes({
        manifest: tonSubmissionManifest,
        destinationBinding: tonSubmissionManifest.destinationBinding,
        destinationBindingHash: HEX32_H,
        publicInputs: sampleTonPublicInputs,
        statementHash: HEX32_G,
        statement_hash: HEX32_G,
      }),
    /statementHash must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalSccpTonSubmissionMetadataBytes({
        manifest: { ...tonSubmissionManifest, local_domain: SCCP_DOMAIN_SORA },
        destinationBinding: tonSubmissionManifest.destinationBinding,
        destinationBindingHash: HEX32_H,
        publicInputs: sampleTonPublicInputs,
        statementHash: HEX32_G,
      }),
    /manifest\.localDomain must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalSccpTonSubmissionMetadataBytes({
        manifest: {
          ...tonSubmissionManifest,
          message_backend: tonSubmissionManifest.messageBackend,
        },
        destinationBinding: tonSubmissionManifest.destinationBinding,
        destinationBindingHash: HEX32_H,
        publicInputs: sampleTonPublicInputs,
        statementHash: HEX32_G,
      }),
    /messageBackend must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalSccpTonSubmissionMetadataBytes({
        manifest: {
          ...tonSubmissionManifest,
          verifierBackend: { key: SCCP_TON_CONTRACT_PROOF_BACKEND_V1 },
        },
        destinationBinding: tonSubmissionManifest.destinationBinding,
        destinationBindingHash: HEX32_H,
        publicInputs: sampleTonPublicInputs,
        statementHash: HEX32_G,
      }),
    /verifierBackendKey must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalSccpTonSubmissionMetadataBytes({
        manifest: tonSubmissionManifest,
        destinationBinding: tonSubmissionManifest.destinationBinding,
        destinationBindingHash: HEX32_A,
        publicInputs: sampleTonPublicInputs,
        statementHash: HEX32_G,
      }),
    /destinationBindingHash must match destinationBinding\.bindingHash/,
  );
  assert.throws(
    () =>
      canonicalSccpTonSubmissionMetadataBytes({
        manifest: { ...tonSubmissionManifest, counterpartyDomain: SCCP_DOMAIN_SOL },
        destinationBinding: tonSubmissionManifest.destinationBinding,
        publicInputs: sampleTonPublicInputs,
        statementHash: HEX32_G,
      }),
    /manifest\.counterpartyDomain must be TON/,
  );
  assert.throws(
    () =>
      canonicalSccpTonSubmissionMetadataBytes({
        manifest: { ...tonSubmissionManifest, proofFamily: "debug-proof" },
        destinationBinding: tonSubmissionManifest.destinationBinding,
        publicInputs: sampleTonPublicInputs,
        statementHash: HEX32_G,
      }),
    /proofFamily must be stark-fri-v1/,
  );
  assert.throws(
    () =>
      canonicalSccpTonSubmissionMetadataBytes({
        manifest: { ...tonSubmissionManifest, verifierBackendKey: "debug-ton-contract" },
        destinationBinding: tonSubmissionManifest.destinationBinding,
        publicInputs: sampleTonPublicInputs,
        statementHash: HEX32_G,
      }),
    /verifierBackendKey must be ton-contract-v1/,
  );
  assert.throws(
    () =>
      canonicalSccpTonSubmissionMetadataBytes({
        manifest: tonSubmissionManifest,
        destinationBinding: { ...tonSubmissionManifest.destinationBinding, bindingHash: HEX32_A },
        publicInputs: sampleTonPublicInputs,
        statementHash: HEX32_G,
      }),
    /destinationBinding must match manifest\.destinationBinding/,
  );
  assert.throws(
    () =>
      canonicalSccpTonSubmissionMetadataBytes({
        manifest: tonSubmissionManifest,
        destinationBinding: tonSubmissionManifest.destinationBinding,
        publicInputs: { ...sampleTonPublicInputs, targetDomain: SCCP_DOMAIN_SOL },
        statementHash: HEX32_G,
      }),
    /publicInputs\.targetDomain must be TON/,
  );
  assert.throws(
    () => tronSccpSourceMessageCallData(true, SCCP_DOMAIN_SORA, HEX32_A),
    /sourceDomain must be a u32 domain id/,
  );
  assert.throws(
    () => tronSccpSourceMessageCallData(SCCP_DOMAIN_TRON, false, HEX32_A),
    /targetDomain must be a u32 domain id/,
  );
  assert.deepEqual(
    tronSccpSourceMessageCallData("5", "0", HEX32_A),
    tronSccpSourceMessageCallData(SCCP_DOMAIN_TRON, SCCP_DOMAIN_SORA, HEX32_A),
  );
  for (const [sourceDomain, targetDomain] of [
    ["05", SCCP_DOMAIN_SORA],
    ["0x5", SCCP_DOMAIN_SORA],
    ["+5", SCCP_DOMAIN_SORA],
    [" 5", SCCP_DOMAIN_SORA],
    [5.5, SCCP_DOMAIN_SORA],
    [SCCP_DOMAIN_TRON, "00"],
  ]) {
    assert.throws(
      () => tronSccpSourceMessageCallData(sourceDomain, targetDomain, HEX32_A),
      /must be a u32 domain id/,
    );
  }
});

function sampleWitness(overrides = {}) {
  const witness = {
    targetDomain: SCCP_DOMAIN_SORA,
    finalizedSlot: 321n,
    parentSlot: 320n,
    bankSignatureCount: 8n,
    parentBankHash: `0x${"c0".repeat(32)}`,
    blockhash: "9xQeWvG816bUx9EPfYdLSdJH7Gq2Xv3yQPG8mD3kAcL7",
    bankHash: HEX32_A,
    transactionStatusRoot: HEX32_B,
    messageProofHash: HEX32_C,
    accountInclusionRoot: `0x${"77".repeat(32)}`,
    accountsLtHashChecksum: `0x${"88".repeat(32)}`,
    transactionSignature: SOLANA_SIGNATURE_55,
    emitterProgramId: SOLANA_PROGRAM_42,
    messageId: HEX32_D,
    payloadHash: HEX32_E,
    commitmentRoot: HEX32_F,
    sourceEventDigest: `0x${"34".repeat(32)}`,
    statementHash: HEX32_G,
    destinationBindingHash: HEX32_H,
    ...overrides,
  };
  const hasInclusionBranch =
    overrides.inclusionBranch !== undefined || overrides.inclusion_branch !== undefined;
  const hasTransactionStatusRoot =
    overrides.transactionStatusRoot !== undefined || overrides.transaction_status_root !== undefined;
  if (hasInclusionBranch && !hasTransactionStatusRoot) {
    witness.transactionStatusRoot = solanaSccpTransactionStatusRootFromBranch(witness);
  }
  return witness;
}

function sampleProductionWitness(overrides = {}) {
  const inclusionBranch = overrides.inclusionBranch ?? overrides.inclusion_branch ?? [HEX32_G];
  const blockhash = "0x" + "9a".repeat(32);
  const accountsLtHash = Uint8Array.from(
    { length: 2048 },
    (_, index) => (index % 251) + 1,
  );
  const witness = sampleWitness({
    blockhash,
    accountsLtHash,
    accountsLtHashChecksum: solanaSccpAccountsLtHashChecksum(accountsLtHash),
    bankHash: solanaSccpAgaveBankHash({
      parentBankHash: `0x${"c0".repeat(32)}`,
      bankSignatureCount: 8n,
      blockhash,
      accountsLtHash,
    }),
    sourceStateVerifierHash: HEX32_C,
    sourceAdapterDeploymentHash: HEX32_A,
    sourceAdapterDeploymentReceiptHash: HEX32_B,
    ...overrides,
    inclusionBranch,
  });
  if (
    inclusionBranch.length > 0 &&
    overrides.messageProofHash === undefined &&
    overrides.message_proof_hash === undefined
  ) {
    witness.messageProofHash = solanaSccpMessageProofHash(witness);
  }
  return witness;
}

function sampleSolanaOpenedAccountsLtHashInput(overrides = {}) {
  const voteOpening = {
    address: `0x${"31".repeat(32)}`,
    owner: SCCP_SOLANA_VOTE_PROGRAM_ID,
    lamports: 1_000_000n,
    rentEpoch: 0n,
    executable: false,
    dataHash: `0x${"91".repeat(32)}`,
  };
  const stakeOpening = {
    address: `0x${"32".repeat(32)}`,
    owner: SCCP_SOLANA_STAKE_PROGRAM_ID,
    lamports: 2_000_000n,
    rentEpoch: 0n,
    executable: false,
    dataHash: `0x${"92".repeat(32)}`,
  };
  const stakeHistoryOpening = {
    address: SCCP_SOLANA_STAKE_HISTORY_SYSVAR_ID,
    owner: SCCP_SOLANA_SYSVAR_PROGRAM_ID,
    lamports: 1n,
    rentEpoch: 0n,
    executable: false,
    dataHash: `0x${"93".repeat(32)}`,
  };
  const unopenedOpening = {
    address: `0x${"34".repeat(32)}`,
    owner: SCCP_SOLANA_STAKE_PROGRAM_ID,
    lamports: 3_000_000n,
    rentEpoch: 0n,
    executable: false,
    dataHash: `0x${"94".repeat(32)}`,
  };
  const voteRawData = Uint8Array.from([1, 2, 3]);
  const stakeRawData = Uint8Array.from([4, 5, 6]);
  const stakeHistoryRawData = Uint8Array.from([7, 8, 9]);
  const unopenedRawData = Uint8Array.from([10, 11, 12]);
  const accountsLtHash = solanaSccpAccountsLtHashFromOpenings(
    [voteOpening, stakeOpening, stakeHistoryOpening, unopenedOpening],
    [voteRawData, stakeRawData, stakeHistoryRawData, unopenedRawData],
  );
  return {
    finalizedSlot: 1_296_096n,
    accountInclusionRoot: `0x${"77".repeat(32)}`,
    accountsLtHash,
    accountsLtHashChecksum: solanaSccpAccountsLtHashChecksum(accountsLtHash),
    validatorVoteAccountOpenings: [voteOpening],
    validatorVoteAccountRawData: [voteRawData],
    validatorStakeAccountOpenings: [stakeOpening],
    validatorStakeAccountRawData: [stakeRawData],
    stakeHistorySysvarOpening: stakeHistoryOpening,
    stakeHistorySysvarRawData: stakeHistoryRawData,
    ...overrides,
  };
}

function sampleSolanaAccountsLtHashProofInput(overrides = {}) {
  const opened = sampleSolanaOpenedAccountsLtHashInput(overrides.opened ?? {});
  const parentBankHash = `0x${"c0".repeat(32)}`;
  const blockhash = `0x${"42".repeat(32)}`;
  const bankSignatureCount = 8n;
  const bankHash = solanaSccpAgaveBankHash({
    parentBankHash,
    bankSignatureCount,
    blockhash,
    accountsLtHash: opened.accountsLtHash,
  });
  return {
    ...opened,
    parentSlot: 1_296_095n,
    bankSignatureCount,
    parentBankHash,
    blockhash,
    bankHash,
    transactionStatusRoot: HEX32_B,
    sourceStateVerifierHash: HEX32_A,
    ...overrides,
    opened: undefined,
  };
}

function sampleSolanaFullLightClientAuditProofInput(overrides = {}) {
  const sourceStateVerifierHash = `0x${"99".repeat(32)}`;
  const base = sampleSolanaAccountsLtHashProofInput({
    sourceStateVerifierHash,
  });
  const input = {
    ...sampleSourceRecordInput(SCCP_DOMAIN_SOL),
    sourceStateVerifierHash,
    solanaTowerReplayVerifierHash: `0x${"b1".repeat(32)}`,
    solanaFullAccountsdbLatticeVerifierHash: `0x${"c2".repeat(32)}`,
    solanaBankForkChoiceVerifierHash: `0x${"d3".repeat(32)}`,
    ...base,
    messageProofHash: HEX32_C,
    sourceEventDigest: `0x${"34".repeat(32)}`,
    transactionSignature: SOLANA_SIGNATURE_55,
    emitterProgramId: SOLANA_PROGRAM_42,
    messageId: HEX32_D,
    payloadHash: HEX32_E,
    commitmentRoot: HEX32_F,
    epoch: 3n,
    rootedSlot: 1_296_065n,
    towerVoteSlots: Array.from({ length: Number(SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH) }, (_, index) =>
      1_296_066n + BigInt(index),
    ),
    epochStakeRoot: `0x${"13".repeat(32)}`,
    stakeActivationHash: `0x${"14".repeat(32)}`,
    stakeAccountStateHash: `0x${"15".repeat(32)}`,
    stakeHistoryHash: `0x${"16".repeat(32)}`,
    stakeHistorySysvarAccountHash: `0x${"17".repeat(32)}`,
    accountsLtHashProof: {
      version: 1,
      proofFamily: "stark-fri-v1",
      circuitId: SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1,
      proofBytes: Uint8Array.from([1, 2, 3, 4]),
    },
    ...overrides,
  };
  if (
    overrides.sourceAdapterDeploymentHash === undefined &&
    overrides.source_adapter_deployment_hash === undefined
  ) {
    input.sourceAdapterDeploymentHash = sccpSourceAdapterEngineDeploymentHash(input);
  }
  if (
    overrides.sourceAdapterDeploymentReceiptHash === undefined &&
    overrides.source_adapter_deployment_receipt_hash === undefined
  ) {
    input.sourceAdapterDeploymentReceiptHash =
      input.deploymentReceiptHash ?? input.deployment_receipt_hash;
  }
  return input;
}

test("normalizes Solana SCCP witness input for local proof requests", () => {
  const witness = normalizeSolanaSccpWitness(sampleWitness());

  assert.equal(witness.version, 1);
  assert.equal(witness.sourceDomain, SCCP_DOMAIN_SOL);
  assert.equal(witness.targetDomain, SCCP_DOMAIN_SORA);
  assert.equal(witness.mainnetGenesisHash, SCCP_SOLANA_MAINNET_GENESIS_HASH);
  assert.equal(witness.finalizedSlot, "321");
  assert.equal(witness.parentSlot, "320");
  assert.equal(witness.bankSignatureCount, "8");
  assert.match(witness.blockhash, /^0x[0-9a-f]{64}$/);
  assert.deepEqual(
    canonicalSolanaSccpWitnessBytes(sampleWitness()),
    canonicalSolanaSccpWitnessBytes(sampleWitness({ blockhash: witness.blockhash })),
  );
  assert.equal(
    buildSolanaSccpProofRequest(sampleWitness()).witnessHash,
    buildSolanaSccpProofRequest(sampleWitness({ blockhash: witness.blockhash })).witnessHash,
  );
  assert.equal(
    witness.accountsLtHashProofPublicInputsHash,
    solanaSccpAccountsLtHashProofPublicInputsHash(witness),
  );
  assert.equal(witness.messageId, HEX32_D);
  assert.equal(witness.sourceEventDigest, `0x${"34".repeat(32)}`);
  assert.equal(witness.sourceStateVerifierId, SCCP_SOLANA_MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1);
  assert.equal(witness.sourceStateVerifierHash, SCCP_ZERO_HASH_V1);
  assert.equal(witness.sourceAdapterDeploymentHash, SCCP_ZERO_HASH_V1);
  assert.equal(witness.sourceAdapterDeploymentReceiptHash, SCCP_ZERO_HASH_V1);
  assert.ok(canonicalSolanaSccpWitnessBytes(witness).length > 0);
  for (const [field, expectedError] of [
    ["sourceStateVerifierId", /sourceStateVerifierId/],
    ["sourceStateVerifierHash", /sourceStateVerifierHash/],
    ["sourceAdapterDeploymentHash", /sourceAdapterDeploymentHash/],
    ["sourceAdapterDeploymentReceiptHash", /sourceAdapterDeploymentReceiptHash/],
  ]) {
    assert.throws(
      () => normalizeSolanaSccpWitness(sampleWitness({ [field]: "" })),
      expectedError,
    );
  }
});

test("requires caller-supplied Solana source event digest", () => {
  assert.throws(
    () => normalizeSolanaSccpWitness(sampleWitness({ sourceEventDigest: undefined })),
    /sourceEventDigest must be a hex string/,
  );
  assert.throws(
    () =>
      normalizeSolanaSccpWitness(
        sampleWitness({
          sourceStateVerifierId: "debug-solana-state-verifier",
          sourceStateVerifierHash: HEX32_A,
        }),
      ),
    /sourceStateVerifierId must match Solana AccountsDB verifier profile/,
  );
});

test("derives Solana message proof hash from inclusion witness", () => {
  const inclusionBranch = [HEX32_G];
  const solanaLeafInput = {
    sourceEventDigest: `0x${"34".repeat(32)}`,
    transactionSignature: SOLANA_SIGNATURE_55,
    emitterProgramId: SOLANA_PROGRAM_42,
  };
  const solanaRootInput = {
    ...solanaLeafInput,
    inclusionBranch,
  };
  const transactionStatusRoot = solanaSccpTransactionStatusRootFromBranch(solanaRootInput);
  assert.equal(
    transactionStatusRoot,
    "0xb048ca31d8ad7b2a0d15cbeb81d536350743483d44dd93136e859df93d3863b2",
  );
  const solanaMessageProofInput = {
    ...solanaRootInput,
    transactionStatusRoot,
  };
  const derived = solanaSccpMessageProofHash(solanaMessageProofInput);
  assert.match(derived, /^0x[0-9a-f]{64}$/);
  assert.equal(
    solanaSccpTransactionStatusLeafHash(solanaLeafInput),
    "0x4e12efed6d53466de0596f05aa6cc767df1efd6a4d1549276c4ec8b69118515d",
  );
  for (const [patch, pattern] of [
    [{ source_event_digest: `0x${"34".repeat(32)}` }, /sourceEventDigest must not use multiple aliases/u],
    [{ transaction_signature: SOLANA_SIGNATURE_55 }, /transactionSignature must not use multiple aliases/u],
    [{ emitter_program_id: SOLANA_PROGRAM_42 }, /emitterProgramId must not use multiple aliases/u],
  ]) {
    assert.throws(() => solanaSccpTransactionStatusLeafHash({ ...solanaLeafInput, ...patch }), pattern);
  }
  assert.throws(
    () => solanaSccpTransactionStatusRootFromBranch({ ...solanaRootInput, inclusion_branch: inclusionBranch }),
    /inclusionBranch must not use multiple aliases/u,
  );
  for (const [patch, pattern] of [
    [{ source_event_digest: `0x${"34".repeat(32)}` }, /sourceEventDigest must not use multiple aliases/u],
    [{ receipt_or_message_root: transactionStatusRoot }, /transactionStatusRoot must not use multiple aliases/u],
    [{ transaction_signature: SOLANA_SIGNATURE_55 }, /transactionSignature must not use multiple aliases/u],
    [{ emitter_program_id: SOLANA_PROGRAM_42 }, /emitterProgramId must not use multiple aliases/u],
    [{ inclusion_branch: inclusionBranch }, /inclusionBranch must not use multiple aliases/u],
  ]) {
    assert.throws(() => solanaSccpMessageProofHash({ ...solanaMessageProofInput, ...patch }), pattern);
  }
  assert.throws(
    () =>
      solanaSccpTransactionStatusLeafHash({
        sourceEventDigest: `0x${"34".repeat(32)}`,
        transactionSignature: SOLANA_ZERO_SIGNATURE,
        emitterProgramId: SOLANA_PROGRAM_42,
      }),
    /transactionSignature must not decode to zero/,
  );
  assert.throws(
    () =>
      solanaSccpTransactionStatusLeafHash({
        sourceEventDigest: `0x${"34".repeat(32)}`,
        transactionSignature: SOLANA_SIGNATURE_55,
        emitterProgramId: SOLANA_ZERO_PROGRAM,
      }),
    /emitterProgramId must not decode to zero/,
  );
  assert.ok(
    canonicalSolanaSccpMessageProofBytes(solanaMessageProofInput).length > 0,
  );
  assert.throws(
    () =>
      solanaSccpMessageProofHash({
        sourceEventDigest: `0x${"00".repeat(32)}`,
        transactionStatusRoot,
        transactionSignature: SOLANA_SIGNATURE_55,
        emitterProgramId: SOLANA_PROGRAM_42,
        inclusionBranch,
      }),
    /sourceEventDigest must not be zero/,
  );
  assert.throws(
    () =>
      solanaSccpMessageProofHash({
        sourceEventDigest: `0x${"34".repeat(32)}`,
        transactionStatusRoot: `0x${"00".repeat(32)}`,
        transactionSignature: SOLANA_SIGNATURE_55,
        emitterProgramId: SOLANA_PROGRAM_42,
        inclusionBranch,
      }),
    /transactionStatusRoot must not be zero/,
  );
  assert.throws(
    () =>
      solanaSccpMessageProofHash({
        sourceEventDigest: `0x${"34".repeat(32)}`,
        transactionStatusRoot,
        transactionSignature: SOLANA_ZERO_SIGNATURE,
        emitterProgramId: SOLANA_PROGRAM_42,
        inclusionBranch,
      }),
    /transactionSignature must not decode to zero/,
  );
  assert.throws(
    () =>
      solanaSccpMessageProofHash({
        sourceEventDigest: `0x${"34".repeat(32)}`,
        transactionStatusRoot,
        transactionSignature: SOLANA_SIGNATURE_55,
        emitterProgramId: SOLANA_ZERO_PROGRAM,
        inclusionBranch,
      }),
    /emitterProgramId must not decode to zero/,
  );
  assert.notEqual(
    derived,
    solanaSccpMessageProofHash({
      sourceEventDigest: `0x${"34".repeat(32)}`,
      transactionStatusRoot,
      transactionSignature:
        "2AXDGYSE4f2sz7tvMMzyHvUfcoJmxudvdhBcmiUSo6ijwfYmfZYsKRxboQMPh3R4kUhXRVdtSXFXMheka4Rc4P2",
      emitterProgramId: SOLANA_PROGRAM_42,
      inclusionBranch,
    }),
  );
  assert.notEqual(
    derived,
    solanaSccpMessageProofHash({
      sourceEventDigest: `0x${"34".repeat(32)}`,
      transactionStatusRoot,
      transactionSignature: SOLANA_SIGNATURE_55,
      emitterProgramId: "8qbHbw2BbbTHBW1sbeqakYXVKRQM8Ne7pLK7m6CVfeR",
      inclusionBranch,
    }),
  );
  assert.equal(
    normalizeSolanaSccpWitness(sampleWitness({ messageProofHash: undefined, inclusionBranch }))
      .messageProofHash,
    derived,
  );
  const normalized = normalizeSolanaSccpWitness(
    sampleWitness({ messageProofHash: "", inclusionBranch }),
  );
  assert.equal(normalized.messageProofHash, derived);
  assert.deepEqual(normalized.inclusionBranch, [HEX32_G]);
  assert.throws(
    () =>
      normalizeSolanaSccpWitness(
        sampleWitness({ sourceEventDigest: `0x${"00".repeat(32)}`, inclusionBranch }),
      ),
    /sourceEventDigest must not be zero/,
  );
  assert.throws(
    () =>
      normalizeSolanaSccpWitness(
        sampleWitness({ transactionSignature: SOLANA_ZERO_SIGNATURE, inclusionBranch }),
      ),
    /transactionSignature must not decode to zero/,
  );
  assert.throws(
    () =>
      normalizeSolanaSccpWitness(
        sampleWitness({ emitterProgramId: SOLANA_ZERO_PROGRAM, inclusionBranch }),
      ),
    /emitterProgramId must not decode to zero/,
  );
  assert.throws(
    () => normalizeSolanaSccpWitness(sampleWitness({ messageProofHash: HEX32_C, inclusionBranch })),
    /messageProofHash must match inclusionBranch/,
  );
  assert.ok(
    canonicalSolanaSccpWitnessBytes(normalized).length >
      canonicalSolanaSccpWitnessBytes(sampleWitness()).length,
  );
  assert.throws(
    () =>
      solanaSccpMessageProofHash({
        sourceEventDigest: `0x${"34".repeat(32)}`,
        transactionStatusRoot,
        transactionSignature: SOLANA_SIGNATURE_55,
        emitterProgramId: SOLANA_PROGRAM_42,
        inclusionBranch: [],
      }),
    /inclusionBranch must not be empty/,
  );
  assert.throws(
    () =>
      solanaSccpMessageProofHash({
        sourceEventDigest: `0x${"34".repeat(32)}`,
        transactionStatusRoot,
        transactionSignature: SOLANA_SIGNATURE_55,
        emitterProgramId: SOLANA_PROGRAM_42,
        inclusionBranch: [`0x${"ab".repeat(31)}`],
      }),
    /inclusionBranch\[0\] must be 32 bytes/,
  );
  assert.throws(
    () =>
      solanaSccpMessageProofHash({
        sourceEventDigest: `0x${"34".repeat(32)}`,
        transactionStatusRoot,
        transactionSignature: "not-a-solana-signature",
        emitterProgramId: SOLANA_PROGRAM_42,
        inclusionBranch,
      }),
    /transactionSignature must be canonical base58/,
  );
});

test("derives Solana epoch stake root for finalized-slot vote witnesses", () => {
  const input = {
    epoch: 3n,
    validatorPublicKeys: [`0x${"11".repeat(32)}`, `0x${"22".repeat(32)}`],
    validatorStakes: [1n, 2n],
  };

  assert.equal(SCCP_SOLANA_MAINNET_SLOTS_PER_EPOCH, 432_000n);
  assert.equal(solanaSccpMainnetEpochForSlot(864_000n), 2n);
  assert.equal(canonicalSolanaSccpEpochStakeRootBytes(input).length, 134);
  assert.equal(
    solanaSccpEpochStakeRoot(input),
    "0x1d86a5ecfac6e63bfcefdc1a3bfefd962a33e2a4cf65cd4e8518bcebea771f0a",
  );
  assert.equal(
    solanaSccpEpochStakeRoot({
      finalizedSlot: 1_296_000n,
      validator_public_keys: input.validatorPublicKeys,
      validator_stakes: input.validatorStakes,
    }),
    solanaSccpEpochStakeRoot(input),
  );
  assert.throws(
    () =>
      solanaSccpEpochStakeRoot({
        ...input,
        validator_public_keys: input.validatorPublicKeys,
      }),
    /validatorPublicKeys must not use multiple aliases/,
  );
  assert.throws(
    () =>
      solanaSccpEpochStakeRoot({
        ...input,
        validator_stakes: input.validatorStakes,
      }),
    /validatorStakes must not use multiple aliases/,
  );
  assert.throws(
    () =>
      solanaSccpEpochStakeRoot({
        ...input,
        validatorPublicKeys: [`0x${"11".repeat(31)}`],
        validatorStakes: [1n],
      }),
    /validatorPublicKeys\[0\] must be 32 bytes/,
  );
  assert.throws(
    () =>
      solanaSccpEpochStakeRoot({
        ...input,
        validatorPublicKeys: [`0x${"00".repeat(32)}`],
        validatorStakes: [1n],
      }),
    /validatorPublicKeys\[0\] must not be zero/,
  );
  const oversizedValidatorPublicKeys = Array.from(
    { length: SCCP_SOLANA_MAX_VALIDATORS + 1 },
    (_, index) => `0x${"00".repeat(24)}${(index + 1).toString(16).padStart(16, "0")}`,
  );
  assert.throws(
    () =>
      solanaSccpEpochStakeRoot({
        ...input,
        validatorPublicKeys: oversizedValidatorPublicKeys,
        validatorStakes: Array.from({ length: oversizedValidatorPublicKeys.length }, () => 1n),
      }),
    /validatorPublicKeys must contain 1\.\.8192 entries/,
  );
});

test("derives Solana stake activation hash for active vote witnesses", () => {
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
  assert.throws(
    () => solanaSccpStakeActivationHash({ ...input, validator_epoch: input.epoch }),
    /epoch must not use multiple aliases/,
  );
  assert.throws(
    () =>
      solanaSccpStakeActivationHash({
        ...input,
        activation_epochs: input.validatorActivationEpochs,
      }),
    /validatorActivationEpochs must not use multiple aliases/,
  );
  assert.throws(
    () =>
      solanaSccpStakeActivationHash({
        ...input,
        deactivation_epochs: input.validatorDeactivationEpochs,
      }),
    /validatorDeactivationEpochs must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpStakeActivationHash({ ...input, validatorActivationEpochs: [4n, 2n] }),
    /validatorActivationEpochs\[0\] must be less than epoch/,
  );
  assert.throws(
    () => solanaSccpStakeActivationHash({ ...input, validatorActivationEpochs: [3n, 2n] }),
    /validatorActivationEpochs\[0\] must be less than epoch/,
  );
  assert.throws(
    () => solanaSccpStakeActivationHash({ ...input, validatorDeactivationEpochs: [(1n << 64n) - 1n, 2n] }),
    /validatorDeactivationEpochs\[1\] must be greater than activation epoch/,
  );
  assert.equal(
    solanaSccpStakeActivationHash({ ...input, validatorDeactivationEpochs: [(1n << 64n) - 1n, 3n] }).length,
    66,
  );
  assert.throws(
    () => solanaSccpStakeActivationHash({ ...input, validatorActivationEpochs: [0n] }),
    /validator activation epochs must match/,
  );
});

test("derives Solana account opening hash for vote and stake account metadata", () => {
  const input = {
    address: `0x${"31".repeat(32)}`,
    owner: SCCP_SOLANA_VOTE_PROGRAM_ID,
    lamports: 1_000_000n,
    rentEpoch: 0n,
    executable: false,
    dataHash: `0x${"71".repeat(32)}`,
  };

  assert.equal(canonicalSolanaSccpAccountOpeningBytes(input).length, 122);
  const hash = solanaSccpAccountOpeningHash(input);
  assert.match(hash, /^0x[0-9a-f]{64}$/);
  assert.notEqual(hash, solanaSccpAccountOpeningHash({ ...input, owner: SCCP_SOLANA_STAKE_PROGRAM_ID }));
  assert.notEqual(hash, solanaSccpAccountOpeningHash({ ...input, executable: true }));
  assert.throws(
    () => solanaSccpAccountOpeningHash({ ...input, accountAddress: input.address }),
    /address must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpAccountOpeningHash({ ...input, ownerProgramId: input.owner }),
    /owner must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpAccountOpeningHash({ ...input, rent_epoch: input.rentEpoch }),
    /rentEpoch must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpAccountOpeningHash({ ...input, data_hash: input.dataHash }),
    /dataHash must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpAccountOpeningHash({ ...input, lamports: 0n }),
    /lamports must be greater than zero/,
  );
});

test("derives Solana Agave account lattice hash helpers", () => {
  const opening = {
    address: `0x${"31".repeat(32)}`,
    owner: SCCP_SOLANA_VOTE_PROGRAM_ID,
    lamports: 1_000_000n,
    rentEpoch: 0n,
    executable: false,
    dataHash: `0x${"71".repeat(32)}`,
  };
  const rawData = Uint8Array.from([1, 2, 3, 4, 5]);

  const accountLtHash = solanaSccpAccountLtHash(opening, rawData);
  assert.equal(accountLtHash.length, 2048);
  assert.match(solanaSccpAccountsLtHashChecksum(accountLtHash), /^0x[0-9a-f]{64}$/);
  assert.deepEqual(
    solanaSccpAccountLtHash({ ...opening, rentEpoch: 1n }, rawData),
    accountLtHash,
  );
  assert.notDeepEqual(
    solanaSccpAccountLtHash({ ...opening, lamports: opening.lamports + 1n }, rawData),
    accountLtHash,
  );
  assert.deepEqual(
    solanaSccpAccountLtHash({ ...opening, lamports: 0n }, rawData),
    new Uint8Array(2048),
  );
  assert.throws(
    () => solanaSccpAccountLtHash({ ...opening, executable: "false" }, rawData),
    /executable must be a boolean/,
  );
  assert.throws(
    () => solanaSccpAccountLtHash({ ...opening, accountAddress: opening.address }, rawData),
    /address must not use multiple aliases/,
  );
  assert.equal(
    solanaSccpAccountsLtHashFromOpenings([opening], [rawData]).length,
    2048,
  );
});

test("derives opened Solana AccountsLtHash contribution and residual bindings", () => {
  const voteOpening = {
    address: `0x${"31".repeat(32)}`,
    owner: SCCP_SOLANA_VOTE_PROGRAM_ID,
    lamports: 1_000_000n,
    rentEpoch: 0n,
    executable: false,
    dataHash: `0x${"91".repeat(32)}`,
  };
  const stakeOpening = {
    address: `0x${"32".repeat(32)}`,
    owner: SCCP_SOLANA_STAKE_PROGRAM_ID,
    lamports: 2_000_000n,
    rentEpoch: 0n,
    executable: false,
    dataHash: `0x${"92".repeat(32)}`,
  };
  const stakeHistoryOpening = {
    address: SCCP_SOLANA_STAKE_HISTORY_SYSVAR_ID,
    owner: SCCP_SOLANA_SYSVAR_PROGRAM_ID,
    lamports: 1n,
    rentEpoch: 0n,
    executable: false,
    dataHash: `0x${"93".repeat(32)}`,
  };
  const unopenedOpening = {
    address: `0x${"34".repeat(32)}`,
    owner: SCCP_SOLANA_STAKE_PROGRAM_ID,
    lamports: 3_000_000n,
    rentEpoch: 0n,
    executable: false,
    dataHash: `0x${"94".repeat(32)}`,
  };
  const voteRawData = Uint8Array.from([1, 2, 3]);
  const stakeRawData = Uint8Array.from([4, 5, 6]);
  const stakeHistoryRawData = Uint8Array.from([7, 8, 9]);
  const unopenedRawData = Uint8Array.from([10, 11, 12]);
  const accountsLtHash = solanaSccpAccountsLtHashFromOpenings(
    [voteOpening, stakeOpening, stakeHistoryOpening, unopenedOpening],
    [voteRawData, stakeRawData, stakeHistoryRawData, unopenedRawData],
  );
  const openedLtHash = solanaSccpAccountsLtHashFromOpenings(
    [voteOpening, stakeOpening, stakeHistoryOpening],
    [voteRawData, stakeRawData, stakeHistoryRawData],
  );
  const unopenedLtHash = solanaSccpAccountsLtHashFromOpenings(
    [unopenedOpening],
    [unopenedRawData],
  );
  const input = {
    finalizedSlot: 1_296_096n,
    accountInclusionRoot: `0x${"77".repeat(32)}`,
    accountsLtHash,
    accountsLtHashChecksum: solanaSccpAccountsLtHashChecksum(accountsLtHash),
    validatorVoteAccountOpenings: [voteOpening],
    validatorVoteAccountRawData: [voteRawData],
    validatorStakeAccountOpenings: [stakeOpening],
    validatorStakeAccountRawData: [stakeRawData],
    stakeHistorySysvarOpening: stakeHistoryOpening,
    stakeHistorySysvarRawData: stakeHistoryRawData,
  };

  assert.deepEqual(solanaSccpAccountsLtHashOpenedResidual(input), unopenedLtHash);
  assert.equal(
    solanaSccpAccountsLtHashOpenedResidualChecksum(input),
    solanaSccpAccountsLtHashChecksum(unopenedLtHash),
  );
  assert.equal(
    canonicalSolanaSccpAccountsLtHashOpenedContributionsBytes(input).length,
    10_696,
  );
  assert.equal(
    solanaSccpAccountsLtHashOpenedContributionsHash(input),
    "0x07270072f8b70b755ed491c1582b40050a484edd67752a8a0bbbd97aa175d4f9",
  );
  assert.notEqual(
    solanaSccpAccountsLtHashOpenedContributionsHash({
      ...input,
      validatorVoteAccountRawData: [Uint8Array.from([1, 2, 4])],
    }),
    solanaSccpAccountsLtHashOpenedContributionsHash(input),
  );
  assert.throws(
    () => solanaSccpAccountsLtHashOpenedContributionsHash({
      ...input,
      accountsLtHashChecksum: `0x${"88".repeat(32)}`,
    }),
    /accountsLtHashChecksum must match accountsLtHash/,
  );
  assert.throws(
    () => solanaSccpAccountsLtHashOpenedContributionsHash({
      ...input,
      finalized_slot: input.finalizedSlot,
    }),
    /finalizedSlot must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpAccountsLtHashOpenedContributionsHash({
      ...input,
      accounts_root: input.accountInclusionRoot,
    }),
    /accountInclusionRoot must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpAccountsLtHashOpenedContributionsHash({
      ...input,
      accounts_lt_hash_root: input.accountsLtHashChecksum,
    }),
    /accountsLtHashChecksum must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpAccountsLtHashOpenedContributionsHash({
      ...input,
      accounts_lt_hash: input.accountsLtHash,
    }),
    /accountsLtHash must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpAccountsLtHashOpenedContributionsHash({
      ...input,
      vote_account_openings: input.validatorVoteAccountOpenings,
    }),
    /validatorVoteAccountOpenings must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpAccountsLtHashOpenedContributionsHash({
      ...input,
      stake_history_sysvar_opening: input.stakeHistorySysvarOpening,
    }),
    /stakeHistorySysvarOpening must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpAccountsLtHashOpenedContributionsHash({
      ...input,
      accountsLtHash: openedLtHash,
      accountsLtHashChecksum: solanaSccpAccountsLtHashChecksum(openedLtHash),
    }),
    /openedAccountsLtHashResidual must not be zero/,
  );
  assert.throws(
    () => solanaSccpAccountsLtHashOpenedContributionsHash({
      ...input,
      validatorStakeAccountOpenings: [
        { ...stakeOpening, address: voteOpening.address },
      ],
    }),
    /opened account addresses must be unique/,
  );
  assert.throws(
    () => solanaSccpAccountsLtHashOpenedContributionsHash({
      ...input,
      validatorVoteAccountOpenings: [{ ...voteOpening, lamports: 0n }],
    }),
    /lamports must be greater than zero/,
  );
  assert.throws(
    () => solanaSccpAccountsLtHashOpenedContributionsHash({
      ...input,
      validatorVoteAccountOpenings: Array.from(
        { length: SCCP_SOLANA_MAX_VALIDATORS + 1 },
        () => voteOpening,
      ),
      validatorVoteAccountRawData: Array.from(
        { length: SCCP_SOLANA_MAX_VALIDATORS + 1 },
        () => voteRawData,
      ),
    }),
    /validatorVoteAccountOpenings.*at most/,
  );
});

test("builds Solana AccountsLtHash source-state proof requests", () => {
  const input = sampleSolanaAccountsLtHashProofInput();
  const request = buildSolanaSccpAccountsLtHashProofRequest(input);

  assert.throws(
    () =>
      buildSolanaSccpAccountsLtHashProofRequest({
        ...input,
        finalized_slot: input.finalizedSlot,
      }),
    /finalizedSlot must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildSolanaSccpAccountsLtHashProofRequest({
        ...input,
        blockhashBytes: Buffer.from(input.blockhash.slice(2), "hex"),
      }),
    /blockhash must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildSolanaSccpAccountsLtHashProofRequest({
        ...input,
        source_state_verifier_hash: input.sourceStateVerifierHash,
      }),
    /sourceStateVerifierHash must not use multiple aliases/,
  );

  assertImmutableFastpqProofRequest(request, [
    "statementBytes",
    "accountCommitmentBytes",
    "verificationContextBytes",
    "schemaDescriptor",
  ]);
  assert.equal(request.version, 1);
  assert.equal(request.proofFamily, "stark-fri-v1");
  assert.equal(request.circuitId, SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1);
  assert.equal(request.parameterSet, "fastpq-lane-balanced");
  assert.equal(request.sourceStateVerifierId, SCCP_SOLANA_MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1);
  assert.equal(request.sourceStateVerifierHash, HEX32_A);
  assert.equal(
    request.accountsLtHashProofPublicInputsHash,
    solanaSccpAccountsLtHashProofPublicInputsHash(input),
  );
  assert.equal(
    request.openedAccountsLtHashContributionsHash,
    solanaSccpAccountsLtHashOpenedContributionsHash(input),
  );
  const voteLtHash = solanaSccpAccountLtHash(
    input.validatorVoteAccountOpenings[0],
    input.validatorVoteAccountRawData[0],
  );
  const stakeLtHash = solanaSccpAccountLtHash(
    input.validatorStakeAccountOpenings[0],
    input.validatorStakeAccountRawData[0],
  );
  const stakeHistoryLtHash = solanaSccpAccountLtHash(
    input.stakeHistorySysvarOpening,
    input.stakeHistorySysvarRawData,
  );
  const precomputedOpenedRowsInput = {
    ...input,
    validatorVoteAccountLtHashes: [voteLtHash],
    validatorStakeAccountLtHashes: [stakeLtHash],
    stakeHistorySysvarAccountLtHash: stakeHistoryLtHash,
  };
  assert.equal(
    solanaSccpAccountsLtHashOpenedContributionsHash(precomputedOpenedRowsInput),
    request.openedAccountsLtHashContributionsHash,
  );
  const wrongVoteLtHash = new Uint8Array(voteLtHash);
  wrongVoteLtHash[0] ^= 1;
  assert.throws(
    () =>
      solanaSccpAccountsLtHashOpenedContributionsHash({
        ...input,
        validatorVoteAccountLtHashes: [wrongVoteLtHash],
      }),
    /validatorVoteAccountLtHashes\[0\] must match/,
  );
  assert.throws(
    () =>
      solanaSccpAccountsLtHashOpenedResidualChecksum({
        ...input,
        stakeHistorySysvarAccountLtHash: wrongVoteLtHash,
      }),
    /stakeHistorySysvarAccountLtHash must match/,
  );
  assert.equal(
    request.openedAccountsLtHashResidualChecksum,
    solanaSccpAccountsLtHashOpenedResidualChecksum(input),
  );
  assert.deepEqual(
    request.statementBytes,
    canonicalSolanaSccpAccountsLtHashProofPublicInputsBytes(input),
  );
  assert.throws(
    () => canonicalSolanaSccpAccountsLtHashProofPublicInputsBytes({ ...input, bankHash: HEX32_C }),
    /bankHash must match Agave bank hash inputs/,
  );
  assert.throws(
    () =>
      canonicalSolanaSccpAccountsLtHashProofPublicInputsBytes({
        ...input,
        sourceDomain: SCCP_DOMAIN_SOL,
        source_domain: SCCP_DOMAIN_SOL,
      }),
    /sourceDomain must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalSolanaSccpAccountsLtHashProofPublicInputsBytes({
        ...input,
        finalized_slot: input.finalizedSlot,
      }),
    /finalizedSlot must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalSolanaSccpAccountsLtHashProofPublicInputsBytes({
        ...input,
        blockhashBytes: Buffer.from(input.blockhash.slice(2), "hex"),
      }),
    /blockhash must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalSolanaSccpAccountsLtHashProofPublicInputsBytes({
        ...input,
        accountsRoot: input.accountInclusionRoot,
      }),
    /accountInclusionRoot must not use multiple aliases/,
  );
  assert.throws(
    () =>
      solanaSccpAccountsLtHashProofPublicInputsHash({
        ...input,
        accountsLtHashChecksum: HEX32_C,
      }),
    /accountsLtHashChecksum must match accountsLtHash/,
  );
  assert.deepEqual(
    request.accountCommitmentBytes,
    canonicalSolanaSccpAccountsLtHashCommitmentBytes(input),
  );
  assert.deepEqual(
    request.verificationContextBytes,
    canonicalSolanaSccpAccountsLtHashVerificationContextBytes(input),
  );
  assert.deepEqual(
    request.publicInputColumns,
    solanaSccpAccountsLtHashPublicInputColumns(input),
  );
  assert.equal(request.publicInputColumns[1][0], SOLANA_MAINNET_GENESIS_PUBLIC_INPUT);
  assert.equal(
    request.publicInputColumns.at(-2)[0],
    request.openedAccountsLtHashContributionsHash,
  );
  assert.equal(
    request.publicInputColumns.at(-1)[0],
    request.openedAccountsLtHashResidualChecksum,
  );
  assert.deepEqual(
    request.schemaDescriptor,
    solanaSccpAccountsLtHashOpenVerifySchemaDescriptor(input),
  );
  assert.ok(
    Buffer.from(request.schemaDescriptor).includes(
      Buffer.from("opened_accounts_lt_hash_residual_checksum"),
    ),
  );
  assert.ok(Buffer.from(request.schemaDescriptor).includes(Buffer.from("source_state_verifier_id")));
  assert.ok(Buffer.from(request.schemaDescriptor).includes(Buffer.from("mainnet_genesis_hash")));
  assert.ok(
    Buffer.from(request.schemaDescriptor).includes(
      Buffer.from(SCCP_SOLANA_MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1),
    ),
  );
  assert.ok(Buffer.from(request.schemaDescriptor).includes(Buffer.from("source_state_verifier_hash")));
  assert.ok(
    Buffer.from(request.schemaDescriptor).includes(Buffer.from(HEX32_A.slice(2), "hex")),
  );
  assert.deepEqual(
    request.fastpqTransitions.map((transition) => transition.key),
    [
      "sccp:solana:accounts-lt:v1:statement",
      "sccp:solana:accounts-lt:v1:accounts",
      "sccp:solana:accounts-lt:v1:opened-contributions",
      "sccp:solana:accounts-lt:v1:residual",
      "sccp:solana:accounts-lt:v1:context",
    ],
  );
  assert.equal(request.fastpqPublicInputs.oldRoot, input.parentBankHash);
  assert.equal(request.fastpqPublicInputs.newRoot, input.bankHash);
  const proofCapsule = wrapSolanaSccpSourceStateVerificationProof(
    new Uint8Array([1, 2, 3]),
    request,
  );
  assert.equal(Object.isFrozen(proofCapsule), true);
  assert.equal(proofCapsule.version, 1);
  assert.equal(proofCapsule.proofFamily, SCCP_STARK_FRI_PROOF_FAMILY_V1);
  assert.equal(proofCapsule.circuitId, SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1);
  assert.deepEqual(proofCapsule.proofBytes, new Uint8Array([1, 2, 3]));
  assert.equal(proofCapsule.proofBase64, "AQID");
  assert.match(solanaSccpAccountsLtHashProofHash(proofCapsule), /^0x[0-9a-f]{64}$/);
  assert.throws(
    () =>
      canonicalSolanaSccpSourceStateVerificationProofBytes({
        ...proofCapsule,
        proofBase64: "AAAA",
      }),
    /sourceStateProof\.proofBase64 must match sourceStateProof\.proofBytes/,
  );
  assert.throws(
    () =>
      canonicalSolanaSccpSourceStateVerificationProofBytes({
        ...proofCapsule,
        proof_base64: proofCapsule.proofBase64,
      }),
    /sourceStateProof\.proofBase64 must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalSolanaSccpSourceStateVerificationProofBytes({
        ...proofCapsule,
        circuit_id: proofCapsule.circuitId,
      }),
    /sourceStateProof\.circuitId must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalSolanaSccpSourceStateVerificationProofBytes({
        ...proofCapsule,
        proof_bytes: proofCapsule.proofBytes,
      }),
    /sourceStateProof\.proofBytes must not use multiple aliases/,
  );
  const exposedProofBytes = proofCapsule.proofBytes;
  exposedProofBytes[0] = 9;
  assert.deepEqual(proofCapsule.proofBytes, new Uint8Array([1, 2, 3]));
  assert.throws(
    () => wrapSolanaSccpSourceStateVerificationProof(new Uint8Array([0, 0]), request),
    /all zero/,
  );
  const oversizedProofBytes = new Uint8Array(SCCP_SOURCE_STATE_MAX_PROOF_BYTES + 1).fill(1);
  assert.throws(
    () => wrapSolanaSccpSourceStateVerificationProof(oversizedProofBytes, request),
    /at most/,
  );
  assert.throws(
    () =>
      canonicalSolanaSccpSourceStateVerificationProofBytes({
        ...proofCapsule,
        proofBytes: oversizedProofBytes,
      }),
    /at most/,
  );
  assert.throws(
    () =>
      canonicalSolanaSccpSourceStateVerificationProofBytes({
        version: 1,
        proofFamily: "x".repeat(SCCP_SOURCE_STATE_MAX_PROOF_LABEL_BYTES + 1),
        circuitId: request.circuitId,
        proofBytes: new Uint8Array([1]),
      }),
    /proofFamily.*at most/,
  );
  assert.throws(
    () =>
      canonicalSolanaSccpSourceStateVerificationProofBytes({
        version: 1,
        proofFamily: SCCP_STARK_FRI_PROOF_FAMILY_V1,
        circuitId: "x".repeat(SCCP_SOURCE_STATE_MAX_PROOF_LABEL_BYTES + 1),
        proofBytes: new Uint8Array([1]),
      }),
    /circuitId.*at most/,
  );
  const wrongGenesisRequest = mutableFastpqProofRequest(request);
  wrongGenesisRequest.publicInputColumns[1][0] = HEX32_A;
  assert.throws(
    () => wrapSolanaSccpSourceStateVerificationProof(new Uint8Array([1]), wrongGenesisRequest),
    /mainnet_genesis_hash/,
  );
  const wrongResidualColumnRequest = mutableFastpqProofRequest(request);
  wrongResidualColumnRequest.publicInputColumns.at(-1)[0] = HEX32_C;
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(new Uint8Array([1]), wrongResidualColumnRequest),
    /opened_accounts_lt_hash_residual_checksum/,
  );
  const staleAccountsHashRequest = mutableFastpqProofRequest(request);
  staleAccountsHashRequest.accountsLtHashProofPublicInputsHash = HEX32_C;
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(new Uint8Array([1]), staleAccountsHashRequest),
    /accountsLtHashProofPublicInputsHash must match request\.statementBytes/,
  );
  const wrongAccountsDsidRequest = mutableFastpqProofRequest(request);
  wrongAccountsDsidRequest.fastpqPublicInputs.dsid = "0x00000000000000000000000000000000";
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(new Uint8Array([1]), wrongAccountsDsidRequest),
    /fastpqPublicInputs\.dsid/,
  );
  const wrongAccountsTxSetRequest = mutableFastpqProofRequest(request);
  wrongAccountsTxSetRequest.fastpqPublicInputs.txSetHash = HEX32_C;
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(new Uint8Array([1]), wrongAccountsTxSetRequest),
    /fastpqPublicInputs\.txSetHash/,
  );
  const duplicateSourceDomainAliasRequest = mutableFastpqProofRequest(request);
  duplicateSourceDomainAliasRequest.source_domain = duplicateSourceDomainAliasRequest.sourceDomain;
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(
        new Uint8Array([1]),
        duplicateSourceDomainAliasRequest,
      ),
    /request\.sourceDomain.*multiple aliases/,
  );
  const duplicateFastpqAliasRequest = mutableFastpqProofRequest(request);
  duplicateFastpqAliasRequest.fastpqPublicInputs.tx_set_hash =
    duplicateFastpqAliasRequest.fastpqPublicInputs.txSetHash;
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(
        new Uint8Array([1]),
        duplicateFastpqAliasRequest,
      ),
    /request\.fastpqPublicInputs\.txSetHash.*multiple aliases/,
  );
  const duplicateTransitionAliasRequest = mutableFastpqProofRequest(request);
  duplicateTransitionAliasRequest.fastpqTransitions[0].new_value =
    duplicateTransitionAliasRequest.fastpqTransitions[0].newValue;
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(
        new Uint8Array([1]),
        duplicateTransitionAliasRequest,
      ),
    /request\.fastpqTransitions\[0\]\.newValue.*multiple aliases/,
  );
  const wrongTransitionRequest = mutableFastpqProofRequest(request);
  wrongTransitionRequest.fastpqTransitions[0].newValue = "0x00";
  assert.throws(
    () => wrapSolanaSccpSourceStateVerificationProof(new Uint8Array([1]), wrongTransitionRequest),
    /canonical Solana source-state request/,
  );
  const wrongOldValueTransitionRequest = mutableFastpqProofRequest(request);
  wrongOldValueTransitionRequest.fastpqTransitions[0].oldValue = "0x00";
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(
        new Uint8Array([1]),
        wrongOldValueTransitionRequest,
      ),
    /canonical Solana source-state request/,
  );
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(new Uint8Array([1]), {
        ...request,
        circuitId: SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1,
      }),
    /OpenVerify circuit/,
  );
  const requestWithoutSourceDomain = { ...request };
  delete requestWithoutSourceDomain.sourceDomain;
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(
        new Uint8Array([1]),
        requestWithoutSourceDomain,
      ),
    /sourceDomain is required/,
  );
  const requestWithoutStatement = { ...request };
  delete requestWithoutStatement.statementBytes;
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(new Uint8Array([1]), requestWithoutStatement),
    /request\.statementBytes is required/,
  );
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(new Uint8Array([1]), {
        ...request,
        parameterSet: "debug",
      }),
    /request\.parameterSet/,
  );
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(new Uint8Array([1]), {
        ...request,
        sourceStateVerifierHash: SCCP_SOLANA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1,
      }),
    /Solana template verifier hash/,
  );
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(new Uint8Array([1]), {
        ...request,
        openedAccountsLtHashResidualChecksum: SCCP_ZERO_HASH_V1,
      }),
    /request\.openedAccountsLtHashResidualChecksum must not be zero/,
  );
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(new Uint8Array([1]), {
        ...request,
        parentSlot: request.finalizedSlot,
      }),
    /direct parent/,
  );

  const zeroAccountsLtHash = new Uint8Array(2048);
  const zeroAccountsLtHashChecksum = solanaSccpAccountsLtHashChecksum(zeroAccountsLtHash);
  assert.match(zeroAccountsLtHashChecksum, /^0x[0-9a-f]{64}$/);
  assert.throws(
    () =>
      solanaSccpAgaveBankHash({
        parentBankHash: input.parentBankHash,
        bankSignatureCount: input.bankSignatureCount,
        blockhash: input.blockhash,
        accountsLtHash: zeroAccountsLtHash,
      }),
    /accountsLtHash must not be zero/,
  );
  assert.throws(
    () =>
      solanaSccpAccountsLtHashOpenedContributionsHash({
        ...input,
        accountsLtHash: zeroAccountsLtHash,
        accountsLtHashChecksum: zeroAccountsLtHashChecksum,
      }),
    /accountsLtHash must not be zero/,
  );

  assert.throws(
    () =>
      buildSolanaSccpAccountsLtHashProofRequest({
        ...input,
        sourceStateVerifierHash: SCCP_ZERO_HASH_V1,
      }),
    /sourceStateVerifierHash must not be zero/,
  );
  assert.throws(
    () =>
      buildSolanaSccpAccountsLtHashProofRequest({
        ...input,
        sourceStateVerifierHash: SCCP_SOLANA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1,
      }),
    /Solana template verifier hash/,
  );
  assert.throws(
    () => buildSolanaSccpAccountsLtHashProofRequest({ ...input, bankHash: HEX32_C }),
    /bankHash must match Agave bank hash inputs/,
  );
  const snakeAccountsLtHashInput = { ...input, accounts_lt_hash: input.accountsLtHash };
  delete snakeAccountsLtHashInput.accountsLtHash;
  const snakeAccountsLtHashRequest =
    buildSolanaSccpAccountsLtHashProofRequest(snakeAccountsLtHashInput);
  assert.equal(
    snakeAccountsLtHashRequest.openedAccountsLtHashContributionsHash,
    request.openedAccountsLtHashContributionsHash,
  );
  assert.equal(
    snakeAccountsLtHashRequest.openedAccountsLtHashResidualChecksum,
    request.openedAccountsLtHashResidualChecksum,
  );
});

test("builds Solana full light-client audit role proof requests", () => {
  const input = sampleSolanaFullLightClientAuditProofInput();
  const requests = buildSolanaSccpFullLightClientAuditProofRequests(input);
  const finalityContextHash = solanaSccpFinalityContextHash(input);
  const accountsLtHashProofHash = solanaSccpAccountsLtHashProofHash(
    input.accountsLtHashProof,
  );
  assert.throws(
    () =>
      buildSolanaSccpTowerReplayProofRequest(
        sampleSolanaFullLightClientAuditProofInput({
          solanaTowerReplayVerifierHash: requests.towerReplay.auditStatementHash,
        }),
      ),
    /role-separated/,
  );
  assert.throws(
    () =>
      buildSolanaSccpFullLightClientAuditProofRequests({
        ...input,
        tower_vote_slots: input.towerVoteSlots,
      }),
    /towerVoteSlots must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildSolanaSccpFullLightClientAuditProofRequests({
        ...input,
        finalityContextHash,
        finality_context_hash: finalityContextHash,
      }),
    /finalityContextHash must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildSolanaSccpFullLightClientAuditProofRequests({
        ...input,
        sourceVerifierMaterial: {},
        source_verifier_material: {},
      }),
    /sourceVerifierMaterial must not use multiple aliases/,
  );
  const snakeAccountsLtHashInput = { ...input, accounts_lt_hash: input.accountsLtHash };
  delete snakeAccountsLtHashInput.accountsLtHash;
  const snakeAccountsLtHashRequests =
    buildSolanaSccpFullLightClientAuditProofRequests(snakeAccountsLtHashInput);
  assert.equal(
    snakeAccountsLtHashRequests.towerReplay.openedAccountsLtHashContributionsHash,
    requests.towerReplay.openedAccountsLtHashContributionsHash,
  );
  assert.equal(
    snakeAccountsLtHashRequests.towerReplay.openedAccountsLtHashResidualChecksum,
    requests.towerReplay.openedAccountsLtHashResidualChecksum,
  );
  assert.equal(
    snakeAccountsLtHashRequests.towerReplay.auditStatementHash,
    requests.towerReplay.auditStatementHash,
  );
  const expectedVectors = {
    towerReplay: {
      statementHash: "0x2ead9384eaa2351b45a81bb22384a9bc9ed7c0793b06d0d3eb15424ef28929e3",
      statementLength: 777,
      publicInputColumns: [
        ["0x0100000000000000000000000000000000000000000000000000000000000000"],
        ["0x0300000000000000000000000000000000000000000000000000000000000000"],
        [SOLANA_MAINNET_GENESIS_PUBLIC_INPUT],
        ["0xe0c6130000000000000000000000000000000000000000000000000000000000"],
        ["0xb553931911947ab6caa4eba88d6aee62738b40f2e4d8d572e5e6616890abefbb"],
        ["0x2ead9384eaa2351b45a81bb22384a9bc9ed7c0793b06d0d3eb15424ef28929e3"],
        ["0xf0c76a74d7368857b724a8299f0851a30041acfbb03d6fc6bd4a6070358c093c"],
        ["0x9c33ee13a70d2c960e27e28680f7816b84bda7d6cb4888fb449f6407c87a2bbd"],
        ["0x3e0126e340dac71435abbb43b2df3bb5635568e8445326cd8723fef8a3dfd78f"],
        ["0xb1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1"],
        ["0x0300000000000000000000000000000000000000000000000000000000000000"],
        ["0xc1c6130000000000000000000000000000000000000000000000000000000000"],
        ["0xdfc6130000000000000000000000000000000000000000000000000000000000"],
        ["0x17a9f46bb57527c1579df8463067264c93125f1b5315fe3b537022809e76f3bc"],
        ["0xfc2832401bd6d624ab198e85a6ad1c889e09b393b3d16fff25a080c230c809dc"],
        ["0x922a426e06d6263986a0c9ff0f956f5429288c9c1310cb67fbaf30918de58b40"],
        ["0xaf75ee33d0fc85873b5302df026eaceddd40184c0f210a37968feea3b38d5ca0"],
        ["0xb114fd98978cd6d734a070976fb2e30a92110731bcc81ed2ace2698221aee727"],
        ["0x1313131313131313131313131313131313131313131313131313131313131313"],
        ["0x1414141414141414141414141414141414141414141414141414141414141414"],
        ["0x1515151515151515151515151515151515151515151515151515151515151515"],
        ["0x1616161616161616161616161616161616161616161616161616161616161616"],
        ["0x1717171717171717171717171717171717171717171717171717171717171717"],
        ["0x7777777777777777777777777777777777777777777777777777777777777777"],
      ],
    },
    fullAccountsdbLattice: {
      statementHash: "0x016d361178fe1ed787add1eb9b75b5cc37453995e24b0acd845bd977e1cc9df0",
      statementLength: 440,
      publicInputColumns: [
        ["0x0200000000000000000000000000000000000000000000000000000000000000"],
        ["0x0300000000000000000000000000000000000000000000000000000000000000"],
        [SOLANA_MAINNET_GENESIS_PUBLIC_INPUT],
        ["0xe0c6130000000000000000000000000000000000000000000000000000000000"],
        ["0xb553931911947ab6caa4eba88d6aee62738b40f2e4d8d572e5e6616890abefbb"],
        ["0x016d361178fe1ed787add1eb9b75b5cc37453995e24b0acd845bd977e1cc9df0"],
        ["0xf0c76a74d7368857b724a8299f0851a30041acfbb03d6fc6bd4a6070358c093c"],
        ["0x9c33ee13a70d2c960e27e28680f7816b84bda7d6cb4888fb449f6407c87a2bbd"],
        ["0x3e0126e340dac71435abbb43b2df3bb5635568e8445326cd8723fef8a3dfd78f"],
        ["0xc2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2"],
        ["0x0300000000000000000000000000000000000000000000000000000000000000"],
        ["0xc1c6130000000000000000000000000000000000000000000000000000000000"],
        ["0xdfc6130000000000000000000000000000000000000000000000000000000000"],
        ["0x17a9f46bb57527c1579df8463067264c93125f1b5315fe3b537022809e76f3bc"],
        ["0xfc2832401bd6d624ab198e85a6ad1c889e09b393b3d16fff25a080c230c809dc"],
        ["0x7777777777777777777777777777777777777777777777777777777777777777"],
        ["0xba606dacb76b0b03f395e6177a4a46cbe07f729678ab3a28f5ad8d7619cffc62"],
        ["0xc1b7c880344a2551d0842848f68b8519027e8b228a4c92c4e754141821d63810"],
        ["0x07270072f8b70b755ed491c1582b40050a484edd67752a8a0bbbd97aa175d4f9"],
        ["0x336bb79a5e96c331ddca555aedde346438de4ca1b227ae09f7faaa5e0e455be0"],
        ["0xfc2832401bd6d624ab198e85a6ad1c889e09b393b3d16fff25a080c230c809dc"],
      ],
    },
    bankForkChoice: {
      statementHash: "0x0c6a73bb4622acbb67c562c0a890237ca77619b33fececb645ee33b2028ed6a8",
      statementLength: 509,
      publicInputColumns: [
        ["0x0300000000000000000000000000000000000000000000000000000000000000"],
        ["0x0300000000000000000000000000000000000000000000000000000000000000"],
        [SOLANA_MAINNET_GENESIS_PUBLIC_INPUT],
        ["0xe0c6130000000000000000000000000000000000000000000000000000000000"],
        ["0xb553931911947ab6caa4eba88d6aee62738b40f2e4d8d572e5e6616890abefbb"],
        ["0x0c6a73bb4622acbb67c562c0a890237ca77619b33fececb645ee33b2028ed6a8"],
        ["0xf0c76a74d7368857b724a8299f0851a30041acfbb03d6fc6bd4a6070358c093c"],
        ["0x9c33ee13a70d2c960e27e28680f7816b84bda7d6cb4888fb449f6407c87a2bbd"],
        ["0x3e0126e340dac71435abbb43b2df3bb5635568e8445326cd8723fef8a3dfd78f"],
        ["0xd3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3"],
        ["0x0300000000000000000000000000000000000000000000000000000000000000"],
        ["0xc1c6130000000000000000000000000000000000000000000000000000000000"],
        ["0xdfc6130000000000000000000000000000000000000000000000000000000000"],
        ["0x17a9f46bb57527c1579df8463067264c93125f1b5315fe3b537022809e76f3bc"],
        ["0xfc2832401bd6d624ab198e85a6ad1c889e09b393b3d16fff25a080c230c809dc"],
        ["0xc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0"],
        ["0x46bf9f58208a9c61b931640824eb13d636d3af5b0268cce866c958367bd6a451"],
        ["0x4242424242424242424242424242424242424242424242424242424242424242"],
        ["0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"],
        ["0x7777777777777777777777777777777777777777777777777777777777777777"],
        ["0xba606dacb76b0b03f395e6177a4a46cbe07f729678ab3a28f5ad8d7619cffc62"],
        ["0x0800000000000000000000000000000000000000000000000000000000000000"],
        ["0x1d2a51ef7c068fe46c9f588c252ce9cea8b66d87453bf73c9920005802e738bc"],
        ["0xb114fd98978cd6d734a070976fb2e30a92110731bcc81ed2ace2698221aee727"],
        ["0xaf75ee33d0fc85873b5302df026eaceddd40184c0f210a37968feea3b38d5ca0"],
      ],
    },
  };

  assert.ok(
    canonicalSolanaSccpSourceStateVerificationProofBytes(input.accountsLtHashProof).length > 0,
  );
  assert.ok(canonicalSolanaSccpFinalityContextBytes(input).length > 0);
  const witness = normalizeSolanaSccpWitness(input);
  const bankForkHash = solanaSccpBankForkHash(input);
  const directFinalityContext = {
    version: 1,
    epoch: input.epoch,
    rooted_slot: input.rootedSlot,
    parent_slot: input.parentSlot,
    tower_vote_slots: input.towerVoteSlots,
    parent_bank_hash: input.parentBankHash,
    bank_signature_count: input.bankSignatureCount,
    bank_hash_hard_fork_data: witness.bankHashHardForkData,
    epoch_stake_root: input.epochStakeRoot,
    stake_activation_hash: input.stakeActivationHash,
    stake_account_state_hash: input.stakeAccountStateHash,
    stake_history_hash: input.stakeHistoryHash,
    stake_history_sysvar_account_hash: input.stakeHistorySysvarAccountHash,
    account_inclusion_root: witness.accountInclusionRoot,
    accounts_lt_hash_checksum: witness.accountsLtHashChecksum,
    accounts_lt_hash_proof_public_inputs_hash: witness.accountsLtHashProofPublicInputsHash,
    tower_lockout_hash: solanaSccpTowerLockoutHash(input),
    tower_replay_hash: solanaSccpTowerReplayHash({ ...input, bankForkHash }),
    bank_fork_hash: bankForkHash,
  };
  assert.deepEqual(
    canonicalSolanaSccpFinalityContextBytes(directFinalityContext),
    canonicalSolanaSccpFinalityContextBytes(input),
  );
  assert.throws(
    () =>
      canonicalSolanaSccpFinalityContextBytes({
        ...directFinalityContext,
        parentBankHash: directFinalityContext.parent_bank_hash,
      }),
    /parentBankHash must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalSolanaSccpFinalityContextBytes({
        ...directFinalityContext,
        towerVoteSlots: directFinalityContext.tower_vote_slots,
      }),
    /towerVoteSlots must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalSolanaSccpFinalityContextBytes({
        ...directFinalityContext,
        bankForkHash: directFinalityContext.bank_fork_hash,
      }),
    /bankForkHash must not use multiple aliases/,
  );
  assert.deepEqual(
    Object.keys(requests),
    ["towerReplay", "fullAccountsdbLattice", "bankForkChoice"],
  );
  assert.deepEqual(
    Object.values(requests).map((request) => request.role),
    ["tower_replay", "full_accountsdb_lattice", "bank_fork_choice"],
  );
  assert.equal(Object.isFrozen(requests), true);
  assert.equal(
    requests.towerReplay.circuitId,
    SCCP_SOLANA_TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1,
  );
  assert.equal(
    requests.fullAccountsdbLattice.circuitId,
    SCCP_SOLANA_FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1,
  );
  assert.equal(
    requests.bankForkChoice.circuitId,
    SCCP_SOLANA_BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1,
  );
  assert.equal(
    new Set(Object.values(requests).map((request) => request.circuitId)).size,
    3,
  );
  for (const [requestKey, request] of Object.entries(requests)) {
    assertImmutableFastpqProofRequest(request, [
      "statementBytes",
      "verificationContextBytes",
      "schemaDescriptor",
    ]);
    assert.equal(request.auditStatementHash, expectedVectors[requestKey].statementHash);
    assert.equal(request.statementBytes.length, expectedVectors[requestKey].statementLength);
    assert.deepEqual(
      request.publicInputColumns,
      expectedVectors[requestKey].publicInputColumns,
    );
    assert.equal(request.version, 1);
    assert.equal(request.proofFamily, "stark-fri-v1");
    assert.equal(request.parameterSet, "fastpq-lane-balanced");
    assert.equal(request.finalityContextHash, finalityContextHash);
    assert.equal(request.accountsLtHashProofHash, accountsLtHashProofHash);
    assert.equal(request.fullLightClientGateHash, sccpSolanaFullLightClientGateHash(input));
    assert.equal(request.fastpqTransitions.length, 3);
    assert.ok(request.fastpqTransitions.every((transition) => transition.key.startsWith("0x")));
    assert.deepEqual(
      request.schemaDescriptor,
      solanaSccpFullLightClientAuditOpenVerifySchemaDescriptor(input, request.role),
    );
    assert.deepEqual(
      request.publicInputColumns,
      solanaSccpFullLightClientAuditPublicInputColumns(input, request.role),
    );
    assert.equal(
      request.auditStatementHash,
      solanaSccpFullLightClientAuditStatementHash(input, request.role),
    );
    assert.deepEqual(
      request.statementBytes,
      canonicalSolanaSccpFullLightClientAuditStatementBytes(input, request.role),
    );
    if (requestKey === "fullAccountsdbLattice") {
      assert.deepEqual(
        request.statementBytes.slice(-32),
        Uint8Array.from(Buffer.from(accountsLtHashProofHash.slice(2), "hex")),
      );
      assert.notDeepEqual(
        request.statementBytes.slice(-32),
        Uint8Array.from(Buffer.from(witness.accountsLtHashProofPublicInputsHash.slice(2), "hex")),
      );
    }
    const proofCapsule = wrapSolanaSccpSourceStateVerificationProof(
      new Uint8Array([9, 8, 7]),
      request,
    );
    assert.equal(proofCapsule.circuitId, request.circuitId);
    assert.equal(proofCapsule.proofFamily, request.proofFamily);
    assert.deepEqual(proofCapsule.proofBytes, new Uint8Array([9, 8, 7]));
    assert.equal(proofCapsule.proofBase64, "CQgH");
    assert.ok(canonicalSolanaSccpSourceStateVerificationProofBytes(proofCapsule).length > 0);
    assert.throws(
      () => solanaSccpAccountsLtHashProofHash(proofCapsule),
      /Solana AccountsLtHash/,
    );
  }
  const wrongAuditGenesisRequest = mutableFastpqProofRequest(requests.bankForkChoice);
  wrongAuditGenesisRequest.publicInputColumns[2][0] = HEX32_A;
  assert.throws(
    () => wrapSolanaSccpSourceStateVerificationProof(new Uint8Array([9, 8, 7]), wrongAuditGenesisRequest),
    /mainnet_genesis_hash/,
  );
  const wrongAuditStatementColumnRequest = mutableFastpqProofRequest(requests.towerReplay);
  wrongAuditStatementColumnRequest.publicInputColumns[5][0] = HEX32_C;
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(
        new Uint8Array([9, 8, 7]),
        wrongAuditStatementColumnRequest,
      ),
    /audit_statement_hash/,
  );
  const staleAuditHashRequest = mutableFastpqProofRequest(requests.towerReplay);
  staleAuditHashRequest.auditStatementHash = HEX32_C;
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(
        new Uint8Array([9, 8, 7]),
        staleAuditHashRequest,
      ),
    /auditStatementHash must match request\.statementBytes/,
  );
  const wrongAuditDsidRequest = mutableFastpqProofRequest(requests.towerReplay);
  wrongAuditDsidRequest.fastpqPublicInputs.dsid = "0x00000000000000000000000000000000";
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(
        new Uint8Array([9, 8, 7]),
        wrongAuditDsidRequest,
      ),
    /fastpqPublicInputs\.dsid/,
  );
  const wrongAuditTxSetRequest = mutableFastpqProofRequest(requests.towerReplay);
  wrongAuditTxSetRequest.fastpqPublicInputs.txSetHash = HEX32_C;
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(
        new Uint8Array([9, 8, 7]),
        wrongAuditTxSetRequest,
      ),
    /fastpqPublicInputs\.txSetHash/,
  );
  const duplicateAuditRoleAliasRequest = mutableFastpqProofRequest(requests.towerReplay);
  duplicateAuditRoleAliasRequest.audit_role = duplicateAuditRoleAliasRequest.role;
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(
        new Uint8Array([9, 8, 7]),
        duplicateAuditRoleAliasRequest,
      ),
    /request\.role.*multiple aliases/,
  );
  const duplicateAuditFastpqAliasRequest = mutableFastpqProofRequest(requests.towerReplay);
  duplicateAuditFastpqAliasRequest.fastpqPublicInputs.old_root =
    duplicateAuditFastpqAliasRequest.fastpqPublicInputs.oldRoot;
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(
        new Uint8Array([9, 8, 7]),
        duplicateAuditFastpqAliasRequest,
      ),
    /request\.fastpqPublicInputs\.oldRoot.*multiple aliases/,
  );
  const wrongAuditTransitionRequest = mutableFastpqProofRequest(requests.towerReplay);
  wrongAuditTransitionRequest.fastpqTransitions[0].newValue = "0x00";
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(
        new Uint8Array([9, 8, 7]),
        wrongAuditTransitionRequest,
      ),
    /canonical Solana source-state request/,
  );
  const wrongAuditOldValueTransitionRequest = mutableFastpqProofRequest(requests.towerReplay);
  wrongAuditOldValueTransitionRequest.fastpqTransitions[0].oldValue = "0x00";
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(
        new Uint8Array([9, 8, 7]),
        wrongAuditOldValueTransitionRequest,
      ),
    /canonical Solana source-state request/,
  );
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(new Uint8Array([9, 8, 7]), {
        ...requests.towerReplay,
        role: "bank_fork_choice",
      }),
    /request\.roleCode must match request\.role/,
  );
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(new Uint8Array([9, 8, 7]), {
        ...requests.towerReplay,
        verifierHash: SCCP_ZERO_HASH_V1,
      }),
    /request\.verifierHash must not be zero/,
  );
  const reusedSourceStateVerifierRequest = mutableFastpqProofRequest(requests.towerReplay);
  reusedSourceStateVerifierRequest.verifierHash =
    reusedSourceStateVerifierRequest.sourceStateVerifierHash;
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(
        new Uint8Array([9, 8, 7]),
        reusedSourceStateVerifierRequest,
      ),
    /role-separated/,
  );
  const reusedAuditStatementRequest = mutableFastpqProofRequest(requests.towerReplay);
  reusedAuditStatementRequest.verifierHash = reusedAuditStatementRequest.auditStatementHash;
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(
        new Uint8Array([9, 8, 7]),
        reusedAuditStatementRequest,
      ),
    /role-separated/,
  );
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(new Uint8Array([9, 8, 7]), {
        ...requests.towerReplay,
        sourceStateVerifierHash: SCCP_SOLANA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1,
      }),
    /Solana template verifier hash/,
  );
  assert.throws(
    () =>
      wrapSolanaSccpSourceStateVerificationProof(new Uint8Array([9, 8, 7]), {
        ...requests.towerReplay,
        parameterSet: "debug",
      }),
    /request\.parameterSet/,
  );
  assert.equal(
    requests.towerReplay.voteMessageHash,
    solanaSccpVoteMessageHash({
      sourceDomain: SCCP_DOMAIN_SOL,
      finalizedSlot: input.finalizedSlot,
      blockhash: input.blockhash,
      bankHash: input.bankHash,
      transactionStatusRoot: input.transactionStatusRoot,
      messageProofHash: input.messageProofHash,
      finalityContextHash,
    }),
  );
  assert.equal(
    requests.fullAccountsdbLattice.publicInputColumns.at(-1)[0],
    accountsLtHashProofHash,
  );
  assert.deepEqual(
    requests.bankForkChoice.publicInputColumns[19],
    [input.accountInclusionRoot],
  );
  assert.ok(
    Buffer.from(requests.towerReplay.schemaDescriptor).includes(
      Buffer.from("mainnet_genesis_hash"),
    ),
  );
  assert.ok(
    Buffer.from(requests.towerReplay.schemaDescriptor).includes(
      Buffer.from("full_light_client_gate_hash"),
    ),
  );
  assert.deepEqual(
    requests.towerReplay.publicInputColumns[20],
    [input.stakeAccountStateHash],
  );
  assert.deepEqual(
    requests.towerReplay.publicInputColumns[22],
    [input.stakeHistorySysvarAccountHash],
  );
  assert.deepEqual(
    requests.towerReplay.publicInputColumns[23],
    [input.accountInclusionRoot],
  );
  assert.ok(
    Buffer.from(requests.towerReplay.schemaDescriptor).includes(
      Buffer.from("stake_account_state_hash"),
    ),
  );
  assert.ok(
    Buffer.from(requests.towerReplay.schemaDescriptor).includes(
      Buffer.from("stake_history_sysvar_account_hash"),
    ),
  );
  assert.ok(
    Buffer.from(requests.towerReplay.schemaDescriptor).includes(
      Buffer.from("account_inclusion_root"),
    ),
  );
  assert.ok(
    Buffer.from(requests.bankForkChoice.schemaDescriptor).includes(
      Buffer.from("account_inclusion_root"),
    ),
  );
  assert.ok(
    Buffer.from(requests.bankForkChoice.schemaDescriptor).includes(
      Buffer.from("bank_hash_hard_fork_data_hash"),
    ),
  );
  assert.notEqual(
    requests.towerReplay.auditStatementHash,
    requests.bankForkChoice.auditStatementHash,
  );
  assert.throws(
    () => buildSolanaSccpTowerReplayProofRequest({ ...input, accountsLtHashProof: undefined }),
    /accountsLtHashProofHash/,
  );
  const proofHashOnlyInput = { ...input, accountsLtHashProof: undefined };
  proofHashOnlyInput.accountsLtHashProofHash = accountsLtHashProofHash;
  assert.throws(
    () => buildSolanaSccpTowerReplayProofRequest(proofHashOnlyInput),
    /accountsLtHashProof is required/,
  );
  assert.throws(
    () => solanaSccpAccountsLtHashProofHash({
      ...input.accountsLtHashProof,
      proofBytes: new Uint8Array([0, 0, 0]),
    }),
    /proofBytes must not be all zero/,
  );
  assert.throws(
    () => solanaSccpAccountsLtHashProofHash({
      ...input.accountsLtHashProof,
      proof_base64: "AAAA",
    }),
    /accountsLtHashProof\.proofBase64 must match accountsLtHashProof\.proofBytes/,
  );
  assert.throws(
    () => solanaSccpAccountsLtHashProofHash({
      ...input.accountsLtHashProof,
      version: 0,
    }),
    /accountsLtHashProof\.version must be 1/,
  );
  assert.throws(
    () => canonicalSolanaSccpSourceStateVerificationProofBytes({
      ...input.accountsLtHashProof,
      version: null,
    }),
    /sourceStateProof\.version must be 1/,
  );
  assert.throws(
    () => canonicalSolanaSccpSourceStateVerificationProofBytes({
      ...input.accountsLtHashProof,
      proofFamily: null,
    }),
    /sourceStateProof\.proofFamily/,
  );
  assert.throws(
    () => canonicalSolanaSccpSourceStateVerificationProofBytes({
      ...input.accountsLtHashProof,
      circuitId: SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1,
    }),
    /Solana source-state/,
  );
  assert.throws(
    () => buildSolanaSccpTowerReplayProofRequest({
      ...input,
      accountsLtHashProofHash: HEX32_A,
    }),
    /accountsLtHashProofHash must match/,
  );
  assert.throws(
    () => buildSolanaSccpTowerReplayProofRequest({
      ...input,
      sourceVerifierMaterialHash: HEX32_A,
    }),
    /sourceVerifierMaterialHash must match sourceVerifierMaterial/,
  );
  assert.throws(
    () => buildSolanaSccpTowerReplayProofRequest({
      ...input,
      sourceAdapterDeploymentHash: HEX32_B,
    }),
    /sourceAdapterDeploymentHash must match sourceAdapterDeployment/,
  );
  const missingWitnessDeploymentHash = {
    ...input,
    sourceAdapterDeployment: { ...input },
  };
  delete missingWitnessDeploymentHash.sourceAdapterDeploymentHash;
  delete missingWitnessDeploymentHash.sourceAdapterDeploymentReceiptHash;
  assert.throws(
    () => buildSolanaSccpTowerReplayProofRequest(missingWitnessDeploymentHash),
    /sourceAdapterDeploymentHash must match witness/,
  );
  assert.throws(
    () => buildSolanaSccpTowerReplayProofRequest({
      ...input,
      fullLightClientGateHash: HEX32_B,
    }),
    /fullLightClientGateHash must match sourceAdapterDeployment/,
  );
  assert.throws(
    () => buildSolanaSccpTowerReplayProofRequest({
      ...input,
      sourceAdapterDeployment: { ...input },
      sourceAdapterDeploymentReceiptHash: HEX32_B,
    }),
    /sourceAdapterDeploymentReceiptHash must match witness/,
  );
  assert.throws(
    () => buildSolanaSccpTowerReplayProofRequest({
      ...input,
      solanaTowerReplayVerifierHash: input.solanaFullAccountsdbLatticeVerifierHash,
    }),
    /role-separated/,
  );
  assert.throws(
    () => buildSolanaSccpTowerReplayProofRequest({
      ...input,
      solanaTowerReplayVerifierHash: input.sourceStateVerifierHash,
    }),
    /must not reuse/,
  );
  assert.throws(
    () => buildSolanaSccpTowerReplayProofRequest({
      ...input,
      solanaTowerReplayVerifierHash:
        SOLANA_TEMPLATE_SOURCE_MATERIAL_COMPONENT_HASHES.get("sourceTrustAnchorHash"),
    }),
    /template material/,
  );
});

test("Solana source-state prover wraps linked AccountsLtHash proof bytes", async () => {
  const seen = [];
  const prover = new SolanaSccpSourceStateProver({
    async prove(request, options) {
      seen.push({ request, options });
      assert.equal(request.circuitId, SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1);
      return {
        proof_bytes: new Uint8Array([1, 2, 3]),
        version: 1,
        proofFamily: SCCP_STARK_FRI_PROOF_FAMILY_V1,
        circuitId: request.circuitId,
        proofBase64: "AQID",
        parameterSet: request.parameterSet,
        sourceDomain: String(request.sourceDomain),
        finalizedSlot: BigInt(request.finalizedSlot),
        sourceStateVerifierId: request.sourceStateVerifierId,
        sourceStateVerifierHash: request.sourceStateVerifierHash.toUpperCase(),
        accountsLtHashProofPublicInputsHash:
          request.accountsLtHashProofPublicInputsHash.toUpperCase(),
        openedAccountsLtHashContributionsHash:
          request.openedAccountsLtHashContributionsHash.toUpperCase(),
        openedAccountsLtHashResidualChecksum:
          request.openedAccountsLtHashResidualChecksum.toUpperCase(),
        publicInputColumns: request.publicInputColumns,
        fastpqPublicInputs: request.fastpqPublicInputs,
        fastpqTransitions: request.fastpqTransitions,
        statementBytes: request.statementBytes,
        accountCommitmentBytes: request.accountCommitmentBytes,
        verificationContextBytes: request.verificationContextBytes,
        schemaDescriptor: request.schemaDescriptor,
      };
    },
  });

  const proof = await prover.proveAccountsLtHash(sampleSolanaAccountsLtHashProofInput(), {
    source: "ui",
  });

  assert.equal(seen.length, 1);
  assert.equal(seen[0].options.source, "ui");
  assert.equal(proof.circuitId, SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1);
  assert.deepEqual(proof.proofBytes, new Uint8Array([1, 2, 3]));
  assert.equal(proof.proofBase64, "AQID");
  const fastpqAliasProof = await new SolanaSccpSourceStateProver({
    prove(request) {
      const upperHex = (value) => value === "0x" ? value : value.toUpperCase();
      return {
        proofBytes: [7, 8, 9],
        fastpqPublicInputs: {
          dsid: request.fastpqPublicInputs.dsid.toUpperCase(),
          slot: BigInt(request.fastpqPublicInputs.slot),
          old_root: request.fastpqPublicInputs.oldRoot.toUpperCase(),
          new_root: request.fastpqPublicInputs.newRoot.toUpperCase(),
          perm_root: request.fastpqPublicInputs.permRoot.toUpperCase(),
          tx_set_hash: request.fastpqPublicInputs.txSetHash.toUpperCase(),
        },
        fastpqTransitions: request.fastpqTransitions.map((transition) => ({
          key: transition.key,
          operation: transition.operation,
          old_value: upperHex(transition.oldValue),
          new_value: upperHex(transition.newValue),
        })),
      };
    },
  }).proveAccountsLtHash(sampleSolanaAccountsLtHashProofInput());
  assert.deepEqual(fastpqAliasProof.proofBytes, new Uint8Array([7, 8, 9]));
  const oversizedProofBytes = new Uint8Array(SCCP_SOURCE_STATE_MAX_PROOF_BYTES + 1).fill(1);
  await assert.rejects(
    () =>
      new SolanaSccpSourceStateProver({
        prove() {
          return {
            proofBytes: oversizedProofBytes,
            proofBase64: "AQID",
          };
        },
      }).proveAccountsLtHash(sampleSolanaAccountsLtHashProofInput()),
    /at most/,
  );
  await assert.rejects(
    () =>
      new SolanaSccpSourceStateProver({
        prove() {
          return {
            proofBytes: [1, 2, 3],
            proof_bytes: [1, 2, 3],
          };
        },
      }).proveAccountsLtHash(sampleSolanaAccountsLtHashProofInput()),
    /source-state prover result\.proofBytes/u,
  );
  await assert.rejects(
    () =>
      new SolanaSccpSourceStateProver({
        prove() {
          return {
            proofBytes: [1, 2, 3],
            proofBase64: "AQID",
            proof_base64: "AQID",
          };
        },
      }).proveAccountsLtHash(sampleSolanaAccountsLtHashProofInput()),
    /source-state prover result\.proofBase64/u,
  );
  await assert.rejects(
    () =>
      new SolanaSccpSourceStateProver({
        prove() {
          return {
            proofBytes: [1, 2, 3],
            proofVersion: 0,
          };
        },
      }).proveAccountsLtHash(sampleSolanaAccountsLtHashProofInput()),
    /source-state prover result\.version/u,
  );
  await assert.rejects(
    () =>
      new SolanaSccpSourceStateProver({
        prove() {
          return {
            proofBytes: [1, 2, 3],
            version: 1,
            proof_version: 1,
          };
        },
      }).proveAccountsLtHash(sampleSolanaAccountsLtHashProofInput()),
    /source-state prover result\.version/u,
  );
  await assert.rejects(
    () =>
      new SolanaSccpSourceStateProver({
        prove() {
          return {
            proofBytes: [1, 2, 3],
            proofFamily: ` ${SCCP_STARK_FRI_PROOF_FAMILY_V1} `,
          };
        },
      }).proveAccountsLtHash(sampleSolanaAccountsLtHashProofInput()),
    /source-state prover result\.proofFamily/u,
  );
  await assert.rejects(
    () =>
      new SolanaSccpSourceStateProver({
        prove(request) {
          return {
            proofBytes: [1, 2, 3],
            circuitId: ` ${request.circuitId} `,
          };
        },
      }).proveAccountsLtHash(sampleSolanaAccountsLtHashProofInput()),
    /source-state prover result\.circuitId/u,
  );
  await assert.rejects(
    () =>
      new SolanaSccpSourceStateProver({
        prove(request) {
          return {
            proofBytes: [1, 2, 3],
            parameterSet: ` ${request.parameterSet} `,
          };
        },
      }).proveAccountsLtHash(sampleSolanaAccountsLtHashProofInput()),
    /source-state prover result\.parameterSet/u,
  );
  await assert.rejects(
    () => new SolanaSccpSourceStateProver().proveRequest(seen[0].request),
    (error) =>
      error.code === "ERR_SCCP_SOLANA_SOURCE_STATE_PROVER_UNAVAILABLE" &&
      /source-state prover is not linked/.test(error.message),
  );
  await assert.rejects(
    () =>
      new SolanaSccpSourceStateProver({
        prove(request) {
          return {
            proofBytes: [1, 2, 3],
            sourceStateVerifierHash: HEX32_C,
            publicInputColumns: request.publicInputColumns,
          };
        },
      }).proveAccountsLtHash(sampleSolanaAccountsLtHashProofInput()),
    /source-state prover result\.sourceStateVerifierHash must match request\.sourceStateVerifierHash/u,
  );
  await assert.rejects(
    () =>
      new SolanaSccpSourceStateProver({
        prove(request) {
          const columns = request.publicInputColumns.map((row) => [...row]);
          columns[0][0] = ` ${columns[0][0]} `;
          return {
            proofBytes: [1, 2, 3],
            publicInputColumns: columns,
          };
        },
      }).proveAccountsLtHash(sampleSolanaAccountsLtHashProofInput()),
    /source-state prover result\.publicInputColumns must match request\.publicInputColumns/u,
  );
  await assert.rejects(
    () =>
      new SolanaSccpSourceStateProver({
        prove(request) {
          return {
            proofBytes: [1, 2, 3],
            publicInputColumns: [[HEX32_C], ...request.publicInputColumns.slice(1)],
          };
        },
      }).proveAccountsLtHash(sampleSolanaAccountsLtHashProofInput()),
    /source-state prover result\.publicInputColumns must match request\.publicInputColumns/u,
  );
  await assert.rejects(
    () =>
      new SolanaSccpSourceStateProver({
        prove(request) {
          return {
            proofBytes: [1, 2, 3],
            statementBytes: Uint8Array.from([request.statementBytes[0] ^ 0xff]),
          };
        },
      }).proveAccountsLtHash(sampleSolanaAccountsLtHashProofInput()),
    /source-state prover result\.statementBytes must match request\.statementBytes/u,
  );
});

test("Solana source-state prover snapshots mutable callback requests", async () => {
  const builtRequest = buildSolanaSccpAccountsLtHashProofRequest(
    sampleSolanaAccountsLtHashProofInput(),
  );
  const mutableRequest = mutableFastpqProofRequest(builtRequest);
  const expectedStatementByte = mutableRequest.statementBytes[0];
  const prover = new SolanaSccpSourceStateProver({
    prove(request) {
      assert.notStrictEqual(request, mutableRequest);
      assert.equal(Object.isFrozen(request), true);
      assert.equal(Object.isFrozen(request.fastpqTransitions), true);
      assert.equal(Object.isFrozen(request.fastpqTransitions[0]), true);
      assert.throws(() => {
        request.circuitId = SCCP_SOLANA_BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1;
      }, TypeError);
      assert.throws(() => {
        request.fastpqTransitions[0].newValue = "0x00";
      }, TypeError);
      const exposedStatement = request.statementBytes;
      exposedStatement[0] ^= 0xff;
      assert.equal(request.statementBytes[0], expectedStatementByte);
      mutableRequest.circuitId = SCCP_SOLANA_BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1;
      return [4, 5, 6];
    },
  });

  const proof = await prover.proveRequest(mutableRequest);

  assert.equal(proof.circuitId, builtRequest.circuitId);
  assert.equal(proof.proofBase64, "BAUG");
});

test("Solana source-state prover wraps all full-light audit role proofs", async () => {
  const roles = [];
  const camelRole = {
    tower_replay: "towerReplay",
    full_accountsdb_lattice: "fullAccountsdbLattice",
    bank_fork_choice: "bankForkChoice",
  };
  const prover = new SolanaSccpSourceStateProver({
    prove(request) {
      roles.push(request.role);
      return {
        proofBytes: [9, 8, 7],
        version: 1,
        proofFamily: SCCP_STARK_FRI_PROOF_FAMILY_V1,
        circuitId: request.circuitId,
        parameterSet: request.parameterSet,
        role: camelRole[request.role],
        roleCode: BigInt(request.roleCode),
        sourceDomain: String(request.sourceDomain),
        finalizedSlot: BigInt(request.finalizedSlot),
        verifierId: request.verifierId,
        verifierHash: request.verifierHash.toUpperCase(),
        sourceStateVerifierId: request.sourceStateVerifierId,
        sourceStateVerifierHash: request.sourceStateVerifierHash.toUpperCase(),
        sourceVerifierMaterialHash: request.sourceVerifierMaterialHash.toUpperCase(),
        sourceAdapterDeploymentHash: request.sourceAdapterDeploymentHash.toUpperCase(),
        fullLightClientGateHash: request.fullLightClientGateHash.toUpperCase(),
        finalityContextHash: request.finalityContextHash.toUpperCase(),
        voteMessageHash: request.voteMessageHash.toUpperCase(),
        accountsLtHashProofHash: request.accountsLtHashProofHash.toUpperCase(),
        auditStatementHash: request.auditStatementHash.toUpperCase(),
        publicInputColumns: request.publicInputColumns,
        fastpqPublicInputs: request.fastpqPublicInputs,
        fastpqTransitions: request.fastpqTransitions,
        statementBytes: request.statementBytes,
        verificationContextBytes: request.verificationContextBytes,
        schemaDescriptor: request.schemaDescriptor,
      };
    },
  });

  const proofs = await prover.proveFullLightClientAudit(
    sampleSolanaFullLightClientAuditProofInput(),
  );

  assert.equal(Object.isFrozen(proofs), true);
  assert.deepEqual(roles, ["tower_replay", "full_accountsdb_lattice", "bank_fork_choice"]);
  assert.equal(proofs.towerReplay.circuitId, SCCP_SOLANA_TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1);
  assert.equal(
    proofs.fullAccountsdbLattice.circuitId,
    SCCP_SOLANA_FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1,
  );
  assert.equal(proofs.bankForkChoice.circuitId, SCCP_SOLANA_BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1);
  assert.equal(proofs.bankForkChoice.proofBase64, "CQgH");
  await assert.rejects(
    () =>
      new SolanaSccpSourceStateProver({
        prove() {
          return new Uint8Array([0, 0]);
        },
      }).proveFullLightClientAudit(sampleSolanaFullLightClientAuditProofInput()),
    /all zero/,
  );
  await assert.rejects(
    () =>
      new SolanaSccpSourceStateProver({
        prove(request) {
          return {
            proofBytes: [1, 2, 3],
            circuitId: request.role === "tower_replay"
              ? SCCP_SOLANA_BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1
              : request.circuitId,
          };
        },
      }).proveFullLightClientAudit(sampleSolanaFullLightClientAuditProofInput()),
    /result\.circuitId/u,
  );
  await assert.rejects(
    () =>
      new SolanaSccpSourceStateProver({
        prove(request) {
          return {
            proofBytes: [1, 2, 3],
            circuitId: request.circuitId,
            proofFamily: SCCP_STARK_FRI_PROOF_FAMILY_V1,
            proofBase64: "AAAA",
            version: 1,
          };
        },
      }).proveFullLightClientAudit(sampleSolanaFullLightClientAuditProofInput()),
    /result\.proofBase64/u,
  );
  await assert.rejects(
    () =>
      new SolanaSccpSourceStateProver({
        prove(request) {
          return {
            proofBytes: [1, 2, 3],
            circuitId: request.circuitId,
            proofFamily: SCCP_STARK_FRI_PROOF_FAMILY_V1,
            proofBase64: " AQID ",
            version: 1,
          };
        },
      }).proveFullLightClientAudit(sampleSolanaFullLightClientAuditProofInput()),
    /result\.proofBase64/u,
  );
  await assert.rejects(
    () =>
      new SolanaSccpSourceStateProver({
        prove(request) {
          return {
            proofBytes: [1, 2, 3],
            role: ` ${request.role} `,
          };
        },
      }).proveFullLightClientAudit(sampleSolanaFullLightClientAuditProofInput()),
    /source-state prover result\.role must match request\.role/u,
  );
  await assert.rejects(
    () =>
      new SolanaSccpSourceStateProver({
        prove(request) {
          return {
            proofBytes: [1, 2, 3],
            role: request.role === "tower_replay" ? "bank_fork_choice" : request.role,
          };
        },
      }).proveFullLightClientAudit(sampleSolanaFullLightClientAuditProofInput()),
    /source-state prover result\.role must match request\.role/u,
  );
  await assert.rejects(
    () =>
      new SolanaSccpSourceStateProver({
        prove(request) {
          return {
            proofBytes: [1, 2, 3],
            verifierHash: request.role === "tower_replay" ? HEX32_A : request.verifierHash,
          };
        },
      }).proveFullLightClientAudit(sampleSolanaFullLightClientAuditProofInput()),
    /source-state prover result\.verifierHash must match request\.verifierHash/u,
  );
});

test("derives Solana vote and stake account data hashes from semantic fields", () => {
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
  assert.match(voteHash, /^0x[0-9a-f]{64}$/);
  assert.notEqual(voteHash, solanaSccpVoteAccountDataHash({ ...voteInput, authorizedVoter: `0x${"62".repeat(32)}` }));
  assert.notEqual(voteHash, solanaSccpVoteAccountDataHash({ ...voteInput, inflationRewardsCommissionBps: 701n }));
  assert.throws(
    () => solanaSccpVoteAccountDataHash({ ...voteInput, node_pubkey: voteInput.nodePubkey }),
    /nodePubkey must not use multiple aliases/,
  );
  assert.throws(
    () =>
      solanaSccpVoteAccountDataHash({
        ...voteInput,
        inflation_rewards_commission_bps: voteInput.inflationRewardsCommissionBps,
      }),
    /inflationRewardsCommissionBps must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpVoteAccountDataHash({ ...voteInput, tower_vote_slots: towerVoteSlots }),
    /towerVoteSlots must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpVoteAccountDataHash({ ...voteInput, towerVoteSlots: [10n, ...towerVoteSlots.slice(1)] }),
    /towerVoteSlots\[0\]/,
  );

  const stakeInput = {
    staker: `0x${"81".repeat(32)}`,
    withdrawer: `0x${"91".repeat(32)}`,
    voterPubkey: `0x${"a1".repeat(32)}`,
    delegatedStake: 1_000n,
    activationEpoch: 2n,
    deactivationEpoch: 9n,
    warmupCooldownRateBytes: [0x0a, 0xd7, 0xa3, 0x70, 0x3d, 0x0a, 0xb7, 0x3f],
    creditsObserved: 123n,
    stakeFlags: 1n,
  };
  assert.equal(canonicalSolanaSccpStakeAccountDataBytes(stakeInput).length, 154);
  const stakeHash = solanaSccpStakeAccountDataHash(stakeInput);
  assert.match(stakeHash, /^0x[0-9a-f]{64}$/);
  assert.notEqual(stakeHash, solanaSccpStakeAccountDataHash({ ...stakeInput, voterPubkey: `0x${"a2".repeat(32)}` }));
  assert.match(
    solanaSccpStakeAccountDataHash({
      ...stakeInput,
      warmupCooldownRateBytes: [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xd0, 0x3f],
    }),
    /^0x[0-9a-f]{64}$/,
  );
  assert.throws(
    () => solanaSccpStakeAccountDataHash({ ...stakeInput, warmupCooldownRateBytes: new Uint8Array(8) }),
    /warmupCooldownRateBytes/,
  );
  assert.notEqual(stakeHash, solanaSccpStakeAccountDataHash({ ...stakeInput, stakeFlags: 0n }));
  assert.throws(
    () => solanaSccpStakeAccountDataHash({ ...stakeInput, voter_pubkey: stakeInput.voterPubkey }),
    /voterPubkey must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpStakeAccountDataHash({ ...stakeInput, delegated_stake: stakeInput.delegatedStake }),
    /delegatedStake must not use multiple aliases/,
  );
  assert.throws(
    () =>
      solanaSccpStakeAccountDataHash({
        ...stakeInput,
        warmup_cooldown_rate_bytes: stakeInput.warmupCooldownRateBytes,
      }),
    /warmupCooldownRateBytes must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpStakeAccountDataHash({ ...stakeInput, deactivationEpoch: 2n }),
    /deactivationEpoch/,
  );
  assert.throws(
    () => solanaSccpStakeAccountDataHash({ ...stakeInput, stakeFlags: 2n }),
    /stakeFlags/,
  );
  assert.throws(
    () => solanaSccpStakeAccountDataHash({ ...stakeInput, warmupCooldownRateBytes: [1, 2, 3] }),
    /warmupCooldownRateBytes/,
  );
});

test("derives Solana vote account data hash from raw VoteState account bytes", () => {
  const voteAccountAddress = `0x${"81".repeat(32)}`;
  const rawV3 = sampleSolanaVoteStateAccount(true);
  const parsed = solanaSccpVoteAccountDataFromRawVoteState(rawV3, 3n, voteAccountAddress);
  assert.deepEqual(Array.from(parsed.nodePubkey), Array(32).fill(0x51));
  assert.deepEqual(Array.from(parsed.authorizedVoter), Array(32).fill(0x61));
  assert.deepEqual(Array.from(parsed.authorizedWithdrawer), Array(32).fill(0x71));
  assert.deepEqual(Array.from(parsed.inflationRewardsCollector), Array(32).fill(0x81));
  assert.deepEqual(Array.from(parsed.blockRevenueCollector), Array(32).fill(0x51));
  assert.equal(parsed.inflationRewardsCommissionBps, 700n);
  assert.equal(parsed.blockRevenueCommissionBps, 10_000n);
  assert.equal(parsed.pendingDelegatorRewards, 0n);
  assert.equal(parsed.blsPubkeyCompressed.length, 0);
  assert.equal(parsed.rootSlot, 10n);
  assert.deepEqual(parsed.towerVoteSlots, Array.from({ length: 31 }, (_, index) => 11n + BigInt(index)));
  assert.equal(
    solanaSccpVoteAccountDataHashFromRawVoteState(rawV3, 3n, voteAccountAddress),
    solanaSccpVoteAccountDataHash(parsed),
  );
  assert.equal(
    solanaSccpVoteAccountDataHashFromRawVoteStateV1OrV3(rawV3, 3n, voteAccountAddress),
    solanaSccpVoteAccountDataHash(parsed),
  );

  const rawV1 = sampleSolanaVoteStateAccount(false);
  assert.deepEqual(
    solanaSccpVoteAccountDataFromRawVoteState(rawV1, 3n, voteAccountAddress).towerVoteSlots,
    parsed.towerVoteSlots,
  );

  const rawV4 = sampleSolanaVoteStateV4Account(true);
  const parsedV4 = solanaSccpVoteAccountDataFromRawVoteState(rawV4, 3n, voteAccountAddress);
  assert.deepEqual(Array.from(parsedV4.inflationRewardsCollector), Array(32).fill(0x81));
  assert.deepEqual(Array.from(parsedV4.blockRevenueCollector), Array(32).fill(0x91));
  assert.equal(parsedV4.inflationRewardsCommissionBps, 1_234n);
  assert.equal(parsedV4.blockRevenueCommissionBps, 9_876n);
  assert.equal(parsedV4.pendingDelegatorRewards, 456n);
  assert.deepEqual(Array.from(parsedV4.blsPubkeyCompressed), Array(48).fill(0xa5));
  const v4InflationCommissionBpsOffset = 4 + 4 * 32;
  const excessiveInflationCommissionV4 = rawV4.slice();
  new DataView(
    excessiveInflationCommissionV4.buffer,
    excessiveInflationCommissionV4.byteOffset,
    excessiveInflationCommissionV4.byteLength,
  ).setUint16(v4InflationCommissionBpsOffset, 10_001, true);
  assert.throws(
    () =>
      solanaSccpVoteAccountDataFromRawVoteState(
        excessiveInflationCommissionV4,
        3n,
        voteAccountAddress,
      ),
    /inflationRewardsCommissionBps must be at most 10000/u,
  );
  const excessiveBlockCommissionV4 = rawV4.slice();
  new DataView(
    excessiveBlockCommissionV4.buffer,
    excessiveBlockCommissionV4.byteOffset,
    excessiveBlockCommissionV4.byteLength,
  ).setUint16(v4InflationCommissionBpsOffset + 2, 10_001, true);
  assert.throws(
    () =>
      solanaSccpVoteAccountDataFromRawVoteState(
        excessiveBlockCommissionV4,
        3n,
        voteAccountAddress,
      ),
    /blockRevenueCommissionBps must be at most 10000/u,
  );
  assert.throws(
    () =>
      solanaSccpVoteAccountDataHash({
        ...parsedV4,
        blsPubkeyCompressed: new Uint8Array(48),
      }),
    /blsPubkeyCompressed/u,
  );
  const allZeroBlsV4 = rawV4.slice();
  const v4BlsPubkeyOffset = 4 + 4 * 32 + 2 + 2 + 8 + 1;
  allZeroBlsV4.fill(0, v4BlsPubkeyOffset, v4BlsPubkeyOffset + 48);
  assert.throws(
    () => solanaSccpVoteAccountDataFromRawVoteState(allZeroBlsV4, 3n, voteAccountAddress),
    /blsPubkeyCompressed/u,
  );
  const parsedV4FourAuthorized = solanaSccpVoteAccountDataFromRawVoteState(
    sampleSolanaVoteStateV4Account(true, 4),
    3n,
    voteAccountAddress,
  );
  assert.deepEqual(Array.from(parsedV4FourAuthorized.authorizedVoter), Array(32).fill(0x62));

  const wrongVoteCount = rawV3.slice();
  new DataView(wrongVoteCount.buffer).setBigUint64(4 + 32 + 32 + 1, 30n, true);
  assert.throws(
    () => solanaSccpVoteAccountDataFromRawVoteState(wrongVoteCount, 3n, voteAccountAddress),
    /31 active post-root slots/u,
  );

  const voteEntryOffset = 4 + 32 + 32 + 1 + 8;
  const firstVoteSlotOffset = voteEntryOffset + 1;
  const firstConfirmationOffset = firstVoteSlotOffset + 8;
  const secondVoteSlotOffset = voteEntryOffset + (1 + 8 + 4) + 1;
  const rootOptionOffset = voteEntryOffset + (31 * (1 + 8 + 4));

  const wrongConfirmationCount = rawV3.slice();
  new DataView(wrongConfirmationCount.buffer).setUint32(firstConfirmationOffset, 30, true);
  assert.throws(
    () => solanaSccpVoteAccountDataFromRawVoteState(wrongConfirmationCount, 3n, voteAccountAddress),
    /invalid Tower confirmation count/u,
  );

  const repeatedVoteSlot = rawV3.slice();
  new DataView(repeatedVoteSlot.buffer).setBigUint64(secondVoteSlotOffset, 11n, true);
  assert.throws(
    () => solanaSccpVoteAccountDataFromRawVoteState(repeatedVoteSlot, 3n, voteAccountAddress),
    /greater than the previous slot/u,
  );

  const noRoot = rawV3.slice();
  noRoot[rootOptionOffset] = 0;
  assert.throws(
    () => solanaSccpVoteAccountDataFromRawVoteState(noRoot, 3n, voteAccountAddress),
    /rooted vote state/u,
  );

  const rootOverlapsVoteStack = rawV3.slice();
  new DataView(rootOverlapsVoteStack.buffer).setBigUint64(rootOptionOffset + 1, 11n, true);
  assert.throws(
    () => solanaSccpVoteAccountDataFromRawVoteState(rootOverlapsVoteStack, 3n, voteAccountAddress),
    /greater than the previous slot/u,
  );

  const badPriorVoters = rawV3.slice();
  const priorVotersOffset = rootOptionOffset + 1 + 8 + 8 + (2 * (8 + 32));
  const zeroPriorVoterWithEpochBounds = rawV3.slice();
  new DataView(zeroPriorVoterWithEpochBounds.buffer).setBigUint64(
    priorVotersOffset + 32,
    1n,
    true,
  );
  assert.throws(
    () =>
      solanaSccpVoteAccountDataFromRawVoteState(
        zeroPriorVoterWithEpochBounds,
        3n,
        voteAccountAddress,
      ),
    /priorVoters\[0\]/u,
  );
  badPriorVoters[priorVotersOffset + (32 * (32 + 8 + 8)) + 8] = 2;
  assert.throws(
    () => solanaSccpVoteAccountDataFromRawVoteState(badPriorVoters, 3n, voteAccountAddress),
    /priorVoters/u,
  );

  const v4AuthorizedVotersOffset = 4 + 32 + 32 + 32 + 32 + 2 + 2 + 8 + 1 + 48 + 8
    + (31 * (1 + 8 + 4)) + 1 + 8;
  const zeroFutureAuthorizedVoter = sampleSolanaVoteStateV4Account(true, 4);
  const fourthAuthorizedVoterKeyOffset = v4AuthorizedVotersOffset + 8 + (3 * (8 + 32)) + 8;
  zeroFutureAuthorizedVoter.fill(
    0,
    fourthAuthorizedVoterKeyOffset,
    fourthAuthorizedVoterKeyOffset + 32,
  );
  assert.throws(
    () =>
      solanaSccpVoteAccountDataFromRawVoteState(
        zeroFutureAuthorizedVoter,
        3n,
        voteAccountAddress,
      ),
    /authorizedVoters\[3\]\.authorizedVoter/u,
  );
  const tooManyV4AuthorizedVoters = sampleSolanaVoteStateV4Account(true, 5);
  assert.throws(
    () =>
      solanaSccpVoteAccountDataFromRawVoteState(
        tooManyV4AuthorizedVoters,
        3n,
        voteAccountAddress,
      ),
    /1..4 entries for VoteStateV4/u,
  );

  const tooManyEpochCredits = rawV4.slice();
  const v4EpochCreditsOffset = v4AuthorizedVotersOffset + 8 + (2 * (8 + 32));
  new DataView(tooManyEpochCredits.buffer).setBigUint64(v4EpochCreditsOffset, 65n, true);
  assert.throws(
    () => solanaSccpVoteAccountDataFromRawVoteState(tooManyEpochCredits, 3n, voteAccountAddress),
    /epochCredits/u,
  );

  const v3EpochCreditsOffset = priorVotersOffset + (32 * (32 + 8 + 8)) + 8 + 1;
  const futureEpochCredit = rawV3.slice();
  const futureEpochCreditView = new DataView(futureEpochCredit.buffer);
  futureEpochCreditView.setBigUint64(v3EpochCreditsOffset, 1n, true);
  futureEpochCreditView.setBigUint64(v3EpochCreditsOffset + 8, 4n, true);
  futureEpochCreditView.setBigUint64(v3EpochCreditsOffset + 16, 1n, true);
  assert.throws(
    () => solanaSccpVoteAccountDataFromRawVoteState(futureEpochCredit, 3n, voteAccountAddress),
    /epochCredits/u,
  );

  const lastTimestampSlotOffset = v3EpochCreditsOffset + 8;
  const futureLastTimestampSlot = rawV3.slice();
  new DataView(futureLastTimestampSlot.buffer).setBigUint64(lastTimestampSlotOffset, 42n, true);
  assert.throws(
    () => solanaSccpVoteAccountDataFromRawVoteState(futureLastTimestampSlot, 3n, voteAccountAddress),
    /lastTimestamp/u,
  );

  const negativeLastTimestamp = rawV3.slice();
  const negativeLastTimestampView = new DataView(negativeLastTimestamp.buffer);
  negativeLastTimestampView.setBigUint64(lastTimestampSlotOffset, 41n, true);
  negativeLastTimestampView.setBigInt64(lastTimestampSlotOffset + 8, -1n, true);
  assert.throws(
    () => solanaSccpVoteAccountDataFromRawVoteState(negativeLastTimestamp, 3n, voteAccountAddress),
    /lastTimestamp/u,
  );

  const nonzeroPadding = rawV3.slice();
  nonzeroPadding[nonzeroPadding.length - 1] = 1;
  assert.throws(
    () => solanaSccpVoteAccountDataFromRawVoteState(nonzeroPadding, 3n, voteAccountAddress),
    /padding/u,
  );

  assert.throws(
    () => solanaSccpVoteAccountDataFromRawVoteState(rawV3, 0n, voteAccountAddress),
    /at or before epoch/u,
  );
});

test("derives Solana stake account data hash from raw StakeStateV2 account bytes", () => {
  const raw = sampleSolanaStakeStateV2StakeAccount();
  const parsed = solanaSccpStakeAccountDataFromRawStakeStateV2(raw);
  assert.deepEqual(Array.from(parsed.staker), Array(32).fill(0x81));
  assert.deepEqual(Array.from(parsed.withdrawer), Array(32).fill(0x91));
  assert.deepEqual(Array.from(parsed.voterPubkey), Array(32).fill(0xa1));
  assert.equal(parsed.delegatedStake, 1_000n);
  assert.equal(parsed.activationEpoch, 2n);
  assert.equal(parsed.deactivationEpoch, 9n);
  assert.deepEqual(
    Array.from(parsed.warmupCooldownRateBytes),
    [0x0a, 0xd7, 0xa3, 0x70, 0x3d, 0x0a, 0xb7, 0x3f],
  );
  assert.equal(parsed.creditsObserved, 123n);
  assert.equal(parsed.stakeFlags, 1n);
  assert.equal(
    solanaSccpStakeAccountDataHashFromRawStakeStateV2(raw),
    solanaSccpStakeAccountDataHash(parsed),
  );

  const wrongVariant = raw.slice();
  new DataView(wrongVariant.buffer).setUint32(0, 1, true);
  assert.throws(() => solanaSccpStakeAccountDataFromRawStakeStateV2(wrongVariant), /StakeStateV2::Stake/);
  assert.throws(() => solanaSccpStakeAccountDataFromRawStakeStateV2(raw.slice(0, 199)), /200-byte/);

  const hiddenPadding = raw.slice();
  hiddenPadding[197] = 1;
  assert.throws(() => solanaSccpStakeAccountDataFromRawStakeStateV2(hiddenPadding), /padding/);

  const unknownFlags = raw.slice();
  unknownFlags[196] = 2;
  assert.throws(() => solanaSccpStakeAccountDataFromRawStakeStateV2(unknownFlags), /StakeFlags/);

  const zeroVoter = raw.slice();
  zeroVoter.fill(0, 124, 156);
  assert.throws(() => solanaSccpStakeAccountDataFromRawStakeStateV2(zeroVoter), /voterPubkey/);

  const zeroDelegation = raw.slice();
  new DataView(zeroDelegation.buffer).setBigUint64(156, 0n, true);
  assert.throws(
    () => solanaSccpStakeAccountDataFromRawStakeStateV2(zeroDelegation),
    /delegatedStake/,
  );

  const legacyWarmupCooldownRate = raw.slice();
  legacyWarmupCooldownRate.set([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xd0, 0x3f], 180);
  assert.equal(
    Array.from(
      solanaSccpStakeAccountDataFromRawStakeStateV2(legacyWarmupCooldownRate)
        .warmupCooldownRateBytes,
    ).join(","),
    "0,0,0,0,0,0,208,63",
  );

  const zeroWarmupCooldownRate = raw.slice();
  zeroWarmupCooldownRate.fill(0, 180, 188);
  assert.throws(
    () => solanaSccpStakeAccountDataFromRawStakeStateV2(zeroWarmupCooldownRate),
    /warmupCooldownRateBytes/,
  );

  const invalidEpochOrder = raw.slice();
  new DataView(invalidEpochOrder.buffer).setBigUint64(172, 2n, true);
  assert.throws(
    () => solanaSccpStakeAccountDataFromRawStakeStateV2(invalidEpochOrder),
    /deactivationEpoch/,
  );
});

test("derives Solana stake account state hash for account openings", () => {
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
  assert.throws(
    () => solanaSccpStakeAccountStateHash({ ...input, validator_epoch: input.epoch }),
    /epoch must not use multiple aliases/,
  );
  assert.throws(
    () =>
      solanaSccpStakeAccountStateHash({
        ...input,
        validator_vote_account_addresses: input.validatorVoteAccountAddresses,
      }),
    /validatorVoteAccountAddresses must not use multiple aliases/,
  );
  assert.throws(
    () =>
      solanaSccpStakeAccountStateHash({
        ...input,
        voteAccountHashes: input.validatorVoteAccountHashes,
      }),
    /validatorVoteAccountHashes must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpStakeAccountStateHash({ ...input, validatorVoteAccountAddresses: [`0x${"33".repeat(32)}`] }),
    /validatorVoteAccountAddresses must match validatorPublicKeys/,
  );
  assert.throws(
    () =>
      solanaSccpStakeAccountStateHash({
        ...input,
        validatorVoteAccountAddresses: [`0x${"33".repeat(32)}`, `0x${"33".repeat(32)}`],
      }),
    /validatorVoteAccountAddresses must not contain duplicates/,
  );
  assert.throws(
    () =>
      solanaSccpStakeAccountStateHash({
        ...input,
        validatorStakeAccountAddresses: [`0x${"55".repeat(32)}`, `0x${"44".repeat(32)}`],
      }),
    /validatorStakeAccountAddresses\[1\] must differ from vote account/,
  );
  assert.throws(
    () =>
      solanaSccpStakeAccountStateHash({
        ...input,
        validatorVoteAccountAddresses: [`0x${"66".repeat(32)}`, `0x${"44".repeat(32)}`],
      }),
    /validatorVoteAccountAddresses\[0\] must not overlap stake accounts/,
  );
  assert.throws(
    () =>
      solanaSccpStakeAccountStateHash({
        ...input,
        validatorVoteAccountHashes: [`0x${"77".repeat(32)}`, `0x${"00".repeat(32)}`],
      }),
    /validatorVoteAccountHashes\[1\] must not be zero/,
  );
});

test("derives Solana stake history hash for delegated and effective stake", () => {
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
  assert.throws(
    () =>
      solanaSccpStakeHistoryHash({
        ...input,
        validator_public_keys: input.validatorPublicKeys,
      }),
    /validatorPublicKeys must not use multiple aliases/,
  );
  assert.throws(
    () =>
      solanaSccpStakeHistoryHash({
        ...input,
        delegated_stakes: input.validatorDelegatedStakes,
      }),
    /validatorDelegatedStakes must not use multiple aliases/,
  );
  assert.throws(
    () =>
      solanaSccpStakeHistoryHash({
        ...input,
        stake_history: input.stakeHistoryEntries,
      }),
    /stakeHistoryEntries must not use multiple aliases/,
  );
  assert.throws(
    () =>
      solanaSccpStakeHistoryHash({
        ...input,
        activation_epochs: input.validatorActivationEpochs,
      }),
    /validatorActivationEpochs must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpStakeHistoryHash({ ...input, validatorDelegatedStakes: [0n, 3n] }),
    /validatorDelegatedStakes\[0\] must be at least validatorStakes\[0\]/,
  );
  assert.throws(
    () => solanaSccpStakeHistoryHash({ ...input, validatorStakes: [1n, 1n] }),
    /validatorStakes\[1\] must equal replayed StakeHistory effective stake/,
  );
  assert.throws(
    () =>
      solanaSccpStakeHistoryHash({
        ...input,
        stakeHistoryEntries: [
          input.stakeHistoryEntries[0],
          { ...input.stakeHistoryEntries[1], effective: 4n },
        ],
      }),
    /signed epoch StakeHistory effective stake must equal replayed validator effective stake/,
  );
  assert.throws(
    () => solanaSccpStakeHistoryHash({ ...input, stakeHistoryEntries: input.stakeHistoryEntries.slice(0, 1) }),
    /stakeHistoryEntries must include the signed epoch/,
  );
  assert.throws(
    () => solanaSccpStakeHistoryHash({ ...input, stakeHistoryEntries: [...input.stakeHistoryEntries].reverse() }),
    /stakeHistoryEntries must be sorted by strictly increasing epoch/,
  );
});

test("derives Solana StakeHistory sysvar data hash from sorted entries", () => {
  const input = {
    stakeHistoryEntries: [
      { epoch: 2n, effective: 10n, activating: 3n, deactivating: 1n },
      { epoch: 3n, effective: 12n, activating: 0n, deactivating: 0n },
    ],
  };

  const canonical = canonicalSolanaSccpStakeHistorySysvarDataBytes(input);
  assert.equal(canonical.length, 72);
  const newestEpoch = new DataView(
    canonical.buffer,
    canonical.byteOffset,
    canonical.byteLength,
  ).getBigUint64(8, true);
  assert.equal(newestEpoch, 3n);
  const hash = solanaSccpStakeHistorySysvarDataHash(input);
  assert.match(hash, /^0x[0-9a-f]{64}$/u);
  assert.equal(solanaSccpStakeHistorySysvarDataHashFromRawData(canonical), hash);
  assert.throws(
    () =>
      solanaSccpStakeHistorySysvarDataHash({
        ...input,
        stake_history: input.stakeHistoryEntries,
      }),
    /stakeHistoryEntries must not use multiple aliases/u,
  );
  assert.notEqual(
    hash,
    solanaSccpStakeHistorySysvarDataHash({
      stakeHistoryEntries: [
        input.stakeHistoryEntries[0],
        { ...input.stakeHistoryEntries[1], effective: 13n },
      ],
    }),
  );
  assert.throws(
    () =>
      solanaSccpStakeHistorySysvarDataHash({
        stakeHistoryEntries: [...input.stakeHistoryEntries].reverse(),
      }),
    /stakeHistoryEntries must be sorted by strictly increasing epoch/u,
  );
  assert.throws(
    () => solanaSccpStakeHistorySysvarDataHashFromRawData(canonical.slice(0, 9)),
    /bincode Vec/u,
  );
  const wrongCount = canonical.slice();
  new DataView(wrongCount.buffer, wrongCount.byteOffset, wrongCount.byteLength).setBigUint64(0, 3n, true);
  assert.throws(
    () => solanaSccpStakeHistorySysvarDataHashFromRawData(wrongCount),
    /1..512/u,
  );
  const ascendingRaw = canonical.slice();
  const newestEntry = canonical.slice(8, 40);
  const oldestEntry = canonical.slice(40, 72);
  ascendingRaw.set(oldestEntry, 8);
  ascendingRaw.set(newestEntry, 40);
  assert.throws(
    () => solanaSccpStakeHistorySysvarDataHashFromRawData(ascendingRaw),
    /newest-first/u,
  );
});

test("derives Solana tower lockout hash for finalized-slot context", () => {
  const input = {
    finalizedSlot: 1_296_096n,
    rootedSlot: 1_296_065n,
    parentSlot: 1_296_095n,
    parentBankHash: `0x${"33".repeat(32)}`,
  };

  assert.equal(SCCP_SOLANA_TOWER_LOCKOUT_CONFIRMATION_DEPTH, 32n);
  assert.equal(SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH, 31n);
  assert.equal(canonicalSolanaSccpTowerLockoutBytes(input).length, 73);
  assert.match(solanaSccpTowerLockoutHash(input), /^0x[0-9a-f]{64}$/);
  assert.equal(
    solanaSccpTowerLockoutHash({ ...input, epoch: 3n }),
    solanaSccpTowerLockoutHash(input),
  );
  assert.throws(
    () => solanaSccpTowerLockoutHash({ ...input, finalized_slot: input.finalizedSlot }),
    /finalizedSlot must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpTowerLockoutHash({ ...input, epoch: 3n, validatorEpoch: 3n }),
    /epoch must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpTowerLockoutHash({ ...input, rooted_slot: input.rootedSlot }),
    /rootedSlot must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpTowerLockoutHash({ ...input, parent_bank_hash: input.parentBankHash }),
    /parentBankHash must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpTowerLockoutHash({ ...input, epoch: 4n }),
    /epoch must match Solana mainnet finalizedSlot/,
  );
  assert.throws(
    () => solanaSccpTowerLockoutHash({ ...input, rootedSlot: 1_296_066n }),
    /rootedSlot must satisfy/,
  );
  assert.throws(
    () => solanaSccpTowerLockoutHash({ ...input, parentSlot: 1_296_094n }),
    /parentSlot must be the direct parent/,
  );
  assert.throws(
    () => solanaSccpTowerLockoutHash({ ...input, parentBankHash: `0x${"00".repeat(32)}` }),
    /parentBankHash must not be zero/,
  );
});

test("derives Solana tower replay hash for finalized-slot vote stack", () => {
  const input = {
    finalizedSlot: 1_296_096n,
    rootedSlot: 1_296_065n,
    parentSlot: 1_296_095n,
    bankForkHash: HEX32_A,
    towerVoteSlots: Array.from({ length: Number(SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH) }, (_, index) =>
      1_296_066n + BigInt(index),
    ),
  };

  assert.equal(canonicalSolanaSccpTowerReplayBytes(input).length, 573);
  assert.match(solanaSccpTowerReplayHash(input), /^0x[0-9a-f]{64}$/);
  assert.equal(
    solanaSccpTowerReplayHash({ ...input, epoch: 3n }),
    solanaSccpTowerReplayHash(input),
  );
  assert.notEqual(
    solanaSccpTowerReplayHash({ ...input, bankForkHash: HEX32_B }),
    solanaSccpTowerReplayHash(input),
  );
  assert.throws(
    () => solanaSccpTowerReplayHash({ ...input, finalized_slot: input.finalizedSlot }),
    /finalizedSlot must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpTowerReplayHash({ ...input, bank_fork_hash: input.bankForkHash }),
    /bankForkHash must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpTowerReplayHash({ ...input, voteSlots: input.towerVoteSlots }),
    /towerVoteSlots must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpTowerReplayHash({ ...input, bankForkHash: SCCP_ZERO_HASH_V1 }),
    /bankForkHash must not be zero/,
  );
  assert.throws(
    () => solanaSccpTowerReplayHash({ ...input, epoch: 4n }),
    /epoch must match Solana mainnet finalizedSlot/,
  );
  assert.throws(
    () => solanaSccpTowerReplayHash({ ...input, towerVoteSlots: input.towerVoteSlots.slice(1) }),
    /towerVoteSlots must contain 31 active post-root slots/,
  );
  const unsortedVoteSlots = input.towerVoteSlots.slice();
  [unsortedVoteSlots[0], unsortedVoteSlots[1]] = [unsortedVoteSlots[1], unsortedVoteSlots[0]];
  assert.throws(
    () => solanaSccpTowerReplayHash({ ...input, towerVoteSlots: unsortedVoteSlots }),
    /towerVoteSlots must be strictly increasing/,
  );
  const wrongLastVoteSlots = input.towerVoteSlots.slice();
  wrongLastVoteSlots[wrongLastVoteSlots.length - 1] -= 1n;
  assert.throws(
    () => solanaSccpTowerReplayHash({ ...input, towerVoteSlots: wrongLastVoteSlots }),
    /last towerVoteSlots entry must equal finalizedSlot/,
  );
});

test("derives Solana bank-fork hash for finalized-bank context", () => {
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
  const accountsLtHashChecksum = solanaSccpAccountsLtHashChecksum(accountsLtHash);
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
    accountsLtHashChecksum,
  };

  assert.match(bankHash, /^0x[0-9a-f]{64}$/);
  assert.equal(canonicalSolanaSccpBankForkBytes(input).length, 229);
  assert.equal(
    solanaSccpBankForkHash(input),
    "0x8c496fb25a4499947e454a84f638211a84445748bc5242fbb6fb511edd82e531",
  );
  assert.throws(
    () => solanaSccpBankForkHash({ ...input, bankSignatureCount: 0n }),
    /bankSignatureCount must be nonzero/,
  );
  assert.equal(
    solanaSccpBankForkHash({ ...input, epoch: 3n }),
    solanaSccpBankForkHash(input),
  );
  assert.throws(
    () => solanaSccpBankForkHash({ ...input, finalized_slot: input.finalizedSlot }),
    /finalizedSlot must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpBankForkHash({ ...input, epoch: 3n, validatorEpoch: 3n }),
    /epoch must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpBankForkHash({ ...input, parent_slot: input.parentSlot }),
    /parentSlot must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpBankForkHash({ ...input, bank_hash: input.bankHash }),
    /bankHash must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpBankForkHash({
      ...input,
      receiptOrMessageRoot: input.transactionStatusRoot,
    }),
    /transactionStatusRoot must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpBankForkHash({ ...input, epoch: 4n }),
    /epoch must match Solana mainnet finalizedSlot/,
  );
  assert.throws(
    () => solanaSccpBankForkHash({ ...input, parentSlot: 1_296_094n }),
    /parentSlot must be the direct parent/,
  );
  assert.throws(
    () => solanaSccpBankForkHash({ ...input, bankHash: input.parentBankHash }),
    /parentBankHash must differ from bankHash/,
  );
  assert.throws(
    () => solanaSccpBankForkHash({ ...input, bankHash: `0x${"44".repeat(32)}` }),
    /bankHash must match Agave bank hash inputs/,
  );
  assert.throws(
    () => solanaSccpBankForkHash({ ...input, blockhash: `0x${"00".repeat(32)}` }),
    /blockhash must not be zero/,
  );
  assert.throws(
    () => solanaSccpBankForkHash({ ...input, accountInclusionRoot: `0x${"00".repeat(32)}` }),
    /accountInclusionRoot must not be zero/,
  );
  assert.throws(
    () => solanaSccpBankForkHash({ ...input, accountsLtHashChecksum: `0x${"00".repeat(32)}` }),
    /accountsLtHashChecksum must not be zero/,
  );
  assert.throws(
    () => solanaSccpBankForkHash({ ...input, accountsLtHashChecksum: `0x${"88".repeat(32)}` }),
    /accountsLtHashChecksum must match accountsLtHash/,
  );
  assert.throws(
    () => solanaSccpAgaveBankHash({
      parentBankHash,
      parent_bank_hash: parentBankHash,
      bankSignatureCount,
      blockhash,
      accountsLtHash,
    }),
    /parentBankHash must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpAgaveBankHash({
      parentBankHash,
      bankSignatureCount,
      bank_signature_count: bankSignatureCount,
      blockhash,
      accountsLtHash,
    }),
    /bankSignatureCount must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpAgaveBankHash({
      parentBankHash,
      bankSignatureCount,
      blockhash,
      blockhashBytes: Buffer.from(blockhash.slice(2), "hex"),
      accountsLtHash,
    }),
    /blockhash must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpAgaveBankHash({
      parentBankHash,
      bankSignatureCount,
      blockhash,
      accountsLtHash,
      accounts_lt_hash: accountsLtHash,
    }),
    /accountsLtHash must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpAgaveBankHash({ ...input, bankHashHardForkData: new Uint8Array(1025) }),
    /bankHashHardForkData is too large/,
  );
});

test("derives Solana account inclusion leaves, branches, and roots", () => {
  const finalizedSlot = 1_296_096n;
  const openings = [
    {
      address: `0x${"31".repeat(32)}`,
      owner: SCCP_SOLANA_VOTE_PROGRAM_ID,
      lamports: 1_000_000n,
      rentEpoch: 0n,
      executable: false,
      dataHash: `0x${"91".repeat(32)}`,
    },
    {
      address: `0x${"41".repeat(32)}`,
      owner: SCCP_SOLANA_STAKE_PROGRAM_ID,
      lamports: 1_000_001n,
      rentEpoch: 0n,
      executable: false,
      dataHash: `0x${"92".repeat(32)}`,
    },
    {
      address: `0x${"51".repeat(32)}`,
      owner: SCCP_SOLANA_STAKE_PROGRAM_ID,
      lamports: 1_000_002n,
      rentEpoch: 0n,
      executable: false,
      dataHash: `0x${"93".repeat(32)}`,
    },
  ];
  const rawData = [
    `0x${"01".repeat(64)}`,
    `0x${"02".repeat(64)}`,
    `0x${"03".repeat(64)}`,
  ];
  const leafInputs = openings.map((opening, index) => ({
    finalizedSlot,
    opening,
    rawData: rawData[index],
  }));
  assert.equal(canonicalSolanaSccpAccountInclusionLeafBytes(leafInputs[0]).length, 109);
  assert.match(solanaSccpAccountRawDataHash(rawData[0]), /^0x[0-9a-f]{64}$/);
  assert.throws(
    () =>
      canonicalSolanaSccpAccountInclusionLeafBytes({
        ...leafInputs[0],
        finalized_slot: finalizedSlot,
      }),
    /finalizedSlot must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalSolanaSccpAccountInclusionLeafBytes({
        ...leafInputs[0],
        accountOpening: leafInputs[0].opening,
      }),
    /opening must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalSolanaSccpAccountInclusionLeafBytes({
        ...leafInputs[0],
        raw_data: rawData[0],
      }),
    /rawData must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalSolanaSccpAccountInclusionLeafBytes({
        ...leafInputs[0],
        rawDataHash: `0x${"44".repeat(32)}`,
      }),
    /rawDataHash must match rawData/,
  );
  assert.throws(
    () =>
      canonicalSolanaSccpAccountInclusionLeafBytes({
        ...leafInputs[0],
        opening: { ...leafInputs[0].opening, accountAddress: leafInputs[0].opening.address },
      }),
    /opening\.address must not use multiple aliases/,
  );
  const leaves = leafInputs.map(solanaSccpAccountInclusionLeafHash);
  assert.equal(canonicalSolanaSccpAccountInclusionNodeBytes(leaves[0], leaves[1]).length, 65);
  assert.match(solanaSccpAccountInclusionNodeHash(leaves[0], leaves[1]), /^0x[0-9a-f]{64}$/);

  const { root, branches } = solanaSccpAccountInclusionRootAndBranches(leaves);
  assert.match(root, /^0x[0-9a-f]{64}$/);
  assert.equal(branches.length, leaves.length);
  assert.equal(solanaSccpAccountInclusionRootFromBranch(leaves[0], branches[0]), root);
  assert.equal(solanaSccpAccountInclusionRootFromBranch(leaves[1], branches[1]), root);
  assert.ok(Object.isFrozen(branches));
  assert.ok(Object.isFrozen(branches[0]));
  assert.throws(() => branches.push([]), /object is not extensible|read only/);
  assert.throws(() => branches[0].push(HEX32_E), /object is not extensible|read only/);

  const openedWitness = solanaSccpOpenedAccountInclusionWitness({
    finalizedSlot,
    validatorVoteAccountOpenings: [openings[0]],
    validatorVoteAccountRawData: [rawData[0]],
    validatorStakeAccountOpenings: [openings[1]],
    validatorStakeAccountRawData: [rawData[1]],
    stakeHistorySysvarOpening: openings[2],
    stakeHistorySysvarRawData: rawData[2],
    accountInclusionRoot: root,
  });
  assert.deepEqual(openedWitness.branches, branches);
  assert.deepEqual(openedWitness.validatorVoteAccountBranches, [branches[0]]);
  assert.deepEqual(openedWitness.validatorStakeAccountBranches, [branches[1]]);
  assert.deepEqual(openedWitness.stakeHistorySysvarBranch, branches[2]);
  assert.ok(Object.isFrozen(openedWitness));
  assert.ok(Object.isFrozen(openedWitness.branches));
  assert.ok(Object.isFrozen(openedWitness.branches[0]));
  assert.throws(() => openedWitness.branches.push([]), /object is not extensible|read only/);
  assert.throws(() => openedWitness.branches[0].push(HEX32_E), /object is not extensible|read only/);
  assert.throws(
    () => solanaSccpOpenedAccountInclusionWitness({
      finalizedSlot,
      finalized_slot: finalizedSlot,
      validatorVoteAccountOpenings: [openings[0]],
      validatorVoteAccountRawData: [rawData[0]],
      validatorStakeAccountOpenings: [openings[1]],
      validatorStakeAccountRawData: [rawData[1]],
      stakeHistorySysvarOpening: openings[2],
      stakeHistorySysvarRawData: rawData[2],
    }),
    /finalizedSlot must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpOpenedAccountInclusionWitness({
      finalizedSlot,
      validatorVoteAccountOpenings: [openings[0]],
      vote_account_openings: [openings[0]],
      validatorVoteAccountRawData: [rawData[0]],
      validatorStakeAccountOpenings: [openings[1]],
      validatorStakeAccountRawData: [rawData[1]],
      stakeHistorySysvarOpening: openings[2],
      stakeHistorySysvarRawData: rawData[2],
    }),
    /validatorVoteAccountOpenings must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpOpenedAccountInclusionWitness({
      finalizedSlot,
      validatorVoteAccountOpenings: [openings[0]],
      validatorVoteAccountRawData: [rawData[0]],
      validatorStakeAccountOpenings: [openings[1]],
      validatorStakeAccountRawData: [rawData[1]],
      stakeHistorySysvarOpening: openings[2],
      stake_history_sysvar_opening: openings[2],
      stakeHistorySysvarRawData: rawData[2],
    }),
    /stakeHistorySysvarOpening must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpOpenedAccountInclusionWitness({
      finalizedSlot,
      validatorVoteAccountOpenings: [openings[0]],
      validatorVoteAccountRawData: [rawData[0]],
      validatorStakeAccountOpenings: [openings[1]],
      validatorStakeAccountRawData: [rawData[1]],
      stakeHistorySysvarOpening: openings[2],
      stakeHistorySysvarRawData: rawData[2],
      accountInclusionRoot: root,
      accountsRoot: root,
    }),
    /accountInclusionRoot must not use multiple aliases/,
  );
  assert.throws(
    () => solanaSccpOpenedAccountInclusionWitness({
      finalizedSlot,
      validatorVoteAccountOpenings: [openings[0]],
      validatorVoteAccountRawData: [rawData[0]],
      validatorStakeAccountOpenings: [{ ...openings[1], address: openings[0].address }],
      validatorStakeAccountRawData: [rawData[1]],
      stakeHistorySysvarOpening: openings[2],
      stakeHistorySysvarRawData: rawData[2],
    }),
    /opened account addresses must be unique/,
  );
  assert.throws(
    () => solanaSccpOpenedAccountInclusionWitness({
      finalizedSlot,
      validatorVoteAccountOpenings: [openings[0]],
      validatorVoteAccountRawData: [rawData[0]],
      validatorStakeAccountOpenings: [openings[1]],
      validatorStakeAccountRawData: [rawData[1]],
      stakeHistorySysvarOpening: openings[2],
      stakeHistorySysvarRawData: rawData[2],
      accountInclusionRoot: `0x${"77".repeat(32)}`,
    }),
    /accountInclusionRoot must match opened account inclusion witness/,
  );

  const mutatedLeaf = solanaSccpAccountInclusionLeafHash({
    finalizedSlot,
    opening: openings[0],
    rawData: `0x${"04".repeat(64)}`,
  });
  assert.notEqual(solanaSccpAccountInclusionRootFromBranch(mutatedLeaf, branches[0]), root);
  assert.throws(
    () => solanaSccpAccountInclusionRootFromBranch(`0x${"00".repeat(32)}`, []),
    /leaf/,
  );
  assert.throws(
    () =>
      solanaSccpAccountInclusionRootFromBranch(
        leaves[0],
        Array.from({ length: 65 }, () => HEX32_E),
      ),
    /at most 64/,
  );
  assert.throws(
    () => solanaSccpOpenedAccountInclusionWitness({
      finalizedSlot,
      validatorVoteAccountOpenings: Array.from(
        { length: SCCP_SOLANA_MAX_VALIDATORS + 1 },
        () => openings[0],
      ),
      validatorVoteAccountRawData: Array.from(
        { length: SCCP_SOLANA_MAX_VALIDATORS + 1 },
        () => rawData[0],
      ),
      validatorStakeAccountOpenings: [openings[1]],
      validatorStakeAccountRawData: [rawData[1]],
      stakeHistorySysvarOpening: openings[2],
      stakeHistorySysvarRawData: rawData[2],
    }),
    /validatorVoteAccountOpenings.*at most/,
  );
  assert.throws(() => solanaSccpAccountRawDataHash("0x"), /rawData/);
  assert.throws(() => solanaSccpAccountInclusionRootAndBranches([leaves[0], leaves[0]]), /unique/);
});

test("derives Groth16 BN254 public signal words for EVM and TRON provers", () => {
  const signals = sccpGroth16Bn254PublicSignalWords({
    publicInputs: {
      version: 1,
      messageId: `0x${"11".repeat(32)}`,
      payloadHash: `0x${"22".repeat(32)}`,
      targetDomain: 5,
      commitmentRoot: `0x${"33".repeat(32)}`,
      finalityHeight: 19n,
      finalityBlockHash: `0x${"44".repeat(32)}`,
    },
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: `0x${"55".repeat(32)}`,
    destinationBindingHash: `0x${"66".repeat(32)}`,
  });

  assert.deepEqual(signals, [
    "0x0ffdbc782e79d1dc508e08af01e87f16d93b6e58e4861a0b8155455e3ee7a683",
    "0x0c5398ea95021a790e276e3ece1592b32b85751dc77e50293c867a5f2e0131bb",
    "0x21aac4195d8db839756f61c0780675823e15456c92acf135c36e02367c8fd11f",
    "0x01c73f2f9156a52493a9beabeec73e62deed32fcef2e3e6fac86a79f0764f0bc",
    "0x0ca6bbc36d23183d027c8df09f06c39e64abbb0bb4d6a4c37369d2c36f41a888",
    "0x2b153d0fe1bc6e2a6d44e851523edb1511dac55443ca80c22cbe9cb7423886dc",
    "0x2697e4e42f34b673b4aa254c6a92de09304e84c1a667c7d266777775a231efb4",
    "0x16fbe0c1d659f142b3e7815b24df66da3cfd89cc42d051b04bc31aae6925c396",
    "0x1157cd422e2089145c9cf93794dd6a0a1c3b1a611c22a5fe999d0542f62535d8",
  ]);

  const changedDestination = sccpGroth16Bn254PublicSignalWords({
    publicInputs: {
      version: 1,
      messageId: `0x${"11".repeat(32)}`,
      payloadHash: `0x${"22".repeat(32)}`,
      targetDomain: 5,
      commitmentRoot: `0x${"33".repeat(32)}`,
      finalityHeight: 19n,
      finalityBlockHash: `0x${"44".repeat(32)}`,
    },
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: `0x${"55".repeat(32)}`,
    destinationBindingHash: `0x${"67".repeat(32)}`,
  });
  assert.deepEqual(signals.slice(0, 8), changedDestination.slice(0, 8));
  assert.notEqual(signals[8], changedDestination[8]);
});

test("binds TRON Groth16 proof requests to public signals and relay context", () => {
  const bundleBytes = Uint8Array.from([5, 6, 7]);
  const sourceProofBytes = Uint8Array.from([9, 10]);
  const request = buildTronSccpProofRequest({
    publicInputs: sampleTronPublicInputs,
    bundleBytes,
    sourceProofBytes,
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBindingHash: HEX32_H,
  });

  const expectedSignals = sccpGroth16Bn254PublicSignalWords({
    publicInputs: sampleTronPublicInputs,
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBindingHash: HEX32_H,
  });

  assert.equal(request.backend, SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1);
  assert.equal(request.sourceDomain, SCCP_DOMAIN_SORA);
  assert.equal(request.targetDomain, SCCP_DOMAIN_TRON);
  assert.deepEqual(request.publicSignalWords, expectedSignals);
  assert.deepEqual(request.proofContext, {
    version: 1,
    statementHash: HEX32_G,
    destinationBindingHash: HEX32_H,
  });
  assert.equal(Object.isFrozen(request), true);
  assert.equal(Object.isFrozen(request.publicInputs), true);
  assert.equal(Object.isFrozen(request.publicSignalWords), true);
  assert.match(request.requestHash, /^0x[0-9a-f]{64}$/);
  assert.notEqual(
    request.requestHash,
    buildTronSccpProofRequest({
      publicInputs: sampleTronPublicInputs,
      bundleBytes: [5, 6, 7],
      sourceProofBytes: [9, 10],
      sourceDomain: SCCP_DOMAIN_SORA,
      statementHash: HEX32_G,
      destinationBindingHash: HEX32_B,
    }).requestHash,
  );
  assert.notEqual(
    request.requestHash,
    buildTronSccpProofRequest({
      publicInputs: sampleTronPublicInputs,
      bundleBytes: [5, 6, 7],
      sourceProofBytes: [9, 11],
      sourceDomain: SCCP_DOMAIN_SORA,
      statementHash: HEX32_G,
      destinationBindingHash: HEX32_H,
    }).requestHash,
  );
  assert.notEqual(
    request.requestHash,
    buildTronSccpProofRequest({
      publicInputs: sampleTronPublicInputs,
      bundleBytes: [5, 6, 7, 9],
      sourceProofBytes: [10],
      sourceDomain: SCCP_DOMAIN_SORA,
      statementHash: HEX32_G,
      destinationBindingHash: HEX32_H,
    }).requestHash,
  );
  assert.throws(
    () =>
      buildTronSccpProofRequest({
        publicInputs: sampleTronPublicInputs,
        public_inputs: sampleTronPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /publicInputs must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildTronSccpProofRequest({
        publicInputs: sampleTronPublicInputs,
        bundleBytes: [5, 6, 7],
        bundle_bytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /bundleBytes must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildTronSccpProofRequest({
        publicInputs: sampleTronPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        source_proof_bytes: [9, 10],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /sourceProofBytes must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildTronSccpProofRequest({
        publicInputs: sampleTronPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        source_domain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /sourceDomain must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildTronSccpProofRequest({
        publicInputs: sampleTronPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        proofContext: { statementHash: HEX32_G, destinationBindingHash: HEX32_H },
        proof_context: { statementHash: HEX32_G, destinationBindingHash: HEX32_H },
      }),
    /proofContext must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildTronSccpProofRequest({
        publicInputs: sampleTronPublicInputs,
        bundleBytes: [5, 6, 7],
        destinationBindingHash: HEX32_H,
    }),
    /statementHash must be a hex string/,
  );
  assert.throws(
    () =>
      buildTronSccpProofRequest({
        publicInputs: { ...sampleTronPublicInputs, payloadHash: SCCP_ZERO_HASH_V1 },
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /publicInputs\.payloadHash must not be zero/,
  );
  assert.throws(
    () =>
      buildTronSccpProofRequest({
        publicInputs: { ...sampleTronPublicInputs, payloadHash: `${sampleTronPublicInputs.payloadHash} ` },
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /publicInputs\.payloadHash must be canonical hex/,
  );
  assert.throws(
    () =>
      buildTronSccpProofRequest({
        publicInputs: sampleTronPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: ` ${HEX32_G}`,
        destinationBindingHash: HEX32_H,
      }),
    /statementHash must be canonical hex/,
  );
  assert.throws(
    () =>
      buildTronSccpProofRequest({
        publicInputs: sampleTronPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_ETH,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /sourceDomain must be SORA/,
  );
  assert.throws(
    () =>
      buildTronSccpProofRequest({
        publicInputs: { ...sampleTronPublicInputs, targetDomain: SCCP_DOMAIN_TON },
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /publicInputs\.targetDomain must be TRON/,
  );
  assert.throws(
    () =>
      buildTronSccpProofRequest({
        publicInputs: sampleTronPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: SCCP_ZERO_HASH_V1,
        destinationBindingHash: HEX32_H,
      }),
    /statementHash must not be zero/,
  );
  assert.throws(
    () =>
      buildTronSccpProofRequest({
        publicInputs: sampleTronPublicInputs,
        bundleBytes: [],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /bundleBytes must not be empty/,
  );
  assert.throws(
    () =>
      buildTronSccpProofRequest({
        publicInputs: sampleTronPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        backend: "debug-tron-backend",
      }),
    /backend must be tron-groth16-bn254-v1/,
  );

  bundleBytes[0] = 99;
  sourceProofBytes[0] = 99;
  assert.deepEqual(Array.from(request.bundleBytes), [5, 6, 7]);
  assert.deepEqual(Array.from(request.sourceProofBytes), [9, 10]);

  const exposedPublicInputs = request.publicInputsBytes;
  const exposedBundle = request.bundleBytes;
  const exposedSourceProof = request.sourceProofBytes;
  exposedPublicInputs[0] = 99;
  exposedBundle[0] = 99;
  exposedSourceProof[0] = 99;
  assert.notEqual(request.publicInputsBytes[0], 99);
  assert.deepEqual(Array.from(request.bundleBytes), [5, 6, 7]);
  assert.deepEqual(Array.from(request.sourceProofBytes), [9, 10]);
});

test("binds EVM-family Groth16 proof requests to public signals and relay context", () => {
  const request = buildEvmSccpProofRequest({
    publicInputs: sampleEvmPublicInputs,
    bundleBytes: [5, 6, 7],
    sourceProofBytes: [9, 10],
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBindingHash: HEX32_H,
  });

  const expectedSignals = sccpGroth16Bn254PublicSignalWords({
    publicInputs: sampleEvmPublicInputs,
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBindingHash: HEX32_H,
  });

  assert.equal(request.backend, SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1);
  assert.equal(request.sourceDomain, SCCP_DOMAIN_SORA);
  assert.equal(request.targetDomain, SCCP_DOMAIN_ETH);
  assert.deepEqual(request.publicSignalWords, expectedSignals);
  assert.deepEqual(request.proofContext, {
    version: 1,
    statementHash: HEX32_G,
    destinationBindingHash: HEX32_H,
  });
  assert.equal(Object.isFrozen(request), true);
  assert.equal(Object.isFrozen(request.publicInputs), true);
  assert.equal(Object.isFrozen(request.publicSignalWords), true);
  assert.equal(Object.isFrozen(request.proofContext), true);
  assert.match(request.requestHash, /^0x[0-9a-f]{64}$/);

  const bscRequest = buildEvmSccpProofRequest({
    publicInputs: { ...sampleEvmPublicInputs, targetDomain: SCCP_DOMAIN_BSC },
    bundleBytes: [5, 6, 7],
    sourceProofBytes: [9, 10],
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBindingHash: HEX32_H,
  });
  assert.equal(bscRequest.backend, SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1);
  assert.equal(bscRequest.targetDomain, SCCP_DOMAIN_BSC);
  assert.notEqual(bscRequest.requestHash, request.requestHash);
  assert.notEqual(bscRequest.publicSignalWords[2], request.publicSignalWords[2]);

  assert.notEqual(
    request.requestHash,
    buildEvmSccpProofRequest({
      publicInputs: sampleEvmPublicInputs,
      bundleBytes: [5, 6, 7],
      sourceProofBytes: [9, 11],
      sourceDomain: SCCP_DOMAIN_SORA,
      statementHash: HEX32_G,
      destinationBindingHash: HEX32_H,
    }).requestHash,
  );
  assert.notEqual(
    request.requestHash,
    buildEvmSccpProofRequest({
      publicInputs: sampleEvmPublicInputs,
      bundleBytes: [5, 6, 7, 9],
      sourceProofBytes: [10],
      sourceDomain: SCCP_DOMAIN_SORA,
      statementHash: HEX32_G,
      destinationBindingHash: HEX32_H,
    }).requestHash,
  );
  assert.throws(
    () =>
      buildEvmSccpProofRequest({
        publicInputs: sampleEvmPublicInputs,
        public_inputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /publicInputs must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildEvmSccpProofRequest({
        publicInputs: {
          ...sampleEvmPublicInputs,
          message_id: sampleEvmPublicInputs.messageId,
        },
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /publicInputs\.messageId must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      sccpGroth16Bn254PublicSignalWords({
        publicInputs: sampleEvmPublicInputs,
        public_inputs: sampleEvmPublicInputs,
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /publicInputs must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      buildEvmSccpProofRequest({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        bundle_bytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /bundleBytes must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildEvmSccpProofRequest({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        source_proof_bytes: [9, 10],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /sourceProofBytes must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildEvmSccpProofRequest({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        source_domain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /sourceDomain must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildEvmSccpProofRequest({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        proofContext: { statementHash: HEX32_G, destinationBindingHash: HEX32_H },
        proof_context: { statementHash: HEX32_G, destinationBindingHash: HEX32_H },
      }),
    /proofContext must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildEvmSccpProofRequest({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        destinationBindingHash: HEX32_H,
    }),
    /statementHash must be a hex string/,
  );
  assert.throws(
    () =>
      buildEvmSccpProofRequest({
        publicInputs: { ...sampleEvmPublicInputs, finalityHeight: 0n },
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /publicInputs\.finalityHeight must not be zero/,
  );
  assert.throws(
    () =>
      buildEvmSccpProofRequest({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_ETH,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /sourceDomain must be SORA/,
  );
  assert.throws(
    () =>
      buildEvmSccpProofRequest({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_TON,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /sourceDomain must be SORA/,
  );
  assert.throws(
    () =>
      buildEvmSccpProofRequest({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: false,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /sourceDomain must be a u32 domain id/,
  );
  assert.throws(
    () =>
      buildEvmSccpProofRequest({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: null,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /sourceDomain must be a u32 domain id/,
  );
  assert.throws(
    () =>
      buildEvmSccpProofRequest({
        publicInputs: { ...sampleEvmPublicInputs, targetDomain: true },
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /publicInputs\.targetDomain must be a u32 domain id/,
  );
  assert.throws(
    () =>
      buildEvmSccpProofRequest({
        publicInputs: { ...sampleEvmPublicInputs, targetDomain: SCCP_DOMAIN_TON },
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /publicInputs\.targetDomain must be ETH or BSC/,
  );
  assert.throws(
    () =>
      buildEvmSccpProofRequest({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: SCCP_ZERO_HASH_V1,
      }),
    /destinationBindingHash must not be zero/,
  );
  assert.throws(
    () =>
      buildEvmSccpProofRequest({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /bundleBytes must not be empty/,
  );
  assert.throws(
    () =>
      buildEvmSccpProofRequest({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        backend: "debug-evm-backend",
      }),
    /backend must be evm-groth16-bn254-v1/,
  );
});

test("builds EVM-family and TRON Groth16 contract-call submissions", () => {
  const evmBinding = sampleEvmDestinationBinding();
  const request = buildEvmSccpProofRequest({
    publicInputs: sampleEvmPublicInputs,
    bundleBytes: [5, 6, 7],
    sourceProofBytes: [9, 10],
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBinding: evmBinding,
  });
  const proofResult = wrapEvmSccpProofResult(GROTH16_PROOF_BYTES, request);
  const submission = buildEvmSccpSubmission({ proofResult });

  assert.equal(Object.isFrozen(proofResult), true);
  assert.equal(Object.isFrozen(proofResult.publicInputs), true);
  assert.equal(Object.isFrozen(proofResult.publicSignalWords), true);
  assert.equal(Object.isFrozen(proofResult.proofContext), true);
  const mutableRequest = {
    ...request,
    publicInputs: { ...request.publicInputs },
    publicSignalWords: [...request.publicSignalWords],
    proofContext: { ...request.proofContext },
  };
  const mutableRequestResult = wrapEvmSccpProofResult(GROTH16_PROOF_BYTES, mutableRequest);
  mutableRequest.publicInputs.messageId = HEX32_D;
  mutableRequest.publicSignalWords[0] = HEX32_D;
  mutableRequest.proofContext.statementHash = HEX32_D;
  assert.equal(mutableRequestResult.publicInputs.messageId, request.publicInputs.messageId);
  assert.equal(mutableRequestResult.publicSignalWords[0], request.publicSignalWords[0]);
  assert.equal(mutableRequestResult.proofContext.statementHash, request.proofContext.statementHash);
  assert.equal(submission.envelopeEncoding, SCCP_EVM_CONTRACT_CALL_ABI_TUPLE_V1);
  assert.equal(submission.platformPayload, "evm_groth16_contract_call");
  assert.equal(submission.submissionKind, "contract_call");
  assert.equal(submission.contractMethod, "submitSccpMessageProof(bytes,bytes32[6],bytes32)");
  assert.equal(submission.functionSelector, SCCP_SUBMIT_MESSAGE_PROOF_SELECTOR_V1);
  assert.equal(submission.callDataHex, submission.envelopeHex);
  assert.equal(submission.callData.length, 676);
  assert.equal(submission.publicInputWordsBytes.length, 6 * 32);
  assert.deepEqual(
    submission.arguments.map(({ key, encoding }) => [key, encoding]),
    [
      ["proof_bytes", "raw_bytes"],
      ["public_inputs", "abi_bytes32x6"],
      ["statement_hash", "abi_bytes32"],
    ],
  );
  assert.equal(submission.publicInputWords[0], sampleEvmPublicInputs.messageId);
  assert.equal(submission.publicInputWords[2], `0x${"00".repeat(31)}01`);
  assert.equal(submission.publicInputWords[4], `0x${"00".repeat(31)}13`);
  assert.deepEqual(Array.from(proofResult.bundleBytes), [5, 6, 7]);
  assert.deepEqual(Array.from(proofResult.sourceProofBytes), [9, 10]);
  assert.throws(
    () => buildEvmSccpSubmission({ proofResult, proof_result: proofResult }),
    /proofResult must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      buildEvmSccpSubmission({
        proofResult: { ...proofResult, request_hash: proofResult.requestHash },
      }),
    /proofResult\.requestHash must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      buildEvmSccpSubmission({
        proofResult: { ...proofResult, envelope_hash: proofResult.envelopeHash },
      }),
    /proofResult\.envelopeHash must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      buildEvmSccpSubmission({
        proofResult,
        bundleBytes: Uint8Array.from([5, 6, 7]),
        bundle_bytes: Uint8Array.from([5, 6, 7]),
      }),
    /bundleBytes must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      buildEvmSccpSubmission({
        proofResult: {
          ...proofResult,
          proofContext: {
            ...proofResult.proofContext,
            statementHash: proofResult.statementHash,
            statement_hash: proofResult.statementHash,
          },
        },
      }),
    /proofResult\.proofContext\.statementHash must not use multiple aliases/u,
  );
  const exposedProofBytes = proofResult.proofBytes;
  const exposedBundleBytes = proofResult.bundleBytes;
  const exposedSourceProofBytes = proofResult.sourceProofBytes;
  exposedProofBytes[0] = 255;
  exposedBundleBytes[0] = 255;
  exposedSourceProofBytes[0] = 255;
  assert.notEqual(proofResult.proofBytes[0], 255);
  assert.deepEqual(Array.from(proofResult.bundleBytes), [5, 6, 7]);
  assert.deepEqual(Array.from(proofResult.sourceProofBytes), [9, 10]);
  assert.equal(submission.callDataHex.startsWith(SCCP_SUBMIT_MESSAGE_PROOF_SELECTOR_V1), true);
  assert.equal(submission.callDataHex.slice(10, 74), `${"0".repeat(61)}100`);
  assert.equal(submission.callDataHex.slice(522, 586), `${"0".repeat(61)}180`);
  assert.equal(
    submission.callDataHex.slice(586),
    Array.from(GROTH16_PROOF_BYTES, (byte) => byte.toString(16).padStart(2, "0")).join(""),
  );
  const omittedSourceProofResult = wrapEvmSccpProofResult(
    GROTH16_PROOF_BYTES,
    buildEvmSccpProofRequest({
      publicInputs: sampleEvmPublicInputs,
      bundleBytes: [5, 6, 7],
      sourceDomain: SCCP_DOMAIN_SORA,
      statementHash: HEX32_G,
      destinationBinding: evmBinding,
    }),
  );
  const omittedSourceSubmission = buildEvmSccpSubmission({
    proofResult: omittedSourceProofResult,
  });
  assert.equal(omittedSourceProofResult.sourceProofBytes.length, 0);
  assert.deepEqual(Array.from(omittedSourceSubmission.proofBytes), Array.from(GROTH16_PROOF_BYTES));
  const exposedCallData = submission.callData;
  exposedCallData[0] = 0;
  assert.notEqual(submission.callData[0], 0);

  const tronBinding = sampleTronDestinationBinding();
  const tronRequest = buildTronSccpProofRequest({
    publicInputs: sampleTronPublicInputs,
    bundleBytes: [5, 6, 7],
    sourceProofBytes: [9, 10],
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBinding: tronBinding,
  });
  const tronProofResult = wrapTronSccpProofResult(GROTH16_PROOF_BYTES, tronRequest);
  const tronSubmission = buildTronSccpSubmission({
    proofResult: tronProofResult,
  });
  assert.equal(Object.isFrozen(tronProofResult), true);
  assert.equal(Object.isFrozen(tronProofResult.publicInputs), true);
  assert.equal(Object.isFrozen(tronProofResult.publicSignalWords), true);
  assert.equal(Object.isFrozen(tronProofResult.proofContext), true);
  assert.equal(tronSubmission.envelopeEncoding, SCCP_TRON_CONTRACT_CALL_ABI_TUPLE_V1);
  assert.equal(tronSubmission.platformPayload, "tron_contract_call");
  assert.equal(tronSubmission.targetDomain, SCCP_DOMAIN_TRON);
  assert.equal(tronSubmission.publicInputWords[2], `0x${"00".repeat(31)}05`);
  assert.deepEqual(Array.from(tronProofResult.bundleBytes), [5, 6, 7]);
  assert.deepEqual(Array.from(tronProofResult.sourceProofBytes), [9, 10]);
  assert.throws(
    () =>
      buildTronSccpSubmission({
        proofResult: {
          ...tronProofResult,
          request_hash: tronProofResult.requestHash,
        },
      }),
    /proofResult\.requestHash must not use multiple aliases/u,
  );
  assert.equal(tronSubmission.callDataHex.startsWith(SCCP_SUBMIT_MESSAGE_PROOF_SELECTOR_V1), true);
  assert.equal(tronSubmission.callData.length, 676);
  assert.deepEqual(
    tronSubmission.callData,
    sccpSubmitMessageProofCallData(
      GROTH16_PROOF_BYTES,
      sampleTronPublicInputs,
      tronProofResult.statementHash,
    ),
  );
  const omittedTronSourceProofResult = wrapTronSccpProofResult(
    GROTH16_PROOF_BYTES,
    buildTronSccpProofRequest({
      publicInputs: sampleTronPublicInputs,
      bundleBytes: [5, 6, 7],
      sourceDomain: SCCP_DOMAIN_SORA,
      statementHash: HEX32_G,
      destinationBinding: tronBinding,
    }),
  );
  const omittedTronSourceSubmission = buildTronSccpSubmission({
    proofResult: omittedTronSourceProofResult,
  });
  assert.equal(omittedTronSourceProofResult.sourceProofBytes.length, 0);
  assert.deepEqual(Array.from(omittedTronSourceSubmission.proofBytes), Array.from(GROTH16_PROOF_BYTES));

  const changedProof = Uint8Array.from(GROTH16_PROOF_BYTES);
  changedProof.set(
    abiWord(0x30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd45n),
    11 * 32,
  );
  assert.throws(
    () => buildEvmSccpSubmission({ proofResult, proofBytes: changedProof }),
    /proofBytes must match proofResult\.proofBytes/,
  );
  assert.throws(
    () => buildEvmSccpSubmission({ proofResult, publicInputs: null }),
    /publicInputs must be an object/,
  );
  assert.throws(
    () => buildEvmSccpSubmission({ proofResult, proofBytes: null }),
    /proofBytes must be bytes or hex/,
  );
  assert.throws(
    () => buildEvmSccpSubmission({ proofResult, statementHash: null }),
    /statementHash/,
  );
  assert.throws(
    () => buildEvmSccpSubmission({ proofResult, destinationBindingHash: null }),
    /destinationBindingHash/,
  );
  assert.throws(
    () => buildEvmSccpSubmission({ proofResult, publicSignalWords: null }),
    /publicSignalWords must contain 9 words/,
  );
  assert.throws(
    () => buildEvmSccpSubmission({ proofResult, bundleBytes: null }),
    /bundleBytes must be bytes or hex/,
  );
  assert.throws(
    () => buildEvmSccpSubmission({ proofResult: { ...proofResult, envelopeHash: HEX32_A } }),
    /proofResult\.envelopeHash must match wrapped proof bytes/,
  );
  assert.throws(
    () => buildEvmSccpSubmission({ proofResult: { ...proofResult, proofBase64: "AAAA" } }),
    /proofResult\.proofBase64 must match proofResult\.proofBytes/,
  );
  assert.throws(
    () =>
      buildEvmSccpSubmission({
        proofResult: { ...proofResult, bundleBytes: [5, 6, 8] },
      }),
    /proofResult\.requestHash must match bundleBytes and sourceProofBytes/,
  );
  assert.throws(
    () => buildTronSccpSubmission({ proofResult: { ...tronProofResult, envelopeHash: HEX32_A } }),
    /proofResult\.envelopeHash must match wrapped proof bytes/,
  );
  assert.throws(
    () =>
      buildTronSccpSubmission({
        proofResult: { ...tronProofResult, proofBase64: "AAAA" },
      }),
    /proofResult\.proofBase64 must match proofResult\.proofBytes/,
  );
  assert.throws(
    () =>
      buildTronSccpSubmission({
        proofResult: { ...tronProofResult, bundleBytes: [5, 6, 8] },
      }),
    /proofResult\.requestHash must match bundleBytes and sourceProofBytes/,
  );
  assert.throws(
    () => buildEvmSccpSubmission({ proofResult, publicSignalWords: Array(9).fill(HEX32_A) }),
    /publicSignalWords must match publicInputs and proof context/,
  );
  assert.throws(
    () => buildTronSccpSubmission({ proofResult }),
    /backend must match request/,
  );
  assert.throws(
    () =>
      buildEvmSccpSubmission({
        proofBytes: GROTH16_PROOF_BYTES,
        publicInputs: { ...sampleEvmPublicInputs, targetDomain: SCCP_DOMAIN_TON },
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /publicInputs\.targetDomain must be ETH or BSC/,
  );
});

test("binds Substrate runtime proof requests to relay context", () => {
  const request = buildSubstrateSccpProofRequest({
    publicInputs: sampleSubstratePublicInputs,
    bundleBytes: [5, 6, 7],
    sourceProofBytes: [9, 10],
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBindingHash: HEX32_H,
  });

  assert.equal(request.backend, SCCP_SUBSTRATE_RUNTIME_PROOF_BACKEND_V1);
  assert.equal(request.sourceDomain, SCCP_DOMAIN_SORA);
  assert.equal(request.targetDomain, SCCP_DOMAIN_SORA2);
  assert.equal(Object.isFrozen(request), true);
  assert.equal(Object.isFrozen(request.publicInputs), true);
  assert.equal(Object.isFrozen(request.proofContext), true);
  assert.deepEqual(request.proofContext, {
    version: 1,
    statementHash: HEX32_G,
    destinationBindingHash: HEX32_H,
  });
  assert.match(request.requestHash, /^0x[0-9a-f]{64}$/);

  const kusamaRequest = buildSubstrateSccpProofRequest({
    publicInputs: {
      ...sampleSubstratePublicInputs,
      targetDomain: SCCP_DOMAIN_SORA_KUSAMA,
    },
    bundleBytes: [5, 6, 7],
    sourceProofBytes: [9, 10],
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBindingHash: HEX32_H,
  });
  assert.equal(kusamaRequest.targetDomain, SCCP_DOMAIN_SORA_KUSAMA);
  assert.notEqual(kusamaRequest.requestHash, request.requestHash);

  const exposedPublicInputs = request.publicInputsBytes;
  const exposedBundle = request.bundleBytes;
  const exposedSourceProof = request.sourceProofBytes;
  exposedPublicInputs[0] = 99;
  exposedBundle[0] = 99;
  exposedSourceProof[0] = 99;
  assert.notEqual(request.publicInputsBytes[0], 99);
  assert.deepEqual(Array.from(request.bundleBytes), [5, 6, 7]);
  assert.deepEqual(Array.from(request.sourceProofBytes), [9, 10]);

  assert.notEqual(
    request.requestHash,
    buildSubstrateSccpProofRequest({
      publicInputs: sampleSubstratePublicInputs,
      bundleBytes: [5, 6, 7],
      sourceProofBytes: [9, 11],
      sourceDomain: SCCP_DOMAIN_SORA,
      statementHash: HEX32_G,
      destinationBindingHash: HEX32_H,
    }).requestHash,
  );
  assert.throws(
    () =>
      buildSubstrateSccpProofRequest({
        publicInputs: sampleSubstratePublicInputs,
        public_inputs: sampleSubstratePublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /publicInputs must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildSubstrateSccpProofRequest({
        publicInputs: sampleSubstratePublicInputs,
        bundleBytes: [5, 6, 7],
        bundle_bytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /bundleBytes must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildSubstrateSccpProofRequest({
        publicInputs: sampleSubstratePublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        source_proof_bytes: [9, 10],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /sourceProofBytes must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildSubstrateSccpProofRequest({
        publicInputs: sampleSubstratePublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        source_domain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /sourceDomain must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildSubstrateSccpProofRequest({
        publicInputs: sampleSubstratePublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        proofContext: { statementHash: HEX32_G, destinationBindingHash: HEX32_H },
        proof_context: { statementHash: HEX32_G, destinationBindingHash: HEX32_H },
      }),
    /proofContext must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildSubstrateSccpProofRequest({
        publicInputs: sampleSubstratePublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        statement_hash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /statementHash must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildSubstrateSccpProofRequest({
        publicInputs: sampleSubstratePublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBinding: { bindingHash: HEX32_H, binding_hash: HEX32_H },
      }),
    /destinationBinding\.bindingHash must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildSubstrateSccpProofRequest({
        publicInputs: sampleSubstratePublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        sourceDomain: SCCP_DOMAIN_TRON,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /sourceDomain must be SORA/,
  );
  assert.throws(
    () =>
      buildSubstrateSccpProofRequest({
        publicInputs: { ...sampleSubstratePublicInputs, targetDomain: SCCP_DOMAIN_TON },
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /publicInputs\.targetDomain must be a Substrate-family SCCP domain/,
  );
  assert.throws(
    () =>
      buildSubstrateSccpProofRequest({
        publicInputs: sampleSubstratePublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA2,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /sourceDomain must be SORA/,
  );
  assert.throws(
    () =>
      buildSubstrateSccpProofRequest({
        publicInputs: sampleSubstratePublicInputs,
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        backend: "debug-substrate-backend",
      }),
    /backend must be substrate-runtime-v1/,
  );
});

test("builds Substrate SCCP runtime-call submissions", () => {
  const request = buildSubstrateSccpProofRequest({
    publicInputs: sampleSubstratePublicInputs,
    bundleBytes: [5, 6, 7],
    sourceProofBytes: [9, 10],
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBindingHash: HEX32_H,
  });
  const proofResult = wrapSubstrateSccpProofResult([1, 2, 3, 4], request);
  const submission = buildSubstrateSccpSubmission({ proofResult });

  assert.equal(submission.proofFamily, SCCP_STARK_FRI_PROOF_FAMILY_V1);
  assert.equal(submission.verifierBackend, SCCP_SUBSTRATE_RUNTIME_PROOF_BACKEND_V1);
  assert.equal(submission.platformPayload, "substrate_runtime_call");
  assert.equal(submission.envelopeEncoding, SCCP_SUBSTRATE_RUNTIME_CALL_SCALE_V1);
  assert.equal(submission.submissionKind, "runtime_call");
  assert.equal(
    submission.verifierEntrypoint,
    SCCP_SUBSTRATE_SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1,
  );
  assert.equal(submission.sourceDomain, SCCP_DOMAIN_SORA);
  assert.equal(submission.targetDomain, SCCP_DOMAIN_SORA2);
  assert.equal(submission.requestHash, request.requestHash);
  assert.deepEqual(
    submission.arguments.map((argument) => argument.key),
    ["proof_bytes", "public_inputs", "bundle_bytes"],
  );
  assert.equal(submission.runtimeCallHex, submission.envelopeHex);
  assert.match(submission.runtimeCallHex, /^0x[0-9a-f]+$/);
  assert.equal(
    submission.runtimeCallHex.startsWith(
      `0x7c${Buffer.from(SCCP_SUBSTRATE_SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1).toString("hex")}10`,
    ),
    true,
  );
  assert.deepEqual(Array.from(submission.proofBytes), [1, 2, 3, 4]);
  assert.deepEqual(Array.from(submission.publicInputsBytes), Array.from(request.publicInputsBytes));
  assert.deepEqual(Array.from(submission.bundleBytes), [5, 6, 7]);

  const exposedRuntimeCall = submission.runtimeCall;
  exposedRuntimeCall[0] = 0;
  assert.notEqual(submission.runtimeCall[0], 0);

  assert.throws(
    () => buildSubstrateSccpSubmission({ proofResult, proof_result: proofResult }),
    /proofResult must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      buildSubstrateSccpSubmission({
        proofResult: { ...proofResult, request_hash: proofResult.requestHash },
      }),
    /proofResult\.requestHash must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      buildSubstrateSccpSubmission({
        proofResult: { ...proofResult, envelope_hash: proofResult.envelopeHash },
      }),
    /proofResult\.envelopeHash must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      buildSubstrateSccpSubmission({
        proofResult,
        bundleBytes: [5, 6, 7],
        bundle_bytes: [5, 6, 7],
      }),
    /bundleBytes must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      buildSubstrateSccpSubmission({
        proofResult: {
          ...proofResult,
          proofContext: {
            ...proofResult.proofContext,
            statementHash: proofResult.statementHash,
            statement_hash: proofResult.statementHash,
          },
        },
      }),
    /proofContext\.statementHash must not use multiple aliases/u,
  );
  assert.throws(
    () => buildSubstrateSccpSubmission({ proofResult, bundleBytes: [5, 6, 8] }),
    /bundleBytes must match proofResult\.bundleBytes/,
  );
  assert.throws(
    () =>
      buildSubstrateSccpSubmission({
        proofResult: { ...proofResult, envelopeHash: HEX32_A },
      }),
    /envelopeHash must match request/,
  );
  assert.throws(
    () =>
      buildSubstrateSccpSubmission({
        proofResult,
        publicInputsBytes: [1, 2, 3],
      }),
    /publicInputsBytes must match canonical SCCP transparent public inputs/,
  );
});

test("builds deterministic Solana SCCP proof requests", () => {
  const request = buildSolanaSccpProofRequest(sampleWitness());

  assert.equal(Object.isFrozen(request), true);
  assert.equal(Object.isFrozen(request.publicInputs), true);
  assert.equal(Object.isFrozen(request.witness), true);
  assert.equal(Object.isFrozen(request.proofContext), true);
  assert.equal(Object.isFrozen(request.sourceAdapterDeploymentBinding), true);
  assert.equal(request.version, 1);
  assert.equal(request.backend, SCCP_SOLANA_RECURSIVE_PROOF_BACKEND_V1);
  assert.equal(request.sourceDomain, SCCP_DOMAIN_SOL);
  assert.equal(request.publicInputs.messageId, HEX32_D);
  assert.equal(request.publicInputs.bankHash, HEX32_A);
  assert.equal(request.publicInputs.transactionStatusRoot, HEX32_B);
  assert.equal(request.publicInputs.messageProofHash, HEX32_C);
  assert.equal(request.publicInputs.parentSlot, "320");
  assert.equal(request.publicInputs.bankSignatureCount, "8");
  assert.equal(
    request.publicInputs.accountsLtHashProofPublicInputsHash,
    solanaSccpAccountsLtHashProofPublicInputsHash(request.witness),
  );
  assert.equal(request.publicInputs.statementHash, HEX32_G);
  assert.equal(request.publicInputs.destinationBindingHash, HEX32_H);
  assert.equal(request.sourceStateVerifierId, SCCP_SOLANA_MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1);
  assert.equal(request.sourceStateVerifierHash, SCCP_ZERO_HASH_V1);
  assert.equal(
    request.publicInputs.sourceStateVerifierId,
    SCCP_SOLANA_MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1,
  );
  assert.equal(request.publicInputs.sourceStateVerifierHash, SCCP_ZERO_HASH_V1);
  assert.equal(request.publicInputs.sourceAdapterDeploymentHash, SCCP_ZERO_HASH_V1);
  assert.equal(request.publicInputs.sourceAdapterDeploymentReceiptHash, SCCP_ZERO_HASH_V1);
  assert.equal(
    request.sourceAdapterDeploymentBindingHash,
    sccpSourceAdapterDeploymentBindingHash(request.sourceAdapterDeploymentBinding),
  );
  assert.deepEqual(request.proofContext, {
    version: 1,
    statementHash: HEX32_G,
    destinationBindingHash: HEX32_H,
  });
  assert.match(request.witnessHash, /^0x[0-9a-f]{64}$/);
  assert.match(request.proofContextHash, /^0x[0-9a-f]{64}$/);
  assert.match(request.sourceAdapterDeploymentBindingHash, /^0x[0-9a-f]{64}$/);
  assert.ok(canonicalSolanaSccpProofContextBytes(request.proofContext).length > 0);
  assert.ok(canonicalSolanaSccpAccountsLtHashProofPublicInputsBytes(request.witness).length > 250);
  assert.throws(
    () => buildSolanaSccpProofRequest(sampleWitness({ finalized_slot: 321n })),
    /finalizedSlot.*multiple aliases/,
  );
  assert.throws(
    () => buildSolanaSccpProofRequest(sampleWitness({ blockhashBytes: HEX32_A })),
    /blockhash.*multiple aliases/,
  );
  assert.throws(
    () => buildSolanaSccpProofRequest(sampleWitness({ message_id: HEX32_D })),
    /messageId.*multiple aliases/,
  );
  assert.throws(
    () =>
      buildSolanaSccpProofRequest(
        sampleWitness({
          proofContext: {
            statementHash: HEX32_G,
            statement_hash: HEX32_G,
            destinationBindingHash: HEX32_H,
          },
        }),
      ),
    /statementHash.*multiple aliases/,
  );
});

test("requires Solana SCCP proof context for local proof requests", () => {
  assert.throws(
    () => buildSolanaSccpProofRequest(sampleWitness({ statementHash: undefined })),
    /statementHash must be a hex string/,
  );
  assert.throws(
    () => buildSolanaSccpProofRequest(sampleWitness({ statementHash: SCCP_ZERO_HASH_V1 })),
    /statementHash must not be zero/,
  );
  assert.throws(
    () => buildSolanaSccpProofRequest(sampleWitness({ targetDomain: SCCP_DOMAIN_TON })),
    /targetDomain must be SORA/,
  );
  assert.throws(
    () =>
      normalizeSolanaSccpProofContext({
        statementHash: HEX32_G,
        destinationBindingHash: SCCP_ZERO_HASH_V1,
      }),
    /destinationBindingHash must not be zero/,
  );
  assert.throws(
    () => normalizeSolanaSccpProofContext({ statementHash: HEX32_G }),
    /destinationBindingHash must be a hex string/,
  );
});

test("binds source adapter deployment context for UI provers", () => {
  const request = buildSolanaSccpProofRequest(
    sampleWitness({
      sourceAdapterDeploymentHash: HEX32_A,
      sourceAdapterDeploymentReceiptHash: HEX32_B,
    }),
  );

  assert.equal(request.publicInputs.sourceAdapterDeploymentHash, HEX32_A);
  assert.equal(request.publicInputs.sourceAdapterDeploymentReceiptHash, HEX32_B);
  assert.equal(
    canonicalSccpSourceAdapterDeploymentBindingBytes(request.sourceAdapterDeploymentBinding).length,
    73,
  );
  assert.equal(
    request.sourceAdapterDeploymentBindingHash,
    sccpSourceAdapterDeploymentBindingHash(request.sourceAdapterDeploymentBinding),
  );
  assert.throws(
    () =>
      normalizeSccpSourceAdapterDeploymentBinding({
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: SCCP_ZERO_HASH_V1,
      }),
    /must both be zero or both be non-zero/,
  );
  assert.throws(
    () =>
      normalizeSccpSourceAdapterDeploymentBinding({
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: HEX32_A,
      }),
    /must differ from sourceAdapterDeploymentReceiptHash/,
  );
  assert.throws(
    () =>
      normalizeSccpSourceAdapterDeploymentBinding({
        sourceAdapterDeploymentHash: `${HEX32_A} `,
        sourceAdapterDeploymentReceiptHash: HEX32_B,
      }),
    /sourceAdapterDeploymentHash must be canonical hex/,
  );
});

test("derives canonical source adapter verifier VK hashes for UI tooling", () => {
  const vectors = new Map([
    [SCCP_DOMAIN_ETH, "0x2140903293411cad0f0eb217d8beb18d3a188edf7bba455098589a2409445e46"],
    [SCCP_DOMAIN_BSC, "0x12536f25748a6520f10ebd42a7bcccd6ec181b9d53129795c8e186dc6e8b18cc"],
    [SCCP_DOMAIN_SOL, "0xe7bc29d06bf56184183c3fc59a0e934cd1d8e16751f1eda2efaaf88aa350b9d6"],
    [SCCP_DOMAIN_TON, "0xf03f70e8cb504e69b0611df224c2783d04d8f4ee93beae7a62e1cd0a49703bad"],
    [SCCP_DOMAIN_TRON, "0x0e12ad03def9d75887d4d6437e63539cef97c54db4769881eeda757a88826364"],
    [SCCP_DOMAIN_SORA_KUSAMA, "0xf7768653132995511594e6e7edb4af22f78bba615650d9dda72f14bb18984daf"],
    [SCCP_DOMAIN_SORA_POLKADOT, "0x4f8456bf8626436a16d763c40bf23dffb962232f0766c4ae33d6e594f8be1635"],
    [SCCP_DOMAIN_SORA2, "0x96bbfa08489249b28a1444d0dcb9d5b4023bd688091f31c0b435601dad48dbb4"],
  ]);
  for (const [sourceDomain, expected] of vectors.entries()) {
    assert.equal(sccpSourceAdapterVerifierVkHash({ sourceDomain }), expected);
  }
  assert.throws(
    () => sccpSourceAdapterVerifierVkHash({ sourceDomain: SCCP_DOMAIN_TON, targetDomain: SCCP_DOMAIN_TON }),
    /targetDomain must be SORA/,
  );
});

test("derives native destination binding hashes for UI tooling", () => {
  const vectors = new Map([
    [
      SCCP_DOMAIN_SOL,
      [
        "sccp:0:3:sol:solana-program-v1:2",
        "0x078578f0aa27daa2972d6c19d1d26dbb6bf6ba1e8df84e283d7ef101fc46abf6",
      ],
    ],
    [
      SCCP_DOMAIN_TON,
      [
        "sccp:0:4:ton:ton-contract-v1:3",
        "0x8651c1b818973f92050f69e66e8491e9681d23db1cb37393b9ea15c5e7e02799",
      ],
    ],
    [
      SCCP_DOMAIN_SORA_KUSAMA,
      [
        "sccp:0:6:sora-kusama:substrate-runtime-v1:5",
        "0x2ee5c37634c3fab7e9086ea43af7553089fc24dc2ce27d76c46ef4c3da57bb56",
      ],
    ],
    [
      SCCP_DOMAIN_SORA_POLKADOT,
      [
        "sccp:0:7:sora-polkadot:substrate-runtime-v1:5",
        "0x570ec340d4fee4a84eaa7a53b19baa53c9f4f8d7f64c3c43639fde0c6b3fdef0",
      ],
    ],
    [
      SCCP_DOMAIN_SORA2,
      [
        "sccp:0:8:sora2:substrate-runtime-v1:5",
        "0xda5d48fe26518cd8cff6bdaa7cf8e37c7302d1e66469efed4ef2cf340c55b9e4",
      ],
    ],
  ]);
  for (const [targetDomain, [expectedKey, expectedHash]] of vectors.entries()) {
    assert.equal(sccpDestinationBindingKey({ targetDomain }), expectedKey);
    assert.equal(sccpDestinationBindingHash(targetDomain), expectedHash);
  }
  assert.throws(
    () => sccpDestinationBindingHash(SCCP_DOMAIN_ETH),
    /native SCCP destination lane/,
  );
});

test("derives EVM and TRON destination bindings for UI provers", () => {
  const evmInput = {
    targetDomain: SCCP_DOMAIN_ETH,
    networkId: `0x${"33".repeat(32)}`,
    verifierAddress: `0x${"11".repeat(20)}`,
    bridgeAddress: `0x${"22".repeat(20)}`,
    verifierCodeHash: `0x${"bb".repeat(32)}`,
    verifierKeyHash: `0x${"cc".repeat(32)}`,
  };
  const evmBinding = evmSccpDestinationBinding(evmInput);
  assert.equal(
    evmBinding.key,
    `evm:0:1:${"33".repeat(32)}:0x${"11".repeat(20)}:0x${"22".repeat(20)}:0x${"bb".repeat(32)}:0x${"cc".repeat(32)}`,
  );
  assert.equal(
    evmBinding.bindingHash,
    "0x3ad95ac3e5bc2892f768aae40a3b7ba673d561858b7d1318fbb9f6eba83207bf",
  );
  assert.equal(evmSccpDestinationBindingHash(evmInput), evmBinding.bindingHash);
  const evmRequest = buildEvmSccpProofRequest({
    publicInputs: sampleEvmPublicInputs,
    bundleBytes: [5, 6, 7],
    sourceProofBytes: [9, 10],
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBinding: evmBinding,
  });
  assert.equal(evmRequest.destinationBindingHash, evmBinding.bindingHash);
  const evmSubmissionForSubmit = buildEvmSccpSubmission({
    proofResult: wrapEvmSccpProofResult(GROTH16_PROOF_BYTES, evmRequest),
  });
  const evmMessageBundle = {
    commitment: { message_id: sampleEvmPublicInputs.messageId },
    commitment_root: sampleEvmPublicInputs.commitmentRoot,
  };
  const evmSubmitPayload = buildEvmSccpBridgeProofSubmitPayload({
    authority: "alice@sora",
    publicKeyHex: "ed0123",
    signatureB64: "sig",
    messageBundle: evmMessageBundle,
    submission: evmSubmissionForSubmit,
    destinationBinding: evmBinding,
    creationTimeMs: 123,
  });
  assert.equal(Object.isFrozen(evmSubmitPayload), true);
  assert.equal(evmSubmitPayload.authority, "alice@sora");
  assert.equal(evmSubmitPayload.public_key_hex, "ed0123");
  assert.equal(evmSubmitPayload.signature_b64, "sig");
  assert.equal(evmSubmitPayload.network_id_hex, evmBinding.networkId);
  assert.equal(evmSubmitPayload.verifier_address_hex, evmBinding.verifierAddress);
  assert.equal(evmSubmitPayload.bridge_address_hex, evmBinding.bridgeAddress);
  assert.equal(evmSubmitPayload.verifier_code_hash_hex, evmBinding.verifierCodeHash);
  assert.equal(evmSubmitPayload.verifier_key_hash_hex, evmBinding.verifierKeyHash);
  assert.equal(evmSubmitPayload.expected_destination_binding_hash_hex, evmBinding.bindingHash);
  assert.equal(evmSubmitPayload.creation_time_ms, 123);
  assert.equal(
    evmSubmitPayload.proof_bytes_hex,
    `0x${Array.from(GROTH16_PROOF_BYTES, (byte) => byte.toString(16).padStart(2, "0")).join("")}`,
  );
  assert.throws(
    () =>
      buildEvmSccpBridgeProofSubmitPayload({
        authority: "alice@sora",
        messageBundle: {
          ...evmMessageBundle,
          commitment: { message_id: HEX32_D },
        },
        submission: evmSubmissionForSubmit,
        destinationBinding: evmBinding,
      }),
    /proofBytes\.messageId must match messageBundle\.commitment\.messageId/,
  );
  assert.throws(
    () =>
      buildEvmSccpBridgeProofSubmitPayload({
        authority: "alice@sora",
        messageBundle: {
          ...evmMessageBundle,
          commitment_root: HEX32_D,
        },
        submission: evmSubmissionForSubmit,
        destinationBinding: evmBinding,
      }),
    /proofBytes\.commitmentRoot must match messageBundle\.commitmentRoot/,
  );
  assert.throws(
    () =>
      buildEvmSccpBridgeProofSubmitPayload({
        authority: "alice@sora",
        messageBundle: {},
        submission: { ...evmSubmissionForSubmit, destinationBindingHash: HEX32_A },
        destinationBinding: evmBinding,
      }),
    /submission destinationBindingHash must match destinationBinding/,
  );
  assert.throws(
    () =>
      buildEvmSccpBridgeProofSubmitPayload({
        authority: "alice@sora",
        messageBundle: {},
        submission: evmSubmissionForSubmit,
        sccp_submission: evmSubmissionForSubmit,
        destinationBinding: evmBinding,
      }),
    /submission must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildEvmSccpProofRequest({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBinding: evmBinding,
        destinationBindingHash: HEX32_A,
      }),
    /destinationBindingHash must match destinationBinding/,
  );
  assert.throws(
    () =>
      buildEvmSccpProofRequest({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBinding: evmBinding,
        destination_binding: evmBinding,
      }),
    /destinationBinding must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildEvmSccpProofRequest({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBinding: evmBinding,
        destinationBindingHash: evmBinding.bindingHash,
        destination_binding_hash: evmBinding.bindingHash,
      }),
    /destinationBindingHash must not use multiple aliases/,
  );
  assert.throws(
    () => evmSccpDestinationBinding({ ...evmInput, bindingHash: HEX32_A }),
    /destinationBinding\.bindingHash must match/,
  );
  assert.throws(
    () => evmSccpDestinationBinding({ ...evmInput, network_id: evmInput.networkId }),
    /destinationBinding\.networkId must not use multiple aliases/,
  );
  assert.throws(
    () => evmSccpDestinationBinding({ ...evmInput, verifier_address: evmInput.verifierAddress }),
    /destinationBinding\.verifierAddress must not use multiple aliases/,
  );
  assert.throws(
    () => evmSccpDestinationBinding({ ...evmInput, bindingHash: evmBinding.bindingHash, binding_hash: evmBinding.bindingHash }),
    /destinationBinding\.bindingHash must not use multiple aliases/,
  );
  assert.throws(
    () => evmSccpDestinationBinding({ ...evmInput, bridgeAddress: evmInput.verifierAddress }),
    /verifierAddress must differ from bridgeAddress/,
  );

  const tronInput = {
    networkId: `0x${"33".repeat(32)}`,
    verifierAddress: "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
    verifierCodeHash: `0x${"bb".repeat(32)}`,
    verifierKeyHash: `0x${"cc".repeat(32)}`,
  };
  const tronBinding = tronSccpDestinationBinding(tronInput);
  assert.equal(
    tronBinding.key,
    `tron:0:5:${"33".repeat(32)}:TJRabPrwbZy45sbavfcjinPJC18kjpRTv8:0x${"bb".repeat(32)}:0x${"cc".repeat(32)}`,
  );
  assert.equal(
    tronBinding.bindingHash,
    "0x17c953ad5b8c9a2b6f7102aca993fa7c427d018505cf4f58fac35ea454caba7f",
  );
  assert.equal(tronSccpDestinationBindingHash(tronInput), tronBinding.bindingHash);
  const tronRequest = buildTronSccpProofRequest({
    publicInputs: sampleTronPublicInputs,
    bundleBytes: [5, 6, 7],
    sourceProofBytes: [9, 10],
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBinding: tronBinding,
  });
  assert.equal(tronRequest.destinationBindingHash, tronBinding.bindingHash);
  const tronSubmissionForSubmit = buildTronSccpSubmission({
    proofResult: wrapTronSccpProofResult(GROTH16_PROOF_BYTES, tronRequest),
  });
  const tronMessageBundle = {
    commitment: { message_id: sampleTronPublicInputs.messageId },
    commitment_root: sampleTronPublicInputs.commitmentRoot,
  };
  const tronSubmitPayload = buildTronSccpBridgeProofSubmitPayload({
    authority: "alice@sora",
    messageBundle: tronMessageBundle,
    tronSccpSubmission: tronSubmissionForSubmit,
    destinationBinding: tronBinding,
  });
  assert.equal(Object.isFrozen(tronSubmitPayload), true);
  assert.equal(tronSubmitPayload.network_id_hex, tronBinding.networkId);
  assert.equal(tronSubmitPayload.tron_verifier_address, tronBinding.verifierAddress);
  assert.equal(tronSubmitPayload.verifier_address_hex, undefined);
  assert.equal(tronSubmitPayload.bridge_address_hex, undefined);
  assert.equal(tronSubmitPayload.verifier_code_hash_hex, tronBinding.verifierCodeHash);
  assert.equal(tronSubmitPayload.verifier_key_hash_hex, tronBinding.verifierKeyHash);
  assert.equal(tronSubmitPayload.expected_destination_binding_hash_hex, tronBinding.bindingHash);
  assert.equal(
    tronSubmitPayload.proof_bytes_hex,
    `0x${Array.from(GROTH16_PROOF_BYTES, (byte) => byte.toString(16).padStart(2, "0")).join("")}`,
  );
  assert.throws(
    () =>
      buildTronSccpBridgeProofSubmitPayload({
        authority: "alice@sora",
        messageBundle: {
          ...tronMessageBundle,
          commitment_root: HEX32_D,
        },
        submission: tronSubmissionForSubmit,
        destinationBinding: tronBinding,
      }),
    /proofBytes\.commitmentRoot must match messageBundle\.commitmentRoot/,
  );
  assert.throws(
    () =>
      buildTronSccpBridgeProofSubmitPayload({
        authority: "alice@sora",
        messageBundle: {},
        submission: { ...tronSubmissionForSubmit, targetDomain: SCCP_DOMAIN_ETH },
        destinationBinding: tronBinding,
      }),
    /submission targetDomain must match destinationBinding/,
  );
  assert.throws(
    () =>
      buildTronSccpProofRequest({
        publicInputs: sampleTronPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBinding: tronBinding,
        destinationBindingHash: HEX32_A,
      }),
    /destinationBindingHash must match destinationBinding/,
  );
  assert.throws(
    () =>
      buildTronSccpProofRequest({
        publicInputs: sampleTronPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBinding: tronBinding,
        destination_binding: tronBinding,
      }),
    /destinationBinding must not use multiple aliases/,
  );
  assert.throws(
    () => tronSccpDestinationBinding({ ...tronInput, bindingHash: HEX32_A }),
    /destinationBinding\.bindingHash must match/,
  );
  assert.throws(
    () => tronSccpDestinationBinding({ ...tronInput, network_id: tronInput.networkId }),
    /destinationBinding\.networkId must not use multiple aliases/,
  );
  assert.throws(
    () => tronSccpDestinationBinding({ ...tronInput, verifier_address: tronInput.verifierAddress }),
    /destinationBinding\.verifierAddress must not use multiple aliases/,
  );
  assert.throws(
    () => tronSccpDestinationBinding({ ...tronInput, backend: SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1, verifierBackend: SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1 }),
    /destinationBinding\.verifierBackend must not use multiple aliases/,
  );
  assert.throws(
    () =>
      tronSccpDestinationBinding({
        ...tronInput,
        verifierAddress: "TJRabPrwbZy45sbavfcjinPJC18kjpRTv9",
      }),
    /base58check checksum/,
  );
  assert.throws(
    () =>
      tronSccpDestinationBinding({
        ...tronInput,
        verifierAddress: " TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
      }),
    /canonical base58check/,
  );
});

const sampleSourceRecordInput = (sourceDomain) => {
  const input = {
    sourceDomain,
    sourceTrustAnchorHash: `0x${"44".repeat(32)}`,
    consensusVerifierHash: `0x${"55".repeat(32)}`,
    messageInclusionVerifierHash: `0x${"66".repeat(32)}`,
    finalityPolicyHash: `0x${"88".repeat(32)}`,
    deploymentReceiptHash: `0x${"aa".repeat(32)}`,
  };
  if ([SCCP_DOMAIN_ETH, SCCP_DOMAIN_BSC, SCCP_DOMAIN_TRON].includes(sourceDomain)) {
    input.bridgeAddress = `0x${"11".repeat(20)}`;
    input.sourceBridgeEmitterCodeHash = `0x${"77".repeat(32)}`;
  }
  if (
    [
      SCCP_DOMAIN_SOL,
      SCCP_DOMAIN_TON,
      SCCP_DOMAIN_SORA_KUSAMA,
      SCCP_DOMAIN_SORA_POLKADOT,
      SCCP_DOMAIN_SORA2,
    ].includes(sourceDomain)
  ) {
    input.sourceStateVerifierHash = `0x${"77".repeat(32)}`;
  }
  if (sourceDomain === SCCP_DOMAIN_TRON) {
    input.networkId = `0x${"33".repeat(32)}`;
    input.ownerAddress = `0x${"22".repeat(20)}`;
    input.configHash = "0xe986dd67bfa2307b4e00cf46bde41a88003a55c5b7fea311fa106614b2252f9d";
  }
  return input;
};

test("derives SCCP source material and deployment record hashes for UI tooling", () => {
  const materialVectors = new Map([
    [SCCP_DOMAIN_ETH, "0x035c5a35f6412d45ed10389741016d067bd6d0b874a38cd744922c599e0a2fdd"],
    [SCCP_DOMAIN_BSC, "0x1630e4d75e2676cc443e07b0477303240ae4cff13bdf9fe61725b4a9a4ee959a"],
    [SCCP_DOMAIN_SOL, "0x499a7363142d5fcfe3a79b11a29ae2ad897e853649e80e39a162b8942f908331"],
    [SCCP_DOMAIN_TON, "0x08b11177113ac2d9f612abdf767a017de560d805e965b3dc32e28c8748ea2ebc"],
    [SCCP_DOMAIN_TRON, "0x68c20262e44676bd5f3c4ec428f063373147a1ca14c5885648a9c651b3bcd8d8"],
    [SCCP_DOMAIN_SORA_KUSAMA, "0x012c66498a85190d6075c441fad30fe01816796ee1713838fe8bb97f2ad1c924"],
    [SCCP_DOMAIN_SORA_POLKADOT, "0x40cd55d64e92d688b839242e170f1722485cddf2e42b4ff22e53c5e7723e570d"],
    [SCCP_DOMAIN_SORA2, "0x6fc968441106993502dd05ebeadea1dbfee0f7814680f1ad006d4584c99a8a2d"],
  ]);
  const deploymentVectors = new Map([
    [SCCP_DOMAIN_ETH, "0xd08e3344760aabfb4ba891990c852846d04a5735647174ce6e3ab0f2cad57f4d"],
    [SCCP_DOMAIN_BSC, "0x7d47ade779a5bddb3a5f283600af677db8605b75a00516a4328f3823ff28fb2d"],
    [SCCP_DOMAIN_SOL, "0xcdb2a81cb31e58d9bc1f4292d33c3f4990b2d2008dda1b9b1275aaac087461cc"],
    [SCCP_DOMAIN_TON, "0x5c4e226c1f4619311762a9c889f8e3b99ea6f020317c2e8a0c76a08d7a70f887"],
    [SCCP_DOMAIN_TRON, "0x94dbe28a2fb16e043b83639b6dea8ec62f53679599ef1dd220fd13c71c7bdcb8"],
    [SCCP_DOMAIN_SORA_KUSAMA, "0xda47a31715813ef5bff0882cd0e0e8b0cc89d426e005e37e0f94a2bdba2043cd"],
    [SCCP_DOMAIN_SORA_POLKADOT, "0x2a57fe4beb69e8201299f2c01259a025cafc8388bb38e2a727c2fc872893e13a"],
    [SCCP_DOMAIN_SORA2, "0xdac819bff0aa57f7596f06297dfec39027aaab63213497020b772c355a6eaecb"],
  ]);
  for (const [sourceDomain, materialHash] of materialVectors.entries()) {
    const input = sampleSourceRecordInput(sourceDomain);
    const material = normalizeSccpSourceVerifierMaterial(input);
    const deployment = normalizeSccpSourceAdapterEngineDeployment(input);
    assert.equal(material.placeholderMaterial, false);
    assert.equal(canonicalSccpSourceVerifierMaterialBytes(material).length > 0, true);
    assert.equal(canonicalSccpSourceAdapterEngineDeploymentBytes(deployment).length > 0, true);
    assert.equal(sccpSourceVerifierMaterialHash(input), materialHash);
    assert.equal(sccpSourceAdapterEngineDeploymentHash(input), deploymentVectors.get(sourceDomain));
  }
  assert.throws(
    () =>
      normalizeSccpSourceVerifierMaterial({
        ...sampleSourceRecordInput(SCCP_DOMAIN_SOL),
        source_domain: SCCP_DOMAIN_TON,
      }),
    /sourceDomain must not use multiple aliases/,
  );
  assert.throws(
    () =>
      normalizeSccpSourceVerifierMaterial({
        ...sampleSourceRecordInput(SCCP_DOMAIN_SOL),
        source_state_verifier_hash: `0x${"99".repeat(32)}`,
      }),
    /sourceStateVerifierHash must not use multiple aliases/,
  );
  assert.throws(
    () =>
      normalizeSccpSourceVerifierMaterial({
        ...sampleSourceRecordInput(SCCP_DOMAIN_TRON),
        sourceBridgeNetworkId: `0x${"99".repeat(32)}`,
      }),
    /sourceBridgeNetworkId must not use multiple aliases/,
  );
  assert.throws(
    () =>
      normalizeSccpSourceVerifierMaterial({
        ...sampleSourceRecordInput(SCCP_DOMAIN_ETH),
        sourceStateVerifierHash: `0x${"77".repeat(32)}`,
      }),
    /sourceStateVerifierHash is not used for sourceDomain/,
  );
  assert.throws(
    () =>
      normalizeSccpSourceVerifierMaterial({
        ...sampleSourceRecordInput(SCCP_DOMAIN_SOL),
        bridgeAddress: `0x${"11".repeat(20)}`,
      }),
    /sourceBridgeEmitterAddress is not used for sourceDomain/,
  );
  assert.throws(
    () =>
      normalizeSccpSourceVerifierMaterial({
        ...sampleSourceRecordInput(SCCP_DOMAIN_ETH),
        networkId: `0x${"33".repeat(32)}`,
      }),
    /sourceBridgeNetworkId is not used for sourceDomain/,
  );
  assert.throws(
    () =>
      normalizeSccpSourceAdapterEngineDeployment({
        ...sampleSourceRecordInput(SCCP_DOMAIN_ETH),
        targetDomain: SCCP_DOMAIN_SORA,
        target_domain: SCCP_DOMAIN_SORA,
      }),
    /targetDomain must not use multiple aliases/,
  );
  assert.throws(
    () =>
      normalizeSccpSourceAdapterEngineDeployment({
        ...sampleSourceRecordInput(SCCP_DOMAIN_SOL),
        solanaTowerReplayVerifierHash: `0x${"bb".repeat(32)}`,
        solana_tower_replay_verifier_hash: `0x${"bc".repeat(32)}`,
      }),
    /solanaTowerReplayVerifierHash must not use multiple aliases/,
  );
  assert.throws(
    () =>
      normalizeSccpSourceAdapterEngineDeployment({
        ...sampleSourceRecordInput(SCCP_DOMAIN_TON),
        tonMasterchainConfigVerifierHash: `0x${"bb".repeat(32)}`,
        ton_masterchain_config_verifier_hash: `0x${"bc".repeat(32)}`,
      }),
    /tonMasterchainConfigVerifierHash must not use multiple aliases/,
  );
  assert.throws(
    () =>
      normalizeSccpSourceAdapterEngineDeployment({
        ...sampleSourceRecordInput(SCCP_DOMAIN_SOL),
        solanaTowerReplayVerifierHash: null,
      }),
    /solanaTowerReplayVerifierHash/,
  );
  assert.throws(
    () =>
      normalizeSccpSourceAdapterEngineDeployment({
        ...sampleSourceRecordInput(SCCP_DOMAIN_TON),
        tonMasterchainConfigVerifierHash: null,
      }),
    /tonMasterchainConfigVerifierHash/,
  );
  assert.throws(
    () =>
      normalizeSccpSourceAdapterEngineDeployment({
        ...sampleSourceRecordInput(SCCP_DOMAIN_ETH),
        deployment_receipt_hash: `0x${"99".repeat(32)}`,
      }),
    /deploymentReceiptHash must not use multiple aliases/,
  );
  for (const [field, templateHash] of TON_TEMPLATE_SOURCE_MATERIAL_COMPONENT_HASHES.entries()) {
    assert.throws(
      () =>
        normalizeSccpSourceVerifierMaterial({
          ...sampleSourceRecordInput(SCCP_DOMAIN_TON),
          [field]: templateHash,
        }),
      /TON template (verifier|component) hash/,
    );
  }
  for (const [field, templateHash] of TRON_TEMPLATE_SOURCE_MATERIAL_COMPONENT_HASHES.entries()) {
    assert.throws(
      () =>
        normalizeSccpSourceVerifierMaterial({
          ...sampleSourceRecordInput(SCCP_DOMAIN_TRON),
          [field]: templateHash,
        }),
      /TRON template component hash/,
    );
  }
  for (const [field, templateHash] of SOLANA_TEMPLATE_SOURCE_MATERIAL_COMPONENT_HASHES.entries()) {
    assert.throws(
      () =>
        normalizeSccpSourceVerifierMaterial({
          ...sampleSourceRecordInput(SCCP_DOMAIN_SOL),
          [field]: templateHash,
        }),
      /Solana template (verifier|component) hash/,
    );
  }
  assert.throws(
    () =>
      normalizeSccpSourceVerifierMaterial({
        ...sampleSourceRecordInput(SCCP_DOMAIN_TRON),
        configHash: `0x${"99".repeat(32)}`,
      }),
    /TRON source bridge config fields/,
  );
  assert.throws(
    () =>
      normalizeSccpSourceVerifierMaterial({
        ...sampleSourceRecordInput(SCCP_DOMAIN_ETH),
        consensusVerifierHash: `0x${"44".repeat(32)}`,
      }),
    /role-separated/,
  );
  assert.throws(
    () =>
      normalizeSccpSourceAdapterEngineDeployment({
        ...sampleSourceRecordInput(SCCP_DOMAIN_ETH),
        deploymentReceiptHash: sccpSourceAdapterVerifierVkHash({
          sourceDomain: SCCP_DOMAIN_ETH,
          targetDomain: SCCP_DOMAIN_SORA,
        }),
      }),
    /role-separated/,
  );
  assert.throws(
    () =>
      normalizeSccpSourceAdapterEngineDeployment({
        ...sampleSourceRecordInput(SCCP_DOMAIN_ETH),
        adapterProofFamily: null,
      }),
    /adapterProofFamily/,
  );
  assert.throws(
    () =>
      normalizeSccpSourceAdapterEngineDeployment({
        ...sampleSourceRecordInput(SCCP_DOMAIN_ETH),
        targetDomain: null,
      }),
    /targetDomain must be a u32 domain id/,
  );

  const auditedSolanaDeployment = {
    ...sampleSourceRecordInput(SCCP_DOMAIN_SOL),
    solanaTowerReplayVerifierHash: `0x${"bb".repeat(32)}`,
    solanaFullAccountsdbLatticeVerifierHash: `0x${"cc".repeat(32)}`,
    solanaBankForkChoiceVerifierHash: `0x${"dd".repeat(32)}`,
  };
  assert.equal(
    sccpSourceAdapterEngineDeploymentHash(auditedSolanaDeployment),
    "0x97e5c4196aff6387b9d973e663de3ce9345e1d8c3de89d22505b2197e282dc61",
  );
  assert.equal(
    sccpSolanaFullLightClientGateHash(auditedSolanaDeployment),
    "0x2c94b86a665bb68708b762c678661f5e9879bd588627e93a640796eeaef970f9",
  );
  assert.throws(
    () => sccpSolanaFullLightClientGateHash(sampleSourceRecordInput(SCCP_DOMAIN_SOL)),
    /audited Solana -> SORA deployment/,
  );
  assert.throws(
    () =>
      sccpSourceAdapterEngineDeploymentHash({
        ...sampleSourceRecordInput(SCCP_DOMAIN_SOL),
        solanaTowerReplayVerifierHash: `0x${"bb".repeat(32)}`,
      }),
    /Solana audit verifier hashes/,
  );
  assert.throws(
    () =>
      sccpSourceAdapterEngineDeploymentHash({
        ...sampleSourceRecordInput(SCCP_DOMAIN_SOL),
        solanaTowerReplayVerifierHash: `0x${"bb".repeat(32)}`,
        solanaFullAccountsdbLatticeVerifierHash: `0x${"bb".repeat(32)}`,
        solanaBankForkChoiceVerifierHash: `0x${"dd".repeat(32)}`,
      }),
    /role-separated/,
  );
  assert.throws(
    () =>
      sccpSolanaFullLightClientGateHash({
        ...sampleSourceRecordInput(SCCP_DOMAIN_SOL),
        solanaTowerReplayVerifierHash: `0x${"77".repeat(32)}`,
        solanaFullAccountsdbLatticeVerifierHash: `0x${"cc".repeat(32)}`,
        solanaBankForkChoiceVerifierHash: `0x${"dd".repeat(32)}`,
      }),
    /sourceStateVerifierHash/,
  );
  for (const roleField of ["adapterVerifierVkHash", "deploymentReceiptHash"]) {
    const sourceRecord = sampleSourceRecordInput(SCCP_DOMAIN_SOL);
    const reusedHash =
      roleField === "adapterVerifierVkHash"
        ? sccpSourceAdapterVerifierVkHash({
            sourceDomain: SCCP_DOMAIN_SOL,
            targetDomain: SCCP_DOMAIN_SORA,
          })
        : sourceRecord[roleField];
    assert.throws(
      () =>
        sccpSolanaFullLightClientGateHash({
          ...sourceRecord,
          solanaTowerReplayVerifierHash: reusedHash,
          solanaFullAccountsdbLatticeVerifierHash: `0x${"cc".repeat(32)}`,
          solanaBankForkChoiceVerifierHash: `0x${"dd".repeat(32)}`,
        }),
      new RegExp(roleField),
    );
  }
  assert.throws(
    () =>
      sccpSolanaFullLightClientGateHash({
        ...sampleSourceRecordInput(SCCP_DOMAIN_SOL),
        solanaTowerReplayVerifierHash:
          SOLANA_TEMPLATE_SOURCE_MATERIAL_COMPONENT_HASHES.get("sourceTrustAnchorHash"),
        solanaFullAccountsdbLatticeVerifierHash: `0x${"cc".repeat(32)}`,
        solanaBankForkChoiceVerifierHash: `0x${"dd".repeat(32)}`,
      }),
    /template material/,
  );
  assert.throws(
    () =>
      sccpSourceAdapterEngineDeploymentHash({
        ...sampleSourceRecordInput(SCCP_DOMAIN_TON),
        solanaTowerReplayVerifierHash: `0x${"bb".repeat(32)}`,
        solanaFullAccountsdbLatticeVerifierHash: `0x${"cc".repeat(32)}`,
        solanaBankForkChoiceVerifierHash: `0x${"dd".repeat(32)}`,
      }),
    /only used for Solana deployments/,
  );

  const auditedTonDeployment = {
    ...sampleSourceRecordInput(SCCP_DOMAIN_TON),
    tonMasterchainConfigVerifierHash: `0x${"bb".repeat(32)}`,
    tonValidatorSetTransitionVerifierHash: `0x${"cc".repeat(32)}`,
    tonShardAccountsDictionaryVerifierHash: `0x${"dd".repeat(32)}`,
  };
  assert.equal(
    sccpSourceAdapterEngineDeploymentHash(auditedTonDeployment),
    "0x61e5d710ccbc902be00a38a5a80d05c19de97105605a3f93d4f8067862d81f07",
  );
  assert.equal(
    sccpTonFullLightClientGateHash(auditedTonDeployment),
    "0xc32d8cfc2e273646abb00911b9a15e7ee0ab1721b04a6e89a060422dd3cc4596",
  );
  assert.throws(
    () => sccpTonFullLightClientGateHash(sampleSourceRecordInput(SCCP_DOMAIN_TON)),
    /audited TON -> SORA deployment/,
  );
  assert.throws(
    () =>
      sccpSourceAdapterEngineDeploymentHash({
        ...sampleSourceRecordInput(SCCP_DOMAIN_TON),
        tonMasterchainConfigVerifierHash: `0x${"bb".repeat(32)}`,
      }),
    /TON audit verifier hashes/,
  );
  assert.throws(
    () =>
      sccpSourceAdapterEngineDeploymentHash({
        ...sampleSourceRecordInput(SCCP_DOMAIN_SOL),
        tonMasterchainConfigVerifierHash: `0x${"bb".repeat(32)}`,
        tonValidatorSetTransitionVerifierHash: `0x${"cc".repeat(32)}`,
        tonShardAccountsDictionaryVerifierHash: `0x${"dd".repeat(32)}`,
      }),
    /only used for TON deployments/,
  );

  assert.throws(
    () =>
      sccpSourceAdapterEngineDeploymentHash({
        ...sampleSourceRecordInput(SCCP_DOMAIN_ETH),
        adapterVerifierVkHash: `0x${"99".repeat(32)}`,
      }),
    /canonical source-adapter verifier profile/,
  );
});

const sampleTonFullLightClientAuditProofInput = (overrides = {}) => {
  const configLeaf = {
    sourceDomain: SCCP_DOMAIN_TON,
    masterchainSeqno: 19n,
    masterchainBlockHash: HEX32_A,
    shardStateRoot: TON_SHARD_STATE_ROOT_HASH,
    validatorSetHash: TON_VALIDATOR_SET_HASH,
    validatorSetPayloadHash: TON_VALIDATOR_SET_PAYLOAD_HASH,
  };
  const configLeafHash = tonMasterchainConfigLeafHash(configLeaf);
  const configProof = {
    ...configLeaf,
    configRoot: TON_MASTERCHAIN_CONFIG_ROOT,
    configLeafHash,
    configLeafIndex: SCCP_TON_CURRENT_VALIDATOR_SET_CONFIG_PARAM,
    configValueHash: TON_MASTERCHAIN_CONFIG_VALUE_HASH,
    configDictionaryProofBoc: Buffer.from(TON_MASTERCHAIN_CONFIG_PROOF_BOC_HEX, "hex"),
    configInclusionBranch: [],
  };
  return {
    ...sampleSourceRecordInput(SCCP_DOMAIN_TON),
    sourceStateVerifierHash: `0x${"d4".repeat(32)}`,
    sourceTrustAnchorHash: TON_VALIDATOR_SET_HASH,
    consensusVerifierHash: `0x${"b2".repeat(32)}`,
    messageInclusionVerifierHash: `0x${"c3".repeat(32)}`,
    finalityPolicyHash: `0x${"c4".repeat(32)}`,
    tonMasterchainConfigVerifierHash: `0x${"b1".repeat(32)}`,
    tonValidatorSetTransitionVerifierHash: `0x${"c2".repeat(32)}`,
    tonShardAccountsDictionaryVerifierHash: `0x${"d3".repeat(32)}`,
    masterchainSeqno: 19n,
    masterchainWorkchainId: -1,
    masterchainShard: 0x8000000000000000n,
    masterchainBlockHash: HEX32_A,
    masterchainFileHash: `0x${"a5".repeat(32)}`,
    validatorSetHash: TON_VALIDATOR_SET_HASH,
    masterchainConfigRoot: TON_MASTERCHAIN_CONFIG_ROOT,
    masterchainConfigProofHash: tonMasterchainConfigProofHash(configProof),
    validatorSetPayloadHash: TON_VALIDATOR_SET_PAYLOAD_HASH,
    configLeafHash,
    configValueHash: TON_MASTERCHAIN_CONFIG_VALUE_HASH,
    shardWorkchainId: 0,
    shardShard: 0x8000000000000000n,
    shardSeqno: 7n,
    shardBlockHash: HEX32_B,
    shardFileHash: `0x${"bc".repeat(32)}`,
    shardStateRoot: TON_SHARD_STATE_ROOT_HASH,
    transactionRoot: TON_HASHMAP_E_VALUE_HASH,
    transactionLt: 7n,
    shardStateProofBoc: Buffer.from(TON_SHARD_STATE_PROOF_BOC_HEX, "hex"),
    shardStateDictionaryRoot: TON_SHARD_ACCOUNTS_ROOT_HASH,
    shardStateDictionaryKeyBitLen: 256,
    shardStateDictionaryKey: TON_SHARD_ACCOUNT_KEY,
    shardStateDictionaryProofBoc: Buffer.from(TON_SHARD_ACCOUNTS_BOC_HEX, "hex"),
    masterchainSignatureHash: TON_MASTERCHAIN_SIGNATURES_HASH,
    shardProofHash: "0x32d8b496320e6a1ce5ccf671f2bd6f0d09cb53afed8c123b86cb9327b77c88cf",
    configDictionaryProofBoc: Buffer.from(TON_MASTERCHAIN_CONFIG_PROOF_BOC_HEX, "hex"),
    validatorSetTransitionProofs: [],
    shardStateVerificationProof: {
      version: 1,
      proofFamily: "stark-fri-v1",
      circuitId: SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1,
      proofBytes: Uint8Array.from([0x11, 0x22, 0x33, 0x44]),
    },
    ...overrides,
  };
};

test("builds TON full light-client audit role proof requests", () => {
  const input = sampleTonFullLightClientAuditProofInput();
  const requests = buildTonSccpFullLightClientAuditProofRequests(input);
  const shardStateProofPublicInputsHash = tonShardStateProofPublicInputsHash(input);
  const shardStateVerificationProofHash = tonSccpShardStateVerificationProofHash(
    input.shardStateVerificationProof,
  );
  assert.ok(
    canonicalTonSccpSourceStateVerificationProofBytes(input.shardStateVerificationProof).length > 0,
  );

  assert.deepEqual(
    Object.keys(requests),
    ["masterchainConfig", "validatorSetTransition", "shardAccountsDictionary"],
  );
  assert.deepEqual(
    Object.values(requests).map((request) => request.role),
    ["masterchain_config", "validator_set_transition", "shard_accounts_dictionary"],
  );
  assert.equal(Object.isFrozen(requests), true);
  assert.equal(
    requests.masterchainConfig.circuitId,
    SCCP_TON_MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1,
  );
  assert.equal(
    requests.validatorSetTransition.circuitId,
    SCCP_TON_VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1,
  );
  assert.equal(
    requests.shardAccountsDictionary.circuitId,
    SCCP_TON_SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1,
  );
  assert.equal(
    new Set(Object.values(requests).map((request) => request.circuitId)).size,
    3,
  );
  assert.ok(canonicalTonSccpFullLightClientAuditStatementBytes(input, "masterchainConfig").length > 0);
  for (const request of Object.values(requests)) {
    assertImmutableFastpqProofRequest(request, [
      "statementBytes",
      "verificationContextBytes",
      "schemaDescriptor",
    ]);
    assert.equal(request.version, 1);
    assert.equal(request.proofFamily, "stark-fri-v1");
    assert.equal(request.parameterSet, "fastpq-lane-balanced");
    assert.equal(request.sourceDomain, SCCP_DOMAIN_TON);
    assert.equal(request.masterchainSeqno, "19");
    assert.equal(request.shardSeqno, "7");
    assert.equal(request.sourceStateVerifierId, SCCP_TON_MAINNET_SHARD_STATE_VERIFIER_ID_V1);
    assert.equal(request.fullLightClientGateHash, sccpTonFullLightClientGateHash(input));
    assert.equal(request.shardStateProofPublicInputsHash, shardStateProofPublicInputsHash);
    assert.equal(request.shardStateVerificationProofHash, shardStateVerificationProofHash);
    assert.equal(
      request.auditStatementHash,
      tonSccpFullLightClientAuditStatementHash(input, request.role),
    );
    assert.deepEqual(
      request.schemaDescriptor,
      tonSccpFullLightClientAuditOpenVerifySchemaDescriptor(input, request.role),
    );
    assert.deepEqual(
      request.publicInputColumns,
      tonSccpFullLightClientAuditPublicInputColumns(input, request.role),
    );
    assert.equal(request.publicInputColumns.length, request.role === "validator_set_transition" ? 16 : 17);
    assert.equal(request.fastpqTransitions.length, 3);
    assert.deepEqual(
      request.fastpqTransitions,
      [...request.fastpqTransitions].sort((left, right) => left.key.localeCompare(right.key)),
    );
    assert.ok(request.fastpqTransitions.every((transition) => transition.key.startsWith("0x")));
  }
  assert.equal(requests.masterchainConfig.fastpqPublicInputs.oldRoot, TON_MASTERCHAIN_CONFIG_ROOT);
  assert.equal(requests.validatorSetTransition.fastpqPublicInputs.oldRoot, TON_VALIDATOR_SET_HASH);
  assert.equal(requests.shardAccountsDictionary.fastpqPublicInputs.newRoot, TON_HASHMAP_E_VALUE_HASH);
  assert.throws(
    () =>
      buildTonSccpFullLightClientAuditProofRequests(
        sampleTonFullLightClientAuditProofInput({
          tonValidatorSetTransitionVerifierHash: `0x${"b1".repeat(32)}`,
        }),
      ),
    /role-separated/,
  );
  assert.throws(
    () =>
      buildTonSccpFullLightClientAuditProofRequests(
        sampleTonFullLightClientAuditProofInput({
          tonMasterchainConfigVerifierHash: `0x${"d4".repeat(32)}`,
        }),
      ),
    /must not reuse/,
  );
  const requestHashReplayInput = sampleTonFullLightClientAuditProofInput();
  const requestHashReplay = tonSccpFullLightClientAuditStatementHash(
    requestHashReplayInput,
    "masterchainConfig",
  );
  assert.throws(
    () =>
      buildTonSccpFullLightClientAuditProofRequests(
        sampleTonFullLightClientAuditProofInput({
          tonMasterchainConfigVerifierHash: requestHashReplay,
        }),
      ),
    /request-bound hashes/,
  );
  assert.throws(
    () =>
      buildTonSccpFullLightClientAuditProofRequests(
        sampleTonFullLightClientAuditProofInput({
          tonMasterchainConfigVerifierHash: TON_TEMPLATE_SOURCE_MATERIAL_COMPONENT_HASHES.get("sourceTrustAnchorHash"),
        }),
      ),
    /built-in template material/,
  );
  assert.throws(
    () =>
      buildTonSccpFullLightClientAuditProofRequests(
        sampleTonFullLightClientAuditProofInput({
          shardStateVerificationProofHash: HEX32_A,
        }),
      ),
    /shardStateVerificationProofHash/,
  );
  const hashOnlyInput = sampleTonFullLightClientAuditProofInput({
    shardStateVerificationProofHash: shardStateVerificationProofHash,
  });
  delete hashOnlyInput.shardStateVerificationProof;
  assert.throws(
    () => buildTonSccpFullLightClientAuditProofRequests(hashOnlyInput),
    /shardStateVerificationProof is required/,
  );
  assert.throws(
    () =>
      canonicalTonSccpSourceStateVerificationProofBytes({
        ...input.shardStateVerificationProof,
        circuitId: SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1,
      }),
    /TON source-state/,
  );
  assert.throws(
    () =>
      canonicalTonSccpSourceStateVerificationProofBytes({
        ...input.shardStateVerificationProof,
        proofBytes: new Uint8Array([0, 0, 0]),
      }),
    /proofBytes must not be all zero/,
  );
  assert.throws(
    () =>
      canonicalTonSccpSourceStateVerificationProofBytes({
        ...input.shardStateVerificationProof,
        proofBase64: Buffer.from(input.shardStateVerificationProof.proofBytes).toString("base64"),
        proof_base64: Buffer.from(input.shardStateVerificationProof.proofBytes).toString("base64"),
      }),
    /sourceStateProof\.proofBase64 must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalTonSccpSourceStateVerificationProofBytes({
        ...input.shardStateVerificationProof,
        circuit_id: input.shardStateVerificationProof.circuitId,
      }),
    /sourceStateProof\.circuitId must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalTonSccpSourceStateVerificationProofBytes({
        ...input.shardStateVerificationProof,
        proof_bytes: input.shardStateVerificationProof.proofBytes,
      }),
    /sourceStateProof\.proofBytes must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildTonSccpFullLightClientAuditProofRequests(
        sampleTonFullLightClientAuditProofInput({
          masterchainConfigProofHash: HEX32_A,
        }),
      ),
    /masterchainConfigProofHash/,
  );
  assert.throws(
    () =>
      buildTonSccpFullLightClientAuditProofRequests(
        sampleTonFullLightClientAuditProofInput({
          validator_set_payload_hash: TON_VALIDATOR_SET_PAYLOAD_HASH,
        }),
      ),
    /validatorSetPayloadHash must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildTonSccpFullLightClientAuditProofRequests(
        sampleTonFullLightClientAuditProofInput({
          masterchainConfigProof: {
            configLeafHash: input.configLeafHash,
          },
        }),
      ),
    /configLeafHash must not use top-level and masterchainConfigProof aliases/,
  );
  assert.throws(
    () =>
      buildTonSccpFullLightClientAuditProofRequests(
        sampleTonFullLightClientAuditProofInput({
          sourceVerifierMaterial: {},
          source_verifier_material: {},
        }),
      ),
    /sourceVerifierMaterial must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildTonSccpFullLightClientAuditProofRequests(
        sampleTonFullLightClientAuditProofInput({
          sourceAdapterDeployment: {},
          source_adapter_deployment: {},
        }),
      ),
    /sourceAdapterDeployment must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildTonSccpFullLightClientAuditProofRequests(
        sampleTonFullLightClientAuditProofInput({
          masterchainConfigProof: {},
          masterchain_config_proof: {},
        }),
      ),
    /masterchainConfigProof must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildTonSccpFullLightClientAuditProofRequests(
        sampleTonFullLightClientAuditProofInput({
          shardStateProofPublicInputsHash: shardStateProofPublicInputsHash,
          shard_state_proof_public_inputs_hash: shardStateProofPublicInputsHash,
        }),
      ),
    /shardStateProofPublicInputsHash must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildTonSccpFullLightClientAuditProofRequests(
        sampleTonFullLightClientAuditProofInput({
          shardStateVerificationProofHash: shardStateVerificationProofHash,
          shard_state_verification_proof_hash: shardStateVerificationProofHash,
        }),
      ),
    /shardStateVerificationProofHash must not use multiple aliases/,
  );
});

test("wraps TON source-state proof requests with user-side proof bytes", async () => {
  const input = sampleTonFullLightClientAuditProofInput();
  const shardRequest = buildTonShardStateProofRequest(input);
  const wrappedShard = wrapTonSccpSourceStateVerificationProof(
    Uint8Array.from([9, 8, 7]),
    shardRequest,
  );
  assert.equal(wrappedShard.circuitId, SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1);
  assert.equal(wrappedShard.proofBase64, "CQgH");
  assert.ok(canonicalTonSccpSourceStateVerificationProofBytes(wrappedShard).length > 0);

  const auditRequests = buildTonSccpFullLightClientAuditProofRequests(input);
  const wrappedAudit = wrapTonSccpSourceStateVerificationProof(
    Uint8Array.from([1, 2, 3]),
    auditRequests.masterchainConfig,
  );
  assert.equal(
    wrappedAudit.circuitId,
    SCCP_TON_MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1,
  );
  assert.ok(canonicalTonSccpSourceStateVerificationProofBytes(wrappedAudit).length > 0);
  assert.throws(
    () => wrapTonSccpSourceStateVerificationProof(Uint8Array.from([0, 0]), shardRequest),
    /proofBytes must not be all zero/,
  );
  const tamperedShardRequest = mutableFastpqProofRequest(shardRequest);
  tamperedShardRequest.fastpqTransitions[0].newValue = "0x00";
  assert.throws(
    () => wrapTonSccpSourceStateVerificationProof(Uint8Array.from([9, 8, 7]), tamperedShardRequest),
    /canonical TON source-state request/u,
  );
  const tamperedShardHashRequest = mutableFastpqProofRequest(shardRequest);
  tamperedShardHashRequest.shardStateProofPublicInputsHash = HEX32_A;
  assert.throws(
    () => wrapTonSccpSourceStateVerificationProof(Uint8Array.from([9, 8, 7]), tamperedShardHashRequest),
    /statementBytes/u,
  );
  const tamperedShardDsidRequest = mutableFastpqProofRequest(shardRequest);
  tamperedShardDsidRequest.fastpqPublicInputs.dsid = "0x00000000000000000000000000000000";
  assert.throws(
    () => wrapTonSccpSourceStateVerificationProof(Uint8Array.from([9, 8, 7]), tamperedShardDsidRequest),
    /fastpqPublicInputs\.dsid/u,
  );
  const duplicateShardAliasRequest = mutableFastpqProofRequest(shardRequest);
  duplicateShardAliasRequest.source_domain = duplicateShardAliasRequest.sourceDomain;
  assert.throws(
    () => wrapTonSccpSourceStateVerificationProof(Uint8Array.from([9, 8, 7]), duplicateShardAliasRequest),
    /multiple aliases/u,
  );
  const duplicateShardFastpqAliasRequest = mutableFastpqProofRequest(shardRequest);
  duplicateShardFastpqAliasRequest.fastpqPublicInputs.tx_set_hash =
    duplicateShardFastpqAliasRequest.fastpqPublicInputs.txSetHash;
  assert.throws(
    () => wrapTonSccpSourceStateVerificationProof(
      Uint8Array.from([9, 8, 7]),
      duplicateShardFastpqAliasRequest,
    ),
    /multiple aliases/u,
  );
  const tamperedAuditRequest = mutableFastpqProofRequest(auditRequests.masterchainConfig);
  tamperedAuditRequest.fastpqTransitions[0].newValue = "0x00";
  assert.throws(
    () => wrapTonSccpSourceStateVerificationProof(Uint8Array.from([9, 8, 7]), tamperedAuditRequest),
    /canonical TON source-state request/u,
  );
  const tamperedAuditHashRequest = mutableFastpqProofRequest(auditRequests.masterchainConfig);
  tamperedAuditHashRequest.auditStatementHash = HEX32_A;
  assert.throws(
    () => wrapTonSccpSourceStateVerificationProof(Uint8Array.from([9, 8, 7]), tamperedAuditHashRequest),
    /statementBytes/u,
  );
  const tamperedAuditTxRequest = mutableFastpqProofRequest(auditRequests.masterchainConfig);
  tamperedAuditTxRequest.fastpqPublicInputs.txSetHash = HEX32_A;
  assert.throws(
    () => wrapTonSccpSourceStateVerificationProof(Uint8Array.from([9, 8, 7]), tamperedAuditTxRequest),
    /fastpqPublicInputs\.txSetHash/u,
  );

  let preflightCallbackInvoked = false;
  const preflightCheckingProver = new TonSccpSourceStateProver({
    prove() {
      preflightCallbackInvoked = true;
      return { proofBytes: Uint8Array.from([9, 8, 7]) };
    },
  });
  await assert.rejects(
    () => preflightCheckingProver.proveRequest(tamperedShardRequest),
    /canonical TON source-state request/u,
  );
  assert.equal(preflightCallbackInvoked, false);
  await assert.rejects(
    () => preflightCheckingProver.proveRequest(tamperedAuditRequest),
    /canonical TON source-state request/u,
  );
  assert.equal(preflightCallbackInvoked, false);

  const roles = [];
  const prover = new TonSccpSourceStateProver({
    prove(request) {
      roles.push(request.role ?? "shard_state");
      const result = {
        proofBytes: Uint8Array.from([9, 8, 7]),
        version: 1,
        proofFamily: SCCP_STARK_FRI_PROOF_FAMILY_V1,
        circuitId: request.circuitId,
        proofBase64: "CQgH",
        parameterSet: request.parameterSet,
        sourceDomain: request.sourceDomain,
        masterchainSeqno: request.masterchainSeqno,
        shardSeqno: request.shardSeqno,
        sourceStateVerifierId: request.sourceStateVerifierId,
        sourceStateVerifierHash: request.sourceStateVerifierHash,
        shardStateProofPublicInputsHash: request.shardStateProofPublicInputsHash,
        publicInputColumns: request.publicInputColumns,
        fastpqPublicInputs: request.fastpqPublicInputs,
        fastpqTransitions: request.fastpqTransitions,
        statementBytes: request.statementBytes,
        verificationContextBytes: request.verificationContextBytes,
        schemaDescriptor: request.schemaDescriptor,
      };
      if (request.role !== undefined) {
        result.role = request.role;
        result.roleCode = request.roleCode;
        result.verifierId = request.verifierId;
        result.verifierHash = request.verifierHash;
        result.sourceVerifierMaterialHash = request.sourceVerifierMaterialHash;
        result.sourceAdapterDeploymentHash = request.sourceAdapterDeploymentHash;
        result.fullLightClientGateHash = request.fullLightClientGateHash;
        result.shardStateVerificationProofHash = request.shardStateVerificationProofHash;
        result.auditStatementHash = request.auditStatementHash;
      } else {
        result.witnessCommitmentBytes = request.witnessCommitmentBytes;
      }
      return result;
    },
  });
  const shardProof = await prover.proveShardState(input);
  const auditProofs = await prover.proveFullLightClientAudit(input);
  assert.equal(shardProof.circuitId, SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1);
  assert.deepEqual(roles, [
    "shard_state",
    "masterchain_config",
    "validator_set_transition",
    "shard_accounts_dictionary",
  ]);
  assert.deepEqual(Object.keys(auditProofs), [
    "masterchainConfig",
    "validatorSetTransition",
    "shardAccountsDictionary",
  ]);
  assert.equal(
    auditProofs.validatorSetTransition.circuitId,
    SCCP_TON_VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1,
  );
  assert.equal(
    auditProofs.shardAccountsDictionary.circuitId,
    SCCP_TON_SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1,
  );
  assert.equal(auditProofs.shardAccountsDictionary.proofBase64, "CQgH");
  await assert.rejects(
    () => new TonSccpSourceStateProver().proveRequest(shardRequest),
    (error) =>
      error?.code === "ERR_SCCP_TON_SOURCE_STATE_PROVER_UNAVAILABLE" &&
      /source-state prover is not linked/.test(error.message),
  );
  await assert.rejects(
    () =>
      new TonSccpSourceStateProver({
        prove() {
          return {
            proofBytes: [1, 2, 3],
            proofFamily: "debug-proof-family",
          };
        },
      }).proveShardState(input),
    /result\.proofFamily/u,
  );
  await assert.rejects(
    () =>
      new TonSccpSourceStateProver({
        prove(request) {
          return {
            proofBytes: [1, 2, 3],
            circuit_id: request.circuitId,
            proof_family: SCCP_STARK_FRI_PROOF_FAMILY_V1,
            proof_base64: "AAAA",
            version: 1,
          };
        },
      }).proveShardState(input),
    /result\.proofBase64/u,
  );
  await assert.rejects(
    () =>
      new TonSccpSourceStateProver({
        prove(request) {
          return {
            proofBytes: [1, 2, 3],
            statementBytes: Uint8Array.from([request.statementBytes[0] ^ 0xff]),
          };
        },
      }).proveShardState(input),
    /source-state prover result\.statementBytes must match request\.statementBytes/u,
  );
  await assert.rejects(
    () =>
      new TonSccpSourceStateProver({
        prove(request) {
          return {
            proofBytes: [1, 2, 3],
            masterchainSeqno: (BigInt(request.masterchainSeqno) + 1n).toString(),
          };
        },
      }).proveShardState(input),
    /source-state prover result\.masterchainSeqno must match request\.masterchainSeqno/u,
  );
  await assert.rejects(
    () =>
      new TonSccpSourceStateProver({
        prove(request) {
          return {
            proofBytes: [1, 2, 3],
            shardStateVerificationProofHash:
              request.shardStateVerificationProofHash === HEX32_A ? HEX32_B : HEX32_A,
          };
        },
      }).proveFullLightClientAudit(input),
    /source-state prover result\.shardStateVerificationProofHash must match request\.shardStateVerificationProofHash/u,
  );
});

test("TON source-state prover snapshots mutable callback requests", async () => {
  const builtRequest = buildTonShardStateProofRequest(sampleTonFullLightClientAuditProofInput());
  const mutableRequest = mutableFastpqProofRequest(builtRequest);
  const expectedStatementByte = mutableRequest.statementBytes[0];
  const prover = new TonSccpSourceStateProver({
    prove(request) {
      assert.notStrictEqual(request, mutableRequest);
      assert.equal(Object.isFrozen(request), true);
      assert.equal(Object.isFrozen(request.fastpqTransitions), true);
      assert.equal(Object.isFrozen(request.fastpqTransitions[0]), true);
      assert.throws(() => {
        request.circuitId = SCCP_TON_MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1;
      }, TypeError);
      assert.throws(() => {
        request.fastpqTransitions[0].newValue = "0x00";
      }, TypeError);
      const exposedStatement = request.statementBytes;
      exposedStatement[0] ^= 0xff;
      assert.equal(request.statementBytes[0], expectedStatementByte);
      mutableRequest.circuitId = SCCP_TON_MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1;
      return [4, 5, 6];
    },
  });

  const proof = await prover.proveRequest(mutableRequest);

  assert.equal(proof.circuitId, builtRequest.circuitId);
  assert.equal(proof.proofBase64, "BAUG");
});

test("builds Solana SCCP program instruction submission data", () => {
  const solanaDestinationBindingHash = sccpDestinationBindingHash(SCCP_DOMAIN_SOL);
  const proofRequest = buildSolanaSccpProofRequest(
    sampleProductionWitness({ destinationBindingHash: solanaDestinationBindingHash }),
  );
  const proofResult = wrapSolanaSccpProofResult(
    Uint8Array.from([1, 2, 3, 4]),
    proofRequest,
  );
  const transparentPublicInputs = {
    messageId: proofRequest.publicInputs.messageId,
    payloadHash: proofRequest.publicInputs.payloadHash,
    targetDomain: SCCP_DOMAIN_SOL,
    commitmentRoot: proofRequest.publicInputs.commitmentRoot,
    finalityHeight: proofRequest.publicInputs.finalizedSlot,
    finalityBlockHash: proofRequest.publicInputs.bankHash,
  };
  const submission = buildSolanaSccpSubmission({
    publicInputs: transparentPublicInputs,
    proofResult,
    bundleBytes: Uint8Array.from([5, 6, 7]),
  });

  assert.equal(submission.envelopeEncoding, SCCP_SOLANA_BORSH_INSTRUCTION_V1);
  assert.equal(submission.submissionKind, "program_instruction");
  assert.equal(submission.verifierEntrypoint, "submit_sccp_message_proof");
  assert.equal(Object.isFrozen(submission), true);
  assert.deepEqual(
    submission.arguments.map((argument) => argument.key),
    [
      "proof_bytes",
      "public_inputs",
      "bundle_bytes",
      "statement_hash",
      "destination_binding_hash",
      "proof_context_hash",
    ],
  );
  assert.equal(submission.publicInputsBytes.length, 141);
  assert.equal(
    submission.proofContextHash,
    solanaSccpProofContextHash({
      statementHash: HEX32_G,
      destinationBindingHash: solanaDestinationBindingHash,
    }),
  );
  assert.equal(submission.instructionDataHex, submission.envelopeHex);
  assert.equal(
    new TextDecoder().decode(submission.instructionData.slice(4, 29)),
    "submit_sccp_message_proof",
  );
  const exposedSubmissionProof = submission.proofBytes;
  exposedSubmissionProof[0] = 99;
  assert.deepEqual(Array.from(submission.proofBytes), [1, 2, 3, 4]);
  const exposedInstructionData = submission.instructionData;
  exposedInstructionData[4] = 99;
  assert.equal(
    new TextDecoder().decode(submission.instructionData.slice(4, 29)),
    "submit_sccp_message_proof",
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        proofResult,
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /requires transparent publicInputs/,
  );
  const proofResultWithoutEnvelope = { ...proofResult };
  delete proofResultWithoutEnvelope.envelopeHash;
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult: proofResultWithoutEnvelope,
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.envelopeHash must be non-zero/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult: { ...proofResult, envelopeHash: HEX32_A },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.envelopeHash must match wrapped proof bytes/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult,
        proofBytes: Uint8Array.from([9]),
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofBytes must match proofResult\.proofBytes/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult: { ...proofResult, version: 2 },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.version must be 1/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult: { ...proofResult, proofBase64: "AAAA" },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.proofBase64 must match proofResult\.proofBytes/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult: { ...proofResult, proof_bytes: proofResult.proofBytes },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.proofBytes.*multiple aliases/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult: {
          ...proofResult,
          proof_context_hash: proofResult.proofContextHash,
        },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.proofContextHash.*multiple aliases/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult: {
          ...proofResult,
          publicInputs: {
            ...proofResult.publicInputs,
            bank_hash: proofResult.publicInputs.bankHash,
          },
        },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.publicInputs\.bankHash.*multiple aliases/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult: { ...proofResult, witnessHash: SCCP_ZERO_HASH_V1 },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.witnessHash must not be zero/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult: { ...proofResult, proofContextHash: HEX32_C },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofContextHash must match/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult: {
          ...proofResult,
          proofContext: { ...proofResult.proofContext, version: 2 },
        },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.proofContext\.version must be 1/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult,
        bundleBytes: Uint8Array.from([5, 6, 7]),
        proofContext: false,
      }),
    /proofContext must be an object/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult,
        bundleBytes: Uint8Array.from([5, 6, 7]),
        proofContext: null,
      }),
    /proofContext must be an object/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult,
        proofBytes: null,
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofBytes must be bytes or hex/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult,
        statementHash: null,
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /statementHash/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult,
        proofContextHash: null,
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofContextHash/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: null,
        transparentPublicInputs,
        proofResult,
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /publicInputs/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult: { ...proofResult, proofContext: false },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofContext must be an object/,
  );
  const proofResultWithoutDeploymentBinding = { ...proofResult };
  delete proofResultWithoutDeploymentBinding.sourceAdapterDeploymentBinding;
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult: proofResultWithoutDeploymentBinding,
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.sourceAdapterDeploymentBinding is required/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult: {
          ...proofResult,
          sourceAdapterDeploymentBinding: {
            ...proofResult.sourceAdapterDeploymentBinding,
            version: 2,
          },
        },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.sourceAdapterDeploymentBinding\.version must be 1/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult: {
          ...proofResult,
          sourceAdapterDeploymentBinding: {
            ...proofResult.sourceAdapterDeploymentBinding,
            sourceAdapterDeploymentHash: HEX32_C,
          },
        },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /sourceAdapterDeploymentBindingHash must match sourceAdapterDeploymentBinding/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult: {
          ...proofResult,
          publicInputs: {
            ...proofResult.publicInputs,
            sourceAdapterDeploymentHash: HEX32_C,
          },
        },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.publicInputs\.sourceAdapterDeploymentHash must match sourceAdapterDeploymentBinding/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult: {
          ...proofResult,
          publicInputs: {
            ...proofResult.publicInputs,
            sourceStateVerifierId: "sccp:solana:wrong-source-state-verifier:v1",
          },
        },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.publicInputs\.sourceStateVerifierId must match proofResult\.sourceStateVerifierId/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult: {
          ...proofResult,
          publicInputs: {
            ...proofResult.publicInputs,
            sourceStateVerifierHash: HEX32_D,
          },
        },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.publicInputs\.sourceStateVerifierHash must match proofResult\.sourceStateVerifierHash/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult: {
          ...proofResult,
          publicInputs: {
            ...proofResult.publicInputs,
            parentSlot: proofResult.publicInputs.finalizedSlot,
          },
        },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.publicInputs\.parentSlot must be the direct parent/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofResult: {
          ...proofResult,
          publicInputs: {
            ...proofResult.publicInputs,
            messageProofHash: SCCP_ZERO_HASH_V1,
          },
        },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.publicInputs\.messageProofHash must not be zero/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: { ...transparentPublicInputs, messageId: HEX32_A },
        proofResult,
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.publicInputs\.messageId must match/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: transparentPublicInputs,
        proofBytes: proofResult.proofBytes,
        bundleBytes: Uint8Array.from([8]),
        statementHash: HEX32_G,
        destinationBindingHash: solanaDestinationBindingHash,
      }),
    /proofResult must be a wrapped Solana SCCP proof result/,
  );
  const mismatchedPublicInputsBytes =
    canonicalSccpMessageTransparentPublicInputsBytes(submission.publicInputs);
  mismatchedPublicInputsBytes[5] ^= 1;
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: {
          ...submission.publicInputs,
          targetDomain: SCCP_DOMAIN_SORA,
        },
        proofResult,
        proofBytes: Uint8Array.from([1, 2]),
        bundleBytes: Uint8Array.from([5, 6, 7]),
        statementHash: HEX32_G,
        destinationBindingHash: solanaDestinationBindingHash,
      }),
    /publicInputs\.targetDomain must be Solana/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: submission.publicInputs,
        publicInputsBytes: mismatchedPublicInputsBytes,
        proofResult,
        proofBytes: Uint8Array.from([1, 2]),
        bundleBytes: Uint8Array.from([5, 6, 7]),
        statementHash: HEX32_G,
        destinationBindingHash: solanaDestinationBindingHash,
      }),
    /publicInputsBytes must match canonical/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: submission.publicInputs,
        proofResult,
        proofBytes: [1],
        bundleBytes: [2],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /destinationBindingHash must match canonical Solana destination binding/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: submission.publicInputs,
        proofResult,
        proofBytes: [0, 0],
        bundleBytes: [2],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /proofBytes must not be all zero/,
  );
  assert.throws(
    () =>
      buildSolanaSccpSubmission({
        publicInputs: submission.publicInputs,
        proofResult,
        proofBytes: [1],
        bundleBytes: [2],
        statementHash: HEX32_G,
        destinationBindingHash: solanaDestinationBindingHash,
        proofContextHash: HEX32_C,
      }),
    /proofContextHash must match/,
  );
});

test("rejects malformed EVM-family and TRON Groth16 proof tuples", () => {
  const evmBinding = sampleEvmDestinationBinding();
  const evmRequest = buildEvmSccpProofRequest({
    publicInputs: sampleEvmPublicInputs,
    bundleBytes: [5, 6, 7],
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBinding: evmBinding,
  });
  assert.throws(
    () => wrapEvmSccpProofResult(groth16ProofBytes([abiWord(2)]), evmRequest),
    /proofBytes\.version must be 1/,
  );

  const tronBinding = sampleTronDestinationBinding();
  const tronRequest = buildTronSccpProofRequest({
    publicInputs: sampleTronPublicInputs,
    bundleBytes: [5, 6, 7],
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBinding: tronBinding,
  });
  const outOfRangeA = groth16ProofBytes();
  outOfRangeA.fill(0xff, 4 * 32, 5 * 32);
  assert.throws(
    () => wrapTronSccpProofResult(outOfRangeA, tronRequest),
    /proofBytes\.a\.x must be a BN254 base-field element/,
  );

  const zeroA = groth16ProofBytes();
  zeroA.fill(0, 4 * 32, 6 * 32);
  assert.throws(
    () => wrapTronSccpProofResult(zeroA, tronRequest),
    /proofBytes\.a must not be zero/,
  );

  const zeroB = groth16ProofBytes();
  zeroB.fill(0, 6 * 32, 10 * 32);
  assert.throws(
    () => wrapTronSccpProofResult(zeroB, tronRequest),
    /proofBytes\.b must not be zero/,
  );

  const zeroC = groth16ProofBytes();
  zeroC.fill(0, 10 * 32, 12 * 32);
  assert.throws(
    () => wrapTronSccpProofResult(zeroC, tronRequest),
    /proofBytes\.c must not be zero/,
  );

  const offCurveC = groth16ProofBytes();
  offCurveC.set(abiWord(3), 11 * 32);
  assert.throws(
    () => wrapEvmSccpProofResult(offCurveC, evmRequest),
    /proofBytes\.c must be a BN254 G1 point/,
  );

  const offCurveB = groth16ProofBytes();
  offCurveB[6 * 32 + 31] ^= 0x01;
  assert.throws(
    () => wrapTronSccpProofResult(offCurveB, tronRequest),
    /proofBytes\.b must be a BN254 G2 point/,
  );

  const nonSubgroupB = groth16ProofBytes();
  [
    abiWord(0),
    abiWord(1),
    abiWord(0x0cf32d3c49a2cb8a092f24ec3201e68dc299b6216e6321ee60573e3a7f596ea8n),
    abiWord(0x07bca656753ef8cbee60335acbffe3def91636952d4ab9eb0b839c7f3566c0e2n),
  ].forEach((word, offset) => nonSubgroupB.set(word, (6 + offset) * 32));
  assert.throws(
    () => wrapEvmSccpProofResult(nonSubgroupB, evmRequest),
    /proofBytes\.b must be a BN254 G2 point/,
  );
  assert.throws(
    () => wrapTronSccpProofResult(nonSubgroupB, tronRequest),
    /proofBytes\.b must be a BN254 G2 point/,
  );

  const wrongMessageId = groth16ProofBytes();
  wrongMessageId.fill(0x22, 32, 64);
  assert.throws(
    () => wrapEvmSccpProofResult(wrongMessageId, evmRequest),
    /proofBytes\.messageId must match publicInputs\.messageId/,
  );

  const wrongSourceDomain = groth16ProofBytes();
  wrongSourceDomain.set(abiWord(999), 2 * 32);
  assert.throws(
    () => wrapTronSccpProofResult(wrongSourceDomain, tronRequest),
    /proofBytes\.sourceDomain must match sourceDomain/,
  );
  assert.throws(
    () => sccpSubmitMessageProofCallData(wrongSourceDomain, sampleTronPublicInputs, HEX32_G),
    /proofBytes\.sourceDomain must match sourceDomain/,
  );
  assert.throws(
    () =>
      sccpSubmitMessageProofCallData(
        wrongSourceDomain,
        sampleTronPublicInputs,
        HEX32_G,
        SCCP_DOMAIN_ETH,
      ),
    /sourceDomain must be SORA/,
  );

  const wrongCommitmentRoot = groth16ProofBytes();
  wrongCommitmentRoot.fill(0x44, 3 * 32, 4 * 32);
  assert.throws(
    () =>
      buildEvmSccpSubmission({
        proofBytes: wrongCommitmentRoot,
        publicInputs: sampleEvmPublicInputs,
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /proofBytes\.commitmentRoot must match publicInputs\.commitmentRoot/,
  );
});

test("does not generate Solana SCCP proofs without a linked local prover", async () => {
  const prover = new SolanaSccpProver();

  await assert.rejects(
    () => prover.prove(sampleWitness()),
    (error) => error?.code === "ERR_SCCP_SOLANA_PROVER_UNAVAILABLE",
  );
});

test("wraps externally generated Solana SCCP proof bytes", async () => {
  const productionWitness = sampleProductionWitness();
  let callbackRequest;
  const prover = new SolanaSccpProver({
    prove: async (request) => {
      callbackRequest = request;
      assert.equal(Object.isFrozen(request), true);
      assert.equal(request.backend, SCCP_SOLANA_RECURSIVE_PROOF_BACKEND_V1);
      assert.equal(request.proofContext.statementHash, HEX32_G);
      const proofBytes = Uint8Array.from([1, 2, 3, 4]);
      const wrapped = wrapSolanaSccpProofResult(proofBytes, request);
      return {
        proofBytes,
        proofBase64: "AQIDBA==",
        publicInputs: request.publicInputs,
        sourceStateVerifierId: request.sourceStateVerifierId,
        sourceStateVerifierHash: request.sourceStateVerifierHash,
        proofContext: request.proofContext,
        sourceAdapterDeploymentBinding: request.sourceAdapterDeploymentBinding,
        witnessHash: request.witnessHash,
        proofContextHash: request.proofContextHash,
        sourceAdapterDeploymentBindingHash: request.sourceAdapterDeploymentBindingHash,
        envelopeHash: wrapped.envelopeHash,
      };
    },
  });

  const result = await prover.prove(productionWitness);
  const request = buildSolanaSccpProofRequest(productionWitness);
  const directResult = wrapSolanaSccpProofResult(Uint8Array.from([1, 2, 3, 4]), request);

  assert.notEqual(callbackRequest, request);
  assert.deepEqual(callbackRequest, request);
  assert.deepEqual(Array.from(result.proofBytes), [1, 2, 3, 4]);
  assert.equal(Object.isFrozen(result), true);
  assert.equal(result.proofBase64, "AQIDBA==");
  assert.equal(result.proofContextHash, request.proofContextHash);
  assert.equal(directResult.envelopeHash, result.envelopeHash);
  assert.match(result.envelopeHash, /^0x[0-9a-f]{64}$/);
  assert.throws(
    () =>
      wrapSolanaSccpProofResult(
        new Uint8Array(SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1).fill(1),
        request,
      ),
    /at most/,
  );
  const exposedProof = result.proofBytes;
  exposedProof[0] = 99;
  assert.deepEqual(Array.from(result.proofBytes), [1, 2, 3, 4]);
  assert.throws(
    () => wrapSolanaSccpProofResult([1], { ...request, witnessHash: HEX32_A }),
    /Solana SCCP proof request must be canonical/,
  );
  assert.throws(
    () =>
      wrapSolanaSccpProofResult([1], {
        ...request,
        publicInputs: { ...request.publicInputs, bankHash: HEX32_B },
      }),
    /Solana SCCP proof request must be canonical/,
  );
  await assert.rejects(
    () =>
      new SolanaSccpProver({
        prove: async (linkedRequest) => ({
          proofBytes: [1, 2, 3, 4],
          publicInputs: {
            ...linkedRequest.publicInputs,
            messageId: HEX32_A,
          },
        }),
      }).prove(productionWitness),
    /proofResult\.publicInputs must match request/,
  );
  await assert.rejects(
    () =>
      new SolanaSccpProver({
        prove: async () => ({
          proofBytes: [1, 2, 3, 4],
          proofBase64: "AAAA",
        }),
      }).prove(productionWitness),
    /proofResult\.proofBase64 must match proofResult\.proofBytes/,
  );
  await assert.rejects(
    () =>
      new SolanaSccpProver({
        prove: async () => ({
          proofBytes: [1, 2, 3, 4],
          proof_bytes: [1, 2, 3, 4],
        }),
      }).prove(productionWitness),
    /proofResult\.proofBytes.*multiple aliases/,
  );
  await assert.rejects(
    () =>
      new SolanaSccpProver({
        prove: async (linkedRequest) => ({
          proofBytes: [1, 2, 3, 4],
          sourceStateVerifierId: linkedRequest.sourceStateVerifierId,
          source_state_verifier_id: linkedRequest.sourceStateVerifierId,
        }),
      }).prove(productionWitness),
    /proofResult\.sourceStateVerifierId.*multiple aliases/,
  );
  await assert.rejects(
    () =>
      new SolanaSccpProver({
        prove: async (linkedRequest) => ({
          proofBytes: [1, 2, 3, 4],
          publicInputs: {
            ...linkedRequest.publicInputs,
            bank_hash: linkedRequest.publicInputs.bankHash,
          },
        }),
      }).prove(productionWitness),
    /proofResult\.publicInputs\.bankHash.*multiple aliases/,
  );
  await assert.rejects(
    () =>
      new SolanaSccpProver({
        prove: async (linkedRequest) => ({
          proofBytes: [1, 2, 3, 4],
          proofContext: {
            ...linkedRequest.proofContext,
            statementHash: HEX32_A,
          },
        }),
      }).prove(productionWitness),
    /proofResult\.proofContext must match request/,
  );

  const zeroProofProver = new SolanaSccpProver({
    prove: async () => ({ proofBytes: [0, 0] }),
  });
  await assert.rejects(
    () => zeroProofProver.prove(productionWitness),
    /proofBytes must not be all zero/,
  );

  await assert.rejects(
    () =>
      new SolanaSccpProver({
        prove: async () => {
          throw new Error("local prover should not be invoked");
        },
      }).prove(sampleProductionWitness({ mainnetGenesisHash: "devnet" })),
    /mainnetGenesisHash must match Solana mainnet-beta/,
  );

  await assert.rejects(
    () =>
      new SolanaSccpProver({
        prove: async () => {
          throw new Error("local prover should not be invoked");
        },
      }).prove(sampleProductionWitness({ accountsLtHash: undefined })),
    /accountsLtHash must be present for Solana production proofs/,
  );

  await assert.rejects(
    () =>
      new SolanaSccpProver({
        prove: async () => {
          throw new Error("local prover should not be invoked");
        },
      }).prove(sampleWitness()),
    /sourceStateVerifierHash must not be zero for Solana production proofs/,
  );

  await assert.rejects(
    () =>
      new SolanaSccpProver({
        prove: async () => {
          throw new Error("local prover should not be invoked");
        },
      }).prove(
        sampleProductionWitness({
          sourceStateVerifierHash: SCCP_SOLANA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1,
        }),
      ),
    /Solana template verifier hash/,
  );

  await assert.rejects(
    () =>
      new SolanaSccpProver({
        prove: async () => {
          throw new Error("local prover should not be invoked");
        },
      }).prove(
        sampleWitness({
          sourceStateVerifierHash: HEX32_C,
          sourceAdapterDeploymentHash: HEX32_A,
          sourceAdapterDeploymentReceiptHash: HEX32_B,
        }),
      ),
    /inclusionBranch must not be empty for Solana production proofs/,
  );
});

test("builds TON SCCP internal message BOC in browser-safe JavaScript", () => {
  const rawMessageInput = {
    publicInputs: sampleTonPublicInputs,
    proofBytes: Uint8Array.from([1, 2, 3, 4]),
    bundleBytes: Uint8Array.from([5, 6, 7]),
    statementHash: HEX32_B,
    destinationBindingHash: HEX32_G,
    metadataBytes: Uint8Array.from([8, 9]),
  };
  assert.throws(
    () => buildSccpTonMessageBodyBoc(rawMessageInput),
    /proofResult must be a wrapped TON SCCP proof result/,
  );
  assert.throws(
    () => buildTonSccpSubmission(rawMessageInput),
    /proofResult must be a wrapped TON SCCP proof result/,
  );

  const request = buildTonSccpProofRequest({
    publicInputs: sampleTonPublicInputs,
    bundleBytes: Uint8Array.from([5, 6, 7]),
    sourceProofBytes: Uint8Array.from([9, 10]),
    statementHash: HEX32_B,
    destinationBindingHash: HEX32_G,
    sourceStateVerifierHash: HEX32_C,
    sourceAdapterDeploymentHash: HEX32_A,
    sourceAdapterDeploymentReceiptHash: HEX32_D,
  });
  const proofResult = wrapTonSccpProofResult(Uint8Array.from([1, 2, 3, 4]), request);
  const messageBodyBoc = buildSccpTonMessageBodyBoc({
    proofResult,
    bundleBytes: Uint8Array.from([5, 6, 7]),
    metadataBytes: Uint8Array.from([8, 9]),
  });

  assert.deepEqual(Array.from(messageBodyBoc.slice(0, 4)), [0xb5, 0xee, 0x9c, 0x72]);
  assert.ok(messageBodyBoc.length > canonicalSccpMessageTransparentPublicInputsBytes(sampleTonPublicInputs).length);

  const submission = buildTonSccpSubmission({
    proofResult,
    bundleBytes: Uint8Array.from([5, 6, 7]),
    metadataBytes: Uint8Array.from([8, 9]),
  });
  assert.equal(submission.envelopeEncoding, SCCP_TON_MESSAGE_BODY_BOC_V1);
  assert.equal(submission.arguments[0].key, "message_body_boc");
  assert.equal(submission.arguments[0].encoding, "ton_boc");
  assert.equal(submission.envelopeHex, submission.messageBodyBocHex);
  assert.equal(submission.arguments[0].bytes, submission.messageBodyBocHex);
  assert.equal(Object.isFrozen(submission), true);
  assert.equal(Object.isFrozen(submission.arguments), true);
  assert.equal(Object.isFrozen(submission.arguments[0]), true);
  const exposedMessageBodyBoc = submission.messageBodyBoc;
  const exposedEnvelopeBytes = submission.envelopeBytes;
  exposedMessageBodyBoc[0] = 0;
  exposedEnvelopeBytes[0] = 0;
  assert.equal(submission.messageBodyBoc[0], 0xb5);
  assert.equal(submission.envelopeBytes[0], 0xb5);

  const proofResultSubmission = buildTonSccpSubmission({
    proofResult,
    bundleBytes: Uint8Array.from([5, 6, 7]),
    metadataBytes: Uint8Array.from([8, 9]),
  });
  assert.equal(proofResultSubmission.envelopeHex, submission.envelopeHex);
  assert.deepEqual(Array.from(proofResult.bundleBytes), [5, 6, 7]);
  assert.deepEqual(Array.from(proofResult.sourceProofBytes), [9, 10]);
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult,
        proof_result: proofResult,
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult: { ...proofResult, proof_bytes: proofResult.proofBytes },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.proofBytes must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult: { ...proofResult, request_hash: proofResult.requestHash },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.requestHash must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult: { ...proofResult, proof_context: proofResult.proofContext },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.proofContext must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult,
        bundleBytes: Uint8Array.from([5, 6, 7]),
        bundle_bytes: Uint8Array.from([5, 6, 7]),
      }),
    /bundleBytes must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult,
        publicInputs: null,
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /publicInputs must be an object/,
  );
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult,
        proofBytes: null,
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofBytes must be bytes or hex/,
  );
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult,
        statementHash: null,
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /statementHash/,
  );
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult,
        destinationBindingHash: null,
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /destinationBindingHash/,
  );
  const omittedSourceProofResult = wrapTonSccpProofResult(
    Uint8Array.from([1, 2, 3, 4]),
    buildTonSccpProofRequest({
      publicInputs: sampleTonPublicInputs,
      bundleBytes: Uint8Array.from([5, 6, 7]),
      statementHash: HEX32_B,
      destinationBindingHash: HEX32_G,
      sourceStateVerifierHash: HEX32_C,
      sourceAdapterDeploymentHash: HEX32_A,
      sourceAdapterDeploymentReceiptHash: HEX32_D,
    }),
  );
  const omittedSourceProofSubmission = buildTonSccpSubmission({
    proofResult: omittedSourceProofResult,
    bundleBytes: Uint8Array.from([5, 6, 7]),
    metadataBytes: Uint8Array.from([8, 9]),
  });
  assert.equal(omittedSourceProofResult.sourceProofBytes.length, 0);
  assert.equal(omittedSourceProofSubmission.envelopeHex, submission.envelopeHex);
  for (const badMetadataBytes of [false, 0, ""]) {
    assert.throws(
      () =>
        buildTonSccpSubmission({
          proofResult,
          bundleBytes: Uint8Array.from([5, 6, 7]),
          metadataBytes: badMetadataBytes,
        }),
      /metadataBytes must be bytes or hex|metadataBytes must be canonical hex/,
    );
  }
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult: { ...proofResult, proofContext: false },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.proofContext must be an object/,
  );
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult: { ...proofResult, proofContext: null },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.proofContext must be an object/,
  );
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult,
        bundleBytes: Uint8Array.from([5, 6, 8]),
      }),
    /bundleBytes must match proofResult\.bundleBytes/,
  );
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult: {
          ...proofResult,
          bundleBytes: Uint8Array.from([5, 6, 8]),
        },
        bundleBytes: Uint8Array.from([5, 6, 8]),
      }),
    /proofResult\.requestHash must match bundleBytes and sourceProofBytes/,
  );
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult,
        proofBytes: Uint8Array.from([4, 3, 2, 1]),
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofBytes must match proofResult\.proofBytes/,
  );
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult,
        publicInputs: { ...sampleTonPublicInputs, messageId: HEX32_A },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /publicInputs must match proofResult\.publicInputs/,
  );
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult: { ...proofResult, envelopeHash: HEX32_A },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.envelopeHash must match wrapped proof bytes/,
  );
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult: { ...proofResult, sourceStateVerifierHash: SCCP_ZERO_HASH_V1 },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.sourceStateVerifierHash must not be zero/,
  );
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult: {
          ...proofResult,
          sourceAdapterDeploymentBinding: {
            ...proofResult.sourceAdapterDeploymentBinding,
            targetDomain: SCCP_DOMAIN_TON,
          },
        },
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofResult\.sourceAdapterDeploymentBinding\.targetDomain must be SORA/,
  );
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult,
        publicInputs: { ...sampleTonPublicInputs, targetDomain: SCCP_DOMAIN_SOL },
        proofBytes: Uint8Array.from([1, 2, 3, 4]),
        bundleBytes: Uint8Array.from([5, 6, 7]),
        statementHash: HEX32_B,
        destinationBindingHash: HEX32_G,
      }),
    /publicInputs\.targetDomain must be TON/,
  );
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult,
        bundleBytes: Uint8Array.from([]),
      }),
    /bundleBytes must not be empty/,
  );
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult,
        proofBytes: Uint8Array.from([0, 0]),
        bundleBytes: Uint8Array.from([5, 6, 7]),
      }),
    /proofBytes must not be all zero/,
  );
  const oversizedTonMessageProof = new Uint8Array(4096 * 127).fill(1);
  const oversizedTonMessageResult = wrapTonSccpProofResult(oversizedTonMessageProof, request);
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult: oversizedTonMessageResult,
        bundleBytes: Uint8Array.from([5, 6, 7]),
        metadataBytes: Uint8Array.from([8, 9]),
      }),
    /TON BOC contains too many cells/,
  );

  const manifest = {
    version: 1,
    localDomain: SCCP_DOMAIN_SORA,
    counterpartyDomain: SCCP_DOMAIN_TON,
    securityModel: "RecursiveZk",
    anchorGovernance: "CryptographicProof",
    verifierTarget: "TonContract",
    verifierBackendFamily: "TonContract",
    proofFamily: SCCP_STARK_FRI_PROOF_FAMILY_V1,
    verifierBackendKey: SCCP_TON_CONTRACT_PROOF_BACKEND_V1,
    messageBackend: "sccp-message-v1",
    registryBackend: "sccp-registry-v1",
    manifestSeed: "iroha:sccp:bridge-proof:message:stark-fri:v1:ton",
    destinationBinding: { key: "sora:ton", bindingHash: HEX32_H },
  };
  assert.throws(
    () =>
      buildTonSccpSubmission({
        proofResult,
        bundleBytes: Uint8Array.from([5, 6, 7]),
        destinationBindingHash: HEX32_G,
        manifest,
      }),
    /destinationBindingHash must match destinationBinding\.bindingHash/,
  );
});

test("binds TON proof requests to relay context and source adapter deployment", () => {
  const request = buildTonSccpProofRequest({
    publicInputs: sampleTonPublicInputs,
    bundleBytes: [5, 6, 7],
    sourceProofBytes: [9, 10],
    statementHash: HEX32_G,
    destinationBindingHash: HEX32_H,
    sourceStateVerifierHash: HEX32_C,
    sourceAdapterDeploymentHash: HEX32_A,
    sourceAdapterDeploymentReceiptHash: HEX32_B,
  });

  assert.equal(request.backend, SCCP_TON_CONTRACT_PROOF_BACKEND_V1);
  assert.equal(request.sourceDomain, SCCP_DOMAIN_TON);
  assert.equal(request.sourceStateVerifierId, SCCP_TON_MAINNET_SHARD_STATE_VERIFIER_ID_V1);
  assert.equal(request.sourceStateVerifierHash, HEX32_C);
  assert.deepEqual(request.proofContext, {
    version: 1,
    statementHash: HEX32_G,
    destinationBindingHash: HEX32_H,
  });
  assert.equal(request.sourceAdapterDeploymentBinding.sourceDomain, SCCP_DOMAIN_TON);
  assert.equal(request.sourceAdapterDeploymentBinding.targetDomain, SCCP_DOMAIN_SORA);
  assert.equal(request.sourceAdapterDeploymentBinding.sourceAdapterDeploymentHash, HEX32_A);
  assert.equal(request.sourceAdapterDeploymentBinding.sourceAdapterDeploymentReceiptHash, HEX32_B);
  assert.equal(Object.isFrozen(request), true);
  assert.equal(Object.isFrozen(request.publicInputs), true);
  assert.equal(Object.isFrozen(request.proofContext), true);
  assert.equal(Object.isFrozen(request.sourceAdapterDeploymentBinding), true);
  assert.equal(
    request.sourceAdapterDeploymentBindingHash,
    sccpSourceAdapterDeploymentBindingHash(request.sourceAdapterDeploymentBinding),
  );
  assert.match(request.requestHash, /^0x[0-9a-f]{64}$/);
  assert.notEqual(
    request.requestHash,
    buildTonSccpProofRequest({
      publicInputs: sampleTonPublicInputs,
      bundleBytes: [5, 6, 7],
      sourceProofBytes: [9, 10],
      statementHash: HEX32_G,
      destinationBindingHash: HEX32_H,
      sourceStateVerifierHash: HEX32_C,
      sourceAdapterDeploymentHash: HEX32_C,
      sourceAdapterDeploymentReceiptHash: HEX32_D,
    }).requestHash,
  );
  assert.notEqual(
    request.requestHash,
    buildTonSccpProofRequest({
      publicInputs: sampleTonPublicInputs,
      bundleBytes: [5, 6, 7],
      sourceProofBytes: [9, 10],
      statementHash: HEX32_G,
      destinationBindingHash: HEX32_H,
      sourceStateVerifierHash: HEX32_D,
      sourceAdapterDeploymentHash: HEX32_A,
      sourceAdapterDeploymentReceiptHash: HEX32_B,
    }).requestHash,
  );
  assert.notEqual(
    buildTonSccpProofRequest({
      publicInputs: sampleTonPublicInputs,
      bundleBytes: [5, 6, 7],
      sourceProofBytes: [9, 10],
      statementHash: HEX32_G,
      destinationBindingHash: HEX32_H,
      sourceStateVerifierHash: HEX32_C,
      sourceAdapterDeploymentHash: HEX32_A,
      sourceAdapterDeploymentReceiptHash: HEX32_B,
    }).requestHash,
    buildTonSccpProofRequest({
      publicInputs: sampleTonPublicInputs,
      bundleBytes: [5, 6, 7, 9],
      sourceProofBytes: [10],
      statementHash: HEX32_G,
      destinationBindingHash: HEX32_H,
      sourceStateVerifierHash: HEX32_C,
      sourceAdapterDeploymentHash: HEX32_A,
      sourceAdapterDeploymentReceiptHash: HEX32_B,
    }).requestHash,
  );
  const validTonProofRequestInput = () => ({
    publicInputs: sampleTonPublicInputs,
    bundleBytes: [5, 6, 7],
    sourceProofBytes: [9, 10],
    statementHash: HEX32_G,
    destinationBindingHash: HEX32_H,
    sourceStateVerifierHash: HEX32_C,
    sourceAdapterDeploymentHash: HEX32_A,
    sourceAdapterDeploymentReceiptHash: HEX32_B,
  });
  assert.throws(
    () =>
      buildTonSccpProofRequest({
        ...validTonProofRequestInput(),
        public_inputs: sampleTonPublicInputs,
      }),
    /publicInputs must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildTonSccpProofRequest({
        ...validTonProofRequestInput(),
        proofContext: {
          statementHash: HEX32_G,
          statement_hash: HEX32_G,
          destinationBindingHash: HEX32_H,
        },
      }),
    /statementHash must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildTonSccpProofRequest({
        ...validTonProofRequestInput(),
        proofContext: {
          statementHash: HEX32_G,
          destinationBindingHash: HEX32_H,
          destinationBinding: { bindingHash: HEX32_A },
        },
      }),
    /destinationBindingHash must match destinationBinding\.bindingHash/,
  );
  assert.throws(
    () =>
      buildTonSccpProofRequest({
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: HEX32_C,
        sourceAdapterDeploymentBinding: {
          sourceAdapterDeploymentHash: HEX32_A,
          source_adapter_deployment_hash: HEX32_A,
          sourceAdapterDeploymentReceiptHash: HEX32_B,
        },
      }),
    /sourceAdapterDeploymentHash must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildTonSccpProofRequest({
        ...validTonProofRequestInput(),
        sourceAdapterDeploymentBinding: {
          sourceAdapterDeploymentHash: HEX32_C,
          sourceAdapterDeploymentReceiptHash: HEX32_B,
        },
      }),
    /sourceAdapterDeploymentHash must match sourceAdapterDeploymentBinding\.sourceAdapterDeploymentHash/,
  );
  assert.throws(
    () =>
      buildTonSccpProofRequest({
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierId: "debug-ton-state-verifier",
        sourceStateVerifierHash: HEX32_C,
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: HEX32_B,
      }),
    /sourceStateVerifierId must match TON shard-state verifier profile/,
  );
  assert.throws(
    () =>
      buildTonSccpProofRequest({
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: SCCP_ZERO_HASH_V1,
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: HEX32_B,
      }),
    /sourceStateVerifierHash must not be zero/,
  );
  assert.throws(
    () =>
      buildTonSccpProofRequest({
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: TON_TEMPLATE_SOURCE_STATE_VERIFIER_HASH,
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: HEX32_B,
      }),
    /TON template verifier hash/,
  );
  assert.throws(
    () =>
      buildTonSccpProofRequest({
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: HEX32_C,
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: SCCP_ZERO_HASH_V1,
      }),
    /must both be zero or both be non-zero/,
  );
  assert.throws(
    () =>
      buildTonSccpProofRequest({
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: HEX32_C,
      }),
    /requires non-zero source adapter deployment binding/,
  );
  assert.throws(
    () =>
      buildTonSccpProofRequest({
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: HEX32_C,
        sourceAdapterDeploymentBinding: {
          sourceDomain: SCCP_DOMAIN_TON,
          targetDomain: SCCP_DOMAIN_TON,
          sourceAdapterDeploymentHash: HEX32_A,
          sourceAdapterDeploymentReceiptHash: HEX32_B,
        },
      }),
    /deployment binding targetDomain must be SORA/,
  );
  assert.throws(
    () =>
      buildTonSccpProofRequest({
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: HEX32_C,
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: HEX32_B,
        sourceAdapterDeploymentBinding: false,
      }),
    /sourceAdapterDeploymentBinding must be an object/,
  );
  assert.throws(
    () =>
      buildTonSccpProofRequest({
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: HEX32_C,
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: HEX32_B,
      }),
    /bundleBytes must not be empty/,
  );
  assert.throws(
    () =>
      buildTonSccpProofRequest({
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: HEX32_C,
        sourceDomain: SCCP_DOMAIN_SOL,
      }),
    /sourceDomain must be TON/,
  );
  assert.throws(
    () =>
      buildTonSccpProofRequest({
        publicInputs: { ...sampleTonPublicInputs, targetDomain: SCCP_DOMAIN_SOL },
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: HEX32_C,
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: HEX32_B,
      }),
    /publicInputs\.targetDomain must be TON/,
  );
  assert.throws(
    () =>
      buildTonSccpProofRequest({
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        backend: "debug-ton-backend",
        sourceStateVerifierHash: HEX32_C,
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: HEX32_B,
      }),
    /backend must be ton-contract-v1/,
  );
  assert.throws(
    () =>
      buildTonSccpProofRequest({
        publicInputs: { ...sampleTonPublicInputs, payloadHash: `${HEX32_E} ` },
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: HEX32_C,
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: HEX32_B,
      }),
    /publicInputs\.payloadHash must be canonical hex/,
  );
  assert.throws(
    () =>
      buildTonSccpProofRequest({
        publicInputs: { ...sampleTonPublicInputs, finalityHeight: "019" },
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: HEX32_C,
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: HEX32_B,
      }),
    /publicInputs\.finalityHeight must be an unsigned integer/,
  );
  assert.throws(
    () =>
      buildTonSccpProofRequest({
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: ` ${HEX32_G}`,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: HEX32_C,
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: HEX32_B,
      }),
    /statementHash must be canonical hex/,
  );
  assert.throws(
    () =>
      buildTonSccpProofRequest({
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: HEX32_C,
        sourceAdapterDeploymentHash: `${HEX32_A} `,
        sourceAdapterDeploymentReceiptHash: HEX32_B,
      }),
    /sourceAdapterDeploymentHash must be canonical hex/,
  );
  assert.throws(
    () =>
      buildTonSccpProofRequest({
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: HEX32_C,
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: ` ${HEX32_B}`,
      }),
    /sourceAdapterDeploymentReceiptHash must be canonical hex/,
  );

  const exposedPublicInputs = request.publicInputsBytes;
  const exposedBundle = request.bundleBytes;
  const exposedSourceProof = request.sourceProofBytes;
  exposedPublicInputs[0] = 99;
  exposedBundle[0] = 99;
  exposedSourceProof[0] = 99;
  assert.notEqual(request.publicInputsBytes[0], 99);
  assert.deepEqual(Array.from(request.bundleBytes), [5, 6, 7]);
  assert.deepEqual(Array.from(request.sourceProofBytes), [9, 10]);
});

test("matches TON proof request hash golden vector across SDKs", () => {
  const publicInputs = {
    version: 1,
    messageId: `0x${"11".repeat(32)}`,
    payloadHash: `0x${"22".repeat(32)}`,
    targetDomain: SCCP_DOMAIN_TON,
    commitmentRoot: `0x${"33".repeat(32)}`,
    finalityHeight: 123456789n,
    finalityBlockHash: `0x${"44".repeat(32)}`,
  };
  const request = buildTonSccpProofRequest({
    publicInputs,
    bundleBytes: [1, 2, 3, 4, 5, 6, 7, 8, 9],
    sourceProofBytes: [0x51, 0x52, 0x53],
    statementHash: `0x${"55".repeat(32)}`,
    destinationBindingHash: `0x${"66".repeat(32)}`,
    sourceStateVerifierHash: `0x${"42".repeat(32)}`,
    sourceAdapterDeploymentBinding: {
      sourceDomain: SCCP_DOMAIN_TON,
      targetDomain: SCCP_DOMAIN_SORA,
      sourceAdapterDeploymentHash: HEX32_A,
      sourceAdapterDeploymentReceiptHash: HEX32_B,
    },
  });

  assert.equal(
    `0x${Buffer.from(canonicalSccpMessageTransparentPublicInputsBytes(publicInputs)).toString("hex")}`,
    "0x011111111111111111111111111111111111111111111111111111111111111111222222222222222222222222222222222222222222222222222222222222222204000000333333333333333333333333333333333333333333333333333333333333333315cd5b07000000004444444444444444444444444444444444444444444444444444444444444444",
  );
  assert.equal(
    request.sourceAdapterDeploymentBindingHash,
    "0x7d35b186e3d49aed31693e33d33355fa8fa9032160c929f2c7fe260094f6ccdf",
  );
  assert.equal(
    request.requestHash,
    "0xb3a61f09923efd639a0263de6b45eec6ddd5de679bfaab1b6ec1c591fd1b1d1b",
  );

  const proofResult = wrapTonSccpProofResult([0x91, 0x92, 0x93, 0x94, 0x95], request);
  assert.equal(
    proofResult.envelopeHash,
    "0xa2bc6697b237fd4b2dd3f60f187a184793104a99372dcdf60c7ec585ef32f5ab",
  );
});

test("rejects non-empty all-zero source proof bytes in SCCP proof requests", () => {
  const zeroSourceProofBytes = Uint8Array.from([0, 0, 0]);

  assert.throws(
    () =>
      buildEvmSccpProofRequest({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: zeroSourceProofBytes,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /sourceProofBytes must not be all zero/,
  );
  assert.throws(
    () =>
      buildTronSccpProofRequest({
        publicInputs: sampleTronPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: zeroSourceProofBytes,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /sourceProofBytes must not be all zero/,
  );
  assert.throws(
    () =>
      buildSubstrateSccpProofRequest({
        publicInputs: sampleSubstratePublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: zeroSourceProofBytes,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /sourceProofBytes must not be all zero/,
  );
  assert.throws(
    () =>
      buildTonSccpProofRequest({
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: zeroSourceProofBytes,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: HEX32_C,
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: HEX32_B,
      }),
    /sourceProofBytes must not be all zero/,
  );

  assert.equal(
    buildEvmSccpProofRequest({
      publicInputs: sampleEvmPublicInputs,
      bundleBytes: [5, 6, 7],
      statementHash: HEX32_G,
      destinationBindingHash: HEX32_H,
    }).sourceProofBytes.length,
    0,
  );
  assert.equal(
    buildTronSccpProofRequest({
      publicInputs: sampleTronPublicInputs,
      bundleBytes: [5, 6, 7],
      statementHash: HEX32_G,
      destinationBindingHash: HEX32_H,
    }).sourceProofBytes.length,
    0,
  );
  assert.equal(
    buildSubstrateSccpProofRequest({
      publicInputs: sampleSubstratePublicInputs,
      bundleBytes: [5, 6, 7],
      statementHash: HEX32_G,
      destinationBindingHash: HEX32_H,
    }).sourceProofBytes.length,
    0,
  );
  assert.equal(
    buildTonSccpProofRequest({
      publicInputs: sampleTonPublicInputs,
      bundleBytes: [5, 6, 7],
      statementHash: HEX32_G,
      destinationBindingHash: HEX32_H,
      sourceStateVerifierHash: HEX32_C,
      sourceAdapterDeploymentHash: HEX32_A,
      sourceAdapterDeploymentReceiptHash: HEX32_B,
    }).sourceProofBytes.length,
    0,
  );

  for (const badSourceProofBytes of [false, 0, ""]) {
    assert.throws(
      () =>
        buildTonSccpProofRequest({
          publicInputs: sampleTonPublicInputs,
          bundleBytes: [5, 6, 7],
          sourceProofBytes: badSourceProofBytes,
          statementHash: HEX32_G,
          destinationBindingHash: HEX32_H,
          sourceStateVerifierHash: HEX32_C,
          sourceAdapterDeploymentHash: HEX32_A,
          sourceAdapterDeploymentReceiptHash: HEX32_B,
        }),
      /sourceProofBytes must be bytes or hex|sourceProofBytes must be canonical hex/,
    );
  }
});

test("derives TON SCCP shard proof hashes from branch witness material", () => {
  const input = {
    sourceEventDigest: `0x${"34".repeat(32)}`,
    masterchainSeqno: 19n,
    masterchainBlockHash: HEX32_A,
    shardWorkchainId: 0,
    shardShard: 0x8000000000000000n,
    shardSeqno: 7n,
    shardBlockHash: HEX32_B,
    shardFileHash: `0x${"bc".repeat(32)}`,
    shardStateRoot: HEX32_C,
    transactionRoot: HEX32_D,
    transactionLt: 7n,
    shardStateLeafIndex: 0n,
    shardStateInclusionBranch: [HEX32_F],
    inclusionBranch: [HEX32_E],
  };

  const bytes = canonicalTonSccpShardProofBytes(input);
  assert.equal(bytes.length, 309);
  assert.equal(bytes[0], 1);

  const hash = tonSccpShardProofHash(input);
  assert.equal(hash, "0x09c63ca1185b537f0a37b7b248600a0992e5b7ed64ace9d1d437db7caae00686");
  assert.notEqual(hash, tonSccpShardProofHash({ ...input, shardStateInclusionBranch: [HEX32_E] }));
  assert.notEqual(hash, tonSccpShardProofHash({ ...input, inclusionBranch: [HEX32_F] }));
  assert.throws(
    () => canonicalTonSccpShardProofBytes({ ...input, masterchain_seqno: input.masterchainSeqno }),
    /masterchainSeqno must not use multiple aliases/,
  );
  assert.throws(
    () => canonicalTonSccpShardProofBytes({ ...input, receiptOrMessageRoot: input.transactionRoot }),
    /transactionRoot must not use multiple aliases/,
  );
  assert.throws(
    () => canonicalTonSccpShardProofBytes({ ...input, inclusion_branch: input.inclusionBranch }),
    /inclusionBranch must not use multiple aliases/,
  );

  const dictionaryInput = {
    ...input,
    shardStateRoot: TON_SHARD_STATE_ROOT_HASH,
    transactionRoot: TON_HASHMAP_E_VALUE_HASH,
    shardStateProofBoc: Buffer.from(TON_SHARD_STATE_PROOF_BOC_HEX, "hex"),
    shardStateDictionaryRoot: TON_SHARD_ACCOUNTS_ROOT_HASH,
    shardStateDictionaryKeyBitLen: 256,
    shardStateDictionaryKey: TON_SHARD_ACCOUNT_KEY,
    shardStateDictionaryProofBoc: Buffer.from(TON_SHARD_ACCOUNTS_BOC_HEX, "hex"),
    shardStateInclusionBranch: [],
  };
  assert.equal(
    JSON.stringify(
      tonShardAccountsLastTransaction(
        dictionaryInput.shardStateDictionaryProofBoc,
        dictionaryInput.shardStateDictionaryKey,
        dictionaryInput.shardStateDictionaryKeyBitLen,
      ),
      (_, value) => (typeof value === "bigint" ? value.toString() : value),
    ),
    JSON.stringify({ hash: TON_HASHMAP_E_VALUE_HASH, lt: "7" }),
  );
  assert.equal(
    tonShardAccountsLastTransactionHash(
      dictionaryInput.shardStateDictionaryProofBoc,
      dictionaryInput.shardStateDictionaryKey,
      dictionaryInput.shardStateDictionaryKeyBitLen,
    ),
    TON_HASHMAP_E_VALUE_HASH,
  );
  const dictionaryBytes = canonicalTonSccpShardProofBytes(dictionaryInput);
  const dictionaryHash = tonSccpShardProofHash(dictionaryInput);
  assert.equal(dictionaryBytes.length, 662);
  assert.equal(dictionaryHash, "0x32d8b496320e6a1ce5ccf671f2bd6f0d09cb53afed8c123b86cb9327b77c88cf");
  assert.notEqual(dictionaryHash, hash);
  const shardStateSourceStateInput = {
    sourceDomain: 4,
    masterchainSeqno: 19n,
    masterchainWorkchainId: -1,
    masterchainShard: 0x8000000000000000n,
    masterchainBlockHash: HEX32_A,
    masterchainFileHash: `0x${"a5".repeat(32)}`,
    validatorSetHash: TON_VALIDATOR_SET_HASH,
    masterchainConfigRoot: TON_MASTERCHAIN_CONFIG_ROOT,
    masterchainConfigProofHash: TON_SHARD_STATE_MASTERCHAIN_CONFIG_PROOF_HASH,
    shardWorkchainId: 0,
    shardShard: 0x8000000000000000n,
    shardSeqno: 7n,
    shardBlockHash: HEX32_B,
    shardFileHash: `0x${"bc".repeat(32)}`,
    shardStateRoot: TON_SHARD_STATE_ROOT_HASH,
    transactionRoot: TON_HASHMAP_E_VALUE_HASH,
    transactionLt: 7n,
    shardStateProofBoc: Buffer.from(TON_SHARD_STATE_PROOF_BOC_HEX, "hex"),
    shardStateDictionaryRoot: TON_SHARD_ACCOUNTS_ROOT_HASH,
    shardStateDictionaryKeyBitLen: 256,
    shardStateDictionaryKey: TON_SHARD_ACCOUNT_KEY,
    shardStateDictionaryProofBoc: Buffer.from(TON_SHARD_ACCOUNTS_BOC_HEX, "hex"),
    masterchainSignatureHash: TON_MASTERCHAIN_SIGNATURES_HASH,
    shardProofHash: dictionaryHash,
    configDictionaryProofBoc: Buffer.from(TON_MASTERCHAIN_CONFIG_PROOF_BOC_HEX, "hex"),
    sourceStateVerifierHash: `0x${"d4".repeat(32)}`,
    sourceTrustAnchorHash: TON_VALIDATOR_SET_HASH,
    consensusVerifierHash: `0x${"b2".repeat(32)}`,
    messageInclusionVerifierHash: `0x${"c3".repeat(32)}`,
    finalityPolicyHash: `0x${"c4".repeat(32)}`,
  };
  assert.equal(canonicalTonShardStateProofPublicInputsBytes(shardStateSourceStateInput).length, 603);
  assert.equal(
    tonShardStateProofPublicInputsHash(shardStateSourceStateInput),
    "0x82bdedb87242c4bb073b7c97cb339b7f1300e3692e327c5bc8233bd105cafb19",
  );
  assert.equal(canonicalTonShardStateWitnessCommitmentBytes(shardStateSourceStateInput).length, 480);
  assert.equal(canonicalTonShardStateVerificationContextBytes(shardStateSourceStateInput).length, 467);
  assert.equal(tonShardStateOpenVerifySchemaDescriptor(shardStateSourceStateInput).length, 436);
  const request = buildTonShardStateProofRequest(shardStateSourceStateInput);
  assertImmutableFastpqProofRequest(request, [
    "statementBytes",
    "witnessCommitmentBytes",
    "verificationContextBytes",
    "schemaDescriptor",
  ]);
  assert.equal(request.circuitId, "sccp-ton-shard-state-light-client-v1");
  assert.deepEqual(request.fastpqPublicInputs, {
    dsid: "0x27e44edc7d124906a8176e94557996c3",
    slot: "19",
    oldRoot: TON_MASTERCHAIN_CONFIG_ROOT,
    newRoot: TON_SHARD_STATE_ROOT_HASH,
    permRoot: TON_SHARD_ACCOUNTS_ROOT_HASH,
    txSetHash: "0x82bdedb87242c4bb073b7c97cb339b7f1300e3692e327c5bc8233bd105cafb19",
  });
  assert.deepEqual(
    request.fastpqTransitions.map((transition) => transition.key),
    [
      "sccp:ton:shard-state:v1:statement",
      "sccp:ton:shard-state:v1:witness",
      "sccp:ton:shard-state:v1:context",
    ],
  );
  assert.throws(
    () =>
      buildTonShardStateProofRequest({
        ...shardStateSourceStateInput,
        masterchain_seqno: shardStateSourceStateInput.masterchainSeqno,
      }),
    /masterchainSeqno must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalTonShardStateProofPublicInputsBytes({
        ...shardStateSourceStateInput,
        shard_state_dictionary_proof_boc: shardStateSourceStateInput.shardStateDictionaryProofBoc,
      }),
    /shardStateDictionaryProofBoc must not use multiple aliases/,
  );
  assert.throws(
    () =>
      buildTonShardStateProofRequest({
        ...shardStateSourceStateInput,
        masterchainConfigProof: {
          configDictionaryProofBoc: shardStateSourceStateInput.configDictionaryProofBoc,
        },
      }),
    /configDictionaryProofBoc must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalTonShardStateWitnessCommitmentBytes({
        ...shardStateSourceStateInput,
        source_state_verifier_hash: shardStateSourceStateInput.sourceStateVerifierHash,
      }),
    /sourceStateVerifierHash must not use multiple aliases/,
  );
  const transitionProof = {
    version: 1,
    sourceDomain: SCCP_DOMAIN_TON,
    fromValidatorSetSeqno: 7n,
    toValidatorSetSeqno: 8n,
    masterchainSeqno: 19n,
    masterchainWorkchainId: -1,
    masterchainShard: 0x8000000000000000n,
    masterchainBlockHash: HEX32_A,
    masterchainFileHash: `0x${"a5".repeat(32)}`,
    parentValidatorSetHash: TON_VALIDATOR_SET_HASH,
    nextValidatorSetHash: TON_NEXT_VALIDATOR_SET_HASH,
    nextValidatorSetPayload: Buffer.from(TON_NEXT_VALIDATOR_SET_PAYLOAD_HEX, "hex"),
    nextValidatorSetPayloadHash: TON_NEXT_VALIDATOR_SET_PAYLOAD_HASH,
    nextValidatorSetConfigHash: HEX32_C,
    transitionMessageHash: TON_VALIDATOR_SET_TRANSITION_MESSAGE_HASH,
    transitionSignatureHash: TON_VALIDATOR_SET_TRANSITION_SIGNATURE_HASH,
    validatorSignatureProof: {
      version: 1,
      totalWeight: 3n,
      signedWeight: 3n,
      blockMessageHash: TON_VALIDATOR_SET_TRANSITION_MESSAGE_HASH,
      validatorPublicKeys: [`0x${"11".repeat(32)}`, `0x${"22".repeat(32)}`],
      validatorWeights: [1n, 2n],
      signersBitmap: [0x03],
      signatures: [new Uint8Array(64).fill(0xab), new Uint8Array(64).fill(0xcd)],
    },
  };
  const transitionBoundInput = {
    ...shardStateSourceStateInput,
    validatorSetTransitionProofs: [transitionProof],
  };
  const tamperedTransitionSignature = Uint8Array.from(
    transitionProof.validatorSignatureProof.signatures[0],
  );
  tamperedTransitionSignature[0] ^= 0x01;
  assert.throws(
    () => canonicalTonShardStateProofPublicInputsBytes({
      ...transitionBoundInput,
      validatorSetTransitionProofs: [
        {
          ...transitionProof,
          validatorSignatureProof: {
            ...transitionProof.validatorSignatureProof,
            signatures: [
              tamperedTransitionSignature,
              transitionProof.validatorSignatureProof.signatures[1],
            ],
          },
        },
      ],
    }),
    /transitionSignatureHash/,
  );
  assert.throws(
    () =>
      buildTonShardStateProofRequest({
        ...shardStateSourceStateInput,
        sourceStateVerifierHash: TON_TEMPLATE_SOURCE_STATE_VERIFIER_HASH,
      }),
    /TON template verifier hash/,
  );
  assert.deepEqual(tonShardStatePublicInputColumns(shardStateSourceStateInput)[15], [
    "0x82bdedb87242c4bb073b7c97cb339b7f1300e3692e327c5bc8233bd105cafb19",
  ]);
  assert.throws(
    () => tonShardStateProofPublicInputsHash({ ...shardStateSourceStateInput, transactionRoot: HEX32_C }),
    /last transaction hash/,
  );
  assert.throws(
    () => canonicalTonSccpShardProofBytes({ ...dictionaryInput, shardStateInclusionBranch: [HEX32_F] }),
    /must be empty/,
  );
  assert.throws(
    () => canonicalTonSccpShardProofBytes({ ...dictionaryInput, shardStateProofBoc: new Uint8Array() }),
    /must not be empty/,
  );
  assert.throws(
    () => canonicalTonSccpShardProofBytes({ ...dictionaryInput, shardStateRoot: `0x${"66".repeat(32)}` }),
    /root must match shardStateRoot/,
  );
  assert.throws(
    () => canonicalTonSccpShardProofBytes({ ...dictionaryInput, transactionRoot: `0x${"66".repeat(32)}` }),
    /ShardAccount last transaction hash must match transactionRoot/,
  );
  assert.throws(
    () => canonicalTonSccpShardProofBytes({ ...dictionaryInput, transactionLt: 8n }),
    /ShardAccount last transaction lt must match transactionLt/,
  );
  assert.throws(
    () =>
      canonicalTonSccpShardProofBytes({
        ...dictionaryInput,
        shardStateDictionaryRoot: `0x${"66".repeat(32)}`,
      }),
    /accounts root must match shardStateDictionaryRoot/,
  );
  assert.throws(
    () =>
      canonicalTonSccpShardProofBytes({
        ...dictionaryInput,
        shardStateDictionaryRoot: `0x${"00".repeat(32)}`,
      }),
    /must not be zero/,
  );
  const wrongGlobalIdProofBoc = Buffer.from(TON_SHARD_STATE_PROOF_BOC_HEX, "hex");
  const wrongGlobalIdTagOffset = wrongGlobalIdProofBoc.indexOf(Buffer.from([0x90, 0x23, 0xaf, 0xe2]));
  assert.notEqual(wrongGlobalIdTagOffset, -1);
  wrongGlobalIdProofBoc.fill(0, wrongGlobalIdTagOffset + 4, wrongGlobalIdTagOffset + 8);
  assert.equal(tonShardStateAccountsRootHash(wrongGlobalIdProofBoc), TON_SHARD_ACCOUNTS_ROOT_HASH);
  assert.throws(
    () =>
      canonicalTonSccpShardProofBytes({
        ...dictionaryInput,
        shardStateRoot: tonShardStateProofRootHash(wrongGlobalIdProofBoc),
        shardStateProofBoc: wrongGlobalIdProofBoc,
      }),
    /global_id/,
  );
  const wrongWorkchainIdProofBoc = Buffer.from(TON_SHARD_STATE_PROOF_BOC_HEX, "hex");
  const wrongWorkchainIdTagOffset = wrongWorkchainIdProofBoc.indexOf(Buffer.from([0x90, 0x23, 0xaf, 0xe2]));
  assert.notEqual(wrongWorkchainIdTagOffset, -1);
  const wrongWorkchainShardIdentOffset = wrongWorkchainIdTagOffset + 8;
  wrongWorkchainIdProofBoc.fill(0xff, wrongWorkchainShardIdentOffset + 1, wrongWorkchainShardIdentOffset + 5);
  assert.equal(tonShardStateAccountsRootHash(wrongWorkchainIdProofBoc), TON_SHARD_ACCOUNTS_ROOT_HASH);
  assert.throws(
    () =>
      canonicalTonSccpShardProofBytes({
        ...dictionaryInput,
        shardStateRoot: tonShardStateProofRootHash(wrongWorkchainIdProofBoc),
        shardStateProofBoc: wrongWorkchainIdProofBoc,
      }),
    /workchain_id/,
  );
  const zeroGenUtimeProofBoc = Buffer.from(TON_SHARD_STATE_PROOF_BOC_HEX, "hex");
  const zeroGenUtimeTagOffset = zeroGenUtimeProofBoc.indexOf(Buffer.from([0x90, 0x23, 0xaf, 0xe2]));
  assert.notEqual(zeroGenUtimeTagOffset, -1);
  zeroGenUtimeProofBoc.fill(0, zeroGenUtimeTagOffset + 29, zeroGenUtimeTagOffset + 33);
  assert.throws(
    () =>
      canonicalTonSccpShardProofBytes({
        ...dictionaryInput,
        shardStateRoot: tonShardStateProofRootHash(zeroGenUtimeProofBoc),
        shardStateProofBoc: zeroGenUtimeProofBoc,
      }),
    /gen_utime/,
  );
  const futureMinRefMcSeqnoProofBoc = Buffer.from(TON_SHARD_STATE_PROOF_BOC_HEX, "hex");
  const futureMinRefMcSeqnoTagOffset = futureMinRefMcSeqnoProofBoc.indexOf(
    Buffer.from([0x90, 0x23, 0xaf, 0xe2]),
  );
  assert.notEqual(futureMinRefMcSeqnoTagOffset, -1);
  futureMinRefMcSeqnoProofBoc[futureMinRefMcSeqnoTagOffset + 44] = 0x14;
  assert.throws(
    () =>
      canonicalTonSccpShardProofBytes({
        ...dictionaryInput,
        shardStateRoot: tonShardStateProofRootHash(futureMinRefMcSeqnoProofBoc),
        shardStateProofBoc: futureMinRefMcSeqnoProofBoc,
      }),
    /min_ref_mc_seqno/,
  );
  const mismatchedShardPrefixProofBoc = Buffer.from(TON_SHARD_STATE_PROOF_BOC_HEX, "hex");
  const shardStateTagOffset = mismatchedShardPrefixProofBoc.indexOf(Buffer.from([0x90, 0x23, 0xaf, 0xe2]));
  assert.notEqual(shardStateTagOffset, -1);
  const shardIdentOffset = shardStateTagOffset + 8;
  mismatchedShardPrefixProofBoc[shardIdentOffset] = 0x08;
  mismatchedShardPrefixProofBoc[shardIdentOffset + 5] = 0x12;
  assert.equal(tonShardStateAccountsRootHash(mismatchedShardPrefixProofBoc), TON_SHARD_ACCOUNTS_ROOT_HASH);
  assert.throws(
    () =>
      canonicalTonSccpShardProofBytes({
        ...dictionaryInput,
        shardStateRoot: tonShardStateProofRootHash(mismatchedShardPrefixProofBoc),
        shardStateProofBoc: mismatchedShardPrefixProofBoc,
        shardShard: 0x1280000000000000n,
      }),
    /ShardIdent prefix/,
  );
  assert.throws(
    () =>
      canonicalTonSccpShardProofBytes({
        ...dictionaryInput,
        shardStateDictionaryKeyBitLen: 7,
        shardStateDictionaryKey: Uint8Array.from([17]),
      }),
    /key bit length must be 256/,
  );
  assert.throws(
    () => canonicalTonSccpShardProofBytes({ ...input, inclusionBranch: [Uint8Array.from([1, 2, 3])] }),
    /must be 32 bytes/,
  );
  assert.throws(
    () =>
      canonicalTonSccpShardProofBytes({
        ...input,
        inclusionBranch: Array.from({ length: 65 }, () => HEX32_E),
      }),
    /at most 64/,
  );
  assert.throws(
    () =>
      canonicalTonSccpShardProofBytes({
        ...input,
        shardStateInclusionBranch: [Uint8Array.from([1, 2, 3])],
      }),
    /must be 32 bytes/,
  );
});

test("derives TON validator-set transition transcript hashes from UI witness material", () => {
  const validatorSet = {
    validatorPublicKeys: [`0x${"11".repeat(32)}`, `0x${"22".repeat(32)}`],
    validatorWeights: [1n, 2n],
  };
  const nextValidatorSet = {
    validatorPublicKeys: [`0x${"33".repeat(32)}`, `0x${"44".repeat(32)}`],
    validatorWeights: [3n, 4n],
  };
  const nextValidatorSetPayload = canonicalTonValidatorSetPayloadBytes(nextValidatorSet);
  const transitionMessage = {
    sourceDomain: SCCP_DOMAIN_TON,
    fromValidatorSetSeqno: 7n,
    toValidatorSetSeqno: 8n,
    masterchainSeqno: 19n,
    masterchainWorkchainId: -1,
    masterchainShard: 0x8000000000000000n,
    masterchainBlockHash: HEX32_A,
    masterchainFileHash: `0x${"a5".repeat(32)}`,
    parentValidatorSetHash: TON_VALIDATOR_SET_HASH,
    nextValidatorSetHash: TON_NEXT_VALIDATOR_SET_HASH,
    nextValidatorSetPayload,
    nextValidatorSetPayloadHash: TON_NEXT_VALIDATOR_SET_PAYLOAD_HASH,
    nextValidatorSetConfigHash: HEX32_C,
  };
  const transitionSignature = {
    version: 1,
    ...transitionMessage,
    transitionMessageHash: TON_VALIDATOR_SET_TRANSITION_MESSAGE_HASH,
    validatorSignatureProof: {
      version: 1,
      totalWeight: 3n,
      signedWeight: 3n,
      blockMessageHash: TON_VALIDATOR_SET_TRANSITION_MESSAGE_HASH,
      ...validatorSet,
      signersBitmap: [0x03],
      signatures: [new Uint8Array(64).fill(0xab), new Uint8Array(64).fill(0xcd)],
    },
  };

  assert.equal(canonicalTonValidatorSetBytes(validatorSet).length, 85);
  assert.equal(tonValidatorSetHash(validatorSet), TON_VALIDATOR_SET_HASH);
  assert.equal(Buffer.from(nextValidatorSetPayload).toString("hex"), TON_NEXT_VALIDATOR_SET_PAYLOAD_HEX);
  assert.equal(tonValidatorSetPayloadHash(nextValidatorSetPayload), TON_NEXT_VALIDATOR_SET_PAYLOAD_HASH);
  assert.equal(tonValidatorSetHashFromPayload(nextValidatorSetPayload), TON_NEXT_VALIDATOR_SET_HASH);
  assert.equal(canonicalTonValidatorSetTransitionMessageBytes(transitionMessage).length, 233);
  assert.equal(
    tonValidatorSetTransitionMessageHash(transitionMessage),
    TON_VALIDATOR_SET_TRANSITION_MESSAGE_HASH,
  );
  assert.equal(canonicalTonValidatorSetTransitionSignatureBytes(transitionSignature).length, 676);
  assert.equal(
    tonValidatorSetTransitionSignatureHash(transitionSignature),
    TON_VALIDATOR_SET_TRANSITION_SIGNATURE_HASH,
  );
  assert.throws(
    () =>
      canonicalTonValidatorSetBytes({
        ...validatorSet,
        validator_public_keys: validatorSet.validatorPublicKeys,
      }),
    /validatorPublicKeys must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalTonValidatorSetTransitionMessageBytes({
        ...transitionMessage,
        masterchain_seqno: transitionMessage.masterchainSeqno,
      }),
    /masterchainSeqno must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalTonValidatorSetTransitionSignatureBytes({
        ...transitionSignature,
        next_validator_set_payload_hash: TON_NEXT_VALIDATOR_SET_PAYLOAD_HASH,
      }),
    /nextValidatorSetPayloadHash must not use multiple aliases/,
  );
  assert.throws(
    () =>
      canonicalTonValidatorSetTransitionSignatureBytes({
        ...transitionSignature,
        validatorSignatureProof: {
          ...transitionSignature.validatorSignatureProof,
          total_weight: 3n,
        },
      }),
    /totalWeight must not use multiple aliases/,
  );
  assert.throws(
    () => canonicalTonValidatorSetTransitionMessageBytes({ ...transitionMessage, version: 0 }),
    /TON validator-set transition version/,
  );
  assert.throws(
    () => canonicalTonValidatorSetTransitionSignatureBytes({ ...transitionSignature, version: 0 }),
    /TON validator-set transition proof version/,
  );
  assert.throws(
    () =>
      canonicalTonValidatorSetTransitionSignatureBytes({
        ...transitionSignature,
        validatorSignatureProof: {
          ...transitionSignature.validatorSignatureProof,
          version: 0,
        },
      }),
    /TON validator signature proof version/,
  );
  assert.notEqual(
    tonValidatorSetTransitionMessageHash({
      ...transitionMessage,
      nextValidatorSetConfigHash: HEX32_D,
    }),
    TON_VALIDATOR_SET_TRANSITION_MESSAGE_HASH,
  );
  assert.throws(
    () =>
      canonicalTonValidatorSetTransitionSignatureBytes({
        ...transitionSignature,
        parentValidatorSetHash: HEX32_D,
      }),
    /parentValidatorSetHash/,
  );
  assert.throws(
    () =>
      canonicalTonValidatorSetTransitionSignatureBytes({
        ...transitionSignature,
        transitionMessageHash: HEX32_D,
      }),
    /transitionMessageHash/,
  );
  assert.throws(
    () =>
      canonicalTonValidatorSetTransitionMessageBytes({
        ...transitionMessage,
        toValidatorSetSeqno: 9n,
      }),
    /toValidatorSetSeqno/,
  );
  assert.throws(
    () =>
      canonicalTonValidatorSetTransitionSignatureBytes({
        ...transitionSignature,
        validatorSignatureProof: {
          ...transitionSignature.validatorSignatureProof,
          blockMessageHash: HEX32_D,
        },
      }),
    /blockMessageHash/,
  );
  assert.throws(
    () => canonicalTonValidatorSetBytes({ ...validatorSet, validatorWeights: [1n, 0n] }),
    /must not be zero/,
  );
  assert.throws(
    () =>
      canonicalTonValidatorSetBytes({
        ...validatorSet,
        validatorPublicKeys: [new Uint8Array(32), validatorSet.validatorPublicKeys[1]],
      }),
    /must not be zero/,
  );
  const zeroKeyValidatorSetPayload = canonicalTonValidatorSetPayloadBytes(validatorSet);
  zeroKeyValidatorSetPayload.fill(0, 5, 37);
  assert.throws(
    () => tonValidatorSetHashFromPayload(zeroKeyValidatorSetPayload),
    /must not be zero/,
  );
  const oversizedTonValidatorSet = {
    validatorPublicKeys: Array.from({ length: 1025 }, (_, index) => {
      const publicKey = new Uint8Array(32);
      publicKey[0] = 0x80;
      new DataView(publicKey.buffer).setUint32(28, index, true);
      return publicKey;
    }),
    validatorWeights: Array.from({ length: 1025 }, () => 1n),
  };
  assert.throws(
    () => canonicalTonValidatorSetBytes(oversizedTonValidatorSet),
    /1..1024/,
  );
  const oversizedTonValidatorSetPayload = new Uint8Array(5 + 1025 * 40);
  const oversizedTonValidatorSetPayloadView = new DataView(oversizedTonValidatorSetPayload.buffer);
  oversizedTonValidatorSetPayloadView.setUint8(0, 1);
  oversizedTonValidatorSetPayloadView.setUint32(1, 1025, true);
  for (let index = 0; index < 1025; index += 1) {
    const offset = 5 + index * 40;
    oversizedTonValidatorSetPayload[offset] = 0x80;
    oversizedTonValidatorSetPayloadView.setUint32(offset + 28, index, true);
    oversizedTonValidatorSetPayloadView.setBigUint64(offset + 32, 1n, true);
  }
  assert.throws(
    () => tonValidatorSetHashFromPayload(oversizedTonValidatorSetPayload),
    /validator count/,
  );
  assert.throws(
    () =>
      canonicalTonValidatorSetTransitionSignatureBytes({
        ...transitionSignature,
        validatorSignatureProof: {
          ...transitionSignature.validatorSignatureProof,
          signatures: [new Uint8Array(64)],
        },
      }),
    /signatures length/,
  );
  assert.throws(
    () =>
      canonicalTonValidatorSetTransitionSignatureBytes({
        ...transitionSignature,
        validatorSignatureProof: {
          ...transitionSignature.validatorSignatureProof,
          signedWeight: 1n,
          signersBitmap: [0x01],
          signatures: [new Uint8Array(64)],
        },
      }),
    /greater than two thirds/,
  );
  assert.throws(
    () =>
      canonicalTonValidatorSetTransitionSignatureBytes({
        ...transitionSignature,
        validatorSignatureProof: {
          ...transitionSignature.validatorSignatureProof,
          signatures: [new Uint8Array(63), new Uint8Array(64)],
        },
      }),
    /must be 64 bytes/,
  );
  assert.throws(
    () =>
      canonicalTonValidatorSetTransitionSignatureBytes({
        ...transitionSignature,
        validatorSignatureProof: {
          ...transitionSignature.validatorSignatureProof,
          signatures: [new Uint8Array(64), new Uint8Array(64).fill(0x01)],
        },
      }),
    /must not be all zero/,
  );
  assert.throws(
    () =>
      canonicalTonValidatorSetTransitionSignatureBytes({
        ...transitionSignature,
        nextValidatorSetHash: HEX32_B,
      }),
    /nextValidatorSetHash/,
  );
});

test("derives ETH sync-committee transition transcript hashes from UI witness material", () => {
  const parent = {
    syncCommitteePublicKeys: [`0x${"11".repeat(48)}`, `0x${"22".repeat(48)}`],
    syncCommitteeWeights: [1n, 2n],
    syncCommitteePops: [`0x${"aa".repeat(96)}`, `0x${"bb".repeat(96)}`],
  };
  const nextCommittee = {
    syncCommitteePublicKeys: [`0x${"33".repeat(48)}`, `0x${"44".repeat(48)}`],
    syncCommitteeWeights: [3n, 4n],
    syncCommitteePops: [`0x${"cc".repeat(96)}`, `0x${"dd".repeat(96)}`],
  };
  const nextSyncCommitteePayload = canonicalEthSyncCommitteePayloadBytes(nextCommittee);
  const transitionMessage = {
    sourceDomain: SCCP_DOMAIN_ETH,
    fromSyncPeriod: 7n,
    toSyncPeriod: 8n,
    transitionSlot: 19n,
    finalizedBeaconRoot: HEX32_A,
    parentSyncCommitteeHash: ETH_SYNC_COMMITTEE_HASH,
    nextSyncCommitteeHash: ETH_NEXT_SYNC_COMMITTEE_HASH,
    nextSyncCommitteePayloadHash: ETH_NEXT_SYNC_COMMITTEE_PAYLOAD_HASH,
    nextSyncCommitteeBranchHash: `0x${"be".repeat(32)}`,
  };
  const transitionSignature = {
    version: 1,
    ...transitionMessage,
    nextSyncCommitteePayload,
    transitionMessageHash: ETH_SYNC_COMMITTEE_TRANSITION_MESSAGE_HASH,
    syncCommitteeProof: {
      version: 1,
      totalWeight: 3n,
      signedWeight: 3n,
      syncCommitteeMessageHash: ETH_SYNC_COMMITTEE_TRANSITION_MESSAGE_HASH,
      ...parent,
      signersBitmap: [0x03],
      aggregateSignature: new Uint8Array(96).fill(0xee),
    },
  };

  assert.equal(ethSyncCommitteeHash(parent), ETH_SYNC_COMMITTEE_HASH);
  assert.equal(Buffer.from(nextSyncCommitteePayload).toString("hex"), ETH_NEXT_SYNC_COMMITTEE_PAYLOAD_HEX);
  assert.equal(ethSyncCommitteeHashFromPayload(nextSyncCommitteePayload), ETH_NEXT_SYNC_COMMITTEE_HASH);
  assert.equal(ethSyncCommitteePayloadHash(nextSyncCommitteePayload), ETH_NEXT_SYNC_COMMITTEE_PAYLOAD_HASH);
  assert.throws(
    () =>
      canonicalEthSyncCommitteePayloadBytes({
        ...parent,
        sync_committee_public_keys: parent.syncCommitteePublicKeys,
      }),
    /syncCommitteePublicKeys must not use multiple aliases/u,
  );
  assert.equal(canonicalEthSyncCommitteeTransitionMessageBytes(transitionMessage).length, 189);
  assert.equal(
    ethSyncCommitteeTransitionMessageHash(transitionMessage),
    ETH_SYNC_COMMITTEE_TRANSITION_MESSAGE_HASH,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteeTransitionMessageBytes({
        ...transitionMessage,
        sourceDomain: SCCP_DOMAIN_BSC,
      }),
    /sourceDomain/,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteeTransitionMessageBytes({
        ...transitionMessage,
        source_domain: SCCP_DOMAIN_ETH,
      }),
    /sourceDomain must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteeTransitionMessageBytes({
        ...transitionMessage,
        from_sync_period: 7n,
      }),
    /fromSyncPeriod must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteeTransitionMessageBytes({
        ...transitionMessage,
        next_sync_committee_payload_hash: ETH_NEXT_SYNC_COMMITTEE_PAYLOAD_HASH,
      }),
    /nextSyncCommitteePayloadHash must not use multiple aliases/u,
  );
  assert.equal(canonicalEthSyncCommitteeTransitionSignatureBytes(transitionSignature).length, 1068);
  assert.equal(
    ethSyncCommitteeTransitionSignatureHash(transitionSignature),
    ETH_SYNC_COMMITTEE_TRANSITION_SIGNATURE_HASH,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteeTransitionSignatureBytes({
        ...transitionSignature,
        sync_committee_proof: transitionSignature.syncCommitteeProof,
      }),
    /syncCommitteeProof must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteeTransitionSignatureBytes({
        ...transitionSignature,
        next_sync_committee_payload: nextSyncCommitteePayload,
      }),
    /nextSyncCommitteePayload must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteeTransitionSignatureBytes({
        ...transitionSignature,
        transition_message_hash: ETH_SYNC_COMMITTEE_TRANSITION_MESSAGE_HASH,
      }),
    /transitionMessageHash must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteeTransitionSignatureBytes({
        ...transitionSignature,
        syncCommitteeProof: {
          ...transitionSignature.syncCommitteeProof,
          signers_bitmap: transitionSignature.syncCommitteeProof.signersBitmap,
        },
      }),
    /signersBitmap must not use multiple aliases/u,
  );
  assert.throws(
    () => canonicalEthSyncCommitteeTransitionSignatureBytes({ ...transitionSignature, version: 0 }),
    /ETH sync-committee transition signature version/,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteeTransitionSignatureBytes({
        ...transitionSignature,
        syncCommitteeProof: {
          ...transitionSignature.syncCommitteeProof,
          version: 0,
        },
      }),
    /syncCommitteeProof\.version/,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteeTransitionSignatureBytes({
        ...transitionSignature,
        syncCommitteeProof: {
          ...transitionSignature.syncCommitteeProof,
          version: null,
        },
      }),
    /syncCommitteeProof\.version must be 1/,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteePayloadBytes({
        syncCommitteePublicKeys: Array.from({ length: 513 }, () => `0x${"11".repeat(48)}`),
        syncCommitteeWeights: Array.from({ length: 513 }, () => 1n),
        syncCommitteePops: Array.from({ length: 513 }, () => `0x${"aa".repeat(96)}`),
      }),
    /at most 512/,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteePayloadBytes({
        ...parent,
        syncCommitteePublicKeys: [`0x${"11".repeat(47)}`, `0x${"22".repeat(48)}`],
      }),
    /48 bytes/,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteePayloadBytes({
        ...parent,
        syncCommitteePublicKeys: [`0x${"00".repeat(48)}`, `0x${"22".repeat(48)}`],
      }),
    /must not be zero/,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteePayloadBytes({
        ...parent,
        syncCommitteePops: [new Uint8Array(96), `0x${"bb".repeat(96)}`],
      }),
    /must not be zero/,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteeTransitionSignatureBytes({
        ...transitionSignature,
        syncCommitteeProof: {
          ...transitionSignature.syncCommitteeProof,
          signersBitmap: new Uint8Array(65),
        },
      }),
    /signersBitmap/,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteeTransitionSignatureBytes({
        ...transitionSignature,
        syncCommitteeProof: {
          ...transitionSignature.syncCommitteeProof,
          signersBitmap: [0x04],
        },
      }),
    /padding bits/,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteeTransitionSignatureBytes({
        ...transitionSignature,
        syncCommitteeProof: {
          ...transitionSignature.syncCommitteeProof,
          signersBitmap: [0x00],
        },
      }),
    /select at least one/,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteeTransitionSignatureBytes({
        ...transitionSignature,
        syncCommitteeProof: {
          ...transitionSignature.syncCommitteeProof,
          totalWeight: 4n,
        },
      }),
    /totalWeight/,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteeTransitionSignatureBytes({
        ...transitionSignature,
        syncCommitteeProof: {
          ...transitionSignature.syncCommitteeProof,
          signedWeight: 2n,
        },
      }),
    /signedWeight/,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteeTransitionSignatureBytes({
        ...transitionSignature,
        syncCommitteeProof: {
          ...transitionSignature.syncCommitteeProof,
          signedWeight: 1n,
          signersBitmap: [0x01],
        },
      }),
    /greater than two thirds/,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteeTransitionSignatureBytes({
        ...transitionSignature,
        syncCommitteeProof: {
          ...transitionSignature.syncCommitteeProof,
          aggregateSignature: new Uint8Array(95),
        },
      }),
    /96 bytes/,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteeTransitionSignatureBytes({
        ...transitionSignature,
        syncCommitteeProof: {
          ...transitionSignature.syncCommitteeProof,
          aggregateSignature: new Uint8Array(96),
        },
      }),
    /all zero/,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteeTransitionSignatureBytes({
        ...transitionSignature,
        nextSyncCommitteePayloadHash: HEX32_B,
      }),
    /nextSyncCommitteePayloadHash/,
  );
  assert.throws(
    () =>
      canonicalEthSyncCommitteeTransitionSignatureBytes({
        ...transitionSignature,
        nextSyncCommitteeHash: HEX32_B,
      }),
    /nextSyncCommitteeHash/,
  );
});

test("derives ETH beacon execution payload SSZ roots from UI witness material", () => {
  const headerRlp = sampleEthExecutionHeaderRlp();
  const executionPayloadRoot = ethExecutionPayloadHeaderRootFromRlp(headerRlp);
  const executionPayloadBranch = [
    HEX32_E,
    `0x${"ff".repeat(32)}`,
    `0x${"11".repeat(32)}`,
    `0x${"22".repeat(32)}`,
  ];
  const beaconBodyRoot = ethBeaconBodyRootFromExecutionPayloadBranch(
    executionPayloadRoot,
    executionPayloadBranch,
  );
  const beaconHeaderInput = {
    beaconSlot: 320n,
    beaconProposerIndex: 17n,
    beaconParentRoot: HEX32_A,
    beaconStateRoot: HEX32_B,
    beaconBodyRoot,
  };
  const beaconHeaderRoot = ethBeaconBlockHeaderRoot(beaconHeaderInput);

  assert.equal(
    executionPayloadRoot,
    "0xc029dda492d2e41ad72bd83f1727a67e5331f413ec29d5c31de955d0bea24624",
  );
  assert.equal(
    beaconBodyRoot,
    "0x431e6bef5e759e8fdf32d8e8ed1ff761933ddb4de24ec9ae8e2aa0d25fe861ba",
  );
  assert.equal(
    beaconHeaderRoot,
    "0xd54b406debae26e6ebaef512cc4f9e6bc12cf02af0d4476895383b37f682a179",
  );
  assert.throws(
    () => ethBeaconBlockHeaderRoot({ ...beaconHeaderInput, slot: 320n }),
    /slot must not use multiple aliases/u,
  );
  assert.throws(
    () => ethBeaconBlockHeaderRoot({ ...beaconHeaderInput, proposerIndex: 17n }),
    /proposerIndex must not use multiple aliases/u,
  );
  assert.throws(
    () => ethBeaconBlockHeaderRoot({ ...beaconHeaderInput, body_root: beaconBodyRoot }),
    /bodyRoot must not use multiple aliases/u,
  );
  assert.notEqual(
    ethBeaconBodyRootFromExecutionPayloadBranch(executionPayloadRoot, [
      `0x${"ff".repeat(32)}`,
      `0x${"ff".repeat(32)}`,
      `0x${"11".repeat(32)}`,
      `0x${"22".repeat(32)}`,
    ]),
    beaconBodyRoot,
  );
  assert.throws(
    () => ethBeaconBodyRootFromExecutionPayloadBranch(executionPayloadRoot, [HEX32_E]),
    /executionPayloadBranch/,
  );
  assert.throws(
    () => ethExecutionPayloadHeaderRootFromRlp(Uint8Array.from([0x80])),
    /RLP list/,
  );
});

test("derives TON masterchain config proof hashes from UI witness material", () => {
  const validatorSet = {
    validatorPublicKeys: [`0x${"11".repeat(32)}`, `0x${"22".repeat(32)}`],
    validatorWeights: [1n, 2n],
  };
  const validatorSetPayload = canonicalTonValidatorSetPayloadBytes(validatorSet);
  const leafInput = {
    sourceDomain: SCCP_DOMAIN_TON,
    masterchainSeqno: 19n,
    masterchainBlockHash: HEX32_A,
    shardStateRoot: HEX32_C,
    validatorSetHash: TON_VALIDATOR_SET_HASH,
    validatorSetPayloadHash: TON_VALIDATOR_SET_PAYLOAD_HASH,
  };
  const proofInput = {
    ...leafInput,
    configRoot: TON_MASTERCHAIN_CONFIG_ROOT,
    configLeafHash: TON_MASTERCHAIN_CONFIG_LEAF_HASH,
    configLeafIndex: SCCP_TON_CURRENT_VALIDATOR_SET_CONFIG_PARAM,
    configValueHash: TON_MASTERCHAIN_CONFIG_VALUE_HASH,
    configDictionaryProofBoc: Buffer.from(TON_MASTERCHAIN_CONFIG_PROOF_BOC_HEX, "hex"),
    configInclusionBranch: [],
  };

  assert.equal(tonValidatorSetPayloadHash(validatorSetPayload), TON_VALIDATOR_SET_PAYLOAD_HASH);
  assert.deepEqual(
    tonConfigValidatorSetPayloadFromProofBoc(proofInput.configDictionaryProofBoc),
    validatorSetPayload,
  );
  assert.equal(
    tonConfigValidatorSetPayloadHashFromProofBoc(proofInput.configDictionaryProofBoc),
    TON_VALIDATOR_SET_PAYLOAD_HASH,
  );
  assert.equal(tonHashmapEProofRootHash(proofInput.configDictionaryProofBoc), TON_MASTERCHAIN_CONFIG_ROOT);
  assert.equal(
    tonHashmapECellRefValueHash(
      proofInput.configDictionaryProofBoc,
      Uint8Array.from([0, 0, 0, Number(SCCP_TON_CURRENT_VALIDATOR_SET_CONFIG_PARAM)]),
      32,
    ),
    TON_MASTERCHAIN_CONFIG_VALUE_HASH,
  );
  assert.equal(canonicalTonMasterchainConfigLeafBytes(leafInput).length, 141);
  assert.equal(tonMasterchainConfigLeafHash(leafInput), TON_MASTERCHAIN_CONFIG_LEAF_HASH);
  assert.equal(canonicalTonMasterchainConfigProofBytes(proofInput).length, 411);
  assert.equal(tonMasterchainConfigProofHash(proofInput), TON_MASTERCHAIN_CONFIG_PROOF_HASH);
  assert.throws(
    () => canonicalTonMasterchainConfigLeafBytes({ ...leafInput, source_domain: SCCP_DOMAIN_TON }),
    /sourceDomain must not use multiple aliases/,
  );
  assert.throws(
    () => canonicalTonMasterchainConfigProofBytes({ ...proofInput, config_root: TON_MASTERCHAIN_CONFIG_ROOT }),
    /configRoot must not use multiple aliases/,
  );
  assert.throws(
    () => canonicalTonMasterchainConfigLeafBytes({ ...leafInput, version: 0 }),
    /TON masterchain config leaf version/,
  );
  assert.throws(
    () => canonicalTonMasterchainConfigProofBytes({ ...proofInput, version: 0 }),
    /TON masterchain config proof version/,
  );
  assert.throws(
    () => tonMasterchainConfigProofHash({ ...proofInput, configLeafIndex: 0n }),
    /config param 34/,
  );
  assert.throws(
    () =>
      canonicalTonMasterchainConfigProofBytes({
        ...proofInput,
        configValueHash: HEX32_E,
      }),
    /value does not match/,
  );
  assert.throws(
    () =>
      canonicalTonMasterchainConfigProofBytes({
        ...proofInput,
        validatorSetPayloadHash: HEX32_E,
      }),
    /ValidatorSet/,
  );
  assert.throws(
    () =>
      canonicalTonMasterchainConfigProofBytes({
        ...proofInput,
        configLeafHash: HEX32_E,
      }),
    /configLeafHash/,
  );
  assert.throws(
    () =>
      canonicalTonMasterchainConfigProofBytes({
        ...proofInput,
        validatorSetHash: HEX32_E,
        configLeafHash: tonMasterchainConfigLeafHash({
          ...leafInput,
          validatorSetHash: HEX32_E,
        }),
      }),
    /validatorSetHash/,
  );
  assert.throws(
    () =>
      canonicalTonMasterchainConfigProofBytes({
        ...proofInput,
        sourceDomain: SCCP_DOMAIN_SOL,
        configLeafHash: tonMasterchainConfigLeafHash({
          ...leafInput,
          sourceDomain: SCCP_DOMAIN_SOL,
        }),
      }),
    /sourceDomain/,
  );
  assert.throws(
    () =>
      canonicalTonMasterchainConfigProofBytes({
        ...proofInput,
        configInclusionBranch: [HEX32_E],
      }),
    /must be empty/,
  );
});

test("derives TON masterchain block-message and signature hashes from UI witness material", () => {
  const validatorSet = {
    validatorPublicKeys: [`0x${"11".repeat(32)}`, `0x${"22".repeat(32)}`],
    validatorWeights: [1n, 2n],
  };
  const blockMessage = {
    sourceDomain: SCCP_DOMAIN_TON,
    masterchainSeqno: 19n,
    masterchainWorkchainId: -1,
    masterchainShard: 0x8000000000000000n,
    masterchainBlockHash: HEX32_A,
    masterchainFileHash: `0x${"a5".repeat(32)}`,
    validatorSetHash: TON_VALIDATOR_SET_HASH,
    masterchainConfigRoot: TON_MASTERCHAIN_CONFIG_ROOT,
    masterchainConfigProofHash: TON_MASTERCHAIN_CONFIG_PROOF_HASH,
    shardWorkchainId: 0,
    shardShard: 0x8000000000000000n,
    shardSeqno: 7n,
    shardBlockHash: HEX32_B,
    shardFileHash: `0x${"bc".repeat(32)}`,
    shardStateRoot: HEX32_C,
    transactionRoot: HEX32_D,
    shardProofHash: HEX32_E,
  };
  const signatures = {
    version: 1,
    totalWeight: 3n,
    signedWeight: 3n,
    blockMessageHash: TON_MASTERCHAIN_BLOCK_MESSAGE_HASH,
    ...validatorSet,
    validatorSetHash: TON_VALIDATOR_SET_HASH,
    signersBitmap: [0x03],
    signatures: [new Uint8Array(64).fill(0xab), new Uint8Array(64).fill(0xcd)],
  };

  assert.equal(canonicalTonMasterchainBlockMessageBytes(blockMessage).length, 365);
  assert.equal(tonMasterchainBlockMessageHash(blockMessage), TON_MASTERCHAIN_BLOCK_MESSAGE_HASH);
  assert.equal(canonicalTonMasterchainValidatorSignaturesBytes(signatures).length, 322);
  assert.equal(tonMasterchainValidatorSignaturesHash(signatures), TON_MASTERCHAIN_SIGNATURES_HASH);
  assert.notEqual(
    tonMasterchainBlockMessageHash({ ...blockMessage, shardProofHash: HEX32_F }),
    TON_MASTERCHAIN_BLOCK_MESSAGE_HASH,
  );
  assert.throws(
    () => canonicalTonMasterchainBlockMessageBytes({ ...blockMessage, shard_proof_hash: HEX32_E }),
    /shardProofHash must not use multiple aliases/,
  );
  assert.throws(
    () => canonicalTonMasterchainValidatorSignaturesBytes({ ...signatures, validator_set_hash: TON_VALIDATOR_SET_HASH }),
    /validatorSetHash must not use multiple aliases/,
  );
  assert.throws(
    () => canonicalTonMasterchainBlockMessageBytes({ ...blockMessage, masterchainWorkchainId: 0 }),
    /masterchainWorkchainId/,
  );
  assert.throws(
    () => canonicalTonMasterchainBlockMessageBytes({ ...blockMessage, masterchainShard: 0n }),
    /masterchainShard/,
  );
  assert.throws(
    () => canonicalTonMasterchainBlockMessageBytes({ ...blockMessage, masterchainFileHash: `0x${"00".repeat(32)}` }),
    /masterchainFileHash/,
  );
  assert.throws(
    () => canonicalTonMasterchainBlockMessageBytes({ ...blockMessage, shardWorkchainId: -1 }),
    /shardWorkchainId/,
  );
  assert.throws(
    () => canonicalTonMasterchainBlockMessageBytes({ ...blockMessage, shardSeqno: 0n }),
    /shardSeqno/,
  );
  assert.throws(
    () => canonicalTonMasterchainBlockMessageBytes({ ...blockMessage, shardFileHash: `0x${"00".repeat(32)}` }),
    /shardFileHash/,
  );
  assert.throws(
    () => canonicalTonMasterchainValidatorSignaturesBytes({ ...signatures, validatorSetHash: HEX32_B }),
    /validatorSetHash/,
  );
  assert.throws(
    () =>
      canonicalTonMasterchainValidatorSignaturesBytes({
        ...signatures,
        signatures: [new Uint8Array(64), new Uint8Array(64).fill(0x01)],
      }),
    /must not be all zero/,
  );
});

test("derives TON BoC root hashes for UI proof material", () => {
  const boc = Buffer.from(TON_ORDINARY_BOC_HEX, "hex");

  assert.deepEqual(tonBocRootHashes(boc), [TON_ORDINARY_BOC_ROOT_HASH]);
  assert.equal(tonBocSingleRootHash(boc), TON_ORDINARY_BOC_ROOT_HASH);
  assert.equal(
    tonBocSingleRootHash(Buffer.from(TON_ORDINARY_BOC_CRC_HEX, "hex")),
    TON_ORDINARY_BOC_ROOT_HASH,
  );

  const badCrc = Buffer.from(TON_ORDINARY_BOC_CRC_HEX, "hex");
  badCrc[badCrc.length - 1] ^= 0x01;
  assert.throws(() => tonBocSingleRootHash(badCrc), /CRC32C/);

  const changedChild = Uint8Array.from(boc);
  changedChild[changedChild.length - 1] ^= 0x01;
  assert.notEqual(tonBocSingleRootHash(changedChild), TON_ORDINARY_BOC_ROOT_HASH);

  const cyclicRef = Uint8Array.from(boc);
  cyclicRef[14] = 0;
  assert.throws(() => tonBocSingleRootHash(cyclicRef), /forward internal refs/);

  const explicitHashDescriptor = Uint8Array.from(boc);
  explicitHashDescriptor[11] |= 0x10;
  assert.throws(() => tonBocSingleRootHash(explicitHashDescriptor), /descriptor/);

  const invalidPartialData = Uint8Array.from(boc);
  invalidPartialData[16] = 1;
  invalidPartialData[17] = 0;
  assert.throws(() => tonBocSingleRootHash(invalidPartialData), /padding/);

  assert.equal(
    tonBocSingleRootHash(Buffer.from(TON_PRUNED_BRANCH_BOC_HEX, "hex")),
    TON_PRUNED_BRANCH_ROOT_HASH,
  );
  assert.equal(
    tonBocSingleRootHash(Buffer.from(TON_LEGACY_PRUNED_PROOF_BOC_HEX, "hex")),
    TON_LEGACY_PRUNED_PROOF_ROOT_HASH,
  );
  assert.equal(
    tonBocSingleRootHash(Buffer.from(TON_MERKLE_PROOF_BOC_HEX, "hex")),
    TON_MERKLE_PROOF_ROOT_HASH,
  );
  const mismatchedMerkleProof = Buffer.from(TON_MERKLE_PROOF_BOC_HEX, "hex");
  mismatchedMerkleProof[14] ^= 0x01;
  assert.throws(() => tonBocSingleRootHash(mismatchedMerkleProof), /Merkle proof/);
});

test("derives TON HashmapE cell-ref value hashes for UI proof material", () => {
  assert.equal(
    tonHashmapECellRefValueHash(Buffer.from(TON_HASHMAP_E_CELL_REF_BOC_HEX, "hex"), Uint8Array.from([17]), 8),
    TON_HASHMAP_E_VALUE_HASH,
  );
  assert.equal(
    tonHashmapECellRefValueHash(Buffer.from(TON_HASHMAP_E_CELL_REF_BOC_HEX, "hex"), Uint8Array.from([18]), 8),
    null,
  );
  assert.throws(
    () => tonHashmapECellRefValueHash(Buffer.from(TON_HASHMAP_E_CELL_REF_BOC_HEX, "hex"), Uint8Array.from([17]), 7),
    /key length/,
  );
  assert.equal(
    tonHashmapECellRefValueHash(Buffer.from(TON_HASHMAP_E_DIRECT_PROOF_BOC_HEX, "hex"), Uint8Array.from([17]), 8),
    TON_HASHMAP_E_VALUE_HASH,
  );
  assert.equal(
    tonHashmapECellRefValueHash(Buffer.from(TON_HASHMAP_E_DIRECT_PROOF_BOC_HEX, "hex"), Uint8Array.from([1]), 8),
    null,
  );
  assert.equal(
    tonHashmapECellRefValueHash(Buffer.from(TON_HASHMAP_E_MERKLE_PROOF_BOC_HEX, "hex"), Uint8Array.from([17]), 8),
    TON_HASHMAP_E_VALUE_HASH,
  );
});

test("derives TON ShardStateUnsplit accounts roots for UI proof material", () => {
  const shardStateProofBoc = Buffer.from(TON_SHARD_STATE_PROOF_BOC_HEX, "hex");
  assert.equal(tonShardStateProofRootHash(shardStateProofBoc), TON_SHARD_STATE_ROOT_HASH);
  assert.equal(tonShardStateAccountsRootHash(shardStateProofBoc), TON_SHARD_ACCOUNTS_ROOT_HASH);

  const badTag = Buffer.from(shardStateProofBoc);
  const tagOffset = badTag.indexOf(Buffer.from("9023afe2", "hex"));
  assert.notEqual(tagOffset, -1);
  badTag[tagOffset] ^= 0x01;
  assert.throws(() => tonShardStateAccountsRootHash(badTag), /ShardStateUnsplit/);
  const shardIdentOffset = tagOffset + 8;
  const badShardIdentTag = Buffer.from(shardStateProofBoc);
  badShardIdentTag[shardIdentOffset] |= 0x80;
  assert.throws(() => tonShardStateAccountsRootHash(badShardIdentTag), /ShardIdent/);
  const badShardIdentPrefixLen = Buffer.from(shardStateProofBoc);
  badShardIdentPrefixLen[shardIdentOffset] = 0x3d;
  assert.throws(() => tonShardStateAccountsRootHash(badShardIdentPrefixLen), /ShardIdent/);
  const basechainCustom = Buffer.from(shardStateProofBoc);
  basechainCustom[tagOffset + 45] |= 0x40;
  assert.throws(() => tonShardStateAccountsRootHash(basechainCustom), /custom/);
});

test("derives EVM, BSC, TRON, and Substrate source proof hashes from UI witness material", () => {
  const sourceEventDigest = `0x${"34".repeat(32)}`;
  const inclusionBranch = [HEX32_E];
  const evmInput = {
    sourceEventDigest,
    beaconSlot: 11n,
    executionBlockNumber: 12n,
    executionBlockHash: HEX32_A,
    executionReceiptsRoot: EVM_RECEIPT_STATE_TRANSACTION_ROOT,
    beaconFinalizedRoot: HEX32_C,
    syncCommitteeRoot: HEX32_D,
    receiptRootIndex: 0n,
    receiptTrieProofNodes: [EVM_RECEIPT_STATE_MPT_NODE_HEX],
    inclusionBranch,
  };
  const bscInput = {
    sourceEventDigest,
    validatorEpoch: 21n,
    blockNumber: 22n,
    blockHash: HEX32_A,
    receiptsRoot: EVM_RECEIPT_STATE_TRANSACTION_ROOT,
    validatorSetHash: HEX32_C,
    commitSealHash: HEX32_D,
    receiptRootIndex: 0n,
    receiptTrieProofNodes: [EVM_RECEIPT_STATE_MPT_NODE_HEX],
    inclusionBranch,
  };
  const tronInput = {
    sourceEventDigest,
    receiptRoot: HEX32_B,
    transactionRoot: HEX32_D,
    inclusionBranch,
  };
  const tronReceiptStateInput = {
    sourceEventDigest,
    receiptRoot: HEX32_B,
    transactionRoot: TRON_RECEIPT_STATE_TRANSACTION_ROOT,
    receiptRootIndex: 0n,
    receiptTrieProofNodes: [TRON_RECEIPT_STATE_MPT_NODE_HEX],
    inclusionBranch,
  };
  const tronTransactionSourceInput = {
    sourceEventDigest,
    receiptRoot: HEX32_B,
    transactionRoot: TRON_TRANSACTION_SOURCE_ROOT,
    transactionIndex: 0n,
    transactionCount: 1n,
    transactionBytes: TRON_TRANSACTION_SOURCE_BYTES_HEX,
    transactionMerkleBranch: [],
    inclusionBranch: [`0x${"aa".repeat(32)}`],
  };
  const substrateInput = {
    sourceDomain: 6,
    sourceEventDigest,
    sourceEventLeafIndex: 0n,
    finalizedBlockNumber: 31n,
    grandpaSetId: 32n,
    blockHash: HEX32_A,
    authoritySetHash: HEX32_C,
    eventsRoot: HEX32_B,
    inclusionBranch,
  };
  const substrateRuntimeStorageInput = {
    ...substrateInput,
    sourceTrustAnchorHash: HEX32_A,
    consensusVerifierHash: HEX32_B,
    messageInclusionVerifierHash: HEX32_C,
    finalityPolicyHash: HEX32_D,
    sourceStateVerifierHash: HEX32_F,
  };

  assert.equal(canonicalEvmSccpReceiptProofBytes(evmInput).length, 306);
  assert.throws(
    () => canonicalEvmSccpReceiptProofBytes({ ...evmInput, sourceDomain: SCCP_DOMAIN_BSC }),
    /sourceDomain/,
  );
  for (const [patch, pattern] of [
    [{ sourceDomain: SCCP_DOMAIN_ETH, source_domain: SCCP_DOMAIN_ETH }, /sourceDomain must not use multiple aliases/u],
    [{ source_event_digest: sourceEventDigest }, /sourceEventDigest must not use multiple aliases/u],
    [{ beacon_slot: 11n }, /beaconSlot must not use multiple aliases/u],
    [{ finalityHeight: 12n }, /executionBlockNumber must not use multiple aliases/u],
    [{ finalityBlockHash: HEX32_A }, /executionBlockHash must not use multiple aliases/u],
    [{ receipt_or_message_root: EVM_RECEIPT_STATE_TRANSACTION_ROOT }, /executionReceiptsRoot must not use multiple aliases/u],
    [{ beacon_finalized_root: HEX32_C }, /beaconFinalizedRoot must not use multiple aliases/u],
    [{ sync_committee_root: HEX32_D }, /syncCommitteeRoot must not use multiple aliases/u],
    [{ receipt_root_index: 0n }, /receiptRootIndex must not use multiple aliases/u],
    [{ receipt_trie_proof_nodes: [EVM_RECEIPT_STATE_MPT_NODE_HEX] }, /receiptTrieProofNodes must not use multiple aliases/u],
    [{ inclusion_branch: inclusionBranch }, /inclusionBranch must not use multiple aliases/u],
  ]) {
    assert.throws(() => canonicalEvmSccpReceiptProofBytes({ ...evmInput, ...patch }), pattern);
  }
  assert.equal(canonicalBscSccpReceiptProofBytes(bscInput).length, 306);
  assert.throws(
    () => canonicalBscSccpReceiptProofBytes({ ...bscInput, sourceDomain: SCCP_DOMAIN_ETH }),
    /sourceDomain/,
  );
  for (const [patch, pattern] of [
    [{ sourceDomain: SCCP_DOMAIN_BSC, source_domain: SCCP_DOMAIN_BSC }, /sourceDomain must not use multiple aliases/u],
    [{ source_event_digest: sourceEventDigest }, /sourceEventDigest must not use multiple aliases/u],
    [{ finalityHeight: 22n }, /blockNumber must not use multiple aliases/u],
    [{ finalityBlockHash: HEX32_A }, /blockHash must not use multiple aliases/u],
    [{ receipt_or_message_root: EVM_RECEIPT_STATE_TRANSACTION_ROOT }, /receiptsRoot must not use multiple aliases/u],
    [{ validator_set_hash: HEX32_C }, /validatorSetHash must not use multiple aliases/u],
    [{ commit_seal_hash: HEX32_D }, /commitSealHash must not use multiple aliases/u],
    [{ receipt_root_index: 0n }, /receiptRootIndex must not use multiple aliases/u],
    [{ receipt_trie_proof_nodes: [EVM_RECEIPT_STATE_MPT_NODE_HEX] }, /receiptTrieProofNodes must not use multiple aliases/u],
    [{ inclusion_branch: inclusionBranch }, /inclusionBranch must not use multiple aliases/u],
  ]) {
    assert.throws(() => canonicalBscSccpReceiptProofBytes({ ...bscInput, ...patch }), pattern);
  }
  assert.equal(
    `0x${Buffer.from(canonicalEvmReceiptRootMptValue(HEX32_B)).toString("hex")}`,
    EVM_RECEIPT_ROOT_MPT_VALUE_HEX,
  );
  assert.throws(() => canonicalEvmReceiptRootMptValue("0x1234"), /32 bytes/);
  assert.equal(
    `0x${Buffer.from(canonicalTronReceiptRootMptValue(HEX32_B)).toString("hex")}`,
    TRON_RECEIPT_ROOT_MPT_VALUE_HEX,
  );
  assert.throws(() => canonicalTronReceiptRootMptValue("0x1234"), /32 bytes/);
  assert.throws(() => canonicalTronReceiptRootMptValue(SCCP_ZERO_HASH_V1), /must not be zero/);
  assert.equal(canonicalTronSccpReceiptProofBytes(tronInput).length, 133);
  for (const [patch, pattern] of [
    [{ source_event_digest: sourceEventDigest }, /sourceEventDigest must not use multiple aliases/u],
    [{ receipt_or_message_root: HEX32_B }, /receiptRoot must not use multiple aliases/u],
    [{ transaction_root: HEX32_D }, /transactionRoot must not use multiple aliases/u],
    [{ inclusion_branch: inclusionBranch }, /inclusionBranch must not use multiple aliases/u],
  ]) {
    assert.throws(() => canonicalTronSccpReceiptProofBytes({ ...tronInput, ...patch }), pattern);
  }
  for (const fieldName of ["sourceEventDigest", "receiptRoot", "transactionRoot"]) {
    assert.throws(
      () =>
        canonicalTronSccpReceiptProofBytes({
          ...tronInput,
          [fieldName]: SCCP_ZERO_HASH_V1,
        }),
      /must not be zero/,
    );
  }
  assert.throws(
    () => canonicalTronSccpReceiptProofBytes({ ...tronInput, inclusionBranch: [] }),
    /inclusionBranch must not be empty/,
  );
  assert.equal(canonicalTronSccpReceiptStateProofBytes(tronReceiptStateInput).length, 186);
  for (const [patch, pattern] of [
    [{ source_event_digest: sourceEventDigest }, /sourceEventDigest must not use multiple aliases/u],
    [{ receipt_or_message_root: HEX32_B }, /receiptRoot must not use multiple aliases/u],
    [{ transaction_root: TRON_RECEIPT_STATE_TRANSACTION_ROOT }, /transactionRoot must not use multiple aliases/u],
    [{ receipt_root_index: 0n }, /receiptRootIndex must not use multiple aliases/u],
    [{ receipt_trie_proof_nodes: [TRON_RECEIPT_STATE_MPT_NODE_HEX] }, /receiptTrieProofNodes must not use multiple aliases/u],
    [{ inclusion_branch: inclusionBranch }, /inclusionBranch must not use multiple aliases/u],
  ]) {
    assert.throws(() => canonicalTronSccpReceiptStateProofBytes({ ...tronReceiptStateInput, ...patch }), pattern);
  }
  for (const fieldName of ["sourceEventDigest", "receiptRoot", "transactionRoot"]) {
    assert.throws(
      () =>
        canonicalTronSccpReceiptStateProofBytes({
          ...tronReceiptStateInput,
          [fieldName]: SCCP_ZERO_HASH_V1,
        }),
      /must not be zero/,
    );
  }
  assert.throws(
    () => canonicalTronSccpReceiptStateProofBytes({ ...tronReceiptStateInput, inclusionBranch: [] }),
    /inclusionBranch must not be empty/,
  );
  assert.equal(tronSccpReceiptStateProofHash(tronReceiptStateInput), TRON_RECEIPT_STATE_PROOF_HASH);
  assert.equal(
    Buffer.from(tronSccpSourceMessageCallData(5, 0, sourceEventDigest)).toString("hex"),
    TRON_SOURCE_MESSAGE_CALL_DATA_HEX,
  );
  assert.throws(
    () => tronSccpSourceMessageCallData(0, 0, sourceEventDigest),
    /sourceDomain/,
  );
  assert.throws(
    () => tronSccpSourceMessageCallData(5, 5, sourceEventDigest),
    /targetDomain/,
  );
  assert.throws(
    () => tronSccpSourceMessageCallData(5, 0, `0x${"00".repeat(32)}`),
    /sourceEventDigest/,
  );
  assert.equal(canonicalTronSccpTransactionSourceProofBytes(tronTransactionSourceInput).length, 476);
  assert.deepEqual(
    canonicalTronSccpTransactionSourceProofBytes({
      ...tronTransactionSourceInput,
      sourceBridgeEmitterAddress: `0x${"45".repeat(20)}`,
      sourceBridgeOwnerAddress: "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf",
    }),
    canonicalTronSccpTransactionSourceProofBytes(tronTransactionSourceInput),
  );
  for (const [patch, pattern] of [
    [{ source_event_digest: sourceEventDigest }, /sourceEventDigest must not use multiple aliases/u],
    [{ receipt_or_message_root: HEX32_B }, /receiptRoot must not use multiple aliases/u],
    [{ transaction_root: TRON_TRANSACTION_SOURCE_ROOT }, /transactionRoot must not use multiple aliases/u],
    [{ transaction_index: 0n }, /transactionIndex must not use multiple aliases/u],
    [{ transaction_count: 1n }, /transactionCount must not use multiple aliases/u],
    [{ transaction_bytes: TRON_TRANSACTION_SOURCE_BYTES_HEX }, /transactionBytes must not use multiple aliases/u],
    [{ transaction_merkle_branch: [] }, /transactionMerkleBranch must not use multiple aliases/u],
    [{ inclusion_branch: [`0x${"aa".repeat(32)}`] }, /inclusionBranch must not use multiple aliases/u],
    [
      { bridgeAddress: `0x${"45".repeat(20)}`, sourceBridgeEmitterAddress: `0x${"45".repeat(20)}` },
      /sourceBridgeEmitterAddress must not use multiple aliases/u,
    ],
    [
      {
        ownerAddress: "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf",
        sourceBridgeOwnerAddress: "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf",
      },
      /sourceBridgeOwnerAddress must not use multiple aliases/u,
    ],
  ]) {
    assert.throws(
      () => canonicalTronSccpTransactionSourceProofBytes({ ...tronTransactionSourceInput, ...patch }),
      pattern,
    );
  }
  assert.equal(
    tronSccpTransactionSourceProofHash(tronTransactionSourceInput),
    TRON_TRANSACTION_SOURCE_PROOF_HASH,
  );
  const omittedDefaultRetTransactionSourceInput = {
    ...tronTransactionSourceInput,
    transactionRoot: "0x62489e5ad22dd0fc7a4b8444c2b17ef28c2c885a01bd0f97fd7f63fbfb1552bd",
    transactionBytes: TRON_TRANSACTION_SOURCE_BYTES_HEX.replace("2a0410001801", "2a021801"),
  };
  assert.equal(
    canonicalTronSccpTransactionSourceProofBytes(omittedDefaultRetTransactionSourceInput).length,
    474,
  );
  assert.equal(
    tronSccpTransactionSourceProofHash(omittedDefaultRetTransactionSourceInput),
    "0xdb367957f5100b81ef1b074867c5c7c846c8bb3b44353668f65bf1c8ec805a18",
  );
  assert.throws(
    () =>
      canonicalTronSccpTransactionSourceProofBytes({
        ...tronTransactionSourceInput,
        sourceBridgeEmitterAddress: `0x${"46".repeat(20)}`,
        sourceBridgeOwnerAddress: "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf",
      }),
    /successful TRON TriggerSmartContract source call/,
  );
  assert.throws(
    () =>
      canonicalTronSccpTransactionSourceProofBytes({
        ...tronTransactionSourceInput,
        sourceBridgeEmitterAddress: `0x${"45".repeat(20)}`,
        sourceBridgeOwnerAddress: `0x${"22".repeat(20)}`,
      }),
    /successful TRON TriggerSmartContract source call/,
  );
  assert.throws(
    () =>
      canonicalTronSccpTransactionSourceProofBytes({
        ...tronTransactionSourceInput,
        transactionBytes: TRON_TRANSACTION_SOURCE_BYTES_HEX.replace(
          "fe8973c012a0410001801",
          "fe8973c1f2a0410001801",
        ),
      }),
    /successful TRON TriggerSmartContract source call/,
  );
  assert.throws(
    () =>
      canonicalTronSccpTransactionSourceProofBytes({
        ...tronTransactionSourceInput,
        transactionBytes: TRON_TRANSACTION_SOURCE_BYTES_HEX.replace(
          "cc58d7ac52c9111792495fee682b53cab96ff4229043c5b8b90c31447f5934553d8854ab35de34372c13331bf3ef5cefd8f2cc5ad026faf223da83969fe8973c01",
          "b50455577deef2a0d6c3c521d97de050d5b9ba46df00c8ddad014bac4ca3345173223f1d4c5940538f1b1da069bed6828a9b27794bd1eac1a35810baaef28d2101",
        ),
      }),
    /successful TRON TriggerSmartContract source call/,
  );
  assert.throws(
    () => canonicalTronSccpTransactionSourceProofBytes({ ...tronTransactionSourceInput, transactionIndex: 1n }),
    /transactionIndex/,
  );
  assert.throws(
    () =>
      canonicalTronSccpTransactionSourceProofBytes({
        ...tronTransactionSourceInput,
        transactionCount: 3n,
        transactionMerkleBranch: [`0x${"11".repeat(31)}`],
      }),
    /transactionMerkleBranch/,
  );
  assert.throws(
    () => canonicalTronSccpTransactionSourceProofBytes({ ...tronTransactionSourceInput, transactionRoot: HEX32_C }),
    /transactionRoot/,
  );
  assert.throws(
    () =>
      canonicalTronSccpTransactionSourceProofBytes({
        ...tronTransactionSourceInput,
        transactionRoot: "0xe4a77765ae41dc30b8bf3f7d9847170e0646e3dd0189433d2e3c88296221c942",
        transactionIndex: 1n,
        transactionCount: 3n,
        transactionBytes: "0x123456",
        transactionMerkleBranch: [`0x${"11".repeat(32)}`, `0x${"22".repeat(32)}`],
      }),
    /truncated protobuf bytes field/,
  );
  assert.equal(canonicalSubstrateSccpStorageProofBytes(substrateInput).length, 225);
  for (const [patch, pattern] of [
    [{ source_domain: SCCP_DOMAIN_SORA_KUSAMA }, /sourceDomain must not use multiple aliases/u],
    [{ source_event_digest: sourceEventDigest }, /sourceEventDigest must not use multiple aliases/u],
    [{ leaf_index: 0n }, /sourceEventLeafIndex must not use multiple aliases/u],
    [{ finality_height: 31n }, /finalizedBlockNumber must not use multiple aliases/u],
    [{ grandpa_set_id: 32n }, /grandpaSetId must not use multiple aliases/u],
    [{ finality_block_hash: HEX32_A }, /blockHash must not use multiple aliases/u],
    [{ authority_set_hash: HEX32_C }, /authoritySetHash must not use multiple aliases/u],
    [{ receipt_or_message_root: HEX32_B }, /eventsRoot must not use multiple aliases/u],
    [{ inclusion_branch: inclusionBranch }, /inclusionBranch must not use multiple aliases/u],
  ]) {
    assert.throws(() => canonicalSubstrateSccpStorageProofBytes({ ...substrateInput, ...patch }), pattern);
  }
  assert.equal(
    Buffer.from(
      canonicalSubstrateSccpRuntimeStorageVerificationStatementBytes(
        substrateRuntimeStorageInput,
      ),
    ).toString("hex"),
    Buffer.from(canonicalSubstrateSccpStorageProofBytes(substrateInput)).toString("hex"),
  );
  const substrateRuntimeStoragePublicInputsHash =
    substrateSccpRuntimeStorageProofPublicInputsHash(substrateRuntimeStorageInput);
  assert.match(substrateRuntimeStoragePublicInputsHash, /^0x[0-9a-f]{64}$/);
  const substrateRuntimeStorageColumns =
    substrateSccpRuntimeStoragePublicInputColumns(substrateRuntimeStorageInput);
  assert.equal(substrateRuntimeStorageColumns.length, 11);
  assert.deepEqual(substrateRuntimeStorageColumns[6], [
    substrateSccpStorageProofHash(substrateInput),
  ]);
  assert.deepEqual(substrateRuntimeStorageColumns[8], [
    "0x26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7",
  ]);
  assert.deepEqual(substrateRuntimeStorageColumns[10], [
    substrateRuntimeStoragePublicInputsHash,
  ]);
  const substrateRuntimeStorageContext =
    canonicalSubstrateSccpRuntimeStorageVerificationContextBytes(
      substrateRuntimeStorageInput,
    );
  assert.notEqual(
    Buffer.from(substrateRuntimeStorageContext).toString("hex"),
    Buffer.from(
      canonicalSubstrateSccpRuntimeStorageVerificationContextBytes({
        ...substrateRuntimeStorageInput,
        sourceStateVerifierHash: HEX32_G,
      }),
    ).toString("hex"),
  );
  const substrateRuntimeStorageRequest =
    buildSubstrateSccpRuntimeStorageProofRequest(substrateRuntimeStorageInput);
  assertImmutableFastpqProofRequest(
    substrateRuntimeStorageRequest,
    ["statementBytes", "verificationContextBytes", "schemaDescriptor"],
  );
  assert.equal(
    substrateRuntimeStorageRequest.circuitId,
    SCCP_SUBSTRATE_RUNTIME_STORAGE_OPEN_VERIFY_CIRCUIT_ID_V1,
  );
  assert.equal(
    substrateRuntimeStorageRequest.runtimeStorageProofPublicInputsHash,
    substrateRuntimeStoragePublicInputsHash,
  );
  assert.equal(substrateRuntimeStorageRequest.fastpqPublicInputs.slot, "31");
  assert.deepEqual(
    substrateRuntimeStorageRequest.fastpqTransitions.map((transition) => transition.key),
    [
      "sccp:substrate:runtime-storage:v1:context",
      "sccp:substrate:runtime-storage:v1:statement",
      "sccp:substrate:runtime-storage:v1:storage-key",
    ],
  );
  assert.equal(
    substrateSccpRuntimeStorageOpenVerifySchemaDescriptor(
      substrateRuntimeStorageInput,
    ).length,
    substrateRuntimeStorageRequest.schemaDescriptor.length,
  );
  assert.throws(
    () =>
      buildSubstrateSccpRuntimeStorageProofRequest({
        ...substrateRuntimeStorageInput,
        storageProofHash: substrateSccpStorageProofHash(substrateInput),
        storage_proof_hash: substrateSccpStorageProofHash(substrateInput),
      }),
    /storageProofHash must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      buildSubstrateSccpRuntimeStorageProofRequest({
        ...substrateRuntimeStorageInput,
        sourceVerifierMaterial: { sourceDomain: SCCP_DOMAIN_SORA_KUSAMA },
        source_verifier_material: { sourceDomain: SCCP_DOMAIN_SORA_KUSAMA },
      }),
    /sourceVerifierMaterial must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      substrateSccpRuntimeStorageOpenVerifySchemaDescriptor({
        sourceDomain: SCCP_DOMAIN_SORA_KUSAMA,
        source_domain: SCCP_DOMAIN_SORA_KUSAMA,
      }),
    /sourceDomain must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      buildSubstrateSccpRuntimeStorageProofRequest({
        ...substrateRuntimeStorageInput,
        storageProofHash: HEX32_A,
      }),
    /storageProofHash/,
  );
  assert.throws(
    () =>
      buildSubstrateSccpRuntimeStorageProofRequest({
        ...substrateRuntimeStorageInput,
        sourceStateVerifierHash:
          "0xaf2d28b3e07447239f28e90ce4fdee7e6cd3778c087eaeda7170781eb4b76b9c",
      }),
    /template verifier hash/,
  );
  assert.throws(
    () =>
      buildSubstrateSccpRuntimeStorageProofRequest({
        ...substrateRuntimeStorageInput,
        sourceVerifierMaterial: { sourceDomain: SCCP_DOMAIN_ETH },
      }),
    /sourceVerifierMaterial\.sourceDomain/,
  );
  assert.throws(
    () =>
      buildSubstrateSccpRuntimeStorageProofRequest({
        ...substrateRuntimeStorageInput,
        sourceVerifierMaterial: {
          sourceDomain: SCCP_DOMAIN_SORA_KUSAMA,
          source_domain: SCCP_DOMAIN_SORA_KUSAMA,
        },
      }),
    /sourceVerifierMaterial\.sourceDomain/,
  );

  const parentRawHeaderInput = {
    number: 12344n,
    txTrieRoot: HEX32_C,
    accountStateRoot: HEX32_A,
    parentBlockId: HEX32_B,
    witnessAddress: `0x41${"11".repeat(20)}`,
    headerVersion: 1,
    timestampMs: 1700000012344n,
  };
  const parentRawHeader = canonicalTronRawBlockHeaderBytes(parentRawHeaderInput);
  const rawHeaderInput = {
    number: 12345n,
    txTrieRoot: HEX32_D,
    accountStateRoot: `0x${"ee".repeat(32)}`,
    parentBlockId: TRON_PARENT_BLOCK_ID,
    witnessAddress: `0x41${"11".repeat(20)}`,
    headerVersion: 1,
    timestampMs: 1700000012345n,
  };
  const rawHeader = canonicalTronRawBlockHeaderBytes(rawHeaderInput);
  assert.equal(Buffer.from(parentRawHeader).toString("hex"), TRON_PARENT_RAW_HEADER_HEX);
  assert.equal(Buffer.from(rawHeader).toString("hex"), TRON_RAW_HEADER_HEX);
  for (const [patch, pattern] of [
    [{ blockNumber: 12345n }, /number must not use multiple aliases/u],
    [{ tx_trie_root: HEX32_D }, /txTrieRoot must not use multiple aliases/u],
    [{ account_state_root: `0x${"ee".repeat(32)}` }, /accountStateRoot must not use multiple aliases/u],
    [{ parent_block_id: TRON_PARENT_BLOCK_ID }, /parentBlockId must not use multiple aliases/u],
    [{ witness_address: `0x41${"11".repeat(20)}` }, /witnessAddress must not use multiple aliases/u],
    [{ header_version: 1 }, /headerVersion must not use multiple aliases/u],
    [{ timestamp_ms: 1700000012345n }, /timestampMs must not use multiple aliases/u],
  ]) {
    assert.throws(() => canonicalTronRawBlockHeaderBytes({ ...rawHeaderInput, ...patch }), pattern);
  }
  assert.equal(tronRawBlockHeaderHash(parentRawHeader), TRON_PARENT_RAW_HEADER_HASH);
  assert.equal(tronRawBlockHeaderHash(rawHeader), TRON_RAW_HEADER_HASH);
  assert.equal(tronBlockIdFromRawDataHash(12344n, TRON_PARENT_RAW_HEADER_HASH), TRON_PARENT_BLOCK_ID);
  assert.equal(tronBlockIdFromRawDataHash("12344", TRON_PARENT_RAW_HEADER_HASH), TRON_PARENT_BLOCK_ID);
  assert.equal(tronBlockIdFromRawDataHash(12345n, TRON_RAW_HEADER_HASH), TRON_BLOCK_ID);
  for (const blockNumber of ["012345", "0x3039", "+12345", " 12345", 12345.5]) {
    assert.throws(
      () => tronBlockIdFromRawDataHash(blockNumber, TRON_RAW_HEADER_HASH),
      /number must be (an unsigned integer|a non-negative safe integer)/,
    );
  }
  assert.throws(
    () =>
      canonicalTronRawBlockHeaderBytes({
        number: 12346n,
        txTrieRoot: HEX32_D,
        accountStateRoot: `0x${"ee".repeat(32)}`,
        parentBlockId: TRON_BLOCK_ID,
        witnessAddress: `0x41${"00".repeat(20)}`,
        headerVersion: 1,
        timestampMs: 1700000012346n,
      }),
    /TRON 0x41-prefixed address/,
  );
  const solidHeaderProof = {
    rawData: rawHeader,
    witnessSignature: tronHeaderSignature(0),
    parentRawData: parentRawHeader,
    parentWitnessSignature: tronHeaderSignature(27),
    rawDataHash: TRON_RAW_HEADER_HASH,
    parentRawDataHash: TRON_PARENT_RAW_HEADER_HASH,
    blockId: TRON_BLOCK_ID,
    txTrieRoot: HEX32_D,
    accountStateRoot: `0x${"ee".repeat(32)}`,
    parentBlockId: TRON_PARENT_BLOCK_ID,
    witnessAddress: `0x41${"11".repeat(20)}`,
    timestampMs: 1700000012345n,
    headerVersion: 1,
  };
  assert.equal(canonicalTronSolidBlockHeaderProofBytes(solidHeaderProof).length, 650);
  assert.equal(tronSolidBlockHeaderProofHash(solidHeaderProof), TRON_SOLID_BLOCK_HEADER_PROOF_HASH);
  for (const [patch, pattern] of [
    [{ raw_data: rawHeader }, /rawData must not use multiple aliases/u],
    [{ witness_signature: tronHeaderSignature(0) }, /witnessSignature must not use multiple aliases/u],
    [{ parent_raw_data: parentRawHeader }, /parentRawData must not use multiple aliases/u],
    [{ parent_witness_signature: tronHeaderSignature(27) }, /parentWitnessSignature must not use multiple aliases/u],
    [{ raw_data_hash: TRON_RAW_HEADER_HASH }, /rawDataHash must not use multiple aliases/u],
    [{ parent_raw_data_hash: TRON_PARENT_RAW_HEADER_HASH }, /parentRawDataHash must not use multiple aliases/u],
    [{ block_id: TRON_BLOCK_ID }, /blockId must not use multiple aliases/u],
    [{ tx_trie_root: HEX32_D }, /txTrieRoot must not use multiple aliases/u],
    [{ account_state_root: `0x${"ee".repeat(32)}` }, /accountStateRoot must not use multiple aliases/u],
    [{ parent_block_id: TRON_PARENT_BLOCK_ID }, /parentBlockId must not use multiple aliases/u],
    [{ witness_address: `0x41${"11".repeat(20)}` }, /witnessAddress must not use multiple aliases/u],
    [{ timestamp_ms: 1700000012345n }, /timestampMs must not use multiple aliases/u],
    [{ header_version: 1 }, /headerVersion must not use multiple aliases/u],
  ]) {
    assert.throws(() => canonicalTronSolidBlockHeaderProofBytes({ ...solidHeaderProof, ...patch }), pattern);
  }
  assert.throws(
    () =>
      canonicalTronSolidBlockHeaderProofBytes({
        ...solidHeaderProof,
        rawDataHash: HEX32_A,
      }),
    /rawDataHash must match rawData/,
  );
  const overlongKeyRawHeader = new Uint8Array(rawHeader.length + 1);
  overlongKeyRawHeader[0] = 0x88;
  overlongKeyRawHeader[1] = 0x00;
  overlongKeyRawHeader.set(rawHeader.slice(1), 2);
  const overlongKeyRawHeaderHash = tronRawBlockHeaderHash(overlongKeyRawHeader);
  assert.throws(
    () =>
      canonicalTronSolidBlockHeaderProofBytes({
        ...solidHeaderProof,
        rawData: overlongKeyRawHeader,
        rawDataHash: overlongKeyRawHeaderHash,
        blockId: tronBlockIdFromRawDataHash(12345n, overlongKeyRawHeaderHash),
      }),
    /protobuf varint must be canonical/,
  );
  assert.throws(
    () =>
      canonicalTronSolidBlockHeaderProofBytes({
        ...solidHeaderProof,
        witnessAddress: `0x41${"00".repeat(20)}`,
      }),
    /TRON 0x41-prefixed address/,
  );
  assert.throws(
    () =>
      canonicalTronSolidBlockHeaderProofBytes({
        ...solidHeaderProof,
        rawData: new Uint8Array(16 * 1024 + 1).fill(0xaa),
      }),
    /rawData and parentRawData must be at most/,
  );
  assert.throws(
    () =>
      canonicalTronSolidBlockHeaderProofBytes({
        ...solidHeaderProof,
        witnessSignature: new Uint8Array(65).fill(0xaa),
      }),
    /TRON header signatures must be canonical low-S/,
  );
  assert.throws(
    () =>
      canonicalTronSolidBlockHeaderProofBytes({
        ...solidHeaderProof,
        parentWitnessSignature: tronHeaderSignature(4),
      }),
    /TRON header signatures must be canonical low-S/,
  );
  assert.throws(
    () => canonicalTronSolidBlockHeaderProofBytes({ ...solidHeaderProof, version: 0 }),
    /version must be 1/,
  );
  const zeroRHeaderSignature = tronHeaderSignature(0);
  zeroRHeaderSignature.fill(0, 0, 32);
  assert.throws(
    () =>
      canonicalTronSolidBlockHeaderProofBytes({
        ...solidHeaderProof,
        witnessSignature: zeroRHeaderSignature,
      }),
    /TRON header signatures must be canonical low-S/,
  );

  assert.match(evmSccpReceiptProofHash(evmInput), /^0x[0-9a-f]{64}$/);
  assert.notEqual(
    evmSccpReceiptProofHash(evmInput),
    evmSccpReceiptProofHash({ ...evmInput, inclusionBranch: [HEX32_F] }),
  );
  assert.notEqual(
    bscSccpReceiptProofHash(bscInput),
    bscSccpReceiptProofHash({ ...bscInput, inclusionBranch: [HEX32_F] }),
  );
  assert.throws(
    () => canonicalSccpMessageTransparentPublicInputsBytes({ ...sampleEvmPublicInputs, version: 0 }),
    /publicInputs\.version/,
  );
  assert.throws(
    () => canonicalSccpMessageTransparentPublicInputsBytes({ ...sampleEvmPublicInputs, version: null }),
    /publicInputs\.version must be 1/,
  );
  const validatorPayloadInput = {
    validatorAddresses: [`0x${"11".repeat(20)}`, `0x${"22".repeat(20)}`],
    validatorPowers: [1n, 2n],
  };
  const validatorPayload = canonicalBscValidatorSetPayloadBytes(validatorPayloadInput);
  assert.equal(Buffer.from(validatorPayload).toString("hex"), BSC_VALIDATOR_SET_PAYLOAD_HEX);
  assert.equal(bscValidatorSetPayloadHash(validatorPayloadInput), BSC_VALIDATOR_SET_PAYLOAD_HASH);
  assert.equal(bscValidatorSetPayloadHash(validatorPayload), BSC_VALIDATOR_SET_PAYLOAD_HASH);
  assert.equal(bscValidatorSetHashFromPayload(validatorPayloadInput), BSC_VALIDATOR_SET_HASH);
  assert.throws(
    () =>
      canonicalBscValidatorSetPayloadBytes({
        ...validatorPayloadInput,
        validator_addresses: validatorPayloadInput.validatorAddresses,
      }),
    /validatorAddresses must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      canonicalBscValidatorSetPayloadBytes({
        ...validatorPayloadInput,
        validator_powers: validatorPayloadInput.validatorPowers,
      }),
    /validatorPowers must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      canonicalBscValidatorSetPayloadBytes({
        validatorAddresses: Array.from({ length: 256 }, (_, index) =>
          `0x${(index + 1).toString(16).padStart(40, "0")}`,
        ),
        validatorPowers: Array.from({ length: 256 }, () => 1n),
      }),
    /at most 255/,
  );
  const parliaPayload = canonicalBscValidatorSetPayloadBytes({
    validatorAddresses: validatorPayloadInput.validatorAddresses,
    validatorPowers: [1n, 1n],
  });
  const parliaExtra = sampleBscParliaExtra();
  assert.equal(
    Buffer.from(bscValidatorSetPayloadFromParliaExtra(parliaExtra)).toString("hex"),
    Buffer.from(parliaPayload).toString("hex"),
  );
  assert.equal(
    Buffer.from(bscValidatorSetPayloadFromHeaderRlp(sampleBscParliaHeaderRlp(parliaExtra))).toString("hex"),
    Buffer.from(parliaPayload).toString("hex"),
  );
  assert.throws(() => bscValidatorSetPayloadFromHeaderRlp(Uint8Array.from([0x80])), /RLP list/);
  assert.throws(
    () =>
      canonicalBscValidatorSetPayloadBytes({
        validatorAddresses: [`0x${"11".repeat(20)}`, `0x${"11".repeat(20)}`],
        validatorPowers: [1n, 2n],
      }),
    /unique/,
  );
  assert.throws(
    () =>
      canonicalBscValidatorSetPayloadBytes({
        validatorAddresses: [`0x${"11".repeat(20)}`],
        validatorPowers: [0n],
      }),
    /must not be zero/,
  );
  const commitMessage = {
    validatorEpoch: 2n,
    blockNumber: 401n,
    blockHash: `0x${"22".repeat(32)}`,
    receiptsRoot: `0x${"33".repeat(32)}`,
    validatorSetHash: BSC_COMMIT_VALIDATOR_SET_HASH,
  };
  assert.equal(canonicalBscCommitMessageBytes(commitMessage).length, 117);
  assert.equal(bscCommitMessageHash(commitMessage), BSC_COMMIT_MESSAGE_HASH);
  assert.throws(
    () => bscCommitMessageHash({ ...commitMessage, sourceDomain: SCCP_DOMAIN_ETH }),
    /sourceDomain/,
  );
  assert.throws(
    () => bscCommitMessageHash({ ...commitMessage, validator_epoch: 2n }),
    /validatorEpoch must not use multiple aliases/u,
  );
  assert.throws(
    () => bscCommitMessageHash({ ...commitMessage, validator_set_hash: BSC_COMMIT_VALIDATOR_SET_HASH }),
    /validatorSetHash must not use multiple aliases/u,
  );
  const commitSeal = {
    totalPower: 4n,
    signedPower: 3n,
    commitMessageHash: BSC_COMMIT_MESSAGE_HASH,
    validatorPublicKeys: BSC_COMMIT_VALIDATOR_PUBLIC_KEYS,
    validatorPowers: BSC_COMMIT_VALIDATOR_POWERS,
    signersBitmap: "0x07",
    signatures: BSC_COMMIT_SIGNATURES,
    validatorSetHash: BSC_COMMIT_VALIDATOR_SET_HASH,
  };
  assert.equal(canonicalBscCommitSealBytes(commitSeal).length, 297);
  assert.equal(bscCommitSealHash(commitSeal), BSC_COMMIT_SEAL_HASH);
  assert.throws(
    () =>
      canonicalBscCommitSealBytes({
        ...commitSeal,
        signedPower: 2n,
        signersBitmap: "0x03",
        signatures: BSC_COMMIT_SIGNATURES.slice(0, 2),
      }),
    /two thirds/,
  );
  assert.throws(
    () => canonicalBscCommitSealBytes({ ...commitSeal, signersBitmap: "0x1f" }),
    /padding bits/,
  );
  assert.throws(
    () =>
      canonicalBscCommitSealBytes({
        ...commitSeal,
        signatures: [
          `0x31${BSC_COMMIT_SIGNATURES[0].slice(4)}`,
          ...BSC_COMMIT_SIGNATURES.slice(1),
        ],
      }),
    /recover/,
  );
  assert.throws(
    () => canonicalBscCommitSealBytes({ ...commitSeal, validatorSetHash: HEX32_A }),
    /validatorSetHash/,
  );
  assert.throws(
    () => canonicalBscCommitSealBytes({ ...commitSeal, total_power: 4n }),
    /totalPower must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      canonicalBscCommitSealBytes({
        ...commitSeal,
        validator_set_hash: BSC_COMMIT_VALIDATOR_SET_HASH,
      }),
    /validatorSetHash must not use multiple aliases/u,
  );
  const storageValue = "0x02";
  const storageValueHash = bscValidatorSetStorageValueHash(storageValue);
  const metadataProof = {
    stateRoot: HEX32_A,
    nextValidatorSetPayloadHash: BSC_VALIDATOR_SET_PAYLOAD_HASH,
    validatorContractAddress: `0x${"00".repeat(18)}1000`,
    accountProofNodes: [`0xf842a0${"11".repeat(32)}`],
    storageRoot: HEX32_B,
    validatorSetLengthSlot: HEX32_C,
    validatorSetLengthValue: storageValue,
    validatorSetLengthValueHash: storageValueHash,
    validatorSetLengthProofNodes: [`0xe4822080a0${"22".repeat(32)}`],
    validatorStorageProofs: [
      {
        validatorIndex: 0,
        storageSlot: HEX32_D,
        storageValue: `0x94${"11".repeat(20)}`,
        storageValueHash: bscValidatorSetStorageValueHash(`0x94${"11".repeat(20)}`),
        storageProofNodes: [`0xe4822080a0${"33".repeat(32)}`],
      },
      {
        validatorIndex: 1,
        storageSlot: HEX32_E,
        storageValue: `0x94${"22".repeat(20)}`,
        storageValueHash: bscValidatorSetStorageValueHash(`0x94${"22".repeat(20)}`),
        storageProofNodes: [`0xe4822080a0${"44".repeat(32)}`],
      },
    ],
  };
  assert.equal(canonicalBscValidatorSetMetadataProofBytes(metadataProof).length, 560);
  const metadataHash = bscValidatorSetMetadataProofHash(metadataProof);
  assert.throws(
    () => canonicalBscValidatorSetMetadataProofBytes({ ...metadataProof, version: 0 }),
    /BSC ValidatorSet metadata proof version/,
  );
  assert.throws(
    () => canonicalBscValidatorSetMetadataProofBytes({ ...metadataProof, state_root: HEX32_A }),
    /stateRoot must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      canonicalBscValidatorSetMetadataProofBytes({
        ...metadataProof,
        validator_set_length_proof_nodes: metadataProof.validatorSetLengthProofNodes,
      }),
    /validatorSetLengthProofNodes must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      canonicalBscValidatorSetMetadataProofBytes({
        ...metadataProof,
        validator_storage_proofs: metadataProof.validatorStorageProofs,
      }),
    /validatorStorageProofs must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      canonicalBscValidatorSetMetadataProofBytes({
        ...metadataProof,
        validatorStorageProofs: [
          {
            ...metadataProof.validatorStorageProofs[0],
            version: 0,
          },
        ],
      }),
    /BSC validator storage proof version/,
  );
  assert.throws(
    () =>
      canonicalBscValidatorSetMetadataProofBytes({
        ...metadataProof,
        validatorSetLengthValueHash: HEX32_F,
      }),
    /validatorSetLengthValueHash/,
  );
  assert.throws(
    () =>
      canonicalBscValidatorSetMetadataProofBytes({
        ...metadataProof,
        validatorStorageProofs: [
          {
            ...metadataProof.validatorStorageProofs[0],
            storage_proof_nodes: metadataProof.validatorStorageProofs[0].storageProofNodes,
          },
        ],
      }),
    /storageProofNodes must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      canonicalBscValidatorSetMetadataProofBytes({
        ...metadataProof,
        validatorStorageProofs: [
          {
            ...metadataProof.validatorStorageProofs[0],
            storage_value_hash: metadataProof.validatorStorageProofs[0].storageValueHash,
          },
        ],
      }),
    /storageValueHash must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      canonicalBscValidatorSetMetadataProofBytes({
        ...metadataProof,
        validatorStorageProofs: [
          {
            ...metadataProof.validatorStorageProofs[0],
            storageValueHash: HEX32_F,
          },
        ],
      }),
    /storageValueHash/,
  );
  assert.notEqual(
    metadataHash,
    bscValidatorSetMetadataProofHash({ ...metadataProof, stateRoot: HEX32_B }),
  );
  const transitionMessage = {
    fromValidatorEpoch: 41n,
    toValidatorEpoch: 42n,
    transitionBlockNumber: 8400n,
    transitionBlockHash: HEX32_A,
    parentValidatorSetHash: HEX32_B,
    nextValidatorSetHash: BSC_VALIDATOR_SET_HASH,
    nextValidatorSetPayloadHash: BSC_VALIDATOR_SET_PAYLOAD_HASH,
    validatorSetMetadataProofHash: metadataHash,
  };
  assert.equal(canonicalBscValidatorSetTransitionMessageBytes(transitionMessage).length, 189);
  assert.throws(
    () =>
      canonicalBscValidatorSetTransitionMessageBytes({
        ...transitionMessage,
        sourceDomain: SCCP_DOMAIN_BSC,
        source_domain: SCCP_DOMAIN_BSC,
      }),
    /sourceDomain must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      canonicalBscValidatorSetTransitionMessageBytes({
        ...transitionMessage,
        from_validator_epoch: 41n,
      }),
    /fromValidatorEpoch must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      canonicalBscValidatorSetTransitionMessageBytes({
        ...transitionMessage,
        next_validator_set_payload_hash: BSC_VALIDATOR_SET_PAYLOAD_HASH,
      }),
    /nextValidatorSetPayloadHash must not use multiple aliases/u,
  );
  assert.throws(
    () => canonicalBscValidatorSetTransitionMessageBytes({ ...transitionMessage, version: 0 }),
    /BSC validator-set transition message version/,
  );
  assert.notEqual(
    bscValidatorSetTransitionMessageHash(transitionMessage),
    bscValidatorSetTransitionMessageHash({
      ...transitionMessage,
      validatorSetMetadataProofHash: HEX32_C,
    }),
  );
  assert.throws(
    () => bscValidatorSetTransitionMessageHash({
      ...transitionMessage,
      transitionBlockNumber: 8401n,
    }),
    /epoch-start block/,
  );
  assert.throws(
    () => bscValidatorSetTransitionMessageHash({
      ...transitionMessage,
      toValidatorEpoch: 43n,
    }),
    /fromValidatorEpoch/,
  );
  assert.throws(
    () => bscValidatorSetTransitionMessageHash({
      ...transitionMessage,
      sourceDomain: 0,
    }),
    /sourceDomain/,
  );
  const witnessPayloadInput = {
    witnessAddresses: [`0x41${"11".repeat(20)}`, `0x41${"22".repeat(20)}`],
    witnessWeights: [1n, 2n],
  };
  const witnessPayload = canonicalTronWitnessSchedulePayloadBytes(witnessPayloadInput);
  assert.equal(Buffer.from(witnessPayload).toString("hex"), TRON_WITNESS_SCHEDULE_PAYLOAD_HEX);
  assert.equal(
    tronWitnessSchedulePayloadHash(witnessPayloadInput),
    TRON_WITNESS_SCHEDULE_PAYLOAD_HASH,
  );
  assert.equal(tronWitnessSchedulePayloadHash(witnessPayload), TRON_WITNESS_SCHEDULE_PAYLOAD_HASH);
  assert.equal(tronWitnessScheduleHashFromPayload(witnessPayloadInput), TRON_WITNESS_SCHEDULE_HASH);
  assert.throws(
    () =>
      canonicalTronWitnessSchedulePayloadBytes({
        ...witnessPayloadInput,
        witness_addresses: witnessPayloadInput.witnessAddresses,
      }),
    /witnessAddresses must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      canonicalTronWitnessSchedulePayloadBytes({
        ...witnessPayloadInput,
        witness_weights: witnessPayloadInput.witnessWeights,
      }),
    /witnessWeights must not use multiple aliases/u,
  );
  const zeroWitnessPayload = Uint8Array.from(
    Buffer.from(`010100000041${"00".repeat(20)}0100000000000000`, "hex"),
  );
  assert.throws(
    () => tronWitnessSchedulePayloadHash(zeroWitnessPayload),
    /TRON 0x41-prefixed address/,
  );
  assert.throws(
    () => tronWitnessScheduleHashFromPayload(zeroWitnessPayload),
    /TRON 0x41-prefixed address/,
  );
  assert.throws(
    () =>
      canonicalTronWitnessSchedulePayloadBytes({
        witnessAddresses: Array.from(
          { length: 65 },
          (_, index) => `0x41${"11".repeat(19)}${index.toString(16).padStart(2, "0")}`,
        ),
        witnessWeights: Array.from({ length: 65 }, () => 1n),
      }),
    /at most 64/,
  );
  assert.throws(
    () =>
      canonicalTronWitnessSchedulePayloadBytes({
        witnessAddresses: [`0x41${"11".repeat(20)}`, `0x41${"11".repeat(20)}`],
        witnessWeights: [1n, 2n],
      }),
    /unique/,
  );
  assert.throws(
    () =>
      canonicalTronWitnessSchedulePayloadBytes({
        witnessAddresses: [`0x41${"00".repeat(20)}`],
        witnessWeights: [1n],
      }),
    /TRON 0x41-prefixed address/,
  );
  assert.throws(
    () =>
      canonicalTronWitnessSchedulePayloadBytes({
        witnessAddresses: [`0x41${"11".repeat(20)}`],
        witnessWeights: [0n],
      }),
    /must not be zero/,
  );
  assert.throws(
    () =>
      canonicalTronWitnessSchedulePayloadBytes({
        witnessAddresses: [`0x41${"11".repeat(20)}`, `0x41${"22".repeat(20)}`],
        witnessWeights: [(1n << 64n) - 1n, 1n],
      }),
    /total must fit u64/,
  );
  const overflowingWitnessPayload = Uint8Array.from(
    Buffer.from(
      `010200000041${"11".repeat(20)}ffffffffffffffff41${"22".repeat(20)}0100000000000000`,
      "hex",
    ),
  );
  assert.throws(
    () => tronWitnessSchedulePayloadHash(overflowingWitnessPayload),
    /total weight must fit u64/,
  );
  assert.throws(
    () => tronWitnessScheduleHashFromPayload(overflowingWitnessPayload),
    /total weight must fit u64/,
  );
  const tronSolidMessage = {
    sourceDomain: SCCP_DOMAIN_TRON,
    solidBlockNumber: 12345n,
    blockHash: TRON_BLOCK_ID,
    witnessScheduleHash: TRON_WITNESS_SCHEDULE_HASH,
    receiptRoot: HEX32_B,
    transactionRoot: HEX32_D,
    receiptProofHash: HEX32_C,
  };
  assert.equal(canonicalTronSolidBlockMessageBytes(tronSolidMessage).length, 173);
  assert.equal(tronSolidBlockMessageHash(tronSolidMessage), TRON_SOLID_BLOCK_MESSAGE_HASH);
  for (const [patch, pattern] of [
    [{ source_domain: SCCP_DOMAIN_TRON }, /sourceDomain must not use multiple aliases/u],
    [{ solid_block_number: 12345n }, /solidBlockNumber must not use multiple aliases/u],
    [{ block_hash: TRON_BLOCK_ID }, /blockHash must not use multiple aliases/u],
    [{ witness_schedule_hash: TRON_WITNESS_SCHEDULE_HASH }, /witnessScheduleHash must not use multiple aliases/u],
    [{ receipt_root: HEX32_B }, /receiptRoot must not use multiple aliases/u],
    [{ transaction_root: HEX32_D }, /transactionRoot must not use multiple aliases/u],
    [{ receipt_proof_hash: HEX32_C }, /receiptProofHash must not use multiple aliases/u],
  ]) {
    assert.throws(() => canonicalTronSolidBlockMessageBytes({ ...tronSolidMessage, ...patch }), pattern);
  }
  assert.throws(
    () => canonicalTronSolidBlockMessageBytes({ ...tronSolidMessage, sourceDomain: SCCP_DOMAIN_ETH }),
    /sourceDomain must be TRON/,
  );
  assert.throws(
    () => canonicalTronSolidBlockMessageBytes({ ...tronSolidMessage, receiptRoot: SCCP_ZERO_HASH_V1 }),
    /receiptRoot must not be zero/,
  );
  const tronWitnessSeal = {
    version: 1,
    totalWeight: 1n,
    signedWeight: 1n,
    solidBlockMessageHash: `0x${TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR}`,
    witnessAddresses: [TRON_TEST_OWNER_ADDRESS],
    witnessWeights: [1n],
    signersBitmap: "0x01",
    signatures: [`0x${TRON_SOURCE_EVENT_SIGNATURE_VECTOR}`],
  };
  assert.equal(canonicalTronWitnessSealBytes(tronWitnessSeal).length, 200);
  assert.equal(tronWitnessSealHash(tronWitnessSeal), TRON_WITNESS_SEAL_HASH);
  for (const [patch, pattern] of [
    [{ total_weight: 1n }, /totalWeight must not use multiple aliases/u],
    [{ signed_weight: 1n }, /signedWeight must not use multiple aliases/u],
    [{ solid_block_message_hash: `0x${TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR}` }, /solidBlockMessageHash must not use multiple aliases/u],
    [{ witness_addresses: [TRON_TEST_OWNER_ADDRESS] }, /witnessAddresses must not use multiple aliases/u],
    [{ witness_weights: [1n] }, /witnessWeights must not use multiple aliases/u],
    [{ signers_bitmap: "0x01" }, /signersBitmap must not use multiple aliases/u],
  ]) {
    assert.throws(() => canonicalTronWitnessSealBytes({ ...tronWitnessSeal, ...patch }), pattern);
  }
  assert.throws(
    () => canonicalTronWitnessSealBytes({ ...tronWitnessSeal, signatures: [] }),
    /signatures length/,
  );
  assert.throws(
    () =>
      canonicalTronWitnessSealBytes({
        ...tronWitnessSeal,
        witnessAddresses: [`0x41${"11".repeat(20)}`],
      }),
    /declared signer/,
  );
  const authorityPayloadInput = {
    authorityPublicKeys: [`0x${"11".repeat(32)}`, `0x${"22".repeat(32)}`],
    authorityWeights: [1n, 2n],
  };
  const authorityPayload = canonicalSubstrateAuthoritySetPayloadBytes(authorityPayloadInput);
  assert.equal(
    Buffer.from(authorityPayload).toString("hex"),
    SUBSTRATE_AUTHORITY_SET_PAYLOAD_HEX,
  );
  assert.equal(
    substrateAuthoritySetPayloadHash(authorityPayloadInput),
    SUBSTRATE_AUTHORITY_SET_PAYLOAD_HASH,
  );
  assert.equal(
    substrateAuthoritySetPayloadHash(authorityPayload),
    SUBSTRATE_AUTHORITY_SET_PAYLOAD_HASH,
  );
  assert.equal(
    substrateAuthoritySetHashFromPayload(authorityPayloadInput),
    SUBSTRATE_AUTHORITY_SET_HASH,
  );
  assert.throws(
    () =>
      canonicalSubstrateAuthoritySetPayloadBytes({
        ...authorityPayloadInput,
        authority_public_keys: authorityPayloadInput.authorityPublicKeys,
      }),
    /authorityPublicKeys must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      canonicalSubstrateAuthoritySetPayloadBytes({
        ...authorityPayloadInput,
        authority_weights: authorityPayloadInput.authorityWeights,
      }),
    /authorityWeights must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      canonicalSubstrateAuthoritySetPayloadBytes({
        authorityPublicKeys: [`0x${"11".repeat(32)}`, `0x${"11".repeat(32)}`],
        authorityWeights: [1n, 2n],
      }),
    /unique/,
  );
  assert.throws(
    () =>
      canonicalSubstrateAuthoritySetPayloadBytes({
        authorityPublicKeys: [`0x${"00".repeat(32)}`],
        authorityWeights: [1n],
      }),
    /must not be zero/,
  );
  const zeroAuthorityPayload = new Uint8Array(45);
  zeroAuthorityPayload[0] = 1;
  zeroAuthorityPayload[1] = 1;
  zeroAuthorityPayload[37] = 1;
  assert.throws(
    () => substrateAuthoritySetHashFromPayload(zeroAuthorityPayload),
    /must not be zero/,
  );
  assert.throws(
    () =>
      canonicalSubstrateAuthoritySetPayloadBytes({
        authorityPublicKeys: [`0x${"11".repeat(32)}`],
        authorityWeights: [0n],
      }),
    /must not be zero/,
  );
  assert.throws(
    () =>
      canonicalSubstrateAuthoritySetPayloadBytes({
        authorityPublicKeys: Array.from({ length: 2049 }, () => `0x${"11".repeat(32)}`),
        authorityWeights: Array.from({ length: 2049 }, () => 1n),
      }),
    /at most 2048/,
  );
  assert.notEqual(
    tronSccpReceiptProofHash(tronInput),
    tronSccpReceiptProofHash({ ...tronInput, inclusionBranch: [HEX32_F] }),
  );
  assert.notEqual(
    tronSccpReceiptStateProofHash(tronReceiptStateInput),
    tronSccpReceiptStateProofHash({ ...tronReceiptStateInput, receiptRootIndex: 1n }),
  );
  assert.throws(
    () =>
      canonicalTronSccpReceiptStateProofBytes({
        ...tronReceiptStateInput,
        receiptTrieProofNodes: [],
      }),
    /receiptTrieProofNodes must not be empty/,
  );
  assert.throws(
    () =>
      canonicalTronSccpTransactionSourceProofBytes({
        ...tronTransactionSourceInput,
        inclusionBranch: [],
      }),
    /inclusionBranch must not be empty/,
  );
  assert.notEqual(
    substrateSccpStorageProofHash(substrateInput),
    substrateSccpStorageProofHash({ ...substrateInput, inclusionBranch: [HEX32_F] }),
  );
  assert.notEqual(
    substrateSccpStorageProofHash(substrateInput),
    substrateSccpStorageProofHash({ ...substrateInput, sourceEventLeafIndex: 1n }),
  );
  assert.throws(
    () => canonicalSubstrateSccpStorageProofBytes({ ...substrateInput, sourceDomain: undefined }),
    /sourceDomain must be a u32 domain id/,
  );
});

test("derives TRON witness-schedule transition transcript hashes from UI witness material", () => {
  const parentPayload = Buffer.from(TRON_PARENT_WITNESS_SCHEDULE_PAYLOAD_HEX, "hex");
  const nextPayload = Buffer.from(TRON_WITNESS_SCHEDULE_PAYLOAD_HEX, "hex");
  assert.equal(
    tronWitnessScheduleHashFromPayload(parentPayload),
    TRON_PARENT_WITNESS_SCHEDULE_HASH,
  );
  assert.equal(tronWitnessScheduleHashFromPayload(nextPayload), TRON_WITNESS_SCHEDULE_HASH);
  assert.equal(tronWitnessSchedulePayloadHash(nextPayload), TRON_WITNESS_SCHEDULE_PAYLOAD_HASH);

  const messageInput = {
    sourceDomain: SCCP_DOMAIN_TRON,
    fromWitnessScheduleEpoch: 7n,
    toWitnessScheduleEpoch: 8n,
    transitionBlockNumber: 12345n,
    transitionBlockHash: TRON_BLOCK_ID,
    parentWitnessScheduleHash: TRON_PARENT_WITNESS_SCHEDULE_HASH,
    nextWitnessScheduleHash: TRON_WITNESS_SCHEDULE_HASH,
    nextWitnessSchedulePayload: nextPayload,
  };
  const u32le = (value) => {
    const out = Buffer.alloc(4);
    out.writeUInt32LE(value);
    return out;
  };
  const u64le = (value) => {
    const out = Buffer.alloc(8);
    out.writeBigUInt64LE(BigInt(value));
    return out;
  };
  const expectedMessage = Buffer.concat([
    Buffer.from([1]),
    u32le(SCCP_DOMAIN_TRON),
    u64le(7n),
    u64le(8n),
    u64le(12345n),
    Buffer.from(TRON_BLOCK_ID.slice(2), "hex"),
    Buffer.from(TRON_PARENT_WITNESS_SCHEDULE_HASH.slice(2), "hex"),
    Buffer.from(TRON_WITNESS_SCHEDULE_HASH.slice(2), "hex"),
    Buffer.from(TRON_WITNESS_SCHEDULE_PAYLOAD_HASH.slice(2), "hex"),
  ]);
  assert.deepEqual(
    Buffer.from(canonicalTronWitnessScheduleTransitionMessageBytes(messageInput)),
    expectedMessage,
  );
  assert.equal(expectedMessage.length, 157);
  assert.equal(
    tronWitnessScheduleTransitionMessageHash(messageInput),
    TRON_WITNESS_SCHEDULE_TRANSITION_MESSAGE_HASH,
  );
  for (const [patch, pattern] of [
    [{ source_domain: SCCP_DOMAIN_TRON }, /sourceDomain must not use multiple aliases/u],
    [{ from_witness_schedule_epoch: 7n }, /fromWitnessScheduleEpoch must not use multiple aliases/u],
    [{ to_witness_schedule_epoch: 8n }, /toWitnessScheduleEpoch must not use multiple aliases/u],
    [{ transition_block_number: 12345n }, /transitionBlockNumber must not use multiple aliases/u],
    [{ transition_block_hash: TRON_BLOCK_ID }, /transitionBlockHash must not use multiple aliases/u],
    [{ parent_witness_schedule_hash: TRON_PARENT_WITNESS_SCHEDULE_HASH }, /parentWitnessScheduleHash must not use multiple aliases/u],
    [{ next_witness_schedule_hash: TRON_WITNESS_SCHEDULE_HASH }, /nextWitnessScheduleHash must not use multiple aliases/u],
    [{ next_witness_schedule_payload: nextPayload }, /nextWitnessSchedulePayload must not use multiple aliases/u],
  ]) {
    assert.throws(
      () => canonicalTronWitnessScheduleTransitionMessageBytes({ ...messageInput, ...patch }),
      pattern,
    );
  }
  assert.throws(
    () =>
      canonicalTronWitnessScheduleTransitionMessageBytes({
        ...messageInput,
        nextWitnessSchedulePayloadHash: TRON_WITNESS_SCHEDULE_PAYLOAD_HASH,
        next_witness_schedule_payload_hash: TRON_WITNESS_SCHEDULE_PAYLOAD_HASH,
      }),
    /nextWitnessSchedulePayloadHash must not use multiple aliases/u,
  );
  assert.notEqual(
    tronWitnessScheduleTransitionMessageHash({ ...messageInput, transitionBlockHash: HEX32_D }),
    tronWitnessScheduleTransitionMessageHash(messageInput),
  );
  assert.throws(
    () =>
      canonicalTronWitnessScheduleTransitionMessageBytes({
        ...messageInput,
        toWitnessScheduleEpoch: 9n,
      }),
    /toWitnessScheduleEpoch/,
  );
  assert.throws(
    () =>
      canonicalTronWitnessScheduleTransitionMessageBytes({
        ...messageInput,
        sourceDomain: SCCP_DOMAIN_ETH,
      }),
    /sourceDomain/,
  );
  assert.throws(
    () =>
      canonicalTronWitnessScheduleTransitionMessageBytes({
        ...messageInput,
        nextWitnessScheduleHash: HEX32_D,
      }),
    /nextWitnessScheduleHash/,
  );

  const sealInput = {
    ...messageInput,
    nextWitnessSchedulePayloadHash: TRON_WITNESS_SCHEDULE_PAYLOAD_HASH,
    transitionMessageHash: TRON_WITNESS_SCHEDULE_TRANSITION_MESSAGE_HASH,
    sealProof: {
      version: 1,
      totalWeight: 1n,
      signedWeight: 1n,
      solidBlockMessageHash: TRON_WITNESS_SCHEDULE_TRANSITION_MESSAGE_HASH,
      witnessAddresses: [TRON_TEST_OWNER_ADDRESS],
      witnessWeights: [1n],
      signersBitmap: "0x01",
      signatures: [TRON_WITNESS_SCHEDULE_TRANSITION_SIGNATURE],
    },
  };
  assert.equal(canonicalTronWitnessScheduleTransitionSealBytes(sealInput).length, 456);
  assert.equal(
    tronWitnessScheduleTransitionSealHash(sealInput),
    TRON_WITNESS_SCHEDULE_TRANSITION_SEAL_HASH,
  );
  for (const [patch, pattern] of [
    [{ next_witness_schedule_payload_hash: TRON_WITNESS_SCHEDULE_PAYLOAD_HASH }, /nextWitnessSchedulePayloadHash must not use multiple aliases/u],
    [{ transition_message_hash: TRON_WITNESS_SCHEDULE_TRANSITION_MESSAGE_HASH }, /transitionMessageHash must not use multiple aliases/u],
    [{ seal_proof: sealInput.sealProof }, /sealProof must not use multiple aliases/u],
    [{ witnessSealProof: sealInput.sealProof }, /sealProof must not use multiple aliases/u],
  ]) {
    assert.throws(() => canonicalTronWitnessScheduleTransitionSealBytes({ ...sealInput, ...patch }), pattern);
  }
  assert.throws(
    () =>
      canonicalTronWitnessScheduleTransitionSealBytes({
        ...sealInput,
        sealProof: { ...sealInput.sealProof, total_weight: 1n },
      }),
    /totalWeight must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      canonicalTronWitnessScheduleTransitionSealBytes({
        ...sealInput,
        transitionMessageHash: HEX32_D,
      }),
    /transitionMessageHash/,
  );
  const badSignature = `0x${(
    Number.parseInt(TRON_WITNESS_SCHEDULE_TRANSITION_SIGNATURE.slice(2, 4), 16) ^ 1
  ).toString(16).padStart(2, "0")}${TRON_WITNESS_SCHEDULE_TRANSITION_SIGNATURE.slice(4)}`;
  assert.throws(
    () =>
      canonicalTronWitnessScheduleTransitionSealBytes({
        ...sealInput,
        sealProof: { ...sealInput.sealProof, signatures: [badSignature] },
      }),
    /declared signer/,
  );
  assert.throws(
    () =>
      canonicalTronWitnessScheduleTransitionSealBytes({
        ...sealInput,
        nextWitnessSchedulePayloadHash: HEX32_D,
      }),
    /nextWitnessSchedulePayloadHash/,
  );
});

test("derives Substrate authority-set transition transcript hashes from UI witness material", () => {
  const parent = {
    authorityPublicKeys: [`0x${"11".repeat(32)}`, `0x${"22".repeat(32)}`, `0x${"33".repeat(32)}`],
    authorityWeights: [5n, 7n, 11n],
  };
  const nextSet = {
    authorityPublicKeys: [`0x${"aa".repeat(32)}`, `0x${"bb".repeat(32)}`, `0x${"cc".repeat(32)}`],
    authorityWeights: [13n, 17n, 19n],
  };
  const parentPayload = canonicalSubstrateAuthoritySetPayloadBytes(parent);
  const nextPayload = canonicalSubstrateAuthoritySetPayloadBytes(nextSet);

  assert.equal(Buffer.from(parentPayload).toString("hex"), SUBSTRATE_PARENT_AUTHORITY_SET_PAYLOAD_HEX);
  assert.equal(Buffer.from(nextPayload).toString("hex"), SUBSTRATE_NEXT_AUTHORITY_SET_PAYLOAD_HEX);
  assert.equal(substrateAuthoritySetHashFromPayload(parentPayload), SUBSTRATE_PARENT_AUTHORITY_SET_HASH);
  assert.equal(substrateAuthoritySetHashFromPayload(nextPayload), SUBSTRATE_NEXT_AUTHORITY_SET_HASH);
  assert.equal(substrateAuthoritySetPayloadHash(nextPayload), SUBSTRATE_NEXT_AUTHORITY_SET_PAYLOAD_HASH);

  const messageInput = {
    sourceDomain: SCCP_DOMAIN_SORA_KUSAMA,
    fromGrandpaSetId: 41n,
    toGrandpaSetId: 42n,
    transitionBlockNumber: 9001n,
    transitionBlockHash: `0x${"44".repeat(32)}`,
    parentAuthoritySetHash: SUBSTRATE_PARENT_AUTHORITY_SET_HASH,
    nextAuthoritySetHash: SUBSTRATE_NEXT_AUTHORITY_SET_HASH,
    nextAuthoritySetPayloadHash: SUBSTRATE_NEXT_AUTHORITY_SET_PAYLOAD_HASH,
  };
  assert.equal(canonicalSubstrateAuthoritySetTransitionMessageBytes(messageInput).length, 157);
  assert.equal(
    substrateAuthoritySetTransitionMessageHash(messageInput),
    SUBSTRATE_AUTHORITY_SET_TRANSITION_MESSAGE_HASH,
  );
  for (const [patch, pattern] of [
    [{ source_domain: SCCP_DOMAIN_SORA_KUSAMA }, /sourceDomain must not use multiple aliases/u],
    [{ from_grandpa_set_id: 41n }, /fromGrandpaSetId must not use multiple aliases/u],
    [{ to_grandpa_set_id: 42n }, /toGrandpaSetId must not use multiple aliases/u],
    [{ transition_block_number: 9001n }, /transitionBlockNumber must not use multiple aliases/u],
    [{ transition_block_hash: `0x${"44".repeat(32)}` }, /transitionBlockHash must not use multiple aliases/u],
    [{ parent_authority_set_hash: SUBSTRATE_PARENT_AUTHORITY_SET_HASH }, /parentAuthoritySetHash must not use multiple aliases/u],
    [{ next_authority_set_hash: SUBSTRATE_NEXT_AUTHORITY_SET_HASH }, /nextAuthoritySetHash must not use multiple aliases/u],
    [{ next_authority_set_payload_hash: SUBSTRATE_NEXT_AUTHORITY_SET_PAYLOAD_HASH }, /nextAuthoritySetPayloadHash must not use multiple aliases/u],
    [{ nextAuthoritySetProofHash: SUBSTRATE_NEXT_AUTHORITY_SET_PAYLOAD_HASH }, /nextAuthoritySetPayloadHash must not use multiple aliases/u],
  ]) {
    assert.throws(
      () => canonicalSubstrateAuthoritySetTransitionMessageBytes({ ...messageInput, ...patch }),
      pattern,
    );
  }

  const justificationInput = {
    ...messageInput,
    version: 1,
    nextAuthoritySetPayload: nextPayload,
    transitionMessageHash: SUBSTRATE_AUTHORITY_SET_TRANSITION_MESSAGE_HASH,
    grandpaJustification: {
      version: 1,
      totalWeight: 23n,
      signedWeight: 23n,
      precommitMessageHash: SUBSTRATE_AUTHORITY_SET_TRANSITION_MESSAGE_HASH,
      ...parent,
      signersBitmap: Uint8Array.from([0x07]),
      signatures: [
        new Uint8Array(64).fill(0x77),
        new Uint8Array(64).fill(0x88),
        new Uint8Array(64).fill(0x99),
      ],
    },
  };
  assert.equal(
    canonicalSubstrateAuthoritySetTransitionJustificationBytes(justificationInput).length,
    752,
  );
  assert.equal(
    substrateAuthoritySetTransitionJustificationHash(justificationInput),
    SUBSTRATE_AUTHORITY_SET_TRANSITION_JUSTIFICATION_HASH,
  );
  for (const [patch, pattern] of [
    [{ grandpa_justification: justificationInput.grandpaJustification }, /grandpaJustification must not use multiple aliases/u],
    [{ next_authority_set_payload: nextPayload }, /nextAuthoritySetPayload must not use multiple aliases/u],
    [{ transition_message_hash: SUBSTRATE_AUTHORITY_SET_TRANSITION_MESSAGE_HASH }, /transitionMessageHash must not use multiple aliases/u],
    [{ source_domain: SCCP_DOMAIN_SORA_KUSAMA }, /sourceDomain must not use multiple aliases/u],
  ]) {
    assert.throws(
      () => canonicalSubstrateAuthoritySetTransitionJustificationBytes({ ...justificationInput, ...patch }),
      pattern,
    );
  }
  for (const [patch, pattern] of [
    [{ total_weight: 23n }, /totalWeight must not use multiple aliases/u],
    [{ signed_weight: 23n }, /signedWeight must not use multiple aliases/u],
    [{ precommit_message_hash: SUBSTRATE_AUTHORITY_SET_TRANSITION_MESSAGE_HASH }, /precommitMessageHash must not use multiple aliases/u],
    [{ authority_public_keys: parent.authorityPublicKeys }, /authorityPublicKeys must not use multiple aliases/u],
    [{ authority_weights: parent.authorityWeights }, /authorityWeights must not use multiple aliases/u],
    [{ signers_bitmap: Uint8Array.from([0x07]) }, /signersBitmap must not use multiple aliases/u],
  ]) {
    assert.throws(
      () =>
        canonicalSubstrateAuthoritySetTransitionJustificationBytes({
          ...justificationInput,
          grandpaJustification: {
            ...justificationInput.grandpaJustification,
            ...patch,
          },
        }),
      pattern,
    );
  }
  assert.throws(
    () => canonicalSubstrateAuthoritySetTransitionJustificationBytes({ ...justificationInput, version: 0 }),
    /Substrate authority-set transition justification version/,
  );
  assert.throws(
    () =>
      canonicalSubstrateAuthoritySetTransitionJustificationBytes({
        ...justificationInput,
        grandpaJustification: {
          ...justificationInput.grandpaJustification,
          version: 0,
        },
      }),
    /Substrate GRANDPA justification version/,
  );
  assert.throws(
    () =>
      canonicalSubstrateAuthoritySetTransitionJustificationBytes({
        ...justificationInput,
        nextAuthoritySetPayloadHash: HEX32_B,
      }),
    /nextAuthoritySetPayloadHash must match/,
  );
  assert.throws(
    () =>
      canonicalSubstrateAuthoritySetTransitionJustificationBytes({
        ...justificationInput,
        nextAuthoritySetHash: HEX32_B,
      }),
    /nextAuthoritySetHash must match/,
  );
  assert.throws(
    () =>
      canonicalSubstrateAuthoritySetTransitionJustificationBytes({
        ...justificationInput,
        parentAuthoritySetHash: HEX32_B,
      }),
    /parentAuthoritySetHash must match/,
  );
  assert.throws(
    () =>
      canonicalSubstrateAuthoritySetTransitionJustificationBytes({
        ...justificationInput,
        grandpaJustification: {
          ...justificationInput.grandpaJustification,
          signersBitmap: new Uint8Array(257),
        },
      }),
    /signersBitmap/,
  );
  assert.throws(
    () =>
      canonicalSubstrateAuthoritySetTransitionJustificationBytes({
        ...justificationInput,
        grandpaJustification: {
          ...justificationInput.grandpaJustification,
          signatures: [new Uint8Array(64).fill(0x77), new Uint8Array(64).fill(0x88)],
        },
      }),
    /signatures length/,
  );
  assert.throws(
    () =>
      canonicalSubstrateAuthoritySetTransitionJustificationBytes({
        ...justificationInput,
        grandpaJustification: {
          ...justificationInput.grandpaJustification,
          signedWeight: 12n,
        },
      }),
    /signedWeight/,
  );
  assert.throws(
    () =>
      canonicalSubstrateAuthoritySetTransitionJustificationBytes({
        ...justificationInput,
        grandpaJustification: {
          ...justificationInput.grandpaJustification,
          signedWeight: 12n,
          signersBitmap: Uint8Array.from([0x03]),
          signatures: [new Uint8Array(64).fill(0x77), new Uint8Array(64).fill(0x88)],
        },
      }),
    /greater than two thirds/,
  );
  assert.throws(
    () =>
      canonicalSubstrateAuthoritySetTransitionJustificationBytes({
        ...justificationInput,
        grandpaJustification: {
          ...justificationInput.grandpaJustification,
          signatures: [
            new Uint8Array(64).fill(0x77),
            new Uint8Array(64),
            new Uint8Array(64).fill(0x99),
          ],
        },
      }),
    /must not be all zero/,
  );
});

test("does not generate TON SCCP proofs without a linked local prover", async () => {
  const prover = new TonSccpProver();

  await assert.rejects(
    () =>
      prover.prove({
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: HEX32_C,
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: HEX32_B,
      }),
    (error) => error?.code === "ERR_SCCP_TON_PROVER_UNAVAILABLE",
  );
});

test("rejects non-production TON SCCP input before invoking the linked prover", async () => {
  let invoked = false;
  const prover = new TonSccpProver({
    prove: async () => {
      invoked = true;
      return { proofBytes: [1, 2, 3, 4] };
    },
  });

  await assert.rejects(
    () =>
      prover.prove({
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: SCCP_ZERO_HASH_V1,
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: HEX32_B,
      }),
    /sourceStateVerifierHash/,
  );
  assert.equal(invoked, false);
});

test("accepts callable and snake_case SCCP witness providers for UI proof generation", async () => {
  const evmBinding = sampleEvmDestinationBinding();
  const evmInput = {
    publicInputs: { ...sampleEvmPublicInputs },
    bundleBytes: Uint8Array.from([5, 6, 7]),
    statementHash: HEX32_G,
    destinationBinding: evmBinding,
  };
  const evmProver = new EvmSccpProver({
    witnessProvider: async (input, options) => {
      assert.equal(options.portal, true);
      assert.notEqual(input, evmInput);
      input.publicInputs.messageId = HEX32_A;
      input.bundleBytes[0] = 0xff;
      return { ...evmInput, sourceProofBytes: [9, 10] };
    },
    prove: async (request) => {
      assert.deepEqual(Array.from(request.sourceProofBytes), [9, 10]);
      return { proofBytes: GROTH16_PROOF_BYTES };
    },
  });
  const evmResult = await evmProver.prove(evmInput, { portal: true });
  assert.deepEqual(Array.from(evmResult.sourceProofBytes), [9, 10]);
  assert.equal(evmInput.publicInputs.messageId, sampleEvmPublicInputs.messageId);
  assert.deepEqual(Array.from(evmInput.bundleBytes), [5, 6, 7]);

  const tonProver = new TonSccpProver({
    witness_provider: {
      resolve_witness: async (input, options) => {
        assert.equal(options.mobile, true);
        return {
          ...input,
          sourceAdapterDeploymentHash: HEX32_A,
          sourceAdapterDeploymentReceiptHash: HEX32_B,
        };
      },
    },
    prove: async (request) => {
      assert.equal(
        request.sourceAdapterDeploymentBinding.sourceAdapterDeploymentHash,
        HEX32_A,
      );
      return { proofBytes: [1, 2, 3, 4] };
    },
  });
  const tonResult = await tonProver.prove(
    {
      publicInputs: sampleTonPublicInputs,
      bundleBytes: [5, 6, 7],
      statementHash: HEX32_G,
      destinationBindingHash: HEX32_H,
      sourceStateVerifierHash: HEX32_C,
    },
    { mobile: true },
  );
  assert.equal(
    tonResult.sourceAdapterDeploymentBinding.sourceAdapterDeploymentReceiptHash,
    HEX32_B,
  );

  await assert.rejects(
    () =>
      new EvmSccpProver({ witnessProvider: {} }).buildRequest({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /witnessProvider must be a function or expose resolveWitness\/resolve_witness/,
  );
});

test("rejects duplicate SCCP UI prover option aliases", async () => {
  const witnessProvider = async (input) => input;
  const prove = async () => ({ proofBytes: [1, 2, 3, 4] });

  assert.throws(
    () =>
      new SolanaSccpProver({
        witnessProvider,
        witness_provider: witnessProvider,
      }),
    /Solana SCCP prover witnessProvider must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      new SolanaSccpProver({
        prove,
        proveFn: prove,
      }),
    /Solana SCCP prover prove must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      new SolanaSccpSourceStateProver({
        prove,
        prove_fn: prove,
      }),
    /Solana SCCP source-state prover prove must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      new TonSccpSourceStateProver({
        prove,
        proveFn: prove,
      }),
    /TON SCCP source-state prover prove must not use multiple aliases/u,
  );
  await assert.rejects(
    () =>
      new SolanaSccpProver({
        witnessProvider: {
          resolveWitness: witnessProvider,
          resolve_witness: witnessProvider,
        },
        prove,
      }).buildRequest(sampleProductionWitness()),
    /Solana SCCP witnessProvider resolver must not use multiple aliases/u,
  );
  const callableProviderWithDuplicateResolvers = async (input) => input;
  callableProviderWithDuplicateResolvers.resolveWitness = witnessProvider;
  callableProviderWithDuplicateResolvers.resolve_witness = witnessProvider;
  await assert.rejects(
    () =>
      new SolanaSccpProver({
        witnessProvider: callableProviderWithDuplicateResolvers,
        prove,
      }).buildRequest(sampleProductionWitness()),
    /Solana SCCP witnessProvider resolver must not use multiple aliases/u,
  );
});

test("resolves SCCP UI witness providers before web local prover callbacks", async () => {
  const solanaResolvedDestinationBindingHash = sccpDestinationBindingHash(SCCP_DOMAIN_SOL);
  const solanaExpectedRequest = buildSolanaSccpProofRequest(
    sampleProductionWitness({
      destinationBindingHash: solanaResolvedDestinationBindingHash,
    }),
  );
  let solanaResolved = false;
  const solanaResult = await new SolanaSccpProver({
    witnessProvider: {
      async resolveWitness(input, options) {
        assert.equal(options.portal, true);
        assert.equal(input.destinationBindingHash, HEX32_H);
        solanaResolved = true;
        return {
          ...input,
          destinationBindingHash: solanaResolvedDestinationBindingHash,
        };
      },
    },
    prove: async (request, options) => {
      assert.equal(options.portal, true);
      assert.equal(solanaResolved, true);
      assert.equal(
        request.proofContext.destinationBindingHash,
        solanaResolvedDestinationBindingHash,
      );
      assert.equal(request.proofContextHash, solanaExpectedRequest.proofContextHash);
      return { proofBytes: [1, 2, 3, 4] };
    },
  }).prove(sampleProductionWitness(), { portal: true });
  assert.equal(solanaResult.witnessHash, solanaExpectedRequest.witnessHash);
  assert.equal(solanaResult.proofContextHash, solanaExpectedRequest.proofContextHash);

  let tonResolved = false;
  const tonResult = await new TonSccpProver({
    witnessProvider: async (input, options) => {
      assert.equal(options.portal, true);
      assert.equal(input.sourceProofBytes, undefined);
      tonResolved = true;
      return {
        ...input,
        sourceProofBytes: [9, 10],
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: HEX32_B,
      };
    },
    prove: async (request, options) => {
      assert.equal(options.portal, true);
      assert.equal(tonResolved, true);
      assert.deepEqual(Array.from(request.sourceProofBytes), [9, 10]);
      return { proofBytes: [1, 2, 3, 4] };
    },
  }).prove(
    {
      publicInputs: sampleTonPublicInputs,
      bundleBytes: [5, 6, 7],
      statementHash: HEX32_G,
      destinationBindingHash: HEX32_H,
      sourceStateVerifierHash: HEX32_C,
    },
    { portal: true },
  );
  assert.deepEqual(Array.from(tonResult.sourceProofBytes), [9, 10]);

  let tronResolved = false;
  const tronBinding = sampleTronDestinationBinding();
  const tronResult = await new TronSccpProver({
    witnessProvider: async (input, options) => {
      assert.equal(options.portal, true);
      assert.equal(input.sourceProofBytes, undefined);
      tronResolved = true;
      return { ...input, sourceProofBytes: [9, 10] };
    },
    prove: async (request, options) => {
      assert.equal(options.portal, true);
      assert.equal(tronResolved, true);
      assert.deepEqual(Array.from(request.sourceProofBytes), [9, 10]);
      return { proofBytes: GROTH16_PROOF_BYTES };
    },
  }).prove(
    {
      publicInputs: sampleTronPublicInputs,
      bundleBytes: [5, 6, 7],
      sourceDomain: SCCP_DOMAIN_SORA,
      statementHash: HEX32_G,
      destinationBinding: tronBinding,
    },
    { portal: true },
  );
  assert.deepEqual(Array.from(tronResult.sourceProofBytes), [9, 10]);

  let substrateResolved = false;
  const substrateResult = await new SubstrateSccpProver({
    witness_provider: {
      async resolve_witness(input, options) {
        assert.equal(options.portal, true);
        assert.equal(input.sourceProofBytes, undefined);
        substrateResolved = true;
        return { ...input, sourceProofBytes: [9, 10] };
      },
    },
    prove: async (request, options) => {
      assert.equal(options.portal, true);
      assert.equal(substrateResolved, true);
      assert.deepEqual(Array.from(request.sourceProofBytes), [9, 10]);
      return { proofBytes: [1, 2, 3, 4] };
    },
  }).prove(
    {
      publicInputs: sampleSubstratePublicInputs,
      bundleBytes: [5, 6, 7],
      sourceDomain: SCCP_DOMAIN_SORA,
      statementHash: HEX32_G,
      destinationBindingHash: HEX32_H,
    },
    { portal: true },
  );
  assert.deepEqual(Array.from(substrateResult.sourceProofBytes), [9, 10]);
});

test("Solana local prover receives deep-snapshotted UI payload metadata", async () => {
  const payloadBytes = new Uint8Array([7, 8, 9]);
  const payload = {
    metadata: {
      route: ["portal"],
      bytes: payloadBytes,
    },
  };

  const result = await new SolanaSccpProver({
    prove(request) {
      assert.equal(Object.isFrozen(request), true);
      assert.equal(Object.isFrozen(request.witness), true);
      assert.equal(Object.isFrozen(request.witness.payload), true);
      assert.equal(Object.isFrozen(request.witness.payload.metadata), true);
      assert.equal(Object.isFrozen(request.witness.payload.metadata.route), true);
      assert.throws(() => {
        request.witness.payload.metadata.route.push("mutated");
      }, TypeError);

      request.witness.payload.metadata.bytes[0] = 0xff;
      assert.equal(payloadBytes[0], 7);
      return { proofBytes: [1, 2, 3, 4] };
    },
  }).prove(sampleProductionWitness({ payload }));

  assert.equal(result.proofBase64, "AQIDBA==");
  assert.deepEqual(payload.metadata.route, ["portal"]);
  assert.equal(payloadBytes[0], 7);
});

test("does not generate TRON SCCP proofs without a linked Groth16 prover", async () => {
  const prover = new TronSccpProver();

  await assert.rejects(
    () =>
      prover.prove({
        publicInputs: sampleTronPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    (error) => error?.code === "ERR_SCCP_TRON_PROVER_UNAVAILABLE",
  );
});

test("does not generate EVM-family SCCP proofs without a linked Groth16 prover", async () => {
  const prover = new EvmSccpProver();

  await assert.rejects(
    () =>
      prover.prove({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    (error) => error?.code === "ERR_SCCP_EVM_PROVER_UNAVAILABLE",
  );
});

test("does not generate Substrate SCCP proofs without a linked runtime prover", async () => {
  const prover = new SubstrateSccpProver();

  await assert.rejects(
    () =>
      prover.prove({
        publicInputs: sampleSubstratePublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    (error) => error?.code === "ERR_SCCP_SUBSTRATE_PROVER_UNAVAILABLE",
  );
});

test("wraps EVM-family Groth16 proof bytes with a request-bound envelope hash", async () => {
  let callbackRequest;
  const destinationBinding = sampleEvmDestinationBinding();
  const prover = new EvmSccpProver({
    prove: async (request) => {
      callbackRequest = request;
      assert.equal(request.backend, SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1);
      assert.equal(request.targetDomain, SCCP_DOMAIN_ETH);
      assert.deepEqual(
        request.publicSignalWords,
        sccpGroth16Bn254PublicSignalWords({
          publicInputs: sampleEvmPublicInputs,
          sourceDomain: SCCP_DOMAIN_SORA,
          statementHash: HEX32_G,
          destinationBindingHash: destinationBinding.bindingHash,
        }),
      );
      return { proofBytes: GROTH16_PROOF_BYTES };
    },
  });

  const result = await prover.prove({
    publicInputs: sampleEvmPublicInputs,
    bundleBytes: [5, 6, 7],
    sourceProofBytes: [9, 10],
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBinding,
  });
  const request = buildEvmSccpProofRequest({
    publicInputs: sampleEvmPublicInputs,
    bundleBytes: [5, 6, 7],
    sourceProofBytes: [9, 10],
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBinding,
  });
  const directResult = wrapEvmSccpProofResult(GROTH16_PROOF_BYTES, request);
  assert.notEqual(callbackRequest, request);
  assert.deepEqual(callbackRequest, request);
  await assert.rejects(
    () =>
      new EvmSccpProver({
        prove: async (linkedRequest) => ({
          proofBytes: GROTH16_PROOF_BYTES,
          requestHash: linkedRequest.requestHash,
          request_hash: linkedRequest.requestHash,
        }),
      }).prove({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBinding,
      }),
    /proofResult\.requestHash must not use multiple aliases/u,
  );
  await assert.rejects(
    () =>
      new EvmSccpProver({
        prove: async (linkedRequest) => {
          const wrapped = wrapEvmSccpProofResult(GROTH16_PROOF_BYTES, linkedRequest);
          return {
            proofBytes: GROTH16_PROOF_BYTES,
            envelopeHash: wrapped.envelopeHash,
            envelope_hash: wrapped.envelopeHash,
          };
        },
      }).prove({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBinding,
      }),
    /proofResult\.envelopeHash must not use multiple aliases/u,
  );

  assert.equal(result.backend, SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1);
  assert.equal(result.proofBytes.length, SCCP_GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1);
  assert.match(result.proofBase64, /^[A-Za-z0-9+/]+=*$/);
  assert.match(result.requestHash, /^0x[0-9a-f]{64}$/);
  assert.match(result.envelopeHash, /^0x[0-9a-f]{64}$/);
  assert.equal(directResult.envelopeHash, result.envelopeHash);
  assert.equal(result.statementHash, HEX32_G);
  assert.equal(result.destinationBindingHash, destinationBinding.bindingHash);
  assert.deepEqual(result.destinationBinding, destinationBinding);
  const exposedBundle = result.bundleBytes;
  const exposedSourceProof = result.sourceProofBytes;
  exposedBundle[0] = 99;
  exposedSourceProof[0] = 99;
  assert.deepEqual(Array.from(result.bundleBytes), [5, 6, 7]);
  assert.deepEqual(Array.from(result.sourceProofBytes), [9, 10]);
});

test("wraps TON proof bytes with an immutable request-bound envelope hash", async () => {
  let callbackRequest;
  const prover = new TonSccpProver({
    prove: async (request) => {
      callbackRequest = request;
      assert.equal(Object.isFrozen(request), true);
      assert.equal(Object.isFrozen(request.publicInputs), true);
      assert.equal(Object.isFrozen(request.proofContext), true);
      assert.equal(Object.isFrozen(request.sourceAdapterDeploymentBinding), true);
      request.bundleBytes[0] = 99;
      assert.deepEqual(Array.from(request.bundleBytes), [5, 6, 7]);
      return {
        proofBytes: [1, 2, 3, 4],
        publicInputs: request.publicInputs,
        proofContext: request.proofContext,
        statementHash: request.statementHash,
        destinationBindingHash: request.destinationBindingHash,
      };
    },
  });

  const result = await prover.prove({
    publicInputs: sampleTonPublicInputs,
    bundleBytes: [5, 6, 7],
    sourceProofBytes: [9, 10],
    statementHash: HEX32_G,
    destinationBindingHash: HEX32_H,
    sourceStateVerifierHash: HEX32_C,
    sourceAdapterDeploymentHash: HEX32_A,
    sourceAdapterDeploymentReceiptHash: HEX32_B,
  });
  const request = buildTonSccpProofRequest({
    publicInputs: sampleTonPublicInputs,
    bundleBytes: [5, 6, 7],
    sourceProofBytes: [9, 10],
    statementHash: HEX32_G,
    destinationBindingHash: HEX32_H,
    sourceStateVerifierHash: HEX32_C,
    sourceAdapterDeploymentHash: HEX32_A,
    sourceAdapterDeploymentReceiptHash: HEX32_B,
  });
  const directResult = wrapTonSccpProofResult([1, 2, 3, 4], request);
  assert.notEqual(callbackRequest, request);
  assert.deepEqual(callbackRequest, request);
  assert.throws(
    () =>
      wrapTonSccpProofResult(
        new Uint8Array(SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1).fill(1),
        request,
      ),
    /at most/,
  );
  await assert.rejects(
    () =>
      new TonSccpProver({
        prove: async (linkedRequest) => {
          const wrapped = wrapTonSccpProofResult([1, 2, 3, 4], linkedRequest);
          return {
            proofBytes: [1, 2, 3, 4],
            requestHash: wrapped.requestHash,
            request_hash: wrapped.requestHash,
          };
        },
      }).prove({
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: HEX32_C,
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: HEX32_B,
      }),
    /proofResult\.requestHash.*multiple aliases/u,
  );
  await assert.rejects(
    () =>
      new TonSccpProver({
        prove: async () => ({
          proofBytes: [1, 2, 3, 4],
          publicInputs: { ...sampleTonPublicInputs, messageId: HEX32_B },
        }),
      }).prove({
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: HEX32_C,
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: HEX32_B,
      }),
    /proofResult\.publicInputs must match request\.publicInputs/u,
  );

  assert.equal(result.backend, SCCP_TON_CONTRACT_PROOF_BACKEND_V1);
  assert.equal(Object.isFrozen(result), true);
  assert.equal(Object.isFrozen(result.publicInputs), true);
  assert.equal(Object.isFrozen(result.proofContext), true);
  assert.equal(Object.isFrozen(result.sourceAdapterDeploymentBinding), true);
  assert.equal(result.proofBase64, "AQIDBA==");
  const exposedProof = result.proofBytes;
  exposedProof[0] = 99;
  assert.deepEqual(Array.from(result.proofBytes), [1, 2, 3, 4]);
  const exposedBundle = result.bundleBytes;
  const exposedSourceProof = result.sourceProofBytes;
  exposedBundle[0] = 99;
  exposedSourceProof[0] = 99;
  assert.deepEqual(Array.from(result.bundleBytes), [5, 6, 7]);
  assert.deepEqual(Array.from(result.sourceProofBytes), [9, 10]);
  assert.match(result.requestHash, /^0x[0-9a-f]{64}$/);
  assert.match(result.envelopeHash, /^0x[0-9a-f]{64}$/);
  assert.equal(directResult.envelopeHash, result.envelopeHash);
  assert.equal(result.statementHash, HEX32_G);
  assert.equal(result.destinationBindingHash, HEX32_H);
});

test("wraps Substrate runtime proof bytes with a request-bound envelope hash", async () => {
  let callbackRequest;
  const prover = new SubstrateSccpProver({
    prove: async (request) => {
      callbackRequest = request;
      assert.equal(Object.isFrozen(request), true);
      assert.equal(Object.isFrozen(request.publicInputs), true);
      assert.equal(Object.isFrozen(request.proofContext), true);
      assert.equal(request.backend, SCCP_SUBSTRATE_RUNTIME_PROOF_BACKEND_V1);
      assert.equal(request.targetDomain, SCCP_DOMAIN_SORA2);
      assert.equal(request.sourceDomain, SCCP_DOMAIN_SORA);
      request.bundleBytes[0] = 99;
      assert.deepEqual(Array.from(request.bundleBytes), [5, 6, 7]);
      return { proofBytes: [1, 2, 3, 4] };
    },
  });

  const result = await prover.prove({
    publicInputs: sampleSubstratePublicInputs,
    bundleBytes: [5, 6, 7],
    sourceProofBytes: [9, 10],
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBindingHash: HEX32_H,
  });
  const request = buildSubstrateSccpProofRequest({
    publicInputs: sampleSubstratePublicInputs,
    bundleBytes: [5, 6, 7],
    sourceProofBytes: [9, 10],
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBindingHash: HEX32_H,
  });
  const directResult = wrapSubstrateSccpProofResult([1, 2, 3, 4], request);
  assert.notEqual(callbackRequest, request);
  assert.deepEqual(callbackRequest, request);
  assert.throws(
    () =>
      wrapSubstrateSccpProofResult(
        new Uint8Array(SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1).fill(1),
        request,
      ),
    /at most/,
  );
  await assert.rejects(
    () =>
      new SubstrateSccpProver({
        prove: async (linkedRequest) => ({
          proofBytes: [1, 2, 3, 4],
          requestHash: linkedRequest.requestHash,
          request_hash: linkedRequest.requestHash,
        }),
      }).prove({
        publicInputs: sampleSubstratePublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /proofResult\.requestHash must not use multiple aliases/u,
  );
  await assert.rejects(
    () =>
      new SubstrateSccpProver({
        prove: async (linkedRequest) => {
          const wrapped = wrapSubstrateSccpProofResult([1, 2, 3, 4], linkedRequest);
          return {
            proofBytes: [1, 2, 3, 4],
            envelopeHash: wrapped.envelopeHash,
            envelope_hash: wrapped.envelopeHash,
          };
        },
      }).prove({
        publicInputs: sampleSubstratePublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /proofResult\.envelopeHash must not use multiple aliases/u,
  );

  assert.equal(result.backend, SCCP_SUBSTRATE_RUNTIME_PROOF_BACKEND_V1);
  assert.equal(Object.isFrozen(result), true);
  assert.equal(Object.isFrozen(result.publicInputs), true);
  assert.equal(Object.isFrozen(result.proofContext), true);
  assert.equal(result.proofBase64, "AQIDBA==");
  const exposedProof = result.proofBytes;
  exposedProof[0] = 99;
  assert.deepEqual(Array.from(result.proofBytes), [1, 2, 3, 4]);
  const exposedBundle = result.bundleBytes;
  const exposedSourceProof = result.sourceProofBytes;
  exposedBundle[0] = 99;
  exposedSourceProof[0] = 99;
  assert.deepEqual(Array.from(result.bundleBytes), [5, 6, 7]);
  assert.deepEqual(Array.from(result.sourceProofBytes), [9, 10]);
  assert.match(result.requestHash, /^0x[0-9a-f]{64}$/);
  assert.match(result.envelopeHash, /^0x[0-9a-f]{64}$/);
  assert.equal(directResult.envelopeHash, result.envelopeHash);
  assert.equal(result.statementHash, HEX32_G);
  assert.equal(result.destinationBindingHash, HEX32_H);
});

test("wraps TRON Groth16 proof bytes with a request-bound envelope hash", async () => {
  let callbackRequest;
  const destinationBinding = sampleTronDestinationBinding();
  const prover = new TronSccpProver({
    prove: async (request) => {
      callbackRequest = request;
      assert.equal(request.backend, SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1);
      request.bundleBytes[0] = 99;
      assert.deepEqual(Array.from(request.bundleBytes), [5, 6, 7]);
      assert.deepEqual(
        request.publicSignalWords,
        sccpGroth16Bn254PublicSignalWords({
          publicInputs: sampleTronPublicInputs,
          sourceDomain: SCCP_DOMAIN_SORA,
          statementHash: HEX32_G,
          destinationBindingHash: destinationBinding.bindingHash,
        }),
      );
      return { proofBytes: GROTH16_PROOF_BYTES };
    },
  });

  const result = await prover.prove({
    publicInputs: sampleTronPublicInputs,
    bundleBytes: [5, 6, 7],
    sourceProofBytes: [9, 10],
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBinding,
  });
  const request = buildTronSccpProofRequest({
    publicInputs: sampleTronPublicInputs,
    bundleBytes: [5, 6, 7],
    sourceProofBytes: [9, 10],
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBinding,
  });
  const directResult = wrapTronSccpProofResult(GROTH16_PROOF_BYTES, request);
  assert.notEqual(callbackRequest, request);
  assert.deepEqual(callbackRequest, request);
  await assert.rejects(
    () =>
      new TronSccpProver({
        prove: async (linkedRequest) => {
          const wrapped = wrapTronSccpProofResult(GROTH16_PROOF_BYTES, linkedRequest);
          return {
            proofBytes: GROTH16_PROOF_BYTES,
            envelopeHash: wrapped.envelopeHash,
            envelope_hash: wrapped.envelopeHash,
          };
        },
      }).prove({
        publicInputs: sampleTronPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBinding,
      }),
    /proofResult\.envelopeHash must not use multiple aliases/u,
  );

  assert.equal(result.backend, SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1);
  assert.equal(result.proofBytes.length, SCCP_GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1);
  assert.match(result.proofBase64, /^[A-Za-z0-9+/]+=*$/);
  assert.equal(Object.isFrozen(result), true);
  const exposedProof = result.proofBytes;
  exposedProof[0] = 99;
  assert.equal(result.proofBytes[0], GROTH16_PROOF_BYTES[0]);
  const exposedBundle = result.bundleBytes;
  const exposedSourceProof = result.sourceProofBytes;
  exposedBundle[0] = 99;
  exposedSourceProof[0] = 99;
  assert.deepEqual(Array.from(result.bundleBytes), [5, 6, 7]);
  assert.deepEqual(Array.from(result.sourceProofBytes), [9, 10]);
  assert.match(result.requestHash, /^0x[0-9a-f]{64}$/);
  assert.match(result.envelopeHash, /^0x[0-9a-f]{64}$/);
  assert.equal(directResult.envelopeHash, result.envelopeHash);
  assert.equal(result.statementHash, HEX32_G);
  assert.equal(result.destinationBindingHash, destinationBinding.bindingHash);
  assert.deepEqual(result.destinationBinding, destinationBinding);
});

test("rejects mutated EVM-family, TRON, and Substrate proof requests before wrapping", () => {
  const evmBinding = sampleEvmDestinationBinding();
  const evmRequest = buildEvmSccpProofRequest({
    publicInputs: sampleEvmPublicInputs,
    bundleBytes: [5, 6, 7],
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBinding: evmBinding,
  });
  const hashOnlyEvmRequest = buildEvmSccpProofRequest({
    publicInputs: sampleEvmPublicInputs,
    bundleBytes: [5, 6, 7],
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBindingHash: evmBinding.bindingHash,
  });
  assert.throws(
    () => wrapEvmSccpProofResult(GROTH16_PROOF_BYTES, hashOnlyEvmRequest),
    /EVM-family SCCP production proofs must include destinationBinding deployment material/,
  );
  assert.throws(
    () =>
      wrapEvmSccpProofResult(GROTH16_PROOF_BYTES, {
        ...evmRequest,
        requestHash: HEX32_A,
      }),
    /EVM-family SCCP proof request must be canonical/,
  );

  const tronBinding = sampleTronDestinationBinding();
  const tronRequest = buildTronSccpProofRequest({
    publicInputs: sampleTronPublicInputs,
    bundleBytes: [5, 6, 7],
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBinding: tronBinding,
  });
  const hashOnlyTronRequest = buildTronSccpProofRequest({
    publicInputs: sampleTronPublicInputs,
    bundleBytes: [5, 6, 7],
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBindingHash: tronBinding.bindingHash,
  });
  assert.throws(
    () => wrapTronSccpProofResult(GROTH16_PROOF_BYTES, hashOnlyTronRequest),
    /TRON SCCP production proofs must include destinationBinding deployment material/,
  );
  const mutatedPublicSignalWords = [...tronRequest.publicSignalWords];
  mutatedPublicSignalWords[8] =
    mutatedPublicSignalWords[8] === HEX32_A ? HEX32_B : HEX32_A;
  assert.throws(
    () =>
      wrapTronSccpProofResult(GROTH16_PROOF_BYTES, {
        ...tronRequest,
        publicSignalWords: mutatedPublicSignalWords,
      }),
    /TRON SCCP proof request must be canonical/,
  );

  const substrateRequest = buildSubstrateSccpProofRequest({
    publicInputs: sampleSubstratePublicInputs,
    bundleBytes: [5, 6, 7],
    sourceProofBytes: [9, 10],
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: HEX32_G,
    destinationBindingHash: HEX32_H,
  });
  assert.throws(
    () =>
      wrapSubstrateSccpProofResult([1, 2, 3, 4], {
        ...substrateRequest,
        proofContext: {
          ...substrateRequest.proofContext,
          destinationBindingHash: HEX32_A,
        },
      }),
    /Substrate SCCP proof request must be canonical/,
  );
});

test("rejects non-production EVM-family, TRON, and Substrate inputs before callbacks", async () => {
  let invoked = false;
  await assert.rejects(
    () =>
      new EvmSccpProver({
        prove: async () => {
          invoked = true;
          return { proofBytes: GROTH16_PROOF_BYTES };
        },
      }).prove({
        publicInputs: { ...sampleEvmPublicInputs, targetDomain: SCCP_DOMAIN_TRON },
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /publicInputs\.targetDomain/,
  );
  assert.equal(invoked, false);

  await assert.rejects(
    () =>
      new TronSccpProver({
        prove: async () => {
          invoked = true;
          return { proofBytes: GROTH16_PROOF_BYTES };
        },
      }).prove({
        publicInputs: { ...sampleTronPublicInputs, targetDomain: SCCP_DOMAIN_ETH },
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /publicInputs\.targetDomain/,
  );
  assert.equal(invoked, false);

  await assert.rejects(
    () =>
      new SubstrateSccpProver({
        prove: async () => {
          invoked = true;
          return { proofBytes: [1, 2, 3, 4] };
        },
      }).prove({
        publicInputs: { ...sampleSubstratePublicInputs, targetDomain: SCCP_DOMAIN_ETH },
        bundleBytes: [5, 6, 7],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /publicInputs\.targetDomain/,
  );
  assert.equal(invoked, false);
});

test("accepts submit-ready SCCP prover requests with omitted source proof material", async () => {
  for (const [Prover, input, proofBytes] of [
    [
      EvmSccpProver,
      {
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBinding: sampleEvmDestinationBinding(),
      },
      GROTH16_PROOF_BYTES,
    ],
    [
      TronSccpProver,
      {
        publicInputs: sampleTronPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBinding: sampleTronDestinationBinding(),
      },
      GROTH16_PROOF_BYTES,
    ],
    [
      SubstrateSccpProver,
      {
        publicInputs: sampleSubstratePublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      },
      [1, 2, 3, 4],
    ],
    [
      TonSccpProver,
      {
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [5, 6, 7],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: HEX32_C,
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: HEX32_B,
      },
      [1, 2, 3, 4],
    ],
  ]) {
    let invoked = false;
    const result = await new Prover({
      prove: async () => {
        invoked = true;
        return { proofBytes };
      },
    }).prove(input);
    assert.equal(invoked, true);
    assert.equal(result.sourceProofBytes.length, 0);
  }
});

test("rejects all-zero proof bytes across SCCP local prover wrappers", async () => {
  await assert.rejects(
    () =>
      new EvmSccpProver({
        prove: async () => ({ proofBytes: [0, 0] }),
      }).prove({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBinding: sampleEvmDestinationBinding(),
      }),
    /proofBytes must not be all zero/,
  );

  await assert.rejects(
    () =>
      new TronSccpProver({
        prove: async () => ({ proofBytes: [0, 0] }),
      }).prove({
        publicInputs: sampleTronPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBinding: sampleTronDestinationBinding(),
      }),
    /proofBytes must not be all zero/,
  );

  await assert.rejects(
    () =>
      new TonSccpProver({
        prove: async () => ({ proofBytes: [0, 0] }),
      }).prove({
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: HEX32_C,
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: HEX32_B,
      }),
    /proofBytes must not be all zero/,
  );

  await assert.rejects(
    () =>
      new SubstrateSccpProver({
        prove: async () => ({ proofBytes: [0, 0] }),
      }).prove({
        publicInputs: sampleSubstratePublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /proofBytes must not be all zero/,
  );
});

test("rejects TON, EVM, TRON, and Substrate prover results with mismatched metadata", async () => {
  const evmBinding = sampleEvmDestinationBinding();
  const tronBinding = sampleTronDestinationBinding();
  const tonRequestInput = {
    publicInputs: sampleTonPublicInputs,
    bundleBytes: [5, 6, 7],
    sourceProofBytes: [9, 10],
    statementHash: HEX32_G,
    destinationBindingHash: HEX32_H,
    sourceStateVerifierHash: HEX32_C,
    sourceAdapterDeploymentHash: HEX32_A,
    sourceAdapterDeploymentReceiptHash: HEX32_B,
  };

  await assert.rejects(
    () =>
      new TonSccpProver({
        prove: async () => ({
          proofBytes: [1, 2, 3, 4],
          proofBase64: "AAAA",
        }),
      }).prove(tonRequestInput),
    /proofResult\.proofBase64/,
  );

  await assert.rejects(
    () =>
      new TonSccpProver({
        prove: async () => ({
          proofBytes: [1, 2, 3, 4],
          proof_base64: " AQIDBA== ",
        }),
      }).prove(tonRequestInput),
    /proofResult\.proofBase64/,
  );

  await assert.rejects(
    () =>
      new EvmSccpProver({
        prove: async () => ({
          proofBytes: GROTH16_PROOF_BYTES,
          publicInputs: { ...sampleEvmPublicInputs, commitmentRoot: HEX32_A },
        }),
      }).prove({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBinding: evmBinding,
      }),
    /proofResult\.publicInputs/,
  );

  await assert.rejects(
    () =>
      new EvmSccpProver({
        prove: async (request) => ({
          proofBytes: GROTH16_PROOF_BYTES,
          publicInputs: request.publicInputs,
          public_inputs: request.publicInputs,
        }),
      }).prove({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBinding: evmBinding,
      }),
    /proofResult\.publicInputs/u,
  );

  await assert.rejects(
    () =>
      new EvmSccpProver({
        prove: async () => ({
          proofBytes: GROTH16_PROOF_BYTES,
          proofBase64: "AAAA",
        }),
      }).prove({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBinding: evmBinding,
      }),
    /proofResult\.proofBase64/,
  );

  await assert.rejects(
    () =>
      new EvmSccpProver({
        prove: async () => ({
          proofBytes: GROTH16_PROOF_BYTES,
          proofBase64: Buffer.from(GROTH16_PROOF_BYTES).toString("base64"),
          proof_base64: Buffer.from(GROTH16_PROOF_BYTES).toString("base64"),
        }),
      }).prove({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBinding: evmBinding,
      }),
    /proofResult\.proofBase64/u,
  );

  await assert.rejects(
    () =>
      new EvmSccpProver({
        prove: async () => ({
          proofBytes: GROTH16_PROOF_BYTES,
          proofBase64: ` ${Buffer.from(GROTH16_PROOF_BYTES).toString("base64")} `,
        }),
      }).prove({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBinding: evmBinding,
      }),
    /proofResult\.proofBase64/,
  );

  await assert.rejects(
    () =>
      new EvmSccpProver({
        prove: async () => ({
          proofBytes: GROTH16_PROOF_BYTES,
          publicInputs: null,
        }),
      }).prove({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBinding: evmBinding,
      }),
    /publicInputs/,
  );

  await assert.rejects(
    () =>
      new TronSccpProver({
        prove: async () => ({
          proofBytes: GROTH16_PROOF_BYTES,
          proof_base64: "AAAA",
        }),
      }).prove({
        publicInputs: sampleTronPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBinding: tronBinding,
      }),
    /proofResult\.proofBase64/,
  );

  await assert.rejects(
    () =>
      new TronSccpProver({
        prove: async () => ({
          proofBytes: GROTH16_PROOF_BYTES,
          proofContext: {
            version: 1,
            statementHash: HEX32_G,
            destinationBindingHash: HEX32_A,
          },
        }),
      }).prove({
        publicInputs: sampleTronPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBinding: tronBinding,
      }),
    /proofResult\.proofContext/,
  );

  await assert.rejects(
    () =>
      new TronSccpProver({
        prove: async () => ({
          proofBytes: GROTH16_PROOF_BYTES,
          requestHash: null,
        }),
      }).prove({
        publicInputs: sampleTronPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBinding: tronBinding,
      }),
    /requestHash/,
  );

  await assert.rejects(
    () =>
      new SubstrateSccpProver({
        prove: async () => ({
          proofBytes: [1, 2, 3, 4],
          proofBase64: "AAAA",
        }),
      }).prove({
        publicInputs: sampleSubstratePublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /proofResult\.proofBase64/,
  );

  await assert.rejects(
    () =>
      new SubstrateSccpProver({
        prove: async () => ({
          proofBytes: [1, 2, 3, 4],
          publicInputs: {
            ...sampleSubstratePublicInputs,
            commitmentRoot: HEX32_A,
          },
        }),
      }).prove({
        publicInputs: sampleSubstratePublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /proofResult\.publicInputs/,
  );

  await assert.rejects(
    () =>
      new SubstrateSccpProver({
        prove: async () => ({
          proofBytes: [1, 2, 3, 4],
          proofContext: {
            version: 1,
            statementHash: HEX32_G,
            destinationBindingHash: HEX32_A,
          },
        }),
      }).prove({
        publicInputs: sampleSubstratePublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /proofResult\.proofContext/,
  );
});

test("rejects non-canonical EVM-family and TRON Groth16 proof lengths", async () => {
  const evmBinding = sampleEvmDestinationBinding();
  const tronBinding = sampleTronDestinationBinding();
  await assert.rejects(
    () =>
      new EvmSccpProver({
        prove: async () => ({ proofBytes: [1, 2, 3, 4] }),
      }).prove({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBinding: evmBinding,
      }),
    /proofBytes must be 384 bytes/,
  );

  await assert.rejects(
    () =>
      new TronSccpProver({
        prove: async () => ({ proofBytes: [1, 2, 3, 4] }),
      }).prove({
        publicInputs: sampleTronPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBinding: tronBinding,
      }),
    /proofBytes must be 384 bytes/,
  );
});

test("rejects SCCP prover results bound to a different request context", async () => {
  const evmBinding = sampleEvmDestinationBinding();
  const tronBinding = sampleTronDestinationBinding();
  await assert.rejects(
    () =>
      new SolanaSccpProver({
        prove: async () => ({
          proofBytes: [1, 2, 3, 4],
          proofContextHash: HEX32_A,
        }),
      }).prove(sampleProductionWitness()),
    /proofContextHash must match request/,
  );
  await assert.rejects(
    () =>
      new SolanaSccpProver({
        prove: async () => ({
          proofBytes: [1, 2, 3, 4],
          sourceStateVerifierId: "sccp:solana:wrong-source-state-verifier:v1",
        }),
      }).prove(sampleProductionWitness()),
    /proofResult\.sourceStateVerifierId must match request/,
  );
  await assert.rejects(
    () =>
      new SolanaSccpProver({
        prove: async () => ({
          proofBytes: [1, 2, 3, 4],
          sourceStateVerifierHash: HEX32_D,
        }),
      }).prove(sampleProductionWitness()),
    /proofResult\.sourceStateVerifierHash must match request/,
  );
  await assert.rejects(
    () =>
      new SolanaSccpProver({
        prove: async (request) => ({
          proofBytes: [1, 2, 3, 4],
          sourceAdapterDeploymentBinding: {
            ...request.sourceAdapterDeploymentBinding,
            sourceAdapterDeploymentHash: HEX32_D,
          },
        }),
      }).prove(sampleProductionWitness()),
    /proofResult\.sourceAdapterDeploymentBinding must match request/,
  );

  await assert.rejects(
    () =>
      new TonSccpProver({
        prove: async () => ({
          proofBytes: [1, 2, 3, 4],
          sourceAdapterDeploymentBindingHash: HEX32_C,
        }),
      }).prove({
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
        sourceStateVerifierHash: HEX32_D,
        sourceAdapterDeploymentHash: HEX32_A,
        sourceAdapterDeploymentReceiptHash: HEX32_B,
      }),
    /sourceAdapterDeploymentBindingHash must match request/,
  );

  await assert.rejects(
    () =>
      new EvmSccpProver({
        prove: async () => ({
          proofBytes: GROTH16_PROOF_BYTES,
          requestHash: HEX32_A,
        }),
      }).prove({
        publicInputs: sampleEvmPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBinding: evmBinding,
      }),
    /requestHash must match request/,
  );

  await assert.rejects(
    () =>
      new TronSccpProver({
        prove: async () => ({
          backend: SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
          proofBytes: GROTH16_PROOF_BYTES,
        }),
      }).prove({
        publicInputs: sampleTronPublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBinding: tronBinding,
      }),
    /backend must match request/,
  );

  await assert.rejects(
    () =>
      new SubstrateSccpProver({
        prove: async () => ({
          proofBytes: [1, 2, 3, 4],
          requestHash: HEX32_A,
        }),
      }).prove({
        publicInputs: sampleSubstratePublicInputs,
        bundleBytes: [5, 6, 7],
        sourceProofBytes: [9, 10],
        statementHash: HEX32_G,
        destinationBindingHash: HEX32_H,
      }),
    /requestHash must match request/,
  );
});
