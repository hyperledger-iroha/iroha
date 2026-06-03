import { Buffer } from "node:buffer";
import { blake3 } from "@noble/hashes/blake3";
import { sha256 } from "@noble/hashes/sha2";
import {
  AccountAddress,
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

const ALIGNMENT = 16;
const COMPACT_LEN_FLAG = 0x02;
const UINT64_MASK = 0xffff_ffff_ffff_ffffn;
const CRC64_REFLECTED_POLY = 0xc96c5795d7870f42n;
const ASSET_DEFINITION_ADDRESS_VERSION = 1;
const BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const UINT128_MASK = (1n << 128n) - 1n;
const HASH_LITERAL_RE = /^hash:([0-9A-Fa-f]{64})#([0-9A-Fa-f]{4})$/;
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
  "RecordSccpMessage",
];
const RECORD_SCCP_MESSAGE_WIRE_ID =
  "iroha_data_model::isi::bridge::RecordSccpMessage";
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
const INNER_SCHEMA_HASH_BY_WIRE_ID = Object.freeze({
  "iroha.mint": Buffer.from("ec0b538ed0e5b46ed163e0aedb335e73", "hex"),
  "iroha.burn": Buffer.from("361f279124a0aad61978c80ff1c9ce0a", "hex"),
  "iroha.register": Buffer.from("2e9fa44b44ac5295a0b34e05edcb4133", "hex"),
  "iroha.transfer": Buffer.from("a4174c78d6341f8f98fc2adae8ed67b9", "hex"),
  "iroha.custom": Buffer.from("6b86902a75600648d186d52cd662b229", "hex"),
  "iroha.execute_trigger": Buffer.from(
    "d8988afd2c1dee721564dd8d57841eff",
    "hex",
  ),
  "iroha.rwa": Buffer.from("4a07cd02fdfb5fe81043a1ba7bf72123", "hex"),
  [RECORD_SCCP_MESSAGE_WIRE_ID]: Buffer.from(
    "d89e5307d9c06f39f39086ffff9fc5d0",
    "hex",
  ),
  "iroha_data_model::isi::kaigi::CreateKaigi": Buffer.from(
    "24ee2ad1d6a56d3524ee2ad1d6a56d35",
    "hex",
  ),
  "iroha_data_model::isi::kaigi::JoinKaigi": Buffer.from(
    "5077ea3be6f706825077ea3be6f70682",
    "hex",
  ),
  "iroha_data_model::isi::kaigi::LeaveKaigi": Buffer.from(
    "d74b8812a0a2681cd74b8812a0a2681c",
    "hex",
  ),
  "iroha_data_model::isi::kaigi::EndKaigi": Buffer.from(
    "85befda0409d3c0485befda0409d3c04",
    "hex",
  ),
  "iroha_data_model::isi::kaigi::RecordKaigiUsage": Buffer.from(
    "e20fb919a4056c21e20fb919a4056c21",
    "hex",
  ),
  "iroha_data_model::isi::kaigi::SetKaigiRelayManifest": Buffer.from(
    "726dd6413d1d2b01726dd6413d1d2b01",
    "hex",
  ),
  "iroha_data_model::isi::kaigi::RegisterKaigiRelay": Buffer.from(
    "b40e80079720b8a2b40e80079720b8a2",
    "hex",
  ),
  "iroha_data_model::isi::governance::ProposeDeployContract": Buffer.from(
    "d92fab6392e8299fd92fab6392e8299f",
    "hex",
  ),
  "iroha_data_model::isi::governance::CastZkBallot": Buffer.from(
    "58d9049c2c73912958d9049c2c739129",
    "hex",
  ),
  "iroha_data_model::isi::governance::CastPlainBallot": Buffer.from(
    "9969f69b4a99a0749969f69b4a99a074",
    "hex",
  ),
  "iroha_data_model::isi::governance::EnactReferendum": Buffer.from(
    "564da81425d228de564da81425d228de",
    "hex",
  ),
  "iroha_data_model::isi::governance::FinalizeReferendum": Buffer.from(
    "316f68c14913465e316f68c14913465e",
    "hex",
  ),
  "iroha_data_model::isi::governance::PersistCouncilForEpoch": Buffer.from(
    "25f004fc72a647fa25f004fc72a647fa",
    "hex",
  ),
  "iroha_data_model::isi::social::ClaimTwitterFollowReward": Buffer.from(
    "9c61d408efe778839c61d408efe77883",
    "hex",
  ),
  "iroha_data_model::isi::social::SendToTwitter": Buffer.from(
    "a1aef2203c4f83cda1aef2203c4f83cd",
    "hex",
  ),
  "iroha_data_model::isi::social::CancelTwitterEscrow": Buffer.from(
    "31c358e3880dbffe31c358e3880dbffe",
    "hex",
  ),
  "iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode": Buffer.from(
    "63eec8b1a5dfcb1263eec8b1a5dfcb12",
    "hex",
  ),
  "iroha_data_model::isi::smart_contract_code::RegisterSmartContractBytes": Buffer.from(
    "458b53cef6502236458b53cef6502236",
    "hex",
  ),
  "iroha_data_model::isi::smart_contract_code::DeactivateContractInstance": Buffer.from(
    "351293113eec3144351293113eec3144",
    "hex",
  ),
  "iroha_data_model::isi::smart_contract_code::ActivateContractInstance": Buffer.from(
    "829e0d2a934213bf829e0d2a934213bf",
    "hex",
  ),
  "iroha_data_model::isi::smart_contract_code::RemoveSmartContractBytes": Buffer.from(
    "645fa1f41c603c82645fa1f41c603c82",
    "hex",
  ),
  "iroha_data_model::isi::zk::RegisterZkAsset": Buffer.from(
    "5d14a5ea7a6d1c255d14a5ea7a6d1c25",
    "hex",
  ),
  "iroha_data_model::isi::zk::RegisterAssetHiddenZkPool": Buffer.from(
    "3b18eae8897ad15fabaafa03b589e243",
    "hex",
  ),
  "iroha_data_model::isi::zk::RegisterZkAceIdentityCommitment": schemaHashForTypeName(
    "iroha_data_model::isi::zk::RegisterZkAceIdentityCommitment",
  ),
  "iroha_data_model::isi::zk::RotateZkAceIdentityCommitment": schemaHashForTypeName(
    "iroha_data_model::isi::zk::RotateZkAceIdentityCommitment",
  ),
  "iroha_data_model::isi::zk::RevokeZkAceIdentityCommitment": schemaHashForTypeName(
    "iroha_data_model::isi::zk::RevokeZkAceIdentityCommitment",
  ),
  "zk::ScheduleConfidentialPolicyTransition": Buffer.from(
    "836fd710eab04142836fd710eab04142",
    "hex",
  ),
  "zk::CancelConfidentialPolicyTransition": Buffer.from(
    "c8b4798fe99aba33c8b4798fe99aba33",
    "hex",
  ),
  "iroha_data_model::isi::zk::Shield": Buffer.from(
    "644a69b3e27c574b644a69b3e27c574b",
    "hex",
  ),
  "iroha_data_model::isi::zk::ZkTransfer": Buffer.from(
    "a54e2391aea3a8b6a54e2391aea3a8b6",
    "hex",
  ),
  "iroha_data_model::isi::zk::AssetHiddenZkTransfer": Buffer.from(
    "db10e28def5ce4715a0a20eff60259fc",
    "hex",
  ),
  "iroha_data_model::isi::zk::SubmitZkAceAuthorizedTransfer": schemaHashForTypeName(
    "iroha_data_model::isi::zk::SubmitZkAceAuthorizedTransfer",
  ),
  "iroha_data_model::isi::zk::Unshield": Buffer.from(
    "eb6a8611ac89d632eb6a8611ac89d632",
    "hex",
  ),
  "iroha_data_model::isi::zk::CreateElection": Buffer.from(
    "6612c94b6f84c9cb6612c94b6f84c9cb",
    "hex",
  ),
  "iroha_data_model::isi::zk::SubmitBallot": Buffer.from(
    "4319232398af7d414319232398af7d41",
    "hex",
  ),
  "iroha_data_model::isi::zk::FinalizeElection": Buffer.from(
    "9cd931a79ced1cb69cd931a79ced1cb6",
    "hex",
  ),
  "iroha_data_model::isi::verifying_keys::RegisterVerifyingKey": schemaHashForTypeName(
    "iroha_data_model::isi::verifying_keys::RegisterVerifyingKey",
  ),
  "iroha_data_model::isi::verifying_keys::UpdateVerifyingKey": schemaHashForTypeName(
    "iroha_data_model::isi::verifying_keys::UpdateVerifyingKey",
  ),
});
const INNER_HEADER_PADDING_BY_WIRE_ID = Object.freeze({
  "iroha_data_model::isi::governance::CastPlainBallot": 8,
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
let forcePureJsInstructionCodec = false;

class BufferReader {
  constructor(buffer, context, lengthFlags = 0) {
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
  const native = getNativeBinding();
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
    message.includes("(not registered)") ||
    message.includes("instruction payload must use canonical Norito framing")
  );
}

function shouldUsePureJsInstructionFallback(error) {
  return isNativeBindingUnavailable(error) || isNativeBindingUnsupportedInstruction(error);
}

function encodeNormalizedInstruction(normalized) {
  let encoded;
  if (forcePureJsInstructionCodec) {
    encoded = encodePureJsInstruction(normalized);
    cacheInstructionRoundTrip(encoded, normalized);
    return encoded;
  }
  try {
    const native = resolveNative("noritoEncodeInstruction");
    encoded = native.noritoEncodeInstruction(JSON.stringify(normalized));
  } catch (error) {
    if (!shouldUsePureJsInstructionFallback(error)) {
      throw error;
    }
    try {
      encoded = encodePureJsInstruction(normalized);
      if (isNativeBindingUnsupportedInstruction(error)) {
        forcePureJsInstructionCodec = true;
      }
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
  return message.startsWith("Internal Norito canonicalization supports ");
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
          encodeNoritoStringValue,
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
        encodeOptionValue(
          request.fee_sponsor ?? request.feeSponsor ?? null,
          encodeNoritoStringValue,
          "MultisigProposeDto.fee_sponsor",
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
    ]),
  );
  return frameNoritoPayload(payload, MULTISIG_PROPOSE_DTO_SCHEMA_HASH, COMPACT_LEN_FLAG);
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
          encodeNoritoStringValue,
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
        encodeOptionValue(
          request.gas_asset_id ?? request.gasAssetId ?? null,
          encodeNoritoStringValue,
          "MultisigContractCallProposeDto.gas_asset_id",
        ),
      ],
      [
        encodeOptionValue(
          request.fee_sponsor ?? request.feeSponsor ?? null,
          encodeNoritoStringValue,
          "MultisigContractCallProposeDto.fee_sponsor",
        ),
      ],
      [
        encodeOptionValue(
          request.gas_limit ?? request.gasLimit ?? null,
          encodeU64NumberValue,
          "MultisigContractCallProposeDto.gas_limit",
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
          encodeNoritoStringValue,
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
        encodeOptionValue(
          request.fee_sponsor ?? request.feeSponsor ?? null,
          encodeNoritoStringValue,
          "MultisigContractCallApproveDto.fee_sponsor",
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
  if (forcePureJsInstructionCodec) {
    try {
      const decoded = decodePureJsInstruction(buffer);
      return options.parseJson === false ? JSON.stringify(decoded) : decoded;
    } catch {
      // Some callers may still pass bytes produced by the native binding before
      // the stale-binding fallback was enabled.
    }
  }
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
    } catch {
      throw error;
    }
  }
  if (options.parseJson === false) {
    return json;
  }
  return JSON.parse(json);
}

/**
 * Encode an `iroha_data_model::zk::OpenVerifyEnvelope` as standalone Norito bytes.
 *
 * @param {object} envelope
 * @returns {Buffer}
 */
export function noritoEncodePrivacyProofEnvelope(envelope) {
  const payload = encodeOpenVerifyEnvelopePayload(envelope, "OpenVerifyEnvelope");
  return frameNoritoPayload(payload, OPEN_VERIFY_ENVELOPE_SCHEMA_HASH, 0);
}

/**
 * Decode standalone Norito bytes for `iroha_data_model::zk::OpenVerifyEnvelope`.
 *
 * @param {ArrayBufferView | ArrayBuffer | Buffer | string} bytes
 * @returns {object}
 */
export function noritoDecodePrivacyProofEnvelope(bytes) {
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
  return encodePureJsInstructionPayload(instruction);
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
        true,
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
    case "iroha_data_model::isi::zk::RegisterZkAceIdentityCommitment":
    case "iroha_data_model::isi::zk::RotateZkAceIdentityCommitment":
    case "iroha_data_model::isi::zk::RevokeZkAceIdentityCommitment":
    case "zk::ScheduleConfidentialPolicyTransition":
    case "zk::CancelConfidentialPolicyTransition":
    case "iroha_data_model::isi::zk::Shield":
    case "iroha_data_model::isi::zk::ZkTransfer":
    case "iroha_data_model::isi::zk::AssetHiddenZkTransfer":
    case "iroha_data_model::isi::zk::SubmitZkAceAuthorizedTransfer":
    case "iroha_data_model::isi::zk::Unshield":
    case "iroha_data_model::isi::zk::CreateElection":
    case "iroha_data_model::isi::zk::SubmitBallot":
    case "iroha_data_model::isi::zk::FinalizeElection":
      return decodeZkInstructionPayload(wireId, payload);
    case "iroha_data_model::isi::verifying_keys::RegisterVerifyingKey":
    case "iroha_data_model::isi::verifying_keys::UpdateVerifyingKey":
      return decodeVerifyingKeyInstructionPayload(wireId, payload);
    default:
      const cached = getCachedInstruction(buffer);
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
  const outerPayload = encodeInstructionBoxPayload(wireId, innerPayload, 0);
  return frameNoritoPayload(outerPayload, INSTRUCTION_BOX_SCHEMA_HASH, 0);
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
          true,
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
          amount: decodeU128StringValue(fields.amount, "CastPlainBallot.amount"),
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
          amount: decodeNumericValue(fields.amount, "SendToTwitter.amount"),
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
    case "iroha_data_model::isi::zk::RegisterZkAceIdentityCommitment": {
      const fields = decodeStructFields(payload, "zk.RegisterZkAceIdentityCommitment", [
        "asset",
        "identity_commitment",
        "policy_hash",
        "allowed_accounts",
        "action_class",
        "domain_tag",
        "verifier_key",
      ]);
      return {
        zk: {
          RegisterZkAceIdentityCommitment: {
            asset: decodeAssetDefinitionIdValue(
              fields.asset,
              "zk.RegisterZkAceIdentityCommitment.asset",
            ),
            identity_commitment: Array.from(
              decodeFixedBytesValue(
                fields.identity_commitment,
                32,
                "zk.RegisterZkAceIdentityCommitment.identity_commitment",
              ),
            ),
            policy_hash: Array.from(
              decodeFixedBytesValue(
                fields.policy_hash,
                32,
                "zk.RegisterZkAceIdentityCommitment.policy_hash",
              ),
            ),
            allowed_accounts: decodeNoritoVec(
              fields.allowed_accounts,
              (entry, index) =>
                decodeAccountIdValue(
                  entry,
                  `zk.RegisterZkAceIdentityCommitment.allowed_accounts[${index}]`,
                ),
              "zk.RegisterZkAceIdentityCommitment.allowed_accounts",
            ),
            action_class: decodeStringValue(
              fields.action_class,
              "zk.RegisterZkAceIdentityCommitment.action_class",
            ),
            domain_tag: decodeStringValue(
              fields.domain_tag,
              "zk.RegisterZkAceIdentityCommitment.domain_tag",
            ),
            verifier_key: decodeVerifyingKeyIdValue(
              fields.verifier_key,
              "zk.RegisterZkAceIdentityCommitment.verifier_key",
            ),
          },
        },
      };
    }
    case "iroha_data_model::isi::zk::RotateZkAceIdentityCommitment": {
      const fields = decodeStructFields(payload, "zk.RotateZkAceIdentityCommitment", [
        "asset",
        "old_identity_commitment",
        "new_identity_commitment",
        "policy_hash",
        "allowed_accounts",
        "action_class",
        "domain_tag",
        "verifier_key",
      ]);
      return {
        zk: {
          RotateZkAceIdentityCommitment: {
            asset: decodeAssetDefinitionIdValue(
              fields.asset,
              "zk.RotateZkAceIdentityCommitment.asset",
            ),
            old_identity_commitment: Array.from(
              decodeFixedBytesValue(
                fields.old_identity_commitment,
                32,
                "zk.RotateZkAceIdentityCommitment.old_identity_commitment",
              ),
            ),
            new_identity_commitment: Array.from(
              decodeFixedBytesValue(
                fields.new_identity_commitment,
                32,
                "zk.RotateZkAceIdentityCommitment.new_identity_commitment",
              ),
            ),
            policy_hash: Array.from(
              decodeFixedBytesValue(
                fields.policy_hash,
                32,
                "zk.RotateZkAceIdentityCommitment.policy_hash",
              ),
            ),
            allowed_accounts: decodeNoritoVec(
              fields.allowed_accounts,
              (entry, index) =>
                decodeAccountIdValue(
                  entry,
                  `zk.RotateZkAceIdentityCommitment.allowed_accounts[${index}]`,
                ),
              "zk.RotateZkAceIdentityCommitment.allowed_accounts",
            ),
            action_class: decodeStringValue(
              fields.action_class,
              "zk.RotateZkAceIdentityCommitment.action_class",
            ),
            domain_tag: decodeStringValue(
              fields.domain_tag,
              "zk.RotateZkAceIdentityCommitment.domain_tag",
            ),
            verifier_key: decodeVerifyingKeyIdValue(
              fields.verifier_key,
              "zk.RotateZkAceIdentityCommitment.verifier_key",
            ),
          },
        },
      };
    }
    case "iroha_data_model::isi::zk::RevokeZkAceIdentityCommitment": {
      const fields = decodeStructFields(payload, "zk.RevokeZkAceIdentityCommitment", [
        "asset",
        "identity_commitment",
        "reason_hash",
      ]);
      return {
        zk: {
          RevokeZkAceIdentityCommitment: {
            asset: decodeAssetDefinitionIdValue(
              fields.asset,
              "zk.RevokeZkAceIdentityCommitment.asset",
            ),
            identity_commitment: Array.from(
              decodeFixedBytesValue(
                fields.identity_commitment,
                32,
                "zk.RevokeZkAceIdentityCommitment.identity_commitment",
              ),
            ),
            reason_hash: decodeOptionValue(
              fields.reason_hash,
              (entry, context) => Array.from(decodeFixedByteArrayArchiveValue(entry, 32, context)),
              "zk.RevokeZkAceIdentityCommitment.reason_hash",
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
            amount: decodeU128SafeNumberValue(fields.amount, "zk.Shield.amount"),
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
    case "iroha_data_model::isi::zk::SubmitZkAceAuthorizedTransfer": {
      const fields = decodeStructFields(payload, "zk.SubmitZkAceAuthorizedTransfer", [
        "from",
        "to",
        "asset",
        "amount",
        "identity_commitment",
        "tx_digest",
        "chain_id",
        "domain_tag",
        "action_class",
        "replay_nullifier",
        "policy_hash",
        "proof",
      ]);
      return {
        zk: {
          SubmitZkAceAuthorizedTransfer: {
            from: decodeAccountIdValue(fields.from, "zk.SubmitZkAceAuthorizedTransfer.from"),
            to: decodeAccountIdValue(fields.to, "zk.SubmitZkAceAuthorizedTransfer.to"),
            asset: decodeAssetDefinitionIdValue(
              fields.asset,
              "zk.SubmitZkAceAuthorizedTransfer.asset",
            ),
            amount: decodeU128SafeNumberValue(
              fields.amount,
              "zk.SubmitZkAceAuthorizedTransfer.amount",
            ),
            identity_commitment: Array.from(
              decodeFixedBytesValue(
                fields.identity_commitment,
                32,
                "zk.SubmitZkAceAuthorizedTransfer.identity_commitment",
              ),
            ),
            tx_digest: Array.from(
              decodeFixedBytesValue(
                fields.tx_digest,
                32,
                "zk.SubmitZkAceAuthorizedTransfer.tx_digest",
              ),
            ),
            chain_id: decodeStringValue(fields.chain_id, "zk.SubmitZkAceAuthorizedTransfer.chain_id"),
            domain_tag: decodeStringValue(
              fields.domain_tag,
              "zk.SubmitZkAceAuthorizedTransfer.domain_tag",
            ),
            action_class: decodeStringValue(
              fields.action_class,
              "zk.SubmitZkAceAuthorizedTransfer.action_class",
            ),
            replay_nullifier: Array.from(
              decodeFixedBytesValue(
                fields.replay_nullifier,
                32,
                "zk.SubmitZkAceAuthorizedTransfer.replay_nullifier",
              ),
            ),
            policy_hash: Array.from(
              decodeFixedBytesValue(
                fields.policy_hash,
                32,
                "zk.SubmitZkAceAuthorizedTransfer.policy_hash",
              ),
            ),
            proof: decodeProofAttachmentValue(
              fields.proof,
              "zk.SubmitZkAceAuthorizedTransfer.proof",
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
            public_amount: decodeU128SafeNumberValue(
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
          quantity: decodeNumericValue(fields.quantity, "TransferRwa.quantity"),
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
          quantity: decodeNumericValue(fields.quantity, "ForceTransferRwa.quantity"),
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
      quantity: decodeNumericValue(fields.quantity, `${name}.quantity`),
    },
  };
}

function encodeTransferObjectBody(
  value,
  context,
  encodeSource,
  encodeObject,
  encodeDestination,
  wrapObject = false,
) {
  return encodeStructValue([
    [encodeSource(value.source, `${context}.source`)],
    [
      wrapObject
        ? encodeNoritoField(encodeObject(value.object, `${context}.object`))
        : encodeObject(value.object, `${context}.object`),
    ],
    [encodeDestination(value.destination, `${context}.destination`)],
  ]);
}

function decodeTransferObjectBody(
  payload,
  context,
  decodeSource,
  decodeObject,
  decodeDestination,
  wrapObject = false,
) {
  const fields = decodeStructFields(payload, context, ["source", "object", "destination"]);
  return {
    source: decodeSource(fields.source, `${context}.source`),
    object: wrapObject
      ? decodeNestedValue(fields.object, decodeObject, `${context}.object`)
      : decodeObject(fields.object, `${context}.object`),
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
  return encodeNoritoStringValue(assertNonEmptyString(value, context));
}

function decodeDomainIdValue(payload, context) {
  return decodeStringValue(payload, context);
}

function encodeArchivedDomainIdValue(value, context) {
  return encodeNoritoField(encodeDomainIdValue(value, context));
}

function decodeArchivedDomainIdValue(payload, context) {
  return decodeNestedValue(payload, decodeDomainIdValue, context);
}

function encodeNameValue(value, context) {
  return encodeNoritoStringValue(assertNonEmptyString(value, context));
}

function decodeNameValue(payload, context) {
  return decodeStringValue(payload, context);
}

function encodeRoleIdValue(value, context) {
  return encodeNoritoStringValue(assertNonEmptyString(value, context));
}

function decodeRoleIdValue(payload, context) {
  return decodeStringValue(payload, context);
}

function encodeNftIdValue(value, context) {
  const literal = assertNonEmptyString(value, context);
  const separator = literal.indexOf("$");
  if (separator <= 0 || separator === literal.length - 1) {
    throw new Error(`${context} must use name$domain`);
  }
  return encodeTupleValue([
    encodeNoritoField(
      encodeDomainIdValue(literal.slice(separator + 1), `${context}.domain`),
    ),
    encodeNameValue(literal.slice(0, separator), `${context}.name`),
  ]);
}

function decodeNftIdValue(payload, context) {
  const fields = decodeTupleFields(payload, context, ["domain", "name"]);
  return `${decodeNameValue(fields.name, `${context}.name`)}$${decodeNestedValue(fields.domain, decodeDomainIdValue, `${context}.domain`)}`;
}

function encodeRwaIdValue(value, context) {
  const literal = assertNonEmptyString(value, context);
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
    [encodeNoritoField(encodeDomainIdValue(value.id, `${context}.id`))],
    [encodeOptionValue(value.logo, encodeSorafsUriValue, `${context}.logo`)],
    [encodeMetadataValue(value.metadata ?? {}, `${context}.metadata`)],
  ]);
}

function decodeNewDomainValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["id", "logo", "metadata"]);
  return {
    id: decodeNestedValue(fields.id, decodeDomainIdValue, `${context}.id`),
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
        [encodeNumericValue(instruction.SendToTwitter.amount, "SendToTwitter.amount")],
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
    [encodeU128Value(value.amount, "CastPlainBallot.amount")],
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
    ["RegisterZkAceIdentityCommitment", "iroha_data_model::isi::zk::RegisterZkAceIdentityCommitment", encodeRegisterZkAceIdentityCommitmentPayload],
    ["RotateZkAceIdentityCommitment", "iroha_data_model::isi::zk::RotateZkAceIdentityCommitment", encodeRotateZkAceIdentityCommitmentPayload],
    ["RevokeZkAceIdentityCommitment", "iroha_data_model::isi::zk::RevokeZkAceIdentityCommitment", encodeRevokeZkAceIdentityCommitmentPayload],
    ["ScheduleConfidentialPolicyTransition", "zk::ScheduleConfidentialPolicyTransition", encodeScheduleConfidentialPolicyTransitionPayload],
    ["CancelConfidentialPolicyTransition", "zk::CancelConfidentialPolicyTransition", encodeCancelConfidentialPolicyTransitionPayload],
    ["Shield", "iroha_data_model::isi::zk::Shield", encodeShieldPayload],
    ["ZkTransfer", "iroha_data_model::isi::zk::ZkTransfer", encodeZkTransferPayload],
    ["AssetHiddenZkTransfer", "iroha_data_model::isi::zk::AssetHiddenZkTransfer", encodeAssetHiddenZkTransferPayload],
    ["SubmitZkAceAuthorizedTransfer", "iroha_data_model::isi::zk::SubmitZkAceAuthorizedTransfer", encodeSubmitZkAceAuthorizedTransferPayload],
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

function encodeRegisterZkAceIdentityCommitmentPayload(value) {
  return encodeStructValue([
    [encodeAssetDefinitionIdValue(value.asset, "zk.RegisterZkAceIdentityCommitment.asset")],
    [encodeFixedBytesValue(value.identity_commitment, 32, "zk.RegisterZkAceIdentityCommitment.identity_commitment")],
    [encodeFixedBytesValue(value.policy_hash, 32, "zk.RegisterZkAceIdentityCommitment.policy_hash")],
    [encodeNoritoVec(value.allowed_accounts ?? [], (entry, index) =>
      encodeAccountIdValue(entry, `zk.RegisterZkAceIdentityCommitment.allowed_accounts[${index}]`),
    )],
    [encodeNoritoStringValue(assertNonEmptyString(value.action_class, "zk.RegisterZkAceIdentityCommitment.action_class"))],
    [encodeNoritoStringValue(assertNonEmptyString(value.domain_tag, "zk.RegisterZkAceIdentityCommitment.domain_tag"))],
    [encodeVerifyingKeyIdValue(value.verifier_key, "zk.RegisterZkAceIdentityCommitment.verifier_key")],
  ]);
}

function encodeRotateZkAceIdentityCommitmentPayload(value) {
  return encodeStructValue([
    [encodeAssetDefinitionIdValue(value.asset, "zk.RotateZkAceIdentityCommitment.asset")],
    [encodeFixedBytesValue(value.old_identity_commitment, 32, "zk.RotateZkAceIdentityCommitment.old_identity_commitment")],
    [encodeFixedBytesValue(value.new_identity_commitment, 32, "zk.RotateZkAceIdentityCommitment.new_identity_commitment")],
    [encodeFixedBytesValue(value.policy_hash, 32, "zk.RotateZkAceIdentityCommitment.policy_hash")],
    [encodeNoritoVec(value.allowed_accounts ?? [], (entry, index) =>
      encodeAccountIdValue(entry, `zk.RotateZkAceIdentityCommitment.allowed_accounts[${index}]`),
    )],
    [encodeNoritoStringValue(assertNonEmptyString(value.action_class, "zk.RotateZkAceIdentityCommitment.action_class"))],
    [encodeNoritoStringValue(assertNonEmptyString(value.domain_tag, "zk.RotateZkAceIdentityCommitment.domain_tag"))],
    [encodeVerifyingKeyIdValue(value.verifier_key, "zk.RotateZkAceIdentityCommitment.verifier_key")],
  ]);
}

function encodeRevokeZkAceIdentityCommitmentPayload(value) {
  return encodeStructValue([
    [encodeAssetDefinitionIdValue(value.asset, "zk.RevokeZkAceIdentityCommitment.asset")],
    [encodeFixedBytesValue(value.identity_commitment, 32, "zk.RevokeZkAceIdentityCommitment.identity_commitment")],
    [
      encodeOptionValue(
        value.reason_hash,
        (entry, context) => encodeFixedByteArrayArchiveValue(entry, 32, context),
        "zk.RevokeZkAceIdentityCommitment.reason_hash",
      ),
    ],
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
    [encodeU128Value(value.amount, "zk.Shield.amount")],
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

function encodeSubmitZkAceAuthorizedTransferPayload(value) {
  return encodeStructValue([
    [encodeAccountIdValue(value.from, "zk.SubmitZkAceAuthorizedTransfer.from")],
    [encodeAccountIdValue(value.to, "zk.SubmitZkAceAuthorizedTransfer.to")],
    [encodeAssetDefinitionIdValue(value.asset, "zk.SubmitZkAceAuthorizedTransfer.asset")],
    [encodeU128Value(value.amount, "zk.SubmitZkAceAuthorizedTransfer.amount")],
    [encodeFixedBytesValue(value.identity_commitment, 32, "zk.SubmitZkAceAuthorizedTransfer.identity_commitment")],
    [encodeFixedBytesValue(value.tx_digest, 32, "zk.SubmitZkAceAuthorizedTransfer.tx_digest")],
    [encodeNoritoStringValue(assertNonEmptyString(value.chain_id, "zk.SubmitZkAceAuthorizedTransfer.chain_id"))],
    [encodeNoritoStringValue(assertNonEmptyString(value.domain_tag, "zk.SubmitZkAceAuthorizedTransfer.domain_tag"))],
    [encodeNoritoStringValue(assertNonEmptyString(value.action_class, "zk.SubmitZkAceAuthorizedTransfer.action_class"))],
    [encodeFixedBytesValue(value.replay_nullifier, 32, "zk.SubmitZkAceAuthorizedTransfer.replay_nullifier")],
    [encodeFixedBytesValue(value.policy_hash, 32, "zk.SubmitZkAceAuthorizedTransfer.policy_hash")],
    [encodeProofAttachmentValue(value.proof, "zk.SubmitZkAceAuthorizedTransfer.proof")],
  ]);
}

function encodeUnshieldPayload(value) {
  return encodeStructValue([
    [encodeAssetDefinitionIdValue(value.asset, "zk.Unshield.asset")],
    [encodeAccountIdValue(value.to, "zk.Unshield.to")],
    [encodeU128Value(value.public_amount, "zk.Unshield.public_amount")],
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
  const literal = assertNonEmptyString(
    typeof value === "string" ? value : `${value.domain_id}:${value.call_name}`,
    context,
  );
  const separator = literal.indexOf(":");
  if (separator <= 0 || separator === literal.length - 1) {
    throw new Error(`${context} must use domain:call format`);
  }
  return encodeStructValue([
    [encodeNoritoField(encodeDomainIdValue(literal.slice(0, separator), `${context}.domain_id`))],
    [encodeNameValue(literal.slice(separator + 1), `${context}.call_name`)],
  ]);
}

function decodeKaigiIdValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["domain_id", "call_name"]);
  return {
    domain_id: decodeNestedValue(fields.domain_id, decodeDomainIdValue, `${context}.domain_id`),
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
    [encodeNumericValue(value.quantity, "TransferRwa.quantity")],
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
    [encodeNumericValue(value.quantity, "RedeemRwa.quantity")],
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
    [encodeNumericValue(value.quantity, "HoldRwa.quantity")],
  ]);
}

function encodeReleaseRwaPayload(value) {
  return encodeStructValue([
    [encodeRwaIdValue(value.rwa, "ReleaseRwa.rwa")],
    [encodeNumericValue(value.quantity, "ReleaseRwa.quantity")],
  ]);
}

function encodeForceTransferRwaPayload(value) {
  return encodeStructValue([
    [encodeRwaIdValue(value.rwa, "ForceTransferRwa.rwa")],
    [encodeNumericValue(value.quantity, "ForceTransferRwa.quantity")],
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
    [encodeNumericValue(value.quantity, `${context}.quantity`)],
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
    quantity: decodeNumericValue(fields.quantity, `${context}.quantity`),
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
    [encodeNumericValue(value.quantity, `${context}.quantity`)],
  ]);
}

function decodeRwaParentRefValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["rwa", "quantity"]);
  return {
    rwa: decodeRwaIdValue(fields.rwa, `${context}.rwa`),
    quantity: decodeNumericValue(fields.quantity, `${context}.quantity`),
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
    encodeNoritoField(encodeNumericValue(value.object, `${context}.object`)),
    encodeNoritoField(encodeAssetIdValue(value.destination, `${context}.destination`)),
  ]);
}

function decodeAssetInstructionBody(payload, context) {
  const reader = new BufferReader(payload, context);
  const object = decodeNumericValue(readNoritoField(reader, "object"), `${context}.object`);
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
    encodeNoritoField(encodeNumericValue(value.object, "Transfer.Asset.object")),
    encodeNoritoField(encodeAccountIdValue(value.destination, "Transfer.Asset.destination")),
  ]);
}

function decodeTransferAssetBody(payload) {
  const reader = new BufferReader(payload, "Transfer.Asset");
  const source = decodeAssetIdValue(readNoritoField(reader, "source"), "Transfer.Asset.source");
  const object = decodeNumericValue(readNoritoField(reader, "object"), "Transfer.Asset.object");
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

function encodeStringValue(value, context) {
  if (typeof value !== "string") {
    throw new TypeError(`${context} must be a string`);
  }
  return encodeNoritoStringValue(value);
}

function encodeHashLiteralBytes(value, context) {
  if (Buffer.isBuffer(value) || ArrayBuffer.isView(value) || value instanceof ArrayBuffer || Array.isArray(value)) {
    return encodeFixedBytesValue(value, 32, context);
  }
  const literal = assertNonEmptyString(value, context);
  const match = HASH_LITERAL_RE.exec(literal);
  if (match) {
    const [, body, checksum] = match;
    const upper = body.toUpperCase();
    const expected = computeHashLiteralCrc("hash", upper);
    if (checksum.toUpperCase() !== expected) {
      throw new Error(`${context} has invalid checksum; expected ${expected}`);
    }
    return Buffer.from(upper, "hex");
  }
  if (/^[0-9A-Fa-f]{64}$/.test(literal)) {
    return Buffer.from(literal, "hex");
  }
  throw new Error(`${context} must be a 32-byte hash literal or hex string`);
}

function decodeHashLiteral(payload, context) {
  const bytes = decodeFixedBytesValue(payload, 32, context);
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
  const normalized = assertNonEmptyString(value, context).toLowerCase();
  if (normalized === "zk") {
    return encodeEnumTagValue(0);
  }
  if (normalized === "plain") {
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
  const normalized = assertNonEmptyString(value, context)
    .toLowerCase()
    .replace(/[^a-z0-9]/g, "");
  switch (normalized) {
    case "halo2ipapasta":
    case "halo2pasta":
    case "halo2ipa":
    case "pasta":
      return encodeEnumTagValue(0);
    case "halo2bn254":
      return encodeEnumTagValue(1);
    case "groth16":
      return encodeEnumTagValue(2);
    case "stark":
    case "starkfri":
    case "starkfrisha256goldilocks":
      return encodeEnumTagValue(3);
    case "unsupported":
      return encodeEnumTagValue(4);
    case "halo2ipaorchard":
    case "orchard":
    case "zcashorchard":
      return encodeEnumTagValue(5);
    case "groth16bls12377":
    case "groth16bls12377decaf377":
    case "bls12377":
    case "decaf377":
    case "masp":
    case "penumbra":
    case "penumbramasp":
    case "halo2ipapenumbra":
    case "halo2ipamasp":
      return encodeEnumTagValue(6);
    case "fcmppluspluscurvetree":
    case "fcmp":
    case "monero":
    case "monerofcmp":
    case "monerofcmpplusplus":
    case "curvetree":
    case "halo2ipamonero":
    case "halo2ipacurvetree":
      return encodeEnumTagValue(7);
    case "latticepcssis":
    case "latticepcszk":
    case "jindo":
    case "jindolatticepcszk":
    case "jindolatticepcszkv0":
    case "jindolatticepcssis":
      return encodeEnumTagValue(8);
    case "starkfrimiden":
    case "midenstark":
      return encodeEnumTagValue(9);
    case "aztecplonkishprivatekernel":
    case "aztecprivatekernel":
      return encodeEnumTagValue(10);
    case "pqmaspstarkfri":
    case "pqmaspstark":
    case "starkfripqmaspstarkfri":
    case "postquantummasp":
      return encodeEnumTagValue(11);
    case "anonymouspgc":
    case "anonymouspgckoutofn":
    case "anonymouspgckoutofnv1":
      return encodeEnumTagValue(12);
    case "verange":
    case "verangetransparentrange":
    case "verangetransparentrangev1":
      return encodeEnumTagValue(13);
    case "zkat":
    case "zkatpolicyprivateauthenticator":
    case "zkatpolicyprivateauthv1":
      return encodeEnumTagValue(14);
    case "recursiveanonymousadmission":
    case "recursiveanonymousadmissionv0":
    case "zkamsrecursiveadmission":
    case "zkamsrecursiveadmissionv0":
      return encodeEnumTagValue(15);
    case "vegaexistingcredentialzk":
    case "vegaexistingcredentialzkv0":
      return encodeEnumTagValue(16);
    case "silentthresholdanoncred":
    case "silentthresholdanoncredv0":
    case "silentthresholdanonymouscredential":
    case "thresholdanonymouscredentials":
      return encodeEnumTagValue(17);
    case "zkx509":
    case "zkvmx509identity":
    case "zkx509onchainidentity":
    case "zkx509onchainidentityv0":
      return encodeEnumTagValue(18);
    case "siswithhints":
    case "sishints":
    case "sishintsanoncredpqv0":
    case "latticeanonymouscredentials":
      return encodeEnumTagValue(19);
    default:
      throw new Error(`${context} uses unsupported backend tag ${value}`);
  }
}

function decodeBackendTagValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("tag");
  reader.assertEof();
  switch (tag) {
    case 0:
      return "Halo2IpaPasta";
    case 1:
      return "Halo2Bn254";
    case 2:
      return "Groth16";
    case 3:
      return "Stark";
    case 4:
      return "Unsupported";
    case 5:
      return "Halo2IpaOrchard";
    case 6:
      return "Groth16Bls12377";
    case 7:
      return "FcmpPlusPlusCurveTree";
    case 8:
      return "LatticePcsSis";
    case 9:
      return "MidenStark";
    case 10:
      return "AztecPlonkishPrivateKernel";
    case 11:
      return "PqMaspStarkFri";
    case 12:
      return "AnonymousPgc";
    case 13:
      return "VeRange";
    case 14:
      return "ZkAt";
    case 15:
      return "RecursiveAnonymousAdmission";
    case 16:
      return "VegaExistingCredentialZk";
    case 17:
      return "SilentThresholdAnoncred";
    case 18:
      return "ZkX509";
    case 19:
      return "SisWithHints";
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
  return encodeStructValue([
    [encodeBackendTagValue(value.backend, `${context}.backend`)],
    [encodeNoritoStringValue(assertNonEmptyString(value.circuit_id, `${context}.circuit_id`))],
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

function encodeContractManifestValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  return encodeStructValue([
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
        value.kotoba ?? null,
        encodeKotobaTranslationEntriesValue,
        `${context}.kotoba`,
      ),
    ],
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
    "code_hash",
    "abi_hash",
    "compiler_fingerprint",
    "features_bitmap",
    "access_set_hints",
    "entrypoints",
    "kotoba",
    "provenance",
  ]);
  const decoded = {
    entrypoints: decodeOptionValue(
      fields.entrypoints,
      decodeEntrypointDescriptorsValue,
      `${context}.entrypoints`,
    ),
    kotoba: decodeOptionValue(
      fields.kotoba,
      decodeKotobaTranslationEntriesValue,
      `${context}.kotoba`,
    ),
  };
  const code_hash = decodeOptionValue(fields.code_hash, decodeHashValue, `${context}.code_hash`);
  const abi_hash = decodeOptionValue(fields.abi_hash, decodeHashValue, `${context}.abi_hash`);
  const compiler_fingerprint = decodeOptionValue(
    fields.compiler_fingerprint,
    decodeStringValue,
    `${context}.compiler_fingerprint`,
  );
  const features_bitmap = decodeOptionValue(
    fields.features_bitmap,
    decodeU64NumberValue,
    `${context}.features_bitmap`,
  );
  const access_set_hints = decodeOptionValue(
    fields.access_set_hints,
    decodeAccessSetHintsValue,
    `${context}.access_set_hints`,
  );
  const provenance = decodeOptionValue(
    fields.provenance,
    decodeManifestProvenanceValue,
    `${context}.provenance`,
  );
  if (code_hash !== null) {
    decoded.code_hash = code_hash;
  }
  if (abi_hash !== null) {
    decoded.abi_hash = abi_hash;
  }
  if (compiler_fingerprint !== null) {
    decoded.compiler_fingerprint = compiler_fingerprint;
  }
  if (features_bitmap !== null) {
    decoded.features_bitmap = features_bitmap;
  }
  if (access_set_hints !== null) {
    decoded.access_set_hints = access_set_hints;
  }
  if (provenance !== null) {
    decoded.provenance = provenance;
  }
  return decoded;
}

function encodeAccessSetHintsValue(value, context) {
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
  const reader = new BufferReader(payload, context);
  const read_keys = readNoritoField(reader, "read_keys");
  const write_keys = readNoritoField(reader, "write_keys");
  const dynamic_reads =
    reader.offset < reader.buffer.length ? readNoritoField(reader, "dynamic_reads") : null;
  const dynamic_writes =
    reader.offset < reader.buffer.length ? readNoritoField(reader, "dynamic_writes") : null;
  reader.assertEof();
  return {
    read_keys: decodeNoritoVec(
      read_keys,
      (entry, index) => decodeStringValue(entry, `${context}.read_keys[${index}]`),
      `${context}.read_keys`,
    ),
    write_keys: decodeNoritoVec(
      write_keys,
      (entry, index) => decodeStringValue(entry, `${context}.write_keys[${index}]`),
      `${context}.write_keys`,
    ),
    dynamic_reads:
      dynamic_reads === null
        ? []
        : decodeNoritoVec(
            dynamic_reads,
            (entry, index) => decodeDynamicAccessHintValue(entry, `${context}.dynamic_reads[${index}]`),
            `${context}.dynamic_reads`,
          ),
    dynamic_writes:
      dynamic_writes === null
        ? []
        : decodeNoritoVec(
            dynamic_writes,
            (entry, index) => decodeDynamicAccessHintValue(entry, `${context}.dynamic_writes[${index}]`),
            `${context}.dynamic_writes`,
          ),
  };
}

function encodeDynamicAccessHintValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
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
  const triggers = value.triggers ?? [];
  if (!Array.isArray(triggers)) {
    throw new TypeError(`${context}.triggers must be an array`);
  }
  if (triggers.length > 0) {
    throw new Error(`${context}.triggers are not yet supported by the manifest codec`);
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
        value.return_type ?? value.returnType ?? null,
        encodeNoritoStringValue,
        `${context}.return_type`,
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
      encodeNoritoVec(value.read_keys ?? value.readKeys ?? [], (entry, index) =>
        encodeNoritoStringValue(
          assertNonEmptyString(entry, `${context}.read_keys[${index}]`),
        ),
      ),
    ],
    [
      encodeNoritoVec(value.write_keys ?? value.writeKeys ?? [], (entry, index) =>
        encodeNoritoStringValue(
          assertNonEmptyString(entry, `${context}.write_keys[${index}]`),
        ),
      ),
    ],
    [
      encodeOptionValue(
        value.access_hints_complete ?? value.accessHintsComplete ?? null,
        encodeBoolValue,
        `${context}.access_hints_complete`,
      ),
    ],
    [
      encodeNoritoVec(
        value.access_hints_skipped ?? value.accessHintsSkipped ?? [],
        (entry, index) =>
          encodeNoritoStringValue(
            assertNonEmptyString(entry, `${context}.access_hints_skipped[${index}]`),
          ),
      ),
    ],
    [encodeNoritoVec(triggers, (entry, index) =>
      encodeUnsupportedManifestTriggerValue(entry, `${context}.triggers[${index}]`),
    )],
  ]);
}

function decodeEntrypointDescriptorValue(payload, context) {
  const fields = decodeStructFields(payload, context, [
    "name",
    "kind",
    "params",
    "return_type",
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
    return_type: decodeOptionValue(
      fields.return_type,
      decodeStringValue,
      `${context}.return_type`,
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
      (entry, index) => decodeUnsupportedManifestTriggerValue(entry, `${context}.triggers[${index}]`),
      `${context}.triggers`,
    ),
  };
}

function encodeEntryPointKindValue(value, context) {
  const kind =
    typeof value === "string"
      ? value
      : value?.kind ?? value?.Kind ?? value?.variant ?? value?.tag;
  const normalized = assertNonEmptyString(kind, context).toLowerCase();
  switch (normalized) {
    case "public":
      return encodeEnumTagValue(0);
    case "view":
      return encodeEnumTagValue(1);
    case "hajimari":
      return encodeEnumTagValue(2);
    case "kaizen":
      return encodeEnumTagValue(3);
    default:
      throw new Error(`${context} must be Public, View, Hajimari, or Kaizen`);
  }
}

function decodeEntryPointKindValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("tag");
  reader.assertEof();
  switch (tag) {
    case 0:
      return { kind: "Public", value: null };
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
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.name, `${context}.name`))],
    [encodeNoritoStringValue(assertNonEmptyString(value.type_name ?? value.typeName, `${context}.type_name`))],
  ]);
}

function decodeEntrypointParamDescriptorValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["name", "type_name"]);
  return {
    name: decodeStringValue(fields.name, `${context}.name`),
    type_name: decodeStringValue(fields.type_name, `${context}.type_name`),
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
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.msg_id ?? value.msgId, `${context}.msg_id`))],
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
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.lang, `${context}.lang`))],
    [encodeNoritoStringValue(assertNonEmptyString(value.text, `${context}.text`))],
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
  const signatureLiteral = assertNonEmptyString(value.signature, `${context}.signature`);
  if (
    signatureLiteral.length % 2 !== 0 ||
    !/^[0-9A-Fa-f]+$/u.test(signatureLiteral)
  ) {
    throw new Error(`${context}.signature must be an even-length hexadecimal string`);
  }
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.signer, `${context}.signer`))],
    [
      encodeNoritoVec(Array.from(Buffer.from(signatureLiteral, "hex")), (byte, index) =>
        encodeU8Value(byte, `${context}.signature[${index}]`),
      ),
    ],
  ]);
}

function decodeManifestProvenanceValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["signer", "signature"]);
  return {
    signer: decodeStringValue(fields.signer, `${context}.signer`),
    signature: Buffer.from(
      decodeNoritoVec(
        fields.signature,
        (entry, index) => decodeU8Value(entry, `${context}.signature[${index}]`),
        `${context}.signature`,
      ),
    ).toString("hex").toUpperCase(),
  };
}

function encodeUnsupportedManifestTriggerValue(_value, context) {
  throw new Error(`${context} is not yet supported by the manifest codec`);
}

function decodeUnsupportedManifestTriggerValue(_payload, context) {
  throw new Error(`${context} is not yet supported by the manifest codec`);
}

function encodeNumericValue(value, context) {
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

function decodeNumericValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const mantissaPayload = readNoritoField(reader, "mantissa");
  const scalePayload = readNoritoField(reader, "scale");
  reader.assertEof();

  const mantissaReader = new BufferReader(mantissaPayload, `${context}.mantissa`);
  const byteLength = mantissaReader.readU32LE("byteLength");
  const bytes = mantissaReader.readBytes(byteLength, "bytes");
  mantissaReader.assertEof();

  const scaleReader = new BufferReader(scalePayload, `${context}.scale`);
  const scale = scaleReader.readU32LE("value");
  scaleReader.assertEof();

  const mantissa = twosBytesToBigInt(bytes);
  return formatNumericLiteral(mantissa, scale);
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

function decodeStringValue(payload, context, lengthFlags = 0) {
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

function decodeNoritoFrame(buffer, context, expectedSchemaHash) {
  const reader = new BufferReader(buffer, context);
  const magic = reader.readBytes(4, "magic").toString("ascii");
  if (magic !== "NRT0") {
    throw new Error(`${context} is not an NRT0 frame`);
  }
  const major = reader.readU8("versionMajor");
  const minor = reader.readU8("versionMinor");
  if (major !== 0 || minor !== 0) {
    throw new Error(`${context} uses unsupported NRT0 version ${major}.${minor}`);
  }
  const schemaHash = reader.readBytes(16, "schemaHash");
  if (expectedSchemaHash && !schemaHash.equals(expectedSchemaHash)) {
    throw new Error(`${context} schema hash did not match the expected type`);
  }
  reader.readU8("reserved");
  const payloadLength = bigintToSafeNumber(
    reader.readU64LE("payloadLength"),
    `${context}.payloadLength`,
  );
  const expectedCrc = reader.readU64LE("payloadCrc");
  const flags = reader.readU8("flags");
  const paddingLength = reader.buffer.length - reader.offset - payloadLength;
  if (paddingLength < 0) {
    throw new Error(`${context} payload length exceeds the available frame bytes`);
  }
  if (paddingLength > 0) {
    const padding = reader.readBytes(paddingLength, "padding");
    if (padding.some((byte) => byte !== 0)) {
      throw new Error(`${context} contains non-zero alignment padding`);
    }
  }
  const payload = reader.readBytes(payloadLength, "payload");
  reader.assertEof();
  const actualCrc = crc64Ecma(payload);
  if (actualCrc !== expectedCrc) {
    throw new Error(`${context} CRC64 mismatch`);
  }
  return { payload, schemaHash, flags };
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
  let literal;
  if (typeof value === "string") {
    literal = value.trim();
  } else if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      throw new TypeError(`${context} must be a finite number`);
    }
    literal = String(value);
  } else if (typeof value === "bigint") {
    literal = value.toString();
  } else {
    throw new TypeError(`${context} must be a numeric string, number, or bigint`);
  }

  if (!/^-?\d+(?:\.\d+)?$/.test(literal)) {
    throw new TypeError(`${context} must use plain decimal notation`);
  }

  const negative = literal.startsWith("-");
  const unsigned = negative ? literal.slice(1) : literal;
  const [integerPart, fractionalPart = ""] = unsigned.split(".");
  const scale = fractionalPart.length;
  const digits = `${integerPart}${fractionalPart}`.replace(/^0+(?=\d)/, "");
  const mantissa = BigInt(`${negative ? "-" : ""}${digits || "0"}`);
  return { mantissa, scale };
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
