import { test as baseTest } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import {
  buildRegisterDomainTransaction,
  buildExecutableBatchTransaction,
  buildExecutableBatchTransactionPayload,
  buildTransaction,
  buildTransactionPayload,
  signQuotedTransactionPayload,
  quoteAndSignTransaction,
  buildRegisterPinManifestInstruction,
  buildRegisterPinManifestTransaction,
  buildIvmProvedTransaction,
  buildIvmProvedTransactionPayload,
  signQuotedIvmProvedTransactionPayload,
  submitIvmProvedContractCall,
  buildConfidentialTransferProofV2,
  buildConfidentialUnshieldProofV2,
  buildConfidentialUnshieldProofV3,
  buildApplySccpRouteGovernanceInstruction,
  buildApplySccpRouteGovernanceTransaction,
  buildMintAssetTransaction,
  buildMintAndTransferTransaction,
  buildRegisterDomainAndMintTransaction,
  buildRegisterAssetDefinitionAndMintTransaction,
  buildRegisterAssetDefinitionMintAndTransferTransaction,
  buildTransferAssetTransaction,
  buildRegisterRwaTransaction,
  buildTransferRwaTransaction,
  buildSetRwaKeyValueTransaction,
  buildRemoveRwaKeyValueTransaction,
  buildCreateKaigiTransaction,
  buildJoinKaigiTransaction,
  buildRegisterKaigiRelayTransaction,
  buildRegisterSmartContractCodeTransaction,
  buildRegisterSmartContractBytesTransaction,
  buildRemoveSmartContractBytesTransaction,
  buildProposeDeployContractTransaction,
  buildCastZkBallotTransaction,
  buildCastPlainBallotTransaction,
  buildEnactReferendumTransaction,
  buildFinalizeReferendumTransaction,
  buildPersistCouncilForEpochTransaction,
  buildRegisterZkAssetTransaction,
  buildScheduleConfidentialPolicyTransitionTransaction,
  buildCancelConfidentialPolicyTransitionTransaction,
  buildCreateElectionTransaction,
  buildSubmitBallotTransaction,
  buildFinalizeElectionTransaction,
  hashSignedTransaction,
  hashSignedTransactionPayload,
  hashInstructionBatch,
  feePaymentIntentToNoritoJson,
} from "../src/transaction.js";
import * as transactionExports from "../src/transaction.js";
import {
  buildBurnAssetInstruction,
  buildMintAssetInstruction,
  buildRegisterDomainInstruction,
  buildSetAccountKeyValueInstruction,
  buildTransferAssetInstruction,
} from "../src/instructionBuilders.js";
import { AccountAddress } from "../src/address.js";
import { ToriiClient } from "../src/toriiClient.js";
import { NetworkId } from "../src/networkId.js";
import {
  computeIvmArtifactHashes,
  IVM_ARTIFACT_MAX_BYTES,
} from "../src/ivmArtifact.js";
import { makeNativeTest } from "./helpers/native.js";

const AUTHORITY_PUBLIC_KEY_HEX =
  "CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
const AUTHORITY_ID = i105FromEd25519PublicKeyHex(AUTHORITY_PUBLIC_KEY_HEX);
const AUTHORITY_ID_INPUT = i105FromEd25519PublicKeyHex(
  AUTHORITY_PUBLIC_KEY_HEX,
);
const PRIVATE_KEY = Buffer.from(
  "CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53",
  "hex",
);
const NETWORK_ID = NetworkId.parse(
  "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
);
const NETWORK_ID_BYTES = Buffer.from(NETWORK_ID.toBytes());
const AUTHORITY_FEE_PAYMENT = Object.freeze({
  payer: "authority",
  chargeLimits: Object.freeze([]),
});
const IVM_AUTHORITY_FEE_PAYMENT = Object.freeze({
  payer: "authority",
  chargeLimits: Object.freeze([]),
  gasLimit: 1_000,
});
const ZK_IVM_BYTECODE_BASE64 = Buffer.from([
  0x49, 0x56, 0x4d, 0x00,
  0x01, 0x01, 0x01, 0x00,
  0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
  0x01,
  ...Array(32).fill(0),
]).toString("base64");
const {
  codeHashHex: ZK_IVM_CODE_HASH_HEX,
  artifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
} = computeIvmArtifactHashes(Buffer.from(ZK_IVM_BYTECODE_BASE64, "base64"));
const RELAY_PUBLIC_KEY_HEX =
  "641297079357229F295938A4B5A333DE35069BF47B9D0704E45805713D13C201";
const RELAY_ACCOUNT_ID = i105FromEd25519PublicKeyHex(RELAY_PUBLIC_KEY_HEX);
const RELAY_ACCOUNT_ID_INPUT =
  i105FromEd25519PublicKeyHex(RELAY_PUBLIC_KEY_HEX);
const ASSET_DEFINITION_ID = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
const LILY_ASSET_DEFINITION_ID = "61CtjvNd9T3THAR65GsMVHr82Bjc";
const CANONICAL_ASSET_ID_INPUT = `${ASSET_DEFINITION_ID}#${AUTHORITY_ID}`;
const CANONICAL_LILY_ASSET_ID_INPUT = `${LILY_ASSET_DEFINITION_ID}#${AUTHORITY_ID}`;
const SECOND_CANONICAL_ASSET_ID_INPUT = `${ASSET_DEFINITION_ID}#${RELAY_ACCOUNT_ID}`;
const ASSET_ID = CANONICAL_ASSET_ID_INPUT;
const ASSET_ID_INPUT = CANONICAL_ASSET_ID_INPUT;
const RWA_ID =
  "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef$commodities.sora";
const test = makeNativeTest(baseTest);

function i105FromEd25519PublicKeyHex(publicKeyHex) {
  const publicKey = Buffer.from(publicKeyHex.trim(), "hex");
  return AccountAddress.fromAccount({ publicKey }).toI105();
}

function encodeAssetIdForKnownAccount(assetDefinitionId, accountId) {
  assert.equal(assetDefinitionId, ASSET_DEFINITION_ID);
  if (accountId === AUTHORITY_ID || accountId === AUTHORITY_ID_INPUT) {
    return CANONICAL_ASSET_ID_INPUT;
  }
  throw new Error(
    `unexpected account id for test asset encoding: ${accountId}`,
  );
}

function crc16(tag, body) {
  let crc = 0xffff;
  const processByte = (byte) => {
    crc ^= (byte & 0xff) << 8;
    for (let i = 0; i < 8; i += 1) {
      if ((crc & 0x8000) !== 0) {
        crc = ((crc << 1) ^ 0x1021) & 0xffff;
      } else {
        crc = (crc << 1) & 0xffff;
      }
    }
  };

  for (const byte of Buffer.from(tag, "utf8")) {
    processByte(byte);
  }
  processByte(":".charCodeAt(0));
  for (const byte of Buffer.from(body, "utf8")) {
    processByte(byte);
  }
  return crc & 0xffff;
}

function normalizedHashHex(bytes) {
  const buffer = Buffer.from(bytes);
  if (buffer.length !== 32) {
    throw new TypeError("hash literal test helper requires 32 bytes");
  }
  buffer[buffer.length - 1] |= 1;
  const body = buffer.toString("hex").toUpperCase();
  const checksum = crc16("hash", body)
    .toString(16)
    .toUpperCase()
    .padStart(4, "0");
  return `hash:${body}#${checksum}`;
}

function hex32(byte) {
  return `0x${Buffer.alloc(32, byte).toString("hex")}`;
}

function mutateFirstSignedTransactionSignatureByte(signedTransaction) {
  const bytes = Buffer.from(signedTransaction);
  const bareOffset = bytes[0] === 1 ? 1 : 0;

  const readCompactLength = (offset) => {
    let value = 0;
    let shift = 0;
    for (let index = 0; index < 10; index += 1) {
      const byte = bytes[offset + index];
      assert.notEqual(byte, undefined, "compact length must not be truncated");
      value += (byte & 0x7f) * 2 ** shift;
      if ((byte & 0x80) === 0) {
        assert.ok(Number.isSafeInteger(value), "compact length must be safe");
        return { next: offset + index + 1, value };
      }
      shift += 7;
    }
    throw new Error("compact length must terminate within 10 bytes");
  };

  const signatureField = readCompactLength(bareOffset);
  const signaturePayload = readCompactLength(signatureField.next);
  assert.ok(
    signaturePayload.next + signaturePayload.value <= bytes.length,
    "signature payload must fit in the transaction",
  );
  assert.equal(
    bytes.readBigUInt64LE(signaturePayload.next),
    64n,
    "fixture must contain one 64-byte Ed25519 signature",
  );
  const firstSignatureByte = readCompactLength(signaturePayload.next + 8);
  assert.equal(firstSignatureByte.value, 1, "signature byte field must contain one byte");
  bytes[firstSignatureByte.next] ^= 0x80;
  return bytes;
}

function buildSampleSccpRemoveAction() {
  return {
    action: "Remove",
    route: {
      lane_id: {
        source: { network: "bsc_testnet", profile: null },
        target: { network: "sora_taira", profile: null },
      },
      route_id: "taira_bsc_xor",
      asset_key: "xor",
      revision: 1,
    },
  };
}

function toByteArray(bytes) {
  return Array.from(Buffer.from(bytes));
}

function buildSampleRegisterDomain(additionalOptions = {}) {
  return buildRegisterDomainTransaction({
    networkId: NETWORK_ID,
    authority: AUTHORITY_ID_INPUT,
    feePayment: AUTHORITY_FEE_PAYMENT,
    domainId: "garden_of_live_flowers.sora",
    metadata: { key: "value" },
    creationTimeMs: 1_700_000_000_000,
    ttlMs: 5_000,
    nonce: 42,
    privateKey: PRIVATE_KEY,
    ...additionalOptions,
  });
}

test("buildRegisterDomainTransaction returns canonical hash", () => {
  const built = buildSampleRegisterDomain();
  assert.ok(Buffer.isBuffer(built.signedTransaction));
  assert.ok(Buffer.isBuffer(built.hash));
  assert.equal(built.hash.length, 32);

  const recomputed = hashSignedTransaction(built.signedTransaction, {
    encoding: "buffer",
  });
  assert.deepEqual(recomputed, built.hash);
});

test("hashSignedTransactionPayload returns the detached scaffold preimage", () => {
  const first = buildSampleRegisterDomain();
  const signatureMutated = mutateFirstSignedTransactionSignatureByte(
    first.signedTransaction,
  );
  const firstPayloadHash = hashSignedTransactionPayload(
    first.signedTransaction,
    { encoding: "buffer" },
  );
  const secondPayloadHash = hashSignedTransactionPayload(
    signatureMutated,
    { encoding: "buffer" },
  );

  assert.equal(firstPayloadHash.length, 32);
  assert.deepEqual(firstPayloadHash, secondPayloadHash);
  assert.notDeepEqual(
    hashSignedTransaction(first.signedTransaction, { encoding: "buffer" }),
    hashSignedTransaction(signatureMutated, { encoding: "buffer" }),
  );
});

test("hashInstructionBatch binds a settlement batch to its source marker", () => {
  const transfer = buildTransferAssetInstruction({
    sourceAssetHoldingId: CANONICAL_ASSET_ID_INPUT,
    quantity: "2800",
    destinationAccountId: RELAY_ACCOUNT_ID_INPUT,
  });
  const batchFor = (sourceTxHash) => [
    buildSetAccountKeyValueInstruction({
      accountId: AUTHORITY_ID_INPUT,
      key: `pk_cbuae_settlement_${sourceTxHash}`,
      value: {
        protocol: "pk-cbuae-settlement",
        version: 1,
        source_tx_hash: sourceTxHash,
      },
    }),
    transfer,
  ];
  const first = hashInstructionBatch(batchFor("a".repeat(64)), {
    encoding: "buffer",
  });
  const second = hashInstructionBatch(batchFor("b".repeat(64)), {
    encoding: "buffer",
  });

  assert.equal(first.length, 32);
  assert.equal(second.length, 32);
  assert.notDeepEqual(first, second);
});

test("buildRegisterDomainTransaction accepts metadata JSON strings", () => {
  const built = buildSampleRegisterDomain({
    metadata: JSON.stringify({ foo: "bar" }),
  });
  const recomputed = hashSignedTransaction(built.signedTransaction, {
    encoding: "buffer",
  });
  assert.deepEqual(recomputed, built.hash);
});

test("buildTransaction normalizes instruction objects", () => {
  const instruction = buildMintAssetInstruction({
    assetId: ASSET_ID_INPUT,
    quantity: "2",
  });
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x01, 0x02]),
    hash: Buffer.alloc(32, 0xaa),
  };

  withNativeBinding(
    {
      buildTransaction: (
        networkId,
        authority,
        instructions,
        feePaymentJson,
        metadataPayload,
        creationTimeMs,
        ttlMs,
        nonce,
        secret,
        privateKeyAlgorithm,
      ) => {
        captures.push({
          networkId,
          authority,
          instructions,
          feePaymentJson,
          metadataPayload,
          creationTimeMs,
          ttlMs,
          nonce,
          secret,
          privateKeyAlgorithm,
        });
        return fakeResult;
      },
    },
    () => {
      const built = buildTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        instructions: [instruction],
        metadata: { tag: "value" },
        creationTimeMs: 10,
        ttlMs: 20,
        nonce: 5,
        privateKey: PRIVATE_KEY,
        privateKeyAlgorithm: "secp256k1",
      });
      assert.deepEqual(
        built.signedTransaction,
        Buffer.from(fakeResult.signed_transaction),
      );
      assert.deepEqual(built.hash, Buffer.from(fakeResult.hash));
    },
  );

  assert.equal(captures.length, 1);
  const call = captures[0];
  assert.deepEqual(Buffer.from(call.networkId), NETWORK_ID_BYTES);
  assert.equal(call.authority, AUTHORITY_ID);
  assert.deepEqual(call.instructions, [JSON.stringify(instruction)]);
  assert.deepEqual(JSON.parse(call.feePaymentJson), {
    payer: "authority",
    value: { charge_limits: [], gas_limit: null },
  });
  assert.equal(call.metadataPayload, JSON.stringify({ tag: "value" }));
  assert.equal(call.creationTimeMs, 10);
  assert.equal(call.ttlMs, 20);
  assert.equal(call.nonce, 5);
  assert.equal(call.privateKeyAlgorithm, "secp256k1");
});

test("mixed executable batch builder forwards ordered copied entries", () => {
  const instruction = buildMintAssetInstruction({
    assetId: ASSET_ID_INPUT,
    quantity: "2",
  });
  const expectedCodeHash = Buffer.alloc(32, 0x41);
  const argumentsBytes = Uint8Array.from([0x4b, 0x4f, 0x54, 0x4f]);
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x04, 0x01]),
    hash: Buffer.alloc(32, 0xab),
  };
  withNativeBinding(
    {
      buildExecutableBatchTransaction: (...args) => {
        captures.push(args);
        return fakeResult;
      },
    },
    () => {
      const result = buildExecutableBatchTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        entries: [
          { kind: "instruction", instruction },
          {
            kind: "contractCall",
            contractAddress:
              "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh",
            expectedCodeHash,
            entrypoint: "run",
            arguments: argumentsBytes,
          },
          { kind: "instruction", instruction },
        ],
        feePayment: IVM_AUTHORITY_FEE_PAYMENT,
        privateKey: PRIVATE_KEY,
      });
      assert.deepEqual(result.signedTransaction, fakeResult.signed_transaction);
      assert.deepEqual(result.hash, fakeResult.hash);
    },
  );
  expectedCodeHash.fill(0);
  argumentsBytes.fill(0);

  assert.equal(captures.length, 1);
  const entries = captures[0][2].map((entry) => JSON.parse(entry));
  assert.deepEqual(entries.map(({ kind }) => kind), [
    "instruction",
    "contractCall",
    "instruction",
  ]);
  assert.equal(entries[1].expectedCodeHash, "41".repeat(32).toUpperCase());
  assert.deepEqual(entries[1].arguments, [0x4b, 0x4f, 0x54, 0x4f]);
  assert.equal(JSON.parse(captures[0][3]).value.gas_limit, 1000);
});

test("mixed executable batch draft and validation reject missing requirements", () => {
  const call = {
    kind: "contractCall",
    contractAddress:
      "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh",
    expectedCodeHash: Buffer.alloc(32, 0x41),
    entrypoint: "run",
  };
  withNativeBinding(
    {
      buildExecutableBatchTransactionPayload: (...args) => ({
        payload_json: JSON.stringify({ instructions: { Batch: [] } }),
        payload_bytes: Buffer.from([4]),
        payload_hash: Buffer.alloc(32, 5),
        args,
      }),
    },
    () => {
      assert.throws(
        () =>
          buildExecutableBatchTransactionPayload({
            networkId: NETWORK_ID,
            authority: AUTHORITY_ID_INPUT,
            entries: [call],
            feePayment: AUTHORITY_FEE_PAYMENT,
          }),
        /gasLimit is required/u,
      );
      assert.throws(
        () =>
          buildExecutableBatchTransactionPayload({
            networkId: NETWORK_ID,
            authority: AUTHORITY_ID_INPUT,
            entries: [],
            feePayment: IVM_AUTHORITY_FEE_PAYMENT,
          }),
        /non-empty array/u,
      );
      assert.throws(
        () =>
          buildExecutableBatchTransactionPayload({
            networkId: NETWORK_ID,
            authority: AUTHORITY_ID_INPUT,
            entries: [{ ...call, expectedCodeHash: Buffer.alloc(31) }],
            feePayment: IVM_AUTHORITY_FEE_PAYMENT,
          }),
        /exactly 32 bytes/u,
      );
      for (const contractAddress of [
        "abc",
        call.contractAddress.toUpperCase(),
        `${call.contractAddress.slice(0, -1)}p`,
        "irohac1qyqqqqqqqqqqqzgfpg9scrgwpugpzysnzs23v9ccrydpk8q7ca9ly",
        "irohac1qgqqqqqqqqqqqzgfpg9scrgwpugpzysnzs23v9ccrydpk8qhk43nl",
      ]) {
        assert.throws(
          () =>
            buildExecutableBatchTransactionPayload({
              networkId: NETWORK_ID,
              authority: AUTHORITY_ID_INPUT,
              entries: [{ ...call, contractAddress }],
              feePayment: IVM_AUTHORITY_FEE_PAYMENT,
            }),
          /contractAddress/u,
        );
      }
    },
  );
});

test("quote-to-sign helpers preserve the exact unsigned payload", () => {
  const instruction = buildMintAssetInstruction({
    assetId: ASSET_ID_INPUT,
    quantity: "2",
  });
  const payload = {
    domain: { kind: "network", value: NETWORK_ID.literal },
    authority: AUTHORITY_ID,
    creation_time_ms: 10,
    instructions: { Instructions: [instruction] },
    time_to_live_ms: 20,
    nonce: 5,
    fee_payment: {
      payer: "authority",
      value: { charge_limits: [], gas_limit: null },
    },
    metadata: {},
  };
  const payloadJson = JSON.stringify(payload);
  const draftCaptures = [];
  const signCaptures = [];
  withNativeBinding(
    {
      buildTransactionPayload: (...args) => {
        draftCaptures.push(args);
        return {
          payload_json: payloadJson,
          payload_bytes: Buffer.from([0x10, 0x11]),
          payload_hash: Buffer.alloc(32, 0x12),
        };
      },
      signQuotedTransactionPayload: (...args) => {
        signCaptures.push(args);
        return {
          signed_transaction: Buffer.from([0x20, 0x21]),
          hash: Buffer.alloc(32, 0x22),
        };
      },
    },
    () => {
      const draft = buildTransactionPayload({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        instructions: [instruction],
        feePayment: AUTHORITY_FEE_PAYMENT,
        metadata: {},
        creationTimeMs: 10,
        ttlMs: 20,
        nonce: 5,
      });
      assert.deepEqual(draft.payload, payload);
      assert.equal(draft.payloadJson, payloadJson);
      const quotedIntent = {
        payer: "authority",
        value: {
          charge_limits: [
            {
              kind: { kind: "nexus", value: null },
              asset_definition_id: ASSET_DEFINITION_ID,
              max_amount: "3",
            },
          ],
          gas_limit: null,
        },
      };
      const signed = signQuotedTransactionPayload({
        networkId: NETWORK_ID,
        payload: draft,
        quotedFeePayment: quotedIntent,
        privateKey: PRIVATE_KEY,
        privateKeyAlgorithm: "ed25519",
      });
      assert.deepEqual(signed.signedTransaction, Buffer.from([0x20, 0x21]));
      assert.deepEqual(signed.hash, Buffer.alloc(32, 0x22));
      assert.deepEqual(Buffer.from(signCaptures[0][0]), NETWORK_ID_BYTES);
      assert.equal(signCaptures[0][1], payloadJson);
      assert.deepEqual(JSON.parse(signCaptures[0][2]), quotedIntent);
    },
  );
  assert.equal(draftCaptures.length, 1);
  assert.deepEqual(Buffer.from(draftCaptures[0][0]), NETWORK_ID_BYTES);
  assert.equal(draftCaptures[0][1], AUTHORITY_ID);
  assert.deepEqual(draftCaptures[0][2], [JSON.stringify(instruction)]);
});

test("quoteAndSignTransaction performs the guided exact-payload flow", async () => {
  const instruction = buildMintAssetInstruction({
    assetId: ASSET_ID_INPUT,
    quantity: "1",
  });
  const payload = {
    domain: { kind: "network", value: NETWORK_ID.literal },
    authority: AUTHORITY_ID,
    fee_payment: {
      payer: "authority",
      value: { charge_limits: [], gas_limit: null },
    },
  };
  const quotedIntent = {
    payer: "authority",
    value: {
      charge_limits: [
        {
          kind: { kind: "nexus", value: null },
          asset_definition_id: ASSET_DEFINITION_ID,
          max_amount: "1",
        },
      ],
      gas_limit: null,
    },
  };
  const calls = [];
  await withNativeBindingAsync(
    {
      buildTransactionPayload: () => ({
        payload_json: JSON.stringify(payload),
        payload_bytes: Buffer.from([1]),
        payload_hash: Buffer.alloc(32, 2),
      }),
      signQuotedTransactionPayload: (...args) => {
        calls.push(["sign", ...args]);
        return {
          signed_transaction: Buffer.from([3]),
          hash: Buffer.alloc(32, 4),
        };
      },
    },
    async () => {
      const client = {
        async quoteFees(draft, options) {
          calls.push(["quote", draft, options]);
          return { intent: quotedIntent, observation: {}, components: [], capacities: [] };
        },
      };
      const result = await quoteAndSignTransaction(client, {
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        instructions: [instruction],
        feePayment: AUTHORITY_FEE_PAYMENT,
        privateKey: PRIVATE_KEY,
      });
      assert.deepEqual(result.signedTransaction, Buffer.from([3]));
      assert.deepEqual(result.quote.intent, quotedIntent);
    },
  );
  assert.equal(calls[0][0], "quote");
  assert.equal(calls[0][2].canonicalAuth.accountId, AUTHORITY_ID_INPUT);
  assert.equal(calls[1][0], "sign");
  assert.deepEqual(Buffer.from(calls[1][1]), NETWORK_ID_BYTES);
  assert.equal(calls[1][2], JSON.stringify(payload));
  assert.deepEqual(JSON.parse(calls[1][3]), quotedIntent);
});

baseTest("quoted transaction signers require one nominal NetworkId", () => {
  let nativeCalls = 0;
  const binding = {
    signQuotedTransactionPayload() {
      nativeCalls += 1;
      throw new Error("native signer must not run");
    },
    signQuotedIvmProvedTransactionPayload() {
      nativeCalls += 1;
      throw new Error("native signer must not run");
    },
  };
  withNativeBinding(binding, () => {
    for (const signer of [
      (networkInput) =>
        signQuotedTransactionPayload({
          ...networkInput,
          payload: {},
          quotedFeePayment: AUTHORITY_FEE_PAYMENT,
          privateKey: PRIVATE_KEY,
        }),
      (networkInput) =>
        signQuotedIvmProvedTransactionPayload({
          ...networkInput,
          payload: {},
          attachment: {},
          quotedFeePayment: AUTHORITY_FEE_PAYMENT,
          privateKey: PRIVATE_KEY,
        }),
    ]) {
      assert.throws(
        () => signer({ networkId: NETWORK_ID.literal }),
        /input\.networkId must be a NetworkId/u,
      );
      for (const retired of ["chain", "chainId", "chain_id"]) {
        assert.throws(
          () => signer({ networkId: NETWORK_ID, [retired]: "retired" }),
          /is unsupported; provide the nominal networkId field/u,
        );
      }
    }
  });
  assert.equal(nativeCalls, 0);
});

baseTest("buildRegisterPinManifestInstruction binds the canonical pin fields", () => {
  const successor = Buffer.alloc(32, 0x44);
  const instruction = buildRegisterPinManifestInstruction({
    manifestPayload: Buffer.from("manifest"),
    alias: {
      namespace: "docs",
      name: "main",
      proof: Buffer.from("alias-proof"),
    },
    successorOf: successor,
  });

  assert.deepEqual(instruction, {
    RegisterPinManifest: {
      manifest_payload: Buffer.from("manifest").toString("base64"),
      alias: {
        namespace: "docs",
        name: "main",
        proof: Buffer.from("alias-proof").toString("base64"),
      },
      successor_of: [...successor],
    },
  });
  assert.throws(
    () =>
      buildRegisterPinManifestInstruction({
        manifestPayload: Buffer.alloc(0),
      }),
    /manifestPayload must contain/,
  );
  assert.throws(
    () =>
      buildRegisterPinManifestInstruction({
        manifestPayload: Buffer.from("manifest"),
        successorOf: Buffer.alloc(32),
      }),
    /32 non-zero bytes/,
  );
  for (const retiredField of ["submittedEpoch", "submitted_epoch"]) {
    assert.throws(
      () =>
        buildRegisterPinManifestInstruction({
          manifestPayload: Buffer.from("manifest"),
          [retiredField]: 42,
        }),
      /no longer accepts a submitted epoch/,
    );
  }
});

baseTest("buildRegisterPinManifestTransaction rejects a retired submitted epoch", () => {
  assert.throws(
    () =>
      buildRegisterPinManifestTransaction(null, {
        manifestPayload: Buffer.from("manifest"),
        submittedEpoch: 42,
      }),
    /no longer accepts a submitted epoch/,
  );
});

test("buildRegisterPinManifestTransaction quotes and signs exactly one instruction", async () => {
  const draftCalls = [];
  const payload = {
    domain: { kind: "network", value: NETWORK_ID.literal },
    authority: AUTHORITY_ID,
    fee_payment: {
      payer: "authority",
      value: { charge_limits: [], gas_limit: null },
    },
  };
  const quotedIntent = payload.fee_payment;
  await withNativeBindingAsync(
    {
      buildTransactionPayload: (...args) => {
        draftCalls.push(args);
        return {
          payload_json: JSON.stringify(payload),
          payload_bytes: Buffer.from([1]),
          payload_hash: Buffer.alloc(32, 2),
        };
      },
      signQuotedTransactionPayload: () => ({
        signed_transaction: Buffer.from([3]),
        hash: Buffer.alloc(32, 4),
      }),
    },
    async () => {
      const client = {
        async quoteFees() {
          return {
            intent: quotedIntent,
            observation: {},
            components: [],
            capacities: [],
          };
        },
      };
      const result = await buildRegisterPinManifestTransaction(client, {
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        privateKey: PRIVATE_KEY,
        manifestPayload: Buffer.from("manifest"),
      });
      assert.deepEqual(result.signedTransaction, Buffer.from([3]));
    },
  );

  assert.equal(draftCalls.length, 1);
  assert.equal(draftCalls[0][2].length, 1);
  assert.deepEqual(JSON.parse(draftCalls[0][2][0]), {
    RegisterPinManifest: {
      manifest_payload: Buffer.from("manifest").toString("base64"),
      alias: null,
      successor_of: null,
    },
  });

});

test("proved-IVM quote draft preserves the proof attachment through signing", () => {
  const payload = {
    domain: { kind: "network", value: NETWORK_ID.literal },
    authority: AUTHORITY_ID,
    fee_payment: {
      payer: "authority",
      value: { charge_limits: [], gas_limit: 5000 },
    },
  };
  const proved = { bytecode: "TlJUMAAAAA==", overlay: [] };
  const attachment = {
    backend: "halo2/ipa",
    proof: { backend: "halo2/ipa", bytes: [1, 2, 3] },
    vk_ref: { backend: "halo2/ipa", name: "ivm-exec-v1" },
  };
  const calls = [];
  withNativeBinding(
    {
      buildIvmProvedTransactionPayload: (...args) => {
        calls.push(["draft", ...args]);
        return {
          payload_json: JSON.stringify(payload),
          payload_bytes: Buffer.from([0x10]),
          payload_hash: Buffer.alloc(32, 0x11),
        };
      },
      signQuotedIvmProvedTransactionPayload: (...args) => {
        calls.push(["sign", ...args]);
        return {
          signed_transaction: Buffer.from([0x20]),
          hash: Buffer.alloc(32, 0x21),
        };
      },
    },
    () => {
      const draft = buildIvmProvedTransactionPayload({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        proved,
        attachment,
        feePayment: {
          ...AUTHORITY_FEE_PAYMENT,
          gasLimit: 5000,
        },
      });
      const signed = signQuotedIvmProvedTransactionPayload({
        networkId: NETWORK_ID,
        payload: draft,
        quotedFeePayment: payload.fee_payment,
        privateKey: PRIVATE_KEY,
      });
      assert.deepEqual(draft.attachment, attachment);
      assert.equal(draft.attachmentJson, JSON.stringify(attachment));
      assert.deepEqual(signed.signedTransaction, Buffer.from([0x20]));
    },
  );
  assert.equal(calls[0][0], "draft");
  assert.equal(calls[1][0], "sign");
  assert.deepEqual(Buffer.from(calls[1][1]), NETWORK_ID_BYTES);
  assert.equal(calls[1][2], JSON.stringify(payload));
  assert.equal(calls[1][3], JSON.stringify(attachment));
  assert.deepEqual(JSON.parse(calls[1][4]), payload.fee_payment);
});

test("feePaymentIntentToNoritoJson binds exact sponsor revision and limits", () => {
  const programId = `${AUTHORITY_ID_INPUT}/wallet-onboarding`;
  const parsed = JSON.parse(
    feePaymentIntentToNoritoJson({
      payer: "sponsor",
      programId,
      programRevision: "7",
      chargeLimits: [
        {
          kind: "nexus",
          assetDefinitionId: ASSET_DEFINITION_ID,
          maxAmount: "1.25",
        },
      ],
      gasLimit: "5000",
    }),
  );
  assert.deepEqual(parsed, {
    payer: "sponsor",
    value: {
      program_id: {
        sponsor: AUTHORITY_ID_INPUT,
        name: "wallet-onboarding",
      },
      program_revision: 7,
      charge_limits: [
        {
          kind: { kind: "nexus", value: null },
          asset_definition_id: ASSET_DEFINITION_ID,
          max_amount: "1.25",
        },
      ],
      gas_limit: 5000,
    },
  });
});

test("buildTransaction requires an explicit fee payment intent", () => {
  withNativeBinding(
    {
      buildTransaction: () => {
        throw new Error("native builder should not be called");
      },
    },
    () => {
      assert.throws(
        () =>
          buildTransaction({
            networkId: NETWORK_ID,
            authority: AUTHORITY_ID_INPUT,
            instructions: [{ Log: { level: "INFO", message: "hello" } }],
            privateKey: PRIVATE_KEY,
          }),
        /feePayment must be a non-null object/i,
      );
    },
  );
});

test("buildApplySccpRouteGovernanceInstruction wraps one exact closed action", () => {
  const action = buildSampleSccpRemoveAction();
  assert.deepEqual(
    buildApplySccpRouteGovernanceInstruction(action),
    { ApplySccpRouteGovernance: { action } },
  );
});

test("SCCP route governance action rejects aliases and retired manifests", () => {
  for (const action of [
    { ...buildSampleSccpRemoveAction(), manifest: {} },
    { action: "Remove", route: { ...buildSampleSccpRemoveAction().route, routeId: "alias" } },
    { action: "UpsertManifest", route: {} },
  ]) {
    assert.throws(() => buildApplySccpRouteGovernanceInstruction(action));
  }
});

test("SCCP route governance transaction submits one typed atomic action", () => {
  const action = buildSampleSccpRemoveAction();
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x10, 0x20]),
    hash: Buffer.alloc(32, 0xb1),
  };

  withNativeBinding(
    {
      buildTransaction: (
        networkId,
        authority,
        instructions,
        feePaymentJson,
        metadataPayload,
        creationTimeMs,
        ttlMs,
        nonce,
        secret,
      ) => {
        captures.push({
          networkId,
          authority,
          instructions: instructions.map((payload) => JSON.parse(payload)),
          feePaymentJson,
          metadataPayload,
          creationTimeMs,
          ttlMs,
          nonce,
          secret,
        });
        return fakeResult;
      },
    },
    () => {
      const built = buildApplySccpRouteGovernanceTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        action,
        metadata: { op: "apply-sccp-route-governance" },
        creationTimeMs: 1_700_000_000_000,
        ttlMs: 5_000,
        nonce: 7,
        privateKey: PRIVATE_KEY,
      });
      assert.deepEqual(built.hash, Buffer.from(fakeResult.hash));
    },
  );

  assert.equal(captures.length, 1);
  assert.deepEqual(captures[0].instructions, [
    { ApplySccpRouteGovernance: { action } },
  ]);
  assert.equal(JSON.parse(captures[0].feePaymentJson).payer, "authority");
  assert.deepEqual(JSON.parse(captures[0].metadataPayload), {
    op: "apply-sccp-route-governance",
  });
  assert.equal(captures[0].creationTimeMs, 1_700_000_000_000);
  assert.equal(captures[0].ttlMs, 5_000);
  assert.equal(captures[0].nonce, 7);
  assert.equal(captures[0].secret.equals(PRIVATE_KEY), true);
});

test("transaction helper wrappers forward privateKeyAlgorithm", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x03, 0x04]),
    hash: Buffer.alloc(32, 0xbb),
  };

  withNativeBinding(
    {
      buildTransaction: (
        _chainId,
        _authority,
        _instructions,
        _feePaymentJson,
        _metadataPayload,
        _creationTimeMs,
        _ttlMs,
        _nonce,
        _secret,
        privateKeyAlgorithm,
      ) => {
        captures.push(privateKeyAlgorithm);
        return fakeResult;
      },
    },
    () => {
      buildMintAssetTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        assetHoldingId: ASSET_ID_INPUT,
        quantity: "2",
        privateKey: PRIVATE_KEY,
        privateKeyAlgorithm: "secp256k1",
      });
    },
  );

  assert.deepEqual(captures, ["secp256k1"]);
});

test("transaction helper wrappers do not omit privateKeyAlgorithm forwarding", () => {
  const source = readFileSync(
    new URL("../src/transaction.js", import.meta.url),
    "utf8",
  );
  assert.equal(
    /privateKey,\n\s*\}\);/u.test(source),
    false,
    "a buildTransaction call forwards privateKey but not privateKeyAlgorithm",
  );
  assert.equal(
    /privateKey,\n\}\) \{/u.test(source),
    false,
    "a transaction helper accepts privateKey without privateKeyAlgorithm",
  );
});

test("buildTransaction rejects empty instruction arrays", () => {
  assert.throws(
    () =>
      buildTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        instructions: [],
        privateKey: PRIVATE_KEY,
      }),
    /non-empty array/i,
  );
});

 test("buildIvmProvedTransaction normalizes proved executable and attachment", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x03, 0x04]),
    hash: Buffer.alloc(32, 0xbb),
  };
  const proved = {
    bytecode: ZK_IVM_BYTECODE_BASE64,
    overlay: [],
    events_commitment: normalizedHashHex(Buffer.alloc(32, 0x01)),
    gas_policy_commitment: normalizedHashHex(Buffer.alloc(32, 0x02)),
  };
  const attachment = {
    backend: "halo2/ipa",
    proof: {
      backend: "halo2/ipa",
      bytes: [1, 2, 3],
    },
    vk_ref: {
      backend: "halo2/ipa",
      name: "ivm-exec-v1",
    },
  };

  withNativeBinding(
    {
      buildIvmProvedTransaction: (
        networkId,
        authority,
        provedPayload,
        attachmentPayload,
        feePaymentJson,
        metadataPayload,
        creationTimeMs,
        ttlMs,
        nonce,
        secret,
      ) => {
        captures.push({
          networkId,
          authority,
          provedPayload,
          attachmentPayload,
          feePaymentJson,
          metadataPayload,
          creationTimeMs,
          ttlMs,
          nonce,
          secret,
        });
        return fakeResult;
      },
    },
    () => {
      const built = buildIvmProvedTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: IVM_AUTHORITY_FEE_PAYMENT,
        proved,
        attachment,
        metadata: { purpose: "proof-test" },
        creationTimeMs: 10,
        ttlMs: 20,
        nonce: 5,
        privateKey: PRIVATE_KEY,
      });
      assert.deepEqual(
        built.signedTransaction,
        Buffer.from(fakeResult.signed_transaction),
      );
      assert.deepEqual(built.hash, Buffer.from(fakeResult.hash));
    },
  );

  assert.equal(captures.length, 1);
  const call = captures[0];
  assert.deepEqual(Buffer.from(call.networkId), NETWORK_ID_BYTES);
  assert.equal(call.authority, AUTHORITY_ID);
  assert.deepEqual(JSON.parse(call.provedPayload), proved);
  assert.deepEqual(JSON.parse(call.attachmentPayload), attachment);
  assert.equal(JSON.parse(call.feePaymentJson).value.gas_limit, 1_000);
  assert.equal(call.metadataPayload, JSON.stringify({ purpose: "proof-test" }));
  assert.equal(call.creationTimeMs, 10);
  assert.equal(call.ttlMs, 20);
  assert.equal(call.nonce, 5);
});

test("buildIvmProvedTransaction rejects empty proved payload strings", () => {
  withNativeBinding(
    {
      buildIvmProvedTransaction: () => {
        throw new Error("native builder should not be called");
      },
    },
    () => {
      assert.throws(
        () =>
          buildIvmProvedTransaction({
            networkId: NETWORK_ID,
            authority: AUTHORITY_ID_INPUT,
            feePayment: IVM_AUTHORITY_FEE_PAYMENT,
            proved: " ",
            attachment: {},
            privateKey: PRIVATE_KEY,
          }),
        /proved must not be an empty JSON string/i,
      );
    },
  );
});

test("submitIvmProvedContractCall rejects code and proof substitution before signing", async (t) => {
  const validProved = {
    bytecode: ZK_IVM_BYTECODE_BASE64,
    overlay: [],
    events_commitment: normalizedHashHex(Buffer.alloc(32, 0x01)),
    gas_policy_commitment: normalizedHashHex(Buffer.alloc(32, 0x02)),
  };
  const validAttachment = {
    backend: "halo2/ipa",
    proof: { backend: "halo2/ipa", bytes: [1, 2, 3] },
    vk_ref: { backend: "halo2/ipa", name: "ivm-exec-v1" },
  };
  const headerSubstitutedArtifact = Buffer.from(
    ZK_IVM_BYTECODE_BASE64,
    "base64",
  );
  headerSubstitutedArtifact[16] ^= 0x80;
  const headerSubstitutedBytecode = headerSubstitutedArtifact.toString("base64");
  const bodySubstitutedArtifact = Buffer.concat([
    Buffer.from(ZK_IVM_BYTECODE_BASE64, "base64"),
    Buffer.from([0x80]),
  ]);
  const bodySubstitutedBytecode = bodySubstitutedArtifact.toString("base64");
  const bodySubstitutedArtifactSha256Hex = createHash("sha256")
    .update(bodySubstitutedArtifact)
    .digest("hex");

  async function rejectsBeforeSigning({
    input = {},
    simulationCodeHash = ZK_IVM_CODE_HASH_HEX,
    simulationOverrides = {},
    fetchedBytecode = ZK_IVM_BYTECODE_BASE64,
    fetchedCodeResponse,
    derivedProved = validProved,
    attachment = validAttachment,
    options = {},
    onCodeFetch = () => {},
    onProof = () => {},
    expected,
  }) {
    const calls = {
      simulate: 0,
      fetchCode: 0,
      derive: 0,
      prove: 0,
      sign: 0,
      submit: 0,
    };
    const client = new ToriiClient("https://localhost:8080", {
      fetchImpl: async () => {
        throw new Error("network fetch should be replaced by focused stubs");
      },
    });
    client.simulateContractCall = async () => {
      calls.simulate += 1;
      return {
        ok: true,
        dataspace: "universal",
        contract_address: "irohac1routerfixture",
        code_hash_hex: simulationCodeHash,
        abi_hash_hex: "22".repeat(32),
        entrypoint: "route_swap",
        normalized_payload: null,
        gas_limit: 5000,
        gas_used: 1,
        queued_instructions: [],
        result: null,
        error: null,
        vm_diagnostic: null,
        ...simulationOverrides,
      };
    };
    client.getContractCodeBytes = async (codeHash, fetchOptions) => {
      calls.fetchCode += 1;
      onCodeFetch(codeHash, fetchOptions);
      return fetchedCodeResponse === undefined
        ? { code_b64: fetchedBytecode }
        : fetchedCodeResponse;
    };
    client.deriveIvmProved = async () => {
      calls.derive += 1;
      return { proved: derivedProved };
    };
    client.proveIvmAndWait = async () => {
      calls.prove += 1;
      onProof();
      return {
        job_id: "ab".repeat(16),
        status: "done",
        error: null,
        proved: derivedProved,
        attachment,
      };
    };
    client.submitTransaction = async () => {
      calls.submit += 1;
      throw new Error("transaction must not submit");
    };

    const previous = globalThis.__IROHA_NATIVE_BINDING__;
    globalThis.__IROHA_NATIVE_BINDING__ = {
      buildIvmProvedTransaction: () => {
        calls.sign += 1;
        throw new Error("transaction must not be signed");
      },
    };
    try {
      await assert.rejects(
        () =>
          submitIvmProvedContractCall(
            client,
            {
              networkId: NETWORK_ID,
              authority: AUTHORITY_ID_INPUT,
              privateKey: PRIVATE_KEY,
              vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
              contractAlias: "dlmm_router::dlmm.universal",
              feePayment: {
                ...IVM_AUTHORITY_FEE_PAYMENT,
                gasLimit: 5000,
              },
              ...input,
            },
            options,
          ),
        expected,
      );
    } finally {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
    assert.equal(calls.sign, 0);
    assert.equal(calls.submit, 0);
    return calls;
  }

  await t.test("rejects invalid wait options before every remote side effect", async () => {
    for (const [options, expected] of [
      [{ proofIntervalMs: -1 }, /proofIntervalMs.*non-negative/i],
      [{ proofTimeoutMs: Number.NaN }, /proofTimeoutMs.*integer/i],
      [{ waitForCommit: "true" }, /waitForCommit must be a boolean/],
      [{ transactionIntervalMs: -1 }, /intervalMs.*non-negative/i],
      [{ transactionTimeoutMs: -1 }, /timeoutMs.*non-negative/i],
      [
        { transactionStatusScope: "global" },
        /transactionStatusScope is unsupported/u,
      ],
    ]) {
      const calls = await rejectsBeforeSigning({ options, expected });
      assert.equal(calls.simulate, 0);
      assert.equal(calls.fetchCode, 0);
      assert.equal(calls.derive, 0);
      assert.equal(calls.prove, 0);
    }
  });

  await t.test("requires caller-trusted code and full-artifact hashes before simulation", async () => {
    const missing = await rejectsBeforeSigning({
      expected: /expectedCodeHashHex must be exactly 32 hexadecimal bytes/,
    });
    assert.deepEqual(missing, {
      simulate: 0,
      fetchCode: 0,
      derive: 0,
      prove: 0,
      sign: 0,
      submit: 0,
    });
    const duplicate = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expected_code_hash_hex: ZK_IVM_CODE_HASH_HEX,
      },
      expected: /must use exactly one of expectedCodeHashHex, expected_code_hash_hex/,
    });
    assert.equal(duplicate.simulate, 0);

    const missingArtifactHash = await rejectsBeforeSigning({
      input: { expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX },
      expected: /expectedArtifactSha256Hex must be exactly 32 hexadecimal bytes/,
    });
    assert.equal(missingArtifactHash.simulate, 0);
  });

  await t.test("rejects every conflicting input alias before simulation", async () => {
    const trusted = {
      expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
      expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
    };
    const cases = [
      [{ ...trusted, chain: "test-chain" }, /input\.chain is unsupported/u],
      [{ ...trusted, chainId: "test-chain" }, /input\.chainId is unsupported/u],
      [{ ...trusted, chain_id: "test-chain" }, /input\.chain_id is unsupported/u],
      [{ ...trusted, private_key: PRIVATE_KEY }, /exactly one of privateKey, private_key/],
      [
        {
          ...trusted,
          privateKeyAlgorithm: "ed25519",
          private_key_algorithm: "ed25519",
        },
        /exactly one of privateKeyAlgorithm, private_key_algorithm/,
      ],
      [
        { ...trusted, vk_ref: { backend: "halo2/ipa", name: "ivm-exec-v1" } },
        /exactly one of vkRef, vk_ref/,
      ],
      [
        { ...trusted, contract_alias: "attacker::router.universal" },
        /exactly one of contractAlias, contract_alias/,
      ],
      [
        { ...trusted, contractAddress: "irohac1attacker" },
        /exactly one of contractAddress or contractAlias/,
      ],
      [
        { ...trusted, fee_payment: IVM_AUTHORITY_FEE_PAYMENT },
        /exactly one of feePayment, fee_payment/,
      ],
      [
        { ...trusted, gasAssetId: null },
        /gasAssetId is retired/,
      ],
      [
        { ...trusted, feeSponsor: null },
        /feeSponsor is retired/,
      ],
      [
        { ...trusted, gasLimit: 5000 },
        /gasLimit is retired/,
      ],
      [
        { ...trusted, creationTimeMs: 1, creation_time_ms: 1 },
        /exactly one of creationTimeMs, creation_time_ms/,
      ],
      [{ ...trusted, ttlMs: 1, ttl_ms: 1 }, /exactly one of ttlMs, ttl_ms/],
      [
        {
          ...trusted,
          requiredOverlayTransfer: null,
          required_overlay_transfer: null,
        },
        /exactly one of requiredOverlayTransfer, required_overlay_transfer/,
      ],
      [
        {
          ...trusted,
          validationFeePolicy: null,
          validation_fee_policy: null,
        },
        /exactly one of validationFeePolicy, validation_fee_policy/,
      ],
      [
        {
          expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
          expected_code_hash_hex: ZK_IVM_CODE_HASH_HEX,
          expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
        },
        /exactly one of expectedCodeHashHex, expected_code_hash_hex/,
      ],
      [
        {
          ...trusted,
          expected_artifact_sha256_hex: ZK_IVM_ARTIFACT_SHA256_HEX,
        },
        /exactly one of expectedArtifactSha256Hex, expected_artifact_sha256_hex/,
      ],
    ];
    for (const [input, expected] of cases) {
      const calls = await rejectsBeforeSigning({ input, expected });
      assert.equal(calls.simulate, 0);
      assert.equal(calls.derive, 0);
      assert.equal(calls.prove, 0);
    }
  });

  await t.test("rejects malformed signing, metadata, payload, and timing inputs before simulation", async () => {
    const trusted = {
      expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
      expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
    };
    const circularPayload = {};
    circularPayload.self = circularPayload;
    for (const [input, expected] of [
      [{ ...trusted, privateKey: Buffer.alloc(31) }, /32- or 64-byte Ed25519 key/],
      [{ ...trusted, privateKeyAlgorithm: " ed25519" }, /surrounding whitespace/],
      [{ ...trusted, privateKeyAlgorithm: "attacker" }, /unsupported crypto algorithm/],
      [{ ...trusted, metadata: { contract_address: "attacker" } }, /reserved/],
      [{ ...trusted, payload: circularPayload }, /must be JSON-serializable/],
      [
        {
          ...trusted,
          feePayment: { ...IVM_AUTHORITY_FEE_PAYMENT, gasLimit: 0 },
        },
        /gasLimit.*outside|non-zero/i,
      ],
      [{ ...trusted, creationTimeMs: -1 }, /non-negative/i],
      [{ ...trusted, ttlMs: Number.NaN }, /integer/i],
      [{ ...trusted, nonce: 0 }, /positive|greater than zero/i],
      [{ ...trusted, nonce: 0x1_0000_0000 }, /fit in u32/],
    ]) {
      const calls = await rejectsBeforeSigning({ input, expected });
      assert.equal(calls.simulate, 0);
      assert.equal(calls.fetchCode, 0);
      assert.equal(calls.derive, 0);
    }
  });

  await t.test("rejects a substituted simulation binding before code fetch", async () => {
    const calls = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      },
      simulationCodeHash: "11".repeat(32),
      expected: /does not match caller-trusted expected code hash/,
    });
    assert.equal(calls.simulate, 1);
    assert.equal(calls.fetchCode, 0);
    assert.equal(calls.derive, 0);
    assert.equal(calls.prove, 0);
  });

  await t.test("rejects substituted simulation target, entrypoint, and gas before code fetch", async () => {
    const trusted = {
      expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
      expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
    };
    for (const attack of [
      {
        input: {
          ...trusted,
          contractAlias: undefined,
          contractAddress: "irohac1trustedfixture",
        },
        expected: /different contract address than requested/,
      },
      {
        input: { ...trusted, entrypoint: "trusted_entrypoint" },
        expected: /different entrypoint than requested/,
      },
      {
        input: { ...trusted, payload: { amount: "7" } },
        simulationOverrides: { normalized_payload: { amount: "8" } },
        expected: /normalized payload differs from the requested payload/,
      },
      {
        input: trusted,
        simulationOverrides: { gas_limit: 5001 },
        expected: /gas limit 5001 does not match requested gas limit 5000/,
      },
    ]) {
      const calls = await rejectsBeforeSigning(attack);
      assert.equal(calls.simulate, 1);
      assert.equal(calls.fetchCode, 0);
      assert.equal(calls.derive, 0);
    }
  });

  await t.test("rejects header, body, and encoding substitutions before derive", async () => {
    const headerSubstitution = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      },
      fetchedBytecode: headerSubstitutedBytecode,
      expected: /artifact SHA-256 .* does not match caller-trusted expected artifact SHA-256/,
    });
    assert.equal(headerSubstitution.fetchCode, 1);
    assert.equal(headerSubstitution.derive, 0);

    const bodySubstitution = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: bodySubstitutedArtifactSha256Hex,
      },
      fetchedBytecode: bodySubstitutedBytecode,
      expected: /bytecode hash .* does not match expected code hash/,
    });
    assert.equal(bodySubstitution.fetchCode, 1);
    assert.equal(bodySubstitution.derive, 0);

    const nonCanonical = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      },
      fetchedBytecode: `${ZK_IVM_BYTECODE_BASE64}\n`,
      expected: /canonical standard base64/,
    });
    assert.equal(nonCanonical.fetchCode, 1);
    assert.equal(nonCanonical.derive, 0);

    const maxBase64Length = Math.ceil(IVM_ARTIFACT_MAX_BYTES / 3) * 4;
    for (const fetchedBytecode of [
      "A".repeat(maxBase64Length + 1),
      Buffer.alloc(IVM_ARTIFACT_MAX_BYTES + 1).toString("base64"),
    ]) {
      const oversized = await rejectsBeforeSigning({
        input: {
          expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
          expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
        },
        fetchedBytecode,
        expected: /exceeds the 4194304-byte artifact limit/,
      });
      assert.equal(oversized.simulate, 1);
      assert.equal(oversized.fetchCode, 1);
      assert.equal(oversized.derive, 0);
      assert.equal(oversized.prove, 0);
      assert.equal(oversized.sign, 0);
      assert.equal(oversized.submit, 0);
    }
  });

  await t.test("forwards the validated AbortSignal to the code-byte fetch", async () => {
    const controller = new AbortController();
    let captured;
    const calls = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      },
      options: { signal: controller.signal },
      fetchedBytecode: `${ZK_IVM_BYTECODE_BASE64}\n`,
      onCodeFetch(codeHash, fetchOptions) {
        captured = { codeHash, fetchOptions };
      },
      expected: /canonical standard base64/,
    });
    assert.equal(calls.fetchCode, 1);
    assert.equal(captured.codeHash, ZK_IVM_CODE_HASH_HEX);
    assert.equal(captured.fetchOptions.signal, controller.signal);
  });

  await t.test("rejects ambiguous code-byte response shapes before derive", async () => {
    const calls = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      },
      fetchedCodeResponse: {
        code_b64: ZK_IVM_BYTECODE_BASE64,
        attacker_bytecode: headerSubstitutedBytecode,
      },
      expected: /must contain exactly the code_b64 field/,
    });
    assert.equal(calls.simulate, 1);
    assert.equal(calls.fetchCode, 1);
    assert.equal(calls.derive, 0);
    assert.equal(calls.prove, 0);
    assert.equal(calls.sign, 0);
    assert.equal(calls.submit, 0);
  });

  await t.test("rejects derive bytecode substitution before proving", async () => {
    const calls = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      },
      derivedProved: { ...validProved, bytecode: headerSubstitutedBytecode },
      expected: /differs from the code-hash-bound deployed contract bytecode/,
    });
    assert.equal(calls.derive, 1);
    assert.equal(calls.prove, 0);
  });

  await t.test("rejects attachment backend and vk_ref substitution after proof", async () => {
    const backend = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      },
      attachment: { ...validAttachment, backend: "stark/fri" },
      expected: /attachment backend differs/,
    });
    assert.equal(backend.prove, 1);

    const proofBackend = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      },
      attachment: {
        ...validAttachment,
        proof: { ...validAttachment.proof, backend: "stark/fri" },
      },
      expected: /attachment proof backend differs/,
    });
    assert.equal(proofBackend.prove, 1);

    const vkRef = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      },
      attachment: {
        ...validAttachment,
        vk_ref: { backend: "halo2/ipa", name: "attacker-key" },
      },
      expected: /attachment vk_ref differs/,
    });
    assert.equal(vkRef.prove, 1);
  });

  await t.test("honors abort after proof before signing or submission", async () => {
    const controller = new AbortController();
    const calls = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      },
      options: { signal: controller.signal },
      onProof() {
        controller.abort(new Error("abort before final transaction submission"));
      },
      expected: /abort before final transaction submission/,
    });
    assert.equal(calls.prove, 1);
    assert.equal(calls.sign, 0);
    assert.equal(calls.submit, 0);
  });

  await t.test("honors intrinsic abort state despite hostile signal shadows", async () => {
    const controller = new AbortController();
    const calls = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      },
      options: { signal: controller.signal },
      onProof() {
        controller.abort(new Error("intrinsic abort must prevent signing"));
        Object.defineProperties(controller.signal, {
          aborted: { value: false },
          reason: { value: undefined },
          throwIfAborted: { value() {} },
        });
      },
      expected: /intrinsic abort must prevent signing/,
    });
    assert.equal(calls.prove, 1);
    assert.equal(calls.sign, 0);
    assert.equal(calls.submit, 0);
  });
});

test("submitIvmProvedContractCall proof-binds, quotes, rebuilds, and signs", async () => {
  const requiredOverlayTransfer = {
    sourceAssetHoldingId: CANONICAL_ASSET_ID_INPUT,
    quantity: "0.1",
    destinationAccountId: RELAY_ACCOUNT_ID_INPUT,
  };
  const expectedTransfer = buildTransferAssetInstruction({
    sourceAssetHoldingId: CANONICAL_ASSET_ID_INPUT,
    quantity: "0.1",
    destinationAccountId: RELAY_ACCOUNT_ID_INPUT,
  });
  const principalTransfer = buildTransferAssetInstruction({
    sourceAssetHoldingId: CANONICAL_ASSET_ID_INPUT,
    quantity: "1",
    destinationAccountId: RELAY_ACCOUNT_ID_INPUT,
  });
  const proved = {
    bytecode: ZK_IVM_BYTECODE_BASE64,
    overlay: [principalTransfer, expectedTransfer],
    events_commitment: normalizedHashHex(Buffer.alloc(32, 0x01)),
    gas_policy_commitment: normalizedHashHex(Buffer.alloc(32, 0x02)),
  };
  const attachment = {
    backend: "halo2/ipa",
    proof: { backend: "halo2/ipa", bytes: [1, 2, 3] },
    vk_ref: { backend: "halo2/ipa", name: "ivm-exec-v1" },
  };
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () => {
      throw new Error("network fetch should be replaced by focused client stubs");
    },
  });
  const captures = {};
  const submissionController = new AbortController();
  client.simulateContractCall = async (request) => {
    captures.simulationRequest = request;
    return {
      ok: true,
      dataspace: "universal",
      contract_address: "irohac1routerfixture",
      code_hash_hex: ZK_IVM_CODE_HASH_HEX,
      abi_hash_hex: "22".repeat(32),
      entrypoint: "route_swap",
      normalized_payload: { amount: "7" },
      gas_limit: 5000,
      gas_used: 900,
      queued_instructions: [principalTransfer, expectedTransfer],
      result: null,
      error: null,
      vm_diagnostic: null,
    };
  };
  client.getContractCodeBytes = async (codeHash) => {
    captures.codeHash = codeHash;
    return { code_b64: proved.bytecode };
  };
  client.deriveIvmProved = async (request) => {
    captures.deriveRequest = request;
    return { proved };
  };
  client.proveIvmAndWait = async (request, options) => {
    captures.proveRequest = request;
    captures.proveOptions = options;
    return {
      job_id: "ab".repeat(16),
      status: "done",
      error: null,
      proved,
      attachment,
    };
  };
  const quotedIntent = {
    payer: "authority",
    value: {
      charge_limits: [
        {
          kind: { kind: "pipeline_gas", value: null },
          asset_definition_id: ASSET_DEFINITION_ID,
          max_amount: "1",
        },
      ],
      gas_limit: 5000,
    },
  };
  client.quoteFees = async (draft, options) => {
    captures.feeQuoteDraft = draft;
    captures.feeQuoteOptions = options;
    return {
      intent: quotedIntent,
      observation: {},
      components: [],
      capacities: [],
    };
  };
  client.submitTransaction = async (payload, options) => {
    captures.submitted = Buffer.from(payload);
    captures.submitOptions = options;
    return { accepted: true };
  };

  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = {
    buildIvmProvedTransactionPayload: (
      networkId,
      authority,
      provedPayload,
      attachmentPayload,
      feePaymentJson,
      metadataPayload,
      creationTimeMs,
      ttlMs,
      nonce,
    ) => {
      captures.draft = {
        networkId,
        authority,
        proved: JSON.parse(provedPayload),
        attachment: JSON.parse(attachmentPayload),
        feePayment: JSON.parse(feePaymentJson),
        metadata: JSON.parse(metadataPayload),
        creationTimeMs,
        ttlMs,
        nonce,
      };
      const payload = {
        domain: { kind: "network", value: NETWORK_ID.literal },
        authority,
        fee_payment: JSON.parse(feePaymentJson),
      };
      return {
        payload_json: JSON.stringify(payload),
        payload_bytes: Buffer.from([0x01, 0x02]),
        payload_hash: Buffer.alloc(32, 0xaa),
      };
    },
    signQuotedIvmProvedTransactionPayload: (
      networkId,
      payloadJson,
      attachmentJson,
      quotedFeePaymentJson,
      secret,
    ) => {
      captures.signed = {
        networkId: Buffer.from(networkId),
        payload: JSON.parse(payloadJson),
        attachment: JSON.parse(attachmentJson),
        quotedFeePayment: JSON.parse(quotedFeePaymentJson),
        secret: Buffer.from(secret),
      };
      return {
        signed_transaction: Buffer.from([0x03, 0x04]),
        hash: Buffer.alloc(32, 0xbb),
      };
    },
  };
  let result;
  try {
    result = await submitIvmProvedContractCall(
      client,
      {
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        privateKey: PRIVATE_KEY,
        vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
        contractAlias: "dlmm_router::dlmm.universal",
        entrypoint: "route_swap",
        payload: { amount: "7" },
        feePayment: { ...IVM_AUTHORITY_FEE_PAYMENT, gasLimit: 5000 },
        metadata: { request_id: "swap-7" },
        requiredOverlayTransfer,
      },
      {
        proofIntervalMs: 0,
        proofTimeoutMs: 1000,
        signal: submissionController.signal,
      },
    );
  } finally {
    globalThis.__IROHA_NATIVE_BINDING__ = previous;
  }

  assert.equal(captures.codeHash, ZK_IVM_CODE_HASH_HEX);
  assert.equal(
    Object.prototype.hasOwnProperty.call(captures.proveRequest, "proved"),
    false,
  );
  assert.equal(captures.proveRequest.bytecode, proved.bytecode);
  assert.deepEqual(captures.deriveRequest.metadata, {
    request_id: "swap-7",
    contract_address: "irohac1routerfixture",
    contract_entrypoint: "route_swap",
    contract_alias: "dlmm_router::dlmm.universal",
    contract_payload: { amount: "7" },
  });
  assert.deepEqual(captures.draft.proved, proved);
  assert.deepEqual(captures.draft.attachment, attachment);
  assert.deepEqual(captures.signed.networkId, NETWORK_ID_BYTES);
  assert.deepEqual(captures.signed.attachment, attachment);
  assert.deepEqual(captures.signed.quotedFeePayment, quotedIntent);
  assert.deepEqual(captures.feeQuoteDraft.payload, captures.signed.payload);
  assert.equal(captures.feeQuoteOptions.canonicalAuth.accountId, AUTHORITY_ID_INPUT);
  assert.deepEqual(
    Buffer.from(captures.feeQuoteOptions.canonicalAuth.privateKey),
    PRIVATE_KEY,
  );
  assert.deepEqual(captures.submitted, Buffer.from([0x03, 0x04]));
  assert.equal(captures.submitOptions.signal, submissionController.signal);
  assert.equal(result.hash, "bb".repeat(32));
  assert.deepEqual(result.requiredOverlayTransfer, expectedTransfer);
  assert.deepEqual(result.feeQuote.intent, quotedIntent);
});

test("submitIvmProvedContractCall keeps a 4 MiB proof request below Torii's default cap", async () => {
  const artifact = Buffer.alloc(IVM_ARTIFACT_MAX_BYTES);
  Buffer.from(ZK_IVM_BYTECODE_BASE64, "base64")
    .subarray(0, 17)
    .copy(artifact, 0);
  const hashes = computeIvmArtifactHashes(artifact);
  const bytecode = artifact.toString("base64");
  const proved = {
    bytecode,
    overlay: [],
    events_commitment: "01".repeat(32),
    gas_policy_commitment: "02".repeat(32),
  };
  let proofRequest;
  let signCalls = 0;
  let submitCalls = 0;
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () => {
      throw new Error("focused stubs must replace network access");
    },
  });
  client.simulateContractCall = async () => ({
    ok: true,
    dataspace: "universal",
    contract_address: "irohac1routerfixture",
    code_hash_hex: hashes.codeHashHex,
    abi_hash_hex: "22".repeat(32),
    entrypoint: "route_swap",
    normalized_payload: null,
    gas_limit: 5000,
    gas_used: 1,
    queued_instructions: [],
    result: null,
    error: null,
    vm_diagnostic: null,
  });
  client.getContractCodeBytes = async () => ({ code_b64: bytecode });
  client.deriveIvmProved = async () => ({ proved });
  client.proveIvmAndWait = async (request) => {
    proofRequest = request;
    throw new Error("stop after capturing the proof request");
  };
  client.submitTransaction = async () => {
    submitCalls += 1;
    throw new Error("must not submit");
  };
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = {
    buildIvmProvedTransaction() {
      signCalls += 1;
      throw new Error("must not sign");
    },
  };
  try {
    await assert.rejects(
      () =>
        submitIvmProvedContractCall(client, {
          networkId: NETWORK_ID,
          authority: AUTHORITY_ID_INPUT,
          privateKey: PRIVATE_KEY,
          vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
          expectedCodeHashHex: hashes.codeHashHex,
          expectedArtifactSha256Hex: hashes.artifactSha256Hex,
          contractAlias: "dlmm_router::dlmm.universal",
          feePayment: { ...IVM_AUTHORITY_FEE_PAYMENT, gasLimit: 5000 },
        }),
      /stop after capturing the proof request/,
    );
  } finally {
    globalThis.__IROHA_NATIVE_BINDING__ = previous;
  }
  assert.equal(Object.hasOwn(proofRequest, "proved"), false);
  assert.equal(proofRequest.bytecode, bytecode);
  assert.ok(
    Buffer.byteLength(JSON.stringify(proofRequest), "utf8") <= 8 * 1024 * 1024,
    "max artifact proof request must fit Torii's default 8 MiB body cap",
  );
  assert.equal(signCalls, 0);
  assert.equal(submitCalls, 0);
});


test("submitIvmProvedContractCall preserves generic non-policy overlay assertions", async () => {
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () => {
      throw new Error("network fetch should not run");
    },
  });
  let simulationCalled = false;
  client.simulateContractCall = async () => {
    simulationCalled = true;
    return {
      ok: true,
      dataspace: "universal",
      contract_address: "irohac1routerfixture",
      code_hash_hex: ZK_IVM_CODE_HASH_HEX,
      abi_hash_hex: "22".repeat(32),
      entrypoint: "generic_route",
      normalized_payload: null,
      gas_limit: 5000,
      gas_used: 1,
      queued_instructions: [],
      result: null,
      error: null,
      vm_diagnostic: null,
    };
  };
  client.getContractCodeBytes = async () => ({
    code_b64: ZK_IVM_BYTECODE_BASE64,
  });
  client.deriveIvmProved = async () => ({
    proved: {
      bytecode: ZK_IVM_BYTECODE_BASE64,
      overlay: [],
      events_commitment: normalizedHashHex(Buffer.alloc(32, 0x01)),
      gas_policy_commitment: normalizedHashHex(Buffer.alloc(32, 0x02)),
    },
  });
  let proveCalled = false;
  client.proveIvmAndWait = async () => {
    proveCalled = true;
    throw new Error("proof should not start");
  };
  await assert.rejects(
    () =>
      submitIvmProvedContractCall(client, {
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        privateKey: PRIVATE_KEY,
        vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
        contractAlias: "dlmm_router::dlmm.universal",
        feePayment: { ...IVM_AUTHORITY_FEE_PAYMENT, gasLimit: 5000 },
        requiredOverlayTransfer: {
          sourceAssetHoldingId: CANONICAL_ASSET_ID_INPUT,
          quantity: "0.1",
          destinationAccountId: RELAY_ACCOUNT_ID_INPUT,
        },
      }),
    /must contain the required overlay transfer exactly once \(found 0\)/,
  );
  assert.equal(simulationCalled, true);
  assert.equal(proveCalled, false);
});


test("submitIvmProvedContractCall rejects caller validation-fee metadata", async () => {
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () => {
      throw new Error("network fetch should be replaced by focused client stubs");
    },
  });
  client.simulateContractCall = async () => ({
    ok: true,
    dataspace: "universal",
    contract_address: "irohac1routerfixture",
    code_hash_hex: ZK_IVM_CODE_HASH_HEX,
    abi_hash_hex: "22".repeat(32),
    entrypoint: "route_swap",
    normalized_payload: null,
    gas_limit: 5000,
    gas_used: 1,
    queued_instructions: [],
    result: null,
    error: null,
    vm_diagnostic: null,
  });
  client.getContractCodeBytes = async () => ({ code_b64: ZK_IVM_BYTECODE_BASE64 });
  let deriveCalled = false;
  client.deriveIvmProved = async () => {
    deriveCalled = true;
    throw new Error("derive should not start");
  };
  await assert.rejects(
    () =>
      submitIvmProvedContractCall(client, {
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        privateKey: PRIVATE_KEY,
        vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
        contractAlias: "dlmm_router::dlmm.universal",
        feePayment: { ...IVM_AUTHORITY_FEE_PAYMENT, gasLimit: 5000 },
        metadata: { validation_fee_policy_hash: "00".repeat(32) },
        validationFeePolicy: {},
      }),
    /metadata\.validation_fee_policy_hash is reserved/,
  );
  assert.equal(deriveCalled, false);
});


test("submitIvmProvedContractCall rejects conventional non-ZK deployed bytecode", async () => {
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () => {
      throw new Error("network fetch should be replaced by focused client stubs");
    },
  });
  client.simulateContractCall = async () => ({
    ok: true,
    dataspace: "universal",
    contract_address: "irohac1routerfixture",
    code_hash_hex: ZK_IVM_CODE_HASH_HEX,
    abi_hash_hex: "22".repeat(32),
    entrypoint: "route_swap",
    normalized_payload: null,
    gas_limit: 5000,
    gas_used: 1,
    queued_instructions: [],
    result: null,
    error: null,
    vm_diagnostic: null,
  });
  const nonZkBytecode = Buffer.from(ZK_IVM_BYTECODE_BASE64, "base64");
  nonZkBytecode[6] = 0;
  client.getContractCodeBytes = async () => ({
    code_b64: nonZkBytecode.toString("base64"),
  });
  let deriveCalled = false;
  client.deriveIvmProved = async () => {
    deriveCalled = true;
    throw new Error("derive should not start");
  };

  await assert.rejects(
    () =>
      submitIvmProvedContractCall(client, {
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        privateKey: PRIVATE_KEY,
        vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
        contractAlias: "dlmm_router::dlmm.universal",
        entrypoint: "route_swap",
        feePayment: { ...IVM_AUTHORITY_FEE_PAYMENT, gasLimit: 5000 },
      }),
    /not ZK mode/,
  );
  assert.equal(deriveCalled, false);
});

test("submitIvmProvedContractCall rejects a prover payload that differs from derivation", async () => {
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () => {
      throw new Error("network fetch should be replaced by focused client stubs");
    },
  });
  const derived = {
    bytecode: ZK_IVM_BYTECODE_BASE64,
    overlay: [],
    events_commitment: normalizedHashHex(Buffer.alloc(32, 0x01)),
    gas_policy_commitment: normalizedHashHex(Buffer.alloc(32, 0x02)),
  };
  client.simulateContractCall = async () => ({
    ok: true,
    dataspace: "universal",
    contract_address: "irohac1routerfixture",
    code_hash_hex: ZK_IVM_CODE_HASH_HEX,
    abi_hash_hex: "22".repeat(32),
    entrypoint: "route_swap",
    normalized_payload: null,
    gas_limit: 5000,
    gas_used: 1,
    queued_instructions: [],
    result: null,
    error: null,
    vm_diagnostic: null,
  });
  client.getContractCodeBytes = async () => ({ code_b64: derived.bytecode });
  client.deriveIvmProved = async () => ({ proved: derived });
  client.proveIvmAndWait = async () => ({
    job_id: "ab".repeat(16),
    status: "done",
    error: null,
    proved: {
      ...derived,
      events_commitment: normalizedHashHex(Buffer.alloc(32, 0x03)),
    },
    attachment: { backend: "halo2/ipa" },
  });
  let submitCalled = false;
  client.submitTransaction = async () => {
    submitCalled = true;
    throw new Error("transaction should not submit");
  };

  await assert.rejects(
    () =>
      submitIvmProvedContractCall(client, {
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        privateKey: PRIVATE_KEY,
        vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
        contractAlias: "dlmm_router::dlmm.universal",
        entrypoint: "route_swap",
        feePayment: { ...IVM_AUTHORITY_FEE_PAYMENT, gasLimit: 5000 },
      }),
    /different from the authoritative derived payload/,
  );
  assert.equal(submitCalled, false);
});

test("buildMintAssetTransaction returns canonical hash", () => {
  const built = buildMintAssetTransaction({
    networkId: NETWORK_ID,
    authority: AUTHORITY_ID_INPUT,
    feePayment: AUTHORITY_FEE_PAYMENT,
    assetId: CANONICAL_ASSET_ID_INPUT,
    quantity: "10",
    privateKey: PRIVATE_KEY,
  });
  assert.ok(Buffer.isBuffer(built.signedTransaction));
  const recomputed = hashSignedTransaction(built.signedTransaction, {
    encoding: "buffer",
  });
  assert.deepEqual(recomputed, built.hash);
});

test("buildTransferAssetTransaction returns canonical hash", () => {
  const built = buildTransferAssetTransaction({
    networkId: NETWORK_ID,
    authority: AUTHORITY_ID_INPUT,
    feePayment: AUTHORITY_FEE_PAYMENT,
    sourceAssetId: CANONICAL_ASSET_ID_INPUT,
    quantity: "3",
    destinationAccountId: AUTHORITY_ID_INPUT,
    privateKey: PRIVATE_KEY,
  });
  assert.ok(Buffer.isBuffer(built.signedTransaction));
  const recomputed = hashSignedTransaction(built.signedTransaction, {
    encoding: "buffer",
  });
  assert.deepEqual(recomputed, built.hash);
});

test("buildTransferRwaTransaction returns canonical hash", () => {
  const built = buildTransferRwaTransaction({
    networkId: NETWORK_ID,
    authority: AUTHORITY_ID_INPUT,
    feePayment: AUTHORITY_FEE_PAYMENT,
    sourceAccountId: AUTHORITY_ID_INPUT,
    rwaId: RWA_ID,
    quantity: "3",
    destinationAccountId: AUTHORITY_ID_INPUT,
    privateKey: PRIVATE_KEY,
  });
  assert.ok(Buffer.isBuffer(built.signedTransaction));
  const recomputed = hashSignedTransaction(built.signedTransaction, {
    encoding: "buffer",
  });
  assert.deepEqual(recomputed, built.hash);
});

baseTest("transaction builders reject padded authority and asset definition IDs before native dispatch", () => {
  const calls = [];
  withNativeBinding(
    {
      buildTransaction: () => {
        calls.push("buildTransaction");
        return {
          signed_transaction: Buffer.from([0x47]),
          hash: Buffer.alloc(32, 0x47),
        };
      },
    },
    () => {
      assert.throws(
        () =>
          buildTransaction({
            networkId: NETWORK_ID,
            authority: ` ${AUTHORITY_ID_INPUT}`,
            feePayment: AUTHORITY_FEE_PAYMENT,
            instructions: [{ RegisterDomain: { id: "wonderland" } }],
            privateKey: PRIVATE_KEY,
          }),
        /authority must not contain surrounding whitespace/u,
      );
      assert.throws(
        () =>
          buildRegisterAssetDefinitionAndMintTransaction({
            networkId: NETWORK_ID,
            authority: AUTHORITY_ID_INPUT,
            feePayment: AUTHORITY_FEE_PAYMENT,
            assetDefinition: {
              assetDefinitionId: `${ASSET_DEFINITION_ID} `,
              spec: { scale: 0 },
            },
            privateKey: PRIVATE_KEY,
          }),
        /assetDefinition\.assetDefinitionId must not contain surrounding whitespace/u,
      );
    },
  );
  assert.deepEqual(calls, []);
});

test("buildRegisterRwaTransaction forwards canonical instruction payload", () => {
  const captures = [];
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({
          authority,
          instructions: instructions.map((json) => JSON.parse(json)),
        });
        return {
          signed_transaction: Buffer.from([0x44]),
          hash: Buffer.alloc(32, 0xdd),
        };
      },
    },
    () =>
      buildRegisterRwaTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        rwa: {
          domain: "commodities.sora",
          quantity: "10.5",
          spec: { scale: 1 },
          primaryReference: "vault-cert-001",
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  assert.equal(captures[0].authority, AUTHORITY_ID);
  assert.deepEqual(captures[0].instructions[0], {
    RegisterRwa: {
      rwa: {
        domain: "commodities.sora",
        quantity: "10.5",
        spec: { scale: 1 },
        primary_reference: "vault-cert-001",
        status: null,
        metadata: {},
        parents: [],
        controls: {
          controller_accounts: [],
          controller_roles: [],
          freeze_enabled: false,
          hold_enabled: false,
          force_transfer_enabled: false,
          redeem_enabled: false,
        },
      },
    },
  });
});

test("buildRwaKeyValueTransactions forward canonical instruction payloads", () => {
  const captures = [];
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({
          authority,
          instructions: instructions.map((json) => JSON.parse(json)),
        });
        return {
          signed_transaction: Buffer.from([0x45]),
          hash: Buffer.alloc(32, 0xee),
        };
      },
    },
    () => {
      buildSetRwaKeyValueTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        rwaId: RWA_ID,
        key: "grade",
        value: { origin: "AE", score: BigInt(9) },
        privateKey: PRIVATE_KEY,
      });
      buildRemoveRwaKeyValueTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        rwaId: RWA_ID,
        key: "grade",
        privateKey: PRIVATE_KEY,
      });
    },
  );
  assert.equal(captures.length, 2);
  assert.equal(captures[0].authority, AUTHORITY_ID);
  assert.deepEqual(captures[0].instructions[0], {
    SetRwaKeyValue: {
      rwa: RWA_ID,
      key: "grade",
      value: { origin: "AE", score: "9" },
    },
  });
  assert.deepEqual(captures[1].instructions[0], {
    RemoveRwaKeyValue: {
      rwa: RWA_ID,
      key: "grade",
    },
  });
});

test("buildMintAndTransferTransaction composes instructions in order", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x10, 0x20]),
    hash: Buffer.alloc(32, 0xbb),
  };
  withNativeBinding(
    {
      buildTransaction: (_, __, instructions) => {
        captures.push(instructions.map((payload) => JSON.parse(payload)));
        return fakeResult;
      },
    },
    () => {
      const result = buildMintAndTransferTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        mint: { assetId: ASSET_ID_INPUT, quantity: "6" },
        transfer: {
          quantity: "2",
          destinationAccountId: AUTHORITY_ID_INPUT,
        },
        privateKey: PRIVATE_KEY,
      });
      assert.deepEqual(result.hash, Buffer.from(fakeResult.hash));
    },
  );
  assert.equal(captures.length, 1);
  const [mintInstruction, transferInstruction] = captures[0];
  assert.deepEqual(mintInstruction, {
    Mint: { Asset: { destination: ASSET_ID, object: "6" } },
  });
  assert.deepEqual(transferInstruction, {
    Transfer: {
      Asset: {
        source: ASSET_ID,
        object: "2",
        destination: AUTHORITY_ID,
      },
    },
  });
});

test("buildRegisterDomainAndMintTransaction supports mint arrays", () => {
  const captures = [];
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({
          authority,
          instructions: instructions.map((j) => JSON.parse(j)),
        });
        return {
          signed_transaction: Buffer.from([0x30]),
          hash: Buffer.alloc(32, 0xcc),
        };
      },
    },
    () =>
      buildRegisterDomainAndMintTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        domain: { domainId: "wonderland.sora" },
        mints: [
          { assetId: ASSET_ID_INPUT, quantity: "4" },
          { assetId: CANONICAL_LILY_ASSET_ID_INPUT, quantity: "1" },
        ],
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  assert.equal(instructions.length, 3);
  assert.deepEqual(
    instructions[0],
    buildRegisterDomainInstruction({ domainId: "wonderland.sora" }),
  );
  assert.deepEqual(instructions[1], {
    Mint: { Asset: { destination: ASSET_ID, object: "4" } },
  });
  assert.deepEqual(instructions[2], {
    Mint: {
      Asset: {
        destination: CANONICAL_LILY_ASSET_ID_INPUT,
        object: "1",
      },
    },
  });
});

test("buildRegisterAssetDefinitionMintAndTransferTransaction supports transfer arrays", () => {
  const captures = [];
  const secondAccountIdPublicKeyHex =
    "1AA70BFDE38BFD7CBE6AD29E59F290D4A4B0DD02792C0CE7371477C4E0D62759";
  const secondAccountId = i105FromEd25519PublicKeyHex(
    secondAccountIdPublicKeyHex,
  );
  const secondAccountIdInput = i105FromEd25519PublicKeyHex(
    secondAccountIdPublicKeyHex,
  );
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({
          authority,
          instructions: instructions.map((j) => JSON.parse(j)),
        });
        return {
          signed_transaction: Buffer.from([0x31]),
          hash: Buffer.alloc(32, 0xdd),
        };
      },
    },
    () =>
      buildRegisterAssetDefinitionMintAndTransferTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        assetDefinition: { assetDefinitionId: ASSET_DEFINITION_ID },
        mints: [
          { assetId: CANONICAL_ASSET_ID_INPUT, quantity: "7" },
          { assetId: SECOND_CANONICAL_ASSET_ID_INPUT, quantity: "2" },
        ],
        transfers: [
          { quantity: "5", destinationAccountId: AUTHORITY_ID_INPUT },
          {
            sourceAssetId: SECOND_CANONICAL_ASSET_ID_INPUT,
            quantity: "1",
            destinationAccountId: secondAccountIdInput,
          },
        ],
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  assert.equal(instructions.length, 5);
  assert.deepEqual(instructions[1], {
    Mint: {
      Asset: {
        destination: CANONICAL_ASSET_ID_INPUT,
        object: "7",
      },
    },
  });
  assert.deepEqual(instructions[2], {
    Mint: {
      Asset: {
        destination: SECOND_CANONICAL_ASSET_ID_INPUT,
        object: "2",
      },
    },
  });
  assert.deepEqual(instructions[3], {
    Transfer: {
      Asset: {
        source: CANONICAL_ASSET_ID_INPUT,
        object: "5",
        destination: AUTHORITY_ID,
      },
    },
  });
  assert.deepEqual(instructions[4], {
    Transfer: {
      Asset: {
        source: SECOND_CANONICAL_ASSET_ID_INPUT,
        object: "1",
        destination: secondAccountId,
      },
    },
  });
});

test("buildRegisterAssetDefinitionMintAndTransferTransaction derives asset ids from accountId", () => {
  const captures = [];
  withNativeBinding(
    {
      encodeAssetId: encodeAssetIdForKnownAccount,
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({
          authority,
          instructions: instructions.map((j) => JSON.parse(j)),
        });
        return {
          signed_transaction: Buffer.from([0x32]),
          hash: Buffer.alloc(32, 0xee),
        };
      },
    },
    () =>
      buildRegisterAssetDefinitionMintAndTransferTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        assetDefinition: { assetDefinitionId: ASSET_DEFINITION_ID },
        mints: [
          {
            accountId: AUTHORITY_ID_INPUT,
            assetId: CANONICAL_ASSET_ID_INPUT,
            quantity: "1",
          },
        ],
        transfers: [
          { quantity: "1", destinationAccountId: AUTHORITY_ID_INPUT },
        ],
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  assert.deepEqual(captures[0].instructions[1], {
    Mint: {
      Asset: {
        destination: CANONICAL_ASSET_ID_INPUT,
        object: "1",
      },
    },
  });
});

test("buildMintAndTransferTransaction returns canonical hash", () => {
  const built = buildMintAndTransferTransaction({
    networkId: NETWORK_ID,
    authority: AUTHORITY_ID_INPUT,
    feePayment: AUTHORITY_FEE_PAYMENT,
    mint: { assetId: CANONICAL_ASSET_ID_INPUT, quantity: "8" },
    transfer: {
      sourceAssetId: CANONICAL_ASSET_ID_INPUT,
      quantity: "3",
      destinationAccountId: AUTHORITY_ID_INPUT,
    },
    privateKey: PRIVATE_KEY,
  });
  assert.ok(Buffer.isBuffer(built.signedTransaction));
  const recomputed = hashSignedTransaction(built.signedTransaction, {
    encoding: "buffer",
  });
  assert.deepEqual(recomputed, built.hash);
});

test("buildRegisterAssetDefinitionMintAndTransferTransaction returns canonical hash", () => {
  const built = buildRegisterAssetDefinitionMintAndTransferTransaction({
    networkId: NETWORK_ID,
    authority: AUTHORITY_ID_INPUT,
    feePayment: AUTHORITY_FEE_PAYMENT,
    assetDefinition: { assetDefinitionId: ASSET_DEFINITION_ID },
    mint: { assetId: CANONICAL_ASSET_ID_INPUT, quantity: "4" },
    transfer: {
      sourceAssetId: CANONICAL_ASSET_ID_INPUT,
      destinationAccountId: AUTHORITY_ID_INPUT,
      quantity: "1",
    },
    privateKey: PRIVATE_KEY,
  });
  assert.ok(Buffer.isBuffer(built.signedTransaction));
  const recomputed = hashSignedTransaction(built.signedTransaction, {
    encoding: "buffer",
  });
  assert.deepEqual(recomputed, built.hash);
});

test("buildCreateKaigiTransaction composes Kaigi create instruction", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x40]),
    hash: Buffer.alloc(32, 0x55),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({
          authority,
          instructions: instructions.map((payload) => JSON.parse(payload)),
        });
        return fakeResult;
      },
    },
    () =>
      buildCreateKaigiTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        call: {
          id: { domainId: "wonderland.sora", callName: "weekly-sync" },
          host: AUTHORITY_ID_INPUT,
          gasRatePerMinute: 120,
          relayManifest: {
            expiryMs: 1700111000000,
            hops: [
              {
                relayId: RELAY_ACCOUNT_ID_INPUT,
                hpkePublicKey: Buffer.alloc(32, 0x01),
                weight: 2,
              },
            ],
          },
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  assert.equal(instructions.length, 1);
  const created = instructions[0];
  assert.deepEqual(created.Kaigi.CreateKaigi.call.id, {
    domain_id: "wonderland.sora",
    call_name: "weekly-sync",
  });
  assert.equal(created.Kaigi.CreateKaigi.call.gas_rate_per_minute, 120);
  assert.deepEqual(created.Kaigi.CreateKaigi.call.relay_manifest.hops[0], {
    relay_id: RELAY_ACCOUNT_ID,
    hpke_public_key: "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=",
    weight: 2,
  });
  assert.equal(created.Kaigi.CreateKaigi.commitment, null);
});

test("buildCreateKaigiTransaction preserves privacy artifacts", () => {
  const captures = [];
  const commitment = Buffer.alloc(32, 0x33);
  const nullifier = Buffer.alloc(32, 0x44);
  const proof = Buffer.from([0xfa, 0xce]);
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({
          authority,
          instructions: instructions.map((payload) => JSON.parse(payload)),
        });
        return {
          signed_transaction: Buffer.from([0x40]),
          hash: Buffer.alloc(32, 0x55),
        };
      },
    },
    () =>
      buildCreateKaigiTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        call: {
          id: "wonderland.sora:private-room",
          host: AUTHORITY_ID_INPUT,
          privacyMode: "ZkRosterV1",
          commitment: { commitment, aliasTag: "host" },
          nullifier: { digest: nullifier, issuedAtMs: 12 },
          proof,
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  const [{ instructions }] = captures;
  const created = instructions[0].Kaigi.CreateKaigi;
  assert.equal(created.commitment.commitment, normalizedHashHex(commitment));
  assert.equal(created.nullifier.digest, normalizedHashHex(nullifier));
  assert.equal(created.proof, proof.toString("base64"));
});

test("buildJoinKaigiTransaction normalizes binary fields", () => {
  const commitment = Buffer.alloc(32, 0x77);
  const nullifier = Buffer.alloc(32, 0x22);
  const proof = Buffer.from([0xaa, 0xbb, 0xcc, 0xdd]);
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x41]),
    hash: Buffer.alloc(32, 0x66),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({
          authority,
          instructions: instructions.map((payload) => JSON.parse(payload)),
        });
        return fakeResult;
      },
    },
    () =>
      buildJoinKaigiTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        join: {
          callId: "wonderland.sora:weekly-sync",
          participant: AUTHORITY_ID_INPUT,
          commitment: { commitment, aliasTag: "alice" },
          nullifier: { digest: nullifier, issuedAtMs: 42 },
          proof,
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  const joinInstruction = instructions[0].Kaigi.JoinKaigi;
  assert.equal(joinInstruction.participant, AUTHORITY_ID);
  assert.equal(
    joinInstruction.commitment.commitment,
    normalizedHashHex(commitment),
  );
  assert.equal(joinInstruction.nullifier.digest, normalizedHashHex(nullifier));
  assert.equal(joinInstruction.nullifier.issued_at_ms, 42);
  assert.equal(joinInstruction.proof, proof.toString("base64"));
});

test("buildRegisterKaigiRelayTransaction encodes hpke key", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x42]),
    hash: Buffer.alloc(32, 0x77),
  };
  const relayId = RELAY_ACCOUNT_ID;
  const relayIdInput = RELAY_ACCOUNT_ID_INPUT;
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({
          authority,
          instructions: instructions.map((payload) => JSON.parse(payload)),
        });
        return fakeResult;
      },
    },
    () =>
      buildRegisterKaigiRelayTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        relay: {
          relayId: relayIdInput,
          hpkePublicKey: Buffer.alloc(32, 0xaa),
          bandwidthClass: 6,
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  const relayInstruction = instructions[0].Kaigi.RegisterKaigiRelay;
  assert.equal(relayInstruction.relay.relay_id, relayId);
  assert.equal(
    relayInstruction.relay.hpke_public_key,
    "qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqo=",
  );
  assert.equal(relayInstruction.relay.bandwidth_class, 6);
});

baseTest("buildProposeDeployContractTransaction wraps proposal", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x10]),
    hash: Buffer.alloc(32, 0x10),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({
          authority,
          instructions: instructions.map((payload) => JSON.parse(payload)),
        });
        return fakeResult;
      },
    },
    () =>
      buildProposeDeployContractTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        proposal: {
          contractAddress:
            "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
          codeHash: "aa".repeat(32),
          abiHash: "bb".repeat(32),
          window: { lower: 1, upper: 2 },
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const propose = captures[0].instructions[0].ProposeDeployContract;
  assert.equal(
    propose.contract_address,
    "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
  );
});

baseTest("buildCastZkBallotTransaction encodes ballot", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x11]),
    hash: Buffer.alloc(32, 0x11),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push(JSON.parse(instructions[0]));
        return fakeResult;
      },
    },
    () =>
      buildCastZkBallotTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        ballot: {
          electionId: "ref-1",
          proof: Buffer.alloc(32, 0x01),
          publicInputs: { direction: "Aye" },
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures[0].CastZkBallot.election_id, "ref-1");
});

test("buildCastPlainBallotTransaction normalizes amount", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x12]),
    hash: Buffer.alloc(32, 0x12),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push(JSON.parse(instructions[0]));
        return fakeResult;
      },
    },
    () =>
      buildCastPlainBallotTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        ballot: {
          referendumId: "ref-2",
          owner: AUTHORITY_ID_INPUT,
          amount: "10",
          durationBlocks: 5,
          direction: "aye",
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures[0].CastPlainBallot.direction, 0);
});

test("buildEnactReferendumTransaction wraps enactment", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x13]),
    hash: Buffer.alloc(32, 0x13),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push(JSON.parse(instructions[0]));
        return fakeResult;
      },
    },
    () =>
      buildEnactReferendumTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        enactment: {
          referendumId: Buffer.alloc(32, 0x33),
          preimageHash: Buffer.alloc(32, 0x44),
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.ok(captures[0].EnactReferendum);
});

test("buildFinalizeReferendumTransaction preserves one exact proposal digest", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x14]),
    hash: Buffer.alloc(32, 0x14),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push(JSON.parse(instructions[0]));
        return fakeResult;
      },
    },
    () =>
      buildFinalizeReferendumTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        finalization: {
          referendumId: "55".repeat(32),
          proposalId: Buffer.alloc(32, 0x55),
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.deepEqual(
    captures[0].FinalizeReferendum.proposal_id,
    toByteArray(Buffer.alloc(32, 0x55)),
  );
  assert.equal(captures[0].FinalizeReferendum.referendum_id, "55".repeat(32));
});

baseTest("buildFinalizeReferendumTransaction rejects mismatch before native dispatch", () => {
  assert.throws(
    () =>
      buildFinalizeReferendumTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        finalization: {
          referendumId: "55".repeat(32),
          proposalId: Buffer.alloc(32, 0x56),
        },
        privateKey: PRIVATE_KEY,
      }),
    /referendumId must equal proposalId/,
  );
});

test("buildPersistCouncilForEpochTransaction wraps council", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x15]),
    hash: Buffer.alloc(32, 0x15),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push(JSON.parse(instructions[0]));
        return fakeResult;
      },
    },
    () =>
      buildPersistCouncilForEpochTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        record: {
          epoch: 1,
          members: [AUTHORITY_ID_INPUT],
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures[0].PersistCouncilForEpoch.members.length, 1);
});

test("buildRegisterSmartContractCodeTransaction wraps manifest instruction", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x01]),
    hash: Buffer.alloc(32, 0xbb),
  };
  withNativeBinding(
    {
      buildTransaction: (
        networkId,
        authority,
        instructions,
        metadataPayload,
        creationTimeMs,
        ttlMs,
        nonce,
        secret,
      ) => {
        captures.push({
          networkId,
          authority,
          instructions,
          metadataPayload,
          creationTimeMs,
          ttlMs,
          nonce,
          secret,
        });
        return fakeResult;
      },
    },
    () => {
      const result = buildRegisterSmartContractCodeTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        manifest: {
          codeHash: Buffer.alloc(32, 0xaa),
          compilerFingerprint: "rustc",
        },
        privateKey: PRIVATE_KEY,
      });
      assert.ok(Buffer.isBuffer(result.hash));
    },
  );
  assert.equal(captures.length, 1);
  const parsed = JSON.parse(captures[0].instructions[0]);
  assert.equal(
    parsed.RegisterSmartContractCode.manifest.compiler_fingerprint,
    "rustc",
  );
});

test("buildRegisterSmartContractBytesTransaction encodes code payload", () => {
  const codeBytes = Buffer.from([0xde, 0xad]);
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x02]),
    hash: Buffer.alloc(32, 0xcc),
  };
  withNativeBinding(
    {
      buildTransaction: (
        networkId,
        authority,
        instructions,
        metadataPayload,
        creationTimeMs,
        ttlMs,
        nonce,
        secret,
      ) => {
        captures.push({
          networkId,
          authority,
          instructions,
          metadataPayload,
          creationTimeMs,
          ttlMs,
          nonce,
          secret,
        });
        return fakeResult;
      },
    },
    () => {
      const result = buildRegisterSmartContractBytesTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        codeHash: Buffer.alloc(32, 0xdd),
        code: codeBytes,
        privateKey: PRIVATE_KEY,
      });
      assert.ok(Buffer.isBuffer(result.signedTransaction));
    },
  );
  const parsed = JSON.parse(captures[0].instructions[0]);
  assert.equal(
    parsed.RegisterSmartContractBytes.code,
    codeBytes.toString("base64"),
  );
});

test("buildRemoveSmartContractBytesTransaction wraps removal payload", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x04]),
    hash: Buffer.alloc(32, 0xee),
  };
  withNativeBinding(
    {
      buildTransaction: (...args) => {
        captures.push(args[2]);
        return fakeResult;
      },
    },
    () => {
      buildRemoveSmartContractBytesTransaction({
        networkId: NETWORK_ID,
        authority: AUTHORITY_ID_INPUT,
        feePayment: AUTHORITY_FEE_PAYMENT,
        codeHash: Buffer.alloc(32, 0xaa),
        reason: "cleanup",
        privateKey: PRIVATE_KEY,
      });
    },
  );
  const parsed = JSON.parse(captures[0][0]);
  assert.equal(parsed.RemoveSmartContractBytes.reason, "cleanup");
});

baseTest("confidential proof builders preserve exact canonical native results", () => {
  const backend = "halo2/ipa";
  const verifyingKey = {
    id: { backend },
    record: {
      circuit_id: "confidential-transfer-v2",
      backend,
      inline_key: {
        backend,
        bytes_b64: Buffer.from([1, 2, 3]).toString("base64"),
      },
    },
  };
  const spendKey = Buffer.alloc(32, 0x42);
  const rhoHex = Buffer.alloc(32, 0x51).toString("hex");
  const diversifierHex = Buffer.alloc(32, 0x52).toString("hex");
  const ownerTagHex = Buffer.alloc(32, 0x53).toString("hex");
  const rootHintHex = Buffer.alloc(32, 0x54).toString("hex");
  const treeCommitment = Buffer.alloc(32, 0x55);
  const input = {
    amount: "7",
    rhoHex,
    diversifierHex,
    leafIndex: 0,
  };
  const transferOutput = { amount: "7", rhoHex, ownerTagHex };
  const unshieldOutput = { amount: "1", rhoHex };
  const nullifier = Buffer.alloc(32, 0x61);
  const outputCommitment = Buffer.alloc(32, 0x62);
  const root = Buffer.alloc(32, 0x63);
  const proof = Buffer.from([0x64]);
  const calls = [];

  withNativeBinding(
    {
      buildConfidentialTransferProofV2: (...args) => {
        calls.push(["transfer", args]);
        return {
          nullifiers: [nullifier],
          outputCommitments: [outputCommitment],
          root,
          proof,
        };
      },
      buildConfidentialUnshieldProofV2: (...args) => {
        calls.push(["unshield-v2", args]);
        return { nullifiers: [nullifier], root, proof };
      },
      buildConfidentialUnshieldProofV3: (...args) => {
        calls.push(["unshield-v3", args]);
        return {
          nullifiers: [nullifier],
          outputCommitments: [outputCommitment],
          root,
          proof,
        };
      },
    },
    () => {
      const transfer = buildConfidentialTransferProofV2({
        networkId: NETWORK_ID,
        assetDefinitionId: ASSET_DEFINITION_ID,
        spendKey,
        treeCommitments: [treeCommitment],
        inputs: [input],
        outputs: [transferOutput],
        rootHintHex,
        verifyingKey,
      });
      assert.deepEqual(transfer, {
        nullifiers: [nullifier],
        root,
        proof,
        outputCommitments: [outputCommitment],
      });

      const unshieldV2 = buildConfidentialUnshieldProofV2({
        networkId: NETWORK_ID,
        assetDefinitionId: ASSET_DEFINITION_ID,
        spendKey,
        treeCommitments: [treeCommitment],
        inputs: [input],
        publicAmount: "7",
        rootHintHex,
        verifyingKey,
      });
      assert.deepEqual(unshieldV2, { nullifiers: [nullifier], root, proof });

      const unshieldV3 = buildConfidentialUnshieldProofV3({
        networkId: NETWORK_ID,
        assetDefinitionId: ASSET_DEFINITION_ID,
        spendKey,
        treeCommitments: [treeCommitment],
        inputs: [input],
        outputs: [unshieldOutput],
        publicAmount: "6",
        rootHintHex,
        verifyingKey,
      });
      assert.deepEqual(unshieldV3, {
        nullifiers: [nullifier],
        root,
        proof,
        outputCommitments: [outputCommitment],
      });

      buildConfidentialUnshieldProofV3({
        networkId: NETWORK_ID,
        assetDefinitionId: ASSET_DEFINITION_ID,
        spendKey,
        treeCommitments: [treeCommitment],
        inputs: [input],
        publicAmount: "7",
        rootHintHex,
        verifyingKey,
      });
    },
  );

  assert.equal(calls.length, 4);
  assert.deepEqual(calls[0][1][3], [treeCommitment.toString("hex")]);
  assert.deepEqual(calls[0][1][4], [input]);
  assert.deepEqual(calls[0][1][5], [transferOutput]);
  assert.equal(calls[1][0], "unshield-v2");
  assert.equal(calls[1][1][9].toString("base64"), "AQID");
  assert.deepEqual(calls[2][1][5], [unshieldOutput]);
  assert.deepEqual(calls[3][1][5], []);
});

baseTest("retired generic confidential transaction builders are not exported", () => {
  for (const parts of [["Shi", "eld"], ["Zk", "Transfer"], ["Un", "shield"]]) {
    const exportedName = ["build", parts.join(""), "Transaction"].join("");
    assert.equal(transactionExports[exportedName], undefined, exportedName);
  }
});

test("supported confidential transaction builders wrap expected instruction payloads", () => {
  const proof = {
    backend: "halo2/ipa",
    proof: Buffer.from("proof"),
    verifyingKeyRef: { backend: "halo2/ipa", name: "vk_governance" },
  };
  const register = captureInstructionObject(() =>
    buildRegisterZkAssetTransaction({
      networkId: NETWORK_ID,
      authority: AUTHORITY_ID_INPUT,
      feePayment: AUTHORITY_FEE_PAYMENT,
      registration: {
        assetDefinitionId: ASSET_DEFINITION_ID,
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(register.zk?.RegisterZkAsset);

  const policy = captureInstructionObject(() =>
    buildScheduleConfidentialPolicyTransitionTransaction({
      networkId: NETWORK_ID,
      authority: AUTHORITY_ID_INPUT,
      feePayment: AUTHORITY_FEE_PAYMENT,
      transition: {
        assetDefinitionId: ASSET_DEFINITION_ID,
        newMode: "TransparentOnly",
        effectiveHeight: 5,
        transitionId: Buffer.alloc(32, 0xaa),
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(policy.zk?.ScheduleConfidentialPolicyTransition);

  const cancel = captureInstructionObject(() =>
    buildCancelConfidentialPolicyTransitionTransaction({
      networkId: NETWORK_ID,
      authority: AUTHORITY_ID_INPUT,
      feePayment: AUTHORITY_FEE_PAYMENT,
      cancellation: {
        assetDefinitionId: ASSET_DEFINITION_ID,
        transitionId: Buffer.alloc(32, 0xbb),
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(cancel.zk?.CancelConfidentialPolicyTransition);

  const election = captureInstructionObject(() =>
    buildCreateElectionTransaction({
      networkId: NETWORK_ID,
      authority: AUTHORITY_ID_INPUT,
      feePayment: AUTHORITY_FEE_PAYMENT,
      election: {
        electionId: "election-1",
        options: 2,
        eligibleRoot: Buffer.alloc(32, 0x05),
        startTs: 1,
        endTs: 2,
        ballotVerifyingKey: "halo2/ipa:vk_ballot",
        tallyVerifyingKey: { backend: "halo2/ipa", name: "vk_tally" },
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(election.zk?.CreateElection);

  const ballot = captureInstructionObject(() =>
    buildSubmitBallotTransaction({
      networkId: NETWORK_ID,
      authority: AUTHORITY_ID_INPUT,
      feePayment: AUTHORITY_FEE_PAYMENT,
      ballot: {
        electionId: "election-1",
        ciphertext: Buffer.from("encrypted"),
        ballotProof: proof,
        nullifier: Buffer.alloc(32, 0x06),
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(ballot.zk?.SubmitBallot);

  const finalize = captureInstructionObject(() =>
    buildFinalizeElectionTransaction({
      networkId: NETWORK_ID,
      authority: AUTHORITY_ID_INPUT,
      feePayment: AUTHORITY_FEE_PAYMENT,
      finalization: {
        electionId: "election-1",
        tally: [1n],
        tallyProof: proof,
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(finalize.zk?.FinalizeElection);
});

function captureInstructionObject(buildFn) {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0xff]),
    hash: Buffer.alloc(32, 0xff),
  };
  withNativeBinding(
    {
      buildTransaction: (
        networkId,
        authority,
        instructions,
        metadataPayload,
        creationTimeMs,
        ttlMs,
        nonce,
        secret,
      ) => {
        captures.push({
          networkId,
          authority,
          instructions: instructions.map((payload) => JSON.parse(payload)),
          metadataPayload,
          creationTimeMs,
          ttlMs,
          nonce,
          secret,
        });
        return fakeResult;
      },
    },
    buildFn,
  );
  return captures[0].instructions[0];
}

function withNativeBinding(binding, fn) {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = binding;
  try {
    return fn();
  } finally {
    globalThis.__IROHA_NATIVE_BINDING__ = previous;
  }
}

async function withNativeBindingAsync(binding, fn) {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = binding;
  try {
    return await fn();
  } finally {
    globalThis.__IROHA_NATIVE_BINDING__ = previous;
  }
}
