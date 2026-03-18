import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve as resolvePath } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import { computeAxtBindingFromNorito, poseidonHashBytes } from "../src/poseidon2.js";

const MODULE_DIR = dirname(fileURLToPath(import.meta.url));
const DESCRIPTOR_FIXTURE = JSON.parse(
  readFileSync(
    resolvePath(
      MODULE_DIR,
      "../../../crates/iroha_data_model/tests/fixtures/axt_descriptor_multi_ds.json",
    ),
    "utf8",
  ),
);

const VECTORS = [
  { input: Buffer.alloc(0), expectedHex: "0843c20816b2e5f066615395ab6086be2b9369ab86e0e1411127e97dbfe83830" },
  { input: Buffer.from("poseidon", "utf8"), expectedHex: "00558ac681a18bfa3a3d0b1476c062d5f27bd5f5f8d0305384320daba23a8a04" },
  { input: Buffer.from([0, 1, 2, 3, 4, 5, 6, 7, 8]), expectedHex: "4a5d661ffacf5310272ae6e106821ade2f21a0dc265f2e25890e98c665a82f08" },
];

test("poseidonHashBytes matches Rust vectors", () => {
  for (const { input, expectedHex } of VECTORS) {
    const digest = poseidonHashBytes(input);
    assert.equal(digest.toString("hex"), expectedHex);
  }
});

test("computeAxtBindingFromNorito matches fixture binding", () => {
  const descriptorBytes = Buffer.from(DESCRIPTOR_FIXTURE.descriptor_hex, "hex");
  const binding = computeAxtBindingFromNorito(descriptorBytes);
  assert.equal(binding.toString("hex"), DESCRIPTOR_FIXTURE.binding_hex);
});
