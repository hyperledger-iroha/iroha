import assert from "node:assert/strict";
import { generateKeyPairSync, sign as signRaw } from "node:crypto";
import test from "node:test";

import {
  AccountAddress,
  ToriiClient,
  buildIdentifierRequestForPolicy,
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
const BFV_SEED_HEX =
  "00112233445566778899AABBCCDDEEFF00112233445566778899AABBCCDDEEFF";
const BFV_PUBLIC_PARAMETERS = {
  parameters: {
    polynomial_degree: 8,
    plaintext_modulus: 256,
    ciphertext_modulus: 16_777_216,
    decomposition_base_log: 12,
  },
  public_key: {
    a: [3503246, 2379264, 12091019, 30169, 15804162, 8155629, 2418997, 3003107],
    b: [11472226, 15791131, 10301391, 6321610, 502045, 1948157, 5332249, 12641494],
  },
  max_input_bytes: 3,
};
const BFV_ENCRYPTED_INPUT_HEX =
  "4e525430000035a9bf76d68dbb0c35a9bf76d68dbb0c00b0040000000000007f6fd892e275492500a804000000000000040000000000000020010000000000008800000000000000080000000000000008000000000000002bab6f00000000000800000000000000440e93000000000008000000000000005b2502000000000008000000000000004a671400000000000800000000000000bc3e2600000000000800000000000000413d86000000000008000000000000005619f800000000000800000000000000bd73fa0000000000880000000000000008000000000000000800000000000000ee884300000000000800000000000000dd21b100000000000800000000000000fe7c52000000000008000000000000001639a5000000000008000000000000006a979d00000000000800000000000000ddd4430000000000080000000000000051086700000000000800000000000000ef13ae00000000002001000000000000880000000000000008000000000000000800000000000000776dc80000000000080000000000000093060d0000000000080000000000000033077500000000000800000000000000ddc4190000000000080000000000000062ea230000000000080000000000000056ef0b00000000000800000000000000ab52d500000000000800000000000000e9457c0000000000880000000000000008000000000000000800000000000000f2214200000000000800000000000000c9edcf000000000008000000000000001dfb5a00000000000800000000000000d16e640000000000080000000000000016ec0f000000000008000000000000003dee83000000000008000000000000006e7efa00000000000800000000000000c1fbbc0000000000200100000000000088000000000000000800000000000000080000000000000066c74d00000000000800000000000000c9c04900000000000800000000000000f01e8700000000000800000000000000aed22c000000000008000000000000006121980000000000080000000000000036ac8d00000000000800000000000000d143930000000000080000000000000089206d0000000000880000000000000008000000000000000800000000000000417ded00000000000800000000000000d79c33000000000008000000000000009f332d0000000000080000000000000091fe5700000000000800000000000000533de8000000000008000000000000005db9df00000000000800000000000000a8c213000000000008000000000000006e03c20000000000200100000000000088000000000000000800000000000000080000000000000003d656000000000008000000000000005d874500000000000800000000000000567ab30000000000080000000000000007272f00000000000800000000000000ff6d0a00000000000800000000000000077467000000000008000000000000006d1c1a00000000000800000000000000704fc100000000008800000000000000080000000000000008000000000000002f884f0000000000080000000000000041b0a000000000000800000000000000cbf92a000000000008000000000000005748720000000000080000000000000060909200000000000800000000000000f5f5dc00000000000800000000000000445a3a00000000000800000000000000999f680000000000";

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

function signedReceiptFixture(overrides = {}) {
  const { privateKey, publicKey } = generateKeyPairSync("ed25519");
  const der = publicKey.export({ format: "der", type: "spki" });
  const rawPublicKey = new Uint8Array(der.subarray(der.length - 32));
  const signaturePayloadHex = overrides.signaturePayloadHex ?? "01020304A0";
  const signature = signRaw(
    null,
    irohaPrehash(Buffer.from(signaturePayloadHex, "hex")),
    privateKey,
  ).toString("hex").toUpperCase();
  return {
    resolver_public_key: ed25519MultihashLiteral(rawPublicKey),
    signature,
    signature_payload_hex: signaturePayloadHex,
    signature_payload: {
      policy_id: POLICY_ID,
      opaque_id: OPAQUE_ID,
      receipt_hash: RECEIPT_HASH,
      uaid: UAID,
      account_id: ACCOUNT_ID,
      resolved_at_ms: 42,
      expires_at_ms: 142,
    },
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

test("resolveIdentifier posts plaintext input and normalizes response", async () => {
  let lastRequest = null;
  const signedReceipt = signedReceiptFixture();
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
        signature: signedReceipt.signature,
        signature_payload_hex: signedReceipt.signature_payload_hex,
        signature_payload: signedReceipt.signature_payload,
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
  assert.equal(result.signature, signedReceipt.signature);
  assert.equal(result.signature_payload.policy_id, POLICY_ID);
  assert.equal(
    verifyIdentifierResolutionReceipt(result, {
      policy_id: POLICY_ID,
      owner: ACCOUNT_ID,
      active: true,
      normalization: "phone_e164",
      resolver_public_key: signedReceipt.resolver_public_key,
      backend: "bfv-affine-sha3-256-v1",
    }),
    true,
  );
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
    }),
    {
      policyId: "string#retail",
      encryptedInput: BFV_ENCRYPTED_INPUT_HEX,
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
  const signedReceipt = signedReceiptFixture({
    signaturePayloadHex: "0A0B0C0D",
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
      });
      return jsonResponse(200, {
        policy_id: POLICY_ID,
        opaque_id: OPAQUE_ID,
        receipt_hash: RECEIPT_HASH,
        uaid: UAID,
        account_id: ACCOUNT_ID,
        resolved_at_ms: 7,
        backend: "bfv-affine-sha3-256-v1",
        signature: signedReceipt.signature,
        signature_payload_hex: signedReceipt.signature_payload_hex,
        signature_payload: {
          ...signedReceipt.signature_payload,
          resolved_at_ms: 7,
          expires_at_ms: null,
        },
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

test("buildIdentifierRequestForPolicy canonicalizes plaintext input", () => {
  const request = buildIdentifierRequestForPolicy(
    {
      policy_id: POLICY_ID,
      owner: ACCOUNT_ID,
      active: true,
      normalization: "phone_e164",
      resolver_public_key: "ed25519:ed0120" + "11".repeat(32),
      backend: "bfv-affine-sha3-256-v1",
      input_encryption: "bfv-v1",
    },
    { input: " +1 (555) 123-4567 " },
  );
  assert.deepEqual(request, {
    policyId: POLICY_ID,
    input: "+15551234567",
  });
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
