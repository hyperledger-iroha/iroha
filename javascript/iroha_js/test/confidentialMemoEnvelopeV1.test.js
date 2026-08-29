import assert from "node:assert/strict";
import test from "node:test";

import {
  CONFIDENTIAL_MEMO_MAX_CIPHERTEXT_BYTES_V1,
  CONFIDENTIAL_MEMO_RECIPIENT_SLOTS_V1,
  CONFIDENTIAL_MEMO_WIRE_MAGIC_V1,
  noritoDecodeConfidentialMemoEnvelopeV1,
  noritoEncodeConfidentialMemoEnvelopeV1,
} from "../src/norito.js";

const SUITE = "ml-kem-768-xchacha20-poly1305-v1";
const SLOT_WIRE_BYTES = 1 + 1088 + 24 + 48;

function filled(length, value) {
  return Uint8Array.from({ length }, () => value);
}

function envelope() {
  return {
    slots: Array.from({ length: CONFIDENTIAL_MEMO_RECIPIENT_SLOTS_V1 }, (_, index) => ({
      suite: SUITE,
      encapsulation: filled(1088, index + 1),
      wrap_nonce: filled(24, index + 17),
      wrapped_memo_key: filled(48, index + 33),
    })),
    payload_nonce: filled(24, 0xa5),
    ciphertext: filled(16, 0x5a),
  };
}

test("confidential memo V1 round-trips one exact-eight-slot canonical wire", () => {
  const value = envelope();
  const encoded = noritoEncodeConfidentialMemoEnvelopeV1(value);
  assert.deepEqual(
    Array.from(encoded.subarray(0, CONFIDENTIAL_MEMO_WIRE_MAGIC_V1.length)),
    CONFIDENTIAL_MEMO_WIRE_MAGIC_V1,
  );
  const decoded = noritoDecodeConfidentialMemoEnvelopeV1(encoded);
  assert.equal(decoded.slots.length, 8);
  assert.equal(decoded.slots[0].suite, SUITE);
  assert.deepEqual(
    noritoEncodeConfidentialMemoEnvelopeV1(decoded),
    encoded,
  );
});

test("confidential memo V1 rejects old bytes, aliases, padding gaps, and trailing data", () => {
  assert.throws(
    () => noritoDecodeConfidentialMemoEnvelopeV1(filled(80, 1)),
    /wire magic/u,
  );

  const withAlias = envelope();
  withAlias.payloadNonce = withAlias.payload_nonce;
  assert.throws(
    () => noritoEncodeConfidentialMemoEnvelopeV1(withAlias),
    /unknown field payloadNonce/u,
  );

  const short = envelope();
  short.slots.pop();
  assert.throws(
    () => noritoEncodeConfidentialMemoEnvelopeV1(short),
    /exactly 8 entries/u,
  );

  const encoded = noritoEncodeConfidentialMemoEnvelopeV1(envelope());
  assert.throws(
    () => noritoDecodeConfidentialMemoEnvelopeV1(Uint8Array.from([...encoded, 0])),
    /trailing/u,
  );
});

test("confidential memo V1 rejects duplicate and inert slots", () => {
  const duplicate = envelope();
  duplicate.slots[7] = duplicate.slots[0];
  assert.throws(
    () => noritoEncodeConfidentialMemoEnvelopeV1(duplicate),
    /duplicates an earlier slot/u,
  );

  const inert = envelope();
  inert.slots[0].wrap_nonce = new Uint8Array(24);
  assert.throws(
    () => noritoEncodeConfidentialMemoEnvelopeV1(inert),
    /must not be all zero/u,
  );
});

test("confidential memo V1 rejects malformed lengths before allocation", () => {
  const oversized = envelope();
  oversized.ciphertext = new Uint8Array(
    CONFIDENTIAL_MEMO_MAX_CIPHERTEXT_BYTES_V1 + 1,
  );
  assert.throws(
    () => noritoEncodeConfidentialMemoEnvelopeV1(oversized),
    /must be 16\.\.65536 bytes/u,
  );

  const encoded = Buffer.from(noritoEncodeConfidentialMemoEnvelopeV1(envelope()));
  const lengthOffset =
    CONFIDENTIAL_MEMO_WIRE_MAGIC_V1.length +
    CONFIDENTIAL_MEMO_RECIPIENT_SLOTS_V1 * SLOT_WIRE_BYTES +
    24;
  const noncanonical = Buffer.concat([
    encoded.subarray(0, lengthOffset),
    Buffer.from([0x90, 0x00]),
    encoded.subarray(lengthOffset + 1),
  ]);
  assert.throws(
    () => noritoDecodeConfidentialMemoEnvelopeV1(noncanonical),
    /not minimally encoded/u,
  );
});
