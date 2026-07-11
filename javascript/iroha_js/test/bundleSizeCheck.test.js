// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import test from "node:test";

import {
  BUNDLE_TARGETS,
  findForbiddenBrowserInputs,
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

test("bundle-size check gates the complete public browser aggregate", () => {
  const target = BUNDLE_TARGETS.find(({ label }) => label.includes("public aggregate"));
  assert.ok(target, "public browser aggregate bundle target is required");
  assert.equal(target.platform, "browser");
  assert.match(target.entryPoint, /dist[/\\]browser\.js$/u);
  assert.equal(target.forbidNodeInputs, true);
  assert.ok(target.limitKb > 0 && target.limitKb <= 300);
});

test("bundle-size check gates canonical requests as a browser subpath", () => {
  const target = BUNDLE_TARGETS.find(({ label }) => label.includes("canonicalRequest"));
  assert.ok(target, "browser canonical-request bundle target is required");
  assert.equal(target.platform, "browser");
  assert.match(target.entryPoint, /dist[/\\]canonicalRequest\.js$/u);
  assert.equal(target.forbidNodeInputs, true);
  assert.ok(target.limitKb > 0 && target.limitKb <= 75);
});

test("bundle-size check gates the IVM artifact helper as a browser leaf", () => {
  const target = BUNDLE_TARGETS.find(({ label }) => label.includes("ivmArtifact"));
  assert.ok(target, "browser IVM artifact bundle target is required");
  assert.equal(target.platform, "browser");
  assert.match(target.entryPoint, /dist[/\\]ivmArtifact\.js$/u);
  assert.equal(target.forbidNodeInputs, true);
  assert.equal(target.forbidGlobalBuffer, true);
  assert.ok(target.limitKb > 0 && target.limitKb <= 12);
});

test("browser graph guard detects every forbidden Node-only edge", () => {
  const candidates = [
    "node:crypto",
    "/package/dist/crypto.js",
    "/package/dist/cryptoHash.js",
    "/package/dist/native.js",
    "/package/dist/toriiClient.js",
    "/package/dist/crypto.browser.js",
    "/package/dist/native.browser.js",
    "/package/dist/toriiBrowserClient.js",
  ];
  assert.deepEqual(findForbiddenBrowserInputs(candidates), candidates.slice(0, 5));
});

test("public browser aggregate bundles without Node inputs or global Buffer shims", async () => {
  const target = BUNDLE_TARGETS.find(({ label }) => label.includes("public aggregate"));
  assert.ok(target);
  const { build } = await import("esbuild");
  const result = await build({
    entryPoints: [target.entryPoint],
    bundle: true,
    write: false,
    platform: "browser",
    target: target.target,
    format: "esm",
    treeShaking: true,
    sourcemap: false,
    minify: true,
    metafile: true,
  });
  assert.deepEqual(
    findForbiddenBrowserInputs(Object.keys(result.metafile.inputs)),
    [],
  );
  assert.ok(result.outputFiles[0].contents.byteLength <= target.limitKb * 1024);
  assert.doesNotMatch(
    result.outputFiles[0].text,
    /(?:globalThis|window|global)\.Buffer\s*=/u,
  );
});

test("IVM artifact browser leaf stays below 12 KiB without Node or Buffer shims", async () => {
  const target = BUNDLE_TARGETS.find(({ label }) => label.includes("ivmArtifact"));
  assert.ok(target);
  const { build } = await import("esbuild");
  const result = await build({
    entryPoints: [target.entryPoint],
    bundle: true,
    write: false,
    platform: "browser",
    target: target.target,
    format: "esm",
    treeShaking: true,
    sourcemap: false,
    minify: true,
    metafile: true,
  });
  assert.deepEqual(
    findForbiddenBrowserInputs(Object.keys(result.metafile.inputs)),
    [],
  );
  assert.ok(result.outputFiles[0].contents.byteLength <= 12 * 1024);
  assert.doesNotMatch(
    result.outputFiles[0].text,
    /(?:globalThis|window|global)\.Buffer\s*=/u,
  );
});
