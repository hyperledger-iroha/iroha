"use strict";

import test from "node:test";
import assert from "node:assert/strict";

import {
  buildExecuteTriggerInstruction,
  buildExecuteTriggerNorito,
  buildMultisigTriggerArgs,
  isMultisigSignerAuthorized,
  buildMultisigExecuteTriggerInstruction,
  buildProposeMultisigExecuteTriggerInstruction,
  buildMultisigContractCallProposeRequest,
  buildMultisigContractCallApproveRequest,
  ValidationError,
  ValidationErrorCode,
} from "../src/index.js";
import { MultisigSpecBuilder } from "../src/multisig.js";
import { AccountAddress } from "../src/address.js";
import { noritoDecodeInstruction } from "../src/norito.js";

const ALICE_KEY = Buffer.from(
  "B935AAF1F4E44B3DB79E5E5A9BA4569E6F3E2310C219F3DDD56D3277828D5480",
  "hex",
);
const BOB_KEY = Buffer.from(
  "641297079357229F295938A4B5A333DE35069BF47B9D0704E45805713D13C201",
  "hex",
);
const CAROL_KEY = Buffer.from(
  "D7C74E6FC2B79C3C7CDA4B6B5BFCB0DDE0F2B6B5FB3E27391C40CC8E7A1A1C11",
  "hex",
);
const CONTROLLER_KEY = Buffer.from(
  "B7D3A8A20C1EF77F6C2B7B4AA3AA7B4D52A7B2FAF77F0F45B1A16E7A8E0B3C01",
  "hex",
);

const ALICE_ID = AccountAddress.fromAccount({ publicKey: ALICE_KEY }).toI105();
const BOB_ID = AccountAddress.fromAccount({ publicKey: BOB_KEY }).toI105();
const CAROL_ID = AccountAddress.fromAccount({ publicKey: CAROL_KEY }).toI105();
const CONTROLLER_ID = AccountAddress.fromAccount({ publicKey: CONTROLLER_KEY }).toI105();

function sampleSpec() {
  return new MultisigSpecBuilder()
    .setQuorum(2)
    .setTransactionTtlMs(60_000)
    .addSignatory(ALICE_ID, 1)
    .addSignatory(BOB_ID, 1)
    .build();
}

test("buildExecuteTriggerInstruction and buildExecuteTriggerNorito round-trip canonically", () => {
  const instruction = buildExecuteTriggerInstruction("staged_mint_request_hbl", {
    action: "create",
    request_id: "mr1",
  });
  assert.deepEqual(instruction, {
    ExecuteTrigger: {
      trigger: "staged_mint_request_hbl",
      args: {
        action: "create",
        request_id: "mr1",
      },
    },
  });

  const encoded = buildExecuteTriggerNorito("staged_mint_request_hbl", {
    action: "create",
    request_id: "mr1",
  });
  assert.deepEqual(noritoDecodeInstruction(encoded), instruction);
});

test("buildMultisigTriggerArgs normalizes lifecycle and lookup presets", () => {
  assert.deepEqual(
    buildMultisigTriggerArgs("lifecycle", {
      action: "create",
      requestId: "req-1",
      fiId: "banka",
      toAccountId: ALICE_ID,
      amountI64: "42",
      requestedByActorId: { account: BOB_ID },
      createdAtMs: 1,
      expiresAtMs: 2,
    }),
    {
      action: "create",
      request_id: "req-1",
      fi_id: "banka",
      to_account_id: ALICE_ID,
      amount_i64: 42,
      requested_by_actor_id: { account: BOB_ID },
      created_at_ms: 1,
      expires_at_ms: 2,
    },
  );

  assert.deepEqual(
    buildMultisigTriggerArgs("lookup", {
      requestId: "req-2",
      requestedByActorId: { account: ALICE_ID },
    }),
    {
      request_id: "req-2",
      requested_by_actor_id: { account: ALICE_ID },
    },
  );
});

test("isMultisigSignerAuthorized recognizes configured signers", () => {
  const spec = sampleSpec();
  assert.equal(isMultisigSignerAuthorized(spec, ALICE_ID), true);
  assert.equal(isMultisigSignerAuthorized(spec, CAROL_ID), false);
});

test("buildMultisigExecuteTriggerInstruction enforces strict signer checks when requested", () => {
  const spec = sampleSpec();
  assert.deepEqual(
    buildMultisigExecuteTriggerInstruction({
      trigger: "staged_mint_request_hbl",
      argPreset: "lookup",
      argInput: { requestId: "req-3" },
      signerAccountId: ALICE_ID,
      multisigSpec: spec,
      strictSignerCheck: true,
    }),
    {
      ExecuteTrigger: {
        trigger: "staged_mint_request_hbl",
        args: {
          request_id: "req-3",
        },
      },
    },
  );

  assert.throws(
    () =>
      buildMultisigExecuteTriggerInstruction({
        trigger: "staged_mint_request_hbl",
        argPreset: "lookup",
        argInput: { requestId: "req-4" },
        signerAccountId: CAROL_ID,
        multisigSpec: spec,
        strictSignerCheck: true,
      }),
    (error) =>
      error instanceof ValidationError &&
      error.code === ValidationErrorCode.INVALID_ACCOUNT_ID &&
      /not present in multisigSpec\.signatories/.test(error.message),
  );
});

test("buildProposeMultisigExecuteTriggerInstruction wraps a single ExecuteTrigger proposal", () => {
  const spec = sampleSpec();
  const payload = buildProposeMultisigExecuteTriggerInstruction({
    accountId: CONTROLLER_ID,
    trigger: "staged_mint_request_hbl",
    argPreset: "lookup",
    argInput: { requestId: "req-5" },
    signerAccountId: BOB_ID,
    spec,
    strictSignerCheck: true,
    transactionTtlMs: 45_000,
  });

  assert.deepEqual(payload, {
    Custom: {
      payload: {
        Propose: {
          account: CONTROLLER_ID,
          instructions: [
            {
              ExecuteTrigger: {
                trigger: "staged_mint_request_hbl",
                args: {
                  request_id: "req-5",
                },
              },
            },
          ],
          transaction_ttl_ms: 45_000,
        },
      },
    },
  });
});

test("buildMultisigContractCallProposeRequest builds normalized Torii payloads", () => {
  const spec = sampleSpec();
  const payload = buildMultisigContractCallProposeRequest({
    multisigAccountAlias: "mintops@banka",
    signerAccountId: ALICE_ID,
    contractAddress: "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
    entrypoint: "execute",
    trigger: "staged_mint_request_hbl",
    argPreset: "lifecycle",
    argInput: {
      action: "create",
      requestId: "req-6",
      toAccountId: BOB_ID,
      amountI64: 10,
    },
    multisigSpec: spec,
    strictSignerCheck: true,
    gasAssetId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
    gasLimit: 5,
  });

  assert.deepEqual(payload, {
    multisig_account_alias: "mintops@banka",
    signer_account_id: ALICE_ID,
    contract_address: "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
    entrypoint: "execute",
    payload: {
      trigger: "staged_mint_request_hbl",
      args: {
        action: "create",
        request_id: "req-6",
        to_account_id: BOB_ID,
        amount_i64: 10,
      },
    },
    gas_asset_id: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
    gas_limit: 5,
  });
});

test("buildMultisigContractCallApproveRequest normalizes selector and lookup keys", () => {
  const payload = buildMultisigContractCallApproveRequest({
    multisigAccountId: CONTROLLER_ID,
    signerAccountId: BOB_ID,
    instructionsHash: "AA".repeat(32),
    signatureB64: "AQ==",
  });

  assert.deepEqual(payload, {
    multisig_account_id: CONTROLLER_ID,
    signer_account_id: BOB_ID,
    instructions_hash: "aa".repeat(32),
    signature_b64: "AQ==",
  });
});

test("buildMultisigContractCallProposeRequest accepts detached private key variants", () => {
  const fromHex = buildMultisigContractCallProposeRequest({
    multisigAccountId: CONTROLLER_ID,
    signerAccountId: ALICE_ID,
    contractAddress: "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
    entrypoint: "execute",
    trigger: "staged_mint_request_hbl",
    args: { request_id: "req-7" },
    privateKeyHex: "AA".repeat(32),
    privateKeyAlgorithm: "ml-dsa",
  });
  assert.equal(fromHex.private_key, `ml-dsa:${"aa".repeat(32)}`);

  const fromBytes = buildMultisigContractCallProposeRequest({
    multisigAccountAlias: "mintops@banka",
    signerAccountId: ALICE_ID,
    contractAddress: "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
    entrypoint: "execute",
    trigger: "staged_mint_request_hbl",
    args: { request_id: "req-8" },
    privateKeyBytes: Buffer.alloc(32, 0x11),
  });
  assert.equal(fromBytes.private_key, `ed25519:${"11".repeat(32)}`);

  const multihash = buildMultisigContractCallProposeRequest({
    multisigAccountAlias: "mintops@banka",
    signerAccountId: ALICE_ID,
    contractAddress: "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
    entrypoint: "execute",
    trigger: "staged_mint_request_hbl",
    args: { request_id: "req-9" },
    privateKeyMultihash: "ed25519:prebuilt",
  });
  assert.equal(multihash.private_key, "ed25519:prebuilt");
});

test("buildMultisigContractCallApproveRequest accepts snake_case detached key fields", () => {
  const payload = buildMultisigContractCallApproveRequest({
    multisig_account_id: CONTROLLER_ID,
    signer_account_id: BOB_ID,
    instructions_hash: "AA".repeat(32),
    private_key_hex: "22".repeat(32),
    private_key_algorithm: "sm2",
  });

  assert.deepEqual(payload, {
    multisig_account_id: CONTROLLER_ID,
    signer_account_id: BOB_ID,
    instructions_hash: "aa".repeat(32),
    private_key: `sm2:${"22".repeat(32)}`,
  });
});
