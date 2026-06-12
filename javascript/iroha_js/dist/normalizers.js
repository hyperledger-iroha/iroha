import { Buffer } from "node:buffer";
import { blake3 } from "@noble/hashes/blake3";
import {
  AccountAddress,
  AccountAddressError,
  AccountAddressErrorCode,
} from "./address.js";
import {
  createValidationError,
  ValidationError,
  ValidationErrorCode,
} from "./validationError.js";

export { ValidationError, ValidationErrorCode };

function fail(code, message, path) {
  throw createValidationError(code, message, path);
}

const BASE58_PATTERN = /^[1-9A-HJ-NP-Za-km-z]+$/;
const BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const BASE58_INDEX = new Map(Array.from(BASE58_ALPHABET, (symbol, index) => [symbol, index]));
const ASSET_DEFINITION_ADDRESS_VERSION = 1;
const ASSET_DEFINITION_ADDRESS_LEN = 21;
const ALIAS_LOCAL_PATTERN = /^[a-z0-9]+(?:[._-][a-z0-9]+)*$/;
const ALIAS_SCOPE_SEGMENT_PATTERN = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

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

function looksLikeCanonicalI105Literal(raw) {
  if (typeof raw !== "string") {
    return false;
  }
  if (/\s/.test(raw) || raw.includes("@") || raw.includes("#") || raw.includes("$")) {
    return false;
  }
  if (raw.length < 32 || raw.length > 160) {
    return false;
  }
  const sentinelMatch = /^(?:sora|test|dev)/u.exec(raw);
  if (!sentinelMatch) {
    return false;
  }
  const payload = raw.slice(sentinelMatch[0].length);
  if (!payload) {
    return false;
  }
  return /^[1-9A-HJ-NP-Za-km-z\p{Script=Katakana}ｰﾞﾟ]+$/u.test(payload);
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
      `${name} must be a canonical I105 account id`,
      name,
    );
  }

  try {
    const { address, chainDiscriminant } = AccountAddress.parseEncoded(raw);
    if (typeof chainDiscriminant === "number") {
      return address.toI105(chainDiscriminant);
    }
    return looksLikeCanonicalI105Literal(raw) ? raw : address.toI105();
  } catch (error) {
    if (error instanceof AccountAddressError) {
      if (looksLikeCanonicalI105Literal(raw)) {
        return raw;
      }
      fail(
        ValidationErrorCode.INVALID_ACCOUNT_ID,
        `${name} must be a canonical I105 account id`,
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
      if (looksLikeCanonicalI105Literal(raw)) {
        return raw;
      }
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

export function normalizeAccountAliasLiteral(value, name) {
  const alias = assertString(value, name).trim();
  if (alias.length === 0) {
    fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty string`, name);
  }
  const aliasParts = alias.split("@");
  const scopeParts = aliasParts[1]?.split(".") ?? [];
  if (
    aliasParts.length !== 2 ||
    !aliasParts[0] ||
    !aliasParts[1] ||
    scopeParts.length < 1 ||
    scopeParts.length > 2 ||
    scopeParts.some((part) => !part) ||
    /\s/.test(alias)
  ) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must use name@dataspace or name@domain.dataspace form`,
      name,
    );
  }
  return alias;
}

export function normalizeAccountIdOrAliasLiteral(value, name) {
  const raw = assertString(value, name).trim();
  if (raw.includes("@")) {
    return normalizeAccountAliasLiteral(raw, name);
  }
  return normalizeAccountId(raw, name);
}

export function normalizeAssetId(value, name) {
  const raw = assertString(value, name).trim();
  if (raw.length === 0) {
    fail(ValidationErrorCode.INVALID_ASSET_ID, `${name} must be a non-empty string`, name);
  }
  if (raw.includes("#")) {
    fail(
      ValidationErrorCode.INVALID_ASSET_ID,
      `${name} must be a canonical Base58 asset id; asset aliases and asset holding ids are not accepted here`,
      name,
    );
  }
  if (
    /\s/.test(raw) ||
    raw.includes("%") ||
    raw.includes("/") ||
    raw.includes("?") ||
    raw.includes(":") ||
    !/^[1-9A-HJ-NP-Za-km-z]+$/.test(raw)
  ) {
    fail(
      ValidationErrorCode.INVALID_ASSET_ID,
      `${name} must be a canonical unprefixed Base58 asset id`,
      name,
    );
  }
  return raw;
}

function decodeBase58Literal(value, name) {
  if (!value.length) {
    fail(
      ValidationErrorCode.INVALID_ASSET_DEFINITION_ID,
      `${name} must be a canonical Base58 asset definition id`,
      name,
    );
  }
  const digits = [];
  for (const symbol of value) {
    const digit = BASE58_INDEX.get(symbol);
    if (digit === undefined) {
      fail(
        ValidationErrorCode.INVALID_ASSET_DEFINITION_ID,
        `${name} must be a canonical Base58 asset definition id`,
        name,
      );
    }
    digits.push(digit);
  }
  const scratch = Array.from(digits);
  let leadingZeroCount = 0;
  while (leadingZeroCount < scratch.length && scratch[leadingZeroCount] === 0) {
    leadingZeroCount += 1;
  }
  const decoded = [];
  let start = leadingZeroCount;
  while (start < scratch.length) {
    let remainder = 0;
    for (let index = start; index < scratch.length; index += 1) {
      const accumulator = remainder * 58 + scratch[index];
      scratch[index] = Math.floor(accumulator / 256);
      remainder = accumulator % 256;
    }
    decoded.push(remainder);
    while (start < scratch.length && scratch[start] === 0) {
      start += 1;
    }
  }
  for (let index = 0; index < leadingZeroCount; index += 1) {
    decoded.push(0);
  }
  decoded.reverse();
  return Uint8Array.from(decoded);
}

function bytesEqual(left, right) {
  if (left.length !== right.length) return false;
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) return false;
  }
  return true;
}

function isUuidV4Bytes(bytes) {
  return bytes.length === 16 && (bytes[6] >> 4) === 0b0100 && (bytes[8] & 0b1100_0000) === 0b1000_0000;
}

export function normalizeAssetDefinitionId(value, name = "asset_definition_id") {
  const raw = assertString(value, name).trim();
  if (raw.length === 0) {
    fail(
      ValidationErrorCode.INVALID_ASSET_DEFINITION_ID,
      `${name} must be a canonical Base58 asset definition id`,
      name,
    );
  }
  if (raw.includes(":") || raw.includes("#") || !BASE58_PATTERN.test(raw)) {
    fail(
      ValidationErrorCode.INVALID_ASSET_DEFINITION_ID,
      `${name} must be a canonical Base58 asset definition id`,
      name,
    );
  }
  const payload = decodeBase58Literal(raw, name);
  if (payload.length !== ASSET_DEFINITION_ADDRESS_LEN) {
    fail(
      ValidationErrorCode.INVALID_ASSET_DEFINITION_ID,
      `${name} must contain exactly 21 decoded bytes`,
      name,
    );
  }
  if (payload[0] !== ASSET_DEFINITION_ADDRESS_VERSION) {
    fail(
      ValidationErrorCode.INVALID_ASSET_DEFINITION_ID,
      `${name} version is not supported`,
      name,
    );
  }
  const expectedChecksum = blake3(payload.subarray(0, 17)).subarray(0, 4);
  if (!bytesEqual(payload.subarray(17), expectedChecksum)) {
    fail(
      ValidationErrorCode.INVALID_ASSET_DEFINITION_ID,
      `${name} checksum is invalid`,
      name,
    );
  }
  if (!isUuidV4Bytes(payload.subarray(1, 17))) {
    fail(
      ValidationErrorCode.INVALID_ASSET_DEFINITION_ID,
      `${name} is not a canonical UUIDv4-backed asset definition id`,
      name,
    );
  }
  return raw;
}

export function tryNormalizeAssetDefinitionId(value, name = "asset_definition_id") {
  try {
    return normalizeAssetDefinitionId(String(value ?? ""), name);
  } catch {
    return null;
  }
}

export function normalizeAssetHoldingId(value, name) {
  const raw = assertString(value, name).trim();
  if (raw.length === 0) {
    fail(ValidationErrorCode.INVALID_ASSET_ID, `${name} must be a non-empty string`, name);
  }
  const parts = raw.split("#");
  if (parts.length < 2 || parts.length > 3) {
    fail(
      ValidationErrorCode.INVALID_ASSET_ID,
      `${name} must use '<base58-asset-definition-id>#<i105-account-id>' with optional '#dataspace:<id>' suffix`,
      name,
    );
  }
  const [assetId, accountId, scope] = parts;
  const normalizedAssetId = normalizeAssetDefinitionId(assetId, `${name}.assetId`);
  const normalizedAccountId = normalizeAccountId(accountId, `${name}.accountId`);
  if (scope === undefined) {
    return `${normalizedAssetId}#${normalizedAccountId}`;
  }
  const scopeMatch = /^dataspace:(\d+)$/.exec(scope);
  if (!scopeMatch) {
    fail(
      ValidationErrorCode.INVALID_ASSET_ID,
      `${name}.scope must use 'dataspace:<id>' when present`,
      name,
    );
  }
  return `${normalizedAssetId}#${normalizedAccountId}#dataspace:${scopeMatch[1]}`;
}

export function composeAssetHoldingId(assetId, accountId, dataspaceId, name = "asset_holding_id") {
  const normalizedAssetId = normalizeAssetDefinitionId(assetId, `${name}.assetId`);
  const normalizedAccountId = normalizeAccountId(accountId, `${name}.accountId`);
  const scope = String(dataspaceId ?? "").trim();
  if (!scope) return `${normalizedAssetId}#${normalizedAccountId}`;
  if (!/^\d+$/.test(scope)) {
    fail(
      ValidationErrorCode.INVALID_ASSET_ID,
      `${name}.scope must use decimal digits when present`,
      `${name}.scope`,
    );
  }
  return `${normalizedAssetId}#${normalizedAccountId}#dataspace:${scope}`;
}

export function extractAssetDefinitionId(value, name = "asset_id") {
  const raw = assertString(value, name).trim();
  if (raw.includes("#")) {
    return normalizeAssetHoldingId(raw, name).split("#", 1)[0];
  }
  return normalizeAssetDefinitionId(raw, name);
}

export function tryExtractAssetDefinitionId(value, name = "asset_id") {
  try {
    return extractAssetDefinitionId(String(value ?? ""), name);
  } catch {
    return null;
  }
}

export function assetReferencesMatch(left, right) {
  const leftDefinition = tryExtractAssetDefinitionId(left);
  const rightDefinition = tryExtractAssetDefinitionId(right);
  return Boolean(leftDefinition && rightDefinition && leftDefinition === rightDefinition);
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

export function normalizeI105AccountId(value, name = "account_id") {
  return normalizeAccountId(value, name);
}

export function tryNormalizeI105AccountId(value, name = "account_id") {
  try {
    return normalizeI105AccountId(String(value ?? ""), name);
  } catch {
    return null;
  }
}

export function normalizeToriiAccountReference(value, name = "account_id") {
  const literal = String(value ?? "").trim();
  if (!literal || literal.includes("@") || /\s/.test(literal)) {
    return "";
  }
  return tryNormalizeI105AccountId(literal, name) ?? "";
}

function normalizeAliasScope(scope, name, separator) {
  const parts = scope.split(".");
  if (parts.length === 0 || parts.length > 2) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must use <name>${separator}<dataspace> or <name>${separator}<domain>.<dataspace> form`,
      name,
    );
  }
  for (const part of parts) {
    if (!ALIAS_SCOPE_SEGMENT_PATTERN.test(part)) {
      fail(
        ValidationErrorCode.INVALID_STRING,
        `${name} must use <name>${separator}<dataspace> or <name>${separator}<domain>.<dataspace> form`,
        name,
      );
    }
  }
  return parts.join(".");
}

function normalizeScopedAlias(value, name, separator, options = {}) {
  const raw = assertString(value, name).trim();
  if (raw.length === 0 || /\s/.test(raw)) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must use <name>${separator}<dataspace> or <name>${separator}<domain>.<dataspace> form`,
      name,
    );
  }
  const firstIndex = raw.indexOf(separator);
  const lastIndex = raw.lastIndexOf(separator);
  if (firstIndex <= 0 || firstIndex !== lastIndex || lastIndex >= raw.length - 1) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must use <name>${separator}<dataspace> or <name>${separator}<domain>.<dataspace> form`,
      name,
    );
  }
  const local = raw.slice(0, firstIndex).trim().toLowerCase();
  const scope = raw.slice(lastIndex + 1).trim().toLowerCase();
  if (!ALIAS_LOCAL_PATTERN.test(local)) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must use <name>${separator}<dataspace> or <name>${separator}<domain>.<dataspace> form`,
      name,
    );
  }
  if (options.rejectI105Local && tryNormalizeI105AccountId(local, `${name}.local`)) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must use <name>${separator}<dataspace> or <name>${separator}<domain>.<dataspace> form`,
      name,
    );
  }
  const normalizedScope = normalizeAliasScope(scope, name, separator);
  return `${local}${separator}${normalizedScope}`;
}

export function normalizeAccountAliasFqn(value, name = "alias_fqn") {
  return normalizeScopedAlias(value, name, "@", { rejectI105Local: true });
}

export function tryNormalizeAccountAliasFqn(value, name = "alias_fqn") {
  try {
    return normalizeAccountAliasFqn(String(value ?? ""), name);
  } catch {
    return null;
  }
}

export function normalizeAssetAliasFqn(value, name = "asset_alias_fqn") {
  return normalizeScopedAlias(value, name, "#");
}

export function tryNormalizeAssetAliasFqn(value, name = "asset_alias_fqn") {
  try {
    return normalizeAssetAliasFqn(String(value ?? ""), name);
  } catch {
    return null;
  }
}
