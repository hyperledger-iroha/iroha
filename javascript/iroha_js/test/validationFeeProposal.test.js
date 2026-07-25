import assert from "node:assert/strict";
import { test } from "node:test";
import { ed25519 } from "@noble/curves/ed25519";

import { AccountAddress } from "../src/address.js";
import { noritoDecodeInstruction, noritoEncodeInstruction } from "../src/norito.js";
import { buildTransactionPayload } from "../src/transaction.js";
import { computeValidationFeePolicyProposalFingerprintV1 } from "../src/validationFeeProposal.js";
import { makeNativeTest } from "./helpers/native.js";

const nativeTest = makeNativeTest(test);

function withNativeBinding(native, body) {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = native;
  try {
    return body();
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
}

test("validation-fee proposal fingerprint delegates exact native policy and lifecycle bytes", () => {
  const policy = Object.freeze({
    schema_version: 1,
    chain_id: "validation-fee-test",
  });
  const lifecycleId = "56".repeat(32);
  const fingerprint = withNativeBinding(
    {
      validationFeePolicyProposalFingerprintV1(policyJson, lifecycleBytes) {
        assert.deepEqual(JSON.parse(policyJson), policy);
        assert.deepEqual(lifecycleBytes, Buffer.from(lifecycleId, "hex"));
        return Buffer.from("12".repeat(32), "hex");
      },
    },
    () =>
      computeValidationFeePolicyProposalFingerprintV1(policy, lifecycleId),
  );
  assert.equal(fingerprint, "12".repeat(32));
});

test("validation-fee proposal fingerprint accepts no-payout and even-ending output", () => {
  const fingerprint = withNativeBinding(
    {
      validationFeePolicyProposalFingerprintV1(_policyJson, lifecycleBytes) {
        assert.equal(lifecycleBytes, null);
        return Buffer.from("34".repeat(32), "hex");
      },
    },
    () => computeValidationFeePolicyProposalFingerprintV1({ schema_version: 1 }),
  );
  assert.equal(fingerprint, "34".repeat(32));
});

test("validation-fee proposal fingerprint rejects legacy lifecycle encodings", () => {
  const unexpectedNative = {
    validationFeePolicyProposalFingerprintV1() {
      assert.fail("invalid lifecycle input must not reach native code");
    },
  };
  withNativeBinding(unexpectedNative, () => {
    assert.throws(
      () =>
        computeValidationFeePolicyProposalFingerprintV1(
          { schema_version: 1 },
          `0x${"56".repeat(32)}`,
        ),
      /64 lowercase hexadecimal/u,
    );
    assert.throws(
      () =>
        computeValidationFeePolicyProposalFingerprintV1(
          { schema_version: 1 },
          "00".repeat(32),
        ),
      /must be non-zero/u,
    );
  });
});

nativeTest("real native addon fingerprints, decodes, and rebuilds the policy instruction", () => {
  const authority = AccountAddress.fromAccount({
    publicKey: Buffer.from(ed25519.getPublicKey(Buffer.alloc(32, 0x11))),
  }).toI105();
  const policy = {
    schema_version: 1,
    chain_id: "validation-fee-js-test",
    genesis_hash: "12".repeat(32),
    policy_version: "1",
    previous_policy_hash: null,
    ds_asset_id: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
    ds_scale: 2,
    fee: "0.1",
    treasury_account_id: authority,
    charging_mode: {
      charging_mode: "PER_QUALIFYING_TRANSFER_INSTRUCTION",
      value: null,
    },
    effective_from_height: "121100",
    expires_after_height: null,
    exemption_classes: [],
    treasury_payout_binding: null,
  };
  const proposalId =
    computeValidationFeePolicyProposalFingerprintV1(policy);
  assert.match(proposalId, /^[0-9a-f]{64}$/u);
  assert.equal(
    computeValidationFeePolicyProposalFingerprintV1(policy),
    proposalId,
  );

  const instruction = {
    ProposeValidationFeePolicy: {
      policy,
      payout_lifecycle_proposal_id: null,
      referendum_window: { lower: "100", upper: "140" },
      mode: "Plain",
    },
  };
  const encoded = noritoEncodeInstruction(instruction);
  const decoded = noritoDecodeInstruction(encoded);
  assert.equal(decoded.ProposeValidationFeePolicy.mode, "Plain");
  assert.deepEqual(
    noritoEncodeInstruction(decoded),
    encoded,
    "native decode/encode must preserve exact InstructionBox bytes",
  );

  const draft = buildTransactionPayload({
    chainId: "validation-fee-js-test",
    authority,
    instructions: [decoded],
    feePayment: { payer: "authority", chargeLimits: [] },
    creationTimeMs: 1_700_000_000_000,
    ttlMs: 60_000,
    nonce: 9,
  });
  assert.ok(draft.payloadBytes.length > 0);
  assert.ok(draft.payloadHash.length > 0);
  assert.equal(draft.payload.instructions.Instructions.length, 1);
});
