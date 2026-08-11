import assert from "node:assert/strict";
import test from "node:test";

import { validatePrivacyExact12NetworkBindingsV1 } from "../src/privacyExact12Network.js";

function compact(value) {
  const bytes = [];
  let remaining = value;
  do {
    const chunk = remaining & 0x7f;
    remaining = Math.floor(remaining / 128);
    bytes.push(chunk | (remaining === 0 ? 0 : 0x80));
  } while (remaining !== 0);
  return Buffer.from(bytes);
}

function fields(...values) {
  return Buffer.concat(
    values.map((value) => {
      const bytes = Buffer.from(value);
      return Buffer.concat([compact(bytes.length), bytes]);
    }),
  );
}

function u32(value) {
  const bytes = Buffer.alloc(4);
  bytes.writeUInt32LE(value);
  return bytes;
}

function fixture(networkId) {
  const context = fields(networkId, ...Array.from({ length: 7 }, () => Buffer.alloc(0)));
  const statement = fields(context, ...Array.from({ length: 10 }, () => Buffer.alloc(0)));
  const domain = Buffer.concat([u32(0), fields(networkId)]);
  return { statementTag: 0, statementContent: statement, projectionDomain: domain, unsignedDomain: domain, context: "row[0]" };
}

test("Exact12 network binding accepts one exact NetworkId", () => {
  const networkId = Buffer.from([...Array(31).fill(0xa4), 0xa5]);
  assert.doesNotThrow(() => validatePrivacyExact12NetworkBindingsV1(fixture(networkId)));
});

test("Exact12 network binding rejects Genesis, labels, and cross-network replay", () => {
  const networkId = Buffer.from([...Array(31).fill(0xa4), 0xa5]);
  const foreign = Buffer.from([...Array(31).fill(0xb4), 0xb5]);
  const baseline = fixture(networkId);
  for (const changed of [
    { ...baseline, unsignedDomain: u32(1) },
    { ...baseline, unsignedDomain: Buffer.concat([u32(0), fields("taira")]) },
    { ...baseline, unsignedDomain: Buffer.concat([u32(0), fields(foreign)]) },
    fixture(Buffer.alloc(32)),
  ]) {
    assert.throws(() => validatePrivacyExact12NetworkBindingsV1(changed), /Network|marked|genesis/u);
  }
});

test("Exact12 network binding rejects non-canonical and trailing fields", () => {
  const networkId = Buffer.from([...Array(31).fill(0xa4), 0xa5]);
  const baseline = fixture(networkId);
  assert.throws(
    () =>
      validatePrivacyExact12NetworkBindingsV1({
        ...baseline,
        unsignedDomain: Buffer.concat([u32(0), Buffer.from([0xa0, 0]), networkId]),
      }),
    /minimally/u,
  );
  assert.throws(
    () =>
      validatePrivacyExact12NetworkBindingsV1({
        ...baseline,
        statementContent: Buffer.concat([baseline.statementContent, Buffer.of(0)]),
      }),
    /trailing/u,
  );
});
