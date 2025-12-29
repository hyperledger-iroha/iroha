#!/usr/bin/env node
// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import fs from "node:fs/promises";
import { createReadStream } from "node:fs";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const JS_DIR = path.resolve(SCRIPT_DIR, "..");
const REPO_ROOT = path.resolve(JS_DIR, "..", "..");

export async function recordReleaseProvenance(options = {}) {
  const packageJsonPath = path.join(JS_DIR, "package.json");
  const pkg = JSON.parse(await fs.readFile(packageJsonPath, "utf8"));
  const version = pkg.version;
  if (!version) {
    throw new Error(`Missing version field in ${path.relative(REPO_ROOT, packageJsonPath)}`);
  }

  const now = options.now?.() ?? new Date();
  const stamp = formatTimestamp(now);
  const baseOutDir = path.resolve(
    REPO_ROOT,
    options.outDir ?? path.join("artifacts", "js-sdk-provenance"),
  );
  const outputDir = path.join(baseOutDir, `v${version}_${stamp}`);
  const packDir = path.join(outputDir, "npm-pack");
  await fs.mkdir(packDir, { recursive: true });

  const packRunner = options.packRunner ?? defaultPackRunner;
  const { entries, tarballPath } = await packRunner({
    packDir,
    packageDir: JS_DIR,
    packageVersion: version,
  });
  if (!Array.isArray(entries) || entries.length === 0) {
    throw new Error("npm pack did not return any entries");
  }
  if (!tarballPath) {
    throw new Error("npm pack runner did not provide a tarball path");
  }

  const tarballStat = await fs.stat(tarballPath);
  const sha256 = await hashFile(tarballPath, "sha256");
  const sha512 = await hashFile(tarballPath, "sha512");

  const npmVersionProvider =
    options.npmVersionProvider ?? (() => runCommand("npm", ["--version"], { cwd: JS_DIR }));
  const npmVersionResult = await npmVersionProvider();
  const npmVersion =
    typeof npmVersionResult === "string"
      ? npmVersionResult.trim()
      : npmVersionResult.stdout.trim();

  const gitDescribe =
    typeof options.gitDescribe === "string"
      ? options.gitDescribe
      : (await runCommand("git", ["rev-parse", "HEAD"], { cwd: REPO_ROOT })).stdout.trim();

  const metadata = {
    version,
    createdAt: now.toISOString(),
    gitCommit: gitDescribe,
    nodeVersion: process.version,
    npmVersion,
    tarball: {
      filename: path.basename(tarballPath),
      relativePath: path.relative(REPO_ROOT, tarballPath),
      size: entries.at(-1)?.size ?? tarballStat.size,
      integrity: entries.at(-1)?.integrity ?? null,
      sha256,
      sha512,
    },
    packEntries: entries.length,
  };

  const nativeInfo = await collectNativeInfo(JS_DIR);
  if (nativeInfo) {
    metadata.native = nativeInfo;
  }

  const metadataPath = path.join(outputDir, "metadata.json");
  const packManifestPath = path.join(outputDir, "npm-pack.json");
  const checksumPath = path.join(outputDir, "checksums.txt");

  await fs.writeFile(metadataPath, `${JSON.stringify(metadata, null, 2)}\n`);
  await fs.writeFile(packManifestPath, `${JSON.stringify(entries, null, 2)}\n`);
  const checksumLines = [
    `sha256  ${sha256}  ${metadata.tarball.filename}`,
    `sha512  ${sha512}  ${metadata.tarball.filename}`,
  ];
  if (nativeInfo?.sha256) {
    checksumLines.push(`sha256  ${nativeInfo.sha256}  ${nativeInfo.filename}`);
  }
  if (nativeInfo?.sha512) {
    checksumLines.push(`sha512  ${nativeInfo.sha512}  ${nativeInfo.filename}`);
  }
  await fs.writeFile(checksumPath, `${checksumLines.join("\n")}\n`);

  const packageJsonSnapshotPath = path.join(outputDir, "package.json");
  await fs.copyFile(packageJsonPath, packageJsonSnapshotPath);

  if (!options.quiet) {
    const rel = path.relative(REPO_ROOT, outputDir);
    console.log(`Recorded JS SDK provenance under ${rel}`);
  }

  return {
    outputDir,
    tarballPath,
    metadataPath,
    packManifestPath,
    checksumPath,
    packageJsonSnapshotPath,
  };
}

async function defaultPackRunner({ packDir }) {
  const { stdout } = await runCommand(
    "npm",
    ["pack", "--json", "--pack-destination", packDir],
    { cwd: JS_DIR },
  );
  const trimmed = stdout.trim();
  const entries = trimmed ? JSON.parse(trimmed) : [];
  if (!Array.isArray(entries) || entries.length === 0) {
    throw new Error("npm pack did not emit JSON metadata");
  }
  const filename = entries.at(-1)?.filename;
  if (!filename) {
    throw new Error("npm pack metadata missing filename");
  }
  const tarballPath = path.join(packDir, filename);
  return { entries, tarballPath };
}

async function runCommand(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      ...options,
      stdio: ["ignore", "pipe", "pipe"],
    });
    const stdoutChunks = [];
    const stderrChunks = [];
    child.stdout.on("data", (chunk) => stdoutChunks.push(chunk));
    child.stderr.on("data", (chunk) => stderrChunks.push(chunk));
    child.once("error", reject);
    child.once("close", (code) => {
      if (code !== 0) {
        reject(
          new Error(
            `${command} ${args.join(" ")} exited with code ${code}: ${Buffer.concat(stderrChunks).toString("utf8")}`,
          ),
        );
        return;
      }
      resolve({
        stdout: Buffer.concat(stdoutChunks).toString("utf8"),
        stderr: Buffer.concat(stderrChunks).toString("utf8"),
      });
    });
  });
}

async function hashFile(filePath, algorithm) {
  return new Promise((resolve, reject) => {
    const hash = createHash(algorithm);
    const stream = createReadStream(filePath);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.once("error", reject);
    stream.once("end", () => resolve(hash.digest("hex")));
  });
}

function formatTimestamp(date) {
  const iso = date.toISOString().replace(/[-:]/g, "");
  const [stamp] = iso.split(".");
  return stamp;
}

function parseCliArgs(argv) {
  const options = {};
  for (let index = 2; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--out-dir") {
      index += 1;
      options.outDir = argv[index];
      continue;
    }
    if (arg === "--quiet") {
      options.quiet = true;
      continue;
    }
    throw new Error(`Unknown argument: ${arg}`);
  }
  return options;
}

async function main() {
  const options = parseCliArgs(process.argv);
  await recordReleaseProvenance(options);
}

if (isMainModule()) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  });
}

function isMainModule() {
  if (process.argv.length < 2) {
    return false;
  }
  const invoked = path.resolve(process.argv[1]);
  const self = fileURLToPath(import.meta.url);
  return invoked === self;
}

async function collectNativeInfo(jsDir) {
  const nativePath = path.join(jsDir, "native", "iroha_js_host.node");
  const manifestPath = path.join(jsDir, "native", "iroha_js_host.checksums.json");
  if (!(await exists(nativePath))) {
    return null;
  }
  const stat = await fs.stat(nativePath);
  const sha256 = await hashFile(nativePath, "sha256");
  const sha512 = await hashFile(nativePath, "sha512");
  const platformKey = `${process.platform}-${process.arch}`;
  let expectedSha256 = null;
  let manifestEntries = null;
  if (await exists(manifestPath)) {
    try {
      const raw = JSON.parse(await fs.readFile(manifestPath, "utf8"));
      manifestEntries = raw?.entries ?? raw;
      expectedSha256 =
        manifestEntries?.[platformKey]?.sha256 ??
        manifestEntries?.[platformKey.toLowerCase()]?.sha256 ??
        null;
    } catch {
      expectedSha256 = null;
    }
  }
  return {
    filename: path.basename(nativePath),
    relativePath: path.relative(REPO_ROOT, nativePath),
    size: stat.size,
    sha256,
    sha512,
    expectedSha256,
    platform: platformKey,
    manifestFilename: (await exists(manifestPath)) ? path.basename(manifestPath) : null,
  };
}

async function exists(candidatePath) {
  try {
    await fs.access(candidatePath);
    return true;
  } catch {
    return false;
  }
}
