import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

test("Offline SDK declarations compile under strict TypeScript", () => {
  const packageRoot = fileURLToPath(new URL("..", import.meta.url));
  const tsc = fileURLToPath(new URL("../node_modules/typescript/bin/tsc", import.meta.url));
  const fixture = fileURLToPath(
    new URL("../fixtures/typescript/offlineApi.types.ts", import.meta.url),
  );
  const result = spawnSync(
    process.execPath,
    [
      tsc,
      "--noEmit",
      "--strict",
      "--exactOptionalPropertyTypes",
      "--noUncheckedIndexedAccess",
      "--skipLibCheck",
      "--target",
      "ES2022",
      "--module",
      "NodeNext",
      "--moduleResolution",
      "NodeNext",
      fixture,
    ],
    { cwd: packageRoot, encoding: "utf8" },
  );
  assert.equal(result.status, 0, `tsc failed:\n${result.stdout}\n${result.stderr}`);
});

test("Offline clients expose only the final first-release route family", () => {
  const source = readFileSync(new URL("../src/offlineApi.js", import.meta.url), "utf8");
  for (const route of [
    "/v1/offline/readiness",
    "/v1/offline/top-up",
    "/v1/offline/redeem",
    "/v1/offline/operations",
  ]) {
    assert.match(source, new RegExp(route.replaceAll("/", "\\/"), "u"));
  }
  assert.doesNotMatch(source, /\/v1\/offline\/v2|\/v1\/offline\/notes\//u);
});
