#!/usr/bin/env node
// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import fs from "node:fs/promises";
import { createReadStream } from "node:fs";
import path from "node:path";
import os from "node:os";
import process from "node:process";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";

const DEFAULT_REGISTRY = "https://registry.npmjs.org/";
const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const JS_DIR = path.resolve(SCRIPT_DIR, "..");
const REPO_ROOT = path.resolve(JS_DIR, "..", "..");

const collectNativeEvidence = async (repoRoot) => {
  const nativePath = path.join(repoRoot, "javascript", "iroha_js", "native", "iroha_js_host.node");
  const manifestPath = path.join(
    repoRoot,
    "javascript",
    "iroha_js",
    "native",
    "iroha_js_host.checksums.json",
  );
  if (!(await exists(nativePath))) {
    return null;
  }
  const stat = await fs.stat(nativePath);
  const sha256 = await hashFile(nativePath, "sha256");
  const sha512 = await hashFile(nativePath, "sha512");
  const platformKey = `${process.platform}-${process.arch}`;
  let expectedSha256 = null;
  if (await exists(manifestPath)) {
    try {
      const raw = JSON.parse(await fs.readFile(manifestPath, "utf8"));
      const entries = raw?.entries ?? raw;
      expectedSha256 =
        entries?.[platformKey]?.sha256 ??
        entries?.[platformKey.toLowerCase()]?.sha256 ??
        null;
    } catch {
      expectedSha256 = null;
    }
  }
  return {
    filename: path.basename(nativePath),
    size: stat.size,
    sha256,
    sha512,
    expectedSha256,
    platform: platformKey,
    manifestPresent: await exists(manifestPath),
  };
};

export async function verifyReleaseTarball(options = {}) {
  const version = options.version;
  if (!version) {
    throw new Error("Missing required version");
  }
  const registry = options.registry ?? DEFAULT_REGISTRY;
  const repoRoot = options.repoRoot ?? REPO_ROOT;
  const now = options.now?.() ?? new Date();
  const outDir = path.resolve(
    repoRoot,
    options.outDir ?? path.join("artifacts", "js", "verification"),
  );
  const stamp = formatTimestamp(now);
  const verificationDir = path.join(outDir, `v${version}_${stamp}`);
  await fs.mkdir(verificationDir, { recursive: true });

  const resolvedLocalTarball =
    options.localTarball != null
      ? await resolveLocalTarball(options.localTarball)
      : null;

  const localPackRunner = options.localPackRunner ?? defaultLocalPackRunner;
  const remotePackRunner = options.remotePackRunner ?? defaultRemotePackRunner;

  const { tarballPath: localTarballPath, cleanup: cleanupLocal } =
    resolvedLocalTarball != null
      ? { tarballPath: resolvedLocalTarball, cleanup: async () => {} }
      : await localPackRunner({ keepTemp: Boolean(options.keepTemp) });

  const { tarballPath: remoteTarballPath, cleanup: cleanupRemote } =
    await remotePackRunner({
      version,
      registry,
      keepTemp: Boolean(options.keepTemp),
    });

  const localSha256 = await hashFile(localTarballPath, "sha256");
  const remoteSha256 = await hashFile(remoteTarballPath, "sha256");
  const localSha512 = await hashFile(localTarballPath, "sha512");
  const remoteSha512 = await hashFile(remoteTarballPath, "sha512");
  const matched =
    localSha256 === remoteSha256 && localSha512 === remoteSha512;

  const native = await collectNativeEvidence(repoRoot);

  const localCopyName = `local-${path.basename(localTarballPath)}`;
  const remoteCopyName = `remote-${path.basename(remoteTarballPath)}`;
  const localCopyPath = path.join(verificationDir, localCopyName);
  const remoteCopyPath = path.join(verificationDir, remoteCopyName);

  await fs.copyFile(localTarballPath, localCopyPath);
  await fs.copyFile(remoteTarballPath, remoteCopyPath);

  if (!options.keepTemp) {
    await cleanupLocal();
    await cleanupRemote();
  }

  const summary = {
    version,
    registry,
    matched,
    timestamp: now.toISOString(),
    local: {
      filename: localCopyName,
      sha256: localSha256,
      sha512: localSha512,
      source: resolvedLocalTarball ? "provided" : "npm-pack",
    },
    remote: {
      filename: remoteCopyName,
      sha256: remoteSha256,
      sha512: remoteSha512,
      spec: `@iroha/iroha-js@${version}`,
    },
    native,
  };

  const summaryPath = path.join(verificationDir, "summary.json");
  await fs.writeFile(summaryPath, `${JSON.stringify(summary, null, 2)}\n`);

  const checksumsPath = path.join(verificationDir, "checksums.txt");
  const checksumLines = [
    `sha256  ${localSha256}  ${localCopyName}`,
    `sha256  ${remoteSha256}  ${remoteCopyName}`,
    `sha512  ${localSha512}  ${localCopyName}`,
    `sha512  ${remoteSha512}  ${remoteCopyName}`,
  ];
  if (native?.sha256) {
    checksumLines.push(`sha256  ${native.sha256}  ${native.filename}`);
  }
  if (native?.sha512) {
    checksumLines.push(`sha512  ${native.sha512}  ${native.filename}`);
  }
  await fs.writeFile(checksumsPath, `${checksumLines.join("\n")}\n`);

  if (!options.quiet) {
    const rel = path.relative(repoRoot, verificationDir);
    if (matched) {
      console.log(`[verify-release] MATCH — artefacts stored under ${rel}`);
    } else {
      console.warn(
        `[verify-release] MISMATCH — artefacts stored under ${rel}; compare checksum evidence and roll back if required`,
      );
    }
  }

  return {
    matched,
    outputDir: verificationDir,
    summaryPath,
    checksumsPath,
    localCopyPath,
    remoteCopyPath,
    localSha256,
    remoteSha256,
    localSha512,
    remoteSha512,
    native,
  };
}

async function defaultLocalPackRunner({ keepTemp }) {
  const tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), "iroha-js-local-pack-"));
  const { entries, tarballPath } = await runNpmPack({
    args: ["pack", "--json", "--pack-destination", tmpDir],
    packDir: tmpDir,
  });
  validatePackEntries(entries);
  const cleanup = async () => {
    if (!keepTemp) {
      await fs.rm(tmpDir, { recursive: true, force: true });
    }
  };
  return { tarballPath, cleanup };
}

async function defaultRemotePackRunner({ version, registry, keepTemp }) {
  if (!version) {
    throw new Error("Remote pack runner requires a version");
  }
  const tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), "iroha-js-remote-pack-"));
  const { entries, tarballPath } = await runNpmPack({
    args: [
      "pack",
      `@iroha/iroha-js@${version}`,
      "--json",
      "--pack-destination",
      tmpDir,
      "--registry",
      registry,
    ],
    packDir: tmpDir,
  });
  validatePackEntries(entries);
  const cleanup = async () => {
    if (!keepTemp) {
      await fs.rm(tmpDir, { recursive: true, force: true });
    }
  };
  return { tarballPath, cleanup };
}

async function runNpmPack({ args, packDir }) {
  if (!packDir) {
    throw new Error("runNpmPack requires packDir");
  }
  const { stdout } = await runCommand("npm", args, { cwd: JS_DIR });
  const trimmed = stdout.trim();
  const entries = trimmed ? JSON.parse(trimmed) : [];
  const filename = entries.at(-1)?.filename;
  if (!filename) {
    throw new Error("npm pack output missing filename");
  }
  return {
    entries,
    tarballPath: path.join(packDir, filename),
  };
}

function validatePackEntries(entries) {
  if (!Array.isArray(entries) || entries.length === 0) {
    throw new Error("npm pack did not emit entries");
  }
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

async function resolveLocalTarball(inputPath) {
  const abs = path.isAbsolute(inputPath)
    ? inputPath
    : path.resolve(process.cwd(), inputPath);
  const stat = await fs.stat(abs);
  if (!stat.isFile()) {
    throw new Error(`Local tarball is not a file: ${abs}`);
  }
  return abs;
}

function formatTimestamp(date) {
  const iso = date.toISOString().replace(/[-:]/g, "");
  const [stamp] = iso.split(".");
  return stamp;
}

function parseCliArgs(argv) {
  const options = {};
  for (let i = 2; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--help" || arg === "-h") {
      options.help = true;
      continue;
    }
    if (arg === "--version" || arg === "-v") {
      i += 1;
      options.version = argv[i];
      continue;
    }
    if (arg.startsWith("--version=")) {
      options.version = arg.split("=", 2)[1];
      continue;
    }
    if (arg === "--registry") {
      i += 1;
      options.registry = argv[i];
      continue;
    }
    if (arg.startsWith("--registry=")) {
      options.registry = arg.split("=", 2)[1];
      continue;
    }
    if (arg === "--local") {
      i += 1;
      options.localTarball = argv[i];
      continue;
    }
    if (arg.startsWith("--local=")) {
      options.localTarball = arg.split("=", 2)[1];
      continue;
    }
    if (arg === "--keep-temp") {
      options.keepTemp = true;
      continue;
    }
    if (arg === "--out-dir") {
      i += 1;
      options.outDir = argv[i];
      continue;
    }
    if (arg.startsWith("--out-dir=")) {
      options.outDir = arg.split("=", 2)[1];
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

function printUsage() {
  console.log(`Usage: node scripts/verify-release-tarball.mjs --version <semver> [--local <tarball>] [--registry <url>] [--out-dir <path>] [--keep-temp]\n`);
}

async function main() {
  const options = parseCliArgs(process.argv);
  if (options.help) {
    printUsage();
    return;
  }
  if (!options.version) {
    printUsage();
    throw new Error("Missing required --version argument");
  }
  const result = await verifyReleaseTarball(options);
  if (!result.matched) {
    process.exitCode = 1;
  }
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

const exists = async (candidatePath) => {
  try {
    await fs.access(candidatePath);
    return true;
  } catch {
    return false;
  }
};
