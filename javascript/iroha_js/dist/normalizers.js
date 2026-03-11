import { Buffer } from "node:buffer";
import {
  AccountAddress,
  AccountAddressError,
  AccountAddressErrorCode,
  DEFAULT_DOMAIN_NAME,
} from "./address.js";
import {
  createValidationError,
  ValidationErrorCode,
} from "./validationError.js";

function fail(code, message, path) {
  throw createValidationError(code, message, path);
}

function assertString(value, name) {
  if (typeof value !== "string" || value.length === 0) {
    fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty string`, name);
  }
  return value;
}

function normalizeUaidLiteral(value, name) {
  const literal = assertString(value, name).trim();
  if (!literal) {
    fail(ValidationErrorCode.INVALID_ACCOUNT_ID, `${name} must be a non-empty string`, name);
  }
  let hexPortion;
  if (literal.slice(0, 5).toLowerCase() === "uaid:") {
    hexPortion = literal.slice(5).trim();
  } else {
    hexPortion = literal;
  }
  if (hexPortion.length !== 64 || !/^[0-9a-fA-F]+$/.test(hexPortion)) {
    fail(
      ValidationErrorCode.INVALID_ACCOUNT_ID,
      `${name} must contain 64 hex characters`,
      name,
    );
  }
  if (!/[13579bdf]$/i.test(hexPortion)) {
    fail(
      ValidationErrorCode.INVALID_ACCOUNT_ID,
      `${name} must have least significant bit set to 1`,
      name,
    );
  }
  return `uaid:${hexPortion.toLowerCase()}`;
}

function normalizeOpaqueLiteral(value, name) {
  const literal = assertString(value, name).trim();
  if (!literal) {
    fail(ValidationErrorCode.INVALID_ACCOUNT_ID, `${name} must be a non-empty string`, name);
  }
  let hexPortion;
  if (literal.slice(0, 7).toLowerCase() === "opaque:") {
    hexPortion = literal.slice(7).trim();
  } else {
    hexPortion = literal;
  }
  if (hexPortion.length !== 64 || !/^[0-9a-fA-F]+$/.test(hexPortion)) {
    fail(
      ValidationErrorCode.INVALID_ACCOUNT_ID,
      `${name} must contain 64 hex characters`,
      name,
    );
  }
  return `opaque:${hexPortion.toLowerCase()}`;
}

export function canonicalizeMultihashHex(value, name) {
  const trimmed = value.trim();
  if (trimmed.length === 0 || trimmed.length % 2 !== 0) {
    fail(
      ValidationErrorCode.INVALID_HEX,
      `${name} must be an even-length hexadecimal string`,
      name,
    );
  }
  if (!/^[0-9A-Fa-f]+$/.test(trimmed)) {
    fail(
      ValidationErrorCode.INVALID_HEX,
      `${name} must be an even-length hexadecimal string`,
      name,
    );
  }
  let bytes;
  try {
    bytes = Buffer.from(trimmed, "hex");
  } catch {
    fail(
      ValidationErrorCode.INVALID_HEX,
      `${name} must be an even-length hexadecimal string`,
      name,
    );
  }
  if (bytes.length === 0) {
    fail(ValidationErrorCode.INVALID_MULTIHASH, `${name} must contain multihash bytes`, name);
  }

  let fnEnd = 0;
  while (fnEnd < bytes.length && (bytes[fnEnd] & 0x80) !== 0) {
    fnEnd += 1;
  }
  if (fnEnd >= bytes.length) {
    fail(
      ValidationErrorCode.INVALID_MULTIHASH,
      `${name} is missing multihash function bytes`,
      name,
    );
  }

  let lenEnd = fnEnd + 1;
  while (lenEnd < bytes.length && (bytes[lenEnd] & 0x80) !== 0) {
    lenEnd += 1;
  }
  if (lenEnd >= bytes.length) {
    fail(
      ValidationErrorCode.INVALID_MULTIHASH,
      `${name} is missing multihash length bytes`,
      name,
    );
  }

  const payload = bytes.subarray(lenEnd + 1);
  if (payload.length === 0) {
    fail(
      ValidationErrorCode.INVALID_MULTIHASH,
      `${name} must include a multihash payload`,
      name,
    );
  }
  const declaredLength = decodeVarint(bytes, fnEnd + 1, lenEnd, name);
  if (declaredLength !== payload.length) {
    fail(
      ValidationErrorCode.INVALID_MULTIHASH,
      `${name} declares ${declaredLength} bytes but contains ${payload.length} bytes`,
      name,
    );
  }

  const fnHex = bytes.subarray(0, fnEnd + 1).toString("hex").toUpperCase();
  const lenHex = bytes.subarray(fnEnd + 1, lenEnd + 1).toString("hex").toUpperCase();
  const payloadHex = payload.toString("hex").toUpperCase();

  return `${fnHex}${lenHex}${payloadHex}`;
}

function decodeVarint(bytes, start, end, name) {
  let value = 0n;
  let shift = 0n;
  for (let index = start; index <= end; index += 1) {
    const byte = BigInt(bytes[index]);
    value |= (byte & 0x7fn) << shift;
    if ((byte & 0x80n) === 0n) {
      break;
    }
    shift += 7n;
    if (shift > 63n) {
      fail(
        ValidationErrorCode.INVALID_MULTIHASH,
        `${name} contains an invalid multihash length`,
        name,
      );
    }
  }
  if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
    fail(
      ValidationErrorCode.INVALID_MULTIHASH,
      `${name} contains an oversized multihash length`,
      name,
    );
  }
  return Number(value);
}

export function normalizeAccountId(value, name) {
  const raw = assertString(value, name).trim();
  if (raw.length === 0) {
    fail(ValidationErrorCode.INVALID_ACCOUNT_ID, `${name} must be a non-empty string`, name);
  }

  if (raw.includes("@")) {
    fail(
      ValidationErrorCode.INVALID_ACCOUNT_ID,
      `${name} must not include '@domain'; use an encoded I105 account id`,
      name,
    );
  }
  if (
    raw.slice(0, 5).toLowerCase() === "uaid:" ||
    raw.slice(0, 7).toLowerCase() === "opaque:" ||
    /^[0-9a-fA-F]{64}$/.test(raw)
  ) {
    fail(
      ValidationErrorCode.INVALID_ACCOUNT_ID,
      `${name} must be an I105 account id`,
      name,
    );
  }

  try {
    const { address } = AccountAddress.parseEncoded(raw);
    return address.toI105();
  } catch (error) {
    if (error instanceof AccountAddressError) {
      fail(
        ValidationErrorCode.INVALID_ACCOUNT_ID,
        `${name} must be an I105 account id`,
        name,
      );
    }
    throw error;
  }
}

export function ensureCanonicalAccountId(value, name) {
  const raw = assertString(value, name).trim();
  if (raw.length === 0) {
    fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty string`, name);
  }
  if (raw.includes("@")) {
    fail(
      ValidationErrorCode.INVALID_ACCOUNT_ID,
      `${name} must be a canonical I105 account id`,
      name,
    );
  }
  if (
    raw.slice(0, 5).toLowerCase() === "uaid:" ||
    raw.slice(0, 7).toLowerCase() === "opaque:" ||
    /^[0-9a-fA-F]{64}$/.test(raw)
  ) {
    fail(
      ValidationErrorCode.INVALID_ACCOUNT_ID,
      `${name} must be a canonical I105 account id`,
      name,
    );
  }
  let parsed;
  try {
    parsed = AccountAddress.parseEncoded(raw);
  } catch (error) {
    if (error instanceof AccountAddressError) {
      throw createValidationError(
        ValidationErrorCode.INVALID_ACCOUNT_ID,
        `${name} must be a canonical I105 account id`,
        name,
        error,
      );
    }
    throw error;
  }
  const canonical = parsed.address.toI105();
  if (raw !== canonical) {
    fail(
      ValidationErrorCode.INVALID_ACCOUNT_ID,
      `${name} must use canonical I105 account id form`,
      name,
    );
  }
  return canonical;
}

export function normalizeAssetId(value, name) {
  const raw = assertString(value, name).trim();
  if (raw.length === 0) {
    fail(ValidationErrorCode.INVALID_ASSET_ID, `${name} must be a non-empty string`, name);
  }
  if (raw.includes("#")) {
    fail(
      ValidationErrorCode.INVALID_ASSET_ID,
      `${name} must use encoded AssetId form 'norito:<hex>'; legacy 'asset#domain#account' and 'asset##account' forms are not supported`,
      name,
    );
  }
  const prefix = "norito:";
  if (!raw.startsWith(prefix)) {
    fail(
      ValidationErrorCode.INVALID_ASSET_ID,
      `${name} must use encoded AssetId form 'norito:<hex>'`,
      name,
    );
  }
  const body = raw.slice(prefix.length).trim();
  if (body.length === 0 || body.length % 2 !== 0 || !/^[0-9a-fA-F]+$/.test(body)) {
    fail(
      ValidationErrorCode.INVALID_ASSET_ID,
      `${name} must use encoded AssetId form 'norito:<hex>'`,
      name,
    );
  }
  return `${prefix}${body.toLowerCase()}`;
}
