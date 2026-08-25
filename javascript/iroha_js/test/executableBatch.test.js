import assert from "node:assert/strict";
import test from "node:test";

import { ed25519 } from "@noble/curves/ed25519";

import { AccountAddress } from "../src/address.js";
import { buildCancelSmartContractCodeUploadInstruction } from "../src/instructionBuilders.js";
import { getNativeBinding } from "../src/native.js";
import { NetworkId } from "../src/networkId.js";
import { feePaymentIntentToNoritoJson } from "../src/transaction.js";
import {
  browserTransactionPayloadHashHex,
  buildBrowserExecutableBatchPayload,
  buildBrowserInstructionTransactionPayload,
  finalizeBrowserExecutableBatchTransaction,
  validateBrowserExecutableBatchSignable,
} from "../src/transactionCodec.js";

const PRIVATE_KEY = Buffer.from(
  "CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53",
  "hex",
);
const PUBLIC_KEY = Buffer.from(ed25519.getPublicKey(PRIVATE_KEY));
const AUTHORITY = AccountAddress.fromAccount({
  algorithm: "ed25519",
  publicKey: PUBLIC_KEY,
}).toI105(753);
const CONTRACT_ADDRESS =
  "irohac1qyqqqqqqqqqqqq9rdnnncuwseflztqwhmppl0fyvc37w8gq9q6pxl";
const NETWORK_ID = NetworkId.parse(
  "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149",
);
const INVALID_CONTRACT_ADDRESSES = Object.freeze([
  "abc",
  ` ${CONTRACT_ADDRESS}`,
  CONTRACT_ADDRESS.toUpperCase(),
  `${CONTRACT_ADDRESS.slice(0, -1)}p`,
  "irohac1qyqqqqqqqqqqqzgfpg9scrgwpugpzysnzs23v9ccrydpk8q7ca9ly",
  "irohac1qgqqqqqqqqqqqzgfpg9scrgwpugpzysnzs23v9ccrydpk8qhk43nl",
  "irohac1qyqqqqqqqqqqqzgfpg9scrgwpugpzysnzs23v9ccrydpkk75nd5",
  "irohac1qyqqqqqqqqqqqzgfpg9scrgwpugpzysnzs23v9ccrydpk8p2lc7wy",
]);

function readField(input, offset) {
  let length = 0n;
  let shift = 0n;
  let cursor = offset;
  for (;;) {
    assert.ok(cursor < input.length, "field length must not be truncated");
    const byte = input[cursor];
    cursor += 1;
    length |= BigInt(byte & 0x7f) << shift;
    if ((byte & 0x80) === 0) break;
    shift += 7n;
  }
  assert.ok(length <= BigInt(Number.MAX_SAFE_INTEGER));
  const end = cursor + Number(length);
  assert.ok(end <= input.length, "field payload must not be truncated");
  return { value: input.subarray(cursor, end), next: end };
}

function payloadExecutable(payload) {
  let offset = 0;
  for (let index = 0; index < 4; index += 1) {
    const decoded = readField(payload, offset);
    offset = decoded.next;
    if (index === 3) return decoded.value;
  }
  throw new Error("payload executable field is missing");
}

function batchEntryValues(executable) {
  assert.equal(executable.readUInt32LE(0), 4);
  const vectorField = readField(executable, 4);
  assert.equal(vectorField.next, executable.length);
  const vector = vectorField.value;
  const count = Number(vector.readBigUInt64LE(0));
  const entries = [];
  let offset = 8;
  for (let index = 0; index < count; index += 1) {
    const decoded = readField(vector, offset);
    entries.push(decoded.value);
    offset = decoded.next;
  }
  assert.equal(offset, vector.length);
  return entries;
}

test("browser executable batch preserves mixed order, tag, and copied bytes", () => {
  const hash = Buffer.alloc(32, 0x41);
  const argumentsBytes = Uint8Array.from([0x4b, 0x4f, 0x54, 0x4f]);
  const instruction = buildCancelSmartContractCodeUploadInstruction({
    codeHash: hash.toString("hex"),
  });
  const input = {
    networkId: NETWORK_ID,
    authority: AUTHORITY,
    chainDiscriminant: 753,
    entries: [
      { kind: "instruction", instruction },
      {
        kind: "contractCall",
        contractAddress: CONTRACT_ADDRESS,
        expectedCodeHash: hash,
        entrypoint: "run",
        arguments: argumentsBytes,
      },
      { kind: "instruction", instruction },
    ],
    feePayment: {
      payer: "authority",
      chargeLimits: [],
      gasLimit: 10_000,
    },
    creationTimeMs: 123_456,
    nonce: 7,
  };

  const payload = buildBrowserExecutableBatchPayload(input);
  hash.fill(0);
  argumentsBytes.fill(0);

  const entries = batchEntryValues(payloadExecutable(payload));
  assert.deepEqual(entries.map((entry) => entry.readUInt32LE(0)), [0, 1, 0]);
  const invocationField = readField(entries[1], 4);
  assert.equal(invocationField.next, entries[1].length);
  const invocation = invocationField.value;
  const address = readField(invocation, 0);
  const expectedHash = readField(invocation, address.next);
  const entrypoint = readField(invocation, expectedHash.next);
  const argumentsOption = readField(invocation, entrypoint.next);
  assert.deepEqual(expectedHash.value, Buffer.alloc(32, 0x41));
  assert.equal(argumentsOption.value[0], 1);
  const argumentRecord = readField(argumentsOption.value, 1);
  assert.equal(argumentRecord.value.readBigUInt64LE(0), 4n);
  assert.deepEqual(
    argumentRecord.value.subarray(8),
    Buffer.from([0x4b, 0x4f, 0x54, 0x4f]),
  );

  const hashHex = browserTransactionPayloadHashHex(payload);
  const signable = validateBrowserExecutableBatchSignable({
    networkId: NETWORK_ID,
    payloadBytes: payload,
    payloadHashHex: hashHex,
    authority: AUTHORITY,
    signingPublicKey: PUBLIC_KEY,
    signatureAlgorithm: "ed25519",
  });
  const signature = Buffer.from(
    ed25519.sign(Buffer.from(hashHex, "hex"), PRIVATE_KEY),
  );
  const finalized = finalizeBrowserExecutableBatchTransaction(
    signable,
    signature,
    PUBLIC_KEY,
  );
  assert.equal(finalized.signedTransaction[0], 1);
});

test("canonical browser instruction transactions use native-instruction tag zero", () => {
  const instruction = buildCancelSmartContractCodeUploadInstruction({
    codeHash: "41".repeat(32),
  });
  const payload = buildBrowserInstructionTransactionPayload({
    networkId: NETWORK_ID,
    authority: AUTHORITY,
    chainDiscriminant: 753,
    instructions: [instruction],
    feePayment: { payer: "authority", chargeLimits: [] },
    creationTimeMs: 123_456,
  });
  assert.equal(payloadExecutable(payload).readUInt32LE(0), 0);
});

test("browser executable batch bytes match the native Rust builder", () => {
  const expectedCodeHash = Buffer.alloc(32, 0x41);
  const argumentsBytes = Buffer.from([0x4b, 0x4f, 0x54, 0x4f]);
  const instruction = buildCancelSmartContractCodeUploadInstruction({
    codeHash: expectedCodeHash.toString("hex"),
  });
  const entries = [
    { kind: "instruction", instruction },
    {
      kind: "contractCall",
      contractAddress: CONTRACT_ADDRESS,
      expectedCodeHash,
      entrypoint: "run",
      arguments: argumentsBytes,
    },
    { kind: "instruction", instruction },
  ];
  const feePayment = {
    payer: "authority",
    chargeLimits: [],
    gasLimit: 10_000,
  };
  const browser = buildBrowserExecutableBatchPayload({
    networkId: NETWORK_ID,
    authority: AUTHORITY,
    chainDiscriminant: 753,
    entries,
    feePayment,
    creationTimeMs: 123_456,
    nonce: 7,
  });
  const native = getNativeBinding();
  assert.equal(typeof native.buildExecutableBatchTransactionPayload, "function");
  const nativePayload = native.buildExecutableBatchTransactionPayload(
    NETWORK_ID.toBytes(),
    AUTHORITY,
    entries.map((entry) =>
      entry.kind === "instruction"
        ? JSON.stringify(entry)
        : JSON.stringify({
            ...entry,
            expectedCodeHash: expectedCodeHash.toString("hex").toUpperCase(),
            arguments: Array.from(argumentsBytes),
          }),
    ),
    feePaymentIntentToNoritoJson(feePayment),
    null,
    123_456,
    null,
    7,
  );
  assert.deepEqual(browser, Buffer.from(nativePayload.payloadBytes));
});

test("browser executable batch rejects invalid calls before encoding", () => {
  const instruction = buildCancelSmartContractCodeUploadInstruction({
    codeHash: "41".repeat(32),
  });
  const base = {
    networkId: NETWORK_ID,
    authority: AUTHORITY,
    chainDiscriminant: 753,
    entries: [
      { kind: "instruction", instruction },
      {
        kind: "contractCall",
        contractAddress: CONTRACT_ADDRESS,
        expectedCodeHash: Buffer.alloc(32, 0x41),
        entrypoint: "run",
      },
    ],
    feePayment: { payer: "authority", chargeLimits: [], gasLimit: 1_000 },
  };
  assert.throws(
    () =>
      buildBrowserExecutableBatchPayload({
        ...base,
        feePayment: { payer: "authority", chargeLimits: [] },
      }),
    /gasLimit is required/u,
  );
  assert.throws(
    () =>
      buildBrowserExecutableBatchPayload({
        ...base,
        entries: [
          base.entries[0],
          { ...base.entries[1], expectedCodeHash: Buffer.alloc(31) },
        ],
      }),
    /exactly 32 bytes/u,
  );
  assert.throws(
    () =>
      buildBrowserExecutableBatchPayload({
        ...base,
        entries: [base.entries[0], { ...base.entries[1], entrypoint: "" }],
      }),
    /non-empty exact string/u,
  );
  assert.throws(
    () =>
      buildBrowserExecutableBatchPayload({
        ...base,
        entries: [
          base.entries[0],
          {
            ...base.entries[1],
            arguments: new Uint8Array(1024 * 1024 + 1),
          },
        ],
      }),
    /exceeds 1048576 bytes/u,
  );
  for (const contractAddress of INVALID_CONTRACT_ADDRESSES) {
    assert.throws(
      () =>
        buildBrowserExecutableBatchPayload({
          ...base,
          entries: [
            base.entries[0],
            { ...base.entries[1], contractAddress },
          ],
        }),
      /contractAddress/u,
    );
  }
});

test("external executable batch validation rejects a noncanonical address", () => {
  const instruction = buildCancelSmartContractCodeUploadInstruction({
    codeHash: "41".repeat(32),
  });
  const payload = buildBrowserExecutableBatchPayload({
    networkId: NETWORK_ID,
    authority: AUTHORITY,
    chainDiscriminant: 753,
    entries: [
      { kind: "instruction", instruction },
      {
        kind: "contractCall",
        contractAddress: CONTRACT_ADDRESS,
        expectedCodeHash: Buffer.alloc(32, 0x41),
        entrypoint: "run",
      },
    ],
    feePayment: { payer: "authority", chargeLimits: [], gasLimit: 1_000 },
    creationTimeMs: 123_456,
  });
  const tampered = Buffer.from(payload);
  const addressOffset = tampered.indexOf(Buffer.from(CONTRACT_ADDRESS, "utf8"));
  assert.notEqual(addressOffset, -1);
  Buffer.from(CONTRACT_ADDRESS.toUpperCase(), "utf8").copy(
    tampered,
    addressOffset,
  );

  assert.throws(
    () =>
      validateBrowserExecutableBatchSignable({
        networkId: NETWORK_ID,
        payloadBytes: tampered,
        payloadHashHex: browserTransactionPayloadHashHex(tampered),
        authority: AUTHORITY,
        signingPublicKey: PUBLIC_KEY,
        signatureAlgorithm: "ed25519",
      }),
    (error) => error?.code === "malformed_payload",
  );
});
