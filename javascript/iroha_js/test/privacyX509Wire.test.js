import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";
import {
  PRIVACY_X509_WIRE_ID_V1,
  parsePrivacyX509RequestV1,
  preparePrivacyX509DirectBytesV1,
  validatePrivacyX509FixtureBindingsV1,
} from "../src/privacyX509Wire.js";
import {
  frameNoritoPayload, schemaHashForTypeName,
  encodeNoritoStringValue, withNoritoCompactLengths, validateNoritoFrame,
  u32ToLittleEndianBuffer as u32, u64ToLittleEndianBuffer as u64,
} from "../src/noritoValueCodecs.js";

const requestSchema = "iroha_data_model::isi::privacy::SubmitPrivacyZkX509CertificateV1";
function length(value) {
  const result = [];
  do { const part = value & 127; value >>>= 7; result.push(part | (value ? 128 : 0)); } while (value);
  return Buffer.from(result);
}
function fields(values) { return Buffer.concat(values.flatMap((value) => [length(value.length), value])); }
function tagged(tag, value) { return Buffer.concat([u32(tag), fields([value])]); }
function vector(value) { return Buffer.concat([u64(value.length), value]); }
function frame(value) { return frameNoritoPayload(value, schemaHashForTypeName(requestSchema), 2, 0); }
function split(value, count) {
  let cursor = 0;
  return Array.from({ length: count }, () => {
    let size = 0; let shift = 0; let next;
    do { next = value[cursor++]; size += (next & 127) * 2 ** shift; shift += 7; } while (next & 128);
    const out = Buffer.from(value.subarray(cursor, cursor + size)); cursor += size; return out;
  });
}
const limits = {
  max_actions_per_transaction: 1, max_proof_bytes_per_action: 9 * 1024 * 1024,
  max_action_bytes: 10 * 1024 * 1024, max_privacy_bytes_per_transaction: 10 * 1024 * 1024,
  max_statement_and_encrypted_output_bytes_per_transaction: 256 * 1024,
};

// Deliberately structural public byte vectors: no valid credential, proof, State or admission.
function structural({ action = 0, slot = 0, intent = Buffer.alloc(32), time = 7, nonce = 1 } = {}) {
  const network = Buffer.alloc(32, 1);
  const context = [network, u32(0), fields([intent]), ...Array.from({ length: 5 }, (_, i) => fields([Buffer.alloc(32, i + 1)]))];
  const statement = [fields(context), ...Array.from({ length: 19 }, () => u64(1))];
  const request = frame(fields([fields(statement), fields([u32(action), u32(slot)])]));
  const instruction = fields([withNoritoCompactLengths(() => encodeNoritoStringValue(PRIVACY_X509_WIRE_ID_V1)), vector(request)]);
  const payloadFields = [tagged(0, network), statement[17], u64(time),
    tagged(0, Buffer.concat([u64(1), fields([instruction])])), Buffer.concat([Buffer.of(1), fields([u64(60_000)])]),
    Buffer.concat([Buffer.of(1), fields([u32(nonce)])]), Buffer.of(3), u64(0), Buffer.of(0)];
  return { request, payload: fields(payloadFields), payloadFields };
}

test("X509 structural parser rejects noncanonical and substituted request fields", () => {
  const { request } = structural();
  assert.equal(parsePrivacyX509RequestV1(request).actionIndex, 0);
  assert.throws(() => parsePrivacyX509RequestV1(structural({ slot: 1 }).request), /slot/);
  assert.throws(() => parsePrivacyX509RequestV1(structural({ intent: Buffer.alloc(32, 1) }).request), /ZERO/);
  assert.throws(() => parsePrivacyX509RequestV1(Buffer.concat([request, Buffer.of(0)])));
  assert.throws(() => parsePrivacyX509RequestV1(Buffer.alloc(262_145)), /ceiling/);
  assert.throws(() => parsePrivacyX509RequestV1(frame(Buffer.from([0x80, 0x00, 0x00]))), /noncanonical/);
  assert.throws(() => parsePrivacyX509RequestV1(frame(Buffer.alloc(10, 255))), /overflow/);
  assert.throws(() => parsePrivacyX509RequestV1("not bytes"), /bytes/);
});

test("X509 structural strings match the maintained canonical Norito owner", () => {
  const source = structural();
  const plan = preparePrivacyX509DirectBytesV1(source.request, source.payload);
  const actionFields = split(validateNoritoFrame(plan.actionStatementNorito).payload, 7);
  const canonicalString = withNoritoCompactLengths(() => encodeNoritoStringValue(PRIVACY_X509_WIRE_ID_V1));
  assert.deepEqual(actionFields[2], tagged(0, canonicalString));
  const obsolete = fields([vector(Buffer.from(PRIVACY_X509_WIRE_ID_V1)), vector(source.request)]);
  const changed = source.payloadFields.slice();
  changed[3] = tagged(0, Buffer.concat([u64(1), fields([obsolete])]));
  assert.throws(() => preparePrivacyX509DirectBytesV1(source.request, fields(changed)), /instruction wire/);
});

test("X509 structural preparation retains original byte inputs and protects returned copies", () => {
  const source = structural();
  const plan = preparePrivacyX509DirectBytesV1(source.request, source.payload);
  const expected = plan.materializedStatementNorito;
  const proof = Buffer.of(1, 2, 3);
  const original = plan.completePayload(proof, limits);
  source.request.fill(0); source.payload.fill(0);
  plan.materializedStatementNorito.fill(0); plan.actionStatementNorito.fill(0);
  plan.actionStatementDigest.fill(0); plan.orderedIntentDigest.fill(0);
  assert.deepEqual(plan.materializedStatementNorito, expected);
  assert.deepEqual(plan.completePayload(proof, limits), original);
  const other = structural({ time: 8 });
  const changed = preparePrivacyX509DirectBytesV1(other.request, other.payload);
  assert.notDeepEqual(plan.orderedIntentDigest, changed.orderedIntentDigest);
  assert.notDeepEqual(plan.actionStatementDigest, changed.actionStatementDigest);
  assert.notDeepEqual(plan.materializedStatementNorito, changed.materializedStatementNorito);
});

test("X509 structural preparation refuses changed network authority instruction and action reference", () => {
  const source = structural();
  for (const index of [0, 1, 3, 8]) {
    const changed = source.payloadFields.slice();
    changed[index] = Buffer.concat([changed[index], Buffer.of(4)]);
    assert.throws(() => preparePrivacyX509DirectBytesV1(source.request, fields(changed)));
  }
  const other = structural({ action: 1 });
  assert.throws(() => preparePrivacyX509DirectBytesV1(other.request, other.payload), /direct action/);
});

test("X509 structural proof insertion changes only the signed occurrence and enforces exact bounds", () => {
  const { request, payload } = structural();
  const plan = preparePrivacyX509DirectBytesV1(request, payload);
  const proof = Buffer.of(1, 2, 3);
  const first = split(plan.completePayload(proof, limits), 9);
  const second = split(plan.completePayload(Buffer.of(2, 2, 3), limits), 9);
  assert.deepEqual(first.slice(0, 8), second.slice(0, 8));
  assert.notDeepEqual(first[8], second[8]);
  const exact = { ...limits, max_action_bytes: request.length + proof.length,
    max_privacy_bytes_per_transaction: request.length + proof.length, max_proof_bytes_per_action: proof.length };
  assert.doesNotThrow(() => plan.completePayload(proof, exact));
  for (const key of ["max_action_bytes", "max_privacy_bytes_per_transaction", "max_proof_bytes_per_action"]) {
    assert.throws(() => plan.completePayload(proof, { ...exact, [key]: exact[key] - 1 }), /bounds/);
  }
  for (const bad of [Buffer.alloc(0), Buffer.alloc(3)]) assert.throws(() => plan.completePayload(bad, limits), /bounds/);
  assert.throws(() => plan.completePayload(proof, { ...limits, max_actions_per_transaction: 0 }), /bounds/);
  assert.throws(() => plan.completePayload(proof, { ...limits, max_action_bytes: NaN }), /committed limit/);
  assert.throws(() => plan.completePayload(proof, { ...limits, max_action_bytes: 2 ** 32 }), /committed limit/);
});

test("X509 native regenerated Exact12 fixture matches canonical attachment projections", async () => {
  const { noritoDecodePrivacyExact12FixtureBundleBase64V1 } = await import("../src/norito.js");
  const archive = fs.readFileSync(new URL("../../../fixtures/privacy/exact12_typed_fixture_bundle_v1.norito.b64", import.meta.url), "utf8").trimEnd();
  const bundle = noritoDecodePrivacyExact12FixtureBundleBase64V1(archive);
  const row = bundle.rows.find((item) => item.protocolId === "iroha-zk-x509-stark-p256-v1");
  assert.equal(row.submitProofWireId, PRIVACY_X509_WIRE_ID_V1);
  assert.doesNotThrow(() => validatePrivacyX509FixtureBindingsV1(row));
  for (const key of ["statementNorito", "submitProofInstructionNorito", "transactionIntentProjectionNorito",
    "transactionIntentDigest", "unsignedTransactionPayloadNorito"]) {
    const mutated = Buffer.from(row[key]); mutated[mutated.length - 1] ^= 1;
    assert.throws(() => validatePrivacyX509FixtureBindingsV1({ ...row, [key]: mutated }));
  }
});
