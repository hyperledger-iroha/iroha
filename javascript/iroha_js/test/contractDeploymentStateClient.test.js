import assert from "node:assert/strict";
import test from "node:test";

import { ed25519 } from "@noble/curves/ed25519";

import { AccountAddress } from "../src/address.js";
import { verifyEd25519 } from "../src/crypto.browser.js";
import { canonicalRequestSignatureMessage } from "../src/canonicalRequest.js";
import { ToriiBrowserClient } from "../src/toriiBrowserClient.js";

const PRIVATE_KEY = Buffer.from(
  "CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53",
  "hex",
);
const PUBLIC_KEY = Buffer.from(ed25519.getPublicKey(PRIVATE_KEY));
const AUTHORITY = AccountAddress.fromAccount({
  algorithm: "ed25519",
  publicKey: PUBLIC_KEY,
}).toI105(753);

function hashLiteral(hex) {
  const body = hex.toUpperCase();
  let crc = 0xffff;
  for (const byte of Buffer.from(`hash:${body}`, "utf8")) {
    crc ^= (byte & 0xff) << 8;
    for (let bit = 0; bit < 8; bit += 1) {
      crc =
        (crc & 0x8000) !== 0
          ? ((crc << 1) ^ 0x1021) & 0xffff
          : (crc << 1) & 0xffff;
    }
  }
  return `hash:${body}#${crc.toString(16).toUpperCase().padStart(4, "0")}`;
}

function stateResponse(overrides = {}) {
  return {
    authority: AUTHORITY,
    contract_alias: "demo::universal",
    deploy_nonce: "7",
    dataspace_alias: "universal",
    dataspace_id: "0",
    previous_contract_address: null,
    observed_block_height: "11",
    observed_block_hash: hashLiteral("ab".repeat(32)),
    ledger_time_ms: "123456",
    chain_discriminant: "753",
    ...overrides,
  };
}

test("deployment-state client signs the exact body and validates the exact DTO", async () => {
  const timestampMs = 1_717_171_717_000;
  const nonce = "deployment-state-nonce";
  let observed = null;
  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async (url, init) => {
      observed = { url: String(url), init };
      return new Response(JSON.stringify(stateResponse()), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    },
  });

  const result = await client.getContractDeploymentState(
    {
      authority: AUTHORITY,
      contract_alias: "demo::universal",
    },
    {
      authAccountId: "operator@universal",
      timestampMs,
      nonce,
      sign: ({ message }) => ed25519.sign(message, PRIVATE_KEY),
    },
  );

  assert.equal(observed.url, "https://torii.example/v1/contracts/deployment-state");
  assert.equal(
    observed.init.body,
    JSON.stringify({ authority: AUTHORITY, contract_alias: "demo::universal" }),
  );
  assert.equal(observed.init.headers["X-Iroha-Account"], "operator@universal");
  const message = canonicalRequestSignatureMessage({
    method: "POST",
    path: "/v1/contracts/deployment-state",
    body: observed.init.body,
    timestampMs,
    nonce,
  });
  assert.equal(
    verifyEd25519(
      message,
      Buffer.from(observed.init.headers["X-Iroha-Signature"], "base64"),
      PUBLIC_KEY,
    ),
    true,
  );
  assert.deepEqual(result, stateResponse());
  assert.equal(Object.isFrozen(result), true);
});

test("deployment-state client rejects request and response shape drift", async () => {
  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () =>
      new Response(JSON.stringify(stateResponse({ extra: true })), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
  });

  await assert.rejects(
    client.getContractDeploymentState(
      { authority: AUTHORITY, contract_alias: "demo::universal", extra: true },
      { headers: {} },
    ),
    /requires exactly authority and contract_alias/u,
  );
  await assert.rejects(
    client.getContractDeploymentState(
      { authority: AUTHORITY, contract_alias: "demo::universal" },
      { headers: {} },
    ),
    /missing or unsupported fields/u,
  );
});
