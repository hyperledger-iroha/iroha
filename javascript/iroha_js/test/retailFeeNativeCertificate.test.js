import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { normalizeParliamentGovernanceCertificateV1 } from "../src/parliamentApiV1.js";

// Serialized by the native data-model unit fixture. This tests the wire contract;
// it is synthetic and cannot establish finality or authorize production fees.
const fixture = JSON.parse(readFileSync(new URL("./fixtures/retail_fee_native_certificate_v1.json", import.meta.url), "utf8"));

test("fee policy disclosure accepts the exact native Parliament certificate JSON", () => {
  assert.deepEqual(normalizeParliamentGovernanceCertificateV1(fixture), fixture);
  assert.ok(Array.isArray(fixture.effect_preimage_hash));
  assert.deepEqual(fixture.risk_tier, { tier: "Standard", details: null });
});

test("certificate validation rejects altered native unit and commitment encodings", () => {
  for (const mutate of [
    value => { delete value.risk_tier.details; },
    value => { value.risk_tier.details = { approved: true }; },
    value => { value.effect_preimage_hash = Buffer.from(value.effect_preimage_hash).toString("hex"); },
    value => { delete value.body_bindings[0].ballot.outcome.details; },
    value => { value.body_bindings[0].sortition_request.governance_attempt_id = "ee".repeat(32); },
  ]) {
    const candidate = structuredClone(fixture);
    mutate(candidate);
    assert.throws(() => normalizeParliamentGovernanceCertificateV1(candidate));
  }
});
