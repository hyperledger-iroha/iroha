import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { contractPayloadDigestHex } from "../src/contractPayload.js";
import { NetworkId } from "../src/networkId.js";
import { LocalSigningContext, ToriiClient } from "../src/toriiClient.js";

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
  assert.equal(typeof boundary.payload.exact_int, "string");
  assert.equal(
    boundary.payload.exact_int,
    "1606938044258990275541962092341162602522202993782792835301376",
  );
  assert.equal(typeof boundary.payload.exact_decimal, "string");
  assert.equal(boundary.payload.exact_decimal, "-12345678901234567890.125");
  assert.equal(typeof boundary.payload.exact_quantity, "string");
  assert.equal(
    boundary.payload.exact_quantity,
    "12345678901234567890.0000000000000000000000000001",
  );
  const fetchImpl = async (_url, init) => {
    submittedBody = JSON.parse(init.body);
    return new Response("fixture boundary reached", { status: 503 });
  };
  const networkId = NetworkId.parse(
    "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
  );
  const client = new ToriiClient("https://fixture.invalid", {
    fetchImpl,
    localSigningContext: new LocalSigningContext(networkId),
  });

  await assert.rejects(
    client.prepareContractCall({
      ...boundary,
      draftIntent: {
        executableB64: "AQ==",
        metadataB64: "AA==",
        contractAddress:
          "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
        codeHashHex: "11".repeat(32),
        payloadDigestHex: contractPayloadDigestHex(boundary.payload),
      },
    }),
    /503/u,
  );

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
