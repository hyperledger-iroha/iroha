import test from "node:test";
import { SORAFS_REPLICATION_ORDER_CHUNKER_HANDLES_V1 } from "../src/sorafsReplicationProfiles.js";
import assert from "node:assert/strict";
import { createSorafsReplicationResponseNormalizer } from "../src/sorafsReplicationResponses.js";
import { parseStrictLosslessIntegerJson, stringifyStrictLosslessIntegerJson } from "../src/strictLosslessJson.js";
import { sorafsReplicationProjectionFixture, sorafsReplicationAttestationFixture } from "./helpers/sorafsReplicationProjection.js";
const owner = "canonical-owner";
let accountCalls = 0;
const { normalize, orderRecord } = createSorafsReplicationResponseNormalizer({ account(value) { accountCalls++; assert.equal(value, owner); return value; } });
const fixture = () => ({ attestation: sorafsReplicationAttestationFixture(), total_count: 1, returned_count: 1, offset: 0, limit: 50, replication_orders: [sorafsReplicationProjectionFixture(owner)] });
const get = (object, path) => path.reduce((value, key) => value[key], object);
function set(object, path, value) { get(object, path.slice(0, -1))[path.at(-1)] = value; }
function rejects(mutate) { const value = fixture(); mutate(value, value.replication_orders[0]); assert.throws(() => normalize(value)); }

test("replication projection retains complete native order, completion authority and finalized anchor", () => {
  const value = fixture(); accountCalls = 0;
  assert.deepEqual(normalize(value), value);
  assert.equal(accountCalls, 3);
  assert.deepEqual(normalize(parseStrictLosslessIntegerJson(JSON.stringify(value), "replication")), value);
  assert.equal(Object.hasOwn(normalize(value).replication_orders[0], "receipts"), false);
});

test("replication projection requires every current nested field and rejects retired receipt shapes", () => {
  const paths = [[], ["attestation"], ["replication_orders", 0], ["replication_orders", 0, "order"], ["replication_orders", 0, "order", "sla"], ["replication_orders", 0, "order", "assignments", 0], ["replication_orders", 0, "order", "metadata", 0], ["replication_orders", 0, "provider_completions", 0], ["replication_orders", 0, "provider_completions", 0, "completion_authority"], ["replication_orders", 0, "provider_completions", 0, "completion_authority", "signer_policy"], ["replication_orders", 0, "provider_completions", 0, "finalized_anchor"]];
  for (const path of paths) {
    rejects(value => { get(value, path).unknown = 1; });
    for (const key of Object.keys(get(fixture(), path))) rejects(value => { delete get(value, path)[key]; });
  }
  rejects((_, row) => { row.receipts = []; });
  rejects(value => { value.attestation = null; });
});

test("replication lifecycle retains all four exact native statuses", () => {
  for (const state of ["pending", "completed", "cancelled", "expired"]) {
    const value = fixture(), row = value.replication_orders[0];
    if (state === "pending") row.status = { state };
    if (state === "cancelled") row.status = { state, epoch: row.issued_epoch + 20 };
    if (state === "expired") row.status = { state, epoch: row.deadline_epoch + 1 };
    if (state === "completed") {
      const second = structuredClone(row.provider_completions[0]);
      second.provider_hex = row.providers[1]; second.completion_epoch++;
      row.provider_completions.push(second); row.status = { state, epoch: second.completion_epoch };
    }
    assert.deepEqual(normalize(value, { status: state }), value);
  }
  for (const status of [{ state: "Pending" }, { state: "pending", epoch: null }, { state: "cancelled" }, { state: "completed", epoch: null }, { state: "anything", epoch: 1 }]) rejects((_, row) => { row.status = status; });
  rejects((_, row) => { row.status = { state: "completed", epoch: row.issued_epoch + 10 }; });
  rejects((_, row) => { row.status = { state: "expired", epoch: row.deadline_epoch }; });
});

test("replication completion enforces signer predecessor, assignment and provider binding", () => {
  const value = fixture(), policy = value.replication_orders[0].provider_completions[0].completion_authority.signer_policy;
  policy.revision = 1; policy.predecessor_digest_hex = null;
  assert.deepEqual(normalize(value), value);
  for (const mutation of [
    row => { row.provider_completions[0].completion_authority.signer_policy.predecessor_digest_hex = null; },
    row => { row.provider_completions[0].completion_authority.signer_policy.revision = 1; },
    row => { row.provider_completions[0].assignment_revision = 2; },
    row => { row.provider_completions[0].finalized_anchor.height = 0; },
    row => { row.provider_completions[0].finalized_anchor.block_hash_hex = "00".repeat(32); },
    row => { row.provider_completions[0].completion_epoch = row.deadline_epoch + 1; },
    row => { row.provider_completions[0].provider_hex = "ff".repeat(32); },
    row => { row.provider_completions.push(structuredClone(row.provider_completions[0])); },
    row => { row.providers.reverse(); },
    row => { row.order.issued_at++; },
  ]) rejects((_, row) => mutation(row));
});

const numericPaths = [
  ["attestation", "block_height"], ["total_count"], ["returned_count"], ["offset"], ["limit"],
  ...[["issued_epoch"], ["deadline_epoch"], ["assignment_revision"], ["order", "version"], ["order", "target_replicas"], ["order", "issued_at"], ["order", "deadline_at"], ["order", "assignments", 0, "slice_gib"], ["order", "sla", "ingest_deadline_secs"], ["order", "sla", "min_availability_percent_milli"], ["order", "sla", "min_por_success_percent_milli"], ["provider_completions", 0, "completion_epoch"], ["provider_completions", 0, "assignment_revision"], ["provider_completions", 0, "completion_authority", "signer_policy", "revision"], ["provider_completions", 0, "finalized_anchor", "height"]].map(path => ["replication_orders", 0, ...path]),
];
test("every replication integer field rejects strings, unsafe numbers and alternate JSON numeric tokens", () => {
  for (const path of numericPaths) {
    for (const token of ["1.0", "1e0", "-0"]) {
      const value = fixture(); set(value, path, "__token__");
      assert.throws(() => parseStrictLosslessIntegerJson(JSON.stringify(value).replace('"__token__"', token), "replication"), /canonical integers/);
    }
    for (const invalid of ["1", 1.5, -1, Number.MAX_SAFE_INTEGER + 1, 1n << 64n]) rejects(value => set(value, path, invalid));
  }
  rejects((_, row) => { row.status = { state: "expired", epoch: "18446744073709551615" }; });
});

test("replication full-u64 fields remain lossless within each native lifecycle", () => {
  const maximum = (1n << 64n) - 1n, value = fixture(), row = value.replication_orders[0];
  value.attestation.block_height = maximum; value.total_count = maximum; value.limit = 1;
  row.issued_epoch = maximum - 100_000n; row.deadline_epoch = maximum;
  row.order.issued_at = row.issued_epoch; row.order.deadline_at = row.deadline_epoch;
  row.assignment_revision = maximum;
  row.order.assignments[0].slice_gib = maximum;
  const completion = row.provider_completions[0];
  completion.completion_epoch = maximum; completion.assignment_revision = maximum;
  completion.completion_authority.signer_policy.revision = maximum; completion.finalized_anchor.height = maximum;
  assert.deepEqual(normalize(parseStrictLosslessIntegerJson(stringifyStrictLosslessIntegerJson(value, "replication"), "replication"), { limit: 1 }), value);
  row.status = { state: "cancelled", epoch: maximum };
  assert.deepEqual(normalize(value, { limit: 1 }), value);
});

test("replication projection enforces resource caps without restricting u64 payload data to safe integers", () => {
  rejects(value => { value.limit = 501; });
  rejects(value => { value.offset = 2 ** 32; });
  rejects((_, row) => { row.canonical_order_b64 = Buffer.alloc(256 * 1024 + 1).toString("base64"); });
  rejects((_, row) => { row.order.assignments[0].lane = " Padded "; });
  rejects((_, row) => { row.order.sla.ingest_deadline_secs = 0; });
  rejects((_, row) => { row.order.metadata[0].value = " padded "; });
  rejects((_, row) => { row.order.metadata[0].key = "UPPER"; });
  rejects((_, row) => { row.order.metadata[0].value = "é".repeat(2049); });
  rejects((_, row) => { row.order.metadata = Array.from({ length: 17 }, (_, index) => ({ key: String(index), value: "x".repeat(4096) })); });
  rejects((_, row) => { row.order.metadata[0].value = 1.5; });
});

test("replication pagination and filters match the current clamped native response", () => {
  const empty = { ...fixture(), replication_orders: [], total_count: 2, returned_count: 0, offset: 2 };
  assert.deepEqual(normalize(empty, { offset: 0xffff_ffff }), empty);
  rejects(value => { value.returned_count = 0; });
  rejects(value => { value.total_count = 0; });
  rejects(value => { value.total_count = 2; });
  assert.throws(() => normalize(fixture(), { status: "completed" }), /filter/);
  assert.throws(() => normalize(fixture(), { manifest_digest: "ff".repeat(32) }), /filter/);
  assert.throws(() => normalize(fixture(), { limit: 20 }), /pagination/);
  const row = fixture().replication_orders[0];
  assert.deepEqual(orderRecord(row, "record"), row);
});


test("native metadata whitespace preserves FEFF while rejecting Unicode White_Space edges", () => {
  const value = fixture(); value.replication_orders[0].order.metadata[0].value = "\uFEFF";
  assert.deepEqual(normalize(value), value);
  for (const text of ["\u00a0text", "text\u00a0", "\u0085text", "text\u202f"]) {
    rejects((_, row) => { row.order.metadata[0].value = text; });
  }
});

test("completion authority equality is independent of canonical account parsing", () => {
  const secondOwner = "canonical-second-owner";
  const { normalize: normalizeTwoOwners } = createSorafsReplicationResponseNormalizer({
    account(value) { assert.ok(value === owner || value === secondOwner); return value; },
  });
  const value = fixture(), completion = value.replication_orders[0].provider_completions[0];
  completion.completed_by = secondOwner;
  assert.throws(() => normalizeTwoOwners(value), /completion authority/);
  completion.completion_authority.provider_owner = secondOwner;
  assert.deepEqual(normalizeTwoOwners(value), value);
});

test("retained completion anchor may precede and differ from the current inventory tip", () => {
  const value = fixture();
  value.replication_orders[0].provider_completions[0].finalized_anchor = { height: 17, block_hash_hex: "61".repeat(32) };
  assert.deepEqual(normalize(value), value);
});


test("completion anchors cannot postdate or contradict the attested committed prefix", () => {
  rejects((_, row) => { row.provider_completions[0].finalized_anchor.height = 52; });
  rejects((_, row) => { row.provider_completions[0].finalized_anchor.block_hash_hex = "61".repeat(32); });
  rejects(value => { value.attestation = { block_height: 0, block_hash_hex: null, chain_id: "fixture-chain" }; });
  const value = fixture();
  assert.deepEqual(normalize(value), value, "equal-height hashes agree exactly");
});

test("completion anchor ordering preserves adjacent full-width u64 heights", () => {
  const value = fixture(), maximum = (1n << 64n) - 1n;
  value.attestation.block_height = maximum;
  const anchor = value.replication_orders[0].provider_completions[0].finalized_anchor;
  anchor.height = maximum - 1n;
  anchor.block_hash_hex = "61".repeat(32);
  assert.deepEqual(normalize(value), value, "older full-width anchor retains its own hash");
  anchor.height = maximum;
  assert.throws(() => normalize(value), /attested committed prefix/);
  anchor.block_hash_hex = value.attestation.block_hash_hex;
  assert.deepEqual(normalize(value), value);
  value.attestation.block_height = maximum - 1n;
  assert.throws(() => normalize(value), /attested committed prefix/);
});


test("replication profiles share the immutable canonical native registry handles", () => {
  assert.deepEqual(SORAFS_REPLICATION_ORDER_CHUNKER_HANDLES_V1,
    ["sorafs.sf1@1.0.0", "sorafs.sf2@1.0.0"]);
  assert.equal(Object.isFrozen(SORAFS_REPLICATION_ORDER_CHUNKER_HANDLES_V1), true);
  for (const handle of SORAFS_REPLICATION_ORDER_CHUNKER_HANDLES_V1) {
    const value = fixture(); value.replication_orders[0].order.chunking_profile = handle;
    assert.deepEqual(normalize(value), value);
  }
  for (const handle of ["sorafs/sf1@1.0.0", "sorafs-sf1", "sf1", "sorafs.sf3@1.0.0",
    "sorafs.sf1@2.0.0", "SORAFS.sf1@1.0.0", "sorafs.sf1@1.0.0 ", ""]) {
    rejects((_, row) => { row.order.chunking_profile = handle; });
  }
});
