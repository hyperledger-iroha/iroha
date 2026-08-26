# SoraFS Pin Registry Operations

This runbook documents how to monitor and triage the SoraFS pin registry and its replication
service-level agreements (SLAs). Torii publishes only the consensus-maintained global pin
accounting summary as periodic inventory telemetry. It does not scan registry collections to
reconstruct lifecycle, alias, replication-order, or SLA aggregates. Exact retained-record and
live-content-byte charges come from the same O(1) summary exposed by
`PinManifestPageV1.charged_usage` at a finalized height/hash. Import the curated dashboard
(`specs/grafana_sorafs_pin_registry.json`) for the two supported global gauges; use bounded,
finalized registry queries and GovernanceLog DAG records for detailed triage.

> Public and in-depth documentation is maintained in the sibling
> `iroha-docs` repository and published at <https://docs.iroha.tech/>.

## Metric reference

| Metric | Labels | Description |
| ------ | ------ | ----------- |
| `torii_sorafs_pin_retained_manifests` | — | Consensus-maintained count of retained pin lifecycle records, including retired records retained in state. |
| `torii_sorafs_pin_live_content_bytes` | — | Consensus-maintained content bytes represented by live pins. |

Both gauges are read in O(1) from finalized consensus accounting. They are not substitutes for
status-specific registry queries.

## Authoritative pin accounting and readback

- `GET /v1/sorafs/pin` returns bounded `PinManifestSummaryV1` rows in ascending
  digest order. `limit` is `1..=256` and `max_bytes` is
  `1024..=262144`; both ceilings apply to every response.
- Continue only with the returned non-zero `next_after_digest` as the exclusive
  `after_digest_hex` value. Repeat the page's paired
  `finalized_cursor.height` and `finalized_cursor.block_hash` as
  `expected_finalized_height` and
  `expected_finalized_block_hash_hex`. A stale anchor returns HTTP 409. Offset
  pagination is not part of the first-release pin-list contract.
- Read `charged_usage.manifest_count` and `charged_usage.content_bytes` for the
  authoritative charge at that anchor. `manifest_count` includes retired
  lifecycle records that remain in consensus state; `content_bytes` includes
  live pins. Core maintains both transactionally with global/per-account
  admission, manual retirement, and consensus-time expiry; do not sum a full
  page set or scrape Prometheus to reconstruct them.
- Use `GET /v1/sorafs/pin/{digest_hex}` for one exact
  `PinManifestFinalizedRecordV1`. Alias proofs, metadata, council-envelope
  commitment, and fee-payment detail are deliberately absent from list rows;
  the exact route is bounded to a single admitted record and accepts only the
  optional paired finalized precondition.

## Grafana dashboard

The dashboard JSON ships with two panels:

1. **Retained pin lifecycle records** – `torii_sorafs_pin_retained_manifests`.
2. **Live pinned content bytes** – `torii_sorafs_pin_live_content_bytes`.

Do not derive alerts about lifecycle status, alias counts, replication backlog, expiry, or SLA
latency from these totals. Export the bounded finalized registry pages at a fixed height/hash,
derive the operational report outside request handling, and retain the query anchor with the
report. Add consensus-maintained counters before introducing any always-on Prometheus alert that
requires those dimensions.

## Triage workflow

1. **Identify cause**
   - Export replication orders at one finalized height/hash and separate pending, completed, expired, and pin-retirement-cancelled records.
   - Correlate late or expired orders with PoR failures and provider availability.
2. **Validate provider status**
   - Run `iroha app sorafs providers list` and verify the advertised capabilities match replication requirements.
   - Check `torii_sorafs_capacity_*` gauges to confirm provisioned GiB and PoR success.
3. **Reassign replication**
   - Issue new orders via `sorafs_manifest_builder capacity replication-order` when the finalized export shows fewer than 5 epochs of deadline slack (manifest/CAR packaging uses `iroha app sorafs toolkit pack`).
   - Notify governance if the finalized alias query shows bindings without an active manifest.
4. **Document outcome**
   - Record incident notes in the SoraFS operations log with timestamps and affected manifest digests.
   - Update this runbook if new failure modes or dashboards are introduced.

## Rollout plan

Follow this staged procedure when enabling or tightening the alias cache policy in production:

1. **Prepare configuration**
   - Update `torii.sorafs_alias_cache` in `iroha_config` (user → actual) with the agreed TTLs and
     grace windows: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`,
     `revocation_ttl`, `rotation_max_age`, `successor_grace`, and `governance_grace`. The
     defaults match the policy in `specs/sorafs_alias_policy.md`.
   - For SDKs, distribute the same values through their configuration layers
     (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation,
     successor, governance)` in Rust / NAPI / Python bindings) so client enforcement matches the
     gateway.
2. **Dry-run in staging**
   - Deploy the config change to a staging cluster that mirrors production topology.
   - Run `cargo xtask sorafs-pin-fixtures` to confirm the canonical alias fixtures still decode and
     round-trip; any mismatch implies upstream manifest drift that must be addressed first.
   - Exercise `/v1/sorafs/pin/{digest_hex}` with no cursor, a matching paired
     `expected_finalized_height`/`expected_finalized_block_hash_hex`, and a
     stale pair. Require exact `PinManifestFinalizedRecordV1` JSON, `no-store`,
     and HTTP 409 for the stale cursor.
   - Exercise `/v1/sorafs/pin` with the minimum and maximum row/byte limits,
     each lifecycle filter, and at least two pages. Require ascending bounded
     summaries, exclusive `next_after_digest` continuation under the same
     height/hash pair, exact `charged_usage`, `no-store`, and HTTP 409 after
     substituting either finalized coordinate. Require rejection of offsets,
     duplicate/unknown selectors, unpaired anchors, zero cursors, and pages
     whose encoded rows exceed `max_bytes`.
   - Exercise `/v1/sorafs/aliases` separately with a fresh exact-network
     canonical-account request signature and synthetic proofs covering fresh,
     refresh-window, expired, and hard-expired cases. Validate its HTTP status
     codes, `Sora-Proof-Status`, `Retry-After`, `Warning`, and JSON body fields
     against this runbook; unsigned requests must fail before inventory
     materialization.
3. **Enable in production**
   - Roll out the new configuration via the standard change window. Apply it to Torii first, then
     restart gateways/SDK services once the node confirms the new policy in logs.
   - Import `specs/grafana_sorafs_pin_registry.json` into Grafana (or update existing dashboards)
     and pin the alias cache refresh panels to the NOC workspace.
4. **Post-deployment verification**
   - Monitor `torii_sorafs_alias_cache_refresh_total` and
     `torii_sorafs_alias_cache_age_seconds` for 30 minutes. Spikes in the `error`/`expired` curves
     should correlate with policy refresh windows; unexpected growth means operators must inspect
     alias proofs and provider health before continuing.
   - Confirm client-side logs show the same policy decisions (SDKs will surface errors when the proof
     is stale or expired). Absence of client warnings indicates a misconfiguration.
5. **Fallback**
   - If alias issuance falls behind and the refresh window trips frequently, temporarily relax the
     policy by increasing `refresh_window` and `positive_ttl` in config, then redeploy. Keep
     `hard_expiry` intact so truly stale proofs are still rejected.
   - Revert to the prior configuration by restoring the previous `iroha_config` snapshot if telemetry
     continues to show elevated `error` counts, then open an incident to trace alias generation
     delays.

## Related materials

- `specs/sorafs/pin_registry_plan.md` — implementation roadmap and governance context.
- `specs/sorafs/runbooks/sorafs_node_ops.md` — storage worker operations, complements this registry playbook.
