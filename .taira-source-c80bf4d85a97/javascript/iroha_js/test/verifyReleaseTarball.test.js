// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { verifyReleaseTarball } from "../scripts/verify-release-tarball.mjs";

async function createTempDir(prefix = "iroha-js-verify-test-") {
  return fs.mkdtemp(path.join(os.tmpdir(), prefix));
}

async function fileExists(filePath) {
  try {
    await fs.access(filePath);
    return true;
  } catch {
    return false;
  }
}

test("verifyReleaseTarball records matching tarballs", async () => {
  const repoRoot = await createTempDir();
  const outDir = path.join(repoRoot, "artifacts");
  const tarballContent = Buffer.from("hello-world");

  const localPackRunner = async () => {
    const tarballPath = path.join(repoRoot, "local.tgz");
    await fs.writeFile(tarballPath, tarballContent);
    return {
      tarballPath,
      cleanup: async () => {},
    };
  };

  const remotePackRunner = async () => {
    const tarballPath = path.join(repoRoot, "remote.tgz");
    await fs.writeFile(tarballPath, tarballContent);
    return {
      tarballPath,
      cleanup: async () => {},
    };
  };

  const result = await verifyReleaseTarball({
    version: "0.0.1",
    registry: "https://registry.npmjs.org/",
    repoRoot,
    outDir,
    localPackRunner,
    remotePackRunner,
    now: () => new Date("2026-02-19T00:00:00Z"),
    quiet: true,
  });

  assert.equal(result.matched, true);
  assert.ok(await fileExists(result.summaryPath));
  assert.ok(await fileExists(result.localCopyPath));
  assert.ok(await fileExists(result.remoteCopyPath));
  assert.ok(await fileExists(result.checksumsPath));
  const summary = JSON.parse(await fs.readFile(result.summaryPath, "utf8"));
  assert.equal(summary.version, "0.0.1");
  assert.equal(summary.matched, true);
  assert.equal(summary.registry, "https://registry.npmjs.org/");
  assert.equal(summary.local.sha512.length, 128);
  assert.equal(summary.remote.sha512, summary.local.sha512);
  assert.equal(result.localSha256, summary.local.sha256);
  assert.equal(result.localSha512, summary.local.sha512);
  const checksums = await fs.readFile(result.checksumsPath, "utf8");
  assert.match(checksums, /sha256/);
  assert.match(checksums, /sha512/);
  await fs.rm(repoRoot, { recursive: true, force: true });
});

test("verifyReleaseTarball captures checksum mismatches", async () => {
  const repoRoot = await createTempDir();
  const outDir = path.join(repoRoot, "artifacts");

  const localPackRunner = async () => {
    const tarballPath = path.join(repoRoot, "local-mismatch.tgz");
    await fs.writeFile(tarballPath, "local");
    return {
      tarballPath,
      cleanup: async () => {},
    };
  };

  const remotePackRunner = async () => {
    const tarballPath = path.join(repoRoot, "remote-mismatch.tgz");
    await fs.writeFile(tarballPath, "remote");
    return {
      tarballPath,
      cleanup: async () => {},
    };
  };

  const result = await verifyReleaseTarball({
    version: "9.9.9",
    registry: "https://registry.npmjs.org/",
    repoRoot,
    outDir,
    localPackRunner,
    remotePackRunner,
    now: () => new Date("2026-02-19T10:00:00Z"),
    quiet: true,
  });

  assert.equal(result.matched, false);
  const summary = JSON.parse(await fs.readFile(result.summaryPath, "utf8"));
  assert.equal(summary.matched, false);
  assert.notEqual(result.localSha256, result.remoteSha256);
  assert.notEqual(result.localSha512, result.remoteSha512);

  await fs.rm(repoRoot, { recursive: true, force: true });
});
