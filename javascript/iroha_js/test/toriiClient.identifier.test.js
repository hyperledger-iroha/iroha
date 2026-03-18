import assert from "node:assert/strict";
import { generateKeyPairSync } from "node:crypto";
import test from "node:test";

import { AccountAddress, ToriiClient, normalizeIdentifierInput } from "../src/index.js";
import { ValidationError } from "../src/validationError.js";

function ed25519PublicKeyBytes() {
  const { publicKey } = generateKeyPairSync("ed25519");
  const der = publicKey.export({ format: "der", type: "spki" });
  return new Uint8Array(der.subarray(der.length - 32));
}

function demoAccountId(domain) {
  const address = AccountAddress.fromAccount({ domain, publicKey: ed25519PublicKeyBytes() });
  return address.toI105();
}

const ACCOUNT_ID = demoAccountId("directory");
const POLICY_ID = "phone#retail";
const OPAQUE_ID = `opaque:${"11".repeat(32)}`;
const RECEIPT_HASH = "22".repeat(32);
const UAID = `uaid:${"33".repeat(31)}35`;
const SIGNATURE = "AA".repeat(64);

function jsonResponse(status, body) {
  return new Response(body == null ? null : JSON.stringify(body), {
    status,
    headers: body == null ? undefined : { "Content-Type": "application/json" },
  });
}

test("listIdentifierPolicies normalizes BFV metadata", async () => {
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (input, init) => {
      assert.equal(init.method, "GET");
      assert.equal(new URL(input).pathname, "/v1/identifier-policies");
      return jsonResponse(200, {
        total: 1,
        items: [
          {
            policy_id: POLICY_ID,
            owner: ACCOUNT_ID,
            active: true,
            normalization: "phone_e164",
            resolver_public_key: "ed25519:resolver-key",
            backend: "bfv-affine-sha3-256-v1",
            input_encryption: "bfv-v1",
            input_encryption_public_parameters: "ABCD",
            note: "retail phone policy",
          },
        ],
      });
    },
  });

  const result = await client.listIdentifierPolicies();
  assert.equal(result.total, 1);
  assert.equal(result.items[0].policy_id, POLICY_ID);
  assert.equal(result.items[0].owner, ACCOUNT_ID);
  assert.equal(result.items[0].input_encryption, "bfv-v1");
  assert.equal(result.items[0].input_encryption_public_parameters, "ABCD");
});

test("resolveIdentifier posts plaintext input and normalizes response", async () => {
  let lastRequest = null;
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (input, init) => {
      lastRequest = { input, init };
      assert.equal(init.method, "POST");
      const payload = JSON.parse(init.body);
      assert.deepEqual(payload, {
        policy_id: POLICY_ID,
        input: "+1 (555) 123-4567",
      });
      return jsonResponse(200, {
        policy_id: POLICY_ID,
        opaque_id: OPAQUE_ID,
        receipt_hash: RECEIPT_HASH,
        uaid: UAID,
        account_id: ACCOUNT_ID,
        resolved_at_ms: 42,
        expires_at_ms: 142,
        backend: "bfv-affine-sha3-256-v1",
        signature: SIGNATURE,
      });
    },
  });

  const result = await client.resolveIdentifier({
    policyId: POLICY_ID,
    input: " +1 (555) 123-4567 ",
  });
  assert.equal(new URL(lastRequest.input).pathname, "/v1/identifiers/resolve");
  assert.equal(result.policy_id, POLICY_ID);
  assert.equal(result.opaque_id, OPAQUE_ID);
  assert.equal(result.receipt_hash, RECEIPT_HASH);
  assert.equal(result.uaid, UAID);
  assert.equal(result.account_id, ACCOUNT_ID);
  assert.equal(result.signature, SIGNATURE);
});

test("resolveIdentifier validates exactly one input mode", async () => {
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async () => jsonResponse(200, {}),
  });

  await assert.rejects(
    () => client.resolveIdentifier({ policyId: POLICY_ID }),
    (error) => error instanceof ValidationError,
  );
  await assert.rejects(
    () =>
      client.resolveIdentifier({
        policyId: POLICY_ID,
        input: "alice@example.com",
        encryptedInput: "ABCD",
      }),
    (error) => error instanceof ValidationError,
  );
});

test("resolveIdentifier accepts encrypted input and returns null for missing bindings", async () => {
  let callCount = 0;
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (_input, init) => {
      callCount += 1;
      const payload = JSON.parse(init.body);
      assert.equal(payload.encrypted_input, "ABCD");
      return jsonResponse(404, {});
    },
  });

  const result = await client.resolveIdentifier({
    policyId: POLICY_ID,
    encryptedInput: "ABCD",
  });
  assert.equal(callCount, 1);
  assert.equal(result, null);
});

test("issueIdentifierClaimReceipt posts account-scoped requests", async () => {
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (input, init) => {
      assert.equal(
        new URL(input).pathname,
        `/v1/accounts/${encodeURIComponent(ACCOUNT_ID)}/identifiers/claim-receipt`,
      );
      const payload = JSON.parse(init.body);
      assert.deepEqual(payload, {
        policy_id: POLICY_ID,
        encrypted_input: "ABCD",
      });
      return jsonResponse(200, {
        policy_id: POLICY_ID,
        opaque_id: OPAQUE_ID,
        receipt_hash: RECEIPT_HASH,
        uaid: UAID,
        account_id: ACCOUNT_ID,
        resolved_at_ms: 7,
        backend: "bfv-affine-sha3-256-v1",
        signature: SIGNATURE,
      });
    },
  });

  const result = await client.issueIdentifierClaimReceipt(ACCOUNT_ID, {
    policyId: POLICY_ID,
    encryptedInput: "ABCD",
  });
  assert.equal(result.opaque_id, OPAQUE_ID);
  assert.equal(result.account_id, ACCOUNT_ID);
});

test("normalizeIdentifierInput remains available from the public SDK entrypoint", () => {
  assert.equal(
    normalizeIdentifierInput(" Alice.Example@Example.COM ", "email_address", "email"),
    "alice.example@example.com",
  );
});
