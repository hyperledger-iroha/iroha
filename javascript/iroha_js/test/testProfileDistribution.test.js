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

function runUnitProfile(root) {
  const env = { ...process.env };
  delete env.NODE_TEST_CONTEXT;
  return spawnSync(process.execPath, ["scripts/run-test-profile.mjs", "unit"], {
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
  const result = runUnitProfile(root);
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
  const result = runUnitProfile(root);
  assert.equal(result.status, 1, result.stderr + result.stdout);
  assert.match(result.stderr, /distribution publication failed/u);
  assert.equal(existsSync(join(root, "test-started")), false);
});
