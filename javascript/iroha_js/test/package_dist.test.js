"use strict";

import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import * as packageExports from "../dist/index.js";

const packageJson = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url), "utf8"),
);
test("package dist exposes the current general-purpose SDK entrypoint", () => {
  for (const name of [
    "AccountAddress",
    "ToriiClient",
    "ToriiBrowserClient",
    "buildTransaction",
    "noritoEncodeInstruction",
    "privacyCapabilitiesV1",
  ]) {
    assert.notEqual(packageExports[name], undefined, `${name} is exported`);
  }
});

test("package publishes the exact general-purpose subpath inventory", () => {
  assert.deepEqual(Object.keys(packageJson.exports).sort(), [
    ".",
    "./address",
    "./blake2b",
    "./browser",
    "./canonical-request",
    "./connect-browser",
    "./crypto",
    "./instruction-builders",
    "./ivm-artifact",
    "./kotodama-compiler",
    "./nexus-app",
    "./norito",
    "./normalizers",
    "./sccp",
    "./sorafs",
    "./torii",
    "./torii-browser",
    "./transaction-codec",
  ]);
});
