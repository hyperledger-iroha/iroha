"use strict";

import test from "node:test";
import assert from "node:assert/strict";

import { buildProposeMultisigInstruction, ValidationError } from "../src/index.js";
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
const CONTROLLER_KEY = Buffer.from(
  "B7D3A8A20C1EF77F6C2B7B4AA3AA7B4D52A7B2FAF77F0F45B1A16E7A8E0B3C01",
  "hex",
);
const ALICE_ID = AccountAddress.fromAccount({ domain: DOMAIN, publicKey: ALICE_KEY }).toIH58();
const BOB_ID = AccountAddress.fromAccount({ domain: DOMAIN, publicKey: BOB_KEY }).toIH58();
const CONTROLLER_ID = AccountAddress.fromAccount({
  domain: DOMAIN,
  publicKey: CONTROLLER_KEY,
}).toIH58();

const sampleSpec = () =>
  new MultisigSpecBuilder()
    .setQuorum(2)
    .setTransactionTtlMs(60_000)
    .addSignatory(ALICE_ID, 1)
    .addSignatory(BOB_ID, 1)
    .build();

test("multisig propose builder enforces TTL cap", () => {
  const spec = sampleSpec();
  assert.throws(
    () =>
      buildProposeMultisigInstruction({
        accountId: CONTROLLER_ID,
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
    accountId: CONTROLLER_ID,
    instructions: [{ Log: { Level: "INFO", message: "hello" } }],
    spec,
    transactionTtlMs: 30_000,
  });

  assert.deepEqual(payload, {
    Custom: {
      payload: {
        Propose: {
          account: CONTROLLER_ID,
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
        accountId: CONTROLLER_ID,
        instructions: [],
        spec,
      }),
    (error) => error instanceof TypeError && /instructions/.test(error.message),
  );
});

test("multisig propose builder propagates domain drift", () => {
  const spec = new MultisigSpecBuilder()
    .setQuorum(2)
    .setTransactionTtlMs(60_000)
    .addSignatory("alice@wonderland", 1)
    .addSignatory("bob@wonderland", 1)
    .build();
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
