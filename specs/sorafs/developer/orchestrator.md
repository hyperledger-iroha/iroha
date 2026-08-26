---
title: SoraFS Orchestrator Configuration
summary: Configure the multi-source fetch orchestrator, interpret failures, and debug telemetry output.
---

> Public and in-depth documentation is maintained in the sibling
> `iroha-docs` repository and published at <https://docs.iroha.tech/>.

# Multi-Source Fetch Orchestrator Guide

The SoraFS multi-source fetch orchestrator drives deterministic, parallel
downloads from the provider set published in governance-backed adverts. This
guide explains how to configure the orchestrator, what failure signals to expect
during rollouts, and which telemetry streams expose health indicators.

## 1. Configuration Overview

The orchestrator merges three sources of configuration:

| Source | Purpose | Notes |
|--------|---------|-------|
| `OrchestratorConfig.scoreboard` | Normalises provider weights, validates telemetry freshness, and persists the JSON scoreboard used for audits. | Backed by `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | Applies runtime limits (retry budgets, concurrency bounds, verification toggles). | Maps to `FetchOptions` in `crates/sorafs_car::multi_fetch`. |
| CLI / SDK parameters | Cap the number of peers, attach telemetry regions, and surface deny/boost policies. | `sorafs_cli fetch` exposes these flags directly; SDKs thread them via `OrchestratorConfig`. |

The JSON helpers in `crates/sorafs_orchestrator::bindings` serialise the entire
configuration into Norito JSON, making it portable across SDK bindings and
automation.

### 1.1 Sample JSON Configuration

```json
{
  "scoreboard": {
    "latency_cap_ms": 6000,
    "weight_scale": 12000,
    "telemetry_grace_secs": 900,
    "persist_path": "/var/lib/sorafs/scoreboards/latest.json"
  },
  "fetch": {
    "verify_lengths": true,
    "verify_digests": true,
    "retry_budget": 4,
    "provider_failure_threshold": 3,
    "global_parallel_limit": 8
  },
  "telemetry_region": "iad-prod",
  "max_providers": 6,
  "transport_policy": "soranet-first"
}
```

Persist the file through the usual `iroha_config` layering (`defaults/`, user,
actual) so deterministic deployments inherit the same limits across nodes.
For a direct-only fallback profile that aligns with the SNNet-5a rollout,
consult `fixtures/documentation/sorafs_direct_mode_policy.json` and the companion
guidance in `specs/sorafs/direct_mode_pack.md`.

### 1.2 Compliance Overrides

SNNet-9 threads governance-driven compliance into the orchestrator. A new
`compliance` object in the Norito JSON configuration captures the carve-outs
that force the fetch pipeline into direct-only mode:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```

- `operator_jurisdictions` declares the ISO‑3166 alpha‑2 codes where this
  orchestrator instance operates. Codes are normalised to uppercase during
  parsing.
- `jurisdiction_opt_outs` mirrors the governance register. When any operator
  jurisdiction appears on the list, the orchestrator enforces
  `transport_policy=direct-only` and emits the policy fallback reason
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` lists manifest digests (blinded CIDs, encoded as
  uppercase hex). Matching payloads also force direct-only scheduling and
  surface the `compliance_blinded_cid_opt_out` fallback in telemetry.
- `audit_contacts` records the URIs governance expects operators to publish in
  their GAR playbooks.
- `attestations` captures the signed compliance packets backing the policy.
  Each entry defines an optional `jurisdiction` (ISO-3166 alpha-2 code), a
  `document_uri`, the canonical 64-character `digest_hex`, the issuance
  timestamp `issued_at_ms`, and an optional `expires_at_ms`. These artefacts
  flow into the orchestrator’s audit checklist so governance tooling can link
  overrides to the signed paperwork.

Provide the compliance block via the usual configuration layering so operators
receive deterministic overrides. The orchestrator applies compliance _after_
write-mode hints: even if an SDK requests `upload-pq-only`, jurisdictional or
manifest opt-outs still fall back to direct-only transport and fail fast when no
compliant providers exist.

Canonical opt-out catalogues live under
`governance/compliance/soranet_opt_outs.json`; the Governance Council publishes
updates via tagged releases. A complete example configuration (including
attestations) is available in `fixtures/documentation/sorafs_compliance_policy.json`, and
the operational process is captured in the
[GAR compliance playbook](../../soranet/gar_compliance_playbook.md).

### 1.3 CLI & SDK Knobs

| Flag / Field | Effect |
|--------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Limits how many providers survive the scoreboard filter. The production default is 64 and the hard maximum is 256; `None` is normalised to the finite default. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Caps attempts per chunk to `1..=16`. Unbounded programmatic configurations fail closed; exhausting the limit raises `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Injects latency/failure/reputation snapshots into the scoreboard builder. Stale telemetry beyond `telemetry_grace_secs` marks providers ineligible. |
| `--scoreboard-out` | Persists the computed scoreboard (eligible + ineligible providers) for post-run inspection. |
| `--scoreboard-now` | Overrides the scoreboard timestamp (Unix seconds) so fixture captures remain deterministic. |
| `--deny-provider` / score policy hook | Deterministically exclude providers from scheduling without deleting adverts. Useful for fast-response blacklisting. |
| `--boost-provider=name:delta` | Adjust the weighted round-robin credits for a provider while leaving governance weights untouched. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Labels emitted metrics and structured logs so dashboards can pivot by geography or rollout wave. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | Defaults to `soranet-first` now that the multi-source orchestrator is the baseline. Use `direct-only` only when staging a downgrade or following a compliance directive, and reserve `soranet-strict` for PQ-only pilots; compliance overrides remain the hard ceiling. |

SoraNet-first is now the default, and rollbacks must be documented explicitly. When SNNet-4/5/5a/5b/6a/7/8/12/13 graduate, governance will ratchet the required posture forward (towards `soranet-strict`); until then, only incident-driven overrides should force `direct-only`, and they must cite the corresponding SNNet blocker in the rollout log.

All flags above accept `--`-style syntax in both `sorafs_cli fetch` and the
developer-facing `sorafs_fetch` binary. SDKs expose the same options via typed
builders.

Telemetry records may include `reputation_score_bps` (`0..=10000`) from a
validated `ReputationSnapshotV1`. The scoreboard treats this as a multiplicative
weight component: lower reputation reduces scheduling share, while explicit
telemetry penalties remain the hard-exclusion path. `sorafs_fetch` rejects
out-of-range reputation values during telemetry JSON parsing. JavaScript native
host callers, the Rust-backed Python bridge, and `connect_norito_bridge`
options JSON use the same optional field and bound. See
`specs/sorafs/reputation_operator.md` for snapshot generation and proof
verification.

### 1.4 Guard Cache Management

The CLI now wires in the SoraNet guard selector so operators can pin entry
relays deterministically ahead of the full SNNet-5 transport rollout. Three
new flags control the workflow:

| Flag | Purpose |
|------|---------|
| `--guard-directory <PATH>` / `--guard-directory-digest <HEX>` | Points to the Norito snapshot and supplies the independently distributed, domain-separated BLAKE3 digest of those exact bytes. Both are required before the directory may refresh the guard cache. |
| `--guard-cache <PATH>` | Persists the authenticated Norito-encoded `GuardSet`. It is accepted only together with `--guard-cache-key-file` and a current `--guard-directory` plus independently trusted digest. Cached state is never a directory freshness or membership trust anchor. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Optional overrides for the number of entry guards to pin (default 3) and the retention window (default 30 days). |
| `--guard-cache-key-file <PATH>` | Owner-private file containing exactly 32 raw non-zero bytes used to authenticate every persisted guard cache with a keyed BLAKE3 tag. It is accepted only together with `--guard-cache`; raw key material is never accepted on argv. |

The first-release cache is a version-8 authenticated envelope. The loader caps
the file at 2 MiB, authenticates the opaque inner payload before decoding guard
records, permits at most 16 unique guards, and rejects non-canonical or invalid
persisted relay identity, endpoint, tag, weight, region, and certificate fields. There is no
unsigned persistence mode. On Unix, cache admission additionally requires an
owner-private single-link regular file under an invoking-user-owned directory
that is not group/world writable. Persistence uses a mode-0600 staging file and
same-directory atomic rename. Symlink, hard-link, permissive-file, and writable
parent paths fail closed; platforms without equivalent owner/mode/link custody
checks do not expose persistent guard caches. The key file is subject to the
same direct-file custody rules and must contain raw bytes, not hexadecimal text
or a newline-terminated value.

Guard directory payloads use a compact schema:

The `--guard-directory` flag now expects a Norito-encoded
`GuardDirectorySnapshotV2` payload. The binary snapshot contains:

- `version` — schema version (currently `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` — consensus
  metadata that must match every embedded certificate.
- `issuers` — governance issuers with `fingerprint`, `ed25519_public`, and `mldsa65_public`.
  Fingerprints are computed as
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — a list of SRCv2 bundles (`RelayCertificateBundleV2::try_to_cbor()` output). Each bundle
  carries the relay descriptor, capability flags, authenticated NK2/NK3 suite ordering, and dual
  Ed25519/ML-DSA-65 signatures. Static relay KEM keys and KEM-rotation policies are not part of SRCv2.
Snapshots are rejected if the version is unknown or if the validity window is
inconsistent (`valid_after_unix > valid_until_unix` or
`published_at_unix > valid_until_unix`). Runtime authentication also requires
`valid_after_unix <= now < valid_until_unix`.
Embedded issuer keys prove only signature self-consistency; the independently
obtained exact snapshot digest authenticates the issuer set. SRCv2 decoding
rejects oversized containers before allocation. The first-release directory
envelope is limited to 5 MiB, 16 issuers, 64 relays, 1,952 bytes per issuer
ML-DSA-65 public key, and 64 KiB per signed bundle; each bundle permits at most
16 endpoints, 8 tags per endpoint, 2 handshake suites, 2,048 UTF-8 bytes per
URL, 64 bytes per tag, and a 56 KiB certificate. Local node, relay, and CLI
consumers open only a direct regular file without following its final path
component, pin file identity and length across a max-plus-one bounded read, and
then enter the schema-specific Norito decode budget. CLI HTTP fetches reject a
declared `Content-Length` above 5 MiB before reading and apply the same
max-plus-one bound while streaming chunked or otherwise lengthless responses.

The CLI verifies every bundle against the declared issuer keys before merging the directory with

Invoke the CLI with `--guard-directory` to merge the latest consensus with the
existing cache. The selector preserves pinned guards that are still within the
retention window and eligible in the directory; new relays replace expired
entries. After a successful fetch the updated cache is written back to the path
supplied via `--guard-cache`, keeping subsequent sessions deterministic while
each run reconciles it against a current authenticated directory. SDKs can
reproduce the same behaviour by calling
`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)` and
threading the resulting `GuardSet` and the same authenticated `RelayDirectory`
through `SorafsGatewayFetchOptions`. Programmatic fetches fail closed when a
guard set has no directory or the directory is unauthenticated, not yet valid,
or expired, even when circuit lifecycle management is disabled.

The authenticated NK2/NK3 suite list enables the selector to identify
PQ-capable guards. Stage policies (`anon-guard-pq`, `anon-majority-pq`,
`anon-strict-pq`) now demote classical relays automatically: when a PQ guard is
available the selector drops excess classical pins so subsequent sessions favour
hybrid handshakes. CLI/SDK summaries surface the resulting mix via
`anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`,
`anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio`, and the companion candidate/deficit/supply delta
fields, making brownouts and classical fallbacks explicit.

Guard directories may now embed a complete SRCv2 bundle via
`certificate_base64`. The orchestrator decodes every bundle, re-validates the
Ed25519/ML-DSA signatures, and retains the parsed certificate alongside the
guard cache. When a certificate is present it becomes the canonical source for
PQ handshake capability, handshake suite preferences, and weighting; expired
certificates are ignored for PQ selection. Guard caches never persist static
relay KEM material. `telemetry::sorafs.guard` and
`telemetry::sorafs.circuit` record the validity window, handshake suites, and
whether dual signatures were observed for each guard.

Use the CLI subcommands to keep snapshots in sync with the directory publisher:

```bash
# Download + verify the latest snapshot before refreshing the cache
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-snapshot-digest <trusted-snapshot-digest-hex>

# Re-verify a snapshot shipped by another team or via an artefact repository
sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-snapshot-digest <trusted-snapshot-digest-hex>
```

Fetch verifies every SRCv2 bundle, exact snapshot digest, and current validity
window before writing to disk. The embedded `directory_hash` binds relay
certificates to consensus metadata but is not an authentication pin.
`guard-directory inspect --path ...` is available for local structural
diagnostics and digest calculation; its summary is explicitly labelled
`structural_inspection_only`. Verification emits a structured summary
(`version`, snapshot digest, authentication state, validity window, relay
counts, PQ ratios) so operators can capture audit logs alongside the snapshot.

### 1.5 Circuit Lifecycle Manager

When both a relay directory and guard cache are provided, the orchestrator
activates the circuit lifecycle manager to pre-build and renew SoraNet circuits
before each fetch. Configuration lives in `OrchestratorConfig`
(`crates/sorafs_orchestrator/src/lib.rs:305`) via two new fields:

- `relay_directory`: carries the SNNet-3 directory snapshot so middle/exit hops
  can be selected deterministically.
- `circuit_manager`: optional configuration (enabled by default) controlling the
  circuit TTL.

Norito JSON now accepts a `circuit_manager` block:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

SDKs forward directory data through
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), and the CLI wires it automatically whenever
`--guard-directory` is supplied (`crates/iroha_cli/src/commands/sorafs.rs:365`).

The manager accepts only a directory whose half-open validity window contains
the selection time, and every circuit expiry is capped at the directory's
`valid_until`. It renews circuits whenever the active anonymity policy changes,
the TTL elapses, or any entry, middle, or exit descriptor is removed or changes
role, endpoints, authenticated handshake suites, or certificate validity. The helper `refresh_circuits`
invoked ahead of each fetch (`crates/sorafs_orchestrator/src/lib.rs:1346`)
emits `CircuitEvent` logs so operators can trace lifecycle decisions. The soak
test `circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) demonstrates stable latency
across three guard rotations; see the accompanying report at
`specs/soranet/reports/circuit_stability.md:1`.

### 1.6 Relay privacy feed boundary

Provider-advertised `privacy_events_url` values are untrusted network input. The
collector accepts only canonical HTTPS URLs without credentials, query strings,
or fragments. Literal and DNS-resolved loopback, private, link-local,
unspecified, multicast, documentation, and reserved addresses are rejected; a
mixed public/private DNS answer rejects the entire endpoint. Accepted DNS
answers are pinned into a dedicated no-proxy client for the collector lifetime,
redirects and implicit retries are disabled, and each response is capped at
1 MiB, 4096 NDJSON records, and 16 KiB per record. At most 64 provider feeds run
for a fetch session. These checks also apply to the derived
`/policy/proxy-toggle` request because it reuses the validated host and pinned
client.

### 1.7 Local QUIC Proxy

The orchestrator can optionally spawn a local QUIC proxy so browser extensions
and SDK adapters do not have to manage certificates or guard cache keys. The
proxy binds to a loopback address and generates a fresh 256-bit client
capability on every start. Possession of that capability, not loopback location,
authorises use of the proxy. Callers obtain it from
`LocalQuicProxyHandle::export_client_capability()` or the handle's out-of-band
bootstrap manifest. Guard cache keys remain proxy-local and are never included
in either manifest.
Transport events emitted by the proxy are counted via
`sorafs_orchestrator_transport_events_total`.

Enable the proxy through the new `local_proxy` block in the orchestrator JSON:

```json
"local_proxy": {
  "bind_addr": "127.0.0.1:9443",
  "telemetry_label": "dev-proxy",
  "guard_cache_key_hex": "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
  "emit_browser_manifest": true,
  "proxy_mode": "bridge",
  "prewarm_circuits": true,
  "max_streams_per_circuit": 64,
  "circuit_ttl_hint_secs": 300,
  "norito_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito"
  },
  "kaigi_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito",
    "room_policy": "public"
  }
}
```

- `bind_addr` controls which loopback address the proxy listens on (use `0`
  port to request an ephemeral port). Non-loopback bind addresses are
  rejected so the local proxy cannot be exposed as a remotely reachable
  bridge.
- `telemetry_label` propagates into metrics so dashboards can distinguish
  proxies from fetch sessions.
- `guard_cache_key_hex` (optional) supplies proxy-local key material for
  session cache-tag HMACs. It is redacted from diagnostics and is never exposed
  through the browser manifest.
- `emit_browser_manifest` toggles whether the handshake returns a manifest that
  extensions can store and validate.
- `proxy_mode` selects whether the proxy bridges traffic locally (`bridge`) or
  only emits metadata so SDKs can open SoraNet circuits themselves
  (`metadata-only`). The proxy defaults to `bridge`; set `metadata-only` when a
  workstation should expose the manifest without relaying streams.
- `prewarm_circuits`, `max_streams_per_circuit`, and `circuit_ttl_hint_secs`
  surface additional hints to the browser so it can budget parallel streams and
  understand how aggressively the proxy reuses circuits. The stream limit is
  enforced by QUIC and must be within `1..=256`; it is no longer advisory.
- `car_bridge` (optional) points at a local CAR archive cache. The `extension`
  field controls the suffix appended when the stream target omits `*.car`; set
  `allow_zst = true` to serve pre-compressed `*.car.zst` payloads directly.
- `kaigi_bridge` (optional) exposes spooled Kaigi routes to the proxy. The
  `room_policy` field advertises whether the bridge operates in `public` or
  `authenticated` mode so browser clients can preselect the correct GAR labels.
- `sorafs_cli fetch` exposes `--local-proxy-mode=bridge|metadata-only` and
  `--local-proxy-norito-spool=PATH` overrides, letting operators toggle the
  runtime mode or point at alternate spools without modifying the JSON policy.
- `downgrade_remediation` (optional) configures the automatic downgrade hook.
  When enabled the orchestrator watches relay telemetry for downgrade bursts
  and, after the configured `threshold` within `window_secs`, forces the local
  proxy into the `target_mode` (default `metadata-only`). Once downgrades stop
  the proxy reverts to `resume_mode` after `cooldown_secs`. Use the `modes`
  array to scope the trigger to specific relay roles (defaults to entry relays).

When the proxy runs in bridge mode it serves four application services:

- **`tcp`** – accepts only a canonical public `host:port` authority. The proxy
  resolves the name once, rejects any private/reserved or mixed DNS answer, and
  connects directly to the pinned address set. This prevents the loopback-bound
  listener from becoming an SSRF tunnel into localhost, cloud metadata, or the
  operator LAN.

- **`norito`** – the client’s stream target is resolved relative to
  `norito_bridge.spool_dir`. Targets are sanitised (no traversal, no absolute
  paths), and when the file lacks an extension the configured suffix is applied
  before the payload is streamed verbatim to the browser.
- **`car`** – stream targets resolve inside `car_bridge.cache_dir`, inherit the
  configured default extension, and reject compressed payloads unless
  `allow_zst` is set. Successful bridges reply with `STREAM_ACK_OK` before
  transferring the archive bytes so clients can pipeline verification.
- **`kaigi`** – streams an authenticated/public room spool using the same
  filesystem rules as `norito` while reporting the configured room policy.

All filesystem services canonicalise a non-symlink root at startup and fail
closed on platforms that cannot enforce Unix custody. Every root and descendant
must be owned by the proxy's effective UID and not be group/world writable;
root-owned read-only ancestors and a root-owned sticky boundary with an
owner-held protected child are permitted. Served files must also have exactly
one hard link. The proxy rejects symlink components and non-regular files, caps
payloads at 1 GiB, and streams exactly the opened file length with backpressure.
Inode, length, and timestamp identity is checked before and after transfer so
replacement or truncation closes the stream. Client errors never disclose the
resolved host filesystem path. This custody model treats other processes under
the same UID and the system administrator as trusted; do not configure a bridge
root governed by a writable ACL that grants either authority to a less-trusted
principal.

Protocol guardrails for the first release:

- Handshake and stream-open frames are capped at 64 KiB and rejected when the
  length prefix exceeds the limit.
- Control fields are capped at 1024 bytes, 0-RTT is disabled, at most 128
  connections are serviced concurrently, connects time out after 10 seconds,
  and a stream is terminated after the five-minute transfer ceiling.
- Handshake frames must set `version = 1` and carry the 32-byte
  `client_capability` decoded from the out-of-band bootstrap value. Capability
  comparison is constant-time. Missing or incorrect credentials receive only a
  generic rejected acknowledgement with no manifest, and no application stream
  is accepted before authentication. TLS 0-RTT is disabled, so an encrypted
  handshake frame cannot be replayed as early data on a new connection.
- Stream-open frames must set `version = 1`; mismatches return
  `STREAM_ACK_UNSUPPORTED_VERSION`.

In both cases the proxy computes and supplies the cache-tag HMAC when a
proxy-local guard cache key is configured; the key itself is never sent during
the handshake. It records `norito_*` / `car_*` telemetry reason
codes so dashboards can differentiate successes, missing files, and sanitisation
failures at a glance.

`Orchestrator::local_proxy().await` exposes the running handle so callers can
read the certificate PEM, export the client capability, fetch the out-of-band
browser bootstrap manifest, or request a graceful shutdown when the application
exits. The capability is a bearer secret for the lifetime of that proxy start;
do not log it or pass it to another local user. Restart the proxy to rotate it.

When enabled, the proxy now serves **manifest v2** records. Besides the existing
certificate, v2 adds:

- `client_capability_hex`, a 64-character lowercase bootstrap secret. It is
  present only in the handle/out-of-band manifest and is always absent from the
  in-band handshake acknowledgement.
- `alpn` (`"sorafs-proxy/1"`) and a `capabilities` array so clients can confirm
  the stream protocol they should speak.
- A per-handshake `session_id` and cache-tagging salt (`cache_tagging` block) to
  derive per-session guard affinities and HMAC tags.
- Circuit and guard-selection hints (`circuit`, `guard_selection`,
  `route_hints`) so browser integrations can expose richer UI before streams are
  opened.
- `telemetry_v2` with sampling and privacy knobs for local instrumentation.
- Each `STREAM_ACK_OK` includes `cache_tag_hex`. Clients mirror the value in
  the `x-sorafs-cache-tag` header when issuing HTTP or TCP requests so cached
  guard selections remain encrypted at rest.

continue to rely on the v1 subset.

## 2. Failure Semantics

The orchestrator enforces strict capability and budget checks before a single
byte is transferred. Failures fall into three categories:

1. **Eligibility failures (pre-flight).** Providers missing range capability,
   expired adverts, or stale telemetry are logged in the scoreboard artefact and
   omitted from scheduling. CLI summaries populate the `ineligible_providers`
   array with reasons so operators can inspect governance drift without scraping
   logs.
2. **Runtime exhaustion.** Each provider tracks consecutive failures. Once the
   configured `provider_failure_threshold` is reached, the provider is marked
   `disabled` for the remainder of the session. If every provider transitions to
   `disabled`, the orchestrator returns
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Deterministic aborts.** Hard limits surface as structured errors:
   - `MultiSourceError::NoCompatibleProviders` — the manifest requires a chunk
     span or alignment the remaining providers cannot honour.
   - `MultiSourceError::ExhaustedRetries` — the per-chunk retry budget was
     consumed.
   - `MultiSourceError::ObserverFailed` — downstream observers (streaming hooks)
     rejected a verified chunk.

Every error embeds the offending chunk index and, when available, the final
provider failure reason. Treat these as release blockers—retries with the same
input will reproduce the failure until the underlying advert, telemetry, or
provider health changes.

### 2.1 Scoreboard Persistence

When `persist_path` is configured, the orchestrator writes the final scoreboard
after each run. The JSON document contains:

- `eligibility` (`eligible` or `ineligible::<reason>`).
- `weight` (normalised weight assigned for this run).
- `provider` metadata (identifier, endpoints, concurrency budget).

Archive scoreboard snapshots alongside release artefacts so blacklisting and
rollout decisions remain auditable.

## 3. Telemetry & Debugging

### 3.1 Prometheus Metrics

The orchestrator emits the following metrics via `iroha_telemetry`:

| Metric | Labels | Description |
|--------|--------|-------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Gauge of in-flight orchestrated fetches. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Histogram recording end-to-end fetch latency. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Counter of terminal failures (retries exhausted, no providers, observer failure). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Counter of retry attempts per provider. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Counter of session-level provider failures leading to disablement. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Count of anonymity policy decisions (met vs. brownout) grouped by rollout stage and fallback reason. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Histogram of PQ relay share among the selected SoraNet set. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Histogram of PQ relay supply ratios in the scoreboard snapshot. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Histogram of the policy shortfall (gap between target and actual PQ share). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | Histogram of classical relay share used in each session. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Histogram of classical relay counts selected per session. |

Integrate the metrics into staging dashboards before flipping production knobs.
The recommended layout mirrors the SF-6 observability plan:

1. **Active fetches** — alerts if the gauge climbs without matching completions.
2. **Retry ratio** — warns when `retry` counters exceed historical baselines.
3. **Provider failures** — triggers pager alerts when any provider crosses
   `session_failure > 0` within 15 minutes.

### 3.2 Structured Log Targets

The orchestrator publishes structured events to deterministic targets:

- `telemetry::sorafs.fetch.lifecycle` — `start` and `complete` lifecycle
  markers with chunk counts, retries, and total duration.
- `telemetry::sorafs.fetch.retry` — retry events (`provider`, `reason`,
  `attempts`) for feeding into manual triage.
- `telemetry::sorafs.fetch.provider_failure` — providers disabled due to
  repeated errors.
- `telemetry::sorafs.fetch.error` — terminal failures summarised with
  `reason` and optional provider metadata.

Forward these streams to the existing Norito log pipeline so incident response
has a single source of truth. Lifecycle events expose the PQ/classical mix via
`anonymity_effective_policy`, `anonymity_pq_ratio`,
`anonymity_classical_ratio`, and their companion counters,
making it straightforward to wire dashboards without scraping metrics. During
GA rollouts, pin the log level to `info` for lifecycle/retry events and rely on
`warn` for terminal errors.

### 3.3 JSON Summaries

Both `sorafs_cli fetch` and the Rust SDK return a structured summary containing:

- `provider_reports` with success/failure counts and whether a provider was
  disabled.
- `chunk_receipts` detailing which provider satisfied each chunk.
- `retry_stats` and `ineligible_providers` arrays.

Archive the summary file when debugging misbehaving providers—the receipts map
directly to the log metadata above.

## 4. Operational Checklist

1. **Stage configuration in CI.** Run `sorafs_fetch` with the target
   configuration, pass `--scoreboard-out` to capture the eligibility view, and
   diff against the previous release. Any unexpected ineligible provider halts
   the promotion.
2. **Validate telemetry.** Ensure the deployment exports `sorafs.fetch.*`
   metrics and structured logs before enabling multi-source fetches for users.
   The absence of metrics typically indicates the orchestrator facade was not
   invoked.
3. **Document overrides.** When applying emergency `--deny-provider` or
   `--boost-provider` settings, commit the JSON (or CLI invocation) to your
   change log. Rollbacks must revert the override and capture a new scoreboard
   snapshot.
4. **Re-run smoke tests.** After modifying retry budgets or provider caps,
   re-fetch the canonical fixture (`fixtures/sorafs_manifest/ci_sample/`) and
   verify chunk receipts remain deterministic.

Following the steps above keeps orchestrator behaviour reproducible across
staged rollouts and provides the telemetry necessary for incident response.

### 4.1 Policy Overrides

Operators can pin the active transport/anonymity stage without editing the
base configuration by setting `policy_override.transport_policy` and
`policy_override.anonymity_policy` in their `orchestrator` JSON (or supplying
`--transport-policy-override=` / `--anonymity-policy-override=` to
`sorafs_cli fetch`). When either override is present the orchestrator skips the
usual brownout fallback: if the requested PQ tier cannot be satisfied, the
fetch fails with `no providers` instead of quietly downgrading. Rollback to the
default behaviour is as simple as clearing the override fields.

The standard `iroha_cli app sorafs fetch` command exposes the same override flags,
forwarding them to the gateway client so ad-hoc fetches and automation scripts
share the identical stage pinning behaviour.

Cross-SDK fixtures live under `fixtures/sorafs_gateway/policy_override/`. The
CLI, Rust client, JavaScript bindings, and Swift harness decode
`override.json` in their parity suites so any change to the override payloads
must update that fixture and re-run `cargo test -p iroha`, `npm test`, and
`swift test` to keep the SDKs aligned. Always attach the regenerated fixture to
change review so downstream consumers can diff the override contract.

Governance requires a runbook entry for every override. Record the reason,
expected duration, and rollback trigger in your change log, notify the PQ
ratchet rotation channel, and append the signed approval to the same artifact
bundle that stores the scoreboard snapshot. Overrides are intended for short
emergencies (e.g., PQ guard brownouts); long-running policy changes must go
through the normal council vote so nodes converge on the new default.

### 4.2 PQ Ratchet Fire Drill

- **Runbook:** Follow `specs/soranet/pq_ratchet_runbook.md` for the
  promotion/demotion rehearsal, including guard-directory handling and rollback.
- **Dashboard:** Import `dashboards/grafana/soranet_pq_ratchet.json` to monitor
  `sorafs_orchestrator_policy_events_total`, brownout rate, and PQ ratio mean
  during the drill.
- **Automation:** `cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics`
  exercises the same transitions and verifies that metrics increment as
  expected before operators run the live drill.

## 5. Rollout Playbooks

The SNNet-5 transport rollout introduces new guard selection, governance
attestations, and policy fallbacks. The playbooks below codify the sequence to
follow before enabling multi-source fetches for end users, plus the downgrade
path back to direct mode.

### 5.1 Developer Pre-Flight (CI / Staging)

1. **Regenerate scoreboards in CI.** Run `sorafs_cli fetch` (or the SDK
   equivalent) against the `fixtures/sorafs_manifest/ci_sample/` manifest with
   the candidate configuration. Persist the scoreboard via
   `--scoreboard-out=artifacts/sorafs/scoreboard.json` and assert:
   - `anonymity_status=="met"` and the `anonymity_pq_ratio` meets the targeted
     stage (`anon-guard-pq`, `anon-majority-pq`, or `anon-strict-pq`).
   - The deterministic chunk receipts still match the golden set committed to
     the repository.
2. **Verify manifest governance.** Inspect the CLI / SDK summary and ensure the
   newly surfaced `manifest_governance.council_signatures` array contains the
   expected Council fingerprints. This confirms gateway responses ship the GAR
   envelope and that `validate_manifest` accepted it.
3. **Exercise compliance overrides.** Load each jurisdictional profile from
   `fixtures/documentation/sorafs_compliance_policy.json` and assert the orchestrator
   emits the correct policy fallback (`compliance_jurisdiction_opt_out` or
   `compliance_blinded_cid_opt_out`). Record the resulting fetch failure when no
   compliant transports are available.
4. **Simulate downgrade.** Flip `transport_policy` to `direct-only` in the
   configuration under test and re-run the fetch to ensure the orchestrator
   falls back to Torii/QUIC without touching SoraNet relays. Keep this JSON
   variant under version control so it can be promoted rapidly during an
   incident.

### 5.2 Operator Rollout (Production Waves)

1. **Stage the configuration via `iroha_config`.** Publish the exact JSON used
   in CI as an `actual` layer override. Confirm the orchestrator pod / binary
   logs the new configuration hash on startup.
2. **Prime guard caches.** Refresh the relay directory via `--guard-directory`
   and persist the Norito guard cache with `--guard-cache` plus its required
   `--guard-cache-key-file`. Verify the authenticated version-8 envelope and
   store it under versioned change control without logging or archiving the
   owner-private key file.
3. **Enable telemetry dashboards.** Before serving user traffic, ensure the
   environment publishes `sorafs.fetch.*`, `sorafs_orchestrator_policy_events_total`, and
   the proxy metrics (when using the local QUIC proxy). Alarms should be tied to
   `anonymity_brownout_effective` and compliance fallback counters.
4. **Run live smoke tests.** Fetch a governance-approved manifest through each
   provider cohort (PQ, classical, and direct) and confirm chunk receipts,
   CAR digests, and council signatures match the CI baseline.
5. **Communicate activation.** Update the rollout tracker with the
   `scoreboard.json` artefact, the guard cache fingerprint, and a link to the
   logs showing manifest governance verification for the first production fetch.

### 5.3 Downgrade / Rollback Procedure

When incidents, PQ deficits, or regulatory requests force a rollback, follow
this deterministic sequence:

1. **Switch transport policy.** Apply `transport_policy=direct-only` (and, if
   immediately halts new SoraNet circuit construction.
2. **Flush guard state.** Delete or archive the guard cache file referenced by
   `--guard-cache` so subsequent runs do not attempt to reuse pinned relays.
   Skip this step only when a rapid re-enable is planned and the cache remains
   valid.
3. **Disable local proxies.** If the local QUIC proxy was in `bridge` mode,
   restart the orchestrator with `proxy_mode="metadata-only"` or remove the
   `local_proxy` block entirely. Document the port release so workstation and
   browser integrations revert to direct Torii access.
4. **Clear compliance overrides.** Append a jurisdictional opt-out entry (or a
   blinded-CID entry) to the compliance policy for the affected payloads so
   automation and dashboards reflect the intentional direct-mode operation.
5. **Capture audit evidence.** Run a post-change fetch with `--scoreboard-out`
   and store the CLI JSON summary (including `manifest_governance`) alongside
   the incident ticket.

### 5.4 Regulated Deployment Checklist

| Checkpoint | Purpose | Recommended Evidence |
|------------|---------|----------------------|
| Compliance policy staged | Confirms the jurisdictional carve-out aligns with GAR filings. | Signed `soranet_opt_outs.json` snapshot + orchestrator config diff. |
| Manifest governance recorded | Proves Council signatures accompany every gateway manifest. | `sorafs_cli fetch ... --output /dev/null --summary out.json` with `manifest_governance.council_signatures` archived. |
| Attestation inventory | Tracks the documents referenced in `compliance.attestations`. | Store PDFs/JSON artefacts alongside the attestation digest and expiry. |
| Downgrade drill logged | Ensures rollback remains deterministic. | Quarterly dry-run record showing direct-only policy applied and guard cache cleared. |
| Telemetry retention | Provides forensic data for regulators. | Dashboard export or OTEL snapshot confirming `sorafs.fetch.*` and compliance fallbacks are being retained per policy. |

Operators should review the checklist prior to each rollout window and furnish
the evidence pack to governance or regulators on request. Developers can reuse
the same artefacts for postmortem packets when brownouts or compliance overrides
are triggered during testing.
