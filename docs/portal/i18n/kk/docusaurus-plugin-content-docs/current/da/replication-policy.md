---
lang: kk
direction: ltr
source: docs/portal/docs/da/replication-policy.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

title: Data Availability Replication Policy
sidebar_label: Replication Policy
description: Governance-enforced retention profiles applied to all DA ingest submissions.
---

:::note Canonical Source
:::

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

Taikai routing manifests (`taikai.trm`) declare an `availability_class`
(`hot`, `warm`, or `cold`). Torii enforces the matching policy before chunking
so operators can scale replica counts per stream without editing the global
table. Defaults:

| Availability class | Hot retention | Cold retention | Required replicas | Storage class | Governance tag |
|--------------------|---------------|----------------|-------------------|----------------|----------------|
| `hot` | 24 hours | 14 days | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 hours | 30 days | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 hour | 180 days | 3 | `cold` | `da.taikai.archive` |

Missing hints default to `hot` so live broadcasts retain the strongest policy.
Override the defaults via
`torii.da_ingest.replication_policy.taikai_availability` if your network uses
different targets.

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

Taikai availability classes can be overridden independently via
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "cold"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 3600          # 1 h
cold_retention_secs = 15552000     # 180 d
required_replicas = 3
storage_class = "cold"
governance_tag = "da.taikai.archive"
```

## Enforcement semantics

- Torii replaces the user-supplied `RetentionPolicy` with the enforced profile
  before chunking or manifest emission.
- Pre-built manifests that declare a mismatched retention profile are rejected
  with `400 schema mismatch` so stale clients cannot weaken the contract.
- Every override event is logged (`blob_class`, submitted vs expected policy)
  to surface non-compliant callers during rollout.

See [Data Availability Ingest Plan](ingest-plan.md) (Validation checklist) for the updated gate
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
   `docs/examples/da_manifest_review_template.md`
   packet for Parliament votes.
3. **Trigger re-replication.** When the audit reports a shortfall, issue a fresh
   `ReplicationOrderV1` via the governance tooling described in
   [SoraFS storage capacity marketplace](../sorafs/storage-capacity-marketplace.md) and re-run the audit
   until the replica set converges. For emergency overrides, pair the CLI output
   with `iroha app da prove-availability` so that SREs can reference the same digest
   and PDP evidence.

Regression coverage lives in `integration_tests/tests/da/replication_policy.rs`;
the suite submits a mismatched retention policy to `/v1/da/ingest` and verifies
that the fetched manifest exposes the enforced profile instead of the caller
intent.
