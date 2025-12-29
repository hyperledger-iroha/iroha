import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildAnalyticsPayload,
  normaliseSampleRate,
  shouldSendEvent,
} from '../analytics.js';

test('normaliseSampleRate clamps to [0, 1]', () => {
  assert.equal(normaliseSampleRate(-1), 0);
  assert.equal(normaliseSampleRate(0), 0);
  assert.equal(normaliseSampleRate(0.5), 0.5);
  assert.equal(normaliseSampleRate(2), 1);
  assert.equal(normaliseSampleRate('0.2'), 0.2);
  assert.equal(normaliseSampleRate('not-a-number'), 0);
});

test('shouldSendEvent honours sampling thresholds', () => {
  assert.equal(shouldSendEvent(0), false);
  assert.equal(shouldSendEvent(1), true);
  const deterministicRandom = () => 0.25;
  assert.equal(shouldSendEvent(0.5, deterministicRandom), true);
  assert.equal(shouldSendEvent(0.2, deterministicRandom), false);
});

test('buildAnalyticsPayload formats timestamps and metadata', () => {
  const ts = new Date('2025-03-01T10:00:00.000Z');
  const payload = buildAnalyticsPayload({
    path: '/reference',
    locale: 'en',
    release: 'release-123',
    event: 'custom',
    timestamp: ts,
  });
  assert.deepEqual(payload, {
    event: 'custom',
    path: '/reference',
    locale: 'en',
    release: 'release-123',
    ts: '2025-03-01T10:00:00.000Z',
  });
});
