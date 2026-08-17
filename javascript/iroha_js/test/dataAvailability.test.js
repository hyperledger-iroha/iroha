import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { blake3 } from "@noble/hashes/blake3";
import { ed25519 } from "@noble/curves/ed25519";

import {
  buildDaIngestRequest,
  computeDaIngestSigningDigest,
  emitDaProofSummaryArtifact,
  generateDaProofSummary,
} from "../src/dataAvailability.js";
import { AccountAddress } from "../src/address.js";
import { signEd25519 } from "../src/crypto.js";
import { NetworkId } from "../src/networkId.js";

const PRIVATE_KEY = Buffer.alloc(32, 0x24);
const CLIENT_BLOB_ID = "11".repeat(32);
const PAYLOAD = Buffer.from("payload-for-da");
const NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0xA5));
const OWNER = AccountAddress.fromAccount({
  publicKey: Buffer.from(ed25519.getPublicKey(PRIVATE_KEY)),
}).toI105();

test("buildDaIngestRequest signs the complete canonical intent and encodes DA fields", () => {
  const chunkSize = 1024;
  const { request, artifacts } = buildDaIngestRequest({
    payload: PAYLOAD,
    networkId: NETWORK_ID,
    owner: OWNER,
    clientBlobId: CLIENT_BLOB_ID,
    privateKey: PRIVATE_KEY,
    chunkSize,
    laneId: 5,
    epoch: 6,
    sequence: 7,
    codec: "text/plain",
    blobClass: { class: "Custom", value: 99 },
    erasureProfile: {
      dataShards: 2,
      parityShards: 1,
      chunkAlignment: 4,
      fecScheme: { scheme: "Custom", value: 7 },
    },
    retentionPolicy: {
      hotRetentionSecs: 120,
      coldRetentionSecs: 240,
      requiredReplicas: 2,
      storageClass: "Cold",
      governanceTag: "da.custom",
    },
    metadata: { note: "hello-world" },
    compression: "Zstd",
  });

  const signingDigest = computeDaIngestSigningDigest(request);
  const expectedSignatureHex = signEd25519(signingDigest, PRIVATE_KEY)
    .toString("hex")
    .toUpperCase();
  const expectedBlobId = Buffer.from(CLIENT_BLOB_ID, "hex");

  assert.equal(request.signatures[0].signature, expectedSignatureHex);
  assert.equal(request.signatures[0].signature, artifacts.signatureHex);
  assert.equal(artifacts.signingDigestHex, signingDigest.toString("hex").toUpperCase());
  assert.equal(request.signatures[0].signer, artifacts.signerPublicKey);
  assert.equal(request.network_id, NETWORK_ID.toString());
  assert.equal(request.owner, OWNER);
  assert.deepEqual(request.client_blob_id, [Array.from(expectedBlobId.values())]);
  assert.equal(request.payload, PAYLOAD.toString("base64"));
  assert.equal(request.total_size, PAYLOAD.length);
  assert.equal(request.chunk_size, chunkSize);
  assert.equal(request.lane_id, 5);
  assert.equal(request.epoch, 6);
  assert.equal(request.sequence, 7);
  assert.equal(request.codec[0], "text/plain");
  assert.deepEqual(request.blob_class, { class: "Custom", value: 99 });
  assert.deepEqual(request.erasure_profile, {
    data_shards: 2,
    parity_shards: 1,
    row_parity_stripes: 0,
    chunk_alignment: 4,
    fec_scheme: { scheme: "Custom", value: 7 },
  });
  assert.deepEqual(request.retention_policy, {
    hot_retention_secs: 120,
    cold_retention_secs: 240,
    required_replicas: 2,
    storage_class: { type: "Cold", value: null },
    governance_tag: ["da.custom"],
  });
  assert.equal(request.compression, "Zstd");
  assert.equal(request.norito_manifest, null);
  assert.deepEqual(request.metadata.items, [
    {
      key: "note",
      value: Buffer.from("hello-world").toString("base64"),
      visibility: { visibility: "Public", value: null },
      encryption: { cipher: "None", params: null },
    },
  ]);

  const tampered = structuredClone(request);
  tampered.erasure_profile.parity_shards = 2;
  assert.notDeepEqual(
    computeDaIngestSigningDigest(tampered),
    signingDigest,
    "resource-sensitive erasure fields must be signed",
  );
});

test("DA signing digest rejects pre-release requests with omitted V1 fields", () => {
  const { request } = buildDaIngestRequest({
    payload: PAYLOAD,
    networkId: NETWORK_ID,
    owner: OWNER,
    clientBlobId: CLIENT_BLOB_ID,
    signerPublicKey:
      "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245",
    signatureHex: "aa".repeat(64),
  });
  assert.equal(request.compression, "Identity");
  assert.equal(request.norito_manifest, null);

  for (const path of [
    ["compression"],
    ["norito_manifest"],
    ["erasure_profile", "row_parity_stripes"],
  ]) {
    const incomplete = structuredClone(request);
    const parent = path.slice(0, -1).reduce((value, key) => value[key], incomplete);
    delete parent[path.at(-1)];
    assert.throws(
      () => computeDaIngestSigningDigest(incomplete),
      /compression|norito_manifest|row_parity_stripes/u,
      `omitted ${path.join(".")} must reject`,
    );
  }
});

test("DA intent digest matches the shared Rust protocol vector", () => {
  const vectorPrivateKey = Buffer.alloc(32, 0x19);
  const vectorOwner = AccountAddress.fromAccount({
    publicKey: Buffer.from(ed25519.getPublicKey(vectorPrivateKey)),
  }).toI105();
  const vectorPayload = Buffer.from("hello data availability");
  const digest = computeDaIngestSigningDigest({
    network_id: NETWORK_ID.toString(),
    owner: vectorOwner,
    client_blob_id: [
      Array.from({ length: 32 }, (_, index) => (0x11 + index) & 0xff),
    ],
    lane_id: 2,
    epoch: 42,
    sequence: 7,
    blob_class: { class: "TaikaiSegment", value: null },
    codec: ["cmaf"],
    erasure_profile: {
      data_shards: 8,
      parity_shards: 4,
      row_parity_stripes: 2,
      chunk_alignment: 12,
      fec_scheme: { scheme: "Rs12_10", value: null },
    },
    retention_policy: {
      hot_retention_secs: 86_400,
      cold_retention_secs: 30 * 86_400,
      required_replicas: 4,
      storage_class: { type: "Hot", value: null },
      governance_tag: ["da.test"],
    },
    chunk_size: 1 << 20,
    total_size: 23,
    payload_hash: [Array.from(blake3(vectorPayload))],
    compression: "Identity",
    norito_manifest: Buffer.from([0xaa, 0xbb, 0xcc]).toString("base64"),
    payload: vectorPayload.toString("base64"),
    metadata: {
      items: [
        {
          key: "content_type",
          value: Buffer.from("video/mp4").toString("base64"),
          visibility: { visibility: "Public", value: null },
          encryption: { cipher: "None", params: null },
        },
      ],
    },
  });

  assert.equal(
    digest.toString("hex").toUpperCase(),
    "B97871DB051776138277C9000393FDC259910663A8C751D37BD054A0DA369DDA",
  );
});

test("generateDaProofSummary normalizes native output for JS callers", () => {
  const manifestBytes = Buffer.from([0x01, 0x02]);
  const payloadBytes = Buffer.from([0x03]);
  const nativeCalls = [];
  const rawSummary = createNativeProofSummary();
  const summary = generateDaProofSummary(manifestBytes, payloadBytes, {
    __nativeBinding: {
      daGenerateProofs(manifest, payload, options) {
        nativeCalls.push({ manifest, payload, options });
        return rawSummary;
      },
    },
    sampleCount: 3n,
    sampleSeed: 7n,
    leafIndexes: [1n, 3],
  });

  assert.equal(nativeCalls.length, 1);
  assert.deepEqual(nativeCalls[0].manifest, manifestBytes);
  assert.deepEqual(nativeCalls[0].payload, payloadBytes);
  assert.deepEqual(nativeCalls[0].options, {
    sample_count: 3,
    sample_seed: 7,
    leaf_indexes: [1, 3],
  });

  const proof = summary.proofs[0];
  assert.equal(typeof summary.leaf_count, "bigint");
  assert.equal(summary.blob_hash_hex, rawSummary.blob_hash_hex);
  assert.equal(
    Buffer.from(proof.leaf_bytes_b64, "base64").toString("utf8"),
    "leaf-bytes",
  );
  assert.equal(proof.leaf_bytes_b64, rawSummary.proofs[0].leaf_bytes_b64);
  assert.equal(typeof proof.payload_len, "bigint");
  assert.deepEqual(proof.segment_leaves_hex, rawSummary.proofs[0].segment_leaves_hex);
  assert.deepEqual(proof.chunk_segments_hex, rawSummary.proofs[0].chunk_segments_hex);
  assert.equal(proof.chunk_count, Number(rawSummary.proofs[0].chunk_count));
  assert.deepEqual(
    proof.chunk_merkle_path_hex,
    rawSummary.proofs[0].chunk_merkle_path_hex,
  );
  assert.equal(proof.verified, true);
});

test("generateDaProofSummary accepts array-like payloads", () => {
  const nativeCalls = [];
  const summary = generateDaProofSummary([1, 2], [3, 4], {
    __nativeBinding: {
      daGenerateProofs(manifest, payload, options) {
        nativeCalls.push({ manifest, payload, options });
        return createNativeProofSummary();
      },
    },
  });

  assert.equal(nativeCalls.length, 1);
  assert.deepEqual(Array.from(nativeCalls[0].manifest.values()), [1, 2]);
  assert.deepEqual(Array.from(nativeCalls[0].payload.values()), [3, 4]);
  assert.equal(summary.proofs.length, 1);
});

test("generateDaProofSummary rejects non-byte arrays", () => {
  assert.throws(
    () =>
      generateDaProofSummary([256], [1], {
        __nativeBinding: { daGenerateProofs: () => createNativeProofSummary() },
      }),
    (error) => error instanceof TypeError && /manifestBytes\[0\]/i.test(error.message),
  );
});

test("generateDaProofSummary rejects coercible non-byte array entries", () => {
  for (const entry of ["1", true, null]) {
    assert.throws(
      () =>
        generateDaProofSummary([entry], [1], {
          __nativeBinding: { daGenerateProofs: () => createNativeProofSummary() },
        }),
      (error) => error instanceof TypeError && /manifestBytes\[0\]/i.test(error.message),
    );
  }
});

test("emitDaProofSummaryArtifact writes JSON artifacts with normalized fields", async () => {
  const tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), "da-proof-"));
  const outputPath = path.join(tmpDir, "artifact.json");
  const summary = generateDaProofSummary(Buffer.from([0xaa]), Buffer.from([0xbb]), {
    __nativeBinding: { daGenerateProofs: () => createNativeProofSummary() },
  });

  const { artifact, outputPath: resolved, summary: returnedSummary } =
    await emitDaProofSummaryArtifact({
      summary,
      outputPath,
      manifestPath: "norito.manifest",
      payloadPath: "payload.bin",
    });

  try {
    assert.equal(returnedSummary, summary);
    assert.equal(resolved, path.resolve(outputPath));
    assert.equal(artifact.manifest_path, "norito.manifest");
    assert.equal(artifact.payload_path, "payload.bin");
    assert.equal(artifact.proofs[0].payload_len, createLargePayloadLength().toString());

    const onDisk = await fs.readFile(outputPath, "utf8");
    assert.ok(onDisk.endsWith("\n"));
    assert.deepEqual(JSON.parse(onDisk), artifact);
  } finally {
    await fs.rm(tmpDir, { recursive: true, force: true });
  }
});

function createLargePayloadLength() {
  return BigInt(Number.MAX_SAFE_INTEGER) + 10n;
}

function createNativeProofSummary() {
  const leafBytesB64 = Buffer.from("leaf-bytes").toString("base64");
  return {
    blob_hash_hex: "aa".repeat(32),
    chunk_root_hex: "bb".repeat(32),
    por_root_hex: "cc".repeat(32),
    leaf_count: BigInt(Number.MAX_SAFE_INTEGER) + 2n,
    segment_count: 1n,
    chunk_count: 2,
    sample_count: 3,
    sample_seed: 7n,
    proof_count: 1,
    proofs: [
      {
        origin: "Gateway",
        leaf_index: 4,
        chunk_index: 5,
        segment_index: 6,
        leaf_offset: 1n,
        leaf_length: 2,
        segment_offset: 3n,
        segment_length: 4,
        chunk_offset: 5n,
        chunk_length: 6,
        payload_len: createLargePayloadLength(),
        chunk_digest_hex: "dd".repeat(32),
        chunk_root_hex: "ee".repeat(32),
        segment_digest_hex: "ff".repeat(32),
        leaf_digest_hex: "11".repeat(32),
        leaf_bytes_b64: leafBytesB64,
        segment_leaves_hex: ["22".repeat(32)],
        chunk_segments_hex: ["33".repeat(32)],
        chunk_count: 2n,
        chunk_merkle_path_hex: ["44".repeat(32)],
        verified: true,
      },
    ],
  };
}
