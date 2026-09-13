import test from "node:test";
import assert from "node:assert/strict";
import { parseStrictLosslessIntegerJson, stringifyStrictLosslessIntegerJson } from "../src/strictLosslessJson.js";
import { createSorafsAliasResponseNormalizers } from "../src/sorafsAliasResponses.js";
import { sorafsAliasProjectionFixture, sorafsAliasAttestationFixture } from "./helpers/sorafsAliasProjection.js";

// The projection delegates actual account identity to the existing Torii owner;
// this suite isolates structural response validation and verifies that delegation.
let accountReads = 0;
const owner = "canonical-owner";
const { normalizeSorafsAliasListResponse: normalize, normalizeSorafsAliasRecord: alias } =
  createSorafsAliasResponseNormalizers({
    requireAccountId(value) {
      accountReads++;
      assert.equal(value, owner);
      return value;
    },
  });
function page() {
  return { attestation: sorafsAliasAttestationFixture(), total_count: 1, returned_count: 1,
    offset: 0, limit: 50, aliases: [sorafsAliasProjectionFixture(owner)] };
}
function rejects(mutator, pattern = /required|exact|bound|variant|projection|must|pagination|omit/u) {
  const input = page();
  mutator(input, input.aliases[0]);
  assert.throws(() => normalize(input), pattern);
}

test("alias projection retains the complete Rust response and delegates account identity", () => {
  const input = page(), snapshot = structuredClone(input);
  accountReads = 0;
  const result = normalize(input);
  assert.deepEqual(result, snapshot);
  assert.deepEqual(input, snapshot);
  assert.notEqual(result.aliases[0].cache_evaluation, input.aliases[0].cache_evaluation);
  assert.equal(accountReads, 1);
  const empty = { ...page(), aliases: [], total_count: 0, returned_count: 0 };
  assert.deepEqual(normalize(empty), empty);
  const noBlock = { ...empty, attestation: { ...empty.attestation, block_height: 0, block_hash_hex: null } };
  assert.deepEqual(normalize(noBlock), noBlock);
});

test("alias projection requires all emitted fields and omits absent live expiry", () => {
  for (const field of Object.keys(page())) rejects(input => { delete input[field]; });
  for (const field of Object.keys(page().aliases[0]).filter(field => field !== "proof_expires_in_seconds")) {
    rejects((_, record) => { delete record[field]; });
  }
  const input = page();
  delete input.aliases[0].proof_expires_in_seconds;
  assert.equal(Object.hasOwn(normalize(input).aliases[0], "proof_expires_in_seconds"), false);
  rejects((_, record) => { record.proof_expires_in_seconds = null; });
  rejects(input => { input.attestation = null; });
});

test("alias nested projections reject unknown and missing fields", () => {
  const paths = [
    ["attestation"], ["aliases", 0], ["aliases", 0, "cache_evaluation"],
    ["aliases", 0, "cache_evaluation", "successor"],
    ["aliases", 0, "cache_evaluation", "governance"],
    ["aliases", 0, "cache_evaluation", "governance", "flags"], ["aliases", 0, "lineage"],
  ];
  for (const path of paths) {
    rejects(input => { path.reduce((value, key) => value[key], input).unknown = true; });
    const fields = Object.keys(path.reduce((value, key) => value[key], page()));
    for (const field of fields.filter(field => field !== "proof_expires_in_seconds")) {
      rejects(input => { delete path.reduce((value, key) => value[key], input)[field]; });
    }
  }
});

test("alias cache reason and label variants remain closed and case-sensitive", () => {
  for (const invalid of ["allow", "Serve", " serve ", null]) {
    rejects((_, record) => { record.cache_decision = invalid; });
  }
  for (const invalid of ["ttl_ok", "approved_successor_pending", "Unknown", null]) {
    rejects((_, record) => { record.cache_reasons = [invalid]; });
  }
  for (const invalid of ["ok", "Fresh", " fresh ", null]) {
    rejects((_, record) => { record.cache_state = invalid; });
  }
  const input = page();
  const record = input.aliases[0];
  record.cache_decision = record.cache_evaluation.decision = "hold";
  record.cache_reasons = record.cache_evaluation.reasons = ["ApprovedSuccessorPending"];
  assert.equal(normalize(input).aliases[0].cache_reasons[0], "ApprovedSuccessorPending");
  rejects((_, record) => { record.cache_reasons = ["RotationDue", "RefreshWindow"]; }, /sorted/u);
  rejects((_, record) => { record.cache_reasons = ["RotationDue", "RotationDue"]; }, /unique/u);
});

function withSuccessor(status) {
  const input = page(), record = input.aliases[0];
  record.lineage.immediate_successor = { digest_hex: "b".repeat(64), status,
    approved_epoch: null, approved_at: null, status_timestamp_unix: null };
  record.lineage.is_head = false;
  record.cache_evaluation.successor.exists = true;
  return input;
}

test("alias manifest status is a closed discriminated projection", () => {
  for (const status of [{ state: "pending" }, { state: "approved", epoch: 7 }, { state: "retired", epoch: 9 }]) {
    const input = withSuccessor(status);
    assert.deepEqual(normalize(input).aliases[0].lineage.immediate_successor.status, status);
  }
  for (const status of [{ state: "pending", epoch: null }, { state: "pending", epoch: 0 },
    { state: "approved" }, { state: "Approved", epoch: 7 }, { state: "retired", epoch: "9" },
    { state: "pending", unknown: null }]) {
    assert.throws(() => normalize(withSuccessor(status)), /epoch|variant|exact/u);
  }
});

test("alias integer fields preserve full-width u64 and reject coercion", () => {
  const input = page(), record = input.aliases[0], max = (1n << 64n) - 1n;
  input.attestation.block_height = max;
  record.bound_epoch = record.expiry_epoch = max;
  record.proof_expires_at_unix = record.cache_evaluation.ttl_expires_at_unix = max;
  const result = normalize(input);
  assert.equal(result.attestation.block_height, max);
  assert.equal(result.aliases[0].expiry_epoch, max);
  for (const invalid of ["1", true, 1.1, -0, -1, null, Number.MAX_SAFE_INTEGER + 1, max + 1n]) {
    rejects((_, record) => { record.bound_epoch = invalid; }, /integer/u);
  }
  rejects(input => { input.limit = 501; });
  rejects(input => { input.offset = Number.MAX_SAFE_INTEGER + 1; });
});

test("alias projection rejects mismatched duplicated state and pagination", () => {
  rejects((_, record) => { record.status_label = "expired"; }, /contradicts/u);
  rejects((_, record) => { record.cache_evaluation.decision = "refuse"; }, /contradicts/u);
  rejects((_, record) => { record.cache_evaluation.reasons = ["ExpiredTTL"]; }, /contradicts/u);
  rejects((_, record) => { record.cache_evaluation.ttl_expires_at_unix++; }, /contradicts/u);
  rejects((_, record) => { record.cache_evaluation.governance.flags.revoked = true; }, /contradicts/u);
  rejects((_, record) => { record.cache_evaluation.successor.head_hex = "c".repeat(64); }, /contradicts/u);
  rejects((_, record) => { record.cache_evaluation.successor.exists = true; }, /contradicts/u);
  rejects(input => { input.returned_count = 0; }, /pagination/u);
  rejects(input => { input.offset = 2; }, /pagination/u);
  rejects(input => { input.limit = 0; }, /pagination/u);
});

test("alias projection rejects alternate hex/base64 and accessor or sparse records", () => {
  rejects((_, record) => { record.manifest_digest_hex = "A".repeat(64); }, /lowercase/u);
  rejects((_, record) => { record.proof_b64 = " cHJvb2Y="; });
  rejects((_, record) => { record.proof_b64 = "cHJvb2Z="; }, /base64/u);
  rejects((_, record) => { record.proof_b64 = ""; });
  rejects((_, record) => { record.proof_b64 = Buffer.alloc(1024 * 1024 + 1).toString("base64"); });
  rejects((_, record) => { record.cache_evaluation.ttl_expires_at = "1970-02-30T00:00:00Z"; }, /timestamp/u);
  rejects((_, record) => { record.cache_evaluation.ttl_expires_at = "1970-01-01T00:01:40+00:00"; }, /timestamp/u);
  let reads = 0;
  rejects(input => { Object.defineProperty(input.attestation, "chain_id", { enumerable: true, get() { reads++; return "chain"; } }); });
  rejects((_, record) => { Object.defineProperty(record.cache_reasons, "0", { enumerable: true, get() { reads++; return "ExpiredTTL"; } }); });
  assert.equal(reads, 0);
  rejects(input => { input.aliases = Array(1); });
  rejects((_, record) => { record[Symbol("hidden")] = 1; });
  assert.throws(() => alias({}, "alias"), /required/u);
});


test("the alias transport JSON owner preserves large tokens and rejects duplicate keys", () => {
  const input = page();
  input.aliases[0].bound_epoch = (1n << 64n) - 1n;
  const raw = stringifyStrictLosslessIntegerJson(input, "alias response");
  const parsed = parseStrictLosslessIntegerJson(raw, "alias response");
  assert.equal(normalize(parsed).aliases[0].bound_epoch, input.aliases[0].bound_epoch);
  const duplicate = raw.replace('"total_count":1', '"total_count":1,"total_count":0');
  assert.throws(() => parseStrictLosslessIntegerJson(duplicate, "alias response"), /duplicate/u);
});


test("governance references retain native opaque strings with exact sorted uniqueness", () => {
  const input = page();
  const references = ["", " padded ", "opaque-reference"];
  input.aliases[0].cache_evaluation.governance.ref_ids = references;
  assert.deepEqual(normalize(input).aliases[0].cache_evaluation.governance.ref_ids, references);
  assert.deepEqual(input.aliases[0].cache_evaluation.governance.ref_ids, references);
  for (const value of [null, 1, true, {}, []]) {
    rejects((_, record) => { record.cache_evaluation.governance.ref_ids = [value]; }, /must be a string/u);
  }
  rejects((_, record) => { record.cache_evaluation.governance.ref_ids = ["", ""]; }, /unique/u);
  rejects((_, record) => { record.cache_evaluation.governance.ref_ids = ["opaque-reference", " padded "]; }, /sorted/u);
});
