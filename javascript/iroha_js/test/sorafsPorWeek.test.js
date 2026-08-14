import assert from "node:assert/strict";
import test from "node:test";

import { normalizeIsoWeekLabel } from "../src/sorafsPorWeek.js";

const Client = {
  _normalizeUnsignedInteger(value, name, { allowZero }) {
    assert.equal(allowZero, false);
    if (!Number.isSafeInteger(value) || value < 1) {
      throw new TypeError(`${name} must be a positive safe integer`);
    }
    return value;
  },
};

test("normalizes canonical string and structured ISO weeks", () => {
  assert.equal(normalizeIsoWeekLabel(" 2026-W08 ", "week", Client), "2026-W08");
  assert.equal(
    normalizeIsoWeekLabel({ year: 2026, week: 8 }, "week", Client),
    "2026-W08",
  );
});

test("rejects malformed and out-of-range ISO weeks", () => {
  assert.throws(() => normalizeIsoWeekLabel("2026-08", "week", Client));
  assert.throws(() => normalizeIsoWeekLabel({ year: 2026, week: 54 }, "week", Client));
  assert.throws(() => normalizeIsoWeekLabel(null, "week", Client));
});
