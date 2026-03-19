/**
 * Entry point for the experimental Iroha JS SDK.
 *
 * Native bindings (Norito + crypto) are provided via `iroha_js_host`. When the
 * native module is unavailable, the SDK falls back to pure JS implementations.
 */
export {
  AccountAddress,
  AccountAddressError,
  AccountAddressErrorCode,
  DEFAULT_DOMAIN_NAME,
  decodeI105AccountAddress,
  encodeI105AccountAddress,
  inspectAccountId,
  configureCurveSupport,
} from "./address.js";
export { normalizeIdentifierInput } from "./normalizers.js";
export { MultisigSpecBuilder, MultisigSpec } from "./multisig.js";
export { ValidationError, ValidationErrorCode } from "./validationError.js";
export {
  ToriiClient,
  TransactionStatusError,
  TransactionTimeoutError,
  IsoMessageTimeoutError,
  ToriiDataModelCompatibilityError,
  ToriiHttpError,
  extractPipelineStatusKind,
  decodePdpCommitmentHeader,
  buildConnectWebSocketUrl,
  encryptIdentifierInputForPolicy,
  buildIdentifierRequestForPolicy,
  buildRbcSampleRequest,
  getIdentifierBfvPublicParameters,
  openConnectWebSocket,
  verifyIdentifierResolutionReceipt,
} from "./toriiClient.js";
export { NoritoRpcClient, NoritoRpcError } from "./noritoRpcClient.js";
export {
  generateKeyPair,
  publicKeyFromPrivate,
  signEd25519,
  verifyEd25519,
  deriveConfidentialKeyset,
  deriveConfidentialKeysetFromHex,
  generateSm2KeyPair,
  deriveSm2KeyPairFromSeed,
  loadSm2KeyPair,
  signSm2,
  verifySm2,
  sm2PublicKeyMultihash,
  SM2_PRIVATE_KEY_LENGTH,
  SM2_PUBLIC_KEY_LENGTH,
  SM2_SIGNATURE_LENGTH,
  SM2_DEFAULT_DISTINGUISHED_ID,
  sm2FixtureFromSeed,
} from "./crypto.js";
export {
  canonicalQueryString,
  canonicalRequestMessage,
  buildCanonicalRequestHeaders,
} from "./canonicalRequest.js";
export {
  buildTouchManifest,
  buildAxtDescriptor,
  normalizeAxtRejectContext,
  buildHandleRefreshRequest,
  computeAxtBinding,
} from "./axt.js";
export { noritoEncodeInstruction, noritoDecodeInstruction } from "./norito.js";
export {
  buildGatewayRequest,
  computePayloadHashLiteral,
  loadComputeFixtures,
  simulateCompute,
  validatePayloadHash,
} from "./compute.js";
export {
  laneRelayEnvelopeSample,
  verifyLaneRelayEnvelope,
  verifyLaneRelayEnvelopeJson,
  verifyLaneRelayEnvelopes,
  decodeLaneRelayEnvelope,
  laneSettlementHash,
} from "./nexus.js";
export {
  hashSignedTransaction,
  resignSignedTransaction,
  encodeSignedTransactionNorito,
  buildRegisterDomainTransaction,
  buildTransaction,
  buildMintAssetTransaction,
  buildBurnAssetTransaction,
  buildBurnTriggerTransaction,
  buildMintTriggerTransaction,
  buildMintAndTransferTransaction,
  buildRegisterDomainAndMintTransaction,
  buildRegisterAccountAndTransferTransaction,
  buildRegisterAssetDefinitionAndMintTransaction,
  buildRegisterAssetDefinitionMintAndTransferTransaction,
  buildRegisterMultisigTransaction,
  buildTransferAssetTransaction,
  buildTransferAssetDefinitionTransaction,
  buildTransferDomainTransaction,
  buildTransferNftTransaction,
  buildCreateKaigiTransaction,
  buildJoinKaigiTransaction,
  buildLeaveKaigiTransaction,
  buildEndKaigiTransaction,
  buildRecordKaigiUsageTransaction,
  buildSetKaigiRelayManifestTransaction,
  buildRegisterKaigiRelayTransaction,
  buildRegisterSmartContractCodeTransaction,
  buildRegisterSmartContractBytesTransaction,
  buildDeactivateContractInstanceTransaction,
  buildActivateContractInstanceTransaction,
  buildRemoveSmartContractBytesTransaction,
  buildProposeDeployContractTransaction,
  buildCastZkBallotTransaction,
  buildCastPlainBallotTransaction,
  buildEnactReferendumTransaction,
  buildFinalizeReferendumTransaction,
  buildPersistCouncilForEpochTransaction,
  buildRegisterZkAssetTransaction,
  buildScheduleConfidentialPolicyTransitionTransaction,
  buildCancelConfidentialPolicyTransitionTransaction,
  buildShieldTransaction,
  buildZkTransferTransaction,
  buildUnshieldTransaction,
  buildCreateElectionTransaction,
  buildSubmitBallotTransaction,
  buildFinalizeElectionTransaction,
  buildTimeTriggerAction,
  buildPrecommitTriggerAction,
  submitSignedTransaction,
} from "./transaction.js";
export {
  buildOfflineEnvelope,
  parseOfflineEnvelope,
  serializeOfflineEnvelope,
  readOfflineEnvelopeFile,
  replayOfflineEnvelope,
  writeOfflineEnvelopeFile,
} from "./offlineEnvelope.js";
export {
  OfflineQrPayloadKind,
  OfflineQrStreamDecoder,
  OfflineQrStreamEncoder,
  OfflineQrStreamFrame,
  OfflineQrStreamFrameEncoding,
  OfflineQrStreamFrameKind,
  OfflineQrStreamScanSession,
  OfflineQrStreamPlaybackSkin,
  OfflineQrStreamOptions,
  OfflineQrStreamEnvelope,
  OfflineQrStreamTheme,
  encodeQrFrameText,
  decodeQrFrameText,
  scanQrStreamFrames,
  sakuraQrStreamTheme,
  sakuraStormQrStreamTheme,
  sakuraQrStreamSkin,
  sakuraStormQrStreamSkin,
  sakuraQrStreamReducedMotionSkin,
  sakuraQrStreamLowPowerSkin,
} from "./offlineQrStream.js";
export {
  PETAL_STREAM_GRID_SIZES,
  OfflinePetalStreamOptions,
  OfflinePetalStreamGrid,
  OfflinePetalStreamSampleGrid,
  OfflinePetalStreamEncoder,
  OfflinePetalStreamDecoder,
  OfflinePetalStreamScanSession,
  samplePetalStreamGridFromRgba,
  decodePetalStreamFrameAuto,
} from "./offlinePetalStream.js";
export {
  OfflineCounterJournal,
  OfflineCounterJournalError,
  OfflineCounterPlatform,
} from "./offlineCounterJournal.js";
export {
  buildBurnAssetInstruction,
  buildMintAssetInstruction,
  buildMintTriggerRepetitionsInstruction,
  buildBurnTriggerRepetitionsInstruction,
  buildRegisterDomainInstruction,
  buildRegisterAccountInstruction,
  buildRegisterMultisigInstruction,
  buildProposeMultisigInstruction,
  buildTransferAssetInstruction,
  buildTransferDomainInstruction,
  buildTransferAssetDefinitionInstruction,
  buildTransferNftInstruction,
  buildCreateKaigiInstruction,
  buildJoinKaigiInstruction,
  buildLeaveKaigiInstruction,
  buildEndKaigiInstruction,
  buildRecordKaigiUsageInstruction,
  buildSetKaigiRelayManifestInstruction,
  buildRegisterKaigiRelayInstruction,
  buildRegisterSmartContractCodeInstruction,
  buildRegisterSmartContractBytesInstruction,
  buildDeactivateContractInstanceInstruction,
  buildActivateContractInstanceInstruction,
  buildRemoveSmartContractBytesInstruction,
  buildProposeDeployContractInstruction,
  buildCastZkBallotInstruction,
  buildCastPlainBallotInstruction,
  buildEnactReferendumInstruction,
  buildFinalizeReferendumInstruction,
  buildPersistCouncilForEpochInstruction,
  buildClaimTwitterFollowRewardInstruction,
  buildSendToTwitterInstruction,
  buildCancelTwitterEscrowInstruction,
  buildRegisterZkAssetInstruction,
  buildScheduleConfidentialPolicyTransitionInstruction,
  buildCancelConfidentialPolicyTransitionInstruction,
  buildShieldInstruction,
  buildZkTransferInstruction,
  buildUnshieldInstruction,
  buildCreateElectionInstruction,
  buildSubmitBallotInstruction,
  buildFinalizeElectionInstruction,
  encodeInstruction,
  normalizeAccountId,
  normalizeAssetId,
} from "./instructionBuilders.js";
export {
  resolveToriiClientConfig,
  extractToriiFeatureConfig,
  extractConfidentialGasConfig,
  DEFAULT_TORII_CLIENT_CONFIG,
  DEFAULT_RETRY_PROFILE_PIPELINE,
  DEFAULT_RETRY_PROFILE_STREAMING,
} from "./config.js";
export {
  buildDaIngestRequest,
  deriveDaChunkerHandle,
  generateDaProofSummary,
  buildDaProofSummaryArtifact,
  emitDaProofSummaryArtifact,
} from "./dataAvailability.js";
export {
  buildPacs008Message,
  buildPacs009Message,
  buildSamplePacs008Message,
  buildSamplePacs009Message,
  buildCamt052Message,
  buildCamt056Message,
  buildSampleCamt052Message,
  buildSampleCamt056Message,
} from "./isoBridge.js";
export { decodeReplicationOrder, SorafsGatewayFetchError, sorafsGatewayFetch } from "./sorafs.js";
export { ConnectRetryPolicy } from "./connectRetryPolicy.js";

import * as toriiNamespace from "./toriiClient.js";
import * as noritoNamespace from "./norito.js";
import * as cryptoNamespace from "./crypto.js";
import * as offlineNamespace from "./offlineEnvelope.js";
import * as offlineCounterNamespace from "./offlineCounterJournal.js";
import * as offlineQrStreamNamespace from "./offlineQrStream.js";
import * as offlinePetalStreamNamespace from "./offlinePetalStream.js";

export const Torii = toriiNamespace;
export const Norito = noritoNamespace;
export const Crypto = cryptoNamespace;
export const Offline = offlineNamespace;
export const OfflineCounters = offlineCounterNamespace;
export const OfflineQrStream = offlineQrStreamNamespace;
export const OfflinePetalStream = offlinePetalStreamNamespace;
export {
  ConnectError,
  ConnectErrorCategory,
  connectErrorFrom,
  ConnectQueueError,
} from "./connectError.js";
export { generateConnectSid, createConnectSessionPreview } from "./connectSession.js";
export { bootstrapConnectPreviewSession } from "./connectPreviewFlow.js";
export {
  appendConnectQueueMetric,
  defaultConnectQueueRoot,
  deriveConnectSessionDirectory,
  exportConnectQueueEvidence,
  readConnectQueueSnapshot,
  updateConnectQueueSnapshot,
  writeConnectQueueSnapshot,
} from "./connectQueueDiagnostics.js";
export { ConnectQueueJournal } from "./connectQueueJournal.js";
export {
  ConnectDirection,
  ConnectJournalRecord,
  ConnectJournalError,
} from "./connectJournalRecord.js";
export { SoranetPuzzleClient, SoranetPuzzleError } from "./soranetPuzzleClient.js";
export {
  deriveGatewayHosts as deriveSoradnsGatewayHosts,
  hostPatternsCoverDerivedHosts,
  canonicalGatewaySuffix,
  prettyGatewaySuffix,
  canonicalGatewayWildcard,
} from "./soradns.js";
export {
  captureSumeragiTelemetrySnapshot,
  appendSumeragiTelemetrySnapshot,
} from "./telemetryReplay.js";
