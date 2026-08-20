import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve as resolvePath } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import {
  buildAxtDescriptor,
  buildHandleRefreshRequest,
  computeAxtBinding,
  normalizeAxtRejectContext,
} from "../src/axt.js";

const MODULE_DIR = dirname(fileURLToPath(import.meta.url));
const DESCRIPTOR_FIXTURE = JSON.parse(
  readFileSync(
    resolvePath(
      MODULE_DIR,
      "../../../crates/iroha_data_model/tests/fixtures/axt_descriptor_multi_ds.json",
    ),
    "utf8",
  ),
);

test("normalizeAxtRejectContext preserves exact policy hints and ids", () => {
  const ctx = normalizeAxtRejectContext({
    reason: "era",
    dataspace: 7,
    lane: 2,
    snapshot_version: 55,
    detail: "stale handle",
    active_handle_era: 9,
    next_handle_counter: 4,
  });
  assert.equal(ctx.reason, "era");
  assert.equal(ctx.dataspace, 7);
  assert.equal(ctx.lane, 2);
  assert.equal(ctx.snapshot_version, 55);
  assert.equal(ctx.detail, "stale handle");
  assert.equal(ctx.active_handle_era, 9);
  assert.equal(ctx.next_handle_counter, 4);
});

test("normalizeAxtRejectContext rejects camelCase fields", () => {
  assert.throws(
    () =>
      normalizeAxtRejectContext({
        reason: "sub_nonce",
        dataspaceId: 11,
        targetLane: 3,
        snapshotVersion: 101,
        detail: null,
        activeHandleEra: 12,
        nextHandleCounter: 6,
      }),
    {
      name: "TypeError",
      message: /canonical AXT fields/,
    },
  );
});

test("normalizeAxtRejectContext rejects retired minimum terminology", () => {
  assert.throws(
    () =>
      normalizeAxtRejectContext({
        reason: "era",
        next_min_handle_era: 12,
        next_min_sub_nonce: 6,
      }),
    {
      name: "TypeError",
      message: /canonical AXT fields/,
    },
  );
});

test("normalizeAxtRejectContext requires the exact closed V1 layout", () => {
  const exact = {
    reason: "policy",
    dataspace: null,
    lane: null,
    snapshot_version: null,
    detail: "policy rejected",
    active_handle_era: null,
    next_handle_counter: null,
  };
  for (const field of Object.keys(exact)) {
    const shortened = { ...exact };
    delete shortened[field];
    assert.throws(() => normalizeAxtRejectContext(shortened), {
      name: "TypeError",
      message: new RegExp(`${field} is required`),
    });
  }
  assert.throws(
    () => normalizeAxtRejectContext({ ...exact, pre_release_field: null }),
    {
      name: "TypeError",
      message: /not a canonical AXT field/,
    },
  );
  assert.deepEqual(normalizeAxtRejectContext(exact), exact);
});

test("buildHandleRefreshRequest applies overrides", () => {
  const base = {
    reason: "era",
    dataspace: 1,
    lane: 4,
    active_handle_era: 3,
    next_handle_counter: 2,
    snapshot_version: 9,
    detail: "era too low",
  };
  const request = buildHandleRefreshRequest(base, {
    targetLane: 5,
    activeHandleEra: 7,
    reason: "sub_nonce",
  });
  assert.deepEqual(request, {
    dataspace: 1,
    targetLane: 5,
    activeHandleEra: 7,
    nextHandleCounter: 2,
    reason: "sub_nonce",
    snapshotVersion: 9,
    detail: "era too low",
  });
});

test("computeAxtBinding matches fixture binding", () => {
  const descriptorBytes = Buffer.from(DESCRIPTOR_FIXTURE.descriptor_hex, "hex");
  const binding = computeAxtBinding(descriptorBytes);
  assert.equal(binding.toString("hex"), DESCRIPTOR_FIXTURE.binding_hex);
});

test("buildAxtDescriptor canonicalises inputs through native binding", () => {
  const result = buildAxtDescriptor({
    dsids: [7, 1, 7],
    touches: [
      { dsid: 7, read: ["reports/"], write: ["audits/", "aggregates/", "audits/"] },
      { dsid: 1, read: ["orders/", "payments/"], write: ["ledger/"] },
    ],
  });
  assert.deepEqual(result.descriptor.dsids, [1, 7]);
  assert.equal(result.bindingHex, DESCRIPTOR_FIXTURE.binding_hex);
  assert.equal(result.native, true);
});

test("buildAxtDescriptor rejects non-integer dsids", () => {
  assert.throws(
    () => buildAxtDescriptor({ dsids: [1.5] }),
    /dsids\[0\]/,
  );
});
