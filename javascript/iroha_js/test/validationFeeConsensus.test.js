import assert from "node:assert/strict";
import test from "node:test";

import {
  encodeValidationFeeCurrentPolicyProofRequestV1,
  normalizeValidationFeeLedgerBindingV1,
  verifyValidationFeeCurrentPolicyProofV1,
} from "../src/validationFeeConsensus.js";
import { ToriiClient } from "../src/toriiClient.js";

const binding = Object.freeze({
  schema: "cbsi.mobile-validation-fee-ledger-binding.v1",
  chainId: "iroha3-nexus",
  genesisHash: "11".repeat(32),
  policyChainGenesisHash: "33".repeat(32),
  checkpoint: Object.freeze({
    height: 100,
    contextId: "55".repeat(32),
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

test("immutable ledger binding rejects aliases and unknown trust inputs", () => {
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
        genesisHash: "AA".repeat(32),
      }),
    /lowercase hexadecimal/u,
  );
});

test("request encoder delegates only after strict checkpoint validation", () => {
  const evenEndingCheckpoint = Object.freeze({
    height: binding.checkpoint.height,
    contextId: "02".repeat(32),
  });
  withNativeBinding(
    {
      connectNoritoBridgeAbiVersion() {
        return 21;
      },
      validationFeeCurrentPolicyProofRequestV1(height, context) {
        assert.equal(height, 100n);
        assert.deepEqual(context, Buffer.from("02".repeat(32), "hex"));
        return Buffer.from([1, 2, 3]);
      },
      validationFeeVerifyCurrentPolicyProofV1() {},
    },
    () => {
      assert.deepEqual(
        encodeValidationFeeCurrentPolicyProofRequestV1(evenEndingCheckpoint),
        Buffer.from([1, 2, 3]),
      );
    },
  );
});

test("native verified projection remains bound to the release checkpoint", () => {
  const projection = {
    schema: "iroha.validation_fee.verified_policy_projection.v1",
    version: 1,
    chain_id: binding.chainId,
    genesis_hash: binding.genesisHash,
    policy_chain_genesis_hash: binding.policyChainGenesisHash,
    registry_hash: "77".repeat(32),
    head_policy_version: 2,
    head_policy_hash: "99".repeat(32),
    current_policy: null,
    trusted_checkpoint_height: 100,
    trusted_checkpoint_context_id: binding.checkpoint.contextId,
    evaluated_block_height: 127,
    evaluated_context_id: "bc".repeat(32),
    evaluated_block_hash: "de".repeat(32),
    observed_ledger_tip_height: 190,
    more_available: true,
  };
  withNativeBinding(
    {
      connectNoritoBridgeAbiVersion() {
        return 21;
      },
      validationFeeCurrentPolicyProofRequestV1() {},
      validationFeeVerifyCurrentPolicyProofV1(
        proof,
        chainId,
        genesis,
        policyGenesis,
        height,
        context,
      ) {
        assert.deepEqual(proof, Buffer.from([9]));
        assert.equal(chainId, binding.chainId);
        assert.deepEqual(genesis, Buffer.from(binding.genesisHash, "hex"));
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
    },
  );
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
        /ABI 21/u,
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
    assert.equal(normalizedBinding.chainId, binding.chainId);
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

  const result = await client.catchUpValidationFeeCurrentPolicyProof(binding);
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
    client.catchUpValidationFeeCurrentPolicyProof(binding),
    /did not advance/u,
  );
});
