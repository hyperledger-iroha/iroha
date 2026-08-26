import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  NexusAppClient,
  NexusAppError,
  nexusPayloadHashHex,
} from "../src/nexusApp.js";
import { NetworkId } from "../src/networkId.js";
import {
  browserSignedTransactionHashHex,
  finalizeBrowserSignedTransaction,
} from "../src/transactionCodec.js";
import { AccountAddress } from "../src/address.js";

const fixture = JSON.parse(
  readFileSync(
    new URL("../../../fixtures/sdk/nexus_connect_transfer_v1.json", import.meta.url),
    "utf8",
  ),
);
const fixtureNetworkId = NetworkId.parse(fixture.transfer_input.network_id);
const fixtureChainDiscriminant = fixture.transfer_input.account_chain_discriminant;
const fixtureAuthority = fixture.transfer_input.authority;
const fixtureDestination = fixture.transfer_input.destination_account_id;
const fixtureSourceAsset = fixture.transfer_input.source_asset_id;
const foreignNetworkId = NetworkId.fromBytes(Buffer.alloc(32, 0x55));
const fixturePayloadBytes = Buffer.from(fixture.expected.payload_bytes_hex, "hex");
const fixturePublicKey = Buffer.from(
  fixture.connect.approval_frame.signing_public_key_hex,
  "hex",
);
const fixtureWalletSignature = Buffer.from(
  fixture.expected.wallet_signature_hex,
  "hex",
);
const fixtureErrorCase = (name) => {
  const found = fixture.error_cases.find((candidate) => candidate.name === name);
  assert.ok(found, `shared fixture error case ${name}`);
  return found;
};
const fixtureFinalized = finalizeBrowserSignedTransaction(
  {
    networkId: fixtureNetworkId,
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

function authoritativeAppliedStatus(
  hash = fixtureSignedTransactionHashHex,
  blockHeight = 1,
) {
  return {
    hash,
    status: { kind: "Applied", block_height: blockHeight },
    summary: "Applied",
    diagnostics: [],
    scope: "global",
    resolved_from: "state",
  };
}

function fixtureFeePayment() {
  return {
    payer: fixture.transfer_input.fee_payment.payer,
    chargeLimits: [...fixture.transfer_input.fee_payment.value.charge_limits],
    gasLimit: fixture.transfer_input.fee_payment.value.gas_limit,
  };
}

function fixtureSignable(overrides = {}) {
  return {
    networkId: fixtureNetworkId,
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
    chainDiscriminant: fixtureChainDiscriminant,
    networkId: fixtureNetworkId,
    authority: fixtureAuthority,
    signingPublicKey: fixturePublicKey,
    transactionCodec,
  });
}

function finalizationHarness(finalizedResult, submission = { accepted: true }) {
  const calls = { finalized: 0, submitted: 0, waited: 0 };
  const client = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
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
        return authoritativeAppliedStatus();
      },
    },
  });
  return { client, calls };
}

test("NexusAppError keeps retry classification and context immutable", () => {
  const submission = { accepted: true };
  const status = { status: "Rejected" };
  const error = new NexusAppError("transaction_rejected", "rejected", 0, {
    phase: "status_wait",
    submissionState: "submitted",
    signedTransactionHashHex: fixtureSignedTransactionHashHex,
    submission,
    status,
  });
  const expected = {
    code: "transaction_rejected",
    cause: 0,
    phase: "status_wait",
    submissionState: "submitted",
    signedTransactionHashHex: fixtureSignedTransactionHashHex,
    submission,
    status,
  };
  for (const [field, value] of Object.entries(expected)) {
    assert.equal(error[field], value, field);
    const descriptor = Object.getOwnPropertyDescriptor(error, field);
    assert.equal(descriptor?.writable, false, `${field} must not be writable`);
    assert.equal(descriptor?.configurable, false, `${field} must not be configurable`);
    assert.equal(descriptor?.enumerable, true, `${field} must be enumerable`);
    assert.throws(() => {
      error[field] = "mutated";
    }, TypeError);
    assert.throws(() => {
      delete error[field];
    }, TypeError);
  }
});

test("NexusAppClient builds a signable transfer draft", () => {
  const payloadBytes = Buffer.from("canonical-transfer-payload");
  const client = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    networkId: fixtureNetworkId,
    authority: fixtureAuthority,
    signingPublicKey: fixturePublicKey,
    transactionCodec: {
      buildTransferPayload(input) {
        assert.equal(input.chainDiscriminant, fixtureChainDiscriminant);
        assert.equal(input.networkId, fixtureNetworkId);
        assert.equal(input.authority, fixtureAuthority);
        assert.equal(input.quantity, "12.5");
        assert.equal(input.destinationAccountId, fixtureDestination);
        return payloadBytes;
      },
    },
  });

  const draft = client.buildTransferDraft({
    sourceAssetHoldingId: fixtureSourceAsset,
    quantity: "12.5",
    destinationAccountId: fixtureDestination,
    feePayment: fixtureFeePayment(),
  });

  assert.equal(draft.signable.networkId, fixtureNetworkId);
  assert.deepEqual(draft.signable.payloadBytes, payloadBytes);
  assert.equal(
    draft.signable.payloadHashHex,
    nexusPayloadHashHex(payloadBytes),
  );
});

test("NexusAppClient validates quantities before invoking custom transaction codecs", () => {
  let codecCalls = 0;
  const client = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    networkId: fixtureNetworkId,
    authority: fixtureAuthority,
    signingPublicKey: fixturePublicKey,
    transactionCodec: {
      buildTransferPayload(input) {
        codecCalls += 1;
        assert.equal(input.quantity, "7");
        return Buffer.from([1]);
      },
    },
  });
  const base = {
    sourceAssetHoldingId: fixtureSourceAsset,
    destinationAccountId: fixtureDestination,
    feePayment: fixtureFeePayment(),
  };

  for (const quantity of [7, " 7", "07", "+7", "7.0", "-7"]) {
    assert.throws(
      () => client.buildTransferDraft({ ...base, quantity }),
      (error) => error instanceof NexusAppError && error.code === "invalid_transfer_input",
      String(quantity),
    );
  }
  assert.equal(codecCalls, 0);

  client.buildTransferDraft({ ...base, quantity: 7n });
  assert.equal(codecCalls, 1);
});

test("NexusAppClient snapshots extension owners and invokes capabilities intrinsically", async () => {
  const connect = {};
  let connectClient;
  const startCapability = function startConnect() {
    assert.equal(this, connect);
    return { sid: "stable-connect-owner" };
  };
  Object.defineProperty(startCapability, "apply", {
    value() {
      throw new Error("a capability's own apply property must not run");
    },
  });
  Object.defineProperty(connect, "startConnect", {
    get() {
      connectClient.connect = {};
      return startCapability;
    },
  });
  connectClient = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant, connectTransport: connect });
  assert.equal((await connectClient.startConnect()).sid, "stable-connect-owner");

  const transactionCodec = {};
  let codecClient;
  Object.defineProperty(transactionCodec, "buildTransferPayload", {
    get() {
      codecClient.transactionCodec = {};
      return function buildTransferPayload() {
        assert.equal(this, transactionCodec);
        return fixturePayloadBytes;
      };
    },
  });
  codecClient = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    networkId: fixtureNetworkId,
    authority: fixture.transfer_input.authority,
    signingPublicKey: fixturePublicKey,
    transactionCodec,
  });
  const draft = codecClient.buildTransferDraft({
    sourceAssetHoldingId: fixture.transfer_input.source_asset_id,
    quantity: fixture.transfer_input.quantity,
    destinationAccountId: fixture.transfer_input.destination_account_id,
    feePayment: fixtureFeePayment(),
  });
  assert.deepEqual(draft.signable.payloadBytes, fixturePayloadBytes);
});

test("NexusAppClient payload hashing matches the shared Nexus fixture", () => {
  const payloadBytes = Buffer.from(fixture.expected.payload_bytes_hex, "hex");
  assert.equal(
    nexusPayloadHashHex(payloadBytes),
    fixture.expected.payload_hash_hex,
  );

  const client = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    networkId: fixtureNetworkId,
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
    feePayment: fixtureFeePayment(),
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
    chainDiscriminant: fixtureChainDiscriminant,
    networkId: fixtureNetworkId,
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
    feePayment: fixtureFeePayment(),
    metadata: fixture.transfer_input.metadata,
  });

  assert.deepEqual(draft.signable.payloadBytes, fixturePayloadBytes);
  assert.equal(draft.signable.payloadHashHex, fixture.expected.payload_hash_hex);
});

test("NexusAppClient requires one exact account chain context", () => {
  const input = {
    sourceAssetHoldingId: fixture.transfer_input.source_asset_id,
    quantity: fixture.transfer_input.quantity,
    destinationAccountId: fixture.transfer_input.destination_account_id,
    feePayment: fixtureFeePayment(),
  };
  assert.throws(
    () =>
      new NexusAppClient({
        networkId: fixtureNetworkId,
        authority: fixture.transfer_input.authority,
        signingPublicKey: fixturePublicKey,
      }).buildTransferDraft(input),
    (error) => error instanceof NexusAppError && error.code === "invalid_config",
  );
  assert.throws(
    () =>
      new NexusAppClient({
        chainDiscriminant: fixtureChainDiscriminant + 1,
        networkId: fixtureNetworkId,
        authority: fixture.transfer_input.authority,
        signingPublicKey: fixturePublicKey,
      }).buildTransferDraft(input),
    (error) =>
      error instanceof NexusAppError &&
      error.code === "invalid_account_id",
  );
  const mixedChainDestination = AccountAddress.fromI105(
    fixtureDestination,
    fixtureChainDiscriminant,
  ).toI105(fixtureChainDiscriminant + 1);
  assert.throws(
    () =>
      new NexusAppClient({
        chainDiscriminant: fixtureChainDiscriminant,
        networkId: fixtureNetworkId,
        authority: fixtureAuthority,
        signingPublicKey: fixturePublicKey,
      }).buildTransferDraft({
        ...input,
        destinationAccountId: mixedChainDestination,
      }),
    (error) =>
      error instanceof NexusAppError && error.code === "invalid_account_id",
  );
  assert.throws(
    () =>
      new NexusAppClient({
        chainDiscriminant: fixtureChainDiscriminant,
        networkId: fixtureNetworkId,
        authority: fixtureAuthority,
        signingPublicKey: fixturePublicKey,
      }).buildTransferDraft({
        ...input,
        sourceAssetHoldingId: `${fixtureSourceAsset}#dataspace:01`,
      }),
    (error) =>
      error instanceof NexusAppError && error.code === "invalid_account_id",
  );
});

test("NexusAppClient rejects a wrong-chain wallet approval", async () => {
  const wrongChainAccount = AccountAddress.fromI105(
    fixtureAuthority,
    fixtureChainDiscriminant,
  ).toI105(fixtureChainDiscriminant + 1);
  const client = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    connectTransport: {
      awaitApproval() {
        return {
          accountId: wrongChainAccount,
          signingPublicKey: fixturePublicKey,
        };
      },
    },
  });

  await assert.rejects(
    () => client.awaitApproval({ sid: "wrong-chain-approval" }),
    (error) =>
      error instanceof NexusAppError && error.code === "invalid_account_id",
  );
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
    sourceAssetHoldingId: fixtureSourceAsset,
    quantity: "1",
    destinationAccountId: fixtureDestination,
    feePayment: fixtureFeePayment(),
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
          sourceAssetHoldingId: fixtureSourceAsset,
          quantity: "1",
          destinationAccountId: fixtureDestination,
          feePayment: fixtureFeePayment(),
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
          sourceAssetHoldingId: fixtureSourceAsset,
          quantity: "1",
          destinationAccountId: fixtureDestination,
          feePayment: fixtureFeePayment(),
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
          sourceAssetHoldingId: fixtureSourceAsset,
          quantity: "1",
          destinationAccountId: fixtureDestination,
          feePayment: fixtureFeePayment(),
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
          sourceAssetHoldingId: fixtureSourceAsset,
          quantity: "1",
          destinationAccountId: fixtureDestination,
          feePayment: fixtureFeePayment(),
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
    chainDiscriminant: fixtureChainDiscriminant,
    networkId: fixtureNetworkId,
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
          accountId: fixture.transfer_input.authority,
        };
      },
      requestSignature(_session, signable) {
        requested.push(Buffer.from(signable.payloadBytes));
        return { algorithm: "ed25519", signature: walletSignature };
      },
    },
    transactionCodec: {
      buildTransferPayload(input) {
        assert.equal(input.authority, fixture.transfer_input.authority);
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
        return authoritativeAppliedStatus(txHashHex);
      },
    },
  });

  const session = await client.startConnect({ sid: "sid-1" });
  const approval = await client.awaitApproval(session);
  const receipt = await client.transferWithWallet(
    approval.session,
    {
      sourceAssetHoldingId: fixture.transfer_input.source_asset_id,
      quantity: "1",
      destinationAccountId: fixtureDestination,
      feePayment: fixtureFeePayment(),
    },
    { timeoutMs: 1 },
  );

  assert.equal(receipt.signedTransactionHashHex, hashHex);
  assert.deepEqual(receipt.signedTransaction, signedTransaction);
  assert.deepEqual(requested, [payloadBytes]);
  assert.deepEqual(submitted, [signedTransaction]);
  assert.deepEqual(waited, [hashHex]);
});

test("NexusAppClient accepts the complete default browser Connect approval proof", async () => {
  const walletPublicKey = new Uint8Array(32).fill(0xa5);
  const approvalSignature = new Uint8Array(64).fill(0x5a);
  const appSession = {
    waitForApproval() {
      return {
        accountId: fixture.transfer_input.authority,
        signingPublicKey: Uint8Array.from(fixturePublicKey),
        walletPublicKey,
        signature: approvalSignature,
      };
    },
  };
  const client = new NexusAppClient({ chainDiscriminant: fixtureChainDiscriminant });

  const approval = await client.awaitApproval({
    sid: "sid-browser-proof",
    appSession,
  });

  assert.equal(approval.accountId, fixture.transfer_input.authority);
  assert.deepEqual(approval.signingPublicKey, fixturePublicKey);
  assert.notDeepEqual(approval.signingPublicKey, walletPublicKey);
  assert.equal(approval.session.appSession, appSession);
  assert.equal(approval.session.approvedAccountId, fixture.transfer_input.authority);
  assert.deepEqual(approval.session.signingPublicKey, fixturePublicKey);
});

test("NexusAppClient keeps browser approval proofs strict and transport-local", async () => {
  const validProof = {
    accountId: fixture.transfer_input.authority,
    signingPublicKey: Uint8Array.from(fixturePublicKey),
    walletPublicKey: new Uint8Array(32).fill(0xa5),
    signature: new Uint8Array(64).fill(0x5a),
  };
  const missingWalletKey = { ...validProof };
  delete missingWalletKey.walletPublicKey;
  const missingSigningKey = { ...validProof };
  delete missingSigningKey.signingPublicKey;
  const missingSignature = { ...validProof };
  delete missingSignature.signature;
  const malformedProofs = [
    missingSigningKey,
    { ...validProof, signingPublicKey: new Uint8Array(31) },
    { ...validProof, signingPublicKey: Buffer.alloc(32) },
    missingWalletKey,
    { ...validProof, walletPublicKey: new Uint8Array(31) },
    { ...validProof, walletPublicKey: new Uint8Array(33) },
    { ...validProof, walletPublicKey: Buffer.alloc(32) },
    { ...validProof, walletPublicKey: new ArrayBuffer(32) },
    { ...validProof, walletPublicKey: new DataView(new ArrayBuffer(32)) },
    { ...validProof, walletPublicKey: new Uint16Array(16) },
    { ...validProof, walletPublicKey: new Array(32).fill(0) },
    { ...validProof, walletPublicKey: "00".repeat(32) },
    missingSignature,
    { ...validProof, signature: new Uint8Array(63) },
    { ...validProof, signature: new Uint8Array(65) },
    { ...validProof, unsupported: true },
    { ...validProof, [Symbol("unsupported")]: true },
    Object.assign(Object.create({ inherited: true }), validProof),
  ];
  if (typeof SharedArrayBuffer !== "undefined") {
    malformedProofs.push({
      ...validProof,
      walletPublicKey: new Uint8Array(new SharedArrayBuffer(32)),
    });
  }
  const revoked = Proxy.revocable(validProof, {});
  revoked.revoke();
  for (const [index, proof] of malformedProofs.entries()) {
    const client = new NexusAppClient({ chainDiscriminant: fixtureChainDiscriminant });
    await assert.rejects(
      () =>
        client.awaitApproval({
          sid: `sid-malformed-browser-proof-${index}`,
          appSession: {
            waitForApproval() {
              return proof;
            },
          },
        }),
      (error) =>
      error instanceof NexusAppError && error.code === "invalid_wallet_approval",
    );
  }

  await assert.rejects(
    () =>
      new NexusAppClient({ chainDiscriminant: fixtureChainDiscriminant }).awaitApproval({
        sid: "sid-revoked-browser-proof",
        appSession: {
          waitForApproval() {
            return revoked.proxy;
          },
        },
      }),
    (error) =>
      error instanceof NexusAppError && error.code === "connect_approval_failed",
  );

  let accessorGets = 0;
  const accessorProof = {
    accountId: validProof.accountId,
    signingPublicKey: validProof.signingPublicKey,
    signature: validProof.signature,
  };
  Object.defineProperty(accessorProof, "walletPublicKey", {
    enumerable: true,
    get() {
      accessorGets += 1;
      return validProof.walletPublicKey;
    },
  });
  await assert.rejects(
    () =>
      new NexusAppClient({ chainDiscriminant: fixtureChainDiscriminant }).awaitApproval({
        sid: "sid-accessor-browser-proof",
        appSession: {
          waitForApproval() {
            return accessorProof;
          },
        },
      }),
    (error) =>
      error instanceof NexusAppError && error.code === "invalid_wallet_approval",
  );
  assert.equal(accessorGets, 0);

  let customApprovalCalls = 0;
  const customTransportClient = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    connectTransport: {
      awaitApproval() {
        customApprovalCalls += 1;
        return validProof;
      },
    },
  });
  await assert.rejects(
    () => customTransportClient.awaitApproval({ sid: "sid-custom-proof-boundary" }),
    (error) =>
      error instanceof NexusAppError && error.code === "invalid_wallet_approval",
  );
  assert.equal(customApprovalCalls, 1);
});

test("NexusAppClient accepts raw wallet signature byte inputs", async () => {
  const payloadBytes = fixturePayloadBytes;
  const signable = {
    networkId: fixtureNetworkId,
    payloadBytes,
    payloadHashHex: nexusPayloadHashHex(payloadBytes),
    authority: fixture.transfer_input.authority,
    signingPublicKey: fixturePublicKey,
    signatureAlgorithm: "ed25519",
  };
  const walletSignature = fixtureWalletSignature;
  const signedTransaction = fixtureSignedTransaction;
  const hashHex = fixtureSignedTransactionHashHex;
  let finalizedSignature = null;
  let submittedPayload = null;

  const client = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
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

test("NexusAppClient validates and detaches canonical signables before every signer callback", async () => {
  const signable = fixtureSignable();
  const originalPayload = Buffer.from(signable.payloadBytes);
  const originalPublicKey = Buffer.from(signable.signingPublicKey);
  let callbackCalls = 0;
  const client = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    authority: fixture.transfer_input.authority,
    signingPublicKey: fixturePublicKey,
    connectTransport: {
      requestSignature(_session, received) {
        callbackCalls += 1;
        assert.notStrictEqual(received.payloadBytes, signable.payloadBytes);
        assert.notStrictEqual(received.signingPublicKey, signable.signingPublicKey);
        assert.deepEqual(received.payloadBytes, originalPayload);
        assert.deepEqual(received.signingPublicKey, originalPublicKey);
        received.payloadBytes.fill(0);
        received.signingPublicKey.fill(0);
        return fixtureWalletSignature;
      },
    },
  });
  const session = {
    sid: "sid-canonical",
    approvedAccountId: fixture.transfer_input.authority,
    signingPublicKey: fixturePublicKey,
  };
  const signature = await client.requestSignature(session, signable);
  assert.deepEqual(signature.signature, fixtureWalletSignature);
  assert.equal(callbackCalls, 1);
  assert.deepEqual(signable.payloadBytes, originalPayload);
  assert.deepEqual(signable.signingPublicKey, originalPublicKey);

  const malformedPayload = Buffer.from(fixturePayloadBytes);
  malformedPayload[0] ^= 0xff;
  const destinationAccount = fixture.transfer_input.destination_account_id;
  for (const [label, candidate, candidateSession] of [
    [
      "foreign NetworkId",
      fixtureSignable({ networkId: foreignNetworkId }),
      session,
    ],
    [
      "wrong hash",
      fixtureSignable({ payloadHashHex: "d".repeat(64) }),
      session,
    ],
    [
      "noncanonical payload bytes",
      fixtureSignable({
        payloadBytes: malformedPayload,
        payloadHashHex: nexusPayloadHashHex(malformedPayload),
      }),
      session,
    ],
    [
      "wrong asserted authority",
      fixtureSignable({ authority: destinationAccount }),
      session,
    ],
    [
      "wrong signable key",
      fixtureSignable({ signingPublicKey: Buffer.alloc(32, 0x7f) }),
      session,
    ],
    [
      "wrong approved account",
      fixtureSignable(),
      {
        sid: "sid-wrong-account",
        approvedAccountId: destinationAccount,
      },
    ],
    [
      "wrong approved key",
      fixtureSignable(),
      {
        sid: "sid-wrong-key",
        approvedAccountId: fixture.transfer_input.authority,
        signingPublicKey: Buffer.alloc(32, 0x7f),
      },
    ],
    [
      "oversized approved account",
      fixtureSignable(),
      {
        sid: "sid-oversized-account",
        approvedAccountId: "x".repeat(1_000_000),
      },
    ],
  ]) {
    let injectedCalls = 0;
    let appSessionCalls = 0;
    const adversarial = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
      connectTransport: {
        requestSignature() {
          injectedCalls += 1;
          return undefined;
        },
      },
    });
    await assert.rejects(
      () =>
        adversarial.requestSignature(
          {
            ...candidateSession,
            appSession: {
              signTransaction() {
                appSessionCalls += 1;
                return fixtureWalletSignature;
              },
            },
          },
          candidate,
        ),
      Error,
      label,
    );
    assert.equal(injectedCalls, 0, `${label} must not invoke configured signer`);
    assert.equal(appSessionCalls, 0, `${label} must not invoke app-session signer`);
  }
});

test("NexusAppClient rejects conflicting Connect approval and transfer alias families", async () => {
  const destinationAccount = fixture.transfer_input.destination_account_id;
  let signerCalls = 0;
  const signerClient = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    connectTransport: {
      requestSignature() {
        signerCalls += 1;
        return fixtureWalletSignature;
      },
    },
  });
  await assert.rejects(
    () =>
      signerClient.requestSignature(
        {
          sid: "sid-conflict",
          approvedAccountId: fixture.transfer_input.authority,
          approved_account: destinationAccount,
          signingPublicKey: fixturePublicKey,
        },
        fixtureSignable(),
      ),
    (error) =>
      error instanceof NexusAppError && error.code === "invalid_connect_session",
  );
  assert.equal(signerCalls, 0);

  await assert.rejects(
    () =>
      signerClient.requestSignature(
        {
          sid: "sid-key-conflict",
          approvedAccountId: fixture.transfer_input.authority,
          signingPublicKey: fixturePublicKey,
          signing_public_key: Buffer.alloc(32, 0x7f),
        },
        fixtureSignable(),
      ),
    (error) =>
      error instanceof NexusAppError && error.code === "invalid_connect_session",
  );
  assert.equal(signerCalls, 0);

  for (const returnedSession of [
    {
      sid: "sid-uri-conflict",
      walletLaunchUri: "iroha://wallet-a",
      wallet_uri: "iroha://wallet-b",
    },
    {
      sid: "sid-token-conflict",
      tokenApp: "token-a",
      token_app: "token-b",
    },
  ]) {
    const sessionClient = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
      connectTransport: {
        startConnect() {
          return returnedSession;
        },
      },
    });
    await assert.rejects(
      () => sessionClient.startConnect({ sid: returnedSession.sid }),
      (error) =>
        error instanceof NexusAppError && error.code === "invalid_connect_session",
    );
  }

  const approvalClient = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    connectTransport: {
      awaitApproval() {
        return {
          accountId: fixture.transfer_input.authority,
          account_id: destinationAccount,
          signingPublicKey: fixturePublicKey,
        };
      },
    },
  });
  await assert.rejects(
    () => approvalClient.awaitApproval({ sid: "sid-approval-conflict" }),
    (error) =>
      error instanceof NexusAppError && error.code === "invalid_wallet_approval",
  );

  const approvalKeyClient = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    connectTransport: {
      awaitApproval() {
        return {
          accountId: fixture.transfer_input.authority,
          signingPublicKey: fixturePublicKey,
          signing_public_key: Buffer.alloc(32, 0x7f),
        };
      },
    },
  });
  await assert.rejects(
    () => approvalKeyClient.awaitApproval({ sid: "sid-approval-key-conflict" }),
    (error) =>
      error instanceof NexusAppError && error.code === "invalid_wallet_approval",
  );

  assert.throws(
    () =>
      new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
        authority: fixture.transfer_input.authority,
        accountId: destinationAccount,
      }),
    (error) => error instanceof NexusAppError && error.code === "invalid_config",
  );

  const equivalentSession = await new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    connectTransport: {
      startConnect() {
        return {
          sid: "sid-equivalent",
          walletLaunchUri: "iroha://wallet",
          wallet_uri: "iroha://wallet",
          tokenApp: "token",
          token_app: "token",
          signingPublicKey: fixturePublicKey,
          signing_public_key: Buffer.from(fixturePublicKey),
        };
      },
    },
  }).startConnect({ sid: "sid-equivalent" });
  assert.equal(equivalentSession.walletLaunchUri, "iroha://wallet");
  assert.equal(equivalentSession.tokenApp, "token");
  assert.deepEqual(equivalentSession.signingPublicKey, fixturePublicKey);

  const equivalentApproval = await new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    authority: fixture.transfer_input.authority,
    accountId: fixture.transfer_input.authority,
    connectTransport: {
      awaitApproval() {
        return {
          accountId: fixture.transfer_input.authority,
          account_id: fixture.transfer_input.authority,
          signingPublicKey: fixturePublicKey,
          signing_public_key: Buffer.from(fixturePublicKey),
        };
      },
    },
  }).awaitApproval({ sid: "sid-equivalent-approval" });
  assert.equal(equivalentApproval.accountId, fixture.transfer_input.authority);
  assert.deepEqual(equivalentApproval.signingPublicKey, fixturePublicKey);

  let codecCalls = 0;
  const draft = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    networkId: fixtureNetworkId,
    authority: fixture.transfer_input.authority,
    signingPublicKey: fixturePublicKey,
    transactionCodec: {
      buildTransferPayload() {
        codecCalls += 1;
        return fixturePayloadBytes;
      },
    },
  });
  for (const [label, input] of [
    [
      "authority",
      {
        authority: fixture.transfer_input.authority,
        accountId: destinationAccount,
        sourceAssetHoldingId: fixture.transfer_input.source_asset_id,
        quantity: "1",
        destinationAccountId: destinationAccount,
      },
    ],
    [
      "source asset",
      {
        sourceAssetHoldingId: fixture.transfer_input.source_asset_id,
        sourceAssetId: `other#${fixture.transfer_input.authority}`,
        quantity: "1",
        destinationAccountId: destinationAccount,
      },
    ],
    [
      "destination",
      {
        sourceAssetHoldingId: fixture.transfer_input.source_asset_id,
        quantity: "1",
        destinationAccountId: destinationAccount,
        to: fixture.transfer_input.authority,
      },
    ],
  ]) {
    assert.throws(
      () => draft.buildTransferDraft(input),
      (error) =>
        error instanceof NexusAppError && error.code === "invalid_transfer_input",
      label,
    );
  }
  assert.equal(codecCalls, 0, "conflicting draft aliases must fail before codec callbacks");

  const equivalent = draft.buildTransferDraft({
    authority: fixture.transfer_input.authority,
    accountId: fixture.transfer_input.authority,
    sourceAssetHoldingId: fixture.transfer_input.source_asset_id,
    sourceAssetId: fixture.transfer_input.source_asset_id,
    quantity: fixture.transfer_input.quantity,
    destinationAccountId: destinationAccount,
    destination: destinationAccount,
    feePayment: fixtureFeePayment(),
  });
  assert.deepEqual(equivalent.signable.payloadBytes, fixturePayloadBytes);
  assert.equal(codecCalls, 1, "equivalent aliases remain compatible");
});

test("NexusAppClient binds approval keys and session provenance from the shared fixture", async () => {
  const keyMismatch = fixtureErrorCase("approval signing key mismatch");
  const substitutedSession = fixtureErrorCase("approval session substitution");
  const callerSession = {
    sid: fixture.connect.sid,
    walletLaunchUri: fixture.connect.wallet_launch_uri,
  };

  for (const errorCase of [keyMismatch, substitutedSession]) {
    let approvalCalls = 0;
    const client = new NexusAppClient({
      chainDiscriminant: fixtureChainDiscriminant,
      connectTransport: {
        awaitApproval() {
          approvalCalls += 1;
          const approval = errorCase.approval_frame;
          return {
            accountId: approval.account_id,
            signingPublicKey: Buffer.from(
              approval.signing_public_key_hex,
              "hex",
            ),
            ...(approval.session === undefined
              ? {}
              : {
                  session: {
                    sid: approval.session.sid,
                    walletLaunchUri: approval.session.wallet_launch_uri,
                  },
                }),
          };
        },
      },
    });
    await assert.rejects(
      () => client.awaitApproval(callerSession),
      (error) =>
        error instanceof NexusAppError &&
        error.code === errorCase.expected_code,
      errorCase.name,
    );
    assert.equal(approvalCalls, 1);
    assert.equal(callerSession.sid, fixture.connect.sid);
  }
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
  const alternateSignedTransaction = Buffer.from(fixtureSignedTransaction);
  const signaturePrefix = Buffer.from([
    1,
    fixtureWalletSignature[0],
    1,
    fixtureWalletSignature[1],
    1,
    fixtureWalletSignature[2],
  ]);
  const signaturePrefixOffset = alternateSignedTransaction.indexOf(signaturePrefix);
  assert.notEqual(signaturePrefixOffset, -1);
  const firstSignatureByteOffset = signaturePrefixOffset + 1;
  alternateSignedTransaction[firstSignatureByteOffset] ^= 0xff;
  const alternateHash = browserSignedTransactionHashHex(
    alternateSignedTransaction,
  );
  const revoked = Proxy.revocable({}, {});
  revoked.revoke();
  const hostileFinalizerResult = new Proxy({}, {
    getPrototypeOf() {
      throw revoked.proxy;
    },
  });
  const oversizedSigned = [];
  oversizedSigned.length = 1024 * 1024 + 4097;
  const cases = [
    [fixtureSignedTransaction, "invalid_transaction_hash"],
    [hostileFinalizerResult, "invalid_signed_transaction"],
    [{ signedTransaction: fixtureSignedTransaction }, "invalid_transaction_hash"],
    [
      {
        signedTransaction: fixtureSignedTransaction,
        signedTransactionHash: Buffer.alloc(32, 0x12),
      },
      "invalid_transaction_hash",
    ],
    [
      { signedTransaction: Buffer.from("opaque"), hashHex: "b".repeat(64) },
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
    [
      {
        signedTransaction: alternateSignedTransaction,
        hashHex: alternateHash,
      },
      "signed_transaction_mismatch",
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
      (error) => {
        assert.ok(error instanceof NexusAppError);
        assert.equal(error.code, code);
        assert.equal(error.phase, "finalization");
        assert.equal(error.submissionState, "not_submitted");
        return true;
      },
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
    chainDiscriminant: fixtureChainDiscriminant,
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
            networkId: fixtureNetworkId,
            payloadBytes: Buffer.from("payload"),
            payloadHashHex: nexusPayloadHashHex(Buffer.from("payload")),
            authority: fixtureAuthority,
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
            networkId: fixtureNetworkId,
            payloadBytes: Buffer.from("payload"),
            payloadHashHex: nexusPayloadHashHex(Buffer.from("payload")),
            authority: fixtureAuthority,
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
    chainDiscriminant: fixtureChainDiscriminant,
    networkId: fixtureNetworkId,
    authority: fixture.transfer_input.authority,
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
      networkId: fixtureNetworkId,
      payloadBytes: payload,
      payloadHashHex: nexusPayloadHashHex(payload),
      authority: fixture.transfer_input.authority,
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

test("NexusAppClient rejects missing and malformed approval accounts", async () => {
  const missingAccountClient = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
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
    chainDiscriminant: fixtureChainDiscriminant,
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
      error.code === "invalid_account_id",
  );
});

test("NexusAppClient rejects authority mismatch before wallet signature request", async () => {
  let requested = false;
  const client = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    networkId: fixtureNetworkId,
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
          approvedAccountId: fixtureAuthority,
          signingPublicKey: Buffer.alloc(32, 1),
        },
        {
          authority: fixtureDestination,
          sourceAssetHoldingId: `asset#${fixtureDestination}`,
          quantity: "1",
          destinationAccountId: fixtureAuthority,
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
    chainDiscriminant: fixtureChainDiscriminant,
    networkId: fixtureNetworkId,
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
      approvedAccount: fixture.transfer_input.authority,
      signingPublicKey: fixturePublicKey,
    },
    {
      sourceAssetHoldingId: fixture.transfer_input.source_asset_id,
      quantity: "1",
      destinationAccountId: fixtureDestination,
      feePayment: fixtureFeePayment(),
    },
    { wait: false },
  );

  assert.equal(requestedAuthority, fixture.transfer_input.authority);
});

test("NexusAppClient rejects invalid signature lengths", async () => {
  const client = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    signingPublicKey: fixturePublicKey,
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
          networkId: fixtureNetworkId,
          payloadBytes: Buffer.from("payload"),
          payloadHashHex: nexusPayloadHashHex(Buffer.from("payload")),
          authority: fixtureAuthority,
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
          networkId: fixtureNetworkId,
          payloadBytes: fixturePayloadBytes,
          payloadHashHex: nexusPayloadHashHex(fixturePayloadBytes),
          authority: fixture.transfer_input.authority,
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
    networkId: fixtureNetworkId,
    payloadBytes: fixturePayloadBytes,
    payloadHashHex: nexusPayloadHashHex(fixturePayloadBytes),
    authority: fixture.transfer_input.authority,
    signingPublicKey: fixturePublicKey,
    signatureAlgorithm: "ed25519",
  };
  const signature = { algorithm: "ed25519", signature: fixtureWalletSignature };
  const signedTransaction = fixtureSignedTransaction;
  const localHash = fixtureSignedTransactionHashHex;

  const mismatchClient = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
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
    (error) => {
      assert.ok(error instanceof NexusAppError);
      assert.equal(error.code, "invalid_submission_response");
      assert.equal(error.submissionState, "submitted");
      assert.equal(error.signedTransactionHashHex, localHash);
      return true;
    },
  );

  const submitFailureClient = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
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
    (error) => {
      assert.ok(error instanceof NexusAppError);
      assert.equal(error.code, "submission_outcome_unknown");
      assert.equal(error.submissionState, "unknown");
      assert.equal(error.signedTransactionHashHex, localHash);
      return true;
    },
  );

  const statusFailureClient = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
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
    (error) => {
      assert.ok(error instanceof NexusAppError);
      assert.equal(error.code, "status_wait_failed");
      assert.equal(error.submissionState, "submitted");
      assert.equal(error.signedTransactionHashHex, localHash);
      return true;
    },
  );
});

test("NexusAppClient reconciles all Torii External receipt hash aliases", async () => {
  const canonicalHash = fixtureSignedTransactionHashHex;
  const finalizer = {
    signedTransaction: fixtureSignedTransaction,
    hashHex: canonicalHash,
  };
  for (const submission of [
    { hashHex: canonicalHash, tx_hash: "d".repeat(64) },
    { hashHex: canonicalHash, payload: { entrypoint_hash: "d".repeat(64) } },
    { hashHex: canonicalHash, payload: { signed_transaction_hash: "d".repeat(64) } },
    {
      payload: {
        tx_hash: canonicalHash,
        entrypoint_hash: "d".repeat(64),
      },
    },
    { hashHex: "A".repeat(64) },
    { txHash: [256] },
  ]) {
    const harness = finalizationHarness(finalizer, submission);
    await assert.rejects(
      () =>
        harness.client.finalizeAndSubmit(
          fixtureSignable(),
          fixtureWalletSignature,
        ),
      (error) => {
        assert.ok(error instanceof NexusAppError);
        assert.equal(error.code, "invalid_submission_response");
        assert.equal(error.submissionState, "submitted");
        assert.equal(error.signedTransactionHashHex, canonicalHash);
        assert.equal(error.submission, submission);
        return true;
      },
    );
    assert.equal(harness.calls.submitted, 1);
    assert.equal(harness.calls.waited, 0);
  }

  const equivalent = finalizationHarness(finalizer, {
    hashHex: canonicalHash,
    hash: Buffer.from(canonicalHash, "hex"),
    transaction_hash: canonicalHash,
    entrypoint_hash: canonicalHash,
    tx_hash: Buffer.from(canonicalHash, "hex"),
    payload: {
      tx_hash: canonicalHash,
      entrypoint_hash: canonicalHash,
      signed_transaction_hash: canonicalHash,
    },
  });
  const receipt = await equivalent.client.finalizeAndSubmit(
    fixtureSignable(),
    fixtureWalletSignature,
  );
  assert.equal(receipt.signedTransactionHashHex, canonicalHash);
  assert.equal(
    receipt.submission.payload.signed_transaction_hash,
    canonicalHash,
    "the raw Torii receipt must retain the authoritative signed identity",
  );
  assert.equal(receipt.status.hash, canonicalHash);
  assert.deepEqual(equivalent.calls, { finalized: 1, submitted: 1, waited: 1 });
});

test("NexusAppClient aborts after finalization without submitting", async () => {
  const controller = new AbortController();
  const reason = new Error("cancelled-during-finalization");
  const harness = finalizationHarness(() => {
    controller.abort(reason);
    return {
      signedTransaction: fixtureSignedTransaction,
      hashHex: fixtureSignedTransactionHashHex,
    };
  });

  await assert.rejects(
    () =>
      harness.client.finalizeAndSubmit(
        fixtureSignable(),
        fixtureWalletSignature,
        { signal: controller.signal },
      ),
    (error) => {
      assert.ok(error instanceof NexusAppError);
      assert.equal(error.code, "operation_aborted");
      assert.equal(error.cause, reason);
      assert.equal(error.submissionState, "not_submitted");
      return true;
    },
  );
  assert.deepEqual(harness.calls, { finalized: 1, submitted: 0, waited: 0 });
});

test("NexusAppClient preserves falsey abort reasons", async () => {
  for (const reason of [0, false, "", null]) {
    const controller = new AbortController();
    controller.abort(reason);
    const harness = finalizationHarness({
      signedTransaction: fixtureSignedTransaction,
      hashHex: fixtureSignedTransactionHashHex,
    });

    await assert.rejects(
      () =>
        harness.client.finalizeAndSubmit(
          fixtureSignable(),
          fixtureWalletSignature,
          { signal: controller.signal },
        ),
      (error) => {
        assert.ok(error instanceof NexusAppError);
        assert.equal(error.code, "operation_aborted");
        assert.equal(error.cause, reason);
        assert.equal(error.submissionState, "not_submitted");
        return true;
      },
    );
    assert.deepEqual(harness.calls, { finalized: 0, submitted: 0, waited: 0 });
  }
});

test("NexusAppClient trusts intrinsic AbortSignal state over hostile shadows", async () => {
  const controller = new AbortController();
  const reason = new Error("intrinsic abort must stop Nexus submission");
  controller.abort(reason);
  Object.defineProperties(controller.signal, {
    aborted: { value: false },
    reason: { value: undefined },
    addEventListener: { value() {} },
    removeEventListener: { value() {} },
  });
  const harness = finalizationHarness({
    signedTransaction: fixtureSignedTransaction,
    hashHex: fixtureSignedTransactionHashHex,
  });

  await assert.rejects(
    () =>
      harness.client.finalizeAndSubmit(
        fixtureSignable(),
        fixtureWalletSignature,
        { signal: controller.signal },
      ),
    (error) => {
      assert.ok(error instanceof NexusAppError);
      assert.equal(error.code, "operation_aborted");
      assert.equal(error.cause, reason);
      assert.equal(error.submissionState, "not_submitted");
      return true;
    },
  );
  assert.deepEqual(harness.calls, { finalized: 0, submitted: 0, waited: 0 });
});

test("NexusAppClient reads the submit callback once before its final abort check", async () => {
  const controller = new AbortController();
  let submitReads = 0;
  let waitReads = 0;
  let submissions = 0;
  let waits = 0;
  const toriiClient = {};
  Object.defineProperty(toriiClient, "submitTransaction", {
    get() {
      submitReads += 1;
      if (submitReads > 1) {
        controller.abort(new Error("submit callback was read twice"));
      }
      return async function submitTransaction() {
        assert.equal(this, toriiClient);
        submissions += 1;
        return { hashHex: fixtureSignedTransactionHashHex };
      };
    },
  });
  Object.defineProperty(toriiClient, "waitForTransactionStatus", {
    get() {
      waitReads += 1;
      if (waitReads > 1) {
        controller.abort(new Error("wait callback was read twice"));
      }
      return async function waitForTransactionStatus() {
        assert.equal(this, toriiClient);
        waits += 1;
        return authoritativeAppliedStatus();
      };
    },
  });
  const client = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    signingPublicKey: fixturePublicKey,
    transactionCodec: {
      finalizeSignedTransaction() {
        return {
          signedTransaction: fixtureSignedTransaction,
          hashHex: fixtureSignedTransactionHashHex,
        };
      },
    },
    toriiClient,
  });

  const receipt = await client.finalizeAndSubmit(
    fixtureSignable(),
    fixtureWalletSignature,
    { signal: controller.signal },
  );

  assert.equal(submitReads, 1);
  assert.equal(waitReads, 1);
  assert.equal(submissions, 1);
  assert.equal(waits, 1);
  assert.equal(controller.signal.aborted, false);
  assert.equal(receipt.signedTransactionHashHex, fixtureSignedTransactionHashHex);
});

test("NexusAppClient honors aborts from Torii capability getters before dispatch", async () => {
  for (const abortingField of [
    "submitTransaction",
    "waitForTransactionStatus",
  ]) {
    const controller = new AbortController();
    const reason = new Error(`cancelled by ${abortingField} getter`);
    let submitReads = 0;
    let waitReads = 0;
    let submissions = 0;
    let waits = 0;
    const toriiClient = {};
    Object.defineProperty(toriiClient, "submitTransaction", {
      get() {
        submitReads += 1;
        if (abortingField === "submitTransaction") controller.abort(reason);
        return async function submitTransaction() {
          submissions += 1;
          return { hashHex: fixtureSignedTransactionHashHex };
        };
      },
    });
    Object.defineProperty(toriiClient, "waitForTransactionStatus", {
      get() {
        waitReads += 1;
        if (abortingField === "waitForTransactionStatus") {
          controller.abort(reason);
        }
        return async function waitForTransactionStatus() {
          waits += 1;
          return authoritativeAppliedStatus();
        };
      },
    });
    const client = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
      signingPublicKey: fixturePublicKey,
      transactionCodec: {
        finalizeSignedTransaction() {
          return {
            signedTransaction: fixtureSignedTransaction,
            hashHex: fixtureSignedTransactionHashHex,
          };
        },
      },
      toriiClient,
    });

    await assert.rejects(
      () =>
        client.finalizeAndSubmit(fixtureSignable(), fixtureWalletSignature, {
          signal: controller.signal,
        }),
      (error) => {
        assert.ok(error instanceof NexusAppError);
        assert.equal(error.code, "operation_aborted");
        assert.equal(error.cause, reason);
        assert.equal(error.submissionState, "not_submitted");
        return true;
      },
    );
    assert.equal(submitReads, 1, abortingField);
    assert.equal(
      waitReads,
      abortingField === "submitTransaction" ? 0 : 1,
      abortingField,
    );
    assert.equal(submissions, 0, abortingField);
    assert.equal(waits, 0, abortingField);
  }
});

test("NexusAppClient requires and snapshots wait capabilities before submission", async () => {
  const missingWaiter = finalizationHarness({
    signedTransaction: fixtureSignedTransaction,
    hashHex: fixtureSignedTransactionHashHex,
  });
  delete missingWaiter.client.toriiClient.waitForTransactionStatus;
  await assert.rejects(
    () =>
      missingWaiter.client.finalizeAndSubmit(
        fixtureSignable(),
        fixtureWalletSignature,
      ),
    (error) => {
      assert.ok(error instanceof NexusAppError);
      assert.equal(error.code, "status_wait_unavailable");
      assert.equal(error.submissionState, "not_submitted");
      return true;
    },
  );
  assert.deepEqual(missingWaiter.calls, {
    finalized: 1,
    submitted: 0,
    waited: 0,
  });

  for (const field of ["submitTransaction", "waitForTransactionStatus"]) {
    const reason = new Error(`${field} getter failed`);
    let submissions = 0;
    const toriiClient = {
      async submitTransaction() {
        submissions += 1;
        return { hashHex: fixtureSignedTransactionHashHex };
      },
      async waitForTransactionStatus() {
        return authoritativeAppliedStatus();
      },
    };
    Object.defineProperty(toriiClient, field, {
      get() {
        throw reason;
      },
    });
    const client = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
      signingPublicKey: fixturePublicKey,
      transactionCodec: {
        finalizeSignedTransaction() {
          return {
            signedTransaction: fixtureSignedTransaction,
            hashHex: fixtureSignedTransactionHashHex,
          };
        },
      },
      toriiClient,
    });
    await assert.rejects(
      () => client.finalizeAndSubmit(fixtureSignable(), fixtureWalletSignature),
      (error) => {
        assert.ok(error instanceof NexusAppError);
        assert.equal(
          error.code,
          field === "submitTransaction"
            ? "torii_client_unavailable"
            : "status_wait_unavailable",
        );
        assert.equal(error.cause, reason);
        assert.equal(error.submissionState, "not_submitted");
        return true;
      },
    );
    assert.equal(submissions, 0);
  }
});

test("NexusAppClient checks cancellation between finalizer and capability access", async () => {
  const controller = new AbortController();
  const reason = new Error("cancelled by finalizer getter");
  let finalizerReads = 0;
  let finalizerCalls = 0;
  let submitReads = 0;
  const transactionCodec = {};
  Object.defineProperty(transactionCodec, "finalizeSignedTransaction", {
    get() {
      finalizerReads += 1;
      controller.abort(reason);
      return function finalizeSignedTransaction() {
        finalizerCalls += 1;
        return {
          signedTransaction: fixtureSignedTransaction,
          hashHex: fixtureSignedTransactionHashHex,
        };
      };
    },
  });
  const toriiClient = {
    get submitTransaction() {
      submitReads += 1;
      return async () => ({ hashHex: fixtureSignedTransactionHashHex });
    },
    async waitForTransactionStatus() {
      return authoritativeAppliedStatus();
    },
  };
  const client = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    signingPublicKey: fixturePublicKey,
    transactionCodec,
    toriiClient,
  });

  await assert.rejects(
    () =>
      client.finalizeAndSubmit(fixtureSignable(), fixtureWalletSignature, {
        signal: controller.signal,
      }),
    (error) => {
      assert.ok(error instanceof NexusAppError);
      assert.equal(error.code, "operation_aborted");
      assert.equal(error.cause, reason);
      return true;
    },
  );
  assert.equal(finalizerReads, 1);
  assert.equal(finalizerCalls, 0);
  assert.equal(submitReads, 0);
});

test("NexusAppClient snapshots a custom finalizer and preserves its receiver", async () => {
  let finalizerReads = 0;
  let finalizerCalls = 0;
  const transactionCodec = {};
  let client;
  Object.defineProperty(transactionCodec, "finalizeSignedTransaction", {
    get() {
      finalizerReads += 1;
      if (finalizerReads > 1) throw new Error("finalizer read twice");
      client.transactionCodec = {
        finalizeSignedTransaction() {
          throw new Error("replacement codec must not become the receiver");
        },
      };
      return function finalizeSignedTransaction() {
        assert.equal(this, transactionCodec);
        finalizerCalls += 1;
        return {
          signedTransaction: fixtureSignedTransaction,
          hashHex: fixtureSignedTransactionHashHex,
        };
      };
    },
  });
  client = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    signingPublicKey: fixturePublicKey,
    transactionCodec,
    toriiClient: {
      async submitTransaction() {
        return { hashHex: fixtureSignedTransactionHashHex };
      },
    },
  });

  await client.finalizeAndSubmit(fixtureSignable(), fixtureWalletSignature, {
    wait: false,
  });
  assert.equal(finalizerReads, 1);
  assert.equal(finalizerCalls, 1);

  const reason = new Error("finalizer getter failed");
  const throwingCodec = {};
  Object.defineProperty(throwingCodec, "finalizeSignedTransaction", {
    get() {
      throw reason;
    },
  });
  let submissions = 0;
  const throwingClient = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    signingPublicKey: fixturePublicKey,
    transactionCodec: throwingCodec,
    toriiClient: {
      async submitTransaction() {
        submissions += 1;
      },
    },
  });
  await assert.rejects(
    () =>
      throwingClient.finalizeAndSubmit(
        fixtureSignable(),
        fixtureWalletSignature,
        { wait: false },
      ),
    (error) => {
      assert.ok(error instanceof NexusAppError);
      assert.equal(error.code, "invalid_transaction_codec");
      assert.equal(error.cause, reason);
      assert.equal(error.phase, "finalization");
      return true;
    },
  );
  assert.equal(submissions, 0);
});

test("NexusAppClient reports cancellation during submission as already submitted", async () => {
  const controller = new AbortController();
  const reason = new Error("cancelled while Torii accepted the transaction");
  const submission = { hashHex: fixtureSignedTransactionHashHex };
  let waits = 0;
  const client = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    signingPublicKey: fixturePublicKey,
    transactionCodec: {
      finalizeSignedTransaction() {
        return {
          signedTransaction: fixtureSignedTransaction,
          hashHex: fixtureSignedTransactionHashHex,
        };
      },
    },
    toriiClient: {
      async submitTransaction() {
        controller.abort(reason);
        return submission;
      },
      async waitForTransactionStatus() {
        waits += 1;
        return authoritativeAppliedStatus();
      },
    },
  });

  await assert.rejects(
    () =>
      client.finalizeAndSubmit(fixtureSignable(), fixtureWalletSignature, {
        signal: controller.signal,
      }),
    (error) => {
      assert.ok(error instanceof NexusAppError);
      assert.equal(error.code, "status_wait_aborted");
      assert.equal(error.cause, reason);
      assert.equal(error.submissionState, "submitted");
      assert.equal(error.signedTransactionHashHex, fixtureSignedTransactionHashHex);
      assert.equal(error.submission, submission);
      assert.throws(() => {
        error.submissionState = "not_submitted";
      }, TypeError);
      return true;
    },
  );
  assert.equal(waits, 0);
});

test("NexusAppClient enforces abort and timeout around injected waiters", async () => {
  const controller = new AbortController();
  const reason = new Error("stop an uncooperative waiter");
  let waiterStarted;
  const started = new Promise((resolve) => {
    waiterStarted = resolve;
  });
  const client = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    signingPublicKey: fixturePublicKey,
    transactionCodec: {
      finalizeSignedTransaction() {
        return {
          signedTransaction: fixtureSignedTransaction,
          hashHex: fixtureSignedTransactionHashHex,
        };
      },
    },
    toriiClient: {
      async submitTransaction() {
        return { hashHex: fixtureSignedTransactionHashHex };
      },
      waitForTransactionStatus() {
        waiterStarted();
        return new Promise(() => {});
      },
    },
  });
  const pending = client.finalizeAndSubmit(
    fixtureSignable(),
    fixtureWalletSignature,
    { signal: controller.signal, timeoutMs: null },
  );
  await started;
  controller.abort(reason);
  await assert.rejects(pending, (error) => {
    assert.ok(error instanceof NexusAppError);
    assert.equal(error.code, "status_wait_aborted");
    assert.equal(error.cause, reason);
    assert.equal(error.submissionState, "submitted");
    return true;
  });

  const timed = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    signingPublicKey: fixturePublicKey,
    transactionCodec: {
      finalizeSignedTransaction() {
        return {
          signedTransaction: fixtureSignedTransaction,
          hashHex: fixtureSignedTransactionHashHex,
        };
      },
    },
    toriiClient: {
      async submitTransaction() {
        return { hashHex: fixtureSignedTransactionHashHex };
      },
      waitForTransactionStatus() {
        return new Promise(() => {});
      },
    },
  });
  await assert.rejects(
    () =>
      timed.finalizeAndSubmit(fixtureSignable(), fixtureWalletSignature, {
        timeoutMs: 5,
      }),
    (error) => {
      assert.ok(error instanceof NexusAppError);
      assert.equal(error.code, "status_wait_timeout");
      assert.equal(error.submissionState, "submitted");
      assert.equal(error.signedTransactionHashHex, fixtureSignedTransactionHashHex);
      return true;
    },
  );
});

test("NexusAppClient observes waiter rejection after synchronous cancellation", async () => {
  const controller = new AbortController();
  const abortReason = new Error("waiter cancelled itself");
  const lateRejection = new Error("late waiter rejection");
  let thenCalls = 0;
  const client = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    signingPublicKey: fixturePublicKey,
    transactionCodec: {
      finalizeSignedTransaction() {
        return {
          signedTransaction: fixtureSignedTransaction,
          hashHex: fixtureSignedTransactionHashHex,
        };
      },
    },
    toriiClient: {
      async submitTransaction() {
        return { hashHex: fixtureSignedTransactionHashHex };
      },
      waitForTransactionStatus() {
        controller.abort(abortReason);
        return {
          then(_resolve, reject) {
            thenCalls += 1;
            queueMicrotask(() => reject(lateRejection));
          },
        };
      },
    },
  });

  await assert.rejects(
    () =>
      client.finalizeAndSubmit(fixtureSignable(), fixtureWalletSignature, {
        signal: controller.signal,
        timeoutMs: null,
      }),
    (error) => {
      assert.ok(error instanceof NexusAppError);
      assert.equal(error.code, "status_wait_aborted");
      assert.equal(error.cause, abortReason);
      return true;
    },
  );
  await new Promise((resolve) => setTimeout(resolve, 0));
  assert.equal(thenCalls, 1);
});

test("NexusAppClient never reads hostile rejection messages", async () => {
  const hostile = {};
  let messageReads = 0;
  Object.defineProperty(hostile, "message", {
    get() {
      messageReads += 1;
      throw new Error("message getter must not run");
    },
  });
  const createClient = (toriiClient) =>
    new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
      signingPublicKey: fixturePublicKey,
      transactionCodec: {
        finalizeSignedTransaction() {
          return {
            signedTransaction: fixtureSignedTransaction,
            hashHex: fixtureSignedTransactionHashHex,
          };
        },
      },
      toriiClient,
    });
  await assert.rejects(
    () =>
      createClient({
        async submitTransaction() {
          throw hostile;
        },
      }).finalizeAndSubmit(fixtureSignable(), fixtureWalletSignature, {
        wait: false,
      }),
    (error) => {
      assert.ok(error instanceof NexusAppError);
      assert.equal(error.code, "submission_outcome_unknown");
      assert.equal(error.cause, hostile);
      return true;
    },
  );
  await assert.rejects(
    () =>
      createClient({
        async submitTransaction() {
          return { hashHex: fixtureSignedTransactionHashHex };
        },
        async waitForTransactionStatus() {
          throw hostile;
        },
      }).finalizeAndSubmit(fixtureSignable(), fixtureWalletSignature),
    (error) => {
      assert.ok(error instanceof NexusAppError);
      assert.equal(error.code, "status_wait_failed");
      assert.equal(error.cause, hostile);
      return true;
    },
  );
  assert.equal(messageReads, 0);
});

test("NexusAppClient rejects status-only options when waiting is disabled", async () => {
  const statusOnlyOptions = {
    intervalMs: 0,
    timeoutMs: null,
    maxAttempts: 1,
    scope: "local",
    successStatuses: ["Committed"],
    failureStatuses: ["Rejected"],
    terminalStatuses: ["Committed"],
    onStatus() {},
    signal: new AbortController().signal,
  };
  for (const [field, value] of Object.entries(statusOnlyOptions)) {
    const harness = finalizationHarness({
      signedTransaction: fixtureSignedTransaction,
      hashHex: fixtureSignedTransactionHashHex,
    });
    await assert.rejects(
      () =>
        harness.client.finalizeAndSubmit(
          fixtureSignable(),
          fixtureWalletSignature,
          { wait: false, [field]: value },
        ),
      (error) => {
        assert.ok(error instanceof NexusAppError);
        assert.equal(error.code, "invalid_finalize_options");
        assert.match(error.message, new RegExp(`\\.${field} `, "u"));
        return true;
      },
    );
    assert.deepEqual(harness.calls, {
      finalized: 0,
      submitted: 0,
      waited: 0,
    });
  }
});

test("NexusAppClient prevalidates all wait options before Torii side effects", async () => {
  const aborted = new AbortController();
  aborted.abort(new Error("cancelled-before-submit"));
  const invalidOptions = [
    { successStatuses: ["Committed"] },
    { failureStatuses: ["Rejected"] },
    { failureStatuses: ["Applied"] },
    { terminalStatuses: ["Committed"] },
    { intervalMs: -1 },
    { timeoutMs: Number.MAX_SAFE_INTEGER + 1 },
    { maxAttempts: 0 },
    { scope: undefined },
    { scope: null },
    { scope: "global" },
    { onStatus: "not-a-callback" },
    { signal: { aborted: false } },
    { signal: aborted.signal },
    { wait: "yes" },
  ];
  for (const options of invalidOptions) {
    const harness = finalizationHarness({
      signedTransaction: fixtureSignedTransaction,
      hashHex: fixtureSignedTransactionHashHex,
    });
    await assert.rejects(
      () =>
        harness.client.finalizeAndSubmit(
          fixtureSignable(),
          fixtureWalletSignature,
          options,
        ),
      Error,
      JSON.stringify(options),
    );
    assert.deepEqual(
      harness.calls,
      { finalized: 0, submitted: 0, waited: 0 },
      "invalid wait options must fail before finalizer, submit, and wait callbacks",
    );
  }

  let observedOptions = null;
  const fixedPolicy = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    signingPublicKey: fixturePublicKey,
    transactionCodec: {
      finalizeSignedTransaction() {
        return {
          signedTransaction: fixtureSignedTransaction,
          hashHex: fixtureSignedTransactionHashHex,
        };
      },
    },
    toriiClient: {
      async submitTransaction() {
        return { hashHex: fixtureSignedTransactionHashHex };
      },
      async waitForTransactionStatus(_hashHex, options) {
        observedOptions = options;
        return authoritativeAppliedStatus();
      },
    },
  });
  await fixedPolicy.finalizeAndSubmit(
    fixtureSignable(),
    fixtureWalletSignature,
    {
      intervalMs: 0,
    },
  );
  assert.equal("successStatuses" in observedOptions, false);
  assert.equal("failureStatuses" in observedOptions, false);
  assert.equal("terminalStatuses" in observedOptions, false);
  assert.equal("scope" in observedOptions, false);
  assert.equal(observedOptions.intervalMs, 0);
});

test("NexusAppClient rejects a delegated waiter that returns before Applied", async () => {
  const client = new NexusAppClient({
    chainDiscriminant: fixtureChainDiscriminant,
    signingPublicKey: fixturePublicKey,
    transactionCodec: {
      finalizeSignedTransaction() {
        return {
          signedTransaction: fixtureSignedTransaction,
          hashHex: fixtureSignedTransactionHashHex,
        };
      },
    },
    toriiClient: {
      async submitTransaction() {
        return { hashHex: fixtureSignedTransactionHashHex };
      },
      async waitForTransactionStatus() {
        return { status: "Committed" };
      },
    },
  });

  await assert.rejects(
    () => client.finalizeAndSubmit(fixtureSignable(), fixtureWalletSignature),
    (error) => {
      assert.ok(error instanceof NexusAppError);
      assert.equal(error.code, "status_wait_non_applied");
      assert.equal(error.submissionState, "submitted");
      return true;
    },
  );
});
