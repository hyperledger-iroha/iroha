import { keccak_256 } from "@noble/hashes/sha3";
import { sha256 } from "@noble/hashes/sha2";

import { AccountAddress } from "./address.js";
import { blake2b256 } from "./blake2b.js";
import { validateNoritoFrame } from "./norito.js";
import { normalizeAssetDefinitionId } from "./normalizers.js";
import { NumericV1 } from "./numericV1.js";

/** First-release SCCP protocol domains. Domain 4 remains retired and reserved. */
export const SCCP_DOMAIN_SORA = 0;
export const SCCP_DOMAIN_ETH = 1;
export const SCCP_DOMAIN_BSC = 2;
export const SCCP_DOMAIN_SOLANA = 3;
export const SCCP_DOMAIN_TRON = 5;

/** Closed first-release SCCP payload codec inventory. */
export const SCCP_CODEC_CANONICAL_TEXT = 1;
export const SCCP_CODEC_EVM_ADDRESS20 = 2;
export const SCCP_CODEC_TRON_ADDRESS21 = 5;
export const SCCP_CODEC_SOLANA_PUBKEY32 = 6;

export const SCCP_CODEC_KEYS = Object.freeze({
  [SCCP_CODEC_CANONICAL_TEXT]: "canonical_text",
  [SCCP_CODEC_EVM_ADDRESS20]: "evm_address20",
  [SCCP_CODEC_TRON_ADDRESS21]: "tron_address21",
  [SCCP_CODEC_SOLANA_PUBKEY32]: "solana_pubkey32",
});

/** SCCP V1 carries only the exact value-moving transfer payload. */
export const SCCP_PAYLOAD_KINDS = Object.freeze(["transfer"]);

const SOURCE_EVENT_PREFIX = new TextEncoder().encode("sccp:source:event:v1");
const LANE_HASH_PREFIX = new TextEncoder().encode("sccp:lane-id:v1");
const LANE_MESSAGE_ID_PREFIX = new TextEncoder().encode("sccp:lane-message-id:v1");
const HUB_LEAF_PREFIX = new TextEncoder().encode("sccp:hub:leaf:v1");
const HUB_NODE_PREFIX = new TextEncoder().encode("sccp:hub:node:v1");
const PAYLOAD_HASH_PREFIX = new TextEncoder().encode("sccp:payload:v1");
const EVM_DESTINATION_BINDING_PREFIX = new TextEncoder().encode(
  "iroha:sccp:evm-destination-binding:v1",
);
const TRON_DESTINATION_BINDING_PREFIX = new TextEncoder().encode(
  "iroha:sccp:tron-destination-binding:v1",
);
const SOLANA_DESTINATION_BINDING_PREFIX = new TextEncoder().encode(
  "iroha:sccp:solana-destination-binding:v1",
);
const SOLANA_NATIVE_VERIFIER_CONFIG_PREFIX = new TextEncoder().encode(
  "sccp:solana:verifier-config:v1",
);
const CONCRETE_ROUTE_CONFIG_PREFIX = new TextEncoder().encode(
  "sccp:concrete-route-config:v1",
);
const EVM_GROTH16_BACKEND = new TextEncoder().encode("evm-groth16-bn254-v1");
const TRON_GROTH16_BACKEND = new TextEncoder().encode("tron-groth16-bn254-v1");
const SOLANA_GROTH16_BACKEND = new TextEncoder().encode("solana-groth16-bn254-v1");
const SOURCE_EMITTER_HASH_PREFIX = new TextEncoder().encode(
  "sccp:source-emitter-identity:v1",
);
const SOURCE_IDENTITY_HASH_PREFIX = new TextEncoder().encode("sccp:source-identity:v1");
const SEMANTIC_PROOF_PROFILE_PREFIX = new TextEncoder().encode(
  "sccp:semantic-proof-profile:v1",
);
const SORA_FINALITY_ANCHOR_PREFIX = new TextEncoder().encode(
  "sccp:sora-finality-anchor:v1",
);
const PUBLIC_SIGNAL_SCHEMA_HASH =
  "7567439F41173D6745A3D51923CB70371ACC7D66F23CEFB4100D6D5D7A432CBB";
const SORA_TAIRA_CHAIN_ID_HASH =
  "CF1CFC0F57B0BFA4C21882A9870317A1F4812F86533897095E3944BE34C5BBA7";
const TAIRA_XOR_ASSET_DEFINITION_ID = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
export const SCCP_SORA_OUTBOUND_EXECUTION_SEMANTICS_V1 =
  "ivm_proved_record_sccp_message_v1";
export const SCCP_MAX_SORA_OUTBOUND_GAS_LIMIT_V1 = 1_000_000_000;
const TAIRA_I105_DISCRIMINANT = 369;
const MAX_WIRE_BYTES = 16 * 1024 * 1024;
const MAX_DESTINATION_ARTIFACT_BYTES = MAX_WIRE_BYTES + 64 * 1024;
const DESTINATION_PROOF_NORITO_TYPE =
  "iroha_sccp::SccpGroth16Bn254ProofArtifactV1";
const NATIVE_MESSAGE_PROOF_NORITO_TYPE =
  "iroha_sccp::native_admission::SccpNativeInboundMessageProofV1";
const MAX_U64 = 0xffff_ffff_ffff_ffffn;
const MAX_U128 = 0xffff_ffff_ffff_ffff_ffff_ffff_ffff_ffffn;
const MAX_U32 = 0xffff_ffff;
const MAX_MERKLE_PROOF_STEPS = 64;
const MAX_FINALITY_PROOF_BYTES = 16 * 1024 * 1024;
export const SCCP_SOLANA_TESTNET_GENESIS_HASH =
  "4uhcVJyU9pJkvQyS88uRDiswHXSCkY3zQawwpjk2NsNY";
// Keep this module browser-native. `instructionBuilders.js` imports SCCP at
// module evaluation time, before an application could install a Node Buffer
// compatibility global, so even an otherwise unrelated browser transfer flow
// must not depend on `Buffer` here.
const SOLANA_TESTNET_GENESIS_BYTES = Uint8Array.from([
  0x3a, 0x13, 0x2e, 0xce, 0x10, 0x30, 0x5e, 0xc1,
  0x83, 0x07, 0x25, 0x50, 0x2f, 0xa2, 0xb7, 0xe7,
  0xeb, 0x81, 0x57, 0xe9, 0x12, 0x3d, 0x4c, 0x1f,
  0x65, 0x4a, 0x71, 0x78, 0x71, 0x61, 0xdc, 0x21,
]);
const SOLANA_CLASSIC_TOKEN_PROGRAM_ID = Uint8Array.from([
  6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70, 206, 235, 121, 172,
  28, 180, 133, 237, 95, 91, 55, 145, 58, 140, 245, 133, 126, 255, 0, 169,
]);
const CLOSED_DOMAINS = new Set([
  SCCP_DOMAIN_SORA,
  SCCP_DOMAIN_ETH,
  SCCP_DOMAIN_BSC,
  SCCP_DOMAIN_SOLANA,
  SCCP_DOMAIN_TRON,
]);
const BN254_BASE_FIELD_MODULUS =
  0x30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47n;
const ROUTE_KEY = /^[a-z0-9](?:[a-z0-9_-]{0,62}[a-z0-9])?$/u;

const NETWORKS = Object.freeze({
  "sora-taira": Object.freeze({ tag: 1, domain: SCCP_DOMAIN_SORA, sora: true }),
  "ethereum-mainnet": Object.freeze({ tag: 2, domain: SCCP_DOMAIN_ETH, sora: false }),
  "ethereum-sepolia": Object.freeze({ tag: 3, domain: SCCP_DOMAIN_ETH, sora: false }),
  "bsc-mainnet": Object.freeze({ tag: 4, domain: SCCP_DOMAIN_BSC, sora: false }),
  "bsc-testnet": Object.freeze({ tag: 5, domain: SCCP_DOMAIN_BSC, sora: false }),
  "tron-mainnet": Object.freeze({ tag: 10, domain: SCCP_DOMAIN_TRON, sora: false }),
  "tron-nile": Object.freeze({ tag: 11, domain: SCCP_DOMAIN_TRON, sora: false }),
  "tron-shasta": Object.freeze({ tag: 12, domain: SCCP_DOMAIN_TRON, sora: false }),
  "solana-testnet": Object.freeze({
    tag: 13,
    domain: SCCP_DOMAIN_SOLANA,
    sora: false,
    genesisHash: SCCP_SOLANA_TESTNET_GENESIS_HASH,
  }),
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
  solana_agave_v1: new Set(["solana-testnet"]),
});

const DESTINATION_BACKENDS = Object.freeze({
  evm_groth16_bn254_v1: "evm",
  tron_groth16_bn254_v1: "tron",
  solana_groth16_bn254_v1: "solana",
});

const INBOUND_ACTIVATABLE_PROFILES = new Set([
  "ethereum-mainnet",
  "bsc-mainnet",
  "tron-mainnet",
  "solana-testnet",
]);

const CAPABILITY_PATHS = Object.freeze({
  registry_path: "/v1/sccp/registry",
  message_bundle_path: "/v1/sccp/proofs/message/{message_id}",
  proof_request_path: "/v1/sccp/proof-requests/{message_id}",
  recent_messages_path: "/v1/sccp/messages/recent",
  sora_outbound_material_path:
    "/v1/sccp/routes/{source_profile}/{route_id}/{asset_key}/{revision}/sora-outbound-material",
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
  "route_configuration_hash_hex",
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
  for (const key of Reflect.ownKeys(value)) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (
      typeof key !== "string" ||
      descriptor === undefined ||
      descriptor.get !== undefined ||
      descriptor.set !== undefined ||
      !descriptor.enumerable
    ) {
      throw new TypeError(`${label} must contain only enumerable string-keyed data fields`);
    }
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
    throw new TypeError(`${label} must be a safe integer in ${minimum}..${maximum}`);
  }
  return value;
}

function protocolDomain(value, label) {
  const domain = integer(value, label, SCCP_DOMAIN_SORA, SCCP_DOMAIN_TRON);
  if (!CLOSED_DOMAINS.has(domain)) {
    throw new TypeError(`${label} is an unsupported or reserved SCCP domain`);
  }
  return domain;
}

function canonicalUnsignedDecimal(value, label, maximum, { positive = false } = {}) {
  const pattern = positive ? /^[1-9][0-9]*$/u : /^(?:0|[1-9][0-9]*)$/u;
  const maximumText = maximum.toString();
  if (
    typeof value !== "string" ||
    value.length > maximumText.length ||
    !pattern.test(value) ||
    (value.length === maximumText.length && value > maximumText)
  ) {
    throw new TypeError(
      `${label} must be a canonical ${positive ? "positive " : ""}u${maximum === MAX_U64 ? 64 : 128} decimal string`,
    );
  }
  return value;
}

function boolean(value, label) {
  if (typeof value !== "boolean") throw new TypeError(`${label} must be boolean`);
  return value;
}

function array(value, label) {
  if (!Array.isArray(value)) throw new TypeError(`${label} must be an array`);
  for (let index = 0; index < value.length; index += 1) {
    const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
    if (
      descriptor === undefined ||
      descriptor.get !== undefined ||
      descriptor.set !== undefined ||
      !descriptor.enumerable
    ) {
      throw new TypeError(`${label} must be a dense data-only array`);
    }
  }
  for (const key of Reflect.ownKeys(value)) {
    if (key === "length") continue;
    if (
      typeof key !== "string" ||
      !/^(?:0|[1-9][0-9]*)$/u.test(key) ||
      Number(key) >= value.length
    ) {
      throw new TypeError(`${label} must not contain non-index properties`);
    }
  }
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

function bytesFromUpperHex(value, label, byteLength) {
  return Uint8Array.from(Buffer.from(exactUpperHex(value, label, byteLength), "hex"));
}

function concatenateBytes(...values) {
  const result = new Uint8Array(values.reduce((total, value) => total + value.length, 0));
  let offset = 0;
  for (const value of values) {
    result.set(value, offset);
    offset += value.length;
  }
  return result;
}

function unsignedLittleEndian(value, width, label) {
  integer(value, label, 0);
  return unsignedBigIntLittleEndian(BigInt(value), width, label);
}

function unsignedBigIntLittleEndian(value, width, label) {
  if (typeof value !== "bigint" || value < 0n) {
    throw new TypeError(`${label} must be an unsigned integer`);
  }
  let remaining = value;
  const result = new Uint8Array(width);
  for (let index = 0; index < width; index += 1) {
    result[index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  if (remaining !== 0n) throw new TypeError(`${label} exceeds u${width * 8}`);
  return result;
}

function lengthPrefixedBytes(value, label) {
  const bytes = binary(value, label);
  if (bytes.length > MAX_U32) {
    throw new TypeError(`${label} exceeds the SCCP V1 u32 length bound`);
  }
  return concatenateBytes(
    unsignedLittleEndian(bytes.length, 4, `${label} byte length`),
    bytes,
  );
}

function prefixedBlake2b(prefix, payload) {
  return Uint8Array.from(blake2b256(concatenateBytes(prefix, payload)));
}

function prefixedKeccak(prefix, payload) {
  return Uint8Array.from(keccak_256(concatenateBytes(prefix, payload)));
}

function prefixedLowerHex(value) {
  return `0x${lowerHexBytes(value)}`;
}

function abiWordUnsigned(value, label) {
  integer(value, label, 0);
  let remaining = BigInt(value);
  const result = new Uint8Array(32);
  for (let index = result.length - 1; index >= 0 && remaining !== 0n; index -= 1) {
    result[index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  if (remaining !== 0n) throw new TypeError(`${label} exceeds one ABI word`);
  return result;
}

function abiWordAddress20(value, label, { tron = false } = {}) {
  const address = bytesFromUpperHex(value, label, 20);
  const result = new Uint8Array(32);
  if (tron) result[11] = 0x41;
  result.set(address, 12);
  return result;
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
    (value.length - 2) / 2 > maximumBytes ||
    !/^0x(?:[0-9a-f]{2})+$/u.test(value)
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

function canonicalNoritoBase64(value, label, typeName, maximumBytes) {
  const decoded = canonicalBase64(value, label, { maximumBytes });
  validateNoritoFrame(decoded, {
    context: label,
    expectedTypeName: typeName,
    expectedPaddingLength: 0,
    requireNonEmptyPayload: true,
  });
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

function parseDirectedLane(value, label) {
  const record = exactFields(value, new Set(["source", "target"]), label);
  const source = parseNetwork(record.source, `${label}.source`);
  const target = parseNetwork(record.target, `${label}.target`);
  if (source.sora === target.sora || source.domain === target.domain) {
    throw new TypeError(`${label} must join exactly one Taira and one external profile`);
  }
  return Object.freeze({ source, target });
}

function parseLaneId(value, label) {
  const { source, target } = parseDirectedLane(value, label);
  if (source.sora || target.profile !== "sora-taira" || source.domain === target.domain) {
    throw new TypeError(`${label} must be an exact supported external-to-Taira lane`);
  }
  return Object.freeze({ source, target });
}

function parseOutboundLane(value, label) {
  const { source, target } = parseDirectedLane(value, label);
  if (source.profile !== "sora-taira" || target.sora || source.domain === target.domain) {
    throw new TypeError(`${label} must be an exact supported Taira-to-external lane`);
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
  if (network.profile.startsWith("tron-")) return "tron";
  if (network.profile.startsWith("solana-")) return "solana";
  return "evm";
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

function parseInboundFinalityCutoff(value, activation, label) {
  if (value === null) {
    if (activation === "retired") {
      throw new TypeError(`${label} is required for a retired SCCP route`);
    }
    return null;
  }
  if (activation !== "retired") {
    throw new TypeError(`${label} is allowed only for a retired SCCP route`);
  }
  const record = exactFields(
    value,
    new Set(["trust_anchor_hash", "max_anchor_interval_height"]),
    label,
  );
  return Object.freeze({
    trustAnchorHash: exactUpperHex(record.trust_anchor_hash, `${label}.trust_anchor_hash`, 32),
    maxAnchorIntervalHeight: integer(
      record.max_anchor_interval_height,
      `${label}.max_anchor_interval_height`,
      1,
    ),
  });
}

function parseEmitter(value, lane, label) {
  const record = exactFields(value, new Set(["emitter", "identity"]), label);
  const family = canonicalText(record.emitter, `${label}.emitter`, 16);
  if (family !== emitterFamily(lane.source)) {
    throw new TypeError(`${label}.emitter does not match the lane source`);
  }
  if (family === "solana") {
    const identity = exactFields(
      record.identity,
      new Set([
        "program_id",
        "program_data_address",
        "program_data_slot",
        "state_account",
        "program_code_hash",
        "route_config_hash",
      ]),
      `${label}.identity`,
    );
    const programId = exactUpperHex(
      identity.program_id,
      `${label}.identity.program_id`,
      32,
    );
    const programDataAddress = exactUpperHex(
      identity.program_data_address,
      `${label}.identity.program_data_address`,
      32,
    );
    const programDataSlot = integer(
      identity.program_data_slot,
      `${label}.identity.program_data_slot`,
      1,
    );
    const stateAccount = exactUpperHex(
      identity.state_account,
      `${label}.identity.state_account`,
      32,
    );
    const programCodeHash = exactUpperHex(
      identity.program_code_hash,
      `${label}.identity.program_code_hash`,
      32,
    );
    const routeConfigHash = exactUpperHex(
      identity.route_config_hash,
      `${label}.identity.route_config_hash`,
      32,
    );
    const roles = [
      programId,
      programDataAddress,
      stateAccount,
      programCodeHash,
      routeConfigHash,
    ];
    if (new Set(roles).size !== roles.length) {
      throw new TypeError(`${label} reuses a Solana source deployment role`);
    }
    const canonicalBytes = concatenateBytes(
      Uint8Array.of(1, 2),
      ...roles.slice(0, 2).map((role) => Uint8Array.from(Buffer.from(role, "hex"))),
      unsignedLittleEndian(programDataSlot, 8, `${label}.identity.program_data_slot`),
      ...roles.slice(2).map((role) => Uint8Array.from(Buffer.from(role, "hex"))),
    );
    return Object.freeze({
      family,
      address: programId,
      runtime: programCodeHash,
      configuration: routeConfigHash,
      programDataAddress,
      programDataSlot,
      stateAccount,
      canonicalBytes,
    });
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
  const canonicalBytes = concatenateBytes(
    Uint8Array.of(1, family === "tron" ? 1 : 0),
    Uint8Array.from(Buffer.from(address, "hex")),
    Uint8Array.from(Buffer.from(runtime, "hex")),
    Uint8Array.from(Buffer.from(configuration, "hex")),
  );
  return Object.freeze({ family, address, runtime, configuration, canonicalBytes });
}

function parseSourceIdentity(value, lane, label) {
  const record = exactFields(value, new Set(["lane", "emitter"]), label);
  const identityLane = parseLaneId(record.lane, `${label}.lane`);
  if (!sameLane(identityLane, lane)) throw new TypeError(`${label}.lane does not match the route`);
  const emitter = parseEmitter(record.emitter, lane, `${label}.emitter`);
  const laneBytes = canonicalLaneBytes(identityLane);
  const identityBytes = concatenateBytes(
    Uint8Array.of(1),
    lengthPrefixedBytes(laneBytes, `${label}.lane`),
    lengthPrefixedBytes(emitter.canonicalBytes, `${label}.emitter`),
  );
  return Object.freeze({
    ...emitter,
    emitterIdentityHash: prefixedLowerHex(
      prefixedBlake2b(SOURCE_EMITTER_HASH_PREFIX, emitter.canonicalBytes),
    ),
    sourceIdentityHash: prefixedLowerHex(
      prefixedBlake2b(SOURCE_IDENTITY_HASH_PREFIX, identityBytes),
    ),
  });
}

function parseG1(value, label) {
  const record = exactFields(value, new Set(["x", "y"]), label);
  const coordinates = [
    exactUpperHex(record.x, `${label}.x`, 32, { nonzero: false }),
    exactUpperHex(record.y, `${label}.y`, 32, { nonzero: false }),
  ];
  if (coordinates.every((coordinate) => /^0+$/u.test(coordinate))) {
    throw new TypeError(`${label} must not be the BN254 point at infinity`);
  }
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
  const coordinates = fields.map((field) => {
    const coordinate = exactUpperHex(record[field], `${label}.${field}`, 32, {
      nonzero: false,
    });
    if (BigInt(`0x${coordinate}`) >= BN254_BASE_FIELD_MODULUS) {
      throw new TypeError(`${label}.${field} is not a BN254 field element`);
    }
    return coordinate;
  });
  if (coordinates.every((coordinate) => /^0+$/u.test(coordinate))) {
    throw new TypeError(`${label} must not be the BN254 point at infinity`);
  }
  return coordinates;
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
    "signal_10",
  ];
  const ic = exactFields(record.ic, new Set(icFields), `${label}.ic`);
  for (const field of icFields) words.push(...parseG1(ic[field], `${label}.ic.${field}`));
  if (words.length !== 38) throw new TypeError(`${label} must contain exactly 38 ABI words`);
  return Uint8Array.from(Buffer.from(words.join(""), "hex"));
}

function parseSemanticProofProfile(value, label) {
  const record = exactFields(value, new Set(["profile", "commitments"]), label);
  if (
    canonicalText(record.profile, `${label}.profile`, 64) !==
    "sora_taira_finality_inclusion_groth16_bn254"
  ) {
    throw new TypeError(`${label}.profile is unsupported or retired`);
  }
  const commitments = exactFields(
    record.commitments,
    new Set([
      "version",
      "circuit_commitment",
      "witness_generator_commitment",
      "public_signal_schema_hash",
    ]),
    `${label}.commitments`,
  );
  integer(commitments.version, `${label}.commitments.version`, 1, 1);
  const roles = [
    bytesFromUpperHex(
      commitments.circuit_commitment,
      `${label}.commitments.circuit_commitment`,
      32,
    ),
    bytesFromUpperHex(
      commitments.witness_generator_commitment,
      `${label}.commitments.witness_generator_commitment`,
      32,
    ),
    bytesFromUpperHex(
      commitments.public_signal_schema_hash,
      `${label}.commitments.public_signal_schema_hash`,
      32,
    ),
  ];
  if (commitments.public_signal_schema_hash !== PUBLIC_SIGNAL_SCHEMA_HASH) {
    throw new TypeError(`${label} does not commit the exact eleven-signal schema`);
  }
  if (new Set(roles.map(lowerHexBytes)).size !== roles.length) {
    throw new TypeError(`${label} reuses a semantic commitment role`);
  }
  const canonical = concatenateBytes(Uint8Array.of(1, 0, 1), ...roles);
  return Object.freeze({
    hash: Uint8Array.from(
      keccak_256(concatenateBytes(SEMANTIC_PROOF_PROFILE_PREFIX, canonical)),
    ),
    roles: Object.freeze(roles),
  });
}

function parseSoraFinalityAnchor(value, label) {
  const record = exactFields(
    value,
    new Set([
      "version",
      "source_network",
      "protocol_version",
      "chain_id_hash",
      "checkpoint_height",
      "checkpoint_block_hash",
      "checkpoint_context_id",
      "checkpoint_finality_artifact_hash",
    ]),
    label,
  );
  integer(record.version, `${label}.version`, 1, 1);
  const source = parseNetwork(record.source_network, `${label}.source_network`);
  if (source.profile !== "sora-taira") {
    throw new TypeError(`${label}.source_network must be SORA Taira`);
  }
  const protocolVersion = integer(
    record.protocol_version,
    `${label}.protocol_version`,
    4,
    4,
  );
  const chainHash = bytesFromUpperHex(record.chain_id_hash, `${label}.chain_id_hash`, 32);
  if (record.chain_id_hash !== SORA_TAIRA_CHAIN_ID_HASH) {
    throw new TypeError(`${label}.chain_id_hash is not the Taira chain commitment`);
  }
  const checkpointHeight = integer(
    record.checkpoint_height,
    `${label}.checkpoint_height`,
    1,
  );
  const checkpointHash = bytesFromUpperHex(
    record.checkpoint_block_hash,
    `${label}.checkpoint_block_hash`,
    32,
  );
  const contextId = bytesFromUpperHex(
    record.checkpoint_context_id,
    `${label}.checkpoint_context_id`,
    32,
  );
  const finalityArtifactHash = bytesFromUpperHex(
    record.checkpoint_finality_artifact_hash,
    `${label}.checkpoint_finality_artifact_hash`,
    32,
  );
  const roles = [chainHash, checkpointHash, contextId, finalityArtifactHash];
  if (new Set(roles.map(lowerHexBytes)).size !== roles.length) {
    throw new TypeError(`${label} reuses a consensus hash role`);
  }
  const canonical = concatenateBytes(
    Uint8Array.of(1, NETWORKS["sora-taira"].tag),
    unsignedLittleEndian(protocolVersion, 2, `${label}.protocol_version`),
    chainHash,
    unsignedLittleEndian(checkpointHeight, 8, `${label}.checkpoint_height`),
    checkpointHash,
    contextId,
    finalityArtifactHash,
  );
  return Object.freeze({
    hash: Uint8Array.from(
      keccak_256(concatenateBytes(SORA_FINALITY_ANCHOR_PREFIX, canonical)),
    ),
    roles: Object.freeze(roles),
  });
}

function requireDistinctProofPolicyRoles(semantic, anchor, label) {
  const roles = [...semantic.roles, semantic.hash, ...anchor.roles, anchor.hash];
  if (roles.some(allZero) || new Set(roles.map(lowerHexBytes)).size !== roles.length) {
    throw new TypeError(`${label} reuses a proof-policy hash role`);
  }
}

function parseOutboundProofPolicy(value, label) {
  const record = exactFields(
    value,
    new Set(["version", "semantic_profile", "sora_finality_anchor"]),
    label,
  );
  integer(record.version, `${label}.version`, 1, 1);
  const semantic = parseSemanticProofProfile(
    record.semantic_profile,
    `${label}.semantic_profile`,
  );
  const anchor = parseSoraFinalityAnchor(
    record.sora_finality_anchor,
    `${label}.sora_finality_anchor`,
  );
  requireDistinctProofPolicyRoles(semantic, anchor, label);
  return Object.freeze({
    semanticHash: semantic.hash,
    semanticRoles: semantic.roles,
    anchorHash: anchor.hash,
  });
}

function portableVerifyingKeyIdField(value, label) {
  const text = canonicalText(value, label, 256);
  if (
    !/^[a-z0-9](?:[a-z0-9_/:.-]*[a-z0-9])?$/u.test(text) ||
    ["..", "//", ":::", "/:", ":/", "/.", "./", ":.", ".:"].some((part) =>
      text.includes(part),
    )
  ) {
    throw new TypeError(`${label} must use portable verification-key registry syntax`);
  }
  return text;
}

function parseSoraOutboundExecutionPolicy(value, label) {
  const record = exactFields(
    value,
    new Set(["version", "semantics", "contract_artifact_sha256", "vk_ref", "gas_limit"]),
    label,
  );
  integer(record.version, `${label}.version`, 1, 1);
  if (
    canonicalText(record.semantics, `${label}.semantics`, 64) !==
    SCCP_SORA_OUTBOUND_EXECUTION_SEMANTICS_V1
  ) {
    throw new TypeError(`${label}.semantics is unsupported or retired`);
  }
  const contractArtifactSha256 = bytesFromUpperHex(
    record.contract_artifact_sha256,
    `${label}.contract_artifact_sha256`,
    32,
  );
  const vkRef = exactFields(
    record.vk_ref,
    new Set(["backend", "name", "version", "commitment"]),
    `${label}.vk_ref`,
  );
  const backend = portableVerifyingKeyIdField(vkRef.backend, `${label}.vk_ref.backend`);
  const name = portableVerifyingKeyIdField(vkRef.name, `${label}.vk_ref.name`);
  const version = integer(vkRef.version, `${label}.vk_ref.version`, 1, 0xffff_ffff);
  const commitment = bytesFromUpperHex(vkRef.commitment, `${label}.vk_ref.commitment`, 32);
  if (commitment.every((byte) => byte === 0)) {
    throw new TypeError(`${label}.vk_ref.commitment must be nonzero`);
  }
  const gasLimit = integer(
    record.gas_limit,
    `${label}.gas_limit`,
    1,
    SCCP_MAX_SORA_OUTBOUND_GAS_LIMIT_V1,
  );
  return Object.freeze({
    contractArtifactSha256,
    backend,
    name,
    version,
    commitment,
    gasLimit,
  });
}

function canonicalNetworkBytes(network) {
  const prefix = Uint8Array.of(1, network.tag);
  const domain = unsignedLittleEndian(network.domain, 4, `${network.profile}.domain`);
  let identity;
  switch (network.profile) {
    case "sora-taira":
      identity = Uint8Array.from(Buffer.from("fc56984b2be7431d840e21514d1883f0", "hex"));
      break;
    case "ethereum-mainnet":
      identity = unsignedLittleEndian(1, 8, `${network.profile}.chain_id`);
      break;
    case "ethereum-sepolia":
      identity = unsignedLittleEndian(11_155_111, 8, `${network.profile}.chain_id`);
      break;
    case "bsc-mainnet":
      identity = unsignedLittleEndian(56, 8, `${network.profile}.chain_id`);
      break;
    case "bsc-testnet":
      identity = unsignedLittleEndian(97, 8, `${network.profile}.chain_id`);
      break;
    case "tron-mainnet":
      identity = unsignedLittleEndian(0x2b66_53dc, 4, `${network.profile}.network_id`);
      break;
    case "tron-nile":
      identity = unsignedLittleEndian(0xcd86_90dc, 4, `${network.profile}.network_id`);
      break;
    case "tron-shasta":
      identity = unsignedLittleEndian(0x94a9_059e, 4, `${network.profile}.network_id`);
      break;
    case "solana-testnet":
      identity = SOLANA_TESTNET_GENESIS_BYTES;
      break;
    default:
      throw new TypeError(`${network.profile} is not a supported SCCP V1 profile`);
  }
  return concatenateBytes(prefix, domain, identity);
}

function canonicalLaneBytes(lane) {
  const source = canonicalNetworkBytes(lane.source);
  const target = canonicalNetworkBytes(lane.target);
  return concatenateBytes(
    Uint8Array.of(1),
    unsignedLittleEndian(source.length, 4, "SCCP source network byte length"),
    source,
    unsignedLittleEndian(target.length, 4, "SCCP target network byte length"),
    target,
  );
}

function laneHash(lane) {
  const canonical = canonicalLaneBytes(lane);
  return Uint8Array.from(blake2b256(concatenateBytes(LANE_HASH_PREFIX, canonical)));
}

function externalNetworkParameters(network) {
  switch (network.profile) {
    case "ethereum-mainnet":
      return Object.freeze({ chainOrNetworkId: 1, routeId: "taira_eth_xor" });
    case "ethereum-sepolia":
      return Object.freeze({ chainOrNetworkId: 11_155_111, routeId: "taira_eth_xor" });
    case "bsc-mainnet":
      return Object.freeze({ chainOrNetworkId: 56, routeId: "taira_bsc_xor" });
    case "bsc-testnet":
      return Object.freeze({ chainOrNetworkId: 97, routeId: "taira_bsc_xor" });
    case "tron-mainnet":
      return Object.freeze({ chainOrNetworkId: 0x2b66_53dc, routeId: "taira_tron_xor" });
    case "tron-nile":
      return Object.freeze({ chainOrNetworkId: 0xcd86_90dc, routeId: "taira_tron_xor" });
    case "tron-shasta":
      return Object.freeze({ chainOrNetworkId: 0x94a9_059e, routeId: "taira_tron_xor" });
    case "solana-testnet":
      return Object.freeze({ chainOrNetworkId: null, routeId: "taira_sol_xor" });
    default:
      throw new TypeError(`${network.profile} is not an external SCCP V1 destination`);
  }
}

function requireDistinctHashRoles(roles, label) {
  if (roles.some(allZero) || new Set(roles.map(lowerHexBytes)).size !== roles.length) {
    throw new TypeError(`${label} reuses a role-separated hash`);
  }
}

function deriveSolanaNativeVerifierConfigHash({
  lane,
  routeRevision,
  values,
  policy,
  multiplier,
  sourceProgramId,
  label,
}) {
  if (lane.source.profile !== "solana-testnet" || lane.target.profile !== "sora-taira") {
    throw new TypeError(`${label} must use the exact Solana-testnet to SORA-Taira lane`);
  }
  const sourceLaneHash = laneHash(lane);
  const destinationLaneHash = laneHash({ source: lane.target, target: lane.source });
  const [circuitCommitment, witnessGeneratorCommitment, publicSignalSchemaHash] =
    policy.semanticRoles;
  return Uint8Array.from(
    sha256(
      concatenateBytes(
        SOLANA_NATIVE_VERIFIER_CONFIG_PREFIX,
        Uint8Array.of(1),
        canonicalNetworkBytes(lane.target),
        canonicalNetworkBytes(lane.source),
        new TextEncoder().encode("taira_sol_xor"),
        unsignedLittleEndian(routeRevision, 4, "SCCP Solana route revision"),
        Uint8Array.of(3),
        new TextEncoder().encode("xor"),
        unsignedBigIntLittleEndian(
          BigInt(multiplier),
          16,
          "SCCP Taira-to-SPL-token multiplier",
        ),
        values.verifier_key_hash,
        values.native_verifier_program_id,
        values.route_program_id,
        sourceProgramId,
        values.route_state_account,
        values.token_mint_address,
        SOLANA_CLASSIC_TOKEN_PROGRAM_ID,
        sourceLaneHash,
        destinationLaneHash,
        policy.anchorHash,
        policy.semanticHash,
        circuitCommitment,
        witnessGeneratorCommitment,
        publicSignalSchemaHash,
      ),
    ),
  );
}

function deriveDestinationHashes({
  family,
  lane,
  addresses,
  hashes,
  policy,
  routeRevision,
  multiplier,
}) {
  const network = lane.source;
  const { chainOrNetworkId, routeId } = externalNetworkParameters(network);
  const [tokenAddress, verifierAddress, routeAddress] = addresses;
  const [tokenCodeHash, verifierCodeHash, verifierKeyHash] = hashes;
  const destinationBindingHash = Uint8Array.from(
    keccak_256(
      concatenateBytes(
        keccak_256(
          family === "tron"
            ? TRON_DESTINATION_BINDING_PREFIX
            : EVM_DESTINATION_BINDING_PREFIX,
        ),
        keccak_256(family === "tron" ? TRON_GROTH16_BACKEND : EVM_GROTH16_BACKEND),
        abiWordUnsigned(chainOrNetworkId, `${network.profile}.chain_or_network_id`),
        abiWordUnsigned(SCCP_DOMAIN_SORA, "SCCP source domain"),
        abiWordUnsigned(network.domain, "SCCP target domain"),
        abiWordAddress20(verifierAddress, "SCCP verifier address", {
          tron: family === "tron",
        }),
        abiWordAddress20(routeAddress, "SCCP route address", { tron: family === "tron" }),
        verifierCodeHash,
        verifierKeyHash,
        policy.semanticHash,
        policy.anchorHash,
      ),
    ),
  );

  const sourceLaneHash = laneHash(lane);
  const destinationLaneHash = laneHash({ source: lane.target, target: lane.source });
  const routeRoles = [
    sourceLaneHash,
    destinationLaneHash,
    tokenCodeHash,
    verifierCodeHash,
    verifierKeyHash,
    policy.semanticHash,
    policy.anchorHash,
  ];
  if (family === "tron") routeRoles.push(destinationBindingHash);
  requireDistinctHashRoles(routeRoles, "SCCP route configuration");

  const deploymentConfigWords = [
    abiWordAddress20(tokenAddress, "SCCP token address"),
    tokenCodeHash,
    abiWordAddress20(verifierAddress, "SCCP verifier address"),
    verifierCodeHash,
    verifierKeyHash,
    policy.semanticHash,
    policy.anchorHash,
  ];
  if (family === "tron") deploymentConfigWords.push(destinationBindingHash);
  const deploymentConfigHash = Uint8Array.from(
    keccak_256(concatenateBytes(...deploymentConfigWords)),
  );
  const assetRouteConfigHash = Uint8Array.from(
    keccak_256(
      concatenateBytes(
        keccak_256(new TextEncoder().encode("xor")),
        keccak_256(new TextEncoder().encode(routeId)),
        abiWordUnsigned(routeRevision, "SCCP route revision"),
        abiWordUnsigned(multiplier, "SCCP Taira-to-token multiplier"),
      ),
    ),
  );
  const routeConfigurationHash = Uint8Array.from(
    keccak_256(
      concatenateBytes(
        keccak_256(CONCRETE_ROUTE_CONFIG_PREFIX),
        abiWordUnsigned(network.domain, "SCCP target domain"),
        abiWordUnsigned(network.tag, "SCCP target network tag"),
        abiWordUnsigned(chainOrNetworkId, `${network.profile}.chain_or_network_id`),
        sourceLaneHash,
        destinationLaneHash,
        deploymentConfigHash,
        assetRouteConfigHash,
      ),
    ),
  );
  return Object.freeze({
    destinationBindingHash,
    deploymentConfigHash,
    routeConfigurationHash,
  });
}

function parseSolanaDestinationDeployment(
  deploymentValue,
  lane,
  routeRevision,
  sourceProgramId,
  label,
  { enforceNativeConfig = true } = {},
) {
  integer(routeRevision, `${label}.route_revision`, 1, 1);
  const fields = [
    "token_mint_address",
    "route_program_id",
    "route_program_data_address",
    "route_program_data_slot",
    "route_state_account",
    "route_program_code_hash",
    "native_verifier_program_id",
    "native_verifier_program_data_address",
    "native_verifier_program_data_slot",
    "native_verifier_material_account",
    "native_verifier_program_code_hash",
    "native_verifier_config_hash",
    "verifying_key",
    "verifier_key_hash",
    "outbound_proof_policy",
    "taira_to_token_multiplier",
  ];
  const deployment = exactFields(deploymentValue, new Set(fields), `${label}.deployment`);
  const addressFields = [
    "token_mint_address",
    "route_program_id",
    "route_program_data_address",
    "route_state_account",
    "native_verifier_program_id",
    "native_verifier_program_data_address",
    "native_verifier_material_account",
  ];
  const hashFields = [
    "route_program_code_hash",
    "native_verifier_program_code_hash",
    "native_verifier_config_hash",
    "verifier_key_hash",
  ];
  const values = Object.fromEntries(
    [...addressFields, ...hashFields].map((field) => [
      field,
      bytesFromUpperHex(deployment[field], `${label}.deployment.${field}`, 32),
    ]),
  );
  const routeProgramDataSlot = integer(
    deployment.route_program_data_slot,
    `${label}.deployment.route_program_data_slot`,
    1,
  );
  const nativeVerifierProgramDataSlot = integer(
    deployment.native_verifier_program_data_slot,
    `${label}.deployment.native_verifier_program_data_slot`,
    1,
  );
  const keyBytes = parseVerifyingKey(deployment.verifying_key, `${label}.deployment.verifying_key`);
  if (
    lowerHexBytes(keccak_256(keyBytes)).toUpperCase() !== deployment.verifier_key_hash
  ) {
    throw new TypeError(`${label}.deployment.verifier_key_hash does not match verifying_key`);
  }
  const policy = parseOutboundProofPolicy(
    deployment.outbound_proof_policy,
    `${label}.deployment.outbound_proof_policy`,
  );
  const allRoles = [
    ...addressFields.map((field) => values[field]),
    ...hashFields.map((field) => values[field]),
    policy.semanticHash,
    policy.anchorHash,
    SOLANA_CLASSIC_TOKEN_PROGRAM_ID,
  ];
  requireDistinctHashRoles(allRoles, `${label}.deployment`);
  if (
    [values.route_program_id, values.native_verifier_program_id].some(
      (programId) => lowerHexBytes(programId) === lowerHexBytes(sourceProgramId),
    )
  ) {
    throw new TypeError(`${label}.source_program_id reuses a destination program role`);
  }
  const multiplier = integer(
    deployment.taira_to_token_multiplier,
    `${label}.deployment.taira_to_token_multiplier`,
    1,
    1,
  );
  const nativeVerifierConfigHash = deriveSolanaNativeVerifierConfigHash({
    lane,
    routeRevision,
    values,
    policy,
    multiplier,
    sourceProgramId,
    label,
  });
  if (
    enforceNativeConfig &&
    lowerHexBytes(nativeVerifierConfigHash) !== lowerHexBytes(values.native_verifier_config_hash)
  ) {
    throw new TypeError(
      `${label}.deployment.native_verifier_config_hash does not match the exact V1 preimage`,
    );
  }

  const destinationBindingHash = Uint8Array.from(
    keccak_256(
      concatenateBytes(
        SOLANA_DESTINATION_BINDING_PREFIX,
        Uint8Array.of(1),
        SOLANA_GROTH16_BACKEND,
        Uint8Array.of(NETWORKS["solana-testnet"].tag),
        unsignedLittleEndian(SCCP_DOMAIN_SORA, 4, "SCCP source domain"),
        unsignedLittleEndian(SCCP_DOMAIN_SOLANA, 4, "SCCP target domain"),
        SOLANA_TESTNET_GENESIS_BYTES,
        values.token_mint_address,
        values.route_program_id,
        values.route_program_data_address,
        unsignedLittleEndian(
          routeProgramDataSlot,
          8,
          `${label}.deployment.route_program_data_slot`,
        ),
        values.route_state_account,
        values.route_program_code_hash,
        values.native_verifier_program_id,
        values.native_verifier_program_data_address,
        unsignedLittleEndian(
          nativeVerifierProgramDataSlot,
          8,
          `${label}.deployment.native_verifier_program_data_slot`,
        ),
        values.native_verifier_material_account,
        values.native_verifier_program_code_hash,
        values.native_verifier_config_hash,
        values.verifier_key_hash,
        policy.semanticHash,
        policy.anchorHash,
      ),
    ),
  );

  const sourceLaneHash = laneHash(lane);
  const destinationLaneHash = laneHash({ source: lane.target, target: lane.source });
  requireDistinctHashRoles(
    [
      sourceLaneHash,
      destinationLaneHash,
      values.route_program_code_hash,
      values.native_verifier_program_code_hash,
      values.native_verifier_config_hash,
      values.verifier_key_hash,
      policy.semanticHash,
      policy.anchorHash,
      destinationBindingHash,
    ],
    "SCCP Solana route configuration",
  );
  const deploymentConfigHash = Uint8Array.from(
    keccak_256(
      concatenateBytes(
        values.token_mint_address,
        values.route_program_id,
        values.route_program_data_address,
        unsignedLittleEndian(
          routeProgramDataSlot,
          8,
          `${label}.deployment.route_program_data_slot`,
        ),
        values.route_state_account,
        values.route_program_code_hash,
        values.native_verifier_program_id,
        values.native_verifier_program_data_address,
        unsignedLittleEndian(
          nativeVerifierProgramDataSlot,
          8,
          `${label}.deployment.native_verifier_program_data_slot`,
        ),
        values.native_verifier_material_account,
        values.native_verifier_program_code_hash,
        values.native_verifier_config_hash,
        values.verifier_key_hash,
        policy.semanticHash,
        policy.anchorHash,
        destinationBindingHash,
      ),
    ),
  );
  const assetRouteConfigHash = Uint8Array.from(
    keccak_256(
      concatenateBytes(
        new TextEncoder().encode("xor"),
        new TextEncoder().encode("taira_sol_xor"),
        unsignedLittleEndian(routeRevision, 4, "SCCP route revision"),
        unsignedLittleEndian(multiplier, 8, "SCCP Taira-to-token multiplier"),
      ),
    ),
  );
  const routeConfigurationHash = Uint8Array.from(
    keccak_256(
      concatenateBytes(
        CONCRETE_ROUTE_CONFIG_PREFIX,
        Uint8Array.of(1),
        unsignedLittleEndian(SCCP_DOMAIN_SOLANA, 4, "SCCP target domain"),
        Uint8Array.of(NETWORKS["solana-testnet"].tag),
        SOLANA_TESTNET_GENESIS_BYTES,
        sourceLaneHash,
        destinationLaneHash,
        deploymentConfigHash,
        assetRouteConfigHash,
      ),
    ),
  );
  return Object.freeze({
    family: "solana",
    routeAddress: lowerHexBytes(values.route_program_id).toUpperCase(),
    routeCodeHash: lowerHexBytes(values.route_program_code_hash).toUpperCase(),
    destinationBindingHash,
    deploymentConfigHash,
    routeConfigurationHash,
    nativeVerifierConfigHash,
    proofPolicyRoles: Object.freeze([
      values.verifier_key_hash,
      policy.semanticHash,
      policy.anchorHash,
    ]),
    deploymentRoles: Object.freeze(allRoles.map(lowerHexBytes)),
  });
}

function parseDestination(value, lane, routeRevision, source, label) {
  const record = exactFields(value, new Set(["family", "deployment"]), label);
  const family = canonicalText(record.family, `${label}.family`, 16);
  if (family !== emitterFamily(lane.source)) {
    throw new TypeError(`${label}.family does not match the lane source`);
  }
  if (family === "solana") {
    if (source?.family !== "solana") {
      throw new TypeError(`${label} requires an exact Solana source identity`);
    }
    return parseSolanaDestinationDeployment(
      record.deployment,
      lane,
      routeRevision,
      bytesFromUpperHex(source.address, `${label}.source_program_id`, 32),
      label,
    );
  }
  const fields = [
    "token_address",
    "token_code_hash",
    "verifier_address",
    "verifier_code_hash",
    "verifying_key",
    "verifier_key_hash",
    "outbound_proof_policy",
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
  const policy = parseOutboundProofPolicy(
    deployment.outbound_proof_policy,
    `${label}.deployment.outbound_proof_policy`,
  );
  const deploymentHashes = [
    ...hashes,
    lowerHexBytes(policy.semanticHash).toUpperCase(),
    lowerHexBytes(policy.anchorHash).toUpperCase(),
  ];
  if (new Set(deploymentHashes).size !== deploymentHashes.length) {
    throw new TypeError(`${label}.deployment reuses a role-separated policy or code hash`);
  }
  const multiplier = integer(
    deployment.taira_to_token_multiplier,
    `${label}.deployment.taira_to_token_multiplier`,
    1_000_000_000,
    1_000_000_000,
  );
  const derived = deriveDestinationHashes({
    family,
    lane,
    addresses,
    hashes: hashes.slice(0, 3).map((hash) => Uint8Array.from(Buffer.from(hash, "hex"))),
    policy,
    routeRevision,
    multiplier,
  });
  return Object.freeze({
    family,
    routeAddress: addresses[2],
    routeCodeHash: hashes[3],
    proofPolicyRoles: Object.freeze([
      Uint8Array.from(Buffer.from(hashes[2], "hex")),
      policy.semanticHash,
      policy.anchorHash,
    ]),
    ...derived,
  });
}

/** Derive exact governed hashes for one Solana-testnet XOR destination deployment. */
export function deriveSccpSolanaDestinationHashesV1(
  deployment,
  sourceProgramId,
  routeRevision = 1,
) {
  const revision = integer(routeRevision, "SCCP Solana route revision", 1, 1);
  const lane = parseLaneId(
    {
      source: { network: "solana_testnet", profile: null },
      target: { network: "sora_taira", profile: null },
    },
    "SCCP Solana destination lane",
  );
  const parsed = parseSolanaDestinationDeployment(
    deployment,
    lane,
    revision,
    bytesFromUpperHex(sourceProgramId, "SCCP Solana source program id", 32),
    "SCCP Solana destination",
  );
  return Object.freeze({
    destination_binding_hash: prefixedLowerHex(parsed.destinationBindingHash),
    deployment_config_hash: prefixedLowerHex(parsed.deploymentConfigHash),
    route_configuration_hash: prefixedLowerHex(parsed.routeConfigurationHash),
  });
}

/**
 * Derive the exact one-way material-config commitment for the native Solana verifier.
 *
 * Destination-binding and route-configuration hashes are deliberately excluded:
 * both commit this result and feeding either one back would require a cryptographic
 * fixed point. The supplied config field is ignored so deployment tooling can derive
 * the commitment before creating its config-addressed material account.
 */
export function deriveSccpSolanaNativeVerifierConfigHashV1(
  deployment,
  sourceProgramId,
  routeRevision = 1,
) {
  const revision = integer(routeRevision, "SCCP Solana route revision", 1, 1);
  const lane = parseLaneId(
    {
      source: { network: "solana_testnet", profile: null },
      target: { network: "sora_taira", profile: null },
    },
    "SCCP Solana destination lane",
  );
  const parsed = parseSolanaDestinationDeployment(
    deployment,
    lane,
    revision,
    bytesFromUpperHex(sourceProgramId, "SCCP Solana source program id", 32),
    "SCCP Solana destination",
    { enforceNativeConfig: false },
  );
  return prefixedLowerHex(parsed.nativeVerifierConfigHash);
}

/** Derive exact governed emitter and source-identity hashes for Solana testnet. */
export function deriveSccpSolanaSourceIdentityHashesV1(identity) {
  const record = exactFields(
    identity,
    new Set(["lane", "emitter"]),
    "SCCP Solana source identity",
  );
  const lane = parseLaneId(record.lane, "SCCP Solana source identity.lane");
  if (lane.source.profile !== "solana-testnet") {
    throw new TypeError("SCCP Solana source identity must use the exact Solana-testnet lane");
  }
  const parsed = parseSourceIdentity(record, lane, "SCCP Solana source identity");
  if (parsed.family !== "solana") {
    throw new TypeError("SCCP Solana source identity must contain a Solana emitter");
  }
  return Object.freeze({
    source_emitter_identity_hash: parsed.emitterIdentityHash,
    source_identity_hash: parsed.sourceIdentityHash,
  });
}

function parseSettlement(value, label) {
  const record = exactFields(
    value,
    new Set(["asset_definition_id", "custody_account_id", "payload_amount_scale"]),
    label,
  );
  const assetDefinitionId = canonicalText(
    record.asset_definition_id,
    `${label}.asset_definition_id`,
    512,
  );
  if (assetDefinitionId !== TAIRA_XOR_ASSET_DEFINITION_ID) {
    throw new TypeError(`${label}.asset_definition_id is not the first-release Taira XOR asset`);
  }
  const custody = canonicalText(record.custody_account_id, `${label}.custody_account_id`, 512);
  AccountAddress.fromAccountId(custody);
  const payloadAmountScale = integer(
    record.payload_amount_scale,
    `${label}.payload_amount_scale`,
    9,
    9,
  );
  return Object.freeze({ payloadAmountScale });
}

function parseGovernedRoute(value, lane, label) {
  const fields = new Set([
    "lane_id",
    "route_id",
    "asset_key",
    "revision",
    "activation",
    "inbound_finality_cutoff",
    "source_identity",
    "destination",
    "sora_outbound_execution_policy",
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
  const { routeId: expectedRouteId } = externalNetworkParameters(lane.source);
  const settlement = parseSettlement(record.settlement, `${label}.settlement`);
  if (
    record.route_id !== expectedRouteId ||
    record.asset_key !== "xor" ||
    settlement.payloadAmountScale !== 9
  ) {
    throw new TypeError(`${label} does not identify the exact first-release XOR route`);
  }
  const activation = parseActivation(record.activation, `${label}.activation`);
  const inboundFinalityCutoff = parseInboundFinalityCutoff(
    record.inbound_finality_cutoff,
    activation,
    `${label}.inbound_finality_cutoff`,
  );
  const source = parseSourceIdentity(record.source_identity, lane, `${label}.source_identity`);
  const destination = parseDestination(
    record.destination,
    lane,
    revision,
    source,
    `${label}.destination`,
  );
  const executionPolicy = parseSoraOutboundExecutionPolicy(
    record.sora_outbound_execution_policy,
    `${label}.sora_outbound_execution_policy`,
  );
  requireDistinctHashRoles(
    [
      executionPolicy.contractArtifactSha256,
      executionPolicy.commitment,
      destination.destinationBindingHash,
      destination.routeConfigurationHash,
      ...destination.proofPolicyRoles,
    ],
    `${label}.sora_outbound_execution_policy`,
  );
  if (source.family !== destination.family) throw new TypeError(`${label} family roles disagree`);
  if (
    source.family === "solana" &&
    [source.address, source.programDataAddress, source.stateAccount, source.runtime].some((role) =>
      destination.deploymentRoles.includes(role.toLowerCase()),
    )
  ) {
    throw new TypeError(`${label} aliases a Solana source role with a destination deployment role`);
  }
  if (
    source.family !== "solana" &&
    (source.address !== destination.routeAddress || source.runtime !== destination.routeCodeHash)
  ) {
    throw new TypeError(`${label} source emitter does not identify the destination route deployment`);
  }
  if (
    source.configuration !== lowerHexBytes(destination.routeConfigurationHash).toUpperCase()
  ) {
    throw new TypeError(
      `${label} source emitter route_config_hash does not match the exact destination route configuration`,
    );
  }
  return Object.freeze({
    lineage: `${record.route_id}\u0000${record.asset_key}`,
    key: `${lane.source.profile}\u0000${lane.target.profile}\u0000${record.route_id}\u0000${record.asset_key}\u0000${revision}`,
    revision,
    activation,
    inboundFinalityCutoff,
    destinationBindingHash: destination.destinationBindingHash,
    deploymentConfigHash: destination.deploymentConfigHash,
    routeConfigurationHash: destination.routeConfigurationHash,
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
    (current === "staged" && ["bidirectional", "inbound_only", "retired"].includes(next)) ||
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
      new Set(["key", "expected_current", "next", "inbound_finality_cutoff"]),
      "SCCP route action.SetActivation",
    );
    parseRouteKey(update.key, "SCCP route action.SetActivation.key");
    const current = parseActivation(
      update.expected_current,
      "SCCP route action.SetActivation.expected_current",
    );
    const next = parseActivation(update.next, "SCCP route action.SetActivation.next");
    parseInboundFinalityCutoff(
      update.inbound_finality_cutoff,
      next,
      "SCCP route action.SetActivation.inbound_finality_cutoff",
    );
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
        "previous_inbound_finality_cutoff",
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
    parseInboundFinalityCutoff(
      update.previous_inbound_finality_cutoff,
      previousNext,
      "SCCP route action.SwitchRevision.previous_inbound_finality_cutoff",
    );
    const previousTransitionValid =
      previousNext === "retired"
        ? ["bidirectional", "inbound_only", "paused"].includes(expected)
        : canTransitionActivation(expected, previousNext);
    if (
      !sameLane(previous.lane, successor.lane) ||
      previous.routeId !== successor.routeId ||
      previous.assetKey !== successor.assetKey ||
      successor.revision !== previous.revision + 1 ||
      !previousTransitionValid ||
      !["inbound_only", "paused", "retired"].includes(previousNext) ||
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
    if (!/^[\x21-\x7e]+$/u.test(text)) {
      try {
        const { address, chainDiscriminant } = AccountAddress.parseEncoded(text);
        if (address.toI105(chainDiscriminant) !== text) {
          throw new TypeError("canonical_text I105 rendering is not canonical");
        }
      } catch (error) {
        throw new TypeError(
          "canonical_text must contain printable ASCII or an exact canonical I105 account address",
          { cause: error },
        );
      }
    }
    return new TextEncoder().encode(text);
  }
  const raw = binary(value, SCCP_CODEC_KEYS[codec]);
  if (allZero(raw)) throw new TypeError(`${SCCP_CODEC_KEYS[codec]} must be nonzero`);
  if (codec === SCCP_CODEC_EVM_ADDRESS20 && raw.length !== 20) {
    throw new TypeError("evm_address20 must contain exactly 20 bytes");
  }
  if (codec === SCCP_CODEC_SOLANA_PUBKEY32 && raw.length !== 32) {
    throw new TypeError("solana_pubkey32 must contain exactly 32 bytes");
  }
  if (
    codec === SCCP_CODEC_TRON_ADDRESS21 &&
    (raw.length !== 21 || raw[0] !== 0x41 || allZero(raw.subarray(1)))
  ) {
    throw new TypeError("tron_address21 must contain 0x41 and a nonzero 20-byte address");
  }
  return raw;
}

function accountCodecForDomain(domain) {
  switch (domain) {
    case SCCP_DOMAIN_SORA:
      return SCCP_CODEC_CANONICAL_TEXT;
    case SCCP_DOMAIN_ETH:
    case SCCP_DOMAIN_BSC:
      return SCCP_CODEC_EVM_ADDRESS20;
    case SCCP_DOMAIN_SOLANA:
      return SCCP_CODEC_SOLANA_PUBKEY32;
    case SCCP_DOMAIN_TRON:
      return SCCP_CODEC_TRON_ADDRESS21;
    default:
      throw new TypeError("domain is unsupported or reserved");
  }
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

function normalizeRegistryLimits(value) {
  const fields = new Set([
    "max_governed_lanes",
    "max_live_governed_routes",
    "max_live_routes_per_lane",
    "max_retained_routes_per_lane",
    "max_retained_native_trust_anchors_per_lane",
  ]);
  const record = exactFields(value, fields, "SCCP registry limits");
  const limits = Object.freeze(
    Object.fromEntries(
      [...fields].map((field) => [
        field,
        integer(record[field], `SCCP registry limits.${field}`, 1, 0xffff_ffff),
      ]),
    ),
  );
  const expected = [16, 64, 8, 64, 4096];
  if ([...fields].some((field, index) => limits[field] !== expected[index])) {
    throw new TypeError("SCCP registry limits must equal the fixed V1 capacities");
  }
  return limits;
}

function normalizeResourceLimits(value) {
  const countFields = new Set([
    "max_outbound_messages_per_block",
    "max_proofs_per_transaction",
    "max_proofs_per_block",
    "max_native_headers_per_transaction",
    "max_native_headers_per_block",
    "max_ethereum_light_client_updates_per_transaction",
    "max_ethereum_light_client_updates_per_block",
    "max_secp256k1_recoveries_per_transaction",
    "max_secp256k1_recoveries_per_block",
    "max_bls_aggregate_checks_per_transaction",
    "max_bls_aggregate_checks_per_block",
    "max_bls_signer_contributions_per_transaction",
    "max_bls_signer_contributions_per_block",
    "max_bn254_pairing_checks_per_transaction",
    "max_bn254_pairing_checks_per_block",
  ]);
  const byteFields = new Set([
    "max_outbound_message_payload_bytes",
    "max_pending_outbound_messages",
    "max_pending_outbound_payload_bytes",
    "max_proof_bytes_per_proof",
    "max_proof_bytes_per_transaction",
    "max_proof_bytes_per_block",
    "max_native_header_bytes_per_transaction",
    "max_native_header_bytes_per_block",
  ]);
  const fields = new Set([...countFields, ...byteFields]);
  const record = exactFields(value, fields, "SCCP resource limits");
  const limits = Object.freeze(
    Object.fromEntries(
      [...fields].map((field) => [
        field,
        integer(
          record[field],
          `SCCP resource limits.${field}`,
          1,
          countFields.has(field) ? 0xffff_ffff : Number.MAX_SAFE_INTEGER,
        ),
      ]),
    ),
  );
  if (
    limits.max_outbound_messages_per_block !== 512 ||
    limits.max_outbound_message_payload_bytes !== 4096
  ) {
    throw new TypeError("SCCP outbound message limits must equal the fixed V1 capacities");
  }
  if (limits.max_proof_bytes_per_proof > limits.max_proof_bytes_per_transaction) {
    throw new TypeError("SCCP per-proof byte limit exceeds its transaction limit");
  }
  const orderedPairs = [
    [limits.max_proofs_per_transaction, limits.max_proofs_per_block],
    [limits.max_proof_bytes_per_transaction, limits.max_proof_bytes_per_block],
    [limits.max_native_headers_per_transaction, limits.max_native_headers_per_block],
    [
      limits.max_ethereum_light_client_updates_per_transaction,
      limits.max_ethereum_light_client_updates_per_block,
    ],
    [limits.max_native_header_bytes_per_transaction, limits.max_native_header_bytes_per_block],
    [
      limits.max_secp256k1_recoveries_per_transaction,
      limits.max_secp256k1_recoveries_per_block,
    ],
    [
      limits.max_bls_aggregate_checks_per_transaction,
      limits.max_bls_aggregate_checks_per_block,
    ],
    [
      limits.max_bls_signer_contributions_per_transaction,
      limits.max_bls_signer_contributions_per_block,
    ],
    [
      limits.max_bn254_pairing_checks_per_transaction,
      limits.max_bn254_pairing_checks_per_block,
    ],
  ];
  if (orderedPairs.some(([transaction, block]) => transaction > block)) {
    throw new TypeError("SCCP transaction resource limits must not exceed block limits");
  }
  return limits;
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
    "sora_outbound_material_path",
    "registry_limits",
    "resource_limits",
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
    "sora_outbound_material_path",
    "registry_limits",
    "resource_limits",
  ]);
  const record = exactFields(value, allowed, "SCCP capabilities", required);
  const proofSubmitPath = exactCapabilityPath(
    record.proof_submit_path,
    "proof_submit_path",
    true,
  );
  const nativeMessageSubmitPath = exactCapabilityPath(
    record.native_message_submit_path,
    "native_message_submit_path",
    true,
  );
  if ((proofSubmitPath === null) !== (nativeMessageSubmitPath === null)) {
    throw new TypeError(
      "SCCP capabilities must advertise proof and native-message submit paths together",
    );
  }
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
    sora_outbound_material_path: exactCapabilityPath(
      record.sora_outbound_material_path,
      "sora_outbound_material_path",
    ),
    registry_limits: normalizeRegistryLimits(record.registry_limits),
    resource_limits: normalizeResourceLimits(record.resource_limits),
    proof_submit_path: proofSubmitPath,
    native_message_submit_path: nativeMessageSubmitPath,
  });
}

/**
 * Normalize the route-scoped, governance-derived SORA outbound IVM material.
 *
 * The returned artifact is accepted only when its SHA-256 matches the exact
 * policy committed by the response. Optional expectations bind an HTTP path
 * (and capability snapshot) to the returned typed route instead of permitting
 * caller-selected bytecode, verification keys, or gas policy.
 */
export function normalizeSccpSoraOutboundMaterial(value, expectations = {}) {
  const label = "SCCP SORA outbound material";
  const record = exactFields(
    value,
    new Set([
      "version",
      "registry_revision",
      "route_key",
      "route_configuration_hash",
      "destination_binding_hash",
      "settlement_asset_definition_id",
      "policy",
      "contract_artifact_b64",
      "contract_code_hash",
      "verifying_key_version",
    ]),
    label,
  );
  integer(record.version, `${label}.version`, 1, 1);
  const registryRevision = exactLowerHex(
    record.registry_revision,
    `${label}.registry_revision`,
    32,
    { prefix: true },
  );
  const routeKey = parseRouteKey(record.route_key, `${label}.route_key`);
  if (routeKey.lane.target.profile !== "sora-taira" || routeKey.lane.source.sora) {
    throw new TypeError(`${label}.route_key must identify one external-to-Taira lane`);
  }
  const routeConfigurationHash = exactLowerHex(
    record.route_configuration_hash,
    `${label}.route_configuration_hash`,
    32,
    { prefix: true },
  );
  const destinationBindingHash = exactLowerHex(
    record.destination_binding_hash,
    `${label}.destination_binding_hash`,
    32,
    { prefix: true },
  );
  if (routeConfigurationHash === destinationBindingHash) {
    throw new TypeError(`${label} aliases its route and destination commitments`);
  }
  if (record.settlement_asset_definition_id !== TAIRA_XOR_ASSET_DEFINITION_ID) {
    throw new TypeError(`${label}.settlement_asset_definition_id is not canonical Taira XOR`);
  }
  const policy = parseSoraOutboundExecutionPolicy(record.policy, `${label}.policy`);
  const contractArtifact = canonicalBase64(
    record.contract_artifact_b64,
    `${label}.contract_artifact_b64`,
    { maximumBytes: MAX_WIRE_BYTES },
  );
  const artifactSha256 = Buffer.from(sha256(contractArtifact)).toString("hex").toUpperCase();
  const governedArtifactSha256 = Buffer.from(policy.contractArtifactSha256)
    .toString("hex")
    .toUpperCase();
  if (artifactSha256 !== governedArtifactSha256) {
    throw new TypeError(`${label}.contract_artifact_b64 does not match policy SHA-256`);
  }
  exactLowerHex(record.contract_code_hash, `${label}.contract_code_hash`, 32, {
    prefix: true,
  });
  integer(
    record.verifying_key_version,
    `${label}.verifying_key_version`,
    0,
    0xffff_ffff,
  );
  if (record.verifying_key_version !== policy.version) {
    throw new TypeError(
      `${label}.verifying_key_version does not match the governed key version`,
    );
  }

  const expected = exactFields(
    expectations,
    new Set([
      "sourceProfile",
      "routeId",
      "assetKey",
      "revision",
      "registryRevision",
    ]),
    `${label} expectations`,
    new Set(),
  );
  const expectationPairs = [
    ["sourceProfile", routeKey.lane.source.profile],
    ["routeId", routeKey.routeId],
    ["assetKey", routeKey.assetKey],
    ["revision", routeKey.revision],
    ["registryRevision", registryRevision],
  ];
  for (const [field, actual] of expectationPairs) {
    if (Object.prototype.hasOwnProperty.call(expected, field) && expected[field] !== actual) {
      throw new TypeError(`${label}.${field} does not match the requested route context`);
    }
  }

  return deepFreezeClone(record);
}

/** Validate the authoritative typed SCCP registry without reinterpreting it as a manifest. */
export function normalizeSccpRegistry(value) {
  const record = exactFields(value, new Set(["version", "lanes"]), "SCCP registry");
  integer(record.version, "SCCP registry.version", 1, 1);
  const lanes = array(record.lanes, "SCCP registry.lanes");
  if (lanes.length > 16) throw new TypeError("SCCP registry contains more than 16 lanes");
  const laneKeys = new Set();
  const routeKeys = new Set();
  let liveRouteCount = 0;
  lanes.forEach((entry, laneIndex) => {
    const label = `SCCP registry.lanes[${laneIndex}]`;
    const laneRecord = exactFields(
      entry,
      new Set([
        "lane_id",
        "native_trust_anchors",
        "current_native_trust_anchor_hash",
        "routes",
      ]),
      label,
    );
    const lane = parseLaneId(laneRecord.lane_id, `${label}.lane_id`);
    const laneKey = `${lane.source.profile}\u0000${lane.target.profile}`;
    if (laneKeys.has(laneKey)) throw new TypeError("SCCP registry contains a duplicate lane");
    laneKeys.add(laneKey);
    const nativeTrustAnchors = array(
      laneRecord.native_trust_anchors,
      `${label}.native_trust_anchors`,
    );
    if (nativeTrustAnchors.length > 4096) {
      throw new TypeError(`${label} contains more than 4,096 retained native trust anchors`);
    }
    const anchorHashes = new Set();
    const parsedAnchors = [];
    let previousAnchor = null;
    nativeTrustAnchors.forEach((anchor, anchorIndex) => {
      const anchorLabel = `${label}.native_trust_anchors[${anchorIndex}]`;
      const parsed = parseNativeTrustAnchor(anchor, lane, anchorLabel);
      if (parsed === null) throw new TypeError(`${anchorLabel} must not be null`);
      if (anchorHashes.has(parsed.anchor_hash)) {
        throw new TypeError(`${label} contains a duplicate native trust-anchor hash`);
      }
      if (
        previousAnchor !== null &&
        (parsed.backend.backend !== previousAnchor.backend.backend ||
          parsed.checkpoint_height <= previousAnchor.checkpoint_height)
      ) {
        throw new TypeError(
          `${label}.native_trust_anchors must advance monotonically within one backend`,
        );
      }
      anchorHashes.add(parsed.anchor_hash);
      parsedAnchors.push(parsed);
      previousAnchor = parsed;
    });
    const currentNativeTrustAnchorHash =
      laneRecord.current_native_trust_anchor_hash === null
        ? null
        : exactUpperHex(
            laneRecord.current_native_trust_anchor_hash,
            `${label}.current_native_trust_anchor_hash`,
            32,
          );
    const expectedCurrentAnchorHash = previousAnchor?.anchor_hash ?? null;
    if (currentNativeTrustAnchorHash !== expectedCurrentAnchorHash) {
      throw new TypeError(
        `${label}.current_native_trust_anchor_hash must name the last retained anchor`,
      );
    }
    const routes = array(laneRecord.routes, `${label}.routes`);
    if (routes.length < 1) {
      throw new TypeError(`${label}.routes must contain at least one route`);
    }
    if (routes.length > 64) {
      throw new TypeError(`${label} contains more than 64 retained route revisions`);
    }
    const lineages = new Map();
    let laneLiveRouteCount = 0;
    routes.forEach((route, routeIndex) => {
      const parsed = parseGovernedRoute(route, lane, `${label}.routes[${routeIndex}]`);
      if (
        previousAnchor === null &&
        ["bidirectional", "inbound_only"].includes(parsed.activation)
      ) {
        throw new TypeError(`${label} cannot enable inbound settlement without a trust anchor`);
      }
      if (
        ["bidirectional", "inbound_only"].includes(parsed.activation) &&
        !INBOUND_ACTIVATABLE_PROFILES.has(lane.source.profile)
      ) {
        throw new TypeError(
          `${label} cannot enable inbound settlement for an unapproved staging profile`,
        );
      }
      if (routeKeys.has(parsed.key)) throw new TypeError("SCCP registry contains a duplicate route");
      routeKeys.add(parsed.key);
      if (parsed.activation !== "retired") {
        laneLiveRouteCount += 1;
        liveRouteCount += 1;
      }
      if (parsed.inboundFinalityCutoff !== null) {
        const anchorIndex = parsedAnchors.findIndex(
          (anchor) => anchor.anchor_hash === parsed.inboundFinalityCutoff.trustAnchorHash,
        );
        if (
          anchorIndex < 0 ||
          anchorIndex + 1 >= parsedAnchors.length ||
          parsedAnchors[anchorIndex + 1].checkpoint_height !==
            parsed.inboundFinalityCutoff.maxAnchorIntervalHeight
        ) {
          throw new TypeError(
            `${label}.routes[${routeIndex}].inbound_finality_cutoff must close one complete retained anchor interval`,
          );
        }
      }
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
    if (laneLiveRouteCount > 8) {
      throw new TypeError(`${label} contains more than 8 live routes`);
    }
  });
  if (liveRouteCount > 64) {
    throw new TypeError("SCCP registry contains more than 64 live routes");
  }
  return deepFreezeClone(record);
}

function parseProjectionText(value, label) {
  const tagged = exactFields(value, new Set(["CanonicalText"]), label);
  const payload = exactFields(
    tagged.CanonicalText,
    new Set(["value"]),
    `${label}.CanonicalText`,
  );
  return canonicalText(payload.value, `${label}.CanonicalText.value`, 512);
}

function parseProjectionRecipient(value, domain, label) {
  const tag =
    domain === SCCP_DOMAIN_TRON
      ? "TronAddress21"
      : domain === SCCP_DOMAIN_SOLANA
        ? "SolanaPubkey32"
        : "EvmAddress20";
  const bytes =
    domain === SCCP_DOMAIN_TRON ? 21 : domain === SCCP_DOMAIN_SOLANA ? 32 : 20;
  const tagged = exactFields(value, new Set([tag]), label);
  const payload = exactFields(tagged[tag], new Set(["bytes"]), `${label}.${tag}`);
  const address = exactLowerHex(payload.bytes, `${label}.${tag}.bytes`, bytes, { prefix: true });
  if (domain === SCCP_DOMAIN_TRON && !address.startsWith("0x41")) {
    throw new TypeError(`${label}.TronAddress21.bytes must use the canonical 0x41 prefix`);
  }
  if (domain === SCCP_DOMAIN_SOLANA && /^0x0{64}$/u.test(address)) {
    throw new TypeError(`${label}.SolanaPubkey32.bytes must be nonzero`);
  }
}

function parsePayloadProjection(value, expectedDomain, label) {
  const tagged = exactFields(value, new Set(["Transfer"]), label);
  const transfer = exactFields(
    tagged.Transfer,
    new Set([
      "version",
      "source_domain",
      "dest_domain",
      "nonce",
      "route_revision",
      "asset_home_domain",
      "asset_id",
      "amount",
      "sender",
      "recipient",
      "route_id",
    ]),
    `${label}.Transfer`,
  );
  integer(transfer.version, `${label}.Transfer.version`, 1, 1);
  integer(transfer.source_domain, `${label}.Transfer.source_domain`, SCCP_DOMAIN_SORA, SCCP_DOMAIN_SORA);
  const domain = integer(transfer.dest_domain, `${label}.Transfer.dest_domain`, 1, 5);
  if (
    domain !== expectedDomain ||
    ![
      SCCP_DOMAIN_ETH,
      SCCP_DOMAIN_BSC,
      SCCP_DOMAIN_SOLANA,
      SCCP_DOMAIN_TRON,
    ].includes(domain)
  ) {
    throw new TypeError(`${label}.Transfer.dest_domain does not match the discovery record`);
  }
  canonicalUnsignedDecimal(transfer.nonce, `${label}.Transfer.nonce`, MAX_U64);
  integer(transfer.route_revision, `${label}.Transfer.route_revision`, 1, 0xffff_ffff);
  integer(
    transfer.asset_home_domain,
    `${label}.Transfer.asset_home_domain`,
    SCCP_DOMAIN_SORA,
    SCCP_DOMAIN_SORA,
  );
  if (parseProjectionText(transfer.asset_id, `${label}.Transfer.asset_id`) !== "xor") {
    throw new TypeError(`${label}.Transfer.asset_id must be canonical XOR`);
  }
  canonicalUnsignedDecimal(transfer.amount, `${label}.Transfer.amount`, MAX_U128, {
    positive: true,
  });
  parseProjectionText(transfer.sender, `${label}.Transfer.sender`);
  parseProjectionRecipient(transfer.recipient, domain, `${label}.Transfer.recipient`);
  const routeId = parseProjectionText(transfer.route_id, `${label}.Transfer.route_id`);
  const expectedRouteId =
    domain === SCCP_DOMAIN_ETH
      ? "taira_eth_xor"
      : domain === SCCP_DOMAIN_BSC
        ? "taira_bsc_xor"
        : domain === SCCP_DOMAIN_SOLANA
          ? "taira_sol_xor"
        : "taira_tron_xor";
  if (routeId !== expectedRouteId) {
    throw new TypeError(`${label}.Transfer.route_id does not match its destination domain`);
  }
  return deepFreezeClone(tagged);
}

/** Normalize newest-first SCCP discovery with only bundle and proof-request links. */
export function normalizeSccpRecentMessages(value) {
  const root = exactFields(
    value,
    new Set(["items", "next"]),
    "SCCP recent messages",
    new Set(["items"]),
  );
  const rawItems = array(root.items, "SCCP recent messages.items");
  if (rawItems.length > 50) {
    throw new TypeError("SCCP recent messages must contain at most 50 items");
  }
  const items = rawItems.map((entry, index) => {
    const label = `SCCP recent messages.items[${index}]`;
    const allowed = new Set([
      "height",
      "commitment_index",
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
      "commitment_index",
      "message_id_hex",
      "kind",
      "source_profile",
      "target_profile",
      "destination_binding_hash",
      "route_configuration_hash",
      "target_domain",
      "amount",
      "payload_projection",
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
    const amount = canonicalUnsignedDecimal(record.amount, `${label}.amount`, MAX_U128, {
      positive: true,
    });
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
    const payloadProjection = parsePayloadProjection(
      record.payload_projection,
      target.domain,
      `${label}.payload_projection`,
    );
    const assetId = optionalText("asset_id");
    const routeId = optionalText("route_id");
    const recipient = optionalText("recipient");
    if (
      (assetId !== null && assetId !== payloadProjection.Transfer.asset_id.CanonicalText.value) ||
      (routeId !== null && routeId !== payloadProjection.Transfer.route_id.CanonicalText.value) ||
      recipient !== null ||
      amount !== String(payloadProjection.Transfer.amount)
    ) {
      throw new TypeError(`${label} summary fields disagree with payload_projection`);
    }
    return Object.freeze({
      height: integer(record.height, `${label}.height`, 1),
      commitment_index: integer(
        record.commitment_index,
        `${label}.commitment_index`,
        0,
        511,
      ),
      message_id_hex: messageId,
      kind: "transfer",
      source_profile: source.profile,
      target_profile: target.profile,
      destination_binding_hash: destinationBindingHash,
      route_configuration_hash: routeConfigurationHash,
      target_domain: target.domain,
      asset_id: assetId,
      route_id: routeId,
      recipient,
      amount,
      payload_projection: payloadProjection,
      links: Object.freeze({
        bundle_path: expectedBundle,
        proof_request_path: expectedRequest,
      }),
    });
  });
  for (let index = 1; index < items.length; index += 1) {
    const previous = items[index - 1];
    const current = items[index];
    if (previous.height < current.height) {
      throw new TypeError("SCCP recent messages must be height-descending");
    }
    if (
      previous.height === current.height &&
      current.commitment_index !== previous.commitment_index + 1
    ) {
      throw new TypeError(
        "SCCP recent messages at one height must have contiguous ascending commitment indices",
      );
    }
    if (previous.height > current.height && current.commitment_index !== 0) {
      throw new TypeError("SCCP recent messages at an older height must begin at commitment index zero");
    }
  }
  if (new Set(items.map(({ message_id_hex: messageId }) => messageId)).size !== items.length) {
    throw new TypeError("SCCP recent messages contain duplicate message ids");
  }
  let next = null;
  if (root.next !== undefined) {
    const cursor = exactFields(
      root.next,
      new Set(["from", "after_index"]),
      "SCCP recent messages.next",
    );
    next = Object.freeze({
      from: integer(cursor.from, "SCCP recent messages.next.from", 1),
      after_index: integer(
        cursor.after_index,
        "SCCP recent messages.next.after_index",
        0,
        511,
      ),
    });
    const last = items.at(-1);
    if (
      last === undefined ||
      next.from !== last.height ||
      next.after_index !== last.commitment_index
    ) {
      throw new TypeError("SCCP recent continuation must identify the last returned item");
    }
  }
  return Object.freeze({ items: Object.freeze(items), next });
}

function validateCanonicalTextBytes(bytes, label) {
  if (bytes.length === 0 || bytes.length > 256) {
    throw new TypeError(`${label} must contain 1..256 canonical text bytes`);
  }
  if (bytes.every((byte) => byte >= 0x21 && byte <= 0x7e)) return;
  let text;
  try {
    text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch (error) {
    throw new TypeError(`${label} must contain canonical UTF-8`, { cause: error });
  }
  try {
    const { address, chainDiscriminant } = AccountAddress.parseEncoded(text);
    if (address.toI105(chainDiscriminant) !== text) {
      throw new TypeError("I105 rendering is not canonical");
    }
  } catch (error) {
    throw new TypeError(
      `${label} must contain printable ASCII or an exact canonical I105 account address`,
      { cause: error },
    );
  }
}

function validateCodecValue(record, codecField, valueField, domain = null, label = "SCCP transfer") {
  const codec = integer(record[codecField], `${label}.${codecField}`, 1, 6);
  if (!Object.prototype.hasOwnProperty.call(SCCP_CODEC_KEYS, codec)) {
    throw new TypeError(`${label}.${codecField} is unsupported or retired`);
  }
  if (domain !== null) {
    const expected = accountCodecForDomain(domain);
    if (codec !== expected) {
      throw new TypeError(`${label}.${codecField} does not match its protocol domain`);
    }
  }
  const value = exactVariableHex(record[valueField], `${label}.${valueField}`, {
    maximumBytes: 256,
  });
  const bytes = Uint8Array.from(Buffer.from(value.slice(2), "hex"));
  const nonzero = bytes.some((byte) => byte !== 0);
  if (codec === SCCP_CODEC_CANONICAL_TEXT) {
    validateCanonicalTextBytes(bytes, `${label}.${valueField}`);
  } else if (codec === SCCP_CODEC_EVM_ADDRESS20) {
    if (bytes.length !== 20 || !nonzero) {
      throw new TypeError(`${label}.${valueField} does not match evm_address20`);
    }
  } else if (codec === SCCP_CODEC_TRON_ADDRESS21) {
    if (bytes.length !== 21 || bytes[0] !== 0x41 || !bytes.slice(1).some(Boolean)) {
      throw new TypeError(`${label}.${valueField} does not match tron_address21`);
    }
  } else if (codec === SCCP_CODEC_SOLANA_PUBKEY32) {
    if (bytes.length !== 32 || !nonzero) {
      throw new TypeError(`${label}.${valueField} does not match solana_pubkey32`);
    }
  }
  return Object.freeze({ codec, bytes });
}

function parseTransfer(value, lane = null, label = "SCCP transfer") {
  const fields = new Set([
    "version",
    "source_domain",
    "dest_domain",
    "nonce",
    "route_revision",
    "asset_home_domain",
    "asset_id_codec",
    "asset_id",
    "amount",
    "sender_codec",
    "sender",
    "recipient_codec",
    "recipient",
    "route_id_codec",
    "route_id",
  ]);
  const record = exactFields(value, fields, label);
  integer(record.version, `${label}.version`, 1, 1);
  const sourceDomain = protocolDomain(record.source_domain, `${label}.source_domain`);
  const destinationDomain = protocolDomain(record.dest_domain, `${label}.dest_domain`);
  if (sourceDomain === destinationDomain) {
    throw new TypeError(`${label} source and destination domains must differ`);
  }
  if (
    lane !== null &&
    (sourceDomain !== lane.source.domain || destinationDomain !== lane.target.domain)
  ) {
    throw new TypeError(`${label} domains do not match its exact lane`);
  }
  const nonce = canonicalUnsignedDecimal(record.nonce, `${label}.nonce`, MAX_U64);
  const routeRevision = integer(record.route_revision, `${label}.route_revision`, 1, MAX_U32);
  const assetHomeDomain = protocolDomain(record.asset_home_domain, `${label}.asset_home_domain`);
  const assetId = validateCodecValue(record, "asset_id_codec", "asset_id", null, label);
  const amount = canonicalUnsignedDecimal(record.amount, `${label}.amount`, MAX_U128, {
    positive: true,
  });
  const sender = validateCodecValue(record, "sender_codec", "sender", sourceDomain, label);
  const recipient = validateCodecValue(
    record,
    "recipient_codec",
    "recipient",
    destinationDomain,
    label,
  );
  const routeId = validateCodecValue(record, "route_id_codec", "route_id", null, label);
  return Object.freeze({
    record,
    sourceDomain,
    destinationDomain,
    nonce: BigInt(nonce),
    routeRevision,
    assetHomeDomain,
    assetId,
    amount: BigInt(amount),
    sender,
    recipient,
    routeId,
  });
}

function validateTransfer(value, lane) {
  parseTransfer(value, lane);
}

/** Encode one strict `TransferPayloadV1` in the Rust-independent V1 layout. */
export function canonicalSccpTransferPayloadBytes(payload) {
  const parsed = parseTransfer(payload, null, "SCCP transfer payload");
  return concatenateBytes(
    Uint8Array.of(1),
    unsignedLittleEndian(parsed.sourceDomain, 4, "payload.source_domain"),
    unsignedLittleEndian(parsed.destinationDomain, 4, "payload.dest_domain"),
    unsignedBigIntLittleEndian(parsed.nonce, 8, "payload.nonce"),
    unsignedLittleEndian(parsed.routeRevision, 4, "payload.route_revision"),
    unsignedLittleEndian(parsed.assetHomeDomain, 4, "payload.asset_home_domain"),
    Uint8Array.of(parsed.assetId.codec),
    lengthPrefixedBytes(parsed.assetId.bytes, "payload.asset_id"),
    unsignedBigIntLittleEndian(parsed.amount, 16, "payload.amount"),
    Uint8Array.of(parsed.sender.codec),
    lengthPrefixedBytes(parsed.sender.bytes, "payload.sender"),
    Uint8Array.of(parsed.recipient.codec),
    lengthPrefixedBytes(parsed.recipient.bytes, "payload.recipient"),
    Uint8Array.of(parsed.routeId.codec),
    lengthPrefixedBytes(parsed.routeId.bytes, "payload.route_id"),
  );
}

function parsePayload(value, lane = null, label = "SCCP payload") {
  const envelope = exactFields(value, new Set(["Transfer"]), label);
  const transfer = parseTransfer(envelope.Transfer, lane, `${label}.Transfer`);
  const bytes = concatenateBytes(
    Uint8Array.of(2),
    canonicalSccpTransferPayloadBytes(envelope.Transfer),
  );
  return Object.freeze({ envelope, transfer, bytes, kind: "Transfer" });
}

/** Encode one strict externally-tagged `SccpPayloadV1`. */
export function canonicalSccpPayloadBytes(payload) {
  return parsePayload(payload).bytes;
}

/** Hash canonical SCCP payload bytes under the V1 payload role separator. */
export function sccpPayloadHash(payloadBytes) {
  return prefixedLowerHex(
    prefixedBlake2b(PAYLOAD_HASH_PREFIX, binary(payloadBytes, "SCCP payload bytes")),
  );
}

/** Hash one exact directed SCCP lane under the V1 lane role separator. */
export function sccpLaneIdHash(laneValue) {
  const lane = parseDirectedLane(laneValue, "SCCP lane");
  return prefixedLowerHex(laneHash(lane));
}

/** Derive the exact directed-lane-bound SCCP message identity. */
export function sccpMessageId(laneValue, payload) {
  const lane = parseDirectedLane(laneValue, "SCCP message lane");
  const parsed = parsePayload(payload, lane, "SCCP message payload");
  const laneBytes = canonicalLaneBytes(lane);
  const preimage = concatenateBytes(
    Uint8Array.of(1),
    lengthPrefixedBytes(laneBytes, "SCCP message lane"),
    lengthPrefixedBytes(parsed.bytes, "SCCP message payload"),
  );
  const messageId = prefixedKeccak(LANE_MESSAGE_ID_PREFIX, preimage);
  if (allZero(messageId)) throw new TypeError("SCCP message id must be nonzero");
  return prefixedLowerHex(messageId);
}

function hash32(value, label) {
  const canonical = exactLowerHex(value, label, 32, { prefix: true });
  return Uint8Array.from(Buffer.from(canonical.slice(2), "hex"));
}

function requireDistinctBinaryRoles(roles, label) {
  if (roles.some(allZero) || new Set(roles.map(lowerHexBytes)).size !== roles.length) {
    throw new TypeError(`${label} reuses a zero or colliding hash role`);
  }
}

function parseOutboundContext(value, label) {
  const record = exactFields(
    value,
    new Set(["lane", "destination_binding_hash", "route_configuration_hash"]),
    label,
  );
  const lane = parseOutboundLane(record.lane, `${label}.lane`);
  const destinationBindingHash = hash32(
    record.destination_binding_hash,
    `${label}.destination_binding_hash`,
  );
  const routeConfigurationHash = hash32(
    record.route_configuration_hash,
    `${label}.route_configuration_hash`,
  );
  requireDistinctBinaryRoles(
    [laneHash(lane), destinationBindingHash, routeConfigurationHash],
    label,
  );
  return Object.freeze({ record, lane, destinationBindingHash, routeConfigurationHash });
}

function parseHubCommitment(value, label = "SCCP hub commitment") {
  const record = exactFields(
    value,
    new Set(["version", "kind", "context", "message_id", "payload_hash"]),
    label,
  );
  integer(record.version, `${label}.version`, 1, 1);
  if (record.kind !== "Transfer") {
    throw new TypeError(`${label}.kind is unsupported or retired`);
  }
  const context = parseOutboundContext(record.context, `${label}.context`);
  const messageId = hash32(record.message_id, `${label}.message_id`);
  const payloadHash = hash32(record.payload_hash, `${label}.payload_hash`);
  requireDistinctBinaryRoles(
    [
      laneHash(context.lane),
      context.destinationBindingHash,
      context.routeConfigurationHash,
      messageId,
      payloadHash,
    ],
    label,
  );
  return Object.freeze({ record, context, messageId, payloadHash });
}

/** Build a complete outbound commitment from its governed context and payload. */
export function sccpHubCommitmentFromPayload(contextValue, payload) {
  const context = parseOutboundContext(contextValue, "SCCP outbound context");
  const parsedPayload = parsePayload(payload, context.lane, "SCCP outbound payload");
  const messageId = hash32(
    sccpMessageId(context.record.lane, payload),
    "SCCP derived message id",
  );
  const payloadHash = hash32(sccpPayloadHash(parsedPayload.bytes), "SCCP derived payload hash");
  requireDistinctBinaryRoles(
    [
      laneHash(context.lane),
      context.destinationBindingHash,
      context.routeConfigurationHash,
      messageId,
      payloadHash,
    ],
    "SCCP outbound commitment",
  );
  return deepFreezeClone({
    version: 1,
    kind: "Transfer",
    context: context.record,
    message_id: prefixedLowerHex(messageId),
    payload_hash: prefixedLowerHex(payloadHash),
  });
}

/** Encode a structurally valid `SccpHubCommitmentV1`. */
export function canonicalSccpHubCommitmentBytes(commitment) {
  const parsed = parseHubCommitment(commitment);
  return concatenateBytes(
    Uint8Array.of(1, 5, parsed.context.lane.source.tag, parsed.context.lane.target.tag),
    parsed.context.destinationBindingHash,
    parsed.context.routeConfigurationHash,
    parsed.messageId,
    parsed.payloadHash,
  );
}

/** Hash one canonical hub commitment as an SCCP Merkle leaf. */
export function sccpCommitmentLeafHash(commitment) {
  return prefixedLowerHex(
    prefixedBlake2b(HUB_LEAF_PREFIX, canonicalSccpHubCommitmentBytes(commitment)),
  );
}

function parseMerkleProof(value, label = "SCCP Merkle proof") {
  const record = exactFields(value, new Set(["steps"]), label);
  const steps = array(record.steps, `${label}.steps`);
  if (steps.length > MAX_MERKLE_PROOF_STEPS) {
    throw new TypeError(`${label} exceeds ${MAX_MERKLE_PROOF_STEPS} steps`);
  }
  return Object.freeze(
    steps.map((step, index) => {
      const stepLabel = `${label}.steps[${index}]`;
      const item = exactFields(step, new Set(["sibling_hash", "sibling_is_left"]), stepLabel);
      return Object.freeze({
        siblingHash: hash32(item.sibling_hash, `${stepLabel}.sibling_hash`),
        siblingIsLeft: boolean(item.sibling_is_left, `${stepLabel}.sibling_is_left`),
      });
    }),
  );
}

/** Encode a bounded, bottom-up `SccpMerkleProofV1`. */
export function canonicalSccpMerkleProofBytes(proof) {
  const steps = parseMerkleProof(proof);
  return concatenateBytes(
    unsignedLittleEndian(steps.length, 4, "SCCP Merkle proof step count"),
    ...steps.map((step) =>
      concatenateBytes(step.siblingHash, Uint8Array.of(step.siblingIsLeft ? 1 : 0)),
    ),
  );
}

function merkleNodeHash(left, right) {
  return prefixedBlake2b(HUB_NODE_PREFIX, concatenateBytes(left, right));
}

/** Reconstruct the canonical SCCP Merkle root for one commitment and path. */
export function sccpMerkleRootFromCommitment(commitment, proof) {
  const parsedCommitment = parseHubCommitment(commitment);
  const steps = parseMerkleProof(proof);
  let current = prefixedBlake2b(
    HUB_LEAF_PREFIX,
    canonicalSccpHubCommitmentBytes(parsedCommitment.record),
  );
  for (const step of steps) {
    current = step.siblingIsLeft
      ? merkleNodeHash(step.siblingHash, current)
      : merkleNodeHash(current, step.siblingHash);
  }
  return prefixedLowerHex(current);
}

function equalBytes(left, right) {
  return left.length === right.length && left.every((byte, index) => byte === right[index]);
}

/** Validate and encode one canonical Taira-origin SCCP message bundle. */
export function canonicalTairaSccpMessageBundleBytes(bundle) {
  const record = exactFields(
    bundle,
    new Set([
      "version",
      "commitment_root",
      "commitment",
      "merkle_proof",
      "payload",
      "finality_proof",
    ]),
    "Taira SCCP message bundle",
  );
  integer(record.version, "Taira SCCP message bundle.version", 1, 1);
  const commitmentRoot = hash32(
    record.commitment_root,
    "Taira SCCP message bundle.commitment_root",
  );
  const parsedCommitment = parseHubCommitment(
    record.commitment,
    "Taira SCCP message bundle.commitment",
  );
  const parsedPayload = parsePayload(
    record.payload,
    parsedCommitment.context.lane,
    "Taira SCCP message bundle.payload",
  );
  const expectedCommitment = sccpHubCommitmentFromPayload(
    parsedCommitment.context.record,
    parsedPayload.envelope,
  );
  const commitmentBytes = canonicalSccpHubCommitmentBytes(parsedCommitment.record);
  if (!equalBytes(commitmentBytes, canonicalSccpHubCommitmentBytes(expectedCommitment))) {
    throw new TypeError("Taira SCCP message bundle commitment does not match its payload");
  }
  const merkleProofBytes = canonicalSccpMerkleProofBytes(record.merkle_proof);
  const derivedRoot = hash32(
    sccpMerkleRootFromCommitment(parsedCommitment.record, record.merkle_proof),
    "Taira SCCP message bundle derived root",
  );
  if (!equalBytes(commitmentRoot, derivedRoot)) {
    throw new TypeError("Taira SCCP message bundle commitment root does not match its Merkle path");
  }
  requireDistinctBinaryRoles(
    [
      laneHash(parsedCommitment.context.lane),
      parsedCommitment.context.destinationBindingHash,
      parsedCommitment.context.routeConfigurationHash,
      parsedCommitment.messageId,
      parsedCommitment.payloadHash,
      commitmentRoot,
    ],
    "Taira SCCP message bundle",
  );
  const finalityHex = exactVariableHex(
    record.finality_proof,
    "Taira SCCP message bundle.finality_proof",
    { maximumBytes: MAX_FINALITY_PROOF_BYTES },
  );
  const finalityProof = Uint8Array.from(Buffer.from(finalityHex.slice(2), "hex"));
  return concatenateBytes(
    Uint8Array.of(1),
    commitmentRoot,
    lengthPrefixedBytes(commitmentBytes, "Taira SCCP message bundle.commitment"),
    lengthPrefixedBytes(merkleProofBytes, "Taira SCCP message bundle.merkle_proof"),
    lengthPrefixedBytes(parsedPayload.bytes, "Taira SCCP message bundle.payload"),
    lengthPrefixedBytes(finalityProof, "Taira SCCP message bundle.finality_proof"),
  );
}

/** Encode the six base SCCP destination-verifier public inputs. */
export function canonicalSccpMessagePublicInputsBytes(value) {
  const fields = new Set([
    "version",
    "message_id",
    "payload_hash",
    "target_domain",
    "commitment_root",
    "finality_height",
    "finality_block_hash",
  ]);
  const record = exactFields(value, fields, "SCCP message public inputs");
  integer(record.version, "SCCP message public inputs.version", 1, 1);
  const targetDomain = protocolDomain(
    record.target_domain,
    "SCCP message public inputs.target_domain",
  );
  if (targetDomain === SCCP_DOMAIN_SORA) {
    throw new TypeError("SCCP message public inputs target must be external");
  }
  const finalityHeight = BigInt(
    canonicalUnsignedDecimal(
      record.finality_height,
      "SCCP message public inputs.finality_height",
      MAX_U64,
      { positive: true },
    ),
  );
  return concatenateBytes(
    Uint8Array.of(1),
    hash32(record.message_id, "SCCP message public inputs.message_id"),
    hash32(record.payload_hash, "SCCP message public inputs.payload_hash"),
    unsignedLittleEndian(targetDomain, 4, "SCCP message public inputs.target_domain"),
    hash32(record.commitment_root, "SCCP message public inputs.commitment_root"),
    unsignedBigIntLittleEndian(
      finalityHeight,
      8,
      "SCCP message public inputs.finality_height",
    ),
    hash32(record.finality_block_hash, "SCCP message public inputs.finality_block_hash"),
  );
}

/** Normalize a raw JSON `TairaSccpMessageProofV1` bundle. */
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
  const commitmentRoot = exactLowerHex(record.commitment_root, "SCCP message bundle.commitment_root", 32, {
    prefix: true,
  });
  const commitment = exactFields(
    record.commitment,
    new Set(["version", "kind", "context", "message_id", "payload_hash"]),
    "SCCP message bundle.commitment",
  );
  integer(commitment.version, "SCCP message bundle.commitment.version", 1, 1);
  if (commitment.kind !== "Transfer") {
    throw new TypeError("SCCP message bundle commitment kind is unsupported or retired");
  }
  const context = exactFields(
    commitment.context,
    new Set(["lane", "destination_binding_hash", "route_configuration_hash"]),
    "SCCP message bundle.commitment.context",
  );
  const lane = parseOutboundLane(context.lane, "SCCP message bundle.commitment.context.lane");
  const destinationBindingHash = exactLowerHex(
    context.destination_binding_hash,
    "SCCP message bundle.commitment.context.destination_binding_hash",
    32,
    { prefix: true },
  );
  const routeConfigurationHash = exactLowerHex(
    context.route_configuration_hash,
    "SCCP message bundle.commitment.context.route_configuration_hash",
    32,
    { prefix: true },
  );
  const messageId = exactLowerHex(
    commitment.message_id,
    "SCCP message bundle.commitment.message_id",
    32,
    { prefix: true },
  );
  const payloadHash = exactLowerHex(
    commitment.payload_hash,
    "SCCP message bundle.commitment.payload_hash",
    32,
    { prefix: true },
  );
  const hashRoles = [
    commitmentRoot,
    destinationBindingHash,
    routeConfigurationHash,
    messageId,
    payloadHash,
  ];
  if (new Set(hashRoles).size !== hashRoles.length) {
    throw new TypeError("SCCP message bundle reuses role-separated commitments");
  }
  const merkle = exactFields(
    record.merkle_proof,
    new Set(["steps"]),
    "SCCP message bundle.merkle_proof",
  );
  const steps = array(merkle.steps, "SCCP message bundle.merkle_proof.steps");
  if (steps.length > 64) {
    throw new TypeError("SCCP message bundle Merkle proof exceeds 64 steps");
  }
  steps.forEach((step, index) => {
    const item = exactFields(
      step,
      new Set(["sibling_hash", "sibling_is_left"]),
      `SCCP message bundle.merkle_proof.steps[${index}]`,
    );
    exactLowerHex(
      item.sibling_hash,
      `SCCP message bundle.merkle_proof.steps[${index}].sibling_hash`,
      32,
      { prefix: true },
    );
    boolean(item.sibling_is_left, `SCCP message bundle.merkle_proof.steps[${index}].sibling_is_left`);
  });
  const payload = exactFields(
    record.payload,
    new Set(["Transfer"]),
    "SCCP message bundle.payload",
  );
  validateTransfer(payload.Transfer, lane);
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
  const targetDomain = protocolDomain(record.target_domain, `${label}.target_domain`);
  if (targetDomain === SCCP_DOMAIN_SORA) {
    throw new TypeError(`${label}.target_domain must be external`);
  }
  if (typeof record.finality_height !== "string" || !/^[1-9][0-9]*$/u.test(record.finality_height)) {
    throw new TypeError(`${label}.finality_height must be a positive canonical u64 string`);
  }
  canonicalUnsignedDecimal(record.finality_height, `${label}.finality_height`, MAX_U64, {
    positive: true,
  });
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
    "semantic_proof_profile",
    "semantic_proof_profile_hash",
    "sora_finality_anchor",
    "sora_finality_anchor_hash",
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
  const semantic = parseSemanticProofProfile(
    record.semantic_proof_profile,
    "SCCP proof request.semantic_proof_profile",
  );
  const anchor = parseSoraFinalityAnchor(
    record.sora_finality_anchor,
    "SCCP proof request.sora_finality_anchor",
  );
  requireDistinctProofPolicyRoles(semantic, anchor, "SCCP proof request outbound policy");
  const hashes = [
    "verifier_key_hash",
    "semantic_proof_profile_hash",
    "sora_finality_anchor_hash",
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
  if (`0x${lowerHexBytes(semantic.hash)}` !== record.semantic_proof_profile_hash) {
    throw new TypeError(
      "SCCP proof request semantic_proof_profile_hash does not match its typed profile",
    );
  }
  if (`0x${lowerHexBytes(anchor.hash)}` !== record.sora_finality_anchor_hash) {
    throw new TypeError(
      "SCCP proof request sora_finality_anchor_hash does not match its typed anchor",
    );
  }
  const publicHashes = [
    inputs.message_id,
    inputs.payload_hash,
    inputs.commitment_root,
    inputs.finality_block_hash,
  ];
  const commitmentRoles = [...publicHashes, ...hashes.map((field) => record[field])];
  if (new Set(commitmentRoles).size !== commitmentRoles.length) {
    throw new TypeError("SCCP proof request reuses role-separated commitments");
  }
  exactVariableHex(record.bundle_bytes, "SCCP proof request.bundle_bytes");
  return deepFreezeClone(record);
}

function validateAuthority(value, label) {
  const authority = canonicalText(value, label, 512);
  AccountAddress.fromAccountId(authority, TAIRA_I105_DISCRIMINANT);
  return authority;
}

function normalizeSccpFeePayment(value, label) {
  const record = exactFields(
    value,
    new Set(["payer", "value"]),
    label,
    new Set(["payer", "value"]),
  );
  if (record.payer !== "authority" && record.payer !== "sponsor") {
    throw new TypeError(`${label}.payer must be authority or sponsor`);
  }
  const bodyFields = new Set(["charge_limits", "gas_limit"]);
  const requiredBodyFields = new Set(["charge_limits"]);
  if (record.payer === "sponsor") {
    bodyFields.add("program_id");
    bodyFields.add("program_revision");
    requiredBodyFields.add("program_id");
    requiredBodyFields.add("program_revision");
  }
  const body = exactFields(
    record.value,
    bodyFields,
    `${label}.value`,
    requiredBodyFields,
  );
  if (!Array.isArray(body.charge_limits) || body.charge_limits.length > 2) {
    throw new TypeError(`${label}.value.charge_limits must contain at most two entries`);
  }
  let previousKind = -1;
  const chargeLimits = body.charge_limits.map((raw, index) => {
    const context = `${label}.value.charge_limits[${index}]`;
    const limit = exactFields(
      raw,
      new Set(["kind", "asset_definition_id", "max_amount"]),
      context,
      new Set(["kind", "asset_definition_id", "max_amount"]),
    );
    const taggedKind = exactFields(
      limit.kind,
      new Set(["kind", "value"]),
      `${context}.kind`,
      new Set(["kind", "value"]),
    );
    if (taggedKind.value !== null) {
      throw new TypeError(`${context}.kind.value must be null`);
    }
    const kindIndex = taggedKind.kind === "nexus"
      ? 0
      : taggedKind.kind === "pipeline_gas"
        ? 1
        : -1;
    if (kindIndex < 0 || kindIndex <= previousKind) {
      throw new TypeError(
        `${label}.value.charge_limits must be unique and ordered nexus before pipeline_gas`,
      );
    }
    previousKind = kindIndex;
    const assetDefinitionId = normalizeAssetDefinitionId(
      limit.asset_definition_id,
      `${context}.asset_definition_id`,
    );
    if (assetDefinitionId !== limit.asset_definition_id) {
      throw new TypeError(`${context}.asset_definition_id must use its exact canonical form`);
    }
    let amount;
    try {
      amount = NumericV1.decodeQuantityJson(limit.max_amount);
    } catch (error) {
      throw new TypeError(`${context}.max_amount must be a canonical positive quantity`, {
        cause: error,
      });
    }
    if (amount.mantissa <= 0n) {
      throw new TypeError(`${context}.max_amount must be positive`);
    }
    return {
      kind: { kind: taggedKind.kind, value: null },
      asset_definition_id: assetDefinitionId,
      max_amount: amount.toString(),
    };
  });
  const normalizedBody = { charge_limits: chargeLimits, gas_limit: null };
  if (body.gas_limit !== undefined && body.gas_limit !== null) {
    normalizedBody.gas_limit = integer(
      body.gas_limit,
      `${label}.value.gas_limit`,
      1,
    );
  }
  if (record.payer === "sponsor") {
    const program = exactFields(
      body.program_id,
      new Set(["sponsor", "name"]),
      `${label}.value.program_id`,
      new Set(["sponsor", "name"]),
    );
    const sponsor = validateAuthority(
      program.sponsor,
      `${label}.value.program_id.sponsor`,
    );
    const name = canonicalText(program.name, `${label}.value.program_id.name`, 255);
    if (name.normalize("NFC") !== name || /[\s@#$\/]/u.test(name)) {
      throw new TypeError(`${label}.value.program_id.name must be canonical`);
    }
    normalizedBody.program_id = { sponsor, name };
    normalizedBody.program_revision = integer(
      body.program_revision,
      `${label}.value.program_revision`,
      1,
    );
  }
  return { payer: record.payer, value: normalizedBody };
}

function detachedSigningState(record, label, creationTime) {
  const hasSignature = record.signature_b64 !== undefined;
  const hasTransactionPayload = record.transaction_payload_b64 !== undefined;
  if (hasSignature !== hasTransactionPayload) {
    throw new TypeError(
      `${label} must omit both signature_b64 and transaction_payload_b64 for preparation or provide both for signed submission`,
    );
  }
  if (!hasSignature) return {};
  if (creationTime === undefined) {
    throw new TypeError(`${label}.creation_time_ms is required for signed submission`);
  }
  canonicalBase64(record.signature_b64, `${label}.signature_b64`, {
    maximumBytes: 16 * 1024,
  });
  canonicalBase64(record.transaction_payload_b64, `${label}.transaction_payload_b64`);
  return {
    signature_b64: record.signature_b64,
    transaction_payload_b64: record.transaction_payload_b64,
  };
}

/** Build the sole supported SORA-origin destination-proof submission. */
export function normalizeBridgeProofSubmitPayload(value) {
  const record = exactFields(
    value,
    new Set([
      "authority",
      "fee_payment",
      "signature_b64",
      "transaction_payload_b64",
      "destination_proof_b64",
      "creation_time_ms",
    ]),
    "bridge proof submit",
    new Set(["authority", "fee_payment", "destination_proof_b64"]),
  );
  canonicalNoritoBase64(
    record.destination_proof_b64,
    "bridge proof submit.destination_proof_b64",
    DESTINATION_PROOF_NORITO_TYPE,
    MAX_DESTINATION_ARTIFACT_BYTES,
  );
  const creationTime =
    record.creation_time_ms === undefined
      ? undefined
      : integer(
          record.creation_time_ms,
          "bridge proof submit.creation_time_ms",
          1,
        );
  const result = {
    authority: validateAuthority(record.authority, "bridge proof submit.authority"),
    fee_payment: normalizeSccpFeePayment(
      record.fee_payment,
      "bridge proof submit.fee_payment",
    ),
    ...detachedSigningState(record, "bridge proof submit", creationTime),
    destination_proof_b64: record.destination_proof_b64,
  };
  if (creationTime !== undefined) result.creation_time_ms = creationTime;
  return Object.freeze(result);
}

/** Build the sole supported native inbound message submission. */
export function normalizeBridgeMessageSubmitPayload(value) {
  const record = exactFields(
    value,
    new Set([
      "authority",
      "fee_payment",
      "signature_b64",
      "transaction_payload_b64",
      "native_proof_b64",
      "creation_time_ms",
    ]),
    "bridge message submit",
    new Set(["authority", "fee_payment", "native_proof_b64"]),
  );
  canonicalNoritoBase64(
    record.native_proof_b64,
    "bridge message submit.native_proof_b64",
    NATIVE_MESSAGE_PROOF_NORITO_TYPE,
    MAX_WIRE_BYTES,
  );
  const creationTime =
    record.creation_time_ms === undefined
      ? undefined
      : integer(
          record.creation_time_ms,
          "bridge message submit.creation_time_ms",
          1,
        );
  const result = {
    authority: validateAuthority(record.authority, "bridge message submit.authority"),
    fee_payment: normalizeSccpFeePayment(
      record.fee_payment,
      "bridge message submit.fee_payment",
    ),
    ...detachedSigningState(record, "bridge message submit", creationTime),
    native_proof_b64: record.native_proof_b64,
  };
  if (creationTime !== undefined) result.creation_time_ms = creationTime;
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
    route_configuration_hash_hex: exactLowerHex(
      record.route_configuration_hash_hex,
      "route_configuration_hash_hex",
      32,
    ),
    range_start_height: rangeStart,
    range_end_height: rangeEnd,
    creation_time_ms: creationTime,
    tx_hash_hex: txHash,
    transaction_payload_b64: record.transaction_payload_b64,
    signing_message_b64: record.signing_message_b64,
  });
  const expected = exactFields(
    expectations,
    new Set(["submitted", "creation_time_ms"]),
    "bridge response expectations",
    new Set(),
  );
  if (
    expected.creation_time_ms !== undefined &&
    result.creation_time_ms !== expected.creation_time_ms
  ) {
    throw new TypeError("bridge submit response.creation_time_ms does not match the request");
  }
  if (expected.submitted !== undefined) {
    boolean(expected.submitted, "bridge response expectations.submitted");
    if (result.submitted !== expected.submitted) {
      throw new TypeError("bridge submit response.submitted does not match the request signing state");
    }
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
    // Every numeric field in the closed SCCP V1 JSON surface is an unsigned
    // integer. Preserve its exact wire meaning by rejecting signs, fractions,
    // exponents, and leading zeroes before JSON.parse can coerce them.
    const match = /^(?:(?:0|[1-9][0-9]*)|true|false|null)/u.exec(
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
