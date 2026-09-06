import assert from "node:assert/strict";
import test from "node:test";

import { AccountAddress } from "../src/address.js";
import { createNativeRuntime } from "../src/nativeRuntime.js";
import { NetworkId } from "../src/networkId.js";
import * as transactionModule from "../src/transaction.js";

const { _createTransactionApi } = transactionModule;

const NETWORK_ID = NetworkId.parse(
  "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
);
const AUTHORITY = AccountAddress.fromAccount({
  publicKey: Buffer.from(
    "d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737",
    "hex",
  ),
}).toI105();
const INPUT = Object.freeze({
  networkId: NETWORK_ID,
  authority: AUTHORITY,
  instructions: Object.freeze([
    Object.freeze({ Log: Object.freeze({ level: "INFO", message: "runtime" }) }),
  ]),
  feePayment: Object.freeze({ payer: "authority", chargeLimits: Object.freeze([]) }),
  privateKey: Buffer.alloc(32, 0x11),
});

function fakeBinding(tag, byte) {
  return {
    tag,
    buildTransaction(...args) {
      assert.equal(Object.getPrototypeOf(this), null);
      assert.equal(Object.isFrozen(this), true);
      assert.equal(args.at(-1), null);
      return {
        signed_transaction: Buffer.from([byte]),
        hash: Buffer.alloc(32, this.tag.charCodeAt(0)),
      };
    },
  };
}

test("transaction APIs isolate immutable native runtimes across parallel calls", async () => {
  const bindingA = fakeBinding("A", 0xa1);
  const bindingB = fakeBinding("B", 0xb2);
  const apiA = _createTransactionApi(createNativeRuntime(bindingA));
  const apiB = _createTransactionApi(createNativeRuntime(bindingB));

  bindingA.tag = "mutated";
  bindingA.buildTransaction = () => {
    throw new Error("post-construction mutation must not be observed");
  };

  const [resultA, resultB] = await Promise.all([
    Promise.resolve().then(() => apiA.buildTransaction(INPUT)),
    Promise.resolve().then(() => apiB.buildTransaction(INPUT)),
  ]);

  assert.equal(Object.isFrozen(apiA), true);
  assert.deepEqual(resultA.signedTransaction, Buffer.from([0xa1]));
  assert.deepEqual(resultB.signedTransaction, Buffer.from([0xb2]));
  assert.equal(resultA.hash[0], "A".charCodeAt(0));
  assert.equal(resultB.hash[0], "B".charCodeAt(0));

  const composite = apiA.buildMintAssetTransaction({
    networkId: NETWORK_ID,
    authority: AUTHORITY,
    assetHoldingId: `62Fk4FPcMuLvW5QjDGNF2a4jAmjM#${AUTHORITY}`,
    quantity: "1",
    feePayment: INPUT.feePayment,
    privateKey: INPUT.privateKey,
  });
  assert.deepEqual(composite.signedTransaction, Buffer.from([0xa1]));
});

test("transaction runtime facade covers every local transaction function", () => {
  const api = _createTransactionApi(createNativeRuntime({}));
  const sourceOnlyFunctions = new Set([
    "_createTransactionApi",
    "requirePrivacyExact12CapabilityAdmissionV1",
  ]);
  const expected = Object.entries(transactionModule)
    .filter(([name, value]) =>
      typeof value === "function" && !sourceOnlyFunctions.has(name))
    .map(([name]) => name)
    .sort();

  assert.deepEqual(Object.keys(api).sort(), expected);
});

test("transaction composites reject an explicit null privateKeyAlgorithm", () => {
  let nativeCalls = 0;
  const api = _createTransactionApi(createNativeRuntime({
    buildTransaction() {
      nativeCalls += 1;
      throw new Error("native builder must not run");
    },
  }));

  assert.throws(
    () => api.buildMintAssetTransaction({
      networkId: NETWORK_ID,
      authority: AUTHORITY,
      assetHoldingId: `62Fk4FPcMuLvW5QjDGNF2a4jAmjM#${AUTHORITY}`,
      quantity: "1",
      feePayment: INPUT.feePayment,
      privateKey: INPUT.privateKey,
      privateKeyAlgorithm: null,
    }),
    /privateKeyAlgorithm must be a supported crypto algorithm string/u,
  );
  assert.equal(nativeCalls, 0);
});
