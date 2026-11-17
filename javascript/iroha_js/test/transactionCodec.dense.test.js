import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { ed25519 } from "@noble/curves/ed25519.js";
import { AccountAddress } from "../src/address.js";
import { blake2b256 } from "../src/blake2b.js";
import { NetworkId } from "../src/networkId.js";
import { assetDefinitionAddressFromUuidBytes } from "../src/norito.js";
import {
  BufferReader, decodeStructFields, encodeNoritoField, readNoritoField, validateNoritoFrame,
  withNoritoWireLayout,
} from "../src/noritoValueCodecs.js";
import {
  browserSignedTransactionHashHex, browserTransactionPayloadHashHex,
  buildBrowserTransferPayload, finalizeBrowserSignedTransaction,
  validateBrowserTransferSignable,
} from "../src/transactionCodec.js";

const load = (name) => JSON.parse(readFileSync(new URL(`../../../fixtures/norito_rpc/${name}`, import.meta.url), "utf8"));
const dense = load("transaction_fixtures.dense.json").fixtures.find((entry) => entry.name === "transfer_asset");
const current = load("transaction_payloads.json").find((entry) => entry.name === dense.name);
const densePayload = Buffer.from(dense.payload_base64, "base64");
const denseSigned = Buffer.from(dense.signed_base64, "base64");
const currentPayload = validateNoritoFrame(Buffer.from(current.payload_base64, "base64")).payload;
const currentSigned = validateNoritoFrame(Buffer.from(current.signed_base64, "base64")).payload;
const networkId = NetworkId.parse(current.network_id);
const denseOptions = { layout: "dense" };
const denseBareOptions = { layout: "dense", format: "bare" };
// Same public deterministic fixture material as tools/norito_codegen_exporter/src/norito_rpc.rs.
const privateKey = Buffer.from("android-fixture-signing-key-0102", "utf8");
const publicKey = Buffer.from(ed25519.getPublicKey(privateKey));
const markedHash = (bytes) => {
  const digest = Buffer.from(blake2b256(bytes));
  digest[31] |= 1;
  return digest;
};

function transferInput() {
  return withNoritoWireLayout("dense", () => {
    const payload = decodeStructFields(densePayload, "fixture", [
      "domain", "authority", "creation", "executable", "ttl", "nonce", "fees", "metadata", "attachments",
    ]);
    const executable = new BufferReader(payload.executable, "executable");
    assert.equal(executable.readEnumTag("tag"), 0);
    const instructions = new BufferReader(readNoritoField(executable, "instructions"), "instructions");
    assert.equal(instructions.readCount("count"), 1);
    const instruction = decodeStructFields(readNoritoField(instructions, "instruction"), "instruction", ["wireId", "container"]);
    const container = new BufferReader(instruction.container, "container");
    const transfer = new BufferReader(readNoritoField(container, "payload"), "transfer");
    assert.equal(transfer.readEnumTag("tag"), 2);
    const fields = decodeStructFields(readNoritoField(transfer, "body"), "transfer", ["source", "quantity", "destination"]);
    const source = decodeStructFields(fields.source, "source", ["owner", "definition", "scope"]);
    const destination = new BufferReader(fields.destination, "destination");
    assert.equal(destination.readEnumTag("tag"), 0);
    const key = new BufferReader(readNoritoField(destination, "key"), "key");
    assert.equal(key.readCount("count"), 33);
    assert.equal(key.readU8("algorithm"), 0);
    const destinationKey = Buffer.from(key.readBytes(32, "publicKey"));
    const quantity = decodeStructFields(fields.quantity, "quantity", ["mantissa", "scale"]);
    assert.equal(quantity.scale.readUInt32LE(0), 0);
    assert.equal(quantity.mantissa.readUInt32LE(0), 1);
    const input = {
      networkId, networkPrefix: 753, authority: current.authority,
      sourceAssetHoldingId: `${assetDefinitionAddressFromUuidBytes(source.definition)}#${current.authority}`,
      quantity: quantity.mantissa[4].toString(),
      destinationAccountId: "fixture-recipient",
      feePayment: { payer: "authority", chargeLimits: [] },
      metadata: {}, creationTimeMs: current.creation_time_ms,
      ttlMs: current.time_to_live_ms, nonce: current.nonce,
    };
    return { input, destinationKey };
  });
}

// Account literal admission is Rust-owned even in the current browser builder.
// This narrow unit-test double supplies only the two fixture-approved identities;
// every transaction byte and signature is still produced/validated by the real SDK.
function withFixtureAccountAdmission(t, callback) {
  const { input, destinationKey } = transferInput();
  t.mock.method(AccountAddress, "parseEncoded", (literal, prefix) => {
    assert.equal(prefix, 753);
    const key = literal === current.authority ? publicKey :
      literal === input.destinationAccountId ? destinationKey : null;
    assert.ok(key, "only fixture identities may be admitted by this test double");
    return {
      chainDiscriminant: 753,
      address: {
        toI105: () => literal,
        controllerInfo: () => ({ tag: 0, curve: 1, publicKey: key }),
      },
    };
  });
  return callback(input);
}

function signable(payload, layout) {
  return {
    networkId, networkPrefix: 753, authority: current.authority,
    signingPublicKey: publicKey, signatureAlgorithm: "ed25519", payloadBytes: payload,
    payloadHashHex: browserTransactionPayloadHashHex(payload, 753, { layout }),
  };
}

test("dense transfer fixture hashes and typed structure validation need no native addon", () => {
  assert.equal(browserTransactionPayloadHashHex(densePayload, 753, denseOptions), dense.signing_hash);
  const expectedIntentHash = markedHash(Buffer.concat([Buffer.of(0), withNoritoWireLayout("dense", () => encodeNoritoField(densePayload))]));
  assert.equal(browserSignedTransactionHashHex(denseSigned, 753, denseBareOptions), expectedIntentHash.toString("hex"));
  assert.equal(browserSignedTransactionHashHex(Buffer.concat([Buffer.of(1), currentSigned]), 753), current.signed_hash);
  assert.throws(() => browserSignedTransactionHashHex(denseSigned, 753), /version-1/);
  assert.throws(() => browserSignedTransactionHashHex(currentSigned, 753, denseBareOptions));
  assert.throws(() => browserSignedTransactionHashHex(denseSigned, 753, { format: "bare" }));
});

test("existing transfer builder and external signer reproduce dense fixture with fixture-only account admission", (t) => {
  withFixtureAccountAdmission(t, (input) => {
    const payload = buildBrowserTransferPayload(input, denseOptions);
    assert.deepEqual(payload, densePayload);
    const signed = signable(payload, "dense");
    assert.equal(validateBrowserTransferSignable(signed, {}, denseOptions).payloadHashHex, dense.signing_hash);
    const signature = Buffer.from(ed25519.sign(Buffer.from(dense.signing_hash, "hex"), privateKey));
    const finalized = finalizeBrowserSignedTransaction(signed, signature, publicKey, denseBareOptions);
    assert.deepEqual(finalized.signedTransaction, denseSigned);
    assert.equal(finalized.hashHex, browserSignedTransactionHashHex(denseSigned, 753, denseBareOptions));
    assert.throws(() => finalizeBrowserSignedTransaction(signed, signature, publicKey, denseOptions), /no governed versioned/);
    assert.throws(() => validateBrowserTransferSignable(signed), /domain|variant|u32/);
    assert.deepEqual(buildBrowserTransferPayload(input), currentPayload);
    const currentSignable = signable(currentPayload, "current");
    const currentSignature = ed25519.sign(Buffer.from(currentSignable.payloadHashHex, "hex"), privateKey);
    assert.deepEqual(finalizeBrowserSignedTransaction(currentSignable, currentSignature, publicKey, { format: "bare" }).signedTransaction, currentSigned);
    assert.deepEqual(finalizeBrowserSignedTransaction(currentSignable, currentSignature, publicKey).signedTransaction, Buffer.concat([Buffer.of(1), currentSigned]));
  });
});

test("dense transaction options, truncation, suffixes, and noncanonical tags fail closed", (t) => {
  for (const options of [{ layout: "guess" }, { layout: null }, { layout: "dense", extra: true }]) {
    assert.throws(() => browserTransactionPayloadHashHex(densePayload, 753, options));
  }
  assert.throws(() => browserSignedTransactionHashHex(denseSigned, 753, { layout: "dense", format: "auto" }));
  assert.throws(() => browserTransactionPayloadHashHex(Buffer.alloc(1024 * 1024 + 1), 753, denseOptions), /dense transfer payload exceeds/);
  for (const signed of [denseSigned.subarray(0, -1), Buffer.concat([denseSigned, Buffer.of(0)])]) {
    assert.throws(() => browserSignedTransactionHashHex(signed, 753, denseBareOptions));
  }
  withFixtureAccountAdmission(t, (input) => {
    // Replace Network's canonical tag 00 by the overlong varint 80 00 and
    // increase its enclosing field length; checksums cannot mask this bare error.
    const nonminimalTag = Buffer.concat([Buffer.of(densePayload[0] + 1, 0x80, 0), densePayload.subarray(2)]);
    const oversizedTag = Buffer.concat([Buffer.of(densePayload[0] + 4, 0xff, 0xff, 0xff, 0xff, 0x1f), densePayload.subarray(2)]);
    for (const payload of [densePayload.subarray(0, -1), Buffer.concat([densePayload, Buffer.of(0)]), nonminimalTag, oversizedTag]) {
      assert.throws(() => validateBrowserTransferSignable(signable(payload, "dense"), {}, denseOptions));
    }
    assert.throws(() => buildBrowserTransferPayload(input, { layout: "dense", format: "bare" }));
    assert.deepEqual(buildBrowserTransferPayload(input), currentPayload, "failure restores current layout context");
  });
});


test("dense transfer retains fee scalar widths, data space scope, and metadata values", (t) => {
  withFixtureAccountAdmission(t, (input) => {
    const assetDefinitionId = input.sourceAssetHoldingId.split("#")[0];
    const chargeLimits = [
      { kind: "nexus", assetDefinitionId, maxAmount: "1.25" },
      { kind: "pipelineGas", assetDefinitionId, maxAmount: "65536" },
    ];
    for (const feePayment of [
      { payer: "authority", chargeLimits, gasLimit: 9000n },
      { payer: "sponsor", programId: `${input.authority}/fixture`, programRevision: 300n, chargeLimits, gasLimit: 9000n },
    ]) {
      const payload = buildBrowserTransferPayload({
        ...input, sourceAssetHoldingId: `${input.sourceAssetHoldingId}#dataspace:128`,
        quantity: "10.25", nonce: null, feePayment,
        metadata: { message: "a".repeat(128), record: { count: 128, ok: true } },
      }, denseOptions);
      const signed = signable(payload, "dense");
      validateBrowserTransferSignable(signed, { networkId, networkPrefix: 753, authority: current.authority, signingPublicKey: publicKey }, denseOptions);
      const signature = ed25519.sign(Buffer.from(signed.payloadHashHex, "hex"), privateKey);
      const finalized = finalizeBrowserSignedTransaction(signed, signature, publicKey, denseBareOptions);
      assert.equal(browserSignedTransactionHashHex(finalized.signedTransaction, 753, denseBareOptions), finalized.hashHex);
    }
  });
});
