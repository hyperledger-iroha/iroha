import assert from "node:assert/strict";
import { generateKeyPairSync } from "node:crypto";
import test from "node:test";

import { AccountAddress, ToriiClient } from "../src/index.js";

function ed25519PublicKeyBytes() {
  const { publicKey } = generateKeyPairSync("ed25519");
  const der = publicKey.export({ format: "der", type: "spki" });
  return new Uint8Array(der.subarray(der.length - 32));
}

function demoAccountId() {
  const address = AccountAddress.fromAccount({ publicKey: ed25519PublicKeyBytes() });
  return address.toI105();
}

function jsonResponse(status, body) {
  return new Response(body == null ? null : JSON.stringify(body), {
    status,
    headers: body == null ? undefined : { "Content-Type": "application/json" },
  });
}

const ACCOUNT_ID = demoAccountId();
const PROGRAM_ID = "identifier_lookup_retail";
const RECEIPT = {
  payload: {
    program_id: { name: PROGRAM_ID },
    program_digest: `hash:${"11".repeat(32).toUpperCase()}#ABCD`,
    backend: "bfv-programmed-sha3-256-v1",
    verification_mode: {
      mode: "Signed",
      value: null,
    },
    output_hash: `hash:${"22".repeat(32).toUpperCase()}#BCDE`,
    associated_data_hash: `hash:${"33".repeat(32).toUpperCase()}#CDEF`,
    executed_at_ms: 42,
    expires_at_ms: 142,
  },
  signature: "AA".repeat(64),
};

test("listRamLfeProgramPolicies normalizes BFV metadata", async () => {
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (input, init) => {
      assert.equal(init.method, "GET");
      assert.equal(new URL(input).pathname, "/v1/ram-lfe/program-policies");
      return jsonResponse(200, {
        total: 1,
        items: [
          {
            program_id: PROGRAM_ID,
            owner: ACCOUNT_ID,
            active: true,
            resolver_public_key: "ed25519:resolver-key",
            backend: "bfv-programmed-sha3-256-v1",
            verification_mode: "signed",
            input_encryption: "bfv-v1",
            input_encryption_public_parameters: "ABCD",
            input_encryption_public_parameters_decoded: {
              parameters: {
                polynomial_degree: 64,
                plaintext_modulus: 257,
                ciphertext_modulus: 1099511627776,
                decomposition_base_log: 12,
              },
              public_key: {
                b: [1, 2, 3],
                a: [4, 5, 6],
              },
              max_input_bytes: 32,
            },
            note: "retail programmed policy",
          },
        ],
      });
    },
  });

  const result = await client.listRamLfeProgramPolicies();
  assert.equal(result.total, 1);
  assert.equal(result.items[0].program_id, PROGRAM_ID);
  assert.equal(result.items[0].owner, ACCOUNT_ID);
  assert.equal(result.items[0].verification_mode, "signed");
  assert.equal(result.items[0].input_encryption, "bfv-v1");
  assert.equal(
    result.items[0].input_encryption_public_parameters_decoded.parameters.polynomial_degree,
    64,
  );
});

test("executeRamLfeProgram posts plaintext input and preserves raw receipt", async () => {
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (input, init) => {
      assert.equal(init.method, "POST");
      assert.equal(
        new URL(input).pathname,
        `/v1/ram-lfe/programs/${encodeURIComponent(PROGRAM_ID)}/execute`,
      );
      const payload = JSON.parse(init.body);
      assert.deepEqual(payload, {
        input_hex: "ABCD",
      });
      return jsonResponse(200, {
        program_id: PROGRAM_ID,
        opaque_hash: "opaque-hash-literal",
        receipt_hash: "receipt-hash-literal",
        output_hex: "C0FFEE",
        output_hash: "output-hash-literal",
        associated_data_hash: "associated-data-hash-literal",
        executed_at_ms: 42,
        expires_at_ms: 142,
        backend: "bfv-programmed-sha3-256-v1",
        verification_mode: "signed",
        receipt: RECEIPT,
      });
    },
  });

  const result = await client.executeRamLfeProgram(PROGRAM_ID, {
    inputHex: "ABCD",
  });
  assert.equal(result.program_id, PROGRAM_ID);
  assert.equal(result.output_hex, "C0FFEE");
  assert.equal(result.verification_mode, "signed");
  assert.deepEqual(result.receipt, RECEIPT);
});

test("executeRamLfeProgram returns null for missing programs", async () => {
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (_input, init) => {
      const payload = JSON.parse(init.body);
      assert.equal(payload.encrypted_input, "ABCD");
      return jsonResponse(404, {});
    },
  });

  const result = await client.executeRamLfeProgram(PROGRAM_ID, {
    encryptedInput: "ABCD",
  });
  assert.equal(result, null);
});

test("verifyRamLfeReceipt posts raw receipt payloads", async () => {
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (input, init) => {
      assert.equal(init.method, "POST");
      assert.equal(new URL(input).pathname, "/v1/ram-lfe/receipts/verify");
      const payload = JSON.parse(init.body);
      assert.deepEqual(payload, {
        receipt: RECEIPT,
        output_hex: "C0FFEE",
      });
      return jsonResponse(200, {
        valid: true,
        program_id: PROGRAM_ID,
        backend: "bfv-programmed-sha3-256-v1",
        verification_mode: "signed",
        output_hash: "output-hash-literal",
        associated_data_hash: "associated-data-hash-literal",
        output_hash_matches: true,
      });
    },
  });

  const result = await client.verifyRamLfeReceipt({
    receipt: RECEIPT,
    outputHex: "C0FFEE",
  });
  assert.equal(result.valid, true);
  assert.equal(result.program_id, PROGRAM_ID);
  assert.equal(result.output_hash_matches, true);
});
