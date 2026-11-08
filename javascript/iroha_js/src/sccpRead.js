// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

// Native JSON contracts: iroha_sccp::api::{SccpCapabilitiesV1,
// SccpRecentMessagesV1} and iroha_data_model::sccp::registry::SccpRouteV1.
// These are public metadata reads, not certificate verification or write authority.
import { parseCanonicalI105AccountLiteral } from "./address.js";

export const SCCP_CAPABILITIES_MAX_BYTES = 256 * 1024;
export const SCCP_REGISTRY_MAX_BYTES = 4 * 1024 * 1024;
export const SCCP_RECENT_MAX_BYTES = 1024 * 1024;
export const SCCP_RECENT_MAX_ITEMS = 50;

const EXTERNAL_NETWORKS = Object.freeze({
  "ethereum-mainnet": "ethereum_mainnet",
  "bsc-mainnet": "bsc_mainnet",
  "ton-mainnet": "ton_mainnet",
});
const ROUTE_IDS = Object.freeze({
  ethereum_mainnet: "taira_eth_xor",
  bsc_mainnet: "taira_bsc_xor",
  ton_mainnet: "taira_ton_xor",
});

function fail(context, expected) {
  throw new TypeError(`${context} must be ${expected}`);
}
function object(value, context) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(context, "an object");
  }
}
function fields(shape) {
  return (value, context) => {
    object(value, context);
    for (const key of Object.keys(value)) {
      if (!Object.hasOwn(shape, key)) fail(`${context}.${key}`, "absent (unknown field)");
    }
    for (const [key, validate] of Object.entries(shape)) {
      if (!Object.hasOwn(value, key)) fail(`${context}.${key}`, "present");
      validate(value[key], `${context}.${key}`);
    }
  };
}
function nullable(validate) {
  return (value, context) => { if (value !== null) validate(value, context); };
}
function array(validate, maximum) {
  return (value, context) => {
    if (!Array.isArray(value) || value.length > maximum) {
      fail(context, `an array of at most ${maximum} entries`);
    }
    value.forEach((entry, index) => validate(entry, `${context}[${index}]`));
  };
}
function uint(bits) {
  const maximum = (1n << BigInt(bits)) - 1n;
  return (value, context) => {
    if (!((typeof value === "number" && Number.isSafeInteger(value)) ||
      typeof value === "bigint") || value < 0 || BigInt(value) > maximum) {
      fail(context, `an unsigned ${bits}-bit JSON integer`);
    }
  };
}
const u16 = uint(16);
const u32 = uint(32);
const u64 = uint(64);
const u128 = uint(128);
function u128Text(value, context) {
  if (typeof value !== "string" || !/^(?:0|[1-9][0-9]{0,38})$/.test(value) ||
    BigInt(value) > (1n << 128n) - 1n) fail(context, "canonical u128 decimal text");
}
function quantity(value, context) {
  // Native Quantity shares the signed 512-bit mantissa and scale <= 28 domain.
  if (typeof value !== "string" || value.length > 155 ||
    !/^(?:0|[1-9][0-9]*)(?:\.[0-9]{0,27}[1-9])?$/.test(value) ||
    value.replace(".", "").length > 154 || BigInt(value.replace(".", "")) >= (1n << 511n)) {
    fail(context, "canonical nonnegative Quantity text within the native domain");
  }
}
function text(value, context) {
  if (typeof value !== "string" || value.length === 0) fail(context, "nonempty text");
}
function bool(value, context) {
  if (typeof value !== "boolean") fail(context, "a boolean");
}
function nil(value, context) {
  if (value !== null) fail(context, "null");
}
function hex(bytes) {
  const pattern = new RegExp(`^[0-9a-f]{${bytes * 2}}$`);
  return (value, context) => {
    if (typeof value !== "string" || !pattern.test(value)) {
      fail(context, `${bytes}-byte lowercase hex text`);
    }
  };
}
const hash = hex(32);
function account(value, context) {
  text(value, context);
  parseCanonicalI105AccountLiteral(value);
}
function tagged(tag, content, variants) {
  return (value, context) => {
    object(value, context);
    const variant = value[tag];
    if (typeof variant !== "string" || !Object.hasOwn(variants, variant)) {
      fail(`${context}.${tag}`, `one of ${Object.keys(variants).join(", ")}`);
    }
    fields({ [tag]: text, [content]: variants[variant] })(value, context);
  };
}
function units(tag, variants, content = "detail") {
  return tagged(tag, content, Object.fromEntries(variants.map((variant) => [variant, nil])));
}
const network = units("network", Object.values(EXTERNAL_NETWORKS), "profile");
const pendingReason = units("reason", ["disabled", "revision_not_settleable",
  "liability_shortfall", "credit_refused", "fee_sink_unavailable", "block_leaves_full",
  "held", "refund_deferred"]);
const statusHeight = fields({ height: u64 });
const voidStatus = fields({
  kind: units("kind", ["expired", "frozen"]),
  proven_at_height: u64,
  refund_pending: nullable(pendingReason),
});
const outboundStatus = tagged("status", "detail", {
  recorded: nil, voided: voidStatus, refunded: statusHeight, stranded: statusHeight,
});
const outboundState = tagged("state", "detail", {
  recorded: fields({ deadline_ms: u64, provable: bool }),
  voided: voidStatus, refunded: statusHeight, stranded: statusHeight,
});
const settled = fields({ height: u64, payout: u128 });
const inboundStatus = tagged("status", "detail", {
  pending: fields({ reason: pendingReason }), released: settled, stranded: settled,
  bounced: fields({ bounce_message_id: hash, payout: u128 }),
});
function payload(value, context) {
  // RFC 4648 padded standard base64, bounded by SCCP MAX_PAYLOAD_BYTES (4096).
  if (typeof value !== "string" || value.length > 5464 ||
    !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) {
    fail(context, "canonical padded base64 of at most 4096 bytes");
  }
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  const padding = value.endsWith("==") ? 2 : value.endsWith("=") ? 1 : 0;
  if (value.length / 4 * 3 - padding > 4096 ||
    (padding === 2 && (alphabet.indexOf(value.at(-3)) & 15) !== 0) ||
    (padding === 1 && (alphabet.indexOf(value.at(-2)) & 3) !== 0)) {
    fail(context, "canonical padded base64 of at most 4096 bytes");
  }
}
const outboundRecord = fields({
  network, revision: u32, nonce: u64, height: u64, commitment_index: u32,
  deadline_ms: u64, sender: account, amount: u128Text, payload, leaf: hash,
  status: outboundStatus,
});
const inboundRecord = fields({
  network, revision: u32, payload,
  source_locator: fields({ source_height: u64, block_hash: hash, index_in_block: u32 }),
  proven_at_height: u64, fee_due: u128Text, status: inboundStatus,
});
const message = tagged("direction", "detail", {
  outbound: fields({ message_id: hash, record: outboundRecord, state: outboundState }),
  inbound: fields({ message_id: hash, record: inboundRecord }),
});
const parameters = fields({
  enabled: bool, committee_validity_ms: u64, committee_heartbeat_ms: u64,
  forgery_evidence_grace_ms: u64, destination_progress_cap_ms: u64, outbound_ttl_ms: u64,
  min_outbound_amount: u128Text, inbound_self_claim_fee: u128Text,
  max_exempt_transactions_per_block: u32, fast_pause_panel_seats: u32,
  fast_pause_panel_term_ms: u64, fast_pause_seat_accept_window_ms: u64,
  fast_pause_endorse_window_ms: u64, fast_pause_renew_lead_ms: u64, fast_pause_hold_ms: u64,
});
const capabilities = fields({
  network_id: hash, consensus_instance: hash,
  finality: fields({ x_layout_id: hex(16), dst_sig: hex(43), result_tag: hex(24), result_body_tag: hex(29) }),
  current_generation: nullable(fields({ generation: u64, committee_root: hash, start_height: u64, deadline_ms: u64 })),
  coalition_slashable_floor: nullable(quantity), liability_clock: u64,
  forgery_max_generation: nullable(u64),
  pause_panel_ids: nullable(fields({ primary: hash, backup: hash })),
  parliament_binding_ballot_available: bool, reconciliation_available: bool,
  parameters: nullable(parameters), committed_height: u64,
  light_client_profiles: array(fields({
    network, version: u32, profile_hash: hash, activation_height: nullable(u64),
    proposal_id: nullable(hash), supported_until_ms: nullable(u64),
  }), 3),
  chain_id: text,
  profiles: array(fields({ network, supported_until_ms: nullable(u64) }), 3),
  profiles_policy_hash: hash,
  limits: fields({ recent_messages: u32, outbound_page: u32, controls_page: u32, rotations_page: u32 }),
  path_templates: array(text, 128),
});
const tonCode = fields({ hash, depth: u16 });
const deployment = tagged("family", "deployment", {
  evm: fields({ address: hex(20), runtime_code_hash: hash }),
  ton: fields({ master_account: hash, minter_code: tonCode, wallet_code: tonCode, bucket_code: tonCode }),
});
const revision = fields({
  revision: u32, deployment, destination_word: hash,
  initial_committee: fields({ generation: u64, committee_root: hash, start_height: u64, until_ms: u64 }),
  activation: units("activation", ["staged", "bidirectional", "paused", "inbound_only", "retired"]),
  destination_frozen: bool, destination_paused: bool, next_control_nonce: u64,
  max_wrapped_supply: u128Text, liability: u128Text, next_outbound_nonce: u64,
  registered_at_height: u64, forgery_floor: u64,
  quarantine: nullable(fields({ certificate_id: hash, since_height: u64 })), ever_activated: bool,
});
function revisions(value, context) {
  object(value, context);
  for (const [key, entry] of Object.entries(value)) {
    if (!/^[1-9][0-9]{0,9}$/.test(key) || BigInt(key) > 0xffffffffn) {
      fail(`${context}.${key}`, "a nonzero u32 revision key");
    }
    revision(entry, `${context}.${key}`);
    if (BigInt(entry.revision) !== BigInt(key)) fail(`${context}.${key}.revision`, "equal to its map key");
  }
}
const route = fields({ network, route_id: text, escrow: account, stranded: u128Text, revisions });

function uniqueNetworks(values, context) {
  const seen = new Set();
  for (const value of values) {
    const name = value.network.network;
    if (seen.has(name)) fail(context, "free of duplicate networks");
    seen.add(name);
  }
}

/** Check the exact current capability record without treating it as an authenticated trust anchor. */
export function normalizeSccpCapabilitiesV1(value) {
  const context = "SCCP capabilities response";
  capabilities(value, context);
  uniqueNetworks(value.profiles, `${context}.profiles`);
  uniqueNetworks(value.light_client_profiles, `${context}.light_client_profiles`);
  return value;
}

/** Check native registry records and preserve their original snake_case JSON. */
export function normalizeSccpRegistryV1(value) {
  const context = "SCCP registry response";
  array(route, 3)(value, context);
  uniqueNetworks(value, context);
  for (const entry of value) {
    if (entry.route_id !== ROUTE_IDS[entry.network.network]) fail(`${context}.route_id`, "the current network route id");
    for (const item of Object.values(entry.revisions)) {
      const family = entry.network.network === "ton_mainnet" ? "ton" : "evm";
      if (item.deployment.family !== family) fail(`${context}.deployment.family`, "the network's deployment family");
    }
  }
  return value;
}

function cursor(value, direction, context) {
  if (typeof value !== "string" || value.length > 85) fail(context, "a bounded recent-message cursor");
  const parts = value.split(":");
  if (parts.length !== 2 || !/^(?:0|[1-9][0-9]{0,19})$/.test(parts[0])) {
    fail(context, "a canonical height:position cursor");
  }
  u64(BigInt(parts[0]), context);
  if (direction === "outbound") {
    if (!/^(?:0|[1-9][0-9]{0,9})$/.test(parts[1])) fail(context, "a canonical outbound cursor");
    u32(BigInt(parts[1]), context);
  } else {
    hash(parts[1], context);
  }
}

/** Validate options before transport; the native route admits at most 50 records. */
export function sccpRecentParams(options) {
  const direction = options.direction === undefined ? "outbound" : options.direction;
  if (direction !== "outbound" && direction !== "inbound") fail("SCCP direction", "outbound or inbound");
  const limit = options.limit === undefined ? SCCP_RECENT_MAX_ITEMS : options.limit;
  if (!Number.isInteger(limit) || limit < 1 || limit > SCCP_RECENT_MAX_ITEMS) fail("SCCP limit", "an integer in 1..50");
  const params = { direction, limit };
  if (options.network !== undefined) {
    if (typeof options.network !== "string" || !Object.hasOwn(EXTERNAL_NETWORKS, options.network)) {
      fail("SCCP network", "a current external profile key");
    }
    params.network = options.network;
  }
  if (options.before !== undefined) {
    cursor(options.before, direction, "SCCP before");
    params.before = options.before;
  }
  return params;
}

/** Check the requested direction, network, page bound, native rows and cursor. */
export function normalizeSccpRecentMessagesV1(value, params) {
  const context = "SCCP recent messages response";
  fields({ messages: array(message, params.limit), next_before: nullable(text) })(value, context);
  if (value.next_before !== null) cursor(value.next_before, params.direction, `${context}.next_before`);
  const seen = new Set();
  for (const entry of value.messages) {
    if (entry.direction !== params.direction) fail(`${context}.direction`, "the requested direction");
    if (params.network !== undefined && entry.detail.record.network.network !== EXTERNAL_NETWORKS[params.network]) {
      fail(`${context}.network`, "the requested network");
    }
    if (seen.has(entry.detail.message_id)) fail(`${context}.messages`, "free of duplicate message ids");
    seen.add(entry.detail.message_id);
  }
  return value;
}
