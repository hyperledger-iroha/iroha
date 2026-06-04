import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  getPrivacyAlgorithmDescriptor as getSrcPrivacyAlgorithmDescriptor,
  getPrivacyAlgorithmDescriptors as getSrcPrivacyAlgorithmDescriptors,
  getPrivacyCapabilities as getSrcPrivacyCapabilities,
  getPrivacyCriteria as getSrcPrivacyCriteria,
  validatePrivacyAlgorithmDescriptor as validateSrcPrivacyAlgorithmDescriptor,
} from "../src/privacyAlgorithms.js";
import * as jsSrcCrypto from "../src/crypto.js";
import * as jsSrcBrowserCrypto from "../src/crypto.browser.js";
import * as jsSrcInstructionBuilders from "../src/instructionBuilders.js";
import * as jsSrcPackage from "../src/index.js";
import {
  getPrivacyAlgorithmDescriptor as getDistPrivacyAlgorithmDescriptor,
  getPrivacyAlgorithmDescriptors as getDistPrivacyAlgorithmDescriptors,
  getPrivacyCapabilities as getDistPrivacyCapabilities,
  getPrivacyCriteria as getDistPrivacyCriteria,
  validatePrivacyAlgorithmDescriptor as validateDistPrivacyAlgorithmDescriptor,
} from "../dist/privacyAlgorithms.js";
import * as jsDistCrypto from "../dist/crypto.js";
import * as jsDistBrowserCrypto from "../dist/crypto.browser.js";
import * as jsDistInstructionBuilders from "../dist/instructionBuilders.js";
import * as jsDistPackage from "../dist/index.js";

const PYTHON_PRIVACY_CATALOG = fileURLToPath(
  new URL("../../../python/iroha_python/src/iroha_python/privacy_catalog.py", import.meta.url),
);
const JS_DECLARATIONS = "javascript/iroha_js/index.d.ts";
const REPO_ROOT = fileURLToPath(new URL("../../..", import.meta.url));
const PRODUCTION_GATE_VERSION = "privacy-production-gate-v1";
const PRODUCTION_GATE_REQUIREMENTS = Object.freeze([
  Object.freeze(["real_proving", "real proving engine is not registered"]),
  Object.freeze(["real_verification", "real verifier is not registered"]),
  Object.freeze(["chain_admission", "chain admission path is not enabled"]),
  Object.freeze(["sdk_parity", "cross-SDK parity is incomplete"]),
  Object.freeze(["wallet_state", "wallet/state support is incomplete"]),
  Object.freeze(["deterministic_tests", "deterministic tests are incomplete"]),
  Object.freeze(["fuzzing", "fuzzing gate is incomplete"]),
  Object.freeze(["performance_gates", "performance gate is incomplete"]),
  Object.freeze(["external_audit", "external audit signoff is missing"]),
]);
const PRODUCTION_GATE_REQUIRED_REASONS = Object.freeze(
  PRODUCTION_GATE_REQUIREMENTS.map(([_key, reason]) => reason),
);
const SUPPLEMENTAL_FAIL_CLOSED_REASONS = Object.freeze([
  "implementation stage is not production-hardened",
  "planned SDK entrypoints remain",
  "dev fixture entrypoints are not production entrypoints",
  "Iroha production allowlist is not enabled for this audited row",
]);
const POST_QUANTUM_REQUIRED_SOURCE_URLS = Object.freeze([
  "https://csrc.nist.gov/pubs/fips/203/final",
  "https://csrc.nist.gov/pubs/fips/204/final",
  "https://csrc.nist.gov/pubs/fips/205/final",
]);
const POST_QUANTUM_REQUIRED_PLANNED_ENTRYPOINT_FRAGMENTS = Object.freeze([
  "MlDsa",
  "MlKem",
]);
const POST_QUANTUM_REQUIRED_SECURITY_NOTE_TOKENS = Object.freeze(["ML-DSA", "ML-KEM"]);
const POST_QUANTUM_REQUIRED_FAILURE_MODE_TOKENS = Object.freeze(["ML-DSA", "ML-KEM"]);
const POST_QUANTUM_REQUIRED_STATE_TOKENS = Object.freeze(["ML-KEM"]);
const RESEARCH_TARGET_REQUIRED_SOURCE_URLS_BY_ID = Object.freeze({
  "orchard-halo2-actions-v1": Object.freeze(["https://zips.z.cash/zip-0224"]),
  "penumbra-masp-v1": Object.freeze([
    "https://protocol.penumbra.zone/main/shielded_pool.html",
  ]),
  "monero-fcmp-plus-plus-v1": Object.freeze([
    "https://web.getmonero.org/2024/04/27/fcmps.html",
  ]),
  "miden-stark-note-v1": Object.freeze([
    "https://docs.miden.xyz/core-concepts/miden-base/transaction/",
    "https://docs.miden.xyz/core-concepts/miden-base/note/",
  ]),
  "aztec-private-rollup-v1": Object.freeze([
    "https://docs.aztec.network/developers/nightly/docs/foundational-topics/advanced/circuits/private_kernel",
  ]),
  "pq-masp-stark-v0": POST_QUANTUM_REQUIRED_SOURCE_URLS,
});
const LEDGER_MUTATION_PROTECTION_METADATA_TOKENS = Object.freeze([
  "nullifier",
  "replay",
  "revocation",
  "link-tag",
  "link tag",
]);
const TYPED_CHAIN_ADMISSION_METADATA_FIELDS = Object.freeze([
  "chain_requirements",
  "setup_steps",
  "execution_steps",
]);
const TYPED_CHAIN_ADMISSION_TYPE_TOKENS = Object.freeze(["typed", "zk::"]);
const TYPED_CHAIN_ADMISSION_MUTATION_TOKENS = Object.freeze([
  "instruction",
  "transaction",
  "isi",
  "zk::",
]);
const STATEFUL_LEDGER_STATE_TOKENS = Object.freeze([
  "nullifier",
  "commitment",
  "accumulator",
  "root",
  "revocation",
  "replay",
  "link-tag",
  "link tag",
  "tree",
]);
const STATEFUL_LEDGER_PERSISTENCE_METADATA_FIELDS = Object.freeze([
  "security_notes",
  "failure_modes",
  "setup_steps",
  "execution_steps",
  "chain_requirements",
]);
const STATEFUL_LEDGER_PERSISTENCE_TOKEN_GROUPS = Object.freeze([
  Object.freeze(["persist", "persistence", "restart", "recovery"]),
  Object.freeze(["replay", "nullifier", "revocation", "link-tag", "link tag"]),
]);
const WALLET_STATE_REQUIRED_IMPLEMENTATION_STAGES = new Set([
  "chain-executable",
  "sdk-builder",
  "research-target-as-of-2026-05",
  "production-hardened",
]);
const SOURCE_REFERENCED_IMPLEMENTATION_STAGES = new Set([
  "chain-executable",
  "sdk-builder",
  "component",
  "research-target-as-of-2026-05",
  "production-hardened",
]);
const WALLET_STATE_REQUIRED_EXCLUDED_CATEGORIES = new Set(["proof_backend"]);
const WALLET_STATE_METADATA_TOKENS = Object.freeze(["wallet", "witness"]);
const CREDENTIAL_STATE_REQUIRED_CATEGORIES = new Set([
  "admission",
  "credential",
  "identity",
]);
const CREDENTIAL_STATE_METADATA_TOKENS = Object.freeze(["commitment", "accumulator"]);
const VERIFIER_KEY_RECORD_METADATA_FIELDS = Object.freeze([
  "required_state",
  "chain_requirements",
  "setup_steps",
]);
const VERIFIER_KEY_RECORD_METADATA_TOKENS = Object.freeze(["verifier key", "verifier-key"]);
const CHAIN_DOMAIN_BINDING_METADATA_FIELDS = Object.freeze([
  "public_inputs_schema",
  "security_notes",
  "failure_modes",
  "setup_steps",
  "execution_steps",
]);
const CHAIN_DOMAIN_BINDING_METADATA_TOKENS = Object.freeze([
  "domain_separator",
  "domain-separat",
  "domain separat",
  "chain_id",
  "chain_tag",
  "tx_digest",
  "transaction",
  "reference_block",
  "reference block",
  "rollup_state",
  "rollup state",
  "anchor",
  "epoch",
]);
const SOURCE_REFERENCED_HARDENING_NOTE_TOKEN_GROUPS = Object.freeze([
  Object.freeze(["audit", "audited", "review"]),
  Object.freeze(["fuzz", "fuzzing"]),
  Object.freeze(["performance", "benchmark", "latency"]),
]);
const WALLET_WITNESS_PRIVACY_NOTE_TOKEN_GROUPS = Object.freeze([
  Object.freeze(["wallet", "witness", "private input", "private inputs", "plaintext", "secret"]),
  Object.freeze([
    "local",
    "not exposed",
    "not be exposed",
    "not leak",
    "must not expose",
    "must not leak",
    "never leave",
  ]),
]);
const VERIFIER_NEGATIVE_FAILURE_MODE_TOKEN_GROUPS = Object.freeze([
  Object.freeze(["malformed proof", "invalid proof", "proof parse", "proof rejected"]),
  Object.freeze([
    "wrong verifier key",
    "verifier key mismatch",
    "verifier-key mismatch",
    "unknown verifier key",
  ]),
  Object.freeze(["public input mismatch", "wrong public input", "public-input mismatch"]),
]);
const PUBLIC_INPUT_SCHEMA_FORBIDDEN_PAYLOAD_TOKEN_SEGMENTS = Object.freeze([
  "proof",
  "proofs",
  "witness",
  "witnesses",
]);
const RESEARCH_TARGET_PRODUCTION_READINESS_TOKENS = Object.freeze(["production"]);
const RESEARCH_TARGET_READINESS_EVIDENCE_TOKENS = Object.freeze([
  "audit",
  "audited",
  "review",
]);
const RUST_NATIVE_SUPPLEMENTAL_FAIL_CLOSED_REASONS = Object.freeze([
  "real protocol engine is not production-enabled",
  "Iroha production allowlist is not enabled for this audited row",
]);
const REQUIRED_PRIVACY_PLAN_ROWS = Object.freeze([
  Object.freeze(["anonymous-pgc-k-out-of-n-v1", "sdk-builder", "anonymous-pgc"]),
  Object.freeze(["verange-transparent-range-v1", "component", "verange"]),
  Object.freeze(["zkat-policy-private-auth-v1", "sdk-builder", "zkat"]),
  Object.freeze([
    "zk-ams-recursive-admission-v0",
    "sdk-builder",
    "recursive-anonymous-admission",
  ]),
  Object.freeze([
    "vega-existing-credential-zk-v0",
    "sdk-builder",
    "vega-existing-credential-zk",
  ]),
  Object.freeze([
    "silent-threshold-anoncred-v0",
    "sdk-builder",
    "silent-threshold-anoncred",
  ]),
  Object.freeze(["zk-x509-onchain-identity-v0", "sdk-builder", "zk-x509"]),
  Object.freeze(["jindo-lattice-pcs-zk-v0", "sdk-builder", "lattice-pcs-sis"]),
  Object.freeze(["sis-hints-anoncred-pq-v0", "sdk-builder", "sis-with-hints"]),
  Object.freeze([
    "orchard-halo2-actions-v1",
    "research-target-as-of-2026-05",
    "halo2-ipa-orchard",
  ]),
  Object.freeze([
    "penumbra-masp-v1",
    "research-target-as-of-2026-05",
    "groth16-bls12-377",
  ]),
  Object.freeze([
    "monero-fcmp-plus-plus-v1",
    "research-target-as-of-2026-05",
    "fcmp-plus-plus-curve-tree",
  ]),
  Object.freeze([
    "miden-stark-note-v1",
    "research-target-as-of-2026-05",
    "miden-stark",
  ]),
  Object.freeze([
    "aztec-private-rollup-v1",
    "research-target-as-of-2026-05",
    "aztec-plonkish-private-kernel",
  ]),
  Object.freeze(["pq-masp-stark-v0", "research-target-as-of-2026-05", "pq-masp-stark-fri"]),
]);
const BRIDGE_MISSING_REASON_SOURCES = Object.freeze([
  Object.freeze({
    label: "Java Android",
    path: "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java",
    start: "PRODUCTION_GATE_MISSING =",
    end: "));",
  }),
  Object.freeze({
    label: "Kotlin JVM",
    path: "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt",
    start: "val MISSING_REASONS: List<String> =",
    end: "@JvmStatic",
  }),
  Object.freeze({
    label: "Swift",
    path: "IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift",
    start: "public static let missingReasons = [",
    end: "]",
  }),
  Object.freeze({
    label: "C#",
    path: "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs",
    start: "public static IReadOnlyList<string> MissingReasons",
    end: "});",
  }),
]);
const RUST_PRIVACY_ALGORITHM_SOURCES = Object.freeze([
  Object.freeze({
    label: "connect_norito_bridge",
    path: "crates/connect_norito_bridge/src/lib.rs",
  }),
  Object.freeze({
    label: "iroha_js_host",
    path: "crates/iroha_js_host/src/lib.rs",
  }),
  Object.freeze({
    label: "iroha_python_rs",
    path: "python/iroha_python/iroha_python_rs/src/lib.rs",
  }),
]);
const DERIVED_JS_COMPATIBILITY_FIELDS = Object.freeze([
  "hiddenFeatures",
  "hidden_features",
  "requirements",
  "limitations",
  "status",
  "unavailableReason",
  "unavailable_reason",
  "verifierKeyMetadata",
  "verifier_key_metadata",
  "backendFamily",
  "backend_family",
  "productionReady",
  "production_ready",
  "productionGate",
  "production_gate",
]);
const PUBLIC_PRIVACY_API_DECLARATION_SURFACES = Object.freeze([
  Object.freeze({
    label: "JS TypeScript declarations",
    path: JS_DECLARATIONS,
  }),
  Object.freeze({
    label: "Python package exports",
    path: "python/iroha_python/src/iroha_python/__init__.py",
  }),
  Object.freeze({
    label: "Swift privacy bridge",
    path: "IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift",
  }),
  Object.freeze({
    label: "Java Android privacy bridge",
    path: "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java",
  }),
  Object.freeze({
    label: "Kotlin JVM privacy bridge",
    path: "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt",
  }),
  Object.freeze({
    label: "C# privacy native bridge",
    path: "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs",
  }),
]);
const PUBLIC_PRIVACY_API_SOURCE_SCAN_SURFACES = Object.freeze([
  Object.freeze({
    label: "JS src SDK",
    root: "javascript/iroha_js/src",
    extensions: Object.freeze([".js"]),
    language: "javascript",
  }),
  Object.freeze({
    label: "JS dist SDK",
    root: "javascript/iroha_js/dist",
    extensions: Object.freeze([".js"]),
    language: "javascript",
  }),
  Object.freeze({
    label: "Python SDK",
    root: "python/iroha_python/src/iroha_python",
    extensions: Object.freeze([".py"]),
    language: "python",
  }),
  Object.freeze({
    label: "Swift SDK",
    root: "IrohaSwift/Sources/IrohaSwift",
    extensions: Object.freeze([".swift"]),
    language: "swift",
  }),
  Object.freeze({
    label: "Java Android SDK",
    root: "java/iroha_android/src/main/java",
    extensions: Object.freeze([".java"]),
    language: "java",
  }),
  Object.freeze({
    label: "Kotlin JVM SDK",
    root: "kotlin/core-jvm/src/main/java",
    extensions: Object.freeze([".kt"]),
    language: "kotlin",
  }),
  Object.freeze({
    label: "C# SDK",
    root: "csharp/src",
    extensions: Object.freeze([".cs"]),
    language: "csharp",
  }),
]);
function snakeEntrypointName(entrypoint) {
  return entrypoint.replace(/(?<!^)(?=[A-Z])/g, "_").toLowerCase();
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function publicApiNameVariants(entrypoint) {
  const snakeEntrypoint = snakeEntrypointName(entrypoint);
  return [
    entrypoint,
    `${entrypoint[0].toUpperCase()}${entrypoint.slice(1)}`,
    snakeEntrypoint,
    snakeEntrypoint.replace("ve_range", "verange"),
    snakeEntrypoint.replace("zk_at", "zkat"),
  ];
}

function rawJsPrivacyDescriptor(patch = {}) {
  const researchPatch = patch.implementationStage === "research-target-as-of-2026-05";
  return {
    id: "shield",
    name: "Shape check",
    shortName: "Shape",
    summary: "Descriptor used to test hostile catalog input validation.",
    category: "payment",
    maturity: "specification",
    coveredCriteria: [],
    proofFamily: "shape-proof",
    publicInputsSchema: "root,domain_separator",
    verifierKeyId: "shape_verifier_v0",
    pqLayers: {
      proof: false,
      authorization: false,
      noteEncryption: false,
    },
    implementationStage: "production-hardened",
    recommendedFor: ["shape validation"],
    sourceReferences: [
      {
        label: "Shape fixture",
        url: "https://zips.z.cash/zip-0224",
      },
    ],
    securityNotes: [
      "Production readiness requires audit review for shape proof constraints.",
      "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
    ],
    requiredState: ["shape verifier key registry"],
    failureModes: ["shape proof rejected"],
    setupSteps: ["Register shape verifier key"],
    executionSteps: ["Build shape proof"],
    sdkEntrypoints: researchPatch ? [] : ["buildShapeProof"],
    plannedSdkEntrypoints: [],
    chainRequirements: ["shape verifier key registry"],
    ...patch,
  };
}

function validatorSurfaces() {
  return [
    ["src", validateSrcPrivacyAlgorithmDescriptor],
    ["dist", validateDistPrivacyAlgorithmDescriptor],
  ];
}

function assertJsValidatorsReject(patch, pattern) {
  for (const [label, validate] of validatorSurfaces()) {
    assert.throws(
      () => validate(rawJsPrivacyDescriptor(patch), 99),
      pattern,
      `${label} validator must reject hostile privacy descriptor patch ${JSON.stringify(patch)}`,
    );
  }
}

function entrypointIsDevFixture(entrypoint) {
  const normalized = entrypoint.replaceAll("-", "_").toLowerCase();
  const compact = entrypoint.toLowerCase().replace(/[^a-z0-9]/g, "");
  return (
    normalized.includes("devfixture") ||
    normalized.includes("dev_fixture") ||
    normalized.includes("devprooffixture") ||
    normalized.includes("dev_proof_fixture") ||
    normalized.includes("fixture") ||
    normalized.includes("mock") ||
    compact.includes("devfixture") ||
    compact.includes("devprooffixture") ||
    compact.includes("fixture") ||
    compact.includes("mock")
  );
}

function entrypointIsLocalVerifier(entrypoint) {
  const segments = entrypoint.split(".");
  const name = segments[segments.length - 1];
  const lower = name.toLowerCase();
  return (
    lower.startsWith("verify") &&
    (lower.endsWith("locally") ||
      lower.endsWith("local") ||
      lower.includes("localverifier") ||
      lower.includes("localonly"))
  );
}

function entrypointIsInstructionBuilder(entrypoint) {
  const segments = entrypoint.split(".");
  const name = segments[segments.length - 1];
  return name.endsWith("Instruction");
}

function entrypointIsPlannedLedgerMutation(entrypoint) {
  const segments = entrypoint.split(".");
  const name = segments[segments.length - 1];
  return (
    name.endsWith("Instruction") ||
    name.endsWith("Transaction") ||
    name.includes("Submit")
  );
}

function entrypointIsProofHelper(entrypoint) {
  const segments = entrypoint.split(".");
  const name = segments[segments.length - 1];
  return (
    name.includes("ProofEnvelope") ||
    name.includes("ProofWitness") ||
    name.includes("ProofPublicInputs") ||
    name.includes("ProofRequest") ||
    name.includes("ProofCommitment")
  );
}

function entrypointIsProductionProofBuilder(entrypoint) {
  const segments = entrypoint.split(".");
  const name = segments[segments.length - 1];
  return (
    name.startsWith("build") &&
    name.includes("Proof") &&
    !entrypointIsInstructionBuilder(entrypoint) &&
    !entrypointIsPlannedLedgerMutation(entrypoint) &&
    !entrypointIsProofHelper(entrypoint) &&
    !entrypointIsDevFixture(entrypoint)
  );
}

function entrypointIsExplicitDevFixture(entrypoint) {
  const normalized = entrypoint.replaceAll("-", "_").toLowerCase();
  const compact = entrypoint.toLowerCase().replace(/[^a-z0-9]/g, "");
  return (
    normalized.includes("devfixture") ||
    normalized.includes("dev_fixture") ||
    normalized.includes("devprooffixture") ||
    normalized.includes("dev_proof_fixture") ||
    compact.includes("devfixture") ||
    compact.includes("devprooffixture")
  );
}

function loadPythonPrivacyCatalog() {
  const script = `
import importlib.util
import json
import sys

path = sys.argv[1]
spec = importlib.util.spec_from_file_location("privacy_catalog_direct", path)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
print(json.dumps({
    "criteria": module.get_privacy_criteria(),
    "descriptors": module.get_privacy_algorithm_descriptors(),
    "backend_family_items": list(module.BACKEND_FAMILY_BY_ALGORITHM_ID.items()),
}, sort_keys=False))
`;
  const result = spawnSync("python3", ["-c", script, PYTHON_PRIVACY_CATALOG], {
    encoding: "utf8",
  });
  assert.equal(
    result.status,
    0,
    `python privacy catalog loader failed\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
  );
  return JSON.parse(result.stdout);
}

function toPythonDescriptorShape(descriptor) {
  return {
    id: descriptor.id,
    name: descriptor.name,
    short_name: descriptor.shortName,
    summary: descriptor.summary,
    category: descriptor.category,
    maturity: descriptor.maturity,
    covered_criteria: descriptor.coveredCriteria,
    proof_family: descriptor.proofFamily,
    public_inputs_schema: descriptor.publicInputsSchema,
    verifier_key_id: descriptor.verifierKeyId,
    backend_family: descriptor.backendFamily,
    pq_layers: {
      proof: descriptor.pqLayers.proof,
      authorization: descriptor.pqLayers.authorization,
      note_encryption: descriptor.pqLayers.noteEncryption,
    },
    implementation_stage: descriptor.implementationStage,
    recommended_for: descriptor.recommendedFor,
    source_references: descriptor.sourceReferences,
    security_notes: descriptor.securityNotes,
    required_state: descriptor.requiredState,
    failure_modes: descriptor.failureModes,
    setup_steps: descriptor.setupSteps,
    execution_steps: descriptor.executionSteps,
    sdk_entrypoints: descriptor.sdkEntrypoints,
    planned_sdk_entrypoints: descriptor.plannedSdkEntrypoints,
    chain_requirements: descriptor.chainRequirements,
    production_ready: descriptor.productionReady,
    production_gate: {
      version: descriptor.productionGate.version,
      ready: descriptor.productionGate.ready,
      gates: descriptor.productionGate.gates,
      missing: descriptor.productionGate.missing,
      audit_references: descriptor.productionGate.auditReferences,
    },
  };
}

function assertFailClosedDescriptor(label, descriptor) {
  const expectedGateEntries = PRODUCTION_GATE_REQUIREMENTS.map(([key]) => [key, false]);
  const expectedMissingReasons = [
    ...PRODUCTION_GATE_REQUIRED_REASONS,
    ...SUPPLEMENTAL_FAIL_CLOSED_REASONS.filter((reason) =>
      descriptor.production_gate.missing.includes(reason),
    ),
  ];

  assert.equal(
    descriptor.production_gate.version,
    PRODUCTION_GATE_VERSION,
    `${label} ${descriptor.id} production gate version drifted`,
  );
  assert.equal(
    descriptor.production_ready,
    false,
    `${label} ${descriptor.id} must not claim production readiness`,
  );
  assert.equal(
    descriptor.production_gate.ready,
    false,
    `${label} ${descriptor.id} production gate must fail closed`,
  );
  assert.deepEqual(
    descriptor.production_gate.audit_references,
    [],
    `${label} ${descriptor.id} must not claim audit references before signoff`,
  );
  assert.deepEqual(
    descriptor.production_gate.gates,
    Object.fromEntries(expectedGateEntries),
    `${label} ${descriptor.id} production gate keys must be stable and fail closed`,
  );
  assert.deepEqual(
    Object.entries(descriptor.production_gate.gates),
    expectedGateEntries,
    `${label} ${descriptor.id} production gate keys must stay in canonical order`,
  );
  assert.deepEqual(
    Object.values(descriptor.production_gate.gates),
    Object.values(descriptor.production_gate.gates).map(() => false),
    `${label} ${descriptor.id} must keep every production gate false`,
  );
  assert.equal(
    new Set(descriptor.production_gate.missing).size,
    descriptor.production_gate.missing.length,
    `${label} ${descriptor.id} production gate missing reasons must not contain duplicates`,
  );
  for (const missing of [
    ...PRODUCTION_GATE_REQUIRED_REASONS,
    "Iroha production allowlist is not enabled for this audited row",
  ]) {
    assert.ok(
      descriptor.production_gate.missing.includes(missing),
      `${label} ${descriptor.id} missing production gate reason ${missing}`,
    );
  }
  assert.deepEqual(
    descriptor.production_gate.missing,
    expectedMissingReasons,
    `${label} ${descriptor.id} production gate missing reasons must stay canonical and ordered`,
  );
}

function canonicalBridgeMissingReasons() {
  return [
    ...PRODUCTION_GATE_REQUIRED_REASONS,
    ...SUPPLEMENTAL_FAIL_CLOSED_REASONS,
  ];
}

function fileText(relativePath) {
  return readFileSync(new URL(relativePath, `file://${REPO_ROOT}/`), "utf8");
}

function sourceFilesUnder(relativeRoot, extensions) {
  const files = [];
  const ignoredDirectories = new Set([".git", ".gradle", ".swiftpm", "bin", "build", "dist", "node_modules", "obj"]);
  const walk = (absoluteDirectory, relativeDirectory) => {
    for (const entry of readdirSync(absoluteDirectory, { withFileTypes: true })) {
      if (entry.isDirectory()) {
        if (!ignoredDirectories.has(entry.name)) {
          walk(
            `${absoluteDirectory}/${entry.name}`,
            relativeDirectory === "" ? entry.name : `${relativeDirectory}/${entry.name}`,
          );
        }
        continue;
      }
      if (!entry.isFile()) {
        continue;
      }
      if (extensions.some((extension) => entry.name.endsWith(extension))) {
        files.push(
          relativeDirectory === ""
            ? `${relativeRoot}/${entry.name}`
            : `${relativeRoot}/${relativeDirectory}/${entry.name}`,
        );
      }
    }
  };
  walk(`${REPO_ROOT}/${relativeRoot}`, "");
  return files.sort();
}

function publicDeclarationPatterns(language, name) {
  const escaped = escapeRegExp(name);
  switch (language) {
    case "javascript":
      return [
        new RegExp(`\\bexport\\s+(?:async\\s+)?function\\s+${escaped}\\s*\\(`),
        new RegExp(`\\bexport\\s+(?:const|let|var)\\s+${escaped}\\b`),
        new RegExp(`\\bexport\\s*\\{[^}]*\\b${escaped}\\b[^}]*\\}`),
      ];
    case "python":
      return [
        new RegExp(`^(?:def\\s+${escaped}\\s*\\(|${escaped}\\s*=)`, "m"),
      ];
    case "swift":
      return [
        new RegExp(`^\\s*public\\s+(?:static\\s+)?func\\s+${escaped}\\s*\\(`, "m"),
        new RegExp(`^\\s*public\\s+(?:static\\s+)?(?:let|var)\\s+${escaped}\\b`, "m"),
      ];
    case "java":
      return [
        new RegExp(
          `^\\s*public\\s+(?:static\\s+)?(?:final\\s+)?[\\w<>\\[\\].?,\\s]+\\s+${escaped}\\s*\\(`,
          "m",
        ),
      ];
    case "kotlin":
      return [
        new RegExp(`^\\s*(?!(?:private|internal)\\b)(?:public\\s+)?fun\\s+${escaped}\\s*\\(`, "m"),
        new RegExp(
          `^\\s*(?!(?:private|internal)\\b)(?:public\\s+)?(?:val|var)\\s+${escaped}\\b`,
          "m",
        ),
      ];
    case "csharp":
      return [
        new RegExp(
          `^\\s*public\\s+(?:static\\s+)?[\\w<>\\[\\].?,\\s]+\\s+${escaped}\\s*(?:\\(|\\{)`,
          "m",
        ),
      ];
    default:
      throw new Error(`unsupported source declaration scan language ${language}`);
  }
}

function publicPrivacyApiSourceTexts() {
  return PUBLIC_PRIVACY_API_SOURCE_SCAN_SURFACES.flatMap((surface) =>
    sourceFilesUnder(surface.root, surface.extensions).map((path) => ({
      ...surface,
      path,
      text: fileText(path),
    })),
  );
}

function assertPythonCatalogDefensiveCopyCoverage() {
  const text = fileText("python/iroha_python/tests/privacy_catalog_test.py");
  for (const snippet of [
    'descriptors[0]["pq_layers"]["proof"] = "tampered"',
    'descriptors[0]["production_gate"]["audit_references"].append(',
    'planned["planned_sdk_entrypoints"].clear()',
    'source_descriptor["source_references"][0]["url"] = "https://audit.example/forged"',
    'descriptor["source_references"][0]["label"] = "forged source"',
    'capabilities["privacy_algorithms"][0]["pq_layers"]["proof"] = "tampered"',
    'source_descriptor["source_references"].append(',
  ]) {
    assert.ok(
      text.includes(snippet),
      `Python privacy catalog defensive-copy coverage missing ${snippet}`,
    );
  }
}

function extractJsBackendFamilyEntries(text, label) {
  const block = requireMatch(
    text,
    /const BACKEND_FAMILY_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);/,
    `${label} backend family registration map`,
  )[1];
  const entries = [...block.matchAll(/^\s*(?:"([^"]+)"|([A-Za-z_$][\w$]*)):\s*"([^"]+)",?$/gm)]
    .map((match) => [match[1] ?? match[2], match[3]]);

  assert.ok(entries.length > 0, `${label} backend family registration map is empty`);
  assert.equal(
    new Set(entries.map(([id]) => id)).size,
    entries.length,
    `${label} backend family registration map contains duplicate ids`,
  );
  return entries;
}

function isBackendFamilyName(value) {
  return /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/.test(value) && !value.includes("--");
}

function assertBackendFamilyRegistrationParity(pythonCatalog) {
  const expected = pythonCatalog.descriptors.map((descriptor) => [
    descriptor.id,
    descriptor.backend_family,
  ]);

  assert.deepEqual(
    pythonCatalog.backend_family_items,
    expected,
    "Python backend-family registration map must exactly match catalog row order",
  );
  for (const source of [
    Object.freeze({
      label: "JS src",
      path: "javascript/iroha_js/src/privacyAlgorithms.js",
    }),
    Object.freeze({
      label: "JS dist",
      path: "javascript/iroha_js/dist/privacyAlgorithms.js",
    }),
  ]) {
    assert.ok(
      fileText(source.path).includes(
        "privacy algorithm backend-family registration must exactly match catalog ids",
      ),
      `${source.label} must keep runtime backend-family registration exactness guard`,
    );
    assert.ok(
      fileText(source.path).includes(
        "catalogLabelClaimsProductionReadiness(backendFamily)",
      ),
      `${source.label} must reject backend-family production/mainnet/audit claim labels`,
    );
    assert.ok(
      fileText(source.path).includes("function isBackendFamilyName(value)") &&
        fileText(source.path).includes("!isBackendFamilyName(backendFamily)"),
      `${source.label} must reject backend-family labels that cannot be encoded as vk_ref backend components`,
    );
    assert.match(
      fileText(source.path),
      /function\s+isBackendFamilyName\([^)]*\)\s*\{[\s\S]*\^\[a-z0-9\]\(\?:\[a-z0-9-\]\*\[a-z0-9\]\)\?\$[\s\S]*!value\.includes\("--"\)/,
      `${source.label} must reject uppercase, dotted, underscored, and repeated-separator backend-family aliases before vk_ref binding`,
    );
    assert.ok(
      fileText(source.path).includes("compactProductionClaimText(value)") &&
        fileText(source.path).includes("PRODUCTION_CLAIM_CONFUSABLES"),
      `${source.label} must fold Unicode-confusable production/mainnet/audit claim labels before compact matching`,
    );
    assert.deepEqual(
      extractJsBackendFamilyEntries(fileText(source.path), source.label),
      expected,
      `${source.label} backend-family registration map drifted from Python catalog`,
    );
    for (const [algorithmId, backendFamily] of extractJsBackendFamilyEntries(
      fileText(source.path),
      source.label,
    )) {
      assert.ok(
        isBackendFamilyName(backendFamily),
        `${source.label} backend family for ${algorithmId} must be a vk_ref backend component`,
      );
    }
    assert.equal(
      isBackendFamilyName("Halo2-ipa-pasta"),
      false,
      `${source.label} backend family validator must reject uppercase aliases`,
    );
    for (const backendFamily of [
      "halo2.ipa.pasta",
      "halo2_ipa_pasta",
      "halo2--ipa-pasta",
    ]) {
      assert.equal(
        isBackendFamilyName(backendFamily),
        false,
        `${source.label} backend family validator must reject non-canonical separator ${backendFamily}`,
      );
    }
    for (const backendFamily of [
      ".halo2-ipa-pasta",
      "-halo2-ipa-pasta",
      "_halo2-ipa-pasta",
      "halo2-ipa-pasta.",
      "halo2-ipa-pasta-",
      "halo2-ipa-pasta_",
    ]) {
      assert.equal(
        isBackendFamilyName(backendFamily),
        false,
        `${source.label} backend family validator must reject edge separator ${backendFamily}`,
      );
    }
  }
  const pythonCatalogSource = fileText("python/iroha_python/src/iroha_python/privacy_catalog.py");
  assert.ok(
    pythonCatalogSource.includes("_compact_production_claim_text") &&
      pythonCatalogSource.includes("_PRODUCTION_CLAIM_CONFUSABLES"),
    "Python privacy catalog must fold Unicode-confusable production/mainnet/audit claim labels before compact matching",
  );
  assert.ok(
    pythonCatalogSource.includes("def _is_backend_family_name(value: str) -> bool") &&
      pythonCatalogSource.includes("_is_backend_family_name(backend_family)"),
    "Python privacy catalog must reject backend-family labels that cannot be encoded as vk_ref backend components",
  );
}

function requireMatch(text, pattern, label) {
  const match = text.match(pattern);
  assert.notEqual(match, null, `${label} missing pattern ${pattern}`);
  return match;
}

function extractQuotedStringsBetween(text, startMarker, endMarker, label) {
  const start = text.indexOf(startMarker);
  assert.notEqual(start, -1, `${label} missing start marker ${startMarker}`);
  const end = text.indexOf(endMarker, start + startMarker.length);
  assert.notEqual(end, -1, `${label} missing end marker ${endMarker}`);
  const block = text.slice(start, end + endMarker.length);
  return [...block.matchAll(/"([^"]+)"/g)].map((match) => match[1]);
}

function assertBridgeMissingReasonParity(pythonCatalog) {
  const expected = canonicalBridgeMissingReasons();
  const catalogMissingReasons = new Set(
    pythonCatalog.descriptors.flatMap((descriptor) => descriptor.production_gate.missing),
  );

  for (const reason of expected) {
    assert.ok(catalogMissingReasons.has(reason), `catalog missing fail-closed reason ${reason}`);
  }
  assert.equal(
    new Set(expected).size,
    expected.length,
    "canonical bridge missing reasons must not contain duplicates",
  );

  for (const source of BRIDGE_MISSING_REASON_SOURCES) {
    const reasons = extractQuotedStringsBetween(
      fileText(source.path),
      source.start,
      source.end,
      source.label,
    );
    assert.deepEqual(
      reasons,
      expected,
      `${source.label} privacy bridge missing production-gate reasons drifted`,
    );
  }
}

function extractRustPrivacyAlgorithmEntries(text, label) {
  const start = text.indexOf("const PRIVACY_ALGORITHM_ENTRIES");
  assert.notEqual(start, -1, `${label} missing PRIVACY_ALGORITHM_ENTRIES`);
  const end = text.indexOf("struct PrivacyProductionGateStatusV1", start);
  assert.notEqual(end, -1, `${label} missing PrivacyProductionGateStatusV1 marker`);
  const block = text.slice(start, end);
  const entries = [...block.matchAll(/PrivacyAlgorithmEntry\s*\{([\s\S]*?)\n\s*\},/g)].map(
    (match, index) => {
      const entry = match[1];
      const stringField = (field) =>
        requireMatch(
          entry,
          new RegExp(`${field}:\\s*"([^"]+)"`),
          `${label} privacy algorithm entry ${index} ${field}`,
        )[1];
      const listField = (field) => {
        const list = requireMatch(
          entry,
          new RegExp(`${field}:\\s*&\\[([\\s\\S]*?)\\]`),
          `${label} privacy algorithm entry ${index} ${field}`,
        )[1];
        return [...list.matchAll(/"([^"]+)"/g)].map((item) => item[1]);
      };
      return {
        id: stringField("id"),
        proof_family: stringField("proof_family"),
        backend_family: stringField("backend_family"),
        sdk_entrypoints: listField("sdk_entrypoints"),
        planned_sdk_entrypoints: listField("planned_entrypoints"),
      };
    },
  );

  assert.ok(entries.length > 0, `${label} native privacy capability catalog is empty`);
  assert.equal(
    new Set(entries.map(({ id }) => id)).size,
    entries.length,
    `${label} native privacy capability catalog has duplicate algorithm ids`,
  );
  return entries;
}

function extractRustPrivacyProductionGateContract(text, label) {
  const version = requireMatch(
    text,
    /const PRIVACY_PRODUCTION_GATE_VERSION:\s*&str\s*=\s*"([^"]+)"/,
    `${label} native privacy production gate version`,
  )[1];
  const requirementsBlock = requireMatch(
    text,
    /const PRIVACY_PRODUCTION_GATE_REQUIREMENTS:\s*&\[\(&str,\s*&str\)\]\s*=\s*&\[([\s\S]*?)\];/,
    `${label} native privacy production gate requirements`,
  )[1];
  const requirements = [...requirementsBlock.matchAll(/\("([^"]+)",\s*"([^"]+)"\)/g)].map(
    (match) => [match[1], match[2]],
  );

  const gateStart = text.indexOf("fn privacy_production_gate()");
  assert.notEqual(gateStart, -1, `${label} missing privacy_production_gate`);
  const gateEnd = text.indexOf("fn privacy_capabilities()", gateStart);
  assert.notEqual(gateEnd, -1, `${label} missing privacy_capabilities marker`);
  const gateBlock = text.slice(gateStart, gateEnd);
  const supplementalBlock = requireMatch(
    gateBlock,
    /\.chain\(\s*\[\s*([\s\S]*?)\]\s*\.into_iter\(\)/,
    `${label} native privacy supplemental missing reasons`,
  )[1];
  const supplementalMissingReasons = [...supplementalBlock.matchAll(/"([^"]+)"/g)].map(
    (match) => match[1],
  );

  const capabilitiesStart = text.indexOf("fn privacy_capabilities()");
  assert.notEqual(capabilitiesStart, -1, `${label} missing privacy_capabilities`);
  const capabilitiesEnd = text.indexOf("fn privacy_algorithm_entry", capabilitiesStart);
  assert.notEqual(capabilitiesEnd, -1, `${label} missing privacy_algorithm_entry marker`);
  const capabilitiesBlock = text.slice(capabilitiesStart, capabilitiesEnd);

  return {
    version,
    requirements,
    supplementalMissingReasons,
    gateBlock,
    capabilitiesBlock,
  };
}

function assertRustNativeProductionGateParity(pythonCatalog) {
  const expectedGates = Object.fromEntries(
    PRODUCTION_GATE_REQUIREMENTS.map(([key]) => [key, false]),
  );
  const expectedGateEntries = PRODUCTION_GATE_REQUIREMENTS.map(([key]) => [key, false]);
  for (const descriptor of pythonCatalog.descriptors) {
    const expectedMissingReasons = [
      ...PRODUCTION_GATE_REQUIRED_REASONS,
      ...SUPPLEMENTAL_FAIL_CLOSED_REASONS.filter((reason) =>
        descriptor.production_gate.missing.includes(reason),
      ),
    ];

    assert.equal(
      descriptor.production_gate.version,
      PRODUCTION_GATE_VERSION,
      `Python catalog ${descriptor.id} production gate version drifted`,
    );
    assert.deepEqual(
      descriptor.production_gate.gates,
      expectedGates,
      `Python catalog ${descriptor.id} production gate keys drifted`,
    );
    assert.deepEqual(
      Object.entries(descriptor.production_gate.gates),
      expectedGateEntries,
      `Python catalog ${descriptor.id} production gate key order drifted`,
    );
    assert.equal(
      new Set(descriptor.production_gate.missing).size,
      descriptor.production_gate.missing.length,
      `Python catalog ${descriptor.id} production gate missing reasons contain duplicates`,
    );
    assert.deepEqual(
      descriptor.production_gate.missing,
      expectedMissingReasons,
      `Python catalog ${descriptor.id} production gate missing reasons drifted`,
    );
  }

  for (const source of RUST_PRIVACY_ALGORITHM_SOURCES) {
    const contract = extractRustPrivacyProductionGateContract(fileText(source.path), source.label);
    assert.equal(
      contract.version,
      PRODUCTION_GATE_VERSION,
      `${source.label} native privacy production gate version drifted`,
    );
    assert.deepEqual(
      contract.requirements,
      PRODUCTION_GATE_REQUIREMENTS,
      `${source.label} native privacy production gate requirements drifted`,
    );
    assert.deepEqual(
      contract.supplementalMissingReasons,
      RUST_NATIVE_SUPPLEMENTAL_FAIL_CLOSED_REASONS,
      `${source.label} native privacy supplemental missing reasons drifted`,
    );
    for (const requiredSnippet of [
      "ready: false",
      "passed: false",
      "audit_references: Vec::new()",
    ]) {
      assert.ok(
        contract.gateBlock.includes(requiredSnippet),
        `${source.label} native privacy gate must contain ${requiredSnippet}`,
      );
    }
    for (const requiredSnippet of [
      "version: PRIVACY_FFI_VERSION_V1",
      "gate_version: PRIVACY_PRODUCTION_GATE_VERSION.to_owned()",
      "production_ready: false",
      "production_gate: privacy_production_gate()",
    ]) {
      assert.ok(
        contract.capabilitiesBlock.includes(requiredSnippet),
        `${source.label} native privacy capabilities must contain ${requiredSnippet}`,
      );
    }
  }
}

function assertRustNativeCatalogParity(pythonCatalog) {
  const expected = pythonCatalog.descriptors.map((descriptor) => ({
    id: descriptor.id,
    proof_family: descriptor.proof_family,
    backend_family: descriptor.backend_family,
    sdk_entrypoints: descriptor.sdk_entrypoints,
    planned_sdk_entrypoints: descriptor.planned_sdk_entrypoints,
  }));
  for (const descriptor of expected) {
    assert.equal(
      typeof descriptor.backend_family,
      "string",
      `native privacy backend family missing for ${descriptor.id}`,
    );
  }
  for (const source of RUST_PRIVACY_ALGORITHM_SOURCES) {
    const actual = extractRustPrivacyAlgorithmEntries(fileText(source.path), source.label);
    assert.deepEqual(
      actual,
      expected,
      `${source.label} native privacy capability catalog drifted from SDK catalog`,
    );
  }
}

function assertNoDuplicateEntrypoints(label, descriptor) {
  for (const field of ["sdk_entrypoints", "planned_sdk_entrypoints"]) {
    const values = descriptor[field];
    assert.equal(
      new Set(values).size,
      values.length,
      `${label} ${descriptor.id} field ${field} must not contain duplicate entrypoints`,
    );
  }
  for (const entrypoint of descriptor.planned_sdk_entrypoints) {
    assert.equal(
      descriptor.sdk_entrypoints.includes(entrypoint),
      false,
      `${label} ${descriptor.id} planned entrypoint ${entrypoint} is already executable`,
    );
    assert.equal(
      entrypointIsDevFixture(entrypoint),
      false,
      `${label} ${descriptor.id} planned entrypoint ${entrypoint} must not be a fixture/mock entrypoint`,
    );
    assert.equal(
      entrypointIsLocalVerifier(entrypoint),
      false,
      `${label} ${descriptor.id} planned entrypoint ${entrypoint} must not be a local-only verifier entrypoint`,
    );
  }
  if (descriptor.sdk_entrypoints.some(entrypointIsLocalVerifier)) {
    assert.ok(
      descriptor.sdk_entrypoints.some(entrypointIsExplicitDevFixture),
      `${label} ${descriptor.id} local-only verifier SDK entrypoints must be paired with an explicit DevFixture entrypoint`,
    );
  }
  if (descriptor.sdk_entrypoints.some(entrypointIsExplicitDevFixture)) {
    assert.ok(
      descriptor.planned_sdk_entrypoints.some(entrypointIsProductionProofBuilder),
      `${label} ${descriptor.id} DevFixture SDK entrypoints must retain a planned production proof builder`,
    );
  }
  if (descriptor.implementation_stage === "component") {
    for (const entrypoint of [
      ...descriptor.sdk_entrypoints,
      ...descriptor.planned_sdk_entrypoints,
    ]) {
      assert.equal(
        entrypointIsInstructionBuilder(entrypoint),
        false,
        `${label} ${descriptor.id} component entrypoint ${entrypoint} must not be an instruction builder`,
      );
    }
  }
  const plannedLedgerMutations = descriptor.planned_sdk_entrypoints.filter(
    entrypointIsPlannedLedgerMutation,
  );
  if (plannedLedgerMutations.length > 0) {
    const protectionValues = [
      ...descriptor.required_state,
      ...descriptor.failure_modes,
      ...descriptor.chain_requirements,
    ].map((value) => value.toLowerCase());
    assert.ok(
      LEDGER_MUTATION_PROTECTION_METADATA_TOKENS.some((token) =>
        protectionValues.some((value) => value.includes(token)),
      ),
      `${label} ${descriptor.id} planned ledger-mutating entrypoints missing protection metadata`,
    );
    const typedAdmissionText = TYPED_CHAIN_ADMISSION_METADATA_FIELDS.flatMap(
      (field) => descriptor[field],
    ).join(" ").toLowerCase();
    assert.ok(
      TYPED_CHAIN_ADMISSION_TYPE_TOKENS.some((token) =>
        typedAdmissionText.includes(token),
      ) &&
        TYPED_CHAIN_ADMISSION_MUTATION_TOKENS.some((token) =>
          typedAdmissionText.includes(token),
      ),
      `${label} ${descriptor.id} planned ledger-mutating entrypoints missing typed chain admission metadata`,
    );
    const requiredStateText = descriptor.required_state.join(" ").toLowerCase();
    if (STATEFUL_LEDGER_STATE_TOKENS.some((token) => requiredStateText.includes(token))) {
      const persistenceText = STATEFUL_LEDGER_PERSISTENCE_METADATA_FIELDS.flatMap(
        (field) => descriptor[field],
      ).join(" ").toLowerCase();
      for (const tokens of STATEFUL_LEDGER_PERSISTENCE_TOKEN_GROUPS) {
        assert.ok(
          tokens.some((token) => persistenceText.includes(token)),
          `${label} ${descriptor.id} planned ledger-mutating entrypoints missing restart/persistence metadata for ${tokens.join("/")}`,
        );
      }
    }
  }
  if (
    WALLET_STATE_REQUIRED_IMPLEMENTATION_STAGES.has(descriptor.implementation_stage) &&
    !WALLET_STATE_REQUIRED_EXCLUDED_CATEGORIES.has(descriptor.category)
  ) {
    const requiredStateText = descriptor.required_state.join(" ").toLowerCase();
    assert.ok(
      WALLET_STATE_METADATA_TOKENS.some((token) => requiredStateText.includes(token)),
      `${label} ${descriptor.id} missing wallet or witness required-state metadata`,
    );
    const securityNotesText = descriptor.security_notes.join(" ").toLowerCase();
    for (const tokens of WALLET_WITNESS_PRIVACY_NOTE_TOKEN_GROUPS) {
      assert.ok(
        tokens.some((token) => securityNotesText.includes(token)),
        `${label} ${descriptor.id} missing wallet/witness privacy note for ${tokens.join("/")}`,
      );
    }
  }
  if (
    descriptor.implementation_stage !== null &&
    CREDENTIAL_STATE_REQUIRED_CATEGORIES.has(descriptor.category)
  ) {
    const requiredStateText = descriptor.required_state.join(" ").toLowerCase();
    assert.ok(
      CREDENTIAL_STATE_METADATA_TOKENS.some((token) => requiredStateText.includes(token)),
      `${label} ${descriptor.id} missing credential commitment/accumulator required-state metadata`,
    );
  }
  if (
    SOURCE_REFERENCED_IMPLEMENTATION_STAGES.has(descriptor.implementation_stage) &&
    descriptor.verifier_key_id !== null
  ) {
    for (const token of (descriptor.public_inputs_schema ?? "").split(",").filter(Boolean)) {
      const forbiddenSegment = token
        .split("_")
        .find((segment) => PUBLIC_INPUT_SCHEMA_FORBIDDEN_PAYLOAD_TOKEN_SEGMENTS.includes(segment));
      assert.equal(
        forbiddenSegment,
        undefined,
        `${label} ${descriptor.id} public input schema must not include proof/witness payload token ${token}`,
      );
    }
    const failureModesText = descriptor.failure_modes.join(" ").toLowerCase();
    for (const tokens of VERIFIER_NEGATIVE_FAILURE_MODE_TOKEN_GROUPS) {
      assert.ok(
        tokens.some((token) => failureModesText.includes(token)),
        `${label} ${descriptor.id} missing source-referenced verifier negative failure mode for ${tokens.join("/")}`,
      );
    }
    const verifierKeyRecordText = VERIFIER_KEY_RECORD_METADATA_FIELDS.flatMap(
      (field) => descriptor[field],
    ).join(" ").toLowerCase();
    assert.ok(
      VERIFIER_KEY_RECORD_METADATA_TOKENS.some((token) =>
        verifierKeyRecordText.includes(token),
      ),
      `${label} ${descriptor.id} missing verifier-key record metadata`,
    );
  }
  if (
    SOURCE_REFERENCED_IMPLEMENTATION_STAGES.has(descriptor.implementation_stage) &&
    descriptor.verifier_key_id !== null
  ) {
    const chainDomainBindingText = CHAIN_DOMAIN_BINDING_METADATA_FIELDS.flatMap(
      (field) => {
        const value = descriptor[field];
        return Array.isArray(value) ? value : [value];
      },
    ).join(" ").toLowerCase();
    assert.ok(
      CHAIN_DOMAIN_BINDING_METADATA_TOKENS.some((token) =>
        chainDomainBindingText.includes(token),
      ),
      `${label} ${descriptor.id} missing chain/domain binding metadata`,
    );
  }
  if (SOURCE_REFERENCED_IMPLEMENTATION_STAGES.has(descriptor.implementation_stage)) {
    const securityNotesText = descriptor.security_notes.join(" ").toLowerCase();
    for (const tokens of SOURCE_REFERENCED_HARDENING_NOTE_TOKEN_GROUPS) {
      assert.ok(
        tokens.some((token) => securityNotesText.includes(token)),
        `${label} ${descriptor.id} missing source-referenced hardening gate note for ${tokens.join("/")}`,
      );
    }
  }
  if (descriptor.implementation_stage === "research-target-as-of-2026-05") {
    assert.equal(
      descriptor.sdk_entrypoints.some(entrypointIsDevFixture),
      false,
      `${label} ${descriptor.id} research targets must not expose fixture/mock SDK entrypoints`,
    );
    assert.equal(
      descriptor.sdk_entrypoints.some(entrypointIsLocalVerifier),
      false,
      `${label} ${descriptor.id} research targets must not expose local-only verifier SDK entrypoints`,
    );
    assert.equal(
      descriptor.sdk_entrypoints.length,
      0,
      `${label} ${descriptor.id} research targets must keep executable SDK entrypoints planned-only`,
    );
    const requiredResearchSourceUrls = RESEARCH_TARGET_REQUIRED_SOURCE_URLS_BY_ID[descriptor.id];
    assert.ok(
      requiredResearchSourceUrls,
      `${label} ${descriptor.id} research target missing exact source URL contract`,
    );
    const sourceUrls = new Set(
      descriptor.source_references.map((reference) => reference.url),
    );
    for (const requiredUrl of requiredResearchSourceUrls) {
      assert.ok(
        sourceUrls.has(requiredUrl),
        `${label} ${descriptor.id} research target missing exact source URL ${requiredUrl}`,
      );
    }
    const securityNotesText = descriptor.security_notes.join(" ").toLowerCase();
    assert.ok(
      RESEARCH_TARGET_PRODUCTION_READINESS_TOKENS.every((token) =>
        securityNotesText.includes(token),
      ),
      `${label} ${descriptor.id} research target missing production readiness note`,
    );
    assert.ok(
      RESEARCH_TARGET_READINESS_EVIDENCE_TOKENS.some((token) =>
        securityNotesText.includes(token),
      ),
      `${label} ${descriptor.id} research target missing audit/review readiness note`,
    );
  }
  if (descriptor.covered_criteria.includes("post_quantum")) {
    const sourceUrls = new Set(
      descriptor.source_references.map((reference) => reference.url),
    );
    for (const requiredUrl of POST_QUANTUM_REQUIRED_SOURCE_URLS) {
      assert.ok(
        sourceUrls.has(requiredUrl),
        `${label} ${descriptor.id} post_quantum row missing source ${requiredUrl}`,
      );
    }
    const plannedEntrypointNames = descriptor.planned_sdk_entrypoints.map((entrypoint) => {
      const segments = entrypoint.split(".");
      return segments[segments.length - 1];
    });
    for (const requiredFragment of POST_QUANTUM_REQUIRED_PLANNED_ENTRYPOINT_FRAGMENTS) {
      assert.ok(
        plannedEntrypointNames.some((name) => name.includes(requiredFragment)),
        `${label} ${descriptor.id} post_quantum row missing planned SDK entrypoint fragment ${requiredFragment}`,
      );
    }
    for (const [fieldName, values, requiredTokens] of [
      ["security_notes", descriptor.security_notes, POST_QUANTUM_REQUIRED_SECURITY_NOTE_TOKENS],
      ["failure_modes", descriptor.failure_modes, POST_QUANTUM_REQUIRED_FAILURE_MODE_TOKENS],
      ["required_state", descriptor.required_state, POST_QUANTUM_REQUIRED_STATE_TOKENS],
    ]) {
      for (const requiredToken of requiredTokens) {
        assert.ok(
          values.some((value) => value.includes(requiredToken)),
          `${label} ${descriptor.id} post_quantum row missing ${fieldName} token ${requiredToken}`,
        );
      }
    }
  }
}

function assertRequiredPrivacyPlanRows(label, descriptors) {
  const descriptorById = new Map(
    descriptors.map((descriptor) => [descriptor.id, descriptor]),
  );
  for (const [algorithmId, implementationStage, backendFamily] of REQUIRED_PRIVACY_PLAN_ROWS) {
    const descriptor = descriptorById.get(algorithmId);
    assert.notEqual(
      descriptor,
      undefined,
      `${label} missing required production privacy plan row ${algorithmId}`,
    );
    assert.equal(
      descriptor.implementation_stage,
      implementationStage,
      `${label} ${algorithmId} required production privacy plan stage drifted`,
    );
    assert.equal(
      descriptor.backend_family,
      backendFamily,
      `${label} ${algorithmId} required production privacy plan backend drifted`,
    );
    assert.ok(
      descriptor.planned_sdk_entrypoints.some(entrypointIsProductionProofBuilder),
      `${label} ${algorithmId} required production privacy plan row must retain a planned production proof builder until production gates pass`,
    );
  }
}

function assertResearchTargetSdkEntrypointsFailClosed(label, descriptors) {
  assert.ok(
    descriptors.some(
      (descriptor) =>
        descriptor.implementation_stage !== "research-target-as-of-2026-05" &&
        descriptor.sdk_entrypoints.length > 0,
    ),
    `${label} non-research SDK entrypoints missing`,
  );
  for (const descriptor of descriptors) {
    if (descriptor.implementation_stage !== "research-target-as-of-2026-05") {
      continue;
    }
    assert.equal(
      descriptor.sdk_entrypoints.length,
      0,
      `${label} ${descriptor.id} research target executable SDK entrypoints must stay planned-only`,
    );
    assert.ok(
      descriptor.planned_sdk_entrypoints.length > 0,
      `${label} ${descriptor.id} research target planned SDK entrypoints missing`,
    );
  }
}

function assertExecutableEntrypointsExported(label, descriptors, moduleExports) {
  for (const descriptor of descriptors) {
    for (const entrypoint of descriptor.sdkEntrypoints) {
      assert.equal(
        typeof moduleExports[entrypoint],
        "function",
        `${label} ${descriptor.id} executable SDK entrypoint ${entrypoint} must be exported`,
      );
    }
  }
}

function assertExecutableEntrypointsDeclared(label, descriptors, declarationText) {
  for (const descriptor of descriptors) {
    for (const entrypoint of descriptor.sdkEntrypoints) {
      assert.equal(
        new RegExp(`\\bexport\\s+function\\s+${escapeRegExp(entrypoint)}\\s*\\(`).test(
          declarationText,
        ),
        true,
        `${label} ${descriptor.id} executable SDK entrypoint ${entrypoint} must be declared`,
      );
    }
  }
}

function assertCatalogParity(label, criteria, descriptors, pythonCatalog) {
  const normalizedDescriptors = descriptors.map(toPythonDescriptorShape);
  assert.deepEqual(criteria, pythonCatalog.criteria, `${label} privacy criteria drifted`);
  assert.deepEqual(
    normalizedDescriptors.map(({ id }) => id),
    pythonCatalog.descriptors.map(({ id }) => id),
    `${label} privacy algorithm id order drifted`,
  );
  assertRequiredPrivacyPlanRows(label, normalizedDescriptors);
  assertResearchTargetSdkEntrypointsFailClosed(label, normalizedDescriptors);

  const verifierKeyIds = normalizedDescriptors
    .map((descriptor) => descriptor.verifier_key_id)
    .filter((verifierKeyId) => verifierKeyId !== null);
  assert.equal(
    new Set(verifierKeyIds).size,
    verifierKeyIds.length,
    `${label} privacy verifier key ids must be unique`,
  );

  for (const [index, descriptor] of normalizedDescriptors.entries()) {
    assert.deepEqual(
      descriptor,
      Object.fromEntries(
        Object.entries(pythonCatalog.descriptors[index]).filter(([key]) =>
          Object.hasOwn(descriptor, key),
        ),
      ),
      `${label} privacy algorithm descriptor ${descriptor.id} drifted from Python catalog`,
    );
    assertFailClosedDescriptor(label, descriptor);
    assertNoDuplicateEntrypoints(label, descriptor);
  }
}

test("privacy algorithm catalogs stay fail-closed and in parity across JS and Python", () => {
  const pythonCatalog = loadPythonPrivacyCatalog();

  assertCatalogParity(
    "src",
    getSrcPrivacyCriteria(),
    getSrcPrivacyAlgorithmDescriptors(),
    pythonCatalog,
  );
  assertCatalogParity(
    "dist",
    getDistPrivacyCriteria(),
    getDistPrivacyAlgorithmDescriptors(),
    pythonCatalog,
  );
  assertBridgeMissingReasonParity(pythonCatalog);
  assertBackendFamilyRegistrationParity(pythonCatalog);
  assertPythonCatalogDefensiveCopyCoverage();
  assertRustNativeProductionGateParity(pythonCatalog);
  assertRustNativeCatalogParity(pythonCatalog);
});

test("privacy algorithm catalogs require proof builders on required production plan rows", () => {
  for (const [label, text] of [
    ["JS source", fileText("javascript/iroha_js/src/privacyAlgorithms.js")],
    ["JS dist", fileText("javascript/iroha_js/dist/privacyAlgorithms.js")],
  ]) {
    assert.match(
      text,
      /function\s+entrypointIsProofHelper\([^)]*\)\s*\{[\s\S]*ProofEnvelope[\s\S]*ProofWitness[\s\S]*ProofPublicInputs[\s\S]*ProofRequest[\s\S]*ProofCommitment/,
      `${label} must classify proof helper and wrapper entrypoints`,
    );
    assert.match(
      text,
      /function\s+entrypointIsProductionProofBuilder\([^)]*\)\s*\{[\s\S]*name\.startsWith\("build"\)[\s\S]*name\.includes\("Proof"\)[\s\S]*!entrypointIsInstructionBuilder\(entrypoint\)[\s\S]*!entrypointIsPlannedLedgerMutation\(entrypoint\)[\s\S]*!entrypointIsProofHelper\(entrypoint\)[\s\S]*!entrypointIsDevFixture\(entrypoint\)/,
      `${label} production proof-builder classifier must reject ledger mutations and proof helpers`,
    );
    assert.match(
      text,
      /function\s+validateRequiredPrivacyPlanRows\([^)]*\)\s*\{[\s\S]*REQUIRED_PRIVACY_PLAN_ROWS[\s\S]*entrypointIsProductionProofBuilder[\s\S]*must retain a planned production proof builder/,
      `${label} required production plan rows must require planned production proof builders`,
    );
  }

  const pythonCatalogSource = fileText(
    "python/iroha_python/src/iroha_python/privacy_catalog.py",
  );
  assert.match(
    pythonCatalogSource,
    /def\s+_validate_required_privacy_plan_rows[\s\S]*REQUIRED_PRIVACY_PLAN_ROWS[\s\S]*_entrypoint_is_production_proof_builder[\s\S]*must retain a planned production proof/,
    "Python required production plan rows must require planned production proof builders",
  );
  assert.match(
    pythonCatalogSource,
    /def\s+_entrypoint_is_production_proof_builder[\s\S]*_entrypoint_is_instruction_builder\(entrypoint\)[\s\S]*_entrypoint_is_planned_ledger_mutation\(entrypoint\)[\s\S]*_entrypoint_is_dev_fixture\(entrypoint\)/,
    "Python production proof-builder classifier must reject ledger mutations",
  );
  assert.match(
    pythonCatalogSource,
    /def\s+_entrypoint_is_proof_helper[\s\S]*ProofEnvelope[\s\S]*ProofWitness[\s\S]*ProofPublicInputs[\s\S]*ProofRequest[\s\S]*ProofCommitment[\s\S]*def\s+_entrypoint_is_production_proof_builder[\s\S]*_entrypoint_is_proof_helper\(entrypoint\)/,
    "Python production proof-builder classifier must reject proof helpers",
  );

  const pythonCatalogTests = fileText("python/iroha_python/tests/privacy_catalog_test.py");
  assert.match(
    pythonCatalogTests,
    /(?=[\s\S]*test_privacy_catalog_rejects_required_production_privacy_plan_without_proof_builder)(?=[\s\S]*deriveOrchardWitness)(?=[\s\S]*buildAnonymousPgcProductionInstruction)(?=[\s\S]*buildAnonymousPgcProofTransaction)(?=[\s\S]*buildSubmitAnonymousPgcProof)(?=[\s\S]*buildAnonymousPgcProofEnvelope)(?=[\s\S]*buildAnonymousPgcProofWitness)(?=[\s\S]*buildAnonymousPgcProofPublicInputs)(?=[\s\S]*buildAnonymousPgcProofRequest)(?=[\s\S]*buildAnonymousPgcProofCommitment)(?=[\s\S]*buildAnonymousPgcDevProofFixture)/,
    "Python tests must cover helper-only, instruction-only, transaction-only, submit-only, proof-helper-only, and fixture-only required rows",
  );
});

test("privacy algorithm JS getters return immutable fail-closed production metadata", () => {
  for (const [label, getCapabilities, getDescriptors, getDescriptor] of [
    [
      "src",
      getSrcPrivacyCapabilities,
      getSrcPrivacyAlgorithmDescriptors,
      getSrcPrivacyAlgorithmDescriptor,
    ],
    [
      "dist",
      getDistPrivacyCapabilities,
      getDistPrivacyAlgorithmDescriptors,
      getDistPrivacyAlgorithmDescriptor,
    ],
  ]) {
    const capabilities = getCapabilities();
    const descriptors = getDescriptors();
    const descriptor = descriptors.find((entry) => entry.plannedSdkEntrypoints.length > 0);
    assert.ok(descriptor, `${label} must expose a planned fail-closed privacy row`);
    const lookup = getDescriptor(descriptor.id);
    assert.ok(lookup, `${label} single descriptor lookup must find ${descriptor.id}`);

    assert.notEqual(descriptor, lookup, `${label} descriptor getters must return fresh objects`);
    assert.equal(Object.isFrozen(capabilities), true, `${label} capabilities must be frozen`);
    assert.equal(
      Object.isFrozen(capabilities.privacyAlgorithms),
      true,
      `${label} privacy algorithm array must be frozen`,
    );
    assert.equal(
      Object.isFrozen(descriptors),
      true,
      `${label} descriptor array must be frozen`,
    );

    for (const frozenDescriptor of [descriptor, lookup]) {
      assert.equal(
        Object.isFrozen(frozenDescriptor),
        true,
        `${label} descriptor ${frozenDescriptor.id} must be frozen`,
      );
      assert.equal(
        Object.isFrozen(frozenDescriptor.pqLayers),
        true,
        `${label} descriptor ${frozenDescriptor.id} pqLayers must be frozen`,
      );
      assert.equal(
        Object.isFrozen(frozenDescriptor.sourceReferences),
        true,
        `${label} descriptor ${frozenDescriptor.id} sourceReferences must be frozen`,
      );
      if (frozenDescriptor.sourceReferences.length > 0) {
        assert.equal(
          Object.isFrozen(frozenDescriptor.sourceReferences[0]),
          true,
          `${label} descriptor ${frozenDescriptor.id} sourceReference rows must be frozen`,
        );
      }
      assert.equal(
        Object.isFrozen(frozenDescriptor.productionGate),
        true,
        `${label} descriptor ${frozenDescriptor.id} productionGate must be frozen`,
      );
      assert.equal(
        Object.isFrozen(frozenDescriptor.productionGate.gates),
        true,
        `${label} descriptor ${frozenDescriptor.id} productionGate.gates must be frozen`,
      );
      assert.equal(
        Object.isFrozen(frozenDescriptor.productionGate.missing),
        true,
        `${label} descriptor ${frozenDescriptor.id} productionGate.missing must be frozen`,
      );
      assert.equal(
        Object.isFrozen(frozenDescriptor.productionGate.auditReferences),
        true,
        `${label} descriptor ${frozenDescriptor.id} productionGate.auditReferences must be frozen`,
      );
      assert.equal(frozenDescriptor.productionReady, false);
      assert.equal(frozenDescriptor.productionGate.ready, false);
      assert.equal(frozenDescriptor.productionGate.gates.external_audit, false);
      assert.ok(
        frozenDescriptor.productionGate.missing.includes("planned SDK entrypoints remain"),
        `${label} descriptor ${frozenDescriptor.id} must expose planned-entrypoint production blocker`,
      );

      assert.throws(() => {
        frozenDescriptor.productionReady = true;
      });
      assert.throws(() => {
        frozenDescriptor.productionGate.ready = true;
      });
      assert.throws(() => {
        frozenDescriptor.productionGate.gates.external_audit = true;
      });
      assert.throws(() => {
        frozenDescriptor.productionGate.missing.length = 0;
      });
      assert.throws(() => {
        frozenDescriptor.productionGate.auditReferences.push({
          label: "forged audit",
          url: "https://audit.example/forged",
        });
      });
      assert.throws(() => {
        frozenDescriptor.pqLayers.proof = true;
      });
      assert.throws(() => {
        frozenDescriptor.plannedSdkEntrypoints.length = 0;
      });
      assert.throws(() => {
        frozenDescriptor.sourceReferences.push({
          label: "forged source",
          url: "https://audit.example/forged",
        });
      });
    }

    assert.throws(() => {
      capabilities.privacyAlgorithms.length = 0;
    });
    assert.throws(() => {
      capabilities.privacyCriteria.push("tampered");
    });

    const fresh = getDescriptor(descriptor.id);
    assert.ok(fresh, `${label} fresh descriptor lookup must find ${descriptor.id}`);
    assert.equal(fresh.productionReady, false);
    assert.equal(fresh.productionGate.ready, false);
    assert.equal(fresh.productionGate.gates.external_audit, false);
    assert.ok(
      fresh.productionGate.missing.includes("planned SDK entrypoints remain"),
      `${label} fresh descriptor ${descriptor.id} must remain fail-closed`,
    );
    assert.deepEqual(
      fresh.plannedSdkEntrypoints,
      descriptor.plannedSdkEntrypoints,
      `${label} planned SDK entrypoints must survive attempted mutation`,
    );
  }
});

test("privacy algorithm JS validators reject supplied derived production fields", () => {
  for (const field of DERIVED_JS_COMPATIBILITY_FIELDS) {
    assertJsValidatorsReject(
      { [field]: field.endsWith("Gate") || field.endsWith("_gate") ? { ready: true } : "forged" },
      new RegExp(`field ${field} is derived and must not be supplied`),
    );
  }
});

test("privacy algorithm JS validators reject hostile catalog descriptor shapes", () => {
  for (const [patch, pattern] of [
    [
      { id: "unmapped-backend-family" },
      /registered non-none backend family/,
    ],
    [
      { auditReferences: [{ label: "forged", url: "https://audit.example/forged" }] },
      /field auditReferences is not a supported privacy catalog field/,
    ],
    [{ shortName: "" }, /shortName must be a non-empty string/],
    [{ shortName: " Shape" }, /shortName must be clean and already trimmed/],
    [{ summary: "   " }, /summary must be a non-empty string/],
    [{ summary: "Descriptor\u007fsummary" }, /summary must be clean and already trimmed/],
    [
      { summary: "Mainnet-ready audited production proof." },
      /summary must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { summary: "M\u0430innet-re\u0430dy proof." },
      /summary must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { summary: "Claimed production proof." },
      /summary must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { name: "Claimed mainnet transfer" },
      /name must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { shortName: "Audit claim" },
      /shortName must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { id: "mainnet-ready-shield" },
      /id must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { id: "claimed-mainnet-shield" },
      /id must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [{ id: "Shield" }, /id must be lowercase and URL-safe/],
    [{ id: "shield.v1" }, /id must be lowercase and URL-safe/],
    [{ id: "shield/../../admin" }, /id must be lowercase and URL-safe/],
    [{ id: "_shield" }, /id must be lowercase and URL-safe/],
    [{ id: "-shield" }, /id must be lowercase and URL-safe/],
    [{ id: "shield_" }, /id must be lowercase and URL-safe/],
    [{ id: "shield-" }, /id must be lowercase and URL-safe/],
    [{ proofFamily: "" }, /proofFamily must be a non-empty string/],
    [{ proofFamily: " halo2-ipa" }, /proofFamily must be clean and already trimmed/],
    [{ proofFamily: "Halo2" }, /proofFamily must be a proof family name/],
    [{ proofFamily: "halo2..ipa" }, /proofFamily must be a proof family name/],
    [{ proofFamily: "halo2/../ipa" }, /proofFamily must be a proof family name/],
    [{ proofFamily: "halo2--ipa" }, /proofFamily must be a proof family name/],
    [{ proofFamily: "/halo2" }, /proofFamily must be a proof family name/],
    [{ proofFamily: "-halo2" }, /proofFamily must be a proof family name/],
    [{ proofFamily: "halo2/" }, /proofFamily must be a proof family name/],
    [{ proofFamily: "halo2-" }, /proofFamily must be a proof family name/],
    [
      { proofFamily: "halo2/mainnet-ready" },
      /proofFamily must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { proofFamily: "halo2/production-claim" },
      /proofFamily must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { publicInputsSchema: "" },
      /publicInputsSchema must be a non-empty string or null/,
    ],
    [
      { publicInputsSchema: "root,\nproof" },
      /publicInputsSchema must be clean and already trimmed/,
    ],
    [
      { publicInputsSchema: "root," },
      /publicInputsSchema token 1 must be a non-empty public input name/,
    ],
    [
      { publicInputsSchema: "root, proof" },
      /publicInputsSchema token 1 must be clean and already trimmed/,
    ],
    [
      { publicInputsSchema: "root,Proof" },
      /publicInputsSchema token 1 must be a lowercase public input name/,
    ],
    [
      { publicInputsSchema: "root,1proof" },
      /publicInputsSchema token 1 must be a lowercase public input name/,
    ],
    [
      { publicInputsSchema: "root,field_" },
      /publicInputsSchema token 1 must be a lowercase public input name/,
    ],
    [
      { publicInputsSchema: "root,field__digest" },
      /publicInputsSchema token 1 must be a lowercase public input name/,
    ],
    [
      { publicInputsSchema: "root,proof" },
      /publicInputsSchema token 1 must not include proof or witness payload metadata/,
    ],
    [
      { publicInputsSchema: "root,recursive_proof_digest" },
      /publicInputsSchema token 1 must not include proof or witness payload metadata/,
    ],
    [
      { publicInputsSchema: "root,wallet_witness_digest" },
      /publicInputsSchema token 1 must not include proof or witness payload metadata/,
    ],
    [
      { publicInputsSchema: "root,production_gate_passed" },
      /publicInputsSchema token 1 must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { publicInputsSchema: "root,audit_claim" },
      /publicInputsSchema token 1 must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { publicInputsSchema: "root,root" },
      /publicInputsSchema token 1 duplicates root/,
    ],
    [
      { verifierKeyId: "   " },
      /verifierKeyId must be a non-empty string or null/,
    ],
    [
      { verifierKeyId: 7 },
      /verifierKeyId must be a non-empty string or null/,
    ],
    [
      { verifierKeyId: "zk::Shield\t" },
      /verifierKeyId must be clean and already trimmed/,
    ],
    [
      { publicInputsSchema: null, verifierKeyId: "orphan_verifier_key" },
      /publicInputsSchema and verifierKeyId must be supplied together/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: null },
      /publicInputsSchema and verifierKeyId must be supplied together/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: "VerifierKey" },
      /verifierKeyId must be a verifier key id/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: "verifier_key_" },
      /verifierKeyId must be a verifier key id/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: "verifier__key" },
      /verifierKeyId must be a verifier key id/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: "verifier.key" },
      /verifierKeyId must be a verifier key id/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: "zk:Shield" },
      /verifierKeyId must be a verifier key id/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: "zk_::Shield" },
      /verifierKeyId must be a verifier key id/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: "zk::" },
      /verifierKeyId must be a verifier key id/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: "zk::Shield_" },
      /verifierKeyId must be a verifier key id/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: "zk::Shield__Key" },
      /verifierKeyId must be a verifier key id/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: "zk::Shield/../../admin" },
      /verifierKeyId must be a verifier key id/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: "audited_production_vk" },
      /verifierKeyId must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [{ category: "production_claim" }, /category must be a known category/],
    [{ maturity: "audited" }, /maturity must be a known maturity/],
    [
      { implementationStage: "Production Hardened" },
      /implementationStage must be a lowercase hyphenated identifier/,
    ],
    [
      { implementationStage: "audited-production" },
      /implementationStage must be a known implementation stage/,
    ],
    [
      { implementationStage: "production-ready" },
      /implementationStage must be a known implementation stage/,
    ],
    [{ coveredCriteria: ["hide_sender", "hide_sender"] }, /duplicates hide_sender/],
    [{ coveredCriteria: ["hide_operator"] }, /must be a known privacy criterion/],
    [
      { recommendedFor: ["audit evidence", "audit evidence"] },
      /recommendedFor\[1\] duplicates audit evidence/,
    ],
    [
      { recommendedFor: [" audit evidence"] },
      /recommendedFor\[0\] must be clean and already trimmed/,
    ],
    [
      { recommendedFor: ["Production-ready bank deployment"] },
      /recommendedFor\[0\] must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { recommendedFor: ["claimed audit rollout"] },
      /recommendedFor\[0\] must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { chainRequirements: ["production-ready verifier"] },
      /chainRequirements\[0\] must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { requiredState: ["claimed mainnet root"] },
      /requiredState\[0\] must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { setupSteps: ["Install audit claim verifier"] },
      /setupSteps\[0\] must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { executionSteps: ["Submit claimed production proof"] },
      /executionSteps\[0\] must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { sdkEntrypoints: ["buildMainnetReadyProof"] },
      /sdkEntrypoints\[0\] must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { plannedSdkEntrypoints: ["buildAuditSignoffProof"] },
      /plannedSdkEntrypoints\[0\] must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { plannedSdkEntrypoints: ["buildClaimedAuditProof"] },
      /plannedSdkEntrypoints\[0\] must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { securityNotes: ["line\nbreak"] },
      /securityNotes\[0\] must be clean and already trimmed/,
    ],
    [
      { securityNotes: ["External audit completed and production sign-off received."] },
      /securityNotes\[0\] must describe missing audit\/review gates, not completed audit or signoff claims/,
    ],
    [
      { securityNotes: ["A.u.d.i.t passed; s.e.c.u.r.i.t.y review approved."] },
      /securityNotes\[0\] must describe missing audit\/review gates, not completed audit or signoff claims/,
    ],
    [
      { securityNotes: ["External \u0430udit p\u0430ssed."] },
      /securityNotes\[0\] must describe missing audit\/review gates, not completed audit or signoff claims/,
    ],
    [
      { securityNotes: ["Claimed audit coverage is present."] },
      /securityNotes\[0\] must describe missing audit\/review gates, not completed audit or signoff claims/,
    ],
    [
      { securityNotes: ["Mainnet claim accepted by reviewer."] },
      /securityNotes\[0\] must describe missing audit\/review gates, not completed audit or signoff claims/,
    ],
    [
      { failureModes: ["External audit completed."] },
      /failureModes\[0\] must describe concrete failure modes, not completed audit or signoff claims/,
    ],
    [
      { failureModes: ["Mainnet claim accepted by reviewer."] },
      /failureModes\[0\] must describe concrete failure modes, not completed audit or signoff claims/,
    ],
    [
      { chainRequirements: ["verifier registry", "verifier registry"] },
      /chainRequirements\[1\] duplicates verifier registry/,
    ],
    [
      { chainRequirements: ["registry\u007f"] },
      /chainRequirements\[0\] must be clean and already trimmed/,
    ],
    [
      { sourceReferences: [{ label: " paper", url: "https://example.invalid" }] },
      /sourceReferences\[0\]\.label must be clean and bounded/,
    ],
    [
      { sourceReferences: [{ label: "paper\nnext", url: "https://example.invalid" }] },
      /sourceReferences\[0\]\.label must be clean and bounded/,
    ],
    [
      { sourceReferences: [{ label: "paper\u007f", url: "https://example.invalid" }] },
      /sourceReferences\[0\]\.label must be clean and bounded/,
    ],
    [
      { sourceReferences: [{ label: "p".repeat(161), url: "https://example.invalid" }] },
      /sourceReferences\[0\]\.label must be clean and bounded/,
    ],
    [
      { sourceReferences: [{ label: "External audit signoff", url: "https://zips.z.cash/zip-0224" }] },
      /sourceReferences\[0\]\.label must describe protocol source material, not audit\/signoff evidence/,
    ],
    [
      { sourceReferences: [{ label: "Protocol s.e.c.u.r.i.t.y review", url: "https://zips.z.cash/zip-0224" }] },
      /sourceReferences\[0\]\.label must describe protocol source material, not audit\/signoff evidence/,
    ],
    [
      { sourceReferences: [{ label: "Protocol security rev\u0456ew", url: "https://zips.z.cash/zip-0224" }] },
      /sourceReferences\[0\]\.label must describe protocol source material, not audit\/signoff evidence/,
    ],
    [
      { sourceReferences: [{ label: "External.review report", url: "https://zips.z.cash/zip-0224" }] },
      /sourceReferences\[0\]\.label must describe protocol source material, not audit\/signoff evidence/,
    ],
    [
      { sourceReferences: [{ label: "\u0391ssurance.report", url: "https://zips.z.cash/zip-0224" }] },
      /sourceReferences\[0\]\.label must describe protocol source material, not audit\/signoff evidence/,
    ],
    [
      { sourceReferences: [{ label: "Production-ready protocol source", url: "https://zips.z.cash/zip-0224" }] },
      /sourceReferences\[0\]\.label must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "http://example.invalid" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "HTTPS://example.invalid" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: " https://example.invalid" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://example.invalid/path\nnext" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://user:pass@example.invalid" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.ca\u0455h/zip-0224" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://xn--cah-ghd.org/source" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/prot\u03bfcol/protocol.pdf" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224?claim=m\u0430innet" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://ZIPS.z.cash/zip-0224" }] },
      /sourceReferences\[0\]\.url must be canonical/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash:443/zip-0224" }] },
      /sourceReferences\[0\]\.url must be canonical/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash./zip-0224" }] },
      /sourceReferences\[0\]\.url must be canonical/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/protocol/../zip-0224" }] },
      /sourceReferences\[0\]\.url must be canonical/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/protocol/%2e%2e/zip-0224" }] },
      /sourceReferences\[0\]\.url must be canonical/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://127%2e0%2e0%2e1/source" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://localhost%2elocaltest%2eme/source" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://example.invalid\\evil" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224?section=notes%ZZappendix" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224#external-audit-complete" }] },
      /sourceReferences\[0\]\.url must describe protocol source material, not audit\/signoff or readiness evidence/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224?production=ready" }] },
      /sourceReferences\[0\]\.url must describe protocol source material, not audit\/signoff or readiness evidence/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224?evidence=audit%3Dcomplete" }] },
      /sourceReferences\[0\]\.url must describe protocol source material, not audit\/signoff or readiness evidence/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224?evidence=production%253Dready" }] },
      /sourceReferences\[0\]\.url must describe protocol source material, not audit\/signoff or readiness evidence/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224?evidence=mainnet%2520claim" }] },
      /sourceReferences\[0\]\.url must describe protocol source material, not audit\/signoff or readiness evidence/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224#external-%2561udit-complete" }] },
      /sourceReferences\[0\]\.url must describe protocol source material, not audit\/signoff or readiness evidence/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224?evidence=production%2525253Dready" }] },
      /sourceReferences\[0\]\.url must describe protocol source material, not audit\/signoff or readiness evidence/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://audit.example/forged-signoff" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://127.0.0.1.nip.io/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://10.0.0.1.sslip.io/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://localhost.localtest.me/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://lvh.me/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://[::ffff:127.0.0.1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://[::7f00:1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://[64:ff9b::7f00:1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://[::ffff:c0a8:101]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://[2001:0000:4136:e378:8000:63bf:3fff:fdd2]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://[100::]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://[2001:20::1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://[fec0::1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://[2002:7f00:1::]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      {
        sourceReferences: [
          {
            label: "paper",
            url: "https://example.invalid",
            productionGate: { ready: true },
          },
        ],
      },
      /sourceReferences\[0\] field productionGate is not supported/,
    ],
    [
      {
        sourceReferences: [
          { label: "paper", url: "https://zips.z.cash/zip-0224" },
          { label: "paper", url: "https://zips.z.cash/zip-0225" },
        ],
      },
      /sourceReferences\[1\] duplicates label paper/,
    ],
    [
      {
        sourceReferences: [
          { label: "paper A", url: "https://zips.z.cash/zip-0224" },
          { label: "paper B", url: "https://zips.z.cash/zip-0224" },
        ],
      },
      /sourceReferences\[1\] duplicates url https:\/\/zips\.z\.cash\/zip-0224/,
    ],
    [
      { implementationStage: "chain-executable", sourceReferences: [] },
      /sourceReferences is required for source-referenced implementation stages/,
    ],
    [
      { implementationStage: "sdk-builder", sourceReferences: undefined },
      /sourceReferences is required for source-referenced implementation stages/,
    ],
    [
      { implementationStage: "component", sourceReferences: [] },
      /sourceReferences is required for source-referenced implementation stages/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        securityNotes: ["Production readiness requires audit review."],
        sourceReferences: [],
      },
      /sourceReferences is required for source-referenced implementation stages/,
    ],
    [
      {
        id: "orchard-halo2-actions-v1",
        implementationStage: "research-target-as-of-2026-05",
        sourceReferences: [
          {
            label: "Zcash Protocol Specification",
            url: "https://zips.z.cash/protocol/protocol.pdf",
          },
        ],
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildOrchardActionBundleProofV1"],
      },
      /sourceReferences must include exact research target source URLs/,
    ],
    [
      {
        id: "orchard-halo2-actions-v1",
        implementationStage: "research-target-as-of-2026-05",
        sourceReferences: [
          {
            label: "ZIP 224 Orchard Shielded Protocol",
            url: "https://zips.z.cash/zip-0224",
          },
        ],
        securityNotes: [
          "Orchard note semantics must remain domain-separated.",
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
          "Hardening gates require parser fuzzing, performance review, and external audit.",
        ],
        requiredState: [
          "Orchard note commitment tree",
          "wallet Orchard witness store",
        ],
        failureModes: [
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildOrchardActionBundleProofV1"],
      },
      /securityNotes must include production readiness audit or review gating for research targets/,
    ],
    [
      { implementationStage: "production-hardened", sourceReferences: [] },
      /sourceReferences is required for source-referenced implementation stages/,
    ],
    [
      { sourceReferences: [{ label: "placeholder", url: "https://example.invalid/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "test", url: "https://example.test/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "example", url: "https://example.com/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "localhost", url: "https://localhost/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "loopback", url: "https://127.0.0.1/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "private", url: "https://10.0.0.1/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "private", url: "https://172.16.0.1/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "private", url: "https://192.168.1.10/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "link local", url: "https://169.254.1.1/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "carrier nat", url: "https://100.64.0.1/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "documentation", url: "https://192.0.2.1/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "documentation", url: "https://198.51.100.10/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "documentation", url: "https://203.0.113.5/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "ipv6 loopback", url: "https://[::1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "ipv6 link local", url: "https://[fe80::1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "ipv6 ula", url: "https://[fc00::1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "ipv6 documentation", url: "https://[2001:db8::1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "local dns", url: "https://source.local/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "internal dns", url: "https://source.internal/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { implementationStage: "sdk-builder", recommendedFor: [] },
      /recommendedFor must be non-empty for source-referenced implementation stages/,
    ],
    [
      { implementationStage: "component", recommendedFor: undefined },
      /recommendedFor must be non-empty for source-referenced implementation stages/,
    ],
    [
      { implementationStage: "chain-executable", chainRequirements: [] },
      /chainRequirements must be non-empty for source-referenced implementation stages/,
    ],
    [
      { implementationStage: "sdk-builder", securityNotes: [] },
      /securityNotes must be non-empty for source-referenced implementation stages/,
    ],
    [
      { implementationStage: "component", requiredState: [] },
      /requiredState must be non-empty for source-referenced implementation stages/,
    ],
    [
      { implementationStage: "chain-executable", failureModes: [] },
      /failureModes must be non-empty for source-referenced implementation stages/,
    ],
    [
      { implementationStage: "sdk-builder", setupSteps: [] },
      /setupSteps must be non-empty for source-referenced implementation stages/,
    ],
    [
      { implementationStage: "component", executionSteps: [] },
      /executionSteps must be non-empty for source-referenced implementation stages/,
    ],
    [
      {
        implementationStage: "production-hardened",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [],
      },
      /source-referenced implementation stages must expose at least one executable or planned SDK entrypoint/,
    ],
    [
      {
        implementationStage: "sdk-builder",
        publicInputsSchema: null,
        verifierKeyId: null,
        plannedSdkEntrypoints: ["buildFutureShapeProof"],
      },
      /publicInputsSchema must be non-empty for source-referenced implementation stages/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        publicInputsSchema: null,
        verifierKeyId: null,
        plannedSdkEntrypoints: ["buildFutureShapeProof"],
      },
      /publicInputsSchema must be non-empty for source-referenced implementation stages/,
    ],
    [
      {
        implementationStage: "production-hardened",
        publicInputsSchema: null,
        verifierKeyId: null,
      },
      /publicInputsSchema must be non-empty for source-referenced implementation stages/,
    ],
    [
      {
        implementationStage: "sdk-builder",
        proofFamily: "none",
        plannedSdkEntrypoints: ["buildFutureShapeProof"],
      },
      /proofFamily must be a concrete proof family for source-referenced implementation stages/,
    ],
    [
      {
        implementationStage: "production-hardened",
        proofFamily: "none",
      },
      /proofFamily must be a concrete proof family for source-referenced implementation stages/,
    ],
    [
      {
        id: "transparent-transfer",
        implementationStage: "sdk-builder",
        plannedSdkEntrypoints: ["buildFutureShapeProof"],
      },
      /registered non-none backend family for source-referenced implementation stages/,
    ],
    [
      {
        id: "unmapped-backend-family",
        implementationStage: "production-hardened",
      },
      /registered non-none backend family for source-referenced implementation stages/,
    ],
    [
      {
        implementationStage: "sdk-builder",
        sdkEntrypoints: ["buildShapeProof"],
        plannedSdkEntrypoints: [],
      },
      /plannedSdkEntrypoints must be non-empty for pre-production source-referenced implementation stages/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [],
      },
      /plannedSdkEntrypoints must be non-empty for pre-production source-referenced implementation stages/,
    ],
    [
      {
        pqLayers: {
          proof: false,
          authorization: false,
          noteEncryption: false,
          audit: true,
        },
      },
      /pqLayers field audit is not supported/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: false,
          authorization: true,
          noteEncryption: true,
        },
      },
      /coveredCriteria post_quantum requires all pqLayers to be true/,
    ],
    [
      {
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: false,
          noteEncryption: true,
        },
      },
      /coveredCriteria post_quantum requires all pqLayers to be true/,
    ],
    [
      {
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: false,
        },
      },
      /coveredCriteria post_quantum requires all pqLayers to be true/,
    ],
    [
      {
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: false,
          authorization: false,
          noteEncryption: false,
        },
      },
      /coveredCriteria post_quantum requires all pqLayers to be true/,
    ],
    [
      {
        coveredCriteria: [],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
      },
      /pqLayers with all layers true requires coveredCriteria post_quantum/,
    ],
    [
      {
        coveredCriteria: ["hide_amount"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
      },
      /pqLayers with all layers true requires coveredCriteria post_quantum/,
    ],
    [
      {
        coveredCriteria: ["hide_amount", "hide_sender"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
      },
      /pqLayers with all layers true requires coveredCriteria post_quantum/,
    ],
    [
      {
        id: "pq-masp-stark-v0",
        implementationStage: "research-target-as-of-2026-05",
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
        sourceReferences: [
          {
            label: "FIPS 203",
            url: "https://csrc.nist.gov/pubs/fips/203/final",
          },
          {
            label: "FIPS 204",
            url: "https://csrc.nist.gov/pubs/fips/204/final",
          },
        ],
      },
      /sourceReferences must include NIST FIPS 203, FIPS 204, and FIPS 205/,
    ],
    [
      {
        id: "pq-masp-stark-v0",
        implementationStage: "research-target-as-of-2026-05",
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
        sourceReferences: [
          {
            label: "FIPS 203",
            url: "https://csrc.nist.gov/pubs/fips/203/final",
          },
          {
            label: "FIPS 204",
            url: "https://csrc.nist.gov/pubs/fips/204/final",
          },
          {
            label: "FIPS 205",
            url: "https://csrc.nist.gov/pubs/fips/205/final",
          },
        ],
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildPqMaspStarkTransferProofV0",
          "encapsulateMlKem",
        ],
      },
      /plannedSdkEntrypoints must include planned ML-DSA authorization and ML-KEM note-encryption SDK entrypoints/,
    ],
    [
      {
        id: "pq-masp-stark-v0",
        implementationStage: "research-target-as-of-2026-05",
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
        sourceReferences: [
          {
            label: "FIPS 203",
            url: "https://csrc.nist.gov/pubs/fips/203/final",
          },
          {
            label: "FIPS 204",
            url: "https://csrc.nist.gov/pubs/fips/204/final",
          },
          {
            label: "FIPS 205",
            url: "https://csrc.nist.gov/pubs/fips/205/final",
          },
        ],
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildPqMaspStarkTransferProofV0",
          "generateMlDsaKeyPair",
        ],
      },
      /plannedSdkEntrypoints must include planned ML-DSA authorization and ML-KEM note-encryption SDK entrypoints/,
    ],
    [
      {
        id: "pq-masp-stark-v0",
        implementationStage: "research-target-as-of-2026-05",
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
        sourceReferences: [
          {
            label: "FIPS 203",
            url: "https://csrc.nist.gov/pubs/fips/203/final",
          },
          {
            label: "FIPS 204",
            url: "https://csrc.nist.gov/pubs/fips/204/final",
          },
          {
            label: "FIPS 205",
            url: "https://csrc.nist.gov/pubs/fips/205/final",
          },
        ],
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildPqMaspStarkTransferProofV0",
          "generateMlDsaKeyPair",
          "encapsulateMlKem",
        ],
        securityNotes: ["ML-DSA domains require audit"],
        failureModes: ["ML-DSA or ML-KEM domain mismatch"],
        requiredState: ["ML-KEM encrypted note payload store"],
      },
      /securityNotes must include post-quantum primitive risk notes/,
    ],
    [
      {
        id: "pq-masp-stark-v0",
        implementationStage: "research-target-as-of-2026-05",
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
        sourceReferences: [
          {
            label: "FIPS 203",
            url: "https://csrc.nist.gov/pubs/fips/203/final",
          },
          {
            label: "FIPS 204",
            url: "https://csrc.nist.gov/pubs/fips/204/final",
          },
          {
            label: "FIPS 205",
            url: "https://csrc.nist.gov/pubs/fips/205/final",
          },
        ],
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildPqMaspStarkTransferProofV0",
          "generateMlDsaKeyPair",
          "encapsulateMlKem",
        ],
        securityNotes: ["ML-DSA and ML-KEM primitive domains require audit"],
        failureModes: ["ML-KEM domain mismatch"],
        requiredState: ["ML-KEM encrypted note payload store"],
      },
      /failureModes must include post-quantum primitive failure modes/,
    ],
    [
      {
        id: "pq-masp-stark-v0",
        implementationStage: "research-target-as-of-2026-05",
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
        sourceReferences: [
          {
            label: "FIPS 203",
            url: "https://csrc.nist.gov/pubs/fips/203/final",
          },
          {
            label: "FIPS 204",
            url: "https://csrc.nist.gov/pubs/fips/204/final",
          },
          {
            label: "FIPS 205",
            url: "https://csrc.nist.gov/pubs/fips/205/final",
          },
        ],
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildPqMaspStarkTransferProofV0",
          "generateMlDsaKeyPair",
          "encapsulateMlKem",
        ],
        securityNotes: ["ML-DSA and ML-KEM primitive domains require audit"],
        failureModes: ["ML-DSA or ML-KEM domain mismatch"],
        requiredState: ["PQ nullifier set"],
      },
      /requiredState must include post-quantum note-encryption state/,
    ],
    [{ sdkEntrypoints: [" buildProof"] }, /sdkEntrypoints\[0\] must be clean and already trimmed/],
    [
      { plannedSdkEntrypoints: ["buildFuture\t"] },
      /plannedSdkEntrypoints\[0\] must be clean and already trimmed/,
    ],
    [{ sdkEntrypoints: ["buildProof-withSuffix"] }, /must be an SDK entrypoint name/],
    [{ sdkEntrypoints: ["build$Proof"] }, /must be an SDK entrypoint name/],
    [{ sdkEntrypoints: ["_buildProof"] }, /must be an SDK entrypoint name/],
    [{ sdkEntrypoints: ["buildProof_"] }, /must be an SDK entrypoint name/],
    [{ sdkEntrypoints: ["build_Proof"] }, /must be an SDK entrypoint name/],
    [
      { sdkEntrypoints: ["Iroha._Privacy.buildProof"] },
      /must be an SDK entrypoint name/,
    ],
    [
      { sdkEntrypoints: ["Iroha.Privacy_.buildProof"] },
      /must be an SDK entrypoint name/,
    ],
    [{ plannedSdkEntrypoints: ["buildFuture$Proof"] }, /must be an SDK entrypoint name/],
    [{ plannedSdkEntrypoints: ["_buildFutureProof"] }, /must be an SDK entrypoint name/],
    [{ plannedSdkEntrypoints: ["buildFutureProof_"] }, /must be an SDK entrypoint name/],
    [{ plannedSdkEntrypoints: ["buildFuture_Proof"] }, /must be an SDK entrypoint name/],
    [
      { plannedSdkEntrypoints: ["Iroha._Privacy.buildFutureProof"] },
      /must be an SDK entrypoint name/,
    ],
    [
      { plannedSdkEntrypoints: ["Iroha.Privacy_.buildFutureProof"] },
      /must be an SDK entrypoint name/,
    ],
    [
      { plannedSdkEntrypoints: ["buildFutureDev.Proof.Fixture"] },
      /fixture\/mock entrypoint/,
    ],
    [
      { plannedSdkEntrypoints: ["verifyFutureShapeProofLocally"] },
      /local-only verifier entrypoint/,
    ],
    [
      { plannedSdkEntrypoints: ["verifyFutureShapeProofLocal"] },
      /local-only verifier entrypoint/,
    ],
    [
      { plannedSdkEntrypoints: ["Iroha.Privacy.verifyFutureShapeProofLocally"] },
      /local-only verifier entrypoint/,
    ],
    [
      { plannedSdkEntrypoints: ["Iroha.Privacy.verifyFutureShapeProofLocalVerifier"] },
      /local-only verifier entrypoint/,
    ],
    [
      {
        implementationStage: "chain-executable",
        sdkEntrypoints: ["buildShapeDevProofFixture"],
      },
      /chain-executable targets cannot advertise fixture\/mock SDK entrypoints/,
    ],
    [
      {
        implementationStage: "chain-executable",
        sdkEntrypoints: ["buildShapeDev.Proof.Fixture"],
      },
      /chain-executable targets cannot advertise fixture\/mock SDK entrypoints/,
    ],
    [
      {
        implementationStage: "chain-executable",
        sdkEntrypoints: ["verifyShapeProofLocally"],
      },
      /chain-executable targets cannot advertise local-only verifier SDK entrypoints/,
    ],
    [
      {
        implementationStage: "chain-executable",
        sdkEntrypoints: ["verifyShapeProofLocal"],
      },
      /chain-executable targets cannot advertise local-only verifier SDK entrypoints/,
    ],
    [
      {
        implementationStage: "chain-executable",
        sdkEntrypoints: ["Iroha.Privacy.verifyShapeProofLocally"],
      },
      /chain-executable targets cannot advertise local-only verifier SDK entrypoints/,
    ],
    [
      {
        implementationStage: "component",
        sdkEntrypoints: ["buildShapeInstruction"],
        plannedSdkEntrypoints: ["buildShapeProofV1"],
      },
      /component targets cannot advertise instruction SDK entrypoint/,
    ],
    [
      {
        implementationStage: "component",
        sdkEntrypoints: ["Iroha.Privacy.buildShapeInstruction"],
        plannedSdkEntrypoints: ["buildShapeProofV1"],
      },
      /component targets cannot advertise instruction SDK entrypoint/,
    ],
    [
      {
        implementationStage: "component",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildShapeInstruction"],
      },
      /component targets cannot advertise instruction SDK entrypoint/,
    ],
    [
      {
        implementationStage: "component",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["Iroha.Privacy.buildShapeInstruction"],
      },
      /component targets cannot advertise instruction SDK entrypoint/,
    ],
    [
      {
        implementationStage: null,
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildShapeTransferInstruction",
          "buildShapeAuthorizedTransaction",
        ],
        requiredState: ["shape verifier registry"],
        failureModes: ["shape verifier mismatch"],
        chainRequirements: ["shape verifier registry"],
      },
      /ledger-mutating entries require replay, nullifier, revocation, or link-tag protection metadata/,
    ],
    [
      {
        implementationStage: null,
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildShapeTransferInstruction",
          "buildShapeAuthorizedTransaction",
        ],
        requiredState: ["shape replay guard"],
        failureModes: ["shape replay"],
        chainRequirements: ["shape verifier registry"],
        setupSteps: ["Register shape verifier."],
        executionSteps: ["Submit shape proof."],
      },
      /ledger-mutating entries require explicit typed chain admission metadata/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: [
          "zkAt verifier key registry",
          "typed zk::ZkAtPolicyCommitment instruction admission",
        ],
        securityNotes: [
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
          "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
        ],
        requiredState: [
          "policy commitment registry",
          "authorization replay guard",
          "wallet policy witness store",
          "zkAt verifier key registry",
        ],
        failureModes: ["authorization replay"],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: [
          "Submit typed zk::ZkAtPolicyCommitment instruction with tx_digest.",
        ],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyCommitmentInstruction"],
      },
      /ledger-mutating entries require restart\/persistence metadata for root, nullifier, revocation, or replay state/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier"],
        securityNotes: ["Policy proof review required."],
        requiredState: ["policy commitment registry"],
        failureModes: [
          "policy-root substitution",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register policy verifier."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /requiredState must include wallet or witness state metadata for source-referenced privacy flows/,
    ],
    [
      {
        id: "vega-existing-credential-zk-v0",
        category: "credential",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "Vega source",
            url: "https://www.microsoft.com/en-us/research/publication/vega-low-latency-zero-knowledge-proofs-over-existing-credentials/",
          },
        ],
        recommendedFor: ["credential predicate proofs"],
        chainRequirements: ["credential predicate verifier"],
        securityNotes: [
          "Credential proof review required.",
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
        ],
        requiredState: [
          "credential issuer registry",
          "wallet credential witness store",
          "revocation policy",
        ],
        failureModes: [
          "credential replay",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register credential verifier."],
        executionSteps: ["Build credential proof."],
        proofFamily: "existing-credential-zk",
        publicInputsSchema: "issuer_commitment,credential_schema",
        verifierKeyId: "vega_existing_credential_zk_v0",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildVegaCredentialPredicateProofV0"],
      },
      /requiredState must include credential, identity, or admission commitment\/accumulator state metadata/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier"],
        securityNotes: [
          "Policy proof review required.",
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
        ],
        requiredState: ["policy commitment registry", "wallet policy witness store"],
        failureModes: [
          "policy-root substitution",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /must include verifier-key record metadata for source-referenced verifier entries/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier key registry"],
        securityNotes: [
          "Policy proof review required.",
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
        ],
        requiredState: [
          "policy commitment registry",
          "wallet policy witness store",
          "zkAt verifier key registry",
        ],
        failureModes: [
          "policy-root substitution",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,policy_hash",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /must include chain\/domain binding metadata for source-referenced verifier entries/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier key registry"],
        securityNotes: [
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
          "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
        ],
        requiredState: [
          "policy commitment registry",
          "authorization replay guard",
          "wallet policy witness store",
          "zkAt verifier key registry",
        ],
        failureModes: ["authorization replay"],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /failureModes must include malformed-proof, wrong-verifier-key, and wrong-public-input rejection for source-referenced verifier entries/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier key registry"],
        securityNotes: [
          "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
        ],
        requiredState: ["policy commitment registry", "wallet policy witness store"],
        failureModes: [
          "policy-root substitution",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /securityNotes must include wallet\/witness privacy notes for source-referenced privacy flows/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier key registry"],
        securityNotes: [
          "Policy proof review required.",
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
        ],
        requiredState: ["policy commitment registry", "wallet policy witness store"],
        failureModes: [
          "policy-root substitution",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /securityNotes must include audit\/review, fuzzing, and performance hardening gates for source-referenced entries/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture"],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /research targets cannot advertise fixture\/mock SDK entrypoints/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDev.Proof.Fixture"],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /research targets cannot advertise fixture\/mock SDK entrypoints/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        sdkEntrypoints: ["verifyShapeProofLocally"],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /research targets cannot advertise local-only verifier SDK entrypoints/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        sdkEntrypoints: ["verifyShapeProofLocalVerifier"],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /research targets cannot advertise local-only verifier SDK entrypoints/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        sdkEntrypoints: ["Iroha.Privacy.verifyShapeProofLocally"],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /research targets cannot advertise local-only verifier SDK entrypoints/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        sdkEntrypoints: ["verifyShapeProof"],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /research targets cannot advertise executable SDK entrypoints/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        sdkEntrypoints: ["buildShapeProductionProof"],
        plannedSdkEntrypoints: ["buildShapeProductionProofV1"],
      },
      /research targets cannot advertise executable SDK entrypoints/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        sdkEntrypoints: ["buildShapeProofEnvelope"],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /research targets cannot advertise executable SDK entrypoints/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        sdkEntrypoints: ["buildShapeProductionInstruction"],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /research targets cannot advertise executable SDK entrypoints/,
    ],
    [
      {
        implementationStage: "production-hardened",
        sdkEntrypoints: ["buildShapeDevProofFixture"],
      },
      /production-hardened targets cannot advertise fixture\/mock SDK entrypoints/,
    ],
    [
      {
        implementationStage: "production-hardened",
        sdkEntrypoints: ["buildFutureDev.Proof.Fixture"],
      },
      /production-hardened targets cannot advertise fixture\/mock SDK entrypoints/,
    ],
    [
      {
        implementationStage: "production-hardened",
        sdkEntrypoints: ["buildFutureMockProofV2"],
      },
      /production-hardened targets cannot advertise fixture\/mock SDK entrypoints/,
    ],
    [
      {
        implementationStage: "production-hardened",
        sdkEntrypoints: ["verifyShapeProofLocally"],
      },
      /production-hardened targets cannot advertise local-only verifier SDK entrypoints/,
    ],
    [
      {
        implementationStage: "production-hardened",
        sdkEntrypoints: ["verifyShapeProofLocalVerifier"],
      },
      /production-hardened targets cannot advertise local-only verifier SDK entrypoints/,
    ],
    [
      {
        implementationStage: "production-hardened",
        sdkEntrypoints: ["Iroha.Privacy.verifyShapeProofLocally"],
      },
      /production-hardened targets cannot advertise local-only verifier SDK entrypoints/,
    ],
    [
      {
        implementationStage: "sdk-builder",
        sdkEntrypoints: ["verifyShapeProofLocally"],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /executable local-only verifier SDK entrypoints must be paired with an explicit DevFixture entrypoint/,
    ],
    [
      {
        implementationStage: "sdk-builder",
        sdkEntrypoints: ["verifyShapeProofLocalVerifier"],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /executable local-only verifier SDK entrypoints must be paired with an explicit DevFixture entrypoint/,
    ],
    [
      {
        implementationStage: "sdk-builder",
        sdkEntrypoints: ["Iroha.Privacy.verifyShapeProofLocally"],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /executable local-only verifier SDK entrypoints must be paired with an explicit DevFixture entrypoint/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildProofFixture"],
      },
      /fixture\/mock SDK entrypoints must use explicit DevFixture names/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildMockProof"],
      },
      /fixture\/mock SDK entrypoints must use explicit DevFixture names/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildMockProofV2"],
      },
      /fixture\/mock SDK entrypoints must use explicit DevFixture names/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildProof.Fixture"],
      },
      /fixture\/mock SDK entrypoints must use explicit DevFixture names/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture"],
      },
      /executable DevFixture SDK entrypoints must be paired with a local verifier entrypoint/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevFixture"],
      },
      /executable DevFixture SDK entrypoints must be paired with a local verifier entrypoint/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [],
      },
      /executable DevFixture SDK entrypoints must include a security note that marks dev fixtures as non-production/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: ["The SDK dev fixture is deterministic only."],
      },
      /executable DevFixture SDK entrypoints must include a security note that marks dev fixtures as non-production/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: ["Production Shape proofs remain unavailable."],
      },
      /executable DevFixture SDK entrypoints must include a security note that marks dev fixtures as non-production/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: [],
      },
      /executable DevFixture SDK entrypoints must retain planned production SDK entrypoints until production gates pass/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: [
          "buildShapeProductionInstruction",
          "buildShapeProofInstruction",
        ],
      },
      /executable DevFixture SDK entrypoints must retain a planned production proof builder until production gates pass/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: ["buildShapeProofTransaction"],
      },
      /executable DevFixture SDK entrypoints must retain a planned production proof builder until production gates pass/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: ["buildSubmitShapeProof"],
      },
      /executable DevFixture SDK entrypoints must retain a planned production proof builder until production gates pass/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: ["buildShapeProofEnvelope"],
      },
      /executable DevFixture SDK entrypoints must retain a planned production proof builder until production gates pass/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: ["buildShapeProofWitness"],
      },
      /executable DevFixture SDK entrypoints must retain a planned production proof builder until production gates pass/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: ["buildShapeProofPublicInputs"],
      },
      /executable DevFixture SDK entrypoints must retain a planned production proof builder until production gates pass/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: ["buildShapeProofRequest"],
      },
      /executable DevFixture SDK entrypoints must retain a planned production proof builder until production gates pass/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: ["buildShapeProofCommitment"],
      },
      /executable DevFixture SDK entrypoints must retain a planned production proof builder until production gates pass/,
    ],
    [
      {
        sdkEntrypoints: ["buildProof"],
        plannedSdkEntrypoints: ["buildProof"],
      },
      /is already executable/,
    ],
    [
      {
        implementationStage: "catalog-as-of-2026-05",
        sdkEntrypoints: ["buildForgedProductionProof"],
      },
      /catalog-only targets cannot advertise SDK entrypoints/,
    ],
    [
      {
        implementationStage: "production-hardened",
        plannedSdkEntrypoints: ["buildFutureProductionProof"],
      },
      /production-hardened targets cannot retain planned SDK entrypoints/,
    ],
  ]) {
    assertJsValidatorsReject(patch, pattern);
  }
});

test("planned privacy SDK entrypoints remain unexported until production gates pass", () => {
  const descriptors = getSrcPrivacyAlgorithmDescriptors();
  const plannedEntryPoints = new Set(
    descriptors.flatMap((descriptor) => descriptor.plannedSdkEntrypoints),
  );
  const executableEntryPoints = new Set(
    descriptors.flatMap((descriptor) => descriptor.sdkEntrypoints),
  );
  const sourceCapabilityKeys = new Set(Object.keys(getSrcPrivacyCapabilities()));
  const distCapabilityKeys = new Set(Object.keys(getDistPrivacyCapabilities()));
  const publicApiDeclarationTexts = PUBLIC_PRIVACY_API_DECLARATION_SURFACES.map(
    ({ label, path }) => [label, fileText(path)],
  );
  const publicApiSourceTexts = publicPrivacyApiSourceTexts();
  const moduleExportSurfaces = [
    ["JS src package", jsSrcPackage],
    ["JS src crypto", jsSrcCrypto],
    ["JS src browser crypto", jsSrcBrowserCrypto],
    ["JS src instruction builders", jsSrcInstructionBuilders],
    ["JS dist package", jsDistPackage],
    ["JS dist crypto", jsDistCrypto],
    ["JS dist browser crypto", jsDistBrowserCrypto],
    ["JS dist instruction builders", jsDistInstructionBuilders],
  ];

  assertExecutableEntrypointsExported("JS src package", descriptors, jsSrcPackage);
  assertExecutableEntrypointsExported(
    "JS dist package",
    getDistPrivacyAlgorithmDescriptors(),
    jsDistPackage,
  );
  assertExecutableEntrypointsDeclared(
    "JS TypeScript declarations",
    descriptors,
    fileText(JS_DECLARATIONS),
  );

  assert.ok(
    plannedEntryPoints.size > 0,
    "privacy catalog must include planned production entrypoints",
  );
  for (const entrypoint of plannedEntryPoints) {
    assert.equal(
      executableEntryPoints.has(entrypoint),
      false,
      `${entrypoint} must not be both planned and executable`,
    );
    for (const [label, moduleExports] of moduleExportSurfaces) {
      for (const name of publicApiNameVariants(entrypoint)) {
        assert.equal(
          Object.hasOwn(moduleExports, name),
          false,
          `${entrypoint} must not be exported as ${name} from ${label} until production gates pass`,
        );
      }
    }
    for (const [label, text] of publicApiDeclarationTexts) {
      for (const name of publicApiNameVariants(entrypoint)) {
        assert.equal(
          new RegExp(`\\b${escapeRegExp(name)}\\b`).test(text),
          false,
          `${entrypoint} must not be declared as ${name} in ${label} until production gates pass`,
        );
      }
    }
    for (const source of publicApiSourceTexts) {
      for (const name of publicApiNameVariants(entrypoint)) {
        for (const pattern of publicDeclarationPatterns(source.language, name)) {
          assert.equal(
            pattern.test(source.text),
            false,
            `${entrypoint} must not be publicly declared as ${name} in ${source.label} ${source.path} until production gates pass`,
          );
        }
      }
    }
    for (const name of publicApiNameVariants(entrypoint)) {
      const capabilityKey = snakeEntrypointName(name);
      assert.equal(
        sourceCapabilityKeys.has(capabilityKey),
        false,
        `${entrypoint} must not have a JS src capability key ${capabilityKey} until production gates pass`,
      );
      assert.equal(
        distCapabilityKeys.has(capabilityKey),
        false,
        `${entrypoint} must not have a JS dist capability key ${capabilityKey} until production gates pass`,
      );
    }
  }

  for (const descriptor of descriptors) {
    if (descriptor.plannedSdkEntrypoints.length > 0) {
      assert.equal(descriptor.productionReady, false);
      assert.equal(descriptor.productionGate.ready, false);
      assert.ok(
        descriptor.productionGate.missing.includes("planned SDK entrypoints remain"),
        `${descriptor.id} must explain that planned SDK entrypoints still block production`,
      );
    }
  }

  const anonymousPgc = descriptors.find(
    (descriptor) => descriptor.id === "anonymous-pgc-k-out-of-n-v1",
  );
  assert.ok(anonymousPgc, "Anonymous PGC catalog row must exist");
  assert.deepEqual(
    anonymousPgc.chainRequirements.filter((requirement) =>
      requirement.includes("zk::") && requirement.includes("instruction"),
    ),
    [
      "typed zk::RegisterAnonymousPgcAccountCommitment instruction",
      "typed zk::SubmitAnonymousPgcTransfer instruction",
    ],
    "Anonymous PGC planned ledger mutations must retain explicit typed zk:: instruction metadata",
  );
  const zkat = descriptors.find(
    (descriptor) => descriptor.id === "zkat-policy-private-auth-v1",
  );
  assert.ok(zkat, "ZK-AT catalog row must exist");
  assert.deepEqual(
    zkat.chainRequirements.filter((requirement) => requirement.includes("zk::")),
    [
      "typed zk::RegisterZkAtPolicyCommitment instruction",
      "typed zk::SubmitZkAtAuthorizedTransaction admission",
    ],
    "ZK-AT planned ledger mutations must retain explicit typed zk:: admission metadata",
  );
});
