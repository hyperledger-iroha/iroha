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

test("normalizeAxtRejectContext preserves minima and ids", () => {
  const ctx = normalizeAxtRejectContext({
    reason: "era",
    dataspace: 7,
    target_lane: 2,
    snapshot_version: 55,
    detail: "stale handle",
    next_min_handle_era: 9,
    next_min_sub_nonce: 4,
  });
  assert.equal(ctx.reason, "era");
  assert.equal(ctx.dataspace, 7);
  assert.equal(ctx.lane, 2);
  assert.equal(ctx.snapshot_version, 55);
  assert.equal(ctx.detail, "stale handle");
  assert.equal(ctx.next_min_handle_era, 9);
  assert.equal(ctx.next_min_sub_nonce, 4);
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
        nextMinHandleEra: 12,
        nextMinSubNonce: 6,
      }),
    {
      name: "TypeError",
      message: /dataspace/,
    },
  );
});

test("buildHandleRefreshRequest applies overrides", () => {
  const base = {
    reason: "era",
    dataspace: 1,
    lane: 4,
    next_min_handle_era: 3,
    next_min_sub_nonce: 2,
    snapshot_version: 9,
    detail: "era too low",
  };
  const request = buildHandleRefreshRequest(base, {
    targetLane: 5,
    nextMinHandleEra: 7,
    reason: "sub_nonce",
  });
  assert.deepEqual(request, {
    dataspace: 1,
    targetLane: 5,
    nextMinHandleEra: 7,
    nextMinSubNonce: 2,
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

test("buildAxtDescriptor canonicalises inputs without native binding", () => {
  const result = buildAxtDescriptor({
    dsids: [7, 1, 7],
    touches: [
      { dsid: 7, read: ["reports/"], write: ["audits/", "aggregates/", "audits/"] },
      { dsid: 1, read: ["orders/", "payments/"], write: ["ledger/"] },
    ],
    descriptorBytes: Buffer.from(DESCRIPTOR_FIXTURE.descriptor_hex, "hex"),
  });
  assert.deepEqual(result.descriptor.dsids, [1, 7]);
  assert.equal(result.bindingHex, DESCRIPTOR_FIXTURE.binding_hex);
});
