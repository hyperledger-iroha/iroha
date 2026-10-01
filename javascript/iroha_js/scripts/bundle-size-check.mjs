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
    target: "node20.19",
    // Keep optional validation behind its declared module boundary. Report
    // eager and deferred bytes independently without enforcing code-size caps.
    lazyChunks: Object.freeze([
      Object.freeze({
        specifier: "./toriiOptional.js",
        entryPoint: join(ROOT, "src", "toriiOptional.js"),
        edgeCount: 1,
      }),
      Object.freeze({
        specifier: "./sumeragiTyped.js",
        entryPoint: join(ROOT, "src", "sumeragiTyped.js"),
        edgeCount: 2,
      }),
    ]),
  }),
  Object.freeze({
    label: "transactionCodec.js (browser)",
    entryPoint: join(ROOT, "dist", "transactionCodec.js"),
    platform: "browser",
    target: "es2020",
    // Audit the shipped browser entrypoint rather than the Node-capable source graph.
    forbidNodeInputs: true,
    forbidGlobalBuffer: true,
  }),
  Object.freeze({
    label: "nexusApp.js (browser)",
    entryPoint: join(ROOT, "dist", "nexusApp.js"),
    platform: "browser",
    target: "es2020",
    forbidNodeInputs: true,
    forbidGlobalBuffer: true,
  }),
  Object.freeze({
    label: "canonicalRequest.js (browser)",
    entryPoint: join(ROOT, "dist", "canonicalRequest.js"),
    platform: "browser",
    target: "es2020",
    forbidNodeInputs: true,
    forbidGlobalBuffer: true,
  }),
  Object.freeze({
    label: "ivmArtifact.js (browser)",
    entryPoint: join(ROOT, "dist", "ivmArtifact.js"),
    platform: "browser",
    target: "es2020",
    // This leaf helper must remain suitable for strict-DOM browser consumers.
    forbidNodeInputs: true,
    forbidGlobalBuffer: true,
  }),
  Object.freeze({
    label: "kotodamaCompiler/browser.js (browser)",
    entryPoint: join(ROOT, "dist", "kotodamaCompiler", "browser.js"),
    platform: "browser",
    target: "es2020",
    forbidNodeInputs: true,
    forbidGlobalBuffer: true,
  }),
  Object.freeze({
    label: "browser.js (public aggregate)",
    entryPoint: join(ROOT, "dist", "browser.js"),
    platform: "browser",
    target: "es2020",
    // Inventory eager and deferred outputs separately so module ownership and
    // browser isolation remain verifiable as the implementation grows.
    lazyChunks: Object.freeze([
      Object.freeze({
        specifier: "./sumeragiTyped.js",
        entryPoint: join(ROOT, "dist", "sumeragiTyped.js"),
        edgeCount: 2,
      }),
      Object.freeze({
        specifier: "./smartContractDeploymentSubmit.js",
        entryPoint: join(ROOT, "dist", "smartContractDeploymentSubmit.js"),
        edgeCount: 1,
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
  auditLiteralLazyEdges(inputs, target);

  const rootOutput = findEntryOutput(outputs, target.entryPoint);
  if (!rootOutput) {
    throw new Error(`${target.label} split graph is missing its eager entry output`);
  }
  const lazyOutputs = new Map();
  for (const lazy of lazyChunks) {
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
  return Object.freeze({
    eagerBytes: outputBytes(outputs, eagerOutputs),
    eagerOutputs: Object.freeze(Array.from(eagerOutputs)),
    lazyChunks: Object.freeze(lazyMetrics),
    combinedBytes: outputBytes(outputs, accountedOutputs),
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
    `Bundled ${target.label} eager: ${formatBundleSize(metrics.eagerBytes)} KiB (${metrics.eagerBytes} bytes)`,
  );
  for (const lazy of metrics.lazyChunks) {
    log(
      `Bundled ${target.label} lazy ${lazy.specifier}: ${formatBundleSize(lazy.bytes)} KiB (${lazy.bytes} bytes)`,
    );
  }
  log(
    `Bundled ${target.label} combined: ${formatBundleSize(metrics.combinedBytes)} KiB (${metrics.combinedBytes} unique bytes)`,
  );

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
  log(`Audited ${target.label}: ${kb} KiB (${bytes} bytes)`);
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
