import test from "node:test";
import assert from "node:assert/strict";

process.env.IROHA_JS_DISABLE_NATIVE = "1";

const { AccountAddress } = await import("../src/address.js");
const {
  buildRegisterRwaInstruction,
  buildTransferRwaInstruction,
  buildMergeRwasInstruction,
  buildRedeemRwaInstruction,
  buildFreezeRwaInstruction,
  buildUnfreezeRwaInstruction,
  buildHoldRwaInstruction,
  buildReleaseRwaInstruction,
  buildForceTransferRwaInstruction,
  buildSetRwaControlsInstruction,
  buildSetRwaKeyValueInstruction,
  buildRemoveRwaKeyValueInstruction,
} = await import("../src/instructionBuilders.js");
const {
  buildRegisterRwaTransaction,
  buildTransferRwaTransaction,
  buildSetRwaKeyValueTransaction,
  buildRemoveRwaKeyValueTransaction,
} = await import("../src/transaction.js");

const AUTHORITY = AccountAddress.fromAccount({
  domain: "wonderland",
  publicKey: Buffer.from(
    "ce7fa46c9dce7ea4b125e2e36bdb63ea33073e7590ac92816ae1e861b7048b03",
    "hex",
  ),
}).toI105();
const DESTINATION = AccountAddress.fromAccount({
  domain: "wonderland",
  publicKey: Buffer.from(
    "641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201",
    "hex",
  ),
}).toI105();
const PRIVATE_KEY = Buffer.alloc(32, 0x11);
const RWA_ID =
  "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef$commodities";
const RWA_ID_HASH_UPPER =
  "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF$commodities";

test("RWA instruction builders normalize canonical payloads in JS-only mode", () => {
  const register = buildRegisterRwaInstruction({
    rwa: {
      domain: "commodities",
      quantity: "10.5",
      spec: { scale: 1 },
      primaryReference: "vault-cert-001",
      metadata: { origin: "AE" },
      parents: [{ rwa: RWA_ID_HASH_UPPER, quantity: "1.25" }],
    },
  });
  const transfer = buildTransferRwaInstruction({
    sourceAccountId: AUTHORITY,
    rwaId: RWA_ID_HASH_UPPER,
    quantity: "2.5",
    destinationAccountId: DESTINATION,
  });
  const merge = buildMergeRwasInstruction({
    merge: {
      parents: [{ rwa: RWA_ID, quantity: "3" }],
      primaryReference: "blend-001",
      metadata: { grade: "A" },
    },
  });
  const redeem = buildRedeemRwaInstruction({ rwaId: RWA_ID, quantity: 1 });
  const freeze = buildFreezeRwaInstruction({ rwaId: RWA_ID });
  const unfreeze = buildUnfreezeRwaInstruction({ rwaId: RWA_ID });
  const hold = buildHoldRwaInstruction({ rwaId: RWA_ID, quantity: "0.5" });
  const release = buildReleaseRwaInstruction({ rwaId: RWA_ID, quantity: "0.25" });
  const forceTransfer = buildForceTransferRwaInstruction({
    rwaId: RWA_ID,
    quantity: "1.5",
    destinationAccountId: DESTINATION,
  });
  const controls = buildSetRwaControlsInstruction({
    rwaId: RWA_ID,
    controls: {
      controllerAccounts: [AUTHORITY],
      freezeEnabled: true,
    },
  });
  const setMetadata = buildSetRwaKeyValueInstruction({
    rwaId: RWA_ID,
    key: "grade",
    value: { origin: "AE", lot: BigInt(3) },
  });
  const removeMetadata = buildRemoveRwaKeyValueInstruction({
    rwaId: RWA_ID,
    key: "grade",
  });

  assert.deepEqual(register, {
    RegisterRwa: {
      rwa: {
        domain: "commodities",
        quantity: "10.5",
        spec: { scale: 1 },
        primary_reference: "vault-cert-001",
        status: null,
        metadata: { origin: "AE" },
        parents: [{ rwa: RWA_ID, quantity: "1.25" }],
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
  assert.deepEqual(transfer, {
    TransferRwa: {
      source: AUTHORITY,
      rwa: RWA_ID,
      quantity: "2.5",
      destination: DESTINATION,
    },
  });
  assert.deepEqual(merge, {
    MergeRwas: {
      parents: [{ rwa: RWA_ID, quantity: "3" }],
      primary_reference: "blend-001",
      status: null,
      metadata: { grade: "A" },
    },
  });
  assert.deepEqual(redeem, { RedeemRwa: { rwa: RWA_ID, quantity: "1" } });
  assert.deepEqual(freeze, { FreezeRwa: { rwa: RWA_ID } });
  assert.deepEqual(unfreeze, { UnfreezeRwa: { rwa: RWA_ID } });
  assert.deepEqual(hold, { HoldRwa: { rwa: RWA_ID, quantity: "0.5" } });
  assert.deepEqual(release, { ReleaseRwa: { rwa: RWA_ID, quantity: "0.25" } });
  assert.deepEqual(forceTransfer, {
    ForceTransferRwa: {
      rwa: RWA_ID,
      quantity: "1.5",
      destination: DESTINATION,
    },
  });
  assert.deepEqual(controls, {
    SetRwaControls: {
      rwa: RWA_ID,
      controls: {
        controller_accounts: [AUTHORITY],
        controller_roles: [],
        freeze_enabled: true,
        hold_enabled: false,
        force_transfer_enabled: false,
        redeem_enabled: false,
      },
    },
  });
  assert.deepEqual(setMetadata, {
    SetRwaKeyValue: {
      rwa: RWA_ID,
      key: "grade",
      value: { origin: "AE", lot: "3" },
    },
  });
  assert.deepEqual(removeMetadata, {
    RemoveRwaKeyValue: {
      rwa: RWA_ID,
      key: "grade",
    },
  });
});

test("RWA transaction builders serialize canonical instructions through injected native binding", () => {
  const captures = [];
  globalThis.__IROHA_NATIVE_BINDING__ = {
    buildTransaction(
      chainId,
      authority,
      instructions,
      metadataPayload,
      creationTimeMs,
      ttlMs,
      nonce,
      secret,
    ) {
      captures.push({
        chainId,
        authority,
        instructions,
        metadataPayload,
        creationTimeMs,
        ttlMs,
        nonce,
        secret,
      });
      return {
        signed_transaction: Buffer.from([0x01, 0x02, 0x03]),
        hash: Buffer.alloc(32, 0xaa),
      };
    },
  };

  try {
    const register = buildRegisterRwaTransaction({
      chainId: "test-chain",
      authority: AUTHORITY,
      rwa: {
        domain: "commodities",
        quantity: "10.5",
        spec: { scale: 1 },
        primaryReference: "vault-cert-001",
      },
      metadata: { tag: "register" },
      creationTimeMs: 10,
      ttlMs: 20,
      nonce: 5,
      privateKey: PRIVATE_KEY,
    });
    const transfer = buildTransferRwaTransaction({
      chainId: "test-chain",
      authority: AUTHORITY,
      sourceAccountId: AUTHORITY,
      rwaId: RWA_ID_HASH_UPPER,
      quantity: "2.5",
      destinationAccountId: DESTINATION,
      privateKey: PRIVATE_KEY,
    });
    const setMetadata = buildSetRwaKeyValueTransaction({
      chainId: "test-chain",
      authority: AUTHORITY,
      rwaId: RWA_ID,
      key: "grade",
      value: { score: BigInt(9) },
      privateKey: PRIVATE_KEY,
    });
    const removeMetadata = buildRemoveRwaKeyValueTransaction({
      chainId: "test-chain",
      authority: AUTHORITY,
      rwaId: RWA_ID,
      key: "grade",
      privateKey: PRIVATE_KEY,
    });

    assert.equal(register.hash.length, 32);
    assert.equal(transfer.hash.length, 32);
    assert.equal(setMetadata.hash.length, 32);
    assert.equal(removeMetadata.hash.length, 32);
    assert.deepEqual(register.signedTransaction, Buffer.from([0x01, 0x02, 0x03]));
    assert.deepEqual(transfer.signedTransaction, Buffer.from([0x01, 0x02, 0x03]));
    assert.deepEqual(setMetadata.signedTransaction, Buffer.from([0x01, 0x02, 0x03]));
    assert.deepEqual(removeMetadata.signedTransaction, Buffer.from([0x01, 0x02, 0x03]));
  } finally {
    delete globalThis.__IROHA_NATIVE_BINDING__;
  }

  assert.equal(captures.length, 4);
  assert.equal(captures[0].chainId, "test-chain");
  assert.equal(captures[0].authority, AUTHORITY);
  assert.equal(captures[0].metadataPayload, JSON.stringify({ tag: "register" }));
  assert.equal(captures[0].creationTimeMs, 10);
  assert.equal(captures[0].ttlMs, 20);
  assert.equal(captures[0].nonce, 5);
  assert.deepEqual(Buffer.from(captures[0].secret), PRIVATE_KEY);
  assert.deepEqual(JSON.parse(captures[0].instructions[0]), {
    RegisterRwa: {
      rwa: {
        domain: "commodities",
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
  assert.deepEqual(JSON.parse(captures[1].instructions[0]), {
    TransferRwa: {
      source: AUTHORITY,
      rwa: RWA_ID,
      quantity: "2.5",
      destination: DESTINATION,
    },
  });
  assert.deepEqual(JSON.parse(captures[2].instructions[0]), {
    SetRwaKeyValue: {
      rwa: RWA_ID,
      key: "grade",
      value: { score: "9" },
    },
  });
  assert.deepEqual(JSON.parse(captures[3].instructions[0]), {
    RemoveRwaKeyValue: {
      rwa: RWA_ID,
      key: "grade",
    },
  });
});
