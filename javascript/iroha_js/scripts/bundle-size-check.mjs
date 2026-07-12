#!/usr/bin/env node
// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import { readFile } from "node:fs/promises";
import { spawn } from "node:child_process";
import process from "node:process";
import { join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = resolve(__filename, "..");
const ROOT = resolve(__dirname, "..");

export const BUNDLE_TARGETS = Object.freeze([
  Object.freeze({
    label: "toriiClient.js",
    entryPoint: join(ROOT, "src", "toriiClient.js"),
    platform: "node",
    target: "node18",
    // This direct entrypoint intentionally exposes the complete Torii surface. The audited
    // first-release baseline is 851,381 bytes with pinned esbuild; 864 KiB (884,736
    // bytes) leaves 33,355 bytes, or 3.92%, of regression headroom after removal of
    // the uncatalogued global RBC sampling/session and collector-plan surfaces.
    limitKb: 864,
  }),
  Object.freeze({
    label: "transactionCodec.js (browser)",
    entryPoint: join(ROOT, "src", "transactionCodec.js"),
    platform: "browser",
    target: "es2020",
    // Pinned-esbuild baseline is 125,424 bytes (122.5 KiB); the 132 KiB cap
    // retains 9,744 bytes (7.77%) without masking browser-codec growth.
    limitKb: 132,
    forbidNodeInputs: true,
    forbidGlobalBuffer: true,
  }),
  Object.freeze({
    label: "nexusApp.js (browser)",
    entryPoint: join(ROOT, "src", "nexusApp.js"),
    platform: "browser",
    target: "es2020",
    // The browser-safe Nexus facade includes Connect, strict Ed25519 verification,
    // canonical transaction finalization, and bounded Torii submission/polling.
    // The current 206,556-byte (201.7 KiB) baseline leaves 3,364 bytes
    // (1.63%) below the 205 KiB ceiling.
    limitKb: 205,
    forbidNodeInputs: true,
    forbidGlobalBuffer: true,
  }),
  Object.freeze({
    label: "canonicalRequest.js (browser)",
    entryPoint: join(ROOT, "dist", "canonicalRequest.js"),
    platform: "browser",
    target: "es2020",
    // First packed browser-safe baseline is 67.9 KiB with pinned esbuild.
    limitKb: 75,
    forbidNodeInputs: true,
    forbidGlobalBuffer: true,
  }),
  Object.freeze({
    label: "ivmArtifact.js (browser)",
    entryPoint: join(ROOT, "dist", "ivmArtifact.js"),
    platform: "browser",
    target: "es2020",
    // This leaf helper must remain suitable for strict-DOM browser consumers.
    limitKb: 12,
    forbidNodeInputs: true,
    forbidGlobalBuffer: true,
  }),
  Object.freeze({
    label: "kotodamaCompiler/browser.js (browser)",
    entryPoint: join(ROOT, "dist", "kotodamaCompiler", "browser.js"),
    platform: "browser",
    target: "es2020",
    // Pinned-esbuild baseline is 51,000 bytes (49.8 KiB); 51 KiB leaves 1,224
    // bytes (2.40%) while covering artifact/CNTR validation and the complete
    // remote compiler transport boundary.
    limitKb: 51,
    forbidNodeInputs: true,
    forbidGlobalBuffer: true,
  }),
  Object.freeze({
    label: "browser.js (public aggregate)",
    entryPoint: join(ROOT, "dist", "browser.js"),
    platform: "browser",
    target: "es2020",
    // The browser-clean public aggregate is 304,434 bytes (297.3 KiB) with
    // pinned esbuild; 328 KiB leaves 31,438 bytes (10.33%) for the complete
    // namespace after retired consensus diagnostics were removed.
    limitKb: 328,
    forbidNodeInputs: true,
    forbidGlobalBuffer: true,
  }),
]);

const NODE_ONLY_BROWSER_INPUT_PATTERNS = Object.freeze([
  /^node:/u,
  /(?:^|[/\\])(?:src|dist)[/\\]crypto\.js$/u,
  /(?:^|[/\\])(?:src|dist)[/\\]cryptoHash\.js$/u,
  /(?:^|[/\\])(?:src|dist)[/\\]native\.js$/u,
  /(?:^|[/\\])(?:src|dist)[/\\]toriiClient\.js$/u,
]);

const GLOBAL_BUFFER_MUTATION_PATTERNS = Object.freeze([
  /(?:globalThis|window|global|self)(?:\.Buffer|\[["']Buffer["']\])\s*(?:=|\|\|=|\?\?=|&&=|\+=|-=|\*=|\/=|%=|\*\*=|<<=|>>=|>>>=|&=|\^=|\|=|\+\+|--)/u,
  /(?:\+\+|--)(?:globalThis|window|global|self)(?:\.Buffer|\[["']Buffer["']\])/u,
  /(?:Object|Reflect)\.defineProperty\(\s*(?:globalThis|window|global|self)\s*,\s*["']Buffer["']/u,
  /Object\.defineProperties\(\s*(?:globalThis|window|global|self)\s*,\s*\{[^}]{0,512}(?:["']Buffer["']|Buffer)\s*:/u,
  /Object\.assign\(\s*(?:globalThis|window|global|self)\s*,\s*\{[^}]{0,512}(?:["']Buffer["']|Buffer)\s*:/u,
]);

export function findForbiddenBrowserInputs(inputs) {
  return inputs.filter((input) =>
    NODE_ONLY_BROWSER_INPUT_PATTERNS.some((pattern) => pattern.test(input)),
  );
}

export function hasForbiddenGlobalBufferMutation(source) {
  return GLOBAL_BUFFER_MUTATION_PATTERNS.some((pattern) => pattern.test(source));
}

const BUFFER_RUNTIME_PROBE = [
  'import { readFileSync } from "node:fs";',
  'const source = readFileSync(0, "utf8");',
  '// Initialize Node\'s lazy Fetch/Undici globals while its own Buffer is still present.',
  'void globalThis.fetch; void globalThis.Headers; void globalThis.Request; void globalThis.Response;',
  'if (!Reflect.deleteProperty(globalThis, "Buffer")) {',
  '  throw new Error("runtime probe could not remove global Buffer");',
  '}',
  'await import("data:text/javascript;charset=utf-8," + encodeURIComponent(source) + "#iroha-buffer-probe");',
  'if (Object.prototype.hasOwnProperty.call(globalThis, "Buffer")) {',
  '  throw new Error("browser bundle installed global Buffer");',
  '}',
].join("\n");

async function assertNoRuntimeGlobalBufferInstall(source, label) {
  await new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(
      process.execPath,
      ["--input-type=module", "--eval", BUFFER_RUNTIME_PROBE],
      {
        stdio: ["pipe", "ignore", "pipe"],
        env: {},
      },
    );
    let stderr = "";
    const timeout = setTimeout(() => {
      child.kill("SIGKILL");
    }, 15_000);
    child.stderr.setEncoding("utf8");
    child.stderr.on("data", (chunk) => {
      if (stderr.length < 8_192) {
        stderr += chunk.slice(0, 8_192 - stderr.length);
      }
    });
    child.once("error", (error) => {
      clearTimeout(timeout);
      rejectPromise(
        new Error(`${label} browser runtime Buffer probe failed to start`, {
          cause: error,
        }),
      );
    });
    child.once("close", (code, signal) => {
      clearTimeout(timeout);
      if (code === 0) {
        resolvePromise();
        return;
      }
      const diagnostic = stderr.replace(/\s+/gu, " ").trim().slice(0, 500);
      rejectPromise(
        new Error(
          `${label} installs a forbidden global Buffer shim at runtime` +
            `${signal ? ` (${signal})` : ""}${diagnostic ? `: ${diagnostic}` : ""}`,
        ),
      );
    });
    child.stdin.end(source, "utf8");
  });
}

async function loadRequiredEsbuild(loadEsbuild) {
  let esbuild;
  try {
    esbuild = await loadEsbuild();
  } catch (error) {
    throw new Error(
      "bundle-size-check requires the pinned esbuild devDependency; run npm install before release checks",
      { cause: error },
    );
  }
  if (typeof esbuild?.build !== "function") {
    throw new Error("bundle-size-check requires an esbuild module exposing build()");
  }
  return esbuild;
}

async function checkBundle(esbuild, target, log) {
  const result = await esbuild.build({
    entryPoints: [target.entryPoint],
    bundle: true,
    write: false,
    platform: target.platform,
    target: target.target,
    format: "esm",
    treeShaking: true,
    sourcemap: false,
    minify: true,
    metafile: target.forbidNodeInputs === true,
  });
  const output = result.outputFiles?.[0];
  if (!output) {
    throw new Error(`esbuild did not produce a bundle for ${target.label}`);
  }
  const bytes = output.contents?.byteLength ?? Buffer.byteLength(output.text ?? "", "utf8");
  const kb = (bytes / 1024).toFixed(1);
  const hasSizeLimit = Number.isFinite(target.limitKb);
  log(
    hasSizeLimit
      ? `Bundled ${target.label}: ${kb} KiB (limit ${target.limitKb} KiB)`
      : `Audited ${target.label}: ${kb} KiB browser graph`,
  );
  if (hasSizeLimit && bytes > target.limitKb * 1024) {
    throw new Error(
      `${target.label} bundle size ${kb} KiB exceeds limit ${target.limitKb} KiB`,
    );
  }
  if (target.forbidNodeInputs === true) {
    const forbidden = findForbiddenBrowserInputs(
      Object.keys(result.metafile?.inputs ?? {}),
    );
    if (forbidden.length > 0) {
      throw new Error(
        `${target.label} includes forbidden Node-only inputs: ${forbidden.join(", ")}`,
      );
    }
  }
  const outputText = output.text ?? Buffer.from(output.contents ?? []).toString("utf8");
  if (target.forbidGlobalBuffer === true && hasForbiddenGlobalBufferMutation(outputText)) {
    throw new Error(`${target.label} installs a forbidden global Buffer shim`);
  }
  if (target.runtimeNoGlobalBuffer === true) {
    await assertNoRuntimeGlobalBufferInstall(outputText, target.label);
  }
}

export function listExplicitBrowserExports(pkg) {
  const grouped = new Map();
  for (const [subpath, configured] of Object.entries(pkg?.exports ?? {})) {
    if (
      configured === null ||
      typeof configured !== "object" ||
      !Object.prototype.hasOwnProperty.call(configured, "browser")
    ) {
      continue;
    }
    const target = configured.browser;
    if (typeof target !== "string" || !target.startsWith("./dist/")) {
      throw new Error(
        `${subpath} explicit browser export should point to built dist artifacts`,
      );
    }
    const subpaths = grouped.get(target) ?? [];
    subpaths.push(subpath);
    grouped.set(target, subpaths);
  }
  return Array.from(grouped, ([target, subpaths]) =>
    Object.freeze({
      target,
      subpaths: Object.freeze(subpaths.slice()),
    }),
  );
}

async function checkExplicitBrowserExportGraphs(esbuild, pkg, log) {
  for (const { target, subpaths } of listExplicitBrowserExports(pkg)) {
    await checkBundle(
      esbuild,
      {
        label: `${subpaths.join(", ")} explicit browser export`,
        entryPoint: resolve(ROOT, target),
        platform: "browser",
        target: "es2020",
        forbidNodeInputs: true,
        forbidGlobalBuffer: true,
        runtimeNoGlobalBuffer: true,
      },
      log,
    );
  }
}

function exportTarget(pkg, subpath, condition) {
  const configured = pkg.exports?.[subpath];
  if (typeof configured === "string") return configured;
  return configured?.[condition] ?? configured?.import;
}

async function checkDistExport(pkg, subpath, condition) {
  const target = exportTarget(pkg, subpath, condition);
  if (!target?.startsWith("./dist/")) {
    throw new Error(`${subpath} ${condition} export should point to built dist artifacts`);
  }
  const distPath = resolve(ROOT, target);
  try {
    await readFile(distPath, "utf8");
  } catch (error) {
    throw new Error(
      `${subpath} export points to ${pathToFileURL(distPath)}, but the file is missing. Run npm run build:dist.`,
      { cause: error },
    );
  }
}

export async function runBundleSizeCheck({
  loadEsbuild = () => import("esbuild"),
  log = console.log,
} = {}) {
  const esbuild = await loadRequiredEsbuild(loadEsbuild);
  for (const target of BUNDLE_TARGETS) {
    await checkBundle(esbuild, target, log);
  }

  const pkg = JSON.parse(await readFile(join(ROOT, "package.json"), "utf8"));
  await checkExplicitBrowserExportGraphs(esbuild, pkg, log);
  await checkDistExport(pkg, "./torii", "import");
  await checkDistExport(pkg, "./transaction-codec", "browser");
  await checkDistExport(pkg, "./nexus-app", "browser");
  await checkDistExport(pkg, "./canonical-request", "browser");
  await checkDistExport(pkg, "./ivm-artifact", "browser");
  await checkDistExport(pkg, "./kotodama-compiler", "browser");
  await checkDistExport(pkg, "./browser", "browser");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  runBundleSizeCheck().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
