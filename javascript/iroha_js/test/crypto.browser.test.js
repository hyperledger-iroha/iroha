import { test } from "node:test";
import assert from "node:assert/strict";
import { buildKaigiRosterJoinProof } from "../src/crypto.browser.js";

test("browser crypto bundle exposes Kaigi roster proof helper as unsupported", () => {
  assert.throws(
    () => buildKaigiRosterJoinProof({ seed: Buffer.from("seed") }),
    /buildKaigiRosterJoinProof is unavailable in browser-only crypto builds/,
  );
});
