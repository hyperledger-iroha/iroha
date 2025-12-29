import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import assert from "node:assert/strict";

import {
  deriveDaChunkerHandle,
  generateDaProofSummary,
  buildDaProofSummaryArtifact,
  emitDaProofSummaryArtifact,
} from "../src/dataAvailability.js";

function buildStubProofSummary() {
  return {
    blob_hash_hex: "aa",
    chunk_root_hex: "bb",
    por_root_hex: "cc",
    leaf_count: 4n,
    segment_count: 2n,
    chunk_count: 1n,
    sample_count: 5,
    sample_seed: 7n,
    proof_count: 1,
    proofs: [
      {
        origin: "sampled",
        leaf_index: 0,
        chunk_index: 0,
        segment_index: 0,
        leaf_offset: 0n,
        leaf_length: 32,
        segment_offset: 0n,
        segment_length: 256,
        chunk_offset: 0n,
        chunk_length: 512,
        payload_len: 1024n,
        chunk_digest_hex: "11",
        chunk_root_hex: "22",
        segment_digest_hex: "33",
        leaf_digest_hex: "44",
        leaf_bytes_b64: Buffer.from([1, 2, 3]).toString("base64"),
        segment_leaves_hex: ["aa"],
        chunk_segments_hex: ["bb"],
        chunk_roots_hex: ["cc"],
        verified: true,
      },
    ],
  };
}

test("generateDaProofSummary normalises options and transforms native payloads", () => {
  const manifestBytes = Buffer.from("manifest-fixture");
  const payloadBytes = Buffer.from("payload-fixture");
  const stubSummary = buildStubProofSummary();
  const stubBinding = {
    daGenerateProofs: (manifest, payload, options) => {
      assert.deepEqual(manifest, manifestBytes);
      assert.deepEqual(payload, payloadBytes);
      assert.deepEqual(options, {
        sample_count: 5,
        sample_seed: 7,
        leaf_indexes: [0, 1],
      });
      return stubSummary;
    },
  };

  const summary = generateDaProofSummary(manifestBytes, payloadBytes, {
    sampleCount: 5,
    sampleSeed: 7n,
    leafIndexes: [0, 1n],
    __nativeBinding: stubBinding,
  });

  assert.equal(summary.blobHashHex, "aa");
  assert.equal(summary.chunkRootHex, "bb");
  assert.equal(summary.leafCount, 4);
  assert.equal(summary.sampleSeed, 7);
  assert.equal(summary.sampleCount, 5);
  assert.equal(summary.proofCount, 1);
  assert.equal(summary.proofs.length, 1);
  assert.deepEqual(summary.proofs[0].segmentLeavesHex, ["aa"]);
  assert.equal(summary.proofs[0].leafBytes.length, 3);
  assert.equal(summary.proofs[0].leafBytesB64, Buffer.from([1, 2, 3]).toString("base64"));
  assert(summary.proofs[0].verified);
});

test("generateDaProofSummary validates empty inputs", () => {
  const stubBinding = {
    daGenerateProofs: () => {
      throw new Error("should not be called");
    },
  };
  assert.throws(
    () => generateDaProofSummary(Buffer.alloc(0), Buffer.from([1]), { __nativeBinding: stubBinding }),
    /manifestBytes must contain at least one byte/i,
  );
  assert.throws(
    () => generateDaProofSummary(Buffer.from([1]), Buffer.alloc(0), { __nativeBinding: stubBinding }),
    /payloadBytes must contain at least one byte/i,
  );
});

test("generateDaProofSummary rejects unsupported options", () => {
  const stubBinding = {
    daGenerateProofs: () => {
      throw new Error("should not be called");
    },
  };
  assert.throws(
    () =>
      generateDaProofSummary(Buffer.from("manifest"), Buffer.from("payload"), {
        sampleCount: 1,
        extra: true,
        __nativeBinding: stubBinding,
      }),
    /generateDaProofSummary options contains unsupported fields: extra/,
  );
});

test("generateDaProofSummary accepts snake_case proof options", () => {
  const manifestBytes = Buffer.from("manifest-fixture");
  const payloadBytes = Buffer.from("payload-fixture");
  const stubSummary = buildStubProofSummary();
  const stubBinding = {
    daGenerateProofs: (manifest, payload, options) => {
      assert.deepEqual(manifest, manifestBytes);
      assert.deepEqual(payload, payloadBytes);
      assert.deepEqual(options, {
        sample_count: 3,
        sample_seed: 9,
        leaf_indexes: [5],
      });
      return stubSummary;
    },
  };

  const summary = generateDaProofSummary(manifestBytes, payloadBytes, {
    sample_count: 3,
    sample_seed: 9,
    leaf_indexes: [5],
    __nativeBinding: stubBinding,
  });

  assert.equal(summary.sampleCount, 5);
  assert.equal(summary.sampleSeed, 7);
  assert.equal(summary.proofs.length, 1);
});

test("deriveDaChunkerHandle enforces supported options", () => {
  const manifestBytes = Buffer.from("manifest-bytes");
  const stubBinding = {
    daManifestChunkerHandle: (manifest) => {
      assert.deepEqual(manifest, manifestBytes);
      return ["chunker"];
    },
  };

  assert.deepEqual(
    deriveDaChunkerHandle(manifestBytes, { __nativeBinding: stubBinding }),
    ["chunker"],
  );

  assert.throws(
    () =>
      deriveDaChunkerHandle(manifestBytes, {
        __nativeBinding: stubBinding,
        extra: "nope",
      }),
    /deriveDaChunkerHandle options contains unsupported fields: extra/,
  );
});

test("buildDaProofSummaryArtifact produces Norito-aligned payload", () => {
  const summary = {
    blobHashHex: "AA",
    chunkRootHex: "BB",
    porRootHex: "CC",
    leafCount: 4n,
    segmentCount: 2,
    chunkCount: 1,
    sampleCount: 5,
    sampleSeed: 7n,
    proofCount: 1,
    proofs: [
      {
        origin: "explicit",
        leafIndex: 0,
        chunkIndex: 1,
        segmentIndex: 2,
        leafOffset: 0n,
        leafLength: 32,
        segmentOffset: 64n,
        segmentLength: 128,
        chunkOffset: 256n,
        chunkLength: 512,
        payloadLength: 1_024n,
        chunkDigestHex: "11",
        chunkRootHex: "22",
        segmentDigestHex: "33",
        leafDigestHex: "44",
        leafBytes: Buffer.from([9, 8, 7]),
        segmentLeavesHex: ["aa"],
        chunkSegmentsHex: ["bb"],
        chunkRootsHex: ["cc"],
        verified: true,
      },
    ],
  };
  const artifact = buildDaProofSummaryArtifact(summary, {
    manifestPath: "/tmp/manifest.norito",
    payloadPath: "/tmp/payload.car",
  });
  assert.equal(artifact.manifest_path, "/tmp/manifest.norito");
  assert.equal(artifact.payload_path, "/tmp/payload.car");
  assert.equal(artifact.blob_hash, "aa");
  assert.equal(artifact.chunk_root, "bb");
  assert.equal(artifact.por_root, "cc");
  assert.equal(artifact.proofs.length, 1);
  assert.equal(artifact.proofs[0].leaf_bytes_b64, Buffer.from([9, 8, 7]).toString("base64"));
  assert.equal(artifact.proofs[0].chunk_digest, "11");
  assert.equal(artifact.proofs[0].chunk_roots[0], "cc");
});

test("emitDaProofSummaryArtifact writes JSON artifacts", async () => {
  const summary = {
    blobHashHex: "DD",
    chunkRootHex: "EE",
    porRootHex: "FF",
    leafCount: 2,
    segmentCount: 1,
    chunkCount: 1,
    sampleCount: 1,
    sampleSeed: 1,
    proofCount: 1,
    proofs: [
      {
        origin: "sampled",
        leafIndex: 0,
        chunkIndex: 0,
        segmentIndex: 0,
        leafOffset: 0,
        leafLength: 32,
        segmentOffset: 0,
        segmentLength: 64,
        chunkOffset: 0,
        chunkLength: 64,
        payloadLength: 64,
        chunkDigestHex: "55",
        chunkRootHex: "66",
        segmentDigestHex: "77",
        leafDigestHex: "88",
        leafBytesB64: Buffer.from([1, 2]).toString("base64"),
        segmentLeavesHex: [],
        chunkSegmentsHex: [],
        chunkRootsHex: [],
        verified: true,
      },
    ],
  };
  const tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), "iroha-js-da-"));
  const outputPath = path.join(tmpDir, "proof.json");
  const result = await emitDaProofSummaryArtifact({
    summary,
    manifestPath: "./manifest.to",
    payloadPath: "./payload.car",
    outputPath,
    pretty: false,
  });
  assert.equal(result.outputPath, outputPath);
  const written = await fs.readFile(outputPath, "utf8");
  const parsed = JSON.parse(written);
  assert.equal(parsed.blob_hash, "dd");
  assert.equal(parsed.manifest_path, "./manifest.to");
  assert.equal(parsed.proof_count, 1);
});
