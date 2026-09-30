// Synchronous request ownership and the optional receipt-decoder boundary.
import assert from "node:assert/strict";
import { resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";
import { BUNDLE_TARGETS, analyzeSplitBundle } from "../scripts/bundle-size-check.mjs";
import {
  LocalSigningContext,
  SorafsOrderbookSubmissionAmbiguousError,
  ToriiClient,
} from "../src/toriiClient.js";
import { NetworkId } from "../src/networkId.js";
import { prepareSorafsOrderbookSubmission, SORAFS_ORDERBOOK_TRANSACTION_MAX_BYTES_V1 } from "../src/sorafsOrderbookPreflight.js";
import { TORII_TEST_NATIVE_BINDING } from "../src/toriiTestHooks.js";

const NETWORK = NetworkId.parse(
  "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
);

for (const [method, route, path] of [
  ["submitSorafsOrderbookOrder", "order", "orders"],
  ["submitSorafsOrderbookCancel", "cancel", "cancel"],
  ["submitSorafsOrderbookReceipt", "receipt", "receipts"],
]) {
  test(`${route} takes custody before its first asynchronous boundary`, async () => {
    const trace = [];
    const identity = { entrypointHash: "ab".repeat(32), signedTransactionHash: "cd".repeat(32) };
    const transportError = new Error("original transport result");
    const signed = Uint8Array.of(1, 2, 3);
    const options = { expectedReceiptSigner: "original signer" };
    let releaseValidation;
    const client = new ToriiClient("https://torii.example", {
      localSigningContext: new LocalSigningContext(NETWORK, 369),
      timeoutMs: 5_000,
      [TORII_TEST_NATIVE_BINDING]: {
        inspectSorafsOrderbookSubmissionForDiscriminantV1(actualRoute, network, chain, signer, body) {
          trace.push("inspect");
          assert.equal(actualRoute, route);
          assert.equal(network.length, 32);
          assert.equal(chain, 369);
          assert.equal(signer, "original signer");
          assert.deepEqual([...body], [1, 2, 3]);
          return identity;
        },
        verifySorafsOrderbookSubmissionReceiptV1() {
          assert.fail("a failed transport has no receipt to verify");
        },
      },
      fetchImpl: async (url, init) => {
        trace.push("dispatch");
        assert.equal(url, `https://torii.example/v1/sorafs/orderbook/${path}`);
        assert.deepEqual([...init.body], [1, 2, 3]);
        assert.equal(init.headers["Content-Type"], "application/x-norito");
        assert.equal(init.headers.Accept, "application/x-norito");
        assert.equal(init.headers["Accept-Encoding"], "identity");
        assert.equal(init.redirect, "error");
        throw transportError;
      },
    });
    client._ensureDataModelValidation = () => {
      trace.push("validation");
      return new Promise((resolveValidation) => { releaseValidation = resolveValidation; });
    };
    const pending = client[method](signed, options);
    assert.deepEqual(trace, ["inspect", "validation"]);
    signed.fill(255);
    options.expectedReceiptSigner = "replacement signer";
    identity.signedTransactionHash = "ef".repeat(32);
    client._request = () => assert.fail("request changed after custody");
    client._ensureDataModelValidation = () => assert.fail("validation changed after custody");
    releaseValidation();
    await assert.rejects(pending, (error) => {
      assert.ok(error instanceof SorafsOrderbookSubmissionAmbiguousError);
      assert.strictEqual(error.cause, transportError);
      assert.equal(error.expectedIdentity.signedTransactionHash, "cd".repeat(32));
      return true;
    });
    assert.deepEqual(trace, ["inspect", "validation", "dispatch"]);
  });
}

test("orderbook preflight is eager and receipt decoding stays in the existing optional chunk", async () => {
  const root = fileURLToPath(new URL("..", import.meta.url));
  const target = BUNDLE_TARGETS.find(({ label }) => label === "toriiClient.js");
  const result = await build({
    absWorkingDir: root, entryPoints: [target.entryPoint], bundle: true, splitting: true,
    write: false, outdir: resolve(root, ".orderbook-boundary-audit"), entryNames: "entry",
    chunkNames: "[hash]", platform: target.platform, target: target.target,
    format: "esm", treeShaking: true, minify: true, metafile: true, charset: "utf8",
  });
  const metrics = analyzeSplitBundle(result, target);
  const contains = (outputs, name) => outputs.some((output) =>
    Object.hasOwn(result.metafile.outputs[output].inputs, `src/${name}`));
  assert.ok(contains(metrics.eagerOutputs, "sorafsOrderbookPreflight.js"));
  assert.equal(contains(metrics.eagerOutputs, "sorafsOrderbookSubmission.js"), false);
  const optional = metrics.lazyChunks.find(({ specifier }) => specifier === "./toriiOptional.js");
  assert.ok(contains(optional.outputs, "sorafsOrderbookSubmission.js"));
});

function prepareBytes(signedTransaction, inspect) {
  return prepareSorafsOrderbookSubmission({
    route: "order", signedTransaction, expectedNetworkIdBytes: new Uint8Array(32),
    expectedChainDiscriminant: 369, expectedReceiptSigner: "signer", context: "submit",
    native: {
      inspectSorafsOrderbookSubmissionForDiscriminantV1: inspect,
      verifySorafsOrderbookSubmissionReceiptV1() {},
    },
  });
}

test("orderbook enforces branded byte bounds before copying or inspecting", () => {
  const large = new ArrayBuffer(SORAFS_ORDERBOOK_TRANSACTION_MAX_BYTES_V1 + 1);
  const inputs = [large, new Uint8Array(large), new DataView(large)];
  for (const input of inputs) {
    Object.defineProperty(input, "byteLength", { value: 1 });
    Object.defineProperty(input, "byteOffset", { value: 0 });
    Object.defineProperty(input, "buffer", { value: new ArrayBuffer(1) });
  }
  const from = Buffer.from;
  let copies = 0;
  Buffer.from = (...args) => { copies += 1; return from(...args); };
  try {
    for (const input of inputs) {
      assert.throws(() => prepareBytes(input, () => assert.fail("oversized bytes reached native inspection")),
        { name: "RangeError", message: `submit.signedTransaction must contain 1..${SORAFS_ORDERBOOK_TRANSACTION_MAX_BYTES_V1} bytes` });
    }
    assert.equal(copies, 0, "rejected geometry must not allocate a byte copy");
  } finally {
    Buffer.from = from;
  }
});

test("orderbook snapshots exact typed-array and DataView windows without own getters", () => {
  for (const view of [new Uint16Array(new ArrayBuffer(8), 2, 2), new DataView(new ArrayBuffer(8), 2, 4)]) {
    const bytes = new Uint8Array(view.buffer, 2, 4);
    bytes.set([1, 2, 3, 4]);
    for (const name of ["buffer", "byteOffset", "byteLength"]) {
      Object.defineProperty(view, name, { get() { assert.fail("untrusted geometry getter executed"); } });
    }
    const prepared = prepareBytes(view, (_route, _network, _chain, _signer, body) => {
      assert.deepEqual([...body], [1, 2, 3, 4]);
      return { entrypointHash: "ab".repeat(32), signedTransactionHash: "cd".repeat(32) };
    });
    bytes.fill(9);
    assert.deepEqual([...prepared.body], [1, 2, 3, 4]);
  }
});

test("orderbook rejects shared backing and forged byte-view lookalikes before native inspection", () => {
  const shared = new SharedArrayBuffer(8);
  for (const input of [shared, new Uint8Array(shared), new DataView(shared), { buffer: new ArrayBuffer(1), byteOffset: 0, byteLength: 1 }]) {
    assert.throws(() => prepareBytes(input, () => assert.fail("unstable bytes reached native inspection")),
      { name: "TypeError", message: "submit.signedTransaction must be exact bytes backed by an ordinary ArrayBuffer" });
  }
});
