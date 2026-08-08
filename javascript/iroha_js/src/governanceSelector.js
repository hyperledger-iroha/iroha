/** Maximum UTF-8 byte length of a canonical first-release governance selector. */
export const GOVERNANCE_SELECTOR_V1_MAX_BYTES = 128;

/**
 * Return whether `value` is one canonical V1 governance path selector.
 *
 * The grammar is ASCII-only RFC 3986 unreserved text, with a leading dot
 * excluded to prevent path-normalization aliases.
 */
export function isCanonicalGovernanceSelectorV1(value) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > GOVERNANCE_SELECTOR_V1_MAX_BYTES
  ) {
    return false;
  }
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    const unreserved =
      (code >= 48 && code <= 57) ||
      (code >= 65 && code <= 90) ||
      (code >= 97 && code <= 122) ||
      code === 45 ||
      code === 46 ||
      code === 95 ||
      code === 126;
    if (!unreserved || (index === 0 && code === 46)) {
      return false;
    }
  }
  return true;
}
