import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { ToriiClient } from "../src/toriiClient.js";

const fixture = JSON.parse(
  readFileSync(
    new URL("../../../fixtures/kotodama/entrypoint_argument_record_v1.json", import.meta.url),
    "utf8",
  ),
);

test("contract call preserves the shared Rust argument-record fixture at the Torii boundary", async () => {
  assert.equal(fixture.codec, "EntrypointArgumentRecordV1");
  assert.equal(fixture.generator, "ivm::encode_argument_record_from_json");
  assert.match(fixture.entrypoint_argument_schema_v1.schema_hash_hex, /^[0-9a-f]{64}$/u);
  assert.match(fixture.entrypoint_argument_record_v1.norito_hex, /^(?:[0-9a-f]{2})+$/u);

  let submittedBody;
  const boundary = fixture.torii_boundary;
  const fetchImpl = async (_url, init) => {
    submittedBody = JSON.parse(init.body);
    return new Response(
      JSON.stringify({
        ok: true,
        submitted: false,
        dataspace: "universal",
        code_hash_hex: "11".repeat(32),
        abi_hash_hex: "22".repeat(32),
        creation_time_ms: 1,
        entrypoint: boundary.entrypoint,
        transaction_scaffold_b64: "AQ==",
        signed_transaction_b64: "AQ==",
        signing_message_b64: Buffer.alloc(32, 1).toString("base64"),
        operation_receipt: {
          operation_kind: "contract_call",
          status: "pending_signature",
          transport: "torii",
          dataspace: "universal",
          contract_alias: boundary.contract_alias,
          entrypoint: boundary.entrypoint,
          gas_limit: boundary.fee_payment.value.gas_limit,
          fee_payment: boundary.fee_payment,
          payload_digest_hex: "33".repeat(32),
        },
      }),
      { status: 200, headers: { "content-type": "application/json" } },
    );
  };
  const client = new ToriiClient("https://fixture.invalid", { fetchImpl });

  await client.prepareContractCall(boundary);

  assert.deepEqual(submittedBody, {
    authority: boundary.authority,
    contract_alias: boundary.contract_alias,
    entrypoint: boundary.entrypoint,
    payload: boundary.payload,
    fee_payment: boundary.fee_payment,
  });
  assert.equal(Object.hasOwn(submittedBody, "argument_record"), false);
  assert.equal(Object.hasOwn(submittedBody, "argument_record_norito_hex"), false);
});
