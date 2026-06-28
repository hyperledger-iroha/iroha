import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import test from "node:test";

import { AccountAddress } from "../src/address.js";
import {
  KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
  KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
  KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1,
  KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1,
  KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1,
  KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT,
  KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT,
  KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION,
  KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
  KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION,
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
  KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN,
  KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN,
  KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN,
  KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1,
  KAGEMUSHA_RECURSIVE_SPEND_APPEND_REQUEST_WIRE_NAME,
  KAGEMUSHA_RECURSIVE_SPEND_INIT_REQUEST_WIRE_NAME,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
  KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
  KAGEMUSHA_RECURSIVE_SPEND_RECORD_BUNDLE_WIRE_NAME,
  KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_WIRE_NAME,
  KAGEMUSHA_RECURSIVE_SPEND_VERIFY_REQUEST_WIRE_NAME,
  KAGEMUSHA_RECURSIVE_SPEND_VERIFY_RESULT_WIRE_NAME,
  KAGEMUSHA_VERIFYING_KEY_RECORD_WIRE_NAME,
  buildKagemushaRecursiveSpendableNoteDescriptor,
  buildKagemushaRecursiveSpendVerifierRecordRef,
  canAppendKagemushaRecursiveSpendWitnesslessLineage,
  canProveKagemushaRecursiveSpendAppendOutputProofCircuitId,
  canRedeemKagemushaRecursiveSpendWitnessless,
  canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId,
  decodeKagemushaRecursiveSpendBundle,
  decodeKagemushaRecursiveSpendVerifyResult,
  deriveConfidentialNoteV2,
  deriveConfidentialNullifierV2,
  deriveConfidentialOwnerTagV2,
  encodeKagemushaRecursiveSpendAppendRequest,
  encodeKagemushaRecursiveSpendInitRequest,
  encodeKagemushaRecursiveSpendRedeemRequest,
  encodeKagemushaRecursiveSpendVerifyRequest,
  isKagemushaCompactPaymentTokenNativeAvailable,
  isKagemushaPallasOpenEnvelopeBuilderNativeAvailable,
  isKagemushaRecursiveAggregationProofBundleNativeAvailable,
  isKagemushaRecursiveCompactPaymentTokenNativeAvailable,
  isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable,
  isKagemushaRecursiveCompactUnavailable,
  isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable,
  isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable,
  isKagemushaRecursiveSpendLineageProofCircuitId,
  isKagemushaRecursiveSpendLineageAppendOutputCircuitId,
  isKagemushaRecursiveSpendNativeAvailable,
  isSupportedKagemushaRecursiveSpendAppendOutputProofCircuitId,
  isSupportedKagemushaRecursiveSpendAppendProofTransition,
  isSupportedKagemushaRecursiveSpendPreviousProofCircuitId,
  kagemushaBuildPallasOpenEnvelopesArchive,
  kagemushaBuildPreviousProofOpenEnvelopesArchive,
  kagemushaProveVerifiedCompactPaymentTokenWithRecords,
  kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes,
  kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes,
  kagemushaVerifyRecursiveCompactPaymentToken,
  kagemushaRecursiveSpendCompactPaymentTokenFromBundle,
  kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection,
  kagemushaRecursiveSpendAppend,
  kagemushaRecursiveSpendInit,
  kagemushaRecursiveSpendLineageKeyArtifactsForAppend,
  kagemushaRecursiveSpendLineageKeyArtifactsForInit,
  kagemushaRecursiveSpendLineageAppendBoundary,
  kagemushaRecursiveSpendLineageWitnessAppendResult,
  kagemushaRecursiveSpendLineageWitnessFromInitResult,
  kagemushaRecursiveSpendRedeem,
  kagemushaRecursiveSpendAppendTyped,
  kagemushaRecursiveSpendInitTyped,
  kagemushaRecursiveSpendRedeemTyped,
  kagemushaRecursiveSpendTransitionProfileAppend,
  kagemushaRecursiveSpendTransitionProfileInit,
  kagemushaRecursiveSpendVerify,
  kagemushaRecursiveSpendVerifyTyped,
  normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId,
  preferredKagemushaOfflineSpendMode,
  preferredKagemushaOfflineSpendModeForCapabilities,
  preferredKagemushaRecursiveSpendAppendOutputProofCircuitId,
  requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput,
  requiresKagemushaRecursiveSpendLineageKeyArtifactsForInit,
  requiresKagemushaRecursiveSpendLineageWitnessForRedeem,
  requiresKagemushaRecursiveSpendPreviousLineageVerifierRecordForAppend,
  requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend,
} from "../src/crypto.js";

const UNSUPPORTED_RECURSIVE_SPEND_PROOF_CIRCUIT_ID =
  "kagemusha-recursive-spend-lineage-badhop-v1";

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

function kagemushaRequestCodecError(kind, field, messagePattern) {
  return (error) =>
    error?.kind === kind &&
    error.field === field &&
    (messagePattern == null ||
      (typeof messagePattern === "string"
        ? error.message === messagePattern
        : messagePattern.test(error.message)));
}

function completeRecursiveSpendBinding(overrides = {}) {
  return {
    connectNoritoBridgeAbiVersion() {
      return KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION + 1;
    },
    kagemushaRecursiveSpendInit(request) {
      rejectMalformedProbe("init", request);
      return kagemushaNoritoFrameWithPayload(0x31);
    },
    kagemushaRecursiveSpendAppend(request) {
      rejectMalformedProbe("append", request);
      return kagemushaNoritoFrameWithPayload(0x32);
    },
    kagemushaRecursiveSpendTransitionProfileInit(request) {
      rejectMalformedProbe("transition-profile-init", request);
      return kagemushaNoritoFrameWithPayload(0x37);
    },
    kagemushaRecursiveSpendTransitionProfileAppend(request) {
      rejectMalformedProbe("transition-profile-append", request);
      return kagemushaNoritoFrameWithPayload(0x38);
    },
    kagemushaRecursiveSpendLineageAppendBoundary(profile) {
      rejectMalformedProbe("lineage-append-boundary", profile);
      return kagemushaNoritoFrameWithPayload(0x39);
    },
    kagemushaRecursiveSpendLineageWitnessFromInitResult(request, bundle) {
      rejectMalformedProbe("lineage-init", request, bundle);
      return kagemushaNoritoFrameWithPayload(0x33);
    },
    kagemushaRecursiveSpendLineageWitnessAppendResult(previousWitness, request, bundle) {
      rejectMalformedProbe("lineage-append", previousWitness, request, bundle);
      return kagemushaNoritoFrameWithPayload(0x34);
    },
    kagemushaRecursiveSpendVerify(request) {
      rejectMalformedProbe("verify", request);
      return kagemushaNoritoFrameWithPayload(0x35);
    },
    kagemushaRecursiveSpendRedeem(request) {
      rejectMalformedProbe("redeem", request);
      return kagemushaNoritoFrameWithPayload(0x36);
    },
    ...overrides,
  };
}

function kagemushaNoritoFrame(schemaByte) {
  const frame = Buffer.alloc(40);
  frame.write("NRT0", 0, "ascii");
  frame.fill(schemaByte, 6, 22);
  return frame;
}

const TEST_NORITO_COMPACT_LEN_FLAG = 0x02;
const TEST_NORITO_PACKED_STRUCT_FLAG = 0x04;
const TEST_NORITO_FIELD_BITSET_FLAG = 0x20;
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

function kagemushaNoritoFrameWithPayload(schemaByte) {
  const frame = Buffer.concat([
    kagemushaNoritoFrame(schemaByte),
    Buffer.from([0x00, 0x00, 0xa5, 0x5a, 0x11]),
  ]);
  frame.writeBigUInt64LE(3n, 23);
  Buffer.from([0xb9, 0xd3, 0xa8, 0x0c, 0xcd, 0x5d, 0x13, 0x24]).copy(frame, 31);
  return frame;
}

function kagemushaNoritoFrameWithHeaderPadding(archive, padding) {
  return Buffer.concat([
    archive.subarray(0, 40),
    Buffer.from(padding),
    archive.subarray(40),
  ]);
}

const TEST_CRC64_MASK = 0xffff_ffff_ffff_ffffn;
const TEST_CRC64_REFLECTED_POLY = 0xc96c_5795_d787_0f42n;
const TEST_CRC64_TABLE = (() => {
  const table = new Array(256);
  for (let index = 0; index < 256; index += 1) {
    let crc = BigInt(index);
    for (let bit = 0; bit < 8; bit += 1) {
      crc =
        (crc & 1n) !== 0n
          ? (crc >> 1n) ^ TEST_CRC64_REFLECTED_POLY
          : crc >> 1n;
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

function kagemushaNoritoFrameFromPayload(schemaByte, payload) {
  const payloadBuffer = Buffer.from(payload);
  const frame = Buffer.concat([kagemushaNoritoFrame(schemaByte), payloadBuffer]);
  frame.writeBigUInt64LE(BigInt(payloadBuffer.length), 23);
  frame.writeBigUInt64LE(testCrc64(payloadBuffer), 31);
  return frame;
}

function kagemushaNoritoFrameFromSchemaHash(schemaHash, payload, flags = 0) {
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

function kagemushaOverlongCompactLength(value) {
  if (value < 0 || value >= 0x80) {
    throw new Error("test helper only encodes small overlong lengths");
  }
  return Buffer.from([value | 0x80, 0x00]);
}

function kagemushaOversizedTerminalCompactLength() {
  return Buffer.concat([Buffer.alloc(9, 0x80), Buffer.from([0x02])]);
}

function kagemushaHugeCanonicalCompactLength() {
  return Buffer.concat([Buffer.alloc(9, 0x80), Buffer.from([0x01])]);
}

function kagemushaNoritoField(payload, flags = TEST_NORITO_COMPACT_LEN_FLAG) {
  const bytes = Buffer.from(payload);
  return Buffer.concat([kagemushaNoritoLength(bytes.length, flags), bytes]);
}

function kagemushaNoritoString(value, flags = TEST_NORITO_COMPACT_LEN_FLAG) {
  const bytes = Buffer.from(value, "utf8");
  return Buffer.concat([kagemushaNoritoLength(bytes.length, flags), bytes]);
}

function kagemushaNoritoByteVec(value) {
  const bytes = Buffer.from(value);
  const length = Buffer.alloc(8);
  length.writeBigUInt64LE(BigInt(bytes.length));
  return Buffer.concat([length, bytes]);
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
  const backend = Buffer.from(KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND, "utf8");
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

function kagemushaLineageProvingKeyArchive(circuitId, verifierKey, seed, options = {}) {
  const flags = options.flags ?? TEST_NORITO_COMPACT_LEN_FLAG;
  const version = Buffer.alloc(2);
  version.writeUInt16LE(options.version ?? 1);
  const vkCommitment = options.vkCommitment ?? kagemushaVerifierKeyCommitment(verifierKey);
  const provingKey = options.provingKey ?? Buffer.alloc(64, seed);
  const payload = Buffer.concat([
    kagemushaNoritoField(version, flags),
    kagemushaNoritoField(kagemushaNoritoString(circuitId, flags), flags),
    kagemushaNoritoField(vkCommitment, flags),
    kagemushaNoritoField(kagemushaNoritoByteVec(provingKey), flags),
    options.trailingPayload ?? Buffer.alloc(0),
  ]);
  return kagemushaNoritoFrameFromSchemaHash(
    options.schemaHash ?? KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
    payload,
    flags,
  );
}

function kagemushaInputArchive(schemaByte = 0x50) {
  return kagemushaNoritoFrameWithPayload(schemaByte);
}

const RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE = kagemushaInputArchive(0xe1);
const RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE = kagemushaInputArchive(0xe2);

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

function sharedRecursiveSpendAbi7Archives() {
  return JSON.parse(
    readFileSync(
      new URL("../../../fixtures/kagemusha_recursive_spend_abi7/archives.json", import.meta.url),
      "utf8",
    ),
  );
}

function sharedRecursiveSpendAbi7Manifest() {
  return JSON.parse(
    readFileSync(
      new URL("../../../fixtures/kagemusha_recursive_spend_abi7/manifest.json", import.meta.url),
      "utf8",
    ),
  );
}

function sharedRecursiveSpendArchive(name) {
  const entry = sharedRecursiveSpendArchives().archives.find(
    (archive) => archive.name === name,
  );
  assert.ok(entry, `missing ABI-6 archive fixture ${name}`);
  return Buffer.from(entry.bytes_base64, "base64");
}

function sharedRecursiveSpendAbi7Archive(name) {
  const entry = sharedRecursiveSpendAbi7Archives().archives.find(
    (archive) => archive.name === name,
  );
  assert.ok(entry, `missing ABI-7 archive fixture ${name}`);
  return Buffer.from(entry.bytes_base64, "base64");
}

function kagemushaSchemaHashForTypeName(typeName) {
  return createHash("sha256")
    .update(Buffer.from("norito:v1:type-name\0", "utf8"))
    .update(Buffer.from(typeName, "utf8"))
    .digest()
    .subarray(0, 16);
}

function syntheticKagemushaArchive(typeName, seed, flags = TEST_NORITO_COMPACT_LEN_FLAG) {
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(typeName),
    Buffer.from([seed]),
    flags,
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
  return kagemushaNoritoFrameFromSchemaHash(
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
  return kagemushaNoritoFrameFromSchemaHash(
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

function assertKagemushaArchiveSchema(archive, typeName) {
  const buffer = Buffer.from(archive);
  assert.equal(buffer.subarray(0, 4).toString("ascii"), "NRT0");
  assert.deepEqual(buffer.subarray(6, 22), kagemushaSchemaHashForTypeName(typeName));
  assert.equal(buffer[22], 0);
  assert.equal(buffer[39], TEST_NORITO_COMPACT_LEN_FLAG);
  const length = Number(buffer.readBigUInt64LE(23));
  const payload = buffer.subarray(buffer.length - length);
  assert.equal(testCrc64(payload), buffer.readBigUInt64LE(31));
  return payload;
}

function kagemushaReadCompactLength(buffer, offset) {
  let value = 0n;
  let shift = 0n;
  let cursor = offset;
  for (let index = 0; index < 10; index += 1) {
    assert.ok(cursor < buffer.length, "compact length must not be truncated");
    const byte = BigInt(buffer[cursor]);
    cursor += 1;
    value |= (byte & 0x7fn) << shift;
    if ((byte & 0x80n) === 0n) {
      return { value: Number(value), offset: cursor };
    }
    shift += 7n;
  }
  throw new Error("compact length is too long");
}

function kagemushaReadField(buffer, offset) {
  const length = kagemushaReadCompactLength(buffer, offset);
  const end = length.offset + length.value;
  assert.ok(end <= buffer.length, "field payload must not be truncated");
  return {
    payload: buffer.subarray(length.offset, end),
    offset: end,
  };
}

function kagemushaReadAllFields(payload) {
  const fields = [];
  let offset = 0;
  while (offset < payload.length) {
    const field = kagemushaReadField(payload, offset);
    fields.push(field.payload);
    offset = field.offset;
  }
  assert.equal(offset, payload.length);
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
    const field = kagemushaReadField(buffer, offset);
    fields.push(field.payload);
    offset = field.offset;
  }
  assert.equal(offset, buffer.length);
  return fields;
}

function kagemushaEncodeSequenceFields(fields) {
  return Buffer.concat([u64LE(fields.length), ...fields.map((field) => kagemushaNoritoField(field))]);
}

function recursiveSpendBundleWithAccumulatorField(fieldIndex, replacement) {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("init_bundle"),
    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
  );
  const bundleFields = kagemushaReadAllFields(payload);
  const accumulatorFields = kagemushaReadAllFields(bundleFields[0]);
  accumulatorFields[fieldIndex] = Buffer.from(replacement);
  bundleFields[0] = Buffer.concat(
    accumulatorFields.map((field) => kagemushaNoritoField(field)),
  );
  return kagemushaNoritoFrameFromSchemaHash(
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
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("init_bundle"),
    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
  );
  const bundleFields = kagemushaReadAllFields(payload);
  const accumulatorFields = kagemushaReadAllFields(bundleFields[0]);
  accumulatorFields[5] = kagemushaEncodeSequenceFields(
    nullifiers.map((nullifier) => Buffer.from(nullifier)),
  );
  bundleFields[0] = Buffer.concat(
    accumulatorFields.map((field) => kagemushaNoritoField(field)),
  );
  const proofFields = kagemushaReadAllFields(bundleFields[1]);
  const proofBoxFields = kagemushaReadAllFields(proofFields[3]);
  proofBoxFields[1] = kagemushaNoritoByteVec(Buffer.alloc(0));
  proofFields[3] = Buffer.concat(proofBoxFields.map((field) => kagemushaNoritoField(field)));
  bundleFields[1] = Buffer.concat(proofFields.map((field) => kagemushaNoritoField(field)));
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithTrailingBundleField() {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("init_bundle"),
    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
  );
  const bundleFields = kagemushaReadAllFields(payload);
  bundleFields.push(kagemushaNoritoString("ignored-extra-bundle-field"));
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendVerifyResultWithTrailingField() {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendAbi7Archive("verify_result"),
    KAGEMUSHA_RECURSIVE_SPEND_VERIFY_RESULT_WIRE_NAME,
  );
  const fields = kagemushaReadAllFields(payload);
  fields.push(Buffer.from([0x01]));
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_VERIFY_RESULT_WIRE_NAME),
    Buffer.concat(fields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithTopupAnchorNullifiersAndTrailingAccumulatorField(nullifiers) {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("init_bundle"),
    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
  );
  const bundleFields = kagemushaReadAllFields(payload);
  const accumulatorFields = kagemushaReadAllFields(bundleFields[0]);
  accumulatorFields[5] = kagemushaEncodeSequenceFields(
    nullifiers.map((nullifier) => Buffer.from(nullifier)),
  );
  accumulatorFields.push(kagemushaNoritoString("ignored-extra-accumulator-field"));
  bundleFields[0] = Buffer.concat(
    accumulatorFields.map((field) => kagemushaNoritoField(field)),
  );
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendLineageWitnessWithTrailingField() {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("lineage_witness_append_result"),
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
  );
  const fields = kagemushaReadAllFields(payload);
  fields.push(kagemushaNoritoString("ignored-extra-lineage-witness-field"));
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME),
    Buffer.concat(fields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendLineageWitnessWithTrailingPreviousProofsField() {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("lineage_witness_append_result"),
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
  );
  const fields = kagemushaReadAllFields(payload);
  fields[3] = Buffer.concat([
    fields[3],
    kagemushaNoritoField(kagemushaNoritoString("ignored-extra-previous-proofs-field")),
  ]);
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME),
    Buffer.concat(fields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendLineageWitnessWithPreviousProofCountPrefixOnly(count) {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("lineage_witness_append_result"),
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
  );
  const fields = kagemushaReadAllFields(payload);
  fields[3] = u64LE(count);
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME),
    Buffer.concat(fields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendLineageWitnessWithTrailingPreviousProofField() {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("lineage_witness_append_result"),
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
  );
  const fields = kagemushaReadAllFields(payload);
  const previousProofs = kagemushaReadSequenceFields(fields[3]);
  assert.ok(previousProofs.length > 0);
  const previousProofFields = kagemushaReadAllFields(previousProofs[0]);
  previousProofFields.push(kagemushaNoritoString("ignored-extra-previous-proof-field"));
  previousProofs[0] = Buffer.concat(
    previousProofFields.map((field) => kagemushaNoritoField(field)),
  );
  fields[3] = kagemushaEncodeSequenceFields(previousProofs);
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME),
    Buffer.concat(fields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendLineageWitnessWithTrailingPreviousVerifierKeyIdField() {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("lineage_witness_append_result"),
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
  );
  const fields = kagemushaReadAllFields(payload);
  const previousProofs = kagemushaReadSequenceFields(fields[3]);
  assert.ok(previousProofs.length > 0);
  const previousProofFields = kagemushaReadAllFields(previousProofs[0]);
  const verifierKeyIdFields = kagemushaReadAllFields(previousProofFields[0]);
  verifierKeyIdFields.push(kagemushaNoritoString("ignored-extra-previous-verifier-key-field"));
  previousProofFields[0] = Buffer.concat(
    verifierKeyIdFields.map((field) => kagemushaNoritoField(field)),
  );
  previousProofs[0] = Buffer.concat(
    previousProofFields.map((field) => kagemushaNoritoField(field)),
  );
  fields[3] = kagemushaEncodeSequenceFields(previousProofs);
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME),
    Buffer.concat(fields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendLineageWitnessWithPreviousProofField(fieldIndex, replacement) {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("lineage_witness_append_result"),
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
  );
  const fields = kagemushaReadAllFields(payload);
  const previousProofs = kagemushaReadSequenceFields(fields[3]);
  assert.ok(previousProofs.length > 0);
  const previousProofFields = kagemushaReadAllFields(previousProofs[0]);
  previousProofFields[fieldIndex] = Buffer.from(replacement);
  previousProofs[0] = Buffer.concat(
    previousProofFields.map((field) => kagemushaNoritoField(field)),
  );
  fields[3] = kagemushaEncodeSequenceFields(previousProofs);
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME),
    Buffer.concat(fields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendLineageWitnessWithPreviousProofBoxBackend(proofBackend) {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("lineage_witness_append_result"),
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
  );
  const fields = kagemushaReadAllFields(payload);
  const previousProofs = kagemushaReadSequenceFields(fields[3]);
  assert.ok(previousProofs.length > 0);
  const previousProofFields = kagemushaReadAllFields(previousProofs[0]);
  const proofBoxFields = kagemushaReadAllFields(previousProofFields[3]);
  proofBoxFields[0] = kagemushaNoritoString(proofBackend);
  previousProofFields[3] = Buffer.concat(
    proofBoxFields.map((field) => kagemushaNoritoField(field)),
  );
  previousProofs[0] = Buffer.concat(
    previousProofFields.map((field) => kagemushaNoritoField(field)),
  );
  fields[3] = kagemushaEncodeSequenceFields(previousProofs);
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME),
    Buffer.concat(fields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendLineageWitnessWithPreviousProofBoxBackendAndEmptyProofBytes(proofBackend) {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("lineage_witness_append_result"),
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
  );
  const fields = kagemushaReadAllFields(payload);
  const previousProofs = kagemushaReadSequenceFields(fields[3]);
  assert.ok(previousProofs.length > 0);
  const previousProofFields = kagemushaReadAllFields(previousProofs[0]);
  const proofBoxFields = kagemushaReadAllFields(previousProofFields[3]);
  proofBoxFields[0] = kagemushaNoritoString(proofBackend);
  proofBoxFields[1] = kagemushaNoritoByteVec(Buffer.alloc(0));
  previousProofFields[3] = Buffer.concat(
    proofBoxFields.map((field) => kagemushaNoritoField(field)),
  );
  previousProofs[0] = Buffer.concat(
    previousProofFields.map((field) => kagemushaNoritoField(field)),
  );
  fields[3] = kagemushaEncodeSequenceFields(previousProofs);
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME),
    Buffer.concat(fields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendLineageWitnessWithEmptyPreviousProofBytes() {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("lineage_witness_append_result"),
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
  );
  const fields = kagemushaReadAllFields(payload);
  const previousProofs = kagemushaReadSequenceFields(fields[3]);
  assert.ok(previousProofs.length > 0);
  const previousProofFields = kagemushaReadAllFields(previousProofs[0]);
  const proofBoxFields = kagemushaReadAllFields(previousProofFields[3]);
  proofBoxFields[1] = u64LE(0);
  previousProofFields[3] = Buffer.concat(
    proofBoxFields.map((field) => kagemushaNoritoField(field)),
  );
  previousProofs[0] = Buffer.concat(
    previousProofFields.map((field) => kagemushaNoritoField(field)),
  );
  fields[3] = kagemushaEncodeSequenceFields(previousProofs);
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME),
    Buffer.concat(fields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithTrailingAccumulatorField() {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("init_bundle"),
    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
  );
  const bundleFields = kagemushaReadAllFields(payload);
  const accumulatorFields = kagemushaReadAllFields(bundleFields[0]);
  accumulatorFields.push(kagemushaNoritoString("ignored-extra-accumulator-field"));
  bundleFields[0] = Buffer.concat(
    accumulatorFields.map((field) => kagemushaNoritoField(field)),
  );
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithCurrentNoteField(fieldIndex, replacement) {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("init_bundle"),
    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
  );
  const bundleFields = kagemushaReadAllFields(payload);
  const accumulatorFields = kagemushaReadAllFields(bundleFields[0]);
  const currentNoteFields = kagemushaReadAllFields(accumulatorFields[22]);
  currentNoteFields[fieldIndex] = Buffer.from(replacement);
  accumulatorFields[22] = Buffer.concat(
    currentNoteFields.map((field) => kagemushaNoritoField(field)),
  );
  bundleFields[0] = Buffer.concat(
    accumulatorFields.map((field) => kagemushaNoritoField(field)),
  );
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithTrailingCurrentNoteField() {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("init_bundle"),
    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
  );
  const bundleFields = kagemushaReadAllFields(payload);
  const accumulatorFields = kagemushaReadAllFields(bundleFields[0]);
  const currentNoteFields = kagemushaReadAllFields(accumulatorFields[22]);
  currentNoteFields.push(kagemushaNoritoString("ignored-extra-current-note-field"));
  accumulatorFields[22] = Buffer.concat(
    currentNoteFields.map((field) => kagemushaNoritoField(field)),
  );
  bundleFields[0] = Buffer.concat(
    accumulatorFields.map((field) => kagemushaNoritoField(field)),
  );
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithCurrentNoteFieldAndTrailingField(fieldIndex, replacement) {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("init_bundle"),
    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
  );
  const bundleFields = kagemushaReadAllFields(payload);
  const accumulatorFields = kagemushaReadAllFields(bundleFields[0]);
  const currentNoteFields = kagemushaReadAllFields(accumulatorFields[22]);
  currentNoteFields[fieldIndex] = Buffer.from(replacement);
  currentNoteFields.push(kagemushaNoritoString("ignored-extra-current-note-field"));
  accumulatorFields[22] = Buffer.concat(
    currentNoteFields.map((field) => kagemushaNoritoField(field)),
  );
  bundleFields[0] = Buffer.concat(
    accumulatorFields.map((field) => kagemushaNoritoField(field)),
  );
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithEqualCurrentNoteNullifier() {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("init_bundle"),
    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
  );
  const bundleFields = kagemushaReadAllFields(payload);
  const accumulatorFields = kagemushaReadAllFields(bundleFields[0]);
  const currentNoteFields = kagemushaReadAllFields(accumulatorFields[22]);
  currentNoteFields[1] = Buffer.from(currentNoteFields[0]);
  accumulatorFields[22] = Buffer.concat(
    currentNoteFields.map((field) => kagemushaNoritoField(field)),
  );
  bundleFields[0] = Buffer.concat(
    accumulatorFields.map((field) => kagemushaNoritoField(field)),
  );
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithEqualCurrentNoteNullifierAndTrailingField() {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("init_bundle"),
    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
  );
  const bundleFields = kagemushaReadAllFields(payload);
  const accumulatorFields = kagemushaReadAllFields(bundleFields[0]);
  const currentNoteFields = kagemushaReadAllFields(accumulatorFields[22]);
  currentNoteFields[1] = Buffer.from(currentNoteFields[0]);
  currentNoteFields.push(kagemushaNoritoString("ignored-extra-current-note-field"));
  accumulatorFields[22] = Buffer.concat(
    currentNoteFields.map((field) => kagemushaNoritoField(field)),
  );
  bundleFields[0] = Buffer.concat(
    accumulatorFields.map((field) => kagemushaNoritoField(field)),
  );
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithProofCircuitId(proofCircuitId) {
  const payload = Buffer.from(
    assertKagemushaArchiveSchema(
      sharedRecursiveSpendArchive("init_bundle"),
      KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
    ),
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
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    payload,
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithProofBackend(proofBackend) {
  const payload = Buffer.from(
    assertKagemushaArchiveSchema(
      sharedRecursiveSpendArchive("init_bundle"),
      KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
    ),
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
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    payload,
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithProofBoxBackend(proofBackend) {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("init_bundle"),
    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
  );
  const bundleFields = kagemushaReadAllFields(payload);
  const proofFields = kagemushaReadAllFields(bundleFields[1]);
  const proofBoxFields = kagemushaReadAllFields(proofFields[3]);
  proofBoxFields[0] = kagemushaNoritoString(proofBackend);
  proofFields[3] = Buffer.concat(proofBoxFields.map((field) => kagemushaNoritoField(field)));
  bundleFields[1] = Buffer.concat(proofFields.map((field) => kagemushaNoritoField(field)));
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithProofBoxBackendAndEmptyProofBytes(proofBackend) {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("init_bundle"),
    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
  );
  const bundleFields = kagemushaReadAllFields(payload);
  const proofFields = kagemushaReadAllFields(bundleFields[1]);
  const proofBoxFields = kagemushaReadAllFields(proofFields[3]);
  proofBoxFields[0] = kagemushaNoritoString(proofBackend);
  proofBoxFields[1] = kagemushaNoritoByteVec(Buffer.alloc(0));
  proofFields[3] = Buffer.concat(proofBoxFields.map((field) => kagemushaNoritoField(field)));
  bundleFields[1] = Buffer.concat(proofFields.map((field) => kagemushaNoritoField(field)));
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithTrailingVerifierKeyIdField() {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("init_bundle"),
    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
  );
  const bundleFields = kagemushaReadAllFields(payload);
  const proofFields = kagemushaReadAllFields(bundleFields[1]);
  const verifierKeyIdFields = kagemushaReadAllFields(proofFields[0]);
  verifierKeyIdFields.push(kagemushaNoritoString("ignored-extra-verifier-key-field"));
  proofFields[0] = Buffer.concat(
    verifierKeyIdFields.map((field) => kagemushaNoritoField(field)),
  );
  bundleFields[1] = Buffer.concat(proofFields.map((field) => kagemushaNoritoField(field)));
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithTrailingRecursiveProofField() {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("init_bundle"),
    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
  );
  const bundleFields = kagemushaReadAllFields(payload);
  const proofFields = kagemushaReadAllFields(bundleFields[1]);
  proofFields.push(kagemushaNoritoString("ignored-extra-recursive-proof-field"));
  bundleFields[1] = Buffer.concat(proofFields.map((field) => kagemushaNoritoField(field)));
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithTrailingProofBoxField() {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("init_bundle"),
    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
  );
  const bundleFields = kagemushaReadAllFields(payload);
  const proofFields = kagemushaReadAllFields(bundleFields[1]);
  const proofBoxFields = kagemushaReadAllFields(proofFields[3]);
  proofBoxFields.push(kagemushaNoritoString("ignored-extra-proof-box-field"));
  proofFields[3] = Buffer.concat(proofBoxFields.map((field) => kagemushaNoritoField(field)));
  bundleFields[1] = Buffer.concat(proofFields.map((field) => kagemushaNoritoField(field)));
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithEmptyProofBytes() {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("init_bundle"),
    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
  );
  const bundleFields = kagemushaReadAllFields(payload);
  const proofFields = kagemushaReadAllFields(bundleFields[1]);
  const proofBoxFields = kagemushaReadAllFields(proofFields[3]);
  proofBoxFields[1] = kagemushaNoritoByteVec(Buffer.alloc(0));
  proofFields[3] = Buffer.concat(proofBoxFields.map((field) => kagemushaNoritoField(field)));
  bundleFields[1] = Buffer.concat(proofFields.map((field) => kagemushaNoritoField(field)));
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithEmptyProofPublicInputs() {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("init_bundle"),
    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
  );
  const bundleFields = kagemushaReadAllFields(payload);
  const proofFields = kagemushaReadAllFields(bundleFields[1]);
  proofFields[1] = Buffer.alloc(0);
  bundleFields[1] = Buffer.concat(proofFields.map((field) => kagemushaNoritoField(field)));
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithZeroProofPublicInputsHash() {
  return recursiveSpendBundleWithProofPublicInputsHash(Buffer.alloc(32));
}

function recursiveSpendBundleWithProofPublicInputsHash(replacement) {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("init_bundle"),
    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
  );
  const bundleFields = kagemushaReadAllFields(payload);
  const proofFields = kagemushaReadAllFields(bundleFields[1]);
  proofFields[2] = replacement;
  bundleFields[1] = Buffer.concat(proofFields.map((field) => kagemushaNoritoField(field)));
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
}

function recursiveSpendBundleWithMismatchedProofPublicInputsHash() {
  const payload = assertKagemushaArchiveSchema(
    sharedRecursiveSpendArchive("init_bundle"),
    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
  );
  const bundleFields = kagemushaReadAllFields(payload);
  const proofFields = kagemushaReadAllFields(bundleFields[1]);
  const mismatchedHash = Buffer.from(proofFields[2]);
  mismatchedHash[0] ^= 0x01;
  proofFields[2] = mismatchedHash;
  bundleFields[1] = Buffer.concat(proofFields.map((field) => kagemushaNoritoField(field)));
  return kagemushaNoritoFrameFromSchemaHash(
    kagemushaSchemaHashForTypeName(KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME),
    Buffer.concat(bundleFields.map((field) => kagemushaNoritoField(field))),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
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

function kagemushaZeroNumericPayload() {
  return kagemushaNumericPayload(Buffer.alloc(0));
}

function kagemushaReadOptionSome(payload) {
  assert.equal(payload[0], 1);
  const field = kagemushaReadField(payload, 1);
  assert.equal(field.offset, payload.length);
  return field.payload;
}

function kagemushaReadOptionNone(payload) {
  assert.deepEqual(payload, Buffer.from([0]));
}

function kagemushaReadFixedBytesPayload(payload, expectedLength) {
  const bytes = [];
  let offset = 0;
  while (offset < payload.length) {
    const field = kagemushaReadField(payload, offset);
    assert.equal(field.payload.length, 1);
    bytes.push(field.payload[0]);
    offset = field.offset;
  }
  assert.equal(bytes.length, expectedLength);
  return Buffer.from(bytes);
}

function recursiveSpendNote(amount = "7", commitmentSeed = 0x21, nullifierSeed = 0x22) {
  return buildKagemushaRecursiveSpendableNoteDescriptor({
    noteCommitment: Buffer.alloc(32, commitmentSeed),
    spendNullifier: Buffer.alloc(32, nullifierSeed),
    amount,
  });
}

function recursiveSpendVerifierRecord() {
  return buildKagemushaRecursiveSpendVerifierRecordRef({
    verifierKeyId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    recordBytes: syntheticKagemushaArchive(KAGEMUSHA_VERIFYING_KEY_RECORD_WIRE_NAME, 0x66),
  });
}

function recursiveSpendRecipient() {
  return AccountAddress.fromAccount({ publicKey: Buffer.alloc(32, 0x44) }).toI105();
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
      () => kagemushaRecursiveSpendLineageWitnessFromInitResult(Buffer.alloc(0), kagemushaInputArchive(0x51)),
      /requestArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageWitnessFromInitResult(kagemushaInputArchive(0x52), Buffer.alloc(0)),
      /bundleArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageWitnessAppendResult(Buffer.alloc(0), kagemushaInputArchive(0x53), kagemushaInputArchive(0x54)),
      /previousWitnessArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageWitnessAppendResult(kagemushaInputArchive(0x55), Buffer.alloc(0), kagemushaInputArchive(0x56)),
      /requestArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageWitnessAppendResult(kagemushaInputArchive(0x57), kagemushaInputArchive(0x58), Buffer.alloc(0)),
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
    assert.throws(
      () => kagemushaProveVerifiedCompactPaymentTokenWithRecords(Buffer.alloc(0)),
      /recordBundleArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaBuildPallasOpenEnvelopesArchive(Buffer.alloc(0)),
      /recordBundleArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaBuildPreviousProofOpenEnvelopesArchive(Buffer.alloc(0)),
      /previousBundleArchive must not be empty/,
    );
    assert.throws(
      () =>
        kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
          Buffer.alloc(0),
          kagemushaInputArchive(0xd1),
        ),
      /recordBundleArchive must not be empty/,
    );
    assert.throws(
      () =>
        kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
          kagemushaInputArchive(0xd2),
          Buffer.alloc(0),
        ),
      /pallasOpenEnvelopesArchive must not be empty/,
    );
    assert.throws(
      () =>
        kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
          Buffer.alloc(0),
          kagemushaInputArchive(0xc7),
          RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        ),
      /recordBundleArchive must not be empty/,
    );
    assert.throws(
      () =>
        kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
          kagemushaInputArchive(0xc8),
          Buffer.alloc(0),
          RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        ),
      /pallasOpenEnvelopesArchive must not be empty/,
    );
    assert.throws(
      () =>
        kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
          kagemushaInputArchive(0xc9),
          kagemushaInputArchive(0xca),
          Buffer.alloc(0),
        ),
      /recursiveCompactKeyArtifactsArchive must not be empty/,
    );
    assert.throws(
      () =>
        kagemushaVerifyRecursiveCompactPaymentToken(
          Buffer.alloc(0),
          RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
      ),
      /compactTokenArchive must not be empty/,
    );
    assert.throws(
      () =>
        kagemushaVerifyRecursiveCompactPaymentToken(
          kagemushaInputArchive(0x4b),
          Buffer.alloc(0),
        ),
      /recursiveCompactVerifierKeysArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendCompactPaymentTokenFromBundle(Buffer.alloc(0)),
      /bundleArchive must not be empty/,
    );
    assert.throws(
      () =>
        kagemushaVerifyRecursiveCompactPaymentToken(
          Buffer.from([1]),
          RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        ),
      /compactTokenArchive must be a valid Norito archive/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendCompactPaymentTokenFromBundle(Buffer.from([1])),
      /bundleArchive must be a valid Norito archive/,
    );
    assert.throws(
      () =>
        kagemushaVerifyRecursiveCompactPaymentToken(
          kagemushaNoritoFrame(0x4b),
          RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        ),
      /compactTokenArchive must contain a non-empty Norito payload/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendCompactPaymentTokenFromBundle(kagemushaNoritoFrame(0x4c)),
      /bundleArchive must contain a non-empty Norito payload/,
    );
  });
});

test("Kagemusha recursive spend helpers reject missing request archives before native calls", () => {
  const validArchive = kagemushaInputArchive(0x5f);
  const cases = [
    ["kagemushaRecursiveSpendInit", ["requestArchive"], (args) => kagemushaRecursiveSpendInit(args[0])],
    ["kagemushaRecursiveSpendAppend", ["requestArchive"], (args) => kagemushaRecursiveSpendAppend(args[0])],
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
      (args) => kagemushaRecursiveSpendLineageWitnessAppendResult(args[0], args[1], args[2]),
    ],
    ["kagemushaRecursiveSpendVerify", ["requestArchive"], (args) => kagemushaRecursiveSpendVerify(args[0])],
    ["kagemushaRecursiveSpendRedeem", ["requestArchive"], (args) => kagemushaRecursiveSpendRedeem(args[0])],
    [
      "kagemushaProveVerifiedCompactPaymentTokenWithRecords",
      ["recordBundleArchive"],
      (args) => kagemushaProveVerifiedCompactPaymentTokenWithRecords(args[0]),
    ],
    [
      "kagemushaBuildPallasOpenEnvelopesArchive",
      ["recordBundleArchive"],
      (args) => kagemushaBuildPallasOpenEnvelopesArchive(args[0]),
    ],
    [
      "kagemushaBuildPreviousProofOpenEnvelopesArchive",
      ["previousBundleArchive"],
      (args) => kagemushaBuildPreviousProofOpenEnvelopesArchive(args[0]),
    ],
    [
      "kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes",
      ["recordBundleArchive", "pallasOpenEnvelopesArchive"],
      (args) =>
        kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
          args[0],
          args[1],
        ),
    ],
    [
      "kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes",
      [
        "recordBundleArchive",
        "pallasOpenEnvelopesArchive",
        "recursiveCompactKeyArtifactsArchive",
      ],
      (args) =>
        kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
          args[0],
          args[1],
          args[2],
        ),
    ],
    [
      "kagemushaVerifyRecursiveCompactPaymentToken",
      ["compactTokenArchive", "recursiveCompactVerifierKeysArchive"],
      (args) => kagemushaVerifyRecursiveCompactPaymentToken(args[0], args[1]),
    ],
    [
      "kagemushaRecursiveSpendCompactPaymentTokenFromBundle",
      ["bundleArchive"],
      (args) => kagemushaRecursiveSpendCompactPaymentTokenFromBundle(args[0]),
    ],
    [
      "kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection",
      ["compactTokenArchive", "verifierRecordArchive"],
      (args) =>
        kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(args[0], args[1]),
    ],
  ];

  withNativeBinding({}, () => {
    for (const [helperName, archiveNames, call] of cases) {
      for (let fieldIndex = 0; fieldIndex < archiveNames.length; fieldIndex += 1) {
        const args = Array.from({ length: archiveNames.length }, () => validArchive);
        args[fieldIndex] = undefined;
        assert.throws(
          () => call(args),
          new RegExp(`${archiveNames[fieldIndex]} must be a Buffer, string, or ArrayBuffer view`),
          `${helperName} ${archiveNames[fieldIndex]} missing archive`,
        );
      }
    }
  });
});

test("Kagemusha recursive spend helpers reject oversized request archives before native calls", () => {
  const oversizedArchive = new Uint8Array(KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES + 1);
  const validArchive = kagemushaInputArchive(0x60);

  assert.throws(
    () => kagemushaRecursiveSpendInit(oversizedArchive),
    /requestArchive must not exceed/,
  );
  assert.throws(
    () => kagemushaRecursiveSpendAppend(oversizedArchive),
    /requestArchive must not exceed/,
  );
  assert.throws(
    () => kagemushaRecursiveSpendTransitionProfileInit(oversizedArchive),
    /requestArchive must not exceed/,
  );
  assert.throws(
    () => kagemushaRecursiveSpendTransitionProfileAppend(oversizedArchive),
    /requestArchive must not exceed/,
  );
  assert.throws(
    () => kagemushaRecursiveSpendLineageAppendBoundary(oversizedArchive),
    /profileArchive must not exceed/,
  );
  assert.throws(
    () => kagemushaRecursiveSpendLineageWitnessFromInitResult(oversizedArchive, validArchive),
    /requestArchive must not exceed/,
  );
  assert.throws(
    () => kagemushaRecursiveSpendLineageWitnessFromInitResult(validArchive, oversizedArchive),
    /bundleArchive must not exceed/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageWitnessAppendResult(
        oversizedArchive,
        validArchive,
        validArchive,
      ),
    /previousWitnessArchive must not exceed/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageWitnessAppendResult(
        validArchive,
        oversizedArchive,
        validArchive,
      ),
    /requestArchive must not exceed/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageWitnessAppendResult(
        validArchive,
        validArchive,
        oversizedArchive,
      ),
    /bundleArchive must not exceed/,
  );
  assert.throws(
    () => kagemushaRecursiveSpendVerify(oversizedArchive),
    /requestArchive must not exceed/,
  );
  assert.throws(
    () => kagemushaRecursiveSpendRedeem(oversizedArchive),
    /requestArchive must not exceed/,
  );
  assert.throws(
    () => kagemushaProveVerifiedCompactPaymentTokenWithRecords(oversizedArchive),
    /recordBundleArchive must not exceed/,
  );
  assert.throws(
    () => kagemushaBuildPallasOpenEnvelopesArchive(oversizedArchive),
    /recordBundleArchive must not exceed/,
  );
  assert.throws(
    () => kagemushaBuildPreviousProofOpenEnvelopesArchive(oversizedArchive),
    /previousBundleArchive must not exceed/,
  );
  assert.throws(
    () =>
      kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
        oversizedArchive,
        validArchive,
      ),
    /recordBundleArchive must not exceed/,
  );
  assert.throws(
    () =>
      kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
        validArchive,
        oversizedArchive,
      ),
    /pallasOpenEnvelopesArchive must not exceed/,
  );
  assert.throws(
    () =>
      kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        oversizedArchive,
        validArchive,
        RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
      ),
    /recordBundleArchive must not exceed/,
  );
  assert.throws(
    () =>
      kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        validArchive,
        oversizedArchive,
        RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
      ),
    /pallasOpenEnvelopesArchive must not exceed/,
  );
  assert.throws(
    () =>
      kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        validArchive,
        validArchive,
        oversizedArchive,
      ),
    /recursiveCompactKeyArtifactsArchive must not exceed/,
  );
  assert.throws(
    () =>
      kagemushaVerifyRecursiveCompactPaymentToken(
        oversizedArchive,
        RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
      ),
    /compactTokenArchive must not exceed/,
  );
  assert.throws(
    () =>
      kagemushaVerifyRecursiveCompactPaymentToken(
        validArchive,
        oversizedArchive,
      ),
    /recursiveCompactVerifierKeysArchive must not exceed/,
  );
  assert.throws(
    () => kagemushaRecursiveSpendCompactPaymentTokenFromBundle(oversizedArchive),
    /bundleArchive must not exceed/,
  );
  assert.throws(
    () =>
      kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
        oversizedArchive,
        validArchive,
      ),
    /compactTokenArchive must not exceed/,
  );
  assert.throws(
    () =>
      kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
        validArchive,
        oversizedArchive,
      ),
    /verifierRecordArchive must not exceed/,
  );
});

test("Kagemusha recursive spend helpers reject malformed Norito request archives before native calls", () => {
  assert.throws(
    () => kagemushaRecursiveSpendInit(Buffer.from([1])),
    /requestArchive must be a valid Norito archive/,
  );
  assert.throws(
    () => kagemushaRecursiveSpendLineageAppendBoundary(Buffer.from([1])),
    /profileArchive must be a valid Norito archive/,
  );
  assert.throws(
    () => kagemushaRecursiveSpendLineageWitnessFromInitResult(kagemushaInputArchive(0x59), Buffer.from([1])),
    /bundleArchive must be a valid Norito archive/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageWitnessAppendResult(
        kagemushaInputArchive(0x5a),
        Buffer.from([1]),
        kagemushaInputArchive(0x5b),
      ),
    /requestArchive must be a valid Norito archive/,
  );
  assert.throws(
    () =>
      kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        Buffer.from([1]),
        kagemushaInputArchive(0xc9),
        RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
      ),
    /recordBundleArchive must be a valid Norito archive/,
  );
  assert.throws(
    () =>
      kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        kagemushaInputArchive(0xca),
        Buffer.from([1]),
        RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
      ),
    /pallasOpenEnvelopesArchive must be a valid Norito archive/,
  );
  assert.throws(
    () =>
      kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        kagemushaInputArchive(0xca),
        kagemushaInputArchive(0xcb),
        Buffer.from([1]),
    ),
    /recursiveCompactKeyArtifactsArchive must be a valid Norito archive/,
  );
  assert.throws(
    () =>
      kagemushaVerifyRecursiveCompactPaymentToken(
        kagemushaInputArchive(0x4b),
        Buffer.from([1]),
      ),
    /recursiveCompactVerifierKeysArchive must be a valid Norito archive/,
  );
  assert.throws(
    () => kagemushaProveVerifiedCompactPaymentTokenWithRecords(Buffer.from([1])),
    /recordBundleArchive must be a valid Norito archive/,
  );
  assert.throws(
    () => kagemushaBuildPallasOpenEnvelopesArchive(Buffer.from([1])),
    /recordBundleArchive must be a valid Norito archive/,
  );
  assert.throws(
    () => kagemushaBuildPreviousProofOpenEnvelopesArchive(Buffer.from([1])),
    /previousBundleArchive must be a valid Norito archive/,
  );
  assert.throws(
    () =>
      kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
        Buffer.from([1]),
        kagemushaInputArchive(0xd3),
      ),
    /recordBundleArchive must be a valid Norito archive/,
  );
  assert.throws(
    () =>
      kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
        kagemushaInputArchive(0xd4),
        Buffer.from([1]),
      ),
    /pallasOpenEnvelopesArchive must be a valid Norito archive/,
  );
});

test("Kagemusha recursive spend helpers reject empty-payload Norito request archives before native calls", () => {
  assert.throws(
    () => kagemushaRecursiveSpendVerify(kagemushaNoritoFrame(0x5c)),
    /requestArchive must contain a non-empty Norito payload/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageWitnessAppendResult(
        kagemushaNoritoFrame(0x5d),
        kagemushaInputArchive(0x5e),
        kagemushaInputArchive(0x5f),
      ),
    /previousWitnessArchive must contain a non-empty Norito payload/,
  );
  assert.throws(
    () =>
      kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        kagemushaNoritoFrame(0xcb),
        kagemushaInputArchive(0xcc),
        RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
      ),
    /recordBundleArchive must contain a non-empty Norito payload/,
  );
  assert.throws(
    () =>
      kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        kagemushaInputArchive(0xcd),
        kagemushaNoritoFrame(0xce),
        RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
      ),
    /pallasOpenEnvelopesArchive must contain a non-empty Norito payload/,
  );
  assert.throws(
    () =>
      kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        kagemushaInputArchive(0xcd),
        kagemushaInputArchive(0xce),
        kagemushaNoritoFrame(0xcf),
    ),
    /recursiveCompactKeyArtifactsArchive must contain a non-empty Norito payload/,
  );
  assert.throws(
    () =>
      kagemushaVerifyRecursiveCompactPaymentToken(
        kagemushaInputArchive(0x4b),
        kagemushaNoritoFrame(0x4c),
      ),
    /recursiveCompactVerifierKeysArchive must contain a non-empty Norito payload/,
  );
  assert.throws(
    () => kagemushaProveVerifiedCompactPaymentTokenWithRecords(kagemushaNoritoFrame(0xd5)),
    /recordBundleArchive must contain a non-empty Norito payload/,
  );
  assert.throws(
    () => kagemushaBuildPallasOpenEnvelopesArchive(kagemushaNoritoFrame(0xda)),
    /recordBundleArchive must contain a non-empty Norito payload/,
  );
  assert.throws(
    () => kagemushaBuildPreviousProofOpenEnvelopesArchive(kagemushaNoritoFrame(0xdb)),
    /previousBundleArchive must contain a non-empty Norito payload/,
  );
  assert.throws(
    () =>
      kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
        kagemushaNoritoFrame(0xd6),
        kagemushaInputArchive(0xd7),
      ),
    /recordBundleArchive must contain a non-empty Norito payload/,
  );
  assert.throws(
    () =>
      kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
        kagemushaInputArchive(0xd8),
        kagemushaNoritoFrame(0xd9),
      ),
    /pallasOpenEnvelopesArchive must contain a non-empty Norito payload/,
  );
});

test("Kagemusha recursive spend shared ABI-6 fixture matches SDK surface", () => {
  const manifest = sharedRecursiveSpendManifest();
  assert.equal(
    manifest.schema,
    "iroha.kagemusha.recursive_spend.abi6.fixture_manifest.v1",
  );
  assert.equal(
    manifest.native_bridge_abi_version,
    KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION,
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
        "change_output",
        "lineage_verifier_record",
        "block_height",
        "lineage_verifier_records",
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
    "703128068fa36897c952640cb77006af29a8aa802d67da82c97e73c8e0ef1864",
  );
  assert.ok(redeemArchive.byte_len > 0);
  assert.ok(Buffer.from(redeemArchive.bytes_base64, "base64").length > 0);
  const redeemInstructionArchive = archiveFixture.archives.find(
    (archive) => archive.name === "redeem_instruction",
  );
  assert.equal(redeemInstructionArchive.norito_type, "RedeemKagemushaRecursive");
  assert.equal(
    redeemInstructionArchive.sha256_hex,
    "e05fb3ebb3a3e823f65403e09d1aa6e5deab0145f7aa0827f66a371ad633cc3e",
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

test("Kagemusha recursive spend shared ABI-7 fixture manifest matches archive fixture", () => {
  const manifest = sharedRecursiveSpendAbi7Manifest();
  assert.deepEqual(Object.keys(manifest).sort(), [
    "archive_fixture",
    "domains",
    "fixture_kind",
    "generator",
    "native_bridge_abi_version",
    "operation_count",
    "operations",
    "schema",
  ]);
  assert.equal(
    manifest.schema,
    "iroha.kagemusha.recursive_spend.abi7.fixture_manifest.v1",
  );
  assert.equal(manifest.fixture_kind, "native_bridge_norito_archives");
  assert.equal(
    manifest.native_bridge_abi_version,
    KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION,
  );
  assert.deepEqual(manifest.archive_fixture, {
    path: "fixtures/kagemusha_recursive_spend_abi7/archives.json",
    schema: "iroha.kagemusha.recursive_spend.abi7.archive_fixtures.v1",
  });
  assert.deepEqual(Object.keys(manifest.archive_fixture).sort(), ["path", "schema"]);
  assert.deepEqual(manifest.generator, {
    crate: "iroha_python_rs",
    test: "kagemusha_recursive_spend_abi7_archive_fixture_matches_python_native_bridge",
    print_env: "KAGEMUSHA_RECURSIVE_SPEND_PRINT_ABI7_ARCHIVES",
  });
  assert.deepEqual(Object.keys(manifest.generator).sort(), ["crate", "print_env", "test"]);
  assert.deepEqual(Object.keys(manifest.domains).sort(), [
    "fixture_label",
    "lineage_accumulator",
  ]);
  assert.equal(
    manifest.domains.lineage_accumulator,
    "iroha:kagemusha:v1:recursive-spend-accumulator",
  );
  assert.equal(manifest.domains.fixture_label, "kagemusha-recursive-spend-python-real");

  const expectedOperations = new Map([
    ["append_bundle", ["append", "KagemushaRecursiveSpendBundleV1", "bundle"]],
    ["verify_request", ["verify", "KagemushaRecursiveSpendVerifyRequestV1", "request"]],
    ["verify_result", ["verify", "KagemushaRecursiveSpendVerifyResultV1", "result"]],
    ["redeem_request", ["redeem", "KagemushaRecursiveSpendRedeemRequestV1", "request"]],
    ["redeem_instruction", ["redeem", "RedeemKagemushaRecursive", "instruction"]],
  ]);
  assert.equal(manifest.operation_count, expectedOperations.size);
  assert.equal(manifest.operations.length, manifest.operation_count);
  assert.deepEqual(
    new Set(manifest.operations.map((operation) => operation.name)),
    new Set(expectedOperations.keys()),
  );
  for (const operation of manifest.operations) {
    const [expectedOperation, expectedType, expectedKind] = expectedOperations.get(
      operation.name,
    );
    assert.deepEqual(Object.keys(operation).sort(), [
      "archive_kind",
      "name",
      "norito_type",
      "operation",
    ]);
    assert.equal(operation.operation, expectedOperation);
    assert.equal(operation.norito_type, expectedType);
    assert.equal(operation.archive_kind, expectedKind);
  }

  const archiveFixture = sharedRecursiveSpendAbi7Archives();
  assert.deepEqual(Object.keys(archiveFixture).sort(), [
    "archives",
    "fixture_kind",
    "native_bridge_abi_version",
    "schema",
  ]);
  assert.equal(archiveFixture.schema, manifest.archive_fixture.schema);
  assert.equal(archiveFixture.fixture_kind, "native_bridge_norito_archives");
  assert.equal(
    archiveFixture.native_bridge_abi_version,
    manifest.native_bridge_abi_version,
  );
  assert.equal(archiveFixture.archives.length, expectedOperations.size);
  assert.deepEqual(
    new Set(archiveFixture.archives.map((archive) => archive.name)),
    new Set(expectedOperations.keys()),
  );
  for (const archive of archiveFixture.archives) {
    assert.deepEqual(Object.keys(archive).sort(), [
      "byte_len",
      "bytes_base64",
      "name",
      "norito_type",
      "operation",
      "sha256_hex",
    ]);
    const [expectedOperation, expectedType] = expectedOperations.get(archive.name);
    assert.equal(archive.operation, expectedOperation);
    assert.equal(archive.norito_type, expectedType);
    const archiveBytes = Buffer.from(archive.bytes_base64, "base64");
    assert.equal(archive.byte_len, archiveBytes.length);
    assert.equal(
      archive.sha256_hex,
      createHash("sha256").update(archiveBytes).digest("hex"),
    );
  }
});

test("Kagemusha recursive spend typed codecs decode ABI-6 and ABI-7 fixtures", () => {
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_VERIFY_RESULT_WIRE_NAME,
    "iroha_data_model::offline::model::KagemushaRecursiveSpendVerifyResultV1",
  );
  const abi6Result = decodeKagemushaRecursiveSpendVerifyResult(
    sharedRecursiveSpendArchive("verify_result"),
  );
  assert.equal(abi6Result.valid, false);
  assert.equal(abi6Result.hopCount, 2);
  assert.equal(abi6Result.hop_count, abi6Result.hopCount);
  assert.equal(abi6Result.encodedBytes > 0, true);
  assert.equal(abi6Result.witnesslessRedeemSupported, false);
  assert.equal(abi6Result.lineageWitnessRequired, true);
  assert.equal(abi6Result.lineageWitnessRequiredForRedeem, abi6Result.lineageWitnessRequired);
  assert.equal(
    abi6Result.lineage_witness_required_for_redeem,
    abi6Result.lineage_witness_required,
  );

  const abi7Result = decodeKagemushaRecursiveSpendVerifyResult(
    sharedRecursiveSpendAbi7Archive("verify_result"),
  );
  assert.equal(abi7Result.valid, true);
  assert.equal(abi7Result.witnesslessRedeemSupported, false);
  assert.equal(abi7Result.lineageWitnessRequired, true);
  assert.equal(abi7Result.lineageWitnessRequiredForRedeem, abi7Result.lineageWitnessRequired);
  assert.equal(
    abi7Result.lineage_witness_required_for_redeem,
    abi7Result.lineage_witness_required,
  );
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendVerifyResult(
        recursiveSpendVerifyResultWithTrailingField(),
      ),
    /verifyResult has trailing bytes/u,
  );

  const initBundle = decodeKagemushaRecursiveSpendBundle(
    sharedRecursiveSpendArchive("init_bundle"),
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN,
    "iroha:kagemusha:v1:recursive-spend-accumulator",
  );
  assert.equal(
    initBundle.proofCircuitId,
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
  );
  assert.equal(initBundle.hopCount, 1);
  assert.equal(initBundle.chainId.length > 0, true);
  assert.equal(initBundle.asset, "686w6ABhTWPaCrWNjjXs7X1SW6w9");
  const fallbackAssetBundle = decodeKagemushaRecursiveSpendBundle(
    recursiveSpendBundleWithAccumulatorField(
      2,
      kagemushaFixedArrayPayload(0x01, 16),
    ),
  );
  assert.equal(
    fallbackAssetBundle.asset,
    "hex:01010101010101010101010101010101",
  );
  assert.equal(initBundle.initialRoot.length, 32);
  assert.equal(initBundle.finalRoot.length, 32);
  assert.equal(initBundle.currentNote.amount, "7");
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
  assert.equal(appendBundle.hopCount, 1);
  assert.equal(appendBundle.asset, "7Y5nGzchCJcxcv98NUoBfwBR1nTk");
  assert.equal(
    appendBundle.proofCircuitId,
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
  );
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(
        recursiveSpendBundleWithProofCircuitId(
          UNSUPPORTED_RECURSIVE_SPEND_PROOF_CIRCUIT_ID,
        ),
    ),
    kagemushaRequestCodecError("archive", "bundle.proof_circuit_id", null),
  );
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
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(recursiveSpendBundleWithEmptyProofBytes()),
    kagemushaRequestCodecError("archive", "bundle.proof_bytes", null),
  );
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
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(recursiveSpendBundleWithTrailingBundleField()),
    kagemushaRequestCodecError("archive", "bundle", /bundle has trailing bytes/),
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

  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(
        recursiveSpendBundleWithAccumulatorField(
          0,
          kagemushaNoritoString(
            "iroha:kagemusha:v1:recursive-spend-accumulator-digest",
          ),
        ),
    ),
    kagemushaRequestCodecError("archive", "bundle.accumulator.domain", null),
  );
  assert.throws(
    () =>
      decodeKagemushaRecursiveSpendBundle(recursiveSpendBundleWithTrailingAccumulatorField()),
    kagemushaRequestCodecError("archive", "bundle", /accumulator has trailing bytes/),
  );
  const malformedAccumulatorFields = [
    [
      0,
      kagemushaNoritoString(" iroha:kagemusha:v1:recursive-spend-accumulator"),
      "bundle.accumulator.domain",
      null,
    ],
    [
      0,
      kagemushaNoritoString("iroha:Kagemusha:v1:recursive-spend-accumulator"),
      "bundle.accumulator.domain",
      null,
    ],
    [
      1,
      kagemushaNoritoString("kagemusha-recursive-spend-abi-chain"),
      "bundle.accumulator.chain_id",
      null,
    ],
    [
      1,
      kagemushaNoritoField(kagemushaNoritoString("")),
      "bundle.accumulator.chain_id",
      /non-empty unpadded string/,
      "field",
    ],
    [
      1,
      kagemushaNoritoField(kagemushaNoritoString(" kagemusha-recursive-spend-abi-chain")),
      "bundle.accumulator.chain_id",
      /non-empty unpadded string/,
      "field",
    ],
    [
      1,
      kagemushaNoritoField(kagemushaNoritoString("kagemusha-recursive-spend-abi-chain ")),
      "bundle.accumulator.chain_id",
      /non-empty unpadded string/,
      "field",
    ],
    [
      1,
      kagemushaNoritoField(kagemushaNoritoString("kagemusha recursive-spend-abi-chain")),
      "bundle.accumulator.chain_id",
      /portable registry syntax/,
      "field",
    ],
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
    [6, kagemushaU32Payload(0), "bundle.accumulator.hop_count", null],
    [
      6,
      kagemushaCountPrefixedFixedArrayPayload(0x06, 4),
      "bundle.accumulator.hop_count",
      null,
    ],
    [
      6,
      kagemushaU32Payload(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 + 1),
      "bundle.accumulator.hop_count",
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
    expectedKind = "archive",
  ] of malformedAccumulatorFields) {
    assert.throws(
      () =>
        decodeKagemushaRecursiveSpendBundle(
          recursiveSpendBundleWithAccumulatorField(fieldIndex, replacement),
        ),
      kagemushaRequestCodecError(expectedKind, expectedField, expectedMessage),
    );
  }
});

test("Kagemusha recursive spend typed encoders write request schemas and compact layouts", () => {
  const recordBundle = syntheticKagemushaRecordBundleArchive();
  const recordBundleWithOverLimitStepCount = syntheticKagemushaRecordBundleArchive(1, {
    stepsPayload: u64LE(KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS + 1),
  });
  const pallasOpenEnvelopes = syntheticPallasOpenEnvelopesArchive();
  const verifierRecord = recursiveSpendVerifierRecord();
  const note = recursiveSpendNote();
  const initArtifactVerifierKey = kagemushaLineageVerifierKey(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    0x91,
  );
  const initArtifactProvingKeyArchive = kagemushaLineageProvingKeyArchive(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    initArtifactVerifierKey,
    0x92,
  );
  const initLineageKeyArtifacts = kagemushaRecursiveSpendLineageKeyArtifactsForInit(
    128,
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
    initArtifactVerifierKey,
    initArtifactProvingKeyArchive,
  );
  const appendArtifactVerifierKey = kagemushaLineageVerifierKey(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    0x93,
  );
  const appendArtifactProvingKeyArchive = kagemushaLineageProvingKeyArchive(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    appendArtifactVerifierKey,
    0x94,
  );
  const appendLineageKeyArtifacts = kagemushaRecursiveSpendLineageKeyArtifactsForAppend(
    64,
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
    appendArtifactVerifierKey,
    appendArtifactProvingKeyArchive,
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendInitRequest({
        recordBundle: recordBundleWithOverLimitStepCount,
        pallasOpenEnvelopes,
        currentNote: note,
        lineageKeyArtifacts: initLineageKeyArtifacts,
      }),
    kagemushaRequestCodecError("archive", "recordBundle.steps", /fold step count is out of range/),
  );

  const initPayload = assertKagemushaArchiveSchema(
    encodeKagemushaRecursiveSpendInitRequest({
      recordBundle,
      pallasOpenEnvelopes,
      currentNote: note,
      lineageVerifierKey: initArtifactVerifierKey,
      lineageProvingKeyArchive: initArtifactProvingKeyArchive,
      blockHeight: 7,
    }),
    KAGEMUSHA_RECURSIVE_SPEND_INIT_REQUEST_WIRE_NAME,
  );
  const initFields = kagemushaReadAllFields(initPayload);
  assert.equal(initFields.length, 6);
  assert.deepEqual(
    initFields[0],
    assertKagemushaArchiveSchema(recordBundle, KAGEMUSHA_RECURSIVE_SPEND_RECORD_BUNDLE_WIRE_NAME),
  );
  assert.equal(initFields[1].readBigUInt64LE(0), BigInt(pallasOpenEnvelopes.length));
  assert.equal(initFields[3][0], 1);
  assert.equal(initFields[4][0], 1);
  assert.equal(initFields[5][0], 1);

  const initWithArtifactsPayload = assertKagemushaArchiveSchema(
    encodeKagemushaRecursiveSpendInitRequest({
      recordBundle,
      pallasOpenEnvelopes,
      currentNote: note,
      lineageKeyArtifacts: initLineageKeyArtifacts,
    }),
    KAGEMUSHA_RECURSIVE_SPEND_INIT_REQUEST_WIRE_NAME,
  );
  const initWithArtifactsFields = kagemushaReadAllFields(initWithArtifactsPayload);
  assert.equal(initWithArtifactsFields.length, 6);
  assert.equal(initWithArtifactsFields[3][0], 1);
  assert.equal(initWithArtifactsFields[4][0], 1);

  const appendPayload = assertKagemushaArchiveSchema(
    encodeKagemushaRecursiveSpendAppendRequest({
      previousBundle: sharedRecursiveSpendArchive("init_bundle"),
      recordBundle,
      pallasOpenEnvelopes,
      currentNote: note,
      previousLineageVerifierRecord: verifierRecord,
      blockHeight: 8,
    }),
    KAGEMUSHA_RECURSIVE_SPEND_APPEND_REQUEST_WIRE_NAME,
  );
  const appendFields = kagemushaReadAllFields(appendPayload);
  assert.equal(appendFields.length, 10);
  assert.deepEqual(
    appendFields[1],
    assertKagemushaArchiveSchema(recordBundle, KAGEMUSHA_RECURSIVE_SPEND_RECORD_BUNDLE_WIRE_NAME),
  );
  assert.deepEqual(appendFields[4], Buffer.from([0]));
  assert.equal(appendFields[5][0], 1);
  assert.equal(appendFields[6].readBigUInt64LE(0), 0n);

  const appendWithArtifactsPayload = assertKagemushaArchiveSchema(
    encodeKagemushaRecursiveSpendAppendRequest({
      previousBundle: sharedRecursiveSpendArchive("init_bundle"),
      recordBundle,
      pallasOpenEnvelopes,
      currentNote: note,
      outputProofCircuitId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
      previousLineageVerifierRecord: verifierRecord,
      previousProofOpenEnvelopes: syntheticPallasOpenEnvelopesArchive(),
      lineageKeyArtifacts: appendLineageKeyArtifacts,
    }),
    KAGEMUSHA_RECURSIVE_SPEND_APPEND_REQUEST_WIRE_NAME,
  );
  const appendWithArtifactsFields = kagemushaReadAllFields(appendWithArtifactsPayload);
  assert.equal(appendWithArtifactsFields.length, 10);
  assert.ok(kagemushaReadOptionSome(appendWithArtifactsFields[7]).length > 0);
  assert.ok(kagemushaReadOptionSome(appendWithArtifactsFields[8]).length > 0);

  const verifyPayload = assertKagemushaArchiveSchema(
    encodeKagemushaRecursiveSpendVerifyRequest({
      bundle: sharedRecursiveSpendArchive("init_bundle"),
      lineageVerifierRecord: verifierRecord,
      blockHeight: 9,
    }),
    KAGEMUSHA_RECURSIVE_SPEND_VERIFY_REQUEST_WIRE_NAME,
  );
  const verifyFields = kagemushaReadAllFields(verifyPayload);
  assert.equal(verifyFields.length, 3);
  assert.equal(verifyFields[1][0], 1);

  const redeemPayload = assertKagemushaArchiveSchema(
    encodeKagemushaRecursiveSpendRedeemRequest({
      bundle: sharedRecursiveSpendArchive("init_bundle"),
      recipient: recursiveSpendRecipient(),
      publicAmount: "6",
      redeemProof: syntheticKagemushaArchive(KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME, 0x64),
      lineageWitness: sharedRecursiveSpendArchive("lineage_witness_append_result"),
      changeOutput: Buffer.from(Array.from({ length: 32 }, (_, index) => 0x80 + index)),
      lineageVerifierRecord: verifierRecord,
      blockHeight: 10,
    }),
    KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_WIRE_NAME,
  );
  const redeemFields = kagemushaReadAllFields(redeemPayload);
  assert.equal(redeemFields.length, 9);
  assert.equal(redeemFields[1].readUInt32LE(0), 0);
  assert.deepEqual(redeemFields[2], Buffer.from([6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]));
  assert.deepEqual(redeemFields[3], Buffer.from([0x64]));
  assert.equal(redeemFields[4][0], 1);
  assert.deepEqual(
    kagemushaReadFixedBytesPayload(kagemushaReadOptionSome(redeemFields[5]), 32),
    Buffer.from(Array.from({ length: 32 }, (_, index) => 0x80 + index)),
  );
  assert.equal(redeemFields[6][0], 1);
  assert.equal(redeemFields[7][0], 1);
  assert.deepEqual(kagemushaReadSequenceFields(redeemFields[8]), []);

  const exactRedeemPayload = assertKagemushaArchiveSchema(
    encodeKagemushaRecursiveSpendRedeemRequest({
      bundle: sharedRecursiveSpendArchive("init_bundle"),
      recipient: recursiveSpendRecipient(),
      publicAmount: "7",
      redeemProof: syntheticKagemushaArchive(KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME, 0x66),
      lineageVerifierRecord: verifierRecord,
    }),
    KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_WIRE_NAME,
  );
  const exactRedeemFields = kagemushaReadAllFields(exactRedeemPayload);
  assert.equal(exactRedeemFields.length, 9);
  kagemushaReadOptionNone(exactRedeemFields[4]);
  kagemushaReadOptionNone(exactRedeemFields[5]);
  assert.equal(exactRedeemFields[6][0], 1);
  kagemushaReadOptionNone(exactRedeemFields[7]);
  assert.deepEqual(kagemushaReadSequenceFields(exactRedeemFields[8]), []);

  const pluralRedeemPayload = assertKagemushaArchiveSchema(
    encodeKagemushaRecursiveSpendRedeemRequest({
      bundle: sharedRecursiveSpendArchive("init_bundle"),
      recipient: recursiveSpendRecipient(),
      publicAmount: "7",
      redeemProof: syntheticKagemushaArchive(KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME, 0x67),
      lineageVerifierRecords: [verifierRecord],
    }),
    KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_WIRE_NAME,
  );
  const pluralRedeemFields = kagemushaReadAllFields(pluralRedeemPayload);
  assert.equal(pluralRedeemFields.length, 9);
  kagemushaReadOptionNone(pluralRedeemFields[6]);
  assert.deepEqual(kagemushaReadSequenceFields(pluralRedeemFields[8]), [
    assertKagemushaArchiveSchema(
      verifierRecord.recordBytes,
      KAGEMUSHA_VERIFYING_KEY_RECORD_WIRE_NAME,
    ),
  ]);
});

test("Kagemusha recursive spend typed codecs reject malformed inputs before native dispatch", () => {
  const invalidPositiveU128Amounts = [
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
  for (const amount of invalidPositiveU128Amounts) {
    assert.throws(
      () => recursiveSpendNote(amount),
      kagemushaRequestCodecError("field", "amount", null),
    );
  }
  assert.throws(
    () => recursiveSpendNote("7", 0),
    kagemushaRequestCodecError("field", "noteCommitment", null),
  );
  assert.throws(
    () => recursiveSpendNote("7", 0x22, 0x22),
    kagemushaRequestCodecError("field", "spendNullifier", null),
  );

  const recordBundle = syntheticKagemushaRecordBundleArchive();
  const pallasOpenEnvelopes = syntheticPallasOpenEnvelopesArchive();
  const note = recursiveSpendNote();
  const verifierRecord = recursiveSpendVerifierRecord();
  const redeemProof = syntheticKagemushaArchive(KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME, 0x77);
  const lineageWitness = sharedRecursiveSpendArchive("lineage_witness_append_result");
  const invalidPublicAmounts = [
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
  const initLineageVerifierKey = kagemushaLineageVerifierKey(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    0x90,
  );
  const initLineageProvingKeyArchive = kagemushaLineageProvingKeyArchive(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    initLineageVerifierKey,
    0x91,
  );
  const initLineageKeyArtifacts = kagemushaRecursiveSpendLineageKeyArtifactsForInit(
    128,
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
    initLineageVerifierKey,
    initLineageProvingKeyArchive,
  );
  const appendLineageVerifierKey = kagemushaLineageVerifierKey(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    0x92,
  );
  const appendLineageKeyArtifacts = kagemushaRecursiveSpendLineageKeyArtifactsForAppend(
    64,
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
    appendLineageVerifierKey,
    kagemushaLineageProvingKeyArchive(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
      appendLineageVerifierKey,
      0x93,
    ),
  );
  const previousProofOpenEnvelopes = syntheticPallasOpenEnvelopesArchive();
  for (const publicAmount of invalidPublicAmounts) {
    assert.throws(
      () =>
        encodeKagemushaRecursiveSpendRedeemRequest({
          bundle: sharedRecursiveSpendArchive("init_bundle"),
          recipient: recursiveSpendRecipient(),
          publicAmount,
          redeemProof,
        }),
      kagemushaRequestCodecError("field", "publicAmount", null),
    );
  }
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendArchive("init_bundle"),
        recipient: recursiveSpendRecipient(),
        public_amount: "0007",
        redeemProof,
      }),
    kagemushaRequestCodecError("field", "publicAmount", /canonical decimal u128/),
  );
  for (const [changeOutput, errorPattern] of [
    [Buffer.alloc(31, 1), /changeOutput must be 32 bytes/],
    [Buffer.alloc(32), /changeOutput must be non-zero/],
  ]) {
    assert.throws(
      () =>
        encodeKagemushaRecursiveSpendRedeemRequest({
          bundle: sharedRecursiveSpendArchive("init_bundle"),
          recipient: recursiveSpendRecipient(),
          publicAmount: "7",
          redeemProof,
          changeOutput,
        }),
      kagemushaRequestCodecError("field", "changeOutput", errorPattern),
    );
  }
  const partialBundle = sharedRecursiveSpendAbi7Archive("append_bundle");
  const partialSummary = decodeKagemushaRecursiveSpendBundle(partialBundle);
  assert.ok(partialSummary.topupAnchorNullifiers.length > 0);
  for (const changeOutput of [
    partialSummary.currentNote.noteCommitment,
    partialSummary.currentNote.spendNullifier,
    partialSummary.topupAnchorNullifiers[0],
  ]) {
    assert.throws(
      () =>
        encodeKagemushaRecursiveSpendRedeemRequest({
          bundle: partialBundle,
          recipient: recursiveSpendRecipient(),
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
        bundle: sharedRecursiveSpendArchive("init_bundle"),
        recipient: recursiveSpendRecipient(),
        publicAmount: "6",
        redeemProof,
      }),
    kagemushaRequestCodecError("field", "changeOutput", /changeOutput is required/),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendArchive("init_bundle"),
        recipient: recursiveSpendRecipient(),
        publicAmount: "8",
        redeemProof,
      }),
    kagemushaRequestCodecError("field", "publicAmount", /publicAmount must not exceed/),
  );
  for (const publicAmount of ["7", "8"]) {
    assert.throws(
      () =>
        encodeKagemushaRecursiveSpendRedeemRequest({
          bundle: sharedRecursiveSpendArchive("init_bundle"),
          recipient: recursiveSpendRecipient(),
          publicAmount,
          redeemProof,
          changeOutput: Buffer.alloc(32, 0x42),
        }),
      kagemushaRequestCodecError("field", "publicAmount", /publicAmount must be less/),
    );
  }
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendAbi7Archive("append_bundle"),
        recipient: recursiveSpendRecipient(),
        publicAmount: "7",
        redeemProof,
      }),
    kagemushaRequestCodecError("field", "lineageWitness", /required for this bundle/),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendAbi7Archive("append_bundle"),
        recipient: recursiveSpendRecipient(),
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
        recipient: recursiveSpendRecipient(),
        publicAmount: "7",
        redeemProof,
        lineageVerifierRecords: [
          {
            verifierKeyId: "danglingRedeemLineageRecords",
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
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendArchive("init_bundle"),
        recipient: recursiveSpendRecipient(),
        publicAmount: "7",
        redeemProof,
      }),
    kagemushaRequestCodecError(
      "field",
      "lineageVerifierRecord",
      /required for reserved-lineage bundles/,
    ),
  );
  const reservedMissingRecordMasksMalformedWitness = Buffer.from([0]);
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendArchive("init_bundle"),
        recipient: recursiveSpendRecipient(),
        publicAmount: "7",
        redeemProof,
        lineageWitness: reservedMissingRecordMasksMalformedWitness,
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
        bundle: sharedRecursiveSpendArchive("init_bundle"),
        recipient: recursiveSpendRecipient(),
        publicAmount: "7",
        redeemProof,
        lineageWitness: Buffer.from([0]),
        lineageVerifierRecord: {
          verifierKeyId: "forgedRedeemLineageRecordBeforeWitness",
          recordBytes: Buffer.from([0]),
        },
      }),
    kagemushaRequestCodecError("archive", "lineageVerifierRecord", /valid Norito archive/),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendArchive("init_bundle"),
        recipient: recursiveSpendRecipient(),
        publicAmount: "7",
        redeemProof: syntheticKagemushaArchive(KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME, 0x7c),
        lineageWitness: syntheticKagemushaArchive(
          KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
          0x7d,
        ),
        lineageVerifierRecord: verifierRecord,
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
          bundle: sharedRecursiveSpendArchive("init_bundle"),
          recipient: recursiveSpendRecipient(),
          publicAmount: "7",
          redeemProof,
          lineageWitness: lineageWitnessArchive,
          lineageVerifierRecord: verifierRecord,
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
          currentNote: note,
          lineageVerifierKey: initLineageVerifierKey,
          lineageProvingKeyArchive: initLineageProvingKeyArchive,
          blockHeight,
        }),
    ],
    [
      "append",
      (blockHeight) =>
        encodeKagemushaRecursiveSpendAppendRequest({
          previousBundle: sharedRecursiveSpendArchive("init_bundle"),
          recordBundle,
          pallasOpenEnvelopes,
          currentNote: note,
          previousLineageVerifierRecord: verifierRecord,
          blockHeight,
        }),
    ],
    [
      "verify",
      (blockHeight) =>
        encodeKagemushaRecursiveSpendVerifyRequest({
          bundle: sharedRecursiveSpendArchive("init_bundle"),
          lineageVerifierRecord: verifierRecord,
          blockHeight,
        }),
    ],
    [
      "redeem",
      (blockHeight) =>
        encodeKagemushaRecursiveSpendRedeemRequest({
          bundle: sharedRecursiveSpendArchive("init_bundle"),
          recipient: recursiveSpendRecipient(),
          publicAmount: "7",
          redeemProof,
          lineageWitness,
          lineageVerifierRecord: verifierRecord,
          blockHeight,
        }),
    ],
  ];
  const invalidBlockHeights = [
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
  assert.equal(Object.is(invalidBlockHeights.at(-1), -0), true);
  for (const [name, encode] of blockHeightEncoders) {
    for (const blockHeight of invalidBlockHeights) {
      assert.throws(
        () => encode(blockHeight),
        kagemushaRequestCodecError("field", "blockHeight", null),
        `${name} accepted non-canonical blockHeight ${JSON.stringify(blockHeight)}`,
      );
    }
  }
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendArchive("init_bundle"),
        recipient: recursiveSpendRecipient(),
        publicAmount: "7",
        redeemProof,
        lineageWitness,
        lineageVerifierRecord: verifierRecord,
        block_height: "0007",
      }),
    kagemushaRequestCodecError("field", "blockHeight", null),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendInitRequest({
        recordBundle,
        pallasOpenEnvelopes,
        currentNote: note,
        lineageProvingKeyArchive: syntheticKagemushaArchive("test::Key", 0x73),
      }),
    kagemushaRequestCodecError("field", "lineageVerifierKey", null),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendInitRequest({
        recordBundle,
        pallasOpenEnvelopes,
        currentNote: note,
        lineageKeyArtifacts: appendLineageKeyArtifacts,
      }),
    kagemushaRequestCodecError("field", "lineageKeyArtifacts", null),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendInitRequest({
        recordBundle,
        pallasOpenEnvelopes,
        currentNote: note,
        lineageKeyArtifacts: initLineageKeyArtifacts,
        lineageVerifierKey: Buffer.from("vk"),
      }),
    kagemushaRequestCodecError(
      "field",
      "lineageKeyArtifacts",
      /must not be combined with raw key fields/,
    ),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendInitRequest({
        recordBundle,
        pallasOpenEnvelopes,
        currentNote: note,
        lineageVerifierKey: initLineageVerifierKey,
        lineageProvingKeyArchive: appendLineageKeyArtifacts.lineageProvingKeyArchive,
      }),
    kagemushaRequestCodecError("field", "lineageKeyArtifacts", /lineageKeyArtifacts:/),
  );
  const malformedInitPallasOpenEnvelopes = [
    [
      Buffer.from([1, 2, 3]),
      "pallasOpenEnvelopes",
      /valid Norito archive/,
    ],
    [
      syntheticKagemushaArchive("test::PallasOpenEnvelopes", 0x72),
      "pallasOpenEnvelopes",
      /valid Vec<iroha_zkp_halo2::OpenVerifyEnvelope>/,
    ],
    [
      syntheticPallasOpenEnvelopesArchive(2),
      "pallasOpenEnvelopes",
      /requires exactly 1 envelope\(s\)/,
    ],
    [
      syntheticPallasOpenEnvelopesArchive(1, { includeDomainTag: false }),
      "pallasOpenEnvelopes[0].domain_tag",
      /is required/,
    ],
  ];
  for (const [malformedArchive, expectedField, expectedMessage] of malformedInitPallasOpenEnvelopes) {
    assert.throws(
      () =>
        encodeKagemushaRecursiveSpendInitRequest({
          recordBundle,
          pallasOpenEnvelopes: malformedArchive,
          currentNote: note,
          lineageVerifierKey: initLineageVerifierKey,
          lineageProvingKeyArchive: initLineageProvingKeyArchive,
        }),
      kagemushaRequestCodecError("archive", expectedField, expectedMessage),
    );
  }
  const malformedPallasTranscriptLabels = ["", "\u00e9".repeat(65)];
  for (const transcriptLabel of malformedPallasTranscriptLabels) {
    assert.throws(
      () =>
        encodeKagemushaRecursiveSpendInitRequest({
          recordBundle,
          pallasOpenEnvelopes: syntheticPallasOpenEnvelopesArchive(1, {
            transcriptLabel,
          }),
          currentNote: note,
          lineageVerifierKey: initLineageVerifierKey,
          lineageProvingKeyArchive: initLineageProvingKeyArchive,
        }),
      kagemushaRequestCodecError("archive", "pallasOpenEnvelopes[0]", /transcript_label is invalid/),
    );
  }
  const malformedPallasMetadataPayloads = [
    {
      metadataField: "vk_commitment",
      options: { vkCommitmentPayload: kagemushaFixedArrayPayload(0x70, 32) },
    },
    {
      metadataField: "vk_commitment",
      options: { vkCommitmentOptionPayload: optionRawWithTrailingByte(syntheticFixed32(0x70)) },
    },
    {
      metadataField: "vk_commitment",
      options: { vkCommitmentOptionPayload: optionRawWithUnknownTag() },
      expectedMessage: /option tag must be 0 or 1/,
    },
    {
      metadataField: "vk_commitment",
      options: { vkCommitmentOptionPayload: optionRawWithDeclaredLengthTooLong(syntheticFixed32(0x70)) },
      expectedMessage: /payload length mismatch/,
    },
    {
      metadataField: "public_inputs_schema_hash",
      options: { publicInputsSchemaHashPayload: kagemushaFixedArrayPayload(0x71, 32) },
    },
    {
      metadataField: "public_inputs_schema_hash",
      options: {
        publicInputsSchemaHashOptionPayload: optionRawWithTrailingByte(syntheticFixed32(0x71)),
      },
    },
    {
      metadataField: "public_inputs_schema_hash",
      options: { publicInputsSchemaHashOptionPayload: optionRawWithUnknownTag() },
      expectedMessage: /option tag must be 0 or 1/,
    },
    {
      metadataField: "public_inputs_schema_hash",
      options: {
        publicInputsSchemaHashOptionPayload: optionRawWithDeclaredLengthTooLong(syntheticFixed32(0x71)),
      },
      expectedMessage: /payload length mismatch/,
    },
    {
      metadataField: "domain_tag",
      options: { domainTagPayload: kagemushaFixedArrayPayload(0x72, 32) },
    },
    {
      metadataField: "domain_tag",
      options: { domainTagOptionPayload: optionRawWithTrailingByte(syntheticFixed32(0x72)) },
    },
    {
      metadataField: "domain_tag",
      options: { domainTagOptionPayload: optionRawWithUnknownTag() },
      expectedMessage: /option tag must be 0 or 1/,
    },
    {
      metadataField: "domain_tag",
      options: { domainTagOptionPayload: optionRawWithDeclaredLengthTooLong(syntheticFixed32(0x72)) },
      expectedMessage: /payload length mismatch/,
    },
  ];
  for (const { metadataField, options, expectedMessage = null } of malformedPallasMetadataPayloads) {
    assert.throws(
      () =>
        encodeKagemushaRecursiveSpendInitRequest({
          recordBundle,
          pallasOpenEnvelopes: syntheticPallasOpenEnvelopesArchive(1, options),
          currentNote: note,
          lineageVerifierKey: initLineageVerifierKey,
          lineageProvingKeyArchive: initLineageProvingKeyArchive,
        }),
      (error) =>
        error?.kind === "archive" &&
        error.field === `pallasOpenEnvelopes[0].${metadataField}` &&
        (expectedMessage == null || expectedMessage.test(error.message)),
      `accepted stale fixed-array Pallas metadata payload for ${metadataField}`,
    );
  }
  const malformedPallasSequencePayloads = [
    {
      options: { paramsGSequencePayload: u64LE(5) },
      field: "pallasOpenEnvelopes[0].params",
      previousField: "previousProofOpenEnvelopes[0].params",
      message: /generator count mismatch/,
    },
    {
      options: { proofLSequencePayload: u64LE(3) },
      field: "pallasOpenEnvelopes[0].proof",
      previousField: "previousProofOpenEnvelopes[0].proof",
      message: /round count mismatch/,
    },
  ];
  for (const { options, field, message } of malformedPallasSequencePayloads) {
    assert.throws(
      () =>
        encodeKagemushaRecursiveSpendInitRequest({
          recordBundle,
          pallasOpenEnvelopes: syntheticPallasOpenEnvelopesArchive(1, options),
          currentNote: note,
          lineageVerifierKey: initLineageVerifierKey,
          lineageProvingKeyArchive: initLineageProvingKeyArchive,
        }),
      kagemushaRequestCodecError("archive", field, message),
      `accepted raw count-prefix-only Pallas sequence payload for ${field}`,
    );
  }
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendInitRequest({
        recordBundle,
        pallasOpenEnvelopes,
        currentNote: note,
        lineageVerifierKey: initLineageVerifierKey,
        lineageProvingKeyArchive: initLineageProvingKeyArchive,
        blockHeight: -1,
      }),
    kagemushaRequestCodecError("field", "blockHeight", null),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendAppendRequest({
        previousBundle: sharedRecursiveSpendArchive("init_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote: note,
      }),
    kagemushaRequestCodecError("field", "previousLineageVerifierRecord", null),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendAppendRequest({
        previousBundle: sharedRecursiveSpendArchive("init_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote: note,
        outputProofCircuitId: "kagemusha-recursive-spend-invalid-output-v1",
        previousLineageVerifierRecord: recursiveSpendVerifierRecord(),
        lineageKeyArtifacts: appendLineageKeyArtifacts,
      }),
    kagemushaRequestCodecError("field", "outputProofCircuitId", null),
  );
  const previousOpeningsWithoutLineageRecord = syntheticPallasOpenEnvelopesArchive();
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendAppendRequest({
        previousBundle: sharedRecursiveSpendArchive("init_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote: note,
        outputProofCircuitId:
          KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        previousProofOpenEnvelopes: previousOpeningsWithoutLineageRecord,
      }),
    kagemushaRequestCodecError("field", "previousLineageVerifierRecord", null),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendAppendRequest({
        previousBundle: sharedRecursiveSpendAbi7Archive("append_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote: note,
        previousLineageVerifierRecord: recursiveSpendVerifierRecord(),
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
        currentNote: note,
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
        previousBundle: sharedRecursiveSpendArchive("init_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote: note,
        previousLineageVerifierRecord: recursiveSpendVerifierRecord(),
        previousProofOpenEnvelopes,
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
        previousBundle: sharedRecursiveSpendArchive("init_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote: note,
        previousLineageVerifierRecord: {
          verifierKeyId: "malformedPreviousLineageRecordBeforeOpenings",
          recordBytes: Buffer.from([0]),
        },
        previousProofOpenEnvelopes,
      }),
    kagemushaRequestCodecError(
      "archive",
      "previousLineageVerifierRecord",
      /valid Norito archive/,
    ),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendAppendRequest({
        previousBundle: sharedRecursiveSpendArchive("init_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote: note,
        outputProofCircuitId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        previousLineageVerifierRecord: recursiveSpendVerifierRecord(),
        lineageVerifierKey: Buffer.from("vk"),
        lineageProvingKeyArchive: syntheticKagemushaArchive("test::Key", 0x76),
      }),
    kagemushaRequestCodecError("field", "previousProofOpenEnvelopes", null),
  );
  for (const transcriptLabel of malformedPallasTranscriptLabels) {
    assert.throws(
      () =>
        encodeKagemushaRecursiveSpendAppendRequest({
          previousBundle: sharedRecursiveSpendArchive("init_bundle"),
          recordBundle,
          pallasOpenEnvelopes,
          currentNote: note,
          outputProofCircuitId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
          previousLineageVerifierRecord: recursiveSpendVerifierRecord(),
          previousProofOpenEnvelopes: syntheticPallasOpenEnvelopesArchive(1, {
            transcriptLabel,
          }),
          lineageKeyArtifacts: appendLineageKeyArtifacts,
        }),
      kagemushaRequestCodecError(
        "archive",
        "previousProofOpenEnvelopes[0]",
        /transcript_label is invalid/,
      ),
    );
  }
  for (const { metadataField, options, expectedMessage = null } of malformedPallasMetadataPayloads) {
    assert.throws(
      () =>
        encodeKagemushaRecursiveSpendAppendRequest({
          previousBundle: sharedRecursiveSpendArchive("init_bundle"),
          recordBundle,
          pallasOpenEnvelopes,
          currentNote: note,
          outputProofCircuitId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
          previousLineageVerifierRecord: recursiveSpendVerifierRecord(),
          previousProofOpenEnvelopes: syntheticPallasOpenEnvelopesArchive(1, options),
          lineageKeyArtifacts: appendLineageKeyArtifacts,
        }),
      (error) =>
        error?.kind === "archive" &&
        error.field === `previousProofOpenEnvelopes[0].${metadataField}` &&
        (expectedMessage == null || expectedMessage.test(error.message)),
      `accepted stale fixed-array previous-proof Pallas metadata payload for ${metadataField}`,
    );
  }
  for (const { options, previousField, message } of malformedPallasSequencePayloads) {
    assert.throws(
      () =>
        encodeKagemushaRecursiveSpendAppendRequest({
          previousBundle: sharedRecursiveSpendArchive("init_bundle"),
          recordBundle,
          pallasOpenEnvelopes,
          currentNote: note,
          outputProofCircuitId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
          previousLineageVerifierRecord: recursiveSpendVerifierRecord(),
          previousProofOpenEnvelopes: syntheticPallasOpenEnvelopesArchive(1, options),
          lineageKeyArtifacts: appendLineageKeyArtifacts,
        }),
      kagemushaRequestCodecError("archive", previousField, message),
      `accepted raw count-prefix-only previous-proof Pallas sequence payload for ${previousField}`,
    );
  }
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendAppendRequest({
        previousBundle: sharedRecursiveSpendArchive("init_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote: note,
        outputProofCircuitId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        previousLineageVerifierRecord: recursiveSpendVerifierRecord(),
        previousProofOpenEnvelopes: syntheticPallasOpenEnvelopesArchive(2),
        lineageKeyArtifacts: appendLineageKeyArtifacts,
      }),
    kagemushaRequestCodecError(
      "archive",
      "previousProofOpenEnvelopes",
      /requires exactly 1 envelope\(s\)/,
    ),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendAppendRequest({
        previousBundle: sharedRecursiveSpendArchive("init_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote: note,
        outputProofCircuitId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        previousLineageVerifierRecord: recursiveSpendVerifierRecord(),
        previousProofOpenEnvelopes,
        lineageKeyArtifacts: initLineageKeyArtifacts,
      }),
    kagemushaRequestCodecError("field", "lineageKeyArtifacts", null),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendAppendRequest({
        previousBundle: sharedRecursiveSpendArchive("init_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote: note,
        outputProofCircuitId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        previousLineageVerifierRecord: recursiveSpendVerifierRecord(),
        previousProofOpenEnvelopes,
        lineageKeyArtifacts: appendLineageKeyArtifacts,
        lineageVerifierKey: Buffer.from("vk"),
      }),
    kagemushaRequestCodecError(
      "field",
      "lineageKeyArtifacts",
      /must not be combined with raw key fields/,
    ),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendAppendRequest({
        previousBundle: sharedRecursiveSpendArchive("init_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote: note,
        outputProofCircuitId: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        previousLineageVerifierRecord: recursiveSpendVerifierRecord(),
        previousProofOpenEnvelopes,
        lineageVerifierKey: appendLineageVerifierKey,
        lineageProvingKeyArchive: initLineageProvingKeyArchive,
      }),
    kagemushaRequestCodecError("field", "lineageKeyArtifacts", /lineageKeyArtifacts:/),
  );
  for (const lineageKeyFields of [
    { lineageKeyArtifacts: appendLineageKeyArtifacts },
    { lineageVerifierKey: Buffer.from("vk") },
    { lineageProvingKeyArchive: syntheticKagemushaArchive("test::Key", 0x76) },
    {
      lineageVerifierKey: Buffer.from("vk"),
      lineageProvingKeyArchive: syntheticKagemushaArchive("test::Key", 0x76),
    },
  ]) {
    assert.throws(
      () =>
        encodeKagemushaRecursiveSpendAppendRequest({
          previousBundle: sharedRecursiveSpendArchive("init_bundle"),
          recordBundle,
          pallasOpenEnvelopes,
          currentNote: note,
          previousLineageVerifierRecord: verifierRecord,
          ...lineageKeyFields,
        }),
      kagemushaRequestCodecError(
        "field",
        "lineageKeyArtifacts",
        /only valid for lineage append output/,
      ),
    );
  }
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendVerifyRequest({
        bundle: syntheticKagemushaArchive(
          KAGEMUSHA_RECURSIVE_SPEND_RECORD_BUNDLE_WIRE_NAME,
          0x77,
        ),
      }),
    kagemushaRequestCodecError("archive", "bundle", /must use/),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendVerifyRequest({
        bundle: sharedRecursiveSpendArchive("init_bundle"),
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
        lineageVerifierRecord: verifierRecord,
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
        recipient: recursiveSpendRecipient(),
        publicAmount: "7",
        redeemProof: syntheticKagemushaArchive(KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME, 0x78),
        lineageVerifierRecord: verifierRecord,
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
        recipient: recursiveSpendRecipient(),
        publicAmount: "7",
        redeemProof: syntheticKagemushaArchive(KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME, 0x78),
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
        recipient: recursiveSpendRecipient(),
        publicAmount: "7",
        redeemProof: syntheticKagemushaArchive(KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME, 0x79),
        lineageWitness: sharedRecursiveSpendArchive("lineage_witness_append_result"),
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
        recipient: recursiveSpendRecipient(),
        publicAmount: "7",
        redeemProof: syntheticKagemushaArchive(KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME, 0x7a),
        lineageWitness: sharedRecursiveSpendArchive("lineage_witness_from_init_result"),
        lineageVerifierRecord: verifierRecord,
      }),
    kagemushaRequestCodecError(
      "field",
      "lineageVerifierRecord",
      /only valid for reserved-lineage bundles or lineage witnesses/,
    ),
  );
  assert.doesNotThrow(() =>
    encodeKagemushaRecursiveSpendRedeemRequest({
      bundle: sharedRecursiveSpendAbi7Archive("append_bundle"),
      recipient: recursiveSpendRecipient(),
      publicAmount: "7",
      redeemProof: syntheticKagemushaArchive(KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME, 0x7b),
      lineageWitness: sharedRecursiveSpendArchive("lineage_witness_append_result"),
      lineageVerifierRecord: verifierRecord,
    }),
  );
  assert.throws(
    () =>
      encodeKagemushaRecursiveSpendRedeemRequest({
        bundle: sharedRecursiveSpendArchive("init_bundle"),
        recipient: "alice@wonderland",
        publicAmount: "7",
        redeemProof: syntheticKagemushaArchive(KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME, 0x78),
        lineageVerifierRecord: verifierRecord,
      }),
    kagemushaRequestCodecError("field", "recipient", /canonical I105 account form/),
  );
  const tampered = Buffer.from(sharedRecursiveSpendArchive("init_bundle"));
  tampered[6] ^= 0x7f;
  assert.throws(
    () => decodeKagemushaRecursiveSpendBundle(tampered),
    kagemushaRequestCodecError("archive", "bundle", /must use/),
  );
});

test("Kagemusha recursive spend typed helpers delegate encoded requests", () => {
  const calls = [];
  const binding = completeRecursiveSpendBinding({
    kagemushaRecursiveSpendInit(request) {
      rejectMalformedProbe("init", request);
      calls.push(["init", Buffer.from(request)]);
      return kagemushaNoritoFrameWithPayload(0x31);
    },
    kagemushaRecursiveSpendAppend(request) {
      rejectMalformedProbe("append", request);
      calls.push(["append", Buffer.from(request)]);
      return kagemushaNoritoFrameWithPayload(0x32);
    },
    kagemushaRecursiveSpendVerify(request) {
      rejectMalformedProbe("verify", request);
      calls.push(["verify", Buffer.from(request)]);
      return sharedRecursiveSpendArchive("verify_result");
    },
    kagemushaRecursiveSpendRedeem(request) {
      rejectMalformedProbe("redeem", request);
      calls.push(["redeem", Buffer.from(request)]);
      return kagemushaNoritoFrameWithPayload(0x36);
    },
  });
  const recordBundle = syntheticKagemushaRecordBundleArchive();
  const pallasOpenEnvelopes = syntheticPallasOpenEnvelopesArchive();
  const verifierRecord = recursiveSpendVerifierRecord();
  const note = recursiveSpendNote();
  const initLineageVerifierKey = kagemushaLineageVerifierKey(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    0x83,
  );
  const initLineageProvingKeyArchive = kagemushaLineageProvingKeyArchive(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    initLineageVerifierKey,
    0x84,
  );

  withNativeBinding(binding, () => {
    assert.ok(
      kagemushaRecursiveSpendInitTyped({
        recordBundle,
        pallasOpenEnvelopes,
        currentNote: note,
        lineageVerifierKey: initLineageVerifierKey,
        lineageProvingKeyArchive: initLineageProvingKeyArchive,
      }).subarray(0, 4).equals(Buffer.from("NRT0", "ascii")),
    );
    assert.equal(calls.at(-1)[0], "init");
    assertKagemushaArchiveSchema(
      calls.at(-1)[1],
      KAGEMUSHA_RECURSIVE_SPEND_INIT_REQUEST_WIRE_NAME,
    );

    assert.ok(
      kagemushaRecursiveSpendAppendTyped({
        previousBundle: sharedRecursiveSpendArchive("init_bundle"),
        recordBundle,
        pallasOpenEnvelopes,
        currentNote: note,
        previousLineageVerifierRecord: verifierRecord,
      }).subarray(0, 4).equals(Buffer.from("NRT0", "ascii")),
    );
    assert.equal(calls.at(-1)[0], "append");
    assertKagemushaArchiveSchema(
      calls.at(-1)[1],
      KAGEMUSHA_RECURSIVE_SPEND_APPEND_REQUEST_WIRE_NAME,
    );

    const verifyResult = kagemushaRecursiveSpendVerifyTyped({
      bundle: sharedRecursiveSpendArchive("init_bundle"),
      lineageVerifierRecord: verifierRecord,
    });
    assert.equal(verifyResult.hopCount, 2);
    assert.equal(calls.at(-1)[0], "verify");
    assertKagemushaArchiveSchema(
      calls.at(-1)[1],
      KAGEMUSHA_RECURSIVE_SPEND_VERIFY_REQUEST_WIRE_NAME,
    );

    assert.ok(
      kagemushaRecursiveSpendRedeemTyped({
        bundle: sharedRecursiveSpendArchive("init_bundle"),
        recipient: recursiveSpendRecipient(),
        publicAmount: "7",
        redeemProof: syntheticKagemushaArchive(KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME, 0x84),
        lineageWitness: sharedRecursiveSpendArchive("lineage_witness_append_result"),
        lineageVerifierRecord: verifierRecord,
      }).subarray(0, 4).equals(Buffer.from("NRT0", "ascii")),
    );
    assert.equal(calls.at(-1)[0], "redeem");
    assertKagemushaArchiveSchema(
      calls.at(-1)[1],
      KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_WIRE_NAME,
    );
  });
});

test("confidential v2 derivation helpers reject padded chain and asset IDs before native dispatch", () => {
  const calls = [];
  const commitment = Buffer.alloc(32, 0x6d);
  const nullifier = Buffer.alloc(32, 0x6e);
  const rho = Buffer.alloc(32, 0x51);
  const ownerTag = Buffer.alloc(32, 0x52);
  const spendKey = Buffer.alloc(32, 0x53);
  const binding = {
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

  withNativeBinding(binding, () => {
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
  });

  assert.deepEqual(calls, []);
});

test("Kagemusha offline spend mode prefers recursive compact when ABI-7 support is complete", () => {
  const completeBinding = completeRecursiveSpendBinding();

  assert.equal(
    KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1,
    "recursive_compact_v1",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION,
    7,
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1,
    "kagemusha-recursive-compact-v1",
  );
  assert.equal(
    isKagemushaRecursiveCompactUnavailable(
      new Error(KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT),
    ),
    true,
  );
  assert.equal(
    isKagemushaRecursiveCompactUnavailable(
      `bridge: ${KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT}`,
    ),
    true,
  );
  assert.equal(
    isKagemushaRecursiveCompactUnavailable(
      new Error("recursive compact proof composition unavailable"),
    ),
    false,
  );
  assert.equal(isKagemushaRecursiveCompactUnavailable(null), false);
  assert.equal(
    preferredKagemushaOfflineSpendModeForCapabilities(true, true),
    KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1,
  );
  assert.equal(
    preferredKagemushaOfflineSpendModeForCapabilities(true, false),
    KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1,
  );
  assert.equal(
    preferredKagemushaOfflineSpendModeForCapabilities(false, true),
    KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1,
  );
  assert.equal(
    preferredKagemushaOfflineSpendModeForCapabilities(false, false),
    KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
  );
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
    assert.equal(isKagemushaRecursiveCompactPaymentTokenNativeAvailable(), false);
    assert.equal(
      isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable(),
      false,
    );
    assert.throws(
      () =>
        kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
          kagemushaInputArchive(0xc1),
          kagemushaInputArchive(0xc2),
          RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        ),
      /recursive compact Kagemusha payment-token prover requires native bridge ABI 7/,
    );
  });
  withNativeBinding(
    {
      ...completeBinding,
      kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes() {
        return Uint8Array.from([99]);
      },
    },
    () => {
      assert.equal(isKagemushaRecursiveCompactPaymentTokenNativeAvailable(), false);
      assert.equal(
        isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable(),
        false,
      );
      assert.equal(
        preferredKagemushaOfflineSpendMode(),
        KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1,
      );
      assert.throws(
        () =>
          kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
            kagemushaInputArchive(0xc3),
            kagemushaInputArchive(0xc4),
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
          ),
        /recursive compact Kagemusha payment-token prover requires native bridge ABI 7/,
      );
    },
  );
  withNativeBinding(
    {
      ...completeBinding,
      kagemushaVerifyRecursiveCompactPaymentToken(token, verifierKeys) {
        rejectMalformedProbe("recursive-compact-verify", token, verifierKeys);
        return Buffer.from(token)[6] === 0x4b;
      },
    },
    () => {
      assert.equal(isKagemushaRecursiveCompactPaymentTokenNativeAvailable(), false);
      assert.equal(isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable(), true);
      assert.throws(
        () =>
          kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
            kagemushaInputArchive(0xca),
            kagemushaInputArchive(0xcb),
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
          ),
        /recursive compact Kagemusha payment-token prover requires native bridge ABI 7/,
      );
      assert.equal(
        kagemushaVerifyRecursiveCompactPaymentToken(
          kagemushaNoritoFrameWithPayload(0x4b),
          RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        ),
        true,
      );
      assert.equal(
        kagemushaVerifyRecursiveCompactPaymentToken(
          kagemushaNoritoFrameWithPayload(0x4c),
          RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        ),
        false,
      );
    },
  );
  withNativeBinding(
    {
      ...completeBinding,
      kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes() {
        throw new Error("Kagemusha recursive compact proof unavailable");
      },
      kagemushaVerifyRecursiveCompactPaymentToken(token, verifierKeys) {
        rejectMalformedProbe("recursive-compact-verify", token, verifierKeys);
        return true;
      },
    },
    () => {
      assert.equal(isKagemushaRecursiveCompactPaymentTokenNativeAvailable(), false);
      assert.equal(
        isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable(),
        true,
      );
      assert.throws(
        () =>
          kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
            kagemushaInputArchive(0xcc),
            kagemushaInputArchive(0xcd),
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
          ),
        /recursive compact Kagemusha payment-token prover requires native bridge ABI 7/,
      );
    },
  );
  withNativeBinding(
    {
      ...completeBinding,
      kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        record,
        pallasOpenEnvelopes,
        keyArtifacts,
      ) {
        rejectMalformedProbe("recursive-compact", record, pallasOpenEnvelopes, keyArtifacts);
        return kagemushaNoritoFrameWithPayload(0x4e);
      },
      kagemushaVerifyRecursiveCompactPaymentToken() {
        throw new Error("Kagemusha recursive compact verifier unavailable");
      },
    },
    () => {
      assert.equal(isKagemushaRecursiveCompactPaymentTokenNativeAvailable(), false);
      assert.equal(
        isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable(),
        false,
      );
      assert.throws(
        () =>
          kagemushaVerifyRecursiveCompactPaymentToken(
            kagemushaNoritoFrameWithPayload(0x4b),
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
          ),
        /recursive compact Kagemusha payment-token verifier requires native bridge ABI 7 with the compact verifier symbol/,
      );
    },
  );
  withNativeBinding(
    {
      ...completeBinding,
      kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        record,
        pallasOpenEnvelopes,
        keyArtifacts,
      ) {
        rejectMalformedProbe("recursive-compact", record, pallasOpenEnvelopes, keyArtifacts);
        throw new Error("recursive compact proof composition unavailable");
      },
      kagemushaVerifyRecursiveCompactPaymentToken(token, verifierKeys) {
        rejectMalformedProbe("recursive-compact-verify", token, verifierKeys);
        return true;
      },
    },
    () => {
      assert.equal(isKagemushaRecursiveCompactPaymentTokenNativeAvailable(), true);
      assert.equal(
        isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable(),
        true,
      );
      assert.throws(
        () =>
          kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
            kagemushaInputArchive(0xc7),
            kagemushaInputArchive(0xc8),
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
          ),
        /recursive compact proof composition unavailable/,
      );
    },
  );
  withNativeBinding(
    {
      ...completeBinding,
      kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        record,
        pallasOpenEnvelopes,
        keyArtifacts,
      ) {
        rejectMalformedProbe("recursive-compact", record, pallasOpenEnvelopes, keyArtifacts);
        return kagemushaNoritoFrameWithPayload(0x4a);
      },
      kagemushaVerifyRecursiveCompactPaymentToken(token, verifierKeys) {
        rejectMalformedProbe("recursive-compact-verify", token, verifierKeys);
        return Buffer.from(token)[6] === 0x4b;
      },
    },
    () => {
      assert.equal(isKagemushaRecursiveCompactPaymentTokenNativeAvailable(), true);
      assert.equal(
        isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable(),
        true,
      );
      assert.equal(
        preferredKagemushaOfflineSpendMode(),
        KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1,
      );
      assert.deepEqual(
        kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
          kagemushaInputArchive(0xc5),
          kagemushaInputArchive(0xc6),
          RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        ),
        kagemushaNoritoFrameWithPayload(0x4a),
      );
      assert.equal(
        kagemushaVerifyRecursiveCompactPaymentToken(
          kagemushaNoritoFrameWithPayload(0x4b),
          RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        ),
        true,
      );
      assert.equal(
        kagemushaVerifyRecursiveCompactPaymentToken(
          kagemushaNoritoFrameWithPayload(0x4c),
          RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        ),
        false,
      );
    },
  );
  withNativeBinding(
    {
      ...completeBinding,
      kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        record,
        pallasOpenEnvelopes,
        keyArtifacts,
      ) {
        rejectMalformedProbe("recursive-compact", record, pallasOpenEnvelopes, keyArtifacts);
        return Uint8Array.from([10]);
      },
      kagemushaVerifyRecursiveCompactPaymentToken() {
        return true;
      },
    },
    () => {
      assert.equal(isKagemushaRecursiveCompactPaymentTokenNativeAvailable(), false);
      assert.equal(
        isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable(),
        false,
      );
      assert.throws(
        () =>
          kagemushaVerifyRecursiveCompactPaymentToken(
            kagemushaNoritoFrameWithPayload(0x4b),
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
          ),
        /recursive compact Kagemusha payment-token verifier requires native bridge ABI 7 with the compact verifier symbol/,
      );
    },
  );
  withNativeBinding(
    {
      ...completeBinding,
      kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        record,
        pallasOpenEnvelopes,
        keyArtifacts,
      ) {
        rejectMalformedProbe("recursive-compact", record, pallasOpenEnvelopes, keyArtifacts);
        return kagemushaNoritoFrameWithPayload(0x4d);
      },
      kagemushaVerifyRecursiveCompactPaymentToken(token, verifierKeys) {
        rejectMalformedProbe("recursive-compact-verify", token, verifierKeys);
        return Uint8Array.from([1]);
      },
    },
    () => {
      assert.equal(isKagemushaRecursiveCompactPaymentTokenNativeAvailable(), true);
      assert.equal(
        isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable(),
        true,
      );
      assert.throws(
        () =>
          kagemushaVerifyRecursiveCompactPaymentToken(
            kagemushaNoritoFrameWithPayload(0x4b),
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
          ),
        /kagemushaVerifyRecursiveCompactPaymentToken returned a non-boolean result/,
      );
    },
  );
  withNativeBinding({ kagemushaRecursiveSpendInit() {} }, () => {
    assert.equal(
      preferredKagemushaOfflineSpendMode(),
      KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
    );
  });
});

test("Kagemusha recursive spend compact projection probes availability and validates native output", () => {
  const abi7Binding = {
    connectNoritoBridgeAbiVersion() {
      return KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION;
    },
  };
  const bundleArchive = kagemushaInputArchive(0xe1);

  withNativeBinding(abi7Binding, () => {
    assert.equal(
      isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable(),
      false,
    );
    assert.throws(
      () => kagemushaRecursiveSpendCompactPaymentTokenFromBundle(bundleArchive),
      /compact projection symbol/,
    );
  });

  withNativeBinding(
    {
      ...abi7Binding,
      kagemushaRecursiveSpendCompactPaymentTokenFromBundle(bundle) {
        rejectMalformedProbe("recursive-spend-compact-projection", bundle);
        return kagemushaNoritoFrameWithPayload(0x4f);
      },
    },
    () => {
      assert.equal(
        isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable(),
        true,
      );
      assert.deepEqual(
        kagemushaRecursiveSpendCompactPaymentTokenFromBundle(bundleArchive),
        kagemushaNoritoFrameWithPayload(0x4f),
      );
    },
  );

  withNativeBinding(
    {
      ...abi7Binding,
      kagemushaRecursiveSpendCompactPaymentTokenFromBundle(bundle) {
        rejectMalformedProbe("recursive-spend-compact-projection", bundle);
        return Uint8Array.from([1]);
      },
    },
    () => {
      assert.equal(
        isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable(),
        true,
      );
      assert.throws(
        () => kagemushaRecursiveSpendCompactPaymentTokenFromBundle(bundleArchive),
        /returned invalid Norito archive/,
      );
    },
  );
});

test("Kagemusha recursive spend compact projection verifier probes and delegates", () => {
  const compactTokenArchive = kagemushaInputArchive(0xe2);
  const verifierRecordArchive = kagemushaInputArchive(0xe3);
  const baseBinding = {
    connectNoritoBridgeAbiVersion() {
      return KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION;
    },
  };

  withNativeBinding(baseBinding, () => {
    assert.equal(
      isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable(),
      false,
    );
    assert.throws(
      () =>
        kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
          compactTokenArchive,
          verifierRecordArchive,
        ),
      /compact projection verifier symbols/,
    );
  });

  const calls = [];
  withNativeBinding(
    {
      ...baseBinding,
      kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(token, record) {
        rejectMalformedProbe("recursive-spend-compact-projection-verify", token, record);
        calls.push(["verify", Buffer.from(token), Buffer.from(record)]);
        return false;
      },
      kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
        token,
        record,
        blockHeight,
      ) {
        rejectMalformedProbe("recursive-spend-compact-projection-verify-height", token, record);
        calls.push(["verify-at-height", Buffer.from(token), Buffer.from(record), blockHeight]);
        return true;
      },
    },
    () => {
      assert.equal(
        isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable(),
        true,
      );
      assert.equal(
        kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
          compactTokenArchive,
          verifierRecordArchive,
        ),
        false,
      );
      assert.deepEqual(calls.at(-1), [
        "verify",
        compactTokenArchive,
        verifierRecordArchive,
      ]);
      assert.equal(
        kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
          compactTokenArchive,
          verifierRecordArchive,
          2,
        ),
        true,
      );
      assert.deepEqual(calls.at(-1), [
        "verify-at-height",
        compactTokenArchive,
        verifierRecordArchive,
        2,
      ]);
      assert.equal(
        kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
          compactTokenArchive,
          verifierRecordArchive,
          2n,
        ),
        true,
      );
      assert.deepEqual(calls.at(-1), [
        "verify-at-height",
        compactTokenArchive,
        verifierRecordArchive,
        2n,
      ]);
      assert.equal(
        kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
          compactTokenArchive,
          verifierRecordArchive,
          0xffff_ffff_ffff_ffffn,
        ),
        true,
      );
      assert.deepEqual(calls.at(-1), [
        "verify-at-height",
        compactTokenArchive,
        verifierRecordArchive,
        0xffff_ffff_ffff_ffffn,
      ]);
      const callsBeforeInvalidHeights = calls.length;
      const invalidBlockHeights = [
        [true, /blockHeight must be a number or bigint/],
        [false, /blockHeight must be a number or bigint/],
        ["1", /blockHeight must be a number or bigint/],
        [{ value: 1 }, /blockHeight must be a number or bigint/],
        [1.5, /blockHeight must be an integer/],
        [NaN, /blockHeight must be an integer/],
        [Infinity, /blockHeight must be an integer/],
        [-1, /blockHeight must be non-negative/],
        [-0, /blockHeight must be non-negative/],
        [-1n, /blockHeight must be non-negative/],
        [
          Number.MAX_SAFE_INTEGER + 1,
          /blockHeight number must be a safe integer; use bigint for larger u64 values/,
        ],
        [0x1_0000_0000_0000_0000n, /blockHeight must fit in u64/],
      ];
      for (const [badHeight, errorPattern] of invalidBlockHeights) {
        assert.throws(
          () =>
            kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
              compactTokenArchive,
              verifierRecordArchive,
              badHeight,
            ),
          errorPattern,
        );
        assert.equal(calls.length, callsBeforeInvalidHeights);
      }
    },
  );

  withNativeBinding(
    {
      ...baseBinding,
      kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(token, record) {
        rejectMalformedProbe("recursive-spend-compact-projection-verify", token, record);
        return Uint8Array.from([1]);
      },
      kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(token, record) {
        rejectMalformedProbe("recursive-spend-compact-projection-verify-height", token, record);
        return false;
      },
    },
    () => {
      assert.equal(
        isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable(),
        true,
      );
      assert.throws(
        () =>
          kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
            compactTokenArchive,
            verifierRecordArchive,
          ),
        /kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection returned a non-boolean result/,
      );
    },
  );

  withNativeBinding(
    {
      ...baseBinding,
      kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(token, record) {
        calls.push(["invalid-verify", Buffer.from(token), Buffer.from(record)]);
        throw new Error("native compact projection verifier should not run");
      },
      kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(token, record) {
        calls.push(["invalid-verify-height", Buffer.from(token), Buffer.from(record)]);
        throw new Error("native compact projection verifier should not run");
      },
    },
    () => {
      const invalidArchives = [
        [Buffer.alloc(0), "must not be empty"],
        [
          new Uint8Array(KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES + 1),
          "must not exceed",
        ],
        [Buffer.from([1]), "must be a valid Norito archive"],
        [kagemushaNoritoFrame(0xe4), "must contain a non-empty Norito payload"],
        [undefined, "must be a Buffer, string, or ArrayBuffer view"],
      ];
      const callsBeforeInvalidArchives = calls.length;
      for (const [invalidArchive, expectedMessage] of invalidArchives) {
        assert.throws(
          () =>
            kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
              invalidArchive,
              verifierRecordArchive,
            ),
          new RegExp(`compactTokenArchive ${expectedMessage}`),
        );
        assert.throws(
          () =>
            kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
              invalidArchive,
              verifierRecordArchive,
              9,
            ),
          new RegExp(`compactTokenArchive ${expectedMessage}`),
        );
        assert.throws(
          () =>
            kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
              compactTokenArchive,
              invalidArchive,
              9,
            ),
          new RegExp(`verifierRecordArchive ${expectedMessage}`),
        );
        assert.throws(
          () =>
            kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
              compactTokenArchive,
              invalidArchive,
            ),
          new RegExp(`verifierRecordArchive ${expectedMessage}`),
        );
        assert.equal(calls.length, callsBeforeInvalidArchives);
      }
      assert.throws(
        () =>
          kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
            compactTokenArchive,
            Buffer.from([1]),
          ),
        /verifierRecordArchive must be a valid Norito archive/,
      );
      assert.equal(calls.length, callsBeforeInvalidArchives);
    },
  );
});

test("Kagemusha record-backed JS builders probe availability and validate native output", () => {
  const recordBundle = kagemushaInputArchive(0xda);
  const pallasOpenEnvelopes = kagemushaInputArchive(0xdb);
  const binding = {
    connectNoritoBridgeAbiVersion() {
      return 6;
    },
    kagemushaProveVerifiedCompactPaymentTokenWithRecords(record) {
      rejectMalformedProbe("compact-token", record);
      return kagemushaNoritoFrameWithPayload(0xdc);
    },
    kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
      record,
      pallas,
    ) {
      rejectMalformedProbe("recursive-aggregation", record, pallas);
      return kagemushaNoritoFrameWithPayload(0xdd);
    },
  };

  withNativeBinding(binding, () => {
    assert.equal(isKagemushaCompactPaymentTokenNativeAvailable(), true);
    assert.equal(isKagemushaRecursiveAggregationProofBundleNativeAvailable(), true);
    assert.deepEqual(
      kagemushaProveVerifiedCompactPaymentTokenWithRecords(recordBundle),
      kagemushaNoritoFrameWithPayload(0xdc),
    );
    assert.deepEqual(
      kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
        recordBundle,
        pallasOpenEnvelopes,
      ),
      kagemushaNoritoFrameWithPayload(0xdd),
    );
  });

  withNativeBinding(
    {
      ...binding,
      connectNoritoBridgeAbiVersion() {
        return 5;
      },
    },
    () => {
      assert.equal(isKagemushaCompactPaymentTokenNativeAvailable(), false);
      assert.equal(isKagemushaRecursiveAggregationProofBundleNativeAvailable(), false);
      assert.throws(
        () => kagemushaProveVerifiedCompactPaymentTokenWithRecords(recordBundle),
        /Kagemusha compact payment-token prover requires native bridge ABI 6/,
      );
      assert.throws(
        () =>
          kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
            recordBundle,
            pallasOpenEnvelopes,
          ),
        /Kagemusha recursive aggregation proof-bundle prover requires native bridge ABI 6/,
      );
    },
  );

  withNativeBinding(
    {
      ...binding,
      kagemushaProveVerifiedCompactPaymentTokenWithRecords(record) {
        rejectMalformedProbe("compact-token", record);
        return Buffer.from([1]);
      },
      kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
        record,
        pallas,
      ) {
        rejectMalformedProbe("recursive-aggregation", record, pallas);
        return kagemushaNoritoFrame(0xde);
      },
    },
    () => {
      assert.equal(isKagemushaCompactPaymentTokenNativeAvailable(), true);
      assert.equal(isKagemushaRecursiveAggregationProofBundleNativeAvailable(), true);
      assert.throws(
        () => kagemushaProveVerifiedCompactPaymentTokenWithRecords(recordBundle),
        /native kagemushaProveVerifiedCompactPaymentTokenWithRecords returned invalid Norito archive/,
      );
      assert.throws(
        () =>
          kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
            recordBundle,
            pallasOpenEnvelopes,
          ),
        /native kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes returned empty Norito payload/,
      );
    },
  );
});

test("Kagemusha Pallas open-envelope JS builders probe availability and validate native output", () => {
  const recordBundle = kagemushaInputArchive(0xe4);
  const previousBundle = kagemushaInputArchive(0xe5);
  const binding = {
    connectNoritoBridgeAbiVersion() {
      return KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION;
    },
    kagemushaBuildPallasOpenEnvelopesArchive(record) {
      rejectMalformedProbe("pallas-builder", record);
      return kagemushaNoritoFrameWithPayload(0xe6);
    },
    kagemushaBuildPreviousProofOpenEnvelopesArchive(previous) {
      rejectMalformedProbe("previous-proof-builder", previous);
      return kagemushaNoritoFrameWithPayload(0xe7);
    },
  };

  withNativeBinding(binding, () => {
    assert.equal(isKagemushaPallasOpenEnvelopeBuilderNativeAvailable(), true);
    assert.deepEqual(
      kagemushaBuildPallasOpenEnvelopesArchive(recordBundle),
      kagemushaNoritoFrameWithPayload(0xe6),
    );
    assert.deepEqual(
      kagemushaBuildPreviousProofOpenEnvelopesArchive(previousBundle),
      kagemushaNoritoFrameWithPayload(0xe7),
    );
  });

  withNativeBinding(
    {
      ...binding,
      connectNoritoBridgeAbiVersion() {
        return KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION - 1;
      },
    },
    () => {
      assert.equal(isKagemushaPallasOpenEnvelopeBuilderNativeAvailable(), false);
      assert.throws(
        () => kagemushaBuildPallasOpenEnvelopesArchive(recordBundle),
        /Kagemusha Pallas open-envelope builders require native bridge ABI 7/,
      );
      assert.throws(
        () => kagemushaBuildPreviousProofOpenEnvelopesArchive(previousBundle),
        /Kagemusha Pallas open-envelope builders require native bridge ABI 7/,
      );
    },
  );

  withNativeBinding(
    {
      ...binding,
      kagemushaBuildPallasOpenEnvelopesArchive(record) {
        rejectMalformedProbe("pallas-builder", record);
        return Buffer.from([1]);
      },
      kagemushaBuildPreviousProofOpenEnvelopesArchive(previous) {
        rejectMalformedProbe("previous-proof-builder", previous);
        return Buffer.from([1]);
      },
    },
    () => {
      assert.equal(isKagemushaPallasOpenEnvelopeBuilderNativeAvailable(), true);
      assert.throws(
        () => kagemushaBuildPallasOpenEnvelopesArchive(recordBundle),
        /native kagemushaBuildPallasOpenEnvelopesArchive returned invalid Norito archive/,
      );
      assert.throws(
        () => kagemushaBuildPreviousProofOpenEnvelopesArchive(previousBundle),
        /native kagemushaBuildPreviousProofOpenEnvelopesArchive returned invalid Norito archive/,
      );
    },
  );

  withNativeBinding(
    {
      ...binding,
      kagemushaBuildPallasOpenEnvelopesArchive(record) {
        rejectMalformedProbe("pallas-builder", record);
        return kagemushaNoritoFrame(0xe8);
      },
      kagemushaBuildPreviousProofOpenEnvelopesArchive(previous) {
        rejectMalformedProbe("previous-proof-builder", previous);
        return kagemushaNoritoFrame(0xe9);
      },
    },
    () => {
      assert.equal(isKagemushaPallasOpenEnvelopeBuilderNativeAvailable(), true);
      assert.throws(
        () => kagemushaBuildPallasOpenEnvelopesArchive(recordBundle),
        /native kagemushaBuildPallasOpenEnvelopesArchive returned empty Norito payload/,
      );
      assert.throws(
        () => kagemushaBuildPreviousProofOpenEnvelopesArchive(previousBundle),
        /native kagemushaBuildPreviousProofOpenEnvelopesArchive returned empty Norito payload/,
      );
    },
  );
});

test("Kagemusha recursive spend exports stable proof circuit ids", () => {
  assert.equal(KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION, 6);
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
  const whitespaceLineageOutputCircuitId =
    ` ${KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1} `;
  assert.equal(
    normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(
      whitespaceLineageOutputCircuitId,
    ),
    whitespaceLineageOutputCircuitId,
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
  assert.equal(
    isSupportedKagemushaRecursiveSpendAppendOutputProofCircuitId(
      whitespaceLineageOutputCircuitId,
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
  assert.equal(requiresKagemushaRecursiveSpendLineageKeyArtifactsForInit(), true);
  assert.equal(
    requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput(
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    ),
    true,
  );
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
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    "unknown-kagemusha-recursive-spend-circuit",
  ]) {
    assert.equal(
      requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput(outputCircuitId),
      false,
    );
  }
  const initVerifierKey = kagemushaLineageVerifierKey(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    0xe7,
  );
  const initProvingKeyArchive = kagemushaLineageProvingKeyArchive(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    initVerifierKey,
    0xe8,
  );
  const oldHashInitProvingKeyArchive = Buffer.from(initProvingKeyArchive);
  OLD_KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH.copy(
    oldHashInitProvingKeyArchive,
    6,
  );
  const appendVerifierKey = kagemushaLineageVerifierKey(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    0xa7,
  );
  const appendProvingKeyArchive = kagemushaLineageProvingKeyArchive(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    appendVerifierKey,
    0xa8,
  );
  const jsLineageArtifacts = kagemushaRecursiveSpendLineageKeyArtifactsForInit(
    128,
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
    initVerifierKey,
    initProvingKeyArchive,
  );
  const exposedVerifierKey = jsLineageArtifacts.lineageVerifierKey;
  const exposedProvingKey = jsLineageArtifacts.lineageProvingKeyArchive;
  exposedVerifierKey[0] = 0;
  exposedProvingKey[0] = 0;
  assert.deepEqual(jsLineageArtifacts.lineageVerifierKey, initVerifierKey);
  assert.deepEqual(jsLineageArtifacts.lineageProvingKeyArchive, initProvingKeyArchive);
  assert.notStrictEqual(
    jsLineageArtifacts.lineageVerifierKey,
    jsLineageArtifacts.lineageVerifierKey,
  );
  const jsAppendLineageArtifacts = kagemushaRecursiveSpendLineageKeyArtifactsForAppend(
    64,
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
    appendVerifierKey,
    appendProvingKeyArchive,
  );
  assert.equal(jsAppendLineageArtifacts.isInitArtifact, false);
  assert.equal(jsAppendLineageArtifacts.isAppendArtifact, true);
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        appendVerifierKey,
        initProvingKeyArchive,
      ),
    /lineage_verifier_key/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForAppend(
        64,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        appendProvingKeyArchive,
      ),
    /lineage_verifier_key/,
  );
  for (const backend of ["halo2/kzg", " halo2/ipa", "halo2/ipa ", "HALO2/IPA"]) {
    assert.throws(
      () =>
        kagemushaRecursiveSpendLineageKeyArtifactsForInit(
          128,
          backend,
          initVerifierKey,
          initProvingKeyArchive,
        ),
      /lineage_verifier_key/,
    );
  }
  const whitespaceCidVerifierKey = kagemushaLineageVerifierKey(
    ` ${KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1} `,
    0xa5,
  );
  const whitespaceCidProvingKeyArchive = kagemushaLineageProvingKeyArchive(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    whitespaceCidVerifierKey,
    0xa6,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        whitespaceCidVerifierKey,
        whitespaceCidProvingKeyArchive,
      ),
    /lineage_verifier_key/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        appendProvingKeyArchive,
      ),
    /lineage_proving_key_archive/,
  );
  const circuitIdBytes = Buffer.from(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    "utf8",
  );
  const overlongVersionLengthArchive = kagemushaNoritoFrameFromSchemaHash(
    KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
    Buffer.concat([
      kagemushaOverlongCompactLength(2),
      Buffer.from([1, 0]),
      kagemushaNoritoField(
        kagemushaNoritoString(
          KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        ),
      ),
      kagemushaNoritoField(kagemushaVerifierKeyCommitment(initVerifierKey)),
      kagemushaNoritoField(kagemushaNoritoByteVec(Buffer.alloc(64, 0xad))),
    ]),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        overlongVersionLengthArchive,
      ),
    /lineage_proving_key_archive/,
  );
  const oversizedTerminalCompactLengthArchive = kagemushaNoritoFrameFromSchemaHash(
    KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
    Buffer.concat([
      kagemushaOversizedTerminalCompactLength(),
      Buffer.from([1, 0]),
      kagemushaNoritoField(
        kagemushaNoritoString(
          KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        ),
      ),
      kagemushaNoritoField(kagemushaVerifierKeyCommitment(initVerifierKey)),
      kagemushaNoritoField(kagemushaNoritoByteVec(Buffer.alloc(64, 0xb0))),
    ]),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        oversizedTerminalCompactLengthArchive,
      ),
    /lineage_proving_key_archive/,
  );
  const hugeCanonicalCompactLengthArchive = kagemushaNoritoFrameFromSchemaHash(
    KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
    Buffer.concat([
      kagemushaHugeCanonicalCompactLength(),
      Buffer.from([1, 0]),
      kagemushaNoritoField(
        kagemushaNoritoString(
          KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        ),
      ),
      kagemushaNoritoField(kagemushaVerifierKeyCommitment(initVerifierKey)),
      kagemushaNoritoField(kagemushaNoritoByteVec(Buffer.alloc(64, 0xb1))),
    ]),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        hugeCanonicalCompactLengthArchive,
      ),
    /lineage_proving_key_archive/,
  );
  const overlongCircuitStringArchive = kagemushaNoritoFrameFromSchemaHash(
    KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
    Buffer.concat([
      kagemushaNoritoField(Buffer.from([1, 0])),
      kagemushaNoritoField(
        Buffer.concat([kagemushaOverlongCompactLength(circuitIdBytes.length), circuitIdBytes]),
      ),
      kagemushaNoritoField(kagemushaVerifierKeyCommitment(initVerifierKey)),
      kagemushaNoritoField(kagemushaNoritoByteVec(Buffer.alloc(64, 0xae))),
    ]),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        overlongCircuitStringArchive,
      ),
    /lineage_proving_key_archive/,
  );
  const invalidUtf8CircuitArchive = kagemushaNoritoFrameFromSchemaHash(
    KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
    Buffer.concat([
      kagemushaNoritoField(Buffer.from([1, 0])),
      kagemushaNoritoField(Buffer.concat([kagemushaNoritoLength(1), Buffer.from([0xff])])),
      kagemushaNoritoField(kagemushaVerifierKeyCommitment(initVerifierKey)),
      kagemushaNoritoField(
        kagemushaNoritoByteVec(Buffer.concat([circuitIdBytes, Buffer.alloc(64, 0xaf)])),
      ),
    ]),
    TEST_NORITO_COMPACT_LEN_FLAG,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        invalidUtf8CircuitArchive,
      ),
    /lineage_proving_key_archive/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        kagemushaLineageProvingKeyArchive(
          KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
          initVerifierKey,
          0xe8,
          {
            provingKey: Buffer.concat([
              Buffer.from(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
                "utf8",
              ),
              Buffer.alloc(64, 0xa6),
            ]),
          },
        ),
      ),
    /lineage_proving_key_archive/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        oldHashInitProvingKeyArchive,
      ),
    /lineage_proving_key_archive/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        kagemushaLineageProvingKeyArchive(
          KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
          appendVerifierKey,
          0xe8,
          {
            provingKey: Buffer.concat([
              kagemushaVerifierKeyCommitment(initVerifierKey),
              Buffer.alloc(64, 0xa7),
            ]),
          },
        ),
      ),
    /lineage_proving_key_archive/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        kagemushaNoritoFrameFromPayload(
          0x9a,
          Buffer.concat([
            Buffer.from([1, 0]),
            Buffer.from(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1, "utf8"),
            kagemushaVerifierKeyCommitment(initVerifierKey),
            Buffer.alloc(64, 0xe8),
          ]),
        ),
      ),
    /lineage_proving_key_archive/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        kagemushaLineageProvingKeyArchive(
          KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
          initVerifierKey,
          0xe8,
          { trailingPayload: Buffer.from([0x7f]) },
        ),
      ),
    /lineage_proving_key_archive/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        kagemushaLineageProvingKeyArchive(
          KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
          initVerifierKey,
          0xe8,
          { flags: TEST_NORITO_COMPACT_LEN_FLAG | TEST_NORITO_PACKED_STRUCT_FLAG },
        ),
      ),
    /lineage_proving_key_archive/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        kagemushaLineageProvingKeyArchive(
          KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
          initVerifierKey,
          0xe8,
          { flags: TEST_NORITO_COMPACT_LEN_FLAG | TEST_NORITO_FIELD_BITSET_FLAG },
        ),
      ),
    /lineage_proving_key_archive/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        kagemushaLineageProvingKeyArchive(
          KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
          initVerifierKey,
          0xe8,
          { version: 2 },
        ),
      ),
    /lineage_proving_key_archive/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        kagemushaLineageProvingKeyArchive(
          KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
          initVerifierKey,
          0xe8,
        ),
      ),
    /lineage_proving_key_archive/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        kagemushaLineageProvingKeyArchive(
          KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
          initVerifierKey,
          0xe8,
          { vkCommitment: Buffer.alloc(32, 0x5a) },
        ),
      ),
    /lineage_proving_key_archive/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        kagemushaLineageProvingKeyArchive(
          KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
          initVerifierKey,
          0xe8,
          { provingKey: Buffer.alloc(0) },
        ),
      ),
    /lineage_proving_key_archive/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        kagemushaLineageProvingKeyArchive(
          KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
          initVerifierKey,
          0xe8,
          { schemaHash: Buffer.alloc(16, 0x9a) },
        ),
      ),
    /lineage_proving_key_archive/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        Buffer.from([0x91]),
        initProvingKeyArchive,
      ),
    /lineage_verifier_key/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        Buffer.concat([
          initVerifierKey,
          kagemushaZk1Tlv(
            "CID1",
            Buffer.from(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1, "utf8"),
          ),
        ]),
        initProvingKeyArchive,
      ),
    /lineage_verifier_key/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        Buffer.from([0x92]),
      ),
    /lineage_proving_key_archive/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        kagemushaNoritoFrameFromPayload(
          0x9a,
          Buffer.concat([Buffer.alloc(32, 0x01), Buffer.alloc(64, 0xe8)]),
        ),
      ),
    /lineage_proving_key_archive/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        kagemushaNoritoFrameFromPayload(
          0x9a,
          Buffer.concat([
            Buffer.from(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1, "utf8"),
            Buffer.alloc(32, 0x00),
            Buffer.alloc(64, 0xe8),
          ]),
        ),
      ),
    /lineage_proving_key_archive/,
  );
  assert.throws(
    () =>
      kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        initVerifierKey,
        kagemushaNoritoFrame(0x9a),
      ),
    /lineage_proving_key_archive/,
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
    isSupportedKagemushaRecursiveSpendPreviousProofCircuitId(
      whitespaceLineageOutputCircuitId,
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
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, Number.NEGATIVE_INFINITY],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 1n],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, new Number(1)],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, true],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, false],
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
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(Number.NEGATIVE_INFINITY), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(1n), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(new Number(1)), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(true), false);
  assert.equal(canAppendKagemushaRecursiveSpendWitnesslessLineage(false), false);
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
    [whitespaceLineageOutputCircuitId, 1],
    [KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 1.5],
    [KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, Number.NaN],
    [KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, Number.POSITIVE_INFINITY],
    [KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, Number.NEGATIVE_INFINITY],
    [KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 1n],
    [KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, new Number(1)],
    [KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, true],
    [KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, false],
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
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
      whitespaceLineageOutputCircuitId,
      1,
    ],
    [
      whitespaceLineageOutputCircuitId,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
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
      1.5,
    ],
    [
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      Number.NaN,
    ],
    [
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      Number.POSITIVE_INFINITY,
    ],
    [
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      Number.NEGATIVE_INFINITY,
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
    [
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      true,
    ],
    [
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      false,
    ],
    [
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      "1",
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
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, Number.NEGATIVE_INFINITY],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 1n],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, new Number(1)],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, true],
    [KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, false],
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
  const initRequest = kagemushaInputArchive(0x61);
  const appendRequest = kagemushaInputArchive(0x62);
  const transitionInitRequest = kagemushaInputArchive(0x63);
  const transitionAppendRequest = kagemushaInputArchive(0x64);
  const boundaryProfile = kagemushaInputArchive(0x65);
  const lineageInitRequest = kagemushaInputArchive(0x66);
  const lineageInitBundle = kagemushaInputArchive(0x67);
  const lineageAppendPreviousWitness = kagemushaInputArchive(0x68);
  const lineageAppendRequest = kagemushaInputArchive(0x69);
  const lineageAppendBundle = kagemushaInputArchive(0x6a);
  const verifyRequest = kagemushaInputArchive(0x6b);
  const redeemRequest = kagemushaInputArchive(0x6c);
  const binding = {
    connectNoritoBridgeAbiVersion() {
      return 6;
    },
    kagemushaRecursiveSpendInit(request) {
      rejectMalformedProbe("init", request);
      calls.push(["init", Buffer.from(request)]);
      return kagemushaNoritoFrameWithPayload(0x31);
    },
    kagemushaRecursiveSpendAppend(request) {
      rejectMalformedProbe("append", request);
      calls.push(["append", Buffer.from(request)]);
      return kagemushaNoritoFrameWithPayload(0x32);
    },
    kagemushaRecursiveSpendTransitionProfileInit(request) {
      rejectMalformedProbe("transition-profile-init", request);
      calls.push(["transition-profile-init", Buffer.from(request)]);
      return kagemushaNoritoFrameWithPayload(0x37);
    },
    kagemushaRecursiveSpendTransitionProfileAppend(request) {
      rejectMalformedProbe("transition-profile-append", request);
      calls.push(["transition-profile-append", Buffer.from(request)]);
      return kagemushaNoritoFrameWithPayload(0x38);
    },
    kagemushaRecursiveSpendLineageAppendBoundary(profile) {
      rejectMalformedProbe("lineage-append-boundary", profile);
      calls.push(["lineage-append-boundary", Buffer.from(profile)]);
      return kagemushaNoritoFrameWithPayload(0x39);
    },
    kagemushaRecursiveSpendLineageWitnessFromInitResult(request, bundle) {
      rejectMalformedProbe("lineage-init", request, bundle);
      calls.push(["lineage-init", Buffer.from(request), Buffer.from(bundle)]);
      return kagemushaNoritoFrameWithPayload(0x33);
    },
    kagemushaRecursiveSpendLineageWitnessAppendResult(previousWitness, request, bundle) {
      rejectMalformedProbe("lineage-append", previousWitness, request, bundle);
      calls.push([
        "lineage-append",
        Buffer.from(previousWitness),
        Buffer.from(request),
        Buffer.from(bundle),
      ]);
      return kagemushaNoritoFrameWithPayload(0x34);
    },
    kagemushaRecursiveSpendVerify(request) {
      rejectMalformedProbe("verify", request);
      calls.push(["verify", Buffer.from(request)]);
      return kagemushaNoritoFrameWithPayload(0x35);
    },
    kagemushaRecursiveSpendRedeem(request) {
      rejectMalformedProbe("redeem", request);
      calls.push(["redeem", Buffer.from(request)]);
      return kagemushaNoritoFrameWithPayload(0x36);
    },
  };

  withNativeBinding(binding, () => {
    assert.equal(isKagemushaRecursiveSpendNativeAvailable(), true);
    assert.deepEqual(
      kagemushaRecursiveSpendInit(initRequest),
      kagemushaNoritoFrameWithPayload(0x31),
    );
    assert.deepEqual(
      kagemushaRecursiveSpendAppend(appendRequest),
      kagemushaNoritoFrameWithPayload(0x32),
    );
    assert.deepEqual(
      kagemushaRecursiveSpendTransitionProfileInit(transitionInitRequest),
      kagemushaNoritoFrameWithPayload(0x37),
    );
    assert.deepEqual(
      kagemushaRecursiveSpendTransitionProfileAppend(transitionAppendRequest),
      kagemushaNoritoFrameWithPayload(0x38),
    );
    assert.deepEqual(
      kagemushaRecursiveSpendLineageAppendBoundary(boundaryProfile),
      kagemushaNoritoFrameWithPayload(0x39),
    );
    assert.deepEqual(
      kagemushaRecursiveSpendLineageWitnessFromInitResult(lineageInitRequest, lineageInitBundle),
      kagemushaNoritoFrameWithPayload(0x33),
    );
    assert.deepEqual(
      kagemushaRecursiveSpendLineageWitnessAppendResult(
        lineageAppendPreviousWitness,
        lineageAppendRequest,
        lineageAppendBundle,
      ),
      kagemushaNoritoFrameWithPayload(0x34),
    );
    assert.deepEqual(
      kagemushaRecursiveSpendVerify(verifyRequest),
      kagemushaNoritoFrameWithPayload(0x35),
    );
    assert.deepEqual(
      kagemushaRecursiveSpendRedeem(redeemRequest),
      kagemushaNoritoFrameWithPayload(0x36),
    );
  });

  assert.deepEqual(calls, [
    ["init", initRequest],
    ["append", appendRequest],
    ["transition-profile-init", transitionInitRequest],
    ["transition-profile-append", transitionAppendRequest],
    ["lineage-append-boundary", boundaryProfile],
    ["lineage-init", lineageInitRequest, lineageInitBundle],
    ["lineage-append", lineageAppendPreviousWitness, lineageAppendRequest, lineageAppendBundle],
    ["verify", verifyRequest],
    ["redeem", redeemRequest],
  ]);
});

test("Kagemusha recursive spend lineage helpers pass owned archive copies to native", () => {
  const calls = [];
  const initRequest = kagemushaInputArchive(0xa1);
  const initBundle = Uint8Array.from(kagemushaInputArchive(0xa2));
  const appendPreviousWitness = kagemushaInputArchive(0xa3);
  const appendRequest = Uint8Array.from(kagemushaInputArchive(0xa4));
  const appendBundle = kagemushaInputArchive(0xa5);
  const expectedInitRequest = Buffer.from(initRequest);
  const expectedInitBundle = Buffer.from(initBundle);
  const expectedAppendPreviousWitness = Buffer.from(appendPreviousWitness);
  const expectedAppendRequest = Buffer.from(appendRequest);
  const expectedAppendBundle = Buffer.from(appendBundle);
  const binding = completeRecursiveSpendBinding({
    kagemushaRecursiveSpendLineageWitnessFromInitResult(request, bundle) {
      rejectMalformedProbe("lineage-init", request, bundle);
      calls.push(["lineage-init", request, bundle]);
      return kagemushaNoritoFrameWithPayload(0xb1);
    },
    kagemushaRecursiveSpendLineageWitnessAppendResult(previousWitness, request, bundle) {
      rejectMalformedProbe("lineage-append", previousWitness, request, bundle);
      calls.push(["lineage-append", previousWitness, request, bundle]);
      return kagemushaNoritoFrameWithPayload(0xb2);
    },
  });

  withNativeBinding(binding, () => {
    assert.deepEqual(
      kagemushaRecursiveSpendLineageWitnessFromInitResult(initRequest, initBundle),
      kagemushaNoritoFrameWithPayload(0xb1),
    );
    assert.deepEqual(
      kagemushaRecursiveSpendLineageWitnessAppendResult(
        appendPreviousWitness,
        appendRequest,
        appendBundle,
      ),
      kagemushaNoritoFrameWithPayload(0xb2),
    );
  });

  initRequest[6] = 0x7f;
  initBundle[6] = 0x7f;
  appendPreviousWitness[6] = 0x7f;
  appendRequest[6] = 0x7f;
  appendBundle[6] = 0x7f;

  assert.notStrictEqual(calls[0][1], initRequest);
  assert.notStrictEqual(calls[1][1], appendPreviousWitness);
  assert.deepEqual(calls, [
    ["lineage-init", expectedInitRequest, expectedInitBundle],
    ["lineage-append", expectedAppendPreviousWitness, expectedAppendRequest, expectedAppendBundle],
  ]);
});

test("Kagemusha recursive spend availability requires native bridge ABI 6", () => {
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
      () => kagemushaRecursiveSpendInit(kagemushaInputArchive(0x70)),
      /Kagemusha recursive spend helper 'kagemushaRecursiveSpendInit' is unavailable/,
    );
  });

  for (const abiVersion of [
    "6",
    true,
    -1,
    6.5,
    Number.NaN,
    Number.POSITIVE_INFINITY,
    Number.MAX_SAFE_INTEGER + 1,
    0x1_0000_0000,
  ]) {
    withNativeBinding(
      completeRecursiveSpendBinding({
        connectNoritoBridgeAbiVersion() {
          return abiVersion;
        },
        kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes() {
          return Uint8Array.from([10]);
        },
        kagemushaVerifyRecursiveCompactPaymentToken() {
          return true;
        },
      }),
      () => {
        assert.equal(isKagemushaRecursiveSpendNativeAvailable(), false);
        assert.equal(isKagemushaRecursiveCompactPaymentTokenNativeAvailable(), false);
        assert.equal(
          preferredKagemushaOfflineSpendMode(),
          KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
        );
      },
    );
  }
});

test("Kagemusha recursive spend availability rejects broken native bridge ABI probes", () => {
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
      () => kagemushaRecursiveSpendInit(kagemushaInputArchive(0x71)),
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
        () => kagemushaRecursiveSpendVerify(kagemushaInputArchive(0x72)),
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
        () => kagemushaRecursiveSpendVerify(kagemushaInputArchive(0x73)),
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
      () => kagemushaRecursiveSpendInit(kagemushaInputArchive(0x80)),
      /native kagemushaRecursiveSpendInit returned empty output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendAppend(kagemushaInputArchive(0x81)),
      /native kagemushaRecursiveSpendAppend returned empty output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendTransitionProfileInit(kagemushaInputArchive(0x82)),
      /native kagemushaRecursiveSpendTransitionProfileInit returned empty output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendTransitionProfileAppend(kagemushaInputArchive(0x83)),
      /native kagemushaRecursiveSpendTransitionProfileAppend returned empty output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageAppendBoundary(kagemushaInputArchive(0x84)),
      /native kagemushaRecursiveSpendLineageAppendBoundary returned empty output/,
    );
    assert.throws(
      () =>
        kagemushaRecursiveSpendLineageWitnessFromInitResult(
          kagemushaInputArchive(0x85),
          kagemushaInputArchive(0x86),
        ),
      /native kagemushaRecursiveSpendLineageWitnessFromInitResult returned empty output/,
    );
    assert.throws(
      () =>
        kagemushaRecursiveSpendLineageWitnessAppendResult(
          kagemushaInputArchive(0x87),
          kagemushaInputArchive(0x88),
          kagemushaInputArchive(0x89),
        ),
      /native kagemushaRecursiveSpendLineageWitnessAppendResult returned empty output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendVerify(kagemushaInputArchive(0x8a)),
      /native kagemushaRecursiveSpendVerify returned empty output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendRedeem(kagemushaInputArchive(0x8b)),
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
      () => kagemushaRecursiveSpendRedeem(kagemushaInputArchive(0x8c)),
      /native kagemushaRecursiveSpendRedeem returned oversized output/,
    );
  });
});

test("Kagemusha recursive spend helpers reject malformed Norito native outputs", () => {
  function assertRejectsMalformedNativeRedeemOutput(output) {
    const binding = completeRecursiveSpendBinding({
      kagemushaRecursiveSpendRedeem(request) {
        rejectMalformedProbe("redeem", request);
        return output;
      },
    });

    withNativeBinding(binding, () => {
      assert.throws(
        () => kagemushaRecursiveSpendRedeem(kagemushaInputArchive(0x8d)),
        /native kagemushaRecursiveSpendRedeem returned invalid Norito archive/,
      );
    });
  }

  assertRejectsMalformedNativeRedeemOutput(Buffer.from([0x01]));

  const compressed = kagemushaNoritoFrameWithPayload(0x36);
  compressed[22] = 1;
  assertRejectsMalformedNativeRedeemOutput(compressed);

  const unsupportedFlags = kagemushaNoritoFrameWithPayload(0x36);
  unsupportedFlags[39] = 0x08;
  assertRejectsMalformedNativeRedeemOutput(unsupportedFlags);

  const invalidFieldBitset = kagemushaNoritoFrameWithPayload(0x36);
  invalidFieldBitset[39] = 0x20;
  assertRejectsMalformedNativeRedeemOutput(invalidFieldBitset);

  assertRejectsMalformedNativeRedeemOutput(
    kagemushaNoritoFrameWithHeaderPadding(
      kagemushaNoritoFrameWithPayload(0x36),
      Buffer.from([0x7f]),
    ),
  );
  assertRejectsMalformedNativeRedeemOutput(
    kagemushaNoritoFrameWithHeaderPadding(
      kagemushaNoritoFrameWithPayload(0x36),
      Buffer.alloc(65),
    ),
  );
});

test("Kagemusha recursive spend helpers reject empty-payload Norito native outputs", () => {
  const binding = completeRecursiveSpendBinding({
    kagemushaRecursiveSpendRedeem(request) {
      rejectMalformedProbe("redeem", request);
      return kagemushaNoritoFrame(0x36);
    },
  });

  withNativeBinding(binding, () => {
    assert.throws(
      () => kagemushaRecursiveSpendRedeem(kagemushaInputArchive(0x8e)),
      /native kagemushaRecursiveSpendRedeem returned empty Norito payload/,
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
      () => kagemushaRecursiveSpendInit(kagemushaInputArchive(0x90)),
      /native kagemushaRecursiveSpendInit returned no output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendAppend(kagemushaInputArchive(0x91)),
      /native kagemushaRecursiveSpendAppend returned no output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendTransitionProfileInit(kagemushaInputArchive(0x92)),
      /native kagemushaRecursiveSpendTransitionProfileInit returned no output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendTransitionProfileAppend(kagemushaInputArchive(0x93)),
      /native kagemushaRecursiveSpendTransitionProfileAppend returned no output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageAppendBoundary(kagemushaInputArchive(0x94)),
      /native kagemushaRecursiveSpendLineageAppendBoundary returned no output/,
    );
    assert.throws(
      () =>
        kagemushaRecursiveSpendLineageWitnessFromInitResult(
          kagemushaInputArchive(0x95),
          kagemushaInputArchive(0x96),
        ),
      /native kagemushaRecursiveSpendLineageWitnessFromInitResult returned no output/,
    );
    assert.throws(
      () =>
        kagemushaRecursiveSpendLineageWitnessAppendResult(
          kagemushaInputArchive(0x97),
          kagemushaInputArchive(0x98),
          kagemushaInputArchive(0x99),
        ),
      /native kagemushaRecursiveSpendLineageWitnessAppendResult returned no output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendVerify(kagemushaInputArchive(0x9a)),
      /native kagemushaRecursiveSpendVerify returned no output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendRedeem(kagemushaInputArchive(0x9b)),
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
      () => kagemushaRecursiveSpendInit(kagemushaInputArchive(0xa0)),
      /native kagemushaRecursiveSpendInit returned text instead of Norito bytes/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendAppend(kagemushaInputArchive(0xa1)),
      /native kagemushaRecursiveSpendAppend returned text instead of Norito bytes/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendTransitionProfileInit(kagemushaInputArchive(0xa2)),
      /native kagemushaRecursiveSpendTransitionProfileInit returned text instead of Norito bytes/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendTransitionProfileAppend(kagemushaInputArchive(0xa3)),
      /native kagemushaRecursiveSpendTransitionProfileAppend returned text instead of Norito bytes/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageAppendBoundary(kagemushaInputArchive(0xa4)),
      /native kagemushaRecursiveSpendLineageAppendBoundary returned text instead of Norito bytes/,
    );
    assert.throws(
      () =>
        kagemushaRecursiveSpendLineageWitnessFromInitResult(
          kagemushaInputArchive(0xa5),
          kagemushaInputArchive(0xa6),
        ),
      /native kagemushaRecursiveSpendLineageWitnessFromInitResult returned text instead of Norito bytes/,
    );
    assert.throws(
      () =>
        kagemushaRecursiveSpendLineageWitnessAppendResult(
          kagemushaInputArchive(0xa7),
          kagemushaInputArchive(0xa8),
          kagemushaInputArchive(0xa9),
        ),
      /native kagemushaRecursiveSpendLineageWitnessAppendResult returned text instead of Norito bytes/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendVerify(kagemushaInputArchive(0xaa)),
      /native kagemushaRecursiveSpendVerify returned text instead of Norito bytes/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendRedeem(kagemushaInputArchive(0xab)),
      /native kagemushaRecursiveSpendRedeem returned text instead of Norito bytes/,
    );
  });
});

test("Kagemusha recursive spend redeem propagates native over-cap hop-count rejection", () => {
  const calls = [];
  const request = kagemushaInputArchive(0xac);
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
      () => kagemushaRecursiveSpendRedeem(request),
      /bundle\.accumulator\.hop_count/,
    );
  });

  assert.deepEqual(calls, [request]);
});

test("Kagemusha recursive spend helpers propagate forged lineage verifier-record rejection", () => {
  const calls = [];
  const verifyRequest = kagemushaInputArchive(0xad);
  const redeemRequest = kagemushaInputArchive(0xae);
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
      () => kagemushaRecursiveSpendVerify(verifyRequest),
      /lineage_verifier_record\.commitment/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendRedeem(redeemRequest),
      /lineage_verifier_record\.commitment/,
    );
  });

  assert.deepEqual(calls, [
    ["verify", verifyRequest],
    ["redeem", redeemRequest],
  ]);
});

test("Kagemusha recursive spend transition profile append propagates forged opening rejection", () => {
  const calls = [];
  const request = kagemushaInputArchive(0xaf);
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
      () => kagemushaRecursiveSpendTransitionProfileAppend(request),
      /hop domain metadata mismatch/,
    );
  });

  assert.deepEqual(calls, [request]);
});

test("Kagemusha recursive spend availability fails closed when native methods are partial", () => {
  withNativeBinding({ kagemushaRecursiveSpendInit() {} }, () => {
    assert.equal(isKagemushaRecursiveSpendNativeAvailable(), false);
    assert.throws(
      () => kagemushaRecursiveSpendInit(kagemushaInputArchive(0xb0)),
      /Kagemusha recursive spend helper 'kagemushaRecursiveSpendInit' is unavailable/,
    );
  });
});
