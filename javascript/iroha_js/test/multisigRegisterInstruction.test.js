"use strict";

import test from "node:test";
import assert from "node:assert/strict";

import {
  buildRegisterMultisigInstruction,
  ValidationError,
  ValidationErrorCode,
} from "../src/index.js";
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
const ALICE_ID = AccountAddress.fromAccount({ publicKey: ALICE_KEY }).toI105();
const BOB_ID = AccountAddress.fromAccount({ publicKey: BOB_KEY }).toI105();
const CONTROLLER_ID = AccountAddress.fromAccount({ publicKey: CONTROLLER_KEY,
}).toI105();

test("multisig register builder accepts encoded-only controller/signatory ids", () => {
  const spec = new MultisigSpecBuilder()
    .setQuorum(2)
    .setTransactionTtlMs(60_000)
    .addSignatory(ALICE_ID, 1)
    .addSignatory(BOB_ID, 1)
    .build();

  const payload = buildRegisterMultisigInstruction({
    accountId: CONTROLLER_ID,
    spec,
  });

  assert.deepEqual(payload, {
    Custom: {
      payload: {
        Register: {
          account: CONTROLLER_ID,
          spec: spec.toPayload(),
        },
      },
    },
  });
});

test("multisig register builder rejects legacy @domain account literals", () => {
  const spec = new MultisigSpecBuilder()
    .setQuorum(1)
    .setTransactionTtlMs(10_000)
    .addSignatory(`${ALICE_ID}@banka.dataspace`, 1)
    .build();

  assert.throws(
    () =>
      buildRegisterMultisigInstruction({
        accountId: `${CONTROLLER_ID}@banka.dataspace`,
        spec,
      }),
    (error) =>
      error instanceof ValidationError &&
      error.code === ValidationErrorCode.INVALID_ACCOUNT_ID &&
      /must not include '@domain'/i.test(error.message),
  );
});
