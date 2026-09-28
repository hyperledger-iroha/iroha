import assert from "node:assert/strict";
import test from "node:test";
import { createConfidentialRootComputer } from "../src/confidentialProofBuilders.js";
import { createNativeRuntime } from "../src/nativeRuntime.js";
import * as publicApi from "../src/index.js";
import * as transactionApi from "../src/transaction.js";

const hex = "11".repeat(32);
test("root computation validates bounded canonical public leaves before native dispatch", async () => {
  let calls = 0;
  const compute = createConfidentialRootComputer(createNativeRuntime({ computeConfidentialRoot() { calls += 1; return Buffer.alloc(32, 1); } }));
  const tooMany = new Array(65537);
  Object.defineProperty(tooMany, 0, { get() { throw new Error("unexpected entry read"); } });
  for (const commitments of [undefined, {}, tooMany, new Array(1), [Buffer.alloc(31)], ["FF".repeat(32)], [` ${hex}`]]) {
    await assert.rejects(() => compute({ commitments }));
  }
  await assert.rejects(() => compute({ commitments: [], depth: 1 }), /not a canonical field/u);
  assert.equal(calls, 0);
  await compute({ commitments: [] });
  await compute({ commitments: Array(65536).fill(hex) });
  assert.equal(calls, 2);
});

test("root helper snapshots the native runtime, awaits native work and returns a defensive copy", async () => {
  const root = Buffer.alloc(32, 0x21), calls = [];
  let finish;
  const binding = { computeConfidentialRoot(leaves) { calls.push(leaves); return new Promise(resolve => { finish = resolve; }); } };
  const compute = createConfidentialRootComputer(createNativeRuntime(binding));
  binding.computeConfidentialRoot = () => { throw new Error("mutated binding"); };
  const pending = compute({ commitments: [Buffer.from(hex, "hex"), hex] });
  assert.ok(pending instanceof Promise);
  assert.deepEqual(calls, [[hex, hex]]);
  finish(root);
  const actual = await pending;
  root.fill(0);
  assert.deepEqual(actual, Buffer.alloc(32, 0x21));
  const malformed = createConfidentialRootComputer(createNativeRuntime({ computeConfidentialRoot: () => Buffer.alloc(31) }));
  await assert.rejects(malformed({ commitments: [] }), /must be 32 bytes/u);
  const missing = createConfidentialRootComputer(createNativeRuntime({}));
  await assert.rejects(missing({ commitments: [] }), error => error.code === "NATIVE_UNAVAILABLE");
});

test("public SDK exports canonical wallet and root APIs without caller-key proof builders", () => {
  assert.equal(typeof publicApi.ConfidentialProver, "function");
  assert.equal(typeof publicApi.computeConfidentialRoot, "function");
  for (const suffix of ["TransferProofV2", "UnshieldProofV2", "UnshieldProofV3"]) {
    for (const api of [publicApi, transactionApi]) assert.equal(api[`buildConfidential${suffix}`], undefined);
  }
});
