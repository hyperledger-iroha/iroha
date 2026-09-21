import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  parseGovernanceReferendumResponseV1,
  parseGovernanceTallyResponseV1,
  parseGovernanceLocksResponseV1,
} from "../src/governancePlainV1.js";
import {
  parseStrictLosslessIntegerJson,
  stringifyStrictLosslessIntegerJson,
} from "../src/strictLosslessJson.js";

const U64 = (1n << 64n) - 1n;
const U128 = (1n << 128n) - 1n;
const fixtures = new URL("../../../fixtures/governance/plain_v1/", import.meta.url);
const read = (name) => readFileSync(new URL(name, fixtures), "utf8");
const parse = (raw) => parseStrictLosslessIntegerJson(raw, "governance fixture");
const fixture = (name) => parse(read(`${name}.json`));
const assertWireEqual = (actual, expected) => assert.deepEqual(
  parse(stringifyStrictLosslessIntegerJson(actual, "actual governance value")),
  parse(stringifyStrictLosslessIntegerJson(expected, "expected governance value")),
);
const decoders = {
  referendum: parseGovernanceReferendumResponseV1,
  tally: (value) => parseGovernanceTallyResponseV1(value, "ref-1"),
  locks: (value) => parseGovernanceLocksResponseV1(value, "ref-1"),
};

for (const entry of JSON.parse(read("cases.json")).cases) {
  test(`shared raw governance vector: ${entry.file}`, () => {
    const value = parse(read(entry.file));
    if (entry.valid) {
      const decoded = decoders[entry.kind](value);
      assertWireEqual(decoded, value);
      assert.deepEqual(parse(stringifyStrictLosslessIntegerJson(decoded, "decoded governance value")), value);
    } else {
      assert.throws(() => decoders[entry.kind](value), /governance|unsigned/u);
    }
  });
}

test("unquoted u64/u128 wire tokens preserve every bit and authoritative coordinates", () => {
  const max = decoders.tally(fixture("tally-max"));
  assert.equal(max.approve, U128);
  assert.equal(max.evaluated_block_height, U64);
  assert.equal(max.evaluated_block_hash, "12".repeat(32));
  const large = decoders.tally(fixture("tally-large"));
  assert.equal(large.approve, 18446744073709551617n);
  assert.equal(large.reject, 9007199254740993n);
  assert.equal(large.evaluated_block_height, 9007199254740993n);
  assert.equal(large.abstain, 0);
  assert.equal(decoders.referendum(fixture("referendum-max")).referendum.plain_result.content.approve, U128);
  const lock = Object.values(decoders.locks(fixture("locks")).locks.locks)[0];
  assert.equal(lock.expiry_height, U64);
  assert.equal(lock.duration_blocks, U64 - 1n);
  assert.equal(lock.amount, "18446744073709551616.25");
});

function rejectMutation(name, kind, baseline, mutate, pattern) {
  test(name, () => {
    const value = fixture(baseline);
    // Establish that the intended field change is the sole invalidity.
    assert.doesNotThrow(() => decoders[kind](value));
    mutate(value);
    assert.throws(() => decoders[kind](value), pattern);
  });
}

for (const status of ["Proposed", "Open", "Closed"]) {
  test(`Zk ${status} retains both explicit NotApplicable variants`, () => {
    const value = fixture("referendum-zk");
    value.referendum.status = status;
    assertWireEqual(decoders.referendum(value), value);
  });
}
for (const status of ["Proposed", "Open"]) {
  test(`Plain ${status} retains frozen policy and explicit Pending result`, () => {
    const value = fixture("referendum-open");
    value.referendum.status = status;
    assertWireEqual(decoders.referendum(value), value);
  });
}
for (const [approve, reject, abstain, minimum, approved] of [
  [2, 1, 7, 10, true], [2, 1, 6, 10, false], [1, 2, 7, 10, false],
  [0, 0, 10, 10, false], [0, 0, 0, 0, false], [U128, 0, 0, U128, true],
]) {
  test(`closed decision checks frozen threshold and turnout: ${approve}/${reject}/${abstain}/${minimum}`, () => {
    const value = fixture("referendum-closed");
    value.referendum.plain_context.content.minimum_turnout = minimum;
    value.referendum.plain_result.content = { approve, reject, abstain, approved };
    assertWireEqual(decoders.referendum(value), value);
    value.referendum.plain_result.content.approved = !approved;
    assert.throws(() => decoders.referendum(value), /differs from its frozen policy/u);
  });
}

for (const field of ["h_start", "h_end", "status", "mode", "plain_context", "plain_result"]) {
  rejectMutation(`referendum requires ${field}`, "referendum", "referendum-open",
    (value) => { delete value.referendum[field]; }, /must contain exactly/u);
}
for (const field of Object.keys(fixture("referendum-open").referendum.plain_context.content)) {
  rejectMutation(`frozen policy requires ${field}`, "referendum", "referendum-open",
    (value) => { delete value.referendum.plain_context.content[field]; }, /must contain exactly/u);
}
for (const [field, invalid, error] of [
  ["asset_scale", 29, /unsigned bound/u],
  ["asset_scale", "2", /integer token/u],
  ["conviction_step_blocks", 0, /conviction parameters/u],
  ["max_conviction", 0, /conviction parameters/u],
  ["approval_threshold_denominator", 0, /conviction parameters/u],
  ["approval_threshold_numerator", 4, /conviction parameters/u],
  ["minimum_turnout", U128 + 1n, /unsigned bound/u],
  ["minimum_bond", "0.001", /frozen u128 units/u],
  ["minimum_bond", U128.toString(), /frozen u128 units/u],
  ["minimum_bond", "0.250", /canonical/u],
  ["minimum_bond", 0, /canonical/u],
  ["bond_escrow_account", " account", /exact token/u],
  ["slash_receiver_account", "", /exact token/u],
  ["asset_definition_id", "asset\n", /exact token/u],
]) {
  rejectMutation(`frozen policy rejects ${field}=${String(invalid)}`, "referendum", "referendum-open",
    (value) => { value.referendum.plain_context.content[field] = invalid; }, error);
}
test("frozen minimum accepts the exact u128 boundary at the frozen scale", () => {
  const value = fixture("referendum-open");
  value.referendum.plain_context.content.asset_scale = 0;
  value.referendum.plain_context.content.minimum_bond = U128.toString();
  assertWireEqual(decoders.referendum(value), value);
});
for (const [label, change, pattern] of [
  ["unknown status", (r) => { r.status = "Passed"; }, /status is unknown/u],
  ["unknown mode", (r) => { r.mode = "PLAIN"; }, /requires its frozen/u],
  ["missing context content", (r) => { delete r.plain_context.content; }, /must contain exactly/u],
  ["old context string", (r) => { r.plain_context = "Conviction"; }, /must be an object/u],
  ["wrong context variant", (r) => { r.plain_context.kind = "NotApplicable"; }, /requires its frozen/u],
  ["missing result content", (r) => { delete r.plain_result.content; }, /must contain exactly/u],
  ["non-null Pending content", (r) => { r.plain_result.content = {}; }, /requires Pending/u],
  ["closed pending", (r) => { r.status = "Closed"; }, /requires Decided/u],
  ["Zk with Plain policy", (r) => { r.mode = "Zk"; }, /requires NotApplicable/u],
  ["extra field", (r) => { r.live_policy = {}; }, /must contain exactly/u],
]) {
  rejectMutation(`referendum rejects ${label}`, "referendum", "referendum-open",
    (value) => change(value.referendum), pattern);
}
rejectMutation("closed result rejects aggregate u128 overflow", "referendum", "referendum-max",
  (v) => { v.referendum.plain_result.content.abstain = 1; }, /aggregate exceeds u128/u);
rejectMutation("closed result requires boolean approved", "referendum", "referendum-closed",
  (v) => { v.referendum.plain_result.content.approved = "true"; }, /differs from its frozen policy/u);
rejectMutation("open referendum cannot carry Decided result", "referendum", "referendum-closed",
  (v) => { v.referendum.status = "Open"; }, /requires Pending/u);
rejectMutation("Zk cannot carry a PLAIN result", "referendum", "referendum-zk",
  (v) => { v.referendum.plain_result.kind = "Pending"; }, /requires NotApplicable/u);

for (const kind of ["referendum", "locks"]) {
  rejectMutation(`${kind} rejects non-boolean found`, kind, `${kind}-missing`,
    (v) => { v.found = "false"; }, /must be boolean/u);
  rejectMutation(`${kind} notfound rejects null record instead of omitted field`, kind, `${kind}-missing`,
    (v) => { v[kind] = null; }, /must contain exactly/u);
}
for (const field of ["owner", "amount", "slashed", "expiry_height", "direction", "duration_blocks", "custody"]) {
  rejectMutation(`lock requires ${field}`, "locks", "locks",
    (v) => { delete Object.values(v.locks.locks)[0][field]; }, /must contain exactly/u);
}
for (const [label, change, pattern] of [
  ["null custody", (r) => { r.custody = null; }, /must be an object/u],
  ["copied owner", (r) => { r.owner = r.custody.bond_escrow_account; }, /differs from its map key/u],
  ["custody field missing", (r) => { delete r.custody.escrowed; }, /must contain exactly/u],
  ["custody field extra", (r) => { r.custody.alias = "account"; }, /must contain exactly/u],
  ["nonboolean escrow", (r) => { r.custody.escrowed = 1; }, /must be a boolean/u],
  ["duration overflow", (r) => { r.duration_blocks = U64 + 1n; }, /unsigned bound/u],
  ["expiry quoted", (r) => { r.expiry_height = "55"; }, /integer token/u],
  ["direction overflow", (r) => { r.direction = 256; }, /unsigned bound/u],
  ["noncanonical amount", (r) => { r.amount = "1.0"; }, /canonical/u],
  ["numeric slashed", (r) => { r.slashed = 1; }, /canonical/u],
]) {
  rejectMutation(`lock rejects ${label}`, "locks", "locks",
    (v) => change(Object.values(v.locks.locks)[0]), pattern);
}
test("locks preserve u8 direction and escrow flag without asserting an unavailable Plain context", () => {
  const value = fixture("locks");
  const record = Object.values(value.locks.locks)[0];
  record.direction = 255;
  record.custody.escrowed = false;
  assertWireEqual(decoders.locks(value), value);
});
rejectMutation("locks reject the retired flattened corpus", "locks", "locks",
  (v) => { v.locks = v.locks.locks; }, /corpus must contain exactly/u);
rejectMutation("locks reject a copied referendum selector", "locks", "locks",
  (v) => { v.referendum_id = "ref-other"; }, /differs from request/u);

for (const [kind, name, field, max] of [
  ["referendum", "referendum-open", "h_end", U64],
  ["tally", "tally-empty", "approve", U128],
]) {
  for (const invalid of [-1, -0, 0.5, Number.MAX_SAFE_INTEGER + 1, true, "0", max + 1n]) {
    rejectMutation(`${kind}.${field} refuses invalid integer ${typeof invalid}:${String(invalid)}`,
      kind, name, (value) => { (kind === "referendum" ? value.referendum : value)[field] = invalid; },
      /integer token|unsigned bound/u);
  }
}
for (const raw of [
  '{"found":false,"found":false}',
  '{"found":false,"extra":1e3}',
  '{"found":false,"extra":0.0}',
  '{"found":false,"extra":-0}',
]) {
  test(`lossless JSON reader refuses ambiguous wire tokens: ${raw}`, () => {
    assert.throws(() => parse(raw));
  });
}
test("response decoder rejects getters without executing them", () => {
  let called = false;
  const value = { get found() { called = true; return false; } };
  assert.throws(() => decoders.referendum(value), /enumerable data fields/u);
  assert.equal(called, false);
});
