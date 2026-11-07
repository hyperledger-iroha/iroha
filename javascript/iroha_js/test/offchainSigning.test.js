import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { ed25519 } from "@noble/curves/ed25519.js";

import { NetworkId } from "../src/networkId.js";
import {
  OffchainSignaturePurposeV1,
  TORII_CANONICAL_REQUEST_DOMAIN_TAG_V1,
  buildOffchainSigningMessageV1,
  normalizeAccountSigningKey,
  offchainPurposeForConnectRawTag,
  offchainSigningDomainV1,
  verifyOffchainSignatureV1,
} from "../src/offchainSigning.js";

const vectors = JSON.parse(
  readFileSync(new URL("../../../fixtures/account/offchain_signing_vectors.json", import.meta.url), "utf8"),
);

// The canonical account bytes come from the address vectors; parsing the I105
// literal instead needs the native codec, which `npm test` builds.
function caseInputs(vector) {
  return {
    purpose: vector.purpose,
    networkId: NetworkId.fromBytes(Buffer.from(vector.network_id_hex, "hex")),
    accountCanonicalBytes: Buffer.from(vector.account_canonical_hex.slice(2), "hex"),
    generation: vector.generation,
    domainTag: vector.domain_tag,
    bytes: Buffer.from(vector.bytes_hex, "hex"),
  };
}

test("the off-chain envelope matches the shared Rust vectors byte for byte", () => {
  assert.ok(vectors.cases.length > 0);
  for (const vector of vectors.cases) {
    const message = buildOffchainSigningMessageV1(caseInputs(vector));
    assert.equal(Buffer.from(message).toString("hex"), vector.message_hex, vector.case_id);
  }
});

test("the envelope is never a 32-byte hash and starts with its domain", () => {
  const vector = vectors.cases.find((entry) => entry.case_id === "empty-bytes");
  const message = buildOffchainSigningMessageV1(caseInputs(vector));
  assert.notEqual(message.length, 32);
  assert.deepEqual(
    Buffer.from(message.subarray(0, offchainSigningDomainV1().length)),
    Buffer.from("iroha:offchain:v1\0", "utf8"),
  );
});

test("the Connect raw purpose follows the tag", () => {
  assert.equal(
    offchainPurposeForConnectRawTag(TORII_CANONICAL_REQUEST_DOMAIN_TAG_V1),
    OffchainSignaturePurposeV1.TORII_CANONICAL_REQUEST,
  );
  assert.equal(
    offchainPurposeForConnectRawTag("iroha-connect/v1/login"),
    OffchainSignaturePurposeV1.CONNECT_RAW,
  );
});

test("envelope inputs are validated", () => {
  const inputs = caseInputs(vectors.cases[0]);
  assert.throws(() => buildOffchainSigningMessageV1({ ...inputs, purpose: 3 }), /purpose/u);
  assert.throws(() => buildOffchainSigningMessageV1({ ...inputs, generation: -1 }), /generation/u);
  assert.throws(() => buildOffchainSigningMessageV1({ ...inputs, generation: 2 ** 32 }), /generation/u);
  assert.throws(() => buildOffchainSigningMessageV1({ ...inputs, domainTag: "" }), /domainTag/u);
  assert.throws(() => buildOffchainSigningMessageV1({ ...inputs, bytes: "text" }), /bytes/u);
});

test("signing keys normalize from the Torii JSON body and from bytes", () => {
  const signingPrivateKey = new Uint8Array(32).fill(0x31);
  const publicKey = ed25519.getPublicKey(signingPrivateKey);
  const hex = Buffer.from(publicKey).toString("hex").toUpperCase();
  const fromJson = normalizeAccountSigningKey({ public_key: `ed0120${hex}`, generation: 2 });
  assert.deepEqual(Buffer.from(fromJson.publicKey), Buffer.from(publicKey));
  assert.equal(fromJson.generation, 2);
  assert.equal(Object.isFrozen(fromJson), true);
  assert.deepEqual(normalizeAccountSigningKey({ publicKey, generation: 2 }), fromJson);
  for (const invalid of [
    { public_key: `ed0120${hex.toLowerCase()}`, generation: 2 },
    { public_key: `e70120${hex}`, generation: 2 },
    { publicKey: publicKey.subarray(1), generation: 2 },
    { publicKey, generation: 1.5 },
    null,
  ]) {
    assert.throws(() => normalizeAccountSigningKey(invalid));
  }
});

test("only the registered signing key verifies, and only in its own context", () => {
  const vector = vectors.cases.find((entry) => entry.case_id === "connect-raw");
  const inputs = caseInputs(vector);
  const signingPrivateKey = new Uint8Array(32).fill(0x31);
  const signingKey = { publicKey: ed25519.getPublicKey(signingPrivateKey), generation: inputs.generation };
  const signature = ed25519.sign(buildOffchainSigningMessageV1(inputs), signingPrivateKey);
  const verify = (overrides = {}) => verifyOffchainSignatureV1({
    signingKey,
    purpose: inputs.purpose,
    networkId: inputs.networkId,
    accountCanonicalBytes: inputs.accountCanonicalBytes,
    domainTag: inputs.domainTag,
    bytes: inputs.bytes,
    signature,
    ...overrides,
  });
  assert.equal(verify(), true);
  assert.equal(verify({ purpose: OffchainSignaturePurposeV1.TORII_CANONICAL_REQUEST }), false);
  assert.equal(verify({ domainTag: "iroha-connect/v1/other" }), false);
  assert.equal(verify({ bytes: Buffer.from("other") }), false);
  assert.equal(verify({ signingKey: { ...signingKey, generation: inputs.generation + 1 } }), false);
  // A signature of the bare bytes, the old raw-signing form, never verifies.
  assert.equal(verify({ signature: ed25519.sign(inputs.bytes, signingPrivateKey) }), false);
  const otherPrivateKey = new Uint8Array(32).fill(0x32);
  assert.equal(
    verify({ signature: ed25519.sign(buildOffchainSigningMessageV1(inputs), otherPrivateKey) }),
    false,
  );
  assert.equal(verify({ signature: new Uint8Array(64) }), false);
});
