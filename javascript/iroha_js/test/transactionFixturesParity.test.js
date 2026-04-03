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

function selectFixture(fixtures, name) {
  const match = fixtures.find((fixture) => fixture?.name === name);
  if (!match) {
    throw new Error(`Fixture '${name}' is missing`);
  }
  return match;
}

function irohaHashHex(bytes) {
  const digest = Buffer.from(blake2b256(bytes));
  if (digest.length > 0) {
    digest[digest.length - 1] |= 1;
  }
  return digest.toString("hex");
}

test("fixture base64 hashes match manifest", () => {
  for (const fixture of canonicalManifest.fixtures) {
    const payloadBytes = Buffer.from(fixture.payload_base64, "base64");
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

    const signedBytes = Buffer.from(fixture.signed_base64, "base64");
    assert.equal(
      signedBytes.length,
      fixture.signed_len,
      `${fixture.name}: signed length mismatch`,
    );
    assert.equal(
      irohaHashHex(signedBytes),
      fixture.signed_hash,
      `${fixture.name}: signed hash mismatch`,
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
