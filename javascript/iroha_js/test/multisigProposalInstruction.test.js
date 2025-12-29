"use strict";

import test from "node:test";
import assert from "node:assert/strict";

import { buildProposeMultisigInstruction, ValidationError } from "../src/index.js";
import { MultisigSpecBuilder } from "../src/multisig.js";

const sampleSpec = () =>
  new MultisigSpecBuilder()
    .setQuorum(2)
    .setTransactionTtlMs(60_000)
    .addSignatory("alice@wonderland", 1)
    .addSignatory("bob@wonderland", 1)
    .build();

test("multisig propose builder enforces TTL cap", () => {
  const spec = sampleSpec();
  assert.throws(
    () =>
      buildProposeMultisigInstruction({
        accountId: "controller@wonderland",
        instructions: [{ Log: { Level: "INFO", message: "hello" } }],
        spec,
        transactionTtlMs: 120_000,
      }),
    (error) => error instanceof RangeError && /policy cap 60000/.test(error.message),
  );
});

test("multisig propose builder accepts TTL at or below cap", () => {
  const spec = sampleSpec();
  const payload = buildProposeMultisigInstruction({
    accountId: "controller@wonderland",
    instructions: [{ Log: { Level: "INFO", message: "hello" } }],
    spec,
    transactionTtlMs: 30_000,
  });

  assert.deepEqual(payload, {
    Custom: {
      payload: {
        Propose: {
          account: "controller@wonderland",
          instructions: [{ Log: { Level: "INFO", message: "hello" } }],
          transaction_ttl_ms: 30_000,
        },
      },
    },
  });
});

test("multisig propose builder requires instructions", () => {
  const spec = sampleSpec();
  assert.throws(
    () =>
      buildProposeMultisigInstruction({
        accountId: "controller@wonderland",
        instructions: [],
        spec,
      }),
    (error) => error instanceof TypeError && /instructions/.test(error.message),
  );
});

test("multisig propose builder propagates domain drift", () => {
  const spec = sampleSpec();
  assert.throws(
    () =>
      buildProposeMultisigInstruction({
        accountId: "controller@narnia",
        instructions: [{ Log: { Level: "INFO", message: "hello" } }],
        spec,
      }),
    (error) => error instanceof ValidationError && /domain narnia/.test(error.message),
  );
});
