import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  PRIVACY_FFI_ERROR_INVALID_REQUEST as JS_SRC_PRIVACY_FFI_ERROR_INVALID_REQUEST,
  PRIVACY_FFI_ERROR_MALFORMED_NORITO as JS_SRC_PRIVACY_FFI_ERROR_MALFORMED_NORITO,
  PRIVACY_FFI_ERROR_NULL_POINTER as JS_SRC_PRIVACY_FFI_ERROR_NULL_POINTER,
  PRIVACY_FFI_ERROR_PRODUCTION_DISABLED as JS_SRC_PRIVACY_FFI_ERROR_PRODUCTION_DISABLED,
  PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM as JS_SRC_PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM,
  PRIVACY_FFI_STATUS_ERROR as JS_SRC_PRIVACY_FFI_STATUS_ERROR,
  PRIVACY_FFI_VERSION_V1 as JS_SRC_PRIVACY_FFI_VERSION_V1,
  PRIVACY_NATIVE_ARCHIVE_MAX_BYTES as JS_SRC_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
  PRIVACY_REQUIRED_BRIDGE_ABI_VERSION as JS_SRC_PRIVACY_REQUIRED_BRIDGE_ABI_VERSION,
} from "../src/crypto.js";
import {
  PRIVACY_FFI_ERROR_INVALID_REQUEST as JS_BROWSER_SRC_PRIVACY_FFI_ERROR_INVALID_REQUEST,
  PRIVACY_FFI_ERROR_MALFORMED_NORITO as JS_BROWSER_SRC_PRIVACY_FFI_ERROR_MALFORMED_NORITO,
  PRIVACY_FFI_ERROR_NULL_POINTER as JS_BROWSER_SRC_PRIVACY_FFI_ERROR_NULL_POINTER,
  PRIVACY_FFI_ERROR_PRODUCTION_DISABLED as JS_BROWSER_SRC_PRIVACY_FFI_ERROR_PRODUCTION_DISABLED,
  PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM as JS_BROWSER_SRC_PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM,
  PRIVACY_FFI_STATUS_ERROR as JS_BROWSER_SRC_PRIVACY_FFI_STATUS_ERROR,
  PRIVACY_FFI_VERSION_V1 as JS_BROWSER_SRC_PRIVACY_FFI_VERSION_V1,
  PRIVACY_NATIVE_ARCHIVE_MAX_BYTES as JS_BROWSER_SRC_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
  PRIVACY_REQUIRED_BRIDGE_ABI_VERSION as JS_BROWSER_SRC_PRIVACY_REQUIRED_BRIDGE_ABI_VERSION,
} from "../src/crypto.browser.js";
import {
  PRIVACY_FFI_ERROR_INVALID_REQUEST as JS_DIST_PRIVACY_FFI_ERROR_INVALID_REQUEST,
  PRIVACY_FFI_ERROR_MALFORMED_NORITO as JS_DIST_PRIVACY_FFI_ERROR_MALFORMED_NORITO,
  PRIVACY_FFI_ERROR_NULL_POINTER as JS_DIST_PRIVACY_FFI_ERROR_NULL_POINTER,
  PRIVACY_FFI_ERROR_PRODUCTION_DISABLED as JS_DIST_PRIVACY_FFI_ERROR_PRODUCTION_DISABLED,
  PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM as JS_DIST_PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM,
  PRIVACY_FFI_STATUS_ERROR as JS_DIST_PRIVACY_FFI_STATUS_ERROR,
  PRIVACY_FFI_VERSION_V1 as JS_DIST_PRIVACY_FFI_VERSION_V1,
  PRIVACY_NATIVE_ARCHIVE_MAX_BYTES as JS_DIST_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
  PRIVACY_REQUIRED_BRIDGE_ABI_VERSION as JS_DIST_PRIVACY_REQUIRED_BRIDGE_ABI_VERSION,
} from "../dist/crypto.js";
import {
  PRIVACY_FFI_ERROR_INVALID_REQUEST as JS_BROWSER_DIST_PRIVACY_FFI_ERROR_INVALID_REQUEST,
  PRIVACY_FFI_ERROR_MALFORMED_NORITO as JS_BROWSER_DIST_PRIVACY_FFI_ERROR_MALFORMED_NORITO,
  PRIVACY_FFI_ERROR_NULL_POINTER as JS_BROWSER_DIST_PRIVACY_FFI_ERROR_NULL_POINTER,
  PRIVACY_FFI_ERROR_PRODUCTION_DISABLED as JS_BROWSER_DIST_PRIVACY_FFI_ERROR_PRODUCTION_DISABLED,
  PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM as JS_BROWSER_DIST_PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM,
  PRIVACY_FFI_STATUS_ERROR as JS_BROWSER_DIST_PRIVACY_FFI_STATUS_ERROR,
  PRIVACY_FFI_VERSION_V1 as JS_BROWSER_DIST_PRIVACY_FFI_VERSION_V1,
  PRIVACY_NATIVE_ARCHIVE_MAX_BYTES as JS_BROWSER_DIST_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
  PRIVACY_REQUIRED_BRIDGE_ABI_VERSION as JS_BROWSER_DIST_PRIVACY_REQUIRED_BRIDGE_ABI_VERSION,
} from "../dist/crypto.browser.js";
import {
  getPrivacyAlgorithmDescriptors as getSrcPrivacyAlgorithmDescriptors,
} from "../src/privacyAlgorithms.js";
import {
  getPrivacyAlgorithmDescriptors as getDistPrivacyAlgorithmDescriptors,
} from "../dist/privacyAlgorithms.js";

const REPO_ROOT = fileURLToPath(new URL("../../..", import.meta.url));
const PYTHON_PRIVACY_CATALOG = fileURLToPath(
  new URL("../../../python/iroha_python/src/iroha_python/privacy_catalog.py", import.meta.url),
);

const EXPECTED_CONTRACT = Object.freeze({
  requiredBridgeAbiVersion: 6,
  ffiVersionV1: 1,
  statusError: 1,
  errorNullPointer: 1,
  errorMalformedNorito: 2,
  errorUnsupportedAlgorithm: 3,
  errorProductionDisabled: 4,
  errorInvalidRequest: 5,
});
const LEGACY_PRIVACY_MALFORMED_AVAILABILITY_PROBE_ARCHIVE =
  "iroha-privacy-native-availability-probe-v1";
const EXPECTED_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES = 64 * 1024 * 1024;
const EXPECTED_PRIVACY_C_FFI_SYMBOLS = Object.freeze([
  "iroha_privacy_capabilities_v1",
  "iroha_privacy_build_proof_v1",
  "iroha_privacy_verify_proof_v1",
  "iroha_privacy_free_buffer",
]);
const REQUIRED_PRIVACY_HEADER_NEGATIVE_CONTROL_MODES = Object.freeze([
  "--negative-control-missing-privacy-header",
  "--negative-control-bad-privacy-signature",
  "--negative-control-missing-privacy-rust-export",
]);
const EXPECTED_PRIVACY_JNI_METHODS = Object.freeze([
  "nativeBridgeAbiVersion",
  "nativeCapabilities",
  "nativeBuildProof",
  "nativeVerifyProof",
]);
const EXPECTED_PRIVACY_PROOF_REQUEST_FIELDS = Object.freeze([
  Object.freeze(["algorithm_id", "String"]),
  Object.freeze(["entrypoint", "String"]),
  Object.freeze(["vk_ref", "String"]),
  Object.freeze(["public_inputs", "Vec<u8>"]),
  Object.freeze(["witness", "Vec<u8>"]),
  Object.freeze(["proof", "Vec<u8>"]),
]);
const EXPECTED_PRIVACY_PROOF_RESULT_FIELDS = Object.freeze([
  Object.freeze(["version", "u32"]),
  Object.freeze(["status", "u32"]),
  Object.freeze(["error_code", "u32"]),
  Object.freeze(["message", "String"]),
  Object.freeze(["algorithm_id", "String"]),
  Object.freeze(["entrypoint", "String"]),
  Object.freeze(["vk_ref", "String"]),
  Object.freeze(["public_inputs", "Vec<u8>"]),
  Object.freeze(["proof", "Vec<u8>"]),
  Object.freeze(["verified", "bool"]),
]);
const EXPECTED_PRIVACY_CAPABILITY_FIELDS = Object.freeze([
  Object.freeze(["algorithm_id", "String"]),
  Object.freeze(["proof_family", "String"]),
  Object.freeze(["backend_family", "String"]),
  Object.freeze(["sdk_entrypoints", "Vec<String>"]),
  Object.freeze(["planned_entrypoints", "Vec<String>"]),
  Object.freeze(["production_ready", "bool"]),
  Object.freeze(["production_gate", "PrivacyProductionGateV1"]),
]);
const EXPECTED_PRIVACY_CAPABILITIES_FIELDS = Object.freeze([
  Object.freeze(["version", "u32"]),
  Object.freeze(["gate_version", "String"]),
  Object.freeze(["algorithms", "Vec<PrivacyCapabilityV1>"]),
]);
const EXPECTED_PRIVACY_PRODUCTION_GATE_FIELDS = Object.freeze([
  Object.freeze(["version", "String"]),
  Object.freeze(["ready", "bool"]),
  Object.freeze(["gates", "Vec<PrivacyProductionGateStatusV1>"]),
  Object.freeze(["missing", "Vec<String>"]),
  Object.freeze(["audit_references", "Vec<String>"]),
]);
const EXPECTED_PRIVACY_PRODUCTION_GATE_STATUS_FIELDS = Object.freeze([
  Object.freeze(["key", "String"]),
  Object.freeze(["passed", "bool"]),
]);
const EXPECTED_SDK_PRIVACY_PRODUCTION_GATE_MISSING_REASONS = Object.freeze([
  "real proving engine is not registered",
  "real verifier is not registered",
  "chain admission path is not enabled",
  "cross-SDK parity is incomplete",
  "wallet/state support is incomplete",
  "deterministic tests are incomplete",
  "fuzzing gate is incomplete",
  "performance gate is incomplete",
  "external audit signoff is missing",
  "implementation stage is not production-hardened",
  "planned SDK entrypoints remain",
  "dev fixture entrypoints are not production entrypoints",
  "Iroha production allowlist is not enabled for this audited row",
]);
const EXPECTED_PRIVACY_OPERATION_VARIANTS = Object.freeze(["Build", "Verify"]);
const EXPECTED_PENDING_PRIVACY_BACKEND_LABELS = Object.freeze([
  "halo2-ipa-orchard",
  "groth16-bls12-377",
  "fcmp-plus-plus-curve-tree",
  "lattice-pcs-sis",
  "miden-stark",
  "aztec-plonkish-private-kernel",
  "pq-masp-stark-fri",
  "anonymous-pgc",
  "verange",
  "zkat",
  "recursive-anonymous-admission",
  "vega-existing-credential-zk",
  "silent-threshold-anoncred",
  "zk-x509",
  "sis-with-hints",
]);
const EXPECTED_ADVERSARIAL_PENDING_PRIVACY_BACKEND_LABELS = Object.freeze([
  "halo2/ipa/orchard/dev-fixture",
  "stark/fri/miden/claimed-production",
  "anonymous-pgc-k-out-of-n-v1-production",
  "sis-hints-anoncred-pq-v0-devfixture",
  "groth16/bls12-377/../../prod",
  "post-quantum-masp/audit-claimed",
]);
const EXPECTED_ADVERSARIAL_DEVELOPER_BACKEND_LABELS = Object.freeze([
  "stark/fri/dev-fixture",
  "stark/fri/d-e-v-f-i-x-t-u-r-e",
  "stark/fri/dev",
  "stark/fri/d-e-v",
  "stark/fri/test",
  "stark/fri/t-e-s-t",
  "stark/fri/placeholder",
  "halo2/ipa:dev-fixture",
  "halo2/ipa:dev",
  "halo2/ipa:d-e-v",
  "halo2/ipa:dummy",
  "halo2/ipa:f-a-k-e",
  "halo2/ipa:stub",
  "halo2/ipa:s-a-m-p-l-e",
]);
const EXPECTED_UNSTABLE_STARK_FRI_PROFILE_LABELS = Object.freeze([
  "stark/fri/latest",
  "stark/fri/attestation",
  "stark/fri/contest",
]);
const EXPECTED_DEVELOPER_ONLY_NATIVE_HALO2_PROFILE_LABELS = Object.freeze([
  "halo2/pasta/asset-hidden-transfer-public-test",
  "halo2/ipa/asset-hidden-transfer-public-test",
  "halo2/ipa:asset-hidden-transfer-public-test",
]);
const EXPECTED_TOY_NATIVE_HALO2_PROFILE_LABELS = Object.freeze([
  "halo2/pasta/tiny-add",
  "halo2/ipa/tiny-add",
  "halo2/ipa:tiny-add",
  "halo2/pasta/tiny-commit-open",
]);
const EXPECTED_LEGACY_VOTE_NATIVE_HALO2_PROFILE_LABELS = Object.freeze([
  "halo2/pasta/vote-bool-commit",
  "halo2/ipa/vote-bool-commit",
  "halo2/ipa:vote-bool-commit",
  "halo2/pasta/vote-bool-commit-merkle2",
  "halo2/ipa/vote-bool-commit-merkle8",
  "halo2/ipa:vote-bool-commit-merkle16",
]);
const EXPECTED_LEGACY_ANON_TRANSFER_NATIVE_HALO2_PROFILE_LABELS = Object.freeze([
  "halo2/pasta/anon-transfer-2x2",
  "halo2/ipa/anon-transfer-2x2",
  "halo2/ipa:anon-transfer-2x2",
  "halo2/pasta/anon-transfer-2x2-merkle2",
  "halo2/ipa/anon-transfer-2x2-merkle8",
  "halo2/ipa:anon-transfer-2x2-merkle16",
]);
const EXPECTED_UNREGISTERED_STARK_FRI_PROFILE_LABELS = Object.freeze([
  ...EXPECTED_UNSTABLE_STARK_FRI_PROFILE_LABELS,
  "stark/fri/random-profile",
  "stark/fri/sha512-goldilocks",
  "stark/fri/audit-proof-v1",
]);
const EXPECTED_SWIFT_PRIVACY_CAPABILITY_FIELDS = Object.freeze([
  "swiftSdkAvailable",
  "bridgeAvailable",
  "productionReady",
  "productionGate",
]);
const EXPECTED_KOTLIN_PRIVACY_CAPABILITY_FIELDS = Object.freeze([
  "kotlinSdkAvailable",
  "bridgeAvailable",
  "productionReady",
  "productionGate",
]);
const EXPECTED_CSHARP_PRIVACY_CAPABILITY_FIELDS = Object.freeze([
  "CSharpSdkAvailable",
  "BridgeAvailable",
  "ProductionReady",
  "ProductionGate",
]);
const EXPECTED_JAVA_PRIVACY_CAPABILITY_FIELDS = Object.freeze([
  "androidSdkAvailable",
  "bridgeAvailable",
  "productionGateVersion",
  "productionReady",
  "realProving",
  "realVerification",
  "chainAdmission",
  "sdkParity",
  "walletState",
  "deterministicTests",
  "fuzzing",
  "performanceGates",
  "externalAudit",
  "missingProductionGates",
  "auditReferences",
]);
const EXPECTED_SWIFT_PRIVACY_BRIDGE_METHODS = Object.freeze([
  "privacyCapabilities",
  "capabilitiesV1",
  "buildProofV1",
  "verifyProofV1",
]);
const EXPECTED_JAVA_PRIVACY_BRIDGE_METHODS = Object.freeze([
  "isNativeAvailable",
  "privacyCapabilities",
  "capabilitiesArchive",
  "buildProof",
  "verifyProof",
]);
const EXPECTED_KOTLIN_PRIVACY_BRIDGE_METHODS = Object.freeze([
  "isNativeAvailable",
  "privacyCapabilities",
  "capabilitiesArchive",
  "buildProof",
  "verifyProof",
]);
const EXPECTED_CSHARP_PRIVACY_BRIDGE_METHODS = Object.freeze([
  "IsAvailable",
  "GetPrivacyCapabilities",
  "CapabilitiesV1",
  "BuildProofV1",
  "VerifyProofV1",
]);

function source(relativePath) {
  return readFileSync(new URL(relativePath, `file://${REPO_ROOT}/`), "utf8");
}

function requireMatch(text, pattern, label) {
  const match = text.match(pattern);
  assert.ok(match, `${label} did not match ${pattern}`);
  return match;
}

function loadPythonPrivacyAlgorithmDescriptors() {
  const script = `
import importlib.util
import json
import sys

path = sys.argv[1]
spec = importlib.util.spec_from_file_location("privacy_catalog_direct", path)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
print(json.dumps(module.get_privacy_algorithm_descriptors(), sort_keys=False))
`;
  const result = spawnSync("python3", ["-c", script, PYTHON_PRIVACY_CATALOG], {
    encoding: "utf8",
    env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1" },
  });
  assert.equal(
    result.status,
    0,
    `python privacy catalog loader failed\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
  );
  return JSON.parse(result.stdout);
}

function publicProofedVerifierKeyEntries(descriptors, { pythonShape = false } = {}) {
  return descriptors
    .filter((descriptor) => {
      const proofFamily = pythonShape ? descriptor.proof_family : descriptor.proofFamily;
      const verifierKeyId = pythonShape ? descriptor.verifier_key_id : descriptor.verifierKeyId;
      return (
        verifierKeyId !== null &&
        verifierKeyId !== undefined &&
        proofFamily !== "none" &&
        proofFamily !== "commitment-only"
      );
    })
    .map((descriptor) => [
      descriptor.id,
      pythonShape ? descriptor.verifier_key_id : descriptor.verifierKeyId,
    ]);
}

function publicPrivacyCatalogNativeRows(descriptors, { pythonShape = false } = {}) {
  return descriptors.map((descriptor) => ({
    id: descriptor.id,
    proofFamily: pythonShape ? descriptor.proof_family : descriptor.proofFamily,
    backendFamily: pythonShape ? descriptor.backend_family : descriptor.backendFamily,
    sdkEntrypoints: pythonShape ? descriptor.sdk_entrypoints : descriptor.sdkEntrypoints,
    plannedSdkEntrypoints: pythonShape
      ? descriptor.planned_sdk_entrypoints
      : descriptor.plannedSdkEntrypoints,
  }));
}

function parseRustStringField(block, fieldName, label) {
  return requireMatch(
    block,
    new RegExp(`${fieldName}:\\s*"([^"]*)"`, "u"),
    `${label} ${fieldName}`,
  )[1];
}

function parseRustStringArrayField(block, fieldName, label) {
  const body = requireMatch(
    block,
    new RegExp(`${fieldName}:\\s*&\\[([\\s\\S]*?)\\]`, "u"),
    `${label} ${fieldName}`,
  )[1];
  return [...body.matchAll(/"([^"]+)"/g)].map((match) => match[1]);
}

function extractNativePrivacyCatalogRows(text, label) {
  const catalogBody = requireMatch(
    text,
    /const\s+PRIVACY_ALGORITHM_ENTRIES:\s*&\[PrivacyAlgorithmEntry\]\s*=\s*&\[([\s\S]*?)\n\];/,
    `${label} native privacy catalog entries`,
  )[1];
  const rows = [...catalogBody.matchAll(/PrivacyAlgorithmEntry\s*\{([\s\S]*?)\n\s*},/g)].map(
    (match, index) => {
      const block = match[1];
      return {
        id: parseRustStringField(block, "id", `${label} native privacy row ${index}`),
        proofFamily: parseRustStringField(
          block,
          "proof_family",
          `${label} native privacy row ${index}`,
        ),
        backendFamily: parseRustStringField(
          block,
          "backend_family",
          `${label} native privacy row ${index}`,
        ),
        sdkEntrypoints: parseRustStringArrayField(
          block,
          "sdk_entrypoints",
          `${label} native privacy row ${index}`,
        ),
        plannedSdkEntrypoints: parseRustStringArrayField(
          block,
          "planned_entrypoints",
          `${label} native privacy row ${index}`,
        ),
      };
    },
  );

  assert.ok(rows.length > 0, `${label} native privacy catalog entries are empty`);
  assert.equal(
    new Set(rows.map((row) => row.id)).size,
    rows.length,
    `${label} native privacy catalog entries contain duplicate ids`,
  );
  return rows;
}

function extractNativePrivacyVerifierKeyNameMap(text, label) {
  const body = requireMatch(
    text,
    /fn\s+privacy_catalog_vk_ref_name\([^)]*entry:\s*&PrivacyAlgorithmEntry[^)]*\)\s*->\s*&'static str\s*\{\s*match\s+entry\.id\s*\{([\s\S]*?)\n\s*}\s*\n\}/,
    `${label} native verifier-key name map`,
  )[1];
  const entries = [...body.matchAll(/"([^"]+)"\s*=>\s*"([^"]+)"/g)].map((match) => [
    match[1],
    match[2],
  ]);
  assert.ok(entries.length > 0, `${label} native verifier-key name map is empty`);
  assert.equal(
    new Set(entries.map(([algorithmId]) => algorithmId)).size,
    entries.length,
    `${label} native verifier-key name map contains duplicate algorithm ids`,
  );
  assert.equal(
    new Set(entries.map(([_algorithmId, verifierKeyName]) => verifierKeyName)).size,
    entries.length,
    `${label} native verifier-key name map contains duplicate verifier-key names`,
  );
  return new Map(entries);
}

function namesFromMatches(text, pattern) {
  return [...text.matchAll(pattern)].map((match) => match[1]);
}

function quotedStringsFromBlock(text) {
  return namesFromMatches(text, /"([^"]+)"/gu);
}

function assertProductionGateMissingReasons(label, text, pattern) {
  const block = requireMatch(text, pattern, `${label} production gate missing reasons`)[1];
  assert.deepEqual(
    quotedStringsFromBlock(block),
    EXPECTED_SDK_PRIVACY_PRODUCTION_GATE_MISSING_REASONS,
    `${label} production gate missing reasons drifted`,
  );
}

function extractJsCatalogProductionGateMissingReasons(text, label) {
  const requirementsBlock = requireMatch(
    text,
    /const\s+PRODUCTION_GATE_REQUIREMENTS\s*=\s*Object\.freeze\(\[([\s\S]*?)\]\);/,
    `${label} production gate requirements`,
  )[1];
  const requirements = [...requirementsBlock.matchAll(/Object\.freeze\(\["[^"]+",\s*"([^"]+)"\]\)/gu)].map(
    (match) => match[1],
  );
  assert.equal(requirements.length, 9, `${label} production gate requirement count drifted`);
  const supplemental = [
    "PRODUCTION_GATE_MISSING_IMPLEMENTATION_STAGE",
    "PRODUCTION_GATE_MISSING_PLANNED_SDK",
    "PRODUCTION_GATE_MISSING_DEV_FIXTURE",
    "PRODUCTION_GATE_MISSING_ALLOWLIST",
  ].map((constant) =>
    requireMatch(
      text,
      new RegExp(`${constant}\\s*=\\s*(?:\\(\\s*)?"([^"]+)"`, "u"),
      `${label} ${constant}`,
    )[1],
  );
  return [...requirements, ...supplemental];
}

function extractPythonCatalogProductionGateMissingReasons(text, label) {
  const requirementsBlock = requireMatch(
    text,
    /PRODUCTION_GATE_REQUIREMENTS\s*=\s*\(([\s\S]*?)\n\)/,
    `${label} production gate requirements`,
  )[1];
  const requirements = [...requirementsBlock.matchAll(/\("[^"]+",\s*"([^"]+)"\)/gu)].map(
    (match) => match[1],
  );
  assert.equal(requirements.length, 9, `${label} production gate requirement count drifted`);
  const supplemental = [
    "PRODUCTION_GATE_MISSING_IMPLEMENTATION_STAGE",
    "PRODUCTION_GATE_MISSING_PLANNED_SDK",
    "PRODUCTION_GATE_MISSING_DEV_FIXTURE",
    "PRODUCTION_GATE_MISSING_ALLOWLIST",
  ].map((constant) =>
    requireMatch(
      text,
      new RegExp(`${constant}\\s*=\\s*(?:\\(\\s*)?"([^"]+)"`, "u"),
      `${label} ${constant}`,
    )[1],
  );
  return [...requirements, ...supplemental];
}

function escapeRegExp(text) {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function assertRunnerRejectsNodeMajor(script, envName, label) {
  const tmp = mkdtempSync(`${tmpdir()}/iroha-js-runner-node-`);
  const fakeNode = `${tmp}/node`;
  try {
    writeFileSync(
      fakeNode,
      [
        "#!/usr/bin/env bash",
        "if [[ \"${1:-}\" == \"--version\" ]]; then",
        "  printf '%s\\n' 'v26.0.0'",
        "  exit 0",
        "fi",
        "printf '%s\\n' \"unexpected fake node invocation: $*\" >&2",
        "exit 64",
        "",
      ].join("\n"),
    );
    chmodSync(fakeNode, 0o755);

    const result = spawnSync("bash", [script], {
      cwd: REPO_ROOT,
      encoding: "utf8",
      env: { ...process.env, [envName]: fakeNode },
    });

    assert.notEqual(result.status, 0, `${label} must reject non-Node-20 overrides`);
    assert.match(result.stdout, /^v26\.0\.0$/m, `${label} must print the selected Node version`);
    assert.match(result.stderr, /require Node 20/u, `${label} must explain the Node 20 gate`);
    assert.doesNotMatch(
      result.stderr,
      /unexpected fake node invocation/u,
      `${label} must fail before running tests through the fake Node binary`,
    );
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

function assertRunnerRejectsPythonMajor(script, envName, label) {
  const tmp = mkdtempSync(`${tmpdir()}/iroha-python-runner-`);
  const fakePython = `${tmp}/python3`;
  try {
    writeFileSync(
      fakePython,
      [
        "#!/usr/bin/env bash",
        "case \"${1:-}\" in",
        "  -c)",
        "    printf '%s\\n' '3.9'",
        "    exit 0",
        "    ;;",
        "  --version)",
        "    printf '%s\\n' 'Python 3.9.6'",
        "    exit 0",
        "    ;;",
        "esac",
        "printf '%s\\n' \"unexpected fake python invocation: $*\" >&2",
        "exit 64",
        "",
      ].join("\n"),
    );
    chmodSync(fakePython, 0o755);

    const result = spawnSync("bash", [script], {
      cwd: REPO_ROOT,
      encoding: "utf8",
      env: { ...process.env, [envName]: fakePython },
    });

    assert.notEqual(result.status, 0, `${label} must reject non-Python-3.11 overrides`);
    assert.match(result.stdout, /^Python 3\.9\.6$/m, `${label} must print the selected Python version`);
    assert.match(result.stderr, /require Python 3\.11/u, `${label} must explain the Python 3.11 gate`);
    assert.doesNotMatch(
      result.stderr,
      /unexpected fake python invocation/u,
      `${label} must fail before venv setup or native builds`,
    );
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

function assertRunnerPropagatesSwiftParseFailure(script, envName, label) {
  const tmp = mkdtempSync(`${tmpdir()}/iroha-swift-runner-`);
  const fakeSwiftc = `${tmp}/swiftc`;
  try {
    writeFileSync(
      fakeSwiftc,
      [
        "#!/usr/bin/env bash",
        "if [[ \"${1:-}\" == \"--version\" ]]; then",
        "  printf '%s\\n' 'Swift version 5.10.1 (fake)'",
        "  exit 0",
        "fi",
        "printf '%s\\n' \"fake swift parse failed: $*\" >&2",
        "exit 66",
        "",
      ].join("\n"),
    );
    chmodSync(fakeSwiftc, 0o755);

    const result = spawnSync("bash", [script], {
      cwd: REPO_ROOT,
      encoding: "utf8",
      env: { ...process.env, [envName]: fakeSwiftc },
    });

    assert.notEqual(result.status, 0, `${label} must propagate swiftc parse failures`);
    assert.match(result.stdout, /Swift version 5\.10\.1 \(fake\)/u, `${label} must print swiftc version evidence`);
    assert.match(result.stderr, /fake swift parse failed:/u, `${label} must execute the parse command`);
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

function assertRunnerRejectsJavaHome(script, envName, label) {
  const tmp = mkdtempSync(`${tmpdir()}/iroha-jdk-runner-`);
  const binDir = `${tmp}/bin`;
  const fakeJava = `${binDir}/java`;
  try {
    mkdirSync(binDir, { recursive: true });
    writeFileSync(
      fakeJava,
      [
        "#!/usr/bin/env bash",
        "printf '%s\\n' 'openjdk version \"25.0.1\" 2026-01-01' >&2",
        "exit 0",
        "",
      ].join("\n"),
    );
    chmodSync(fakeJava, 0o755);

    const result = spawnSync("bash", [script], {
      cwd: REPO_ROOT,
      encoding: "utf8",
      env: { ...process.env, [envName]: tmp },
    });

    assert.notEqual(result.status, 0, `${label} must reject non-JDK-21 homes`);
    assert.match(result.stderr, /JDK 21 home/u, `${label} must explain the JDK 21 gate`);
    assert.doesNotMatch(result.stderr, /gradle|javac/u, `${label} must fail before JVM tests or javac`);
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

function assertRunnerRejectsDotnetSdk(script, envName, label) {
  const tmp = mkdtempSync(`${tmpdir()}/iroha-dotnet-runner-`);
  const fakeDotnet = `${tmp}/dotnet`;
  try {
    writeFileSync(
      fakeDotnet,
      [
        "#!/usr/bin/env bash",
        "if [[ \"${1:-}\" == \"--version\" ]]; then",
        "  printf '%s\\n' '7.0.404'",
        "  exit 0",
        "fi",
        "printf '%s\\n' \"unexpected fake dotnet invocation: $*\" >&2",
        "exit 64",
        "",
      ].join("\n"),
    );
    chmodSync(fakeDotnet, 0o755);

    const result = spawnSync("bash", [script], {
      cwd: REPO_ROOT,
      encoding: "utf8",
      env: { ...process.env, [envName]: fakeDotnet },
    });

    assert.notEqual(result.status, 0, `${label} must reject non-.NET-8 SDKs`);
    assert.match(result.stdout, /^7\.0\.404$/m, `${label} must print dotnet version evidence`);
    assert.match(result.stderr, /\.NET SDK 8\.0\.x/u, `${label} must explain the .NET 8 gate`);
    assert.doesNotMatch(
      result.stderr,
      /unexpected fake dotnet invocation/u,
      `${label} must fail before dotnet test`,
    );
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

function negativeControlModesFromInventory(text, startMarker, endMarker) {
  const start = text.indexOf(startMarker);
  assert.notEqual(start, -1, `missing ${startMarker}`);
  const end = text.indexOf(endMarker, start);
  assert.notEqual(end, -1, `missing ${endMarker}`);
  const modes = namesFromMatches(
    text.slice(start, end),
    /(--negative-control-[A-Za-z0-9-]+)/gu,
  );
  assert.equal(new Set(modes).size, modes.length, `${startMarker} must not duplicate modes`);
  return [...modes].sort();
}

function isPathInventoryString(value) {
  return (
    !/\s/u.test(value) &&
    (value.includes("/") || /\.(?:cs|h|java|js|json|kt|md|py|rs|sh|swift|toml|ts|yaml|yml)$/u.test(value))
  );
}

function quotedStringsFromInventory(text, startMarker, endMarker) {
  const start = text.indexOf(startMarker);
  assert.notEqual(start, -1, `missing ${startMarker}`);
  const end = text.indexOf(endMarker, start);
  assert.notEqual(end, -1, `missing ${endMarker}`);
  const paths = namesFromMatches(
    text.slice(start, end),
    /"([^"]+)"/gu,
  ).filter(isPathInventoryString);
  assert.equal(new Set(paths).size, paths.length, `${startMarker} must not duplicate paths`);
  return [...paths].sort();
}

function assertWorkflowIncludesPaths(workflow, paths, label) {
  for (const path of paths) {
    assert.ok(
      new RegExp(`- "${escapeRegExp(path)}"`).test(workflow) ||
        workflow
          .match(/- "([^"]+\/\*\*)"/gu)
          ?.some((entry) => path.startsWith(entry.slice(3, -3))) === true,
      `${label} workflow paths must include ${path}`,
    );
  }
}

function assertWorkflowRunsNegativeControlModes(workflow, command, modes, label) {
  for (const mode of modes) {
    assert.match(
      workflow,
      new RegExp(`^\\s+${escapeRegExp(command)} ${escapeRegExp(mode)}$`, "m"),
      `${label} workflow must run ${mode}`,
    );
  }
}

function numberConst(text, pattern, label) {
  return Number(requireMatch(text, pattern, label)[1]);
}

function intExpressionConst(text, pattern, label) {
  const expression = requireMatch(text, pattern, label)[1].replace(/\s+/g, "");
  assert.match(expression, /^\d+(?:\*\d+)*$/, `${label} must be an integer product`);
  return expression
    .split("*")
    .map((term) => Number(term))
    .reduce((accumulator, term) => accumulator * term, 1);
}

function rustConst(text, name) {
  return numberConst(
    text,
    new RegExp(`const\\s+${name}\\s*:\\s*u32\\s*=\\s*(\\d+)\\s*;`),
    `Rust const ${name}`,
  );
}

function rustUsizeExpressionConst(text, name) {
  return intExpressionConst(
    text,
    new RegExp(`const\\s+${name}\\s*:\\s*usize\\s*=\\s*([^;]+);`),
    `Rust const ${name}`,
  );
}

function javaConst(text, name) {
  return numberConst(
    text,
    new RegExp(`public\\s+static\\s+final\\s+int\\s+${name}\\s*=\\s*(\\d+)\\s*;`),
    `Java const ${name}`,
  );
}

function kotlinConst(text, name) {
  return numberConst(
    text,
    new RegExp(`const\\s+val\\s+${name}\\s*:\\s*Int\\s*=\\s*(\\d+)`),
    `Kotlin const ${name}`,
  );
}

function csharpConst(text, name) {
  return numberConst(
    text,
    new RegExp(`public\\s+const\\s+uint\\s+${name}\\s*=\\s*(\\d+)\\s*;`),
    `C# const ${name}`,
  );
}

function pythonFinal(text, name) {
  return numberConst(
    text,
    new RegExp(`${name}\\s*:\\s*Final\\[int\\]\\s*=\\s*(\\d+)`),
    `Python Final ${name}`,
  );
}

function swiftStatic(text, name) {
  return numberConst(
    text,
    new RegExp(`public\\s+static\\s+let\\s+${name}\\s*:\\s*UInt32\\s*=\\s*(\\d+)`),
    `Swift static ${name}`,
  );
}

function swiftStaticIntExpression(text, name) {
  return intExpressionConst(
    text,
    new RegExp(`public\\s+static\\s+let\\s+${name}\\s*=\\s*([^\\n]+)`),
    `Swift static ${name}`,
  );
}

function rustBridgeAbiReturn(text, functionName) {
  const body = requireMatch(
    text,
    new RegExp(`fn\\s+${functionName}\\([^)]*\\)\\s*->\\s*u32\\s*\\{([^}]*)\\}`),
    `Rust bridge ABI return ${functionName}`,
  )[1].trim();
  if (/^\d+$/.test(body)) {
    return Number(body);
  }
  return rustConst(text, body);
}

function assertContractSubset(label, actual) {
  for (const [key, value] of Object.entries(actual)) {
    assert.equal(value, EXPECTED_CONTRACT[key], `${label} ${key} drifted`);
  }
}

function rustPrivacyFfiConstants(text, { includesNullPointer }) {
  const constants = {
    ffiVersionV1: rustConst(text, "PRIVACY_FFI_VERSION_V1"),
    statusError: rustConst(text, "PRIVACY_FFI_STATUS_ERROR"),
    errorMalformedNorito: rustConst(text, "PRIVACY_FFI_ERROR_MALFORMED_NORITO"),
    errorUnsupportedAlgorithm: rustConst(text, "PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM"),
    errorProductionDisabled: rustConst(text, "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED"),
    errorInvalidRequest: rustConst(text, "PRIVACY_FFI_ERROR_INVALID_REQUEST"),
  };
  if (includesNullPointer) {
    constants.errorNullPointer = rustConst(text, "PRIVACY_FFI_ERROR_NULL_POINTER");
  }
  return constants;
}

function jvmPrivacyFfiConstants(text, constReader) {
  return {
    requiredBridgeAbiVersion: constReader(text, "REQUIRED_BRIDGE_ABI_VERSION"),
    ffiVersionV1: constReader(text, "PRIVACY_FFI_VERSION_V1"),
    statusError: constReader(text, "STATUS_ERROR"),
    errorNullPointer: constReader(text, "ERROR_NULL_POINTER"),
    errorMalformedNorito: constReader(text, "ERROR_MALFORMED_NORITO"),
    errorUnsupportedAlgorithm: constReader(text, "ERROR_UNSUPPORTED_ALGORITHM"),
    errorProductionDisabled: constReader(text, "ERROR_PRODUCTION_DISABLED"),
    errorInvalidRequest: constReader(text, "ERROR_INVALID_REQUEST"),
  };
}

function rustStructFields(text, name) {
  const body = requireMatch(
    text,
    new RegExp(`struct\\s+${name}\\s*\\{([\\s\\S]*?)\\}`),
    `Rust struct ${name}`,
  )[1];
  return body
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => line.replace(/,$/, ""))
    .map((line) => {
      const match = line.match(/^([a-zA-Z_][a-zA-Z0-9_]*)\s*:\s*(.+)$/);
      assert.ok(match, `${name} field line has unexpected shape: ${line}`);
      return [match[1], match[2]];
    });
}

function assertRustStructIsNorito(text, name, expectedFields, label) {
  assert.match(
    text,
    new RegExp(
      `#\\[derive\\([^\\]]*norito::Encode,\\s*norito::Decode[^\\]]*\\)\\]\\s*struct\\s+${name}\\s*\\{`,
    ),
    `${label} ${name} must derive Norito Encode/Decode`,
  );
  assert.deepEqual(rustStructFields(text, name), expectedFields, `${label} ${name} schema drifted`);
}

function rustEnumVariants(text, name) {
  const body = requireMatch(
    text,
    new RegExp(`enum\\s+${name}\\s*\\{([\\s\\S]*?)\\}`),
    `Rust enum ${name}`,
  )[1];
  return body
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => line.replace(/,$/, ""));
}

function assertRustNoMangleExport(text, name, signaturePattern, label) {
  assert.match(
    text,
    new RegExp(`#\\[unsafe\\(no_mangle\\)\\]\\s*pub\\s+${signaturePattern}\\s+${name}\\s*\\(`),
    `${label} must export ${name} with the stable privacy FFI symbol`,
  );
}

function sliceBetween(text, start, end, label) {
  const startIndex = text.indexOf(start);
  assert.notEqual(startIndex, -1, `${label} missing start marker ${start}`);
  const endIndex = text.indexOf(end, startIndex + start.length);
  assert.notEqual(endIndex, -1, `${label} missing end marker ${end}`);
  return text.slice(startIndex, endIndex);
}

function sliceFrom(text, start, label) {
  const startIndex = text.indexOf(start);
  assert.notEqual(startIndex, -1, `${label} missing start marker ${start}`);
  return text.slice(startIndex);
}

function assertNoDirectAlgorithmCapabilityFields(label, body) {
  assert.doesNotMatch(
    body,
    /\b(?:anonymousPgc|AnonymousPgc|verange|VeRange|zkat|ZkAt|zkAms|ZkAms|vega|Vega|silentThreshold|SilentThreshold|zkX509|ZkX509|jindo|Jindo|sisHints|SisHints|orchard|Orchard|penumbra|Penumbra|fcmp|Fcmp|miden|Miden|aztec|Aztec|pqMasp|PqMasp|mlKem|MlKem|assetHiddenTransferProof|AssetHiddenTransferProof|build[A-Z]|verify[A-Z])\b/,
    `${label} privacy capabilities must not expose direct algorithm capability fields`,
  );
}

test("privacy FFI ABI and deterministic error constants stay in parity", () => {
  assertContractSubset("JS src crypto", {
    ffiVersionV1: JS_SRC_PRIVACY_FFI_VERSION_V1,
    requiredBridgeAbiVersion: JS_SRC_PRIVACY_REQUIRED_BRIDGE_ABI_VERSION,
    statusError: JS_SRC_PRIVACY_FFI_STATUS_ERROR,
    errorNullPointer: JS_SRC_PRIVACY_FFI_ERROR_NULL_POINTER,
    errorMalformedNorito: JS_SRC_PRIVACY_FFI_ERROR_MALFORMED_NORITO,
    errorUnsupportedAlgorithm: JS_SRC_PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM,
    errorProductionDisabled: JS_SRC_PRIVACY_FFI_ERROR_PRODUCTION_DISABLED,
    errorInvalidRequest: JS_SRC_PRIVACY_FFI_ERROR_INVALID_REQUEST,
  });
  assert.equal(
    JS_SRC_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
    EXPECTED_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
    "JS src privacy native archive cap drifted",
  );
  assert.equal(
    JS_BROWSER_SRC_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
    EXPECTED_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
    "JS browser src privacy native archive cap drifted",
  );
  assertContractSubset("JS browser src crypto", {
    ffiVersionV1: JS_BROWSER_SRC_PRIVACY_FFI_VERSION_V1,
    requiredBridgeAbiVersion: JS_BROWSER_SRC_PRIVACY_REQUIRED_BRIDGE_ABI_VERSION,
    statusError: JS_BROWSER_SRC_PRIVACY_FFI_STATUS_ERROR,
    errorNullPointer: JS_BROWSER_SRC_PRIVACY_FFI_ERROR_NULL_POINTER,
    errorMalformedNorito: JS_BROWSER_SRC_PRIVACY_FFI_ERROR_MALFORMED_NORITO,
    errorUnsupportedAlgorithm: JS_BROWSER_SRC_PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM,
    errorProductionDisabled: JS_BROWSER_SRC_PRIVACY_FFI_ERROR_PRODUCTION_DISABLED,
    errorInvalidRequest: JS_BROWSER_SRC_PRIVACY_FFI_ERROR_INVALID_REQUEST,
  });
  assertContractSubset("JS dist crypto", {
    ffiVersionV1: JS_DIST_PRIVACY_FFI_VERSION_V1,
    requiredBridgeAbiVersion: JS_DIST_PRIVACY_REQUIRED_BRIDGE_ABI_VERSION,
    statusError: JS_DIST_PRIVACY_FFI_STATUS_ERROR,
    errorNullPointer: JS_DIST_PRIVACY_FFI_ERROR_NULL_POINTER,
    errorMalformedNorito: JS_DIST_PRIVACY_FFI_ERROR_MALFORMED_NORITO,
    errorUnsupportedAlgorithm: JS_DIST_PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM,
    errorProductionDisabled: JS_DIST_PRIVACY_FFI_ERROR_PRODUCTION_DISABLED,
    errorInvalidRequest: JS_DIST_PRIVACY_FFI_ERROR_INVALID_REQUEST,
  });
  assert.equal(
    JS_DIST_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
    EXPECTED_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
    "JS dist privacy native archive cap drifted",
  );
  assert.equal(
    JS_BROWSER_DIST_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
    EXPECTED_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
    "JS browser dist privacy native archive cap drifted",
  );
  assertContractSubset("JS browser dist crypto", {
    ffiVersionV1: JS_BROWSER_DIST_PRIVACY_FFI_VERSION_V1,
    requiredBridgeAbiVersion: JS_BROWSER_DIST_PRIVACY_REQUIRED_BRIDGE_ABI_VERSION,
    statusError: JS_BROWSER_DIST_PRIVACY_FFI_STATUS_ERROR,
    errorNullPointer: JS_BROWSER_DIST_PRIVACY_FFI_ERROR_NULL_POINTER,
    errorMalformedNorito: JS_BROWSER_DIST_PRIVACY_FFI_ERROR_MALFORMED_NORITO,
    errorUnsupportedAlgorithm: JS_BROWSER_DIST_PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM,
    errorProductionDisabled: JS_BROWSER_DIST_PRIVACY_FFI_ERROR_PRODUCTION_DISABLED,
    errorInvalidRequest: JS_BROWSER_DIST_PRIVACY_FFI_ERROR_INVALID_REQUEST,
  });

  const pythonCrypto = source("python/iroha_python/src/iroha_python/crypto.py");
  assertContractSubset("Python crypto", {
    ffiVersionV1: pythonFinal(pythonCrypto, "PRIVACY_FFI_VERSION_V1"),
    requiredBridgeAbiVersion: pythonFinal(pythonCrypto, "PRIVACY_REQUIRED_BRIDGE_ABI_VERSION"),
    statusError: pythonFinal(pythonCrypto, "PRIVACY_FFI_STATUS_ERROR"),
    errorNullPointer: pythonFinal(pythonCrypto, "PRIVACY_FFI_ERROR_NULL_POINTER"),
    errorMalformedNorito: pythonFinal(pythonCrypto, "PRIVACY_FFI_ERROR_MALFORMED_NORITO"),
    errorUnsupportedAlgorithm: pythonFinal(pythonCrypto, "PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM"),
    errorProductionDisabled: pythonFinal(pythonCrypto, "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED"),
    errorInvalidRequest: pythonFinal(pythonCrypto, "PRIVACY_FFI_ERROR_INVALID_REQUEST"),
  });
  assert.equal(
    intExpressionConst(
      pythonCrypto,
      /PRIVACY_NATIVE_ARCHIVE_MAX_BYTES\s*:\s*Final\[int\]\s*=\s*([^\n]+)/,
      "Python PRIVACY_NATIVE_ARCHIVE_MAX_BYTES",
    ),
    EXPECTED_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
    "Python privacy native archive cap drifted",
  );

  const swiftBridge = source("IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift");
  assertContractSubset("Swift privacy bridge", {
    ffiVersionV1: swiftStatic(swiftBridge, "ffiVersionV1"),
    requiredBridgeAbiVersion: swiftStatic(swiftBridge, "requiredBridgeAbiVersion"),
    statusError: swiftStatic(swiftBridge, "ffiStatusError"),
    errorNullPointer: swiftStatic(swiftBridge, "ffiErrorNullPointer"),
    errorMalformedNorito: swiftStatic(swiftBridge, "ffiErrorMalformedNorito"),
    errorUnsupportedAlgorithm: swiftStatic(swiftBridge, "ffiErrorUnsupportedAlgorithm"),
    errorProductionDisabled: swiftStatic(swiftBridge, "ffiErrorProductionDisabled"),
    errorInvalidRequest: swiftStatic(swiftBridge, "ffiErrorInvalidRequest"),
  });
  assert.equal(
    swiftStaticIntExpression(swiftBridge, "privacyNativeArchiveMaxBytes"),
    EXPECTED_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
    "Swift privacy native archive cap drifted",
  );
  assert.equal(
    intExpressionConst(
      source("IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"),
      /static\s+let\s+privacyNativeArchiveMaxBytes\s*=\s*([^\n]+)/,
      "Swift NativeBridge privacyNativeArchiveMaxBytes",
    ),
    EXPECTED_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
    "Swift NativeBridge privacy native archive cap drifted",
  );

  const connectBridge = source("crates/connect_norito_bridge/src/lib.rs");
  assertContractSubset("connect_norito_bridge privacy FFI", {
    requiredBridgeAbiVersion: rustBridgeAbiReturn(connectBridge, "connect_norito_bridge_abi_version"),
    ...rustPrivacyFfiConstants(connectBridge, { includesNullPointer: true }),
  });
  assert.equal(
    rustUsizeExpressionConst(connectBridge, "PRIVACY_NATIVE_ARCHIVE_MAX_BYTES"),
    EXPECTED_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
    "connect_norito_bridge privacy native archive cap drifted",
  );

  const jsHost = source("crates/iroha_js_host/src/lib.rs");
  assertContractSubset("iroha_js_host privacy FFI", {
    requiredBridgeAbiVersion: rustBridgeAbiReturn(jsHost, "connect_norito_bridge_abi_version"),
    ...rustPrivacyFfiConstants(jsHost, { includesNullPointer: false }),
  });
  assert.equal(
    rustUsizeExpressionConst(jsHost, "PRIVACY_NATIVE_ARCHIVE_MAX_BYTES"),
    EXPECTED_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
    "iroha_js_host privacy native archive cap drifted",
  );

  const pythonRust = source("python/iroha_python/iroha_python_rs/src/lib.rs");
  assertContractSubset("iroha_python_rs privacy FFI", {
    requiredBridgeAbiVersion: rustBridgeAbiReturn(pythonRust, "privacy_bridge_abi_version_py"),
    ...rustPrivacyFfiConstants(pythonRust, { includesNullPointer: false }),
  });
  assert.equal(
    rustUsizeExpressionConst(pythonRust, "PRIVACY_NATIVE_ARCHIVE_MAX_BYTES"),
    EXPECTED_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
    "iroha_python_rs privacy native archive cap drifted",
  );

  assertContractSubset(
    "Java Android privacy bridge",
    jvmPrivacyFfiConstants(
      source("java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java"),
      javaConst,
    ),
  );
  assert.equal(
    intExpressionConst(
      source("java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java"),
      /public\s+static\s+final\s+int\s+PRIVACY_NATIVE_ARCHIVE_MAX_BYTES\s*=\s*([^;]+);/,
      "Java PRIVACY_NATIVE_ARCHIVE_MAX_BYTES",
    ),
    EXPECTED_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
    "Java Android privacy native archive cap drifted",
  );
  assertContractSubset(
    "Kotlin JVM privacy bridge",
    jvmPrivacyFfiConstants(
      source("kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt"),
      kotlinConst,
    ),
  );
  assert.equal(
    intExpressionConst(
      source("kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt"),
      /const\s+val\s+PRIVACY_NATIVE_ARCHIVE_MAX_BYTES\s*:\s*Int\s*=\s*([^\n]+)/,
      "Kotlin PRIVACY_NATIVE_ARCHIVE_MAX_BYTES",
    ),
    EXPECTED_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
    "Kotlin JVM privacy native archive cap drifted",
  );

  const csharpBridge = source("csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs");
  assertContractSubset("C# privacy bridge", {
    requiredBridgeAbiVersion: csharpConst(csharpBridge, "RequiredBridgeAbiVersion"),
    ffiVersionV1: csharpConst(csharpBridge, "FfiVersionV1"),
    statusError: csharpConst(csharpBridge, "StatusError"),
    errorNullPointer: csharpConst(csharpBridge, "ErrorNullPointer"),
    errorMalformedNorito: csharpConst(csharpBridge, "ErrorMalformedNorito"),
    errorUnsupportedAlgorithm: csharpConst(csharpBridge, "ErrorUnsupportedAlgorithm"),
    errorProductionDisabled: csharpConst(csharpBridge, "ErrorProductionDisabled"),
    errorInvalidRequest: csharpConst(csharpBridge, "ErrorInvalidRequest"),
  });
  assert.equal(
    intExpressionConst(
      csharpBridge,
      /public\s+const\s+int\s+PrivacyNativeArchiveMaxBytes\s*=\s*([^;]+);/,
      "C# PrivacyNativeArchiveMaxBytes",
    ),
    EXPECTED_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
    "C# privacy native archive cap drifted",
  );
});

test("privacy FFI public symbol names stay stable across native bindings", () => {
  const connectBridge = source("crates/connect_norito_bridge/src/lib.rs");
  const connectHeader = source("crates/connect_norito_bridge/include/connect_norito_bridge.h");
  const swiftNativeBridge = source("IrohaSwift/Sources/IrohaSwift/NativeBridge.swift");
  const csharpBridge = source("csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs");
  const javaBridge = source(
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java",
  );
  const kotlinBridge = source(
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt",
  );

  assertRustNoMangleExport(
    connectBridge,
    "iroha_privacy_capabilities_v1",
    'unsafe\\s+extern\\s+"C"\\s+fn',
    "connect_norito_bridge",
  );
  assertRustNoMangleExport(
    connectBridge,
    "iroha_privacy_build_proof_v1",
    'unsafe\\s+extern\\s+"C"\\s+fn',
    "connect_norito_bridge",
  );
  assertRustNoMangleExport(
    connectBridge,
    "iroha_privacy_verify_proof_v1",
    'unsafe\\s+extern\\s+"C"\\s+fn',
    "connect_norito_bridge",
  );
  assertRustNoMangleExport(
    connectBridge,
    "iroha_privacy_free_buffer",
    'extern\\s+"C"\\s+fn',
    "connect_norito_bridge",
  );
  assert.match(
    connectBridge,
    /fn\s+iroha_privacy_build_proof_v1\([\s\S]*?PrivacyProofOperationV1::Build[\s\S]*?\}/,
    "build FFI symbol must dispatch the Build operation",
  );
  assert.match(
    connectBridge,
    /fn\s+iroha_privacy_verify_proof_v1\([\s\S]*?PrivacyProofOperationV1::Verify[\s\S]*?\}/,
    "verify FFI symbol must dispatch the Verify operation",
  );
  assert.match(
    connectBridge,
    /fn\s+iroha_privacy_free_buffer\([^)]*\)\s*\{[\s\S]*?connect_norito_free\(ptr_\);[\s\S]*?\}/,
    "privacy free-buffer symbol must delegate to the bridge allocator free path",
  );

  for (const symbol of EXPECTED_PRIVACY_C_FFI_SYMBOLS) {
    assert.ok(swiftNativeBridge.includes(`dlsym(handle, "${symbol}")`));
    assert.ok(csharpBridge.includes(`EntryPoint = "${symbol}"`));
  }
  for (const symbol of EXPECTED_PRIVACY_C_FFI_SYMBOLS.filter(
    (name) => name !== "iroha_privacy_free_buffer",
  )) {
    assert.match(
      connectHeader,
      new RegExp(`int32_t\\s+${symbol}\\s*\\(`),
      `C bridge header must declare ${symbol}`,
    );
  }
  assert.match(
    connectHeader,
    /void\s+iroha_privacy_free_buffer\s*\(/,
    "C bridge header must declare iroha_privacy_free_buffer",
  );

  for (const method of EXPECTED_PRIVACY_JNI_METHODS) {
    assert.match(
      connectBridge,
      new RegExp(`Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_${method}`),
      `Kotlin/JVM JNI wrapper for ${method} drifted`,
    );
    assert.match(
      connectBridge,
      new RegExp(`Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_${method}`),
      `Java Android JNI wrapper for ${method} drifted`,
    );
  }
  assert.match(javaBridge, /private\s+static\s+native\s+int\s+nativeBridgeAbiVersion\(\);/);
  assert.match(javaBridge, /private\s+static\s+native\s+byte\[\]\s+nativeCapabilities\(\);/);
  assert.match(javaBridge, /private\s+static\s+native\s+byte\[\]\s+nativeBuildProof\(byte\[\]\s+requestArchive\);/);
  assert.match(javaBridge, /private\s+static\s+native\s+byte\[\]\s+nativeVerifyProof\(byte\[\]\s+requestArchive\);/);
  assert.match(kotlinBridge, /private\s+external\s+fun\s+nativeBridgeAbiVersion\(\):\s+Int/);
  assert.match(kotlinBridge, /private\s+external\s+fun\s+nativeCapabilities\(\):\s+ByteArray\?/);
  assert.match(kotlinBridge, /private\s+external\s+fun\s+nativeBuildProof\(requestArchive:\s+ByteArray\):\s+ByteArray\?/);
  assert.match(kotlinBridge, /private\s+external\s+fun\s+nativeVerifyProof\(requestArchive:\s+ByteArray\):\s+ByteArray\?/);
});

test("SDK privacy native bridges expose only generic archive operations", () => {
  const swiftBridge = source("IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift");
  assert.deepEqual(
    namesFromMatches(swiftBridge, /public\s+static\s+func\s+([A-Za-z][A-Za-z0-9_]*)\s*\(/g),
    EXPECTED_SWIFT_PRIVACY_BRIDGE_METHODS,
    "Swift PrivacyNativeBridge public methods drifted",
  );

  const javaBridge = sliceBetween(
    source("java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java"),
    "public final class PrivacyNativeBridge",
    "public static final class PrivacyCapabilities",
    "Java Android PrivacyNativeBridge public method surface",
  );
  assert.deepEqual(
    namesFromMatches(
      javaBridge,
      /public\s+static\s+(?:[A-Za-z0-9_<>\[\]]+\s+)+([A-Za-z][A-Za-z0-9_]*)\s*\(/g,
    ),
    EXPECTED_JAVA_PRIVACY_BRIDGE_METHODS,
    "Java Android PrivacyNativeBridge public methods drifted",
  );

  const kotlinBridge = sliceBetween(
    source("kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt"),
    "class PrivacyNativeBridge private constructor()",
    "internal fun call(",
    "Kotlin/JVM PrivacyNativeBridge public method surface",
  );
  assert.deepEqual(
    namesFromMatches(
      kotlinBridge,
      /@JvmStatic\s*\n\s*fun\s+([A-Za-z][A-Za-z0-9_]*)\s*\(/g,
    ),
    EXPECTED_KOTLIN_PRIVACY_BRIDGE_METHODS,
    "Kotlin/JVM PrivacyNativeBridge public methods drifted",
  );

  const csharpBridge = sliceBetween(
    source("csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs"),
    "public static class PrivacyNative",
    "internal delegate int NativeCapabilitiesCall",
    "C# PrivacyNative public method surface",
  );
  assert.deepEqual(
    namesFromMatches(
      csharpBridge,
      /public\s+static\s+[A-Za-z0-9_<>,\[\]\s]+\s+([A-Za-z][A-Za-z0-9_]*)\s*\(/g,
    ),
    EXPECTED_CSHARP_PRIVACY_BRIDGE_METHODS,
    "C# PrivacyNative public methods drifted",
  );
});

test("privacy FFI SDK wrappers remain binary-only and JSON-free", () => {
  const sources = [
    [
      "JS src crypto privacy FFI",
      sliceBetween(
        source("javascript/iroha_js/src/crypto.js"),
        "function hasPrivacyNativeSurface",
        "export function sm2FixtureFromSeed",
        "JS src crypto privacy FFI",
      ),
      /Buffer|Uint8Array|ArrayBuffer|bytes/,
    ],
    [
      "JS dist crypto privacy FFI",
      sliceBetween(
        source("javascript/iroha_js/dist/crypto.js"),
        "function hasPrivacyNativeSurface",
        "export function sm2FixtureFromSeed",
        "JS dist crypto privacy FFI",
      ),
      /Buffer|Uint8Array|ArrayBuffer|bytes/,
    ],
    [
      "Python crypto privacy FFI",
      sliceFrom(
        source("python/iroha_python/src/iroha_python/crypto.py"),
        "def _privacy_request_archive",
        "Python crypto privacy FFI",
      ),
      /bytes|bytearray|memoryview/,
    ],
    [
      "Swift PrivacyNativeBridge",
      source("IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift"),
      /Data/,
    ],
    [
      "Swift native bridge privacy FFI",
      sliceBetween(
        source("IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"),
        "func privacyCapabilitiesV1",
        "var canUseConnectCrypto",
        "Swift native bridge privacy FFI",
      ),
      /Data|UInt8|UnsafeMutablePointer/,
    ],
    [
      "Java Android privacy bridge",
      source("java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java"),
      /byte\[\]/,
    ],
    [
      "Kotlin JVM privacy bridge",
      source("kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt"),
      /ByteArray/,
    ],
    [
      "C# privacy bridge",
      source("csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs"),
      /byte\[\]|ReadOnlySpan<byte>/,
    ],
  ];

  for (const [label, text, binaryTypePattern] of sources) {
    assert.match(text, binaryTypePattern, `${label} must expose byte-oriented privacy archives`);
    assert.doesNotMatch(text, /\bJSON\b|JSONSerialization|System\.Text\.Json|Gson|org\.json|json\.(?:loads|dumps)|JSON\.(?:parse|stringify)/);
    assert.doesNotMatch(text, /requestJson|resultJson|payloadJson|jsonPayload|jsonResult/i);
  }
});

test("native privacy FFI hosts remain Norito-only and JSON-free", () => {
  const connectBridge = source("crates/connect_norito_bridge/src/lib.rs");
  const jsHost = source("crates/iroha_js_host/src/lib.rs");
  const pythonRust = source("python/iroha_python/iroha_python_rs/src/lib.rs");
  const disallowedJson =
    /\bJSON\b|serde_json|norito::json|json::(?:from|to|Value|Map)|from_json|to_json|request_json|result_json|payload_json/i;
  const noritoOnlySections = [
    [
      "C bridge privacy FFI",
      sliceBetween(
        connectBridge,
        "struct PrivacyProductionGateStatusV1",
        "fn parse_multisig_spec_bytes",
        "C bridge privacy FFI",
      ),
      /slice::from_raw_parts|write_privacy_payload/,
    ],
    [
      "JS NAPI privacy FFI",
      sliceBetween(
        jsHost,
        "const PRIVACY_FFI_VERSION_V1",
        "/// Result of signing a transaction via the native helper.",
        "JS NAPI privacy FFI",
      ),
      /Uint8Array|Buffer/,
    ],
    [
      "Python PyO3 privacy FFI",
      sliceBetween(
        pythonRust,
        "const PRIVACY_FFI_VERSION_V1",
        "#[pymodule]",
        "Python PyO3 privacy FFI",
      ),
      /PyBytes|&\[u8\]/,
    ],
  ];

  for (const [label, text, byteBoundaryPattern] of noritoOnlySections) {
    assert.match(text, /norito::decode_from_bytes/, `${label} must decode Norito request archives`);
    assert.match(text, /norito::to_bytes/, `${label} must encode Norito result archives`);
    assert.match(text, byteBoundaryPattern, `${label} must expose byte-oriented native boundaries`);
    assert.doesNotMatch(text, disallowedJson, `${label} must not parse or render JSON payloads`);
  }

  for (const [label, text] of [
    [
      "C bridge privacy FFI",
      sliceBetween(
        connectBridge,
        "fn write_privacy_payload",
        "unsafe fn read_privacy_request",
        "C bridge privacy output encoder",
      ),
    ],
    [
      "JS NAPI privacy FFI",
      sliceBetween(
        jsHost,
        "fn encode_privacy_archive",
        "#[napi]\n/// Return Norito V1 privacy capability records",
        "JS NAPI privacy output encoder",
      ),
    ],
    [
      "Python PyO3 privacy FFI",
      sliceBetween(
        pythonRust,
        "fn encode_privacy_archive_py",
        "#[pyfunction]\n#[pyo3(name = \"privacy_capabilities_v1\")]",
        "Python PyO3 privacy output encoder",
      ),
    ],
  ]) {
    assert.match(
      text,
      /bytes\.len\(\)\s*>\s*PRIVACY_NATIVE_ARCHIVE_MAX_BYTES/,
      `${label} must reject oversized encoded Norito output archives`,
    );
  }

  const javaPrivacyAdapter = sliceBetween(
    connectBridge,
    "fn java_privacy_capabilities_archive",
    "pub unsafe extern \"system\" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaCompactPaymentTokenProver",
    "C bridge Java privacy JNI adapter",
  );
  assert.match(javaPrivacyAdapter, /norito::to_bytes/, "Java privacy JNI must encode Norito archives");
  assert.match(javaPrivacyAdapter, /read_java_byte_array/, "Java privacy JNI must read byte-array requests");
  assert.match(javaPrivacyAdapter, /byte_array_from_slice/, "Java privacy JNI must return byte-array archives");
  assert.match(javaPrivacyAdapter, /PrivacyProofOperationV1::Build/);
  assert.match(javaPrivacyAdapter, /PrivacyProofOperationV1::Verify/);
  assert.doesNotMatch(
    javaPrivacyAdapter,
    disallowedJson,
    "Java privacy JNI adapter must not parse or render JSON payloads",
  );
});

test("native privacy FFI hosts keep planned entrypoints non-executable", () => {
  for (const [label, text] of [
    ["C bridge privacy FFI", source("crates/connect_norito_bridge/src/lib.rs")],
    ["JS NAPI privacy FFI", source("crates/iroha_js_host/src/lib.rs")],
    ["Python PyO3 privacy FFI", source("python/iroha_python/iroha_python_rs/src/lib.rs")],
  ]) {
    const supportedBody = requireMatch(
      text,
      /fn\s+privacy_entrypoint_supported\([^)]*\)\s*->\s*bool\s*\{([\s\S]*?)\n\}/,
      `${label} privacy_entrypoint_supported`,
    )[1];
    assert.match(
      supportedBody,
      /\.sdk_entrypoints\s*\n\s*\.iter\(\)/,
      `${label} executable entrypoint allowlist must use SDK entrypoints`,
    );
    assert.doesNotMatch(
      supportedBody,
      /planned_entrypoints|chain\s*\(/,
      `${label} executable entrypoint allowlist must not include planned entrypoints`,
    );
    assert.match(
      text,
      /fn\s+privacy_entrypoint_planned\([^)]*\)\s*->\s*bool\s*\{[\s\S]*?\.planned_entrypoints\s*\n\s*\.iter\(\)[\s\S]*?\}/,
      `${label} must retain a dedicated planned-entrypoint classifier`,
    );
    assert.match(
      text,
      /privacy_entrypoint_planned\(entry,\s*&request\.entrypoint\)[\s\S]*?PRIVACY_FFI_ERROR_INVALID_REQUEST[\s\S]*?planned but not executable/,
      `${label} planned entrypoints must fail as non-executable requests`,
    );
    assert.match(
      text,
      /privacy_build_proof_rejects_planned_entrypoint_before_request_validation/,
      `${label} must test planned entrypoint rejection before request validation`,
    );
  }
});

test("native privacy FFI catalogs keep algorithm rows unique and portable", () => {
  for (const [label, text] of [
    ["C bridge privacy FFI", source("crates/connect_norito_bridge/src/lib.rs")],
    ["JS NAPI privacy FFI", source("crates/iroha_js_host/src/lib.rs")],
    ["Python PyO3 privacy FFI", source("python/iroha_python/iroha_python_rs/src/lib.rs")],
  ]) {
    assert.match(
      text,
      /fn\s+privacy_proof_family_is_portable\([^)]*label:\s*&str[^)]*\)\s*->\s*bool\s*\{[\s\S]*!label\.is_empty\(\)[\s\S]*split\(\['-',\s*'\/'\]\)[\s\S]*!part\.is_empty\(\)[\s\S]*byte\.is_ascii_lowercase\(\)[\s\S]*byte\.is_ascii_digit\(\)/,
      `${label} must keep proof families aligned with public lowercase proof-family tokens`,
    );
    const backendFamilyHelper = requireMatch(
      text,
      /fn\s+privacy_vk_ref_backend_family_is_portable\([^)]*field:\s*&str[^)]*\)\s*->\s*bool\s*\{[\s\S]*?\n\}/,
      `${label} verifier-key backend family helper`,
    )[0];
    assert.ok(
      backendFamilyHelper.includes("let Some(first)") &&
        backendFamilyHelper.includes("let Some(last)") &&
        backendFamilyHelper.includes("first.is_ascii_lowercase()") &&
        backendFamilyHelper.includes("last.is_ascii_lowercase()") &&
        backendFamilyHelper.includes("byte.is_ascii_lowercase()") &&
        backendFamilyHelper.includes("byte.is_ascii_digit()") &&
        backendFamilyHelper.includes("b'-' | b'_' | b'.'"),
      `${label} must keep backend families lowercase edge-safe and portable as vk_ref backend components`,
    );
    assert.ok(
      !backendFamilyHelper.includes("is_ascii_alphanumeric()"),
      `${label} must reject uppercase backend-family aliases before vk_ref binding`,
    );
    assert.ok(
      !backendFamilyHelper.includes("b':'") && !backendFamilyHelper.includes("b'/'"),
      `${label} must reject vk_ref delimiter and path separators in backend families`,
    );
    const vkRefNameHelper = requireMatch(
      text,
      /fn\s+privacy_vk_ref_name_is_portable\([^)]*field:\s*&str[^)]*\)\s*->\s*bool\s*\{[\s\S]*?\n\}/,
      `${label} verifier-key reference name helper`,
    )[0];
    assert.ok(
      vkRefNameHelper.includes("let Some(first)") &&
        vkRefNameHelper.includes("first.is_ascii_lowercase()") &&
        vkRefNameHelper.includes("byte.is_ascii_lowercase()") &&
        vkRefNameHelper.includes("byte.is_ascii_digit()") &&
        vkRefNameHelper.includes("byte == b'_'"),
      `${label} must keep vk_ref names lowercase underscore verifier-key labels`,
    );
    assert.ok(
      !vkRefNameHelper.includes("b'-'") &&
        !vkRefNameHelper.includes("b'.'") &&
        !vkRefNameHelper.includes("is_ascii_alphanumeric()"),
      `${label} must reject dash, dot, and uppercase aliases in vk_ref names`,
    );
    const algorithmIdHelper = requireMatch(
      text,
      /fn\s+privacy_algorithm_id_is_portable\([^)]*field:\s*&str[^)]*\)\s*->\s*bool\s*\{[\s\S]*?\n\}/,
      `${label} algorithm id helper`,
    )[0];
    assert.ok(
      algorithmIdHelper.includes("let Some(first)") &&
        algorithmIdHelper.includes("let Some(last)") &&
        algorithmIdHelper.includes("first.is_ascii_lowercase()") &&
        algorithmIdHelper.includes("first.is_ascii_digit()") &&
        algorithmIdHelper.includes("last.is_ascii_lowercase()") &&
        algorithmIdHelper.includes("last.is_ascii_digit()") &&
        algorithmIdHelper.includes("byte.is_ascii_lowercase()") &&
        algorithmIdHelper.includes("byte.is_ascii_digit()") &&
        algorithmIdHelper.includes("b'-' | b'_'"),
      `${label} must keep native algorithm ids aligned with public catalog id edge rules`,
    );
    assert.ok(
      !algorithmIdHelper.includes("b':'") &&
        !algorithmIdHelper.includes("b'.'") &&
        !algorithmIdHelper.includes("is_ascii_alphanumeric()"),
      `${label} must reject uppercase, delimiter, and dotted algorithm ids`,
    );
    assert.match(
      text,
      /fn\s+privacy_sdk_entrypoint_is_portable\([^)]*field:\s*&str[^)]*\)\s*->\s*bool\s*\{[\s\S]*split\('\.'\)[\s\S]*first\.is_ascii_alphabetic\(\)[\s\S]*byte\.is_ascii_alphanumeric\(\)[\s\S]*byte\s*==\s*b'_'/,
      `${label} must validate SDK entrypoint names as dot-separated identifier segments`,
    );
    assert.ok(
      !requireMatch(
        text,
        /fn\s+privacy_sdk_entrypoint_is_portable\([^)]*field:\s*&str[^)]*\)\s*->\s*bool\s*\{[\s\S]*?\n\}/,
        `${label} SDK entrypoint helper`,
      )[0].includes("b'$'"),
      `${label} must reject JavaScript-only dollar identifiers in SDK entrypoints`,
    );
    assert.match(
      text,
      /fn\s+privacy_algorithm_entry_invariants_hold\([^)]*\)\s*->\s*bool\s*\{[\s\S]*privacy_algorithm_id_is_portable\(entry\.id\)[\s\S]*sdk_entrypoints[\s\S]*privacy_sdk_entrypoint_is_portable\(entrypoint\)[\s\S]*planned_entrypoints[\s\S]*privacy_sdk_entrypoint_is_portable\(entrypoint\)/,
      `${label} must apply catalog-shaped identifier checks to catalog IDs and entrypoints`,
    );
    assert.match(
      text,
      /fn\s+privacy_algorithm_entry_invariants_hold\([^)]*\)\s*->\s*bool\s*\{[\s\S]*privacy_proof_family_is_portable\(entry\.proof_family\)[\s\S]*privacy_vk_ref_backend_family_is_portable\(entry\.backend_family\)/,
      `${label} must keep verifier-key backend families request-portable`,
    );
    assert.match(
      text,
      /fn\s+privacy_algorithm_catalog_entries_are_valid\([^)]*\)\s*->\s*bool\s*\{[\s\S]*privacy_algorithm_entry_invariants_hold[\s\S]*other\.id\s*==\s*entry\.id[\s\S]*\}/,
      `${label} must reject duplicate algorithm IDs`,
    );
    assert.match(
      text,
      /debug_assert!\(privacy_algorithm_catalog_invariants_hold\(\)\)/,
      `${label} must assert catalog invariants before capabilities are emitted`,
    );
    assert.match(
      text,
      /privacy_algorithm_catalog_is_unique_portable_and_disjoint/,
      `${label} must test the checked-in catalog invariants`,
    );
    assert.match(
      text,
      /privacy_algorithm_catalog_rejects_adversarial_duplicates_and_unportable_labels[\s\S]*duplicate algorithm IDs[\s\S]*unportable algorithm id[\s\S]*delimited algorithm id[\s\S]*uppercase algorithm id[\s\S]*leading underscore algorithm id[\s\S]*leading hyphen algorithm id[\s\S]*trailing underscore algorithm id[\s\S]*trailing hyphen algorithm id[\s\S]*unportable proof family[\s\S]*uppercase proof family[\s\S]*delimited proof family[\s\S]*empty proof-family segment[\s\S]*unportable backend family[\s\S]*delimited backend family[\s\S]*uppercase backend family[\s\S]*leading separator backend family[\s\S]*trailing separator backend family[\s\S]*duplicate sdk entrypoint[\s\S]*sdk planned overlap[\s\S]*unportable entrypoint[\s\S]*delimited entrypoint[\s\S]*hyphenated entrypoint[\s\S]*dollar entrypoint[\s\S]*empty sdk entrypoint[\s\S]*empty planned entrypoint/,
      `${label} must test adversarial catalog invariant violations`,
    );
  }
});

test("native privacy FFI catalogs keep dev fixtures explicit and non-production", () => {
  for (const [label, text] of [
    ["C bridge privacy FFI", source("crates/connect_norito_bridge/src/lib.rs")],
    ["JS NAPI privacy FFI", source("crates/iroha_js_host/src/lib.rs")],
    ["Python PyO3 privacy FFI", source("python/iroha_python/iroha_python_rs/src/lib.rs")],
  ]) {
    assert.match(
      text,
      /fn\s+privacy_entrypoint_is_dev_fixture\([^)]*\)\s*->\s*bool\s*\{[\s\S]*devfixture[\s\S]*dev_fixture[\s\S]*devprooffixture[\s\S]*fixture[\s\S]*mock[\s\S]*\}/,
      `${label} must classify fixture and mock SDK entrypoints`,
    );
    assert.match(
      text,
      /fn\s+privacy_entrypoint_is_explicit_dev_fixture\([^)]*\)\s*->\s*bool\s*\{[\s\S]*devfixture[\s\S]*dev_fixture[\s\S]*devprooffixture[\s\S]*dev_proof_fixture[\s\S]*\}/,
      `${label} must require explicit DevFixture naming for local fixtures`,
    );
    assert.match(
      text,
      /fn\s+privacy_entrypoint_is_local_verifier\([^)]*\)\s*->\s*bool\s*\{[\s\S]*to_ascii_lowercase\(\)[\s\S]*starts_with\("verify"\)[\s\S]*ends_with\("locally"\)[\s\S]*ends_with\("local"\)[\s\S]*contains\("localverifier"\)[\s\S]*contains\("localonly"\)[\s\S]*\}/,
      `${label} must classify local-only verifier entrypoints`,
    );
    assert.match(
      text,
      /fn\s+privacy_entrypoint_is_proof_helper\([^)]*\)\s*->\s*bool\s*\{[\s\S]*ProofEnvelope[\s\S]*ProofWitness[\s\S]*ProofPublicInputs[\s\S]*ProofRequest[\s\S]*ProofCommitment[\s\S]*\}/,
      `${label} must classify proof helper and wrapper entrypoints`,
    );
    assert.match(
      text,
      /fn\s+privacy_entrypoint_is_production_proof_builder\([^)]*\)\s*->\s*bool\s*\{[\s\S]*starts_with\("build"\)[\s\S]*contains\("Proof"\)[\s\S]*!privacy_entrypoint_is_instruction_builder[\s\S]*!privacy_entrypoint_is_ledger_mutation[\s\S]*!privacy_entrypoint_is_proof_helper[\s\S]*!privacy_entrypoint_is_dev_fixture[\s\S]*\}/,
      `${label} must distinguish planned production proof builders from fixtures, ledger mutations, and proof helpers`,
    );
    assert.match(
      text,
      /fn\s+privacy_algorithm_entry_invariants_hold\([^)]*\)\s*->\s*bool\s*\{[\s\S]*has_local_verifier[\s\S]*has_explicit_dev_fixture[\s\S]*!privacy_entrypoint_is_dev_fixture[\s\S]*privacy_entrypoint_is_explicit_dev_fixture[\s\S]*privacy_entrypoints_include_production_proof_builder\(entry\.planned_entrypoints\)[\s\S]*\}/,
      `${label} must make fixture/local-verifier rules part of catalog invariants`,
    );
    assert.match(
      text,
      /privacy_algorithm_catalog_rejects_adversarial_fixture_and_local_verifier_entrypoints[\s\S]*buildMockProof[\s\S]*verifyShapeProofLocally[\s\S]*verifyShapeProofLocal[\s\S]*verifyShapeProofLocalVerifier[\s\S]*buildShapeDevProofFixture[\s\S]*buildShapeProductionInstruction[\s\S]*buildShapeProofEnvelope/,
      `${label} must test adversarial fixture and local-verifier catalog drift`,
    );
  }
});

test("native privacy FFI catalogs pin required production plan rows", () => {
  for (const [label, text] of [
    ["C bridge privacy FFI", source("crates/connect_norito_bridge/src/lib.rs")],
    ["JS NAPI privacy FFI", source("crates/iroha_js_host/src/lib.rs")],
    ["Python PyO3 privacy FFI", source("python/iroha_python/iroha_python_rs/src/lib.rs")],
  ]) {
    assert.match(
      text,
      /const\s+PRIVACY_REQUIRED_PRODUCTION_PLAN_ROWS:\s*&\[\(&str,\s*&str,\s*&str\)\][\s\S]*anonymous-pgc-k-out-of-n-v1[\s\S]*anonymous-pgc-k-out-of-n[\s\S]*anonymous-pgc[\s\S]*zk-ace-pq-authorization-v0[\s\S]*stark\/fri\/sha256-goldilocks[\s\S]*stark-fri[\s\S]*orchard-halo2-actions-v1[\s\S]*halo2-pasta-action-bundle[\s\S]*halo2-ipa-orchard[\s\S]*pq-masp-stark-v0[\s\S]*stark-fri[\s\S]*pq-masp-stark-fri/,
      `${label} must pin the required production privacy plan rows with proof and backend families`,
    );
    assert.match(
      text,
      /fn\s+privacy_required_production_plan_rows_are_present\([^)]*\)\s*->\s*bool\s*\{[\s\S]*PRIVACY_REQUIRED_PRODUCTION_PLAN_ROWS[\s\S]*entry\.id\s*==\s*\*algorithm_id[\s\S]*entry\.proof_family\s*==\s*\*proof_family[\s\S]*entry\.backend_family\s*==\s*\*backend_family[\s\S]*privacy_entrypoints_include_production_proof_builder\s*\(\s*entry\.planned_entrypoints\s*,?\s*\)[\s\S]*\}/,
      `${label} must validate required row proof families, backend families, and planned production proof builders`,
    );
    assert.match(
      text,
      /fn\s+privacy_algorithm_catalog_invariants_hold\(\)\s*->\s*bool\s*\{[\s\S]*privacy_algorithm_catalog_entries_are_valid\(PRIVACY_ALGORITHM_ENTRIES\)[\s\S]*privacy_required_production_plan_rows_are_present\(PRIVACY_ALGORITHM_ENTRIES\)[\s\S]*\}/,
      `${label} must include required rows in checked-in catalog invariants`,
    );
    assert.match(
      text,
      /privacy_algorithm_catalog_rejects_missing_or_misregistered_required_plan_rows[\s\S]*deriveOrchardWitness[\s\S]*ProofEnvelope[\s\S]*anonymous-pgc-k-out-of-n-v1[\s\S]*wrong-backend[\s\S]*wrong-proof[\s\S]*planned_entrypoints\s*=\s*&\[\][\s\S]*planned production proof builder/,
      `${label} must test missing, proof-drifted, backend-drifted, unplanned, helper-only, and proof-helper required rows`,
    );
  }
});

test("native privacy FFI catalogs require explicit verifier-key name maps", () => {
  for (const [label, text] of [
    ["C bridge privacy FFI", source("crates/connect_norito_bridge/src/lib.rs")],
    ["JS NAPI privacy FFI", source("crates/iroha_js_host/src/lib.rs")],
    ["Python PyO3 privacy FFI", source("python/iroha_python/iroha_python_rs/src/lib.rs")],
  ]) {
    assert.match(
      text,
      /fn\s+privacy_catalog_vk_ref_name_is_registered\([^)]*entry:\s*&PrivacyAlgorithmEntry[^)]*\)\s*->\s*bool\s*\{[\s\S]*privacy_catalog_vk_ref_name\(entry\)[\s\S]*name\s*!=\s*"unknown"[\s\S]*privacy_vk_ref_name_is_portable\(name\)/,
      `${label} must reject catalog rows without explicit verifier-key name mappings`,
    );
    assert.match(
      text,
      /fn\s+privacy_algorithm_entry_invariants_hold\([^)]*entry:\s*&PrivacyAlgorithmEntry[^)]*\)\s*->\s*bool[\s\S]*privacy_catalog_vk_ref_name_is_registered\(entry\)/,
      `${label} must make verifier-key name registration part of row invariants`,
    );
    assert.match(
      text,
      /fn\s+privacy_algorithm_catalog_entries_are_valid\([^)]*\)\s*->\s*bool\s*\{[\s\S]*entries\.iter\(\)\.all\(privacy_algorithm_entry_invariants_hold\)[\s\S]*!privacy_algorithm_catalog_vk_ref_names_have_duplicates\(entries\)/,
      `${label} must reject duplicate verifier-key names across catalog rows`,
    );
    assert.match(
      text,
      /privacy_algorithm_catalog_rejects_missing_verifier_key_name_mappings(?=[\s\S]*all\(privacy_catalog_vk_ref_name_is_registered\))(?=[\s\S]*privacy_algorithm_catalog_vk_ref_names_have_duplicates\(PRIVACY_ALGORITHM_ENTRIES\))(?=[\s\S]*unmapped-mainnet-privacy-row-v1)(?=[\s\S]*buildUnmappedPrivacyProofV1)(?=[\s\S]*unmapped verifier-key names must fail catalog admission)/,
      `${label} must test missing verifier-key map admission failures`,
    );
  }
});

test("native privacy FFI catalog rows match public SDK catalogs", () => {
  const jsSourceRows = publicPrivacyCatalogNativeRows(getSrcPrivacyAlgorithmDescriptors());
  const jsDistRows = publicPrivacyCatalogNativeRows(getDistPrivacyAlgorithmDescriptors());
  const pythonRows = publicPrivacyCatalogNativeRows(loadPythonPrivacyAlgorithmDescriptors(), {
    pythonShape: true,
  });

  assert.ok(jsSourceRows.length > 0, "JS source privacy catalog must expose algorithm rows");
  assert.deepEqual(
    jsDistRows,
    jsSourceRows,
    "JS dist privacy catalog rows must match source catalog",
  );
  assert.deepEqual(
    pythonRows,
    jsSourceRows,
    "Python privacy catalog rows must match JS source catalog",
  );

  for (const [label, text] of [
    ["C bridge privacy FFI", source("crates/connect_norito_bridge/src/lib.rs")],
    ["JS NAPI privacy FFI", source("crates/iroha_js_host/src/lib.rs")],
    ["Python PyO3 privacy FFI", source("python/iroha_python/iroha_python_rs/src/lib.rs")],
  ]) {
    assert.deepEqual(
      extractNativePrivacyCatalogRows(text, label),
      jsSourceRows,
      `${label} native privacy catalog rows must match public SDK row ids, proof/backend families, and entrypoints`,
    );
  }
});

test("native privacy FFI verifier-key maps match public SDK catalogs", () => {
  const jsSourceEntries = publicProofedVerifierKeyEntries(getSrcPrivacyAlgorithmDescriptors());
  const jsDistEntries = publicProofedVerifierKeyEntries(getDistPrivacyAlgorithmDescriptors());
  const pythonEntries = publicProofedVerifierKeyEntries(loadPythonPrivacyAlgorithmDescriptors(), {
    pythonShape: true,
  });

  assert.ok(jsSourceEntries.length > 0, "JS source privacy catalog must expose proofed rows");
  assert.deepEqual(
    jsDistEntries,
    jsSourceEntries,
    "JS dist privacy verifier-key ids must match source catalog",
  );
  assert.deepEqual(
    pythonEntries,
    jsSourceEntries,
    "Python privacy verifier-key ids must match JS source catalog",
  );

  for (const [label, text] of [
    ["C bridge privacy FFI", source("crates/connect_norito_bridge/src/lib.rs")],
    ["JS NAPI privacy FFI", source("crates/iroha_js_host/src/lib.rs")],
    ["Python PyO3 privacy FFI", source("python/iroha_python/iroha_python_rs/src/lib.rs")],
  ]) {
    const nativeMap = extractNativePrivacyVerifierKeyNameMap(text, label);
    const nativeEntries = jsSourceEntries.map(([algorithmId]) => [
      algorithmId,
      nativeMap.get(algorithmId) ?? null,
    ]);

    assert.deepEqual(
      nativeEntries,
      jsSourceEntries,
      `${label} native verifier-key map must match public SDK verifierKeyId values`,
    );
  }
});

test("native privacy FFI catalogs keep component rows proof-only", () => {
  for (const [label, text] of [
    ["C bridge privacy FFI", source("crates/connect_norito_bridge/src/lib.rs")],
    ["JS NAPI privacy FFI", source("crates/iroha_js_host/src/lib.rs")],
    ["Python PyO3 privacy FFI", source("python/iroha_python/iroha_python_rs/src/lib.rs")],
  ]) {
    assert.match(
      text,
      /const\s+PRIVACY_COMPONENT_ALGORITHM_IDS:\s*&\[&str\]\s*=\s*&\[[\s\S]*verange-transparent-range-v1[\s\S]*\]/,
      `${label} must pin proof-only component algorithm IDs`,
    );
    assert.match(
      text,
      /fn\s+privacy_entrypoint_is_ledger_mutation\([^)]*\)\s*->\s*bool\s*\{[\s\S]*ends_with\("Instruction"\)[\s\S]*ends_with\("Transaction"\)[\s\S]*contains\("Submit"\)[\s\S]*\}/,
      `${label} must classify ledger-mutating SDK entrypoints`,
    );
    assert.match(
      text,
      /fn\s+privacy_algorithm_entry_invariants_hold\([^)]*\)\s*->\s*bool\s*\{[\s\S]*privacy_algorithm_entry_is_component[\s\S]*!privacy_entrypoints_include_ledger_mutation\(entry\.sdk_entrypoints\)[\s\S]*!privacy_entrypoints_include_ledger_mutation\(entry\.planned_entrypoints\)[\s\S]*\}/,
      `${label} must enforce proof-only component catalog invariants`,
    );
    assert.match(
      text,
      /privacy_algorithm_catalog_rejects_component_ledger_mutation_entrypoints[\s\S]*buildVeRangeInstruction[\s\S]*Iroha\.Privacy\.buildVeRangeInstruction[\s\S]*buildVeRangeTransaction[\s\S]*buildSubmitVeRangeProof/,
      `${label} must test adversarial component ledger-mutation entrypoints`,
    );
  }
});

test("native privacy FFI catalogs pair planned ledger mutations with production proof builders", () => {
  for (const [label, text] of [
    ["C bridge privacy FFI", source("crates/connect_norito_bridge/src/lib.rs")],
    ["JS NAPI privacy FFI", source("crates/iroha_js_host/src/lib.rs")],
    ["Python PyO3 privacy FFI", source("python/iroha_python/iroha_python_rs/src/lib.rs")],
  ]) {
    assert.match(
      text,
      /fn\s+privacy_entrypoints_include_production_proof_builder\([^)]*\)\s*->\s*bool\s*\{[\s\S]*privacy_entrypoint_is_production_proof_builder/,
      `${label} must expose a production proof-builder slice helper`,
    );
    assert.match(
      text,
      /fn\s+privacy_entrypoint_is_production_proof_builder\([^)]*\)\s*->\s*bool\s*\{[\s\S]*contains\("Proof"\)[\s\S]*!privacy_entrypoint_is_instruction_builder\(entrypoint\)[\s\S]*!privacy_entrypoint_is_ledger_mutation\(entrypoint\)[\s\S]*!privacy_entrypoint_is_dev_fixture\(entrypoint\)[\s\S]*\}/,
      `${label} must not classify ledger mutations or fixtures as production proof builders`,
    );
    assert.match(
      text,
      /fn\s+privacy_algorithm_entry_invariants_hold\([^)]*\)\s*->\s*bool\s*\{[\s\S]*has_planned_ledger_mutation[\s\S]*has_production_proof_builder[\s\S]*!has_planned_ledger_mutation\s*\|\|\s*has_production_proof_builder[\s\S]*\}/,
      `${label} must require production proof builders for planned ledger mutations`,
    );
    assert.match(
      text,
      /privacy_algorithm_catalog_rejects_planned_ledger_mutation_without_production_proof_builder[\s\S]*buildShapeTransferInstruction[\s\S]*buildShapeAuthorizedTransaction[\s\S]*buildSubmitShapeProof[\s\S]*buildShapeProofV1/,
      `${label} must test adversarial planned ledger mutations and a paired proof case`,
    );
  }
});

test("native privacy FFI catalogs keep proofed SDK ledger mutations typed and proof-paired", () => {
  for (const [label, text] of [
    ["C bridge privacy FFI", source("crates/connect_norito_bridge/src/lib.rs")],
    ["JS NAPI privacy FFI", source("crates/iroha_js_host/src/lib.rs")],
    ["Python PyO3 privacy FFI", source("python/iroha_python/iroha_python_rs/src/lib.rs")],
  ]) {
    assert.match(
      text,
      /fn\s+privacy_entrypoint_is_generic_ledger_mutation\([^)]*\)\s*->\s*bool\s*\{[\s\S]*buildTransaction[\s\S]*submitSignedTransaction[\s\S]*\}/,
      `${label} must classify generic transaction/submission entrypoints`,
    );
    assert.match(
      text,
      /fn\s+privacy_entrypoint_is_untyped_ledger_mutation\([^)]*\)\s*->\s*bool\s*\{[\s\S]*privacy_entrypoint_is_ledger_mutation\(entrypoint\)[\s\S]*!name\.ends_with\("Instruction"\)[\s\S]*!name\.ends_with\("Transaction"\)[\s\S]*\}/,
      `${label} must reject untyped submit-shaped ledger mutations`,
    );
    assert.match(
      text,
      /fn\s+privacy_algorithm_entry_is_proofed_privacy\([^)]*\)\s*->\s*bool\s*\{[\s\S]*proof_family\s*!=\s*"none"[\s\S]*proof_family\s*!=\s*"commitment-only"[\s\S]*\}/,
      `${label} must scope SDK ledger proof-pairing to proofed privacy rows`,
    );
    assert.match(
      text,
      /fn\s+privacy_algorithm_entry_invariants_hold\([^)]*\)\s*->\s*bool\s*\{[\s\S]*has_sdk_ledger_mutation[\s\S]*proofed_privacy_row[\s\S]*has_generic_ledger_mutation[\s\S]*has_untyped_ledger_mutation[\s\S]*!proofed_privacy_row\s*\|\|\s*!has_sdk_ledger_mutation\s*\|\|\s*has_production_proof_builder[\s\S]*!proofed_privacy_row\s*\|\|\s*!has_generic_ledger_mutation[\s\S]*!proofed_privacy_row\s*\|\|\s*!has_untyped_ledger_mutation[\s\S]*\}/,
      `${label} must keep proofed SDK ledger mutations typed and proof-paired`,
    );
    assert.match(
      text,
      /privacy_algorithm_catalog_rejects_unpaired_or_generic_sdk_ledger_mutations(?=[\s\S]*transparent-transfer)(?=[\s\S]*confidential-transfer-v2)(?=[\s\S]*SDK_TYPED_INSTRUCTION_WITH_PROOF)(?=[\s\S]*proofed sdk instruction without proof)(?=[\s\S]*Iroha\.Privacy\.submitSignedTransaction)(?=[\s\S]*buildSubmitShapeProof)/,
      `${label} must test proofed SDK ledger mutation pairing and typed-ISI failures`,
    );
  }
});

test("native privacy FFI capabilities keep production gates fail-closed", () => {
  for (const [label, text] of [
    ["C bridge privacy FFI", source("crates/connect_norito_bridge/src/lib.rs")],
    ["JS NAPI privacy FFI", source("crates/iroha_js_host/src/lib.rs")],
    ["Python PyO3 privacy FFI", source("python/iroha_python/iroha_python_rs/src/lib.rs")],
  ]) {
    assert.match(
      text,
      /const\s+PRIVACY_PRODUCTION_GATE_MISSING_ENGINE[\s\S]*real protocol engine is not production-enabled[\s\S]*const\s+PRIVACY_PRODUCTION_GATE_MISSING_ALLOWLIST[\s\S]*Iroha production allowlist is not enabled for this audited row/,
      `${label} must name mandatory production-gate missing evidence`,
    );
    assert.match(
      text,
      /fn\s+privacy_production_gate_key_is_required\([^)]*\)\s*->\s*bool\s*\{[\s\S]*PRIVACY_PRODUCTION_GATE_REQUIREMENTS[\s\S]*required_key[\s\S]*\}/,
      `${label} must classify allowed production gate keys`,
    );
    assert.match(
      text,
      /fn\s+privacy_production_gate_missing_reason_is_required\([^)]*\)\s*->\s*bool\s*\{[\s\S]*PRIVACY_PRODUCTION_GATE_REQUIREMENTS[\s\S]*PRIVACY_PRODUCTION_GATE_MISSING_ENGINE[\s\S]*PRIVACY_PRODUCTION_GATE_MISSING_ALLOWLIST[\s\S]*\}/,
      `${label} must classify allowed production gate missing reasons`,
    );
    assert.match(
      text,
      /fn\s+privacy_gate_statuses_match_requirements\([^)]*\)\s*->\s*bool\s*\{[\s\S]*zip\(PRIVACY_PRODUCTION_GATE_REQUIREMENTS\.iter\(\)\)[\s\S]*status\.key\.as_str\(\)\s*==\s*\*key[\s\S]*!status\.passed[\s\S]*\}/,
      `${label} must require deterministic production gate status ordering`,
    );
    assert.match(
      text,
      /fn\s+privacy_gate_missing_reasons_match_requirements\([^)]*\)\s*->\s*bool\s*\{[\s\S]*take\(PRIVACY_PRODUCTION_GATE_REQUIREMENTS\.len\(\)\)[\s\S]*missing\.as_str\(\)\s*==\s*\*label[\s\S]*PRIVACY_PRODUCTION_GATE_MISSING_ENGINE[\s\S]*PRIVACY_PRODUCTION_GATE_MISSING_ALLOWLIST[\s\S]*\}/,
      `${label} must require deterministic production gate missing-reason ordering`,
    );
    assert.match(
      text,
      /fn\s+privacy_production_gate_invariants_hold\([^)]*\)\s*->\s*bool\s*\{[\s\S]*!gate\.ready[\s\S]*audit_references\.is_empty\(\)[\s\S]*PRIVACY_PRODUCTION_GATE_REQUIREMENTS\.len\(\)[\s\S]*privacy_gate_statuses_match_requirements[\s\S]*privacy_gate_missing_reasons_match_requirements[\s\S]*!privacy_gate_status_keys_have_duplicates[\s\S]*!privacy_string_vec_has_duplicates[\s\S]*privacy_production_gate_key_is_required[\s\S]*!status\.passed[\s\S]*privacy_production_gate_missing_reason_is_required[\s\S]*PRIVACY_PRODUCTION_GATE_MISSING_ENGINE[\s\S]*PRIVACY_PRODUCTION_GATE_MISSING_ALLOWLIST[\s\S]*\}/,
      `${label} must validate fail-closed production gate state`,
    );
    assert.match(
      text,
      /fn\s+privacy_capability_invariants_hold\([^)]*\)\s*->\s*bool\s*\{[\s\S]*privacy_algorithm_entry[\s\S]*privacy_string_vec_matches_slice[\s\S]*!capability\.production_ready[\s\S]*privacy_production_gate_invariants_hold/,
      `${label} must validate each advertised privacy capability row`,
    );
    assert.match(
      text,
      /fn\s+privacy_capability_invariants_hold\([^)]*\)\s*->\s*bool\s*\{[\s\S]*privacy_algorithm_id_is_portable\(&capability\.algorithm_id\)[\s\S]*sdk_entrypoints[\s\S]*privacy_sdk_entrypoint_is_portable\(entrypoint\)[\s\S]*planned_entrypoints[\s\S]*privacy_sdk_entrypoint_is_portable\(entrypoint\)/,
      `${label} must keep emitted privacy capability identifiers catalog-shaped`,
    );
    assert.match(
      text,
      /fn\s+privacy_capability_invariants_hold\([^)]*\)\s*->\s*bool\s*\{[\s\S]*privacy_proof_family_is_portable\(&capability\.proof_family\)[\s\S]*privacy_vk_ref_backend_family_is_portable\(&capability\.backend_family\)/,
      `${label} must keep emitted verifier-key backend families request-portable`,
    );
    assert.match(
      text,
      /privacy_capability_invariants_reject_forged_production_readiness[\s\S]*uppercase proof family[\s\S]*delimited proof family[\s\S]*empty proof-family segment/,
      `${label} must test malformed proof-family capability labels`,
    );
    assert.match(
      text,
      /const\s+PRIVACY_EXPOSED_PRODUCTION_CLAIM_FRAGMENTS:\s*&\[&str\][\s\S]*productionready[\s\S]*productionclaim[\s\S]*claimedproduction[\s\S]*mainnetready[\s\S]*mainnetclaim[\s\S]*auditedproduction[\s\S]*auditsignoff[\s\S]*claimedaudit[\s\S]*securityreviewpassed/,
      `${label} must define native exposed-label production-claim fragments`,
    );
    assert.match(
      text,
      /fn\s+privacy_exposed_label_claims_production_readiness\([^)]*\)\s*->\s*bool\s*\{[\s\S]*PRIVACY_EXPOSED_PRODUCTION_CLAIM_FRAGMENTS[\s\S]*compact\.contains\(fragment\)[\s\S]*\}/,
      `${label} must reject production-ready/mainnet/audit claims in exposed native labels`,
    );
    assert.match(
      text,
      /fn\s+privacy_algorithm_entry_invariants_hold\([^)]*\)\s*->\s*bool\s*\{[\s\S]*!privacy_exposed_label_claims_production_readiness\(entry\.id\)[\s\S]*!privacy_exposed_label_claims_production_readiness\(entry\.proof_family\)[\s\S]*!privacy_exposed_label_claims_production_readiness\(entry\.backend_family\)[\s\S]*sdk_entrypoints[\s\S]*privacy_exposed_label_claims_production_readiness\(entrypoint\)[\s\S]*planned_entrypoints[\s\S]*privacy_exposed_label_claims_production_readiness\(entrypoint\)/,
      `${label} must keep native privacy catalog rows free of production-claim labels`,
    );
    assert.match(
      text,
      /fn\s+privacy_capability_invariants_hold\([^)]*\)\s*->\s*bool\s*\{[\s\S]*!privacy_exposed_label_claims_production_readiness\(&capability\.algorithm_id\)[\s\S]*!privacy_exposed_label_claims_production_readiness\(&capability\.proof_family\)[\s\S]*!privacy_exposed_label_claims_production_readiness\(&capability\.backend_family\)[\s\S]*sdk_entrypoints[\s\S]*!privacy_exposed_label_claims_production_readiness\(entrypoint\)[\s\S]*planned_entrypoints[\s\S]*!privacy_exposed_label_claims_production_readiness\(entrypoint\)/,
      `${label} must keep emitted native privacy capabilities free of production-claim labels`,
    );
    assert.match(
      text,
      /fn\s+privacy_capability_rows_match_catalog_order\([^)]*\)\s*->\s*bool\s*\{[\s\S]*zip\(PRIVACY_ALGORITHM_ENTRIES\.iter\(\)\)[\s\S]*algorithm\.algorithm_id\.as_str\(\)\s*==\s*entry\.id[\s\S]*\}/,
      `${label} must require deterministic privacy capability row ordering`,
    );
    assert.match(
      text,
      /fn\s+privacy_capabilities_invariants_hold\([^)]*\)\s*->\s*bool\s*\{[\s\S]*version\s*==\s*PRIVACY_FFI_VERSION_V1[\s\S]*gate_version\s*==\s*PRIVACY_PRODUCTION_GATE_VERSION[\s\S]*privacy_capability_rows_match_catalog_order[\s\S]*privacy_capability_invariants_hold[\s\S]*other\.algorithm_id\.as_str\(\)\s*==\s*algorithm\.algorithm_id\.as_str\(\)/,
      `${label} must validate the full capability archive`,
    );
    assert.match(
      text,
      /debug_assert!\(privacy_capabilities_invariants_hold\(&capabilities\)\)/,
      `${label} must assert capability invariants before capabilities are emitted`,
    );
    assert.match(
      text,
      /privacy_capabilities_result_invariants_are_fail_closed[\s\S]*privacy_capabilities_invariants_hold/,
      `${label} must test emitted fail-closed capability invariants`,
    );
    assert.match(
      text,
      /privacy_capability_invariants_reject_forged_production_readiness[\s\S]*production_ready\s*=\s*true[\s\S]*production_gate\.ready\s*=\s*true[\s\S]*\.passed\s*=\s*true[\s\S]*shadow_gate[\s\S]*shadow gate[\s\S]*shuffled production gate key order[\s\S]*audit:\/\/forged[\s\S]*external audit signoff is missing[\s\S]*PRIVACY_PRODUCTION_GATE_MISSING_ENGINE[\s\S]*PRIVACY_PRODUCTION_GATE_MISSING_ALLOWLIST[\s\S]*shuffled production-gate missing reasons[\s\S]*external audit signoff passed without evidence[\s\S]*buildShadowProductionProof/,
      `${label} must test adversarial forged production readiness cases`,
    );
    assert.match(
      text,
      /privacy_capability_invariants_reject_forged_production_readiness[\s\S]*halo2-production-ready[\s\S]*audit-signoff-pasta[\s\S]*buildMainnetReadyProof[\s\S]*claimed-mainnet-row[\s\S]*buildClaimedAuditProof/,
      `${label} must test production-ready/mainnet/audit exposed-label claims`,
    );
    assert.match(
      text,
      /privacy_capabilities_invariants_reject_bad_versions_and_duplicate_rows[\s\S]*PRIVACY_FFI_VERSION_V1\s*\+\s*1[\s\S]*privacy-production-gate-v2[\s\S]*shuffled algorithm capability rows[\s\S]*duplicate algorithm capability rows/,
      `${label} must test capability archive version and duplicate-row attacks`,
    );
  }
});

test("native privacy FFI production-disabled responses enumerate all gates", () => {
  for (const [label, text] of [
    ["C bridge privacy FFI", source("crates/connect_norito_bridge/src/lib.rs")],
    ["JS NAPI privacy FFI", source("crates/iroha_js_host/src/lib.rs")],
    ["Python PyO3 privacy FFI", source("python/iroha_python/iroha_python_rs/src/lib.rs")],
  ]) {
    assert.match(
      text,
      /const\s+PRIVACY_PRODUCTION_DISABLED_MESSAGE:\s*&str\s*=\s*"[^"]*exact protocol implementation[^"]*real proving[^"]*real verification[^"]*chain admission[^"]*cross-SDK parity[^"]*wallet\/state support[^"]*deterministic tests[^"]*fuzzing[^"]*performance gates[^"]*external audit[^"]*real protocol engine[^"]*Iroha production allowlist[^"]*"/,
      `${label} must enumerate every production-disabled gate in the public result message`,
    );
    assert.match(
      text,
      /privacy_failure_result\(\s*PRIVACY_FFI_ERROR_PRODUCTION_DISABLED,\s*PRIVACY_PRODUCTION_DISABLED_MESSAGE,\s*Some\(request\),\s*\)/,
      `${label} must use the shared production-disabled message constant`,
    );
    assert.match(
      text,
      /privacy_build_proof_rejects_supported_algorithm_until(?:_production)?_gate_passes[\s\S]*(?:iroha_privacy_build_proof_v1|PrivacyProofOperationV1::Build)[\s\S]*PRIVACY_FFI_ERROR_PRODUCTION_DISABLED[\s\S]*for fragment in \[[\s\S]*"exact protocol implementation"[\s\S]*"real proving"[\s\S]*"real verification"[\s\S]*"chain admission"[\s\S]*"cross-SDK parity"[\s\S]*"wallet\/state support"[\s\S]*"deterministic tests"[\s\S]*"fuzzing"[\s\S]*"performance gates"[\s\S]*"external audit"[\s\S]*"real protocol engine"[\s\S]*"Iroha production allowlist"[\s\S]*result\.message\.contains\(fragment\)[\s\S]*!result\.message\.contains\("secret"\)/,
      `${label} must test that production-disabled build results name every gate without witness leakage`,
    );
    assert.match(
      text,
      /privacy_verify_proof_rejects_supported_algorithm_until(?:_production)?_gate_passes[\s\S]*for fragment in \[[\s\S]*"exact protocol implementation"[\s\S]*"real proving"[\s\S]*"real verification"[\s\S]*"chain admission"[\s\S]*"cross-SDK parity"[\s\S]*"wallet\/state support"[\s\S]*"deterministic tests"[\s\S]*"fuzzing"[\s\S]*"performance gates"[\s\S]*"external audit"[\s\S]*"real protocol engine"[\s\S]*"Iroha production allowlist"[\s\S]*result\.message\.contains\(fragment\)/,
      `${label} must test that production-disabled proof results name every gate`,
    );
  }
});

test("native privacy FFI hosts reject proof/witness operation confusion before production gate", () => {
  for (const [label, text] of [
    ["C bridge privacy FFI", source("crates/connect_norito_bridge/src/lib.rs")],
    ["JS NAPI privacy FFI", source("crates/iroha_js_host/src/lib.rs")],
    ["Python PyO3 privacy FFI", source("python/iroha_python/iroha_python_rs/src/lib.rs")],
  ]) {
    assert.match(
      text,
      /operation\s*==\s*PrivacyProofOperationV1::Build\s*&&\s*!request\.proof\.is_empty\(\)[\s\S]*privacy proof build request must not include proof bytes/,
      `${label} must reject proof bytes on build requests`,
    );
    assert.match(
      text,
      /operation\s*==\s*PrivacyProofOperationV1::Verify\s*&&\s*!request\.witness\.is_empty\(\)[\s\S]*privacy proof verify request must not include witness bytes/,
      `${label} must reject witness bytes on verify requests`,
    );
    assert.match(
      text,
      /operation\s*==\s*PrivacyProofOperationV1::Build\s*&&\s*request\.witness\.is_empty\(\)[\s\S]*privacy proof build request must include witness bytes/,
      `${label} must reject missing witness bytes on build requests`,
    );
    assert.match(
      text,
      /operation\s*==\s*PrivacyProofOperationV1::Verify\s*&&\s*request\.proof\.is_empty\(\)[\s\S]*privacy proof verify request must include proof bytes/,
      `${label} must reject missing proof bytes on verify requests`,
    );
    assert.match(
      text,
      /!privacy_entrypoint_is_production_proof_builder\(&request\.entrypoint\)[\s\S]*privacy proof request entrypoint must be a production proof builder/,
      `${label} must reject non-proof SDK helpers on proof FFI requests`,
    );
    assert.match(
      text,
      /privacy_build_proof_rejects_proof_shadow_before_production_gate/,
      `${label} must test build proof-shadow rejection`,
    );
    assert.match(
      text,
      /privacy_build_proof_rejects_missing_witness_before_production_gate/,
      `${label} must test build missing-witness rejection`,
    );
    assert.match(
      text,
      /privacy_verify_proof_rejects_missing_proof_before_production_gate/,
      `${label} must test verify missing-proof rejection`,
    );
    assert.match(
      text,
      /privacy_verify_proof_rejects_witness_shadow_before_production_gate/,
      `${label} must test verify witness-shadow rejection`,
    );
    assert.match(
      text,
      /privacy_proof_ffi_rejects_non_proof_sdk_entrypoints_before_production_gate[\s\S]*buildRangeCommitment[\s\S]*buildVeRangeDevProofFixture[\s\S]*verifyVeRangeProofLocally[\s\S]*buildZkTransferInstruction[\s\S]*production proof builder/,
      `${label} must test non-proof SDK entrypoint rejection`,
    );
  }
});

test("native privacy FFI hosts reject verifier-key backend drift before production gate", () => {
  for (const [label, text] of [
    ["C bridge privacy FFI", source("crates/connect_norito_bridge/src/lib.rs")],
    ["JS NAPI privacy FFI", source("crates/iroha_js_host/src/lib.rs")],
    ["Python PyO3 privacy FFI", source("python/iroha_python/iroha_python_rs/src/lib.rs")],
  ]) {
    assert.match(
      text,
      /fn\s+privacy_vk_ref_parts\([^)]*\)\s*->\s*Option<\(&str,\s*&str\)>[\s\S]*split_once\(':'\)/,
      `${label} must parse vk_ref as backend:name`,
    );
    assert.match(
      text,
      /backend\.is_empty\(\)\s*\|\|\s*name\.is_empty\(\)\s*\|\|\s*name\.contains\(':'\)/,
      `${label} must reject malformed vk_ref backend/name separators`,
    );
    assert.match(
      text,
      /fn\s+privacy_vk_ref_is_well_formed\([^)]*vk_ref:\s*&str[^)]*\)\s*->\s*bool\s*\{[\s\S]*privacy_vk_ref_parts\(vk_ref\)[\s\S]*privacy_vk_ref_backend_family_is_portable\(backend\)[\s\S]*privacy_vk_ref_name_is_portable\(name\)/,
      `${label} must reject malformed vk_ref shapes before backend binding`,
    );
    assert.match(
      text,
      /fn\s+privacy_vk_ref_matches_backend\([^)]*entry:\s*&PrivacyAlgorithmEntry[^)]*vk_ref:\s*&str[^)]*\)[\s\S]*privacy_vk_ref_parts\(vk_ref\)[\s\S]*backend\s*==\s*entry\.backend_family/,
      `${label} must bind vk_ref backend to the algorithm backend family`,
    );
    assert.match(
      text,
      /!privacy_vk_ref_is_well_formed\(&request\.vk_ref\)[\s\S]*privacy proof request vk_ref must use backend:name with portable verifier-key components[\s\S]*None/,
      `${label} must reject malformed vk_ref without request reflection`,
    );
    assert.match(
      text,
      /!privacy_vk_ref_is_well_formed\(&request\.vk_ref\)[\s\S]*None[\s\S]*let\s+Some\(entry\)\s*=\s*privacy_algorithm_entry\(&request\.algorithm_id\)/,
      `${label} must validate vk_ref shape before catalog lookup and entrypoint binding`,
    );
    assert.match(
      text,
      /!privacy_vk_ref_matches_backend\(entry,\s*&request\.vk_ref\)[\s\S]*privacy proof request vk_ref backend must match algorithm backend family/,
      `${label} must reject wrong verifier-key backend before production-disabled dispatch`,
    );
    assert.match(
      text,
      /privacy_proof_ffi_rejects_malformed_vk_ref_without_reflection[\s\S]*missing-separator[\s\S]*empty-vk-name[\s\S]*extra-separator[\s\S]*delimited-backend[\s\S]*uppercase-backend[\s\S]*leading-separator-backend[\s\S]*trailing-separator-backend[\s\S]*uppercase-vk-name[\s\S]*dotted-vk-name[\s\S]*dashed-vk-name[\s\S]*leading-underscore-vk-name[\s\S]*backend:name/,
      `${label} must test malformed vk_ref rejection without reflection`,
    );
    assert.match(
      text,
      /privacy_proof_ffi_rejects_malformed_vk_ref_before_catalog_binding_without_reflection[\s\S]*vk-ref-order-never-echo[\s\S]*unsupported-algorithm[\s\S]*planned-entrypoint[\s\S]*unregistered-entrypoint[\s\S]*non-proof-entrypoint[\s\S]*backend:name/,
      `${label} must test malformed vk_ref rejection before catalog and entrypoint binding`,
    );
    assert.match(
      text,
      /privacy_proof_ffi_rejects_wrong_backend_vk_ref_before_production_gate[\s\S]*wrong-backend[\s\S]*vk_ref backend[\s\S]*backend family/,
      `${label} must test malformed and wrong-backend vk_ref requests`,
    );
  }
});

test("native privacy FFI hosts reject verifier-key name drift before production gate", () => {
  for (const [label, text] of [
    ["C bridge privacy FFI", source("crates/connect_norito_bridge/src/lib.rs")],
    ["JS NAPI privacy FFI", source("crates/iroha_js_host/src/lib.rs")],
    ["Python PyO3 privacy FFI", source("python/iroha_python/iroha_python_rs/src/lib.rs")],
  ]) {
    assert.match(
      text,
      /fn\s+privacy_catalog_vk_ref_name\([^)]*entry:\s*&PrivacyAlgorithmEntry[^)]*\)\s*->\s*&'static str[\s\S]*"confidential-transfer-v2"\s*=>\s*"confidential_transfer_v2"[\s\S]*"unshield"\s*=>\s*"confidential_unshield_v3"[\s\S]*"zk-ace-pq-authorization-v0"\s*=>\s*"zk_ace_pq_authorization_v0"[\s\S]*"aztec-private-rollup-v1"\s*=>\s*"aztec_private_kernel_v1"[\s\S]*"pq-masp-stark-v0"\s*=>\s*"pq_masp_stark_v0"/,
      `${label} must map native verifier-key names to the public privacy catalog`,
    );
    assert.match(
      text,
      /fn\s+privacy_canonical_vk_ref_name\([^)]*entry:\s*&PrivacyAlgorithmEntry[^)]*\)\s*->\s*String[\s\S]*privacy_catalog_vk_ref_name\(entry\)\.to_owned\(\)/,
      `${label} must derive canonical verifier-key names from the catalog map`,
    );
    assert.match(
      text,
      /fn\s+privacy_vk_ref_name_matches_algorithm\([^)]*entry:\s*&PrivacyAlgorithmEntry[^)]*vk_ref:\s*&str[^)]*\)[\s\S]*privacy_vk_ref_parts\(vk_ref\)[\s\S]*privacy_canonical_vk_ref_name\(entry\)[\s\S]*name\s*==\s*expected_name\.as_str\(\)/,
      `${label} must bind vk_ref names to the selected algorithm`,
    );
    assert.match(
      text,
      /!privacy_vk_ref_name_matches_algorithm\(entry,\s*&request\.vk_ref\)[\s\S]*privacy proof request vk_ref name must match algorithm verifier key name/,
      `${label} must reject same-backend wrong verifier-key names before production-disabled dispatch`,
    );
    assert.match(
      text,
      /privacy_proof_ffi_rejects_wrong_vk_ref_name_before_production_gate[\s\S]*generic-vk-name[\s\S]*foreign-algorithm-vk-name[\s\S]*legacy-vk-prefix[\s\S]*vk_ref name[\s\S]*algorithm verifier key/,
      `${label} must test same-backend wrong verifier-key names`,
    );
  }
});

test("native privacy FFI hosts reject empty public inputs before production gate", () => {
  for (const [label, text] of [
    ["C bridge privacy FFI", source("crates/connect_norito_bridge/src/lib.rs")],
    ["JS NAPI privacy FFI", source("crates/iroha_js_host/src/lib.rs")],
    ["Python PyO3 privacy FFI", source("python/iroha_python/iroha_python_rs/src/lib.rs")],
  ]) {
    assert.match(
      text,
      /request\.public_inputs\.is_empty\(\)[\s\S]*privacy proof request must include non-empty public_inputs/,
      `${label} must reject empty public inputs before production-disabled dispatch`,
    );
    assert.match(
      text,
      /privacy_proof_ffi_rejects_empty_public_inputs_before_production_gate[\s\S]*public_inputs[\s\S]*non-empty[\s\S]*!result\.verified/,
      `${label} must test empty public-input rejection`,
    );
  }
});

test("native privacy FFI hosts bound reflected request fields before production gate", () => {
  for (const [label, text] of [
    ["C bridge privacy FFI", source("crates/connect_norito_bridge/src/lib.rs")],
    ["JS NAPI privacy FFI", source("crates/iroha_js_host/src/lib.rs")],
    ["Python PyO3 privacy FFI", source("python/iroha_python/iroha_python_rs/src/lib.rs")],
  ]) {
    assert.match(
      text,
      /const\s+PRIVACY_REQUEST_TEXT_FIELD_MAX_BYTES:\s*usize\s*=\s*1024\s*;/,
      `${label} must bound reflected text fields`,
    );
    assert.match(
      text,
      /const\s+PRIVACY_REQUEST_PUBLIC_INPUTS_MAX_BYTES:\s*usize\s*=\s*1024\s*\*\s*1024\s*;/,
      `${label} must bound reflected public inputs`,
    );
    assert.match(
      text,
      /const\s+PRIVACY_REQUEST_WITNESS_MAX_BYTES:\s*usize\s*=\s*PRIVACY_NATIVE_ARCHIVE_MAX_BYTES\s*\/\s*2\s*;/,
      `${label} must bound witness bytes before dispatch`,
    );
    assert.match(
      text,
      /const\s+PRIVACY_REQUEST_PROOF_MAX_BYTES:\s*usize\s*=\s*PRIVACY_NATIVE_ARCHIVE_MAX_BYTES\s*\/\s*2\s*;/,
      `${label} must bound proof bytes before dispatch`,
    );
    assert.match(
      text,
      /privacy_request_has_oversized_text_field\(&request\)[\s\S]*privacy proof request text fields exceed maximum length[\s\S]*None/,
      `${label} must reject oversized text fields without request reflection`,
    );
    assert.match(
      text,
      /privacy_request_has_control_text_field\(&request\)[\s\S]*privacy proof request text fields must not contain control characters[\s\S]*None/,
      `${label} must reject control-character text fields without request reflection`,
    );
    assert.match(
      text,
      /privacy_request_has_non_ascii_text_field\(&request\)[\s\S]*privacy proof request text fields must be printable ASCII[\s\S]*None/,
      `${label} must reject non-ASCII text fields without request reflection`,
    );
    assert.match(
      text,
      /fn\s+privacy_text_field_is_portable_identifier\([^)]*\)\s*->\s*bool\s*\{[\s\S]*is_ascii_alphanumeric\(\)[\s\S]*b'-'[\s\S]*b'_'[\s\S]*b'\.'[\s\S]*b':'[\s\S]*\}/,
      `${label} must define the portable text-field alphabet`,
    );
    assert.match(
      text,
      /privacy_request_has_unportable_text_field\(&request\)[\s\S]*privacy proof request text fields must use portable identifier characters[\s\S]*None/,
      `${label} must reject unportable text-field punctuation without request reflection`,
    );
    assert.match(
      text,
      /privacy_request_has_invalid_catalog_shape\(&request\)[\s\S]*privacy proof request algorithm_id and entrypoint must use catalog identifier shapes[\s\S]*None/,
      `${label} must reject invalid algorithm_id and entrypoint shapes without request reflection`,
    );
    assert.match(
      text,
      /fn\s+privacy_request_has_exposed_production_claim_text_field\([^)]*\)\s*->\s*bool\s*\{[\s\S]*privacy_request_text_fields\(request\)[\s\S]*privacy_exposed_label_claims_production_readiness\(field\)[\s\S]*\}/,
      `${label} must define a request text-field production-claim guard`,
    );
    assert.match(
      text,
      /privacy_request_has_exposed_production_claim_text_field\(&request\)[\s\S]*privacy proof request text fields must not claim production\/mainnet\/audit readiness[\s\S]*None/,
      `${label} must reject production/mainnet/audit claim text fields without request reflection`,
    );
    assert.match(
      text,
      /request\.public_inputs\.len\(\)\s*>\s*PRIVACY_REQUEST_PUBLIC_INPUTS_MAX_BYTES[\s\S]*privacy proof request public_inputs exceeds maximum length[\s\S]*None/,
      `${label} must reject oversized public inputs without request reflection`,
    );
    assert.match(
      text,
      /request\.witness\.len\(\)\s*>\s*PRIVACY_REQUEST_WITNESS_MAX_BYTES[\s\S]*privacy proof request witness exceeds maximum length[\s\S]*None/,
      `${label} must reject oversized witnesses without request reflection`,
    );
    assert.match(
      text,
      /request\.proof\.len\(\)\s*>\s*PRIVACY_REQUEST_PROOF_MAX_BYTES[\s\S]*privacy proof request proof exceeds maximum length[\s\S]*None/,
      `${label} must reject oversized proofs without request reflection`,
    );
    assert.match(
      text,
      /privacy_request_rejects_oversized_text_fields_without_reflection/,
      `${label} must test oversized text-field rejection`,
    );
    assert.match(
      text,
      /privacy_request_rejects_control_text_fields_without_reflection/,
      `${label} must test control-character text-field rejection`,
    );
    assert.match(
      text,
      /privacy_request_rejects_non_ascii_text_fields_without_reflection[\s\S]*unicode-text-never-echo[\s\S]*printable ASCII/,
      `${label} must test non-ASCII text-field rejection without reflection`,
    );
    assert.match(
      text,
      /privacy_request_rejects_unportable_text_fields_without_reflection[\s\S]*punctuation-text-never-echo[\s\S]*portable identifier/,
      `${label} must test unportable text-field rejection without reflection`,
    );
    assert.match(
      text,
      /privacy_request_rejects_invalid_catalog_shapes_without_reflection[\s\S]*catalog-shape-text-never-echo[\s\S]*_confidential-transfer-v2[\s\S]*-confidential-transfer-v2[\s\S]*confidential-transfer-v2_[\s\S]*confidential-transfer-v2-[\s\S]*catalog identifier shapes/,
      `${label} must test invalid catalog-shape request rejection without reflection`,
    );
    assert.match(
      text,
      /privacy_request_rejects_exposed_production_claims_without_reflection[\s\S]*forged-mainnet-ready-algorithm[\s\S]*claimed-mainnet-algorithm[\s\S]*buildAuditSignoffProof[\s\S]*buildClaimedAuditProof[\s\S]*buildS\.e\.c\.u\.r\.i\.t\.yReviewPassedProof[\s\S]*externally-audited-confidential-transfer[\s\S]*audit-claim-confidential-transfer[\s\S]*production\/mainnet\/audit readiness/,
      `${label} must test production/mainnet/audit claim rejection without reflection`,
    );
    assert.match(
      text,
      /privacy_request_rejects_oversized_public_inputs_without_reflection/,
      `${label} must test public-input rejection without reflection`,
    );
    assert.match(
      text,
      /privacy_request_rejects_oversized_witness_without_reflection/,
      `${label} must test witness rejection without reflection`,
    );
    assert.match(
      text,
      /privacy_request_rejects_oversized_proof_without_reflection/,
      `${label} must test proof rejection without reflection`,
    );
  }
});

test("native privacy FFI hosts keep failure results non-successful and proof-free", () => {
  for (const [label, text] of [
    ["C bridge privacy FFI", source("crates/connect_norito_bridge/src/lib.rs")],
    ["JS NAPI privacy FFI", source("crates/iroha_js_host/src/lib.rs")],
    ["Python PyO3 privacy FFI", source("python/iroha_python/iroha_python_rs/src/lib.rs")],
  ]) {
    assert.match(
      text,
      /fn\s+privacy_failure_result_invariants_hold\([^)]*\)\s*->\s*bool\s*\{[\s\S]*result\.status\s*==\s*PRIVACY_FFI_STATUS_ERROR[\s\S]*result\.error_code\s*!=\s*0[\s\S]*result\.proof\.is_empty\(\)[\s\S]*!result\.verified[\s\S]*\}/,
      `${label} must define failure-result invariants`,
    );
    assert.match(
      text,
      /debug_assert!\(privacy_failure_result_invariants_hold\(&result\)\)/,
      `${label} must assert failure-result invariants at construction`,
    );
    assert.match(
      text,
      /fn\s+assert_privacy_result_does_not_serialize_witness\([^)]*result:\s*&PrivacyProofResultV1[^)]*witness:\s*&\[u8\][^)]*\)[\s\S]*result\.proof\.is_empty\(\)[\s\S]*privacy result message[\s\S]*Norito privacy result archive/,
      `${label} must expose a witness non-reflection assertion helper`,
    );
    assert.match(
      text,
      /privacy_failure_results_never_serialize_witness_material[\s\S]*witness-never-echo[\s\S]*groth16-bls12-377:confidential_transfer_v2[\s\S]*halo2-ipa-pasta:vk_test[\s\S]*empty_public_inputs[\s\S]*assert_privacy_result_does_not_serialize_witness/,
      `${label} must test witness non-reflection on verifier-key and public-input failures`,
    );
    assert.match(
      text,
      /privacy_failure_results_preserve_error_invariants_without_proof_reflection/,
      `${label} must test proof-free failure results`,
    );
    assert.match(
      text,
      /proof-never-echo[\s\S]*privacy_failure_result_invariants_hold\(&result\)[\s\S]*Norito privacy result archive|proof-never-echo[\s\S]*privacy_failure_result_invariants_hold\(&result\)[\s\S]*encoded privacy result/,
      `${label} must test encoded proof-marker non-reflection`,
    );
  }
});

test("mobile and C# privacy capability models stay coarse and fail-closed", () => {
  const swiftBridge = source("IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift");
  const swiftCapabilitiesBody = requireMatch(
    swiftBridge,
    /public struct PrivacyCapabilities[\s\S]*?\{([\s\S]*?)\n\}\n\npublic enum PrivacyNativeBridge/,
    "Swift PrivacyCapabilities",
  )[1];
  assert.deepEqual(
    namesFromMatches(swiftCapabilitiesBody, /public let ([A-Za-z][A-Za-z0-9_]*)\s*:/g),
    EXPECTED_SWIFT_PRIVACY_CAPABILITY_FIELDS,
    "Swift PrivacyCapabilities field shape drifted",
  );
  assert.match(swiftCapabilitiesBody, /productionReady\s*=\s*false/);
  assert.match(swiftCapabilitiesBody, /productionGate\s*=\s*\.failClosed/);
  assertNoDirectAlgorithmCapabilityFields("Swift", swiftCapabilitiesBody);

  const kotlinBridge = source(
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt",
  );
  const kotlinCapabilitiesMatch = requireMatch(
    kotlinBridge,
    /class PrivacyCapabilities private constructor\(([\s\S]*?)\)\s*\{([\s\S]*?)\n    class PrivacyProductionGate/,
    "Kotlin PrivacyCapabilities",
  );
  const kotlinCapabilitiesParams = kotlinCapabilitiesMatch[1];
  const kotlinCapabilitiesBody = `${kotlinCapabilitiesParams}\n${kotlinCapabilitiesMatch[2]}`;
  assert.deepEqual(
    namesFromMatches(kotlinCapabilitiesParams, /\bval\s+([A-Za-z][A-Za-z0-9_]*)\s*:/g),
    EXPECTED_KOTLIN_PRIVACY_CAPABILITY_FIELDS,
    "Kotlin PrivacyCapabilities field shape drifted",
  );
  assert.match(kotlinCapabilitiesBody, /productionReady\s*=\s*false/);
  assert.match(kotlinCapabilitiesBody, /productionGate\s*=\s*PrivacyProductionGate\.failClosed\(\)/);
  assertNoDirectAlgorithmCapabilityFields("Kotlin", kotlinCapabilitiesBody);

  const csharpBridge = source("csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs");
  const csharpCapabilitiesBody = requireMatch(
    csharpBridge,
    /public sealed class PrivacyCapabilities([\s\S]*?)\n}\n\npublic sealed class PrivacyProductionGate/,
    "C# PrivacyCapabilities",
  )[1];
  assert.deepEqual(
    namesFromMatches(
      csharpCapabilitiesBody,
      /public\s+[^{}\n]+\s+([A-Za-z][A-Za-z0-9_]*)\s*\{\s*get;\s*\}/g,
    ),
    EXPECTED_CSHARP_PRIVACY_CAPABILITY_FIELDS,
    "C# PrivacyCapabilities field shape drifted",
  );
  assert.match(csharpCapabilitiesBody, /productionReady:\s*false/);
  assert.match(csharpCapabilitiesBody, /PrivacyProductionGate\.FailClosed\(\)/);
  assertNoDirectAlgorithmCapabilityFields("C#", csharpCapabilitiesBody);

  const javaBridge = source(
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java",
  );
  const javaCapabilitiesBody = requireMatch(
    javaBridge,
    /public static final class PrivacyCapabilities \{([\s\S]*?)\n  \}\n\n  private static native int nativeBridgeAbiVersion/,
    "Java PrivacyCapabilities",
  )[1];
  assert.deepEqual(
    namesFromMatches(
      javaCapabilitiesBody,
      /private\s+final\s+(?:boolean|String|List<String>)\s+([A-Za-z][A-Za-z0-9_]*);/g,
    ),
    EXPECTED_JAVA_PRIVACY_CAPABILITY_FIELDS,
    "Java PrivacyCapabilities field shape drifted",
  );
  assert.match(javaCapabilitiesBody, /this\.productionReady\s*=\s*false;/);
  assert.match(javaCapabilitiesBody, /this\.missingProductionGates\s*=\s*PRODUCTION_GATE_MISSING;/);
  assert.match(javaCapabilitiesBody, /this\.auditReferences\s*=\s*PRODUCTION_GATE_AUDIT_REFERENCES;/);
  assertNoDirectAlgorithmCapabilityFields("Java", javaCapabilitiesBody);
});

test("SDK privacy production gate missing reasons stay in cross-SDK parity", () => {
  for (const [label, reasons] of [
    [
      "JS source privacy catalog",
      extractJsCatalogProductionGateMissingReasons(
        source("javascript/iroha_js/src/privacyAlgorithms.js"),
        "JS source privacy catalog",
      ),
    ],
    [
      "JS dist privacy catalog",
      extractJsCatalogProductionGateMissingReasons(
        source("javascript/iroha_js/dist/privacyAlgorithms.js"),
        "JS dist privacy catalog",
      ),
    ],
    [
      "Python privacy catalog",
      extractPythonCatalogProductionGateMissingReasons(
        source("python/iroha_python/src/iroha_python/privacy_catalog.py"),
        "Python privacy catalog",
      ),
    ],
  ]) {
    assert.deepEqual(
      reasons,
      EXPECTED_SDK_PRIVACY_PRODUCTION_GATE_MISSING_REASONS,
      `${label} production gate missing reasons drifted`,
    );
  }

  assertProductionGateMissingReasons(
    "Swift privacy native bridge",
    source("IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift"),
    /public static let missingReasons\s*=\s*\[([\s\S]*?)\n    \]/u,
  );
  assertProductionGateMissingReasons(
    "Kotlin privacy native bridge",
    source("kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt"),
    /val\s+MISSING_REASONS:\s*List<String>\s*=\s*Collections\.unmodifiableList\(\s*listOf\(([\s\S]*?)\)\s*,?\s*\)/u,
  );
  assertProductionGateMissingReasons(
    "Android Java privacy native bridge",
    source("java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java"),
    /PRODUCTION_GATE_MISSING\s*=\s*Collections\.unmodifiableList\(\s*Arrays\.asList\(([\s\S]*?)\)\);/u,
  );
  assertProductionGateMissingReasons(
    "C# privacy native bridge",
    source("csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs"),
    /public static IReadOnlyList<string> MissingReasons \{ get; \}\s*=\s*Array\.AsReadOnly\(new\[\]\s*\{([\s\S]*?)\}\);/u,
  );
});

test("privacy FFI Norito schema and proof operation set stay in parity", () => {
  for (const [label, text] of [
    ["connect_norito_bridge", source("crates/connect_norito_bridge/src/lib.rs")],
    ["iroha_js_host", source("crates/iroha_js_host/src/lib.rs")],
    ["iroha_python_rs", source("python/iroha_python/iroha_python_rs/src/lib.rs")],
  ]) {
    assertRustStructIsNorito(
      text,
      "PrivacyProductionGateStatusV1",
      EXPECTED_PRIVACY_PRODUCTION_GATE_STATUS_FIELDS,
      label,
    );
    assertRustStructIsNorito(
      text,
      "PrivacyProductionGateV1",
      EXPECTED_PRIVACY_PRODUCTION_GATE_FIELDS,
      label,
    );
    assertRustStructIsNorito(
      text,
      "PrivacyCapabilityV1",
      EXPECTED_PRIVACY_CAPABILITY_FIELDS,
      label,
    );
    assertRustStructIsNorito(
      text,
      "PrivacyCapabilitiesV1",
      EXPECTED_PRIVACY_CAPABILITIES_FIELDS,
      label,
    );
    assertRustStructIsNorito(
      text,
      "PrivacyProofRequestV1",
      EXPECTED_PRIVACY_PROOF_REQUEST_FIELDS,
      label,
    );
    assertRustStructIsNorito(
      text,
      "PrivacyProofResultV1",
      EXPECTED_PRIVACY_PROOF_RESULT_FIELDS,
      label,
    );
    assert.deepEqual(
      rustEnumVariants(text, "PrivacyProofOperationV1"),
      EXPECTED_PRIVACY_OPERATION_VARIANTS,
      `${label} privacy proof operation set drifted`,
    );
  }
});

test("native privacy FFI archives use public operation schema bytes", () => {
  const nativeHosts = [
    ["C bridge", source("crates/connect_norito_bridge/src/lib.rs")],
    ["JS NAPI host", source("crates/iroha_js_host/src/lib.rs")],
    ["Python PyO3 host", source("python/iroha_python/iroha_python_rs/src/lib.rs")],
  ];

  for (const [label, text] of nativeHosts) {
    assert.match(
      text,
      /const\s+PRIVACY_CAPABILITIES_RESULT_SCHEMA_BYTE:\s*u8\s*=\s*0x50;/,
      `${label} must pin the public capabilities result schema byte`,
    );
    assert.match(
      text,
      /const\s+PRIVACY_BUILD_PROOF_RESULT_SCHEMA_BYTE:\s*u8\s*=\s*0x42;/,
      `${label} must pin the public build-result schema byte`,
    );
    assert.match(
      text,
      /const\s+PRIVACY_VERIFY_PROOF_RESULT_SCHEMA_BYTE:\s*u8\s*=\s*0x56;/,
      `${label} must pin the public verify-result schema byte`,
    );
    assert.match(
      text,
      /const\s+PRIVACY_REQUEST_SCHEMA_BYTE:\s*u8\s*=\s*0x52;/,
      `${label} must pin the public request schema byte`,
    );
    assert.match(
      text,
      /fn\s+privacy_archive_has_repeated_schema_byte/,
      `${label} must detect public repeated-byte privacy schema hashes`,
    );
    assert.match(
      text,
      /fn\s+privacy_patch_archive_schema_hash/,
      `${label} must be able to normalize public request schemas before Rust decode`,
    );
    assert.match(
      text,
      /fn\s+privacy_patch_archive_repeated_schema_byte/,
      `${label} must patch native output archives to public schema bytes`,
    );
    assert.match(
      text,
      /fn\s+privacy_decode_public_request_archive/,
      `${label} must decode privacy proof requests through a public-schema-only helper`,
    );
    assert.match(
      text,
      /fn\s+privacy_decode_public_request_archive[\s\S]*if\s+!privacy_archive_has_repeated_schema_byte\([^,]+,\s*PRIVACY_REQUEST_SCHEMA_BYTE\)[\s\S]*return\s+Err\(\(\)\)[\s\S]*<PrivacyProofRequestV1\s+as\s+norito::NoritoSerialize>::schema_hash\(\)/,
      `${label} must reject private Rust request schemas before native privacy decode`,
    );
    assert.match(
      text,
      /<PrivacyProofRequestV1\s+as\s+norito::NoritoSerialize>::schema_hash\(\)/,
      `${label} must normalize only privacy request archives to the native Rust schema before decode`,
    );
    assert.match(
      text,
      /fn\s+privacy_result_schema_byte\([^)]*PrivacyProofOperationV1[^)]*\)\s*->\s*u8[\s\S]*PrivacyProofOperationV1::Build\s*=>\s*PRIVACY_BUILD_PROOF_RESULT_SCHEMA_BYTE[\s\S]*PrivacyProofOperationV1::Verify\s*=>\s*PRIVACY_VERIFY_PROOF_RESULT_SCHEMA_BYTE/,
      `${label} must map proof operations to public result schema bytes`,
    );
    assert.match(
      text,
      /fn\s+privacy_(?:ffi|native)_archives_use_public_schema_hashes/,
      `${label} must test public request and operation-result schema archives`,
    );
    assert.match(
      text,
      /fn\s+privacy_public_schema_request_archives_reject_operation_confusion/,
      `${label} must test operation-confusion rejection through public request-schema archives`,
    );
    assert.match(
      text,
      /fn\s+privacy_request_archives_reject_private_rust_schema_hashes/,
      `${label} must test that private Rust-schema request archives fail closed at the public FFI boundary`,
    );
    assert.match(
      text,
      /private Rust request schema must not masquerade as the public FFI request schema[\s\S]*assert_malformed_privacy_request_result/,
      `${label} must reject private-schema request archives as malformed before production dispatch`,
    );
    assert.match(
      text,
      /forged-public-build-proof-shadow[\s\S]*public_privacy_request_archive[\s\S]*PrivacyProofOperationV1::Build[\s\S]*must not include/,
      `${label} must reject proof-shadow public-schema build requests before production dispatch`,
    );
    assert.match(
      text,
      /forged-public-verify-witness-shadow[\s\S]*public_privacy_request_archive[\s\S]*PrivacyProofOperationV1::Verify[\s\S]*must not include/,
      `${label} must reject witness-shadow public-schema verify requests before production dispatch`,
    );
    assert.match(
      text,
      /missing_witness[\s\S]*public_privacy_request_archive[\s\S]*PrivacyProofOperationV1::Build[\s\S]*must include[\s\S]*missing_proof[\s\S]*public_privacy_request_archive[\s\S]*PrivacyProofOperationV1::Verify[\s\S]*must include/,
      `${label} must reject missing witness/proof public-schema requests before production dispatch`,
    );
  }

  assert.match(
    sliceBetween(
      nativeHosts[0][1],
      "fn write_privacy_payload",
      "unsafe fn read_privacy_request",
      "C bridge privacy payload writer",
    ),
    /privacy_patch_archive_repeated_schema_byte\(&mut bytes,\s*schema_byte\)/,
    "C bridge must patch every returned privacy archive to its public schema byte",
  );
  assert.match(
    sliceBetween(
      nativeHosts[0][1],
      "unsafe fn read_privacy_request",
      "unsafe fn iroha_privacy_process_request_v1",
      "C bridge privacy request reader",
    ),
    /privacy_decode_public_request_archive\(bytes\)\.map_err\(\|_\|\s*PRIVACY_FFI_ERROR_MALFORMED_NORITO\)/,
    "C bridge raw FFI reader must require public request-schema archives before native decode",
  );

  for (const [label, text] of nativeHosts.slice(1)) {
    assert.match(
      sliceBetween(
        text,
        "fn privacy_result_for_request_archive",
        label === "JS NAPI host" ? "fn encode_privacy_archive" : "fn encode_privacy_archive_py",
        `${label} privacy request decoder`,
      ),
      /privacy_archive_has_repeated_schema_byte\(request_archive,\s*PRIVACY_REQUEST_SCHEMA_BYTE\)[\s\S]*<PrivacyProofRequestV1\s+as\s+norito::NoritoSerialize>::schema_hash\(\)/,
      `${label} must accept public request-schema archives before native decode`,
    );
    assert.match(
      sliceBetween(
        text,
        label === "JS NAPI host" ? "fn encode_privacy_archive" : "fn encode_privacy_archive_py",
        label === "JS NAPI host"
          ? "#[napi]\n/// Return Norito V1 privacy capability records"
          : "#[pyfunction]\n#[pyo3(name = \"privacy_capabilities_v1\")]",
        `${label} privacy output encoder`,
      ),
      /privacy_patch_archive_repeated_schema_byte\(&mut bytes,\s*schema_byte\)/,
      `${label} must patch every returned privacy archive to its public schema byte`,
    );
  }
});

test("pending privacy backend tags stay in cross-SDK parity", () => {
  const rustBackendTags = source("crates/iroha_data_model/src/zk.rs");
  const swiftBackendTags = source("IrohaSwift/Sources/IrohaSwift/VerifyingKeyBackendTag.swift");
  const javaBackendTags = source(
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/zk/VerifyingKeyBackendTag.java",
  );
  const kotlinBackendTags = source(
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/core/model/zk/VerifyingKeyBackendTag.kt",
  );
  const csharpBackendTags = source("csharp/src/Hyperledger.Iroha.Sdk/Zk/VerifyingKeyBackendTag.cs");
  const surfaces = [
    ["Rust data-model BackendTag", rustBackendTags],
    ["JS source Norito codec", source("javascript/iroha_js/src/norito.js")],
    ["JS dist Norito codec", source("javascript/iroha_js/dist/norito.js")],
    ["JS source privacy builders", source("javascript/iroha_js/src/instructionBuilders.js")],
    ["JS dist privacy builders", source("javascript/iroha_js/dist/instructionBuilders.js")],
    ["JS TypeScript declarations", source("javascript/iroha_js/index.d.ts")],
    ["Python OpenVerify codec", source("python/iroha_python/src/iroha_python/verange.py")],
    ["Swift backend tag", swiftBackendTags],
    ["Java Android backend tag", javaBackendTags],
    ["Kotlin/JVM backend tag", kotlinBackendTags],
    ["C# backend tag", csharpBackendTags],
  ];

  for (const [label, text] of surfaces) {
    for (const backendLabel of EXPECTED_PENDING_PRIVACY_BACKEND_LABELS) {
      const compactLabel = backendLabel.replaceAll(/[-_/]/g, "");
      assert.ok(
        text.includes(backendLabel) || text.includes(compactLabel),
        `${label} must include pending privacy backend tag ${backendLabel}`,
      );
    }
  }

  const pendingClassifierSurfaces = [
    [
      "Rust data-model BackendTag",
      rustBackendTags,
      [/is_pending_production_backend\(/, /is_pending_production_backend_label\(/],
    ],
    [
      "Swift backend tag",
      swiftBackendTags,
      [/isPendingProductionBackend/, /init\(catalogLabel raw: String\)/, /isPendingProductionBackendLabel/],
    ],
    [
      "Java Android backend tag",
      javaBackendTags,
      [/isPendingProductionBackend\(\)/, /fromCatalogLabel\(final String raw\)/, /isPendingProductionBackendLabel/],
    ],
    [
      "Kotlin/JVM backend tag",
      kotlinBackendTags,
      [/isPendingProductionBackend/, /fromCatalogLabel\(raw: String\?\)/, /isPendingProductionBackendLabel/],
    ],
    [
      "C# backend tag",
      csharpBackendTags,
      [/IsPendingProductionBackend/, /FromCatalogLabel\(string\? raw\)/, /IsPendingProductionBackendLabel/],
    ],
  ];
  for (const [label, text, patterns] of pendingClassifierSurfaces) {
    for (const pattern of patterns) {
      assert.match(text, pattern, `${label} pending-production classifier drifted`);
    }
  }
});

test("native chain proof admission uses explicit production verifier backend allowlist", () => {
  const coreZk = source("crates/iroha_core/src/zk.rs");
  const worldIsi = source("crates/iroha_core/src/smartcontracts/isi/world.rs");
  const zkVerifyTests = source("crates/iroha_core/tests/zk_verify.rs");
  const jsInstructionBuilders = source("javascript/iroha_js/src/instructionBuilders.js");
  const jsInstructionBuildersDist = source("javascript/iroha_js/dist/instructionBuilders.js");
  const jsToriiClient = source("javascript/iroha_js/src/toriiClient.js");
  const jsToriiClientDist = source("javascript/iroha_js/dist/toriiClient.js");
  const jsToriiClientTests = source("javascript/iroha_js/test/toriiClient.test.js");
  const pythonClient = source("python/iroha_python/src/iroha_python/client.py");
  const pythonPrivacyBackends = source("python/iroha_python/src/iroha_python/_privacy_backends.py");
  const pythonOpenVerifyCodec = source("python/iroha_python/src/iroha_python/verange.py");
  const pythonOpenVerifyTests = source("python/iroha_python/tests/verange_test.py");
  const pythonClientTests = source("python/iroha_python/tests/client_ledger_helpers_test.py");
  const kotlinBackendTag = source(
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/core/model/zk/VerifyingKeyBackendTag.kt",
  );
  const kotlinInstructionUtils = source(
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/core/model/instructions/VerifyingKeyInstructionUtils.kt",
  );
  const kotlinRegisterVk = source(
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/core/model/instructions/RegisterVerifyingKeyInstruction.kt",
  );
  const kotlinUpdateVk = source(
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/core/model/instructions/UpdateVerifyingKeyInstruction.kt",
  );
  const kotlinVkTests = source(
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/core/model/instructions/VerifyingKeyInstructionBuildersTest.kt",
  );
  const kotlinBackendTagTests = source(
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/core/model/zk/VerifyingKeyBackendTagTest.kt",
  );
  const javaBackendTag = source(
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/zk/VerifyingKeyBackendTag.java",
  );
  const javaInstructionUtils = source(
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/VerifyingKeyInstructionUtils.java",
  );
  const javaRegisterVk = source(
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/RegisterVerifyingKeyInstruction.java",
  );
  const javaUpdateVk = source(
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/UpdateVerifyingKeyInstruction.java",
  );
  const javaVkTests = source(
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/model/instructions/VerifyingKeyInstructionUtilsTests.java",
  );
  const csharpBackendTag = source("csharp/src/Hyperledger.Iroha.Sdk/Zk/VerifyingKeyBackendTag.cs");
  const csharpVkTests = source(
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/VerifyingKeyBackendTagTests.cs",
  );
  const swiftBackendTag = source("IrohaSwift/Sources/IrohaSwift/VerifyingKeyBackendTag.swift");
  const swiftToriiClient = source("IrohaSwift/Sources/IrohaSwift/ToriiClient.swift");
  const swiftVkTests = source("IrohaSwift/Tests/IrohaSwiftTests/VerifyingKeyBackendTagTests.swift");
  const swiftToriiTests = source("IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift");

  assert.match(
    coreZk,
    /pub fn production_verify_backend_tag\([\s\S]*is_pending_production_backend_label\([\s\S]*is_production_claim_backend_label\([\s\S]*is_trusted_setup_backend_label\([\s\S]*is_developer_only_backend_label\([\s\S]*BackendTag::Stark[\s\S]*BackendTag::Halo2IpaPasta/,
    "Rust verifier dispatch must expose an explicit production backend tag allowlist",
  );
  assert.match(
    coreZk,
    /pub fn is_production_verify_backend_label\([^)]*\)\s*->\s*bool\s*\{[\s\S]*production_verify_backend_tag\(backend\)\.is_some\(\)/,
    "Rust verifier dispatch must expose a reusable production backend classifier",
  );
  assert.match(
    worldIsi,
    /fn ensure_production_verifying_key_backend_id\([^)]*\)[\s\S]*!crate::zk::is_production_verify_backend_label\(backend\)[\s\S]*unsupported verifying key backends/,
    "Verify-key registration must reject unsupported backend labels before WSV admission",
  );
  assert.match(
    worldIsi,
    /fn ensure_production_verifying_key_backend_id\([^)]*\)[\s\S]*is_production_claim_backend_label\(backend\)[\s\S]*production-claim verifying key backends/,
    "Verify-key registration must classify production-claim backend labels before unsupported fallback",
  );
  assert.match(
    worldIsi,
    /fn validate_proof_attachment[\s\S]*!crate::zk::is_production_verify_backend_label\(attachment\.backend\.as_str\(\)\)[\s\S]*unsupported_proof_backend_error/,
    "VerifyProof attachment validation must reject unsupported backend labels before registry lookup",
  );
  assert.match(
    worldIsi,
    /fn validate_proof_attachment[\s\S]*is_production_claim_backend_label\(attachment\.backend\.as_str\(\)\)[\s\S]*production_claim_proof_backend_error/,
    "VerifyProof attachment validation must classify production-claim backend labels before registry lookup",
  );
  assert.match(
    worldIsi,
    /fn open_verify_backend_tag_matches\([^)]*\)\s*->\s*bool\s*\{[\s\S]*production_verify_backend_tag\(backend\)\.is_some_and/,
    "OpenVerify tag matching must be driven by the production backend allowlist",
  );
  for (const label of [
    "production_verify_backend_allowlist_is_explicit",
    "production_claim_classifier_catches_readiness_and_audit_labels",
    "guardrails_reject_production_claim_backends_before_dispatch",
    "preverify_rejects_production_claim_backends_before_dedup",
    "preverify_rejects_production_claim_backend_labels_before_lookup",
    "register_vk_rejects_production_claim_backend_labels",
    "verify_proof_rejects_production_claim_backend_labels_before_registry_lookup",
    "guardrails_reject_unsupported_backends_before_dispatch",
    "register_vk_rejects_unsupported_backend_labels",
    "verifyproof_rejects_unsupported_backend_before_lookup",
    "halo2/unknown-native-v1",
    "halo2/ipa:production-ready",
  ]) {
    assert.ok(
      coreZk.includes(label) || worldIsi.includes(label) || zkVerifyTests.includes(label),
      `production backend allowlist tests must cover ${label}`,
    );
  }
  for (const [label, text] of [
    ["JS instruction builder source", jsInstructionBuilders],
    ["JS instruction builder dist", jsInstructionBuildersDist],
  ]) {
    assert.match(
      text,
      /function isProductionVerifyBackendLabel\([^)]*\)[\s\S]*isPendingProductionVerifierBackendLabel[\s\S]*isProductionClaimVerifierBackendLabel[\s\S]*isTrustedSetupVerifierBackendLabel[\s\S]*isDeveloperOnlyVerifierBackendLabel/,
      `${label} must keep a fail-closed production verifier backend classifier`,
    );
    assert.ok(
      text.includes("backend.trim() !== backend"),
      `${label} must reject non-canonical whitespace-mutated verifier backend labels`,
    );
    assert.match(
      text,
      /function assertProductionVerifyBackendLabel\([^)]*\)[\s\S]*unsupported production verifier backend/,
      `${label} must throw before unsupported verifier-key ids are built`,
    );
    assert.match(
      text,
      /function normalizePrivacyVerifierKeyIdFromOptions\([^)]*\)[\s\S]*assertProductionVerifyBackendLabel\(id\.backend/,
      `${label} must reject unsupported privacy verifier-key ids before instruction build`,
    );
    assert.match(
      text,
      /function normalizePrivacyBackendTag\([^)]*\)[\s\S]*isPortableVerifierBackendLabel\(raw\)/,
      `${label} must reject non-portable privacy proof-envelope backend labels before tag alias compaction`,
    );
    assert.ok(
      text.includes("return /^[A-Za-z0-9/_.:-]+$/u.test(backend);"),
      `${label} must reject spaces and plus signs in verifier backend labels before alias compaction`,
    );
  }
  for (const [label, text] of [
    ["JS Torii source", jsToriiClient],
    ["JS Torii dist", jsToriiClientDist],
  ]) {
    assert.match(
      text,
      /function isProductionVerifyBackendLabel\([^)]*\)[\s\S]*isPendingProductionVerifierBackendLabel[\s\S]*isProductionClaimVerifierBackendLabel[\s\S]*isTrustedSetupVerifierBackendLabel[\s\S]*isDeveloperOnlyVerifierBackendLabel/,
      `${label} must keep a fail-closed production verifier backend classifier`,
    );
    assert.ok(
      text.includes("backend.trim() !== backend"),
      `${label} must reject non-canonical whitespace-mutated verifier backend labels`,
    );
    assert.match(
      text,
      /function assertProductionVerifyBackendLabel\([^)]*\)[\s\S]*unsupported production verifier backend/,
      `${label} must throw before unsupported verifier-key requests are sent`,
    );
    assert.match(
      text,
      /function normalizeVerifyingKeyRegisterPayload\([^)]*\)[\s\S]*assertProductionVerifyBackendLabel\(record\.backend, "registerVerifyingKey\.backend"\)/,
      `${label} must reject unsupported registerVerifyingKey backends before fetch`,
    );
    assert.match(
      text,
      /function normalizeVerifyingKeyUpdatePayload\([^)]*\)[\s\S]*assertProductionVerifyBackendLabel\(record\.backend, "updateVerifyingKey\.backend"\)/,
      `${label} must reject unsupported updateVerifyingKey backends before fetch`,
    );
    assert.ok(
      text.includes("return /^[A-Za-z0-9/_.:-]+$/u.test(backend);"),
      `${label} must reject spaces and plus signs in verifier backend labels before request dispatch`,
    );
  }
  assert.match(
    pythonPrivacyBackends,
    /def _is_production_verify_backend_label\([^)]*\)[\s\S]*_is_pending_production_backend_label[\s\S]*_is_production_claim_backend_label[\s\S]*_is_trusted_setup_backend_label[\s\S]*_is_developer_only_backend_label/,
    "Python shared privacy backend helpers must keep a fail-closed production verifier backend classifier",
  );
  assert.ok(
    pythonPrivacyBackends.includes("backend.strip() != backend"),
    "Python shared privacy backend helpers must reject non-canonical whitespace-mutated verifier backend labels",
  );
  assert.match(
    pythonClient,
    /from \._privacy_backends import _require_production_verify_backend_label/,
    "Python Torii client must use the shared production verifier backend validator",
  );
  assert.match(
    pythonClient,
    /def submit_zk_verifying_key_registration\([^)]*\)[\s\S]*_normalize_zk_verifying_key_registration_payload\(payload\)/,
    "Python Torii verifier-key registration must validate backend before request dispatch",
  );
  assert.match(
    pythonOpenVerifyCodec,
    /def _normalize_backend\([^)]*\)[\s\S]*any\(not char\.isascii\(\) for char in text\)/,
    "Python OpenVerify proof-envelope backend tags must reject non-ASCII confusables before tag alias compaction",
  );
  assert.ok(
    pythonClientTests.includes(
      "test_zk_verifying_key_registration_rejects_unsupported_backends_before_request",
    ),
    "Python tests must cover unsupported verifier-key registration backends",
  );
  assert.match(
    kotlinBackendTag,
    /fun isProductionVerifyBackendLabel\([^)]*\)[\s\S]*isPendingProductionBackendLabel[\s\S]*isProductionClaimBackendLabel[\s\S]*isTrustedSetupBackendLabel[\s\S]*isDeveloperOnlyBackendLabel/,
    "Kotlin backend tags must expose a production verifier backend classifier",
  );
  assert.ok(
    kotlinBackendTag.includes("label.any { it.code > 0x7F }"),
    "Kotlin catalog backend labels must reject non-ASCII confusables before alias compaction",
  );
  assert.ok(
    kotlinBackendTag.includes("backend.trim() != backend"),
    "Kotlin backend tags must reject non-canonical whitespace-mutated verifier backend labels",
  );
  assert.match(
    kotlinInstructionUtils,
    /fun Map<String, String>\.productionBackend\([^)]*\)[\s\S]*requireProductionVerifyBackendLabel/,
    "Kotlin verifier-key instruction utilities must validate production backend labels",
  );
  assert.match(
    kotlinRegisterVk,
    /RegisterVerifyingKeyInstruction[\s\S]*productionBackend\(backend\)[\s\S]*arguments\.productionBackend\("backend"\)/,
    "Kotlin register verifier-key instruction must validate builder and fromArguments backends",
  );
  assert.match(
    kotlinUpdateVk,
    /UpdateVerifyingKeyInstruction[\s\S]*productionBackend\(backend\)[\s\S]*arguments\.productionBackend\("backend"\)/,
    "Kotlin update verifier-key instruction must validate constructor and fromArguments backends",
  );
  assert.ok(
    kotlinVkTests.includes("register and update reject unsupported production verifier backends"),
    "Kotlin tests must cover unsupported verifier-key instruction backends",
  );
  assert.match(
    javaBackendTag,
    /static boolean isProductionVerifyBackendLabel\([^)]*\)[\s\S]*isPendingProductionBackendLabel[\s\S]*isProductionClaimBackendLabel[\s\S]*isTrustedSetupBackendLabel[\s\S]*isDeveloperOnlyBackendLabel/,
    "Android Java backend tags must expose a production verifier backend classifier",
  );
  assert.match(
    javaBackendTag,
    /fromCatalogLabel\([^)]*\)[\s\S]*hasNonAscii\(label\)[\s\S]*private static boolean hasNonAscii/,
    "Android Java catalog backend labels must reject non-ASCII confusables before alias compaction",
  );
  assert.ok(
    javaBackendTag.includes("!trimWhitespace(backend).equals(backend)"),
    "Android Java backend tags must reject non-canonical whitespace-mutated verifier backend labels",
  );
  assert.match(
    javaInstructionUtils,
    /static String requireProductionBackend\([^)]*\)[\s\S]*requireProductionVerifyBackendLabel/,
    "Android Java verifier-key instruction utilities must validate production backend labels",
  );
  assert.match(
    javaRegisterVk,
    /RegisterVerifyingKeyInstruction[\s\S]*requireProductionBackend\(arguments, "backend"\)[\s\S]*requireProductionBackend\(backend, "backend"\)/,
    "Android Java register verifier-key instruction must validate builder and fromArguments backends",
  );
  assert.match(
    javaUpdateVk,
    /UpdateVerifyingKeyInstruction[\s\S]*requireProductionBackend\(arguments, "backend"\)[\s\S]*requireProductionBackend\(backend, "backend"\)/,
    "Android Java update verifier-key instruction must validate builder and fromArguments backends",
  );
  assert.ok(
    javaVkTests.includes("registerAndUpdateRejectUnsupportedProductionBackends"),
    "Android Java tests must cover unsupported verifier-key instruction backends",
  );
  assert.ok(
    javaBackendTag.includes("trimWhitespace") && javaInstructionUtils.includes("trimWhitespace"),
    "Android Java verifier-key backend validation must not use String.trim() for control-byte suffixes",
  );
  assert.match(
    csharpBackendTag,
    /static bool IsProductionVerifyBackendLabel\([^)]*\)[\s\S]*IsPendingProductionBackendLabel[\s\S]*IsProductionClaimBackendLabel[\s\S]*IsTrustedSetupBackendLabel[\s\S]*IsDeveloperOnlyBackendLabel/,
    "C# backend tags must expose a production verifier backend classifier",
  );
  assert.match(
    csharpBackendTag,
    /FromCatalogLabel\([^)]*\)[\s\S]*HasNonAscii\(label\)[\s\S]*private static bool HasNonAscii/,
    "C# catalog backend labels must reject non-ASCII confusables before alias compaction",
  );
  assert.match(
    csharpBackendTag,
    /var backend = raw;[\s\S]*backend\.Trim\(\) != backend/,
    "C# production verifier backend labels must preserve raw whitespace for fail-closed validation",
  );
  assert.ok(
    csharpVkTests.includes("ProductionVerifierBackendClassifierRejectsUnsafeLabels"),
    "C# tests must cover unsupported production verifier backends",
  );
  assert.match(
    swiftBackendTag,
    /static func isProductionVerifyBackendLabel\([^)]*\)[\s\S]*isPendingProductionBackendLabel[\s\S]*isProductionClaimBackendLabel[\s\S]*isTrustedSetupBackendLabel[\s\S]*isDeveloperOnlyBackendLabel/,
    "Swift backend tags must expose a production verifier backend classifier",
  );
  assert.ok(
    swiftBackendTag.includes("unicodeScalars.contains(where: { $0.value > 127 })"),
    "Swift catalog backend labels must reject non-ASCII confusables before alias compaction",
  );
  assert.ok(
    swiftBackendTag.includes("trimmingCharacters(in: .whitespacesAndNewlines) != backend"),
    "Swift backend tags must reject non-canonical whitespace-mutated verifier backend labels",
  );
  assert.match(
    swiftToriiClient,
    /enum ToriiVerifyingKeyRequestValidation[\s\S]*static func normalizedBackend\([^)]*\)[\s\S]*VerifyingKeyBackendTag\.isProductionVerifyBackendLabel/,
    "Swift Torii verifier-key requests must validate production backends before encoding",
  );
  assert.ok(
    swiftVkTests.includes("testProductionVerifierBackendClassifierRejectsUnsafeLabels"),
    "Swift tests must cover unsupported production verifier backends",
  );
  assert.ok(
    swiftToriiTests.includes("testVerifyingKeyRequestsRejectUnsupportedProductionBackendsBeforeEncoding"),
    "Swift Torii tests must cover unsupported verifier-key request backends",
  );
  for (const [label, text] of [
    ["Rust verifier dispatch", coreZk],
    ["JS instruction builder source", jsInstructionBuilders],
    ["JS instruction builder dist", jsInstructionBuildersDist],
    ["JS Torii source", jsToriiClient],
    ["JS Torii dist", jsToriiClientDist],
    ["Python shared privacy backend helpers", pythonPrivacyBackends],
    ["Kotlin backend tags", kotlinBackendTag],
    ["Android Java backend tags", javaBackendTag],
    ["C# backend tags", csharpBackendTag],
    ["Swift backend tags", swiftBackendTag],
  ]) {
    for (const marker of [
      "productionready",
      "claimedproduction",
      "mainnetready",
      "auditsignoff",
      "externallyaudited",
      "securityreviewpassed",
    ]) {
      assert.ok(text.includes(marker), `${label} must reject compact production-claim marker ${marker}`);
    }
  }
  for (const [label, text] of [
    ["Rust production backend unit tests", coreZk],
    ["Rust VerifyProof admission tests", worldIsi],
    ["Rust attachment preverify tests", zkVerifyTests],
    ["JS Torii verifier-key tests", jsToriiClientTests],
    ["Python Torii verifier-key tests", pythonClientTests],
    ["Kotlin verifier-key backend tag tests", kotlinBackendTagTests],
    ["Android Java verifier-key instruction tests", javaVkTests],
    ["C# verifier-key backend tag tests", csharpVkTests],
    ["Swift verifier-key backend tag tests", swiftVkTests],
  ]) {
    for (const marker of [
      "halo2/ipa:production-ready",
      "halo2/ipa:mainnet-ready",
      "stark/fri/audit-signoff",
      "stark/fri/S.e.c.u.r.i.t.yReviewPassed",
      "stark/fri/a-u-d-i-t-c-l-a-i-m",
    ]) {
      assert.ok(text.includes(marker), `${label} must reject production-claim backend ${marker}`);
    }
  }
  for (const [label, text, marker] of [
    ["Rust production backend unit tests", coreZk, "halo2/ipa\\0"],
    ["JS Torii verifier-key tests", jsToriiClientTests, "halo2/ipa\\0"],
    ["Python Torii verifier-key tests", pythonClientTests, "halo2/ipa\\0"],
    ["Kotlin verifier-key instruction tests", kotlinVkTests, "halo2/ipa\\u0000"],
    ["Android Java verifier-key instruction tests", javaVkTests, "'\\0'"],
    ["C# verifier-key backend tag tests", csharpVkTests, "halo2/ipa\\0"],
    ["Swift verifier-key backend tag tests", swiftVkTests, "halo2/ipa\\0"],
    ["Swift Torii verifier-key tests", swiftToriiTests, "halo2/ipa\\0"],
  ]) {
    assert.ok(text.includes(marker), `${label} must reject NUL-suffixed production backend labels`);
  }
  for (const [label, text, marker] of [
    ["Rust production backend unit tests", coreZk, '" halo2/ipa"'],
    ["JS Torii verifier-key tests", jsToriiClientTests, '" halo2/ipa"'],
    ["Python Torii verifier-key tests", pythonClientTests, '" halo2/ipa"'],
    ["Kotlin verifier-key instruction tests", kotlinVkTests, '" halo2/ipa"'],
    ["Android Java verifier-key instruction tests", javaVkTests, '" halo2/ipa"'],
    ["C# verifier-key backend tag tests", csharpVkTests, '" halo2/ipa"'],
    ["Swift verifier-key backend tag tests", swiftVkTests, '" halo2/ipa"'],
    ["Swift Torii verifier-key tests", swiftToriiTests, '" halo2/ipa"'],
  ]) {
    assert.ok(
      text.includes(marker),
      `${label} must reject whitespace-mutated production backend labels`,
    );
  }
  for (const [label, text, markers] of [
    ["Rust production backend unit tests", coreZk, ["\\u{FF0F}", "\\u{200B}", "\\u{0430}"]],
    ["JS instruction builder tests", source("javascript/iroha_js/test/instructionBuilders.test.js"), ["\\uFF0F", "\\u200B", "\\u0430"]],
    ["JS Torii verifier-key tests", jsToriiClientTests, ["\\uFF0F", "\\u200B", "\\u0430"]],
    ["Python OpenVerify tests", pythonOpenVerifyTests, ["\\uFF0F", "\\u200B", "\\u0430"]],
    ["Python Torii verifier-key tests", pythonClientTests, ["\\uFF0F", "\\u200B", "\\u0430"]],
    ["Kotlin verifier-key backend tag tests", kotlinBackendTagTests, ["\\uFF0F", "\\u200B", "\\u0430"]],
    ["Kotlin verifier-key instruction tests", kotlinVkTests, ["\\uFF0F", "\\u200B", "\\u0430"]],
    ["Android Java verifier-key instruction tests", javaVkTests, ["\\uFF0F", "\\u200B", "\\u0430"]],
    ["C# verifier-key backend tag tests", csharpVkTests, ["\\uFF0F", "\\u200B", "\\u0430"]],
    ["Swift verifier-key backend tag tests", swiftVkTests, ["\\u{FF0F}", "\\u{200B}", "\\u{0430}"]],
    ["Swift Torii verifier-key tests", swiftToriiTests, ["\\u{FF0F}", "\\u{200B}", "\\u{0430}"]],
  ]) {
    for (const marker of markers) {
      assert.ok(
        text.includes(marker),
        `${label} must reject Unicode-confusable production backend labels containing ${marker}`,
      );
    }
  }
  for (const [label, text, marker] of [
    ["Rust verifier dispatch", coreZk, "STARK_FRI_V1_PRODUCTION_PROFILES"],
    ["JS instruction builder source", jsInstructionBuilders, "STARK_FRI_PRODUCTION_BACKEND_LABELS"],
    ["JS instruction builder dist", jsInstructionBuildersDist, "STARK_FRI_PRODUCTION_BACKEND_LABELS"],
    ["JS Torii source", jsToriiClient, "STARK_FRI_PRODUCTION_BACKEND_LABELS"],
    ["JS Torii dist", jsToriiClientDist, "STARK_FRI_PRODUCTION_BACKEND_LABELS"],
    ["Python shared privacy backend helpers", pythonPrivacyBackends, "_STARK_FRI_PRODUCTION_BACKEND_LABELS"],
    ["Kotlin backend tag", kotlinBackendTag, "starkFriProductionBackends"],
    ["Android Java backend tag", javaBackendTag, "STARK_FRI_PRODUCTION_BACKENDS"],
    ["C# backend tag", csharpBackendTag, "StarkFriProductionBackends"],
    ["Swift backend tag", swiftBackendTag, "starkFriProductionBackends"],
  ]) {
    assert.ok(
      text.includes(marker),
      `${label} must keep STARK/FRI profiles on an explicit production allowlist`,
    );
  }
  const rustStarkFriProfiles =
    coreZk.match(/const\s+STARK_FRI_V1_PRODUCTION_PROFILES:\s*&\[&str\]\s*=\s*&\[[\s\S]*?\];/u)?.[0] ?? "";
  assert.ok(
    rustStarkFriProfiles.includes("\"sha256-goldilocks\"") &&
      rustStarkFriProfiles.includes("\"poseidon2-goldilocks\"") &&
      rustStarkFriProfiles.includes("\"sha256_goldilocks.v1\""),
    "Rust STARK/FRI production profile allowlist must include exact concrete profiles",
  );
  for (const exactLabel of EXPECTED_UNSTABLE_STARK_FRI_PROFILE_LABELS) {
    const profile = exactLabel.split("/").pop();
    assert.ok(
      !rustStarkFriProfiles.includes(`"${profile}"`),
      `Rust STARK/FRI production profile allowlist must not include unstable alias ${exactLabel}`,
    );
  }
  for (const [label, text] of [
    ["JS instruction builder source", jsInstructionBuilders],
    ["JS instruction builder dist", jsInstructionBuildersDist],
    ["JS Torii source", jsToriiClient],
    ["JS Torii dist", jsToriiClientDist],
    ["Python shared privacy backend helpers", pythonPrivacyBackends],
    ["Kotlin backend tag", kotlinBackendTag],
    ["Android Java backend tag", javaBackendTag],
    ["C# backend tag", csharpBackendTag],
    ["Swift backend tag", swiftBackendTag],
  ]) {
    assert.ok(
      !text.includes("halo2/pasta/asset-hidden-transfer-public-test"),
      `${label} must not advertise developer-only native Halo2 profile as production`,
    );
    assert.ok(
      !text.includes("halo2/pasta/tiny-"),
      `${label} must not advertise toy native Halo2 profiles as production`,
    );
    assert.ok(
      !text.includes("halo2/pasta/vote-bool-commit"),
      `${label} must not advertise legacy vote native Halo2 profiles as production`,
    );
    for (const exactLabel of EXPECTED_UNSTABLE_STARK_FRI_PROFILE_LABELS) {
      const quoted = new RegExp(`["']${exactLabel.replaceAll("/", "\\/")}["']`);
      assert.ok(
        !quoted.test(text),
        `${label} must not advertise unstable STARK/FRI profile ${exactLabel} as production`,
      );
    }
    for (const exactLabel of [
      "halo2/pasta/anon-transfer-2x2",
      "halo2/pasta/anon-transfer-2x2-merkle2",
      "halo2/pasta/anon-transfer-2x2-merkle8",
      "halo2/pasta/anon-transfer-2x2-merkle16",
    ]) {
      const quoted = new RegExp(`["']${exactLabel.replaceAll("/", "\\/")}["']`);
      assert.ok(
        !quoted.test(text),
        `${label} must not advertise legacy anon-transfer native Halo2 profile ${exactLabel} as production`,
      );
    }
  }
  for (const [label, text] of [
    ["Rust production backend unit tests", coreZk],
    ["JS instruction builder tests", source("javascript/iroha_js/test/instructionBuilders.test.js")],
    ["JS Torii verifier-key tests", jsToriiClientTests],
    ["Python Torii verifier-key tests", pythonClientTests],
    ["Kotlin verifier-key backend tag tests", kotlinBackendTagTests],
    ["Android Java verifier-key instruction tests", javaVkTests],
    ["C# verifier-key backend tag tests", csharpVkTests],
    ["Swift verifier-key backend tag tests", swiftVkTests],
  ]) {
    for (const backend of EXPECTED_UNREGISTERED_STARK_FRI_PROFILE_LABELS) {
      assert.ok(
        text.includes(backend),
        `${label} must reject unregistered STARK/FRI profile ${backend}`,
      );
    }
  }
  for (const [label, text] of [
    ["Rust production backend unit tests", coreZk],
    ["JS instruction builder tests", source("javascript/iroha_js/test/instructionBuilders.test.js")],
    ["JS Torii verifier-key tests", jsToriiClientTests],
    ["Python Torii verifier-key tests", pythonClientTests],
    ["Kotlin verifier-key backend tag tests", kotlinBackendTagTests],
    ["Kotlin verifier-key instruction tests", kotlinVkTests],
    ["Android Java verifier-key instruction tests", javaVkTests],
    ["C# verifier-key backend tag tests", csharpVkTests],
    ["Swift verifier-key backend tag tests", swiftVkTests],
  ]) {
    for (const backend of EXPECTED_TOY_NATIVE_HALO2_PROFILE_LABELS) {
      assert.ok(
        text.includes(backend),
        `${label} must reject toy native Halo2 profile ${backend}`,
      );
    }
  }
  for (const [label, text] of [
    ["Rust production backend unit tests", coreZk],
    ["JS instruction builder tests", source("javascript/iroha_js/test/instructionBuilders.test.js")],
    ["JS Torii verifier-key tests", jsToriiClientTests],
    ["Python Torii verifier-key tests", pythonClientTests],
    ["Kotlin verifier-key backend tag tests", kotlinBackendTagTests],
    ["Kotlin verifier-key instruction tests", kotlinVkTests],
    ["Android Java verifier-key instruction tests", javaVkTests],
    ["C# verifier-key backend tag tests", csharpVkTests],
    ["Swift verifier-key backend tag tests", swiftVkTests],
  ]) {
    for (const backend of EXPECTED_LEGACY_VOTE_NATIVE_HALO2_PROFILE_LABELS) {
      assert.ok(
        text.includes(backend),
        `${label} must reject legacy vote native Halo2 profile ${backend}`,
      );
    }
  }
  for (const [label, text] of [
    ["Rust production backend unit tests", coreZk],
    ["JS instruction builder tests", source("javascript/iroha_js/test/instructionBuilders.test.js")],
    ["JS Torii verifier-key tests", jsToriiClientTests],
    ["Python Torii verifier-key tests", pythonClientTests],
    ["Kotlin verifier-key backend tag tests", source("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/core/model/zk/VerifyingKeyBackendTagTest.kt")],
    ["Kotlin verifier-key instruction tests", kotlinVkTests],
    ["Android Java verifier-key instruction tests", javaVkTests],
    ["C# verifier-key backend tag tests", csharpVkTests],
    ["Swift verifier-key backend tag tests", swiftVkTests],
  ]) {
    for (const backend of EXPECTED_LEGACY_ANON_TRANSFER_NATIVE_HALO2_PROFILE_LABELS) {
      assert.ok(
        text.includes(backend),
        `${label} must reject legacy anon-transfer native Halo2 profile ${backend}`,
      );
    }
  }
});

test("developer-only privacy backend labels stay rejected before production allowlists", () => {
  const surfaces = [
    ["Rust production backend unit tests", source("crates/iroha_core/src/zk.rs")],
    ["JS Torii verifier-key tests", source("javascript/iroha_js/test/toriiClient.test.js")],
    ["JS instruction builder tests", source("javascript/iroha_js/test/instructionBuilders.test.js")],
    ["Python Torii verifier-key tests", source("python/iroha_python/tests/client_ledger_helpers_test.py")],
    [
      "Kotlin verifier-key backend tag tests",
      source("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/core/model/zk/VerifyingKeyBackendTagTest.kt"),
    ],
    [
      "Kotlin verifier-key instruction tests",
      source(
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/core/model/instructions/VerifyingKeyInstructionBuildersTest.kt",
      ),
    ],
    [
      "Android Java verifier-key instruction tests",
      source(
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/model/instructions/VerifyingKeyInstructionUtilsTests.java",
      ),
    ],
    [
      "C# verifier-key backend tag tests",
      source("csharp/tests/Hyperledger.Iroha.Sdk.Tests/VerifyingKeyBackendTagTests.cs"),
    ],
    [
      "Swift verifier-key backend tag tests",
      source("IrohaSwift/Tests/IrohaSwiftTests/VerifyingKeyBackendTagTests.swift"),
    ],
  ];

  for (const [label, text] of surfaces) {
    for (const backendLabel of EXPECTED_ADVERSARIAL_DEVELOPER_BACKEND_LABELS) {
      assert.ok(
        text.includes(backendLabel),
        `${label} must reject developer-only backend label ${backendLabel}`,
      );
    }
    for (const backendLabel of EXPECTED_DEVELOPER_ONLY_NATIVE_HALO2_PROFILE_LABELS) {
      assert.ok(
        text.includes(backendLabel),
        `${label} must reject developer-only native Halo2 profile ${backendLabel}`,
      );
    }
  }

  const developerOnlyTokens = ["fixture", "dev", "test", "dummy", "fake", "stub", "sample", "placeholder"];
  for (const [label, text] of [
    ["Rust classifier", source("crates/iroha_core/src/zk.rs")],
    ["JS instruction builder classifier", source("javascript/iroha_js/src/instructionBuilders.js")],
    ["JS Torii classifier", source("javascript/iroha_js/src/toriiClient.js")],
    ["Python classifier", source("python/iroha_python/src/iroha_python/_privacy_backends.py")],
    [
      "Kotlin classifier",
      source("kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/core/model/zk/VerifyingKeyBackendTag.kt"),
    ],
    [
      "Android Java classifier",
      source("java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/zk/VerifyingKeyBackendTag.java"),
    ],
    ["C# classifier", source("csharp/src/Hyperledger.Iroha.Sdk/Zk/VerifyingKeyBackendTag.cs")],
    ["Swift classifier", source("IrohaSwift/Sources/IrohaSwift/VerifyingKeyBackendTag.swift")],
  ]) {
    for (const token of developerOnlyTokens) {
      assert.ok(
        text.includes(token),
        `${label} must classify ${token} backends before broad production allowlists`,
      );
    }
  }

  for (const [label, text] of [
    ["Rust production backend unit tests", source("crates/iroha_core/src/zk.rs")],
    ["JS Torii verifier-key tests", source("javascript/iroha_js/test/toriiClient.test.js")],
    ["JS instruction builder tests", source("javascript/iroha_js/test/instructionBuilders.test.js")],
    ["Python Torii verifier-key tests", source("python/iroha_python/tests/client_ledger_helpers_test.py")],
    [
      "Kotlin verifier-key backend tag tests",
      source("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/core/model/zk/VerifyingKeyBackendTagTest.kt"),
    ],
    [
      "Android Java verifier-key instruction tests",
      source(
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/model/instructions/VerifyingKeyInstructionUtilsTests.java",
      ),
    ],
    [
      "C# verifier-key backend tag tests",
      source("csharp/tests/Hyperledger.Iroha.Sdk.Tests/VerifyingKeyBackendTagTests.cs"),
    ],
    [
      "Swift verifier-key backend tag tests",
      source("IrohaSwift/Tests/IrohaSwiftTests/VerifyingKeyBackendTagTests.swift"),
    ],
  ]) {
    for (const backendLabel of EXPECTED_UNSTABLE_STARK_FRI_PROFILE_LABELS) {
      assert.ok(
        text.includes(backendLabel),
        `${label} must cover unstable embedded-text STARK/FRI alias ${backendLabel} as rejected`,
      );
    }
  }
});

test("adversarial pending privacy backend aliases stay covered across SDK tests", () => {
  const surfaces = [
    ["JS instruction builder tests", source("javascript/iroha_js/test/instructionBuilders.test.js")],
    ["Python OpenVerify tests", source("python/iroha_python/tests/verange_test.py")],
    [
      "Java Android backend tag tests",
      source(
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/model/instructions/VerifyingKeyInstructionUtilsTests.java",
      ),
    ],
    [
      "Kotlin/JVM backend tag tests",
      source(
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/core/model/zk/VerifyingKeyBackendTagTest.kt",
      ),
    ],
    [
      "Swift backend tag tests",
      source("IrohaSwift/Tests/IrohaSwiftTests/VerifyingKeyBackendTagTests.swift"),
    ],
    [
      "C# backend tag tests",
      source("csharp/tests/Hyperledger.Iroha.Sdk.Tests/VerifyingKeyBackendTagTests.cs"),
    ],
  ];

  for (const [label, text] of surfaces) {
    for (const backendLabel of EXPECTED_ADVERSARIAL_PENDING_PRIVACY_BACKEND_LABELS) {
      assert.ok(
        text.includes(backendLabel),
        `${label} must include adversarial pending backend alias ${backendLabel}`,
      );
    }
  }

  for (const [label, text] of surfaces.slice(2)) {
    assert.match(
      text,
      /isPendingProductionBackend|IsPendingProductionBackend/,
      `${label} must assert fail-closed pending classification`,
    );
  }
  for (const [label, text] of surfaces.slice(2, 4)) {
    assert.match(
      text,
      /parse\(label\)/,
      `${label} must assert adversarial labels are not canonical Norito tags`,
    );
  }
});

test("mobile and C# privacy tests isolate forged production-gate mutations", () => {
  const surfaces = [
    [
      "Swift privacy native tests",
      source("IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift"),
    ],
    [
      "Java Android privacy native tests",
      source("java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridgeTest.java"),
    ],
    [
      "Kotlin/JVM privacy native tests",
      source("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridgeTest.kt"),
    ],
    [
      "C# privacy native tests",
      source("csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs"),
    ],
  ];

  for (const [label, text] of surfaces) {
    assert.ok(
      text.includes("tampered"),
      `${label} must cover missing-reason mutation attempts`,
    );
    assert.ok(
      text.includes("https://audit.example/forged-signoff"),
      `${label} must cover forged audit-reference mutation`,
    );
    assert.match(
      text,
      /(?:fresh[\s\S]*(?:missingProductionGates|productionGate\.missing|ProductionGate\.Missing)[\s\S]*tampered|tampered[\s\S]*fresh[\s\S]*(?:missingProductionGates|productionGate\.missing|ProductionGate\.Missing))/,
      `${label} must prove forged missing reasons do not pollute fresh capabilities`,
    );
    assert.match(
      text,
      /(?:fresh[\s\S]*(?:auditReferences|AuditReferences)[\s\S]*forged-signoff|forged-signoff[\s\S]*fresh[\s\S]*(?:auditReferences|AuditReferences))/,
      `${label} must prove forged audit references do not pollute fresh capabilities`,
    );
    assert.match(
      text,
      /auditReferences|AuditReferences/,
      `${label} must assert audit-reference fail-closed behavior`,
    );
  }
});

test("SDK privacy native tests clear request copies after native failures", () => {
  const surfaces = [
    [
      "JS source privacy native tests",
      source("javascript/iroha_js/test/privacyNative.test.js"),
      /sanitize native exceptions[\s\S]*capturedRequests[\s\S]*request\.every\(\(value\)\s*=>\s*value\s*===\s*0\)/,
    ],
    [
      "JS package dist privacy native tests",
      source("javascript/iroha_js/test/package_dist.test.js"),
      /sanitize native exceptions[\s\S]*capturedRequests[\s\S]*request\.every\(\(value\)\s*=>\s*value\s*===\s*0\)/,
    ],
    [
      "Python privacy native tests",
      source("python/iroha_python/tests/crypto_algorithms_test.py"),
      /sanitize_native_exceptions[\s\S]*native\.requests[\s\S]*all\(value == 0 for value in request\)/,
    ],
    [
      "Java Android privacy native tests",
      source("java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridgeTest.java"),
      /nativeExceptionsAreSanitized[\s\S]*capturedRequests[\s\S]*assertAllZero\(capturedRequests\[0\]\)[\s\S]*assertAllZero\(capturedRequests\[1\]\)/,
    ],
    [
      "Kotlin/JVM privacy native tests",
      source("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridgeTest.kt"),
      /nativeExceptionsAreSanitized[\s\S]*buildRequest[\s\S]*verifyRequest[\s\S]*assertAllZero\(buildRequest\)[\s\S]*assertAllZero\(verifyRequest\)/,
    ],
    [
      "C# privacy native tests",
      source("csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs"),
      /PrivacyNativeSanitizesNativeExceptions[\s\S]*buildRequest[\s\S]*verifyRequest[\s\S]*Array\.TrueForAll\(buildRequest![\s\S]*Array\.TrueForAll\(verifyRequest!/,
    ],
    [
      "Swift privacy native tests",
      source("IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift"),
      /testTemporaryPrivacyRequestArchiveClearsCopyWhenBodyThrows[\s\S]*didClearForTesting[\s\S]*throw LocalError\.nativeFailure[\s\S]*clearedArchive\.allSatisfy\s*\{\s*\$0\s*==\s*0\s*\}/,
    ],
  ];

  for (const [label, text, pattern] of surfaces) {
    assert.match(
      text,
      pattern,
      `${label} must prove copied privacy requests are zeroed after native failures`,
    );
  }
});

test("JS and Python privacy availability probes clear request copies after failures", () => {
  const surfaces = [
    [
      "JS source privacy native tests",
      source("javascript/iroha_js/test/privacyNative.test.js"),
      /availability probes clear request copies after native failures[\s\S]*probe failure after request copy[\s\S]*throwingProbe[\s\S]*badOutputProbe[\s\S]*Buffer\.alloc\(privacyNoritoFrame\(0x52\)\.length\)/,
    ],
    [
      "JS package dist privacy native tests",
      source("javascript/iroha_js/test/package_dist.test.js"),
      /availability clears request copies after failures[\s\S]*probe failure after request copy[\s\S]*throwingProbe[\s\S]*badOutputProbe[\s\S]*Buffer\.alloc\(privacyNoritoFrame\(0x52\)\.length\)/,
    ],
    [
      "Python privacy native tests",
      source("python/iroha_python/tests/crypto_algorithms_test.py"),
      /availability_probes_clear_request_copies_after_failures[\s\S]*probe failure after request copy[\s\S]*throwing_native\.build_request[\s\S]*all\(value == 0 for value in throwing_native\.build_request\)[\s\S]*bad_output_native\.verify_request[\s\S]*all\(value == 0 for value in bad_output_native\.verify_request\)/,
    ],
  ];

  for (const [label, text, pattern] of surfaces) {
    assert.match(
      text,
      pattern,
      `${label} must prove availability probe requests are zeroed after failures`,
    );
  }
});

test("Python privacy native wrappers require the complete FFI method surface", () => {
  const pythonCrypto = source("python/iroha_python/src/iroha_python/crypto.py");
  const pythonTests = source("python/iroha_python/tests/crypto_algorithms_test.py");

  assert.match(
    pythonCrypto,
    /def _missing_privacy_native_methods\([\s\S]*_PRIVACY_NATIVE_METHODS/,
    "Python privacy wrapper must enumerate missing native FFI methods",
  );
  assert.match(
    pythonCrypto,
    /privacy FFI requires complete native method surface; missing/,
    "Python privacy wrapper must fail closed when any privacy FFI method is missing",
  );
  const completeSurfaceTest = sliceBetween(
    pythonTests,
    "def test_privacy_native_availability_requires_complete_method_surface",
    "def test_privacy_native_availability_probes_reject_unsafe_raw_output",
    "Python complete privacy method surface test",
  );
  for (const pattern of [
    /privacy_capabilities_v1\(\)/,
    /privacy_build_proof_v1\(_PRIVACY_REQUEST_ARCHIVE\)/,
    /privacy_verify_proof_v1\(_PRIVACY_REQUEST_ARCHIVE\)/,
  ]) {
    assert.match(
      completeSurfaceTest,
      pattern,
      "Python privacy tests must prove every public wrapper rejects an incomplete method surface",
    );
  }
  assert.match(
    completeSurfaceTest,
    /privacy FFI requires complete native method surface; missing privacy_verify_proof_v1/,
    "Python privacy test must pin the complete-surface error",
  );
  assert.match(
    pythonTests,
    /test_privacy_native_availability_requires_complete_method_surface/,
    "Python privacy tests must keep the complete method surface regression",
  );
});

test("mobile, Swift, and C# privacy native tests isolate hostile native request mutation", () => {
  const surfaces = [
    [
      "Swift privacy native tests",
      source("IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift"),
      "func testHostileTemporaryPrivacyRequestMutationCannotMutateCallerArchive",
      "func testTemporaryPrivacyRequestArchiveClearsCopyWhenBodyThrows",
      [
        /UnsafeMutablePointer\(mutating:\s*buffer\.baseAddress\)/,
        /request\?\[0\]\s*=\s*0x00/,
        /request\?\[6\]\s*=\s*0x7F/,
        /XCTAssertEqual\(requestArchive,\s*originalArchive\)/,
        /clearedArchive\.allSatisfy\s*\{\s*\$0\s*==\s*0\s*\}/,
      ],
    ],
    [
      "Java Android privacy native tests",
      source("java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridgeTest.java"),
      "private static void hostileNativeRequestMutationCannotMutateCallerArchive",
      "private static void rejectsInvalidNoritoRequestsBeforeNativeDispatch",
      [
        /request\[0\]\s*=\s*0x00/,
        /request\[6\]\s*=\s*0x7f/,
        /Arrays\.equals\(requestArchive,\s*originalArchive\)/,
        /assertAllZero\(capturedRequests\[0\]\)/,
        /assertAllZero\(capturedRequests\[1\]\)/,
      ],
    ],
    [
      "Kotlin/JVM privacy native tests",
      source("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridgeTest.kt"),
      "fun hostileNativeRequestMutationCannotMutateCallerArchive",
      "fun rejectsInvalidNoritoRequestsBeforeNativeDispatch",
      [
        /request\[0\]\s*=\s*0x00\.toByte\(\)/,
        /request\[6\]\s*=\s*0x7f\.toByte\(\)/,
        /requestArchive\.contentEquals\(originalArchive\)/,
        /assertAllZero\(buildRequest\)/,
        /assertAllZero\(verifyRequest\)/,
      ],
    ],
    [
      "C# privacy native tests",
      source("csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs"),
      "public void PrivacyNativeHostileRequestMutationCannotMutateCallerArchive",
      "public void PrivacyNativeRejectsMalformedProofRequestsBeforeLoadingNativeBridge",
      [
        /requestPtr\[0\]\s*=\s*0x00/,
        /requestPtr\[6\]\s*=\s*0x7f/,
        /Assert\.Equal\(originalArchive,\s*requestArchive\)/,
        /Array\.TrueForAll\(buildRequest![\s\S]*value\s*=>\s*value\s*==\s*0\)/,
        /Array\.TrueForAll\(verifyRequest![\s\S]*value\s*=>\s*value\s*==\s*0\)/,
      ],
    ],
  ];

  for (const [label, text, start, end, patterns] of surfaces) {
    const testBody = sliceBetween(text, start, end, `${label} hostile request mutation test`);
    for (const pattern of patterns) {
      assert.match(
        testBody,
        pattern,
        `${label} must prove hostile native request mutation cannot affect caller-owned archives`,
      );
    }
  }
});

test("Swift privacy native tests reject malformed request archives before dispatch", () => {
  const requestTest = sliceBetween(
    source("IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift"),
    "func testRejectsInvalidNoritoRequestArchivesBeforeBridgeCall",
    "func testRejectsInvalidNoritoNativeOutput",
    "Swift invalid privacy request archive test",
  );

  for (const pattern of [
    /Data\(\[0x01\]\)/,
    /invalidPrivacyNoritoFrame\(offset:\s*0,\s*value:\s*0x58\)/,
    /invalidPrivacyNoritoFrame\(offset:\s*4,\s*value:\s*1\)/,
    /invalidPrivacyNoritoFrame\(offset:\s*5,\s*value:\s*1\)/,
    /invalidPrivacyNoritoFrame\(offset:\s*22,\s*value:\s*1\)/,
    /invalidPrivacyNoritoDeclaredPayloadLength\(schemaByte:\s*0x52\)/,
    /invalidPrivacyNoritoOversizedPayloadLength\(schemaByte:\s*0x52\)/,
    /invalidPrivacyNoritoFrame\(offset:\s*39,\s*value:\s*0x40\)/,
    /invalidPrivacyNoritoFrame\(offset:\s*39,\s*value:\s*0x20\)/,
    /invalidPrivacyNoritoWithNonzeroPadding\(\)/,
    /invalidPrivacyNoritoWithExcessivePadding\(\)/,
    /invalidPrivacyNoritoFrame\(offset:\s*31,\s*value:\s*1\)/,
    /invalidPrivacyNoritoPayloadTamper\(\)/,
  ]) {
    assert.match(requestTest, pattern);
  }
  assert.match(
    requestTest,
    /XCTFail\("invalid request must not reach native dispatch"\)/,
    "Swift malformed request archives must be rejected before native dispatch",
  );
});

test("JS privacy native tests reject adversarial malformed request archives before dispatch", () => {
  const surfaces = [
    [
      "JS source privacy native tests",
      source("javascript/iroha_js/test/privacyNative.test.js"),
      'test("privacy native wrappers require binary Norito request archives"',
      'test("privacy native wrappers fail when methods are missing"',
    ],
    [
      "JS package dist privacy native tests",
      source("javascript/iroha_js/test/package_dist.test.js"),
      'test("package dist privacy native wrappers reject invalid request archives"',
      'test("package dist privacy native wrappers sanitize native exceptions"',
    ],
  ];

  for (const [label, text, start, end] of surfaces) {
    const fixtures = sliceBetween(
      text,
      "function malformedPrivacyRequestArchives",
      "const PRIVACY_CAPABILITIES_ARCHIVE",
      `${label} malformed request fixtures`,
    );
    for (const pattern of [
      /badMagic[\s\S]*\[0\]\s*=\s*0x00/,
      /badVersion[\s\S]*\[4\]\s*=\s*1/,
      /badMinorVersion[\s\S]*\[5\]\s*=\s*1/,
      /badCompression[\s\S]*\[22\]\s*=\s*1/,
      /badDeclaredPayloadLength[\s\S]*privacyNoritoFrameWithDeclaredPayloadLength\(0x52,\s*6n\)/,
      /badOversizedDeclaredPayloadLength[\s\S]*privacyNoritoFrameWithDeclaredPayloadLength\(\s*0x52,\s*0x8000000000000000n/,
      /badPadding[\s\S]*Buffer\.concat/,
      /badExcessivePadding[\s\S]*privacyNoritoFrameWithPadding\(0x52,\s*65\)/,
      /badFlags[\s\S]*\[39\]\s*=\s*0x08/,
      /badFieldBitsetFlags[\s\S]*\[39\]\s*=\s*0x20/,
      /badChecksum[\s\S]*\[31\]\s*\^=/,
      /badPayload[\s\S]*\[44\]\s*\^=/,
    ]) {
      assert.match(fixtures, pattern, `${label} must keep adversarial Norito request fixtures`);
    }

    const requestTest = sliceBetween(text, start, end, `${label} invalid request test`);
    assert.match(requestTest, /malformedPrivacyRequestArchives\(\)/);
    assert.match(
      requestTest,
      /assert\.fail\("invalid build request must not reach native dispatch"\)/,
      `${label} must keep invalid build requests out of native dispatch`,
    );
    assert.match(
      requestTest,
      /assert\.fail\("invalid verify request must not reach native dispatch"\)/,
      `${label} must keep invalid verify requests out of native dispatch`,
    );
  }
});

test("privacy SDK guard runs wrong-operation result schema regressions", () => {
  const guard = source("ci/check_privacy_sdk_guard.sh");
  const bridgeHeaderRunner = source("ci/check_connect_norito_bridge_header.sh");
  const csharpRunner = source("ci/check_privacy_csharp_sdk.sh");
  const jsRunner = source("ci/check_privacy_js_sdk.sh");
  const jvmRunner = source("ci/check_privacy_jvm_sdk.sh");
  const pythonRunner = source("ci/check_privacy_python_sdk.sh");
  const swiftRunner = source("ci/check_privacy_swift_sdk.sh");
  const workflow = source(".github/workflows/pr_privacy_sdk_guard.yml");
  assert.match(
    guard,
    /SDK_NODE_BIN="\$\(resolve_node_20_bin\)"[\s\S]*PRIVACY_JS_SDK_ROOT="\$\{ROOT_DIR\}"\s*\\[\s\S]*PRIVACY_JS_SDK_NODE_BIN="\$\{SDK_NODE_BIN\}"\s*\\[\s\S]*bash "\$\{ROOT_DIR\}\/ci\/check_privacy_js_sdk\.sh"/,
    "Privacy SDK guard must delegate JavaScript SDK regressions to the focused JS runner",
  );
  assert.match(
    guard,
    /SDK_PYTHON_BIN="\$\(resolve_python_311_bin\)"[\s\S]*PRIVACY_PYTHON_SDK_ROOT="\$\{ROOT_DIR\}"\s*\\[\s\S]*PRIVACY_PYTHON_SDK_PYTHON_BIN="\$\{SDK_PYTHON_BIN\}"\s*\\[\s\S]*PRIVACY_PYTHON_SDK_VENV="\$\{VENV_DIR\}"\s*\\[\s\S]*bash "\$\{ROOT_DIR\}\/ci\/check_privacy_python_sdk\.sh"/,
    "Privacy SDK guard must delegate Python SDK regressions to the focused Python runner",
  );
  assert.match(
    pythonRunner,
    /PYTHON_OVERRIDE="\$\{PRIVACY_PYTHON_SDK_PYTHON_BIN:-\}"[\s\S]*resolve_python_311_bin\(\)[\s\S]*python3\.11[\s\S]*PYTHON_BIN="\$\(resolve_python_311_bin\)"/,
    "Privacy Python SDK runner must keep the documented Python override variable",
  );
  assert.match(
    pythonRunner,
    /export VIRTUAL_ENV="\$\{VENV_DIR\}"[\s\S]*export PATH="\$\{VENV_DIR\}\/bin:\$\{PATH\}"[\s\S]*"\$\{VENV_DIR\}\/bin\/python" -m maturin develop --release/,
    "Privacy Python SDK runner must activate the selected venv before maturin",
  );
  assert.match(
    jsRunner,
    /NODE_OVERRIDE="\$\{PRIVACY_JS_SDK_NODE_BIN:-\}"[\s\S]*is_node_20_bin\(\)[\s\S]*resolve_node_20_bin\(\)[\s\S]*NODE_BIN="\$\(resolve_node_20_bin\)"/,
    "Privacy JavaScript SDK runner must keep the documented Node override variable",
  );
  assert.match(
    jsRunner,
    /NODE_VERSION="\$\("\$\{NODE_BIN\}" --version\)"/,
    "Privacy JavaScript SDK runner must print the selected Node version",
  );
  assert.match(
    jsRunner,
    /printf '%s\\n' "\$\{NODE_VERSION\}"[\s\S]*v20\.\*\) ;;/,
    "Privacy JavaScript SDK runner must reject non-Node-20 runtimes",
  );
  assert.match(
    jsRunner,
    /"\$\{NODE_BIN\}" --test --test-name-pattern "privacy native wrappers reject wrong-operation result schemas"\s*\\\s*\n\s*test\/instructionBuilders\.test\.js/,
    "Privacy JavaScript SDK runner must run the JS source wrong-operation result-schema regression",
  );
  assert.match(
    jsRunner,
    /"\$\{NODE_BIN\}" --test --test-name-pattern "package dist entrypoint exports privacy native archive helpers\|package dist privacy native wrappers reject wrong-operation result schemas\|package declarations mark privacy capability metadata readonly"\s*\\\s*\n\s*test\/package_dist\.test\.js/,
    "Privacy JavaScript SDK runner must run the packaged JS privacy native export, declaration, and wrong-operation result-schema regressions",
  );
  assert.match(
    jsRunner,
    /"\$\{NODE_BIN\}" --test --test-name-pattern "browser crypto exposes native-only helpers as safe stubs"\s*\\\s*\n\s*test\/crypto\.browser\.test\.js/,
    "Privacy JavaScript SDK runner must run the browser privacy native stub regression",
  );
  assert.match(
    pythonRunner,
    /tests\/crypto_algorithms_test\.py/,
    "Privacy Python SDK runner must keep the Python native-wrapper adversarial regressions in scope",
  );
  assert.match(
    pythonRunner,
    /tests\/package_import_fallback_test\.py/,
    "Privacy Python SDK runner must cover package-root native import fallback regressions",
  );
  assert.match(
    bridgeHeaderRunner,
    /required_privacy_ffi[\s\S]*iroha_privacy_capabilities_v1[\s\S]*iroha_privacy_build_proof_v1[\s\S]*iroha_privacy_verify_proof_v1[\s\S]*iroha_privacy_free_buffer/,
    "NoritoBridge header guard must require all privacy FFI symbols",
  );
  assert.match(
    bridgeHeaderRunner,
    /privacy_declaration_pattern[\s\S]*header_privacy_declarations[\s\S]*undeclared_privacy_exports/,
    "NoritoBridge header guard must compare Rust privacy exports against header declarations",
  );
  assert.match(
    bridgeHeaderRunner,
    /expected_privacy_signatures[\s\S]*C header privacy declaration has wrong signature/,
    "NoritoBridge header guard must reject privacy FFI signature drift",
  );
  assert.match(
    swiftRunner,
    /SWIFTC_BIN="\$\{PRIVACY_SWIFT_SDK_SWIFTC_BIN:-swiftc\}"/,
    "Privacy Swift SDK runner must keep the documented swiftc override variable",
  );
  assert.match(
    swiftRunner,
    /"\$\{SWIFTC_BIN\}"\s+-parse\s+-parse-as-library/,
    "Privacy Swift SDK guard must parse the focused Swift privacy source and tests",
  );
  for (const swiftPath of [
    "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift",
    "IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift",
    "IrohaSwift/Sources/IrohaSwift/VerifyingKeyBackendTag.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/VerifyingKeyBackendTagTests.swift",
  ]) {
    assert.ok(
      swiftRunner.includes(swiftPath),
      `Privacy Swift SDK guard must parse ${swiftPath}`,
    );
  }
  assert.match(
    jvmRunner,
    /JAVA_HOME_OVERRIDE="\$\{PRIVACY_JVM_SDK_JAVA_HOME:-\}"/,
    "Privacy JVM SDK runner must keep the documented Java home override variable",
  );
  assert.match(
    jvmRunner,
    /JAVA_HOME must point to a JDK 21 home for privacy JVM SDK tests\./,
    "Privacy JVM SDK runner must reject inherited non-JDK-21 JAVA_HOME values",
  );
  assert.match(
    jvmRunner,
    /:core-jvm:test[\s\S]*--tests org\.hyperledger\.iroha\.sdk\.privacy\.PrivacyNativeBridgeTest[\s\S]*--tests org\.hyperledger\.iroha\.sdk\.core\.model\.zk\.VerifyingKeyBackendTagTest[\s\S]*--tests org\.hyperledger\.iroha\.sdk\.core\.model\.instructions\.VerifyingKeyInstructionBuildersTest/,
    "Privacy JVM SDK guard must run focused Kotlin privacy, backend-tag, and verifier-key builder tests",
  );
  for (const jvmPath of [
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java",
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridgeTest.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/InstructionBox.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/InstructionKind.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/InstructionTemplate.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/RegisterVerifyingKeyInstruction.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/UpdateVerifyingKeyInstruction.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/zk/VerifyingKeyBackendTag.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/zk/VerifyingKeyStatus.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/zk/VerifyingKeyRecordDescription.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/VerifyingKeyInstructionUtils.java",
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/model/instructions/VerifyingKeyInstructionUtilsTests.java",
  ]) {
    assert.ok(jvmRunner.includes(jvmPath), `Privacy JVM SDK guard must compile ${jvmPath}`);
  }
  assert.match(
    jvmRunner,
    /java -ea -cp "\$\{JAVA_OUT\}"\s+\\\s*\n\s+org\.hyperledger\.iroha\.android\.privacy\.PrivacyNativeBridgeTest/,
    "Privacy JVM SDK guard must execute the Java privacy bridge harness",
  );
  assert.match(
    jvmRunner,
    /java -ea -cp "\$\{JAVA_OUT\}"\s+\\\s*\n\s+org\.hyperledger\.iroha\.android\.model\.instructions\.VerifyingKeyInstructionUtilsTests/,
    "Privacy JVM SDK guard must execute the Java backend-tag harness",
  );
  assert.match(
    csharpRunner,
    /DOTNET_BIN="\$\{PRIVACY_CSHARP_DOTNET_BIN:-dotnet\}"/,
    "Privacy C# SDK runner must keep the documented dotnet override variable",
  );
  assert.match(
    csharpRunner,
    /csharp\/tests\/Hyperledger\.Iroha\.Sdk\.Tests\/Hyperledger\.Iroha\.Sdk\.Tests\.csproj/,
    "Privacy SDK guard must keep focused C# privacy runtime tests in scope",
  );
  assert.match(
    guard,
    /csharp\/src\/Hyperledger\.Iroha\.Sdk\/Hyperledger\.Iroha\.Sdk\.csproj/,
    "Privacy SDK guard must cover the C# SDK project file",
  );
  assert.match(
    workflow,
    /"csharp\/src\/Hyperledger\.Iroha\.Sdk\/Hyperledger\.Iroha\.Sdk\.csproj"/,
    "Privacy SDK workflow must trigger on the C# SDK project file",
  );
  assert.match(
    workflow,
    /"ci\/check_privacy_swift_sdk\.sh"/,
    "Privacy SDK workflow must trigger on the Swift privacy guard runner",
  );
  assert.match(
    workflow,
    /"ci\/check_privacy_jvm_sdk\.sh"/,
    "Privacy SDK workflow must trigger on the JVM privacy guard runner",
  );
  assert.match(
    workflow,
    /"ci\/check_privacy_js_sdk\.sh"/,
    "Privacy SDK workflow must trigger on the JavaScript privacy guard runner",
  );
  assert.match(
    workflow,
    /"ci\/check_privacy_python_sdk\.sh"/,
    "Privacy SDK workflow must trigger on the Python privacy guard runner",
  );
  assert.match(
    workflow,
    /"ci\/check_no_tracked_python_bytecode\.sh"/,
    "Privacy SDK workflow must trigger on the tracked Python bytecode guard",
  );
  assert.match(
    workflow,
    /"ci\/check_connect_norito_bridge_header\.sh"/,
    "Privacy SDK workflow must trigger on the native bridge header guard",
  );
  assert.match(
    workflow,
    /"crates\/connect_norito_bridge\/include\/connect_norito_bridge\.h"/,
    "Privacy SDK workflow must trigger on the C bridge header",
  );
  assert.match(
    workflow,
    /"java\/iroha_android\/src\/main\/java\/org\/hyperledger\/iroha\/android\/model\/zk\/VerifyingKeyStatus\.java"/,
    "Privacy SDK workflow must trigger on the Java backend status dependency",
  );
  assert.match(
    csharpRunner,
    /FullyQualifiedName~PrivacyNativeTests/,
    "Privacy C# SDK guard must run the privacy native test class",
  );
  assert.match(
    csharpRunner,
    /FullyQualifiedName~VerifyingKeyBackendTagTests/,
    "Privacy C# SDK guard must run the backend-tag test class",
  );
  assert.match(
    workflow,
    /privacy_csharp_sdk_tests:[\s\S]*actions\/setup-dotnet@v4[\s\S]*dotnet-version:\s+8\.0\.x[\s\S]*run:\s+ci\/check_privacy_csharp_sdk\.sh/,
    "Privacy SDK workflow must run focused C# privacy tests with dotnet 8",
  );
  assert.match(
    workflow,
    /privacy_swift_sdk_parse:[\s\S]*runs-on:\s+macos-latest[\s\S]*run:\s+ci\/check_privacy_swift_sdk\.sh/,
    "Privacy SDK workflow must run focused Swift privacy parsing on macOS",
  );
  assert.match(
    workflow,
    /privacy_jvm_sdk_tests:[\s\S]*actions\/setup-java@v4[\s\S]*distribution:\s+"temurin"[\s\S]*java-version:\s+"21"[\s\S]*run:\s+ci\/check_privacy_jvm_sdk\.sh/,
    "Privacy SDK workflow must run focused JVM and Java privacy tests with Java 21",
  );
  assert.match(
    workflow,
    /privacy_javascript_sdk_tests:[\s\S]*actions\/setup-node@v4[\s\S]*node-version:\s+"20"[\s\S]*npm ci --prefix javascript\/iroha_js[\s\S]*run:\s+ci\/check_privacy_js_sdk\.sh/,
    "Privacy SDK workflow must run focused JavaScript privacy tests with Node 20",
  );
  assert.match(
    workflow,
    /privacy_python_sdk_tests:[\s\S]*actions\/setup-python@v5[\s\S]*python-version:\s+"3\.11"[\s\S]*run:\s+ci\/check_privacy_python_sdk\.sh/,
    "Privacy SDK workflow must run focused Python privacy tests with Python 3.11",
  );
  assertWorkflowIncludesPaths(
    workflow,
    quotedStringsFromInventory(
      guard,
      "required_paths = (",
      "readme_required = {",
    ),
    "Privacy SDK guard",
  );
  assert.match(
    workflow,
    /NoritoBridge privacy header parity[\s\S]*run:\s+ci\/check_connect_norito_bridge_header\.sh[\s\S]*Privacy SDK guard negative controls/,
    "Privacy SDK workflow must run native header parity before guard negative controls",
  );
  for (const mode of REQUIRED_PRIVACY_HEADER_NEGATIVE_CONTROL_MODES) {
    assert.ok(
      bridgeHeaderRunner.includes(mode),
      `NoritoBridge header guard must implement ${mode}`,
    );
  }
  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_connect_norito_bridge_header.sh",
    REQUIRED_PRIVACY_HEADER_NEGATIVE_CONTROL_MODES,
    "NoritoBridge privacy header guard",
  );
  assert.match(
    workflow,
    /Reject tracked Python bytecode[\s\S]*run:\s+bash ci\/check_no_tracked_python_bytecode\.sh[\s\S]*Privacy SDK guard negative controls/,
    "Privacy SDK workflow must reject tracked Python bytecode before guard negative controls",
  );
  assert.match(
    workflow,
    /Reject tracked Python bytecode[\s\S]*run:\s+bash ci\/check_no_tracked_python_bytecode\.sh[\s\S]*- name:\s+Privacy SDK parity and fail-closed guard\s*\n\s*run:\s+ci\/check_privacy_sdk_guard\.sh/,
    "Privacy SDK workflow must reject tracked Python bytecode before the main guard",
  );
  assert.match(
    workflow,
    /privacy_native_bridge_tests:[\s\S]*runs-on:\s+ubuntu-latest[\s\S]*Swatinem\/rust-cache@v2[\s\S]*run:\s+cargo test -p connect_norito_bridge privacy_ --lib -- --test-threads=1/,
    "Privacy SDK workflow must run focused native connect_norito_bridge privacy tests",
  );
  assert.match(
    workflow,
    /privacy-sdk-guard:[\s\S]*needs:\s*\[[^\]]*\bprivacy_native_bridge_tests\b[^\]]*\]/,
    "Privacy SDK guard job must depend on focused native privacy bridge tests",
  );
  assert.match(
    workflow,
    /privacy-sdk-guard:[\s\S]*needs:\s*\[[^\]]*\bprivacy_swift_sdk_parse\b[^\]]*\]/,
    "Privacy SDK guard job must depend on focused Swift privacy parsing",
  );
  assert.match(
    workflow,
    /privacy-sdk-guard:[\s\S]*needs:\s*\[[^\]]*\bprivacy_jvm_sdk_tests\b[^\]]*\]/,
    "Privacy SDK guard job must depend on focused JVM and Java privacy tests",
  );
  assert.match(
    workflow,
    /privacy-sdk-guard:[\s\S]*needs:\s*\[[^\]]*\bprivacy_csharp_sdk_tests\b[^\]]*\]/,
    "Privacy SDK guard job must depend on focused C# privacy tests",
  );
  assert.match(
    workflow,
    /privacy-sdk-guard:[\s\S]*needs:\s*\[[^\]]*\bprivacy_javascript_sdk_tests\b[^\]]*\]/,
    "Privacy SDK guard job must depend on focused JavaScript privacy tests",
  );
  assert.match(
    workflow,
    /privacy-sdk-guard:[\s\S]*needs:\s*\[[^\]]*\bprivacy_python_sdk_tests\b[^\]]*\]/,
    "Privacy SDK guard job must depend on focused Python privacy tests",
  );
  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_privacy_sdk_guard.sh",
    negativeControlModesFromInventory(
      guard,
      "negative_control_commands = (",
      "required_paths = (",
    ),
    "Privacy SDK guard",
  );
  for (const mode of [
    "--negative-control-bridge-header-workflow-path",
    "--negative-control-bridge-header-command-workflow",
    "--negative-control-bridge-header-negative-controls-workflow",
    "--negative-control-bytecode-workflow",
    "--negative-control-workflow-path",
    "--negative-control-workflow-command",
    "--negative-control-backend-tag-workflow-path",
    "--negative-control-browser-test-workflow-path",
    "--negative-control-negative-controls-workflow",
    "--negative-control-negative-controls-comment-workflow",
    "--negative-control-negative-controls-order-workflow",
    "--negative-control-readme-boundary",
    "--negative-control-readme-api",
    "--negative-control-readme-error-code",
    "--negative-control-browser-error-code",
    "--negative-control-browser-dist-error-code",
    "--negative-control-workflow-cancel-in-progress",
    "--negative-control-native-bridge-job-workflow",
    "--negative-control-native-bridge-runner-workflow",
    "--negative-control-native-bridge-test-workflow",
    "--negative-control-native-bridge-needs-workflow",
    "--negative-control-swift-sdk-job-workflow",
    "--negative-control-swift-sdk-runner-workflow",
    "--negative-control-swift-sdk-parse-workflow",
    "--negative-control-swift-sdk-version-script",
    "--negative-control-swift-sdk-override-script",
    "--negative-control-swift-sdk-needs-workflow",
    "--negative-control-jvm-sdk-job-workflow",
    "--negative-control-jvm-sdk-setup-workflow",
    "--negative-control-jvm-sdk-test-workflow",
    "--negative-control-jvm-sdk-java-home-override-script",
    "--negative-control-jvm-sdk-java-home-reject-script",
    "--negative-control-jvm-sdk-needs-workflow",
    "--negative-control-csharp-sdk-job-workflow",
    "--negative-control-csharp-sdk-setup-workflow",
    "--negative-control-csharp-sdk-dotnet-version-workflow",
    "--negative-control-csharp-sdk-setup-order-workflow",
    "--negative-control-csharp-sdk-dotnet-version-script",
    "--negative-control-csharp-sdk-dotnet-override-script",
    "--negative-control-csharp-sdk-dotnet-major-script",
    "--negative-control-csharp-sdk-test-workflow",
    "--negative-control-csharp-sdk-needs-workflow",
    "--negative-control-js-sdk-job-workflow",
    "--negative-control-js-sdk-runner-workflow",
    "--negative-control-js-sdk-node-setup-workflow",
    "--negative-control-js-sdk-node-version-script",
    "--negative-control-js-sdk-node-override-script",
    "--negative-control-js-sdk-node-resolver-script",
    "--negative-control-js-sdk-node-major-script",
    "--negative-control-js-sdk-install-workflow",
    "--negative-control-js-sdk-test-workflow",
    "--negative-control-js-sdk-install-order-workflow",
    "--negative-control-js-sdk-test-order-workflow",
    "--negative-control-js-sdk-needs-workflow",
    "--negative-control-python-sdk-job-workflow",
    "--negative-control-python-sdk-runner-workflow",
    "--negative-control-python-sdk-setup-workflow",
    "--negative-control-python-sdk-rust-cache-workflow",
    "--negative-control-python-sdk-timeout-workflow",
    "--negative-control-python-sdk-test-workflow",
    "--negative-control-python-sdk-override-script",
    "--negative-control-python-sdk-resolver-script",
    "--negative-control-python-sdk-major-script",
    "--negative-control-python-sdk-venv-rebuild-script",
    "--negative-control-python-sdk-native-build-script",
    "--negative-control-python-sdk-venv-activation-script",
    "--negative-control-python-sdk-bytecode-script",
    "--negative-control-python-sdk-needs-workflow",
    "--negative-control-main-rust-cache-workflow",
    "--negative-control-main-timeout-workflow",
  ]) {
    assert.ok(
      guard.includes(`if mode == "${mode}":`),
      `Privacy SDK guard must implement ${mode}`,
    );
    assert.match(
      workflow,
      new RegExp(`^\\s+ci/check_privacy_sdk_guard\\.sh ${mode}$`, "m"),
      `Privacy SDK workflow must run ${mode}`,
    );
  }
  for (const [mode, endMarker] of [
    [
      "--negative-control-browser-error-code",
      'if mode == "--negative-control-browser-dist-error-code":',
    ],
    ["--negative-control-browser-dist-error-code", "\nif mode:"],
  ]) {
    const block = sliceBetween(
      guard,
      `if mode == "${mode}":`,
      endMarker,
      `Privacy SDK guard ${mode} body`,
    );
    assert.match(
      block,
      /finally:\s*\n\s*target\.write_text\(original,\s*encoding="utf-8"\)/,
      `Privacy SDK guard ${mode} must restore mutated browser bundles`,
    );
    assert.match(
      block,
      /subprocess\.run\([\s\S]*timeout=10,[\s\S]*check=False/,
      `Privacy SDK guard ${mode} must bound the browser regression subprocess`,
    );
  }
});

test("privacy JavaScript SDK runner rejects non-Node-20 overrides before tests", () => {
  assertRunnerRejectsNodeMajor(
    "ci/check_privacy_js_sdk.sh",
    "PRIVACY_JS_SDK_NODE_BIN",
    "Privacy JavaScript SDK runner",
  );
});

test("privacy Python SDK runner rejects non-3.11 overrides before native builds", () => {
  assertRunnerRejectsPythonMajor(
    "ci/check_privacy_python_sdk.sh",
    "PRIVACY_PYTHON_SDK_PYTHON_BIN",
    "Privacy Python SDK runner",
  );
});

test("privacy Swift SDK runner propagates parse failures", () => {
  assertRunnerPropagatesSwiftParseFailure(
    "ci/check_privacy_swift_sdk.sh",
    "PRIVACY_SWIFT_SDK_SWIFTC_BIN",
    "Privacy Swift SDK runner",
  );
});

test("privacy JVM SDK runner rejects non-JDK-21 overrides before tests", () => {
  assertRunnerRejectsJavaHome(
    "ci/check_privacy_jvm_sdk.sh",
    "PRIVACY_JVM_SDK_JAVA_HOME",
    "Privacy JVM SDK runner",
  );
});

test("privacy C# SDK runner rejects non-.NET-8 overrides before tests", () => {
  assertRunnerRejectsDotnetSdk(
    "ci/check_privacy_csharp_sdk.sh",
    "PRIVACY_CSHARP_DOTNET_BIN",
    "Privacy C# SDK runner",
  );
});

test("Python privacy native tests reject adversarial malformed request archives before dispatch", () => {
  const pythonTests = source("python/iroha_python/tests/crypto_algorithms_test.py");
  const fixtures = sliceBetween(
    pythonTests,
    "def _malformed_privacy_request_archives",
    "class _FakePrivacyNative",
    "Python malformed privacy request fixtures",
  );
  for (const pattern of [
    /bad_magic[\s\S]*\[0\]\s*=\s*0x00/,
    /bad_version[\s\S]*\[4\]\s*=\s*1/,
    /bad_minor_version[\s\S]*\[5\]\s*=\s*1/,
    /bad_compression[\s\S]*\[22\]\s*=\s*1/,
    /bad_declared_payload_length[\s\S]*_privacy_norito_frame_with_declared_payload_length\(0x52,\s*6\)/,
    /bad_oversized_declared_payload_length[\s\S]*_privacy_norito_frame_with_declared_payload_length\(\s*0x52,\s*0x8000000000000000/,
    /bad_padding[\s\S]*_PRIVACY_REQUEST_ARCHIVE\s*\+\s*b"\\x7f"/,
    /bad_excessive_padding[\s\S]*_privacy_norito_frame_with_padding\(0x52,\s*65\)/,
    /bad_flags[\s\S]*\[39\]\s*=\s*0x08/,
    /bad_field_bitset_flags[\s\S]*\[39\]\s*=\s*0x20/,
    /bad_checksum[\s\S]*\[31\]\s*\^=/,
    /bad_payload[\s\S]*\[44\]\s*\^=/,
  ]) {
    assert.match(fixtures, pattern);
  }

  const requestTest = sliceBetween(
    pythonTests,
    "def test_privacy_native_build_and_verify_reject_invalid_request_archive",
    "def test_privacy_native_build_and_verify_clear_temporary_request_copy",
    "Python invalid privacy request test",
  );
  assert.match(requestTest, /_FakePrivacyNativeMustNotDispatch\(\)/);
  assert.match(requestTest, /_malformed_privacy_request_archives\(\)/);
  assert.match(requestTest, /privacy_build_proof_v1\(malformed_archive\)/);
  assert.match(requestTest, /privacy_verify_proof_v1\(bytearray\(malformed_archive\)\)/);
});

test("JS and Python privacy native tests pin sliced byte-view request handling", () => {
  for (const [label, text, testName] of [
    [
      "JS source privacy native tests",
      source("javascript/iroha_js/test/privacyNative.test.js"),
      'test("privacy native wrappers respect sliced request archive views"',
    ],
    [
      "JS package dist privacy native tests",
      source("javascript/iroha_js/test/package_dist.test.js"),
      'test("package dist privacy native wrappers respect sliced request archive views"',
    ],
  ]) {
    const requestTest = sliceBetween(
      text,
      testName,
      "test(",
      `${label} sliced request archive view test`,
    );
    assert.match(requestTest, /slicedPrivacyView\(PRIVACY_REQUEST_ARCHIVE\)/);
    assert.match(requestTest, /new DataView\(/);
    assert.match(requestTest, /Buffer\.from\(request\)[\s\S]*PRIVACY_REQUEST_ARCHIVE/);
    assert.match(requestTest, /privacyBuildProofV1\(buildView\)/);
    assert.match(requestTest, /privacyVerifyProofV1\(verifyView\)/);
    assert.match(requestTest, /buildRequest\.every\(\(value\)\s*=>\s*value\s*===\s*0\)/);
    assert.match(requestTest, /verifyRequest\.every\(\(value\)\s*=>\s*value\s*===\s*0\)/);
  }

  const pythonTests = source("python/iroha_python/tests/crypto_algorithms_test.py");
  const pythonRequestTest = sliceBetween(
    pythonTests,
    "def test_privacy_native_build_and_verify_respect_sliced_request_views",
    "def test_privacy_native_wrappers_reject_textual_native_output",
    "Python sliced request archive view test",
  );
  assert.match(pythonTests, /def _sliced_privacy_memoryview/);
  assert.match(pythonRequestTest, /_sliced_privacy_memoryview\(_PRIVACY_REQUEST_ARCHIVE\)/);
  assert.match(pythonRequestTest, /prefix=b"\\x99\\x88"/);
  assert.match(pythonRequestTest, /privacy_build_proof_v1\(build_view\)/);
  assert.match(pythonRequestTest, /privacy_verify_proof_v1\(verify_view\)/);
  assert.match(pythonRequestTest, /all\(value == 0 for value in request\)/);
});

test("JS and Python privacy native tests pin sliced byte-view native output handling", () => {
  for (const [label, text, testName] of [
    [
      "JS source privacy native tests",
      source("javascript/iroha_js/test/privacyNative.test.js"),
      'test("privacy native wrappers respect sliced native output archive views"',
    ],
    [
      "JS package dist privacy native tests",
      source("javascript/iroha_js/test/package_dist.test.js"),
      'test("package dist privacy native wrappers respect sliced native output archive views"',
    ],
  ]) {
    const outputTest = sliceBetween(
      text,
      testName,
      "test(",
      `${label} sliced native output archive view test`,
    );
    assert.match(outputTest, /capabilitiesBacking\.subarray\(/);
    assert.match(outputTest, /new DataView\([\s\S]*buildBacking\.buffer/);
    assert.match(outputTest, /verifyBacking\.subarray\(/);
    assert.match(outputTest, /privacyCapabilitiesV1\(\)/);
    assert.match(outputTest, /privacyBuildProofV1\(PRIVACY_REQUEST_ARCHIVE\)/);
    assert.match(outputTest, /privacyVerifyProofV1\(PRIVACY_REQUEST_ARCHIVE\)/);
    assert.match(outputTest, /capabilitiesBacking\[prefixLength\]\s*=\s*0x00/);
    assert.match(outputTest, /buildBacking\[prefixLength\]\s*=\s*0x00/);
    assert.match(outputTest, /verifyBacking\[prefixLength\]\s*=\s*0x00/);
  }

  const pythonTests = source("python/iroha_python/tests/crypto_algorithms_test.py");
  const pythonOutputTest = sliceBetween(
    pythonTests,
    "def test_privacy_native_wrappers_respect_sliced_native_output_views",
    "def test_privacy_native_wrappers_sanitize_native_exceptions_before_exposing_request_bytes",
    "Python sliced native output archive view test",
  );
  assert.match(pythonTests, /class _FakeSlicedOutputPrivacyNative/);
  assert.match(pythonTests, /memoryview\(self\.capabilities_backing\)/);
  assert.match(pythonTests, /memoryview\(self\.build_backing\)/);
  assert.match(pythonTests, /memoryview\(self\.verify_backing\)/);
  assert.match(pythonOutputTest, /privacy_capabilities_v1\(\)/);
  assert.match(pythonOutputTest, /privacy_build_proof_v1\(_PRIVACY_REQUEST_ARCHIVE\)/);
  assert.match(pythonOutputTest, /privacy_verify_proof_v1\(_PRIVACY_REQUEST_ARCHIVE\)/);
  assert.match(pythonOutputTest, /native\.capabilities_backing\[native\.prefix_len\]\s*=\s*0x00/);
  assert.match(pythonOutputTest, /native\.build_backing\[native\.prefix_len\]\s*=\s*0x00/);
  assert.match(pythonOutputTest, /native\.verify_backing\[native\.prefix_len\]\s*=\s*0x00/);
});

test("mobile and C# privacy native tests reject adversarial malformed request archives before dispatch", () => {
  const surfaces = [
    [
      "Java Android privacy native tests",
      source("java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridgeTest.java"),
      "rejectsInvalidNoritoRequestsBeforeNativeDispatch",
      "private static void assertThrows",
      [
        "new byte[] {1}",
        "invalidPrivacyNoritoFrame(0, 'X')",
        "invalidPrivacyNoritoFrame(4, 1)",
        "invalidPrivacyNoritoFrame(5, 1)",
        "invalidPrivacyNoritoFrame(22, 1)",
        "invalidPrivacyNoritoDeclaredPayloadLength(0x52)",
        "invalidPrivacyNoritoOversizedPayloadLength(0x52)",
        "invalidPrivacyNoritoFrame(39, 0x40)",
        "invalidPrivacyNoritoFrame(39, 0x20)",
        "invalidPrivacyNoritoWithNonzeroPadding()",
        "invalidPrivacyNoritoWithExcessivePadding()",
        "invalidPrivacyNoritoFrame(31, 1)",
        "invalidPrivacyNoritoPayloadTamper()",
        "Arrays.copyOf(malformedArchive, malformedArchive.length)",
      ],
    ],
    [
      "Kotlin/JVM privacy native tests",
      source("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridgeTest.kt"),
      "fun rejectsInvalidNoritoRequestsBeforeNativeDispatch",
      "private fun assertFailClosedProductionGate",
      [
        "byteArrayOf(1)",
        "invalidPrivacyNoritoFrame(0, 'X'.code)",
        "invalidPrivacyNoritoFrame(4, 1)",
        "invalidPrivacyNoritoFrame(5, 1)",
        "invalidPrivacyNoritoFrame(22, 1)",
        "invalidPrivacyNoritoDeclaredPayloadLength(0x52)",
        "invalidPrivacyNoritoOversizedPayloadLength(0x52)",
        "invalidPrivacyNoritoFrame(39, 0x40)",
        "invalidPrivacyNoritoFrame(39, 0x20)",
        "invalidPrivacyNoritoWithNonzeroPadding()",
        "invalidPrivacyNoritoWithExcessivePadding()",
        "invalidPrivacyNoritoFrame(31, 1)",
        "invalidPrivacyNoritoPayloadTamper()",
        "malformedArchive.copyOf()",
      ],
    ],
    [
      "C# privacy native tests",
      source("csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs"),
      "public void PrivacyNativeRejectsInvalidProofRequestArchivesBeforeNativeDispatch",
      "public void PrivacyNativeProbeRequiresSuccessfulNonemptyOutput",
      [
        "yield return new byte[] { 0x01 }",
        "InvalidPrivacyNoritoFrame(0, (byte)'X')",
        "InvalidPrivacyNoritoFrame(4, 1)",
        "InvalidPrivacyNoritoFrame(5, 1)",
        "InvalidPrivacyNoritoFrame(22, 1)",
        "InvalidPrivacyNoritoDeclaredPayloadLength(0x52)",
        "InvalidPrivacyNoritoOversizedPayloadLength(0x52)",
        "InvalidPrivacyNoritoFrame(39, 0x40)",
        "InvalidPrivacyNoritoFrame(39, 0x20)",
        "InvalidPrivacyNoritoWithNonzeroPadding()",
        "InvalidPrivacyNoritoWithExcessivePadding()",
        "InvalidPrivacyNoritoFrame(31, 1)",
        "InvalidPrivacyNoritoPayloadTamper()",
        "InvalidPrivacyRequestArchives()",
      ],
    ],
  ];

  for (const [label, text, requestStart, requestEnd, fragments] of surfaces) {
    const requestTest = sliceBetween(text, requestStart, requestEnd, `${label} invalid request test`);
    for (const fragment of fragments) {
      assert.ok(
        text.includes(fragment) || requestTest.includes(fragment),
        `${label} must include ${fragment}`,
      );
    }
    assert.match(
      requestTest,
      /invalid build request (?:must not reach|reached) native dispatch/,
      `${label} must fail if invalid build requests reach native dispatch`,
    );
    assert.match(
      requestTest,
      /invalid verify request (?:must not reach|reached) native dispatch/,
      `${label} must fail if invalid verify requests reach native dispatch`,
    );
    assert.match(
      requestTest,
      /valid Norito V1 archive/,
      `${label} must assert the malformed request diagnostic`,
    );
  }
});

test("SDK privacy native tests pin Norito header padding boundaries", () => {
  const surfaces = [
    [
      "JS source privacy native tests",
      source("javascript/iroha_js/test/privacyNative.test.js"),
      [
        "function privacyNoritoFrameWithPadding",
        "privacyNoritoFrameWithPadding(0x52, 64)",
        "privacyNoritoFrameWithPadding(0x52, 65)",
      ],
    ],
    [
      "JS package dist privacy native tests",
      source("javascript/iroha_js/test/package_dist.test.js"),
      [
        "function privacyNoritoFrameWithPadding",
        "privacyNoritoFrameWithPadding(0x52, 65)",
      ],
    ],
    [
      "Python privacy native tests",
      source("python/iroha_python/tests/crypto_algorithms_test.py"),
      [
        "def _privacy_norito_frame_with_padding",
        "_privacy_norito_frame_with_padding(0x52, 64)",
        "_privacy_norito_frame_with_padding(0x52, 65)",
      ],
    ],
    [
      "Swift privacy native tests",
      source("IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift"),
      [
        "private func privacyNoritoFrameWithPadding",
        "privacyNoritoFrameWithPadding(0x52, paddingLength: 64)",
        "privacyNoritoFrameWithPadding(0x50, paddingLength: 65)",
      ],
    ],
    [
      "Java Android privacy native tests",
      source("java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridgeTest.java"),
      [
        "private static byte[] privacyNoritoFrameWithPadding",
        "privacyNoritoFrameWithPadding(0x52, 64)",
        "privacyNoritoFrameWithPadding(0x50, 65)",
      ],
    ],
    [
      "Kotlin/JVM privacy native tests",
      source("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridgeTest.kt"),
      [
        "private fun privacyNoritoFrameWithPadding",
        "privacyNoritoFrameWithPadding(0x52, 64)",
        "privacyNoritoFrameWithPadding(0x50, 65)",
      ],
    ],
    [
      "C# privacy native tests",
      source("csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs"),
      [
        "private static byte[] PrivacyNoritoFrameWithPadding",
        "PrivacyNoritoFrameWithPadding(0x52, 64)",
        "PrivacyNoritoFrameWithPadding(0x50, 65)",
      ],
    ],
  ];

  for (const [label, text, fragments] of surfaces) {
    for (const fragment of fragments) {
      assert.ok(text.includes(fragment), `${label} must pin ${fragment}`);
    }
  }
});

test("SDK privacy native tests accept complete Norito field-bitset flags", () => {
  const surfaces = [
    [
      "JS source privacy native tests",
      source("javascript/iroha_js/test/privacyNative.test.js"),
      [
        "function privacyNoritoFrameWithFlags",
        'test("privacy native wrappers accept complete field-bitset Norito flags"',
        "privacyNoritoFrameWithFlags(0x52, 0x26)",
        "privacyNoritoFrameWithFlags(0x42, 0x26)",
      ],
    ],
    [
      "JS package dist privacy native tests",
      source("javascript/iroha_js/test/package_dist.test.js"),
      [
        "function privacyNoritoFrameWithFlags",
        'test("package dist privacy native wrappers accept complete field-bitset flags"',
        "privacyNoritoFrameWithFlags(0x52, 0x26)",
        "privacyNoritoFrameWithFlags(0x42, 0x26)",
      ],
    ],
    [
      "Python privacy native tests",
      source("python/iroha_python/tests/crypto_algorithms_test.py"),
      [
        "def _privacy_norito_frame_with_flags",
        "def test_privacy_native_build_and_verify_accept_complete_field_bitset_flags",
        "_privacy_norito_frame_with_flags(0x52, 0x26)",
        "_privacy_norito_frame_with_flags(0x42, 0x26)",
      ],
    ],
    [
      "Swift privacy native tests",
      source("IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift"),
      [
        "private func privacyNoritoFrameWithFlags",
        "privacyNoritoFrameWithFlags(0x52, flags: 0x26)",
        "privacyNoritoFrameWithFlags(0x42, flags: 0x26)",
      ],
    ],
    [
      "Java Android privacy native tests",
      source("java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridgeTest.java"),
      [
        "acceptsCompleteFieldBitsetNoritoFlags();",
        "private static byte[] privacyNoritoFrameWithFlags",
        "privacyNoritoFrameWithFlags(0x52, 0x26)",
        "privacyNoritoFrameWithFlags(0x42, 0x26)",
      ],
    ],
    [
      "Kotlin/JVM privacy native tests",
      source("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridgeTest.kt"),
      [
        "fun acceptsCompleteFieldBitsetNoritoFlags",
        "private fun privacyNoritoFrameWithFlags",
        "privacyNoritoFrameWithFlags(0x52, 0x26)",
        "privacyNoritoFrameWithFlags(0x42, 0x26)",
      ],
    ],
    [
      "C# privacy native tests",
      source("csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs"),
      [
        "PrivacyNativeAcceptsCompleteFieldBitsetProofRequestArchives",
        "private static byte[] PrivacyNoritoFrameWithFlags",
        "PrivacyNoritoFrameWithFlags(0x52, 0x26)",
        "PrivacyNoritoFrameWithFlags(0x42, 0x26)",
      ],
    ],
  ];

  for (const [label, text, fragments] of surfaces) {
    for (const fragment of fragments) {
      assert.ok(text.includes(fragment), `${label} must accept ${fragment}`);
    }
  }
});

test("SDK privacy native tests reject wrong-schema request archives before dispatch", () => {
  const surfaces = [
    [
      "JS source privacy native tests",
      source("javascript/iroha_js/test/privacyNative.test.js"),
      'test("privacy native wrappers require binary Norito request archives"',
      'test("privacy native wrappers reject missing and empty native output"',
      [
        "wrongSchemaPrivacyRequestArchives",
        "privacyNoritoFrameWithSchemaOverride(0x52, 6, 0x42)",
        "privacyNoritoFrameWithSchemaOverride(0x52, 21, 0x56)",
        "requestArchive must use the privacy request schema",
      ],
    ],
    [
      "Swift privacy native tests",
      source("IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift"),
      "func testRejectsWrongSchemaRequestArchivesBeforeBridgeCall",
      "func testRejectsInvalidNoritoNativeOutput",
      [
        "privacyNoritoFrameWithPayload(0x50)",
        "privacyNoritoFrameWithPayload(0x42)",
        "privacyNoritoFrameWithPayload(0x56)",
        "privacyNoritoFrameWithSchemaOverride(0x52, offset: 6, value: 0x42)",
        "privacyNoritoFrameWithSchemaOverride(0x52, offset: 21, value: 0x56)",
        "wrong-schema request must not reach native dispatch",
      ],
    ],
    [
      "Java Android privacy native tests",
      source("java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridgeTest.java"),
      "rejectsWrongSchemaRequestsBeforeNativeDispatch",
      "private static void assertThrows",
      [
        "privacyNoritoFrameWithPayload(0x50)",
        "privacyNoritoFrameWithPayload(0x42)",
        "privacyNoritoFrameWithPayload(0x56)",
        "privacyNoritoFrameWithSchemaOverride(0x52, 6, 0x42)",
        "privacyNoritoFrameWithSchemaOverride(0x52, 21, 0x56)",
        "wrong-schema build request must not reach native dispatch",
        "wrong-schema verify request must not reach native dispatch",
        "requestArchive must use the privacy request schema",
      ],
    ],
    [
      "Kotlin/JVM privacy native tests",
      source("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridgeTest.kt"),
      "fun rejectsWrongSchemaRequestsBeforeNativeDispatch",
      "private fun assertFailClosedProductionGate",
      [
        "privacyNoritoFrameWithPayload(0x50)",
        "privacyNoritoFrameWithPayload(0x42)",
        "privacyNoritoFrameWithPayload(0x56)",
        "privacyNoritoFrameWithSchemaOverride(0x52, 6, 0x42)",
        "privacyNoritoFrameWithSchemaOverride(0x52, 21, 0x56)",
        "wrong-schema build request must not reach native dispatch",
        "wrong-schema verify request must not reach native dispatch",
        "requestArchive must use the privacy request schema",
      ],
    ],
    [
      "C# privacy native tests",
      source("csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs"),
      "public void PrivacyNativeRejectsWrongSchemaProofRequestArchivesBeforeNativeDispatch",
      "public void PrivacyNativeProbeRequiresSuccessfulNonemptyOutput",
      [
        "PrivacyNoritoFrameWithPayload(0x50)",
        "PrivacyNoritoFrameWithPayload(0x42)",
        "PrivacyNoritoFrameWithPayload(0x56)",
        "PrivacyNoritoFrameWithSchemaOverride(0x52, 6, 0x42)",
        "PrivacyNoritoFrameWithSchemaOverride(0x52, 21, 0x56)",
        "wrong-schema build request reached native dispatch",
        "wrong-schema verify request reached native dispatch",
        "privacy request schema",
      ],
    ],
  ];

  for (const [label, text, requestStart, requestEnd, fragments] of surfaces) {
    const requestTest = sliceBetween(text, requestStart, requestEnd, `${label} wrong-schema request test`);
    for (const fragment of fragments) {
      assert.ok(
        text.includes(fragment) || requestTest.includes(fragment),
        `${label} must include ${fragment}`,
      );
    }
  }
});

test("SDK privacy native tests defensively copy native output archives", () => {
  const surfaces = [
    [
      "JS source privacy native tests",
      source("javascript/iroha_js/test/privacyNative.test.js"),
      'test("privacy native wrappers defensively copy native output archives"',
      'test("privacy native wrappers clear temporary request copies after native dispatch"',
      [
        /assert\.notEqual\(capabilitiesArchive,\s*capabilitiesOutput\)/,
        /capabilitiesArchive\[0\]\s*=\s*0x7f[\s\S]*assert\.deepEqual\(capabilitiesOutput,\s*PRIVACY_CAPABILITIES_ARCHIVE\)/,
        /verifyBacking\[1\]\s*=\s*0x7f[\s\S]*assert\.deepEqual\(verifyArchive,\s*PRIVACY_VERIFY_ARCHIVE\)/,
      ],
    ],
    [
      "JS package dist privacy native tests",
      source("javascript/iroha_js/test/package_dist.test.js"),
      'test("package dist privacy native wrappers defensively copy native output archives"',
      'test("package declarations mark privacy capability metadata readonly"',
      [
        /assert\.notEqual\(capabilitiesArchive,\s*capabilitiesOutput\)/,
        /capabilitiesArchive\[0\]\s*=\s*0x7f[\s\S]*assert\.deepEqual\(capabilitiesOutput,\s*PRIVACY_CAPABILITIES_ARCHIVE\)/,
        /verifyBacking\[1\]\s*=\s*0x7f[\s\S]*assert\.deepEqual\(verifyArchive,\s*PRIVACY_VERIFY_ARCHIVE\)/,
      ],
    ],
    [
      "Python privacy native tests",
      source("python/iroha_python/tests/crypto_algorithms_test.py"),
      "def test_privacy_native_wrappers_defensively_copy_native_output_archives",
      "def test_privacy_native_wrappers_sanitize_native_exceptions_before_exposing_request_bytes",
      [
        /native\.capabilities_output\[0\]\s*=\s*0x7F/,
        /native\.build_output\[0\]\s*=\s*0x7F/,
        /native\.verify_backing\[1\]\s*=\s*0x7F/,
        /assert capabilities == _PRIVACY_CAPABILITIES_ARCHIVE[\s\S]*assert build == _PRIVACY_BUILD_ARCHIVE[\s\S]*assert verify == _PRIVACY_VERIFY_ARCHIVE/,
      ],
    ],
    [
      "Swift privacy native tests",
      source("IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift"),
      "func testReadPrivacyNativeOutputCopiesBeforeFreeCallbackCanMutateBuffer",
      "func testReadPrivacyNativeOutputRejectsInvalidArchiveAndFreesPointer",
      [
        /update\(repeating:\s*0x7F,\s*count:\s*bytes\.count\)/,
        /XCTAssertEqual\(archive,\s*Data\(bytes\)\)/,
        /XCTAssertTrue\(freed\)/,
      ],
    ],
    [
      "Java Android privacy native tests",
      source("java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridgeTest.java"),
      "private static void nativeDispatchReturnsDefensiveOutputCopy",
      "private static void nativeExceptionsAreSanitizedBeforeExposingRequestBytes",
      [
        /assert archive != nativeOutput/,
        /nativeOutput\[6\]\s*=\s*0x7f[\s\S]*assert Arrays\.equals\(archive,\s*expectedOutput\)/,
        /archive\[0\]\s*=\s*0x7f[\s\S]*assert nativeOutput\[0\]\s*==\s*'N'/,
      ],
    ],
    [
      "Kotlin/JVM privacy native tests",
      source("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridgeTest.kt"),
      "fun nativeDispatchReturnsDefensiveOutputCopy",
      "fun nativeExceptionsAreSanitizedBeforeExposingRequestBytes",
      [
        /assertTrue\(archive !== nativeOutput\)/,
        /nativeOutput\[6\]\s*=\s*0x7f[\s\S]*assertTrue\(archive\.contentEquals\(expectedOutput\)\)/,
        /archive\[0\]\s*=\s*0x7f[\s\S]*assertEquals\('N'\.code\.toByte\(\),\s*nativeOutput\[0\]\)/,
      ],
    ],
    [
      "C# privacy native tests",
      source("csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs"),
      "public void PrivacyNativeReadOutputCopiesArchiveBeforeFreeCallbackCanMutateBuffer",
      "public void PrivacyNativeReadOutputRejectsInvalidNoritoArchiveAndFreesPointer",
      [
        /Marshal\.Copy\(FilledBytes\(0x7f,\s*bytes\.Length\),\s*0,\s*ptr,\s*bytes\.Length\)/,
        /Assert\.Equal\(bytes,\s*archive\)/,
        /Assert\.True\(freed\)/,
      ],
    ],
  ];

  for (const [label, text, start, end, patterns] of surfaces) {
    const testBody = sliceBetween(text, start, end, `${label} defensive output copy test`);
    for (const pattern of patterns) {
      assert.match(testBody, pattern, `${label} must prove native output bytes are copied defensively`);
    }
  }
});

test("SDK privacy native tests reject malformed native output archives", () => {
  const surfaces = [
    [
      "JS source privacy native tests",
      source("javascript/iroha_js/test/privacyNative.test.js"),
      'test("privacy native wrappers reject invalid Norito-framed native output"',
      'test("privacy native wrappers reject textual native output"',
      [
        /badMinorVersion[\s\S]*\[5\]\s*=\s*1/,
        /badDeclaredPayloadLength[\s\S]*privacyNoritoFrameWithDeclaredPayloadLength\(0x42,\s*6n\)/,
        /badOversizedDeclaredPayloadLength[\s\S]*privacyNoritoFrameWithDeclaredPayloadLength\(\s*0x42,\s*0x8000000000000000n/,
        /badFieldBitsetFlags[\s\S]*\[39\]\s*=\s*0x20/,
        /badChecksum[\s\S]*\[31\]/,
        /badPayload[\s\S]*\[44\]\s*\^=/,
      ],
    ],
    [
      "JS package dist privacy native tests",
      source("javascript/iroha_js/test/package_dist.test.js"),
      'test("package dist privacy native wrappers reject invalid Norito-framed output archives"',
      'test("package dist privacy native wrappers reject oversized request archives"',
      [
        /badMinorVersion[\s\S]*\[5\]\s*=\s*1/,
        /badDeclaredPayloadLength[\s\S]*privacyNoritoFrameWithDeclaredPayloadLength\(0x42,\s*6n\)/,
        /badOversizedDeclaredPayloadLength[\s\S]*privacyNoritoFrameWithDeclaredPayloadLength\(\s*0x42,\s*0x8000000000000000n/,
        /badFieldBitsetFlags[\s\S]*\[39\]\s*=\s*0x20/,
        /badChecksum[\s\S]*\[31\]/,
        /badPayload[\s\S]*\[44\]\s*\^=/,
      ],
    ],
    [
      "Python privacy native tests",
      source("python/iroha_python/tests/crypto_algorithms_test.py"),
      "def test_privacy_native_wrappers_reject_invalid_norito_native_output",
      "def test_privacy_native_wrappers_defensively_copy_native_output_archives",
      [
        /bad_minor_version[\s\S]*\[5\]\s*=\s*1/,
        /bad_declared_payload_length[\s\S]*_privacy_norito_frame_with_declared_payload_length\(0x42,\s*6\)/,
        /bad_oversized_declared_payload_length[\s\S]*_privacy_norito_frame_with_declared_payload_length\(\s*0x42,\s*0x8000000000000000/,
        /bad_field_bitset_flags[\s\S]*\[39\]\s*=\s*0x20/,
        /bad_checksum[\s\S]*\[31\]/,
        /bad_payload[\s\S]*\[44\]\s*\^=/,
      ],
    ],
    [
      "Swift privacy native tests",
      source("IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift"),
      "func testRejectsInvalidNoritoNativeOutput",
      "func testRejectsWrongOperationSchemaNativeOutputs",
      [
        /invalidPrivacyNoritoFrame\(offset:\s*5,\s*value:\s*1\)/,
        /invalidPrivacyNoritoDeclaredPayloadLength\(schemaByte:\s*0x42\)/,
        /invalidPrivacyNoritoOversizedPayloadLength\(schemaByte:\s*0x42\)/,
        /invalidPrivacyNoritoFrame\(offset:\s*39,\s*value:\s*0x20\)/,
        /invalidPrivacyNoritoFrame\(offset:\s*31,\s*value:\s*1\)/,
        /invalidPrivacyNoritoPayloadTamper\(\)/,
      ],
    ],
    [
      "Java Android privacy native tests",
      source("java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridgeTest.java"),
      "private static void rejectsInvalidNoritoNativeOutputs",
      "private static void rejectsWrongOperationSchemaNativeOutputs",
      [
        "invalidPrivacyNoritoFrame(5, 1)",
        "invalidPrivacyNoritoDeclaredPayloadLength(0x42)",
        "invalidPrivacyNoritoOversizedPayloadLength(0x42)",
        "invalidPrivacyNoritoFrame(39, 0x20)",
        "invalidPrivacyNoritoFrame(31, 1)",
        "invalidPrivacyNoritoPayloadTamper()",
      ],
    ],
    [
      "Kotlin/JVM privacy native tests",
      source("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridgeTest.kt"),
      "fun rejectsInvalidNoritoNativeOutputs",
      "fun rejectsWrongOperationSchemaNativeOutputs",
      [
        "invalidPrivacyNoritoFrame(5, 1)",
        "invalidPrivacyNoritoDeclaredPayloadLength(0x42)",
        "invalidPrivacyNoritoOversizedPayloadLength(0x42)",
        "invalidPrivacyNoritoFrame(39, 0x20)",
        "invalidPrivacyNoritoFrame(31, 1)",
        "invalidPrivacyNoritoPayloadTamper()",
      ],
    ],
    [
      "C# privacy native tests",
      source("csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs"),
      "public void PrivacyNativeReadOutputRejectsInvalidNoritoArchiveAndFreesPointer",
      "public void PrivacyNativeReadOutputRejectsWrongOperationSchemaAndFreesPointer",
      [
        "InvalidPrivacyNativeOutputArchives()",
        "InvalidPrivacyNoritoFrame(5, 1)",
        "InvalidPrivacyNoritoDeclaredPayloadLength(0x50)",
        "InvalidPrivacyNoritoOversizedPayloadLength(0x50)",
        "InvalidPrivacyNoritoFrame(39, 0x20)",
        "InvalidPrivacyNoritoFrame(31, 1)",
        "InvalidPrivacyNoritoPayloadTamper()",
      ],
    ],
  ];

  for (const [label, text, start, end, fragments] of surfaces) {
    const testBody = sliceBetween(text, start, end, `${label} malformed native output test`);
    for (const fragment of fragments) {
      if (fragment instanceof RegExp) {
        assert.match(testBody, fragment, `${label} must reject malformed native output ${fragment}`);
      } else {
        assert.ok(
          text.includes(fragment) || testBody.includes(fragment),
          `${label} must reject malformed native output ${fragment}`,
        );
      }
    }
  }
});

test("SDK privacy native availability probes reject adversarial native output archives", () => {
  const surfaces = [
    [
      "JS source privacy native tests",
      source("javascript/iroha_js/test/privacyNative.test.js"),
      'test("privacy native availability probes reject unsafe raw output"',
      'test("privacy native wrappers return opaque archive bytes"',
      [
        "malformedPrivacyNativeOutputArchives(0x50)",
        "malformedPrivacyNativeOutputArchives(0x42)",
        "malformedPrivacyNativeOutputArchives(0x56)",
        /badMinorVersion[\s\S]*\[5\]\s*=\s*1/,
        /badDeclaredPayloadLength[\s\S]*privacyNoritoFrameWithDeclaredPayloadLength\(\s*schemaByte,\s*6n/,
        /badOversizedDeclaredPayloadLength[\s\S]*0x8000000000000000n/,
        /badFieldBitsetFlags[\s\S]*\[39\]\s*=\s*0x20/,
        /badChecksum[\s\S]*\[31\]/,
        /badPayload[\s\S]*\[44\]\s*\^=/,
        "Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f)",
      ],
    ],
    [
      "JS package dist privacy native tests",
      source("javascript/iroha_js/test/package_dist.test.js"),
      'test("package dist privacy native availability probes reject unsafe raw output"',
      'test("package dist privacy native wrappers reject wrong-operation result schemas"',
      [
        "malformedPrivacyNativeOutputArchives(0x50)",
        "malformedPrivacyNativeOutputArchives(0x42)",
        "malformedPrivacyNativeOutputArchives(0x56)",
        /badMinorVersion[\s\S]*\[5\]\s*=\s*1/,
        /badDeclaredPayloadLength[\s\S]*privacyNoritoFrameWithDeclaredPayloadLength\(\s*schemaByte,\s*6n/,
        /badOversizedDeclaredPayloadLength[\s\S]*0x8000000000000000n/,
        /badFieldBitsetFlags[\s\S]*\[39\]\s*=\s*0x20/,
        /badChecksum[\s\S]*\[31\]/,
        /badPayload[\s\S]*\[44\]\s*\^=/,
        "Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f)",
      ],
    ],
    [
      "Python privacy native tests",
      source("python/iroha_python/tests/crypto_algorithms_test.py"),
      "def test_privacy_native_availability_probes_reject_unsafe_raw_output",
      "def test_privacy_native_wrappers_reject_wrong_operation_result_schemas",
      [
        "_malformed_privacy_native_output_archives(0x50)",
        "_malformed_privacy_native_output_archives(0x42)",
        "_malformed_privacy_native_output_archives(0x56)",
        /bad_minor_version[\s\S]*\[5\]\s*=\s*1/,
        /bad_declared_payload_length[\s\S]*_privacy_norito_frame_with_declared_payload_length\(\s*schema_byte,\s*6/,
        /bad_oversized_declared_payload_length[\s\S]*0x8000000000000000/,
        /bad_field_bitset_flags[\s\S]*\[39\]\s*=\s*0x20/,
        /bad_checksum[\s\S]*\[31\]/,
        /bad_payload[\s\S]*\[44\]\s*\^=/,
        'monkeypatch.setattr(crypto_module, "PRIVACY_NATIVE_ARCHIVE_MAX_BYTES", 2)',
      ],
    ],
    [
      "Swift privacy native tests",
      source("IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift"),
      "func testPrivacyNativeProbeResultRequiresSuccessfulNonemptyArchive",
      "func testPrivacyCapabilitiesRemainFailClosedWithBridgeAvailable",
      [
        "invalidPrivacyNoritoFrame(offset: 5, value: 1)",
        "invalidPrivacyNoritoDeclaredPayloadLength()",
        "invalidPrivacyNoritoOversizedPayloadLength()",
        "invalidPrivacyNoritoFrame(offset: 39, value: 0x20)",
        "invalidPrivacyNoritoFrame(offset: 31, value: 1)",
        "invalidPrivacyNoritoPayloadTamper()",
        "privacyNativeArchiveMaxBytes + 1",
      ],
    ],
    [
      "Java Android privacy native tests",
      source("java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridgeTest.java"),
      "private static void nativeProbeRequiresAbiAndAllPrivacySymbols",
      "private static void rejectsNullAndEmptyNativeOutputs",
      [
        "invalidPrivacyNoritoFrame(5, 1)",
        "invalidPrivacyNoritoDeclaredPayloadLength(0x50)",
        "invalidPrivacyNoritoOversizedPayloadLength(0x50)",
        "invalidPrivacyNoritoFrame(39, 0x20)",
        "invalidPrivacyNoritoFrame(31, 1)",
        "invalidPrivacyNoritoPayloadTamper()",
        "PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1",
      ],
    ],
    [
      "Kotlin/JVM privacy native tests",
      source("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridgeTest.kt"),
      "fun nativeProbeRequiresAbiAndAllPrivacySymbols",
      "fun rejectsNullAndEmptyNativeOutputs",
      [
        "invalidPrivacyNoritoFrame(5, 1)",
        "invalidPrivacyNoritoDeclaredPayloadLength()",
        "invalidPrivacyNoritoOversizedPayloadLength()",
        "invalidPrivacyNoritoFrame(39, 0x20)",
        "invalidPrivacyNoritoFrame(31, 1)",
        "invalidPrivacyNoritoPayloadTamper()",
        "PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1",
      ],
    ],
    [
      "C# privacy native tests",
      source("csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs"),
      "public void PrivacyNativeProbeRequiresSuccessfulNonemptyOutput",
      "public void PrivacyNativeAvailabilityProbeArchiveIsStableAndDefensive",
      [
        "InvalidPrivacyNoritoFrame(5, 1)",
        "InvalidPrivacyNoritoDeclaredPayloadLength(0x50)",
        "InvalidPrivacyNoritoOversizedPayloadLength(0x50)",
        "InvalidPrivacyNoritoFrame(39, 0x20)",
        "InvalidPrivacyNoritoFrame(31, 1)",
        "InvalidPrivacyNoritoPayloadTamper()",
        "PrivacyNativeArchiveMaxBytes + 1",
      ],
    ],
  ];

  for (const [label, text, start, end, fragments] of surfaces) {
    const testBody = sliceBetween(text, start, end, `${label} availability probe output test`);
    for (const fragment of fragments) {
      if (fragment instanceof RegExp) {
        assert.match(text, fragment, `${label} must reject availability probe fragment ${fragment}`);
      } else {
        assert.ok(
          text.includes(fragment) || testBody.includes(fragment),
          `${label} must reject availability probe fragment ${fragment}`,
        );
      }
    }
  }
});

test("C# public privacy archive wrappers reject malformed Norito archives", () => {
  const csharpBridge = source("csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs");
  const csharpTests = source("csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs");
  const archiveCopy = sliceBetween(
    csharpBridge,
    "internal static class PrivacyArchiveBytes",
    "public sealed class PrivacyCapabilitiesArchive",
    "C# public privacy archive wrapper copy",
  );

  assert.match(
    archiveCopy,
    /noritoBytes\.Length\s*>\s*PrivacyNative\.PrivacyNativeArchiveMaxBytes/,
    "C# public privacy archive wrappers must reject oversized Norito archives",
  );
  assert.match(
    archiveCopy,
    /PrivacyNative\.IsNoritoV1Archive\(noritoBytes\)/,
    "C# public privacy archive wrappers must validate Norito frame shape",
  );
  assert.match(
    csharpTests,
    /PrivacyNativeArchiveWrappersRejectUnsafeNoritoBytes[\s\S]*foreach\s*\(var malformed in InvalidPrivacyRequestArchives\(\)\)[\s\S]*new PrivacyCapabilitiesArchive\(malformed\)[\s\S]*new PrivacyProofResultArchive\(malformed\)[\s\S]*PrivacyNativeArchiveMaxBytes\s*\+\s*1/,
    "C# tests must reject every adversarial malformed archive and oversized public wrapper archives",
  );
});

test("Swift privacy native availability requires valid Norito proof probes", () => {
  const swiftNativeBridge = source("IrohaSwift/Sources/IrohaSwift/NativeBridge.swift");

  assert.match(
    swiftNativeBridge,
    /privacyRequestSchemaByte:\s*UInt8\s*=\s*0x52[\s\S]*privacyNativeAvailabilityProbeArchive[\s\S]*archive\[0\]\s*=\s*0x4E[\s\S]*archive\[1\]\s*=\s*0x52[\s\S]*archive\[2\]\s*=\s*0x54[\s\S]*archive\[3\]\s*=\s*0x30[\s\S]*for index in 6..<22[\s\S]*archive\[index\]\s*=\s*Self\.privacyRequestSchemaByte/,
    "Swift privacy bridge must build the shared Norito availability probe archive",
  );
  assert.match(
    swiftNativeBridge,
    /probePrivacyNativeAvailability\(\)[\s\S]*probePrivacyCapabilitiesFunction\(privacyCapabilitiesFn\)[\s\S]*probePrivacyProofFunction\(\s*privacyBuildProofFn,\s*expectedSchemaByte:\s*Self\.privacyBuildProofResultSchemaByte[\s\S]*probePrivacyProofFunction\(\s*privacyVerifyProofFn,\s*expectedSchemaByte:\s*Self\.privacyVerifyProofResultSchemaByte[\s\S]*privacyNativeProbeOk\s*=\s*available/,
    "Swift privacy bridge must probe capabilities, build, and verify with operation-specific result schemas before setting availability",
  );
  assert.match(
    swiftNativeBridge,
    /isPrivacyNativeAvailable[\s\S]*privacyCapabilitiesFn\s*!=\s*nil[\s\S]*privacyBuildProofFn\s*!=\s*nil[\s\S]*privacyVerifyProofFn\s*!=\s*nil[\s\S]*privacyNativeProbeOk/,
    "Swift privacy native availability must depend on the completed probe",
  );
  assert.match(
    swiftNativeBridge,
    /isValidPrivacyNativeProbeResult[\s\S]*status == 0[\s\S]*outLen > 0[\s\S]*isValidPrivacyNoritoArchive[\s\S]*hasPrivacyNoritoSchema/,
    "Swift privacy proof probes must require successful operation-specific Norito output",
  );
});

test("privacy native availability proof probes use shared Norito request archives", () => {
  const jsSrc = source("javascript/iroha_js/src/crypto.js");
  const jsDist = source("javascript/iroha_js/dist/crypto.js");
  const pythonCrypto = source("python/iroha_python/src/iroha_python/crypto.py");
  const swiftNativeBridge = source("IrohaSwift/Sources/IrohaSwift/NativeBridge.swift");
  const javaBridge = source(
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java",
  );
  const kotlinBridge = source(
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt",
  );
  const csharpBridge = source("csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs");
  const connectBridge = source("crates/connect_norito_bridge/src/lib.rs");
  const jsHost = source("crates/iroha_js_host/src/lib.rs");
  const pythonRust = source("python/iroha_python/iroha_python_rs/src/lib.rs");

  for (const [label, text, patterns] of [
    [
      "JS src",
      jsSrc,
      [
        /const PRIVACY_REQUEST_SCHEMA_BYTE = 0x52;/,
        /archive\.write\("NRT0", 0, "ascii"\)/,
        /archive\.fill\(PRIVACY_REQUEST_SCHEMA_BYTE, 6, 22\)/,
      ],
    ],
    [
      "JS dist",
      jsDist,
      [
        /const PRIVACY_REQUEST_SCHEMA_BYTE = 0x52;/,
        /archive\.write\("NRT0", 0, "ascii"\)/,
        /archive\.fill\(PRIVACY_REQUEST_SCHEMA_BYTE, 6, 22\)/,
      ],
    ],
    [
      "Python",
      pythonCrypto,
      [
        /_PRIVACY_REQUEST_SCHEMA_BYTE:\s*Final\[int\]\s*=\s*0x52/,
        /b"NRT0\\x00\\x00"\s*\+\s*bytes\(\[_PRIVACY_REQUEST_SCHEMA_BYTE\]\)\s*\*\s*16\s*\+\s*\(b"\\x00"\s*\*\s*18\)/,
      ],
    ],
    [
      "Swift",
      swiftNativeBridge,
      [
        /privacyRequestSchemaByte:\s*UInt8\s*=\s*0x52[\s\S]*privacyNativeAvailabilityProbeArchive[\s\S]*archive\[0\]\s*=\s*0x4E[\s\S]*archive\[1\]\s*=\s*0x52[\s\S]*archive\[2\]\s*=\s*0x54[\s\S]*archive\[3\]\s*=\s*0x30[\s\S]*for index in 6..<22[\s\S]*archive\[index\]\s*=\s*Self\.privacyRequestSchemaByte/,
      ],
    ],
    [
      "Java Android",
      javaBridge,
      [/PRIVACY_SCHEMA_REQUEST\s*=\s*0x52/, /Arrays\.fill\(archive, 6, 22, \(byte\) PRIVACY_SCHEMA_REQUEST\)/],
    ],
    [
      "Kotlin JVM",
      kotlinBridge,
      [/PRIVACY_SCHEMA_REQUEST:\s*Int\s*=\s*0x52/, /archive\.fill\(PRIVACY_SCHEMA_REQUEST\.toByte\(\), 6, 22\)/],
    ],
    [
      "C#",
      csharpBridge,
      [
        /PrivacyRequestSchemaByte\s*=\s*0x52/,
        /PrivacyNoritoMagic\.CopyTo\(archive, 0\)/,
        /Array\.Fill\(archive, PrivacyRequestSchemaByte, 6, 16\)/,
      ],
    ],
  ]) {
    for (const pattern of patterns) {
      assert.match(text, pattern, `${label} privacy availability probe archive drifted`);
    }
    assert.doesNotMatch(
      text,
      new RegExp(LEGACY_PRIVACY_MALFORMED_AVAILABILITY_PROBE_ARCHIVE),
      `${label} must not send the legacy text sentinel as the availability probe`,
    );
  }

  for (const [label, text] of [
    ["C bridge", connectBridge],
    ["JS NAPI host", jsHost],
    ["Python PyO3 host", pythonRust],
  ]) {
    assert.ok(
      text.includes(`b"${LEGACY_PRIVACY_MALFORMED_AVAILABILITY_PROBE_ARCHIVE}"`),
      `${label} native malformed privacy probe vector drifted`,
    );
    assert.match(
      text,
      /fn\s+privacy_native_availability_probe_rejects_with_malformed_error/,
      `${label} must test the native privacy availability probe result envelope`,
    );
    assert.match(
      text,
      /PRIVACY_NATIVE_AVAILABILITY_PROBE_ARCHIVE[\s\S]*PRIVACY_FFI_ERROR_MALFORMED_NORITO/,
      `${label} native privacy availability probe must remain a malformed Norito error`,
    );
    assert.match(
      text,
      /malformed Norito V1 privacy proof request/,
      `${label} native privacy availability probe must keep the deterministic error message`,
    );
  }

  assert.doesNotMatch(javaBridge, /native(?:Build|Verify)Proof\(new byte\[0\]\)/);
  assert.doesNotMatch(kotlinBridge, /native(?:Build|Verify)Proof\(ByteArray\(0\)\)/);
  assert.doesNotMatch(csharpBridge, /IsExpectedEmptyArchiveProbeResult|Array\.Empty<byte>\(\), UIntPtr\.Zero/);

  for (const [label, text] of [
    ["JS src", jsSrc],
    ["JS dist", jsDist],
  ]) {
    assert.match(
      sliceBetween(
        text,
        "function privacyNativeProbeReturnsBytes",
        "function hasPrivacyNative",
        `${label} privacy native probe`,
      ),
      /privacyNativeOutputToBuffer\(result,\s*operation\)/,
      `${label} privacy probes must route native output through the capped archive decoder`,
    );
    assert.match(
      sliceBetween(
        text,
        "function privacyNativeOutputToBuffer",
        "function invokePrivacyNative",
        `${label} privacy native output decoder`,
      ),
      /output\.length\s*>\s*PRIVACY_NATIVE_ARCHIVE_MAX_BYTES/,
      `${label} privacy native output decoder must reject oversized archives`,
    );
    assert.match(
      sliceBetween(
        text,
        "function privacyNativeOutputToBuffer",
        "function invokePrivacyNative",
        `${label} privacy native output decoder`,
      ),
      /assertPrivacyNoritoArchive\(\s*output,\s*operation,\s*"native",\s*privacyExpectedResultSchemaByte\(operation\),\s*\)/,
      `${label} privacy native output decoder must validate Norito frame headers and operation schemas`,
    );
    assert.match(
      sliceBetween(
        text,
        "function assertPrivacyNoritoArchive",
        "function invokePrivacyNative",
        `${label} privacy Norito frame validator`,
      ),
      /PRIVACY_NORITO_MAGIC[\s\S]*readBigUInt64LE\(23\)[\s\S]*privacyCrc64\(payload\)[\s\S]*readBigUInt64LE\(31\)/,
      `${label} privacy Norito frame validator must check magic, payload length, and CRC64`,
    );
  }

  assert.match(
    sliceBetween(
      pythonCrypto,
      "def _privacy_native_probe_returns_bytes",
      "def _invoke_privacy_native",
      "Python privacy native probe",
    ),
    /_privacy_output_archive\(operation,\s*result\)/,
    "Python privacy probes must route native output through the capped archive decoder",
  );
  assert.match(
    sliceBetween(
      pythonCrypto,
      "def _privacy_output_archive",
      "_PRIVACY_NATIVE_METHODS",
      "Python privacy native output decoder",
    ),
    /view\.nbytes\s*>\s*PRIVACY_NATIVE_ARCHIVE_MAX_BYTES/,
    "Python privacy native output decoder must reject oversized archives",
  );
    assert.match(
      sliceBetween(
        pythonCrypto,
        "def _privacy_output_archive",
        "_PRIVACY_NATIVE_METHODS",
        "Python privacy native output decoder",
      ),
      /_assert_privacy_norito_archive\(\s*operation,\s*archive,\s*expected_schema_byte=_privacy_expected_result_schema_byte\(operation\),\s*\)/,
      "Python privacy native output decoder must validate Norito frame headers and operation schemas",
    );
  assert.match(
    sliceBetween(
      pythonCrypto,
      "def _assert_privacy_norito_archive",
      "_PRIVACY_NATIVE_METHODS",
      "Python privacy Norito frame validator",
    ),
    /archive_view\s*=\s*memoryview\(archive\)[\s\S]*_PRIVACY_NORITO_MAGIC[\s\S]*int\.from_bytes\(archive_view\[23:31\][\s\S]*int\.from_bytes\(archive_view\[31:39\][\s\S]*_privacy_crc64\(payload\)/,
    "Python privacy Norito frame validator must check magic, payload length, and CRC64",
  );

  assert.match(
    sliceBetween(
      swiftNativeBridge,
      "static func isValidPrivacyNativeProbeResult",
      "private func probeKagemushaNativeAvailability",
      "Swift privacy native probe result",
    ),
    /outLen\s*<=\s*CUnsignedLong\(Self\.privacyNativeArchiveMaxBytes\)/,
    "Swift privacy availability probes must reject oversized archives",
  );
  assert.match(
    sliceBetween(
      swiftNativeBridge,
      "static func isValidPrivacyNativeProbeResult",
      "private func probeKagemushaNativeAvailability",
      "Swift privacy native probe result",
    ),
    /let\s+archive\s*=\s*Data\(bytes:\s*outPtr,\s*count:\s*Int\(outLen\)\)[\s\S]*Self\.isValidPrivacyNoritoArchive\(archive\)[\s\S]*Self\.hasPrivacyNoritoSchema\(archive,\s*expectedSchemaByte:\s*expectedSchemaByte\)/,
    "Swift privacy availability probes must validate Norito frame headers",
  );
  assert.match(
    sliceBetween(
      swiftNativeBridge,
      "private func probePrivacyNativeAvailability",
      "var canUseConnectCrypto",
      "Swift privacy native availability probes",
    ),
    /privacyCapabilitiesResultSchemaByte[\s\S]*privacyBuildProofResultSchemaByte[\s\S]*privacyVerifyProofResultSchemaByte/,
    "Swift privacy availability probes must require operation-specific result schemas",
  );
  assert.match(
    sliceBetween(
      swiftNativeBridge,
      "static func readPrivacyNativeOutput",
      "var canUseConnectCrypto",
      "Swift privacy native output decoder",
    ),
    /Self\.hasPrivacyNoritoSchema\(archive,\s*expectedSchemaByte:\s*expectedSchemaByte\)/,
    "Swift privacy native output decoder must reject wrong-operation result schemas",
  );
  assert.match(
    sliceBetween(
      swiftNativeBridge,
      "static func isValidPrivacyNoritoArchive",
      "#if canImport(Darwin)",
      "Swift privacy Norito frame validator",
    ),
    /privacyNoritoMagic[\s\S]*readPrivacyUInt64LittleEndian\(bytes,\s*offset:\s*23\)[\s\S]*readPrivacyUInt64LittleEndian\(bytes,\s*offset:\s*31\)[\s\S]*privacyCrc64/,
    "Swift privacy Norito frame validator must check magic, payload length, and CRC64",
  );
  assert.match(
    sliceBetween(
      source("IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift"),
      "static func call(\n        bridgeAvailable: Bool",
      "\n    }\n}",
      "Swift privacy bridge output call",
    ),
    /NoritoNativeBridge\.isValidPrivacyNoritoArchive\(archive\)/,
    "Swift public privacy bridge must validate native Norito frame output",
  );
  assert.match(
    sliceBetween(
      source("IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift"),
      "static func call(\n        bridgeAvailable: Bool",
      "\n    }\n}",
      "Swift privacy bridge output call",
    ),
    /hasPrivacyNoritoSchema\(archive,\s*expectedSchemaByte:\s*expectedSchemaByte\)/,
    "Swift public privacy bridge must reject wrong-operation result schemas",
  );

  assert.match(
    sliceBetween(
      javaBridge,
      "static byte[] requireNativeOutput",
      "private static void requireNative",
      "Java privacy native output decoder",
    ),
    /output\.length\s*>\s*PRIVACY_NATIVE_ARCHIVE_MAX_BYTES/,
    "Java privacy native output decoder must reject oversized archives",
  );
  assert.match(
    sliceBetween(
      javaBridge,
      "static byte[] requireNativeOutput",
      "private static void requireNative",
      "Java privacy native output decoder",
    ),
    /isValidPrivacyNoritoArchive\(output\)/,
    "Java privacy native output decoder must validate Norito frame headers",
  );
  assert.match(
    sliceBetween(
      javaBridge,
      "static boolean returnsOutputProbe",
      "static boolean detectNativeAvailability",
      "Java privacy native probe result",
    ),
    /output\.length\s*<=\s*PRIVACY_NATIVE_ARCHIVE_MAX_BYTES/,
    "Java privacy availability probes must reject oversized archives",
  );
  assert.match(
    sliceBetween(
      javaBridge,
      "static boolean returnsOutputProbe",
      "static boolean detectNativeAvailability",
      "Java privacy native probe result",
    ),
    /isValidPrivacyNoritoArchive\(output\)/,
    "Java privacy availability probes must validate Norito frame headers",
  );
  assert.match(
    sliceBetween(
      javaBridge,
      "static boolean isValidPrivacyNoritoArchive",
      "static boolean detectNativeAvailability",
      "Java privacy Norito frame validator",
    ),
    /PRIVACY_NORITO_MAGIC[\s\S]*readLongLittleEndian\(output,\s*23\)[\s\S]*readLongLittleEndian\(output,\s*31\)[\s\S]*privacyCrc64\(output/,
    "Java privacy Norito frame validator must check magic, payload length, and CRC64",
  );

  assert.match(
    sliceBetween(
      kotlinBridge,
      "internal fun requireNativeOutput",
      "private fun loadLibrary",
      "Kotlin privacy native output decoder",
    ),
    /output\.size\s*>\s*PRIVACY_NATIVE_ARCHIVE_MAX_BYTES/,
    "Kotlin privacy native output decoder must reject oversized archives",
  );
  assert.match(
    sliceBetween(
      kotlinBridge,
      "internal fun requireNativeOutput",
      "private fun loadLibrary",
      "Kotlin privacy native output decoder",
    ),
    /isValidPrivacyNoritoArchive\(output\)/,
    "Kotlin privacy native output decoder must validate Norito frame headers",
  );
  assert.match(
    sliceBetween(
      kotlinBridge,
      "internal fun returnsOutputProbe",
      "internal fun detectNativeAvailability",
      "Kotlin privacy native probe result",
    ),
    /output\.size\s*<=\s*PRIVACY_NATIVE_ARCHIVE_MAX_BYTES/,
    "Kotlin privacy availability probes must reject oversized archives",
  );
  assert.match(
    sliceBetween(
      kotlinBridge,
      "internal fun returnsOutputProbe",
      "internal fun detectNativeAvailability",
      "Kotlin privacy native probe result",
    ),
    /isValidPrivacyNoritoArchive\(output\)/,
    "Kotlin privacy availability probes must validate Norito frame headers",
  );
  assert.match(
    sliceBetween(
      kotlinBridge,
      "internal fun isValidPrivacyNoritoArchive",
      "internal fun detectNativeAvailability",
      "Kotlin privacy Norito frame validator",
    ),
    /PRIVACY_NORITO_MAGIC[\s\S]*readLongLittleEndian\(output,\s*23\)[\s\S]*readLongLittleEndian\(output,\s*31\)[\s\S]*privacyCrc64\(output/,
    "Kotlin privacy Norito frame validator must check magic, payload length, and CRC64",
  );

  assert.match(
    sliceBetween(
      csharpBridge,
      "private static int CheckedArchiveLength",
      "internal static bool IsValidProbeResult",
      "C# privacy native output decoder",
    ),
    /length\s*>\s*PrivacyNativeArchiveMaxBytes/,
    "C# privacy native output decoder must reject oversized archives",
  );
  assert.match(
    sliceBetween(
      csharpBridge,
      "internal static byte[] ReadPrivacyOutput",
      "private static int CheckedArchiveLength",
      "C# privacy native output decoder",
    ),
    /IsNoritoV1Archive\(result\)/,
    "C# privacy native output decoder must validate Norito frame headers",
  );
  assert.match(
    sliceBetween(
      csharpBridge,
      "internal static byte[] ReadPrivacyOutput",
      "private static int CheckedArchiveLength",
      "C# privacy native output decoder",
    ),
    /ExpectedPrivacyResultSchemas\(symbol\)[\s\S]*HasNoritoSchema\(result,\s*schemas\)/,
    "C# privacy native output decoder must reject wrong-operation result schemas",
  );
  assert.match(
    sliceBetween(
      csharpBridge,
      "internal static bool IsValidProbeResult",
      "internal static byte[] PrivacyNativeAvailabilityProbeArchive",
      "C# privacy native probe result",
    ),
    /length\s*>\s*PrivacyNativeArchiveMaxBytes/,
    "C# privacy availability probes must reject oversized archives",
  );
  assert.match(
    sliceBetween(
      csharpBridge,
      "internal static bool IsValidProbeResult",
      "internal static byte[] PrivacyNativeAvailabilityProbeArchive",
      "C# privacy native probe result",
    ),
    /Marshal\.Copy\(outPtr,\s*output[\s\S]*IsNoritoV1Archive\(output\)/,
    "C# privacy availability probes must validate Norito frame headers",
  );
  assert.match(
    sliceBetween(
      csharpBridge,
      "internal static bool IsValidProbeResult",
      "internal static byte[] PrivacyNativeAvailabilityProbeArchive",
      "C# privacy native probe result",
    ),
    /IsNoritoV1Archive\(output\)\s*&&\s*HasNoritoSchema\(output,\s*expectedSchemaBytes\)/,
    "C# privacy availability probes must reject wrong-operation result schemas",
  );
  assert.match(
    sliceBetween(
      csharpBridge,
      "private static bool TryProbeRequiredSymbols",
      "private static bool TryGetAbiVersion",
      "C# privacy native availability probes",
    ),
    /PrivacyCapabilitiesResultSchemaByte[\s\S]*PrivacyBuildProofResultSchemaByte[\s\S]*PrivacyVerifyProofResultSchemaByte/,
    "C# privacy availability probes must require operation-specific result schemas",
  );
  assert.match(
    sliceBetween(
      csharpBridge,
      "internal static bool IsNoritoV1Archive",
      "internal static byte[] PrivacyNativeAvailabilityProbeArchive",
      "C# privacy Norito frame validator",
    ),
    /PrivacyNoritoMagic[\s\S]*ReadUInt64LittleEndian\(archive,\s*23\)[\s\S]*ReadUInt64LittleEndian\(archive,\s*31\)[\s\S]*PrivacyCrc64\(archive/,
    "C# privacy Norito frame validator must check magic, payload length, and CRC64",
  );

  for (const [label, text] of [
    ["JS src", jsSrc],
    ["JS dist", jsDist],
  ]) {
    const requestArchiveDecoder = sliceBetween(
      text,
      "function toPrivacyRequestArchiveBuffer",
      "function privacyNativeOutputToBuffer",
      `${label} privacy native request decoder`,
    );
    assert.match(
      requestArchiveDecoder,
      /request\.length\s*>\s*PRIVACY_NATIVE_ARCHIVE_MAX_BYTES/,
      `${label} privacy request decoder must reject oversized archives before copying`,
    );
    const validateIndex = requestArchiveDecoder.indexOf(
      'assertPrivacyNoritoArchive(request, name, "request", PRIVACY_REQUEST_SCHEMA_BYTE);',
    );
    const copyIndex = requestArchiveDecoder.indexOf("return Buffer.from(request);");
    assert.notEqual(
      validateIndex,
      -1,
      `${label} privacy request decoder must validate Norito frame headers`,
    );
    assert.notEqual(
      copyIndex,
      -1,
      `${label} privacy request decoder must copy only capped request archives`,
    );
    assert.ok(
      validateIndex < copyIndex,
      `${label} privacy request decoder must validate Norito frame headers before copying`,
    );
  }

  const pythonRequestDecoder = sliceBetween(
    pythonCrypto,
    "def _privacy_request_archive",
    "def _clear_privacy_request_archive",
    "Python privacy native request decoder",
  );
  assert.match(
    pythonRequestDecoder,
    /view\.nbytes\s*>\s*PRIVACY_NATIVE_ARCHIVE_MAX_BYTES/,
    "Python privacy request decoder must reject oversized archives before copying",
  );
  assert.match(
    pythonRequestDecoder,
    /_assert_privacy_norito_archive\([\s\S]*"request_archive"[\s\S]*native_output=False[\s\S]*expected_schema_byte=_PRIVACY_REQUEST_SCHEMA_BYTE/,
    "Python privacy request decoder must validate Norito frame headers and request schema before native dispatch",
  );

  assert.match(
    sliceBetween(
      swiftNativeBridge,
      "static func withTemporaryPrivacyRequestArchive",
      "static func clearTemporaryPrivacyRequestArchive",
      "Swift native privacy request copy",
    ),
    /requestArchive\.count\s*<=\s*Self\.privacyNativeArchiveMaxBytes/,
    "Swift native privacy request copy must reject oversized archives before copying",
  );
  assert.match(
    sliceBetween(
      swiftNativeBridge,
      "static func withTemporaryPrivacyRequestArchive",
      "static func clearTemporaryPrivacyRequestArchive",
      "Swift native privacy request copy",
    ),
    /Self\.isValidPrivacyNoritoArchive\(requestArchive\)/,
    "Swift native privacy request copy must validate Norito frame headers before copying",
  );
  assert.match(
    sliceBetween(
      swiftNativeBridge,
      "static func withTemporaryPrivacyRequestArchive",
      "static func clearTemporaryPrivacyRequestArchive",
      "Swift native privacy request copy",
    ),
    /Self\.hasPrivacyNoritoSchema\([\s\S]*requestArchive,[\s\S]*expectedSchemaByte:\s*Self\.privacyRequestSchemaByte[\s\S]*\)/,
    "Swift native privacy request copy must validate the privacy request schema before native dispatch",
  );
  assert.match(
    sliceBetween(
      source("IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift"),
      "static func call(\n        requestArchive: Data",
      "static func call(\n        bridgeAvailable: Bool",
      "Swift privacy bridge request call",
    ),
    /requestArchive\.count\s*<=\s*privacyNativeArchiveMaxBytes/,
    "Swift privacy bridge must reject oversized request archives before bridge dispatch",
  );
  assert.match(
    sliceBetween(
      source("IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift"),
      "static func call(\n        requestArchive: Data",
      "static func call(\n        bridgeAvailable: Bool",
      "Swift privacy bridge request call",
    ),
    /NoritoNativeBridge\.isValidPrivacyNoritoArchive\(requestArchive\)/,
    "Swift privacy bridge must validate request Norito frame headers before bridge dispatch",
  );
  assert.match(
    sliceBetween(
      source("IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift"),
      "static func call(\n        requestArchive: Data",
      "static func call(\n        bridgeAvailable: Bool",
      "Swift privacy bridge request call",
    ),
    /hasPrivacyNoritoSchema\([\s\S]*requestArchive,[\s\S]*expectedSchemaByte:\s*privacyRequestSchemaByte[\s\S]*\)/,
    "Swift privacy bridge must validate the privacy request schema before bridge dispatch",
  );

  assert.match(
    sliceBetween(
      javaBridge,
      "static byte[] call(",
      "static byte[] invokeNativeOutput",
      "Java privacy native request call",
    ),
    /requestArchive\.length\s*>\s*PRIVACY_NATIVE_ARCHIVE_MAX_BYTES/,
    "Java privacy bridge must reject oversized request archives before native dispatch",
  );
  assert.match(
    sliceBetween(
      javaBridge,
      "static byte[] call(",
      "static byte[] invokeNativeOutput",
      "Java privacy native request call",
    ),
    /isValidPrivacyNoritoArchive\(requestArchive\)/,
    "Java privacy bridge must validate request Norito frame headers before native dispatch",
  );
  assert.match(
    sliceBetween(
      javaBridge,
      "static byte[] call(",
      "static byte[] invokeNativeOutput",
      "Java privacy native request call",
    ),
    /hasPrivacyNoritoSchema\(requestArchive,\s*PRIVACY_SCHEMA_REQUEST\)/,
    "Java privacy bridge must validate the privacy request schema before native dispatch",
  );
  assert.match(
    sliceBetween(
      kotlinBridge,
      "internal fun call(",
      "internal fun invokeNativeOutput",
      "Kotlin privacy native request call",
    ),
    /requestArchive\.size\s*<=\s*PRIVACY_NATIVE_ARCHIVE_MAX_BYTES/,
    "Kotlin privacy bridge must reject oversized request archives before native dispatch",
  );
  assert.match(
    sliceBetween(
      kotlinBridge,
      "internal fun call(",
      "internal fun invokeNativeOutput",
      "Kotlin privacy native request call",
    ),
    /isValidPrivacyNoritoArchive\(requestArchive\)/,
    "Kotlin privacy bridge must validate request Norito frame headers before native dispatch",
  );
  assert.match(
    sliceBetween(
      kotlinBridge,
      "internal fun call(",
      "internal fun invokeNativeOutput",
      "Kotlin privacy native request call",
    ),
    /hasPrivacyNoritoSchema\(requestArchive,\s*PRIVACY_SCHEMA_REQUEST\)/,
    "Kotlin privacy bridge must validate the privacy request schema before native dispatch",
  );
  assert.match(
    sliceBetween(
      csharpBridge,
      "internal static byte[] CallProof",
      "private static void RequireAbi",
      "C# privacy native request call",
    ),
    /requestArchive\.Length\s*>\s*PrivacyNativeArchiveMaxBytes/,
    "C# privacy bridge must reject oversized request archives before native dispatch",
  );
  assert.match(
    sliceBetween(
      csharpBridge,
      "internal static byte[] CallProof",
      "private static void RequireAbi",
      "C# privacy native request call",
    ),
    /var\s+request\s*=\s*requestArchive\.ToArray\(\);[\s\S]*IsNoritoV1Archive\(request\)[\s\S]*Array\.Clear\(request,\s*0,\s*request\.Length\)/,
    "C# privacy bridge must validate request Norito frame headers before native dispatch",
  );
  assert.match(
    sliceBetween(
      csharpBridge,
      "internal static byte[] CallProof",
      "private static void RequireAbi",
      "C# privacy native request call",
    ),
    /HasNoritoSchema\(request,\s*PrivacyRequestSchemaByte\)/,
    "C# privacy bridge must validate the privacy request schema before native dispatch",
  );

  assert.match(
    connectBridge,
    /fn\s+privacy_request_archive_out_of_bounds\(len:\s*usize\)\s*->\s*bool\s*\{\s*len\s*==\s*0\s*\|\|\s*len\s*>\s*PRIVACY_NATIVE_ARCHIVE_MAX_BYTES\s*\}/,
    "C bridge must define a testable privacy request archive bounds predicate",
  );
  assert.match(
    sliceBetween(
      connectBridge,
      "fn privacy_result_for_request_archive",
      "fn write_privacy_payload",
      "C bridge shared privacy request decoder",
    ),
    /privacy_request_archive_out_of_bounds\(request_bytes\.len\(\)\)/,
    "C bridge shared privacy decoder must reject oversized request archives",
  );
  assert.match(
    sliceBetween(
      connectBridge,
      "unsafe fn read_privacy_request",
      "unsafe fn iroha_privacy_process_request_v1",
      "C bridge raw privacy request reader",
    ),
    /privacy_request_archive_out_of_bounds\(request_len\)/,
    "C bridge raw privacy request reader must reject oversized pointer archives",
  );
  assert.match(
    connectBridge,
    /fn\s+clear_privacy_output\([^)]*out_ptr:[^)]*out_len:[^)]*\)\s*\{[\s\S]*\*out_ptr\s*=\s*ptr::null_mut\(\);[\s\S]*\*out_len\s*=\s*0;/,
    "C bridge must clear non-null privacy output slots before null-buffer failures",
  );
  assert.match(
    sliceBetween(
      connectBridge,
      "fn write_privacy_payload",
      "unsafe fn read_privacy_request",
      "C bridge privacy payload writer",
    ),
    /clear_privacy_output\(out_ptr,\s*out_len\)[\s\S]*out_ptr\.is_null\(\)\s*\|\|\s*out_len\.is_null\(\)[\s\S]*ERR_NULL_PTR/,
    "C bridge privacy payload writer must clear stale output slots before returning null-pointer errors",
  );
  assert.match(
    sliceBetween(
      connectBridge,
      "unsafe fn iroha_privacy_process_request_v1",
      "#[unsafe(no_mangle)]\npub unsafe extern \"C\" fn iroha_privacy_capabilities_v1",
      "C bridge privacy proof processor",
    ),
    /clear_privacy_output\(out_ptr,\s*out_len\)[\s\S]*out_ptr\.is_null\(\)\s*\|\|\s*out_len\.is_null\(\)[\s\S]*ERR_NULL_PTR/,
    "C bridge privacy proof processor must clear stale output slots before null-output errors",
  );
  assert.match(
    sliceBetween(
      connectBridge,
      "fn privacy_capabilities_reject_missing_output_buffer",
      "fn privacy_free_buffer_tolerates_null_pointer",
      "C bridge privacy output-buffer tests",
    ),
    /out_len:\s*c_ulong\s*=\s*777[\s\S]*assert_eq!\(out_len,\s*0\)[\s\S]*without_provenance_mut::<c_uchar>\(0x01\)[\s\S]*assert!\(out_ptr\.is_null\(\)\)[\s\S]*out_len:\s*c_ulong\s*=\s*911[\s\S]*assert_eq!\(out_len,\s*0\)[\s\S]*without_provenance_mut::<c_uchar>\(0x03\)[\s\S]*assert!\(out_ptr\.is_null\(\)\)/,
    "C bridge privacy output-buffer tests must cover stale pointer and length sentinels",
  );
  assert.match(
    sliceBetween(
      connectBridge,
      "fn privacy_proof_entrypoints_prioritize_missing_output_buffers_over_bad_requests",
      "fn privacy_free_buffer_tolerates_null_pointer",
      "C bridge privacy bad-request output-buffer precedence test",
    ),
    /ptr::null\(\)[\s\S]*oversized_len[\s\S]*ptr::null_mut\(\)[\s\S]*ERR_NULL_PTR[\s\S]*assert_eq!\(out_len,\s*0\)[\s\S]*without_provenance_mut::<c_uchar>\(0x04\)[\s\S]*ptr::null\(\)[\s\S]*oversized_len[\s\S]*ptr::null_mut\(\)[\s\S]*ERR_NULL_PTR[\s\S]*assert!\(out_ptr\.is_null\(\)\)/,
    "C bridge privacy proof entrypoints must prioritize missing output buffers over bad request pointers",
  );
  assert.match(
    connectBridge,
    /fn\s+privacy_request_archive_size_boundaries_are_fail_closed/,
    "C bridge must unit-test privacy request archive size boundaries",
  );

  for (const [label, text] of [
    ["C bridge privacy FFI", connectBridge],
    ["JS NAPI privacy FFI", jsHost],
    ["Python PyO3 privacy FFI", pythonRust],
  ]) {
    assert.match(
      text,
      /fn\s+adversarial_privacy_request_archives\(\)\s*->\s*Vec<\(&'static str,\s*Vec<u8>\)>/,
      `${label} must define adversarial privacy request Norito frame fixtures`,
    );
    assert.match(
      text,
      /bad_magic\[0\][\s\S]*bad_version\[4\][\s\S]*bad_schema\[6\][\s\S]*bad_compression\[22\][\s\S]*bad_payload_length\[30\][\s\S]*bad_crc\[31\][\s\S]*bad_flags\[39\][\s\S]*payload_tamper\[payload_last\]/,
      `${label} malformed-frame fixtures must mutate magic, version, schema, compression, length, CRC, flags, and payload bytes`,
    );
    assert.match(
      text,
      /fn\s+privacy_proof_entrypoints_reject_adversarial_norito_frames/,
      `${label} must unit-test adversarial privacy request Norito frames`,
    );
  }

  for (const [label, text] of [
    ["JS NAPI privacy FFI", jsHost],
    ["Python PyO3 privacy FFI", pythonRust],
  ]) {
    assert.match(
      text,
      /fn\s+privacy_request_archive_out_of_bounds\(len:\s*usize\)\s*->\s*bool\s*\{\s*len\s*==\s*0\s*\|\|\s*len\s*>\s*PRIVACY_NATIVE_ARCHIVE_MAX_BYTES\s*\}/,
      `${label} must define a testable privacy request archive bounds predicate`,
    );
    assert.match(
      sliceBetween(
        text,
        "fn privacy_result_for_request_archive",
        "fn encode_privacy_archive",
        `${label} request decoder`,
      ),
      /privacy_request_archive_out_of_bounds\(request_archive\.len\(\)\)/,
      `${label} must reject oversized request archives before Norito decode`,
    );
    assert.match(
      text,
      /fn\s+privacy_request_archive_size_boundaries_are_fail_closed/,
      `${label} must unit-test privacy request archive size boundaries`,
    );
  }
});
