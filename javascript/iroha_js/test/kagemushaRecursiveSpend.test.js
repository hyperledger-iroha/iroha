import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
  KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1,
  KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION,
  KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
  KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1,
  KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1,
  KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES,
  KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES,
  KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES,
  KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN,
  KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN,
  KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1,
  canAppendKagemushaRecursiveSpendWitnesslessLineage,
  canProveKagemushaRecursiveSpendAppendOutputProofCircuitId,
  canRedeemKagemushaRecursiveSpendWitnessless,
  canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId,
  isKagemushaRecursiveSpendLineageProofCircuitId,
  isKagemushaRecursiveSpendLineageAppendOutputCircuitId,
  isKagemushaRecursiveSpendNativeAvailable,
  isSupportedKagemushaRecursiveSpendAppendOutputProofCircuitId,
  isSupportedKagemushaRecursiveSpendAppendProofTransition,
  isSupportedKagemushaRecursiveSpendPreviousProofCircuitId,
  kagemushaRecursiveSpendAppend,
  kagemushaRecursiveSpendInit,
  kagemushaRecursiveSpendLineageAppendBoundary,
  kagemushaRecursiveSpendLineageWitnessAppendResult,
  kagemushaRecursiveSpendLineageWitnessFromInitResult,
  kagemushaRecursiveSpendRedeem,
  kagemushaRecursiveSpendTransitionProfileAppend,
  kagemushaRecursiveSpendTransitionProfileInit,
  kagemushaRecursiveSpendVerify,
  normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId,
  preferredKagemushaOfflineSpendMode,
  preferredKagemushaRecursiveSpendAppendOutputProofCircuitId,
  requiresKagemushaRecursiveSpendLineageWitnessForRedeem,
  requiresKagemushaRecursiveSpendPreviousLineageVerifierRecordForAppend,
  requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend,
} from "../src/crypto.js";

function withNativeBinding(binding, fn) {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = binding;
  try {
    return fn();
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
}

function isProbeArchive(value) {
  const buffer = Buffer.from(value);
  return buffer.length === 1 && buffer[0] === 0;
}

function rejectMalformedProbe(method, ...archives) {
  if (archives.length > 0 && archives.every(isProbeArchive)) {
    throw new Error(`Kagemusha malformed probe rejected by ${method}`);
  }
}

function completeRecursiveSpendBinding(overrides = {}) {
  return {
    connectNoritoBridgeAbiVersion() {
      return 6;
    },
    kagemushaRecursiveSpendInit(request) {
      rejectMalformedProbe("init", request);
      return Uint8Array.from([1]);
    },
    kagemushaRecursiveSpendAppend(request) {
      rejectMalformedProbe("append", request);
      return Uint8Array.from([2]);
    },
    kagemushaRecursiveSpendTransitionProfileInit(request) {
      rejectMalformedProbe("transition-profile-init", request);
      return Uint8Array.from([7]);
    },
    kagemushaRecursiveSpendTransitionProfileAppend(request) {
      rejectMalformedProbe("transition-profile-append", request);
      return Uint8Array.from([8]);
    },
    kagemushaRecursiveSpendLineageAppendBoundary(profile) {
      rejectMalformedProbe("lineage-append-boundary", profile);
      return Uint8Array.from([9]);
    },
    kagemushaRecursiveSpendLineageWitnessFromInitResult(request, bundle) {
      rejectMalformedProbe("lineage-init", request, bundle);
      return Uint8Array.from([3]);
    },
    kagemushaRecursiveSpendLineageWitnessAppendResult(previousWitness, request, bundle) {
      rejectMalformedProbe("lineage-append", previousWitness, request, bundle);
      return Uint8Array.from([4]);
    },
    kagemushaRecursiveSpendVerify(request) {
      rejectMalformedProbe("verify", request);
      return Uint8Array.from([5]);
    },
    kagemushaRecursiveSpendRedeem(request) {
      rejectMalformedProbe("redeem", request);
      return Uint8Array.from([6]);
    },
    ...overrides,
  };
}

function sharedRecursiveSpendManifest() {
  return JSON.parse(
    readFileSync(
      new URL("../../../fixtures/kagemusha_recursive_spend_abi6/manifest.json", import.meta.url),
      "utf8",
    ),
  );
}

function sharedRecursiveSpendArchives() {
  return JSON.parse(
    readFileSync(
      new URL("../../../fixtures/kagemusha_recursive_spend_abi6/archives.json", import.meta.url),
      "utf8",
    ),
  );
}

test("Kagemusha recursive spend helpers reject empty request archives before native calls", () => {
  withNativeBinding({}, () => {
    assert.throws(
      () => kagemushaRecursiveSpendInit(Buffer.alloc(0)),
      /requestArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendAppend(Buffer.alloc(0)),
      /requestArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendTransitionProfileInit(Buffer.alloc(0)),
      /requestArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendTransitionProfileAppend(Buffer.alloc(0)),
      /requestArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageAppendBoundary(Buffer.alloc(0)),
      /profileArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageWitnessFromInitResult(Buffer.alloc(0), Buffer.from([1])),
      /requestArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageWitnessFromInitResult(Buffer.from([1]), Buffer.alloc(0)),
      /bundleArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageWitnessAppendResult(Buffer.alloc(0), Buffer.from([1]), Buffer.from([2])),
      /previousWitnessArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageWitnessAppendResult(Buffer.from([1]), Buffer.alloc(0), Buffer.from([2])),
      /requestArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageWitnessAppendResult(Buffer.from([1]), Buffer.from([2]), Buffer.alloc(0)),
      /bundleArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendVerify(Buffer.alloc(0)),
      /requestArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendRedeem(Buffer.alloc(0)),
      /requestArchive must not be empty/,
    );
  });
});

test("Kagemusha recursive spend shared ABI-6 fixture matches SDK surface", () => {
  const manifest = sharedRecursiveSpendManifest();
  assert.equal(
    manifest.schema,
    "iroha.kagemusha.recursive_spend.abi6.fixture_manifest.v1",
  );
  assert.equal(
    manifest.bridge_abi_version,
    KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION,
  );
  assert.equal(manifest.operation_count, 9);
  assert.equal(manifest.operations.length, manifest.operation_count);
  assert.deepEqual(
    new Set(manifest.operations.map((operation) => operation.symbol)),
    new Set([
      "connect_norito_kagemusha_recursive_spend_init",
      "connect_norito_kagemusha_recursive_spend_append",
      "connect_norito_kagemusha_recursive_spend_transition_profile_init",
      "connect_norito_kagemusha_recursive_spend_transition_profile_append",
      "connect_norito_kagemusha_recursive_spend_lineage_append_boundary",
      "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result",
      "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result",
      "connect_norito_kagemusha_recursive_spend_verify",
      "connect_norito_kagemusha_recursive_spend_redeem",
    ]),
  );
  const appendWitness = manifest.operations.find(
    (operation) => operation.name === "lineage_witness_append_result",
  );
  assert.equal(appendWitness.input_archives.length, 3);
  assert.equal(appendWitness.output_archive, "KagemushaRecursiveSpendLineageWitnessV1");
  assert.equal(
    manifest.proof_circuit_ids.recursive_aggregation,
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(
    manifest.proof_circuit_ids.reserved_lineage,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(
    manifest.proof_circuit_ids.reserved_lineage_one_hop,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(
    manifest.proof_circuit_ids.reserved_lineage_append,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(manifest.limits.compact_token_max_hops, KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS);
  assert.equal(
    manifest.limits.reserved_lineage_witnessless_max_hops,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1,
  );
  assert.equal(
    manifest.limits.previous_proof_open_envelopes_required_count,
    KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1,
  );
  assert.equal(
    manifest.limits.previous_proof_open_envelopes_max_bytes,
    KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES,
  );
  assert.equal(
    manifest.limits.pallas_open_envelope_max_transcript_label_bytes,
    KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES,
  );
  assert.equal(manifest.limits.native_archive_max_bytes, KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES);
  assert.equal(
    manifest.domains.transition_profile,
    KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN,
  );
  assert.equal(
    manifest.domains.lineage_append_boundary_final_note_binding,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1,
  );
  assert.equal(manifest.payload_benchmarks.semantic_payload_bytes, 1751);
  assert.equal(manifest.payload_benchmarks.reserved_lineage_payload_bytes, 3847);
  assert.equal(manifest.payload_benchmarks.reserved_lineage_transition_profile_bytes, 2817);
  const archiveFixture = sharedRecursiveSpendArchives();
  assert.equal(
    archiveFixture.schema,
    "iroha.kagemusha.recursive_spend.abi6.archive_fixtures.v1",
  );
  assert.deepEqual(
    new Set(archiveFixture.archives.map((archive) => archive.name)),
    new Set([
      "init_request",
      "init_bundle",
      "transition_profile_init",
      "append_request",
      "append_bundle",
      "transition_profile_append",
      "lineage_append_boundary",
      "lineage_witness_from_init_result",
      "lineage_witness_append_result",
      "verify_request",
      "verify_result",
      "redeem_request",
      "redeem_instruction",
    ]),
  );
  const requestFieldsByType = new Map(
    archiveFixture.request_archive_fields.map((entry) => [
      entry.norito_type,
      entry.fields,
    ]),
  );
  const expectedRequestFields = new Map([
    [
      "KagemushaRecursiveSpendInitRequestV1",
      [
        "record_bundle",
        "pallas_open_envelopes_archive",
        "current_note",
        "lineage_verifier_key",
        "lineage_proving_key_archive",
        "block_height",
      ],
    ],
    [
      "KagemushaRecursiveSpendAppendRequestV1",
      [
        "previous_bundle",
        "record_bundle",
        "pallas_open_envelopes_archive",
        "current_note",
        "output_proof_circuit_id",
        "previous_lineage_verifier_record",
        "previous_recursive_proof_open_envelopes_archive",
        "lineage_verifier_key",
        "lineage_proving_key_archive",
        "block_height",
      ],
    ],
    [
      "KagemushaRecursiveSpendVerifyRequestV1",
      ["bundle", "lineage_verifier_record", "block_height"],
    ],
    [
      "KagemushaRecursiveSpendRedeemRequestV1",
      [
        "bundle",
        "recipient",
        "public_amount",
        "redeem_proof",
        "lineage_witness",
        "lineage_verifier_record",
        "block_height",
      ],
    ],
  ]);
  assert.deepEqual(
    new Set(requestFieldsByType.keys()),
    new Set(expectedRequestFields.keys()),
  );
  for (const [requestType, expectedFields] of expectedRequestFields.entries()) {
    const fields = requestFieldsByType.get(requestType);
    assert.deepEqual(
      fields.map((field) => field.name),
      expectedFields,
    );
    const blockHeight = fields.find((field) => field.name === "block_height");
    assert.equal(blockHeight.type, "Option<u64>");
    assert.equal(blockHeight.norito_default, true);
    assert.equal(blockHeight.semantics, "verifier_record_activation_height");
  }

  const redeemArchive = archiveFixture.archives.find(
    (archive) => archive.name === "redeem_request",
  );
  assert.equal(redeemArchive.operation, "redeem");
  assert.equal(redeemArchive.norito_type, "KagemushaRecursiveSpendRedeemRequestV1");
  assert.equal(
    redeemArchive.sha256_hex,
    "b83b33541f50ab893ae356c1f42da60aaf81da95bc4daf871511509fc8eea5b2",
  );
  assert.ok(redeemArchive.byte_len > 0);
  assert.ok(Buffer.from(redeemArchive.bytes_base64, "base64").length > 0);
  const redeemInstructionArchive = archiveFixture.archives.find(
    (archive) => archive.name === "redeem_instruction",
  );
  assert.equal(redeemInstructionArchive.norito_type, "RedeemKagemushaRecursive");
  assert.equal(
    redeemInstructionArchive.sha256_hex,
    "a598660cbfe91a207b64a69b7a9dbdc985fd901c60fe886aecb4dead4115169e",
  );
  assert.equal(
    preferredKagemushaRecursiveSpendAppendOutputProofCircuitId(1),
    manifest.proof_circuit_ids.reserved_lineage_append,
  );
  assert.equal(
    preferredKagemushaRecursiveSpendAppendOutputProofCircuitId(63),
    manifest.proof_circuit_ids.reserved_lineage_append,
  );
  assert.equal(
    preferredKagemushaRecursiveSpendAppendOutputProofCircuitId(64),
    manifest.proof_circuit_ids.recursive_aggregation,
  );
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(0), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(63), true);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(64), false);
  assert.equal(
    canRedeemKagemushaRecursiveSpendWitnessless(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
      2,
    ),
    true,
  );
  assert.equal(
    canRedeemKagemushaRecursiveSpendWitnessless(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      65,
    ),
    false,
  );
});

test("Kagemusha offline spend mode defaults to recursive when native support is complete", () => {
  const completeBinding = completeRecursiveSpendBinding();

  assert.equal(
    preferredKagemushaOfflineSpendMode(true),
    KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1,
  );
  assert.equal(
    preferredKagemushaOfflineSpendMode(false),
    KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
  );
  withNativeBinding(completeBinding, () => {
    assert.equal(
      preferredKagemushaOfflineSpendMode(),
      KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1,
    );
  });
  withNativeBinding({ kagemushaRecursiveSpendInit() {} }, () => {
    assert.equal(
      preferredKagemushaOfflineSpendMode(),
      KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
    );
  });
});

test("Kagemusha recursive spend exports stable proof circuit ids", () => {
  assert.equal(KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION, 6);
  assert.equal(
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    "kagemusha-recursive-aggregation-v1",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    "kagemusha-recursive-spend-lineage-v1",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    "kagemusha-recursive-spend-lineage-onehop-v1",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    "kagemusha-recursive-spend-lineage-append-v1",
  );
  assert.equal(KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS, 64);
  assert.equal(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1, 64);
  assert.equal(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1, true);
  assert.equal(KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1, 1);
  assert.equal(KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES, 8 * 1024 * 1024);
  assert.equal(KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES, 128);
  assert.equal(KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES, 64 * 1024 * 1024);
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN,
    "iroha:kagemusha:v1:recursive-spend-transition-profile",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN,
    "iroha:kagemusha:v1:recursive-spend-transition-profile-digest",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN,
    "iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1,
    "iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1,
    "iroha:kagemusha:recursive-spend-lineage-append-boundary:v1",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1,
    "iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1,
    "iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1",
  );
  assert.equal(
    normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(),
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(
    normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(null),
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(
    normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(""),
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(
    normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    ),
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(
    normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(
      "unknown-kagemusha-recursive-spend-circuit",
    ),
    "unknown-kagemusha-recursive-spend-circuit",
  );
  for (const circuitId of [
    undefined,
    null,
    "",
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
  ]) {
    assert.equal(
      isSupportedKagemushaRecursiveSpendAppendOutputProofCircuitId(circuitId),
      true,
    );
  }
  assert.equal(
    isSupportedKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    ),
    false,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendAppendOutputProofCircuitId(
      "unknown-kagemusha-recursive-spend-circuit",
    ),
    false,
  );
  for (const circuitId of [
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
  ]) {
    assert.equal(isKagemushaRecursiveSpendLineageProofCircuitId(circuitId), true);
  }
  assert.equal(
    isKagemushaRecursiveSpendLineageAppendOutputCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    ),
    false,
  );
  assert.equal(
    isKagemushaRecursiveSpendLineageAppendOutputCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendPreviousProofCircuitId(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendPreviousProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendPreviousProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendPreviousProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendPreviousProofCircuitId(
      "unknown-kagemusha-recursive-spend-circuit",
    ),
    false,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousLineageVerifierRecordForAppend(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    ),
    false,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousLineageVerifierRecordForAppend(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousLineageVerifierRecordForAppend(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousLineageVerifierRecordForAppend(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousLineageVerifierRecordForAppend(
      "unknown-kagemusha-recursive-spend-circuit",
    ),
    false,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendAppendProofTransition(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendAppendProofTransition(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      "",
    ),
    true,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendAppendProofTransition(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendAppendProofTransition(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendAppendProofTransition(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    ),
    false,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendAppendProofTransition(
      "unknown-kagemusha-recursive-spend-circuit",
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    ),
    false,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendAppendProofTransition(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      "unknown-kagemusha-recursive-spend-circuit",
    ),
    false,
  );
  assert.equal(
    canRedeemKagemushaRecursiveSpendWitnessless(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    true,
  );
  assert.equal(
    canRedeemKagemushaRecursiveSpendWitnessless(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    true,
  );
  assert.equal(
    canRedeemKagemushaRecursiveSpendWitnessless(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
      2,
    ),
    true,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendLineageWitnessForRedeem(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    false,
  );
  assert.equal(
    canRedeemKagemushaRecursiveSpendWitnessless(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      64,
    ),
    true,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendLineageWitnessForRedeem(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      64,
    ),
    false,
  );
  for (const [circuitId, hopCount] of [
    [KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 1],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 0],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 65],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 1.5],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, -1],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, Number.MAX_SAFE_INTEGER],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, Number.NaN],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, Number.POSITIVE_INFINITY],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 1n],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, new Number(1)],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, true],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, "1"],
    [undefined, 1],
    [null, 1],
    ["", 1],
    ["unknown-kagemusha-recursive-spend-circuit", 1],
  ]) {
    assert.equal(canRedeemKagemushaRecursiveSpendWitnessless(circuitId, hopCount), false);
    assert.equal(
      requiresKagemushaRecursiveSpendLineageWitnessForRedeem(circuitId, hopCount),
      true,
    );
  }
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(0), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(1), true);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(63), true);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(64), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(1.5), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(-1), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(Number.MAX_SAFE_INTEGER), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(Number.NaN), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(Number.POSITIVE_INFINITY), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(1n), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(new Number(1)), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(true), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage("1"), false);
  assert.equal(
    preferredKagemushaRecursiveSpendAppendOutputProofCircuitId(1),
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(
    preferredKagemushaRecursiveSpendAppendOutputProofCircuitId(63),
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(
    preferredKagemushaRecursiveSpendAppendOutputProofCircuitId(64),
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    "preferred append selector falls back at the witnessless hop cap",
  );
  assert.equal(
    preferredKagemushaRecursiveSpendAppendOutputProofCircuitId(0),
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    true,
  );
  assert.equal(canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(null, 1), true);
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS - 1,
    ),
    true,
  );
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    true,
  );
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    true,
  );
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    false,
  );
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      63,
    ),
    true,
  );
  for (const [circuitId, previousHopCount] of [
    [KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 0],
    [KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 64],
    ["unknown-kagemusha-recursive-spend-circuit", 1],
    [KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 1.5],
    [KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, Number.NaN],
    [KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, Number.POSITIVE_INFINITY],
    [KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 1n],
    [KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, new Number(1)],
    [KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, "1"],
  ]) {
    assert.equal(
      canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
        circuitId,
        previousHopCount,
      ),
      false,
    );
  }
  assert.equal(
    canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    true,
  );
  assert.equal(
    canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    true,
  );
  assert.equal(
    canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    true,
  );
  assert.equal(
    canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    true,
  );
  assert.equal(
    canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    false,
    "semantic previous proofs cannot select Reserved-lineage output",
  );
  for (const [previousCircuitId, outputCircuitId, previousHopCount] of [
    [
      "unknown-kagemusha-recursive-spend-circuit",
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      1,
    ],
    [
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      1,
    ],
    [
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      "unknown-kagemusha-recursive-spend-circuit",
      1,
    ],
    [
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      0,
    ],
    [
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      Number.NaN,
    ],
    [
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      1n,
    ],
    [
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      new Number(1),
    ],
  ]) {
    assert.equal(
      canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
        previousCircuitId,
        outputCircuitId,
        previousHopCount,
      ),
      false,
    );
  }
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    true,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    true,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      64,
    ),
    true,
  );
  for (const [circuitId, previousHopCount] of [
    ["", 1],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 0],
    [KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 1],
    ["unknown-kagemusha-recursive-spend-circuit", 1],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 1.5],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, Number.NaN],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, Number.POSITIVE_INFINITY],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 1n],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, new Number(1)],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, "1"],
  ]) {
    assert.equal(
      requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend(
        circuitId,
        previousHopCount,
      ),
      false,
    );
  }
});

test("Kagemusha recursive spend helpers probe native availability and return Buffers", () => {
  const calls = [];
  const binding = {
    connectNoritoBridgeAbiVersion() {
      return 6;
    },
    kagemushaRecursiveSpendInit(request) {
      rejectMalformedProbe("init", request);
      calls.push(["init", Buffer.from(request)]);
      return Uint8Array.from([1, 2, 3]);
    },
    kagemushaRecursiveSpendAppend(request) {
      rejectMalformedProbe("append", request);
      calls.push(["append", Buffer.from(request)]);
      return Uint8Array.from([4, 5]);
    },
    kagemushaRecursiveSpendTransitionProfileInit(request) {
      rejectMalformedProbe("transition-profile-init", request);
      calls.push(["transition-profile-init", Buffer.from(request)]);
      return Uint8Array.from([14, 15]);
    },
    kagemushaRecursiveSpendTransitionProfileAppend(request) {
      rejectMalformedProbe("transition-profile-append", request);
      calls.push(["transition-profile-append", Buffer.from(request)]);
      return Uint8Array.from([16, 17]);
    },
    kagemushaRecursiveSpendLineageAppendBoundary(profile) {
      rejectMalformedProbe("lineage-append-boundary", profile);
      calls.push(["lineage-append-boundary", Buffer.from(profile)]);
      return Uint8Array.from([18, 19]);
    },
    kagemushaRecursiveSpendLineageWitnessFromInitResult(request, bundle) {
      rejectMalformedProbe("lineage-init", request, bundle);
      calls.push(["lineage-init", Buffer.from(request), Buffer.from(bundle)]);
      return Uint8Array.from([10, 11]);
    },
    kagemushaRecursiveSpendLineageWitnessAppendResult(previousWitness, request, bundle) {
      rejectMalformedProbe("lineage-append", previousWitness, request, bundle);
      calls.push([
        "lineage-append",
        Buffer.from(previousWitness),
        Buffer.from(request),
        Buffer.from(bundle),
      ]);
      return Uint8Array.from([12, 13]);
    },
    kagemushaRecursiveSpendVerify(request) {
      rejectMalformedProbe("verify", request);
      calls.push(["verify", Buffer.from(request)]);
      return Uint8Array.from([6]);
    },
    kagemushaRecursiveSpendRedeem(request) {
      rejectMalformedProbe("redeem", request);
      calls.push(["redeem", Buffer.from(request)]);
      return Uint8Array.from([7, 8, 9]);
    },
  };

  withNativeBinding(binding, () => {
    assert.equal(isKagemushaRecursiveSpendNativeAvailable(), true);
    assert.deepEqual(kagemushaRecursiveSpendInit(Buffer.from([9])), Buffer.from([1, 2, 3]));
    assert.deepEqual(kagemushaRecursiveSpendAppend(Buffer.from([8])), Buffer.from([4, 5]));
    assert.deepEqual(
      kagemushaRecursiveSpendTransitionProfileInit(Buffer.from([2])),
      Buffer.from([14, 15]),
    );
    assert.deepEqual(
      kagemushaRecursiveSpendTransitionProfileAppend(Buffer.from([1])),
      Buffer.from([16, 17]),
    );
    assert.deepEqual(
      kagemushaRecursiveSpendLineageAppendBoundary(Buffer.from([0x22])),
      Buffer.from([18, 19]),
    );
    assert.deepEqual(
      kagemushaRecursiveSpendLineageWitnessFromInitResult(Buffer.from([3]), Buffer.from([4])),
      Buffer.from([10, 11]),
    );
    assert.deepEqual(
      kagemushaRecursiveSpendLineageWitnessAppendResult(
        Buffer.from([5]),
        Buffer.from([6]),
        Buffer.from([7]),
      ),
      Buffer.from([12, 13]),
    );
    assert.deepEqual(kagemushaRecursiveSpendVerify(Buffer.from([7])), Buffer.from([6]));
    assert.deepEqual(kagemushaRecursiveSpendRedeem(Buffer.from([6])), Buffer.from([7, 8, 9]));
  });

  assert.deepEqual(calls, [
    ["init", Buffer.from([9])],
    ["append", Buffer.from([8])],
    ["transition-profile-init", Buffer.from([2])],
    ["transition-profile-append", Buffer.from([1])],
    ["lineage-append-boundary", Buffer.from([0x22])],
    ["lineage-init", Buffer.from([3]), Buffer.from([4])],
    ["lineage-append", Buffer.from([5]), Buffer.from([6]), Buffer.from([7])],
    ["verify", Buffer.from([7])],
    ["redeem", Buffer.from([6])],
  ]);
});

test("Kagemusha recursive spend availability requires bridge ABI 6", () => {
  const binding = {
    connectNoritoBridgeAbiVersion() {
      return 5;
    },
    kagemushaRecursiveSpendInit() {
      throw new Error("Kagemusha probe rejected");
    },
    kagemushaRecursiveSpendAppend() {
      throw new Error("Kagemusha probe rejected");
    },
    kagemushaRecursiveSpendLineageWitnessFromInitResult() {
      throw new Error("Kagemusha probe rejected");
    },
    kagemushaRecursiveSpendLineageWitnessAppendResult() {
      throw new Error("Kagemusha probe rejected");
    },
    kagemushaRecursiveSpendVerify() {
      throw new Error("Kagemusha probe rejected");
    },
    kagemushaRecursiveSpendRedeem() {
      throw new Error("Kagemusha probe rejected");
    },
  };

  withNativeBinding(binding, () => {
    assert.equal(isKagemushaRecursiveSpendNativeAvailable(), false);
    assert.equal(
      preferredKagemushaOfflineSpendMode(),
      KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
    );
    assert.throws(
      () => kagemushaRecursiveSpendInit(Buffer.from([1])),
      /Kagemusha recursive spend helper 'kagemushaRecursiveSpendInit' is unavailable/,
    );
  });

  for (const abiVersion of ["6", true]) {
    withNativeBinding(
      completeRecursiveSpendBinding({
        connectNoritoBridgeAbiVersion() {
          return abiVersion;
        },
      }),
      () => {
        assert.equal(isKagemushaRecursiveSpendNativeAvailable(), false);
        assert.equal(
          preferredKagemushaOfflineSpendMode(),
          KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
        );
      },
    );
  }
});

test("Kagemusha recursive spend availability rejects broken bridge ABI probes", () => {
  const binding = completeRecursiveSpendBinding({
    connectNoritoBridgeAbiVersion() {
      throw new Error("bridge denied");
    },
  });

  withNativeBinding(binding, () => {
    assert.equal(isKagemushaRecursiveSpendNativeAvailable(), false);
    assert.equal(
      preferredKagemushaOfflineSpendMode(),
      KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
    );
    assert.throws(
      () => kagemushaRecursiveSpendInit(Buffer.from([1])),
      /Kagemusha recursive spend helper 'kagemushaRecursiveSpendInit' is unavailable/,
    );
  });
});

test("Kagemusha recursive spend availability rejects permissive native probes", () => {
  const acceptedMethods = [
    "kagemushaRecursiveSpendInit",
    "kagemushaRecursiveSpendAppend",
    "kagemushaRecursiveSpendTransitionProfileInit",
    "kagemushaRecursiveSpendTransitionProfileAppend",
    "kagemushaRecursiveSpendLineageAppendBoundary",
    "kagemushaRecursiveSpendLineageWitnessFromInitResult",
    "kagemushaRecursiveSpendLineageWitnessAppendResult",
    "kagemushaRecursiveSpendVerify",
    "kagemushaRecursiveSpendRedeem",
  ];

  for (const acceptedMethod of acceptedMethods) {
    const binding = completeRecursiveSpendBinding({
      [acceptedMethod]() {
        return Uint8Array.from([0xff]);
      },
    });
    withNativeBinding(binding, () => {
      assert.equal(isKagemushaRecursiveSpendNativeAvailable(), false, acceptedMethod);
      assert.equal(
        preferredKagemushaOfflineSpendMode(),
        KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
        acceptedMethod,
      );
      assert.throws(
        () => kagemushaRecursiveSpendVerify(Buffer.from([1])),
        /Kagemusha recursive spend helper 'kagemushaRecursiveSpendVerify' is unavailable/,
        acceptedMethod,
      );
    });
  }
});

test("Kagemusha recursive spend availability rejects every partial ABI-6 surface", () => {
  const requiredMethods = [
    "kagemushaRecursiveSpendInit",
    "kagemushaRecursiveSpendAppend",
    "kagemushaRecursiveSpendTransitionProfileInit",
    "kagemushaRecursiveSpendTransitionProfileAppend",
    "kagemushaRecursiveSpendLineageAppendBoundary",
    "kagemushaRecursiveSpendLineageWitnessFromInitResult",
    "kagemushaRecursiveSpendLineageWitnessAppendResult",
    "kagemushaRecursiveSpendVerify",
    "kagemushaRecursiveSpendRedeem",
  ];

  for (const missingMethod of requiredMethods) {
    const binding = completeRecursiveSpendBinding();
    delete binding[missingMethod];
    withNativeBinding(binding, () => {
      assert.equal(isKagemushaRecursiveSpendNativeAvailable(), false, missingMethod);
      assert.equal(
        preferredKagemushaOfflineSpendMode(),
        KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
        missingMethod,
      );
      assert.throws(
        () => kagemushaRecursiveSpendVerify(Buffer.from([1])),
        /Kagemusha recursive spend helper 'kagemushaRecursiveSpendVerify' is unavailable/,
        missingMethod,
      );
    });
  }
});

test("Kagemusha recursive spend helpers reject empty native outputs", () => {
  const binding = {
    connectNoritoBridgeAbiVersion() {
      return 6;
    },
    kagemushaRecursiveSpendInit(request) {
      rejectMalformedProbe("init", request);
      return Buffer.alloc(0);
    },
    kagemushaRecursiveSpendAppend(request) {
      rejectMalformedProbe("append", request);
      return Buffer.alloc(0);
    },
    kagemushaRecursiveSpendTransitionProfileInit(request) {
      rejectMalformedProbe("transition-profile-init", request);
      return Buffer.alloc(0);
    },
    kagemushaRecursiveSpendTransitionProfileAppend(request) {
      rejectMalformedProbe("transition-profile-append", request);
      return Buffer.alloc(0);
    },
    kagemushaRecursiveSpendLineageAppendBoundary(profile) {
      rejectMalformedProbe("lineage-append-boundary", profile);
      return Buffer.alloc(0);
    },
    kagemushaRecursiveSpendLineageWitnessFromInitResult(request, bundle) {
      rejectMalformedProbe("lineage-init", request, bundle);
      return Buffer.alloc(0);
    },
    kagemushaRecursiveSpendLineageWitnessAppendResult(previousWitness, request, bundle) {
      rejectMalformedProbe("lineage-append", previousWitness, request, bundle);
      return Buffer.alloc(0);
    },
    kagemushaRecursiveSpendVerify(request) {
      rejectMalformedProbe("verify", request);
      return Buffer.alloc(0);
    },
    kagemushaRecursiveSpendRedeem(request) {
      rejectMalformedProbe("redeem", request);
      return Buffer.alloc(0);
    },
  };

  withNativeBinding(binding, () => {
    assert.throws(
      () => kagemushaRecursiveSpendInit(Buffer.from([1])),
      /native kagemushaRecursiveSpendInit returned empty output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendAppend(Buffer.from([1])),
      /native kagemushaRecursiveSpendAppend returned empty output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendTransitionProfileInit(Buffer.from([1])),
      /native kagemushaRecursiveSpendTransitionProfileInit returned empty output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendTransitionProfileAppend(Buffer.from([1])),
      /native kagemushaRecursiveSpendTransitionProfileAppend returned empty output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageAppendBoundary(Buffer.from([1])),
      /native kagemushaRecursiveSpendLineageAppendBoundary returned empty output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageWitnessFromInitResult(Buffer.from([1]), Buffer.from([2])),
      /native kagemushaRecursiveSpendLineageWitnessFromInitResult returned empty output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageWitnessAppendResult(Buffer.from([1]), Buffer.from([2]), Buffer.from([3])),
      /native kagemushaRecursiveSpendLineageWitnessAppendResult returned empty output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendVerify(Buffer.from([1])),
      /native kagemushaRecursiveSpendVerify returned empty output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendRedeem(Buffer.from([1])),
      /native kagemushaRecursiveSpendRedeem returned empty output/,
    );
  });
});

test("Kagemusha recursive spend helpers reject oversized native outputs", () => {
  const binding = completeRecursiveSpendBinding({
    kagemushaRecursiveSpendRedeem(request) {
      rejectMalformedProbe("redeem", request);
      return Buffer.alloc(KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f);
    },
  });

  withNativeBinding(binding, () => {
    assert.throws(
      () => kagemushaRecursiveSpendRedeem(Buffer.from([1])),
      /native kagemushaRecursiveSpendRedeem returned oversized output/,
    );
  });
});

test("Kagemusha recursive spend helpers reject missing native outputs", () => {
  const binding = {
    connectNoritoBridgeAbiVersion() {
      return 6;
    },
    kagemushaRecursiveSpendInit(request) {
      rejectMalformedProbe("init", request);
      return null;
    },
    kagemushaRecursiveSpendAppend(request) {
      rejectMalformedProbe("append", request);
      return undefined;
    },
    kagemushaRecursiveSpendTransitionProfileInit(request) {
      rejectMalformedProbe("transition-profile-init", request);
      return null;
    },
    kagemushaRecursiveSpendTransitionProfileAppend(request) {
      rejectMalformedProbe("transition-profile-append", request);
      return undefined;
    },
    kagemushaRecursiveSpendLineageAppendBoundary(profile) {
      rejectMalformedProbe("lineage-append-boundary", profile);
      return null;
    },
    kagemushaRecursiveSpendLineageWitnessFromInitResult(request, bundle) {
      rejectMalformedProbe("lineage-init", request, bundle);
      return null;
    },
    kagemushaRecursiveSpendLineageWitnessAppendResult(previousWitness, request, bundle) {
      rejectMalformedProbe("lineage-append", previousWitness, request, bundle);
      return undefined;
    },
    kagemushaRecursiveSpendVerify(request) {
      rejectMalformedProbe("verify", request);
      return null;
    },
    kagemushaRecursiveSpendRedeem(request) {
      rejectMalformedProbe("redeem", request);
      return undefined;
    },
  };

  withNativeBinding(binding, () => {
    assert.throws(
      () => kagemushaRecursiveSpendInit(Buffer.from([1])),
      /native kagemushaRecursiveSpendInit returned no output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendAppend(Buffer.from([1])),
      /native kagemushaRecursiveSpendAppend returned no output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendTransitionProfileInit(Buffer.from([1])),
      /native kagemushaRecursiveSpendTransitionProfileInit returned no output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendTransitionProfileAppend(Buffer.from([1])),
      /native kagemushaRecursiveSpendTransitionProfileAppend returned no output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageAppendBoundary(Buffer.from([1])),
      /native kagemushaRecursiveSpendLineageAppendBoundary returned no output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageWitnessFromInitResult(Buffer.from([1]), Buffer.from([2])),
      /native kagemushaRecursiveSpendLineageWitnessFromInitResult returned no output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageWitnessAppendResult(Buffer.from([1]), Buffer.from([2]), Buffer.from([3])),
      /native kagemushaRecursiveSpendLineageWitnessAppendResult returned no output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendVerify(Buffer.from([1])),
      /native kagemushaRecursiveSpendVerify returned no output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendRedeem(Buffer.from([1])),
      /native kagemushaRecursiveSpendRedeem returned no output/,
    );
  });
});

test("Kagemusha recursive spend helpers reject native text outputs", () => {
  const binding = {
    connectNoritoBridgeAbiVersion() {
      return 6;
    },
    kagemushaRecursiveSpendInit(request) {
      rejectMalformedProbe("init", request);
      return "init";
    },
    kagemushaRecursiveSpendAppend(request) {
      rejectMalformedProbe("append", request);
      return "append";
    },
    kagemushaRecursiveSpendTransitionProfileInit(request) {
      rejectMalformedProbe("transition-profile-init", request);
      return "transition-profile-init";
    },
    kagemushaRecursiveSpendTransitionProfileAppend(request) {
      rejectMalformedProbe("transition-profile-append", request);
      return "transition-profile-append";
    },
    kagemushaRecursiveSpendLineageAppendBoundary(profile) {
      rejectMalformedProbe("lineage-append-boundary", profile);
      return "lineage-append-boundary";
    },
    kagemushaRecursiveSpendLineageWitnessFromInitResult(request, bundle) {
      rejectMalformedProbe("lineage-init", request, bundle);
      return "lineage-init";
    },
    kagemushaRecursiveSpendLineageWitnessAppendResult(previousWitness, request, bundle) {
      rejectMalformedProbe("lineage-append", previousWitness, request, bundle);
      return "lineage-append";
    },
    kagemushaRecursiveSpendVerify(request) {
      rejectMalformedProbe("verify", request);
      return "verify";
    },
    kagemushaRecursiveSpendRedeem(request) {
      rejectMalformedProbe("redeem", request);
      return "redeem";
    },
  };

  withNativeBinding(binding, () => {
    assert.throws(
      () => kagemushaRecursiveSpendInit(Buffer.from([1])),
      /native kagemushaRecursiveSpendInit returned text instead of Norito bytes/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendAppend(Buffer.from([1])),
      /native kagemushaRecursiveSpendAppend returned text instead of Norito bytes/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendTransitionProfileInit(Buffer.from([1])),
      /native kagemushaRecursiveSpendTransitionProfileInit returned text instead of Norito bytes/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendTransitionProfileAppend(Buffer.from([1])),
      /native kagemushaRecursiveSpendTransitionProfileAppend returned text instead of Norito bytes/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageAppendBoundary(Buffer.from([1])),
      /native kagemushaRecursiveSpendLineageAppendBoundary returned text instead of Norito bytes/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageWitnessFromInitResult(Buffer.from([1]), Buffer.from([2])),
      /native kagemushaRecursiveSpendLineageWitnessFromInitResult returned text instead of Norito bytes/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageWitnessAppendResult(Buffer.from([1]), Buffer.from([2]), Buffer.from([3])),
      /native kagemushaRecursiveSpendLineageWitnessAppendResult returned text instead of Norito bytes/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendVerify(Buffer.from([1])),
      /native kagemushaRecursiveSpendVerify returned text instead of Norito bytes/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendRedeem(Buffer.from([1])),
      /native kagemushaRecursiveSpendRedeem returned text instead of Norito bytes/,
    );
  });
});

test("Kagemusha recursive spend redeem propagates native over-cap hop-count rejection", () => {
  const calls = [];
  const rejection = new Error(
    "invalid Kagemusha recursive spend request: bundle.accumulator.hop_count exceeds Reserved-lineage cap",
  );
  const binding = completeRecursiveSpendBinding({
    kagemushaRecursiveSpendRedeem(request) {
      rejectMalformedProbe("redeem", request);
      calls.push(Buffer.from(request));
      throw rejection;
    },
  });

  withNativeBinding(binding, () => {
    assert.equal(isKagemushaRecursiveSpendNativeAvailable(), true);
    assert.throws(
      () => kagemushaRecursiveSpendRedeem(Buffer.from([0x42])),
      /bundle\.accumulator\.hop_count/,
    );
  });

  assert.deepEqual(calls, [Buffer.from([0x42])]);
});

test("Kagemusha recursive spend helpers propagate forged lineage verifier-record rejection", () => {
  const calls = [];
  const rejection = new Error(
    "invalid Kagemusha recursive spend request: lineage_verifier_record.commitment",
  );
  const binding = completeRecursiveSpendBinding({
    kagemushaRecursiveSpendVerify(request) {
      rejectMalformedProbe("verify", request);
      calls.push(["verify", Buffer.from(request)]);
      throw rejection;
    },
    kagemushaRecursiveSpendRedeem(request) {
      rejectMalformedProbe("redeem", request);
      calls.push(["redeem", Buffer.from(request)]);
      throw rejection;
    },
  });

  withNativeBinding(binding, () => {
    assert.equal(isKagemushaRecursiveSpendNativeAvailable(), true);
    assert.throws(
      () => kagemushaRecursiveSpendVerify(Buffer.from([0x51])),
      /lineage_verifier_record\.commitment/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendRedeem(Buffer.from([0x52])),
      /lineage_verifier_record\.commitment/,
    );
  });

  assert.deepEqual(calls, [
    ["verify", Buffer.from([0x51])],
    ["redeem", Buffer.from([0x52])],
  ]);
});

test("Kagemusha recursive spend transition profile append propagates forged opening rejection", () => {
  const calls = [];
  const rejection = new Error(
    "invalid Kagemusha recursive spend request: hop domain metadata mismatch",
  );
  const binding = completeRecursiveSpendBinding({
    kagemushaRecursiveSpendTransitionProfileAppend(request) {
      rejectMalformedProbe("transition-profile-append", request);
      calls.push(Buffer.from(request));
      throw rejection;
    },
  });

  withNativeBinding(binding, () => {
    assert.equal(isKagemushaRecursiveSpendNativeAvailable(), true);
    assert.throws(
      () => kagemushaRecursiveSpendTransitionProfileAppend(Buffer.from([0x53])),
      /hop domain metadata mismatch/,
    );
  });

  assert.deepEqual(calls, [Buffer.from([0x53])]);
});

test("Kagemusha recursive spend availability fails closed when native methods are partial", () => {
  withNativeBinding({ kagemushaRecursiveSpendInit() {} }, () => {
    assert.equal(isKagemushaRecursiveSpendNativeAvailable(), false);
    assert.throws(
      () => kagemushaRecursiveSpendInit(Buffer.from([1])),
      /Kagemusha recursive spend helper 'kagemushaRecursiveSpendInit' is unavailable/,
    );
  });
});
