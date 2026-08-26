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
  canonicalizeDomainLabel,
  curveIdFromAlgorithm,
  curveIdToAlgorithm,
  ensureCurveIdEnabled,
  normalizeBytes,
  validatePublicKeyForCurve,
} from "./address.js";
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
import { getNativeBinding } from "./native.js";
import {
  createNoritoContractCodecs,
  createNoritoProofValueCodecs,
} from "./noritoContractCodecs.js";
import {
  createNoritoGovernanceInstructionBoundary,
  parseStrictGovernanceInstructionJson,
} from "./noritoGovernanceBoundary.js";
import { computeHashLiteralCrc } from "./hashLiteralCrc.js";
import { KotodamaQuantity, NumericV1 } from "./numericV1.js";
import { parseStrictLosslessIntegerJson } from "./strictLosslessJson.js";
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
const UINT128_MASK = (1n << 128n) - 1n;
const HASH_LITERAL_RE = /^hash:([0-9A-Fa-f]{64})#([0-9A-Fa-f]{4})$/;
const CANONICAL_HASH_LITERAL_RE = /^hash:([0-9A-F]{64})#([0-9A-F]{4})$/;
const MULTIHASH_LITERAL_RE = /^([0-9a-fA-F]+)$/;
const DEFAULT_SM2_DISTINGUISHED_ID = new Uint8Array(16);
const SCHEDULE_CONFIDENTIAL_POLICY_TRANSITION_WIRE_ID =
  "zk::ScheduleConfidentialPolicyTransition";
const CANCEL_CONFIDENTIAL_POLICY_TRANSITION_WIRE_ID =
  "zk::CancelConfidentialPolicyTransition";
const SET_ASSET_TRANSFER_AVAILABILITY_VARIANT =
  "SetAssetTransferAvailability";
const SET_TRANSFER_REASON_CONTEXT = "SetAssetTransferAvailability.reason";
const COMPLETE_ORDER_REVISION_CONTEXT = "CompleteReplicationOrder.expected_assignment_revision";
const COMPLETE_ORDER_REVISION_MESSAGE = "CompleteReplicationOrder.expected_assignment_revision must be greater than zero";
const CANCEL_LOCK_REMAINING_CONTEXT = "CancelAssetLock.expected_remaining_amount";
const CANCEL_LOCK_REMAINING_MESSAGE = "CancelAssetLock.expected_remaining_amount must be greater than zero";
const ISSUE_ORDER_DEADLINE_CONTEXT = "IssueReplicationOrder.deadline_epoch";
const ISSUE_ORDER_DEADLINE_MESSAGE = "IssueReplicationOrder.deadline_epoch must be greater than issued_epoch";
const ISSUE_ORDER_PAYLOAD_CONTEXT = "IssueReplicationOrder.order_payload";
const ISSUE_ORDER_EPOCH_CONTEXT = "IssueReplicationOrder.issued_epoch";
const ISSUE_ORDER_ID_CONTEXT = "IssueReplicationOrder.order_id";
const EXPECTED_PREVIOUS_CONTRACT_CONTEXT = "CommitContractDeployment.expected_previous_contract_address";
const SCHEDULE_CONVERSION_WINDOW_CONTEXT = "zk.ScheduleConfidentialPolicyTransition.conversion_window";
const SCHEDULE_EFFECTIVE_HEIGHT_CONTEXT = "zk.ScheduleConfidentialPolicyTransition.effective_height";
const SCHEDULE_TRANSITION_ID_CONTEXT = "zk.ScheduleConfidentialPolicyTransition.transition_id";
const CANCEL_TRANSITION_ID_CONTEXT = "zk.CancelConfidentialPolicyTransition.transition_id";
const SUPPORTED_JS_CANONICALIZATION_INSTRUCTIONS = [
  "Mint.Asset",
  "Mint.TriggerRepetitions",
  "Burn.Asset",
  "Burn.TriggerRepetitions",
  "Transfer.Domain",
  "Transfer.AssetDefinition",
  "Transfer.Asset",
  "Transfer.Nft",
  "Register.Domain",
  "Register.Account",
  "Register.AssetDefinition",
  "ExecuteTrigger",
  "Custom",
  "Kaigi.*",
  "Governance.*",
  "Social.*",
  "SmartContract.*",
  "zk.*",
  "VerifyingKey.*",
  "Rwa.*",
  "CancelAssetLock",
  SET_ASSET_TRANSFER_AVAILABILITY_VARIANT,
  "SoraFS.ReplicationOrder.*",
  "RecordSccpMessage",
];
const CANCEL_ASSET_LOCK_WIRE_ID =
  "iroha_data_model::isi::escrow::CancelAssetLock";
const CANCEL_ASSET_LOCK_V1_SCHEMA_HASH = /* @__PURE__ */ schemaHashForTypeName(
  CANCEL_ASSET_LOCK_WIRE_ID,
);
// A transparent 32-byte EscrowId plus one positive signed-512-bit Quantity
// yields an unpadded canonical archive in this exact range. Enforce it before
// CRC work so an oversized attacker-controlled frame cannot make this fixed
// schema perform an unbounded payload scan.
const CANCEL_ASSET_LOCK_V1_MIN_ARCHIVE_BYTES = 85;
const CANCEL_ASSET_LOCK_V1_MAX_ARCHIVE_BYTES = 148;
const SET_ASSET_TRANSFER_AVAILABILITY_WIRE_ID =
  "iroha.asset.transfer.availability.set";
const ASSET_TRANSFER_AVAILABILITY_MAX_REASON_BYTES_V1 = 512;
const RECORD_SCCP_MESSAGE_WIRE_ID =
  "iroha_data_model::isi::bridge::RecordSccpMessage";
const ISSUE_REPLICATION_ORDER_WIRE_ID =
  "iroha_data_model::isi::sorafs::IssueReplicationOrder";
const COMPLETE_REPLICATION_ORDER_WIRE_ID =
  "iroha_data_model::isi::sorafs::CompleteReplicationOrder";
const EXPIRE_REPLICATION_ORDER_WIRE_ID =
  "iroha_data_model::isi::sorafs::ExpireReplicationOrder";
const REPLICATION_ORDER_V1_SCHEMA_HASH = /* @__PURE__ */ schemaHashForTypeName(
  "sorafs_manifest::capacity::ReplicationOrderV1",
);
const SORAFS_REPLICATION_ORDER_MAX_PAYLOAD_BYTES_V1 = 1024 * 1024;
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
  "iroha_data_model::zk::OpenVerifyEnvelope",
);
const EVENT_FILTER_BOX_SCHEMA_HASH = /* @__PURE__ */ schemaHashForTypeName(
  "iroha_data_model::events::model::EventFilterBox",
);
export const PRIVACY_EXACT12_FIXTURE_BUNDLE_SCHEMA_NAME_V1 =
  "iroha.privacy.exact12-typed-fixture-bundle.v1";
export const PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1 = 2 * 1024 * 1024;
export const PRIVACY_EXACT12_PROTOCOL_IDS_V1 = /* @__PURE__ */ Object.freeze([
  "zk-ace-pq-authorization-v0",
  "anonymous-pgc-k-out-of-n-v1",
  "verange-transparent-range-v1",
  "iroha-zk-ams-v1",
  "vega-existing-credential-zk-v0",
  "iroha-zk-x509-stark-p256-v0",
  "iroha-jindo-polynomial-commitment-v0",
  "iroha-bootle-lantern-anoncred-v1",
  "orchard-halo2-actions-v1",
  "monero-fcmp-plus-plus-v1",
  "iroha-ivm-private-note-stark-v1",
  "pq-masp-stark-v0",
]);
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
  "iroha_data_model::isi::privacy::SubmitPrivacyProofV1",
);
const PRIVACY_EXACT12_TRANSACTION_PAYLOAD_SCHEMA_HASH_V1 = /* @__PURE__ */ schemaHashForTypeName(
  "iroha_data_model::transaction::signed::model::TransactionPayload",
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
  "submit_proof_instruction_norito",
  "transaction_intent_projection_norito",
  "transaction_intent_digest",
  "unsigned_transaction_payload_norito",
  "signed_transaction_versioned_norito",
  "signed_transaction_hash",
]);
const PRIVACY_EXACT12_PUBLIC_ROW_FIELD_NAMES_V1 = /* @__PURE__ */ Object.freeze([
  "protocolId",
  "statementNorito",
  "envelopeNorito",
  "submitProofWireId",
  "submitProofInstructionNorito",
  "transactionIntentProjectionNorito",
  "transactionIntentDigest",
  "unsignedTransactionPayloadNorito",
  "signedTransactionVersionedNorito",
  "signedTransactionHash",
]);
const PRIVACY_EXACT12_ENVELOPE_FIELD_NAMES_V1 = /* @__PURE__ */ Object.freeze([
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
  "iroha_data_model::block::proofs::BlockProofs";
const REGISTER_SMART_CONTRACT_CODE_WIRE_ID = "iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode";
const REGISTER_SMART_CONTRACT_BYTES_WIRE_ID = "iroha_data_model::isi::smart_contract_code::RegisterSmartContractBytes";
const DEACTIVATE_CONTRACT_INSTANCE_WIRE_ID = "iroha_data_model::isi::smart_contract_code::DeactivateContractInstance";
const ACTIVATE_CONTRACT_INSTANCE_WIRE_ID = "iroha_data_model::isi::smart_contract_code::ActivateContractInstance";
const COMMIT_CONTRACT_DEPLOYMENT_WIRE_ID = "iroha_data_model::isi::smart_contract_code::CommitContractDeployment";
const UPLOAD_SMART_CONTRACT_CODE_CHUNK_WIRE_ID = "iroha_data_model::isi::smart_contract_code::UploadSmartContractCodeChunk";
const FINALIZE_SMART_CONTRACT_CODE_UPLOAD_WIRE_ID = "iroha_data_model::isi::smart_contract_code::FinalizeSmartContractCodeUpload";
const CANCEL_SMART_CONTRACT_CODE_UPLOAD_WIRE_ID = "iroha_data_model::isi::smart_contract_code::CancelSmartContractCodeUpload";
const REMOVE_SMART_CONTRACT_BYTES_WIRE_ID = "iroha_data_model::isi::smart_contract_code::RemoveSmartContractBytes";
const CREATE_KAIGI_WIRE_ID = "iroha_data_model::isi::kaigi::CreateKaigi";
const JOIN_KAIGI_WIRE_ID = "iroha_data_model::isi::kaigi::JoinKaigi";
const LEAVE_KAIGI_WIRE_ID = "iroha_data_model::isi::kaigi::LeaveKaigi";
const END_KAIGI_WIRE_ID = "iroha_data_model::isi::kaigi::EndKaigi";
const RECORD_KAIGI_USAGE_WIRE_ID = "iroha_data_model::isi::kaigi::RecordKaigiUsage";
const SET_KAIGI_RELAY_MANIFEST_WIRE_ID = "iroha_data_model::isi::kaigi::SetKaigiRelayManifest";
const REGISTER_KAIGI_RELAY_WIRE_ID = "iroha_data_model::isi::kaigi::RegisterKaigiRelay";
const PROPOSE_DEPLOY_CONTRACT_WIRE_ID = "iroha_data_model::isi::governance::ProposeDeployContract";
const CAST_ZK_BALLOT_WIRE_ID = "iroha_data_model::isi::governance::CastZkBallot";
const CAST_PLAIN_BALLOT_WIRE_ID = "iroha_data_model::isi::governance::CastPlainBallot";
const PERSIST_COUNCIL_FOR_EPOCH_WIRE_ID = "iroha_data_model::isi::governance::PersistCouncilForEpoch";
const CLAIM_TWITTER_FOLLOW_REWARD_WIRE_ID = "iroha_data_model::isi::social::ClaimTwitterFollowReward";
const SEND_TO_TWITTER_WIRE_ID = "iroha_data_model::isi::social::SendToTwitter";
const CANCEL_TWITTER_ESCROW_WIRE_ID = "iroha_data_model::isi::social::CancelTwitterEscrow";
const REGISTER_ZK_ASSET_WIRE_ID = "iroha_data_model::isi::zk::RegisterZkAsset";
const CREATE_ELECTION_WIRE_ID = "iroha_data_model::isi::zk::CreateElection";
const SUBMIT_BALLOT_WIRE_ID = "iroha_data_model::isi::zk::SubmitBallot";
const FINALIZE_ELECTION_WIRE_ID = "iroha_data_model::isi::zk::FinalizeElection";
const REGISTER_VERIFYING_KEY_WIRE_ID = "iroha_data_model::isi::verifying_keys::RegisterVerifyingKey";
const UPDATE_VERIFYING_KEY_WIRE_ID = "iroha_data_model::isi::verifying_keys::UpdateVerifyingKey";
const INNER_TYPE_NAME_BY_WIRE_ID = Object.freeze({
  "iroha.mint": "iroha_data_model::isi::mint_burn::MintBox",
  "iroha.burn": "iroha_data_model::isi::mint_burn::BurnBox",
  "iroha.register": "iroha_data_model::isi::register::RegisterBox",
  "iroha.transfer": "iroha_data_model::isi::transfer::TransferBox",
  "iroha.custom": "iroha_data_model::isi::transparent::CustomInstruction",
  "iroha.execute_trigger": "iroha_data_model::isi::transparent::ExecuteTrigger",
  "iroha.rwa": "iroha_data_model::isi::rwa::RwaInstructionBox",
  [CANCEL_ASSET_LOCK_WIRE_ID]: CANCEL_ASSET_LOCK_WIRE_ID,
  [SET_ASSET_TRANSFER_AVAILABILITY_WIRE_ID]:
    "iroha_data_model::isi::asset_transfer_control::SetAssetTransferAvailability",
  [RECORD_SCCP_MESSAGE_WIRE_ID]: RECORD_SCCP_MESSAGE_WIRE_ID,
  [ISSUE_REPLICATION_ORDER_WIRE_ID]: ISSUE_REPLICATION_ORDER_WIRE_ID,
  [COMPLETE_REPLICATION_ORDER_WIRE_ID]: COMPLETE_REPLICATION_ORDER_WIRE_ID,
  [EXPIRE_REPLICATION_ORDER_WIRE_ID]: EXPIRE_REPLICATION_ORDER_WIRE_ID,
  [CREATE_KAIGI_WIRE_ID]: CREATE_KAIGI_WIRE_ID,
  [JOIN_KAIGI_WIRE_ID]: JOIN_KAIGI_WIRE_ID,
  [LEAVE_KAIGI_WIRE_ID]: LEAVE_KAIGI_WIRE_ID,
  [END_KAIGI_WIRE_ID]: END_KAIGI_WIRE_ID,
  [RECORD_KAIGI_USAGE_WIRE_ID]: RECORD_KAIGI_USAGE_WIRE_ID,
  [SET_KAIGI_RELAY_MANIFEST_WIRE_ID]: SET_KAIGI_RELAY_MANIFEST_WIRE_ID,
  [REGISTER_KAIGI_RELAY_WIRE_ID]: REGISTER_KAIGI_RELAY_WIRE_ID,
  [PROPOSE_DEPLOY_CONTRACT_WIRE_ID]: PROPOSE_DEPLOY_CONTRACT_WIRE_ID,
  [CAST_ZK_BALLOT_WIRE_ID]: CAST_ZK_BALLOT_WIRE_ID,
  [CAST_PLAIN_BALLOT_WIRE_ID]: CAST_PLAIN_BALLOT_WIRE_ID,
  [PERSIST_COUNCIL_FOR_EPOCH_WIRE_ID]: PERSIST_COUNCIL_FOR_EPOCH_WIRE_ID,
  [CLAIM_TWITTER_FOLLOW_REWARD_WIRE_ID]: CLAIM_TWITTER_FOLLOW_REWARD_WIRE_ID,
  [SEND_TO_TWITTER_WIRE_ID]: SEND_TO_TWITTER_WIRE_ID,
  [CANCEL_TWITTER_ESCROW_WIRE_ID]: CANCEL_TWITTER_ESCROW_WIRE_ID,
  [REGISTER_SMART_CONTRACT_CODE_WIRE_ID]: REGISTER_SMART_CONTRACT_CODE_WIRE_ID,
  [REGISTER_SMART_CONTRACT_BYTES_WIRE_ID]: REGISTER_SMART_CONTRACT_BYTES_WIRE_ID,
  [DEACTIVATE_CONTRACT_INSTANCE_WIRE_ID]: DEACTIVATE_CONTRACT_INSTANCE_WIRE_ID,
  [ACTIVATE_CONTRACT_INSTANCE_WIRE_ID]: ACTIVATE_CONTRACT_INSTANCE_WIRE_ID,
  [COMMIT_CONTRACT_DEPLOYMENT_WIRE_ID]: COMMIT_CONTRACT_DEPLOYMENT_WIRE_ID,
  [UPLOAD_SMART_CONTRACT_CODE_CHUNK_WIRE_ID]: UPLOAD_SMART_CONTRACT_CODE_CHUNK_WIRE_ID,
  [FINALIZE_SMART_CONTRACT_CODE_UPLOAD_WIRE_ID]: FINALIZE_SMART_CONTRACT_CODE_UPLOAD_WIRE_ID,
  [CANCEL_SMART_CONTRACT_CODE_UPLOAD_WIRE_ID]: CANCEL_SMART_CONTRACT_CODE_UPLOAD_WIRE_ID,
  [REMOVE_SMART_CONTRACT_BYTES_WIRE_ID]: REMOVE_SMART_CONTRACT_BYTES_WIRE_ID,
  [REGISTER_ZK_ASSET_WIRE_ID]: REGISTER_ZK_ASSET_WIRE_ID,
  [SCHEDULE_CONFIDENTIAL_POLICY_TRANSITION_WIRE_ID]:
    "iroha_data_model::isi::zk::ScheduleConfidentialPolicyTransition",
  [CANCEL_CONFIDENTIAL_POLICY_TRANSITION_WIRE_ID]:
    "iroha_data_model::isi::zk::CancelConfidentialPolicyTransition",
  [CREATE_ELECTION_WIRE_ID]: CREATE_ELECTION_WIRE_ID,
  [SUBMIT_BALLOT_WIRE_ID]: SUBMIT_BALLOT_WIRE_ID,
  [FINALIZE_ELECTION_WIRE_ID]: FINALIZE_ELECTION_WIRE_ID,
  [REGISTER_VERIFYING_KEY_WIRE_ID]: REGISTER_VERIFYING_KEY_WIRE_ID,
  [UPDATE_VERIFYING_KEY_WIRE_ID]: UPDATE_VERIFYING_KEY_WIRE_ID,
});
const INNER_SCHEMA_HASH_BY_WIRE_ID = Object.freeze(
  Object.fromEntries(
    Object.entries(INNER_TYPE_NAME_BY_WIRE_ID).map(([wireId, typeName]) => [
      wireId,
      /* @__PURE__ */ schemaHashForTypeName(typeName),
    ]),
  ),
);
const INNER_HEADER_PADDING_BY_WIRE_ID = Object.freeze({});
const BASE58_LOOKUP = new Map(
  Array.from(BASE58_ALPHABET, (char, index) => [char, BigInt(index)]),
);
const INSTRUCTION_CACHE_SYMBOL = Symbol.for("iroha.js.noritoInstructionCache");
const instructionCache =
  globalThis[INSTRUCTION_CACHE_SYMBOL] ??
  (globalThis[INSTRUCTION_CACHE_SYMBOL] = new Map());
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
      throw new Error(
        `${this.context} has ${this.buffer.length - this.offset} trailing bytes`,
      );
    }
  }

  #ensureAvailable(length, name) {
    if (this.offset + length > this.buffer.length) {
      throw new Error(
        `${this.context}.${name} overran payload (${length} bytes requested, ${this.buffer.length - this.offset} remaining)`,
      );
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

function resolveNative(method) {
  const native = globalThis.__IROHA_NORITO_BINDING__ ?? getNativeBinding();
  if (typeof native[method] !== JS_TYPE_FUNCTION) {
    throw new Error(`Native binding does not expose ${method}`);
  }
  return native;
}

function isNativeBindingUnavailable(error) {
  const message =
    error && typeof error.message === JS_TYPE_STRING ? error.message : String(error ?? "");
  return (
    message.includes("Native binding required") ||
    message.includes("Native binding does not expose") ||
    message.includes("process is not defined") ||
    message.includes("require is not available") ||
    message.includes("createRequire is not a function")
  );
}

function isNativeBindingUnsupportedInstruction(error) {
  const message =
    error && typeof error.message === JS_TYPE_STRING ? error.message : String(error ?? "");
  return (
    message.includes("unsupported zk instruction variant") ||
    message.includes("unsupported instruction") ||
    message.includes("unsupported instruction variant") ||
    message.includes("unknown instruction wire id") ||
    message.includes("unknown instruction schema") ||
    message.includes("unknown instruction `") ||
    message.includes("invalid enum discriminant") ||
    message.includes("(not registered)") ||
    message.includes("instruction payload must use canonical Norito framing")
  );
}

function shouldUsePureJsInstructionFallback(error) {
  return isNativeBindingUnavailable(error) || isNativeBindingUnsupportedInstruction(error);
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
      throw new TypeError(
        `zk.${variant} is retired in ABI V1; use the typed Kagemusha flow`,
      );
    }
  }
}

function encodeNormalizedInstruction(normalized) {
  rejectRetiredGenericZkInstruction(normalized);
  validateGovernanceInstructionBoundary(normalized);
  let encoded;
  try {
    const native = resolveNative("noritoEncodeInstruction");
    encoded = native.noritoEncodeInstruction(JSON.stringify(normalized));
  } catch (error) {
    if (!shouldUsePureJsInstructionFallback(error)) {
      throw error;
    }
    try {
      encoded = encodePureJsInstruction(normalized);
    } catch (fallbackError) {
      if (!isPureJsUnsupportedInstructionError(fallbackError)) {
        throw fallbackError;
      }
      throw error;
    }
  }
  cacheInstructionRoundTrip(encoded, normalized);
  return encoded;
}

function isPureJsUnsupportedInstructionError(error) {
  const message =
    error && typeof error.message === JS_TYPE_STRING ? error.message : String(error ?? "");
  return (
    message.startsWith("Internal Norito canonicalization supports ") ||
    message.startsWith("Internal Norito decoder does not support ")
  );
}

function cacheInstructionRoundTrip(bytes, instruction) {
  try {
    instructionCache.set(
      Buffer.from(bytes).toString(HEX_ENCODING),
      canonicalizeInstructionForCache(instruction),
    );
  } catch {
    // Cache misses must not affect Norito encoding/decoding.
  }
}

function getCachedInstruction(bytes) {
  const cached = instructionCache.get(Buffer.from(bytes).toString(HEX_ENCODING));
  return cached === undefined ? null : cloneJson(cached);
}

function canonicalizeInstructionForCache(instruction) {
  const normalized = normalizeInstructionJsonValue(cloneJson(instruction));
  let canonicalInstruction = normalized;
  if (isPlainObject(instruction.Multisig)) {
    canonicalInstruction = { Custom: { payload: normalized.Multisig } };
  } else if (isPlainObject(instruction.MultisigRegister)) {
    canonicalInstruction = {
      Custom: { payload: { Register: normalized.MultisigRegister } },
    };
  } else if (isPlainObject(instruction.MultisigPropose)) {
    canonicalInstruction = {
      Custom: { payload: { Propose: normalized.MultisigPropose } },
    };
  } else if (isPlainObject(instruction.MultisigApprove)) {
    canonicalInstruction = {
      Custom: { payload: { Approve: normalized.MultisigApprove } },
    };
  } else if (isPlainObject(instruction.MultisigCancel)) {
    canonicalInstruction = {
      Custom: { payload: { Cancel: normalized.MultisigCancel } },
    };
  }
  try {
    return decodePureJsInstruction(encodePureJsInstruction(canonicalInstruction));
  } catch {
    return cloneJson(canonicalInstruction);
  }
}

/**
 * Encode an instruction JSON payload to canonical Norito bytes.
 * @param {object | string | ArrayBufferView | ArrayBuffer | Buffer} instruction
 * @returns {Buffer}
 */
export function noritoEncodeInstruction(instruction) {
  if (isBinaryLike(instruction)) {
    return toBuffer(instruction);
  }
  if (typeof instruction === JS_TYPE_STRING) {
    const trimmed = instruction.trim();
    try {
      const parsed = JSON.parse(trimmed);
      const exactParsed = isStrictGovernanceInstructionCandidate(parsed)
        ? parseStrictGovernanceInstructionJson(trimmed, "governance instruction")
        : parsed;
      const normalized = normalizeInstructionJsonValue(exactParsed);
      return encodeNormalizedInstruction(normalized);
    } catch (error) {
      if (error instanceof SyntaxError) {
        const decoded = tryDecodeBase64(trimmed) ?? tryDecodeHex(trimmed);
        if (decoded) {
          return decoded;
        }
        const native = resolveNative("noritoEncodeInstruction");
        const encoded = native.noritoEncodeInstruction(instruction);
        try {
          cacheInstructionRoundTrip(encoded, JSON.parse(instruction));
        } catch {
          // Raw JSON string was not parseable; leave cache empty.
        }
        return encoded;
      }
      throw error;
    }
  }
  const normalized = normalizeInstructionJsonValue(cloneJson(instruction));
  return encodeNormalizedInstruction(normalized);
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
    throw new TypeError("transaction payload batch must be an array");
  }
  if (payloads.length === 0) {
    throw new TypeError("transaction payload batch must contain at least one payload");
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
    throw new TypeError(
      "SoraFS billing acknowledgement proof must be an object",
    );
  }
  const keys = Object.keys(proof);
  if (
    keys.length !== 2 ||
    !Object.prototype.hasOwnProperty.call(proof, "requestNonceHex") ||
    !Object.prototype.hasOwnProperty.call(proof, "authenticationProof")
  ) {
    throw new TypeError(
      "SoraFS billing acknowledgement proof must contain exactly requestNonceHex and authenticationProof",
    );
  }
  const requestNonceHex = proof.requestNonceHex;
  if (
    typeof requestNonceHex !== JS_TYPE_STRING ||
    !/^[0-9a-f]{64}$/u.test(requestNonceHex) ||
    /^0{64}$/u.test(requestNonceHex)
  ) {
    throw new TypeError(
      "SoraFS billing acknowledgement requestNonceHex must be one non-zero lowercase 32-byte hexadecimal digest",
    );
  }
  const authenticationProof = proof.authenticationProof;
  if (
    !Buffer.isBuffer(authenticationProof) &&
    !ArrayBuffer.isView(authenticationProof) &&
    !(authenticationProof instanceof ArrayBuffer)
  ) {
    throw new TypeError(
      "SoraFS billing acknowledgement authenticationProof must be binary bytes",
    );
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
    throw new RangeError(
      `SoraFS billing acknowledgement authenticationProof must contain 1..=${SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1} bytes`,
    );
  }
  const payload = withNoritoCompactLengths(() =>
    encodeStructValue([
      [
        encodeFixedBytesValue(
          Buffer.from(requestNonceHex, HEX_ENCODING),
          32,
          "SoraFS billing acknowledgement requestNonceHex",
        ),
      ],
      [
        encodeByteVecValue(
          proofBytes,
          "SoraFS billing acknowledgement authenticationProof",
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
    throw new TypeError(`${context} must be an object`);
  }
  assertOnlyObjectKeys(intent, ["payer", "value"], context);
  const payer = assertNonEmptyString(intent.payer, `${context}.payer`);
  if (payer !== "authority" && payer !== "sponsor") {
    throw new TypeError(`${context}.payer must be authority or sponsor`);
  }
  if (!isPlainObject(intent.value)) {
    throw new TypeError(`${context}.value must be an object`);
  }
  const allowedValueFields = ["charge_limits", "gas_limit"];
  if (payer === "sponsor") {
    allowedValueFields.push("program_id", "program_revision");
  }
  assertOnlyObjectKeys(intent.value, allowedValueFields, `${context}.value`);
  if (!Array.isArray(intent.value.charge_limits)) {
    throw new TypeError(`${context}.value.charge_limits must be an array`);
  }
  let previousKind = -1;
  const chargeLimits = encodeNoritoVec(
    Array.from(intent.value.charge_limits, (limit, index) => {
      const itemContext = `${context}.value.charge_limits[${index}]`;
      if (!Object.prototype.hasOwnProperty.call(intent.value.charge_limits, index)) {
        throw new TypeError(`${context}.value.charge_limits must not contain holes`);
      }
      if (!isPlainObject(limit)) {
        throw new TypeError(`${itemContext} must be an object`);
      }
      assertOnlyObjectKeys(
        limit,
        ["kind", "asset_definition_id", "max_amount"],
        itemContext,
      );
      if (!isPlainObject(limit.kind)) {
        throw new TypeError(`${itemContext}.kind must be a tagged unit object`);
      }
      assertOnlyObjectKeys(limit.kind, ["kind", "value"], `${itemContext}.kind`);
      const kind = assertNonEmptyString(limit.kind.kind, `${itemContext}.kind.kind`);
      const kindTag = kind === "nexus" ? 0 : kind === "pipeline_gas" ? 1 : -1;
      if (kindTag < 0 || limit.kind.value !== null) {
        throw new TypeError(
          `${itemContext}.kind must be the canonical nexus or pipeline_gas tagged unit`,
        );
      }
      if (kindTag <= previousKind) {
        throw new TypeError(
          `${context}.value.charge_limits must be unique and ordered nexus before pipeline_gas`,
        );
      }
      previousKind = kindTag;
      const quantity = NumericV1.decodeQuantityJson(limit.max_amount);
      if (quantity.mantissa <= 0n) {
        throw new TypeError(`${itemContext}.max_amount must be greater than zero`);
      }
      return encodeStructValue([
        [encodeEnumTagValue(kindTag)],
        [
          encodeAssetDefinitionIdValue(
            limit.asset_definition_id,
            `${itemContext}.asset_definition_id`,
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
      throw new TypeError(`${context}.value.gas_limit must be non-zero`);
    }
  }
  if (payer === "authority") {
    return encodeEnumTagValue(0, () =>
      encodeStructValue([[chargeLimits], [gasLimit]]),
    );
  }
  if (!isPlainObject(intent.value.program_id)) {
    throw new TypeError(`${context}.value.program_id must be an object`);
  }
  assertOnlyObjectKeys(
    intent.value.program_id,
    ["sponsor", "name"],
    `${context}.value.program_id`,
  );
  const name = assertNonEmptyString(
    intent.value.program_id.name,
    `${context}.value.program_id.name`,
  );
  if (
    name !== intent.value.program_id.name ||
    name.normalize("NFC") !== name ||
    /[\s@#$\/]/u.test(name)
  ) {
    throw new TypeError(`${context}.value.program_id.name must be a canonical Iroha Name`);
  }
  const revision = normalizeU64Input(
    intent.value.program_revision,
    `${context}.value.program_revision`,
  );
  if (revision === 0n) {
    throw new TypeError(`${context}.value.program_revision must be non-zero`);
  }
  const programId = encodeStructValue([
    [
      encodeAccountIdValue(
        intent.value.program_id.sponsor,
        `${context}.value.program_id.sponsor`,
      ),
    ],
    [encodeNoritoStringValue(name)],
  ]);
  return encodeEnumTagValue(1, () =>
    encodeStructValue([
      [programId],
      [encodeU64Value(revision, `${context}.value.program_revision`)],
      [chargeLimits],
      [gasLimit],
    ]),
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
    throw new TypeError(
      `${context} does not accept private-key fields (${fields.join(", ")}); sign the returned transaction draft locally`,
    );
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
export function noritoEncodeMultisigProposeRequest(request) {
  if (!isPlainObject(request)) {
    throw new TypeError("MultisigProposeDto request must be an object");
  }
  if (!Array.isArray(request.instructions)) {
    throw new TypeError("MultisigProposeDto.instructions must be an array");
  }
  rejectInlinePrivateKeyFields(request, "MultisigProposeDto");
  const validationFeeMetadata = normalizeMultisigProposeValidationFeeMetadata(request);
  const payload = withNoritoCompactLengths(() =>
    encodeStructValue([
      ...encodeMultisigAccountSelectorFields(request, "MultisigProposeDto.selector"),
      [
        encodeAccountIdValue(
          request.signer_account_id ?? request.signerAccountId,
          "MultisigProposeDto.signer_account_id",
        ),
      ],
      [
        encodeOptionValue(
          request.public_key_hex ?? request.publicKeyHex ?? null,
          encodeNoritoStringValue,
          "MultisigProposeDto.public_key_hex",
        ),
      ],
      [
        encodeOptionValue(
          request.signature_b64 ?? request.signatureB64 ?? null,
          encodeExactBase64StringValue,
          "MultisigProposeDto.signature_b64",
        ),
      ],
      [
        encodeOptionValue(
          request.creation_time_ms ?? request.creationTimeMs ?? null,
          encodeU64NumberValue,
          "MultisigProposeDto.creation_time_ms",
        ),
      ],
      [
        encodeFeePaymentIntentValue(
          request.fee_payment ?? request.feePayment,
          "MultisigProposeDto.fee_payment",
        ),
      ],
      [
        encodeOptionValue(
          request.memo ?? null,
          encodeNoritoStringValue,
          "MultisigProposeDto.memo",
        ),
      ],
      [
        encodeOptionValue(
          validationFeeMetadata.policyVersion,
          encodeNoritoStringValue,
          "MultisigProposeDto.validation_fee_policy_version",
        ),
      ],
      [
        encodeOptionValue(
          validationFeeMetadata.policyHash,
          encodeNoritoStringValue,
          "MultisigProposeDto.validation_fee_policy_hash",
        ),
      ],
      [
        encodeNoritoVec(request.instructions, (instruction, index) =>
          encodeEmbeddedInstructionBox(
            instruction,
            `MultisigProposeDto.instructions[${index}]`,
          ),
        ),
      ],
      [
        encodeOptionValue(
          validationFeeMetadata.instructionIndex,
          encodeNoritoStringValue,
          "MultisigProposeDto.validation_fee_instruction_index",
        ),
      ],
      [
        encodeOptionValue(
          validationFeeMetadata.transferEntryIndex,
          encodeNoritoStringValue,
          "MultisigProposeDto.validation_fee_transfer_entry_index",
        ),
      ],
    ]),
  );
  return frameNoritoPayload(payload, MULTISIG_PROPOSE_DTO_SCHEMA_HASH, COMPACT_LEN_FLAG);
}

function normalizeMultisigProposeValidationFeeMetadata(request) {
  rejectValidationFeeCamelCaseDtoFields(request);
  const policyVersion = request.validation_fee_policy_version ?? null;
  const policyHash = request.validation_fee_policy_hash ?? null;
  const instructionIndex = request.validation_fee_instruction_index ?? null;
  const transferEntryIndex = request.validation_fee_transfer_entry_index ?? null;
  const hasPolicyVersion = policyVersion !== null && policyVersion !== undefined;
  const hasPolicyHash = policyHash !== null && policyHash !== undefined;
  const hasInstructionIndex = instructionIndex !== null && instructionIndex !== undefined;
  const hasTransferEntryIndex = transferEntryIndex !== null && transferEntryIndex !== undefined;
  if (hasPolicyVersion !== hasPolicyHash) {
    throw new TypeError(
      "MultisigProposeDto.validation_fee_policy_version and validation_fee_policy_hash must be provided together",
    );
  }
  if (!hasPolicyVersion && hasInstructionIndex) {
    throw new TypeError(
      "MultisigProposeDto.validation_fee_instruction_index requires validation fee policy metadata",
    );
  }
  if (!hasPolicyVersion && hasTransferEntryIndex) {
    throw new TypeError(
      "MultisigProposeDto.validation_fee_transfer_entry_index requires validation fee policy metadata",
    );
  }
  if (hasTransferEntryIndex && !hasInstructionIndex) {
    throw new TypeError(
      "MultisigProposeDto.validation_fee_transfer_entry_index requires validation_fee_instruction_index",
    );
  }
  if (!hasPolicyVersion) {
    return { policyVersion: null, policyHash: null, instructionIndex: null, transferEntryIndex: null };
  }
  return {
    policyVersion: normalizeU64Input(
      policyVersion,
      "MultisigProposeDto.validation_fee_policy_version",
    ).toString(),
    policyHash: normalizeValidationFeePolicyHashString(
      policyHash,
      "MultisigProposeDto.validation_fee_policy_hash",
    ),
    instructionIndex: hasInstructionIndex
      ? normalizeU64Input(
          instructionIndex,
          "MultisigProposeDto.validation_fee_instruction_index",
        ).toString()
      : null,
    transferEntryIndex: hasTransferEntryIndex
      ? normalizeU64Input(
          transferEntryIndex,
          "MultisigProposeDto.validation_fee_transfer_entry_index",
        ).toString()
      : null,
  };
}

function rejectValidationFeeCamelCaseDtoFields(request) {
  for (const [camelName, snakeName] of [
    ["validationFeePolicyVersion", "validation_fee_policy_version"],
    ["validationFeePolicyHash", "validation_fee_policy_hash"],
    ["validationFeeInstructionIndex", "validation_fee_instruction_index"],
    ["validationFeeTransferEntryIndex", "validation_fee_transfer_entry_index"],
  ]) {
    if (Object.prototype.hasOwnProperty.call(request, camelName)) {
      throw new TypeError(
        `MultisigProposeDto uses unsupported camelCase validation fee field ${camelName}; use ${snakeName}`,
      );
    }
  }
}

function normalizeValidationFeePolicyHashString(value, context) {
  if (typeof value !== JS_TYPE_STRING) {
    throw new TypeError(`${context} must be a 32-byte hex string`);
  }
  const trimmed = value.trim().toLowerCase();
  const normalized = trimmed.startsWith("0x") ? trimmed.slice(2) : trimmed;
  if (!/^[0-9a-f]{64}$/.test(normalized)) {
    throw new TypeError(`${context} must be a 32-byte hex string`);
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
    throw new TypeError("MultisigContractCallProposeDto request must be an object");
  }
  rejectInlinePrivateKeyFields(request, "MultisigContractCallProposeDto");
  const contractAddress = request.contract_address ?? request.contractAddress ?? null;
  const contractAlias = request.contract_alias ?? request.contractAlias ?? null;
  if ((contractAddress == null) === (contractAlias == null)) {
    throw new TypeError(
      "MultisigContractCallProposeDto requires exactly one of contract_address or contract_alias",
    );
  }
  const payloadValue = request.payload ?? request.contractPayload ?? null;
  const payload = withNoritoCompactLengths(() =>
    encodeStructValue([
      ...encodeMultisigAccountSelectorFields(
        request,
        "MultisigContractCallProposeDto.selector",
      ),
      [
        encodeAccountIdValue(
          request.signer_account_id ?? request.signerAccountId,
          "MultisigContractCallProposeDto.signer_account_id",
        ),
      ],
      [
        encodeOptionValue(
          request.public_key_hex ?? request.publicKeyHex ?? null,
          encodeNoritoStringValue,
          "MultisigContractCallProposeDto.public_key_hex",
        ),
      ],
      [
        encodeOptionValue(
          request.signature_b64 ?? request.signatureB64 ?? null,
          encodeExactBase64StringValue,
          "MultisigContractCallProposeDto.signature_b64",
        ),
      ],
      [
        encodeOptionValue(
          request.creation_time_ms ?? request.creationTimeMs ?? null,
          encodeU64NumberValue,
          "MultisigContractCallProposeDto.creation_time_ms",
        ),
      ],
      [
        encodeOptionValue(
          contractAddress,
          encodeNoritoStringValue,
          "MultisigContractCallProposeDto.contract_address",
        ),
      ],
      [
        encodeOptionValue(
          contractAlias,
          encodeNoritoStringValue,
          "MultisigContractCallProposeDto.contract_alias",
        ),
      ],
      [
        encodeNoritoStringValue(
          assertNonEmptyString(
            request.entrypoint,
            "MultisigContractCallProposeDto.entrypoint",
          ),
        ),
      ],
      [
        encodeOptionValue(
          payloadValue,
          encodeNoritoJsonValue,
          "MultisigContractCallProposeDto.payload",
        ),
      ],
      [
        encodeFeePaymentIntentValue(
          request.fee_payment ?? request.feePayment,
          "MultisigContractCallProposeDto.fee_payment",
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
    throw new TypeError("MultisigContractCallApproveDto request must be an object");
  }
  rejectInlinePrivateKeyFields(request, "MultisigContractCallApproveDto");
  const proposalId = request.proposal_id ?? request.proposalId ?? null;
  const instructionsHash = request.instructions_hash ?? request.instructionsHash ?? null;
  if (proposalId == null && instructionsHash == null) {
    throw new TypeError(
      "MultisigContractCallApproveDto requires proposal_id or instructions_hash",
    );
  }
  const payload = withNoritoCompactLengths(() =>
    encodeStructValue([
      ...encodeMultisigAccountSelectorFields(
        request,
        "MultisigContractCallApproveDto.selector",
      ),
      [
        encodeAccountIdValue(
          request.signer_account_id ?? request.signerAccountId,
          "MultisigContractCallApproveDto.signer_account_id",
        ),
      ],
      [
        encodeOptionValue(
          request.public_key_hex ?? request.publicKeyHex ?? null,
          encodeNoritoStringValue,
          "MultisigContractCallApproveDto.public_key_hex",
        ),
      ],
      [
        encodeOptionValue(
          request.signature_b64 ?? request.signatureB64 ?? null,
          encodeExactBase64StringValue,
          "MultisigContractCallApproveDto.signature_b64",
        ),
      ],
      [
        encodeOptionValue(
          request.creation_time_ms ?? request.creationTimeMs ?? null,
          encodeU64NumberValue,
          "MultisigContractCallApproveDto.creation_time_ms",
        ),
      ],
      [
        encodeFeePaymentIntentValue(
          request.fee_payment ?? request.feePayment,
          "MultisigContractCallApproveDto.fee_payment",
        ),
      ],
      [
        encodeOptionValue(
          proposalId,
          encodeNoritoStringValue,
          "MultisigContractCallApproveDto.proposal_id",
        ),
      ],
      [
        encodeOptionValue(
          instructionsHash,
          encodeNoritoStringValue,
          "MultisigContractCallApproveDto.instructions_hash",
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
    throw new TypeError(
      `${context} requires exactly one of multisig_account_id or multisig_account_alias`,
    );
  }
  return [
    [
      encodeOptionValue(
        multisigAccountId,
        encodeAccountIdValue,
        `${context}.multisig_account_id`,
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

function encodeEmbeddedInstructionBox(instruction, context) {
  const framed = Buffer.from(noritoEncodeInstruction(instruction));
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
export function noritoEncodeInstructionBoxArchive(instruction) {
  return withNoritoLengthFlags(COMPACT_LEN_FLAG, () =>
    encodeEmbeddedInstructionBox(instruction, "instruction"),
  );
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
    "instruction archive",
    outerFlags,
  );
  const wireId = decodeStringValue(
    readNoritoField(outerReader, "wire"),
    "instruction archive.wire",
    outerFlags,
  );
  const innerField = readNoritoField(outerReader, "inner");
  outerReader.assertEof();

  const innerReader = new BufferReader(
    innerField,
    "instruction archive.inner",
    0,
  );
  const innerFrame = readNoritoField(innerReader, "frame");
  innerReader.assertEof();
  const inner = decodeNoritoFrame(
    innerFrame,
    "instruction archive.frame",
    INNER_SCHEMA_HASH_BY_WIRE_ID[wireId] ?? null,
  );
  const decoded = withNoritoLengthFlags(inner.flags, () =>
    decodePureJsInstructionPayload(
      wireId,
      inner.payload,
      inner.flags,
      innerFrame,
    ),
  );
  const canonical = noritoEncodeInstructionBoxArchive(decoded);
  if (!archive.equals(canonical)) {
    throw new Error("instruction archive is not canonical Norito");
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
export function noritoDecodeInstruction(bytes, options = {}) {
  const buffer = toBuffer(bytes);
  let json;
  try {
    const native = resolveNative("noritoDecodeInstruction");
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
  } catch (error) {
    if (!shouldUsePureJsInstructionFallback(error)) {
      throw error;
    }
    try {
      const decoded = decodePureJsInstruction(buffer);
      validateDecodedInstructionProofAttachments(decoded);
      return options.parseJson === false ? JSON.stringify(decoded) : decoded;
    } catch (fallbackError) {
      if (!isPureJsUnsupportedInstructionError(fallbackError)) {
        throw fallbackError;
      }
      throw error;
    }
  }
  const decoded = JSON.parse(json);
  validateDecodedInstructionProofAttachments(decoded);
  return options.parseJson === false ? json : decoded;
}

function validateDecodedInstructionProofAttachments(instruction) {
  rejectRetiredGenericZkInstruction(instruction);
  if (!isPlainObject(instruction) || !isPlainObject(instruction.zk)) {
    return;
  }
  for (const [variant, field] of [
    ["SubmitBallot", "ballot_proof"],
    ["FinalizeElection", "tally_proof"],
  ]) {
    const payload = instruction.zk[variant];
    if (!isPlainObject(payload)) {
      continue;
    }
    if (!Object.prototype.hasOwnProperty.call(payload, field)) {
      throw new TypeError(`zk.${variant}.${field} is required`);
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
export function inspectSubscriptionTriggerAction(encodedAction) {
  if (
    typeof encodedAction !== JS_TYPE_STRING ||
    encodedAction.length === 0 ||
    encodedAction.trim() !== encodedAction
  ) {
    throw new TypeError(
      "inspectSubscriptionTriggerAction encodedAction must be a canonical non-empty string",
    );
  }
  const native = resolveNative("inspectSubscriptionTriggerAction");
  const payload = native.inspectSubscriptionTriggerAction(encodedAction);
  try {
    return JSON.parse(payload);
  } catch (error) {
    throw new Error(
      `native subscription trigger inspection returned invalid JSON: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }
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
    throw new Error(`${context}.leaf_count must be non-zero`);
  }
  return {
    root: decodeHashValue(fields.root, `${context}.root`),
    leaf_count: leafCount,
  };
}

function decodeBlockReceiptProofValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["leaf", "proof"]);
  return {
    leaf: decodeHashValue(fields.leaf, `${context}.leaf`),
    proof: decodeBlockMerkleProofValue(fields.proof, `${context}.proof`),
  };
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

function decodeTransferDeltaTranscriptValue(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "from_account",
    "to_account",
    "asset_definition",
    "amount",
    "from_balance_before",
    "from_balance_after",
    "to_balance_before",
    "to_balance_after",
    "from_smt_witness",
    "to_smt_witness",
  ]);
  return {
    from_account: decodeAccountIdValue(fields.from_account, `${context}.from_account`),
    to_account: decodeAccountIdValue(fields.to_account, `${context}.to_account`),
    asset_definition: decodeAssetDefinitionIdValue(
      fields.asset_definition,
      `${context}.asset_definition`,
    ),
    amount: decodeQuantityValue(fields.amount, `${context}.amount`),
    from_balance_before: decodeQuantityValue(
      fields.from_balance_before,
      `${context}.from_balance_before`,
    ),
    from_balance_after: decodeQuantityValue(
      fields.from_balance_after,
      `${context}.from_balance_after`,
    ),
    to_balance_before: decodeQuantityValue(
      fields.to_balance_before,
      `${context}.to_balance_before`,
    ),
    to_balance_after: decodeQuantityValue(
      fields.to_balance_after,
      `${context}.to_balance_after`,
    ),
    from_smt_witness: decodeTransferSmtWitnessValue(
      fields.from_smt_witness,
      `${context}.from_smt_witness`,
    ),
    to_smt_witness: decodeTransferSmtWitnessValue(
      fields.to_smt_witness,
      `${context}.to_smt_witness`,
    ),
  };
}

function decodeTransferTranscriptValue(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "batch_hash",
    "deltas",
    "authority_digest",
    "poseidon_preimage_digest",
  ]);
  return {
    batch_hash: decodeHashValue(fields.batch_hash, `${context}.batch_hash`),
    deltas: decodeNoritoVec(
      fields.deltas,
      (entry, index) =>
        decodeTransferDeltaTranscriptValue(entry, `${context}.deltas[${index}]`),
      `${context}.deltas`,
    ),
    authority_digest: decodeHashValue(
      fields.authority_digest,
      `${context}.authority_digest`,
    ),
    poseidon_preimage_digest: decodeOptionValue(
      fields.poseidon_preimage_digest,
      decodeHashValue,
      `${context}.poseidon_preimage_digest`,
    ),
  };
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
      throw new Error(`${context} keys are not in canonical strict order`);
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
    throw new Error("BlockProofs uses an unsupported packed Norito layout");
  }
  return withNoritoLengthFlags(frame.flags & COMPACT_LEN_FLAG, () => {
    const fields = decodeStructFields(frame.payload, "BlockProofs", [
      "block_height",
      "block_hash",
      "executed_block_wire_hash",
      "entry_hash",
      "entry_commitment",
      "entry_proof",
      "result_commitment",
      "result_proof",
      "fastpq_transcripts",
    ]);
    const blockHeight = decodeU64Value(fields.block_height, "BlockProofs.block_height");
    if (blockHeight === "0") {
      throw new Error("BlockProofs.block_height must be non-zero");
    }
    const entryCommitment = decodeBlockMerkleCommitmentValue(
      fields.entry_commitment,
      "BlockProofs.entry_commitment",
    );
    const resultCommitment = decodeBlockMerkleCommitmentValue(
      fields.result_commitment,
      "BlockProofs.result_commitment",
    );
    if (entryCommitment.leaf_count !== resultCommitment.leaf_count) {
      throw new Error("BlockProofs entry/result commitment leaf counts must match");
    }
    return {
      block_height: blockHeight,
      block_hash: decodeHashValue(fields.block_hash, "BlockProofs.block_hash"),
      executed_block_wire_hash: decodeHashValue(
        fields.executed_block_wire_hash,
        "BlockProofs.executed_block_wire_hash",
      ),
      entry_hash: decodeHashValue(fields.entry_hash, "BlockProofs.entry_hash"),
      entry_commitment: entryCommitment,
      entry_proof: decodeBlockReceiptProofValue(
        fields.entry_proof,
        "BlockProofs.entry_proof",
      ),
      result_commitment: resultCommitment,
      result_proof: decodeBlockReceiptProofValue(
        fields.result_proof,
        "BlockProofs.result_proof",
      ),
      fastpq_transcripts: decodeFastpqTranscriptMap(
        fields.fastpq_transcripts,
        "BlockProofs.fastpq_transcripts",
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
  const payload = encodeOpenVerifyEnvelopePayload(envelope, "OpenVerifyEnvelope");
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
    "OpenVerifyEnvelope",
    OPEN_VERIFY_ENVELOPE_SCHEMA_HASH,
  );
  return decodeOpenVerifyEnvelopePayload(
    frame.payload,
    "OpenVerifyEnvelope",
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
    throw new RangeError(
      `PrivacyExact12FixtureBundleV1 base64 exceeds the ${PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1}-byte archive limit`,
    );
  }
  const archive = decodeExactStandardBase64(
    value,
    "PrivacyExact12FixtureBundleV1 base64",
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
    throw new TypeError("PrivacyExact12FixtureBundleV1 archive must not be empty");
  }
  if (view.length > PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1) {
    throw new RangeError(
      `PrivacyExact12FixtureBundleV1 archive exceeds ${PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1} bytes`,
    );
  }
  const archive = Buffer.from(view);
  const frame = validateNoritoFrame(archive, {
    context: "PrivacyExact12FixtureBundleV1",
    expectedSchemaHash: PRIVACY_EXACT12_FIXTURE_BUNDLE_SCHEMA_HASH_V1,
    expectedPaddingLength: 0,
    requireNonEmptyPayload: true,
  });
  if (frame.flags !== COMPACT_LEN_FLAG) {
    throw new Error(
      `PrivacyExact12FixtureBundleV1 must use canonical layout flags 0x${COMPACT_LEN_FLAG.toString(16)}`,
    );
  }
  const bundle = withNoritoCompactLengths(() =>
    decodePrivacyExact12FixtureBundlePayloadV1(frame.payload),
  );
  const canonical = encodePrivacyExact12FixtureBundleCanonicalV1(bundle);
  if (!canonical.equals(archive)) {
    throw new Error(
      "PrivacyExact12FixtureBundleV1 archive is not canonical or contains trailing data",
    );
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

function decodePrivacyExact12FixtureBundlePayloadV1(payload) {
  const fields = decodeStructFields(payload, "PrivacyExact12FixtureBundleV1", [
    "version",
    "rows",
  ]);
  const version = decodeU32Value(
    fields.version,
    "PrivacyExact12FixtureBundleV1.version",
  );
  if (version !== 1) {
    throw new RangeError("PrivacyExact12FixtureBundleV1.version must be exactly 1");
  }
  const reader = new BufferReader(
    fields.rows,
    "PrivacyExact12FixtureBundleV1.rows",
    COMPACT_LEN_FLAG,
  );
  const count = bigintToSafeNumber(
    reader.readU64LE("count"),
    "PrivacyExact12FixtureBundleV1.rows.count",
  );
  if (count !== PRIVACY_EXACT12_PROTOCOL_IDS_V1.length) {
    throw new RangeError(
      `PrivacyExact12FixtureBundleV1.rows must contain exactly ${PRIVACY_EXACT12_PROTOCOL_IDS_V1.length} rows`,
    );
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
  const context = `PrivacyExact12FixtureBundleV1.rows[${rowIndex}]`;
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
    throw new TypeError(`${context}.protocol_id contains ${description}`);
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
      `${context}.submit_proof_instruction_norito`,
    ),
    transactionIntentProjectionNorito:
      decodePrivacyExact12NonEmptyByteVectorV1(
        fields.transaction_intent_projection_norito,
        `${context}.transaction_intent_projection_norito`,
      ),
    transactionIntentDigest: decodeFixedBytesValue(
      fields.transaction_intent_digest,
      32,
      `${context}.transaction_intent_digest`,
    ),
    unsignedTransactionPayloadNorito:
      decodePrivacyExact12NonEmptyByteVectorV1(
        fields.unsigned_transaction_payload_norito,
        `${context}.unsigned_transaction_payload_norito`,
      ),
    signedTransactionVersionedNorito:
      decodePrivacyExact12NonEmptyByteVectorV1(
        fields.signed_transaction_versioned_norito,
        `${context}.signed_transaction_versioned_norito`,
      ),
    signedTransactionHash: decodeFixedBytesValue(
      fields.signed_transaction_hash,
      32,
      `${context}.signed_transaction_hash`,
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
    throw new TypeError(`${context} must not be empty`);
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
    throw new TypeError(`${context}.protocolId is unknown, duplicated, or out of order`);
  }
  if (row.submitProofWireId !== PRIVACY_EXACT12_SUBMIT_PROOF_WIRE_ID_V1) {
    throw new TypeError(
      `${context}.submitProofWireId must be exactly ${PRIVACY_EXACT12_SUBMIT_PROOF_WIRE_ID_V1}`,
    );
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
    throw new TypeError(`${context}.statementNorito carries a substituted protocol`);
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
  if (
    decodeU32Value(
      envelopeFields.protocol_id,
      `${context}.envelopeNorito.protocol_id`,
    ) !== rowIndex
  ) {
    throw new TypeError(`${context}.envelopeNorito carries a substituted protocol`);
  }
  if (!envelopeFields.statement.equals(statementFrame.payload)) {
    throw new TypeError(`${context}.envelopeNorito does not contain statementNorito`);
  }
  const proof = decodePrivacyExact12TaggedPayloadV1(
    envelopeFields.proof,
    `${context}.envelopeNorito.proof`,
  );
  if (proof.tag !== rowIndex) {
    throw new TypeError(`${context}.envelopeNorito proof carries a substituted protocol`);
  }

  const instructionFrame = validatePrivacyExact12NestedFrameV1(
    row.submitProofInstructionNorito,
    PRIVACY_EXACT12_SUBMIT_PROOF_SCHEMA_HASH_V1,
    PRIVACY_EXACT12_ALIGNED_NESTED_FRAME_PADDING_V1,
    `${context}.submitProofInstructionNorito`,
  );
  const instructionFields = withNoritoCompactLengths(() =>
    decodeStructFields(
      instructionFrame.payload,
      `${context}.submitProofInstructionNorito.payload`,
      ["envelope"],
    ),
  );
  assertPrivacyExact12CanonicalStructPayloadV1(
    instructionFrame.payload,
    instructionFields,
    ["envelope"],
    `${context}.submitProofInstructionNorito.payload`,
  );
  if (!instructionFields.envelope.equals(envelopeFrame.payload)) {
    throw new TypeError(
      `${context}.submitProofInstructionNorito does not contain envelopeNorito`,
    );
  }

  const projectionFrame = validatePrivacyExact12NestedFrameV1(
    row.transactionIntentProjectionNorito,
    PRIVACY_EXACT12_TRANSACTION_PAYLOAD_SCHEMA_HASH_V1,
    PRIVACY_EXACT12_TRANSACTION_PAYLOAD_FRAME_PADDING_V1,
    `${context}.transactionIntentProjectionNorito`,
  );
  const projectionFields = decodePrivacyExact12TransactionPayloadV1(
    projectionFrame.payload,
    `${context}.transactionIntentProjectionNorito.payload`,
  );
  const unsignedFields = decodePrivacyExact12TransactionPayloadV1(
    row.unsignedTransactionPayloadNorito,
    `${context}.unsignedTransactionPayloadNorito`,
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
      throw new TypeError(
        `${context}.transaction intent projection changed independent field ${field}`,
      );
    }
  }
  if (
    unsignedFields.admission_intent.length !== 4 ||
    unsignedFields.admission_intent.readUInt32LE(0) !== 0
  ) {
    throw new TypeError(
      `${context}.unsignedTransactionPayloadNorito.admission_intent must be TransactionAdmissionIntent::Ordinary`,
    );
  }
  const expectedCreationTime = 1_700_000_000_000n + BigInt(rowIndex);
  if (
    decodeU64Value(
      unsignedFields.creation_time_ms,
      `${context}.unsignedTransactionPayloadNorito.creation_time_ms`,
    ) !== expectedCreationTime.toString()
  ) {
    throw new TypeError(`${context} carries a substituted transaction creation time`);
  }
  const nonce = decodeOptionValue(
    unsignedFields.nonce,
    decodeU32Value,
    `${context}.unsignedTransactionPayloadNorito.nonce`,
  );
  if (nonce !== rowIndex + 1) {
    throw new TypeError(`${context} carries a substituted transaction nonce`);
  }
  const attachments = decodeOptionValue(
    unsignedFields.attachments,
    (payload) => payload,
    `${context}.unsignedTransactionPayloadNorito.attachments`,
  );
  if (attachments !== null) {
    throw new TypeError(`${context} must not carry transaction attachments`);
  }

  const instructionOffset = row.unsignedTransactionPayloadNorito.indexOf(
    row.submitProofInstructionNorito,
  );
  if (instructionOffset < 0) {
    throw new TypeError(
      `${context}.unsignedTransactionPayloadNorito does not contain the byte-complete instruction`,
    );
  }
  if (
    row.unsignedTransactionPayloadNorito.indexOf(
      Buffer.from(PRIVACY_EXACT12_SUBMIT_PROOF_WIRE_ID_V1, UTF8_ENCODING),
    ) < 0
  ) {
    throw new TypeError(
      `${context}.unsignedTransactionPayloadNorito does not contain the exact submission wire id`,
    );
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
    throw new TypeError(`${context}.transactionIntentDigest does not match its projection`);
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
    throw new TypeError(
      `${context}.signedTransactionHash does not match the unsigned transaction intent`,
    );
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
    throw new Error(`${context} must use canonical compact-length layout flags`);
  }
  const canonical = frameNoritoPayload(
    frame.payload,
    schemaHash,
    COMPACT_LEN_FLAG,
    expectedPaddingLength,
  );
  if (!canonical.equals(bytes)) {
    throw new Error(`${context} is not a canonical uncompressed Norito frame`);
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
    throw new Error(`${context} is not a canonical tagged payload`);
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
    throw new Error(`${context} contains a non-canonical field layout`);
  }
}

function validatePrivacyExact12SignedTransactionV1(row, context) {
  const signed = row.signedTransactionVersionedNorito;
  if (signed[0] !== 1) {
    throw new TypeError(
      `${context}.signedTransactionVersionedNorito must use version 1`,
    );
  }
  const payload = signed.subarray(1);
  const fields = withNoritoCompactLengths(() =>
    decodeStructFields(
      payload,
      `${context}.signedTransactionVersionedNorito.payload`,
      ["signature", "payload", "multisig_signatures"],
    ),
  );
  assertPrivacyExact12CanonicalStructPayloadV1(
    payload,
    fields,
    ["signature", "payload", "multisig_signatures"],
    `${context}.signedTransactionVersionedNorito.payload`,
  );
  if (fields.signature.length === 0) {
    throw new TypeError(`${context}.signedTransactionVersionedNorito has no signature`);
  }
  if (!fields.payload.equals(row.unsignedTransactionPayloadNorito)) {
    throw new TypeError(
      `${context}.signedTransactionVersionedNorito does not contain the unsigned payload`,
    );
  }
  const multisig = decodeOptionValue(
    fields.multisig_signatures,
    (entry) => entry,
    `${context}.signedTransactionVersionedNorito.multisig_signatures`,
  );
  if (multisig !== null) {
    throw new TypeError(
      `${context}.signedTransactionVersionedNorito must not carry multisig signatures`,
    );
  }
}

function normalizePrivacyExact12FixtureBundleInputV1(value) {
  assertExactObjectKeys(value, ["version", "rows"], "PrivacyExact12FixtureBundleV1");
  if (value.version !== 1) {
    throw new TypeError("PrivacyExact12FixtureBundleV1.version must be exactly 1");
  }
  if (
    !Array.isArray(value.rows) ||
    value.rows.length !== PRIVACY_EXACT12_PROTOCOL_IDS_V1.length
  ) {
    throw new TypeError(
      `PrivacyExact12FixtureBundleV1.rows must contain exactly ${PRIVACY_EXACT12_PROTOCOL_IDS_V1.length} rows`,
    );
  }
  preflightPrivacyExact12FixtureBundleInputV1(value.rows);
  const rows = value.rows.map((row, rowIndex) => {
    const context = `PrivacyExact12FixtureBundleV1.rows[${rowIndex}]`;
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
        `${context}.submitProofInstructionNorito`,
      ),
      transactionIntentProjectionNorito: normalizePrivacyExact12InputBytesV1(
        row.transactionIntentProjectionNorito,
        `${context}.transactionIntentProjectionNorito`,
      ),
      transactionIntentDigest: normalizePrivacyExact12InputBytesV1(
        row.transactionIntentDigest,
        `${context}.transactionIntentDigest`,
        32,
      ),
      unsignedTransactionPayloadNorito: normalizePrivacyExact12InputBytesV1(
        row.unsignedTransactionPayloadNorito,
        `${context}.unsignedTransactionPayloadNorito`,
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
      throw new TypeError(`${context}.submitProofWireId must be a string`);
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
    const context = `PrivacyExact12FixtureBundleV1.rows[${rowIndex}]`;
    assertExactObjectKeys(row, PRIVACY_EXACT12_PUBLIC_ROW_FIELD_NAMES_V1, context);
    for (const field of PRIVACY_EXACT12_PUBLIC_ROW_FIELD_NAMES_V1) {
      if (field === "protocolId" || field === "submitProofWireId") {
        continue;
      }
      const length = binaryByteLength(row[field]);
      if (length === null) {
        throw new TypeError(`${context}.${field} must be an exact byte sequence`);
      }
      declaredBytes += length;
      if (declaredBytes > PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1) {
        throw new RangeError(
          `PrivacyExact12FixtureBundleV1 fields exceed the ${PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1}-byte archive limit`,
        );
      }
    }
    if (typeof row.submitProofWireId !== JS_TYPE_STRING) {
      throw new TypeError(`${context}.submitProofWireId must be a string`);
    }
    declaredBytes += Buffer.byteLength(row.submitProofWireId, UTF8_ENCODING);
    if (declaredBytes > PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1) {
      throw new RangeError(
        `PrivacyExact12FixtureBundleV1 fields exceed the ${PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1}-byte archive limit`,
      );
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
        throw new TypeError(`${context}[${index}] must be an unsigned byte`);
      }
      bytes[index] = byte;
    }
  } else {
    throw new TypeError(`${context} must be an exact byte sequence`);
  }
  if (exactLength === null && bytes.length === 0) {
    throw new TypeError(`${context} must not be empty`);
  }
  if (exactLength !== null && bytes.length !== exactLength) {
    throw new TypeError(`${context} must contain exactly ${exactLength} bytes`);
  }
  return bytes;
}

function encodePrivacyExact12FixtureBundleCanonicalV1(bundle) {
  const payload = withNoritoCompactLengths(() =>
    encodeStructValue([
      [encodeU32Value(bundle.version, "PrivacyExact12FixtureBundleV1.version")],
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
    throw new RangeError(
      `PrivacyExact12FixtureBundleV1 archive exceeds ${PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1} bytes`,
    );
  }
  return archive;
}

function encodePrivacyExact12FixtureRowV1(row, rowIndex) {
  const context = `PrivacyExact12FixtureBundleV1.rows[${rowIndex}]`;
  return encodeStructValue([
    [encodeU32Value(rowIndex, `${context}.protocol_id`)],
    [encodeByteVecValue(row.statementNorito, `${context}.statement_norito`)],
    [encodeByteVecValue(row.envelopeNorito, `${context}.envelope_norito`)],
    [encodeNoritoStringValue(row.submitProofWireId)],
    [
      encodeByteVecValue(
        row.submitProofInstructionNorito,
        `${context}.submit_proof_instruction_norito`,
      ),
    ],
    [
      encodeByteVecValue(
        row.transactionIntentProjectionNorito,
        `${context}.transaction_intent_projection_norito`,
      ),
    ],
    [
      encodeFixedBytesValue(
        row.transactionIntentDigest,
        32,
        `${context}.transaction_intent_digest`,
      ),
    ],
    [
      encodeByteVecValue(
        row.unsignedTransactionPayloadNorito,
        `${context}.unsigned_transaction_payload_norito`,
      ),
    ],
    [
      encodeByteVecValue(
        row.signedTransactionVersionedNorito,
        `${context}.signed_transaction_versioned_norito`,
      ),
    ],
    [
      encodeFixedBytesValue(
        row.signedTransactionHash,
        32,
        `${context}.signed_transaction_hash`,
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
  throw new TypeError("bytes must be a Buffer, ArrayBuffer, or typed array");
}

function encodePureJsInstruction(instruction) {
  return withNoritoLengthFlags(COMPACT_LEN_FLAG, () =>
    encodePureJsInstructionPayload(instruction),
  );
}

function encodePureJsInstructionPayload(instruction) {
  if (!isPlainObject(instruction)) {
    throw new TypeError("instruction must be a JSON object");
  }
  if (isPlainObject(instruction.Mint)) {
    if (isPlainObject(instruction.Mint.Asset)) {
      const body = encodeAssetInstructionBody(instruction.Mint.Asset, "Mint.Asset");
      return encodeEnumInstruction("iroha.mint", 0, body);
    }
    if (isPlainObject(instruction.Mint.TriggerRepetitions)) {
      const body = encodeTriggerRepetitionsBody(
        instruction.Mint.TriggerRepetitions,
        "Mint.TriggerRepetitions",
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
        "Burn.TriggerRepetitions",
      );
      return encodeEnumInstruction("iroha.burn", 1, body);
    }
  }
  if (isPlainObject(instruction.Transfer) && isPlainObject(instruction.Transfer.Asset)) {
    const body = encodeTransferAssetBody(instruction.Transfer.Asset);
    return encodeEnumInstruction("iroha.transfer", 2, body);
  }
  if (isPlainObject(instruction.Transfer) && isPlainObject(instruction.Transfer.Domain)) {
    return encodeEnumInstruction(
      "iroha.transfer",
      0,
      encodeTransferObjectBody(
        instruction.Transfer.Domain,
        "Transfer.Domain",
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
      "iroha.transfer",
      1,
      encodeTransferObjectBody(
        instruction.Transfer.AssetDefinition,
        "Transfer.AssetDefinition",
        encodeAccountIdValue,
        encodeAssetDefinitionIdValue,
        encodeAccountIdValue,
      ),
    );
  }
  if (isPlainObject(instruction.Transfer) && isPlainObject(instruction.Transfer.Nft)) {
    return encodeEnumInstruction(
      "iroha.transfer",
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
      "iroha.register",
      1,
      encodeNoritoField(encodeNewDomainValue(instruction.Register.Domain, "Register.Domain")),
    );
  }
  if (isPlainObject(instruction.Register) && isPlainObject(instruction.Register.Account)) {
    return encodeEnumInstruction(
      "iroha.register",
      2,
      encodeNoritoField(encodeNewAccountValue(instruction.Register.Account, "Register.Account")),
    );
  }
  if (
    isPlainObject(instruction.Register) &&
    isPlainObject(instruction.Register.AssetDefinition)
  ) {
    return encodeEnumInstruction(
      "iroha.register",
      3,
      encodeNoritoField(
        encodeNewAssetDefinitionValue(
          instruction.Register.AssetDefinition,
          "Register.AssetDefinition",
        ),
      ),
    );
  }
  if (isPlainObject(instruction.ExecuteTrigger)) {
    const payload = encodeExecuteTriggerPayload(instruction.ExecuteTrigger);
    return encodeInstructionEnvelope("iroha.execute_trigger", payload);
  }
  if (Object.prototype.hasOwnProperty.call(instruction, "CancelAssetLock")) {
    assertOnlyObjectKeys(instruction, ["CancelAssetLock"], "instruction");
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
      "instruction",
    );
    return encodeSetAssetTransferAvailabilityInstruction(
      instruction.SetAssetTransferAvailability,
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
      "iroha.custom",
      encodeCustomInstructionPayload(instruction.Custom),
    );
  }
  if (isPlainObject(instruction.Multisig)) {
    return encodeInstructionEnvelope(
      "iroha.custom",
      encodeCustomInstructionPayload({ payload: instruction.Multisig }),
    );
  }
  if (isPlainObject(instruction.MultisigRegister)) {
    return encodeInstructionEnvelope(
      "iroha.custom",
      encodeCustomInstructionPayload({ payload: { Register: instruction.MultisigRegister } }),
    );
  }
  if (isPlainObject(instruction.MultisigPropose)) {
    return encodeInstructionEnvelope(
      "iroha.custom",
      encodeCustomInstructionPayload({ payload: { Propose: instruction.MultisigPropose } }),
    );
  }
  if (isPlainObject(instruction.MultisigApprove)) {
    return encodeInstructionEnvelope(
      "iroha.custom",
      encodeCustomInstructionPayload({ payload: { Approve: instruction.MultisigApprove } }),
    );
  }
  if (isPlainObject(instruction.MultisigCancel)) {
    return encodeInstructionEnvelope(
      "iroha.custom",
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
    instruction.CastPlainBallot ||
    instruction.PersistCouncilForEpoch
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
    instruction.CommitContractDeployment ||
    instruction.UploadSmartContractCodeChunk ||
    instruction.FinalizeSmartContractCodeUpload ||
    instruction.CancelSmartContractCodeUpload ||
    instruction.RemoveSmartContractBytes
  ) {
    return encodeSmartContractInstruction(instruction);
  }
  throw new Error(
    `Internal Norito canonicalization supports ${SUPPORTED_JS_CANONICALIZATION_INSTRUCTIONS.join(", ")}. Received ${describeInstructionShape(instruction)}.`,
  );
}

function decodePureJsInstruction(buffer) {
  const { wireId, payload, innerFlags } = decodeInstructionEnvelope(buffer);
  return withNoritoLengthFlags(innerFlags, () =>
    decodePureJsInstructionPayload(wireId, payload, innerFlags, buffer),
  );
}

function decodePureJsInstructionPayload(wireId, payload, innerFlags, framedInstruction) {
  switch (wireId) {
    case "iroha.mint":
      return { Mint: decodeMintPayload(payload) };
    case "iroha.burn":
      return { Burn: decodeBurnPayload(payload) };
    case "iroha.register":
      return { Register: decodeRegisterPayload(payload) };
    case "iroha.transfer":
      return { Transfer: decodeTransferPayload(payload) };
    case "iroha.custom":
      return { Custom: decodeCustomInstructionPayload(payload) };
    case "iroha.execute_trigger":
      return { ExecuteTrigger: decodeExecuteTriggerPayload(payload) };
    case "iroha.rwa":
      return decodeRwaInstructionPayload(payload);
    case CANCEL_ASSET_LOCK_WIRE_ID:
      return decodeCancelAssetLockInstructionPayload(payload);
    case SET_ASSET_TRANSFER_AVAILABILITY_WIRE_ID:
      return decodeSetAssetTransferAvailabilityInstructionPayload(payload);
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
    case PERSIST_COUNCIL_FOR_EPOCH_WIRE_ID:
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
      return decodeSmartContractInstructionPayload(wireId, payload);
    case CREATE_KAIGI_WIRE_ID:
    case JOIN_KAIGI_WIRE_ID:
    case LEAVE_KAIGI_WIRE_ID:
    case END_KAIGI_WIRE_ID:
    case RECORD_KAIGI_USAGE_WIRE_ID:
    case SET_KAIGI_RELAY_MANIFEST_WIRE_ID:
    case REGISTER_KAIGI_RELAY_WIRE_ID:
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
      const cached = getCachedInstruction(framedInstruction);
      if (cached !== null) {
        return cached;
      }
      throw new Error(
        `Internal Norito decoder does not support ${wireId}. Run \`npm run build:native\` for full instruction coverage.`,
      );
  }
}

function decodeInstructionEnvelope(bytes) {
  const outer = decodeNoritoFrame(bytes, "instruction", INSTRUCTION_BOX_SCHEMA_HASH);
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
  context = "instruction",
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
    throw new Error(
      `${context} uses unsupported instruction wire id ${wireId}; native embedding requires a schema hash`,
    );
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
    "instruction",
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
    throw new TypeError("RecordSccpMessage.payload_bytes is required");
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
    "RecordSccpMessage",
    COMPACT_LEN_FLAG,
  );
  return frameNoritoPayload(
    outerPayload,
    INSTRUCTION_BOX_SCHEMA_HASH,
    COMPACT_LEN_FLAG,
  );
}

function decodeRecordSccpMessagePayload(payload, innerFlags) {
  const reader = new BufferReader(payload, "RecordSccpMessage", innerFlags);
  const field = readNoritoField(reader, "payload_bytes");
  reader.assertEof();
  if (field.length < 8) {
    throw new Error("RecordSccpMessage.payload_bytes is too short");
  }
  const count = bigintToSafeNumber(
    field.readBigUInt64LE(0),
    "RecordSccpMessage.payload_bytes.length",
  );
  const payloadBytes = field.subarray(8);
  if (payloadBytes.length !== count) {
    throw new Error("RecordSccpMessage.payload_bytes length mismatch");
  }
  return { payload_bytes: Array.from(payloadBytes) };
}

function assertWellFormedUtf16(value, context) {
  for (let index = 0; index < value.length; index += 1) {
    const codeUnit = value.charCodeAt(index);
    if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) {
        throw new TypeError(`${context} must not contain unpaired UTF-16 surrogates`);
      }
      index += 1;
    } else if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
      throw new TypeError(`${context} must not contain unpaired UTF-16 surrogates`);
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
    throw new TypeError("CancelAssetLockV1 must be a plain object");
  }
  const keys = Reflect.ownKeys(value);
  if (
    keys.length !== 2 ||
    !keys.includes("escrow_id") ||
    !keys.includes("expected_remaining_amount")
  ) {
    throw new TypeError(
      "CancelAssetLockV1 must contain exactly escrow_id and expected_remaining_amount",
    );
  }

  const { escrow_id: escrowId, expected_remaining_amount: expectedRemainingAmount } =
    value;
  if (typeof escrowId !== JS_TYPE_STRING) {
    throw new TypeError("CancelAssetLockV1.escrow_id must be a string");
  }
  assertWellFormedUtf16(escrowId, "CancelAssetLockV1.escrow_id");
  const hashMatch = CANONICAL_HASH_LITERAL_RE.exec(escrowId);
  if (hashMatch === null) {
    throw new TypeError(
      "CancelAssetLockV1.escrow_id must be one canonical uppercase checksummed hash literal",
    );
  }
  const [, hashBody, checksum] = hashMatch;
  const expectedChecksum = computeHashLiteralCrc("hash", hashBody);
  if (checksum !== expectedChecksum) {
    throw new TypeError(
      `CancelAssetLockV1.escrow_id has invalid checksum; expected ${expectedChecksum}`,
    );
  }
  const hashBytes = Buffer.from(hashBody, HEX_ENCODING);
  if ((hashBytes[hashBytes.length - 1] & 1) === 0) {
    throw new TypeError(
      "CancelAssetLockV1.escrow_id must use a native hash with its marker bit set",
    );
  }

  if (typeof expectedRemainingAmount !== JS_TYPE_STRING) {
    throw new TypeError(
      "CancelAssetLockV1.expected_remaining_amount must be a canonical quantity string",
    );
  }
  assertWellFormedUtf16(
    expectedRemainingAmount,
    "CancelAssetLockV1.expected_remaining_amount",
  );
  const quantity = NumericV1.decodeQuantityJson(expectedRemainingAmount);
  if (quantity.mantissa <= 0n) {
    throw new RangeError(
      "CancelAssetLockV1.expected_remaining_amount must be greater than zero",
    );
  }

  return {
    escrow_id: escrowId,
    expected_remaining_amount: expectedRemainingAmount,
  };
}

function encodeCancelAssetLockPayload(value) {
  if (!isPlainObject(value)) {
    throw new TypeError("CancelAssetLock must be an object");
  }
  assertOnlyObjectKeys(
    value,
    ["escrow_id", "expected_remaining_amount"],
    "CancelAssetLock",
  );
  for (const field of ["escrow_id", "expected_remaining_amount"]) {
    if (!Object.prototype.hasOwnProperty.call(value, field)) {
      throw new TypeError(`CancelAssetLock.${field} is required`);
    }
  }
  const expected = parseNumericLiteral(
    value.expected_remaining_amount,
    CANCEL_LOCK_REMAINING_CONTEXT,
  );
  if (expected.mantissa <= 0n) {
    throw new RangeError(
      CANCEL_LOCK_REMAINING_MESSAGE,
    );
  }
  const payload = encodeStructValue([
    [
      encodeEscrowIdValue(
        value.escrow_id,
        "CancelAssetLock.escrow_id",
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
  const fields = decodeStructFields(payload, "CancelAssetLock", [
    "escrow_id",
    "expected_remaining_amount",
  ]);
  const expectedRemainingAmount = decodeQuantityValue(
    fields.expected_remaining_amount,
    CANCEL_LOCK_REMAINING_CONTEXT,
  );
  if (
    NumericV1.decodeQuantityJson(expectedRemainingAmount).mantissa <= 0n
  ) {
    throw new RangeError(
      CANCEL_LOCK_REMAINING_MESSAGE,
    );
  }
  return {
    CancelAssetLock: {
      escrow_id: decodeEscrowIdValue(
        fields.escrow_id,
        "CancelAssetLock.escrow_id",
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
    throw new TypeError(
      "CancelAssetLockV1 archive must be an owned, full-span Uint8Array",
    );
  }
  if (
    bytes.byteLength < CANCEL_ASSET_LOCK_V1_MIN_ARCHIVE_BYTES ||
    bytes.byteLength > CANCEL_ASSET_LOCK_V1_MAX_ARCHIVE_BYTES
  ) {
    throw new RangeError(
      `CancelAssetLockV1 archive must contain between ${CANCEL_ASSET_LOCK_V1_MIN_ARCHIVE_BYTES} and ${CANCEL_ASSET_LOCK_V1_MAX_ARCHIVE_BYTES} canonical bytes`,
    );
  }
  const archive = Buffer.from(bytes.buffer);
  const frame = validateNoritoFrame(archive, {
    context: "CancelAssetLockV1",
    expectedSchemaHash: CANCEL_ASSET_LOCK_V1_SCHEMA_HASH,
    expectedPaddingLength: 0,
    requireNonEmptyPayload: true,
  });
  if (frame.flags !== COMPACT_LEN_FLAG) {
    throw new Error(
      "CancelAssetLockV1 must use exactly the compact-length Norito flag",
    );
  }
  const decoded = withNoritoCompactLengths(
    () => decodeCancelAssetLockInstructionPayload(frame.payload).CancelAssetLock,
  );
  const canonical = normalizeStrictCancelAssetLockV1(decoded);
  const reencoded = encodeCancelAssetLockV1(canonical);
  if (!archive.equals(reencoded)) {
    throw new Error("CancelAssetLockV1 archive is not byte-canonical");
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
  throw new TypeError(`${context} must be exactly "Enabled" or "Disabled"`);
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
  throw new Error(`${context} uses unsupported availability tag ${tag}`);
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
    throw new TypeError(
      `${context} must be non-empty unpadded text when provided`,
    );
  }
  if (/[\u0000-\u001f\u007f-\u009f]/u.test(reason)) {
    throw new TypeError(`${context} must not contain control characters`);
  }
  if (
    Buffer.byteLength(reason, UTF8_ENCODING) >
    ASSET_TRANSFER_AVAILABILITY_MAX_REASON_BYTES_V1
  ) {
    throw new RangeError(`${context} exceeds 512 UTF-8 bytes`);
  }
}

function encodeSetAssetTransferAvailabilityInstruction(value) {
  if (!isPlainObject(value)) {
    throw new TypeError("SetAssetTransferAvailability must be an object");
  }
  const fields = [
    "account_id",
    "asset_definition_id",
    "expected_revision",
    "incoming",
    "outgoing",
    "reason",
  ];
  assertOnlyObjectKeys(value, fields, SET_ASSET_TRANSFER_AVAILABILITY_VARIANT);
  for (const field of fields.slice(0, 5)) {
    if (!Object.prototype.hasOwnProperty.call(value, field)) {
      throw new TypeError(`SetAssetTransferAvailability.${field} is required`);
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
        "SetAssetTransferAvailability.account_id",
      ),
    ],
    [
      encodeAssetDefinitionIdValue(
        value.asset_definition_id,
        "SetAssetTransferAvailability.asset_definition_id",
      ),
    ],
    [
      encodeU64NumberValue(
        value.expected_revision,
        "SetAssetTransferAvailability.expected_revision",
      ),
    ],
    [
      encodeAssetTransferAvailabilityValue(
        value.incoming,
        "SetAssetTransferAvailability.incoming",
      ),
    ],
    [
      encodeAssetTransferAvailabilityValue(
        value.outgoing,
        "SetAssetTransferAvailability.outgoing",
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
    "account_id",
    "asset_definition_id",
    "expected_revision",
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
        "SetAssetTransferAvailability.account_id",
      ),
      asset_definition_id: decodeAssetDefinitionIdValue(
        fields.asset_definition_id,
        "SetAssetTransferAvailability.asset_definition_id",
      ),
      expected_revision: decodeU64Value(
        fields.expected_revision,
        "SetAssetTransferAvailability.expected_revision",
      ),
      incoming: decodeAssetTransferAvailabilityValue(
        fields.incoming,
        "SetAssetTransferAvailability.incoming",
      ),
      outgoing: decodeAssetTransferAvailabilityValue(
        fields.outgoing,
        "SetAssetTransferAvailability.outgoing",
      ),
      reason,
    },
  };
}

function decodeMintPayload(payload) {
  const reader = new BufferReader(payload, "Mint");
  const variantIndex = reader.readU32LE("variantIndex");
  const body = readNoritoField(reader, "body");
  reader.assertEof();
  switch (variantIndex) {
    case 0:
      return { Asset: decodeAssetInstructionBody(body, "Mint.Asset") };
    case 1:
      return {
        TriggerRepetitions: decodeTriggerRepetitionsBody(body, "Mint.TriggerRepetitions"),
      };
    default:
      throw new Error(`Internal Norito decoder does not support Mint variant ${variantIndex}`);
  }
}

function decodeBurnPayload(payload) {
  const reader = new BufferReader(payload, "Burn");
  const variantIndex = reader.readU32LE("variantIndex");
  const body = readNoritoField(reader, "body");
  reader.assertEof();
  switch (variantIndex) {
    case 0:
      return { Asset: decodeAssetInstructionBody(body, "Burn.Asset") };
    case 1:
      return {
        TriggerRepetitions: decodeTriggerRepetitionsBody(body, "Burn.TriggerRepetitions"),
      };
    default:
      throw new Error(`Internal Norito decoder does not support Burn variant ${variantIndex}`);
  }
}

function decodeTransferPayload(payload) {
  const reader = new BufferReader(payload, "Transfer");
  const variantIndex = reader.readU32LE("variantIndex");
  const body = readNoritoField(reader, "body");
  reader.assertEof();
  switch (variantIndex) {
    case 0:
      return {
        Domain: decodeTransferObjectBody(
          body,
          "Transfer.Domain",
          decodeAccountIdValue,
          decodeDomainIdValue,
          decodeAccountIdValue,
        ),
      };
    case 1:
      return {
        AssetDefinition: decodeTransferObjectBody(
          body,
          "Transfer.AssetDefinition",
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
      throw new Error(
        `Internal Norito decoder does not support Transfer variant ${variantIndex}.`,
      );
  }
}

function decodeRegisterPayload(payload) {
  const reader = new BufferReader(payload, "Register");
  const variantIndex = reader.readU32LE("variantIndex");
  const body = readNoritoField(reader, "body");
  reader.assertEof();
  switch (variantIndex) {
    case 1:
      return {
        Domain: decodeNewDomainValue(
          unwrapStructBody(body, "Register.Domain"),
          "Register.Domain",
        ),
      };
    case 2:
      return {
        Account: decodeNewAccountValue(
          unwrapStructBody(body, "Register.Account"),
          "Register.Account",
        ),
      };
    case 3:
      return {
        AssetDefinition: decodeNewAssetDefinitionValue(
          unwrapStructBody(body, "Register.AssetDefinition"),
          "Register.AssetDefinition",
        ),
      };
    default:
      throw new Error(
        `Internal Norito decoder does not support Register variant ${variantIndex}.`,
      );
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
        "contract_address",
        "code_hash",
        "abi_hash",
        "abi_version",
        "manifest_provenance",
      ]);
      const decoded = {
        contract_address: decodeStringValue(
          fields.contract_address,
          "ProposeDeployContract.contract_address",
        ),
        code_hash: decodeGovernanceHash32Value(
          fields.code_hash,
          "ProposeDeployContract.code_hash",
        ),
        abi_hash: decodeGovernanceHash32Value(
          fields.abi_hash,
          "ProposeDeployContract.abi_hash",
        ),
        abi_version: decodeGovernanceAbiVersionValue(
          fields.abi_version,
          "ProposeDeployContract.abi_version",
        ),
      };
      const manifestProvenance = decodeOptionValue(
        fields.manifest_provenance,
        decodeManifestProvenanceValue,
        "ProposeDeployContract.manifest_provenance",
      );
      if (manifestProvenance !== null) {
        decoded.manifest_provenance = manifestProvenance;
      }
      return { ProposeDeployContract: decoded };
    }
    case CAST_ZK_BALLOT_WIRE_ID: {
      const fields = decodeStructFields(payload, "CastZkBallot", [
        "election_id",
        "proof_b64",
        "public_inputs_json",
      ]);
      return {
        CastZkBallot: {
          election_id: decodeStringValue(fields.election_id, "CastZkBallot.election_id"),
          proof_b64: decodeStringValue(fields.proof_b64, "CastZkBallot.proof_b64"),
          public_inputs_json: decodeStringValue(
            fields.public_inputs_json,
            "CastZkBallot.public_inputs_json",
          ),
        },
      };
    }
    case CAST_PLAIN_BALLOT_WIRE_ID: {
      const fields = decodeStructFields(payload, "CastPlainBallot", [
        "referendum_id",
        "owner",
        "amount",
        "duration_blocks",
        "direction",
      ]);
      return {
        CastPlainBallot: {
          referendum_id: decodeStringValue(fields.referendum_id, "CastPlainBallot.referendum_id"),
          owner: decodeAccountIdValue(fields.owner, "CastPlainBallot.owner"),
          amount: decodeQuantityValue(fields.amount, "CastPlainBallot.amount"),
          duration_blocks: decodeU64NumberValue(
            fields.duration_blocks,
            "CastPlainBallot.duration_blocks",
          ),
          direction: decodeU8Value(fields.direction, "CastPlainBallot.direction"),
        },
      };
    }
    case PERSIST_COUNCIL_FOR_EPOCH_WIRE_ID: {
      const fields = decodeStructFields(payload, "PersistCouncilForEpoch", [
        "epoch",
        "members",
        "alternates",
      ]);
      return {
        PersistCouncilForEpoch: {
          epoch: decodeU64NumberValue(fields.epoch, "PersistCouncilForEpoch.epoch"),
          members: decodeNoritoVec(
            fields.members,
            (entry, index) =>
              decodeAccountIdValue(entry, `PersistCouncilForEpoch.members[${index}]`),
            "PersistCouncilForEpoch.members",
          ),
          alternates: decodeNoritoVec(
            fields.alternates,
            (entry, index) =>
              decodeAccountIdValue(entry, `PersistCouncilForEpoch.alternates[${index}]`),
            "PersistCouncilForEpoch.alternates",
          ),
        },
      };
    }
    default:
      throw new Error(`unsupported governance wire id ${wireId}`);
  }
}

function decodeSocialInstructionPayload(wireId, payload) {
  switch (wireId) {
    case CLAIM_TWITTER_FOLLOW_REWARD_WIRE_ID: {
      const fields = decodeStructFields(payload, "ClaimTwitterFollowReward", ["binding_hash"]);
      return {
        ClaimTwitterFollowReward: {
          binding_hash: decodeKeyedHashValue(
            fields.binding_hash,
            "ClaimTwitterFollowReward.binding_hash",
          ),
        },
      };
    }
    case SEND_TO_TWITTER_WIRE_ID: {
      const fields = decodeStructFields(payload, "SendToTwitter", ["binding_hash", "amount"]);
      return {
        SendToTwitter: {
          binding_hash: decodeKeyedHashValue(fields.binding_hash, "SendToTwitter.binding_hash"),
          amount: decodeQuantityValue(fields.amount, "SendToTwitter.amount"),
        },
      };
    }
    case CANCEL_TWITTER_ESCROW_WIRE_ID: {
      const fields = decodeStructFields(payload, "CancelTwitterEscrow", ["binding_hash"]);
      return {
        CancelTwitterEscrow: {
          binding_hash: decodeKeyedHashValue(
            fields.binding_hash,
            "CancelTwitterEscrow.binding_hash",
          ),
        },
      };
    }
    default:
      throw new Error(`unsupported social wire id ${wireId}`);
  }
}

function decodeSmartContractInstructionPayload(wireId, payload) {
  switch (wireId) {
    case REGISTER_SMART_CONTRACT_CODE_WIRE_ID: {
      const fields = decodeStructFields(payload, "RegisterSmartContractCode", ["manifest"]);
      return {
        RegisterSmartContractCode: {
          manifest: decodeContractManifestValue(
            fields.manifest,
            "RegisterSmartContractCode.manifest",
          ),
        },
      };
    }
    case REGISTER_SMART_CONTRACT_BYTES_WIRE_ID: {
      const fields = decodeStructFields(payload, "RegisterSmartContractBytes", [
        "code_hash",
        "code",
      ]);
      return {
        RegisterSmartContractBytes: {
          code_hash: decodeHashValue(fields.code_hash, "RegisterSmartContractBytes.code_hash"),
          code: decodeByteVecAsBase64(fields.code, "RegisterSmartContractBytes.code"),
        },
      };
    }
    case DEACTIVATE_CONTRACT_INSTANCE_WIRE_ID: {
      const fields = decodeStructFields(payload, "DeactivateContractInstance", [
        "contract_address",
        "reason",
      ]);
      return {
        DeactivateContractInstance: {
          contract_address: decodeStringValue(
            fields.contract_address,
            "DeactivateContractInstance.contract_address",
          ),
          reason: decodeOptionValue(
            fields.reason,
            decodeStringValue,
            "DeactivateContractInstance.reason",
          ),
        },
      };
    }
    case ACTIVATE_CONTRACT_INSTANCE_WIRE_ID: {
      const fields = decodeStructFields(payload, "ActivateContractInstance", [
        "contract_address",
        "code_hash",
      ]);
      return {
        ActivateContractInstance: {
          contract_address: decodeStringValue(
            fields.contract_address,
            "ActivateContractInstance.contract_address",
          ),
          code_hash: decodeHashValue(fields.code_hash, "ActivateContractInstance.code_hash"),
        },
      };
    }
    case COMMIT_CONTRACT_DEPLOYMENT_WIRE_ID: {
      const fields = decodeStructFields(payload, "CommitContractDeployment", [
        "expected_deploy_nonce",
        "contract_address",
        "code_hash",
        "contract_alias",
        "lease_expiry_ms",
        "expected_previous_contract_address",
      ]);
      return {
        CommitContractDeployment: {
          expected_deploy_nonce: decodeU64Value(
            fields.expected_deploy_nonce,
            "CommitContractDeployment.expected_deploy_nonce",
          ),
          contract_address: decodeStringValue(
            fields.contract_address,
            "CommitContractDeployment.contract_address",
          ),
          code_hash: decodeHashValue(
            fields.code_hash,
            "CommitContractDeployment.code_hash",
          ),
          contract_alias: decodeStringValue(
            fields.contract_alias,
            "CommitContractDeployment.contract_alias",
          ),
          lease_expiry_ms: decodeOptionValue(
            fields.lease_expiry_ms,
            decodeU64Value,
            "CommitContractDeployment.lease_expiry_ms",
          ),
          expected_previous_contract_address: decodeOptionValue(
            fields.expected_previous_contract_address,
            decodeStringValue,
            EXPECTED_PREVIOUS_CONTRACT_CONTEXT,
          ),
        },
      };
    }
    case UPLOAD_SMART_CONTRACT_CODE_CHUNK_WIRE_ID: {
      const fields = decodeStructFields(payload, "UploadSmartContractCodeChunk", [
        "code_hash",
        "total_size",
        "chunk_index",
        "chunk_count",
        "chunk",
      ]);
      return {
        UploadSmartContractCodeChunk: {
          code_hash: decodeHashValue(
            fields.code_hash,
            "UploadSmartContractCodeChunk.code_hash",
          ),
          total_size: decodeU64Value(
            fields.total_size,
            "UploadSmartContractCodeChunk.total_size",
          ),
          chunk_index: decodeU32Value(
            fields.chunk_index,
            "UploadSmartContractCodeChunk.chunk_index",
          ),
          chunk_count: decodeU32Value(
            fields.chunk_count,
            "UploadSmartContractCodeChunk.chunk_count",
          ),
          chunk: decodeByteVecAsBase64(
            fields.chunk,
            "UploadSmartContractCodeChunk.chunk",
          ),
        },
      };
    }
    case FINALIZE_SMART_CONTRACT_CODE_UPLOAD_WIRE_ID: {
      const fields = decodeStructFields(payload, "FinalizeSmartContractCodeUpload", [
        "code_hash",
        "total_size",
        "chunk_count",
      ]);
      return {
        FinalizeSmartContractCodeUpload: {
          code_hash: decodeHashValue(
            fields.code_hash,
            "FinalizeSmartContractCodeUpload.code_hash",
          ),
          total_size: decodeU64Value(
            fields.total_size,
            "FinalizeSmartContractCodeUpload.total_size",
          ),
          chunk_count: decodeU32Value(
            fields.chunk_count,
            "FinalizeSmartContractCodeUpload.chunk_count",
          ),
        },
      };
    }
    case CANCEL_SMART_CONTRACT_CODE_UPLOAD_WIRE_ID: {
      const fields = decodeStructFields(payload, "CancelSmartContractCodeUpload", [
        "code_hash",
      ]);
      return {
        CancelSmartContractCodeUpload: {
          code_hash: decodeHashValue(
            fields.code_hash,
            "CancelSmartContractCodeUpload.code_hash",
          ),
        },
      };
    }
    case REMOVE_SMART_CONTRACT_BYTES_WIRE_ID: {
      const fields = decodeStructFields(payload, "RemoveSmartContractBytes", [
        "code_hash",
        "reason",
      ]);
      return {
        RemoveSmartContractBytes: {
          code_hash: decodeHashValue(fields.code_hash, "RemoveSmartContractBytes.code_hash"),
          reason: decodeOptionValue(
            fields.reason,
            decodeStringValue,
            "RemoveSmartContractBytes.reason",
          ),
        },
      };
    }
    default:
      throw new Error(`unsupported smart-contract wire id ${wireId}`);
  }
}

function decodeKaigiInstructionPayload(wireId, payload) {
  switch (wireId) {
    case CREATE_KAIGI_WIRE_ID: {
      const fields = decodeStructFields(payload, "Kaigi.CreateKaigi", [
        "call",
        "commitment",
        "nullifier",
        "roster_root",
        "proof",
      ]);
      return {
        Kaigi: {
          CreateKaigi: {
            call: decodeNewKaigiPayload(fields.call, "Kaigi.CreateKaigi.call"),
            commitment: decodeOptionValue(
              fields.commitment,
              decodeKaigiParticipantCommitmentValue,
              "Kaigi.CreateKaigi.commitment",
            ),
            nullifier: decodeOptionValue(
              fields.nullifier,
              decodeKaigiParticipantNullifierValue,
              "Kaigi.CreateKaigi.nullifier",
            ),
            roster_root: decodeOptionValue(
              fields.roster_root,
              decodeHashValue,
              "Kaigi.CreateKaigi.roster_root",
            ),
            proof: decodeOptionValue(
              fields.proof,
              decodeByteVecAsBase64,
              "Kaigi.CreateKaigi.proof",
            ),
          },
        },
      };
    }
    case JOIN_KAIGI_WIRE_ID:
    case LEAVE_KAIGI_WIRE_ID: {
      const fields = decodeStructFields(payload, `Kaigi.${wireId}`, [
        "call_id",
        "participant",
        "commitment",
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
              `Kaigi.${name}.commitment`,
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
      const fields = decodeStructFields(payload, "Kaigi.EndKaigi", [
        "call_id",
        "ended_at_ms",
        "commitment",
        "nullifier",
        "roster_root",
        "proof",
      ]);
      return {
        Kaigi: {
          EndKaigi: {
            call_id: decodeKaigiIdValue(fields.call_id, "Kaigi.EndKaigi.call_id"),
            ended_at_ms: decodeOptionValue(
              fields.ended_at_ms,
              decodeU64NumberValue,
              "Kaigi.EndKaigi.ended_at_ms",
            ),
            commitment: decodeOptionValue(
              fields.commitment,
              decodeKaigiParticipantCommitmentValue,
              "Kaigi.EndKaigi.commitment",
            ),
            nullifier: decodeOptionValue(
              fields.nullifier,
              decodeKaigiParticipantNullifierValue,
              "Kaigi.EndKaigi.nullifier",
            ),
            roster_root: decodeOptionValue(
              fields.roster_root,
              decodeHashValue,
              "Kaigi.EndKaigi.roster_root",
            ),
            proof: decodeOptionValue(
              fields.proof,
              decodeByteVecAsBase64,
              "Kaigi.EndKaigi.proof",
            ),
          },
        },
      };
    }
    case RECORD_KAIGI_USAGE_WIRE_ID: {
      const fields = decodeStructFields(payload, "Kaigi.RecordKaigiUsage", [
        "call_id",
        "duration_ms",
        "billed_gas",
        "usage_commitment",
        "proof",
      ]);
      return {
        Kaigi: {
          RecordKaigiUsage: {
            call_id: decodeKaigiIdValue(fields.call_id, "Kaigi.RecordKaigiUsage.call_id"),
            duration_ms: decodeU64NumberValue(
              fields.duration_ms,
              "Kaigi.RecordKaigiUsage.duration_ms",
            ),
            billed_gas: decodeU64NumberValue(
              fields.billed_gas,
              "Kaigi.RecordKaigiUsage.billed_gas",
            ),
            usage_commitment: decodeOptionValue(
              fields.usage_commitment,
              decodeHashValue,
              "Kaigi.RecordKaigiUsage.usage_commitment",
            ),
            proof: decodeOptionValue(
              fields.proof,
              decodeByteVecAsBase64,
              "Kaigi.RecordKaigiUsage.proof",
            ),
          },
        },
      };
    }
    case SET_KAIGI_RELAY_MANIFEST_WIRE_ID: {
      const fields = decodeStructFields(payload, "Kaigi.SetKaigiRelayManifest", [
        "call_id",
        "relay_manifest",
      ]);
      return {
        Kaigi: {
          SetKaigiRelayManifest: {
            call_id: decodeKaigiIdValue(
              fields.call_id,
              "Kaigi.SetKaigiRelayManifest.call_id",
            ),
            relay_manifest: decodeOptionValue(
              fields.relay_manifest,
              decodeKaigiRelayManifestValue,
              "Kaigi.SetKaigiRelayManifest.relay_manifest",
            ),
          },
        },
      };
    }
    case REGISTER_KAIGI_RELAY_WIRE_ID: {
      const fields = decodeStructFields(payload, "Kaigi.RegisterKaigiRelay", ["relay"]);
      return {
        Kaigi: {
          RegisterKaigiRelay: {
            relay: decodeKaigiRelayRegistrationValue(
              fields.relay,
              "Kaigi.RegisterKaigiRelay.relay",
            ),
          },
        },
      };
    }
    default:
      throw new Error(`unsupported Kaigi wire id ${wireId}`);
  }
}

function decodeZkInstructionPayload(wireId, payload) {
  switch (wireId) {
    case REGISTER_ZK_ASSET_WIRE_ID: {
      const fields = decodeStructFields(payload, "zk.RegisterZkAsset", [
        "asset",
        "vk_unshield",
        "vk_shield",
      ]);
      return {
        zk: {
          RegisterZkAsset: {
            asset: decodeAssetDefinitionIdValue(fields.asset, "zk.RegisterZkAsset.asset"),
            vk_unshield: decodeOptionValue(
              fields.vk_unshield,
              decodeVerifyingKeyIdValue,
              "zk.RegisterZkAsset.vk_unshield",
            ),
            vk_shield: decodeOptionValue(
              fields.vk_shield,
              decodeVerifyingKeyIdValue,
              "zk.RegisterZkAsset.vk_shield",
            ),
          },
        },
      };
    }
    case SCHEDULE_CONFIDENTIAL_POLICY_TRANSITION_WIRE_ID: {
      const fields = decodeStructFields(payload, "zk.ScheduleConfidentialPolicyTransition", [
        "asset",
        "new_mode",
        "effective_height",
        "transition_id",
        "conversion_window",
      ]);
      return {
        zk: {
          ScheduleConfidentialPolicyTransition: {
            asset: decodeAssetDefinitionIdValue(
              fields.asset,
              "zk.ScheduleConfidentialPolicyTransition.asset",
            ),
            new_mode: decodeConfidentialPolicyModeValue(
              fields.new_mode,
              "zk.ScheduleConfidentialPolicyTransition.new_mode",
            ),
            effective_height: decodeU64NumberValue(
              fields.effective_height,
              SCHEDULE_EFFECTIVE_HEIGHT_CONTEXT,
            ),
            transition_id: decodeHashValue(
              fields.transition_id,
              SCHEDULE_TRANSITION_ID_CONTEXT,
            ),
            conversion_window: decodeOptionValue(
              fields.conversion_window,
              decodeU64NumberValue,
              SCHEDULE_CONVERSION_WINDOW_CONTEXT,
            ),
          },
        },
      };
    }
    case CANCEL_CONFIDENTIAL_POLICY_TRANSITION_WIRE_ID: {
      const fields = decodeStructFields(payload, "zk.CancelConfidentialPolicyTransition", [
        "asset",
        "transition_id",
      ]);
      return {
        zk: {
          CancelConfidentialPolicyTransition: {
            asset: decodeAssetDefinitionIdValue(
              fields.asset,
              "zk.CancelConfidentialPolicyTransition.asset",
            ),
            transition_id: decodeHashValue(
              fields.transition_id,
              CANCEL_TRANSITION_ID_CONTEXT,
            ),
          },
        },
      };
    }
    case CREATE_ELECTION_WIRE_ID: {
      const fields = decodeStructFields(payload, "zk.CreateElection", [
        "election_id",
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
            election_id: decodeStringValue(fields.election_id, "zk.CreateElection.election_id"),
            options: decodeU32Value(fields.options, "zk.CreateElection.options"),
            eligible_root: Array.from(
              decodeFixedBytesValue(fields.eligible_root, 32, "zk.CreateElection.eligible_root"),
            ),
            start_ts: decodeU64NumberValue(fields.start_ts, "zk.CreateElection.start_ts"),
            end_ts: decodeU64NumberValue(fields.end_ts, "zk.CreateElection.end_ts"),
            vk_ballot: decodeVerifyingKeyIdValue(
              fields.vk_ballot,
              "zk.CreateElection.vk_ballot",
            ),
            vk_tally: decodeVerifyingKeyIdValue(
              fields.vk_tally,
              "zk.CreateElection.vk_tally",
            ),
            domain_tag: decodeStringValue(fields.domain_tag, "zk.CreateElection.domain_tag"),
          },
        },
      };
    }
    case SUBMIT_BALLOT_WIRE_ID: {
      const fields = decodeStructFields(payload, "zk.SubmitBallot", [
        "election_id",
        "ciphertext",
        "ballot_proof",
        "nullifier",
      ]);
      return {
        zk: {
          SubmitBallot: {
            election_id: decodeStringValue(fields.election_id, "zk.SubmitBallot.election_id"),
            ciphertext: Array.from(
              decodeByteVecValue(fields.ciphertext, "zk.SubmitBallot.ciphertext"),
            ),
            ballot_proof: decodeProofAttachmentValue(
              fields.ballot_proof,
              "zk.SubmitBallot.ballot_proof",
            ),
            nullifier: Array.from(
              decodeFixedBytesValue(fields.nullifier, 32, "zk.SubmitBallot.nullifier"),
            ),
          },
        },
      };
    }
    case FINALIZE_ELECTION_WIRE_ID: {
      const fields = decodeStructFields(payload, "zk.FinalizeElection", [
        "election_id",
        "tally",
        "tally_proof",
      ]);
      return {
        zk: {
          FinalizeElection: {
            election_id: decodeStringValue(
              fields.election_id,
              "zk.FinalizeElection.election_id",
            ),
            tally: decodeNoritoVec(
              fields.tally,
              (entry, index) =>
                decodeU64NumberValue(entry, `zk.FinalizeElection.tally[${index}]`),
              "zk.FinalizeElection.tally",
            ),
            tally_proof: decodeProofAttachmentValue(
              fields.tally_proof,
              "zk.FinalizeElection.tally_proof",
            ),
          },
        },
      };
    }
    default:
      throw new Error(`unsupported zk wire id ${wireId}`);
  }
}

function decodeRwaInstructionPayload(payload) {
  const reader = new BufferReader(payload, "Rwa");
  const variantIndex = reader.readU32LE("variantIndex");
  const body = readNoritoField(reader, "body");
  reader.assertEof();
  switch (variantIndex) {
    case 0: {
      const fields = decodeStructFields(body, "RegisterRwa", ["rwa"]);
      return { RegisterRwa: { rwa: decodeNewRwaValue(fields.rwa, "RegisterRwa.rwa") } };
    }
    case 1: {
      const fields = decodeStructFields(body, "TransferRwa", [
        "source",
        "rwa",
        "quantity",
        "destination",
      ]);
      return {
        TransferRwa: {
          source: decodeAccountIdValue(fields.source, "TransferRwa.source"),
          rwa: decodeRwaIdValue(fields.rwa, "TransferRwa.rwa"),
          quantity: decodeQuantityValue(fields.quantity, "TransferRwa.quantity"),
          destination: decodeAccountIdValue(fields.destination, "TransferRwa.destination"),
        },
      };
    }
    case 2: {
      const fields = decodeStructFields(body, "MergeRwas", [
        "parents",
        "primary_reference",
        "status",
        "metadata",
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
            "MergeRwas.primary_reference",
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
      const fields = decodeStructFields(body, "ForceTransferRwa", [
        "rwa",
        "quantity",
        "destination",
      ]);
      return {
        ForceTransferRwa: {
          rwa: decodeRwaIdValue(fields.rwa, "ForceTransferRwa.rwa"),
          quantity: decodeQuantityValue(fields.quantity, "ForceTransferRwa.quantity"),
          destination: decodeAccountIdValue(fields.destination, "ForceTransferRwa.destination"),
        },
      };
    }
    case 9: {
      const fields = decodeStructFields(body, "SetRwaControls", ["rwa", "controls"]);
      return {
        SetRwaControls: {
          rwa: decodeRwaIdValue(fields.rwa, "SetRwaControls.rwa"),
          controls: decodeRwaControlPolicyValue(fields.controls, "SetRwaControls.controls"),
        },
      };
    }
    case 10: {
      const fields = decodeStructFields(body, "SetRwaKeyValue", ["rwa", "key", "value"]);
      return {
        SetRwaKeyValue: {
          rwa: decodeRwaIdValue(fields.rwa, "SetRwaKeyValue.rwa"),
          key: decodeNameValue(fields.key, "SetRwaKeyValue.key"),
          value: decodeNestedJsonValue(fields.value, "SetRwaKeyValue.value"),
        },
      };
    }
    case 11: {
      const fields = decodeStructFields(body, "RemoveRwaKeyValue", ["rwa", "key"]);
      return {
        RemoveRwaKeyValue: {
          rwa: decodeRwaIdValue(fields.rwa, "RemoveRwaKeyValue.rwa"),
          key: decodeNameValue(fields.key, "RemoveRwaKeyValue.key"),
        },
      };
    }
    default:
      throw new Error(`Internal Norito decoder does not support RWA variant ${variantIndex}`);
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
  const fields = decodeStructFields(payload, name, ["rwa", "quantity"]);
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
    [encodeDestination(value.destination, `${context}.destination`)],
  ]);
}

function decodeTransferObjectBody(
  payload,
  context,
  decodeSource,
  decodeObject,
  decodeDestination,
) {
  const fields = decodeStructFields(payload, context, ["source", JS_TYPE_OBJECT, "destination"]);
  return {
    source: decodeSource(fields.source, `${context}.source`),
    object: decodeObject(fields.object, `${context}.object`),
    destination: decodeDestination(fields.destination, `${context}.destination`),
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
    throw new Error(`${context} option payload is empty`);
  }
  const tag = payload[0];
  if (tag === 0) {
    if (payload.length !== 1) {
      throw new Error(`${context} None option contained trailing bytes`);
    }
    return null;
  }
  if (tag !== 1) {
    throw new Error(`${context} option tag ${tag} is invalid`);
  }
  const reader = new BufferReader(payload.subarray(1), `${context}.some`);
  const inner = readNoritoField(reader, "value");
  reader.assertEof();
  return decode(inner, `${context}.value`);
}

function encodeBoolValue(value, context) {
  if (typeof value !== "boolean") {
    throw new TypeError(`${context} must be a boolean`);
  }
  return Buffer.of(value ? 1 : 0);
}

function decodeBoolValue(payload, context) {
  if (payload.length !== 1 || (payload[0] !== 0 && payload[0] !== 1)) {
    throw new Error(`${context} must contain a canonical boolean byte`);
  }
  return payload[0] === 1;
}

function encodeFixedBytesValue(value, length, context) {
  const bytes = Buffer.from(normalizeBytes(value));
  if (bytes.length !== length) {
    throw new TypeError(`${context} must contain exactly ${length} bytes`);
  }
  return bytes;
}

function decodeFixedBytesValue(payload, length, context) {
  if (payload.length !== length) {
    throw new Error(`${context} must contain exactly ${length} bytes`);
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

function encodeByteVecValue(value, context) {
  const bytes = Buffer.from(normalizeFlexibleBytes(value, context));
  return Buffer.concat([u64ToLittleEndianBuffer(bytes.length), bytes]);
}

function decodeByteVecValue(payload, context, maxLength = null) {
  const reader = new BufferReader(payload, context);
  const length = bigintToSafeNumber(reader.readU64LE("length"), `${context}.length`);
  if (maxLength !== null && length > maxLength) {
    throw new RangeError(
      `${context} exceeds its ${maxLength}-byte decoding limit`,
    );
  }
  const bytes = reader.readBytes(length, "payload");
  reader.assertEof();
  return Buffer.from(bytes);
}

function decodeByteVecAsBase64(payload, context) {
  return decodeByteVecValue(payload, context).toString(BASE64_ENCODING);
}

function normalizeFlexibleBytes(value, context) {
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
  return bigintToSafeNumber(value, context);
}

function encodeU128Value(value, context) {
  const bigint = normalizeU128Input(value, context);
  const buffer = Buffer.allocUnsafe(16);
  let remaining = bigint;
  for (let index = 0; index < 16; index += 1) {
    buffer[index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  return buffer;
}

function decodeU128StringValue(payload, context) {
  return decodeU128BigInt(payload, context).toString();
}

function decodeU128SafeNumberValue(payload, context) {
  return bigintToSafeNumber(decodeU128BigInt(payload, context), context);
}

function decodeU128BigInt(payload, context) {
  if (payload.length !== 16) {
    throw new Error(`${context} must contain exactly sixteen bytes`);
  }
  let value = 0n;
  for (let index = 15; index >= 0; index -= 1) {
    value = (value << 8n) | BigInt(payload[index]);
  }
  return value;
}

function normalizeU128Input(value, context) {
  let parsed;
  if (typeof value === JS_TYPE_BIGINT) {
    parsed = value;
  } else if (typeof value === JS_TYPE_NUMBER) {
    if (!Number.isSafeInteger(value) || value < 0) {
      throw new TypeError(`${context} must be a non-negative safe integer, bigint, or string`);
    }
    parsed = BigInt(value);
  } else if (typeof value === JS_TYPE_STRING && /^\d+$/.test(value.trim())) {
    parsed = BigInt(value.trim());
  } else {
    throw new TypeError(`${context} must be a non-negative safe integer, bigint, or string`);
  }
  if (parsed < 0n || parsed > UINT128_MASK) {
    throw new RangeError(`${context} must fit in an unsigned 128-bit integer`);
  }
  return parsed;
}

function encodeDomainIdValue(value, context) {
  const literal = assertExactNonEmptyString(value, context);
  if (literal.trim() !== literal) {
    throw new TypeError(`${context} must not contain surrounding whitespace`);
  }
  const segments = literal.split(".");
  if (segments.length !== 2 || segments.some((segment) => segment.length === 0)) {
    throw new TypeError(`${context} must use the exact domain.dataspace form`);
  }
  const [name, dataspace] = segments.map((segment) =>
    canonicalizeDomainLabel(segment),
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
    throw new TypeError(`${context} must not contain whitespace`);
  }
  if (/[@#$]/u.test(literal)) {
    throw new TypeError(`${context} contains a reserved Name character`);
  }
  return encodeNoritoStringValue(literal.normalize("NFC"));
}

function decodeNameValue(payload, context) {
  const literal = decodeStringValue(payload, context);
  if (literal.length === 0 || /\p{White_Space}/u.test(literal)) {
    throw new TypeError(`${context} must be a non-empty Name without whitespace`);
  }
  if (/[@#$]/u.test(literal)) {
    throw new TypeError(`${context} contains a reserved Name character`);
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
    throw new Error(`${context} must use name$domain`);
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
    throw new Error(`${context} must use hash$domain`);
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
    throw new TypeError("Custom must be an object");
  }
  return encodeStructValue([
    [encodeNoritoField(encodeNoritoJsonValue(value.payload ?? null))],
  ]);
}

function decodeCustomInstructionPayload(payload) {
  const fields = decodeStructFields(payload, "Custom", ["payload"]);
  return { payload: decodeNestedJsonValue(fields.payload, "Custom.payload") };
}

function encodeNewDomainValue(value, context) {
  return encodeStructValue([
    [encodeDomainIdValue(value.id, `${context}.id`)],
    [encodeOptionValue(value.logo, encodeSorafsUriValue, `${context}.logo`)],
    [encodeMetadataValue(value.metadata ?? {}, `${context}.metadata`)],
  ]);
}

function decodeNewDomainValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["id", "logo", "metadata"]);
  return {
    id: decodeDomainIdValue(fields.id, `${context}.id`),
    logo: decodeOptionValue(fields.logo, decodeSorafsUriValue, `${context}.logo`),
    metadata: decodeMetadataValue(fields.metadata, `${context}.metadata`),
  };
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

function decodeNewAccountValue(payload, context) {
  const fields = decodeStructFields(
    payload,
    context,
    ["id", "metadata", "label", "uaid", "opaque_ids"],
  );
  return {
    id: decodeAccountIdValue(fields.id, `${context}.id`),
    metadata: decodeMetadataValue(fields.metadata, `${context}.metadata`),
    label: decodeOptionValue(fields.label, decodeStringValue, `${context}.label`),
    uaid: decodeOptionValue(fields.uaid, decodeJsonValue, `${context}.uaid`),
    opaque_ids: decodeNoritoVec(
      fields.opaque_ids,
      (entry, index) => decodeJsonValue(entry, `${context}.opaque_ids[${index}]`),
      `${context}.opaque_ids`,
    ),
  };
}

function encodeNewAssetDefinitionValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  const hasOwningDomain = Object.prototype.hasOwnProperty.call(value, "owning_domain");
  const hasCamelOwningDomain = Object.prototype.hasOwnProperty.call(value, "owningDomain");
  if (!hasOwningDomain && !hasCamelOwningDomain) {
    throw new TypeError(
      `${context}.owning_domain is required; use null for an intentionally unowned global definition`,
    );
  }
  if (
    hasOwningDomain &&
    hasCamelOwningDomain &&
    value.owning_domain !== value.owningDomain
  ) {
    throw new TypeError(`${context} ownership aliases disagree`);
  }
  const owningDomain = hasOwningDomain ? value.owning_domain : value.owningDomain;
  if (owningDomain === undefined) {
    throw new TypeError(`${context}.owning_domain must be a domain identifier or null`);
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
    throw new TypeError(`${context}.balance_scope_policy is required`);
  }
  if (
    hasBalanceScopePolicy &&
    hasCamelBalanceScopePolicy &&
    value.balance_scope_policy !== value.balanceScopePolicy
  ) {
    throw new TypeError(`${context} balance-scope policy aliases disagree`);
  }
  const balanceScopePolicy = hasBalanceScopePolicy
    ? value.balance_scope_policy
    : value.balanceScopePolicy;
  if (balanceScopePolicy === "DataspaceRestricted" && owningDomain === null) {
    throw new TypeError(
      `${context}.owning_domain is required for DataspaceRestricted balances`,
    );
  }
  if (
    Object.prototype.hasOwnProperty.call(value, "confidential_policy") ||
    Object.prototype.hasOwnProperty.call(value, "confidentialPolicy")
  ) {
    throw new TypeError(
      `${context} cannot carry confidential policy; use RegisterZkAsset with canonical verifier bindings`,
    );
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
    [encodeMintableValue(value.mintable ?? "Infinitely", `${context}.mintable`)],
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

function decodeNewAssetDefinitionValue(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "id",
    "name",
    "description",
    "alias",
    "spec",
    "mintable",
    "logo",
    "metadata",
    "balance_scope_policy",
    "owning_domain",
  ]);
  return {
    id: decodeAssetDefinitionIdValue(fields.id, `${context}.id`),
    name: decodeStringValue(fields.name, `${context}.name`),
    description: decodeOptionValue(fields.description, decodeStringValue, `${context}.description`),
    alias: decodeOptionValue(
      fields.alias,
      decodeAssetDefinitionAliasValue,
      `${context}.alias`,
    ),
    spec: decodeNumericSpecValue(fields.spec, `${context}.spec`),
    mintable: decodeMintableValue(fields.mintable, `${context}.mintable`),
    logo: decodeOptionValue(fields.logo, decodeSorafsUriValue, `${context}.logo`),
    metadata: decodeMetadataValue(fields.metadata, `${context}.metadata`),
    balance_scope_policy: decodeAssetBalancePolicyValue(
      fields.balance_scope_policy,
      `${context}.balance_scope_policy`,
    ),
    owning_domain: decodeOptionValue(
      fields.owning_domain,
      decodeDomainIdValue,
      `${context}.owning_domain`,
    ),
  };
}

function encodeMetadataValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
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
    throw new TypeError(
      `${context} must contain exactly 64 lowercase hexadecimal characters`,
    );
  }
  if (/^0{64}$/u.test(value)) {
    throw new TypeError(`${context} must not be the zero identifier`);
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
    throw new TypeError(`${context} must not be the zero identifier`);
  }
  return bytes.toString(HEX_ENCODING);
}

function assertExactObjectKeys(value, expectedKeys, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  assertOnlyObjectKeys(value, expectedKeys, context);
  const missing = expectedKeys.find(
    (key) => !Object.prototype.hasOwnProperty.call(value, key),
  );
  if (missing !== undefined) {
    throw new TypeError(`${context} is missing field ${missing}`);
  }
}

function decodeNonzeroFixedBytesHex(payload, context) {
  const bytes = decodeFixedBytesValue(payload, 32, context);
  if (bytes.every((byte) => byte === 0)) {
    throw new TypeError(`${context} must not be zero`);
  }
  return bytes.toString(HEX_ENCODING);
}

function encodeExactAccountIdValue(value, context) {
  if (typeof value !== JS_TYPE_STRING || value.trim() !== value) {
    throw new TypeError(`${context} must be an exact canonical I105 account id`);
  }
  const canonical = normalizeAccountId(value, context);
  if (canonical !== value) {
    throw new TypeError(`${context} must be an exact canonical I105 account id`);
  }
  return encodeAccountIdValue(canonical, context);
}

function encodeProviderIngestCompletionSignerPolicyValue(value, context) {
  assertExactObjectKeys(
    value,
    ["policy_id", "revision", "predecessor_digest", "policy_digest"],
    context,
  );
  const revision = normalizeU64Input(value.revision, `${context}.revision`);
  if (revision === 0n) {
    throw new TypeError(`${context}.revision must be greater than zero`);
  }
  if (revision === 1n && value.predecessor_digest !== null) {
    throw new TypeError(
      `${context}.predecessor_digest must be null at revision 1`,
    );
  }
  if (revision > 1n && value.predecessor_digest === null) {
    throw new TypeError(
      `${context}.predecessor_digest is required after revision 1`,
    );
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
        `${context}.predecessor_digest`,
      ),
    ],
    [policyDigest],
  ]);
}

function decodeProviderIngestCompletionSignerPolicyValue(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "policy_id",
    "revision",
    "predecessor_digest",
    "policy_digest",
  ]);
  const revision = decodeU64NumberValue(fields.revision, `${context}.revision`);
  if (revision === 0) {
    throw new TypeError(`${context}.revision must be greater than zero`);
  }
  const predecessorDigest = decodeOptionValue(
    fields.predecessor_digest,
    (entry, innerContext) => {
      const bytes = decodeFixedByteArrayArchiveValue(entry, 32, innerContext);
      if (bytes.every((byte) => byte === 0)) {
        throw new TypeError(`${innerContext} must not be zero`);
      }
      return bytes.toString(HEX_ENCODING);
    },
    `${context}.predecessor_digest`,
  );
  if (revision === 1 && predecessorDigest !== null) {
    throw new TypeError(
      `${context}.predecessor_digest must be null at revision 1`,
    );
  }
  if (revision > 1 && predecessorDigest === null) {
    throw new TypeError(
      `${context}.predecessor_digest is required after revision 1`,
    );
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
  return encodeStructValue([
    [
      encodeExactAccountIdValue(
        value.provider_owner,
        `${context}.provider_owner`,
      ),
    ],
    [
      encodeProviderIngestCompletionSignerPolicyValue(
        value.signer_policy,
        `${context}.signer_policy`,
      ),
    ],
  ]);
}

function decodeProviderIngestCompletionAuthorityValue(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "provider_owner",
    "signer_policy",
  ]);
  return {
    provider_owner: decodeAccountIdValue(
      fields.provider_owner,
      `${context}.provider_owner`,
    ),
    signer_policy: decodeProviderIngestCompletionSignerPolicyValue(
      fields.signer_policy,
      `${context}.signer_policy`,
    ),
  };
}

function encodeProviderIngestFinalizedAnchorValue(value, context) {
  assertExactObjectKeys(value, ["height", "block_hash"], context);
  const height = normalizeU64Input(value.height, `${context}.height`);
  if (height === 0n) {
    throw new TypeError(`${context}.height must be greater than zero`);
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
    throw new TypeError(`${context}.height must be greater than zero`);
  }
  return {
    height,
    block_hash: decodeNonzeroFixedBytesHex(
      fields.block_hash,
      `${context}.block_hash`,
    ),
  };
}

function decodeReplicationAssignmentProvider(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "provider_id",
    "slice_gib",
    "lane",
  ]);
  const providerId = decodeFixedBytesValue(
    fields.provider_id,
    32,
    `${context}.provider_id`,
  );
  if (providerId.every((byte) => byte === 0)) {
    throw new TypeError(`${context}.provider_id must not be zero`);
  }
  if (decodeU64Value(fields.slice_gib, `${context}.slice_gib`) === "0") {
    throw new TypeError(`${context}.slice_gib must be greater than zero`);
  }
  decodeOptionValue(
    fields.lane,
    decodeStringValue,
    `${context}.lane`,
  );
  return providerId;
}

/**
 * Validate a canonical Norito `ReplicationOrderV1` archive and its optional
 * instruction-level order identifier binding.
 *
 * @param {ArrayBufferView | ArrayBuffer | Buffer} value
 * @param {string | null} [expectedOrderId]
 * @returns {{orderId: string, targetReplicas: number, providerIds: string[], issuedAt: string, deadlineAt: string}}
 */
export function validateSorafsReplicationOrderPayloadV1(
  value,
  expectedOrderId = null,
) {
  const bytes = Buffer.from(normalizeBytes(value));
  if (
    bytes.length === 0 ||
    bytes.length > SORAFS_REPLICATION_ORDER_MAX_PAYLOAD_BYTES_V1
  ) {
    throw new TypeError(
      `ReplicationOrderV1 payload must contain 1..${SORAFS_REPLICATION_ORDER_MAX_PAYLOAD_BYTES_V1} bytes`,
    );
  }
  const frame = decodeNoritoFrame(
    bytes,
    "ReplicationOrderV1",
    REPLICATION_ORDER_V1_SCHEMA_HASH,
  );
  const canonical = frameNoritoPayload(
    frame.payload,
    REPLICATION_ORDER_V1_SCHEMA_HASH,
    frame.flags,
  );
  if (!canonical.equals(bytes)) {
    throw new TypeError(
      "ReplicationOrderV1 payload must use canonical unpadded Norito framing",
    );
  }

  return withNoritoLengthFlags(frame.flags, () => {
    const fields = decodeStructFields(frame.payload, "ReplicationOrderV1", [
      "version",
      "order_id",
      "manifest_cid",
      "manifest_digest",
      "chunking_profile",
      "target_replicas",
      "assignments",
      "issued_at",
      "deadline_at",
      "sla",
      "metadata",
    ]);
    if (decodeU8Value(fields.version, "ReplicationOrderV1.version") !== 1) {
      throw new TypeError("ReplicationOrderV1.version must be 1");
    }
    const orderIdBytes = decodeFixedBytesValue(
      fields.order_id,
      32,
      "ReplicationOrderV1.order_id",
    );
    if (orderIdBytes.every((byte) => byte === 0)) {
      throw new TypeError("ReplicationOrderV1.order_id must not be zero");
    }
    const orderId = orderIdBytes.toString(HEX_ENCODING);
    if (expectedOrderId !== null) {
      const expected = decodeCanonicalReplicationId(
        expectedOrderId,
        ISSUE_ORDER_ID_CONTEXT,
      );
      if (!expected.equals(orderIdBytes)) {
        throw new TypeError(
          "IssueReplicationOrder.order_id must match ReplicationOrderV1.order_id",
        );
      }
    }

    const targetReplicas = decodeU16Value(
      fields.target_replicas,
      "ReplicationOrderV1.target_replicas",
    );
    if (targetReplicas === 0) {
      throw new TypeError("ReplicationOrderV1.target_replicas must be greater than zero");
    }
    const providers = decodeNoritoVec(
      fields.assignments,
      (entry, index) =>
        decodeReplicationAssignmentProvider(
          entry,
          `ReplicationOrderV1.assignments[${index}]`,
        ),
      "ReplicationOrderV1.assignments",
    );
    if (
      providers.length === 0 ||
      providers.length > 1024 ||
      targetReplicas > providers.length
    ) {
      throw new TypeError(
        "ReplicationOrderV1 assignments must contain 1..1024 entries and cover target_replicas",
      );
    }
    for (let index = 1; index < providers.length; index += 1) {
      if (Buffer.compare(providers[index - 1], providers[index]) >= 0) {
        throw new TypeError(
          "ReplicationOrderV1 assignments must use unique, strictly increasing provider_id values",
        );
      }
    }

    const issuedAt = decodeU64Value(
      fields.issued_at,
      "ReplicationOrderV1.issued_at",
    );
    const deadlineAt = decodeU64Value(
      fields.deadline_at,
      "ReplicationOrderV1.deadline_at",
    );
    if (BigInt(deadlineAt) <= BigInt(issuedAt)) {
      throw new TypeError(
        "ReplicationOrderV1.deadline_at must be greater than issued_at",
      );
    }
    return {
      orderId,
      targetReplicas,
      providerIds: providers.map((provider) => provider.toString(HEX_ENCODING)),
      issuedAt,
      deadlineAt,
    };
  });
}

function encodeReplicationOrderInstruction(instruction) {
  if (isPlainObject(instruction.IssueReplicationOrder)) {
    assertOnlyObjectKeys(instruction, ["IssueReplicationOrder"], "instruction");
    const value = instruction.IssueReplicationOrder;
    assertExactObjectKeys(
      value,
      [
        "order_id",
        "order_payload",
        "issued_epoch",
        "deadline_epoch",
        "musubi_archive",
      ],
      "IssueReplicationOrder",
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
      throw new TypeError(
        ISSUE_ORDER_DEADLINE_MESSAGE,
      );
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
            "IssueReplicationOrder.musubi_archive",
          ),
        ],
      ]),
    );
  }
  if (isPlainObject(instruction.CompleteReplicationOrder)) {
    assertExactObjectKeys(
      instruction,
      ["CompleteReplicationOrder"],
      "instruction",
    );
    const value = instruction.CompleteReplicationOrder;
    assertExactObjectKeys(
      value,
      [
        "order_id",
        "provider_id",
        "completion_epoch",
        "expected_authority",
        "expected_assignment_revision",
        "finalized_anchor",
      ],
      "CompleteReplicationOrder",
    );
    const expectedAssignmentRevision = normalizeU64Input(
      value.expected_assignment_revision,
      COMPLETE_ORDER_REVISION_CONTEXT,
    );
    if (expectedAssignmentRevision === 0n) {
      throw new TypeError(
        COMPLETE_ORDER_REVISION_MESSAGE,
      );
    }
    return encodeInstructionEnvelope(
      COMPLETE_REPLICATION_ORDER_WIRE_ID,
      encodeStructValue([
        [
          encodeReplicationIdValue(
            value.order_id,
            "CompleteReplicationOrder.order_id",
          ),
        ],
        [
          encodeReplicationIdValue(
            value.provider_id,
            "CompleteReplicationOrder.provider_id",
          ),
        ],
        [encodeU64Value(
          value.completion_epoch,
          "CompleteReplicationOrder.completion_epoch",
        )],
        [
          encodeProviderIngestCompletionAuthorityValue(
            value.expected_authority,
            "CompleteReplicationOrder.expected_authority",
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
            "CompleteReplicationOrder.finalized_anchor",
          ),
        ],
      ]),
    );
  }
  if (isPlainObject(instruction.ExpireReplicationOrder)) {
    assertOnlyObjectKeys(instruction, ["ExpireReplicationOrder"], "instruction");
    const value = instruction.ExpireReplicationOrder;
    assertOnlyObjectKeys(
      value,
      ["order_id", "expiration_epoch"],
      "ExpireReplicationOrder",
    );
    return encodeInstructionEnvelope(
      EXPIRE_REPLICATION_ORDER_WIRE_ID,
      encodeStructValue([
        [
          encodeReplicationIdValue(
            value.order_id,
            "ExpireReplicationOrder.order_id",
          ),
        ],
        [encodeU64Value(
          value.expiration_epoch,
          "ExpireReplicationOrder.expiration_epoch",
        )],
      ]),
    );
  }
  throw new TypeError("unsupported SoraFS replication-order instruction");
}

function decodeReplicationOrderInstructionPayload(wireId, payload) {
  if (wireId === ISSUE_REPLICATION_ORDER_WIRE_ID) {
    const fields = decodeStructFields(payload, "IssueReplicationOrder", [
      "order_id",
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
      "IssueReplicationOrder.musubi_archive",
    );
    if (deadlineEpoch <= issuedEpoch) {
      throw new TypeError(
        ISSUE_ORDER_DEADLINE_MESSAGE,
      );
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
    const fields = decodeStructFields(payload, "CompleteReplicationOrder", [
      "order_id",
      "provider_id",
      "completion_epoch",
      "expected_authority",
      "expected_assignment_revision",
      "finalized_anchor",
    ]);
    const expectedAssignmentRevision = decodeU64NumberValue(
      fields.expected_assignment_revision,
      COMPLETE_ORDER_REVISION_CONTEXT,
    );
    if (expectedAssignmentRevision === 0) {
      throw new TypeError(
        COMPLETE_ORDER_REVISION_MESSAGE,
      );
    }
    return {
      CompleteReplicationOrder: {
        order_id: decodeReplicationIdValue(
          fields.order_id,
          "CompleteReplicationOrder.order_id",
        ),
        provider_id: decodeReplicationIdValue(
          fields.provider_id,
          "CompleteReplicationOrder.provider_id",
        ),
        completion_epoch: decodeU64NumberValue(
          fields.completion_epoch,
          "CompleteReplicationOrder.completion_epoch",
        ),
        expected_authority: decodeProviderIngestCompletionAuthorityValue(
          fields.expected_authority,
          "CompleteReplicationOrder.expected_authority",
        ),
        expected_assignment_revision: expectedAssignmentRevision,
        finalized_anchor: decodeProviderIngestFinalizedAnchorValue(
          fields.finalized_anchor,
          "CompleteReplicationOrder.finalized_anchor",
        ),
      },
    };
  }
  const fields = decodeStructFields(payload, "ExpireReplicationOrder", [
    "order_id",
    "expiration_epoch",
  ]);
  return {
    ExpireReplicationOrder: {
      order_id: decodeReplicationIdValue(
        fields.order_id,
        "ExpireReplicationOrder.order_id",
      ),
      expiration_epoch: decodeU64NumberValue(
        fields.expiration_epoch,
        "ExpireReplicationOrder.expiration_epoch",
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
  if (isPlainObject(instruction.PersistCouncilForEpoch)) {
    return encodeInstructionEnvelope(
      PERSIST_COUNCIL_FOR_EPOCH_WIRE_ID,
      encodePersistCouncilForEpochPayload(instruction.PersistCouncilForEpoch),
    );
  }
  throw new Error(
    `Internal Norito canonicalization does not support governance instruction ${describeInstructionShape(instruction)}`,
  );
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
      encodeStructValue([
        [encodeKeyedHashValue(instruction.SendToTwitter.binding_hash, "SendToTwitter.binding_hash")],
        [encodeQuantityValue(instruction.SendToTwitter.amount, "SendToTwitter.amount")],
      ]),
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
  throw new Error(
    `Internal Norito canonicalization does not support social instruction ${describeInstructionShape(instruction)}`,
  );
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
      encodeStructValue([
        [encodeHashValue(
          instruction.RegisterSmartContractBytes.code_hash,
          "RegisterSmartContractBytes.code_hash",
        )],
        [encodeByteVecValue(
          instruction.RegisterSmartContractBytes.code,
          "RegisterSmartContractBytes.code",
        )],
      ]),
    );
  }
  if (isPlainObject(instruction.DeactivateContractInstance)) {
    return encodeInstructionEnvelope(
      DEACTIVATE_CONTRACT_INSTANCE_WIRE_ID,
      encodeStructValue([
        [encodeNoritoStringValue(
          assertNonEmptyString(
            instruction.DeactivateContractInstance.contract_address,
            "DeactivateContractInstance.contract_address",
          ),
        )],
        [encodeOptionValue(
          instruction.DeactivateContractInstance.reason,
          encodeNoritoStringValue,
          "DeactivateContractInstance.reason",
        )],
      ]),
    );
  }
  if (isPlainObject(instruction.ActivateContractInstance)) {
    return encodeInstructionEnvelope(
      ACTIVATE_CONTRACT_INSTANCE_WIRE_ID,
      encodeStructValue([
        [encodeNoritoStringValue(
          assertNonEmptyString(
            instruction.ActivateContractInstance.contract_address,
            "ActivateContractInstance.contract_address",
          ),
        )],
        [encodeHashValue(
          instruction.ActivateContractInstance.code_hash,
          "ActivateContractInstance.code_hash",
        )],
      ]),
    );
  }
  if (isPlainObject(instruction.CommitContractDeployment)) {
    return encodeInstructionEnvelope(
      COMMIT_CONTRACT_DEPLOYMENT_WIRE_ID,
      encodeStructValue([
        [encodeU64Value(
          instruction.CommitContractDeployment.expected_deploy_nonce,
          "CommitContractDeployment.expected_deploy_nonce",
        )],
        [encodeNoritoStringValue(assertNonEmptyString(
          instruction.CommitContractDeployment.contract_address,
          "CommitContractDeployment.contract_address",
        ))],
        [encodeHashValue(
          instruction.CommitContractDeployment.code_hash,
          "CommitContractDeployment.code_hash",
        )],
        [encodeNoritoStringValue(assertNonEmptyString(
          instruction.CommitContractDeployment.contract_alias,
          "CommitContractDeployment.contract_alias",
        ))],
        [encodeOptionValue(
          instruction.CommitContractDeployment.lease_expiry_ms,
          encodeU64Value,
          "CommitContractDeployment.lease_expiry_ms",
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
      encodeStructValue([
        [encodeHashValue(
          instruction.UploadSmartContractCodeChunk.code_hash,
          "UploadSmartContractCodeChunk.code_hash",
        )],
        [encodeU64Value(
          instruction.UploadSmartContractCodeChunk.total_size,
          "UploadSmartContractCodeChunk.total_size",
        )],
        [encodeU32Value(
          instruction.UploadSmartContractCodeChunk.chunk_index,
          "UploadSmartContractCodeChunk.chunk_index",
        )],
        [encodeU32Value(
          instruction.UploadSmartContractCodeChunk.chunk_count,
          "UploadSmartContractCodeChunk.chunk_count",
        )],
        [encodeByteVecValue(
          instruction.UploadSmartContractCodeChunk.chunk,
          "UploadSmartContractCodeChunk.chunk",
        )],
      ]),
    );
  }
  if (isPlainObject(instruction.FinalizeSmartContractCodeUpload)) {
    return encodeInstructionEnvelope(
      FINALIZE_SMART_CONTRACT_CODE_UPLOAD_WIRE_ID,
      encodeStructValue([
        [encodeHashValue(
          instruction.FinalizeSmartContractCodeUpload.code_hash,
          "FinalizeSmartContractCodeUpload.code_hash",
        )],
        [encodeU64Value(
          instruction.FinalizeSmartContractCodeUpload.total_size,
          "FinalizeSmartContractCodeUpload.total_size",
        )],
        [encodeU32Value(
          instruction.FinalizeSmartContractCodeUpload.chunk_count,
          "FinalizeSmartContractCodeUpload.chunk_count",
        )],
      ]),
    );
  }
  if (isPlainObject(instruction.CancelSmartContractCodeUpload)) {
    return encodeInstructionEnvelope(
      CANCEL_SMART_CONTRACT_CODE_UPLOAD_WIRE_ID,
      encodeStructValue([
        [encodeHashValue(
          instruction.CancelSmartContractCodeUpload.code_hash,
          "CancelSmartContractCodeUpload.code_hash",
        )],
      ]),
    );
  }
  if (isPlainObject(instruction.RemoveSmartContractBytes)) {
    return encodeInstructionEnvelope(
      REMOVE_SMART_CONTRACT_BYTES_WIRE_ID,
      encodeStructValue([
        [encodeHashValue(
          instruction.RemoveSmartContractBytes.code_hash,
          "RemoveSmartContractBytes.code_hash",
        )],
        [encodeOptionValue(
          instruction.RemoveSmartContractBytes.reason,
          encodeNoritoStringValue,
          "RemoveSmartContractBytes.reason",
        )],
      ]),
    );
  }
  throw new Error(
    `Internal Norito canonicalization does not support smart-contract instruction ${describeInstructionShape(instruction)}`,
  );
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
    throw new TypeError(`${context} must be exactly 32 bytes of lowercase hexadecimal`);
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
    throw new Error(`${context}.version must be ${GOVERNANCE_HASH32_WIRE_VERSION_V1}`);
  }
  const declaredLength = decodeU16Value(fields.declared_len, `${context}.declared_len`);
  if (declaredLength !== GOVERNANCE_HASH32_LENGTH) {
    throw new Error(`${context}.declared_len must be ${GOVERNANCE_HASH32_LENGTH}`);
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
    [encodeNoritoStringValue(assertNonEmptyString(value.contract_address, "ProposeDeployContract.contract_address"))],
    [encodeGovernanceHash32Value(value.code_hash, "ProposeDeployContract.code_hash")],
    [encodeGovernanceHash32Value(value.abi_hash, "ProposeDeployContract.abi_hash")],
    [encodeGovernanceAbiVersionValue(value.abi_version, "ProposeDeployContract.abi_version")],
    [encodeOptionValue(
      value.manifest_provenance ?? null,
      encodeManifestProvenanceValue,
      "ProposeDeployContract.manifest_provenance",
    )],
  ]);
}

function encodeCastZkBallotPayload(value) {
  validateCastZkBallotPayload(value);
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.election_id, "CastZkBallot.election_id"))],
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

function encodePersistCouncilForEpochPayload(value) {
  return encodeStructValue([
    [encodeU64NumberValue(value.epoch, "PersistCouncilForEpoch.epoch")],
    [encodeNoritoVec(value.members ?? [], (member, index) =>
      encodeAccountIdValue(member, `PersistCouncilForEpoch.members[${index}]`),
    )],
    [encodeNoritoVec(value.alternates ?? [], (member, index) =>
      encodeAccountIdValue(member, `PersistCouncilForEpoch.alternates[${index}]`),
    )],
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
  throw new Error(
    `Internal Norito canonicalization does not support Kaigi instruction ${describeInstructionShape(instruction)}`,
  );
}

function encodeCreateKaigiPayload(value) {
  return encodeStructValue([
    [encodeNewKaigiValue(value.call, "Kaigi.CreateKaigi.call")],
    [encodeOptionValue(value.commitment, encodeKaigiParticipantCommitmentValue, "Kaigi.CreateKaigi.commitment")],
    [encodeOptionValue(value.nullifier, encodeKaigiParticipantNullifierValue, "Kaigi.CreateKaigi.nullifier")],
    [encodeOptionValue(value.roster_root, encodeHashValue, "Kaigi.CreateKaigi.roster_root")],
    [encodeOptionValue(value.proof, encodeByteVecValue, "Kaigi.CreateKaigi.proof")],
  ]);
}

function encodeJoinLeaveKaigiPayload(value, name) {
  return encodeStructValue([
    [encodeKaigiIdValue(value.call_id, `Kaigi.${name}.call_id`)],
    [encodeAccountIdValue(value.participant, `Kaigi.${name}.participant`)],
    [encodeOptionValue(value.commitment, encodeKaigiParticipantCommitmentValue, `Kaigi.${name}.commitment`)],
    [encodeOptionValue(value.nullifier, encodeKaigiParticipantNullifierValue, `Kaigi.${name}.nullifier`)],
    [encodeOptionValue(value.roster_root, encodeHashValue, `Kaigi.${name}.roster_root`)],
    [encodeOptionValue(value.proof, encodeByteVecValue, `Kaigi.${name}.proof`)],
  ]);
}

function encodeEndKaigiPayload(value) {
  return encodeStructValue([
    [encodeKaigiIdValue(value.call_id, "Kaigi.EndKaigi.call_id")],
    [encodeOptionValue(value.ended_at_ms, encodeU64NumberValue, "Kaigi.EndKaigi.ended_at_ms")],
    [encodeOptionValue(value.commitment, encodeKaigiParticipantCommitmentValue, "Kaigi.EndKaigi.commitment")],
    [encodeOptionValue(value.nullifier, encodeKaigiParticipantNullifierValue, "Kaigi.EndKaigi.nullifier")],
    [encodeOptionValue(value.roster_root, encodeHashValue, "Kaigi.EndKaigi.roster_root")],
    [encodeOptionValue(value.proof, encodeByteVecValue, "Kaigi.EndKaigi.proof")],
  ]);
}

function encodeRecordKaigiUsagePayload(value) {
  return encodeStructValue([
    [encodeKaigiIdValue(value.call_id, "Kaigi.RecordKaigiUsage.call_id")],
    [encodeU64NumberValue(value.duration_ms, "Kaigi.RecordKaigiUsage.duration_ms")],
    [encodeU64NumberValue(value.billed_gas, "Kaigi.RecordKaigiUsage.billed_gas")],
    [encodeOptionValue(value.usage_commitment, encodeHashValue, "Kaigi.RecordKaigiUsage.usage_commitment")],
    [encodeOptionValue(value.proof, encodeByteVecValue, "Kaigi.RecordKaigiUsage.proof")],
  ]);
}

function encodeSetKaigiRelayManifestPayload(value) {
  return encodeStructValue([
    [encodeKaigiIdValue(value.call_id, "Kaigi.SetKaigiRelayManifest.call_id")],
    [encodeOptionValue(value.relay_manifest, encodeKaigiRelayManifestValue, "Kaigi.SetKaigiRelayManifest.relay_manifest")],
  ]);
}

function encodeRegisterKaigiRelayPayload(value) {
  return encodeStructValue([
    [encodeKaigiRelayRegistrationValue(value.relay, "Kaigi.RegisterKaigiRelay.relay")],
  ]);
}

function encodeVerifyingKeyInstruction(instruction) {
  const entries = [
    [
      "RegisterVerifyingKey",
      REGISTER_VERIFYING_KEY_WIRE_ID,
      encodeVerifyingKeyInstructionPayload,
    ],
    [
      "UpdateVerifyingKey",
      UPDATE_VERIFYING_KEY_WIRE_ID,
      encodeVerifyingKeyInstructionPayload,
    ],
  ];
  for (const [key, wireId, encode] of entries) {
    if (isPlainObject(instruction[key])) {
      return encodeInstructionEnvelope(
        wireId,
        encode(instruction[key], `verifying_keys.${key}`),
      );
    }
  }
  throw new Error(
    `Internal Norito canonicalization does not support verifying-key instruction ${describeInstructionShape(instruction)}`,
  );
}

function encodeVerifyingKeyInstructionPayload(value, context) {
  return encodeStructValue([
    [encodeVerifyingKeyIdValue(value.id, `${context}.id`)],
    [encodeVerifyingKeyRecordValue(value.record, `${context}.record`)],
  ]);
}

function decodeVerifyingKeyInstructionPayload(wireId, payload) {
  const variant =
    wireId === REGISTER_VERIFYING_KEY_WIRE_ID
      ? "RegisterVerifyingKey"
      : "UpdateVerifyingKey";
  const fields = decodeStructFields(payload, `verifying_keys.${variant}`, [
    "id",
    "record",
  ]);
  return {
    verifying_keys: {
      [variant]: {
        id: decodeVerifyingKeyIdValue(fields.id, `verifying_keys.${variant}.id`),
        record: decodeVerifyingKeyRecordValue(
          fields.record,
          `verifying_keys.${variant}.record`,
        ),
      },
    },
  };
}

function encodeZkInstruction(instruction) {
  const entries = [
    ["RegisterZkAsset", REGISTER_ZK_ASSET_WIRE_ID, encodeRegisterZkAssetPayload],
    ["ScheduleConfidentialPolicyTransition", SCHEDULE_CONFIDENTIAL_POLICY_TRANSITION_WIRE_ID, encodeScheduleConfidentialPolicyTransitionPayload],
    ["CancelConfidentialPolicyTransition", CANCEL_CONFIDENTIAL_POLICY_TRANSITION_WIRE_ID, encodeCancelConfidentialPolicyTransitionPayload],
    ["CreateElection", CREATE_ELECTION_WIRE_ID, encodeCreateElectionPayload],
    ["SubmitBallot", SUBMIT_BALLOT_WIRE_ID, encodeSubmitBallotPayload],
    ["FinalizeElection", FINALIZE_ELECTION_WIRE_ID, encodeFinalizeElectionPayload],
  ];
  for (const [key, wireId, encode] of entries) {
    if (isPlainObject(instruction[key])) {
      return encodeInstructionEnvelope(wireId, encode(instruction[key], `zk.${key}`));
    }
  }
  throw new Error(
    `Internal Norito canonicalization does not support zk instruction ${describeInstructionShape(instruction)}`,
  );
}

function encodeRegisterZkAssetPayload(value) {
  assertExactObjectKeys(
    value,
    ["asset", "vk_unshield", "vk_shield"],
    "zk.RegisterZkAsset",
  );
  return encodeStructValue([
    [encodeAssetDefinitionIdValue(value.asset, "zk.RegisterZkAsset.asset")],
    [encodeOptionValue(value.vk_unshield, encodeVerifyingKeyIdValue, "zk.RegisterZkAsset.vk_unshield")],
    [encodeOptionValue(value.vk_shield, encodeVerifyingKeyIdValue, "zk.RegisterZkAsset.vk_shield")],
  ]);
}

function encodeScheduleConfidentialPolicyTransitionPayload(value) {
  return encodeStructValue([
    [encodeAssetDefinitionIdValue(value.asset, "zk.ScheduleConfidentialPolicyTransition.asset")],
    [encodeConfidentialPolicyModeValue(value.new_mode, "zk.ScheduleConfidentialPolicyTransition.new_mode")],
    [encodeU64NumberValue(value.effective_height, SCHEDULE_EFFECTIVE_HEIGHT_CONTEXT)],
    [encodeHashValue(value.transition_id, SCHEDULE_TRANSITION_ID_CONTEXT)],
    [encodeOptionValue(value.conversion_window, encodeU64NumberValue, SCHEDULE_CONVERSION_WINDOW_CONTEXT)],
  ]);
}

function encodeCancelConfidentialPolicyTransitionPayload(value) {
  return encodeStructValue([
    [encodeAssetDefinitionIdValue(value.asset, "zk.CancelConfidentialPolicyTransition.asset")],
    [encodeHashValue(value.transition_id, CANCEL_TRANSITION_ID_CONTEXT)],
  ]);
}

function encodeCreateElectionPayload(value) {
  return encodeStructValue([
    [encodeNoritoStringValue(assertCanonicalGovernanceSelectorV1(
      value.election_id,
      "zk.CreateElection.election_id",
    ))],
    [encodeU32Value(value.options, "zk.CreateElection.options")],
    [encodeFixedBytesValue(value.eligible_root, 32, "zk.CreateElection.eligible_root")],
    [encodeU64NumberValue(value.start_ts, "zk.CreateElection.start_ts")],
    [encodeU64NumberValue(value.end_ts, "zk.CreateElection.end_ts")],
    [encodeVerifyingKeyIdValue(value.vk_ballot, "zk.CreateElection.vk_ballot")],
    [encodeVerifyingKeyIdValue(value.vk_tally, "zk.CreateElection.vk_tally")],
    [encodeNoritoStringValue(assertNonEmptyString(value.domain_tag, "zk.CreateElection.domain_tag"))],
  ]);
}

function encodeSubmitBallotPayload(value) {
  return encodeStructValue([
    [encodeNoritoStringValue(assertCanonicalGovernanceSelectorV1(
      value.election_id,
      "zk.SubmitBallot.election_id",
    ))],
    [encodeByteVecValue(value.ciphertext, "zk.SubmitBallot.ciphertext")],
    [encodeProofAttachmentValue(value.ballot_proof, "zk.SubmitBallot.ballot_proof")],
    [encodeFixedBytesValue(value.nullifier, 32, "zk.SubmitBallot.nullifier")],
  ]);
}

function encodeFinalizeElectionPayload(value) {
  return encodeStructValue([
    [encodeNoritoStringValue(assertCanonicalGovernanceSelectorV1(
      value.election_id,
      "zk.FinalizeElection.election_id",
    ))],
    [encodeNoritoVec(value.tally ?? [], (entry, index) =>
      encodeU64NumberValue(entry, `zk.FinalizeElection.tally[${index}]`),
    )],
    [encodeProofAttachmentValue(value.tally_proof, "zk.FinalizeElection.tally_proof")],
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
  throw new Error(
    `Internal Norito canonicalization does not support RWA instruction ${describeInstructionShape(instruction)}`,
  );
}

function encodeKaigiIdValue(value, context) {
  const literal = assertExactNonEmptyString(
    typeof value === JS_TYPE_STRING ? value : `${value.domain_id}:${value.call_name}`,
    context,
  );
  const separator = literal.indexOf(":");
  if (separator <= 0 || separator === literal.length - 1) {
    throw new Error(`${context} must use domain:call format`);
  }
  return encodeStructValue([
    [encodeDomainIdValue(literal.slice(0, separator), `${context}.domain_id`)],
    [encodeNameValue(literal.slice(separator + 1), `${context}.call_name`)],
  ]);
}

function decodeKaigiIdValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["domain_id", "call_name"]);
  return {
    domain_id: decodeDomainIdValue(fields.domain_id, `${context}.domain_id`),
    call_name: decodeNameValue(fields.call_name, `${context}.call_name`),
  };
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

function decodeNewKaigiPayload(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "id",
    "host",
    "title",
    "description",
    "max_participants",
    "gas_rate_per_minute",
    "metadata",
    "scheduled_start_ms",
    "billing_account",
    "privacy_mode",
    "room_policy",
    "relay_manifest",
  ]);
  return {
    id: decodeKaigiIdValue(fields.id, `${context}.id`),
    host: decodeAccountIdValue(fields.host, `${context}.host`),
    title: decodeOptionValue(fields.title, decodeStringValue, `${context}.title`),
    description: decodeOptionValue(
      fields.description,
      decodeStringValue,
      `${context}.description`,
    ),
    max_participants: decodeOptionValue(
      fields.max_participants,
      decodeU32Value,
      `${context}.max_participants`,
    ),
    gas_rate_per_minute: decodeU64NumberValue(
      fields.gas_rate_per_minute,
      `${context}.gas_rate_per_minute`,
    ),
    metadata: decodeMetadataValue(fields.metadata, `${context}.metadata`),
    scheduled_start_ms: decodeOptionValue(
      fields.scheduled_start_ms,
      decodeU64NumberValue,
      `${context}.scheduled_start_ms`,
    ),
    billing_account: decodeOptionValue(
      fields.billing_account,
      decodeAccountIdValue,
      `${context}.billing_account`,
    ),
    privacy_mode: decodeKaigiPrivacyModeValue(
      fields.privacy_mode,
      `${context}.privacy_mode`,
    ),
    room_policy: decodeKaigiRoomPolicyValue(fields.room_policy, `${context}.room_policy`),
    relay_manifest: decodeOptionValue(
      fields.relay_manifest,
      decodeKaigiRelayManifestValue,
      `${context}.relay_manifest`,
    ),
  };
}

function encodeKaigiParticipantCommitmentValue(value, context) {
  return encodeStructValue([
    [encodeHashValue(value.commitment, `${context}.commitment`)],
    [encodeOptionValue(value.alias_tag, encodeNoritoStringValue, `${context}.alias_tag`)],
  ]);
}

function decodeKaigiParticipantCommitmentValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["commitment", "alias_tag"]);
  return {
    commitment: decodeHashValue(fields.commitment, `${context}.commitment`),
    alias_tag: decodeOptionValue(fields.alias_tag, decodeStringValue, `${context}.alias_tag`),
  };
}

function encodeKaigiParticipantNullifierValue(value, context) {
  return encodeStructValue([
    [encodeHashValue(value.digest, `${context}.digest`)],
    [encodeU64NumberValue(value.issued_at_ms, `${context}.issued_at_ms`)],
  ]);
}

function decodeKaigiParticipantNullifierValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["digest", "issued_at_ms"]);
  return {
    digest: decodeHashValue(fields.digest, `${context}.digest`),
    issued_at_ms: decodeU64NumberValue(fields.issued_at_ms, `${context}.issued_at_ms`),
  };
}

function encodeKaigiRelayManifestValue(value, context) {
  return encodeStructValue([
    [encodeNoritoVec(value.hops ?? [], (hop, index) =>
      encodeKaigiRelayHopValue(hop, `${context}.hops[${index}]`),
    )],
    [encodeU64NumberValue(value.expiry_ms, `${context}.expiry_ms`)],
  ]);
}

function decodeKaigiRelayManifestValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["hops", "expiry_ms"]);
  return {
    hops: decodeNoritoVec(
      fields.hops,
      (entry, index) => decodeKaigiRelayHopValue(entry, `${context}.hops[${index}]`),
      `${context}.hops`,
    ),
    expiry_ms: decodeU64NumberValue(fields.expiry_ms, `${context}.expiry_ms`),
  };
}

function encodeKaigiRelayHopValue(value, context) {
  return encodeStructValue([
    [encodeAccountIdValue(value.relay_id, `${context}.relay_id`)],
    [encodeByteVecValue(value.hpke_public_key, `${context}.hpke_public_key`)],
    [encodeU8Value(value.weight, `${context}.weight`)],
  ]);
}

function decodeKaigiRelayHopValue(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "relay_id",
    "hpke_public_key",
    "weight",
  ]);
  return {
    relay_id: decodeAccountIdValue(fields.relay_id, `${context}.relay_id`),
    hpke_public_key: decodeByteVecAsBase64(
      fields.hpke_public_key,
      `${context}.hpke_public_key`,
    ),
    weight: decodeU8Value(fields.weight, `${context}.weight`),
  };
}

function encodeKaigiRelayRegistrationValue(value, context) {
  return encodeStructValue([
    [encodeAccountIdValue(value.relay_id, `${context}.relay_id`)],
    [
      encodeNoritoField(
        Buffer.from(normalizeFlexibleBytes(value.hpke_public_key, `${context}.hpke_public_key`)),
      ),
    ],
    [encodeU8Value(value.bandwidth_class, `${context}.bandwidth_class`)],
  ]);
}

function decodeKaigiRelayRegistrationValue(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "relay_id",
    "hpke_public_key",
    "bandwidth_class",
  ]);
  return {
    relay_id: decodeAccountIdValue(fields.relay_id, `${context}.relay_id`),
    hpke_public_key: (() => {
      const reader = new BufferReader(fields.hpke_public_key, `${context}.hpke_public_key.outer`);
      const bytes = readNoritoField(reader, "value");
      reader.assertEof();
      return Buffer.from(bytes).toString(BASE64_ENCODING);
    })(),
    bandwidth_class: decodeU8Value(fields.bandwidth_class, `${context}.bandwidth_class`),
  };
}

function encodeRegisterRwaPayload(value) {
  return encodeStructValue([
    [encodeNewRwaValue(value.rwa, "RegisterRwa.rwa")],
  ]);
}

function encodeTransferRwaPayload(value) {
  return encodeStructValue([
    [encodeAccountIdValue(value.source, "TransferRwa.source")],
    [encodeRwaIdValue(value.rwa, "TransferRwa.rwa")],
    [encodeQuantityValue(value.quantity, "TransferRwa.quantity")],
    [encodeAccountIdValue(value.destination, "TransferRwa.destination")],
  ]);
}

function encodeMergeRwasPayload(value) {
  return encodeStructValue([
    [encodeNoritoVec(value.parents ?? [], (parent, index) =>
      encodeRwaParentRefValue(parent, `MergeRwas.parents[${index}]`),
    )],
    [encodeNoritoStringValue(assertNonEmptyString(value.primary_reference, "MergeRwas.primary_reference"))],
    [encodeOptionValue(value.status, encodeNameValue, "MergeRwas.status")],
    [encodeMetadataValue(value.metadata ?? {}, "MergeRwas.metadata")],
  ]);
}

function encodeRedeemRwaPayload(value) {
  return encodeStructValue([
    [encodeRwaIdValue(value.rwa, "RedeemRwa.rwa")],
    [encodeQuantityValue(value.quantity, "RedeemRwa.quantity")],
  ]);
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
  return encodeStructValue([
    [encodeRwaIdValue(value.rwa, "HoldRwa.rwa")],
    [encodeQuantityValue(value.quantity, "HoldRwa.quantity")],
  ]);
}

function encodeReleaseRwaPayload(value) {
  return encodeStructValue([
    [encodeRwaIdValue(value.rwa, "ReleaseRwa.rwa")],
    [encodeQuantityValue(value.quantity, "ReleaseRwa.quantity")],
  ]);
}

function encodeForceTransferRwaPayload(value) {
  return encodeStructValue([
    [encodeRwaIdValue(value.rwa, "ForceTransferRwa.rwa")],
    [encodeQuantityValue(value.quantity, "ForceTransferRwa.quantity")],
    [encodeAccountIdValue(value.destination, "ForceTransferRwa.destination")],
  ]);
}

function encodeSetRwaControlsPayload(value) {
  return encodeStructValue([
    [encodeRwaIdValue(value.rwa, "SetRwaControls.rwa")],
    [encodeRwaControlPolicyValue(value.controls, "SetRwaControls.controls")],
  ]);
}

function encodeSetRwaKeyValuePayload(value) {
  return encodeStructValue([
    [encodeRwaIdValue(value.rwa, "SetRwaKeyValue.rwa")],
    [encodeNameValue(value.key, "SetRwaKeyValue.key")],
    [encodeNoritoField(encodeNoritoJsonValue(value.value))],
  ]);
}

function encodeRemoveRwaKeyValuePayload(value) {
  return encodeStructValue([
    [encodeRwaIdValue(value.rwa, "RemoveRwaKeyValue.rwa")],
    [encodeNameValue(value.key, "RemoveRwaKeyValue.key")],
  ]);
}

function encodeNewRwaValue(value, context) {
  return encodeStructValue([
    [encodeArchivedDomainIdValue(value.domain, `${context}.domain`)],
    [encodeQuantityValue(value.quantity, `${context}.quantity`)],
    [encodeNumericSpecValue(value.spec ?? { scale: null }, `${context}.spec`)],
    [encodeNoritoStringValue(assertNonEmptyString(value.primary_reference, `${context}.primary_reference`))],
    [encodeOptionValue(value.status, encodeNameValue, `${context}.status`)],
    [encodeMetadataValue(value.metadata ?? {}, `${context}.metadata`)],
    [encodeNoritoVec(value.parents ?? [], (parent, index) =>
      encodeRwaParentRefValue(parent, `${context}.parents[${index}]`),
    )],
    [encodeRwaControlPolicyValue(value.controls ?? {}, `${context}.controls`)],
  ]);
}

function decodeNewRwaValue(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "domain",
    "quantity",
    "spec",
    "primary_reference",
    "status",
    "metadata",
    "parents",
    "controls",
  ]);
  return {
    domain: decodeArchivedDomainIdValue(fields.domain, `${context}.domain`),
    quantity: decodeQuantityValue(fields.quantity, `${context}.quantity`),
    spec: decodeNumericSpecValue(fields.spec, `${context}.spec`),
    primary_reference: decodeStringValue(
      fields.primary_reference,
      `${context}.primary_reference`,
    ),
    status: decodeOptionValue(fields.status, decodeNameValue, `${context}.status`),
    metadata: decodeMetadataValue(fields.metadata, `${context}.metadata`),
    parents: decodeNoritoVec(
      fields.parents,
      (entry, index) => decodeRwaParentRefValue(entry, `${context}.parents[${index}]`),
      `${context}.parents`,
    ),
    controls: decodeRwaControlPolicyValue(fields.controls, `${context}.controls`),
  };
}

function encodeRwaParentRefValue(value, context) {
  return encodeStructValue([
    [encodeRwaIdValue(value.rwa, `${context}.rwa`)],
    [encodeQuantityValue(value.quantity, `${context}.quantity`)],
  ]);
}

function decodeRwaParentRefValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["rwa", "quantity"]);
  return {
    rwa: decodeRwaIdValue(fields.rwa, `${context}.rwa`),
    quantity: decodeQuantityValue(fields.quantity, `${context}.quantity`),
  };
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

function decodeRwaControlPolicyValue(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "controller_accounts",
    "controller_roles",
    "freeze_enabled",
    "hold_enabled",
    "force_transfer_enabled",
    "redeem_enabled",
  ]);
  return {
    controller_accounts: decodeNoritoVec(
      fields.controller_accounts,
      (entry, index) =>
        decodeAccountIdValue(entry, `${context}.controller_accounts[${index}]`),
      `${context}.controller_accounts`,
    ),
    controller_roles: decodeNoritoVec(
      fields.controller_roles,
      (entry, index) => decodeRoleIdValue(entry, `${context}.controller_roles[${index}]`),
      `${context}.controller_roles`,
    ),
    freeze_enabled: decodeBoolValue(fields.freeze_enabled, `${context}.freeze_enabled`),
    hold_enabled: decodeBoolValue(fields.hold_enabled, `${context}.hold_enabled`),
    force_transfer_enabled: decodeBoolValue(
      fields.force_transfer_enabled,
      `${context}.force_transfer_enabled`,
    ),
    redeem_enabled: decodeBoolValue(fields.redeem_enabled, `${context}.redeem_enabled`),
  };
}

function encodeAssetInstructionBody(value, context) {
  return Buffer.concat([
    encodeNoritoField(encodeQuantityValue(value.object, `${context}.object`)),
    encodeNoritoField(encodeAssetIdValue(value.destination, `${context}.destination`)),
  ]);
}

function decodeAssetInstructionBody(payload, context) {
  const reader = new BufferReader(payload, context);
  const object = decodeQuantityValue(readNoritoField(reader, JS_TYPE_OBJECT), `${context}.object`);
  const destination = decodeAssetIdValue(
    readNoritoField(reader, "destination"),
    `${context}.destination`,
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
    readNoritoField(reader, "destination"),
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
          assertNonEmptyString(value.destination, `${context}.destination`),
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
      new BufferReader(readNoritoField(reader, "destination"), `${context}.destination.outer`),
      "value",
    ),
    `${context}.destination`,
  );
  reader.assertEof();
  return { object, destination };
}

function encodeExecuteTriggerPayload(value) {
  if (!isPlainObject(value)) {
    throw new TypeError("ExecuteTrigger must be an object");
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
    throw new Error(`${context} could not resolve account controller information`);
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
      throw new Error(`${context} uses unsupported account controller tag ${controller.tag}`);
  }
}

/** @internal Exact compact-length AccountId value encoding for typed policy codecs. */
export function encodeAccountIdNoritoValue(value, context = "AccountId") {
  return withNoritoCompactLengths(() =>
    Uint8Array.from(encodeAccountIdValue(value, context)),
  );
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
    throw new Error(`${context} uses unsupported account controller variant ${kind}`);
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
    throw new Error(`${context}.publicKey payload is empty`);
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
      throw new Error(`${context}[${index}] must contain exactly one byte`);
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
      throw new Error(`${context} uses unsupported public-key algorithm ${algorithm}`);
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
      throw new Error(`${context} uses unsupported public-key algorithm tag ${tag}`);
  }
}

function encodeMultisigPolicyPayload(policy, context) {
  if (!Array.isArray(policy.members) || policy.members.length === 0) {
    throw new Error(`${context} multisig policy must contain at least one member`);
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
    throw new Error(`${context} must decode to exactly 21 bytes`);
  }
  if (payload[0] !== ASSET_DEFINITION_ADDRESS_VERSION) {
    throw new Error(`${context} version byte ${payload[0]} is not supported`);
  }
  const checksum = payload.subarray(17);
  const expected = assetDefinitionChecksum(payload.subarray(0, 17));
  if (!checksum.equals(expected)) {
    throw new Error(`${context} checksum is invalid`);
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
    throw new Error(`${context} must use dataspace:<id> when present`);
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
  throw new Error(`${context} uses unsupported scope variant ${kind}`);
}

function encodeHashValue(value, context) {
  return encodeHashLiteralBytes(value, context);
}

function decodeHashValue(payload, context) {
  return decodeHashLiteral(payload, context);
}

function encodeEscrowIdValue(value, context) {
  if (typeof value !== JS_TYPE_STRING) {
    throw new TypeError(`${context} must be a canonical checksummed hash literal`);
  }
  const match = HASH_LITERAL_RE.exec(value);
  if (
    match === null ||
    match[1] !== match[1].toUpperCase() ||
    match[2] !== match[2].toUpperCase()
  ) {
    throw new TypeError(
      `${context} must use canonical uppercase hash:<hex>#<checksum> syntax`,
    );
  }
  const bytes = encodeHashValue(value, context);
  if ((bytes[bytes.length - 1] & 1) === 0) {
    throw new TypeError(`${context} must use a native hash with its marker bit set`);
  }
  return bytes;
}

function decodeEscrowIdValue(payload, context) {
  if (payload.length !== 32 || (payload[payload.length - 1] & 1) === 0) {
    throw new TypeError(`${context} must use a native hash with its marker bit set`);
  }
  return decodeHashValue(payload, context);
}

function encodeStringValue(value, context) {
  if (typeof value !== JS_TYPE_STRING) {
    throw new TypeError(`${context} must be a string`);
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
        throw new Error(`${context} has invalid checksum; expected ${expected}`);
      }
      bytes = Buffer.from(upper, HEX_ENCODING);
    } else if (/^[0-9A-Fa-f]{64}$/.test(literal)) {
      bytes = Buffer.from(literal, HEX_ENCODING);
    } else {
      throw new Error(`${context} must be a 32-byte hash literal or hex string`);
    }
  }
  if ((bytes[bytes.length - 1] & 1) === 0) {
    throw new TypeError(`${context} must use a native hash with its marker bit set`);
  }
  return bytes;
}

function decodeHashLiteral(payload, context) {
  const bytes = decodeFixedBytesValue(payload, 32, context);
  if ((bytes[bytes.length - 1] & 1) === 0) {
    throw new TypeError(`${context} must use a native hash with its marker bit set`);
  }
  const body = bytes.toString(HEX_ENCODING).toUpperCase();
  return `hash:${body}#${computeHashLiteralCrc("hash", body)}`;
}

function encodeKeyedHashValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.pepper_id, `${context}.pepper_id`))],
    [encodeHashValue(value.digest, `${context}.digest`)],
  ]);
}

function decodeKeyedHashValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["pepper_id", "digest"]);
  return {
    pepper_id: decodeStringValue(fields.pepper_id, `${context}.pepper_id`),
    digest: decodeHashValue(fields.digest, `${context}.digest`),
  };
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
    case "Infinitely":
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
      throw new Error(`${context} uses unsupported mintability ${normalized.kind}`);
  }
}

function decodeMintableValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("tag");
  if (tag === 0 || tag === 1 || tag === 2) {
    reader.assertEof();
    return ["Infinitely", "Once", "Not"][tag];
  }
  if (tag !== 3) {
    throw new Error(`${context} uses unsupported mintability ${tag}`);
  }
  const body = readNoritoField(reader, "tokens");
  reader.assertEof();
  const fields = decodeStructFields(body, `${context}.tokens`, ["value"]);
  const tokens = decodeU32Value(fields.value, `${context}.tokens.value`);
  if (tokens === 0) {
    throw new Error(`${context}.tokens must be non-zero`);
  }
  return `Limited(${tokens})`;
}

function parseMintableLabel(value, context) {
  const label = assertNonEmptyString(value, context);
  if (label === "Infinitely" || label === "Once" || label === "Not") {
    return { kind: label };
  }
  const match = /^Limited\((\d+)\)$/.exec(label);
  if (match) {
    return { kind: "Limited", tokens: parseMintabilityTokens(match[1], `${context}.tokens`) };
  }
  throw new Error(`${context} must be Infinitely, Once, Not, or Limited(n)`);
}

function parseMintabilityTokens(value, context) {
  if (typeof value !== JS_TYPE_STRING || !/^\d+$/.test(value)) {
    throw new TypeError(`${context} must be a positive unsigned 32-bit integer`);
  }
  const normalized = Number(value);
  if (!Number.isInteger(normalized) || normalized <= 0 || normalized > 0xffff_ffff) {
    throw new TypeError(`${context} must be a positive unsigned 32-bit integer`);
  }
  return normalized;
}

function encodeAssetBalancePolicyValue(value, context) {
  const normalized = assertNonEmptyString(value, context);
  if (normalized === "Global") {
    return encodeEnumTagValue(0);
  }
  if (normalized === "DataspaceRestricted") {
    return encodeEnumTagValue(1);
  }
  throw new Error(`${context} must be Global or DataspaceRestricted`);
}

function decodeAssetBalancePolicyValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("tag");
  reader.assertEof();
  switch (tag) {
    case 0:
      return "Global";
    case 1:
      return "DataspaceRestricted";
    default:
      throw new Error(`${context} uses unsupported balance policy ${tag}`);
  }
}

function encodeAssetDefinitionAliasValue(value, context) {
  const literal = assertNonEmptyString(value, context);
  if (!literal.includes("#")) {
    throw new Error(`${context} must use <name>#<dataspace> or <name>#<domain>.<dataspace>`);
  }
  return encodeStructValue([[encodeNoritoStringValue(literal)]]);
}

function decodeAssetDefinitionAliasValue(payload, context) {
  return decodeNestedValue(payload, decodeStringValue, context);
}

function encodeSorafsUriValue(value, context) {
  if (typeof value !== JS_TYPE_STRING) {
    throw new TypeError(`${context} must be a string`);
  }
  if (value.trim() !== value || value.includes("\u0000") || /[\u0001-\u001f\u007f]/u.test(value)) {
    throw new Error(`${context} must not contain whitespace padding or control characters`);
  }
  if (!value.startsWith("sorafs://") || value.length === "sorafs://".length) {
    throw new Error(`${context} must use a non-empty sorafs:// URI`);
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
  throw new Error(`${context} must be Transparent or ZkRosterV1`);
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
      throw new Error(`${context} uses unsupported privacy mode ${tag}`);
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
  throw new Error(`${context} must be Public or Authenticated`);
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
      throw new Error(`${context} uses unsupported room policy ${tag}`);
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
  throw new Error(`${context} must be TransparentOnly, ShieldedOnly, or Convertible`);
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
      throw new Error(`${context} uses unsupported confidential policy mode ${tag}`);
  }
}

function encodeVerifyingKeyIdValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.backend, `${context}.backend`))],
    [encodeNoritoStringValue(assertNonEmptyString(value.name, `${context}.name`))],
  ]);
}

function decodeVerifyingKeyIdValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["backend", "name"]);
  return {
    backend: decodeStringValue(fields.backend, `${context}.backend`),
    name: decodeStringValue(fields.name, `${context}.name`),
  };
}

function encodeBackendBytesBoxValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.backend, `${context}.backend`))],
    [encodeByteVecValue(value.bytes, `${context}.bytes`)],
  ]);
}

function decodeProofBoxValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["backend", "bytes"]);
  const backend = decodeStringValue(fields.backend, `${context}.backend`);
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
    backend: decodeStringValue(fields.backend, `${context}.backend`),
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
      throw new Error(`${context} uses unknown or non-canonical backend label ${backend}`);
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
      throw new Error(`${context} uses unsupported backend tag ${tag}`);
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
      throw new Error(`${context} must be Proposed, Active, or Withdrawn`);
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
      throw new Error(`${context} uses unsupported confidential status ${tag}`);
  }
}

function encodeVerifyingKeyRecordValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  return encodeStructValue([
    [encodeU32Value(value.version, `${context}.version`)],
    [encodeNoritoStringValue(assertNonEmptyString(value.circuit_id, `${context}.circuit_id`))],
    [encodeOptionValue(value.owner_manifest_id, encodeNoritoStringValue, `${context}.owner_manifest_id`)],
    [encodeNoritoStringValue(assertNonEmptyString(value.namespace, `${context}.namespace`))],
    [encodeBackendTagValue(value.backend, `${context}.backend`)],
    [encodeNoritoStringValue(assertNonEmptyString(value.curve, `${context}.curve`))],
    [encodeFixedBytesValue(value.public_inputs_schema_hash, 32, `${context}.public_inputs_schema_hash`)],
    [encodeFixedBytesValue(value.commitment, 32, `${context}.commitment`)],
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
    "public_inputs_schema_hash",
    "commitment",
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
    backend: decodeBackendTagValue(fields.backend, `${context}.backend`),
    curve: decodeStringValue(fields.curve, `${context}.curve`),
    public_inputs_schema_hash: Array.from(
      decodeFixedBytesValue(
        fields.public_inputs_schema_hash,
        32,
        `${context}.public_inputs_schema_hash`,
      ),
    ),
    commitment: Array.from(
      decodeFixedBytesValue(fields.commitment, 32, `${context}.commitment`),
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
    throw new TypeError(`${context} must be an object`);
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
    throw new TypeError(
      `${context}.circuit_id must not contain surrounding whitespace`,
    );
  }
  return encodeStructValue([
    [encodeBackendTagValue(value.backend, `${context}.backend`)],
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
      backend: decodeBackendTagValue(fields.backend, `${context}.backend`),
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
          `${context}.vk_commitment`,
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
  const backend = decodeStringValue(readNoritoField(reader, "backend"), `${context}.backend`);
  const proof = decodeProofBoxValue(readNoritoField(reader, "proof"), `${context}.proof`);
  const vk_ref = decodeVerifyingKeyIdValue(readNoritoField(reader, "vk_ref"), `${context}.vk_ref`);
  const vk_commitment =
    reader.offset < reader.buffer.length
      ? decodeOptionValue(
          readNoritoField(reader, "vk_commitment"),
          (entry, innerContext) =>
            Array.from(decodeFixedByteArrayArchiveValue(entry, 32, innerContext)),
          `${context}.vk_commitment`,
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
    throw new TypeError(`${context} must be an object`);
  }
  assertOnlyObjectKeys(
    value,
    ["backend", "proof", "vk_ref", "vk_commitment", "envelope_hash", "lane_privacy"],
    context,
  );
  for (const field of ["backend", "proof", "vk_ref"]) {
    if (!Object.prototype.hasOwnProperty.call(value, field)) {
      throw new TypeError(`${context}.${field} is required`);
    }
  }
  const backend = assertPortableProofIdField(value.backend, `${context}.backend`);
  const proof = normalizeCanonicalProofBoxValue(value.proof, backend, `${context}.proof`);
  const vkRef = normalizeCanonicalProofVerifyingKeyId(
    value.vk_ref,
    `${context}.vk_ref`,
  );
  if (vkRef.backend !== backend) {
    throw new TypeError(`${context}.vk_ref.backend must match ${context}.backend`);
  }

  const normalized = { backend, proof, vk_ref: vkRef };
  if (value.vk_commitment !== undefined && value.vk_commitment !== null) {
    normalized.vk_commitment = normalizeNonZeroProofDigest(
      value.vk_commitment,
      `${context}.vk_commitment`,
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
      throw new TypeError(`${context}.envelope_hash must match proof bytes`);
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
    throw new TypeError(`${context} must be an object`);
  }
  assertExactObjectKeys(value, ["backend", "bytes"], context);
  const proofBackend = assertPortableProofIdField(value.backend, `${context}.backend`);
  if (proofBackend !== backend) {
    throw new TypeError(`${context}.backend must match the attachment backend`);
  }
  if (typeof value.bytes === JS_TYPE_STRING) {
    throw new TypeError(`${context}.bytes must be an exact non-empty byte sequence`);
  }
  const declaredLength = binaryByteLength(value.bytes);
  if (declaredLength !== null && declaredLength > proofBoxMaxProofBytes(backend)) {
    throw new RangeError(
      `${context} exceeds the complete ${PROOF_BOX_MAX_ENCODED_BYTES}-byte ProofBox limit`,
    );
  }
  const bytes = Array.from(normalizeBytes(value.bytes));
  if (bytes.length === 0) {
    throw new TypeError(`${context}.bytes must not be empty`);
  }
  if (!proofBoxFitsEncodedBudget(backend, bytes.length)) {
    throw new RangeError(
      `${context} exceeds the complete ${PROOF_BOX_MAX_ENCODED_BYTES}-byte ProofBox limit`,
    );
  }
  return { backend: proofBackend, bytes };
}

function normalizeCanonicalProofVerifyingKeyId(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  assertExactObjectKeys(value, ["backend", "name"], context);
  return {
    backend: assertPortableProofIdField(value.backend, `${context}.backend`),
    name: assertPortableProofIdField(value.name, `${context}.name`),
  };
}

function assertPortableProofIdField(value, context) {
  if (!isPortableVerifyingKeyIdField(value)) {
    throw new TypeError(`${context} must use portable verifier-key registry syntax`);
  }
  return value;
}

function normalizeNonZeroProofDigest(value, context) {
  const bytes = Array.from(encodeFixedBytesValue(value, 32, context));
  if (bytes.every((byte) => byte === 0)) {
    throw new TypeError(`${context} must be non-zero`);
  }
  return bytes;
}

function normalizeCanonicalLanePrivacyProofValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  assertExactObjectKeys(value, ["commitment_id", "witness"], context);
  if (
    !Number.isInteger(value.commitment_id) ||
    value.commitment_id < 0 ||
    value.commitment_id > 0xffff
  ) {
    throw new RangeError(`${context}.commitment_id must fit within a u16`);
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
    throw new TypeError(`${context} must be an object`);
  }
  assertExactObjectKeys(value, ["kind", "payload"], context);
  if (value.kind !== "merkle") {
    throw new TypeError(`${context}.kind must be exactly merkle`);
  }
  if (!isPlainObject(value.payload)) {
    throw new TypeError(`${context}.payload must be an object`);
  }
  assertExactObjectKeys(value.payload, ["leaf", "proof"], `${context}.payload`);
  const leaf = Array.from(
    encodeFixedBytesValue(value.payload.leaf, 32, `${context}.payload.leaf`),
  );
  if (!isPlainObject(value.payload.proof)) {
    throw new TypeError(`${context}.payload.proof must be an object`);
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
    throw new RangeError(
      `${context}.payload.proof.audit_path must contain 1..=${LANE_PRIVACY_MERKLE_MAX_DEPTH} siblings`,
    );
  }
  if (!laneMerkleLeafIndexFitsDepth(leafIndex, auditPath.length)) {
    throw new RangeError(
      `${context}.payload.proof.leaf_index is impossible for the Merkle path depth`,
    );
  }
  const canonicalPath = auditPath.map((entry, index) => {
    if (entry === null || entry === undefined) {
      throw new TypeError(
        `${context}.payload.proof.audit_path[${index}] must contain a sibling`,
      );
    }
    const siblingContext = `${context}.payload.proof.audit_path[${index}]`;
    const siblingBytes = encodeHashLiteralBytes(entry, siblingContext);
    if (typeof entry === JS_TYPE_STRING) {
      const canonical = decodeHashLiteral(siblingBytes, siblingContext);
      if (entry !== canonical) {
        throw new TypeError(`${siblingContext} must be a canonical HashOf literal`);
      }
      return canonical;
    }
    const sibling = Array.from(siblingBytes);
    if ((sibling[31] & 1) === 0) {
      throw new TypeError(
        `${siblingContext} is not a canonical prehashed HashOf`,
      );
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
  return encodeStructValue([
    [encodeU16Value(value.commitment_id, `${context}.commitment_id`)],
    [encodeLanePrivacyWitnessValue(value.witness, `${context}.witness`)],
  ]);
}

function decodeLanePrivacyProofValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["commitment_id", "witness"]);
  return {
    commitment_id: decodeU16Value(fields.commitment_id, `${context}.commitment_id`),
    witness: decodeLanePrivacyWitnessValue(fields.witness, `${context}.witness`),
  };
}

function encodeLanePrivacyWitnessValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  const kind = assertNonEmptyString(value.kind, `${context}.kind`).toLowerCase();
  if (kind === "merkle") {
    const auditPath =
      value.payload?.proof?.audit_path ?? value.payload?.proof?.auditPath;
    if (!Array.isArray(auditPath) || auditPath.length === 0) {
      throw new Error(
        `${context}.payload.proof.audit_path must contain at least one sibling`,
      );
    }
    if (auditPath.some((entry) => entry === null || entry === undefined)) {
      throw new Error(`${context}.payload.proof.audit_path must not omit siblings`);
    }
    return encodeEnumTagValue(0, () =>
      encodeStructValue([
        [encodeFixedBytesValue(value.payload.leaf, 32, `${context}.payload.leaf`)],
        [encodeMerkleProofValue(value.payload.proof, `${context}.payload.proof`)],
      ]),
    );
  }
  throw new Error(`${context}.kind must be merkle`);
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
        throw new Error(
          `${context}.payload.proof.audit_path must contain at least one sibling`,
        );
      }
      if (proof.audit_path.some((entry) => entry === null)) {
        throw new Error(`${context}.payload.proof.audit_path must not omit siblings`);
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
      throw new Error(`${context} uses unsupported lane privacy witness ${tag}`);
  }
}

const [
  encodeMerkleProofValue,
  decodeMerkleProofValue,
  encodeConfidentialEncryptedPayloadValue,
  decodeConfidentialEncryptedPayloadValue,
] = /* @__PURE__ */ createNoritoProofValueCodecs(
  BufferReader, LANE_PRIVACY_MERKLE_MAX_DEPTH, decodeHashValue,
  decodeNoritoVec, decodeOptionValue, decodeTupleFields,
  decodeU32Value, decodeUnsignedLeb128, encodeCompactLength,
  encodeFixedBytesValue, encodeHashLiteralBytes, encodeNoritoVec,
  encodeOptionValue, encodeTupleValue, encodeU32Value,
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
function encodeEventFilterBoxFramePayload(value, context) {
  const frameBytes = decodeExactStandardBase64(value, context);
  const frame = decodeNoritoFrame(frameBytes, context, EVENT_FILTER_BOX_SCHEMA_HASH);
  const expectedFlags = noritoLengthFlags & COMPACT_LEN_FLAG;
  if (frame.flags !== expectedFlags) {
    throw new Error(
      `${context} uses Norito layout flags ${frame.flags}; expected ${expectedFlags}`,
    );
  }
  const canonical = frameNoritoPayload(
    frame.payload,
    EVENT_FILTER_BOX_SCHEMA_HASH,
    frame.flags,
  );
  if (!canonical.equals(frameBytes)) {
    throw new Error(`${context} must be a canonical unpadded EventFilterBox frame`);
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
    throw new TypeError(`${context} must be exact standard-base64`);
  }
  const bytes = Buffer.from(value, BASE64_ENCODING);
  if (bytes.length === 0 || bytes.toString(BASE64_ENCODING) !== value) {
    throw new TypeError(`${context} must be exact standard-base64`);
  }
  return bytes;
}

function assertOnlyObjectKeys(value, allowedKeys, context) {
  const allowed = new Set(allowedKeys);
  const unknown = Object.keys(value).find((key) => !allowed.has(key));
  if (unknown !== undefined) {
    throw new TypeError(`${context} contains unknown field ${unknown}`);
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
    throw new RangeError(`${context}.mantissa exceeds the signed 512-bit bound`);
  }
  const bytes = mantissaReader.readBytes(byteLength, "bytes");
  mantissaReader.assertEof();
  if (bytes.length === 1 && bytes[0] === 0) {
    throw new TypeError(`${context}.mantissa uses a noncanonical zero encoding`);
  }
  if (bytes.length > 1) {
    const last = bytes[bytes.length - 1];
    const previous = bytes[bytes.length - 2];
    if ((last === 0 && (previous & 0x80) === 0)
      || (last === 0xff && (previous & 0x80) !== 0)) {
      throw new TypeError(`${context}.mantissa has redundant sign extension`);
    }
  }

  const scaleReader = new BufferReader(scalePayload, `${context}.scale`);
  const scale = scaleReader.readU32LE("value");
  scaleReader.assertEof();
  if (scale > NumericV1.MAX_SCALE) {
    throw new RangeError(`${context}.scale exceeds ${NumericV1.MAX_SCALE}`);
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
    throw new TypeError(`${context} must be an unsigned 8-bit integer`);
  }
  return Buffer.of(normalized);
}

function decodeU8Value(payload, context) {
  if (payload.length !== 1) {
    throw new Error(`${context} must contain exactly one byte`);
  }
  return payload[0];
}

function encodeU16Value(value, context) {
  const normalized = Number(value);
  if (!Number.isInteger(normalized) || normalized < 0 || normalized > 0xffff) {
    throw new TypeError(`${context} must be an unsigned 16-bit integer`);
  }
  return u16ToLittleEndianBuffer(normalized);
}

function decodeU16Value(payload, context) {
  if (payload.length !== 2) {
    throw new Error(`${context} must contain exactly two bytes`);
  }
  return payload.readUInt16LE(0);
}

function encodeU32Value(value, context) {
  const normalized = Number(value);
  if (!Number.isInteger(normalized) || normalized < 0 || normalized > 0xffff_ffff) {
    throw new TypeError(`${context} must be an unsigned 32-bit integer`);
  }
  return u32ToLittleEndianBuffer(normalized);
}

function decodeU32Value(payload, context) {
  if (payload.length !== 4) {
    throw new Error(`${context} must contain exactly four bytes`);
  }
  return payload.readUInt32LE(0);
}

function encodeU64Value(value, context) {
  const bigint = normalizeU64Input(value, context);
  return u64ToLittleEndianBuffer(bigint);
}

function decodeU64Value(payload, context) {
  if (payload.length !== 8) {
    throw new Error(`${context} must contain exactly eight bytes`);
  }
  return payload.readBigUInt64LE(0).toString();
}

function encodeNoritoStringValue(value) {
  return encodeNoritoField(Buffer.from(value, UTF8_ENCODING));
}

function encodeExactBase64StringValue(value, context) {
  if (typeof value !== JS_TYPE_STRING) {
    throw new TypeError(`${context} must be a string`);
  }
  if (value.length === 0 || value.trim() !== value || /\s/u.test(value)) {
    throw new TypeError(`${context} must be exact standard-base64`);
  }
  if (!/^[A-Za-z0-9+/]*={0,2}$/u.test(value) || value.length % 4 !== 0) {
    throw new TypeError(`${context} must be exact standard-base64`);
  }
  const decoded = Buffer.from(value, BASE64_ENCODING);
  if (decoded.length === 0 || decoded.toString(BASE64_ENCODING) !== value) {
    throw new TypeError(`${context} must be exact standard-base64`);
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
    throw new RangeError(`${context} exceeds the ${maxCount}-item limit`);
  }
  const values = [];
  for (let index = 0; index < count; index += 1) {
    const itemPayload = readNoritoField(reader, `item${index}`);
    values.push(decode(itemPayload, index));
  }
  reader.assertEof();
  return values;
}

function looksLikeNoritoFrame(buffer) {
  return buffer.length >= 40 && buffer.subarray(0, 4).toString("ascii") === "NRT0";
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
    throw new Error(
      `${context} is shorter than the ${NORITO_FRAME_HEADER_LENGTH}-byte Norito header`,
    );
  }
  if (buffer.subarray(0, 4).toString("ascii") !== "NRT0") {
    throw new Error(`${context} is not an NRT0 frame`);
  }
  const major = buffer[4];
  const minor = buffer[5];
  if (major !== 0 || minor !== 0) {
    throw new Error(`${context} uses unsupported NRT0 version ${major}.${minor}`);
  }

  const schemaHash = buffer.subarray(6, 22);
  if (schemaHash.every((byte) => byte === 0)) {
    throw new Error(`${context} uses the reserved all-zero schema hash`);
  }
  let expectedSchemaHash = null;
  if (options.expectedSchemaHash !== undefined) {
    expectedSchemaHash = toBuffer(options.expectedSchemaHash);
    if (expectedSchemaHash.length !== 16) {
      throw new TypeError(`${context} expected schema hash must contain exactly 16 bytes`);
    }
  }
  if (options.expectedTypeName !== undefined) {
    if (
      typeof options.expectedTypeName !== JS_TYPE_STRING ||
      options.expectedTypeName.length === 0
    ) {
      throw new TypeError(`${context} expected Rust type name must be non-empty`);
    }
    const fromTypeName = /* @__PURE__ */ schemaHashForTypeName(options.expectedTypeName);
    if (expectedSchemaHash !== null && !expectedSchemaHash.equals(fromTypeName)) {
      throw new TypeError(`${context} expected schema constraints contradict each other`);
    }
    expectedSchemaHash = fromTypeName;
  }
  if (expectedSchemaHash !== null && !schemaHash.equals(expectedSchemaHash)) {
    throw new Error(`${context} schema hash did not match the expected type`);
  }

  const compression = buffer[22];
  if (compression !== 0) {
    throw new Error(`${context} must use uncompressed Norito payload encoding`);
  }
  const payloadLength = bigintToSafeNumber(
    buffer.readBigUInt64LE(23),
    `${context}.payloadLength`,
  );
  if (options.requireNonEmptyPayload === true && payloadLength === 0) {
    throw new Error(`${context} must contain a non-empty Norito payload`);
  }
  const expectedCrc = buffer.readBigUInt64LE(31);
  const flags = buffer[39];
  if ((flags & ~NORITO_SUPPORTED_HEADER_FLAGS) !== 0) {
    throw new Error(`${context} uses unsupported Norito header flags 0x${flags.toString(16)}`);
  }
  if (
    (flags & NORITO_FIELD_BITSET_FLAG) !== 0 &&
    (flags & (NORITO_PACKED_STRUCT_FLAG | COMPACT_LEN_FLAG)) !==
      (NORITO_PACKED_STRUCT_FLAG | COMPACT_LEN_FLAG)
  ) {
    throw new Error(`${context} uses an invalid Norito header flag combination`);
  }

  const paddingLength = buffer.length - NORITO_FRAME_HEADER_LENGTH - payloadLength;
  if (paddingLength < 0) {
    throw new Error(`${context} payload length exceeds the available frame bytes`);
  }
  if (paddingLength > NORITO_MAX_HEADER_PADDING) {
    throw new Error(
      `${context} exceeds the ${NORITO_MAX_HEADER_PADDING}-byte Norito header-padding bound`,
    );
  }
  if (options.expectedPaddingLength !== undefined) {
    if (
      !Number.isInteger(options.expectedPaddingLength) ||
      options.expectedPaddingLength < 0 ||
      options.expectedPaddingLength > NORITO_MAX_HEADER_PADDING
    ) {
      throw new TypeError(
        `${context} expected padding length must be an integer from 0 through ${NORITO_MAX_HEADER_PADDING}`,
      );
    }
    if (paddingLength !== options.expectedPaddingLength) {
      throw new Error(
        `${context} must contain exactly ${options.expectedPaddingLength} bytes of header padding`,
      );
    }
  }
  const payloadStart = NORITO_FRAME_HEADER_LENGTH + paddingLength;
  const padding = buffer.subarray(NORITO_FRAME_HEADER_LENGTH, payloadStart);
  if (padding.some((byte) => byte !== 0)) {
    throw new Error(`${context} contains non-zero alignment padding or trailing bytes`);
  }
  const payload = buffer.subarray(payloadStart, payloadStart + payloadLength);
  if (payload.length !== payloadLength || payloadStart + payload.length !== buffer.length) {
    throw new Error(`${context} contains trailing bytes outside the declared payload`);
  }
  const actualCrc = crc64Xz(payload);
  if (actualCrc !== expectedCrc) {
    throw new Error(`${context} CRC64 mismatch`);
  }
  return { payload, schemaHash, flags };
}

function decodeNoritoFrame(buffer, context, expectedSchemaHash) {
  if (buffer.length < NORITO_FRAME_HEADER_LENGTH) {
    // Preserve the established decoder diagnostic while the exported preflight
    // helper reports the more specific SCCP-facing short-header error.
    throw new Error(`${context} reader overran payload while reading Norito header`);
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
      throw new RangeError(`${context} must fit in an unsigned 64-bit integer`);
    }
    return value;
  }
  if (typeof value === JS_TYPE_NUMBER) {
    if (!Number.isInteger(value) || value < 0 || !Number.isSafeInteger(value)) {
      throw new TypeError(`${context} must be a non-negative safe integer or bigint`);
    }
    return BigInt(value);
  }
  if (typeof value === JS_TYPE_STRING && /^\d+$/.test(value.trim())) {
    const parsed = BigInt(value.trim());
    if (parsed > UINT64_MASK) {
      throw new RangeError(`${context} must fit in an unsigned 64-bit integer`);
    }
    return parsed;
  }
  throw new TypeError(`${context} must be a bigint, integer number, or decimal string`);
}

function bigintToSafeNumber(value, context) {
  if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new RangeError(`${context} exceeds JavaScript's safe integer range`);
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
    throw new TypeError(
      `${context} must be a KotodamaQuantity, canonical quantity string, or bigint; JavaScript numbers are rejected`,
    );
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
    throw new Error(`${context} uses unsupported public-key curve ${curve}`);
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
    throw new Error(`${context} must be a canonical public-key multihash literal`);
  }
  const bytes = Buffer.from(normalized, HEX_ENCODING);
  let offset = 0;
  const [multicodec, multicodecBytes] = decodeUnsignedLeb128(bytes, offset, `${context}.multicodec`);
  offset += multicodecBytes;
  const [payloadLength, payloadLengthBytes] = decodeUnsignedLeb128(bytes, offset, `${context}.length`);
  offset += payloadLengthBytes;
  const remaining = bytes.subarray(offset);
  if (remaining.length !== payloadLength) {
    throw new Error(`${context} public-key multihash length header is invalid`);
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
      throw new RangeError(`${context} varint exceeds an unsigned 64-bit integer`);
    }
    value |= (byte & 0x7fn) << shift;
    if ((byte & 0x80n) === 0n) {
      if (used > 0 && byte === 0n) {
        throw new Error(`${context} varint is not minimally encoded`);
      }
      if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
        throw new RangeError(`${context} exceeds JavaScript's safe integer range`);
      }
      return [Number(value), cursor - offset];
    }
    shift += 7n;
  }
  if (cursor >= buffer.length) {
    throw new Error(`${context} varint is truncated`);
  }
  throw new RangeError(`${context} varint exceeds an unsigned 64-bit integer`);
}

function curveIdForMulticodec(multicodec, context) {
  const entry = getCurveEntryByPublicKeyMulticodec(multicodec);
  if (!entry) {
    throw new Error(`${context} uses unsupported public-key multicodec ${multicodec}`);
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
      throw new Error(`${context} must be valid Base58`);
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
    throw new TypeError(`${context} must be a non-empty string`);
  }
  return value.trim();
}

function assertExactNonEmptyString(value, context) {
  if (typeof value !== JS_TYPE_STRING || value.length === 0) {
    throw new TypeError(`${context} must be a non-empty string`);
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
