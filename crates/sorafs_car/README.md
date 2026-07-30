# SoraFS CAR planning crate

Utility crate that produces deterministic CARv2 build plans from raw payloads
and can emit spec-compliant CAR archives (pragma + header + CARv1 payload +
MultihashIndexSorted index).

Right now it exposes:

- `CarBuildPlan::single_file[_with_profile]` to derive chunk offsets/lengths
  and BLAKE3 digests for a single payload. The `_with_profile` variant lets you
  substitute an alternate `ChunkProfile` without touching the default registry.
- `CarBuildPlan::from_files[_with_profile]` to plan multi-file directory trees
  (paths are provided as UTF-8 components) and return the concatenated payload
  bytes.
- `CarBuildPlan::from_directory[_with_profile]` to walk a real filesystem tree
  (lexicographic order, UTF-8 paths) and emit the same plan/payload pair in one
  call.
- `CarWriter::write_to` which emits a CARv2 container with:
  - the canonical 11-byte pragma and 40-byte header,
  - a CARv1 payload (header + chunk blocks encoded as raw multicodec nodes),
  - a MultihashIndexSorted (`0x0401`) index keyed by BLAKE3 digests, and
  - the overall CAR size and BLAKE3 digest (across the entire file).
  The writer derives the balanced dag-cbor file root automatically; callers
  can use `CarWriter::with_expected_roots` to assert that the computed root
  list matches an expected value before bytes are written. `CarWriteStats`
  includes the `ChunkProfile` used, making it easy for callers to cross-check
  registry expectations.
- `CarStreamingWriter::write_from_reader` which accepts any `Read`
  implementor (files, pipes, network streams) and emits the same CARv2
  output without buffering the entire payload. Chunks are hashed as they
  stream past, so digest mismatches are detected immediately.
- `ChunkStore` for transactional chunk verification plus canonical PoR metadata.
  `ingest_bytes`, plan-based ingestion, proof construction, and deterministic
  sampling all return structured errors; allocation, source-I/O, and digest
  failures are never represented as a successful empty result.

## First-release safety contract

- Every externally supplied `CarBuildPlan` is validated before payload I/O or
  plan-sized allocation. Validation enforces contiguous exact chunk/file
  coverage, the production 4 MiB chunk ceiling, at most 4,194,304 chunks and
  1,000,000 files, portable logical paths, checked arithmetic, and the configured
  heap ceiling (512 MiB by default).
- Empty payloads have one canonical representation: zero chunks, an empty
  BLAKE3 payload digest, and the zero PoR root. Empty logical files own no
  synthetic chunk.
- `CarBuildPlan::from_directory` is fail-closed outside Unix. On Unix it uses
  no-follow handles and file-identity snapshots, rejects links and special
  files, detects replacement/growth/truncation during the scan, and bounds
  depth, entry count, path length, and eager payload bytes before allocation.
- `DirectoryChunkSink` publishes only into an absent destination below an
  existing private canonical parent. It verifies chunk order, metadata,
  lengths, and digests in same-filesystem staging, re-reads every staged chunk,
  then atomically renames it. Its result distinguishes durable publication from
  publication whose post-rename durability could not be confirmed; callers must
  not retry the latter as though nothing was published.
- PoR constructors validate exact chunk/segment/leaf geometry and every stored
  commitment. Proof builders re-check source leaf bytes, proof verification is
  allocation-free, and multi-proof sampling is rejected before payload I/O when
  its conservative aggregate heap estimate exceeds the store limit.
- Production fetch paths use `CarBuildPlan::try_chunk_fetch_specs`; malformed
  plans or allocation failures remain errors rather than masquerading as an
  empty fetch plan. Raw spec arrays are only emitted as typed fields inside a
  recognized versioned report; they are never accepted as standalone plans.

The companion CLI `sorafs_manifest_chunk_store` ingests payloads with the deterministic
chunker, emits a CAR plan report, and supports `--list-profiles` along with
either `--profile-id=<id>` or the canonical handle form
`--profile=<namespace.name@semver>` for tooling that wants to introspect the
registry. The CLI
is shipped from the `sorafs_car` crate so CAR, fetch, and chunk-store tooling
stay in lockstep. Example:

```
cargo run -p sorafs_car --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf1@1.0.0 \
  --json-out=chunk_report.json \
  payload.tar
```

## CLI examples

Ingest a payload, emit a CAR plan report, and persist both the main summary,
the PoR tree, and sampled proofs in JSON:

```
cargo run -p sorafs_car --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf1@1.0.0 \
  --json-out=chunk_report.json \
  --chunk-dir-out=chunk_payloads \
  --por-json-out=por_tree.json \
  --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json \
  payload.tar

# `--profile-id=1` is accepted, but prefer the handle form so automation remains
# stable if registry IDs change.
```

- Pass `--chunk-fetch-plan-out=path` to persist the canonical
  `sorafs.chunk_fetch_plan.v1` envelope. It binds the ordered chunk
  specifications to the non-zero BLAKE3 digest of the complete payload so other
  tools (or `sorafs_fetch`) can reuse the plan without invoking the manifest
  builder.
- Pass `--chunk-dir-out=dir` to persist deterministic `chunk_00000.bin` payload
  files through the same disk-backed chunk sink used by `ChunkStore`; the target
  directory must be absent, and the JSON report includes a
  `persisted_chunks` record with file names, offsets, lengths, and BLAKE3
  digests.

List the registered chunker profiles:

```
cargo run -p sorafs_car --bin sorafs_manifest_chunk_store -- --list-profiles
```

Reassemble a payload from multiple local providers using the multi-source fetch
orchestrator. The standalone plan must be the strict
`sorafs.chunk_fetch_plan.v1` object emitted by
`sorafs_manifest_builder --chunk-fetch-plan-out`; retired bare arrays are
rejected:

```
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=chunk_fetch_plan.json \
  --provider=alpha=/srv/providers/alpha/payload.bin \
  --provider=beta=/srv/providers/beta/payload.bin#4@3 \
  --output=assembled.bin \
  --car-out=payload.car \
  --json-out=fetch_report.json
```

- `#N` suffix raises the provider’s in-flight concurrency to `N` chunk requests.
- `@W` suffix assigns a scheduling weight (defaults to 1) so higher priority
  providers can be favoured when slots are available.
- Add `--expect-payload-digest=<hex>` and/or `--expect-payload-len=<bytes>` to
  verify the reconstructed payload against manifest expectations during the run.
- `--manifest-report=report.json` lets the CLI consume the JSON emitted by
  `sorafs_manifest_builder`; it will reuse `chunk_fetch_specs`, `payload_digest_hex`,
  and `payload_len` so you don’t have to pass those values manually. The
  whole-payload `payload_digest_hex` is mandatory: the fetch path never treats
  `manifest.car_digest_hex` as a fallback because that field commits the entire
  canonical CARv2 archive.
- `--manifest=manifest.to` feeds the Norito-encoded manifest to the CLI so it can
  invoke the trustless `CarVerifier` after assembly and attach verification
  results to the JSON report.
- When `--output` is supplied the CLI streams verified chunks directly to disk
  while downloads are still in flight. A final byte-count and BLAKE3 digest
  check guarantees the streamed file matches the assembled payload before the
  run succeeds.
- The JSON report includes aggregated telemetry fields such as
  `chunk_retry_total`, `chunk_retry_rate`, `chunk_attempt_total`,
  `chunk_attempt_average`, `provider_success_total`, `provider_failure_total`,
  `provider_failure_rate`, and `provider_disabled_total` to feed dashboards or
  smoke-test assertions.
- `--provider-metrics-out=path` writes just the `provider_reports` array to a
  separate JSON file so scripts can ingest provider health without walking the
  full report payload.
- `--provider-advert=name=/path/to/advert.norito` loads metadata (availability,
  stake, max streams, rendezvous topics) from a signed provider advert and uses
  it to tune scheduling weights, concurrency, and the JSON report.
- `--car-out=path` streams the verified payload into a CARv2 archive using the
  supplied chunk plan so you can stage CAR artefacts without re-reading the
  payload later.

## Validation

Focused library and CLI suites cover canonical CAR layout, plan-bound streaming,
empty/multi-file datasets, hostile path and filesystem races, allocation/count
overflow, corrupted chunk and proof data, incomplete or reordered sinks, and
post-rename durability ambiguity. Run `cargo test -p sorafs_car` for the crate
suite and `cargo clippy -p sorafs_car --all-targets -- -D warnings` for strict
linting once shared workspace dependencies are stable.
