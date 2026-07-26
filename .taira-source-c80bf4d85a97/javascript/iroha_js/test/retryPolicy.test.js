import assert from "node:assert/strict";
import test from "node:test";

import {
  DEFAULT_RETRY_PROFILE_PIPELINE,
  DEFAULT_RETRY_PROFILE_STREAMING,
  DEFAULT_TORII_CLIENT_CONFIG,
  resolveToriiClientConfig,
} from "../src/config.js";

test("default retry profile exports stay aligned with policy", () => {
  assert.equal(
    DEFAULT_RETRY_PROFILE_PIPELINE.maxRetries,
    5,
    "pipeline profile maxRetries should match policy",
  );
  assert.deepEqual(
    DEFAULT_RETRY_PROFILE_PIPELINE.retryStatuses,
    [408, 425, 429, 500, 502, 503, 504],
    "pipeline retryable statuses should match policy",
  );
  assert.deepEqual(
    DEFAULT_RETRY_PROFILE_PIPELINE.retryMethods,
    ["GET", "POST", "HEAD"],
    "pipeline retryable methods should match policy",
  );

  assert.equal(
    DEFAULT_RETRY_PROFILE_STREAMING.maxRetries,
    6,
    "streaming profile maxRetries should match policy",
  );
  assert.deepEqual(
    DEFAULT_RETRY_PROFILE_STREAMING.retryStatuses,
    [408, 425, 429, 500, 502, 503, 504],
    "streaming retryable statuses should match policy",
  );
  assert.deepEqual(
    DEFAULT_RETRY_PROFILE_STREAMING.retryMethods,
    ["GET"],
    "streaming retryable methods should match policy",
  );
});

test("resolveToriiClientConfig populates retry profile map from defaults", () => {
  const resolved = resolveToriiClientConfig();
  assert.equal(resolved.maxRetries, DEFAULT_TORII_CLIENT_CONFIG.maxRetries);

  const pipeline = resolved.retryProfiles.pipeline;
  assert.equal(pipeline.maxRetries, DEFAULT_RETRY_PROFILE_PIPELINE.maxRetries);
  assert.deepEqual(
    Array.from(pipeline.retryStatuses).sort(),
    [...DEFAULT_RETRY_PROFILE_PIPELINE.retryStatuses].sort(),
  );
  assert.deepEqual(
    Array.from(pipeline.retryMethods).sort(),
    [...DEFAULT_RETRY_PROFILE_PIPELINE.retryMethods].sort(),
  );

  const streaming = resolved.retryProfiles.streaming;
  assert.equal(streaming.maxRetries, DEFAULT_RETRY_PROFILE_STREAMING.maxRetries);
  assert.deepEqual(
    Array.from(streaming.retryMethods).sort(),
    [...DEFAULT_RETRY_PROFILE_STREAMING.retryMethods].sort(),
  );
});

test("resolveToriiClientConfig applies retry profile overrides", () => {
  const resolved = resolveToriiClientConfig({
    config: {
      retryProfiles: {
        pipeline: {
          maxRetries: 9,
          backoffInitialMs: 150,
          retryMethods: ["get", "post"],
          retryStatuses: [502, 503],
        },
      },
    },
  });

  const pipeline = resolved.retryProfiles.pipeline;
  assert.equal(pipeline.maxRetries, 9);
  assert.equal(pipeline.backoffInitialMs, 150);
  assert.deepEqual(Array.from(pipeline.retryMethods).sort(), ["GET", "POST"]);
  assert.deepEqual(Array.from(pipeline.retryStatuses).sort(), [502, 503]);
});

test("resolveToriiClientConfig rejects fractional retry integers", () => {
  assert.throws(
    () =>
      resolveToriiClientConfig({
        config: { toriiClient: { maxRetries: 1.5 } },
      }),
    /maxRetries/,
  );
});
