# DA-2 Golden Fixtures

These fixtures capture the canonical outputs of the DA ingest pipeline for the
`sample_request()` payload used in `crates/iroha_torii/src/da/tests.rs` tests:

- `manifests/<blob_class>/manifest.norito.hex` – hex-encoded Norito
  `DaManifestV1` bytes persisted by `/v1/da/ingest` after chunking and
  retention-policy enforcement. The four fixture sets cover the currently
  supported blob classes (`taikai_segment`, `nexus_lane_sidecar`,
  `governance_artifact`, and `custom_0042`) so adding a new class requires
  updating the fixtures alongside the enum.
- `manifests/<blob_class>/manifest.json` – Norito JSON rendering of the same
  manifest to aid SDK teams that need a human-readable reference.
- `sample_chunk_records.txt` – deterministic list of chunk files emitted when
  chunk metadata is streamed via `ChunkStore::ingest_plan_stream_to_directory`.

The fixtures are referenced by `streaming_chunk_ingest_matches_fixture` and
`manifest_persistence_matches_fixture` to ensure end-to-end ingest behaviour
remains reproducible. Update them by running the ignored test
`cargo test -p iroha_torii regenerate_da_ingest_fixtures -- --ignored --nocapture`.

The shared Rust, JavaScript and Swift producer tests also read
`request_signing_v1.json`. Its identity and RFC 1950 DEFLATE cases sign the
same canonical payload using different transport bytes. Each case fixes the
request-content commitment (the BLAKE3 content digest as an Iroha hash, with
its final bit set), the outer authorization digest, the canonical public-key
literal and the Ed25519 signature.

`pin_intent_v1.json` is a signed stored `DaPinIntent` in the frozen baseline
layout: its scope carries no commitment digest, and `request_content_hash` is a
hash literal. The producer's scope witness signs the scope together with the
digest of its paired commitment record, `BlobDigest([0x44; 32])` for this
fixture; `signed_pin_intent_fixture_authenticates_both_domains_and_round_trips`
verifies it against that digest.
