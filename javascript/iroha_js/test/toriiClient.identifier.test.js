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
  "4e52543000001042e5b988077612440e4cd45673596b00b0040000000000004887a2a6d485fb5100a804000000000000040000000000000020010000000000008800000000000000080000000000000008000000000000002cab6c00000000000800000000000000440e92000000000008000000000000005a25000000000000080000000000000049671100000000000800000000000000bd3e2300000000000800000000000000403d85000000000008000000000000005619f900000000000800000000000000bd73fc0000000000880000000000000008000000000000000800000000000000ed884300000000000800000000000000dc21b000000000000800000000000000fe7c50000000000008000000000000001639a3000000000008000000000000006b979b00000000000800000000000000ddd4410000000000080000000000000052086600000000000800000000000000ee13ae00000000002001000000000000880000000000000008000000000000000800000000000000d96d690000000000080000000000000092060e0000000000080000000000000034077500000000000800000000000000dcc4190000000000080000000000000062ea230000000000080000000000000055ef0a00000000000800000000000000ac52d400000000000800000000000000e945790000000000880000000000000008000000000000000800000000000000f3214400000000000800000000000000caedd2000000000008000000000000001cfb5b00000000000800000000000000d26e660000000000080000000000000016ec0e000000000008000000000000003cee83000000000008000000000000006d7ef900000000000800000000000000c2fbbb00000000002001000000000000880000000000000008000000000000000800000000000000c9c7eb00000000000800000000000000c8c04800000000000800000000000000ef1e8700000000000800000000000000aed22c000000000008000000000000006021990000000000080000000000000035ac8c00000000000800000000000000d24393000000000008000000000000008a206d0000000000880000000000000008000000000000000800000000000000407ded00000000000800000000000000d79c3400000000000800000000000000a0332c0000000000080000000000000091fe5700000000000800000000000000543de8000000000008000000000000005eb9df00000000000800000000000000a7c213000000000008000000000000006e03c20000000000200100000000000088000000000000000800000000000000080000000000000003d654000000000008000000000000005c874400000000000800000000000000567ab50000000000080000000000000007273100000000000800000000000000ff6d0a00000000000800000000000000077466000000000008000000000000006c1c1a000000000008000000000000006f4fc200000000008800000000000000080000000000000008000000000000002f884f0000000000080000000000000041b0a100000000000800000000000000caf929000000000008000000000000005848730000000000080000000000000061909200000000000800000000000000f5f5dd00000000000800000000000000435a3b000000000008000000000000009a9f690000000000";

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
  await assert.rejects(
    () =>
      client.resolveIdentifier({
        policyId: POLICY_ID,
        encryptedInput: "ABC",
        outputOpening: sampleOutputOpening(),
      }),
    (error) => error instanceof ValidationError,
  );
  await assert.rejects(
    () =>
      client.resolveIdentifier({
        policyId: POLICY_ID,
        encryptedInput: "ABCD",
        outputOpening: null,
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

test("encryptIdentifierInputForPolicy rejects adversarial BFV public parameters", () => {
  const basePolicy = {
    policy_id: "string#retail",
    owner: ACCOUNT_ID,
    active: true,
    normalization: "exact",
    resolver_public_key: "ed25519:ed0120" + "11".repeat(32),
    backend: "bfv-affine-sha3-256-v1",
    input_encryption: "bfv-v1",
  };
  const cloneParameters = () => JSON.parse(JSON.stringify(BFV_PUBLIC_PARAMETERS));
  const cases = [
    {
      name: "non-divisible ciphertext modulus",
      expected: ValidationError,
      params: (() => {
        const params = cloneParameters();
        params.parameters.ciphertext_modulus += 1;
        return params;
      })(),
    },
    {
      name: "non-power-of-two polynomial degree",
      expected: ValidationError,
      params: (() => {
        const params = cloneParameters();
        params.parameters.polynomial_degree = 63;
        return params;
      })(),
    },
    {
      name: "decomposition base outside supported range",
      expected: ValidationError,
      params: (() => {
        const params = cloneParameters();
        params.parameters.decomposition_base_log = 17;
        return params;
      })(),
    },
    {
      name: "public key length mismatch",
      expected: ValidationError,
      params: (() => {
        const params = cloneParameters();
        params.public_key.b = params.public_key.b.slice(1);
        return params;
      })(),
    },
    {
      name: "zero max input byte count",
      expected: ValidationError,
      params: (() => {
        const params = cloneParameters();
        params.max_input_bytes = 0;
        return params;
      })(),
    },
    {
      name: "public key coefficient outside modulus",
      expected: ValidationError,
      params: (() => {
        const params = cloneParameters();
        params.public_key.a[0] = params.parameters.ciphertext_modulus;
        return params;
      })(),
    },
    {
      name: "max input byte count outside one plaintext slot",
      expected: ValidationError,
      params: (() => {
        const params = cloneParameters();
        params.max_input_bytes = params.parameters.plaintext_modulus;
        return params;
      })(),
    },
    {
      name: "missing decoded public parameters",
      expected: /missing decoded BFV public parameters/,
      params: null,
    },
  ];

  for (const { name, expected, params } of cases) {
    assert.throws(
      () =>
        encryptIdentifierInputForPolicy(
          { ...basePolicy, input_encryption_public_parameters_decoded: params },
          "ab",
          { seedHex: BFV_SEED_HEX },
        ),
      expected,
      name,
    );
  }
});

test("encryptIdentifierInputForPolicy rejects adversarial client encryption inputs", () => {
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

  assert.throws(
    () => encryptIdentifierInputForPolicy(policy, "abcd", { seedHex: BFV_SEED_HEX }),
    ValidationError,
    "input longer than max_input_bytes must be rejected before encryption",
  );
  assert.throws(
    () =>
      encryptIdentifierInputForPolicy(policy, "ab", {
        seed: Buffer.alloc(32, 1),
        seedHex: BFV_SEED_HEX,
      }),
    ValidationError,
    "ambiguous deterministic seed inputs must be rejected",
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
  const policy = {
    policy_id: POLICY_ID,
    owner: ACCOUNT_ID,
    active: true,
    normalization: "phone_e164",
    resolver_public_key: "ed25519:ed0120" + "11".repeat(32),
    backend: "bfv-programmed-sha3-256-v1",
    input_encryption: "bfv-v1",
  };
  const opening = sampleOutputOpening();
  const cases = [
    {
      name: "plaintext input without client-side encryption",
      options: { input: " +1 (555) 123-4567 ", outputOpening: opening },
    },
    {
      name: "both plaintext and encrypted inputs",
      options: {
        input: " +1 (555) 123-4567 ",
        encryptedInput: "ABCD",
        encrypt: true,
        outputOpening: opening,
      },
    },
    {
      name: "seed with pre-encrypted input",
      options: { encryptedInput: "ABCD", seedHex: BFV_SEED_HEX, outputOpening: opening },
    },
    {
      name: "missing output opening",
      options: { encryptedInput: "ABCD" },
    },
    {
      name: "legacy plaintext hex alias",
      options: { encryptedInput: "ABCD", inputHex: "313233", outputOpening: opening },
    },
  ];

  for (const { name, options } of cases) {
    assert.throws(
      () => buildIdentifierRequestForPolicy(policy, options),
      ValidationError,
      name,
    );
  }
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
