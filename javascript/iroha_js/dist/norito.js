import { Buffer } from "buffer";
import { blake3 } from "@noble/hashes/blake3";
import { blake2b } from "@noble/hashes/blake2.js";
import { sha256 } from "@noble/hashes/sha2";
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
import { analyzeEntrypointValueTypeV1 } from "./entrypointSchema.js";
import { KotodamaQuantity, NumericV1 } from "./numericV1.js";

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
const CRC64_REFLECTED_POLY = 0xc96c5795d7870f42n;
const ASSET_DEFINITION_ADDRESS_VERSION = 1;
const BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const UINT128_MASK = (1n << 128n) - 1n;
const HASH_LITERAL_RE = /^hash:([0-9A-Fa-f]{64})#([0-9A-Fa-f]{4})$/;
const CANONICAL_HASH_LITERAL_RE = /^hash:([0-9A-F]{64})#([0-9A-F]{4})$/;
const MULTIHASH_LITERAL_RE = /^([0-9a-fA-F]+)$/;
const DEFAULT_SM2_DISTINGUISHED_ID = new Uint8Array(16);
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
  "SetAssetTransferAvailability",
  "SoraFS.ReplicationOrder.*",
  "RecordSccpMessage",
];
const CANCEL_ASSET_LOCK_WIRE_ID =
  "iroha_data_model::isi::escrow::CancelAssetLock";
const CANCEL_ASSET_LOCK_V1_SCHEMA_HASH = schemaHashForTypeName(
  CANCEL_ASSET_LOCK_WIRE_ID,
);
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
const REPLICATION_ORDER_V1_SCHEMA_HASH = schemaHashForTypeName(
  "sorafs_manifest::capacity::ReplicationOrderV1",
);
const SORAFS_REPLICATION_ORDER_MAX_PAYLOAD_BYTES_V1 = 1024 * 1024;
const INSTRUCTION_BOX_SCHEMA_HASH = Buffer.from(
  "862a7d77075d4d23ff6c1261db027811",
  "hex",
);
const MULTISIG_PROPOSE_DTO_SCHEMA_HASH = schemaHashForTypeName(
  "iroha_torii::routing::MultisigProposeDto",
);
const MULTISIG_CONTRACT_CALL_PROPOSE_DTO_SCHEMA_HASH = schemaHashForTypeName(
  "iroha_torii::routing::MultisigContractCallProposeDto",
);
const MULTISIG_CONTRACT_CALL_APPROVE_DTO_SCHEMA_HASH = schemaHashForTypeName(
  "iroha_torii::routing::MultisigContractCallApproveDto",
);
const OPEN_VERIFY_ENVELOPE_SCHEMA_HASH = schemaHashForTypeName(
  "iroha_data_model::zk::OpenVerifyEnvelope",
);
const EVENT_FILTER_BOX_SCHEMA_HASH = schemaHashForTypeName(
  "iroha_data_model::events::model::EventFilterBox",
);
const TRANSACTION_PAYLOAD_BATCH_SCHEMA_HASH = schemaHashForTypeName(
  "alloc::vec::Vec<alloc::vec::Vec<u8>>",
);
export const SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_NAME_V1 =
  "iroha.torii.v1.sorafs.billing.acknowledgement_proof";
const SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_HASH_V1 =
  schemaHashForTypeName(
    SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_NAME_V1,
  );
export const SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1 =
  64 * 1024;
const CONTRACT_MANIFEST_SIGNATURE_PAYLOAD_SCHEMA_HASH = Buffer.from(
  "b4bb42540d44c468ed44d5f94c59b007",
  "hex",
);
const BLOCK_PROOFS_TYPE_NAME =
  "iroha_data_model::block::proofs::BlockProofs";
const BLOCK_MERKLE_MAX_HEIGHT = 32;
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
  "iroha_data_model::isi::kaigi::CreateKaigi":
    "iroha_data_model::isi::kaigi::CreateKaigi",
  "iroha_data_model::isi::kaigi::JoinKaigi":
    "iroha_data_model::isi::kaigi::JoinKaigi",
  "iroha_data_model::isi::kaigi::LeaveKaigi":
    "iroha_data_model::isi::kaigi::LeaveKaigi",
  "iroha_data_model::isi::kaigi::EndKaigi":
    "iroha_data_model::isi::kaigi::EndKaigi",
  "iroha_data_model::isi::kaigi::RecordKaigiUsage":
    "iroha_data_model::isi::kaigi::RecordKaigiUsage",
  "iroha_data_model::isi::kaigi::SetKaigiRelayManifest":
    "iroha_data_model::isi::kaigi::SetKaigiRelayManifest",
  "iroha_data_model::isi::kaigi::RegisterKaigiRelay":
    "iroha_data_model::isi::kaigi::RegisterKaigiRelay",
  "iroha_data_model::isi::governance::ProposeDeployContract":
    "iroha_data_model::isi::governance::ProposeDeployContract",
  "iroha_data_model::isi::governance::CastZkBallot":
    "iroha_data_model::isi::governance::CastZkBallot",
  "iroha_data_model::isi::governance::CastPlainBallot":
    "iroha_data_model::isi::governance::CastPlainBallot",
  "iroha_data_model::isi::governance::EnactReferendum":
    "iroha_data_model::isi::governance::EnactReferendum",
  "iroha_data_model::isi::governance::FinalizeReferendum":
    "iroha_data_model::isi::governance::FinalizeReferendum",
  "iroha_data_model::isi::governance::PersistCouncilForEpoch":
    "iroha_data_model::isi::governance::PersistCouncilForEpoch",
  "iroha_data_model::isi::social::ClaimTwitterFollowReward":
    "iroha_data_model::isi::social::ClaimTwitterFollowReward",
  "iroha_data_model::isi::social::SendToTwitter":
    "iroha_data_model::isi::social::SendToTwitter",
  "iroha_data_model::isi::social::CancelTwitterEscrow":
    "iroha_data_model::isi::social::CancelTwitterEscrow",
  "iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode":
    "iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode",
  "iroha_data_model::isi::smart_contract_code::RegisterSmartContractBytes":
    "iroha_data_model::isi::smart_contract_code::RegisterSmartContractBytes",
  "iroha_data_model::isi::smart_contract_code::DeactivateContractInstance":
    "iroha_data_model::isi::smart_contract_code::DeactivateContractInstance",
  "iroha_data_model::isi::smart_contract_code::ActivateContractInstance":
    "iroha_data_model::isi::smart_contract_code::ActivateContractInstance",
  "iroha_data_model::isi::smart_contract_code::CommitContractDeployment":
    "iroha_data_model::isi::smart_contract_code::CommitContractDeployment",
  "iroha_data_model::isi::smart_contract_code::UploadSmartContractCodeChunk":
    "iroha_data_model::isi::smart_contract_code::UploadSmartContractCodeChunk",
  "iroha_data_model::isi::smart_contract_code::FinalizeSmartContractCodeUpload":
    "iroha_data_model::isi::smart_contract_code::FinalizeSmartContractCodeUpload",
  "iroha_data_model::isi::smart_contract_code::CancelSmartContractCodeUpload":
    "iroha_data_model::isi::smart_contract_code::CancelSmartContractCodeUpload",
  "iroha_data_model::isi::smart_contract_code::RemoveSmartContractBytes":
    "iroha_data_model::isi::smart_contract_code::RemoveSmartContractBytes",
  "iroha_data_model::isi::zk::RegisterZkAsset":
    "iroha_data_model::isi::zk::RegisterZkAsset",
  "iroha_data_model::isi::zk::RegisterAssetHiddenZkPool":
    "iroha_data_model::isi::zk::RegisterAssetHiddenZkPool",
  "zk::ScheduleConfidentialPolicyTransition":
    "iroha_data_model::isi::zk::ScheduleConfidentialPolicyTransition",
  "zk::CancelConfidentialPolicyTransition":
    "iroha_data_model::isi::zk::CancelConfidentialPolicyTransition",
  "iroha_data_model::isi::zk::Shield":
    "iroha_data_model::isi::zk::Shield",
  "iroha_data_model::isi::zk::ZkTransfer":
    "iroha_data_model::isi::zk::ZkTransfer",
  "iroha_data_model::isi::zk::AssetHiddenZkTransfer":
    "iroha_data_model::isi::zk::AssetHiddenZkTransfer",
  "iroha_data_model::isi::zk::Unshield":
    "iroha_data_model::isi::zk::Unshield",
  "iroha_data_model::isi::zk::CreateElection":
    "iroha_data_model::isi::zk::CreateElection",
  "iroha_data_model::isi::zk::SubmitBallot":
    "iroha_data_model::isi::zk::SubmitBallot",
  "iroha_data_model::isi::zk::FinalizeElection":
    "iroha_data_model::isi::zk::FinalizeElection",
  "iroha_data_model::isi::verifying_keys::RegisterVerifyingKey":
    "iroha_data_model::isi::verifying_keys::RegisterVerifyingKey",
  "iroha_data_model::isi::verifying_keys::UpdateVerifyingKey":
    "iroha_data_model::isi::verifying_keys::UpdateVerifyingKey",
});
const INNER_SCHEMA_HASH_BY_WIRE_ID = Object.freeze(
  Object.fromEntries(
    Object.entries(INNER_TYPE_NAME_BY_WIRE_ID).map(([wireId, typeName]) => [
      wireId,
      schemaHashForTypeName(typeName),
    ]),
  ),
);
const INNER_HEADER_PADDING_BY_WIRE_ID = Object.freeze({
  "iroha_data_model::isi::zk::Shield": 8,
  "iroha_data_model::isi::zk::Unshield": 8,
});

const CRC64_TABLE = (() => {
  const table = new Array(256);
  for (let index = 0; index < 256; index += 1) {
    let crc = BigInt(index);
    for (let bit = 0; bit < 8; bit += 1) {
      if ((crc & 1n) !== 0n) {
        crc = (crc >> 1n) ^ CRC64_REFLECTED_POLY;
      } else {
        crc >>= 1n;
      }
    }
    table[index] = crc;
  }
  return table;
})();

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
  if (typeof structuredClone === "function") {
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
  if (typeof native[method] !== "function") {
    throw new Error(`Native binding does not expose ${method}`);
  }
  return native;
}

function isNativeBindingUnavailable(error) {
  const message =
    error && typeof error.message === "string" ? error.message : String(error ?? "");
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
    error && typeof error.message === "string" ? error.message : String(error ?? "");
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

function encodeNormalizedInstruction(normalized) {
  const deployProposal = normalized?.ProposeDeployContract;
  if (
    isPlainObject(deployProposal) &&
    deployProposal.mode !== undefined &&
    deployProposal.mode !== null
  ) {
    // Rust's JSON bridge has historically accepted case-folded enum text.
    // Bind the public JS wire contract to the exact canonical spellings before
    // native dispatch so non-canonical JSON cannot acquire canonical bytes.
    encodeVotingModeValue(deployProposal.mode, "ProposeDeployContract.mode");
  }
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
    error && typeof error.message === "string" ? error.message : String(error ?? "");
  return (
    message.startsWith("Internal Norito canonicalization supports ") ||
    message.startsWith("Internal Norito decoder does not support ")
  );
}

function cacheInstructionRoundTrip(bytes, instruction) {
  try {
    instructionCache.set(
      Buffer.from(bytes).toString("hex"),
      canonicalizeInstructionForCache(instruction),
    );
  } catch {
    // Cache misses must not affect Norito encoding/decoding.
  }
}

function getCachedInstruction(bytes) {
  const cached = instructionCache.get(Buffer.from(bytes).toString("hex"));
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
  if (typeof instruction === "string") {
    const trimmed = instruction.trim();
    try {
      const parsed = JSON.parse(trimmed);
      const normalized = normalizeInstructionJsonValue(parsed);
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
    typeof requestNonceHex !== "string" ||
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
          Buffer.from(requestNonceHex, "hex"),
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
          request.private_key ?? request.privateKey ?? null,
          encodeNoritoStringValue,
          "MultisigProposeDto.private_key",
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
  if (typeof value !== "string") {
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
          request.private_key ?? request.privateKey ?? null,
          encodeNoritoStringValue,
          "MultisigContractCallProposeDto.private_key",
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
          request.private_key ?? request.privateKey ?? null,
          encodeNoritoStringValue,
          "MultisigContractCallApproveDto.private_key",
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
      return options.parseJson === false ? JSON.stringify(decoded) : decoded;
    } catch (fallbackError) {
      if (!isPureJsUnsupportedInstructionError(fallbackError)) {
        throw fallbackError;
      }
      throw error;
    }
  }
  if (options.parseJson === false) {
    return json;
  }
  return JSON.parse(json);
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
    typeof encodedAction !== "string" ||
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
    ).toString("hex"),
    root_after: decodeFixedByteArrayArchiveValue(
      fields.root_after,
      32,
      `${context}.root_after`,
    ).toString("hex"),
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
        ).toString("hex"),
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
      "entry_hash",
      "entry_root",
      "entry_proof",
      "result_root",
      "result_proof",
      "fastpq_transcripts",
    ]);
    const blockHeight = decodeU64Value(fields.block_height, "BlockProofs.block_height");
    if (blockHeight === "0") {
      throw new Error("BlockProofs.block_height must be non-zero");
    }
    return {
      block_height: blockHeight,
      entry_hash: decodeHashValue(fields.entry_hash, "BlockProofs.entry_hash"),
      entry_root: decodeHashValue(fields.entry_root, "BlockProofs.entry_root"),
      entry_proof: decodeBlockReceiptProofValue(
        fields.entry_proof,
        "BlockProofs.entry_proof",
      ),
      result_root: decodeOptionValue(
        fields.result_root,
        decodeHashValue,
        "BlockProofs.result_root",
      ),
      result_proof: decodeOptionValue(
        fields.result_proof,
        decodeBlockReceiptProofValue,
        "BlockProofs.result_proof",
      ),
      fastpq_transcripts: decodeFastpqTranscriptMap(
        fields.fastpq_transcripts,
        "BlockProofs.fastpq_transcripts",
      ),
    };
  });
}

function blockProofHashBytes(value, context) {
  const bytes = encodeHashLiteralBytes(value, context);
  if ((bytes[bytes.length - 1] & 1) !== 1) {
    throw new Error(`${context} does not carry Iroha's hash marker bit`);
  }
  return bytes;
}

function blockProofHashesEqual(left, right, context) {
  return blockProofHashBytes(left, `${context}.left`).equals(
    blockProofHashBytes(right, `${context}.right`),
  );
}

/** Verify one Iroha block Merkle audit path locally. */
export function verifyBlockMerkleProof(leaf, proof, root) {
  try {
    const leafBytes = blockProofHashBytes(leaf, "Merkle proof leaf");
    const rootBytes = blockProofHashBytes(root, "Merkle proof root");
    if (!isPlainObject(proof)) return false;
    const leafIndex = proof.leaf_index;
    const auditPath = proof.audit_path;
    if (
      !Number.isInteger(leafIndex) ||
      leafIndex < 0 ||
      leafIndex > 0xffff_ffff ||
      !Array.isArray(auditPath) ||
      auditPath.length > BLOCK_MERKLE_MAX_HEIGHT
    ) {
      return false;
    }
    if (leafIndex >= 2 ** auditPath.length) return false;

    let index = 2 ** auditPath.length - 1 + leafIndex;
    let accumulator = leafBytes;
    for (let level = 0; level < auditPath.length; level += 1) {
      const rawSibling = auditPath[level];
      const sibling = rawSibling === null
        ? null
        : blockProofHashBytes(rawSibling, `Merkle proof audit_path[${level}]`);
      const currentIsRight = index % 2 === 0;
      if (currentIsRight && sibling === null) return false;
      if (!currentIsRight && sibling === null) {
        index = Math.max(0, index - 1) >> 1;
        continue;
      }
      const parentInput = currentIsRight
        ? Buffer.concat([sibling, accumulator])
        : Buffer.concat([accumulator, sibling]);
      accumulator = Buffer.from(blake2b(parentInput, { dkLen: 32 }));
      accumulator[31] |= 1;
      index = Math.max(0, index - 1) >> 1;
    }
    return accumulator.equals(rootBytes);
  } catch {
    return false;
  }
}

/** Verify the locally-checkable entry and execution paths in `BlockProofs`. */
export function verifyBlockProofs(proofs) {
  const invalid = {
    valid: false,
    entry_hash_matches: false,
    entry_proof_valid: false,
    result_pair_consistent: false,
    result_proof_valid: null,
  };
  if (!isPlainObject(proofs) || !isPlainObject(proofs.entry_proof)) return invalid;
  try {
    const entryHashMatches = blockProofHashesEqual(
      proofs.entry_hash,
      proofs.entry_proof.leaf,
      "BlockProofs entry hash",
    );
    const entryProofValid = verifyBlockMerkleProof(
      proofs.entry_proof.leaf,
      proofs.entry_proof.proof,
      proofs.entry_root,
    );
    const hasResultRoot = proofs.result_root !== null && proofs.result_root !== undefined;
    const hasResultProof = proofs.result_proof !== null && proofs.result_proof !== undefined;
    const resultPairConsistent = hasResultRoot === hasResultProof;
    const resultProofValid = !hasResultRoot && !hasResultProof
      ? null
      : resultPairConsistent && isPlainObject(proofs.result_proof)
        ? verifyBlockMerkleProof(
            proofs.result_proof.leaf,
            proofs.result_proof.proof,
            proofs.result_root,
          )
        : false;
    return {
      valid:
        entryHashMatches &&
        entryProofValid &&
        resultPairConsistent &&
        resultProofValid !== false,
      entry_hash_matches: entryHashMatches,
      entry_proof_valid: entryProofValid,
      result_pair_consistent: resultPairConsistent,
      result_proof_valid: resultProofValid,
    };
  } catch {
    return invalid;
  }
}

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
  if (typeof bytes === "string") {
    const trimmed = bytes.trim();
    if (/^[0-9a-fA-F]+$/.test(trimmed) && trimmed.length % 2 === 0) {
      buffer = Buffer.from(trimmed, "hex");
    } else {
      buffer = Buffer.from(trimmed, "base64");
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
      "SetAssetTransferAvailability",
    )
  ) {
    assertOnlyObjectKeys(
      instruction,
      ["SetAssetTransferAvailability"],
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
    instruction.EnactReferendum ||
    instruction.FinalizeReferendum ||
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
    case "iroha_data_model::isi::governance::ProposeDeployContract":
    case "iroha_data_model::isi::governance::CastZkBallot":
    case "iroha_data_model::isi::governance::CastPlainBallot":
    case "iroha_data_model::isi::governance::EnactReferendum":
    case "iroha_data_model::isi::governance::FinalizeReferendum":
    case "iroha_data_model::isi::governance::PersistCouncilForEpoch":
      return decodeGovernanceInstructionPayload(wireId, payload);
    case "iroha_data_model::isi::social::ClaimTwitterFollowReward":
    case "iroha_data_model::isi::social::SendToTwitter":
    case "iroha_data_model::isi::social::CancelTwitterEscrow":
      return decodeSocialInstructionPayload(wireId, payload);
    case "iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode":
    case "iroha_data_model::isi::smart_contract_code::RegisterSmartContractBytes":
    case "iroha_data_model::isi::smart_contract_code::DeactivateContractInstance":
    case "iroha_data_model::isi::smart_contract_code::ActivateContractInstance":
    case "iroha_data_model::isi::smart_contract_code::CommitContractDeployment":
    case "iroha_data_model::isi::smart_contract_code::UploadSmartContractCodeChunk":
    case "iroha_data_model::isi::smart_contract_code::FinalizeSmartContractCodeUpload":
    case "iroha_data_model::isi::smart_contract_code::CancelSmartContractCodeUpload":
    case "iroha_data_model::isi::smart_contract_code::RemoveSmartContractBytes":
      return decodeSmartContractInstructionPayload(wireId, payload);
    case "iroha_data_model::isi::kaigi::CreateKaigi":
    case "iroha_data_model::isi::kaigi::JoinKaigi":
    case "iroha_data_model::isi::kaigi::LeaveKaigi":
    case "iroha_data_model::isi::kaigi::EndKaigi":
    case "iroha_data_model::isi::kaigi::RecordKaigiUsage":
    case "iroha_data_model::isi::kaigi::SetKaigiRelayManifest":
    case "iroha_data_model::isi::kaigi::RegisterKaigiRelay":
      return decodeKaigiInstructionPayload(wireId, payload);
    case "iroha_data_model::isi::zk::RegisterZkAsset":
    case "iroha_data_model::isi::zk::RegisterAssetHiddenZkPool":
    case "zk::ScheduleConfidentialPolicyTransition":
    case "zk::CancelConfidentialPolicyTransition":
    case "iroha_data_model::isi::zk::Shield":
    case "iroha_data_model::isi::zk::ZkTransfer":
    case "iroha_data_model::isi::zk::AssetHiddenZkTransfer":
    case "iroha_data_model::isi::zk::Unshield":
    case "iroha_data_model::isi::zk::CreateElection":
    case "iroha_data_model::isi::zk::SubmitBallot":
    case "iroha_data_model::isi::zk::FinalizeElection":
      return decodeZkInstructionPayload(wireId, payload);
    case "iroha_data_model::isi::verifying_keys::RegisterVerifyingKey":
    case "iroha_data_model::isi::verifying_keys::UpdateVerifyingKey":
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
    value !== null && typeof value === "object"
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
  if (typeof escrowId !== "string") {
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
  const hashBytes = Buffer.from(hashBody, "hex");
  if ((hashBytes[hashBytes.length - 1] & 1) === 0) {
    throw new TypeError(
      "CancelAssetLockV1.escrow_id must use a native hash with its marker bit set",
    );
  }

  if (typeof expectedRemainingAmount !== "string") {
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
    "CancelAssetLock.expected_remaining_amount",
  );
  if (expected.mantissa <= 0n) {
    throw new RangeError(
      "CancelAssetLock.expected_remaining_amount must be greater than zero",
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
        "CancelAssetLock.expected_remaining_amount",
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
    "CancelAssetLock.expected_remaining_amount",
  );
  if (
    NumericV1.decodeQuantityJson(expectedRemainingAmount).mantissa <= 0n
  ) {
    throw new RangeError(
      "CancelAssetLock.expected_remaining_amount must be greater than zero",
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
 * @returns {Buffer}
 */
export function encodeCancelAssetLockV1(value) {
  const canonical = normalizeStrictCancelAssetLockV1(value);
  const payload = withNoritoCompactLengths(() =>
    encodeCancelAssetLockPayload(canonical),
  );
  return frameNoritoPayload(
    payload,
    CANCEL_ASSET_LOCK_V1_SCHEMA_HASH,
    COMPACT_LEN_FLAG,
  );
}

/**
 * Decode one exact schema-bound bare `CancelAssetLock` V1 archive.
 *
 * Only byte containers are accepted. Textual hex/base64 aliases, arrays,
 * padding, substituted schemas or flags, and trailing bytes are rejected.
 *
 * @param {ArrayBufferView | ArrayBuffer | Buffer} bytes
 * @returns {{escrow_id: string, expected_remaining_amount: string}}
 */
export function decodeCancelAssetLockV1(bytes) {
  if (!isBinaryLike(bytes)) {
    throw new TypeError(
      "CancelAssetLockV1 archive must be a Buffer, ArrayBuffer, or typed array",
    );
  }
  const archive = toBuffer(bytes);
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
    typeof reason !== "string" ||
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
    Buffer.byteLength(reason, "utf8") >
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
  assertOnlyObjectKeys(value, fields, "SetAssetTransferAvailability");
  for (const field of fields.slice(0, 5)) {
    if (!Object.prototype.hasOwnProperty.call(value, field)) {
      throw new TypeError(`SetAssetTransferAvailability.${field} is required`);
    }
  }
  const reason = value.reason ?? null;
  validateAssetTransferAvailabilityReason(
    reason,
    "SetAssetTransferAvailability.reason",
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
        "SetAssetTransferAvailability.reason",
      ),
    ],
  ]);
  return encodeInstructionEnvelope(
    SET_ASSET_TRANSFER_AVAILABILITY_WIRE_ID,
    payload,
  );
}

function decodeSetAssetTransferAvailabilityInstructionPayload(payload) {
  const fields = decodeStructFields(payload, "SetAssetTransferAvailability", [
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
    "SetAssetTransferAvailability.reason",
  );
  validateAssetTransferAvailabilityReason(
    reason,
    "SetAssetTransferAvailability.reason",
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
    case "iroha_data_model::isi::governance::ProposeDeployContract": {
      const fields = decodeStructFields(payload, "ProposeDeployContract", [
        "contract_address",
        "code_hash_hex",
        "abi_hash_hex",
        "abi_version",
        "window",
        "mode",
        "limits",
      ]);
      const decoded = {
        contract_address: decodeStringValue(
          fields.contract_address,
          "ProposeDeployContract.contract_address",
        ),
        code_hash_hex: decodeStringValue(fields.code_hash_hex, "ProposeDeployContract.code_hash_hex"),
        abi_hash_hex: decodeStringValue(fields.abi_hash_hex, "ProposeDeployContract.abi_hash_hex"),
        abi_version: decodeStringValue(fields.abi_version, "ProposeDeployContract.abi_version"),
      };
      const window = decodeOptionValue(fields.window, decodeAtWindowValue, "ProposeDeployContract.window");
      const mode = decodeOptionValue(fields.mode, decodeVotingModeValue, "ProposeDeployContract.mode");
      const limits = decodeOptionValue(fields.limits, decodeJsonValue, "ProposeDeployContract.limits");
      if (window !== null) {
        decoded.window = window;
      }
      if (mode !== null) {
        decoded.mode = mode;
      }
      if (limits !== null) {
        decoded.limits = limits;
      }
      return { ProposeDeployContract: decoded };
    }
    case "iroha_data_model::isi::governance::CastZkBallot": {
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
    case "iroha_data_model::isi::governance::CastPlainBallot": {
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
    case "iroha_data_model::isi::governance::EnactReferendum": {
      const fields = decodeStructFields(payload, "EnactReferendum", [
        "referendum_id",
        "preimage_hash",
        "at_window",
      ]);
      return {
        EnactReferendum: {
          referendum_id: Array.from(
            decodeFixedBytesValue(fields.referendum_id, 32, "EnactReferendum.referendum_id"),
          ),
          preimage_hash: Array.from(
            decodeFixedBytesValue(fields.preimage_hash, 32, "EnactReferendum.preimage_hash"),
          ),
          at_window: decodeAtWindowValue(fields.at_window, "EnactReferendum.at_window"),
        },
      };
    }
    case "iroha_data_model::isi::governance::FinalizeReferendum": {
      const fields = decodeStructFields(payload, "FinalizeReferendum", [
        "referendum_id",
        "proposal_id",
      ]);
      return {
        FinalizeReferendum: {
          referendum_id: decodeStringValue(fields.referendum_id, "FinalizeReferendum.referendum_id"),
          proposal_id: Array.from(
            decodeFixedBytesValue(fields.proposal_id, 32, "FinalizeReferendum.proposal_id"),
          ),
        },
      };
    }
    case "iroha_data_model::isi::governance::PersistCouncilForEpoch": {
      const fields = decodeStructFields(payload, "PersistCouncilForEpoch", [
        "epoch",
        "members",
        "alternates",
        "verified",
        "candidates_count",
        "derived_by",
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
          verified: decodeU32Value(fields.verified, "PersistCouncilForEpoch.verified"),
          candidates_count: decodeU32Value(
            fields.candidates_count,
            "PersistCouncilForEpoch.candidates_count",
          ),
          derived_by: decodeCouncilDerivationKindValue(
            fields.derived_by,
            "PersistCouncilForEpoch.derived_by",
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
    case "iroha_data_model::isi::social::ClaimTwitterFollowReward": {
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
    case "iroha_data_model::isi::social::SendToTwitter": {
      const fields = decodeStructFields(payload, "SendToTwitter", ["binding_hash", "amount"]);
      return {
        SendToTwitter: {
          binding_hash: decodeKeyedHashValue(fields.binding_hash, "SendToTwitter.binding_hash"),
          amount: decodeQuantityValue(fields.amount, "SendToTwitter.amount"),
        },
      };
    }
    case "iroha_data_model::isi::social::CancelTwitterEscrow": {
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
    case "iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode": {
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
    case "iroha_data_model::isi::smart_contract_code::RegisterSmartContractBytes": {
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
    case "iroha_data_model::isi::smart_contract_code::DeactivateContractInstance": {
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
    case "iroha_data_model::isi::smart_contract_code::ActivateContractInstance": {
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
    case "iroha_data_model::isi::smart_contract_code::CommitContractDeployment": {
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
            "CommitContractDeployment.expected_previous_contract_address",
          ),
        },
      };
    }
    case "iroha_data_model::isi::smart_contract_code::UploadSmartContractCodeChunk": {
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
    case "iroha_data_model::isi::smart_contract_code::FinalizeSmartContractCodeUpload": {
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
    case "iroha_data_model::isi::smart_contract_code::CancelSmartContractCodeUpload": {
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
    case "iroha_data_model::isi::smart_contract_code::RemoveSmartContractBytes": {
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
    case "iroha_data_model::isi::kaigi::CreateKaigi": {
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
    case "iroha_data_model::isi::kaigi::JoinKaigi":
    case "iroha_data_model::isi::kaigi::LeaveKaigi": {
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
    case "iroha_data_model::isi::kaigi::EndKaigi": {
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
    case "iroha_data_model::isi::kaigi::RecordKaigiUsage": {
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
    case "iroha_data_model::isi::kaigi::SetKaigiRelayManifest": {
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
    case "iroha_data_model::isi::kaigi::RegisterKaigiRelay": {
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
    case "iroha_data_model::isi::zk::RegisterZkAsset": {
      const fields = decodeStructFields(payload, "zk.RegisterZkAsset", [
        "asset",
        "mode",
        "allow_shield",
        "allow_unshield",
        "vk_transfer",
        "vk_unshield",
        "vk_shield",
      ]);
      return {
        zk: {
          RegisterZkAsset: {
            asset: decodeAssetDefinitionIdValue(fields.asset, "zk.RegisterZkAsset.asset"),
            mode: decodeZkAssetModeValue(fields.mode, "zk.RegisterZkAsset.mode"),
            allow_shield: decodeBoolValue(
              fields.allow_shield,
              "zk.RegisterZkAsset.allow_shield",
            ),
            allow_unshield: decodeBoolValue(
              fields.allow_unshield,
              "zk.RegisterZkAsset.allow_unshield",
            ),
            vk_transfer: decodeOptionValue(
              fields.vk_transfer,
              decodeVerifyingKeyIdValue,
              "zk.RegisterZkAsset.vk_transfer",
            ),
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
    case "iroha_data_model::isi::zk::RegisterAssetHiddenZkPool": {
      const fields = decodeStructFields(payload, "zk.RegisterAssetHiddenZkPool", [
        "pool_id",
        "storage_asset",
        "asset_set_root",
        "vk_transfer",
      ]);
      return {
        zk: {
          RegisterAssetHiddenZkPool: {
            pool_id: decodeStringValue(
              fields.pool_id,
              "zk.RegisterAssetHiddenZkPool.pool_id",
            ),
            storage_asset: decodeAssetDefinitionIdValue(
              fields.storage_asset,
              "zk.RegisterAssetHiddenZkPool.storage_asset",
            ),
            asset_set_root: Array.from(
              decodeFixedBytesValue(
                fields.asset_set_root,
                32,
                "zk.RegisterAssetHiddenZkPool.asset_set_root",
              ),
            ),
            vk_transfer: decodeVerifyingKeyIdValue(
              fields.vk_transfer,
              "zk.RegisterAssetHiddenZkPool.vk_transfer",
            ),
          },
        },
      };
    }
    case "zk::ScheduleConfidentialPolicyTransition": {
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
              "zk.ScheduleConfidentialPolicyTransition.effective_height",
            ),
            transition_id: decodeHashValue(
              fields.transition_id,
              "zk.ScheduleConfidentialPolicyTransition.transition_id",
            ),
            conversion_window: decodeOptionValue(
              fields.conversion_window,
              decodeU64NumberValue,
              "zk.ScheduleConfidentialPolicyTransition.conversion_window",
            ),
          },
        },
      };
    }
    case "zk::CancelConfidentialPolicyTransition": {
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
              "zk.CancelConfidentialPolicyTransition.transition_id",
            ),
          },
        },
      };
    }
    case "iroha_data_model::isi::zk::Shield": {
      const fields = decodeStructFields(payload, "zk.Shield", [
        "asset",
        "from",
        "amount",
        "note_commitment",
        "enc_payload",
      ]);
      return {
        zk: {
          Shield: {
            asset: decodeAssetDefinitionIdValue(fields.asset, "zk.Shield.asset"),
            from: decodeAccountIdValue(fields.from, "zk.Shield.from"),
            amount: decodeQuantityValue(fields.amount, "zk.Shield.amount"),
            note_commitment: Array.from(
              decodeFixedBytesValue(fields.note_commitment, 32, "zk.Shield.note_commitment"),
            ),
            enc_payload: decodeConfidentialEncryptedPayloadValue(
              fields.enc_payload,
              "zk.Shield.enc_payload",
            ),
          },
        },
      };
    }
    case "iroha_data_model::isi::zk::ZkTransfer": {
      const fields = decodeStructFields(payload, "zk.ZkTransfer", [
        "asset",
        "inputs",
        "outputs",
        "proof",
        "root_hint",
      ]);
      return {
        zk: {
          ZkTransfer: {
            asset: decodeAssetDefinitionIdValue(fields.asset, "zk.ZkTransfer.asset"),
            inputs: decodeNoritoVec(
              fields.inputs,
              (entry, index) =>
                Array.from(
                  decodeFixedByteArrayArchiveValue(
                    entry,
                    32,
                    `zk.ZkTransfer.inputs[${index}]`,
                  ),
                ),
              "zk.ZkTransfer.inputs",
            ),
            outputs: decodeNoritoVec(
              fields.outputs,
              (entry, index) =>
                Array.from(
                  decodeFixedByteArrayArchiveValue(
                    entry,
                    32,
                    `zk.ZkTransfer.outputs[${index}]`,
                  ),
                ),
              "zk.ZkTransfer.outputs",
            ),
            proof: decodeProofAttachmentValue(fields.proof, "zk.ZkTransfer.proof"),
            root_hint: decodeOptionValue(
              fields.root_hint,
              (entry, context) =>
                Array.from(decodeFixedByteArrayArchiveValue(entry, 32, context)),
              "zk.ZkTransfer.root_hint",
            ),
          },
        },
      };
    }
    case "iroha_data_model::isi::zk::AssetHiddenZkTransfer": {
      const fields = decodeStructFields(payload, "zk.AssetHiddenZkTransfer", [
        "pool_id",
        "inputs",
        "outputs",
        "proof",
        "root_hint",
      ]);
      return {
        zk: {
          AssetHiddenZkTransfer: {
            pool_id: decodeStringValue(
              fields.pool_id,
              "zk.AssetHiddenZkTransfer.pool_id",
            ),
            inputs: decodeNoritoVec(
              fields.inputs,
              (entry, index) =>
                Array.from(
                  decodeFixedByteArrayArchiveValue(
                    entry,
                    32,
                    `zk.AssetHiddenZkTransfer.inputs[${index}]`,
                  ),
                ),
              "zk.AssetHiddenZkTransfer.inputs",
            ),
            outputs: decodeNoritoVec(
              fields.outputs,
              (entry, index) =>
                Array.from(
                  decodeFixedByteArrayArchiveValue(
                    entry,
                    32,
                    `zk.AssetHiddenZkTransfer.outputs[${index}]`,
                  ),
                ),
              "zk.AssetHiddenZkTransfer.outputs",
            ),
            proof: decodeProofAttachmentValue(
              fields.proof,
              "zk.AssetHiddenZkTransfer.proof",
            ),
            root_hint: decodeOptionValue(
              fields.root_hint,
              (entry, context) =>
                Array.from(decodeFixedByteArrayArchiveValue(entry, 32, context)),
              "zk.AssetHiddenZkTransfer.root_hint",
            ),
          },
        },
      };
    }
    case "iroha_data_model::isi::zk::Unshield": {
      let fields;
      try {
        fields = decodeStructFields(payload, "zk.Unshield", [
          "asset",
          "to",
          "public_amount",
          "inputs",
          "outputs",
          "proof",
          "root_hint",
        ]);
      } catch (_error) {
        fields = decodeStructFields(payload, "zk.Unshield", [
          "asset",
          "to",
          "public_amount",
          "inputs",
          "proof",
          "root_hint",
        ]);
        fields.outputs = encodeNoritoVec([], (entry) => entry);
      }
      return {
        zk: {
          Unshield: {
            asset: decodeAssetDefinitionIdValue(fields.asset, "zk.Unshield.asset"),
            to: decodeAccountIdValue(fields.to, "zk.Unshield.to"),
            public_amount: decodeQuantityValue(
              fields.public_amount,
              "zk.Unshield.public_amount",
            ),
            inputs: decodeNoritoVec(
              fields.inputs,
              (entry, index) =>
                Array.from(
                  decodeFixedByteArrayArchiveValue(
                    entry,
                    32,
                    `zk.Unshield.inputs[${index}]`,
                  ),
                ),
              "zk.Unshield.inputs",
            ),
            outputs: decodeNoritoVec(
              fields.outputs,
              (entry, index) =>
                Array.from(
                  decodeFixedByteArrayArchiveValue(
                    entry,
                    32,
                    `zk.Unshield.outputs[${index}]`,
                  ),
                ),
              "zk.Unshield.outputs",
            ),
            proof: decodeProofAttachmentValue(fields.proof, "zk.Unshield.proof"),
            root_hint: decodeOptionValue(
              fields.root_hint,
              (entry, context) =>
                Array.from(decodeFixedByteArrayArchiveValue(entry, 32, context)),
              "zk.Unshield.root_hint",
            ),
          },
        },
      };
    }
    case "iroha_data_model::isi::zk::CreateElection": {
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
    case "iroha_data_model::isi::zk::SubmitBallot": {
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
    case "iroha_data_model::isi::zk::FinalizeElection": {
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
  const fields = decodeStructFields(payload, context, ["source", "object", "destination"]);
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

function decodeByteVecValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const length = bigintToSafeNumber(reader.readU64LE("length"), `${context}.length`);
  const bytes = reader.readBytes(length, "payload");
  reader.assertEof();
  return Buffer.from(bytes);
}

function decodeByteVecAsBase64(payload, context) {
  return decodeByteVecValue(payload, context).toString("base64");
}

function normalizeFlexibleBytes(value, context) {
  if (typeof value === "string") {
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
  if (typeof value === "bigint") {
    parsed = value;
  } else if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || value < 0) {
      throw new TypeError(`${context} must be a non-negative safe integer, bigint, or string`);
    }
    parsed = BigInt(value);
  } else if (typeof value === "string" && /^\d+$/.test(value.trim())) {
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
        value.balance_scope_policy ?? value.balanceScopePolicy ?? "Global",
        `${context}.balance_scope_policy`,
      ),
    ],
    [
      encodeAssetConfidentialPolicyValue(
        value.confidential_policy ?? value.confidentialPolicy ?? defaultAssetConfidentialPolicy(),
        `${context}.confidential_policy`,
      ),
    ],
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
    "confidential_policy",
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
    confidential_policy: decodeAssetConfidentialPolicyValue(
      fields.confidential_policy,
      `${context}.confidential_policy`,
    ),
  };
}

function defaultAssetConfidentialPolicy() {
  return {
    mode: "TransparentOnly",
    vk_set_hash: null,
    poseidon_params_id: null,
    pedersen_params_id: null,
    pending_transition: null,
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
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/u.test(value)) {
    throw new TypeError(
      `${context} must contain exactly 64 lowercase hexadecimal characters`,
    );
  }
  if (/^0{64}$/u.test(value)) {
    throw new TypeError(`${context} must not be the zero identifier`);
  }
  return Buffer.from(value, "hex");
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
  return bytes.toString("hex");
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
  return bytes.toString("hex");
}

function encodeExactAccountIdValue(value, context) {
  if (typeof value !== "string" || value.trim() !== value) {
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
      return bytes.toString("hex");
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
    const orderId = orderIdBytes.toString("hex");
    if (expectedOrderId !== null) {
      const expected = decodeCanonicalReplicationId(
        expectedOrderId,
        "IssueReplicationOrder.order_id",
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
      providerIds: providers.map((provider) => provider.toString("hex")),
      issuedAt,
      deadlineAt,
    };
  });
}

function encodeReplicationOrderInstruction(instruction) {
  if (isPlainObject(instruction.IssueReplicationOrder)) {
    assertOnlyObjectKeys(instruction, ["IssueReplicationOrder"], "instruction");
    const value = instruction.IssueReplicationOrder;
    assertOnlyObjectKeys(
      value,
      ["order_id", "order_payload", "issued_epoch", "deadline_epoch"],
      "IssueReplicationOrder",
    );
    const orderPayload = decodeExactStandardBase64(
      value.order_payload,
      "IssueReplicationOrder.order_payload",
    );
    validateSorafsReplicationOrderPayloadV1(orderPayload, value.order_id);
    const issuedEpoch = normalizeU64Input(
      value.issued_epoch,
      "IssueReplicationOrder.issued_epoch",
    );
    const deadlineEpoch = normalizeU64Input(
      value.deadline_epoch,
      "IssueReplicationOrder.deadline_epoch",
    );
    if (deadlineEpoch <= issuedEpoch) {
      throw new TypeError(
        "IssueReplicationOrder.deadline_epoch must be greater than issued_epoch",
      );
    }
    return encodeInstructionEnvelope(
      ISSUE_REPLICATION_ORDER_WIRE_ID,
      encodeStructValue([
        [
          encodeReplicationIdValue(
            value.order_id,
            "IssueReplicationOrder.order_id",
          ),
        ],
        [encodeByteVecValue(orderPayload, "IssueReplicationOrder.order_payload")],
        [encodeU64Value(issuedEpoch, "IssueReplicationOrder.issued_epoch")],
        [encodeU64Value(deadlineEpoch, "IssueReplicationOrder.deadline_epoch")],
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
      "CompleteReplicationOrder.expected_assignment_revision",
    );
    if (expectedAssignmentRevision === 0n) {
      throw new TypeError(
        "CompleteReplicationOrder.expected_assignment_revision must be greater than zero",
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
            "CompleteReplicationOrder.expected_assignment_revision",
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
    ]);
    const orderId = decodeReplicationIdValue(
      fields.order_id,
      "IssueReplicationOrder.order_id",
    );
    const orderPayload = decodeByteVecValue(
      fields.order_payload,
      "IssueReplicationOrder.order_payload",
    );
    validateSorafsReplicationOrderPayloadV1(orderPayload, orderId);
    const issuedEpoch = decodeU64NumberValue(
      fields.issued_epoch,
      "IssueReplicationOrder.issued_epoch",
    );
    const deadlineEpoch = decodeU64NumberValue(
      fields.deadline_epoch,
      "IssueReplicationOrder.deadline_epoch",
    );
    if (deadlineEpoch <= issuedEpoch) {
      throw new TypeError(
        "IssueReplicationOrder.deadline_epoch must be greater than issued_epoch",
      );
    }
    return {
      IssueReplicationOrder: {
        order_id: orderId,
        order_payload: orderPayload.toString("base64"),
        issued_epoch: issuedEpoch,
        deadline_epoch: deadlineEpoch,
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
      "CompleteReplicationOrder.expected_assignment_revision",
    );
    if (expectedAssignmentRevision === 0) {
      throw new TypeError(
        "CompleteReplicationOrder.expected_assignment_revision must be greater than zero",
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
      "iroha_data_model::isi::governance::ProposeDeployContract",
      encodeProposeDeployContractPayload(instruction.ProposeDeployContract),
    );
  }
  if (isPlainObject(instruction.CastZkBallot)) {
    return encodeInstructionEnvelope(
      "iroha_data_model::isi::governance::CastZkBallot",
      encodeCastZkBallotPayload(instruction.CastZkBallot),
    );
  }
  if (isPlainObject(instruction.CastPlainBallot)) {
    return encodeInstructionEnvelope(
      "iroha_data_model::isi::governance::CastPlainBallot",
      encodeCastPlainBallotPayload(instruction.CastPlainBallot),
    );
  }
  if (isPlainObject(instruction.EnactReferendum)) {
    return encodeInstructionEnvelope(
      "iroha_data_model::isi::governance::EnactReferendum",
      encodeEnactReferendumPayload(instruction.EnactReferendum),
    );
  }
  if (isPlainObject(instruction.FinalizeReferendum)) {
    return encodeInstructionEnvelope(
      "iroha_data_model::isi::governance::FinalizeReferendum",
      encodeFinalizeReferendumPayload(instruction.FinalizeReferendum),
    );
  }
  if (isPlainObject(instruction.PersistCouncilForEpoch)) {
    return encodeInstructionEnvelope(
      "iroha_data_model::isi::governance::PersistCouncilForEpoch",
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
      "iroha_data_model::isi::social::ClaimTwitterFollowReward",
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
      "iroha_data_model::isi::social::SendToTwitter",
      encodeStructValue([
        [encodeKeyedHashValue(instruction.SendToTwitter.binding_hash, "SendToTwitter.binding_hash")],
        [encodeQuantityValue(instruction.SendToTwitter.amount, "SendToTwitter.amount")],
      ]),
    );
  }
  if (isPlainObject(instruction.CancelTwitterEscrow)) {
    return encodeInstructionEnvelope(
      "iroha_data_model::isi::social::CancelTwitterEscrow",
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
      "iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode",
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
      "iroha_data_model::isi::smart_contract_code::RegisterSmartContractBytes",
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
      "iroha_data_model::isi::smart_contract_code::DeactivateContractInstance",
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
      "iroha_data_model::isi::smart_contract_code::ActivateContractInstance",
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
      "iroha_data_model::isi::smart_contract_code::CommitContractDeployment",
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
          "CommitContractDeployment.expected_previous_contract_address",
        )],
      ]),
    );
  }
  if (isPlainObject(instruction.UploadSmartContractCodeChunk)) {
    return encodeInstructionEnvelope(
      "iroha_data_model::isi::smart_contract_code::UploadSmartContractCodeChunk",
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
      "iroha_data_model::isi::smart_contract_code::FinalizeSmartContractCodeUpload",
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
      "iroha_data_model::isi::smart_contract_code::CancelSmartContractCodeUpload",
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
      "iroha_data_model::isi::smart_contract_code::RemoveSmartContractBytes",
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

function encodeProposeDeployContractPayload(value) {
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.contract_address, "ProposeDeployContract.contract_address"))],
    [encodeNoritoStringValue(assertNonEmptyString(value.code_hash_hex, "ProposeDeployContract.code_hash_hex"))],
    [encodeNoritoStringValue(assertNonEmptyString(value.abi_hash_hex, "ProposeDeployContract.abi_hash_hex"))],
    [encodeNoritoStringValue(assertNonEmptyString(value.abi_version, "ProposeDeployContract.abi_version"))],
    [encodeOptionValue(value.window ?? null, encodeAtWindowValue, "ProposeDeployContract.window")],
    [encodeOptionValue(value.mode ?? null, encodeVotingModeValue, "ProposeDeployContract.mode")],
    [encodeOptionValue(value.limits ?? null, encodeNoritoJsonValue, "ProposeDeployContract.limits")],
  ]);
}

function encodeCastZkBallotPayload(value) {
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.election_id, "CastZkBallot.election_id"))],
    [encodeNoritoStringValue(assertNonEmptyString(value.proof_b64, "CastZkBallot.proof_b64"))],
    [encodeNoritoStringValue(
      assertNonEmptyString(value.public_inputs_json ?? "{}", "CastZkBallot.public_inputs_json"),
    )],
  ]);
}

function encodeCastPlainBallotPayload(value) {
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.referendum_id, "CastPlainBallot.referendum_id"))],
    [encodeAccountIdValue(value.owner, "CastPlainBallot.owner")],
    [encodeQuantityValue(value.amount, "CastPlainBallot.amount")],
    [encodeU64NumberValue(value.duration_blocks, "CastPlainBallot.duration_blocks")],
    [encodeU8Value(value.direction, "CastPlainBallot.direction")],
  ]);
}

function encodeEnactReferendumPayload(value) {
  return encodeStructValue([
    [encodeFixedBytesValue(value.referendum_id, 32, "EnactReferendum.referendum_id")],
    [encodeFixedBytesValue(value.preimage_hash, 32, "EnactReferendum.preimage_hash")],
    [encodeAtWindowValue(value.at_window ?? { lower: 0, upper: 0 }, "EnactReferendum.at_window")],
  ]);
}

function encodeFinalizeReferendumPayload(value) {
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.referendum_id, "FinalizeReferendum.referendum_id"))],
    [encodeFixedBytesValue(value.proposal_id, 32, "FinalizeReferendum.proposal_id")],
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
    [encodeU32Value(value.verified ?? 0, "PersistCouncilForEpoch.verified")],
    [encodeU32Value(value.candidates_count, "PersistCouncilForEpoch.candidates_count")],
    [encodeCouncilDerivationKindValue(value.derived_by, "PersistCouncilForEpoch.derived_by")],
  ]);
}

function encodeAtWindowValue(value, context) {
  return encodeStructValue([
    [encodeU64NumberValue(value.lower ?? 0, `${context}.lower`)],
    [encodeU64NumberValue(value.upper ?? 0, `${context}.upper`)],
  ]);
}

function decodeAtWindowValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["lower", "upper"]);
  return {
    lower: decodeU64NumberValue(fields.lower, `${context}.lower`),
    upper: decodeU64NumberValue(fields.upper, `${context}.upper`),
  };
}

function encodeKaigiInstruction(instruction) {
  if (isPlainObject(instruction.CreateKaigi)) {
    return encodeInstructionEnvelope(
      "iroha_data_model::isi::kaigi::CreateKaigi",
      encodeCreateKaigiPayload(instruction.CreateKaigi),
    );
  }
  if (isPlainObject(instruction.JoinKaigi)) {
    return encodeInstructionEnvelope(
      "iroha_data_model::isi::kaigi::JoinKaigi",
      encodeJoinLeaveKaigiPayload(instruction.JoinKaigi, "JoinKaigi"),
    );
  }
  if (isPlainObject(instruction.LeaveKaigi)) {
    return encodeInstructionEnvelope(
      "iroha_data_model::isi::kaigi::LeaveKaigi",
      encodeJoinLeaveKaigiPayload(instruction.LeaveKaigi, "LeaveKaigi"),
    );
  }
  if (isPlainObject(instruction.EndKaigi)) {
    return encodeInstructionEnvelope(
      "iroha_data_model::isi::kaigi::EndKaigi",
      encodeEndKaigiPayload(instruction.EndKaigi),
    );
  }
  if (isPlainObject(instruction.RecordKaigiUsage)) {
    return encodeInstructionEnvelope(
      "iroha_data_model::isi::kaigi::RecordKaigiUsage",
      encodeRecordKaigiUsagePayload(instruction.RecordKaigiUsage),
    );
  }
  if (isPlainObject(instruction.SetKaigiRelayManifest)) {
    return encodeInstructionEnvelope(
      "iroha_data_model::isi::kaigi::SetKaigiRelayManifest",
      encodeSetKaigiRelayManifestPayload(instruction.SetKaigiRelayManifest),
    );
  }
  if (isPlainObject(instruction.RegisterKaigiRelay)) {
    return encodeInstructionEnvelope(
      "iroha_data_model::isi::kaigi::RegisterKaigiRelay",
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
      "iroha_data_model::isi::verifying_keys::RegisterVerifyingKey",
      encodeVerifyingKeyInstructionPayload,
    ],
    [
      "UpdateVerifyingKey",
      "iroha_data_model::isi::verifying_keys::UpdateVerifyingKey",
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
    wireId === "iroha_data_model::isi::verifying_keys::RegisterVerifyingKey"
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
    ["RegisterZkAsset", "iroha_data_model::isi::zk::RegisterZkAsset", encodeRegisterZkAssetPayload],
    ["RegisterAssetHiddenZkPool", "iroha_data_model::isi::zk::RegisterAssetHiddenZkPool", encodeRegisterAssetHiddenZkPoolPayload],
    ["ScheduleConfidentialPolicyTransition", "zk::ScheduleConfidentialPolicyTransition", encodeScheduleConfidentialPolicyTransitionPayload],
    ["CancelConfidentialPolicyTransition", "zk::CancelConfidentialPolicyTransition", encodeCancelConfidentialPolicyTransitionPayload],
    ["Shield", "iroha_data_model::isi::zk::Shield", encodeShieldPayload],
    ["ZkTransfer", "iroha_data_model::isi::zk::ZkTransfer", encodeZkTransferPayload],
    ["AssetHiddenZkTransfer", "iroha_data_model::isi::zk::AssetHiddenZkTransfer", encodeAssetHiddenZkTransferPayload],
    ["Unshield", "iroha_data_model::isi::zk::Unshield", encodeUnshieldPayload],
    ["CreateElection", "iroha_data_model::isi::zk::CreateElection", encodeCreateElectionPayload],
    ["SubmitBallot", "iroha_data_model::isi::zk::SubmitBallot", encodeSubmitBallotPayload],
    ["FinalizeElection", "iroha_data_model::isi::zk::FinalizeElection", encodeFinalizeElectionPayload],
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
  return encodeStructValue([
    [encodeAssetDefinitionIdValue(value.asset, "zk.RegisterZkAsset.asset")],
    [encodeZkAssetModeValue(value.mode, "zk.RegisterZkAsset.mode")],
    [encodeBoolValue(value.allow_shield, "zk.RegisterZkAsset.allow_shield")],
    [encodeBoolValue(value.allow_unshield, "zk.RegisterZkAsset.allow_unshield")],
    [encodeOptionValue(value.vk_transfer, encodeVerifyingKeyIdValue, "zk.RegisterZkAsset.vk_transfer")],
    [encodeOptionValue(value.vk_unshield, encodeVerifyingKeyIdValue, "zk.RegisterZkAsset.vk_unshield")],
    [encodeOptionValue(value.vk_shield, encodeVerifyingKeyIdValue, "zk.RegisterZkAsset.vk_shield")],
  ]);
}

function encodeRegisterAssetHiddenZkPoolPayload(value) {
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.pool_id, "zk.RegisterAssetHiddenZkPool.pool_id"))],
    [encodeAssetDefinitionIdValue(value.storage_asset, "zk.RegisterAssetHiddenZkPool.storage_asset")],
    [encodeFixedBytesValue(value.asset_set_root, 32, "zk.RegisterAssetHiddenZkPool.asset_set_root")],
    [encodeVerifyingKeyIdValue(value.vk_transfer, "zk.RegisterAssetHiddenZkPool.vk_transfer")],
  ]);
}

function encodeScheduleConfidentialPolicyTransitionPayload(value) {
  return encodeStructValue([
    [encodeAssetDefinitionIdValue(value.asset, "zk.ScheduleConfidentialPolicyTransition.asset")],
    [encodeConfidentialPolicyModeValue(value.new_mode, "zk.ScheduleConfidentialPolicyTransition.new_mode")],
    [encodeU64NumberValue(value.effective_height, "zk.ScheduleConfidentialPolicyTransition.effective_height")],
    [encodeHashValue(value.transition_id, "zk.ScheduleConfidentialPolicyTransition.transition_id")],
    [encodeOptionValue(value.conversion_window, encodeU64NumberValue, "zk.ScheduleConfidentialPolicyTransition.conversion_window")],
  ]);
}

function encodeCancelConfidentialPolicyTransitionPayload(value) {
  return encodeStructValue([
    [encodeAssetDefinitionIdValue(value.asset, "zk.CancelConfidentialPolicyTransition.asset")],
    [encodeHashValue(value.transition_id, "zk.CancelConfidentialPolicyTransition.transition_id")],
  ]);
}

function encodeShieldPayload(value) {
  return encodeStructValue([
    [encodeAssetDefinitionIdValue(value.asset, "zk.Shield.asset")],
    [encodeAccountIdValue(value.from, "zk.Shield.from")],
    [encodeQuantityValue(value.amount, "zk.Shield.amount")],
    [encodeFixedBytesValue(value.note_commitment, 32, "zk.Shield.note_commitment")],
    [encodeConfidentialEncryptedPayloadValue(value.enc_payload, "zk.Shield.enc_payload")],
  ]);
}

function encodeZkTransferPayload(value) {
  return encodeStructValue([
    [encodeAssetDefinitionIdValue(value.asset, "zk.ZkTransfer.asset")],
    [encodeNoritoVec(value.inputs ?? [], (entry, index) =>
      encodeFixedByteArrayArchiveValue(entry, 32, `zk.ZkTransfer.inputs[${index}]`),
    )],
    [encodeNoritoVec(value.outputs ?? [], (entry, index) =>
      encodeFixedByteArrayArchiveValue(entry, 32, `zk.ZkTransfer.outputs[${index}]`),
    )],
    [encodeProofAttachmentValue(value.proof, "zk.ZkTransfer.proof")],
    [
      encodeOptionValue(
        value.root_hint,
        (entry, context) => encodeFixedByteArrayArchiveValue(entry, 32, context),
        "zk.ZkTransfer.root_hint",
      ),
    ],
  ]);
}

function encodeAssetHiddenZkTransferPayload(value) {
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.pool_id, "zk.AssetHiddenZkTransfer.pool_id"))],
    [encodeNoritoVec(value.inputs ?? [], (entry, index) =>
      encodeFixedByteArrayArchiveValue(entry, 32, `zk.AssetHiddenZkTransfer.inputs[${index}]`),
    )],
    [encodeNoritoVec(value.outputs ?? [], (entry, index) =>
      encodeFixedByteArrayArchiveValue(entry, 32, `zk.AssetHiddenZkTransfer.outputs[${index}]`),
    )],
    [encodeProofAttachmentValue(value.proof, "zk.AssetHiddenZkTransfer.proof")],
    [
      encodeOptionValue(
        value.root_hint,
        (entry, context) => encodeFixedByteArrayArchiveValue(entry, 32, context),
        "zk.AssetHiddenZkTransfer.root_hint",
      ),
    ],
  ]);
}

function encodeUnshieldPayload(value) {
  return encodeStructValue([
    [encodeAssetDefinitionIdValue(value.asset, "zk.Unshield.asset")],
    [encodeAccountIdValue(value.to, "zk.Unshield.to")],
    [encodeQuantityValue(value.public_amount, "zk.Unshield.public_amount")],
    [encodeNoritoVec(value.inputs ?? [], (entry, index) =>
      encodeFixedByteArrayArchiveValue(entry, 32, `zk.Unshield.inputs[${index}]`),
    )],
    [encodeNoritoVec(value.outputs ?? [], (entry, index) =>
      encodeFixedByteArrayArchiveValue(entry, 32, `zk.Unshield.outputs[${index}]`),
    )],
    [encodeProofAttachmentValue(value.proof, "zk.Unshield.proof")],
    [
      encodeOptionValue(
        value.root_hint,
        (entry, context) => encodeFixedByteArrayArchiveValue(entry, 32, context),
        "zk.Unshield.root_hint",
      ),
    ],
  ]);
}

function encodeCreateElectionPayload(value) {
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.election_id, "zk.CreateElection.election_id"))],
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
    [encodeNoritoStringValue(assertNonEmptyString(value.election_id, "zk.SubmitBallot.election_id"))],
    [encodeByteVecValue(value.ciphertext, "zk.SubmitBallot.ciphertext")],
    [encodeProofAttachmentValue(value.ballot_proof, "zk.SubmitBallot.ballot_proof")],
    [encodeFixedBytesValue(value.nullifier, 32, "zk.SubmitBallot.nullifier")],
  ]);
}

function encodeFinalizeElectionPayload(value) {
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.election_id, "zk.FinalizeElection.election_id"))],
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
    typeof value === "string" ? value : `${value.domain_id}:${value.call_name}`,
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
      return Buffer.from(bytes).toString("base64");
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
  const object = decodeQuantityValue(readNoritoField(reader, "object"), `${context}.object`);
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
  const object = decodeQuantityValue(readNoritoField(reader, "object"), "Transfer.Asset.object");
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
  const object = decodeU32Value(readNoritoField(reader, "object"), `${context}.object`);
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
  if (!controller || typeof controller.tag !== "number") {
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
    case "ed25519":
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
      return curveIdFromAlgorithm("ed25519");
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
  if (typeof value !== "string") {
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
  if (typeof value !== "string") {
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
      bytes = Buffer.from(upper, "hex");
    } else if (/^[0-9A-Fa-f]{64}$/.test(literal)) {
      bytes = Buffer.from(literal, "hex");
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
  const body = bytes.toString("hex").toUpperCase();
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
  const normalized =
    typeof value === "string" ? parseMintableLabel(value, context) : parseMintableObject(value, context);
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

function parseMintableObject(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be a string or object`);
  }
  const kind = assertNonEmptyString(value.kind, `${context}.kind`);
  if (kind === "Infinitely" || kind === "Once" || kind === "Not") {
    return { kind };
  }
  if (kind === "Limited") {
    return {
      kind,
      tokens: parseMintabilityTokens(value.tokens ?? value.value, `${context}.tokens`),
    };
  }
  throw new Error(`${context}.kind must be Infinitely, Once, Not, or Limited`);
}

function parseMintabilityTokens(value, context) {
  let normalized;
  if (typeof value === "string") {
    if (!/^\d+$/.test(value)) {
      throw new TypeError(`${context} must be a positive unsigned 32-bit integer`);
    }
    normalized = Number(value);
  } else {
    normalized = Number(value);
  }
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

function encodeAssetConfidentialPolicyValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  return encodeStructValue([
    [encodeConfidentialPolicyModeValue(value.mode ?? "TransparentOnly", `${context}.mode`)],
    [encodeOptionValue(value.vk_set_hash ?? null, encodeHashValue, `${context}.vk_set_hash`)],
    [encodeOptionValue(value.poseidon_params_id ?? null, encodeU32Value, `${context}.poseidon_params_id`)],
    [encodeOptionValue(value.pedersen_params_id ?? null, encodeU32Value, `${context}.pedersen_params_id`)],
    [
      encodeOptionValue(
        value.pending_transition ?? null,
        encodeConfidentialPolicyTransitionValue,
        `${context}.pending_transition`,
      ),
    ],
  ]);
}

function decodeAssetConfidentialPolicyValue(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "mode",
    "vk_set_hash",
    "poseidon_params_id",
    "pedersen_params_id",
    "pending_transition",
  ]);
  return {
    mode: decodeConfidentialPolicyModeValue(fields.mode, `${context}.mode`),
    vk_set_hash: decodeOptionValue(fields.vk_set_hash, decodeHashValue, `${context}.vk_set_hash`),
    poseidon_params_id: decodeOptionValue(
      fields.poseidon_params_id,
      decodeU32Value,
      `${context}.poseidon_params_id`,
    ),
    pedersen_params_id: decodeOptionValue(
      fields.pedersen_params_id,
      decodeU32Value,
      `${context}.pedersen_params_id`,
    ),
    pending_transition: decodeOptionValue(
      fields.pending_transition,
      decodeConfidentialPolicyTransitionValue,
      `${context}.pending_transition`,
    ),
  };
}

function encodeConfidentialPolicyTransitionValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  return encodeStructValue([
    [encodeConfidentialPolicyModeValue(value.new_mode, `${context}.new_mode`)],
    [encodeU64NumberValue(value.effective_height, `${context}.effective_height`)],
    [encodeConfidentialPolicyModeValue(value.previous_mode, `${context}.previous_mode`)],
    [encodeHashValue(value.transition_id, `${context}.transition_id`)],
    [encodeOptionValue(value.conversion_window ?? null, encodeU64NumberValue, `${context}.conversion_window`)],
  ]);
}

function decodeConfidentialPolicyTransitionValue(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "new_mode",
    "effective_height",
    "previous_mode",
    "transition_id",
    "conversion_window",
  ]);
  return {
    new_mode: decodeConfidentialPolicyModeValue(fields.new_mode, `${context}.new_mode`),
    effective_height: decodeU64NumberValue(fields.effective_height, `${context}.effective_height`),
    previous_mode: decodeConfidentialPolicyModeValue(fields.previous_mode, `${context}.previous_mode`),
    transition_id: decodeHashValue(fields.transition_id, `${context}.transition_id`),
    conversion_window: decodeOptionValue(
      fields.conversion_window,
      decodeU64NumberValue,
      `${context}.conversion_window`,
    ),
  };
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
  if (typeof value !== "string") {
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

function encodeCouncilDerivationKindValue(value, context) {
  const normalized = assertNonEmptyString(value, context).toLowerCase();
  if (normalized === "vrf") {
    return encodeEnumTagValue(0);
  }
  throw new Error(`${context} must be Vrf`);
}

function decodeCouncilDerivationKindValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("tag");
  reader.assertEof();
  switch (tag) {
    case 0:
      return "Vrf";
    case 1:
      throw new Error(`${context} uses unsupported derivation kind 1`);
    default:
      throw new Error(`${context} uses unsupported derivation kind ${tag}`);
  }
}

function encodeVotingModeValue(value, context) {
  if (value === "Zk") {
    return encodeEnumTagValue(0);
  }
  if (value === "Plain") {
    return encodeEnumTagValue(1);
  }
  throw new Error(`${context} must be Zk or Plain`);
}

function decodeVotingModeValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("tag");
  reader.assertEof();
  switch (tag) {
    case 0:
      return "Zk";
    case 1:
      return "Plain";
    default:
      throw new Error(`${context} uses unsupported voting mode ${tag}`);
  }
}

function encodeKaigiPrivacyModeValue(value, context) {
  const mode =
    typeof value === "string" ? value : value?.mode ?? value?.privacy_mode ?? value?.kind;
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
  const policy = typeof value === "string" ? value : value?.policy ?? value?.room_policy;
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

function encodeZkAssetModeValue(value, context) {
  const normalized = assertNonEmptyString(value, context).toLowerCase();
  if (normalized === "zknative") {
    return encodeEnumTagValue(0);
  }
  if (normalized === "hybrid") {
    return encodeEnumTagValue(1);
  }
  throw new Error(`${context} must be ZkNative or Hybrid`);
}

function decodeZkAssetModeValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("tag");
  reader.assertEof();
  switch (tag) {
    case 0:
      return "ZkNative";
    case 1:
      return "Hybrid";
    default:
      throw new Error(`${context} uses unsupported zk asset mode ${tag}`);
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

function encodeProofBoxValue(value, context) {
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
  return {
    backend: decodeStringValue(fields.backend, `${context}.backend`),
    bytes: Array.from(decodeByteVecValue(fields.bytes, `${context}.bytes`)),
  };
}

function encodeVerifyingKeyBoxValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.backend, `${context}.backend`))],
    [encodeByteVecValue(value.bytes, `${context}.bytes`)],
  ]);
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
    [encodeOptionValue(value.key, encodeVerifyingKeyBoxValue, `${context}.key`)],
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
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  const parts = [
    encodeNoritoField(encodeNoritoStringValue(assertNonEmptyString(value.backend, `${context}.backend`))),
    encodeNoritoField(encodeProofBoxValue(value.proof, `${context}.proof`)),
    encodeNoritoField(encodeVerifyingKeyIdValue(value.vk_ref, `${context}.vk_ref`)),
  ];
  const hasLanePrivacy = value.lane_privacy !== undefined && value.lane_privacy !== null;
  const hasEnvelopeHash = hasLanePrivacy || (value.envelope_hash !== undefined && value.envelope_hash !== null);
  const hasVkCommitment = hasEnvelopeHash || (value.vk_commitment !== undefined && value.vk_commitment !== null);
  if (hasVkCommitment) {
    parts.push(
      encodeNoritoField(
        encodeOptionValue(
          value.vk_commitment,
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
          value.envelope_hash,
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
        encodeOptionValue(value.lane_privacy, encodeLanePrivacyProofValue, `${context}.lane_privacy`),
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
  return {
    backend,
    proof,
    vk_ref,
    vk_commitment,
    envelope_hash,
    lane_privacy,
  };
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
    return encodeEnumTagValue(0, () =>
      encodeStructValue([
        [encodeFixedBytesValue(value.payload.leaf, 32, `${context}.payload.leaf`)],
        [encodeMerkleProofValue(value.payload.proof, `${context}.payload.proof`)],
      ]),
    );
  }
  if (kind === "snark") {
    return encodeEnumTagValue(1, () =>
      encodeStructValue([
        [encodeByteVecValue(value.payload.public_inputs, `${context}.payload.public_inputs`)],
        [encodeByteVecValue(value.payload.proof, `${context}.payload.proof`)],
      ]),
    );
  }
  throw new Error(`${context}.kind must be merkle or snark`);
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
      return {
        kind: "merkle",
        payload: {
          leaf: Array.from(decodeFixedBytesValue(fields.leaf, 32, `${context}.payload.leaf`)),
          proof: decodeMerkleProofValue(fields.proof, `${context}.payload.proof`),
        },
      };
    }
    case 1: {
      const fields = decodeStructFields(body ?? Buffer.alloc(0), `${context}.snark`, [
        "public_inputs",
        "proof",
      ]);
      return {
        kind: "snark",
        payload: {
          public_inputs: Array.from(
            decodeByteVecValue(fields.public_inputs, `${context}.payload.public_inputs`),
          ),
          proof: Array.from(decodeByteVecValue(fields.proof, `${context}.payload.proof`)),
        },
      };
    }
    default:
      throw new Error(`${context} uses unsupported lane privacy witness ${tag}`);
  }
}

function encodeMerkleProofValue(value, context) {
  return encodeTupleValue([
    encodeU32Value(value.leaf_index ?? value.leafIndex, `${context}.leaf_index`),
    encodeNoritoVec(value.audit_path ?? value.auditPath ?? [], (entry, index) =>
      encodeOptionValue(
        entry,
        (item, innerContext) => encodeFixedBytesValue(item, 32, innerContext),
        `${context}.audit_path[${index}]`,
      ),
    ),
  ]);
}

function decodeMerkleProofValue(payload, context) {
  const fields = decodeTupleFields(payload, context, ["leaf_index", "audit_path"]);
  return {
    leaf_index: decodeU32Value(fields.leaf_index, `${context}.leaf_index`),
    audit_path: decodeNoritoVec(
      fields.audit_path,
      (entry, index) =>
        decodeOptionValue(
          entry,
          (item, innerContext) => Array.from(decodeFixedBytesValue(item, 32, innerContext)),
          `${context}.audit_path[${index}]`,
        ),
      `${context}.audit_path`,
    ),
  };
}

function encodeConfidentialEncryptedPayloadValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  const version = encodeU8Value(value.version, `${context}.version`);
  const ephemeral = encodeFixedBytesValue(value.ephemeral_pubkey, 32, `${context}.ephemeral_pubkey`);
  const nonce = encodeFixedBytesValue(value.nonce, 24, `${context}.nonce`);
  const ciphertext = Buffer.from(normalizeFlexibleBytes(value.ciphertext, `${context}.ciphertext`));
  return Buffer.concat([
    version,
    ephemeral,
    nonce,
    encodeCompactLength(ciphertext.length),
    ciphertext,
  ]);
}

function decodeConfidentialEncryptedPayloadValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const version = reader.readU8("version");
  const ephemeral_pubkey = Array.from(reader.readBytes(32, "ephemeral_pubkey"));
  const nonce = Array.from(reader.readBytes(24, "nonce"));
  const [ciphertextLength, lengthBytes] = decodeUnsignedLeb128(
    payload,
    reader.offset,
    `${context}.ciphertext.length`,
  );
  reader.offset += lengthBytes;
  const ciphertext = reader.readBytes(ciphertextLength, "ciphertext");
  reader.assertEof();
  return {
    version,
    ephemeral_pubkey,
    nonce,
    ciphertext: Buffer.from(ciphertext).toString("base64"),
  };
}

const CONTRACT_MANIFEST_KEYS = Object.freeze([
  "seiyaku_name",
  "code_hash",
  "abi_hash",
  "compiler_fingerprint",
  "features_bitmap",
  "access_set_hints",
  "entrypoints",
  "states",
  "error_codes",
  "kotoba",
  "provenance",
]);

function contractManifestSignatureFields(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  assertOnlyObjectKeys(value, CONTRACT_MANIFEST_KEYS, context);
  return [
    [encodeOptionValue(value.seiyaku_name, encodeNoritoStringValue, `${context}.seiyaku_name`)],
    [encodeOptionValue(value.code_hash, encodeHashValue, `${context}.code_hash`)],
    [encodeOptionValue(value.abi_hash, encodeHashValue, `${context}.abi_hash`)],
    [encodeOptionValue(value.compiler_fingerprint, encodeNoritoStringValue, `${context}.compiler_fingerprint`)],
    [encodeOptionValue(value.features_bitmap, encodeU64NumberValue, `${context}.features_bitmap`)],
    [encodeOptionValue(value.access_set_hints, encodeAccessSetHintsValue, `${context}.access_set_hints`)],
    [
      encodeOptionValue(
        value.entrypoints ?? null,
        encodeEntrypointDescriptorsValue,
        `${context}.entrypoints`,
      ),
    ],
    [
      encodeOptionValue(
        value.states ?? null,
        encodeStateDescriptorsValue,
        `${context}.states`,
      ),
    ],
    [
      encodeOptionValue(
        value.error_codes ?? null,
        encodeContractErrorCodeDescriptorsValue,
        `${context}.error_codes`,
      ),
    ],
    [
      encodeOptionValue(
        value.kotoba ?? null,
        encodeKotobaTranslationEntriesValue,
        `${context}.kotoba`,
      ),
    ],
  ];
}

function encodeContractManifestSignaturePayloadValue(value, context) {
  return encodeStructValue(contractManifestSignatureFields(value, context));
}

function encodeContractManifestValue(value, context) {
  return encodeStructValue([
    ...contractManifestSignatureFields(value, context),
    [
      encodeOptionValue(
        value.provenance ?? null,
        encodeManifestProvenanceValue,
        `${context}.provenance`,
      ),
    ],
  ]);
}

function decodeContractManifestValue(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "seiyaku_name",
    "code_hash",
    "abi_hash",
    "compiler_fingerprint",
    "features_bitmap",
    "access_set_hints",
    "entrypoints",
    "states",
    "error_codes",
    "kotoba",
    "provenance",
  ]);
  return {
    seiyaku_name: decodeOptionValue(
      fields.seiyaku_name,
      decodeStringValue,
      `${context}.seiyaku_name`,
    ),
    code_hash: decodeOptionValue(
      fields.code_hash,
      decodeHashValue,
      `${context}.code_hash`,
    ),
    abi_hash: decodeOptionValue(
      fields.abi_hash,
      decodeHashValue,
      `${context}.abi_hash`,
    ),
    compiler_fingerprint: decodeOptionValue(
      fields.compiler_fingerprint,
      decodeStringValue,
      `${context}.compiler_fingerprint`,
    ),
    features_bitmap: decodeOptionValue(
      fields.features_bitmap,
      decodeU64NumberValue,
      `${context}.features_bitmap`,
    ),
    access_set_hints: decodeOptionValue(
      fields.access_set_hints,
      decodeAccessSetHintsValue,
      `${context}.access_set_hints`,
    ),
    entrypoints: decodeOptionValue(
      fields.entrypoints,
      decodeEntrypointDescriptorsValue,
      `${context}.entrypoints`,
    ),
    states: decodeOptionValue(
      fields.states,
      decodeStateDescriptorsValue,
      `${context}.states`,
    ),
    error_codes: decodeOptionValue(
      fields.error_codes,
      decodeContractErrorCodeDescriptorsValue,
      `${context}.error_codes`,
    ),
    kotoba: decodeOptionValue(
      fields.kotoba,
      decodeKotobaTranslationEntriesValue,
      `${context}.kotoba`,
    ),
    provenance: decodeOptionValue(
      fields.provenance,
      decodeManifestProvenanceValue,
      `${context}.provenance`,
    ),
  };
}

function encodeAccessSetHintsValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  assertOnlyObjectKeys(value, [
    "read_keys",
    "write_keys",
    "dynamic_reads",
    "dynamic_writes",
  ], context);
  return encodeStructValue([
    [encodeNoritoVec(value.read_keys ?? [], (entry, index) =>
      encodeNoritoStringValue(assertNonEmptyString(entry, `${context}.read_keys[${index}]`)),
    )],
    [encodeNoritoVec(value.write_keys ?? [], (entry, index) =>
      encodeNoritoStringValue(assertNonEmptyString(entry, `${context}.write_keys[${index}]`)),
    )],
    [encodeNoritoVec(value.dynamic_reads ?? [], (entry, index) =>
      encodeDynamicAccessHintValue(entry, `${context}.dynamic_reads[${index}]`),
    )],
    [encodeNoritoVec(value.dynamic_writes ?? [], (entry, index) =>
      encodeDynamicAccessHintValue(entry, `${context}.dynamic_writes[${index}]`),
    )],
  ]);
}

function decodeAccessSetHintsValue(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "read_keys",
    "write_keys",
    "dynamic_reads",
    "dynamic_writes",
  ]);
  return {
    read_keys: decodeNoritoVec(
      fields.read_keys,
      (entry, index) => decodeStringValue(entry, `${context}.read_keys[${index}]`),
      `${context}.read_keys`,
    ),
    write_keys: decodeNoritoVec(
      fields.write_keys,
      (entry, index) => decodeStringValue(entry, `${context}.write_keys[${index}]`),
      `${context}.write_keys`,
    ),
    dynamic_reads: decodeNoritoVec(
      fields.dynamic_reads,
      (entry, index) =>
        decodeDynamicAccessHintValue(entry, `${context}.dynamic_reads[${index}]`),
      `${context}.dynamic_reads`,
    ),
    dynamic_writes: decodeNoritoVec(
      fields.dynamic_writes,
      (entry, index) =>
        decodeDynamicAccessHintValue(entry, `${context}.dynamic_writes[${index}]`),
      `${context}.dynamic_writes`,
    ),
  };
}

function encodeDynamicAccessHintValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  assertOnlyObjectKeys(value, ["base_key", "key_type", "bound_kind", "max_keys"], context);
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.base_key, `${context}.base_key`))],
    [encodeNoritoStringValue(assertNonEmptyString(value.key_type, `${context}.key_type`))],
    [encodeNoritoStringValue(assertNonEmptyString(value.bound_kind, `${context}.bound_kind`))],
    [encodeU32Value(value.max_keys, `${context}.max_keys`)],
  ]);
}

function decodeDynamicAccessHintValue(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "base_key",
    "key_type",
    "bound_kind",
    "max_keys",
  ]);
  return {
    base_key: decodeStringValue(fields.base_key, `${context}.base_key`),
    key_type: decodeStringValue(fields.key_type, `${context}.key_type`),
    bound_kind: decodeStringValue(fields.bound_kind, `${context}.bound_kind`),
    max_keys: decodeU32Value(fields.max_keys, `${context}.max_keys`),
  };
}

function encodeEntrypointDescriptorsValue(value, context) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array`);
  }
  return encodeNoritoVec(value, (entry, index) =>
    encodeEntrypointDescriptorValue(entry, `${context}[${index}]`),
  );
}

function decodeEntrypointDescriptorsValue(payload, context) {
  return decodeNoritoVec(
    payload,
    (entry, index) => decodeEntrypointDescriptorValue(entry, `${context}[${index}]`),
    context,
  );
}

function encodeEntrypointDescriptorValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  assertOnlyObjectKeys(value, [
    "name",
    "kind",
    "params",
    "argument_schema",
    "return_type",
    "return_schema",
    "permission",
    "read_keys",
    "write_keys",
    "access_hints_complete",
    "access_hints_skipped",
    "triggers",
  ], context);
  const triggers = value.triggers ?? [];
  if (!Array.isArray(triggers)) {
    throw new TypeError(`${context}.triggers must be an array`);
  }
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.name, `${context}.name`))],
    [encodeEntryPointKindValue(value.kind, `${context}.kind`)],
    [
      encodeNoritoVec(value.params ?? [], (param, index) =>
        encodeEntrypointParamDescriptorValue(param, `${context}.params[${index}]`),
      ),
    ],
    [
      encodeOptionValue(
        value.argument_schema ?? null,
        encodeEntrypointArgumentSchemaValue,
        `${context}.argument_schema`,
      ),
    ],
    [
      encodeOptionValue(
        value.return_type ?? null,
        encodeNoritoStringValue,
        `${context}.return_type`,
      ),
    ],
    [
      encodeOptionValue(
        value.return_schema ?? null,
        encodeEntrypointValueTypeValue,
        `${context}.return_schema`,
      ),
    ],
    [
      encodeOptionValue(
        value.permission ?? null,
        encodeNoritoStringValue,
        `${context}.permission`,
      ),
    ],
    [
      encodeNoritoVec(value.read_keys ?? [], (entry, index) =>
        encodeNoritoStringValue(
          assertNonEmptyString(entry, `${context}.read_keys[${index}]`),
        ),
      ),
    ],
    [
      encodeNoritoVec(value.write_keys ?? [], (entry, index) =>
        encodeNoritoStringValue(
          assertNonEmptyString(entry, `${context}.write_keys[${index}]`),
        ),
      ),
    ],
    [
      encodeOptionValue(
        value.access_hints_complete ?? null,
        encodeBoolValue,
        `${context}.access_hints_complete`,
      ),
    ],
    [
      encodeNoritoVec(
        value.access_hints_skipped ?? [],
        (entry, index) =>
          encodeNoritoStringValue(
            assertNonEmptyString(entry, `${context}.access_hints_skipped[${index}]`),
          ),
      ),
    ],
    [
      encodeNoritoVec(triggers, (entry, index) =>
        encodeManifestTriggerDescriptorValue(entry, `${context}.triggers[${index}]`),
      ),
    ],
  ]);
}

function decodeEntrypointDescriptorValue(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "name",
    "kind",
    "params",
    "argument_schema",
    "return_type",
    "return_schema",
    "permission",
    "read_keys",
    "write_keys",
    "access_hints_complete",
    "access_hints_skipped",
    "triggers",
  ]);
  return {
    name: decodeStringValue(fields.name, `${context}.name`),
    kind: decodeEntryPointKindValue(fields.kind, `${context}.kind`),
    params: decodeNoritoVec(
      fields.params,
      (entry, index) => decodeEntrypointParamDescriptorValue(entry, `${context}.params[${index}]`),
      `${context}.params`,
    ),
    argument_schema: decodeOptionValue(
      fields.argument_schema,
      decodeEntrypointArgumentSchemaValue,
      `${context}.argument_schema`,
    ),
    return_type: decodeOptionValue(
      fields.return_type,
      decodeStringValue,
      `${context}.return_type`,
    ),
    return_schema: decodeOptionValue(
      fields.return_schema,
      decodeEntrypointValueTypeValue,
      `${context}.return_schema`,
    ),
    permission: decodeOptionValue(
      fields.permission,
      decodeStringValue,
      `${context}.permission`,
    ),
    read_keys: decodeNoritoVec(
      fields.read_keys,
      (entry, index) => decodeStringValue(entry, `${context}.read_keys[${index}]`),
      `${context}.read_keys`,
    ),
    write_keys: decodeNoritoVec(
      fields.write_keys,
      (entry, index) => decodeStringValue(entry, `${context}.write_keys[${index}]`),
      `${context}.write_keys`,
    ),
    access_hints_complete: decodeOptionValue(
      fields.access_hints_complete,
      decodeBoolValue,
      `${context}.access_hints_complete`,
    ),
    access_hints_skipped: decodeNoritoVec(
      fields.access_hints_skipped,
      (entry, index) =>
        decodeStringValue(entry, `${context}.access_hints_skipped[${index}]`),
      `${context}.access_hints_skipped`,
    ),
    triggers: decodeNoritoVec(
      fields.triggers,
      (entry, index) =>
        decodeManifestTriggerDescriptorValue(entry, `${context}.triggers[${index}]`),
      `${context}.triggers`,
    ),
  };
}

function encodeEntryPointKindValue(value, context) {
  const kind = typeof value === "string" ? value : value?.kind;
  const normalized = assertNonEmptyString(kind, context).toLowerCase();
  switch (normalized) {
    case "kotoage":
      return encodeEnumTagValue(0);
    case "view":
      return encodeEnumTagValue(1);
    case "hajimari":
      return encodeEnumTagValue(2);
    case "kaizen":
      return encodeEnumTagValue(3);
    default:
      throw new Error(`${context} must be Kotoage, View, Hajimari, or Kaizen`);
  }
}

function decodeEntryPointKindValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("tag");
  reader.assertEof();
  switch (tag) {
    case 0:
      return { kind: "Kotoage", value: null };
    case 1:
      return { kind: "View", value: null };
    case 2:
      return { kind: "Hajimari", value: null };
    case 3:
      return { kind: "Kaizen", value: null };
    default:
      throw new Error(`${context} uses unsupported entrypoint kind ${tag}`);
  }
}

function encodeEntrypointParamDescriptorValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  assertOnlyObjectKeys(value, ["name", "type_name"], context);
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.name, `${context}.name`))],
    [encodeNoritoStringValue(assertNonEmptyString(value.type_name, `${context}.type_name`))],
  ]);
}

function decodeEntrypointParamDescriptorValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["name", "type_name"]);
  return {
    name: decodeStringValue(fields.name, `${context}.name`),
    type_name: decodeStringValue(fields.type_name, `${context}.type_name`),
  };
}

function encodeEntrypointArgumentSchemaValue(value, context) {
  if (!isPlainObject(value) || !Array.isArray(value.fields)) {
    throw new TypeError(`${context} must contain a fields array`);
  }
  assertOnlyObjectKeys(value, ["fields"], context);
  return encodeStructValue([
    [
      encodeNoritoVec(value.fields, (field, index) =>
        encodeEntrypointArgumentFieldValue(field, `${context}.fields[${index}]`),
      ),
    ],
  ]);
}

function decodeEntrypointArgumentSchemaValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["fields"]);
  return {
    fields: decodeNoritoVec(
      fields.fields,
      (field, index) =>
        decodeEntrypointArgumentFieldValue(field, `${context}.fields[${index}]`),
      `${context}.fields`,
    ),
  };
}

function encodeEntrypointArgumentFieldValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  assertOnlyObjectKeys(value, ["name", "ty"], context);
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.name, `${context}.name`))],
    [encodeEntrypointValueTypeValue(value.ty, `${context}.ty`)],
  ]);
}

function decodeEntrypointArgumentFieldValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["name", "ty"]);
  return {
    name: decodeStringValue(fields.name, `${context}.name`),
    ty: decodeEntrypointValueTypeValue(fields.ty, `${context}.ty`),
  };
}

function encodeEntrypointValueTypeValue(value, context) {
  if (!isPlainObject(value) || !Array.isArray(value.nodes)) {
    throw new TypeError(`${context} must contain a nodes array`);
  }
  assertOnlyObjectKeys(value, ["nodes"], context);
  analyzeEntrypointValueTypeV1(value, context);
  return encodeStructValue([
    [
      encodeNoritoVec(value.nodes, (node, index) =>
        encodeEntrypointValueTypeNodeValue(node, `${context}.nodes[${index}]`),
      ),
    ],
  ]);
}

function decodeEntrypointValueTypeValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["nodes"]);
  const value = {
    nodes: decodeNoritoVec(
      fields.nodes,
      (node, index) =>
        decodeEntrypointValueTypeNodeValue(node, `${context}.nodes[${index}]`),
      `${context}.nodes`,
    ),
  };
  analyzeEntrypointValueTypeV1(value, context);
  return value;
}

function taggedEnumParts(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be a tagged object`);
  }
  assertOnlyObjectKeys(value, ["kind", "value"], context);
  return {
    kind: assertNonEmptyString(value.kind, `${context}.kind`),
    value: value.value ?? null,
  };
}

function encodeEntrypointValueTypeNodeValue(value, context) {
  const tagged = taggedEnumParts(value, context);
  switch (tagged.kind) {
    case "Struct":
      return encodeEnumTagValue(0, () =>
        encodeEntrypointStructTypeNodeValue(tagged.value, `${context}.value`),
      );
    case "Tuple":
      return encodeEnumTagValue(1, () =>
        encodeU16Value(tagged.value, `${context}.value`),
      );
    case "Option":
      requireNullEnumPayload(tagged.value, context);
      return encodeEnumTagValue(2);
    case "Result":
      requireNullEnumPayload(tagged.value, context);
      return encodeEnumTagValue(3);
    case "List":
      return encodeEnumTagValue(4, () =>
        encodeEntrypointListTypeNodeValue(tagged.value, `${context}.value`),
      );
    case "Leaf":
      return encodeEnumTagValue(5, () =>
        encodeEntrypointValueKindValue(tagged.value, `${context}.value`),
      );
    default:
      throw new Error(`${context}.kind uses unsupported value-type node ${tagged.kind}`);
  }
}

function decodeEntrypointValueTypeNodeValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("tag");
  switch (tag) {
    case 0:
      return {
        kind: "Struct",
        value: decodeEntrypointStructTypeNodeValue(
          readSingleEnumPayload(reader, context),
          `${context}.value`,
        ),
      };
    case 1:
      return {
        kind: "Tuple",
        value: decodeU16Value(readSingleEnumPayload(reader, context), `${context}.value`),
      };
    case 2:
      reader.assertEof();
      return { kind: "Option", value: null };
    case 3:
      reader.assertEof();
      return { kind: "Result", value: null };
    case 4:
      return {
        kind: "List",
        value: decodeEntrypointListTypeNodeValue(
          readSingleEnumPayload(reader, context),
          `${context}.value`,
        ),
      };
    case 5:
      return {
        kind: "Leaf",
        value: decodeEntrypointValueKindValue(
          readSingleEnumPayload(reader, context),
          `${context}.value`,
        ),
      };
    default:
      throw new Error(`${context} uses unsupported value-type node tag ${tag}`);
  }
}

function readSingleEnumPayload(reader, context) {
  const value = readNoritoField(reader, "value");
  reader.assertEof();
  return value;
}

function requireNullEnumPayload(value, context) {
  if (value !== null && value !== undefined) {
    throw new TypeError(`${context}.value must be null for a unit variant`);
  }
}

function encodeEntrypointStructTypeNodeValue(value, context) {
  if (!isPlainObject(value) || !Array.isArray(value.fields)) {
    throw new TypeError(`${context} must contain a fields array`);
  }
  assertOnlyObjectKeys(value, ["name", "fields"], context);
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.name, `${context}.name`))],
    [
      encodeNoritoVec(value.fields, (field, index) =>
        encodeNoritoStringValue(
          assertNonEmptyString(field, `${context}.fields[${index}]`),
        ),
      ),
    ],
  ]);
}

function decodeEntrypointStructTypeNodeValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["name", "fields"]);
  return {
    name: decodeStringValue(fields.name, `${context}.name`),
    fields: decodeNoritoVec(
      fields.fields,
      (field, index) => decodeStringValue(field, `${context}.fields[${index}]`),
      `${context}.fields`,
    ),
  };
}

function encodeEntrypointListTypeNodeValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  assertOnlyObjectKeys(value, ["capacity"], context);
  return encodeStructValue([
    [encodeU8Value(value.capacity, `${context}.capacity`)],
  ]);
}

function decodeEntrypointListTypeNodeValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["capacity"]);
  return {
    capacity: decodeU8Value(fields.capacity, `${context}.capacity`),
  };
}

const ENTRYPOINT_VALUE_KIND_NAMES = Object.freeze([
  "Int",
  "Decimal",
  "Quantity",
  "Bool",
  "String",
  "Json",
  "Name",
  "AccountId",
  "AssetDefinitionId",
  "AssetId",
  "DomainId",
  "NftId",
  "DataSpaceId",
  "Blob",
]);

function encodeEntrypointValueKindValue(value, context) {
  const tagged = taggedEnumParts(value, context);
  requireNullEnumPayload(tagged.value, context);
  const tag = ENTRYPOINT_VALUE_KIND_NAMES.indexOf(tagged.kind);
  if (tag < 0) {
    throw new Error(`${context}.kind uses unsupported value kind ${tagged.kind}`);
  }
  return encodeEnumTagValue(tag);
}

function decodeEntrypointValueKindValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("tag");
  reader.assertEof();
  const kind = ENTRYPOINT_VALUE_KIND_NAMES[tag];
  if (kind === undefined) {
    throw new Error(`${context} uses unsupported value-kind tag ${tag}`);
  }
  return { kind, value: null };
}

function encodeStateDescriptorsValue(value, context) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array`);
  }
  return encodeNoritoVec(value, (entry, index) =>
    encodeStateDescriptorValue(entry, `${context}[${index}]`),
  );
}

function decodeStateDescriptorsValue(payload, context) {
  return decodeNoritoVec(
    payload,
    (entry, index) => decodeStateDescriptorValue(entry, `${context}[${index}]`),
    context,
  );
}

function encodeStateDescriptorValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  assertOnlyObjectKeys(value, ["name", "type_name"], context);
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.name, `${context}.name`))],
    [
      encodeNoritoStringValue(
        assertNonEmptyString(value.type_name, `${context}.type_name`),
      ),
    ],
  ]);
}

function decodeStateDescriptorValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["name", "type_name"]);
  return {
    name: decodeStringValue(fields.name, `${context}.name`),
    type_name: decodeStringValue(fields.type_name, `${context}.type_name`),
  };
}

function encodeContractErrorCodeDescriptorsValue(value, context) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array`);
  }
  return encodeNoritoVec(value, (entry, index) =>
    encodeContractErrorCodeDescriptorValue(entry, `${context}[${index}]`),
  );
}

function decodeContractErrorCodeDescriptorsValue(payload, context) {
  return decodeNoritoVec(
    payload,
    (entry, index) =>
      decodeContractErrorCodeDescriptorValue(entry, `${context}[${index}]`),
    context,
  );
}

function encodeContractErrorCodeDescriptorValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  assertOnlyObjectKeys(value, ["namespace", "name", "code"], context);
  return encodeStructValue([
    [
      encodeNoritoStringValue(
        assertNonEmptyString(value.namespace, `${context}.namespace`),
      ),
    ],
    [encodeNoritoStringValue(assertNonEmptyString(value.name, `${context}.name`))],
    [encodeU32Value(value.code, `${context}.code`)],
  ]);
}

function decodeContractErrorCodeDescriptorValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["namespace", "name", "code"]);
  return {
    namespace: decodeStringValue(fields.namespace, `${context}.namespace`),
    name: decodeStringValue(fields.name, `${context}.name`),
    code: decodeU32Value(fields.code, `${context}.code`),
  };
}

function encodeKotobaTranslationEntriesValue(value, context) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array`);
  }
  return encodeNoritoVec(value, (entry, index) =>
    encodeKotobaTranslationEntryValue(entry, `${context}[${index}]`),
  );
}

function decodeKotobaTranslationEntriesValue(payload, context) {
  return decodeNoritoVec(
    payload,
    (entry, index) => decodeKotobaTranslationEntryValue(entry, `${context}[${index}]`),
    context,
  );
}

function encodeKotobaTranslationEntryValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  assertOnlyObjectKeys(value, ["msg_id", "translations"], context);
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.msg_id, `${context}.msg_id`))],
    [
      encodeNoritoVec(value.translations ?? [], (entry, index) =>
        encodeKotobaTranslationValue(entry, `${context}.translations[${index}]`),
      ),
    ],
  ]);
}

function decodeKotobaTranslationEntryValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["msg_id", "translations"]);
  return {
    msg_id: decodeStringValue(fields.msg_id, `${context}.msg_id`),
    translations: decodeNoritoVec(
      fields.translations,
      (entry, index) => decodeKotobaTranslationValue(entry, `${context}.translations[${index}]`),
      `${context}.translations`,
    ),
  };
}

function encodeKotobaTranslationValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  assertOnlyObjectKeys(value, ["lang", "text"], context);
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.lang, `${context}.lang`))],
    [encodeStringValue(value.text, `${context}.text`)],
  ]);
}

function decodeKotobaTranslationValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["lang", "text"]);
  return {
    lang: decodeStringValue(fields.lang, `${context}.lang`),
    text: decodeStringValue(fields.text, `${context}.text`),
  };
}

function encodeManifestProvenanceValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  assertOnlyObjectKeys(value, ["signer", "signature"], context);
  const signer = parsePublicKeyLiteral(value.signer, `${context}.signer`);
  const signatureLiteral = assertNonEmptyString(value.signature, `${context}.signature`);
  if (
    signatureLiteral.length % 2 !== 0 ||
    !/^[0-9A-Fa-f]+$/u.test(signatureLiteral)
  ) {
    throw new Error(`${context}.signature must be an even-length hexadecimal string`);
  }
  const signature = Buffer.from(signatureLiteral, "hex");
  validateManifestSignatureBytes(signature, `${context}.signature`);
  return encodeStructValue([
    [encodePublicKeyValue(signer, `${context}.signer`)],
    [encodeConstVecU8Value(signature)],
  ]);
}

function decodeManifestProvenanceValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["signer", "signature"]);
  const signer = decodePublicKeyValue(fields.signer, `${context}.signer`);
  const signature = decodeConstVecU8Value(fields.signature, `${context}.signature`);
  validateManifestSignatureBytes(signature, `${context}.signature`);
  return {
    signer: publicKeyLiteralFromParts(
      signer.curve,
      signer.publicKey,
      `${context}.signer`,
    ),
    signature: signature.toString("hex").toUpperCase(),
  };
}

function validateManifestSignatureBytes(signature, context) {
  if (signature.length === 0) {
    throw new Error(`${context} must not be empty`);
  }
  if (signature.every((byte) => byte === 0)) {
    throw new Error(`${context} must not be all zero`);
  }
}

function encodeManifestTriggerDescriptorValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  assertOnlyObjectKeys(value, [
    "id",
    "repeats",
    "filter",
    "authority",
    "metadata",
    "callback",
  ], context);
  return encodeStructValue([
    [encodeTriggerIdValue(value.id, `${context}.id`)],
    [encodeTriggerRepeatsValue(value.repeats, `${context}.repeats`)],
    [encodeEventFilterBoxFramePayload(value.filter, `${context}.filter`)],
    [
      encodeOptionValue(
        value.authority ?? null,
        encodeAccountIdValue,
        `${context}.authority`,
      ),
    ],
    [encodeMetadataValue(value.metadata ?? {}, `${context}.metadata`)],
    [encodeTriggerCallbackValue(value.callback, `${context}.callback`)],
  ]);
}

function decodeManifestTriggerDescriptorValue(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "id",
    "repeats",
    "filter",
    "authority",
    "metadata",
    "callback",
  ]);
  return {
    id: decodeTriggerIdValue(fields.id, `${context}.id`),
    repeats: decodeTriggerRepeatsValue(fields.repeats, `${context}.repeats`),
    filter: decodeEventFilterBoxFramePayload(fields.filter, `${context}.filter`),
    authority: decodeOptionValue(
      fields.authority,
      decodeAccountIdValue,
      `${context}.authority`,
    ),
    metadata: decodeMetadataValue(fields.metadata, `${context}.metadata`),
    callback: decodeTriggerCallbackValue(fields.callback, `${context}.callback`),
  };
}

function encodeTriggerIdValue(value, context) {
  return encodeStructValue([
    [encodeNameValue(assertNonEmptyString(value, context), `${context}.name`)],
  ]);
}

function decodeTriggerIdValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["name"]);
  return decodeNameValue(fields.name, `${context}.name`);
}

function encodeTriggerRepeatsValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(
      `${context} must be {Indefinitely:null} or {Exactly:<u32>}`,
    );
  }
  const keys = Object.keys(value);
  if (keys.length !== 1) {
    throw new TypeError(`${context} must contain exactly one repeat variant`);
  }
  if (keys[0] === "Indefinitely") {
    requireNullEnumPayload(value.Indefinitely, context);
    return encodeEnumTagValue(0);
  }
  if (keys[0] === "Exactly") {
    return encodeEnumTagValue(1, () =>
      encodeU32Value(value.Exactly, `${context}.Exactly`),
    );
  }
  throw new Error(`${context} uses unsupported repeat variant ${keys[0]}`);
}

function decodeTriggerRepeatsValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("tag");
  if (tag === 0) {
    reader.assertEof();
    return { Indefinitely: null };
  }
  if (tag === 1) {
    return {
      Exactly: decodeU32Value(
        readSingleEnumPayload(reader, context),
        `${context}.Exactly`,
      ),
    };
  }
  throw new Error(`${context} uses unsupported repeat tag ${tag}`);
}

function encodeTriggerCallbackValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  assertOnlyObjectKeys(value, ["namespace", "entrypoint"], context);
  return encodeStructValue([
    [
      encodeOptionValue(
        value.namespace ?? null,
        encodeNoritoStringValue,
        `${context}.namespace`,
      ),
    ],
    [
      encodeNoritoStringValue(
        assertNonEmptyString(value.entrypoint, `${context}.entrypoint`),
      ),
    ],
  ]);
}

function decodeTriggerCallbackValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["namespace", "entrypoint"]);
  return {
    namespace: decodeOptionValue(
      fields.namespace,
      decodeStringValue,
      `${context}.namespace`,
    ),
    entrypoint: decodeStringValue(fields.entrypoint, `${context}.entrypoint`),
  };
}

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
  ).toString("base64");
}

function decodeExactStandardBase64(value, context) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.trim() !== value ||
    value.length % 4 !== 0 ||
    !/^[A-Za-z0-9+/]*={0,2}$/u.test(value)
  ) {
    throw new TypeError(`${context} must be exact standard-base64`);
  }
  const bytes = Buffer.from(value, "base64");
  if (bytes.length === 0 || bytes.toString("base64") !== value) {
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
  return encodeNoritoField(Buffer.from(value, "utf8"));
}

function encodeExactBase64StringValue(value, context) {
  if (typeof value !== "string") {
    throw new TypeError(`${context} must be a string`);
  }
  if (value.length === 0 || value.trim() !== value || /\s/u.test(value)) {
    throw new TypeError(`${context} must be exact standard-base64`);
  }
  if (!/^[A-Za-z0-9+/]*={0,2}$/u.test(value) || value.length % 4 !== 0) {
    throw new TypeError(`${context} must be exact standard-base64`);
  }
  const decoded = Buffer.from(value, "base64");
  if (decoded.length === 0 || decoded.toString("base64") !== value) {
    throw new TypeError(`${context} must be exact standard-base64`);
  }
  return encodeNoritoStringValue(value);
}

function decodeStringValue(payload, context, lengthFlags = noritoLengthFlags) {
  const reader = new BufferReader(payload, context, lengthFlags);
  const stringBytes = readNoritoField(reader, "value");
  reader.assertEof();
  return stringBytes.toString("utf8");
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

function decodeNoritoVec(payload, decode, context) {
  const reader = new BufferReader(payload, context, noritoLengthFlags);
  const count = bigintToSafeNumber(reader.readU64LE("count"), `${context}.count`);
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
      Buffer.from("norito:v1:type-name\0", "utf8"),
      Buffer.from(typeName, "utf8"),
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
      typeof options.expectedTypeName !== "string" ||
      options.expectedTypeName.length === 0
    ) {
      throw new TypeError(`${context} expected Rust type name must be non-empty`);
    }
    const fromTypeName = schemaHashForTypeName(options.expectedTypeName);
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
  const actualCrc = crc64Ecma(payload);
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
    u64ToLittleEndianBuffer(crc64Ecma(payload)),
    Buffer.from([flags & 0xff]),
  ]);
  return Buffer.concat([header, Buffer.alloc(padding), payload]);
}

function crc64Ecma(payload) {
  let crc = UINT64_MASK;
  for (const byte of payload) {
    const index = Number((crc ^ BigInt(byte)) & 0xffn);
    crc = CRC64_TABLE[index] ^ (crc >> 8n);
  }
  return BigInt.asUintN(64, crc ^ UINT64_MASK);
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
  if (typeof value === "bigint") {
    if (value < 0n || value > UINT64_MASK) {
      throw new RangeError(`${context} must fit in an unsigned 64-bit integer`);
    }
    return value;
  }
  if (typeof value === "number") {
    if (!Number.isInteger(value) || value < 0 || !Number.isSafeInteger(value)) {
      throw new TypeError(`${context} must be a non-negative safe integer or bigint`);
    }
    return BigInt(value);
  }
  if (typeof value === "string" && /^\d+$/.test(value.trim())) {
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
  } else if (typeof value === "string") {
    quantity = NumericV1.decodeQuantityJson(value);
  } else if (typeof value === "bigint") {
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
  ]).toString("hex");
  return `${prefixHex}${bytes.toString("hex").toUpperCase()}`;
}

function parsePublicKeyLiteral(literal, context) {
  const normalized = assertNonEmptyString(literal, context);
  if (!MULTIHASH_LITERAL_RE.test(normalized) || normalized.length % 2 !== 0) {
    throw new Error(`${context} must be a canonical public-key multihash literal`);
  }
  const bytes = Buffer.from(normalized, "hex");
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
  while (cursor < buffer.length) {
    const byte = BigInt(buffer[cursor]);
    cursor += 1;
    value |= (byte & 0x7fn) << shift;
    if ((byte & 0x80n) === 0n) {
      return [Number(value), cursor - offset];
    }
    shift += 7n;
  }
  throw new Error(`${context} varint is truncated`);
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

function computeHashLiteralCrc(tag, body) {
  let crc = 0xffff;
  const processByte = (byte) => {
    crc ^= (byte & 0xff) << 8;
    for (let i = 0; i < 8; i += 1) {
      if ((crc & 0x8000) !== 0) {
        crc = ((crc << 1) ^ 0x1021) & 0xffff;
      } else {
        crc = (crc << 1) & 0xffff;
      }
    }
  };
  for (const byte of Buffer.from(tag, "utf8")) {
    processByte(byte);
  }
  processByte(":".charCodeAt(0));
  for (const byte of Buffer.from(body, "utf8")) {
    processByte(byte);
  }
  return (crc & 0xffff).toString(16).toUpperCase().padStart(4, "0");
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
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new TypeError(`${context} must be a non-empty string`);
  }
  return value.trim();
}

function assertExactNonEmptyString(value, context) {
  if (typeof value !== "string" || value.length === 0) {
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
  const message = error && typeof error.message === "string" ? error.message : "";
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
    const decoded = Buffer.from(compact, "base64");
    if (decoded.length === 0) {
      return null;
    }
    if (decoded.toString("base64") !== compact) {
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
    const decoded = Buffer.from(compact, "hex");
    return decoded.length > 0 ? decoded : null;
  } catch {
    return null;
  }
}

function tryDecodeWithRelocatedStorage(native, buffer) {
  const extra = ALIGNMENT - 1;
  const constructors = [];
  if (typeof SharedArrayBuffer === "function") {
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
