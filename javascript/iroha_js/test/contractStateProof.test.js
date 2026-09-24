import assert from "node:assert/strict";
import test from "node:test";

import { computeHashLiteralCrc } from "../src/hashLiteralCrc.js";
import {
  verifyContractStateValueInclusionV1,
  verifyContractStateValueInclusionJsonV1,
} from "../src/contractStateProof.js";
import * as browser from "../src/browser.js";
import * as root from "../src/index.js";

function literal(hex) {
  const body = hex.toUpperCase();
  return `hash:${body}#${computeHashLiteralCrc("hash", body)}`;
}

// Independently derived with Python hashlib.blake2b(digest_size=32), applying
// Iroha's low-bit marker after each key, value, leaf, branch and root hash.
const ROOT = literal("194a8961806570284bf970836427142baa1ddcab853f1ee2c3ae672e1da8acb3");
const SIBLING = literal("482931df820458f6bf299fa0e88e37f2378b3e8558c2c254ad03755ed8790947");
const PATH = "sc/alpha/Balance";
const PROOF = Object.freeze({
  version: 1,
  path: PATH,
  value: [111, 110, 101],
  leaf_count: 2,
  steps: [{ bit: 0, prefix: Array(32).fill(0), sibling: SIBLING }],
});

test("contract-state membership verifier matches the independent two-leaf vector", () => {
  for (const runtime of [root, browser]) {
    assert.equal(typeof runtime.verifyContractStateValueInclusionV1, "function");
    assert.equal(runtime.verifyContractStateValueInclusionV1(PROOF, PATH, ROOT), true);
  }
});

test("contract-state membership rejects exact-key, value and root substitution", () => {
  assert.equal(verifyContractStateValueInclusionV1(PROOF, "sc/beta/Balance", ROOT), false);
  assert.equal(verifyContractStateValueInclusionV1({ ...PROOF, value: [111, 110, 102] }, PATH, ROOT), false);
  assert.equal(verifyContractStateValueInclusionV1({ ...PROOF, leaf_count: 1 }, PATH, ROOT), false);
  assert.equal(verifyContractStateValueInclusionV1({ ...PROOF, steps: [] }, PATH, ROOT), false);
  assert.equal(verifyContractStateValueInclusionV1(PROOF, PATH, SIBLING), false);
  assert.equal(verifyContractStateValueInclusionV1({ ...PROOF, version: 2 }, PATH, ROOT), false);
  assert.equal(verifyContractStateValueInclusionV1({ ...PROOF, legacy_root: ROOT }, PATH, ROOT), false);
  assert.equal(verifyContractStateValueInclusionV1({ ...PROOF, steps: [{ ...PROOF.steps[0], bit: 256 }] }, PATH, ROOT), false);
  assert.equal(verifyContractStateValueInclusionV1({ ...PROOF, value: "b25l" }, PATH, ROOT), false);
  assert.equal(verifyContractStateValueInclusionV1({ ...PROOF, path: "sc/alpha/\ud800" }, "sc/alpha/\ud800", ROOT), false);
});

test("contract-state JSON rejects duplicate keys and malformed wire values", () => {
  const source = JSON.stringify(PROOF);
  assert.equal(verifyContractStateValueInclusionJsonV1(source, PATH, ROOT), true);
  assert.equal(verifyContractStateValueInclusionJsonV1(
    source.replace('"version":1', '"version":1,"version":1'), PATH, ROOT,
  ), false);
  assert.equal(verifyContractStateValueInclusionJsonV1(
    source.replace('"version":1', '"version":1,"unknown":0'), PATH, ROOT,
  ), false);
  assert.equal(verifyContractStateValueInclusionJsonV1(
    Uint8Array.from([0xff]), PATH, ROOT,
  ), false);
});
