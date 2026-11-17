import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { ed25519 } from "@noble/curves/ed25519.js";
import { NetworkId } from "../src/networkId.js";
import { createNativeRuntime } from "../src/nativeRuntime.js";
import { _createNoritoInstructionApi } from "../src/norito.js";
import * as transactions from "../src/transaction.js";
import { nativeBinding, nativeBindingError } from "./helpers/native.js";

const load = (name) => JSON.parse(readFileSync(new URL(`../../../fixtures/norito_rpc/${name}`, import.meta.url), "utf8"));
const dense = load("transaction_fixtures.dense.json").fixtures;
const current = new Map(load("transaction_fixtures.manifest.json").fixtures.map((entry) => [entry.name, entry]));
const source = new Map(load("transaction_payloads.json").map((entry) => [entry.name, entry]));
const key = Buffer.from("android-fixture-signing-key-0102");
const prefix = 753;
const nativeOptions = { skip: nativeBindingError?.nativeStatus === "missing_file" && "native addon is absent; run npm run build:native" };

for (const fixture of dense) {
  test(`native typed dense transaction parity: ${fixture.name}`, nativeOptions, () => {
    if (!nativeBinding) throw nativeBindingError ?? new Error("native binding unavailable");
    const origin = source.get(fixture.name);
    const networkId = NetworkId.parse(origin.network_id);
    const denseOptions = { layout: "dense", format: "bare" };
    for (const [entry, options] of [[fixture, denseOptions], [current.get(fixture.name), { layout: "current", format: "framed" }]]) {
      const payload = Buffer.from(entry.payload_base64, "base64");
      const signed = Buffer.from(entry.signed_base64, "base64");
      const payloadJson = transactions.decodeTransactionPayloadJson(payload, prefix, options);
      const signedJson = transactions.decodeSignedTransaction(signed, prefix, { ...options, parseJson: false });
      // The native owner materializes TransactionPayload and typed InstructionBox values.
      assert.deepEqual(transactions.encodeTransactionPayloadJson(payloadJson, prefix, options), payload);
      assert.deepEqual(transactions.encodeSignedTransaction(signedJson, prefix, options), signed);
      const signingHash = transactions.transactionPayloadSigningHash(payload, options);
      if (options.layout === "dense") assert.equal(signingHash.toString("hex"), entry.signing_hash);
      assert.equal(transactions.hashSignedTransactionPayload(signed, options), signingHash.toString("hex"));
      assert.deepEqual(transactions.signTransactionPayload(networkId, payload, key, prefix, options), signed);
      assert.deepEqual(transactions.resignSignedTransaction(networkId, signed, key, options), signed);
      assert.throws(() => transactions.decodeTransactionPayloadJson(payload, prefix, { ...options, layout: options.layout === "dense" ? "current" : "dense" }));
      for (const bad of [payload.subarray(0, -1), Buffer.concat([payload, Buffer.from([0])])]) {
        assert.throws(() => transactions.decodeTransactionPayloadJson(bad, prefix, options));
      }
    }
    const payload = Buffer.from(fixture.payload_base64, "base64");
    const signingHash = transactions.transactionPayloadSigningHash(payload, denseOptions);
    const finalized = transactions.finalizeSignedTransaction({
      networkId, payloadBytes: payload, signature: ed25519.sign(signingHash, key),
      publicKey: ed25519.getPublicKey(key), authority: origin.authority,
    }, denseOptions);
    assert.deepEqual(finalized.signedTransaction, Buffer.from(fixture.signed_base64, "base64"));
    assert.equal(finalized.hash.toString("hex"), transactions.hashSignedTransaction(finalized.signedTransaction, denseOptions));
    const json = transactions.decodeTransactionPayloadJson(payload, prefix, denseOptions);
    const frame = transactions.encodeTransactionPayloadJson(json, prefix, { layout: "dense", format: "framed" });
    assert.equal(frame[5], 1);
    assert.equal(transactions.decodeTransactionPayloadJson(frame, prefix, { layout: "dense", format: "framed" }), json);
    const unknownMinor = Buffer.from(frame);
    unknownMinor[5] = 2;
    assert.throws(() => transactions.decodeTransactionPayloadJson(unknownMinor, prefix, { layout: "dense", format: "framed" }));
    assert.throws(() => transactions.signTransactionPayload(NetworkId.fromBytes(Buffer.alloc(32, 7)), payload, key, prefix, denseOptions));
  });
}

test("native instruction wrappers carry layout through frame and archive canonicalization", () => {
  const calls = [];
  const binding = Object.fromEntries([
    "noritoEncodeInstruction", "noritoDecodeInstruction",
    "noritoEncodeInstructionBoxArchive", "noritoDecodeInstructionBoxArchive",
  ].map((name) => [name, function (...args) {
    assert.ok(Object.isFrozen(this));
    calls.push([name, args]);
    return name.includes("Decode") ? '{"Log":{"level":"INFO","message":"layout"}}' : Buffer.from([42]);
  }]));
  const api = _createNoritoInstructionApi(createNativeRuntime(binding));
  for (const layout of ["current", "dense"]) {
    const options = { layout };
    api.noritoEncodeInstruction({ Log: { level: "INFO", message: "layout" } }, prefix, options);
    api.noritoEncodeInstruction(Buffer.from([42]), prefix, options);
    api.noritoDecodeInstruction(Buffer.from([42]), prefix, { ...options, parseJson: false });
    api.noritoEncodeInstructionBoxArchive({ Log: { level: "INFO", message: "layout" } }, prefix, options);
    api.noritoDecodeInstructionBoxArchive(Buffer.from([42]), prefix, options);
    assert.ok(calls.splice(0).every(([, args]) => args.at(-1) === layout));
  }
});

test("native transaction wrappers snapshot exact layout options and immutable contexts", () => {
  const calls = [];
  const binding = Object.fromEntries([
    "decodeTransactionPayloadJson", "encodeTransactionPayloadJson", "encodeSignedTransactionJson",
    "decodeSignedTransactionJson", "signTransactionPayload", "signTransaction",
    "transactionPayloadSigningHash", "hashSignedTransaction", "hashSignedTransactionPayload",
  ].map((name) => [name, function (...args) {
    assert.ok(Object.isFrozen(this));
    calls.push([name, args]);
    return name.startsWith("decode") ? "{}" : Buffer.alloc(32, 8);
  }]));
  const api = transactions._createTransactionApi(createNativeRuntime(binding));
  binding.decodeTransactionPayloadJson = () => { throw new Error("mutable binding used"); };
  const networkId = NetworkId.parse(source.get(dense[0].name).network_id);
  for (const layout of ["current", "dense"]) {
    const options = { layout, format: "bare" };
    api.decodeTransactionPayloadJson(Buffer.from([0]), prefix, options);
    api.encodeTransactionPayloadJson("{}", prefix, options);
    api.encodeSignedTransaction("{}", prefix, options);
    assert.equal(api.decodeSignedTransaction(Buffer.from([0]), prefix, { ...options, parseJson: false }), "{}");
    api.signTransactionPayload(networkId, Buffer.from([0]), key, prefix, options);
    api.resignSignedTransaction(networkId, Buffer.from([0]), key, options);
    api.transactionPayloadSigningHash(Buffer.from([0]), options);
    api.hashSignedTransaction(Buffer.from([0]), { ...options, encoding: "buffer" });
    api.hashSignedTransactionPayload(Buffer.from([0]), options);
    assert.ok(calls.splice(0).every(([, args]) => args.at(-2) === layout && args.at(-1) === "bare"));
  }
  api.transactionPayloadSigningHash(Buffer.from([0]));
  assert.deepEqual(calls.pop()[1].slice(-2), ["current", "bare"]);
  api.hashSignedTransaction(Buffer.from([0]));
  assert.deepEqual(calls.pop()[1].slice(-2), ["current", "versioned"]);
  for (const bad of [null, [], { layout: "Dense" }, { layout: "dense" }, { format: "auto" }, { extra: true }, Object.create({ layout: "dense" }), { get layout() { throw new Error("getter invoked"); } }, Object.defineProperty({}, "layout", { value: "dense" }), { [Symbol("layout")]: "dense" }]) {
    assert.throws(() => api.hashSignedTransaction(Buffer.from([0]), bad), (error) => !error.message.includes("getter invoked"));
  }
  assert.throws(() => api.decodeSignedTransaction(Buffer.from([0]), prefix, { parseJson: 1 }));
  assert.throws(() => api.hashSignedTransaction(Buffer.from([0]), { encoding: null }));
  assert.throws(() => api.encodeTransactionPayloadJson({}, prefix));
  assert.throws(() => api.encodeSignedTransaction({}, prefix));
  assert.equal(calls.length, 0);
});

test("native core builders and finalizer forward layout without changing defaults", async (t) => {
  const { AccountAddress } = await import("../src/address.js");
  const origin = source.get(dense[0].name);
  // Address admission remains Rust-owned; this test only inspects wrapper transport.
  t.mock.method(AccountAddress, "parseEncoded", (literal) => {
    assert.equal(literal, origin.authority);
    return { chainDiscriminant: prefix, address: { toI105: () => literal } };
  });
  const calls = [];
  const methods = ["buildTransaction", "buildRegisterDomainTransaction", "buildExecutableBatchTransaction", "buildIvmProvedTransaction", "buildTransactionPayload", "buildExecutableBatchTransactionPayload", "buildIvmProvedTransactionPayload", "signQuotedTransactionPayload", "signQuotedIvmProvedTransactionPayload", "finalizeSignedTransaction"];
  const binding = Object.fromEntries(methods.map((name) => [name, (...args) => {
    calls.push([name, args]);
    return { signedTransaction: Buffer.from([1]), hash: Buffer.alloc(32), payloadJson: "{}", payloadBytes: Buffer.from([2]), payloadHash: Buffer.alloc(32) };
  }]));
  const api = transactions._createTransactionApi(createNativeRuntime(binding));
  const base = {
    networkId: NetworkId.parse(origin.network_id), authority: origin.authority,
    privateKey: key, feePayment: { payer: "authority", chargeLimits: [] },
    instructions: [{ Log: { level: "INFO", message: "layout" } }],
    domainId: "wonderland", proved: {}, attachment: {},
    entries: [{ kind: "instruction", instruction: { Log: { level: "INFO", message: "layout" } } }],
    payload: { payloadJson: "{}", attachment: {} }, quotedFeePayment: { payer: "authority", chargeLimits: [] },
  };
  for (const layout of ["current", "dense"]) {
    for (const name of methods) {
      const payloadBuilder = name.startsWith("build") && name.endsWith("Payload");
      const options = payloadBuilder ? { layout } : { layout, format: "bare" };
      const input = name === "finalizeSignedTransaction" ? {
        networkId: base.networkId, authority: base.authority, payloadBytes: Buffer.from([0]),
        signature: Buffer.alloc(64), publicKey: Buffer.alloc(32),
      } : base;
      api[name](input, options);
      const [called, args] = calls.pop();
      assert.equal(called, name);
      assert.deepEqual(args.slice(payloadBuilder ? -1 : -2), payloadBuilder ? [layout] : [layout, "bare"]);
      assert.throws(() => api[name](input, { ...options, unknown: true }));
    }
  }
  api.buildMintAssetTransaction({ ...base, assetHoldingId: `62Fk4FPcMuLvW5QjDGNF2a4jAmjM#${base.authority}`, quantity: "1" }, { layout: "dense", format: "bare" });
  assert.deepEqual(calls.pop()[1].slice(-2), ["dense", "bare"]);
  api.buildTransaction(base);
  assert.deepEqual(calls.pop()[1].slice(-2), ["current", "versioned"]);
});
