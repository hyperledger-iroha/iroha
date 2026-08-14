#!/usr/bin/env node

import assert from "node:assert/strict";
import { createHash, createPublicKey, verify } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..");
const fixturesDir = path.join(repoRoot, "fixtures", "sorafs_chunker");
const canonicalProfile = "sorafs.sf1@1.0.0";
const ed25519SpkiPrefix = Buffer.from("302a300506032b6570032100", "hex");
const maxCheckedFileBytes = 2 * 1024 * 1024;
const blake3Iv = Uint32Array.from([
  0x6a09e667,
  0xbb67ae85,
  0x3c6ef372,
  0xa54ff53a,
  0x510e527f,
  0x9b05688c,
  0x1f83d9ab,
  0x5be0cd19,
]);
const blake3MessagePermutation = Uint8Array.from([
  2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8,
]);
const blake3ChunkBytes = 1024;
const blake3BlockBytes = 64;
const blake3ChunkStart = 1;
const blake3ChunkEnd = 2;
const blake3Parent = 4;
const blake3Root = 8;

function rotateRight32(value, bits) {
  return ((value >>> bits) | (value << (32 - bits))) >>> 0;
}

function blake3Mix(state, a, b, c, d, x, y) {
  state[a] = (state[a] + state[b] + x) >>> 0;
  state[d] = rotateRight32(state[d] ^ state[a], 16);
  state[c] = (state[c] + state[d]) >>> 0;
  state[b] = rotateRight32(state[b] ^ state[c], 12);
  state[a] = (state[a] + state[b] + y) >>> 0;
  state[d] = rotateRight32(state[d] ^ state[a], 8);
  state[c] = (state[c] + state[d]) >>> 0;
  state[b] = rotateRight32(state[b] ^ state[c], 7);
}

function blake3Round(state, message) {
  blake3Mix(state, 0, 4, 8, 12, message[0], message[1]);
  blake3Mix(state, 1, 5, 9, 13, message[2], message[3]);
  blake3Mix(state, 2, 6, 10, 14, message[4], message[5]);
  blake3Mix(state, 3, 7, 11, 15, message[6], message[7]);
  blake3Mix(state, 0, 5, 10, 15, message[8], message[9]);
  blake3Mix(state, 1, 6, 11, 12, message[10], message[11]);
  blake3Mix(state, 2, 7, 8, 13, message[12], message[13]);
  blake3Mix(state, 3, 4, 9, 14, message[14], message[15]);
}

function blake3Permute(message) {
  return Uint32Array.from(blake3MessagePermutation, (index) => message[index]);
}

function blake3BlockWords(block) {
  const padded = Buffer.alloc(blake3BlockBytes);
  block.copy(padded);
  const words = new Uint32Array(16);
  for (let index = 0; index < words.length; index += 1) {
    words[index] = padded.readUInt32LE(index * 4);
  }
  return words;
}

function blake3Compress(inputCv, blockWords, counter, blockLength, flags) {
  const state = new Uint32Array(16);
  state.set(inputCv, 0);
  state.set(blake3Iv.subarray(0, 4), 8);
  const counterBigInt = BigInt(counter);
  state[12] = Number(counterBigInt & 0xffff_ffffn);
  state[13] = Number((counterBigInt >> 32n) & 0xffff_ffffn);
  state[14] = blockLength;
  state[15] = flags;

  let message = Uint32Array.from(blockWords);
  for (let round = 0; round < 7; round += 1) {
    blake3Round(state, message);
    if (round !== 6) {
      message = blake3Permute(message);
    }
  }

  const output = new Uint32Array(16);
  for (let index = 0; index < 8; index += 1) {
    output[index] = (state[index] ^ state[index + 8]) >>> 0;
    output[index + 8] = (state[index + 8] ^ inputCv[index]) >>> 0;
  }
  return output;
}

function blake3Output(inputCv, blockWords, counter, blockLength, flags) {
  return { inputCv, blockWords, counter, blockLength, flags };
}

function blake3ChainingValue(output) {
  return blake3Compress(
    output.inputCv,
    output.blockWords,
    output.counter,
    output.blockLength,
    output.flags,
  ).subarray(0, 8);
}

function blake3ChunkOutput(chunk, chunkIndex) {
  const blockCount = Math.max(1, Math.ceil(chunk.length / blake3BlockBytes));
  let chainingValue = Uint32Array.from(blake3Iv);
  for (let blockIndex = 0; blockIndex < blockCount - 1; blockIndex += 1) {
    const offset = blockIndex * blake3BlockBytes;
    const flags = blockIndex === 0 ? blake3ChunkStart : 0;
    chainingValue = blake3Compress(
      chainingValue,
      blake3BlockWords(chunk.subarray(offset, offset + blake3BlockBytes)),
      chunkIndex,
      blake3BlockBytes,
      flags,
    ).subarray(0, 8);
  }

  const finalOffset = (blockCount - 1) * blake3BlockBytes;
  const finalBlock = chunk.subarray(finalOffset);
  const flags = blake3ChunkEnd | (blockCount === 1 ? blake3ChunkStart : 0);
  return blake3Output(
    chainingValue,
    blake3BlockWords(finalBlock),
    chunkIndex,
    finalBlock.length,
    flags,
  );
}

function blake3ParentOutput(left, right) {
  const words = new Uint32Array(16);
  words.set(left, 0);
  words.set(right, 8);
  return blake3Output(blake3Iv, words, 0, blake3BlockBytes, blake3Parent);
}

function blake3SubtreeOutput(chunks, start, length) {
  if (length === 1) {
    return blake3ChunkOutput(chunks[start], start);
  }
  let leftLength = 1;
  while (leftLength * 2 < length) {
    leftLength *= 2;
  }
  const left = blake3SubtreeOutput(chunks, start, leftLength);
  const right = blake3SubtreeOutput(chunks, start + leftLength, length - leftLength);
  return blake3ParentOutput(blake3ChainingValue(left), blake3ChainingValue(right));
}

/** Return the canonical 32-byte unkeyed BLAKE3 digest for `input`. */
export function blake3Digest(input) {
  const bytes = Buffer.isBuffer(input) ? input : Buffer.from(input);
  const chunks = [];
  for (let offset = 0; offset < bytes.length; offset += blake3ChunkBytes) {
    chunks.push(bytes.subarray(offset, offset + blake3ChunkBytes));
  }
  if (chunks.length === 0) {
    chunks.push(Buffer.alloc(0));
  }
  const root = blake3SubtreeOutput(chunks, 0, chunks.length);
  const words = blake3Compress(
    root.inputCv,
    root.blockWords,
    0,
    root.blockLength,
    root.flags | blake3Root,
  );
  const digest = Buffer.alloc(32);
  for (let index = 0; index < 8; index += 1) {
    digest.writeUInt32LE(words[index], index * 4);
  }
  return digest;
}

function readBoundedRegularFile(filePath, label) {
  const noFollow = fs.constants.O_NOFOLLOW ?? 0;
  const descriptor = fs.openSync(filePath, fs.constants.O_RDONLY | noFollow);
  try {
    const before = fs.fstatSync(descriptor, { bigint: true });
    assert.ok(before.isFile(), `${label} must be a regular file`);
    assert.equal(before.nlink, 1n, `${label} must have exactly one hard link`);
    assert.ok(
      before.size <= BigInt(maxCheckedFileBytes),
      `${label} exceeds the ${maxCheckedFileBytes}-byte bound`,
    );
    const bytes = Buffer.alloc(Number(before.size) + 1);
    let offset = 0;
    while (offset < bytes.length) {
      const count = fs.readSync(descriptor, bytes, offset, bytes.length - offset, null);
      if (count === 0) {
        break;
      }
      offset += count;
    }
    assert.equal(offset, Number(before.size), `${label} changed size while read`);

    const openedAfter = fs.fstatSync(descriptor, { bigint: true });
    const pathAfter = fs.lstatSync(filePath, { bigint: true });
    assert.ok(pathAfter.isFile() && !pathAfter.isSymbolicLink(), `${label} path changed type`);
    for (const field of ["dev", "ino", "size", "mtimeNs", "ctimeNs", "nlink"]) {
      assert.equal(openedAfter[field], before[field], `${label} open file changed at ${field}`);
      assert.equal(pathAfter[field], before[field], `${label} path changed at ${field}`);
    }
    return bytes.subarray(0, offset);
  } finally {
    fs.closeSync(descriptor);
  }
}

/** Validate one manifest file row against its exact bytes. */
export function verifyManifestFile(fixturesRoot, file) {
  assert.equal(typeof file?.file, "string", "manifest file name");
  assert.match(file.blake3, /^[0-9a-f]{64}$/, `${file.file} blake3 shape`);
  const filePath = path.join(fixturesRoot, file.file);
  const bytes = readBoundedRegularFile(filePath, file.file);
  assert.equal(bytes.length, file.size, `${file.file} size`);
  const actualDigest = blake3Digest(bytes).toString("hex");
  assert.equal(actualDigest, file.blake3, `${file.file} blake3`);
}

/** Recompute and authenticate the digest named by a signature envelope. */
export function verifySignedManifestDigest(fixturesRoot, signatures) {
  assert.equal(typeof signatures?.manifest, "string", "signature manifest name");
  assert.match(signatures.manifest_blake3, /^[0-9a-f]{64}$/, "manifest digest shape");
  const manifestBytes = readBoundedRegularFile(
    path.join(fixturesRoot, signatures.manifest),
    signatures.manifest,
  );
  const manifestDigest = blake3Digest(manifestBytes);
  assert.equal(
    manifestDigest.toString("hex"),
    signatures.manifest_blake3,
    "signed manifest blake3",
  );
  return manifestDigest;
}

function readText(relativePath) {
  return readBoundedRegularFile(path.join(repoRoot, relativePath), relativePath).toString("utf8");
}

function readJson(relativePath) {
  return JSON.parse(readText(relativePath));
}

function generateFixtureInput(fixture) {
  assert.match(fixture.inputSeed, /^0x[0-9a-fA-F]+$/, "fixture input seed");
  assert.ok(
    Number.isSafeInteger(fixture.inputLength) &&
      fixture.inputLength >= 0 &&
      fixture.inputLength <= maxCheckedFileBytes,
    "fixture input length is bounded",
  );
  const mask64 = (1n << 64n) - 1n;
  const multiplier = BigInt(fixture.prngMultiplier);
  const increment = BigInt(fixture.prngIncrement);
  let state = BigInt(fixture.inputSeed);
  const input = Buffer.allocUnsafe(fixture.inputLength);
  for (let index = 0; index < input.length; index += 1) {
    state = (state * multiplier + increment) & mask64;
    input[index] = Number((state >> 32n) & 0xffn);
  }
  return input;
}

/** Recompute every per-chunk BLAKE3 stored by an SF1 fixture. */
export function verifyFixtureChunkDigests(fixture, input) {
  assert.equal(input.length, fixture.inputLength, "fixture input byte length");
  for (let index = 0; index < fixture.chunkCount; index += 1) {
    const offset = fixture.chunkOffsets[index];
    const length = fixture.chunkLengths[index];
    assert.ok(
      Number.isSafeInteger(offset) && Number.isSafeInteger(length) && offset >= 0 && length >= 0,
      `chunk ${index} has bounded integer coordinates`,
    );
    const end = offset + length;
    assert.ok(end <= input.length, `chunk ${index} is inside the fixture input`);
    const actualDigest = blake3Digest(input.subarray(offset, end)).toString("hex");
    assert.equal(actualDigest, fixture.chunkDigestsBlake3[index], `chunk ${index} BLAKE3`);
  }
}

function fail(message) {
  throw new Error(`[sf1-vectors] ${message}`);
}

function extractDelimited(content, needle, open, close) {
  const needleIndex = content.indexOf(needle);
  if (needleIndex < 0) {
    fail(`missing ${needle}`);
  }
  const openIndex = needle.endsWith(open)
    ? needleIndex + needle.length - 1
    : content.indexOf(open, needleIndex + needle.length);
  if (openIndex < 0) {
    fail(`missing ${open} after ${needle}`);
  }

  let depth = 0;
  for (let index = openIndex; index < content.length; index += 1) {
    const char = content[index];
    if (char === open) {
      depth += 1;
    } else if (char === close) {
      depth -= 1;
      if (depth === 0) {
        return content.slice(openIndex + 1, index);
      }
    }
  }
  fail(`missing ${close} after ${needle}`);
}

function extractString(content, pattern, label) {
  const match = content.match(pattern);
  if (!match) {
    fail(`missing ${label}`);
  }
  return match[1];
}

function extractNumber(content, pattern, label) {
  return Number.parseInt(extractString(content, pattern, label), 10);
}

function extractJsonIntegerText(content, property) {
  return extractString(content, new RegExp(`"${property}"\\s*:\\s*(\\d+)`), property);
}

function parseStringList(raw) {
  return [...raw.matchAll(/"((?:[^"\\]|\\.)*)"/g)].map((match) => JSON.parse(`"${match[1]}"`));
}

function parseNumberList(raw) {
  const matches = raw.match(/\d+/g);
  return matches ? matches.map((value) => Number.parseInt(value, 10)) : [];
}

function assertDeepEqual(actual, expected, label) {
  if (isDeepStrictEqual(actual, expected)) {
    return;
  }
  console.error(`[sf1-vectors] ${label} mismatch`);
  console.error("expected:", JSON.stringify(expected, null, 2));
  console.error("actual:  ", JSON.stringify(actual, null, 2));
  process.exitCode = 1;
  throw new Error(`${label} mismatch`);
}

function hexToBuffer(hex, label, expectedBytes) {
  assert.match(hex, /^[0-9a-fA-F]+$/, `${label} must be hex`);
  if (hex.length !== expectedBytes * 2) {
    fail(`${label} must be ${expectedBytes} bytes`);
  }
  return Buffer.from(hex, "hex");
}

function chunkPlanDigestSha3_256(offsets, lengths, digests) {
  assert.equal(offsets.length, lengths.length, "chunk-plan offset/length count");
  assert.equal(offsets.length, digests.length, "chunk-plan offset/digest count");
  const hasher = createHash("sha3-256");
  for (let index = 0; index < offsets.length; index += 1) {
    const metadata = Buffer.alloc(16);
    metadata.writeBigUInt64LE(BigInt(offsets[index]), 0);
    metadata.writeBigUInt64LE(BigInt(lengths[index]), 8);
    hasher.update(metadata);
    hasher.update(hexToBuffer(digests[index], `chunk digest ${index}`, 32));
  }
  return hasher.digest("hex");
}

function normalizedJsonFixture() {
  const relativePath = "fixtures/sorafs_chunker/sf1_profile_v1.json";
  const text = readText(relativePath);
  const json = JSON.parse(text);
  return {
    profile: json.profile,
    profileAliases: json.profile_aliases,
    inputSeed: json.input_seed,
    inputLength: json.input_length,
    prngMultiplier: extractJsonIntegerText(text, "multiplier"),
    prngIncrement: extractJsonIntegerText(text, "increment"),
    chunkCount: json.chunk_count,
    chunkLengths: json.chunk_lengths,
    chunkOffsets: json.chunk_offsets,
    chunkDigestSha3_256: json.chunk_digest_sha3_256,
    chunkDigestsBlake3: json.chunk_digests_blake3,
  };
}

function normalizedTypeScriptFixture() {
  const content = readText("fixtures/sorafs_chunker/sf1_profile_v1.ts");
  return {
    profile: extractString(content, /profile:\s*"([^"]+)"/, "TypeScript profile"),
    profileAliases: parseStringList(extractDelimited(content, "profileAliases:", "[", "]")),
    inputSeed: extractString(content, /inputSeed:\s*"([^"]+)"/, "TypeScript inputSeed"),
    inputLength: extractNumber(content, /inputLength:\s*(\d+)/, "TypeScript inputLength"),
    prngMultiplier: extractString(
      content,
      /prngMultiplier:\s*"(\d+)"/,
      "TypeScript prngMultiplier",
    ),
    prngIncrement: extractString(
      content,
      /prngIncrement:\s*"(\d+)"/,
      "TypeScript prngIncrement",
    ),
    chunkCount: extractNumber(content, /chunkCount:\s*(\d+)/, "TypeScript chunkCount"),
    chunkLengths: parseNumberList(extractDelimited(content, "chunkLengths: [", "[", "]")),
    chunkOffsets: parseNumberList(extractDelimited(content, "chunkOffsets: [", "[", "]")),
    chunkDigestSha3_256: extractString(
      content,
      /chunkDigestSha3_256:\s*"([0-9a-f]+)"/,
      "TypeScript chunkDigestSha3_256",
    ),
    chunkDigestsBlake3: parseStringList(
      extractDelimited(content, "chunkDigestsBlake3: [", "[", "]"),
    ),
  };
}

function normalizedRustFixture() {
  const content = readText("fixtures/sorafs_chunker/sf1_profile_v1.rs");
  return {
    profile: extractString(content, /PROFILE:\s*&str\s*=\s*"([^"]+)"/, "Rust PROFILE"),
    profileAliases: parseStringList(
      extractDelimited(content, "PROFILE_ALIASES: &[&str] = &[", "[", "]"),
    ),
    inputSeed: extractString(content, /INPUT_SEED:\s*&str\s*=\s*"([^"]+)"/, "Rust INPUT_SEED"),
    inputLength: extractNumber(content, /INPUT_LENGTH:\s*usize\s*=\s*(\d+)/, "Rust INPUT_LENGTH"),
    prngMultiplier: extractString(
      content,
      /PRNG_MULTIPLIER:\s*u64\s*=\s*(\d+)u64/,
      "Rust PRNG_MULTIPLIER",
    ),
    prngIncrement: extractString(
      content,
      /PRNG_INCREMENT:\s*u64\s*=\s*(\d+)u64/,
      "Rust PRNG_INCREMENT",
    ),
    chunkCount: extractNumber(content, /CHUNK_COUNT:\s*usize\s*=\s*(\d+)/, "Rust CHUNK_COUNT"),
    chunkLengths: parseNumberList(
      extractDelimited(content, "CHUNK_LENGTHS: [usize; 5] = [", "[", "]"),
    ),
    chunkOffsets: parseNumberList(
      extractDelimited(content, "CHUNK_OFFSETS: [usize; 5] = [", "[", "]"),
    ),
    chunkDigestSha3_256: extractString(
      content,
      /CHUNK_DIGEST_SHA3_256:\s*&str\s*=\s*"([0-9a-f]+)"/,
      "Rust CHUNK_DIGEST_SHA3_256",
    ),
    chunkDigestsBlake3: parseStringList(
      extractDelimited(content, "CHUNK_DIGESTS_BLAKE3: [&str; 5] = [", "[", "]"),
    ),
  };
}

function normalizedGoFixture() {
  const content = readText("fixtures/sorafs_chunker/sf1_profile_v1.go");
  return {
    profile: extractString(content, /Profile:\s*"([^"]+)"/, "Go Profile"),
    profileAliases: parseStringList(extractDelimited(content, "ProfileAliases:", "{", "}")),
    inputSeed: extractString(content, /InputSeed:\s*"([^"]+)"/, "Go InputSeed"),
    inputLength: extractNumber(content, /InputLength:\s*(\d+)/, "Go InputLength"),
    prngMultiplier: extractString(content, /PRNGMultiplier:\s*(\d+)/, "Go PRNGMultiplier"),
    prngIncrement: extractString(content, /PRNGIncrement:\s*(\d+)/, "Go PRNGIncrement"),
    chunkCount: extractNumber(content, /ChunkCount:\s*(\d+)/, "Go ChunkCount"),
    chunkLengths: parseNumberList(extractDelimited(content, "ChunkLengths:", "{", "}")),
    chunkOffsets: parseNumberList(extractDelimited(content, "ChunkOffsets:", "{", "}")),
    chunkDigestSha3_256: extractString(
      content,
      /ChunkDigestSHA3_256:\s*"([0-9a-f]+)"/,
      "Go ChunkDigestSHA3_256",
    ),
    chunkDigestsBlake3: parseStringList(
      extractDelimited(content, "ChunkDigestsBLAKE3:", "{", "}"),
    ),
  };
}

function assertCanonicalFixtureShape(fixture) {
  assert.equal(fixture.profile, canonicalProfile, "fixture profile");
  assert.ok(
    Array.isArray(fixture.profileAliases) && fixture.profileAliases.includes(canonicalProfile),
    "fixture profileAliases must include canonical profile",
  );
  assert.equal(fixture.chunkCount, fixture.chunkLengths.length, "chunk_lengths count");
  assert.equal(fixture.chunkCount, fixture.chunkOffsets.length, "chunk_offsets count");
  assert.equal(fixture.chunkCount, fixture.chunkDigestsBlake3.length, "chunk_digests count");
  assert.equal(
    fixture.chunkLengths.reduce((sum, value) => sum + value, 0),
    fixture.inputLength,
    "chunk lengths must sum to input length",
  );
  for (let index = 0; index < fixture.chunkOffsets.length; index += 1) {
    const expectedOffset =
      index === 0 ? 0 : fixture.chunkOffsets[index - 1] + fixture.chunkLengths[index - 1];
    assert.equal(fixture.chunkOffsets[index], expectedOffset, `chunk offset ${index}`);
  }

  const generatedInput = generateFixtureInput(fixture);
  const checkedInput = readBoundedRegularFile(
    path.join(repoRoot, "fuzz/sorafs_chunker/sf1_profile_v1_input.bin"),
    "fuzz/sorafs_chunker/sf1_profile_v1_input.bin",
  );
  assert.equal(checkedInput.length, generatedInput.length, "canonical fuzz input length");
  assert.equal(
    Buffer.compare(checkedInput, generatedInput),
    0,
    "canonical fuzz input matches the fixture PRNG",
  );
  verifyFixtureChunkDigests(fixture, generatedInput);

  const computed = chunkPlanDigestSha3_256(
    fixture.chunkOffsets,
    fixture.chunkLengths,
    fixture.chunkDigestsBlake3,
  );
  assert.equal(computed, fixture.chunkDigestSha3_256, "canonical chunk-plan transcript");

  const contentChanged = [...fixture.chunkDigestsBlake3];
  const changedDigest = Buffer.from(contentChanged[0], "hex");
  changedDigest[0] ^= 1;
  contentChanged[0] = changedDigest.toString("hex");
  assert.notEqual(
    chunkPlanDigestSha3_256(fixture.chunkOffsets, fixture.chunkLengths, contentChanged),
    fixture.chunkDigestSha3_256,
    "content mutation with unchanged boundaries must change chunk-plan digest",
  );
}

function verifyManifestMetadata(expected) {
  const manifest = readJson("fixtures/sorafs_chunker/manifest_blake3.json");
  assert.equal(manifest.profile, expected.profile, "manifest profile");
  assertDeepEqual(manifest.profile_aliases, expected.profileAliases, "manifest profile_aliases");
  assert.equal(
    manifest.chunk_digest_sha3_256,
    expected.chunkDigestSha3_256,
    "manifest chunk digest",
  );

  const expectedFiles = new Set([
    "sf1_profile_v1.json",
    "sf1_profile_v1.rs",
    "sf1_profile_v1.ts",
    "sf1_profile_v1.go",
  ]);
  const actualFiles = new Set();
  for (const file of manifest.files ?? []) {
    if (actualFiles.has(file.file)) {
      fail(`duplicate manifest file ${file.file}`);
    }
    actualFiles.add(file.file);
    if (!expectedFiles.has(file.file)) {
      fail(`unexpected manifest file ${file.file}`);
    }
    verifyManifestFile(fixturesDir, file);
  }
  assertDeepEqual([...actualFiles].sort(), [...expectedFiles].sort(), "manifest file set");
}

function verifyManifestSignatures(expected) {
  const signatures = readJson("fixtures/sorafs_chunker/manifest_signatures.json");
  assert.equal(signatures.profile, expected.profile, "signature profile");
  assertDeepEqual(signatures.profile_aliases, expected.profileAliases, "signature aliases");
  assert.equal(signatures.manifest, "manifest_blake3.json", "signature manifest name");
  assert.match(signatures.manifest_blake3, /^[0-9a-f]{64}$/, "manifest digest shape");
  assert.equal(
    signatures.chunk_digest_sha3_256,
    expected.chunkDigestSha3_256,
    "signature chunk digest",
  );
  assert.ok(Array.isArray(signatures.signatures), "signatures array");
  assert.ok(signatures.signatures.length > 0, "at least one manifest signature");

  const manifestDigest = verifySignedManifestDigest(fixturesDir, signatures);
  const seenSigners = new Set();
  for (const [index, entry] of signatures.signatures.entries()) {
    assert.equal(entry.algorithm, "ed25519", `signature ${index} algorithm`);
    const signer = hexToBuffer(entry.signer, `signature ${index} signer`, 32);
    assert.ok(!seenSigners.has(entry.signer), `signature ${index} signer is unique`);
    seenSigners.add(entry.signer);
    const signature = hexToBuffer(entry.signature, `signature ${index} signature`, 64);
    assert.equal(
      entry.signer_multihash,
      `ed0120${entry.signer.toUpperCase()}`,
      `signature ${index} signer_multihash`,
    );
    const publicKey = createPublicKey({
      key: Buffer.concat([ed25519SpkiPrefix, signer]),
      format: "der",
      type: "spki",
    });
    assert.ok(
      verify(null, manifestDigest, publicKey, signature),
      `signature ${index} verifies manifest digest`,
    );
  }
}

function main() {
  const expected = normalizedJsonFixture();
  assertCanonicalFixtureShape(expected);

  assertDeepEqual(normalizedTypeScriptFixture(), expected, "TypeScript fixture");
  assertDeepEqual(normalizedRustFixture(), expected, "Rust fixture");
  assertDeepEqual(normalizedGoFixture(), expected, "Go fixture");
  verifyManifestMetadata(expected);
  verifyManifestSignatures(expected);

  console.log("[sf1-vectors] TypeScript, Rust, and Go fixtures match JSON");
  console.log("[sf1-vectors] offset/length/BLAKE3 SHA3 transcript verified");
  console.log("[sf1-vectors] manifest metadata and Ed25519 signatures verified");
}

if (process.argv[1] && path.resolve(process.argv[1]) === __filename) {
  main();
}
