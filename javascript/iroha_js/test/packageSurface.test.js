import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const PACKAGE_ROOT = fileURLToPath(new URL("..", import.meta.url));
const REPOSITORY_ROOT = path.resolve(PACKAGE_ROOT, "../..");
const CANONICAL_README = "README.md";
const BACKUP_ARTIFACT_SUFFIX = /(?:\.bak|\.backup|\.old|\.orig|\.rej|\.save|\.tmp|~)$/iu;

function readPackManifest() {
  const packed = spawnSync(
    "npm",
    ["pack", "--dry-run", "--ignore-scripts", "--json"],
    { cwd: PACKAGE_ROOT, encoding: "utf8" },
  );
  assert.equal(
    packed.status,
    0,
    `npm pack --dry-run failed:\n${packed.stdout}\n${packed.stderr}`,
  );

  let result;
  assert.doesNotThrow(() => {
    result = JSON.parse(packed.stdout);
  }, `npm pack --dry-run emitted invalid JSON:\n${packed.stdout}`);
  assert.equal(result.length, 1, "npm pack must describe exactly one package");
  assert.ok(Array.isArray(result[0].files), "npm pack manifest is missing files");
  return result[0];
}

test("npm package surface includes exact license and no backup documentation", () => {
  const packageJson = JSON.parse(
    fs.readFileSync(path.join(PACKAGE_ROOT, "package.json"), "utf8"),
  );
  assert.equal(packageJson.files.includes("README.md"), true);
  assert.equal(packageJson.files.includes("LICENSE"), true);

  const packageLicense = fs.readFileSync(path.join(PACKAGE_ROOT, "LICENSE"));
  const repositoryLicense = fs.readFileSync(
    path.join(REPOSITORY_ROOT, "LICENSE"),
  );
  assert.deepEqual(
    packageLicense,
    repositoryLicense,
    "package LICENSE must exactly match the repository license",
  );

  const manifest = readPackManifest();
  const packagedPaths = manifest.files.map((entry) => entry.path);
  assert.equal(
    packagedPaths.filter((entry) => entry === "LICENSE").length,
    1,
    "package must contain exactly one root LICENSE",
  );
  assert.equal(
    manifest.files.find((entry) => entry.path === "LICENSE")?.size,
    packageLicense.byteLength,
    "packed LICENSE size must match the canonical package license",
  );
  assert.equal(
    packagedPaths.filter((entry) => entry === CANONICAL_README).length,
    1,
    "package must contain exactly one canonical root README",
  );

  const forbiddenArtifacts = packagedPaths.filter((entry) => {
    const basename = path.posix.basename(entry);
    const nonCanonicalReadme =
      /^readme/iu.test(basename) && basename !== CANONICAL_README;
    return nonCanonicalReadme || BACKUP_ARTIFACT_SUFFIX.test(basename);
  });
  assert.deepEqual(
    forbiddenArtifacts,
    [],
    `package contains backup or non-canonical README artifacts: ${forbiddenArtifacts.join(", ")}`,
  );
});

test("script-disabled source archives contain every declared runtime entrypoint", () => {
  const packageJson = JSON.parse(
    fs.readFileSync(path.join(PACKAGE_ROOT, "package.json"), "utf8"),
  );
  const manifestPaths = new Set(
    readPackManifest().files.map((entry) => entry.path.replaceAll("\\", "/")),
  );
  const tracked = spawnSync(
    "git",
    ["ls-files", "-z", "--", "javascript/iroha_js/package.json", "javascript/iroha_js/src"],
    { cwd: REPOSITORY_ROOT, encoding: "buffer" },
  );
  assert.equal(
    tracked.status,
    0,
    `git ls-files failed:\n${tracked.stderr.toString("utf8")}`,
  );
  const trackedPaths = new Set(
    tracked.stdout
      .toString("utf8")
      .split("\0")
      .filter(Boolean),
  );
  assert.equal(
    trackedPaths.has("javascript/iroha_js/package.json"),
    true,
    "source-archive package manifest must be tracked",
  );
  const runtimeTargets = new Set([packageJson.main]);

  for (const descriptor of Object.values(packageJson.exports)) {
    runtimeTargets.add(descriptor.import);
    if (descriptor.browser !== undefined) runtimeTargets.add(descriptor.browser);
  }
  for (const [source, replacement] of Object.entries(packageJson.browser)) {
    runtimeTargets.add(source);
    runtimeTargets.add(replacement);
  }

  for (const target of runtimeTargets) {
    assert.match(
      target,
      /^\.\/src\//u,
      `${target} must resolve tracked source without a package lifecycle build`,
    );
    const packagePath = target.slice(2);
    assert.equal(
      manifestPaths.has(packagePath),
      true,
      `${target} is absent from an npm pack --ignore-scripts archive`,
    );
    assert.equal(
      fs.existsSync(path.join(PACKAGE_ROOT, packagePath)),
      true,
      `${target} is absent from the source checkout`,
    );
    assert.equal(
      trackedPaths.has(`javascript/iroha_js/${packagePath}`),
      true,
      `${target} is absent from the Git source archive`,
    );
  }
});
