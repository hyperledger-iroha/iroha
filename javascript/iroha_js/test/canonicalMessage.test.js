import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { canonicalRequestMessage } from "../src/canonicalMessage.js";

test("canonical request body hashes the exact bytes of every ArrayBuffer view", () => {
  const bytes = Uint8Array.from({ length: 32 }, (_, index) => index + 1);
  const views = [
    new DataView(bytes.buffer, 8, 16),
    new Uint16Array(bytes.buffer, 8, 8),
    new Int32Array(bytes.buffer, 8, 4),
    new Float64Array(bytes.buffer, 8, 2),
    new BigUint64Array(bytes.buffer, 8, 2),
    bytes.subarray(8, 24),
    Buffer.from(bytes.buffer, 8, 16),
  ];
  const digest = createHash("sha256").update(bytes.subarray(8, 24)).digest("hex");
  for (const body of views) {
    assert.equal(
      canonicalRequestMessage({ method: "POST", path: "/v1/test", body }).toString(),
      `POST\n/v1/test\n\n${digest}`,
      body.constructor.name,
    );
  }
});

test("canonical request body retains string, ArrayBuffer, and empty body semantics", () => {
  const expected = canonicalRequestMessage({ method: "POST", path: "/v1/test", body: "日本語" });
  const bytes = new TextEncoder().encode("日本語");
  for (const body of [bytes, bytes.buffer, Buffer.from(bytes)]) {
    assert.deepEqual(canonicalRequestMessage({ method: "POST", path: "/v1/test", body }), expected);
  }
  const empty = canonicalRequestMessage({ method: "GET", path: "/v1/test" });
  for (const body of ["", new ArrayBuffer(0), new DataView(new ArrayBuffer(4), 2, 0)]) {
    assert.deepEqual(canonicalRequestMessage({ method: "GET", path: "/v1/test", body }), empty);
  }
});
