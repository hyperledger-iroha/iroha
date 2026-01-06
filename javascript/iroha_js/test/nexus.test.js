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
