/**
 * Helper utilities for the docs analytics pipeline.
 *
 * These functions stay dependency-free so we can exercise them with the
 * lightweight Node test suite that already covers other widgets.
 */

/**
 * Clamp a configured sample rate to the inclusive [0, 1] range.
 *
 * @param {number|string|undefined} value
 * @returns {number}
 */
export function normaliseSampleRate(value) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return 0;
  }
  if (parsed >= 1) {
    return 1;
  }
  return parsed;
}

/**
 * Decide whether an analytics event should be sent according to the sampling
 * rate. Accepts an injectable RNG so tests can assert behaviour deterministically.
 *
 * @param {number} sampleRate
 * @param {() => number} randomFn
 * @returns {boolean}
 */
export function shouldSendEvent(sampleRate, randomFn = Math.random) {
  if (sampleRate <= 0) {
    return false;
  }
  if (sampleRate >= 1) {
    return true;
  }
  return randomFn() < sampleRate;
}

/**
 * Build the payload emitted by the analytics tracker. The timestamp can be
 * injected for tests; otherwise the current time is used.
 *
 * @param {object} options
 * @param {string} options.path
 * @param {string} options.locale
 * @param {string} options.release
 * @param {string} [options.event]
 * @param {Date} [options.timestamp]
 * @returns {{event: string, path: string, locale: string, release: string, ts: string}}
 */
export function buildAnalyticsPayload({
  path,
  locale,
  release,
  event = 'pageview',
  timestamp = new Date(),
}) {
  return {
    event,
    path,
    locale,
    release,
    ts: timestamp.toISOString(),
  };
}
