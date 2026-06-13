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
const OPAQUE_HASH = "11".repeat(32);
const RECEIPT_HASH = "22".repeat(32);
const OUTPUT_HASH = "44".repeat(32);
const ASSOCIATED_DATA_HASH = "55".repeat(32);
const PROOF_SCHEMA_HASH = "66".repeat(32);
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

function ramLfeExecuteResponse(overrides = {}) {
  return {
    program_id: PROGRAM_ID,
    opaque_hash: OPAQUE_HASH,
    receipt_hash: RECEIPT_HASH,
    output_ciphertext: "C0FFEE",
    output_hash: OUTPUT_HASH,
    associated_data_hash: ASSOCIATED_DATA_HASH,
    executed_at_ms: 42,
    expires_at_ms: 142,
    backend: "bfv-programmed-sha3-256-v1",
    verification_mode: "signed",
    receipt: RECEIPT,
    ...overrides,
  };
}

function ramLfeReceiptVerifyResponse(overrides = {}) {
  return {
    valid: true,
    program_id: PROGRAM_ID,
    backend: "bfv-programmed-sha3-256-v1",
    verification_mode: "signed",
    output_hash: OUTPUT_HASH,
    associated_data_hash: ASSOCIATED_DATA_HASH,
    output_hash_matches: true,
    ...overrides,
  };
}

function ramLfeProgramPolicy(overrides = {}) {
  return {
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
    proof_verifier: {
      proof_backend: "halo2-ipa",
      circuit_id: "ram-lfe-v1",
      public_inputs_schema_hash: PROOF_SCHEMA_HASH,
      verifying_key_bytes_b64: "AQID",
    },
    note: "retail programmed policy",
    ...overrides,
  };
}

function ramLfeProgramPolicyListResponse(itemOverrides = {}) {
  return {
    total: 1,
    items: [ramLfeProgramPolicy(itemOverrides)],
  };
}

test("listRamLfeProgramPolicies parses exact BFV metadata", async () => {
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (input, init) => {
      assert.equal(init.method, "GET");
      assert.equal(new URL(input).pathname, "/v1/ram-lfe/program-policies");
      return jsonResponse(200, ramLfeProgramPolicyListResponse());
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
  assert.equal(result.items[0].proof_verifier.proof_backend, "halo2-ipa");
  assert.equal(result.items[0].proof_verifier.public_inputs_schema_hash, PROOF_SCHEMA_HASH);
});

test("listRamLfeProgramPolicies rejects non-exact policy metadata", async () => {
  const cases = [
    ["program_id", { program_id: ` ${PROGRAM_ID}` }],
    ["owner", { owner: `${ACCOUNT_ID} ` }],
    ["resolver_public_key", { resolver_public_key: " ed25519:resolver-key" }],
    ["backend", { backend: "BFV-programmed-sha3-256-v1" }],
    ["verification_mode", { verification_mode: " signed" }],
    ["input_encryption", { input_encryption: "bfv-v1 " }],
    ["input_encryption_public_parameters", { input_encryption_public_parameters: " ABCD" }],
  ];

  for (const [field, overrides] of cases) {
    const client = new ToriiClient("https://example.test", {
      fetchImpl: async () => jsonResponse(200, ramLfeProgramPolicyListResponse(overrides)),
    });
    await assert.rejects(
      () => client.listRamLfeProgramPolicies(),
      new RegExp(`ram-lfe program policy list response\\.items\\[0\\]\\.${field}`),
      `RAM-LFE program policy ${field} exactness`,
    );
  }
});

test("listRamLfeProgramPolicies rejects non-exact proof-verifier metadata", async () => {
  const cases = [
    ["proof_backend", { proof_backend: " halo2-ipa" }],
    ["circuit_id", { circuit_id: "ram-lfe-v1 " }],
    ["public_inputs_schema_hash", { public_inputs_schema_hash: ` ${PROOF_SCHEMA_HASH}` }],
    ["verifying_key_bytes_b64", { verifying_key_bytes_b64: "AQID " }],
  ];

  for (const [field, proofOverrides] of cases) {
    const client = new ToriiClient("https://example.test", {
      fetchImpl: async () =>
        jsonResponse(200, ramLfeProgramPolicyListResponse({
          proof_verifier: {
            ...ramLfeProgramPolicy().proof_verifier,
            ...proofOverrides,
          },
        })),
    });
    await assert.rejects(
      () => client.listRamLfeProgramPolicies(),
      new RegExp(
        `ram-lfe program policy list response\\.items\\[0\\]\\.proof_verifier\\.${field}`,
      ),
      `RAM-LFE proof verifier ${field} exactness`,
    );
  }
});

test("executeRamLfeProgram posts encrypted input and preserves raw receipt", async () => {
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (input, init) => {
      assert.equal(init.method, "POST");
      assert.equal(
        new URL(input).pathname,
        `/v1/ram-lfe/programs/${encodeURIComponent(PROGRAM_ID)}/execute`,
      );
      const payload = JSON.parse(init.body);
      assert.deepEqual(payload, {
        encrypted_input: "ABCD",
      });
      return jsonResponse(200, ramLfeExecuteResponse());
    },
  });

  const result = await client.executeRamLfeProgram(PROGRAM_ID, {
    encryptedInput: "ABCD",
  });
  assert.equal(result.program_id, PROGRAM_ID);
  assert.equal(result.output_ciphertext, "C0FFEE");
  assert.equal(result.output_hash, OUTPUT_HASH);
  assert.equal(result.verification_mode, "signed");
  assert.deepEqual(result.receipt, RECEIPT);
});

test("executeRamLfeProgram rejects non-exact response fields", async () => {
  const cases = [
    ["program_id", ramLfeExecuteResponse({ program_id: ` ${PROGRAM_ID}` })],
    ["opaque_hash", ramLfeExecuteResponse({ opaque_hash: `${OPAQUE_HASH} ` })],
    ["receipt_hash", ramLfeExecuteResponse({ receipt_hash: ` ${RECEIPT_HASH}` })],
    ["output_ciphertext", ramLfeExecuteResponse({ output_ciphertext: " C0FFEE" })],
    ["output_hash", ramLfeExecuteResponse({ output_hash: `${OUTPUT_HASH} ` })],
    [
      "associated_data_hash",
      ramLfeExecuteResponse({ associated_data_hash: ` ${ASSOCIATED_DATA_HASH}` }),
    ],
    ["backend", ramLfeExecuteResponse({ backend: "BFV-programmed-sha3-256-v1" })],
    ["verification_mode", ramLfeExecuteResponse({ verification_mode: " signed" })],
  ];

  for (const [field, body] of cases) {
    const client = new ToriiClient("https://example.test", {
      fetchImpl: async () => jsonResponse(200, body),
    });
    await assert.rejects(
      () => client.executeRamLfeProgram(PROGRAM_ID, { encryptedInput: "ABCD" }),
      new RegExp(`ram-lfe execute response\\.${field}`),
      `RAM-LFE execute response ${field} exactness`,
    );
  }
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

test("executeRamLfeProgram rejects unsupported inputHex option", async () => {
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async () => {
      throw new Error("request should not be sent");
    },
  });

  await assert.rejects(
    () => client.executeRamLfeProgram(PROGRAM_ID, { inputHex: "ABCD" }),
    (error) => error.name === "ValidationError",
  );
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
      return jsonResponse(200, ramLfeReceiptVerifyResponse());
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

test("verifyRamLfeReceipt rejects non-exact response fields", async () => {
  const cases = [
    ["program_id", ramLfeReceiptVerifyResponse({ program_id: `${PROGRAM_ID} ` })],
    ["backend", ramLfeReceiptVerifyResponse({ backend: " bfv-programmed-sha3-256-v1" })],
    ["verification_mode", ramLfeReceiptVerifyResponse({ verification_mode: "Signed" })],
    ["output_hash", ramLfeReceiptVerifyResponse({ output_hash: ` ${OUTPUT_HASH}` })],
    [
      "associated_data_hash",
      ramLfeReceiptVerifyResponse({ associated_data_hash: `${ASSOCIATED_DATA_HASH} ` }),
    ],
  ];

  for (const [field, body] of cases) {
    const client = new ToriiClient("https://example.test", {
      fetchImpl: async () => jsonResponse(200, body),
    });
    await assert.rejects(
      () => client.verifyRamLfeReceipt({ receipt: RECEIPT, outputHex: "C0FFEE" }),
      new RegExp(`ram-lfe receipt verify response\\.${field}`),
      `RAM-LFE receipt verify response ${field} exactness`,
    );
  }
});
