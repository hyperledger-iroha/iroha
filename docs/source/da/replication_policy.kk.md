---
lang: kk
direction: ltr
source: docs/source/da/replication_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 70163ed6740c80c48c78ae918c37d34e0022ab97ffabce6d451bbf85060e24b4
source_last_modified: "2026-01-22T14:35:37.691616+00:00"
translation_last_reviewed: 2026-02-07
---

# Data Availability Replication Policy (DA-4)

_Status: In Progress — Owners: Core Protocol WG / Storage Team / SRE_

The DA ingestion pipeline now enforces deterministic retention targets for
every blob class described in `roadmap.md` (workstream DA-4). Torii refuses to
persist caller-provided retention envelopes that do not match the configured
policy, guaranteeing that every validator/storage node retains the required
number of epochs and replicas without relying on submitter intent.

## Default policy

| Blob class | Hot retention | Cold retention | Required replicas | Storage class | Governance tag |
|------------|---------------|----------------|-------------------|----------------|----------------|
| `taikai_segment` | 24 hours | 14 days | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 hours | 7 days | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 hours | 180 days | 3 | `cold` | `da.governance` |
| _Default (all other classes)_ | 6 hours | 30 days | 3 | `warm` | `da.default` |

These values are embedded in `torii.da_ingest.replication_policy` and applied to
all `/v1/da/ingest` submissions. Torii rewrites manifests with the enforced
retention profile and emits a warning when callers provide mismatched values so
operators can detect stale SDKs.

### Taikai availability classes

Taikai routing manifests (`taikai.trm` metadata) now include an
`availability_class` hint (`Hot`, `Warm`, or `Cold`). When present, Torii
selects the matching retention profile from `torii.da_ingest.replication_policy`
before chunking the payload, allowing event operators to downgrade inactive
renditions without editing the global policy table. The defaults are:

| Availability class | Hot retention | Cold retention | Required replicas | Storage class | Governance tag |
|--------------------|---------------|----------------|-------------------|----------------|----------------|
| `hot` | 24 hours | 14 days | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 hours | 30 days | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 hour | 180 days | 3 | `cold` | `da.taikai.archive` |

If the manifest omits `availability_class`, the ingest path falls back to the
`hot` profile so live streams keep their full replica set. Operators can
override these values by editing the new
`torii.da_ingest.replication_policy.taikai_availability` block in configuration.

## Configuration

The policy lives under `torii.da_ingest.replication_policy` and exposes a
*default* template plus an array of per-class overrides. Class identifiers are
case-insensitive and accept `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, or `custom:<u16>` for governance-approved extensions.
Storage classes accept `hot`, `warm`, or `cold`.

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

Leave the block untouched to run with the defaults listed above. To tighten a
class, update the matching override; to change the baseline for new classes,
edit `default_retention`.

To adjust specific Taikai availability classes, add entries under
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "warm"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 14400         # 4 h
cold_retention_secs = 604800       # 7 d
required_replicas = 4
storage_class = "warm"
governance_tag = "da.taikai.warm"
```

## Enforcement semantics

- Torii replaces the user-supplied `RetentionPolicy` with the enforced profile
  before chunking or manifest emission.
- Pre-built manifests that declare a mismatched retention profile are rejected
  with `400 schema mismatch` so stale clients cannot weaken the contract.
- Every override event is logged (`blob_class`, submitted vs expected policy)
  to surface non-compliant callers during rollout.

See `docs/source/da/ingest_plan.md` (Validation checklist) for the updated gate
covering retention enforcement.

## Re-replication workflow (DA-4 follow-up)

Retention enforcement is only the first step. Operators must also prove that
live manifests and replication orders stay aligned with the configured policy so
that SoraFS can automatically re-replicate out-of-compliance blobs.

1. **Watch for drift.** Torii emits
   `overriding DA retention policy to match configured network baseline` whenever
   a caller submits stale retention values. Pair that log with
   `torii_sorafs_replication_*` telemetry to spot replica shortfalls or delayed
   redeployments.
2. **Diff intent vs live replicas.** Use the new audit helper:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   The command loads `torii.da_ingest.replication_policy` from the provided
   config, decodes each manifest (JSON or Norito), and optionally matches any
   `ReplicationOrderV1` payloads by manifest digest. The summary flags two
   conditions:

   - `policy_mismatch` – the manifest retention profile diverges from the enforced
     policy (this should never happen unless Torii is misconfigured).
   - `replica_shortfall` – the live replication order requests fewer replicas than
     `RetentionPolicy.required_replicas` or provides fewer assignments than its
     target.

   A non-zero exit status indicates an active shortfall so CI/on-call automation
   can page immediately. Attach the JSON report to the
   `docs/examples/da_manifest_review_template.md` packet for Parliament votes.
3. **Trigger re-replication.** When the audit reports a shortfall, issue a fresh
   `ReplicationOrderV1` via the governance tooling described in
   `docs/source/sorafs/storage_capacity_marketplace.md` and re-run the audit
   until the replica set converges. For emergency overrides, pair the CLI output
   with `iroha app da prove-availability` so that SREs can reference the same digest
   and PDP evidence.

Regression coverage lives in `integration_tests/tests/da/replication_policy.rs`;
the suite submits a mismatched retention policy to `/v1/da/ingest` and verifies
that the fetched manifest exposes the enforced profile instead of the caller
intent.

## Proof-health telemetry & dashboards (DA-5 bridge)

Roadmap item **DA-5** requires PDP/PoTR enforcement outcomes to be auditable in
real time. `SorafsProofHealthAlert` events now drive a dedicated set of
Prometheus metrics:

- `torii_sorafs_proof_health_alerts_total{provider_id,trigger,penalty}`
- `torii_sorafs_proof_health_pdp_failures{provider_id}`
- `torii_sorafs_proof_health_potr_breaches{provider_id}`
- `torii_sorafs_proof_health_penalty_nano{provider_id}`
- `torii_sorafs_proof_health_cooldown{provider_id}`
- `torii_sorafs_proof_health_window_end_epoch{provider_id}`

The **SoraFS PDP & PoTR Health** Grafana board
(`dashboards/grafana/sorafs_pdp_potr_health.json`) now exposes those signals:

- *Proof Health Alerts by Trigger* charts alert rates by trigger/penalty flag so
  Taikai/CDN operators can prove whether PDP-only, PoTR-only, or dual strikes are
  firing.
- *Providers in Cooldown* reports the live sum of providers currently under a
  SorafsProofHealthAlert cooldown.
- *Proof Health Window Snapshot* merges the PDP/PoTR counters, penalty amount,
  cooldown flag, and strike window end epoch per provider so governance reviewers
  can attach the table to incident packets.

Runbooks should link these panels when presenting DA enforcement evidence; they
tie the CLI proof-stream failures directly to on-chain penalty metadata and
provide the observability hook called out in the roadmap.
