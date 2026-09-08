import { createNoritoReplicationOrderValidator } from "./noritoReplicationOrderValidator.js";
import { createNoritoRecordDecoder, createNoritoRecordEncoder } from "./noritoRecordDecoder.js";
import { rejectError, rejectRange, rejectType } from "./validationThrow.js";
import { kaigiScalarBytesV1 } from "./kaigiScalarV1.js";
import { Buffer } from "buffer";
import {
  BASE58_ALPHABET_TEXT,
  BASE64_ENCODING,
  ED25519_ALGORITHM,
  HEX_ENCODING,
  JS_TYPE_BIGINT,
  JS_TYPE_FUNCTION,
  JS_TYPE_NUMBER,
  JS_TYPE_OBJECT,
  JS_TYPE_STRING,
  UTF8_ENCODING,
} from "./commonLiterals.js";
import { blake3 } from "@noble/hashes/blake3";
import { sha256 } from "@noble/hashes/sha2";
import { blake2b256 } from "./blake2b.js";
import { createBlockProofVerification } from "./blockProofVerification.js";
import { crc64Xz } from "./crc64Xz.js";
import {
  AccountAddress,
  curveIdFromAlgorithm,
  curveIdToAlgorithm,
  ensureCurveIdEnabled,
  normalizeBytes,
  validatePublicKeyForCurve,
} from "./address.js";
import { canonicalizeDomainIdLabel } from "./domainId.js";
import {
  getCurveEntryByPublicKeyMulticodec,
  publicKeyMulticodecForCurveId,
} from "./curveRegistry.js";
import { MultisigSpec } from "./multisig.js";
import {
  normalizeAccountId,
  normalizeAssetHoldingId,
  normalizeAssetId,
} from "./normalizers.js";
import {
  defaultNativeRuntime,
  resolveNativeRuntimeBinding,
} from "./nativeRuntime.js";
import {
  createNoritoContractCodecs,
  createNoritoMerkleProofCodecs,
  createNoritoConfidentialMemoCodecs,
} from "./noritoContractCodecs.js";
import {
  createNoritoGovernanceInstructionBoundary,
  parseStrictGovernanceInstructionJson,
} from "./noritoGovernanceBoundary.js";
import { computeHashLiteralCrc } from "./hashLiteralCrc.js";
import { createNoritoNftMarketCodecs, NFT_MARKET_INSTRUCTION_NAMES_V1, NFT_MARKET_INSTRUCTION_WIRE_IDS_V1 } from "./noritoNftMarketCodecs.js";
import {createNoritoGameCodecs} from './noritoGameCodecs.js';
import { GAME_INSTRUCTION_NAMES_V1, GAME_INSTRUCTION_WIRE_IDS_V1, gameValueMaximumBytesV1 } from './noritoGameRegistry.js';
import {createNoritoGameInstructionCodecs} from './noritoGameInstructionCodecs.js';
import {createNoritoGameResourceEngine} from './noritoGameResourceEngine.js';
import { KotodamaQuantity, NumericV1 } from "./numericV1.js";
import {
  PRIVACY_EXACT12_TRANSACTION_PAYLOAD_FIELD_NAMES_V1,
  validatePrivacyExact12NetworkBindingsV1,
} from "./privacyExact12Network.js";
import {
  LANE_PRIVACY_MERKLE_MAX_DEPTH,
  PROOF_BOX_MAX_ENCODED_BYTES,
  isPortableVerifyingKeyIdField,
  laneMerkleLeafIndexFitsDepth,
  proofBoxFitsEncodedBudget,
  proofBoxMaxProofBytes,
} from "./proofAttachment.js";

const TEXT_SMART_CONTRACT_CODE = "smart_contract_code::";
const TEXT_MUST_BE_GREATER_THAN_ZERO = " must be greater than zero";
const TEXT_MUST_BE_AN_OBJECT_2 = " must be an object";
const TEXT_CONTRACT_ADDRESS_2 = "contract_address";
const TEXT_CANCEL_CONFIDENTIAL_POLICY_TRANSITION = "CancelConfidentialPolicyTransition";
const TEXT_EXPECTED_REVISION_2 = "expected_revision";
const TEXT_ASSET_DEFINITION_ID = "asset_definition_id";
const TEXT_SCHEDULE_CONFIDENTIAL_POLICY_TRANSITION = "ScheduleConfidentialPolicyTransition";
const TEXT_MUST_CONTAIN = " must contain ";
const TEXT_CANONICAL = " canonical ";
const TEXT_REQUIRES_VALIDATION_FEE_POLICY_METADATA = " requires validation fee policy metadata";
const TEXT_EXPECTED_REMAINING_AMOUNT = "expected_remaining_amount";
const TEXT_CARRIES_A_SUBSTITUTED = " carries a substituted ";
const TEXT_SET_CONTRACT_PARLIAMENT_DELEGATION_2 = "SetContractParliamentDelegation";
const TEXT_FINALIZE_SMART_CONTRACT_CODE_UPLOAD_2 = "FinalizeSmartContractCodeUpload";
const TEXT_MUST_BE = " must be ";
const TEXT_ELECTION_ID = "election_id";
const TEXT_TRANSACTION_INTENT_PROJECTION_NORITO_2 = "transactionIntentProjectionNorito";
const TEXT_COMMITMENT = "commitment";
const TEXT_EXPECTED_ASSIGNMENT_REVISION = "expected_assignment_revision";
const TEXT_UNSIGNED_TRANSACTION_PAYLOAD_NORITO_3 = "unsignedTransactionPayloadNorito";
const TEXT_UPLOAD_SMART_CONTRACT_CODE_CHUNK_2 = "UploadSmartContractCodeChunk";
const TEXT_EXPIRE_REPLICATION_ORDER_2 = "ExpireReplicationOrder";
const TEXT_SUBMIT_PROOF_INSTRUCTION_NORITO = "submit_proof_instruction_norito";
const TEXT_REGISTER_SMART_CONTRACT_BYTES = "RegisterSmartContractBytes";
const TEXT_DEACTIVATE_CONTRACT_INSTANCE_2 = "DeactivateContractInstance";
const TEXT_EXCEEDS_THE = " exceeds the ";
const TEXT_EXPECTED_AUTHORITY = "expected_authority";
const TEXT_CODE_HASH_2 = "code_hash";
const TEXT_MUST_NOT_BE = " must not be ";
const TEXT_TRANSACTION_INTENT = "transaction_intent_";
const TEXT_SUBMIT_PROOF_INSTRUCTION_NORITO_2 = "submitProofInstructionNorito";
const TEXT_ACTIVATE_CONTRACT_INSTANCE = "ActivateContractInstance";
const TEXT_REMOVE_SMART_CONTRACT_BYTES = "RemoveSmartContractBytes";
const TEXT_REGISTER_VERIFYING_KEY = "RegisterVerifyingKey";
const TEXT_INSTRUCTION = "instruction ";
const TEXT_PRIMARY_REFERENCE = "primary_reference";
const TEXT_MUST_USE_A_NATIVE_HASH_WITH_ITS_MARKER_BIT_SET_2 = " must use a native hash with its marker bit set";
const TEXT_VALIDATION_FEE = "validation_fee_";
const TEXT_ACCOUNT_ID = "account_id";
const TEXT_SET_ASSET_TRANSFER_BLACKLIST_2 = "SetAssetTransferBlacklist";
const TEXT_REPORT_KAIGI_RELAY_HEALTH = "ReportKaigiRelayHealth";
const TEXT_IROHA_DATA_MODEL = "iroha_data_model::";
const TEXT_OFFER_CONTRACT_OWNERSHIP = "OfferContractOwnership";
const TEXT_FINALIZE_ELECTION = "FinalizeElection";
const TEXT_CREATION_TIME_MS = "creation_time_ms";
const TEXT_COMPLETION_EPOCH = "completion_epoch";
const TEXT_FINALIZED_ANCHOR = "finalized_anchor";
const TEXT_EXPIRATION_EPOCH = "expiration_epoch";
const TEXT_SET_KAIGI_RELAY_MANIFEST = "SetKaigiRelayManifest";
const TEXT_REGISTER_ZK_ASSET = "RegisterZkAsset";
const TEXT_UPDATE_VERIFYING_KEY = "UpdateVerifyingKey";
const TEXT_PREDECESSOR_DIGEST = "predecessor_digest";
const TEXT_PUBLIC_INPUTS_SCHEMA_HASH = "public_inputs_schema_hash";
const TEXT_UNSUPPORTED = "unsupported ";
const TEXT_COMPLETE_REPLICATION_ORDER_2 = "CompleteReplicationOrder";
const TEXT_CLAIM_TWITTER_FOLLOW_REWARD = "ClaimTwitterFollowReward";


const TEXT_IROHA_DATA_MODEL_ISI = (TEXT_IROHA_DATA_MODEL + "isi::");
const TEXT_IROHA_INSTRUCTION_V1 = "iroha.instruction.v1::";
const TEXT_MULTISIG_PROPOSE_DTO_VALIDATION_FEE = ("MultisigProposeDto." + TEXT_VALIDATION_FEE);
const TEXT_INTERNAL_NORITO_CANONICALIZATION_DOES_NOT_SUPPORT = "Internal Norito canonicalization does not support ";
const TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1 = "PrivacyExact12FixtureBundleV1.";
const TEXT_SET_ASSET_TRANSFER_AVAILABILITY = "SetAssetTransferAvailability.";
const TEXT_REPLICATION_ORDER_V1 = "ReplicationOrderV1.";
const TEXT_MUST_BE_AN_OBJECT = TEXT_MUST_BE_AN_OBJECT_2;
const TEXT_USES_UNSUPPORTED = (" uses " + TEXT_UNSUPPORTED);
const TEXT_MULTISIG_CONTRACT_CALL_PROPOSE_DTO = "MultisigContractCallProposeDto.";
const TEXT_COMPLETE_REPLICATION_ORDER = "CompleteReplicationOrder.";
const TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1_2 = "PrivacyExact12FixtureBundleV1 ";
const TEXT_INTERNAL_NORITO_DECODER_DOES_NOT_SUPPORT = "Internal Norito decoder does not support ";
const TEXT_MULTISIG_CONTRACT_CALL_APPROVE_DTO = "MultisigContractCallApproveDto.";
const TEXT_ZK_CREATE_ELECTION = "zk.CreateElection.";
const TEXT_SET_ASSET_TRANSFER_CONTROL = "SetAssetTransferControl.";
const TEXT_NATIVE_GAME_VALUE_EXCEEDS_ITS_COMPILED_PAYLOAD_LIMIT = "native game value exceeds its compiled payload limit";
const TEXT_SORA_FS_BILLING_ACKNOWLEDGEMENT = "SoraFS billing acknowledgement ";
const TEXT_MUST_USE_A_NATIVE_HASH_WITH_ITS_MARKER_BIT_SET = TEXT_MUST_USE_A_NATIVE_HASH_WITH_ITS_MARKER_BIT_SET_2;
const TEXT_ZK_SCHEDULE_CONFIDENTIAL_POLICY_TRANSITION = "zk.ScheduleConfidentialPolicyTransition.";
const TEXT_MUST_CONTAIN_EXACTLY = (TEXT_MUST_CONTAIN + "exactly ");
const TEXT_PROPOSE_DEPLOY_CONTRACT = "ProposeDeployContract.";
const TEXT_MUST_BE_EXACT_STANDARD_BASE64 = (TEXT_MUST_BE + "exact standard-base64");
const TEXT_ISSUE_REPLICATION_ORDER = "IssueReplicationOrder.";
const TEXT_PAYLOAD_PROOF_AUDIT_PATH_MUST = ".payload.proof.audit_path must ";
const TEXT_MULTISIG_PROPOSE_DTO = "MultisigProposeDto.";
const TEXT_COMMIT_CONTRACT_DEPLOYMENT = "CommitContractDeployment.";
const TEXT_UNSIGNED_TRANSACTION_PAYLOAD_NORITO = ".unsignedTransactionPayloadNorito.";
const TEXT_SIGNED_TRANSACTION_VERSIONED_NORITO = ".signedTransactionVersionedNorito ";
const TEXT_ZK_FINALIZE_ELECTION = "zk.FinalizeElection.";
const TEXT_CANCEL_ASSET_LOCK_V1 = "CancelAssetLockV1.";
const TEXT_CONTRACT_ADDRESS = TEXT_CONTRACT_ADDRESS_2;
const TEXT_ENVELOPE_NORITO_CARRIES_A_SUBSTITUTED = (".envelopeNorito" + TEXT_CARRIES_A_SUBSTITUTED);
const TEXT_MUST_BE_A = (TEXT_MUST_BE + "a ");
const TEXT_SET_ASSET_TRANSFER_BLACKLIST = "SetAssetTransferBlacklist.";
const TEXT_EXPECTED_REVISION = TEXT_EXPECTED_REVISION_2;
const TEXT_ZK_SUBMIT_BALLOT = "zk.SubmitBallot.";
const TEXT_RECORD_SCCP_MESSAGE_PAYLOAD = "RecordSccpMessage.payload_";
const TEXT_IS_REQUIRED = " is required";
const TEXT_SIGNED_TRANSACTION_VERSIONED_NORITO_2 = ".signedTransactionVersionedNorito.";
const TEXT_BACKEND = ".backend";
const TEXT_MUST_NOT_CONTAIN = " must not contain ";
const TEXT_CANCEL_ASSET_LOCK_V1_2 = "CancelAssetLockV1 ";
const TEXT_VALUE_PROGRAM = ".value.program_";
const TEXT_PREDECESSOR_DIGEST_IS_REQUIRED_AFTER_REVISION_1 = ".predecessor_digest is required after revision 1";
const TEXT_UNSIGNED_TRANSACTION_PAYLOAD_NORITO_DOES_NOT_CONTAIN_THE = ".unsignedTransactionPayloadNorito does not contain the ";
const TEXT_PREDECESSOR_DIGEST_MUST_BE_NULL_AT_REVISION_1 = (".predecessor_digest" + TEXT_MUST_BE + "null at revision 1");
const TEXT_EXPIRE_REPLICATION_ORDER = "ExpireReplicationOrder.";
const TEXT_DESTINATION = ".destination";
const TEXT_MUST_BE_AN_EXACT_CANONICAL_I105_ACCOUNT_ID = (" must be an exact" + TEXT_CANONICAL + "I105 account id");
const TEXT_VARINT_EXCEEDS_AN_UNSIGNED_64_BIT_INTEGER = " varint exceeds an unsigned 64-bit integer";
const TEXT_EXCEEDS_JAVA_SCRIPT_S_SAFE_INTEGER_RANGE = " exceeds JavaScript's safe integer range";
const TEXT_CODE_HASH = TEXT_CODE_HASH_2;
const TEXT_NATIVE_GAME_VALUE_IS_NOT_BYTE_CANONICAL = "native game value is not byte-canonical";
const TEXT_MUST_FIT_IN_AN_UNSIGNED_64_BIT_INTEGER = " must fit in an unsigned 64-bit integer";
const TEXT_SUBMIT_PROOF_WIRE_ID_MUST_BE = (".submitProofWireId" + TEXT_MUST_BE);
const TEXT_TRANSACTION_INTENT_PROJECTION_NORITO = ("." + TEXT_TRANSACTION_INTENT + "projection_norito");
const TEXT_SUBMIT_PROOF_INSTRUCTION_NORITO_PAYLOAD = ".submitProofInstructionNorito.payload";
const TEXT_VERIFYING_KEYS = "verifying_keys.";
const TEXT_CANCEL_ASSET_LOCK = "CancelAssetLock.";
const TEXT_VALUE_CHARGE_LIMITS_MUST = ".value.charge_limits must ";
const TEXT_BLOCK_PROOFS = "BlockProofs.";
const TEXT_UNSIGNED_TRANSACTION_PAYLOAD_NORITO_2 = ".unsigned_transaction_payload_norito";
const TEXT_SIGNED_TRANSACTION = ".signed_transaction_";


// Reuse exact wire names and diagnostic fields throughout this module.
const CONTEXT_MINT_TRIGGER_REPETITIONS = "Mint.TriggerRepetitions";
const CONTEXT_BURN_TRIGGER_REPETITIONS = "Burn.TriggerRepetitions";
const CONTEXT_TRANSFER_DOMAIN = "Transfer.Domain";
const CONTEXT_TRANSFER_ASSET_DEFINITION = "Transfer.AssetDefinition";
const CONTEXT_REGISTER_DOMAIN = "Register.Domain";
const CONTEXT_REGISTER_ACCOUNT = "Register.Account";
const CONTEXT_REGISTER_ASSET_DEFINITION = "Register.AssetDefinition";
const WIRE_TYPE_TOP_UP_KAGEMUSHA_V1 = "TopUpKagemushaV1";
const WIRE_TYPE_CANCEL_ASSET_LOCK = "CancelAssetLock";
const WIRE_TYPE_RECORD_SCCP_MESSAGE = "RecordSccpMessage";
const WIRE_FIELD_ASSET_DEFINITION_ID = TEXT_ASSET_DEFINITION_ID;
const FIELD_INSTRUCTION = "instruction";
const WIRE_TYPE_OPEN_VERIFY_ENVELOPE = "OpenVerifyEnvelope";
const WIRE_TYPE_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1 = "PrivacyExact12FixtureBundleV1";
const WIRE_ID_IROHA_TRANSFER = "iroha.transfer";
const WIRE_ID_IROHA_REGISTER = "iroha.register";
const WIRE_ID_IROHA_CUSTOM = "iroha.custom";
const WIRE_FIELD_EXPECTED_REMAINING_AMOUNT = TEXT_EXPECTED_REMAINING_AMOUNT;
const WIRE_FIELD_ACCOUNT_ID = TEXT_ACCOUNT_ID;
const WIRE_FIELD_EXPECTED_REVISION = TEXT_EXPECTED_REVISION;
const FIELD_VARIANT_INDEX = "variantIndex";
const WIRE_FIELD_CONTRACT_ADDRESS = TEXT_CONTRACT_ADDRESS;
const WIRE_FIELD_CODE_HASH = TEXT_CODE_HASH;
const WIRE_FIELD_ELECTION_ID = TEXT_ELECTION_ID;
const FIELD_COMMITMENT = TEXT_COMMITMENT;
const FIELD_QUANTITY = "quantity";
const FIELD_DESTINATION = "destination";
const FIELD_METADATA = "metadata";
const WIRE_TYPE_DATASPACE_RESTRICTED = "DataspaceRestricted";
const WIRE_TYPE_INFINITELY = "Infinitely";
const WIRE_FIELD_ORDER_ID = "order_id";
const WIRE_TYPE_ISSUE_REPLICATION_ORDER = "IssueReplicationOrder";
const WIRE_TYPE_COMPLETE_REPLICATION_ORDER = TEXT_COMPLETE_REPLICATION_ORDER_2;
const WIRE_TYPE_EXPIRE_REPLICATION_ORDER = TEXT_EXPIRE_REPLICATION_ORDER_2;

const ALIGNMENT = 16;
const COMPACT_LEN_FLAG = 0x02;
const NORITO_FRAME_HEADER_LENGTH = 40;
const NORITO_MAX_HEADER_PADDING = 64;
const NORITO_PACKED_SEQ_FLAG = 0x01;
const NORITO_PACKED_STRUCT_FLAG = 0x04;
const NORITO_FIELD_BITSET_FLAG = 0x20;
const NORITO_SUPPORTED_HEADER_FLAGS =
  NORITO_PACKED_SEQ_FLAG |
  COMPACT_LEN_FLAG |
  NORITO_PACKED_STRUCT_FLAG |
  NORITO_FIELD_BITSET_FLAG;
const UINT64_MASK = 0xffff_ffff_ffff_ffffn;
const ASSET_DEFINITION_ADDRESS_VERSION = 1;
const BASE58_ALPHABET = BASE58_ALPHABET_TEXT;
const HASH_LITERAL_RE = /^hash:([0-9A-Fa-f]{64})#([0-9A-Fa-f]{4})$/;
const CANONICAL_HASH_LITERAL_RE = /^hash:([0-9A-F]{64})#([0-9A-F]{4})$/;
const MULTIHASH_LITERAL_RE = /^([0-9a-fA-F]+)$/;
const SCHEDULE_CONFIDENTIAL_POLICY_TRANSITION_WIRE_ID =
  ("zk::" + TEXT_SCHEDULE_CONFIDENTIAL_POLICY_TRANSITION);
const CANCEL_CONFIDENTIAL_POLICY_TRANSITION_WIRE_ID =
  ("zk::" + TEXT_CANCEL_CONFIDENTIAL_POLICY_TRANSITION);
const SET_ASSET_TRANSFER_AVAILABILITY_VARIANT =
  "SetAssetTransferAvailability";
const SET_ASSET_TRANSFER_BLACKLIST_VARIANT = TEXT_SET_ASSET_TRANSFER_BLACKLIST_2;
const SET_ASSET_TRANSFER_CONTROL_VARIANT = "SetAssetTransferControl";
const SET_TRANSFER_REASON_CONTEXT = (TEXT_SET_ASSET_TRANSFER_AVAILABILITY + "reason");
const COMPLETE_ORDER_REVISION_CONTEXT = (TEXT_COMPLETE_REPLICATION_ORDER + TEXT_EXPECTED_ASSIGNMENT_REVISION);
const COMPLETE_ORDER_REVISION_MESSAGE = (TEXT_COMPLETE_REPLICATION_ORDER + "expected_assignment_revision" + TEXT_MUST_BE_GREATER_THAN_ZERO);
const CANCEL_LOCK_REMAINING_CONTEXT = (TEXT_CANCEL_ASSET_LOCK + TEXT_EXPECTED_REMAINING_AMOUNT);
const CANCEL_LOCK_REMAINING_MESSAGE = (TEXT_CANCEL_ASSET_LOCK + "expected_remaining_amount" + TEXT_MUST_BE_GREATER_THAN_ZERO);
const ISSUE_ORDER_DEADLINE_CONTEXT = (TEXT_ISSUE_REPLICATION_ORDER + "deadline_epoch");
const ISSUE_ORDER_DEADLINE_MESSAGE = (TEXT_ISSUE_REPLICATION_ORDER + "deadline_epoch" + TEXT_MUST_BE + "greater than issued_epoch");
const ISSUE_ORDER_PAYLOAD_CONTEXT = (TEXT_ISSUE_REPLICATION_ORDER + "order_payload");
const ISSUE_ORDER_EPOCH_CONTEXT = (TEXT_ISSUE_REPLICATION_ORDER + "issued_epoch");
const ISSUE_ORDER_ID_CONTEXT = (TEXT_ISSUE_REPLICATION_ORDER + "order_id");
const EXPECTED_PREVIOUS_CONTRACT_CONTEXT = (TEXT_COMMIT_CONTRACT_DEPLOYMENT + "expected_previous_" + TEXT_CONTRACT_ADDRESS_2);
const SCHEDULE_CONVERSION_WINDOW_CONTEXT = (TEXT_ZK_SCHEDULE_CONFIDENTIAL_POLICY_TRANSITION + "conversion_window");
const SCHEDULE_EFFECTIVE_HEIGHT_CONTEXT = (TEXT_ZK_SCHEDULE_CONFIDENTIAL_POLICY_TRANSITION + "effective_height");
const SCHEDULE_TRANSITION_ID_CONTEXT = (TEXT_ZK_SCHEDULE_CONFIDENTIAL_POLICY_TRANSITION + "transition_id");
const SUPPORTED_JS_CANONICALIZATION_INSTRUCTIONS = [
  "Mint.Asset",
  CONTEXT_MINT_TRIGGER_REPETITIONS,
  "Burn.Asset",
  CONTEXT_BURN_TRIGGER_REPETITIONS,
  CONTEXT_TRANSFER_DOMAIN,
  CONTEXT_TRANSFER_ASSET_DEFINITION,
  "Transfer.Asset",
  "Transfer.Nft",
  CONTEXT_REGISTER_DOMAIN,
  CONTEXT_REGISTER_ACCOUNT,
  CONTEXT_REGISTER_ASSET_DEFINITION,
  "ExecuteTrigger",
  "Custom",
  "Kaigi.*",
  "Governance.*",
  "Social.*",
  "SmartContract.*",
  WIRE_TYPE_TOP_UP_KAGEMUSHA_V1,
  "zk.*",
  "VerifyingKey.*",
  "Rwa.*",
  WIRE_TYPE_CANCEL_ASSET_LOCK,
  SET_ASSET_TRANSFER_AVAILABILITY_VARIANT,
  SET_ASSET_TRANSFER_BLACKLIST_VARIANT,
  SET_ASSET_TRANSFER_CONTROL_VARIANT,
  "SoraFS.ReplicationOrder.*",
  WIRE_TYPE_RECORD_SCCP_MESSAGE,
];
const CANCEL_ASSET_LOCK_WIRE_ID =
  (TEXT_IROHA_INSTRUCTION_V1 + "escrow::CancelAssetLock");
const CANCEL_ASSET_LOCK_INNER_TYPE_NAME =
  `${TEXT_IROHA_DATA_MODEL_ISI}escrow::CancelAssetLock`;
const CANCEL_ASSET_LOCK_V1_SCHEMA_HASH = /* @__PURE__ */ schemaHashForTypeName(
  CANCEL_ASSET_LOCK_INNER_TYPE_NAME,
);
// A transparent 32-byte EscrowId plus one positive signed-512-bit Quantity
// yields an unpadded canonical archive in this exact range. Enforce it before
// CRC work so an oversized attacker-controlled frame cannot make this fixed
// schema perform an unbounded payload scan.
const CANCEL_ASSET_LOCK_V1_MIN_ARCHIVE_BYTES = 85;
const CANCEL_ASSET_LOCK_V1_MAX_ARCHIVE_BYTES = 148;
const SET_ASSET_TRANSFER_AVAILABILITY_WIRE_ID =
  "iroha.asset.transfer.availability.set";
const SET_ASSET_TRANSFER_BLACKLIST_WIRE_ID =
  "iroha.asset.transfer.blacklist.set";
const SET_ASSET_TRANSFER_CONTROL_WIRE_ID =
  "iroha.asset.transfer.control.set";
const ASSET_TRANSFER_AVAILABILITY_MAX_REASON_BYTES_V1 = 512;
const RECORD_SCCP_MESSAGE_WIRE_ID =
  (TEXT_IROHA_INSTRUCTION_V1 + "bridge::RecordSccpMessage");
const ISSUE_REPLICATION_ORDER_WIRE_ID =
  (TEXT_IROHA_INSTRUCTION_V1 + "sorafs::IssueReplicationOrder");
const COMPLETE_REPLICATION_ORDER_WIRE_ID =
  (TEXT_IROHA_INSTRUCTION_V1 + "sorafs::" + TEXT_COMPLETE_REPLICATION_ORDER_2);
const EXPIRE_REPLICATION_ORDER_WIRE_ID =
  (TEXT_IROHA_INSTRUCTION_V1 + "sorafs::" + TEXT_EXPIRE_REPLICATION_ORDER_2);
const REPLICATION_ORDER_V1_SCHEMA_HASH = /* @__PURE__ */ schemaHashForTypeName(
  "sorafs_manifest::capacity::ReplicationOrderV1",
);
const SORAFS_REPLICATION_ORDER_MAX_PAYLOAD_BYTES_V1 = 1024 * 1024;
const SORAFS_REPLICATION_ORDER_CHUNKER_HANDLES_V1 = /* @__PURE__ */ new Set([
  "sorafs.sf1@1.0.0",
  "sorafs.sf2@1.0.0",
]);
const INSTRUCTION_BOX_SCHEMA_HASH = Buffer.from(
  "862a7d77075d4d23ff6c1261db027811",
  HEX_ENCODING,
);
const MULTISIG_PROPOSE_DTO_SCHEMA_HASH = /* @__PURE__ */ schemaHashForTypeName(
  "iroha_torii::routing::MultisigProposeDto",
);
const MULTISIG_CONTRACT_CALL_PROPOSE_DTO_SCHEMA_HASH = /* @__PURE__ */ schemaHashForTypeName(
  "iroha_torii::routing::MultisigContractCallProposeDto",
);
const MULTISIG_CONTRACT_CALL_APPROVE_DTO_SCHEMA_HASH = /* @__PURE__ */ schemaHashForTypeName(
  "iroha_torii::routing::MultisigContractCallApproveDto",
);
const OPEN_VERIFY_ENVELOPE_SCHEMA_HASH = /* @__PURE__ */ schemaHashForTypeName(
  (TEXT_IROHA_DATA_MODEL + "zk::OpenVerifyEnvelope"),
);
const EVENT_FILTER_BOX_SCHEMA_HASH = /* @__PURE__ */ schemaHashForTypeName(
  (TEXT_IROHA_DATA_MODEL + "events::model::EventFilterBox"),
);
export const PRIVACY_EXACT12_FIXTURE_BUNDLE_SCHEMA_NAME_V1 =
  "iroha.privacy.exact12-typed-fixture-bundle.v1";
export const PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1 = 2 * 1024 * 1024;
export const PRIVACY_EXACT12_PROTOCOL_IDS_V1 = /* @__PURE__ */ Object.freeze([
  "zk-ace-pq-authorization-v1",
  "anonymous-pgc-k-out-of-n-v1",
  "verange-transparent-range-v1",
  "iroha-zk-ams-v1",
  "vega-existing-credential-zk-v1",
  "iroha-zk-x509-stark-p256-v1",
  "iroha-jindo-polynomial-commitment-v1",
  "iroha-bootle-lantern-anoncred-v1",
  "orchard-halo2-actions-v1",
  "monero-fcmp-plus-plus-v1",
  "iroha-ivm-private-note-stark-v1",
  "pq-masp-stark-v1",
]);
/** Exact bare-wire magic for `ConfidentialMemoEnvelopeV1`. */
export const CONFIDENTIAL_MEMO_WIRE_MAGIC_V1 = /* @__PURE__ */ Object.freeze([
  0x49, 0x52, 0x48, 0x43, 0x4d, 0x31, 0xa5, 0x5a,
]);
/** Every confidential memo has exactly eight real-or-padding slots. */
export const CONFIDENTIAL_MEMO_RECIPIENT_SLOTS_V1 = 8;
/** Maximum authenticated memo ciphertext accepted by the V1 wire. */
export const CONFIDENTIAL_MEMO_MAX_CIPHERTEXT_BYTES_V1 = 64 * 1024;
const PRIVACY_EXACT12_FIXTURE_BUNDLE_SCHEMA_HASH_V1 = /* @__PURE__ */ schemaHashForTypeName(
  PRIVACY_EXACT12_FIXTURE_BUNDLE_SCHEMA_NAME_V1,
);
const PRIVACY_EXACT12_STATEMENT_SCHEMA_HASH_V1 = /* @__PURE__ */ schemaHashForTypeName(
  "iroha.privacy.statement.v1",
);
const PRIVACY_EXACT12_ENVELOPE_SCHEMA_HASH_V1 = /* @__PURE__ */ schemaHashForTypeName(
  "iroha.privacy.proof-envelope.v1",
);
const PRIVACY_EXACT12_SUBMIT_PROOF_SCHEMA_HASH_V1 = /* @__PURE__ */ schemaHashForTypeName(
  `${TEXT_IROHA_DATA_MODEL_ISI}privacy::SubmitPrivacyProofV1`,
);
const PRIVACY_EXACT12_TRANSACTION_PAYLOAD_SCHEMA_HASH_V1 = /* @__PURE__ */ schemaHashForTypeName(
  (TEXT_IROHA_DATA_MODEL + "transaction::signed::model::TransactionPayload"),
);
const PRIVACY_EXACT12_SUBMIT_PROOF_WIRE_ID_V1 =
  "iroha.privacy.submit_proof.v1";
const PRIVACY_EXACT12_INTENT_DIGEST_DOMAIN_V1 = /* @__PURE__ */ Buffer.from(
  "iroha.privacy.transaction-intent-digest.v1",
  "ascii",
);
const PRIVACY_EXACT12_ALIGNED_NESTED_FRAME_PADDING_V1 = 8;
const PRIVACY_EXACT12_TRANSACTION_PAYLOAD_FRAME_PADDING_V1 = 0;
const PRIVACY_EXACT12_ROW_FIELD_NAMES_V1 = /* @__PURE__ */ Object.freeze([
  "protocol_id",
  "statement_norito",
  "envelope_norito",
  "submit_proof_wire_id",
  TEXT_SUBMIT_PROOF_INSTRUCTION_NORITO,
  (TEXT_TRANSACTION_INTENT + "projection_norito"),
  (TEXT_TRANSACTION_INTENT + "digest"),
  "unsigned_transaction_payload_norito",
  "signed_transaction_versioned_norito",
  "signed_transaction_hash",
]);
const PRIVACY_EXACT12_PUBLIC_ROW_FIELD_NAMES_V1 = /* @__PURE__ */ Object.freeze([
  "protocolId",
  "statementNorito",
  "envelopeNorito",
  "submitProofWireId",
  TEXT_SUBMIT_PROOF_INSTRUCTION_NORITO_2,
  TEXT_TRANSACTION_INTENT_PROJECTION_NORITO_2,
  "transactionIntentDigest",
  TEXT_UNSIGNED_TRANSACTION_PAYLOAD_NORITO_3,
  "signedTransactionVersionedNorito",
  "signedTransactionHash",
]);
const PRIVACY_EXACT12_ENVELOPE_FIELD_NAMES_V1 = /* @__PURE__ */ Object.freeze([
  "wire_magic",
  ("catalog_" + TEXT_COMMITMENT),
  "protocol_id",
  "proof_system_id",
  "engine_id",
  "parameter_id",
  "parameter_digest",
  "verifier_digest",
  "statement_schema_digest",
  "engine_manifest_digest",
  "statement_digest",
  "statement",
  "proof",
]);
const PRIVACY_EXACT12_WIRE_MAGIC_V1 = /* @__PURE__ */ Buffer.from("4952485a4b31a55a", "hex");
const PRIVACY_EXACT12_CATALOG_COMMITMENT_V1 = /* @__PURE__ */ Buffer.from(
  "e037f13904a0307c00db15d85cfb406bd79772d20144a949def0f3fda78e342e747f65787cbfbffac94f11c369e2bbff",
  "hex",
);
const PRIVACY_EXACT12_PROOF_ENGINE_TAGS_V1 = /* @__PURE__ */ Object.freeze([
  0, 2, 3, 1, 4, 0, 5, 8, 6, 7, 0, 0,
]);
const TRANSACTION_PAYLOAD_BATCH_SCHEMA_HASH = /* @__PURE__ */ schemaHashForTypeName(
  "alloc::vec::Vec<alloc::vec::Vec<u8>>",
);
export const SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_NAME_V1 =
  "iroha.torii.v1.sorafs.billing.acknowledgement_proof";
const SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_HASH_V1 =
  /* @__PURE__ */ schemaHashForTypeName(
    SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_NAME_V1,
  );
export const SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1 =
  64 * 1024;
const CONTRACT_MANIFEST_SIGNATURE_PAYLOAD_SCHEMA_HASH = Buffer.from(
  "b4bb42540d44c468ed44d5f94c59b007",
  HEX_ENCODING,
);
const BLOCK_PROOFS_TYPE_NAME =
  (TEXT_IROHA_DATA_MODEL + "block::proofs::BlockProofs");
const REGISTER_SMART_CONTRACT_CODE_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + TEXT_SMART_CONTRACT_CODE + "RegisterSmartContractCode");
const REGISTER_SMART_CONTRACT_BYTES_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + TEXT_SMART_CONTRACT_CODE + "RegisterSmartContractBytes");
const DEACTIVATE_CONTRACT_INSTANCE_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + TEXT_SMART_CONTRACT_CODE + "DeactivateContractInstance");
const ACTIVATE_CONTRACT_INSTANCE_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + TEXT_SMART_CONTRACT_CODE + "ActivateContractInstance");
const SET_CONTRACT_PARLIAMENT_DELEGATION_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + TEXT_SMART_CONTRACT_CODE + "SetContractParliamentDelegation");
const OFFER_CONTRACT_OWNERSHIP_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + TEXT_SMART_CONTRACT_CODE + "OfferContractOwnership");
const ACCEPT_CONTRACT_OWNERSHIP_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + TEXT_SMART_CONTRACT_CODE + "AcceptContractOwnership");
const CANCEL_CONTRACT_OWNERSHIP_OFFER_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + TEXT_SMART_CONTRACT_CODE + "CancelContractOwnershipOffer");
const COMMIT_CONTRACT_DEPLOYMENT_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + TEXT_SMART_CONTRACT_CODE + "CommitContractDeployment");
const UPLOAD_SMART_CONTRACT_CODE_CHUNK_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + TEXT_SMART_CONTRACT_CODE + "UploadSmartContractCodeChunk");
const FINALIZE_SMART_CONTRACT_CODE_UPLOAD_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + TEXT_SMART_CONTRACT_CODE + "FinalizeSmartContractCodeUpload");
const CANCEL_SMART_CONTRACT_CODE_UPLOAD_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + TEXT_SMART_CONTRACT_CODE + "CancelSmartContractCodeUpload");
const REMOVE_SMART_CONTRACT_BYTES_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + TEXT_SMART_CONTRACT_CODE + "RemoveSmartContractBytes");
const CREATE_KAIGI_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + "kaigi::CreateKaigi");
const JOIN_KAIGI_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + "kaigi::JoinKaigi");
const LEAVE_KAIGI_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + "kaigi::LeaveKaigi");
const END_KAIGI_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + "kaigi::EndKaigi");
const RECORD_KAIGI_USAGE_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + "kaigi::RecordKaigiUsage");
const SET_KAIGI_RELAY_MANIFEST_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + "kaigi::" + TEXT_SET_KAIGI_RELAY_MANIFEST);
const REGISTER_KAIGI_RELAY_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + "kaigi::RegisterKaigiRelay");
const UNREGISTER_KAIGI_RELAY_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + "kaigi::UnregisterKaigiRelay");
const REPORT_KAIGI_RELAY_HEALTH_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + "kaigi::" + TEXT_REPORT_KAIGI_RELAY_HEALTH);
const PROPOSE_DEPLOY_CONTRACT_WIRE_ID =
  (TEXT_IROHA_INSTRUCTION_V1 + "governance::ProposeDeployContract");
const CAST_ZK_BALLOT_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + "governance::CastZkBallot");
const CAST_PLAIN_BALLOT_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + "governance::CastPlainBallot");
const CLAIM_TWITTER_FOLLOW_REWARD_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + "social::" + TEXT_CLAIM_TWITTER_FOLLOW_REWARD);
const SEND_TO_TWITTER_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + "social::SendToTwitter");
const CANCEL_TWITTER_ESCROW_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + "social::CancelTwitterEscrow");
const REGISTER_ZK_ASSET_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + "zk::" + TEXT_REGISTER_ZK_ASSET);
const CREATE_ELECTION_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + "zk::CreateElection");
const SUBMIT_BALLOT_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + "zk::SubmitBallot");
const FINALIZE_ELECTION_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + "zk::" + TEXT_FINALIZE_ELECTION);
const REGISTER_VERIFYING_KEY_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + "verifying_keys::" + TEXT_REGISTER_VERIFYING_KEY);
const UPDATE_VERIFYING_KEY_WIRE_ID = (TEXT_IROHA_INSTRUCTION_V1 + "verifying_keys::" + TEXT_UPDATE_VERIFYING_KEY);
const TOP_UP_KAGEMUSHA_WIRE_ID = "iroha.kagemusha.v1.top_up";
const TOP_UP_KAGEMUSHA_INNER_TYPE_NAME =
  `${TEXT_IROHA_DATA_MODEL_ISI}kagemusha_v1::TopUpKagemushaV1`;
const KAGEMUSHA_TOP_UP_REQUEST_SCHEMA_NAME =
  "iroha.torii.v1.kagemusha.top_up.request";
const KAGEMUSHA_TOP_UP_REQUEST_SCHEMA_HASH = /* @__PURE__ */ schemaHashForTypeName(
  KAGEMUSHA_TOP_UP_REQUEST_SCHEMA_NAME,
);
const KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES = 16 * 1024;
const KAGEMUSHA_TOP_UP_REQUEST_HEADER_PADDING = 8;
const INNER_TYPE_NAME_BY_WIRE_ID = Object.freeze({
  ...Object.fromEntries(NFT_MARKET_INSTRUCTION_NAMES_V1.map((name, index) => [NFT_MARKET_INSTRUCTION_WIRE_IDS_V1[index], `${TEXT_IROHA_DATA_MODEL_ISI}nft_market::${name}`])),
  ...Object.fromEntries(GAME_INSTRUCTION_NAMES_V1.map((name, index) => [GAME_INSTRUCTION_WIRE_IDS_V1[index], `${TEXT_IROHA_DATA_MODEL_ISI}game::${name}`])),
  "iroha.mint": `${TEXT_IROHA_DATA_MODEL_ISI}mint_burn::MintBox`,
  "iroha.burn": `${TEXT_IROHA_DATA_MODEL_ISI}mint_burn::BurnBox`,
  "iroha.register": `${TEXT_IROHA_DATA_MODEL_ISI}register::RegisterBox`,
  "iroha.transfer": `${TEXT_IROHA_DATA_MODEL_ISI}transfer::TransferBox`,
  "iroha.custom": `${TEXT_IROHA_DATA_MODEL_ISI}transparent::CustomInstruction`,
  "iroha.execute_trigger": `${TEXT_IROHA_DATA_MODEL_ISI}transparent::ExecuteTrigger`,
  "iroha.rwa": `${TEXT_IROHA_DATA_MODEL_ISI}rwa::RwaInstructionBox`,
  [CANCEL_ASSET_LOCK_WIRE_ID]: CANCEL_ASSET_LOCK_INNER_TYPE_NAME,
  [SET_ASSET_TRANSFER_AVAILABILITY_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}asset_transfer_control::SetAssetTransferAvailability`,
  [SET_ASSET_TRANSFER_BLACKLIST_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}asset_transfer_control::${TEXT_SET_ASSET_TRANSFER_BLACKLIST_2}`,
  [SET_ASSET_TRANSFER_CONTROL_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}asset_transfer_control::SetAssetTransferControl`,
  [RECORD_SCCP_MESSAGE_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}bridge::RecordSccpMessage`,
  [ISSUE_REPLICATION_ORDER_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}sorafs::IssueReplicationOrder`,
  [COMPLETE_REPLICATION_ORDER_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}sorafs::${TEXT_COMPLETE_REPLICATION_ORDER_2}`,
  [EXPIRE_REPLICATION_ORDER_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}sorafs::${TEXT_EXPIRE_REPLICATION_ORDER_2}`,
  [CREATE_KAIGI_WIRE_ID]: `${TEXT_IROHA_DATA_MODEL_ISI}kaigi::CreateKaigi`,
  [JOIN_KAIGI_WIRE_ID]: `${TEXT_IROHA_DATA_MODEL_ISI}kaigi::JoinKaigi`,
  [LEAVE_KAIGI_WIRE_ID]: `${TEXT_IROHA_DATA_MODEL_ISI}kaigi::LeaveKaigi`,
  [END_KAIGI_WIRE_ID]: `${TEXT_IROHA_DATA_MODEL_ISI}kaigi::EndKaigi`,
  [RECORD_KAIGI_USAGE_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}kaigi::RecordKaigiUsage`,
  [SET_KAIGI_RELAY_MANIFEST_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}kaigi::${TEXT_SET_KAIGI_RELAY_MANIFEST}`,
  [REGISTER_KAIGI_RELAY_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}kaigi::RegisterKaigiRelay`,
  [UNREGISTER_KAIGI_RELAY_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}kaigi::UnregisterKaigiRelay`,
  [REPORT_KAIGI_RELAY_HEALTH_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}kaigi::${TEXT_REPORT_KAIGI_RELAY_HEALTH}`,
  [PROPOSE_DEPLOY_CONTRACT_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}governance::ProposeDeployContract`,
  [CAST_ZK_BALLOT_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}governance::CastZkBallot`,
  [CAST_PLAIN_BALLOT_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}governance::CastPlainBallot`,
  [CLAIM_TWITTER_FOLLOW_REWARD_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}social::${TEXT_CLAIM_TWITTER_FOLLOW_REWARD}`,
  [SEND_TO_TWITTER_WIRE_ID]: `${TEXT_IROHA_DATA_MODEL_ISI}social::SendToTwitter`,
  [CANCEL_TWITTER_ESCROW_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}social::CancelTwitterEscrow`,
  [REGISTER_SMART_CONTRACT_CODE_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}${TEXT_SMART_CONTRACT_CODE}RegisterSmartContractCode`,
  [REGISTER_SMART_CONTRACT_BYTES_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}${TEXT_SMART_CONTRACT_CODE}RegisterSmartContractBytes`,
  [DEACTIVATE_CONTRACT_INSTANCE_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}${TEXT_SMART_CONTRACT_CODE}DeactivateContractInstance`,
  [ACTIVATE_CONTRACT_INSTANCE_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}${TEXT_SMART_CONTRACT_CODE}ActivateContractInstance`,
  [SET_CONTRACT_PARLIAMENT_DELEGATION_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}${TEXT_SMART_CONTRACT_CODE}SetContractParliamentDelegation`,
  [OFFER_CONTRACT_OWNERSHIP_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}${TEXT_SMART_CONTRACT_CODE}OfferContractOwnership`,
  [ACCEPT_CONTRACT_OWNERSHIP_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}${TEXT_SMART_CONTRACT_CODE}AcceptContractOwnership`,
  [CANCEL_CONTRACT_OWNERSHIP_OFFER_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}${TEXT_SMART_CONTRACT_CODE}CancelContractOwnershipOffer`,
  [COMMIT_CONTRACT_DEPLOYMENT_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}${TEXT_SMART_CONTRACT_CODE}CommitContractDeployment`,
  [UPLOAD_SMART_CONTRACT_CODE_CHUNK_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}${TEXT_SMART_CONTRACT_CODE}UploadSmartContractCodeChunk`,
  [FINALIZE_SMART_CONTRACT_CODE_UPLOAD_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}${TEXT_SMART_CONTRACT_CODE}FinalizeSmartContractCodeUpload`,
  [CANCEL_SMART_CONTRACT_CODE_UPLOAD_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}${TEXT_SMART_CONTRACT_CODE}CancelSmartContractCodeUpload`,
  [REMOVE_SMART_CONTRACT_BYTES_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}${TEXT_SMART_CONTRACT_CODE}RemoveSmartContractBytes`,
  [REGISTER_ZK_ASSET_WIRE_ID]: `${TEXT_IROHA_DATA_MODEL_ISI}zk::${TEXT_REGISTER_ZK_ASSET}`,
  [SCHEDULE_CONFIDENTIAL_POLICY_TRANSITION_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}zk::${TEXT_SCHEDULE_CONFIDENTIAL_POLICY_TRANSITION}`,
  [CANCEL_CONFIDENTIAL_POLICY_TRANSITION_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}zk::${TEXT_CANCEL_CONFIDENTIAL_POLICY_TRANSITION}`,
  [CREATE_ELECTION_WIRE_ID]: `${TEXT_IROHA_DATA_MODEL_ISI}zk::CreateElection`,
  [SUBMIT_BALLOT_WIRE_ID]: `${TEXT_IROHA_DATA_MODEL_ISI}zk::SubmitBallot`,
  [FINALIZE_ELECTION_WIRE_ID]: `${TEXT_IROHA_DATA_MODEL_ISI}zk::${TEXT_FINALIZE_ELECTION}`,
  [REGISTER_VERIFYING_KEY_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}verifying_keys::${TEXT_REGISTER_VERIFYING_KEY}`,
  [UPDATE_VERIFYING_KEY_WIRE_ID]:
    `${TEXT_IROHA_DATA_MODEL_ISI}verifying_keys::${TEXT_UPDATE_VERIFYING_KEY}`,
  [TOP_UP_KAGEMUSHA_WIRE_ID]: TOP_UP_KAGEMUSHA_INNER_TYPE_NAME,
});
const INSTRUCTION_WIRE_SCHEMA_BINDINGS = /* @__PURE__ */ (() => Object.freeze(
  Object.entries(INNER_TYPE_NAME_BY_WIRE_ID).map(
    ([outerWireId, innerTypeName]) =>
      Object.freeze({ outerWireId, innerTypeName }),
  ),
))();
const INNER_SCHEMA_HASH_BY_WIRE_ID = Object.freeze(
  Object.fromEntries(
    Object.entries(INNER_TYPE_NAME_BY_WIRE_ID).map(([wireId, typeName]) => [
      wireId,
      /* @__PURE__ */ schemaHashForTypeName(typeName),
    ]),
  ),
);
// `TopUpKagemushaV1` embeds a `u128`-aligned request, so its dynamic
// instruction frame has the eight zero bytes required to align the payload
// after Norito's 40-byte header.  This is part of the canonical
// `InstructionBox` bytes and must match `frame_bare_with_header_flags::<T>`.
const INNER_HEADER_PADDING_BY_WIRE_ID = Object.freeze({
  [TOP_UP_KAGEMUSHA_WIRE_ID]: 8,
});
const BASE58_LOOKUP = new Map(
  Array.from(BASE58_ALPHABET, (char, index) => [char, BigInt(index)]),
);
let noritoLengthFlags = 0;
class BufferReader {
  constructor(buffer, context, lengthFlags = noritoLengthFlags) {
    this.buffer = buffer;
    this.context = context;
    this.lengthFlags = lengthFlags;
    this.offset = 0;
  }
  readU8(name) {
    this.#ensureAvailable(1, name);
    const value = this.buffer[this.offset];
    this.offset += 1;
    return value;
  }
  readU16LE(name) {
    this.#ensureAvailable(2, name);
    const value = this.buffer.readUInt16LE(this.offset);
    this.offset += 2;
    return value;
  }
  readU32LE(name) {
    this.#ensureAvailable(4, name);
    const value = this.buffer.readUInt32LE(this.offset);
    this.offset += 4;
    return value;
  }
  readU64LE(name) {
    this.#ensureAvailable(8, name);
    const value = this.buffer.readBigUInt64LE(this.offset);
    this.offset += 8;
    return value;
  }
  readLength(name) {
    if ((this.lengthFlags & COMPACT_LEN_FLAG) !== 0) {
      const [value, bytesRead] = decodeUnsignedLeb128(
        this.buffer,
        this.offset,
        `${this.context}.${name}`,
      );
      this.offset += bytesRead;
      return value;
    }
    return bigintToSafeNumber(this.readU64LE(name), `${this.context}.${name}`);
  }
  readBytes(length, name) {
    const safeLength = Number(length);
    this.#ensureAvailable(safeLength, name);
    const value = this.buffer.subarray(this.offset, this.offset + safeLength);
    this.offset += safeLength;
    return value;
  }

  assertEof() {
    if (this.offset !== this.buffer.length) {
      rejectError(`${this.context} has ${this.buffer.length - this.offset} trailing bytes`);
    }
  }

  #ensureAvailable(length, name) {
    if (this.offset + length > this.buffer.length) {
      rejectError(`${this.context}.${name} overran payload (${length} bytes requested, ${this.buffer.length - this.offset} remaining)`);
    }
  }
}

function cloneJson(value) {
  if (typeof structuredClone === JS_TYPE_FUNCTION) {
    return structuredClone(value);
  }
  return JSON.parse(JSON.stringify(value));
}

function normalizeInstructionJsonValue(value) {
  if (value instanceof MultisigSpec) {
    return normalizeInstructionJsonValue(value.toPayload());
  }
  if (
    isPlainObject(value) &&
    value.quorum !== undefined &&
    value.signatories !== undefined &&
    (value.transaction_ttl_ms !== undefined || value.transactionTtlMs !== undefined)
  ) {
    return {
      quorum: normalizeInstructionJsonValue(value.quorum),
      signatories: normalizeInstructionJsonValue(value.signatories),
      transaction_ttl_ms: normalizeInstructionJsonValue(
        value.transaction_ttl_ms ?? value.transactionTtlMs,
      ),
    };
  }
  if (value instanceof Map) {
    return Object.fromEntries(
      Array.from(value.entries())
        .sort(([left], [right]) => String(left).localeCompare(String(right)))
        .map(([key, entryValue]) => [String(key), normalizeInstructionJsonValue(entryValue)]),
    );
  }
  if (Array.isArray(value)) {
    return value.map((entry) => normalizeInstructionJsonValue(entry));
  }
  if (isPlainObject(value)) {
    const normalized = {};
    for (const [key, entryValue] of Object.entries(value)) {
      normalized[key] = normalizeInstructionJsonValue(entryValue);
    }
    return normalized;
  }
  return value;
}

function resolveNative(method, nativeRuntime) {
  const native = resolveNativeRuntimeBinding(nativeRuntime);
  if (typeof native[method] !== JS_TYPE_FUNCTION) {
    rejectError(`Native binding does not expose ${method}`);
  }
  return native;
}

const {
  assertCanonicalGovernanceSelectorV1,
  isStrictGovernanceInstructionCandidate,
  validateCastZkBallotPayload,
  validateGovernanceInstructionBoundary,
  validateProposeDeployContractPayload,
} = /* @__PURE__ */ createNoritoGovernanceInstructionBoundary({
    assertExactNonEmptyString,
    assertOnlyObjectKeys,
    decodeExactStandardBase64,
    decodeManifestProvenanceValue: (...args) =>
      decodeManifestProvenanceValue(...args),
    encodeManifestProvenanceValue: (...args) =>
      encodeManifestProvenanceValue(...args),
    isPlainObject,
  });

const RETIRED_GENERIC_ZK_VARIANTS = Object.freeze([
  ["Shi", "eld"].join(""),
  ["Zk", "Transfer"].join(""),
  ["Un", "shield"].join(""),
]);

function rejectRetiredGenericZkInstruction(instruction) {
  if (!isPlainObject(instruction) || !isPlainObject(instruction.zk)) {
    return;
  }
  for (const variant of RETIRED_GENERIC_ZK_VARIANTS) {
    if (Object.prototype.hasOwnProperty.call(instruction.zk, variant)) {
      rejectType(`zk.${variant} is retired in ABI V1; use the typed KAGEMUSHA flow`);
    }
  }
}

function encodeNormalizedInstruction(normalized, nativeRuntime) {
  rejectRetiredGenericZkInstruction(normalized);
  validateGovernanceInstructionBoundary(normalized);
  try {
    return encodePureJsInstruction(normalized);
  } catch (error) {
    if (!isPureJsUnsupportedInstructionError(error)) {
      throw error;
    }
  }
  const native = resolveNative("noritoEncodeInstruction", nativeRuntime);
  return native.noritoEncodeInstruction(JSON.stringify(normalized));
}

class PureJsUnsupportedInstructionError extends Error {}

function isPureJsUnsupportedInstructionError(error) {
  return error instanceof PureJsUnsupportedInstructionError;
}

/**
 * Encode an instruction JSON payload to canonical Norito bytes.
 * @param {object | string | ArrayBufferView | ArrayBuffer | Buffer} instruction
 * @returns {Buffer}
 */
function encodeInstruction(instruction, nativeRuntime) {
  if (isBinaryLike(instruction)) {
    return toBuffer(instruction);
  }
  if (typeof instruction === JS_TYPE_STRING) {
    const trimmed = instruction.trim();
    let parsed;
    try {
      parsed = JSON.parse(trimmed);
    } catch (error) {
      if (!(error instanceof SyntaxError)) {
        throw error;
      }
      const decoded = tryDecodeBase64(trimmed) ?? tryDecodeHex(trimmed);
      if (decoded) {
        return decoded;
      }
      const native = resolveNative("noritoEncodeInstruction", nativeRuntime);
      return native.noritoEncodeInstruction(instruction);
    }
    const exactParsed = isStrictGovernanceInstructionCandidate(parsed)
      ? parseStrictGovernanceInstructionJson(trimmed, "governance instruction")
      : parsed;
    const normalized = normalizeInstructionJsonValue(exactParsed);
    return encodeNormalizedInstruction(normalized, nativeRuntime);
  }
  const normalized = normalizeInstructionJsonValue(cloneJson(instruction));
  return encodeNormalizedInstruction(normalized, nativeRuntime);
}

export function noritoEncodeInstruction(instruction) {
  return encodeInstruction(instruction, defaultNativeRuntime);
}

/**
 * Encode a `/v1/pipeline/transactions/batch` request body.
 *
 * Torii expects a Norito `Vec<Vec<u8>>` where each inner byte vector is one
 * versioned signed transaction payload.
 *
 * @param {ReadonlyArray<ArrayBufferView | ArrayBuffer | Buffer>} payloads
 * @returns {Buffer}
 */
export function noritoEncodeTransactionPayloadBatch(payloads) {
  if (!Array.isArray(payloads)) {
    rejectType(("transaction payload batch" + TEXT_MUST_BE + "an array"));
  }
  if (payloads.length === 0) {
    rejectType(("transaction payload batch" + TEXT_MUST_CONTAIN + "at least one payload"));
  }
  const payload = withNoritoCompactLengths(() =>
    encodeNoritoVec(payloads, (item, index) =>
      encodeByteVecValue(item, `transaction payload batch[${index}]`),
    ),
  );
  return frameNoritoPayload(
    payload,
    TRANSACTION_PAYLOAD_BATCH_SCHEMA_HASH,
    COMPACT_LEN_FLAG,
  );
}

/**
 * Encode the exact shared V1 SoraFS billing acknowledgement proof.
 *
 * The input surface is deliberately closed: nonce bytes, snake-case aliases,
 * hexadecimal proof strings, and additional fields are not accepted.
 *
 * @param {{requestNonceHex: string, authenticationProof: ArrayBufferView | ArrayBuffer | Buffer}} proof
 * @returns {Buffer}
 */
export function noritoEncodeSorafsBillingAcknowledgementProofV1(proof) {
  if (!isPlainObject(proof)) {
    rejectType((TEXT_SORA_FS_BILLING_ACKNOWLEDGEMENT + "proof" + TEXT_MUST_BE_AN_OBJECT_2));
  }
  const keys = Object.keys(proof);
  if (
    keys.length !== 2 ||
    !Object.prototype.hasOwnProperty.call(proof, "requestNonceHex") ||
    !Object.prototype.hasOwnProperty.call(proof, "authenticationProof")
  ) {
    rejectType((TEXT_SORA_FS_BILLING_ACKNOWLEDGEMENT + "proof" + TEXT_MUST_CONTAIN + "exactly requestNonceHex and authenticationProof"));
  }
  const requestNonceHex = proof.requestNonceHex;
  if (
    typeof requestNonceHex !== JS_TYPE_STRING ||
    !/^[0-9a-f]{64}$/u.test(requestNonceHex) ||
    /^0{64}$/u.test(requestNonceHex)
  ) {
    rejectType((TEXT_SORA_FS_BILLING_ACKNOWLEDGEMENT + "requestNonceHex" + TEXT_MUST_BE + "one non-zero lowercase 32-byte hexadecimal digest"));
  }
  const authenticationProof = proof.authenticationProof;
  if (
    !Buffer.isBuffer(authenticationProof) &&
    !ArrayBuffer.isView(authenticationProof) &&
    !(authenticationProof instanceof ArrayBuffer)
  ) {
    rejectType((TEXT_SORA_FS_BILLING_ACKNOWLEDGEMENT + "authenticationProof" + TEXT_MUST_BE + "binary bytes"));
  }
  const proofBytes = Buffer.isBuffer(authenticationProof)
    ? Buffer.from(authenticationProof)
    : ArrayBuffer.isView(authenticationProof)
      ? Buffer.from(
          authenticationProof.buffer,
          authenticationProof.byteOffset,
          authenticationProof.byteLength,
        )
      : Buffer.from(authenticationProof);
  if (
    proofBytes.length === 0 ||
    proofBytes.length >
      SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1
  ) {
    rejectRange(`${TEXT_SORA_FS_BILLING_ACKNOWLEDGEMENT}authenticationProof${TEXT_MUST_CONTAIN}1..=${SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1} bytes`);
  }
  const payload = withNoritoCompactLengths(() =>
    encodeStructValue([
      [
        encodeFixedBytesValue(
          Buffer.from(requestNonceHex, HEX_ENCODING),
          32,
          (TEXT_SORA_FS_BILLING_ACKNOWLEDGEMENT + "requestNonceHex"),
        ),
      ],
      [
        encodeByteVecValue(
          proofBytes,
          (TEXT_SORA_FS_BILLING_ACKNOWLEDGEMENT + "authenticationProof"),
        ),
      ],
    ]),
  );
  return frameNoritoPayload(
    payload,
    SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_HASH_V1,
    COMPACT_LEN_FLAG,
  );
}

/**
 * Encode the exact current Rust `ContractManifestSignaturePayload` frame.
 *
 * Provenance is deliberately excluded: this is the canonical message signed
 * by `ContractManifest::try_signed` and verified by smart-contract admission.
 *
 * @param {object} manifest
 * @returns {Buffer}
 */
export function noritoEncodeContractManifestSignaturePayload(manifest) {
  const payload = withNoritoCompactLengths(() =>
    encodeContractManifestSignaturePayloadValue(
      manifest,
      "ContractManifestSignaturePayload",
    ),
  );
  return frameNoritoPayload(
    payload,
    CONTRACT_MANIFEST_SIGNATURE_PAYLOAD_SCHEMA_HASH,
    COMPACT_LEN_FLAG,
  );
}

function encodeFeePaymentIntentValue(intent, context) {
  if (!isPlainObject(intent)) {
    rejectType(`${context}${TEXT_MUST_BE_AN_OBJECT}`);
  }
  assertOnlyObjectKeys(intent, ["payer", "value"], context);
  const payer = assertNonEmptyString(intent.payer, `${context}.payer`);
  if (payer !== "authority" && payer !== "sponsor") {
    rejectType(`${context}.payer${TEXT_MUST_BE}authority or sponsor`);
  }
  if (!isPlainObject(intent.value)) {
    rejectType(`${context}.value${TEXT_MUST_BE_AN_OBJECT_2}`);
  }
  const allowedValueFields = ["charge_limits", "gas_limit"];
  if (payer === "sponsor") {
    allowedValueFields.push("program_id", "program_revision");
  }
  assertOnlyObjectKeys(intent.value, allowedValueFields, `${context}.value`);
  if (!Array.isArray(intent.value.charge_limits)) {
    rejectType(`${context}${TEXT_VALUE_CHARGE_LIMITS_MUST}be an array`);
  }
  let previousKind = -1;
  const chargeLimits = encodeNoritoVec(
    Array.from(intent.value.charge_limits, (limit, index) => {
      const itemContext = `${context}.value.charge_limits[${index}]`;
      if (!Object.prototype.hasOwnProperty.call(intent.value.charge_limits, index)) {
        rejectType(`${context}${TEXT_VALUE_CHARGE_LIMITS_MUST}not contain holes`);
      }
      if (!isPlainObject(limit)) {
        rejectType(`${itemContext}${TEXT_MUST_BE_AN_OBJECT}`);
      }
      assertOnlyObjectKeys(
        limit,
        ["kind", WIRE_FIELD_ASSET_DEFINITION_ID, "max_amount"],
        itemContext,
      );
      if (!isPlainObject(limit.kind)) {
        rejectType(`${itemContext}.kind${TEXT_MUST_BE}a tagged unit object`);
      }
      assertOnlyObjectKeys(limit.kind, ["kind", "value"], `${itemContext}.kind`);
      const kind = assertNonEmptyString(limit.kind.kind, `${itemContext}.kind.kind`);
      const kindTag = kind === "nexus" ? 0 : kind === "pipeline_gas" ? 1 : -1;
      if (kindTag < 0 || limit.kind.value !== null) {
        rejectType(`${itemContext}.kind must be the${TEXT_CANONICAL}nexus or pipeline_gas tagged unit`);
      }
      if (kindTag <= previousKind) {
        rejectType(`${context}${TEXT_VALUE_CHARGE_LIMITS_MUST}be unique and ordered nexus before pipeline_gas`);
      }
      previousKind = kindTag;
      const quantity = NumericV1.decodeQuantityJson(limit.max_amount);
      if (quantity.mantissa <= 0n) {
        rejectType(`${itemContext}.max_amount${TEXT_MUST_BE_GREATER_THAN_ZERO}`);
      }
      return encodeStructValue([
        [encodeEnumTagValue(kindTag)],
        [
          encodeAssetDefinitionIdValue(
            limit.asset_definition_id,
            `${itemContext}.${TEXT_ASSET_DEFINITION_ID}`,
          ),
        ],
        [encodeQuantityValue(limit.max_amount, `${itemContext}.max_amount`)],
      ]);
    }),
    (encoded) => encoded,
  );
  const gasLimit = encodeOptionValue(
    intent.value.gas_limit ?? null,
    encodeU64NumberValue,
    `${context}.value.gas_limit`,
  );
  if (intent.value.gas_limit !== undefined && intent.value.gas_limit !== null) {
    const normalizedGas = normalizeU64Input(
      intent.value.gas_limit,
      `${context}.value.gas_limit`,
    );
    if (normalizedGas === 0n) {
      rejectType(`${context}.value.gas_limit${TEXT_MUST_BE}non-zero`);
    }
  }
  if (payer === "authority") {
    return encodeEnumTagValue(0, () =>
      encodeStructValue([[chargeLimits], [gasLimit]]),
    );
  }
  if (!isPlainObject(intent.value.program_id)) {
    rejectType(`${context}${TEXT_VALUE_PROGRAM}id${TEXT_MUST_BE_AN_OBJECT_2}`);
  }
  assertOnlyObjectKeys(
    intent.value.program_id,
    ["sponsor", "name"],
    `${context}${TEXT_VALUE_PROGRAM}id`,
  );
  const name = assertNonEmptyString(
    intent.value.program_id.name,
    `${context}${TEXT_VALUE_PROGRAM}id.name`,
  );
  if (
    name !== intent.value.program_id.name ||
    name.normalize("NFC") !== name ||
    /[\s@#$\/]/u.test(name)
  ) {
    rejectType(`${context}${TEXT_VALUE_PROGRAM}id.name must be a${TEXT_CANONICAL}Iroha Name`);
  }
  const revision = normalizeU64Input(
    intent.value.program_revision,
    `${context}${TEXT_VALUE_PROGRAM}revision`,
  );
  if (revision === 0n) {
    rejectType(`${context}${TEXT_VALUE_PROGRAM}revision${TEXT_MUST_BE}non-zero`);
  }
  const programId = encodeStructValue([
    [
      encodeAccountIdValue(
        intent.value.program_id.sponsor,
        `${context}${TEXT_VALUE_PROGRAM}id.sponsor`,
      ),
    ],
    [encodeNoritoStringValue(name)],
  ]);
  return encodeEnumTagValue(1, () =>
    encodeStructValue([
      [programId],
      [encodeU64Value(revision, `${context}${TEXT_VALUE_PROGRAM}revision`)],
      [chargeLimits],
      [gasLimit],
    ]),
  );
}

/** @internal Encode one exact compact-length `FeePaymentIntent` archive. */
export function noritoEncodeFeePaymentIntentArchive(intent) {
  return withNoritoCompactLengths(() =>
    Uint8Array.from(
      encodeFeePaymentIntentValue(intent, "FeePaymentIntent"),
    ),
  );
}

const INLINE_PRIVATE_KEY_FIELDS = new Set([
  "private_key",
  "privateKey",
  "private_key_hex",
  "privateKeyHex",
  "private_key_bytes",
  "privateKeyBytes",
  "private_key_multihash",
  "privateKeyMultihash",
  "private_key_algorithm",
  "privateKeyAlgorithm",
]);

function rejectInlinePrivateKeyFields(request, context) {
  const fields = Object.keys(request).filter((key) =>
    INLINE_PRIVATE_KEY_FIELDS.has(key),
  );
  if (fields.length !== 0) {
    rejectType(`${context} does not accept private-key fields (${fields.join(", ")}); sign the returned transaction draft locally`);
  }
}

/**
 * Encode a `/v1/multisig/propose` request DTO as a native Norito body.
 *
 * Torii's `NoritoJson<MultisigProposeDto>` extractor accepts this payload with
 * `Content-Type: application/x-norito`. The `instructions` entries are normal
 * InstructionBox values embedded in the DTO, not base64 strings inside JSON.
 *
 * @param {object} request
 * @returns {Buffer}
 */
function encodeMultisigProposeRequest(request, nativeRuntime) {
  if (!isPlainObject(request)) {
    rejectType(("MultisigProposeDto request" + TEXT_MUST_BE_AN_OBJECT_2));
  }
  if (!Array.isArray(request.instructions)) {
    rejectType((TEXT_MULTISIG_PROPOSE_DTO + "instructions" + TEXT_MUST_BE + "an array"));
  }
  rejectInlinePrivateKeyFields(request, "MultisigProposeDto");
  const validationFeeMetadata = normalizeMultisigProposeValidationFeeMetadata(request);
  const payload = withNoritoCompactLengths(() =>
    encodeStructValue([
      ...encodeMultisigAccountSelectorFields(request, (TEXT_MULTISIG_PROPOSE_DTO + "selector")),
      [
        encodeAccountIdValue(
          request.signer_account_id ?? request.signerAccountId,
          (TEXT_MULTISIG_PROPOSE_DTO + "signer_" + TEXT_ACCOUNT_ID),
        ),
      ],
      [
        encodeOptionValue(
          request.public_key_hex ?? request.publicKeyHex ?? null,
          encodeNoritoStringValue,
          (TEXT_MULTISIG_PROPOSE_DTO + "public_key_hex"),
        ),
      ],
      [
        encodeOptionValue(
          request.signature_b64 ?? request.signatureB64 ?? null,
          encodeExactBase64StringValue,
          (TEXT_MULTISIG_PROPOSE_DTO + "signature_b64"),
        ),
      ],
      [
        encodeOptionValue(
          request.creation_time_ms ?? request.creationTimeMs ?? null,
          encodeU64NumberValue,
          (TEXT_MULTISIG_PROPOSE_DTO + TEXT_CREATION_TIME_MS),
        ),
      ],
      [
        encodeFeePaymentIntentValue(
          request.fee_payment ?? request.feePayment,
          (TEXT_MULTISIG_PROPOSE_DTO + "fee_payment"),
        ),
      ],
      [
        encodeOptionValue(
          request.memo ?? null,
          encodeNoritoStringValue,
          (TEXT_MULTISIG_PROPOSE_DTO + "memo"),
        ),
      ],
      [
        encodeOptionValue(
          validationFeeMetadata.policyVersion,
          encodeNoritoStringValue,
          `${TEXT_MULTISIG_PROPOSE_DTO_VALIDATION_FEE}policy_version`,
        ),
      ],
      [
        encodeOptionValue(
          validationFeeMetadata.policyHash,
          encodeNoritoStringValue,
          `${TEXT_MULTISIG_PROPOSE_DTO_VALIDATION_FEE}policy_hash`,
        ),
      ],
      [
        encodeOptionValue(
          validationFeeMetadata.hijiriFeeQuoteHash,
          encodeNoritoStringValue,
          `${TEXT_MULTISIG_PROPOSE_DTO_VALIDATION_FEE}hijiri_fee_quote_hash`,
        ),
      ],
      [
        encodeNoritoVec(request.instructions, (instruction, index) =>
          encodeEmbeddedInstructionBox(
            instruction,
            `${TEXT_MULTISIG_PROPOSE_DTO}instructions[${index}]`,
            nativeRuntime,
          ),
        ),
      ],
      [
        encodeOptionValue(
          validationFeeMetadata.instructionIndex,
          encodeNoritoStringValue,
          `${TEXT_MULTISIG_PROPOSE_DTO_VALIDATION_FEE}instruction_index`,
        ),
      ],
      [
        encodeOptionValue(
          validationFeeMetadata.transferEntryIndex,
          encodeNoritoStringValue,
          `${TEXT_MULTISIG_PROPOSE_DTO_VALIDATION_FEE}transfer_entry_index`,
        ),
      ],
    ]),
  );
  return frameNoritoPayload(payload, MULTISIG_PROPOSE_DTO_SCHEMA_HASH, COMPACT_LEN_FLAG);
}

export function noritoEncodeMultisigProposeRequest(request) {
  return encodeMultisigProposeRequest(request, defaultNativeRuntime);
}

function normalizeMultisigProposeValidationFeeMetadata(request) {
  rejectValidationFeeCamelCaseDtoFields(request);
  const policyVersion = request.validation_fee_policy_version ?? null;
  const policyHash = request.validation_fee_policy_hash ?? null;
  const hijiriFeeQuoteHash = request.validation_fee_hijiri_fee_quote_hash ?? null;
  const instructionIndex = request.validation_fee_instruction_index ?? null;
  const transferEntryIndex = request.validation_fee_transfer_entry_index ?? null;
  const hasPolicyVersion = policyVersion !== null && policyVersion !== undefined;
  const hasPolicyHash = policyHash !== null && policyHash !== undefined;
  const hasHijiriFeeQuoteHash =
    hijiriFeeQuoteHash !== null && hijiriFeeQuoteHash !== undefined;
  const hasInstructionIndex = instructionIndex !== null && instructionIndex !== undefined;
  const hasTransferEntryIndex = transferEntryIndex !== null && transferEntryIndex !== undefined;
  if (hasPolicyVersion !== hasPolicyHash) {
    rejectType(`${TEXT_MULTISIG_PROPOSE_DTO_VALIDATION_FEE}policy_version and validation_fee_policy_hash${TEXT_MUST_BE}provided together`);
  }
  if (!hasPolicyVersion && hasHijiriFeeQuoteHash) {
    rejectType(`${TEXT_MULTISIG_PROPOSE_DTO_VALIDATION_FEE}hijiri_fee_quote_hash${TEXT_REQUIRES_VALIDATION_FEE_POLICY_METADATA}`);
  }
  if (!hasPolicyVersion && hasInstructionIndex) {
    rejectType(`${TEXT_MULTISIG_PROPOSE_DTO_VALIDATION_FEE}instruction_index${TEXT_REQUIRES_VALIDATION_FEE_POLICY_METADATA}`);
  }
  if (!hasPolicyVersion && hasTransferEntryIndex) {
    rejectType(`${TEXT_MULTISIG_PROPOSE_DTO_VALIDATION_FEE}transfer_entry_index${TEXT_REQUIRES_VALIDATION_FEE_POLICY_METADATA}`);
  }
  if (hasTransferEntryIndex && !hasInstructionIndex) {
    rejectType(`${TEXT_MULTISIG_PROPOSE_DTO_VALIDATION_FEE}transfer_entry_index requires ${TEXT_VALIDATION_FEE}instruction_index`);
  }
  if (!hasPolicyVersion) {
    return {
      policyVersion: null,
      policyHash: null,
      hijiriFeeQuoteHash: null,
      instructionIndex: null,
      transferEntryIndex: null,
    };
  }
  return {
    policyVersion: normalizeU64Input(
      policyVersion,
      `${TEXT_MULTISIG_PROPOSE_DTO_VALIDATION_FEE}policy_version`,
    ).toString(),
    policyHash: normalizeValidationFeePolicyHashString(
      policyHash,
      `${TEXT_MULTISIG_PROPOSE_DTO_VALIDATION_FEE}policy_hash`,
    ),
    hijiriFeeQuoteHash: hasHijiriFeeQuoteHash
      ? normalizeValidationFeePolicyHashString(
          hijiriFeeQuoteHash,
          `${TEXT_MULTISIG_PROPOSE_DTO_VALIDATION_FEE}hijiri_fee_quote_hash`,
        )
      : null,
    instructionIndex: hasInstructionIndex
      ? normalizeU64Input(
          instructionIndex,
          `${TEXT_MULTISIG_PROPOSE_DTO_VALIDATION_FEE}instruction_index`,
        ).toString()
      : null,
    transferEntryIndex: hasTransferEntryIndex
      ? normalizeU64Input(
          transferEntryIndex,
          `${TEXT_MULTISIG_PROPOSE_DTO_VALIDATION_FEE}transfer_entry_index`,
        ).toString()
      : null,
  };
}

function rejectValidationFeeCamelCaseDtoFields(request) {
  for (const [camelName, snakeName] of [
    ["validationFeePolicyVersion", (TEXT_VALIDATION_FEE + "policy_version")],
    ["validationFeePolicyHash", (TEXT_VALIDATION_FEE + "policy_hash")],
    ["validationFeeHijiriFeeQuoteHash", (TEXT_VALIDATION_FEE + "hijiri_fee_quote_hash")],
    ["validationFeeInstructionIndex", (TEXT_VALIDATION_FEE + "instruction_index")],
    ["validationFeeTransferEntryIndex", (TEXT_VALIDATION_FEE + "transfer_entry_index")],
  ]) {
    if (Object.prototype.hasOwnProperty.call(request, camelName)) {
      rejectType(`MultisigProposeDto uses ${TEXT_UNSUPPORTED}camelCase validation fee field ${camelName}; use ${snakeName}`);
    }
  }
}

function normalizeValidationFeePolicyHashString(value, context) {
  if (typeof value !== JS_TYPE_STRING) {
    rejectType(`${context}${TEXT_MUST_BE_A}32-byte hex string`);
  }
  const trimmed = value.trim().toLowerCase();
  const normalized = trimmed.startsWith("0x") ? trimmed.slice(2) : trimmed;
  if (!/^[0-9a-f]{64}$/.test(normalized)) {
    rejectType(`${context}${TEXT_MUST_BE_A}32-byte hex string`);
  }
  return normalized;
}

/**
 * Encode a `/v1/contracts/call/multisig/propose` request DTO as a native Norito body.
 *
 * Torii's `NoritoJson<MultisigContractCallProposeDto>` extractor accepts this
 * payload with `Content-Type: application/x-norito`.
 *
 * @param {object} request
 * @returns {Buffer}
 */
export function noritoEncodeMultisigContractCallProposeRequest(request) {
  if (!isPlainObject(request)) {
    rejectType(("MultisigContractCallProposeDto request" + TEXT_MUST_BE_AN_OBJECT_2));
  }
  rejectInlinePrivateKeyFields(request, "MultisigContractCallProposeDto");
  const contractAddress = request.contract_address ?? request.contractAddress ?? null;
  const contractAlias = request.contract_alias ?? request.contractAlias ?? null;
  if ((contractAddress == null) === (contractAlias == null)) {
    rejectType("MultisigContractCallProposeDto requires exactly one of contract_address or contract_alias");
  }
  const payloadValue = request.payload ?? request.contractPayload ?? null;
  const payload = withNoritoCompactLengths(() =>
    encodeStructValue([
      ...encodeMultisigAccountSelectorFields(
        request,
        (TEXT_MULTISIG_CONTRACT_CALL_PROPOSE_DTO + "selector"),
      ),
      [
        encodeAccountIdValue(
          request.signer_account_id ?? request.signerAccountId,
          (TEXT_MULTISIG_CONTRACT_CALL_PROPOSE_DTO + "signer_" + TEXT_ACCOUNT_ID),
        ),
      ],
      [
        encodeOptionValue(
          request.public_key_hex ?? request.publicKeyHex ?? null,
          encodeNoritoStringValue,
          (TEXT_MULTISIG_CONTRACT_CALL_PROPOSE_DTO + "public_key_hex"),
        ),
      ],
      [
        encodeOptionValue(
          request.signature_b64 ?? request.signatureB64 ?? null,
          encodeExactBase64StringValue,
          (TEXT_MULTISIG_CONTRACT_CALL_PROPOSE_DTO + "signature_b64"),
        ),
      ],
      [
        encodeOptionValue(
          request.creation_time_ms ?? request.creationTimeMs ?? null,
          encodeU64NumberValue,
          (TEXT_MULTISIG_CONTRACT_CALL_PROPOSE_DTO + TEXT_CREATION_TIME_MS),
        ),
      ],
      [
        encodeOptionValue(
          contractAddress,
          encodeNoritoStringValue,
          (TEXT_MULTISIG_CONTRACT_CALL_PROPOSE_DTO + TEXT_CONTRACT_ADDRESS_2),
        ),
      ],
      [
        encodeOptionValue(
          contractAlias,
          encodeNoritoStringValue,
          (TEXT_MULTISIG_CONTRACT_CALL_PROPOSE_DTO + "contract_alias"),
        ),
      ],
      [
        encodeNoritoStringValue(
          assertNonEmptyString(
            request.entrypoint,
            (TEXT_MULTISIG_CONTRACT_CALL_PROPOSE_DTO + "entrypoint"),
          ),
        ),
      ],
      [
        encodeOptionValue(
          payloadValue,
          encodeNoritoJsonValue,
          (TEXT_MULTISIG_CONTRACT_CALL_PROPOSE_DTO + "payload"),
        ),
      ],
      [
        encodeFeePaymentIntentValue(
          request.fee_payment ?? request.feePayment,
          (TEXT_MULTISIG_CONTRACT_CALL_PROPOSE_DTO + "fee_payment"),
        ),
      ],
    ]),
  );
  return frameNoritoPayload(
    payload,
    MULTISIG_CONTRACT_CALL_PROPOSE_DTO_SCHEMA_HASH,
    COMPACT_LEN_FLAG,
  );
}

/**
 * Encode a `/v1/contracts/call/multisig/approve` request DTO as a native Norito body.
 *
 * @param {object} request
 * @returns {Buffer}
 */
export function noritoEncodeMultisigContractCallApproveRequest(request) {
  if (!isPlainObject(request)) {
    rejectType(("MultisigContractCallApproveDto request" + TEXT_MUST_BE_AN_OBJECT_2));
  }
  rejectInlinePrivateKeyFields(request, "MultisigContractCallApproveDto");
  const proposalId = request.proposal_id ?? request.proposalId ?? null;
  const instructionsHash = request.instructions_hash ?? request.instructionsHash ?? null;
  if (proposalId == null && instructionsHash == null) {
    rejectType("MultisigContractCallApproveDto requires proposal_id or instructions_hash");
  }
  const payload = withNoritoCompactLengths(() =>
    encodeStructValue([
      ...encodeMultisigAccountSelectorFields(
        request,
        (TEXT_MULTISIG_CONTRACT_CALL_APPROVE_DTO + "selector"),
      ),
      [
        encodeAccountIdValue(
          request.signer_account_id ?? request.signerAccountId,
          (TEXT_MULTISIG_CONTRACT_CALL_APPROVE_DTO + "signer_" + TEXT_ACCOUNT_ID),
        ),
      ],
      [
        encodeOptionValue(
          request.public_key_hex ?? request.publicKeyHex ?? null,
          encodeNoritoStringValue,
          (TEXT_MULTISIG_CONTRACT_CALL_APPROVE_DTO + "public_key_hex"),
        ),
      ],
      [
        encodeOptionValue(
          request.signature_b64 ?? request.signatureB64 ?? null,
          encodeExactBase64StringValue,
          (TEXT_MULTISIG_CONTRACT_CALL_APPROVE_DTO + "signature_b64"),
        ),
      ],
      [
        encodeOptionValue(
          request.creation_time_ms ?? request.creationTimeMs ?? null,
          encodeU64NumberValue,
          (TEXT_MULTISIG_CONTRACT_CALL_APPROVE_DTO + TEXT_CREATION_TIME_MS),
        ),
      ],
      [
        encodeFeePaymentIntentValue(
          request.fee_payment ?? request.feePayment,
          (TEXT_MULTISIG_CONTRACT_CALL_APPROVE_DTO + "fee_payment"),
        ),
      ],
      [
        encodeOptionValue(
          proposalId,
          encodeNoritoStringValue,
          (TEXT_MULTISIG_CONTRACT_CALL_APPROVE_DTO + "proposal_id"),
        ),
      ],
      [
        encodeOptionValue(
          instructionsHash,
          encodeNoritoStringValue,
          (TEXT_MULTISIG_CONTRACT_CALL_APPROVE_DTO + "instructions_hash"),
        ),
      ],
    ]),
  );
  return frameNoritoPayload(
    payload,
    MULTISIG_CONTRACT_CALL_APPROVE_DTO_SCHEMA_HASH,
    COMPACT_LEN_FLAG,
  );
}

function encodeMultisigAccountSelectorFields(request, context) {
  const multisigAccountId = request.multisig_account_id ?? request.multisigAccountId ?? null;
  const multisigAccountAlias =
    request.multisig_account_alias ?? request.multisigAccountAlias ?? null;
  if ((multisigAccountId == null) === (multisigAccountAlias == null)) {
    rejectType(`${context} requires exactly one of multisig_account_id or multisig_account_alias`);
  }
  return [
    [
      encodeOptionValue(
        multisigAccountId,
        encodeAccountIdValue,
        `${context}.multisig_${TEXT_ACCOUNT_ID}`,
      ),
    ],
    [
      encodeOptionValue(
        multisigAccountAlias,
        encodeNoritoStringValue,
        `${context}.multisig_account_alias`,
      ),
    ],
  ];
}

function encodeEmbeddedInstructionBox(
  instruction,
  context,
  nativeRuntime = defaultNativeRuntime,
) {
  const framed = Buffer.from(encodeInstruction(instruction, nativeRuntime));
  const { wireId, payload, innerFlags, innerFrame } = decodeInstructionEnvelope(framed);
  const outerFlags = noritoLengthFlags & COMPACT_LEN_FLAG;
  return encodeInstructionBoxPayload(
    wireId,
    payload,
    outerFlags,
    context,
    innerFlags,
    innerFrame,
  );
}

/**
 * Encode one canonical `InstructionBox` archive for inclusion in a compact
 * transaction payload. The public instruction frame is decoded and rebuilt so
 * both its outer schema and its inner instruction schema are verified before
 * the archive crosses the signing boundary.
 */
function encodeInstructionBoxArchive(instruction, nativeRuntime) {
  return withNoritoLengthFlags(COMPACT_LEN_FLAG, () =>
    encodeEmbeddedInstructionBox(instruction, FIELD_INSTRUCTION, nativeRuntime),
  );
}

export function noritoEncodeInstructionBoxArchive(instruction) {
  return encodeInstructionBoxArchive(instruction, defaultNativeRuntime);
}

/**
 * Decode one exact canonical `InstructionBox` archive embedded in a compact
 * transaction payload.
 *
 * Unlike {@link noritoDecodeInstruction}, this accepts the bare archive used
 * inside `TransactionPayload::instructions`, not a framed public instruction.
 * The decoded value is re-encoded and compared byte-for-byte so alternate
 * length encodings, frame flags, padding, or payload layouts are rejected at
 * signing boundaries.
 *
 * @param {ArrayBufferView | ArrayBuffer | Buffer} bytes
 * @returns {unknown}
 */
export function noritoDecodeInstructionBoxArchive(bytes) {
  const archive = toBuffer(bytes);
  const outerFlags = COMPACT_LEN_FLAG;
  const outerReader = new BufferReader(
    archive,
    (TEXT_INSTRUCTION + "archive"),
    outerFlags,
  );
  const wireId = decodeStringValue(
    readNoritoField(outerReader, "wire"),
    (TEXT_INSTRUCTION + "archive.wire"),
    outerFlags,
  );
  const innerField = readNoritoField(outerReader, "inner");
  outerReader.assertEof();

  const innerReader = new BufferReader(
    innerField,
    (TEXT_INSTRUCTION + "archive.inner"),
    0,
  );
  const innerFrame = readNoritoField(innerReader, "frame");
  innerReader.assertEof();
  const inner = decodeNoritoFrame(
    innerFrame,
    (TEXT_INSTRUCTION + "archive.frame"),
    INNER_SCHEMA_HASH_BY_WIRE_ID[wireId] ?? null,
  );
  const decoded = withNoritoLengthFlags(inner.flags, () =>
    decodePureJsInstructionPayload(
      wireId,
      inner.payload,
      inner.flags,
    ),
  );
  const canonical = noritoEncodeInstructionBoxArchive(decoded);
  if (!archive.equals(canonical)) {
    rejectError(("instruction archive is not" + TEXT_CANONICAL + "Norito"));
  }
  return decoded;
}

/**
 * Decode canonical Norito instruction bytes back to JSON.
 *
 * When `options.parseJson !== false`, the result is the parsed JSON payload.
 * Otherwise the raw JSON string returned by the native binding is emitted.
 *
 * @param {ArrayBufferView | ArrayBuffer | Buffer} bytes
 * @param {{ parseJson?: boolean }} [options]
 * @returns {string | unknown}
 */
function decodeInstruction(bytes, options, nativeRuntime) {
  const buffer = toBuffer(bytes);
  try {
    const decoded = decodePureJsInstruction(buffer);
    validateDecodedInstructionProofAttachments(decoded);
    return options.parseJson === false ? JSON.stringify(decoded) : decoded;
  } catch (error) {
    if (!isPureJsUnsupportedInstructionError(error)) {
      throw error;
    }
  }
  let json;
  const native = resolveNative("noritoDecodeInstruction", nativeRuntime);
  try {
    json = native.noritoDecodeInstruction(buffer);
  } catch (error) {
    if (!isAlignmentError(error)) {
      throw error;
    }
    const decoded =
      tryDecodeWithAlignedBuffer(native, buffer) ??
      tryDecodeWithRelocatedStorage(native, buffer);
    if (decoded === null) {
      throw error;
    }
    json = decoded;
  }
  const decoded = JSON.parse(json);
  validateDecodedInstructionProofAttachments(decoded);
  return options.parseJson === false ? json : decoded;
}

export function noritoDecodeInstruction(bytes, options = {}) {
  return decodeInstruction(bytes, options, defaultNativeRuntime);
}

function validateDecodedInstructionProofAttachments(instruction) {
  rejectRetiredGenericZkInstruction(instruction);
  if (!isPlainObject(instruction) || !isPlainObject(instruction.zk)) {
    return;
  }
  for (const [variant, field] of [
    ["SubmitBallot", "ballot_proof"],
    [TEXT_FINALIZE_ELECTION, "tally_proof"],
  ]) {
    const payload = instruction.zk[variant];
    if (!isPlainObject(payload)) {
      continue;
    }
    if (!Object.prototype.hasOwnProperty.call(payload, field)) {
      rejectType(`zk.${variant}.${field}${TEXT_IS_REQUIRED}`);
    }
    normalizeCanonicalProofAttachmentValue(
      payload[field],
      `zk.${variant}.${field}`,
    );
  }
}

/**
 * Decode and fail closed on one first-release subscription trigger action.
 *
 * The native binding verifies the complete encoded action, including its
 * syscall-only IVM program, repeat policy, filter, retry policy, and metadata.
 * Callers must still bind the returned semantic summary to their reviewed
 * account, subscription, trigger id, and charge time.
 *
 * @param {string} encodedAction
 * @returns {object}
 */
function inspectTriggerAction(encodedAction, nativeRuntime) {
  if (
    typeof encodedAction !== JS_TYPE_STRING ||
    encodedAction.length === 0 ||
    encodedAction.trim() !== encodedAction
  ) {
    rejectType(("inspectSubscriptionTriggerAction encodedAction must be a" + TEXT_CANONICAL + "non-empty string"));
  }
  const native = resolveNative(
    "inspectSubscriptionTriggerAction",
    nativeRuntime,
  );
  const payload = native.inspectSubscriptionTriggerAction(encodedAction);
  try {
    return JSON.parse(payload);
  } catch (error) {
    rejectError(`native subscription trigger inspection returned invalid JSON: ${
        error instanceof Error ? error.message : String(error)
      }`);
  }
}

export function inspectSubscriptionTriggerAction(encodedAction) {
  return inspectTriggerAction(encodedAction, defaultNativeRuntime);
}

/** @internal Source-level test facade; intentionally absent from package exports. */
export function _createNoritoInstructionApi(nativeRuntime) {
  return Object.freeze({
    _instructionWireSchemaBindings: () => INSTRUCTION_WIRE_SCHEMA_BINDINGS,
    inspectSubscriptionTriggerAction: (encodedAction) =>
      inspectTriggerAction(encodedAction, nativeRuntime),
    noritoDecodeInstruction: (bytes, options = {}) =>
      decodeInstruction(bytes, options, nativeRuntime),
    noritoEncodeInstruction: (instruction) =>
      encodeInstruction(instruction, nativeRuntime),
    noritoEncodeInstructionBoxArchive: (instruction) =>
      encodeInstructionBoxArchive(instruction, nativeRuntime),
    noritoEncodeMultisigProposeRequest: (request) =>
      encodeMultisigProposeRequest(request, nativeRuntime),
  });
}

function decodeBlockMerkleProofValue(payload, context) {
  const fields = decodeTupleFields(payload, context, ["leaf_index", "audit_path"]);
  return {
    leaf_index: decodeU32Value(fields.leaf_index, `${context}.leaf_index`),
    audit_path: decodeNoritoVec(
      fields.audit_path,
      (entry, index) =>
        decodeOptionValue(
          entry,
          decodeHashValue,
          `${context}.audit_path[${index}]`,
        ),
      `${context}.audit_path`,
    ),
  };
}

function decodeBlockMerkleCommitmentValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["root", "leaf_count"]);
  const leafCount = decodeU64Value(fields.leaf_count, `${context}.leaf_count`);
  if (leafCount === "0") {
    rejectError(`${context}.leaf_count${TEXT_MUST_BE}non-zero`);
  }
  return {
    root: decodeHashValue(fields.root, `${context}.root`),
    leaf_count: leafCount,
  };
}

const BlockReceiptProofValueFields = [
    ["leaf", decodeHashValue, 0],
    ["proof", decodeBlockMerkleProofValue, 0],
  ];

  function decodeBlockReceiptProofValue(payload, context) {
    return decodeRecordFields(payload, context, BlockReceiptProofValueFields);
  }

function decodeTransferSmtWitnessValue(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "root_before",
    "root_after",
    "path_bits",
    "siblings",
  ]);
  return {
    root_before: decodeFixedByteArrayArchiveValue(
      fields.root_before,
      32,
      `${context}.root_before`,
    ).toString(HEX_ENCODING),
    root_after: decodeFixedByteArrayArchiveValue(
      fields.root_after,
      32,
      `${context}.root_after`,
    ).toString(HEX_ENCODING),
    path_bits: decodeNoritoVec(
      fields.path_bits,
      (entry, index) => decodeU8Value(entry, `${context}.path_bits[${index}]`),
      `${context}.path_bits`,
    ),
    siblings: decodeNoritoVec(
      fields.siblings,
      (entry, index) =>
        decodeFixedByteArrayArchiveValue(
          entry,
          32,
          `${context}.siblings[${index}]`,
        ).toString(HEX_ENCODING),
      `${context}.siblings`,
    ),
  };
}

const TransferDeltaTranscriptValueFields = [
    ["from_account", decodeAccountIdValue, 0],
    ["to_account", decodeAccountIdValue, 0],
    ["asset_definition", decodeAssetDefinitionIdValue, 0],
    ["amount", decodeQuantityValue, 0],
    ["from_balance_before", decodeQuantityValue, 0],
    ["from_balance_after", decodeQuantityValue, 0],
    ["to_balance_before", decodeQuantityValue, 0],
    ["to_balance_after", decodeQuantityValue, 0],
    ["from_smt_witness", decodeTransferSmtWitnessValue, 0],
    ["to_smt_witness", decodeTransferSmtWitnessValue, 0],
  ];

  function decodeTransferDeltaTranscriptValue(payload, context) {
    return decodeRecordFields(payload, context, TransferDeltaTranscriptValueFields);
  }

const TransferTranscriptValueFields = [
    ["batch_hash", decodeHashValue, 0],
    ["deltas", decodeTransferDeltaTranscriptValue, 2],
    ["authority_digest", decodeHashValue, 0],
    ["poseidon_preimage_digest", decodeHashValue, 1],
  ];

  function decodeTransferTranscriptValue(payload, context) {
    return decodeRecordFields(payload, context, TransferTranscriptValueFields);
  }

function decodeFastpqTranscriptMap(payload, context) {
  const reader = new BufferReader(payload, context);
  const count = bigintToSafeNumber(reader.readU64LE("count"), `${context}.count`);
  const entries = [];
  let previousKey = null;
  for (let index = 0; index < count; index += 1) {
    const keyPayload = readNoritoField(reader, `key${index}`);
    const valuePayload = readNoritoField(reader, `value${index}`);
    const keyBytes = decodeFixedBytesValue(keyPayload, 32, `${context}.key[${index}]`);
    if (previousKey !== null && Buffer.compare(previousKey, keyBytes) >= 0) {
      rejectError(`${context} keys are not in${TEXT_CANONICAL}strict order`);
    }
    previousKey = keyBytes;
    const key = decodeHashValue(keyPayload, `${context}.key[${index}]`);
    const value = decodeNoritoVec(
      valuePayload,
      (entry, transcriptIndex) =>
        decodeTransferTranscriptValue(
          entry,
          `${context}[${key}][${transcriptIndex}]`,
        ),
      `${context}[${key}]`,
    );
    entries.push([key, value]);
  }
  reader.assertEof();
  return Object.fromEntries(entries);
}

/**
 * Decode the canonical Norito `BlockProofs` response returned by
 * `/v1/ledger/block/{height}/proof/{entry_hash}`.
 *
 * @param {ArrayBufferView | ArrayBuffer | Buffer} bytes
 * @returns {object}
 */
export function noritoDecodeBlockProofs(bytes) {
  const frame = validateNoritoFrame(bytes, {
    context: "BlockProofs",
    expectedTypeName: BLOCK_PROOFS_TYPE_NAME,
    requireNonEmptyPayload: true,
  });
  if ((frame.flags & (NORITO_PACKED_SEQ_FLAG | NORITO_PACKED_STRUCT_FLAG | NORITO_FIELD_BITSET_FLAG)) !== 0) {
    rejectError(("BlockProofs uses an " + TEXT_UNSUPPORTED + "packed Norito layout"));
  }
  return withNoritoLengthFlags(frame.flags & COMPACT_LEN_FLAG, () => {
    const fields = decodeStructFields(frame.payload, "BlockProofs", [
      "block_height",
      "block_hash",
      "executed_block_wire_hash",
      "entry_hash",
      ("entry_" + TEXT_COMMITMENT),
      "entry_proof",
      ("result_" + TEXT_COMMITMENT),
      "result_proof",
      "fastpq_transcripts",
    ]);
    const blockHeight = decodeU64Value(fields.block_height, (TEXT_BLOCK_PROOFS + "block_height"));
    if (blockHeight === "0") {
      rejectError((TEXT_BLOCK_PROOFS + "block_height" + TEXT_MUST_BE + "non-zero"));
    }
    const entryCommitment = decodeBlockMerkleCommitmentValue(
      fields.entry_commitment,
      (TEXT_BLOCK_PROOFS + "entry_" + TEXT_COMMITMENT),
    );
    const resultCommitment = decodeBlockMerkleCommitmentValue(
      fields.result_commitment,
      (TEXT_BLOCK_PROOFS + "result_" + TEXT_COMMITMENT),
    );
    if (entryCommitment.leaf_count !== resultCommitment.leaf_count) {
      rejectError("BlockProofs entry/result commitment leaf counts must match");
    }
    return {
      block_height: blockHeight,
      block_hash: decodeHashValue(fields.block_hash, (TEXT_BLOCK_PROOFS + "block_hash")),
      executed_block_wire_hash: decodeHashValue(
        fields.executed_block_wire_hash,
        (TEXT_BLOCK_PROOFS + "executed_block_wire_hash"),
      ),
      entry_hash: decodeHashValue(fields.entry_hash, (TEXT_BLOCK_PROOFS + "entry_hash")),
      entry_commitment: entryCommitment,
      entry_proof: decodeBlockReceiptProofValue(
        fields.entry_proof,
        (TEXT_BLOCK_PROOFS + "entry_proof"),
      ),
      result_commitment: resultCommitment,
      result_proof: decodeBlockReceiptProofValue(
        fields.result_proof,
        (TEXT_BLOCK_PROOFS + "result_proof"),
      ),
      fastpq_transcripts: decodeFastpqTranscriptMap(
        fields.fastpq_transcripts,
        (TEXT_BLOCK_PROOFS + "fastpq_transcripts"),
      ),
    };
  });
}

const { verifyBlockMerkleProof, verifyBlockProofs } =
  createBlockProofVerification(encodeHashLiteralBytes);
export { verifyBlockMerkleProof, verifyBlockProofs };

/**
 * Encode an `iroha_data_model::zk::OpenVerifyEnvelope` as standalone Norito bytes.
 *
 * @param {object} envelope
 * @returns {Buffer}
 */
export function noritoEncodeOpenVerifyEnvelope(envelope) {
  const payload = encodeOpenVerifyEnvelopePayload(envelope, WIRE_TYPE_OPEN_VERIFY_ENVELOPE);
  return frameNoritoPayload(payload, OPEN_VERIFY_ENVELOPE_SCHEMA_HASH, 0);
}

/**
 * Decode standalone Norito bytes for `iroha_data_model::zk::OpenVerifyEnvelope`.
 *
 * @param {ArrayBufferView | ArrayBuffer | Buffer | string} bytes
 * @returns {object}
 */
export function noritoDecodeOpenVerifyEnvelope(bytes) {
  let buffer;
  if (typeof bytes === JS_TYPE_STRING) {
    const trimmed = bytes.trim();
    if (/^[0-9a-fA-F]+$/.test(trimmed) && trimmed.length % 2 === 0) {
      buffer = Buffer.from(trimmed, HEX_ENCODING);
    } else {
      buffer = Buffer.from(trimmed, BASE64_ENCODING);
    }
  } else {
    buffer = toBuffer(bytes);
  }
  const frame = decodeNoritoFrame(
    buffer,
    WIRE_TYPE_OPEN_VERIFY_ENVELOPE,
    OPEN_VERIFY_ENVELOPE_SCHEMA_HASH,
  );
  return decodeOpenVerifyEnvelopePayload(
    frame.payload,
    WIRE_TYPE_OPEN_VERIFY_ENVELOPE,
    frame.flags,
  );
}

/**
 * Decode the exact canonical-standard-base64 form of the checked Rust Exact12
 * fixture archive without consulting the native binding.
 *
 * @param {string} value
 * @returns {object}
 */
export function noritoDecodePrivacyExact12FixtureBundleBase64V1(value) {
  const maximumBase64Length =
    Math.ceil(PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1 / 3) * 4;
  if (typeof value !== JS_TYPE_STRING || value.length > maximumBase64Length) {
    rejectRange(`${TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1_2}base64${TEXT_EXCEEDS_THE}${PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1}-byte archive limit`);
  }
  const archive = decodeExactStandardBase64(
    value,
    (TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1_2 + "base64"),
  );
  return noritoDecodePrivacyExact12FixtureBundleV1(archive);
}

/**
 * Decode one canonical outer `PrivacyExact12FixtureBundleV1` archive.
 * Every nested byte-complete field remains byte-exact and is returned as a
 * copied `Uint8Array`.
 *
 * @param {ArrayBufferView | ArrayBuffer | Buffer} bytes
 * @returns {object}
 */
export function noritoDecodePrivacyExact12FixtureBundleV1(bytes) {
  const view = toBuffer(bytes);
  if (view.length === 0) {
    rejectType((TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1_2 + "archive" + TEXT_MUST_NOT_BE + "empty"));
  }
  if (view.length > PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1) {
    rejectRange(`${TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1_2}archive exceeds ${PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1} bytes`);
  }
  const archive = Buffer.from(view);
  const frame = validateNoritoFrame(archive, {
    context: WIRE_TYPE_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1,
    expectedSchemaHash: PRIVACY_EXACT12_FIXTURE_BUNDLE_SCHEMA_HASH_V1,
    expectedPaddingLength: 0,
    requireNonEmptyPayload: true,
  });
  if (frame.flags !== COMPACT_LEN_FLAG) {
    rejectError(`${TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1_2}must use${TEXT_CANONICAL}layout flags 0x${COMPACT_LEN_FLAG.toString(16)}`);
  }
  const bundle = withNoritoCompactLengths(() =>
    decodePrivacyExact12FixtureBundlePayloadV1(frame.payload),
  );
  const canonical = encodePrivacyExact12FixtureBundleCanonicalV1(bundle);
  if (!canonical.equals(archive)) {
    rejectError((TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1_2 + "archive is not" + TEXT_CANONICAL + "or contains trailing data"));
  }
  return externalizePrivacyExact12FixtureBundleV1(bundle);
}

/**
 * Encode a fully byte-complete Exact12 bundle using the canonical Rust outer
 * archive layout. Inputs must retain all twelve protocol rows and cross-field
 * bindings.
 *
 * @param {object} value
 * @returns {Uint8Array}
 */
export function noritoEncodePrivacyExact12FixtureBundleV1(value) {
  const bundle = normalizePrivacyExact12FixtureBundleInputV1(value);
  return Uint8Array.from(encodePrivacyExact12FixtureBundleCanonicalV1(bundle));
}

/**
 * Encode the sole first-release confidential memo envelope.
 *
 * The input must use the exact snake-case V1 shape. No version aliases,
 * recipient-count field, empty slots, or legacy X25519 payload are accepted.
 *
 * @param {object} value
 * @returns {Uint8Array}
 */
export function noritoEncodeConfidentialMemoEnvelopeV1(value) {
  return Uint8Array.from(
    confidentialMemoValueCodecs[0](value, "ConfidentialMemoEnvelopeV1"),
  );
}

/**
 * Decode exactly one canonical first-release confidential memo envelope.
 *
 * @param {Uint8Array | Buffer | number[]} bytes
 * @returns {object}
 */
export function noritoDecodeConfidentialMemoEnvelopeV1(bytes) {
  const payload = Buffer.from(
    normalizeFlexibleBytes(bytes, "ConfidentialMemoEnvelopeV1 wire"),
  );
  return confidentialMemoValueCodecs[1](payload, "ConfidentialMemoEnvelopeV1");
}

function decodePrivacyExact12FixtureBundlePayloadV1(payload) {
  const fields = decodeStructFields(payload, WIRE_TYPE_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1, [
    "version",
    "rows",
  ]);
  const version = decodeU32Value(
    fields.version,
    (TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1 + "version"),
  );
  if (version !== 1) {
    rejectRange((TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1 + "version" + TEXT_MUST_BE + "exactly 1"));
  }
  const reader = new BufferReader(
    fields.rows,
    (TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1 + "rows"),
    COMPACT_LEN_FLAG,
  );
  const count = bigintToSafeNumber(
    reader.readU64LE("count"),
    (TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1 + "rows.count"),
  );
  if (count !== PRIVACY_EXACT12_PROTOCOL_IDS_V1.length) {
    rejectRange(`${TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1}rows${TEXT_MUST_CONTAIN}exactly ${PRIVACY_EXACT12_PROTOCOL_IDS_V1.length} rows`);
  }
  const rows = [];
  for (let index = 0; index < count; index += 1) {
    rows.push(
      decodePrivacyExact12FixtureRowV1(
        readNoritoField(reader, `row${index}`),
        index,
      ),
    );
  }
  reader.assertEof();
  return { version, rows };
}

function decodePrivacyExact12FixtureRowV1(payload, rowIndex) {
  const context = `${TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1}rows[${rowIndex}]`;
  const fields = decodeStructFields(
    payload,
    context,
    PRIVACY_EXACT12_ROW_FIELD_NAMES_V1,
  );
  const protocolDiscriminant = decodeU32Value(
    fields.protocol_id,
    `${context}.protocol_id`,
  );
  if (protocolDiscriminant !== rowIndex) {
    const description =
      protocolDiscriminant < PRIVACY_EXACT12_PROTOCOL_IDS_V1.length
        ? `duplicate, substituted, or reordered protocol ${PRIVACY_EXACT12_PROTOCOL_IDS_V1[protocolDiscriminant]}`
        : `unknown protocol discriminant ${protocolDiscriminant}`;
    rejectType(`${context}.protocol_id contains ${description}`);
  }
  const row = {
    protocolId: PRIVACY_EXACT12_PROTOCOL_IDS_V1[rowIndex],
    statementNorito: decodePrivacyExact12NonEmptyByteVectorV1(
      fields.statement_norito,
      `${context}.statement_norito`,
    ),
    envelopeNorito: decodePrivacyExact12NonEmptyByteVectorV1(
      fields.envelope_norito,
      `${context}.envelope_norito`,
    ),
    submitProofWireId: decodeStringValue(
      fields.submit_proof_wire_id,
      `${context}.submit_proof_wire_id`,
    ),
    submitProofInstructionNorito: decodePrivacyExact12NonEmptyByteVectorV1(
      fields.submit_proof_instruction_norito,
      `${context}.${TEXT_SUBMIT_PROOF_INSTRUCTION_NORITO}`,
    ),
    transactionIntentProjectionNorito:
      decodePrivacyExact12NonEmptyByteVectorV1(
        fields.transaction_intent_projection_norito,
        `${context}${TEXT_TRANSACTION_INTENT_PROJECTION_NORITO}`,
      ),
    transactionIntentDigest: decodeFixedBytesValue(
      fields.transaction_intent_digest,
      32,
      `${context}.${TEXT_TRANSACTION_INTENT}digest`,
    ),
    unsignedTransactionPayloadNorito:
      decodePrivacyExact12NonEmptyByteVectorV1(
        fields.unsigned_transaction_payload_norito,
        `${context}${TEXT_UNSIGNED_TRANSACTION_PAYLOAD_NORITO_2}`,
      ),
    signedTransactionVersionedNorito:
      decodePrivacyExact12NonEmptyByteVectorV1(
        fields.signed_transaction_versioned_norito,
        `${context}${TEXT_SIGNED_TRANSACTION}versioned_norito`,
      ),
    signedTransactionHash: decodeFixedBytesValue(
      fields.signed_transaction_hash,
      32,
      `${context}${TEXT_SIGNED_TRANSACTION}hash`,
    ),
  };
  validatePrivacyExact12FixtureRowBindingsV1(row, rowIndex, context);
  return row;
}

function decodePrivacyExact12NonEmptyByteVectorV1(payload, context) {
  const bytes = decodeByteVecValue(
    payload,
    context,
    PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1,
  );
  if (bytes.length === 0) {
    rejectType(`${context}${TEXT_MUST_NOT_BE}empty`);
  }
  return bytes;
}

function validatePrivacyExact12FixtureRowBindingsV1(row, rowIndex, context) {
  return withNoritoCompactLengths(() =>
    validatePrivacyExact12FixtureRowBindingsCompactV1(row, rowIndex, context),
  );
}

function validatePrivacyExact12FixtureRowBindingsCompactV1(
  row,
  rowIndex,
  context,
) {
  if (row.protocolId !== PRIVACY_EXACT12_PROTOCOL_IDS_V1[rowIndex]) {
    rejectType(`${context}.protocolId is unknown, duplicated, or out of order`);
  }
  if (row.submitProofWireId !== PRIVACY_EXACT12_SUBMIT_PROOF_WIRE_ID_V1) {
    rejectType(`${context}${TEXT_SUBMIT_PROOF_WIRE_ID_MUST_BE}exactly ${PRIVACY_EXACT12_SUBMIT_PROOF_WIRE_ID_V1}`);
  }

  const statementFrame = validatePrivacyExact12NestedFrameV1(
    row.statementNorito,
    PRIVACY_EXACT12_STATEMENT_SCHEMA_HASH_V1,
    PRIVACY_EXACT12_ALIGNED_NESTED_FRAME_PADDING_V1,
    `${context}.statementNorito`,
  );
  const statement = decodePrivacyExact12TaggedPayloadV1(
    statementFrame.payload,
    `${context}.statementNorito.payload`,
  );
  if (statement.tag !== rowIndex) {
    rejectType(`${context}.statementNorito${TEXT_CARRIES_A_SUBSTITUTED}protocol`);
  }

  const envelopeFrame = validatePrivacyExact12NestedFrameV1(
    row.envelopeNorito,
    PRIVACY_EXACT12_ENVELOPE_SCHEMA_HASH_V1,
    PRIVACY_EXACT12_ALIGNED_NESTED_FRAME_PADDING_V1,
    `${context}.envelopeNorito`,
  );
  const envelopeFields = withNoritoCompactLengths(() =>
    decodeStructFields(
      envelopeFrame.payload,
      `${context}.envelopeNorito.payload`,
      PRIVACY_EXACT12_ENVELOPE_FIELD_NAMES_V1,
    ),
  );
  assertPrivacyExact12CanonicalStructPayloadV1(
    envelopeFrame.payload,
    envelopeFields,
    PRIVACY_EXACT12_ENVELOPE_FIELD_NAMES_V1,
    `${context}.envelopeNorito.payload`,
  );
  if (!envelopeFields.wire_magic.equals(PRIVACY_EXACT12_WIRE_MAGIC_V1)) {
    rejectType(`${context}.envelopeNorito carries an invalid final V1 wire marker`);
  }
  if (!envelopeFields.catalog_commitment.equals(PRIVACY_EXACT12_CATALOG_COMMITMENT_V1)) {
    rejectType(`${context}${TEXT_ENVELOPE_NORITO_CARRIES_A_SUBSTITUTED}Exact12 catalog ${TEXT_COMMITMENT}`);
  }
  for (const field of ["proof_system_id", "engine_id"]) {
    if (decodeU32Value(envelopeFields[field], `${context}.${field}`) !==
        PRIVACY_EXACT12_PROOF_ENGINE_TAGS_V1[rowIndex]) {
      rejectType(`${context}${TEXT_ENVELOPE_NORITO_CARRIES_A_SUBSTITUTED}${field}`);
    }
  }
  if (
    decodeU32Value(
      envelopeFields.protocol_id,
      `${context}.envelopeNorito.protocol_id`,
    ) !== rowIndex
  ) {
    rejectType(`${context}${TEXT_ENVELOPE_NORITO_CARRIES_A_SUBSTITUTED}protocol`);
  }
  if (!envelopeFields.statement.equals(statementFrame.payload)) {
    rejectType(`${context}.envelopeNorito does not contain statementNorito`);
  }
  const proof = decodePrivacyExact12TaggedPayloadV1(
    envelopeFields.proof,
    `${context}.envelopeNorito.proof`,
  );
  if (proof.tag !== rowIndex) {
    rejectType(`${context}.envelopeNorito proof${TEXT_CARRIES_A_SUBSTITUTED}protocol`);
  }

  const instructionFrame = validatePrivacyExact12NestedFrameV1(
    row.submitProofInstructionNorito,
    PRIVACY_EXACT12_SUBMIT_PROOF_SCHEMA_HASH_V1,
    PRIVACY_EXACT12_ALIGNED_NESTED_FRAME_PADDING_V1,
    `${context}.${TEXT_SUBMIT_PROOF_INSTRUCTION_NORITO_2}`,
  );
  const instructionFields = withNoritoCompactLengths(() =>
    decodeStructFields(
      instructionFrame.payload,
      `${context}${TEXT_SUBMIT_PROOF_INSTRUCTION_NORITO_PAYLOAD}`,
      ["envelope"],
    ),
  );
  assertPrivacyExact12CanonicalStructPayloadV1(
    instructionFrame.payload,
    instructionFields,
    ["envelope"],
    `${context}${TEXT_SUBMIT_PROOF_INSTRUCTION_NORITO_PAYLOAD}`,
  );
  if (!instructionFields.envelope.equals(envelopeFrame.payload)) {
    rejectType(`${context}.submitProofInstructionNorito does not contain envelopeNorito`);
  }

  const projectionFrame = validatePrivacyExact12NestedFrameV1(
    row.transactionIntentProjectionNorito,
    PRIVACY_EXACT12_TRANSACTION_PAYLOAD_SCHEMA_HASH_V1,
    PRIVACY_EXACT12_TRANSACTION_PAYLOAD_FRAME_PADDING_V1,
    `${context}.${TEXT_TRANSACTION_INTENT_PROJECTION_NORITO_2}`,
  );
  const projectionFields = decodePrivacyExact12TransactionPayloadV1(
    projectionFrame.payload,
    `${context}.transactionIntentProjectionNorito.payload`,
  );
  const unsignedFields = decodePrivacyExact12TransactionPayloadV1(
    row.unsignedTransactionPayloadNorito,
    `${context}.${TEXT_UNSIGNED_TRANSACTION_PAYLOAD_NORITO_3}`,
  );
  validatePrivacyExact12NetworkBindingsV1({
    statementTag: statement.tag,
    statementContent: statement.content,
    projectionDomain: projectionFields.domain,
    unsignedDomain: unsignedFields.domain,
    context,
  });
  for (const field of PRIVACY_EXACT12_TRANSACTION_PAYLOAD_FIELD_NAMES_V1) {
    if (field !== "instructions" && !projectionFields[field].equals(unsignedFields[field])) {
      rejectType(`${context}.transaction intent projection changed independent field ${field}`);
    }
  }
  if (
    unsignedFields.admission_intent.length !== 4 ||
    unsignedFields.admission_intent.readUInt32LE(0) !== 0
  ) {
    rejectType(`${context}${TEXT_UNSIGNED_TRANSACTION_PAYLOAD_NORITO}admission_intent${TEXT_MUST_BE}TransactionAdmissionIntent::Ordinary`);
  }
  const expectedCreationTime = 1_700_000_000_000n + BigInt(rowIndex);
  if (
    decodeU64Value(
      unsignedFields.creation_time_ms,
      `${context}${TEXT_UNSIGNED_TRANSACTION_PAYLOAD_NORITO}${TEXT_CREATION_TIME_MS}`,
    ) !== expectedCreationTime.toString()
  ) {
    rejectType(`${context}${TEXT_CARRIES_A_SUBSTITUTED}transaction creation time`);
  }
  const nonce = decodeOptionValue(
    unsignedFields.nonce,
    decodeU32Value,
    `${context}${TEXT_UNSIGNED_TRANSACTION_PAYLOAD_NORITO}nonce`,
  );
  if (nonce !== rowIndex + 1) {
    rejectType(`${context}${TEXT_CARRIES_A_SUBSTITUTED}transaction nonce`);
  }
  const attachments = decodeOptionValue(
    unsignedFields.attachments,
    (payload) => payload,
    `${context}${TEXT_UNSIGNED_TRANSACTION_PAYLOAD_NORITO}attachments`,
  );
  if (attachments !== null) {
    rejectType(`${context} must not carry transaction attachments`);
  }

  const instructionOffset = row.unsignedTransactionPayloadNorito.indexOf(
    row.submitProofInstructionNorito,
  );
  if (instructionOffset < 0) {
    rejectType(`${context}${TEXT_UNSIGNED_TRANSACTION_PAYLOAD_NORITO_DOES_NOT_CONTAIN_THE}byte-complete instruction`);
  }
  if (
    row.unsignedTransactionPayloadNorito.indexOf(
      Buffer.from(PRIVACY_EXACT12_SUBMIT_PROOF_WIRE_ID_V1, UTF8_ENCODING),
    ) < 0
  ) {
    rejectType(`${context}${TEXT_UNSIGNED_TRANSACTION_PAYLOAD_NORITO_DOES_NOT_CONTAIN_THE}exact submission wire id`);
  }

  const expectedIntentDigest = Buffer.from(
    blake3(
      Buffer.concat([
        PRIVACY_EXACT12_INTENT_DIGEST_DOMAIN_V1,
        u64ToLittleEndianBuffer(row.transactionIntentProjectionNorito.length),
        row.transactionIntentProjectionNorito,
      ]),
    ),
  );
  if (!expectedIntentDigest.equals(row.transactionIntentDigest)) {
    rejectType(`${context}.transactionIntentDigest does not match its projection`);
  }

  validatePrivacyExact12SignedTransactionV1(row, context);

  const transactionHashPreimage = Buffer.concat([
    u32ToLittleEndianBuffer(0),
    encodeCompactLength(row.unsignedTransactionPayloadNorito.length),
    row.unsignedTransactionPayloadNorito,
  ]);
  const expectedTransactionHash = Buffer.from(blake2b256(transactionHashPreimage));
  expectedTransactionHash[31] |= 1;
  if (!expectedTransactionHash.equals(row.signedTransactionHash)) {
    rejectType(`${context}.signedTransactionHash does not match the unsigned transaction intent`);
  }
}

function validatePrivacyExact12NestedFrameV1(
  bytes,
  schemaHash,
  expectedPaddingLength,
  context,
) {
  const frame = validateNoritoFrame(bytes, {
    context,
    expectedSchemaHash: schemaHash,
    expectedPaddingLength,
    requireNonEmptyPayload: true,
  });
  if (frame.flags !== COMPACT_LEN_FLAG) {
    rejectError(`${context} must use${TEXT_CANONICAL}compact-length layout flags`);
  }
  const canonical = frameNoritoPayload(
    frame.payload,
    schemaHash,
    COMPACT_LEN_FLAG,
    expectedPaddingLength,
  );
  if (!canonical.equals(bytes)) {
    rejectError(`${context} is not a${TEXT_CANONICAL}uncompressed Norito frame`);
  }
  return frame;
}

function decodePrivacyExact12TaggedPayloadV1(payload, context) {
  const reader = new BufferReader(payload, context, COMPACT_LEN_FLAG);
  const tag = reader.readU32LE("tag");
  const content = readNoritoField(reader, "content");
  reader.assertEof();
  const canonical = Buffer.concat([
    u32ToLittleEndianBuffer(tag),
    encodeCompactLength(content.length),
    content,
  ]);
  if (!canonical.equals(payload)) {
    rejectError(`${context} is not a${TEXT_CANONICAL}tagged payload`);
  }
  return { tag, content };
}

function decodePrivacyExact12TransactionPayloadV1(payload, context) {
  const fields = withNoritoCompactLengths(() =>
    decodeStructFields(
      payload,
      context,
      PRIVACY_EXACT12_TRANSACTION_PAYLOAD_FIELD_NAMES_V1,
    ),
  );
  assertPrivacyExact12CanonicalStructPayloadV1(
    payload,
    fields,
    PRIVACY_EXACT12_TRANSACTION_PAYLOAD_FIELD_NAMES_V1,
    context,
  );
  return fields;
}

function assertPrivacyExact12CanonicalStructPayloadV1(
  payload,
  fields,
  fieldNames,
  context,
) {
  const canonical = withNoritoCompactLengths(() =>
    encodeStructValue(fieldNames.map((field) => [fields[field]])),
  );
  if (!canonical.equals(payload)) {
    rejectError(`${context} contains a non-canonical field layout`);
  }
}

function validatePrivacyExact12SignedTransactionV1(row, context) {
  const signed = row.signedTransactionVersionedNorito;
  if (signed[0] !== 1) {
    rejectType(`${context}${TEXT_SIGNED_TRANSACTION_VERSIONED_NORITO}must use version 1`);
  }
  const payload = signed.subarray(1);
  const fields = withNoritoCompactLengths(() =>
    decodeStructFields(
      payload,
      `${context}${TEXT_SIGNED_TRANSACTION_VERSIONED_NORITO_2}payload`,
      ["signature", "payload", "multisig_signatures"],
    ),
  );
  assertPrivacyExact12CanonicalStructPayloadV1(
    payload,
    fields,
    ["signature", "payload", "multisig_signatures"],
    `${context}${TEXT_SIGNED_TRANSACTION_VERSIONED_NORITO_2}payload`,
  );
  if (fields.signature.length === 0) {
    rejectType(`${context}${TEXT_SIGNED_TRANSACTION_VERSIONED_NORITO}has no signature`);
  }
  if (!fields.payload.equals(row.unsignedTransactionPayloadNorito)) {
    rejectType(`${context}${TEXT_SIGNED_TRANSACTION_VERSIONED_NORITO}does not contain the unsigned payload`);
  }
  const multisig = decodeOptionValue(
    fields.multisig_signatures,
    (entry) => entry,
    `${context}${TEXT_SIGNED_TRANSACTION_VERSIONED_NORITO_2}multisig_signatures`,
  );
  if (multisig !== null) {
    rejectType(`${context}${TEXT_SIGNED_TRANSACTION_VERSIONED_NORITO}must not carry multisig signatures`);
  }
}

function normalizePrivacyExact12FixtureBundleInputV1(value) {
  assertExactObjectKeys(value, ["version", "rows"], WIRE_TYPE_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1);
  if (value.version !== 1) {
    rejectType((TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1 + "version" + TEXT_MUST_BE + "exactly 1"));
  }
  if (
    !Array.isArray(value.rows) ||
    value.rows.length !== PRIVACY_EXACT12_PROTOCOL_IDS_V1.length
  ) {
    rejectType(`${TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1}rows${TEXT_MUST_CONTAIN}exactly ${PRIVACY_EXACT12_PROTOCOL_IDS_V1.length} rows`);
  }
  preflightPrivacyExact12FixtureBundleInputV1(value.rows);
  const rows = value.rows.map((row, rowIndex) => {
    const context = `${TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1}rows[${rowIndex}]`;
    const normalized = {
      protocolId: row.protocolId,
      statementNorito: normalizePrivacyExact12InputBytesV1(
        row.statementNorito,
        `${context}.statementNorito`,
      ),
      envelopeNorito: normalizePrivacyExact12InputBytesV1(
        row.envelopeNorito,
        `${context}.envelopeNorito`,
      ),
      submitProofWireId: row.submitProofWireId,
      submitProofInstructionNorito: normalizePrivacyExact12InputBytesV1(
        row.submitProofInstructionNorito,
        `${context}.${TEXT_SUBMIT_PROOF_INSTRUCTION_NORITO_2}`,
      ),
      transactionIntentProjectionNorito: normalizePrivacyExact12InputBytesV1(
        row.transactionIntentProjectionNorito,
        `${context}.${TEXT_TRANSACTION_INTENT_PROJECTION_NORITO_2}`,
      ),
      transactionIntentDigest: normalizePrivacyExact12InputBytesV1(
        row.transactionIntentDigest,
        `${context}.transactionIntentDigest`,
        32,
      ),
      unsignedTransactionPayloadNorito: normalizePrivacyExact12InputBytesV1(
        row.unsignedTransactionPayloadNorito,
        `${context}.${TEXT_UNSIGNED_TRANSACTION_PAYLOAD_NORITO_3}`,
      ),
      signedTransactionVersionedNorito: normalizePrivacyExact12InputBytesV1(
        row.signedTransactionVersionedNorito,
        `${context}.signedTransactionVersionedNorito`,
      ),
      signedTransactionHash: normalizePrivacyExact12InputBytesV1(
        row.signedTransactionHash,
        `${context}.signedTransactionHash`,
        32,
      ),
    };
    if (typeof normalized.submitProofWireId !== JS_TYPE_STRING) {
      rejectType(`${context}${TEXT_SUBMIT_PROOF_WIRE_ID_MUST_BE}a string`);
    }
    validatePrivacyExact12FixtureRowBindingsV1(normalized, rowIndex, context);
    return normalized;
  });
  return { version: 1, rows };
}

function preflightPrivacyExact12FixtureBundleInputV1(rows) {
  let declaredBytes = 0;
  for (let rowIndex = 0; rowIndex < rows.length; rowIndex += 1) {
    const row = rows[rowIndex];
    const context = `${TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1}rows[${rowIndex}]`;
    assertExactObjectKeys(row, PRIVACY_EXACT12_PUBLIC_ROW_FIELD_NAMES_V1, context);
    for (const field of PRIVACY_EXACT12_PUBLIC_ROW_FIELD_NAMES_V1) {
      if (field === "protocolId" || field === "submitProofWireId") {
        continue;
      }
      const length = binaryByteLength(row[field]);
      if (length === null) {
        rejectType(`${context}.${field}${TEXT_MUST_BE}an exact byte sequence`);
      }
      declaredBytes += length;
      if (declaredBytes > PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1) {
        rejectRange(`${TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1_2}fields exceed the ${PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1}-byte archive limit`);
      }
    }
    if (typeof row.submitProofWireId !== JS_TYPE_STRING) {
      rejectType(`${context}${TEXT_SUBMIT_PROOF_WIRE_ID_MUST_BE}a string`);
    }
    declaredBytes += Buffer.byteLength(row.submitProofWireId, UTF8_ENCODING);
    if (declaredBytes > PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1) {
      rejectRange(`${TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1_2}fields exceed the ${PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1}-byte archive limit`);
    }
  }
}

function normalizePrivacyExact12InputBytesV1(value, context, exactLength = null) {
  let bytes;
  if (Buffer.isBuffer(value)) {
    bytes = Buffer.from(value);
  } else if (ArrayBuffer.isView(value)) {
    bytes = Buffer.from(value.buffer, value.byteOffset, value.byteLength);
    bytes = Buffer.from(bytes);
  } else if (value instanceof ArrayBuffer) {
    bytes = Buffer.from(value.slice(0));
  } else if (Array.isArray(value)) {
    bytes = Buffer.allocUnsafe(value.length);
    for (let index = 0; index < value.length; index += 1) {
      const byte = value[index];
      if (!Number.isInteger(byte) || byte < 0 || byte > 0xff) {
        rejectType(`${context}[${index}]${TEXT_MUST_BE}an unsigned byte`);
      }
      bytes[index] = byte;
    }
  } else {
    rejectType(`${context}${TEXT_MUST_BE}an exact byte sequence`);
  }
  if (exactLength === null && bytes.length === 0) {
    rejectType(`${context}${TEXT_MUST_NOT_BE}empty`);
  }
  if (exactLength !== null && bytes.length !== exactLength) {
    rejectType(`${context}${TEXT_MUST_CONTAIN_EXACTLY}${exactLength} bytes`);
  }
  return bytes;
}

function encodePrivacyExact12FixtureBundleCanonicalV1(bundle) {
  const payload = withNoritoCompactLengths(() =>
    encodeStructValue([
      [encodeU32Value(bundle.version, (TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1 + "version"))],
      [
        encodeNoritoVec(bundle.rows, (row, rowIndex) =>
          encodePrivacyExact12FixtureRowV1(row, rowIndex),
        ),
      ],
    ]),
  );
  const archive = frameNoritoPayload(
    payload,
    PRIVACY_EXACT12_FIXTURE_BUNDLE_SCHEMA_HASH_V1,
    COMPACT_LEN_FLAG,
    0,
  );
  if (archive.length > PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1) {
    rejectRange(`${TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1_2}archive exceeds ${PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1} bytes`);
  }
  return archive;
}

function encodePrivacyExact12FixtureRowV1(row, rowIndex) {
  const context = `${TEXT_PRIVACY_EXACT12_FIXTURE_BUNDLE_V1}rows[${rowIndex}]`;
  return encodeStructValue([
    [encodeU32Value(rowIndex, `${context}.protocol_id`)],
    [encodeByteVecValue(row.statementNorito, `${context}.statement_norito`)],
    [encodeByteVecValue(row.envelopeNorito, `${context}.envelope_norito`)],
    [encodeNoritoStringValue(row.submitProofWireId)],
    [
      encodeByteVecValue(
        row.submitProofInstructionNorito,
        `${context}.${TEXT_SUBMIT_PROOF_INSTRUCTION_NORITO}`,
      ),
    ],
    [
      encodeByteVecValue(
        row.transactionIntentProjectionNorito,
        `${context}${TEXT_TRANSACTION_INTENT_PROJECTION_NORITO}`,
      ),
    ],
    [
      encodeFixedBytesValue(
        row.transactionIntentDigest,
        32,
        `${context}.${TEXT_TRANSACTION_INTENT}digest`,
      ),
    ],
    [
      encodeByteVecValue(
        row.unsignedTransactionPayloadNorito,
        `${context}${TEXT_UNSIGNED_TRANSACTION_PAYLOAD_NORITO_2}`,
      ),
    ],
    [
      encodeByteVecValue(
        row.signedTransactionVersionedNorito,
        `${context}${TEXT_SIGNED_TRANSACTION}versioned_norito`,
      ),
    ],
    [
      encodeFixedBytesValue(
        row.signedTransactionHash,
        32,
        `${context}${TEXT_SIGNED_TRANSACTION}hash`,
      ),
    ],
  ]);
}

function externalizePrivacyExact12FixtureBundleV1(bundle) {
  return {
    version: bundle.version,
    rows: bundle.rows.map((row) => ({
      protocolId: row.protocolId,
      statementNorito: Uint8Array.from(row.statementNorito),
      envelopeNorito: Uint8Array.from(row.envelopeNorito),
      submitProofWireId: row.submitProofWireId,
      submitProofInstructionNorito: Uint8Array.from(
        row.submitProofInstructionNorito,
      ),
      transactionIntentProjectionNorito: Uint8Array.from(
        row.transactionIntentProjectionNorito,
      ),
      transactionIntentDigest: Uint8Array.from(row.transactionIntentDigest),
      unsignedTransactionPayloadNorito: Uint8Array.from(
        row.unsignedTransactionPayloadNorito,
      ),
      signedTransactionVersionedNorito: Uint8Array.from(
        row.signedTransactionVersionedNorito,
      ),
      signedTransactionHash: Uint8Array.from(row.signedTransactionHash),
    })),
  };
}

function isBinaryLike(value) {
  return (
    Buffer.isBuffer(value) ||
    ArrayBuffer.isView(value) ||
    value instanceof ArrayBuffer
  );
}

function toBuffer(value) {
  if (Buffer.isBuffer(value)) {
    return value;
  }
  if (ArrayBuffer.isView(value)) {
    return Buffer.from(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    return Buffer.from(value);
  }
  rejectType(("bytes" + TEXT_MUST_BE + "a Buffer, ArrayBuffer, or typed array"));
}

function encodePureJsInstruction(instruction) {
  return withNoritoLengthFlags(COMPACT_LEN_FLAG, () =>
    encodePureJsInstructionPayload(instruction),
  );
}

function decodeCanonicalKagemushaTopUpRequestArchive(value, context) {
  const archive = toBuffer(value);
  if (archive.length === 0 || archive.length > KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES) {
    rejectRange(`${context}${TEXT_MUST_BE_A}non-empty${TEXT_CANONICAL}KAGEMUSHA top-up request no larger than ${KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES} bytes`);
  }
  const decoded = validateNoritoFrame(archive, {
    context,
    expectedSchemaHash: KAGEMUSHA_TOP_UP_REQUEST_SCHEMA_HASH,
    expectedPaddingLength: KAGEMUSHA_TOP_UP_REQUEST_HEADER_PADDING,
    requireNonEmptyPayload: true,
  });
  if (decoded.flags !== COMPACT_LEN_FLAG) {
    rejectError(`${context} must use the${TEXT_CANONICAL}compact-length Norito layout`);
  }
  const canonical = frameNoritoPayload(
    decoded.payload,
    KAGEMUSHA_TOP_UP_REQUEST_SCHEMA_HASH,
    COMPACT_LEN_FLAG,
    KAGEMUSHA_TOP_UP_REQUEST_HEADER_PADDING,
  );
  if (!archive.equals(canonical)) {
    rejectError(`${context} is not${TEXT_CANONICAL}Norito`);
  }
  return decoded.payload;
}

function encodeTopUpKagemushaInstruction(value) {
  assertOnlyObjectKeys(value, ["request"], WIRE_TYPE_TOP_UP_KAGEMUSHA_V1);
  const requestPayload = decodeCanonicalKagemushaTopUpRequestArchive(
    value.request,
    "TopUpKagemushaV1.request",
  );
  return encodeInstructionEnvelope(
    TOP_UP_KAGEMUSHA_WIRE_ID,
    encodeNoritoField(requestPayload),
  );
}

function decodeTopUpKagemushaInstructionPayload(payload, innerFlags) {
  if (innerFlags !== COMPACT_LEN_FLAG) {
    rejectError(("TopUpKagemushaV1 must use the" + TEXT_CANONICAL + "compact-length Norito layout"));
  }
  const reader = new BufferReader(payload, WIRE_TYPE_TOP_UP_KAGEMUSHA_V1, innerFlags);
  const requestPayload = readNoritoField(reader, "request");
  reader.assertEof();
  const request = frameNoritoPayload(
    requestPayload,
    KAGEMUSHA_TOP_UP_REQUEST_SCHEMA_HASH,
    COMPACT_LEN_FLAG,
    KAGEMUSHA_TOP_UP_REQUEST_HEADER_PADDING,
  );
  decodeCanonicalKagemushaTopUpRequestArchive(
    request,
    "TopUpKagemushaV1.request",
  );
  return { TopUpKagemushaV1: { request } };
}

function encodePureJsInstructionPayload(instruction) {
  const nftNames = NFT_MARKET_INSTRUCTION_NAMES_V1.filter(name => Object.prototype.hasOwnProperty.call(instruction, name));
  if (nftNames.length) {
    const name = nftNames[0]; assertExactObjectKeys(instruction, [name], FIELD_INSTRUCTION);
    return encodeInstructionEnvelope(NFT_MARKET_INSTRUCTION_WIRE_IDS_V1[NFT_MARKET_INSTRUCTION_NAMES_V1.indexOf(name)], nftMarketCodecsV1.encode(name, instruction[name]));
  }

  const gameNames = GAME_INSTRUCTION_NAMES_V1.filter((name) => Object.prototype.hasOwnProperty.call(instruction, name));
  if (gameNames.length > 0) {
    assertExactObjectKeys(instruction, [gameNames[0]], FIELD_INSTRUCTION);
    const name = gameNames[0];
    return encodeInstructionEnvelope(GAME_INSTRUCTION_WIRE_IDS_V1[GAME_INSTRUCTION_NAMES_V1.indexOf(name)], instructionGameCodecsV1.encode(name, instruction[name]));
  }
  if (!isPlainObject(instruction)) {
    rejectType(("instruction" + TEXT_MUST_BE + "a JSON object"));
  }
  if (Object.prototype.hasOwnProperty.call(instruction, WIRE_TYPE_TOP_UP_KAGEMUSHA_V1)) {
    assertOnlyObjectKeys(instruction, [WIRE_TYPE_TOP_UP_KAGEMUSHA_V1], FIELD_INSTRUCTION);
    if (!isPlainObject(instruction.TopUpKagemushaV1)) {
      rejectType(("TopUpKagemushaV1" + TEXT_MUST_BE_AN_OBJECT_2));
    }
    return encodeTopUpKagemushaInstruction(instruction.TopUpKagemushaV1);
  }
  if (isPlainObject(instruction.Mint)) {
    if (isPlainObject(instruction.Mint.Asset)) {
      const body = encodeAssetInstructionBody(instruction.Mint.Asset, "Mint.Asset");
      return encodeEnumInstruction("iroha.mint", 0, body);
    }
    if (isPlainObject(instruction.Mint.TriggerRepetitions)) {
      const body = encodeTriggerRepetitionsBody(
        instruction.Mint.TriggerRepetitions,
        CONTEXT_MINT_TRIGGER_REPETITIONS,
      );
      return encodeEnumInstruction("iroha.mint", 1, body);
    }
  }
  if (isPlainObject(instruction.Burn)) {
    if (isPlainObject(instruction.Burn.Asset)) {
      const body = encodeAssetInstructionBody(instruction.Burn.Asset, "Burn.Asset");
      return encodeEnumInstruction("iroha.burn", 0, body);
    }
    if (isPlainObject(instruction.Burn.TriggerRepetitions)) {
      const body = encodeTriggerRepetitionsBody(
        instruction.Burn.TriggerRepetitions,
        CONTEXT_BURN_TRIGGER_REPETITIONS,
      );
      return encodeEnumInstruction("iroha.burn", 1, body);
    }
  }
  if (isPlainObject(instruction.Transfer) && isPlainObject(instruction.Transfer.Asset)) {
    const body = encodeTransferAssetBody(instruction.Transfer.Asset);
    return encodeEnumInstruction(WIRE_ID_IROHA_TRANSFER, 2, body);
  }
  if (isPlainObject(instruction.Transfer) && isPlainObject(instruction.Transfer.Domain)) {
    return encodeEnumInstruction(
      WIRE_ID_IROHA_TRANSFER,
      0,
      encodeTransferObjectBody(
        instruction.Transfer.Domain,
        CONTEXT_TRANSFER_DOMAIN,
        encodeAccountIdValue,
        encodeDomainIdValue,
        encodeAccountIdValue,
      ),
    );
  }
  if (
    isPlainObject(instruction.Transfer) &&
    isPlainObject(instruction.Transfer.AssetDefinition)
  ) {
    return encodeEnumInstruction(
      WIRE_ID_IROHA_TRANSFER,
      1,
      encodeTransferObjectBody(
        instruction.Transfer.AssetDefinition,
        CONTEXT_TRANSFER_ASSET_DEFINITION,
        encodeAccountIdValue,
        encodeAssetDefinitionIdValue,
        encodeAccountIdValue,
      ),
    );
  }
  if (isPlainObject(instruction.Transfer) && isPlainObject(instruction.Transfer.Nft)) {
    return encodeEnumInstruction(
      WIRE_ID_IROHA_TRANSFER,
      3,
      encodeTransferObjectBody(
        instruction.Transfer.Nft,
        "Transfer.Nft",
        encodeAccountIdValue,
        encodeNftIdValue,
        encodeAccountIdValue,
      ),
    );
  }
  if (isPlainObject(instruction.Register) && isPlainObject(instruction.Register.Domain)) {
    return encodeEnumInstruction(
      WIRE_ID_IROHA_REGISTER,
      1,
      encodeNoritoField(encodeNewDomainValue(instruction.Register.Domain, CONTEXT_REGISTER_DOMAIN)),
    );
  }
  if (isPlainObject(instruction.Register) && isPlainObject(instruction.Register.Account)) {
    return encodeEnumInstruction(
      WIRE_ID_IROHA_REGISTER,
      2,
      encodeNoritoField(encodeNewAccountValue(instruction.Register.Account, CONTEXT_REGISTER_ACCOUNT)),
    );
  }
  if (
    isPlainObject(instruction.Register) &&
    isPlainObject(instruction.Register.AssetDefinition)
  ) {
    return encodeEnumInstruction(
      WIRE_ID_IROHA_REGISTER,
      3,
      encodeNoritoField(
        encodeNewAssetDefinitionValue(
          instruction.Register.AssetDefinition,
          CONTEXT_REGISTER_ASSET_DEFINITION,
        ),
      ),
    );
  }
  if (isPlainObject(instruction.ExecuteTrigger)) {
    const payload = encodeExecuteTriggerPayload(instruction.ExecuteTrigger);
    return encodeInstructionEnvelope("iroha.execute_trigger", payload);
  }
  if (Object.prototype.hasOwnProperty.call(instruction, WIRE_TYPE_CANCEL_ASSET_LOCK)) {
    assertOnlyObjectKeys(instruction, [WIRE_TYPE_CANCEL_ASSET_LOCK], FIELD_INSTRUCTION);
    return encodeCancelAssetLockInstruction(instruction.CancelAssetLock);
  }
  if (
    Object.prototype.hasOwnProperty.call(
      instruction,
      SET_ASSET_TRANSFER_AVAILABILITY_VARIANT,
    )
  ) {
    assertOnlyObjectKeys(
      instruction,
      [SET_ASSET_TRANSFER_AVAILABILITY_VARIANT],
      FIELD_INSTRUCTION,
    );
    return encodeSetAssetTransferAvailabilityInstruction(
      instruction.SetAssetTransferAvailability,
    );
  }
  if (
    Object.prototype.hasOwnProperty.call(
      instruction,
      SET_ASSET_TRANSFER_BLACKLIST_VARIANT,
    )
  ) {
    assertOnlyObjectKeys(
      instruction,
      [SET_ASSET_TRANSFER_BLACKLIST_VARIANT],
      FIELD_INSTRUCTION,
    );
    return encodeSetAssetTransferBlacklistInstruction(
      instruction.SetAssetTransferBlacklist,
    );
  }
  if (
    Object.prototype.hasOwnProperty.call(
      instruction,
      SET_ASSET_TRANSFER_CONTROL_VARIANT,
    )
  ) {
    assertOnlyObjectKeys(
      instruction,
      [SET_ASSET_TRANSFER_CONTROL_VARIANT],
      FIELD_INSTRUCTION,
    );
    return encodeSetAssetTransferControlInstruction(
      instruction.SetAssetTransferControl,
    );
  }
  if (
    isPlainObject(instruction.IssueReplicationOrder) ||
    isPlainObject(instruction.CompleteReplicationOrder) ||
    isPlainObject(instruction.ExpireReplicationOrder)
  ) {
    return encodeReplicationOrderInstruction(instruction);
  }
  if (isPlainObject(instruction.RecordSccpMessage)) {
    return encodeRecordSccpMessageInstruction(instruction.RecordSccpMessage);
  }
  if (isPlainObject(instruction.Custom)) {
    return encodeInstructionEnvelope(
      WIRE_ID_IROHA_CUSTOM,
      encodeCustomInstructionPayload(instruction.Custom),
    );
  }
  if (isPlainObject(instruction.Multisig)) {
    return encodeInstructionEnvelope(
      WIRE_ID_IROHA_CUSTOM,
      encodeCustomInstructionPayload({ payload: instruction.Multisig }),
    );
  }
  if (isPlainObject(instruction.MultisigRegister)) {
    return encodeInstructionEnvelope(
      WIRE_ID_IROHA_CUSTOM,
      encodeCustomInstructionPayload({ payload: { Register: instruction.MultisigRegister } }),
    );
  }
  if (isPlainObject(instruction.MultisigPropose)) {
    return encodeInstructionEnvelope(
      WIRE_ID_IROHA_CUSTOM,
      encodeCustomInstructionPayload({ payload: { Propose: instruction.MultisigPropose } }),
    );
  }
  if (isPlainObject(instruction.MultisigApprove)) {
    return encodeInstructionEnvelope(
      WIRE_ID_IROHA_CUSTOM,
      encodeCustomInstructionPayload({ payload: { Approve: instruction.MultisigApprove } }),
    );
  }
  if (isPlainObject(instruction.MultisigCancel)) {
    return encodeInstructionEnvelope(
      WIRE_ID_IROHA_CUSTOM,
      encodeCustomInstructionPayload({ payload: { Cancel: instruction.MultisigCancel } }),
    );
  }
  if (isPlainObject(instruction.Kaigi)) {
    return encodeKaigiInstruction(instruction.Kaigi);
  }
  if (isPlainObject(instruction.zk)) {
    return encodeZkInstruction(instruction.zk);
  }
  if (isPlainObject(instruction.verifying_keys)) {
    return encodeVerifyingKeyInstruction(instruction.verifying_keys);
  }
  if (isPlainObject(instruction.VerifyingKeys)) {
    return encodeVerifyingKeyInstruction(instruction.VerifyingKeys);
  }
  if (
    isPlainObject(instruction.RegisterVerifyingKey) ||
    isPlainObject(instruction.UpdateVerifyingKey)
  ) {
    return encodeVerifyingKeyInstruction(instruction);
  }
  if (instruction.RegisterRwa || instruction.TransferRwa || instruction.MergeRwas) {
    return encodeRwaInstruction(instruction);
  }
  if (
    instruction.RedeemRwa ||
    instruction.FreezeRwa ||
    instruction.UnfreezeRwa ||
    instruction.HoldRwa ||
    instruction.ReleaseRwa ||
    instruction.ForceTransferRwa ||
    instruction.SetRwaControls ||
    instruction.SetRwaKeyValue ||
    instruction.RemoveRwaKeyValue
  ) {
    return encodeRwaInstruction(instruction);
  }
  if (
    instruction.ProposeDeployContract ||
    instruction.CastZkBallot ||
    instruction.CastPlainBallot
  ) {
    return encodeGovernanceInstruction(instruction);
  }
  if (
    instruction.ClaimTwitterFollowReward ||
    instruction.SendToTwitter ||
    instruction.CancelTwitterEscrow
  ) {
    return encodeSocialInstruction(instruction);
  }
  if (
    instruction.RegisterSmartContractCode ||
    instruction.RegisterSmartContractBytes ||
    instruction.DeactivateContractInstance ||
    instruction.ActivateContractInstance ||
    instruction.SetContractParliamentDelegation ||
    instruction.OfferContractOwnership ||
    instruction.AcceptContractOwnership ||
    instruction.CancelContractOwnershipOffer ||
    instruction.CommitContractDeployment ||
    instruction.UploadSmartContractCodeChunk ||
    instruction.FinalizeSmartContractCodeUpload ||
    instruction.CancelSmartContractCodeUpload ||
    instruction.RemoveSmartContractBytes
  ) {
    return encodeSmartContractInstruction(instruction);
  }
  throw new PureJsUnsupportedInstructionError(
    `Internal Norito canonicalization supports ${SUPPORTED_JS_CANONICALIZATION_INSTRUCTIONS.join(", ")}. Received ${describeInstructionShape(instruction)}.`,
  );
}

function decodePureJsInstruction(buffer) {
  const { wireId, payload, innerFlags } = decodeInstructionEnvelope(buffer);
  return withNoritoLengthFlags(innerFlags, () =>
    decodePureJsInstructionPayload(wireId, payload, innerFlags),
  );
}

function decodePureJsInstructionPayload(wireId, payload, innerFlags) {
  const nftIndex = NFT_MARKET_INSTRUCTION_WIRE_IDS_V1.indexOf(wireId);
  if (nftIndex >= 0) { const name = NFT_MARKET_INSTRUCTION_NAMES_V1[nftIndex]; return { [name]: nftMarketCodecsV1.decode(name, payload) }; }

  const gameIndex = GAME_INSTRUCTION_WIRE_IDS_V1.indexOf(wireId);
  if (gameIndex >= 0) {
    const name = GAME_INSTRUCTION_NAMES_V1[gameIndex];
    return { [name]: instructionGameCodecsV1.decode(name, payload) };
  }
  switch (wireId) {
    case "iroha.mint":
      return { Mint: decodeMintPayload(payload) };
    case "iroha.burn":
      return { Burn: decodeBurnPayload(payload) };
    case WIRE_ID_IROHA_REGISTER:
      return { Register: decodeRegisterPayload(payload) };
    case WIRE_ID_IROHA_TRANSFER:
      return { Transfer: decodeTransferPayload(payload) };
    case WIRE_ID_IROHA_CUSTOM:
      return { Custom: decodeCustomInstructionPayload(payload) };
    case "iroha.execute_trigger":
      return { ExecuteTrigger: decodeExecuteTriggerPayload(payload) };
    case "iroha.rwa":
      return decodeRwaInstructionPayload(payload);
    case TOP_UP_KAGEMUSHA_WIRE_ID:
      return decodeTopUpKagemushaInstructionPayload(payload, innerFlags);
    case CANCEL_ASSET_LOCK_WIRE_ID:
      return decodeCancelAssetLockInstructionPayload(payload);
    case SET_ASSET_TRANSFER_AVAILABILITY_WIRE_ID:
      return decodeSetAssetTransferAvailabilityInstructionPayload(payload);
    case SET_ASSET_TRANSFER_BLACKLIST_WIRE_ID:
      return decodeSetAssetTransferBlacklistInstructionPayload(payload);
    case SET_ASSET_TRANSFER_CONTROL_WIRE_ID:
      return decodeSetAssetTransferControlInstructionPayload(payload);
    case ISSUE_REPLICATION_ORDER_WIRE_ID:
    case COMPLETE_REPLICATION_ORDER_WIRE_ID:
    case EXPIRE_REPLICATION_ORDER_WIRE_ID:
      return decodeReplicationOrderInstructionPayload(wireId, payload);
    case RECORD_SCCP_MESSAGE_WIRE_ID:
      return {
        RecordSccpMessage: decodeRecordSccpMessagePayload(payload, innerFlags),
      };
    case PROPOSE_DEPLOY_CONTRACT_WIRE_ID:
    case CAST_ZK_BALLOT_WIRE_ID:
    case CAST_PLAIN_BALLOT_WIRE_ID:
      return decodeGovernanceInstructionPayload(wireId, payload);
    case CLAIM_TWITTER_FOLLOW_REWARD_WIRE_ID:
    case SEND_TO_TWITTER_WIRE_ID:
    case CANCEL_TWITTER_ESCROW_WIRE_ID:
      return decodeSocialInstructionPayload(wireId, payload);
    case REGISTER_SMART_CONTRACT_CODE_WIRE_ID:
    case REGISTER_SMART_CONTRACT_BYTES_WIRE_ID:
    case DEACTIVATE_CONTRACT_INSTANCE_WIRE_ID:
    case ACTIVATE_CONTRACT_INSTANCE_WIRE_ID:
    case COMMIT_CONTRACT_DEPLOYMENT_WIRE_ID:
    case UPLOAD_SMART_CONTRACT_CODE_CHUNK_WIRE_ID:
    case FINALIZE_SMART_CONTRACT_CODE_UPLOAD_WIRE_ID:
    case CANCEL_SMART_CONTRACT_CODE_UPLOAD_WIRE_ID:
    case REMOVE_SMART_CONTRACT_BYTES_WIRE_ID:
    case SET_CONTRACT_PARLIAMENT_DELEGATION_WIRE_ID:
    case OFFER_CONTRACT_OWNERSHIP_WIRE_ID:
    case ACCEPT_CONTRACT_OWNERSHIP_WIRE_ID:
    case CANCEL_CONTRACT_OWNERSHIP_OFFER_WIRE_ID:
      return decodeSmartContractInstructionPayload(wireId, payload);
    case CREATE_KAIGI_WIRE_ID:
    case JOIN_KAIGI_WIRE_ID:
    case LEAVE_KAIGI_WIRE_ID:
    case END_KAIGI_WIRE_ID:
    case RECORD_KAIGI_USAGE_WIRE_ID:
    case SET_KAIGI_RELAY_MANIFEST_WIRE_ID:
    case REGISTER_KAIGI_RELAY_WIRE_ID:
    case UNREGISTER_KAIGI_RELAY_WIRE_ID:
    case REPORT_KAIGI_RELAY_HEALTH_WIRE_ID:
      return decodeKaigiInstructionPayload(wireId, payload);
    case REGISTER_ZK_ASSET_WIRE_ID:
    case SCHEDULE_CONFIDENTIAL_POLICY_TRANSITION_WIRE_ID:
    case CANCEL_CONFIDENTIAL_POLICY_TRANSITION_WIRE_ID:
    case CREATE_ELECTION_WIRE_ID:
    case SUBMIT_BALLOT_WIRE_ID:
    case FINALIZE_ELECTION_WIRE_ID:
      return decodeZkInstructionPayload(wireId, payload);
    case REGISTER_VERIFYING_KEY_WIRE_ID:
    case UPDATE_VERIFYING_KEY_WIRE_ID:
      return decodeVerifyingKeyInstructionPayload(wireId, payload);
    default:
      throw new PureJsUnsupportedInstructionError(
        `${TEXT_INTERNAL_NORITO_DECODER_DOES_NOT_SUPPORT}${wireId}. Run \`npm run build:native\` for full instruction coverage.`,
      );
  }
}

function decodeInstructionEnvelope(bytes) {
  const outer = decodeNoritoFrame(bytes, FIELD_INSTRUCTION, INSTRUCTION_BOX_SCHEMA_HASH);
  const outerReader = new BufferReader(outer.payload, "instruction.outer", outer.flags);
  const wireId = decodeStringValue(
    readNoritoField(outerReader, "wire"),
    "instruction.outer.wire",
    outer.flags,
  );
  const innerField = readNoritoField(outerReader, "inner");
  const innerReader = new BufferReader(
    innerField,
    "instruction.outer.inner",
    0,
  );
  const innerBytes = readNoritoField(innerReader, "frame");
  innerReader.assertEof();
  outerReader.assertEof();
  const inner = decodeNoritoFrame(
    innerBytes,
    "instruction.inner",
    INNER_SCHEMA_HASH_BY_WIRE_ID[wireId] ?? null,
  );
  return { wireId, payload: inner.payload, innerFlags: inner.flags, innerFrame: innerBytes };
}

function encodeInstructionBoxPayload(
  wireId,
  innerPayload,
  outerFlags,
  context = FIELD_INSTRUCTION,
  innerFlags = noritoLengthFlags & COMPACT_LEN_FLAG,
  decodedInnerFrame = null,
) {
  const innerSchemaHash = INNER_SCHEMA_HASH_BY_WIRE_ID[wireId];
  let innerFrame;
  if (innerSchemaHash) {
    innerFrame = frameNoritoPayload(
      innerPayload,
      innerSchemaHash,
      innerFlags,
      INNER_HEADER_PADDING_BY_WIRE_ID[wireId] ?? 0,
    );
  } else if (decodedInnerFrame !== null) {
    innerFrame = Buffer.from(decodedInnerFrame);
  } else {
    rejectError(`${context}${TEXT_USES_UNSUPPORTED}${TEXT_INSTRUCTION}wire id ${wireId}; native embedding requires a schema hash`);
  }
  const innerFieldPayload = withNoritoU64Lengths(() => encodeNoritoField(innerFrame));
  return withNoritoLengthFlags(outerFlags, () =>
    Buffer.concat([
      encodeNoritoField(encodeNoritoStringValue(wireId)),
      encodeNoritoField(innerFieldPayload),
    ]),
  );
}

function encodeInstructionEnvelope(wireId, innerPayload) {
  const flags = noritoLengthFlags & COMPACT_LEN_FLAG;
  const outerPayload = encodeInstructionBoxPayload(
    wireId,
    innerPayload,
    flags,
    FIELD_INSTRUCTION,
    flags,
  );
  return frameNoritoPayload(outerPayload, INSTRUCTION_BOX_SCHEMA_HASH, flags);
}

function encodeEnumInstruction(wireId, variantIndex, bodyPayload) {
  const innerPayload = Buffer.concat([
    u32ToLittleEndianBuffer(variantIndex),
    encodeNoritoField(bodyPayload),
  ]);
  return encodeInstructionEnvelope(wireId, innerPayload);
}

function recordSccpPayloadBytes(input) {
  const selected =
    input.payload_bytes ??
    input.payloadBytes ??
    input.payload_bytes_hex ??
    input.payloadBytesHex;
  if (selected === undefined || selected === null) {
    rejectType((TEXT_RECORD_SCCP_MESSAGE_PAYLOAD + "bytes is required"));
  }
  return Buffer.from(normalizeBytes(selected));
}

function encodeRecordSccpMessagePayload(input) {
  const payloadBytes = recordSccpPayloadBytes(input);
  const vecPayload = Buffer.concat([
    u64ToLittleEndianBuffer(BigInt(payloadBytes.length)),
    payloadBytes,
  ]);
  return encodeNoritoField(vecPayload);
}

function encodeRecordSccpMessageInstruction(input) {
  const payload = withNoritoCompactLengths(() =>
    encodeRecordSccpMessagePayload(input),
  );
  const outerPayload = encodeInstructionBoxPayload(
    RECORD_SCCP_MESSAGE_WIRE_ID,
    payload,
    COMPACT_LEN_FLAG,
    WIRE_TYPE_RECORD_SCCP_MESSAGE,
    COMPACT_LEN_FLAG,
  );
  return frameNoritoPayload(
    outerPayload,
    INSTRUCTION_BOX_SCHEMA_HASH,
    COMPACT_LEN_FLAG,
  );
}

function decodeRecordSccpMessagePayload(payload, innerFlags) {
  const reader = new BufferReader(payload, WIRE_TYPE_RECORD_SCCP_MESSAGE, innerFlags);
  const field = readNoritoField(reader, "payload_bytes");
  reader.assertEof();
  if (field.length < 8) {
    rejectError((TEXT_RECORD_SCCP_MESSAGE_PAYLOAD + "bytes is too short"));
  }
  const count = bigintToSafeNumber(
    field.readBigUInt64LE(0),
    (TEXT_RECORD_SCCP_MESSAGE_PAYLOAD + "bytes.length"),
  );
  const payloadBytes = field.subarray(8);
  if (payloadBytes.length !== count) {
    rejectError((TEXT_RECORD_SCCP_MESSAGE_PAYLOAD + "bytes length mismatch"));
  }
  return { payload_bytes: Array.from(payloadBytes) };
}

function assertWellFormedUtf16(value, context) {
  for (let index = 0; index < value.length; index += 1) {
    const codeUnit = value.charCodeAt(index);
    if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) {
        rejectType(`${context}${TEXT_MUST_NOT_CONTAIN}unpaired UTF-16 surrogates`);
      }
      index += 1;
    } else if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
      rejectType(`${context}${TEXT_MUST_NOT_CONTAIN}unpaired UTF-16 surrogates`);
    }
  }
}

function normalizeStrictCancelAssetLockV1(value) {
  const prototype =
    value !== null && typeof value === JS_TYPE_OBJECT
      ? Object.getPrototypeOf(value)
      : undefined;
  if (
    prototype !== Object.prototype &&
    prototype !== null
  ) {
    rejectType((TEXT_CANCEL_ASSET_LOCK_V1_2 + "must be a plain object"));
  }
  const keys = Reflect.ownKeys(value);
  if (
    keys.length !== 2 ||
    !keys.includes("escrow_id") ||
    !keys.includes(WIRE_FIELD_EXPECTED_REMAINING_AMOUNT)
  ) {
    rejectType((TEXT_CANCEL_ASSET_LOCK_V1_2 + "must contain exactly escrow_id and " + TEXT_EXPECTED_REMAINING_AMOUNT));
  }

  const { escrow_id: escrowId, expected_remaining_amount: expectedRemainingAmount } =
    value;
  if (typeof escrowId !== JS_TYPE_STRING) {
    rejectType((TEXT_CANCEL_ASSET_LOCK_V1 + "escrow_id" + TEXT_MUST_BE + "a string"));
  }
  assertWellFormedUtf16(escrowId, (TEXT_CANCEL_ASSET_LOCK_V1 + "escrow_id"));
  const hashMatch = CANONICAL_HASH_LITERAL_RE.exec(escrowId);
  if (hashMatch === null) {
    rejectType((TEXT_CANCEL_ASSET_LOCK_V1 + "escrow_id must be one" + TEXT_CANONICAL + "uppercase checksummed hash literal"));
  }
  const [, hashBody, checksum] = hashMatch;
  const expectedChecksum = computeHashLiteralCrc("hash", hashBody);
  if (checksum !== expectedChecksum) {
    rejectType(`${TEXT_CANCEL_ASSET_LOCK_V1}escrow_id has invalid checksum; expected ${expectedChecksum}`);
  }
  const hashBytes = Buffer.from(hashBody, HEX_ENCODING);
  if ((hashBytes[hashBytes.length - 1] & 1) === 0) {
    rejectType((TEXT_CANCEL_ASSET_LOCK_V1 + "escrow_id" + TEXT_MUST_USE_A_NATIVE_HASH_WITH_ITS_MARKER_BIT_SET_2));
  }

  if (typeof expectedRemainingAmount !== JS_TYPE_STRING) {
    rejectType((TEXT_CANCEL_ASSET_LOCK_V1 + "expected_remaining_amount must be a" + TEXT_CANONICAL + "quantity string"));
  }
  assertWellFormedUtf16(
    expectedRemainingAmount,
    (TEXT_CANCEL_ASSET_LOCK_V1 + TEXT_EXPECTED_REMAINING_AMOUNT),
  );
  const quantity = NumericV1.decodeQuantityJson(expectedRemainingAmount);
  if (quantity.mantissa <= 0n) {
    rejectRange((TEXT_CANCEL_ASSET_LOCK_V1 + "expected_remaining_amount" + TEXT_MUST_BE_GREATER_THAN_ZERO));
  }

  return {
    escrow_id: escrowId,
    expected_remaining_amount: expectedRemainingAmount,
  };
}

function encodeCancelAssetLockPayload(value) {
  if (!isPlainObject(value)) {
    rejectType(("CancelAssetLock" + TEXT_MUST_BE_AN_OBJECT_2));
  }
  assertOnlyObjectKeys(
    value,
    ["escrow_id", WIRE_FIELD_EXPECTED_REMAINING_AMOUNT],
    WIRE_TYPE_CANCEL_ASSET_LOCK,
  );
  for (const field of ["escrow_id", WIRE_FIELD_EXPECTED_REMAINING_AMOUNT]) {
    if (!Object.prototype.hasOwnProperty.call(value, field)) {
      rejectType(`${TEXT_CANCEL_ASSET_LOCK}${field}${TEXT_IS_REQUIRED}`);
    }
  }
  const expected = parseNumericLiteral(
    value.expected_remaining_amount,
    CANCEL_LOCK_REMAINING_CONTEXT,
  );
  if (expected.mantissa <= 0n) {
    rejectRange(CANCEL_LOCK_REMAINING_MESSAGE);
  }
  const payload = encodeStructValue([
    [
      encodeEscrowIdValue(
        value.escrow_id,
        (TEXT_CANCEL_ASSET_LOCK + "escrow_id"),
      ),
    ],
    [
      encodeQuantityValue(
        value.expected_remaining_amount,
        CANCEL_LOCK_REMAINING_CONTEXT,
      ),
    ],
  ]);
  return payload;
}

function encodeCancelAssetLockInstruction(value) {
  return encodeInstructionEnvelope(
    CANCEL_ASSET_LOCK_WIRE_ID,
    encodeCancelAssetLockPayload(value),
  );
}

function decodeCancelAssetLockInstructionPayload(payload) {
  const fields = decodeStructFields(payload, WIRE_TYPE_CANCEL_ASSET_LOCK, [
    "escrow_id",
    WIRE_FIELD_EXPECTED_REMAINING_AMOUNT,
  ]);
  const expectedRemainingAmount = decodeQuantityValue(
    fields.expected_remaining_amount,
    CANCEL_LOCK_REMAINING_CONTEXT,
  );
  if (
    NumericV1.decodeQuantityJson(expectedRemainingAmount).mantissa <= 0n
  ) {
    rejectRange(CANCEL_LOCK_REMAINING_MESSAGE);
  }
  return {
    CancelAssetLock: {
      escrow_id: decodeEscrowIdValue(
        fields.escrow_id,
        (TEXT_CANCEL_ASSET_LOCK + "escrow_id"),
      ),
      expected_remaining_amount: expectedRemainingAmount,
    },
  };
}

/**
 * Encode the schema-bound bare `CancelAssetLock` V1 archive.
 *
 * The input is the exact two-field wire object. Hash bytes, hex strings,
 * base64 strings, camel-case aliases, and nested compatibility shapes are not
 * accepted for either field.
 *
 * @param {{escrow_id: string, expected_remaining_amount: string}} value
 * @returns {Uint8Array<ArrayBuffer>}
 */
export function encodeCancelAssetLockV1(value) {
  const canonical = normalizeStrictCancelAssetLockV1(value);
  const payload = withNoritoCompactLengths(() =>
    encodeCancelAssetLockPayload(canonical),
  );
  return Uint8Array.from(
    frameNoritoPayload(
      payload,
      CANCEL_ASSET_LOCK_V1_SCHEMA_HASH,
      COMPACT_LEN_FLAG,
    ),
  );
}

function isExactOwnedUint8Array(value) {
  if (
    !(value instanceof Uint8Array) ||
    Buffer.isBuffer(value) ||
    Object.getPrototypeOf(value) !== Uint8Array.prototype
  ) {
    return false;
  }
  try {
    const buffer = value.buffer;
    return (
      Object.getPrototypeOf(buffer) === ArrayBuffer.prototype &&
      value.byteOffset === 0 &&
      value.byteLength === buffer.byteLength
    );
  } catch {
    return false;
  }
}

/**
 * Decode one exact schema-bound bare `CancelAssetLock` V1 archive.
 *
 * Textual hex/base64 aliases, arrays, padding, substituted schemas or flags,
 * and trailing bytes are rejected.
 *
 * The archive must be an ordinary, full-span `Uint8Array` backed by its own
 * `ArrayBuffer`. Buffer, ArrayBuffer, shared, subclass, and partial-view aliases
 * are rejected.
 *
 * @param {Uint8Array<ArrayBuffer>} bytes
 * @returns {{escrow_id: string, expected_remaining_amount: string}}
 */
export function decodeCancelAssetLockV1(bytes) {
  if (!isExactOwnedUint8Array(bytes)) {
    rejectType((TEXT_CANCEL_ASSET_LOCK_V1_2 + "archive" + TEXT_MUST_BE + "an owned, full-span Uint8Array"));
  }
  if (
    bytes.byteLength < CANCEL_ASSET_LOCK_V1_MIN_ARCHIVE_BYTES ||
    bytes.byteLength > CANCEL_ASSET_LOCK_V1_MAX_ARCHIVE_BYTES
  ) {
    rejectRange(`${TEXT_CANCEL_ASSET_LOCK_V1_2}archive${TEXT_MUST_CONTAIN}between ${CANCEL_ASSET_LOCK_V1_MIN_ARCHIVE_BYTES} and ${CANCEL_ASSET_LOCK_V1_MAX_ARCHIVE_BYTES}${TEXT_CANONICAL}bytes`);
  }
  const archive = Buffer.from(bytes.buffer);
  const frame = validateNoritoFrame(archive, {
    context: "CancelAssetLockV1",
    expectedSchemaHash: CANCEL_ASSET_LOCK_V1_SCHEMA_HASH,
    expectedPaddingLength: 0,
    requireNonEmptyPayload: true,
  });
  if (frame.flags !== COMPACT_LEN_FLAG) {
    rejectError((TEXT_CANCEL_ASSET_LOCK_V1_2 + "must use exactly the compact-length Norito flag"));
  }
  const decoded = withNoritoCompactLengths(
    () => decodeCancelAssetLockInstructionPayload(frame.payload).CancelAssetLock,
  );
  const canonical = normalizeStrictCancelAssetLockV1(decoded);
  const reencoded = encodeCancelAssetLockV1(canonical);
  if (!archive.equals(reencoded)) {
    rejectError((TEXT_CANCEL_ASSET_LOCK_V1_2 + "archive is not byte-canonical"));
  }
  return canonical;
}

function encodeAssetTransferAvailabilityValue(value, context) {
  if (value === "Enabled") {
    return encodeEnumTagValue(0);
  }
  if (value === "Disabled") {
    return encodeEnumTagValue(1);
  }
  rejectType(`${context}${TEXT_MUST_BE}exactly "Enabled" or "Disabled"`);
}

function decodeAssetTransferAvailabilityValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("tag");
  reader.assertEof();
  if (tag === 0) {
    return "Enabled";
  }
  if (tag === 1) {
    return "Disabled";
  }
  rejectError(`${context}${TEXT_USES_UNSUPPORTED}availability tag ${tag}`);
}

function validateAssetTransferAvailabilityReason(reason, context) {
  if (reason === null) {
    return;
  }
  if (
    typeof reason !== JS_TYPE_STRING ||
    reason.length === 0 ||
    reason.trim() !== reason
  ) {
    rejectType(`${context}${TEXT_MUST_BE}non-empty unpadded text when provided`);
  }
  if (/[\u0000-\u001f\u007f-\u009f]/u.test(reason)) {
    rejectType(`${context}${TEXT_MUST_NOT_CONTAIN}control characters`);
  }
  if (
    Buffer.byteLength(reason, UTF8_ENCODING) >
    ASSET_TRANSFER_AVAILABILITY_MAX_REASON_BYTES_V1
  ) {
    rejectRange(`${context} exceeds 512 UTF-8 bytes`);
  }
}

function encodeSetAssetTransferAvailabilityInstruction(value) {
  if (!isPlainObject(value)) {
    rejectType(("SetAssetTransferAvailability" + TEXT_MUST_BE_AN_OBJECT_2));
  }
  const fields = [
    WIRE_FIELD_ACCOUNT_ID,
    WIRE_FIELD_ASSET_DEFINITION_ID,
    WIRE_FIELD_EXPECTED_REVISION,
    "incoming",
    "outgoing",
    "reason",
  ];
  assertOnlyObjectKeys(value, fields, SET_ASSET_TRANSFER_AVAILABILITY_VARIANT);
  for (const field of fields.slice(0, 5)) {
    if (!Object.prototype.hasOwnProperty.call(value, field)) {
      rejectType(`${TEXT_SET_ASSET_TRANSFER_AVAILABILITY}${field}${TEXT_IS_REQUIRED}`);
    }
  }
  const reason = value.reason ?? null;
  validateAssetTransferAvailabilityReason(
    reason,
    SET_TRANSFER_REASON_CONTEXT,
  );
  const payload = encodeStructValue([
    [
      encodeAccountIdValue(
        value.account_id,
        (TEXT_SET_ASSET_TRANSFER_AVAILABILITY + TEXT_ACCOUNT_ID),
      ),
    ],
    [
      encodeAssetDefinitionIdValue(
        value.asset_definition_id,
        (TEXT_SET_ASSET_TRANSFER_AVAILABILITY + TEXT_ASSET_DEFINITION_ID),
      ),
    ],
    [
      encodeU64NumberValue(
        value.expected_revision,
        (TEXT_SET_ASSET_TRANSFER_AVAILABILITY + TEXT_EXPECTED_REVISION_2),
      ),
    ],
    [
      encodeAssetTransferAvailabilityValue(
        value.incoming,
        (TEXT_SET_ASSET_TRANSFER_AVAILABILITY + "incoming"),
      ),
    ],
    [
      encodeAssetTransferAvailabilityValue(
        value.outgoing,
        (TEXT_SET_ASSET_TRANSFER_AVAILABILITY + "outgoing"),
      ),
    ],
    [
      encodeOptionValue(
        reason,
        encodeStringValue,
        SET_TRANSFER_REASON_CONTEXT,
      ),
    ],
  ]);
  return encodeInstructionEnvelope(
    SET_ASSET_TRANSFER_AVAILABILITY_WIRE_ID,
    payload,
  );
}

function decodeSetAssetTransferAvailabilityInstructionPayload(payload) {
  const fields = decodeStructFields(payload, SET_ASSET_TRANSFER_AVAILABILITY_VARIANT, [
    WIRE_FIELD_ACCOUNT_ID,
    WIRE_FIELD_ASSET_DEFINITION_ID,
    WIRE_FIELD_EXPECTED_REVISION,
    "incoming",
    "outgoing",
    "reason",
  ]);
  const reason = decodeOptionValue(
    fields.reason,
    decodeStringValue,
    SET_TRANSFER_REASON_CONTEXT,
  );
  validateAssetTransferAvailabilityReason(
    reason,
    SET_TRANSFER_REASON_CONTEXT,
  );
  return {
    SetAssetTransferAvailability: {
      account_id: decodeAccountIdValue(
        fields.account_id,
        (TEXT_SET_ASSET_TRANSFER_AVAILABILITY + TEXT_ACCOUNT_ID),
      ),
      asset_definition_id: decodeAssetDefinitionIdValue(
        fields.asset_definition_id,
        (TEXT_SET_ASSET_TRANSFER_AVAILABILITY + TEXT_ASSET_DEFINITION_ID),
      ),
      expected_revision: decodeU64Value(
        fields.expected_revision,
        (TEXT_SET_ASSET_TRANSFER_AVAILABILITY + TEXT_EXPECTED_REVISION_2),
      ),
      incoming: decodeAssetTransferAvailabilityValue(
        fields.incoming,
        (TEXT_SET_ASSET_TRANSFER_AVAILABILITY + "incoming"),
      ),
      outgoing: decodeAssetTransferAvailabilityValue(
        fields.outgoing,
        (TEXT_SET_ASSET_TRANSFER_AVAILABILITY + "outgoing"),
      ),
      reason,
    },
  };
}

function encodeSetAssetTransferBlacklistInstruction(value) {
  if (!isPlainObject(value)) {
    rejectType(("SetAssetTransferBlacklist" + TEXT_MUST_BE_AN_OBJECT_2));
  }
  const fields = [WIRE_FIELD_ACCOUNT_ID, WIRE_FIELD_ASSET_DEFINITION_ID, "blacklisted"];
  assertOnlyObjectKeys(value, fields, SET_ASSET_TRANSFER_BLACKLIST_VARIANT);
  for (const field of fields) {
    if (!Object.prototype.hasOwnProperty.call(value, field)) {
      rejectType(`${TEXT_SET_ASSET_TRANSFER_BLACKLIST}${field}${TEXT_IS_REQUIRED}`);
    }
  }
  return encodeInstructionEnvelope(
    SET_ASSET_TRANSFER_BLACKLIST_WIRE_ID,
    encodeCanonicalRecordFields(value, TEXT_SET_ASSET_TRANSFER_BLACKLIST_2, SetAssetTransferBlacklistInstructionFields),
  );
}

function decodeSetAssetTransferBlacklistInstructionPayload(payload) {

  return {
    SetAssetTransferBlacklist: decodeRecordFields(payload, SET_ASSET_TRANSFER_BLACKLIST_VARIANT, SetAssetTransferBlacklistInstructionFields),
  };
}

function encodeAssetTransferControlWindowValue(value, context) {
  switch (value) {
    case "Day":
      return encodeEnumTagValue(0);
    case "Week":
      return encodeEnumTagValue(1);
    case "Month":
      return encodeEnumTagValue(2);
    default:
      rejectType(`${context}${TEXT_MUST_BE}exactly "Day", "Week", or "Month"`);
  }
}

function decodeAssetTransferControlWindowValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("tag");
  reader.assertEof();
  switch (tag) {
    case 0:
      return "Day";
    case 1:
      return "Week";
    case 2:
      return "Month";
    default:
      rejectError(`${context}${TEXT_USES_UNSUPPORTED}transfer-control window tag ${tag}`);
  }
}

function encodeAssetTransferLimitValue(value, context) {
  if (!isPlainObject(value)) {
    rejectType(`${context}${TEXT_MUST_BE_AN_OBJECT}`);
  }
  const fields = ["window", "cap_amount"];
  assertOnlyObjectKeys(value, fields, context);
  for (const field of fields) {
    if (!Object.prototype.hasOwnProperty.call(value, field)) {
      rejectType(`${context}.${field}${TEXT_IS_REQUIRED}`);
    }
  }
  return encodeCanonicalRecordFields(value, context, AssetTransferLimitValueFields);
}

const AssetTransferLimitValueFields = [
    ["window", decodeAssetTransferControlWindowValue, 0, encodeAssetTransferControlWindowValue, 0],
    ["cap_amount", decodeQuantityValue, 1, encodeQuantityValue, 1],
  ];

  function decodeAssetTransferLimitValue(payload, context) {
    return decodeRecordFields(payload, context, AssetTransferLimitValueFields);
  }

function encodeSetAssetTransferControlInstruction(value) {
  if (!isPlainObject(value)) {
    rejectType(("SetAssetTransferControl" + TEXT_MUST_BE_AN_OBJECT_2));
  }
  const fields = [WIRE_FIELD_ACCOUNT_ID, WIRE_FIELD_ASSET_DEFINITION_ID, "limits"];
  assertOnlyObjectKeys(value, fields, SET_ASSET_TRANSFER_CONTROL_VARIANT);
  for (const field of fields) {
    if (!Object.prototype.hasOwnProperty.call(value, field)) {
      rejectType(`${TEXT_SET_ASSET_TRANSFER_CONTROL}${field}${TEXT_IS_REQUIRED}`);
    }
  }
  if (!Array.isArray(value.limits)) {
    rejectType((TEXT_SET_ASSET_TRANSFER_CONTROL + "limits" + TEXT_MUST_BE + "an array"));
  }
  for (let index = 0; index < value.limits.length; index += 1) {
    if (!Object.prototype.hasOwnProperty.call(value.limits, index)) {
      rejectType((TEXT_SET_ASSET_TRANSFER_CONTROL + "limits must not contain holes"));
    }
  }
  return encodeInstructionEnvelope(
    SET_ASSET_TRANSFER_CONTROL_WIRE_ID,
    encodeStructValue([
      [
        encodeAccountIdValue(
          value.account_id,
          (TEXT_SET_ASSET_TRANSFER_CONTROL + TEXT_ACCOUNT_ID),
        ),
      ],
      [
        encodeAssetDefinitionIdValue(
          value.asset_definition_id,
          (TEXT_SET_ASSET_TRANSFER_CONTROL + TEXT_ASSET_DEFINITION_ID),
        ),
      ],
      [
        encodeNoritoVec(value.limits, (limit, index) =>
          encodeAssetTransferLimitValue(
            limit,
            `${TEXT_SET_ASSET_TRANSFER_CONTROL}limits[${index}]`,
          ),
        ),
      ],
    ]),
  );
}

function decodeSetAssetTransferControlInstructionPayload(payload) {
  const fields = decodeStructFields(payload, SET_ASSET_TRANSFER_CONTROL_VARIANT, [
    WIRE_FIELD_ACCOUNT_ID,
    WIRE_FIELD_ASSET_DEFINITION_ID,
    "limits",
  ]);
  return {
    SetAssetTransferControl: {
      account_id: decodeAccountIdValue(
        fields.account_id,
        (TEXT_SET_ASSET_TRANSFER_CONTROL + TEXT_ACCOUNT_ID),
      ),
      asset_definition_id: decodeAssetDefinitionIdValue(
        fields.asset_definition_id,
        (TEXT_SET_ASSET_TRANSFER_CONTROL + TEXT_ASSET_DEFINITION_ID),
      ),
      limits: decodeNoritoVec(
        fields.limits,
        (limit, index) =>
          decodeAssetTransferLimitValue(
            limit,
            `${TEXT_SET_ASSET_TRANSFER_CONTROL}limits[${index}]`,
          ),
        (TEXT_SET_ASSET_TRANSFER_CONTROL + "limits"),
      ),
    },
  };
}

function decodeMintPayload(payload) {
  const reader = new BufferReader(payload, "Mint");
  const variantIndex = reader.readU32LE(FIELD_VARIANT_INDEX);
  const body = readNoritoField(reader, "body");
  reader.assertEof();
  switch (variantIndex) {
    case 0:
      return { Asset: decodeAssetInstructionBody(body, "Mint.Asset") };
    case 1:
      return {
        TriggerRepetitions: decodeTriggerRepetitionsBody(body, CONTEXT_MINT_TRIGGER_REPETITIONS),
      };
    default:
      rejectError(`${TEXT_INTERNAL_NORITO_DECODER_DOES_NOT_SUPPORT}Mint variant ${variantIndex}`);
  }
}

function decodeBurnPayload(payload) {
  const reader = new BufferReader(payload, "Burn");
  const variantIndex = reader.readU32LE(FIELD_VARIANT_INDEX);
  const body = readNoritoField(reader, "body");
  reader.assertEof();
  switch (variantIndex) {
    case 0:
      return { Asset: decodeAssetInstructionBody(body, "Burn.Asset") };
    case 1:
      return {
        TriggerRepetitions: decodeTriggerRepetitionsBody(body, CONTEXT_BURN_TRIGGER_REPETITIONS),
      };
    default:
      rejectError(`${TEXT_INTERNAL_NORITO_DECODER_DOES_NOT_SUPPORT}Burn variant ${variantIndex}`);
  }
}

function decodeTransferPayload(payload) {
  const reader = new BufferReader(payload, "Transfer");
  const variantIndex = reader.readU32LE(FIELD_VARIANT_INDEX);
  const body = readNoritoField(reader, "body");
  reader.assertEof();
  switch (variantIndex) {
    case 0:
      return {
        Domain: decodeTransferObjectBody(
          body,
          CONTEXT_TRANSFER_DOMAIN,
          decodeAccountIdValue,
          decodeDomainIdValue,
          decodeAccountIdValue,
        ),
      };
    case 1:
      return {
        AssetDefinition: decodeTransferObjectBody(
          body,
          CONTEXT_TRANSFER_ASSET_DEFINITION,
          decodeAccountIdValue,
          decodeAssetDefinitionIdValue,
          decodeAccountIdValue,
        ),
      };
    case 2:
      return { Asset: decodeTransferAssetBody(body) };
    case 3:
      return {
        Nft: decodeTransferObjectBody(
          body,
          "Transfer.Nft",
          decodeAccountIdValue,
          decodeNftIdValue,
          decodeAccountIdValue,
        ),
      };
    default:
      rejectError(`${TEXT_INTERNAL_NORITO_DECODER_DOES_NOT_SUPPORT}Transfer variant ${variantIndex}.`);
  }
}

function decodeRegisterPayload(payload) {
  const reader = new BufferReader(payload, "Register");
  const variantIndex = reader.readU32LE(FIELD_VARIANT_INDEX);
  const body = readNoritoField(reader, "body");
  reader.assertEof();
  switch (variantIndex) {
    case 1:
      return {
        Domain: decodeNewDomainValue(
          unwrapStructBody(body, CONTEXT_REGISTER_DOMAIN),
          CONTEXT_REGISTER_DOMAIN,
        ),
      };
    case 2:
      return {
        Account: decodeNewAccountValue(
          unwrapStructBody(body, CONTEXT_REGISTER_ACCOUNT),
          CONTEXT_REGISTER_ACCOUNT,
        ),
      };
    case 3:
      return {
        AssetDefinition: decodeNewAssetDefinitionValue(
          unwrapStructBody(body, CONTEXT_REGISTER_ASSET_DEFINITION),
          CONTEXT_REGISTER_ASSET_DEFINITION,
        ),
      };
    default:
      rejectError(`${TEXT_INTERNAL_NORITO_DECODER_DOES_NOT_SUPPORT}Register variant ${variantIndex}.`);
  }
}

function unwrapStructBody(payload, context) {
  const reader = new BufferReader(payload, `${context}.outer`);
  const inner = readNoritoField(reader, "value");
  reader.assertEof();
  return inner;
}

function decodeGovernanceInstructionPayload(wireId, payload) {
  switch (wireId) {
    case PROPOSE_DEPLOY_CONTRACT_WIRE_ID: {
      const fields = decodeStructFields(payload, "ProposeDeployContract", [
        WIRE_FIELD_CONTRACT_ADDRESS,
        WIRE_FIELD_CODE_HASH,
        "abi_hash",
        "abi_version",
        "manifest_provenance",
      ]);
      const decoded = {
        contract_address: decodeStringValue(
          fields.contract_address,
          (TEXT_PROPOSE_DEPLOY_CONTRACT + TEXT_CONTRACT_ADDRESS_2),
        ),
        code_hash: decodeGovernanceHash32Value(
          fields.code_hash,
          (TEXT_PROPOSE_DEPLOY_CONTRACT + TEXT_CODE_HASH_2),
        ),
        abi_hash: decodeGovernanceHash32Value(
          fields.abi_hash,
          (TEXT_PROPOSE_DEPLOY_CONTRACT + "abi_hash"),
        ),
        abi_version: decodeGovernanceAbiVersionValue(
          fields.abi_version,
          (TEXT_PROPOSE_DEPLOY_CONTRACT + "abi_version"),
        ),
      };
      const manifestProvenance = decodeOptionValue(
        fields.manifest_provenance,
        decodeManifestProvenanceValue,
        (TEXT_PROPOSE_DEPLOY_CONTRACT + "manifest_provenance"),
      );
      if (manifestProvenance !== null) {
        decoded.manifest_provenance = manifestProvenance;
      }
      return { ProposeDeployContract: decoded };
    }
    case CAST_ZK_BALLOT_WIRE_ID: {

      return {
        CastZkBallot: decodeRecordFields(payload, "CastZkBallot", CastZkBallotInstructionFields),
      };
    }
    case CAST_PLAIN_BALLOT_WIRE_ID: {

      return {
        CastPlainBallot: decodeRecordFields(payload, "CastPlainBallot", CastPlainBallotInstructionFields),
      };
    }
    default:
      rejectError(`${TEXT_UNSUPPORTED}governance wire id ${wireId}`);
  }
}

function decodeSocialInstructionPayload(wireId, payload) {
  switch (wireId) {
    case CLAIM_TWITTER_FOLLOW_REWARD_WIRE_ID: {

      return {
        ClaimTwitterFollowReward: decodeRecordFields(payload, TEXT_CLAIM_TWITTER_FOLLOW_REWARD, ClaimTwitterFollowRewardInstructionFields),
      };
    }
    case SEND_TO_TWITTER_WIRE_ID: {

      return {
        SendToTwitter: decodeRecordFields(payload, "SendToTwitter", SendToTwitterInstructionFields),
      };
    }
    case CANCEL_TWITTER_ESCROW_WIRE_ID: {

      return {
        CancelTwitterEscrow: decodeRecordFields(payload, "CancelTwitterEscrow", CancelTwitterEscrowInstructionFields),
      };
    }
    default:
      rejectError(`${TEXT_UNSUPPORTED}social wire id ${wireId}`);
  }
}

function decodeSmartContractInstructionPayload(wireId, payload) {
  switch (wireId) {
    case REGISTER_SMART_CONTRACT_CODE_WIRE_ID: {

      return {
        RegisterSmartContractCode: decodeRecordFields(payload, "RegisterSmartContractCode", RegisterSmartContractCodeInstructionFields),
      };
    }
    case REGISTER_SMART_CONTRACT_BYTES_WIRE_ID: {

      return {
        RegisterSmartContractBytes: decodeRecordFields(payload, TEXT_REGISTER_SMART_CONTRACT_BYTES, RegisterSmartContractBytesInstructionFields),
      };
    }
    case DEACTIVATE_CONTRACT_INSTANCE_WIRE_ID: {

      return {
        DeactivateContractInstance: decodeRecordFields(payload, TEXT_DEACTIVATE_CONTRACT_INSTANCE_2, DeactivateContractInstanceInstructionFields),
      };
    }
    case ACTIVATE_CONTRACT_INSTANCE_WIRE_ID: {

      return {
        ActivateContractInstance: decodeRecordFields(payload, TEXT_ACTIVATE_CONTRACT_INSTANCE, ActivateContractInstanceInstructionFields),
      };
    }
    case SET_CONTRACT_PARLIAMENT_DELEGATION_WIRE_ID: {

      return {
        SetContractParliamentDelegation: decodeRecordFields(payload, TEXT_SET_CONTRACT_PARLIAMENT_DELEGATION_2, SetContractParliamentDelegationInstructionFields),
      };
    }
    case OFFER_CONTRACT_OWNERSHIP_WIRE_ID: {

      return {
        OfferContractOwnership: decodeRecordFields(payload, TEXT_OFFER_CONTRACT_OWNERSHIP, OfferContractOwnershipInstructionFields),
      };
    }
    case ACCEPT_CONTRACT_OWNERSHIP_WIRE_ID:
    case CANCEL_CONTRACT_OWNERSHIP_OFFER_WIRE_ID: {
      const name = wireId === ACCEPT_CONTRACT_OWNERSHIP_WIRE_ID
        ? "AcceptContractOwnership"
        : "CancelContractOwnershipOffer";
      const fields = decodeStructFields(payload, name, [
        WIRE_FIELD_CONTRACT_ADDRESS,
        WIRE_FIELD_EXPECTED_REVISION,
      ]);
      return {
        [name]: {
          contract_address: decodeStringValue(
            fields.contract_address,
            `${name}.${TEXT_CONTRACT_ADDRESS_2}`,
          ),
          expected_revision: decodeU64Value(
            fields.expected_revision,
            `${name}.${TEXT_EXPECTED_REVISION_2}`,
          ),
        },
      };
    }
    case COMMIT_CONTRACT_DEPLOYMENT_WIRE_ID: {

      return {
        CommitContractDeployment: decodeRecordFields(payload, "CommitContractDeployment", CommitContractDeploymentInstructionFields),
      };
    }
    case UPLOAD_SMART_CONTRACT_CODE_CHUNK_WIRE_ID: {

      return {
        UploadSmartContractCodeChunk: decodeRecordFields(payload, TEXT_UPLOAD_SMART_CONTRACT_CODE_CHUNK_2, UploadSmartContractCodeChunkInstructionFields),
      };
    }
    case FINALIZE_SMART_CONTRACT_CODE_UPLOAD_WIRE_ID: {

      return {
        FinalizeSmartContractCodeUpload: decodeRecordFields(payload, TEXT_FINALIZE_SMART_CONTRACT_CODE_UPLOAD_2, FinalizeSmartContractCodeUploadInstructionFields),
      };
    }
    case CANCEL_SMART_CONTRACT_CODE_UPLOAD_WIRE_ID: {

      return {
        CancelSmartContractCodeUpload: decodeRecordFields(payload, "CancelSmartContractCodeUpload", CancelSmartContractCodeUploadInstructionFields),
      };
    }
    case REMOVE_SMART_CONTRACT_BYTES_WIRE_ID: {

      return {
        RemoveSmartContractBytes: decodeRecordFields(payload, TEXT_REMOVE_SMART_CONTRACT_BYTES, RemoveSmartContractBytesInstructionFields),
      };
    }
    default:
      rejectError(`${TEXT_UNSUPPORTED}smart-contract wire id ${wireId}`);
  }
}

function decodeKaigiInstructionPayload(wireId, payload) {
  switch (wireId) {
    case CREATE_KAIGI_WIRE_ID: {

      return {
        Kaigi: {
          CreateKaigi: decodeRecordFields(payload, "Kaigi.CreateKaigi", Kaigi_CreateKaigiInstructionFields),
        },
      };
    }
    case JOIN_KAIGI_WIRE_ID:
    case LEAVE_KAIGI_WIRE_ID: {
      const fields = decodeStructFields(payload, `Kaigi.${wireId}`, [
        "call_id",
        "participant",
        FIELD_COMMITMENT,
        "nullifier",
        "roster_root",
        "proof",
      ]);
      const name = wireId.endsWith("JoinKaigi") ? "JoinKaigi" : "LeaveKaigi";
      return {
        Kaigi: {
          [name]: {
            call_id: decodeKaigiIdValue(fields.call_id, `Kaigi.${name}.call_id`),
            participant: decodeAccountIdValue(
              fields.participant,
              `Kaigi.${name}.participant`,
            ),
            commitment: decodeOptionValue(
              fields.commitment,
              decodeKaigiParticipantCommitmentValue,
              `Kaigi.${name}.${TEXT_COMMITMENT}`,
            ),
            nullifier: decodeOptionValue(
              fields.nullifier,
              decodeKaigiParticipantNullifierValue,
              `Kaigi.${name}.nullifier`,
            ),
            roster_root: decodeOptionValue(
              fields.roster_root,
              decodeHashValue,
              `Kaigi.${name}.roster_root`,
            ),
            proof: decodeOptionValue(
              fields.proof,
              decodeByteVecAsBase64,
              `Kaigi.${name}.proof`,
            ),
          },
        },
      };
    }
    case END_KAIGI_WIRE_ID: {

      return {
        Kaigi: {
          EndKaigi: decodeRecordFields(payload, "Kaigi.EndKaigi", Kaigi_EndKaigiInstructionFields),
        },
      };
    }
    case RECORD_KAIGI_USAGE_WIRE_ID: {

      return {
        Kaigi: {
          RecordKaigiUsage: decodeRecordFields(payload, "Kaigi.RecordKaigiUsage", Kaigi_RecordKaigiUsageInstructionFields),
        },
      };
    }
    case SET_KAIGI_RELAY_MANIFEST_WIRE_ID: {

      return {
        Kaigi: {
          SetKaigiRelayManifest: decodeRecordFields(payload, ("Kaigi." + TEXT_SET_KAIGI_RELAY_MANIFEST), Kaigi_SetKaigiRelayManifestInstructionFields),
        },
      };
    }
    case REGISTER_KAIGI_RELAY_WIRE_ID: {

      return {
        Kaigi: {
          RegisterKaigiRelay: decodeRecordFields(payload, "Kaigi.RegisterKaigiRelay", Kaigi_RegisterKaigiRelayInstructionFields),
        },
      };
    }
    case UNREGISTER_KAIGI_RELAY_WIRE_ID: {

      return {
        Kaigi: {
          UnregisterKaigiRelay: decodeRecordFields(payload, "Kaigi.UnregisterKaigiRelay", Kaigi_UnregisterKaigiRelayInstructionFields),
        },
      };
    }
    case REPORT_KAIGI_RELAY_HEALTH_WIRE_ID: {

      return {
        Kaigi: {
          ReportKaigiRelayHealth: decodeRecordFields(payload, ("Kaigi." + TEXT_REPORT_KAIGI_RELAY_HEALTH), Kaigi_ReportKaigiRelayHealthInstructionFields),
        },
      };
    }
    default:
      rejectError(`${TEXT_UNSUPPORTED}Kaigi wire id ${wireId}`);
  }
}

function decodeZkInstructionPayload(wireId, payload) {
  switch (wireId) {
    case REGISTER_ZK_ASSET_WIRE_ID: {

      return {
        zk: {
          RegisterZkAsset: decodeRecordFields(payload, ("zk." + TEXT_REGISTER_ZK_ASSET), zk_RegisterZkAssetInstructionFields),
        },
      };
    }
    case SCHEDULE_CONFIDENTIAL_POLICY_TRANSITION_WIRE_ID: {

      return {
        zk: {
          ScheduleConfidentialPolicyTransition: decodeRecordFields(payload, ("zk." + TEXT_SCHEDULE_CONFIDENTIAL_POLICY_TRANSITION), zk_ScheduleConfidentialPolicyTransitionInstructionFields),
        },
      };
    }
    case CANCEL_CONFIDENTIAL_POLICY_TRANSITION_WIRE_ID: {

      return {
        zk: {
          CancelConfidentialPolicyTransition: decodeRecordFields(payload, ("zk." + TEXT_CANCEL_CONFIDENTIAL_POLICY_TRANSITION), zk_CancelConfidentialPolicyTransitionInstructionFields),
        },
      };
    }
    case CREATE_ELECTION_WIRE_ID: {
      const fields = decodeStructFields(payload, "zk.CreateElection", [
        WIRE_FIELD_ELECTION_ID,
        "options",
        "eligible_root",
        "start_ts",
        "end_ts",
        "vk_ballot",
        "vk_tally",
        "domain_tag",
      ]);
      return {
        zk: {
          CreateElection: {
            election_id: decodeStringValue(fields.election_id, (TEXT_ZK_CREATE_ELECTION + TEXT_ELECTION_ID)),
            options: decodeU32Value(fields.options, (TEXT_ZK_CREATE_ELECTION + "options")),
            eligible_root: Array.from(
              decodeFixedBytesValue(fields.eligible_root, 32, (TEXT_ZK_CREATE_ELECTION + "eligible_root")),
            ),
            start_ts: decodeU64NumberValue(fields.start_ts, (TEXT_ZK_CREATE_ELECTION + "start_ts")),
            end_ts: decodeU64NumberValue(fields.end_ts, (TEXT_ZK_CREATE_ELECTION + "end_ts")),
            vk_ballot: decodeVerifyingKeyIdValue(
              fields.vk_ballot,
              (TEXT_ZK_CREATE_ELECTION + "vk_ballot"),
            ),
            vk_tally: decodeVerifyingKeyIdValue(
              fields.vk_tally,
              (TEXT_ZK_CREATE_ELECTION + "vk_tally"),
            ),
            domain_tag: decodeStringValue(fields.domain_tag, (TEXT_ZK_CREATE_ELECTION + "domain_tag")),
          },
        },
      };
    }
    case SUBMIT_BALLOT_WIRE_ID: {
      const fields = decodeStructFields(payload, "zk.SubmitBallot", [
        WIRE_FIELD_ELECTION_ID,
        "ciphertext",
        "ballot_proof",
        "nullifier",
      ]);
      return {
        zk: {
          SubmitBallot: {
            election_id: decodeStringValue(fields.election_id, (TEXT_ZK_SUBMIT_BALLOT + TEXT_ELECTION_ID)),
            ciphertext: Array.from(
              decodeByteVecValue(fields.ciphertext, (TEXT_ZK_SUBMIT_BALLOT + "ciphertext")),
            ),
            ballot_proof: decodeProofAttachmentValue(
              fields.ballot_proof,
              (TEXT_ZK_SUBMIT_BALLOT + "ballot_proof"),
            ),
            nullifier: Array.from(
              decodeFixedBytesValue(fields.nullifier, 32, (TEXT_ZK_SUBMIT_BALLOT + "nullifier")),
            ),
          },
        },
      };
    }
    case FINALIZE_ELECTION_WIRE_ID: {
      const fields = decodeStructFields(payload, ("zk." + TEXT_FINALIZE_ELECTION), [
        WIRE_FIELD_ELECTION_ID,
        "tally",
        "tally_proof",
      ]);
      return {
        zk: {
          FinalizeElection: {
            election_id: decodeStringValue(
              fields.election_id,
              (TEXT_ZK_FINALIZE_ELECTION + TEXT_ELECTION_ID),
            ),
            tally: decodeNoritoVec(
              fields.tally,
              (entry, index) =>
                decodeU64NumberValue(entry, `${TEXT_ZK_FINALIZE_ELECTION}tally[${index}]`),
              (TEXT_ZK_FINALIZE_ELECTION + "tally"),
            ),
            tally_proof: decodeProofAttachmentValue(
              fields.tally_proof,
              (TEXT_ZK_FINALIZE_ELECTION + "tally_proof"),
            ),
          },
        },
      };
    }
    default:
      rejectError(`${TEXT_UNSUPPORTED}zk wire id ${wireId}`);
  }
}

function decodeRwaInstructionPayload(payload) {
  const reader = new BufferReader(payload, "Rwa");
  const variantIndex = reader.readU32LE(FIELD_VARIANT_INDEX);
  const body = readNoritoField(reader, "body");
  reader.assertEof();
  switch (variantIndex) {
    case 0: {

      return { RegisterRwa: decodeRecordFields(body, "RegisterRwa", RegisterRwaInstructionFields) };
    }
    case 1: {

      return {
        TransferRwa: decodeRecordFields(body, "TransferRwa", TransferRwaInstructionFields),
      };
    }
    case 2: {
      const fields = decodeStructFields(body, "MergeRwas", [
        "parents",
        TEXT_PRIMARY_REFERENCE,
        "status",
        FIELD_METADATA,
      ]);
      return {
        MergeRwas: {
          parents: decodeNoritoVec(
            fields.parents,
            (entry, index) => decodeRwaParentRefValue(entry, `MergeRwas.parents[${index}]`),
            "MergeRwas.parents",
          ),
          primary_reference: decodeStringValue(
            fields.primary_reference,
            ("MergeRwas." + TEXT_PRIMARY_REFERENCE),
          ),
          status: decodeOptionValue(fields.status, decodeNameValue, "MergeRwas.status"),
          metadata: decodeMetadataValue(fields.metadata, "MergeRwas.metadata"),
        },
      };
    }
    case 3:
      return decodeSimpleRwaQuantityInstruction(body, "RedeemRwa");
    case 4:
      return decodeSimpleRwaInstruction(body, "FreezeRwa");
    case 5:
      return decodeSimpleRwaInstruction(body, "UnfreezeRwa");
    case 6:
      return decodeSimpleRwaQuantityInstruction(body, "HoldRwa");
    case 7:
      return decodeSimpleRwaQuantityInstruction(body, "ReleaseRwa");
    case 8: {

      return {
        ForceTransferRwa: decodeRecordFields(body, "ForceTransferRwa", ForceTransferRwaInstructionFields),
      };
    }
    case 9: {

      return {
        SetRwaControls: decodeRecordFields(body, "SetRwaControls", SetRwaControlsInstructionFields),
      };
    }
    case 10: {

      return {
        SetRwaKeyValue: decodeRecordFields(body, "SetRwaKeyValue", SetRwaKeyValueInstructionFields),
      };
    }
    case 11: {

      return {
        RemoveRwaKeyValue: decodeRecordFields(body, "RemoveRwaKeyValue", RemoveRwaKeyValueInstructionFields),
      };
    }
    default:
      rejectError(`${TEXT_INTERNAL_NORITO_DECODER_DOES_NOT_SUPPORT}RWA variant ${variantIndex}`);
  }
}

function decodeSimpleRwaInstruction(payload, name) {
  const fields = decodeStructFields(payload, name, ["rwa"]);
  return {
    [name]: {
      rwa: decodeRwaIdValue(fields.rwa, `${name}.rwa`),
    },
  };
}

function decodeSimpleRwaQuantityInstruction(payload, name) {
  const fields = decodeStructFields(payload, name, ["rwa", FIELD_QUANTITY]);
  return {
    [name]: {
      rwa: decodeRwaIdValue(fields.rwa, `${name}.rwa`),
      quantity: decodeQuantityValue(fields.quantity, `${name}.quantity`),
    },
  };
}

function encodeTransferObjectBody(
  value,
  context,
  encodeSource,
  encodeObject,
  encodeDestination,
) {
  return encodeStructValue([
    [encodeSource(value.source, `${context}.source`)],
    [encodeObject(value.object, `${context}.object`)],
    [encodeDestination(value.destination, `${context}${TEXT_DESTINATION}`)],
  ]);
}

function decodeTransferObjectBody(
  payload,
  context,
  decodeSource,
  decodeObject,
  decodeDestination,
) {
  const fields = decodeStructFields(payload, context, ["source", JS_TYPE_OBJECT, FIELD_DESTINATION]);
  return {
    source: decodeSource(fields.source, `${context}.source`),
    object: decodeObject(fields.object, `${context}.object`),
    destination: decodeDestination(fields.destination, `${context}${TEXT_DESTINATION}`),
  };
}

function encodeStructValue(fields) {
  const parts = [];
  for (const payloads of fields) {
    for (const payload of payloads) {
      parts.push(encodeNoritoField(payload));
    }
  }
  return Buffer.concat(parts);
}

function decodeStructFields(payload, context, names) {
  const reader = new BufferReader(payload, context);
  const result = {};
  for (const name of names) {
    result[name] = readNoritoField(reader, name);
  }
  reader.assertEof();
  return result;
}

function encodeTupleValue(payloads) {
  return encodeStructValue(payloads.map((payload) => [payload]));
}

function decodeTupleFields(payload, context, names) {
  return decodeStructFields(payload, context, names);
}

function encodeOptionValue(value, encode, context) {
  if (value === undefined || value === null) {
    return Buffer.of(0);
  }
  return Buffer.concat([Buffer.of(1), encodeNoritoField(encode(value, context))]);
}

function decodeOptionValue(payload, decode, context) {
  if (payload.length === 0) {
    rejectError(`${context} option payload is empty`);
  }
  const tag = payload[0];
  if (tag === 0) {
    if (payload.length !== 1) {
      rejectError(`${context} None option contained trailing bytes`);
    }
    return null;
  }
  if (tag !== 1) {
    rejectError(`${context} option tag ${tag} is invalid`);
  }
  const reader = new BufferReader(payload.subarray(1), `${context}.some`);
  const inner = readNoritoField(reader, "value");
  reader.assertEof();
  return decode(inner, `${context}.value`);
}

function encodeBoolValue(value, context) {
  if (typeof value !== "boolean") {
    rejectType(`${context}${TEXT_MUST_BE_A}boolean`);
  }
  return Buffer.of(value ? 1 : 0);
}

function decodeBoolValue(payload, context) {
  if (payload.length !== 1 || (payload[0] !== 0 && payload[0] !== 1)) {
    rejectError(`${context}${TEXT_MUST_CONTAIN}a canonical boolean byte`);
  }
  return payload[0] === 1;
}

function encodeFixedBytesValue(value, length, context) {
  const bytes = Buffer.from(normalizeBytes(value));
  if (bytes.length !== length) {
    rejectType(`${context}${TEXT_MUST_CONTAIN_EXACTLY}${length} bytes`);
  }
  return bytes;
}

function decodeFixedBytesValue(payload, length, context) {
  if (payload.length !== length) {
    rejectError(`${context}${TEXT_MUST_CONTAIN_EXACTLY}${length} bytes`);
  }
  return Buffer.from(payload);
}

function encodeFixedByteArrayArchiveValue(value, length, context) {
  const bytes = encodeFixedBytesValue(value, length, context);
  const parts = [];
  for (let index = 0; index < bytes.length; index += 1) {
    parts.push(encodeNoritoField(encodeU8Value(bytes[index], `${context}[${index}]`)));
  }
  return Buffer.concat(parts);
}

function decodeFixedByteArrayArchiveValue(payload, length, context) {
  const reader = new BufferReader(payload, context);
  const out = Buffer.alloc(length);
  for (let index = 0; index < length; index += 1) {
    out[index] = decodeU8Value(
      readNoritoField(reader, `item${index}`),
      `${context}[${index}]`,
    );
  }
  reader.assertEof();
  return out;
}

function encodeByteVecValue(value, _context) {
  const bytes = Buffer.from(normalizeFlexibleBytes(value));
  return Buffer.concat([u64ToLittleEndianBuffer(bytes.length), bytes]);
}

function decodeByteVecValue(payload, context, maxLength = null) {
  const reader = new BufferReader(payload, context);
  const length = bigintToSafeNumber(reader.readU64LE("length"), `${context}.length`);
  if (maxLength !== null && length > maxLength) {
    rejectRange(`${context} exceeds its ${maxLength}-byte decoding limit`);
  }
  const bytes = reader.readBytes(length, "payload");
  reader.assertEof();
  return Buffer.from(bytes);
}

function decodeByteVecAsBase64(payload, context) {
  return decodeByteVecValue(payload, context).toString(BASE64_ENCODING);
}

function normalizeFlexibleBytes(value) {
  if (typeof value === JS_TYPE_STRING) {
    const base64 = tryDecodeBase64(value.trim());
    if (base64) {
      return Array.from(base64);
    }
  }
  return Array.from(normalizeBytes(value));
}

function encodeU64NumberValue(value, context) {
  return encodeU64Value(value, context);
}

function decodeU64NumberValue(payload, context) {
  const value = BigInt(decodeU64Value(payload, context));
  return value <= BigInt(Number.MAX_SAFE_INTEGER) ? Number(value) : value.toString(10);
}

function encodeDomainIdValue(value, context) {
  const literal = assertExactNonEmptyString(value, context);
  if (literal.trim() !== literal) {
    rejectType(`${context}${TEXT_MUST_NOT_CONTAIN}surrounding whitespace`);
  }
  const segments = literal.split(".");
  if (segments.length !== 2 || segments.some((segment) => segment.length === 0)) {
    rejectType(`${context} must use the exact domain.dataspace form`);
  }
  const [name, dataspace] = segments.map((segment) =>
    canonicalizeDomainIdLabel(segment, `${context} label`),
  );
  return encodeStructValue([
    [encodeNameValue(name, `${context}.name`)],
    [encodeNameValue(dataspace, `${context}.dataspace`)],
  ]);
}

function decodeDomainIdValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["name", "dataspace"]);
  return `${decodeNameValue(fields.name, `${context}.name`)}.${decodeNameValue(
    fields.dataspace,
    `${context}.dataspace`,
  )}`;
}

function encodeArchivedDomainIdValue(value, context) {
  return encodeDomainIdValue(value, context);
}

function decodeArchivedDomainIdValue(payload, context) {
  return decodeDomainIdValue(payload, context);
}

function encodeNameValue(value, context) {
  const literal = assertExactNonEmptyString(value, context);
  if (/\p{White_Space}/u.test(literal)) {
    rejectType(`${context}${TEXT_MUST_NOT_CONTAIN}whitespace`);
  }
  if (/[@#$]/u.test(literal)) {
    rejectType(`${context} contains a reserved Name character`);
  }
  return encodeNoritoStringValue(literal.normalize("NFC"));
}

function decodeNameValue(payload, context) {
  const literal = decodeStringValue(payload, context);
  if (literal.length === 0 || /\p{White_Space}/u.test(literal)) {
    rejectType(`${context}${TEXT_MUST_BE_A}non-empty Name without whitespace`);
  }
  if (/[@#$]/u.test(literal)) {
    rejectType(`${context} contains a reserved Name character`);
  }
  return literal.normalize("NFC");
}

function encodeRoleIdValue(value, context) {
  return encodeNoritoField(encodeNameValue(value, `${context}.name`));
}

function decodeRoleIdValue(payload, context) {
  return decodeNestedValue(payload, decodeNameValue, `${context}.name`);
}

function encodeNftIdValue(value, context) {
  const literal = assertExactNonEmptyString(value, context);
  const separator = literal.indexOf("$");
  if (separator <= 0 || separator === literal.length - 1) {
    rejectError(`${context} must use name$domain`);
  }
  const domain = literal.slice(separator + 1);
  return encodeTupleValue([
    encodeDomainIdValue(
      domain.includes(".") ? domain : `${domain}.universal`,
      `${context}.domain`,
    ),
    encodeNameValue(literal.slice(0, separator), `${context}.name`),
  ]);
}

function decodeNftIdValue(payload, context) {
  const fields = decodeTupleFields(payload, context, ["domain", "name"]);
  return `${decodeNameValue(fields.name, `${context}.name`)}$${decodeDomainIdValue(
    fields.domain,
    `${context}.domain`,
  )}`;
}

function encodeRwaIdValue(value, context) {
  const literal = assertExactNonEmptyString(value, context);
  const separator = literal.indexOf("$");
  if (separator <= 0 || separator === literal.length - 1) {
    rejectError(`${context} must use hash$domain`);
  }
  return encodeStructValue([
    [encodeArchivedDomainIdValue(literal.slice(separator + 1), `${context}.domain`)],
    [encodeHashLiteralBytes(literal.slice(0, separator), `${context}.hash`)],
  ]);
}

function decodeRwaIdValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["domain", "hash"]);
  return `${decodeHashLiteral(fields.hash, `${context}.hash`).slice(5, 69).toLowerCase()}$${decodeArchivedDomainIdValue(fields.domain, `${context}.domain`)}`;
}

function encodeCustomInstructionPayload(value) {
  if (!isPlainObject(value)) {
    rejectType(("Custom" + TEXT_MUST_BE_AN_OBJECT_2));
  }
  return encodeStructValue([
    [encodeNoritoField(encodeNoritoJsonValue(value.payload ?? null))],
  ]);
}

function decodeCustomInstructionPayload(payload) {

  return decodeRecordFields(payload, "Custom", CustomInstructionFields);
}

function encodeNewDomainValue(value, context) {
  return encodeStructValue([
    [encodeDomainIdValue(value.id, `${context}.id`)],
    [encodeOptionValue(value.logo, encodeSorafsUriValue, `${context}.logo`)],
    [encodeMetadataValue(value.metadata ?? {}, `${context}.metadata`)],
  ]);
}

const NewDomainValueFields = [
    ["id", decodeDomainIdValue, 0],
    ["logo", decodeSorafsUriValue, 1],
    [FIELD_METADATA, decodeMetadataValue, 0],
  ];

  function decodeNewDomainValue(payload, context) {
    return decodeRecordFields(payload, context, NewDomainValueFields);
  }

function encodeNewAccountValue(value, context) {
  return encodeStructValue([
    [encodeAccountIdValue(value.id, `${context}.id`)],
    [encodeMetadataValue(value.metadata ?? {}, `${context}.metadata`)],
    [encodeOptionValue(value.label ?? null, encodeNoritoStringValue, `${context}.label`)],
    [encodeOptionValue(value.uaid ?? null, encodeNoritoJsonValue, `${context}.uaid`)],
    [encodeNoritoVec(value.opaque_ids ?? [], (entry, index) =>
      encodeNoritoJsonValue(entry, `${context}.opaque_ids[${index}]`),
    )],
  ]);
}

const NewAccountValueFields = [
    ["id", decodeAccountIdValue, 0],
    [FIELD_METADATA, decodeMetadataValue, 0],
    ["label", decodeStringValue, 1],
    ["uaid", decodeJsonValue, 1],
    ["opaque_ids", decodeJsonValue, 2],
  ];

  function decodeNewAccountValue(payload, context) {
    return decodeRecordFields(payload, context, NewAccountValueFields);
  }

function encodeNewAssetDefinitionValue(value, context) {
  if (!isPlainObject(value)) {
    rejectType(`${context}${TEXT_MUST_BE_AN_OBJECT}`);
  }
  const hasOwningDomain = Object.prototype.hasOwnProperty.call(value, "owning_domain");
  const hasCamelOwningDomain = Object.prototype.hasOwnProperty.call(value, "owningDomain");
  if (!hasOwningDomain && !hasCamelOwningDomain) {
    rejectType(`${context}.owning_domain is required; use null for an intentionally unowned global definition`);
  }
  if (
    hasOwningDomain &&
    hasCamelOwningDomain &&
    value.owning_domain !== value.owningDomain
  ) {
    rejectType(`${context} ownership aliases disagree`);
  }
  const owningDomain = hasOwningDomain ? value.owning_domain : value.owningDomain;
  if (owningDomain === undefined) {
    rejectType(`${context}.owning_domain${TEXT_MUST_BE}a domain identifier or null`);
  }
  const hasBalanceScopePolicy = Object.prototype.hasOwnProperty.call(
    value,
    "balance_scope_policy",
  );
  const hasCamelBalanceScopePolicy = Object.prototype.hasOwnProperty.call(
    value,
    "balanceScopePolicy",
  );
  if (!hasBalanceScopePolicy && !hasCamelBalanceScopePolicy) {
    rejectType(`${context}.balance_scope_policy is required`);
  }
  if (
    hasBalanceScopePolicy &&
    hasCamelBalanceScopePolicy &&
    value.balance_scope_policy !== value.balanceScopePolicy
  ) {
    rejectType(`${context} balance-scope policy aliases disagree`);
  }
  const balanceScopePolicy = hasBalanceScopePolicy
    ? value.balance_scope_policy
    : value.balanceScopePolicy;
  if (balanceScopePolicy === WIRE_TYPE_DATASPACE_RESTRICTED && owningDomain === null) {
    rejectType(`${context}.owning_domain is required for DataspaceRestricted balances`);
  }
  if (
    Object.prototype.hasOwnProperty.call(value, "confidential_policy") ||
    Object.prototype.hasOwnProperty.call(value, "confidentialPolicy")
  ) {
    rejectType(`${context} cannot carry confidential policy; use RegisterZkAsset with${TEXT_CANONICAL}verifier bindings`);
  }
  return encodeStructValue([
    [encodeAssetDefinitionIdValue(value.id, `${context}.id`)],
    [encodeStringValue(value.name ?? "", `${context}.name`)],
    [encodeOptionValue(value.description ?? null, encodeStringValue, `${context}.description`)],
    [
      encodeOptionValue(
        value.alias ?? null,
        encodeAssetDefinitionAliasValue,
        `${context}.alias`,
      ),
    ],
    [encodeNumericSpecValue(value.spec ?? { scale: null }, `${context}.spec`)],
    [encodeMintableValue(value.mintable ?? WIRE_TYPE_INFINITELY, `${context}.mintable`)],
    [encodeOptionValue(value.logo ?? null, encodeSorafsUriValue, `${context}.logo`)],
    [encodeMetadataValue(value.metadata ?? {}, `${context}.metadata`)],
    [
      encodeAssetBalancePolicyValue(
        balanceScopePolicy,
        `${context}.balance_scope_policy`,
      ),
    ],
    [encodeOptionValue(owningDomain, encodeDomainIdValue, `${context}.owning_domain`)],
  ]);
}

const NewAssetDefinitionValueFields = [
    ["id", decodeAssetDefinitionIdValue, 0],
    ["name", decodeStringValue, 0],
    ["description", decodeStringValue, 1],
    ["alias", decodeAssetDefinitionAliasValue, 1],
    ["spec", decodeNumericSpecValue, 0],
    ["mintable", decodeMintableValue, 0],
    ["logo", decodeSorafsUriValue, 1],
    [FIELD_METADATA, decodeMetadataValue, 0],
    ["balance_scope_policy", decodeAssetBalancePolicyValue, 0],
    ["owning_domain", decodeDomainIdValue, 1],
  ];

  function decodeNewAssetDefinitionValue(payload, context) {
    return decodeRecordFields(payload, context, NewAssetDefinitionValueFields);
  }

function encodeMetadataValue(value, context) {
  if (!isPlainObject(value)) {
    rejectType(`${context}${TEXT_MUST_BE_AN_OBJECT}`);
  }
  const entries = Object.keys(value)
    .sort()
    .map((key) => [key, value[key]]);
  return encodeNoritoVec(entries, ([key, json]) =>
    encodeTupleValue([
      encodeNameValue(key, `${context}.${key}`),
      encodeNoritoJsonValue(json),
    ]),
  );
}

function decodeMetadataValue(payload, context) {
  const entries = decodeNoritoVec(
    payload,
    (entry, index) => {
      const fields = decodeTupleFields(entry, `${context}[${index}]`, ["key", "value"]);
      return [
        decodeNameValue(fields.key, `${context}[${index}].key`),
        decodeJsonValue(fields.value, `${context}[${index}].value`),
      ];
    },
    context,
  );
  return Object.fromEntries(entries);
}

function decodeNestedJsonValue(payload, context) {
  const reader = new BufferReader(payload, `${context}.outer`);
  const inner = readNoritoField(reader, "value");
  reader.assertEof();
  return decodeJsonValue(inner, context);
}

function decodeNestedValue(payload, decode, context) {
  const reader = new BufferReader(payload, `${context}.outer`);
  const inner = readNoritoField(reader, "value");
  reader.assertEof();
  return decode(inner, context);
}

function decodeCanonicalReplicationId(value, context) {
  if (typeof value !== JS_TYPE_STRING || !/^[0-9a-f]{64}$/u.test(value)) {
    rejectType(`${context}${TEXT_MUST_CONTAIN_EXACTLY}64 lowercase hexadecimal characters`);
  }
  if (/^0{64}$/u.test(value)) {
    rejectType(`${context}${TEXT_MUST_NOT_BE}the zero identifier`);
  }
  return Buffer.from(value, HEX_ENCODING);
}

function encodeReplicationIdValue(value, context) {
  return encodeNoritoField(decodeCanonicalReplicationId(value, context));
}

function decodeReplicationIdValue(payload, context) {
  const bytes = decodeNestedValue(
    payload,
    (inner, innerContext) => decodeFixedBytesValue(inner, 32, innerContext),
    `${context}.value`,
  );
  if (bytes.every((byte) => byte === 0)) {
    rejectType(`${context}${TEXT_MUST_NOT_BE}the zero identifier`);
  }
  return bytes.toString(HEX_ENCODING);
}

function assertExactObjectKeys(value, expectedKeys, context) {
  if (!isPlainObject(value)) {
    rejectType(`${context}${TEXT_MUST_BE_AN_OBJECT}`);
  }
  assertOnlyObjectKeys(value, expectedKeys, context);
  const missing = expectedKeys.find(
    (key) => !Object.prototype.hasOwnProperty.call(value, key),
  );
  if (missing !== undefined) {
    rejectType(`${context} is missing field ${missing}`);
  }
}

function decodeNonzeroFixedBytesHex(payload, context) {
  const bytes = decodeFixedBytesValue(payload, 32, context);
  if (bytes.every((byte) => byte === 0)) {
    rejectType(`${context}${TEXT_MUST_NOT_BE}zero`);
  }
  return bytes.toString(HEX_ENCODING);
}

function encodeExactAccountIdValue(value, context) {
  if (typeof value !== JS_TYPE_STRING || value.trim() !== value) {
    rejectType(`${context}${TEXT_MUST_BE_AN_EXACT_CANONICAL_I105_ACCOUNT_ID}`);
  }
  const canonical = normalizeAccountId(value, context);
  if (canonical !== value) {
    rejectType(`${context}${TEXT_MUST_BE_AN_EXACT_CANONICAL_I105_ACCOUNT_ID}`);
  }
  return encodeAccountIdValue(canonical, context);
}

function encodeProviderIngestCompletionSignerPolicyValue(value, context) {
  assertExactObjectKeys(
    value,
    ["policy_id", "revision", TEXT_PREDECESSOR_DIGEST, "policy_digest"],
    context,
  );
  const revision = normalizeU64Input(value.revision, `${context}.revision`);
  if (revision === 0n) {
    rejectType(`${context}.revision${TEXT_MUST_BE_GREATER_THAN_ZERO}`);
  }
  if (revision === 1n && value.predecessor_digest !== null) {
    rejectType(`${context}${TEXT_PREDECESSOR_DIGEST_MUST_BE_NULL_AT_REVISION_1}`);
  }
  if (revision > 1n && value.predecessor_digest === null) {
    rejectType(`${context}${TEXT_PREDECESSOR_DIGEST_IS_REQUIRED_AFTER_REVISION_1}`);
  }
  const policyId = decodeCanonicalReplicationId(
    value.policy_id,
    `${context}.policy_id`,
  );
  const policyDigest = decodeCanonicalReplicationId(
    value.policy_digest,
    `${context}.policy_digest`,
  );
  return encodeStructValue([
    [policyId],
    [encodeU64Value(revision, `${context}.revision`)],
    [
      encodeOptionValue(
        value.predecessor_digest,
        (entry, innerContext) =>
          encodeFixedByteArrayArchiveValue(
            decodeCanonicalReplicationId(entry, innerContext),
            32,
            innerContext,
          ),
        `${context}.${TEXT_PREDECESSOR_DIGEST}`,
      ),
    ],
    [policyDigest],
  ]);
}

function decodeProviderIngestCompletionSignerPolicyValue(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "policy_id",
    "revision",
    TEXT_PREDECESSOR_DIGEST,
    "policy_digest",
  ]);
  const revision = decodeU64NumberValue(fields.revision, `${context}.revision`);
  if (revision === 0) {
    rejectType(`${context}.revision${TEXT_MUST_BE_GREATER_THAN_ZERO}`);
  }
  const predecessorDigest = decodeOptionValue(
    fields.predecessor_digest,
    (entry, innerContext) => {
      const bytes = decodeFixedByteArrayArchiveValue(entry, 32, innerContext);
      if (bytes.every((byte) => byte === 0)) {
        rejectType(`${innerContext}${TEXT_MUST_NOT_BE}zero`);
      }
      return bytes.toString(HEX_ENCODING);
    },
    `${context}.${TEXT_PREDECESSOR_DIGEST}`,
  );
  if (revision === 1 && predecessorDigest !== null) {
    rejectType(`${context}${TEXT_PREDECESSOR_DIGEST_MUST_BE_NULL_AT_REVISION_1}`);
  }
  if (revision > 1 && predecessorDigest === null) {
    rejectType(`${context}${TEXT_PREDECESSOR_DIGEST_IS_REQUIRED_AFTER_REVISION_1}`);
  }
  return {
    policy_id: decodeNonzeroFixedBytesHex(
      fields.policy_id,
      `${context}.policy_id`,
    ),
    revision,
    predecessor_digest: predecessorDigest,
    policy_digest: decodeNonzeroFixedBytesHex(
      fields.policy_digest,
      `${context}.policy_digest`,
    ),
  };
}

function encodeProviderIngestCompletionAuthorityValue(value, context) {
  assertExactObjectKeys(
    value,
    ["provider_owner", "signer_policy"],
    context,
  );
  return encodeCanonicalRecordFields(value, context, ProviderIngestCompletionAuthorityValueFields);
}

const ProviderIngestCompletionAuthorityValueFields = [
    ["provider_owner", decodeAccountIdValue, 0, encodeExactAccountIdValue, 0],
    ["signer_policy", decodeProviderIngestCompletionSignerPolicyValue, 0, encodeProviderIngestCompletionSignerPolicyValue, 0],
  ];

  function decodeProviderIngestCompletionAuthorityValue(payload, context) {
    return decodeRecordFields(payload, context, ProviderIngestCompletionAuthorityValueFields);
  }

function encodeProviderIngestFinalizedAnchorValue(value, context) {
  assertExactObjectKeys(value, ["height", "block_hash"], context);
  const height = normalizeU64Input(value.height, `${context}.height`);
  if (height === 0n) {
    rejectType(`${context}.height${TEXT_MUST_BE_GREATER_THAN_ZERO}`);
  }
  return encodeStructValue([
    [encodeU64Value(height, `${context}.height`)],
    [
      decodeCanonicalReplicationId(
        value.block_hash,
        `${context}.block_hash`,
      ),
    ],
  ]);
}

function decodeProviderIngestFinalizedAnchorValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["height", "block_hash"]);
  const height = decodeU64NumberValue(fields.height, `${context}.height`);
  if (height === 0) {
    rejectType(`${context}.height${TEXT_MUST_BE_GREATER_THAN_ZERO}`);
  }
  return {
    height,
    block_hash: decodeNonzeroFixedBytesHex(
      fields.block_hash,
      `${context}.block_hash`,
    ),
  };
}

export const validateSorafsReplicationOrderPayloadV1 = /* @__PURE__ */ createNoritoReplicationOrderValidator(
  BASE64_ENCODING,
  FIELD_METADATA,
  HEX_ENCODING,
  ISSUE_ORDER_ID_CONTEXT,
  REPLICATION_ORDER_V1_SCHEMA_HASH,
  SORAFS_REPLICATION_ORDER_CHUNKER_HANDLES_V1,
  SORAFS_REPLICATION_ORDER_MAX_PAYLOAD_BYTES_V1,
  TEXT_CANONICAL,
  TEXT_EXCEEDS_THE,
  TEXT_ISSUE_REPLICATION_ORDER,
  TEXT_MUST_BE,
  TEXT_MUST_BE_GREATER_THAN_ZERO,
  TEXT_MUST_CONTAIN,
  TEXT_MUST_NOT_BE,
  TEXT_REPLICATION_ORDER_V1,
  UTF8_ENCODING,
  WIRE_FIELD_ORDER_ID,
  decodeByteVecValue,
  decodeCanonicalReplicationId,
  decodeFixedBytesValue,
  decodeNonzeroFixedBytesHex,
  decodeNoritoFrame,
  decodeNoritoVec,
  decodeOptionValue,
  decodeStringValue,
  decodeStructFields,
  decodeU16Value,
  decodeU32Value,
  decodeU64Value,
  decodeU8Value,
  frameNoritoPayload,
  normalizeBytes,
  rejectType,
  withNoritoLengthFlags,
);

function encodeReplicationOrderInstruction(instruction) {
  if (isPlainObject(instruction.IssueReplicationOrder)) {
    assertOnlyObjectKeys(instruction, [WIRE_TYPE_ISSUE_REPLICATION_ORDER], FIELD_INSTRUCTION);
    const value = instruction.IssueReplicationOrder;
    assertExactObjectKeys(
      value,
      [
        WIRE_FIELD_ORDER_ID,
        "order_payload",
        "issued_epoch",
        "deadline_epoch",
        "musubi_archive",
      ],
      WIRE_TYPE_ISSUE_REPLICATION_ORDER,
    );
    const orderPayload = decodeExactStandardBase64(
      value.order_payload,
      ISSUE_ORDER_PAYLOAD_CONTEXT,
    );
    validateSorafsReplicationOrderPayloadV1(orderPayload, value.order_id);
    const issuedEpoch = normalizeU64Input(
      value.issued_epoch,
      ISSUE_ORDER_EPOCH_CONTEXT,
    );
    const deadlineEpoch = normalizeU64Input(
      value.deadline_epoch,
      ISSUE_ORDER_DEADLINE_CONTEXT,
    );
    if (deadlineEpoch <= issuedEpoch) {
      rejectType(ISSUE_ORDER_DEADLINE_MESSAGE);
    }
    return encodeInstructionEnvelope(
      ISSUE_REPLICATION_ORDER_WIRE_ID,
      encodeStructValue([
        [
          encodeReplicationIdValue(
            value.order_id,
            ISSUE_ORDER_ID_CONTEXT,
          ),
        ],
        [encodeByteVecValue(orderPayload, ISSUE_ORDER_PAYLOAD_CONTEXT)],
        [encodeU64Value(issuedEpoch, ISSUE_ORDER_EPOCH_CONTEXT)],
        [encodeU64Value(deadlineEpoch, ISSUE_ORDER_DEADLINE_CONTEXT)],
        [
          encodeOptionValue(
            value.musubi_archive,
            encodeReplicationIdValue,
            (TEXT_ISSUE_REPLICATION_ORDER + "musubi_archive"),
          ),
        ],
      ]),
    );
  }
  if (isPlainObject(instruction.CompleteReplicationOrder)) {
    assertExactObjectKeys(
      instruction,
      [WIRE_TYPE_COMPLETE_REPLICATION_ORDER],
      FIELD_INSTRUCTION,
    );
    const value = instruction.CompleteReplicationOrder;
    assertExactObjectKeys(
      value,
      [
        WIRE_FIELD_ORDER_ID,
        "provider_id",
        TEXT_COMPLETION_EPOCH,
        TEXT_EXPECTED_AUTHORITY,
        TEXT_EXPECTED_ASSIGNMENT_REVISION,
        TEXT_FINALIZED_ANCHOR,
      ],
      WIRE_TYPE_COMPLETE_REPLICATION_ORDER,
    );
    const expectedAssignmentRevision = normalizeU64Input(
      value.expected_assignment_revision,
      COMPLETE_ORDER_REVISION_CONTEXT,
    );
    if (expectedAssignmentRevision === 0n) {
      rejectType(COMPLETE_ORDER_REVISION_MESSAGE);
    }
    return encodeInstructionEnvelope(
      COMPLETE_REPLICATION_ORDER_WIRE_ID,
      encodeStructValue([
        [
          encodeReplicationIdValue(
            value.order_id,
            (TEXT_COMPLETE_REPLICATION_ORDER + "order_id"),
          ),
        ],
        [
          encodeReplicationIdValue(
            value.provider_id,
            (TEXT_COMPLETE_REPLICATION_ORDER + "provider_id"),
          ),
        ],
        [encodeU64Value(
          value.completion_epoch,
          (TEXT_COMPLETE_REPLICATION_ORDER + TEXT_COMPLETION_EPOCH),
        )],
        [
          encodeProviderIngestCompletionAuthorityValue(
            value.expected_authority,
            (TEXT_COMPLETE_REPLICATION_ORDER + TEXT_EXPECTED_AUTHORITY),
          ),
        ],
        [
          encodeU64Value(
            expectedAssignmentRevision,
            COMPLETE_ORDER_REVISION_CONTEXT,
          ),
        ],
        [
          encodeProviderIngestFinalizedAnchorValue(
            value.finalized_anchor,
            (TEXT_COMPLETE_REPLICATION_ORDER + TEXT_FINALIZED_ANCHOR),
          ),
        ],
      ]),
    );
  }
  if (isPlainObject(instruction.ExpireReplicationOrder)) {
    assertOnlyObjectKeys(instruction, [WIRE_TYPE_EXPIRE_REPLICATION_ORDER], FIELD_INSTRUCTION);
    const value = instruction.ExpireReplicationOrder;
    assertOnlyObjectKeys(
      value,
      [WIRE_FIELD_ORDER_ID, TEXT_EXPIRATION_EPOCH],
      WIRE_TYPE_EXPIRE_REPLICATION_ORDER,
    );
    return encodeInstructionEnvelope(
      EXPIRE_REPLICATION_ORDER_WIRE_ID,
      encodeCanonicalRecordFields(value, TEXT_EXPIRE_REPLICATION_ORDER_2, CanonicalEncodingFields3),
    );
  }
  rejectType((TEXT_UNSUPPORTED + "SoraFS replication-order instruction"));
}

function decodeReplicationOrderInstructionPayload(wireId, payload) {
  if (wireId === ISSUE_REPLICATION_ORDER_WIRE_ID) {
    const fields = decodeStructFields(payload, WIRE_TYPE_ISSUE_REPLICATION_ORDER, [
      WIRE_FIELD_ORDER_ID,
      "order_payload",
      "issued_epoch",
      "deadline_epoch",
      "musubi_archive",
    ]);
    const orderId = decodeReplicationIdValue(
      fields.order_id,
      ISSUE_ORDER_ID_CONTEXT,
    );
    const orderPayload = decodeByteVecValue(
      fields.order_payload,
      ISSUE_ORDER_PAYLOAD_CONTEXT,
    );
    validateSorafsReplicationOrderPayloadV1(orderPayload, orderId);
    const issuedEpoch = decodeU64NumberValue(
      fields.issued_epoch,
      ISSUE_ORDER_EPOCH_CONTEXT,
    );
    const deadlineEpoch = decodeU64NumberValue(
      fields.deadline_epoch,
      ISSUE_ORDER_DEADLINE_CONTEXT,
    );
    const musubiArchive = decodeOptionValue(
      fields.musubi_archive,
      decodeReplicationIdValue,
      (TEXT_ISSUE_REPLICATION_ORDER + "musubi_archive"),
    );
    if (deadlineEpoch <= issuedEpoch) {
      rejectType(ISSUE_ORDER_DEADLINE_MESSAGE);
    }
    return {
      IssueReplicationOrder: {
        order_id: orderId,
        order_payload: orderPayload.toString(BASE64_ENCODING),
        issued_epoch: issuedEpoch,
        deadline_epoch: deadlineEpoch,
        musubi_archive: musubiArchive,
      },
    };
  }
  if (wireId === COMPLETE_REPLICATION_ORDER_WIRE_ID) {
    const fields = decodeStructFields(payload, WIRE_TYPE_COMPLETE_REPLICATION_ORDER, [
      WIRE_FIELD_ORDER_ID,
      "provider_id",
      TEXT_COMPLETION_EPOCH,
      TEXT_EXPECTED_AUTHORITY,
      TEXT_EXPECTED_ASSIGNMENT_REVISION,
      TEXT_FINALIZED_ANCHOR,
    ]);
    const expectedAssignmentRevision = decodeU64NumberValue(
      fields.expected_assignment_revision,
      COMPLETE_ORDER_REVISION_CONTEXT,
    );
    if (expectedAssignmentRevision === 0) {
      rejectType(COMPLETE_ORDER_REVISION_MESSAGE);
    }
    return {
      CompleteReplicationOrder: {
        order_id: decodeReplicationIdValue(
          fields.order_id,
          (TEXT_COMPLETE_REPLICATION_ORDER + "order_id"),
        ),
        provider_id: decodeReplicationIdValue(
          fields.provider_id,
          (TEXT_COMPLETE_REPLICATION_ORDER + "provider_id"),
        ),
        completion_epoch: decodeU64NumberValue(
          fields.completion_epoch,
          (TEXT_COMPLETE_REPLICATION_ORDER + TEXT_COMPLETION_EPOCH),
        ),
        expected_authority: decodeProviderIngestCompletionAuthorityValue(
          fields.expected_authority,
          (TEXT_COMPLETE_REPLICATION_ORDER + TEXT_EXPECTED_AUTHORITY),
        ),
        expected_assignment_revision: expectedAssignmentRevision,
        finalized_anchor: decodeProviderIngestFinalizedAnchorValue(
          fields.finalized_anchor,
          (TEXT_COMPLETE_REPLICATION_ORDER + TEXT_FINALIZED_ANCHOR),
        ),
      },
    };
  }
  const fields = decodeStructFields(payload, WIRE_TYPE_EXPIRE_REPLICATION_ORDER, [
    WIRE_FIELD_ORDER_ID,
    TEXT_EXPIRATION_EPOCH,
  ]);
  return {
    ExpireReplicationOrder: {
      order_id: decodeReplicationIdValue(
        fields.order_id,
        (TEXT_EXPIRE_REPLICATION_ORDER + "order_id"),
      ),
      expiration_epoch: decodeU64NumberValue(
        fields.expiration_epoch,
        (TEXT_EXPIRE_REPLICATION_ORDER + TEXT_EXPIRATION_EPOCH),
      ),
    },
  };
}

function encodeGovernanceInstruction(instruction) {
  if (isPlainObject(instruction.ProposeDeployContract)) {
    return encodeInstructionEnvelope(
      PROPOSE_DEPLOY_CONTRACT_WIRE_ID,
      encodeProposeDeployContractPayload(instruction.ProposeDeployContract),
    );
  }
  if (isPlainObject(instruction.CastZkBallot)) {
    return encodeInstructionEnvelope(
      CAST_ZK_BALLOT_WIRE_ID,
      encodeCastZkBallotPayload(instruction.CastZkBallot),
    );
  }
  if (isPlainObject(instruction.CastPlainBallot)) {
    return encodeInstructionEnvelope(
      CAST_PLAIN_BALLOT_WIRE_ID,
      encodeCastPlainBallotPayload(instruction.CastPlainBallot),
    );
  }
  rejectError(`${TEXT_INTERNAL_NORITO_CANONICALIZATION_DOES_NOT_SUPPORT}governance ${TEXT_INSTRUCTION}${describeInstructionShape(instruction)}`);
}

function encodeSocialInstruction(instruction) {
  if (isPlainObject(instruction.ClaimTwitterFollowReward)) {
    return encodeInstructionEnvelope(
      CLAIM_TWITTER_FOLLOW_REWARD_WIRE_ID,
      encodeStructValue([
        [encodeKeyedHashValue(
          instruction.ClaimTwitterFollowReward.binding_hash,
          "ClaimTwitterFollowReward.binding_hash",
        )],
      ]),
    );
  }
  if (isPlainObject(instruction.SendToTwitter)) {
    return encodeInstructionEnvelope(
      SEND_TO_TWITTER_WIRE_ID,
      encodeCanonicalRecordFields(instruction, "SendToTwitter", SendToTwitterInstructionFields, "SendToTwitter"),
    );
  }
  if (isPlainObject(instruction.CancelTwitterEscrow)) {
    return encodeInstructionEnvelope(
      CANCEL_TWITTER_ESCROW_WIRE_ID,
      encodeStructValue([
        [encodeKeyedHashValue(
          instruction.CancelTwitterEscrow.binding_hash,
          "CancelTwitterEscrow.binding_hash",
        )],
      ]),
    );
  }
  rejectError(`${TEXT_INTERNAL_NORITO_CANONICALIZATION_DOES_NOT_SUPPORT}social ${TEXT_INSTRUCTION}${describeInstructionShape(instruction)}`);
}

function encodeContractLifecycleOwnerValue(value, context) {
  assertExactObjectKeys(value, ["owner", "value"], context);
  if (value.owner === "Account") {
    return encodeEnumTagValue(0, () =>
      encodeAccountIdValue(value.value, `${context}.value`));
  }
  if (value.owner === "Parliament") {
    if (value.value !== null) {
      rejectType(`${context}.value${TEXT_MUST_BE}null for Parliament`);
    }
    return encodeEnumTagValue(1);
  }
  rejectType(`${context}.owner${TEXT_MUST_BE}Account or Parliament`);
}

function decodeContractLifecycleOwnerValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("owner");
  if (tag === 0) {
    const account = decodeAccountIdValue(
      readNoritoField(reader, "value"),
      `${context}.value`,
    );
    reader.assertEof();
    return { owner: "Account", value: account };
  }
  if (tag === 1) {
    reader.assertEof();
    return { owner: "Parliament", value: null };
  }
  rejectType(`${context}.owner contains ${TEXT_UNSUPPORTED}variant ${tag}`);
}

function encodeSmartContractInstruction(instruction) {
  return withNoritoCompactLengths(() =>
    encodeSmartContractInstructionCompact(instruction),
  );
}

function encodeSmartContractInstructionCompact(instruction) {
  if (isPlainObject(instruction.RegisterSmartContractCode)) {
    return encodeInstructionEnvelope(
      REGISTER_SMART_CONTRACT_CODE_WIRE_ID,
      encodeStructValue([
        [encodeContractManifestValue(
          instruction.RegisterSmartContractCode.manifest,
          "RegisterSmartContractCode.manifest",
        )],
      ]),
    );
  }
  if (isPlainObject(instruction.RegisterSmartContractBytes)) {
    return encodeInstructionEnvelope(
      REGISTER_SMART_CONTRACT_BYTES_WIRE_ID,
      encodeCanonicalRecordFields(instruction, TEXT_REGISTER_SMART_CONTRACT_BYTES, RegisterSmartContractBytesInstructionFields, TEXT_REGISTER_SMART_CONTRACT_BYTES),
    );
  }
  if (isPlainObject(instruction.DeactivateContractInstance)) {
    return encodeInstructionEnvelope(
      DEACTIVATE_CONTRACT_INSTANCE_WIRE_ID,
      encodeCanonicalRecordFields(instruction, TEXT_DEACTIVATE_CONTRACT_INSTANCE_2, DeactivateContractInstanceInstructionFields, TEXT_DEACTIVATE_CONTRACT_INSTANCE_2),
    );
  }
  if (isPlainObject(instruction.ActivateContractInstance)) {
    return encodeInstructionEnvelope(
      ACTIVATE_CONTRACT_INSTANCE_WIRE_ID,
      encodeCanonicalRecordFields(instruction, TEXT_ACTIVATE_CONTRACT_INSTANCE, ActivateContractInstanceInstructionFields, TEXT_ACTIVATE_CONTRACT_INSTANCE),
    );
  }
  if (isPlainObject(instruction.SetContractParliamentDelegation)) {
    return encodeInstructionEnvelope(
      SET_CONTRACT_PARLIAMENT_DELEGATION_WIRE_ID,
      encodeCanonicalRecordFields(instruction, TEXT_SET_CONTRACT_PARLIAMENT_DELEGATION_2, SetContractParliamentDelegationInstructionFields, TEXT_SET_CONTRACT_PARLIAMENT_DELEGATION_2),
    );
  }
  if (isPlainObject(instruction.OfferContractOwnership)) {
    return encodeInstructionEnvelope(
      OFFER_CONTRACT_OWNERSHIP_WIRE_ID,
      encodeCanonicalRecordFields(instruction, TEXT_OFFER_CONTRACT_OWNERSHIP, OfferContractOwnershipInstructionFields, TEXT_OFFER_CONTRACT_OWNERSHIP),
    );
  }
  for (const [name, wireId] of [
    ["AcceptContractOwnership", ACCEPT_CONTRACT_OWNERSHIP_WIRE_ID],
    ["CancelContractOwnershipOffer", CANCEL_CONTRACT_OWNERSHIP_OFFER_WIRE_ID],
  ]) {
    if (isPlainObject(instruction[name])) {
      return encodeInstructionEnvelope(
        wireId,
        encodeStructValue([
          [encodeNoritoStringValue(assertNonEmptyString(
            instruction[name].contract_address,
            `${name}.${TEXT_CONTRACT_ADDRESS_2}`,
          ))],
          [encodeU64Value(
            instruction[name].expected_revision,
            `${name}.${TEXT_EXPECTED_REVISION_2}`,
          )],
        ]),
      );
    }
  }
  if (isPlainObject(instruction.CommitContractDeployment)) {
    return encodeInstructionEnvelope(
      COMMIT_CONTRACT_DEPLOYMENT_WIRE_ID,
      encodeStructValue([
        [encodeU64Value(
          instruction.CommitContractDeployment.expected_deploy_nonce,
          (TEXT_COMMIT_CONTRACT_DEPLOYMENT + "expected_deploy_nonce"),
        )],
        [encodeNoritoStringValue(assertNonEmptyString(
          instruction.CommitContractDeployment.contract_address,
          (TEXT_COMMIT_CONTRACT_DEPLOYMENT + TEXT_CONTRACT_ADDRESS_2),
        ))],
        [encodeHashValue(
          instruction.CommitContractDeployment.code_hash,
          (TEXT_COMMIT_CONTRACT_DEPLOYMENT + TEXT_CODE_HASH_2),
        )],
        [encodeNoritoStringValue(assertNonEmptyString(
          instruction.CommitContractDeployment.contract_alias,
          (TEXT_COMMIT_CONTRACT_DEPLOYMENT + "contract_alias"),
        ))],
        [encodeOptionValue(
          instruction.CommitContractDeployment.lease_expiry_ms,
          encodeU64Value,
          (TEXT_COMMIT_CONTRACT_DEPLOYMENT + "lease_expiry_ms"),
        )],
        [encodeOptionValue(
          instruction.CommitContractDeployment.expected_previous_contract_address,
          encodeNoritoStringValue,
          EXPECTED_PREVIOUS_CONTRACT_CONTEXT,
        )],
      ]),
    );
  }
  if (isPlainObject(instruction.UploadSmartContractCodeChunk)) {
    return encodeInstructionEnvelope(
      UPLOAD_SMART_CONTRACT_CODE_CHUNK_WIRE_ID,
      encodeCanonicalRecordFields(instruction, TEXT_UPLOAD_SMART_CONTRACT_CODE_CHUNK_2, UploadSmartContractCodeChunkInstructionFields, TEXT_UPLOAD_SMART_CONTRACT_CODE_CHUNK_2),
    );
  }
  if (isPlainObject(instruction.FinalizeSmartContractCodeUpload)) {
    return encodeInstructionEnvelope(
      FINALIZE_SMART_CONTRACT_CODE_UPLOAD_WIRE_ID,
      encodeCanonicalRecordFields(instruction, TEXT_FINALIZE_SMART_CONTRACT_CODE_UPLOAD_2, FinalizeSmartContractCodeUploadInstructionFields, TEXT_FINALIZE_SMART_CONTRACT_CODE_UPLOAD_2),
    );
  }
  if (isPlainObject(instruction.CancelSmartContractCodeUpload)) {
    return encodeInstructionEnvelope(
      CANCEL_SMART_CONTRACT_CODE_UPLOAD_WIRE_ID,
      encodeStructValue([
        [encodeHashValue(
          instruction.CancelSmartContractCodeUpload.code_hash,
          ("CancelSmartContractCodeUpload." + TEXT_CODE_HASH_2),
        )],
      ]),
    );
  }
  if (isPlainObject(instruction.RemoveSmartContractBytes)) {
    return encodeInstructionEnvelope(
      REMOVE_SMART_CONTRACT_BYTES_WIRE_ID,
      encodeCanonicalRecordFields(instruction, TEXT_REMOVE_SMART_CONTRACT_BYTES, RemoveSmartContractBytesInstructionFields, TEXT_REMOVE_SMART_CONTRACT_BYTES),
    );
  }
  rejectError(`${TEXT_INTERNAL_NORITO_CANONICALIZATION_DOES_NOT_SUPPORT}smart-contract ${TEXT_INSTRUCTION}${describeInstructionShape(instruction)}`);
}

const GOVERNANCE_HASH32_WIRE_VERSION_V1 = 1;
const GOVERNANCE_HASH32_LENGTH = 32;

function encodeGovernanceHash32Value(value, context) {
  const bytes = Buffer.from(assertExactNonEmptyString(value, context), HEX_ENCODING);
  if (
    value.length !== GOVERNANCE_HASH32_LENGTH * 2 ||
    value !== value.toLowerCase() ||
    !/^[0-9a-f]{64}$/u.test(value)
  ) {
    rejectType(`${context}${TEXT_MUST_BE}exactly 32 bytes of lowercase hexadecimal`);
  }
  return encodeStructValue([
    [encodeU16Value(GOVERNANCE_HASH32_WIRE_VERSION_V1, `${context}.version`)],
    [encodeU16Value(GOVERNANCE_HASH32_LENGTH, `${context}.declared_len`)],
    [encodeFixedBytesValue(bytes, GOVERNANCE_HASH32_LENGTH, `${context}.bytes`)],
  ]);
}

function decodeGovernanceHash32Value(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "version",
    "declared_len",
    "bytes",
  ]);
  const version = decodeU16Value(fields.version, `${context}.version`);
  if (version !== GOVERNANCE_HASH32_WIRE_VERSION_V1) {
    rejectError(`${context}.version${TEXT_MUST_BE}${GOVERNANCE_HASH32_WIRE_VERSION_V1}`);
  }
  const declaredLength = decodeU16Value(fields.declared_len, `${context}.declared_len`);
  if (declaredLength !== GOVERNANCE_HASH32_LENGTH) {
    rejectError(`${context}.declared_len${TEXT_MUST_BE}${GOVERNANCE_HASH32_LENGTH}`);
  }
  return decodeFixedBytesValue(
    fields.bytes,
    GOVERNANCE_HASH32_LENGTH,
    `${context}.bytes`,
  ).toString(HEX_ENCODING);
}

function encodeGovernanceAbiVersionValue(value, context) {
  return encodeStructValue([[encodeU16Value(value, `${context}.value`)]]);
}

function decodeGovernanceAbiVersionValue(payload, context) {
  const fields = decodeTupleFields(payload, context, ["value"]);
  return decodeU16Value(fields.value, `${context}.value`);
}

function encodeProposeDeployContractPayload(value) {
  validateProposeDeployContractPayload(value);
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.contract_address, (TEXT_PROPOSE_DEPLOY_CONTRACT + TEXT_CONTRACT_ADDRESS_2)))],
    [encodeGovernanceHash32Value(value.code_hash, (TEXT_PROPOSE_DEPLOY_CONTRACT + TEXT_CODE_HASH_2))],
    [encodeGovernanceHash32Value(value.abi_hash, (TEXT_PROPOSE_DEPLOY_CONTRACT + "abi_hash"))],
    [encodeGovernanceAbiVersionValue(value.abi_version, (TEXT_PROPOSE_DEPLOY_CONTRACT + "abi_version"))],
    [encodeOptionValue(
      value.manifest_provenance ?? null,
      encodeManifestProvenanceValue,
      (TEXT_PROPOSE_DEPLOY_CONTRACT + "manifest_provenance"),
    )],
  ]);
}

function encodeCastZkBallotPayload(value) {
  validateCastZkBallotPayload(value);
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.election_id, ("CastZkBallot." + TEXT_ELECTION_ID)))],
    [encodeExactBase64StringValue(value.proof_b64, "CastZkBallot.proof_b64")],
    [encodeNoritoStringValue(
      assertNonEmptyString(value.public_inputs_json ?? "{}", "CastZkBallot.public_inputs_json"),
    )],
  ]);
}

function encodeCastPlainBallotPayload(value) {
  return encodeStructValue([
    [encodeNoritoStringValue(assertCanonicalGovernanceSelectorV1(
      value.referendum_id,
      "CastPlainBallot.referendum_id",
    ))],
    [encodeAccountIdValue(value.owner, "CastPlainBallot.owner")],
    [encodeQuantityValue(value.amount, "CastPlainBallot.amount")],
    [encodeU64NumberValue(value.duration_blocks, "CastPlainBallot.duration_blocks")],
    [encodeU8Value(value.direction, "CastPlainBallot.direction")],
  ]);
}

function encodeKaigiInstruction(instruction) {
  if (isPlainObject(instruction.CreateKaigi)) {
    return encodeInstructionEnvelope(
      CREATE_KAIGI_WIRE_ID,
      encodeCreateKaigiPayload(instruction.CreateKaigi),
    );
  }
  if (isPlainObject(instruction.JoinKaigi)) {
    return encodeInstructionEnvelope(
      JOIN_KAIGI_WIRE_ID,
      encodeJoinLeaveKaigiPayload(instruction.JoinKaigi, "JoinKaigi"),
    );
  }
  if (isPlainObject(instruction.LeaveKaigi)) {
    return encodeInstructionEnvelope(
      LEAVE_KAIGI_WIRE_ID,
      encodeJoinLeaveKaigiPayload(instruction.LeaveKaigi, "LeaveKaigi"),
    );
  }
  if (isPlainObject(instruction.EndKaigi)) {
    return encodeInstructionEnvelope(
      END_KAIGI_WIRE_ID,
      encodeEndKaigiPayload(instruction.EndKaigi),
    );
  }
  if (isPlainObject(instruction.RecordKaigiUsage)) {
    return encodeInstructionEnvelope(
      RECORD_KAIGI_USAGE_WIRE_ID,
      encodeRecordKaigiUsagePayload(instruction.RecordKaigiUsage),
    );
  }
  if (isPlainObject(instruction.SetKaigiRelayManifest)) {
    return encodeInstructionEnvelope(
      SET_KAIGI_RELAY_MANIFEST_WIRE_ID,
      encodeSetKaigiRelayManifestPayload(instruction.SetKaigiRelayManifest),
    );
  }
  if (isPlainObject(instruction.RegisterKaigiRelay)) {
    return encodeInstructionEnvelope(
      REGISTER_KAIGI_RELAY_WIRE_ID,
      encodeRegisterKaigiRelayPayload(instruction.RegisterKaigiRelay),
    );
  }
  if (isPlainObject(instruction.UnregisterKaigiRelay)) {
    return encodeInstructionEnvelope(
      UNREGISTER_KAIGI_RELAY_WIRE_ID,
      encodeUnregisterKaigiRelayPayload(instruction.UnregisterKaigiRelay),
    );
  }
  if (isPlainObject(instruction.ReportKaigiRelayHealth)) {
    return encodeInstructionEnvelope(
      REPORT_KAIGI_RELAY_HEALTH_WIRE_ID,
      encodeReportKaigiRelayHealthPayload(instruction.ReportKaigiRelayHealth),
    );
  }
  rejectError(`${TEXT_INTERNAL_NORITO_CANONICALIZATION_DOES_NOT_SUPPORT}Kaigi ${TEXT_INSTRUCTION}${describeInstructionShape(instruction)}`);
}

function encodeCreateKaigiPayload(value) {
  return encodeCanonicalRecordFields(value, "Kaigi.CreateKaigi", Kaigi_CreateKaigiInstructionFields);
}

function encodeJoinLeaveKaigiPayload(value, name) {
  return encodeStructValue([
    [encodeKaigiIdValue(value.call_id, `Kaigi.${name}.call_id`)],
    [encodeAccountIdValue(value.participant, `Kaigi.${name}.participant`)],
    [encodeOptionValue(value.commitment, encodeKaigiParticipantCommitmentValue, `Kaigi.${name}.${TEXT_COMMITMENT}`)],
    [encodeOptionValue(value.nullifier, encodeKaigiParticipantNullifierValue, `Kaigi.${name}.nullifier`)],
    [encodeOptionValue(value.roster_root, encodeHashValue, `Kaigi.${name}.roster_root`)],
    [encodeOptionValue(value.proof, encodeByteVecValue, `Kaigi.${name}.proof`)],
  ]);
}

function encodeEndKaigiPayload(value) {
  return encodeCanonicalRecordFields(value, "Kaigi.EndKaigi", Kaigi_EndKaigiInstructionFields);
}

function encodeRecordKaigiUsagePayload(value) {
  return encodeCanonicalRecordFields(value, "Kaigi.RecordKaigiUsage", Kaigi_RecordKaigiUsageInstructionFields);
}

function encodeSetKaigiRelayManifestPayload(value) {
  return encodeCanonicalRecordFields(value, ("Kaigi." + TEXT_SET_KAIGI_RELAY_MANIFEST), Kaigi_SetKaigiRelayManifestInstructionFields);
}

function encodeRegisterKaigiRelayPayload(value) {
  return encodeStructValue([
    [encodeKaigiRelayRegistrationValue(value.relay, "Kaigi.RegisterKaigiRelay.relay")],
  ]);
}

function encodeUnregisterKaigiRelayPayload(value) {
  return encodeStructValue([
    [encodeAccountIdValue(value.relay_id, "Kaigi.UnregisterKaigiRelay.relay_id")],
  ]);
}

function encodeReportKaigiRelayHealthPayload(value) {
  return encodeCanonicalRecordFields(value, ("Kaigi." + TEXT_REPORT_KAIGI_RELAY_HEALTH), Kaigi_ReportKaigiRelayHealthInstructionFields);
}

function encodeVerifyingKeyInstruction(instruction) {
  const entries = [
    [
      TEXT_REGISTER_VERIFYING_KEY,
      REGISTER_VERIFYING_KEY_WIRE_ID,
      encodeVerifyingKeyInstructionPayload,
    ],
    [
      TEXT_UPDATE_VERIFYING_KEY,
      UPDATE_VERIFYING_KEY_WIRE_ID,
      encodeVerifyingKeyInstructionPayload,
    ],
  ];
  for (const [key, wireId, encode] of entries) {
    if (isPlainObject(instruction[key])) {
      return encodeInstructionEnvelope(
        wireId,
        encode(instruction[key], `${TEXT_VERIFYING_KEYS}${key}`),
      );
    }
  }
  rejectError(`${TEXT_INTERNAL_NORITO_CANONICALIZATION_DOES_NOT_SUPPORT}verifying-key ${TEXT_INSTRUCTION}${describeInstructionShape(instruction)}`);
}

function encodeVerifyingKeyInstructionPayload(value, context) {
  return encodeCanonicalRecordFields(value, context, CanonicalEncodingFields18);
}

function decodeVerifyingKeyInstructionPayload(wireId, payload) {
  const variant =
    wireId === REGISTER_VERIFYING_KEY_WIRE_ID
      ? TEXT_REGISTER_VERIFYING_KEY
      : TEXT_UPDATE_VERIFYING_KEY;
  const fields = decodeStructFields(payload, `${TEXT_VERIFYING_KEYS}${variant}`, [
    "id",
    "record",
  ]);
  return {
    verifying_keys: {
      [variant]: {
        id: decodeVerifyingKeyIdValue(fields.id, `${TEXT_VERIFYING_KEYS}${variant}.id`),
        record: decodeVerifyingKeyRecordValue(
          fields.record,
          `${TEXT_VERIFYING_KEYS}${variant}.record`,
        ),
      },
    },
  };
}

function encodeZkInstruction(instruction) {
  const entries = [
    [TEXT_REGISTER_ZK_ASSET, REGISTER_ZK_ASSET_WIRE_ID, encodeRegisterZkAssetPayload],
    [TEXT_SCHEDULE_CONFIDENTIAL_POLICY_TRANSITION, SCHEDULE_CONFIDENTIAL_POLICY_TRANSITION_WIRE_ID, encodeScheduleConfidentialPolicyTransitionPayload],
    [TEXT_CANCEL_CONFIDENTIAL_POLICY_TRANSITION, CANCEL_CONFIDENTIAL_POLICY_TRANSITION_WIRE_ID, encodeCancelConfidentialPolicyTransitionPayload],
    ["CreateElection", CREATE_ELECTION_WIRE_ID, encodeCreateElectionPayload],
    ["SubmitBallot", SUBMIT_BALLOT_WIRE_ID, encodeSubmitBallotPayload],
    [TEXT_FINALIZE_ELECTION, FINALIZE_ELECTION_WIRE_ID, encodeFinalizeElectionPayload],
  ];
  for (const [key, wireId, encode] of entries) {
    if (isPlainObject(instruction[key])) {
      return encodeInstructionEnvelope(wireId, encode(instruction[key], `zk.${key}`));
    }
  }
  rejectError(`${TEXT_INTERNAL_NORITO_CANONICALIZATION_DOES_NOT_SUPPORT}zk ${TEXT_INSTRUCTION}${describeInstructionShape(instruction)}`);
}

function encodeRegisterZkAssetPayload(value) {
  assertExactObjectKeys(
    value,
    ["asset", "vk_unshield", "vk_shield"],
    ("zk." + TEXT_REGISTER_ZK_ASSET),
  );
  return encodeCanonicalRecordFields(value, ("zk." + TEXT_REGISTER_ZK_ASSET), zk_RegisterZkAssetInstructionFields);
}

function encodeScheduleConfidentialPolicyTransitionPayload(value) {
  return encodeStructValue([
    [encodeAssetDefinitionIdValue(value.asset, (TEXT_ZK_SCHEDULE_CONFIDENTIAL_POLICY_TRANSITION + "asset"))],
    [encodeConfidentialPolicyModeValue(value.new_mode, (TEXT_ZK_SCHEDULE_CONFIDENTIAL_POLICY_TRANSITION + "new_mode"))],
    [encodeU64NumberValue(value.effective_height, SCHEDULE_EFFECTIVE_HEIGHT_CONTEXT)],
    [encodeHashValue(value.transition_id, SCHEDULE_TRANSITION_ID_CONTEXT)],
    [encodeOptionValue(value.conversion_window, encodeU64NumberValue, SCHEDULE_CONVERSION_WINDOW_CONTEXT)],
  ]);
}

function encodeCancelConfidentialPolicyTransitionPayload(value) {
  return encodeCanonicalRecordFields(value, ("zk." + TEXT_CANCEL_CONFIDENTIAL_POLICY_TRANSITION), zk_CancelConfidentialPolicyTransitionInstructionFields);
}

function encodeCreateElectionPayload(value) {
  return encodeStructValue([
    [encodeNoritoStringValue(assertCanonicalGovernanceSelectorV1(
      value.election_id,
      (TEXT_ZK_CREATE_ELECTION + TEXT_ELECTION_ID),
    ))],
    [encodeU32Value(value.options, (TEXT_ZK_CREATE_ELECTION + "options"))],
    [encodeFixedBytesValue(value.eligible_root, 32, (TEXT_ZK_CREATE_ELECTION + "eligible_root"))],
    [encodeU64NumberValue(value.start_ts, (TEXT_ZK_CREATE_ELECTION + "start_ts"))],
    [encodeU64NumberValue(value.end_ts, (TEXT_ZK_CREATE_ELECTION + "end_ts"))],
    [encodeVerifyingKeyIdValue(value.vk_ballot, (TEXT_ZK_CREATE_ELECTION + "vk_ballot"))],
    [encodeVerifyingKeyIdValue(value.vk_tally, (TEXT_ZK_CREATE_ELECTION + "vk_tally"))],
    [encodeNoritoStringValue(assertNonEmptyString(value.domain_tag, (TEXT_ZK_CREATE_ELECTION + "domain_tag")))],
  ]);
}

function encodeSubmitBallotPayload(value) {
  return encodeStructValue([
    [encodeNoritoStringValue(assertCanonicalGovernanceSelectorV1(
      value.election_id,
      (TEXT_ZK_SUBMIT_BALLOT + TEXT_ELECTION_ID),
    ))],
    [encodeByteVecValue(value.ciphertext, (TEXT_ZK_SUBMIT_BALLOT + "ciphertext"))],
    [encodeProofAttachmentValue(value.ballot_proof, (TEXT_ZK_SUBMIT_BALLOT + "ballot_proof"))],
    [encodeFixedBytesValue(value.nullifier, 32, (TEXT_ZK_SUBMIT_BALLOT + "nullifier"))],
  ]);
}

function encodeFinalizeElectionPayload(value) {
  return encodeStructValue([
    [encodeNoritoStringValue(assertCanonicalGovernanceSelectorV1(
      value.election_id,
      (TEXT_ZK_FINALIZE_ELECTION + TEXT_ELECTION_ID),
    ))],
    [encodeNoritoVec(value.tally ?? [], (entry, index) =>
      encodeU64NumberValue(entry, `${TEXT_ZK_FINALIZE_ELECTION}tally[${index}]`),
    )],
    [encodeProofAttachmentValue(value.tally_proof, (TEXT_ZK_FINALIZE_ELECTION + "tally_proof"))],
  ]);
}

function encodeRwaInstruction(instruction) {
  const variants = [
    ["RegisterRwa", 0, encodeRegisterRwaPayload],
    ["TransferRwa", 1, encodeTransferRwaPayload],
    ["MergeRwas", 2, encodeMergeRwasPayload],
    ["RedeemRwa", 3, encodeRedeemRwaPayload],
    ["FreezeRwa", 4, encodeFreezeRwaPayload],
    ["UnfreezeRwa", 5, encodeUnfreezeRwaPayload],
    ["HoldRwa", 6, encodeHoldRwaPayload],
    ["ReleaseRwa", 7, encodeReleaseRwaPayload],
    ["ForceTransferRwa", 8, encodeForceTransferRwaPayload],
    ["SetRwaControls", 9, encodeSetRwaControlsPayload],
    ["SetRwaKeyValue", 10, encodeSetRwaKeyValuePayload],
    ["RemoveRwaKeyValue", 11, encodeRemoveRwaKeyValuePayload],
  ];
  for (const [key, index, encode] of variants) {
    if (isPlainObject(instruction[key])) {
      return encodeEnumInstruction("iroha.rwa", index, encode(instruction[key], key));
    }
  }
  rejectError(`${TEXT_INTERNAL_NORITO_CANONICALIZATION_DOES_NOT_SUPPORT}RWA ${TEXT_INSTRUCTION}${describeInstructionShape(instruction)}`);
}

function encodeKaigiIdValue(value, context) {
  const literal = assertExactNonEmptyString(
    typeof value === JS_TYPE_STRING ? value : `${value.domain_id}:${value.call_name}`,
    context,
  );
  const separator = literal.indexOf(":");
  if (separator <= 0 || separator === literal.length - 1) {
    rejectError(`${context} must use domain:call format`);
  }
  return encodeStructValue([
    [encodeDomainIdValue(literal.slice(0, separator), `${context}.domain_id`)],
    [encodeNameValue(literal.slice(separator + 1), `${context}.call_name`)],
  ]);
}

const KaigiIdValueFields = [
    ["domain_id", decodeDomainIdValue, 0],
    ["call_name", decodeNameValue, 0],
  ];

  function decodeKaigiIdValue(payload, context) {
    return decodeRecordFields(payload, context, KaigiIdValueFields);
  }

function encodeNewKaigiValue(value, context) {
  return encodeStructValue([
    [encodeKaigiIdValue(value.id, `${context}.id`)],
    [encodeAccountIdValue(value.host, `${context}.host`)],
    [encodeOptionValue(value.title, encodeNoritoStringValue, `${context}.title`)],
    [encodeOptionValue(value.description, encodeNoritoStringValue, `${context}.description`)],
    [encodeOptionValue(value.max_participants, encodeU32Value, `${context}.max_participants`)],
    [encodeU64NumberValue(value.gas_rate_per_minute ?? 0, `${context}.gas_rate_per_minute`)],
    [encodeMetadataValue(value.metadata ?? {}, `${context}.metadata`)],
    [encodeOptionValue(value.scheduled_start_ms, encodeU64NumberValue, `${context}.scheduled_start_ms`)],
    [encodeOptionValue(value.billing_account, encodeAccountIdValue, `${context}.billing_account`)],
    [encodeKaigiPrivacyModeValue(value.privacy_mode, `${context}.privacy_mode`)],
    [encodeKaigiRoomPolicyValue(value.room_policy ?? { policy: "Authenticated", state: null }, `${context}.room_policy`)],
    [encodeOptionValue(value.relay_manifest, encodeKaigiRelayManifestValue, `${context}.relay_manifest`)],
  ]);
}

const NewKaigiPayloadFields = [
    ["id", decodeKaigiIdValue, 0],
    ["host", decodeAccountIdValue, 0],
    ["title", decodeStringValue, 1],
    ["description", decodeStringValue, 1],
    ["max_participants", decodeU32Value, 1],
    ["gas_rate_per_minute", decodeU64NumberValue, 0],
    [FIELD_METADATA, decodeMetadataValue, 0],
    ["scheduled_start_ms", decodeU64NumberValue, 1],
    ["billing_account", decodeAccountIdValue, 1],
    ["privacy_mode", decodeKaigiPrivacyModeValue, 0],
    ["room_policy", decodeKaigiRoomPolicyValue, 0],
    ["relay_manifest", decodeKaigiRelayManifestValue, 1],
  ];

  function decodeNewKaigiPayload(payload, context) {
    return decodeRecordFields(payload, context, NewKaigiPayloadFields);
  }

function encodeKaigiScalarValue(value, context) {
  return Buffer.from(kaigiScalarBytesV1(value, context));
}

function decodeKaigiScalarValue(payload, context) {
  return Array.from(kaigiScalarBytesV1(payload, context));
}

function encodeKaigiParticipantCommitmentValue(value, context) {
  if (Object.keys(value).length !== 1 || !(FIELD_COMMITMENT in value)) {
    rejectType(`${context} requires only ${TEXT_COMMITMENT}`);
  }
  return encodeStructValue([
    [encodeKaigiScalarValue(value.commitment, `${context}.${TEXT_COMMITMENT}`)],
  ]);
}

const KaigiParticipantCommitmentValueFields = [
    [FIELD_COMMITMENT, decodeKaigiScalarValue, 0],
  ];

  function decodeKaigiParticipantCommitmentValue(payload, context) {
    return decodeRecordFields(payload, context, KaigiParticipantCommitmentValueFields);
  }

function encodeKaigiParticipantNullifierValue(value, context) {
  if (Object.keys(value).length !== 1 || !("digest" in value)) {
    rejectType(`${context} requires only digest`);
  }
  return encodeStructValue([
    [encodeKaigiScalarValue(value.digest, `${context}.digest`)],
  ]);
}

const KaigiParticipantNullifierValueFields = [
    ["digest", decodeKaigiScalarValue, 0],
  ];

  function decodeKaigiParticipantNullifierValue(payload, context) {
    return decodeRecordFields(payload, context, KaigiParticipantNullifierValueFields);
  }

function encodeKaigiRelayManifestValue(value, context) {
  return encodeCanonicalRecordFields(value, context, KaigiRelayManifestValueFields);
}

const KaigiRelayManifestValueFields = [
    ["hops", decodeKaigiRelayHopValue, 2, encodeKaigiRelayHopValue, 3],
    ["expiry_ms", decodeU64NumberValue, 0, encodeU64NumberValue, 0],
  ];

  function decodeKaigiRelayManifestValue(payload, context) {
    return decodeRecordFields(payload, context, KaigiRelayManifestValueFields);
  }

function encodeKaigiRelayHopValue(value, context) {
  return encodeCanonicalRecordFields(value, context, KaigiRelayHopValueFields);
}

const KaigiRelayHopValueFields = [
    ["relay_id", decodeAccountIdValue, 0, encodeAccountIdValue, 0],
    ["hpke_public_key", decodeByteVecAsBase64, 0, encodeByteVecValue, 0],
    ["weight", decodeU8Value, 0, encodeU8Value, 0],
  ];

  function decodeKaigiRelayHopValue(payload, context) {
    return decodeRecordFields(payload, context, KaigiRelayHopValueFields);
  }

function encodeKaigiRelayRegistrationValue(value, context) {
  return encodeCanonicalRecordFields(value, context, KaigiRelayRegistrationValueFields);
}

const KaigiRelayRegistrationValueFields = [
    ["relay_id", decodeAccountIdValue, 0, encodeAccountIdValue, 0],
    ["hpke_public_key", decodeByteVecAsBase64, 0, encodeByteVecValue, 0],
    ["bandwidth_class", decodeU8Value, 0, encodeU8Value, 0],
  ];

  function decodeKaigiRelayRegistrationValue(payload, context) {
    return decodeRecordFields(payload, context, KaigiRelayRegistrationValueFields);
  }

function encodeKaigiRelayHealthStatusValue(value, context) {
  const status = typeof value === JS_TYPE_STRING ? value : value?.status;
  switch (status) {
    case "Healthy":
      return encodeEnumTagValue(0);
    case "Degraded":
      return encodeEnumTagValue(1);
    case "Unavailable":
      return encodeEnumTagValue(2);
    default:
      rejectType(`${context}${TEXT_MUST_BE}Healthy, Degraded, or Unavailable`);
  }
}

function decodeKaigiRelayHealthStatusValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("tag");
  reader.assertEof();
  let status;
  switch (tag) {
    case 0:
      status = "Healthy";
      break;
    case 1:
      status = "Degraded";
      break;
    case 2:
      status = "Unavailable";
      break;
    default:
      rejectError(`${context}${TEXT_USES_UNSUPPORTED}relay health status ${tag}`);
  }
  return { status, state: null };
}

function validateKaigiRelayHealthNotesValue(value, context) {
  if (typeof value !== JS_TYPE_STRING) {
    rejectType(`${context}${TEXT_MUST_BE_A}string`);
  }
  assertWellFormedUtf16(value, context);
  let scalarCount = 0;
  for (let index = 0; index < value.length; index += 1) {
    const codeUnit = value.charCodeAt(index);
    if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
      index += 1;
    }
    scalarCount += 1;
    if (scalarCount > 512) {
      rejectRange(`${context} must not exceed 512 Unicode scalar values`);
    }
  }
  return value;
}

function encodeKaigiRelayHealthNotesValue(value, context) {
  return encodeNoritoStringValue(
    validateKaigiRelayHealthNotesValue(value, context),
  );
}

function decodeKaigiRelayHealthNotesValue(payload, context) {
  return validateKaigiRelayHealthNotesValue(
    decodeStringValue(payload, context),
    context,
  );
}

function encodeRegisterRwaPayload(value) {
  return encodeStructValue([
    [encodeNewRwaValue(value.rwa, "RegisterRwa.rwa")],
  ]);
}

function encodeTransferRwaPayload(value) {
  return encodeCanonicalRecordFields(value, "TransferRwa", TransferRwaInstructionFields);
}

function encodeMergeRwasPayload(value) {
  return encodeStructValue([
    [encodeNoritoVec(value.parents ?? [], (parent, index) =>
      encodeRwaParentRefValue(parent, `MergeRwas.parents[${index}]`),
    )],
    [encodeNoritoStringValue(assertNonEmptyString(value.primary_reference, ("MergeRwas." + TEXT_PRIMARY_REFERENCE)))],
    [encodeOptionValue(value.status, encodeNameValue, "MergeRwas.status")],
    [encodeMetadataValue(value.metadata ?? {}, "MergeRwas.metadata")],
  ]);
}

function encodeRedeemRwaPayload(value) {
  return encodeCanonicalRecordFields(value, "RedeemRwa", CanonicalEncodingFields25);
}

function encodeFreezeRwaPayload(value) {
  return encodeStructValue([
    [encodeRwaIdValue(value.rwa, "FreezeRwa.rwa")],
  ]);
}

function encodeUnfreezeRwaPayload(value) {
  return encodeStructValue([
    [encodeRwaIdValue(value.rwa, "UnfreezeRwa.rwa")],
  ]);
}

function encodeHoldRwaPayload(value) {
  return encodeCanonicalRecordFields(value, "HoldRwa", CanonicalEncodingFields26);
}

function encodeReleaseRwaPayload(value) {
  return encodeCanonicalRecordFields(value, "ReleaseRwa", CanonicalEncodingFields27);
}

function encodeForceTransferRwaPayload(value) {
  return encodeCanonicalRecordFields(value, "ForceTransferRwa", ForceTransferRwaInstructionFields);
}

function encodeSetRwaControlsPayload(value) {
  return encodeCanonicalRecordFields(value, "SetRwaControls", SetRwaControlsInstructionFields);
}

function encodeSetRwaKeyValuePayload(value) {
  return encodeStructValue([
    [encodeRwaIdValue(value.rwa, "SetRwaKeyValue.rwa")],
    [encodeNameValue(value.key, "SetRwaKeyValue.key")],
    [encodeNoritoField(encodeNoritoJsonValue(value.value))],
  ]);
}

function encodeRemoveRwaKeyValuePayload(value) {
  return encodeCanonicalRecordFields(value, "RemoveRwaKeyValue", RemoveRwaKeyValueInstructionFields);
}

function encodeNewRwaValue(value, context) {
  return encodeStructValue([
    [encodeArchivedDomainIdValue(value.domain, `${context}.domain`)],
    [encodeQuantityValue(value.quantity, `${context}.quantity`)],
    [encodeNumericSpecValue(value.spec ?? { scale: null }, `${context}.spec`)],
    [encodeNoritoStringValue(assertNonEmptyString(value.primary_reference, `${context}.${TEXT_PRIMARY_REFERENCE}`))],
    [encodeOptionValue(value.status, encodeNameValue, `${context}.status`)],
    [encodeMetadataValue(value.metadata ?? {}, `${context}.metadata`)],
    [encodeNoritoVec(value.parents ?? [], (parent, index) =>
      encodeRwaParentRefValue(parent, `${context}.parents[${index}]`),
    )],
    [encodeRwaControlPolicyValue(value.controls ?? {}, `${context}.controls`)],
  ]);
}

const NewRwaValueFields = [
    ["domain", decodeArchivedDomainIdValue, 0],
    [FIELD_QUANTITY, decodeQuantityValue, 0],
    ["spec", decodeNumericSpecValue, 0],
    [TEXT_PRIMARY_REFERENCE, decodeStringValue, 0],
    ["status", decodeNameValue, 1],
    [FIELD_METADATA, decodeMetadataValue, 0],
    ["parents", decodeRwaParentRefValue, 2],
    ["controls", decodeRwaControlPolicyValue, 0],
  ];

  function decodeNewRwaValue(payload, context) {
    return decodeRecordFields(payload, context, NewRwaValueFields);
  }

function encodeRwaParentRefValue(value, context) {
  return encodeCanonicalRecordFields(value, context, RwaParentRefValueFields);
}

const RwaParentRefValueFields = [
    ["rwa", decodeRwaIdValue, 0, encodeRwaIdValue, 0],
    ["quantity", decodeQuantityValue, 0, encodeQuantityValue, 0],
  ];

  function decodeRwaParentRefValue(payload, context) {
    return decodeRecordFields(payload, context, RwaParentRefValueFields);
  }

function encodeRwaControlPolicyValue(value, context) {
  return encodeStructValue([
    [encodeNoritoVec(value.controller_accounts ?? [], (entry, index) =>
      encodeAccountIdValue(entry, `${context}.controller_accounts[${index}]`),
    )],
    [encodeNoritoVec(value.controller_roles ?? [], (entry, index) =>
      encodeRoleIdValue(entry, `${context}.controller_roles[${index}]`),
    )],
    [encodeBoolValue(Boolean(value.freeze_enabled), `${context}.freeze_enabled`)],
    [encodeBoolValue(Boolean(value.hold_enabled), `${context}.hold_enabled`)],
    [encodeBoolValue(Boolean(value.force_transfer_enabled), `${context}.force_transfer_enabled`)],
    [encodeBoolValue(Boolean(value.redeem_enabled), `${context}.redeem_enabled`)],
  ]);
}

const RwaControlPolicyValueFields = [
    ["controller_accounts", decodeAccountIdValue, 2],
    ["controller_roles", decodeRoleIdValue, 2],
    ["freeze_enabled", decodeBoolValue, 0],
    ["hold_enabled", decodeBoolValue, 0],
    ["force_transfer_enabled", decodeBoolValue, 0],
    ["redeem_enabled", decodeBoolValue, 0],
  ];

  function decodeRwaControlPolicyValue(payload, context) {
    return decodeRecordFields(payload, context, RwaControlPolicyValueFields);
  }

function encodeAssetInstructionBody(value, context) {
  return Buffer.concat([
    encodeNoritoField(encodeQuantityValue(value.object, `${context}.object`)),
    encodeNoritoField(encodeAssetIdValue(value.destination, `${context}${TEXT_DESTINATION}`)),
  ]);
}

function decodeAssetInstructionBody(payload, context) {
  const reader = new BufferReader(payload, context);
  const object = decodeQuantityValue(readNoritoField(reader, JS_TYPE_OBJECT), `${context}.object`);
  const destination = decodeAssetIdValue(
    readNoritoField(reader, FIELD_DESTINATION),
    `${context}${TEXT_DESTINATION}`,
  );
  reader.assertEof();
  return { object, destination };
}

function encodeTransferAssetBody(value) {
  return Buffer.concat([
    encodeNoritoField(encodeAssetIdValue(value.source, "Transfer.Asset.source")),
    encodeNoritoField(encodeQuantityValue(value.object, "Transfer.Asset.object")),
    encodeNoritoField(encodeAccountIdValue(value.destination, "Transfer.Asset.destination")),
  ]);
}

function decodeTransferAssetBody(payload) {
  const reader = new BufferReader(payload, "Transfer.Asset");
  const source = decodeAssetIdValue(readNoritoField(reader, "source"), "Transfer.Asset.source");
  const object = decodeQuantityValue(readNoritoField(reader, JS_TYPE_OBJECT), "Transfer.Asset.object");
  const destination = decodeAccountIdValue(
    readNoritoField(reader, FIELD_DESTINATION),
    "Transfer.Asset.destination",
  );
  reader.assertEof();
  return { source, object, destination };
}

function encodeTriggerRepetitionsBody(value, context) {
  return Buffer.concat([
    encodeNoritoField(encodeU32Value(value.object, `${context}.object`)),
    encodeNoritoField(
      encodeNoritoField(
        encodeNoritoStringValue(
          assertNonEmptyString(value.destination, `${context}${TEXT_DESTINATION}`),
        ),
      ),
    ),
  ]);
}

function decodeTriggerRepetitionsBody(payload, context) {
  const reader = new BufferReader(payload, context);
  const object = decodeU32Value(readNoritoField(reader, JS_TYPE_OBJECT), `${context}.object`);
  const destination = decodeStringValue(
    readNoritoField(
      new BufferReader(readNoritoField(reader, FIELD_DESTINATION), `${context}.destination.outer`),
      "value",
    ),
    `${context}${TEXT_DESTINATION}`,
  );
  reader.assertEof();
  return { object, destination };
}

function encodeExecuteTriggerPayload(value) {
  if (!isPlainObject(value)) {
    rejectType(("ExecuteTrigger" + TEXT_MUST_BE_AN_OBJECT_2));
  }
  const trigger = assertNonEmptyString(value.trigger, "ExecuteTrigger.trigger");
  return Buffer.concat([
    encodeNoritoField(encodeNoritoField(encodeNoritoStringValue(trigger))),
    encodeNoritoField(encodeNoritoField(encodeNoritoJsonValue(value.args ?? null))),
  ]);
}

function decodeExecuteTriggerPayload(payload) {
  const reader = new BufferReader(payload, "ExecuteTrigger");
  const trigger = decodeStringValue(
    readNoritoField(
      new BufferReader(readNoritoField(reader, "trigger"), "ExecuteTrigger.trigger.outer"),
      "value",
    ),
    "ExecuteTrigger.trigger",
  );
  const args = decodeJsonValue(
    readNoritoField(
      new BufferReader(readNoritoField(reader, "args"), "ExecuteTrigger.args.outer"),
      "value",
    ),
    "ExecuteTrigger.args",
  );
  reader.assertEof();
  return { trigger, args };
}

function encodeAccountIdValue(value, context) {
  const literal = normalizeAccountId(value, context);
  const address = AccountAddress.fromI105(literal);
  const controller = address._controller;
  if (!controller || typeof controller.tag !== JS_TYPE_NUMBER) {
    rejectError(`${context} could not resolve account controller information`);
  }
  switch (controller.tag) {
    case 0:
      return Buffer.concat([
        u32ToLittleEndianBuffer(0),
        encodeNoritoField(encodePublicKeyValue(controller, context)),
      ]);
    case 1:
      return Buffer.concat([
        u32ToLittleEndianBuffer(1),
        encodeNoritoField(encodeMultisigPolicyPayload(controller, context)),
      ]);
    default:
      rejectError(`${context}${TEXT_USES_UNSUPPORTED}account controller tag ${controller.tag}`);
  }
}

/** @internal Exact compact-length AccountId value encoding for typed policy codecs. */
export function encodeAccountIdNoritoValue(value, context = "AccountId") {
  return withNoritoCompactLengths(() =>
    Uint8Array.from(encodeAccountIdValue(value, context)),
  );
}

/** @internal Exact compact-length AccountId value decoding for typed policy codecs. */
export function decodeAccountIdNoritoValue(payload, context = "AccountId") {
  const bytes = Buffer.from(normalizeFlexibleBytes(payload, context));
  return withNoritoCompactLengths(() => decodeAccountIdValue(bytes, context));
}

/** @internal Decode and re-encode one exact compact-length AccountId value. */
export function _canonicalAccountIdNoritoValue(payload, context = "AccountId") {
  return encodeAccountIdNoritoValue(decodeAccountIdNoritoValue(payload, context), context);
}

function decodeAccountIdValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const kind = reader.readU32LE("kind");
  const controllerPayload = readNoritoField(reader, "payload");
  reader.assertEof();
  let header;
  let controller;
  if (kind === 0) {
    const { curve, publicKey } = decodePublicKeyValue(controllerPayload, context);
    header = { version: 0, classId: 0, normVersion: 1, extFlag: false };
    controller = { tag: 0, curve, publicKey };
  } else if (kind === 1) {
    const policy = decodeMultisigPolicyPayload(controllerPayload, context);
    header = { version: 0, classId: 1, normVersion: 1, extFlag: false };
    controller = { tag: 1, ...policy };
  } else {
    rejectError(`${context}${TEXT_USES_UNSUPPORTED}account controller variant ${kind}`);
  }
  return new AccountAddress(header, controller).toI105();
}

function encodePublicKeyValue(controller, context) {
  ensureCurveIdEnabled(controller.curve, context);
  const publicKey = Buffer.from(normalizeBytes(controller.publicKey));
  validatePublicKeyForCurve(controller.curve, publicKey, context);
  return encodeConstVecU8Value(
    Buffer.concat([Buffer.of(algorithmTagForCurveId(controller.curve, context)), publicKey]),
  );
}

function decodePublicKeyValue(payload, context) {
  const bytes = decodeConstVecU8Value(payload, `${context}.publicKey`);
  if (bytes.length === 0) {
    rejectError(`${context}.publicKey payload is empty`);
  }
  const curve = curveIdForAlgorithmTag(bytes[0], `${context}.publicKey.algorithm`);
  const publicKey = bytes.subarray(1);
  validatePublicKeyForCurve(curve, publicKey, `${context}.publicKey.payload`);
  return { curve, publicKey: Buffer.from(publicKey) };
}

function encodeConstVecU8Value(bytes) {
  const normalized = Buffer.from(normalizeFlexibleBytes(bytes, "ConstVec<u8>"));
  const parts = [u64ToLittleEndianBuffer(normalized.length)];
  for (const byte of normalized) {
    parts.push(encodeNoritoLength(1), Buffer.of(byte));
  }
  return Buffer.concat(parts);
}

function decodeConstVecU8Value(payload, context) {
  const reader = new BufferReader(payload, context, noritoLengthFlags);
  const count = bigintToSafeNumber(reader.readU64LE("count"), `${context}.count`);
  const bytes = Buffer.allocUnsafe(count);
  for (let index = 0; index < count; index += 1) {
    const item = readNoritoField(reader, `item${index}`);
    if (item.length !== 1) {
      rejectError(`${context}[${index}]${TEXT_MUST_CONTAIN}exactly one byte`);
    }
    bytes[index] = item[0];
  }
  reader.assertEof();
  return bytes;
}

function algorithmTagForCurveId(curve, context) {
  const algorithm = curveIdToAlgorithm(curve);
  switch (algorithm) {
    case ED25519_ALGORITHM:
      return 0;
    case "secp256k1":
      return 1;
    case "bls_normal":
      return 2;
    case "bls_small":
      return 3;
    case "ml-dsa":
      return 4;
    case "gost3410-2012-256-paramset-a":
      return 5;
    case "gost3410-2012-256-paramset-b":
      return 6;
    case "gost3410-2012-256-paramset-c":
      return 7;
    case "gost3410-2012-512-paramset-a":
      return 8;
    case "gost3410-2012-512-paramset-b":
      return 9;
    case "sm2":
      return 10;
    default:
      rejectError(`${context}${TEXT_USES_UNSUPPORTED}public-key algorithm ${algorithm}`);
  }
}

function curveIdForAlgorithmTag(tag, context) {
  switch (tag) {
    case 0:
      return curveIdFromAlgorithm(ED25519_ALGORITHM);
    case 1:
      return curveIdFromAlgorithm("secp256k1");
    case 2:
      return curveIdFromAlgorithm("bls_normal");
    case 3:
      return curveIdFromAlgorithm("bls_small");
    case 4:
      return curveIdFromAlgorithm("ml-dsa");
    case 5:
      return curveIdFromAlgorithm("gost3410-2012-256-paramset-a");
    case 6:
      return curveIdFromAlgorithm("gost3410-2012-256-paramset-b");
    case 7:
      return curveIdFromAlgorithm("gost3410-2012-256-paramset-c");
    case 8:
      return curveIdFromAlgorithm("gost3410-2012-512-paramset-a");
    case 9:
      return curveIdFromAlgorithm("gost3410-2012-512-paramset-b");
    case 10:
      return curveIdFromAlgorithm("sm2");
    default:
      rejectError(`${context}${TEXT_USES_UNSUPPORTED}public-key algorithm tag ${tag}`);
  }
}

function encodeMultisigPolicyPayload(policy, context) {
  if (!Array.isArray(policy.members) || policy.members.length === 0) {
    rejectError(`${context} multisig policy${TEXT_MUST_CONTAIN}at least one member`);
  }
  return Buffer.concat([
    encodeNoritoField(encodeU8Value(policy.version, `${context}.version`)),
    encodeNoritoField(encodeU16Value(policy.threshold, `${context}.threshold`)),
    encodeNoritoField(
      encodeNoritoVec(policy.members, (member, index) =>
        encodeMultisigMemberPayload(member, `${context}.members[${index}]`),
      ),
    ),
  ]);
}

function decodeMultisigPolicyPayload(payload, context) {
  const reader = new BufferReader(payload, context);
  const version = decodeU8Value(readNoritoField(reader, "version"), `${context}.version`);
  const threshold = decodeU16Value(readNoritoField(reader, "threshold"), `${context}.threshold`);
  const members = decodeNoritoVec(
    readNoritoField(reader, "members"),
    (memberPayload, index) =>
      decodeMultisigMemberPayload(memberPayload, `${context}.members[${index}]`),
    `${context}.members`,
  );
  reader.assertEof();
  return { version, threshold, members };
}

function encodeMultisigMemberPayload(member, context) {
  return Buffer.concat([
    encodeNoritoField(encodePublicKeyValue(member, `${context}.public_key`)),
    encodeNoritoField(encodeU16Value(member.weight, `${context}.weight`)),
  ]);
}

function decodeMultisigMemberPayload(payload, context) {
  const reader = new BufferReader(payload, context);
  const { curve, publicKey } = decodePublicKeyValue(
    readNoritoField(reader, "publicKey"),
    `${context}.publicKey`,
  );
  const weight = decodeU16Value(readNoritoField(reader, "weight"), `${context}.weight`);
  reader.assertEof();
  return { curve, publicKey, weight };
}

function encodeAssetIdValue(value, context) {
  const literal = normalizeAssetHoldingId(value, context);
  const [definitionId, accountId, scopeLiteral] = literal.split("#");
  return Buffer.concat([
    encodeNoritoField(encodeAccountIdValue(accountId, `${context}.accountId`)),
    encodeNoritoField(encodeAssetDefinitionIdValue(definitionId, `${context}.assetDefinitionId`)),
    encodeNoritoField(encodeAssetBalanceScopeValue(scopeLiteral, `${context}.scope`)),
  ]);
}

function decodeAssetIdValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const accountId = decodeAccountIdValue(
    readNoritoField(reader, "account"),
    `${context}.account`,
  );
  const definitionId = decodeAssetDefinitionIdValue(
    readNoritoField(reader, "definition"),
    `${context}.definition`,
  );
  const scopeSuffix = decodeAssetBalanceScopeValue(
    readNoritoField(reader, "scope"),
    `${context}.scope`,
  );
  reader.assertEof();
  return `${definitionId}#${accountId}${scopeSuffix}`;
}

function encodeAssetDefinitionIdValue(value, context) {
  const literal = normalizeAssetId(value, context);
  const payload = decodeBase58(literal, context);
  if (payload.length !== 21) {
    rejectError(`${context} must decode to exactly 21 bytes`);
  }
  if (payload[0] !== ASSET_DEFINITION_ADDRESS_VERSION) {
    rejectError(`${context} version byte ${payload[0]} is not supported`);
  }
  const checksum = payload.subarray(17);
  const expected = assetDefinitionChecksum(payload.subarray(0, 17));
  if (!checksum.equals(expected)) {
    rejectError(`${context} checksum is invalid`);
  }
  return encodeFixedByteArrayArchiveValue(payload.subarray(1, 17), 16, context);
}

/** @internal Exact compact-length AssetDefinitionId value encoding for typed policy codecs. */
export function encodeAssetDefinitionIdNoritoValue(
  value,
  context = "AssetDefinitionId",
) {
  return withNoritoCompactLengths(() =>
    Uint8Array.from(encodeAssetDefinitionIdValue(value, context)),
  );
}

function decodeAssetDefinitionIdValue(payload, context) {
  const bytes = decodeFixedByteArrayArchiveValue(payload, 16, context);
  const payloadBytes = Buffer.concat([
    Buffer.from([ASSET_DEFINITION_ADDRESS_VERSION]),
    bytes,
  ]);
  return encodeBase58(Buffer.concat([payloadBytes, assetDefinitionChecksum(payloadBytes)]));
}

function encodeAssetBalanceScopeValue(scopeLiteral, context) {
  if (scopeLiteral === undefined) {
    return u32ToLittleEndianBuffer(0);
  }
  const match = /^dataspace:(\d+)$/.exec(scopeLiteral);
  if (!match) {
    rejectError(`${context} must use dataspace:<id> when present`);
  }
  return Buffer.concat([
    u32ToLittleEndianBuffer(1),
    encodeNoritoField(
      encodeNoritoField(encodeU64Value(match[1], `${context}.dataspace.value`)),
    ),
  ]);
}

function decodeAssetBalanceScopeValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const kind = reader.readU32LE("kind");
  if (kind === 0) {
    reader.assertEof();
    return "";
  }
  if (kind === 1) {
    const dataspacePayload = readNoritoField(reader, "dataspace");
    const dataspaceReader = new BufferReader(dataspacePayload, `${context}.dataspace`);
    const dataspace = decodeU64Value(
      readNoritoField(dataspaceReader, "value"),
      `${context}.dataspace.value`,
    );
    dataspaceReader.assertEof();
    reader.assertEof();
    return `#dataspace:${dataspace}`;
  }
  rejectError(`${context}${TEXT_USES_UNSUPPORTED}scope variant ${kind}`);
}

function encodeHashValue(value, context) {
  return encodeHashLiteralBytes(value, context);
}

function decodeHashValue(payload, context) {
  return decodeHashLiteral(payload, context);
}

function encodeEscrowIdValue(value, context) {
  if (typeof value !== JS_TYPE_STRING) {
    rejectType(`${context}${TEXT_MUST_BE_A}canonical checksummed hash literal`);
  }
  const match = HASH_LITERAL_RE.exec(value);
  if (
    match === null ||
    match[1] !== match[1].toUpperCase() ||
    match[2] !== match[2].toUpperCase()
  ) {
    rejectType(`${context} must use${TEXT_CANONICAL}uppercase hash:<hex>#<checksum> syntax`);
  }
  const bytes = encodeHashValue(value, context);
  if ((bytes[bytes.length - 1] & 1) === 0) {
    rejectType(`${context}${TEXT_MUST_USE_A_NATIVE_HASH_WITH_ITS_MARKER_BIT_SET}`);
  }
  return bytes;
}

function decodeEscrowIdValue(payload, context) {
  if (payload.length !== 32 || (payload[payload.length - 1] & 1) === 0) {
    rejectType(`${context}${TEXT_MUST_USE_A_NATIVE_HASH_WITH_ITS_MARKER_BIT_SET}`);
  }
  return decodeHashValue(payload, context);
}

function encodeStringValue(value, context) {
  if (typeof value !== JS_TYPE_STRING) {
    rejectType(`${context}${TEXT_MUST_BE_A}string`);
  }
  return encodeNoritoStringValue(value);
}

function encodeHashLiteralBytes(value, context) {
  let bytes;
  if (Buffer.isBuffer(value) || ArrayBuffer.isView(value) || value instanceof ArrayBuffer || Array.isArray(value)) {
    bytes = encodeFixedBytesValue(value, 32, context);
  } else {
    const literal = assertExactNonEmptyString(value, context);
    const match = HASH_LITERAL_RE.exec(literal);
    if (match) {
      const [, body, checksum] = match;
      const upper = body.toUpperCase();
      const expected = computeHashLiteralCrc("hash", upper);
      if (checksum.toUpperCase() !== expected) {
        rejectError(`${context} has invalid checksum; expected ${expected}`);
      }
      bytes = Buffer.from(upper, HEX_ENCODING);
    } else if (/^[0-9A-Fa-f]{64}$/.test(literal)) {
      bytes = Buffer.from(literal, HEX_ENCODING);
    } else {
      rejectError(`${context}${TEXT_MUST_BE_A}32-byte hash literal or hex string`);
    }
  }
  if ((bytes[bytes.length - 1] & 1) === 0) {
    rejectType(`${context}${TEXT_MUST_USE_A_NATIVE_HASH_WITH_ITS_MARKER_BIT_SET}`);
  }
  return bytes;
}

function decodeHashLiteral(payload, context) {
  const bytes = decodeFixedBytesValue(payload, 32, context);
  if ((bytes[bytes.length - 1] & 1) === 0) {
    rejectType(`${context}${TEXT_MUST_USE_A_NATIVE_HASH_WITH_ITS_MARKER_BIT_SET}`);
  }
  const body = bytes.toString(HEX_ENCODING).toUpperCase();
  return `hash:${body}#${computeHashLiteralCrc("hash", body)}`;
}

function encodeKeyedHashValue(value, context) {
  if (!isPlainObject(value)) {
    rejectType(`${context}${TEXT_MUST_BE_AN_OBJECT}`);
  }
  return encodeCanonicalRecordFields(value, context, KeyedHashValueFields);
}

const KeyedHashValueFields = [
    ["pepper_id", decodeStringValue, 0, encodeRequiredRecordString, 0],
    ["digest", decodeHashValue, 0, encodeHashValue, 0],
  ];

  function decodeKeyedHashValue(payload, context) {
    return decodeRecordFields(payload, context, KeyedHashValueFields);
  }

function encodeNumericSpecValue(value, context) {
  const scale = value?.scale ?? null;
  return encodeOptionValue(scale, encodeU32Value, `${context}.scale`);
}

function decodeNumericSpecValue(payload, context) {
  return {
    scale: decodeOptionValue(payload, decodeU32Value, `${context}.scale`),
  };
}

function encodeMintableValue(value, context) {
  const normalized = parseMintableLabel(value, context);
  switch (normalized.kind) {
    case WIRE_TYPE_INFINITELY:
      return encodeEnumTagValue(0);
    case "Once":
      return encodeEnumTagValue(1);
    case "Not":
      return encodeEnumTagValue(2);
    case "Limited":
      return encodeEnumTagValue(3, () =>
        encodeStructValue([[encodeU32Value(normalized.tokens, `${context}.tokens`)]]),
      );
    default:
      rejectError(`${context}${TEXT_USES_UNSUPPORTED}mintability ${normalized.kind}`);
  }
}

function decodeMintableValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("tag");
  if (tag === 0 || tag === 1 || tag === 2) {
    reader.assertEof();
    return [WIRE_TYPE_INFINITELY, "Once", "Not"][tag];
  }
  if (tag !== 3) {
    rejectError(`${context}${TEXT_USES_UNSUPPORTED}mintability ${tag}`);
  }
  const body = readNoritoField(reader, "tokens");
  reader.assertEof();
  const fields = decodeStructFields(body, `${context}.tokens`, ["value"]);
  const tokens = decodeU32Value(fields.value, `${context}.tokens.value`);
  if (tokens === 0) {
    rejectError(`${context}.tokens${TEXT_MUST_BE}non-zero`);
  }
  return `Limited(${tokens})`;
}

function parseMintableLabel(value, context) {
  const label = assertNonEmptyString(value, context);
  if (label === WIRE_TYPE_INFINITELY || label === "Once" || label === "Not") {
    return { kind: label };
  }
  const match = /^Limited\((\d+)\)$/.exec(label);
  if (match) {
    return { kind: "Limited", tokens: parseMintabilityTokens(match[1], `${context}.tokens`) };
  }
  rejectError(`${context}${TEXT_MUST_BE}Infinitely, Once, Not, or Limited(n)`);
}

function parseMintabilityTokens(value, context) {
  if (typeof value !== JS_TYPE_STRING || !/^\d+$/.test(value)) {
    rejectType(`${context}${TEXT_MUST_BE_A}positive unsigned 32-bit integer`);
  }
  const normalized = Number(value);
  if (!Number.isInteger(normalized) || normalized <= 0 || normalized > 0xffff_ffff) {
    rejectType(`${context}${TEXT_MUST_BE_A}positive unsigned 32-bit integer`);
  }
  return normalized;
}

function encodeAssetBalancePolicyValue(value, context) {
  const normalized = assertNonEmptyString(value, context);
  if (normalized === "Global") {
    return encodeEnumTagValue(0);
  }
  if (normalized === WIRE_TYPE_DATASPACE_RESTRICTED) {
    return encodeEnumTagValue(1);
  }
  rejectError(`${context}${TEXT_MUST_BE}Global or DataspaceRestricted`);
}

function decodeAssetBalancePolicyValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("tag");
  reader.assertEof();
  switch (tag) {
    case 0:
      return "Global";
    case 1:
      return WIRE_TYPE_DATASPACE_RESTRICTED;
    default:
      rejectError(`${context}${TEXT_USES_UNSUPPORTED}balance policy ${tag}`);
  }
}

function encodeAssetDefinitionAliasValue(value, context) {
  const literal = assertNonEmptyString(value, context);
  if (!literal.includes("#")) {
    rejectError(`${context} must use <name>#<dataspace> or <name>#<domain>.<dataspace>`);
  }
  return encodeStructValue([[encodeNoritoStringValue(literal)]]);
}

function decodeAssetDefinitionAliasValue(payload, context) {
  return decodeNestedValue(payload, decodeStringValue, context);
}

function encodeSorafsUriValue(value, context) {
  if (typeof value !== JS_TYPE_STRING) {
    rejectType(`${context}${TEXT_MUST_BE_A}string`);
  }
  if (value.trim() !== value || value.includes("\u0000") || /[\u0001-\u001f\u007f]/u.test(value)) {
    rejectError(`${context}${TEXT_MUST_NOT_CONTAIN}whitespace padding or control characters`);
  }
  if (!value.startsWith("sorafs://") || value.length === "sorafs://".length) {
    rejectError(`${context} must use a non-empty sorafs:// URI`);
  }
  return encodeStructValue([[encodeNoritoStringValue(value)]]);
}

function decodeSorafsUriValue(payload, context) {
  return decodeNestedValue(payload, decodeStringValue, context);
}

function encodeEnumTagValue(index, encodePayload) {
  const payload = encodePayload ? encodeNoritoField(encodePayload()) : Buffer.alloc(0);
  return Buffer.concat([u32ToLittleEndianBuffer(index), payload]);
}

function encodeKaigiPrivacyModeValue(value, context) {
  const mode =
    typeof value === JS_TYPE_STRING ? value : value?.mode ?? value?.privacy_mode ?? value?.kind;
  const normalized = assertNonEmptyString(mode ?? "Transparent", context).toLowerCase();
  if (normalized === "transparent") {
    return encodeEnumTagValue(0);
  }
  if (normalized === "zkrosterv1") {
    return encodeEnumTagValue(1);
  }
  rejectError(`${context}${TEXT_MUST_BE}Transparent or ZkRosterV1`);
}

function decodeKaigiPrivacyModeValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("tag");
  reader.assertEof();
  switch (tag) {
    case 0:
      return { mode: "Transparent", state: null };
    case 1:
      return { mode: "ZkRosterV1", state: null };
    default:
      rejectError(`${context}${TEXT_USES_UNSUPPORTED}privacy mode ${tag}`);
  }
}

function encodeKaigiRoomPolicyValue(value, context) {
  const policy = typeof value === JS_TYPE_STRING ? value : value?.policy ?? value?.room_policy;
  const normalized = assertNonEmptyString(policy ?? "Authenticated", context).toLowerCase();
  if (normalized === "public") {
    return encodeEnumTagValue(0);
  }
  if (normalized === "authenticated") {
    return encodeEnumTagValue(1);
  }
  rejectError(`${context}${TEXT_MUST_BE}Public or Authenticated`);
}

function decodeKaigiRoomPolicyValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("tag");
  reader.assertEof();
  switch (tag) {
    case 0:
      return { policy: "Public", state: null };
    case 1:
      return { policy: "Authenticated", state: null };
    default:
      rejectError(`${context}${TEXT_USES_UNSUPPORTED}room policy ${tag}`);
  }
}

function encodeConfidentialPolicyModeValue(value, context) {
  const normalized = assertNonEmptyString(value, context).toLowerCase();
  if (normalized === "transparentonly") {
    return encodeEnumTagValue(0);
  }
  if (normalized === "shieldedonly") {
    return encodeEnumTagValue(1);
  }
  if (normalized === "convertible") {
    return encodeEnumTagValue(2);
  }
  rejectError(`${context}${TEXT_MUST_BE}TransparentOnly, ShieldedOnly, or Convertible`);
}

function decodeConfidentialPolicyModeValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("tag");
  reader.assertEof();
  switch (tag) {
    case 0:
      return "TransparentOnly";
    case 1:
      return "ShieldedOnly";
    case 2:
      return "Convertible";
    default:
      rejectError(`${context}${TEXT_USES_UNSUPPORTED}confidential policy mode ${tag}`);
  }
}

function encodeVerifyingKeyIdValue(value, context) {
  if (!isPlainObject(value)) {
    rejectType(`${context}${TEXT_MUST_BE_AN_OBJECT}`);
  }
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.backend, `${context}${TEXT_BACKEND}`))],
    [encodeNoritoStringValue(assertNonEmptyString(value.name, `${context}.name`))],
  ]);
}

const VerifyingKeyIdValueFields = [
    ["backend", decodeStringValue, 0],
    ["name", decodeStringValue, 0],
  ];

  function decodeVerifyingKeyIdValue(payload, context) {
    return decodeRecordFields(payload, context, VerifyingKeyIdValueFields);
  }

function encodeBackendBytesBoxValue(value, context) {
  if (!isPlainObject(value)) {
    rejectType(`${context}${TEXT_MUST_BE_AN_OBJECT}`);
  }
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.backend, `${context}${TEXT_BACKEND}`))],
    [encodeByteVecValue(value.bytes, `${context}.bytes`)],
  ]);
}

function decodeProofBoxValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["backend", "bytes"]);
  const backend = decodeStringValue(fields.backend, `${context}${TEXT_BACKEND}`);
  return {
    backend,
    bytes: Array.from(
      decodeByteVecValue(
        fields.bytes,
        `${context}.bytes`,
        proofBoxMaxProofBytes(backend),
      ),
    ),
  };
}

function decodeVerifyingKeyBoxValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["backend", "bytes"]);
  return {
    backend: decodeStringValue(fields.backend, `${context}${TEXT_BACKEND}`),
    bytes: Array.from(decodeByteVecValue(fields.bytes, `${context}.bytes`)),
  };
}

function encodeBackendTagValue(value, context) {
  const backend = assertExactNonEmptyString(value, context);
  switch (backend) {
    case "halo2-ipa-pasta":
      return encodeEnumTagValue(0);
    case "stark":
      return encodeEnumTagValue(1);
    default:
      rejectError(`${context} uses unknown or non-canonical backend label ${backend}`);
  }
}

function decodeBackendTagValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("tag");
  reader.assertEof();
  switch (tag) {
    case 0:
      return "halo2-ipa-pasta";
    case 1:
      return "stark";
    default:
      rejectError(`${context}${TEXT_USES_UNSUPPORTED}backend tag ${tag}`);
  }
}

function encodeConfidentialStatusValue(value, context) {
  const normalized = assertNonEmptyString(value, context).toLowerCase();
  switch (normalized) {
    case "proposed":
      return encodeU8Value(0, context);
    case "active":
      return encodeU8Value(1, context);
    case "withdrawn":
      return encodeU8Value(2, context);
    default:
      rejectError(`${context}${TEXT_MUST_BE}Proposed, Active, or Withdrawn`);
  }
}

function decodeConfidentialStatusValue(payload, context) {
  const tag = decodeU8Value(payload, context);
  switch (tag) {
    case 0:
      return "Proposed";
    case 1:
      return "Active";
    case 2:
      return "Withdrawn";
    default:
      rejectError(`${context}${TEXT_USES_UNSUPPORTED}confidential status ${tag}`);
  }
}

function encodeVerifyingKeyRecordValue(value, context) {
  if (!isPlainObject(value)) {
    rejectType(`${context}${TEXT_MUST_BE_AN_OBJECT}`);
  }
  return encodeStructValue([
    [encodeU32Value(value.version, `${context}.version`)],
    [encodeNoritoStringValue(assertNonEmptyString(value.circuit_id, `${context}.circuit_id`))],
    [encodeOptionValue(value.owner_manifest_id, encodeNoritoStringValue, `${context}.owner_manifest_id`)],
    [encodeNoritoStringValue(assertNonEmptyString(value.namespace, `${context}.namespace`))],
    [encodeBackendTagValue(value.backend, `${context}${TEXT_BACKEND}`)],
    [encodeNoritoStringValue(assertNonEmptyString(value.curve, `${context}.curve`))],
    [encodeFixedBytesValue(value.public_inputs_schema_hash, 32, `${context}.${TEXT_PUBLIC_INPUTS_SCHEMA_HASH}`)],
    [encodeFixedBytesValue(value.commitment, 32, `${context}.${TEXT_COMMITMENT}`)],
    [encodeU32Value(value.vk_len, `${context}.vk_len`)],
    [encodeU32Value(value.max_proof_bytes, `${context}.max_proof_bytes`)],
    [encodeOptionValue(value.gas_schedule_id, encodeNoritoStringValue, `${context}.gas_schedule_id`)],
    [encodeOptionValue(value.metadata_uri_cid, encodeNoritoStringValue, `${context}.metadata_uri_cid`)],
    [encodeOptionValue(value.vk_bytes_cid, encodeNoritoStringValue, `${context}.vk_bytes_cid`)],
    [encodeOptionValue(value.activation_height, encodeU64NumberValue, `${context}.activation_height`)],
    [encodeOptionValue(value.withdraw_height, encodeU64NumberValue, `${context}.withdraw_height`)],
    [encodeOptionValue(value.key, encodeBackendBytesBoxValue, `${context}.key`)],
    [encodeConfidentialStatusValue(value.status, `${context}.status`)],
  ]);
}

function decodeVerifyingKeyRecordValue(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "version",
    "circuit_id",
    "owner_manifest_id",
    "namespace",
    "backend",
    "curve",
    TEXT_PUBLIC_INPUTS_SCHEMA_HASH,
    FIELD_COMMITMENT,
    "vk_len",
    "max_proof_bytes",
    "gas_schedule_id",
    "metadata_uri_cid",
    "vk_bytes_cid",
    "activation_height",
    "withdraw_height",
    "key",
    "status",
  ]);
  return {
    version: decodeU32Value(fields.version, `${context}.version`),
    circuit_id: decodeStringValue(fields.circuit_id, `${context}.circuit_id`),
    owner_manifest_id: decodeOptionValue(
      fields.owner_manifest_id,
      decodeStringValue,
      `${context}.owner_manifest_id`,
    ),
    namespace: decodeStringValue(fields.namespace, `${context}.namespace`),
    backend: decodeBackendTagValue(fields.backend, `${context}${TEXT_BACKEND}`),
    curve: decodeStringValue(fields.curve, `${context}.curve`),
    public_inputs_schema_hash: Array.from(
      decodeFixedBytesValue(
        fields.public_inputs_schema_hash,
        32,
        `${context}.${TEXT_PUBLIC_INPUTS_SCHEMA_HASH}`,
      ),
    ),
    commitment: Array.from(
      decodeFixedBytesValue(fields.commitment, 32, `${context}.${TEXT_COMMITMENT}`),
    ),
    vk_len: decodeU32Value(fields.vk_len, `${context}.vk_len`),
    max_proof_bytes: decodeU32Value(
      fields.max_proof_bytes,
      `${context}.max_proof_bytes`,
    ),
    gas_schedule_id: decodeOptionValue(
      fields.gas_schedule_id,
      decodeStringValue,
      `${context}.gas_schedule_id`,
    ),
    metadata_uri_cid: decodeOptionValue(
      fields.metadata_uri_cid,
      decodeStringValue,
      `${context}.metadata_uri_cid`,
    ),
    vk_bytes_cid: decodeOptionValue(
      fields.vk_bytes_cid,
      decodeStringValue,
      `${context}.vk_bytes_cid`,
    ),
    activation_height: decodeOptionValue(
      fields.activation_height,
      decodeU64NumberValue,
      `${context}.activation_height`,
    ),
    withdraw_height: decodeOptionValue(
      fields.withdraw_height,
      decodeU64NumberValue,
      `${context}.withdraw_height`,
    ),
    key: decodeOptionValue(
      fields.key,
      decodeVerifyingKeyBoxValue,
      `${context}.key`,
    ),
    status: decodeConfidentialStatusValue(fields.status, `${context}.status`),
  };
}

function encodeOpenVerifyEnvelopePayload(value, context) {
  if (!isPlainObject(value)) {
    rejectType(`${context}${TEXT_MUST_BE_AN_OBJECT}`);
  }
  assertOnlyObjectKeys(
    value,
    ["backend", "circuit_id", "vk_hash", "public_inputs", "proof_bytes", "aux"],
    context,
  );
  const circuitId = assertExactNonEmptyString(
    value.circuit_id,
    `${context}.circuit_id`,
  );
  if (circuitId.trim() !== circuitId) {
    rejectType(`${context}.circuit_id must not contain surrounding whitespace`);
  }
  return encodeStructValue([
    [encodeBackendTagValue(value.backend, `${context}${TEXT_BACKEND}`)],
    [encodeNoritoStringValue(circuitId)],
    [encodeFixedBytesValue(value.vk_hash, 32, `${context}.vk_hash`)],
    [encodeByteVecValue(value.public_inputs, `${context}.public_inputs`)],
    [encodeByteVecValue(value.proof_bytes, `${context}.proof_bytes`)],
    [encodeByteVecValue(value.aux ?? [], `${context}.aux`)],
  ]);
}

function decodeOpenVerifyEnvelopePayload(payload, context, flags = 0) {
  return withNoritoLengthFlags(flags & COMPACT_LEN_FLAG, () => {
    const fields = decodeStructFields(payload, context, [
      "backend",
      "circuit_id",
      "vk_hash",
      "public_inputs",
      "proof_bytes",
      "aux",
    ]);
    return {
      backend: decodeBackendTagValue(fields.backend, `${context}${TEXT_BACKEND}`),
      circuit_id: decodeStringValue(fields.circuit_id, `${context}.circuit_id`),
      vk_hash: Array.from(decodeFixedBytesValue(fields.vk_hash, 32, `${context}.vk_hash`)),
      public_inputs: Array.from(
        decodeByteVecValue(fields.public_inputs, `${context}.public_inputs`),
      ),
      proof_bytes: Array.from(
        decodeByteVecValue(fields.proof_bytes, `${context}.proof_bytes`),
      ),
      aux: Array.from(decodeByteVecValue(fields.aux, `${context}.aux`)),
    };
  });
}

function encodeProofAttachmentValue(value, context) {
  const attachment = normalizeCanonicalProofAttachmentValue(value, context);
  const parts = [
    encodeNoritoField(encodeNoritoStringValue(attachment.backend)),
    encodeNoritoField(encodeBackendBytesBoxValue(attachment.proof, `${context}.proof`)),
    encodeNoritoField(encodeVerifyingKeyIdValue(attachment.vk_ref, `${context}.vk_ref`)),
  ];
  const hasLanePrivacy = attachment.lane_privacy !== undefined && attachment.lane_privacy !== null;
  const hasEnvelopeHash = hasLanePrivacy || (attachment.envelope_hash !== undefined && attachment.envelope_hash !== null);
  const hasVkCommitment = hasEnvelopeHash || (attachment.vk_commitment !== undefined && attachment.vk_commitment !== null);
  if (hasVkCommitment) {
    parts.push(
      encodeNoritoField(
        encodeOptionValue(
          attachment.vk_commitment,
          (entry, innerContext) =>
            encodeFixedByteArrayArchiveValue(entry, 32, innerContext),
          `${context}.vk_${TEXT_COMMITMENT}`,
        ),
      ),
    );
  }
  if (hasEnvelopeHash) {
    parts.push(
      encodeNoritoField(
        encodeOptionValue(
          attachment.envelope_hash,
          (entry, innerContext) =>
            encodeFixedByteArrayArchiveValue(entry, 32, innerContext),
          `${context}.envelope_hash`,
        ),
      ),
    );
  }
  if (hasLanePrivacy) {
    parts.push(
      encodeNoritoField(
        encodeOptionValue(attachment.lane_privacy, encodeLanePrivacyProofValue, `${context}.lane_privacy`),
      ),
    );
  }
  return Buffer.concat(parts);
}

function decodeProofAttachmentValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const backend = decodeStringValue(readNoritoField(reader, "backend"), `${context}${TEXT_BACKEND}`);
  const proof = decodeProofBoxValue(readNoritoField(reader, "proof"), `${context}.proof`);
  const vk_ref = decodeVerifyingKeyIdValue(readNoritoField(reader, "vk_ref"), `${context}.vk_ref`);
  const vk_commitment =
    reader.offset < reader.buffer.length
      ? decodeOptionValue(
          readNoritoField(reader, ("vk_" + TEXT_COMMITMENT)),
          (entry, innerContext) =>
            Array.from(decodeFixedByteArrayArchiveValue(entry, 32, innerContext)),
          `${context}.vk_${TEXT_COMMITMENT}`,
        )
      : null;
  const envelope_hash =
    reader.offset < reader.buffer.length
      ? decodeOptionValue(
          readNoritoField(reader, "envelope_hash"),
          (entry, innerContext) =>
            Array.from(decodeFixedByteArrayArchiveValue(entry, 32, innerContext)),
          `${context}.envelope_hash`,
        )
      : null;
  const lane_privacy =
    reader.offset < reader.buffer.length
      ? decodeOptionValue(
          readNoritoField(reader, "lane_privacy"),
          decodeLanePrivacyProofValue,
          `${context}.lane_privacy`,
        )
      : null;
  reader.assertEof();
  return normalizeCanonicalProofAttachmentValue({
    backend,
    proof,
    vk_ref,
    vk_commitment,
    envelope_hash,
    lane_privacy,
  }, context);
}

function normalizeCanonicalProofAttachmentValue(value, context) {
  if (!isPlainObject(value)) {
    rejectType(`${context}${TEXT_MUST_BE_AN_OBJECT}`);
  }
  assertOnlyObjectKeys(
    value,
    ["backend", "proof", "vk_ref", ("vk_" + TEXT_COMMITMENT), "envelope_hash", "lane_privacy"],
    context,
  );
  for (const field of ["backend", "proof", "vk_ref"]) {
    if (!Object.prototype.hasOwnProperty.call(value, field)) {
      rejectType(`${context}.${field}${TEXT_IS_REQUIRED}`);
    }
  }
  const backend = assertPortableProofIdField(value.backend, `${context}${TEXT_BACKEND}`);
  const proof = normalizeCanonicalProofBoxValue(value.proof, backend, `${context}.proof`);
  const vkRef = normalizeCanonicalProofVerifyingKeyId(
    value.vk_ref,
    `${context}.vk_ref`,
  );
  if (vkRef.backend !== backend) {
    rejectType(`${context}.vk_ref.backend must match ${context}${TEXT_BACKEND}`);
  }

  const normalized = { backend, proof, vk_ref: vkRef };
  if (value.vk_commitment !== undefined && value.vk_commitment !== null) {
    normalized.vk_commitment = normalizeNonZeroProofDigest(
      value.vk_commitment,
      `${context}.vk_${TEXT_COMMITMENT}`,
    );
  }
  if (value.envelope_hash !== undefined && value.envelope_hash !== null) {
    const envelopeHash = normalizeNonZeroProofDigest(
      value.envelope_hash,
      `${context}.envelope_hash`,
    );
    const expected = Array.from(blake2b256(Buffer.from(proof.bytes)));
    expected[31] |= 1;
    if (!envelopeHash.every((byte, index) => byte === expected[index])) {
      rejectType(`${context}.envelope_hash must match proof bytes`);
    }
    normalized.envelope_hash = envelopeHash;
  }
  if (value.lane_privacy !== undefined && value.lane_privacy !== null) {
    normalized.lane_privacy = normalizeCanonicalLanePrivacyProofValue(
      value.lane_privacy,
      `${context}.lane_privacy`,
    );
  }
  return normalized;
}

function normalizeCanonicalProofBoxValue(value, backend, context) {
  if (!isPlainObject(value)) {
    rejectType(`${context}${TEXT_MUST_BE_AN_OBJECT}`);
  }
  assertExactObjectKeys(value, ["backend", "bytes"], context);
  const proofBackend = assertPortableProofIdField(value.backend, `${context}${TEXT_BACKEND}`);
  if (proofBackend !== backend) {
    rejectType(`${context}.backend must match the attachment backend`);
  }
  if (typeof value.bytes === JS_TYPE_STRING) {
    rejectType(`${context}.bytes${TEXT_MUST_BE}an exact non-empty byte sequence`);
  }
  const declaredLength = binaryByteLength(value.bytes);
  if (declaredLength !== null && declaredLength > proofBoxMaxProofBytes(backend)) {
    rejectRange(`${context}${TEXT_EXCEEDS_THE}complete ${PROOF_BOX_MAX_ENCODED_BYTES}-byte ProofBox limit`);
  }
  const bytes = Array.from(normalizeBytes(value.bytes));
  if (bytes.length === 0) {
    rejectType(`${context}.bytes${TEXT_MUST_NOT_BE}empty`);
  }
  if (!proofBoxFitsEncodedBudget(backend, bytes.length)) {
    rejectRange(`${context}${TEXT_EXCEEDS_THE}complete ${PROOF_BOX_MAX_ENCODED_BYTES}-byte ProofBox limit`);
  }
  return { backend: proofBackend, bytes };
}

function normalizeCanonicalProofVerifyingKeyId(value, context) {
  if (!isPlainObject(value)) {
    rejectType(`${context}${TEXT_MUST_BE_AN_OBJECT}`);
  }
  assertExactObjectKeys(value, ["backend", "name"], context);
  return {
    backend: assertPortableProofIdField(value.backend, `${context}${TEXT_BACKEND}`),
    name: assertPortableProofIdField(value.name, `${context}.name`),
  };
}

function assertPortableProofIdField(value, context) {
  if (!isPortableVerifyingKeyIdField(value)) {
    rejectType(`${context} must use portable verifier-key registry syntax`);
  }
  return value;
}

function normalizeNonZeroProofDigest(value, context) {
  const bytes = Array.from(encodeFixedBytesValue(value, 32, context));
  if (bytes.every((byte) => byte === 0)) {
    rejectType(`${context}${TEXT_MUST_BE}non-zero`);
  }
  return bytes;
}

function normalizeCanonicalLanePrivacyProofValue(value, context) {
  if (!isPlainObject(value)) {
    rejectType(`${context}${TEXT_MUST_BE_AN_OBJECT}`);
  }
  assertExactObjectKeys(value, ["commitment_id", "witness"], context);
  if (
    !Number.isInteger(value.commitment_id) ||
    value.commitment_id < 0 ||
    value.commitment_id > 0xffff
  ) {
    rejectRange(`${context}.commitment_id must fit within a u16`);
  }
  return {
    commitment_id: value.commitment_id,
    witness: normalizeCanonicalLanePrivacyWitnessValue(
      value.witness,
      `${context}.witness`,
    ),
  };
}

function normalizeCanonicalLanePrivacyWitnessValue(value, context) {
  if (!isPlainObject(value)) {
    rejectType(`${context}${TEXT_MUST_BE_AN_OBJECT}`);
  }
  assertExactObjectKeys(value, ["kind", "payload"], context);
  if (value.kind !== "merkle") {
    rejectType(`${context}.kind${TEXT_MUST_BE}exactly merkle`);
  }
  if (!isPlainObject(value.payload)) {
    rejectType(`${context}.payload${TEXT_MUST_BE_AN_OBJECT_2}`);
  }
  assertExactObjectKeys(value.payload, ["leaf", "proof"], `${context}.payload`);
  const leaf = Array.from(
    encodeFixedBytesValue(value.payload.leaf, 32, `${context}.payload.leaf`),
  );
  if (!isPlainObject(value.payload.proof)) {
    rejectType(`${context}.payload.proof${TEXT_MUST_BE_AN_OBJECT_2}`);
  }
  assertExactObjectKeys(
    value.payload.proof,
    ["leaf_index", "audit_path"],
    `${context}.payload.proof`,
  );
  const leafIndex = value.payload.proof.leaf_index;
  const auditPath = value.payload.proof.audit_path;
  if (
    !Array.isArray(auditPath) ||
    auditPath.length < 1 ||
    auditPath.length > LANE_PRIVACY_MERKLE_MAX_DEPTH
  ) {
    rejectRange(`${context}${TEXT_PAYLOAD_PROOF_AUDIT_PATH_MUST}contain 1..=${LANE_PRIVACY_MERKLE_MAX_DEPTH} siblings`);
  }
  if (!laneMerkleLeafIndexFitsDepth(leafIndex, auditPath.length)) {
    rejectRange(`${context}.payload.proof.leaf_index is impossible for the Merkle path depth`);
  }
  const canonicalPath = auditPath.map((entry, index) => {
    if (entry === null || entry === undefined) {
      rejectType(`${context}.payload.proof.audit_path[${index}]${TEXT_MUST_CONTAIN}a sibling`);
    }
    const siblingContext = `${context}.payload.proof.audit_path[${index}]`;
    const siblingBytes = encodeHashLiteralBytes(entry, siblingContext);
    if (typeof entry === JS_TYPE_STRING) {
      const canonical = decodeHashLiteral(siblingBytes, siblingContext);
      if (entry !== canonical) {
        rejectType(`${siblingContext}${TEXT_MUST_BE_A}canonical HashOf literal`);
      }
      return canonical;
    }
    const sibling = Array.from(siblingBytes);
    if ((sibling[31] & 1) === 0) {
      rejectType(`${siblingContext} is not a${TEXT_CANONICAL}prehashed HashOf`);
    }
    return sibling;
  });
  return {
    kind: "merkle",
    payload: {
      leaf,
      proof: { leaf_index: leafIndex, audit_path: canonicalPath },
    },
  };
}

function binaryByteLength(value) {
  if (Array.isArray(value) || Buffer.isBuffer(value)) {
    return value.length;
  }
  if (value instanceof ArrayBuffer || ArrayBuffer.isView(value)) {
    return value.byteLength;
  }
  return null;
}

function encodeLanePrivacyProofValue(value, context) {
  return encodeCanonicalRecordFields(value, context, LanePrivacyProofValueFields);
}

const LanePrivacyProofValueFields = [
    ["commitment_id", decodeU16Value, 0, encodeU16Value, 0],
    ["witness", decodeLanePrivacyWitnessValue, 0, encodeLanePrivacyWitnessValue, 0],
  ];

  function decodeLanePrivacyProofValue(payload, context) {
    return decodeRecordFields(payload, context, LanePrivacyProofValueFields);
  }

function encodeLanePrivacyWitnessValue(value, context) {
  if (!isPlainObject(value)) {
    rejectType(`${context}${TEXT_MUST_BE_AN_OBJECT}`);
  }
  const kind = assertNonEmptyString(value.kind, `${context}.kind`).toLowerCase();
  if (kind === "merkle") {
    const auditPath =
      value.payload?.proof?.audit_path ?? value.payload?.proof?.auditPath;
    if (!Array.isArray(auditPath) || auditPath.length === 0) {
      rejectError(`${context}${TEXT_PAYLOAD_PROOF_AUDIT_PATH_MUST}contain at least one sibling`);
    }
    if (auditPath.some((entry) => entry === null || entry === undefined)) {
      rejectError(`${context}${TEXT_PAYLOAD_PROOF_AUDIT_PATH_MUST}not omit siblings`);
    }
    return encodeEnumTagValue(0, () =>
      encodeStructValue([
        [encodeFixedBytesValue(value.payload.leaf, 32, `${context}.payload.leaf`)],
        [encodeMerkleProofValue(value.payload.proof, `${context}.payload.proof`)],
      ]),
    );
  }
  rejectError(`${context}.kind${TEXT_MUST_BE}merkle`);
}

function decodeLanePrivacyWitnessValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("tag");
  const body = reader.offset < reader.buffer.length ? readNoritoField(reader, "body") : null;
  reader.assertEof();
  switch (tag) {
    case 0: {
      const fields = decodeStructFields(body ?? Buffer.alloc(0), `${context}.merkle`, [
        "leaf",
        "proof",
      ]);
      const proof = decodeMerkleProofValue(fields.proof, `${context}.payload.proof`);
      if (proof.audit_path.length === 0) {
        rejectError(`${context}${TEXT_PAYLOAD_PROOF_AUDIT_PATH_MUST}contain at least one sibling`);
      }
      if (proof.audit_path.some((entry) => entry === null)) {
        rejectError(`${context}${TEXT_PAYLOAD_PROOF_AUDIT_PATH_MUST}not omit siblings`);
      }
      return {
        kind: "merkle",
        payload: {
          leaf: Array.from(decodeFixedBytesValue(fields.leaf, 32, `${context}.payload.leaf`)),
          proof,
        },
      };
    }
    default:
      rejectError(`${context}${TEXT_USES_UNSUPPORTED}lane privacy witness ${tag}`);
  }
}

  const decodeRecordFields = /* @__PURE__ */ createNoritoRecordDecoder(
    decodeStructFields, decodeOptionValue, decodeNoritoVec,
  );

const [encodeMerkleProofValue, decodeMerkleProofValue] =
  /* @__PURE__ */ createNoritoMerkleProofCodecs(
    LANE_PRIVACY_MERKLE_MAX_DEPTH, decodeHashValue, decodeNoritoVec,
    decodeOptionValue, decodeTupleFields, decodeU32Value, encodeHashLiteralBytes,
    encodeNoritoVec, encodeOptionValue, encodeTupleValue, encodeU32Value,
  );

const confidentialMemoValueCodecs =
  /* @__PURE__ */ createNoritoConfidentialMemoCodecs(
    BufferReader, decodeUnsignedLeb128, encodeCompactLength, encodeFixedBytesValue,
    encodeU8Value, isPlainObject, normalizeFlexibleBytes,
  );

const [
  encodeContractManifestSignaturePayloadValue,
  encodeContractManifestValue,
  decodeContractManifestValue,
  encodeManifestProvenanceValue,
  decodeManifestProvenanceValue,
] = /* @__PURE__ */ createNoritoContractCodecs(
  BufferReader, assertNonEmptyString, assertOnlyObjectKeys,
  decodeAccountIdValue, decodeBoolValue, decodeConstVecU8Value,
  decodeEventFilterBoxFramePayload, decodeHashValue, decodeMetadataValue,
  decodeNameValue, decodeNoritoVec, decodeOptionValue,
  decodePublicKeyValue, decodeStringValue, decodeStructFields,
  decodeU16Value, decodeU32Value, decodeU64NumberValue,
  decodeU8Value, encodeAccountIdValue, encodeBoolValue,
  encodeConstVecU8Value, encodeEnumTagValue, encodeEventFilterBoxFramePayload,
  encodeHashValue, encodeMetadataValue, encodeNameValue,
  encodeNoritoStringValue, encodeNoritoVec, encodeOptionValue,
  encodePublicKeyValue, encodeStringValue, encodeStructValue,
  encodeU16Value, encodeU32Value, encodeU64NumberValue,
  encodeU8Value, isPlainObject, parsePublicKeyLiteral,
  publicKeyLiteralFromParts, readNoritoField,
);
const nftMarketCodecsV1 = /* @__PURE__ */ createNoritoNftMarketCodecs({
  encodeStructValue, decodeStructFields, encodeEscrowIdValue, decodeEscrowIdValue,
  encodeNftIdValue, decodeNftIdValue, encodeAccountIdValue, decodeAccountIdValue,
  encodeAssetDefinitionIdValue, decodeAssetDefinitionIdValue, encodeQuantityValue, decodeQuantityValue,
  encodeMetadataValue, decodeMetadataValue, encodeU16Value, decodeU16Value, encodeU32Value, encodeU64Value, decodeU64Value,
  encodeOptionValue, decodeOptionValue,
});
/** Canonical bounded native NFT offer and metadata value encoding. */
export function noritoEncodeNftMarketValueV1(name, value) {
  return withNoritoLengthFlags(COMPACT_LEN_FLAG, () => {
    const bytes = nftMarketCodecsV1.encode(name, value);
    if (bytes.length > 64 * 1024) throw new RangeError("native NFT value exceeds bound");
    return bytes;
  });
}
/** Exact decode, rejecting trailing bytes, alternate layouts and unknown fields. */
export function noritoDecodeNftMarketValueV1(name, value) {
  const bytes = toBuffer(value);
  if (bytes.length > 64 * 1024) throw new RangeError("native NFT value exceeds bound");
  return withNoritoLengthFlags(COMPACT_LEN_FLAG, () => {
    const decoded = nftMarketCodecsV1.decode(name, bytes);
    if (!nftMarketCodecsV1.encode(name, decoded).equals(bytes)) throw new TypeError("native NFT value is not byte-canonical");
    return decoded;
  });
}
// Resource primitives keep the same compact layout and exact re-encoding checks
// as public values, without importing the complete application value catalog.
const resourceCodecs = /* @__PURE__ */ createNoritoGameResourceEngine({
  encodePrimitive(name, value) {
    if (name !== "u8") return noritoEncodeNftMarketValueV1(name, value);
    return withNoritoLengthFlags(COMPACT_LEN_FLAG, () => {
      const bytes = instructionGameCodecsV1.encode(name, value);
      if (bytes.length > gameValueMaximumBytesV1(name)) {
        throw new RangeError(TEXT_NATIVE_GAME_VALUE_EXCEEDS_ITS_COMPILED_PAYLOAD_LIMIT);
      }
      return bytes;
    });
  },
  decodePrimitive(name, value) {
    if (name !== "u8") return noritoDecodeNftMarketValueV1(name, value);
    const bytes = toBuffer(value);
    if (bytes.length > gameValueMaximumBytesV1(name)) {
      throw new RangeError(TEXT_NATIVE_GAME_VALUE_EXCEEDS_ITS_COMPILED_PAYLOAD_LIMIT);
    }
    return withNoritoLengthFlags(COMPACT_LEN_FLAG, () => {
      const decoded = instructionGameCodecsV1.decode(name, bytes);
      if (!instructionGameCodecsV1.encode(name, decoded).equals(bytes)) {
        throw new TypeError(TEXT_NATIVE_GAME_VALUE_IS_NOT_BYTE_CANONICAL);
      }
      return decoded;
    });
  },
});
/** Encode one exact bounded resource value using the canonical primitive owner. */
export function encodeGameResourceValueV1(name, value) {
  return resourceCodecs.encode(name, value);
}
/** Decode one bounded resource value with byte-canonical re-encoding. */
export function decodeGameResourceValueV1(name, value) {
  return resourceCodecs.decode(name, value);
}
const gamePrimitivesV1 = {
  resourceCodecs,
  encodeNftIdValue, decodeNftIdValue,
  encodeStructValue, decodeStructFields, encodeNoritoVec, decodeNoritoVec,
  encodeEscrowIdValue, decodeEscrowIdValue,
  encodeAssetDefinitionIdValue, decodeAssetDefinitionIdValue,
  encodeQuantityValue, decodeQuantityValue, encodeBoolValue, decodeBoolValue,
  encodeU8Value, encodeU16Value, encodeU32Value, encodeU64Value,
  decodeU8Value, decodeU16Value, decodeU32Value, decodeU64Value,
  encodePublicKeyValue, decodePublicKeyValue, parsePublicKeyLiteral, publicKeyLiteralFromParts,
  encodeConstVecU8Value, decodeConstVecU8Value, encodeByteVecValue, decodeByteVecValue,
  encodeOptionValue, decodeOptionValue, encodeAccountIdValue, decodeAccountIdValue, encodeEnumTagValue,
};
const instructionGameCodecsV1 = /* @__PURE__ */ createNoritoGameInstructionCodecs(gamePrimitivesV1);
const gameCodecsV1 = /* @__PURE__ */ createNoritoGameCodecs(gamePrimitivesV1);

/** Encode one exact native game or compiled adapter value with the consensus bare compact layout. */
export function noritoEncodeGameValueV1(name, value) {
  return withNoritoLengthFlags(COMPACT_LEN_FLAG, () => {
    const bytes = gameCodecsV1.encode(name, value);
    if (bytes.length > gameValueMaximumBytesV1(name)) throw new RangeError(TEXT_NATIVE_GAME_VALUE_EXCEEDS_ITS_COMPILED_PAYLOAD_LIMIT);
    return bytes;
  });
}

/** Decode an exact native game or compiled adapter value and reject noncanonical byte encodings. */
export function noritoDecodeGameValueV1(name, value) {
  const bytes = toBuffer(value);
  if (bytes.length > gameValueMaximumBytesV1(name)) throw new RangeError(TEXT_NATIVE_GAME_VALUE_EXCEEDS_ITS_COMPILED_PAYLOAD_LIMIT);
  return withNoritoLengthFlags(COMPACT_LEN_FLAG, () => {
    const decoded = gameCodecsV1.decode(name, bytes);
    if (!gameCodecsV1.encode(name, decoded).equals(bytes)) throw new TypeError(TEXT_NATIVE_GAME_VALUE_IS_NOT_BYTE_CANONICAL);
    return decoded;
  });
}

function encodeEventFilterBoxFramePayload(value, context) {
  const frameBytes = decodeExactStandardBase64(value, context);
  const frame = decodeNoritoFrame(frameBytes, context, EVENT_FILTER_BOX_SCHEMA_HASH);
  const expectedFlags = noritoLengthFlags & COMPACT_LEN_FLAG;
  if (frame.flags !== expectedFlags) {
    rejectError(`${context} uses Norito layout flags ${frame.flags}; expected ${expectedFlags}`);
  }
  const canonical = frameNoritoPayload(
    frame.payload,
    EVENT_FILTER_BOX_SCHEMA_HASH,
    frame.flags,
  );
  if (!canonical.equals(frameBytes)) {
    rejectError(`${context}${TEXT_MUST_BE_A}canonical unpadded EventFilterBox frame`);
  }
  return frame.payload;
}

function decodeEventFilterBoxFramePayload(payload, _context) {
  return frameNoritoPayload(
    payload,
    EVENT_FILTER_BOX_SCHEMA_HASH,
    noritoLengthFlags & COMPACT_LEN_FLAG,
  ).toString(BASE64_ENCODING);
}

function decodeExactStandardBase64(value, context) {
  if (
    typeof value !== JS_TYPE_STRING ||
    value.length === 0 ||
    value.trim() !== value ||
    value.length % 4 !== 0 ||
    !/^[A-Za-z0-9+/]*={0,2}$/u.test(value)
  ) {
    rejectType(`${context}${TEXT_MUST_BE_EXACT_STANDARD_BASE64}`);
  }
  const bytes = Buffer.from(value, BASE64_ENCODING);
  if (bytes.length === 0 || bytes.toString(BASE64_ENCODING) !== value) {
    rejectType(`${context}${TEXT_MUST_BE_EXACT_STANDARD_BASE64}`);
  }
  return bytes;
}

function assertOnlyObjectKeys(value, allowedKeys, context) {
  const allowed = new Set(allowedKeys);
  const unknown = Object.keys(value).find((key) => !allowed.has(key));
  if (unknown !== undefined) {
    rejectType(`${context} contains unknown field ${unknown}`);
  }
}

function encodeQuantityValue(value, context) {
  const { mantissa, scale } = parseNumericLiteral(value, context);
  const mantissaBytes = bigintToTwosBytes(mantissa);
  const mantissaPayload = Buffer.concat([
    u32ToLittleEndianBuffer(mantissaBytes.length),
    mantissaBytes,
  ]);
  return Buffer.concat([
    encodeNoritoField(mantissaPayload),
    encodeNoritoField(u32ToLittleEndianBuffer(scale)),
  ]);
}

/** @internal Exact compact-length Quantity value encoding for typed policy codecs. */
export function encodeQuantityNoritoValue(value, context = "Quantity") {
  return withNoritoCompactLengths(() =>
    Uint8Array.from(encodeQuantityValue(value, context)),
  );
}

// Low-level wire decoder retained for the NumericV1-backed Quantity payload.
function decodeNumericValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const mantissaPayload = readNoritoField(reader, "mantissa");
  const scalePayload = readNoritoField(reader, "scale");
  reader.assertEof();

  const mantissaReader = new BufferReader(mantissaPayload, `${context}.mantissa`);
  const byteLength = mantissaReader.readU32LE("byteLength");
  if (byteLength > NumericV1.MAX_MANTISSA_BYTES) {
    rejectRange(`${context}.mantissa${TEXT_EXCEEDS_THE}signed 512-bit bound`);
  }
  const bytes = mantissaReader.readBytes(byteLength, "bytes");
  mantissaReader.assertEof();
  if (bytes.length === 1 && bytes[0] === 0) {
    rejectType(`${context}.mantissa uses a noncanonical zero encoding`);
  }
  if (bytes.length > 1) {
    const last = bytes[bytes.length - 1];
    const previous = bytes[bytes.length - 2];
    if ((last === 0 && (previous & 0x80) === 0)
      || (last === 0xff && (previous & 0x80) !== 0)) {
      rejectType(`${context}.mantissa has redundant sign extension`);
    }
  }

  const scaleReader = new BufferReader(scalePayload, `${context}.scale`);
  const scale = scaleReader.readU32LE("value");
  scaleReader.assertEof();
  if (scale > NumericV1.MAX_SCALE) {
    rejectRange(`${context}.scale exceeds ${NumericV1.MAX_SCALE}`);
  }

  const mantissa = twosBytesToBigInt(bytes);
  return NumericV1.decodeQuantityJson(formatNumericLiteral(mantissa, scale)).toString();
}

function decodeQuantityValue(payload, context) {
  const literal = decodeNumericValue(payload, context);
  return NumericV1.decodeQuantityJson(literal).toString();
}

function encodeU8Value(value, context) {
  const normalized = Number(value);
  if (!Number.isInteger(normalized) || normalized < 0 || normalized > 0xff) {
    rejectType(`${context}${TEXT_MUST_BE}an unsigned 8-bit integer`);
  }
  return Buffer.of(normalized);
}

function decodeU8Value(payload, context) {
  if (payload.length !== 1) {
    rejectError(`${context}${TEXT_MUST_CONTAIN_EXACTLY}one byte`);
  }
  return payload[0];
}

function encodeU16Value(value, context) {
  const normalized = Number(value);
  if (!Number.isInteger(normalized) || normalized < 0 || normalized > 0xffff) {
    rejectType(`${context}${TEXT_MUST_BE}an unsigned 16-bit integer`);
  }
  return u16ToLittleEndianBuffer(normalized);
}

function decodeU16Value(payload, context) {
  if (payload.length !== 2) {
    rejectError(`${context}${TEXT_MUST_CONTAIN_EXACTLY}two bytes`);
  }
  return payload.readUInt16LE(0);
}

function encodeU32Value(value, context) {
  const normalized = Number(value);
  if (!Number.isInteger(normalized) || normalized < 0 || normalized > 0xffff_ffff) {
    rejectType(`${context}${TEXT_MUST_BE}an unsigned 32-bit integer`);
  }
  return u32ToLittleEndianBuffer(normalized);
}

function decodeU32Value(payload, context) {
  if (payload.length !== 4) {
    rejectError(`${context}${TEXT_MUST_CONTAIN_EXACTLY}four bytes`);
  }
  return payload.readUInt32LE(0);
}

function encodeU64Value(value, context) {
  const bigint = normalizeU64Input(value, context);
  return u64ToLittleEndianBuffer(bigint);
}

function decodeU64Value(payload, context) {
  if (payload.length !== 8) {
    rejectError(`${context}${TEXT_MUST_CONTAIN_EXACTLY}eight bytes`);
  }
  return payload.readBigUInt64LE(0).toString();
}

function encodeNoritoStringValue(value) {
  return encodeNoritoField(Buffer.from(value, UTF8_ENCODING));
}

function encodeExactBase64StringValue(value, context) {
  if (typeof value !== JS_TYPE_STRING) {
    rejectType(`${context}${TEXT_MUST_BE_A}string`);
  }
  if (value.length === 0 || value.trim() !== value || /\s/u.test(value)) {
    rejectType(`${context}${TEXT_MUST_BE_EXACT_STANDARD_BASE64}`);
  }
  if (!/^[A-Za-z0-9+/]*={0,2}$/u.test(value) || value.length % 4 !== 0) {
    rejectType(`${context}${TEXT_MUST_BE_EXACT_STANDARD_BASE64}`);
  }
  const decoded = Buffer.from(value, BASE64_ENCODING);
  if (decoded.length === 0 || decoded.toString(BASE64_ENCODING) !== value) {
    rejectType(`${context}${TEXT_MUST_BE_EXACT_STANDARD_BASE64}`);
  }
  return encodeNoritoStringValue(value);
}

function decodeStringValue(payload, context, lengthFlags = noritoLengthFlags) {
  const reader = new BufferReader(payload, context, lengthFlags);
  const stringBytes = readNoritoField(reader, "value");
  reader.assertEof();
  return stringBytes.toString(UTF8_ENCODING);
}

function encodeNoritoJsonValue(value) {
  return encodeStructValue([
    [encodeNoritoStringValue(canonicalJsonStringify(value))],
  ]);
}

function decodeJsonValue(payload, context) {
  const fields = decodeTupleFields(payload, context, ["value"]);
  return JSON.parse(decodeStringValue(fields.value, `${context}.value`));
}

function readNoritoField(reader, name) {
  const length = reader.readLength(`${name}.length`);
  return reader.readBytes(length, `${name}.payload`);
}

function encodeNoritoField(payload) {
  return Buffer.concat([encodeNoritoLength(payload.length), payload]);
}

function encodeNoritoVec(values, encode) {
  const payloads = values.map(encode);
  const parts = [u64ToLittleEndianBuffer(payloads.length)];
  for (const payload of payloads) {
    parts.push(encodeNoritoLength(payload.length), payload);
  }
  return Buffer.concat(parts);
}

function withNoritoCompactLengths(fn) {
  return withNoritoLengthFlags(COMPACT_LEN_FLAG, fn);
}

function withNoritoU64Lengths(fn) {
  return withNoritoLengthFlags(0, fn);
}

function withNoritoLengthFlags(flags, fn) {
  const previous = noritoLengthFlags;
  noritoLengthFlags = flags;
  try {
    return fn();
  } finally {
    noritoLengthFlags = previous;
  }
}

function encodeNoritoLength(value) {
  if ((noritoLengthFlags & COMPACT_LEN_FLAG) !== 0) {
    return encodeUnsignedLeb128(value);
  }
  return u64ToLittleEndianBuffer(value);
}

function decodeNoritoVec(payload, decode, context, maxCount = null) {
  const reader = new BufferReader(payload, context, noritoLengthFlags);
  const count = bigintToSafeNumber(reader.readU64LE("count"), `${context}.count`);
  if (maxCount !== null && count > maxCount) {
    rejectRange(`${context}${TEXT_EXCEEDS_THE}${maxCount}-item limit`);
  }
  const values = [];
  for (let index = 0; index < count; index += 1) {
    const itemPayload = readNoritoField(reader, `item${index}`);
    values.push(decode(itemPayload, index));
  }
  reader.assertEof();
  return values;
}

function schemaHashForTypeName(typeName) {
  const input = Uint8Array.from(
    Buffer.concat([
      Buffer.from("norito:v1:type-name\0", UTF8_ENCODING),
      Buffer.from(typeName, UTF8_ENCODING),
    ]),
  );
  const digest = sha256(
    input,
  );
  return Buffer.from(digest.subarray(0, 16));
}

/**
 * Validate one canonical, uncompressed Norito v1 frame without decoding its payload.
 *
 * The schema can be bound either by its exact hash or by the Rust type name from
 * which Norito derives that hash. The returned payload is a view over the input.
 *
 * @param {ArrayBufferView | ArrayBuffer | Buffer} bytes
 * @param {{
 *   context?: string,
 *   expectedSchemaHash?: ArrayBufferView | ArrayBuffer | Buffer,
 *   expectedTypeName?: string,
 *   expectedPaddingLength?: number,
 *   requireNonEmptyPayload?: boolean,
 * }} [options]
 * @returns {{payload: Buffer, schemaHash: Buffer, flags: number}}
 */
export function validateNoritoFrame(bytes, options = {}) {
  const context = options.context ?? "Norito frame";
  const buffer = toBuffer(bytes);
  if (buffer.length < NORITO_FRAME_HEADER_LENGTH) {
    rejectError(`${context} is shorter than the ${NORITO_FRAME_HEADER_LENGTH}-byte Norito header`);
  }
  if (buffer.subarray(0, 4).toString("ascii") !== "NRT0") {
    rejectError(`${context} is not an NRT0 frame`);
  }
  const major = buffer[4];
  const minor = buffer[5];
  if (major !== 0 || minor !== 0) {
    rejectError(`${context}${TEXT_USES_UNSUPPORTED}NRT0 version ${major}.${minor}`);
  }

  const schemaHash = buffer.subarray(6, 22);
  if (schemaHash.every((byte) => byte === 0)) {
    rejectError(`${context} uses the reserved all-zero schema hash`);
  }
  let expectedSchemaHash = null;
  if (options.expectedSchemaHash !== undefined) {
    expectedSchemaHash = toBuffer(options.expectedSchemaHash);
    if (expectedSchemaHash.length !== 16) {
      rejectType(`${context} expected schema hash${TEXT_MUST_CONTAIN}exactly 16 bytes`);
    }
  }
  if (options.expectedTypeName !== undefined) {
    if (
      typeof options.expectedTypeName !== JS_TYPE_STRING ||
      options.expectedTypeName.length === 0
    ) {
      rejectType(`${context} expected Rust type name${TEXT_MUST_BE}non-empty`);
    }
    const fromTypeName = /* @__PURE__ */ schemaHashForTypeName(options.expectedTypeName);
    if (expectedSchemaHash !== null && !expectedSchemaHash.equals(fromTypeName)) {
      rejectType(`${context} expected schema constraints contradict each other`);
    }
    expectedSchemaHash = fromTypeName;
  }
  if (expectedSchemaHash !== null && !schemaHash.equals(expectedSchemaHash)) {
    rejectError(`${context} schema hash did not match the expected type`);
  }

  const compression = buffer[22];
  if (compression !== 0) {
    rejectError(`${context} must use uncompressed Norito payload encoding`);
  }
  const payloadLength = bigintToSafeNumber(
    buffer.readBigUInt64LE(23),
    `${context}.payloadLength`,
  );
  if (options.requireNonEmptyPayload === true && payloadLength === 0) {
    rejectError(`${context}${TEXT_MUST_CONTAIN}a non-empty Norito payload`);
  }
  const expectedCrc = buffer.readBigUInt64LE(31);
  const flags = buffer[39];
  if ((flags & ~NORITO_SUPPORTED_HEADER_FLAGS) !== 0) {
    rejectError(`${context}${TEXT_USES_UNSUPPORTED}Norito header flags 0x${flags.toString(16)}`);
  }
  if (
    (flags & NORITO_FIELD_BITSET_FLAG) !== 0 &&
    (flags & (NORITO_PACKED_STRUCT_FLAG | COMPACT_LEN_FLAG)) !==
      (NORITO_PACKED_STRUCT_FLAG | COMPACT_LEN_FLAG)
  ) {
    rejectError(`${context} uses an invalid Norito header flag combination`);
  }

  const paddingLength = buffer.length - NORITO_FRAME_HEADER_LENGTH - payloadLength;
  if (paddingLength < 0) {
    rejectError(`${context} payload length${TEXT_EXCEEDS_THE}available frame bytes`);
  }
  if (paddingLength > NORITO_MAX_HEADER_PADDING) {
    rejectError(`${context}${TEXT_EXCEEDS_THE}${NORITO_MAX_HEADER_PADDING}-byte Norito header-padding bound`);
  }
  if (options.expectedPaddingLength !== undefined) {
    if (
      !Number.isInteger(options.expectedPaddingLength) ||
      options.expectedPaddingLength < 0 ||
      options.expectedPaddingLength > NORITO_MAX_HEADER_PADDING
    ) {
      rejectType(`${context} expected padding length${TEXT_MUST_BE}an integer from 0 through ${NORITO_MAX_HEADER_PADDING}`);
    }
    if (paddingLength !== options.expectedPaddingLength) {
      rejectError(`${context}${TEXT_MUST_CONTAIN_EXACTLY}${options.expectedPaddingLength} bytes of header padding`);
    }
  }
  const payloadStart = NORITO_FRAME_HEADER_LENGTH + paddingLength;
  const padding = buffer.subarray(NORITO_FRAME_HEADER_LENGTH, payloadStart);
  if (padding.some((byte) => byte !== 0)) {
    rejectError(`${context} contains non-zero alignment padding or trailing bytes`);
  }
  const payload = buffer.subarray(payloadStart, payloadStart + payloadLength);
  if (payload.length !== payloadLength || payloadStart + payload.length !== buffer.length) {
    rejectError(`${context} contains trailing bytes outside the declared payload`);
  }
  const actualCrc = crc64Xz(payload);
  if (actualCrc !== expectedCrc) {
    rejectError(`${context} CRC64 mismatch`);
  }
  return { payload, schemaHash, flags };
}

function decodeNoritoFrame(buffer, context, expectedSchemaHash) {
  if (buffer.length < NORITO_FRAME_HEADER_LENGTH) {
    // Preserve the established decoder diagnostic while the exported preflight
    // helper reports the more specific SCCP-facing short-header error.
    rejectError(`${context} reader overran payload while reading Norito header`);
  }
  return validateNoritoFrame(buffer, {
    context,
    ...(expectedSchemaHash == null ? {} : { expectedSchemaHash }),
  });
}

function frameNoritoPayload(payload, schemaHash, flags = 0, padding = 0) {
  const header = Buffer.concat([
    Buffer.from("NRT0", "ascii"),
    Buffer.from([0, 0]),
    schemaHash,
    Buffer.from([0]),
    u64ToLittleEndianBuffer(payload.length),
    u64ToLittleEndianBuffer(crc64Xz(payload)),
    Buffer.from([flags & 0xff]),
  ]);
  return Buffer.concat([header, Buffer.alloc(padding), payload]);
}

function u16ToLittleEndianBuffer(value) {
  const buffer = Buffer.allocUnsafe(2);
  buffer.writeUInt16LE(value, 0);
  return buffer;
}

function u32ToLittleEndianBuffer(value) {
  const buffer = Buffer.allocUnsafe(4);
  buffer.writeUInt32LE(value, 0);
  return buffer;
}

function u64ToLittleEndianBuffer(value) {
  const buffer = Buffer.allocUnsafe(8);
  buffer.writeBigUInt64LE(normalizeU64Input(value, "u64"), 0);
  return buffer;
}

function normalizeU64Input(value, context) {
  if (typeof value === JS_TYPE_BIGINT) {
    if (value < 0n || value > UINT64_MASK) {
      rejectRange(`${context}${TEXT_MUST_FIT_IN_AN_UNSIGNED_64_BIT_INTEGER}`);
    }
    return value;
  }
  if (typeof value === JS_TYPE_NUMBER) {
    if (!Number.isInteger(value) || value < 0 || !Number.isSafeInteger(value)) {
      rejectType(`${context}${TEXT_MUST_BE_A}non-negative safe integer or bigint`);
    }
    return BigInt(value);
  }
  if (typeof value === JS_TYPE_STRING && /^\d+$/.test(value.trim())) {
    const parsed = BigInt(value.trim());
    if (parsed > UINT64_MASK) {
      rejectRange(`${context}${TEXT_MUST_FIT_IN_AN_UNSIGNED_64_BIT_INTEGER}`);
    }
    return parsed;
  }
  rejectType(`${context}${TEXT_MUST_BE_A}bigint, integer number, or decimal string`);
}

function bigintToSafeNumber(value, context) {
  if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
    rejectRange(`${context}${TEXT_EXCEEDS_JAVA_SCRIPT_S_SAFE_INTEGER_RANGE}`);
  }
  return Number(value);
}

function parseNumericLiteral(value, context) {
  let quantity;
  if (value instanceof KotodamaQuantity) {
    quantity = new KotodamaQuantity(value.mantissa, value.scale);
  } else if (typeof value === JS_TYPE_STRING) {
    quantity = NumericV1.decodeQuantityJson(value);
  } else if (typeof value === JS_TYPE_BIGINT) {
    quantity = new KotodamaQuantity(value, 0);
  } else {
    rejectType(`${context}${TEXT_MUST_BE_A}KotodamaQuantity,${TEXT_CANONICAL}quantity string, or bigint; JavaScript numbers are rejected`);
  }
  return { mantissa: quantity.mantissa, scale: quantity.scale };
}

function formatNumericLiteral(mantissa, scale) {
  const negative = mantissa < 0n;
  let digits = (negative ? -mantissa : mantissa).toString();
  if (scale === 0) {
    return `${negative ? "-" : ""}${digits}`;
  }
  while (digits.length <= scale) {
    digits = `0${digits}`;
  }
  const split = digits.length - scale;
  return `${negative ? "-" : ""}${digits.slice(0, split)}.${digits.slice(split)}`;
}

function bigintToTwosBytes(value) {
  if (value === 0n) {
    return Buffer.alloc(0);
  }

  if (value > 0n) {
    const bytes = [];
    let remaining = value;
    while (remaining > 0n) {
      bytes.push(Number(remaining & 0xffn));
      remaining >>= 8n;
    }
    if ((bytes[bytes.length - 1] & 0x80) !== 0) {
      bytes.push(0);
    }
    return Buffer.from(bytes);
  }

  let byteLength = 1;
  while (value < -(1n << BigInt(byteLength * 8 - 1))) {
    byteLength += 1;
  }
  let encoded = (1n << BigInt(byteLength * 8)) + value;
  const bytes = [];
  for (let index = 0; index < byteLength; index += 1) {
    bytes.push(Number(encoded & 0xffn));
    encoded >>= 8n;
  }
  while (bytes.length > 1 && bytes[bytes.length - 1] === 0xff && (bytes[bytes.length - 2] & 0x80) !== 0) {
    bytes.pop();
  }
  return Buffer.from(bytes);
}

function twosBytesToBigInt(bytes) {
  if (bytes.length === 0) {
    return 0n;
  }
  let value = 0n;
  for (let index = bytes.length - 1; index >= 0; index -= 1) {
    value = (value << 8n) | BigInt(bytes[index]);
  }
  if ((bytes[bytes.length - 1] & 0x80) !== 0) {
    value -= 1n << BigInt(bytes.length * 8);
  }
  return value;
}

function publicKeyLiteralFromParts(curve, publicKey, context) {
  ensureCurveIdEnabled(curve, context);
  const bytes = Buffer.from(normalizeBytes(publicKey));
  validatePublicKeyForCurve(curve, bytes, context);
  const multicodec = publicKeyMulticodecForCurveId(curve);
  if (multicodec === null) {
    rejectError(`${context}${TEXT_USES_UNSUPPORTED}public-key curve ${curve}`);
  }
  const prefixHex = Buffer.concat([
    encodeUnsignedLeb128(multicodec),
    encodeUnsignedLeb128(bytes.length),
  ]).toString(HEX_ENCODING);
  return `${prefixHex}${bytes.toString(HEX_ENCODING).toUpperCase()}`;
}

function parsePublicKeyLiteral(literal, context) {
  const normalized = assertNonEmptyString(literal, context);
  if (!MULTIHASH_LITERAL_RE.test(normalized) || normalized.length % 2 !== 0) {
    rejectError(`${context}${TEXT_MUST_BE_A}canonical public-key multihash literal`);
  }
  const bytes = Buffer.from(normalized, HEX_ENCODING);
  let offset = 0;
  const [multicodec, multicodecBytes] = decodeUnsignedLeb128(bytes, offset, `${context}.multicodec`);
  offset += multicodecBytes;
  const [payloadLength, payloadLengthBytes] = decodeUnsignedLeb128(bytes, offset, `${context}.length`);
  offset += payloadLengthBytes;
  const remaining = bytes.subarray(offset);
  if (remaining.length !== payloadLength) {
    rejectError(`${context} public-key multihash length header is invalid`);
  }
  const curve = curveIdForMulticodec(multicodec, context);
  const publicKey = remaining;
  ensureCurveIdEnabled(curve, context);
  validatePublicKeyForCurve(curve, publicKey, context);
  return { curve, publicKey: Buffer.from(publicKey) };
}

function encodeUnsignedLeb128(value) {
  let remaining = BigInt(value);
  const bytes = [];
  while (remaining >= 0x80n) {
    bytes.push(Number((remaining & 0x7fn) | 0x80n));
    remaining >>= 7n;
  }
  bytes.push(Number(remaining));
  return Buffer.from(bytes);
}

function decodeUnsignedLeb128(buffer, offset, context) {
  let value = 0n;
  let shift = 0n;
  let cursor = offset;
  for (let used = 0; used < 10 && cursor < buffer.length; used += 1) {
    const byte = BigInt(buffer[cursor]);
    cursor += 1;
    if (used === 9 && (byte & 0xfen) !== 0n) {
      rejectRange(`${context}${TEXT_VARINT_EXCEEDS_AN_UNSIGNED_64_BIT_INTEGER}`);
    }
    value |= (byte & 0x7fn) << shift;
    if ((byte & 0x80n) === 0n) {
      if (used > 0 && byte === 0n) {
        rejectError(`${context} varint is not minimally encoded`);
      }
      if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
        rejectRange(`${context}${TEXT_EXCEEDS_JAVA_SCRIPT_S_SAFE_INTEGER_RANGE}`);
      }
      return [Number(value), cursor - offset];
    }
    shift += 7n;
  }
  if (cursor >= buffer.length) {
    rejectError(`${context} varint is truncated`);
  }
  rejectRange(`${context}${TEXT_VARINT_EXCEEDS_AN_UNSIGNED_64_BIT_INTEGER}`);
}

function curveIdForMulticodec(multicodec, context) {
  const entry = getCurveEntryByPublicKeyMulticodec(multicodec);
  if (!entry) {
    rejectError(`${context}${TEXT_USES_UNSUPPORTED}public-key multicodec ${multicodec}`);
  }
  return entry.id;
}

function encodeCompactLength(length) {
  let remaining = length >>> 0;
  const bytes = [];
  do {
    const chunk = remaining & 0x7f;
    remaining >>>= 7;
    bytes.push(remaining === 0 ? chunk : chunk | 0x80);
  } while (remaining !== 0);
  return Buffer.from(bytes);
}

function assetDefinitionChecksum(payload) {
  return Buffer.from(blake3(payload)).subarray(0, 4);
}

function decodeBase58(value, context) {
  let number = 0n;
  for (const char of value) {
    const digit = BASE58_LOOKUP.get(char);
    if (digit === undefined) {
      rejectError(`${context}${TEXT_MUST_BE}valid Base58`);
    }
    number = number * 58n + digit;
  }

  const bytes = [];
  while (number > 0n) {
    bytes.push(Number(number & 0xffn));
    number >>= 8n;
  }
  bytes.reverse();

  let leadingZeroes = 0;
  for (const char of value) {
    if (char !== "1") {
      break;
    }
    leadingZeroes += 1;
  }

  return Buffer.concat([Buffer.alloc(leadingZeroes), Buffer.from(bytes)]);
}

function encodeBase58(bytes) {
  let number = 0n;
  for (const byte of bytes) {
    number = (number << 8n) | BigInt(byte);
  }

  const encoded = [];
  while (number > 0n) {
    const remainder = Number(number % 58n);
    encoded.push(BASE58_ALPHABET[remainder]);
    number /= 58n;
  }

  for (const byte of bytes) {
    if (byte !== 0) {
      break;
    }
    encoded.push("1");
  }

  return encoded.reverse().join("") || "1";
}

function canonicalJsonStringify(value) {
  return JSON.stringify(canonicalizeJsonValue(normalizeInstructionJsonValue(cloneJson(value))));
}

function canonicalizeJsonValue(value) {
  if (Array.isArray(value)) {
    return value.map(canonicalizeJsonValue);
  }
  if (isPlainObject(value)) {
    const out = {};
    for (const key of Object.keys(value).sort()) {
      out[key] = canonicalizeJsonValue(value[key]);
    }
    return out;
  }
  return value;
}

function assertNonEmptyString(value, context) {
  if (typeof value !== JS_TYPE_STRING || value.trim().length === 0) {
    rejectType(`${context}${TEXT_MUST_BE_A}non-empty string`);
  }
  return value.trim();
}

function assertExactNonEmptyString(value, context) {
  if (typeof value !== JS_TYPE_STRING || value.length === 0) {
    rejectType(`${context}${TEXT_MUST_BE_A}non-empty string`);
  }
  return value;
}

function describeInstructionShape(instruction) {
  const topLevelKeys = Object.keys(instruction);
  if (topLevelKeys.length === 0) {
    return "an empty object";
  }
  const [topLevel] = topLevelKeys;
  if (isPlainObject(instruction[topLevel])) {
    const nestedKeys = Object.keys(instruction[topLevel]);
    if (nestedKeys.length > 0) {
      return `${topLevel}.${nestedKeys[0]}`;
    }
  }
  return topLevel;
}

function isPlainObject(value) {
  return Object.prototype.toString.call(value) === "[object Object]";
}

function isAlignmentError(error) {
  const message = error && typeof error.message === JS_TYPE_STRING ? error.message : "";
  return message.includes("requires 16-byte alignment");
}

function tryDecodeWithAlignedBuffer(native, buffer) {
  const candidate = allocateAlignedBuffer(buffer.length);
  if (candidate === null) {
    return null;
  }
  buffer.copy(candidate);
  try {
    return native.noritoDecodeInstruction(candidate);
  } catch (inner) {
    if (isAlignmentError(inner)) {
      return null;
    }
    throw inner;
  }
}

function allocateAlignedBuffer(length) {
  if (length === 0) {
    return Buffer.alloc(0);
  }
  const candidate = Buffer.alloc(length);
  if ((candidate.byteOffset & (ALIGNMENT - 1)) === 0) {
    return candidate;
  }
  return null;
}

function tryDecodeBase64(value) {
  if (!value) {
    return null;
  }
  const compact = value.replace(/\s+/g, "");
  if (compact.length === 0 || compact.length % 4 !== 0) {
    return null;
  }
  const paddingIndex = compact.indexOf("=");
  if (paddingIndex !== -1) {
    const head = compact.slice(0, paddingIndex);
    const padding = compact.slice(paddingIndex);
    if (!/^[0-9A-Za-z+/]*$/.test(head) || !/^={1,2}$/.test(padding)) {
      return null;
    }
  } else if (!/^[0-9A-Za-z+/]+$/.test(compact)) {
    return null;
  }
  try {
    const decoded = Buffer.from(compact, BASE64_ENCODING);
    if (decoded.length === 0) {
      return null;
    }
    if (decoded.toString(BASE64_ENCODING) !== compact) {
      return null;
    }
    return decoded;
  } catch {
    return null;
  }
}

function tryDecodeHex(value) {
  if (!value) {
    return null;
  }
  const compact = value.replace(/^0x/i, "");
  if (compact.length === 0 || compact.length % 2 !== 0 || /[^0-9A-Fa-f]/.test(compact)) {
    return null;
  }
  try {
    const decoded = Buffer.from(compact, HEX_ENCODING);
    return decoded.length > 0 ? decoded : null;
  } catch {
    return null;
  }
}

function tryDecodeWithRelocatedStorage(native, buffer) {
  const extra = ALIGNMENT - 1;
  const constructors = [];
  if (typeof SharedArrayBuffer === JS_TYPE_FUNCTION) {
    constructors.push((size) => new SharedArrayBuffer(size));
  }
  constructors.push((size) => new ArrayBuffer(size));

  for (const createStorage of constructors) {
    for (let pad = 0; pad <= extra; pad += 1) {
      let storage;
      try {
        storage = createStorage(buffer.length + extra);
      } catch {
        continue;
      }
      const raw = new Uint8Array(storage);
      raw.set(buffer, pad);
      const candidate = Buffer.from(raw.buffer, pad, buffer.length);
      if ((candidate.byteOffset & (ALIGNMENT - 1)) !== 0) {
        continue;
      }
      try {
        return native.noritoDecodeInstruction(candidate);
      } catch (inner) {
        if (isAlignmentError(inner)) {
          continue;
        }
        throw inner;
      }
    }
  }
  return null;
}

// Ordered instruction records share the existing primitive wire decoders.
const SetAssetTransferBlacklistInstructionFields = [
  [TEXT_ACCOUNT_ID, decodeAccountIdValue, 0, encodeAccountIdValue, 0],
  [TEXT_ASSET_DEFINITION_ID, decodeAssetDefinitionIdValue, 0, encodeAssetDefinitionIdValue, 0],
  ["blacklisted", decodeBoolValue, 0, encodeBoolValue, 0],
];

const CastZkBallotInstructionFields = [
  [TEXT_ELECTION_ID, decodeStringValue],
  ["proof_b64", decodeStringValue],
  ["public_inputs_json", decodeStringValue],
];

const CastPlainBallotInstructionFields = [
  ["referendum_id", decodeStringValue],
  ["owner", decodeAccountIdValue],
  ["amount", decodeQuantityValue],
  ["duration_blocks", decodeU64NumberValue],
  ["direction", decodeU8Value],
];

const ClaimTwitterFollowRewardInstructionFields = [
  ["binding_hash", decodeKeyedHashValue],
];

const SendToTwitterInstructionFields = [
  ["binding_hash", decodeKeyedHashValue, 0, encodeKeyedHashValue, 0],
  ["amount", decodeQuantityValue, 0, encodeQuantityValue, 0],
];

const CancelTwitterEscrowInstructionFields = [
  ["binding_hash", decodeKeyedHashValue],
];

const RegisterSmartContractCodeInstructionFields = [
  ["manifest", decodeContractManifestValue],
];

const RegisterSmartContractBytesInstructionFields = [
  [TEXT_CODE_HASH_2, decodeHashValue, 0, encodeHashValue, 0],
  ["code", decodeByteVecAsBase64, 0, encodeByteVecValue, 0],
];

const DeactivateContractInstanceInstructionFields = [
  [TEXT_CONTRACT_ADDRESS_2, decodeStringValue, 0, encodeRequiredRecordString, 0],
  [TEXT_EXPECTED_REVISION_2, decodeU64Value, 0, encodeU64Value, 0],
  ["reason", decodeStringValue, 1, encodeNoritoStringValue, 1],
];

const ActivateContractInstanceInstructionFields = [
  [TEXT_CONTRACT_ADDRESS_2, decodeStringValue, 0, encodeRequiredRecordString, 0],
  [TEXT_EXPECTED_REVISION_2, decodeU64Value, 0, encodeU64Value, 0],
  [TEXT_CODE_HASH_2, decodeHashValue, 0, encodeHashValue, 0],
];

const SetContractParliamentDelegationInstructionFields = [
  [TEXT_CONTRACT_ADDRESS_2, decodeStringValue, 0, encodeRequiredRecordString, 0],
  [TEXT_EXPECTED_REVISION_2, decodeU64Value, 0, encodeU64Value, 0],
  ["delegated", decodeBoolValue, 0, encodeBoolValue, 0],
];

const OfferContractOwnershipInstructionFields = [
  [TEXT_CONTRACT_ADDRESS_2, decodeStringValue, 0, encodeRequiredRecordString, 0],
  [TEXT_EXPECTED_REVISION_2, decodeU64Value, 0, encodeU64Value, 0],
  ["new_owner", decodeContractLifecycleOwnerValue, 0, encodeContractLifecycleOwnerValue, 0],
];

const CommitContractDeploymentInstructionFields = [
  ["expected_deploy_nonce", decodeU64Value],
  [TEXT_CONTRACT_ADDRESS, decodeStringValue],
  [TEXT_CODE_HASH, decodeHashValue],
  ["contract_alias", decodeStringValue],
  ["lease_expiry_ms", decodeU64Value, 1],
  [("expected_previous_" + TEXT_CONTRACT_ADDRESS_2), decodeStringValue, 1],
];

const UploadSmartContractCodeChunkInstructionFields = [
  [TEXT_CODE_HASH_2, decodeHashValue, 0, encodeHashValue, 0],
  ["total_size", decodeU64Value, 0, encodeU64Value, 0],
  ["chunk_index", decodeU32Value, 0, encodeU32Value, 0],
  ["chunk_count", decodeU32Value, 0, encodeU32Value, 0],
  ["chunk", decodeByteVecAsBase64, 0, encodeByteVecValue, 0],
];

const FinalizeSmartContractCodeUploadInstructionFields = [
  [TEXT_CODE_HASH_2, decodeHashValue, 0, encodeHashValue, 0],
  ["total_size", decodeU64Value, 0, encodeU64Value, 0],
  ["chunk_count", decodeU32Value, 0, encodeU32Value, 0],
];

const CancelSmartContractCodeUploadInstructionFields = [
  [TEXT_CODE_HASH, decodeHashValue],
];

const RemoveSmartContractBytesInstructionFields = [
  [TEXT_CODE_HASH_2, decodeHashValue, 0, encodeHashValue, 0],
  ["reason", decodeStringValue, 1, encodeNoritoStringValue, 1],
];

const Kaigi_CreateKaigiInstructionFields = [
  ["call", decodeNewKaigiPayload, 0, encodeNewKaigiValue, 0],
  [TEXT_COMMITMENT, decodeKaigiParticipantCommitmentValue, 1, encodeKaigiParticipantCommitmentValue, 1],
  ["nullifier", decodeKaigiParticipantNullifierValue, 1, encodeKaigiParticipantNullifierValue, 1],
  ["roster_root", decodeHashValue, 1, encodeHashValue, 1],
  ["proof", decodeByteVecAsBase64, 1, encodeByteVecValue, 1],
];

const Kaigi_EndKaigiInstructionFields = [
  ["call_id", decodeKaigiIdValue, 0, encodeKaigiIdValue, 0],
  ["ended_at_ms", decodeU64NumberValue, 1, encodeU64NumberValue, 1],
  [TEXT_COMMITMENT, decodeKaigiParticipantCommitmentValue, 1, encodeKaigiParticipantCommitmentValue, 1],
  ["nullifier", decodeKaigiParticipantNullifierValue, 1, encodeKaigiParticipantNullifierValue, 1],
  ["roster_root", decodeHashValue, 1, encodeHashValue, 1],
  ["proof", decodeByteVecAsBase64, 1, encodeByteVecValue, 1],
];

const Kaigi_RecordKaigiUsageInstructionFields = [
  ["call_id", decodeKaigiIdValue, 0, encodeKaigiIdValue, 0],
  ["duration_ms", decodeU64NumberValue, 0, encodeU64NumberValue, 0],
  ["billed_gas", decodeU64NumberValue, 0, encodeU64NumberValue, 0],
  [("usage_" + TEXT_COMMITMENT), decodeKaigiScalarValue, 1, encodeKaigiScalarValue, 1],
  ["proof", decodeByteVecAsBase64, 1, encodeByteVecValue, 1],
];

const Kaigi_SetKaigiRelayManifestInstructionFields = [
  ["call_id", decodeKaigiIdValue, 0, encodeKaigiIdValue, 0],
  ["relay_manifest", decodeKaigiRelayManifestValue, 1, encodeKaigiRelayManifestValue, 1],
];

const Kaigi_RegisterKaigiRelayInstructionFields = [
  ["relay", decodeKaigiRelayRegistrationValue],
];

const Kaigi_UnregisterKaigiRelayInstructionFields = [
  ["relay_id", decodeAccountIdValue],
];

const Kaigi_ReportKaigiRelayHealthInstructionFields = [
  ["call_id", decodeKaigiIdValue, 0, encodeKaigiIdValue, 0],
  ["relay_id", decodeAccountIdValue, 0, encodeAccountIdValue, 0],
  ["status", decodeKaigiRelayHealthStatusValue, 0, encodeKaigiRelayHealthStatusValue, 0],
  ["reported_at_ms", decodeU64NumberValue, 0, encodeU64NumberValue, 0],
  ["notes", decodeKaigiRelayHealthNotesValue, 1, encodeKaigiRelayHealthNotesValue, 1],
];

const zk_RegisterZkAssetInstructionFields = [
  ["asset", decodeAssetDefinitionIdValue, 0, encodeAssetDefinitionIdValue, 0],
  ["vk_unshield", decodeVerifyingKeyIdValue, 1, encodeVerifyingKeyIdValue, 1],
  ["vk_shield", decodeVerifyingKeyIdValue, 1, encodeVerifyingKeyIdValue, 1],
];

const zk_ScheduleConfidentialPolicyTransitionInstructionFields = [
  ["asset", decodeAssetDefinitionIdValue],
  ["new_mode", decodeConfidentialPolicyModeValue],
  ["effective_height", decodeU64NumberValue],
  ["transition_id", decodeHashValue],
  ["conversion_window", decodeU64NumberValue, 1],
];

const zk_CancelConfidentialPolicyTransitionInstructionFields = [
  ["asset", decodeAssetDefinitionIdValue, 0, encodeAssetDefinitionIdValue, 0],
  ["transition_id", decodeHashValue, 0, encodeHashValue, 0],
];

const RegisterRwaInstructionFields = [
  ["rwa", decodeNewRwaValue],
];

const TransferRwaInstructionFields = [
  ["source", decodeAccountIdValue, 0, encodeAccountIdValue, 0],
  ["rwa", decodeRwaIdValue, 0, encodeRwaIdValue, 0],
  ["quantity", decodeQuantityValue, 0, encodeQuantityValue, 0],
  ["destination", decodeAccountIdValue, 0, encodeAccountIdValue, 0],
];

const ForceTransferRwaInstructionFields = [
  ["rwa", decodeRwaIdValue, 0, encodeRwaIdValue, 0],
  ["quantity", decodeQuantityValue, 0, encodeQuantityValue, 0],
  ["destination", decodeAccountIdValue, 0, encodeAccountIdValue, 0],
];

const SetRwaControlsInstructionFields = [
  ["rwa", decodeRwaIdValue, 0, encodeRwaIdValue, 0],
  ["controls", decodeRwaControlPolicyValue, 0, encodeRwaControlPolicyValue, 0],
];

const SetRwaKeyValueInstructionFields = [
  ["rwa", decodeRwaIdValue],
  ["key", decodeNameValue],
  ["value", decodeNestedJsonValue],
];

const RemoveRwaKeyValueInstructionFields = [
  ["rwa", decodeRwaIdValue, 0, encodeRwaIdValue, 0],
  ["key", decodeNameValue, 0, encodeNameValue, 0],
];

const CustomInstructionFields = [
  ["payload", decodeNestedJsonValue],
];

function encodeRequiredRecordString(value, context) { return encodeNoritoStringValue(assertNonEmptyString(value, context)); }
const encodeCanonicalRecordFields = /* @__PURE__ */ createNoritoRecordEncoder(
  encodeStructValue, encodeOptionValue, encodeNoritoVec,
);
const CanonicalEncodingFields3 = [
  ["order_id", , , encodeReplicationIdValue, 0],
  [TEXT_EXPIRATION_EPOCH, , , encodeU64Value, 0],
];

const CanonicalEncodingFields18 = [
  ["id", , , encodeVerifyingKeyIdValue, 0],
  ["record", , , encodeVerifyingKeyRecordValue, 0],
];

const CanonicalEncodingFields25 = [
  ["rwa", , , encodeRwaIdValue, 0],
  ["quantity", , , encodeQuantityValue, 0],
];

const CanonicalEncodingFields26 = [
  ["rwa", , , encodeRwaIdValue, 0],
  ["quantity", , , encodeQuantityValue, 0],
];

const CanonicalEncodingFields27 = [
  ["rwa", , , encodeRwaIdValue, 0],
  ["quantity", , , encodeQuantityValue, 0],
];
