#!/usr/bin/env node
// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import { readFile } from "node:fs/promises";
import { spawn } from "node:child_process";
import process from "node:process";
import { dirname, join, relative, resolve } from "node:path";
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
    // The first-release simplification audit measured a 1,082,470-byte eager
    // closure before moving Norito-heavy validation, Kagemusha, SCCP, and route
    // governance behind the existing optional boundary. The reviewed eager path
    // is now 813,893 bytes (-24.8%); the 797 KiB ceiling leaves 2,235 bytes while
    // every deferred closure remains independently inventoried below.
    limitKb: 797,
    reviewedEagerBytes: 813_893,
    reviewedCombinedBytes: 1_212_491,
    lazyChunks: Object.freeze([
      Object.freeze({
        specifier: "./toriiOptional.js",
        entryPoint: join(ROOT, "src", "toriiOptional.js"),
        edgeCount: 1,
        reviewedBytes: 326_105,
        limitKb: 319,
      }),
      Object.freeze({
        specifier: "./sumeragiTyped.js",
        entryPoint: join(ROOT, "src", "sumeragiTyped.js"),
        edgeCount: 3,
        reviewedBytes: 72_493,
        limitKb: 72,
      }),
    ]),
  }),
  Object.freeze({
    label: "transactionCodec.js (browser)",
    entryPoint: join(ROOT, "dist", "transactionCodec.js"),
    platform: "browser",
    target: "es2020",
    // Browser package mapping is defined for checked-in dist paths, so audit the
    // shipped entrypoint rather than the Node-capable source graph. The reviewed
    // first-release codec closure is 308,737 bytes after exact ProofAttachment,
    // instruction archive, response-binding validation, and a declaration-exact
    // public facade. The 304 KiB ceiling leaves 2,559 bytes.
    limitKb: 304,
    forbidNodeInputs: true,
    forbidGlobalBuffer: true,
  }),
  Object.freeze({
    label: "nexusApp.js (browser)",
    entryPoint: join(ROOT, "dist", "nexusApp.js"),
    platform: "browser",
    target: "es2020",
    // The shipped browser-safe Nexus facade measured 371,403 bytes in the protected
    // pre-reset tree. The exact first-release public graph is now 359,417 bytes
    // (-3.23%). The 354 KiB ceiling leaves 3,079 bytes.
    limitKb: 354,
    forbidNodeInputs: true,
    forbidGlobalBuffer: true,
  }),
  Object.freeze({
    label: "canonicalRequest.js (browser)",
    entryPoint: join(ROOT, "dist", "canonicalRequest.js"),
    platform: "browser",
    target: "es2020",
    // Protected pre-reset baseline: 97,869 bytes. Current V1: 93,480 bytes
    // (-4.48%). The 94 KiB ceiling leaves 2,776 bytes.
    limitKb: 94,
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
    // Pinned-esbuild predecessor is 52,156 bytes. Exact V1 manifest state-type,
    // feature-bit, dynamic-access, and trigger-identifier validation produces
    // 52,928 bytes (+1.48%); the 53 KiB ceiling keeps this required boundary hardening
    // below the release-wide 5% regression limit.
    limitKb: 53,
    forbidNodeInputs: true,
    forbidGlobalBuffer: true,
  }),
  Object.freeze({
    label: "browser.js (public aggregate)",
    entryPoint: join(ROOT, "dist", "browser.js"),
    platform: "browser",
    target: "es2020",
    // The prior first-release aggregate measured a 517,186-byte eager split closure
    // on the pinned runner. Removing feature-specific exports from the aggregate
    // reduces the reviewed eager surface to 489,420 bytes (-5.37%); the 480 KiB
    // ceiling leaves 2,100 bytes. The typed Sumeragi parser and deployment-submit
    // continuation remain separately inventoried so startup and deferred code
    // cannot trade against one another.
    limitKb: 480,
    reviewedEagerBytes: 489_420,
    reviewedCombinedBytes: 571_416,
    lazyChunks: Object.freeze([
      Object.freeze({
        specifier: "./sumeragiTyped.js",
        entryPoint: join(ROOT, "dist", "sumeragiTyped.js"),
        edgeCount: 2,
        reviewedBytes: 72_806,
        limitKb: 72,
      }),
      Object.freeze({
        specifier: "./smartContractDeploymentSubmit.js",
        entryPoint: join(ROOT, "dist", "smartContractDeploymentSubmit.js"),
        edgeCount: 1,
        reviewedBytes: 9_190,
        limitKb: 9,
      }),
    ]),
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

function outputKeyForImport(outputs, importer, imported) {
  const candidates = [
    imported,
    relative(ROOT, resolve(ROOT, imported)),
    relative(ROOT, resolve(dirname(resolve(ROOT, importer)), imported)),
  ];
  return candidates.find((candidate) => Object.hasOwn(outputs, candidate));
}

function findEntryOutput(outputs, entryPoint) {
  return Object.entries(outputs).find(
    ([, output]) =>
      typeof output.entryPoint === "string" &&
      resolve(ROOT, output.entryPoint) === resolve(entryPoint),
  )?.[0];
}

function staticOutputClosure(outputs, rootOutput, label) {
  const closure = new Set();
  const visit = (outputName) => {
    if (closure.has(outputName)) return;
    const output = outputs[outputName];
    if (!output) {
      throw new Error(`${label} references missing split output ${outputName}`);
    }
    closure.add(outputName);
    for (const imported of output.imports ?? []) {
      if (imported.external === true) continue;
      if (imported.kind === "dynamic-import") continue;
      if (imported.kind !== "import-statement") {
        throw new Error(
          `${label} has unsupported internal split edge ${imported.kind ?? "unknown"}`,
        );
      }
      const importedOutput = outputKeyForImport(outputs, outputName, imported.path);
      if (!importedOutput) {
        throw new Error(`${label} references missing split output ${imported.path}`);
      }
      visit(importedOutput);
    }
  };
  visit(rootOutput);
  return closure;
}

function outputBytes(outputs, names) {
  return Array.from(names, (name) => outputs[name].bytes).reduce(
    (total, bytes) => total + bytes,
    0,
  );
}

function validateLazyBudget(lazy, target) {
  if (!Number.isSafeInteger(lazy.reviewedBytes) || lazy.reviewedBytes <= 0) {
    throw new Error(`${target.label} ${lazy.specifier} has no reviewed byte baseline`);
  }
  if (!Number.isSafeInteger(lazy.limitKb) || lazy.limitKb <= 0) {
    throw new Error(`${target.label} ${lazy.specifier} has no explicit lazy limit`);
  }
  const limitBytes = lazy.limitKb * 1024;
  if (lazy.reviewedBytes > limitBytes) {
    throw new Error(
      `${target.label} ${lazy.specifier} reviewed baseline exceeds its lazy limit`,
    );
  }
  if (limitBytes > Math.floor(lazy.reviewedBytes * 1.05)) {
    throw new Error(
      `${target.label} ${lazy.specifier} lazy limit exceeds the protected 5% baseline policy`,
    );
  }
}

function auditLiteralLazyEdges(inputs, target) {
  const lazyChunks = target.lazyChunks ?? [];
  const bySpecifier = new Map(lazyChunks.map((lazy) => [lazy.specifier, lazy]));
  const byEntryPoint = new Map(
    lazyChunks.map((lazy) => [resolve(lazy.entryPoint), lazy]),
  );
  const counts = new Map(lazyChunks.map((lazy) => [lazy, 0]));

  for (const [importer, input] of Object.entries(inputs)) {
    for (const imported of input.imports ?? []) {
      const byOriginal = bySpecifier.get(imported.original);
      const resolvedImport = imported.external === true
        ? undefined
        : resolve(ROOT, imported.path);
      const byResolvedPath = byEntryPoint.get(resolvedImport);
      const configured = byOriginal ?? byResolvedPath;

      if (configured && imported.external === true) {
        throw new Error(
          `${target.label} externalized ${configured.specifier}; lazy modules must be emitted split chunks`,
        );
      }
      if (configured && imported.kind !== "dynamic-import") {
        throw new Error(
          `${target.label} reclassified ${configured.specifier} as ${imported.kind ?? "unknown"}`,
        );
      }
      if (imported.external === true || imported.kind !== "dynamic-import") continue;
      if (!byOriginal || !byResolvedPath || byOriginal !== byResolvedPath) {
        throw new Error(
          `${target.label} has unapproved local dynamic import ${imported.original ?? imported.path} from ${importer}`,
        );
      }
      counts.set(configured, counts.get(configured) + 1);
    }
  }

  for (const lazy of lazyChunks) {
    if (counts.get(lazy) !== lazy.edgeCount) {
      throw new Error(
        `${target.label} requires exactly ${lazy.edgeCount} literal dynamic import edge(s) for ${lazy.specifier}; found ${counts.get(lazy)}`,
      );
    }
  }
}

export function analyzeSplitBundle(result, target) {
  const outputs = result.metafile?.outputs ?? {};
  const inputs = result.metafile?.inputs ?? {};
  const lazyChunks = target.lazyChunks ?? [];
  if (lazyChunks.length === 0) {
    throw new Error(`${target.label} has no configured lazy chunks`);
  }
  if (
    !Number.isSafeInteger(target.reviewedEagerBytes) ||
    target.reviewedEagerBytes <= 0 ||
    target.reviewedEagerBytes > target.limitKb * 1024
  ) {
    throw new Error(`${target.label} has an invalid reviewed eager baseline`);
  }
  if (target.limitKb * 1024 > Math.floor(target.reviewedEagerBytes * 1.05)) {
    throw new Error(
      `${target.label} eager limit exceeds the protected 5% baseline policy`,
    );
  }
  auditLiteralLazyEdges(inputs, target);

  const rootOutput = findEntryOutput(outputs, target.entryPoint);
  if (!rootOutput) {
    throw new Error(`${target.label} split graph is missing its eager entry output`);
  }
  const lazyOutputs = new Map();
  for (const lazy of lazyChunks) {
    validateLazyBudget(lazy, target);
    const outputName = findEntryOutput(outputs, lazy.entryPoint);
    if (!outputName) {
      throw new Error(`${target.label} did not emit lazy chunk ${lazy.specifier}`);
    }
    lazyOutputs.set(lazy, outputName);
  }

  const allowedLazyOutputs = new Set(lazyOutputs.values());
  const seenLazyOutputEdges = new Map(
    lazyChunks.map((lazy) => [lazy, 0]),
  );
  for (const [outputName, output] of Object.entries(outputs)) {
    for (const imported of output.imports ?? []) {
      if (imported.external === true) continue;
      const importedOutput = outputKeyForImport(outputs, outputName, imported.path);
      if (!importedOutput) {
        throw new Error(`${target.label} references missing split output ${imported.path}`);
      }
      const configured = Array.from(lazyOutputs).find(
        ([, lazyOutput]) => lazyOutput === importedOutput,
      )?.[0];
      if (configured && imported.kind !== "dynamic-import") {
        throw new Error(
          `${target.label} reclassified ${configured.specifier} output as ${imported.kind ?? "unknown"}`,
        );
      }
      if (imported.kind === "dynamic-import") {
        if (!allowedLazyOutputs.has(importedOutput)) {
          throw new Error(
            `${target.label} emitted unapproved lazy output ${imported.path}`,
          );
        }
        seenLazyOutputEdges.set(
          configured,
          seenLazyOutputEdges.get(configured) + 1,
        );
      }
    }
  }
  for (const lazy of lazyChunks) {
    if (seenLazyOutputEdges.get(lazy) === 0) {
      throw new Error(`${target.label} cannot reach lazy chunk ${lazy.specifier}`);
    }
  }

  const eagerOutputs = staticOutputClosure(outputs, rootOutput, target.label);
  const accountedOutputs = new Set(eagerOutputs);
  const lazyMetrics = [];
  for (const lazy of lazyChunks) {
    const closure = staticOutputClosure(
      outputs,
      lazyOutputs.get(lazy),
      `${target.label} ${lazy.specifier}`,
    );
    const incrementalOutputs = new Set(
      Array.from(closure).filter((outputName) => !eagerOutputs.has(outputName)),
    );
    const overlap = Array.from(incrementalOutputs).filter((outputName) =>
      accountedOutputs.has(outputName),
    );
    if (overlap.length > 0) {
      throw new Error(
        `${target.label} lazy closures overlap outside the eager graph: ${overlap.join(", ")}`,
      );
    }
    for (const outputName of incrementalOutputs) accountedOutputs.add(outputName);
    lazyMetrics.push(
      Object.freeze({
        specifier: lazy.specifier,
        bytes: outputBytes(outputs, incrementalOutputs),
        reviewedBytes: lazy.reviewedBytes,
        limitKb: lazy.limitKb,
        outputs: Object.freeze(Array.from(incrementalOutputs)),
      }),
    );
  }

  const unaccountedOutputs = Object.keys(outputs).filter(
    (outputName) => !accountedOutputs.has(outputName),
  );
  if (unaccountedOutputs.length > 0) {
    throw new Error(
      `${target.label} emitted unaccounted split outputs: ${unaccountedOutputs.join(", ")}`,
    );
  }
  const reviewedCombinedBytes =
    target.reviewedEagerBytes +
    lazyChunks.reduce((total, lazy) => total + lazy.reviewedBytes, 0);
  if (target.reviewedCombinedBytes !== reviewedCombinedBytes) {
    throw new Error(
      `${target.label} reviewed combined baseline must equal eager plus non-overlapping lazy baselines`,
    );
  }

  const combinedLimitKb =
    target.limitKb + lazyChunks.reduce((total, lazy) => total + lazy.limitKb, 0);
  if (reviewedCombinedBytes > combinedLimitKb * 1024) {
    throw new Error(`${target.label} reviewed combined baseline exceeds its limit`);
  }
  if (combinedLimitKb * 1024 > Math.floor(reviewedCombinedBytes * 1.05)) {
    throw new Error(
      `${target.label} combined limit exceeds the protected 5% baseline policy`,
    );
  }

  return Object.freeze({
    eagerBytes: outputBytes(outputs, eagerOutputs),
    eagerOutputs: Object.freeze(Array.from(eagerOutputs)),
    lazyChunks: Object.freeze(lazyMetrics),
    combinedBytes: outputBytes(outputs, accountedOutputs),
    combinedLimitKb,
    outputs: Object.freeze(Array.from(accountedOutputs)),
  });
}

function formatBundleSize(bytes) {
  return (bytes / 1024).toFixed(1);
}

async function checkSplitBundle(esbuild, target, log) {
  const result = await esbuild.build({
    absWorkingDir: ROOT,
    entryPoints: [target.entryPoint],
    bundle: true,
    splitting: true,
    write: false,
    outdir: join(ROOT, ".bundle-audit"),
    entryNames: "entry",
    chunkNames: "[hash]",
    platform: target.platform,
    target: target.target,
    format: "esm",
    treeShaking: true,
    sourcemap: false,
    minify: true,
    metafile: true,
    charset: "utf8",
  });
  const metrics = analyzeSplitBundle(result, target);
  if ((result.outputFiles?.length ?? 0) !== metrics.outputs.length) {
    throw new Error(`${target.label} split outputs do not match the metafile inventory`);
  }
  for (const outputName of metrics.outputs) {
    const output = result.outputFiles.find(
      (candidate) => resolve(candidate.path) === resolve(ROOT, outputName),
    );
    const expectedBytes = result.metafile.outputs[outputName].bytes;
    const actualBytes = output?.contents?.byteLength ??
      Buffer.byteLength(output?.text ?? "", "utf8");
    if (!output || actualBytes !== expectedBytes) {
      throw new Error(`${target.label} split output ${outputName} byte count is incomplete`);
    }
  }
  log(
    `Bundled ${target.label} eager: ${formatBundleSize(metrics.eagerBytes)} KiB (${metrics.eagerBytes} bytes; reviewed ${target.reviewedEagerBytes} bytes; limit ${target.limitKb} KiB)`,
  );
  if (metrics.eagerBytes > target.limitKb * 1024) {
    throw new Error(
      `${target.label} eager bundle size ${formatBundleSize(metrics.eagerBytes)} KiB exceeds limit ${target.limitKb} KiB`,
    );
  }
  for (const lazy of metrics.lazyChunks) {
    log(
      `Bundled ${target.label} lazy ${lazy.specifier}: ${formatBundleSize(lazy.bytes)} KiB (${lazy.bytes} bytes; reviewed ${lazy.reviewedBytes} bytes; limit ${lazy.limitKb} KiB)`,
    );
    if (lazy.bytes > lazy.limitKb * 1024) {
      throw new Error(
        `${target.label} lazy ${lazy.specifier} size ${formatBundleSize(lazy.bytes)} KiB exceeds limit ${lazy.limitKb} KiB`,
      );
    }
  }
  log(
    `Bundled ${target.label} combined: ${formatBundleSize(metrics.combinedBytes)} KiB (${metrics.combinedBytes} unique bytes; reviewed ${target.reviewedCombinedBytes} bytes; limit ${metrics.combinedLimitKb} KiB = eager plus lazy limits)`,
  );
  if (metrics.combinedBytes > metrics.combinedLimitKb * 1024) {
    throw new Error(
      `${target.label} combined bundle size ${formatBundleSize(metrics.combinedBytes)} KiB exceeds limit ${metrics.combinedLimitKb} KiB`,
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
  if (target.forbidGlobalBuffer === true) {
    for (const output of result.outputFiles ?? []) {
      const outputText =
        output.text ?? Buffer.from(output.contents ?? []).toString("utf8");
      if (hasForbiddenGlobalBufferMutation(outputText)) {
        throw new Error(`${target.label} installs a forbidden global Buffer shim`);
      }
    }
  }
  return metrics;
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
    if ((target.lazyChunks?.length ?? 0) > 0) {
      await checkSplitBundle(esbuild, target, log);
    } else {
      await checkBundle(esbuild, target, log);
    }
  }

  const pkg = JSON.parse(await readFile(join(ROOT, "package.json"), "utf8"));
  await checkExplicitBrowserExportGraphs(esbuild, pkg, log);
  await checkDistExport(pkg, "./torii", "import");
  await checkDistExport(pkg, "./transaction-codec", "browser");
  await checkDistExport(pkg, "./smart-contract-deployment", "browser");
  await checkDistExport(pkg, "./nexus-app", "browser");
  await checkDistExport(pkg, "./canonical-request", "browser");
  await checkDistExport(pkg, "./ivm-artifact", "browser");
  await checkDistExport(pkg, "./kotodama-compiler", "browser");
  await checkDistExport(pkg, "./sumeragi-typed", "browser");
  await checkDistExport(pkg, "./browser", "browser");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  runBundleSizeCheck().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
