import assert from "node:assert/strict";
import test from "node:test";

import {
  encodeValidationFeeCurrentPolicyProofRequestV1,
  normalizeValidationFeeLedgerBindingV1,
  verifyValidationFeeCurrentPolicyProofV1,
} from "../src/validationFeeConsensus.js";
import { NetworkId } from "../src/networkId.js";
import { ToriiClient } from "../src/toriiClient.js";

const binding = Object.freeze({
  schema: "cbsi.mobile-validation-fee-ledger-binding.v1",
  networkId: NetworkId.fromBytes(Buffer.from("13".repeat(32), "hex")),
  policyChainGenesisHash: "35".repeat(32),
  checkpoint: Object.freeze({
    height: 100,
    contextId: "57".repeat(32),
  }),
});

function completePlainElectorateRules() {
  return {
    voting_asset_id: "xor#sora",
    bond_escrow_account: "bond-escrow-account",
    slash_receiver_account: "slash-receiver-account",
    ballot_amount: "150",
    ballot_duration_blocks: "3600",
    citizenship_amount: "10000",
    max_members: "256",
    conviction_step_blocks: "100",
    max_conviction: "6",
    min_turnout: "1",
    approval_threshold_numerator: "1",
    approval_threshold_denominator: "2",
    eligibility_rule: {
      rule: "proposal_operator_at_or_before_gate_others_after_gate",
      value: null,
    },
  };
}

function completeParliamentProposal(kind, proposalOctet, rosterOctet, snapshotOctet) {
  const proposalId = proposalOctet.repeat(32);
  return {
    proposal_kind: kind,
    proposal_id: proposalId,
    payload_hash: proposalId,
    parliament_roster_root: rosterOctet.repeat(32),
    plainElectorateRules: completePlainElectorateRules(),
    plainElectorateSnapshot: {
      rosterRoot: snapshotOctet.repeat(32),
      memberCount: "2",
      capturedAtHeight: "1000",
      approvalGateHeight: "999",
    },
    enactment_window: {
      opens_at_height: "1000",
      closes_at_height: "4599",
      enacted_at_height: "4600",
    },
    finalization: {
      proposal_id: proposalId,
      referendum_id: proposalId,
      finalized_at_height: "4599",
      mode: "PLAIN",
      approve: "2",
      reject: "0",
      abstain: "0",
      min_turnout: "1",
      approval_threshold_numerator: "1",
      approval_threshold_denominator: "2",
      approved: true,
    },
  };
}

function completeCurrentPolicy() {
  return {
    activePolicyVersion: "1",
    activePolicyHash: "ab".repeat(32),
    feeAssetDefinitionId: "ds#sora",
    feeScale: 2,
    feeMinorUnits: "10",
    chargingMode: "PER_QUALIFYING_TRANSFER_INSTRUCTION",
    effectiveFromHeight: "125560",
    expiresAfterHeight: null,
    parliament: {
      validationFeePolicy: completeParliamentProposal(
        "ValidationFeePolicyV1",
        "02",
        "04",
        "06",
      ),
      payoutLifecycle: completeParliamentProposal(
        "ValidationFeePayoutLifecycleV1",
        "08",
        "0a",
        "0c",
      ),
      payoutLifecycleSealHash: "cd".repeat(32),
    },
    payout: {
      contractAddress: "validation-fee-contract",
      codeHash: "aa".repeat(32),
      entrypoint: "autonomous_validation_fee_tick",
      dsAssetDefinitionId: "ds#sora",
      xorAssetDefinitionId: "xor#sora",
      treasuryAccountId: "treasury-account",
      vaultAccountId: "vault-account",
      batchDsMinorUnits: "1000",
      dsScale: 2,
      xorOutputMin: "4",
      xorOutputMax: "100",
      recipients: [0, 1, 2, 3].map((index) => ({
        account_id: `validator-${index}`,
        share_basis_points: 2500,
      })),
    },
  };
}

function completeVerifiedProjection() {
  return {
    schema: "iroha.validation_fee.verified_policy_projection.v1",
    version: 1,
    network_id: binding.networkId.toString(),
    policy_chain_genesis_hash: binding.policyChainGenesisHash,
    registry_hash: "79".repeat(32),
    head_policy_version: 1,
    head_policy_hash: "ab".repeat(32),
    current_policy: completeCurrentPolicy(),
    trusted_checkpoint_height: 100,
    trusted_checkpoint_context_id: binding.checkpoint.contextId,
    evaluated_block_height: 127,
    evaluated_context_id: "bd".repeat(32),
    evaluated_block_hash: "df".repeat(32),
    observed_ledger_tip_height: 190,
    more_available: true,
  };
}

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

function verifyProjectionFixture(projection) {
  return withNativeBinding(
    {
      connectNoritoBridgeAbiVersion() {
        return 22;
      },
      validationFeeCurrentPolicyProofRequestV1() {},
      validationFeeVerifyCurrentPolicyProofV1() {
        return JSON.stringify(projection);
      },
    },
    () =>
      verifyValidationFeeCurrentPolicyProofV1(
        Buffer.from([9]),
        binding,
        binding.checkpoint,
      ),
  );
}

test("immutable ledger binding requires marked Iroha hashes and rejects aliases", () => {
  const normalized = normalizeValidationFeeLedgerBindingV1(binding);
  assert.equal(normalized.networkId, binding.networkId);
  assert.equal(normalized.policyChainGenesisHash, "35".repeat(32));
  assert.equal(normalized.checkpoint.contextId, "57".repeat(32));
  assert.throws(
    () =>
      normalizeValidationFeeLedgerBindingV1({
        ...binding,
        signedPolicy: {},
      }),
    /must contain exactly/u,
  );
  assert.throws(
    () =>
      normalizeValidationFeeLedgerBindingV1({
        ...binding,
        chainId: "legacy-label",
      }),
    /must contain exactly/u,
  );
  assert.throws(
    () => NetworkId.fromBytes(Buffer.from("12".repeat(32), "hex")),
    /canonical Iroha hash marker/u,
  );
});

test("request encoder delegates only after strict checkpoint validation", () => {
  const checkpoint = Object.freeze({
    height: binding.checkpoint.height,
    contextId: "03".repeat(32),
  });
  withNativeBinding(
    {
      connectNoritoBridgeAbiVersion() {
        return 22;
      },
      validationFeeCurrentPolicyProofRequestV1(height, context) {
        assert.equal(height, 100n);
        assert.deepEqual(context, Buffer.from("03".repeat(32), "hex"));
        return Buffer.from([1, 2, 3]);
      },
      validationFeeVerifyCurrentPolicyProofV1() {},
    },
    () => {
      assert.deepEqual(
        encodeValidationFeeCurrentPolicyProofRequestV1(checkpoint),
        Buffer.from([1, 2, 3]),
      );
      assert.throws(
        () =>
          encodeValidationFeeCurrentPolicyProofRequestV1({
            ...checkpoint,
            contextId: "00".repeat(32),
          }),
        /must be non-zero/u,
      );
      assert.throws(
        () =>
          encodeValidationFeeCurrentPolicyProofRequestV1({
            ...checkpoint,
            contextId: "02".repeat(32),
          }),
        /canonical Iroha hash marker/u,
      );
    },
  );
});

test("native verified projection remains bound to the release checkpoint", () => {
  const projection = {
    schema: "iroha.validation_fee.verified_policy_projection.v1",
    version: 1,
    network_id: binding.networkId.toString(),
    policy_chain_genesis_hash: binding.policyChainGenesisHash,
    registry_hash: "79".repeat(32),
    head_policy_version: 2,
    head_policy_hash: "9b".repeat(32),
    current_policy: null,
    trusted_checkpoint_height: 100,
    trusted_checkpoint_context_id: binding.checkpoint.contextId,
    evaluated_block_height: 127,
    evaluated_context_id: "bd".repeat(32),
    evaluated_block_hash: "df".repeat(32),
    observed_ledger_tip_height: 190,
    more_available: true,
  };
  withNativeBinding(
    {
      connectNoritoBridgeAbiVersion() {
        return 22;
      },
      validationFeeCurrentPolicyProofRequestV1() {},
      validationFeeVerifyCurrentPolicyProofV1(
        proof,
        networkId,
        policyGenesis,
        height,
        context,
      ) {
        assert.deepEqual(proof, Buffer.from([9]));
        assert.deepEqual(networkId, Buffer.from(binding.networkId.toBytes()));
        assert.deepEqual(
          policyGenesis,
          Buffer.from(binding.policyChainGenesisHash, "hex"),
        );
        assert.equal(height, 100n);
        assert.deepEqual(context, Buffer.from(binding.checkpoint.contextId, "hex"));
        return JSON.stringify(projection);
      },
    },
    () => {
      const verified = verifyValidationFeeCurrentPolicyProofV1(
        Buffer.from([9]),
        binding,
        binding.checkpoint,
      );
      assert.equal(verified.head_policy_version, 2n);
      assert.equal(verified.evaluated_block_height, 127n);
      assert.equal(verified.more_available, true);
      assert.equal(Object.isFrozen(verified), true);
      projection.evaluated_block_hash = "de".repeat(32);
      assert.throws(
        () =>
          verifyValidationFeeCurrentPolicyProofV1(
            Buffer.from([9]),
            binding,
            binding.checkpoint,
          ),
        /canonical Iroha hash marker/u,
      );
      projection.evaluated_block_hash = "df".repeat(32);
    },
  );
});

test("verified current policy enforces and freezes the complete nested projection", () => {
  const verified = verifyProjectionFixture(completeVerifiedProjection());
  assert.equal(verified.current_policy.activePolicyVersion, "1");
  assert.equal(
    verified.current_policy.parliament.validationFeePolicy
      .plainElectorateRules.bond_escrow_account,
    "bond-escrow-account",
  );
  assert.equal(
    verified.current_policy.parliament.payoutLifecycle
      .plainElectorateSnapshot.rosterRoot,
    "0c".repeat(32),
  );
  assert.equal(
    verified.current_policy.payout.recipients[3].share_basis_points,
    2500,
  );
  assert.equal(
    verified.current_policy.payout.dsAssetDefinitionId,
    "ds#sora",
  );
  assert.equal(verified.current_policy.payout.batchDsMinorUnits, "1000");
  assert.equal(verified.current_policy.payout.dsScale, 2);
  assert.equal(Object.isFrozen(verified.current_policy), true);
  assert.equal(Object.isFrozen(verified.current_policy.parliament), true);
  assert.equal(
    Object.isFrozen(
      verified.current_policy.parliament.validationFeePolicy
        .plainElectorateRules.eligibility_rule,
    ),
    true,
  );
  assert.equal(Object.isFrozen(verified.current_policy.payout.recipients), true);
  assert.equal(
    Object.isFrozen(verified.current_policy.payout.recipients[0]),
    true,
  );
});

test("verified current policy rejects missing, extra, and mistyped nested fields", () => {
  const malformedFixtures = [
    {
      label: "missing proposal-bound escrow",
      mutate(projection) {
        delete projection.current_policy.parliament.validationFeePolicy
          .plainElectorateRules.bond_escrow_account;
      },
      error: /plainElectorateRules must contain exactly/u,
    },
    {
      label: "extra frozen-snapshot field",
      mutate(projection) {
        projection.current_policy.parliament.payoutLifecycle
          .plainElectorateSnapshot.members = [];
      },
      error: /plainElectorateSnapshot must contain exactly/u,
    },
    {
      label: "retired SBD payout field",
      mutate(projection) {
        projection.current_policy.payout.sbdAssetDefinitionId =
          projection.current_policy.payout.dsAssetDefinitionId;
      },
      error: /payout must contain exactly/u,
    },
    {
      label: "mistyped payout share",
      mutate(projection) {
        projection.current_policy.payout.recipients[0].share_basis_points =
          "2500";
      },
      error: /share_basis_points must be an unsigned integer/u,
    },
    {
      label: "wrong finalization anchor",
      mutate(projection) {
        projection.current_policy.parliament.validationFeePolicy
          .finalization.finalized_at_height = "4598";
      },
      error: /violates its frozen electorate or enactment anchors/u,
    },
    {
      label: "wrong CBSI ballot amount",
      mutate(projection) {
        projection.current_policy.parliament.validationFeePolicy
          .plainElectorateRules.ballot_amount = "151";
      },
      error: /violates the bounded PLAIN electorate invariants/u,
    },
  ];
  for (const fixture of malformedFixtures) {
    const projection = completeVerifiedProjection();
    fixture.mutate(projection);
    assert.throws(
      () => verifyProjectionFixture(projection),
      fixture.error,
      fixture.label,
    );
  }
});

test("validation-fee proof path rejects a stale native bridge ABI", () => {
  withNativeBinding(
    {
      connectNoritoBridgeAbiVersion() {
        return 20;
      },
      validationFeeCurrentPolicyProofRequestV1() {
        return Buffer.from([1]);
      },
      validationFeeVerifyCurrentPolicyProofV1() {},
    },
    () => {
      assert.throws(
        () => encodeValidationFeeCurrentPolicyProofRequestV1(binding.checkpoint),
        /ABI 22/u,
      );
    },
  );
});

test("proof catch-up promotes only consecutive locally verified pages", async () => {
  const client = new ToriiClient("https://torii.invalid", {
    fetchImpl: async () => {
      throw new Error("network must not be used by this fixture");
    },
  });
  const visited = [];
  client.getValidationFeeCurrentPolicyProofPage = async (
    normalizedBinding,
    checkpoint,
  ) => {
    visited.push(checkpoint.height);
    assert.equal(normalizedBinding.networkId, binding.networkId);
    const nextHeight = checkpoint.height === 100n ? 127n : 190n;
    return Object.freeze({
      proofNorito: Buffer.from([Number(nextHeight % 256n)]),
      projection: Object.freeze({
        evaluated_block_height: nextHeight,
        more_available: nextHeight !== 190n,
      }),
      promotedCheckpoint: Object.freeze({
        height: nextHeight,
        contextId: nextHeight === 127n ? "77".repeat(32) : "99".repeat(32),
      }),
    });
  };

  const result = await client.catchUpValidationFeeCurrentPolicyProof(binding, {});
  assert.deepEqual(visited, [100n, 127n]);
  assert.equal(result.pagesVerified, 2);
  assert.equal(result.promotedCheckpoint.height, 190n);
  assert.equal(Object.isFrozen(result), true);
});

test("proof catch-up fails closed when a non-final page does not advance", async () => {
  const client = new ToriiClient("https://torii.invalid", {
    fetchImpl: async () => {
      throw new Error("network must not be used by this fixture");
    },
  });
  client.getValidationFeeCurrentPolicyProofPage = async (_binding, checkpoint) =>
    Object.freeze({
      proofNorito: Buffer.from([1]),
      projection: Object.freeze({
        evaluated_block_height: checkpoint.height,
        more_available: true,
      }),
      promotedCheckpoint: checkpoint,
    });

  await assert.rejects(
    client.catchUpValidationFeeCurrentPolicyProof(binding, {}),
    /did not advance/u,
  );
});
