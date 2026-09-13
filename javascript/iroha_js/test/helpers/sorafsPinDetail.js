/** Native PinManifestFinalizedRecordV1 JSON fixture; identity supplied by the caller. */
export function sorafsPinDetailFixture(owner, digestHex = "ee".repeat(32)) {
  return {
    finalized_cursor: { height: 51, block_hash: Array(32).fill(0x42) },
    manifest: {
      digest: Array.from(Buffer.from(digestHex, "hex")),
      root_cid: [1, 0x71, 0x1f, 32, ...Array(32).fill(0x33)],
      chunker: { profile_id: 1, namespace: "sorafs", name: "sf1", semver: "1.0.0", multihash_code: 31 },
      chunk_digest_sha3_256: Array(32).fill(0x22),
      por_root: Array(32).fill(0x55),
      content_length: 4096,
      policy: { min_replicas: 3, storage_class: { type: "Hot", value: null }, retention_epoch: 100 },
      submitted_by: owner,
      submitted_epoch: 42,
      approved_epoch: 45,
      alias: { namespace: "docs", name: "main", proof: Buffer.from("pin-alias").toString("base64") },
      successor_of: Array(32).fill(0xff),
      metadata: { note: "demo" },
      status: { status: "Approved", value: 45 },
      council_envelope_digest: Array(32).fill(0x11),
    },
  };
}

/** Convert only native fixed-byte fields to the typed SDK readback representation. */
export function typedSorafsPinDetailFixture(value) {
  const result = structuredClone(value);
  result.finalized_cursor.block_hash = Uint8Array.from(result.finalized_cursor.block_hash);
  for (const field of ["digest", "root_cid", "chunk_digest_sha3_256", "por_root", "successor_of", "council_envelope_digest"]) {
    if (result.manifest[field] != null) result.manifest[field] = Uint8Array.from(result.manifest[field]);
  }
  return result;
}
