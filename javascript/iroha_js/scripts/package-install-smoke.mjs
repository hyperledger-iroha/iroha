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

const SCRIPT_DIR = resolve(fileURLToPath(import.meta.url), "..");
const ROOT = resolve(SCRIPT_DIR, "..");
const npmCommand = process.platform === "win32" ? "npm.cmd" : "npm";

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

function validatePackPaths(metadata) {
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
    "src/index.js",
    "dist/index.js",
    "dist/ivmArtifact.js",
    "scripts/build-dist.mjs",
  ]) {
    if (!paths.has(required)) {
      throw new Error(`package smoke missing required tar entry: ${required}`);
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
      "const checks = [",
      '  ["AccountAddress", sdk.AccountAddress],',
      '  ["ToriiClient", sdk.ToriiClient],',
      '  ["computeIvmArtifactHashes", artifact.computeIvmArtifactHashes],',
      '  ["buildCanonicalRequestHeaders", canonical.buildCanonicalRequestHeaders],',
      '  ["buildBrowserTransferPayload", codec.buildBrowserTransferPayload],',
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

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
