import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

import {
  buildDaIngestRequest,
  emitDaProofSummaryArtifact,
  generateDaProofSummary,
} from "../src/dataAvailability.js";
import { signEd25519 } from "../src/crypto.js";

const PRIVATE_KEY = Buffer.alloc(32, 0x24);
const CLIENT_BLOB_ID = "11".repeat(32);
const PAYLOAD = Buffer.from("payload-for-da");

test("buildDaIngestRequest signs payloads and encodes DA fields", () => {
  const chunkSize = 1024;
  const { request, artifacts } = buildDaIngestRequest({
    payload: PAYLOAD,
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

  const expectedSignatureHex = signEd25519(PAYLOAD, PRIVATE_KEY)
    .toString("hex")
    .toUpperCase();
  const expectedBlobId = Buffer.from(CLIENT_BLOB_ID, "hex");

  assert.equal(request.signature, expectedSignatureHex);
  assert.equal(request.signature, artifacts.signatureHex);
  assert.equal(request.submitter, artifacts.submitterPublicKey);
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
  assert.deepEqual(request.metadata.items, [
    {
      key: "note",
      value: Buffer.from("hello-world").toString("base64"),
      visibility: { visibility: "Public", value: null },
      encryption: { cipher: "None", params: null },
    },
  ]);
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
  assert.deepEqual(proof.chunk_roots_hex, rawSummary.proofs[0].chunk_roots_hex);
  assert.equal(proof.verified, true);
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
        chunk_roots_hex: ["44".repeat(32)],
        verified: true,
      },
    ],
  };
}
