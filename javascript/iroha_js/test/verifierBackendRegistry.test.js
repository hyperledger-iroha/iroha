import { test } from "node:test";
import assert from "node:assert/strict";

import * as sourceRegistry from "../src/verifierBackendRegistry.js";
import * as distRegistry from "../dist/verifierBackendRegistry.js";
import { createVerifyingKeyClient as createSourceVerifyingKeyClient } from "../src/verifyingKeyClient.js";
import { createVerifyingKeyClient as createDistVerifyingKeyClient } from "../dist/verifyingKeyClient.js";

const EXPECTED_BINDINGS = Object.freeze([
  ["halo2/ipa", "halo2-ipa-pasta"],
  ["halo2/pasta/kaigi-roster-v1", "halo2-ipa-pasta"],
  ["halo2/pasta/kaigi-usage-v1", "halo2-ipa-pasta"],
  ["halo2/pasta/ivm-execution-v1", "halo2-ipa-pasta"],
  [
    "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
    "halo2-ipa-pasta",
  ],
  [
    "halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
    "halo2-ipa-pasta",
  ],
  [
    "halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3",
    "halo2-ipa-pasta",
  ],
  [
    "halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4",
    "halo2-ipa-pasta",
  ],
  ["stark/fri", "stark"],
  ["stark/fri/sha256-goldilocks", "stark"],
  ["stark/fri/poseidon2-goldilocks", "stark"],
  ["stark/fri/sha256_goldilocks.v1", "stark"],
]);

const SURFACES = Object.freeze([
  ["source", sourceRegistry],
  ["dist", distRegistry],
]);

test("exports the exact immutable engines, labels, and ordered bindings", () => {
  for (const [surface, registry] of SURFACES) {
    assert.deepEqual(
      registry.OPEN_VERIFY_BACKEND_TAGS_V1,
      ["halo2-ipa-pasta", "stark"],
      surface,
    );
    assert.deepEqual(
      registry.VERIFIER_BACKEND_REGISTRY_LABELS_V1,
      EXPECTED_BINDINGS.map(([label]) => label),
      surface,
    );
    assert.deepEqual(
      registry.VERIFIER_BACKEND_REGISTRY_BINDINGS_V1.map(({ label, engine }) => [
        label,
        engine,
      ]),
      EXPECTED_BINDINGS,
      surface,
    );
    assert.equal(
      new Set(registry.VERIFIER_BACKEND_REGISTRY_LABELS_V1).size,
      EXPECTED_BINDINGS.length,
      `${surface}: unique registry labels`,
    );
    assert.ok(Object.isFrozen(registry.OPEN_VERIFY_BACKEND_TAGS_V1), surface);
    assert.ok(
      Object.isFrozen(registry.VERIFIER_BACKEND_REGISTRY_LABELS_V1),
      surface,
    );
    assert.ok(
      Object.isFrozen(registry.VERIFIER_BACKEND_REGISTRY_BINDINGS_V1),
      surface,
    );
    for (const binding of registry.VERIFIER_BACKEND_REGISTRY_BINDINGS_V1) {
      assert.ok(Object.isFrozen(binding), `${surface}: ${binding.label}`);
    }
    assert.throws(
      () => registry.VERIFIER_BACKEND_REGISTRY_LABELS_V1.push("stark/fri/latest"),
      TypeError,
      surface,
    );
  }
});

test("binds every exact label to one generic engine and nothing else", () => {
  for (const [surface, registry] of SURFACES) {
    for (const [label, engine] of EXPECTED_BINDINGS) {
      assert.equal(registry.verifierBackendRegistryTagV1(label), engine, label);
      assert.equal(registry.isVerifierBackendRegistryLabelV1(label), true, label);
      assert.equal(
        registry.requireVerifierBackendRegistryLabelV1(label),
        label,
        label,
      );
    }

    for (const rejected of [
      null,
      undefined,
      0,
      {},
      "",
      "halo2-ipa-pasta",
      "stark",
      " halo2/ipa",
      "halo2/ipa ",
      "HALO2/IPA",
      "halo2//ipa",
      "halo2/ipa/",
      "halo2/ipa:ivm-execution-v1",
      "halo2/pasta/ivm_execution_v1",
      "halo2/pasta/kagemusha-recursive-spend-step-eq-two-parent-operation-protocol-v2",
      "stark/fri/latest",
      "stark/fri/sha256-goldilocks/extra",
      "stark/fri/sha256-goldilocks\u200B",
      "halo2\uFF0Fipa",
      "halo2/\u200Bipa",
      "h\u0430lo2/ipa",
      "groth16/bn254",
      "halo2/kzg",
      "zkat",
      "silent-threshold-anoncred",
      "sis-hints-anoncred-pq-v0",
      "sis-with-hints",
    ]) {
      assert.equal(
        registry.verifierBackendRegistryTagV1(rejected),
        null,
        `${surface}: ${String(rejected)}`,
      );
      assert.equal(
        registry.isVerifierBackendRegistryLabelV1(rejected),
        false,
        `${surface}: ${String(rejected)}`,
      );
      assert.throws(
        () => registry.requireVerifierBackendRegistryLabelV1(rejected, "vk.backend"),
        /vk\.backend uses unsupported verifier-registry label/u,
        `${surface}: ${String(rejected)}`,
      );
    }
  }
});

test("rejects structural mutations of every admitted registry label", () => {
  for (const [surface, registry] of SURFACES) {
    for (const [label] of EXPECTED_BINDINGS) {
      const last = label.at(-1);
      const replacement = last === "x" ? "y" : "x";
      for (const mutation of [
        ` ${label}`,
        `${label} `,
        label.toUpperCase(),
        `${label}/`,
        `${label}\u0000`,
        `${label}\u200B`,
        label.replace("/", "//"),
        `${label.slice(0, -1)}${replacement}`,
      ]) {
        assert.equal(
          registry.verifierBackendRegistryTagV1(mutation),
          null,
          `${surface}: ${mutation}`,
        );
      }
    }
  }
});

test("verifying-key helpers consume the shared closed registry", () => {
  const dependencyStubs = Array.from({ length: 16 }, () => () => {});
  for (const [surface, createClient] of [
    ["source", createSourceVerifyingKeyClient],
    ["dist", createDistVerifyingKeyClient],
  ]) {
    const client = createClient(...dependencyStubs);
    for (const [label] of EXPECTED_BINDINGS) {
      assert.equal(client.backend(label, `${surface}.backend`), label, label);
    }
    for (const rejected of [
      "halo2-ipa-pasta",
      "stark",
      " halo2/ipa",
      "stark/fri/latest",
    ]) {
      assert.throws(
        () => client.backend(rejected, `${surface}.backend`),
        /unsupported production verifier backend|surrounding whitespace/u,
        `${surface}: ${rejected}`,
      );
    }
  }
});
