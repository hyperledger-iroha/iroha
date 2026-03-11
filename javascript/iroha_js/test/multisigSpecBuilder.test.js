"use strict";

import test from "node:test";
import assert from "node:assert/strict";

import { MultisigSpecBuilder } from "../src/multisig.js";
import { AccountAddress } from "../src/address.js";

const DOMAIN = "wonderland";
const ALICE_KEY = Buffer.from(
  "B935AAF1F4E44B3DB79E5E5A9BA4569E6F3E2310C219F3DDD56D3277828D5480",
  "hex",
);
const BOB_KEY = Buffer.from(
  "641297079357229F295938A4B5A333DE35069BF47B9D0704E45805713D13C201",
  "hex",
);
const ALICE_ID = AccountAddress.fromAccount({ domain: DOMAIN, publicKey: ALICE_KEY }).toI105();
const BOB_ID = AccountAddress.fromAccount({ domain: DOMAIN, publicKey: BOB_KEY }).toI105();

test("builder produces deterministic payload and json", () => {
  const builder = new MultisigSpecBuilder()
    .setQuorum(3)
    .setTransactionTtlMs(60_000)
    .addSignatory(ALICE_ID, 2)
    .addSignatory(BOB_ID, 1);

  const spec = builder.build();
  assert.equal(spec.quorum, 3);
  assert.equal(spec.transactionTtlMs, 60_000);
  assert.deepEqual(spec.toPayload(), {
    signatories: {
      [ALICE_ID]: 2,
      [BOB_ID]: 1,
    },
    quorum: 3,
    transaction_ttl_ms: 60_000,
  });

  const json = builder.toJSON(true);
  assert.match(json, /\"transaction_ttl_ms\": 60000/);
  const ordered = [ALICE_ID, BOB_ID].sort();
  assert.ok(json.indexOf(ordered[0]) < json.indexOf(ordered[1]));
});

test("proposal ttl preview clamps overrides above the policy cap", () => {
  const spec = new MultisigSpecBuilder()
    .setQuorum(1)
    .setTransactionTtlMs(60_000)
    .addSignatory(ALICE_ID, 1)
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
    .addSignatory(ALICE_ID, 1)
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
    .addSignatory(ALICE_ID, 1)
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
    () => builder.addSignatory(ALICE_ID, tooLarge),
    /safe integer/i,
  );
});
