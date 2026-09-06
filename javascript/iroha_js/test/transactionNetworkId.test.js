import assert from "node:assert/strict";
import test from "node:test";

import { AccountAddress } from "../src/address.js";
import { NetworkId } from "../src/networkId.js";
import {
  _createTransactionApi,
  buildMintAssetTransaction,
  buildTransaction,
} from "../src/transaction.js";
import { createNativeRuntime } from "../src/nativeRuntime.js";

const NETWORK_ID = NetworkId.parse(
  "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
);
const AUTHORITY = AccountAddress.fromAccount({
  publicKey: Buffer.from(
    "d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737",
    "hex",
  ),
}).toI105();
const BASE_INPUT = Object.freeze({
  networkId: NETWORK_ID,
  authority: AUTHORITY,
  instructions: Object.freeze([
    Object.freeze({ Log: Object.freeze({ level: "INFO", message: "network" }) }),
  ]),
  feePayment: Object.freeze({ payer: "authority", chargeLimits: Object.freeze([]) }),
  privateKey: Uint8Array.from({ length: 32 }, () => 7),
});

test("ordinary transaction host entrypoints forward exactly 32 NetworkId bytes", () => {
  const calls = [];
  const transaction = _createTransactionApi(createNativeRuntime({
    buildTransaction(...args) {
      calls.push(args);
      return {
        signed_transaction: Uint8Array.from([1, 2, 3]),
        hash: Uint8Array.from({ length: 32 }, () => 0xff),
      };
    },
  }));
  const built = transaction.buildTransaction(BASE_INPUT);
  assert.deepEqual(calls[0][0], Buffer.from(NETWORK_ID.toBytes()));
  assert.equal(calls[0][0].length, NetworkId.BYTE_LENGTH);
  assert.deepEqual(built.signedTransaction, Buffer.from([1, 2, 3]));
});

test("ordinary transaction APIs reject chain aliases and structural NetworkId substitutes", () => {
  for (const field of ["chain", "chainId", "chain_id"]) {
    assert.throws(
      () => buildTransaction({ ...BASE_INPUT, [field]: "legacy-chain" }),
      new RegExp(`input\\.${field} is unsupported`, "u"),
      `generic ${field}`,
    );
    assert.throws(
      () =>
        buildMintAssetTransaction({
          ...BASE_INPUT,
          [field]: "legacy-chain",
          assetHoldingId: `asset#${AUTHORITY}`,
          quantity: "1",
        }),
      new RegExp(`input\\.${field} is unsupported`, "u"),
      `convenience ${field}`,
    );
  }

  for (const networkId of [
    NETWORK_ID.literal,
    NETWORK_ID.toBytes(),
    { literal: NETWORK_ID.literal, toBytes: () => NETWORK_ID.toBytes() },
  ]) {
    assert.throws(
      () => buildTransaction({ ...BASE_INPUT, networkId }),
      /input\.networkId must be a NetworkId/u,
    );
  }
});
