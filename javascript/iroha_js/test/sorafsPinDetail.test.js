import test from "node:test";
import assert from "node:assert/strict";
import { parseStrictLosslessJson, parseStrictLosslessIntegerJson } from "../src/strictLosslessJson.js";
import { createSorafsPinDetailNormalizer, normalizeSorafsPinU64 } from "../src/sorafsPinDetail.js";
import { sorafsPinDetailFixture, typedSorafsPinDetailFixture } from "./helpers/sorafsPinDetail.js";

const owner = "canonical-owner";
const calls = [];
const { normalize } = createSorafsPinDetailNormalizer({
  bytes(value, context, { exactLength }) {
    assert.ok(Array.isArray(value), context);
    assert.equal(value.length, exactLength, context);
    assert.deepEqual(Object.keys(value), Array.from({ length: exactLength }, (_, i) => String(i)), context);
    for (const byte of value) assert.ok(Number.isInteger(byte) && byte >= 0 && byte <= 255, context);
    return Uint8Array.from(value);
  },
  account(value) { calls.push(["account", value]); assert.equal(value, owner); return value; },
  asset(value) { calls.push(["asset", value]); assert.equal(value, "asset-id"); return value; },
  quantity(value) { calls.push(["quantity", value]); assert.equal(value, "12.5"); return value; },
});
const fixture = () => sorafsPinDetailFixture(owner);
const floatingPointPaths = [["manifest", "metadata"]];
const parse = (text) => parseStrictLosslessJson(text, "pin detail", { floatingPointPaths });
const get = (value, path) => path.reduce((node, key) => node[key], value);
function set(value, path, replacement) { get(value, path.slice(0, -1))[path.at(-1)] = replacement; }
function rejects(mutate) { const value = fixture(); mutate(value, value.manifest); assert.throws(() => normalize(value)); }

// Structural parsing delegates identities and quantity strings to existing SDK owners.
test("finalized pin detail retains every native field and delegates identity validation", () => {
  const value = fixture();
  value.manifest.pin_fee_payment = { paid_by: owner, fee_asset_id: "asset-id", treasury_account_id: owner, amount: "12.5" };
  value.manifest.status = { status: "Retired", value: 50 };
  value.manifest.retirement_reason = " opaque reason ";
  calls.length = 0;
  assert.deepEqual(normalize(value), typedSorafsPinDetailFixture(value));
  assert.deepEqual(calls, [["account", owner], ["account", owner], ["asset", "asset-id"], ["account", owner], ["quantity", "12.5"]]);
  assert.deepEqual(normalize(parse(JSON.stringify(value))), typedSorafsPinDetailFixture(value));
});

test("finalized pin detail enforces current exact fields and rejects retired wrappers", () => {
  const required = Object.keys(fixture().manifest).filter(key => key !== "successor_of");
  for (const key of required) rejects((_, record) => { delete record[key]; });
  for (const key of ["attestation", "aliases", "replication_orders"]) rejects(value => { value[key] = []; });
  for (const key of ["digest_hex", "pin_policy", "lineage", "governance_refs", "status_timestamp_unix"]) rejects((_, record) => { record[key] = null; });
  for (const path of [["finalized_cursor"], ["manifest", "chunker"], ["manifest", "policy"], ["manifest", "policy", "storage_class"], ["manifest", "alias"], ["manifest", "status"]]) {
    rejects(value => { get(value, path).extra = true; });
    for (const key of Object.keys(get(fixture(), path))) rejects(value => { delete get(value, path)[key]; });
  }
  rejects(value => { delete value.finalized_cursor; });
  rejects(value => { delete value.manifest; });
});

test("native optional fields distinguish explicit null from omitted fields", () => {
  const value = fixture();
  value.manifest.alias = null;
  value.manifest.council_envelope_digest = null;
  delete value.manifest.successor_of;
  assert.deepEqual(normalize(value), typedSorafsPinDetailFixture(value));
  for (const key of ["successor_of", "retirement_reason", "pin_fee_payment"]) rejects((_, record) => { record[key] = null; });
});

test("native retained approval lifecycle accepts all states and rejects contradictions", () => {
  for (const [status, approved] of [[{ status: "Pending", value: null }, null], [{ status: "Approved", value: 45 }, 45], [{ status: "Retired", value: 50 }, 45], [{ status: "Retired", value: 50 }, null]]) {
    const value = fixture(); value.manifest.status = status; value.manifest.approved_epoch = approved;
    if (approved === null) value.manifest.council_envelope_digest = null;
    assert.deepEqual(normalize(value), typedSorafsPinDetailFixture(value));
  }
  for (const status of [{ status: "pending", value: null }, { status: "Pending", value: 0 }, { status: "Approved", value: null }, { status: "Retired", value: 41 }, { state: "approved", epoch: 45 }]) rejects((_, record) => { record.status = status; });
  for (const approved of [41, 46, null]) rejects((_, record) => { record.approved_epoch = approved; });
  rejects((_, record) => { record.status = { status: "Pending", value: null }; });
  rejects((_, record) => { record.retirement_reason = "premature"; });
  rejects((_, record) => { record.policy.retention_epoch = 45; });
  rejects((_, record) => { record.status = { status: "Retired", value: 50 }; record.policy.retention_epoch = 45; });
  rejects((_, record) => { record.status = { status: "Retired", value: 50 }; record.approved_epoch = null; });
  rejects((_, record) => { record.status = { status: "Pending", value: null }; record.approved_epoch = null; });
  rejects((_, record) => { record.status = { status: "Pending", value: null }; record.approved_epoch = null; record.council_envelope_digest = null; record.retirement_reason = "premature"; });
});

test("native CID, fixed bytes, base64 proof and storage variants retain exact encodings", () => {
  for (const type of ["Hot", "Warm", "Cold"]) {
    const value = fixture(); value.manifest.policy.storage_class.type = type;
    assert.equal(normalize(value).manifest.policy.storage_class.type, type);
  }
  rejects((_, record) => { record.root_cid[1] = 0x55; });
  rejects((_, record) => { record.root_cid.fill(0, 4); });
  rejects((_, record) => { record.root_cid.pop(); });
  rejects((_, record) => { record.digest.fill(0); });
  rejects(value => { value.finalized_cursor.block_hash.fill(0); });
  rejects((_, record) => { record.chunk_digest_sha3_256[0] = 256; });
  for (const proof of ["YQ", "YR==", "YQ==\n", "YQ-_"]) rejects((_, record) => { record.alias.proof = proof; });
  rejects((_, record) => { record.alias.proof = Buffer.alloc(1024 * 1024 + 1).toString("base64"); });
  const value = fixture(); value.manifest.alias.proof = "";
  assert.equal(normalize(value).manifest.alias.proof, "");
  rejects((_, record) => { record.policy.storage_class = "Hot"; });
});

const u64Paths = [
  ["finalized_cursor", "height"], ["manifest", "chunker", "multihash_code"],
  ["manifest", "content_length"], ["manifest", "policy", "retention_epoch"],
  ["manifest", "submitted_epoch"], ["manifest", "approved_epoch"], ["manifest", "status", "value"],
];
test("native u64 readback preserves maximum values within the retained lifecycle", () => {
  const maximum = (1n << 64n) - 1n;
  const value = fixture();
  for (const path of u64Paths) set(value, path, maximum);
  value.manifest.submitted_epoch = maximum - 2n;
  value.manifest.approved_epoch = maximum - 1n;
  value.manifest.status = { status: "Retired", value: maximum };
  const text = JSON.stringify(value, (_, item) => typeof item === "bigint" ? `__u64_${item}__` : item).replace(/"__u64_([0-9]+)__"/gu, "$1");
  assert.deepEqual(normalize(parse(text)), typedSorafsPinDetailFixture(value));
  const pending = fixture();
  pending.manifest.submitted_epoch = maximum;
  pending.manifest.approved_epoch = null;
  pending.manifest.council_envelope_digest = null;
  pending.manifest.status = { status: "Pending", value: null };
  assert.equal(normalize(pending).manifest.submitted_epoch, maximum);
  // Approval at u64::MAX is impossible because approval must precede retention.
  rejects(input => { input.manifest.approved_epoch = maximum; input.manifest.status.value = maximum; input.manifest.policy.retention_epoch = maximum; });
  for (const path of u64Paths) {
    for (const invalid of ["42", -1, -0, 1.5, Number.MAX_SAFE_INTEGER + 1, maximum + 1n]) rejects(input => set(input, path, invalid));
  }
  for (const [input, expected] of [[0, 0], [0n, 0], [Number.MAX_SAFE_INTEGER, Number.MAX_SAFE_INTEGER], [BigInt(Number.MAX_SAFE_INTEGER) + 1n, BigInt(Number.MAX_SAFE_INTEGER) + 1n], [maximum, maximum]]) assert.equal(normalizeSorafsPinU64(input, "u64"), expected);
  rejects(input => { input.finalized_cursor.height = 0; });
  rejects((_, record) => { record.chunker.profile_id = 2 ** 32; });
  rejects((_, record) => { record.policy.min_replicas = 2 ** 16; });
});

test("metadata-only float admission rejects decimal and exponent aliases at every typed numeric field", () => {
  const paths = [...u64Paths, ["manifest", "chunker", "profile_id"], ["manifest", "policy", "min_replicas"], ["manifest", "digest", 0]];
  for (const path of paths) {
    for (const token of ["1.0", "1e0", "1E+0", "-0", "1.5"]) {
      const value = fixture(); set(value, path, "__token__");
      assert.throws(() => parse(JSON.stringify(value).replace('"__token__"', token)), /canonical integers/);
    }
  }
  const value = fixture(); value.manifest.metadata = { fractional: 1.25, nested: [{ exponent: 125, integer: 42 }] };
  const text = JSON.stringify(value).replace('"exponent":125', '"exponent":1.25e2');
  assert.deepEqual(normalize(parse(text)).manifest.metadata, value.manifest.metadata);
  assert.throws(() => parseStrictLosslessIntegerJson(text, "strict integer"), /canonical integers/);
  assert.throws(() => parseStrictLosslessJson(text, "default strict"), /canonical integers/);
  for (const key of ["metadata", "manifest_metadata"]) assert.throws(() => parse(`{"${key}":{"value":1.0}}`), /canonical integers/);
});

test("metadata float admission retains duplicate, unicode and finite-number rejection", () => {
  for (const metadata of ['{"x":1,"x":2}', '{"x":1e309}', '{"x":"\\ud800"}', '{"x":01}', '{"x":1.}', '{"x":1e}']) {
    const text = JSON.stringify(fixture()).replace('"metadata":{"note":"demo"}', `"metadata":${metadata}`);
    assert.throws(() => parse(text));
  }
  for (const duplicate of ['"height":51,"height":51', '"height":51,"\\u0068eight":51']) {
    assert.throws(() => parse(JSON.stringify(fixture()).replace('"height":51', duplicate)), /duplicate/);
  }
  assert.throws(() => parseStrictLosslessJson("1", "context", { floatingPointPaths: [[]] }), /non-empty string paths/);
});

test("finalized detail binds digest and both finalized anchor components", () => {
  const value = fixture();
  const expected = { digestHex: "ee".repeat(32), height: 51n, blockHashHex: "42".repeat(32) };
  assert.deepEqual(normalize(value, expected), typedSorafsPinDetailFixture(value));
  for (const mutation of [{ digestHex: "ff".repeat(32) }, { height: 52 }, { blockHashHex: "43".repeat(32) }]) assert.throws(() => normalize(value, { ...expected, ...mutation }), /differs/);
});
