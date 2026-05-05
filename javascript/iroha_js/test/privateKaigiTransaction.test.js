import test from "node:test";
import assert from "node:assert/strict";

import {
  buildPrivateKaigiFeeSpend,
  buildPrivateCreateKaigiTransaction,
  buildPrivateJoinKaigiTransaction,
  buildPrivateEndKaigiTransaction,
  submitTransactionEntrypoint,
} from "../src/transaction.js";
import { ToriiClient } from "../src/toriiClient.js";

test("buildPrivateCreateKaigiTransaction delegates to native binding", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const calls = [];
  globalThis.__IROHA_NATIVE_BINDING__ = {
    buildPrivateCreateKaigiTransaction(...args) {
      calls.push(args);
      return {
        transactionEntrypoint: Uint8Array.from([1, 2, 3]),
        hash: Uint8Array.from({ length: 32 }, (_, index) => index),
        actionHash: Uint8Array.from({ length: 32 }, (_, index) => 255 - index),
      };
    },
  };

  try {
    const result = buildPrivateCreateKaigiTransaction({
      chainId: "chain-1",
      call: { id: "wonderland:private-room", privacy_mode: "ZkRosterV1" },
      artifacts: { commitment: { commitment: "11".repeat(32) } },
      feeSpend: { asset_definition_id: "xor#universal" },
      metadata: { kaigi: { mode: "private" } },
      creationTimeMs: 1234,
      nonce: 7,
    });

    assert.equal(calls.length, 1);
    assert.equal(calls[0][0], "chain-1");
    assert.deepEqual(JSON.parse(calls[0][1]), {
      id: "wonderland:private-room",
      privacy_mode: "ZkRosterV1",
    });
    assert.deepEqual(JSON.parse(calls[0][4]), {
      kaigi: { mode: "private" },
    });
    assert.deepEqual(Array.from(result.transactionEntrypoint), [1, 2, 3]);
    assert.equal(result.hash.length, 32);
    assert.equal(result.actionHash.length, 32);
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("buildPrivateJoinKaigiTransaction and buildPrivateEndKaigiTransaction pass raw call ids", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const joinCalls = [];
  const endCalls = [];
  globalThis.__IROHA_NATIVE_BINDING__ = {
    buildPrivateJoinKaigiTransaction(...args) {
      joinCalls.push(args);
      return {
        transactionEntrypoint: Uint8Array.from([4]),
        hash: Uint8Array.from({ length: 32 }, () => 4),
        actionHash: Uint8Array.from({ length: 32 }, () => 5),
      };
    },
    buildPrivateEndKaigiTransaction(...args) {
      endCalls.push(args);
      return {
        transactionEntrypoint: Uint8Array.from([6]),
        hash: Uint8Array.from({ length: 32 }, () => 6),
        actionHash: Uint8Array.from({ length: 32 }, () => 7),
      };
    },
  };

  try {
    buildPrivateJoinKaigiTransaction({
      chainId: "chain-1",
      callId: "wonderland:private-room",
      artifacts: { commitment: { commitment: "22".repeat(32) } },
      feeSpend: { asset_definition_id: "xor#universal" },
    });
    buildPrivateEndKaigiTransaction({
      chainId: "chain-1",
      callId: "wonderland:private-room",
      endedAtMs: 4321,
      artifacts: { commitment: { commitment: "33".repeat(32) } },
      feeSpend: { asset_definition_id: "xor#universal" },
    });

    assert.equal(joinCalls[0][1], "wonderland:private-room");
    assert.equal(endCalls[0][1], "wonderland:private-room");
    assert.equal(endCalls[0][2], 4321);
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("buildPrivateKaigiFeeSpend delegates to native binding with registry vk bytes", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const calls = [];
  globalThis.__IROHA_NATIVE_BINDING__ = {
    buildPrivateKaigiFeeSpend(...args) {
      calls.push(args);
      return {
        assetDefinitionId: "xor#universal",
        anchorRoot: Uint8Array.from({ length: 32 }, () => 0xaa),
        nullifiers: [Uint8Array.from({ length: 32 }, () => 0x11)],
        outputCommitments: [Uint8Array.from({ length: 32 }, () => 0x22)],
        encryptedChangePayloads: [Uint8Array.from([0xde, 0xad])],
        proof: Uint8Array.from([7, 7, 7]),
      };
    },
  };

  try {
    const result = buildPrivateKaigiFeeSpend({
      chainId: "chain-1",
      assetDefinitionId: "xor#universal",
      actionHash: Buffer.from("33".repeat(32), "hex"),
      anchorRootHex: "44".repeat(32),
      feeAmount: "0.1",
      verifyingKey: {
        id: {
          backend: "halo2/ipa",
          name: "vk_transfer",
        },
        record: {
          circuit_id: "halo2/ipa:tiny-add",
        },
        inline_key: {
          backend: "halo2/ipa",
          bytes_b64: Buffer.from("fixture-vk", "utf8").toString("base64"),
        },
      },
    });

    assert.equal(calls.length, 1);
    assert.equal(calls[0][0], "chain-1");
    assert.equal(calls[0][1], "xor#universal");
    assert.equal(Buffer.from(calls[0][2]).toString("hex"), "33".repeat(32));
    assert.equal(calls[0][3], "44".repeat(32));
    assert.equal(calls[0][4], "0.1");
    assert.equal(calls[0][5], "halo2/ipa");
    assert.equal(calls[0][6], "halo2/ipa:tiny-add");
    assert.equal(Buffer.from(calls[0][7]).toString("utf8"), "fixture-vk");
    assert.equal(result.asset_definition_id, "xor#universal");
    assert.equal(result.nullifiers.length, 1);
    assert.equal(result.output_commitments.length, 1);
    assert.equal(result.encrypted_change_payloads.length, 1);
    assert.deepEqual(Array.from(result.proof), [7, 7, 7]);
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("submitTransactionEntrypoint waits for a terminal status", async () => {
  const client = new ToriiClient("https://example.test");
  const submitted = [];
  const polled = [];
  client.submitTransaction = async (payload) => {
    submitted.push(Buffer.from(payload));
    return { accepted: true };
  };
  client.getTransactionStatus = async (hashHex) => {
    polled.push(hashHex);
    return { status: polled.length > 1 ? "Committed" : "Pending" };
  };

  const result = await submitTransactionEntrypoint(
    client,
    Buffer.from([9, 8, 7]),
    {
      hashHex: "ab".repeat(32),
      waitForCommit: true,
      pollIntervalMs: 0,
      timeoutMs: 100,
    },
  );

  assert.deepEqual(Array.from(submitted[0]), [9, 8, 7]);
  assert.equal(polled[0], "ab".repeat(32));
  assert.equal(result.hash, "ab".repeat(32));
  assert.equal(result.status.status, "Committed");
});
