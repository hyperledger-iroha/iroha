import { test } from "node:test";
import assert from "node:assert/strict";
import {
  buildKaigiRosterJoinProof,
  generateKeyPair,
  normalizeCryptoAlgorithm,
  supportedCryptoAlgorithms,
} from "../src/crypto.browser.js";

test("browser crypto bundle exposes Kaigi roster proof helper as unsupported", () => {
  assert.throws(
    () => buildKaigiRosterJoinProof({ seed: Buffer.from("seed") }),
    /buildKaigiRosterJoinProof is unavailable in browser-only crypto builds/,
  );
});

test("browser crypto normalizes all algorithm labels but only signs Ed25519 locally", () => {
  assert.ok(supportedCryptoAlgorithms().includes("ml-dsa"));
  assert.equal(normalizeCryptoAlgorithm("gost3410-2012-256-paramset-a"), "gost3410-2012-256-paramset-a");
  assert.equal(generateKeyPair({ seed: Buffer.alloc(32, 7) }).algorithm, "ed25519");
  assert.throws(
    () => generateKeyPair({ algorithm: "ml-dsa", seed: Buffer.alloc(32, 7) }),
    /generateKeyPair\(ml-dsa\) is unavailable in browser-only crypto builds/,
  );
});
