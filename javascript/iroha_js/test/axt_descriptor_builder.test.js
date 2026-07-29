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

test("buildTouchManifest trims Rust whitespace and drops empty keys", () => {
  const manifest = buildTouchManifest(
    [
      "",
      " ",
      "\t\n",
      "\u0085",
      "\u3000",
      "  reports/monthly  ",
      "\u1680reports/monthly\u205f",
    ],
    ["\u2003audits/summary\u202f", "\r\n", "audits/summary"],
  );
  assert.deepEqual(manifest, {
    read: ["reports/monthly"],
    write: ["audits/summary"],
  });
});

test("buildTouchManifest uses Rust UTF-8 ordering for Unicode keys", () => {
  const bmpPrivateUse = "\ue000/bmp-private-use";
  const astral = "\u{10000}/astral";
  assert.ok(astral < bmpPrivateUse, "fixture must differ under JavaScript UTF-16 ordering");

  const manifest = buildTouchManifest(
    [astral, bmpPrivateUse, ` ${astral} `],
    [],
  );

  assert.deepEqual(manifest, {
    read: [bmpPrivateUse, astral],
    write: [],
  });
});

test("buildTouchManifest retains non-Rust BOM code points", () => {
  const manifest = buildTouchManifest(["\ufeff", "\ufeffpath\ufeff"], []);
  assert.deepEqual(manifest, {
    read: ["\ufeff", "\ufeffpath\ufeff"],
    write: [],
  });
});

test("buildTouchManifest rejects unpaired UTF-16 surrogates", () => {
  for (const invalid of ["\ud800", "prefix\udfff", "\ud800suffix"]) {
    assert.throws(
      () => buildTouchManifest([invalid], []),
      /read\[0\] must contain only Unicode scalar values/u,
    );
  }
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

maybeNativeTest("buildAxtDescriptor canonicalises through the native binding", () => {
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
  assert.equal(result.binding?.length, 32);
  assert.ok(Buffer.isBuffer(result.descriptorBytes));
  assert.equal(result.native, true);
});

maybeNativeTest("buildAxtDescriptor canonicalises adversarial touch paths like Rust", () => {
  const bmpPrivateUse = "\ue000/bmp-private-use";
  const astral = "\u{10000}/astral";
  const result = buildAxtDescriptor({
    dsids: [7],
    touches: [
      {
        dsid: 7,
        read: [" ", ` ${astral} `, bmpPrivateUse, astral],
        write: ["\u3000", "\u2003write/path\u202f"],
      },
    ],
    touchManifest: [
      {
        dsid: 7,
        read: ["\u0085", ` ${astral} `, bmpPrivateUse, astral],
        write: ["\t", "\u2003write/path\u202f"],
      },
    ],
  });

  const canonical = {
    read: [bmpPrivateUse, astral],
    write: ["write/path"],
  };
  assert.deepEqual(result.descriptor.touches, [{ dsid: 7, ...canonical }]);
  assert.deepEqual(result.touchManifest, [{ dsid: 7, manifest: canonical }]);
});

maybeNativeTest("buildAxtDescriptor accepts array-like iterables", () => {
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
});
