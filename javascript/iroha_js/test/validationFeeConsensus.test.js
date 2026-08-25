import assert from "node:assert/strict";
import test from "node:test";
import { ed25519 } from "@noble/curves/ed25519";

import { AccountAddress } from "../src/address.js";
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
const proposalOperator = AccountAddress.fromAccount({
  publicKey: Buffer.from(ed25519.getPublicKey(Buffer.alloc(32, 0x31))),
}).toI105();

function id(byte) {
  return byte.toString(16).padStart(2, "0").repeat(32);
}

function completeParliamentProposal(kind, proposalOctet, offset) {
  const proposalId = proposalOctet.repeat(32);
  const governanceAttemptId = id(offset);
  const bodyInstanceId = id(offset + 1);
  const electionAttemptId = id(offset + 2);
  const sortitionRequestId = id(offset + 3);
  const beaconSessionId = id(offset + 4);
  const beaconPulseId = id(offset + 5);
  const root = (delta) => Array(32).fill(offset + delta);
  return {
    proposal_kind: kind,
    proposal_operator: proposalOperator,
    proposal_id: proposalId,
    payload_hash: proposalId,
    governance_certificate_id: id(offset + 6),
    governance_certificate: {
      proposal_content_id: proposalId,
      governance_attempt_id: governanceAttemptId,
      governance_attempt_sequence: 0,
      risk_tier: { tier: "Standard" },
      body_bindings: [{
        body_instance_id: bodyInstanceId,
        election_attempt_id: electionAttemptId,
        election_attempt_sequence: 0,
        sortition_request_id: sortitionRequestId,
        sortition_request: {
          id: sortitionRequestId,
          governance_attempt_id: governanceAttemptId,
          body_election_attempt_id: electionAttemptId,
          body: "policy-jury",
          candidate_root: root(7),
          candidate_count: 3,
          target_seats: 3,
          request_height: 1000,
          pulse_height: 1001,
          beacon_session_id: beaconSessionId,
        },
        body: "policy-jury",
        original_seats: 3,
        beacon_session_id: beaconSessionId,
        beacon_pulse_id: beaconPulseId,
        roster_root: root(8),
        assignment_root: root(9),
        result_root: root(10),
        result_height: 4599,
        public_finding: null,
        ballot: {
          ballot_attempt_id: id(offset + 11),
          ballot_attempt_sequence: 0,
          tle_session_id: id(offset + 12),
          tle_key_session_id: id(offset + 13),
          registration_root: root(14),
          dropout_root: root(15),
          survivor_root: root(16),
          corpus_root: root(17),
          no_recovery_root: root(18),
          timed_commitment_root: root(19),
          release_beacon_session_id: id(offset + 20),
          registered_at_height: 1100,
          registration_close_height: 1200,
          survivor_freeze_height: 1300,
          commitment_close_height: 1400,
          registration_closed_at_height: 1200,
          survivors_frozen_at_height: 1300,
          commitment_closed_at_height: 1400,
          max_ballot_retries: 3,
          max_corpus_entries: 1000,
          release_height: 1500,
          opening_deadline_height: 4599,
          release_pulse_id: id(offset + 21),
          opening_height: 1500,
          opening_root: root(22),
          tally: {
            original_seats: 3,
            accepted_ballots: 3,
            aye: 2,
            nay: 1,
            abstain: 0,
          },
          outcome: { outcome: "Approved" },
        },
      }],
      policy_version: 1,
      effect_preimage_hash: root(23),
      expected_head: { state: "Absent", head: { subject_id: root(24) } },
      certified_at_height: 4599,
      enact_at_height: 4600,
    },
    certified_at_height: "4599",
    enacted_at_height: "4600",
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
        0x20,
      ),
      payoutLifecycle: completeParliamentProposal(
        "ValidationFeePayoutLifecycleV1",
        "08",
        0x50,
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
      .governance_certificate.proposal_content_id,
    "02".repeat(32),
  );
  assert.equal(
    verified.current_policy.parliament.validationFeePolicy.proposal_operator,
    proposalOperator,
  );
  assert.equal(
    verified.current_policy.parliament.payoutLifecycle
      .governance_certificate.body_bindings[0].body,
    "policy-jury",
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
        .governance_certificate.body_bindings[0].ballot.tally,
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
      label: "missing proposal operator",
      mutate(projection) {
        delete projection.current_policy.parliament.validationFeePolicy
          .proposal_operator;
      },
      error: /validationFeePolicy must contain exactly/u,
    },
    {
      label: "empty proposal operator",
      mutate(projection) {
        projection.current_policy.parliament.validationFeePolicy.proposal_operator =
          "";
      },
      error: /proposal_operator must be a non-empty string/u,
    },
    {
      label: "retired PLAIN electorate projection",
      mutate(projection) {
        projection.current_policy.parliament.validationFeePolicy
          .plainElectorateRules = {};
      },
      error: /validationFeePolicy must contain exactly/u,
    },
    {
      label: "extra certificate field",
      mutate(projection) {
        projection.current_policy.parliament.payoutLifecycle
          .governance_certificate.legacy = null;
      },
      error: /certificate contains unknown, aliased, or missing fields/u,
    },
    {
      label: "missing sortition binding",
      mutate(projection) {
        delete projection.current_policy.parliament.validationFeePolicy
          .governance_certificate.body_bindings[0].sortition_request.candidate_root;
      },
      error: /sortition_request contains unknown, aliased, or missing fields/u,
    },
    {
      label: "extra ballot field",
      mutate(projection) {
        projection.current_policy.parliament.validationFeePolicy
          .governance_certificate.body_bindings[0].ballot.raw = {};
      },
      error: /ballot contains unknown, aliased, or missing fields/u,
    },
    {
      label: "legacy flattened ballot outcome",
      mutate(projection) {
        projection.current_policy.parliament.validationFeePolicy
          .governance_certificate.body_bindings[0].ballot.outcome = "Approved";
      },
      error: /ballot\.outcome must be a plain object/u,
    },
    {
      label: "ballot retry ceiling exceeds the Rust contract",
      mutate(projection) {
        projection.current_policy.parliament.validationFeePolicy
          .governance_certificate.body_bindings[0].ballot.max_ballot_retries = 17;
      },
      error: /max_ballot_retries must be an integer from 0 through 16/u,
    },
    {
      label: "sortition target exceeds the Rust contract",
      mutate(projection) {
        projection.current_policy.parliament.validationFeePolicy
          .governance_certificate.body_bindings[0].sortition_request.target_seats =
          1001;
      },
      error: /target_seats must be an integer from 1 through 1000/u,
    },
    {
      label: "release pulse reuses a sortition pulse",
      mutate(projection) {
        const body = projection.current_policy.parliament.validationFeePolicy
          .governance_certificate.body_bindings[0];
        body.ballot.release_pulse_id = body.beacon_pulse_id;
      },
      error: /sortition and release pulse identifiers disjoint/u,
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
      label: "certificate targets another proposal",
      mutate(projection) {
        projection.current_policy.parliament.validationFeePolicy
          .governance_certificate.proposal_content_id = "04".repeat(32);
      },
      error: /differs from its retained governance certificate/u,
    },
    {
      label: "outer certification height differs",
      mutate(projection) {
        projection.current_policy.parliament.validationFeePolicy
          .certified_at_height = "4598";
      },
      error: /differs from its retained governance certificate/u,
    },
    {
      label: "outer certification height is not a decimal string",
      mutate(projection) {
        projection.current_policy.parliament.validationFeePolicy
          .certified_at_height = 4599;
      },
      error: /certified_at_height must be a canonical unsigned decimal string/u,
    },
    {
      label: "non-approving certificate ballot",
      mutate(projection) {
        projection.current_policy.parliament.validationFeePolicy
          .governance_certificate.body_bindings[0].ballot.outcome = {
            outcome: "Rejected",
          };
      },
      error: /approving aggregate outcome/u,
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
        /ABI 23/u,
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
