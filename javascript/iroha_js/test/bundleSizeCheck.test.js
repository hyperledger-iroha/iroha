// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import test from "node:test";

import {
  BUNDLE_TARGETS,
  runBundleSizeCheck,
} from "../scripts/bundle-size-check.mjs";

test("bundle-size check fails closed when esbuild cannot be resolved", async () => {
  await assert.rejects(
    runBundleSizeCheck({
      loadEsbuild: async () => {
        throw new Error("simulated missing esbuild");
      },
      log() {},
    }),
    /requires the pinned esbuild devDependency/u,
  );
});

test("bundle-size check covers the browser transaction codec", () => {
  const target = BUNDLE_TARGETS.find(({ label }) => label.includes("transactionCodec"));
  assert.ok(target, "browser transaction-codec bundle target is required");
  assert.equal(target.platform, "browser");
  assert.match(target.entryPoint, /src[/\\]transactionCodec\.js$/u);
  assert.ok(target.limitKb > 0 && target.limitKb <= 132);
});

test("bundle-size check proves the Nexus app export has a browser-only graph", () => {
  const target = BUNDLE_TARGETS.find(({ label }) => label.includes("nexusApp"));
  assert.ok(target, "browser Nexus app bundle target is required");
  assert.equal(target.platform, "browser");
  assert.match(target.entryPoint, /src[/\\]nexusApp\.js$/u);
  assert.ok(target.limitKb > 0 && target.limitKb <= 205);
});
