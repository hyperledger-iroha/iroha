import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildDeviceCodeRequestPayload,
  buildTokenRequestPayload,
  computeExpiryTimestamp,
  formatDuration,
  normaliseIntervalMs,
} from '../deviceAuth.js';

test('buildDeviceCodeRequestPayload filters optional fields', () => {
  const payload = buildDeviceCodeRequestPayload({
    clientId: 'demo',
    scope: 'openid offline_access',
    audience: '',
  });
  assert.equal(payload.get('client_id'), 'demo');
  assert.equal(payload.get('scope'), 'openid offline_access');
  assert.equal(payload.has('audience'), false);
});

test('buildTokenRequestPayload sets the grant type', () => {
  const payload = buildTokenRequestPayload({clientId: 'demo', deviceCode: 'abc'});
  assert.equal(payload.get('client_id'), 'demo');
  assert.equal(payload.get('device_code'), 'abc');
  assert.equal(payload.get('grant_type'), 'urn:ietf:params:oauth:grant-type:device_code');
});

test('normaliseIntervalMs enforces sane defaults', () => {
  assert.equal(normaliseIntervalMs(2, 5000), 2000);
  assert.equal(normaliseIntervalMs('0', 3000), 3000);
  assert.equal(normaliseIntervalMs(-1, 2500), 2500);
  assert.equal(normaliseIntervalMs(undefined, 4000), 4000);
});

test('computeExpiryTimestamp uses provided clock and fallback', () => {
  const now = 1_000;
  assert.equal(computeExpiryTimestamp(10, 5, now), now + 10_000);
  assert.equal(computeExpiryTimestamp(undefined, 5, now), now + 5_000);
  assert.equal(computeExpiryTimestamp(-2, 5, now), now + 5_000);
});

test('formatDuration prints compact strings', () => {
  assert.equal(formatDuration(5), '5s');
  assert.equal(formatDuration(65), '1m 5s');
  assert.equal(formatDuration(-10), '0s');
});
