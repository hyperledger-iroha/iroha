import assert from "node:assert/strict";
import test from "node:test";

import {
  computeIvmArtifactHashes,
  IVM_PROGRAM_HEADER_LENGTH,
} from "../src/ivmArtifact.js";

const ARTIFACT = Uint8Array.from([
  0x49, 0x56, 0x4d, 0x00,
  0x01, 0x01, 0x01, 0x00,
  0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
  0x01,
]);

test("computeIvmArtifactHashes matches ledger body and full-artifact fixtures", () => {
  assert.equal(IVM_PROGRAM_HEADER_LENGTH, 17);
  assert.deepEqual(computeIvmArtifactHashes(ARTIFACT), {
    codeHashHex:
      "0e5751c026e543b2e8ab2eb06099daa1d1e5df47778f7787faab45cdf12fe3a9",
    artifactSha256Hex:
      "2c35100f8b2b58efb195d158d462a0a3943b1cc24d63eae188674a1d476a8fca",
  });
});

test("computeIvmArtifactHashes distinguishes header and body substitution", () => {
  const original = computeIvmArtifactHashes(ARTIFACT);
  const changedHeader = ARTIFACT.slice();
  changedHeader[16] ^= 0x80;
  const headerHashes = computeIvmArtifactHashes(changedHeader);
  assert.equal(headerHashes.codeHashHex, original.codeHashHex);
  assert.notEqual(headerHashes.artifactSha256Hex, original.artifactSha256Hex);

  const changedBody = Uint8Array.from([...ARTIFACT, 0x80]);
  const bodyHashes = computeIvmArtifactHashes(changedBody);
  assert.notEqual(bodyHashes.codeHashHex, original.codeHashHex);
  assert.notEqual(bodyHashes.artifactSha256Hex, original.artifactSha256Hex);
});

test("computeIvmArtifactHashes rejects ambiguous or malformed binary inputs", () => {
  assert.throws(
    () => computeIvmArtifactHashes("SVZNAA=="),
    /Uint8Array, ArrayBuffer, or ArrayBuffer view/,
  );
  assert.throws(
    () => computeIvmArtifactHashes(ARTIFACT.subarray(0, 16)),
    /at least the 17-byte program header/,
  );
  const badMagic = ARTIFACT.slice();
  badMagic[0] ^= 0xff;
  assert.throws(
    () => computeIvmArtifactHashes(badMagic.buffer),
    /invalid program header magic/,
  );
});
