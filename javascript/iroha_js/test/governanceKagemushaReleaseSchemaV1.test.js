// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { validateKagemushaReleaseSchemaV1 } from "../src/governanceKagemushaReleaseSchemaV1.js";
import { KAGEMUSHA_RELEASE_SCHEMAS_V1 } from "../src/governanceKagemushaReleaseSchemasV1.js";

const OPENAPI_PATH = new URL("../../../artifacts/openapi/torii.json", import.meta.url);
const INSTALL_PATH = new URL(
  "../../../fixtures/governance/kagemusha_verifier_release_install_v1.json", import.meta.url,
);
const ACTIVATE_PATH = new URL(
  "../../../fixtures/governance/kagemusha_verifier_release_activate_v1.json", import.meta.url,
);

const ROOTS = Object.freeze({
  expected_predecessor: "GovernanceKagemushaGovernedVerifierRegistryV1",
  manifest: "GovernanceKagemushaReleaseManifestV1",
  receipt: "GovernanceKagemushaInternalValidationReceiptV1",
  attestation: "GovernanceKagemushaReleaseAttestationV1",
});

function fixture(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

test("KAGEMUSHA release schema closure tracks the canonical Torii OpenAPI artifact", () => {
  const schemas = fixture(OPENAPI_PATH).components.schemas;
  const closure = new Set(Object.values(ROOTS));
  const visit = (node) => {
    if (Array.isArray(node)) {
      node.forEach(visit);
    } else if (node !== null && typeof node === "object") {
      const ref = node.$ref;
      if (typeof ref === "string" && ref.startsWith("#/components/schemas/")) {
        const name = ref.slice("#/components/schemas/".length);
        if (!closure.has(name)) {
          closure.add(name);
          visit(schemas[name]);
        }
      }
      Object.values(node).forEach(visit);
    }
  };
  for (const root of Object.values(ROOTS)) visit(schemas[root]);
  const project = (node) => {
    if (Array.isArray(node)) return node.map(project);
    if (node !== null && typeof node === "object") {
      return Object.fromEntries(Object.entries(node)
        .filter(([key]) => !["description", "title", "example"].includes(key))
        .map(([key, value]) => [key, project(value)]));
    }
    return node;
  };
  assert.deepEqual(Object.keys(KAGEMUSHA_RELEASE_SCHEMAS_V1).sort(), [...closure].sort());
  for (const name of closure) {
    assert.deepEqual(KAGEMUSHA_RELEASE_SCHEMAS_V1[name], project(schemas[name]), name);
  }
  assert.equal(Object.isFrozen(KAGEMUSHA_RELEASE_SCHEMAS_V1.GovernanceKagemushaReleaseManifestV1.properties), true);
});

test("KAGEMUSHA release install fixture admits every nested closed root", () => {
  const { payload } = fixture(INSTALL_PATH);
  for (const [field, schema] of Object.entries(ROOTS)) {
    assert.doesNotThrow(() => validateKagemushaReleaseSchemaV1(schema, payload[field]));
  }
  const malformed = structuredClone(payload);
  malformed.manifest.enabled_profiles[0].hardware_profile.unexpected = true;
  assert.throws(
    () => validateKagemushaReleaseSchemaV1(ROOTS.manifest, malformed.manifest),
    /unknown field/u,
  );
  const outOfRange = structuredClone(payload);
  outOfRange.receipt.fuzz_cases = 9_007_199_254_740_992;
  assert.throws(
    () => validateKagemushaReleaseSchemaV1(ROOTS.receipt, outOfRange.receipt),
    /integer/u,
  );
  const sparse = structuredClone(payload.manifest.release_id);
  delete sparse[0];
  assert.throws(
    () => validateKagemushaReleaseSchemaV1("GovernanceKagemushaBytes32V1", sparse),
    /missing an array item/u,
  );
});

test("KAGEMUSHA activation predecessor is a complete governed registry", () => {
  const { payload } = fixture(ACTIVATE_PATH);
  assert.doesNotThrow(() => validateKagemushaReleaseSchemaV1(
    ROOTS.expected_predecessor, payload.expected_predecessor,
  ));
  assert.doesNotThrow(() => validateKagemushaReleaseSchemaV1(
    "GovernanceKagemushaBytes32V1", payload.successor_release_id,
  ));
  const omitted = structuredClone(payload.expected_predecessor);
  delete omitted.releases[0].receipt_digest;
  assert.throws(
    () => validateKagemushaReleaseSchemaV1(ROOTS.expected_predecessor, omitted),
    /missing required field/u,
  );
});

test("release manifest requires network identity and an exact scoped purpose", () => {
  const manifest = fixture(INSTALL_PATH).payload.manifest;
  for (const field of ["network_id", "purpose"]) {
    const missing = structuredClone(manifest);
    delete missing[field];
    assert.throws(() => validateKagemushaReleaseSchemaV1(ROOTS.manifest, missing), /missing required/u);
  }
  const experiment = {
    ...structuredClone(manifest),
    purpose: {
      kind: "testnet_experiment",
      value: {
        asset_identity_digest: Array(32).fill(1),
        asset_incarnation: Array(32).fill(2),
        asset_scale: 28,
        liability_pool_id: Array(32).fill(3),
      },
    },
  };
  assert.doesNotThrow(() => validateKagemushaReleaseSchemaV1(ROOTS.manifest, experiment));
  for (const purpose of [
    { kind: "production" },
    { kind: "production", value: {} },
    { kind: "unknown", value: null },
    { kind: "testnet_experiment", value: null },
    { ...experiment.purpose, value: { ...experiment.purpose.value, asset_scale: 29 } },
    { ...experiment.purpose, value: { ...experiment.purpose.value, retired: null } },
  ]) {
    assert.throws(() => validateKagemushaReleaseSchemaV1(ROOTS.manifest, { ...manifest, purpose }));
  }
});
