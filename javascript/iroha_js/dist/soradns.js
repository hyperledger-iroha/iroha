import { getNativeBinding } from "./native.js";

const CANONICAL_SUFFIX = "gw.sora.id";
const PRETTY_SUFFIX = "gw.sora.name";
const CANONICAL_WILDCARD = "*.gw.sora.id";

function requireSoradnsNativeBinding() {
  const binding = getNativeBinding();
  if (
    !binding ||
    typeof binding.soradnsDeriveGatewayHosts !== "function"
  ) {
    throw new Error(
      "SoraDNS helpers require the native iroha_js_host module. Run `npm run build:native` before using these helpers.",
    );
  }
  return binding;
}

function canonicalizeHost(host) {
  if (typeof host !== "string") {
    return null;
  }
  const trimmed = host.trim();
  if (!trimmed || trimmed.startsWith(".") || trimmed.endsWith(".")) {
    return null;
  }
  for (let index = 0; index < trimmed.length; index += 1) {
    const code = trimmed.charCodeAt(index);
    const isAlpha =
      (code >= 0x41 && code <= 0x5a) || (code >= 0x61 && code <= 0x7a);
    const isDigit = code >= 0x30 && code <= 0x39;
    if (!isAlpha && !isDigit && code !== 0x2d && code !== 0x2e) {
      return null;
    }
  }
  return trimmed.toLowerCase();
}

function normalisePattern(pattern) {
  if (typeof pattern !== "string") {
    return null;
  }
  const trimmed = pattern.trim();
  if (!trimmed) {
    return null;
  }
  return trimmed.toLowerCase();
}

function readDerivedField(object, ...names) {
  for (const name of names) {
    if (Object.prototype.hasOwnProperty.call(object, name)) {
      return object[name];
    }
  }
  return undefined;
}

/**
 * Compute the deterministic gateway hosts for a SoraDNS FQDN.
 * Returns an immutable object with the canonical host, pretty host,
 * wildcard pattern, and convenience utilities.
 *
 * @param {string} fqdn
 * @returns {{
 *   normalizedName: string,
 *   canonicalLabel: string,
 *   canonicalHost: string,
 *   prettyHost: string,
 *   canonicalWildcard: string,
 *   hostPatterns: string[],
 *   matchesHost(host: string): boolean
 * }}
 */
export function deriveGatewayHosts(fqdn) {
  if (typeof fqdn !== "string") {
    throw new TypeError("fqdn must be a string");
  }
  const binding = requireSoradnsNativeBinding();
  const derived = binding.soradnsDeriveGatewayHosts(fqdn);
  const normalizedName = String(
    readDerivedField(derived, "normalized_name", "normalizedName") ?? "",
  );
  const canonicalLabel = String(
    readDerivedField(derived, "canonical_label", "canonicalLabel") ?? "",
  );
  const canonicalHost = String(
    readDerivedField(derived, "canonical_host", "canonicalHost") ?? "",
  );
  const canonicalWildcard = String(
    readDerivedField(derived, "canonical_wildcard", "canonicalWildcard") ?? "",
  );
  const prettyHost = String(
    readDerivedField(derived, "pretty_host", "prettyHost") ?? "",
  );
  const hostPatternsSource =
    readDerivedField(derived, "host_patterns", "hostPatterns") ?? [];
  const hostPatterns = Array.isArray(hostPatternsSource)
    ? hostPatternsSource.map((pattern) => String(pattern))
    : [];
  return Object.freeze({
    normalizedName,
    canonicalLabel,
    canonicalHost,
    prettyHost,
    canonicalWildcard,
    hostPatterns,
    matchesHost(host) {
      const candidate = canonicalizeHost(host);
      if (!candidate) {
        return false;
      }
      return (
        candidate === canonicalHost ||
        candidate === prettyHost
      );
    },
  });
}

/**
 * Verify that a GAR host pattern list authorises the required hosts.
 *
 * @param {Iterable<string>} patterns
 * @param {{ canonicalHost: string, canonicalWildcard: string, prettyHost: string }} derived
 * @returns {boolean}
 */
export function hostPatternsCoverDerivedHosts(patterns, derived) {
  if (!patterns || !derived) {
    return false;
  }
  const set = new Set();
  for (const pattern of patterns) {
    const normalised = normalisePattern(pattern);
    if (normalised) {
      set.add(normalised);
    }
  }
  return (
    set.has(String(derived.canonicalHost ?? "").toLowerCase()) &&
    set.has(String(derived.canonicalWildcard ?? "").toLowerCase()) &&
    set.has(String(derived.prettyHost ?? "").toLowerCase())
  );
}

export function canonicalGatewaySuffix() {
  return CANONICAL_SUFFIX;
}

export function prettyGatewaySuffix() {
  return PRETTY_SUFFIX;
}

export function canonicalGatewayWildcard() {
  return CANONICAL_WILDCARD;
}
