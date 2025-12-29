#!/usr/bin/env node
// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import { createReadStream } from "node:fs";
import fs from "node:fs/promises";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const JS_DIR = path.resolve(SCRIPT_DIR, "..");
const REPO_ROOT = path.resolve(JS_DIR, "..", "..");
const DEFAULT_CACHE_DIR = path.join(JS_DIR, ".npm-cache-bundle-size");

export function formatBytes(bytes) {
  if (!Number.isFinite(bytes) || bytes < 0) {
    return "0 B";
  }
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let index = 0;
  while (value >= 1024 && index < units.length - 1) {
    value /= 1024;
    index += 1;
  }
  const precision = value >= 10 || index === 0 ? 0 : 1;
  const rendered = value.toFixed(precision).replace(/\.0$/, "");
  return `${rendered} ${units[index]}`;
}

export function buildBundleSummary(packInfo, options = {}) {
  if (!packInfo || typeof packInfo !== "object") {
    throw new TypeError("packInfo must be an object");
  }

  const files = Array.isArray(packInfo.files) ? packInfo.files : [];
  const totalBytes =
    typeof packInfo.unpackedSize === "number"
      ? packInfo.unpackedSize
      : files.reduce((sum, file) => sum + (Number(file.size) || 0), 0);
  const topFiles = files
    .map((file) => ({
      path: file.path,
      bytes: Number(file.size) || 0,
    }))
    .sort((a, b) => b.bytes - a.bytes)
    .slice(0, Math.min(files.length, 10))
    .map((entry) => ({
      ...entry,
      percent: totalBytes > 0 ? Number(((entry.bytes / totalBytes) * 100).toFixed(2)) : 0,
    }));

  return {
    recordedAt: options.recordedAt ?? new Date().toISOString(),
    id: packInfo.id ?? null,
    name: packInfo.name ?? null,
    version: packInfo.version ?? null,
    files: {
      count:
        typeof packInfo.entryCount === "number" ? packInfo.entryCount : files.length,
      totalBytes,
      topFiles,
    },
    tarball: {
      bytes:
        typeof options.tarballBytes === "number"
          ? options.tarballBytes
          : Number(packInfo.size) || 0,
      path: options.tarballPath ?? null,
      shasum: packInfo.shasum ?? null,
      sha256: options.tarballSha256 ?? null,
      integrity: packInfo.integrity ?? null,
    },
  };
}

async function run() {
  const options = parseArgs(process.argv.slice(2));
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), "iroha-js-bundle-"));
  let shouldCleanup = !options.keepTarball;

  try {
    const packResult = await runNpmPack({
      destDir: tempDir,
      cacheDir: options.cacheDir ?? DEFAULT_CACHE_DIR,
    });

    const summary = buildBundleSummary(packResult.packInfo, {
      tarballBytes: packResult.tarballBytes,
      tarballSha256: packResult.tarballSha256,
      tarballPath: path.relative(REPO_ROOT, packResult.tarballPath),
    });

    const reportPath = await writeReport(summary, options.outPath);
    printSummary(summary, reportPath);

    if (options.keepTarball) {
      console.log(
        `[bundle-size] Tarball retained at ${path.relative(
          REPO_ROOT,
          packResult.tarballPath,
        )}`,
      );
      shouldCleanup = false;
    }
  } catch (error) {
    console.error(`[bundle-size] ${error.message}`);
    process.exitCode = 1;
  } finally {
    if (shouldCleanup) {
      await fs.rm(tempDir, { recursive: true, force: true });
    }
  }
}

function parseArgs(argv) {
  const options = {
    keepTarball: false,
    outPath: null,
    cacheDir: null,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--out") {
      options.outPath = argv[index + 1];
      index += 1;
      continue;
    }
    if (arg === "--keep-tarball") {
      options.keepTarball = true;
      continue;
    }
    if (arg === "--cache-dir") {
      options.cacheDir = argv[index + 1];
      index += 1;
      continue;
    }
    if (arg === "--help") {
      printUsage();
      process.exit(0);
    }
    throw new Error(`Unknown argument: ${arg}`);
  }

  return options;
}

function printUsage() {
  console.log(`Usage: npm run report:bundle-size [-- --out <path>] [--keep-tarball]

Options:
  --out <path>       Write the JSON report to the provided path.
  --keep-tarball     Keep the generated npm pack tarball (default: removed).
  --cache-dir <dir>  Override the npm cache directory used for the pack run.
`);
}

async function writeReport(summary, maybeOutPath) {
  const timestamp = summary.recordedAt.replace(/:/g, "-");
  const fallbackPath = path.resolve(
    REPO_ROOT,
    "artifacts",
    "js-sdk-bundle-size",
    `bundle-size-${timestamp}.json`,
  );
  const outPath = maybeOutPath
    ? path.resolve(process.cwd(), maybeOutPath)
    : fallbackPath;
  await fs.mkdir(path.dirname(outPath), { recursive: true });
  await fs.writeFile(outPath, `${JSON.stringify(summary, null, 2)}\n`, "utf8");
  return outPath;
}

async function runNpmPack({ destDir, cacheDir }) {
  await fs.mkdir(cacheDir, { recursive: true });
  const { stdout } = await execFileAsync(
    "npm",
    ["pack", "--json", "--pack-destination", destDir],
    {
      cwd: JS_DIR,
      env: { ...process.env, npm_config_cache: cacheDir },
    },
  );
  const trimmed = stdout.trim();
  if (!trimmed) {
    throw new Error("npm pack produced no JSON output");
  }
  let parsed;
  try {
    parsed = JSON.parse(trimmed);
  } catch (error) {
    throw new Error(`Failed to parse npm pack output: ${error.message}`);
  }
  const packInfo = Array.isArray(parsed) ? parsed.at(-1) : parsed;
  if (!packInfo || typeof packInfo !== "object") {
    throw new Error("npm pack did not return metadata");
  }
  const tarballPath = path.join(destDir, packInfo.filename);
  const tarballStat = await fs.stat(tarballPath);
  const tarballSha256 = await hashFile(tarballPath);

  return {
    packInfo,
    tarballPath,
    tarballBytes: tarballStat.size,
    tarballSha256,
  };
}

function printSummary(summary, reportPath) {
  console.log(
    `[bundle-size] ${summary.name ?? "package"}@${summary.version ?? "unknown"}`,
  );
  console.log(
    `  files: ${summary.files.count} (total ${formatBytes(summary.files.totalBytes)})`,
  );
  console.log(
    `  tarball: ${formatBytes(summary.tarball.bytes)} (${summary.tarball.sha256 ?? "sha256?"})`,
  );
  console.log(`  report: ${path.relative(REPO_ROOT, reportPath)}`);
  if (summary.files.topFiles.length > 0) {
    console.log("  top files:");
    summary.files.topFiles.forEach((file, index) => {
      console.log(
        `    ${String(index + 1).padStart(2, " ")}. ${file.path} — ${formatBytes(file.bytes)} (${file.percent}% of total)`,
      );
    });
  }
}

async function hashFile(filePath) {
  return new Promise((resolve, reject) => {
    const hash = createHash("sha256");
    const stream = createReadStream(filePath);
    stream.on("error", reject);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("end", () => resolve(hash.digest("hex")));
  });
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  run();
}
