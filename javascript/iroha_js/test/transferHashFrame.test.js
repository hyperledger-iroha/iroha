import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import {
  BrowserTransactionCodecError,
  browserSignedTransactionHashHex,
  _browserSignedTransferTransactionHashHex,
} from '../src/transactionCodec.js';

const fixtureText = readFileSync(new URL('../../../fixtures/norito_rpc/iroha_compact_hash_vector.properties', import.meta.url), 'utf8');
const encoded = fixtureText.split('\n').find(line => line.startsWith('versioned.base64=')).slice('versioned.base64='.length);
const canonicalEnvelope = Buffer.from(encoded, 'base64');
assert.equal(canonicalEnvelope.toString('base64'), encoded);
const length = value => {
  const bytes = [];
  do { const low = value % 128; value = Math.floor(value / 128); bytes.push(low | (value ? 128 : 0)); } while (value);
  return Buffer.from(bytes);
};
const field = bytes => Buffer.concat([length(bytes.length), bytes]);
function splitEnvelope(value) {
  const fields = [];
  let offset = 1;
  while (offset < value.length) {
    let count = 0, power = 1, octet;
    do { octet = value[offset++]; count += (octet & 127) * power; power *= 128; } while (octet & 128);
    fields.push(value.subarray(offset, offset + count)); offset += count;
  }
  assert.equal(offset, value.length);
  assert.equal(fields.length, 3);
  return fields;
}
const [signature, payload, multisig] = splitEnvelope(canonicalEnvelope);
const envelope = (payloadValue = payload, multisigValue = multisig) => Buffer.concat([Buffer.of(1), field(signature), field(payloadValue), field(multisigValue)]);
assert.deepEqual(envelope(), canonicalEnvelope);

test('transfer-only hashing rejects execution-sized payloads before parsing their contents', () => {
  assert.throws(
    () => _browserSignedTransferTransactionHashHex(envelope(Buffer.alloc(1024 * 1024 + 1))),
    (error) => {
      assert.ok(error instanceof BrowserTransactionCodecError);
      assert.equal(error.code, 'bounds_exceeded');
      assert.equal(error.message, 'payloadBytes must contain 1..=1048576 bytes');
      return true;
    },
  );
});

const wrongVersion = Buffer.from(canonicalEnvelope); wrongVersion[0] = 2;
const overlongSignatureLength = Buffer.concat([canonicalEnvelope.subarray(0, 2), Buffer.of(canonicalEnvelope[2] | 128, 0), canonicalEnvelope.subarray(3)]);
for (const [label, value, code] of [
  ['empty bytes', Buffer.alloc(0), 'malformed_signed_transaction'],
  ['wrong version', wrongVersion, 'malformed_signed_transaction'],
  ['truncated final field', canonicalEnvelope.subarray(0, -1), 'malformed_payload'],
  ['trailing field bytes', Buffer.concat([canonicalEnvelope, Buffer.of(0)]), 'malformed_payload'],
  ['overlong signature length', overlongSignatureLength, 'malformed_payload'],
  ['empty intent', envelope(Buffer.alloc(0)), 'malformed_signed_transaction'],
  ['multisig attachment', envelope(payload, Buffer.of(1)), 'malformed_signed_transaction'],
]) {
  test(`transfer-only and general hash reject ${label} at the shared frame boundary`, () => {
    let originalError;
    for (const hash of [browserSignedTransactionHashHex, _browserSignedTransferTransactionHashHex]) {
      assert.throws(() => hash(value), error => {
        assert.ok(error instanceof BrowserTransactionCodecError);
        assert.ok(error instanceof TypeError);
        assert.equal(error.code, code);
        if (originalError) assert.equal(error.message, originalError.message);
        originalError = error;
        return true;
      });
    }
  });
}
