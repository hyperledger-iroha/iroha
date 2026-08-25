import assert from "node:assert/strict";
import test from "node:test";

import { AccountAddress } from "../src/address.js";
import { NetworkId } from "../src/networkId.js";
import {
  buildMintAssetTransaction,
  buildTransaction,
} from "../src/transaction.js";

const NETWORK_ID = NetworkId.parse(
  "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149",
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
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = {
    buildTransaction(...args) {
      calls.push(args);
      return {
        signed_transaction: Uint8Array.from([1, 2, 3]),
        hash: Uint8Array.from({ length: 32 }, () => 0xff),
      };
    },
  };
  try {
    const built = buildTransaction(BASE_INPUT);
    assert.deepEqual(calls[0][0], Buffer.from(NETWORK_ID.toBytes()));
    assert.equal(calls[0][0].length, NetworkId.BYTE_LENGTH);
    assert.deepEqual(built.signedTransaction, Buffer.from([1, 2, 3]));
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
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
