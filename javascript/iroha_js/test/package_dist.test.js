"use strict";

import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import * as packageExports from "../dist/index.js";

const packageJson = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url), "utf8"),
);
const declarations = readFileSync(
  new URL("../index.d.ts", import.meta.url),
  "utf8",
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

test("JavaScript SDK publishes no offline-cash lifecycle", () => {
  const forbiddenExport =
    /(?:kagemusha|offline(?:cash|note|topup|redeem|readiness|operation|transfer)|compactpayment|recursiveaggregation)/iu;
  const unexpected = Object.keys(packageExports).filter((name) => forbiddenExport.test(name));
  assert.deepEqual(unexpected, []);

  assert.equal(packageJson.exports["./offline-cash"], undefined);
  assert.equal(packageJson.typesVersions?.["*"]?.["offline-cash"], undefined);
  assert.doesNotMatch(declarations, forbiddenExport);
});

test("built entrypoint has no retired offline modules", () => {
  const entrypoint = readFileSync(
    new URL("../dist/index.js", import.meta.url),
    "utf8",
  );
  assert.doesNotMatch(
    entrypoint,
    /offline(?:Api|CashLifecycle|QrStream)|kagemusha/iu,
  );
});
