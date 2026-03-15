import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildProxiedUrl,
  formatProxyAuth,
  normaliseProxyBase,
} from '../tryItProxy.js';

test('normaliseProxyBase trims whitespace and trailing slash', () => {
  assert.equal(normaliseProxyBase(' https://proxy.example/ '), 'https://proxy.example');
  assert.equal(normaliseProxyBase('http://localhost:8787/'), 'http://localhost:8787');
  assert.equal(normaliseProxyBase(''), '');
  assert.equal(normaliseProxyBase(null), '');
});

test('buildProxiedUrl handles absolute URLs', () => {
  const result = buildProxiedUrl('https://proxy.example', 'https://torii.example/v2/status');
  assert.equal(result, 'https://proxy.example/proxy/v2/status');
});

test('buildProxiedUrl accepts relative paths', () => {
  assert.equal(
    buildProxiedUrl('http://localhost:8787', '/v2/blocks?limit=1'),
    'http://localhost:8787/proxy/v2/blocks?limit=1',
  );
  assert.equal(
    buildProxiedUrl('http://localhost:8787/', 'v1/accounts'),
    'http://localhost:8787/proxy/v2/accounts',
  );
});

test('buildProxiedUrl returns null when proxy disabled', () => {
  assert.equal(buildProxiedUrl('', '/v2/status'), null);
  assert.equal(buildProxiedUrl('', 'https://torii.example/v2/status'), null);
});

test('buildProxiedUrl avoids double proxy prefix', () => {
  assert.equal(
    buildProxiedUrl('https://proxy.example', '/proxy/v2/status'),
    'https://proxy.example/proxy/v2/status',
  );
});

test('formatProxyAuth trims but keeps content intact', () => {
  assert.equal(formatProxyAuth(' Bearer token '), 'Bearer token');
  assert.equal(formatProxyAuth(''), '');
  assert.equal(formatProxyAuth(null), '');
});
