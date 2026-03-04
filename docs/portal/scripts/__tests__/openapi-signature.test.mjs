import assert from 'node:assert/strict';
import test from 'node:test';

import {verifyOpenApiSignature} from '../lib/openapi-signature.mjs';
import {signPayload} from './helpers/openapi-signing.mjs';

test('verifyOpenApiSignature accepts valid signatures', () => {
  const payload = Buffer.from('{"openapi":"3.1.0"}', 'utf8');
  const signature = signPayload(payload);
  assert.doesNotThrow(() =>
    verifyOpenApiSignature({
      algorithm: 'ed25519',
      publicKeyHex: signature.publicKeyHex,
      signatureHex: signature.signatureHex,
      payload,
    }),
  );
});

test('verifyOpenApiSignature rejects tampered payload', () => {
  const payload = Buffer.from('{"openapi":"3.1.0"}', 'utf8');
  const signature = signPayload(payload);
  assert.throws(() =>
    verifyOpenApiSignature({
      algorithm: 'ed25519',
      publicKeyHex: signature.publicKeyHex,
      signatureHex: signature.signatureHex,
      payload: Buffer.from('{"openapi":"3.1.0","tampered":true}', 'utf8'),
    }), /signature verification failed/i);
});
