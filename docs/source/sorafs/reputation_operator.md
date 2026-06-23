---
title: SoraFS Reputation Operator Guide
summary: Generate, validate, publish, and consume deterministic SoraFS provider reputation snapshots.
---

# SoraFS Reputation Operator Guide

SoraFS reputation V1 is anchored in `sorafs_manifest::reputation`. Operators
should treat the Norito snapshot as the canonical artifact, and any JSON view as
a diagnostic or transport representation produced from the same types.

## Canonical Inputs

Each provider score is built from a `ReputationProviderInputV1`:

- `provider_id`: the governance-controlled SoraFS provider id.
- `metrics`: PoR, PDP, PoTR, latency, dispute, token-violation, and repair
  breach values in basis points.
- `reserve_stage`: the Reserve+Rent lifecycle stage.
- `previous_score_bps`: optional prior score for smoothing.
- `active_dispute` and `slashing_event`: hard penalty flags.

The default `ReputationWeightsV1` matches the SFM-3 plan:
PoR 2200, PDP 2000, PoTR 1800, latency 1500, dispute 1000,
token violation 500, and repair breach 1000 basis points.

## Snapshot Generation

Use `build_reputation_snapshot(snapshot_id, generated_at_unix, weights, inputs,
previous_snapshot_id)` to produce a `ReputationSnapshotV1`. The helper validates
weights and inputs, scores providers with fixed-point basis-point arithmetic,
sorts provider entries lexicographically, computes each raw metrics hash, and
stores the Merkle root in the snapshot.

When settlement-satisfaction data is available, use
`build_reputation_snapshot_with_trust_edges(...)` and pass
`ReputationTrustEdgeV1` rows. Each edge names the provider emitting the trust
signal, the provider receiving it, and `trust_bps`. The EigenTrust-style
iteration runs in fixed-point basis points with the V1 alpha value (`8500`) and
keeps the direct metric score as an upper bound, so trust feedback can reduce a
score but cannot override objective proof, reserve, dispute, or slashing
penalties.

Before publishing:

- `snapshot.validate()` must pass.
- `snapshot_id` must be nonzero and unique for the publication period.
- The provider set must be non-empty.
- `generated_at_unix` should be the scheduled publication timestamp.
- Archive the exact Norito bytes used to compute governance and object-storage
  digests.

## Governance Publication

Publish the snapshot through `GovernanceLogPayloadV1::ReputationSnapshot`.
`GovernanceLogNodeV1::validate()` revalidates the embedded snapshot, including
provider ordering, raw metrics hashes, and Merkle root. Consumers should reject
governance nodes whose payload validation fails before using any score for
routing or incentives.

Torii nodes with SoraFS storage enabled can also accept and serve the latest
snapshot through the SoraFS-scoped reputation API:

- `POST /v1/sorafs/reputation/latest`: accept a canonical
  `ReputationSnapshotV1`, validate it, persist it through the configured
  governance publisher, and cache it as the latest snapshot.
- `GET /v1/sorafs/reputation/latest`: return the latest snapshot summary,
  including provider scores and the Merkle root.
- `GET /v1/sorafs/reputation/providers/{provider_id}`: return the provider
  entry and a `ReputationMerkleProofV1` for the latest snapshot.
- `GET /v1/sorafs/reputation/snapshots/{snapshot_id_hex}`: return a previously
  accepted snapshot summary by 16-byte snapshot id.
- `GET /v1/sorafs/reputation/weights`: return the weights and smoothing
  parameters used by the latest snapshot.
- `GET /v1/sorafs/reputation/events?since=N&limit=N`: return sequenced
  reputation snapshot events newer than `since`. Use `next_since` from the
  response as the cursor for the next poll.
- `GET /v1/sorafs/reputation/events/stream?since=N&limit=N`: stream
  `reputation_snapshot` server-sent events. Torii emits the optional backlog
  selected by `since`/`limit` before waiting for live snapshot publications.
- `GET /ws/reputation?since=N&limit=N`: upgrade to a WebSocket that emits
  `reputation_snapshot` JSON text frames for the same backlog and live
  publications. If a client falls behind, Torii sends an `event = "lagged"`
  frame so the client can resynchronize through the REST event list.

Each accepted snapshot also records a `ReputationSnapshotEventV1` containing
the local sequence number, snapshot id, generation timestamp, Merkle root,
provider count, and previous snapshot id.

Successful reputation `GET` responses include `ETag` and
`Cache-Control: public, max-age=30, must-revalidate`. Consumers may repeat the
same request with `If-None-Match`; Torii returns `304 Not Modified` when the
snapshot, provider proof, weights, or event page has not changed.

The node filesystem publisher writes immutable `.to` and `.json` snapshot
artifacts under `reputation/snapshots/<snapshot_id>/` and updates
`reputation/latest.to` plus `reputation/latest.json` pointers. Each artifact has
a `.blake3` sidecar.

## Observability

After Torii validates and accepts a reputation snapshot, it exports:

- `sorafs_reputation_ingest_lag_seconds`
- `sorafs_reputation_snapshot_age_seconds`
- `sorafs_reputation_snapshot_generated_at_unix`
- `sorafs_reputation_provider_count`
- `sorafs_reputation_low_score_providers`
- `sorafs_reputation_score{provider_id}`
- `sorafs_reputation_threshold_crossings_total{level}`

The `sorafs_reputation_score{provider_id}` gauge is bounded to the top 100
providers in the latest accepted snapshot. Providers that fall out of that set
are removed from the exported label set.

Import `dashboards/grafana/sorafs_reputation_health.json` for operator views of
snapshot freshness, publisher lag, provider counts, low-score providers,
top-score trends, and low-score threshold crossings. Install
`dashboards/alerts/sorafs_reputation_rules.yml`; the matching promtool fixture is
`dashboards/alerts/tests/sorafs_reputation_rules.test.yml`.

## Proof Verification

For a provider lookup, call `snapshot.merkle_proof(provider_id)` and return the
matching `ProviderReputationV1` plus `ReputationMerkleProofV1`. Consumers verify
with:

```rust
proof.verify(&provider_record, snapshot.merkle_root)?;
```

Verification fails if the provider id differs, the provider record is tampered,
the proof is too long, or the recomputed path does not match the advertised
root.

Operators can verify archived Norito snapshot/proof artifacts with:

```bash
sorafs_cli reputation verify \
  --snapshot=reputation-snapshot.to \
  --provider-id=provider-a \
  --proof=provider-a-proof.to \
  --summary-out=reputation-verify.json
```

Omit `--provider-id` and `--proof` to validate only the snapshot envelope,
provider ordering, raw metrics hashes, and Merkle root.

Operators can publish and inspect the latest Torii reputation view with:

```bash
sorafs_cli reputation publish \
  --torii-url=https://validator.example \
  --snapshot=reputation-snapshot.to \
  --summary-out=reputation-publish.json

sorafs_cli reputation snapshot \
  --torii-url=https://validator.example \
  --output=reputation-latest.json

sorafs_cli reputation fetch \
  --torii-url=https://validator.example \
  --provider-id=provider-a \
  --format=json \
  --summary-out=provider-a-reputation.json

sorafs_cli reputation watch \
  --torii-url=https://validator.example \
  --since=0 \
  --limit=100 \
  --summary-out=reputation-events.json
```

`publish` reads canonical Norito snapshot bytes, validates them locally, and
posts their JSON representation to Torii. `snapshot` and `fetch` consume the
SoraFS-scoped Torii endpoints. `watch` polls the reputation event endpoint once
by default; pass `--max-polls=0` for continuous polling or a positive
`--max-polls=N` for bounded repeated polls. Use `verify` for archived canonical
snapshot and proof replay.

SDK consumers can use the JavaScript and Python Torii client helpers instead
of assembling reputation URLs by hand. Both SDKs expose latest snapshot,
provider proof, historical snapshot, weights, event polling, and SSE stream
helpers, including `If-None-Match` support for cache-aware polling:

```javascript
const latest = await client.getSorafsReputationLatest({ ifNoneMatch: etag });
const provider = await client.getSorafsReputationProvider("provider-a");
const events = await client.listSorafsReputationEvents({ since: 0, limit: 100 });

for await (const event of client.streamSorafsReputationEvents({ since: 0 })) {
  console.log(event.id, event.data.snapshot_id_hex);
}
```

```python
latest = client.get_sorafs_reputation_latest(if_none_match=etag)
provider = client.get_sorafs_reputation_provider("provider-a")
events = client.list_sorafs_reputation_events(since=0, limit=100)

for event in client.stream_sorafs_reputation_events(since=0, with_metadata=True):
    print(event.id, event.data["snapshot_id_hex"])
```

## Routing Integration

`sorafs_car::scoreboard::TelemetrySnapshot::from_reputation_snapshot(&snapshot)`
converts validated provider scores into scheduler telemetry. The scoreboard uses
`ProviderTelemetry::reputation_score_bps` as a multiplicative weight component,
so low reputation reduces routing share while still letting explicit telemetry
penalties control hard exclusion.

Telemetry JSON accepted by `sorafs_fetch --telemetry-json` may also include:

```json
{
  "provider_id": "provider-a",
  "reputation_score_bps": 9200,
  "last_updated_unix": 1800000000
}
```

Values outside `0..=10000` are rejected during parsing.

## Validation Commands

Focused local validation for this surface:

```bash
CARGO_TARGET_DIR=/tmp/iroha-codex-reputation CARGO_INCREMENTAL=0 \
  cargo test -j 1 -p sorafs_manifest reputation -- --nocapture

CARGO_TARGET_DIR=/tmp/iroha-codex-reputation CARGO_INCREMENTAL=0 \
  cargo test -j 1 -p sorafs_car scoreboard --features manifest -- --nocapture

CARGO_TARGET_DIR=/tmp/iroha-codex-reputation-orch CARGO_INCREMENTAL=0 \
  cargo test -j 1 -p sorafs_orchestrator reputation --test sorafs_cli \
  --features cli-orchestrator -- --nocapture

CARGO_TARGET_DIR=/tmp/iroha-codex-reputation-node CARGO_INCREMENTAL=0 \
  cargo test -j 1 -p sorafs_node reputation -- --nocapture

CARGO_TARGET_DIR=/tmp/iroha-codex-reputation-node CARGO_INCREMENTAL=0 \
  cargo test -j 1 -p iroha_torii reputation -- --nocapture

CARGO_TARGET_DIR=/tmp/iroha-codex-reputation-metrics CARGO_INCREMENTAL=0 \
  cargo test -j 1 -p iroha_telemetry \
  records_sorafs_reputation_snapshot_metrics -- --nocapture

CARGO_TARGET_DIR=/tmp/iroha-codex-reputation-node CARGO_INCREMENTAL=0 \
  cargo test -j 1 -p iroha_torii generated_spec_includes_documented_paths -- --nocapture

jq empty dashboards/grafana/sorafs_reputation_health.json

ruby -e 'require "yaml"; YAML.load_file(ARGV.fetch(0))' \
  dashboards/alerts/sorafs_reputation_rules.yml

ruby -e 'require "yaml"; YAML.load_file(ARGV.fetch(0))' \
  dashboards/alerts/tests/sorafs_reputation_rules.test.yml

cd javascript/iroha_js && \
  node --test --test-name-pattern "SoraFS reputation|sorafs reputation" \
  test/toriiClient.test.js

cd javascript/iroha_js && \
  npx eslint --max-warnings=0 src/toriiClient.js test/toriiClient.test.js

cd python/iroha_python && \
  python3 -m py_compile src/iroha_python/client.py \
  tests/client_sorafs_reputation_test.py
```

Run the full workspace test suite when the validation budget permits.
