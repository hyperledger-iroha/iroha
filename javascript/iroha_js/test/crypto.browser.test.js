import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import {
  buildKaigiRosterJoinProof,
  buildZkAceTransferAuthorizationV1,
  generateKeyPair,
  isKagemushaRecursiveSpendNativeAvailable,
  kagemushaRecursiveSpendInit,
  KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
  KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1,
  normalizeCryptoAlgorithm,
  preferredKagemushaOfflineSpendMode,
  preferredKagemushaOfflineSpendModeForCapabilities,
  supportedCryptoAlgorithms,
} from "../src/crypto.browser.js";
import * as srcBrowserCrypto from "../src/crypto.browser.js";
import * as distBrowserCrypto from "../dist/crypto.browser.js";

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

function browserNoritoFrameFromPayload(schemaByte, payload) {
  const payloadBuffer = Buffer.from(payload);
  const frame = Buffer.concat([browserNoritoFrame(schemaByte), payloadBuffer]);
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

function browserNoritoFrameFromSchemaHash(schemaHash, payload, flags = 0) {
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

function browserNoritoFrame(schemaByte) {
  const frame = Buffer.alloc(40);
  frame.write("NRT0", 0, "ascii");
  frame.fill(schemaByte, 6, 22);
  return frame;
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

function kagemushaVerifierKeyCommitment(crypto, verifierKey) {
  const backend = Buffer.from(crypto.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND, "utf8");
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

function kagemushaLineageProvingKeyArchive(crypto, circuitId, verifierKey, seed) {
  const flags = TEST_NORITO_COMPACT_LEN_FLAG;
  const version = Buffer.alloc(2);
  version.writeUInt16LE(1);
  const payload = Buffer.concat([
    kagemushaNoritoField(version, flags),
    kagemushaNoritoField(kagemushaNoritoString(circuitId, flags), flags),
    kagemushaNoritoField(kagemushaVerifierKeyCommitment(crypto, verifierKey), flags),
    kagemushaNoritoField(kagemushaNoritoByteVec(Buffer.alloc(64, seed)), flags),
  ]);
  return browserNoritoFrameFromSchemaHash(
    KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
    payload,
    flags,
  );
}

test("browser crypto bundle exposes Kaigi roster proof helper as unsupported", () => {
  assert.throws(
    () => buildKaigiRosterJoinProof({ seed: Buffer.from("seed") }),
    /buildKaigiRosterJoinProof is unavailable in browser-only crypto builds/,
  );
  assert.throws(
    () => buildZkAceTransferAuthorizationV1({}),
    /buildZkAceTransferAuthorizationV1 is unavailable in browser-only crypto builds/,
  );
});

test("browser crypto normalizes all algorithm labels but only signs Ed25519 locally", () => {
  assert.ok(supportedCryptoAlgorithms().includes("ml-dsa"));
  assert.equal(normalizeCryptoAlgorithm("gost3410-2012-256-paramset-a"), "gost3410-2012-256-paramset-a");
  for (const [label, crypto] of [
    ["src", srcBrowserCrypto],
    ["dist", distBrowserCrypto],
  ]) {
    assert.equal(crypto.normalizeCryptoAlgorithm("ed-25519"), "ed25519", `${label} keeps ASCII aliases`);
    assert.throws(
      () => crypto.normalizeCryptoAlgorithm("ed\t25519"),
      /unsupported crypto algorithm/,
      `${label} rejects control-character aliases`,
    );
    assert.throws(
      () => crypto.normalizeCryptoAlgorithm("ed\u200B25519"),
      /unsupported crypto algorithm/,
      `${label} rejects zero-width aliases`,
    );
    assert.throws(
      () => crypto.normalizeCryptoAlgorithm("\u0435d25519"),
      /unsupported crypto algorithm/,
      `${label} rejects Cyrillic aliases`,
    );
  }
  assert.equal(generateKeyPair({ seed: Buffer.alloc(32, 7) }).algorithm, "ed25519");
  assert.throws(
    () => generateKeyPair({ algorithm: "ml-dsa", seed: Buffer.alloc(32, 7) }),
    /generateKeyPair\(ml-dsa\) is unavailable in browser-only crypto builds/,
  );
});

test("browser crypto exposes native-only helpers as safe stubs", () => {
  for (const [label, crypto] of [
    ["src", srcBrowserCrypto],
    ["dist", distBrowserCrypto],
  ]) {
    assert.equal(crypto.isPrivacyNativeAvailable(), false, `${label} privacy bridge must be unavailable`);
    assert.equal(
      crypto.isKagemushaRecursiveSpendNativeAvailable(),
      false,
      `${label} Kagemusha native bridge must be unavailable`,
    );
    assert.equal(
      crypto.preferredKagemushaOfflineSpendMode(),
      crypto.KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
      `${label} browser build must default to checked prefold spend mode`,
    );
    assert.equal(
      crypto.preferredKagemushaOfflineSpendMode(true),
      crypto.KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1,
      `${label} native recursive availability should select recursive spend mode`,
    );
    assert.equal(
      crypto.KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1,
      "recursive_compact_v1",
      `${label} exposes recursive compact spend mode`,
    );
    assert.equal(
      crypto.isKagemushaRecursiveCompactUnavailable(
        new Error(
          crypto.KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT,
        ),
      ),
      true,
      `${label} classifies reserved compact payment-token unavailable errors`,
    );
    assert.equal(
      crypto.isKagemushaRecursiveCompactUnavailable(
        `bridge: ${crypto.KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT}`,
      ),
      true,
      `${label} classifies reserved compact multi-hop unavailable errors`,
    );
    assert.equal(
      crypto.isKagemushaRecursiveCompactUnavailable(
        new Error("recursive compact proof composition unavailable"),
      ),
      false,
      `${label} rejects vague recursive compact proof errors`,
    );
    assert.equal(
      crypto.preferredKagemushaOfflineSpendModeForCapabilities(true, true),
      crypto.KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1,
      `${label} capability helper should prefer recursive spend mode`,
    );
    assert.equal(
      crypto.isKagemushaRecursiveCompactPaymentTokenNativeAvailable(),
      false,
      `${label} browser build must not expose native recursive compact prover`,
    );
    assert.equal(
      crypto.isKagemushaCompactPaymentTokenNativeAvailable(),
      false,
      `${label} browser build must not expose native compact-token prover`,
    );
    assert.equal(
      crypto.isKagemushaRecursiveAggregationProofBundleNativeAvailable(),
      false,
      `${label} browser build must not expose native recursive aggregation prover`,
    );
    assert.equal(
      crypto.isKagemushaPallasOpenEnvelopeBuilderNativeAvailable(),
      false,
      `${label} browser build must not expose native Pallas open-envelope builders`,
    );
    assert.equal(
      crypto.isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable(),
      false,
      `${label} browser build must not expose native recursive spend compact projection`,
    );
    assert.equal(
      crypto.isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable(),
      false,
      `${label} browser build must not expose native recursive spend compact projection verifier`,
    );
    assert.throws(
      () => crypto.kagemushaProveVerifiedCompactPaymentTokenWithRecords(),
      /unavailable in browser-only crypto builds/,
      `${label} compact-token prover must be native-only`,
    );
    assert.throws(
      () =>
        crypto.kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(),
      /unavailable in browser-only crypto builds/,
      `${label} recursive aggregation prover must be native-only`,
    );
    assert.throws(
      () =>
        crypto.kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(),
      /unavailable in browser-only crypto builds/,
      `${label} recursive compact prover must be native-only`,
    );
    assert.throws(
      () => crypto.kagemushaBuildPallasOpenEnvelopesArchive(),
      /unavailable in browser-only crypto builds/,
      `${label} Pallas open-envelope builder must be native-only`,
    );
    assert.throws(
      () => crypto.kagemushaBuildPreviousProofOpenEnvelopesArchive(),
      /unavailable in browser-only crypto builds/,
      `${label} previous proof open-envelope builder must be native-only`,
    );
    assert.throws(
      () => crypto.kagemushaRecursiveSpendCompactPaymentTokenFromBundle(),
      /unavailable in browser-only crypto builds/,
      `${label} recursive spend compact projection must be native-only`,
    );
    assert.throws(
      () => crypto.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(),
      /unavailable in browser-only crypto builds/,
      `${label} recursive spend compact projection verifier must be native-only`,
    );
    assert.equal(crypto.KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1, 1);
    assert.equal(
      crypto.KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES,
      8 * 1024 * 1024,
    );
    assert.equal(
      crypto.KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES,
      128,
    );
    assert.equal(
      crypto.KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN,
      "iroha:kagemusha:v1:recursive-spend-transition-profile",
    );
    assert.equal(
      crypto.KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN,
      "iroha:kagemusha:v1:recursive-spend-transition-profile-digest",
    );
    assert.equal(
      crypto.KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN,
      "iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest",
    );
    assert.equal(
      crypto.KAGEMUSHA_RECURSIVE_SPEND_INIT_REQUEST_WIRE_NAME,
      "iroha_data_model::offline::model::KagemushaRecursiveSpendInitRequestV1",
    );
    assert.equal(
      crypto.KAGEMUSHA_RECURSIVE_SPEND_VERIFY_RESULT_WIRE_NAME,
      "iroha_data_model::offline::model::KagemushaRecursiveSpendVerifyResultV1",
    );
    assert.equal(
      crypto.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
      "iroha_data_model::offline::model::KagemushaRecursiveSpendBundleV1",
    );
    assert.equal(
      crypto.normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(""),
      crypto.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    );
    assert.equal(
      crypto.isSupportedKagemushaRecursiveSpendAppendOutputProofCircuitId(""),
      true,
    );
    assert.equal(
      crypto.isSupportedKagemushaRecursiveSpendPreviousProofCircuitId(
        crypto.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      ),
      true,
    );
    assert.equal(
      crypto.requiresKagemushaRecursiveSpendPreviousLineageVerifierRecordForAppend(
        crypto.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      ),
      false,
    );
    assert.equal(
      crypto.requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend("", 1),
      false,
    );
    assert.equal(crypto.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND, "halo2/ipa");
    assert.equal(
      crypto.isSupportedKagemushaRecursiveSpendLineageKeyArtifactOpeningLen(128),
      true,
    );
    assert.equal(
      crypto.isSupportedKagemushaRecursiveSpendLineageKeyArtifactOpeningLen(3),
      false,
    );
    const verifierKey = kagemushaLineageVerifierKey(
      crypto.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
      0xe7,
    );
    const provingKeyArchive = kagemushaLineageProvingKeyArchive(
      crypto,
      crypto.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
      verifierKey,
      0xe8,
    );
    const artifacts = crypto.kagemushaRecursiveSpendLineageKeyArtifactsForInit(
      128,
      crypto.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
      verifierKey,
      provingKeyArchive,
    );
    assert.equal(
      artifacts.proofCircuitId,
      crypto.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    );
    assert.equal(artifacts.isInitArtifact, true);
    assert.equal(artifacts.isAppendArtifact, false);
    assert.deepEqual(artifacts.lineageVerifierKey, verifierKey);
    assert.deepEqual(artifacts.lineageProvingKeyArchive, provingKeyArchive);
    const exposedVerifierKey = artifacts.lineageVerifierKey;
    const exposedProvingKey = artifacts.lineageProvingKeyArchive;
    exposedVerifierKey[0] = 0;
    exposedProvingKey[0] = 0;
    assert.equal(artifacts.lineageVerifierKey[0], 0x5a);
    assert.equal(artifacts.lineageProvingKeyArchive[0], 0x4e);
    assert.notStrictEqual(artifacts.lineageVerifierKey, artifacts.lineageVerifierKey);
    assert.throws(
      () => crypto.kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        crypto.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        kagemushaLineageVerifierKey(
          crypto.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
          0xa7,
        ),
        provingKeyArchive,
      ),
      /lineage_verifier_key/,
    );
    const oldHashProvingKeyArchive = Buffer.from(provingKeyArchive);
    OLD_KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH.copy(
      oldHashProvingKeyArchive,
      6,
    );
    assert.throws(
      () => crypto.kagemushaRecursiveSpendLineageKeyArtifactsForInit(
        128,
        crypto.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        verifierKey,
        oldHashProvingKeyArchive,
      ),
      /lineage_proving_key_archive/,
    );
    assert.throws(
      () => crypto.kagemushaRecursiveSpendLineageKeyArtifactsForAppend(
        3,
        crypto.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        Buffer.from([1]),
        Buffer.from([2]),
      ),
      /verifier_opening_len/,
    );
    assert.equal(crypto.PRIVACY_NATIVE_ARCHIVE_MAX_BYTES, 64 * 1024 * 1024);
    assert.equal(crypto.PRIVACY_FFI_VERSION_V1, 1);
    assert.equal(crypto.PRIVACY_REQUIRED_BRIDGE_ABI_VERSION, 7);
    assert.equal(crypto.PRIVACY_FFI_STATUS_ERROR, 1);
    assert.equal(crypto.PRIVACY_FFI_ERROR_NULL_POINTER, 1);
    assert.equal(crypto.PRIVACY_FFI_ERROR_MALFORMED_NORITO, 2);
    assert.equal(crypto.PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM, 3);
    assert.equal(crypto.PRIVACY_FFI_ERROR_PRODUCTION_DISABLED, 4);
    assert.equal(crypto.PRIVACY_FFI_ERROR_INVALID_REQUEST, 5);
    assert.throws(
      () => crypto.privacyCapabilitiesV1(),
      /privacyCapabilitiesV1 is unavailable in browser-only crypto builds/,
    );
    assert.throws(
      () => crypto.privacyProofRequestV1({
        algorithmId: "zk-ace-pq-authorization-v0",
        entrypoint: "buildZkAceAuthorizationProofV1",
        vkRef: "stark-fri:zk_ace_pq_authorization_v0",
        publicInputs: Buffer.from([1]),
      }),
      /privacyProofRequestV1 is unavailable in browser-only crypto builds/,
    );
    assert.throws(
      () => crypto.privacyBuildProofV1(Buffer.from([1])),
      /privacyBuildProofV1 is unavailable in browser-only crypto builds/,
    );
    assert.throws(
      () => crypto.privacyVerifyProofV1(Buffer.from([1])),
      /privacyVerifyProofV1 is unavailable in browser-only crypto builds/,
    );
    assert.throws(
      () => crypto.kagemushaRecursiveSpendInit(Buffer.from([1])),
      /kagemushaRecursiveSpendInit is unavailable in browser-only crypto builds/,
    );
    assert.throws(
      () => crypto.encodeKagemushaRecursiveSpendInitRequest({}),
      /encodeKagemushaRecursiveSpendInitRequest is unavailable in browser-only crypto builds/,
    );
    assert.throws(
      () => crypto.decodeKagemushaRecursiveSpendBundle(Buffer.from([1])),
      /decodeKagemushaRecursiveSpendBundle is unavailable in browser-only crypto builds/,
    );
    assert.throws(
      () => crypto.kagemushaRecursiveSpendVerifyTyped({}),
      /kagemushaRecursiveSpendVerifyTyped is unavailable in browser-only crypto builds/,
    );
    assert.throws(
      () => crypto.kagemushaRecursiveSpendTransitionProfileInit(Buffer.from([1])),
      /kagemushaRecursiveSpendTransitionProfileInit is unavailable in browser-only crypto builds/,
    );
    assert.throws(
      () => crypto.kagemushaRecursiveSpendTransitionProfileAppend(Buffer.from([1])),
      /kagemushaRecursiveSpendTransitionProfileAppend is unavailable in browser-only crypto builds/,
    );
  }
});

test("browser crypto covers the package root crypto export surface", () => {
  const indexSource = readFileSync(new URL("../src/index.js", import.meta.url), "utf8");
  const browserCryptoSource = readFileSync(
    new URL("../src/crypto.browser.js", import.meta.url),
    "utf8",
  );
  const exportBlocks = [
    ...indexSource.matchAll(/export\s+\{([\s\S]*?)\}\s+from\s+"([^"]+)";/g),
  ];
  const cryptoExportBlock = exportBlocks.find((match) => match[2] === "./crypto.js");
  assert.ok(cryptoExportBlock);
  const rootCryptoExports = cryptoExportBlock[1]
    .split(",")
    .map((name) => name.trim())
    .filter(Boolean);
  const browserExports = new Set(
    [...browserCryptoSource.matchAll(/export\s+(?:const|function|class)\s+([A-Za-z0-9_]+)/g)]
      .map((match) => match[1]),
  );
  assert.deepEqual(
    rootCryptoExports.filter((name) => !browserExports.has(name)),
    [],
  );
});

test("browser package wiring keeps privacy catalogs on mapped crypto stubs", () => {
  const packageJson = JSON.parse(
    readFileSync(new URL("../package.json", import.meta.url), "utf8"),
  );
  assert.equal(packageJson.exports["./crypto"].browser, "./dist/crypto.browser.js");
  assert.equal(packageJson.browser["./dist/crypto.js"], "./dist/crypto.browser.js");

  for (const [label, relativePath] of [
    ["src", "../src/privacyAlgorithms.js"],
    ["dist", "../dist/privacyAlgorithms.js"],
  ]) {
    const source = readFileSync(new URL(relativePath, import.meta.url), "utf8");
    assert.match(
      source,
      /^import \{ isPrivacyNativeAvailable \} from "\.\/crypto\.js";/m,
      `${label} privacy catalog must route bridge detection through browser-mapped crypto.js`,
    );
    assert.doesNotMatch(
      source,
      /iroha_js_host|__IROHA_NATIVE_BINDING__|privacyCapabilitiesV1|privacyBuildProofV1|privacyVerifyProofV1/,
      `${label} privacy catalog must not call native privacy FFI directly`,
    );
  }
});
