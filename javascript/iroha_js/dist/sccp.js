import { keccak_256 } from "@noble/hashes/sha3";

import { AccountAddress } from "./address.js";
import { blake2b256 } from "./blake2b.js";

/** First-release SCCP protocol domains. Tags 3 and 4 are retired and reserved. */
export const SCCP_DOMAIN_SORA = 0;
export const SCCP_DOMAIN_ETH = 1;
export const SCCP_DOMAIN_BSC = 2;
export const SCCP_DOMAIN_TRON = 5;

/** Closed first-release SCCP payload codec inventory. */
export const SCCP_CODEC_CANONICAL_TEXT = 1;
export const SCCP_CODEC_EVM_ADDRESS20 = 2;
export const SCCP_CODEC_TRON_ADDRESS21 = 5;

export const SCCP_CODEC_KEYS = Object.freeze({
  [SCCP_CODEC_CANONICAL_TEXT]: "canonical_text",
  [SCCP_CODEC_EVM_ADDRESS20]: "evm_address20",
  [SCCP_CODEC_TRON_ADDRESS21]: "tron_address21",
});

/** SCCP V1 carries only the exact value-moving transfer payload. */
export const SCCP_PAYLOAD_KINDS = Object.freeze(["transfer"]);

const SOURCE_EVENT_PREFIX = new TextEncoder().encode("sccp:source:event:v1");
const MAX_WIRE_BYTES = 16 * 1024 * 1024;
const BN254_BASE_FIELD_MODULUS =
  0x30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47n;
const ROUTE_KEY = /^[a-z0-9](?:[a-z0-9_-]{0,62}[a-z0-9])?$/u;

const NETWORKS = Object.freeze({
  "sora-nexus": Object.freeze({ tag: 0, domain: SCCP_DOMAIN_SORA, sora: true }),
  "sora-taira": Object.freeze({ tag: 1, domain: SCCP_DOMAIN_SORA, sora: true }),
  "ethereum-mainnet": Object.freeze({ tag: 2, domain: SCCP_DOMAIN_ETH, sora: false }),
  "ethereum-sepolia": Object.freeze({ tag: 3, domain: SCCP_DOMAIN_ETH, sora: false }),
  "bsc-mainnet": Object.freeze({ tag: 4, domain: SCCP_DOMAIN_BSC, sora: false }),
  "bsc-testnet": Object.freeze({ tag: 5, domain: SCCP_DOMAIN_BSC, sora: false }),
  "tron-mainnet": Object.freeze({ tag: 10, domain: SCCP_DOMAIN_TRON, sora: false }),
  "tron-nile": Object.freeze({ tag: 11, domain: SCCP_DOMAIN_TRON, sora: false }),
  "tron-shasta": Object.freeze({ tag: 12, domain: SCCP_DOMAIN_TRON, sora: false }),
});

export const SCCP_NETWORK_PROFILES = Object.freeze(
  Object.fromEntries(
    Object.entries(NETWORKS).map(([profile, descriptor]) => [
      profile,
      Object.freeze({ profile, ...descriptor }),
    ]),
  ),
);

const NETWORK_WIRE_NAMES = Object.freeze(
  Object.fromEntries(
    Object.keys(NETWORKS).map((profile) => [profile.replaceAll("-", "_"), profile]),
  ),
);

const NATIVE_BACKENDS = Object.freeze({
  ethereum_beacon_v1: new Set(["ethereum-mainnet", "ethereum-sepolia"]),
  bsc_parlia_v1: new Set(["bsc-mainnet", "bsc-testnet"]),
  tron_dpos_v1: new Set(["tron-mainnet", "tron-nile", "tron-shasta"]),
});

const DESTINATION_BACKENDS = Object.freeze({
  evm_groth16_bn254_v1: "evm",
  tron_groth16_bn254_v1: "tron",
});

const CAPABILITY_PATHS = Object.freeze({
  registry_path: "/v1/sccp/registry",
  message_bundle_path: "/v1/sccp/proofs/message/{message_id}",
  proof_request_path: "/v1/sccp/proof-requests/{message_id}",
  recent_messages_path: "/v1/sccp/messages/recent",
  proof_submit_path: "/v1/bridge/proofs/submit",
  native_message_submit_path: "/v1/bridge/messages",
});

const BRIDGE_RESPONSE_FIELDS = new Set([
  "submitted",
  "payload_kind",
  "message_id_hex",
  "backend",
  "counterparty_domain",
  "counterparty_chain",
  "manifest_hash_hex",
  "range_start_height",
  "range_end_height",
  "creation_time_ms",
  "tx_hash_hex",
  "transaction_payload_b64",
  "signing_message_b64",
]);

function plainObject(value, label) {
  if (
    value === null ||
    typeof value !== "object" ||
    Array.isArray(value) ||
    (Object.getPrototypeOf(value) !== Object.prototype &&
      Object.getPrototypeOf(value) !== null)
  ) {
    throw new TypeError(`${label} must be a plain object`);
  }
  return value;
}

function exactFields(value, allowed, label, required = allowed) {
  const record = plainObject(value, label);
  for (const key of Object.keys(record)) {
    if (!allowed.has(key)) {
      throw new TypeError(`${label} contains unknown or retired field \`${key}\``);
    }
  }
  for (const key of required) {
    if (!Object.prototype.hasOwnProperty.call(record, key)) {
      throw new TypeError(`${label} is missing required field \`${key}\``);
    }
  }
  return record;
}

function canonicalText(value, label, maximumBytes = 4096) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value !== value.trim() ||
    new TextEncoder().encode(value).length > maximumBytes
  ) {
    throw new TypeError(`${label} must be canonical nonempty text`);
  }
  return value;
}

function integer(value, label, minimum, maximum = Number.MAX_SAFE_INTEGER) {
  if (!Number.isSafeInteger(value) || value < minimum || value > maximum) {
    throw new TypeError(`${label} must be an integer in ${minimum}..${maximum}`);
  }
  return value;
}

function boolean(value, label) {
  if (typeof value !== "boolean") throw new TypeError(`${label} must be boolean`);
  return value;
}

function array(value, label) {
  if (!Array.isArray(value)) throw new TypeError(`${label} must be an array`);
  return value;
}

function binary(value, label) {
  if (value instanceof Uint8Array) return Uint8Array.from(value);
  if (typeof Buffer !== "undefined" && Buffer.isBuffer(value)) {
    return Uint8Array.from(value);
  }
  if (value instanceof ArrayBuffer) return new Uint8Array(value.slice(0));
  if (ArrayBuffer.isView(value)) {
    return Uint8Array.from(
      new Uint8Array(value.buffer, value.byteOffset, value.byteLength),
    );
  }
  throw new TypeError(`${label} must be binary data`);
}

function allZero(value) {
  return value.every((byte) => byte === 0);
}

function lowerHexBytes(value) {
  return Array.from(value, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function exactLowerHex(value, label, byteLength, { prefix = false, nonzero = true } = {}) {
  const pattern = prefix
    ? new RegExp(`^0x[0-9a-f]{${byteLength * 2}}$`, "u")
    : new RegExp(`^[0-9a-f]{${byteLength * 2}}$`, "u");
  if (typeof value !== "string" || !pattern.test(value)) {
    throw new TypeError(
      `${label} must be canonical lowercase ${prefix ? "0x-prefixed " : ""}${byteLength}-byte hex`,
    );
  }
  const body = prefix ? value.slice(2) : value;
  if (nonzero && /^0+$/u.test(body)) throw new TypeError(`${label} must be nonzero`);
  return value;
}

function exactUpperHex(value, label, byteLength, { nonzero = true } = {}) {
  const pattern = new RegExp(`^[0-9A-F]{${byteLength * 2}}$`, "u");
  if (typeof value !== "string" || !pattern.test(value)) {
    throw new TypeError(`${label} must be canonical uppercase ${byteLength}-byte hex`);
  }
  if (nonzero && /^0+$/u.test(value)) throw new TypeError(`${label} must be nonzero`);
  return value;
}

function exactVariableHex(value, label, { maximumBytes = MAX_WIRE_BYTES } = {}) {
  if (
    typeof value !== "string" ||
    !/^0x(?:[0-9a-f]{2})+$/u.test(value) ||
    (value.length - 2) / 2 > maximumBytes
  ) {
    throw new TypeError(`${label} must be canonical nonempty lowercase 0x-prefixed hex`);
  }
  return value;
}

function canonicalBase64(value, label, { maximumBytes = MAX_WIRE_BYTES } = {}) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length % 4 !== 0 ||
    !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/u.test(value)
  ) {
    throw new TypeError(`${label} must be canonical padded base64`);
  }
  const decoded = Uint8Array.from(Buffer.from(value, "base64"));
  if (Buffer.from(decoded).toString("base64") !== value) {
    throw new TypeError(`${label} must be canonical padded base64`);
  }
  if (decoded.length === 0 || decoded.length > maximumBytes) {
    throw new TypeError(`${label} is outside its byte-size bound`);
  }
  return decoded;
}

function canonicalPath(value, label) {
  const path = canonicalText(value, label, 1024);
  if (
    !path.startsWith("/") ||
    path.includes("//") ||
    path.includes("?") ||
    path.includes("#") ||
    path.includes("%") ||
    path.includes("\\")
  ) {
    throw new TypeError(`${label} must be a canonical absolute Torii path`);
  }
  return path;
}

function exactCapabilityPath(value, field, optional = false) {
  if (optional && (value === null || value === undefined)) return null;
  const path = canonicalPath(value, field);
  if (path !== CAPABILITY_PATHS[field]) {
    throw new TypeError(`${field} does not match the SCCP V1 endpoint`);
  }
  return path;
}

function profile(value, label) {
  const key = canonicalText(value, label, 64);
  const descriptor = NETWORKS[key];
  if (!descriptor) throw new TypeError(`${label} is not a supported SCCP V1 profile`);
  return Object.freeze({ profile: key, ...descriptor });
}

function parseNetwork(value, label) {
  const record = exactFields(value, new Set(["network", "profile"]), label);
  if (record.profile !== null) throw new TypeError(`${label}.profile must be null`);
  const wire = canonicalText(record.network, `${label}.network`, 64);
  const key = NETWORK_WIRE_NAMES[wire];
  if (!key) throw new TypeError(`${label}.network is unsupported or retired`);
  return profile(key, `${label}.network`);
}

function parseLaneId(value, label) {
  const record = exactFields(value, new Set(["source", "target"]), label);
  const source = parseNetwork(record.source, `${label}.source`);
  const target = parseNetwork(record.target, `${label}.target`);
  if (source.sora || target.profile !== "sora-taira" || source.domain === target.domain) {
    throw new TypeError(`${label} must be an exact supported external-to-Taira lane`);
  }
  return Object.freeze({ source, target });
}

function sameLane(left, right) {
  return (
    left.source.profile === right.source.profile &&
    left.target.profile === right.target.profile
  );
}

function emitterFamily(network) {
  return network.profile.startsWith("tron-") ? "tron" : "evm";
}

function parseUnitBackend(value, label, contentField, allowed) {
  const tagField = contentField === "protocol" ? "backend" : "backend";
  const record = exactFields(value, new Set([tagField, contentField]), label);
  if (record[contentField] !== null) {
    throw new TypeError(`${label}.${contentField} must be null`);
  }
  const backend = canonicalText(record[tagField], `${label}.${tagField}`, 64);
  if (!Object.prototype.hasOwnProperty.call(allowed, backend)) {
    throw new TypeError(`${label}.${tagField} is unsupported or retired`);
  }
  return backend;
}

function parseNativeTrustAnchor(value, lane, label) {
  if (value === null) return null;
  const record = exactFields(
    value,
    new Set(["backend", "anchor_hash", "checkpoint_height"]),
    label,
  );
  const backend = parseUnitBackend(
    record.backend,
    `${label}.backend`,
    "protocol",
    NATIVE_BACKENDS,
  );
  if (!NATIVE_BACKENDS[backend].has(lane.source.profile)) {
    throw new TypeError(`${label}.backend does not match the lane source`);
  }
  exactUpperHex(record.anchor_hash, `${label}.anchor_hash`, 32);
  integer(record.checkpoint_height, `${label}.checkpoint_height`, 1);
  return record;
}

function parseActivation(value, label) {
  const record = exactFields(value, new Set(["activation", "direction"]), label);
  if (record.direction !== null) throw new TypeError(`${label}.direction must be null`);
  const activation = canonicalText(record.activation, `${label}.activation`, 32);
  if (!["staged", "bidirectional", "inbound_only", "paused", "retired"].includes(activation)) {
    throw new TypeError(`${label}.activation is unsupported`);
  }
  return activation;
}

function parseEmitter(value, lane, label) {
  const record = exactFields(value, new Set(["emitter", "identity"]), label);
  const family = canonicalText(record.emitter, `${label}.emitter`, 16);
  if (family !== emitterFamily(lane.source)) {
    throw new TypeError(`${label}.emitter does not match the lane source`);
  }
  const identity = exactFields(
    record.identity,
    new Set(["address", "runtime_code_hash", "route_config_hash"]),
    `${label}.identity`,
  );
  const address = exactUpperHex(identity.address, `${label}.identity.address`, 20);
  const runtime = exactUpperHex(
    identity.runtime_code_hash,
    `${label}.identity.runtime_code_hash`,
    32,
  );
  const configuration = exactUpperHex(
    identity.route_config_hash,
    `${label}.identity.route_config_hash`,
    32,
  );
  if (runtime === configuration) {
    throw new TypeError(`${label} runtime and route-configuration hashes must be distinct`);
  }
  return Object.freeze({ family, address, runtime, configuration });
}

function parseSourceIdentity(value, lane, label) {
  const record = exactFields(value, new Set(["lane", "emitter"]), label);
  const identityLane = parseLaneId(record.lane, `${label}.lane`);
  if (!sameLane(identityLane, lane)) throw new TypeError(`${label}.lane does not match the route`);
  return parseEmitter(record.emitter, lane, `${label}.emitter`);
}

function parseG1(value, label) {
  const record = exactFields(value, new Set(["x", "y"]), label);
  const coordinates = [
    exactUpperHex(record.x, `${label}.x`, 32),
    exactUpperHex(record.y, `${label}.y`, 32),
  ];
  for (const [index, coordinate] of coordinates.entries()) {
    if (BigInt(`0x${coordinate}`) >= BN254_BASE_FIELD_MODULUS) {
      throw new TypeError(`${label}.${index === 0 ? "x" : "y"} is not a BN254 field element`);
    }
  }
  return coordinates;
}

function parseG2(value, label) {
  const fields = ["x_c0", "x_c1", "y_c0", "y_c1"];
  const record = exactFields(value, new Set(fields), label);
  return fields.map((field) => {
    const coordinate = exactUpperHex(record[field], `${label}.${field}`, 32);
    if (BigInt(`0x${coordinate}`) >= BN254_BASE_FIELD_MODULUS) {
      throw new TypeError(`${label}.${field} is not a BN254 field element`);
    }
    return coordinate;
  });
}

function parseVerifyingKey(value, label) {
  const record = exactFields(
    value,
    new Set(["version", "alpha1", "beta2", "gamma2", "delta2", "ic"]),
    label,
  );
  integer(record.version, `${label}.version`, 1, 1);
  const words = [
    ...parseG1(record.alpha1, `${label}.alpha1`),
    ...parseG2(record.beta2, `${label}.beta2`),
    ...parseG2(record.gamma2, `${label}.gamma2`),
    ...parseG2(record.delta2, `${label}.delta2`),
  ];
  const icFields = [
    "constant",
    "signal_0",
    "signal_1",
    "signal_2",
    "signal_3",
    "signal_4",
    "signal_5",
    "signal_6",
    "signal_7",
    "signal_8",
    "signal_9",
  ];
  const ic = exactFields(record.ic, new Set(icFields), `${label}.ic`);
  for (const field of icFields) words.push(...parseG1(ic[field], `${label}.ic.${field}`));
  if (words.length !== 36) throw new TypeError(`${label} must contain exactly 36 ABI words`);
  return Uint8Array.from(Buffer.from(words.join(""), "hex"));
}

function parseDestination(value, lane, label) {
  const record = exactFields(value, new Set(["family", "deployment"]), label);
  const family = canonicalText(record.family, `${label}.family`, 16);
  if (family !== emitterFamily(lane.source)) {
    throw new TypeError(`${label}.family does not match the lane source`);
  }
  const fields = [
    "token_address",
    "token_code_hash",
    "verifier_address",
    "verifier_code_hash",
    "verifying_key",
    "verifier_key_hash",
    "route_address",
    "route_code_hash",
    "taira_to_token_multiplier",
  ];
  const deployment = exactFields(record.deployment, new Set(fields), `${label}.deployment`);
  const addresses = ["token_address", "verifier_address", "route_address"].map((field) =>
    exactUpperHex(deployment[field], `${label}.deployment.${field}`, 20),
  );
  const hashes = ["token_code_hash", "verifier_code_hash", "verifier_key_hash", "route_code_hash"].map(
    (field) => exactUpperHex(deployment[field], `${label}.deployment.${field}`, 32),
  );
  if (new Set(addresses).size !== addresses.length || new Set(hashes).size !== hashes.length) {
    throw new TypeError(`${label}.deployment reuses a role-separated address or hash`);
  }
  const keyBytes = parseVerifyingKey(deployment.verifying_key, `${label}.deployment.verifying_key`);
  if (lowerHexBytes(keccak_256(keyBytes)).toUpperCase() !== deployment.verifier_key_hash) {
    throw new TypeError(`${label}.deployment.verifier_key_hash does not match verifying_key`);
  }
  integer(
    deployment.taira_to_token_multiplier,
    `${label}.deployment.taira_to_token_multiplier`,
    1_000_000_000,
    1_000_000_000,
  );
  return Object.freeze({
    family,
    routeAddress: addresses[2],
    routeCodeHash: hashes[3],
  });
}

function parseSettlement(value, label) {
  const record = exactFields(
    value,
    new Set(["asset_definition_id", "custody_account_id", "payload_amount_scale"]),
    label,
  );
  canonicalText(record.asset_definition_id, `${label}.asset_definition_id`, 512);
  const custody = canonicalText(record.custody_account_id, `${label}.custody_account_id`, 512);
  AccountAddress.fromAccountId(custody);
  integer(record.payload_amount_scale, `${label}.payload_amount_scale`, 9, 9);
}

function parseGovernedRoute(value, lane, label) {
  const fields = new Set([
    "lane_id",
    "route_id",
    "asset_key",
    "revision",
    "activation",
    "source_identity",
    "destination",
    "settlement",
  ]);
  const record = exactFields(value, fields, label);
  const routeLane = parseLaneId(record.lane_id, `${label}.lane_id`);
  if (!sameLane(routeLane, lane)) throw new TypeError(`${label}.lane_id does not match its lane`);
  for (const field of ["route_id", "asset_key"]) {
    if (typeof record[field] !== "string" || !ROUTE_KEY.test(record[field])) {
      throw new TypeError(`${label}.${field} must be canonical lowercase route text`);
    }
  }
  const revision = integer(record.revision, `${label}.revision`, 1, 0xffff_ffff);
  const activation = parseActivation(record.activation, `${label}.activation`);
  const source = parseSourceIdentity(record.source_identity, lane, `${label}.source_identity`);
  const destination = parseDestination(record.destination, lane, `${label}.destination`);
  if (source.family !== destination.family) throw new TypeError(`${label} family roles disagree`);
  if (source.address !== destination.routeAddress || source.runtime !== destination.routeCodeHash) {
    throw new TypeError(`${label} source emitter does not identify the destination route deployment`);
  }
  parseSettlement(record.settlement, `${label}.settlement`);
  return Object.freeze({
    lineage: `${record.route_id}\u0000${record.asset_key}`,
    key: `${lane.source.profile}\u0000${lane.target.profile}\u0000${record.route_id}\u0000${record.asset_key}\u0000${revision}`,
    revision,
    activation,
  });
}

function deepFreezeClone(value) {
  if (Array.isArray(value)) return Object.freeze(value.map(deepFreezeClone));
  if (value !== null && typeof value === "object") {
    const out = Object.fromEntries(
      Object.entries(value).map(([key, entry]) => [key, deepFreezeClone(entry)]),
    );
    return Object.freeze(out);
  }
  return value;
}

function parseRouteKey(value, label) {
  const record = exactFields(
    value,
    new Set(["lane_id", "route_id", "asset_key", "revision"]),
    label,
  );
  const lane = parseLaneId(record.lane_id, `${label}.lane_id`);
  for (const field of ["route_id", "asset_key"]) {
    if (typeof record[field] !== "string" || !ROUTE_KEY.test(record[field])) {
      throw new TypeError(`${label}.${field} must be canonical lowercase route text`);
    }
  }
  return Object.freeze({
    lane,
    routeId: record.route_id,
    assetKey: record.asset_key,
    revision: integer(record.revision, `${label}.revision`, 1, 0xffff_ffff),
  });
}

function canTransitionActivation(current, next) {
  return (
    (current === "staged" && ["bidirectional", "retired"].includes(next)) ||
    (current === "bidirectional" && ["inbound_only", "paused"].includes(next)) ||
    (current === "inbound_only" && ["paused", "retired"].includes(next)) ||
    (current === "paused" && ["bidirectional", "inbound_only", "retired"].includes(next))
  );
}

/** Validate one closed atomic SCCP route-governance action. */
export function normalizeSccpRouteGovernanceAction(value) {
  const record = exactFields(value, new Set(["action", "route"]), "SCCP route action");
  const action = canonicalText(record.action, "SCCP route action.action", 64);
  const payload = plainObject(record.route, "SCCP route action.route");
  if (action === "Register") {
    const registration = exactFields(
      payload,
      new Set(["route", "native_trust_anchor"]),
      "SCCP route action.Register",
    );
    const routeRecord = plainObject(registration.route, "SCCP route action.Register.route");
    const lane = parseLaneId(routeRecord.lane_id, "SCCP route action.Register.route.lane_id");
    const parsed = parseGovernedRoute(routeRecord, lane, "SCCP route action.Register.route");
    if (parsed.activation !== "staged") {
      throw new TypeError("new SCCP routes must be registered in staged state");
    }
    parseNativeTrustAnchor(
      registration.native_trust_anchor,
      lane,
      "SCCP route action.Register.native_trust_anchor",
    );
  } else if (action === "SetActivation") {
    const update = exactFields(
      payload,
      new Set(["key", "expected_current", "next"]),
      "SCCP route action.SetActivation",
    );
    parseRouteKey(update.key, "SCCP route action.SetActivation.key");
    const current = parseActivation(
      update.expected_current,
      "SCCP route action.SetActivation.expected_current",
    );
    const next = parseActivation(update.next, "SCCP route action.SetActivation.next");
    if (!canTransitionActivation(current, next)) {
      throw new TypeError("SCCP activation transition is not legal");
    }
  } else if (action === "SwitchRevision") {
    const update = exactFields(
      payload,
      new Set([
        "previous_key",
        "expected_previous",
        "previous_next",
        "successor_key",
        "successor_next",
      ]),
      "SCCP route action.SwitchRevision",
    );
    const previous = parseRouteKey(
      update.previous_key,
      "SCCP route action.SwitchRevision.previous_key",
    );
    const successor = parseRouteKey(
      update.successor_key,
      "SCCP route action.SwitchRevision.successor_key",
    );
    const expected = parseActivation(
      update.expected_previous,
      "SCCP route action.SwitchRevision.expected_previous",
    );
    const previousNext = parseActivation(
      update.previous_next,
      "SCCP route action.SwitchRevision.previous_next",
    );
    const successorNext = parseActivation(
      update.successor_next,
      "SCCP route action.SwitchRevision.successor_next",
    );
    if (
      !sameLane(previous.lane, successor.lane) ||
      previous.routeId !== successor.routeId ||
      previous.assetKey !== successor.assetKey ||
      successor.revision !== previous.revision + 1 ||
      !canTransitionActivation(expected, previousNext) ||
      !["inbound_only", "paused"].includes(previousNext) ||
      successorNext !== "bidirectional"
    ) {
      throw new TypeError("SCCP revision switch is not a legal atomic cutover");
    }
  } else if (action === "InitializeTrustAnchor") {
    const update = exactFields(
      payload,
      new Set(["lane_id", "expected_current", "initial"]),
      "SCCP route action.InitializeTrustAnchor",
    );
    const lane = parseLaneId(update.lane_id, "SCCP route action.InitializeTrustAnchor.lane_id");
    if (update.expected_current !== null) {
      throw new TypeError("SCCP initial trust-anchor compare-and-swap must expect null");
    }
    if (
      parseNativeTrustAnchor(
        update.initial,
        lane,
        "SCCP route action.InitializeTrustAnchor.initial",
      ) === null
    ) {
      throw new TypeError("SCCP initial trust anchor is required");
    }
  } else if (action === "AdvanceTrustAnchor") {
    const update = exactFields(
      payload,
      new Set(["lane_id", "expected_current", "next"]),
      "SCCP route action.AdvanceTrustAnchor",
    );
    const lane = parseLaneId(update.lane_id, "SCCP route action.AdvanceTrustAnchor.lane_id");
    const current = parseNativeTrustAnchor(
      update.expected_current,
      lane,
      "SCCP route action.AdvanceTrustAnchor.expected_current",
    );
    const next = parseNativeTrustAnchor(
      update.next,
      lane,
      "SCCP route action.AdvanceTrustAnchor.next",
    );
    if (
      current === null ||
      next === null ||
      current.backend.backend !== next.backend.backend ||
      current.anchor_hash === next.anchor_hash ||
      next.checkpoint_height <= current.checkpoint_height
    ) {
      throw new TypeError("SCCP trust anchor must advance monotonically within one backend");
    }
  } else if (action === "Remove") {
    parseRouteKey(payload, "SCCP route action.Remove");
  } else {
    throw new TypeError("SCCP route action is unsupported or retired");
  }
  return deepFreezeClone(record);
}

/** Validate and normalize one closed SCCP V1 codec value. */
export function normalizeSccpCodecValue(codec, value) {
  if (!Object.prototype.hasOwnProperty.call(SCCP_CODEC_KEYS, codec)) {
    throw new TypeError("codec is unsupported or retired");
  }
  if (codec === SCCP_CODEC_CANONICAL_TEXT) {
    const text = canonicalText(value, "canonical_text", 256);
    if (!/^[\x20-\x7e]+$/u.test(text)) {
      throw new TypeError("canonical_text must contain printable ASCII only");
    }
    return new TextEncoder().encode(text);
  }
  const raw = binary(value, SCCP_CODEC_KEYS[codec]);
  if (allZero(raw)) throw new TypeError(`${SCCP_CODEC_KEYS[codec]} must be nonzero`);
  if (codec === SCCP_CODEC_EVM_ADDRESS20 && raw.length !== 20) {
    throw new TypeError("evm_address20 must contain exactly 20 bytes");
  }
  if (
    codec === SCCP_CODEC_TRON_ADDRESS21 &&
    (raw.length !== 21 || raw[0] !== 0x41 || allZero(raw.subarray(1)))
  ) {
    throw new TypeError("tron_address21 must contain 0x41 and a nonzero 20-byte address");
  }
  return raw;
}

/** Compute the exact contract-side native source-event digest. */
export function sccpSourceEventDigest(laneHash, messageId, payloadHash) {
  const labels = ["laneHash", "messageId", "payloadHash"];
  const roles = [laneHash, messageId, payloadHash].map((value, index) => {
    if (typeof value === "string") {
      exactLowerHex(value, labels[index], 32);
      return Uint8Array.from(Buffer.from(value, "hex"));
    }
    const normalized = binary(value, labels[index]);
    if (normalized.length !== 32 || allZero(normalized)) {
      throw new TypeError(`${labels[index]} must be a nonzero 32-byte hash`);
    }
    return normalized;
  });
  if (new Set(roles.map(lowerHexBytes)).size !== roles.length) {
    throw new TypeError("SCCP lane, message, and payload hash roles must be distinct");
  }
  const preimage = new Uint8Array(SOURCE_EVENT_PREFIX.length + 1 + 96);
  preimage.set(SOURCE_EVENT_PREFIX);
  preimage[SOURCE_EVENT_PREFIX.length] = 1;
  roles.forEach((role, index) =>
    preimage.set(role, SOURCE_EVENT_PREFIX.length + 1 + index * 32),
  );
  return lowerHexBytes(keccak_256(preimage));
}

/** Normalize the closed SCCP endpoint capability snapshot. */
export function normalizeSccpCapabilities(value) {
  const allowed = new Set([
    "version",
    "registry_revision",
    "registry_path",
    "message_bundle_path",
    "proof_request_path",
    "recent_messages_path",
    "proof_submit_path",
    "native_message_submit_path",
  ]);
  const required = new Set([
    "version",
    "registry_revision",
    "registry_path",
    "message_bundle_path",
    "proof_request_path",
    "recent_messages_path",
  ]);
  const record = exactFields(value, allowed, "SCCP capabilities", required);
  return Object.freeze({
    version: integer(record.version, "SCCP capabilities.version", 1, 1),
    registry_revision: exactLowerHex(
      record.registry_revision,
      "SCCP capabilities.registry_revision",
      32,
      { prefix: true },
    ),
    registry_path: exactCapabilityPath(record.registry_path, "registry_path"),
    message_bundle_path: exactCapabilityPath(
      record.message_bundle_path,
      "message_bundle_path",
    ),
    proof_request_path: exactCapabilityPath(
      record.proof_request_path,
      "proof_request_path",
    ),
    recent_messages_path: exactCapabilityPath(
      record.recent_messages_path,
      "recent_messages_path",
    ),
    proof_submit_path: exactCapabilityPath(
      record.proof_submit_path,
      "proof_submit_path",
      true,
    ),
    native_message_submit_path: exactCapabilityPath(
      record.native_message_submit_path,
      "native_message_submit_path",
      true,
    ),
  });
}

/** Validate the authoritative typed SCCP registry without reinterpreting it as a manifest. */
export function normalizeSccpRegistry(value) {
  const record = exactFields(value, new Set(["version", "lanes"]), "SCCP registry");
  integer(record.version, "SCCP registry.version", 1, 1);
  const lanes = array(record.lanes, "SCCP registry.lanes");
  if (lanes.length > 16) throw new TypeError("SCCP registry contains more than 16 lanes");
  const laneKeys = new Set();
  const routeKeys = new Set();
  let routeCount = 0;
  lanes.forEach((entry, laneIndex) => {
    const label = `SCCP registry.lanes[${laneIndex}]`;
    const laneRecord = exactFields(
      entry,
      new Set(["lane_id", "native_trust_anchor", "routes"]),
      label,
    );
    const lane = parseLaneId(laneRecord.lane_id, `${label}.lane_id`);
    const laneKey = `${lane.source.profile}\u0000${lane.target.profile}`;
    if (laneKeys.has(laneKey)) throw new TypeError("SCCP registry contains a duplicate lane");
    laneKeys.add(laneKey);
    parseNativeTrustAnchor(laneRecord.native_trust_anchor, lane, `${label}.native_trust_anchor`);
    const routes = array(laneRecord.routes, `${label}.routes`);
    if (routes.length < 1 || routes.length > 8) {
      throw new TypeError(`${label}.routes must contain 1..8 routes`);
    }
    routeCount += routes.length;
    const lineages = new Map();
    routes.forEach((route, routeIndex) => {
      const parsed = parseGovernedRoute(route, lane, `${label}.routes[${routeIndex}]`);
      if (routeKeys.has(parsed.key)) throw new TypeError("SCCP registry contains a duplicate route");
      routeKeys.add(parsed.key);
      const lineage = lineages.get(parsed.lineage) ?? [];
      lineage.push(parsed);
      lineages.set(parsed.lineage, lineage);
    });
    for (const lineage of lineages.values()) {
      lineage.sort((left, right) => left.revision - right.revision);
      lineage.forEach((route, index) => {
        if (route.revision !== index + 1) {
          throw new TypeError("SCCP route revisions must start at one and contain no gaps");
        }
      });
      if (lineage.filter(({ activation }) => activation === "bidirectional").length > 1) {
        throw new TypeError("SCCP registry enables multiple revisions of one route");
      }
    }
  });
  if (routeCount > 64) throw new TypeError("SCCP registry contains more than 64 routes");
  return deepFreezeClone(record);
}

/** Normalize newest-first SCCP discovery with only bundle and proof-request links. */
export function normalizeSccpRecentMessages(value) {
  const root = exactFields(value, new Set(["items"]), "SCCP recent messages");
  const items = array(root.items, "SCCP recent messages.items").map((entry, index) => {
    const label = `SCCP recent messages.items[${index}]`;
    const allowed = new Set([
      "height",
      "message_id_hex",
      "kind",
      "source_profile",
      "target_profile",
      "destination_binding_hash",
      "route_configuration_hash",
      "target_domain",
      "asset_id",
      "route_id",
      "recipient",
      "amount",
      "payload_projection",
      "links",
    ]);
    const required = new Set([
      "height",
      "message_id_hex",
      "kind",
      "source_profile",
      "target_profile",
      "destination_binding_hash",
      "route_configuration_hash",
      "target_domain",
      "amount",
      "links",
    ]);
    const record = exactFields(entry, allowed, label, required);
    const source = profile(record.source_profile, `${label}.source_profile`);
    const target = profile(record.target_profile, `${label}.target_profile`);
    if (source.profile !== "sora-taira" || target.sora) {
      throw new TypeError(`${label} must describe a Taira-origin external transfer`);
    }
    if (record.kind !== "transfer") throw new TypeError(`${label}.kind must be transfer`);
    const messageId = exactLowerHex(record.message_id_hex, `${label}.message_id_hex`, 32);
    const links = exactFields(
      record.links,
      new Set(["bundle_path", "proof_request_path"]),
      `${label}.links`,
    );
    const expectedBundle = `/v1/sccp/proofs/message/${messageId}`;
    const expectedRequest = `/v1/sccp/proof-requests/${messageId}`;
    if (
      canonicalPath(links.bundle_path, `${label}.links.bundle_path`) !== expectedBundle ||
      canonicalPath(links.proof_request_path, `${label}.links.proof_request_path`) !==
        expectedRequest
    ) {
      throw new TypeError(`${label}.links do not identify this exact message`);
    }
    if (integer(record.target_domain, `${label}.target_domain`, 1, 5) !== target.domain) {
      throw new TypeError(`${label} profile and domain fields disagree`);
    }
    const optionalText = (field) =>
      record[field] === null || record[field] === undefined
        ? null
        : canonicalText(record[field], `${label}.${field}`, 4096);
    const amount = canonicalText(record.amount, `${label}.amount`, 4096);
    if (!/^[1-9][0-9]*$/u.test(amount)) {
      throw new TypeError(`${label}.amount must be a positive canonical decimal string`);
    }
    const destinationBindingHash = exactLowerHex(
      record.destination_binding_hash,
      `${label}.destination_binding_hash`,
      32,
      { prefix: true },
    );
    const routeConfigurationHash = exactLowerHex(
      record.route_configuration_hash,
      `${label}.route_configuration_hash`,
      32,
      { prefix: true },
    );
    if (destinationBindingHash === routeConfigurationHash) {
      throw new TypeError(`${label} binding and route-configuration hashes must be distinct`);
    }
    return Object.freeze({
      height: integer(record.height, `${label}.height`, 1),
      message_id_hex: messageId,
      kind: "transfer",
      source_profile: source.profile,
      target_profile: target.profile,
      destination_binding_hash: destinationBindingHash,
      route_configuration_hash: routeConfigurationHash,
      target_domain: target.domain,
      asset_id: optionalText("asset_id"),
      route_id: optionalText("route_id"),
      recipient: optionalText("recipient"),
      amount,
      payload_projection:
        record.payload_projection === null || record.payload_projection === undefined
          ? null
          : deepFreezeClone(plainObject(record.payload_projection, `${label}.payload_projection`)),
      links: Object.freeze({
        bundle_path: expectedBundle,
        proof_request_path: expectedRequest,
      }),
    });
  });
  for (let index = 1; index < items.length; index += 1) {
    if (items[index - 1].height < items[index].height) {
      throw new TypeError("SCCP recent messages must be newest-first");
    }
  }
  return Object.freeze({ items: Object.freeze(items) });
}

/** Normalize a raw JSON `NexusSccpMessageProofV1` bundle. */
export function normalizeSccpMessageBundle(value) {
  const record = exactFields(
    value,
    new Set([
      "version",
      "commitment_root",
      "commitment",
      "merkle_proof",
      "payload",
      "finality_proof",
    ]),
    "SCCP message bundle",
  );
  integer(record.version, "SCCP message bundle.version", 1, 1);
  exactLowerHex(record.commitment_root, "SCCP message bundle.commitment_root", 32, {
    prefix: true,
  });
  plainObject(record.commitment, "SCCP message bundle.commitment");
  plainObject(record.merkle_proof, "SCCP message bundle.merkle_proof");
  const payload = exactFields(
    record.payload,
    new Set(["Transfer"]),
    "SCCP message bundle.payload",
  );
  plainObject(payload.Transfer, "SCCP message bundle.payload.Transfer");
  exactVariableHex(record.finality_proof, "SCCP message bundle.finality_proof");
  return deepFreezeClone(record);
}

function parsePublicInputs(value, label) {
  const record = exactFields(
    value,
    new Set([
      "version",
      "message_id",
      "payload_hash",
      "target_domain",
      "commitment_root",
      "finality_height",
      "finality_block_hash",
    ]),
    label,
  );
  integer(record.version, `${label}.version`, 1, 1);
  for (const field of ["message_id", "payload_hash", "commitment_root", "finality_block_hash"]) {
    exactLowerHex(record[field], `${label}.${field}`, 32, { prefix: true });
  }
  integer(record.target_domain, `${label}.target_domain`, 1, 5);
  if (typeof record.finality_height !== "string" || !/^[1-9][0-9]*$/u.test(record.finality_height)) {
    throw new TypeError(`${label}.finality_height must be a positive canonical u64 string`);
  }
  if (BigInt(record.finality_height) > 0xffff_ffff_ffff_ffffn) {
    throw new TypeError(`${label}.finality_height exceeds u64`);
  }
  return record;
}

/** Normalize a query-free raw JSON `SccpGroth16Bn254ProofRequestV1`. */
export function normalizeSccpProofRequest(value) {
  const fields = new Set([
    "version",
    "backend",
    "source_network",
    "target_network",
    "public_inputs",
    "verifying_key",
    "verifier_key_hash",
    "bundle_bytes",
    "statement_hash",
    "destination_binding_hash",
    "route_configuration_hash",
    "request_hash",
  ]);
  const record = exactFields(value, fields, "SCCP proof request");
  integer(record.version, "SCCP proof request.version", 1, 1);
  const backend = parseUnitBackend(
    record.backend,
    "SCCP proof request.backend",
    "family",
    DESTINATION_BACKENDS,
  );
  const source = parseNetwork(record.source_network, "SCCP proof request.source_network");
  const target = parseNetwork(record.target_network, "SCCP proof request.target_network");
  if (source.profile !== "sora-taira" || target.sora) {
    throw new TypeError("SCCP proof request must describe an exact Taira-to-external lane");
  }
  if (DESTINATION_BACKENDS[backend] !== emitterFamily(target)) {
    throw new TypeError("SCCP proof request backend does not match target network");
  }
  const inputs = parsePublicInputs(record.public_inputs, "SCCP proof request.public_inputs");
  if (inputs.target_domain !== target.domain) {
    throw new TypeError("SCCP proof request target domain does not match target network");
  }
  const keyBytes = parseVerifyingKey(record.verifying_key, "SCCP proof request.verifying_key");
  const hashes = [
    "verifier_key_hash",
    "statement_hash",
    "destination_binding_hash",
    "route_configuration_hash",
    "request_hash",
  ];
  for (const field of hashes) {
    exactLowerHex(record[field], `SCCP proof request.${field}`, 32, { prefix: true });
  }
  if (`0x${lowerHexBytes(keccak_256(keyBytes))}` !== record.verifier_key_hash) {
    throw new TypeError("SCCP proof request verifier_key_hash does not match verifying_key");
  }
  if (new Set(hashes.map((field) => record[field])).size !== hashes.length) {
    throw new TypeError("SCCP proof request reuses role-separated commitments");
  }
  exactVariableHex(record.bundle_bytes, "SCCP proof request.bundle_bytes");
  return deepFreezeClone(record);
}

function validateAuthority(value, label) {
  const authority = canonicalText(value, label, 512);
  AccountAddress.fromAccountId(authority);
  return authority;
}

function optionalSignature(record, label) {
  if (record.signature_b64 === undefined) return {};
  canonicalBase64(record.signature_b64, `${label}.signature_b64`, { maximumBytes: 4096 });
  return { signature_b64: record.signature_b64 };
}

/** Build the sole supported SORA-origin destination-proof submission. */
export function normalizeBridgeProofSubmitPayload(value) {
  const record = exactFields(
    value,
    new Set(["authority", "signature_b64", "destination_proof_b64", "creation_time_ms"]),
    "bridge proof submit",
    new Set(["authority", "destination_proof_b64"]),
  );
  canonicalBase64(record.destination_proof_b64, "bridge proof submit.destination_proof_b64");
  const result = {
    authority: validateAuthority(record.authority, "bridge proof submit.authority"),
    ...optionalSignature(record, "bridge proof submit"),
    destination_proof_b64: record.destination_proof_b64,
  };
  if (record.creation_time_ms !== undefined) {
    result.creation_time_ms = integer(
      record.creation_time_ms,
      "bridge proof submit.creation_time_ms",
      1,
    );
  }
  return Object.freeze(result);
}

/** Build the sole supported native inbound message submission. */
export function normalizeBridgeMessageSubmitPayload(value) {
  const record = exactFields(
    value,
    new Set(["authority", "signature_b64", "native_proof_b64", "creation_time_ms"]),
    "bridge message submit",
    new Set(["authority", "native_proof_b64"]),
  );
  canonicalBase64(record.native_proof_b64, "bridge message submit.native_proof_b64");
  const result = {
    authority: validateAuthority(record.authority, "bridge message submit.authority"),
    ...optionalSignature(record, "bridge message submit"),
    native_proof_b64: record.native_proof_b64,
  };
  if (record.creation_time_ms !== undefined) {
    result.creation_time_ms = integer(
      record.creation_time_ms,
      "bridge message submit.creation_time_ms",
      1,
    );
  }
  return Object.freeze(result);
}

function irohaPrehash(value) {
  const digest = Uint8Array.from(blake2b256(value));
  digest[digest.length - 1] |= 1;
  return digest;
}

/** Validate the unified exact prepared-or-submitted bridge response. */
export function normalizeSccpBridgeSubmitResponse(value, expectations = {}) {
  const record = exactFields(value, BRIDGE_RESPONSE_FIELDS, "bridge submit response");
  const submitted = boolean(record.submitted, "bridge submit response.submitted");
  if (record.payload_kind !== "transfer") {
    throw new TypeError("bridge submit response.payload_kind must be transfer");
  }
  const counterparty = profile(
    record.counterparty_chain,
    "bridge submit response.counterparty_chain",
  );
  const domain = integer(
    record.counterparty_domain,
    "bridge submit response.counterparty_domain",
    1,
    5,
  );
  if (counterparty.sora || counterparty.domain !== domain) {
    throw new TypeError("bridge submit response counterparty profile/domain disagree");
  }
  const backend = canonicalText(record.backend, "bridge submit response.backend", 128);
  if (!/^bridge\/[a-z0-9/_-]+$/u.test(backend)) {
    throw new TypeError("bridge submit response.backend is not canonical");
  }
  const rangeStart = integer(record.range_start_height, "range_start_height", 1);
  const rangeEnd = integer(record.range_end_height, "range_end_height", rangeStart);
  const creationTime = integer(record.creation_time_ms, "creation_time_ms", 1);
  const txHash =
    record.tx_hash_hex === null
      ? null
      : exactLowerHex(record.tx_hash_hex, "tx_hash_hex", 32);
  const transaction =
    record.transaction_payload_b64 === null
      ? null
      : canonicalBase64(record.transaction_payload_b64, "transaction_payload_b64");
  const signing =
    record.signing_message_b64 === null
      ? null
      : canonicalBase64(record.signing_message_b64, "signing_message_b64", {
          maximumBytes: 32,
        });
  if (signing !== null && signing.length !== 32) {
    throw new TypeError("signing_message_b64 must contain exactly 32 bytes");
  }
  if (submitted) {
    if (txHash === null || transaction !== null || signing !== null) {
      throw new TypeError("submitted response must contain only tx_hash_hex signing state");
    }
  } else {
    if (txHash !== null || transaction === null || signing === null) {
      throw new TypeError("prepared response requires transaction payload and signing message");
    }
    if (lowerHexBytes(irohaPrehash(transaction)) !== lowerHexBytes(signing)) {
      throw new TypeError("signing_message_b64 is not the transaction-payload prehash");
    }
  }
  const result = Object.freeze({
    submitted,
    payload_kind: "transfer",
    message_id_hex: exactLowerHex(record.message_id_hex, "message_id_hex", 32),
    backend,
    counterparty_domain: domain,
    counterparty_chain: counterparty.profile,
    manifest_hash_hex: exactLowerHex(record.manifest_hash_hex, "manifest_hash_hex", 32),
    range_start_height: rangeStart,
    range_end_height: rangeEnd,
    creation_time_ms: creationTime,
    tx_hash_hex: txHash,
    transaction_payload_b64: record.transaction_payload_b64,
    signing_message_b64: record.signing_message_b64,
  });
  const expected = exactFields(
    expectations,
    new Set(["creation_time_ms"]),
    "bridge response expectations",
    new Set(),
  );
  if (
    expected.creation_time_ms !== undefined &&
    result.creation_time_ms !== expected.creation_time_ms
  ) {
    throw new TypeError("bridge submit response.creation_time_ms does not match the request");
  }
  return result;
}

/** Parse strict UTF-8 bridge response JSON while rejecting duplicate keys. */
export function parseSccpBridgeSubmitResponseJson(text, expectations = {}) {
  return normalizeSccpBridgeSubmitResponse(
    parseSccpJsonObject(text, "bridge submit response"),
    expectations,
  );
}

/** Parse one strict SCCP JSON object, rejecting duplicate keys and trailing input. */
export function parseSccpJsonObject(text, label = "SCCP response") {
  if (typeof text !== "string" || text.length === 0) {
    throw new TypeError(`${label} must be nonempty UTF-8 JSON text`);
  }
  const encoded = new TextEncoder().encode(text);
  if (new TextDecoder("utf-8", { fatal: true }).decode(encoded) !== text) {
    throw new TypeError(`${label} must be canonical UTF-8 JSON text`);
  }
  assertNoDuplicateJsonObjectKeys(text, label);
  let value;
  try {
    value = JSON.parse(text);
  } catch (error) {
    throw new TypeError(`${label} must be valid JSON`, { cause: error });
  }
  return plainObject(value, label);
}

function assertNoDuplicateJsonObjectKeys(source, label) {
  let cursor = 0;
  const whitespace = () => {
    while (cursor < source.length && /[\x20\t\r\n]/u.test(source[cursor])) cursor += 1;
  };
  const string = () => {
    if (source[cursor] !== '"') throw new TypeError(`${label} contains invalid JSON`);
    const start = cursor;
    cursor += 1;
    while (cursor < source.length) {
      const character = source[cursor];
      if (character === "\\") cursor += 2;
      else if (character === '"') {
        cursor += 1;
        return JSON.parse(source.slice(start, cursor));
      } else cursor += 1;
    }
    throw new TypeError(`${label} contains an unterminated JSON string`);
  };
  const value = () => {
    whitespace();
    if (source[cursor] === "{") return object();
    if (source[cursor] === "[") return list();
    if (source[cursor] === '"') {
      string();
      return;
    }
    const match = /^(?:-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?|true|false|null)/u.exec(
      source.slice(cursor),
    );
    if (!match) throw new TypeError(`${label} contains invalid JSON`);
    cursor += match[0].length;
  };
  const object = () => {
    cursor += 1;
    whitespace();
    const keys = new Set();
    if (source[cursor] === "}") {
      cursor += 1;
      return;
    }
    for (;;) {
      whitespace();
      const key = string();
      if (keys.has(key)) throw new TypeError(`${label} contains duplicate field \`${key}\``);
      keys.add(key);
      whitespace();
      if (source[cursor] !== ":") throw new TypeError(`${label} contains invalid JSON`);
      cursor += 1;
      value();
      whitespace();
      if (source[cursor] === "}") {
        cursor += 1;
        return;
      }
      if (source[cursor] !== ",") throw new TypeError(`${label} contains invalid JSON`);
      cursor += 1;
    }
  };
  const list = () => {
    cursor += 1;
    whitespace();
    if (source[cursor] === "]") {
      cursor += 1;
      return;
    }
    for (;;) {
      value();
      whitespace();
      if (source[cursor] === "]") {
        cursor += 1;
        return;
      }
      if (source[cursor] !== ",") throw new TypeError(`${label} contains invalid JSON`);
      cursor += 1;
    }
  };
  value();
  whitespace();
  if (cursor !== source.length) throw new TypeError(`${label} contains trailing JSON data`);
}
