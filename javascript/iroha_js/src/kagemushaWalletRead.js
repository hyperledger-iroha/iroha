// Current Kagemusha wallet evidence transport. These originals are unverified;
// HTTP success never establishes issuance, admission, or settlement.
const ID = /^[0-9a-f]{64}$/u;
const MAX_U64 = 18446744073709551615n;
export const KAGEMUSHA_WALLET_READ_LIMITS_V1 = Object.freeze({
  load: 65_536,
  loadFinality: 262_144,
  loadEpoch: 262_144,
  ledgerFinality: 1_327_104,
  ledgerEpoch: 262_144,
});

export function kagemushaWalletReadSelection(kind, input) {
  if (input === null || typeof input !== "object" || Array.isArray(input)) {
    throw new TypeError("Kagemusha wallet selection must be an object");
  }
  const ledger = kind === "ledgerFinality" || kind === "ledgerEpoch";
  const epoch = kind === "loadEpoch" || kind === "ledgerEpoch";
  if (!Object.hasOwn(KAGEMUSHA_WALLET_READ_LIMITS_V1, kind)) throw new TypeError("Unknown Kagemusha wallet original kind");
  const keys = ["schemeId", "walletId", ledger ? "entrypointHash" : "requestId", ...(epoch ? ["boundary"] : [])];
  if (Object.keys(input).some((key) => !keys.includes(key))) throw new TypeError("Unsupported Kagemusha wallet selection field");
  const selection = {};
  for (const key of keys) {
    const value = input[key];
    if (key === "boundary") {
      if (typeof value !== "string" || !/^[1-9][0-9]*$/u.test(value) || value.length > 20 || BigInt(value) < 2n || BigInt(value) > MAX_U64) {
        throw new TypeError("Kagemusha boundary must be a canonical u64 decimal string at least 2");
      }
    } else if (typeof value !== "string" || !ID.test(value) || /^0+$/u.test(value)) {
      throw new TypeError(`Kagemusha ${key} must be nonzero lowercase 32-byte hex`);
    }
    selection[key] = value;
  }
  if (ledger && (Number.parseInt(selection.entrypointHash.slice(-2), 16) & 1) !== 1) {
    throw new TypeError("Kagemusha entrypointHash must carry the Iroha hash marker bit");
  }
  const root = `/v1/kagemusha/${selection.schemeId}/wallets/${selection.walletId}`;
  const selected = ledger ? `/ledger/${selection.entrypointHash}` : `/loads/${selection.requestId}`;
  const suffix = epoch ? `/epochs/${selection.boundary}` : kind === "load" ? "" : "/finality";
  return { selection: Object.freeze(selection), path: root + selected + suffix, maxBytes: KAGEMUSHA_WALLET_READ_LIMITS_V1[kind] };
}

export function requireKagemushaWalletReadEndpoint(baseUrl) {
  const url = new URL(baseUrl);
  if (url.protocol !== "https:" || url.username || url.password || url.search || url.hash || !["", "/"].includes(url.pathname)) {
    throw new TypeError("Kagemusha wallet reads require an HTTPS Torii root without credentials, query, or fragment");
  }
  return url;
}

export function validateKagemushaWalletOriginalResponse(response, expectedUrl, header) {
  if (response.redirected === true || (response.url && response.url !== expectedUrl)) {
    throw new TypeError("Kagemusha wallet original response URL does not match the request");
  }
  if (!/^[ \t]*application\/x-norito[ \t]*$/iu.test(header("content-type") ?? "")) {
    throw new TypeError("Kagemusha wallet originals require application/x-norito");
  }
  const encoding = header("content-encoding");
  if (encoding !== null && encoding.trim().toLowerCase() !== "identity") {
    throw new TypeError("Kagemusha wallet originals must not be content encoded");
  }
}
