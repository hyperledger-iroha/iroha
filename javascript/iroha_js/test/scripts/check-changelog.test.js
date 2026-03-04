import { strict as assert } from "node:assert";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";
import { spawnSync } from "node:child_process";

const scriptUrl = new URL("../../scripts/check-changelog.mjs", import.meta.url);
const scriptPath = fileURLToPath(scriptUrl);

function runCheck({ version, entries }) {
  const dir = mkdtempSync(join(tmpdir(), "iroha-js-changelog-"));
  const pkgPath = join(dir, "package.json");
  const changelogPath = join(dir, "CHANGELOG.md");
  const changelogLines = entries
    .map(
      ({ heading, body = "- placeholder" }) => `## ${heading}\n\n${body}\n`,
    )
    .join("\n");
  writeFileSync(
    pkgPath,
    JSON.stringify(
      {
        name: "@iroha/iroha-js",
        version,
      },
      null,
      2,
    ),
  );
  writeFileSync(changelogPath, `${changelogLines}\n`);
  const env = {
    ...process.env,
    CHECK_CHANGELOG_PACKAGE_JSON: pathToFileURL(pkgPath).href,
    CHECK_CHANGELOG_CHANGELOG: pathToFileURL(changelogPath).href,
  };
  const result = spawnSync(process.execPath, ["--no-warnings", scriptPath], {
    env,
    encoding: "utf8",
  });
  return result;
}

test("succeeds when changelog entry exists and version increases", () => {
  const result = runCheck({
    version: "1.2.0",
    entries: [
      { heading: "[1.2.0] - 2026-03-01" },
      { heading: "[1.1.1] - 2026-02-01" },
    ],
  });
  assert.equal(result.status, 0, `expected success, stderr: ${result.stderr}`);
  assert.match(
    result.stdout,
    /entry for 1\.2\.0 is present and newer than 1\.1\.1/i,
  );
});

test("fails when changelog entry is missing", () => {
  const result = runCheck({
    version: "2.0.0",
    entries: [{ heading: "[1.9.0] - 2026-02-01" }],
  });
  assert.notEqual(result.status, 0);
  assert.match(
    result.stderr,
    /missing an entry for version 2\.0\.0/i,
    "expected error mentioning missing changelog entry",
  );
});

test("fails when version does not increase compared to previous release", () => {
  const result = runCheck({
    version: "1.1.0",
    entries: [
      { heading: "[1.1.0] - 2026-02-01" },
      { heading: "[1.1.2] - 2026-01-15" },
    ],
  });
  assert.notEqual(result.status, 0);
  assert.match(
    result.stderr,
    /must be greater than the previous release 1\.1\.2/i,
  );
});
