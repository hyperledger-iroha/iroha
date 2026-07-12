// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  BUNDLE_TARGETS,
  findForbiddenBrowserInputs,
  hasForbiddenGlobalBufferMutation,
  listExplicitBrowserExports,
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

test("bundle-size targets retain audited ceilings and browser graph guards", () => {
  assert.deepEqual(
    BUNDLE_TARGETS.map(({ label, limitKb, forbidNodeInputs, forbidGlobalBuffer }) => ({
      label,
      limitKb,
      forbidNodeInputs: forbidNodeInputs === true,
      forbidGlobalBuffer: forbidGlobalBuffer === true,
    })),
    [
      {
        label: "toriiClient.js",
        limitKb: 864,
        forbidNodeInputs: false,
        forbidGlobalBuffer: false,
      },
      {
        label: "transactionCodec.js (browser)",
        limitKb: 132,
        forbidNodeInputs: true,
        forbidGlobalBuffer: true,
      },
      {
        label: "nexusApp.js (browser)",
        limitKb: 205,
        forbidNodeInputs: true,
        forbidGlobalBuffer: true,
      },
      {
        label: "canonicalRequest.js (browser)",
        limitKb: 75,
        forbidNodeInputs: true,
        forbidGlobalBuffer: true,
      },
      {
        label: "ivmArtifact.js (browser)",
        limitKb: 12,
        forbidNodeInputs: true,
        forbidGlobalBuffer: true,
      },
      {
        label: "kotodamaCompiler/browser.js (browser)",
        limitKb: 51,
        forbidNodeInputs: true,
        forbidGlobalBuffer: true,
      },
      {
        label: "browser.js (public aggregate)",
        limitKb: 328,
        forbidNodeInputs: true,
        forbidGlobalBuffer: true,
      },
    ],
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
  assert.ok(target.limitKb > 0 && target.limitKb <= 328);
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

test("bundle-size check gates the remote Kotodama compiler browser export", () => {
  const target = BUNDLE_TARGETS.find(({ label }) =>
    label.includes("kotodamaCompiler/browser"),
  );
  assert.ok(target, "browser Kotodama compiler bundle target is required");
  assert.equal(target.platform, "browser");
  assert.match(target.entryPoint, /dist[/\\]kotodamaCompiler[/\\]browser\.js$/u);
  assert.equal(target.forbidNodeInputs, true);
  assert.equal(target.forbidGlobalBuffer, true);
  assert.equal(target.limitKb, 51);
});

test("browser graph guard detects every forbidden Node-only edge", () => {
  const candidates = [
    "node:crypto",
    "src/crypto.js",
    "dist/cryptoHash.js",
    "src/native.js",
    "dist/toriiClient.js",
    "/package/dist/crypto.js",
    "/package/dist/cryptoHash.js",
    "/package/dist/native.js",
    "/package/dist/toriiClient.js",
    "/package/dist/crypto.browser.js",
    "/package/dist/native.browser.js",
    "/package/dist/toriiBrowserClient.js",
  ];
  assert.deepEqual(findForbiddenBrowserInputs(candidates), candidates.slice(0, 9));
});

test("global Buffer guard rejects assignment and property-definition bypasses", () => {
  for (const source of [
    "globalThis.Buffer = value",
    "window.Buffer ||= value",
    "global['Buffer'] ??= value",
    "self[\"Buffer\"] &&= value",
    "globalThis.Buffer++",
    "--window['Buffer']",
    'Object.defineProperty(globalThis, "Buffer", { value })',
    "Reflect.defineProperty(window, 'Buffer', { value })",
    "Object.defineProperties(global, { Buffer: { value } })",
    "Object.assign(self, { Buffer: value })",
  ]) {
    assert.equal(hasForbiddenGlobalBufferMutation(source), true, source);
  }
  assert.equal(hasForbiddenGlobalBufferMutation("const Buffer = LocalBuffer"), false);
  assert.equal(hasForbiddenGlobalBufferMutation("delete globalThis.Buffer"), false);
});

test("browser graph audit derives every explicit browser-conditioned package export", async () => {
  const pkg = JSON.parse(
    await readFile(new URL("../package.json", import.meta.url), "utf8"),
  );
  assert.deepEqual(listExplicitBrowserExports(pkg), [
    { target: "./dist/browser.js", subpaths: ["./browser"] },
    { target: "./dist/transactionCodec.js", subpaths: ["./transaction-codec"] },
    { target: "./dist/normalizers.js", subpaths: ["./normalizers"] },
    { target: "./dist/blake2b.js", subpaths: ["./blake2b"] },
    { target: "./dist/ivmArtifact.js", subpaths: ["./ivm-artifact"] },
    {
      target: "./dist/toriiBrowserClient.js",
      subpaths: ["./torii", "./torii-browser"],
    },
    { target: "./dist/canonicalRequest.js", subpaths: ["./canonical-request"] },
    { target: "./dist/crypto.browser.js", subpaths: ["./crypto"] },
    { target: "./dist/nexusApp.js", subpaths: ["./nexus-app"] },
    {
      target: "./dist/kotodamaCompiler/browser.js",
      subpaths: ["./kotodama-compiler"],
    },
  ]);
});

test("browser graph audit catches Node edges in an export omitted from size budgets", async () => {
  await assert.rejects(
    runBundleSizeCheck({
      loadEsbuild: async () => ({
        async build(options) {
          const entryPoint = options.entryPoints[0];
          return {
            outputFiles: [{ contents: new Uint8Array(), text: "" }],
            metafile: {
              inputs: entryPoint.endsWith("/dist/normalizers.js")
                ? { "node:fs": {} }
                : { [entryPoint]: {} },
            },
          };
        },
      }),
      log() {},
    }),
    /\.\/normalizers explicit browser export includes forbidden Node-only inputs: node:fs/u,
  );
});

test("browser runtime probe catches aliased global Buffer installation", async () => {
  await assert.rejects(
    runBundleSizeCheck({
      loadEsbuild: async () => ({
        async build(options) {
          const entryPoint = options.entryPoints[0];
          const installsBuffer = entryPoint.endsWith("/dist/browser.js");
          const text = installsBuffer
            ? "const root = globalThis; root.Buffer = class BufferShim {};"
            : "export {};";
          return {
            outputFiles: [{ contents: new TextEncoder().encode(text), text }],
            metafile: { inputs: { [entryPoint]: {} } },
          };
        },
      }),
      log() {},
    }),
    /\.\/browser explicit browser export installs a forbidden global Buffer shim at runtime/u,
  );
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
  assert.equal(Object.keys(result.metafile.inputs).length, 51);
  assert.equal(result.outputFiles[0].contents.byteLength, 304_434);
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
  assert.equal(Object.keys(result.metafile.inputs).length, 7);
  assert.equal(result.outputFiles[0].contents.byteLength, 9_644);
  assert.ok(result.outputFiles[0].contents.byteLength <= 12 * 1024);
  assert.doesNotMatch(
    result.outputFiles[0].text,
    /(?:globalThis|window|global)\.Buffer\s*=/u,
  );
});

test("remaining bundle targets retain exact pinned-esbuild baselines", async () => {
  const expected = new Map([
    ["toriiClient.js", { bytes: 851_381, modules: 57 }],
    ["transactionCodec.js (browser)", { bytes: 125_424, modules: 36 }],
    ["nexusApp.js (browser)", { bytes: 206_556, modules: 45 }],
    ["canonicalRequest.js (browser)", { bytes: 69_529, modules: 31 }],
  ]);
  const { build } = await import("esbuild");
  for (const target of BUNDLE_TARGETS.filter(({ label }) => expected.has(label))) {
    const result = await build({
      entryPoints: [target.entryPoint],
      bundle: true,
      write: false,
      platform: target.platform,
      target: target.target,
      format: "esm",
      treeShaking: true,
      sourcemap: false,
      minify: true,
      metafile: true,
    });
    assert.deepEqual(
      {
        bytes: result.outputFiles[0].contents.byteLength,
        modules: Object.keys(result.metafile.inputs).length,
      },
      expected.get(target.label),
      target.label,
    );
  }
});

test("Kotodama compiler browser export stays below 51 KiB without Node or Buffer shims", async () => {
  const target = BUNDLE_TARGETS.find(({ label }) =>
    label.includes("kotodamaCompiler/browser"),
  );
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
  assert.equal(Object.keys(result.metafile.inputs).length, 6);
  assert.equal(result.outputFiles[0].contents.byteLength, 51_362);
  assert.ok(result.outputFiles[0].contents.byteLength <= target.limitKb * 1024);
  assert.doesNotMatch(
    result.outputFiles[0].text,
    /(?:globalThis|window|global)\.Buffer\s*=/u,
  );
});
