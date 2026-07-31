import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { blake2b256 } from "../src/blake2b.js";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..");

function loadJsonRelative(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  return JSON.parse(fs.readFileSync(absolutePath, "utf8"));
}

const canonicalManifest = loadJsonRelative(
  "fixtures/norito_rpc/transaction_fixtures.manifest.json",
);
const sourcePayloadFixtures = loadJsonRelative(
  "java/iroha_android/src/test/resources/transaction_payloads.json",
);

function decodeCanonicalBase64(value, context) {
  if (
    typeof value !== "string" ||
    value.length % 4 !== 0 ||
    !/^[A-Za-z0-9+/]*={0,2}$/u.test(value)
  ) {
    throw new Error(`${context} is invalid base64`);
  }
  const decoded = Buffer.from(value, "base64");
  if (decoded.toString("base64") !== value) {
    throw new Error(`${context} is non-canonical base64`);
  }
  return decoded;
}

function selectFixture(fixtures, name) {
  const match = fixtures.find((fixture) => fixture?.name === name);
  if (!match) {
    throw new Error(`Fixture '${name}' is missing`);
  }
  return match;
}

function assertUniqueFixtureIdentities(fixtures, { requireEncodedFile = false } = {}) {
  assert.ok(Array.isArray(fixtures), "fixture collection must be an array");
  const names = new Set();
  const encodedFiles = new Set();
  const payloadHashes = new Set();
  const payloadBytesValues = new Set();
  const signedHashes = new Set();
  const signedBytesValues = new Set();
  for (const fixture of fixtures) {
    assert.equal(typeof fixture?.name, "string", "fixture name must be a string");
    assert.ok(!names.has(fixture.name), `duplicate fixture name: ${fixture.name}`);
    names.add(fixture.name);
    if (requireEncodedFile) {
      assert.equal(
        typeof fixture.encoded_file,
        "string",
        `${fixture.name}: encoded_file must be a string`,
      );
      assert.ok(
        !encodedFiles.has(fixture.encoded_file),
        `duplicate fixture encoded_file: ${fixture.encoded_file}`,
      );
      encodedFiles.add(fixture.encoded_file);
    }
    assert.equal(
      typeof fixture.payload_hash,
      "string",
      `${fixture.name}: payload_hash must be a string`,
    );
    assert.equal(
      typeof fixture.payload_base64,
      "string",
      `${fixture.name}: payload_base64 must be a string`,
    );
    assert.equal(
      typeof fixture.signed_hash,
      "string",
      `${fixture.name}: signed_hash must be a string`,
    );
    assert.equal(
      typeof fixture.signed_base64,
      "string",
      `${fixture.name}: signed_base64 must be a string`,
    );
    assert.ok(
      !payloadHashes.has(fixture.payload_hash),
      `duplicate fixture payload_hash: ${fixture.payload_hash}`,
    );
    payloadHashes.add(fixture.payload_hash);
    const payloadBytes = decodeCanonicalBase64(
      fixture.payload_base64,
      `${fixture.name}.payload_base64`,
    );
    const payloadIdentity = payloadBytes.toString("hex");
    assert.ok(
      !payloadBytesValues.has(payloadIdentity),
      `duplicate fixture payload bytes: ${fixture.name}`,
    );
    payloadBytesValues.add(payloadIdentity);
    assert.ok(
      !signedHashes.has(fixture.signed_hash),
      `duplicate fixture signed_hash: ${fixture.signed_hash}`,
    );
    signedHashes.add(fixture.signed_hash);
    const signedBytes = decodeCanonicalBase64(
      fixture.signed_base64,
      `${fixture.name}.signed_base64`,
    );
    const signedIdentity = signedBytes.toString("hex");
    assert.ok(
      !signedBytesValues.has(signedIdentity),
      `duplicate fixture signed bytes: ${fixture.name}`,
    );
    signedBytesValues.add(signedIdentity);
  }
}

function validateLoadedFixtureCollections() {
  assertUniqueFixtureIdentities(sourcePayloadFixtures);
  assertUniqueFixtureIdentities(canonicalManifest.fixtures, { requireEncodedFile: true });
  assert.deepEqual(
    sourcePayloadFixtures.map(({ name }) => name).sort(),
    canonicalManifest.fixtures.map(({ name }) => name).sort(),
    "source payloads and manifest must contain exactly the same fixture names",
  );
}

validateLoadedFixtureCollections();

function irohaHashHex(bytes) {
  const digest = Buffer.from(blake2b256(bytes));
  if (digest.length > 0) {
    digest[digest.length - 1] |= 1;
  }
  return digest.toString("hex");
}

function compactLength(value) {
  let remaining = BigInt(value);
  const output = [];
  do {
    let byte = Number(remaining & 0x7fn);
    remaining >>= 7n;
    if (remaining !== 0n) byte |= 0x80;
    output.push(byte);
  } while (remaining !== 0n);
  return Buffer.from(output);
}

function readCompactLength(bytes, offset, context) {
  const start = offset;
  let value = 0n;
  for (let index = 0; index < 10; index += 1) {
    if (offset >= bytes.length) {
      throw new Error(`${context} has a truncated compact length`);
    }
    const byte = bytes[offset];
    offset += 1;
    const chunk = BigInt(byte & 0x7f);
    if (index === 9 && chunk > 1n) {
      throw new Error(`${context} compact length overflows u64`);
    }
    value |= chunk << BigInt(index * 7);
    if ((byte & 0x80) === 0) {
      if (!bytes.subarray(start, offset).equals(compactLength(value))) {
        throw new Error(`${context} has a non-canonical compact length`);
      }
      if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
        throw new Error(`${context} compact length exceeds the safe integer range`);
      }
      return { offset, value: Number(value) };
    }
  }
  throw new Error(`${context} compact length exceeds ten bytes`);
}

function readSizedField(bytes, offset, context) {
  const length = readCompactLength(bytes, offset, context);
  const end = length.offset + length.value;
  if (end > bytes.length) {
    throw new Error(`${context} length exceeds the signed transaction`);
  }
  return { bytes: bytes.subarray(length.offset, end), offset: end };
}

function signedTransactionPayload(canonicalBareSignedTransaction) {
  const bytes = Buffer.from(canonicalBareSignedTransaction);
  const signature = readSizedField(bytes, 0, "signed transaction signature");
  const payload = readSizedField(
    bytes,
    signature.offset,
    "signed transaction payload",
  );
  const multisig = readSizedField(
    bytes,
    payload.offset,
    "signed transaction multisig signatures",
  );
  if (multisig.offset !== bytes.length) {
    throw new Error("signed transaction has trailing bytes");
  }
  return payload.bytes;
}

function signedTransactionHashHex(canonicalBareSignedTransaction) {
  const payload = signedTransactionPayload(canonicalBareSignedTransaction);
  return irohaHashHex(
    Buffer.concat([
      Buffer.alloc(4),
      compactLength(payload.length),
      payload,
    ]),
  );
}

test("fixture collections reject duplicate names and encoded files before lookup", () => {
  const fixture = {
    name: "first",
    encoded_file: "first.norito",
    payload_hash: "payload-hash",
    payload_base64: "AA==",
    signed_hash: "signed-hash",
    signed_base64: "AQ==",
  };
  assert.throws(
    () => assertUniqueFixtureIdentities([fixture, { ...fixture }]),
    /duplicate fixture name: first/,
  );
  assert.throws(
    () =>
      assertUniqueFixtureIdentities(
        [
          { ...fixture, encoded_file: "shared.norito" },
          { ...fixture, name: "second", encoded_file: "shared.norito" },
        ],
        { requireEncodedFile: true },
      ),
    /duplicate fixture encoded_file: shared\.norito/,
  );
  assert.throws(
    () =>
      assertUniqueFixtureIdentities(
        [
          fixture,
          { ...fixture, name: "renamed-clone", encoded_file: "renamed-clone.norito" },
        ],
        { requireEncodedFile: true },
      ),
    /duplicate fixture payload_hash: payload-hash/,
  );
});

test("fixture collections reject invalid and non-canonical base64", () => {
  for (const encoded of ["YQ!!", "Y Q==", "YQ=", "YQ===", "YR=="]) {
    assert.throws(
      () => decodeCanonicalBase64(encoded, "adversarial fixture"),
      /(?:invalid|non-canonical) base64/,
    );
  }
});

test("signed fixture identity parser rejects malformed envelopes", () => {
  for (const [label, bytes, pattern] of [
    ["truncated", Buffer.alloc(0), /truncated compact length/u],
    ["non-canonical", Buffer.from([0x80, 0x00]), /non-canonical compact length/u],
    ["oversized", Buffer.from([0x02, 0x00]), /length exceeds/u],
    ["trailing", Buffer.from([0x00, 0x00, 0x00, 0x00]), /trailing bytes/u],
  ]) {
    assert.throws(() => signedTransactionHashHex(bytes), pattern, label);
  }
});

test("fixture base64 hashes match manifest", () => {
  for (const fixture of canonicalManifest.fixtures) {
    const payloadBytes = decodeCanonicalBase64(
      fixture.payload_base64,
      `${fixture.name}.payload_base64`,
    );
    assert.equal(
      payloadBytes.length,
      fixture.encoded_len,
      `${fixture.name}: payload length mismatch`,
    );
    assert.equal(
      irohaHashHex(payloadBytes),
      fixture.payload_hash,
      `${fixture.name}: payload hash mismatch`,
    );

    const signedBytes = decodeCanonicalBase64(
      fixture.signed_base64,
      `${fixture.name}.signed_base64`,
    );
    assert.equal(
      signedBytes.length,
      fixture.signed_len,
      `${fixture.name}: signed length mismatch`,
    );
    assert.equal(
      signedTransactionHashHex(signedBytes),
      fixture.signed_hash,
      `${fixture.name}: signed hash mismatch`,
    );
    assert.notEqual(
      irohaHashHex(signedBytes),
      fixture.signed_hash,
      `${fixture.name}: raw signed bytes must not alias compact External hash`,
    );
  }
});

test("source payload metadata matches canonical manifest metadata", () => {
  for (const fixture of canonicalManifest.fixtures) {
    const sourceFixture = selectFixture(sourcePayloadFixtures, fixture.name);
    assert.equal(
      sourceFixture.encoded,
      fixture.payload_base64,
      `${fixture.name}: source payload base64 drifted from manifest`,
    );
    assert.equal(
      sourceFixture.signed_base64,
      fixture.signed_base64,
      `${fixture.name}: signed base64 drifted from manifest`,
    );

    const payload = sourceFixture.payload;
    assert.ok(payload, `${fixture.name}: source fixture is missing payload metadata`);
    assert.equal(payload.chain, fixture.chain, `${fixture.name}: chain mismatch`);
    assert.equal(
      payload.authority,
      fixture.authority,
      `${fixture.name}: authority mismatch`,
    );
    assert.equal(
      payload.creation_time_ms,
      fixture.creation_time_ms,
      `${fixture.name}: creation_time_ms mismatch`,
    );
    assert.equal(
      payload.time_to_live_ms ?? null,
      fixture.time_to_live_ms ?? null,
      `${fixture.name}: time_to_live_ms mismatch`,
    );
    assert.equal(
      payload.nonce ?? null,
      fixture.nonce ?? null,
      `${fixture.name}: nonce mismatch`,
    );
  }
});

test("burn_asset fixture retains the expected burn wire payload", () => {
  const burnFixture = selectFixture(sourcePayloadFixtures, "burn_asset");
  const instructions = burnFixture.payload?.executable?.Instructions;
  assert.ok(Array.isArray(instructions), "burn_asset payload must carry wire instructions");
  assert.equal(instructions.length, 1, "burn_asset fixture should contain exactly one instruction");
  assert.equal(instructions[0].wire_name, "iroha.burn");
  assert.equal(typeof instructions[0].payload_base64, "string");
  assert.ok(
    instructions[0].payload_base64.length > 0,
    "burn_asset wire payload must not be empty",
  );
});
