import { Buffer } from "node:buffer";
import {
  AccountAddress,
  AccountAddressError,
  AccountAddressErrorCode,
  AccountAddressFormat,
  DEFAULT_DOMAIN_NAME,
  canonicalizeDomainLabel,
} from "./address.js";
import { getNativeBinding } from "./native.js";
import {
  createValidationError,
  ValidationError,
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

const MULTIHASH_PUBLIC_KEY_ALGORITHMS = new Map([
  [0xed, "ed25519"],
  [0xe7, "secp256k1"],
]);

function decodeVarintWithIndex(bytes, start, name) {
  let value = 0n;
  let shift = 0n;
  let index = start;
  for (; index < bytes.length; index += 1) {
    const byte = BigInt(bytes[index]);
    value |= (byte & 0x7fn) << shift;
    if ((byte & 0x80n) === 0n) {
      if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
        fail(
          ValidationErrorCode.INVALID_MULTIHASH,
          `${name} contains an oversized multihash value`,
          name,
        );
      }
      return { value: Number(value), nextIndex: index + 1 };
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
  fail(
    ValidationErrorCode.INVALID_MULTIHASH,
    `${name} contains an incomplete multihash prefix`,
    name,
  );
  return { value: 0, nextIndex: bytes.length };
}

function decodeMultihashPublicKey(hexValue, name) {
  const bytes = Buffer.from(hexValue, "hex");
  const { value: fnCode, nextIndex: lenIndex } = decodeVarintWithIndex(bytes, 0, name);
  const { value: payloadLength, nextIndex: payloadIndex } = decodeVarintWithIndex(
    bytes,
    lenIndex,
    name,
  );
  const payload = bytes.subarray(payloadIndex);
  if (payload.length !== payloadLength) {
    fail(
      ValidationErrorCode.INVALID_MULTIHASH,
      `${name} declares ${payloadLength} bytes but contains ${payload.length} bytes`,
      name,
    );
  }
  const algorithm = MULTIHASH_PUBLIC_KEY_ALGORITHMS.get(fnCode);
  if (!algorithm) {
    fail(
      ValidationErrorCode.INVALID_MULTIHASH,
      `${name} uses an unsupported multihash algorithm`,
      name,
    );
  }
  return { algorithm, payload };
}

function tryAccountAddressFromSignatory(signatory, domain, name) {
  const trimmed = signatory.trim();
  if (trimmed.length === 0) {
    fail(ValidationErrorCode.INVALID_ACCOUNT_ID, `${name} must be a non-empty string`, name);
  }
  const colonIndex = trimmed.indexOf(":");
  let algorithmPrefix = null;
  let hexBody = trimmed;
  if (colonIndex !== -1) {
    algorithmPrefix = trimmed.slice(0, colonIndex).toLowerCase();
    hexBody = trimmed.slice(colonIndex + 1);
  }
  if (hexBody.startsWith("0x") || hexBody.startsWith("0X")) {
    hexBody = hexBody.slice(2);
  }
  if (!/^[0-9a-fA-F]+$/.test(hexBody) || hexBody.length % 2 !== 0) {
    return null;
  }
  const canonicalHex = canonicalizeMultihashHex(hexBody, name);
  const decoded = decodeMultihashPublicKey(canonicalHex, name);
  if (algorithmPrefix && algorithmPrefix !== decoded.algorithm) {
    fail(
      ValidationErrorCode.INVALID_MULTIHASH,
      `${name} algorithm prefix does not match multihash`,
      name,
    );
  }
  return AccountAddress.fromAccount({
    domain,
    publicKey: decoded.payload,
    algorithm: decoded.algorithm,
  });
}

function tryParseCanonicalSignatory(signatory) {
  try {
    const { address } = AccountAddress.parseAny(signatory);
    return address.canonicalHex();
  } catch (error) {
    if (isAliasFormatError(error)) {
      return null;
    }
    throw error;
  }
}

let cachedNativeBinding;

function canonicalizeUsingNative(signatory, domain) {
  if (cachedNativeBinding === undefined) {
    cachedNativeBinding = getNativeBinding();
  }
  const binding = cachedNativeBinding;
  if (
    !binding ||
    typeof binding.noritoEncodeInstruction !== "function" ||
    typeof binding.noritoDecodeInstruction !== "function"
  ) {
    return null;
  }

  try {
    const payload = JSON.stringify({
      Register: {
        Account: {
          id: `${signatory}@${domain}`,
          metadata: {},
        },
      },
    });
    const encoded = binding.noritoEncodeInstruction(payload);
    const decodedJson = binding.noritoDecodeInstruction(encoded);
    const decoded = JSON.parse(decodedJson);
    const accountId = decoded?.Register?.Account?.id;
    if (typeof accountId === "string") {
      const at = accountId.lastIndexOf("@");
      if (at !== -1) {
        return accountId.slice(0, at);
      }
    }
  } catch {
    return null;
  }
  return null;
}

const ALIAS_FORMAT_ERROR_CODES = new Set([
  AccountAddressErrorCode.INVALID_IH58_PREFIX,
  AccountAddressErrorCode.INVALID_IH58_PREFIX_ENCODING,
  AccountAddressErrorCode.INVALID_IH58_ENCODING,
  AccountAddressErrorCode.CHECKSUM_MISMATCH,
  AccountAddressErrorCode.MISSING_COMPRESSED_SENTINEL,
  AccountAddressErrorCode.COMPRESSED_TOO_SHORT,
  AccountAddressErrorCode.INVALID_COMPRESSED_CHAR,
  AccountAddressErrorCode.INVALID_COMPRESSED_BASE,
  AccountAddressErrorCode.INVALID_COMPRESSED_DIGIT,
  AccountAddressErrorCode.LOCAL_DIGEST_TOO_SHORT,
  AccountAddressErrorCode.UNKNOWN_CONTROLLER_TAG,
  AccountAddressErrorCode.UNEXPECTED_EXTENSION_FLAG,
]);

function isAliasFormatError(error) {
  return (
    error instanceof AccountAddressError &&
    (error.code === AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT ||
      ALIAS_FORMAT_ERROR_CODES.has(error.code))
  );
}

function canonicalizeSignatory(signatory, domain, name) {
  const trimmed = signatory.trim();
  if (trimmed.length === 0) {
    fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty string`, name);
  }

  const nativeCanonical = canonicalizeUsingNative(trimmed, domain);
  if (nativeCanonical) {
    return nativeCanonical;
  }

  if (trimmed.length > 2 && (trimmed.startsWith("0x") || trimmed.startsWith("0X"))) {
    try {
      const { address } = AccountAddress.parseAny(trimmed, undefined, domain);
      const controller = address && address._controller;
      if (controller && controller.publicKey) {
        const keyBytes = Buffer.from(controller.publicKey);
        const curveId = controller.curve;
        if (curveId === 1) {
          const prefix = Buffer.from([0xed, 0x01, keyBytes.length]);
          const candidate = Buffer.concat([prefix, keyBytes]).toString("hex");
          return canonicalizeMultihashHex(candidate, name);
        }
      }
    } catch (error) {
      if (error instanceof AccountAddressError) {
        if (isAliasFormatError(error)) {
          throw error;
        }
        if (error.code !== AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT) {
          throw createValidationError(
            ValidationErrorCode.INVALID_ACCOUNT_ID,
            error.message,
            name,
            error,
          );
        }
      } else {
        throw error;
      }
      // fall back to manual canonicalisation when the format is not recognised.
    }
    const hexBody = trimmed.slice(2);
    return canonicalizeMultihashHex(hexBody, name);
  }

  const direct = tryParseCanonicalSignatory(trimmed);
  if (direct) {
    return direct;
  }

  const colonIndex = trimmed.indexOf(":");
  if (colonIndex !== -1) {
    const algorithm = trimmed.slice(0, colonIndex);
    const body = trimmed.slice(colonIndex + 1);
    const canonicalBody = canonicalizeSignatory(body, domain, `${name}.body`);
    return `${algorithm}:${canonicalBody}`;
  }

  if (/^[0-9A-Fa-f]+$/.test(trimmed) && trimmed.length % 2 === 0) {
    return canonicalizeMultihashHex(trimmed, name);
  }

  return trimmed;
}

function normalizeNameLiteral(value, name) {
  const trimmed = assertString(value, name).trim();
  if (trimmed.length === 0) {
    fail(ValidationErrorCode.INVALID_ACCOUNT_ID, `${name} must be a non-empty string`, name);
  }
  if (/\s/.test(trimmed)) {
    fail(ValidationErrorCode.INVALID_ACCOUNT_ID, `${name} must not contain whitespace`, name);
  }
  if (/[@#$]/.test(trimmed)) {
    fail(
      ValidationErrorCode.INVALID_ACCOUNT_ID,
      `${name} must not include '@', '#', or '$'`,
      name,
    );
  }
  if (typeof trimmed.normalize === "function") {
    return trimmed.normalize("NFC");
  }
  return trimmed;
}

export function normalizeAccountId(value, name) {
  const raw = assertString(value, name).trim();
  if (raw.length === 0) {
    fail(ValidationErrorCode.INVALID_ACCOUNT_ID, `${name} must be a non-empty string`, name);
  }

  if (raw.slice(0, 5).toLowerCase() === "uaid:" || /^[0-9a-fA-F]{64}$/.test(raw)) {
    return normalizeUaidLiteral(raw, name);
  }
  if (raw.slice(0, 7).toLowerCase() === "opaque:") {
    return normalizeOpaqueLiteral(raw, name);
  }

  const atIndex = raw.lastIndexOf("@");
  if (atIndex !== -1) {
    const identifier = raw.slice(0, atIndex).trim();
    const domain = raw.slice(atIndex + 1).trim();
    if (identifier.length === 0) {
      fail(
        ValidationErrorCode.INVALID_ACCOUNT_ID,
        `${name} must include characters before '@'`,
        name,
      );
    }
    if (domain.length === 0) {
      fail(
        ValidationErrorCode.INVALID_ACCOUNT_ID,
        `${name} must include a domain after '@'`,
        name,
      );
    }
    let canonicalDomain;
    try {
      canonicalDomain = canonicalizeDomainLabel(domain);
    } catch (error) {
      if (error instanceof AccountAddressError) {
        throw createValidationError(
          ValidationErrorCode.INVALID_ACCOUNT_ID,
          error.message,
          name,
          error,
        );
      }
      throw error;
    }

    try {
      AccountAddress.parseAny(identifier, undefined, canonicalDomain);
      fail(
        ValidationErrorCode.INVALID_ACCOUNT_ID,
        `${name} must not append '@domain' to encoded addresses`,
        name,
      );
    } catch (error) {
      if (error instanceof AccountAddressError && !isAliasFormatError(error)) {
        throw createValidationError(
          ValidationErrorCode.INVALID_ACCOUNT_ID,
          error.message,
          name,
          error,
        );
      }
      if (!(error instanceof AccountAddressError)) {
        throw error;
      }
    }

    const canonicalAlias = normalizeNameLiteral(identifier, `${name}.alias`);
    try {
      const address = tryAccountAddressFromSignatory(
        identifier,
        canonicalDomain,
        `${name}.identifier`,
      );
      if (address) {
        return address.toIH58();
      }
    } catch (error) {
      if (error instanceof ValidationError || error instanceof AccountAddressError) {
        return `${canonicalAlias}@${canonicalDomain}`;
      }
      throw error;
    }
    return `${canonicalAlias}@${canonicalDomain}`;
  }

  try {
    const { address } = AccountAddress.parseAny(raw);
    return address.toIH58();
  } catch (error) {
    if (error instanceof AccountAddressError) {
      fail(
        ValidationErrorCode.INVALID_ACCOUNT_ID,
        `${name} must be IH58/compressed/0x, uaid:, opaque:, or <alias|public_key>@domain`,
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
      `${name} must be a canonical IH58 account id`,
      name,
    );
  }
  if (raw.slice(0, 5).toLowerCase() === "uaid:" || raw.slice(0, 7).toLowerCase() === "opaque:") {
    fail(
      ValidationErrorCode.INVALID_ACCOUNT_ID,
      `${name} must be a canonical IH58 account id`,
      name,
    );
  }
  let parsed;
  try {
    parsed = AccountAddress.parseAny(raw);
  } catch (error) {
    if (error instanceof AccountAddressError) {
      throw createValidationError(
        ValidationErrorCode.INVALID_ACCOUNT_ID,
        `${name} must be a canonical IH58 account id`,
        name,
        error,
      );
    }
    throw error;
  }
  if (parsed.format !== AccountAddressFormat.IH58) {
    fail(
      ValidationErrorCode.INVALID_ACCOUNT_ID,
      `${name} must be a canonical IH58 account id`,
      name,
    );
  }
  const canonical = parsed.address.toIH58();
  if (raw !== canonical) {
    fail(
      ValidationErrorCode.INVALID_ACCOUNT_ID,
      `${name} must use canonical IH58 account id form`,
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
  const doubleHashIndex = raw.indexOf("##");
  if (doubleHashIndex !== -1) {
    const definition = raw.slice(0, doubleHashIndex);
    const accountPart = raw.slice(doubleHashIndex + 2);
    if (accountPart.length === 0) {
      fail(
        ValidationErrorCode.INVALID_ASSET_ID,
        `${name} must include an account after '##'`,
        name,
      );
    }
    return `${definition}##${normalizeAccountId(
      accountPart,
      `${name}.account`,
    )}`;
  }
  const segments = raw.split("#");
  if (segments.length >= 3) {
    const accountPart = segments.pop();
    if (accountPart === undefined || accountPart.length === 0) {
      fail(ValidationErrorCode.INVALID_ASSET_ID, `${name} must include an account segment`, name);
    }
    segments.push(normalizeAccountId(accountPart, `${name}.account`));
    return segments.join("#");
  }
  if (raw.includes("#")) {
    fail(
      ValidationErrorCode.INVALID_ASSET_ID,
      `${name} must use '##' to separate asset definitions and account IDs`,
      name,
    );
  }
  return raw;
}
