import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  blake3Digest,
  verifyFixtureChunkDigests,
  verifyManifestFile,
  verifySignedManifestDigest,
} from "../check_sf1_vectors.mjs";

test("BLAKE3 implementation covers empty, single-chunk, and tree inputs", () => {
  const patterned = (length) => {
    const bytes = Buffer.allocUnsafe(length);
    for (let index = 0; index < length; index += 1) {
      bytes[index] = (index * 31 + 7) & 0xff;
    }
    return bytes;
  };
  const vectors = [
    [Buffer.alloc(0), "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"],
    [Buffer.from("abc"), "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"],
    [Buffer.alloc(1024), "d6fd9de5bccf223f523b316c9cd1cf9a9d87ea42473d68e011dad13f09bf8917"],
    [Buffer.alloc(1025), "d2beb49d87e59db174cb3ff1440f1899422968df670d060fd7ce759e8cc160e7"],
    [patterned(2049), "3c6cc85bfe26acef94b6bb440e8256c3fc541be3afe18bfffcd2c30c38e8a633"],
    [patterned(4097), "3000e690809e62e93e015f60ad2710797d6b3524c780b3bbb941154616c8a2e2"],
  ];
  for (const [input, expected] of vectors) {
    assert.equal(blake3Digest(input).toString("hex"), expected);
  }
});

test("fixture chunk hashes are recomputed from exact input bytes", () => {
  const input = Buffer.from("abcdef");
  const fixture = {
    inputLength: input.length,
    chunkCount: 2,
    chunkOffsets: [0, 3],
    chunkLengths: [3, 3],
    chunkDigestsBlake3: [
      blake3Digest(input.subarray(0, 3)).toString("hex"),
      blake3Digest(input.subarray(3)).toString("hex"),
    ],
  };
  assert.doesNotThrow(() => verifyFixtureChunkDigests(fixture, input));
  assert.throws(
    () =>
      verifyFixtureChunkDigests(
        {
          ...fixture,
          chunkDigestsBlake3: [...fixture.chunkDigestsBlake3.slice(0, 1), "0".repeat(64)],
        },
        input,
      ),
    /chunk 1 BLAKE3/,
  );
});

test("manifest verification rejects same-size content drift", (context) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "iroha-sf1-vector-check-"));
  context.after(() => fs.rmSync(root, { force: true, recursive: true }));
  const fixture = Buffer.from("abc");
  fs.writeFileSync(path.join(root, "fixture.bin"), fixture);

  const row = {
    file: "fixture.bin",
    size: fixture.length,
    blake3: blake3Digest(fixture).toString("hex"),
  };
  assert.doesNotThrow(() => verifyManifestFile(root, row));
  assert.throws(
    () =>
      verifyManifestFile(root, {
        ...row,
        blake3: blake3Digest(Buffer.from("abd")).toString("hex"),
      }),
    /fixture\.bin blake3/,
  );
});

test("signature envelope digest is recomputed from bounded manifest bytes", (context) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "iroha-sf1-signature-check-"));
  context.after(() => fs.rmSync(root, { force: true, recursive: true }));
  const manifest = Buffer.from('{"files":[]}\n');
  fs.writeFileSync(path.join(root, "manifest.json"), manifest);
  const envelope = {
    manifest: "manifest.json",
    manifest_blake3: blake3Digest(manifest).toString("hex"),
  };
  assert.deepEqual(verifySignedManifestDigest(root, envelope), blake3Digest(manifest));
  assert.throws(
    () =>
      verifySignedManifestDigest(root, {
        ...envelope,
        manifest_blake3: blake3Digest(Buffer.from('{"files":[1]}\n')).toString("hex"),
      }),
    /signed manifest blake3/,
  );

  const oversized = path.join(root, "oversized.bin");
  fs.writeFileSync(oversized, Buffer.alloc(0));
  fs.truncateSync(oversized, 2 * 1024 * 1024 + 1);
  assert.throws(
    () =>
      verifyManifestFile(root, {
        file: "oversized.bin",
        size: 2 * 1024 * 1024 + 1,
        blake3: "0".repeat(64),
      }),
    /exceeds the 2097152-byte bound/,
  );
});
