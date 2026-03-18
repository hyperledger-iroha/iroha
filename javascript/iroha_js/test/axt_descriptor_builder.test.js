import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { buildAxtDescriptor, buildTouchManifest, computeAxtBinding } from "../src/axt.js";
import { makeNativeTest } from "./helpers/native.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const FIXTURE_PATH = path.resolve(
  __dirname,
  "..",
  "..",
  "..",
  "crates/iroha_data_model/tests/fixtures/axt_descriptor_multi_ds.json",
);

const maybeNativeTest = makeNativeTest(test);

test("buildTouchManifest sorts and deduplicates keys", () => {
  const manifest = buildTouchManifest(
    ["reports/monthly", "reports/monthly"],
    ["audits/summary", "aggregates/monthly", "audits/summary"],
  );
  assert.deepEqual(manifest, {
    read: ["reports/monthly"],
    write: ["aggregates/monthly", "audits/summary"],
  });
});

test("buildTouchManifest accepts array-like inputs", () => {
  const arrayLike = { 0: "alpha", 1: "beta", length: 2 };
  const manifest = buildTouchManifest(arrayLike, { 0: "gamma", length: 1 });
  assert.deepEqual(manifest, { read: ["alpha", "beta"], write: ["gamma"] });
});

maybeNativeTest("buildAxtDescriptor matches the golden fixture", () => {
  const fixture = JSON.parse(readFileSync(FIXTURE_PATH, "utf8"));
  const result = buildAxtDescriptor({
    dsids: [7, 1, 7],
    touches: [
      {
        dsid: 7,
        read: ["reports/", "reports/"],
        write: ["audits/", "aggregates/", "audits/"],
      },
      {
        dsid: 1,
        read: ["payments/", "orders/", "orders/"],
        write: ["ledger/"],
      },
    ],
    touchManifest: fixture.touch_manifest,
  });

  assert.deepEqual(result.descriptor, fixture.descriptor);
  assert.deepEqual(result.touchManifest, fixture.touch_manifest);
  assert.equal(result.bindingHex, fixture.binding_hex);
  assert.equal(
    result.binding?.toString("hex"),
    fixture.binding_hex,
    "binding buffer should match fixture hex",
  );
  assert.equal(
    result.descriptorBytes?.toString("hex"),
    fixture.descriptor_hex,
    "descriptor bytes should match fixture Norito encoding",
  );
  assert.equal(result.binding?.length, 32);
  assert.ok(Buffer.isBuffer(result.binding), "binding must be a buffer");
  assert.ok(Buffer.isBuffer(result.descriptorBytes), "descriptor bytes must be a buffer");
  assert.equal(result.native, true);
});

test("computeAxtBinding hashes Norito descriptor bytes to the fixture binding", () => {
  const fixture = JSON.parse(readFileSync(FIXTURE_PATH, "utf8"));
  const binding = computeAxtBinding(Buffer.from(fixture.descriptor_hex, "hex"));
  assert.equal(binding.toString("hex"), fixture.binding_hex);
});

test("buildAxtDescriptor canonicalises without the native binding", () => {
  const previous = process.env.IROHA_JS_DISABLE_NATIVE;
  process.env.IROHA_JS_DISABLE_NATIVE = "1";

  const result = buildAxtDescriptor({
    dsids: [2, 2],
    touches: [{ dsid: 2, read: ["alpha", "alpha"], write: ["beta"] }],
    touchManifest: [{ dsid: 2, read: ["alpha/x"], write: ["beta/y"] }],
  });

  assert.deepEqual(result.descriptor, {
    dsids: [2],
    touches: [{ dsid: 2, read: ["alpha"], write: ["beta"] }],
  });
  assert.deepEqual(result.touchManifest, [
    { dsid: 2, manifest: { read: ["alpha/x"], write: ["beta/y"] } },
  ]);
  assert.equal(result.bindingHex, null);
  assert.equal(result.binding, null);
  assert.equal(result.descriptorBytes, null);
  assert.equal(result.native, false);

  if (previous === undefined) {
    delete process.env.IROHA_JS_DISABLE_NATIVE;
  } else {
    process.env.IROHA_JS_DISABLE_NATIVE = previous;
  }
});

test("buildAxtDescriptor accepts array-like iterables", () => {
  const previous = process.env.IROHA_JS_DISABLE_NATIVE;
  process.env.IROHA_JS_DISABLE_NATIVE = "1";

  const dsids = { 0: 5, length: 1 };
  const touches = {
    0: {
      dsid: 5,
      read: { 0: "alpha", length: 1 },
      write: { 0: "beta", length: 1 },
    },
    length: 1,
  };
  const touchManifest = {
    0: {
      dsid: 5,
      read: { 0: "alpha/x", length: 1 },
      write: { 0: "beta/y", length: 1 },
    },
    length: 1,
  };

  const result = buildAxtDescriptor({ dsids, touches, touchManifest });

  assert.deepEqual(result.descriptor, {
    dsids: [5],
    touches: [{ dsid: 5, read: ["alpha"], write: ["beta"] }],
  });
  assert.deepEqual(result.touchManifest, [
    { dsid: 5, manifest: { read: ["alpha/x"], write: ["beta/y"] } },
  ]);

  if (previous === undefined) {
    delete process.env.IROHA_JS_DISABLE_NATIVE;
  } else {
    process.env.IROHA_JS_DISABLE_NATIVE = previous;
  }
});
