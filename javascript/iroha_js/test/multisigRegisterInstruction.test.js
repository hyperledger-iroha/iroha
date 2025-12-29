"use strict";

import test from "node:test";
import assert from "node:assert/strict";

import {
  buildRegisterMultisigInstruction,
  ValidationError,
  ValidationErrorCode,
} from "../src/index.js";
import { MultisigSpecBuilder } from "../src/multisig.js";

test("multisig register builder requires explicit controller id and matches domains", () => {
  const spec = new MultisigSpecBuilder()
    .setQuorum(2)
    .setTransactionTtlMs(60_000)
    .addSignatory("alice@wonderland", 1)
    .addSignatory("bob@wonderland", 1)
    .build();

  const payload = buildRegisterMultisigInstruction({
    accountId: "controller@wonderland",
    spec,
  });

  assert.deepEqual(payload, {
    Custom: {
      payload: {
        Register: {
          account: "controller@wonderland",
          spec: spec.toPayload(),
        },
      },
    },
  });
});

test("multisig register builder rejects controller domain drift", () => {
  const spec = new MultisigSpecBuilder()
    .setQuorum(1)
    .setTransactionTtlMs(10_000)
    .addSignatory("alice@wonderland", 1)
    .build();

  assert.throws(
    () =>
      buildRegisterMultisigInstruction({
        accountId: "controller@narnia",
        spec,
      }),
    (error) =>
      error instanceof ValidationError &&
      error.code === ValidationErrorCode.INVALID_STRING &&
      /domain narnia/.test(error.message),
  );
});
