#!/usr/bin/env node
// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import { readFile } from "node:fs/promises";
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
    // This direct entrypoint intentionally exposes the complete Torii surface. Keep a tight
    // regression ceiling around the first actually enforced esbuild baseline (796.0 KiB).
    limitKb: 825,
  }),
  Object.freeze({
    label: "transactionCodec.js (browser)",
    entryPoint: join(ROOT, "src", "transactionCodec.js"),
    platform: "browser",
    target: "es2020",
    // Current pinned-esbuild baseline is 121.0 KiB; retain measured headroom without masking bloat.
    limitKb: 132,
  }),
  Object.freeze({
    label: "nexusApp.js (browser)",
    entryPoint: join(ROOT, "src", "nexusApp.js"),
    platform: "browser",
    target: "es2020",
    // The browser-safe Nexus facade includes Connect, strict Ed25519 verification,
    // canonical transaction finalization, and bounded Torii submission/polling.
    // Keep narrow headroom over the first enforced baseline (187.7 KiB).
    limitKb: 205,
  }),
]);

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
  });
  const output = result.outputFiles?.[0];
  if (!output) {
    throw new Error(`esbuild did not produce a bundle for ${target.label}`);
  }
  const bytes = output.contents?.byteLength ?? Buffer.byteLength(output.text ?? "", "utf8");
  const kb = (bytes / 1024).toFixed(1);
  log(`Bundled ${target.label}: ${kb} KiB (limit ${target.limitKb} KiB)`);
  if (bytes > target.limitKb * 1024) {
    throw new Error(
      `${target.label} bundle size ${kb} KiB exceeds limit ${target.limitKb} KiB`,
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
  await checkDistExport(pkg, "./torii", "import");
  await checkDistExport(pkg, "./transaction-codec", "browser");
  await checkDistExport(pkg, "./nexus-app", "browser");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  runBundleSizeCheck().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
