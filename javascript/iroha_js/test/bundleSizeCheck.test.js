// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test, { after, before } from "node:test";
import { fileURLToPath } from "node:url";

import {
  BUNDLE_TARGETS,
  findForbiddenBrowserInputs,
  hasForbiddenGlobalBufferMutation,
  listExplicitBrowserExports,
  runBundleSizeCheck,
} from "../scripts/bundle-size-check.mjs";
import {
  acquireDistLock,
  releaseDistLock,
} from "../scripts/build-dist.mjs";

const PACKAGE_ROOT = fileURLToPath(new URL("..", import.meta.url));
let bundleDistLock;

before(async () => {
  bundleDistLock = await acquireDistLock({ root: PACKAGE_ROOT });
});

after(() => {
  if (bundleDistLock) releaseDistLock(bundleDistLock);
});

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
        limitKb: 969,
        forbidNodeInputs: false,
        forbidGlobalBuffer: false,
      },
      {
        label: "transactionCodec.js (browser)",
        limitKb: 297,
        forbidNodeInputs: true,
        forbidGlobalBuffer: true,
      },
      {
        label: "nexusApp.js (browser)",
        limitKb: 380,
        forbidNodeInputs: true,
        forbidGlobalBuffer: true,
      },
      {
        label: "canonicalRequest.js (browser)",
        limitKb: 100,
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
        limitKb: 469,
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
  assert.match(target.entryPoint, /dist[/\\]transactionCodec\.js$/u);
  assert.ok(target.limitKb > 0 && target.limitKb <= 297);
});

test("bundle-size check proves the Nexus app export has a browser-only graph", () => {
  const target = BUNDLE_TARGETS.find(({ label }) => label.includes("nexusApp"));
  assert.ok(target, "browser Nexus app bundle target is required");
  assert.equal(target.platform, "browser");
  assert.match(target.entryPoint, /dist[/\\]nexusApp\.js$/u);
  assert.ok(target.limitKb > 0 && target.limitKb <= 380);
});

test("bundle-size check gates the complete public browser aggregate", () => {
  const target = BUNDLE_TARGETS.find(({ label }) => label.includes("public aggregate"));
  assert.ok(target, "public browser aggregate bundle target is required");
  assert.equal(target.platform, "browser");
  assert.match(target.entryPoint, /dist[/\\]browser\.js$/u);
  assert.equal(target.forbidNodeInputs, true);
  assert.ok(target.limitKb > 0 && target.limitKb <= 469);
});

test("bundle-size check gates canonical requests as a browser subpath", () => {
  const target = BUNDLE_TARGETS.find(({ label }) => label.includes("canonicalRequest"));
  assert.ok(target, "browser canonical-request bundle target is required");
  assert.equal(target.platform, "browser");
  assert.match(target.entryPoint, /dist[/\\]canonicalRequest\.js$/u);
  assert.equal(target.forbidNodeInputs, true);
  assert.ok(target.limitKb > 0 && target.limitKb <= 100);
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
    {
      target: "./dist/smartContractDeployment.js",
      subpaths: ["./smart-contract-deployment"],
    },
    { target: "./dist/normalizers.js", subpaths: ["./normalizers"] },
    { target: "./dist/blake2b.js", subpaths: ["./blake2b"] },
    { target: "./dist/ivmArtifact.js", subpaths: ["./ivm-artifact"] },
    {
      target: "./dist/ivmArtifactAdmissionWasm.js",
      subpaths: ["./ivm-artifact-admission-wasm"],
    },
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
  assert.equal(Object.keys(result.metafile.inputs).length, 58);
  assert.equal(result.outputFiles[0].contents.byteLength, 469_731);
  assert.ok(
    result.outputFiles[0].contents.byteLength <= Math.floor(458_081 * 1.05),
    "public browser aggregate regressed more than 5% from the protected pre-reset tree",
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
  assert.equal(Object.keys(result.metafile.inputs).length, 7);
  assert.equal(result.outputFiles[0].contents.byteLength, 9_761);
  assert.ok(result.outputFiles[0].contents.byteLength <= 12 * 1024);
  assert.doesNotMatch(
    result.outputFiles[0].text,
    /(?:globalThis|window|global)\.Buffer\s*=/u,
  );
});

test("remaining bundle targets retain exact pinned-esbuild baselines", async () => {
  const predecessor = new Map([
    ["toriiClient.js", 945_975],
    ["transactionCodec.js (browser)", 290_498],
    ["nexusApp.js (browser)", 371_403],
    ["canonicalRequest.js (browser)", 97_869],
  ]);
  const expected = new Map([
    ["toriiClient.js", { bytes: 969_273, modules: 60 }],
    ["transactionCodec.js (browser)", { bytes: 297_440, modules: 46 }],
    ["nexusApp.js (browser)", { bytes: 380_659, modules: 55 }],
    ["canonicalRequest.js (browser)", { bytes: 98_123, modules: 34 }],
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
    const actual = {
      bytes: result.outputFiles[0].contents.byteLength,
      modules: Object.keys(result.metafile.inputs).length,
    };
    if (target.label === "toriiClient.js") {
      assert.equal(
        Object.keys(result.metafile.inputs).some(
          (input) =>
            input.includes("@noble/curves") && input.includes("bls12-381"),
        ),
        false,
        "Torii must use the local synchronous BLS validator, not bundle noble's full curve implementation",
      );
    }
    assert.deepEqual(actual, expected.get(target.label), target.label);
    assert.ok(
      actual.bytes <= Math.floor(predecessor.get(target.label) * 1.05),
      `${target.label} regressed more than 5% from the protected pre-reset tree`,
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
  assert.equal(result.outputFiles[0].contents.byteLength, 52_156);
  assert.ok(
    result.outputFiles[0].contents.byteLength <= Math.floor(51_868 * 1.05),
    "Kotodama compiler browser export regressed more than 5% from the protected pre-reset tree",
  );
  assert.ok(result.outputFiles[0].contents.byteLength <= target.limitKb * 1024);
  assert.doesNotMatch(
    result.outputFiles[0].text,
    /(?:globalThis|window|global)\.Buffer\s*=/u,
  );
});
