import { test } from "node:test";
import assert from "node:assert/strict";
import {
  buildGatewayRequest,
  loadComputeFixtures,
  simulateCompute,
  validatePayloadHash,
} from "../src/compute.js";

test.skip("compute fixtures carry a stable payload hash", () => {
  const fixtures = loadComputeFixtures();
  const hashLiteral = validatePayloadHash(fixtures);
  assert.equal(hashLiteral, fixtures.call.request.payload_hash);
  assert.equal(fixtures.call.route.method, "quote");
});

test.skip("compute simulation echoes payload by default", () => {
  const fixtures = loadComputeFixtures();
  const result = simulateCompute(fixtures);
  const expectedB64 = Buffer.from(fixtures.payload).toString("base64");
  assert.equal(result.responseB64, expectedB64);
  assert.equal(result.outcome, "Success");
});

test.skip("gateway request builder encodes payload", () => {
  const fixtures = loadComputeFixtures();
  const request = buildGatewayRequest(fixtures);
  const expectedB64 = Buffer.from(fixtures.payload).toString("base64");
  assert.equal(request.payload_b64, expectedB64);
  assert.equal(request.codec.codec, "NoritoJson");
});
