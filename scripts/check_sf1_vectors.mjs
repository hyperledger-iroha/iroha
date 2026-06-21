#!/usr/bin/env node

import assert from "node:assert/strict";
import { createPublicKey, verify } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..");
const fixturesDir = path.join(repoRoot, "fixtures", "sorafs_chunker");
const canonicalProfile = "sorafs.sf1@1.0.0";
const ed25519SpkiPrefix = Buffer.from("302a300506032b6570032100", "hex");

function readText(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function readJson(relativePath) {
  return JSON.parse(readText(relativePath));
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
    actualFiles.add(file.file);
    if (!expectedFiles.has(file.file)) {
      fail(`unexpected manifest file ${file.file}`);
    }
    assert.match(file.blake3, /^[0-9a-f]{64}$/, `${file.file} blake3 shape`);
    const stat = fs.statSync(path.join(fixturesDir, file.file));
    assert.equal(stat.size, file.size, `${file.file} size`);
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

  const manifestDigest = hexToBuffer(signatures.manifest_blake3, "manifest_blake3", 32);
  for (const [index, entry] of signatures.signatures.entries()) {
    assert.equal(entry.algorithm, "ed25519", `signature ${index} algorithm`);
    const signer = hexToBuffer(entry.signer, `signature ${index} signer`, 32);
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
  console.log("[sf1-vectors] manifest metadata and Ed25519 signatures verified");
}

main();
