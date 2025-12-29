import assert from "node:assert/strict";
import test from "node:test";

import {
  decodeLaneRelayEnvelope,
  laneRelayEnvelopeSample,
  verifyLaneRelayEnvelopeJson,
  verifyLaneRelayEnvelopes,
} from "../src/index.js";
import { getNativeBinding } from "../src/native.js";

const nativeAvailable = getNativeBinding() !== null;
const maybeTest = nativeAvailable ? test : test.skip;

maybeTest("verifyLaneRelayEnvelopeJson rejects settlement tampering", () => {
  const { valid } = laneRelayEnvelopeSample();
  const decoded = decodeLaneRelayEnvelope(valid);
  verifyLaneRelayEnvelopeJson(decoded);

  const tampered = structuredClone(decoded);
  tampered.settlement_commitment.tx_count += 1;
  assert.throws(
    () => verifyLaneRelayEnvelopeJson(tampered),
    /settlement hash/i,
    "expected settlement hash mismatch to surface",
  );
});

maybeTest("verifyLaneRelayEnvelopes detects duplicate keys", () => {
  const { valid } = laneRelayEnvelopeSample();
  const decoded = decodeLaneRelayEnvelope(valid);

  verifyLaneRelayEnvelopes([decoded]);
  assert.throws(
    () => verifyLaneRelayEnvelopes([decoded, decoded]),
    /duplicate relay envelope/i,
  );
});
