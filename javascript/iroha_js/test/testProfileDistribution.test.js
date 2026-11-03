import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

function profileFixture(t, distributionBuilder, testSource) {
  const root = mkdtempSync(join(tmpdir(), "iroha-js-profile-distribution-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(join(root, "scripts"));
  mkdirSync(join(root, "test"));
  writeFileSync(join(root, "package.json"), '{"type":"module"}\n');
  copyFileSync(
    new URL("../scripts/run-test-profile.mjs", import.meta.url),
    join(root, "scripts", "run-test-profile.mjs"),
  );
  writeFileSync(join(root, "scripts", "build-dist.mjs"), distributionBuilder);
  writeFileSync(join(root, "test", "publication.test.js"), testSource);
  return root;
}

function runProfile(root, profile = "unit", platform) {
  const env = { ...process.env };
  delete env.NODE_TEST_CONTEXT;
  // Only this isolated fixture overrides the host name, to exercise selection
  // for every CI runner without adding a production environment override.
  const args = platform === undefined
    ? ["scripts/run-test-profile.mjs", profile]
    : ["--input-type=module", "--eval", `
Object.defineProperty(process, "platform", { value: ${JSON.stringify(platform)} });
process.argv = [process.execPath, "scripts/run-test-profile.mjs", ${JSON.stringify(profile)}];
await import("./scripts/run-test-profile.mjs");
`];
  return spawnSync(process.execPath, args, {
    cwd: root,
    env,
    encoding: "utf8",
    timeout: 15_000,
  });
}

test("unit profile publishes the distribution before starting its test readers", (t) => {
  const root = profileFixture(t,
    `import { writeFile } from "node:fs/promises";
export async function buildDistribution() {
  await writeFile(new URL("../published", import.meta.url), "current source");
}
`,
    `import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
test("reads current distribution", () => {
  assert.equal(readFileSync(new URL("../published", import.meta.url), "utf8"), "current source");
});
`,
  );
  const result = runProfile(root);
  assert.equal(result.status, 0, result.stderr + result.stdout);
  assert.match(result.stdout, /# pass 1/u);
});

test("distribution publication failure prevents the unit corpus from starting", (t) => {
  const root = profileFixture(t,
    `export async function buildDistribution() {
  throw new Error("distribution publication failed");
}
`,
    `import { writeFileSync } from "node:fs";
writeFileSync(new URL("../test-started", import.meta.url), "started");
`,
  );
  const result = runProfile(root);
  assert.equal(result.status, 1, result.stderr + result.stdout);
  assert.match(result.stderr, /distribution publication failed/u);
  assert.equal(existsSync(join(root, "test-started")), false);
});

for (const platform of ["linux", "darwin", "win32"]) {
  test(`native provenance owns the Apple build contract on ${platform}`, (t) => {
    const root = profileFixture(t, "export async function buildDistribution() {}\n", "");
    writeFileSync(join(root, "test", "nativeBuildProvenance.test.js"), `
import test from "node:test";
test("portable provenance fixture", () => {});
`);
    writeFileSync(join(root, "test", "nativeBuildProfile.test.js"), `
import test from "node:test";
test("Apple build fingerprint fixture", () => {});
`);
    const result = runProfile(root, "native-provenance", platform);
    assert.equal(result.status, 0, result.stderr + result.stdout);
    assert.match(result.stdout, /portable provenance fixture/u);
    assert.equal(result.stdout.includes("Apple build fingerprint fixture"), platform === "darwin");
    assert.match(result.stdout, platform === "darwin" ? /# pass 2/u : /# pass 1/u);
    assert.match(result.stdout, /# skipped 0/u);
  });
}

test("macOS provenance refuses a skipped Apple build contract", (t) => {
  const root = profileFixture(t, "export async function buildDistribution() {}\n", "");
  writeFileSync(join(root, "test", "nativeBuildProvenance.test.js"), `
import test from "node:test";
test("portable provenance fixture", () => {});
`);
  writeFileSync(join(root, "test", "nativeBuildProfile.test.js"), `
import test from "node:test";
test("Apple build fingerprint fixture", { skip: true }, () => {});
`);
  const result = runProfile(root, "native-provenance", "darwin");
  assert.equal(result.status, 1, result.stderr + result.stdout);
  assert.match(result.stderr, /reported skipped or todo tests/u);
});
