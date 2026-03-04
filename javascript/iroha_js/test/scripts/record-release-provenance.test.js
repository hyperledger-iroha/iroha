import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";

import { recordReleaseProvenance } from "../../scripts/record-release-provenance.mjs";

const TEST_DIR = path.dirname(fileURLToPath(import.meta.url));
const JS_DIR = path.resolve(TEST_DIR, "../..");
const PACKAGE_JSON = JSON.parse(
  await fs.readFile(path.join(JS_DIR, "package.json"), "utf8"),
);

test("recordReleaseProvenance writes metadata, checksums, and npm-pack manifest", async () => {
  const tmpRoot = await fs.mkdtemp(path.join(os.tmpdir(), "iroha-js-prov-"));
  const fixtureContent = Buffer.from("iroha-js-fixture");
  const fixtureTarball = path.join(tmpRoot, "fixture.tgz");
  await fs.writeFile(fixtureTarball, fixtureContent);

  const packRunner = async ({ packDir }) => {
    const outputTarball = path.join(packDir, `iroha-iroha-js-${PACKAGE_JSON.version}.tgz`);
    await fs.mkdir(packDir, { recursive: true });
    await fs.copyFile(fixtureTarball, outputTarball);
    return {
      entries: [
        {
          filename: path.basename(outputTarball),
          integrity: "sha512-" + fixtureContent.toString("base64"),
          size: fixtureContent.length,
        },
      ],
      tarballPath: outputTarball,
    };
  };

  const now = () => new Date("2026-02-01T12:34:56.000Z");
  const npmVersionProvider = async () => "10.0.0";
  const result = await recordReleaseProvenance({
    outDir: tmpRoot,
    now,
    packRunner,
    npmVersionProvider,
    gitDescribe: "deadbeef",
    quiet: true,
  });

  const metadata = JSON.parse(await fs.readFile(result.metadataPath, "utf8"));
  assert.equal(metadata.version, PACKAGE_JSON.version);
  assert.equal(metadata.gitCommit, "deadbeef");
  assert.equal(metadata.npmVersion, "10.0.0");
  assert.equal(metadata.tarball.filename, `iroha-iroha-js-${PACKAGE_JSON.version}.tgz`);
  assert.ok(metadata.tarball.sha256);
  assert.ok(metadata.tarball.sha512);
  assert.equal(metadata.packEntries, 1);

  const packageJsonSnapshot = JSON.parse(
    await fs.readFile(result.packageJsonSnapshotPath, "utf8"),
  );
  assert.equal(packageJsonSnapshot.version, PACKAGE_JSON.version);
  assert.equal(packageJsonSnapshot.name, PACKAGE_JSON.name);

  const expectedSha256 = createHash("sha256").update(fixtureContent).digest("hex");
  assert.equal(metadata.tarball.sha256, expectedSha256);

  const packManifest = JSON.parse(await fs.readFile(result.packManifestPath, "utf8"));
  assert.equal(Array.isArray(packManifest), true);
  assert.equal(packManifest.length, 1);
  assert.equal(packManifest[0].filename, metadata.tarball.filename);

  const checksums = await fs.readFile(result.checksumPath, "utf8");
  assert.match(checksums, new RegExp(`sha256\\s+${expectedSha256}\\s+${metadata.tarball.filename}`));
  assert.match(checksums, /sha512\s+[0-9a-f]+\s+/);
});
