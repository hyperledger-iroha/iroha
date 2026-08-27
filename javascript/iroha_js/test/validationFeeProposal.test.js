import assert from "node:assert/strict";
import { test } from "node:test";
import { ed25519 } from "@noble/curves/ed25519";

import { AccountAddress } from "../src/address.js";
import { NetworkId } from "../src/networkId.js";
import { noritoDecodeInstruction, noritoEncodeInstruction } from "../src/norito.js";
import { buildTransactionPayload } from "../src/transaction.js";
import {
  computeValidationFeePayoutLifecycleProposalFingerprintV1,
  computeValidationFeePolicyProposalFingerprintV1,
} from "../src/validationFeeProposal.js";
import { makeNativeTest } from "./helpers/native.js";

const nativeTest = makeNativeTest(test);
const proposalOperator = AccountAddress.fromAccount({
  publicKey: Buffer.from(ed25519.getPublicKey(Buffer.alloc(32, 0x23))),
}).toI105();
const validationFeeNetwork = NetworkId.fromBytes(
  Buffer.from("13".repeat(32), "hex"),
);
const validationFeeNetworkId = validationFeeNetwork.toString();

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
    network_id: validationFeeNetworkId,
  });
  const lifecycleId = "56".repeat(32);
  const fingerprint = withNativeBinding(
    {
      validationFeePolicyProposalFingerprintV1(
        nativeProposalOperator,
        policyJson,
        lifecycleBytes,
      ) {
        assert.equal(nativeProposalOperator, proposalOperator);
        assert.deepEqual(JSON.parse(policyJson), policy);
        assert.deepEqual(lifecycleBytes, Buffer.from(lifecycleId, "hex"));
        return Buffer.from("12".repeat(32), "hex");
      },
    },
    () =>
      computeValidationFeePolicyProposalFingerprintV1(
        proposalOperator,
        policy,
        lifecycleId,
      ),
  );
  assert.equal(fingerprint, "12".repeat(32));
});

test("validation-fee proposal fingerprint accepts no-payout and even-ending output", () => {
  const fingerprint = withNativeBinding(
    {
      validationFeePolicyProposalFingerprintV1(
        nativeProposalOperator,
        _policyJson,
        lifecycleBytes,
      ) {
        assert.equal(nativeProposalOperator, proposalOperator);
        assert.equal(lifecycleBytes, null);
        return Buffer.from("34".repeat(32), "hex");
      },
    },
    () =>
      computeValidationFeePolicyProposalFingerprintV1(
        proposalOperator,
        { schema_version: 1 },
        null,
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
        nativeProposalOperator,
        payoutBindingJson,
      ) {
        assert.equal(nativeProposalOperator, proposalOperator);
        assert.deepEqual(JSON.parse(payoutBindingJson), payoutBinding);
        return Buffer.from("56".repeat(32), "hex");
      },
    },
    () =>
      computeValidationFeePayoutLifecycleProposalFingerprintV1(
        proposalOperator,
        payoutBinding,
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
          proposalOperator,
          { schema_version: 1 },
          `0x${"56".repeat(32)}`,
        ),
      /64 lowercase hexadecimal/u,
    );
    assert.throws(
      () =>
        computeValidationFeePolicyProposalFingerprintV1(
          proposalOperator,
          { schema_version: 1 },
          "00".repeat(32),
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
          proposalOperator,
          [],
        ),
      /policy must be an exact native object/u,
    );
    assert.throws(
      () =>
        computeValidationFeePayoutLifecycleProposalFingerprintV1(
          proposalOperator,
          [],
        ),
      /payoutBinding must be an exact native object/u,
    );
  });
});

test("validation-fee proposal fingerprints reject the retired electorate argument", () => {
  const unexpectedNative = {
    validationFeePolicyProposalFingerprintV1() {
      assert.fail("retired policy arguments must not reach native code");
    },
    validationFeePayoutLifecycleProposalFingerprintV1() {
      assert.fail("retired payout arguments must not reach native code");
    },
  };
  withNativeBinding(unexpectedNative, () => {
    assert.throws(
      () => computeValidationFeePolicyProposalFingerprintV1(
        proposalOperator,
        { schema_version: 1 },
        null,
        {},
      ),
      /exactly two or three arguments/u,
    );
    assert.throws(
      () => computeValidationFeePayoutLifecycleProposalFingerprintV1(
        proposalOperator,
        { entrypoint: "autonomous_validation_fee_tick" },
        {},
      ),
      /exactly two arguments/u,
    );
  });
});

test("validation-fee proposal fingerprints require an explicit canonical operator", () => {
  const unexpectedNative = {
    validationFeePolicyProposalFingerprintV1() {
      assert.fail("invalid proposal operator must not reach native code");
    },
  };
  withNativeBinding(unexpectedNative, () => {
    for (const invalid of [null, "", ` ${proposalOperator}`]) {
      assert.throws(
        () =>
          computeValidationFeePolicyProposalFingerprintV1(
            invalid,
            { schema_version: 1 },
            null,
          ),
        /proposalOperator must be one canonical domainless AccountId/u,
      );
    }
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
            proposalOperator,
            { schema_version: 1 },
            null,
          ),
        /policy proposal fingerprint must contain exactly 32 bytes/u,
      );
      assert.throws(
        () =>
          computeValidationFeePayoutLifecycleProposalFingerprintV1(
            proposalOperator,
            { entrypoint: "autonomous_validation_fee_tick" },
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
    network_id: validationFeeNetworkId,
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
      authority,
      policy,
      null,
    );
  assert.match(proposalId, /^[0-9a-f]{64}$/u);
  assert.equal(
    computeValidationFeePolicyProposalFingerprintV1(
      authority,
      policy,
      null,
    ),
    proposalId,
  );

  const instruction = {
    ProposeValidationFeePolicy: {
      policy,
      payout_lifecycle_proposal_id: null,
    },
  };
  const encoded = noritoEncodeInstruction(instruction);
  const decoded = noritoDecodeInstruction(encoded);
  assert.deepEqual(Object.keys(decoded.ProposeValidationFeePolicy).sort(), [
    "payout_lifecycle_proposal_id",
    "policy",
  ]);
  assert.deepEqual(
    noritoEncodeInstruction(decoded),
    encoded,
    "native decode/encode must preserve exact InstructionBox bytes",
  );

  const draft = buildTransactionPayload({
    networkId: validationFeeNetwork,
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
