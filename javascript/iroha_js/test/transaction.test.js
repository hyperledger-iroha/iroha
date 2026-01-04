import { test as baseTest } from "node:test";
import assert from "node:assert/strict";
import {
  hashSignedTransaction,
  resignSignedTransaction,
  submitSignedTransaction,
  buildMintAndTransferTransaction,
  buildBurnAssetTransaction,
  buildBurnTriggerTransaction,
  buildRegisterDomainAndMintTransaction,
  buildRegisterAccountAndTransferTransaction,
  buildRegisterAssetDefinitionAndMintTransaction,
  buildRegisterAssetDefinitionMintAndTransferTransaction,
} from "../src/transaction.js";
import { ToriiClient } from "../src/toriiClient.js";
import { makeNativeTest } from "./helpers/native.js";

const BASE_URL = "http://localhost:8080";
const AUTHORITY_ID_RAW =
  "ED0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245@wonderland";
const AUTHORITY_ID =
  "34mSYnDgbaJM58rbLoif4Tkp7G8Y5nHVgN4msza9MxHNAxLyVUNWFijRCZkkDnFVEME45izNT@wonderland";
const AUTHORITY_ID_INPUT = AUTHORITY_ID_RAW.toLowerCase();
const PRIVATE_KEY = Buffer.alloc(32, 0x11);
const ASSET_ID = `rose##${AUTHORITY_ID}`;
const ASSET_ID_INPUT = `rose##${AUTHORITY_ID_INPUT}`;
const NEW_ACCOUNT_ID =
  "ED0120C0F6FA775885F8FFB5F203C10EAA90E1B49FB4CD39C0F95CCA1E5A02B5E45F61@wonderland";
const NEW_ACCOUNT_ID_INPUT = NEW_ACCOUNT_ID.toLowerCase();
const ASSET_DEFINITION_ID = "rose#wonderland";
const ASSET_DEFINITION_ID_INPUT = ASSET_DEFINITION_ID.toLowerCase();
const test = makeNativeTest(baseTest);

test("hashSignedTransaction delegates to native binding and returns hex", () => {
  const input = Buffer.from([0xde, 0xad]);
  const fakeHash = Buffer.from([0x01, 0x02, 0x03, 0x04]);
  withNativeBinding({ hashSignedTransaction: () => fakeHash }, () => {
    const result = hashSignedTransaction(input);
    assert.equal(result, fakeHash.toString("hex"));
  });
});

test("submitSignedTransaction submits payload and polls status until terminal", async () => {
  const txBytes = Buffer.from([0xaa]);
  const signedBytes = Buffer.from([0xbb]);
  const hashBytes = Buffer.from([0x10, 0x11, 0x12, 0x13]);
  const submissionResponse = createResponse({
    status: 202,
    jsonData: { status: "Accepted" },
    headers: { "content-type": "application/json" },
  });
  const statusQueue = [
    createResponse({
      status: 202,
      jsonData: { status: "Accepted" },
      headers: { "content-type": "application/json" },
    }),
    createResponse({
      status: 200,
      jsonData: { status: "Pending" },
      headers: { "content-type": "application/json" },
    }),
    createResponse({
      status: 200,
      jsonData: { status: "Committed" },
      headers: { "content-type": "application/json" },
    }),
  ];
  const calls = [];
  const fetchImpl = async (url, init) => {
    calls.push({ url, init });
    if (url.endsWith("/v1/pipeline/transactions")) {
      assert.ok(Buffer.isBuffer(init.body));
      assert.deepEqual([...init.body.values()], [...signedBytes.values()]);
      return submissionResponse;
    }
    return statusQueue.shift() ?? statusQueue[statusQueue.length - 1];
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const binding = {
    hashSignedTransaction: () => hashBytes,
    signTransaction: (tx) => {
      assert.deepEqual([...tx.values()], [...txBytes.values()]);
      return signedBytes;
    },
  };

  const result = await withNativeBinding(binding, () =>
    submitSignedTransaction(client, txBytes, {
      waitForCommit: true,
      pollIntervalMs: 0,
      privateKey: Buffer.alloc(32, 0x01),
    }),
  );

  assert.equal(result.hash, hashBytes.toString("hex"));
  assert.deepEqual(result.status, { status: "Committed" });
  assert.equal(calls.length >= 3, true);
});

test("submitSignedTransaction times out when no terminal status", async () => {
  const txBytes = Buffer.from([0x99]);
  const binding = {
    hashSignedTransaction: () => Buffer.alloc(32, 0x42),
    signTransaction: () => txBytes,
  };
  const fetchImpl = async () =>
    createResponse({
      status: 202,
      jsonData: { status: "Accepted" },
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, { fetchImpl });

  await withNativeBinding(binding, async () => {
    await assert.rejects(
      () =>
        submitSignedTransaction(client, txBytes, {
          waitForCommit: true,
          timeoutMs: 1,
          pollIntervalMs: 0,
          privateKey: Buffer.alloc(32, 0x02),
        }),
      /timed out/i,
    );
  });
});

test("submitSignedTransaction ignores state-only terminal fields", async () => {
  const txBytes = Buffer.from([0x44]);
  const signedBytes = Buffer.from([0x55]);
  const binding = {
    hashSignedTransaction: () => Buffer.alloc(32, 0x33),
    signTransaction: () => signedBytes,
  };
  const submissionResponse = createResponse({
    status: 202,
    jsonData: { status: "Accepted" },
    headers: { "content-type": "application/json" },
  });
  const statusResponse = createResponse({
    status: 200,
    jsonData: { state: "Committed" },
    headers: { "content-type": "application/json" },
  });
  const fetchImpl = async (url) => {
    if (url.endsWith("/v1/pipeline/transactions")) {
      return submissionResponse;
    }
    return statusResponse;
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });

  await withNativeBinding(binding, async () => {
    await assert.rejects(
      () =>
        submitSignedTransaction(client, txBytes, {
          waitForCommit: true,
          timeoutMs: 5,
          pollIntervalMs: 0,
          privateKey: Buffer.alloc(32, 0x03),
        }),
      /timed out/i,
    );
  });
});

test("resignSignedTransaction delegates to native binding", () => {
  const input = Buffer.from([0xde]);
  const key = Buffer.alloc(32, 0x11);
  const output = Buffer.from([0xef]);
  withNativeBinding(
    {
      signTransaction: (tx, pk) => {
        assert.deepEqual([...tx.values()], [...input.values()]);
        assert.deepEqual([...pk.values()], [...key.values()]);
        return output;
      },
    },
    () => {
      const result = resignSignedTransaction(input, key);
      assert.deepEqual(result, output);
    },
  );
});

test("buildMintAndTransferTransaction yields multi-instruction payload", () => {
  const captures = [];
  const result = withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({ authority, instructions: instructions.map((j) => JSON.parse(j)) });
        return {
          signed_transaction: Buffer.from([0x01]),
          hash: Buffer.alloc(32, 0x22),
        };
      },
    },
    () =>
      buildMintAndTransferTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        mint: { assetId: ASSET_ID_INPUT, quantity: "7" },
        transfer: {
          quantity: "3",
          destinationAccountId: AUTHORITY_ID_INPUT,
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [
    { authority, instructions },
  ] = captures;
  assert.equal(authority, AUTHORITY_ID);
  assert.equal(instructions.length, 2);
  assert.equal(result.hash.length, 32);
  assert.deepEqual(instructions[0], {
    Mint: { Asset: { destination: ASSET_ID, object: "7" } },
  });
  assert.deepEqual(instructions[1], {
    Transfer: {
      Asset: {
        source: ASSET_ID,
        object: "3",
        destination: AUTHORITY_ID,
      },
    },
  });
});

test("buildBurnAssetTransaction yields single burn instruction", () => {
  const captures = [];
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({ authority, instructions: instructions.map((j) => JSON.parse(j)) });
        return {
          signed_transaction: Buffer.from([0x12]),
          hash: Buffer.alloc(32, 0x24),
        };
      },
    },
    () =>
      buildBurnAssetTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        assetId: ASSET_ID_INPUT,
        quantity: "4",
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ authority, instructions }] = captures;
  assert.equal(authority, AUTHORITY_ID);
  assert.equal(instructions.length, 1);
  assert.deepEqual(instructions[0], {
    Burn: { Asset: { destination: ASSET_ID, object: "4" } },
  });
});

test("buildBurnTriggerTransaction yields single trigger burn instruction", () => {
  const captures = [];
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({ authority, instructions: instructions.map((j) => JSON.parse(j)) });
        return {
          signed_transaction: Buffer.from([0x21]),
          hash: Buffer.alloc(32, 0x25),
        };
      },
    },
    () =>
      buildBurnTriggerTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        triggerId: "cleanup-trigger",
        repetitions: 1,
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ authority, instructions }] = captures;
  assert.equal(authority, AUTHORITY_ID);
  assert.equal(instructions.length, 1);
  assert.deepEqual(instructions[0], {
    Burn: { TriggerRepetitions: { destination: "cleanup-trigger", object: 1 } },
  });
});

test("buildMintAndTransferTransaction supports transfer arrays", () => {
  const captures = [];
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({ authority, instructions: instructions.map((j) => JSON.parse(j)) });
        return {
          signed_transaction: Buffer.from([0x11]),
          hash: Buffer.alloc(32, 0x23),
        };
      },
    },
    () =>
      buildMintAndTransferTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        mint: { assetId: ASSET_ID_INPUT, quantity: "9" },
        transfers: [
          { quantity: "5", destinationAccountId: AUTHORITY_ID_INPUT },
          {
            sourceAssetId: `${ASSET_DEFINITION_ID_INPUT}#${NEW_ACCOUNT_ID_INPUT}`,
            quantity: "1",
            destinationAccountId: NEW_ACCOUNT_ID_INPUT,
          },
        ],
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  assert.equal(instructions.length, 3);
  assert.deepEqual(instructions[0], {
    Mint: { Asset: { destination: ASSET_ID, object: "9" } },
  });
  assert.deepEqual(instructions[1], {
    Transfer: {
      Asset: {
        source: ASSET_ID,
        object: "5",
        destination: AUTHORITY_ID,
      },
    },
  });
  assert.deepEqual(instructions[2], {
    Transfer: {
      Asset: {
        source: `${ASSET_DEFINITION_ID}#${NEW_ACCOUNT_ID}`,
        object: "1",
        destination: NEW_ACCOUNT_ID,
      },
    },
  });
});

test("buildMintAndTransferTransaction rejects both transfer and transfers", () => {
  assert.throws(
    () =>
      buildMintAndTransferTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        mint: { assetId: ASSET_ID_INPUT, quantity: "4" },
        transfer: { quantity: "2", destinationAccountId: AUTHORITY_ID_INPUT },
        transfers: [],
        privateKey: PRIVATE_KEY,
      }),
    /provide either transfer or transfers/i,
  );
});

test("buildMintAndTransferTransaction requires at least one transfer", () => {
  assert.throws(
    () =>
      buildMintAndTransferTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        mint: { assetId: ASSET_ID_INPUT, quantity: "4" },
        privateKey: PRIVATE_KEY,
      }),
    /transfer or transfers options are required/i,
  );
});

test("buildRegisterDomainAndMintTransaction expands registration and mint", () => {
  const captures = [];
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({ authority, instructions: instructions.map((j) => JSON.parse(j)) });
        return {
          signed_transaction: Buffer.from([0x02]),
          hash: Buffer.alloc(32, 0x33),
        };
      },
    },
    () =>
      buildRegisterDomainAndMintTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        domain: { domainId: "garden_of_live_flowers", metadata: { key: "value" } },
        mint: { assetId: ASSET_ID_INPUT, quantity: "10" },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  assert.equal(instructions.length, 2);
  assert.deepEqual(instructions[0], {
    Register: {
      Domain: {
        id: "garden_of_live_flowers",
        logo: null,
        metadata: { key: "value" },
      },
    },
  });
  assert.deepEqual(instructions[1], {
    Mint: { Asset: { destination: ASSET_ID, object: "10" } },
  });
});

test("buildRegisterDomainAndMintTransaction supports mint arrays", () => {
  const captures = [];
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({ authority, instructions: instructions.map((j) => JSON.parse(j)) });
        return {
          signed_transaction: Buffer.from([0x0A]),
          hash: Buffer.alloc(32, 0x99),
        };
      },
    },
    () =>
      buildRegisterDomainAndMintTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        domain: { domainId: "garden_of_live_flowers" },
        mints: [
          { assetId: ASSET_ID_INPUT, quantity: "3" },
          { assetId: `${ASSET_DEFINITION_ID_INPUT}#${AUTHORITY_ID_INPUT}`, quantity: "1" },
        ],
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  assert.equal(instructions.length, 3);
  assert.deepEqual(instructions[0].Register.Domain.id, "garden_of_live_flowers");
  assert.deepEqual(instructions[1], {
    Mint: { Asset: { destination: ASSET_ID, object: "3" } },
  });
  assert.deepEqual(instructions[2], {
    Mint: {
      Asset: {
        destination: `${ASSET_DEFINITION_ID}#${AUTHORITY_ID}`,
        object: "1",
      },
    },
  });
});

test("buildRegisterDomainAndMintTransaction rejects both mint and mints", () => {
  assert.throws(
    () =>
      buildRegisterDomainAndMintTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        domain: { domainId: "garden_of_live_flowers" },
        mint: { assetId: ASSET_ID_INPUT, quantity: "2" },
        mints: [],
        privateKey: PRIVATE_KEY,
      }),
    /provide either mint or mints/i,
  );
});

test("buildRegisterDomainAndMintTransaction enforces assetId in mints", () => {
  assert.throws(
    () =>
      buildRegisterDomainAndMintTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        domain: { domainId: "garden_of_live_flowers" },
        mints: [{ quantity: "2" }],
        privateKey: PRIVATE_KEY,
      }),
    /mints\[0\]\.assetId must be a non-empty string/i,
  );
});

test("buildRegisterAccountAndTransferTransaction expands registration and transfer", () => {
  const captures = [];
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({ authority, instructions: instructions.map((j) => JSON.parse(j)) });
        return {
          signed_transaction: Buffer.from([0x03]),
          hash: Buffer.alloc(32, 0x44),
        };
      },
    },
    () =>
      buildRegisterAccountAndTransferTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        account: { accountId: NEW_ACCOUNT_ID_INPUT, metadata: { nickname: "alice" } },
        transfer: {
          sourceAssetId: ASSET_ID,
          quantity: "4",
          destinationAccountId: NEW_ACCOUNT_ID_INPUT,
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  assert.equal(instructions.length, 2);
  assert.deepEqual(instructions[0], {
    Register: {
      Account: {
        id: NEW_ACCOUNT_ID,
        metadata: { nickname: "alice" },
      },
    },
  });
  assert.deepEqual(instructions[1], {
    Transfer: {
      Asset: {
        source: ASSET_ID,
        object: "4",
        destination: NEW_ACCOUNT_ID,
      },
    },
  });
});

test("buildRegisterAccountAndTransferTransaction supports transfer arrays", () => {
  const captures = [];
  const secondAccountId =
    "ED0120935DC855E1977DB7CF24E7C4E0015CBAC9D4A2DDB814C3F23A4A032B11D8EBFD@wonderland";
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({ authority, instructions: instructions.map((j) => JSON.parse(j)) });
        return {
          signed_transaction: Buffer.from([0x07]),
          hash: Buffer.alloc(32, 0x88),
        };
      },
    },
    () =>
      buildRegisterAccountAndTransferTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        account: { accountId: NEW_ACCOUNT_ID_INPUT },
        transfers: [
          { sourceAssetId: ASSET_ID, quantity: "2", destinationAccountId: NEW_ACCOUNT_ID_INPUT },
          { sourceAssetId: ASSET_ID, quantity: "1", destinationAccountId: secondAccountId },
        ],
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  assert.equal(instructions.length, 3);
  assert.deepEqual(instructions[0].Register.Account.id, NEW_ACCOUNT_ID);
  assert.deepEqual(instructions[1], {
    Transfer: {
      Asset: {
        source: ASSET_ID,
        object: "2",
        destination: NEW_ACCOUNT_ID,
      },
    },
  });
  assert.deepEqual(instructions[2], {
    Transfer: {
      Asset: {
        source: ASSET_ID,
        object: "1",
        destination: secondAccountId,
      },
    },
  });
});

test("buildRegisterAccountAndTransferTransaction rejects both transfer and transfers", () => {
  assert.throws(
    () =>
      buildRegisterAccountAndTransferTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        account: { accountId: NEW_ACCOUNT_ID_INPUT },
        transfer: { sourceAssetId: ASSET_ID, quantity: "1", destinationAccountId: NEW_ACCOUNT_ID_INPUT },
        transfers: [],
        privateKey: PRIVATE_KEY,
      }),
    /provide either transfer or transfers/i,
  );
});

test("buildRegisterAccountAndTransferTransaction enforces sourceAssetId in transfers", () => {
  assert.throws(
    () =>
      buildRegisterAccountAndTransferTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        account: { accountId: NEW_ACCOUNT_ID_INPUT },
        transfers: [{ quantity: "1", destinationAccountId: NEW_ACCOUNT_ID_INPUT }],
        privateKey: PRIVATE_KEY,
      }),
    /transfers\[0\]\.sourceAssetId is required/i,
  );
});

test("buildRegisterAssetDefinitionAndMintTransaction expands definition and mint", () => {
  const captures = [];
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({ authority, instructions: instructions.map((j) => JSON.parse(j)) });
        return {
          signed_transaction: Buffer.from([0x04]),
          hash: Buffer.alloc(32, 0x55),
        };
      },
    },
    () =>
      buildRegisterAssetDefinitionAndMintTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        assetDefinition: {
          assetDefinitionId: ASSET_DEFINITION_ID,
          metadata: { description: "Rose asset" },
          mintable: "Not",
          spec: { scale: 4 },
          confidentialPolicy: {
            vk_set_hash: "deadbeef",
            pending_transition: { stage: "Queued" },
          },
        },
        mint: {
          accountId: NEW_ACCOUNT_ID_INPUT,
          assetId: `${ASSET_DEFINITION_ID_INPUT}#${NEW_ACCOUNT_ID_INPUT}`,
          quantity: "9",
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  assert.equal(instructions.length, 2);
  assert.deepEqual(instructions[0], {
    Register: {
      AssetDefinition: {
        id: ASSET_DEFINITION_ID,
        logo: null,
        metadata: { description: "Rose asset" },
        mintable: "Not",
        spec: { scale: 4 },
        confidential_policy: {
          mode: "TransparentOnly",
          vk_set_hash: "deadbeef",
          poseidon_params_id: null,
          pedersen_params_id: null,
          pending_transition: { stage: "Queued" },
        },
      },
    },
  });
  assert.deepEqual(instructions[1], {
    Mint: {
      Asset: {
        object: "9",
        destination: `${ASSET_DEFINITION_ID}#${NEW_ACCOUNT_ID}`,
      },
    },
  });
});

test("buildRegisterAssetDefinitionAndMintTransaction supports mint arrays", () => {
  const captures = [];
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({ authority, instructions: instructions.map((j) => JSON.parse(j)) });
        return {
          signed_transaction: Buffer.from([0x08]),
          hash: Buffer.alloc(32, 0x77),
        };
      },
    },
    () =>
      buildRegisterAssetDefinitionAndMintTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        assetDefinition: { assetDefinitionId: ASSET_DEFINITION_ID },
        mints: [
          { accountId: NEW_ACCOUNT_ID_INPUT, quantity: "4" },
          { assetId: `${ASSET_DEFINITION_ID_INPUT}#${AUTHORITY_ID_INPUT}`, quantity: "2" },
        ],
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  assert.equal(instructions.length, 3);
  assert.deepEqual(instructions[1], {
    Mint: {
      Asset: {
        object: "4",
        destination: `${ASSET_DEFINITION_ID}#${NEW_ACCOUNT_ID}`,
      },
    },
  });
  assert.deepEqual(instructions[2], {
    Mint: {
      Asset: {
        object: "2",
        destination: `${ASSET_DEFINITION_ID}#${AUTHORITY_ID}`,
      },
    },
  });
});

test("buildRegisterAssetDefinitionAndMintTransaction rejects both mint and mints", () => {
  assert.throws(
    () =>
      buildRegisterAssetDefinitionAndMintTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        assetDefinition: { assetDefinitionId: ASSET_DEFINITION_ID },
        mint: { accountId: NEW_ACCOUNT_ID_INPUT, quantity: "1" },
        mints: [],
        privateKey: PRIVATE_KEY,
      }),
    /provide either mint or mints/i,
  );
});

test("buildRegisterAssetDefinitionAndMintTransaction enforces mint destination fields", () => {
  assert.throws(
    () =>
      buildRegisterAssetDefinitionAndMintTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        assetDefinition: { assetDefinitionId: ASSET_DEFINITION_ID },
        mints: [{ quantity: "1" }],
        privateKey: PRIVATE_KEY,
      }),
    /mints\[0\].*assetId.*accountId/i,
  );
});

test("buildRegisterAssetDefinitionAndMintTransaction rejects mismatched assetId/accountId", () => {
  assert.throws(
    () =>
      buildRegisterAssetDefinitionAndMintTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        assetDefinition: { assetDefinitionId: ASSET_DEFINITION_ID },
        mints: [
          {
            accountId: NEW_ACCOUNT_ID_INPUT,
            assetId: `${ASSET_DEFINITION_ID}#someone_else`,
            quantity: "1",
          },
        ],
        privateKey: PRIVATE_KEY,
      }),
    /must match/,
  );
});

test("buildRegisterAssetDefinitionMintAndTransferTransaction expands definition, mint, and transfer", () => {
  const captures = [];
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({ authority, instructions: instructions.map((j) => JSON.parse(j)) });
        return {
          signed_transaction: Buffer.from([0x05]),
          hash: Buffer.alloc(32, 0x66),
        };
      },
    },
    () =>
      buildRegisterAssetDefinitionMintAndTransferTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        assetDefinition: { assetDefinitionId: ASSET_DEFINITION_ID },
        mint: { accountId: NEW_ACCOUNT_ID_INPUT, quantity: "5" },
        transfer: { destinationAccountId: AUTHORITY_ID_INPUT, quantity: "2" },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  assert.equal(instructions.length, 3);
  assert.deepEqual(instructions[0].Register.AssetDefinition.id, ASSET_DEFINITION_ID);
  assert.deepEqual(instructions[1], {
    Mint: {
      Asset: {
        destination: `${ASSET_DEFINITION_ID}#${NEW_ACCOUNT_ID}`,
        object: "5",
      },
    },
  });
  assert.deepEqual(instructions[2], {
    Transfer: {
      Asset: {
        source: `${ASSET_DEFINITION_ID}#${NEW_ACCOUNT_ID}`,
        object: "2",
        destination: AUTHORITY_ID,
      },
    },
  });
});

test("buildRegisterAssetDefinitionMintAndTransferTransaction supports transfer arrays", () => {
  const captures = [];
  const secondAccountIdRaw =
    "ED0120A4353E54CDB155483A4A52DEA5FE335DBA74DEFF0E977B5D5F2C3960F4660E21@wonderland";
  const secondAccountId =
    "34mSYnDgbaJM58rbLoif4Tkp7G6sNmuWGM7kusVkmxrHraDMMBo8YhaHY9QEsJcW752nFYoVi@wonderland";
  const secondAccountIdInput = secondAccountIdRaw.toLowerCase();
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({ authority, instructions: instructions.map((j) => JSON.parse(j)) });
        return {
          signed_transaction: Buffer.from([0x06]),
          hash: Buffer.alloc(32, 0x77),
        };
      },
    },
    () =>
      buildRegisterAssetDefinitionMintAndTransferTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        assetDefinition: { assetDefinitionId: ASSET_DEFINITION_ID },
        mints: [
          { accountId: NEW_ACCOUNT_ID_INPUT, quantity: "6" },
          { assetId: `${ASSET_DEFINITION_ID_INPUT}#${AUTHORITY_ID_INPUT}`, quantity: "1" },
        ],
        transfers: [
          { quantity: "4", destinationAccountId: AUTHORITY_ID_INPUT },
          {
            sourceAssetId: `${ASSET_DEFINITION_ID_INPUT}#${AUTHORITY_ID_INPUT}`,
            destinationAccountId: secondAccountIdInput,
            quantity: "1",
          },
        ],
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  assert.equal(instructions.length, 5);
  assert.deepEqual(instructions[3], {
    Transfer: {
      Asset: {
        source: `${ASSET_DEFINITION_ID}#${NEW_ACCOUNT_ID}`,
        object: "4",
        destination: AUTHORITY_ID,
      },
    },
  });
  assert.deepEqual(instructions[4], {
    Transfer: {
      Asset: {
        source: `${ASSET_DEFINITION_ID}#${AUTHORITY_ID}`,
        object: "1",
        destination: secondAccountId,
      },
    },
  });
});

test("buildRegisterAssetDefinitionMintAndTransferTransaction rejects both transfer and transfers", () => {
  assert.throws(
    () =>
      buildRegisterAssetDefinitionMintAndTransferTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        assetDefinition: { assetDefinitionId: ASSET_DEFINITION_ID },
        mint: { accountId: NEW_ACCOUNT_ID_INPUT, quantity: "3" },
        transfer: { destinationAccountId: AUTHORITY_ID_INPUT, quantity: "2" },
        transfers: [],
        privateKey: PRIVATE_KEY,
      }),
    /provide either transfer or transfers/i,
  );
});

test("buildRegisterAssetDefinitionMintAndTransferTransaction rejects both mint and mints", () => {
  assert.throws(
    () =>
      buildRegisterAssetDefinitionMintAndTransferTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        assetDefinition: { assetDefinitionId: ASSET_DEFINITION_ID },
        mint: { accountId: NEW_ACCOUNT_ID_INPUT, quantity: "2" },
        mints: [],
        transfers: [{ quantity: "1", destinationAccountId: AUTHORITY_ID_INPUT }],
        privateKey: PRIVATE_KEY,
      }),
    /provide either mint or mints/i,
  );
});

test("buildRegisterAssetDefinitionMintAndTransferTransaction requires mint spec", () => {
  assert.throws(
    () =>
      buildRegisterAssetDefinitionMintAndTransferTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        assetDefinition: { assetDefinitionId: ASSET_DEFINITION_ID },
        transfers: [{ quantity: "1", destinationAccountId: AUTHORITY_ID_INPUT }],
        privateKey: PRIVATE_KEY,
      }),
    /mint or mints parameters are required/i,
  );
});

test("buildRegisterAssetDefinitionMintAndTransferTransaction validates mint destination", () => {
  assert.throws(
    () =>
      buildRegisterAssetDefinitionMintAndTransferTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        assetDefinition: { assetDefinitionId: ASSET_DEFINITION_ID },
        mints: [{ quantity: "1" }],
        transfers: [{ quantity: "1", destinationAccountId: AUTHORITY_ID_INPUT }],
        privateKey: PRIVATE_KEY,
      }),
    /mints\[0\].*assetId.*accountId/i,
  );
});

test("buildRegisterAssetDefinitionMintAndTransferTransaction rejects mismatched assetId/accountId", () => {
  assert.throws(
    () =>
      buildRegisterAssetDefinitionMintAndTransferTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        assetDefinition: { assetDefinitionId: ASSET_DEFINITION_ID },
        mints: [
          {
            accountId: NEW_ACCOUNT_ID_INPUT,
            assetId: `${ASSET_DEFINITION_ID}#someone_else`,
            quantity: "1",
          },
        ],
        transfers: [{ quantity: "1", destinationAccountId: AUTHORITY_ID_INPUT }],
        privateKey: PRIVATE_KEY,
      }),
    /must match/,
  );
});

function withNativeBinding(binding, fn) {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = binding;
  try {
    return fn();
  } finally {
    globalThis.__IROHA_NATIVE_BINDING__ = previous;
  }
}

function createResponse({ status, jsonData = {}, arrayData, textBody, headers }) {
  return {
    status,
    json: async () => jsonData,
    arrayBuffer: async () => {
      if (arrayData instanceof ArrayBuffer) {
        return arrayData;
      }
      if (ArrayBuffer.isView(arrayData)) {
        return arrayData.buffer.slice(
          arrayData.byteOffset,
          arrayData.byteOffset + arrayData.byteLength,
        );
      }
      return new TextEncoder().encode(textBody ?? JSON.stringify(jsonData ?? {})).buffer;
    },
    text: async () =>
      typeof textBody === "string" ? textBody : JSON.stringify(jsonData ?? {}),
    headers: {
      get(name) {
        if (!headers) {
          return null;
        }
        const normalized = name.toLowerCase();
        for (const [key, value] of Object.entries(headers)) {
          if (key.toLowerCase() === normalized) {
            return value;
          }
        }
        return null;
      },
    },
  };
}
