import assert from "node:assert/strict";
import { createHash, generateKeyPairSync, sign as signRaw } from "node:crypto";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  AccountAddress,
  ToriiClient,
  buildIdentifierRequestForPolicy,
  encodeIdentifierResolutionReceiptAttestation,
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
  "4e52543000001042e5b988077612440e4cd45673596b00b004000000000000dd479e32bf99dbd000a804000000000000040000000000000020010000000000008800000000000000080000000000000008000000000000002dac6c00000000000800000000000000440e92000000000008000000000000005b2600000000000008000000000000004a681100000000000800000000000000bc3d2300000000000800000000000000413e85000000000008000000000000005619f900000000000800000000000000bd73fc0000000000880000000000000008000000000000000800000000000000ee894300000000000800000000000000dd22b000000000000800000000000000fe7c50000000000008000000000000001639a3000000000008000000000000006a969b00000000000800000000000000ddd4410000000000080000000000000051076600000000000800000000000000ef14ae00000000002001000000000000880000000000000008000000000000000800000000000000d86c690000000000080000000000000093070e0000000000080000000000000033067500000000000800000000000000ddc5190000000000080000000000000062ea230000000000080000000000000056f00a00000000000800000000000000ab51d400000000000800000000000000e945790000000000880000000000000008000000000000000800000000000000f2204400000000000800000000000000c9ecd2000000000008000000000000001dfc5b00000000000800000000000000d16d660000000000080000000000000016ec0e000000000008000000000000003def83000000000008000000000000006e7ff900000000000800000000000000c1fabb00000000002001000000000000880000000000000008000000000000000800000000000000c8c6eb00000000000800000000000000c9c14800000000000800000000000000f01f8700000000000800000000000000aed22c000000000008000000000000006122990000000000080000000000000036ad8c00000000000800000000000000d1429300000000000800000000000000891f6d0000000000880000000000000008000000000000000800000000000000417eed00000000000800000000000000d79c34000000000008000000000000009f322c0000000000080000000000000091fe5700000000000800000000000000533ce8000000000008000000000000005db8df00000000000800000000000000a8c313000000000008000000000000006e03c20000000000200100000000000088000000000000000800000000000000080000000000000003d654000000000008000000000000005d884400000000000800000000000000567ab50000000000080000000000000007273100000000000800000000000000ff6d0a00000000000800000000000000077466000000000008000000000000006d1d1a000000000008000000000000007050c200000000008800000000000000080000000000000008000000000000002f884f0000000000080000000000000041b0a100000000000800000000000000cbfa290000000000080000000000000057477300000000000800000000000000608f9200000000000800000000000000f5f5dd00000000000800000000000000445b3b00000000000800000000000000999e690000000000";
const BFV_VECTOR_FIXTURE = JSON.parse(
  readFileSync(
    new URL("../../../fixtures/soracloud/bfv_identifier_vectors_v1.json", import.meta.url),
    "utf8",
  ),
);
const IDENTIFIER_RECEIPT_VECTOR_FIXTURE = JSON.parse(
  readFileSync(
    new URL("../../../fixtures/soracloud/identifier_receipt_vectors_v1.json", import.meta.url),
    "utf8",
  ),
);
const BFV_COMPONENT_DIGEST_RE = /^[0-9A-F]{64}$/u;
const BFV_CHAIN_DIGEST_RE = /^[0-9a-f]{64}$/u;
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

function sha256HexFromHex(hex) {
  return createHash("sha256")
    .update(Buffer.from(hex, "hex"))
    .digest("hex")
    .toUpperCase();
}

function sha256Hex(bytes) {
  return createHash("sha256").update(bytes).digest("hex").toUpperCase();
}

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function assertBfvComponentDigest(label, value, seen) {
  assert.equal(typeof value, "string", `${label}: digest must be a string`);
  assert.match(value, BFV_COMPONENT_DIGEST_RE, `${label}: digest must be canonical uppercase SHA-256`);
  assert.notEqual(value, "0".repeat(64), `${label}: digest must not be zero`);
  assert.equal(seen.has(value), false, `${label}: component digest must be unique`);
  seen.add(value);
}

function assertBfvUpperSha256(label, value) {
  assert.equal(typeof value, "string", `${label}: digest must be a string`);
  assert.match(value, BFV_COMPONENT_DIGEST_RE, `${label}: digest must be canonical uppercase SHA-256`);
  assert.notEqual(value, "0".repeat(64), `${label}: digest must not be zero`);
}

function balancedBfvMultiplicationDepth(inputCount) {
  assert.equal(inputCount > 0, true, "BFV multiplication depth requires at least one input");
  let covered = 1;
  let depth = 0;
  while (covered < inputCount) {
    covered *= 2;
    depth += 1;
  }
  return depth;
}

function assertBfvLowerDigest(label, value) {
  assert.equal(typeof value, "string", `${label}: digest must be a string`);
  assert.match(value, BFV_CHAIN_DIGEST_RE, `${label}: digest must be canonical lowercase hex`);
  assert.notEqual(value, "0".repeat(64), `${label}: digest must not be zero`);
}

function assertBfvRnsPolynomialFixture(label, polynomial, publicDegree, limbCount) {
  assert.equal(polynomial.coefficient_count, publicDegree, `${label}: coefficient count`);
  assert.equal(polynomial.residue_limb_sha256.length, limbCount, `${label}: residue limb count`);
  assertBfvUpperSha256(`${label}: reconstructed coefficients`, polynomial.reconstructed_sha256);
  for (const [index, digest] of polynomial.residue_limb_sha256.entries()) {
    assertBfvUpperSha256(`${label}: residue limb ${index}`, digest);
  }
}

function assertBfvRnsModulusChainFixture(operationVectors, publicDegree) {
  const rns = operationVectors.rns_modulus_chain;
  assert.equal(Array.isArray(rns.moduli), true, "RNS moduli must be an array");
  assert.equal(rns.moduli.length > 0, true, "RNS moduli must not be empty");
  assert.deepEqual([...rns.moduli].sort((lhs, rhs) => lhs - rhs), rns.moduli, "RNS moduli must be sorted");
  for (const [index, modulus] of rns.moduli.entries()) {
    assert.equal(Number.isSafeInteger(modulus), true, `RNS modulus ${index} must be a safe integer`);
    assert.equal(modulus > 2 && modulus % 2 === 1, true, `RNS modulus ${index} must be an odd prime candidate`);
  }
  assert.match(rns.product, /^[0-9]+$/, "RNS product must be decimal");
  assertBfvLowerDigest("RNS chain digest", rns.expected_digest_hex);

  const samples = rns.sample_polynomials;
  assert.equal(samples.lhs_coefficients.length, publicDegree, "RNS lhs sample coefficient count");
  assert.equal(samples.rhs_coefficients.length, publicDegree, "RNS rhs sample coefficient count");
  for (const [label, coefficients] of [
    ["lhs", samples.lhs_coefficients],
    ["rhs", samples.rhs_coefficients],
  ]) {
    for (const [index, coefficient] of coefficients.entries()) {
      assert.equal(Number.isSafeInteger(coefficient), true, `RNS ${label}[${index}] must be a safe integer`);
      assert.equal(coefficient >= 0, true, `RNS ${label}[${index}] must be non-negative`);
    }
  }

  for (const label of ["lhs", "rhs", "sum", "negacyclic_product"]) {
    assertBfvRnsPolynomialFixture(label, samples[label], publicDegree, rns.moduli.length);
  }
}

function assertBfvOperationKeyComponentVectors(operationVectors) {
  assert.equal(operationVectors.vector_set, "soracloud-bfv-operation-v1");
  const publicDegree = operationVectors.public_parameters.polynomial_degree;
  assertBfvRnsModulusChainFixture(operationVectors, publicDegree);
  const evaluationKey = operationVectors.evaluation_key_bundle;
  assert.equal(evaluationKey.decomposition_base_log, operationVectors.public_parameters.decomposition_base_log);
  assert.equal(evaluationKey.decomposition_digit_count, evaluationKey.relinearization_entry_count);
  assert.equal(evaluationKey.relinearization_entries.length, evaluationKey.relinearization_entry_count);
  const componentDigests = new Set();
  for (const [index, entry] of evaluationKey.relinearization_entries.entries()) {
    assert.equal(entry.index, index, `relinearization entry ${index}: index`);
    assert.equal(entry.coefficient_count, publicDegree, `relinearization entry ${index}: coefficient count`);
    assertBfvComponentDigest(`relinearization entry ${index} b`, entry.b_sha256, componentDigests);
    assertBfvComponentDigest(`relinearization entry ${index} a`, entry.a_sha256, componentDigests);
  }
  assert.equal(operationVectors.galois_keys.length, evaluationKey.galois_key_count);
  for (const key of operationVectors.galois_keys) {
    assert.equal(key.entries.length, key.entry_count, `Galois key ${key.automorphism_power}: entry count`);
    for (const [index, entry] of key.entries.entries()) {
      assert.equal(entry.index, index, `Galois key ${key.automorphism_power} entry ${index}: index`);
      assert.equal(entry.coefficient_count, publicDegree, `Galois key ${key.automorphism_power} entry ${index}: coefficient count`);
      assertBfvComponentDigest(`Galois key ${key.automorphism_power} entry ${index} b`, entry.b_sha256, componentDigests);
      assertBfvComponentDigest(`Galois key ${key.automorphism_power} entry ${index} a`, entry.a_sha256, componentDigests);
    }
  }
  assert.equal(operationVectors.galois_switch_vectors.length > 0, true, "Galois switch vectors must not be empty");
  for (const vector of operationVectors.galois_switch_vectors) {
    assert.equal(
      operationVectors.galois_keys.some((key) => key.automorphism_power === vector.automorphism_power),
      true,
      `Galois switch vector ${vector.name}: matching key`,
    );
    assert.equal(vector.input_plaintext_slots.length > 0, true, `Galois switch vector ${vector.name}: plaintext slots`);
    for (const [index, slot] of vector.input_plaintext_slots.entries()) {
      assert.equal(Number.isSafeInteger(slot), true, `Galois switch vector ${vector.name}: slot ${index}`);
      assert.equal(slot >= 0, true, `Galois switch vector ${vector.name}: slot ${index} non-negative`);
    }
    assert.equal(vector.expected_input_ciphertext_bytes > 0, true, `Galois switch vector ${vector.name}: input bytes`);
    assert.equal(vector.expected_output_ciphertext_bytes > 0, true, `Galois switch vector ${vector.name}: output bytes`);
    assertBfvUpperSha256(`Galois switch vector ${vector.name}: input`, vector.expected_input_ciphertext_sha256);
    assertBfvUpperSha256(`Galois switch vector ${vector.name}: output`, vector.expected_output_ciphertext_sha256);
    assertBfvUpperSha256(`Galois switch vector ${vector.name}: plaintext`, vector.expected_plaintext_sha256);
    assert.equal(vector.output_components.coefficient_count, publicDegree, `Galois switch vector ${vector.name}: coefficient count`);
    assertBfvComponentDigest(`Galois switch vector ${vector.name} c0`, vector.output_components.c0_sha256, componentDigests);
    assertBfvComponentDigest(`Galois switch vector ${vector.name} c1`, vector.output_components.c1_sha256, componentDigests);
  }
  assert.equal(operationVectors.packed_galois_switch_vectors.length > 0, true, "packed Galois switch vectors must not be empty");
  for (const vector of operationVectors.packed_galois_switch_vectors) {
    assert.equal(
      operationVectors.galois_keys.some((key) => key.automorphism_power === vector.automorphism_power),
      true,
      `packed Galois switch vector ${vector.name}: matching key`,
    );
    assert.equal(vector.input_packed_slots.length, publicDegree, `packed Galois switch vector ${vector.name}: input slot count`);
    assert.equal(vector.expected_slot_permutation.length, publicDegree, `packed Galois switch vector ${vector.name}: permutation count`);
    assert.equal(vector.expected_packed_slots.length, publicDegree, `packed Galois switch vector ${vector.name}: output slot count`);
    for (const [label, values] of [
      ["input", vector.input_packed_slots],
      ["permutation", vector.expected_slot_permutation],
      ["output", vector.expected_packed_slots],
    ]) {
      for (const [index, value] of values.entries()) {
        assert.equal(Number.isSafeInteger(value), true, `packed Galois switch vector ${vector.name}: ${label} ${index}`);
        assert.equal(value >= 0, true, `packed Galois switch vector ${vector.name}: ${label} ${index} non-negative`);
      }
    }
    assertBfvUpperSha256(`packed Galois switch vector ${vector.name}: packed plaintext`, vector.expected_packed_plaintext_sha256);
    assertBfvUpperSha256(`packed Galois switch vector ${vector.name}: input`, vector.expected_input_ciphertext_sha256);
    assertBfvUpperSha256(`packed Galois switch vector ${vector.name}: output`, vector.expected_output_ciphertext_sha256);
    assertBfvUpperSha256(`packed Galois switch vector ${vector.name}: plaintext`, vector.expected_plaintext_coefficients_sha256);
    assert.equal(vector.output_components.coefficient_count, publicDegree, `packed Galois switch vector ${vector.name}: coefficient count`);
    assertBfvComponentDigest(`packed Galois switch vector ${vector.name} c0`, vector.output_components.c0_sha256, componentDigests);
    assertBfvComponentDigest(`packed Galois switch vector ${vector.name} c1`, vector.output_components.c1_sha256, componentDigests);
  }
  assert.equal(operationVectors.rotation_keys.length, evaluationKey.rotation_key_count);
  for (const key of operationVectors.rotation_keys) {
    const components = key.zero_refresh_components;
    assert.equal(components.coefficient_count, publicDegree, `rotation key ${key.rotation_steps}: coefficient count`);
    assertBfvComponentDigest(`rotation key ${key.rotation_steps} c0`, components.c0_sha256, componentDigests);
    assertBfvComponentDigest(`rotation key ${key.rotation_steps} c1`, components.c1_sha256, componentDigests);
  }
  const bootstrap = operationVectors.bootstrap_key;
  assert.equal(bootstrap.key_id, evaluationKey.bootstrap_key_id);
  assert.equal(bootstrap.max_refresh_rounds, evaluationKey.bootstrap_max_refresh_rounds);
  assert.equal(Number.isSafeInteger(bootstrap.max_refresh_rounds), true, "bootstrap max refresh rounds");
  assert.equal(bootstrap.max_refresh_rounds > 0, true, "bootstrap max refresh rounds positive");
  assert.equal(bootstrap.zero_refresh_components.coefficient_count, publicDegree);
  assertBfvComponentDigest("bootstrap key c0", bootstrap.zero_refresh_components.c0_sha256, componentDigests);
  assertBfvComponentDigest("bootstrap key c1", bootstrap.zero_refresh_components.c1_sha256, componentDigests);
  assert.equal(Array.isArray(bootstrap.round_refreshes), true, "bootstrap round refresh list");
  assert.equal(bootstrap.round_refreshes.length, bootstrap.max_refresh_rounds, "bootstrap round refresh count");
  for (const [index, refresh] of bootstrap.round_refreshes.entries()) {
    assert.equal(refresh.round_index, index, `bootstrap round ${index}: index`);
    assert.equal(refresh.expected_refresh_bytes > 0, true, `bootstrap round ${index}: bytes`);
    assertBfvUpperSha256(`bootstrap round ${index}: refresh`, refresh.expected_refresh_sha256);
    assert.equal(refresh.components.coefficient_count, publicDegree, `bootstrap round ${index}: coefficient count`);
    if (index === 0) {
      assert.equal(refresh.components.c0_sha256, bootstrap.zero_refresh_components.c0_sha256, "bootstrap round 0 c0 mirrors zero_refresh");
      assert.equal(refresh.components.c1_sha256, bootstrap.zero_refresh_components.c1_sha256, "bootstrap round 0 c1 mirrors zero_refresh");
      assertBfvUpperSha256("bootstrap round 0 c0", refresh.components.c0_sha256);
      assertBfvUpperSha256("bootstrap round 0 c1", refresh.components.c1_sha256);
    } else {
      assertBfvComponentDigest(`bootstrap round ${index} c0`, refresh.components.c0_sha256, componentDigests);
      assertBfvComponentDigest(`bootstrap round ${index} c1`, refresh.components.c1_sha256, componentDigests);
    }
  }
  assert.equal(
    bootstrap.round_refreshes[0].expected_refresh_sha256,
    bootstrap.expected_zero_refresh_sha256,
    "bootstrap first round mirrors zero_refresh",
  );
  assert.notEqual(
    bootstrap.round_refreshes[0].expected_refresh_sha256,
    bootstrap.round_refreshes[1]?.expected_refresh_sha256,
    "bootstrap round refresh material must be domain separated",
  );
  assert.equal(operationVectors.bootstrap_refresh_vectors.length > 0, true, "bootstrap refresh vectors must not be empty");
  for (const vector of operationVectors.bootstrap_refresh_vectors) {
    assert.equal(vector.key_id, bootstrap.key_id, `bootstrap refresh vector ${vector.name}: key id`);
    assert.equal(Number.isSafeInteger(vector.refresh_rounds), true, `bootstrap refresh vector ${vector.name}: refresh rounds`);
    assert.equal(vector.refresh_rounds > 0, true, `bootstrap refresh vector ${vector.name}: refresh rounds positive`);
    assert.equal(
      vector.refresh_rounds <= bootstrap.max_refresh_rounds,
      true,
      `bootstrap refresh vector ${vector.name}: refresh rounds within key bound`,
    );
    assert.equal(vector.input_plaintext_slots.length > 0, true, `bootstrap refresh vector ${vector.name}: plaintext slots`);
    for (const [index, slot] of vector.input_plaintext_slots.entries()) {
      assert.equal(Number.isSafeInteger(slot), true, `bootstrap refresh vector ${vector.name}: slot ${index}`);
      assert.equal(slot >= 0, true, `bootstrap refresh vector ${vector.name}: slot ${index} non-negative`);
    }
    assert.equal(vector.expected_input_ciphertext_bytes > 0, true, `bootstrap refresh vector ${vector.name}: input bytes`);
    assert.equal(vector.expected_output_ciphertext_bytes > 0, true, `bootstrap refresh vector ${vector.name}: output bytes`);
    assertBfvUpperSha256(`bootstrap refresh vector ${vector.name}: input`, vector.expected_input_ciphertext_sha256);
    assertBfvUpperSha256(`bootstrap refresh vector ${vector.name}: output`, vector.expected_output_ciphertext_sha256);
    assertBfvUpperSha256(`bootstrap refresh vector ${vector.name}: plaintext`, vector.expected_plaintext_sha256);
    assert.equal(vector.output_components.coefficient_count, publicDegree, `bootstrap refresh vector ${vector.name}: coefficient count`);
    assertBfvComponentDigest(`bootstrap refresh vector ${vector.name} c0`, vector.output_components.c0_sha256, componentDigests);
    assertBfvComponentDigest(`bootstrap refresh vector ${vector.name} c1`, vector.output_components.c1_sha256, componentDigests);
  }
  for (const vector of operationVectors.vectors) {
    const expectedDepth =
      vector.operation === "Multiply" ? balancedBfvMultiplicationDepth(vector.inputs.length) : 0;
    assert.equal(
      vector.requested_multiplication_depth,
      expectedDepth,
      `${vector.name}: requested multiplication depth`,
    );
  }
  const packedRotate = operationVectors.vectors.find((vector) => vector.name === "soracloud-packed-rotate-left-output");
  assert.notEqual(packedRotate, undefined, "packed RotateLeft operation vector must be present");
  assert.equal(packedRotate.operation, "RotateLeft");
  assert.equal(packedRotate.rotation_steps, publicDegree / 2, "packed RotateLeft half rotation");
  assert.equal(packedRotate.automorphism_power, publicDegree + 1, "packed RotateLeft Galois power");
  assert.equal(
    operationVectors.galois_keys.some((key) => key.automorphism_power === packedRotate.automorphism_power),
    true,
    "packed RotateLeft vector must have matching Galois key",
  );
  assert.equal(packedRotate.inputs.length, 1, "packed RotateLeft input count");
  const packedRotateInput = packedRotate.inputs[0];
  assert.equal(packedRotateInput.packed_slots.length, publicDegree, "packed RotateLeft input slot count");
  assert.equal(packedRotate.expected_packed_slots.length, publicDegree, "packed RotateLeft output slot count");
  for (const [label, values] of [
    ["input", packedRotateInput.packed_slots],
    ["output", packedRotate.expected_packed_slots],
  ]) {
    for (const [index, value] of values.entries()) {
      assert.equal(Number.isSafeInteger(value), true, `packed RotateLeft ${label} ${index}`);
      assert.equal(value >= 0, true, `packed RotateLeft ${label} ${index} non-negative`);
    }
  }
  assert.equal(packedRotateInput.expected_ciphertext_bytes > 0, true, "packed RotateLeft input bytes");
  assert.equal(packedRotate.expected_output_ciphertext_bytes > 0, true, "packed RotateLeft output bytes");
  assertBfvUpperSha256("packed RotateLeft input plaintext", packedRotateInput.expected_packed_plaintext_sha256);
  assertBfvUpperSha256("packed RotateLeft input", packedRotateInput.expected_ciphertext_sha256);
  assertBfvUpperSha256("packed RotateLeft output", packedRotate.expected_output_ciphertext_sha256);
  assertBfvUpperSha256("packed RotateLeft plaintext", packedRotate.expected_plaintext_coefficients_sha256);
  assert.equal(packedRotate.output_components.coefficient_count, publicDegree, "packed RotateLeft coefficient count");
  assertBfvComponentDigest("packed RotateLeft c0", packedRotate.output_components.c0_sha256, componentDigests);
  assertBfvComponentDigest("packed RotateLeft c1", packedRotate.output_components.c1_sha256, componentDigests);

  const packedRotateSchedule = operationVectors.vectors.find(
    (vector) => vector.name === "soracloud-packed-rotate-left-schedule-output",
  );
  assert.notEqual(packedRotateSchedule, undefined, "packed RotateLeft schedule vector must be present");
  assert.equal(packedRotateSchedule.operation, "RotateLeft");
  assert.equal(packedRotateSchedule.rotation_steps, 1, "packed RotateLeft schedule rotation");
  assert.equal(Array.isArray(packedRotateSchedule.automorphism_powers), true, "packed RotateLeft schedule powers");
  assert.equal(packedRotateSchedule.automorphism_powers.length > 1, true, "packed RotateLeft schedule must use multiple powers");
  for (const [index, power] of packedRotateSchedule.automorphism_powers.entries()) {
    assert.equal(Number.isSafeInteger(power), true, `packed RotateLeft schedule power ${index}`);
    assert.equal(power > 0, true, `packed RotateLeft schedule power ${index} positive`);
    assert.equal(
      operationVectors.galois_keys.some((key) => key.automorphism_power === power),
      true,
      `packed RotateLeft schedule power ${power} must have matching Galois key`,
    );
  }
  assert.equal(packedRotateSchedule.inputs.length, 1, "packed RotateLeft schedule input count");
  const scheduleInput = packedRotateSchedule.inputs[0];
  assert.equal(scheduleInput.packed_slots.length, publicDegree, "packed RotateLeft schedule input slot count");
  assert.equal(packedRotateSchedule.expected_packed_slots.length, publicDegree, "packed RotateLeft schedule output slot count");
  const expectedScheduleSlots = scheduleInput.packed_slots.slice(1).concat(scheduleInput.packed_slots[0]);
  assert.deepEqual(
    packedRotateSchedule.expected_packed_slots,
    expectedScheduleSlots,
    "packed RotateLeft schedule output slots",
  );
  for (const [label, values] of [
    ["input", scheduleInput.packed_slots],
    ["output", packedRotateSchedule.expected_packed_slots],
  ]) {
    for (const [index, value] of values.entries()) {
      assert.equal(Number.isSafeInteger(value), true, `packed RotateLeft schedule ${label} ${index}`);
      assert.equal(value >= 0, true, `packed RotateLeft schedule ${label} ${index} non-negative`);
    }
  }
  assert.equal(scheduleInput.expected_ciphertext_bytes > 0, true, "packed RotateLeft schedule input bytes");
  assert.equal(packedRotateSchedule.expected_output_ciphertext_bytes > 0, true, "packed RotateLeft schedule output bytes");
  assertBfvUpperSha256("packed RotateLeft schedule input plaintext", scheduleInput.expected_packed_plaintext_sha256);
  assertBfvUpperSha256("packed RotateLeft schedule input", scheduleInput.expected_ciphertext_sha256);
  assertBfvUpperSha256("packed RotateLeft schedule output", packedRotateSchedule.expected_output_ciphertext_sha256);
  assertBfvUpperSha256("packed RotateLeft schedule plaintext", packedRotateSchedule.expected_plaintext_coefficients_sha256);
  assert.equal(
    packedRotateSchedule.output_components.coefficient_count,
    publicDegree,
    "packed RotateLeft schedule coefficient count",
  );
  assertBfvComponentDigest("packed RotateLeft schedule c0", packedRotateSchedule.output_components.c0_sha256, componentDigests);
  assertBfvComponentDigest("packed RotateLeft schedule c1", packedRotateSchedule.output_components.c1_sha256, componentDigests);
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

test("verifyIdentifierResolutionReceipt rejects adversarial receipt mutations", () => {
  const signedReceipt = signedReceiptFixture();
  const policy = {
    policy_id: POLICY_ID,
    owner: ACCOUNT_ID,
    active: true,
    normalization: "phone_e164",
    resolver_public_key: signedReceipt.resolver_public_key,
    backend: "bfv-programmed-sha3-256-v1",
  };

  assert.equal(verifyIdentifierResolutionReceipt(signedReceipt, policy), true);

  const tampered = JSON.parse(JSON.stringify(signedReceipt));
  tampered.payload.execution.output_ciphertext_hash = "67".repeat(32);
  assert.equal(verifyIdentifierResolutionReceipt(tampered, policy), false);

  assert.equal(
    verifyIdentifierResolutionReceipt(signedReceipt, {
      ...policy,
      resolver_public_key: "ed25519:ed0120" + "45".repeat(32),
    }),
    false,
  );

  const malformedSignature = JSON.parse(JSON.stringify(signedReceipt));
  malformedSignature.attestation.signature = "GG";
  assert.throws(
    () => verifyIdentifierResolutionReceipt(malformedSignature, policy),
    /attestation\.signature/,
  );

  const signedWithProofFields = JSON.parse(JSON.stringify(signedReceipt));
  signedWithProofFields.attestation.proof_backend = "halo2/ipa";
  signedWithProofFields.attestation.proof_b64 = "AQID";
  assert.throws(
    () => verifyIdentifierResolutionReceipt(signedWithProofFields, policy),
    /signed attestation must not include proof fields/,
  );

  const proofAttestation = {
    payload: signedReceipt.payload,
    attestation: {
      kind: "proof",
      proof_backend: "halo2/ipa",
      proof_b64: "AQID",
    },
  };
  assert.throws(
    () => verifyIdentifierResolutionReceipt(proofAttestation, policy),
    /proof attestations require an external verifier/,
  );

  assert.throws(
    () =>
      verifyIdentifierResolutionReceipt(signedReceipt, {
        ...policy,
        policy_id: "email#retail",
      }),
    /does not match policy/,
  );
});

test("verifyIdentifierResolutionReceipt matches shared receipt vectors", () => {
  assert.equal(
    IDENTIFIER_RECEIPT_VECTOR_FIXTURE.vector_set,
    "identifier-receipt-attestation-v1",
  );
  const payloadBytes = Buffer.from(
    encodeIdentifierResolutionReceiptPayload(IDENTIFIER_RECEIPT_VECTOR_FIXTURE.receipt.payload),
  );
  assert.equal(
    sha256Hex(payloadBytes),
    IDENTIFIER_RECEIPT_VECTOR_FIXTURE.canonical_payload_sha256,
  );
  assert.equal(
    verifyIdentifierResolutionReceipt(
      IDENTIFIER_RECEIPT_VECTOR_FIXTURE.receipt,
      IDENTIFIER_RECEIPT_VECTOR_FIXTURE.policy,
    ),
    true,
  );

  for (const vector of IDENTIFIER_RECEIPT_VECTOR_FIXTURE.attestation_vectors) {
    const encoded = Buffer.from(
      encodeIdentifierResolutionReceiptAttestation(vector.attestation),
    );
    assert.equal(
      encoded.length,
      vector.expected_attestation_bytes,
      `${vector.name}: attestation byte length`,
    );
    assert.equal(
      sha256Hex(encoded),
      vector.expected_attestation_sha256,
      `${vector.name}: attestation digest`,
    );
    if (vector.attestation.kind === "proof") {
      assert.throws(
        () =>
          verifyIdentifierResolutionReceipt(
            {
              payload: IDENTIFIER_RECEIPT_VECTOR_FIXTURE.receipt.payload,
              attestation: vector.attestation,
            },
            IDENTIFIER_RECEIPT_VECTOR_FIXTURE.policy,
          ),
        /proof attestations require an external verifier/,
        `${vector.name}: proof verifier gate`,
      );
    }
  }

  for (const negative of IDENTIFIER_RECEIPT_VECTOR_FIXTURE.negative_cases) {
    const receipt = JSON.parse(JSON.stringify(IDENTIFIER_RECEIPT_VECTOR_FIXTURE.receipt));
    const policy = JSON.parse(JSON.stringify(IDENTIFIER_RECEIPT_VECTOR_FIXTURE.policy));
    switch (negative.mutation) {
      case "receipt.payload.execution.output_ciphertext_hash":
        receipt.payload.execution.output_ciphertext_hash = negative.value;
        break;
      case "policy.resolver_public_key":
        policy.resolver_public_key = negative.value;
        break;
      case "policy.policy_id":
        policy.policy_id = negative.value;
        break;
      case "receipt.attestation.signature":
        receipt.attestation.signature = negative.value;
        break;
      case "receipt.attestation":
        receipt.attestation = negative.value;
        break;
      default:
        throw new Error(`unhandled receipt vector mutation ${negative.mutation}`);
    }

    if (negative.expected_error_contains) {
      assert.throws(
        () => verifyIdentifierResolutionReceipt(receipt, policy),
        new RegExp(negative.expected_error_contains, "i"),
        negative.name,
      );
    } else {
      assert.equal(
        verifyIdentifierResolutionReceipt(receipt, policy),
        negative.expected_result,
        negative.name,
      );
    }
  }
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

test("encryptIdentifierInputForPolicy matches shared Soracloud BFV vectors", () => {
  assert.equal(BFV_VECTOR_FIXTURE.vector_set, "soracloud-bfv-identifier-envelope-v1");
  const observedDigests = new Set();

  for (const vector of BFV_VECTOR_FIXTURE.vectors) {
    const ciphertextHex = encryptIdentifierInputForPolicy(
      BFV_VECTOR_FIXTURE.policy,
      vector.input_utf8,
      { seedHex: vector.seed_hex },
    );
    assert.equal(
      Buffer.from(ciphertextHex, "hex").length,
      vector.expected_ciphertext_bytes,
      `${vector.name}: ciphertext byte length`,
    );
    assert.equal(
      sha256HexFromHex(ciphertextHex),
      vector.expected_ciphertext_sha256,
      `${vector.name}: ciphertext digest`,
    );
    observedDigests.add(vector.expected_ciphertext_sha256);
  }

  assert.equal(
    observedDigests.size,
    BFV_VECTOR_FIXTURE.vectors.length,
    "fixture vectors must not alias to the same encrypted payload digest",
  );
});

test("encryptIdentifierInputForPolicy matches shared Soracloud BFV operation input vectors", () => {
  const operationVectors = BFV_VECTOR_FIXTURE.operation_vectors;
  assert.equal(operationVectors.vector_set, "soracloud-bfv-operation-v1");
  assertBfvOperationKeyComponentVectors(operationVectors);
  const policy = {
    policy_id: "soracloud-operation#fixture",
    owner: ACCOUNT_ID,
    active: true,
    normalization: "exact",
    resolver_public_key: "ed25519:ed0120" + "11".repeat(32),
    backend: "bfv-programmed-sha3-256-v1",
    input_encryption: "bfv-v1",
    input_encryption_public_parameters_decoded:
      operationVectors.public_parameters_decoded,
  };
  const observedDigests = new Set();
  let inputCount = 0;

  for (const vector of operationVectors.vectors) {
    for (const input of vector.inputs) {
      inputCount += 1;
      if (input.packed_slots !== undefined) {
        assert.equal(input.packed_slots.length, operationVectors.public_parameters.polynomial_degree);
        assertBfvUpperSha256(`${vector.name}/${input.seed_utf8}: packed plaintext`, input.expected_packed_plaintext_sha256);
        assert.equal(input.expected_ciphertext_bytes > 0, true);
        assertBfvUpperSha256(`${vector.name}/${input.seed_utf8}: ciphertext`, input.expected_ciphertext_sha256);
        observedDigests.add(input.expected_ciphertext_sha256);
        continue;
      }
      const ciphertextHex = encryptIdentifierInputForPolicy(
        policy,
        Buffer.from(input.input_hex, "hex"),
        { seed: Buffer.from(input.seed_utf8, "utf8") },
      );
      assert.equal(
        Buffer.from(ciphertextHex, "hex").length,
        input.expected_ciphertext_bytes,
        `${vector.name}/${input.seed_utf8}: ciphertext byte length`,
      );
      assert.equal(
        sha256HexFromHex(ciphertextHex),
        input.expected_ciphertext_sha256,
        `${vector.name}/${input.seed_utf8}: ciphertext digest`,
      );
      observedDigests.add(input.expected_ciphertext_sha256);
    }
  }

  assert.equal(inputCount, 10, "fixture should cover all Add/Multiply/Rotate/Bootstrap inputs");
  assert.equal(
    observedDigests.size,
    inputCount,
    "operation input fixture vectors must not alias to the same encrypted payload digest",
  );
});

test("shared Soracloud BFV key-bundle component vectors reject adversarial drift", () => {
  assert.doesNotThrow(() => assertBfvOperationKeyComponentVectors(BFV_VECTOR_FIXTURE.operation_vectors));
  for (const [name, mutate] of [
    [
      "missing relinearization component",
      (operationVectors) => {
        delete operationVectors.evaluation_key_bundle.relinearization_entries[0].b_sha256;
      },
    ],
    [
      "duplicate component digest",
      (operationVectors) => {
        operationVectors.evaluation_key_bundle.relinearization_entries[1].a_sha256 =
          operationVectors.evaluation_key_bundle.relinearization_entries[0].b_sha256;
      },
    ],
    [
      "noncanonical lowercase component digest",
      (operationVectors) => {
        operationVectors.evaluation_key_bundle.relinearization_entries[0].b_sha256 =
          operationVectors.evaluation_key_bundle.relinearization_entries[0].b_sha256.toLowerCase();
      },
    ],
    [
      "missing Galois component",
      (operationVectors) => {
        delete operationVectors.galois_keys[0].entries[0].a_sha256;
      },
    ],
    [
      "zero refresh component digest",
      (operationVectors) => {
        operationVectors.rotation_keys[0].zero_refresh_components.c1_sha256 = "0".repeat(64);
      },
    ],
    [
      "component coefficient-count drift",
      (operationVectors) => {
        operationVectors.bootstrap_key.zero_refresh_components.coefficient_count = 63;
      },
    ],
    [
      "bootstrap refresh bound drift",
      (operationVectors) => {
        operationVectors.bootstrap_key.max_refresh_rounds = 1;
      },
    ],
    [
      "bootstrap refresh component drift",
      (operationVectors) => {
        operationVectors.bootstrap_refresh_vectors[0].output_components.c0_sha256 = "0".repeat(64);
      },
    ],
    [
      "rotation key count drift",
      (operationVectors) => {
        operationVectors.evaluation_key_bundle.rotation_key_count += 1;
      },
    ],
    [
      "uppercase RNS chain digest",
      (operationVectors) => {
        operationVectors.rns_modulus_chain.expected_digest_hex =
          operationVectors.rns_modulus_chain.expected_digest_hex.toUpperCase();
      },
    ],
    [
      "RNS sample coefficient-count drift",
      (operationVectors) => {
        operationVectors.rns_modulus_chain.sample_polynomials.lhs_coefficients.pop();
      },
    ],
    [
      "RNS residue limb-count drift",
      (operationVectors) => {
        operationVectors.rns_modulus_chain.sample_polynomials.negacyclic_product.residue_limb_sha256.pop();
      },
    ],
  ]) {
    const operationVectors = clone(BFV_VECTOR_FIXTURE.operation_vectors);
    mutate(operationVectors);
    assert.throws(
      () => assertBfvOperationKeyComponentVectors(operationVectors),
      assert.AssertionError,
      name,
    );
  }
});

test("encryptIdentifierInputForPolicy rejects adversarial BFV operation vector inputs", () => {
  const operationVectors = BFV_VECTOR_FIXTURE.operation_vectors;
  const basePolicy = {
    policy_id: "soracloud-operation#fixture",
    owner: ACCOUNT_ID,
    active: true,
    normalization: "exact",
    resolver_public_key: "ed25519:ed0120" + "11".repeat(32),
    backend: "bfv-programmed-sha3-256-v1",
    input_encryption: "bfv-v1",
    input_encryption_public_parameters_decoded:
      operationVectors.public_parameters_decoded,
  };
  const [input] = operationVectors.vectors[0].inputs;
  const inputBytes = Buffer.from(input.input_hex, "hex");
  const seed = Buffer.from(input.seed_utf8, "utf8");

  assert.throws(
    () =>
      encryptIdentifierInputForPolicy(
        { ...basePolicy, normalization: "lowercase_trimmed" },
        inputBytes,
        { seed },
      ),
    ValidationError,
    "raw byte operation inputs must not bypass non-exact normalization",
  );
  assert.throws(
    () => encryptIdentifierInputForPolicy(basePolicy, Buffer.alloc(0), { seed }),
    ValidationError,
    "empty raw byte operation inputs must be rejected",
  );

  const unsupportedEncoding = JSON.parse(
    JSON.stringify(operationVectors.public_parameters_decoded),
  );
  unsupportedEncoding.norito_length_encoding = "compact-v9";
  assert.throws(
    () =>
      encryptIdentifierInputForPolicy(
        {
          ...basePolicy,
          input_encryption_public_parameters_decoded: unsupportedEncoding,
        },
        inputBytes,
        { seed },
      ),
    ValidationError,
    "unknown BFV Norito length encodings must be rejected",
  );

  const unsafeModulus = JSON.parse(
    JSON.stringify(operationVectors.public_parameters_decoded),
  );
  unsafeModulus.parameters.ciphertext_modulus = Number(
    unsafeModulus.parameters.ciphertext_modulus,
  );
  assert.throws(
    () =>
      encryptIdentifierInputForPolicy(
        {
          ...basePolicy,
          input_encryption_public_parameters_decoded: unsafeModulus,
        },
        inputBytes,
        { seed },
      ),
    ValidationError,
    "unsafe numeric ciphertext moduli must be rejected instead of rounded",
  );

  const unsafePublicKey = JSON.parse(
    JSON.stringify(operationVectors.public_parameters_decoded),
  );
  unsafePublicKey.public_key.b[2] = Number(unsafePublicKey.public_key.b[2]);
  assert.throws(
    () =>
      encryptIdentifierInputForPolicy(
        {
          ...basePolicy,
          input_encryption_public_parameters_decoded: unsafePublicKey,
        },
        inputBytes,
        { seed },
      ),
    ValidationError,
    "unsafe numeric public-key coefficients must be rejected instead of rounded",
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
      name: "max input byte count exceeds registered RAM-LFE profile",
      expected: ValidationError,
      params: (() => {
        const params = cloneParameters();
        params.max_input_bytes = 64;
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
