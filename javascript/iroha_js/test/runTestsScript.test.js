import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, relative } from "node:path";
import test from "node:test";

import { collectTestFiles, runTests } from "../scripts/run-tests.mjs";

test("test discovery selects only runnable JavaScript tests in deterministic order", (t) => {
  const testRoot = mkdtempSync(join(tmpdir(), "iroha-js-test-discovery-"));
  t.after(() => rmSync(testRoot, { recursive: true, force: true }));

  const nested = join(testRoot, "nested");
  mkdirSync(nested);
  for (const fixturePath of [
    join(testRoot, "zulu.test.mjs"),
    join(testRoot, "alpha.test.js"),
    join(testRoot, "helper.js"),
    join(nested, "bravo.test.js"),
    join(nested, "compiler.types.ts"),
  ]) {
    writeFileSync(fixturePath, "// fixture\n", "utf8");
  }

  assert.deepEqual(
    collectTestFiles(testRoot).map((file) => relative(testRoot, file)),
    ["alpha.test.js", join("nested", "bravo.test.js"), "zulu.test.mjs"],
  );
});

test("test runner passes the sorted explicit corpus to Node", (t) => {
  const packageRoot = mkdtempSync(join(tmpdir(), "iroha-js-test-runner-"));
  t.after(() => rmSync(packageRoot, { recursive: true, force: true }));

  const testRoot = join(packageRoot, "test");
  mkdirSync(testRoot);
  assert.throws(
    () => runTests({ packageRoot, spawn: () => assert.fail("spawned an empty corpus") }),
    /no JavaScript test files found under/,
  );

  writeFileSync(join(testRoot, "zulu.test.mjs"), "// fixture\n", "utf8");
  writeFileSync(join(testRoot, "alpha.test.js"), "// fixture\n", "utf8");
  let invocation;
  const status = runTests({
    packageRoot,
    testArgs: ["--test-name-pattern=quantity"],
    spawn(command, args, options) {
      invocation = { command, args, options };
      return { status: 0 };
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(invocation, {
    command: process.execPath,
    args: [
      "--test",
      "--test-name-pattern=quantity",
      join("test", "alpha.test.js"),
      join("test", "zulu.test.mjs"),
    ],
    options: { cwd: packageRoot, stdio: "inherit" },
  });
});

test("package test scripts use deterministic explicit discovery", () => {
  const packageJson = JSON.parse(
    readFileSync(new URL("../package.json", import.meta.url), "utf8"),
  );

  assert.equal(
    packageJson.scripts.test,
    "npm run build:native && node ./scripts/run-tests.mjs",
  );
  assert.equal(
    packageJson.scripts["test:dist"],
    "npm run build:dist && node ./scripts/run-tests.mjs",
  );
});
