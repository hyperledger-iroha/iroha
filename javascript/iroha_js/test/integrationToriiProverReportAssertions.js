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

export function assertProverReportResult(result, label = "prover report response") {
  assert.ok(result && typeof result === "object", `${label} must be an object`);
  switch (result.kind) {
    case "reports":
      assert.ok(
        Array.isArray(result.reports),
        `${label}.reports must be an array when kind is reports`,
      );
      result.reports.forEach((entry, index) =>
        assertProverReportRecord(entry, `${label}.reports[${index}]`),
      );
      break;
    case "ids":
      assert.ok(Array.isArray(result.ids), `${label}.ids must be an array when kind is ids`);
      result.ids.forEach((value, index) => {
        assert.ok(
          isNonEmptyString(value),
          `${label}.ids[${index}] must be a non-empty string`,
        );
      });
      break;
    case "messages":
      assert.ok(
        Array.isArray(result.messages),
        `${label}.messages must be an array when kind is messages`,
      );
      result.messages.forEach((entry, index) => {
        assert.ok(isPlainObject(entry), `${label}.messages[${index}] must be an object`);
        assert.ok(
          isNonEmptyString(entry.id),
          `${label}.messages[${index}].id must be a non-empty string`,
        );
        if (entry.error !== null) {
          assert.equal(
            typeof entry.error,
            "string",
            `${label}.messages[${index}].error must be null or a string`,
          );
        }
      });
      break;
    default:
      throw new Error(`${label} has unknown kind: ${String(result.kind)}`);
  }
}

function assertProverReportRecord(entry, label) {
  assert.ok(entry && typeof entry === "object", `${label} must be an object`);
  assert.ok(isNonEmptyString(entry.id), `${label}.id must be a non-empty string`);
  assert.equal(typeof entry.ok, "boolean", `${label}.ok must be a boolean`);
  if (entry.error !== null) {
    assert.equal(typeof entry.error, "string", `${label}.error must be null or a string`);
  }
  assert.ok(isNonEmptyString(entry.content_type), `${label}.content_type must be a string`);
  assertNonNegativeInteger(entry.size, `${label}.size`);
  assertNonNegativeInteger(entry.created_ms, `${label}.created_ms`);
  assertNonNegativeInteger(entry.processed_ms, `${label}.processed_ms`);
  assertNonNegativeInteger(entry.latency_ms, `${label}.latency_ms`);
  if (entry.zk1_tags !== null) {
    assert.ok(Array.isArray(entry.zk1_tags), `${label}.zk1_tags must be an array when present`);
    entry.zk1_tags.forEach((tag, index) => {
      assert.ok(
        isNonEmptyString(tag),
        `${label}.zk1_tags[${index}] must be a non-empty string`,
      );
    });
  }
}

export function hasProverReportEntries(result) {
  switch (result.kind) {
    case "reports":
      return Array.isArray(result.reports) && result.reports.length > 0;
    case "ids":
      return Array.isArray(result.ids) && result.ids.length > 0;
    case "messages":
      return Array.isArray(result.messages) && result.messages.length > 0;
    default:
      return false;
  }
}

export function countFailedProverReports(result) {
  if (result.kind !== "reports" || !Array.isArray(result.reports)) {
    return 0;
  }
  return result.reports.filter((entry) => entry && entry.ok === false).length;
}
