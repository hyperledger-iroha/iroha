// Authoritative standalone governance JSON. These projections do not authorize ballots.
import { NumericV1 } from "./numericV1.js";

const U64 = (1n << 64n) - 1n;
const U128 = (1n << 128n) - 1n;
const SAFE = BigInt(Number.MAX_SAFE_INTEGER);
const POLICY_FIELDS = [
  "asset_definition_id", "asset_scale", "conviction_step_blocks", "max_conviction",
  "approval_threshold_numerator", "approval_threshold_denominator", "minimum_turnout",
  "minimum_bond", "bond_escrow_account", "slash_receiver_account",
];

function object(value, context) {
  if (value === null || typeof value !== "object" || Array.isArray(value)
    || ![Object.prototype, null].includes(Object.getPrototypeOf(value))) {
    throw new TypeError(`${context} must be an object`);
  }
  for (const key of Reflect.ownKeys(value)) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (typeof key !== "string" || !descriptor.enumerable || !("value" in descriptor)) {
      throw new TypeError(`${context} must contain only enumerable data fields`);
    }
  }
  return value;
}

function exact(value, fields, context) {
  const record = object(value, context);
  if (Object.keys(record).length !== fields.length
    || fields.some((field) => !Object.hasOwn(record, field))) {
    throw new TypeError(`${context} must contain exactly ${fields.join(", ")}`);
  }
  return record;
}

function unsigned(value, maximum, context) {
  if (!(typeof value === "bigint"
    || (typeof value === "number" && Number.isSafeInteger(value) && !Object.is(value, -0)))) {
    throw new TypeError(`${context} must be an unsigned JSON integer token`);
  }
  const integer = BigInt(value);
  if (integer < 0n || integer > maximum) throw new RangeError(`${context} exceeds its unsigned bound`);
  return integer <= SAFE ? Number(integer) : integer;
}

function text(value, context) {
  if (typeof value !== "string" || value.length === 0 || /\s|[\u0000-\u001f\u007f]/u.test(value)) {
    throw new TypeError(`${context} must be a non-empty exact token`);
  }
  return value;
}

function quantity(value, context) {
  try {
    return NumericV1.decodeQuantityJson(value);
  } catch (cause) {
    throw new TypeError(`${context} must be a canonical non-negative Kotodama V1 quantity`, { cause });
  }
}

function policy(value) {
  const context = "governance plain_context.content";
  const record = exact(value, POLICY_FIELDS, context);
  const result = {
    asset_definition_id: text(record.asset_definition_id, `${context}.asset_definition_id`),
    asset_scale: unsigned(record.asset_scale, BigInt(NumericV1.MAX_SCALE), `${context}.asset_scale`),
  };
  for (const field of ["conviction_step_blocks", "max_conviction",
    "approval_threshold_numerator", "approval_threshold_denominator"]) {
    result[field] = unsigned(record[field], U64, `${context}.${field}`);
  }
  if (result.conviction_step_blocks === 0 || result.max_conviction === 0
    || result.approval_threshold_denominator === 0
    || BigInt(result.approval_threshold_numerator) > BigInt(result.approval_threshold_denominator)) {
    throw new RangeError(`${context} has invalid frozen conviction parameters`);
  }
  result.minimum_turnout = unsigned(record.minimum_turnout, U128, `${context}.minimum_turnout`);
  const minimum = quantity(record.minimum_bond, `${context}.minimum_bond`);
  if (minimum.scale > result.asset_scale
    || minimum.mantissa * 10n ** BigInt(result.asset_scale - minimum.scale) > U128) {
    throw new RangeError(`${context}.minimum_bond is not representable in frozen u128 units`);
  }
  result.minimum_bond = minimum.toString();
  result.bond_escrow_account = text(record.bond_escrow_account, `${context}.bond_escrow_account`);
  result.slash_receiver_account = text(record.slash_receiver_account, `${context}.slash_receiver_account`);
  return result;
}

function counts(record, context) {
  const result = {};
  for (const field of ["approve", "reject", "abstain"]) {
    result[field] = unsigned(record[field], U128, `${context}.${field}`);
  }
  if (BigInt(result.approve) + BigInt(result.reject) + BigInt(result.abstain) > U128) {
    throw new RangeError(`${context} aggregate exceeds u128`);
  }
  return result;
}

function referendum(value) {
  const record = exact(value,
    ["h_start", "h_end", "status", "mode", "plain_context", "plain_result"],
    "governance referendum");
  const result = {
    h_start: unsigned(record.h_start, U64, "governance referendum.h_start"),
    h_end: unsigned(record.h_end, U64, "governance referendum.h_end"),
    status: record.status,
    mode: record.mode,
  };
  if (!["Proposed", "Open", "Closed"].includes(result.status)) {
    throw new TypeError("governance referendum.status is unknown");
  }
  const context = exact(record.plain_context, ["kind", "content"], "governance plain_context");
  const outcome = exact(record.plain_result, ["kind", "content"], "governance plain_result");
  if (result.mode === "Zk") {
    if (context.kind !== "NotApplicable" || context.content !== null
      || outcome.kind !== "NotApplicable" || outcome.content !== null) {
      throw new TypeError("Zk referendum requires NotApplicable context and result");
    }
    result.plain_context = { kind: "NotApplicable", content: null };
    result.plain_result = { kind: "NotApplicable", content: null };
    return result;
  }
  if (result.mode !== "Plain" || context.kind !== "Conviction") {
    throw new TypeError("Plain referendum requires its frozen Conviction context");
  }
  const frozen = policy(context.content);
  result.plain_context = { kind: "Conviction", content: frozen };
  if (result.status !== "Closed") {
    if (outcome.kind !== "Pending" || outcome.content !== null) {
      throw new TypeError("open or proposed Plain referendum requires Pending result");
    }
    result.plain_result = { kind: "Pending", content: null };
    return result;
  }
  if (outcome.kind !== "Decided") throw new TypeError("closed Plain referendum requires Decided result");
  const decision = exact(outcome.content, ["approve", "reject", "abstain", "approved"], "governance decision");
  const tally = counts(decision, "governance decision");
  const decisive = BigInt(tally.approve) + BigInt(tally.reject);
  const approved = decisive + BigInt(tally.abstain) >= BigInt(frozen.minimum_turnout)
    && decisive !== 0n
    && BigInt(tally.approve) * BigInt(frozen.approval_threshold_denominator)
      >= decisive * BigInt(frozen.approval_threshold_numerator);
  if (decision.approved !== approved) throw new TypeError("closed decision differs from its frozen policy");
  result.plain_result = { kind: "Decided", content: { ...tally, approved } };
  return result;
}

/** Decode the exact native referendum response, including its required frozen context/result. */
export function parseGovernanceReferendumResponseV1(value) {
  const record = object(value, "governance referendum response");
  if (typeof record.found !== "boolean") throw new TypeError("governance referendum.found must be boolean");
  exact(record, record.found ? ["found", "referendum"] : ["found"], "governance referendum response");
  return record.found ? { found: true, referendum: referendum(record.referendum) } : { found: false };
}

/** Decode a bounded exact tally while retaining the authoritative evaluated block coordinates. */
export function parseGovernanceTallyResponseV1(value, expectedId) {
  const record = exact(value, ["referendum_id", "evaluated_block_height", "evaluated_block_hash",
    "approve", "reject", "abstain"], "governance tally");
  if (text(record.referendum_id, "governance tally.referendum_id") !== expectedId) {
    throw new TypeError("governance tally referendum_id differs from request");
  }
  const height = unsigned(record.evaluated_block_height, U64, "governance tally.evaluated_block_height");
  const hash = record.evaluated_block_hash;
  if (typeof hash !== "string" || !/^[0-9a-f]{64}$/u.test(hash)
    || (height === 0) !== (hash === "0".repeat(64))) {
    throw new TypeError("governance tally has invalid evaluated block coordinates");
  }
  return { referendum_id: expectedId, evaluated_block_height: height,
    evaluated_block_hash: hash, ...counts(record, "governance tally") };
}

function lock(value, owner) {
  const context = `governance locks[${owner}]`;
  const record = exact(value, ["owner", "amount", "slashed", "expiry_height",
    "direction", "duration_blocks", "custody"], context);
  if (text(record.owner, `${context}.owner`) !== owner) {
    throw new TypeError(`${context}.owner differs from its map key`);
  }
  const custody = exact(record.custody,
    ["escrowed", "asset_definition_id", "bond_escrow_account", "slash_receiver_account"], `${context}.custody`);
  if (typeof custody.escrowed !== "boolean") throw new TypeError(`${context}.custody.escrowed must be a boolean`);
  return {
    owner,
    amount: quantity(record.amount, `${context}.amount`).toString(),
    slashed: quantity(record.slashed, `${context}.slashed`).toString(),
    expiry_height: unsigned(record.expiry_height, U64, `${context}.expiry_height`),
    direction: unsigned(record.direction, 255n, `${context}.direction`),
    duration_blocks: unsigned(record.duration_blocks, U64, `${context}.duration_blocks`),
    custody: {
      escrowed: custody.escrowed,
      asset_definition_id: text(custody.asset_definition_id, `${context}.custody.asset_definition_id`),
      bond_escrow_account: text(custody.bond_escrow_account, `${context}.custody.bond_escrow_account`),
      slash_receiver_account: text(custody.slash_receiver_account, `${context}.custody.slash_receiver_account`),
    },
  };
}

/** Decode the native response's nested GovernanceLocksForReferendum object without old layouts. */
export function parseGovernanceLocksResponseV1(value, expectedId) {
  const record = object(value, "governance locks response");
  if (typeof record.found !== "boolean") throw new TypeError("governance locks.found must be boolean");
  exact(record, record.found ? ["found", "referendum_id", "locks"] : ["found", "referendum_id"], "governance locks response");
  if (text(record.referendum_id, "governance locks.referendum_id") !== expectedId) {
    throw new TypeError("governance locks referendum_id differs from request");
  }
  if (!record.found) return { found: false, referendum_id: expectedId };
  const wrapper = exact(record.locks, ["locks"], "governance locks corpus");
  const entries = Object.entries(object(wrapper.locks, "governance locks map"));
  const locks = Object.fromEntries(entries.map(([owner, value]) => [owner, lock(value, owner)]));
  return { found: true, referendum_id: expectedId, locks: { locks } };
}
