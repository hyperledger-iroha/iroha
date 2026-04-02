"use strict";

import { Buffer } from "node:buffer";
import { domainToASCII } from "node:url";
import { blake2b256 } from "./blake2b.js";
import { getNativeBinding } from "./native.js";
const DEFAULT_I105_DISCRIMINANT = 0x02f1;
const I105_DISCRIMINANT_MAX = 0xffff;
const HEADER_VERSION_V1 = 0;
const HEADER_NORM_VERSION_V1 = 1;
const I105_SENTINEL_SORA = "sora";
const I105_SENTINEL_TEST = "test";
const I105_SENTINEL_DEV = "dev";
const I105_SENTINEL_NUMERIC_PREFIX = "n";
const I105_SENTINEL_SORA_FULLWIDTH = "ｓｏｒａ";
const I105_SENTINEL_TEST_FULLWIDTH = "ｔｅｓｔ";
const I105_SENTINEL_DEV_FULLWIDTH = "ｄｅｖ";
const I105_SENTINEL_NUMERIC_PREFIX_FULLWIDTH = "ｎ";
const I105_CHECKSUM_LEN = 6;
const BECH32M_CONST = 0x2bc830a3;
const I105_WARNING =
  "i105 addresses use the canonical I105 alphabet: Base58 plus the 47 katakana from the Iroha poem. Render and validate them with the intended chain discriminant.";

const MULTISIG_DIGEST_PERSONALIZATION = (() => {
  const bytes = new Uint8Array(16);
  bytes.set(Buffer.from("iroha-ms-policy", "ascii"));
  return bytes;
})();

let cachedNativeAddressCodec;
let nativeAddressCodecResolved = false;

const BASE58_ALPHABET = Array.from(
  "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz",
);
const IROHA_POEM_KANA_FULLWIDTH = [
  "イ", "ロ", "ハ", "ニ", "ホ", "ヘ", "ト", "チ", "リ", "ヌ", "ル", "ヲ", "ワ", "カ",
  "ヨ", "タ", "レ", "ソ", "ツ", "ネ", "ナ", "ラ", "ム", "ウ", "ヰ", "ノ", "オ", "ク",
  "ヤ", "マ", "ケ", "フ", "コ", "エ", "テ", "ア", "サ", "キ", "ユ", "メ", "ミ", "シ",
  "ヱ", "ヒ", "モ", "セ", "ス",
];
const IROHA_POEM_KANA_HALFWIDTH = [
  "ｲ", "ﾛ", "ﾊ", "ﾆ", "ﾎ", "ﾍ", "ﾄ", "ﾁ", "ﾘ", "ﾇ", "ﾙ", "ｦ", "ﾜ", "ｶ",
  "ﾖ", "ﾀ", "ﾚ", "ｿ", "ﾂ", "ﾈ", "ﾅ", "ﾗ", "ﾑ", "ｳ", "ヰ", "ﾉ", "ｵ", "ｸ",
  "ﾔ", "ﾏ", "ｹ", "ﾌ", "ｺ", "ｴ", "ﾃ", "ｱ", "ｻ", "ｷ", "ﾕ", "ﾒ", "ﾐ", "ｼ",
  "ヱ", "ﾋ", "ﾓ", "ｾ", "ｽ",
];
const I105_ALPHABET = [...BASE58_ALPHABET, ...IROHA_POEM_KANA_FULLWIDTH];
const I105_BASE = I105_ALPHABET.length;

export const AccountAddressErrorCode = Object.freeze({
  UNSUPPORTED_ALGORITHM: "ERR_UNSUPPORTED_ALGORITHM",
  KEY_PAYLOAD_TOO_LONG: "ERR_KEY_PAYLOAD_TOO_LONG",
  INVALID_HEADER_VERSION: "ERR_INVALID_HEADER_VERSION",
  INVALID_NORM_VERSION: "ERR_INVALID_NORM_VERSION",
  INVALID_I105_DISCRIMINANT: "ERR_INVALID_I105_DISCRIMINANT",
  CANONICAL_HASH_FAILURE: "ERR_CANONICAL_HASH_FAILURE",
  INVALID_LENGTH: "ERR_INVALID_LENGTH",
  CHECKSUM_MISMATCH: "ERR_CHECKSUM_MISMATCH",
  INVALID_HEX_ADDRESS: "ERR_INVALID_HEX_ADDRESS",
  DOMAIN_MISMATCH: "ERR_DOMAIN_MISMATCH",
  INVALID_DOMAIN_LABEL: "ERR_INVALID_DOMAIN_LABEL",
  INVALID_REGISTRY_ID: "ERR_INVALID_REGISTRY_ID",
  UNEXPECTED_NETWORK_PREFIX: "ERR_UNEXPECTED_NETWORK_PREFIX",
  UNKNOWN_ADDRESS_CLASS: "ERR_UNKNOWN_ADDRESS_CLASS",
  UNKNOWN_DOMAIN_TAG: "ERR_UNKNOWN_DOMAIN_TAG",
  UNEXPECTED_EXTENSION_FLAG: "ERR_UNEXPECTED_EXTENSION_FLAG",
  UNKNOWN_CONTROLLER_TAG: "ERR_UNKNOWN_CONTROLLER_TAG",
  INVALID_PUBLIC_KEY: "ERR_INVALID_PUBLIC_KEY",
  UNKNOWN_CURVE: "ERR_UNKNOWN_CURVE",
  UNEXPECTED_TRAILING_BYTES: "ERR_UNEXPECTED_TRAILING_BYTES",
  MISSING_I105_SENTINEL: "ERR_MISSING_I105_SENTINEL",
  I105_TOO_SHORT: "ERR_I105_TOO_SHORT",
  INVALID_I105_CHAR: "ERR_INVALID_I105_CHAR",
  INVALID_I105_BASE: "ERR_INVALID_I105_BASE",
  INVALID_I105_DIGIT: "ERR_INVALID_I105_DIGIT",
  LOCAL_DIGEST_TOO_SHORT: "ERR_LOCAL8_DEPRECATED",
  UNSUPPORTED_ADDRESS_FORMAT: "ERR_UNSUPPORTED_ADDRESS_FORMAT",
  MULTISIG_MEMBER_OVERFLOW: "ERR_MULTISIG_MEMBER_OVERFLOW",
  INVALID_MULTISIG_POLICY: "ERR_INVALID_MULTISIG_POLICY",
});
const ACCOUNT_ADDRESS_ERROR_CODES = new Set(Object.values(AccountAddressErrorCode));

export class AccountAddressError extends Error {
  constructor(code, message, options = {}) {
    super(message);
    this.name = "AccountAddressError";
    this.code = code;
    if (options.details !== undefined) {
      this.details = options.details;
    }
    if (options.cause !== undefined) {
      this.cause = options.cause;
    }
  }
}

const AddressClass = Object.freeze({
  SINGLE_KEY: 0,
  MULTI_SIG: 1,
});

const CONTROLLER_TAG_SINGLE = 0x00;
const CONTROLLER_TAG_MULTISIG = 0x01;
const MULTISIG_MEMBER_MAX = 0xff;
const MULTISIG_POLICY_VERSION = 1;
const HEX_BODY_RE = /^[0-9a-fA-F]+$/;

const CurveFeature = Object.freeze({
  NONE: null,
  ML_DSA: "ml-dsa",
  GOST: "gost",
  SM2: "sm",
});

const CurveId = Object.freeze({
  ED25519: 1,
  MLDSA: 2,
  GOST_256_A: 10,
  GOST_256_B: 11,
  GOST_256_C: 12,
  GOST_512_A: 13,
  GOST_512_B: 14,
  SM2: 15,
});

const CURVE_REGISTRY = [
  { id: CurveId.ED25519, feature: CurveFeature.NONE, aliases: ["ed25519", "ed"] },
  { id: CurveId.MLDSA, feature: CurveFeature.ML_DSA, aliases: ["ml-dsa", "mldsa", "ml_dsa"] },
  {
    id: CurveId.GOST_256_A,
    feature: CurveFeature.GOST,
    aliases: ["gost256a", "gost-256-a"],
  },
  {
    id: CurveId.GOST_256_B,
    feature: CurveFeature.GOST,
    aliases: ["gost256b", "gost-256-b"],
  },
  {
    id: CurveId.GOST_256_C,
    feature: CurveFeature.GOST,
    aliases: ["gost256c", "gost-256-c"],
  },
  {
    id: CurveId.GOST_512_A,
    feature: CurveFeature.GOST,
    aliases: ["gost512a", "gost-512-a"],
  },
  {
    id: CurveId.GOST_512_B,
    feature: CurveFeature.GOST,
    aliases: ["gost512b", "gost-512-b"],
  },
  { id: CurveId.SM2, feature: CurveFeature.SM2, aliases: ["sm2", "sm-2"] },
];

const ED25519_FIELD_MODULUS = BigInt("0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffed");
const ED25519_SMALL_ORDER_ENCODINGS = [
  "0100000000000000000000000000000000000000000000000000000000000000",
  "c7176a703d4dd84fba3c0b760d10670f2a2053fa2c39ccc64ec7fd7792ac037a",
  "0000000000000000000000000000000000000000000000000000000000000080",
  "26e8958fc2b227b045c3f489f2ef98f0d5dfac05d3c63339b13802886d53fc05",
  "ecffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
  "26e8958fc2b227b045c3f489f2ef98f0d5dfac05d3c63339b13802886d53fc85",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "c7176a703d4dd84fba3c0b760d10670f2a2053fa2c39ccc64ec7fd7792ac03fa",
].map((hex) => normalizeBytes(hex));

const CURVE_NAME_TO_ENTRY = new Map();
const CURVE_ID_TO_ENTRY = new Map();
for (const entry of CURVE_REGISTRY) {
  CURVE_ID_TO_ENTRY.set(entry.id, entry);
  for (const alias of entry.aliases) {
    CURVE_NAME_TO_ENTRY.set(alias, entry);
  }
}

let enabledCurveIds = new Set([CurveId.ED25519]);
let enabledFeatures = new Set();

const CURVE_PUBLIC_KEY_LENGTH = new Map([
  [CurveId.ED25519, 32],
  [CurveId.MLDSA, 32],
  [CurveId.GOST_256_A, 64],
  [CurveId.GOST_256_B, 64],
  [CurveId.GOST_256_C, 64],
  [CurveId.GOST_512_A, 128],
  [CurveId.GOST_512_B, 128],
  [CurveId.SM2, 65],
]);

function normalizeCurveSupportOptions(options) {
  if (options === undefined) {
    return { allowMlDsa: false, allowGost: false, allowSm2: false };
  }
  if (!isPlainObject(options)) {
    throw new TypeError("configureCurveSupport options must be an object");
  }
  const allowedKeys = new Set(["allowMlDsa", "allowGost", "allowSm2"]);
  const extras = Object.keys(options).filter((key) => !allowedKeys.has(key));
  if (extras.length > 0) {
    throw new TypeError(
      `configureCurveSupport options contains unsupported fields: ${extras.join(", ")}`,
    );
  }
  if (
    Object.prototype.hasOwnProperty.call(options, "allowMlDsa") &&
    typeof options.allowMlDsa !== "boolean"
  ) {
    throw new TypeError("configureCurveSupport options.allowMlDsa must be a boolean");
  }
  if (
    Object.prototype.hasOwnProperty.call(options, "allowGost") &&
    typeof options.allowGost !== "boolean"
  ) {
    throw new TypeError("configureCurveSupport options.allowGost must be a boolean");
  }
  if (
    Object.prototype.hasOwnProperty.call(options, "allowSm2") &&
    typeof options.allowSm2 !== "boolean"
  ) {
    throw new TypeError("configureCurveSupport options.allowSm2 must be a boolean");
  }
  return {
    allowMlDsa: options.allowMlDsa === true,
    allowGost: options.allowGost === true,
    allowSm2: options.allowSm2 === true,
  };
}

export function configureCurveSupport(options) {
  const { allowMlDsa, allowGost, allowSm2 } = normalizeCurveSupportOptions(options);
  enabledCurveIds = new Set([CurveId.ED25519]);
  enabledFeatures = new Set();
  if (allowMlDsa) {
    enabledCurveIds.add(CurveId.MLDSA);
    enabledFeatures.add(CurveFeature.ML_DSA);
  }
  if (allowGost) {
    enabledFeatures.add(CurveFeature.GOST);
    enabledCurveIds.add(CurveId.GOST_256_A);
    enabledCurveIds.add(CurveId.GOST_256_B);
    enabledCurveIds.add(CurveId.GOST_256_C);
    enabledCurveIds.add(CurveId.GOST_512_A);
    enabledCurveIds.add(CurveId.GOST_512_B);
  }
  if (allowSm2) {
    enabledCurveIds.add(CurveId.SM2);
    enabledFeatures.add(CurveFeature.SM2);
  }
}

configureCurveSupport();

function isFeatureEnabled(feature) {
  return feature === CurveFeature.NONE || enabledFeatures.has(feature);
}

function ensureCurveEnabled(entry, context) {
  if (!isFeatureEnabled(entry.feature)) {
    const label = entry.aliases[0];
    throw new AccountAddressError(
      AccountAddressErrorCode.UNSUPPORTED_ALGORITHM,
      `${context ?? "curve"} disabled by configuration: ${label}`,
      { details: { feature: entry.feature, label } },
    );
  }
}

function ensureCurveIdEnabled(curveId, context) {
  const entry = CURVE_ID_TO_ENTRY.get(curveId);
  if (!entry) {
    throw new AccountAddressError(
      AccountAddressErrorCode.UNKNOWN_CURVE,
      `unknown curve id: ${curveId}`,
    );
  }
  ensureCurveEnabled(entry, context ?? `curve id ${curveId}`);
  return entry;
}

function bytesEqual(lhs, rhs) {
  if (lhs.length !== rhs.length) {
    return false;
  }
  for (let index = 0; index < lhs.length; index += 1) {
    if (lhs[index] !== rhs[index]) {
      return false;
    }
  }
  return true;
}

function ed25519CanonicalYCoordinate(keyBytes) {
  const copy = Uint8Array.from(keyBytes);
  copy[copy.length - 1] &= 0x7f;
  let acc = 0n;
  for (let index = copy.length - 1; index >= 0; index -= 1) {
    acc = (acc << 8n) | BigInt(copy[index]);
  }
  return acc;
}

function assertEd25519CanonicalEncoding(keyBytes, context) {
  const y = ed25519CanonicalYCoordinate(keyBytes);
  if (y >= ED25519_FIELD_MODULUS) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_PUBLIC_KEY,
      `non-canonical ed25519 ${context}`,
      { details: { curveId: CurveId.ED25519 } },
    );
  }
}

function assertEd25519NotSmallOrder(keyBytes, context) {
  const isSmallOrder = ED25519_SMALL_ORDER_ENCODINGS.some((candidate) =>
    bytesEqual(candidate, keyBytes),
  );
  if (isSmallOrder) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_PUBLIC_KEY,
      `ed25519 ${context} is small-order (weak); rejected`,
      { details: { curveId: CurveId.ED25519 } },
    );
  }
}

function validatePublicKeyForCurve(curveId, keyBytes, context = "public key") {
  const entry = ensureCurveIdEnabled(curveId, context);
  const expectedLength = CURVE_PUBLIC_KEY_LENGTH.get(entry.id);
  if (expectedLength === undefined) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_PUBLIC_KEY,
      `no validation rule for curve id ${entry.id}`,
      { details: { curveId: entry.id, length: keyBytes.length } },
    );
  }
  if (keyBytes.length !== expectedLength) {
    const label = entry.aliases?.[0] ?? `curve id ${entry.id}`;
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_PUBLIC_KEY,
      `invalid ${label} ${context}: expected ${expectedLength} bytes`,
      { details: { curveId: entry.id, length: keyBytes.length, expectedLength } },
    );
  }
  if (entry.id === CurveId.ED25519) {
    assertEd25519CanonicalEncoding(keyBytes, context);
    assertEd25519NotSmallOrder(keyBytes, context);
  }
}

function invalidMultisigPolicy(policyError, message, extraDetails) {
  const details = { policyError, ...extraDetails };
  throw new AccountAddressError(
    AccountAddressErrorCode.INVALID_MULTISIG_POLICY,
    message ?? `invalid multisig policy: ${policyError}`,
    { details },
  );
}

function normalizePolicyU16(value, context) {
  if (
    typeof value !== "number" ||
    !Number.isFinite(value) ||
    !Number.isInteger(value) ||
    value < 0 ||
    value > 0xffff
  ) {
    throw new TypeError(`${context} must be a 16-bit unsigned integer`);
  }
  return value;
}

function normalizeMultisigMembers(members) {
  if (!Array.isArray(members) || members.length === 0) {
    invalidMultisigPolicy("EmptyMembers", "invalid multisig policy: EmptyMembers");
  }
  const normalized = members.map((member, index) => {
    const curve = member.curve ?? CurveId.ED25519;
    ensureCurveIdEnabled(curve, `multisig member ${index}`);
    const weight = member.weight ?? 0;
    if (!Number.isFinite(weight) || !Number.isInteger(weight)) {
      throw new TypeError("multisig member weight must be an integer");
    }
    if (weight === 0) {
      invalidMultisigPolicy("MemberWeightZero", "invalid multisig policy: MemberWeightZero");
    }
    if (weight < 0 || weight > 0xffff) {
      throw new TypeError("multisig member weight must fit in a 16-bit unsigned integer");
    }
    const publicKey = normalizeBytes(member.publicKey);
    validatePublicKeyForCurve(curve, publicKey, `multisig member ${index} public key`);
    const sortKey = Buffer.concat([
      Buffer.from(curveIdToAlgorithm(curve), "ascii"),
      Buffer.from([0]),
      Buffer.from(publicKey),
    ]);
    return { curve, weight, publicKey, sortKey };
  });
  normalized.sort((left, right) => left.sortKey.compare(right.sortKey));
  for (let index = 1; index < normalized.length; index += 1) {
    if (normalized[index].sortKey.compare(normalized[index - 1].sortKey) === 0) {
      invalidMultisigPolicy("DuplicateMember", "invalid multisig policy: DuplicateMember");
    }
  }
  return normalized.map(({ sortKey, ...rest }) => rest);
}

function validateAndNormalizeMultisigController(controller) {
  const version = normalizePolicyU16(
    controller.version ?? MULTISIG_POLICY_VERSION,
    "multisig policy version",
  );
  if (version !== MULTISIG_POLICY_VERSION) {
    invalidMultisigPolicy("UnsupportedVersion", "invalid multisig policy: UnsupportedVersion", {
      version,
    });
  }
  const memberEntries = controller.members ?? [];
  if (memberEntries.length === 0) {
    invalidMultisigPolicy("EmptyMembers", "invalid multisig policy: EmptyMembers");
  }
  if (memberEntries.length > MULTISIG_MEMBER_MAX) {
    throw new AccountAddressError(
      AccountAddressErrorCode.MULTISIG_MEMBER_OVERFLOW,
      `multisig member overflow: ${memberEntries.length} entries exceeds ${MULTISIG_MEMBER_MAX}`,
      { details: { count: memberEntries.length, limit: MULTISIG_MEMBER_MAX } },
    );
  }
  const threshold = normalizePolicyU16(
    controller.threshold ?? 0,
    "multisig policy threshold",
  );
  if (threshold === 0) {
    invalidMultisigPolicy("ZeroThreshold", "invalid multisig policy: ZeroThreshold");
  }
  const members = normalizeMultisigMembers(memberEntries);
  const totalWeight = members.reduce((acc, member) => acc + member.weight, 0);
  if (threshold > totalWeight) {
    invalidMultisigPolicy(
      "ThresholdExceedsTotal",
      "invalid multisig policy: ThresholdExceedsTotal",
      { threshold, totalWeight },
    );
  }

  return {
    tag: CONTROLLER_TAG_MULTISIG,
    version,
    threshold,
    members,
  };
}

function normalizeBytes(value) {
  if (value instanceof Uint8Array) {
    return new Uint8Array(value);
  }
  if (Buffer.isBuffer(value)) {
    return new Uint8Array(value);
  }
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    return new Uint8Array(value);
  }
  if (Array.isArray(value)) {
    const out = new Uint8Array(value.length);
    for (let index = 0; index < value.length; index += 1) {
      const byte = value[index];
      if (
        typeof byte !== "number" ||
        !Number.isFinite(byte) ||
        !Number.isInteger(byte) ||
        byte < 0 ||
        byte > 0xff
      ) {
        throw new TypeError("byte array entries must be integers between 0 and 255");
      }
      out[index] = byte;
    }
    return out;
  }
  if (typeof value === "string") {
    const trimmed = value.trim();
    const body =
      trimmed.startsWith("0x") || trimmed.startsWith("0X") ? trimmed.slice(2) : trimmed;
    if (body.length === 0 || body.length % 2 !== 0 || !/^[0-9a-fA-F]+$/.test(body)) {
      throw new TypeError("hex string inputs must be even-length and contain only hex digits");
    }
    return new Uint8Array(Buffer.from(body, "hex"));
  }
  throw new TypeError(
    "expected Uint8Array, Buffer, ArrayBuffer, ArrayBufferView, number[], or hex string for byte data",
  );
}

function blake2b256Personalized(data, personalization, includeZeroKeyBlock = false) {
  const normalized = normalizeBytes(data);
  const options = {
    includeZeroKeyBlock: includeZeroKeyBlock === true,
  };
  if (personalization !== undefined && personalization !== null) {
    options.personalization = normalizeBytes(personalization);
  }
  return blake2b256(normalized, options);
}

function curveIdFromAlgorithm(algorithm) {
  const key = String(algorithm).trim().toLowerCase();
  const entry = CURVE_NAME_TO_ENTRY.get(key);
  if (!entry) {
    throw new AccountAddressError(
      AccountAddressErrorCode.UNSUPPORTED_ALGORITHM,
      `unsupported signing algorithm: ${algorithm}`,
      { details: { algorithm } },
    );
  }
  ensureCurveEnabled(entry, `signing algorithm ${algorithm}`);
  return entry.id;
}

function encodeHeader({ version, classId, normVersion, extFlag }) {
  let byte = ((version & 0b111) << 5) | ((classId & 0b11) << 3);
  byte |= (normVersion & 0b11) << 1;
  byte |= extFlag ? 1 : 0;
  return byte;
}

function decodeHeader(byte) {
  const version = (byte >> 5) & 0b111;
  const classBits = (byte >> 3) & 0b11;
  const normVersion = (byte >> 1) & 0b11;
  const extFlag = (byte & 0b1) === 1;
  if (version > 0b111) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_HEADER_VERSION,
      `invalid address header version: ${version}`,
    );
  }
  if (normVersion > 0b11) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_NORM_VERSION,
      `invalid normalization version: ${normVersion}`,
    );
  }
  if (!Object.values(AddressClass).includes(classBits)) {
    throw new AccountAddressError(
      AccountAddressErrorCode.UNKNOWN_ADDRESS_CLASS,
      `unknown address class: ${classBits}`,
    );
  }
  if (extFlag) {
    throw new AccountAddressError(
      AccountAddressErrorCode.UNEXPECTED_EXTENSION_FLAG,
      "unexpected address header extension flag",
    );
  }
  return { version, classId: classBits, normVersion, extFlag };
}

function encodeController(controller) {
  if (controller.tag === CONTROLLER_TAG_SINGLE) {
    validatePublicKeyForCurve(controller.curve, controller.publicKey, "controller public key");
    if (controller.publicKey.length > 0xff) {
      throw new AccountAddressError(
        AccountAddressErrorCode.KEY_PAYLOAD_TOO_LONG,
        `key payload too long: ${controller.publicKey.length} bytes`,
        { details: { length: controller.publicKey.length } },
      );
    }
    const out = new Uint8Array(3 + controller.publicKey.length);
    out[0] = controller.tag;
    out[1] = controller.curve;
    out[2] = controller.publicKey.length;
    out.set(controller.publicKey, 3);
    return out;
  }

  if (controller.tag === CONTROLLER_TAG_MULTISIG) {
    const normalized = validateAndNormalizeMultisigController(controller);
    const members = normalized.members;
    if (members.length > MULTISIG_MEMBER_MAX) {
      throw new AccountAddressError(
        AccountAddressErrorCode.MULTISIG_MEMBER_OVERFLOW,
        `multisig member overflow: ${members.length} entries exceeds ${MULTISIG_MEMBER_MAX}`,
        { details: { count: members.length, limit: MULTISIG_MEMBER_MAX } },
      );
    }
    const parts = [];
    parts.push(normalized.tag);
    parts.push(normalized.version);
    const threshold = normalized.threshold;
    parts.push((threshold >> 8) & 0xff, threshold & 0xff);
    parts.push(members.length);
    for (const member of members) {
      const curve = member.curve ?? CurveId.ED25519;
      ensureCurveIdEnabled(curve, "multisig member");
      parts.push(curve);
      const weight = member.weight ?? 0;
      parts.push((weight >> 8) & 0xff, weight & 0xff);
      const keyBytes = normalizeBytes(member.publicKey);
      validatePublicKeyForCurve(curve, keyBytes, "multisig member public key");
      if (keyBytes.length > 0xffff) {
        throw new AccountAddressError(
          AccountAddressErrorCode.KEY_PAYLOAD_TOO_LONG,
          `key payload too long: ${keyBytes.length} bytes`,
          { details: { length: keyBytes.length } },
        );
      }
      parts.push((keyBytes.length >> 8) & 0xff, keyBytes.length & 0xff);
      parts.push(...keyBytes);
    }
    return Uint8Array.from(parts);
  }

  throw new AccountAddressError(AccountAddressErrorCode.UNKNOWN_CONTROLLER_TAG, "unsupported controller payload variant");
}

function decodeController(bytes, cursor) {
  if (cursor >= bytes.length) {
    throw new AccountAddressError(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for address payload");
  }
  const tag = bytes[cursor];
  cursor += 1;
  if (tag === CONTROLLER_TAG_SINGLE) {
    if (cursor >= bytes.length) {
      throw new AccountAddressError(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for address payload");
    }
    const curve = bytes[cursor];
    cursor += 1;
    const curveId = curveIdFromByte(curve);
    if (cursor >= bytes.length) {
      throw new AccountAddressError(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for address payload");
    }
    const length = bytes[cursor];
    cursor += 1;
    const end = cursor + length;
    if (end > bytes.length) {
      throw new AccountAddressError(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for address payload");
    }
    const publicKey = bytes.slice(cursor, end);
    validatePublicKeyForCurve(curveId, publicKey, "controller public key");
    return [{ tag, curve: curveId, publicKey }, end];
  }

  if (tag === CONTROLLER_TAG_MULTISIG) {
    if (cursor >= bytes.length) {
      throw new AccountAddressError(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for address payload");
    }
    const version = bytes[cursor];
    cursor += 1;
    if (cursor + 1 >= bytes.length) {
      throw new AccountAddressError(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for address payload");
    }
    const threshold = (bytes[cursor] << 8) | bytes[cursor + 1];
    cursor += 2;
    if (cursor >= bytes.length) {
      throw new AccountAddressError(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for address payload");
    }
    const memberCount = bytes[cursor];
    cursor += 1;
    if (memberCount > MULTISIG_MEMBER_MAX) {
      throw new AccountAddressError(
        AccountAddressErrorCode.MULTISIG_MEMBER_OVERFLOW,
        `multisig member overflow: ${memberCount} entries exceeds ${MULTISIG_MEMBER_MAX}`,
        { details: { count: memberCount, limit: MULTISIG_MEMBER_MAX } },
      );
    }
    const members = [];
    for (let index = 0; index < memberCount; index += 1) {
      if (cursor >= bytes.length) {
        throw new AccountAddressError(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for address payload");
      }
      const curve = curveIdFromByte(bytes[cursor]);
      cursor += 1;
      if (cursor + 1 >= bytes.length) {
        throw new AccountAddressError(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for address payload");
      }
      const weight = (bytes[cursor] << 8) | bytes[cursor + 1];
      cursor += 2;
      if (cursor + 1 >= bytes.length) {
        throw new AccountAddressError(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for address payload");
      }
      const keyLength = (bytes[cursor] << 8) | bytes[cursor + 1];
      cursor += 2;
      const end = cursor + keyLength;
      if (end > bytes.length) {
        throw new AccountAddressError(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for address payload");
      }
      const publicKey = bytes.slice(cursor, end);
      cursor = end;
      validatePublicKeyForCurve(curve, publicKey, "multisig member public key");
      members.push({ curve, weight, publicKey });
    }
    const normalized = validateAndNormalizeMultisigController({
      tag,
      version,
      threshold,
      members,
    });
    return [normalized, cursor];
  }

  throw new AccountAddressError(
    AccountAddressErrorCode.UNKNOWN_CONTROLLER_TAG,
    `unknown controller payload tag: ${tag}`,
  );
}

function curveIdFromByte(value) {
  const entry = ensureCurveIdEnabled(value);
  return entry.id;
}

function encodeMultisigPolicyCbor(controller) {
  const { version, threshold, members } = validateAndNormalizeMultisigController(controller);
  const sortedMembers = members;

  const parts = [];
  cborAppendLength(parts, 0b101, 3);
  cborAppendUnsigned(parts, 0x01);
  cborAppendUnsigned(parts, version);
  cborAppendUnsigned(parts, 0x02);
  cborAppendUnsigned(parts, threshold);
  cborAppendUnsigned(parts, 0x03);
  cborAppendLength(parts, 0b100, sortedMembers.length);
  for (const member of sortedMembers) {
    cborAppendLength(parts, 0b101, 3);
    cborAppendUnsigned(parts, 0x01);
    cborAppendUnsigned(parts, member.curve);
    cborAppendUnsigned(parts, 0x02);
    cborAppendUnsigned(parts, member.weight ?? 0);
    cborAppendUnsigned(parts, 0x03);
    cborAppendBytes(parts, member.publicKey);
  }
  return Uint8Array.from(parts);
}

function cborAppendUnsigned(parts, value) {
  if (value >= 0 && value <= 23) {
    parts.push(value);
  } else if (value <= 0xff) {
    parts.push(0x18, value);
  } else if (value <= 0xffff) {
    parts.push(0x19, (value >> 8) & 0xff, value & 0xff);
  } else if (value <= 0xffff_ffff) {
    parts.push(
      0x1a,
      (value >> 24) & 0xff,
      (value >> 16) & 0xff,
      (value >> 8) & 0xff,
      value & 0xff,
    );
  } else {
    const hi = Math.floor(value / 0x1_0000_0000);
    const lo = value & 0xffff_ffff;
    parts.push(
      0x1b,
      (hi >> 24) & 0xff,
      (hi >> 16) & 0xff,
      (hi >> 8) & 0xff,
      hi & 0xff,
      (lo >> 24) & 0xff,
      (lo >> 16) & 0xff,
      (lo >> 8) & 0xff,
      lo & 0xff,
    );
  }
}

function cborAppendLength(parts, major, length) {
  const base = major << 5;
  if (length >= 0 && length <= 23) {
    parts.push(base | length);
  } else if (length <= 0xff) {
    parts.push(base | 24, length);
  } else if (length <= 0xffff) {
    parts.push(base | 25, (length >> 8) & 0xff, length & 0xff);
  } else if (length <= 0xffff_ffff) {
    parts.push(
      base | 26,
      (length >> 24) & 0xff,
      (length >> 16) & 0xff,
      (length >> 8) & 0xff,
      length & 0xff,
    );
  } else {
    const hi = Math.floor(length / 0x1_0000_0000);
    const lo = length & 0xffff_ffff;
    parts.push(
      base | 27,
      (hi >> 24) & 0xff,
      (hi >> 16) & 0xff,
      (hi >> 8) & 0xff,
      hi & 0xff,
      (lo >> 24) & 0xff,
      (lo >> 16) & 0xff,
      (lo >> 8) & 0xff,
      lo & 0xff,
    );
  }
}

function cborAppendBytes(parts, bytes) {
  cborAppendLength(parts, 0b010, bytes.length);
  parts.push(...bytes);
}

function curveIdToAlgorithm(curveId) {
  const entry = ensureCurveIdEnabled(curveId, `curve id ${curveId}`);
  return entry.aliases[0];
}

function bytesToHex(bytes) {
  return Buffer.from(bytes)
    .toString("hex")
    .toUpperCase();
}

function computeMultisigPolicyDigest(bytes) {
  return blake2b256Personalized(bytes, MULTISIG_DIGEST_PERSONALIZATION, true);
}

export function canonicalizeDomainLabel(domain) {
  if (typeof domain !== "string") {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_DOMAIN_LABEL,
      "domain label must be a string",
    );
  }
  const trimmed = domain.trim();
  if (trimmed.length === 0) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_DOMAIN_LABEL,
      "domain label must be a non-empty string",
    );
  }
  if (/\s/.test(trimmed)) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_DOMAIN_LABEL,
      "domain label must not contain whitespace",
    );
  }
  if (/[@#$]/.test(trimmed)) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_DOMAIN_LABEL,
      "domain label must not contain reserved address characters",
    );
  }

  let normalized;
  try {
    normalized = trimmed.normalize("NFC");
  } catch (error) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_DOMAIN_LABEL,
      "domain label normalization failed",
      { cause: error },
    );
  }

  let ascii;
  try {
    ascii = domainToASCII(normalized);
  } catch (error) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_DOMAIN_LABEL,
      "domain label failed UTS-46 ASCII canonicalization",
      { cause: error },
    );
  }
  if (typeof ascii !== "string" || ascii.length === 0) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_DOMAIN_LABEL,
      "domain label failed UTS-46 ASCII canonicalization",
    );
  }

  const canonical = ascii.toLowerCase();
  if (canonical.length === 0 || canonical.length > 63) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_DOMAIN_LABEL,
      "domain label length must be between 1 and 63 characters",
    );
  }
  if (canonical.startsWith("-") || canonical.endsWith("-")) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_DOMAIN_LABEL,
      "domain label must not start or end with a hyphen",
    );
  }
  if (
    canonical.length >= 4 &&
    canonical[2] === "-" &&
    canonical[3] === "-" &&
    !canonical.startsWith("xn--")
  ) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_DOMAIN_LABEL,
      "domain label must not contain a double hyphen in the third and fourth position",
    );
  }
  if (canonical.includes("_")) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_DOMAIN_LABEL,
      "domain label must not include underscores (STD3 restriction)",
    );
  }
  for (let index = 0; index < canonical.length; index += 1) {
    const code = canonical.charCodeAt(index);
    const isDigit = code >= 0x30 && code <= 0x39;
    const isLower = code >= 0x61 && code <= 0x7a;
    const isAllowed = isDigit || isLower || canonical[index] === "-";
    if (!isAllowed) {
      throw new AccountAddressError(
        AccountAddressErrorCode.INVALID_DOMAIN_LABEL,
        "domain label contains unsupported characters",
      );
    }
  }
  return canonical;
}

function canonicalizeDomainName(domain) {
  if (typeof domain !== "string") {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_DOMAIN_LABEL,
      "domain label must be a string",
    );
  }
  const trimmed = domain.trim();
  if (trimmed.length === 0) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_DOMAIN_LABEL,
      "domain label must be a non-empty string",
    );
  }
  const labels = trimmed.split(".");
  if (labels.some((label) => label.length === 0)) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_DOMAIN_LABEL,
      "domain label must not contain empty segments",
    );
  }
  return labels.map((label) => canonicalizeDomainLabel(label)).join(".");
}

export class AccountAddress {
  constructor(header, controller) {
    this._header = header;
    this._controller = controller;
  }

  static fromAccount(options) {
    if (!isPlainObject(options)) {
      throw new TypeError("AccountAddress.fromAccount options must be an object");
    }
    const allowedKeys = new Set(["publicKey", "algorithm"]);
    const extras = Object.keys(options).filter((key) => !allowedKeys.has(key));
    if (extras.length > 0) {
      throw new TypeError(
        `AccountAddress.fromAccount options contains unsupported fields: ${extras.join(", ")}`,
      );
    }
    const { publicKey, algorithm = "ed25519" } = options;
    const header = {
      version: HEADER_VERSION_V1,
      classId: AddressClass.SINGLE_KEY,
      normVersion: HEADER_NORM_VERSION_V1,
      extFlag: false,
    };
    const curve = curveIdFromAlgorithm(algorithm);
    const keyBytes = normalizeBytes(publicKey);
    validatePublicKeyForCurve(curve, keyBytes, "public key");
    const controller = { tag: CONTROLLER_TAG_SINGLE, curve, publicKey: keyBytes };
    return new AccountAddress(header, controller);
  }

  static fromCanonicalBytes(bytes) {
    if (!bytes || bytes.length === 0) {
      throw new AccountAddressError(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for address payload");
    }
    const data = Uint8Array.from(bytes);
    const header = decodeHeader(data[0]);
    let controllerCursor = 1;
    const [controller, decodedCursor] = decodeController(data, controllerCursor);
    controllerCursor = decodedCursor;
    if (controllerCursor !== data.length) {
      throw new AccountAddressError(
        AccountAddressErrorCode.UNEXPECTED_TRAILING_BYTES,
        "unexpected trailing bytes in canonical payload",
      );
    }
    return new AccountAddress(header, controller);
  }

  static fromI105(encoded, expectedPrefix) {
    const literal = typeof encoded === "string" ? encoded.trim() : encoded;
    const normalizedExpectedDiscriminant =
      expectedPrefix === undefined
        ? undefined
        : normalizeI105DiscriminantInput(
            expectedPrefix,
            "AccountAddress.fromI105 expectedPrefix",
          );
    const nativeParsed = parseWithNativeCodec(
      encoded,
      normalizedExpectedDiscriminant,
    );
    if (nativeParsed) {
      const address = AccountAddress.fromCanonicalBytes(nativeParsed.canonicalBytes);
      assertCanonicalI105Literal(literal, address);
      return address;
    }
    const [, canonical] = decodeSupportedI105String(
      encoded,
      normalizedExpectedDiscriminant,
    );
    const address = AccountAddress.fromCanonicalBytes(canonical);
    assertCanonicalI105Literal(literal, address);
    return address;
  }

  static parseEncoded(input, expectedPrefix) {
    if (typeof input !== "string") {
      throw new TypeError("account address literal must be a string");
    }
    const trimmed = input.trim();
    if (trimmed.length === 0) {
      throw new AccountAddressError(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for address payload");
    }
    if (trimmed.includes("@")) {
      throw new AccountAddressError(
        AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
        "account address literals must not include '@domain'; use canonical I105 form",
      );
    }
    if (isCanonicalHexLiteral(trimmed)) {
      throw new AccountAddressError(
        AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
        "canonical hex account addresses are not accepted; use canonical I105 form",
      );
    }
    try {
      const address = AccountAddress.fromI105(trimmed, expectedPrefix);
      return {
        address,
        chainDiscriminant: tryExtractI105Discriminant(trimmed),
      };
    } catch (error) {
      if (error instanceof AccountAddressError) {
        if (error.code !== AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT) {
          throw error;
        }
        throw new AccountAddressError(
          AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
          "unsupported address format",
          { cause: error, details: { causeCode: error.code } },
        );
      }
      throw error;
    }
  }

  canonicalBytes() {
    const headerByte = encodeHeader(this._header);
    const header = Uint8Array.of(headerByte);
    const controller = encodeController(this._controller);
    const out = new Uint8Array(header.length + controller.length);
    out.set(header, 0);
    out.set(controller, header.length);
    return out;
  }

  canonicalHex() {
    const canonical = this.canonicalBytes();
    const rendered = renderWithNativeCodec(canonical, DEFAULT_I105_DISCRIMINANT);
    if (rendered?.canonicalHex) {
      return rendered.canonicalHex;
    }
    return `0x${Buffer.from(canonical).toString("hex")}`;
  }

  toI105(prefix = DEFAULT_I105_DISCRIMINANT) {
    const normalizedDiscriminant = normalizeI105DiscriminantInput(
      prefix,
      "AccountAddress.toI105 chainDiscriminant",
    );
    const canonical = this.canonicalBytes();
    const rendered = renderWithNativeCodec(canonical, normalizedDiscriminant);
    if (rendered?.i105) {
      return rendered.i105;
    }
    return encodeI105String(normalizedDiscriminant, canonical);
  }

  toString() {
    return this.toI105();
  }

  /**
   * Convenience helper that returns canonical I105 plus chain discriminant metadata.
   *
   * @param {number|bigint|string} chainDiscriminant - Chain discriminant (defaults to Sora `753`);
   * accepts numeric strings that will be normalized.
   * @returns {{ i105: string, chainDiscriminant: number, i105Warning: string }}
   */
  displayFormats(chainDiscriminant = DEFAULT_I105_DISCRIMINANT) {
    const normalizedDiscriminant = normalizeI105DiscriminantInput(
      chainDiscriminant,
      "AccountAddress.displayFormats chainDiscriminant",
    );
    const i105 = this.toI105(normalizedDiscriminant);
    return Object.freeze({
      i105,
      chainDiscriminant: normalizedDiscriminant,
      i105Warning: I105_WARNING,
    });
  }

  multisigPolicyInfo() {
    if (this._controller.tag !== CONTROLLER_TAG_MULTISIG) {
      return null;
    }
    const controller = this._controller;
    const ctap2 = encodeMultisigPolicyCbor(controller);
    const digest = computeMultisigPolicyDigest(ctap2);
    const members = controller.members.map((member) => ({
      algorithm: curveIdToAlgorithm(member.curve),
      weight: member.weight,
      publicKeyHex: `0x${bytesToHex(member.publicKey)}`,
    }));
    const totalWeight = members.reduce((acc, member) => acc + member.weight, 0);
    return {
      version: controller.version,
      threshold: controller.threshold,
      totalWeight,
      members,
      ctap2CborHex: `0x${bytesToHex(ctap2)}`,
      digestBlake2b256Hex: `0x${bytesToHex(digest)}`,
    };
  }
}

function assertCanonicalI105Literal(input, address) {
  if (typeof input !== "string") {
    return;
  }
  const discriminant = tryExtractI105Discriminant(input);
  if (discriminant === null) {
    return;
  }
  if (address.toI105(discriminant) !== input) {
    throw new AccountAddressError(
      AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
      "account address literals must use canonical I105 form",
    );
  }
}

function tryExtractI105Discriminant(literal) {
  try {
    const [discriminant] = decodeSupportedI105String(literal);
    return discriminant;
  } catch {
    return undefined;
  }
}

function initNativeAddressCodec(binding) {
  if (!binding) {
    return null;
  }
  if (
    typeof binding.accountAddressParseEncoded !== "function" ||
    typeof binding.accountAddressRender !== "function"
  ) {
    return null;
  }
  return {
    parseEncoded: binding.accountAddressParseEncoded.bind(binding),
    render: binding.accountAddressRender.bind(binding),
  };
}

function resolveNativeBinding() {
  return globalThis.__IROHA_NATIVE_BINDING__ ?? getNativeBinding();
}

function getNativeAddressCodec() {
  if (!nativeAddressCodecResolved) {
    cachedNativeAddressCodec = initNativeAddressCodec(resolveNativeBinding());
    nativeAddressCodecResolved = true;
  }
  return cachedNativeAddressCodec;
}

export function __resetAddressNativeStateForTests() {
  cachedNativeAddressCodec = undefined;
  nativeAddressCodecResolved = false;
}

function parseWithNativeCodec(input, expectedPrefix) {
  const nativeAddressCodec = getNativeAddressCodec();
  if (!nativeAddressCodec) {
    return null;
  }
  try {
    const parsed = nativeAddressCodec.parseEncoded(String(input), expectedPrefix ?? null);
    if (!parsed) {
      return null;
    }
    const canonical = parsed.canonical_bytes;
    if (!canonical || canonical.length === 0) {
      // Some native codec builds may return an empty canonical payload for
      // otherwise parseable literals. Fall back to the pure-JS codec path.
      return null;
    }
    return {
      canonicalBytes: Uint8Array.from(canonical),
      chainDiscriminant: parsed.network_prefix ?? undefined,
    };
  } catch (error) {
    const converted = convertNativeCodecError(error);
    if (
      converted instanceof AccountAddressError &&
      (converted.code === AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT ||
        converted.code === AccountAddressErrorCode.INVALID_I105_CHAR ||
        converted.code === AccountAddressErrorCode.CHECKSUM_MISMATCH ||
        converted.code === AccountAddressErrorCode.UNEXPECTED_NETWORK_PREFIX ||
        converted.code === AccountAddressErrorCode.UNKNOWN_CURVE ||
        converted.code === AccountAddressErrorCode.UNSUPPORTED_ALGORITHM ||
        converted.code === AccountAddressErrorCode.UNKNOWN_CONTROLLER_TAG ||
        converted.code === AccountAddressErrorCode.UNKNOWN_DOMAIN_TAG ||
        converted.code === AccountAddressErrorCode.INVALID_PUBLIC_KEY ||
        converted.code === AccountAddressErrorCode.INVALID_LENGTH)
    ) {
      return null;
    }
    throw converted;
  }
}

function renderWithNativeCodec(canonicalBytes, chainDiscriminant = DEFAULT_I105_DISCRIMINANT) {
  const nativeAddressCodec = getNativeAddressCodec();
  if (!nativeAddressCodec) {
    return null;
  }
  const normalizedDiscriminant = normalizeI105DiscriminantInput(
    chainDiscriminant,
    "renderWithNativeCodec chainDiscriminant",
  );
  try {
    const bytes = Buffer.isBuffer(canonicalBytes)
      ? canonicalBytes
      : Uint8Array.from(canonicalBytes);
    const rendered = nativeAddressCodec.render(bytes, normalizedDiscriminant);
    if (!rendered) {
      return null;
    }
    return {
      canonicalHex: rendered.canonical_hex,
      i105: rendered.i105,
    };
  } catch (error) {
    const converted = convertNativeCodecError(error);
    if (
      converted instanceof AccountAddressError &&
      (converted.code === AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT ||
        converted.code === AccountAddressErrorCode.UNKNOWN_CURVE ||
        converted.code === AccountAddressErrorCode.UNSUPPORTED_ALGORITHM ||
        converted.code === AccountAddressErrorCode.UNKNOWN_CONTROLLER_TAG ||
        converted.code === AccountAddressErrorCode.UNKNOWN_DOMAIN_TAG ||
        converted.code === AccountAddressErrorCode.INVALID_PUBLIC_KEY ||
        converted.code === AccountAddressErrorCode.INVALID_LENGTH)
    ) {
      return null;
    }
    throw converted;
  }
}

function convertNativeCodecError(error) {
  if (!error || typeof error.message !== "string") {
    return new AccountAddressError(
      AccountAddressErrorCode.INVALID_LENGTH,
      "native address codec failure",
      { cause: error },
    );
  }
  const trimmed = error.message.trim();
  const match = /^([A-Z0-9_]+):\s*(.*)$/.exec(trimmed);
  if (match) {
    const [, code, rest] = match;
    if (ACCOUNT_ADDRESS_ERROR_CODES.has(code)) {
      return new AccountAddressError(code, rest || "address codec failure", {
        cause: error,
      });
    }
  }
  return new AccountAddressError(AccountAddressErrorCode.INVALID_LENGTH, trimmed, {
    cause: error,
  });
}

export function encodeI105AccountAddress(canonicalBytes, options = {}) {
  const normalizedOptions = normalizeI105EncodeOptions(
    options,
    "encodeI105AccountAddress",
  );
  return encodeI105String(normalizedOptions.chainDiscriminant, canonicalBytes, normalizedOptions);
}

export function decodeI105AccountAddress(encoded, options = {}) {
  if (typeof encoded !== "string") {
    throw new TypeError("i105 address must be a string");
  }
  const normalizedOptions = normalizeI105DecodeOptions(options);
  const [, canonical] = decodeSupportedI105String(
    encoded,
    normalizedOptions.expectDiscriminant,
  );
  const address = AccountAddress.fromCanonicalBytes(canonical);
  assertCanonicalI105Literal(encoded.trim(), address);
  return canonical;
}

function isCanonicalHexLiteral(literal) {
  if (literal.startsWith("0x") || literal.startsWith("0X")) {
    // Honour the hex prefix regardless of body validity so errors stay in the hex path.
    return true;
  }
  const body = literal;
  return body.length > 0 && body.length % 2 === 0 && HEX_BODY_RE.test(body);
}

function classifyDetectedFormat(literal, inputKind, chainDiscriminant) {
  switch (inputKind) {
    case "i105":
      return {
        kind: "i105",
        chainDiscriminant:
          typeof chainDiscriminant === "number"
            ? chainDiscriminant
            : tryExtractI105Discriminant(literal),
      };
    default:
      return { kind: "unknown" };
  }
}

/**
 * Inspect an account-id literal (canonical I105) and emit canonical encodings.
 *
 * @param {string} literal - Account literal (canonical I105)
 * @param {{ chainDiscriminant?: number, expectDiscriminant?: number }} [options]
 * @returns {{
 *   detectedFormat: { kind: string, chainDiscriminant?: number },
 *   canonicalHex: string,
 *   i105: { value: string, chainDiscriminant: number },
 *   i105Warning: string,
 *   warnings: string[]
 * }}
 */
export function inspectAccountId(literal, options = {}) {
  if (typeof literal !== "string") {
    throw new TypeError("accountId must be a string");
  }
  const trimmed = literal.trim();
  if (trimmed.length === 0) {
    throw new TypeError("accountId must be a non-empty string");
  }

  if (trimmed.includes("@")) {
    throw new TypeError("accountId must not include '@domain'");
  }

  const normalizedOptions = normalizeInspectAccountOptions(options);
  const { address, chainDiscriminant: detectedDiscriminant } = AccountAddress.parseEncoded(
    trimmed,
    normalizedOptions.expectDiscriminant,
  );
  const inputKind = "i105";
  const chainDiscriminant =
    normalizedOptions.chainDiscriminant ?? detectedDiscriminant ?? DEFAULT_I105_DISCRIMINANT;

  return Object.freeze({
    detectedFormat: classifyDetectedFormat(trimmed, inputKind, detectedDiscriminant),
    canonicalHex: address.canonicalHex(),
    i105: { value: address.toI105(chainDiscriminant), chainDiscriminant },
    i105Warning: I105_WARNING,
    warnings: Object.freeze([]),
  });
}

function normalizeI105EncodeOptions(options, context) {
  if (options === undefined) {
    return { chainDiscriminant: DEFAULT_I105_DISCRIMINANT };
  }
  if (!isPlainObject(options)) {
    throw new TypeError(`${context} options must be an object`);
  }
  const allowedKeys = new Set(["chainDiscriminant"]);
  const extras = Object.keys(options).filter((key) => !allowedKeys.has(key));
  if (extras.length > 0) {
    throw new TypeError(
      `${context} options contains unsupported fields: ${extras.join(", ")}`,
    );
  }
  const chainDiscriminant = Object.prototype.hasOwnProperty.call(options, "chainDiscriminant")
    ? normalizeI105DiscriminantInput(
      options.chainDiscriminant,
      `${context} options.chainDiscriminant`,
    )
    : DEFAULT_I105_DISCRIMINANT;
  return { chainDiscriminant };
}

function normalizeI105DecodeOptions(options) {
  if (options === undefined) {
    return { expectDiscriminant: undefined };
  }
  if (!isPlainObject(options)) {
    throw new TypeError("decodeI105AccountAddress options must be an object");
  }
  const allowedKeys = new Set(["expectDiscriminant"]);
  const extras = Object.keys(options).filter((key) => !allowedKeys.has(key));
  if (extras.length > 0) {
    throw new TypeError(
      `decodeI105AccountAddress options contains unsupported fields: ${extras.join(", ")}`,
    );
  }
  let expectDiscriminant;
  if (Object.prototype.hasOwnProperty.call(options, "expectDiscriminant")) {
    expectDiscriminant = normalizeI105DiscriminantInput(
      options.expectDiscriminant,
      "decodeI105AccountAddress options.expectDiscriminant",
    );
  }
  return { expectDiscriminant };
}

function normalizeInspectAccountOptions(options) {
  if (!isPlainObject(options)) {
    throw new TypeError("inspectAccountId options must be an object");
  }
  const allowedKeys = new Set(["chainDiscriminant", "expectDiscriminant"]);
  const extras = Object.keys(options).filter((key) => !allowedKeys.has(key));
  if (extras.length > 0) {
    throw new TypeError(
      `inspectAccountId options contains unsupported fields: ${extras.join(", ")}`,
    );
  }
  let expectDiscriminant;
  if (Object.prototype.hasOwnProperty.call(options, "expectDiscriminant")) {
    expectDiscriminant = normalizeI105DiscriminantInput(
      options.expectDiscriminant,
      "inspectAccountId expectDiscriminant",
    );
  }
  let chainDiscriminant;
  if (Object.prototype.hasOwnProperty.call(options, "chainDiscriminant")) {
    chainDiscriminant = normalizeI105DiscriminantInput(
      options.chainDiscriminant,
      "inspectAccountId chainDiscriminant",
    );
  }
  return { expectDiscriminant, chainDiscriminant };
}

function isPlainObject(value) {
  if (value === null || typeof value !== "object") {
    return false;
  }
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function normalizeI105DiscriminantInput(value, context = "i105 chain discriminant") {
  if (value === undefined || value === null) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_I105_DISCRIMINANT,
      `${context} must be an integer between 0 and ${I105_DISCRIMINANT_MAX}`,
    );
  }
  let numeric;
  if (typeof value === "number") {
    numeric = value;
  } else if (typeof value === "bigint") {
    numeric = Number(value);
  } else if (typeof value === "string") {
    if (value.trim().length === 0) {
      throw new AccountAddressError(
        AccountAddressErrorCode.INVALID_I105_DISCRIMINANT,
        `${context} must be an integer between 0 and ${I105_DISCRIMINANT_MAX}`,
      );
    }
    numeric = Number(value);
  } else {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_I105_DISCRIMINANT,
      `${context} must be an integer between 0 and ${I105_DISCRIMINANT_MAX}`,
    );
  }
  if (!Number.isFinite(numeric) || !Number.isInteger(numeric)) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_I105_DISCRIMINANT,
      `${context} must be an integer between 0 and ${I105_DISCRIMINANT_MAX}`,
    );
  }
  if (numeric < 0 || numeric > I105_DISCRIMINANT_MAX) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_I105_DISCRIMINANT,
      `${context} out of range: ${value}`,
    );
  }
  return numeric;
}

function i105SentinelForDiscriminant(discriminant) {
  switch (discriminant) {
    case DEFAULT_I105_DISCRIMINANT:
      return I105_SENTINEL_SORA;
    case 0x0171:
      return I105_SENTINEL_TEST;
    case 0x0000:
      return I105_SENTINEL_DEV;
    default:
      return `${I105_SENTINEL_NUMERIC_PREFIX}${discriminant}`;
  }
}

function parseI105SentinelAndPayload(encoded) {
  if (typeof encoded !== "string") {
    return null;
  }
  if (
    encoded.startsWith(I105_SENTINEL_SORA) ||
    encoded.startsWith(I105_SENTINEL_SORA_FULLWIDTH)
  ) {
    return [DEFAULT_I105_DISCRIMINANT, encoded.slice(I105_SENTINEL_SORA.length)];
  }
  if (
    encoded.startsWith(I105_SENTINEL_TEST) ||
    encoded.startsWith(I105_SENTINEL_TEST_FULLWIDTH)
  ) {
    return [0x0171, encoded.slice(I105_SENTINEL_TEST.length)];
  }
  if (
    encoded.startsWith(I105_SENTINEL_DEV) ||
    encoded.startsWith(I105_SENTINEL_DEV_FULLWIDTH)
  ) {
    return [0x0000, encoded.slice(I105_SENTINEL_DEV.length)];
  }
  if (
    !encoded.startsWith(I105_SENTINEL_NUMERIC_PREFIX) &&
    !encoded.startsWith(I105_SENTINEL_NUMERIC_PREFIX_FULLWIDTH)
  ) {
    return null;
  }
  const tail = encoded.slice(I105_SENTINEL_NUMERIC_PREFIX.length);
  let index = 0;
  let discriminantDigits = "";
  while (index < tail.length) {
    const asciiDigit = toAsciiDigit(tail[index]);
    if (asciiDigit === null) {
      break;
    }
    discriminantDigits += asciiDigit;
    index += 1;
  }
  if (discriminantDigits.length === 0) {
    return null;
  }
  const discriminant = Number(discriminantDigits);
  if (
    !Number.isInteger(discriminant) ||
    discriminant < 0 ||
    discriminant > I105_DISCRIMINANT_MAX
  ) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_I105_DISCRIMINANT,
      `invalid i105 chain discriminant sentinel: ${encoded}`,
    );
  }
  return [discriminant, tail.slice(index)];
}

function toAsciiDigit(char) {
  if (char >= "0" && char <= "9") {
    return char;
  }
  if (char >= "０" && char <= "９") {
    return String.fromCharCode(char.codePointAt(0) - 0xfee0);
  }
  return null;
}

function encodeI105String(discriminant, canonical) {
  const normalizedDiscriminant = normalizeI105DiscriminantInput(
    discriminant,
    "i105 chain discriminant",
  );
  const canonicalBytes = normalizeBytes(canonical);
  const digits = encodeBaseN(canonicalBytes, I105_BASE);
  const checksum = i105ChecksumDigits(canonicalBytes);
  const sentinel = i105SentinelForDiscriminant(normalizedDiscriminant);
  const parts = [sentinel];
  parts.push(...digits.map((digit) => I105_ALPHABET[digit]));
  parts.push(...checksum.map((digit) => I105_ALPHABET[digit]));
  return parts.join("");
}

function decodeSupportedI105String(encoded, expectedDiscriminant) {
  return decodeI105String(encoded, expectedDiscriminant);
}

function lookupI105Digit(symbol) {
  const canonicalIndex = I105_ALPHABET.indexOf(symbol);
  if (canonicalIndex !== -1) {
    return canonicalIndex;
  }
  const halfwidthIndex = IROHA_POEM_KANA_HALFWIDTH.indexOf(symbol);
  if (halfwidthIndex !== -1) {
    return BASE58_ALPHABET.length + halfwidthIndex;
  }
  return undefined;
}

function decodeI105Payload(payload) {
  const digits = [];
  for (const symbol of Array.from(payload)) {
    const digit = lookupI105Digit(symbol);
    if (digit === undefined) {
      throw new AccountAddressError(
        AccountAddressErrorCode.INVALID_I105_CHAR,
        `invalid character in i105 address: ${symbol}`,
        { details: { char: symbol } },
      );
    }
    digits.push(digit);
  }

  if (digits.length <= I105_CHECKSUM_LEN) {
    throw new AccountAddressError(
      AccountAddressErrorCode.I105_TOO_SHORT,
      "i105 address too short",
    );
  }

  const dataDigits = digits.slice(0, -I105_CHECKSUM_LEN);
  const checksumDigits = digits.slice(-I105_CHECKSUM_LEN);
  const canonicalBytes = decodeBaseN(dataDigits, I105_BASE);
  const expected = i105ChecksumDigits(canonicalBytes);
  if (!Buffer.from(expected).equals(Buffer.from(checksumDigits))) {
    throw new AccountAddressError(
      AccountAddressErrorCode.CHECKSUM_MISMATCH,
      "i105 checksum mismatch",
    );
  }
  return canonicalBytes;
}

function decodeI105String(encoded, expectedDiscriminant) {
  if (typeof encoded !== "string") {
    throw new TypeError("i105 address must be a string");
  }
  const parsed = parseI105SentinelAndPayload(encoded);
  if (!parsed) {
    throw new AccountAddressError(
      AccountAddressErrorCode.MISSING_I105_SENTINEL,
      "i105 address is missing the expected chain-discriminant sentinel",
    );
  }
  const [discriminant, payload] = parsed;
  if (expectedDiscriminant !== undefined) {
    const normalizedExpected = normalizeI105DiscriminantInput(
      expectedDiscriminant,
      "expected i105 chain discriminant",
    );
    if (discriminant !== normalizedExpected) {
      throw new AccountAddressError(
        AccountAddressErrorCode.UNEXPECTED_NETWORK_PREFIX,
        `unexpected i105 chain discriminant: expected ${normalizedExpected}, found ${discriminant}`,
        { details: { expected: normalizedExpected, found: discriminant } },
      );
    }
  }
  const canonicalBytes = decodeI105Payload(payload);
  return [discriminant, canonicalBytes];
}

function encodeBaseN(bytes, base) {
  if (base < 2) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_I105_BASE,
      "invalid base for encoding",
    );
  }
  const value = Array.from(bytes);
  let leading = 0;
  while (leading < value.length && value[leading] === 0) {
    leading += 1;
  }
  const digits = [];
  let start = leading;
  while (start < value.length) {
    let remainder = 0;
    for (let i = start; i < value.length; i += 1) {
      const acc = (remainder << 8) | value[i];
      value[i] = Math.floor(acc / base);
      remainder = acc % base;
    }
    digits.push(remainder);
    while (start < value.length && value[start] === 0) {
      start += 1;
    }
  }
  for (let i = 0; i < leading; i += 1) {
    digits.push(0);
  }
  if (digits.length === 0) {
    digits.push(0);
  }
  digits.reverse();
  return digits;
}

function decodeBaseN(digits, base) {
  if (base < 2) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_I105_BASE,
      "invalid base for decoding",
    );
  }
  if (digits.length === 0) {
    throw new AccountAddressError(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for address payload");
  }
  const value = Array.from(digits);
  for (const digit of value) {
    if (digit < 0 || digit >= base) {
      throw new AccountAddressError(
        AccountAddressErrorCode.INVALID_I105_DIGIT,
        `invalid digit ${digit} for base ${base}`,
        { details: { digit, base } },
      );
    }
  }
  let leading = 0;
  while (leading < value.length && value[leading] === 0) {
    leading += 1;
  }
  const out = [];
  let start = leading;
  while (start < value.length) {
    let remainder = 0;
    for (let i = start; i < value.length; i += 1) {
      const acc = remainder * base + value[i];
      value[i] = Math.floor(acc / 256);
      remainder = acc % 256;
    }
    out.push(remainder);
    while (start < value.length && value[start] === 0) {
      start += 1;
    }
  }
  for (let i = 0; i < leading; i += 1) {
    out.push(0);
  }
  out.reverse();
  return Uint8Array.from(out);
}

function convertToBase32(data) {
  const bytes = Array.from(data);
  let acc = 0;
  let bits = 0;
  const out = [];
  for (const byte of bytes) {
    acc = (acc << 8) | byte;
    bits += 8;
    while (bits >= 5) {
      bits -= 5;
      out.push((acc >> bits) & 0x1f);
    }
  }
  if (bits > 0) {
    out.push((acc << (5 - bits)) & 0x1f);
  }
  return out;
}

function bech32Polymod(values) {
  const generators = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3];
  let chk = 1;
  for (const value of values) {
    const top = chk >> 25;
    chk = ((chk & 0x1ff_ffff) << 5) ^ value;
    generators.forEach((generator, idx) => {
      if ((top >> idx) & 1) {
        chk ^= generator;
      }
    });
  }
  return chk;
}

function expandHrp(hrp) {
  const out = [];
  for (const ch of hrp) {
    const code = ch.codePointAt(0);
    out.push(code >> 5);
  }
  out.push(0);
  for (const ch of hrp) {
    out.push(ch.codePointAt(0) & 0x1f);
  }
  return out;
}

function bech32mChecksum(data) {
  const values = expandHrp("snx");
  values.push(...data);
  values.push(...Array(I105_CHECKSUM_LEN).fill(0));
  const polymod = bech32Polymod(values) ^ BECH32M_CONST;
  const result = [];
  for (let i = 0; i < I105_CHECKSUM_LEN; i += 1) {
    result.push((polymod >> (5 * (I105_CHECKSUM_LEN - 1 - i))) & 0x1f);
  }
  return result;
}

function i105ChecksumDigits(canonical) {
  const base32 = convertToBase32(canonical);
  return bech32mChecksum(base32);
}

export {
  curveIdFromAlgorithm,
  curveIdToAlgorithm,
  ensureCurveIdEnabled,
  normalizeBytes,
  validatePublicKeyForCurve,
};
