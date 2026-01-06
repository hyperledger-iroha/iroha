"use strict";

import test from "node:test";
import assert from "node:assert/strict";

import { MultisigSpecBuilder } from "../src/multisig.js";

test("builder produces deterministic payload and json", () => {
  const builder = new MultisigSpecBuilder()
    .setQuorum(3)
    .setTransactionTtlMs(60_000)
    .addSignatory("alice@wonderland", 2)
    .addSignatory("bob@wonderland", 1);

  const spec = builder.build();
  assert.equal(spec.quorum, 3);
  assert.equal(spec.transactionTtlMs, 60_000);
  assert.deepEqual(spec.toPayload(), {
    signatories: {
      "alice@wonderland": 2,
      "bob@wonderland": 1,
    },
    quorum: 3,
    transaction_ttl_ms: 60_000,
  });

  const json = builder.toJSON(true);
  assert.match(json, /\"transaction_ttl_ms\": 60000/);
  assert.ok(json.indexOf("alice@wonderland") < json.indexOf("bob@wonderland"));
});

test("proposal ttl preview clamps overrides above the policy cap", () => {
  const spec = new MultisigSpecBuilder()
    .setQuorum(1)
    .setTransactionTtlMs(60_000)
    .addSignatory("alice@wonderland", 1)
    .build();

  const preview = spec.previewProposalExpiry({ requestedTtlMs: 120_000, nowMs: 0 });
  assert.equal(preview.wasCapped, true);
  assert.equal(preview.policyCapMs, 60_000);
  assert.equal(preview.effectiveTtlMs, 60_000);
  assert.equal(preview.expiresAtMs, 60_000);
});

test("enforceProposalTtl rejects overrides above the policy cap", () => {
  const spec = new MultisigSpecBuilder()
    .setQuorum(1)
    .setTransactionTtlMs(10_000)
    .addSignatory("alice@wonderland", 1)
    .build();

  assert.throws(
    () => spec.enforceProposalTtl({ requestedTtlMs: 10_001 }),
    /exceeds the policy cap/,
  );
});

test("enforceProposalTtl returns preview for shorter overrides", () => {
  const spec = new MultisigSpecBuilder()
    .setQuorum(1)
    .setTransactionTtlMs(10_000)
    .addSignatory("alice@wonderland", 1)
    .build();

  const preview = spec.enforceProposalTtl({ requestedTtlMs: 2_000, nowMs: 100 });
  assert.equal(preview.wasCapped, false);
  assert.equal(preview.effectiveTtlMs, 2_000);
  assert.equal(preview.expiresAtMs, 2_100);
});

test("builder rejects oversized integer inputs", () => {
  const builder = new MultisigSpecBuilder()
    .setQuorum(1)
    .setTransactionTtlMs(10_000);
  const tooLarge = BigInt(Number.MAX_SAFE_INTEGER) + 1n;
  assert.throws(
    () => builder.addSignatory("alice@wonderland", tooLarge),
    /safe integer/i,
  );
});
