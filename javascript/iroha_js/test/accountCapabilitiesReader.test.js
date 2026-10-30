import assert from "node:assert/strict";
import test from "node:test";
import {
  normalizeAccountCapabilitiesV1,
  readAccountCapabilitiesResponseV1,
} from "../src/accountCapabilities.js";
import { CRYPTO_ALGORITHMS } from "../src/cryptoAlgorithms.js";
import { NetworkId } from "../src/networkId.js";

const CAPABILITIES = Object.freeze({
  schema_version: 1,
  network_id: NetworkId.fromBytes(new Uint8Array(32).fill(0xa5)).literal,
  network_prefix: 369,
  allowed_signing: ["bls_normal", "ed25519", "secp256k1"],
});
const response = (value) => new Response(JSON.stringify(value), {
  headers: { "content-type": "application/json" },
});

test("capability reader accepts unordered discovery and discards unrelated metadata", async () => {
  const decoded = await readAccountCapabilitiesResponseV1(response({
    ...CAPABILITIES,
    default_signing: "ed25519",
    metadata: { operator: "example", revision: 1, notes: [null, true] },
  }));
  assert.deepEqual(decoded, CAPABILITIES);
  assert.ok(Object.isFrozen(decoded));
  assert.ok(Object.isFrozen(decoded.allowed_signing));
  const withoutEd25519 = { ...CAPABILITIES, allowed_signing: ["secp256k1"] };
  assert.deepEqual(await readAccountCapabilitiesResponseV1(response({
    ...withoutEd25519,
    default_signing: "ed25519",
  })), withoutEd25519);
});

test("capability reader requires each known field independently of metadata", async () => {
  for (const field of Object.keys(CAPABILITIES)) {
    const missing = { ...CAPABILITIES, default_signing: "ed25519", metadata: CAPABILITIES };
    delete missing[field];
    await assert.rejects(readAccountCapabilitiesResponseV1(response(missing)), undefined, field);
  }
});

test("capability reader rejects invalid known fields even with unrelated metadata", async () => {
  const invalid = [
    { schema_version: 2 }, { schema_version: "1" },
    { network_id: "not-a-network" }, { network_prefix: "369" },
    { network_prefix: -1 }, { network_prefix: 65536 },
    { allowed_signing: [] }, { allowed_signing: ["unknown"] },
    { allowed_signing: ["Ed25519"] }, { allowed_signing: ["ed25519 "] },
    { allowed_signing: ["ed25519", "secp256k1", "ed25519"] },
  ];
  for (const fields of invalid) {
    await assert.rejects(readAccountCapabilitiesResponseV1(response({
      ...CAPABILITIES, ...fields, default_signing: "ed25519", metadata: CAPABILITIES,
    })), undefined, JSON.stringify(fields));
  }
});

test("capability reader rejects duplicate recognized JSON fields before selecting metadata", async () => {
  const encoded = JSON.stringify(CAPABILITIES);
  for (const [field, value] of Object.entries(CAPABILITIES)) {
    const body = `${encoded.slice(0, -1)},${JSON.stringify(field)}:${JSON.stringify(value)}}`;
    await assert.rejects(readAccountCapabilitiesResponseV1(new Response(body, {
      headers: { "content-type": "application/json" },
    })), /duplicate/u, field);
  }
});

test("capability reader includes ignored metadata in the response size bound", async () => {
  await assert.rejects(readAccountCapabilitiesResponseV1(response({
    ...CAPABILITIES, metadata: "x".repeat(4096),
  })), /4096-byte limit/u);
});

test("capability normalizer accepts all known algorithms in either order without filling holes", () => {
  const algorithms = Object.values(CRYPTO_ALGORITHMS);
  for (const allowed_signing of [algorithms, [...algorithms].reverse()]) {
    const expected = { ...CAPABILITIES, allowed_signing };
    assert.deepEqual(normalizeAccountCapabilitiesV1(expected), expected);
  }
  assert.throws(() => normalizeAccountCapabilitiesV1({
    ...CAPABILITIES, allowed_signing: new Array(1),
  }), /unique canonical algorithm names/u);
});
