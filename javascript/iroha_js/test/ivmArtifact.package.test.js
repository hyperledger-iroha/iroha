import assert from "node:assert/strict";
import test from "node:test";

import {
  computeIvmArtifactHashes as computeFromMain,
  IVM_PROGRAM_HEADER_LENGTH as mainHeaderLength,
} from "../dist/index.js";
import {
  computeIvmArtifactHashes as computeFromBrowser,
  IVM_PROGRAM_HEADER_LENGTH as browserHeaderLength,
} from "@iroha/iroha-js/browser";
import {
  computeIvmArtifactHashes as computeFromSubpath,
  IVM_PROGRAM_HEADER_LENGTH as subpathHeaderLength,
} from "@iroha/iroha-js/ivm-artifact";

const ARTIFACT = Uint8Array.from([
  0x49, 0x56, 0x4d, 0x00,
  0x01, 0x01, 0x01, 0x00,
  0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
  0x01,
]);

test("packed entrypoints expose identical browser-safe IVM artifact identities", () => {
  assert.equal(mainHeaderLength, 17);
  assert.equal(browserHeaderLength, 17);
  assert.equal(subpathHeaderLength, 17);
  const expected = {
    codeHashHex:
      "0e5751c026e543b2e8ab2eb06099daa1d1e5df47778f7787faab45cdf12fe3a9",
    artifactSha256Hex:
      "2c35100f8b2b58efb195d158d462a0a3943b1cc24d63eae188674a1d476a8fca",
  };
  assert.deepEqual(computeFromMain(ARTIFACT), expected);
  assert.deepEqual(computeFromBrowser(ARTIFACT), expected);
  assert.deepEqual(computeFromSubpath(ARTIFACT), expected);
  assert.equal(globalThis.Buffer, Buffer);
});
