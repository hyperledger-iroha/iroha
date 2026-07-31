# SoraFS Chunking → Manifest Pipeline

This note captures the minimum steps required to transform a byte payload into
a Norito-encoded manifest suitable for pinning in the SoraFS registry.

1. **Chunk the payload deterministically**
   - Use `sorafs_car::CarBuildPlan::single_file` (internally uses the SF-1
     chunker) to derive chunk offsets, lengths, and BLAKE3 digests.
   - The plan exposes the payload digest and chunk metadata that downstream
     tooling can reuse for CAR assembly and Proof-of-Replication scheduling.
   - Alternatively, the canonical `sorafs_car::ChunkStore` ingests bytes and
     records deterministic chunk metadata for later CAR construction.
     The store now derives the 64 KiB / 4 KiB PoR sampling tree (domain-tagged,
     chunk-aligned) so schedulers can request Merkle proofs without re-reading
     the payload.
    Use `--por-proof=<chunk>:<segment>:<leaf>` to emit a JSON witness for a
    sampled leaf and `--por-json-out` to write the root digest snapshot for
    later verification. Pair `--por-proof` with `--por-proof-out=path` to persist
    the witness, and use `--por-proof-verify=path` to confirm an existing proof
    matches the computed `por_root_hex` for the current payload. For multiple
    leaves, `--por-sample=<count>` (with optional `--por-sample-seed` and
    `--por-sample-out`) produces deterministic samples while flagging
    `por_samples_truncated=true` whenever the request exceeds the available
    leaves.
   - Persist the chunk offsets/lengths/digests if you intend to build bundle
     proofs (CAR manifests, PoR schedules).
   - Refer to [`sorafs/chunker_registry.md`](chunker_registry.md) for the
     canonical registry entries and negotiation guidance.

2. **Wrap a manifest**
   - Feed the chunking metadata, ordered chunk-plan SHA3-256 commitment, root
     CID, CAR commitments, pin policy, alias claims, and governance signatures
     into `sorafs_manifest::ManifestBuilder`.
   - Call `ManifestV1::encode` to obtain the Norito bytes and
     `ManifestV1::digest` to obtain the canonical digest recorded in the Pin
     Registry.

3. **Publish**
   - Submit the manifest digest through governance (council signature, alias
     proofs) and pin the manifest bytes to SoraFS using the deterministic
     pipeline.
   - Ensure the CAR file (and optional CAR index) referenced by the manifest is
     stored in the same SoraFS pin set.

### CLI quickstart

```bash
cargo run -p sorafs_car --bin sorafs_manifest_builder \
  ./docs.tar \
  --root-cid=0155aa \
  --car-cid=017112... \
  --alias-file=docs:sora:alias_proof.bin \
  --council-signature-file=0123...cafe:council.sig \
  --metadata=build:ci-123 \
  --manifest-out=docs.manifest \
  --manifest-signatures-out=docs.manifest.signatures.json \
  --car-out=docs.car \
  --json-out=docs.report.json
```

The command prints chunk digests and manifest details; when `--manifest-out`
and/or `--car-out` are supplied it writes the Norito payload and a
spec-compliant CARv2 archive (pragma + header + CARv1 blocks + Multihash index)
to disk. If you pass a directory path, the tool walks it recursively (lexicographic
order), chunks every file, and emits a directory-rooted dag-cbor tree whose CID
appears as both the manifest and CAR root. The JSON report includes the computed
CAR payload digest, full archive digest, size, raw CID, and root (splitting out
the dag-cbor codec), along with the manifest's alias/metadata entries. Use
`--root-cid`/`--dag-codec` to *verify* the computed root or codec during CI
runs, `--car-digest` to enforce the BLAKE3-256 digest of the complete canonical
CARv2 archive (pragma, header, embedded CARv1 payload, and index), `--car-cid`
to enforce a precomputed raw (CIDv1, `raw` codec, BLAKE3 multihash) CAR
identifier, and `--json-out` to persist the printed JSON alongside the
manifest/CAR artifacts for downstream automation. The report's explicit
top-level `payload_digest_hex` is the raw-payload commitment; consumers never
substitute `manifest.car_digest_hex` for it.

When `--manifest-signatures-out` is provided (alongside at least one
`--council-signature*` flag) the tool also writes a
`manifest_signatures.json` envelope containing the manifest BLAKE3 digest, the
aggregated chunk plan SHA3-256 digest (offsets, lengths, and chunk BLAKE3
digests), and the supplied council
signatures. The envelope now records the chunker profile in canonical
`namespace.name@semver` form. First-release admission accepts only the canonical
form and canonical lowercase signature encodings. Distribute the envelope with
the manifest and CAR artifacts. When you receive an envelope from an external
signer, add `--manifest-signatures-in=<path>` to have the CLI confirm the digests
and verify each Ed25519 signature against the freshly computed manifest digest.

For controlled local fixture generation, `--council-signing-key-file=<path>`
accepts exactly one raw 32-byte Ed25519 seed and signs the canonical unsigned
manifest digest before encoding the completed manifest. This file-only option
is mutually exclusive with every detached council-signature/public-key input;
the CLI rejects symlinked files or parents, non-regular files, hard links,
non-exact lengths, and any Unix mode that grants group or other access. The
completed signature is verified before any manifest or report is written.
Keep real council keys outside the repository and pass them only through a
mode-`0600` runtime file. The Android codegen replay uses a documented public
TEST-ONLY seed created inside its private temporary directory and deletes it
with that directory; it is never included in generated reports or fixtures.

When submitting through Torii, pass the exact canonical Norito-encoded
`ManifestV1` as base64 in `manifest_payload` on
`POST /v1/sorafs/pin/register` to run the same full manifest
validator before the registration transaction is queued. Torii derives the
manifest digest, chunk-plan digest, root CID, chunker descriptor, content
length, and pin policy from those bytes; the request has no parallel summary
fields that can disagree. The payload must use the single canonical first-release Norito
layout and a positive retention epoch later than submission; legacy flag
layouts, inert commitments, and inline chunker profiles fail before queueing.
`manifest_payload` is mandatory in the first release. Torii places those exact
canonical bytes in `RegisterPinManifest`, and consensus decodes and revalidates
the full manifest before deriving its digest, chunk-plan digest, root CID,
chunker, content length, and pin policy. Direct instruction submission therefore cannot bypass Torii's
manifest checks. When governance requires council signatures, registration
remains pending, without a published alias
or replication order, until a `CanApproveSorafsPin` authority submits the actual
bounded council envelope through `ApprovePinManifest`. A digest without its
envelope is never sufficient for the first approval, and a later replay cannot
replace the stored envelope commitment. Approval must occur before retention
expiry, and alias publication waits until every fallible automatic-order step
has succeeded. Public registrations may include an alias only when the
submitting authority carries `CanBindSorafsAlias`; its canonical bounded proof
must commit to the exact content root plus submission and retention epochs.

The first release accepts only the exact 36-byte binary root CID layout
`CIDv1 | dag-cbor | BLAKE3-256 | 32-byte digest`; non-canonical varints, other
codecs or hash codes, zero digests, truncation, and trailing bytes are rejected.

When multiple chunker profiles are registered you can select one explicitly
with `--chunker-profile-id=<id>`. The flag maps to the numeric identifiers in
[`chunker_registry`](chunker_registry.md) and ensures both the chunking pass and
the emitted manifest reference the same `(namespace, name, semver)` tuple.
Prefer the canonical handle form in automation (`--chunker-profile=sorafs.sf1@1.0.0`),
which avoids hard-coding numeric IDs. Run `sorafs_manifest_chunk_store --list-profiles`
to view the current registry entries (the output mirrors the listing provided
by `sorafs_manifest_chunk_store`), or use `--promote-profile=<handle>` to export
the canonical handle and alias metadata when preparing a registry update.

Auditors can request the full Proof-of-Retrievability tree via
`--por-json-out=path`, which serialises chunk/segment/leaf digests for sampling
verification. Individual witnesses can be exported with
`--por-proof=<chunk>:<segment>:<leaf>` (and validated with
`--por-proof-verify=path`), while `--por-sample=<count>` draws deterministic,
de-duplicated samples for spot checks.

Any flag that writes JSON (`--json-out`, `--chunk-fetch-plan-out`, `--por-json-out`,
etc.) also accepts `-` as the path, allowing you to stream the payload directly
to stdout without creating temporary files.

Use `--chunk-fetch-plan-out=path` to persist the ordered chunk fetch specification
(chunk index, payload offset, length, BLAKE3 digest) that accompanies the manifest
plan. The resulting standalone JSON is a strict
`sorafs.chunk_fetch_plan.v1` object that also commits the non-zero BLAKE3 digest
of the complete payload. Multi-source clients can feed it directly into the
SoraFS fetch orchestrator without re-reading the source payload. The versioned
manifest-builder report also embeds the ordered array under
`chunk_fetch_specs`; that field is not a standalone interchange format.
Both the `chunking` section and `manifest` object expose `profile_aliases`

When re-running the builder (for example in CI or a release pipeline) you can pass
`--plan=chunk_fetch_plan.json` or `--plan=-` to import the previously generated
envelope. The CLI verifies its whole-payload binding and each chunk’s index,
offset, length, and BLAKE3 digest against the freshly derived CAR plan before
ingestion continues. Retired bare arrays are rejected.

### Local orchestration smoke-test

The `sorafs_car` crate now ships `sorafs-fetch`, a developer CLI that consumes the
strict payload-bound V1 envelope and simulates multi-provider retrieval from
local files. Point it at the JSON emitted by `--chunk-fetch-plan-out`, provide one or more
provider payload paths (optionally with `#N` to increase concurrency), and it will
verify chunks, reassemble the payload, and print a JSON report summarising provider
success/failure counts and per-chunk receipts:

```
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=chunk_fetch_plan.json \
  --provider=alpha=./providers/alpha.bin \
  --provider=beta=./providers/beta.bin#4@3 \
  --output=assembled.bin \
  --json-out=fetch_report.json \
  --provider-metrics-out=providers.json \
  --scoreboard-out=scoreboard.json
```

Use this flow to validate orchestrator behaviour or to compare provider payloads
before wiring actual network transports into the SoraFS node.

When you need to hit a live Torii gateway instead of local files, swap the
`--provider=/path` flags for the new HTTP-oriented options:

```
sorafs-fetch \
  --plan=chunk_fetch_plan.json \
  --gateway-provider=name=gw-a,provider-id=<hex>,gateway-key=<ed25519-public-key-hex>,base-url=https://gw-a.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --gateway-client-id=ci-orchestrator \
  --json-out=gateway_fetch_report.json
```

The CLI verifies the stream token against the separately supplied gateway key,
enforces its lifetime and provider/chunker/manifest bindings, and
records the gateway metadata alongside the usual provider receipts so operators
can archive the report as rollout evidence (see the deployment handbook for the
full blue/green flow).

If you pass `--provider-advert=name=/path/to/advert.to`, the CLI now decodes the
Norito envelope, verifies the Ed25519 signature, and enforces that the provider
advertises the `chunk_range_fetch` capability. This keeps the multi-source fetch
simulation aligned with the governance admission policy and prevents accidental

The `#N` suffix increases the provider’s concurrency limit, while `@W` sets its
scheduling weight (defaults to 1 when omitted). When adverts or gateway
descriptors are supplied, the CLI now evaluates the orchestrator scoreboard
before launching a fetch: eligible providers inherit telemetry-aware weights and
the JSON snapshot persists to `--scoreboard-out=<path>` when provided. Providers
that fail capability checks or governance deadlines are dropped automatically
with a warning so runs stay aligned with the admission policy. See
`fixtures/documentation/sorafs_ci_sample/{telemetry.sample.json,scoreboard.json}` for a
sample input/output pair.

Pass `--expect-payload-digest=<hex>` and/or `--expect-payload-len=<bytes>` to
assert that the assembled payload matches manifest expectations before writing
outputs—handy for CI smoke-tests that want to ensure the orchestrator did not
silently drop or reorder chunks.

If you already have the JSON report created by `sorafs_manifest_builder`, pass it
directly via `--manifest-report=docs.report.json`. The fetch CLI will re-use the
embedded `chunk_fetch_specs`, `payload_digest_hex`, and `payload_len` fields so
you don’t need to manage separate plan or validation files.

The fetch report also surfaces aggregated telemetry to aid monitoring:
`chunk_retry_total`, `chunk_retry_rate`, `chunk_attempt_total`,
`chunk_attempt_average`, `provider_success_total`, `provider_failure_total`,
`provider_failure_rate`, and `provider_disabled_total` capture the overall
health of a fetch session and are suitable for Grafana/Loki dashboards or CI
assertions. Use `--provider-metrics-out` to write just the `provider_reports`
array if downstream tooling only needs provider-level stats.

### Next steps

- Capture the CAR metadata alongside manifest digests in governance logs so
  observers can verify CAR contents without re-downloading the payload.
- Integrate the manifest and CAR publishing flow into CI so every docs/artifact
  build automatically produces a manifest, gains signatures, and pins the
  resulting payloads.
