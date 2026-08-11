import { test } from "node:test";
import assert from "node:assert/strict";

import { normalizeIntegrationString, parseBooleanEnv } from "./integrationToriiEnv.js";

test("parseBooleanEnv recognizes disabled integration switches", () => {
  assert.equal(parseBooleanEnv(null), false);
  assert.equal(parseBooleanEnv(""), false);
  assert.equal(parseBooleanEnv("0"), false);
  assert.equal(parseBooleanEnv(" FALSE "), false);
});

test("parseBooleanEnv preserves permissive enabled switches", () => {
  assert.equal(parseBooleanEnv("1"), true);
  assert.equal(parseBooleanEnv("true"), true);
  assert.equal(parseBooleanEnv("enabled"), true);
});

test("normalizeIntegrationString rejects absent and blank values", () => {
  assert.equal(normalizeIntegrationString(null), null);
  assert.equal(normalizeIntegrationString("  "), null);
  assert.equal(normalizeIntegrationString(" value "), "value");
});
