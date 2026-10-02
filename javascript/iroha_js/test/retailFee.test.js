import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { decodeRetailFeeAssessmentMarkerMessage, encodeRetailFeeQuoteRequestV1, retailFeePaymentIntentHash, encodeRetailFeeAssessmentV1, retailFeeAssessmentMarkerMessage, noritoEncodeMultisigProposeRequest } from "../src/norito.js";
const fixture = JSON.parse(readFileSync(new URL("./fixtures/retail_fee_codec_v1.json", import.meta.url), "utf8"));
const hash = (request) => Buffer.from(retailFeePaymentIntentHash(request)).toString("hex");

test("retail proposal codec has one versioned first-release schema and no retired fee fields", () => {
  const codec = readFileSync(new URL("../src/norito.js", import.meta.url), "utf8");
  assert.match(codec, /iroha_torii::routing::MultisigProposeDtoV1/u);
  assert.doesNotMatch(codec, /iroha_torii::routing::MultisigProposeDto["']/u);
  for (const field of [
    "validation_fee_policy_version",
    "validation_fee_policy_hash",
    "validation_fee_hijiri_fee_quote_hash",
    "validation_fee_instruction_index",
    "validation_fee_transfer_entry_index",
  ]) {
    assert.equal(codec.includes(field), false, `${field} is retired from the proposal codec`);
  }
});

test("retail intent commits to exact amount and ordered payment legs", () => {
  assert.equal(encodeRetailFeeQuoteRequestV1(fixture.request).toString("hex"), fixture.request_hex);
  assert.equal(hash(fixture.request), fixture.intent_hash_hex);
  const first = fixture.request.transfers[0];
  const second = { ...first, amount_minor_units: 200 };
  assert.notEqual(hash({ ...fixture.request, transfers: [first, second] }), hash({ ...fixture.request, transfers: [second, first] }));
  assert.notEqual(hash(fixture.request), hash({ ...fixture.request, transfers: [second] }));
});

test("included payments retain a complete zero assessment and marker", () => {
  assert.equal(fixture.assessment.fee_minor, 0);
  assert.equal(retailFeeAssessmentMarkerMessage(fixture.assessment), fixture.marker);
  for (const key of ["fee_minor", "payments_used_before", "policy_revision", "expires_at_ms"]) {
    const changed = { ...fixture.assessment, [key]: fixture.assessment[key] + 1 };
    if (key === "fee_minor") assert.throws(() => retailFeeAssessmentMarkerMessage(changed));
    else assert.notEqual(retailFeeAssessmentMarkerMessage(changed), fixture.marker);
  }
  assert.notEqual(retailFeeAssessmentMarkerMessage({ ...fixture.assessment, retail_enrolled: false }), fixture.marker);
});

test("fee codecs reject hidden overrides, malformed commitments and lossy quantities", () => {
  assert.throws(() => encodeRetailFeeQuoteRequestV1({ ...fixture.request, exempt: true }));
  assert.throws(() => encodeRetailFeeAssessmentV1({ ...fixture.assessment, disabled: true }));
  assert.throws(() => encodeRetailFeeAssessmentV1({ ...fixture.assessment, intent_hash: [1] }));
  for (const amount of [-1, 0.1, Number.MAX_SAFE_INTEGER + 1, "18446744073709551616"]) {
    assert.throws(() => encodeRetailFeeQuoteRequestV1({ ...fixture.request, transfers: [{ ...fixture.request.transfers[0], amount_minor_units: amount }] }));
  }
  assert.throws(() => noritoEncodeMultisigProposeRequest({ instructions: [], validation_fee_instruction_index: "1" }, 753), /unsupported fee field/);
});

test("review decodes exact marker and rejects trailing or noncanonical archives", () => {
  assert.deepEqual(decodeRetailFeeAssessmentMarkerMessage(fixture.marker), fixture.assessment);
  assert.throws(() => decodeRetailFeeAssessmentMarkerMessage(fixture.marker + "00"));
  assert.throws(() => decodeRetailFeeAssessmentMarkerMessage(fixture.marker.toUpperCase()));
});

test("retail codecs enforce positive legs, canonical hash markers and Native count bounds", () => {
  for (const amount of [0, 0n]) {
    assert.throws(() => encodeRetailFeeQuoteRequestV1({ ...fixture.request,
      transfers: [{ ...fixture.request.transfers[0], amount_minor_units: amount }] }));
  }
  assert.throws(() => encodeRetailFeeQuoteRequestV1({ ...fixture.request,
    transfers: Array.from({ length: 1_001 }, () => fixture.request.transfers[0]) }));
  assert.doesNotThrow(() => encodeRetailFeeQuoteRequestV1({ ...fixture.request,
    transfers: Array.from({ length: 1_000 }, () => fixture.request.transfers[0]) }));
  assert.throws(() => encodeRetailFeeAssessmentV1({ ...fixture.assessment, qualifying_payments: 1_001 }));
  for (const key of ["state_commitment", "intent_hash"]) {
    const evenHash = `${fixture.assessment[key].slice(0, 62)}02`;
    assert.throws(() => encodeRetailFeeAssessmentV1({ ...fixture.assessment, [key]: evenHash }));
  }
  assert.throws(() => decodeRetailFeeAssessmentMarkerMessage(
    `iroha:retail_fee:assessment:v1:${"00".repeat(2_034)}`));
});
