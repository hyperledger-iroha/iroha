import assert from "node:assert/strict";

export function isNonEmptyString(value) {
  return typeof value === "string" && value.trim().length > 0;
}

export function isPlainObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

export function assertNonNegativeInteger(value, label) {
  assert.ok(Number.isInteger(value) && value >= 0, `${label} must be a non-negative integer`);
}
