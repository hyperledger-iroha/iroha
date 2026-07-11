import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  NexusAppClient,
  NexusAppError,
  nexusPayloadHashHex,
} from "../src/nexusApp.js";
import { finalizeBrowserSignedTransaction } from "../src/transactionCodec.js";

const fixture = JSON.parse(
  readFileSync(
    new URL("../../../fixtures/sdk/nexus_connect_transfer_v1.json", import.meta.url),
    "utf8",
  ),
);
const fixturePayloadBytes = Buffer.from(fixture.expected.payload_bytes_hex, "hex");
const fixturePublicKey = Buffer.from(
  fixture.connect.approval_frame.signing_public_key_hex,
  "hex",
);
const fixtureWalletSignature = Buffer.from(
  fixture.expected.wallet_signature_hex,
  "hex",
);
const fixtureFinalized = finalizeBrowserSignedTransaction(
  {
    payloadBytes: fixturePayloadBytes,
    payloadHashHex: fixture.expected.payload_hash_hex,
    authority: fixture.transfer_input.authority,
    signingPublicKey: fixturePublicKey,
    signatureAlgorithm: "ed25519",
  },
  fixtureWalletSignature,
  fixturePublicKey,
);
const fixtureSignedTransaction = fixtureFinalized.signedTransaction;
const fixtureSignedTransactionHashHex = fixtureFinalized.hashHex;
const unsupportedSignatureAlgorithms = [
  "secp256k1",
  "",
  " ",
  " Ed25519",
  "Ed25519 ",
  "\tEd25519",
  "Ed25519\n",
  "ed25519 ",
  " ed25519",
  "\ted25519",
  "ed25519\u00A0",
  "0 ",
  " 0",
  "\t0",
  "00",
  "\uFF10",
  "ED25519",
  "Ed25519",
  "ed\t25519",
  "ed\u000025519",
  "ed\u001F25519",
  "ed\u007F25519",
  "ed\u200B25519",
  "\u00A0Ed25519",
  "Ed25519\u00A0",
  "\u0435d25519",
  "ed\uFF0D25519",
  1,
  false,
  Buffer.from("ed25519"),
  ["ed25519"],
];

function fixtureSignable(overrides = {}) {
  return {
    payloadBytes: Buffer.from(fixturePayloadBytes),
    payloadHashHex: fixture.expected.payload_hash_hex,
    authority: fixture.transfer_input.authority,
    signingPublicKey: Buffer.from(fixturePublicKey),
    signatureAlgorithm: "ed25519",
    ...overrides,
  };
}

function draftClient(transactionCodec) {
  return new NexusAppClient({
    chainId: "test-chain",
    authority: "approved-account-i105",
    signingPublicKey: fixturePublicKey,
    transactionCodec,
  });
}

function finalizationHarness(finalizedResult, submission = { accepted: true }) {
  const calls = { finalized: 0, submitted: 0, waited: 0 };
  const client = new NexusAppClient({
    signingPublicKey: fixturePublicKey,
    transactionCodec: {
      finalizeSignedTransaction() {
        calls.finalized += 1;
        return typeof finalizedResult === "function"
          ? finalizedResult()
          : finalizedResult;
      },
    },
    toriiClient: {
      async submitTransaction() {
        calls.submitted += 1;
        return typeof submission === "function" ? submission() : submission;
      },
      async waitForTransactionStatus() {
        calls.waited += 1;
        return { status: "Applied" };
      },
    },
  });
  return { client, calls };
}

test("NexusAppClient builds a signable transfer draft", () => {
  const payloadBytes = Buffer.from("canonical-transfer-payload");
  const client = new NexusAppClient({
    chainId: "test-chain",
    authority: "account-i105",
    signingPublicKey: Buffer.alloc(32, 1),
    transactionCodec: {
      buildTransferPayload(input) {
        assert.equal(input.chainId, "test-chain");
        assert.equal(input.authority, "account-i105");
        assert.equal(input.destinationAccountId, "destination-i105");
        return payloadBytes;
      },
    },
  });

  const draft = client.buildTransferDraft({
    sourceAssetHoldingId: "asset#account-i105",
    quantity: "12.50",
    destinationAccountId: "destination-i105",
  });

  assert.deepEqual(draft.signable.payloadBytes, payloadBytes);
  assert.equal(
    draft.signable.payloadHashHex,
    nexusPayloadHashHex(payloadBytes),
  );
});

test("NexusAppClient payload hashing matches the shared Nexus fixture", () => {
  const payloadBytes = Buffer.from(fixture.expected.payload_bytes_hex, "hex");
  assert.equal(
    nexusPayloadHashHex(payloadBytes),
    fixture.expected.payload_hash_hex,
  );

  const client = new NexusAppClient({
    chainId: fixture.transfer_input.chain_id,
    authority: fixture.transfer_input.authority,
    signingPublicKey: fixture.connect.approval_frame.signing_public_key_hex,
    transactionCodec: {
      buildTransferPayload(input) {
        assert.equal(
          input.sourceAssetHoldingId,
          fixture.transfer_input.source_asset_id,
        );
        assert.equal(
          input.destinationAccountId,
          fixture.transfer_input.destination_account_id,
        );
        assert.equal(input.quantity, fixture.transfer_input.quantity);
        return payloadBytes;
      },
    },
  });

  const draft = client.buildTransferDraft({
    sourceAssetHoldingId: fixture.transfer_input.source_asset_id,
    quantity: fixture.transfer_input.quantity,
    destinationAccountId: fixture.transfer_input.destination_account_id,
    creationTimeMs: fixture.transfer_input.creation_time_ms,
    ttlMs: fixture.transfer_input.ttl_ms,
    nonce: fixture.transfer_input.nonce,
    metadata: fixture.transfer_input.metadata,
  });

  assert.equal(
    draft.signable.payloadHashHex,
    fixture.expected.payload_hash_hex,
  );
  assert.deepEqual(draft.signable.payloadBytes, payloadBytes);
});

test("NexusAppClient default browser codec reproduces the shared Nexus fixture", () => {
  const client = new NexusAppClient({
    chainId: fixture.transfer_input.chain_id,
    authority: fixture.transfer_input.authority,
    signingPublicKey: fixture.connect.approval_frame.signing_public_key_hex,
  });
  const draft = client.buildTransferDraft({
    sourceAssetHoldingId: fixture.transfer_input.source_asset_id,
    quantity: fixture.transfer_input.quantity,
    destinationAccountId: fixture.transfer_input.destination_account_id,
    creationTimeMs: fixture.transfer_input.creation_time_ms,
    ttlMs: fixture.transfer_input.ttl_ms,
    nonce: fixture.transfer_input.nonce,
    metadata: fixture.transfer_input.metadata,
  });

  assert.deepEqual(draft.signable.payloadBytes, fixturePayloadBytes);
  assert.equal(draft.signable.payloadHashHex, fixture.expected.payload_hash_hex);
});

test("NexusAppClient verifies custom payload bytes and hash aliases", () => {
  const expectedHash = fixture.expected.payload_hash_hex;
  const positive = draftClient({
    buildTransferPayload() {
      return {
        payloadBytes: fixturePayloadBytes,
        payload_bytes: Uint8Array.from(fixturePayloadBytes),
        payloadHashHex: expectedHash,
        hash: Buffer.from(expectedHash, "hex"),
      };
    },
    payloadHashHex() {
      return expectedHash;
    },
  }).buildTransferDraft({
    sourceAssetHoldingId: "asset#approved-account-i105",
    quantity: "1",
    destinationAccountId: "destination-i105",
  });
  assert.deepEqual(positive.signable.payloadBytes, fixturePayloadBytes);
  assert.equal(positive.signable.payloadHashHex, expectedHash);

  const conflictingBytes = Buffer.from(fixturePayloadBytes);
  conflictingBytes[0] ^= 0xff;
  for (const [codec, code] of [
    [
      {
        buildTransferPayload() {
          return {
            payloadBytes: fixturePayloadBytes,
            payload_bytes: conflictingBytes,
          };
        },
      },
      "invalid_payload",
    ],
    [
      {
        buildTransferPayload() {
          return {
            payloadBytes: fixturePayloadBytes,
            payloadHashHex: expectedHash,
            hash_hex: "d".repeat(64),
          };
        },
      },
      "payload_hash_mismatch",
    ],
    [
      {
        buildTransferPayload() {
          return fixturePayloadBytes;
        },
        payloadHashHex() {
          return "d".repeat(64);
        },
      },
      "payload_hash_mismatch",
    ],
  ]) {
    assert.throws(
      () =>
        draftClient(codec).buildTransferDraft({
          sourceAssetHoldingId: "asset#approved-account-i105",
          quantity: "1",
          destinationAccountId: "destination-i105",
        }),
      (error) => error instanceof NexusAppError && error.code === code,
    );
  }

  for (const malformed of [
    "a".repeat(63),
    "A".repeat(64),
    `0x${"a".repeat(64)}`,
    ` ${"a".repeat(64)}`,
    "g".repeat(64),
  ]) {
    assert.throws(
      () =>
        draftClient({
          buildTransferPayload() {
            return fixturePayloadBytes;
          },
          payloadHashHex() {
            return malformed;
          },
        }).buildTransferDraft({
          sourceAssetHoldingId: "asset#approved-account-i105",
          quantity: "1",
          destinationAccountId: "destination-i105",
        }),
      (error) => error instanceof NexusAppError && error.code === "invalid_payload_hash",
    );
  }
});

test("NexusAppClient bounds and validates custom payload byte containers before copying", () => {
  const oversizedArray = [];
  oversizedArray.length = 1024 * 1024 + 1;
  const malformedArrays = [
    [-1],
    [256],
    [1.5],
    Object.assign([1], { custom: true }),
    Array(1),
    oversizedArray,
  ];
  for (const payloadBytes of malformedArrays) {
    assert.throws(
      () =>
        draftClient({
          buildTransferPayload() {
            return payloadBytes;
          },
        }).buildTransferDraft({
          sourceAssetHoldingId: "asset#approved-account-i105",
          quantity: "1",
          destinationAccountId: "destination-i105",
        }),
      TypeError,
    );
  }
  for (const payloadBytes of [
    new ArrayBuffer(1024 * 1024 + 1),
    new Uint8Array(new ArrayBuffer(1024 * 1024 + 1)),
  ]) {
    assert.throws(
      () =>
        draftClient({
          buildTransferPayload() {
            return payloadBytes;
          },
        }).buildTransferDraft({
          sourceAssetHoldingId: "asset#approved-account-i105",
          quantity: "1",
          destinationAccountId: "destination-i105",
        }),
      TypeError,
    );
  }
});

test("NexusAppClient runs connect approval, wallet signature, finalize, submit, wait", async () => {
  const payloadBytes = fixturePayloadBytes;
  const walletSignature = fixtureWalletSignature;
  const signedTransaction = fixtureSignedTransaction;
  const hashHex = fixtureSignedTransactionHashHex;
  const submitted = [];
  const waited = [];
  const requested = [];

  const client = new NexusAppClient({
    chainId: "test-chain",
    signingPublicKey: fixturePublicKey,
    connectTransport: {
      startConnect(options) {
        return {
          sid: options.sid,
          walletLaunchUri: `iroha://connect?sid=${options.sid}`,
        };
      },
      awaitApproval() {
        return {
          accountId: "approved-account-i105",
        };
      },
      requestSignature(_session, signable) {
        requested.push(Buffer.from(signable.payloadBytes));
        return { algorithm: "ed25519", signature: walletSignature };
      },
    },
    transactionCodec: {
      buildTransferPayload(input) {
        assert.equal(input.authority, "approved-account-i105");
        return payloadBytes;
      },
      finalizeSignedTransaction(signable, signature, signingPublicKey) {
        assert.equal(signable.payloadHashHex, nexusPayloadHashHex(payloadBytes));
        assert.deepEqual(signature.signature, walletSignature);
        assert.deepEqual(signingPublicKey, fixturePublicKey);
        return { signedTransaction, hashHex };
      },
    },
    toriiClient: {
      async submitTransaction(payload) {
        submitted.push(Buffer.from(payload));
        return { accepted: true };
      },
      async waitForTransactionStatus(txHashHex) {
        waited.push(txHashHex);
        return { status: "Applied" };
      },
    },
  });

  const session = await client.startConnect({ sid: "sid-1" });
  const approval = await client.awaitApproval(session);
  const receipt = await client.transferWithWallet(
    approval.session,
    {
      sourceAssetHoldingId: "asset#approved-account-i105",
      quantity: "1",
      destinationAccountId: "destination-i105",
    },
    { timeoutMs: 1 },
  );

  assert.equal(receipt.signedTransactionHashHex, hashHex);
  assert.deepEqual(receipt.signedTransaction, signedTransaction);
  assert.deepEqual(requested, [payloadBytes]);
  assert.deepEqual(submitted, [signedTransaction]);
  assert.deepEqual(waited, [hashHex]);
});

test("NexusAppClient accepts raw wallet signature byte inputs", async () => {
  const payloadBytes = fixturePayloadBytes;
  const signable = {
    payloadBytes,
    payloadHashHex: nexusPayloadHashHex(payloadBytes),
    authority: "account-i105",
    signingPublicKey: fixturePublicKey,
    signatureAlgorithm: "ed25519",
  };
  const walletSignature = fixtureWalletSignature;
  const signedTransaction = fixtureSignedTransaction;
  const hashHex = fixtureSignedTransactionHashHex;
  let finalizedSignature = null;
  let submittedPayload = null;

  const client = new NexusAppClient({
    signingPublicKey: fixturePublicKey,
    connectTransport: {
      requestSignature() {
        return new Uint8Array(walletSignature);
      },
    },
    transactionCodec: {
      finalizeSignedTransaction(_signable, signature) {
        finalizedSignature = Buffer.from(signature.signature);
        return { signedTransaction, hashHex };
      },
    },
    toriiClient: {
      async submitTransaction(payload) {
        submittedPayload = Buffer.from(payload);
        return { hashHex };
      },
    },
  });

  const requestedSignature = await client.requestSignature({ sid: "sid-1" }, signable);
  assert.deepEqual(requestedSignature.signature, walletSignature);

  const receipt = await client.finalizeAndSubmit(signable, walletSignature, { wait: false });

  assert.deepEqual(finalizedSignature, walletSignature);
  assert.deepEqual(receipt.signedTransaction, signedTransaction);
  assert.deepEqual(submittedPayload, signedTransaction);
});

test("NexusAppClient independently verifies finalized bytes and hash aliases", async () => {
  const canonicalHash = fixtureSignedTransactionHashHex;
  const positiveResult = {
    signedTransaction: fixtureSignedTransaction,
    signed_transaction: Uint8Array.from(fixtureSignedTransaction),
    bytes: fixtureSignedTransaction.toString("hex"),
    hashHex: canonicalHash,
    hash: Buffer.from(canonicalHash, "hex"),
    signedTransactionHashHex: canonicalHash,
  };
  const positive = finalizationHarness(positiveResult);
  const receipt = await positive.client.finalizeAndSubmit(
    fixtureSignable(),
    fixtureWalletSignature,
    { wait: false },
  );
  assert.deepEqual(receipt.signedTransaction, fixtureSignedTransaction);
  assert.equal(receipt.signedTransactionHashHex, canonicalHash);
  assert.deepEqual(positive.calls, { finalized: 1, submitted: 1, waited: 0 });

  const conflictingBytes = Buffer.from(fixtureSignedTransaction);
  conflictingBytes[0] ^= 0xff;
  const oversizedSigned = [];
  oversizedSigned.length = 1024 * 1024 + 4097;
  const cases = [
    [fixtureSignedTransaction, "invalid_transaction_hash"],
    [{ signedTransaction: fixtureSignedTransaction }, "invalid_transaction_hash"],
    [
      { signedTransaction: Buffer.from("opaque"), hashHex: "a".repeat(64) },
      "invalid_signed_transaction",
    ],
    [
      { signedTransaction: fixtureSignedTransaction, hashHex: "A".repeat(64) },
      "invalid_transaction_hash",
    ],
    [
      { signedTransaction: fixtureSignedTransaction, hashHex: "d".repeat(64) },
      "transaction_hash_mismatch",
    ],
    [
      {
        signedTransaction: fixtureSignedTransaction,
        signed_transaction: conflictingBytes,
        hashHex: canonicalHash,
      },
      "invalid_signed_transaction",
    ],
    [
      {
        signedTransaction: fixtureSignedTransaction,
        hashHex: canonicalHash,
        hash: Buffer.alloc(32, 0xdd),
      },
      "transaction_hash_mismatch",
    ],
    [
      { signedTransaction: [256], hashHex: canonicalHash },
      "invalid_signed_transaction",
    ],
    [
      { signedTransaction: oversizedSigned, hashHex: canonicalHash },
      "invalid_signed_transaction",
    ],
    [
      {
        signedTransaction: new ArrayBuffer(1024 * 1024 + 4097),
        hashHex: canonicalHash,
      },
      "invalid_signed_transaction",
    ],
  ];
  for (const [result, code] of cases) {
    const harness = finalizationHarness(result);
    await assert.rejects(
      () =>
        harness.client.finalizeAndSubmit(
          fixtureSignable(),
          fixtureWalletSignature,
          { wait: false },
        ),
      (error) => error instanceof NexusAppError && error.code === code,
    );
    assert.equal(harness.calls.submitted, 0, `${code} must fail before submit`);
    assert.equal(harness.calls.waited, 0, `${code} must fail before wait`);
  }
});

test("NexusAppClient rechecks signable payload hashes before finalization", async () => {
  for (const [payloadHashHex, code] of [
    [undefined, "invalid_payload_hash"],
    ["A".repeat(64), "invalid_payload_hash"],
    [`0x${"a".repeat(64)}`, "invalid_payload_hash"],
    ["d".repeat(64), "payload_hash_mismatch"],
  ]) {
    const harness = finalizationHarness({
      signedTransaction: fixtureSignedTransaction,
      hashHex: fixtureSignedTransactionHashHex,
    });
    await assert.rejects(
      () =>
        harness.client.finalizeAndSubmit(
          fixtureSignable({ payloadHashHex }),
          fixtureWalletSignature,
          { wait: false },
        ),
      (error) => error instanceof NexusAppError && error.code === code,
    );
    assert.deepEqual(harness.calls, { finalized: 0, submitted: 0, waited: 0 });
  }
});

test("NexusAppClient snapshots signature descriptors and rejects ambiguous aliases", async () => {
  const signatureTarget = {
    algorithm: "ed25519",
    signature: fixtureWalletSignature,
  };
  let signatureGets = 0;
  const proxiedSignature = new Proxy(signatureTarget, {
    get(target, property, receiver) {
      signatureGets += 1;
      target.algorithm = "secp256k1";
      if (property === "signature") return Buffer.alloc(64);
      return Reflect.get(target, property, receiver);
    },
  });
  const positive = finalizationHarness({
    signedTransaction: fixtureSignedTransaction,
    hashHex: fixtureSignedTransactionHashHex,
  });
  await positive.client.finalizeAndSubmit(fixtureSignable(), proxiedSignature, {
    wait: false,
  });
  assert.equal(signatureGets, 0);
  assert.equal(signatureTarget.algorithm, "ed25519");

  let accessorReads = 0;
  const accessor = { algorithm: "ed25519" };
  Object.defineProperty(accessor, "signature", {
    enumerable: true,
    get() {
      accessorReads += 1;
      return fixtureWalletSignature;
    },
  });
  for (const signature of [
    {
      algorithm: "ed25519",
      signature: fixtureWalletSignature,
      bytes: Buffer.alloc(64),
    },
    { algorithm: "ed25519", alg: "secp256k1", signature: fixtureWalletSignature },
    accessor,
    new Array(65).fill(1),
  ]) {
    const harness = finalizationHarness({
      signedTransaction: fixtureSignedTransaction,
      hashHex: fixtureSignedTransactionHashHex,
    });
    await assert.rejects(
      () => harness.client.finalizeAndSubmit(fixtureSignable(), signature, { wait: false }),
    );
    assert.equal(harness.calls.finalized, 0);
    assert.equal(harness.calls.submitted, 0);
  }
  assert.equal(accessorReads, 0);
});

test("NexusAppClient rejects non-Ed25519 wallet signatures", async () => {
  const client = new NexusAppClient({
    signingPublicKey: Buffer.alloc(32, 1),
    transactionCodec: {
      finalizeSignedTransaction() {
        throw new Error("should not finalize");
      },
    },
    toriiClient: {
      async submitTransaction() {
        throw new Error("should not submit");
      },
    },
  });
  for (const algorithm of unsupportedSignatureAlgorithms) {
    await assert.rejects(
      () =>
        client.finalizeAndSubmit(
          {
            payloadBytes: Buffer.from("payload"),
            payloadHashHex: nexusPayloadHashHex(Buffer.from("payload")),
            authority: "account-i105",
            signingPublicKey: Buffer.alloc(32, 1),
            signatureAlgorithm: "ed25519",
          },
          { algorithm, signature: Buffer.alloc(64) },
          { wait: false },
        ),
      (error) =>
        error instanceof NexusAppError &&
        error.code === fixture.error_cases[0].expected_code,
    );
  }
  for (const algorithm of unsupportedSignatureAlgorithms) {
    await assert.rejects(
      () =>
        client.finalizeAndSubmit(
          {
            payloadBytes: Buffer.from("payload"),
            payloadHashHex: nexusPayloadHashHex(Buffer.from("payload")),
            authority: "account-i105",
            signingPublicKey: Buffer.alloc(32, 1),
            signatureAlgorithm: algorithm,
          },
          { algorithm: "ed25519", signature: Buffer.alloc(64) },
          { wait: false },
        ),
      (error) =>
        error instanceof NexusAppError &&
        error.code === fixture.error_cases[0].expected_code,
    );
  }
});

test("NexusAppClient accepts exact numeric and string Ed25519 signature algorithm tags", async () => {
  const payload = fixturePayloadBytes;
  const signedTransaction = fixtureSignedTransaction;
  const hashHex = fixtureSignedTransactionHashHex;
  const finalized = [];
  const submitted = [];
  const client = new NexusAppClient({
    chainId: "test-chain",
    authority: "account-i105",
    signingPublicKey: fixturePublicKey,
    transactionCodec: {
      buildTransferPayload() {
        return payload;
      },
      finalizeSignedTransaction(signable, signature, signingPublicKey) {
        finalized.push({ signable, signature, signingPublicKey });
        return { signedTransaction, hashHex };
      },
    },
    toriiClient: {
      async submitTransaction(transaction) {
        submitted.push(Buffer.from(transaction));
        return { accepted: true };
      },
    },
  });

  const receipt = await client.finalizeAndSubmit(
    {
      payloadBytes: payload,
      payloadHashHex: nexusPayloadHashHex(payload),
      authority: "account-i105",
      signingPublicKey: fixturePublicKey,
      signatureAlgorithm: "0",
    },
    { algorithm: 0, signature: fixtureWalletSignature },
    { wait: false },
  );

  assert.deepEqual(receipt.signedTransaction, signedTransaction);
  assert.equal(receipt.signedTransactionHashHex, hashHex);
  assert.equal(finalized[0].signature.algorithm, "ed25519");
  assert.deepEqual(submitted, [signedTransaction]);
});

test("NexusAppClient rejects missing approval account and missing signing key", async () => {
  const missingAccountClient = new NexusAppClient({
    connectTransport: {
      awaitApproval() {
        return {};
      },
    },
  });
  await assert.rejects(
    () => missingAccountClient.awaitApproval({ sid: "sid-1" }),
    (error) =>
      error instanceof NexusAppError &&
      error.code === "approval_missing_account",
  );

  const missingKeyClient = new NexusAppClient({
    connectTransport: {
      awaitApproval() {
        return { accountId: "not-an-i105-account" };
      },
    },
  });
  await assert.rejects(
    () => missingKeyClient.awaitApproval({ sid: "sid-1" }),
    (error) =>
      error instanceof NexusAppError &&
      error.code === "missing_signing_public_key",
  );
});

test("NexusAppClient rejects authority mismatch before wallet signature request", async () => {
  let requested = false;
  const client = new NexusAppClient({
    chainId: "test-chain",
    signingPublicKey: Buffer.alloc(32, 1),
    connectTransport: {
      requestSignature() {
        requested = true;
        throw new Error("should not request signature");
      },
    },
    transactionCodec: {
      buildTransferPayload() {
        return Buffer.from("payload");
      },
    },
  });

  await assert.rejects(
    () =>
      client.transferWithWallet(
        {
          sid: "sid-1",
          approvedAccountId: "approved-account-i105",
          signingPublicKey: Buffer.alloc(32, 1),
        },
        {
          authority: "other-account-i105",
          sourceAssetHoldingId: "asset#other-account-i105",
          quantity: "1",
          destinationAccountId: "destination-i105",
        },
        { wait: false },
      ),
    (error) =>
      error instanceof NexusAppError &&
      error.code === fixture.error_cases[2].expected_code,
  );
  assert.equal(requested, false);
});

test("NexusAppClient accepts shared approvedAccount session field", async () => {
  const payloadBytes = fixturePayloadBytes;
  const signedTransaction = fixtureSignedTransaction;
  const hashHex = fixtureSignedTransactionHashHex;
  let requestedAuthority = null;

  const client = new NexusAppClient({
    chainId: "test-chain",
    transactionCodec: {
      buildTransferPayload(input) {
        requestedAuthority = input.authority;
        return payloadBytes;
      },
      finalizeSignedTransaction() {
        return { signedTransaction, hashHex };
      },
    },
    connectTransport: {
      requestSignature() {
        return { algorithm: "ed25519", signature: fixtureWalletSignature };
      },
    },
    toriiClient: {
      async submitTransaction() {
        return { hashHex };
      },
    },
  });

  await client.transferWithWallet(
    {
      sid: "sid-1",
      approvedAccount: "approved-account-i105",
      signingPublicKey: fixturePublicKey,
    },
    {
      sourceAssetHoldingId: "asset#approved-account-i105",
      quantity: "1",
      destinationAccountId: "destination-i105",
    },
    { wait: false },
  );

  assert.equal(requestedAuthority, "approved-account-i105");
});

test("NexusAppClient rejects invalid signature lengths", async () => {
  const client = new NexusAppClient({
    signingPublicKey: Buffer.alloc(32, 1),
    transactionCodec: {
      finalizeSignedTransaction() {
        throw new Error("should not finalize");
      },
    },
    toriiClient: {
      async submitTransaction() {
        throw new Error("should not submit");
      },
    },
  });

  await assert.rejects(
    () =>
      client.finalizeAndSubmit(
        {
          payloadBytes: Buffer.from("payload"),
          payloadHashHex: nexusPayloadHashHex(Buffer.from("payload")),
          authority: "account-i105",
          signingPublicKey: Buffer.alloc(32, 1),
          signatureAlgorithm: "ed25519",
        },
        { algorithm: "ed25519", signature: Buffer.alloc(63) },
        { wait: false },
      ),
    (error) =>
      error instanceof NexusAppError && error.code === "invalid_signature",
  );

  await assert.rejects(
    () =>
      client.finalizeAndSubmit(
        {
          payloadBytes: fixturePayloadBytes,
          payloadHashHex: nexusPayloadHashHex(fixturePayloadBytes),
          authority: "account-i105",
          signingPublicKey: fixturePublicKey,
          signatureAlgorithm: "ed25519",
        },
        { algorithm: "ed25519", signature: Buffer.alloc(64, 7) },
        { wait: false },
      ),
    (error) =>
      error instanceof NexusAppError && error.code === "invalid_signature",
  );
});

test("NexusAppClient rejects Torii hash mismatches and maps submit/status failures", async () => {
  const signable = {
    payloadBytes: fixturePayloadBytes,
    payloadHashHex: nexusPayloadHashHex(fixturePayloadBytes),
    authority: "account-i105",
    signingPublicKey: fixturePublicKey,
    signatureAlgorithm: "ed25519",
  };
  const signature = { algorithm: "ed25519", signature: fixtureWalletSignature };
  const signedTransaction = fixtureSignedTransaction;
  const localHash = fixtureSignedTransactionHashHex;

  const mismatchClient = new NexusAppClient({
    signingPublicKey: fixturePublicKey,
    transactionCodec: {
      finalizeSignedTransaction() {
        return { signedTransaction, hashHex: localHash };
      },
    },
    toriiClient: {
      async submitTransaction() {
        return { hashHex: "b".repeat(64) };
      },
    },
  });
  await assert.rejects(
    () => mismatchClient.finalizeAndSubmit(signable, signature, { wait: false }),
    (error) =>
      error instanceof NexusAppError &&
      error.code === "transaction_hash_mismatch",
  );

  const submitFailureClient = new NexusAppClient({
    signingPublicKey: fixturePublicKey,
    transactionCodec: {
      finalizeSignedTransaction() {
        return { signedTransaction, hashHex: localHash };
      },
    },
    toriiClient: {
      async submitTransaction() {
        throw new Error("down");
      },
    },
  });
  await assert.rejects(
    () => submitFailureClient.finalizeAndSubmit(signable, signature, { wait: false }),
    (error) => error instanceof NexusAppError && error.code === "submit_failed",
  );

  const statusFailureClient = new NexusAppClient({
    signingPublicKey: fixturePublicKey,
    transactionCodec: {
      finalizeSignedTransaction() {
        return { signedTransaction, hashHex: localHash };
      },
    },
    toriiClient: {
      async submitTransaction() {
        return { hashHex: localHash };
      },
      async waitForTransactionStatus() {
        throw new Error("timeout");
      },
    },
  });
  await assert.rejects(
    () => statusFailureClient.finalizeAndSubmit(signable, signature),
    (error) =>
      error instanceof NexusAppError && error.code === "status_wait_failed",
  );
});

test("NexusAppClient conflict-checks all Torii hash aliases before waiting", async () => {
  const canonicalHash = fixtureSignedTransactionHashHex;
  const finalizer = {
    signedTransaction: fixtureSignedTransaction,
    hashHex: canonicalHash,
  };
  for (const [submission, code] of [
    [
      { hashHex: canonicalHash, tx_hash: "d".repeat(64) },
      "transaction_hash_mismatch",
    ],
    [
      { hashHex: canonicalHash, payload: { signed_transaction_hash: "d".repeat(64) } },
      "transaction_hash_mismatch",
    ],
    [{ hashHex: "A".repeat(64) }, "invalid_transaction_hash"],
    [{ txHash: [256] }, "invalid_transaction_hash"],
  ]) {
    const harness = finalizationHarness(finalizer, submission);
    await assert.rejects(
      () =>
        harness.client.finalizeAndSubmit(
          fixtureSignable(),
          fixtureWalletSignature,
        ),
      (error) => error instanceof NexusAppError && error.code === code,
    );
    assert.equal(harness.calls.submitted, 1);
    assert.equal(harness.calls.waited, 0);
  }

  const equivalent = finalizationHarness(finalizer, {
    hashHex: canonicalHash,
    hash: Buffer.from(canonicalHash, "hex"),
    tx_hash: Buffer.from(canonicalHash, "hex"),
    payload: {
      signedTransactionHashHex: canonicalHash,
      signed_transaction_hash: Buffer.from(canonicalHash, "hex"),
    },
  });
  const receipt = await equivalent.client.finalizeAndSubmit(
    fixtureSignable(),
    fixtureWalletSignature,
  );
  assert.equal(receipt.signedTransactionHashHex, canonicalHash);
  assert.deepEqual(equivalent.calls, { finalized: 1, submitted: 1, waited: 1 });
});
