import assert from "node:assert/strict";
import { generateKeyPairSync, sign as signRaw } from "node:crypto";
import test from "node:test";

import {
  AccountAddress,
  ToriiClient,
  buildIdentifierRequestForPolicy,
  encodeIdentifierResolutionReceiptPayload,
  encryptIdentifierInputForPolicy,
  getIdentifierBfvPublicParameters,
  normalizeIdentifierInput,
  verifyIdentifierResolutionReceipt,
} from "../src/index.js";
import { blake2b256 } from "../src/blake2b.js";
import { ValidationError } from "../src/validationError.js";

function ed25519PublicKeyBytes() {
  const { publicKey } = generateKeyPairSync("ed25519");
  const der = publicKey.export({ format: "der", type: "spki" });
  return new Uint8Array(der.subarray(der.length - 32));
}

function demoAccountId() {
  const address = AccountAddress.fromAccount({ publicKey: ed25519PublicKeyBytes() });
  return address.toI105();
}

const ACCOUNT_ID = demoAccountId();
const POLICY_ID = "phone#retail";
const OPAQUE_ID = `opaque:${"11".repeat(32)}`;
const RECEIPT_HASH = "22".repeat(32);
const UAID = `uaid:${"33".repeat(31)}35`;
const SIGNATURE = "AA".repeat(64);
const PROGRAM_DIGEST = "44".repeat(32);
const INPUT_CIPHERTEXT_HASH = "55".repeat(32);
const OUTPUT_CIPHERTEXT_HASH = "66".repeat(32);
const PARAMETER_DIGEST = "77".repeat(32);
const EVALUATION_KEY_DIGEST = "88".repeat(32);
const OUTPUT_HASH = "99".repeat(32);
const ASSOCIATED_DATA_HASH = "aa".repeat(32);
const BFV_SEED_HEX =
  "00112233445566778899AABBCCDDEEFF00112233445566778899AABBCCDDEEFF";
const BFV_PUBLIC_PARAMETERS = {
  parameters: {
    polynomial_degree: 8,
    plaintext_modulus: 257,
    ciphertext_modulus: 16_842_752,
    decomposition_base_log: 12,
  },
  public_key: {
    a: [3503246, 2379264, 12091019, 30169, 15804162, 8155629, 2418997, 3003107],
    b: [11472226, 15791131, 10301391, 6321610, 502045, 1948157, 5332249, 12641494],
  },
  max_input_bytes: 3,
};
const BFV_ENCRYPTED_INPUT_HEX =
  "4e52543000001042e5b988077612440e4cd45673596b00b00400000000000075615d8ccdac6dc500a804000000000000040000000000000020010000000000008800000000000000080000000000000008000000000000002bab6e00000000000800000000000000440e92000000000008000000000000005b2500000000000008000000000000004a671100000000000800000000000000bc3e2300000000000800000000000000413d85000000000008000000000000005619f900000000000800000000000000bd73fc0000000000880000000000000008000000000000000800000000000000ee884300000000000800000000000000dd21b000000000000800000000000000fe7c50000000000008000000000000001639a3000000000008000000000000006a979b00000000000800000000000000ddd4410000000000080000000000000051086600000000000800000000000000ef13ae00000000002001000000000000880000000000000008000000000000000800000000000000776dca0000000000080000000000000093060e0000000000080000000000000033077500000000000800000000000000ddc4190000000000080000000000000062ea230000000000080000000000000056ef0a00000000000800000000000000ab52d400000000000800000000000000e945790000000000880000000000000008000000000000000800000000000000f2214400000000000800000000000000c9edd2000000000008000000000000001dfb5b00000000000800000000000000d16e660000000000080000000000000016ec0e000000000008000000000000003dee83000000000008000000000000006e7ef900000000000800000000000000c1fbbb0000000000200100000000000088000000000000000800000000000000080000000000000066c74c00000000000800000000000000c9c04800000000000800000000000000f01e8700000000000800000000000000aed22c000000000008000000000000006121990000000000080000000000000036ac8c00000000000800000000000000d143930000000000080000000000000089206d0000000000880000000000000008000000000000000800000000000000417ded00000000000800000000000000d79c34000000000008000000000000009f332c0000000000080000000000000091fe5700000000000800000000000000533de8000000000008000000000000005db9df00000000000800000000000000a8c213000000000008000000000000006e03c20000000000200100000000000088000000000000000800000000000000080000000000000003d654000000000008000000000000005d874400000000000800000000000000567ab50000000000080000000000000007273100000000000800000000000000ff6d0a00000000000800000000000000077466000000000008000000000000006d1c1a00000000000800000000000000704fc200000000008800000000000000080000000000000008000000000000002f884f0000000000080000000000000041b0a100000000000800000000000000cbf929000000000008000000000000005748730000000000080000000000000060909200000000000800000000000000f5f5dd00000000000800000000000000445a3b00000000000800000000000000999f690000000000";

function jsonResponse(status, body) {
  return new Response(body == null ? null : JSON.stringify(body), {
    status,
    headers: body == null ? undefined : { "Content-Type": "application/json" },
  });
}

function irohaPrehash(bytes) {
  const digest = Buffer.from(blake2b256(bytes));
  digest[digest.length - 1] |= 1;
  return digest;
}

function ed25519MultihashLiteral(publicKeyBytes) {
  return `ed25519:ed0120${Buffer.from(publicKeyBytes).toString("hex").toUpperCase()}`;
}

function sampleOutputOpening(overrides = {}) {
  return {
    payload: {
      program_id: POLICY_ID,
      input_ciphertext_hash: INPUT_CIPHERTEXT_HASH,
      output_ciphertext_hash: OUTPUT_CIPHERTEXT_HASH,
      parameter_digest: PARAMETER_DIGEST,
      evaluation_key_digest: EVALUATION_KEY_DIGEST,
      opened_output_hash: OUTPUT_HASH,
      opened_at_ms: 42,
      expires_at_ms: 142,
      ...(overrides.payload ?? {}),
    },
    signature: overrides.signature ?? "ab".repeat(64),
  };
}

function sampleExecution(overrides = {}) {
  return {
    program_id: POLICY_ID,
    program_digest: PROGRAM_DIGEST,
    backend: "bfv-programmed-sha3-256-v1",
    verification_mode: "signed",
    input_ciphertext_hash: INPUT_CIPHERTEXT_HASH,
    output_ciphertext_hash: OUTPUT_CIPHERTEXT_HASH,
    parameter_digest: PARAMETER_DIGEST,
    evaluation_key_digest: EVALUATION_KEY_DIGEST,
    output_hash: OUTPUT_HASH,
    associated_data_hash: ASSOCIATED_DATA_HASH,
    executed_at_ms: 42,
    expires_at_ms: 142,
    ...overrides,
  };
}

function signedReceiptFixture(overrides = {}) {
  const { privateKey, publicKey } = generateKeyPairSync("ed25519");
  const der = publicKey.export({ format: "der", type: "spki" });
  const rawPublicKey = new Uint8Array(der.subarray(der.length - 32));
  const payload = {
    policy_id: POLICY_ID,
    execution: sampleExecution(overrides.execution),
    opening: overrides.opening ?? sampleOutputOpening(),
    opaque_id: OPAQUE_ID,
    receipt_hash: RECEIPT_HASH,
    uaid: UAID,
    account_id: ACCOUNT_ID,
    ...(overrides.payload ?? {}),
  };
  const signaturePayload = encodeIdentifierResolutionReceiptPayload(payload);
  const signature = signRaw(
    null,
    irohaPrehash(signaturePayload),
    privateKey,
  ).toString("hex").toUpperCase();
  return {
    resolver_public_key: ed25519MultihashLiteral(rawPublicKey),
    payload,
    attestation: { kind: "signed", signature },
  };
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
  assert.equal(
    result.items[0].input_encryption_public_parameters_decoded.parameters.polynomial_degree,
    64,
  );
  assert.deepEqual(getIdentifierBfvPublicParameters(result.items[0]), {
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
  });
});

test("resolveIdentifier posts encrypted input with output opening and normalizes response", async () => {
  let lastRequest = null;
  const signedReceipt = signedReceiptFixture();
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (input, init) => {
      lastRequest = { input, init };
      assert.equal(init.method, "POST");
      const payload = JSON.parse(init.body);
      assert.deepEqual(payload, {
        policy_id: POLICY_ID,
        encrypted_input: "ABCD",
        output_opening: sampleOutputOpening(),
      });
      return jsonResponse(200, {
        payload: signedReceipt.payload,
        attestation: signedReceipt.attestation,
      });
    },
  });

  const result = await client.resolveIdentifier({
    policyId: POLICY_ID,
    encryptedInput: "ABCD",
    outputOpening: sampleOutputOpening(),
  });
  assert.equal(new URL(lastRequest.input).pathname, "/v1/identifiers/resolve");
  assert.equal(result.payload.policy_id, POLICY_ID);
  assert.equal(result.payload.opaque_id, OPAQUE_ID);
  assert.equal(result.payload.receipt_hash, RECEIPT_HASH);
  assert.equal(result.payload.uaid, UAID);
  assert.equal(result.payload.account_id, ACCOUNT_ID);
  assert.equal(result.attestation.signature, signedReceipt.attestation.signature);
  assert.equal(result.payload.execution.input_ciphertext_hash, INPUT_CIPHERTEXT_HASH);
  assert.equal(
    verifyIdentifierResolutionReceipt(result, {
      policy_id: POLICY_ID,
      owner: ACCOUNT_ID,
      active: true,
      normalization: "phone_e164",
      resolver_public_key: signedReceipt.resolver_public_key,
      backend: "bfv-programmed-sha3-256-v1",
    }),
    true,
  );
});

test("resolveIdentifier requires encrypted input and output opening", async () => {
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
        outputOpening: sampleOutputOpening(),
      }),
    (error) => error instanceof ValidationError,
  );
  await assert.rejects(
    () =>
      client.resolveIdentifier({
        policyId: POLICY_ID,
        encryptedInput: "ABCD",
      }),
    (error) => error instanceof ValidationError,
  );
});

test("encryptIdentifierInputForPolicy builds deterministic BFV Norito envelopes", () => {
  const policy = {
    policy_id: "string#retail",
    owner: ACCOUNT_ID,
    active: true,
    normalization: "exact",
    resolver_public_key: "ed25519:ed0120" + "11".repeat(32),
    backend: "bfv-affine-sha3-256-v1",
    input_encryption: "bfv-v1",
    input_encryption_public_parameters_decoded: BFV_PUBLIC_PARAMETERS,
  };

  assert.equal(
    encryptIdentifierInputForPolicy(policy, "ab", { seedHex: BFV_SEED_HEX }),
    BFV_ENCRYPTED_INPUT_HEX,
  );
  assert.deepEqual(
    buildIdentifierRequestForPolicy(policy, {
      input: "ab",
      encrypt: true,
      seedHex: BFV_SEED_HEX,
      outputOpening: sampleOutputOpening(),
    }),
    {
      policyId: "string#retail",
      encryptedInput: BFV_ENCRYPTED_INPUT_HEX,
      outputOpening: sampleOutputOpening(),
    },
  );
});

test("resolveIdentifier accepts encrypted input and returns null for missing bindings", async () => {
  let callCount = 0;
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (_input, init) => {
      callCount += 1;
      const payload = JSON.parse(init.body);
      assert.equal(payload.encrypted_input, "ABCD");
      assert.deepEqual(payload.output_opening, sampleOutputOpening());
      return jsonResponse(404, {});
    },
  });

  const result = await client.resolveIdentifier({
    policyId: POLICY_ID,
    encryptedInput: "ABCD",
    outputOpening: sampleOutputOpening(),
  });
  assert.equal(callCount, 1);
  assert.equal(result, null);
});

test("issueIdentifierClaimReceipt posts account-scoped requests", async () => {
  const outputOpening = sampleOutputOpening({ payload: { opened_at_ms: 7, expires_at_ms: null } });
  const signedReceipt = signedReceiptFixture({
    execution: { executed_at_ms: 7, expires_at_ms: null },
    opening: outputOpening,
  });
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
        output_opening: outputOpening,
      });
      return jsonResponse(200, {
        payload: signedReceipt.payload,
        attestation: signedReceipt.attestation,
      });
    },
  });

  const result = await client.issueIdentifierClaimReceipt(ACCOUNT_ID, {
    policyId: POLICY_ID,
    encryptedInput: "ABCD",
    outputOpening,
  });
  assert.equal(result.payload.opaque_id, OPAQUE_ID);
  assert.equal(result.payload.account_id, ACCOUNT_ID);
});

test("issueIdentifierClaimReceipt accepts account aliases on account-id paths", async () => {
  const alias = "operator@banka.universal";
  const outputOpening = sampleOutputOpening({ payload: { opened_at_ms: 7, expires_at_ms: null } });
  const signedReceipt = signedReceiptFixture({
    execution: { executed_at_ms: 7, expires_at_ms: null },
    opening: outputOpening,
  });
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (input, init) => {
      assert.equal(
        new URL(input).pathname,
        `/v1/accounts/${encodeURIComponent(alias)}/identifiers/claim-receipt`,
      );
      const payload = JSON.parse(init.body);
      assert.deepEqual(payload.output_opening, outputOpening);
      return jsonResponse(200, {
        payload: signedReceipt.payload,
        attestation: signedReceipt.attestation,
      });
    },
  });

  const result = await client.issueIdentifierClaimReceipt(alias, {
    policyId: POLICY_ID,
    encryptedInput: "ABCD",
    outputOpening,
  });
  assert.equal(result.payload.account_id, ACCOUNT_ID);
});

test("buildIdentifierRequestForPolicy rejects plaintext request bodies", () => {
  assert.throws(
    () =>
      buildIdentifierRequestForPolicy(
        {
          policy_id: POLICY_ID,
          owner: ACCOUNT_ID,
          active: true,
          normalization: "phone_e164",
          resolver_public_key: "ed25519:ed0120" + "11".repeat(32),
          backend: "bfv-programmed-sha3-256-v1",
          input_encryption: "bfv-v1",
        },
        { input: " +1 (555) 123-4567 ", outputOpening: sampleOutputOpening() },
      ),
    ValidationError,
  );
});

test("getIdentifierClaimByReceiptHash returns null on 404", async () => {
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (input, init) => {
      assert.equal(init.method, "GET");
      assert.equal(
        new URL(input).pathname,
        `/v1/identifiers/receipts/${RECEIPT_HASH}`,
      );
      return jsonResponse(404, {});
    },
  });
  const result = await client.getIdentifierClaimByReceiptHash(RECEIPT_HASH);
  assert.equal(result, null);
});

test("normalizeIdentifierInput remains available from the public SDK entrypoint", () => {
  assert.equal(
    normalizeIdentifierInput(" Alice.Example@Example.COM ", "email_address", "email"),
    "alice.example@example.com",
  );
});
