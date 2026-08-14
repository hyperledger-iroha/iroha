/** Decode strict standard base64 while preserving browser compatibility. */
export function strictDecodeBase64(value) {
  const compact = value.replace(/\s+/gu, "");
  validateBase64Alphabet(compact);
  if (typeof Buffer !== "undefined") {
    const decoded = Buffer.from(compact, "base64");
    ensureBase64RoundTrip(decoded, compact);
    return Uint8Array.from(decoded);
  }
  if (typeof atob === "function") {
    let binary;
    try {
      binary = atob(compact);
    } catch (error) {
      throw error instanceof Error ? error : new Error(String(error));
    }
    const reencoded = typeof btoa === "function" ? btoa(binary) : null;
    if (reencoded && !base64StringsEquivalent(reencoded, compact)) {
      throw new Error("invalid base64 payload");
    }
    const decoded = new Uint8Array(binary.length);
    for (let idx = 0; idx < binary.length; idx += 1) {
      decoded[idx] = binary.charCodeAt(idx);
    }
    return decoded;
  }
  throw new Error("no base64 decoder available");
}

const BASE64_ALPHABET_PATTERN = /^[A-Za-z0-9+/]*={0,2}$/u;
const BASE64_DATA_PATTERN = /[A-Za-z0-9+/]/u;

function validateBase64Alphabet(value) {
  if (!value) {
    throw new Error("payload is empty");
  }
  if (!BASE64_ALPHABET_PATTERN.test(value) || !BASE64_DATA_PATTERN.test(value)) {
    throw new Error("invalid base64 payload");
  }
}

function ensureBase64RoundTrip(buffer, original) {
  const canonical =
    buffer.length === 0 ? "" : Buffer.from(buffer).toString("base64");
  if (!base64StringsEquivalent(canonical, original)) {
    throw new Error("invalid base64 payload");
  }
}

function base64StringsEquivalent(left, right) {
  return stripBase64Padding(left) === stripBase64Padding(right);
}

function stripBase64Padding(value) {
  return value.replace(/=+$/u, "");
}

/** Recursively sort object keys for deterministic validation messages. */
export function sortJsonForErrorMessage(value) {
  if (Array.isArray(value)) {
    return value.map((item) => sortJsonForErrorMessage(item));
  }
  if (!value || typeof value !== "object") {
    return value;
  }
  const sorted = {};
  for (const key of Object.keys(value).sort()) {
    sorted[key] = sortJsonForErrorMessage(value[key]);
  }
  return sorted;
}
