"use strict";

import { Buffer } from "node:buffer";
import { createHash } from "node:crypto";
import { domainToASCII } from "node:url";
import { blake2b256, blake2b512 } from "./blake2b.js";
import { getNativeBinding } from "./native.js";
export const DEFAULT_DOMAIN_NAME = "default";
const DEFAULT_IH58_PREFIX = 753;
const IH58_PREFIX_MAX = 0x3fff;
const IH58_CHECKSUM_PREFIX = Buffer.from("IH58PRE", "ascii");
const HEADER_VERSION_V1 = 0;
const HEADER_NORM_VERSION_V1 = 1;
const COMPRESSED_SENTINEL = "sora";
const COMPRESSED_CHECKSUM_LEN = 6;
const BECH32M_CONST = 0x2bc830a3;
const IH58_CHECKSUM_BYTES = 2;
const COMPRESSED_WARNING =
  "Compressed Sora addresses rely on half-width kana and are only interoperable inside Sora-aware apps. Prefer IH58 when sharing with explorers, wallets, or QR codes.";
const IH58_ALPHABET = [
  "1",
  "2",
  "3",
  "4",
  "5",
  "6",
  "7",
  "8",
  "9",
  "A",
  "B",
  "C",
  "D",
  "E",
  "F",
  "G",
  "H",
  "J",
  "K",
  "L",
  "M",
  "N",
  "P",
  "Q",
  "R",
  "S",
  "T",
  "U",
  "V",
  "W",
  "X",
  "Y",
  "Z",
  "a",
  "b",
  "c",
  "d",
  "e",
  "f",
  "g",
  "h",
  "i",
  "j",
  "k",
  "m",
  "n",
  "o",
  "p",
  "q",
  "r",
  "s",
  "t",
  "u",
  "v",
  "w",
  "x",
  "y",
  "z",
];

const SORA_KANA = [
  "ｲ",
  "ﾛ",
  "ﾊ",
  "ﾆ",
  "ﾎ",
  "ﾍ",
  "ﾄ",
  "ﾁ",
  "ﾘ",
  "ﾇ",
  "ﾙ",
  "ｦ",
  "ﾜ",
  "ｶ",
  "ﾖ",
  "ﾀ",
  "ﾚ",
  "ｿ",
  "ﾂ",
  "ﾈ",
  "ﾅ",
  "ﾗ",
  "ﾑ",
  "ｳ",
  "ヰ",
  "ﾉ",
  "ｵ",
  "ｸ",
  "ﾔ",
  "ﾏ",
  "ｹ",
  "ﾌ",
  "ｺ",
  "ｴ",
  "ﾃ",
  "ｱ",
  "ｻ",
  "ｷ",
  "ﾕ",
  "ﾒ",
  "ﾐ",
  "ｼ",
  "ヱ",
  "ﾋ",
  "ﾓ",
  "ｾ",
  "ｽ",
];

const MULTISIG_DIGEST_PERSONALIZATION = (() => {
  const bytes = new Uint8Array(16);
  bytes.set(Buffer.from("iroha-ms-policy", "ascii"));
  return bytes;
})();

const nativeBinding = getNativeBinding();
const nativeAddressCodec = initNativeAddressCodec(nativeBinding);

const SORA_KANA_FULLWIDTH = [
  "イ",
  "ロ",
  "ハ",
  "ニ",
  "ホ",
  "ヘ",
  "ト",
  "チ",
  "リ",
  "ヌ",
  "ル",
  "ヲ",
  "ワ",
  "カ",
  "ヨ",
  "タ",
  "レ",
  "ソ",
  "ツ",
  "ネ",
  "ナ",
  "ラ",
  "ム",
  "ウ",
  "ヰ",
  "ノ",
  "オ",
  "ク",
  "ヤ",
  "マ",
  "ケ",
  "フ",
  "コ",
  "エ",
  "テ",
  "ア",
  "サ",
  "キ",
  "ユ",
  "メ",
  "ミ",
  "シ",
  "ヱ",
  "ヒ",
  "モ",
  "セ",
  "ス",
];

function domainSelectorDetails(selector) {
  if (!selector) {
    return { tag: null, digestHex: null, registryId: null, label: null };
  }
  return {
    tag: selector.tag,
    digestHex:
      selector.payload && selector.payload.length > 0
        ? Buffer.from(selector.payload).toString("hex")
        : null,
    registryId: decodeRegistryId(selector),
    label: selector.tag === 0 ? DEFAULT_DOMAIN_NAME : null,
  };
}

function assertDomainMatches(address, expectedDomainName) {
  if (expectedDomainName === undefined || expectedDomainName === null) {
    return;
  }
  if (typeof expectedDomainName !== "string") {
    throw new TypeError("expected domain name must be a string");
  }
  const normalizedDomain = expectedDomainName.trim();
  if (normalizedDomain.length === 0) {
    throw new AccountAddressError(
      AccountAddressErrorCode.DOMAIN_MISMATCH,
      "account address domain selector does not match an empty domain",
      { details: { expectedDomain: expectedDomainName } },
    );
  }
  // Account IDs are globally scoped in v1; expectedDomain is validated but not bound to payload.
  canonicalizeDomainLabel(normalizedDomain);
}

const COMPRESSED_ALPHABET = IH58_ALPHABET.concat(SORA_KANA);
const COMPRESSED_ALPHABET_FULLWIDTH = IH58_ALPHABET.concat(SORA_KANA_FULLWIDTH);
const COMPRESSED_INDEX = new Map(COMPRESSED_ALPHABET.map((symbol, idx) => [symbol, idx]));
for (const [idx, symbol] of COMPRESSED_ALPHABET_FULLWIDTH.entries()) {
  COMPRESSED_INDEX.set(symbol, idx);
}
const IH58_INDEX = new Map(IH58_ALPHABET.map((symbol, idx) => [symbol, idx]));
const COMPRESSED_BASE = COMPRESSED_ALPHABET.length;

export const AccountAddressErrorCode = Object.freeze({
  UNSUPPORTED_ALGORITHM: "ERR_UNSUPPORTED_ALGORITHM",
  KEY_PAYLOAD_TOO_LONG: "ERR_KEY_PAYLOAD_TOO_LONG",
  INVALID_HEADER_VERSION: "ERR_INVALID_HEADER_VERSION",
  INVALID_NORM_VERSION: "ERR_INVALID_NORM_VERSION",
  INVALID_IH58_PREFIX: "ERR_INVALID_IH58_PREFIX",
  CANONICAL_HASH_FAILURE: "ERR_CANONICAL_HASH_FAILURE",
  INVALID_IH58_ENCODING: "ERR_INVALID_IH58_ENCODING",
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
  INVALID_IH58_PREFIX_ENCODING: "ERR_INVALID_IH58_PREFIX_ENCODING",
  MISSING_COMPRESSED_SENTINEL: "ERR_MISSING_COMPRESSED_SENTINEL",
  COMPRESSED_TOO_SHORT: "ERR_COMPRESSED_TOO_SHORT",
  INVALID_COMPRESSED_CHAR: "ERR_INVALID_COMPRESSED_CHAR",
  INVALID_COMPRESSED_BASE: "ERR_INVALID_COMPRESSED_BASE",
  INVALID_COMPRESSED_DIGIT: "ERR_INVALID_COMPRESSED_DIGIT",
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

export const AccountAddressFormat = Object.freeze({
  IH58: "ih58",
  COMPRESSED: "compressed",
});

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

function normalizeRegistryId(registryId) {
  let value = registryId;
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (trimmed.length === 0) {
      throw new AccountAddressError(
        AccountAddressErrorCode.INVALID_REGISTRY_ID,
        "registry id must be a numeric value",
      );
    }
    value = Number(trimmed);
  }
  if (typeof value === "bigint") {
    value = Number(value);
  }
  if (
    typeof value !== "number" ||
    !Number.isFinite(value) ||
    Math.floor(value) !== value ||
    value < 0 ||
    value > 0xffffffff
  ) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_REGISTRY_ID,
      "registry id must be a 32-bit unsigned integer",
      { details: { registryId } },
    );
  }
  return value >>> 0;
}

function encodeDomainFromRegistryId(registryId) {
  normalizeRegistryId(registryId);
  // Canonical payloads are globally scoped and no longer encode registry selectors.
  return { tag: 0, payload: null };
}

function decodeRegistryId(selector) {
  if (!selector || selector.tag !== 2 || !selector.payload || selector.payload.length !== 4) {
    return null;
  }
  // Stored as big-endian u32
  return (
    ((selector.payload[0] << 24) |
      (selector.payload[1] << 16) |
      (selector.payload[2] << 8) |
      selector.payload[3]) >>>
    0
  );
}

function encodeDomainFromName(domain) {
  canonicalizeDomainLabel(domain);
  // Canonical payloads are globally scoped and no longer encode local selectors.
  return { tag: 0, payload: null };
}

function encodeDomainFromAccountInputs(domain, registryId) {
  const hasDomain = domain !== undefined && domain !== null;
  const hasRegistryId = registryId !== undefined && registryId !== null;
  if (hasDomain && hasRegistryId) {
    throw new TypeError("fromAccount accepts either domain or registryId, not both");
  }
  if (!hasDomain && !hasRegistryId) {
    throw new TypeError("fromAccount requires a domain or registryId");
  }
  if (hasRegistryId) {
    return encodeDomainFromRegistryId(registryId);
  }
  return encodeDomainFromName(domain);
}

export class AccountAddress {
  constructor(header, domain, controller) {
    this._header = header;
    this._domain = domain;
    this._controller = controller;
  }

  static fromAccount({ domain, registryId, publicKey, algorithm = "ed25519" }) {
    const header = {
      version: HEADER_VERSION_V1,
      classId: AddressClass.SINGLE_KEY,
      normVersion: HEADER_NORM_VERSION_V1,
      extFlag: false,
    };
    const selector = encodeDomainFromAccountInputs(domain, registryId);
    const curve = curveIdFromAlgorithm(algorithm);
    const keyBytes = normalizeBytes(publicKey);
    validatePublicKeyForCurve(curve, keyBytes, "public key");
    const controller = { tag: CONTROLLER_TAG_SINGLE, curve, publicKey: keyBytes };
    return new AccountAddress(header, selector, controller);
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
    return new AccountAddress(header, { tag: 0, payload: null }, controller);
  }

  static fromIH58(encoded, expectedPrefix) {
    const normalizedExpectedPrefix =
      expectedPrefix === undefined
        ? undefined
        : normalizeIh58PrefixInput(
            expectedPrefix,
            "AccountAddress.fromIH58 expectedPrefix",
          );
    const nativeParsed = parseWithNativeCodec(
      encoded,
      normalizedExpectedPrefix,
      AccountAddressFormat.IH58,
    );
    if (nativeParsed) {
      return AccountAddress.fromCanonicalBytes(nativeParsed.canonicalBytes);
    }
    const [prefix, canonical] = decodeIh58String(encoded);
    if (normalizedExpectedPrefix !== undefined && prefix !== normalizedExpectedPrefix) {
      throw new AccountAddressError(
        AccountAddressErrorCode.UNEXPECTED_NETWORK_PREFIX,
        `unexpected IH58 network prefix: expected ${expectedPrefix}, found ${prefix}`,
        { details: { expected: expectedPrefix, found: prefix } },
      );
    }
    return AccountAddress.fromCanonicalBytes(canonical);
  }

  static fromCompressedSora(encoded) {
    const literal = String(encoded);
    if (!literal.startsWith(COMPRESSED_SENTINEL)) {
      throw new AccountAddressError(
        AccountAddressErrorCode.MISSING_COMPRESSED_SENTINEL,
        "compressed address must start with sora sentinel",
      );
    }
    const nativeParsed = parseWithNativeCodec(literal, undefined, AccountAddressFormat.COMPRESSED);
    if (nativeParsed) {
      return AccountAddress.fromCanonicalBytes(nativeParsed.canonicalBytes);
    }
    const canonical = decodeCompressedString(literal);
    return AccountAddress.fromCanonicalBytes(canonical);
  }

  static parseEncoded(input, expectedPrefix, expectedDomainName) {
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
        "account address literals must not include '@domain'; use IH58 or compressed sora",
      );
    }
    if (trimmed.startsWith(COMPRESSED_SENTINEL)) {
      const address = AccountAddress.fromCompressedSora(trimmed);
      assertDomainMatches(address, expectedDomainName);
      return { address, format: AccountAddressFormat.COMPRESSED };
    }
    if (isCanonicalHexLiteral(trimmed)) {
      throw new AccountAddressError(
        AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
        "canonical hex account addresses are not accepted; use IH58 or compressed sora",
      );
    }
    try {
      const address = AccountAddress.fromIH58(trimmed, expectedPrefix);
      assertDomainMatches(address, expectedDomainName);
      return {
        address,
        format: AccountAddressFormat.IH58,
        networkPrefix: tryExtractIh58Prefix(trimmed),
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
    const rendered = renderWithNativeCodec(canonical, DEFAULT_IH58_PREFIX);
    if (rendered?.canonicalHex) {
      return rendered.canonicalHex;
    }
    return `0x${Buffer.from(canonical).toString("hex")}`;
  }

  toIH58(prefix = DEFAULT_IH58_PREFIX) {
    const normalizedPrefix = normalizeIh58PrefixInput(
      prefix,
      "AccountAddress.toIH58 networkPrefix",
    );
    const canonical = this.canonicalBytes();
    const rendered = renderWithNativeCodec(canonical, normalizedPrefix);
    if (rendered?.ih58) {
      return rendered.ih58;
    }
    return encodeIh58String(normalizedPrefix, canonical);
  }

  toCompressedSora() {
    const canonical = this.canonicalBytes();
    const rendered = renderWithNativeCodec(canonical, DEFAULT_IH58_PREFIX);
    if (rendered?.compressed) {
      return rendered.compressed;
    }
    return encodeCompressedString(canonical);
  }

  toCompressedSoraFullWidth() {
    const canonical = this.canonicalBytes();
    const rendered = renderWithNativeCodec(canonical, DEFAULT_IH58_PREFIX);
    if (rendered?.compressedFullWidth) {
      return rendered.compressedFullWidth;
    }
    return encodeCompressedString(canonical, { fullWidth: true });
  }

  toString() {
    return this.canonicalHex();
  }

  /**
   * Convenience helper that returns both IH58 (preferred) and compressed (`sora`, second-best) variants alongside
   * the network prefix. Follow the UX checklist in
   * `docs/source/sns/address_display_guidelines.md` when presenting these values.
   *
   * @param {number|bigint|string} networkPrefix - IH58 prefix (defaults to the Sora Nexus prefix `753`);
   * accepts numeric strings that will be normalized.
   * @returns {{ ih58: string, compressed: string, networkPrefix: number, compressedWarning: string, domainSummary: { kind: string, warning: string | null, selector: { tag: number | null, digestHex: string | null, registryId: number | null, label: string | null } } }}
   */
  displayFormats(networkPrefix = DEFAULT_IH58_PREFIX) {
    const normalizedPrefix = normalizeIh58PrefixInput(
      networkPrefix,
      "AccountAddress.displayFormats networkPrefix",
    );
    const ih58 = this.toIH58(normalizedPrefix);
    const compressed = this.toCompressedSora();
    const domainSummary = this.domainSummary();
    const selector = Object.freeze({
      tag: domainSummary.selector?.tag ?? null,
      digestHex: domainSummary.selector?.digestHex ?? null,
      registryId: domainSummary.selector?.registryId ?? null,
      label: domainSummary.selector?.label ?? null,
    });
    return Object.freeze({
      ih58,
      compressed,
      networkPrefix: normalizedPrefix,
      compressedWarning: COMPRESSED_WARNING,
      domainSummary: Object.freeze({
        kind: domainSummary.kind,
        warning: domainSummary.warning,
        selector,
      }),
    });
  }

  domainSelector() {
    if (!this._domain) {
      return { tag: null, payload: null };
    }
    return Object.freeze({
      tag: this._domain.tag,
      payload: this._domain.payload ? Uint8Array.from(this._domain.payload) : null,
    });
  }

  domainSummary() {
    return summarizeDomainSelector(this._domain);
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

function summarizeDomainSelector(selector) {
  if (!selector || typeof selector.tag !== "number") {
    return {
      kind: "unknown",
      warning: null,
      selector: Object.freeze({ tag: null, digestHex: null, registryId: null, label: null }),
    };
  }
  const details = domainSelectorDetails(selector);
  switch (selector.tag) {
    case 0:
      return {
        kind: "default",
        warning: null,
        selector: Object.freeze(details),
      };
    default:
      return {
        kind: "unknown",
        warning: null,
        selector: Object.freeze({
          ...details,
          tag: selector.tag,
        }),
      };
  }
}

function tryExtractIh58Prefix(literal) {
  try {
    const [prefix] = decodeIh58String(literal);
    return prefix;
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

function parseWithNativeCodec(input, expectedPrefix, requiredFormat) {
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
    const format = normalizeNativeAddressFormat(
      parsed.detected_format,
    );
    if (requiredFormat && (!format || format !== requiredFormat)) {
      throw new AccountAddressError(
        AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
        "unsupported address format",
        { details: { expectedFormat: requiredFormat, actualFormat: format ?? "unknown" } },
      );
    }
    return {
      canonicalBytes: Uint8Array.from(canonical),
      format,
      networkPrefix: parsed.network_prefix ?? undefined,
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

function renderWithNativeCodec(canonicalBytes, networkPrefix = DEFAULT_IH58_PREFIX) {
  if (!nativeAddressCodec) {
    return null;
  }
  const normalizedPrefix = normalizeIh58PrefixInput(
    networkPrefix,
    "renderWithNativeCodec networkPrefix",
  );
  try {
    const bytes = Buffer.isBuffer(canonicalBytes)
      ? canonicalBytes
      : Uint8Array.from(canonicalBytes);
    const rendered = nativeAddressCodec.render(bytes, normalizedPrefix);
    if (!rendered) {
      return null;
    }
    return {
      canonicalHex: rendered.canonical_hex,
      ih58: rendered.ih58,
      compressed: rendered.compressed,
      compressedFullWidth: rendered.compressed_fullwidth,
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

function normalizeNativeAddressFormat(kind) {
  if (!kind) {
    return null;
  }
  const value = String(kind).toLowerCase();
  switch (value) {
    case AccountAddressFormat.IH58:
      return AccountAddressFormat.IH58;
    case AccountAddressFormat.COMPRESSED:
      return AccountAddressFormat.COMPRESSED;
    default:
      return null;
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

export function encodeCompressedAccountAddress(canonicalBytes, options = {}) {
  const normalizedOptions = normalizeCompressedEncodeOptions(
    options,
    "encodeCompressedAccountAddress",
  );
  return encodeCompressedString(canonicalBytes, normalizedOptions);
}

export function decodeCompressedAccountAddress(compressed) {
  if (typeof compressed !== "string") {
    throw new TypeError("compressed address must be a string");
  }
  return decodeCompressedString(compressed);
}

function isCanonicalHexLiteral(literal) {
  if (literal.startsWith("0x") || literal.startsWith("0X")) {
    // Honour the sentinel regardless of body validity so errors stay in the hex path.
    return true;
  }
  const body = literal;
  return body.length > 0 && body.length % 2 === 0 && HEX_BODY_RE.test(body);
}

function classifyDetectedFormat(literal, format, networkPrefix) {
  switch (format) {
    case AccountAddressFormat.COMPRESSED:
      return { kind: "compressed" };
    case AccountAddressFormat.IH58:
      return {
        kind: "ih58",
        networkPrefix:
          typeof networkPrefix === "number" ? networkPrefix : tryExtractIh58Prefix(literal),
      };
    default:
      return { kind: "unknown" };
  }
}

/**
 * Inspect an account-id literal (IH58 (preferred)/sora (second-best)) and emit canonical
 * encodings plus compatibility warnings (for example compressed literals).
 *
 * @param {string} literal - Account literal (IH58 (preferred)/sora (second-best))
 * @param {{ networkPrefix?: number, expectPrefix?: number }} [options]
 * @returns {{
 *   detectedFormat: { kind: string, networkPrefix?: number },
 *   domain: { kind: string, warning: string | null },
 *   canonicalHex: string,
 *   ih58: { value: string, networkPrefix: number },
 *   compressed: string,
 *   compressedWarning: string,
 *   inputDomain: string | null,
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
  const { address, format, networkPrefix: detectedPrefix } = AccountAddress.parseEncoded(
    trimmed,
    normalizedOptions.expectPrefix,
  );
  const networkPrefix =
    normalizedOptions.networkPrefix ?? detectedPrefix ?? DEFAULT_IH58_PREFIX;

  const domainSummary = address.domainSummary();
  const warnings = domainSummary.warning ? [domainSummary.warning] : [];
  if (format === AccountAddressFormat.COMPRESSED) {
    warnings.push(COMPRESSED_WARNING);
  }
  return Object.freeze({
    detectedFormat: classifyDetectedFormat(trimmed, format, detectedPrefix),
    domain: domainSummary,
    canonicalHex: address.canonicalHex(),
    ih58: { value: address.toIH58(networkPrefix), networkPrefix },
    compressed: address.toCompressedSora(),
    compressedWarning: COMPRESSED_WARNING,
    inputDomain: null,
    warnings,
  });
}

function normalizeCompressedEncodeOptions(options, context) {
  if (options === undefined) {
    return { fullWidth: false };
  }
  if (!isPlainObject(options)) {
    throw new TypeError(`${context} options must be an object`);
  }
  const allowedKeys = new Set(["fullWidth"]);
  const extras = Object.keys(options).filter((key) => !allowedKeys.has(key));
  if (extras.length > 0) {
    throw new TypeError(
      `${context} options contains unsupported fields: ${extras.join(", ")}`,
    );
  }
  if (
    Object.prototype.hasOwnProperty.call(options, "fullWidth") &&
    typeof options.fullWidth !== "boolean"
  ) {
    throw new TypeError(`${context} options.fullWidth must be a boolean`);
  }
  return { fullWidth: options.fullWidth === true };
}

function normalizeInspectAccountOptions(options) {
  if (!isPlainObject(options)) {
    throw new TypeError("inspectAccountId options must be an object");
  }
  const allowedKeys = new Set(["networkPrefix", "expectPrefix"]);
  const extras = Object.keys(options).filter((key) => !allowedKeys.has(key));
  if (extras.length > 0) {
    throw new TypeError(
      `inspectAccountId options contains unsupported fields: ${extras.join(", ")}`,
    );
  }
  let expectPrefix;
  if (Object.prototype.hasOwnProperty.call(options, "expectPrefix")) {
    expectPrefix = normalizeIh58PrefixInput(
      options.expectPrefix,
      "inspectAccountId expectPrefix",
    );
  }
  let networkPrefix;
  if (Object.prototype.hasOwnProperty.call(options, "networkPrefix")) {
    networkPrefix = normalizeIh58PrefixInput(
      options.networkPrefix,
      "inspectAccountId networkPrefix",
    );
  }
  return { expectPrefix, networkPrefix };
}

function isPlainObject(value) {
  if (value === null || typeof value !== "object") {
    return false;
  }
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function normalizeIh58PrefixInput(prefix, context = "IH58 prefix") {
  if (prefix === undefined || prefix === null) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_IH58_PREFIX,
      `${context} must be an integer between 0 and ${IH58_PREFIX_MAX}`,
    );
  }
  let numeric;
  if (typeof prefix === "number") {
    numeric = prefix;
  } else if (typeof prefix === "bigint") {
    numeric = Number(prefix);
  } else if (typeof prefix === "string") {
    if (prefix.trim().length === 0) {
      throw new AccountAddressError(
        AccountAddressErrorCode.INVALID_IH58_PREFIX,
        `${context} must be an integer between 0 and ${IH58_PREFIX_MAX}`,
      );
    }
    numeric = Number(prefix);
  } else {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_IH58_PREFIX,
      `${context} must be an integer between 0 and ${IH58_PREFIX_MAX}`,
    );
  }
  if (!Number.isFinite(numeric) || !Number.isInteger(numeric)) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_IH58_PREFIX,
      `${context} must be an integer between 0 and ${IH58_PREFIX_MAX}`,
    );
  }
  if (numeric < 0 || numeric > IH58_PREFIX_MAX) {
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_IH58_PREFIX,
      `invalid IH58 prefix: ${prefix}`,
    );
  }
  return numeric;
}

function encodeIh58Prefix(prefix) {
  const numeric = normalizeIh58PrefixInput(prefix, "IH58 prefix");
  if (numeric <= 63) {
    return Uint8Array.of(numeric);
  }
  const lower = (numeric & 0b0011_1111) | 0b0100_0000;
  const upper = numeric >> 6;
  return Uint8Array.of(lower, upper);
}

function decodeIh58Prefix(bytes) {
  if (bytes.length === 0) {
    throw new AccountAddressError(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for IH58 payload");
  }
  const first = bytes[0];
  if (first <= 63) {
    return [first, 1];
  }
  if (first & 0b0100_0000) {
    if (bytes.length < 2) {
      throw new AccountAddressError(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for IH58 payload");
    }
    const second = bytes[1];
    const value = ((second & 0xff) << 6) | (first & 0b0011_1111);
    return [value, 2];
  }
  throw new AccountAddressError(
    AccountAddressErrorCode.INVALID_IH58_PREFIX_ENCODING,
    `invalid IH58 prefix encoding: ${first}`,
  );
}

function blake2b512Digest(payload) {
  try {
    return createHash("blake2b512").update(payload).digest();
  } catch (error) {
    if (
      error instanceof Error &&
      /digest method not supported/i.test(
        `${error.message}\n${error.stack ?? ""}`,
      )
    ) {
      return Buffer.from(blake2b512(payload));
    }
    throw error;
  }
}

function encodeIh58String(prefix, canonical) {
  const canonicalBytes = normalizeBytes(canonical);
  const prefixBytes = encodeIh58Prefix(prefix);
  const body = new Uint8Array(prefixBytes.length + canonicalBytes.length);
  body.set(prefixBytes, 0);
  body.set(canonicalBytes, prefixBytes.length);
  const checksumInput = Buffer.concat([IH58_CHECKSUM_PREFIX, Buffer.from(body)]);
  const checksum = blake2b512Digest(checksumInput);
  const full = new Uint8Array(body.length + IH58_CHECKSUM_BYTES);
  full.set(body, 0);
  full.set(checksum.subarray(0, IH58_CHECKSUM_BYTES), body.length);
  const digits = encodeBaseN(full, 58);
  return digits.map((digit) => IH58_ALPHABET[digit]).join("");
}

function decodeIh58String(encoded) {
  const digits = [];
  for (const ch of encoded) {
    const value = IH58_INDEX.get(ch);
    if (value === undefined) {
      throw new AccountAddressError(AccountAddressErrorCode.INVALID_IH58_ENCODING, "invalid IH58 base58 encoding");
    }
    digits.push(value);
  }
  const body = decodeBaseN(digits, 58);
  if (body.length < 1 + IH58_CHECKSUM_BYTES) {
    throw new AccountAddressError(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for IH58 payload");
  }
  const checksumBytes = body.slice(body.length - IH58_CHECKSUM_BYTES);
  const payload = body.slice(0, body.length - IH58_CHECKSUM_BYTES);
  const [prefix, prefixLen] = decodeIh58Prefix(payload);
  const checksumInput = Buffer.concat([IH58_CHECKSUM_PREFIX, Buffer.from(payload)]);
  const expected = blake2b512Digest(checksumInput).subarray(
    0,
    IH58_CHECKSUM_BYTES,
  );
  if (!Buffer.from(checksumBytes).equals(Buffer.from(expected))) {
    throw new AccountAddressError(AccountAddressErrorCode.CHECKSUM_MISMATCH, "IH58 checksum mismatch");
  }
  const canonical = payload.slice(prefixLen);
  return [prefix, canonical];
}

function encodeCompressedString(canonical, options = {}) {
  const canonicalBytes = normalizeBytes(canonical);
  const digits = encodeBaseN(canonicalBytes, COMPRESSED_BASE);
  const checksum = compressedChecksumDigits(canonicalBytes);
  const parts = [COMPRESSED_SENTINEL];
  const alphabet = options.fullWidth === true ? COMPRESSED_ALPHABET_FULLWIDTH : COMPRESSED_ALPHABET;
  parts.push(...digits.map((digit) => alphabet[digit]));
  parts.push(...checksum.map((digit) => alphabet[digit]));
  return parts.join("");
}

function decodeCompressedString(encoded) {
  if (!encoded.startsWith(COMPRESSED_SENTINEL)) {
    throw new AccountAddressError(AccountAddressErrorCode.MISSING_COMPRESSED_SENTINEL, "compressed address must start with sora sentinel");
  }
  const payload = encoded.slice(COMPRESSED_SENTINEL.length);
  if (payload.length <= COMPRESSED_CHECKSUM_LEN) {
    throw new AccountAddressError(AccountAddressErrorCode.COMPRESSED_TOO_SHORT, "compressed address is too short");
  }
  const digits = [];
  for (const ch of payload) {
    const value = COMPRESSED_INDEX.get(ch);
    if (value === undefined) {
      throw new AccountAddressError(
        AccountAddressErrorCode.INVALID_COMPRESSED_CHAR,
        `invalid compressed alphabet symbol: ${ch}`,
        { details: { char: ch } },
      );
    }
    digits.push(value);
  }
  const dataDigits = digits.slice(0, -COMPRESSED_CHECKSUM_LEN);
  const checksumDigits = digits.slice(-COMPRESSED_CHECKSUM_LEN);
  const canonical = decodeBaseN(dataDigits, COMPRESSED_BASE);
  const expected = compressedChecksumDigits(canonical);
  if (!Buffer.from(expected).equals(Buffer.from(checksumDigits))) {
    throw new AccountAddressError(AccountAddressErrorCode.CHECKSUM_MISMATCH, "compressed checksum mismatch");
  }
  return canonical;
}

function encodeBaseN(bytes, base) {
  if (base < 2) {
    throw new AccountAddressError(AccountAddressErrorCode.INVALID_COMPRESSED_BASE, "invalid base for encoding");
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
    throw new AccountAddressError(AccountAddressErrorCode.INVALID_COMPRESSED_BASE, "invalid base for decoding");
  }
  if (digits.length === 0) {
    throw new AccountAddressError(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for address payload");
  }
  const value = Array.from(digits);
  for (const digit of value) {
    if (digit < 0 || digit >= base) {
      throw new AccountAddressError(
        AccountAddressErrorCode.INVALID_COMPRESSED_DIGIT,
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
  values.push(...Array(COMPRESSED_CHECKSUM_LEN).fill(0));
  const polymod = bech32Polymod(values) ^ BECH32M_CONST;
  const result = [];
  for (let i = 0; i < COMPRESSED_CHECKSUM_LEN; i += 1) {
    result.push((polymod >> (5 * (COMPRESSED_CHECKSUM_LEN - 1 - i))) & 0x1f);
  }
  return result;
}

function compressedChecksumDigits(canonical) {
  const base32 = convertToBase32(canonical);
  return bech32mChecksum(base32);
}
