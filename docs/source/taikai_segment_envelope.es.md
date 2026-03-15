---
lang: es
direction: ltr
source: docs/source/taikai_segment_envelope.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1fdf7a2acebae39348bc30870bb7c9661216388324d00244a0ba75fc3d22722d
source_last_modified: "2026-01-22T15:38:30.727071+00:00"
translation_last_reviewed: 2026-01-30
---

# Taikai Segment Envelope V1

_Status: Draft update 2026-03-24 (TRM/SSM spec minted)_ — Owners: Media Platform WG / DA Program / Networking TL  
Related roadmap item: **SNNet-13 Taikai mass-broadcast platform**

The Taikai Segment Envelope (TSE) is the canonical metadata record that binds a
live broadcast segment to its deterministic data availability artefacts and
playback descriptors. Publishers emit one envelope per CMAF fragment (per
rendition) once the ingest service has persisted the payload into SoraFS and
computed the manifest commitments. Downstream consumers (gateways, SDKs,
analytics) rely on the envelope to stitch manifests, CAR archives, and
instrumentation without re-deriving ingest metadata.

The V1 envelope is implemented in `iroha_data_model::taikai` as
`TaikaiSegmentEnvelopeV1`, with supporting types exposed for event identifiers,
track metadata, codecs, and CAR pointers. All fields are encoded via Norito and
carry schema metadata for deterministic decoding across Rust, Swift, and JS
tooling.

## Field Breakdown

| Field | Type | Description |
| --- | --- | --- |
| `version` | `u16` | Envelope format version. V1 hard-codes this to `1`. |
| `event_id` | `TaikaiEventId` | Stable identifier for the broadcast (conference, launch, etc.). Backed by the canonical `Name` type so governance naming rules apply. |
| `stream_id` | `TaikaiStreamId` | Logical feed within the event (e.g., `stage-a`, `backstage`, `sign-language`). |
| `rendition_id` | `TaikaiRenditionId` | CMAF ladder rung; one envelope is emitted for every rendition of the segment. |
| `track` | `TaikaiTrackMetadata` | Codec + bitrate metadata. `TaikaiCodec` enumerates governance-approved codecs (`AvcHigh`, `HevcMain10`, `Av1Main`, `AacLc`, `Opus`) with a `Custom(String)` escape hatch. Optional resolution (`TaikaiResolution`) and channel layout (`TaikaiAudioLayout`) fields capture video and audio specifics. |
| `segment_sequence` | `u64` | Monotonic counter per rendition. Used for CAR index ordering and loss detection. |
| `segment_start_pts` | `SegmentTimestamp` | Presentation timestamp offset (microseconds since stream origin). Drives time-based indexes and player seeking. |
| `segment_duration` | `SegmentDuration` | Presentation duration (microseconds). Ensures manifests and CMAF chunk boundaries align. |
| `wallclock_unix_ms` | `u64` | Unix timestamp (milliseconds) when the segment left the encoder, establishing cross-region ordering for analytics. |
| `ingest` | `TaikaiIngestPointer` | Links to deterministic ingest artefacts: manifest hash (`BlobDigest`), storage ticket (`StorageTicketId`), chunk Merkle root (`BlobDigest`), chunk count, and the CAR pointer (`TaikaiCarPointer`). |
| `instrumentation` | `TaikaiInstrumentation` | Optional telemetry hints (encoder→ingest latency, ingest node identifier). |
| `metadata` | `ExtraMetadata` | Open metadata channel with governance-aware visibility controls, shared with other DA artefacts. |

### CAR Pointer

`TaikaiCarPointer` records the multibase DAG-CBOR CID (`cid_multibase`), the
canonical CARv2 file digest (`car_digest`, BLAKE3-256), and the concatenated
archive length in bytes (`car_size_bytes`). The ingest pipeline writes these
values once the manifest is sealed, ensuring every consumer derives identical
CAR commitments even when byte-streaming the archive.

### Instrumentation

`TaikaiInstrumentation` starts with two optional hints:
- `encoder_to_ingest_latency_ms`: duration between encoder output and Torii
  ingest acceptance.
- `ingest_node_id`: identifier for the ingest worker that sealed the segment.

Additional metrics can extend the struct in future revisions without breaking

## Indices and Determinism

The ingest service will emit two canonical indexes backed by the envelope:

1. **Time index** — keyed by `(event_id, stream_id, rendition_id,
   segment_start_pts)` to drive fast range scans when a player resumes at `t`.
2. **CID index** — keyed by `(event_id, stream_id, rendition_id,
   ingest.car.cid_multibase)` for deterministic CAR lookups and provenance
   auditing.

Both indexes use the envelope payload verbatim (no recomputed hashes) so every
consumer observes identical ordering and digest bindings.

Helper types in `iroha_data_model::taikai` capture these keys explicitly:

- `TaikaiTimeIndexKey` stores the time-ordered tuple.
- `TaikaiCidIndexKey` stores the CAR lookup tuple.
- `TaikaiEnvelopeIndexes` bundles both keys.

`TaikaiSegmentEnvelopeV1::indexes()` returns the bundle so ingest code can
persist the indices without reassembling tuples manually.

### Required Ingest Metadata

`/v2/da/ingest` expects Taikai uploads to provide the following metadata keys
(all values encoded as UTF-8 strings inside `ExtraMetadata`):

| Key | Value | Notes |
| --- | --- | --- |
| `taikai.event_id` | Event name (`Name` rules apply) | Required |
| `taikai.stream_id` | Stream identifier (`Name`) | Required |
| `taikai.rendition_id` | Rendition identifier (`Name`) | Required |
| `taikai.track.kind` | `video`, `audio`, or `data` | Required |
| `taikai.track.codec` | `avc-high`, `hevc-main10`, `av1-main`, `aac-lc`, `opus`, or `custom:<name>` | Required |
| `taikai.track.bitrate_kbps` | Average bitrate in kilobits per second | Required |
| `taikai.track.resolution` | `WIDTHxHEIGHT` (video only) | Required for video |
| `taikai.track.audio_layout` | `mono`, `stereo`, `5.1`, `7.1`, or `custom:<channels>` (audio only) | Required for audio |
| `taikai.segment.sequence` | Monotonic segment index (per rendition) | Required |
| `taikai.segment.start_pts` | Start PTS in microseconds | Required |
| `taikai.segment.duration` | Duration in microseconds | Required |
| `taikai.wallclock_unix_ms` | Finalisation wall-clock (Unix ms) | Required |
| `taikai.instrumentation.ingest_latency_ms` | Encoder→ingest latency (ms) | Optional |
| `taikai.instrumentation.live_edge_drift_ms` | Live-edge drift vs ingest arrival (ms; negative = ahead) | Optional |
| `taikai.instrumentation.ingest_node_id` | Ingest worker identifier | Optional |

The CLI bundler can emit this JSON map via `--ingest-metadata-out`, ensuring the
exact key spellings required by Torii.

Prometheus exports `taikai_ingest_live_edge_drift_signed_ms` to retain the signed
drift reported in these fields. The histogram `taikai_ingest_live_edge_drift_ms`
continues to track absolute magnitudes for latency bucketing.

## Encoding Rules

- Norito encoding is mandatory (`application/norito+v1`). JSON helpers are
  provided for SDKs via the `json` feature flag but must not be used for on-wire
  payloads.
- `SegmentTimestamp` and `SegmentDuration` are microseconds. Tooling must avoid
  floating-point conversions to preserve determinism.
- `TaikaiTrackMetadata::average_bitrate_kbps` records the encoder target rate;
  rate-control telemetry belongs in the instrumentation struct.
- The `metadata` field reuses the DA `ExtraMetadata` container, so existing
  governance-only encryption semantics apply.

## Producer Responsibilities

1. Complete DA ingest for the segment and obtain the canonical `DaManifestV1`
   hash + storage ticket.
2. Compute the CAR digest/CID using `sorafs_car::CarWriter` and populate
   `TaikaiCarPointer`.
3. Populate the envelope fields and sign or attest as per GAR v2 policy (to be
   finalised alongside SNNet-9).
4. Persist the envelope in the publisher log, enqueue it for the SoraNS anchor
   service, and push it to downstream SDKs.
5. Include the metadata keys above when submitting the CMAF payload to
   `/v2/da/ingest`. The bundler's `--ingest-metadata-out` file can be fed into
   the ingest client as-is.

## Bundler CLI

`iroha_cli` now ships a deterministic bundler for Taikai segments:

```shell
iroha app taikai bundle \
  --payload segment_0042.m4s \
  --car-out segment_0042.car \
  --envelope-out segment_0042.to \
  --indexes-out segment_0042.indexes.json \
  --ingest-metadata-out segment_0042.da.json \
  --event-id "global-keynote" \
  --stream-id "stage-a" \
  --rendition-id "1080p" \
  --track-kind video \
  --codec av1-main \
  --bitrate-kbps 8000 \
  --resolution 1920x1080 \
  --segment-sequence 42 \
  --segment-start-pts 3600000 \
  --segment-duration 2000000 \
  --wallclock-unix-ms 1702560000000 \
  --manifest-hash <64-hex> \
  --storage-ticket <64-hex>
```

The command:

- Plans and writes a CARv2 archive (pragma, header, payload, index).
- Computes the CAR digest/CID and fills a `TaikaiCarPointer`.
- Derives chunk count and PoR chunk root from the payload before populating
  `TaikaiIngestPointer`.
- Emits a Norito-encoded envelope to `--envelope-out`.
- Optionally renders the JSON `TaikaiEnvelopeIndexes` bundle to `--indexes-out`.
- Optionally renders the ingest metadata map to `--ingest-metadata-out`, matching
  the keys documented above so the `/v2/da/ingest` request can reuse it.
- When using the standalone `taikai_car` binary you can also pass
  `--summary-out <path>` to capture a JSON summary (ingest fields, track
  metadata, CAR/Chunk digests, time/CID indexes, and output paths) for
  automation bundles.
- Use `--car-in <path>` in place of `--payload` to rehydrate metadata from an
  existing CAR archive without re-emitting the payload. Pair it with
  `--summary-in <summary.json>` (a `--summary-out` file, an array entry, or an
  NDJSON line from the ingest watcher) to seed ingest/track fields; `--summary-entry`
  selects the array or NDJSON index when multiple rows are present.

Instrumentation knobs (`--ingest-latency-ms`, `--ingest-node-id`) and the
`--metadata-json` flag let publishers attach telemetry or metadata without
recompiling the payload.

The `sorafs_cli` packaging helper mirrors the bundler so operators that already
depend on the SoraFS toolkit can reuse the same arguments without installing
`iroha_cli`:

```shell
sorafs_cli taikai bundle \
  --payload segment_0042.m4s \
  --car-out segment_0042.car \
  --envelope-out segment_0042.to \
  --indexes-out segment_0042.indexes.json \
  --ingest-metadata-out segment_0042.da.json \
  --summary-out segment_0042.summary.json \
  --event-id "global-keynote" \
  --stream-id "stage-a" \
  --rendition-id "1080p" \
  --track-kind video \
  --codec av1-main \
  --bitrate-kbps 8000 \
  --resolution 1920x1080 \
  --segment-sequence 42 \
  --segment-start-pts 3600000 \
  --segment-duration 2000000 \
  --wallclock-unix-ms 1702560000000 \
  --manifest-hash <64-hex> \
  --storage-ticket <64-hex>
```

This command emits the same CAR/envelope/index/ingest bundles and supports the
optional `--summary-out` JSON for automation evidence.

For build systems that do not want to pull in the full `iroha_cli` toolchain,
the `sorafs_car` crate now exposes a standalone `taikai_car` binary that wraps
the same bundling logic:

```shell
taikai_car \
  --payload segment_0042.m4s \
  --car-out segment_0042.car \
  --envelope-out segment_0042.to \
  --indexes-out segment_0042.indexes.json \
  --ingest-metadata-out segment_0042.da.json \
  --summary-out segment_0042.summary.json \
  --event-id "global-keynote" \
  --stream-id "stage-a" \
  --rendition-id "1080p" \
  --track-kind video \
  --codec av1-main \
  --bitrate-kbps 8000 \
  --resolution 1920x1080 \
  --segment-sequence 42 \
  --segment-start-pts 3600000 \
  --segment-duration 2000000 \
  --wallclock-unix-ms 1702560000000 \
  --manifest-hash <64-hex> \
  --storage-ticket <64-hex>
```

Rehydrating from a stored CAR (no CMAF fragment required):

```shell
taikai_car \
  --car-in segment_0042.car \
  --car-out ./rehydrated/segment_0042.car \
  --envelope-out ./rehydrated/segment_0042.to \
  --indexes-out ./rehydrated/segment_0042.indexes.json \
  --ingest-metadata-out ./rehydrated/segment_0042.ingest.json \
  --summary-in ./artifacts/segment_0042.summary.json
```

In the rehydration path `--summary-in` carries all ingest/track fields, so only
the CAR and output paths are required on the command line.

Both commands emit identical CAR archives, envelopes, and metadata bundles, so
publishers can pick whichever binary aligns with their deployment environment.

### Torii Integration

When `blob_class = TaikaiSegment`, Torii now synthesises the segment envelope
and index bundle during ingest. The artefacts are spooled under
`config.da_ingest.manifest_store_dir/taikai/` using filenames of the form

```
taikai-envelope-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.norito
taikai-indexes-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.json
taikai-ssm-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.norito
taikai-trm-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.norito
```

These files mirror the manifest/PDP spool and provide the inputs required by
the SoraNS anchor service; the SSM file contains the Norito-encoded
`TaikaiSegmentSigningManifestV1` (including alias proofs) that was verified
during admission, and the optional TRM file persists the
`TaikaiRoutingManifestV1` window when publishers provide `taikai.trm`
metadata. Configure Torii with `torii.da_ingest.taikai_anchor`
(`endpoint`, optional `api_token`, and `poll_interval_secs`) to enable the
background uploader that periodically scans the spool and POSTs Taikai payloads
to the anchor endpoint.

## Consumer Expectations

- Verify that `ingest.manifest_hash` and `ingest.car.car_digest` match the
  advertised manifests and CAR archives before promoting a segment to viewers.
- Use `segment_sequence` and timestamp fields to detect gaps. Missing sequence
  numbers MUST trigger repair logic before continuing playback.
- Leverage `instrumentation` for observability dashboards (latency, ingest
  saturation). Missing instrumentation should not block playback.

## Manifest Extensions: TRM & SSM

The segment envelope captures a single CMAF fragment. Operators and SoraNS need
a higher-level manifest describing how those fragments are routed, replicated,
and cryptographically attested. Two Norito structures extend the envelope
surface:

1. **TRM — Taikai Routing Manifest V1**
2. **SSM — Taikai Segment Signing Manifest V1**

TRM describes how a stream/rendition window is anchored inside SoraNS, while SSM
turns each envelope into a signed attestation that Torii admission and gateways
can verify before delivering segments.

### Taikai Routing Manifest (TRM)

| Field | Type | Description |
| --- | --- | --- |
| `version` | `u16` | TRM format version (`1`). |
| `event_id` | `TaikaiEventId` | Matches the segment envelope event. |
| `stream_id` | `TaikaiStreamId` | Logical stream (`stage-a`, `sign-language`, etc.). |
| `segment_window` | `SegmentWindow` (`first` \| `last`) | Inclusive sequence range covered by this manifest. |
| `renditions` | `Vec<TaikaiRenditionRouteV1>` | Per-rendition routing records (see below). |
| `alias_binding` | `TaikaiAliasBinding` | Human-readable anchor (`docs.taikai`, `launch.sora`) plus SoraNS namespace + TTL. |
| `guard_policy` | `TaikaiGuardPolicy` | Required SoraNet circuits/guard quorum for this stream (lane ids + version). |
| `metadata` | `ExtraMetadata` | Deterministic extension channel (policy tags, audience caps, sponsor beacons). |

`TaikaiRenditionRouteV1` entries carry:

| Field | Type | Description |
| --- | --- | --- |
| `rendition_id` | `TaikaiRenditionId` | Ladder rung identifier. |
| `latest_manifest_hash` | `BlobDigest` | Hash from the most recent `DaManifestV1`. |
| `latest_car` | `TaikaiCarPointer` | CAR digest/CID stamp for the active segment window. |
| `availability_class` | `TaikaiAvailabilityClass` | `Hot`, `Warm`, or `Cold` replication policy. |
| `replication_targets` | `Vec<ProviderId>` | Ordered provider IDs responsible for holding the CAR window. |
| `soranet_circuit` | `GuardDirectoryId` | Canonical circuit/governance lane powering the stream. |
| `ssm_range` | `RangeInclusive<u64>` | Segment sequences for which SSMs must be available. |

`TaikaiAliasBinding` reuses the existing SoraNS alias proof vocabulary
(`AliasBindingV1`, `AliasProofBundleV1`) but adds Taikai-specific metadata so
gateways can emit meaningful `Sora-Name` and `Sora-Proof` headers for viewers.

#### Validation rules

Before Torii admits a TRM, the data-model enforces the following invariants:

- The manifest `segment_window` must be well-ordered (`start <= end`).
- Every rendition’s `ssm_range` must also be well-ordered **and** lie within
  the manifest window. A rendition that advertises `ssm_range` slices outside
  of the manifest window fails validation and is rejected before any alias or
  CAR state mutates.
- Rendition identifiers must be unique inside a manifest; duplicate
  `rendition_id` entries are treated as programmer errors so operators cannot
  accidentally override routes.

These checks run via `TaikaiSegmentWindow::validate()` and
`TaikaiRoutingManifestV1::validate()`, giving CLI/SDK tooling the same safety
rails as the runtime. When manifests fail validation, the CLI surfaces
actionable errors and Torii logs the offending rendition/window pair so
publishers can correct their automation before retrying.

### Alias rotation telemetry

Every accepted TRM now produces deterministic telemetry so operators can audit
alias rotations without scraping the spool:

- `taikai_trm_alias_rotations_total{cluster,event,stream,alias_namespace,alias_name}`
  increments whenever `/v2/da/ingest` accepts a routing manifest. Use this
  metric in Grafana/Prometheus to confirm manifests are rolling forward per
  SN13‑C’s rollout plan.
- The Torii `/status` payload exposes a new `taikai_alias_rotations` array
  containing `{cluster,event,stream,alias_namespace,alias_name,
  window_start_sequence,window_end_sequence,manifest_digest_hex,rotations_total,
  last_updated_unix}` entries keyed by (cluster,event,stream). Operators can
  capture this snapshot for evidence bundles before pushing a new alias window
  to SoraNS.

### Lineage hints & duplicate guarding

Torii persists a lineage ledger for every alias under
`config.da_ingest.manifest_store_dir/taikai/` using files named
`taikai-trm-state-<alias_slug>.json`. Each record captures the alias namespace,
alias name, last `segment_window`, manifest digest, and the time the manifest
was accepted. When `/v2/da/ingest` receives a TRM for a given alias it loads the
cached record and rejects the submission if the manifest digest matches the last
accepted digest or if the new window overlaps the stored range. This ensures
publishers can only advance windows forward and prevents replaying stale
manifests.

In addition, every queued Taikai artefact now includes a lineage hint stored at
`taikai-lineage-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.json`.
The anchor worker reads this file and adds a `lineage_hint` object to the POST
payload alongside `envelope_base64`, `indexes`, `ssm_base64`, and `trm_base64`.
The hint has the shape:

```json
{
  "version": 1,
  "alias_namespace": "sora",
  "alias_name": "docs",
  "previous_manifest_digest_hex": "cafebabe...",
  "previous_window_start_sequence": 1201,
  "previous_window_end_sequence": 1320,
  "previous_updated_unix": 1735168000
}
```

When an alias has no prior TRM the `previous_*` fields are `null`. Anchors and
governance tooling can use this hint to confirm that windows are rolling forward
and to reconcile the routing manifest history without re-reading the lineage
ledger.

#### Anchor governance evidence bundle

Roadmap SN13-C now requires SoraNS anchors to preserve both the persisted
lineage ledger (`taikai-trm-state-<alias_slug>.json`) and the emitted hint for
every routing window. Operators should:

1. Copy the lineage ledger and hint spool file from
   `config.da_ingest.manifest_store_dir/taikai/` into an evidence directory
   (for example `artifacts/taikai/anchor/<event>/<alias>/<timestamp>/`).
2. Capture the POST payload that Torii delivered to the SoraNS anchor (the file
   already contains the `lineage_hint` object) and store it alongside the spool
   files.
3. Hash each artefact and record it in the
   [`taikai_anchor_lineage_packet.md`](../examples/taikai_anchor_lineage_packet.md)
   template so governance packets can assert which manifest window superseded
   the previous one. The template mirrors the repo evidence workflow, so you
   can reuse `scripts/repo_evidence_manifest.py` to produce a manifest covering
   the lineage ledger, hint, POST capture, and GAR/TRM digests.

Anchoring teams should submit the completed packet with GAR v2/RPT artefacts so
auditors have deterministic proof that alias rotations advanced monotonically
and that Torii forwarded the lineage hint SoraNS used.

#### Dashboards & alerts

Alias rotation metrics now surface in Grafana via
`dashboards/grafana/taikai_viewer.json`. Use the new **Alias rotations (5m)** panel
and the `event`/`alias` variables to watch for unexpected spikes or stale
windows. The panel plots `increase(taikai_trm_alias_rotations_total[5m])`
grouped by alias so operators can verify that new TRMs land at the cadence
promised in SN13-C. Wire alerts to fire when no rotations occur within the
expected timebox or when multiple rotations hit the same alias within a short
interval—both conditions indicate lineage mismatches that require governance
review.

### Segment Signing Manifest (SSM)

| Field | Type | Description |
| --- | --- | --- |
| `version` | `u16` | SSM format version (`1`). |
| `segment_envelope_hash` | `BlobDigest` | BLAKE3 digest of the Norito-encoded envelope payload. |
| `manifest_hash` | `BlobDigest` | Canonical `DaManifestV1` hash referenced by the envelope. |
| `car_digest` | `BlobDigest` | CAR digest derived from `TaikaiCarPointer`. |
| `publisher_account` | `AccountId` | Account responsible for this broadcast window. |
| `publisher_key` | `PublicKey` | Key used to sign the SSM. |
| `signature` | `SignatureOf<TaikaiSegmentSigningManifestV1>` | Detached signature covering the fields above. |
| `signed_unix_ms` | `u64` | Millisecond timestamp when the signature was produced. |
| `alias_proof` | `AliasProofBundleV1` | Proof stapled into `Sora-Proof` headers so operators can trace the alias state. |
| `metadata` | `ExtraMetadata` | Policy tags (e.g., licensing SKU, region allowlist). |

SSMs bundle the exact bytes that the publisher signed, avoiding opportunistic
hash recomputation inside Torii or the anchor service. The `alias_proof` field
lets gateways propagate the proof they received from SoraNS without fetching it
out-of-band, reducing ingress load during large Taikai launches.

### Publisher Signing & SoraNS Anchoring

1. **Envelope finalisation** — Once the ingest worker writes the envelope and
   CAR summary to disk, it queues the payload for the Taikai anchor task.
2. **SSM construction** — The anchor task builds `TaikaiSegmentSigningManifestV1`
   using the envelope hash, manifest hash, CAR digest, and publisher metadata.
   Signing keys are loaded from the `taikai.publisher_keystore` config or KMS.
3. **Alias proof stapling** — After signing, the anchor task requests an alias
   proof from the SoraNS service (`POST /v2/sorans/alias_proof`) scoped to the
   Taikai alias. The returned `AliasProofBundleV1` is embedded into the SSM and
   cached so gateways can emit `Sora-Proof` headers without additional I/O.
4. **TRM emission** — When a window completes (e.g., every 120 segments) the
   anchor task assembles the TRM: combining the stream/rendition policies,
   rolling window boundaries, replication assignments, and the set of SSM ids
   referenced by that window. The TRM is posted to the SoraNS anchor endpoint,
   which stores it under the human-readable alias and returns a manifest id.
5. **Gateway distribution** — Alias proofs and TRM ids are written to the
   gateway manifest cache. Operators verify them via the existing
   `sorafs alias verify` helper before deploying a new Taikai release.

The SoraNS anchor API accepts the following payload (simplified):

```json
{
  "alias": "docs.sora",
  "trm": "<TaikaiRoutingManifestV1 Norito bytes (base64)>",
  "ssm_refs": ["b3f1…", "2a9d…"],
  "publisher_account": "i105..."
}
```

Responses include the aliased manifest id, `Sora-Proof` header, and TTL so
Torii can update its local cache before the next submission.

### Torii Admission & Proof Verification

Torii admission now requires Taikai uploads to provide both the envelope and a
matching SSM:

1. Clients submit the CMAF payload with the ingest metadata table documented
   earlier **plus** a `taikai.ssm` field containing the Norito-encoded SSM.
   Once a routing window is ready, the same request includes `taikai.trm`
   containing the Norito-encoded `TaikaiRoutingManifestV1` so Torii can spool
   it alongside the envelope.
2. Torii recomputes the envelope hash/manifest hash/car digest locally and
   verifies the publisher signature. If any digest mismatch occurs, admission
   returns `ERR_TAIKAI_SSM_MISMATCH`.
3. The alias proof embedded in the SSM is validated through
   `sorafs_manifest::alias_cache`, emitting `torii_alias_cache_*` metrics and
   reusing the existing SoraFS expiry warnings (`Sora-Proof-Status` headers).
4. Successful uploads atomically spool the envelope, SSM, and (when provided)
   TRM window record. The anchor payload now includes `trm_base64` so the
   SoraNS uploader receives the routing manifest together with the envelope
   artefacts. Upcoming work will refuse to anchor envelopes whose window
   already has a newer TRM id to prevent manifest replays.
5. Audit logs record `(manifest_digest, alias_name, ssm_digest)` so the SoraNS
   anchor service and governance panels can trace every broadcast segment.

Torii stamps additional DA metadata during ingest so downstream DA and cache
pipelines can react deterministically:

- `taikai.availability_class` — `hot`/`warm`/`cold` derived from the TRM (or the enforced storage class when no TRM is present) so retention overrides remain explicit in the manifest.
- `da.proof.tier` — PDP/PoTR tier bound to the enforced storage class, letting proof schedulers and dashboards classify Taikai blobs correctly.
- `taikai.cache_hint` — Norito JSON object carrying `{event, stream, rendition, sequence, payload_len, payload_blake3_hex}` so cache-admission (CAA) emitters can derive deterministic admission/eviction envelopes without re-hashing payloads.

Torii rejects Taikai submissions whose cache hint payload length or digest
disagree with the canonical CMAF bytes, or whose `da.proof.tier` no longer
matches the enforced retention class, ensuring downstream CAA emitters and
PDP/PoTR schedulers consume authenticated hints only.

Gateways enforce the same verification steps by running `xtask sorafs alias
verify --taikai` inside CI. The helper fetches TRMs/SSMs, validates the
publisher signatures, and asserts that the CAR digests match the manifests
returned by Torii. These hooks block deployment when:

- the alias proof expires (`Sora-Proof-Status: expired`),
- the SSM signature fails,
- or the TRM window does not reference the head segment uploaded by the
  publisher.

## Future Work (Tracked under SNNet-13)

- Torii ingest hooks and SoraNS anchoring that publish envelopes produced by
  the bundler.
- GAR v2 policy bindings for broadcast licensing and metric directives.
- Viewer SDK updates that consume `TaikaiSegmentEnvelopeV1` directly and expose
  deterministic metrics such as live-edge offset and rebuffer events.

The current implementation provides the shared data model, schema metadata, and
the CLI bundler required for deterministic publishing. Subsequent patches will
focus on Torii ingestion hooks and SoraNS anchoring atop this foundation.
