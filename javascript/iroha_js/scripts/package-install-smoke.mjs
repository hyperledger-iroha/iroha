#!/usr/bin/env node
import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  acquireDistLock,
  buildDistribution,
  directoryDigest,
  releaseDistLock,
  validateDistOutputs,
} from "./build-dist.mjs";

const SCRIPT_PATH = fileURLToPath(import.meta.url);
const SCRIPT_DIR = resolve(SCRIPT_PATH, "..");
const ROOT = resolve(SCRIPT_DIR, "..");
const npmCommand = process.platform === "win32" ? "npm.cmd" : "npm";
const ALLOWED_RECIPE_PATHS = new Set([
  "recipes/iso_bridge_builder.mjs",
  "recipes/nexus_app_transfer.mjs",
  "recipes/README.md",
]);

function run(command, args, options = {}) {
  return new Promise((resolveRun, rejectRun) => {
    const child = spawn(command, args, {
      cwd: options.cwd,
      env: options.env ?? process.env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    const stdout = [];
    const stderr = [];
    child.stdout.on("data", (chunk) => stdout.push(chunk));
    child.stderr.on("data", (chunk) => stderr.push(chunk));
    child.once("error", rejectRun);
    child.once("close", (code, signal) => {
      const output = Buffer.concat(stdout).toString("utf8");
      const errors = Buffer.concat(stderr).toString("utf8");
      if (code !== 0 || signal !== null) {
        rejectRun(
          new Error(
            `${command} ${args.join(" ")} failed with code=${code} signal=${signal}:\n${output}\n${errors}`,
          ),
        );
        return;
      }
      resolveRun({ stdout: output, stderr: errors });
    });
  });
}

function parsePackMetadata(stdout) {
  let parsed;
  try {
    parsed = JSON.parse(stdout);
  } catch (error) {
    throw new Error(`package smoke could not parse npm pack metadata: ${error.message}`);
  }
  const metadata = Array.isArray(parsed) ? parsed.at(-1) : parsed;
  if (!metadata?.filename || !Array.isArray(metadata.files)) {
    throw new Error("package smoke received incomplete npm pack metadata");
  }
  return metadata;
}

export function validatePackPaths(metadata) {
  const paths = new Set();
  for (const entry of metadata.files) {
    const path = entry?.path;
    if (
      typeof path !== "string" ||
      path.length === 0 ||
      isAbsolute(path) ||
      path.split(/[\\/]/u).includes("..") ||
      path.includes("\0")
    ) {
      throw new Error(`package smoke rejected unsafe tar entry: ${String(path)}`);
    }
    paths.add(path.replaceAll("\\", "/"));
  }
  for (const required of [
    "package.json",
    "ivm-artifact.d.ts",
    "kotodama-compiler.d.ts",
    "src/index.js",
    "dist/index.js",
    "dist/ivmArtifact.js",
    "dist/kotodamaCompiler/browser.js",
    "dist/kotodamaCompiler/index.js",
    "dist/nexusApp.js",
    "nexus-app.d.ts",
    "recipes/iso_bridge_builder.mjs",
    "recipes/nexus_app_transfer.mjs",
    "scripts/build-dist.mjs",
  ]) {
    if (!paths.has(required)) {
      throw new Error(`package smoke missing required tar entry: ${required}`);
    }
  }
  for (const path of paths) {
    if (path.startsWith("recipes/") && !ALLOWED_RECIPE_PATHS.has(path)) {
      throw new Error(`package smoke found forbidden non-portable recipe: ${path}`);
    }
  }
}

async function main() {
  const tempRoot = mkdtempSync(join(tmpdir(), "iroha-js-package-install-"));
  try {
    await buildDistribution({ root: ROOT });

    let packMetadata;
    let tarball;
    const readerLock = await acquireDistLock({ root: ROOT });
    try {
      const src = join(ROOT, "src");
      const dist = join(ROOT, "dist");
      validateDistOutputs(dist);
      const sourceDigest = directoryDigest(src);
      const distDigest = directoryDigest(dist);
      if (sourceDigest !== distDigest) {
        throw new Error(
          `package smoke requires exact src/dist parity (${sourceDigest} != ${distDigest})`,
        );
      }

      const packed = await run(
        npmCommand,
        ["pack", "--ignore-scripts", "--json", "--pack-destination", tempRoot],
        { cwd: ROOT },
      );
      packMetadata = parsePackMetadata(packed.stdout);
      validatePackPaths(packMetadata);
      tarball = join(tempRoot, packMetadata.filename);
    } finally {
      releaseDistLock(readerLock);
    }

    const consumerRoot = join(tempRoot, "consumer");
    mkdirSync(consumerRoot, { recursive: true });
    writeFileSync(
      join(consumerRoot, "package.json"),
      `${JSON.stringify({ private: true, type: "module" })}\n`,
      "utf8",
    );
    await run(
      npmCommand,
      [
        "install",
        "--ignore-scripts",
        "--no-audit",
        "--no-fund",
        "--no-package-lock",
        "--omit=dev",
        tarball,
      ],
      {
        cwd: consumerRoot,
        env: { ...process.env, npm_config_ignore_scripts: "true" },
      },
    );

    const smokeProgram = [
      'import * as sdk from "@iroha/iroha-js";',
      'import * as canonical from "@iroha/iroha-js/canonical-request";',
      'import * as artifact from "@iroha/iroha-js/ivm-artifact";',
      'import * as codec from "@iroha/iroha-js/transaction-codec";',
      'import * as nexus from "@iroha/iroha-js/nexus-app";',
      'import * as compiler from "@iroha/iroha-js/kotodama-compiler";',
      "const checks = [",
      '  ["AccountAddress", sdk.AccountAddress],',
      '  ["ToriiClient", sdk.ToriiClient],',
      '  ["computeIvmArtifactHashes", artifact.computeIvmArtifactHashes],',
      '  ["buildCanonicalRequestHeaders", canonical.buildCanonicalRequestHeaders],',
      '  ["buildBrowserTransferPayload", codec.buildBrowserTransferPayload],',
      '  ["NexusAppClient", nexus.NexusAppClient],',
      '  ["KotodamaCompilerClient", compiler.KotodamaCompilerClient],',
      "];",
      "for (const [name, value] of checks) {",
      '  if (typeof value !== "function") throw new Error(`missing packed export: ${name}`);',
      "}",
      "if (sdk.computeIvmArtifactHashes !== artifact.computeIvmArtifactHashes) {",
      '  throw new Error("root and ivm-artifact subpath exports differ");',
      "}",
    ].join("\n");
    await run(process.execPath, ["--input-type=module", "--eval", smokeProgram], {
      cwd: consumerRoot,
    });
    await run(
      process.execPath,
      [
        "--conditions=browser",
        "--input-type=module",
        "--eval",
        [
          'import { compileKotodamaProgram, KotodamaCompilerClient } from "@iroha/iroha-js/kotodama-compiler";',
          'if (typeof compileKotodamaProgram !== "function" || typeof KotodamaCompilerClient !== "function") {',
          '  throw new Error("missing browser-conditioned Kotodama compiler exports");',
          "}",
        ].join("\n"),
      ],
      { cwd: consumerRoot },
    );

    const installedRecipe = join(
      consumerRoot,
      "node_modules",
      "@iroha",
      "iroha-js",
      "recipes",
      "nexus_app_transfer.mjs",
    );
    const recipe = await run(process.execPath, [installedRecipe], {
      cwd: consumerRoot,
    });
    for (const expected of [
      "payload hash: 2519723601cf2e75576c7f7886e32179eb83f624717552e600108db6e4127f65",
      "signed transaction hash: 6f39fd5e193f09f750939f0b089188b9a327a9dda0c8fb3de312c953bf2d93bb",
      "final status: Committed",
    ]) {
      if (!recipe.stdout.includes(expected)) {
        throw new Error(`packed Nexus recipe output is missing: ${expected}`);
      }
    }

    const installedIsoBuilderRecipe = join(
      consumerRoot,
      "node_modules",
      "@iroha",
      "iroha-js",
      "recipes",
      "iso_bridge_builder.mjs",
    );
    const isoBuilder = await run(process.execPath, [installedIsoBuilderRecipe], {
      cwd: consumerRoot,
      env: {
        ...process.env,
        ISO_MESSAGE_ID: "package-smoke-message",
        ISO_CREATION_TIME: "2026-01-01T00:00:00.000Z",
        ISO_INSTRUCTION_ID: "package-smoke-instruction",
      },
    });
    for (const expected of [
      "<MsgId>package-smoke-message</MsgId>",
      "<InstrId>package-smoke-instruction</InstrId>",
      '<IntrBkSttlmAmt Ccy="EUR">100.00</IntrBkSttlmAmt>',
    ]) {
      if (!isoBuilder.stdout.includes(expected)) {
        throw new Error(`packed ISO builder recipe output is missing: ${expected}`);
      }
    }

    const packageJson = JSON.parse(readFileSync(join(ROOT, "package.json"), "utf8"));
    if (packMetadata.name !== packageJson.name || packMetadata.version !== packageJson.version) {
      throw new Error("package smoke metadata does not match package.json identity");
    }
    console.log(
      `[package-smoke] ${packMetadata.name}@${packMetadata.version}: ${packMetadata.entryCount} files, clean install/import passed`,
    );
  } finally {
    rmSync(tempRoot, { recursive: true, force: true });
  }
}

if (process.argv[1] && resolve(process.argv[1]) === SCRIPT_PATH) {
  main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
