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

export function normalizeUaidLiteral(value, name) {
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

export function normalizeOpaqueLiteral(value, name) {
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

export function normalizeIdentifierInput(value, normalization, name = "identifier") {
  const raw = assertString(value, name);
  const trimmed = raw.trim();
  if (!trimmed) {
    fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty string`, name);
  }
  const mode = assertString(normalization, `${name}Normalization`).trim().toLowerCase();
  switch (mode) {
    case "exact":
      return trimmed;
    case "lowercase_trimmed":
      return trimmed.toLowerCase();
    case "phone_e164":
      return normalizePhoneIdentifier(trimmed, name);
    case "email_address":
      return normalizeEmailIdentifier(trimmed, name);
    case "account_number":
      return normalizeAccountNumberIdentifier(trimmed, name);
    default:
      fail(
        ValidationErrorCode.INVALID_STRING,
        `${name}Normalization must be one of exact, lowercase_trimmed, phone_e164, email_address, or account_number`,
        `${name}Normalization`,
      );
  }
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

function normalizePhoneIdentifier(raw, name) {
  const compact = Array.from(raw)
    .filter((ch) => ![" ", "\t", "\n", "\r", "-", "(", ")", "."].includes(ch))
    .join("");
  const withoutPrefix = compact.startsWith("+")
    ? compact.slice(1)
    : compact.startsWith("00")
      ? compact.slice(2)
      : compact;
  if (!withoutPrefix || !/^[0-9]+$/.test(withoutPrefix)) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must contain digits with optional leading '+' or '00'`,
      name,
    );
  }
  return `+${withoutPrefix}`;
}

function normalizeEmailIdentifier(raw, name) {
  const lowered = raw.trim().toLowerCase();
  const parts = lowered.split("@");
  if (parts.length !== 2 || !parts[0] || !parts[1]) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must contain exactly one '@' with non-empty local and domain parts`,
      name,
    );
  }
  return lowered;
}

function normalizeAccountNumberIdentifier(raw, name) {
  const normalized = Array.from(raw)
    .filter((ch) => ![" ", "\t", "\n", "\r", "-"].includes(ch))
    .map((ch) => ch.toUpperCase())
    .join("");
  if (!normalized || !/^[A-Z0-9_/.]+$/.test(normalized)) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must contain ASCII alphanumeric characters, '_', '/', or '.'`,
      name,
    );
  }
  return normalized;
}

export function normalizeAccountId(value, name) {
  const raw = assertString(value, name).trim();
  if (raw.length === 0) {
    fail(ValidationErrorCode.INVALID_ACCOUNT_ID, `${name} must be a non-empty string`, name);
  }

  if (raw.includes("@")) {
    fail(
      ValidationErrorCode.INVALID_ACCOUNT_ID,
      `${name} must not include '@domain'; use an encoded i105 account id`,
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
      `${name} must be a canonical Katakana i105 account id`,
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
      `${name} must be a canonical Katakana i105 account id`,
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
        `${name} must be a canonical Katakana i105 account id`,
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
      `${name} must use canonical Katakana i105 account id form`,
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
  const parts = raw.split("#");
  if (parts.length < 2 || parts.length > 3) {
    fail(
      ValidationErrorCode.INVALID_ASSET_ID,
      `${name} must use '<base58-asset-id>#<katakana-i105-account-id>' with optional '#dataspace:<id>' suffix`,
      name,
    );
  }
  const [assetDefinitionId, accountId, scope] = parts;
  if (
    assetDefinitionId.length === 0 ||
    /\s/.test(assetDefinitionId) ||
    assetDefinitionId.includes("%") ||
    assetDefinitionId.includes("/") ||
    assetDefinitionId.includes("?") ||
    assetDefinitionId.includes(":")
  ) {
    fail(
      ValidationErrorCode.INVALID_ASSET_ID,
      `${name}.assetDefinitionId must be a canonical unprefixed Base58 asset definition id`,
      name,
    );
  }
  const normalizedAccountId = normalizeAccountId(accountId, `${name}.accountId`);
  if (scope === undefined) {
    return `${assetDefinitionId}#${normalizedAccountId}`;
  }
  const scopeMatch = /^dataspace:(\d+)$/.exec(scope);
  if (!scopeMatch) {
    fail(
      ValidationErrorCode.INVALID_ASSET_ID,
      `${name}.scope must use 'dataspace:<id>' when present`,
      name,
    );
  }
  return `${assetDefinitionId}#${normalizedAccountId}#dataspace:${scopeMatch[1]}`;
}

export function normalizeRwaId(value, name) {
  const raw = assertString(value, name).trim();
  if (raw.length === 0) {
    fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty string`, name);
  }
  if (/\s/.test(raw)) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must use '<64-hex-hash>$<domain>' with no whitespace`,
      name,
    );
  }
  const parts = raw.split("$");
  if (parts.length !== 2 || !parts[0] || !parts[1]) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must use '<64-hex-hash>$<domain>'`,
      name,
    );
  }
  const [hash, domain] = parts;
  if (!/^[0-9a-fA-F]{64}$/.test(hash)) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name}.hash must contain exactly 64 hexadecimal characters`,
      name,
    );
  }
  if (/[#$@]/.test(domain) || domain.length === 0) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name}.domain must be a non-empty domain id`,
      name,
    );
  }
  return `${hash.toLowerCase()}$${domain}`;
}
