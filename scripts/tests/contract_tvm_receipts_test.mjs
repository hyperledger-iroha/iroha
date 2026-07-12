/** Adversarial unit tests for real-TRE receipt classification. */

import assert from "node:assert/strict";
import test from "node:test";

import {
  ConfirmedTvmExecutionFailure,
  expectConfirmedTvmFailure,
  inspectTvmReceipt,
  requireSuccessfulTvmReceipt,
  sendAndConfirmTvm,
  waitForTvmTransaction,
} from "../contract_tvm_receipts.mjs";

const TX = "11".repeat(32);
const OTHER_TX = "22".repeat(32);

function receipt(result = "SUCCESS", overrides = {}) {
  return {
    id: TX,
    blockNumber: 17,
    ...(result === "SUCCESS" ? {} : { result: "FAILED" }),
    receipt: { result },
    ...overrides,
  };
}

test("identity-checked included failure is the only accepted negative", async () => {
  const failed = receipt("REVERT");
  await expectConfirmedTvmFailure(
    async () => requireSuccessfulTvmReceipt(TX, failed, "adversarial call"),
    "adversarial call",
  );
});

test("successful receipt cannot satisfy a negative test", async () => {
  await assert.rejects(
    expectConfirmedTvmFailure(
      async () => requireSuccessfulTvmReceipt(TX, receipt(), "accepted call"),
      "accepted call",
    ),
    /was accepted/,
  );
});

for (const [name, error] of [
  ["transport", new TypeError("fetch failed")],
  ["ABI", new Error("cannot encode tuple")],
  ["broadcast", new Error("node refused broadcast")],
]) {
  test(`${name} error cannot masquerade as TVM rejection`, async () => {
    await assert.rejects(
      expectConfirmedTvmFailure(async () => {
        throw error;
      }, `${name} case`),
      /without a confirmed TVM failed receipt/,
    );
  });
}

test("receipt timeout remains infrastructure failure", async () => {
  const client = { trx: { getTransactionInfo: async () => ({}) } };
  await assert.rejects(
    expectConfirmedTvmFailure(
      () => waitForTvmTransaction(client, TX, { attempts: 2, delayMs: 0 }),
      "timeout case",
    ),
    /without a confirmed TVM failed receipt/,
  );
});

for (const [name, value, pattern] of [
  ["missing id", receipt("REVERT", { id: undefined }), /transaction id/],
  ["typed id", receipt("REVERT", { id: 11 }), /transaction id must be text/],
  ["wrong id", receipt("REVERT", { id: OTHER_TX }), /another transaction/],
  ["missing block", receipt("REVERT", { blockNumber: undefined }), /block number/],
  ["fractional block", receipt("REVERT", { blockNumber: 1.5 }), /block number/],
  ["missing execution", receipt("REVERT", { receipt: {} }), /execution result/],
  ["lowercase result", receipt("revert"), /execution result/],
  [
    "missing failed marker",
    receipt("REVERT", { result: undefined }),
    /failed-transaction marker/,
  ],
  [
    "conflicting success marker",
    receipt("SUCCESS", { result: "FAILED" }),
    /result markers conflict/,
  ],
]) {
  test(`malformed ${name} receipt is not confirmed evidence`, () => {
    assert.throws(() => inspectTvmReceipt(TX, value, name), pattern);
  });
}

test("send path disables library polling and confirms the returned transaction id", async () => {
  let sentOptions;
  const method = {
    send: async (options) => {
      sentOptions = options;
      return `0x${TX}`;
    },
  };
  const client = { trx: {} };
  const result = await sendAndConfirmTvm(
    client,
    method,
    { feeLimit: 7, txLocal: false, shouldPollResponse: true },
    "valid send",
    async (receivedClient, transactionId) => {
      assert.equal(receivedClient, client);
      assert.equal(transactionId, TX);
      return receipt();
    },
  );
  assert.deepEqual(sentOptions, {
    feeLimit: 7,
    txLocal: true,
    shouldPollResponse: false,
  });
  assert.equal(result.receipt.result, "SUCCESS");
});

test("send path exposes a typed failure only after an included failed receipt", async () => {
  const method = { send: async () => TX };
  await assert.rejects(
    sendAndConfirmTvm(
      { trx: {} },
      method,
      {},
      "failed send",
      async () => receipt("OUT_OF_ENERGY"),
    ),
    (error) =>
      error instanceof ConfirmedTvmExecutionFailure &&
      error.transactionId === TX &&
      error.tvmResult === "OUT_OF_ENERGY",
  );
});

test("send path never translates ABI or broadcast errors into typed failures", async () => {
  const failure = new Error("ABI encoder rejected input");
  const method = { send: async () => { throw failure; } };
  await assert.rejects(
    sendAndConfirmTvm({ trx: {} }, method, {}, "bad send"),
    (error) => error === failure,
  );
});
