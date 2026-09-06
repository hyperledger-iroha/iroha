import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";

import * as sourceApi from "../src/index.js";
import * as distApi from "../dist/index.js";
import * as sourcePrivacyApi from "../src/privacyCapabilities.js";
import * as distPrivacyApi from "../dist/privacyCapabilities.js";
import {
  PRIVACY_PROTOCOL_IDS_V1 as SOURCE_IDS,
} from "../src/privacyCapabilities.js";
import {
  PRIVACY_PROTOCOL_IDS_V1 as DIST_IDS,
} from "../dist/privacyCapabilities.js";

const MATRIX_TEXT = readFileSync(
  new URL("../../../fixtures/privacy/exact12_v1.tsv", import.meta.url),
  "utf8",
);
const MATRIX_ROWS = MATRIX_TEXT
  .split("\n")
  .filter((line) => line.length > 0 && !line.startsWith("#"))
  .map((line) => line.split("\t"));
const rows = (kind) => MATRIX_ROWS.filter((row) => row[0] === kind);
const PROTOCOL_ROWS = Object.freeze(rows("protocol"));
const TYPED_ENVELOPE_ROWS = Object.freeze(rows("typed-envelope"));
const RETIRED_PROTOCOL_IDS = Object.freeze(rows("retired").map((row) => row[1]));
const EXPECTED_PROTOCOL_IDS = Object.freeze(PROTOCOL_ROWS.map((row) => row[2]));
const MATRIX_ROW_KINDS = Object.freeze([
  "matrix-version",
  "registry-sha256",
  "protocol",
  "typed-envelope",
  "retired",
]);

const RETIRED_EXPORTS = Object.freeze([
  "getPrivacyAlgorithmDescriptor",
  "getPrivacyAlgorithmDescriptors",
  "getPrivacyCapabilities",
  "getPrivacyCriteria",
  "validatePrivacyAlgorithmDescriptor",
  "buildPrivacyProofEnvelope",
  "noritoEncodePrivacyProofEnvelope",
  "noritoDecodePrivacyProofEnvelope",
  "buildZkAceTransferAuthorizationV1",
  "buildRegisterZkAceIdentityCommitmentInstruction",
  "buildRotateZkAceIdentityCommitmentInstruction",
  "buildRevokeZkAceIdentityCommitmentInstruction",
  "buildZkAceAuthorizationProofV1",
  "buildZkAceAuthorizedTransferInstruction",
  "buildRegisterPrivacyVerifierKeyInstruction",
  "buildRetirePrivacyVerifierKeyInstruction",
  "buildZkAtPolicyProofV1",
  "buildZkAtDevProofFixture",
  "buildZkAmsAdmissionBatchProofV0",
  "buildZkAmsAdmissionDevProofFixture",
  "buildVegaCredentialPredicateProofV0",
  "buildVegaCredentialDevProofFixture",
  "buildSilentThresholdCredentialShowingProofV0",
  "buildSilentThresholdCredentialDevProofFixture",
  "buildZkX509IdentityProofV0",
  "buildZkX509IdentityDevProofFixture",
  "buildJindoLatticeProofV0",
  "buildJindoLatticeDevProofFixture",
  "buildSisHintsAnonymousCredentialProofV0",
  "buildSisHintsCredentialDevProofFixture",
  "buildPenumbraSpendProofV1",
  "buildPenumbraOutputProofV1",
  "buildAztecPrivateKernelProofV1",
  "buildMidenStarkTransactionProofV1",
  "getPrivacyCapabilitiesV1",
  "parsePrivacyCapabilitySnapshotV1",
  "PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1",
  "PrivacyCapabilitySnapshotError",
]);

test("source and checked dist expose exactly the canonical 12 protocol ids", () => {
  assert.deepEqual(SOURCE_IDS, EXPECTED_PROTOCOL_IDS);
  assert.deepEqual(DIST_IDS, EXPECTED_PROTOCOL_IDS);
  assert.equal(new Set(SOURCE_IDS).size, 12);
  assert.equal(new Set(DIST_IDS).size, 12);
});

test("the shared exact12 matrix binds order, routes, typed envelopes, and retired ids", () => {
  assert.equal(MATRIX_TEXT.endsWith("\n"), true);
  assert.equal(MATRIX_TEXT.includes("\r"), false);
  assert.equal(MATRIX_TEXT.slice(0, -1).split("\n").every(Boolean), true);
  assert.equal(
    MATRIX_ROWS.every((row) => MATRIX_ROW_KINDS.includes(row[0])),
    true,
  );
  assert.deepEqual(rows("matrix-version"), [["matrix-version", "1"]]);
  assert.equal(PROTOCOL_ROWS.length, 12);
  assert.deepEqual(
    PROTOCOL_ROWS.map((row) => row[1]),
    Array.from({ length: 12 }, (_, index) => String(index)),
  );
  assert.equal(new Set(EXPECTED_PROTOCOL_IDS).size, 12);
  const registryDigest = createHash("sha256")
    .update(EXPECTED_PROTOCOL_IDS.map((value) => `${value}\n`).join(""))
    .digest("hex");
  assert.deepEqual(rows("registry-sha256"), [["registry-sha256", registryDigest]]);
  assert.deepEqual(
    TYPED_ENVELOPE_ROWS.map((row) => row.slice(1, 4)),
    PROTOCOL_ROWS.map((row) => row.slice(2, 5)),
  );
  assert.equal(TYPED_ENVELOPE_ROWS.length, 12);
  for (const row of TYPED_ENVELOPE_ROWS) {
    assert.equal(row.length, 6);
    for (const digest of row.slice(4)) {
      assert.match(digest, /^[0-9a-f]{64}$/u);
      assert.notEqual(digest, "0".repeat(64));
    }
  }
  assert.equal(new Set(RETIRED_PROTOCOL_IDS).size, RETIRED_PROTOCOL_IDS.length);
  assert.equal(
    RETIRED_PROTOCOL_IDS.every((value) => !EXPECTED_PROTOCOL_IDS.includes(value)),
    true,
  );
});

test("retired catalog and research-builder exports are absent", () => {
  for (const name of RETIRED_EXPORTS) {
    assert.equal(Object.hasOwn(sourceApi, name), false, `source export ${name}`);
    assert.equal(Object.hasOwn(distApi, name), false, `dist export ${name}`);
  }
});

test("Exact12 capability admission is available only from the optional subpath", () => {
  const expectedOptionalExports = [
    "PRIVACY_EXACT12_CAPABILITY_MANIFEST_MAX_BYTES_V1",
    "PRIVACY_EXACT12_CAPABILITY_MANIFEST_VERSION_V1",
    "PRIVACY_PROTOCOL_IDS_V1",
    "PrivacyExact12CapabilityManifestError",
    "PrivacyExact12CapabilityManifestV1",
    "compiledProfileCatalogV1",
    "decodePrivacyExact12CapabilityManifestV1",
    "getPrivacyExact12CapabilityManifestV1",
    "requirePrivacyExact12CapabilityAdmissionV1",
    "requirePrivacyExact12CapabilityTupleV1",
  ];
  for (const [label, rootApi] of [
    ["source root", sourceApi],
    ["dist root", distApi],
  ]) {
    for (const name of [
      "getPrivacyCapabilitiesV1",
      "parsePrivacyCapabilitySnapshotV1",
      "PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1",
      "PRIVACY_PROTOCOL_IDS_V1",
      "PrivacyCapabilitySnapshotError",
    ]) {
      assert.equal(Object.hasOwn(rootApi, name), false, `${label} export ${name}`);
    }
  }
  for (const [label, privacyApi] of [
    ["source optional API", sourcePrivacyApi],
    ["dist optional API", distPrivacyApi],
  ]) {
    assert.deepEqual(Object.keys(privacyApi).sort(), expectedOptionalExports, label);
    for (const name of expectedOptionalExports) {
      assert.notEqual(privacyApi[name], undefined, `${label} export ${name}`);
    }
  }
});
