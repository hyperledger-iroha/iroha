import assert from "node:assert/strict";
import { test as baseTest } from "node:test";

import {
  decodeLaneRelayEnvelope,
  laneRelayEnvelopeSample,
  verifyLaneRelayEnvelope,
  verifyLaneRelayEnvelopes,
} from "../src/nexus.js";
import { makeNativeTest } from "./helpers/native.js";

const test = makeNativeTest(baseTest);

test("verifyLaneRelayEnvelopes accepts canonical relay envelopes", () => {
  const sample = laneRelayEnvelopeSample();
  const decoded = decodeLaneRelayEnvelope(sample.valid);
  verifyLaneRelayEnvelopes([decoded]);
});

test("verifyLaneRelayEnvelopes accepts JSON strings", () => {
  const sample = laneRelayEnvelopeSample();
  const decoded = decodeLaneRelayEnvelope(sample.valid);
  const payload = JSON.stringify(decoded);
  verifyLaneRelayEnvelopes([payload]);
});

test("verifyLaneRelayEnvelopes rejects duplicates", () => {
  const sample = laneRelayEnvelopeSample();
  const decoded = decodeLaneRelayEnvelope(sample.valid);
  assert.throws(
    () => verifyLaneRelayEnvelopes([decoded, decoded]),
    /duplicate relay envelope/,
  );
});

test("verifyLaneRelayEnvelope rejects invalid base64 payloads", () => {
  assert.throws(
    () => verifyLaneRelayEnvelope("not*base64"),
    /base64/,
  );
});

test("verifyLaneRelayEnvelope rejects invalid byte arrays", () => {
  assert.throws(
    () => verifyLaneRelayEnvelope([256]),
    /byte/,
  );
});

test("verifyLaneRelayEnvelope accepts ArrayBuffer inputs", () => {
  const sample = laneRelayEnvelopeSample();
  const buffer = sample.valid;
  const arrayBuffer = buffer.buffer.slice(
    buffer.byteOffset,
    buffer.byteOffset + buffer.byteLength,
  );
  verifyLaneRelayEnvelope(arrayBuffer);
});

test("verifyLaneRelayEnvelope accepts ArrayBufferView inputs", () => {
  const sample = laneRelayEnvelopeSample();
  const buffer = sample.valid;
  const arrayBuffer = buffer.buffer.slice(
    buffer.byteOffset,
    buffer.byteOffset + buffer.byteLength,
  );
  const view = new DataView(arrayBuffer);
  verifyLaneRelayEnvelope(view);
});
