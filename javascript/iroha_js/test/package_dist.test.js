"use strict";

import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { ed25519 } from "@noble/curves/ed25519";

import * as packageExports from "../dist/index.js";

const {
  AccountAddress,
  ToriiClient,
  submitIvmProvedContractCall,
  submitValidationFeeIvmProvedContractCall,
  validationFeePolicyHash,
  verifySignedValidationFeePolicy,
  KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1,
  KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1,
  KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT,
  KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT,
  KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION,
  KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION,
  KAGEMUSHA_RECURSIVE_SPEND_TOPUP_REQUIRED_NATIVE_BRIDGE_ABI_VERSION,
  KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
  KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
  KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1,
  KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1,
  KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES,
  KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES,
  KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES,
  KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN,
  KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN,
  KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN,
  KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1,
  KAGEMUSHA_RECURSIVE_TOPUP_REQUEST_WIRE_NAME,
  KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
  KAGEMUSHA_RECURSIVE_SPEND_VERIFY_RESULT_WIRE_NAME,
  KAGEMUSHA_RECURSIVE_SPEND_RECORD_BUNDLE_WIRE_NAME,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
  KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
  KAGEMUSHA_VERIFYING_KEY_RECORD_WIRE_NAME,
  canAppendKagemushaRecursiveSpendWitnesslessLineage,
  canProveKagemushaRecursiveSpendAppendOutputProofCircuitId,
  canRedeemKagemushaRecursiveSpendWitnessless,
  canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId,
  isKagemushaCompactPaymentTokenNativeAvailable,
  isKagemushaPallasOpenEnvelopeBuilderNativeAvailable,
  isKagemushaRecursiveAggregationProofBundleNativeAvailable,
  isKagemushaRecursiveCompactPaymentTokenNativeAvailable,
  isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable,
  isKagemushaRecursiveCompactUnavailable,
  isSupportedKagemushaRecursiveSpendLineageKeyArtifactOpeningLen,
  isKagemushaRecursiveSpendLineageProofCircuitId,
  isKagemushaRecursiveSpendLineageAppendOutputCircuitId,
  isSupportedKagemushaRecursiveSpendAppendOutputProofCircuitId,
  isSupportedKagemushaRecursiveSpendAppendProofTransition,
  isSupportedKagemushaRecursiveSpendPreviousProofCircuitId,
  normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId,
  preferredKagemushaRecursiveSpendAppendOutputProofCircuitId,
  buildKagemushaRecursiveSpendableNoteDescriptor,
  buildKagemushaRecursiveSpendVerifierRecordRef,
  kagemushaRecursiveSpendLineageKeyArtifacts,
  kagemushaRecursiveSpendLineageKeyArtifactsForAppend,
  kagemushaRecursiveSpendLineageKeyArtifactsForInit,
  validateKagemushaRecursiveSpendLineageKeyArtifacts,
  requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput,
  requiresKagemushaRecursiveSpendLineageKeyArtifactsForInit,
  requiresKagemushaRecursiveSpendLineageWitnessForRedeem,
  requiresKagemushaRecursiveSpendPreviousLineageVerifierRecordForAppend,
  requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend,
  PRIVACY_FFI_ERROR_INVALID_REQUEST,
  PRIVACY_FFI_ERROR_MALFORMED_NORITO,
  PRIVACY_FFI_ERROR_NULL_POINTER,
  PRIVACY_FFI_ERROR_PRODUCTION_DISABLED,
  PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM,
  PRIVACY_FFI_STATUS_ERROR,
  PRIVACY_FFI_VERSION_V1,
  PRIVACY_REQUIRED_BRIDGE_ABI_VERSION,
  preferredKagemushaOfflineSpendMode,
  buildKagemushaInstructionArchiveInstruction,
  buildKagemushaInstructionTransaction,
  buildKagemushaRecursiveRedeemTransaction,
  buildKagemushaRecursiveTopUpTransaction,
  buildConfidentialTransferProofV2,
  buildConfidentialUnshieldProofV2,
  buildConfidentialUnshieldProofV3,
  buildPrivateCreateKaigiTransaction,
  buildPrivateJoinKaigiTransaction,
  buildPrivateEndKaigiTransaction,
  buildPrivateKaigiFeeSpend,
  isKagemushaRecursiveSpendNativeAvailable,
  isKagemushaRecursiveSpendTopUpNativeAvailable,
  isKagemushaSpendAgainMode,
  isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable,
  isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable,
  kagemushaProveVerifiedCompactPaymentTokenWithRecords,
  kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes,
  kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes,
  kagemushaBuildPallasOpenEnvelopesArchive,
  kagemushaBuildPreviousProofOpenEnvelopesArchive,
  kagemushaVerifyRecursiveCompactPaymentToken,
  kagemushaRecursiveSpendCompactPaymentTokenFromBundle,
  kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection,
  kagemushaRecursiveSpendInit,
  kagemushaRecursiveSpendTopUp,
  kagemushaRecursiveSpendAppend,
  kagemushaRecursiveSpendTransitionProfileInit,
  kagemushaRecursiveSpendTransitionProfileAppend,
  kagemushaRecursiveSpendLineageAppendBoundary,
  kagemushaRecursiveSpendLineageWitnessFromInitResult,
  kagemushaRecursiveSpendLineageWitnessAppendResult,
  kagemushaRecursiveSpendVerify,
  kagemushaRecursiveSpendRedeem,
  deriveConfidentialNoteV2,
  deriveConfidentialNullifierV2,
  deriveConfidentialOwnerTagV2,
  encodeKagemushaRecursiveSpendInitRequest,
  encodeKagemushaRecursiveSpendAppendRequest,
  encodeKagemushaRecursiveSpendVerifyRequest,
  encodeKagemushaRecursiveSpendRedeemRequest,
  decodeKagemushaRecursiveSpendBundle,
  decodeKagemushaRecursiveSpendVerifyResult,
  PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
  isPrivacyNativeAvailable,
  privacyCapabilitiesV1,
  privacyProofRequestV1,
  privacyBuildProofV1,
  privacyVerifyProofV1,
  getPrivacyCapabilities,
  buildPrivacyProofEnvelope,
  buildZkAtPolicyProofV1,
  buildZkAtDevProofFixture,
  verifyZkAtPolicyProofV1,
  buildZkAmsAdmissionBatchProofV0,
  buildZkAmsAdmissionDevProofFixture,
  verifyZkAmsAdmissionBatchProofV0,
  buildVegaCredentialPredicateProofV0,
  buildVegaCredentialDevProofFixture,
  verifyVegaCredentialPredicateProofV0,
  buildSilentThresholdCredentialShowingProofV0,
  buildSilentThresholdCredentialDevProofFixture,
  verifySilentThresholdCredentialShowingProofV0,
  buildZkX509IdentityDevProofFixture,
  buildZkX509IdentityProofV0,
  verifyZkX509IdentityProofV0,
  buildJindoLatticePublicInputs,
  buildJindoLatticeProofEnvelope,
  buildJindoLatticeProofV0,
  buildJindoLatticeDevProofFixture,
  verifyJindoPolynomialCommitmentV0,
  verifyJindoLatticeProofLocally,
  buildSisHintsCredentialCommitments,
  buildSisHintsCredentialEnvelope,
  buildSisHintsAnonymousCredentialProofV0,
  buildSisHintsCredentialDevProofFixture,
  verifySisHintsAnonymousCredentialProofV0,
  verifySisHintsCredentialProofLocally,
  buildAnonymousPgcReceiverSet,
  buildAnonymousPgcAccountCommitmentInstruction,
  buildAnonymousPgcKOutOfNProofV1,
  verifyAnonymousPgcKOutOfNProofV1,
  buildAnonymousPgcTransferInstruction,
  buildAnonymousPgcDevProofFixture,
  buildVeRangeDevProofFixture,
  buildOrchardActionBundleProofV1,
  buildOrchardActionBundleInstruction,
  buildPenumbraSpendProofV1,
  buildPenumbraOutputProofV1,
  buildPenumbraShieldedPoolTransaction,
  buildFcmpPlusPlusMembershipProofV1,
  buildFcmpPlusPlusTransferInstruction,
  buildMidenStarkTransactionProofV1,
  buildMidenNoteTransactionInstruction,
  buildAztecPrivateKernelProofV1,
  buildAztecPrivateRollupTransactionInstruction,
  buildPqMaspStarkTransferProofV0,
  buildPqMaspStarkRegisterPoolInstruction,
  buildPqMaspStarkTransferInstruction,
  noritoDecodePrivacyProofEnvelope,
  OfflineCashConfigurationSnapshotError,
  assertOfflineCashConfigurationSnapshotUsable,
} = packageExports;

const deterministicEd25519PublicKey = (seedByte) =>
  Buffer.from(ed25519.getPublicKey(Buffer.alloc(32, seedByte)));

const GENERIC_LINEAGE_FAMILY_ID = "kagemusha-recursive-spend-lineage-v1";
const UNSUPPORTED_RECURSIVE_SPEND_PROOF_CIRCUIT_ID =
  "kagemusha-recursive-spend-lineage-badhop-v1";

function kagemushaRequestCodecError(kind, field, messagePattern) {
  return (error) =>
    error?.kind === kind &&
    error.field === field &&
    (messagePattern == null ||
      (typeof messagePattern === "string"
        ? error.message === messagePattern
        : messagePattern.test(error.message)));
}

function privacyNoritoFrame(schemaByte) {
  const frame = Buffer.alloc(40);
  frame.write("NRT0", 0, "ascii");
  frame.fill(schemaByte, 6, 22);
  return frame;
}

function privacyNoritoFrameWithPayload(schemaByte) {
  const frame = Buffer.concat([
    privacyNoritoFrame(schemaByte),
    Buffer.from([0x00, 0x00, 0xa5, 0x5a, 0x11]),
  ]);
  frame.writeBigUInt64LE(3n, 23);
  Buffer.from([0xb9, 0xd3, 0xa8, 0x0c, 0xcd, 0x5d, 0x13, 0x24]).copy(frame, 31);
  return frame;
}

const TEST_CRC64_MASK = 0xffff_ffff_ffff_ffffn;
const TEST_CRC64_REFLECTED_POLY = 0xc96c_5795_d787_0f42n;
const TEST_CRC64_TABLE = (() => {
  const table = new Array(256);
  for (let index = 0; index < 256; index += 1) {
    let crc = BigInt(index);
    for (let bit = 0; bit < 8; bit += 1) {
      crc =
        (crc & 1n) !== 0n ? (crc >> 1n) ^ TEST_CRC64_REFLECTED_POLY : crc >> 1n;
    }
    table[index] = crc;
  }
  return table;
})();

function testCrc64(payload) {
  let crc = TEST_CRC64_MASK;
  for (const byte of payload) {
    const index = Number((crc ^ BigInt(byte)) & 0xffn);
    crc = TEST_CRC64_TABLE[index] ^ (crc >> 8n);
  }
  return BigInt.asUintN(64, crc ^ TEST_CRC64_MASK);
}

function privacyNoritoFrameFromPayload(schemaByte, payload) {
  const payloadBuffer = Buffer.from(payload);
  const frame = Buffer.concat([privacyNoritoFrame(schemaByte), payloadBuffer]);
  frame.writeBigUInt64LE(BigInt(payloadBuffer.length), 23);
  frame.writeBigUInt64LE(testCrc64(payloadBuffer), 31);
  return frame;
}

const TEST_NORITO_COMPACT_LEN_FLAG = 0x02;
const KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH = Buffer.from(
  "c88489618a012c283ff3bb2ebabc7775",
  "hex",
);
const OLD_KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH = Buffer.from(
  "119f4df38a98ef5848ad0aadb9715779",
  "hex",
);
const PALLAS_OPEN_ENVELOPE_VECTOR_SCHEMA_HASH = Buffer.from(
  "fe3826328f081771750f24fe110260ca",
  "hex",
);
const PACKAGE_DIST_KAGEMUSHA_INSTRUCTION_ARCHIVE_WIRE_NAMES = {
  KagemushaTransfer: "iroha_data_model::isi::offline::KagemushaTransfer",
  RedeemKagemushaRecursive:
    "iroha_data_model::isi::offline::RedeemKagemushaRecursive",
  TopUpKagemushaRecursive:
    "iroha_data_model::isi::offline::TopUpKagemushaRecursive",
};

function privacyNoritoFrameFromSchemaHash(schemaHash, payload, flags = 0) {
  const payloadBuffer = Buffer.from(payload);
  const frame = Buffer.alloc(40);
  frame.write("NRT0", 0, "ascii");
  Buffer.from(schemaHash).copy(frame, 6);
  frame[39] = flags;
  const archive = Buffer.concat([frame, payloadBuffer]);
  archive.writeBigUInt64LE(BigInt(payloadBuffer.length), 23);
  archive.writeBigUInt64LE(testCrc64(payloadBuffer), 31);
  return archive;
}

function kagemushaNoritoLength(value, flags = 0) {
  if ((flags & TEST_NORITO_COMPACT_LEN_FLAG) === 0) {
    const length = Buffer.alloc(8);
    length.writeBigUInt64LE(BigInt(value));
    return length;
  }
  let remaining = BigInt(value);
  const bytes = [];
  while (remaining >= 0x80n) {
    bytes.push(Number((remaining & 0x7fn) | 0x80n));
    remaining >>= 7n;
  }
  bytes.push(Number(remaining));
  return Buffer.from(bytes);
}

function kagemushaNoritoField(payload, flags = TEST_NORITO_COMPACT_LEN_FLAG) {
  const bytes = Buffer.from(payload);
  return Buffer.concat([kagemushaNoritoLength(bytes.length, flags), bytes]);
}

function kagemushaReadNoritoLength(payload, offset, flags = TEST_NORITO_COMPACT_LEN_FLAG) {
  if ((flags & TEST_NORITO_COMPACT_LEN_FLAG) === 0) {
    assert.ok(offset + 8 <= payload.length);
    const value = payload.readBigUInt64LE(offset);
    assert.ok(value <= BigInt(Number.MAX_SAFE_INTEGER));
    return { value: Number(value), offset: offset + 8 };
  }
  let value = 0n;
  let shift = 0n;
  let cursor = offset;
  while (cursor < payload.length) {
    const byte = payload[cursor];
    cursor += 1;
    value |= BigInt(byte & 0x7f) << shift;
    if ((byte & 0x80) === 0) {
      assert.ok(value <= BigInt(Number.MAX_SAFE_INTEGER));
      return { value: Number(value), offset: cursor };
    }
    shift += 7n;
  }
  assert.fail("unterminated Norito compact length");
}

function kagemushaReadNoritoFields(payload, flags = TEST_NORITO_COMPACT_LEN_FLAG) {
  const fields = [];
  let offset = 0;
  while (offset < payload.length) {
    const length = kagemushaReadNoritoLength(payload, offset, flags);
    const end = length.offset + length.value;
    assert.ok(end <= payload.length);
    fields.push(payload.subarray(length.offset, end));
    offset = end;
  }
  return fields;
}

function kagemushaReadSequenceFields(payload) {
  const buffer = Buffer.from(payload);
  assert.ok(buffer.length >= 8, "sequence payload must include a u64 count");
  const count = Number(buffer.readBigUInt64LE(0));
  assert.ok(Number.isSafeInteger(count), "sequence count must fit in Number");
  const fields = [];
  let offset = 8;
  for (let index = 0; index < count; index += 1) {
    const length = kagemushaReadNoritoLength(buffer, offset);
    const end = length.offset + length.value;
    assert.ok(end <= buffer.length);
    fields.push(buffer.subarray(length.offset, end));
    offset = end;
  }
  assert.equal(offset, buffer.length);
  return fields;
}

function kagemushaEncodeSequenceFields(fields) {
  return Buffer.concat([u64LE(fields.length), ...fields.map((field) => kagemushaNoritoField(field))]);
}

function kagemushaU32Payload(value) {
  const payload = Buffer.alloc(4);
  payload.writeUInt32LE(value);
  return payload;
}

function kagemushaNumericPayload(mantissa, scale = 0) {
  const mantissaBytes = Buffer.from(mantissa);
  const mantissaPayload = Buffer.alloc(4 + mantissaBytes.length);
  mantissaPayload.writeUInt32LE(mantissaBytes.length);
  mantissaBytes.copy(mantissaPayload, 4);
  return Buffer.concat([
    kagemushaNoritoField(mantissaPayload),
    kagemushaNoritoField(kagemushaU32Payload(scale)),
  ]);
}

function kagemushaNumericPayloadWithMantissaPayload(mantissaPayload) {
  return Buffer.concat([
    kagemushaNoritoField(mantissaPayload),
    kagemushaNoritoField(kagemushaU32Payload(0)),
  ]);
}

function kagemushaNumericPayloadWithScalePayload(scalePayload) {
  const mantissaPayload = Buffer.from([1, 0, 0, 0, 1]);
  return Buffer.concat([
    kagemushaNoritoField(mantissaPayload),
    kagemushaNoritoField(scalePayload),
  ]);
}

function kagemushaNumericPayloadWithTrailingField() {
  return Buffer.concat([
    kagemushaNumericPayload(Buffer.from([1])),
    kagemushaNoritoField(kagemushaU32Payload(0x42)),
  ]);
}

function kagemushaZeroNumericPayload() {
  return kagemushaNumericPayload(Buffer.alloc(0));
}

function kagemushaFixedArrayPayload(value, count) {
  return Buffer.concat(
    Array.from({ length: count }, () => kagemushaNoritoField(Buffer.from([value]))),
  );
}

function kagemushaCountPrefixedFixedArrayPayload(value, count) {
  const length = Buffer.alloc(8);
  length.writeBigUInt64LE(BigInt(count));
  return Buffer.concat([length, kagemushaFixedArrayPayload(value, count)]);
}

function kagemushaNoritoString(value, flags = TEST_NORITO_COMPACT_LEN_FLAG) {
  const bytes = Buffer.from(value, "utf8");
  return Buffer.concat([kagemushaNoritoLength(bytes.length, flags), bytes]);
}

function kagemushaSchemaHashForTypeName(typeName) {
  return createHash("sha256")
    .update(Buffer.from("norito:v1:type-name\0", "utf8"))
    .update(Buffer.from(typeName, "utf8"))
    .digest()
    .subarray(0, 16);
}

function syntheticKagemushaArchive(typeName, seed, flags = TEST_NORITO_COMPACT_LEN_FLAG) {
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(typeName),
    Buffer.from([seed]),
    flags,
  );
}

function packageDistKagemushaInstructionArchive(type, payload) {
  const wireName = PACKAGE_DIST_KAGEMUSHA_INSTRUCTION_ARCHIVE_WIRE_NAMES[type];
  if (typeof wireName !== "string") {
    throw new TypeError(`unknown package dist Kagemusha instruction archive type: ${type}`);
  }
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(wireName),
    payload,
  );
}

function syntheticKagemushaRecordBundleArchive(hopCount = 1, options = {}) {
  const stepPayload = Buffer.concat(
    Array.from({ length: 6 }, (_, index) => kagemushaNoritoField(Buffer.from([0xa0 + index]))),
  );
  const stepsPayload = options.stepsPayload ?? Buffer.concat([
    u64LE(hopCount),
    ...Array.from({ length: hopCount }, () => kagemushaNoritoField(stepPayload)),
  ]);
  const bundlePayload = Buffer.concat([
    kagemushaNoritoField(Buffer.from([0x41])),
    kagemushaNoritoField(Buffer.from([0x42])),
    kagemushaNoritoField(stepsPayload),
  ]);
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_RECORD_BUNDLE_WIRE_NAME),
    Buffer.concat([
      kagemushaNoritoField(bundlePayload),
      kagemushaNoritoField(Buffer.alloc(0)),
    ]),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function syntheticPallasOpenEnvelopesArchive(count = 1, options = {}) {
  const envelope = syntheticPallasOpenEnvelopePayload(options);
  return privacyNoritoFrameFromSchemaHash(
    PALLAS_OPEN_ENVELOPE_VECTOR_SCHEMA_HASH,
    Buffer.concat([
      u64LE(count),
      ...Array.from({ length: count }, () => kagemushaNoritoField(envelope)),
    ]),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function syntheticPallasOpenEnvelopePayload(options = {}) {
  const n = options.n ?? 4;
  const params = Buffer.concat([
    kagemushaNoritoField(u16LE(1)),
    kagemushaNoritoField(u16LE(options.paramsCurveId ?? 1)),
    kagemushaNoritoField(kagemushaU32Payload(n)),
    kagemushaNoritoField(options.paramsGSequencePayload ?? fixed32Sequence(n, 0x10)),
    kagemushaNoritoField(options.paramsHSequencePayload ?? fixed32Sequence(n, 0x20)),
    kagemushaNoritoField(syntheticFixed32(0x30)),
  ]);
  const publicValue = Buffer.concat([
    kagemushaNoritoField(u16LE(1)),
    kagemushaNoritoField(u16LE(options.publicCurveId ?? 1)),
    kagemushaNoritoField(kagemushaU32Payload(n)),
    kagemushaNoritoField(syntheticFixed32(0x31)),
    kagemushaNoritoField(syntheticFixed32(0x32)),
    kagemushaNoritoField(syntheticFixed32(0x33)),
  ]);
  const proof = Buffer.concat([
    kagemushaNoritoField(u16LE(1)),
    kagemushaNoritoField(options.proofLSequencePayload ?? fixed32Sequence(2, 0x40)),
    kagemushaNoritoField(options.proofRSequencePayload ?? fixed32Sequence(2, 0x50)),
    kagemushaNoritoField(syntheticFixed32(0x60)),
    kagemushaNoritoField(syntheticFixed32(0x61)),
  ]);
  const vkCommitmentOptionPayload =
    options.vkCommitmentOptionPayload ??
    optionRaw(
      options.includeVkCommitment === false
        ? null
        : options.vkCommitmentPayload ?? syntheticFixed32(0x70),
    );
  const publicInputsSchemaHashOptionPayload =
    options.publicInputsSchemaHashOptionPayload ??
    optionRaw(
      options.includePublicInputsSchemaHash === false
        ? null
        : options.publicInputsSchemaHashPayload ?? syntheticFixed32(0x71),
    );
  const domainTagOptionPayload =
    options.domainTagOptionPayload ??
    optionRaw(
      options.includeDomainTag === false
        ? null
        : options.domainTagPayload ?? syntheticFixed32(0x72),
    );
  return Buffer.concat([
    kagemushaNoritoField(params),
    kagemushaNoritoField(publicValue),
    kagemushaNoritoField(proof),
    kagemushaNoritoField(kagemushaNoritoString(options.transcriptLabel ?? "pallas-open")),
    kagemushaNoritoField(vkCommitmentOptionPayload),
    kagemushaNoritoField(publicInputsSchemaHashOptionPayload),
    kagemushaNoritoField(domainTagOptionPayload),
  ]);
}

function u64LE(value) {
  const out = Buffer.alloc(8);
  out.writeBigUInt64LE(BigInt(value));
  return out;
}

function u16LE(value) {
  const out = Buffer.alloc(2);
  out.writeUInt16LE(value);
  return out;
}

function syntheticFixed32(seed) {
  return Buffer.from(Array.from({ length: 32 }, (_, index) => (seed + index) & 0xff));
}

function fixed32Sequence(count, seed) {
  return Buffer.concat([
    u64LE(count),
    ...Array.from({ length: count }, (_, index) =>
      kagemushaNoritoField(syntheticFixed32(seed + index)),
    ),
  ]);
}

function optionRaw(payload) {
  if (payload == null) {
    return Buffer.from([0]);
  }
  return Buffer.concat([
    Buffer.from([1]),
    kagemushaNoritoLength(payload.length, TEST_NORITO_COMPACT_LEN_FLAG),
    Buffer.from(payload),
  ]);
}

function optionRawWithTrailingByte(payload) {
  return Buffer.concat([optionRaw(payload), Buffer.from([0x7f])]);
}

function optionRawWithUnknownTag() {
  return Buffer.from([0x02]);
}

function optionRawWithDeclaredLengthTooLong(payload) {
  return Buffer.concat([
    Buffer.from([1]),
    kagemushaNoritoLength(payload.length + 1, TEST_NORITO_COMPACT_LEN_FLAG),
    Buffer.from(payload),
  ]);
}

function kagemushaNoritoByteVec(value) {
  const bytes = Buffer.from(value);
  const length = Buffer.alloc(8);
  length.writeBigUInt64LE(BigInt(bytes.length));
  return Buffer.concat([length, bytes]);
}

function kagemushaZk1Tlv(tag, payload) {
  const payloadBuffer = Buffer.from(payload);
  const length = Buffer.alloc(4);
  length.writeUInt32LE(payloadBuffer.length);
  return Buffer.concat([Buffer.from(tag, "ascii"), length, payloadBuffer]);
}

function kagemushaLineageVerifierKey(circuitId, seed) {
  return Buffer.concat([
    Buffer.from([0x5a, 0x4b, 0x31, 0x00]),
    kagemushaZk1Tlv("IPAK", Buffer.from([8, 0, 0, 0])),
    kagemushaZk1Tlv("CID1", Buffer.from(circuitId, "utf8")),
    kagemushaZk1Tlv("H2VK", Buffer.alloc(32, seed)),
  ]);
}

function kagemushaVerifierKeyCommitment(verifierKey) {
  const backend = Buffer.from(
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
    "utf8",
  );
  const backendLength = Buffer.alloc(8);
  backendLength.writeBigUInt64BE(BigInt(backend.length));
  const verifierKeyLength = Buffer.alloc(8);
  verifierKeyLength.writeBigUInt64BE(BigInt(verifierKey.length));
  return createHash("sha256")
    .update("iroha:zk:v1:vk")
    .update(backendLength)
    .update(backend)
    .update(verifierKeyLength)
    .update(verifierKey)
    .digest();
}

function kagemushaLineageProvingKeyArchive(circuitId, verifierKey, seed) {
  const flags = TEST_NORITO_COMPACT_LEN_FLAG;
  const version = Buffer.alloc(2);
  version.writeUInt16LE(1);
  const payload = Buffer.concat([
    kagemushaNoritoField(version, flags),
    kagemushaNoritoField(kagemushaNoritoString(circuitId, flags), flags),
    kagemushaNoritoField(kagemushaVerifierKeyCommitment(verifierKey), flags),
    kagemushaNoritoField(kagemushaNoritoByteVec(Buffer.alloc(64, seed)), flags),
  ]);
  return privacyNoritoFrameFromSchemaHash(
    KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
    payload,
    flags,
  );
}

function privacyNoritoFrameWithPadding(schemaByte, paddingLength) {
  const frame = Buffer.concat([
    privacyNoritoFrame(schemaByte),
    Buffer.alloc(paddingLength),
    Buffer.from([0xa5, 0x5a, 0x11]),
  ]);
  frame.writeBigUInt64LE(3n, 23);
  Buffer.from([0xb9, 0xd3, 0xa8, 0x0c, 0xcd, 0x5d, 0x13, 0x24]).copy(frame, 31);
  return frame;
}

function privacyNoritoFrameWithSchemaOverride(schemaByte, offset, value) {
  const frame = Buffer.from(privacyNoritoFrameWithPayload(schemaByte));
  frame[offset] = value;
  return frame;
}

function privacyNoritoFrameWithDeclaredPayloadLength(
  schemaByte,
  payloadLength,
) {
  const frame = Buffer.from(privacyNoritoFrameWithPayload(schemaByte));
  frame.writeBigUInt64LE(BigInt(payloadLength), 23);
  return frame;
}

function privacyNoritoFrameWithFlags(schemaByte, flags) {
  const frame = Buffer.from(privacyNoritoFrameWithPayload(schemaByte));
  frame[39] = flags;
  return frame;
}

function slicedPrivacyView(
  archive,
  prefix = [0xff, 0x7f, 0x42],
  suffix = [0x24, 0x13],
) {
  const backing = Uint8Array.from([...prefix, ...archive, ...suffix]);
  return backing.subarray(prefix.length, prefix.length + archive.length);
}

function malformedPrivacyRequestArchives() {
  const badMagic = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badMagic[0] = 0x00;
  const badVersion = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badVersion[4] = 1;
  const badMinorVersion = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badMinorVersion[5] = 1;
  const badCompression = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badCompression[22] = 1;
  const badDeclaredPayloadLength = privacyNoritoFrameWithDeclaredPayloadLength(
    0x52,
    6n,
  );
  const badOversizedDeclaredPayloadLength =
    privacyNoritoFrameWithDeclaredPayloadLength(0x52, 0x8000000000000000n);
  const badPadding = Buffer.concat([
    PRIVACY_REQUEST_ARCHIVE,
    Buffer.from([0x7f]),
  ]);
  const badExcessivePadding = privacyNoritoFrameWithPadding(0x52, 65);
  const badFlags = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badFlags[39] = 0x08;
  const badFieldBitsetFlags = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badFieldBitsetFlags[39] = 0x20;
  const badChecksum = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badChecksum[31] ^= 0x01;
  const badPayload = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badPayload[44] ^= 0x7f;
  return [
    Buffer.from([1]),
    badMagic,
    badVersion,
    badMinorVersion,
    badCompression,
    badDeclaredPayloadLength,
    badOversizedDeclaredPayloadLength,
    badPadding,
    badExcessivePadding,
    badFlags,
    badFieldBitsetFlags,
    badChecksum,
    badPayload,
  ];
}

const PRIVACY_CAPABILITIES_ARCHIVE = privacyNoritoFrameWithPayload(0x50);
const PRIVACY_BUILD_ARCHIVE = privacyNoritoFrameWithPayload(0x42);
const PRIVACY_VERIFY_ARCHIVE = privacyNoritoFrameWithPayload(0x56);
const PRIVACY_REQUEST_ARCHIVE = privacyNoritoFrameWithPayload(0x52);

function privacyProofRequestNativeArchive() {
  return Uint8Array.from(PRIVACY_REQUEST_ARCHIVE);
}

function malformedPrivacyNativeOutputArchives(schemaByte) {
  const archive = privacyNoritoFrameWithPayload(schemaByte);
  const badMagic = Buffer.from(archive);
  badMagic[0] = 0x00;
  const badVersion = Buffer.from(archive);
  badVersion[4] = 1;
  const badMinorVersion = Buffer.from(archive);
  badMinorVersion[5] = 1;
  const badCompression = Buffer.from(archive);
  badCompression[22] = 1;
  const badDeclaredPayloadLength = privacyNoritoFrameWithDeclaredPayloadLength(
    schemaByte,
    6n,
  );
  const badOversizedDeclaredPayloadLength =
    privacyNoritoFrameWithDeclaredPayloadLength(
      schemaByte,
      0x8000000000000000n,
    );
  const badPadding = Buffer.concat([archive, Buffer.from([0x7f])]);
  const badExcessivePadding = privacyNoritoFrameWithPadding(schemaByte, 65);
  const badFlags = Buffer.from(archive);
  badFlags[39] = 0x08;
  const badFieldBitsetFlags = Buffer.from(archive);
  badFieldBitsetFlags[39] = 0x20;
  const badChecksum = Buffer.from(archive);
  badChecksum[31] ^= 0x01;
  const badPayload = Buffer.from(archive);
  badPayload[44] ^= 0x7f;
  return [
    Buffer.from([1]),
    badMagic,
    badVersion,
    badMinorVersion,
    badCompression,
    badDeclaredPayloadLength,
    badOversizedDeclaredPayloadLength,
    badPadding,
    badExcessivePadding,
    badFlags,
    badFieldBitsetFlags,
    badChecksum,
    badPayload,
  ];
}

const LEGACY_FULLWIDTH_KANA =
  /[イロハニホヘトチリヌルヲワカヨタレソツネナラムウノオクヤマケフコエテアサキユメミシヒモセス]/u;
const HALFWIDTH_KANA = /[ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼﾋﾓｾｽ]/u;
const DECLARATIONS_TEXT = readFileSync(
  new URL("../index.d.ts", import.meta.url),
  "utf8",
);
const PACKAGE_DECLARATION_TEXTS = new Map([
  ["index.d.ts", DECLARATIONS_TEXT],
  [
    "connect.browser.d.ts",
    readFileSync(new URL("../connect.browser.d.ts", import.meta.url), "utf8"),
  ],
  [
    "nexus-app.d.ts",
    readFileSync(new URL("../nexus-app.d.ts", import.meta.url), "utf8"),
  ],
  [
    "kotodama-compiler.d.ts",
    readFileSync(new URL("../kotodama-compiler.d.ts", import.meta.url), "utf8"),
  ],
]);
const SCCP_SOURCE_TEXT = readFileSync(
  new URL("../src/sccp.js", import.meta.url),
  "utf8",
);
const INDEX_SOURCE_TEXT = readFileSync(
  new URL("../src/index.js", import.meta.url),
  "utf8",
);
const DIST_SCCP_TEXT = readFileSync(
  new URL("../dist/sccp.js", import.meta.url),
  "utf8",
);
const DIST_INDEX_TEXT = readFileSync(
  new URL("../dist/index.js", import.meta.url),
  "utf8",
);
const PACKAGE_JSON_TEXT = readFileSync(
  new URL("../package.json", import.meta.url),
  "utf8",
);

function sharedRecursiveSpendArchive(fixtureName, archiveName) {
  const fixture = JSON.parse(
    readFileSync(
      new URL(`../../../fixtures/${fixtureName}/archives.json`, import.meta.url),
      "utf8",
    ),
  );
  const archive = fixture.archives.find((entry) => entry.name === archiveName);
  assert.ok(archive, `missing ${fixtureName} archive fixture ${archiveName}`);
  return Buffer.from(archive.bytes_base64, "base64");
}

function sharedRecursiveSpendAbi6Archive(archiveName) {
  return sharedRecursiveSpendArchive("kagemusha_recursive_spend_abi6", archiveName);
}

function sharedRecursiveSpendAbi7Archive(archiveName) {
  return sharedRecursiveSpendArchive("kagemusha_recursive_spend_abi7", archiveName);
}

function kagemushaArchivePayload(archive) {
  const buffer = Buffer.from(archive);
  const length = Number(buffer.readBigUInt64LE(23));
  return buffer.subarray(buffer.length - length);
}

function recursiveSpendBundleWithAccumulatorDomain(domain) {
  return recursiveSpendBundleWithAccumulatorField(0, kagemushaNoritoString(domain));
}

function recursiveSpendBundleWithAccumulatorField(fieldIndex, replacement) {
  const bundleFields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("init_bundle")),
  );
  const accumulatorFields = kagemushaReadNoritoFields(bundleFields[0]);
  accumulatorFields[fieldIndex] = Buffer.from(replacement);
  bundleFields[0] = Buffer.concat(
    accumulatorFields.map((field) => kagemushaNoritoField(field)),
  );
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithTopupAnchorNullifiers(nullifiers) {
  return recursiveSpendBundleWithAccumulatorField(
    5,
    kagemushaEncodeSequenceFields(nullifiers.map((nullifier) => Buffer.from(nullifier))),
  );
}

function recursiveSpendBundleWithTopupAnchorNullifiersAndEmptyProofBytes(nullifiers) {
  const bundleFields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("init_bundle")),
  );
  const accumulatorFields = kagemushaReadNoritoFields(bundleFields[0]);
  accumulatorFields[5] = kagemushaEncodeSequenceFields(
    nullifiers.map((nullifier) => Buffer.from(nullifier)),
  );
  bundleFields[0] = Buffer.concat(
    accumulatorFields.map((field) => kagemushaNoritoField(field)),
  );
  const proofFields = kagemushaReadNoritoFields(bundleFields[1]);
  const proofBoxFields = kagemushaReadNoritoFields(proofFields[3]);
  proofBoxFields[1] = kagemushaNoritoByteVec(Buffer.alloc(0));
  proofFields[3] = Buffer.concat(
    proofBoxFields.map((field) => kagemushaNoritoField(field)),
  );
  bundleFields[1] = Buffer.concat(proofFields.map((field) => kagemushaNoritoField(field)));
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithTrailingBundleField() {
  const bundleFields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("init_bundle")),
  );
  bundleFields.push(kagemushaNoritoString("ignored-extra-bundle-field"));
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendVerifyResultWithTrailingField() {
  const fields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi7Archive("verify_result")),
  );
  fields.push(Buffer.from([0x01]));
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_VERIFY_RESULT_WIRE_NAME),
    Buffer.concat(fields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithTopupAnchorNullifiersAndTrailingAccumulatorField(nullifiers) {
  const bundleFields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("init_bundle")),
  );
  const accumulatorFields = kagemushaReadNoritoFields(bundleFields[0]);
  accumulatorFields[5] = kagemushaEncodeSequenceFields(
    nullifiers.map((nullifier) => Buffer.from(nullifier)),
  );
  accumulatorFields.push(kagemushaNoritoString("ignored-extra-accumulator-field"));
  bundleFields[0] = Buffer.concat(
    accumulatorFields.map((field) => kagemushaNoritoField(field)),
  );
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendLineageWitnessWithTrailingField() {
  const fields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("lineage_witness_append_result")),
  );
  fields.push(kagemushaNoritoString("ignored-extra-lineage-witness-field"));
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME),
    Buffer.concat(fields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendLineageWitnessWithTrailingPreviousProofsField() {
  const fields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("lineage_witness_append_result")),
  );
  fields[3] = Buffer.concat([
    fields[3],
    kagemushaNoritoField(kagemushaNoritoString("ignored-extra-previous-proofs-field")),
  ]);
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME),
    Buffer.concat(fields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendLineageWitnessWithPreviousProofCountPrefixOnly(count) {
  const fields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("lineage_witness_append_result")),
  );
  fields[3] = u64LE(count);
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME),
    Buffer.concat(fields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendLineageWitnessWithTrailingPreviousProofField() {
  const fields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("lineage_witness_append_result")),
  );
  const previousProofs = kagemushaReadSequenceFields(fields[3]);
  assert.ok(previousProofs.length > 0);
  const previousProofFields = kagemushaReadNoritoFields(previousProofs[0]);
  previousProofFields.push(kagemushaNoritoString("ignored-extra-previous-proof-field"));
  previousProofs[0] = Buffer.concat(
    previousProofFields.map((field) => kagemushaNoritoField(field)),
  );
  fields[3] = kagemushaEncodeSequenceFields(previousProofs);
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME),
    Buffer.concat(fields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendLineageWitnessWithTrailingPreviousVerifierKeyIdField() {
  const fields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("lineage_witness_append_result")),
  );
  const previousProofs = kagemushaReadSequenceFields(fields[3]);
  assert.ok(previousProofs.length > 0);
  const previousProofFields = kagemushaReadNoritoFields(previousProofs[0]);
  const verifierKeyIdFields = kagemushaReadNoritoFields(previousProofFields[0]);
  verifierKeyIdFields.push(kagemushaNoritoString("ignored-extra-previous-verifier-key-field"));
  previousProofFields[0] = Buffer.concat(
    verifierKeyIdFields.map((field) => kagemushaNoritoField(field)),
  );
  previousProofs[0] = Buffer.concat(
    previousProofFields.map((field) => kagemushaNoritoField(field)),
  );
  fields[3] = kagemushaEncodeSequenceFields(previousProofs);
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME),
    Buffer.concat(fields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendLineageWitnessWithPreviousProofField(fieldIndex, replacement) {
  const fields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("lineage_witness_append_result")),
  );
  const previousProofs = kagemushaReadSequenceFields(fields[3]);
  assert.ok(previousProofs.length > 0);
  const previousProofFields = kagemushaReadNoritoFields(previousProofs[0]);
  previousProofFields[fieldIndex] = Buffer.from(replacement);
  previousProofs[0] = Buffer.concat(
    previousProofFields.map((field) => kagemushaNoritoField(field)),
  );
  fields[3] = kagemushaEncodeSequenceFields(previousProofs);
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME),
    Buffer.concat(fields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendLineageWitnessWithPreviousProofBoxBackend(proofBackend) {
  const fields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("lineage_witness_append_result")),
  );
  const previousProofs = kagemushaReadSequenceFields(fields[3]);
  assert.ok(previousProofs.length > 0);
  const previousProofFields = kagemushaReadNoritoFields(previousProofs[0]);
  const proofBoxFields = kagemushaReadNoritoFields(previousProofFields[3]);
  proofBoxFields[0] = kagemushaNoritoString(proofBackend);
  previousProofFields[3] = Buffer.concat(
    proofBoxFields.map((field) => kagemushaNoritoField(field)),
  );
  previousProofs[0] = Buffer.concat(
    previousProofFields.map((field) => kagemushaNoritoField(field)),
  );
  fields[3] = kagemushaEncodeSequenceFields(previousProofs);
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME),
    Buffer.concat(fields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendLineageWitnessWithPreviousProofBoxBackendAndEmptyProofBytes(proofBackend) {
  const fields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("lineage_witness_append_result")),
  );
  const previousProofs = kagemushaReadSequenceFields(fields[3]);
  assert.ok(previousProofs.length > 0);
  const previousProofFields = kagemushaReadNoritoFields(previousProofs[0]);
  const proofBoxFields = kagemushaReadNoritoFields(previousProofFields[3]);
  proofBoxFields[0] = kagemushaNoritoString(proofBackend);
  proofBoxFields[1] = kagemushaNoritoByteVec(Buffer.alloc(0));
  previousProofFields[3] = Buffer.concat(
    proofBoxFields.map((field) => kagemushaNoritoField(field)),
  );
  previousProofs[0] = Buffer.concat(
    previousProofFields.map((field) => kagemushaNoritoField(field)),
  );
  fields[3] = kagemushaEncodeSequenceFields(previousProofs);
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME),
    Buffer.concat(fields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendLineageWitnessWithEmptyPreviousProofBytes() {
  const fields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("lineage_witness_append_result")),
  );
  const previousProofs = kagemushaReadSequenceFields(fields[3]);
  assert.ok(previousProofs.length > 0);
  const previousProofFields = kagemushaReadNoritoFields(previousProofs[0]);
  const proofBoxFields = kagemushaReadNoritoFields(previousProofFields[3]);
  proofBoxFields[1] = u64LE(0);
  previousProofFields[3] = Buffer.concat(
    proofBoxFields.map((field) => kagemushaNoritoField(field)),
  );
  previousProofs[0] = Buffer.concat(
    previousProofFields.map((field) => kagemushaNoritoField(field)),
  );
  fields[3] = kagemushaEncodeSequenceFields(previousProofs);
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME),
    Buffer.concat(fields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithTrailingAccumulatorField() {
  const bundleFields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("init_bundle")),
  );
  const accumulatorFields = kagemushaReadNoritoFields(bundleFields[0]);
  accumulatorFields.push(kagemushaNoritoString("ignored-extra-accumulator-field"));
  bundleFields[0] = Buffer.concat(
    accumulatorFields.map((field) => kagemushaNoritoField(field)),
  );
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithCurrentNoteField(fieldIndex, replacement) {
  const bundleFields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("init_bundle")),
  );
  const accumulatorFields = kagemushaReadNoritoFields(bundleFields[0]);
  const currentNoteFields = kagemushaReadNoritoFields(accumulatorFields[22]);
  currentNoteFields[fieldIndex] = Buffer.from(replacement);
  accumulatorFields[22] = Buffer.concat(
    currentNoteFields.map((field) => kagemushaNoritoField(field)),
  );
  bundleFields[0] = Buffer.concat(
    accumulatorFields.map((field) => kagemushaNoritoField(field)),
  );
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithTrailingCurrentNoteField() {
  const bundleFields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("init_bundle")),
  );
  const accumulatorFields = kagemushaReadNoritoFields(bundleFields[0]);
  const currentNoteFields = kagemushaReadNoritoFields(accumulatorFields[22]);
  currentNoteFields.push(kagemushaNoritoString("ignored-extra-current-note-field"));
  accumulatorFields[22] = Buffer.concat(
    currentNoteFields.map((field) => kagemushaNoritoField(field)),
  );
  bundleFields[0] = Buffer.concat(
    accumulatorFields.map((field) => kagemushaNoritoField(field)),
  );
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithCurrentNoteFieldAndTrailingField(fieldIndex, replacement) {
  const bundleFields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("init_bundle")),
  );
  const accumulatorFields = kagemushaReadNoritoFields(bundleFields[0]);
  const currentNoteFields = kagemushaReadNoritoFields(accumulatorFields[22]);
  currentNoteFields[fieldIndex] = Buffer.from(replacement);
  currentNoteFields.push(kagemushaNoritoString("ignored-extra-current-note-field"));
  accumulatorFields[22] = Buffer.concat(
    currentNoteFields.map((field) => kagemushaNoritoField(field)),
  );
  bundleFields[0] = Buffer.concat(
    accumulatorFields.map((field) => kagemushaNoritoField(field)),
  );
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithEqualCurrentNoteNullifier() {
  const bundleFields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("init_bundle")),
  );
  const accumulatorFields = kagemushaReadNoritoFields(bundleFields[0]);
  const currentNoteFields = kagemushaReadNoritoFields(accumulatorFields[22]);
  currentNoteFields[1] = Buffer.from(currentNoteFields[0]);
  accumulatorFields[22] = Buffer.concat(
    currentNoteFields.map((field) => kagemushaNoritoField(field)),
  );
  bundleFields[0] = Buffer.concat(
    accumulatorFields.map((field) => kagemushaNoritoField(field)),
  );
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithEqualCurrentNoteNullifierAndTrailingField() {
  const bundleFields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("init_bundle")),
  );
  const accumulatorFields = kagemushaReadNoritoFields(bundleFields[0]);
  const currentNoteFields = kagemushaReadNoritoFields(accumulatorFields[22]);
  currentNoteFields[1] = Buffer.from(currentNoteFields[0]);
  currentNoteFields.push(kagemushaNoritoString("ignored-extra-current-note-field"));
  accumulatorFields[22] = Buffer.concat(
    currentNoteFields.map((field) => kagemushaNoritoField(field)),
  );
  bundleFields[0] = Buffer.concat(
    accumulatorFields.map((field) => kagemushaNoritoField(field)),
  );
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithProofCircuitId(proofCircuitId) {
  const payload = Buffer.from(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("init_bundle")),
  );
  const expected = Buffer.from(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    "utf8",
  );
  const replacement = Buffer.from(proofCircuitId, "utf8");
  assert.equal(replacement.length, expected.length);
  let offset = 0;
  let replacements = 0;
  while ((offset = payload.indexOf(expected, offset)) !== -1) {
    replacement.copy(payload, offset);
    offset += replacement.length;
    replacements += 1;
  }
  assert.equal(replacements, 2);
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    payload,
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithProofBackend(proofBackend) {
  const payload = Buffer.from(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("init_bundle")),
  );
  const expected = Buffer.from(KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND, "utf8");
  const replacement = Buffer.from(proofBackend, "utf8");
  assert.equal(replacement.length, expected.length);
  let offset = 0;
  let replacements = 0;
  while ((offset = payload.indexOf(expected, offset)) !== -1) {
    replacement.copy(payload, offset);
    offset += replacement.length;
    replacements += 1;
  }
  assert.equal(replacements, 2);
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    payload,
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithProofBoxBackend(proofBackend) {
  const bundleFields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("init_bundle")),
  );
  const proofFields = kagemushaReadNoritoFields(bundleFields[1]);
  const proofBoxFields = kagemushaReadNoritoFields(proofFields[3]);
  proofBoxFields[0] = kagemushaNoritoString(proofBackend);
  proofFields[3] = Buffer.concat(
    proofBoxFields.map((field) => kagemushaNoritoField(field)),
  );
  bundleFields[1] = Buffer.concat(proofFields.map((field) => kagemushaNoritoField(field)));
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithProofBoxBackendAndEmptyProofBytes(proofBackend) {
  const bundleFields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("init_bundle")),
  );
  const proofFields = kagemushaReadNoritoFields(bundleFields[1]);
  const proofBoxFields = kagemushaReadNoritoFields(proofFields[3]);
  proofBoxFields[0] = kagemushaNoritoString(proofBackend);
  proofBoxFields[1] = kagemushaNoritoByteVec(Buffer.alloc(0));
  proofFields[3] = Buffer.concat(
    proofBoxFields.map((field) => kagemushaNoritoField(field)),
  );
  bundleFields[1] = Buffer.concat(proofFields.map((field) => kagemushaNoritoField(field)));
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithTrailingVerifierKeyIdField() {
  const bundleFields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("init_bundle")),
  );
  const proofFields = kagemushaReadNoritoFields(bundleFields[1]);
  const verifierKeyIdFields = kagemushaReadNoritoFields(proofFields[0]);
  verifierKeyIdFields.push(kagemushaNoritoString("ignored-extra-verifier-key-field"));
  proofFields[0] = Buffer.concat(
    verifierKeyIdFields.map((field) => kagemushaNoritoField(field)),
  );
  bundleFields[1] = Buffer.concat(proofFields.map((field) => kagemushaNoritoField(field)));
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithTrailingRecursiveProofField() {
  const bundleFields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("init_bundle")),
  );
  const proofFields = kagemushaReadNoritoFields(bundleFields[1]);
  proofFields.push(kagemushaNoritoString("ignored-extra-recursive-proof-field"));
  bundleFields[1] = Buffer.concat(proofFields.map((field) => kagemushaNoritoField(field)));
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithTrailingProofBoxField() {
  const bundleFields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("init_bundle")),
  );
  const proofFields = kagemushaReadNoritoFields(bundleFields[1]);
  const proofBoxFields = kagemushaReadNoritoFields(proofFields[3]);
  proofBoxFields.push(kagemushaNoritoString("ignored-extra-proof-box-field"));
  proofFields[3] = Buffer.concat(
    proofBoxFields.map((field) => kagemushaNoritoField(field)),
  );
  bundleFields[1] = Buffer.concat(proofFields.map((field) => kagemushaNoritoField(field)));
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithEmptyProofBytes() {
  const bundleFields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("init_bundle")),
  );
  const proofFields = kagemushaReadNoritoFields(bundleFields[1]);
  const proofBoxFields = kagemushaReadNoritoFields(proofFields[3]);
  proofBoxFields[1] = kagemushaNoritoByteVec(Buffer.alloc(0));
  proofFields[3] = Buffer.concat(
    proofBoxFields.map((field) => kagemushaNoritoField(field)),
  );
  bundleFields[1] = Buffer.concat(proofFields.map((field) => kagemushaNoritoField(field)));
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithEmptyProofPublicInputs() {
  const bundleFields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("init_bundle")),
  );
  const proofFields = kagemushaReadNoritoFields(bundleFields[1]);
  proofFields[1] = Buffer.alloc(0);
  bundleFields[1] = Buffer.concat(proofFields.map((field) => kagemushaNoritoField(field)));
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithZeroProofPublicInputsHash() {
  return recursiveSpendBundleWithProofPublicInputsHash(Buffer.alloc(32));
}

function recursiveSpendBundleWithProofPublicInputsHash(replacement) {
  const bundleFields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("init_bundle")),
  );
  const proofFields = kagemushaReadNoritoFields(bundleFields[1]);
  proofFields[2] = replacement;
  bundleFields[1] = Buffer.concat(proofFields.map((field) => kagemushaNoritoField(field)));
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithMismatchedProofPublicInputsHash() {
  const bundleFields = kagemushaReadNoritoFields(
    kagemushaArchivePayload(sharedRecursiveSpendAbi6Archive("init_bundle")),
  );
  const proofFields = kagemushaReadNoritoFields(bundleFields[1]);
  const mismatchedHash = Buffer.from(proofFields[2]);
  mismatchedHash[0] ^= 0x01;
  proofFields[2] = mismatchedHash;
  bundleFields[1] = Buffer.concat(proofFields.map((field) => kagemushaNoritoField(field)));
  return privacyNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendVerifierRecord() {
  return buildKagemushaRecursiveSpendVerifierRecordRef({
    verifierKeyId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    recordBytes: syntheticKagemushaArchive(KAGEMUSHA_VERIFYING_KEY_RECORD_WIRE_NAME, 0x66),
  });
}

const PRIVACY_PACKAGE_PRODUCTION_GATE_VERSION = "privacy-production-gate-v1";
const PRIVACY_PACKAGE_PRODUCTION_REVIEW_SCOPE_VERSION =
  "privacy-production-review-scope-v1";
const PRIVACY_PACKAGE_PRODUCTION_EVIDENCE_REGISTRY_VERSION =
  "iroha-privacy-production-evidence-registry-v1";
const PRIVACY_PACKAGE_PRODUCTION_SDK_SURFACES = Object.freeze([
  "rust_core",
  "ffi",
  "python",
  "javascript",
  "java_android",
  "kotlin",
  "swift",
  "csharp",
]);
const PRIVACY_PACKAGE_PRODUCTION_SDK_ARTIFACT_KINDS = Object.freeze([
  "types",
  "validation_rules",
  "error_codes",
  "golden_vectors",
]);
const PRIVACY_PACKAGE_REVIEW_SIGNATURE_PREFIX = "ed25519:";

function privacyPackageProductionArtifact(label) {
  return {
    label,
    uri: `sha256:${createHash("sha256").update(label).digest("hex")}`,
  };
}

function privacyPackageProductionReviewSignature(label) {
  return `${PRIVACY_PACKAGE_REVIEW_SIGNATURE_PREFIX}${createHash("sha512")
    .update(label)
    .digest("hex")}`;
}

function privacyPackageProductionEntrypoints(descriptor) {
  const entrypoints = [];
  for (const entrypoint of [
    ...(descriptor.sdkEntrypoints ?? []),
    ...(descriptor.plannedSdkEntrypoints ?? []),
  ]) {
    if (
      typeof entrypoint !== "string" ||
      entrypoint.includes("DevProofFixture") ||
      entrypoint.endsWith("Locally")
    ) {
      continue;
    }
    if (!entrypoints.includes(entrypoint)) {
      entrypoints.push(entrypoint);
    }
  }
  return entrypoints;
}

function privacyPackageProductionSdkExports(entrypoints) {
  return Object.fromEntries(
    PRIVACY_PACKAGE_PRODUCTION_SDK_SURFACES.map((surface) => [
      surface,
      [...entrypoints],
    ]),
  );
}

function privacyPackageProductionSdkParityArtifacts(descriptor) {
  return Object.fromEntries(
    PRIVACY_PACKAGE_PRODUCTION_SDK_ARTIFACT_KINDS.map((kind) => [
      kind,
      Object.fromEntries(
        PRIVACY_PACKAGE_PRODUCTION_SDK_SURFACES.map((surface) => [
          surface,
          privacyPackageProductionArtifact(
            `${descriptor.id}-${surface}-${kind}-sdk-parity`,
          ),
        ]),
      ),
    ]),
  );
}

function privacyPackageProductionEvidenceRow(
  descriptor,
  { chainId, localnetRunId },
) {
  const entrypoints = privacyPackageProductionEntrypoints(descriptor);
  const fuzzArtifact = privacyPackageProductionArtifact(
    `${descriptor.id}-package-fuzz`,
  );
  const performanceArtifact = privacyPackageProductionArtifact(
    `${descriptor.id}-package-performance`,
  );
  return {
    version: PRIVACY_PACKAGE_PRODUCTION_GATE_VERSION,
    coveredAlgorithmId: descriptor.id,
    chainId,
    reviewerIdentity: "package-reviewer@internal.example",
    reviewArtifact: {
      ...privacyPackageProductionArtifact(`${descriptor.id}-package-review`),
      signature: privacyPackageProductionReviewSignature(
        `${descriptor.id}-package-review-signature`,
      ),
    },
    verifierKeyId: descriptor.verifierKeyId,
    proofFamily: descriptor.proofFamily,
    publicInputsSchema: descriptor.publicInputsSchema,
    sdkEntrypoints: privacyPackageProductionSdkExports(entrypoints),
    sdkExports: privacyPackageProductionSdkExports(entrypoints),
    sdkParityArtifacts: privacyPackageProductionSdkParityArtifacts(descriptor),
    requiredState: [...(descriptor.requiredState ?? [])],
    reviewScope: {
      version: PRIVACY_PACKAGE_PRODUCTION_REVIEW_SCOPE_VERSION,
      algorithmId: descriptor.id,
      chainId,
      verifierKeyId: descriptor.verifierKeyId,
      proofFamily: descriptor.proofFamily,
      publicInputsSchema: descriptor.publicInputsSchema,
      sdkEntrypoints: [...entrypoints],
      requiredState: [...(descriptor.requiredState ?? [])],
      fuzzArtifactHash: fuzzArtifact.uri,
      performanceArtifactHash: performanceArtifact.uri,
      localnetRunId,
    },
    fuzzResults: {
      passed: true,
      artifact: fuzzArtifact,
    },
    performanceResults: {
      passed: true,
      artifact: performanceArtifact,
    },
    localnetRunId,
    localnetAcceptance: {
      runId: localnetRunId,
      target: "localnet",
      peerCount: 4,
      peerIds: [
        "package-privacy-peer-1@localnet",
        "package-privacy-peer-2@localnet",
        "package-privacy-peer-3@localnet",
        "package-privacy-peer-4@localnet",
      ],
      chainId,
      smokePassed: true,
      smokeTxHash: privacyPackageProductionArtifact(
        `${descriptor.id}-package-localnet-smoke`,
      ).uri,
      replayRejected: true,
      replayRejectionHash: privacyPackageProductionArtifact(
        `${descriptor.id}-package-localnet-replay`,
      ).uri,
      restartPersistenceChecked: true,
      restartReplayRejected: true,
      restartReplayRejectionHash: privacyPackageProductionArtifact(
        `${descriptor.id}-package-localnet-restart-replay`,
      ).uri,
      stateRecoveryPassed: true,
      stateRecoveryHash: privacyPackageProductionArtifact(
        `${descriptor.id}-package-localnet-state-recovery`,
      ).uri,
      lifecyclePassed: true,
      lifecycleShieldTxHash: privacyPackageProductionArtifact(
        `${descriptor.id}-package-localnet-shield`,
      ).uri,
      lifecycleHopProofHash: privacyPackageProductionArtifact(
        `${descriptor.id}-package-localnet-hop`,
      ).uri,
      lifecycleRecursiveInitHash: privacyPackageProductionArtifact(
        `${descriptor.id}-package-localnet-recursive-init`,
      ).uri,
      lifecycleRecursiveInitVerifyHash: privacyPackageProductionArtifact(
        `${descriptor.id}-package-localnet-recursive-init-verify`,
      ).uri,
      lifecycleRecursiveAppendHash: privacyPackageProductionArtifact(
        `${descriptor.id}-package-localnet-recursive-append`,
      ).uri,
      lifecycleRecursiveAppendVerifyHash: privacyPackageProductionArtifact(
        `${descriptor.id}-package-localnet-recursive-append-verify`,
      ).uri,
      lifecycleUnshieldProofHash: privacyPackageProductionArtifact(
        `${descriptor.id}-package-localnet-unshield`,
      ).uri,
      lifecycleRedeemTxHash: privacyPackageProductionArtifact(
        `${descriptor.id}-package-localnet-redeem`,
      ).uri,
    },
    gateEvidence: Object.fromEntries(
      descriptor.productionGate.requiredGates.map((gate) => [
        gate,
        [privacyPackageProductionArtifact(`${descriptor.id}-${gate}-package-gate`)],
      ]),
    ),
  };
}

function privacyPackageProductionEvidenceManifest(
  descriptors,
  {
    chainId = "boi-package-localnet-4p",
    localnetRunId = "boi-package-localnet-4peer-run-2026-06-14",
  } = {},
) {
  return {
    version: PRIVACY_PACKAGE_PRODUCTION_EVIDENCE_REGISTRY_VERSION,
    rows: descriptors.map((descriptor) =>
      privacyPackageProductionEvidenceRow(descriptor, { chainId, localnetRunId }),
    ),
  };
}

function publicSccpSourceExports() {
  return [
    ...SCCP_SOURCE_TEXT.matchAll(
      /export\s+(?:const|function|class)\s+([A-Za-z0-9_]+)/gu,
    ),
  ]
    .map((match) => match[1])
    .filter((name) => name.startsWith("SCCP_") || /Sccp|sccp/u.test(name));
}

function sccpEntrypointExportNames(text) {
  const match = text.match(/export \{([\s\S]*?)\} from "\.\/sccp\.js";/u);
  assert.notEqual(match, null);
  return new Set(
    [...match[1].matchAll(/\b([A-Za-z_][A-Za-z0-9_]*)\b/gu)].map(
      (item) => item[1],
    ),
  );
}

function declarationExportNames() {
  return new Set(
    [
      ...DECLARATIONS_TEXT.matchAll(
        /export\s+(?:const|function|class|interface|type)\s+([A-Za-z0-9_]+)/gu,
      ),
    ].map((match) => match[1]),
  );
}

function declarationInterface(name) {
  const match = DECLARATIONS_TEXT.match(
    new RegExp(
      `export interface ${name}(?:\\s+extends\\s+[^{]+)?\\s*\\{[\\s\\S]*?\\n\\}`,
    ),
  );
  assert.ok(match, `missing declaration interface ${name}`);
  return match[0];
}

function declarationInterfaceOrType(name) {
  const interfaceMatch = DECLARATIONS_TEXT.match(
    new RegExp(
      `export interface ${name}(?:\\s+extends\\s+[^{]+)?\\s*\\{[\\s\\S]*?\\n\\}`,
    ),
  );
  if (interfaceMatch) {
    return interfaceMatch[0];
  }
  const typeMatch = DECLARATIONS_TEXT.match(
    new RegExp(`export type ${name}\\s*=\\s*[\\s\\S]*?;\\n`, "u"),
  );
  assert.ok(typeMatch, `missing declaration interface or type ${name}`);
  return typeMatch[0];
}

function declarationClass(name) {
  const start = DECLARATIONS_TEXT.indexOf(`export class ${name} {`);
  assert.notEqual(start, -1, `missing declaration class ${name}`);
  const end = DECLARATIONS_TEXT.indexOf("\nexport ", start + 1);
  return end === -1
    ? DECLARATIONS_TEXT.slice(start)
    : DECLARATIONS_TEXT.slice(start, end);
}

test("package dist entrypoint imports and emits halfwidth i105 literals", () => {
  assert.equal(typeof submitIvmProvedContractCall, "function");
  assert.equal(typeof submitValidationFeeIvmProvedContractCall, "function");
  assert.equal(typeof validationFeePolicyHash, "function");
  assert.equal(typeof verifySignedValidationFeePolicy, "function");
  const publicKey = deterministicEd25519PublicKey(0x20);
  const address = AccountAddress.fromAccount({ publicKey });
  const literal = address.toI105(0x02f1);

  assert.match(literal, /^sora/u);
  assert.equal(LEGACY_FULLWIDTH_KANA.test(literal), false);
  assert.equal(HALFWIDTH_KANA.test(literal), true);
});
test("package dist offline cash lifecycle rejects malformed identity, time, issuer key, and ABI gates", () => {
  const ISSUER_PUBLIC_KEY_BASE64 = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8";
  const ISSUER_PUBLIC_KEY_BASE64URL = "__________________________________________8";
  const SHORT_ISSUER_PUBLIC_KEY_BASE64 = "q6urq6urq6urq6urq6urq6urq6urq6urq6urq6urqw";
  const LONG_ISSUER_PUBLIC_KEY_BASE64 = "zc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3N";

  assert.equal(
    assertOfflineCashConfigurationSnapshotUsable(
      {
        chainId: "00000042",
        assetDefinitionId: "pkr#sbp",
        offlinePaymentsEnabled: true,
        issuerPublicKeyBase64: ISSUER_PUBLIC_KEY_BASE64,
        nativeBridgeAbiVersion: 7,
        artifactSetId: "artifact-set",
        circuitId: "kagemusha-recursive-compact-v1",
        createdAtMs: 100,
      },
      { nowMs: 999, requiredNativeBridgeAbiVersion: 7 },
    ),
    true,
  );
  assert.equal(
    assertOfflineCashConfigurationSnapshotUsable(
      {
        chainId: "00000042",
        assetDefinitionId: "pkr#sbp",
        offlinePaymentsEnabled: true,
        issuerPublicKeyBase64: ISSUER_PUBLIC_KEY_BASE64URL,
        nativeBridgeAbiVersion: 7,
        artifactSetId: "artifact-set",
        circuitId: "kagemusha-recursive-compact-v1",
        createdAtMs: 100,
      },
      { nowMs: 999, requiredNativeBridgeAbiVersion: 7 },
    ),
    true,
  );

  for (const [fieldName, value] of [
    ["chainId", ""],
    ["chainId", " 00000042"],
    ["assetDefinitionId", "pkr sbp"],
    ["assetDefinitionId", "pkr#sbp\u2603"],
    ["artifactSetId", "artifact set"],
    ["circuitId", "kagemusha-recursive-compact-v1\n"],
  ]) {
    assert.throws(
      () =>
        assertOfflineCashConfigurationSnapshotUsable(
          {
            chainId: "00000042",
            assetDefinitionId: "pkr#sbp",
            offlinePaymentsEnabled: true,
            issuerPublicKeyBase64: ISSUER_PUBLIC_KEY_BASE64,
            nativeBridgeAbiVersion: 7,
            artifactSetId: "artifact-set",
            circuitId: "kagemusha-recursive-compact-v1",
            createdAtMs: 100,
            [fieldName]: value,
          },
          { nowMs: 200, requiredNativeBridgeAbiVersion: 7 },
        ),
      error =>
        error instanceof OfflineCashConfigurationSnapshotError &&
        error.code === "malformed_snapshot" &&
        error.message.includes(fieldName),
    );
  }

  for (const [fieldName, value] of [
    ["createdAtMs", undefined],
    ["createdAtMs", -1],
    ["createdAtMs", 100.5],
    ["createdAtMs", Number.MAX_SAFE_INTEGER + 1],
    ["createdAtMs", true],
    ["expiresAtMs", -1],
    ["expiresAtMs", 100.5],
    ["expiresAtMs", Number.MAX_SAFE_INTEGER + 1],
    ["expiresAtMs", true],
    ["expiresAtMs", 100],
  ]) {
    assert.throws(
      () =>
        assertOfflineCashConfigurationSnapshotUsable(
          {
            chainId: "00000042",
            assetDefinitionId: "pkr#sbp",
            offlinePaymentsEnabled: true,
            issuerPublicKeyBase64: ISSUER_PUBLIC_KEY_BASE64,
            nativeBridgeAbiVersion: 7,
            createdAtMs: 100,
            expiresAtMs: 1_000,
            [fieldName]: value,
          },
          { nowMs: 200, requiredNativeBridgeAbiVersion: 7 },
        ),
      error =>
        error instanceof OfflineCashConfigurationSnapshotError &&
        error.code === "malformed_snapshot" &&
        error.message.includes(fieldName),
    );
  }

  for (const nowMs of [-1, 999.5, Number.MAX_SAFE_INTEGER + 1, true]) {
    assert.throws(
      () =>
        assertOfflineCashConfigurationSnapshotUsable(
          {
            chainId: "00000042",
            assetDefinitionId: "pkr#sbp",
            offlinePaymentsEnabled: true,
            issuerPublicKeyBase64: ISSUER_PUBLIC_KEY_BASE64,
            nativeBridgeAbiVersion: 7,
            createdAtMs: 100,
          },
          { nowMs, requiredNativeBridgeAbiVersion: 7 },
        ),
      error =>
        error instanceof OfflineCashConfigurationSnapshotError &&
        error.code === "malformed_snapshot" &&
        error.message.includes("nowMs"),
    );
  }

  for (const offlinePaymentsEnabled of [false, "false", "true", 1]) {
    assert.throws(
      () =>
        assertOfflineCashConfigurationSnapshotUsable(
          {
            chainId: "00000042",
            assetDefinitionId: "pkr#sbp",
            offlinePaymentsEnabled,
            issuerPublicKeyBase64: ISSUER_PUBLIC_KEY_BASE64,
            nativeBridgeAbiVersion: 7,
            createdAtMs: 100,
          },
          { nowMs: 200, requiredNativeBridgeAbiVersion: 7 },
        ),
      error =>
        error instanceof OfflineCashConfigurationSnapshotError &&
        error.code === "offline_payments_disabled",
    );
  }

  for (const nativeBridgeAbiVersion of [0, -1, 7.5]) {
    assert.throws(
      () =>
        assertOfflineCashConfigurationSnapshotUsable(
          {
            chainId: "00000042",
            assetDefinitionId: "pkr#sbp",
            offlinePaymentsEnabled: true,
            issuerPublicKeyBase64: ISSUER_PUBLIC_KEY_BASE64,
            nativeBridgeAbiVersion,
            createdAtMs: 100,
          },
          { nowMs: 200, requiredNativeBridgeAbiVersion: 7 },
        ),
      error =>
        error instanceof OfflineCashConfigurationSnapshotError &&
        error.code === "malformed_snapshot",
    );
  }

  for (const requiredNativeBridgeAbiVersion of [0, -1, 7.5]) {
    assert.throws(
      () =>
        assertOfflineCashConfigurationSnapshotUsable(
          {
            chainId: "00000042",
            assetDefinitionId: "pkr#sbp",
            offlinePaymentsEnabled: true,
            issuerPublicKeyBase64: ISSUER_PUBLIC_KEY_BASE64,
            nativeBridgeAbiVersion: 7,
            createdAtMs: 100,
          },
          { nowMs: 200, requiredNativeBridgeAbiVersion },
        ),
      error =>
        error instanceof OfflineCashConfigurationSnapshotError &&
        error.code === "malformed_snapshot",
    );
  }

  for (const issuerPublicKeyBase64 of [
    "",
    ` ${ISSUER_PUBLIC_KEY_BASE64}`,
    `${ISSUER_PUBLIC_KEY_BASE64} `,
    "not base64",
    "!!!!",
    `${ISSUER_PUBLIC_KEY_BASE64}=`,
    SHORT_ISSUER_PUBLIC_KEY_BASE64,
    LONG_ISSUER_PUBLIC_KEY_BASE64,
    "issuer-key\n",
    "issuer-key\u2603",
  ]) {
    assert.throws(
      () =>
        assertOfflineCashConfigurationSnapshotUsable(
          {
            chainId: "00000042",
            assetDefinitionId: "pkr#sbp",
            offlinePaymentsEnabled: true,
            issuerPublicKeyBase64,
            nativeBridgeAbiVersion: 7,
            createdAtMs: 100,
          },
          { nowMs: 200, requiredNativeBridgeAbiVersion: 7 },
        ),
      error =>
        error instanceof OfflineCashConfigurationSnapshotError &&
        error.code === "missing_issuer_public_key",
    );
  }
});

test("package SCCP entrypoint and declarations cover public source exports", () => {
  const sourceExports = publicSccpSourceExports();
  const sourceEntrypointExports = sccpEntrypointExportNames(INDEX_SOURCE_TEXT);
  const distEntrypointExports = sccpEntrypointExportNames(DIST_INDEX_TEXT);
  const declarationExports = declarationExportNames();

  assert.deepEqual(
    sourceExports.filter((name) => !sourceEntrypointExports.has(name)),
    [],
  );
  assert.deepEqual(
    sourceExports.filter((name) => !distEntrypointExports.has(name)),
    [],
  );
  assert.deepEqual(
    sourceExports.filter((name) => !declarationExports.has(name)),
    [],
  );
});

test("package SCCP exports reject retired and diagnostic helper surfaces", () => {
  const declarationExports = declarationExportNames();
  const forbidden = [
    "SCCP_DOMAIN_SOL",
    "SCCP_DOMAIN_TON",
    "SCCP_CODEC_SOLANA_PUBKEY32",
    "SCCP_CODEC_TON_ACCOUNT36",
    "SCCP_STARK_FRI_PROOF_FAMILY_V1",
    "SCCP_BSC_GROTH16_PROOF_SELF_TEST_SCHEMA_V1",
    "normalizeSccpProofManifests",
    "normalizeSccpSourceAdapterEngineDeployment",
    "buildEvmSccpProofRequest",
    "buildTronSccpProofRequest",
    "sccpBuildTonMessageBundleSourceProofWithDeployment",
    "sccpTonFixtureValidatorSetHash",
  ];
  const packageTexts = [
    ["src/index.js", INDEX_SOURCE_TEXT],
    ["src/sccp.js", SCCP_SOURCE_TEXT],
    ["dist/index.js", DIST_INDEX_TEXT],
    ["dist/sccp.js", DIST_SCCP_TEXT],
  ];

  for (const name of forbidden) {
    assert.equal(
      declarationExports.has(name),
      false,
      `${name} must not be declared as a public API`,
    );
    for (const [label, text] of packageTexts) {
      assert.doesNotMatch(
        text,
        new RegExp(`\\b${name}\\b`, "u"),
        `${name} must not be exported from ${label}`,
      );
    }
  }
});

test("package dist entrypoint exports Kagemusha recursive spend helpers", () => {
  const declarationExports = declarationExportNames();
  const expected = [
    "KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1",
    "KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION",
    "KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1",
    "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT",
    "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT",
    "KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION",
    "KAGEMUSHA_RECURSIVE_SPEND_TOPUP_REQUIRED_NATIVE_BRIDGE_ABI_VERSION",
    "KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND",
    "KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1",
    "KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1",
    "KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES",
    "KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES",
    "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
    "KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN",
    "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN",
    "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN",
    "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1",
    "KAGEMUSHA_RECURSIVE_TOPUP_REQUEST_WIRE_NAME",
    "KAGEMUSHA_RECURSIVE_SPEND_INIT_REQUEST_WIRE_NAME",
    "KAGEMUSHA_RECURSIVE_SPEND_APPEND_REQUEST_WIRE_NAME",
    "KAGEMUSHA_RECURSIVE_SPEND_VERIFY_REQUEST_WIRE_NAME",
    "KAGEMUSHA_RECURSIVE_SPEND_VERIFY_RESULT_WIRE_NAME",
    "KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_WIRE_NAME",
    "KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME",
    "KAGEMUSHA_RECURSIVE_SPEND_RECORD_BUNDLE_WIRE_NAME",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME",
    "KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME",
    "KAGEMUSHA_VERIFYING_KEY_RECORD_WIRE_NAME",
    "KagemushaRecursiveSpendRequestCodecError",
    "preferredKagemushaOfflineSpendMode",
    "isKagemushaSpendAgainMode",
    "canRedeemKagemushaRecursiveSpendWitnessless",
    "requiresKagemushaRecursiveSpendLineageWitnessForRedeem",
    "canAppendKagemushaRecursiveSpendWitnesslessLineage",
    "isKagemushaRecursiveSpendLineageProofCircuitId",
    "isKagemushaRecursiveSpendLineageAppendOutputCircuitId",
    "isSupportedKagemushaRecursiveSpendLineageKeyArtifactOpeningLen",
    "kagemushaRecursiveSpendLineageKeyArtifactsForInit",
    "kagemushaRecursiveSpendLineageKeyArtifactsForAppend",
    "kagemushaRecursiveSpendLineageKeyArtifacts",
    "validateKagemushaRecursiveSpendLineageKeyArtifacts",
    "requiresKagemushaRecursiveSpendLineageKeyArtifactsForInit",
    "requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput",
    "normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId",
    "isSupportedKagemushaRecursiveSpendAppendOutputProofCircuitId",
    "isSupportedKagemushaRecursiveSpendAppendProofTransition",
    "isSupportedKagemushaRecursiveSpendPreviousProofCircuitId",
    "requiresKagemushaRecursiveSpendPreviousLineageVerifierRecordForAppend",
    "preferredKagemushaRecursiveSpendAppendOutputProofCircuitId",
    "canProveKagemushaRecursiveSpendAppendOutputProofCircuitId",
    "canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId",
    "requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend",
    "isKagemushaCompactPaymentTokenNativeAvailable",
    "isKagemushaRecursiveAggregationProofBundleNativeAvailable",
    "isKagemushaPallasOpenEnvelopeBuilderNativeAvailable",
    "isKagemushaRecursiveCompactPaymentTokenNativeAvailable",
    "isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable",
    "isKagemushaRecursiveCompactUnavailable",
    "isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable",
    "isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable",
    "isKagemushaRecursiveSpendNativeAvailable",
    "isKagemushaRecursiveSpendTopUpNativeAvailable",
    "kagemushaProveVerifiedCompactPaymentTokenWithRecords",
    "kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes",
    "kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes",
    "kagemushaBuildPallasOpenEnvelopesArchive",
    "kagemushaBuildPreviousProofOpenEnvelopesArchive",
    "kagemushaVerifyRecursiveCompactPaymentToken",
    "kagemushaRecursiveSpendCompactPaymentTokenFromBundle",
    "kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection",
    "kagemushaRecursiveSpendInit",
    "kagemushaRecursiveSpendTopUp",
    "kagemushaRecursiveSpendAppend",
    "kagemushaRecursiveSpendTransitionProfileInit",
    "kagemushaRecursiveSpendTransitionProfileAppend",
    "kagemushaRecursiveSpendLineageAppendBoundary",
    "kagemushaRecursiveSpendLineageWitnessFromInitResult",
    "kagemushaRecursiveSpendLineageWitnessAppendResult",
    "kagemushaRecursiveSpendVerify",
    "kagemushaRecursiveSpendRedeem",
    "buildKagemushaRecursiveTopUpTransaction",
    "buildKagemushaRecursiveSpendableNoteDescriptor",
    "buildKagemushaRecursiveSpendVerifierRecordRef",
    "encodeKagemushaRecursiveSpendInitRequest",
    "encodeKagemushaRecursiveSpendAppendRequest",
    "encodeKagemushaRecursiveSpendVerifyRequest",
    "encodeKagemushaRecursiveSpendRedeemRequest",
    "decodeKagemushaRecursiveSpendVerifyResult",
    "decodeKagemushaRecursiveSpendBundle",
    "kagemushaRecursiveSpendInitTyped",
    "kagemushaRecursiveSpendTopUpTyped",
    "kagemushaRecursiveSpendAppendTyped",
    "kagemushaRecursiveSpendVerifyTyped",
    "kagemushaRecursiveSpendRedeemTyped",
  ];

  for (const name of expected) {
    assert.match(DIST_INDEX_TEXT, new RegExp(`\\b${name}\\b`, "u"));
    assert.ok(
      declarationExports.has(name),
      `missing declaration export ${name}`,
    );
  }
  assert.equal(KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1, "recursive_spend_v1");
  assert.ok(
    !declarationExports.has("KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1"),
    "compact projection must not be exported as a first-release spend mode",
  );
  assert.ok(
    !declarationExports.has("KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V2"),
    "internal V2 artifact tag must not be exported as a product mode",
  );
  assert.ok(
    !declarationExports.has("KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1"),
    "checked-prefold must not be exported as a first-release spend mode",
  );
  assert.doesNotMatch(
    DIST_INDEX_TEXT,
    /\bKAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1\b/u,
  );
  assert.doesNotMatch(DIST_INDEX_TEXT, /\bchecked_prefold_v1\b/u);
  assert.equal(
    KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION,
    7,
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1,
    "kagemusha-recursive-compact-v1",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT,
    "recursive compact Kagemusha payment-token multi-hop proving requires the append verifier batch",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT,
    "recursive compact Kagemusha multi-hop payment-token proving requires the append verifier batch",
  );
  assert.equal(
    isKagemushaRecursiveCompactUnavailable(
      new Error(KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT),
    ),
    true,
  );
  assert.equal(
    isKagemushaRecursiveCompactUnavailable(
      "recursive compact proof composition unavailable",
    ),
    false,
  );
  assert.equal(KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION, 18);
  assert.equal(KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND, "halo2/ipa");
  assert.equal(
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    "kagemusha-recursive-aggregation-v1",
  );
  assert.equal(
    GENERIC_LINEAGE_FAMILY_ID,
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
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1,
    false,
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1,
    1,
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES,
    8 * 1024 * 1024,
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES,
    128,
  );
  assert.equal(KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES, 256 * 1024 * 1024);
  assert.match(
    DECLARATIONS_TEXT,
    /export const KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES: 268435456;/u,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export const KAGEMUSHA_RECURSIVE_TOPUP_REQUEST_WIRE_NAME: "iroha_data_model::offline::model::KagemushaRecursiveSpendTopUpRequestV1";/u,
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN,
    "iroha:kagemusha:v1:recursive-spend-accumulator",
  );
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
    KAGEMUSHA_RECURSIVE_TOPUP_REQUEST_WIRE_NAME,
    "iroha_data_model::offline::model::KagemushaRecursiveSpendTopUpRequestV1",
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export const KAGEMUSHA_RECURSIVE_TOPUP_REQUEST_WIRE_NAME: "iroha_data_model::offline::model::KagemushaRecursiveSpendTopUpRequestV1";/u,
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
    false,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendAppendProofTransition(
      GENERIC_LINEAGE_FAMILY_ID,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    ),
    false,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendAppendProofTransition(
      GENERIC_LINEAGE_FAMILY_ID,
      GENERIC_LINEAGE_FAMILY_ID,
    ),
    false,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendAppendProofTransition(
      GENERIC_LINEAGE_FAMILY_ID,
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    ),
    false,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendAppendProofTransition(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      GENERIC_LINEAGE_FAMILY_ID,
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
  assert.strictEqual(
    normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(),
    undefined,
  );
  assert.strictEqual(
    normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(null),
    null,
  );
  assert.strictEqual(
    normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(""),
    "",
  );
  assert.equal(
    normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(
      GENERIC_LINEAGE_FAMILY_ID,
    ),
    GENERIC_LINEAGE_FAMILY_ID,
  );
  assert.equal(
    normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(
      "unknown-kagemusha-recursive-spend-circuit",
    ),
    "unknown-kagemusha-recursive-spend-circuit",
  );
  for (const circuitId of [
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
  ]) {
    assert.equal(
      isSupportedKagemushaRecursiveSpendAppendOutputProofCircuitId(circuitId),
      true,
    );
  }
  for (const circuitId of [
    GENERIC_LINEAGE_FAMILY_ID,
  ]) {
    assert.equal(
      isSupportedKagemushaRecursiveSpendAppendOutputProofCircuitId(circuitId),
      false,
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
  assert.equal(
    isKagemushaRecursiveSpendLineageProofCircuitId(GENERIC_LINEAGE_FAMILY_ID),
    false,
  );
  for (const circuitId of [
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
  ]) {
    assert.equal(
      isKagemushaRecursiveSpendLineageProofCircuitId(circuitId),
      true,
    );
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
    requiresKagemushaRecursiveSpendLineageKeyArtifactsForInit(),
    false,
  );
  for (const openingLen of [2, 4, 8, 16, 32, 64, 128]) {
    assert.equal(
      isSupportedKagemushaRecursiveSpendLineageKeyArtifactOpeningLen(
        openingLen,
      ),
      true,
    );
    assert.equal(isSupportedKagemushaRecursiveSpendLineageKeyArtifactOpeningLen(openingLen), true);
  }
  for (const openingLen of [0, 1, 3, 65, 129, -2, 2.5, Number.NaN, "2", true]) {
    assert.equal(
      isSupportedKagemushaRecursiveSpendLineageKeyArtifactOpeningLen(
        openingLen,
      ),
      false,
    );
  }
  const verifierKey = kagemushaLineageVerifierKey(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    0xe7,
  );
  const provingKey = kagemushaLineageProvingKeyArchive(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    verifierKey,
    0xe8,
  );
  const expectedVerifierKey = Buffer.from(verifierKey);
  const expectedProvingKey = Buffer.from(provingKey);
  const initArtifacts = kagemushaRecursiveSpendLineageKeyArtifactsForInit(
    128,
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
    verifierKey,
    provingKey,
  );
  verifierKey.fill(0);
  provingKey.fill(0);
  assert.equal(
    initArtifacts.proofCircuitId,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(initArtifacts.verifierOpeningLen, 128);
  assert.equal(initArtifacts.lineageVerifierKeyBackend, "halo2/ipa");
  assert.deepEqual(initArtifacts.lineageVerifierKey, expectedVerifierKey);
  assert.deepEqual(initArtifacts.lineageProvingKeyArchive, expectedProvingKey);
  assert.equal(initArtifacts.isInitArtifact, true);
  assert.equal(initArtifacts.isAppendArtifact, false);
  const appendVerifierKey = kagemushaLineageVerifierKey(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    0xa7,
  );
  const appendProvingKey = kagemushaLineageProvingKeyArchive(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    appendVerifierKey,
    0xa8,
  );
  const appendArtifacts = kagemushaRecursiveSpendLineageKeyArtifactsForAppend(
    64,
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
    appendVerifierKey,
    appendProvingKey,
  );
  assert.equal(
    appendArtifacts.proofCircuitId,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(appendArtifacts.isInitArtifact, false);
  assert.equal(appendArtifacts.isAppendArtifact, true);
  const genericArtifacts = kagemushaRecursiveSpendLineageKeyArtifacts(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    2,
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
    appendVerifierKey,
    appendProvingKey,
  );
  assert.equal(genericArtifacts.verifierOpeningLen, 2);
  assert.deepEqual(
    validateKagemushaRecursiveSpendLineageKeyArtifacts(genericArtifacts),
    genericArtifacts,
  );
  const directVerifierKey = kagemushaLineageVerifierKey(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    0x11,
  );
  const directProvingKey = kagemushaLineageProvingKeyArchive(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    directVerifierKey,
    0x12,
  );
  const directArtifacts = validateKagemushaRecursiveSpendLineageKeyArtifacts({
    ...initArtifacts,
    lineageVerifierKey: directVerifierKey,
    lineageProvingKeyArchive: directProvingKey,
  });
  directVerifierKey.fill(0);
  directProvingKey.fill(0);
  assert.deepEqual(
    directArtifacts.lineageVerifierKey,
    kagemushaLineageVerifierKey(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
      0x11,
    ),
  );
  assert.deepEqual(
    directArtifacts.lineageProvingKeyArchive,
    kagemushaLineageProvingKeyArchive(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
      kagemushaLineageVerifierKey(
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        0x11,
      ),
      0x12,
    ),
  );
  const oldHashProvingKey = Buffer.from(expectedProvingKey);
  OLD_KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH.copy(
    oldHashProvingKey,
    6,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        expectedVerifierKey,
        oldHashProvingKey,
      ),
    /lineage_proving_key_archive/,
  );
  const exposedVerifierKey = directArtifacts.lineageVerifierKey;
  const exposedProvingKey = directArtifacts.lineageProvingKeyArchive;
  exposedVerifierKey[0] = 0;
  exposedProvingKey[0] = 0;
  assert.equal(directArtifacts.lineageVerifierKey[0], 0x5a);
  assert.equal(directArtifacts.lineageProvingKeyArchive[0], 0x4e);
  assert.notStrictEqual(
    directArtifacts.lineageVerifierKey,
    directArtifacts.lineageVerifierKey,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        appendVerifierKey,
        expectedProvingKey,
      ),
    /lineage_verifier_key/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        expectedVerifierKey,
        appendProvingKey,
      ),
    /lineage_proving_key_archive/,
  );
  for (const malformed of [
    [null, /lineage_key_artifacts/],
    [
      {
        ...initArtifacts,
        proofCircuitId: GENERIC_LINEAGE_FAMILY_ID,
      },
      /proof_circuit_id/,
    ],
    [
      {
        ...initArtifacts,
        proofCircuitId: "unknown-kagemusha-recursive-spend-circuit",
      },
      /proof_circuit_id/,
    ],
    [{ ...initArtifacts, verifierOpeningLen: 3 }, /verifier_opening_len/],
    [{ ...initArtifacts, verifierOpeningLen: true }, /verifier_opening_len/],
    [
      { ...initArtifacts, lineageVerifierKeyBackend: "halo2/kzg" },
      /lineage_verifier_key/,
    ],
    [
      { ...initArtifacts, lineageVerifierKeyBackend: " halo2/ipa" },
      /lineage_verifier_key/,
    ],
    [
      { ...initArtifacts, lineageVerifierKeyBackend: "halo2/ipa " },
      /lineage_verifier_key/,
    ],
    [
      { ...initArtifacts, lineageVerifierKeyBackend: "HALO2/IPA" },
      /lineage_verifier_key/,
    ],
    [
      { ...initArtifacts, lineageVerifierKey: Buffer.alloc(0) },
      /lineage_verifier_key/,
    ],
    [
      { ...initArtifacts, lineageProvingKeyArchive: Buffer.alloc(0) },
      /lineage_proving_key_archive/,
    ],
    [
      { ...initArtifacts, lineageVerifierKey: "not-bytes" },
      /lineage_verifier_key/,
    ],
    [
      { ...initArtifacts, lineageProvingKeyArchive: "not-bytes" },
      /lineage_proving_key_archive/,
    ],
  ]) {
    assert.throws(
      () => validateKagemushaRecursiveSpendLineageKeyArtifacts(malformed[0]),
      malformed[1],
    );
  }
  assert.equal(
    requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  for (const outputCircuitId of [
    undefined,
    null,
    "",
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    GENERIC_LINEAGE_FAMILY_ID,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    "unknown-kagemusha-recursive-spend-circuit",
  ]) {
    assert.equal(
      requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput(
        outputCircuitId,
      ),
      false,
    );
  }
  assert.equal(
    isSupportedKagemushaRecursiveSpendPreviousProofCircuitId(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
  assert.equal(
    isSupportedKagemushaRecursiveSpendPreviousProofCircuitId(
      GENERIC_LINEAGE_FAMILY_ID,
    ),
    false,
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
      GENERIC_LINEAGE_FAMILY_ID,
    ),
    false,
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
    canRedeemKagemushaRecursiveSpendWitnessless(
      GENERIC_LINEAGE_FAMILY_ID,
      1,
    ),
    false,
  );
  assert.equal(
    canRedeemKagemushaRecursiveSpendWitnessless(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    false,
  );
  assert.equal(
    canRedeemKagemushaRecursiveSpendWitnessless(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
      2,
    ),
    false,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendLineageWitnessForRedeem(
      GENERIC_LINEAGE_FAMILY_ID,
      1,
    ),
    true,
  );
  assert.equal(
    canRedeemKagemushaRecursiveSpendWitnessless(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    false,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendLineageWitnessForRedeem(
      GENERIC_LINEAGE_FAMILY_ID,
      2,
    ),
    true,
  );
  for (const [circuitId, hopCount] of [
    [GENERIC_LINEAGE_FAMILY_ID, -1],
    [
      GENERIC_LINEAGE_FAMILY_ID,
      Number.MAX_SAFE_INTEGER,
    ],
    [GENERIC_LINEAGE_FAMILY_ID, Number.NaN],
    [
      GENERIC_LINEAGE_FAMILY_ID,
      Number.POSITIVE_INFINITY,
    ],
    [GENERIC_LINEAGE_FAMILY_ID, 1n],
    [GENERIC_LINEAGE_FAMILY_ID, new Number(1)],
    [GENERIC_LINEAGE_FAMILY_ID, true],
    [GENERIC_LINEAGE_FAMILY_ID, "1"],
    [undefined, 1],
    [null, 1],
    ["", 1],
    ["unknown-kagemusha-recursive-spend-circuit", Number.MAX_SAFE_INTEGER],
  ]) {
    assert.equal(
      canRedeemKagemushaRecursiveSpendWitnessless(circuitId, hopCount),
      false,
    );
    assert.equal(
      requiresKagemushaRecursiveSpendLineageWitnessForRedeem(
        circuitId,
        hopCount,
      ),
      true,
    );
  }
  for (const circuitId of [
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
  ]) {
    for (const hopCount of [1, 2, 63, 64]) {
      assert.equal(canRedeemKagemushaRecursiveSpendWitnessless(circuitId, hopCount), false);
      assert.equal(
        requiresKagemushaRecursiveSpendLineageWitnessForRedeem(circuitId, hopCount),
        true,
        `${circuitId} hop ${hopCount} must require a record-backed lineage witness`,
      );
    }
  }
  for (const hopCount of [
    Number.NEGATIVE_INFINITY,
    -1,
    0,
    1,
    2,
    63,
    64,
    Number.MAX_SAFE_INTEGER,
    Number.POSITIVE_INFINITY,
    Number.NaN,
    1.5,
    1n,
    new Number(1),
    true,
    false,
    "1",
  ]) {
    assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(hopCount), false);
  }
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(1n), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(new Number(1)), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(true), false);
  assert.equal(
    preferredKagemushaRecursiveSpendAppendOutputProofCircuitId(1),
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(
    preferredKagemushaRecursiveSpendAppendOutputProofCircuitId(63),
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(
    preferredKagemushaRecursiveSpendAppendOutputProofCircuitId(64),
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    "the semantic append circuit remains preferred while lineage transition verification is unavailable",
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
  assert.equal(isSupportedKagemushaRecursiveSpendAppendOutputProofCircuitId(""), false);
  assert.equal(isSupportedKagemushaRecursiveSpendAppendOutputProofCircuitId(null), false);
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(undefined, 1),
    false,
  );
  assert.equal(
    canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
      undefined,
      1,
    ),
    false,
  );
  assert.equal(canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(null, 1), false);
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS - 1,
    ),
    true,
  );
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      0,
    ),
    false,
  );
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
    ),
    false,
  );
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      GENERIC_LINEAGE_FAMILY_ID,
      1,
    ),
    false,
  );
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    false,
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
      GENERIC_LINEAGE_FAMILY_ID,
      63,
    ),
    false,
  );
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      GENERIC_LINEAGE_FAMILY_ID,
      64,
    ),
    false,
  );
  assert.equal(
    canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
      "unknown-kagemusha-recursive-spend-circuit",
      1,
    ),
    false,
  );
  for (const outputProofCircuitId of [undefined, null, ""]) {
    assert.equal(
      canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(outputProofCircuitId, 1),
      false,
    );
  }
  for (const previousHopCount of [
    1.5,
    Number.NaN,
    Number.POSITIVE_INFINITY,
    1n,
    new Number(1),
    true,
    "1",
  ]) {
    assert.equal(
      canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
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
      GENERIC_LINEAGE_FAMILY_ID,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    false,
  );
  assert.equal(
    canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
      "unknown-kagemusha-recursive-spend-circuit",
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    false,
  );
  assert.equal(
    canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      GENERIC_LINEAGE_FAMILY_ID,
      1,
    ),
    false,
    "semantic previous proofs cannot select Reserved-lineage output",
  );
  assert.equal(
    canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
      GENERIC_LINEAGE_FAMILY_ID,
      GENERIC_LINEAGE_FAMILY_ID,
      1,
    ),
    false,
  );
  assert.equal(
    canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    false,
  );
  assert.equal(
    canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      "unknown-kagemusha-recursive-spend-circuit",
      1,
    ),
    false,
  );
  assert.equal(
    canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      0,
    ),
    false,
  );
  for (const previousHopCount of [Number.NaN, 1n, new Number(1)]) {
    assert.equal(
      canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        previousHopCount,
      ),
      false,
    );
  }
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend(
      GENERIC_LINEAGE_FAMILY_ID,
      1,
    ),
    false,
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
      GENERIC_LINEAGE_FAMILY_ID,
      64,
    ),
    false,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend(
      GENERIC_LINEAGE_FAMILY_ID,
      0,
    ),
    false,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend(
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      1,
    ),
    false,
  );
  assert.equal(
    requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend("", 1),
    false,
  );
  for (const previousHopCount of [
    1.5,
    Number.NaN,
    Number.POSITIVE_INFINITY,
    1n,
    new Number(1),
    "1",
  ]) {
    assert.equal(
      requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend(
        GENERIC_LINEAGE_FAMILY_ID,
        previousHopCount,
      ),
      false,
    );
  }
  assert.equal(
    preferredKagemushaOfflineSpendMode(true),
    KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1,
  );
  assert.equal(preferredKagemushaOfflineSpendMode(false), null);
  assert.equal(isKagemushaSpendAgainMode("recursive_spend_v1"), true);
  assert.equal(isKagemushaSpendAgainMode("recursive_spend_v2"), false);
  assert.equal(isKagemushaSpendAgainMode("recursive_compact_v1"), false);
  assert.equal(isKagemushaSpendAgainMode(" recursive_spend_v1"), false);
  assert.equal(isKagemushaSpendAgainMode("RECURSIVE_SPEND_V1"), false);
  assert.equal(isKagemushaSpendAgainMode(null), false);
  assert.throws(
    () => preferredKagemushaOfflineSpendMode(false, true),
    /requires zero arguments or one boolean pastaCycleV3BackendAvailable argument/u,
  );
  assert.equal(
    typeof isKagemushaRecursiveCompactPaymentTokenNativeAvailable(),
    "boolean",
  );
  assert.equal(
    typeof isKagemushaCompactPaymentTokenNativeAvailable(),
    "boolean",
  );
  assert.equal(
    typeof isKagemushaRecursiveAggregationProofBundleNativeAvailable(),
    "boolean",
  );
  assert.equal(
    typeof isKagemushaPallasOpenEnvelopeBuilderNativeAvailable(),
    "boolean",
  );
  assert.equal(typeof isKagemushaRecursiveSpendNativeAvailable(), "boolean");
  assert.equal(typeof isKagemushaRecursiveSpendTopUpNativeAvailable(), "boolean");
  assert.equal(
    typeof isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable(),
    "boolean",
  );
  assert.equal(
    typeof isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable(),
    "boolean",
  );
  assert.equal(
    typeof isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable(),
    "boolean",
  );
  assert.equal(typeof kagemushaVerifyRecursiveCompactPaymentToken, "function");
  assert.equal(typeof kagemushaBuildPallasOpenEnvelopesArchive, "function");
  assert.equal(
    typeof kagemushaBuildPreviousProofOpenEnvelopesArchive,
    "function",
  );
  assert.equal(
    typeof kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection,
    "function",
  );
  assert.throws(
    () =>
      kagemushaProveVerifiedCompactPaymentTokenWithRecords(
        privacyNoritoFrameWithPayload(0x4d),
      ),
    /Kagemusha compact payment-token prover|unavailable in browser-only crypto builds|Native binding required|invalid Kagemusha record bundle archive/,
  );
  assert.throws(
    () =>
      kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
        privacyNoritoFrameWithPayload(0x4e),
        privacyNoritoFrameWithPayload(0x4f),
      ),
    /Kagemusha recursive aggregation proof-bundle prover|unavailable in browser-only crypto builds|Native binding required|invalid Kagemusha record bundle archive/,
  );
  assert.throws(
    () =>
      kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        privacyNoritoFrameWithPayload(0x4a),
        privacyNoritoFrameWithPayload(0x4c),
        privacyNoritoFrameWithPayload(0x4d),
      ),
    /recursive compact Kagemusha payment-token prover|unavailable in browser-only crypto builds|Native binding required|invalid Kagemusha record bundle archive/,
  );
  assert.throws(
    () =>
      kagemushaBuildPallasOpenEnvelopesArchive(
        privacyNoritoFrameWithPayload(0x4e),
      ),
    /Kagemusha Pallas open-envelope builders|unavailable in browser-only crypto builds|Native binding required|invalid Kagemusha record bundle archive/,
  );
  assert.throws(
    () =>
      kagemushaBuildPreviousProofOpenEnvelopesArchive(
        privacyNoritoFrameWithPayload(0x4f),
      ),
    /Kagemusha Pallas open-envelope builders|unavailable in browser-only crypto builds|Native binding required|invalid Kagemusha recursive spend previous bundle archive/,
  );
  assert.throws(
    () =>
      kagemushaVerifyRecursiveCompactPaymentToken(
        privacyNoritoFrameWithPayload(0x4b),
        privacyNoritoFrameWithPayload(0x4e),
      ),
    /recursive compact Kagemusha payment-token verifier|unavailable in browser-only crypto builds|Native binding required|invalid Kagemusha recursive compact payment token archive/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendCompactPaymentTokenFromBundle(
        privacyNoritoFrameWithPayload(0x4c),
      ),
    /recursive spend compact Kagemusha payment-token projection|unavailable in browser-only crypto builds|Native binding required|invalid Kagemusha recursive spend compact-token bundle archive/,
  );
  assert.throws(
    () =>
      kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
        privacyNoritoFrameWithPayload(0x4c),
        privacyNoritoFrameWithPayload(0x4d),
      ),
    /recursive spend compact Kagemusha payment-token projection verifier|unavailable in browser-only crypto builds|Native binding required|invalid Kagemusha recursive spend compact projection token archive/,
  );
  for (const helper of [
    kagemushaRecursiveSpendInit,
    kagemushaRecursiveSpendTopUp,
    kagemushaRecursiveSpendAppend,
    kagemushaRecursiveSpendTransitionProfileInit,
    kagemushaRecursiveSpendTransitionProfileAppend,
    kagemushaRecursiveSpendLineageAppendBoundary,
    kagemushaRecursiveSpendLineageWitnessFromInitResult,
    kagemushaRecursiveSpendLineageWitnessAppendResult,
    kagemushaRecursiveSpendVerify,
    kagemushaRecursiveSpendRedeem,
  ]) {
    assert.equal(typeof helper, "function");
  }
});

test("package dist Kagemusha transaction helpers copy mutable buffers before native calls", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const calls = [];
  const authority = AccountAddress.fromAccount({
    publicKey: Buffer.from(
      "CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03",
      "hex",
    ),
  }).toI105();
  const transferArchive = packageDistKagemushaInstructionArchive(
    "KagemushaTransfer",
    Buffer.from([0x31, 0x32, 0x33]),
  );
  const redeemInstructionArchive = packageDistKagemushaInstructionArchive(
    "RedeemKagemushaRecursive",
    Buffer.from([0x41, 0x42, 0x43]),
  );
  const topUpInstructionArchive = packageDistKagemushaInstructionArchive(
    "TopUpKagemushaRecursive",
    Buffer.from([0x44, 0x45, 0x46]),
  );
  const redeemRequestArchive = Buffer.from([0x51, 0x52, 0x53]);
  const topUpRequestArchive = Buffer.from([0x54, 0x55, 0x56]);
  const privateKey = Buffer.alloc(32, 0x61);
  const mutableTransferArchive = new Uint8Array(transferArchive);
  const mutableRedeemInstructionArchive = new Uint8Array(redeemInstructionArchive);
  const mutableRedeemRequestArchive = new Uint8Array(redeemRequestArchive);
  const mutableTopUpInstructionArchive = new Uint8Array(topUpInstructionArchive);
  const mutableTopUpRequestArchive = new Uint8Array(topUpRequestArchive);
  const mutablePrivateKey = new Uint8Array(privateKey);
  const fakeResult = {
    signed_transaction: Buffer.from([0x71, 0x72]),
    hash: Buffer.alloc(32, 0x73),
  };
  const directInstruction = buildKagemushaInstructionArchiveInstruction({
    type: "KagemushaTransfer",
    instructionArchive: mutableTransferArchive,
  });

  try {
    globalThis.__IROHA_NATIVE_BINDING__ = {
      kagemushaRecursiveSpendTopUp: (requestArchive) => {
        calls.push({
          type: "topup",
          requestArchive,
        });
        return mutableTopUpInstructionArchive;
      },
      kagemushaRecursiveSpendRedeem: (requestArchive) => {
        calls.push({
          type: "redeem",
          requestArchive,
        });
        return mutableRedeemInstructionArchive;
      },
      buildTransaction: (
        chainId,
        transactionAuthority,
        instructions,
        metadataPayload,
        creationTimeMs,
        ttlMs,
        nonce,
        secret,
      ) => {
        calls.push({
          type: "sign",
          chainId,
          authority: transactionAuthority,
          instructions,
          metadataPayload,
          creationTimeMs,
          ttlMs,
          nonce,
          secret,
        });
        return fakeResult;
      },
    };

    buildKagemushaInstructionTransaction({
      chainId: "test-chain",
      authority,
      instruction_type: "KagemushaTransfer",
      instructionArchive: mutableTransferArchive,
      privateKey: mutablePrivateKey,
    });
    buildKagemushaRecursiveRedeemTransaction({
      chainId: "test-chain",
      authority,
      redeemRequestArchive: mutableRedeemRequestArchive,
      privateKey: mutablePrivateKey,
    });
    buildKagemushaRecursiveTopUpTransaction({
      chainId: "test-chain",
      authority,
      topUpRequestArchive: mutableTopUpRequestArchive,
      privateKey: mutablePrivateKey,
    });
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }

  mutableTransferArchive.fill(0xa5);
  mutableRedeemInstructionArchive.fill(0xa5);
  mutableRedeemRequestArchive.fill(0xa5);
  mutableTopUpInstructionArchive.fill(0xa5);
  mutableTopUpRequestArchive.fill(0xa5);
  mutablePrivateKey.fill(0xa5);

  const expectedTransferInstruction = {
    KagemushaInstructionArchive: {
      type: "KagemushaTransfer",
      bytes_base64: transferArchive.toString("base64"),
    },
  };
  assert.deepEqual(directInstruction, expectedTransferInstruction);
  assert.deepEqual(JSON.parse(calls[0].instructions[0]), expectedTransferInstruction);
  assert.deepEqual(Buffer.from(calls[0].secret), privateKey);
  assert.deepEqual(Buffer.from(calls[1].requestArchive), redeemRequestArchive);
  assert.deepEqual(
    JSON.parse(calls[2].instructions[0]),
    {
      KagemushaInstructionArchive: {
        type: "RedeemKagemushaRecursive",
        bytes_base64: redeemInstructionArchive.toString("base64"),
      },
    },
  );
  assert.deepEqual(Buffer.from(calls[2].secret), privateKey);
  assert.deepEqual(Buffer.from(calls[3].requestArchive), topUpRequestArchive);
  assert.deepEqual(
    JSON.parse(calls[4].instructions[0]),
    {
      KagemushaInstructionArchive: {
        type: "TopUpKagemushaRecursive",
        bytes_base64: topUpInstructionArchive.toString("base64"),
      },
    },
  );
  assert.deepEqual(Buffer.from(calls[4].secret), privateKey);
});

test("package dist Kagemusha transaction helpers reject padded authority before native dispatch", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const calls = [];
  const authority = AccountAddress.fromAccount({
    publicKey: Buffer.from(
      "CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03",
      "hex",
    ),
  }).toI105();
  const transferArchive = packageDistKagemushaInstructionArchive(
    "KagemushaTransfer",
    Buffer.from([0x31, 0x32, 0x33]),
  );
  try {
    globalThis.__IROHA_NATIVE_BINDING__ = {
      buildTransaction: () => {
        calls.push("buildTransaction");
        return {
          signed_transaction: Buffer.from([0x71, 0x72]),
          hash: Buffer.alloc(32, 0x73),
        };
      },
    };
    assert.throws(
      () =>
        buildKagemushaInstructionTransaction({
          chainId: "test-chain",
          authority: `${authority} `,
          instruction_type: "KagemushaTransfer",
          instructionArchive: transferArchive,
          privateKey: Buffer.alloc(32, 0x61),
        }),
      /authority must not contain surrounding whitespace/u,
    );
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
  assert.deepEqual(calls, []);
});

test("package dist Torii contract query helpers reject padded selector filters before dispatch", async () => {
  let fetchCalled = false;
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () => {
      fetchCalled = true;
      throw new Error("fetch must not run for padded selector filters");
    },
  });

  await assert.rejects(
    () => client.listContractActivity({ contractAddress: " tairac1router" }),
    /contractAddress must not contain surrounding whitespace/u,
  );
  await assert.rejects(
    () => client.listContractActivity({ contractAlias: "dlmm_router " }),
    /contractAlias must not contain surrounding whitespace/u,
  );
  await assert.rejects(
    () => client.listContractEvents({ participant: "alice@sora " }),
    /participant must not contain surrounding whitespace/u,
  );
  await assert.rejects(
    () => client.listContractEvents({ assetId: " xor#universal" }),
    /assetId must not contain surrounding whitespace/u,
  );
  assert.throws(
    () => client.streamContractEvents({ contractAlias: " dlmm_router" }),
    /contractAlias must not contain surrounding whitespace/u,
  );
  assert.equal(fetchCalled, false);
});

test("package dist governance deploy normalizes only the supported voting-mode aliases", async () => {
  const capturedModes = [];
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async (_url, init) => {
      capturedModes.push(JSON.parse(init.body).mode);
      return new Response(
        JSON.stringify({
          ok: true,
          proposal_id: "cd".repeat(32),
          tx_instructions: [{ wire_id: "ProposeDeployContract" }],
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      );
    },
  });
  const base = {
    contractAddress: "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
    codeHash: `0x${"1a".repeat(32)}`,
    abiHash: Buffer.alloc(32, 0xbb),
  };

  for (const mode of ["Zk", "zk", "ZKBALLOT", "zk_vote", " Plain ", "plainballot"]) {
    await client.governanceProposeDeployContract({ ...base, mode });
  }
  assert.deepEqual(capturedModes, ["Zk", "Zk", "Zk", "Zk", "Plain", "Plain"]);

  for (const mode of ["zero-knowledge", "zkp", "plaintext", "plain_text", 1]) {
    await assert.rejects(
      () => client.governanceProposeDeployContract({ ...base, mode }),
      /must be either 'Zk' or 'Plain'/u,
    );
  }
  assert.equal(capturedModes.length, 6, "invalid modes must fail before fetch");
});

test("package dist UAID path helpers reject padded literals before dispatch", async () => {
  let fetchCalled = false;
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () => {
      fetchCalled = true;
      throw new Error("fetch must not run for padded UAID path literals");
    },
  });
  const rawHex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
  const canonical = `uaid:${rawHex}`;

  for (const value of [` ${canonical}`, `${canonical} `, `uaid: ${rawHex}`]) {
    await assert.rejects(
      () => client.getUaidPortfolio(value),
      /getUaidPortfolio\.uaid must not contain surrounding whitespace/u,
    );
  }
  assert.equal(fetchCalled, false);
});

test("package dist SNS domain route helpers reject padded selectors before dispatch", async () => {
  let fetchCalled = false;
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () => {
      fetchCalled = true;
      throw new Error("fetch must not run for padded SNS selectors");
    },
  });

  await assert.rejects(
    () => client.getSnsRegistration(" alice.sora"),
    /selector must not contain surrounding whitespace/u,
  );
  await assert.rejects(
    () => client.freezeSnsRegistration("alice.sora ", {}),
    /selector must not contain surrounding whitespace/u,
  );
  assert.equal(fetchCalled, false);
});

test("package dist private Kaigi transaction builders reject padded identifiers before native dispatch", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const calls = [];
  try {
    globalThis.__IROHA_NATIVE_BINDING__ = {
      buildPrivateCreateKaigiTransaction: (chainId, call, artifacts, feeSpend) => {
        calls.push(["create", chainId, call, artifacts, feeSpend]);
        return {
          transactionEntrypoint: Buffer.from("create-entrypoint"),
          hash: Buffer.alloc(32, 0x31),
          actionHash: Buffer.alloc(32, 0x32),
        };
      },
      buildPrivateJoinKaigiTransaction: (chainId, callId, artifacts, feeSpend) => {
        calls.push(["join", chainId, callId, artifacts, feeSpend]);
        return {
          transactionEntrypoint: Buffer.from("join-entrypoint"),
          hash: Buffer.alloc(32, 0x41),
          actionHash: Buffer.alloc(32, 0x42),
        };
      },
      buildPrivateEndKaigiTransaction: (chainId, callId, endedAtMs, artifacts, feeSpend) => {
        calls.push(["end", chainId, callId, endedAtMs, artifacts, feeSpend]);
        return {
          transactionEntrypoint: Buffer.from("end-entrypoint"),
          hash: Buffer.alloc(32, 0x51),
          actionHash: Buffer.alloc(32, 0x52),
        };
      },
    };

    buildPrivateCreateKaigiTransaction({
      chainId: "test-chain",
      call: { callId: "call-1" },
      artifacts: { proof: "proof" },
      feeSpend: { amount: "7" },
    });
    buildPrivateJoinKaigiTransaction({
      chainId: "test-chain",
      callId: "call-1",
      artifacts: { proof: "proof" },
      feeSpend: { amount: "7" },
    });
    buildPrivateEndKaigiTransaction({
      chainId: "test-chain",
      callId: "call-1",
      endedAtMs: 42,
      artifacts: { proof: "proof" },
      feeSpend: { amount: "7" },
    });

    assert.deepEqual(calls.map((entry) => entry.slice(0, 3)), [
      ["create", "test-chain", '{"callId":"call-1"}'],
      ["join", "test-chain", "call-1"],
      ["end", "test-chain", "call-1"],
    ]);

    for (const [label, build, message] of [
      [
        "create chainId",
        () =>
          buildPrivateCreateKaigiTransaction({
            chainId: " test-chain",
            call: { callId: "call-1" },
            artifacts: {},
            feeSpend: {},
          }),
        /privateCreateKaigi\.chainId must not contain surrounding whitespace/u,
      ],
      [
        "join chainId",
        () =>
          buildPrivateJoinKaigiTransaction({
            chainId: "test-chain ",
            callId: "call-1",
            artifacts: {},
            feeSpend: {},
          }),
        /privateJoinKaigi\.chainId must not contain surrounding whitespace/u,
      ],
      [
        "join callId",
        () =>
          buildPrivateJoinKaigiTransaction({
            chainId: "test-chain",
            callId: "\ncall-1",
            artifacts: {},
            feeSpend: {},
          }),
        /privateJoinKaigi\.callId must not contain surrounding whitespace/u,
      ],
      [
        "end chainId",
        () =>
          buildPrivateEndKaigiTransaction({
            chainId: " test-chain",
            callId: "call-1",
            artifacts: {},
            feeSpend: {},
          }),
        /privateEndKaigi\.chainId must not contain surrounding whitespace/u,
      ],
      [
        "end callId",
        () =>
          buildPrivateEndKaigiTransaction({
            chainId: "test-chain",
            callId: "call-1 ",
            artifacts: {},
            feeSpend: {},
          }),
        /privateEndKaigi\.callId must not contain surrounding whitespace/u,
      ],
    ]) {
      const before = calls.length;
      assert.throws(build, message, label);
      assert.equal(calls.length, before, label);
    }
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist confidential proof builders reject padded amount literals before native dispatch", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const calls = [];
  const verifyingKey = {
    id: { backend: "halo2/ipa" },
    record: { circuit_id: "confidential-transfer-v2" },
    inlineKey: { bytesBase64: Buffer.from([1, 2, 3]).toString("base64") },
  };
  const spendKey = Buffer.alloc(32, 0x42);
  const rho = Buffer.alloc(32, 0x51).toString("hex");
  const diversifier = Buffer.alloc(32, 0x52).toString("hex");
  const ownerTag = Buffer.alloc(32, 0x53).toString("hex");
  const rootHint = Buffer.alloc(32, 0x54).toString("hex");
  const treeCommitment = Buffer.alloc(32, 0x55).toString("hex");
  const baseTransfer = {
    chainId: "test-chain",
    assetDefinitionId: "asset#domain",
    spendKey,
    treeCommitments: [treeCommitment],
    inputs: [{ amount: "7", rhoHex: rho, diversifierHex: diversifier }],
    outputs: [{ amount: "7", rhoHex: rho, ownerTagHex: ownerTag }],
    rootHintHex: rootHint,
    verifyingKey,
  };
  try {
    globalThis.__IROHA_NATIVE_BINDING__ = {
      buildPrivateKaigiFeeSpend: () => {
        calls.push("feeSpend");
        throw new Error("fee amount should fail before native call");
      },
      buildConfidentialTransferProofV2: () => {
        calls.push("transfer");
        throw new Error("transfer amount should fail before native call");
      },
      buildConfidentialUnshieldProofV2: () => {
        calls.push("unshieldV2");
        throw new Error("unshield v2 publicAmount should fail before native call");
      },
      buildConfidentialUnshieldProofV3: () => {
        calls.push("unshieldV3");
        throw new Error("unshield v3 publicAmount should fail before native call");
      },
    };
    assert.throws(
      () =>
        buildPrivateKaigiFeeSpend({
          chainId: "test-chain",
          assetDefinitionId: "asset#domain",
          actionHash: Buffer.alloc(32, 0xaa),
          anchorRootHex: Buffer.alloc(32, 0xbb).toString("hex"),
          feeAmount: " 7",
          verifyingKey,
        }),
      /privateKaigiFeeSpend\.feeAmount must not contain surrounding whitespace/u,
    );
    assert.throws(
      () =>
        buildConfidentialTransferProofV2({
          ...baseTransfer,
          inputs: [{ amount: " 7", rhoHex: rho, diversifierHex: diversifier }],
        }),
      /inputs\[0\]\.amount must not contain surrounding whitespace/u,
    );
    assert.throws(
      () =>
        buildConfidentialTransferProofV2({
          ...baseTransfer,
          outputs: [{ amount: "7\n", rhoHex: rho, ownerTagHex: ownerTag }],
      }),
      /outputs\[0\]\.amount must not contain surrounding whitespace/u,
    );
    assert.throws(
      () =>
        buildConfidentialTransferProofV2({
          ...baseTransfer,
          inputs: [{ amount: "7", rhoHex: rho }],
        }),
      /inputs\[0\]\.diversifier is required/u,
    );
    assert.throws(
      () =>
        buildConfidentialTransferProofV2({
          ...baseTransfer,
          inputs: [{ amount: "7", rhoHex: rho, diversifier_hex: diversifier }],
        }),
      /inputs\[0\]\.diversifier must use canonical diversifierHex/u,
    );
    assert.throws(
      () =>
        buildConfidentialTransferProofV2({
          ...baseTransfer,
          inputs: [
            { amount: "7", rhoHex: rho, diversifier: Buffer.alloc(32, 0x52) },
          ],
        }),
      /inputs\[0\]\.diversifier must use canonical diversifierHex/u,
    );
    assert.throws(
      () =>
        buildConfidentialUnshieldProofV2({
          chainId: "test-chain",
          assetDefinitionId: "asset#domain",
          spendKey,
          treeCommitments: [treeCommitment],
          inputs: [{ amount: "7", rhoHex: rho, diversifierHex: diversifier }],
          publicAmount: " 7",
          rootHintHex: rootHint,
          verifyingKey,
        }),
      /publicAmount must not contain surrounding whitespace/u,
    );
    assert.throws(
      () =>
        buildConfidentialUnshieldProofV3({
          chainId: "test-chain",
          assetDefinitionId: "asset#domain",
          spendKey,
          treeCommitments: [treeCommitment],
          inputs: [{ amount: "7", rhoHex: rho, diversifierHex: diversifier }],
          outputs: [{ amount: "7", rhoHex: rho }],
          publicAmount: "7\n",
          rootHintHex: rootHint,
          verifyingKey,
        }),
      /publicAmount must not contain surrounding whitespace/u,
    );
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
  assert.deepEqual(calls, []);
});

test("package dist confidential v2 derivation helpers reject padded chain and asset IDs before native dispatch", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const calls = [];
  const commitment = Buffer.alloc(32, 0x6d);
  const nullifier = Buffer.alloc(32, 0x6e);
  const rho = Buffer.alloc(32, 0x51);
  const ownerTag = Buffer.alloc(32, 0x52);
  const spendKey = Buffer.alloc(32, 0x53);
  try {
    globalThis.__IROHA_NATIVE_BINDING__ = {
      deriveConfidentialNoteV2(assetDefinitionId, amount, rhoHex, ownerTagHex) {
        calls.push(["note", assetDefinitionId, amount, rhoHex, ownerTagHex]);
        return commitment;
      },
      deriveConfidentialNullifierV2(chainId, assetDefinitionId, key, rhoHex) {
        calls.push(["nullifier", chainId, assetDefinitionId, Buffer.from(key), rhoHex]);
        return nullifier;
      },
      deriveConfidentialOwnerTagV2(key, diversifierHex) {
        calls.push(["ownerTag", Buffer.from(key), diversifierHex]);
        return ownerTag;
      },
    };
    assert.equal(
      deriveConfidentialNoteV2({
        assetDefinitionId: "asset#domain",
        amount: "7",
        rho,
        ownerTag,
      }).commitmentHex,
      commitment.toString("hex"),
    );
    assert.deepEqual(calls[0], [
      "note",
      "asset#domain",
      "7",
      rho.toString("hex"),
      ownerTag.toString("hex"),
    ]);
    assert.equal(
      deriveConfidentialNullifierV2({
        chainId: "kagemusha-chain",
        assetDefinitionId: "asset#domain",
        spendKey,
        rho,
      }).nullifierHex,
      nullifier.toString("hex"),
    );
    assert.deepEqual(calls[1], [
      "nullifier",
      "kagemusha-chain",
      "asset#domain",
      spendKey,
      rho.toString("hex"),
    ]);
    assert.equal(
      deriveConfidentialOwnerTagV2(spendKey, {
        diversifierHex: rho.toString("hex"),
      }).toString("hex"),
      ownerTag.toString("hex"),
    );
    assert.deepEqual(calls[2], ["ownerTag", spendKey, rho.toString("hex")]);

    calls.length = 0;
    assert.throws(
      () =>
        deriveConfidentialNoteV2({
          assetDefinitionId: " asset#domain",
          amount: "7",
          rho,
          ownerTag,
        }),
      /assetDefinitionId must not contain surrounding whitespace/u,
    );
    assert.throws(
      () =>
        deriveConfidentialNoteV2({
          assetDefinitionId: "asset#domain",
          amount: "7",
          rhoHex: `${rho.toString("hex")} `,
          ownerTag,
        }),
      /rho must not contain surrounding whitespace/u,
    );
    assert.throws(
      () =>
        deriveConfidentialNoteV2({
          assetDefinitionId: "asset#domain",
          amount: "7",
          rho,
          ownerTagHex: ` ${ownerTag.toString("hex")}`,
        }),
      /ownerTag must not contain surrounding whitespace/u,
    );
    assert.throws(
      () =>
        deriveConfidentialNullifierV2({
          chainId: "kagemusha-chain ",
          assetDefinitionId: "asset#domain",
          spendKey,
          rho,
        }),
      /chainId must not contain surrounding whitespace/u,
    );
    assert.throws(
      () =>
        deriveConfidentialNullifierV2({
          chainId: "kagemusha-chain",
          assetDefinitionId: "asset#domain",
          spendKey,
          rhoHex: `${rho.toString("hex")}\n`,
        }),
      /rho must not contain surrounding whitespace/u,
    );
    assert.throws(
      () =>
        deriveConfidentialNullifierV2({
          chainId: "kagemusha-chain",
          assetDefinitionId: "asset#domain\n",
          spendKey,
          rho,
        }),
      /assetDefinitionId must not contain surrounding whitespace/u,
    );
    assert.throws(
      () =>
        deriveConfidentialOwnerTagV2(spendKey, {
          diversifierHex: `${rho.toString("hex")} `,
        }),
      /diversifier must not contain surrounding whitespace/u,
    );
    assert.throws(
      () => deriveConfidentialOwnerTagV2(spendKey),
      /diversifier is required/u,
    );
    assert.throws(
      () =>
        deriveConfidentialOwnerTagV2(spendKey, {
          diversifier: rho,
        }),
      /diversifier must use canonical diversifierHex/u,
    );
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
  assert.deepEqual(calls, []);
});

test("package dist Kagemusha recursive spend verify result decodes ABI fixtures", () => {
  const abi6Result = decodeKagemushaRecursiveSpendVerifyResult(
    sharedRecursiveSpendAbi6Archive("verify_result"),
  );
  assert.equal(abi6Result.valid, false);
  assert.equal(abi6Result.hopCount, 2);
  assert.equal(abi6Result.witnesslessRedeemSupported, false);
  assert.equal(abi6Result.lineageWitnessRequiredForRedeem, true);
  assert.equal(abi6Result.lineage_witness_required_for_redeem, true);
  assert.equal(Object.hasOwn(abi6Result, "lineageWitnessRequired"), false);
  assert.equal(Object.hasOwn(abi6Result, "lineage_witness_required"), false);

  const abi7Result = decodeKagemushaRecursiveSpendVerifyResult(
    sharedRecursiveSpendAbi7Archive("verify_result"),
  );
  assert.equal(abi7Result.valid, true);
  assert.equal(abi7Result.witnesslessRedeemSupported, false);
  assert.equal(abi7Result.lineageWitnessRequiredForRedeem, true);
  assert.equal(abi7Result.lineage_witness_required_for_redeem, true);
  assert.equal(Object.hasOwn(abi7Result, "lineageWitnessRequired"), false);
  assert.equal(Object.hasOwn(abi7Result, "lineage_witness_required"), false);
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendVerifyResult(
        recursiveSpendVerifyResultWithTrailingField(),
      ),
    /verifyResult has trailing bytes/u,
  );
});

test("package dist Kagemusha recursive spend helpers dispatch owned archive copies and return Buffers", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const calls = [];
  const isProbeCall = (archives) =>
    archives.length > 0 &&
    archives.every((archive) => {
      const bytes = Buffer.from(archive);
      return bytes.length === 1 && bytes[0] === 0;
    });
  const inputArchive = (seed) => Uint8Array.from(privacyNoritoFrameWithPayload(seed));
  const cases = [
    [
      "kagemushaRecursiveSpendInit",
      [inputArchive(0x61)],
      (args) => kagemushaRecursiveSpendInit(args[0]),
      0x31,
    ],
    [
      "kagemushaRecursiveSpendAppend",
      [inputArchive(0x62)],
      (args) => kagemushaRecursiveSpendAppend(args[0]),
      0x32,
    ],
    [
      "kagemushaRecursiveSpendTransitionProfileInit",
      [inputArchive(0x63)],
      (args) => kagemushaRecursiveSpendTransitionProfileInit(args[0]),
      0x37,
    ],
    [
      "kagemushaRecursiveSpendTransitionProfileAppend",
      [inputArchive(0x64)],
      (args) => kagemushaRecursiveSpendTransitionProfileAppend(args[0]),
      0x38,
    ],
    [
      "kagemushaRecursiveSpendLineageAppendBoundary",
      [inputArchive(0x65)],
      (args) => kagemushaRecursiveSpendLineageAppendBoundary(args[0]),
      0x39,
    ],
    [
      "kagemushaRecursiveSpendLineageWitnessFromInitResult",
      [inputArchive(0x66), inputArchive(0x67)],
      (args) => kagemushaRecursiveSpendLineageWitnessFromInitResult(args[0], args[1]),
      0x33,
    ],
    [
      "kagemushaRecursiveSpendLineageWitnessAppendResult",
      [inputArchive(0x68), inputArchive(0x69), inputArchive(0x6a)],
      (args) =>
        kagemushaRecursiveSpendLineageWitnessAppendResult(
          args[0],
          args[1],
          args[2],
        ),
      0x34,
    ],
    [
      "kagemushaRecursiveSpendVerify",
      [inputArchive(0x6b)],
      (args) => kagemushaRecursiveSpendVerify(args[0]),
      0x35,
    ],
    [
      "kagemushaRecursiveSpendRedeem",
      [inputArchive(0x6c)],
      (args) => kagemushaRecursiveSpendRedeem(args[0]),
      0x36,
    ],
  ];
  const nativeOutputs = new Map(
    cases.map(([methodName, , , outputSeed]) => [
      methodName,
      Uint8Array.from(privacyNoritoFrameWithPayload(outputSeed)),
    ]),
  );
  const expectedInputs = cases.map(([, args]) => args.map((arg) => Buffer.from(arg)));
  const expectedOutputs = new Map(
    cases.map(([methodName, , , outputSeed]) => [
      methodName,
      Buffer.from(privacyNoritoFrameWithPayload(outputSeed)),
    ]),
  );
  const binding = {
    connectNoritoBridgeAbiVersion() {
      return KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION;
    },
  };
  for (const [methodName] of cases) {
    binding[methodName] = (...archives) => {
      if (isProbeCall(archives)) {
        throw new Error("Kagemusha probe archive rejected");
      }
      calls.push([methodName, ...archives]);
      return nativeOutputs.get(methodName);
    };
  }

  try {
    globalThis.__IROHA_NATIVE_BINDING__ = binding;
    assert.equal(isKagemushaRecursiveSpendNativeAvailable(), true);
    const results = [];
    for (const [methodName, args, call] of cases) {
      const result = call(args);
      assert.ok(Buffer.isBuffer(result), methodName);
      assert.deepEqual(result, expectedOutputs.get(methodName), methodName);
      assert.notStrictEqual(result, nativeOutputs.get(methodName), methodName);
      results.push([methodName, result]);
    }

    for (const [, args] of cases) {
      for (const arg of args) {
        arg[6] ^= 0x7f;
      }
    }
    for (const output of nativeOutputs.values()) {
      output[6] ^= 0x7f;
    }

    assert.equal(calls.length, cases.length);
    for (let index = 0; index < cases.length; index += 1) {
      const [methodName, args] = cases[index];
      const call = calls[index];
      assert.equal(call[0], methodName);
      for (let argIndex = 0; argIndex < args.length; argIndex += 1) {
        assert.notStrictEqual(call[argIndex + 1], args[argIndex], methodName);
        assert.deepEqual(call[argIndex + 1], expectedInputs[index][argIndex], methodName);
      }
    }
    for (const [methodName, result] of results) {
      assert.deepEqual(result, expectedOutputs.get(methodName), methodName);
    }
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist Kagemusha record-backed and Pallas builders dispatch owned archives", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const calls = [];
  const recordBundle = Uint8Array.from(privacyNoritoFrameWithPayload(0x91));
  const pallasOpenEnvelopes = Uint8Array.from(privacyNoritoFrameWithPayload(0x92));
  const previousBundle = Uint8Array.from(privacyNoritoFrameWithPayload(0x93));
  const expectedInputs = new Map([
    ["recordBundle", Buffer.from(recordBundle)],
    ["pallasOpenEnvelopes", Buffer.from(pallasOpenEnvelopes)],
    ["previousBundle", Buffer.from(previousBundle)],
  ]);
  const outputByMethod = new Map([
    [
      "kagemushaProveVerifiedCompactPaymentTokenWithRecords",
      Uint8Array.from(privacyNoritoFrameWithPayload(0x94)),
    ],
    [
      "kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes",
      Uint8Array.from(privacyNoritoFrameWithPayload(0x95)),
    ],
    [
      "kagemushaBuildPallasOpenEnvelopesArchive",
      Uint8Array.from(privacyNoritoFrameWithPayload(0x96)),
    ],
    [
      "kagemushaBuildPreviousProofOpenEnvelopesArchive",
      Uint8Array.from(privacyNoritoFrameWithPayload(0x97)),
    ],
  ]);
  const expectedOutputs = new Map(
    Array.from(outputByMethod, ([methodName, output]) => [
      methodName,
      Buffer.from(output),
    ]),
  );
  const isProbeCall = (archives) =>
    archives.length > 0 &&
    archives.every((archive) => {
      const bytes = Buffer.from(archive);
      return bytes.length === 1 && bytes[0] === 0;
    });
  const dispatch = (methodName, ...archives) => {
    if (isProbeCall(archives)) {
      throw new Error("Kagemusha probe archive rejected");
    }
    calls.push([methodName, ...archives]);
    return outputByMethod.get(methodName);
  };

  try {
    globalThis.__IROHA_NATIVE_BINDING__ = {
      connectNoritoBridgeAbiVersion() {
        return KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION;
      },
      kagemushaProveVerifiedCompactPaymentTokenWithRecords(record) {
        return dispatch("kagemushaProveVerifiedCompactPaymentTokenWithRecords", record);
      },
      kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
        record,
        pallas,
      ) {
        return dispatch(
          "kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes",
          record,
          pallas,
        );
      },
      kagemushaBuildPallasOpenEnvelopesArchive(record) {
        return dispatch("kagemushaBuildPallasOpenEnvelopesArchive", record);
      },
      kagemushaBuildPreviousProofOpenEnvelopesArchive(previousBundleArchive) {
        return dispatch(
          "kagemushaBuildPreviousProofOpenEnvelopesArchive",
          previousBundleArchive,
        );
      },
    };

    assert.equal(isKagemushaCompactPaymentTokenNativeAvailable(), true);
    assert.equal(isKagemushaRecursiveAggregationProofBundleNativeAvailable(), true);
    assert.equal(isKagemushaPallasOpenEnvelopeBuilderNativeAvailable(), true);

    const results = [
      [
        "kagemushaProveVerifiedCompactPaymentTokenWithRecords",
        kagemushaProveVerifiedCompactPaymentTokenWithRecords(recordBundle),
      ],
      [
        "kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes",
        kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
          recordBundle,
          pallasOpenEnvelopes,
        ),
      ],
      [
        "kagemushaBuildPallasOpenEnvelopesArchive",
        kagemushaBuildPallasOpenEnvelopesArchive(recordBundle),
      ],
      [
        "kagemushaBuildPreviousProofOpenEnvelopesArchive",
        kagemushaBuildPreviousProofOpenEnvelopesArchive(previousBundle),
      ],
    ];
    for (const [methodName, result] of results) {
      assert.ok(Buffer.isBuffer(result), methodName);
      assert.deepEqual(result, expectedOutputs.get(methodName), methodName);
      assert.notStrictEqual(result, outputByMethod.get(methodName), methodName);
    }

    recordBundle[6] ^= 0x7f;
    pallasOpenEnvelopes[6] ^= 0x7f;
    previousBundle[6] ^= 0x7f;
    for (const output of outputByMethod.values()) {
      output[6] ^= 0x7f;
    }

    assert.deepEqual(calls.map((call) => call[0]), [
      "kagemushaProveVerifiedCompactPaymentTokenWithRecords",
      "kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes",
      "kagemushaBuildPallasOpenEnvelopesArchive",
      "kagemushaBuildPreviousProofOpenEnvelopesArchive",
    ]);
    assert.notStrictEqual(calls[0][1], recordBundle);
    assert.deepEqual(calls[0][1], expectedInputs.get("recordBundle"));
    assert.notStrictEqual(calls[1][1], recordBundle);
    assert.notStrictEqual(calls[1][2], pallasOpenEnvelopes);
    assert.deepEqual(calls[1][1], expectedInputs.get("recordBundle"));
    assert.deepEqual(calls[1][2], expectedInputs.get("pallasOpenEnvelopes"));
    assert.notStrictEqual(calls[2][1], recordBundle);
    assert.deepEqual(calls[2][1], expectedInputs.get("recordBundle"));
    assert.notStrictEqual(calls[3][1], previousBundle);
    assert.deepEqual(calls[3][1], expectedInputs.get("previousBundle"));
    for (const [methodName, result] of results) {
      assert.deepEqual(result, expectedOutputs.get(methodName), methodName);
    }
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist Kagemusha record-backed and Pallas builders fail closed on invalid archives", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  let nativeDispatches = 0;
  const isProbeCall = (archives) =>
    archives.length > 0 &&
    archives.every((archive) => {
      const bytes = Buffer.from(archive);
      return bytes.length === 1 && bytes[0] === 0;
    });
  const rejectNativeDispatch = (...archives) => {
    if (isProbeCall(archives)) {
      throw new Error("Kagemusha probe archive rejected");
    }
    nativeDispatches += 1;
    throw new Error("native record-backed or Pallas dispatch should not run");
  };
  const validArchive = privacyNoritoFrameWithPayload(0x98);
  const invalidArchives = [
    [Buffer.alloc(0), "must not be empty"],
    [Buffer.from([0x01]), "must be a valid Norito archive"],
    [privacyNoritoFrame(0x98), "must contain a non-empty Norito payload"],
    [Buffer.alloc(KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f), "must not exceed"],
    [undefined, "must be a Buffer, string, or ArrayBuffer view"],
  ];
  const completeBinding = (overrides = {}) => ({
    connectNoritoBridgeAbiVersion() {
      return KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION;
    },
    kagemushaProveVerifiedCompactPaymentTokenWithRecords(record) {
      if (isProbeCall([record])) {
        throw new Error("Kagemusha probe archive rejected");
      }
      return privacyNoritoFrameWithPayload(0x99);
    },
    kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
      record,
      pallas,
    ) {
      if (isProbeCall([record, pallas])) {
        throw new Error("Kagemusha probe archive rejected");
      }
      return privacyNoritoFrameWithPayload(0x9a);
    },
    kagemushaBuildPallasOpenEnvelopesArchive(record) {
      if (isProbeCall([record])) {
        throw new Error("Kagemusha probe archive rejected");
      }
      return privacyNoritoFrameWithPayload(0x9b);
    },
    kagemushaBuildPreviousProofOpenEnvelopesArchive(previousBundleArchive) {
      if (isProbeCall([previousBundleArchive])) {
        throw new Error("Kagemusha probe archive rejected");
      }
      return privacyNoritoFrameWithPayload(0x9c);
    },
    ...overrides,
  });

  try {
    globalThis.__IROHA_NATIVE_BINDING__ = {
      connectNoritoBridgeAbiVersion() {
        return KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION;
      },
      kagemushaProveVerifiedCompactPaymentTokenWithRecords: rejectNativeDispatch,
      kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes:
        rejectNativeDispatch,
      kagemushaBuildPallasOpenEnvelopesArchive: rejectNativeDispatch,
      kagemushaBuildPreviousProofOpenEnvelopesArchive: rejectNativeDispatch,
    };

    assert.equal(isKagemushaCompactPaymentTokenNativeAvailable(), true);
    assert.equal(isKagemushaRecursiveAggregationProofBundleNativeAvailable(), true);
    assert.equal(isKagemushaPallasOpenEnvelopeBuilderNativeAvailable(), true);

    for (const [invalidArchive, expectedMessage] of invalidArchives) {
      assert.throws(
        () => kagemushaProveVerifiedCompactPaymentTokenWithRecords(invalidArchive),
        new RegExp(`recordBundleArchive ${expectedMessage}`),
      );
      assert.throws(
        () =>
          kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
            invalidArchive,
            validArchive,
          ),
        new RegExp(`recordBundleArchive ${expectedMessage}`),
      );
      assert.throws(
        () =>
          kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
            validArchive,
            invalidArchive,
          ),
        new RegExp(`pallasOpenEnvelopesArchive ${expectedMessage}`),
      );
      assert.throws(
        () => kagemushaBuildPallasOpenEnvelopesArchive(invalidArchive),
        new RegExp(`recordBundleArchive ${expectedMessage}`),
      );
      assert.throws(
        () => kagemushaBuildPreviousProofOpenEnvelopesArchive(invalidArchive),
        new RegExp(`previousBundleArchive ${expectedMessage}`),
      );
    }
    assert.equal(nativeDispatches, 0);

    globalThis.__IROHA_NATIVE_BINDING__ = completeBinding({
      kagemushaProveVerifiedCompactPaymentTokenWithRecords(record) {
        if (isProbeCall([record])) {
          throw new Error("Kagemusha probe archive rejected");
        }
        return Buffer.from([0x01]);
      },
    });
    assert.throws(
      () => kagemushaProveVerifiedCompactPaymentTokenWithRecords(validArchive),
      /native kagemushaProveVerifiedCompactPaymentTokenWithRecords returned invalid Norito archive/,
    );

    globalThis.__IROHA_NATIVE_BINDING__ = completeBinding({
      kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
        record,
        pallas,
      ) {
        if (isProbeCall([record, pallas])) {
          throw new Error("Kagemusha probe archive rejected");
        }
        return privacyNoritoFrame(0x9d);
      },
    });
    assert.throws(
      () =>
        kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
          validArchive,
          validArchive,
        ),
      /native kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes returned empty Norito payload/,
    );

    globalThis.__IROHA_NATIVE_BINDING__ = completeBinding({
      kagemushaBuildPallasOpenEnvelopesArchive(record) {
        if (isProbeCall([record])) {
          throw new Error("Kagemusha probe archive rejected");
        }
        return Buffer.from([0x01]);
      },
      kagemushaBuildPreviousProofOpenEnvelopesArchive(previousBundleArchive) {
        if (isProbeCall([previousBundleArchive])) {
          throw new Error("Kagemusha probe archive rejected");
        }
        return privacyNoritoFrame(0x9e);
      },
    });
    assert.throws(
      () => kagemushaBuildPallasOpenEnvelopesArchive(validArchive),
      /native kagemushaBuildPallasOpenEnvelopesArchive returned invalid Norito archive/,
    );
    assert.throws(
      () => kagemushaBuildPreviousProofOpenEnvelopesArchive(validArchive),
      /native kagemushaBuildPreviousProofOpenEnvelopesArchive returned empty Norito payload/,
    );
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist Kagemusha recursive spend helpers propagate native semantic rejections", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const calls = [];
  const requests = new Map([
    [
      "redeem-over-cap",
      Uint8Array.from(privacyNoritoFrameWithPayload(0x71)),
    ],
    [
      "verify-forged-lineage",
      Uint8Array.from(privacyNoritoFrameWithPayload(0x72)),
    ],
    [
      "redeem-forged-lineage",
      Uint8Array.from(privacyNoritoFrameWithPayload(0x73)),
    ],
    [
      "transition-profile-append-forged-opening",
      Uint8Array.from(privacyNoritoFrameWithPayload(0x74)),
    ],
  ]);
  const expectedRequests = new Map(
    Array.from(requests, ([label, request]) => [label, Buffer.from(request)]),
  );
  const isProbeCall = (archives) =>
    archives.length > 0 &&
    archives.every((archive) => {
      const bytes = Buffer.from(archive);
      return bytes.length === 1 && bytes[0] === 0;
    });
  const rejectProbeOrReturn = (seed) => (...archives) => {
    if (isProbeCall(archives)) {
      throw new Error("Kagemusha probe archive rejected");
    }
    return privacyNoritoFrameWithPayload(seed);
  };
  const nativeMethods = {
    kagemushaRecursiveSpendInit: rejectProbeOrReturn(0x31),
    kagemushaRecursiveSpendAppend: rejectProbeOrReturn(0x32),
    kagemushaRecursiveSpendTransitionProfileInit: rejectProbeOrReturn(0x37),
    kagemushaRecursiveSpendLineageAppendBoundary: rejectProbeOrReturn(0x39),
    kagemushaRecursiveSpendLineageWitnessFromInitResult: rejectProbeOrReturn(0x33),
    kagemushaRecursiveSpendLineageWitnessAppendResult: rejectProbeOrReturn(0x34),
  };
  const semanticErrorsBySeed = new Map([
    [
      0x71,
      new Error(
        "invalid Kagemusha recursive spend request: bundle.accumulator.hop_count exceeds Reserved-lineage cap",
      ),
    ],
    [
      0x73,
      new Error(
        "invalid Kagemusha recursive spend request: lineage_verifier_record.commitment",
      ),
    ],
  ]);

  try {
    globalThis.__IROHA_NATIVE_BINDING__ = {
      connectNoritoBridgeAbiVersion() {
        return KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION;
      },
      ...nativeMethods,
      kagemushaRecursiveSpendTransitionProfileAppend(request) {
        if (isProbeCall([request])) {
          throw new Error("Kagemusha probe archive rejected");
        }
        calls.push(["transition-profile-append-forged-opening", request]);
        throw new Error(
          "invalid Kagemusha recursive spend request: hop domain metadata mismatch",
        );
      },
      kagemushaRecursiveSpendVerify(request) {
        if (isProbeCall([request])) {
          throw new Error("Kagemusha probe archive rejected");
        }
        calls.push(["verify-forged-lineage", request]);
        throw new Error(
          "invalid Kagemusha recursive spend request: lineage_verifier_record.commitment",
        );
      },
      kagemushaRecursiveSpendRedeem(request) {
        if (isProbeCall([request])) {
          throw new Error("Kagemusha probe archive rejected");
        }
        const label = request[6] === 0x71 ? "redeem-over-cap" : "redeem-forged-lineage";
        calls.push([label, request]);
        throw semanticErrorsBySeed.get(request[6]);
      },
    };

    assert.equal(isKagemushaRecursiveSpendNativeAvailable(), true);
    assert.throws(
      () => kagemushaRecursiveSpendRedeem(requests.get("redeem-over-cap")),
      /bundle\.accumulator\.hop_count/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendVerify(requests.get("verify-forged-lineage")),
      /lineage_verifier_record\.commitment/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendRedeem(requests.get("redeem-forged-lineage")),
      /lineage_verifier_record\.commitment/,
    );
    assert.throws(
      () =>
        kagemushaRecursiveSpendTransitionProfileAppend(
          requests.get("transition-profile-append-forged-opening"),
        ),
      /hop domain metadata mismatch/,
    );
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }

  for (const request of requests.values()) {
    request[6] ^= 0x7f;
  }
  assert.equal(calls.length, 4);
  for (const [label, archive] of calls) {
    assert.notStrictEqual(archive, requests.get(label), label);
    assert.deepEqual(archive, expectedRequests.get(label), label);
  }
});

test("package dist Kagemusha recursive spend typed requests bind lineage key artifact packages before native dispatch", () => {
  const recordBundle = syntheticKagemushaRecordBundleArchive();
  const pallasOpenEnvelopes = syntheticPallasOpenEnvelopesArchive();
  const currentNote = {
    noteCommitment: Buffer.alloc(32, 0x21),
    spendNullifier: Buffer.alloc(32, 0x22),
    amount: "7",
  };
  const initVerifierKey = kagemushaLineageVerifierKey(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    0x91,
  );
  const initProvingKey = kagemushaLineageProvingKeyArchive(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    initVerifierKey,
    0x92,
  );
  const initArtifacts = kagemushaRecursiveSpendLineageKeyArtifactsForInit(
    2,
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
    initVerifierKey,
    initProvingKey,
  );
  const appendVerifierKey = kagemushaLineageVerifierKey(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    0x93,
  );
  const appendProvingKey = kagemushaLineageProvingKeyArchive(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    appendVerifierKey,
    0x94,
  );
  const appendArtifacts = kagemushaRecursiveSpendLineageKeyArtifactsForAppend(
    2,
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
    appendVerifierKey,
    appendProvingKey,
  );
  assert.ok(
    Buffer.isBuffer(
      encodeKagemushaRecursiveSpendInitRequest({
        recordBundle,
        pallasOpenEnvelopes,
        currentNote,
        lineageKeyArtifacts: initArtifacts,
      }),
    ),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendInitRequest({
        recordBundle,
        pallasOpenEnvelopes,
        currentNote,
        lineageKeyArtifacts: appendArtifacts,
      }),
    kagemushaRequestCodecError("field", "lineageKeyArtifacts", null),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendInitRequest({
        recordBundle,
        pallasOpenEnvelopes,
        currentNote,
        lineageKeyArtifacts: initArtifacts,
        lineageVerifierKey: Buffer.from("vk"),
      }),
    kagemushaRequestCodecError(
      "field",
      "lineageKeyArtifacts",
      /must not be combined with raw key fields/,
    ),
  );
  const previousLineageVerifierRecord = recursiveSpendVerifierRecord();
  assert.ok(
    Buffer.isBuffer(
      encodeKagemushaRecursiveSpendAppendRequest({
        previousBundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote,
        output_proof_circuit_id: KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        previousLineageVerifierRecord,
      }),
    ),
  );
  const appendRequestWithoutSelector = {
    previousBundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
    recordBundle,
    pallasOpenEnvelopes,
    currentNote,
    previousLineageVerifierRecord,
  };
  for (const request of [
    appendRequestWithoutSelector,
    { ...appendRequestWithoutSelector, outputProofCircuitId: undefined },
    { ...appendRequestWithoutSelector, outputProofCircuitId: null },
    { ...appendRequestWithoutSelector, outputProofCircuitId: "" },
    { ...appendRequestWithoutSelector, output_proof_circuit_id: "" },
  ]) {
    assert.throws(
      () => encodeKagemushaRecursiveSpendAppendRequest(request),
      kagemushaRequestCodecError("field", "outputProofCircuitId", null),
    );
  }
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendAppendRequest({
        previousBundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote,
        outputProofCircuitId: "kagemusha-recursive-spend-invalid-output-v1",
        previousLineageVerifierRecord,
        lineageKeyArtifacts: appendArtifacts,
      }),
    kagemushaRequestCodecError("field", "outputProofCircuitId", null),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendAppendRequest({
        previousBundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote,
        outputProofCircuitId: KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        previousLineageVerifierRecord,
        previousProofOpenEnvelopes: syntheticPallasOpenEnvelopesArchive(),
      }),
    kagemushaRequestCodecError(
      "field",
      "previousProofOpenEnvelopes",
      /only valid for lineage append output/,
    ),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendAppendRequest({
        previousBundle: sharedRecursiveSpendAbi7Archive("append_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote,
        outputProofCircuitId: KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        previousLineageVerifierRecord,
      }),
    kagemushaRequestCodecError(
      "field",
      "previousLineageVerifierRecord",
      /only valid for lineage previous bundles/,
    ),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendAppendRequest({
        previousBundle: sharedRecursiveSpendAbi7Archive("append_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote,
        outputProofCircuitId: KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        previousLineageVerifierRecord: {
          verifierKeyId: "danglingPreviousLineageRecord",
          recordBytes: Buffer.from([0]),
        },
      }),
    kagemushaRequestCodecError(
      "field",
      "previousLineageVerifierRecord",
      /only valid for lineage previous bundles/,
    ),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendAppendRequest({
        previousBundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote,
        outputProofCircuitId: KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        previousLineageVerifierRecord,
        lineageKeyArtifacts: appendArtifacts,
      }),
    kagemushaRequestCodecError(
      "field",
      "lineageKeyArtifacts",
      /only valid for lineage append output/,
    ),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendAppendRequest({
        previousBundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote,
        outputProofCircuitId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        previousLineageVerifierRecord,
        previousProofOpenEnvelopes: syntheticPallasOpenEnvelopesArchive(),
        lineageKeyArtifacts: initArtifacts,
      }),
    kagemushaRequestCodecError(
      "field",
      "outputProofCircuitId",
      /cannot prove selected output circuit at previous hop count/,
    ),
  );
});

test("package dist Kagemusha recursive spend validates init keys and fails closed before append key parsing", () => {
  const recordBundle = syntheticKagemushaRecordBundleArchive();
  const pallasOpenEnvelopes = syntheticPallasOpenEnvelopesArchive();
  const previousProofOpenEnvelopes = syntheticPallasOpenEnvelopesArchive();
  const currentNote = {
    noteCommitment: Buffer.alloc(32, 0x21),
    spendNullifier: Buffer.alloc(32, 0x22),
    amount: "7",
  };
  const initVerifierKey = kagemushaLineageVerifierKey(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    0xa1,
  );
  const initProvingKey = kagemushaLineageProvingKeyArchive(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    initVerifierKey,
    0xa2,
  );
  const appendVerifierKey = kagemushaLineageVerifierKey(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    0xa3,
  );
  const appendProvingKey = kagemushaLineageProvingKeyArchive(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    appendVerifierKey,
    0xa4,
  );
  const previousLineageVerifierRecord = recursiveSpendVerifierRecord();
  assert.ok(
    Buffer.isBuffer(
      encodeKagemushaRecursiveSpendInitRequest({
        recordBundle,
        pallasOpenEnvelopes,
        currentNote,
        lineageVerifierKey: initVerifierKey,
        lineageProvingKeyArchive: initProvingKey,
      }),
    ),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendAppendRequest({
        previousBundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote,
        outputProofCircuitId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        previousLineageVerifierRecord,
        previousProofOpenEnvelopes,
        lineageVerifierKey: appendVerifierKey,
        lineageProvingKeyArchive: appendProvingKey,
      }),
    kagemushaRequestCodecError(
      "field",
      "outputProofCircuitId",
      /cannot prove selected output circuit at previous hop count/,
    ),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendInitRequest({
        recordBundle,
        pallasOpenEnvelopes,
        currentNote,
        lineageProvingKeyArchive: initProvingKey,
      }),
    kagemushaRequestCodecError("field", "lineageVerifierKey", null),
    "package dist accepted init raw lineage proving key without verifier key",
  );
  const semanticInitRequest = encodeKagemushaRecursiveSpendInitRequest({
    recordBundle,
    pallasOpenEnvelopes,
    currentNote,
  });
  assert.ok(Buffer.isBuffer(semanticInitRequest));
  assert.equal(semanticInitRequest.length > 0, true);
  const semanticNullInitRequest = encodeKagemushaRecursiveSpendInitRequest({
    recordBundle,
    pallasOpenEnvelopes,
    currentNote,
    lineageVerifierKey: null,
    lineageProvingKeyArchive: null,
  });
  assert.ok(Buffer.isBuffer(semanticNullInitRequest));
  assert.equal(semanticNullInitRequest.length > 0, true);
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendInitRequest({
        recordBundle,
        pallasOpenEnvelopes,
        currentNote,
        lineageVerifierKey: initVerifierKey,
      }),
    kagemushaRequestCodecError("archive", "lineageProvingKeyArchive", null),
    "package dist accepted init raw lineage verifier key without proving key",
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendInitRequest({
        recordBundle,
        pallasOpenEnvelopes,
        currentNote,
        lineageVerifierKey: initVerifierKey,
        lineageProvingKeyArchive: appendProvingKey,
      }),
    kagemushaRequestCodecError("field", "lineageKeyArtifacts", /lineageKeyArtifacts:/),
    "package dist accepted init raw lineage key profile mismatch",
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendAppendRequest({
        previousBundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote,
        outputProofCircuitId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        previousLineageVerifierRecord,
        lineageVerifierKey: appendVerifierKey,
        lineageProvingKeyArchive: appendProvingKey,
      }),
    kagemushaRequestCodecError("field", "outputProofCircuitId", null),
    "package dist did not fail closed before append key parsing",
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendAppendRequest({
        previousBundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote,
        outputProofCircuitId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        previousLineageVerifierRecord,
        previousProofOpenEnvelopes,
        lineageProvingKeyArchive: appendProvingKey,
      }),
    kagemushaRequestCodecError("field", "outputProofCircuitId", null),
    "package dist parsed append raw lineage proving key while the circuit is unavailable",
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendAppendRequest({
        previousBundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote,
        outputProofCircuitId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        previousLineageVerifierRecord,
        previousProofOpenEnvelopes,
        lineageVerifierKey: appendVerifierKey,
      }),
    kagemushaRequestCodecError("field", "outputProofCircuitId", null),
    "package dist parsed append raw lineage verifier key while the circuit is unavailable",
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendAppendRequest({
        previousBundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote,
        outputProofCircuitId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        previousLineageVerifierRecord,
        previousProofOpenEnvelopes,
        lineageVerifierKey: appendVerifierKey,
        lineageProvingKeyArchive: initProvingKey,
      }),
    kagemushaRequestCodecError("field", "outputProofCircuitId", null),
    "package dist parsed append key profiles while the circuit is unavailable",
  );
});

test("package dist Kagemusha recursive spend fails closed before previous lineage parsing", () => {
  const recordBundle = syntheticKagemushaRecordBundleArchive();
  const pallasOpenEnvelopes = syntheticPallasOpenEnvelopesArchive();
  const currentNote = {
    noteCommitment: Buffer.alloc(32, 0x21),
    spendNullifier: Buffer.alloc(32, 0x22),
    amount: "7",
  };
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendAppendRequest({
        previousBundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote,
        outputProofCircuitId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        previousLineageVerifierRecord: {
          verifierKeyId: "malformedPreviousLineageRecordBeforeOpeningsPackageDist",
          recordBytes: Buffer.from([0]),
        },
        previousProofOpenEnvelopes: syntheticPallasOpenEnvelopesArchive(2),
      }),
    kagemushaRequestCodecError("field", "outputProofCircuitId", null),
    "package dist parsed previous lineage material while the output circuit is unavailable",
  );
});

test("package dist Kagemusha recursive spend typed requests reject malformed Pallas opening archives before native dispatch", () => {
  const recordBundle = syntheticKagemushaRecordBundleArchive();
  const recordBundleWithOverLimitStepCount = syntheticKagemushaRecordBundleArchive(1, {
    stepsPayload: u64LE(KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS + 1),
  });
  const pallasOpenEnvelopes = syntheticPallasOpenEnvelopesArchive();
  const previousProofOpenEnvelopes = syntheticPallasOpenEnvelopesArchive();
  const currentNote = {
    noteCommitment: Buffer.alloc(32, 0x21),
    spendNullifier: Buffer.alloc(32, 0x22),
    amount: "7",
  };
  const initVerifierKey = kagemushaLineageVerifierKey(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    0x95,
  );
  const initProvingKey = kagemushaLineageProvingKeyArchive(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    initVerifierKey,
    0x96,
  );
  const appendVerifierKey = kagemushaLineageVerifierKey(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    0x97,
  );
  const appendProvingKey = kagemushaLineageProvingKeyArchive(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    appendVerifierKey,
    0x98,
  );
  const previousLineageVerifierRecord = recursiveSpendVerifierRecord();
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendInitRequest({
        recordBundle: recordBundleWithOverLimitStepCount,
        pallasOpenEnvelopes,
        currentNote,
        lineageVerifierKey: initVerifierKey,
        lineageProvingKeyArchive: initProvingKey,
      }),
    kagemushaRequestCodecError("archive", "recordBundle.steps", /fold step count is out of range/),
    "package dist accepted raw count-prefix-only record-bundle fold steps",
  );
  const malformedPallasMetadataArchives = [
    [
      "vk_commitment",
      syntheticPallasOpenEnvelopesArchive(1, {
        vkCommitmentPayload: kagemushaFixedArrayPayload(0x70, 32),
      }),
    ],
    [
      "vk_commitment",
      syntheticPallasOpenEnvelopesArchive(1, {
        vkCommitmentOptionPayload: optionRawWithTrailingByte(syntheticFixed32(0x70)),
      }),
    ],
    [
      "vk_commitment",
      syntheticPallasOpenEnvelopesArchive(1, {
        vkCommitmentOptionPayload: optionRawWithUnknownTag(),
      }),
      /option tag must be 0 or 1/,
    ],
    [
      "vk_commitment",
      syntheticPallasOpenEnvelopesArchive(1, {
        vkCommitmentOptionPayload: optionRawWithDeclaredLengthTooLong(syntheticFixed32(0x70)),
      }),
      /payload length mismatch/,
    ],
    [
      "public_inputs_schema_hash",
      syntheticPallasOpenEnvelopesArchive(1, {
        publicInputsSchemaHashPayload: kagemushaFixedArrayPayload(0x71, 32),
      }),
    ],
    [
      "public_inputs_schema_hash",
      syntheticPallasOpenEnvelopesArchive(1, {
        publicInputsSchemaHashOptionPayload: optionRawWithTrailingByte(syntheticFixed32(0x71)),
      }),
    ],
    [
      "public_inputs_schema_hash",
      syntheticPallasOpenEnvelopesArchive(1, {
        publicInputsSchemaHashOptionPayload: optionRawWithUnknownTag(),
      }),
      /option tag must be 0 or 1/,
    ],
    [
      "public_inputs_schema_hash",
      syntheticPallasOpenEnvelopesArchive(1, {
        publicInputsSchemaHashOptionPayload: optionRawWithDeclaredLengthTooLong(syntheticFixed32(0x71)),
      }),
      /payload length mismatch/,
    ],
    [
      "domain_tag",
      syntheticPallasOpenEnvelopesArchive(1, {
        domainTagPayload: kagemushaFixedArrayPayload(0x72, 32),
      }),
    ],
    [
      "domain_tag",
      syntheticPallasOpenEnvelopesArchive(1, {
        domainTagOptionPayload: optionRawWithTrailingByte(syntheticFixed32(0x72)),
      }),
    ],
    [
      "domain_tag",
      syntheticPallasOpenEnvelopesArchive(1, {
        domainTagOptionPayload: optionRawWithUnknownTag(),
      }),
      /option tag must be 0 or 1/,
    ],
    [
      "domain_tag",
      syntheticPallasOpenEnvelopesArchive(1, {
        domainTagOptionPayload: optionRawWithDeclaredLengthTooLong(syntheticFixed32(0x72)),
      }),
      /payload length mismatch/,
    ],
  ];
  const malformedPallasOpenEnvelopes = [
    {
      archive: Buffer.from([1, 2, 3]),
      pallasField: "pallasOpenEnvelopes",
      previousField: "previousProofOpenEnvelopes",
      message: /valid Norito archive/,
    },
    {
      archive: syntheticKagemushaArchive("test::PallasOpenEnvelopes", 0x72),
      pallasField: "pallasOpenEnvelopes",
      previousField: "previousProofOpenEnvelopes",
      message: /valid Vec<iroha_zkp_halo2::OpenVerifyEnvelope>/,
    },
    {
      archive: syntheticPallasOpenEnvelopesArchive(2),
      pallasField: "pallasOpenEnvelopes",
      previousField: "previousProofOpenEnvelopes",
      message: /requires exactly 1 envelope\(s\)/,
    },
    {
      archive: syntheticPallasOpenEnvelopesArchive(1, { includeDomainTag: false }),
      pallasField: "pallasOpenEnvelopes[0].domain_tag",
      previousField: "previousProofOpenEnvelopes[0].domain_tag",
      message: /is required/,
    },
    {
      archive: syntheticPallasOpenEnvelopesArchive(1, { transcriptLabel: "" }),
      pallasField: "pallasOpenEnvelopes[0]",
      previousField: "previousProofOpenEnvelopes[0]",
      message: /transcript_label is invalid/,
    },
    {
      archive: syntheticPallasOpenEnvelopesArchive(1, { transcriptLabel: "\u00e9".repeat(65) }),
      pallasField: "pallasOpenEnvelopes[0]",
      previousField: "previousProofOpenEnvelopes[0]",
      message: /transcript_label is invalid/,
    },
    {
      archive: syntheticPallasOpenEnvelopesArchive(1, { paramsGSequencePayload: u64LE(5) }),
      pallasField: "pallasOpenEnvelopes[0].params",
      previousField: "previousProofOpenEnvelopes[0].params",
      message: /generator count mismatch/,
    },
    {
      archive: syntheticPallasOpenEnvelopesArchive(1, { proofLSequencePayload: u64LE(3) }),
      pallasField: "pallasOpenEnvelopes[0].proof",
      previousField: "previousProofOpenEnvelopes[0].proof",
      message: /round count mismatch/,
    },
    ...malformedPallasMetadataArchives.map(([metadataField, archive, metadataMessage]) => ({
      archive,
      pallasField: `pallasOpenEnvelopes[0].${metadataField}`,
      previousField: `previousProofOpenEnvelopes[0].${metadataField}`,
      message: metadataMessage ?? null,
    })),
  ];
  for (const {
    archive: malformedPallasOpenEnvelopesArchive,
    pallasField,
    message,
  } of malformedPallasOpenEnvelopes) {
    assert.throws(
      () =>
        encodeKagemushaRecursiveSpendInitRequest({
          recordBundle,
          pallasOpenEnvelopes: malformedPallasOpenEnvelopesArchive,
          currentNote,
          lineageVerifierKey: initVerifierKey,
          lineageProvingKeyArchive: initProvingKey,
        }),
      kagemushaRequestCodecError("archive", pallasField, message),
      "package dist accepted malformed init Pallas open-envelope archive",
    );
  }
  for (const {
    archive: malformedPallasOpenEnvelopesArchive,
    pallasField,
    message,
  } of malformedPallasOpenEnvelopes) {
    assert.throws(
      () =>
        encodeKagemushaRecursiveSpendAppendRequest({
          previousBundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
          recordBundle,
          pallasOpenEnvelopes: malformedPallasOpenEnvelopesArchive,
          currentNote,
          outputProofCircuitId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
          previousLineageVerifierRecord,
          previousProofOpenEnvelopes,
          lineageVerifierKey: appendVerifierKey,
          lineageProvingKeyArchive: appendProvingKey,
        }),
      kagemushaRequestCodecError("archive", pallasField, message),
      "package dist accepted malformed append Pallas open-envelope archive",
    );
  }
  for (const {
    archive: malformedPreviousProofOpenEnvelopes,
    previousField,
    message,
  } of malformedPallasOpenEnvelopes) {
    assert.throws(
      () =>
        encodeKagemushaRecursiveSpendAppendRequest({
          previousBundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
          recordBundle,
          pallasOpenEnvelopes,
          currentNote,
          outputProofCircuitId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
          previousLineageVerifierRecord,
          previousProofOpenEnvelopes: malformedPreviousProofOpenEnvelopes,
          lineageVerifierKey: appendVerifierKey,
          lineageProvingKeyArchive: appendProvingKey,
        }),
      kagemushaRequestCodecError("field", "outputProofCircuitId", null),
      `package dist parsed unavailable lineage append opening ${previousField}: ${message}`,
    );
  }
  for (const [
    metadataField,
    malformedPallasOpenEnvelopesArchive,
    expectedMessage,
  ] of malformedPallasMetadataArchives) {
    assert.throws(
      () =>
        encodeKagemushaRecursiveSpendInitRequest({
          recordBundle,
          pallasOpenEnvelopes: malformedPallasOpenEnvelopesArchive,
          currentNote,
          lineageVerifierKey: initVerifierKey,
          lineageProvingKeyArchive: initProvingKey,
        }),
      (error) =>
        error?.kind === "archive" &&
        error.field === `pallasOpenEnvelopes[0].${metadataField}` &&
        (expectedMessage == null || expectedMessage.test(error.message)),
      `package dist accepted stale fixed-array Pallas metadata payload for ${metadataField}`,
    );
    assert.throws(
      () =>
        encodeKagemushaRecursiveSpendAppendRequest({
          previousBundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
          recordBundle,
          pallasOpenEnvelopes,
          currentNote,
          outputProofCircuitId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
          previousLineageVerifierRecord,
          previousProofOpenEnvelopes: malformedPallasOpenEnvelopesArchive,
          lineageVerifierKey: appendVerifierKey,
          lineageProvingKeyArchive: appendProvingKey,
        }),
      kagemushaRequestCodecError("field", "outputProofCircuitId", null),
      `package dist parsed unavailable lineage append metadata ${metadataField}: ${expectedMessage}`,
    );
  }
});

test("package dist Kagemusha recursive spend typed requests reject malformed amount vectors before native dispatch", () => {
  const packageDistInvalidPositiveU128Amounts = [
    "",
    "0",
    "00",
    "01",
    "0007",
    "-1",
    "+1",
    "1.0",
    "1e3",
    "7 ",
    " 7",
    "\t7",
    "7\n",
    String(1n << 128n),
    "9".repeat(40),
  ];
  for (const amount of packageDistInvalidPositiveU128Amounts) {
    assert.throws(
      () =>
        buildKagemushaRecursiveSpendableNoteDescriptor({
          noteCommitment: Buffer.alloc(32, 0x21),
          spendNullifier: Buffer.alloc(32, 0x22),
          amount,
        }),
      kagemushaRequestCodecError("field", "amount", null),
      `package dist accepted malformed note amount ${JSON.stringify(amount)}`,
    );
  }

  const redeemProof = syntheticKagemushaArchive(KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME, 0x98);
  const packageDistInvalidPublicAmounts = [
    "",
    "0",
    "00",
    "01",
    "0007",
    "-1",
    "+1",
    "1.0",
    "1e3",
    "7 ",
    " 7",
    "\t7",
    "7\n",
    String(1n << 128n),
    "9".repeat(40),
  ];
  for (const publicAmount of packageDistInvalidPublicAmounts) {
    assert.throws(
      () =>
        encodeKagemushaRecursiveSpendRedeemRequest({
          bundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
          recipient: "dist-recipient",
          publicAmount,
          redeemProof,
        }),
      kagemushaRequestCodecError("field", "publicAmount", null),
      `package dist accepted malformed publicAmount ${JSON.stringify(publicAmount)}`,
    );
  }
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
        recipient: "dist-recipient",
        public_amount: "0007",
        redeemProof,
      }),
    kagemushaRequestCodecError("field", "publicAmount", /canonical decimal u128/),
  );
});

test("package dist Kagemusha recursive spend typed requests reject malformed blockHeight vectors before native dispatch", () => {
  const recordBundle = syntheticKagemushaRecordBundleArchive();
  const pallasOpenEnvelopes = syntheticPallasOpenEnvelopesArchive();
  const currentNote = {
    noteCommitment: Buffer.alloc(32, 0x21),
    spendNullifier: Buffer.alloc(32, 0x22),
    amount: "7",
  };
  const verifierKey = kagemushaLineageVerifierKey(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    0x96,
  );
  const provingKey = kagemushaLineageProvingKeyArchive(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    verifierKey,
    0x97,
  );
  const lineageKeyArtifacts = kagemushaRecursiveSpendLineageKeyArtifactsForInit(
    2,
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
    verifierKey,
    provingKey,
  );
  const previousLineageVerifierRecord = recursiveSpendVerifierRecord();
  const redeemProof = syntheticKagemushaArchive(KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME, 0x98);
  const lineageWitness = sharedRecursiveSpendAbi6Archive("lineage_witness_append_result");
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendVerifyRequest({
        bundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
      }),
    kagemushaRequestCodecError(
      "field",
      "lineageVerifierRecord",
      /required for reserved-lineage bundles/,
    ),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendVerifyRequest({
        bundle: sharedRecursiveSpendAbi7Archive("append_bundle"),
        lineageVerifierRecord: previousLineageVerifierRecord,
      }),
    kagemushaRequestCodecError(
      "field",
      "lineageVerifierRecord",
      /only valid for reserved-lineage bundles/,
    ),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendVerifyRequest({
        bundle: sharedRecursiveSpendAbi7Archive("append_bundle"),
        lineageVerifierRecord: {
          verifierKeyId: "danglingVerifyLineageRecord",
          recordBytes: Buffer.from([0]),
        },
      }),
    kagemushaRequestCodecError(
      "field",
      "lineageVerifierRecord",
      /only valid for reserved-lineage bundles/,
    ),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendAbi7Archive("append_bundle"),
        recipient: "dist-recipient",
        publicAmount: "7",
        redeemProof,
        lineageVerifierRecord: previousLineageVerifierRecord,
      }),
    kagemushaRequestCodecError(
      "field",
      "lineageVerifierRecord",
      /only valid for reserved-lineage bundles or lineage witnesses/,
    ),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendAbi7Archive("append_bundle"),
        recipient: "dist-recipient",
        publicAmount: "7",
        redeemProof,
        lineageVerifierRecord: {
          verifierKeyId: "danglingRedeemLineageRecord",
          recordBytes: Buffer.from([0]),
        },
      }),
    kagemushaRequestCodecError(
      "field",
      "lineageVerifierRecord",
      /only valid for reserved-lineage bundles or lineage witnesses/,
    ),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendAbi7Archive("append_bundle"),
        recipient: "dist-recipient",
        publicAmount: "7",
        redeemProof,
        lineageWitness: sharedRecursiveSpendAbi6Archive("lineage_witness_append_result"),
      }),
    kagemushaRequestCodecError(
      "field",
      "lineageVerifierRecord",
      /required for lineage witnesses with reserved-lineage previous proofs/,
    ),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendAbi7Archive("append_bundle"),
        recipient: "dist-recipient",
        publicAmount: "7",
        redeemProof,
        lineageWitness: sharedRecursiveSpendAbi6Archive("lineage_witness_from_init_result"),
        lineageVerifierRecord: previousLineageVerifierRecord,
      }),
    kagemushaRequestCodecError(
      "field",
      "lineageVerifierRecord",
      /only valid for reserved-lineage bundles or lineage witnesses/,
    ),
  );
  const reservedMissingRecordMasksMalformedWitnessPackageDist = Buffer.from([0]);
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
        recipient: "dist-recipient",
        publicAmount: "7",
        redeemProof,
        lineageWitness: reservedMissingRecordMasksMalformedWitnessPackageDist,
      }),
    kagemushaRequestCodecError(
      "field",
      "lineageVerifierRecord",
      /required for reserved-lineage bundles/,
    ),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
        recipient: "dist-recipient",
        publicAmount: "7",
        redeemProof,
        lineageWitness: Buffer.from([0]),
        lineageVerifierRecord: {
          verifierKeyId: "forgedRedeemLineageRecordBeforeWitnessPackageDist",
          recordBytes: Buffer.from([0]),
        },
      }),
    kagemushaRequestCodecError("archive", "lineageVerifierRecord", /valid Norito archive/),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
        recipient: "dist-recipient",
        publicAmount: "7",
        redeemProof,
        lineageWitness: syntheticKagemushaArchive(
          KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
          0x9a,
        ),
        lineageVerifierRecord: previousLineageVerifierRecord,
      }),
    kagemushaRequestCodecError("archive", "lineageWitness", /truncated/),
  );
  const malformedLineageWitnesses = [
    [
      recursiveSpendLineageWitnessWithTrailingField(),
      "lineageWitness",
      /lineageWitness has trailing bytes/,
    ],
    [
      recursiveSpendLineageWitnessWithTrailingPreviousProofsField(),
      "lineageWitness.previousRecursiveProofs",
      /lineageWitness\.previousRecursiveProofs/,
    ],
    [
      recursiveSpendLineageWitnessWithPreviousProofCountPrefixOnly(
        KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS + 1,
      ),
      "lineageWitness.previousRecursiveProofs",
      /lineageWitness\.previousRecursiveProofs count exceeds 64/,
    ],
    [
      recursiveSpendLineageWitnessWithTrailingPreviousProofField(),
      "lineageWitness.previousRecursiveProofs",
      /lineageWitness\.previousRecursiveProofs/,
    ],
    [
      recursiveSpendLineageWitnessWithTrailingPreviousVerifierKeyIdField(),
      "lineageWitness.previousRecursiveProofs.verifierKeyId",
      /lineageWitness\.previousRecursiveProofs\.verifierKeyId/,
    ],
    [
      recursiveSpendLineageWitnessWithPreviousProofField(1, Buffer.alloc(0)),
      "lineageWitness.previousRecursiveProofs.proof_public_inputs",
      /lineageWitness\.previousRecursiveProofs\.proof_public_inputs/,
    ],
    [
      recursiveSpendLineageWitnessWithPreviousProofField(2, Buffer.alloc(32)),
      "lineageWitness.previousRecursiveProofs.proof_public_inputs_hash",
      /lineageWitness\.previousRecursiveProofs\.proof_public_inputs_hash/,
    ],
    [
      recursiveSpendLineageWitnessWithPreviousProofField(2, Buffer.alloc(32, 0x44)),
      "lineageWitness.previousRecursiveProofs.proof_public_inputs_hash",
      /lineageWitness\.previousRecursiveProofs\.proof_public_inputs_hash/,
    ],
    [
      recursiveSpendLineageWitnessWithPreviousProofField(2, kagemushaFixedArrayPayload(0x44, 31)),
      "lineageWitness.previousRecursiveProofs.proof_public_inputs_hash",
      /lineageWitness\.previousRecursiveProofs\.proof_public_inputs_hash/,
    ],
    [
      recursiveSpendLineageWitnessWithPreviousProofField(2, kagemushaCountPrefixedFixedArrayPayload(0x44, 32)),
      "lineageWitness.previousRecursiveProofs.proof_public_inputs_hash",
      /lineageWitness\.previousRecursiveProofs\.proof_public_inputs_hash/,
    ],
    [
      recursiveSpendLineageWitnessWithPreviousProofField(2, kagemushaFixedArrayPayload(0x44, 33)),
      "lineageWitness.previousRecursiveProofs.proof_public_inputs_hash",
      /lineageWitness\.previousRecursiveProofs\.proof_public_inputs_hash/,
    ],
    [
      recursiveSpendLineageWitnessWithPreviousProofBoxBackend("halo2/kzg"),
      "lineageWitness.previousRecursiveProofs.proof_backend",
      /lineageWitness\.previousRecursiveProofs\.proof_backend/,
    ],
    [
      recursiveSpendLineageWitnessWithPreviousProofBoxBackendAndEmptyProofBytes("halo2/kzg"),
      "lineageWitness.previousRecursiveProofs.proof_backend",
      /lineageWitness\.previousRecursiveProofs\.proof_backend/,
    ],
    [
      recursiveSpendLineageWitnessWithEmptyPreviousProofBytes(),
      "lineageWitness.previousRecursiveProofs.proof_bytes",
      /lineageWitness\.previousRecursiveProofs\.proof_bytes/,
    ],
  ];
  for (const [lineageWitnessArchive, expectedField, expectedError] of malformedLineageWitnesses) {
    assert.throws(
      () =>
        encodeKagemushaRecursiveSpendRedeemRequest({
          bundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
          recipient: "dist-recipient",
          publicAmount: "7",
          redeemProof,
          lineageWitness: lineageWitnessArchive,
          lineageVerifierRecord: previousLineageVerifierRecord,
        }),
      kagemushaRequestCodecError("archive", expectedField, expectedError),
    );
  }
  const blockHeightEncoders = [
    [
      "init",
      (blockHeight) =>
        encodeKagemushaRecursiveSpendInitRequest({
          recordBundle,
          pallasOpenEnvelopes,
          currentNote,
          lineageKeyArtifacts,
          blockHeight,
        }),
    ],
    [
      "append",
      (blockHeight) =>
        encodeKagemushaRecursiveSpendAppendRequest({
          previousBundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
          recordBundle,
          pallasOpenEnvelopes,
          currentNote,
          outputProofCircuitId: KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
          previousLineageVerifierRecord,
          blockHeight,
        }),
    ],
    [
      "verify",
      (blockHeight) =>
        encodeKagemushaRecursiveSpendVerifyRequest({
          bundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
          lineageVerifierRecord: previousLineageVerifierRecord,
          blockHeight,
        }),
    ],
    [
      "redeem",
      (blockHeight) =>
        encodeKagemushaRecursiveSpendRedeemRequest({
          bundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
          recipient: "dist-recipient",
          publicAmount: "7",
          redeemProof,
          lineageWitness,
          lineageVerifierRecord: previousLineageVerifierRecord,
          blockHeight,
        }),
    ],
  ];
  const packageDistInvalidBlockHeights = [
    "00",
    "01",
    "0007",
    "-0",
    "+7",
    "7 ",
    " 7",
    "18446744073709551616",
    "9".repeat(21),
    -0,
  ];
  assert.equal(Object.is(packageDistInvalidBlockHeights.at(-1), -0), true);
  for (const [name, encode] of blockHeightEncoders) {
    for (const blockHeight of packageDistInvalidBlockHeights) {
      assert.throws(
        () => encode(blockHeight),
        kagemushaRequestCodecError("field", "blockHeight", null),
        `${name} accepted non-canonical package-dist blockHeight ${JSON.stringify(blockHeight)}`,
      );
    }
  }
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
        recipient: "dist-recipient",
        publicAmount: "7",
        redeemProof,
        lineageWitness,
        lineageVerifierRecord: previousLineageVerifierRecord,
        block_height: "0007",
      }),
    kagemushaRequestCodecError("field", "blockHeight", null),
  );
});

test("package dist Kagemusha recursive spend redeem rejects missing lineage material before native dispatch", () => {
  const redeemProof = Buffer.from([0]);
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendAbi7Archive("append_bundle"),
        recipient: "dist-recipient",
        publicAmount: "7",
        redeemProof,
      }),
    kagemushaRequestCodecError("field", "lineageWitness", /required for this bundle/),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
        recipient: "dist-recipient",
        publicAmount: "7",
        redeemProof,
      }),
    kagemushaRequestCodecError(
      "field",
      "lineageVerifierRecord",
      /required for reserved-lineage bundles/,
    ),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendAbi7Archive("append_bundle"),
        recipient: "dist-recipient",
        publicAmount: "7",
        redeemProof,
        lineageVerifierRecords: [
          {
            verifierKeyId: "distDanglingRedeemLineageRecords",
            recordBytes: Buffer.from([0]),
          },
        ],
      }),
    kagemushaRequestCodecError(
      "field",
      "lineageVerifierRecord",
      /only valid for reserved-lineage bundles or lineage witnesses/,
    ),
  );
});

test("package dist Kagemusha recursive spend redeem rejects invalid change-output relationships before native dispatch", () => {
  const verifierRecord = recursiveSpendVerifierRecord();
  const redeemProof = syntheticKagemushaArchive(KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME, 0x64);
  const lineageWitness = syntheticKagemushaArchive(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
    0x65,
  );
  for (const [changeOutput, errorPattern] of [
    [Buffer.alloc(31, 1), /changeOutput must be 32 bytes/],
    [Buffer.alloc(32), /changeOutput must be non-zero/],
  ]) {
    assert.throws(
      () =>
        encodeKagemushaRecursiveSpendRedeemRequest({
          bundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
          recipient: "dist-recipient",
          publicAmount: "7",
          redeemProof,
          lineageWitness,
          changeOutput,
          lineageVerifierRecord: verifierRecord,
        }),
      kagemushaRequestCodecError("field", "changeOutput", errorPattern),
    );
  }
  const partialBundle = sharedRecursiveSpendAbi7Archive("append_bundle");
  const partialSummary = decodeKagemushaRecursiveSpendBundle(partialBundle);
  assert.ok(partialSummary.topupAnchorNullifiers.length > 0);
  assert.ok(partialSummary.topup_anchor_nullifiers.length > 0);
  for (const changeOutput of [
    partialSummary.currentNote.noteCommitment,
    partialSummary.currentNote.spendNullifier,
    partialSummary.topupAnchorNullifiers[0],
  ]) {
    assert.throws(
      () =>
        encodeKagemushaRecursiveSpendRedeemRequest({
          bundle: partialBundle,
          recipient: "dist-recipient",
          publicAmount: "6",
          redeemProof,
          changeOutput,
        }),
      kagemushaRequestCodecError("field", "changeOutput", /must not reuse/),
    );
  }
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
        recipient: "dist-recipient",
        publicAmount: "6",
        redeemProof,
        lineageWitness,
        lineageVerifierRecord: verifierRecord,
      }),
    kagemushaRequestCodecError("field", "changeOutput", /changeOutput is required/),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
        recipient: "dist-recipient",
        publicAmount: "8",
        redeemProof,
        lineageWitness,
        lineageVerifierRecord: verifierRecord,
      }),
    kagemushaRequestCodecError("field", "publicAmount", /publicAmount must not exceed/),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendAbi6Archive("init_bundle"),
        recipient: "dist-recipient",
        publicAmount: "7",
        redeemProof,
        lineageWitness,
        changeOutput: Buffer.alloc(32, 0x42),
        lineageVerifierRecord: verifierRecord,
      }),
    kagemushaRequestCodecError("field", "publicAmount", /publicAmount must be less/),
  );
});

test("package dist Kagemusha recursive spend bundle rejects wrong accumulator domain before native dispatch", () => {
  for (const domain of [
    "iroha:kagemusha:v1:recursive-spend-accumulatoR",
    " iroha:kagemusha:v1:recursive-spend-accumulator",
    "iroha:Kagemusha:v1:recursive-spend-accumulator",
  ]) {
    assert.throws(
      () =>
        decodeKagemushaRecursiveSpendBundle(
          recursiveSpendBundleWithAccumulatorDomain(domain),
        ),
      kagemushaRequestCodecError("archive", "bundle.accumulator.domain", null),
    );
  }
});

test("package dist Kagemusha recursive spend bundle decodes canonical accumulator assets", () => {
  const initBundle = decodeKagemushaRecursiveSpendBundle(
    sharedRecursiveSpendAbi6Archive("init_bundle"),
  );
  assert.equal(initBundle.asset, "686w6ABhTWPaCrWNjjXs7X1SW6w9");
  const rawHexAssetBundle = decodeKagemushaRecursiveSpendBundle(
    recursiveSpendBundleWithAccumulatorField(
      2,
      kagemushaFixedArrayPayload(0x01, 16),
    ),
  );
  assert.equal(
    rawHexAssetBundle.asset,
    "hex:01010101010101010101010101010101",
  );
  assert.ok(initBundle.topupAnchorNullifiers.length >= 2);
  const originalInitialRoot = Buffer.from(initBundle.initialRoot);
  const mutatedInitialRoot = initBundle.initialRoot;
  mutatedInitialRoot[0] ^= 0xff;
  assert.deepEqual(initBundle.initialRoot, originalInitialRoot);
  assert.deepEqual(initBundle.initial_root, originalInitialRoot);
  const originalFinalRoot = Buffer.from(initBundle.finalRoot);
  const mutatedFinalRoot = initBundle.final_root;
  mutatedFinalRoot[0] ^= 0xff;
  assert.deepEqual(initBundle.finalRoot, originalFinalRoot);
  assert.deepEqual(initBundle.final_root, originalFinalRoot);
  const originalTopupAnchorNullifiers = initBundle.topupAnchorNullifiers.map((value) =>
    Buffer.from(value),
  );
  const mutatedTopupAnchors = initBundle.topupAnchorNullifiers;
  mutatedTopupAnchors[0][0] ^= 0xff;
  mutatedTopupAnchors.length = 0;
  assert.deepEqual(initBundle.topupAnchorNullifiers, originalTopupAnchorNullifiers);
  assert.deepEqual(initBundle.topup_anchor_nullifiers, originalTopupAnchorNullifiers);
  const originalNoteCommitment = Buffer.from(initBundle.currentNote.noteCommitment);
  const mutatedNoteCommitment = initBundle.currentNote.note_commitment;
  mutatedNoteCommitment[0] ^= 0xff;
  assert.deepEqual(initBundle.currentNote.noteCommitment, originalNoteCommitment);
  assert.deepEqual(initBundle.current_note.note_commitment, originalNoteCommitment);
  const originalSpendNullifier = Buffer.from(initBundle.currentNote.spendNullifier);
  const mutatedSpendNullifier = initBundle.current_note.spend_nullifier;
  mutatedSpendNullifier[0] ^= 0xff;
  assert.deepEqual(initBundle.currentNote.spendNullifier, originalSpendNullifier);
  assert.deepEqual(initBundle.current_note.spend_nullifier, originalSpendNullifier);
  const mutableVerifierRecordBytes = syntheticKagemushaArchive(
    KAGEMUSHA_VERIFYING_KEY_RECORD_WIRE_NAME,
    0x68,
  );
  const copiedVerifierRecordBytes = Buffer.from(mutableVerifierRecordBytes);
  const copiedVerifierRecord = buildKagemushaRecursiveSpendVerifierRecordRef({
    verifierKeyId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    recordBytes: mutableVerifierRecordBytes,
  });
  assert.equal(Object.isFrozen(copiedVerifierRecord), true);
  mutableVerifierRecordBytes[mutableVerifierRecordBytes.length - 1] ^= 0xff;
  assert.deepEqual(copiedVerifierRecord.recordBytes, copiedVerifierRecordBytes);
  assert.deepEqual(copiedVerifierRecord.record_bytes, copiedVerifierRecordBytes);
  const returnedVerifierRecordBytes = copiedVerifierRecord.record_bytes;
  returnedVerifierRecordBytes[returnedVerifierRecordBytes.length - 1] ^= 0xff;
  assert.deepEqual(copiedVerifierRecord.recordBytes, copiedVerifierRecordBytes);
  assert.deepEqual(copiedVerifierRecord.record_bytes, copiedVerifierRecordBytes);
  const malformedTopupAnchorCases = [
    ["topup anchor empty list", [], "bundle.accumulator.topup_anchor_nullifiers count is out of range"],
    [
      "topup anchor zero nullifier",
      [Buffer.alloc(32)],
      "bundle.accumulator.topup_anchor_nullifiers must not contain zero values",
    ],
    ["topup anchor count over limit", [
      initBundle.topupAnchorNullifiers[0],
      initBundle.topupAnchorNullifiers[1],
      Buffer.alloc(32, 0x34),
    ], "bundle.accumulator.topup_anchor_nullifiers count is out of range"],
    [
      "topup anchor duplicate nullifier",
      [initBundle.topupAnchorNullifiers[0], initBundle.topupAnchorNullifiers[0]],
      "bundle.accumulator.topup_anchor_nullifiers must be strictly sorted and unique",
    ],
    [
      "topup anchor descending order",
      [initBundle.topupAnchorNullifiers[1], initBundle.topupAnchorNullifiers[0]],
      "bundle.accumulator.topup_anchor_nullifiers must be strictly sorted and unique",
    ],
    [
      "topup anchor current note commitment reuse",
      [initBundle.currentNote.noteCommitment],
      "bundle.accumulator.topup_anchor_nullifiers must not reuse current note material",
    ],
    [
      "topup anchor current note spend nullifier reuse",
      [initBundle.currentNote.spendNullifier],
      "bundle.accumulator.topup_anchor_nullifiers must not reuse current note material",
    ],
  ];
  for (const [label, nullifiers, expectedMessage] of malformedTopupAnchorCases) {
    assert.throws(
      () => decodeKagemushaRecursiveSpendBundle(recursiveSpendBundleWithTopupAnchorNullifiers(nullifiers)),
      kagemushaRequestCodecError("archive", "bundle.accumulator.topup_anchor_nullifiers", expectedMessage),
      label,
    );
  }
  assert.throws(
    () => decodeKagemushaRecursiveSpendBundle(
      recursiveSpendBundleWithAccumulatorField(5, u64LE(3)),
    ),
    kagemushaRequestCodecError(
      "archive",
      "bundle.accumulator.topup_anchor_nullifiers",
      "bundle.accumulator.topup_anchor_nullifiers count is out of range",
    ),
    "topup anchor over-limit count prefix",
  );
  assert.throws(
    () => decodeKagemushaRecursiveSpendBundle(
      recursiveSpendBundleWithTopupAnchorNullifiersAndEmptyProofBytes([Buffer.alloc(32)]),
    ),
    kagemushaRequestCodecError(
      "archive",
      "bundle.accumulator.topup_anchor_nullifiers",
      "bundle.accumulator.topup_anchor_nullifiers must not contain zero values",
    ),
    "malformed proof cannot mask invalid top-up anchor nullifiers",
  );
  assert.throws(
    () => decodeKagemushaRecursiveSpendBundle(
      recursiveSpendBundleWithTopupAnchorNullifiersAndTrailingAccumulatorField([Buffer.alloc(32)]),
    ),
    kagemushaRequestCodecError(
      "archive",
      "bundle.accumulator.topup_anchor_nullifiers",
      "bundle.accumulator.topup_anchor_nullifiers must not contain zero values",
    ),
    "trailing accumulator cannot mask invalid top-up anchor nullifiers",
  );
  const maskedTopupAnchorPrecedenceCases = [
    [
      "malformed proof cannot mask current-note top-up anchor reuse",
      recursiveSpendBundleWithTopupAnchorNullifiersAndEmptyProofBytes([
        initBundle.currentNote.noteCommitment,
      ]),
      "bundle.accumulator.topup_anchor_nullifiers must not reuse current note material",
    ],
    [
      "trailing accumulator cannot mask current-note top-up anchor reuse",
      recursiveSpendBundleWithTopupAnchorNullifiersAndTrailingAccumulatorField([
        initBundle.currentNote.spendNullifier,
      ]),
      "bundle.accumulator.topup_anchor_nullifiers must not reuse current note material",
    ],
    [
      "malformed proof cannot mask duplicate top-up anchors",
      recursiveSpendBundleWithTopupAnchorNullifiersAndEmptyProofBytes([
        initBundle.topupAnchorNullifiers[0],
        initBundle.topupAnchorNullifiers[0],
      ]),
      "bundle.accumulator.topup_anchor_nullifiers must be strictly sorted and unique",
    ],
    [
      "trailing accumulator cannot mask descending top-up anchors",
      recursiveSpendBundleWithTopupAnchorNullifiersAndTrailingAccumulatorField([
        initBundle.topupAnchorNullifiers[1],
        initBundle.topupAnchorNullifiers[0],
      ]),
      "bundle.accumulator.topup_anchor_nullifiers must be strictly sorted and unique",
    ],
  ];
  for (const [label, archive, expectedMessage] of maskedTopupAnchorPrecedenceCases) {
    assert.throws(
      () => decodeKagemushaRecursiveSpendBundle(archive),
      kagemushaRequestCodecError(
        "archive",
        "bundle.accumulator.topup_anchor_nullifiers",
        expectedMessage,
      ),
      label,
    );
  }

  const appendBundle = decodeKagemushaRecursiveSpendBundle(
    sharedRecursiveSpendAbi7Archive("append_bundle"),
  );
  assert.equal(appendBundle.asset, "7Y5nGzchCJcxcv98NUoBfwBR1nTk");
});

test("package dist Kagemusha recursive spend bundle rejects raw accumulator chain ids before native dispatch", () => {
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(
        recursiveSpendBundleWithAccumulatorField(
          1,
          kagemushaNoritoString("kagemusha-recursive-spend-abi-chain"),
        ),
      ),
    kagemushaRequestCodecError("archive", "bundle.accumulator.chain_id", null),
  );
});

test("package dist Kagemusha recursive spend bundle rejects empty or padded accumulator chain ids before native dispatch", () => {
  for (const chainId of [
    "",
    " kagemusha-recursive-spend-abi-chain",
    "kagemusha-recursive-spend-abi-chain ",
  ]) {
    assert.throws(
      () =>
        decodeKagemushaRecursiveSpendBundle(
          recursiveSpendBundleWithAccumulatorField(
            1,
            kagemushaNoritoField(kagemushaNoritoString(chainId)),
          ),
        ),
      kagemushaRequestCodecError(
        "field",
        "bundle.accumulator.chain_id",
        /non-empty unpadded string/,
      ),
    );
  }
});

test("package dist Kagemusha recursive spend bundle rejects nonportable accumulator chain ids before native dispatch", () => {
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(
        recursiveSpendBundleWithAccumulatorField(
          1,
          kagemushaNoritoField(kagemushaNoritoString("kagemusha recursive-spend-abi-chain")),
        ),
      ),
    kagemushaRequestCodecError(
      "field",
      "bundle.accumulator.chain_id",
      /portable registry syntax/,
    ),
  );
});

test("package dist Kagemusha recursive spend bundle rejects invalid accumulator field lengths before native dispatch", () => {
  const initBundle = decodeKagemushaRecursiveSpendBundle(
    sharedRecursiveSpendAbi6Archive("init_bundle"),
  );
  const invalidAccumulatorFields = [
    [3, Buffer.alloc(32), "bundle.accumulator.initial_root", null],
    [4, Buffer.alloc(32), "bundle.accumulator.final_root", null],
    [4, initBundle.initialRoot, "bundle.accumulator.final_root", null],
    [2, kagemushaFixedArrayPayload(0x01, 15), "bundle.accumulator.asset", null],
    [2, kagemushaFixedArrayPayload(0x01, 17), "bundle.accumulator.asset", null],
    [2, kagemushaCountPrefixedFixedArrayPayload(0x01, 16), "bundle.accumulator.asset", null],
    [3, kagemushaFixedArrayPayload(0x02, 31), "bundle.accumulator.initial_root", null],
    [3, kagemushaFixedArrayPayload(0x02, 33), "bundle.accumulator.initial_root", null],
    [
      3,
      kagemushaCountPrefixedFixedArrayPayload(0x02, 32),
      "bundle.accumulator.initial_root",
      null,
    ],
    [4, kagemushaFixedArrayPayload(0x03, 31), "bundle.accumulator.final_root", null],
    [4, kagemushaFixedArrayPayload(0x03, 33), "bundle.accumulator.final_root", null],
    [
      4,
      kagemushaCountPrefixedFixedArrayPayload(0x03, 32),
      "bundle.accumulator.final_root",
      null,
    ],
    [7, Buffer.alloc(32), "bundle.accumulator.lineage_digest", null],
    [7, kagemushaFixedArrayPayload(0x07, 31), "bundle.accumulator.lineage_digest", null],
    [7, kagemushaFixedArrayPayload(0x07, 33), "bundle.accumulator.lineage_digest", null],
    [
      7,
      kagemushaCountPrefixedFixedArrayPayload(0x07, 32),
      "bundle.accumulator.lineage_digest",
      null,
    ],
    [
      8,
      Buffer.alloc(32, 0x7d),
      "bundle.accumulator.aggregation_transcript_digest",
      null,
    ],
    [
      8,
      Buffer.alloc(32),
      "bundle.accumulator.aggregation_transcript_digest",
      null,
    ],
    [
      8,
      kagemushaFixedArrayPayload(0x08, 31),
      "bundle.accumulator.aggregation_transcript_digest",
      null,
    ],
    [
      8,
      kagemushaCountPrefixedFixedArrayPayload(0x08, 32),
      "bundle.accumulator.aggregation_transcript_digest",
      null,
    ],
    [
      8,
      kagemushaFixedArrayPayload(0x08, 33),
      "bundle.accumulator.aggregation_transcript_digest",
      null,
    ],
    [9, Buffer.alloc(32), "bundle.accumulator.nullifier_digest", null],
    [9, kagemushaFixedArrayPayload(0x09, 31), "bundle.accumulator.nullifier_digest", null],
    [
      9,
      kagemushaCountPrefixedFixedArrayPayload(0x09, 32),
      "bundle.accumulator.nullifier_digest",
      null,
    ],
    [9, kagemushaFixedArrayPayload(0x09, 33), "bundle.accumulator.nullifier_digest", null],
    [10, Buffer.alloc(32), "bundle.accumulator.output_commitment_digest", null],
    [10, kagemushaFixedArrayPayload(0x0a, 31), "bundle.accumulator.output_commitment_digest", null],
    [
      10,
      kagemushaCountPrefixedFixedArrayPayload(0x0a, 32),
      "bundle.accumulator.output_commitment_digest",
      null,
    ],
    [10, kagemushaFixedArrayPayload(0x0a, 33), "bundle.accumulator.output_commitment_digest", null],
    [11, Buffer.alloc(32), "bundle.accumulator.fold_digest", null],
    [11, kagemushaFixedArrayPayload(0x0b, 31), "bundle.accumulator.fold_digest", null],
    [
      11,
      kagemushaCountPrefixedFixedArrayPayload(0x0b, 32),
      "bundle.accumulator.fold_digest",
      null,
    ],
    [11, kagemushaFixedArrayPayload(0x0b, 33), "bundle.accumulator.fold_digest", null],
    [12, Buffer.alloc(32), "bundle.accumulator.recursive_proof_chain_digest", null],
    [
      12,
      kagemushaFixedArrayPayload(0x0c, 31),
      "bundle.accumulator.recursive_proof_chain_digest",
      null,
    ],
    [
      12,
      kagemushaCountPrefixedFixedArrayPayload(0x0c, 32),
      "bundle.accumulator.recursive_proof_chain_digest",
      null,
    ],
    [
      12,
      kagemushaFixedArrayPayload(0x0c, 33),
      "bundle.accumulator.recursive_proof_chain_digest",
      null,
    ],
    [13, Buffer.alloc(32), "bundle.accumulator.transition_profile_binding_digest", null],
    [
      13,
      kagemushaFixedArrayPayload(0x0d, 31),
      "bundle.accumulator.transition_profile_binding_digest",
      null,
    ],
    [
      13,
      kagemushaCountPrefixedFixedArrayPayload(0x0d, 32),
      "bundle.accumulator.transition_profile_binding_digest",
      null,
    ],
    [
      13,
      kagemushaFixedArrayPayload(0x0d, 33),
      "bundle.accumulator.transition_profile_binding_digest",
      null,
    ],
    [
      14,
      Buffer.alloc(32, 0x7e),
      "bundle.accumulator.append_opening_preflight_digest",
      null,
    ],
    [
      14,
      kagemushaFixedArrayPayload(0x0e, 31),
      "bundle.accumulator.append_opening_preflight_digest",
      null,
    ],
    [
      14,
      kagemushaFixedArrayPayload(0x0e, 33),
      "bundle.accumulator.append_opening_preflight_digest",
      null,
    ],
    [
      14,
      kagemushaCountPrefixedFixedArrayPayload(0x0e, 32),
      "bundle.accumulator.append_opening_preflight_digest",
      null,
    ],
    [
      15,
      Buffer.alloc(32, 0x7f),
      "bundle.accumulator.append_boundary_digest",
      null,
    ],
    [15, kagemushaFixedArrayPayload(0x0f, 31), "bundle.accumulator.append_boundary_digest", null],
    [
      15,
      kagemushaCountPrefixedFixedArrayPayload(0x0f, 32),
      "bundle.accumulator.append_boundary_digest",
      null,
    ],
    [15, kagemushaFixedArrayPayload(0x0f, 33), "bundle.accumulator.append_boundary_digest", null],
    [16, Buffer.alloc(32), "bundle.accumulator.verifier_params_fingerprint", null],
    [16, kagemushaFixedArrayPayload(0x10, 31), "bundle.accumulator.verifier_params_fingerprint", null],
    [
      16,
      kagemushaCountPrefixedFixedArrayPayload(0x10, 32),
      "bundle.accumulator.verifier_params_fingerprint",
      null,
    ],
    [16, kagemushaFixedArrayPayload(0x10, 33), "bundle.accumulator.verifier_params_fingerprint", null],
    [
      17,
      Buffer.alloc(32),
      "bundle.accumulator.fixed_window_table_schedule_digest",
      null,
    ],
    [
      17,
      kagemushaFixedArrayPayload(0x11, 31),
      "bundle.accumulator.fixed_window_table_schedule_digest",
      null,
    ],
    [
      17,
      kagemushaCountPrefixedFixedArrayPayload(0x11, 32),
      "bundle.accumulator.fixed_window_table_schedule_digest",
      null,
    ],
    [
      17,
      kagemushaFixedArrayPayload(0x11, 33),
      "bundle.accumulator.fixed_window_table_schedule_digest",
      null,
    ],
    [
      18,
      Buffer.alloc(32),
      "bundle.accumulator.fixed_window_shared_table_manifest_digest",
      null,
    ],
    [
      18,
      kagemushaFixedArrayPayload(0x12, 31),
      "bundle.accumulator.fixed_window_shared_table_manifest_digest",
      null,
    ],
    [
      18,
      kagemushaCountPrefixedFixedArrayPayload(0x12, 32),
      "bundle.accumulator.fixed_window_shared_table_manifest_digest",
      null,
    ],
    [
      18,
      kagemushaFixedArrayPayload(0x12, 33),
      "bundle.accumulator.fixed_window_shared_table_manifest_digest",
      null,
    ],
    [19, Buffer.alloc(32), "bundle.accumulator.fixed_window_table_base_digest", null],
    [19, kagemushaFixedArrayPayload(0x13, 31), "bundle.accumulator.fixed_window_table_base_digest", null],
    [
      19,
      kagemushaCountPrefixedFixedArrayPayload(0x13, 32),
      "bundle.accumulator.fixed_window_table_base_digest",
      null,
    ],
    [19, kagemushaFixedArrayPayload(0x13, 33), "bundle.accumulator.fixed_window_table_base_digest", null],
    [20, Buffer.alloc(32), "bundle.accumulator.verifier_witness_batch_digest", null],
    [
      20,
      kagemushaFixedArrayPayload(0x14, 31),
      "bundle.accumulator.verifier_witness_batch_digest",
      null,
    ],
    [
      20,
      kagemushaFixedArrayPayload(0x14, 33),
      "bundle.accumulator.verifier_witness_batch_digest",
      null,
    ],
    [
      20,
      kagemushaCountPrefixedFixedArrayPayload(0x14, 32),
      "bundle.accumulator.verifier_witness_batch_digest",
      null,
    ],
    [21, kagemushaU32Payload(3), "bundle.accumulator.verifier_opening_len", null],
    [
      21,
      kagemushaCountPrefixedFixedArrayPayload(0x15, 4),
      "bundle.accumulator.verifier_opening_len",
      null,
    ],
    [21, Buffer.from([2, 0, 0]), "bundle.accumulator.verifier_opening_len", null],
    [21, Buffer.from([2, 0, 0, 0, 0]), "bundle.accumulator.verifier_opening_len", null],
  ];
  const truncatedAccumulatorFieldLabels = [
    [0, "bundle.accumulator.domain"],
    [1, "bundle.accumulator.chain_id"],
    [2, "bundle.accumulator.asset"],
    [3, "bundle.accumulator.initial_root"],
    [4, "bundle.accumulator.final_root"],
  ];
  for (const [fieldIndex, label] of truncatedAccumulatorFieldLabels) {
    const expectedMessage =
      label === "bundle.accumulator.domain" ? "payload length mismatch" : "payload is truncated";
    assert.throws(
      () =>
        decodeKagemushaRecursiveSpendBundle(
          recursiveSpendBundleWithAccumulatorField(
            fieldIndex,
            kagemushaNoritoLength(1, TEST_NORITO_COMPACT_LEN_FLAG),
          ),
        ),
      new RegExp(`${label.replaceAll(".", "\\.")} ${expectedMessage}`),
    );
  }
  for (const [
    fieldIndex,
    replacement,
    expectedField,
    expectedMessage,
  ] of invalidAccumulatorFields) {
    assert.throws(
      () =>
        decodeKagemushaRecursiveSpendBundle(
          recursiveSpendBundleWithAccumulatorField(fieldIndex, replacement),
        ),
      kagemushaRequestCodecError("archive", expectedField, expectedMessage),
    );
  }
});

test("package dist Kagemusha recursive spend bundle rejects invalid accumulator hop counts before native dispatch", () => {
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(recursiveSpendBundleWithTrailingBundleField()),
    kagemushaRequestCodecError("archive", "bundle", /bundle has trailing bytes/),
  );
  for (const hopCount of [
    0,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 + 1,
  ]) {
    assert.throws(
      () =>
        decodeKagemushaRecursiveSpendBundle(
          recursiveSpendBundleWithAccumulatorField(6, kagemushaU32Payload(hopCount)),
        ),
      kagemushaRequestCodecError("archive", "bundle.accumulator.hop_count", null),
    );
  }
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(
        recursiveSpendBundleWithAccumulatorField(
          6,
          kagemushaCountPrefixedFixedArrayPayload(0x06, 4),
        ),
      ),
    kagemushaRequestCodecError("archive", "bundle.accumulator.hop_count", null),
  );
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(recursiveSpendBundleWithTrailingAccumulatorField()),
    kagemushaRequestCodecError("archive", "bundle", /accumulator has trailing bytes/),
  );
});

test("package dist Kagemusha recursive spend bundle rejects unsupported proof circuit ids before native dispatch", () => {
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(
        recursiveSpendBundleWithProofCircuitId(
          UNSUPPORTED_RECURSIVE_SPEND_PROOF_CIRCUIT_ID,
        ),
      ),
    kagemushaRequestCodecError("archive", "bundle.proof_circuit_id", null),
  );
});

test("package dist Kagemusha recursive spend bundle rejects unsupported proof backends before native dispatch", () => {
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(
        recursiveSpendBundleWithProofBackend("halo2/kzg"),
      ),
    kagemushaRequestCodecError("archive", "bundle.proof_backend", null),
  );
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(
        recursiveSpendBundleWithProofBoxBackend("halo2/kzg"),
      ),
    kagemushaRequestCodecError("archive", "bundle.proof_backend", null),
  );
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(
        recursiveSpendBundleWithProofBoxBackendAndEmptyProofBytes("halo2/kzg"),
      ),
    kagemushaRequestCodecError("archive", "bundle.proof_backend", null),
  );
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(
        recursiveSpendBundleWithTrailingVerifierKeyIdField(),
      ),
    kagemushaRequestCodecError("archive", "bundle", /verifierKeyId has trailing bytes/),
  );
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(
        recursiveSpendBundleWithTrailingRecursiveProofField(),
      ),
    kagemushaRequestCodecError("archive", "bundle", /recursiveProof has trailing bytes/),
  );
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(
        recursiveSpendBundleWithTrailingProofBoxField(),
      ),
    kagemushaRequestCodecError("archive", "bundle", /proof has trailing bytes/),
  );
});

test("package dist Kagemusha recursive spend bundle rejects empty proof bytes before native dispatch", () => {
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(recursiveSpendBundleWithEmptyProofBytes()),
    kagemushaRequestCodecError("archive", "bundle.proof_bytes", null),
  );
});

test("package dist Kagemusha recursive spend bundle rejects malformed proof public inputs before native dispatch", () => {
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(recursiveSpendBundleWithEmptyProofPublicInputs()),
    kagemushaRequestCodecError("archive", "bundle.proof_public_inputs", null),
  );
  for (const replacement of [
    kagemushaFixedArrayPayload(0x44, 31),
    kagemushaCountPrefixedFixedArrayPayload(0x44, 32),
    kagemushaFixedArrayPayload(0x44, 33),
  ]) {
    assert.throws(
      () =>
        decodeKagemushaRecursiveSpendBundle(
          recursiveSpendBundleWithProofPublicInputsHash(replacement),
        ),
      kagemushaRequestCodecError("archive", "bundle.proof_public_inputs_hash", null),
    );
  }
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(recursiveSpendBundleWithZeroProofPublicInputsHash()),
    kagemushaRequestCodecError("archive", "bundle.proof_public_inputs_hash", null),
  );
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(recursiveSpendBundleWithMismatchedProofPublicInputsHash()),
    kagemushaRequestCodecError("archive", "bundle.proof_public_inputs_hash", null),
  );
});

test("package dist Kagemusha recursive spend bundle rejects malformed current notes before native dispatch", () => {
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(
        recursiveSpendBundleWithCurrentNoteField(0, Buffer.alloc(32)),
      ),
    kagemushaRequestCodecError("field", "noteCommitment", null),
  );
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(
        recursiveSpendBundleWithCurrentNoteField(1, Buffer.alloc(32)),
      ),
    kagemushaRequestCodecError("field", "spendNullifier", null),
  );
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(
        recursiveSpendBundleWithEqualCurrentNoteNullifier(),
      ),
    kagemushaRequestCodecError("field", "spendNullifier", null),
  );
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(
        recursiveSpendBundleWithCurrentNoteField(2, kagemushaZeroNumericPayload()),
      ),
    kagemushaRequestCodecError(
      "field",
      "bundle.accumulator.current_note.amount",
      /numeric amount must be greater than zero/,
    ),
  );
  const malformedCurrentNoteFieldLengths = [
    [0, kagemushaFixedArrayPayload(0x04, 31), "archive", "bundle.accumulator.current_note.note_commitment", null],
    [0, kagemushaFixedArrayPayload(0x04, 33), "archive", "bundle.accumulator.current_note.note_commitment", null],
    [
      0,
      kagemushaCountPrefixedFixedArrayPayload(0x04, 32),
      "archive",
      "bundle.accumulator.current_note.note_commitment",
      null,
    ],
    [1, kagemushaFixedArrayPayload(0x05, 31), "archive", "bundle.accumulator.current_note.spend_nullifier", null],
    [1, kagemushaFixedArrayPayload(0x05, 33), "archive", "bundle.accumulator.current_note.spend_nullifier", null],
    [
      1,
      kagemushaCountPrefixedFixedArrayPayload(0x05, 32),
      "archive",
      "bundle.accumulator.current_note.spend_nullifier",
      null,
    ],
    [
      2,
      kagemushaNumericPayload(Buffer.from([1]), 1),
      "field",
      "bundle.accumulator.current_note.amount",
      /numeric scale/,
    ],
    [
      2,
      kagemushaNumericPayloadWithScalePayload(
        kagemushaCountPrefixedFixedArrayPayload(0x16, 4),
      ),
      "field",
      "bundle.accumulator.current_note.amount",
      /numeric scale/,
    ],
    [
      2,
      kagemushaNumericPayloadWithMantissaPayload(Buffer.from([2, 0, 0, 0, 1])),
      "archive",
      "bundle.accumulator.current_note.amount",
      null,
    ],
    [
      2,
      kagemushaNumericPayload(Buffer.from([0xff])),
      "field",
      "bundle.accumulator.current_note.amount",
      /numeric amount must be greater than zero/,
    ],
    [
      2,
      kagemushaNumericPayload(Buffer.concat([Buffer.alloc(16), Buffer.from([1])])),
      "field",
      "bundle.accumulator.current_note.amount",
      /fit in u128/,
    ],
    [
      2,
      kagemushaNumericPayloadWithTrailingField(),
      "archive",
      "bundle.accumulator.current_note.amount",
      null,
    ],
  ];
  for (const [
    fieldIndex,
    replacement,
    expectedKind,
    expectedField,
    expectedMessage,
  ] of malformedCurrentNoteFieldLengths) {
    assert.throws(
      () =>
        decodeKagemushaRecursiveSpendBundle(
          recursiveSpendBundleWithCurrentNoteField(fieldIndex, replacement),
        ),
      kagemushaRequestCodecError(expectedKind, expectedField, expectedMessage),
    );
  }
  const currentNoteValidationPrecedenceCases = [
    [
      recursiveSpendBundleWithCurrentNoteFieldAndTrailingField(0, Buffer.alloc(32)),
      "field",
      "noteCommitment",
      null,
    ],
    [
      recursiveSpendBundleWithCurrentNoteFieldAndTrailingField(1, Buffer.alloc(32)),
      "field",
      "spendNullifier",
      null,
    ],
    [
      recursiveSpendBundleWithEqualCurrentNoteNullifierAndTrailingField(),
      "field",
      "spendNullifier",
      null,
    ],
    [
      recursiveSpendBundleWithCurrentNoteFieldAndTrailingField(
        2,
        kagemushaZeroNumericPayload(),
      ),
      "field",
      "bundle.accumulator.current_note.amount",
      /numeric amount must be greater than zero/,
    ],
  ];
  for (const [archive, expectedKind, expectedField, expectedMessage]
    of currentNoteValidationPrecedenceCases) {
    assert.throws(
      () => decodeKagemushaRecursiveSpendBundle(archive),
      kagemushaRequestCodecError(expectedKind, expectedField, expectedMessage),
    );
  }
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(recursiveSpendBundleWithTrailingCurrentNoteField()),
    kagemushaRequestCodecError(
      "archive",
      "bundle.accumulator.current_note",
      /currentNote has trailing bytes/,
    ),
  );
});

test("package dist Kagemusha recursive compact requires key packages before native dispatch", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const calls = [];
  const recordBundle = privacyNoritoFrameWithPayload(0x4a);
  const pallasOpenEnvelopes = privacyNoritoFrameWithPayload(0x4b);
  const keyArtifacts = privacyNoritoFrameWithPayload(0x4c);
  const compactToken = privacyNoritoFrameWithPayload(0x4d);
  const verifierKeys = privacyNoritoFrameWithPayload(0x4e);
  const nativeOutput = privacyNoritoFrameWithPayload(0x4f);
  const isProbeCall = (args) =>
    args.every((arg) => {
      const bytes = Buffer.from(arg);
      return bytes.length === 1 && bytes[0] === 0;
    });

  try {
    globalThis.__IROHA_NATIVE_BINDING__ = {
      connectNoritoBridgeAbiVersion() {
        return KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION;
      },
      kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        ...args
      ) {
        if (isProbeCall(args)) {
          throw new Error("Kagemusha probe archive rejected");
        }
        calls.push(["prove", args]);
        return nativeOutput;
      },
      kagemushaVerifyRecursiveCompactPaymentToken(...args) {
        if (isProbeCall(args)) {
          throw new Error("Kagemusha probe archive rejected");
        }
        calls.push(["verify", args]);
        return true;
      },
    };

    assert.throws(
      () =>
        kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
          recordBundle,
          pallasOpenEnvelopes,
        ),
      /recursiveCompactKeyArtifactsArchive must be a Buffer, string, or ArrayBuffer view/,
    );
    assert.throws(
      () =>
        kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
          recordBundle,
          pallasOpenEnvelopes,
          Buffer.alloc(0),
        ),
      /recursiveCompactKeyArtifactsArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaVerifyRecursiveCompactPaymentToken(compactToken),
      /recursiveCompactVerifierKeysArchive must be a Buffer, string, or ArrayBuffer view/,
    );
    assert.throws(
      () =>
        kagemushaVerifyRecursiveCompactPaymentToken(
          compactToken,
          Buffer.alloc(0),
        ),
      /recursiveCompactVerifierKeysArchive must not be empty/,
    );
    assert.deepEqual(calls, []);

    assert.deepEqual(
      kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        recordBundle,
        pallasOpenEnvelopes,
        keyArtifacts,
      ),
      nativeOutput,
    );
    assert.equal(
      kagemushaVerifyRecursiveCompactPaymentToken(compactToken, verifierKeys),
      true,
    );
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }

  assert.equal(calls.length, 2);
  assert.equal(calls[0][0], "prove");
  assert.equal(calls[0][1].length, 3);
  assert.notStrictEqual(calls[0][1][2], keyArtifacts);
  assert.deepEqual(calls[0][1][2], keyArtifacts);
  keyArtifacts[0] ^= 0xff;
  assert.notDeepEqual(calls[0][1][2], keyArtifacts);

  assert.equal(calls[1][0], "verify");
  assert.equal(calls[1][1].length, 2);
  assert.notStrictEqual(calls[1][1][1], verifierKeys);
  assert.deepEqual(calls[1][1][1], verifierKeys);
  verifierKeys[0] ^= 0xff;
  assert.notDeepEqual(calls[1][1][1], verifierKeys);
});

test("package dist Kagemusha recursive spend compact projection helpers dispatch owned archives", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const calls = [];
  const bundleArchive = Uint8Array.from(privacyNoritoFrameWithPayload(0x81));
  const compactTokenArchive = Uint8Array.from(privacyNoritoFrameWithPayload(0x82));
  const verifierRecordArchive = Uint8Array.from(privacyNoritoFrameWithPayload(0x83));
  const expectedBundleArchive = Buffer.from(bundleArchive);
  const expectedCompactTokenArchive = Buffer.from(compactTokenArchive);
  const expectedVerifierRecordArchive = Buffer.from(verifierRecordArchive);
  const nativeProjectionOutput = Uint8Array.from(privacyNoritoFrameWithPayload(0x84));
  const expectedProjectionOutput = Buffer.from(nativeProjectionOutput);
  const isProbeCall = (archives) =>
    archives.length > 0 &&
    archives.every((archive) => {
      const bytes = Buffer.from(archive);
      return bytes.length === 1 && bytes[0] === 0;
    });

  try {
    globalThis.__IROHA_NATIVE_BINDING__ = {
      connectNoritoBridgeAbiVersion() {
        return KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION;
      },
      kagemushaRecursiveSpendCompactPaymentTokenFromBundle(bundle) {
        if (isProbeCall([bundle])) {
          throw new Error("Kagemusha probe archive rejected");
        }
        calls.push(["project", bundle]);
        return nativeProjectionOutput;
      },
      kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(token, record) {
        if (isProbeCall([token, record])) {
          throw new Error("Kagemusha probe archive rejected");
        }
        calls.push(["verify", token, record]);
        return false;
      },
      kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
        token,
        record,
        blockHeight,
      ) {
        if (isProbeCall([token, record])) {
          throw new Error("Kagemusha probe archive rejected");
        }
        calls.push(["verify-at-height", token, record, blockHeight]);
        return true;
      },
    };

    assert.equal(
      isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable(),
      true,
    );
    assert.equal(
      isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable(),
      true,
    );

    const projection = kagemushaRecursiveSpendCompactPaymentTokenFromBundle(
      bundleArchive,
    );
    assert.ok(Buffer.isBuffer(projection));
    assert.deepEqual(projection, expectedProjectionOutput);
    assert.notStrictEqual(projection, nativeProjectionOutput);

    assert.equal(
      kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
        compactTokenArchive,
        verifierRecordArchive,
      ),
      false,
    );
    assert.equal(
      kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
        compactTokenArchive,
        verifierRecordArchive,
        2,
      ),
      true,
    );
    assert.equal(
      kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
        compactTokenArchive,
        verifierRecordArchive,
        0xffff_ffff_ffff_ffffn,
      ),
      true,
    );

    bundleArchive[6] ^= 0x7f;
    compactTokenArchive[6] ^= 0x7f;
    verifierRecordArchive[6] ^= 0x7f;
    nativeProjectionOutput[6] ^= 0x7f;

    assert.deepEqual(calls.map((call) => call[0]), [
      "project",
      "verify",
      "verify-at-height",
      "verify-at-height",
    ]);
    assert.notStrictEqual(calls[0][1], bundleArchive);
    assert.deepEqual(calls[0][1], expectedBundleArchive);
    for (const call of calls.slice(1)) {
      assert.notStrictEqual(call[1], compactTokenArchive);
      assert.notStrictEqual(call[2], verifierRecordArchive);
      assert.deepEqual(call[1], expectedCompactTokenArchive);
      assert.deepEqual(call[2], expectedVerifierRecordArchive);
    }
    assert.deepEqual(calls[2].slice(3), [2]);
    assert.deepEqual(calls[3].slice(3), [0xffff_ffff_ffff_ffffn]);
    assert.deepEqual(projection, expectedProjectionOutput);

    const callsBeforeInvalidHeights = calls.length;
    for (const [badHeight, errorPattern] of [
      [true, /blockHeight must be a number or bigint/],
      [false, /blockHeight must be a number or bigint/],
      ["1", /blockHeight must be a number or bigint/],
      [{ value: 1 }, /blockHeight must be a number or bigint/],
      [1.5, /blockHeight must be an integer/],
      [Number.NaN, /blockHeight must be an integer/],
      [Infinity, /blockHeight must be an integer/],
      [-1, /blockHeight must be non-negative/],
      [-0, /blockHeight must be non-negative/],
      [-1n, /blockHeight must be non-negative/],
      [Number.MAX_SAFE_INTEGER + 1, /blockHeight number must be a safe integer/],
      [0x1_0000_0000_0000_0000n, /blockHeight must fit in u64/],
    ]) {
      assert.throws(
        () =>
          kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
            expectedCompactTokenArchive,
            expectedVerifierRecordArchive,
            badHeight,
          ),
        errorPattern,
      );
      assert.equal(calls.length, callsBeforeInvalidHeights);
    }
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist Kagemusha recursive spend compact projection helpers fail closed on invalid archives", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  let nativeDispatches = 0;
  const rejectNativeDispatch = (...archives) => {
    const probeArchives = archives.length === 3 ? archives.slice(0, 2) : archives;
    if (
      probeArchives.length > 0 &&
      probeArchives.every((archive) => {
        const bytes = Buffer.from(archive);
        return bytes.length === 1 && bytes[0] === 0;
      })
    ) {
      throw new Error("Kagemusha probe archive rejected");
    }
    nativeDispatches += 1;
    throw new Error("native compact projection dispatch should not run");
  };
  const validArchive = privacyNoritoFrameWithPayload(0x85);
  const invalidArchives = [
    [Buffer.alloc(0), "must not be empty"],
    [Buffer.alloc(KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f), "must not exceed"],
    [Buffer.from([0x01]), "must be a valid Norito archive"],
    [privacyNoritoFrame(0x85), "must contain a non-empty Norito payload"],
    [undefined, "must be a Buffer, string, or ArrayBuffer view"],
  ];

  try {
    globalThis.__IROHA_NATIVE_BINDING__ = {
      connectNoritoBridgeAbiVersion() {
        return KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION;
      },
      kagemushaRecursiveSpendCompactPaymentTokenFromBundle: rejectNativeDispatch,
      kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection: rejectNativeDispatch,
      kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight:
        rejectNativeDispatch,
    };

    assert.equal(
      isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable(),
      true,
    );
    assert.equal(
      isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable(),
      true,
    );
    for (const [invalidArchive, expectedMessage] of invalidArchives) {
      assert.throws(
        () => kagemushaRecursiveSpendCompactPaymentTokenFromBundle(invalidArchive),
        new RegExp(`bundleArchive ${expectedMessage}`),
      );
      assert.throws(
        () =>
          kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
            invalidArchive,
            validArchive,
          ),
        new RegExp(`compactTokenArchive ${expectedMessage}`),
      );
      assert.throws(
        () =>
          kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
            invalidArchive,
            validArchive,
            9,
          ),
        new RegExp(`compactTokenArchive ${expectedMessage}`),
      );
      assert.throws(
        () =>
          kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
            validArchive,
            invalidArchive,
            9,
          ),
        new RegExp(`verifierRecordArchive ${expectedMessage}`),
      );
      assert.throws(
        () =>
          kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
            validArchive,
            invalidArchive,
          ),
          new RegExp(`verifierRecordArchive ${expectedMessage}`),
      );
    }
    assert.throws(
      () =>
        kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
          validArchive,
          Buffer.from([0x01]),
        ),
      /verifierRecordArchive must be a valid Norito archive/,
    );
    assert.equal(nativeDispatches, 0);

    globalThis.__IROHA_NATIVE_BINDING__ = {
      connectNoritoBridgeAbiVersion() {
        return KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION;
      },
      kagemushaRecursiveSpendCompactPaymentTokenFromBundle(bundle) {
        if (Buffer.from(bundle).length === 1) {
          throw new Error("Kagemusha probe archive rejected");
        }
        return Buffer.from([0x01]);
      },
      kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(token, record) {
        if (Buffer.from(token).length === 1 && Buffer.from(record).length === 1) {
          throw new Error("Kagemusha probe archive rejected");
        }
        return Buffer.from([0x01]);
      },
      kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight() {
        throw new Error("Kagemusha probe archive rejected");
      },
    };
    assert.throws(
      () => kagemushaRecursiveSpendCompactPaymentTokenFromBundle(validArchive),
      /returned invalid Norito archive/,
    );
    assert.throws(
      () =>
        kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
          validArchive,
          validArchive,
        ),
      /kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection returned a non-boolean result/,
    );
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist Kagemusha recursive spend availability rejects coerced ABI versions", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  try {
    for (const abiVersion of [
      "18",
      true,
      -1,
      18.5,
      Number.NaN,
      Number.POSITIVE_INFINITY,
      Number.MAX_SAFE_INTEGER + 1,
      0x1_0000_0000,
    ]) {
      globalThis.__IROHA_NATIVE_BINDING__ = {
        connectNoritoBridgeAbiVersion() {
          return abiVersion;
        },
        kagemushaRecursiveSpendInit() {
          return Uint8Array.from([1]);
        },
        kagemushaRecursiveSpendAppend() {
          return Uint8Array.from([2]);
        },
        kagemushaRecursiveSpendLineageWitnessFromInitResult() {
          return Uint8Array.from([3]);
        },
        kagemushaRecursiveSpendLineageWitnessAppendResult() {
          return Uint8Array.from([4]);
        },
        kagemushaRecursiveSpendVerify() {
          return Uint8Array.from([5]);
        },
        kagemushaRecursiveSpendRedeem() {
          return Uint8Array.from([6]);
        },
        kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes() {
          return Uint8Array.from([10]);
        },
        kagemushaVerifyRecursiveCompactPaymentToken() {
          return true;
        },
      };

      assert.equal(isKagemushaRecursiveSpendNativeAvailable(), false);
      assert.equal(isKagemushaRecursiveCompactPaymentTokenNativeAvailable(), false);
      assert.equal(
        isKagemushaRecursiveCompactPaymentTokenNativeAvailable(),
        false,
      );
      assert.equal(
        isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable(),
        false,
      );
    }
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist Kagemusha recursive spend availability rejects broken and permissive native probes", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
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
  const isProbeCall = (archives) =>
    archives.length > 0 &&
    archives.every((archive) => {
      const bytes = Buffer.from(archive);
      return bytes.length === 1 && bytes[0] === 0;
    });
  const rejectProbe = (...archives) => {
    if (isProbeCall(archives)) {
      throw new Error("Kagemusha probe archive rejected");
    }
    return privacyNoritoFrameWithPayload(0x31);
  };
  const completeBinding = (overrides = {}) => ({
    connectNoritoBridgeAbiVersion() {
      return KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION;
    },
    kagemushaRecursiveSpendInit: rejectProbe,
    kagemushaRecursiveSpendAppend: rejectProbe,
    kagemushaRecursiveSpendTransitionProfileInit: rejectProbe,
    kagemushaRecursiveSpendTransitionProfileAppend: rejectProbe,
    kagemushaRecursiveSpendLineageAppendBoundary: rejectProbe,
    kagemushaRecursiveSpendLineageWitnessFromInitResult: rejectProbe,
    kagemushaRecursiveSpendLineageWitnessAppendResult: rejectProbe,
    kagemushaRecursiveSpendVerify: rejectProbe,
    kagemushaRecursiveSpendRedeem: rejectProbe,
    ...overrides,
  });

  try {
    globalThis.__IROHA_NATIVE_BINDING__ = completeBinding({
      connectNoritoBridgeAbiVersion() {
        throw new Error("bridge denied");
      },
    });
    assert.equal(isKagemushaRecursiveSpendNativeAvailable(), false);
    assert.equal(preferredKagemushaOfflineSpendMode(), null);
    assert.throws(
      () => kagemushaRecursiveSpendInit(privacyNoritoFrameWithPayload(0x41)),
      /Kagemusha recursive spend helper 'kagemushaRecursiveSpendInit' is unavailable/,
    );

    for (const acceptedMethod of acceptedMethods) {
      globalThis.__IROHA_NATIVE_BINDING__ = completeBinding({
        [acceptedMethod]() {
          return Uint8Array.from([0xff]);
        },
      });
      assert.equal(
        isKagemushaRecursiveSpendNativeAvailable(),
        false,
        acceptedMethod,
      );
      assert.equal(
        preferredKagemushaOfflineSpendMode(),
        null,
        acceptedMethod,
      );
      assert.throws(
        () => kagemushaRecursiveSpendVerify(privacyNoritoFrameWithPayload(0x42)),
        /Kagemusha recursive spend helper 'kagemushaRecursiveSpendVerify' is unavailable/,
        acceptedMethod,
      );
    }
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist Kagemusha recursive spend availability rejects partial ABI-18 surfaces", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
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
  const rejectProbe = (...archives) => {
    if (
      archives.length > 0 &&
      archives.every((archive) => {
        const bytes = Buffer.from(archive);
        return bytes.length === 1 && bytes[0] === 0;
      })
    ) {
      throw new Error("Kagemusha probe archive rejected");
    }
    return privacyNoritoFrameWithPayload(0x31);
  };
  const completeBinding = () => ({
    connectNoritoBridgeAbiVersion() {
      return KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION;
    },
    kagemushaRecursiveSpendInit: rejectProbe,
    kagemushaRecursiveSpendAppend: rejectProbe,
    kagemushaRecursiveSpendTransitionProfileInit: rejectProbe,
    kagemushaRecursiveSpendTransitionProfileAppend: rejectProbe,
    kagemushaRecursiveSpendLineageAppendBoundary: rejectProbe,
    kagemushaRecursiveSpendLineageWitnessFromInitResult: rejectProbe,
    kagemushaRecursiveSpendLineageWitnessAppendResult: rejectProbe,
    kagemushaRecursiveSpendVerify: rejectProbe,
    kagemushaRecursiveSpendRedeem: rejectProbe,
  });

  try {
    for (const missingMethod of requiredMethods) {
      const binding = completeBinding();
      delete binding[missingMethod];
      globalThis.__IROHA_NATIVE_BINDING__ = binding;
      assert.equal(
        isKagemushaRecursiveSpendNativeAvailable(),
        false,
        missingMethod,
      );
      assert.equal(
        preferredKagemushaOfflineSpendMode(),
        null,
        missingMethod,
      );
      assert.throws(
        () => kagemushaRecursiveSpendVerify(privacyNoritoFrameWithPayload(0x35)),
        /Kagemusha recursive spend helper 'kagemushaRecursiveSpendVerify' is unavailable/,
        missingMethod,
      );
    }
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist Kagemusha recursive spend helpers reject unsafe native outputs", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const request = privacyNoritoFrameWithPayload(0x35);
  const isProbeCall = (archives) =>
    archives.length > 0 &&
    archives.every((archive) => {
      const bytes = Buffer.from(archive);
      return bytes.length === 1 && bytes[0] === 0;
    });
  const nativeMethods = {
    kagemushaRecursiveSpendInit(...archives) {
      if (isProbeCall(archives)) {
        throw new Error("Kagemusha probe archive rejected");
      }
      return privacyNoritoFrameWithPayload(0x31);
    },
    kagemushaRecursiveSpendAppend(...archives) {
      if (isProbeCall(archives)) {
        throw new Error("Kagemusha probe archive rejected");
      }
      return privacyNoritoFrameWithPayload(0x32);
    },
    kagemushaRecursiveSpendTransitionProfileInit(...archives) {
      if (isProbeCall(archives)) {
        throw new Error("Kagemusha probe archive rejected");
      }
      return privacyNoritoFrameWithPayload(0x37);
    },
    kagemushaRecursiveSpendTransitionProfileAppend(...archives) {
      if (isProbeCall(archives)) {
        throw new Error("Kagemusha probe archive rejected");
      }
      return privacyNoritoFrameWithPayload(0x38);
    },
    kagemushaRecursiveSpendLineageAppendBoundary(...archives) {
      if (isProbeCall(archives)) {
        throw new Error("Kagemusha probe archive rejected");
      }
      return privacyNoritoFrameWithPayload(0x39);
    },
    kagemushaRecursiveSpendLineageWitnessFromInitResult(...archives) {
      if (isProbeCall(archives)) {
        throw new Error("Kagemusha probe archive rejected");
      }
      return privacyNoritoFrameWithPayload(0x33);
    },
    kagemushaRecursiveSpendLineageWitnessAppendResult(...archives) {
      if (isProbeCall(archives)) {
        throw new Error("Kagemusha probe archive rejected");
      }
      return privacyNoritoFrameWithPayload(0x34);
    },
    kagemushaRecursiveSpendVerify(...archives) {
      if (isProbeCall(archives)) {
        throw new Error("Kagemusha probe archive rejected");
      }
      return privacyNoritoFrameWithPayload(0x35);
    },
    kagemushaRecursiveSpendRedeem(...archives) {
      if (isProbeCall(archives)) {
        throw new Error("Kagemusha probe archive rejected");
      }
      return privacyNoritoFrameWithPayload(0x36);
    },
  };
  const completeBinding = (methodName, output) => ({
    connectNoritoBridgeAbiVersion() {
      return KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION;
    },
    ...nativeMethods,
    [methodName](...archives) {
      if (isProbeCall(archives)) {
        throw new Error("Kagemusha probe archive rejected");
      }
      return output;
    },
  });
  const calls = [
    [
      "kagemushaRecursiveSpendInit",
      () => kagemushaRecursiveSpendInit(request),
    ],
    [
      "kagemushaRecursiveSpendAppend",
      () => kagemushaRecursiveSpendAppend(request),
    ],
    [
      "kagemushaRecursiveSpendTransitionProfileInit",
      () => kagemushaRecursiveSpendTransitionProfileInit(request),
    ],
    [
      "kagemushaRecursiveSpendTransitionProfileAppend",
      () => kagemushaRecursiveSpendTransitionProfileAppend(request),
    ],
    [
      "kagemushaRecursiveSpendLineageAppendBoundary",
      () => kagemushaRecursiveSpendLineageAppendBoundary(request),
    ],
    [
      "kagemushaRecursiveSpendLineageWitnessFromInitResult",
      () => kagemushaRecursiveSpendLineageWitnessFromInitResult(request, request),
    ],
    [
      "kagemushaRecursiveSpendLineageWitnessAppendResult",
      () => kagemushaRecursiveSpendLineageWitnessAppendResult(request, request, request),
    ],
    [
      "kagemushaRecursiveSpendVerify",
      () => kagemushaRecursiveSpendVerify(request),
    ],
    [
      "kagemushaRecursiveSpendRedeem",
      () => kagemushaRecursiveSpendRedeem(request),
    ],
  ];
  const invalidOutputs = [
    [Buffer.alloc(0), /returned empty output/],
    [null, /returned no output/],
    ["not-bytes", /returned text instead of Norito bytes/],
    [
      Buffer.alloc(KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f),
      /returned oversized output/,
    ],
    [Buffer.from([0x01]), /returned invalid Norito archive/],
    [privacyNoritoFrame(0x36), /returned empty Norito payload/],
  ];

  try {
    for (const [methodName, call] of calls) {
      for (const [output, expectedError] of invalidOutputs) {
        globalThis.__IROHA_NATIVE_BINDING__ = completeBinding(methodName, output);
        assert.throws(call, expectedError, `${methodName} ${expectedError}`);
      }
    }
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist Kagemusha recursive spend helpers reject invalid request archives before native dispatch", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  let nativeDispatches = 0;
  const rejectNativeDispatch = () => {
    nativeDispatches += 1;
    throw new Error("native dispatch should not run for invalid request archives");
  };
  const validArchive = privacyNoritoFrameWithPayload(0x35);
  const invalidArchives = [
    [Buffer.alloc(0), "must not be empty"],
    [
      new Uint8Array(KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES + 1),
      "must not exceed",
    ],
    [Buffer.from([0x01]), "must be a valid Norito archive"],
    [privacyNoritoFrame(0x35), "must contain a non-empty Norito payload"],
    [undefined, "must be a Buffer, string, or ArrayBuffer view"],
  ];
  const helperCases = [
    [
      "kagemushaRecursiveSpendInit",
      ["requestArchive"],
      (args) => kagemushaRecursiveSpendInit(args[0]),
    ],
    [
      "kagemushaRecursiveSpendAppend",
      ["requestArchive"],
      (args) => kagemushaRecursiveSpendAppend(args[0]),
    ],
    [
      "kagemushaRecursiveSpendTransitionProfileInit",
      ["requestArchive"],
      (args) => kagemushaRecursiveSpendTransitionProfileInit(args[0]),
    ],
    [
      "kagemushaRecursiveSpendTransitionProfileAppend",
      ["requestArchive"],
      (args) => kagemushaRecursiveSpendTransitionProfileAppend(args[0]),
    ],
    [
      "kagemushaRecursiveSpendLineageAppendBoundary",
      ["profileArchive"],
      (args) => kagemushaRecursiveSpendLineageAppendBoundary(args[0]),
    ],
    [
      "kagemushaRecursiveSpendLineageWitnessFromInitResult",
      ["requestArchive", "bundleArchive"],
      (args) => kagemushaRecursiveSpendLineageWitnessFromInitResult(args[0], args[1]),
    ],
    [
      "kagemushaRecursiveSpendLineageWitnessAppendResult",
      ["previousWitnessArchive", "requestArchive", "bundleArchive"],
      (args) =>
        kagemushaRecursiveSpendLineageWitnessAppendResult(
          args[0],
          args[1],
          args[2],
        ),
    ],
    [
      "kagemushaRecursiveSpendVerify",
      ["requestArchive"],
      (args) => kagemushaRecursiveSpendVerify(args[0]),
    ],
    [
      "kagemushaRecursiveSpendRedeem",
      ["requestArchive"],
      (args) => kagemushaRecursiveSpendRedeem(args[0]),
    ],
  ];

  try {
    globalThis.__IROHA_NATIVE_BINDING__ = {
      connectNoritoBridgeAbiVersion() {
        return KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION;
      },
      kagemushaRecursiveSpendInit: rejectNativeDispatch,
      kagemushaRecursiveSpendAppend: rejectNativeDispatch,
      kagemushaRecursiveSpendTransitionProfileInit: rejectNativeDispatch,
      kagemushaRecursiveSpendTransitionProfileAppend: rejectNativeDispatch,
      kagemushaRecursiveSpendLineageAppendBoundary: rejectNativeDispatch,
      kagemushaRecursiveSpendLineageWitnessFromInitResult: rejectNativeDispatch,
      kagemushaRecursiveSpendLineageWitnessAppendResult: rejectNativeDispatch,
      kagemushaRecursiveSpendVerify: rejectNativeDispatch,
      kagemushaRecursiveSpendRedeem: rejectNativeDispatch,
    };

    for (const [helperName, archiveNames, call] of helperCases) {
      for (let fieldIndex = 0; fieldIndex < archiveNames.length; fieldIndex += 1) {
        for (const [invalidArchive, expectedMessage] of invalidArchives) {
          const args = Array.from({ length: archiveNames.length }, () => validArchive);
          args[fieldIndex] = invalidArchive;
          assert.throws(
            () => call(args),
            new RegExp(`${archiveNames[fieldIndex]} ${expectedMessage}`),
            `${helperName} ${archiveNames[fieldIndex]} ${expectedMessage}`,
          );
        }
      }
    }
    assert.equal(nativeDispatches, 0);
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist entrypoint exports privacy native archive helpers", () => {
  const declarationExports = declarationExportNames();
  const expected = [
    "PRIVACY_FFI_VERSION_V1",
    "PRIVACY_REQUIRED_BRIDGE_ABI_VERSION",
    "PRIVACY_FFI_STATUS_ERROR",
    "PRIVACY_FFI_ERROR_NULL_POINTER",
    "PRIVACY_FFI_ERROR_MALFORMED_NORITO",
    "PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM",
    "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED",
    "PRIVACY_FFI_ERROR_INVALID_REQUEST",
    "isPrivacyNativeAvailable",
    "privacyCapabilitiesV1",
    "privacyProofRequestV1",
    "privacyBuildProofV1",
    "privacyVerifyProofV1",
    "getPrivacyCapabilities",
  ];

  for (const name of expected) {
    assert.match(DIST_INDEX_TEXT, new RegExp(`\\b${name}\\b`, "u"));
    assert.ok(
      declarationExports.has(name),
      `missing declaration export ${name}`,
    );
  }
  assert.equal(PRIVACY_FFI_VERSION_V1, 1);
  assert.equal(PRIVACY_REQUIRED_BRIDGE_ABI_VERSION, 7);
  assert.equal(PRIVACY_FFI_STATUS_ERROR, 1);
  assert.equal(PRIVACY_FFI_ERROR_NULL_POINTER, 1);
  assert.equal(PRIVACY_FFI_ERROR_MALFORMED_NORITO, 2);
  assert.equal(PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM, 3);
  assert.equal(PRIVACY_FFI_ERROR_PRODUCTION_DISABLED, 4);
  assert.equal(PRIVACY_FFI_ERROR_INVALID_REQUEST, 5);
  assert.equal(typeof isPrivacyNativeAvailable(), "boolean");
  for (const helper of [
    privacyCapabilitiesV1,
    privacyProofRequestV1,
    privacyBuildProofV1,
    privacyVerifyProofV1,
  ]) {
    assert.equal(typeof helper, "function");
  }
  const capabilities = getPrivacyCapabilities();
  assert.equal(capabilities.javascriptSdkAvailable, true);
  assert.equal(typeof capabilities.bridgeAvailable, "boolean");
  assert.deepEqual(Object.keys(capabilities).sort(), [
    "bridgeAvailable",
    "javascriptSdkAvailable",
    "privacyAlgorithms",
    "privacyCriteria",
  ]);
  assert.equal(
    capabilities.privacyAlgorithms.every(
      (descriptor) => descriptor.productionReady === false,
    ),
    true,
  );
  assert.equal(
    capabilities.privacyAlgorithms.every(
      (descriptor) => descriptor.productionGate.ready === false,
    ),
    true,
  );
  assert.equal(Object.isFrozen(capabilities), true);
  assert.equal(Object.isFrozen(capabilities.privacyAlgorithms), true);
  assert.equal(Object.isFrozen(capabilities.privacyAlgorithms[0]), true);
  assert.equal(
    Object.isFrozen(capabilities.privacyAlgorithms[0].productionGate),
    true,
  );
  assert.equal(
    Object.isFrozen(capabilities.privacyAlgorithms[0].productionGate.gates),
    true,
  );
  assert.equal(
    Object.isFrozen(capabilities.privacyAlgorithms[0].productionGate.missing),
    true,
  );
  assert.equal(Object.isFrozen(capabilities.privacyCriteria), true);
  assert.throws(() => {
    capabilities.privacyAlgorithms[0].productionReady = true;
  });
  assert.throws(() => {
    capabilities.privacyAlgorithms[0].productionGate.ready = true;
  });
  assert.throws(() => {
    capabilities.privacyAlgorithms[0].productionGate.gates.external_audit = true;
  });
  assert.throws(() => {
    capabilities.privacyAlgorithms[0].productionGate.missing.length = 0;
  });
  assert.throws(() => {
    capabilities.privacyCriteria.push("tampered");
  });
  const fresh = getPrivacyCapabilities();
  assert.equal(fresh.privacyAlgorithms[0].productionReady, false);
  assert.equal(fresh.privacyAlgorithms[0].productionGate.ready, false);
  assert.equal(
    fresh.privacyAlgorithms[0].productionGate.gates.external_audit,
    false,
  );
  assert.ok(
    fresh.privacyAlgorithms[0].productionGate.missing.includes(
      "internal cryptographic review signoff is missing",
    ),
  );
  assert.deepEqual(fresh.privacyCriteria, capabilities.privacyCriteria);

  const chainId = "boi-package-localnet-4p";
  const productionEvidence = privacyPackageProductionEvidenceManifest(
    capabilities.privacyAlgorithms,
    { chainId },
  );
  const productionCapabilities = getPrivacyCapabilities(productionEvidence, {
    chainId,
  });
  const sourceZkAce = capabilities.privacyAlgorithms.find(
    (descriptor) => descriptor.id === "zk-ace-pq-authorization-v0",
  );
  const zkAce = productionCapabilities.privacyAlgorithms.find(
    (descriptor) => descriptor.id === "zk-ace-pq-authorization-v0",
  );
  assert.ok(sourceZkAce);
  assert.ok(zkAce);
  const expectedEntrypoints = privacyPackageProductionEntrypoints(sourceZkAce);
  const expectedSdkExports =
    privacyPackageProductionSdkExports(expectedEntrypoints);
  assert.equal(zkAce.productionReady, true);
  assert.equal(zkAce.implementationStage, "production-hardened");
  assert.deepEqual(zkAce.plannedSdkEntrypoints, []);
  assert.deepEqual(zkAce.sdkEntrypoints, expectedEntrypoints);
  assert.deepEqual(zkAce.sdkExports, expectedSdkExports);
  assert.deepEqual(zkAce.productionGate.sdkExports, expectedSdkExports);
  assert.equal(zkAce.productionGate.ready, true);
  assert.deepEqual(zkAce.productionGate.missing, []);
  assert.equal(zkAce.productionGate.chainId, chainId);
  assert.equal(
    zkAce.productionGate.reviewerIdentity,
    "package-reviewer@internal.example",
  );
  assert.equal(zkAce.productionGate.reviewScope.algorithm_id, zkAce.id);
  assert.deepEqual(
    zkAce.productionGate.reviewScope.sdk_entrypoints,
    expectedEntrypoints,
  );
  assert.equal(
    zkAce.productionGate.reviewScope.fuzz_artifact_hash,
    zkAce.productionGate.fuzzResults.artifact.uri,
  );
  assert.equal(
    zkAce.productionGate.reviewScope.performance_artifact_hash,
    zkAce.productionGate.performanceResults.artifact.uri,
  );
  assert.equal(
    zkAce.productionGate.localnetAcceptance.chain_id,
    chainId,
  );
  assert.equal(zkAce.productionGate.localnetAcceptance.peer_count, 4);
  assert.equal(zkAce.productionGate.localnetAcceptance.lifecycle_passed, true);
  assert.match(
    zkAce.productionGate.localnetAcceptance.lifecycle_redeem_tx_hash,
    /^sha256:/u,
  );
  assert.deepEqual(
    Object.keys(zkAce.productionGate.sdkParityArtifacts).sort(),
    [...PRIVACY_PACKAGE_PRODUCTION_SDK_ARTIFACT_KINDS].sort(),
  );
  assert.equal(
    Object.keys(zkAce.productionGate.gateEvidence).length,
    zkAce.productionGate.requiredGates.length,
  );
  assert.equal(Object.isFrozen(zkAce.sdkExports), true);
  assert.equal(Object.isFrozen(zkAce.productionGate.reviewScope), true);
  assert.throws(() => {
    zkAce.sdkExports.javascript.push("tamperedEntrypoint");
  });
  assert.throws(() => {
    zkAce.productionGate.reviewScope.algorithm_id = "tampered";
  });
});

test("package dist entrypoint exports production component privacy helpers", () => {
  const declarationExports = declarationExportNames();
  for (const name of [
    "buildJindoLatticeProofV0",
    "verifyJindoPolynomialCommitmentV0",
    "buildSisHintsAnonymousCredentialProofV0",
    "verifySisHintsAnonymousCredentialProofV0",
    "buildZkAtPolicyProofV1",
    "verifyZkAtPolicyProofV1",
    "buildZkAmsAdmissionBatchProofV0",
    "verifyZkAmsAdmissionBatchProofV0",
    "buildVegaCredentialPredicateProofV0",
    "verifyVegaCredentialPredicateProofV0",
    "buildSilentThresholdCredentialShowingProofV0",
    "verifySilentThresholdCredentialShowingProofV0",
    "buildZkX509IdentityProofV0",
    "verifyZkX509IdentityProofV0",
    "buildAnonymousPgcAccountCommitmentInstruction",
    "buildAnonymousPgcKOutOfNProofV1",
    "verifyAnonymousPgcKOutOfNProofV1",
    "buildAnonymousPgcTransferInstruction",
  ]) {
    assert.match(DIST_INDEX_TEXT, new RegExp(`\\b${name}\\b`, "u"));
    assert.ok(
      declarationExports.has(name),
      `missing declaration export ${name}`,
    );
  }
  assert.equal(typeof buildJindoLatticeProofV0, "function");
  assert.equal(typeof verifyJindoPolynomialCommitmentV0, "function");
  assert.equal(typeof buildSisHintsAnonymousCredentialProofV0, "function");
  assert.equal(typeof verifySisHintsAnonymousCredentialProofV0, "function");
  assert.equal(typeof buildZkAtPolicyProofV1, "function");
  assert.equal(typeof verifyZkAtPolicyProofV1, "function");
  assert.equal(typeof buildZkAmsAdmissionBatchProofV0, "function");
  assert.equal(typeof verifyZkAmsAdmissionBatchProofV0, "function");
  assert.equal(typeof buildVegaCredentialPredicateProofV0, "function");
  assert.equal(typeof verifyVegaCredentialPredicateProofV0, "function");
  assert.equal(typeof buildSilentThresholdCredentialShowingProofV0, "function");
  assert.equal(
    typeof verifySilentThresholdCredentialShowingProofV0,
    "function",
  );
  assert.equal(typeof buildZkX509IdentityProofV0, "function");
  assert.equal(typeof verifyZkX509IdentityProofV0, "function");
  assert.equal(
    typeof buildAnonymousPgcAccountCommitmentInstruction,
    "function",
  );
  assert.equal(typeof buildAnonymousPgcKOutOfNProofV1, "function");
  assert.equal(typeof verifyAnonymousPgcKOutOfNProofV1, "function");
  assert.equal(typeof buildAnonymousPgcTransferInstruction, "function");
  assert.match(
    DECLARATIONS_TEXT,
    /export interface JindoPolynomialCommitmentVerificationResult[\s\S]*production: true;[\s\S]*kind: "jindo-lattice-pcs-zk-v0";[\s\S]*backend: "LatticePcsSis";/u,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface SisHintsAnonymousCredentialVerificationResult[\s\S]*production: true;[\s\S]*kind: "sis-hints-anoncred-pq-v0";[\s\S]*backend: "SisWithHints";/u,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface ZkX509IdentityProductionVerificationResult[\s\S]*production: true;[\s\S]*kind: "zk-x509-onchain-identity-v0";[\s\S]*backend: "Stark";/u,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface ZkAtPolicyProofVerificationResult[\s\S]*production: true;[\s\S]*kind: "zkat-policy-private-auth-v1";[\s\S]*backend: "Stark";/u,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface ZkAmsAdmissionProductionVerificationResult[\s\S]*production: true;[\s\S]*kind: "zk-ams-recursive-admission-v0";[\s\S]*backend: "Stark";/u,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface VegaCredentialProductionVerificationResult[\s\S]*production: true;[\s\S]*kind: "vega-existing-credential-zk-v0";[\s\S]*backend: "Stark";/u,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface SilentThresholdCredentialProductionVerificationResult[\s\S]*production: true;[\s\S]*kind: "silent-threshold-anoncred-v0";[\s\S]*backend: "Stark";/u,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface AnonymousPgcProofV1VerificationResult[\s\S]*production: true;[\s\S]*kind: "anonymous-pgc-k-out-of-n-v1";[\s\S]*backend: "Stark";/u,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type AnonymousPgcPaymentBindingHashInput =[\s\S]*\{ paymentBindingHash: BinaryLike; payment_binding_hash\?: never \}[\s\S]*\{ paymentBindingHash\?: never; payment_binding_hash: BinaryLike \};/u,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type AnonymousPgcReceiverSetMaterialInput =[\s\S]*receiverSet\?: AnonymousPgcReceiverSetInput \| AnonymousPgcReceiverSet;[\s\S]*receivers\?: never;[\s\S]*receiver_set\?: AnonymousPgcReceiverSetInput \| AnonymousPgcReceiverSet;[\s\S]*receiverSet\?: never;[\s\S]*receivers\?: ReadonlyArray<AnonymousPgcReceiverInput>;/u,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type AnonymousPgcAccountCommitmentInstructionInput =[\s\S]*AnonymousPgcRequiredAlias2<[\s\S]*"accountCommitment"[\s\S]*"account_commitment"[\s\S]*BinaryLike[\s\S]*AnonymousPgcRequiredAlias2<[\s\S]*"anonymitySetRoot"[\s\S]*"anonymity_set_root"[\s\S]*BinaryLike[\s\S]*AnonymousPgcRequiredAlias2<"chainId", "chain_id", string>/u,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type AnonymousPgcDevProofFixtureInput =\s*AnonymousPgcProofMaterialInput & AnonymousPgcPaymentBindingHashInput;/u,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type AnonymousPgcProofV1Input =\s*AnonymousPgcDevProofFixtureInput & AnonymousPgcProofBytesInput;/u,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type AnonymousPgcProofV1VerificationInput =[\s\S]*AnonymousPgcProofMaterialInput &[\s\S]*AnonymousPgcProofEnvelopeInput &[\s\S]*AnonymousPgcOptionalPaymentBindingHashInput;/u,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type AnonymousPgcTransferInstructionInput =\s*AnonymousPgcProofV1VerificationInput & AnonymousPgcPaymentBindingHashInput;/u,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export interface AnonymousPgcDevProofFixture[\s\S]*public_input_bytes: Buffer;[\s\S]*publicInputBytes: Buffer;/u,
  );
});

test("package dist Anonymous PGC transfer instruction requires explicit payment binding", () => {
  const receiverSet = buildAnonymousPgcReceiverSet({
    threshold: 1,
    receivers: [
      {
        accountCommitment: Buffer.alloc(32, 0x21),
        ciphertextCommitment: Buffer.alloc(32, 0x31),
      },
      {
        accountCommitment: Buffer.alloc(32, 0x22),
        ciphertextCommitment: Buffer.alloc(32, 0x32),
      },
    ],
  });
  const base = {
    receiverSet,
    anonymitySetRoot: Buffer.alloc(32, 0x41),
    payload: Buffer.from("anonymous-pgc:alice:bob:42"),
    balanceCommitments: [Buffer.alloc(32, 0x51), Buffer.alloc(32, 0x52)],
    linkTag: Buffer.alloc(32, 0x61),
    rangeCommitments: [Buffer.alloc(32, 0x71)],
    paymentBindingHash: Buffer.alloc(32, 0x62),
    chainId: "boi-localnet",
    domainSeparator: "boi:anonymous-pgc:v1",
    vkHash: Buffer.alloc(32, 0x55),
  };
  const envelope = buildAnonymousPgcKOutOfNProofV1({
    ...base,
    proofBytes: Buffer.from("external-anonymous-pgc-proof-v1"),
  });
  const devFixture = buildAnonymousPgcDevProofFixture(base);
  const decodedDevFixture = noritoDecodePrivacyProofEnvelope(devFixture.envelope);
  assert.equal(Buffer.isBuffer(devFixture.public_input_bytes), true);
  assert.equal(Buffer.isBuffer(devFixture.publicInputBytes), true);
  assert.equal(devFixture.public_input_bytes.equals(devFixture.publicInputBytes), true);
  assert.equal(
    devFixture.public_input_bytes.equals(Buffer.from(decodedDevFixture.public_inputs)),
    true,
  );
  const transferInput = {
    proofEnvelope: envelope,
    receiverSet,
    payload: base.payload,
    anonymitySetRoot: base.anonymitySetRoot,
    balanceCommitments: base.balanceCommitments,
    linkTag: base.linkTag,
    rangeCommitments: base.rangeCommitments,
    paymentBindingHash: base.paymentBindingHash,
    chainId: base.chainId,
    domainSeparator: base.domainSeparator,
  };
  const transferInputWithoutPaymentBinding = { ...transferInput };
  delete transferInputWithoutPaymentBinding.paymentBindingHash;

  assert.equal(
    verifyAnonymousPgcKOutOfNProofV1(transferInputWithoutPaymentBinding).ok,
    true,
  );
  assert.throws(
    () => buildAnonymousPgcTransferInstruction(transferInputWithoutPaymentBinding),
    /paymentBindingHash is required/,
  );
  assert.throws(
    () =>
      buildAnonymousPgcTransferInstruction({
        ...transferInput,
        paymentBindingHash: Buffer.alloc(32, 0x63),
      }),
    /paymentBindingHash must match the envelope public inputs/,
  );
  assert.throws(
    () =>
      buildAnonymousPgcKOutOfNProofV1({
        ...base,
        payment_binding_hash: base.paymentBindingHash,
        proofBytes: Buffer.from("external-anonymous-pgc-proof-v1"),
      }),
    /multiple payment binding hash aliases/,
  );
  assert.throws(
    () =>
      buildAnonymousPgcKOutOfNProofV1({
        ...base,
        receivers: [
          {
            accountCommitment: Buffer.alloc(32, 0x21),
            ciphertextCommitment: Buffer.alloc(32, 0x31),
          },
          {
            accountCommitment: Buffer.alloc(32, 0x22),
            ciphertextCommitment: Buffer.alloc(32, 0x32),
          },
        ],
        proofBytes: Buffer.from("external-anonymous-pgc-proof-v1"),
      }),
    /receiverSet must not be combined with inline receiver-set fields/,
  );
  assert.throws(
    () =>
      buildAnonymousPgcKOutOfNProofV1({
        ...base,
        maxPayloadBytes: 1024,
        max_payload_bytes: 1024,
        proofBytes: Buffer.from("external-anonymous-pgc-proof-v1"),
      }),
    /multiple max payload byte limit aliases/,
  );
  assert.throws(
    () =>
      verifyAnonymousPgcKOutOfNProofV1({
        ...transferInput,
        payment_binding_hash: base.paymentBindingHash,
      }),
    /multiple paymentBindingHash aliases/,
  );
  assert.throws(
    () =>
      buildAnonymousPgcTransferInstruction({
        ...transferInput,
        payment_binding_hash: base.paymentBindingHash,
      }),
    /multiple payment binding hash aliases/,
  );
  assert.throws(
    () =>
      buildAnonymousPgcAccountCommitmentInstruction({
        accountCommitment: Buffer.alloc(32, 0x21),
        account_commitment: Buffer.alloc(32, 0x21),
        anonymitySetRoot: Buffer.alloc(32, 0x41),
        chainId: "boi-localnet",
      }),
    /multiple account commitment aliases/,
  );
});

test("package dist ZK-AMS production helpers reject dev fixture bytes", () => {
  const fixtureInput = {
    issuerRoot: Buffer.alloc(32, 0x91),
    admissionNullifiers: [Buffer.alloc(32, 0xa1), Buffer.alloc(32, 0xa2)],
    anonymousAccountCommitments: [
      Buffer.alloc(32, 0xb1),
      Buffer.alloc(32, 0xb2),
    ],
    recursiveProof: Buffer.from("zk-ams:recursive-proof:batch-7"),
    domainSeparator: "boi:zk-ams:pilot:v0",
    vkHash: Buffer.alloc(32, 0x66),
  };
  const fixture = buildZkAmsAdmissionDevProofFixture(fixtureInput);
  assert.throws(
    () =>
      buildZkAmsAdmissionBatchProofV0({
        ...fixtureInput,
        proofBytes: fixture.proofBytes,
      }),
    /dev fixture proof/,
  );
  assert.throws(
    () =>
      verifyZkAmsAdmissionBatchProofV0({
        envelope: fixture.envelope,
        issuerRoot: fixtureInput.issuerRoot,
        admissionNullifiers: fixtureInput.admissionNullifiers,
        anonymousAccountCommitments: fixtureInput.anonymousAccountCommitments,
        recursiveProof: fixtureInput.recursiveProof,
        domainSeparator: fixtureInput.domainSeparator,
      }),
    /dev fixture/,
  );
});

test("package dist privacy proof envelopes preserve pending production backend tags", () => {
  const vkHash = Buffer.alloc(32, 0x66);
  const cases = [
    ["halo2-ipa-orchard", "Halo2IpaOrchard"],
    ["halo2/ipa/orchard", "Halo2IpaOrchard"],
    ["halo2-pasta-action-bundle", "Halo2IpaOrchard"],
    ["orchard", "Halo2IpaOrchard"],
    ["zcash-orchard", "Halo2IpaOrchard"],
    ["groth16-bls12-377", "Groth16Bls12377"],
    ["groth16/bls12-377", "Groth16Bls12377"],
    ["bls12-377", "Groth16Bls12377"],
    ["decaf377", "Groth16Bls12377"],
    ["masp", "Groth16Bls12377"],
    ["penumbra-masp", "Groth16Bls12377"],
    ["halo2/ipa/penumbra", "Groth16Bls12377"],
    ["halo2/ipa/masp", "Groth16Bls12377"],
    ["fcmp-plus-plus-curve-tree", "FcmpPlusPlusCurveTree"],
    ["fcmp-plus-plus-curve-trees-bulletproofs", "FcmpPlusPlusCurveTree"],
    ["fcmp++", "FcmpPlusPlusCurveTree"],
    ["monero-fcmp++", "FcmpPlusPlusCurveTree"],
    ["halo2/ipa/monero", "FcmpPlusPlusCurveTree"],
    ["halo2/ipa/curve-tree", "FcmpPlusPlusCurveTree"],
    ["lattice-pcs-sis", "LatticePcsSis"],
    ["jindo-lattice-pcs-zk", "LatticePcsSis"],
    ["jindo-lattice-pcs-zk-v0", "LatticePcsSis"],
    ["Halo2IpaPasta", "Halo2IpaPasta"],
    ["halo2/pasta/kagemusha-recursive-aggregation-v1", "Halo2IpaPasta"],
    ["halo2/pasta/kagemusha-recursive-compact-v1", "Halo2IpaPasta"],
    [
      "halo2/pasta/kagemusha-recursive-spend-lineage-onehop-v1",
      "Halo2IpaPasta",
    ],
    [
      "halo2/pasta/kagemusha-recursive-spend-lineage-append-v1",
      "Halo2IpaPasta",
    ],
    ["stark/fri", "Stark"],
    ["stark/fri/sha256-goldilocks", "Stark"],
    ["stark/fri/poseidon2-goldilocks", "Stark"],
    ["stark/fri/sha256_goldilocks.v1", "Stark"],
    ["miden-stark", "MidenStark"],
    ["stark/fri/miden", "MidenStark"],
    ["stark-vm-note-transaction", "MidenStark"],
    ["aztec-plonkish-private-kernel", "AztecPlonkishPrivateKernel"],
    ["aztec/private-kernel", "AztecPlonkishPrivateKernel"],
    ["plonkish-private-kernel-rollup", "AztecPlonkishPrivateKernel"],
    ["pq-masp-stark-fri", "PqMaspStarkFri"],
    ["stark/fri/pq-masp-stark-fri", "PqMaspStarkFri"],
    ["post-quantum-masp", "PqMaspStarkFri"],
    ["anonymous-pgc", "AnonymousPgc"],
    ["anonymous-pgc-k-out-of-n", "AnonymousPgc"],
    ["anonymous-pgc-k-out-of-n-v1", "AnonymousPgc"],
    ["verange", "VeRange"],
    ["verange-transparent-range", "VeRange"],
    ["verange-transparent-range-v1", "VeRange"],
    ["zkat", "ZkAt"],
    ["zkAt policy-private authenticator", "ZkAt"],
    ["zkat-policy-private-auth-v1", "ZkAt"],
    ["recursive-anonymous-admission", "RecursiveAnonymousAdmission"],
    ["recursive-anonymous-admission-v0", "RecursiveAnonymousAdmission"],
    ["zk-ams-recursive-admission-v0", "RecursiveAnonymousAdmission"],
    ["vega-existing-credential-zk", "VegaExistingCredentialZk"],
    ["vega-existing-credential-zk-v0", "VegaExistingCredentialZk"],
    ["silent-threshold-anoncred", "SilentThresholdAnoncred"],
    ["silent-threshold-anoncred-v0", "SilentThresholdAnoncred"],
    ["threshold-anonymous-credentials", "SilentThresholdAnoncred"],
    ["zk-x509", "ZkX509"],
    ["zkvm-x509-identity", "ZkX509"],
    ["zk-x509-onchain-identity-v0", "ZkX509"],
    ["sis-with-hints", "SisWithHints"],
    ["sis-hints-anoncred-pq-v0", "SisWithHints"],
    ["lattice-anonymous-credentials", "SisWithHints"],
  ];

  for (const [backend, expected] of cases) {
    const encoded = buildPrivacyProofEnvelope({
      backend,
      circuitId: `${backend}:dist-pending-production-shape-v0`,
      vkHash,
      publicInputs: Buffer.from([0x01]),
      proofBytes: Buffer.from([0x02]),
      maxProofBytes: 16,
      maxPublicInputBytes: 16,
    });
    const decoded = noritoDecodePrivacyProofEnvelope(encoded);
    assert.equal(decoded.backend, expected);
  }
});

test("package dist research privacy adapters build envelopes and reject class options", () => {
  class PrivacyOptions {
    constructor(values) {
      Object.assign(this, values);
    }
  }

  const options = {
    vkHash: Buffer.alloc(32, 0x42),
    publicInputs: Buffer.from("production-research-public-inputs"),
    proofBytes: Buffer.from("production-research-proof"),
  };
  const proofHelpers = [
    [buildOrchardActionBundleProofV1, "Halo2IpaOrchard"],
    [buildPenumbraSpendProofV1, "Groth16Bls12377"],
    [buildPenumbraOutputProofV1, "Groth16Bls12377"],
    [buildFcmpPlusPlusMembershipProofV1, "FcmpPlusPlusCurveTree"],
    [buildMidenStarkTransactionProofV1, "MidenStark"],
    [buildAztecPrivateKernelProofV1, "AztecPlonkishPrivateKernel"],
    [buildPqMaspStarkTransferProofV0, "PqMaspStarkFri"],
  ];
  for (const [helper, expectedBackend] of proofHelpers) {
    const envelope = helper(options);
    const decoded = noritoDecodePrivacyProofEnvelope(envelope);
    assert.equal(decoded.backend, expectedBackend);
    assert.deepEqual(decoded.proof_bytes, Array.from(options.proofBytes));
    assert.throws(() => helper(new PrivacyOptions(options)), /plain object/);
  }

  const instructionHelpers = [
    [buildOrchardActionBundleInstruction, "zk::SubmitOrchardActionBundle"],
    [
      buildPenumbraShieldedPoolTransaction,
      "zk::SubmitPenumbraShieldedPoolTransaction",
    ],
    [buildFcmpPlusPlusTransferInstruction, "zk::SubmitFcmpPlusPlusTransfer"],
    [buildMidenNoteTransactionInstruction, "zk::SubmitMidenNoteTransaction"],
    [
      buildAztecPrivateRollupTransactionInstruction,
      "zk::SubmitAztecPrivateRollupTransaction",
    ],
    [buildPqMaspStarkRegisterPoolInstruction, "zk::SubmitPqMaspStarkTransfer"],
    [buildPqMaspStarkTransferInstruction, "zk::SubmitPqMaspStarkTransfer"],
  ];
  for (const [helper, instructionKind] of instructionHelpers) {
    const instruction = helper(options);
    assert.equal(instruction.instruction, instructionKind);
    assert.ok(instruction.proof_envelope_sha256);
    assert.throws(() => helper(new PrivacyOptions(options)), /plain object/);
  }

  const instruction = buildOrchardActionBundleInstruction({
    ...options,
    metadata: { purpose: "boundary-test" },
  });
  assert.deepEqual(instruction.metadata, { purpose: "boundary-test" });
  assert.throws(
    () =>
      buildOrchardActionBundleInstruction({
        ...options,
        metadata: new PrivacyOptions({ purpose: "test" }),
      }),
    /plain object/,
  );
});

test("package dist Jindo production helpers reject dev fixture bytes", () => {
  const base = {
    polynomialJson: { ring: "Rq", degree: 1024, digest: "poly" },
    openingClaimJson: { point: "x=42", value_digest: "value" },
    querySetJson: { queries: [0, 7, 42] },
    parametersJson: { scheme: "jindo-pcs-v0", q_bits: 64 },
    domainSeparator: "boi:jindo:pcs:pilot:v0",
  };
  const vkHash = Buffer.alloc(32, 0xaa);
  const proof = buildJindoLatticeProofV0({
    ...base,
    vkHash,
    proofBytes: Buffer.from("production-jindo-lattice-proof"),
  });
  const decoded = noritoDecodePrivacyProofEnvelope(proof);
  assert.equal(decoded.backend, "LatticePcsSis");

  const verified = verifyJindoPolynomialCommitmentV0({
    envelope: proof,
    ...base,
  });
  assert.equal(verified.production, true);
  assert.equal(verified.kind, "jindo-lattice-pcs-zk-v0");

  assert.throws(
    () => verifyJindoLatticeProofLocally({ envelope: proof, ...base }),
    /must be Unsupported/,
  );

  const fixture = buildJindoLatticeDevProofFixture({ ...base, vkHash });
  assert.throws(
    () =>
      verifyJindoPolynomialCommitmentV0({
        envelope: fixture.envelope,
        ...base,
      }),
    /(unsupported tag|must be LatticePcsSis)/,
  );
  assert.throws(
    () =>
      buildJindoLatticeProofV0({
        ...base,
        vkHash,
        proofBytes: fixture.proofBytes,
      }),
    /dev fixture/,
  );

  const devEncodedAsLattice = buildPrivacyProofEnvelope({
    backend: "lattice-pcs-sis",
    circuitId: decoded.circuit_id,
    vkHash,
    publicInputs: decoded.public_inputs,
    proofBytes: fixture.proofBytes,
  });
  assert.throws(
    () =>
      verifyJindoPolynomialCommitmentV0({
        envelope: devEncodedAsLattice,
        ...base,
      }),
    /dev fixture/,
  );

  for (const backend of ["unsupported", "stark/fri/sha256-goldilocks"]) {
    assert.throws(
      () =>
        buildJindoLatticeProofV0({
          ...base,
          backend,
          vkHash,
          proofBytes: Buffer.from("production-jindo-lattice-proof"),
        }),
      /backend/,
    );
  }
});

test("package dist Jindo and SIS public helpers reject class-instance options", () => {
  class PrivacyOptions {
    constructor(values) {
      Object.assign(this, values);
    }
  }

  const jindoBase = {
    polynomialJson: { ring: "Rq", degree: 1024, digest: "poly" },
    openingClaimJson: { point: "x=42", value_digest: "value" },
    querySetJson: { queries: [0, 7, 42] },
    parametersJson: { scheme: "jindo-pcs-v0", q_bits: 64 },
    domainSeparator: "boi:jindo:pcs:pilot:v0",
  };
  const jindoProofOptions = {
    ...jindoBase,
    vkHash: Buffer.alloc(32, 0xaa),
    proofBytes: Buffer.from("production-jindo-lattice-proof"),
  };

  for (const [helper, options] of [
    [buildJindoLatticePublicInputs, jindoBase],
    [buildJindoLatticeProofEnvelope, jindoProofOptions],
    [buildJindoLatticeProofV0, jindoProofOptions],
    [
      buildJindoLatticeDevProofFixture,
      {
        ...jindoBase,
        vkHash: Buffer.alloc(32, 0xaa),
      },
    ],
  ]) {
    assert.throws(() => helper(new PrivacyOptions(options)), /plain object/);
  }

  const jindoProof = buildJindoLatticeProofV0(jindoProofOptions);
  assert.equal(verifyJindoPolynomialCommitmentV0(jindoProof).ok, true);
  assert.throws(
    () =>
      verifyJindoPolynomialCommitmentV0(
        new PrivacyOptions({ envelope: jindoProof, ...jindoBase }),
      ),
    /plain object/,
  );

  const jindoFixture = buildJindoLatticeDevProofFixture({
    ...jindoBase,
    vkHash: Buffer.alloc(32, 0xaa),
  });
  assert.equal(verifyJindoLatticeProofLocally(jindoFixture.envelope).ok, true);
  assert.throws(
    () =>
      verifyJindoLatticeProofLocally(
        new PrivacyOptions({ envelope: jindoFixture.envelope, ...jindoBase }),
      ),
    /plain object/,
  );

  const sisBase = {
    issuerJson: { issuer: "boi", scheme: "sis-hints-v0" },
    credentialJson: { credential_type: "wallet", nonce: "n-1" },
    showingPolicyJson: { verifier: "boi", purpose: "wallet" },
    parametersJson: { scheme: "sis-hints-anoncred-v0", q_bits: 64 },
    domainSeparator: "boi:sis-hints:pilot:v0",
  };
  const sisProofOptions = {
    ...sisBase,
    vkHash: Buffer.alloc(32, 0xbb),
    proofBytes: Buffer.from("production-sis-hints-proof"),
  };

  for (const [helper, options] of [
    [buildSisHintsCredentialCommitments, sisBase],
    [buildSisHintsCredentialEnvelope, sisProofOptions],
    [buildSisHintsAnonymousCredentialProofV0, sisProofOptions],
    [
      buildSisHintsCredentialDevProofFixture,
      {
        ...sisBase,
        vkHash: Buffer.alloc(32, 0xbb),
      },
    ],
  ]) {
    assert.throws(() => helper(new PrivacyOptions(options)), /plain object/);
  }

  const sisProof = buildSisHintsAnonymousCredentialProofV0(sisProofOptions);
  assert.equal(verifySisHintsAnonymousCredentialProofV0(sisProof).ok, true);
  assert.throws(
    () =>
      verifySisHintsAnonymousCredentialProofV0(
        new PrivacyOptions({ envelope: sisProof, ...sisBase }),
      ),
    /plain object/,
  );

  const sisFixture = buildSisHintsCredentialDevProofFixture({
    ...sisBase,
    vkHash: Buffer.alloc(32, 0xbb),
  });
  assert.equal(
    verifySisHintsCredentialProofLocally(sisFixture.envelope).ok,
    true,
  );
  assert.throws(
    () =>
      verifySisHintsCredentialProofLocally(
        new PrivacyOptions({ envelope: sisFixture.envelope, ...sisBase }),
      ),
    /plain object/,
  );
});

test("package dist SIS-with-hints production helpers reject dev fixture bytes", () => {
  const base = {
    issuerJson: { issuer: "boi", scheme: "sis-hints-v0" },
    credentialJson: { credential_type: "wallet", nonce: "n-1" },
    showingPolicyJson: { verifier: "boi", purpose: "wallet" },
    parametersJson: { scheme: "sis-hints-anoncred-v0", q_bits: 64 },
    domainSeparator: "boi:sis-hints:pilot:v0",
  };
  const vkHash = Buffer.alloc(32, 0xbb);
  const proof = buildSisHintsAnonymousCredentialProofV0({
    ...base,
    vkHash,
    proofBytes: Buffer.from("production-sis-hints-proof"),
  });
  const decoded = noritoDecodePrivacyProofEnvelope(proof);
  assert.equal(decoded.backend, "SisWithHints");

  const verified = verifySisHintsAnonymousCredentialProofV0({
    envelope: proof,
    ...base,
  });
  assert.equal(verified.production, true);
  assert.equal(verified.kind, "sis-hints-anoncred-pq-v0");

  const fixture = buildSisHintsCredentialDevProofFixture({ ...base, vkHash });
  assert.throws(
    () =>
      verifySisHintsAnonymousCredentialProofV0({
        envelope: fixture.envelope,
        ...base,
      }),
    /(unsupported tag|must be SisWithHints)/,
  );
  assert.throws(
    () =>
      buildSisHintsAnonymousCredentialProofV0({
        ...base,
        vkHash,
        proofBytes: fixture.proofBytes,
      }),
    /dev fixture/,
  );

  const devEncodedAsSisHints = buildPrivacyProofEnvelope({
    backend: "sis-with-hints",
    circuitId: decoded.circuit_id,
    vkHash,
    publicInputs: decoded.public_inputs,
    proofBytes: fixture.proofBytes,
  });
  assert.throws(
    () =>
      verifySisHintsAnonymousCredentialProofV0({
        envelope: devEncodedAsSisHints,
        ...base,
      }),
    /dev fixture/,
  );
});

test("package dist Vega production helpers reject dev fixture bytes", () => {
  const accountId = AccountAddress.fromAccount({
    publicKey: deterministicEd25519PublicKey(0x10),
  }).toI105(0x02f1);
  const base = {
    issuerJson: { did: "did:example:issuer:boi", key: "issuer-key-1" },
    predicateJson: { kind: "age_over", attribute: "age", threshold: 18 },
    credentialSchema: "boi-age-credential-v1",
    accountId,
    expirationEpoch: 42,
    domainSeparator: "boi:vega:pilot:v0",
  };
  const vkHash = Buffer.alloc(32, 0x77);
  const proof = buildVegaCredentialPredicateProofV0({
    ...base,
    vkHash,
    proofBytes: Buffer.from("production-vega-predicate-proof"),
  });
  const decoded = noritoDecodePrivacyProofEnvelope(proof);
  assert.equal(decoded.backend, "Stark");

  const verified = verifyVegaCredentialPredicateProofV0({
    envelope: proof,
    ...base,
  });
  assert.equal(verified.production, true);
  assert.equal(verified.kind, "vega-existing-credential-zk-v0");

  const fixture = buildVegaCredentialDevProofFixture({ ...base, vkHash });
  assert.throws(
    () =>
      verifyVegaCredentialPredicateProofV0({
        envelope: fixture.envelope,
        ...base,
      }),
    /dev fixture/,
  );
  assert.throws(
    () =>
      buildVegaCredentialPredicateProofV0({
        ...base,
        vkHash,
        proofBytes: fixture.proofBytes,
      }),
    /dev fixture/,
  );
});

test("package dist silent-threshold production helpers reject dev fixture bytes", () => {
  const base = {
    issuerSetJson: { threshold: 2, issuers: ["a", "b", "c"] },
    thresholdPolicyJson: { threshold: 2, purpose: "wallet" },
    credentialShowingJson: { credential_type: "wallet", nonce: "n-1" },
    verifierPolicyJson: { verifier: "boi", purpose: "wallet" },
    domainSeparator: "boi:silent-threshold:pilot:v0",
  };
  const vkHash = Buffer.alloc(32, 0x88);
  const proof = buildSilentThresholdCredentialShowingProofV0({
    ...base,
    vkHash,
    proofBytes: Buffer.from("production-silent-threshold-showing-proof"),
  });
  const decoded = noritoDecodePrivacyProofEnvelope(proof);
  assert.equal(decoded.backend, "Stark");

  const verified = verifySilentThresholdCredentialShowingProofV0({
    envelope: proof,
    ...base,
  });
  assert.equal(verified.production, true);
  assert.equal(verified.kind, "silent-threshold-anoncred-v0");

  const fixture = buildSilentThresholdCredentialDevProofFixture({
    ...base,
    vkHash,
  });
  assert.throws(
    () =>
      verifySilentThresholdCredentialShowingProofV0({
        envelope: fixture.envelope,
        ...base,
      }),
    /dev fixture/,
  );
  assert.throws(
    () =>
      buildSilentThresholdCredentialShowingProofV0({
        ...base,
        vkHash,
        proofBytes: fixture.proofBytes,
      }),
    /dev fixture/,
  );
});

test("package dist ZK-X.509 production helpers reject dev fixture bytes", () => {
  const accountId = AccountAddress.fromAccount({
    publicKey: deterministicEd25519PublicKey(0x10),
  }).toI105(0x02f1);
  const base = {
    caRootJson: { root: "boi-root-ca", version: 1 },
    certificatePolicyJson: { eku: ["clientAuth"], policy: "wallet" },
    revocationJson: { epoch: 7, root: "revocation-root" },
    subjectJson: { cn: "Bank A", lei: "5493001KJTIIGC8Y1R12" },
    accountId,
    domainSeparator: "boi:zk-x509:pilot:v0",
  };
  const vkHash = Buffer.alloc(32, 0x99);
  const proof = buildZkX509IdentityProofV0({
    ...base,
    vkHash,
    proofBytes: Buffer.from("production-zk-x509-identity-proof"),
  });
  const decoded = noritoDecodePrivacyProofEnvelope(proof);
  assert.equal(decoded.backend, "Stark");

  const verified = verifyZkX509IdentityProofV0({
    envelope: proof,
    ...base,
  });
  assert.equal(verified.production, true);
  assert.equal(verified.kind, "zk-x509-onchain-identity-v0");

  const fixture = buildZkX509IdentityDevProofFixture({ ...base, vkHash });
  assert.throws(
    () =>
      verifyZkX509IdentityProofV0({
        envelope: fixture.envelope,
        ...base,
      }),
    /dev fixture/,
  );
  assert.throws(
    () =>
      buildZkX509IdentityProofV0({
        ...base,
        vkHash,
        proofBytes: fixture.proofBytes,
      }),
    /dev fixture/,
  );
});

test("package dist zkAt production helpers reject dev fixture bytes", () => {
  const accountId = AccountAddress.fromAccount({
    publicKey: deterministicEd25519PublicKey(0x10),
  }).toI105(0x02f1);
  const base = {
    policyJson: { threshold: 2, roles: ["ops", "risk", "treasury"] },
    policyEpoch: 7,
    policySchema: "boi-hidden-threshold-v1",
    payload: Buffer.from("zkat:transparent-transfer:42"),
    accountId,
    actionClass: "transparent_transfer",
    domainSeparator: "boi:zkat:v1",
  };
  const vkHash = Buffer.alloc(32, 0x55);
  const proof = buildZkAtPolicyProofV1({
    ...base,
    vkHash,
    proofBytes: Buffer.from("production-zkat-policy-proof"),
  });
  const decoded = noritoDecodePrivacyProofEnvelope(proof);
  assert.equal(decoded.backend, "Stark");

  const verified = verifyZkAtPolicyProofV1({
    envelope: proof,
    ...base,
  });
  assert.equal(verified.production, true);
  assert.equal(verified.kind, "zkat-policy-private-auth-v1");

  const fixture = buildZkAtDevProofFixture({ ...base, vkHash });
  assert.throws(
    () => verifyZkAtPolicyProofV1({ envelope: fixture.envelope, ...base }),
    /dev fixture/,
  );
  assert.throws(
    () =>
      buildZkAtPolicyProofV1({
        ...base,
        vkHash,
        proofBytes: fixture.proofBytes,
      }),
    /dev fixture/,
  );
});

test("package dist privacy proof envelopes reject production metadata claims", () => {
  const base = {
    backend: "stark/fri/sha256-goldilocks",
    circuitId: "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
    vkHash: Buffer.alloc(32, 0x55),
    publicInputs: Buffer.from([1, 2]),
    proofBytes: Buffer.from("proof"),
  };
  for (const payload of [
    { ...base, backend: "unsupported" },
    { ...base, backend: "mock/dev" },
    { ...base, backend: " unsupported" },
    { ...base, backend: "unsupported " },
    { ...base, backend: " stark/fri/sha256-goldilocks" },
    { ...base, backend: "stark/fri/sha256-goldilocks " },
    { ...base, backend: "stark/fri/sha256 goldilocks" },
    { ...base, backend: "stark/fri/sha256+goldilocks" },
    { ...base, backend: "halo2/ipa+mock" },
    { ...base, backend: "stark/fri/dev-fixture" },
    { ...base, backend: "stark/fri/d-e-v-f-i-x-t-u-r-e" },
    { ...base, backend: "stark/fri/sha512-goldilocks" },
    { ...base, backend: "stark/fri/audit-proof-v1" },
    { ...base, backend: "halo2\uFF0Fipa" },
    { ...base, backend: "halo2/\u200Bipa" },
    { ...base, backend: "h\u0430lo2/ipa" },
    { ...base, backend: "stark\uFF0Ffri/sha256-goldilocks" },
    { ...base, backend: "stark/fri/\u200Bsha256-goldilocks" },
    { ...base, backend: "st\u0430rk/fri/sha256-goldilocks" },
    { ...base, backend: "halo2/ipa/orchard/dev-fixture" },
    { ...base, backend: "halo2/ipa/orchard:production-ready" },
    { ...base, backend: "orchard:mainnet-ready" },
    { ...base, backend: "penumbra-masp:external-security-review" },
    { ...base, backend: "jindo-lattice-pcs-zk:release-ready" },
    { ...base, backend: "stark/fri/miden/claimed-production" },
    { ...base, backend: "miden-stark:dev-fixture" },
    { ...base, backend: "anonymous-pgc-k-out-of-n-v1-production" },
    { ...base, backend: "sis-hints-anoncred-pq-v0-devfixture" },
    { ...base, backend: "sis-with-hints:s-e-c-u-r-i-t-y-a-u-d-i-t-e-d" },
    { ...base, backend: "halo2/ipa/orchard:kzg" },
    { ...base, backend: "orchard:universal-srs" },
    { ...base, backend: "penumbra-masp:kzg" },
    { ...base, backend: "jindo-lattice-pcs-zk:trusted-setup" },
    { ...base, backend: "miden-stark:ptau" },
    { ...base, backend: "sis-with-hints:groth16" },
    { ...base, backend: "pq-masp-stark-fri:kzg" },
    { ...base, backend: "groth16/bls12-377/../../prod" },
    { ...base, backend: "post-quantum-masp/audit-claimed" },
    { ...base, production: true },
    { ...base, productionReady: true },
    { ...base, production_ready: true },
    { ...base, productionGate: { ready: true } },
    { ...base, production_gate: { ready: true } },
  ]) {
    assert.throws(
      () => buildPrivacyProofEnvelope(payload),
      /privacyProofEnvelope/,
    );
  }
});

test("package dist privacy dev proof fixtures reject production metadata claims", () => {
  const accountId = AccountAddress.fromAccount({
    publicKey: deterministicEd25519PublicKey(0x10),
  }).toI105(0x02f1);
  const anonymousPgcReceiverSet = buildAnonymousPgcReceiverSet({
    threshold: 1,
    receivers: [
      {
        accountCommitment: Buffer.alloc(32, 0x21),
        ciphertextCommitment: Buffer.alloc(32, 0x31),
      },
      {
        accountCommitment: Buffer.alloc(32, 0x22),
        ciphertextCommitment: Buffer.alloc(32, 0x32),
      },
    ],
  });
  const devFixtureCases = [
    [
      "zkAt",
      buildZkAtDevProofFixture,
      {
        policyJson: { threshold: 2, roles: ["ops", "risk", "treasury"] },
        policyEpoch: 7,
        policySchema: "boi-hidden-threshold-v1",
        payload: Buffer.from("zkat:transparent-transfer:42"),
        accountId,
        actionClass: "transparent_transfer",
        domainSeparator: "boi:zkat:v1",
        vkHash: Buffer.alloc(32, 0x55),
      },
    ],
    [
      "ZK-AMS",
      buildZkAmsAdmissionDevProofFixture,
      {
        issuerRoot: Buffer.alloc(32, 0x91),
        admissionNullifiers: [Buffer.alloc(32, 0xa1), Buffer.alloc(32, 0xa2)],
        anonymousAccountCommitments: [
          Buffer.alloc(32, 0xb1),
          Buffer.alloc(32, 0xb2),
        ],
        recursiveProof: Buffer.from("zk-ams:recursive-proof:batch-7"),
        domainSeparator: "boi:zk-ams:pilot:v0",
        vkHash: Buffer.alloc(32, 0x66),
      },
    ],
    [
      "Vega",
      buildVegaCredentialDevProofFixture,
      {
        issuerJson: { did: "did:example:issuer:boi", key: "issuer-key-1" },
        predicateJson: { kind: "age_over", attribute: "age", threshold: 18 },
        credentialSchema: "boi-age-credential-v1",
        accountId,
        expirationEpoch: 42,
        domainSeparator: "boi:vega:pilot:v0",
        vkHash: Buffer.alloc(32, 0x77),
      },
    ],
    [
      "Silent Threshold",
      buildSilentThresholdCredentialDevProofFixture,
      {
        issuerSetJson: { threshold: 2, issuers: ["a", "b", "c"] },
        thresholdPolicyJson: { threshold: 2, purpose: "wallet" },
        credentialShowingJson: { credential_type: "wallet", nonce: "n-1" },
        verifierPolicyJson: { verifier: "boi", purpose: "wallet" },
        domainSeparator: "boi:silent-threshold:pilot:v0",
        vkHash: Buffer.alloc(32, 0x88),
      },
    ],
    [
      "ZK-X.509",
      buildZkX509IdentityDevProofFixture,
      {
        caRootJson: { root: "boi-root-ca", version: 1 },
        certificatePolicyJson: { eku: ["clientAuth"], policy: "wallet" },
        revocationJson: { epoch: 7, root: "revocation-root" },
        subjectJson: { cn: "Bank A", lei: "5493001KJTIIGC8Y1R12" },
        accountId,
        domainSeparator: "boi:zk-x509:pilot:v0",
        vkHash: Buffer.alloc(32, 0x99),
      },
    ],
    [
      "Jindo",
      buildJindoLatticeDevProofFixture,
      {
        polynomialJson: { ring: "Rq", degree: 1024, digest: "poly" },
        openingClaimJson: { point: "x=42", value_digest: "value" },
        querySetJson: { queries: [0, 7, 42] },
        parametersJson: { scheme: "jindo-pcs-v0", q_bits: 64 },
        domainSeparator: "boi:jindo:pcs:pilot:v0",
        vkHash: Buffer.alloc(32, 0xaa),
      },
    ],
    [
      "SIS-with-hints",
      buildSisHintsCredentialDevProofFixture,
      {
        issuerJson: { issuer: "boi", scheme: "sis-hints-v0" },
        credentialJson: { credential_type: "wallet", nonce: "n-1" },
        showingPolicyJson: { verifier: "boi", purpose: "wallet" },
        parametersJson: { scheme: "sis-hints-anoncred-v0", q_bits: 64 },
        domainSeparator: "boi:sis-hints:pilot:v0",
        vkHash: Buffer.alloc(32, 0xbb),
      },
    ],
    [
      "Anonymous PGC",
      buildAnonymousPgcDevProofFixture,
      {
        receiverSet: anonymousPgcReceiverSet,
        anonymitySetRoot: Buffer.alloc(32, 0x41),
        payload: Buffer.from("anonymous-pgc:alice:bob:42"),
        balanceCommitments: [Buffer.alloc(32, 0x51), Buffer.alloc(32, 0x52)],
        linkTag: Buffer.alloc(32, 0x61),
        rangeCommitments: [Buffer.alloc(32, 0x71)],
        paymentBindingHash: Buffer.alloc(32, 0x62),
        chainId: "boi-localnet",
        domainSeparator: "boi:anonymous-pgc:v1",
        vkHash: Buffer.alloc(32, 0x55),
      },
    ],
    [
      "VeRange",
      buildVeRangeDevProofFixture,
      {
        commitments: [Buffer.alloc(32, 0x44), Buffer.alloc(32, 0x45)],
        bitLength: 64,
        commitmentScheme: "pedersen-v1",
        domainSeparator: "boi:amount-range:v1",
        payload: Buffer.from("transfer:alice@wonderland:bob@wonderland:42"),
        vkHash: Buffer.alloc(32, 0x55),
      },
    ],
  ];

  for (const [name, builder, input] of devFixtureCases) {
    const fixture = builder(input);
    assert.equal(
      fixture.production,
      false,
      `${name} fixture must stay dev-only`,
    );
    for (const [field, value] of [
      ["production", true],
      ["productionReady", true],
      ["production_ready", true],
      ["productionGate", { ready: true }],
      ["production_gate", { ready: true }],
    ]) {
      assert.throws(
        () => builder({ ...input, [field]: value }),
        new RegExp(field),
        `${name} fixture builder accepted ${field}`,
      );
    }
  }

  const veRangePayload = Buffer.from(
    "transfer:alice@wonderland:bob@wonderland:42",
  );
  const veRangeCommitments = [Buffer.alloc(32, 0x44), Buffer.alloc(32, 0x45)];
  const veRangeFixture = buildVeRangeDevProofFixture({
    commitments: veRangeCommitments,
    bitLength: 64,
    commitmentScheme: "pedersen-v1",
    domainSeparator: "boi:amount-range:v1",
    payload: veRangePayload,
    vkHash: Buffer.alloc(32, 0x55),
  });
  assert.equal(veRangeFixture.production, false);
  assert.equal(veRangeFixture.kind, "verange-dev-fixture-v1");
});

test("package dist privacy native availability rejects coerced ABI versions", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  try {
    for (const abiVersion of ["6", true]) {
      globalThis.__IROHA_NATIVE_BINDING__ = {
        connectNoritoBridgeAbiVersion() {
          return abiVersion;
        },
        privacyCapabilitiesV1() {
          return Uint8Array.from(PRIVACY_CAPABILITIES_ARCHIVE);
        },
        privacyProofRequestV1: privacyProofRequestNativeArchive,
        privacyBuildProofV1() {
          return Uint8Array.from(PRIVACY_BUILD_ARCHIVE);
        },
        privacyVerifyProofV1() {
          return Uint8Array.from(PRIVACY_VERIFY_ARCHIVE);
        },
      };

      assert.equal(isPrivacyNativeAvailable(), false);
    }
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist privacy native availability clears request copies after failures", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const completeBinding = (overrides = {}) => ({
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return Uint8Array.from(PRIVACY_CAPABILITIES_ARCHIVE);
    },
    privacyProofRequestV1: privacyProofRequestNativeArchive,
    privacyBuildProofV1() {
      return Uint8Array.from(PRIVACY_BUILD_ARCHIVE);
    },
    privacyVerifyProofV1() {
      return Uint8Array.from(PRIVACY_VERIFY_ARCHIVE);
    },
    ...overrides,
  });
  let throwingProbe;
  let badOutputProbe;
  let badOutput;

  try {
    globalThis.__IROHA_NATIVE_BINDING__ = completeBinding({
      privacyBuildProofV1(request) {
        throwingProbe = request;
        throw new Error("probe failure after request copy");
      },
    });
    assert.equal(isPrivacyNativeAvailable(), false);

    globalThis.__IROHA_NATIVE_BINDING__ = completeBinding({
      privacyVerifyProofV1(request) {
        badOutputProbe = request;
        badOutput = Buffer.from([0x56]);
        return badOutput;
      },
    });
    assert.equal(isPrivacyNativeAvailable(), false);
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }

  assert.deepEqual(
    Buffer.from(throwingProbe),
    Buffer.alloc(privacyNoritoFrame(0x52).length),
  );
  assert.deepEqual(
    Buffer.from(badOutputProbe),
    Buffer.alloc(privacyNoritoFrame(0x52).length),
  );
  assert.deepEqual(badOutput, Buffer.alloc(1));
});

test("package dist privacy native availability probes reject unsafe raw output", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const completeBinding = (overrides = {}) => ({
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return Uint8Array.from(PRIVACY_CAPABILITIES_ARCHIVE);
    },
    privacyProofRequestV1: privacyProofRequestNativeArchive,
    privacyBuildProofV1() {
      return Uint8Array.from(PRIVACY_BUILD_ARCHIVE);
    },
    privacyVerifyProofV1() {
      return Uint8Array.from(PRIVACY_VERIFY_ARCHIVE);
    },
    ...overrides,
  });
  try {
    const overrides = [
      {
        privacyCapabilitiesV1() {
          return "json is not Norito";
        },
      },
      {
        privacyProofRequestV1() {
          return "json is not Norito";
        },
      },
      {
        privacyProofRequestV1() {
          return Buffer.from(PRIVACY_BUILD_ARCHIVE);
        },
      },
      {
        privacyProofRequestV1() {
          return Buffer.from([0x52]);
        },
      },
      {
        privacyBuildProofV1() {
          return Uint8Array.from([]);
        },
      },
      {
        privacyVerifyProofV1() {
          return undefined;
        },
      },
      {
        privacyBuildProofV1() {
          return [0x42];
        },
      },
      {
        privacyBuildProofV1() {
          return Buffer.from([0x42]);
        },
      },
      {
        privacyCapabilitiesV1() {
          return Buffer.from(PRIVACY_BUILD_ARCHIVE);
        },
      },
      {
        privacyBuildProofV1() {
          return Buffer.from(PRIVACY_VERIFY_ARCHIVE);
        },
      },
      {
        privacyVerifyProofV1() {
          return Buffer.from(PRIVACY_CAPABILITIES_ARCHIVE);
        },
      },
      {
        privacyCapabilitiesV1() {
          const bad = Buffer.from(PRIVACY_CAPABILITIES_ARCHIVE);
          bad[0] = 0x00;
          return bad;
        },
      },
      {
        privacyBuildProofV1() {
          const bad = Buffer.from(PRIVACY_BUILD_ARCHIVE);
          bad[39] = 0x08;
          return bad;
        },
      },
      {
        privacyVerifyProofV1() {
          return Buffer.concat([PRIVACY_VERIFY_ARCHIVE, Buffer.from([0x01])]);
        },
      },
      {
        privacyVerifyProofV1() {
          const bad = Buffer.concat([PRIVACY_VERIFY_ARCHIVE, Buffer.alloc(1)]);
          bad[31] = 0x01;
          return bad;
        },
      },
      {
        privacyVerifyProofV1() {
          throw new Error("native probe failed");
        },
      },
      {
        privacyProofRequestV1() {
          throw new Error("native request probe failed");
        },
      },
      {
        privacyCapabilitiesV1() {
          return Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f);
        },
      },
      {
        privacyProofRequestV1() {
          return Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f);
        },
      },
      {
        privacyBuildProofV1() {
          return Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f);
        },
      },
      {
        privacyVerifyProofV1() {
          return Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f);
        },
      },
    ];

    for (const archive of malformedPrivacyNativeOutputArchives(0x50)) {
      overrides.push({
        privacyCapabilitiesV1() {
          return Buffer.from(archive);
        },
      });
    }
    for (const archive of malformedPrivacyNativeOutputArchives(0x52)) {
      overrides.push({
        privacyProofRequestV1() {
          return Buffer.from(archive);
        },
      });
    }
    for (const archive of malformedPrivacyNativeOutputArchives(0x42)) {
      overrides.push({
        privacyBuildProofV1() {
          return Buffer.from(archive);
        },
      });
    }
    for (const archive of malformedPrivacyNativeOutputArchives(0x56)) {
      overrides.push({
        privacyVerifyProofV1() {
          return Buffer.from(archive);
        },
      });
    }

    for (const override of overrides) {
      globalThis.__IROHA_NATIVE_BINDING__ = completeBinding(override);
      assert.equal(isPrivacyNativeAvailable(), false);
    }
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist privacy native wrappers reject wrong-operation result schemas", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const completeBinding = (overrides = {}) => ({
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return Uint8Array.from(PRIVACY_CAPABILITIES_ARCHIVE);
    },
    privacyProofRequestV1: privacyProofRequestNativeArchive,
    privacyBuildProofV1() {
      return Uint8Array.from(PRIVACY_BUILD_ARCHIVE);
    },
    privacyVerifyProofV1() {
      return Uint8Array.from(PRIVACY_VERIFY_ARCHIVE);
    },
    ...overrides,
  });
  try {
    globalThis.__IROHA_NATIVE_BINDING__ = completeBinding({
      privacyCapabilitiesV1() {
        return privacyNoritoFrameWithSchemaOverride(0x50, 21, 0x42);
      },
    });
    assert.equal(isPrivacyNativeAvailable(), false);
    assert.throws(
      () => privacyCapabilitiesV1(),
      /native privacyCapabilitiesV1 returned unexpected privacy result schema/,
    );

    globalThis.__IROHA_NATIVE_BINDING__ = completeBinding({
      privacyBuildProofV1() {
        return privacyNoritoFrameWithSchemaOverride(0x42, 6, 0x56);
      },
    });
    assert.equal(isPrivacyNativeAvailable(), false);
    assert.throws(
      () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
      /native privacyBuildProofV1 returned unexpected privacy result schema/,
    );

    globalThis.__IROHA_NATIVE_BINDING__ = completeBinding({
      privacyVerifyProofV1() {
        return privacyNoritoFrameWithSchemaOverride(0x56, 21, 0x50);
      },
    });
    assert.equal(isPrivacyNativeAvailable(), false);
    assert.throws(
      () => privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE),
      /native privacyVerifyProofV1 returned unexpected privacy result schema/,
    );

    globalThis.__IROHA_NATIVE_BINDING__ = completeBinding({
      privacyBuildProofV1() {
        assert.fail(
          "wrong-schema build request must not reach native dispatch",
        );
      },
      privacyVerifyProofV1() {
        assert.fail(
          "wrong-schema verify request must not reach native dispatch",
        );
      },
    });
    for (const wrongSchemaArchive of [
      PRIVACY_CAPABILITIES_ARCHIVE,
      PRIVACY_BUILD_ARCHIVE,
      PRIVACY_VERIFY_ARCHIVE,
      privacyNoritoFrameWithSchemaOverride(0x52, 6, 0x42),
      privacyNoritoFrameWithSchemaOverride(0x52, 21, 0x56),
    ]) {
      assert.throws(
        () => privacyBuildProofV1(Buffer.from(wrongSchemaArchive)),
        /requestArchive must use the privacy request schema/,
      );
      assert.throws(
        () => privacyVerifyProofV1(Uint8Array.from(wrongSchemaArchive)),
        /requestArchive must use the privacy request schema/,
      );
    }
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist privacy native wrappers reject oversized output archives", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const oversized = Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f);
  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return oversized;
    },
    privacyProofRequestV1: privacyProofRequestNativeArchive,
    privacyBuildProofV1() {
      return oversized;
    },
    privacyVerifyProofV1() {
      return oversized;
    },
  };
  try {
    assert.throws(
      () => privacyCapabilitiesV1(),
      /native privacyCapabilitiesV1 returned oversized output/,
    );
    assert.throws(
      () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
      /native privacyBuildProofV1 returned oversized output/,
    );
    assert.throws(
      () => privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE),
      /native privacyVerifyProofV1 returned oversized output/,
    );
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist privacy native wrappers reject invalid Norito-framed output archives", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const badMagic = Buffer.from(PRIVACY_CAPABILITIES_ARCHIVE);
  badMagic[0] = 0x00;
  const badVersion = Buffer.from(PRIVACY_BUILD_ARCHIVE);
  badVersion[4] = 1;
  const badMinorVersion = Buffer.from(PRIVACY_BUILD_ARCHIVE);
  badMinorVersion[5] = 1;
  const badDeclaredPayloadLength = privacyNoritoFrameWithDeclaredPayloadLength(
    0x42,
    6n,
  );
  const badOversizedDeclaredPayloadLength =
    privacyNoritoFrameWithDeclaredPayloadLength(0x42, 0x8000000000000000n);
  const badPadding = Buffer.concat([
    PRIVACY_VERIFY_ARCHIVE,
    Buffer.from([0x7f]),
  ]);
  const badFlags = Buffer.from(PRIVACY_BUILD_ARCHIVE);
  badFlags[39] = 0x08;
  const badFieldBitsetFlags = Buffer.from(PRIVACY_BUILD_ARCHIVE);
  badFieldBitsetFlags[39] = 0x20;
  const badChecksum = Buffer.concat([PRIVACY_VERIFY_ARCHIVE, Buffer.alloc(1)]);
  badChecksum[31] = 0x01;
  const badPayload = Buffer.from(privacyNoritoFrameWithPayload(0x57));
  badPayload[44] ^= 0x7f;
  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return badMagic;
    },
    privacyProofRequestV1: privacyProofRequestNativeArchive,
    privacyBuildProofV1() {
      return badVersion;
    },
    privacyVerifyProofV1() {
      return badPadding;
    },
  };
  try {
    assert.throws(
      () => privacyCapabilitiesV1(),
      /native privacyCapabilitiesV1 returned invalid Norito V1 archive/,
    );
    assert.throws(
      () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
      /native privacyBuildProofV1 returned invalid Norito V1 archive/,
    );
    assert.throws(
      () => privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE),
      /native privacyVerifyProofV1 returned invalid Norito V1 archive/,
    );
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }

  for (const invalidBuildOutput of [
    badMinorVersion,
    badDeclaredPayloadLength,
    badOversizedDeclaredPayloadLength,
  ]) {
    globalThis.__IROHA_NATIVE_BINDING__ = {
      connectNoritoBridgeAbiVersion() {
        return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
      },
      privacyCapabilitiesV1() {
        return PRIVACY_CAPABILITIES_ARCHIVE;
      },
      privacyProofRequestV1: privacyProofRequestNativeArchive,
      privacyBuildProofV1() {
        return invalidBuildOutput;
      },
      privacyVerifyProofV1() {
        return PRIVACY_VERIFY_ARCHIVE;
      },
    };
    try {
      assert.throws(
        () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
        /native privacyBuildProofV1 returned invalid Norito V1 archive/,
      );
    } finally {
      if (previous === undefined) {
        delete globalThis.__IROHA_NATIVE_BINDING__;
      } else {
        globalThis.__IROHA_NATIVE_BINDING__ = previous;
      }
    }
  }

  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return PRIVACY_CAPABILITIES_ARCHIVE;
    },
    privacyProofRequestV1: privacyProofRequestNativeArchive,
    privacyBuildProofV1() {
      return badFieldBitsetFlags;
    },
    privacyVerifyProofV1() {
      return PRIVACY_VERIFY_ARCHIVE;
    },
  };
  try {
    assert.throws(
      () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
      /native privacyBuildProofV1 returned invalid Norito V1 archive/,
    );
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }

  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return badPayload;
    },
    privacyProofRequestV1: privacyProofRequestNativeArchive,
    privacyBuildProofV1() {
      return badFlags;
    },
    privacyVerifyProofV1() {
      return badChecksum;
    },
  };
  try {
    assert.throws(
      () => privacyCapabilitiesV1(),
      /native privacyCapabilitiesV1 returned invalid Norito V1 archive/,
    );
    assert.throws(
      () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
      /native privacyBuildProofV1 returned invalid Norito V1 archive/,
    );
    assert.throws(
      () => privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE),
      /native privacyVerifyProofV1 returned invalid Norito V1 archive/,
    );
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist privacy native wrappers reject oversized request archives", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const oversized = Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f);
  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return Uint8Array.from(PRIVACY_CAPABILITIES_ARCHIVE);
    },
    privacyProofRequestV1: privacyProofRequestNativeArchive,
    privacyBuildProofV1() {
      assert.fail("oversized build request must not reach native dispatch");
    },
    privacyVerifyProofV1() {
      assert.fail("oversized verify request must not reach native dispatch");
    },
  };
  try {
    assert.throws(
      () => privacyBuildProofV1(oversized),
      /requestArchive must not exceed/,
    );
    assert.throws(
      () => privacyVerifyProofV1(oversized),
      /requestArchive must not exceed/,
    );
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist privacy native wrappers reject invalid request archives", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return Uint8Array.from(PRIVACY_CAPABILITIES_ARCHIVE);
    },
    privacyProofRequestV1: privacyProofRequestNativeArchive,
    privacyBuildProofV1() {
      assert.fail("invalid build request must not reach native dispatch");
    },
    privacyVerifyProofV1() {
      assert.fail("invalid verify request must not reach native dispatch");
    },
  };
  try {
    for (const malformedArchive of malformedPrivacyRequestArchives()) {
      assert.throws(
        () => privacyBuildProofV1(Buffer.from(malformedArchive)),
        /requestArchive must be a valid Norito V1 archive/,
      );
      assert.throws(
        () => privacyVerifyProofV1(Uint8Array.from(malformedArchive)),
        /requestArchive must be a valid Norito V1 archive/,
      );
    }
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist privacy native wrappers accept complete field-bitset flags", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const requestArchive = privacyNoritoFrameWithFlags(0x52, 0x26);
  const buildArchive = privacyNoritoFrameWithFlags(0x42, 0x26);
  const verifyArchive = privacyNoritoFrameWithFlags(0x56, 0x26);
  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return Uint8Array.from(PRIVACY_CAPABILITIES_ARCHIVE);
    },
    privacyProofRequestV1: privacyProofRequestNativeArchive,
    privacyBuildProofV1(request) {
      assert.deepEqual(Buffer.from(request), requestArchive);
      return buildArchive;
    },
    privacyVerifyProofV1(request) {
      assert.deepEqual(Buffer.from(request), requestArchive);
      return verifyArchive;
    },
  };
  try {
    assert.deepEqual(privacyBuildProofV1(requestArchive), buildArchive);
    assert.deepEqual(privacyVerifyProofV1(requestArchive), verifyArchive);
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist privacy native wrappers sanitize native exceptions", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const witness = Buffer.from("dist-private-witness-never-echo-21f0", "utf8");
  const requestArchive = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  const capturedRequests = [];
  const throwLeakingNativeError = (request) => {
    if (request !== undefined) {
      capturedRequests.push(request);
      assert.notEqual(request, requestArchive);
      assert.deepEqual(Buffer.from(request), PRIVACY_REQUEST_ARCHIVE);
    }
    throw new Error(`native panic included ${witness.toString("utf8")}`);
  };
  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyProofRequestV1: privacyProofRequestNativeArchive,
    privacyCapabilitiesV1: throwLeakingNativeError,
    privacyBuildProofV1: throwLeakingNativeError,
    privacyVerifyProofV1: throwLeakingNativeError,
  };
  try {
    for (const [operation, invoke] of [
      ["privacyCapabilitiesV1", () => privacyCapabilitiesV1()],
      ["privacyBuildProofV1", () => privacyBuildProofV1(requestArchive)],
      ["privacyVerifyProofV1", () => privacyVerifyProofV1(requestArchive)],
    ]) {
      let error;
      try {
        invoke();
      } catch (caught) {
        error = caught;
      }
      assert.ok(error, `${operation} should throw`);
      assert.match(
        error.message,
        new RegExp(`native ${operation} failed`, "u"),
      );
      assert.equal(error.cause, undefined);
      assert.equal(String(error).includes(witness.toString("utf8")), false);
      assert.equal(
        String(error.stack).includes(witness.toString("utf8")),
        false,
      );
    }
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }

  assert.equal(capturedRequests.length, 2);
  for (const request of capturedRequests) {
    assert.equal(
      request.every((value) => value === 0),
      true,
    );
  }
  assert.deepEqual(requestArchive, Buffer.from(PRIVACY_REQUEST_ARCHIVE));
});

test("package dist privacy native wrappers clear temporary request copies", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const requestArchive = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  const originalArchive = Buffer.from(requestArchive);
  let buildRequest;
  let verifyRequest;
  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return Uint8Array.from(PRIVACY_CAPABILITIES_ARCHIVE);
    },
    privacyProofRequestV1: privacyProofRequestNativeArchive,
    privacyBuildProofV1(request) {
      buildRequest = request;
      assert.notEqual(request, requestArchive);
      assert.deepEqual(Buffer.from(request), originalArchive);
      return Uint8Array.from(PRIVACY_BUILD_ARCHIVE);
    },
    privacyVerifyProofV1(request) {
      verifyRequest = request;
      assert.notEqual(request, requestArchive);
      assert.deepEqual(Buffer.from(request), originalArchive);
      return Uint8Array.from(PRIVACY_VERIFY_ARCHIVE);
    },
  };
  try {
    assert.deepEqual(
      privacyBuildProofV1(requestArchive),
      PRIVACY_BUILD_ARCHIVE,
    );
    assert.deepEqual(
      privacyVerifyProofV1(requestArchive),
      PRIVACY_VERIFY_ARCHIVE,
    );
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }

  assert.ok(buildRequest, "build request should be captured");
  assert.ok(verifyRequest, "verify request should be captured");
  assert.equal(
    buildRequest.every((value) => value === 0),
    true,
  );
  assert.equal(
    verifyRequest.every((value) => value === 0),
    true,
  );
  assert.deepEqual(requestArchive, originalArchive);
});

test("package dist privacyProofRequestV1 clears component copies after native dispatch", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const publicInputs = Buffer.from("public-inputs");
  const witness = Uint8Array.from(Buffer.from("secret-witness"));
  const proofBacking = Uint8Array.from([
    0x99,
    ...Buffer.from("proof-bytes"),
    0x88,
  ]);
  const proof = new DataView(proofBacking.buffer, 1, "proof-bytes".length);
  const captured = [];

  const binding = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return Uint8Array.from(PRIVACY_CAPABILITIES_ARCHIVE);
    },
    privacyBuildProofV1() {
      return Uint8Array.from(PRIVACY_BUILD_ARCHIVE);
    },
    privacyVerifyProofV1() {
      return Uint8Array.from(PRIVACY_VERIFY_ARCHIVE);
    },
  };

  try {
    globalThis.__IROHA_NATIVE_BINDING__ = {
      ...binding,
      privacyProofRequestV1(
        _algorithmId,
        _entrypoint,
        _vkRef,
        publicCopy,
        witnessCopy,
        proofCopy,
      ) {
        captured.push([publicCopy, witnessCopy, proofCopy]);
        assert.notEqual(publicCopy, publicInputs);
        assert.notEqual(witnessCopy, witness);
        assert.notEqual(proofCopy.buffer, proofBacking.buffer);
        assert.deepEqual(Buffer.from(publicCopy), publicInputs);
        assert.deepEqual(Buffer.from(witnessCopy), Buffer.from(witness));
        assert.deepEqual(Buffer.from(proofCopy), Buffer.from("proof-bytes"));
        return Uint8Array.from(PRIVACY_REQUEST_ARCHIVE);
      },
    };
    assert.deepEqual(
      privacyProofRequestV1({
        algorithmId: "zk-ace-pq-authorization-v0",
        entrypoint: "buildZkAceAuthorizationProofV1",
        vkRef: "stark-fri:zk_ace_pq_authorization_v0",
        publicInputs,
        witness,
        proof,
      }),
      PRIVACY_REQUEST_ARCHIVE,
    );

    globalThis.__IROHA_NATIVE_BINDING__ = {
      ...binding,
      privacyProofRequestV1(
        _algorithmId,
        _entrypoint,
        _vkRef,
        publicCopy,
        witnessCopy,
        proofCopy,
      ) {
        captured.push([publicCopy, witnessCopy, proofCopy]);
        throw new Error(
          "native proof request failure with private component bytes",
        );
      },
    };
    let error;
    try {
      privacyProofRequestV1({
        algorithmId: "zk-ace-pq-authorization-v0",
        entrypoint: "buildZkAceAuthorizationProofV1",
        vkRef: "stark-fri:zk_ace_pq_authorization_v0",
        publicInputs,
        witness,
        proof,
      });
    } catch (caught) {
      error = caught;
    }
    assert.ok(error, "privacyProofRequestV1 should throw");
    assert.match(error.message, /native privacyProofRequestV1 failed/);
    assert.equal(String(error).includes("private component bytes"), false);
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }

  assert.equal(captured.length, 2);
  for (const copies of captured) {
    for (const copy of copies) {
      assert.equal(
        copy.every((value) => value === 0),
        true,
      );
    }
  }
  assert.deepEqual(publicInputs, Buffer.from("public-inputs"));
  assert.deepEqual(Buffer.from(witness), Buffer.from("secret-witness"));
  assert.deepEqual(
    Buffer.from(proofBacking.subarray(1, 1 + "proof-bytes".length)),
    Buffer.from("proof-bytes"),
  );
});

test("package dist privacy native wrappers respect sliced request archive views", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const buildView = slicedPrivacyView(PRIVACY_REQUEST_ARCHIVE);
  const verifyBacking = Uint8Array.from([
    0x99,
    0x88,
    ...PRIVACY_REQUEST_ARCHIVE,
    0x77,
  ]);
  const verifyView = new DataView(
    verifyBacking.buffer,
    2,
    PRIVACY_REQUEST_ARCHIVE.length,
  );
  let buildRequest;
  let verifyRequest;
  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return Uint8Array.from(PRIVACY_CAPABILITIES_ARCHIVE);
    },
    privacyProofRequestV1: privacyProofRequestNativeArchive,
    privacyBuildProofV1(request) {
      buildRequest = request;
      assert.deepEqual(Buffer.from(request), PRIVACY_REQUEST_ARCHIVE);
      return slicedPrivacyView(PRIVACY_BUILD_ARCHIVE);
    },
    privacyVerifyProofV1(request) {
      verifyRequest = request;
      assert.deepEqual(Buffer.from(request), PRIVACY_REQUEST_ARCHIVE);
      return new DataView(
        slicedPrivacyView(PRIVACY_VERIFY_ARCHIVE).buffer,
        3,
        PRIVACY_VERIFY_ARCHIVE.length,
      );
    },
  };
  try {
    assert.deepEqual(privacyBuildProofV1(buildView), PRIVACY_BUILD_ARCHIVE);
    assert.deepEqual(privacyVerifyProofV1(verifyView), PRIVACY_VERIFY_ARCHIVE);
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }

  assert.deepEqual(Buffer.from(buildView), PRIVACY_REQUEST_ARCHIVE);
  assert.deepEqual(
    Buffer.from(verifyBacking.subarray(2, 2 + PRIVACY_REQUEST_ARCHIVE.length)),
    PRIVACY_REQUEST_ARCHIVE,
  );
  assert.equal(
    buildRequest.every((value) => value === 0),
    true,
  );
  assert.equal(
    verifyRequest.every((value) => value === 0),
    true,
  );
});

test("package dist privacy native wrappers respect sliced native output archive views", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const prefixLength = 3;
  const capabilitiesBacking = Uint8Array.from([
    0xff,
    0x7f,
    0x50,
    ...PRIVACY_CAPABILITIES_ARCHIVE,
    0x24,
  ]);
  const buildBacking = Uint8Array.from([
    0xff,
    0x7f,
    0x42,
    ...PRIVACY_BUILD_ARCHIVE,
    0x13,
  ]);
  const verifyBacking = Uint8Array.from([
    0xff,
    0x7f,
    0x56,
    ...PRIVACY_VERIFY_ARCHIVE,
    0x37,
  ]);

  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return capabilitiesBacking.subarray(
        prefixLength,
        prefixLength + PRIVACY_CAPABILITIES_ARCHIVE.length,
      );
    },
    privacyProofRequestV1: privacyProofRequestNativeArchive,
    privacyBuildProofV1() {
      return new DataView(
        buildBacking.buffer,
        prefixLength,
        PRIVACY_BUILD_ARCHIVE.length,
      );
    },
    privacyVerifyProofV1() {
      return verifyBacking.subarray(
        prefixLength,
        prefixLength + PRIVACY_VERIFY_ARCHIVE.length,
      );
    },
  };
  try {
    const capabilitiesArchive = privacyCapabilitiesV1();
    const buildArchive = privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE);
    const verifyArchive = privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE);

    assert.deepEqual(capabilitiesArchive, PRIVACY_CAPABILITIES_ARCHIVE);
    assert.deepEqual(buildArchive, PRIVACY_BUILD_ARCHIVE);
    assert.deepEqual(verifyArchive, PRIVACY_VERIFY_ARCHIVE);

    capabilitiesBacking[prefixLength] = 0x00;
    buildBacking[prefixLength] = 0x00;
    verifyBacking[prefixLength] = 0x00;

    assert.deepEqual(capabilitiesArchive, PRIVACY_CAPABILITIES_ARCHIVE);
    assert.deepEqual(buildArchive, PRIVACY_BUILD_ARCHIVE);
    assert.deepEqual(verifyArchive, PRIVACY_VERIFY_ARCHIVE);
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package dist privacy native wrappers defensively copy native output archives", () => {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  const capabilitiesOutput = Buffer.from(PRIVACY_CAPABILITIES_ARCHIVE);
  const buildOutput = Buffer.from(PRIVACY_BUILD_ARCHIVE);
  const verifyBacking = Uint8Array.from(
    Buffer.concat([
      Buffer.from([0x00]),
      PRIVACY_VERIFY_ARCHIVE,
      Buffer.from([0x00]),
    ]),
  );
  const verifyOutput = verifyBacking.subarray(
    1,
    1 + PRIVACY_VERIFY_ARCHIVE.length,
  );
  globalThis.__IROHA_NATIVE_BINDING__ = {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return capabilitiesOutput;
    },
    privacyProofRequestV1: privacyProofRequestNativeArchive,
    privacyBuildProofV1() {
      return buildOutput;
    },
    privacyVerifyProofV1() {
      return verifyOutput;
    },
  };
  try {
    const capabilitiesArchive = privacyCapabilitiesV1();
    assert.notEqual(capabilitiesArchive, capabilitiesOutput);
    assert.deepEqual(capabilitiesArchive, PRIVACY_CAPABILITIES_ARCHIVE);
    capabilitiesArchive[0] = 0x7f;
    assert.deepEqual(capabilitiesOutput, PRIVACY_CAPABILITIES_ARCHIVE);

    const buildArchive = privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE);
    assert.notEqual(buildArchive, buildOutput);
    assert.deepEqual(buildArchive, PRIVACY_BUILD_ARCHIVE);
    buildArchive[0] = 0x7f;
    assert.deepEqual(buildOutput, PRIVACY_BUILD_ARCHIVE);

    const verifyArchive = privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE);
    assert.deepEqual(verifyArchive, PRIVACY_VERIFY_ARCHIVE);
    verifyBacking[1] = 0x7f;
    assert.deepEqual(verifyArchive, PRIVACY_VERIFY_ARCHIVE);
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("package declarations mark privacy capability metadata readonly", () => {
  const pqLayers = declarationInterface("PrivacyPqLayers");
  assert.match(pqLayers, /readonly proof: boolean;/);
  assert.match(pqLayers, /readonly authorization: boolean;/);
  assert.match(pqLayers, /readonly noteEncryption: boolean;/);

  assert.match(
    DECLARATIONS_TEXT,
    /export type PrivacyProductionSdkSurface =\s*\|\s*"rust_core"[\s\S]*?\|\s*"csharp";/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type PrivacyProductionSdkExports = Readonly<\s*Record<PrivacyProductionSdkSurface, readonly string\[\]>\s*>;/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type PrivacyProductionSdkParityArtifacts = Readonly<\s*Record<\s*PrivacyProductionSdkParityArtifactKind,\s*Readonly<Record<PrivacyProductionSdkSurface, PrivacyProductionArtifact>>\s*>\s*>;/,
  );

  const artifact = declarationInterface("PrivacyProductionArtifact");
  assert.match(artifact, /readonly label: string;/);
  assert.match(artifact, /readonly uri: string;/);

  const result = declarationInterface("PrivacyProductionResult");
  assert.match(result, /readonly passed: true;/);
  assert.match(result, /readonly artifact: PrivacyProductionArtifact;/);

  const reviewScope = declarationInterface("PrivacyProductionReviewScope");
  assert.match(reviewScope, /readonly algorithm_id: string;/);
  assert.match(reviewScope, /readonly chain_id: string;/);
  assert.match(reviewScope, /readonly verifier_key_id: string \| null;/);
  assert.match(reviewScope, /readonly public_inputs_schema: string \| null;/);
  assert.match(reviewScope, /readonly sdk_entrypoints: readonly string\[\];/);
  assert.match(reviewScope, /readonly required_state: readonly string\[\];/);
  assert.match(reviewScope, /readonly fuzz_artifact_hash: string;/);
  assert.match(reviewScope, /readonly performance_artifact_hash: string;/);
  assert.match(reviewScope, /readonly localnet_run_id: string;/);

  const localnetAcceptance = declarationInterface(
    "PrivacyProductionLocalnetAcceptance",
  );
  assert.match(localnetAcceptance, /readonly target: "localnet";/);
  assert.match(localnetAcceptance, /readonly peer_count: 4;/);
  assert.match(localnetAcceptance, /readonly peer_ids: readonly string\[\];/);
  assert.match(localnetAcceptance, /readonly lifecycle_redeem_tx_hash: string;/);
  assert.match(localnetAcceptance, /readonly lifecycle_passed: true;/);

  const productionGate = declarationInterface("PrivacyProductionGate");
  assert.match(productionGate, /readonly ready: boolean;/);
  assert.match(
    productionGate,
    /readonly gates: Readonly<Record<string, boolean>>;/,
  );
  assert.match(productionGate, /readonly missing: readonly string\[\];/);
  assert.match(
    productionGate,
    /readonly auditReferences: readonly Readonly<\{\s*label: string;\s*url: string;\s*uri\?: string;\s*signature\?: string;\s*\}>\[\];/,
  );
  assert.match(
    productionGate,
    /readonly localnetAcceptance\?: PrivacyProductionLocalnetAcceptance;/,
  );
  assert.match(productionGate, /readonly fuzzResults\?: PrivacyProductionResult;/);
  assert.match(
    productionGate,
    /readonly performanceResults\?: PrivacyProductionResult;/,
  );
  assert.match(
    productionGate,
    /readonly reviewScope\?: PrivacyProductionReviewScope;/,
  );
  assert.match(
    productionGate,
    /readonly sdkExports\?: PrivacyProductionSdkExports;/,
  );
  assert.match(
    productionGate,
    /readonly sdkParityArtifacts\?: PrivacyProductionSdkParityArtifacts;/,
  );
  assert.match(
    productionGate,
    /readonly gateEvidence\?: PrivacyProductionGateEvidence;/,
  );

  const descriptor = declarationInterface("PrivacyAlgorithmDescriptor");
  assert.match(
    descriptor,
    /readonly coveredCriteria: readonly PrivacyCriterionKey\[\];/,
  );
  assert.match(descriptor, /readonly backendFamily: string;/);
  assert.match(descriptor, /readonly pqLayers: PrivacyPqLayers;/);
  assert.match(descriptor, /readonly sdkEntrypoints: readonly string\[\];/);
  assert.match(
    descriptor,
    /readonly sdkExports\?: PrivacyProductionSdkExports;/,
  );
  assert.match(descriptor, /readonly chainRequirements: readonly string\[\];/);
  assert.match(descriptor, /readonly productionReady: boolean;/);
  assert.match(descriptor, /readonly productionGate: PrivacyProductionGate;/);

  const capabilities = declarationInterface("PrivacyCapabilities");
  assert.match(capabilities, /readonly javascriptSdkAvailable: boolean;/);
  assert.match(capabilities, /readonly bridgeAvailable: boolean;/);
  assert.match(
    capabilities,
    /readonly privacyAlgorithms: readonly PrivacyAlgorithmDescriptor\[\];/,
  );
  assert.match(
    capabilities,
    /readonly privacyCriteria: readonly PrivacyCriterionKey\[\];/,
  );

  assert.match(
    DECLARATIONS_TEXT,
    /export function getPrivacyCriteria\(\): readonly PrivacyCriterionKey\[\];/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export function getPrivacyAlgorithmDescriptors\(\s*productionEvidence\?: PrivacyProductionEvidenceRegistry,\s*options\?: PrivacyProductionEvidenceOptions,\s*\): readonly PrivacyAlgorithmDescriptor\[\];/,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export const PRIVACY_NATIVE_ARCHIVE_MAX_BYTES: number;/,
  );
  for (const [name, value] of [
    ["PRIVACY_FFI_STATUS_ERROR", 1],
    ["PRIVACY_FFI_ERROR_NULL_POINTER", 1],
    ["PRIVACY_FFI_ERROR_MALFORMED_NORITO", 2],
    ["PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM", 3],
    ["PRIVACY_FFI_ERROR_PRODUCTION_DISABLED", 4],
    ["PRIVACY_FFI_ERROR_INVALID_REQUEST", 5],
  ]) {
    assert.match(
      DECLARATIONS_TEXT,
      new RegExp(`export const ${name}: ${value};`),
    );
  }
});

test("package declarations mark Kagemusha lineage key artifacts readonly", () => {
  const artifacts = declarationInterface(
    "KagemushaRecursiveSpendLineageKeyArtifacts",
  );
  assert.match(artifacts, /readonly proofCircuitId:/);
  assert.match(
    artifacts,
    /readonly verifierOpeningLen: KagemushaRecursiveSpendLineageKeyArtifactOpeningLen;/,
  );
  assert.match(
    artifacts,
    /readonly lineageVerifierKeyBackend: typeof KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND;/,
  );
  assert.match(artifacts, /readonly lineageVerifierKey: Buffer;/);
  assert.match(artifacts, /readonly lineageProvingKeyArchive: Buffer;/);
  assert.match(artifacts, /readonly isInitArtifact: boolean;/);
  assert.match(artifacts, /readonly isAppendArtifact: boolean;/);
});

test("package declarations require Kagemusha append output selector", () => {
  assert.match(
    DECLARATIONS_TEXT,
    /export interface KagemushaRecursiveSpendAppendRequestBaseInput\s*\{/u,
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export type KagemushaRecursiveSpendAppendRequestInput =\s*KagemushaRecursiveSpendAppendRequestBaseInput &\s*\(\s*\|\s*\{\s*readonly outputProofCircuitId: string;\s*readonly output_proof_circuit_id\?: never;\s*\}\s*\|\s*\{\s*readonly outputProofCircuitId\?: never;\s*readonly output_proof_circuit_id: string;\s*\}\s*\);/u,
    "append request input must require exactly one output selector alias",
  );
  assert.doesNotMatch(
    DECLARATIONS_TEXT,
    /readonly output(?:ProofCircuitId|_proof_circuit_id)\?: string \| null/u,
    "append output selector must not be optional or nullable",
  );
  assert.match(
    DECLARATIONS_TEXT,
    /normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId\(\s*outputProofCircuitId: string,\s*\): string;/u,
    "append selector normalizer declaration must require a string selector",
  );
  assert.match(
    DECLARATIONS_TEXT,
    /canProveKagemushaRecursiveSpendAppendOutputProofCircuitId\(\s*outputProofCircuitId: string,\s*previousHopCount: number,\s*\): boolean;/u,
    "append selector prover preflight declaration must require a string selector",
  );
});

test("package declarations expose recursive compact key-package signatures", () => {
  const packageJson = JSON.parse(PACKAGE_JSON_TEXT);
  assert.equal(packageJson.types, "./index.d.ts");
  assert.equal(packageJson.exports["."].types, "./index.d.ts");
  assert.equal(packageJson.exports["./crypto"].types, "./index.d.ts");
  assert.equal(packageJson.exports["./nexus-app"].browser, "./dist/nexusApp.js");
  assert.equal(packageJson.exports["./nexus-app"].import, "./dist/nexusApp.js");
  assert.equal(packageJson.exports["./nexus-app"].types, "./nexus-app.d.ts");
  assert.equal(
    packageJson.exports["./kotodama-compiler"].import,
    "./dist/kotodamaCompiler/index.js",
  );
  assert.equal(
    packageJson.exports["./kotodama-compiler"].browser,
    "./dist/kotodamaCompiler/browser.js",
  );
  assert.deepEqual(
    Object.keys(packageJson.exports["./kotodama-compiler"]),
    ["types", "browser", "import"],
    "types and browser must precede the universally true import condition",
  );
  assert.deepEqual(packageJson.typesVersions["*"].crypto, ["./index.d.ts"]);
  assert.equal(packageJson.files.includes("index.d.ts"), true);
  const nexusAppDeclarations = PACKAGE_DECLARATION_TEXTS.get("nexus-app.d.ts");
  assert.match(nexusAppDeclarations, /export interface NexusTransactionCodec/);
  assert.match(nexusAppDeclarations, /finalizeSignedTransaction\(/);
  assert.match(nexusAppDeclarations, /export class NexusAppClient/);
  assert.match(
    DECLARATIONS_TEXT,
    /export function kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes\(\s*recordBundleArchive: BinaryLike,\s*pallasOpenEnvelopesArchive: BinaryLike,\s*recursiveCompactKeyArtifactsArchive: BinaryLike,\s*\): Buffer;/u,
    "recursive compact prover declaration must require key artifacts",
  );
  assert.match(
    DECLARATIONS_TEXT,
    /export function kagemushaVerifyRecursiveCompactPaymentToken\(\s*compactTokenArchive: BinaryLike,\s*recursiveCompactVerifierKeysArchive: BinaryLike,\s*\): boolean;/u,
    "recursive compact verifier declaration must require verifier keys",
  );
  assert.doesNotMatch(
    DECLARATIONS_TEXT,
    /recursiveCompact(?:KeyArtifactsArchive|VerifierKeysArchive)\?:\s*BinaryLike/u,
    "recursive compact key packages must not be optional",
  );
});

test("package Nexus browser export has an enforced browser-only dependency graph", () => {
  const source = readFileSync(
    new URL("../src/nexusApp.js", import.meta.url),
    "utf8",
  );
  const dist = readFileSync(
    new URL("../dist/nexusApp.js", import.meta.url),
    "utf8",
  );
  const bundleGate = readFileSync(
    new URL("../scripts/bundle-size-check.mjs", import.meta.url),
    "utf8",
  );
  const browserRegression = readFileSync(
    new URL("./nexusApp.browser.test.js", import.meta.url),
    "utf8",
  );

  assert.equal(dist, source, "Nexus browser source and dist must remain exact");
  assert.match(source, /import \{ Buffer \} from "buffer";/u);
  assert.match(source, /from "\.\/crypto\.browser\.js";/u);
  assert.match(source, /browserTransactionCodec/u);
  assert.match(source, /class BrowserToriiPipelineClient/u);
  assert.doesNotMatch(source, /from "(?:node:|\.\/(?:crypto|native|toriiClient)\.js)/u);
  assert.match(bundleGate, /label: "nexusApp\.js \(browser\)"/u);
  assert.match(bundleGate, /limitKb: 205/u);
  assert.match(browserRegression, /globalThis\.Buffer = undefined/u);
  assert.match(browserRegression, /defaults build, finalize, and submit/u);
});

test("package declarations expose Torii iterable string select projections", () => {
  const options = declarationInterface("IterableQueryOptions");
  assert.match(
    options,
    /\n  select\?: ReadonlyArray<string \| Record<string, unknown>>;\n/u,
  );
  assert.doesNotMatch(
    options,
    /\n  select\?: ReadonlyArray<Record<string, unknown>>;\n/u,
  );
});

test("package declarations expose Torii iterable count mode aliases", () => {
  const listOptions = declarationInterface("IterableListOptions");
  assert.match(listOptions, /\n  countMode\?: ToriiCountMode;\n/u);
  assert.match(listOptions, /\n  count_mode\?: ToriiCountMode;\n/u);

  const queryOptions = declarationInterface("IterableQueryOptions");
  assert.doesNotMatch(queryOptions, /\n  fetchSize\?: NumericLike;\n/u);
  assert.match(queryOptions, /\n  fetch_size\?: NumericLike;\n/u);
  assert.match(queryOptions, /\n  queryName\?: string;\n/u);
  assert.match(queryOptions, /\n  query_name\?: string;\n/u);
});

test("package declarations keep accumulator digests native-owned", () => {
  const accumulatorDigestDeclarationPattern =
    /\b[A-Za-z0-9_]*(?:lineageDigest|LineageDigest|lineage_digest|aggregationTranscriptDigest|AggregationTranscriptDigest|aggregation_transcript_digest|fixedWindowTableScheduleDigest|FixedWindowTableScheduleDigest|fixed_window_table_schedule_digest|fixedWindowSharedTableManifestDigest|FixedWindowSharedTableManifestDigest|fixed_window_shared_table_manifest_digest|fixedWindowTableBaseDigest|FixedWindowTableBaseDigest|fixed_window_table_base_digest|verifierWitnessBatchDigest|VerifierWitnessBatchDigest|verifier_witness_batch_digest|recursiveProofChainDigest|RecursiveProofChainDigest|recursive_proof_chain_digest|proofChainDigest|ProofChainDigest|proof_chain_digest|transitionProfileBindingDigest|TransitionProfileBindingDigest|transition_profile_binding_digest|appendOpeningPreflightDigest|AppendOpeningPreflightDigest|append_opening_preflight_digest|appendBoundaryDigest|AppendBoundaryDigest|append_boundary_digest|recursiveVerifierScalarProjectionDigest|RecursiveVerifierScalarProjectionDigest|recursive_verifier_scalar_projection_digest|previousAccumulatorDigest|PreviousAccumulatorDigest|previous_accumulator_digest|resultingAccumulatorDigest|ResultingAccumulatorDigest|resulting_accumulator_digest|accumulatorDigest|AccumulatorDigest|accumulator_digest)/u;
  const accumulatorMaterialDeclarationPattern =
    /\b[A-Za-z0-9_]*(?:lineageAccumulator|LineageAccumulator|lineage_accumulator|aggregationTranscript|AggregationTranscript|aggregation_transcript|fixedWindowTableSchedule|FixedWindowTableSchedule|fixed_window_table_schedule|fixedWindowSharedTableManifest|FixedWindowSharedTableManifest|fixed_window_shared_table_manifest|fixedWindowTableBase|FixedWindowTableBase|fixed_window_table_base|verifierWitnessBatch|VerifierWitnessBatch|verifier_witness_batch|recursiveProofChain|RecursiveProofChain|recursive_proof_chain|proofChain|ProofChain|proof_chain|appendAccumulator|AppendAccumulator|append_accumulator|recursiveAccumulator|RecursiveAccumulator|recursive_accumulator|terminalAccumulator|TerminalAccumulator|terminal_accumulator|walletRecursiveProofChain|WalletRecursiveProofChain|wallet_recursive_proof_chain|transitionProfileBinding|TransitionProfileBinding|transition_profile_binding|appendOpeningPreflight|AppendOpeningPreflight|append_opening_preflight|recursiveVerifierScalarProjection|RecursiveVerifierScalarProjection|recursive_verifier_scalar_projection|previousAccumulator|PreviousAccumulator|previous_accumulator|resultingAccumulator|ResultingAccumulator|resulting_accumulator|accumulatorSnapshot|AccumulatorSnapshot|accumulator_snapshot|recursiveSnapshot|RecursiveSnapshot|recursive_snapshot|lineageSnapshot|LineageSnapshot|lineage_snapshot|proofState|ProofState|proof_state|recursiveProofState|RecursiveProofState|recursive_proof_state|lineageProofState|LineageProofState|lineage_proof_state|accumulatorState|AccumulatorState|accumulator_state)/u;
  for (const forbiddenName of [
    "lineageDigestV1",
    "LineageDigestBytes",
    "lineage_digest_v1",
    "aggregationTranscriptDigestV1",
    "AggregationTranscriptDigestBytes",
    "aggregation_transcript_digest_bytes",
    "fixedWindowTableScheduleDigestV1",
    "FixedWindowTableScheduleDigestBytes",
    "fixed_window_table_schedule_digest_bytes",
    "fixedWindowSharedTableManifestDigestV1",
    "FixedWindowSharedTableManifestDigestBytes",
    "fixed_window_shared_table_manifest_digest_bytes",
    "fixedWindowTableBaseDigestV1",
    "FixedWindowTableBaseDigestBytes",
    "fixed_window_table_base_digest_bytes",
    "verifierWitnessBatchDigestV1",
    "VerifierWitnessBatchDigestBytes",
    "verifier_witness_batch_digest_bytes",
    "recursiveProofChainDigestV1",
    "RecursiveProofChainDigestBytes",
    "recursive_proof_chain_digest_bytes",
    "proofChainDigestV1",
    "ProofChainDigestBytes",
    "proof_chain_digest_bytes",
    "transitionProfileBindingDigestV1",
    "TransitionProfileBindingDigestBytes",
    "transition_profile_binding_digest_bytes",
    "appendOpeningPreflightDigestV1",
    "AppendOpeningPreflightDigestBytes",
    "append_opening_preflight_digest_bytes",
    "appendBoundaryDigestV1",
    "AppendBoundaryDigestBytes",
    "append_boundary_digest_bytes",
    "recursiveVerifierScalarProjectionDigestV1",
    "RecursiveVerifierScalarProjectionDigestBytes",
    "recursive_verifier_scalar_projection_digest_bytes",
    "previousAccumulatorDigestV1",
    "PreviousAccumulatorDigestBytes",
    "previous_accumulator_digest_v1",
    "resultingAccumulatorDigestV1",
    "ResultingAccumulatorDigestBytes",
    "resulting_accumulator_digest_bytes",
    "terminalAccumulatorDigest",
    "terminalAccumulatorDigestV1",
    "TerminalAccumulatorDigest",
    "terminal_accumulator_digest",
    "terminal_accumulator_digest_v1",
    "walletRecursiveProofChainDigest",
    "walletRecursiveProofChainDigestBytes",
    "wallet_recursive_proof_chain_digest",
    "wallet_recursive_proof_chain_digest_bytes",
    "accumulatorDigestV1",
    "AccumulatorDigestBytes",
    "accumulator_digest_bytes",
  ]) {
    assert.match(
      forbiddenName,
      accumulatorDigestDeclarationPattern,
      `${forbiddenName} must be covered by the accumulator digest denylist`,
    );
  }
  for (const forbiddenName of [
    "terminalAccumulator",
    "terminalAccumulatorV1",
    "TerminalAccumulator",
    "terminal_accumulator",
    "terminal_accumulator_v1",
    "walletRecursiveProofChain",
    "walletRecursiveProofChainBytes",
    "WalletRecursiveProofChain",
    "WalletRecursiveProofChainBytes",
    "wallet_recursive_proof_chain",
    "wallet_recursive_proof_chain_bytes",
    "recursiveProofChainBytes",
    "inputTerminalAccumulator",
    "staleWalletRecursiveProofChain",
    "nativeAppendAccumulatorState",
    "publicProofChainBytes",
    "privateAccumulatorState",
    "externalRecursiveAccumulatorBytes",
    "aggregationTranscript",
    "AggregationTranscriptBytes",
    "aggregation_transcript_v1",
    "fixedWindowTableSchedule",
    "FixedWindowTableScheduleBytes",
    "fixed_window_table_schedule_v1",
    "fixedWindowSharedTableManifest",
    "FixedWindowSharedTableManifestBytes",
    "fixed_window_shared_table_manifest_v1",
    "fixedWindowTableBase",
    "FixedWindowTableBaseBytes",
    "fixed_window_table_base_v1",
    "verifierWitnessBatch",
    "VerifierWitnessBatchBytes",
    "verifier_witness_batch_v1",
    "proofChain",
    "ProofChainState",
    "proof_chain_bytes",
    "appendAccumulator",
    "AppendAccumulatorState",
    "append_accumulator_bytes",
    "recursiveAccumulator",
    "recursiveAccumulatorV1",
    "recursive_accumulator_bytes",
    "transitionProfileBinding",
    "TransitionProfileBindingBytes",
    "transition_profile_binding_v1",
    "appendOpeningPreflight",
    "AppendOpeningPreflightBytes",
    "append_opening_preflight_v1",
    "recursiveVerifierScalarProjection",
    "RecursiveVerifierScalarProjectionBytes",
    "recursive_verifier_scalar_projection_v1",
    "previousAccumulator",
    "PreviousAccumulatorBytes",
    "previous_accumulator_v1",
    "resultingAccumulator",
    "ResultingAccumulatorBytes",
    "resulting_accumulator_v1",
    "lineageAccumulatorState",
    "LineageAccumulatorState",
    "lineage_accumulator_state",
    "recursiveAccumulatorState",
    "RecursiveAccumulatorState",
    "recursive_accumulator_state",
    "recursiveAccumulatorStateBytes",
    "recursive_accumulator_state_bytes",
    "accumulatorSnapshot",
    "AccumulatorSnapshotBytes",
    "accumulator_snapshot_v1",
    "recursiveSnapshot",
    "RecursiveSnapshotBytes",
    "recursive_snapshot_v1",
    "lineageSnapshot",
    "LineageSnapshotBytes",
    "lineage_snapshot_v1",
    "proofState",
    "ProofStateBytes",
    "proof_state_v1",
    "recursiveProofState",
    "RecursiveProofStateBytes",
    "recursive_proof_state_v1",
    "lineageProofState",
    "LineageProofStateBytes",
    "lineage_proof_state_v1",
    "accumulatorState",
    "AccumulatorStateBytes",
    "accumulator_state_v1",
  ]) {
    assert.match(
      forbiddenName,
      accumulatorMaterialDeclarationPattern,
      `${forbiddenName} must be covered by the accumulator material denylist`,
    );
  }
  for (const [name, declarationsText] of PACKAGE_DECLARATION_TEXTS) {
    assert.doesNotMatch(
      declarationsText,
      accumulatorDigestDeclarationPattern,
      `${name}: recursive accumulator digests must remain native-owned`,
    );
    assert.doesNotMatch(
      declarationsText,
      accumulatorMaterialDeclarationPattern,
      `${name}: recursive accumulator material must remain native-owned`,
    );
  }
});

test("package declarations do not advertise privacy production metadata inputs", () => {
  for (const name of [
    "PrivacyProofEnvelopeInput",
    "ZkAtAuthenticatorEnvelopeInput",
    "ZkAmsAdmissionProofEnvelopeInput",
    "VegaCredentialProofEnvelopeInput",
    "SilentThresholdCredentialEnvelopeInput",
    "ZkX509IdentityEnvelopeInput",
    "JindoLatticeProofEnvelopeInput",
    "SisHintsCredentialEnvelopeInput",
    "AnonymousPgcProofMaterialInput",
    "AnonymousPgcDevProofFixtureInput",
    "VeRangeProofEnvelopeInput",
    "VeRangeProofV1Input",
    "VeRangeDevProofFixtureInput",
  ]) {
    const declaration = declarationInterfaceOrType(name);
    assert.doesNotMatch(
      declaration,
      /\bproduction\b/u,
      `${name} exposes production`,
    );
    assert.doesNotMatch(
      declaration,
      /\bproductionReady\b/u,
      `${name} exposes productionReady`,
    );
    assert.doesNotMatch(
      declaration,
      /\bproductionGate\b/u,
      `${name} exposes productionGate`,
    );
  }
});
