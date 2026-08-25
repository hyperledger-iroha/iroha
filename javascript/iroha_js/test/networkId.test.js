import assert from "node:assert/strict";
import test from "node:test";

import { NetworkId, networkIdBytes } from "../src/networkId.js";

const CANONICAL_NETWORK_ID =
  "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149";
const CANONICAL_BYTES = Buffer.from(
  "32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149",
  "hex",
);

test("NetworkId round-trips one exact genesis hash without exposing mutable storage", () => {
  const parsed = NetworkId.parse(CANONICAL_NETWORK_ID);
  const fromBytes = NetworkId.fromBytes(CANONICAL_BYTES);

  assert.equal(parsed.literal, CANONICAL_NETWORK_ID);
  assert.equal(NetworkId.BYTE_LENGTH, 32);
  assert.equal(Object.isFrozen(parsed), true);
  assert.throws(() => {
    NetworkId.BYTE_LENGTH = 31;
  }, TypeError);
  assert.equal(parsed.toString(), CANONICAL_NETWORK_ID);
  assert.equal(JSON.stringify(parsed), JSON.stringify(CANONICAL_NETWORK_ID));
  assert.equal(parsed.equals(fromBytes), true);
  assert.deepEqual(Buffer.from(parsed.toBytes()), CANONICAL_BYTES);

  const copy = parsed.toBytes();
  copy.fill(0);
  assert.deepEqual(Buffer.from(parsed.toBytes()), CANONICAL_BYTES);
  const internalCopy = networkIdBytes(parsed);
  internalCopy.fill(0);
  assert.deepEqual(Buffer.from(networkIdBytes(parsed)), CANONICAL_BYTES);
});

test("NetworkId rejects labels, aliases, malformed literals, and unmarked bytes", () => {
  assert.throws(() => new NetworkId(), /must be created/u);
  for (const literal of [
    "wonderland",
    CANONICAL_NETWORK_ID.toUpperCase(),
    `hash:${CANONICAL_NETWORK_ID}`,
    `${CANONICAL_NETWORK_ID.slice(0, -1)}8`,
    ` ${CANONICAL_NETWORK_ID}`,
  ]) {
    assert.throws(() => NetworkId.parse(literal), /NetworkId/u);
  }
  for (const bytes of [
    Buffer.alloc(31, 1),
    Buffer.alloc(33, 1),
    Buffer.alloc(32, 2),
  ]) {
    assert.throws(() => NetworkId.fromBytes(bytes), /NetworkId bytes/u);
  }
  assert.throws(
    () => networkIdBytes({ literal: CANONICAL_NETWORK_ID, toBytes: () => CANONICAL_BYTES }),
    /must be a NetworkId/u,
  );
});
