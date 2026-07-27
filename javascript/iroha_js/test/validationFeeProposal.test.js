import assert from "node:assert/strict";
import { test } from "node:test";
import { ed25519 } from "@noble/curves/ed25519";

import { AccountAddress } from "../src/address.js";
import { noritoDecodeInstruction, noritoEncodeInstruction } from "../src/norito.js";
import { buildTransactionPayload } from "../src/transaction.js";
import {
  computeValidationFeePayoutLifecycleProposalFingerprintV1,
  computeValidationFeePolicyProposalFingerprintV1,
} from "../src/validationFeeProposal.js";
import { makeNativeTest } from "./helpers/native.js";

const nativeTest = makeNativeTest(test);
const bondEscrowAccount = AccountAddress.fromAccount({
  publicKey: Buffer.from(ed25519.getPublicKey(Buffer.alloc(32, 0x21))),
}).toI105();
const slashReceiverAccount = AccountAddress.fromAccount({
  publicKey: Buffer.from(ed25519.getPublicKey(Buffer.alloc(32, 0x22))),
}).toI105();
const plainElectorateRules = Object.freeze({
  voting_asset_id: "5dHF5UNffENuEg9mhjYwY1jcZ1K5",
  bond_escrow_account: bondEscrowAccount,
  slash_receiver_account: slashReceiverAccount,
  ballot_amount: "150",
  ballot_duration_blocks: "3600",
  citizenship_amount: "10000",
  max_members: "256",
  conviction_step_blocks: "100",
  max_conviction: "6",
  min_turnout: "1",
  approval_threshold_numerator: "1",
  approval_threshold_denominator: "2",
  eligibility_rule: Object.freeze({
    rule: "proposal_operator_at_or_before_gate_others_after_gate",
    value: null,
  }),
});

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
      validationFeePolicyProposalFingerprintV1(
        policyJson,
        lifecycleBytes,
        plainElectorateRulesJson,
      ) {
        assert.deepEqual(JSON.parse(policyJson), policy);
        assert.deepEqual(lifecycleBytes, Buffer.from(lifecycleId, "hex"));
        assert.deepEqual(
          JSON.parse(plainElectorateRulesJson),
          plainElectorateRules,
        );
        return Buffer.from("12".repeat(32), "hex");
      },
    },
    () =>
      computeValidationFeePolicyProposalFingerprintV1(
        policy,
        lifecycleId,
        plainElectorateRules,
      ),
  );
  assert.equal(fingerprint, "12".repeat(32));
});

test("validation-fee proposal fingerprint accepts no-payout and even-ending output", () => {
  const fingerprint = withNativeBinding(
    {
      validationFeePolicyProposalFingerprintV1(
        _policyJson,
        lifecycleBytes,
        plainElectorateRulesJson,
      ) {
        assert.equal(lifecycleBytes, null);
        assert.deepEqual(
          JSON.parse(plainElectorateRulesJson),
          plainElectorateRules,
        );
        return Buffer.from("34".repeat(32), "hex");
      },
    },
    () =>
      computeValidationFeePolicyProposalFingerprintV1(
        { schema_version: 1 },
        null,
        plainElectorateRules,
      ),
  );
  assert.equal(fingerprint, "34".repeat(32));
});

test("validation-fee payout lifecycle fingerprint delegates exact native objects", () => {
  const payoutBinding = Object.freeze({
    entrypoint: "autonomous_validation_fee_tick",
  });
  const fingerprint = withNativeBinding(
    {
      validationFeePayoutLifecycleProposalFingerprintV1(
        payoutBindingJson,
        plainElectorateRulesJson,
      ) {
        assert.deepEqual(JSON.parse(payoutBindingJson), payoutBinding);
        assert.deepEqual(
          JSON.parse(plainElectorateRulesJson),
          plainElectorateRules,
        );
        return Buffer.from("56".repeat(32), "hex");
      },
    },
    () =>
      computeValidationFeePayoutLifecycleProposalFingerprintV1(
        payoutBinding,
        plainElectorateRules,
      ),
  );
  assert.equal(fingerprint, "56".repeat(32));
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
          plainElectorateRules,
        ),
      /64 lowercase hexadecimal/u,
    );
    assert.throws(
      () =>
        computeValidationFeePolicyProposalFingerprintV1(
          { schema_version: 1 },
          "00".repeat(32),
          plainElectorateRules,
        ),
      /must be non-zero/u,
    );
  });
});

test("validation-fee proposal fingerprints require exact object inputs", () => {
  const unexpectedNative = {
    validationFeePolicyProposalFingerprintV1() {
      assert.fail("invalid policy input must not reach native code");
    },
    validationFeePayoutLifecycleProposalFingerprintV1() {
      assert.fail("invalid payout input must not reach native code");
    },
  };
  withNativeBinding(unexpectedNative, () => {
    assert.throws(
      () =>
        computeValidationFeePolicyProposalFingerprintV1(
          { schema_version: 1 },
          null,
          undefined,
        ),
      /plainElectorateRules must be an exact native object/u,
    );
    assert.throws(
      () =>
        computeValidationFeePayoutLifecycleProposalFingerprintV1(
          [],
          plainElectorateRules,
        ),
      /payoutBinding must be an exact native object/u,
    );
  });
});

test("validation-fee proposal fingerprints require exact native digest lengths", () => {
  withNativeBinding(
    {
      validationFeePolicyProposalFingerprintV1() {
        return Buffer.alloc(31);
      },
      validationFeePayoutLifecycleProposalFingerprintV1() {
        return Buffer.alloc(33);
      },
    },
    () => {
      assert.throws(
        () =>
          computeValidationFeePolicyProposalFingerprintV1(
            { schema_version: 1 },
            null,
            plainElectorateRules,
          ),
        /policy proposal fingerprint must contain exactly 32 bytes/u,
      );
      assert.throws(
        () =>
          computeValidationFeePayoutLifecycleProposalFingerprintV1(
            { entrypoint: "autonomous_validation_fee_tick" },
            plainElectorateRules,
          ),
        /payout lifecycle proposal fingerprint must contain exactly 32 bytes/u,
      );
    },
  );
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
    computeValidationFeePolicyProposalFingerprintV1(
      policy,
      null,
      plainElectorateRules,
    );
  assert.match(proposalId, /^[0-9a-f]{64}$/u);
  assert.equal(
    computeValidationFeePolicyProposalFingerprintV1(
      policy,
      null,
      plainElectorateRules,
    ),
    proposalId,
  );

  const instruction = {
    ProposeValidationFeePolicy: {
      policy,
      payout_lifecycle_proposal_id: null,
      plain_electorate_rules: plainElectorateRules,
      referendum_window: { lower: "100", upper: "140" },
      mode: "Plain",
    },
  };
  const encoded = noritoEncodeInstruction(instruction);
  const decoded = noritoDecodeInstruction(encoded);
  assert.equal(decoded.ProposeValidationFeePolicy.mode, "Plain");
  assert.deepEqual(
    decoded.ProposeValidationFeePolicy.plain_electorate_rules,
    plainElectorateRules,
  );
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
