import { Buffer } from "buffer";
import { createValidationError, ValidationErrorCode } from "./validationError.js";

function fail(code, message, path) {
  throw createValidationError(code, message, path);
}

function crc16(tag, body) {
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

  return crc & 0xffff;
}

export function canonicalHashLiteral(buf) {
  const normalized = Buffer.from(buf);
  if (normalized.length !== 32) {
    fail(ValidationErrorCode.INVALID_HEX, "hash must be 32 bytes");
  }
  normalized[normalized.length - 1] |= 1;
  const body = normalized.toString("hex").toUpperCase();
  const checksum = crc16("hash", body).toString(16).toUpperCase().padStart(4, "0");
  return `hash:${body}#${checksum}`;
}

export function parseHashLiteralToBuffer(literal, name) {
  const match = /^hash:([0-9A-Fa-f]{64})#([0-9A-Fa-f]{4})$/.exec(literal.trim());
  if (!match) {
    fail(
      ValidationErrorCode.INVALID_HEX,
      `${name} must be a canonical "hash:<HEX>#<CRC>" literal`,
      name,
    );
  }
  const [, body, checksum] = match;
  const bodyUpper = body.toUpperCase();
  const expected = crc16("hash", bodyUpper).toString(16).toUpperCase().padStart(4, "0");
  if (expected !== checksum.toUpperCase()) {
    fail(
      ValidationErrorCode.INVALID_HEX,
      `${name} has invalid checksum; expected ${expected}`,
      name,
    );
  }
  return Buffer.from(bodyUpper, "hex");
}

export function parseHashLiteral(literal, name) {
  return canonicalHashLiteral(parseHashLiteralToBuffer(literal, name));
}

export function assertString(value, name) {
  if (typeof value !== "string" || value.length === 0) {
    fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty string`, name);
  }
  return value;
}

export function assertNonBlankString(value, name) {
  const raw = assertString(value, name);
  const trimmed = raw.trim();
  if (trimmed.length === 0) {
    fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty string`, name);
  }
  return trimmed;
}

export function assertExactNonBlankString(value, name) {
  const raw = assertString(value, name);
  if (raw.trim().length === 0) {
    fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty string`, name);
  }
  if (raw.trim() !== raw) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must not contain surrounding whitespace`,
      name,
    );
  }
  return raw;
}

export function assertWellFormedUtf16(value, name) {
  for (let index = 0; index < value.length; index += 1) {
    const codeUnit = value.charCodeAt(index);
    if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) {
        fail(
          ValidationErrorCode.INVALID_STRING,
          `${name} must not contain unpaired UTF-16 surrogates`,
          name,
        );
      }
      index += 1;
    } else if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
      fail(
        ValidationErrorCode.INVALID_STRING,
        `${name} must not contain unpaired UTF-16 surrogates`,
        name,
      );
    }
  }
}
