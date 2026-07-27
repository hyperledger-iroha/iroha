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

Before handing material to the external threshold-signing worker:

- `snapshot.validate()` must pass.
- `snapshot_id` must be nonzero and unique for the publication period.
- The provider set must be non-empty.
- `generated_at_unix` should be the scheduled publication timestamp.
- Archive the exact Norito bytes used to compute governance and object-storage
  digests.

## Governance Publication

`sorafs_node::reputation::ReputationIngestService` consumes only finalized
native proof, reputation-journal, repair, orderbook, and reserve projections.
Its five physical query cursors and deterministic provider accumulators share
one canonical restart-safe checkpoint. When every governed feed is complete at
the release-window anchor, the service places the exact unsigned signing
material in a bounded durable outbox. Distinct delivery failures are
idempotent, retry-bounded, and dead-lettered; acknowledgement must bind a
canonical `SignedReputationSnapshotV1` to the exact snapshot, scoring evidence,
policy digest, and signing digest. The projector never holds signing keys or
signs locally.

The external threshold-signing/publication worker must verify the configured
trust policy, publish the signed result through the Governance DAG/committed
projection, and acknowledge the exact outbox item. Consumers must reject
governance nodes whose payload validation fails before using any score for
routing or incentives.

The SoraFS-scoped Torii reputation family is read-only:

- `GET /v1/sorafs/reputation/latest?limit=N`: return the latest snapshot
  summary, including the Merkle root and up to `limit` provider scores while
  preserving total provider counts.
- `GET /v1/sorafs/reputation/providers/{provider_id}`: return the provider
  entry and a `ReputationMerkleProofV1` for the latest snapshot.
- `GET /v1/sorafs/reputation/snapshots/{snapshot_id_hex}?limit=N`: return a
  previously accepted snapshot summary by 16-byte snapshot id with the same
  bounded provider-score readback. The committed publication checkpoint keeps
  an immutable suffix of at most 1,024 authenticated snapshots; unknown or
  evicted ids return `404` and are never substituted with the latest snapshot.
- `GET /v1/sorafs/reputation/weights`: return the weights and smoothing
  parameters used by the latest snapshot.
- `GET /v1/sorafs/reputation/events?since=N&limit=N`: return sequenced
  reputation snapshot events newer than `since`. Use `next_since` from the
  response as the cursor for the next poll.
- `GET /v1/sorafs/reputation/events/stream?since=N&limit=N`: stream
  `reputation_snapshot` server-sent events. Torii emits the optional backlog
  selected by `since`/`limit` before waiting for live snapshot publications.
- `GET /v1/sorafs/reputation/events/ws?since=N&limit=N`: upgrade to a WebSocket that emits
  `reputation_snapshot` JSON text frames for the same backlog and live
  publications. If a client falls behind, Torii sends an `event = "lagged"`
  frame so the client can resynchronize through the REST event list.

Every route in this family has the same hard-cut transport contract. The
request method must be `GET`, the body must be empty, and canonical request
authentication is mandatory. Supply exactly one of:

- the signature quartet `X-Iroha-Account`, `X-Iroha-Signature`,
  `X-Iroha-Timestamp-Ms`, and `X-Iroha-Nonce`; or
- one exact standard-base64 `X-Iroha-Witness`. `X-Iroha-Account` may accompany
  the witness only when it exactly matches the witness subject.

The two modes are mutually exclusive. Missing, partial, duplicated, aliased,
or mixed authentication is rejected. Both proofs bind the exact method, path,
canonical query, and empty body, so clients must target the final Torii origin:
there is no redirect compatibility path and a fixed nonce or static witness
must never be replayed by an implicit retry.

`Last-Event-ID` is rejected for reputation streams. Resume by taking the last
finalized sequence, passing it as the `since` query parameter on a newly
authenticated request, and using `GET /v1/sorafs/reputation/events` to
resynchronize after a `lagged` frame.

The standard daemon now constructs and supervises the finalized-feed projector,
durable journal transaction submitter, and restart reconciliation loop. Startup
still fails closed until deployment-owned finalized-query, external
threshold-signing, and authenticated Governance DAG adapters are injected, so
these reads become available only from the resulting committed projection.
There is deliberately no Torii POST or local `reputation publish` CLI fallback.

Successful finite reputation responses include an `ETag` and
`Cache-Control: private, max-age=30, must-revalidate`. Consumers may repeat the
same authenticated request with `If-None-Match`; Torii returns
`304 Not Modified` when the snapshot, provider proof, weights, or event page
has not changed. SSE and WebSocket handshakes use
`Cache-Control: private, no-store`. Both finite and streaming responses carry:

```text
Vary: X-Iroha-Account, X-Iroha-Signature, X-Iroha-Timestamp-Ms, X-Iroha-Nonce, X-Iroha-Witness
```

## Observability

Once the standard runtime consumes the committed projection, it exports:

- `sorafs_reputation_ingest_lag_seconds`
- `sorafs_reputation_snapshot_age_seconds`
- `sorafs_reputation_snapshot_generated_at_unix`
- `sorafs_reputation_provider_count`
- `sorafs_reputation_low_score_providers`
- `sorafs_reputation_score{provider_id}`
- `sorafs_reputation_threshold_crossings_total{level}`
- `sorafs_reputation_runtime_live`
- `sorafs_reputation_runtime_ready`
- `sorafs_reputation_runtime_dependencies_ready`
- `sorafs_reputation_journal_transaction_submitter_ready`
- `sorafs_reputation_runtime_finalized_height`
- `sorafs_reputation_runtime_consecutive_failures`
- `sorafs_reputation_runtime_material_acknowledged`
- `sorafs_reputation_runtime_provider_count`
- `sorafs_reputation_runtime_ticks_total{result}`

The `sorafs_reputation_score{provider_id}` gauge is bounded to the top 100
providers in the latest accepted snapshot. Providers that fall out of that set
are removed from the exported label set.

The `*_runtime_*` series are payload-free and use no provider/event labels.
Until `V1-BLOCK-REPUTATION-RUNTIME-01` closes,
`sorafs_reputation_journal_transaction_submitter_ready` and aggregate runtime
readiness intentionally remain zero even if committed ingest and external
publication complete.

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

Snapshot publication is not a Torii or local CLI mutation. The finalized-chain
projector durably exposes canonical unsigned material through its bounded
outbox; an independently administered threshold-signing worker validates and
signs that material, then acknowledges the exact canonical envelope. Torii's
reputation family is read-only. Use `verify` for archived canonical snapshot
and proof replay. Live `snapshot`, `fetch`, and `watch` reads require both an
exact canonical single-key I105 account and its matching runtime-only Ed25519
private-key file:

```bash
sorafs_cli reputation snapshot \
  --torii-url=https://validator.example \
  --auth-account="$REPUTATION_READER_I105" \
  --auth-private-key-file=/runtime/sorafs/reputation/reader.key \
  --output=reputation-latest.json

sorafs_cli reputation fetch \
  --torii-url=https://validator.example \
  --provider-id=provider-a \
  --auth-account="$REPUTATION_READER_I105" \
  --auth-private-key-file=/runtime/sorafs/reputation/reader.key \
  --format=json \
  --summary-out=provider-a-reputation.json

sorafs_cli reputation watch \
  --torii-url=https://validator.example \
  --auth-account="$REPUTATION_READER_I105" \
  --auth-private-key-file=/runtime/sorafs/reputation/reader.key \
  --since=0 \
  --limit=100 \
  --max-polls=1 \
  --summary-out=reputation-events.json
```

Provider ids are exact 1..=256-byte ASCII values from
`[A-Za-z0-9_.:-]`; the whole values `.` and `..` are reserved URL dot
segments and are rejected. The CLI and collector do not trim, decode, alias, or
otherwise rewrite this path identity.

The key file contains one canonical private-key token, optionally followed by
one line ending. It must be a stable regular non-symlink file with exactly one
hard link; on Unix it must grant no group or world permissions. The CLI rejects
aliases, multi-key accounts, inline-key and witness compatibility options,
account/key mismatch, unsafe file identity or metadata changes, and malformed
text without printing key bytes. It signs the exact GET path and canonical
query with an empty-body digest, current timestamp, and fresh OS-random nonce
for every request or watch poll. Duplicate scalar options fail before I/O. Its
HTTP client follows no redirects, makes no automatic retries, ignores ambient
proxy configuration, advertises only `Accept-Encoding: identity`, disables
transparent decompression, and rejects any non-identity response. Both success
and error bodies fail closed above the same 4 MiB declared or streamed limit;
Torii error bodies and provider-bearing URLs are never copied into diagnostics.

Before promoting a deployed publisher to routing or incentive enforcement,
collect the rollout evidence bundle with the helper:

```bash
python3 scripts/run_sorafs_reputation_rollout_evidence.py \
  @scripts/examples/sorafs_reputation_rollout_collection.args.example
```

The collection arguments must include the same `--auth-account=I105` and
`--auth-private-key-file=PATH`, plus `--publish-evidence=PATH` pointing to a
reviewed, payload-free result from the external threshold-signing/publication
worker. The collector validates the key path without reading its contents and
forwards the two authentication arguments only to the three live read steps;
it also validates every requested provider id against the exact route grammar
before deriving an injective, portable sharded path: the exact provider-id
bytes are lowercase ASCII-hex in components of at most 64 characters beneath
fixed `provider-by-provider-id` or `verify-by-provider-id` namespaces, ending
in `artifact.json`. The runner creates only the already-validated parent for
the current provider step immediately before launch, and rejects traversal or
symlink substitution; `--dry-run` performs no writes. The expanded response
arguments reject option-prefix abbreviations and repeated scalar flags before
parsing; only `--provider-id` and `--provider-proof` are repeatable. The key
bytes and authentication headers are never placed in the plan or evidence.
Use `--dry-run` first to print the exact read-only command plan. The helper
reads the latest snapshot, fetches each required provider proof, replays
archived proofs, watches bounded reputation events, and invokes the rollout
gate with the reviewed external publication evidence. Operators that collected
the artifacts separately can run the gate directly:

```bash
python3 scripts/check_sorafs_reputation_rollout_evidence.py \
  --evidence publish=reputation-publish.json \
  --evidence latest=reputation-latest.json \
  --evidence provider=provider-a-reputation.json \
  --evidence events=reputation-events.json \
  --evidence verify=reputation-proof-replay.json \
  --evidence metrics=reputation-metrics-canary.json \
  --evidence transport=reputation-transport-canary.json \
  --evidence consumption=reputation-consumption-canary.json \
  --require-provider=provider-a \
  --summary-out=reputation-rollout-summary.json
```

The gate fails closed unless the publish/latest/provider/event/proof artifacts
refer to the same fresh snapshot, deployed metrics show bounded snapshot age and
ingest lag, SSE and WebSocket delivery both observe snapshot events, and routing
plus incentive smoke evidence proves the reputation score was consumed. The
artifacts must stay payload-free: do not archive raw snapshot bytes, proof
frames, HTTP response bodies, bearer tokens, or signing material in the rollout
directory.

The authenticated committed read surface is a completed local L0 component; it
does not close deployment qualification or promotion. L1 remains open until the
four-validator deployment has genuine threshold-signing, immutable finalized
query, authenticated DAG publication/readback/head-inclusion, callback,
restart/failover, transport, and rollout evidence. L2 remains open until the
externally signed release envelope and deterministic aggregate replay accept
the complete 17-lane evidence set. Do not report this lane production-ready
from local route or SDK tests alone.

JavaScript/TypeScript and Python expose the latest, provider, historical
snapshot, weights, event-page, and SSE surfaces. Pass either per-request signer
authentication or an exact witness header; neither SDK accepts canonical auth
proof fields through default headers or permits a raw signature quartet in the
generic header map.

JavaScript finite reads and SSE make one attempt, set redirect mode to `error`,
and never reconnect implicitly:

```javascript
const latest = await client.getSorafsReputationLatest({
  canonicalAuth,
  ifNoneMatch: etag,
});
const provider = await client.getSorafsReputationProvider(
  "provider-a",
  { canonicalAuth },
);
const events = await client.listSorafsReputationEvents({
  canonicalAuth,
  since: 0,
  limit: 100,
});

for await (const event of client.streamSorafsReputationEvents({
  canonicalAuth,
  since: 0,
})) {
  console.log(event.id, event.data.snapshot_id_hex);
}
```

For witness authentication, replace `canonicalAuth` with
`headers: {"X-Iroha-Witness": witnessBase64}` on each call.

Python finite reads and SSE both perform exactly one `stream=True` request,
disable redirects, and require the configured transport adapter to prove
`retries=0` before signing. They advertise only
`Accept-Encoding: identity`, reject non-identity encoding and content-type
aliases, cap finite bodies at 4 MiB and SSE events at 64 KiB, and leave
non-success response bodies unread so failures remain payload-free. Before
returning or delivering a mapping, the client enforces the closed V1 profile
from those bounded bytes: strict UTF-8 without BOM, trailing data, duplicate
keys, floats, or non-finite values, followed by exact field, type, integer,
ordering, count, proof-depth, and event-chain validation. SSE accepts only
exact `reputation_snapshot` or `lagged` frames, requires snapshot frame ids to
equal `data.sequence`, and never reconnects. The stream API has no retry or
backoff knobs; a static witness or caller-fixed nonce is therefore used for one
request only.

```python
latest = client.get_sorafs_reputation_latest(
    canonical_auth=canonical_auth,
    if_none_match=etag,
)
provider = client.get_sorafs_reputation_provider(
    "provider-a",
    canonical_auth=canonical_auth,
)
events = client.list_sorafs_reputation_events(
    canonical_auth=canonical_auth,
    since=0,
    limit=100,
)

for event in client.stream_sorafs_reputation_events(
    canonical_auth=canonical_auth,
    since=0,
    with_metadata=True,
):
    print(event.id, event.data["snapshot_id_hex"])
```

Kotlin core-jvm and the mirrored Java Android SDK expose dedicated typed
`SorafsReputationClient` implementations for the same five finite reads and
authenticated SSE. These clients are signer-only: witness and raw-header modes
are deliberately unsupported. They validate the closed V1 JSON profiles,
perform exactly one signed GET, do not reconnect, expose no
`Last-Event-ID`/resume-header surface, and require a true streaming transport
for SSE rather than falling back to buffered delivery. Their platform
transport does not follow redirects.
The Java Android built-in OkHttp provider also disables HTTP/HTTPS redirects
and connection retries. A caller-injected custom transport retains its own
explicit policy.

```kotlin
val reputation = SorafsReputationClient(URI.create("https://validator.example"))
val latest = reputation.getLatest(canonicalAuth, 100).join()
val events = reputation.listEvents(canonicalAuth, since = "0", limit = 100).join()
val stream = reputation.openEventStream(canonicalAuth, listener, since = "0")
```

```java
var reputation =
    new SorafsReputationClient(URI.create("https://validator.example"));
var latest = reputation.getLatest(canonicalAuth, 100).join();
var events = reputation.listEvents(canonicalAuth, "0", 100).join();
var stream = reputation.openEventStream(canonicalAuth, listener, "0", 100);
```

Swift provides the same five finite projections and one true streaming SSE
surface. Construction requires an HTTPS Torii root and a matching exact
single-key I105 account/Ed25519 private key. Every call creates one freshly
signed empty GET, rejects redirects, validates the closed response schema, caps
finite responses at 4 MiB and each SSE frame at 64 KiB, and never retries or
reconnects:

```swift
let reputation = try SorafsReputationClient(
    baseURL: URL(string: "https://validator.example")!,
    accountId: reputationReaderI105,
    privateKey: reputationReaderPrivateKey
)
let latest = try await reputation.latest(limit: 100)
let events = try await reputation.events(since: 0, limit: 100)
for try await frame in try reputation.streamEvents(since: 0, limit: 100) {
    consume(frame)
}
```

C# exposes typed methods on a `ToriiClient` configured with matching
`CanonicalRequestCredentials`. The internally managed transport disables
redirects; reputation calls fail closed before signing when a public
caller-injected `HttpClient` is used because its retry, redirect, and
decompression policy cannot be proven. Finite responses and SSE frames use the
same 4 MiB and 64 KiB bounds and strict closed V1 validation:

```csharp
using var client = new ToriiClient(
    new Uri("https://validator.example"),
    options: new ToriiClientOptions
    {
        CanonicalRequestCredentials =
            new CanonicalRequestCredentials(reputationReaderI105, privateKeySeed),
    });
var latest = await client.GetSoraFsReputationLatestAsync(limit: 100);
var events = await client.GetSoraFsReputationEventsAsync(since: 0, limit: 100);
await foreach (var frame in client.StreamSoraFsReputationEventsAsync(
                   since: 0,
                   limit: 100))
{
    Consume(frame);
}
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

cd python/iroha_python && \
  python3 -m pytest tests/client_sorafs_reputation_test.py

cd python/iroha_python && \
  python3 -m ruff check src/iroha_python/client.py \
  tests/client_sorafs_reputation_test.py

cd kotlin && \
  ./gradlew :core-jvm:test \
  --tests 'org.hyperledger.iroha.sdk.sorafs.SorafsReputationClientTest' \
  --console=plain

cd java/iroha_android && \
  ./gradlew :core:test \
  --tests 'org.hyperledger.iroha.android.sorafs.SorafsReputationClientTest' \
  --console=plain

cd java/iroha_android && \
  ./gradlew :android:testDebugUnitTest \
  --tests 'org.hyperledger.iroha.android.client.okhttp.OkHttpClientProviderTests' \
  --console=plain

cd IrohaSwift && \
  swift test --filter SorafsReputationClientTests

dotnet test csharp/tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj \
  --filter FullyQualifiedName~SoraFsReputationClientTests

python3 -m pytest -q \
  scripts/tests/run_sorafs_reputation_rollout_evidence_test.py
```

Run the full workspace test suite when the validation budget permits.
