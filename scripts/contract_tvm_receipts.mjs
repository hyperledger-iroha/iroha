/** Receipt classification shared by the real-TRE SCCP contract smoke. */

import assert from "node:assert/strict";

const TRANSACTION_ID = /^[0-9a-f]{64}$/;

function canonicalTransactionId(value, label) {
  assert.equal(typeof value, "string", `${label} transaction id must be text`);
  const result = value.toLowerCase().replace(/^0x/, "");
  assert.match(result, TRANSACTION_ID, `${label} has no canonical transaction id`);
  return result;
}

/** A transaction that a TRE block actually included with a non-success TVM result. */
export class ConfirmedTvmExecutionFailure extends Error {
  constructor(label, transactionId, receipt, tvmResult) {
    super(`${label} produced confirmed TVM failure ${tvmResult}`);
    this.name = "ConfirmedTvmExecutionFailure";
    this.transactionId = transactionId;
    this.receipt = receipt;
    this.tvmResult = tvmResult;
  }
}

/** Validate that a transaction-info document is for the requested included transaction. */
export function inspectTvmReceipt(transactionId, receipt, label) {
  const expectedId = canonicalTransactionId(transactionId, label);
  assert(
    receipt && typeof receipt === "object" && !Array.isArray(receipt),
    `${label} receipt is absent`,
  );
  assert.equal(
    canonicalTransactionId(receipt.id, `${label} receipt`),
    expectedId,
    `${label} receipt belongs to another transaction`,
  );
  assert(
    Number.isSafeInteger(receipt.blockNumber) && receipt.blockNumber >= 0,
    `${label} receipt has no canonical block number`,
  );
  assert(
    receipt.receipt &&
      typeof receipt.receipt === "object" &&
      !Array.isArray(receipt.receipt),
    `${label} receipt has no TVM execution result`,
  );
  const result = receipt.receipt.result;
  assert(
    typeof result === "string" && /^[A-Z][A-Z0-9_]{0,63}$/.test(result),
    `${label} receipt has a malformed TVM execution result`,
  );
  if (result !== "SUCCESS") {
    assert.equal(
      receipt.result,
      "FAILED",
      `${label} receipt does not carry the failed-transaction marker`,
    );
  } else if (receipt.result !== undefined) {
    assert.equal(receipt.result, "SUCCESS", `${label} receipt result markers conflict`);
  }
  return { transactionId: expectedId, result };
}

/** Return a successful receipt or throw only the typed, confirmed failure above. */
export function requireSuccessfulTvmReceipt(transactionId, receipt, label) {
  const inspected = inspectTvmReceipt(transactionId, receipt, label);
  if (inspected.result !== "SUCCESS") {
    throw new ConfirmedTvmExecutionFailure(
      label,
      inspected.transactionId,
      receipt,
      inspected.result,
    );
  }
  return receipt;
}

/** Poll TRE for one bounded transaction receipt without translating transport failures. */
export async function waitForTvmTransaction(
  client,
  transactionId,
  { attempts = 120, delayMs = 500 } = {},
) {
  canonicalTransactionId(transactionId, "TVM transaction");
  assert(Number.isInteger(attempts) && attempts > 0, "receipt attempts must be positive");
  assert(Number.isInteger(delayMs) && delayMs >= 0, "receipt delay must be nonnegative");
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    const info = await client.trx.getTransactionInfo(transactionId);
    if (
      info &&
      typeof info === "object" &&
      !Array.isArray(info) &&
      Object.keys(info).length !== 0
    ) {
      return info;
    }
    if (attempt + 1 < attempts && delayMs !== 0) {
      await new Promise((resolve) => setTimeout(resolve, delayMs));
    }
  }
  throw new Error("TVM transaction did not produce a receipt within the bounded polling window");
}

/** Broadcast one contract method and independently classify its included receipt. */
export async function sendAndConfirmTvm(
  client,
  method,
  options,
  label,
  waitForReceipt = waitForTvmTransaction,
) {
  assert(method && typeof method.send === "function", `${label} has no contract send method`);
  const transactionId = await method.send({
    ...options,
    // Build locally so a reverting call is still broadcast and produces the
    // on-chain failed receipt required as negative evidence. The default TRE
    // trigger endpoint may reject during simulation without mining anything.
    txLocal: true,
    shouldPollResponse: false,
  });
  const canonicalId = canonicalTransactionId(transactionId, label);
  const receipt = await waitForReceipt(client, canonicalId);
  return requireSuccessfulTvmReceipt(canonicalId, receipt, label);
}

/** Accept only a typed failure backed by an identity-checked, included TRE receipt. */
export async function expectConfirmedTvmFailure(action, label) {
  try {
    await action();
  } catch (error) {
    if (error instanceof ConfirmedTvmExecutionFailure) {
      const inspected = inspectTvmReceipt(error.transactionId, error.receipt, label);
      assert.notEqual(inspected.result, "SUCCESS", `${label} carried a successful receipt`);
      assert.equal(inspected.result, error.tvmResult, `${label} failure result changed`);
      return error.receipt;
    }
    throw new Error(
      `${label} failed without a confirmed TVM failed receipt; ` +
        "transport, ABI, and timeout errors are not rejection evidence",
      { cause: error },
    );
  }
  assert.fail(`${label} was accepted`);
}
