import { createValidationError, ValidationErrorCode } from "./validationError.js";

const IBAN_LENGTHS = new Map([
  ["AD", 24],
  ["AE", 23],
  ["AL", 28],
  ["AT", 20],
  ["AZ", 28],
  ["BA", 20],
  ["BE", 16],
  ["BG", 22],
  ["BH", 22],
  ["BR", 29],
  ["BY", 28],
  ["CH", 21],
  ["CR", 22],
  ["CY", 28],
  ["CZ", 24],
  ["DE", 22],
  ["DK", 18],
  ["DO", 28],
  ["EE", 20],
  ["EG", 27],
  ["ES", 24],
  ["FI", 18],
  ["FO", 18],
  ["FR", 27],
  ["GB", 22],
  ["GE", 22],
  ["GI", 23],
  ["GL", 18],
  ["GR", 27],
  ["GT", 28],
  ["HR", 21],
  ["HU", 28],
  ["IE", 22],
  ["IL", 23],
  ["IQ", 23],
  ["IR", 26],
  ["IS", 26],
  ["IT", 27],
  ["JO", 30],
  ["KW", 30],
  ["KZ", 20],
  ["LB", 28],
  ["LC", 32],
  ["LI", 21],
  ["LT", 20],
  ["LU", 20],
  ["LV", 21],
  ["MC", 27],
  ["MD", 24],
  ["ME", 22],
  ["MK", 19],
  ["MR", 27],
  ["MT", 31],
  ["MU", 30],
  ["NL", 18],
  ["NO", 15],
  ["PK", 24],
  ["PL", 28],
  ["PS", 29],
  ["PT", 25],
  ["QA", 29],
  ["RO", 24],
  ["RS", 22],
  ["SA", 24],
  ["SC", 31],
  ["SE", 24],
  ["SI", 19],
  ["SK", 24],
  ["SM", 27],
  ["ST", 25],
  ["SV", 28],
  ["TL", 23],
  ["TN", 24],
  ["TR", 26],
  ["UA", 29],
  ["VA", 22],
  ["VG", 24],
  ["XK", 20],
]);
const IBAN_LENGTH_VALUES = Array.from(IBAN_LENGTHS.values());
const IBAN_MIN_LENGTH = Math.min(...IBAN_LENGTH_VALUES);
const IBAN_MAX_LENGTH = Math.max(...IBAN_LENGTH_VALUES);
const IBAN_ALNUM_REGEX = /^[A-Z0-9]+$/;

/**
 * Determine whether a string looks like an IBAN (format check only).
 * @param {string} value
 * @returns {boolean}
 */
export function looksLikeIban(value) {
  if (typeof value !== "string") {
    return false;
  }
  const normalized = value.replace(/\s+/g, "").toUpperCase();
  const len = normalized.length;
  return len >= IBAN_MIN_LENGTH && len <= IBAN_MAX_LENGTH && IBAN_ALNUM_REGEX.test(normalized);
}

/**
 * Normalize an IBAN (remove whitespace, uppercase) and enforce checksum rules.
 * @param {string} value
 * @param {string} [label]
 * @returns {string}
 */
export function normalizeIban(value, label = "iban") {
  if (typeof value !== "string") {
    throw createValidationError(ValidationErrorCode.INVALID_STRING, `${label} must be a string`, label);
  }
  const normalized = value.replace(/\s+/g, "").toUpperCase();
  if (!normalized) {
    throw invalidIban(`${label} must not be empty`, label);
  }
  if (!IBAN_ALNUM_REGEX.test(normalized)) {
    throw invalidIban(`${label} must be uppercase alphanumeric`, label);
  }
  if (normalized.length < IBAN_MIN_LENGTH || normalized.length > IBAN_MAX_LENGTH) {
    throw invalidIban(
      `${label} must be an IBAN length (${IBAN_MIN_LENGTH}-${IBAN_MAX_LENGTH} characters)`,
      label,
    );
  }
  const country = normalized.slice(0, 2);
  const expectedLength = IBAN_LENGTHS.get(country);
  if (!expectedLength) {
    throw invalidIban(
      `${label} must start with a supported IBAN country code (got ${country})`,
      label,
    );
  }
  if (normalized.length !== expectedLength) {
    throw invalidIban(
      `${label} must be ${expectedLength} characters long for country ${country}`,
      label,
    );
  }
  if (!isAsciiDigit(normalized.charCodeAt(2)) || !isAsciiDigit(normalized.charCodeAt(3))) {
    throw invalidIban(`${label} must use numeric check digits`, label);
  }
  if (!ibanHasValidChecksum(normalized)) {
    throw invalidIban(`${label} failed the IBAN mod-97 check`, label);
  }
  return normalized;
}

function ibanHasValidChecksum(iban) {
  const rearranged = `${iban.slice(4)}${iban.slice(0, 4)}`;
  let remainder = 0;
  for (const ch of rearranged) {
    const digits =
      ch >= "A" && ch <= "Z"
        ? String(ch.charCodeAt(0) - 55)
        : ch;
    for (const digit of digits) {
      remainder = (remainder * 10 + (digit.charCodeAt(0) - 48)) % 97;
    }
  }
  return remainder === 1;
}

function isAsciiDigit(charCode) {
  return charCode >= 48 && charCode <= 57;
}

function invalidIban(message, label) {
  return createValidationError(ValidationErrorCode.INVALID_IBAN, message, label);
}
