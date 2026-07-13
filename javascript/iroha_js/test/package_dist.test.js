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

test("package dist quantity builders reject numbers and noncanonical strings", () => {
  assert.equal(typeof packageExports.NumericV1?.decodeQuantityJson, "function");
  assert.equal(typeof packageExports.KotodamaQuantity, "function");
  const account = packageExports.AccountAddress.fromAccount({
    publicKey: Buffer.from(
      "B935AAF1F4E44B3DB79E5E5A9BA4569E6F3E2310C219F3DDD56D3277828D5480",
      "hex",
    ),
  }).toI105();
  const assetId = `62Fk4FPcMuLvW5QjDGNF2a4jAmjM#${account}`;
  for (const quantity of [1, -1, "+1", "01", "1.0", "1.2300", " 1", "1e0"]) {
    assert.throws(
      () => packageExports.buildMintAssetInstruction({ assetId, quantity }),
      /canonical|JavaScript numbers/u,
    );
  }
  assert.equal(
    packageExports.buildMintAssetInstruction({ assetId, quantity: 1n }).Mint.Asset.object,
    "1",
  );
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
