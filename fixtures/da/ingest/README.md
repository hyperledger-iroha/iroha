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
