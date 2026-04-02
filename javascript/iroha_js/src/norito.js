import { blake3 } from "@noble/hashes/blake3";
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
const UINT64_MASK = 0xffff_ffff_ffff_ffffn;
const CRC64_REFLECTED_POLY = 0xc96c5795d7870f42n;
const ASSET_DEFINITION_ADDRESS_VERSION = 1;
const BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const UINT128_MASK = (1n << 128n) - 1n;
const HASH_LITERAL_RE = /^hash:([0-9A-Fa-f]{64})#([0-9A-Fa-f]{4})$/;
const MULTIHASH_LITERAL_RE = /^([0-9a-fA-F]+)$/;
const DEFAULT_SM2_DISTINGUISHED_ID = new Uint8Array(16);
const SUPPORTED_JS_FALLBACK_INSTRUCTIONS = [
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
  "ExecuteTrigger",
  "Custom",
  "Kaigi.*",
  "Governance.*",
  "Social.*",
  "SmartContract.*",
  "zk.*",
  "Rwa.*",
];
const INSTRUCTION_BOX_SCHEMA_HASH = Buffer.from(
  "73c28483bfb728cd73c28483bfb728cd",
  "hex",
);
const INNER_SCHEMA_HASH_BY_WIRE_ID = Object.freeze({
  "iroha.mint": Buffer.from("a77313c153163964a77313c153163964", "hex"),
  "iroha.burn": Buffer.from("b8a981b5d11aaa54b8a981b5d11aaa54", "hex"),
  "iroha.register": Buffer.from("5321ba5d245cb6f65321ba5d245cb6f6", "hex"),
  "iroha.transfer": Buffer.from("ffb7ba8dc1b0c984ffb7ba8dc1b0c984", "hex"),
  "iroha.custom": Buffer.from("89e9c7d440cf16b689e9c7d440cf16b6", "hex"),
  "iroha.execute_trigger": Buffer.from(
    "f92cdee1227ffa12f92cdee1227ffa12",
    "hex",
  ),
  "iroha.rwa": Buffer.from("8732d4f3ae064a128732d4f3ae064a12", "hex"),
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

class BufferReader {
  constructor(buffer, context) {
    this.buffer = buffer;
    this.context = context;
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
  if (!native || typeof native[method] !== "function") {
    return null;
  }
  return native;
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

function encodeWithPureJsFallback(normalized, originalError = null) {
  try {
    const encoded = encodePureJsInstruction(normalized);
    cacheInstructionRoundTrip(encoded, normalized);
    return encoded;
  } catch {
    if (originalError) {
      throw originalError;
    }
    throw new Error(
      `Pure JS Norito encoding supports ${SUPPORTED_JS_FALLBACK_INSTRUCTIONS.join(", ")}. Received ${describeInstructionShape(normalized)}.`,
    );
  }
}

/**
 * Encode an instruction JSON payload to canonical Norito bytes.
 * @param {object | string | ArrayBufferView | ArrayBuffer | Buffer} instruction
 * @returns {Buffer}
 */
export function noritoEncodeInstruction(instruction) {
  const native = resolveNative("noritoEncodeInstruction");
  if (native) {
    if (typeof instruction === "string") {
      try {
        const parsed = JSON.parse(instruction);
        const normalized = normalizeInstructionJsonValue(parsed);
        try {
          const encoded = native.noritoEncodeInstruction(JSON.stringify(normalized));
          cacheInstructionRoundTrip(encoded, normalized);
          return encoded;
        } catch (error) {
          return encodeWithPureJsFallback(normalized, error);
        }
      } catch {
        const trimmed = instruction.trim();
        const decoded = tryDecodeBase64(trimmed) ?? tryDecodeHex(trimmed);
        if (decoded) {
          return decoded;
        }
        const encoded = native.noritoEncodeInstruction(instruction);
        try {
          cacheInstructionRoundTrip(encoded, JSON.parse(instruction));
        } catch {
          // Raw JSON string was not parseable; leave cache empty.
        }
        return encoded;
      }
    }
    if (isBinaryLike(instruction)) {
      return toBuffer(instruction);
    }
    const normalized = normalizeInstructionJsonValue(cloneJson(instruction));
    try {
      const encoded = native.noritoEncodeInstruction(JSON.stringify(normalized));
      cacheInstructionRoundTrip(encoded, normalized);
      return encoded;
    } catch (error) {
      return encodeWithPureJsFallback(normalized, error);
    }
  }

  if (isBinaryLike(instruction)) {
    return toBuffer(instruction);
  }
  if (typeof instruction === "string") {
    const trimmed = instruction.trim();
    const decoded = tryDecodeBase64(trimmed) ?? tryDecodeHex(trimmed);
    if (decoded) {
      return decoded;
    }
    const parsed = JSON.parse(trimmed);
    const encoded = encodePureJsInstruction(parsed);
    cacheInstructionRoundTrip(encoded, parsed);
    return encoded;
  }
  const normalized = cloneJson(instruction);
  const encoded = encodePureJsInstruction(normalized);
  cacheInstructionRoundTrip(encoded, normalized);
  return encoded;
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
  const native = resolveNative("noritoDecodeInstruction");
  if (native) {
    let json;
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
    if (options.parseJson === false) {
      return json;
    }
    return JSON.parse(json);
  }

  if (looksLikeNoritoFrame(buffer)) {
    const cached = getCachedInstruction(buffer);
    if (cached !== null) {
      return options.parseJson === false ? canonicalJsonStringify(cached) : cached;
    }
    const decoded = decodePureJsInstruction(buffer);
    if (options.parseJson === false) {
      return canonicalJsonStringify(decoded);
    }
    return decoded;
  }

  const json = buffer.toString("utf8");
  try {
    const parsed = JSON.parse(json);
    return options.parseJson === false ? json : parsed;
  } catch {
    throw new Error(
      `Norito decode in JS-only mode supports ${SUPPORTED_JS_FALLBACK_INSTRUCTIONS.join(", ")}. Run \`npm run build:native\` for full instruction coverage.`,
    );
  }
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
  if (isPlainObject(instruction.ExecuteTrigger)) {
    const payload = encodeExecuteTriggerPayload(instruction.ExecuteTrigger);
    return encodeInstructionEnvelope("iroha.execute_trigger", payload);
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
    `Pure JS Norito encoding supports ${SUPPORTED_JS_FALLBACK_INSTRUCTIONS.join(", ")}. Received ${describeInstructionShape(instruction)}.`,
  );
}

function decodePureJsInstruction(buffer) {
  const { wireId, payload } = decodeInstructionEnvelope(buffer);
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
    case "zk::ScheduleConfidentialPolicyTransition":
    case "zk::CancelConfidentialPolicyTransition":
    case "iroha_data_model::isi::zk::Shield":
    case "iroha_data_model::isi::zk::ZkTransfer":
    case "iroha_data_model::isi::zk::Unshield":
    case "iroha_data_model::isi::zk::CreateElection":
    case "iroha_data_model::isi::zk::SubmitBallot":
    case "iroha_data_model::isi::zk::FinalizeElection":
      return decodeZkInstructionPayload(wireId, payload);
    default:
      const cached = getCachedInstruction(buffer);
      if (cached !== null) {
        return cached;
      }
      throw new Error(
        `Pure JS Norito decode does not support ${wireId}. Run \`npm run build:native\` for full instruction coverage.`,
      );
  }
}

function decodeInstructionEnvelope(bytes) {
  const outer = decodeNoritoFrame(bytes, "instruction", INSTRUCTION_BOX_SCHEMA_HASH);
  const outerReader = new BufferReader(outer.payload, "instruction.outer");
  const wireId = decodeStringValue(
    readNoritoField(outerReader, "wire"),
    "instruction.outer.wire",
  );
  const innerField = readNoritoField(outerReader, "inner");
  const innerReader = new BufferReader(innerField, "instruction.outer.inner");
  const innerBytes = readNoritoField(innerReader, "frame");
  innerReader.assertEof();
  outerReader.assertEof();
  const inner = decodeNoritoFrame(
    innerBytes,
    "instruction.inner",
    INNER_SCHEMA_HASH_BY_WIRE_ID[wireId] ?? null,
  );
  return { wireId, payload: inner.payload };
}

function encodeInstructionEnvelope(wireId, innerPayload) {
  const innerSchemaHash = INNER_SCHEMA_HASH_BY_WIRE_ID[wireId];
  if (!innerSchemaHash) {
    throw new Error(`Pure JS Norito encoding does not know the schema hash for ${wireId}`);
  }
  const innerFrame = frameNoritoPayload(
    innerPayload,
    innerSchemaHash,
    0,
    INNER_HEADER_PADDING_BY_WIRE_ID[wireId] ?? 0,
  );
  const outerPayload = Buffer.concat([
    encodeNoritoField(encodeNoritoStringValue(wireId)),
    encodeNoritoField(encodeNoritoField(innerFrame)),
  ]);
  return frameNoritoPayload(outerPayload, INSTRUCTION_BOX_SCHEMA_HASH);
}

function encodeEnumInstruction(wireId, variantIndex, bodyPayload) {
  const innerPayload = Buffer.concat([
    u32ToLittleEndianBuffer(variantIndex),
    encodeNoritoField(bodyPayload),
  ]);
  return encodeInstructionEnvelope(wireId, innerPayload);
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
      throw new Error(`Pure JS Norito decode does not support Mint variant ${variantIndex}`);
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
      throw new Error(`Pure JS Norito decode does not support Burn variant ${variantIndex}`);
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
        `Pure JS Norito decode does not support Transfer variant ${variantIndex}.`,
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
    default:
      throw new Error(
        `Pure JS Norito decode does not support Register variant ${variantIndex}.`,
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
        "namespace",
        "contract_id",
        "code_hash_hex",
        "abi_hash_hex",
        "abi_version",
        "window",
        "mode",
        "limits",
      ]);
      const decoded = {
        namespace: decodeStringValue(fields.namespace, "ProposeDeployContract.namespace"),
        contract_id: decodeStringValue(fields.contract_id, "ProposeDeployContract.contract_id"),
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
        "namespace",
        "contract_id",
        "reason",
      ]);
      return {
        DeactivateContractInstance: {
          namespace: decodeStringValue(fields.namespace, "DeactivateContractInstance.namespace"),
          contract_id: decodeStringValue(
            fields.contract_id,
            "DeactivateContractInstance.contract_id",
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
        "namespace",
        "contract_id",
        "code_hash",
      ]);
      return {
        ActivateContractInstance: {
          namespace: decodeStringValue(fields.namespace, "ActivateContractInstance.namespace"),
          contract_id: decodeStringValue(
            fields.contract_id,
            "ActivateContractInstance.contract_id",
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
    case "iroha_data_model::isi::zk::Unshield": {
      const fields = decodeStructFields(payload, "zk.Unshield", [
        "asset",
        "to",
        "public_amount",
        "inputs",
        "proof",
        "root_hint",
      ]);
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
      throw new Error(`Pure JS Norito decode does not support RWA variant ${variantIndex}`);
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
  const length = reader.readLength("length");
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
    [encodeOptionValue(value.logo, encodeNoritoStringValue, `${context}.logo`)],
    [encodeMetadataValue(value.metadata ?? {}, `${context}.metadata`)],
  ]);
}

function decodeNewDomainValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["id", "logo", "metadata"]);
  return {
    id: decodeNestedValue(fields.id, decodeDomainIdValue, `${context}.id`),
    logo: decodeOptionValue(fields.logo, decodeStringValue, `${context}.logo`),
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
      encodeNoritoField(encodeNoritoJsonValue(json)),
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
        decodeNestedJsonValue(fields.value, `${context}[${index}].value`),
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
    `Pure JS Norito encoding does not support governance instruction ${describeInstructionShape(instruction)}`,
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
    `Pure JS Norito encoding does not support social instruction ${describeInstructionShape(instruction)}`,
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
            instruction.DeactivateContractInstance.namespace,
            "DeactivateContractInstance.namespace",
          ),
        )],
        [encodeNoritoStringValue(
          assertNonEmptyString(
            instruction.DeactivateContractInstance.contract_id,
            "DeactivateContractInstance.contract_id",
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
            instruction.ActivateContractInstance.namespace,
            "ActivateContractInstance.namespace",
          ),
        )],
        [encodeNoritoStringValue(
          assertNonEmptyString(
            instruction.ActivateContractInstance.contract_id,
            "ActivateContractInstance.contract_id",
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
    `Pure JS Norito encoding does not support smart-contract instruction ${describeInstructionShape(instruction)}`,
  );
}

function encodeProposeDeployContractPayload(value) {
  return encodeStructValue([
    [encodeNoritoStringValue(assertNonEmptyString(value.namespace, "ProposeDeployContract.namespace"))],
    [encodeNoritoStringValue(assertNonEmptyString(value.contract_id, "ProposeDeployContract.contract_id"))],
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
    `Pure JS Norito encoding does not support Kaigi instruction ${describeInstructionShape(instruction)}`,
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

function encodeZkInstruction(instruction) {
  const entries = [
    ["RegisterZkAsset", "iroha_data_model::isi::zk::RegisterZkAsset", encodeRegisterZkAssetPayload],
    ["ScheduleConfidentialPolicyTransition", "zk::ScheduleConfidentialPolicyTransition", encodeScheduleConfidentialPolicyTransitionPayload],
    ["CancelConfidentialPolicyTransition", "zk::CancelConfidentialPolicyTransition", encodeCancelConfidentialPolicyTransitionPayload],
    ["Shield", "iroha_data_model::isi::zk::Shield", encodeShieldPayload],
    ["ZkTransfer", "iroha_data_model::isi::zk::ZkTransfer", encodeZkTransferPayload],
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
    `Pure JS Norito encoding does not support zk instruction ${describeInstructionShape(instruction)}`,
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

function encodeUnshieldPayload(value) {
  return encodeStructValue([
    [encodeAssetDefinitionIdValue(value.asset, "zk.Unshield.asset")],
    [encodeAccountIdValue(value.to, "zk.Unshield.to")],
    [encodeU128Value(value.public_amount, "zk.Unshield.public_amount")],
    [encodeNoritoVec(value.inputs ?? [], (entry, index) =>
      encodeFixedByteArrayArchiveValue(entry, 32, `zk.Unshield.inputs[${index}]`),
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
    `Pure JS Norito encoding does not support RWA instruction ${describeInstructionShape(instruction)}`,
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
    encodeNoritoField(encodeNoritoStringValue(assertNonEmptyString(value.destination, `${context}.destination`))),
  ]);
}

function decodeTriggerRepetitionsBody(payload, context) {
  const reader = new BufferReader(payload, context);
  const object = decodeU32Value(readNoritoField(reader, "object"), `${context}.object`);
  const destination = decodeStringValue(
    readNoritoField(reader, "destination"),
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
        encodeNoritoField(encodePublicKeyPayload(controller, context)),
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
    const { curve, publicKey } = decodePublicKeyPayload(controllerPayload, context);
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

function encodePublicKeyPayload(controller, context) {
  return encodeNoritoStringValue(publicKeyLiteralFromParts(controller.curve, controller.publicKey, context));
}

function decodePublicKeyPayload(payload, context) {
  const literal = decodeStringValue(payload, context);
  return parsePublicKeyLiteral(literal, context);
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
    encodeNoritoField(
      encodeNoritoStringValue(publicKeyLiteralFromParts(member.curve, member.publicKey, context)),
    ),
    encodeNoritoField(encodeU16Value(member.weight, `${context}.weight`)),
  ]);
}

function decodeMultisigMemberPayload(payload, context) {
  const reader = new BufferReader(payload, context);
  const { curve, publicKey } = parsePublicKeyLiteral(
    decodeStringValue(readNoritoField(reader, "publicKey"), `${context}.publicKey`),
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
  const aidBytes = payload.subarray(1, 17);
  const parts = [];
  for (const byte of aidBytes) {
    parts.push(encodeNoritoField(Buffer.of(byte)));
  }
  return Buffer.concat(parts);
}

function decodeAssetDefinitionIdValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const aidBytes = [];
  while (reader.offset < reader.buffer.length) {
    const field = readNoritoField(reader, `byte${aidBytes.length}`);
    if (field.length !== 1) {
      throw new Error(`${context} byte${aidBytes.length} must be exactly one byte`);
    }
    aidBytes.push(field[0]);
  }
  if (aidBytes.length !== 16) {
    throw new Error(`${context} must contain exactly 16 UUID bytes`);
  }
  const payloadBytes = Buffer.concat([
    Buffer.from([ASSET_DEFINITION_ADDRESS_VERSION]),
    Buffer.from(aidBytes),
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
    encodeNoritoField(encodeU64Value(match[1], `${context}.dataspace`)),
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
    const dataspace = decodeU64Value(readNoritoField(reader, "dataspace"), `${context}.dataspace`);
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

function encodeEnumTagValue(index, encodePayload) {
  const payload = encodePayload ? encodeNoritoField(encodePayload()) : Buffer.alloc(0);
  return Buffer.concat([u32ToLittleEndianBuffer(index), payload]);
}

function encodeCouncilDerivationKindValue(value, context) {
  const normalized = assertNonEmptyString(value, context).toLowerCase();
  if (normalized === "vrf") {
    return encodeEnumTagValue(0);
  }
  if (normalized === "fallback") {
    return encodeEnumTagValue(1);
  }
  throw new Error(`${context} must be Vrf or Fallback`);
}

function decodeCouncilDerivationKindValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const tag = reader.readU32LE("tag");
  reader.assertEof();
  switch (tag) {
    case 0:
      return "Vrf";
    case 1:
      return "Fallback";
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

function encodeProofAttachmentValue(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  const parts = [
    encodeNoritoField(encodeNoritoStringValue(assertNonEmptyString(value.backend, `${context}.backend`))),
    encodeNoritoField(encodeProofBoxValue(value.proof, `${context}.proof`)),
    encodeNoritoField(encodeOptionValue(value.vk_ref, encodeVerifyingKeyIdValue, `${context}.vk_ref`)),
    encodeNoritoField(encodeOptionValue(value.vk_inline, encodeVerifyingKeyBoxValue, `${context}.vk_inline`)),
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
  const vk_ref = decodeOptionValue(
    readNoritoField(reader, "vk_ref"),
    decodeVerifyingKeyIdValue,
    `${context}.vk_ref`,
  );
  const vk_inline = decodeOptionValue(
    readNoritoField(reader, "vk_inline"),
    decodeVerifyingKeyBoxValue,
    `${context}.vk_inline`,
  );
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
    vk_inline,
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
    [encodeOptionValue(value.entrypoints ?? null, encodeUnsupportedJsonBackedOption, `${context}.entrypoints`)],
    [encodeOptionValue(value.kotoba ?? null, encodeUnsupportedJsonBackedOption, `${context}.kotoba`)],
    [encodeOptionValue(value.provenance ?? null, encodeUnsupportedJsonBackedOption, `${context}.provenance`)],
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
    entrypoints: decodeOptionValue(fields.entrypoints, decodeJsonValue, `${context}.entrypoints`),
    kotoba: decodeOptionValue(fields.kotoba, decodeJsonValue, `${context}.kotoba`),
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
    decodeJsonValue,
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
  ]);
}

function decodeAccessSetHintsValue(payload, context) {
  const fields = decodeStructFields(payload, context, ["read_keys", "write_keys"]);
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
  };
}

function encodeUnsupportedJsonBackedOption(value, context) {
  throw new Error(`${context} is not yet supported by the JS-only Norito fallback`);
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

function decodeStringValue(payload, context) {
  const reader = new BufferReader(payload, context);
  const stringBytes = readNoritoField(reader, "value");
  reader.assertEof();
  return stringBytes.toString("utf8");
}

function encodeNoritoJsonValue(value) {
  return encodeNoritoField(Buffer.from(canonicalJsonStringify(value), "utf8"));
}

function decodeJsonValue(payload, context) {
  return JSON.parse(decodeStringValue(payload, context));
}

function readNoritoField(reader, name) {
  const length = reader.readLength(`${name}.length`);
  return reader.readBytes(length, `${name}.payload`);
}

function encodeNoritoField(payload) {
  return Buffer.concat([u64ToLittleEndianBuffer(payload.length), payload]);
}

function encodeNoritoVec(values, encode) {
  const payloads = values.map(encode);
  const parts = [u64ToLittleEndianBuffer(payloads.length)];
  for (const payload of payloads) {
    parts.push(u64ToLittleEndianBuffer(payload.length), payload);
  }
  return Buffer.concat(parts);
}

function decodeNoritoVec(payload, decode, context) {
  const reader = new BufferReader(payload, context);
  const count = reader.readLength("count");
  const values = [];
  for (let index = 0; index < count; index += 1) {
    const itemLength = reader.readLength(`item${index}.length`);
    const itemPayload = reader.readBytes(itemLength, `item${index}.payload`);
    values.push(decode(itemPayload, index));
  }
  reader.assertEof();
  return values;
}

function looksLikeNoritoFrame(buffer) {
  return buffer.length >= 40 && buffer.subarray(0, 4).toString("ascii") === "NRT0";
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
  const payloadLength = reader.readLength("payloadLength");
  const expectedCrc = reader.readU64LE("payloadCrc");
  reader.readU8("flags");
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
  return { payload, schemaHash };
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
