import assert from 'node:assert/strict';
import {Buffer} from 'node:buffer';
import test from 'node:test';

import {
  decodeProofBundle,
  verifySorafsBinding,
} from '../verify-sorafs-binding.mjs';

test('verifySorafsBinding validates alias, manifest, and proof status', async () => {
  const proofPayload = {
    alias: 'docs.sora',
    manifest: 'bafybeigdemo123',
  };
  const headers = {
    'sora-name': 'docs.sora',
    'sora-proof-status': 'ok',
    'sora-content-cid': 'bafycontentcid',
    'sora-proof': Buffer.from(JSON.stringify(proofPayload)).toString('base64'),
  };

  const summary = await verifySorafsBinding({
    url: 'https://docs.sora/.well-known/sorafs/manifest',
    expectedAlias: 'docs.sora',
    expectedManifest: 'bafybeigdemo123',
    expectedProofStatus: 'ok',
    expectedContentCid: 'bafycontentcid',
    fetchImpl: async () => new Response('ok', {status: 200, headers}),
  });

  assert.equal(summary.headers['sora-name'], 'docs.sora');
  assert.equal(summary.proof.manifest, 'bafybeigdemo123');
  assert.equal(summary.headers['sora-content-cid'], 'bafycontentcid');
});

test('verifySorafsBinding rejects alias mismatches', async () => {
  const proofPayload = {
    alias: 'docs.sora',
    manifest: 'bafybeigdemo123',
  };
  const headers = {
    'sora-name': 'docs.sora',
    'sora-proof-status': 'ok',
    'sora-content-cid': 'bafycontentcid',
    'sora-proof': Buffer.from(JSON.stringify(proofPayload)).toString('base64'),
  };

  await assert.rejects(
    () =>
      verifySorafsBinding({
        url: 'https://docs.sora/.well-known/sorafs/manifest',
        expectedAlias: 'gateway.sora',
        fetchImpl: async () => new Response('ok', {status: 200, headers}),
      }),
    /Sora-Name mismatch/,
  );
});

test('verifySorafsBinding validates Sora-Content-CID presence and value', async () => {
  const headers = {
    'sora-name': 'docs.sora',
    'sora-proof-status': 'ok',
    'sora-proof': Buffer.from(JSON.stringify({alias: 'docs.sora'})).toString('base64'),
  };
  await assert.rejects(
    () =>
      verifySorafsBinding({
        url: 'https://docs.sora/.well-known/sorafs/manifest',
        expectedAlias: 'docs.sora',
        fetchImpl: async () => new Response('ok', {status: 200, headers}),
      }),
    /missing Sora-Content-CID header/,
  );

  const badHeaders = {
    ...headers,
    'sora-content-cid': 'invalid-cid!*',
  };
  await assert.rejects(
    () =>
      verifySorafsBinding({
        url: 'https://docs.sora/.well-known/sorafs/manifest',
        expectedAlias: 'docs.sora',
        fetchImpl: async () => new Response('ok', {status: 200, headers: badHeaders}),
      }),
    /invalid Sora-Content-CID header/,
  );

  const mismatchHeaders = {
    ...headers,
    'sora-content-cid': 'bafycontentcid',
  };
  await assert.rejects(
    () =>
      verifySorafsBinding({
        url: 'https://docs.sora/.well-known/sorafs/manifest',
        expectedAlias: 'docs.sora',
        expectedContentCid: 'differentcid',
        fetchImpl: async () => new Response('ok', {status: 200, headers: mismatchHeaders}),
      }),
    /Sora-Content-CID mismatch/,
  );
});

test('decodeProofBundle reports invalid inputs', () => {
  assert.throws(
    () => decodeProofBundle('@@@'),
    /Sora-Proof header is not valid base64/,
  );
  assert.throws(
    () => decodeProofBundle(Buffer.from('{"alias":').toString('base64')),
    /Sora-Proof header is not valid JSON/,
  );
  assert.throws(
    () => decodeProofBundle(Buffer.from('null').toString('base64')),
    /not an object/,
  );
});
