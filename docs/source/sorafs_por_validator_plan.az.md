---
lang: az
direction: ltr
source: docs/source/sorafs_por_validator_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8940bdf29c615ca36c257b776f7a956665444ab6814c8e1d03faeb0eb6ce1d20
source_last_modified: "2025-12-29T18:16:36.174886+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS PoR Validator CLI & Reporting
summary: Specification for SF-9b covering validator tooling, challenge replay, reporting feeds, and governance handoff.
---

# SoraFS PoR Validator CLI & Reporting

## Goals & Scope
- Provide auditors, SRE, and governance teams with deterministic tooling to inspect, replay, and export Proof-of-Replication (PoR) data produced by the coordinator (SF-9a).
- Deliver command-line workflows and API surfaces that track challenge lifecycles, trigger manual probes, and publish weekly health reports without relying on ad hoc scripts.
- Ensure all outputs rely on Norito payloads, respect audit retention policies, and integrate cleanly with the repair automation and governance DAG pipelines.

This document completes **SF-9b — Validator CLI & reporting** and transitions the PoR/Audit framework (SF-9) into implementation-ready state.

## Validator Personas
- **Auditor:** Independent or council-appointed reviewer validating provider proofs, filing slashing proposals, and verifying remediation.
- **SRE / Ops:** Monitors live challenges, triggers manual probes during incidents, and exports digests for dashboards.
- **Governance Analyst:** Consumes weekly reports, reviews outstanding failures, and confirms that penalties/slashing align with policy.

## CLI Design (`sorafs_cli por ...`)

### Command Inventory
| Command | Description | Output |
|---------|-------------|--------|
| `sorafs_cli por status --manifest <CID> [--provider <ID>] [--epoch <N>] [--format json|table]` | List challenges and outcomes for a manifest/provider, optionally scoped to an epoch. | Tabular or JSON `PorChallengeStatusV1`. |
| `sorafs_cli por show --challenge <UUID>` | Display full details for a single challenge, including Norito payloads and evidence hashes. | YAML/JSON bundle referencing object storage URIs. |
| `sorafs_cli por fetch-proof --challenge <UUID> --out proof.json` | Download `PorChallengeV1` + `PorProofV1` pair and store locally for replay. | File on disk; CLI prints SHA-256 digest. |
| `sorafs_cli por verify --proof proof.json` | Re-run verification logic offline to confirm coordinator verdicts. | Exit status + summary; optional `--verbose` prints per-sample results. |
| `sorafs_cli por trigger --manifest <CID> --provider <ID> [--samples N] [--reason <text>]` | Request a manual challenge outside scheduled cadence (requires governance/auditor signature). | Returns new challenge ID + Norito envelope digest. |
| `sorafs_cli por report --week <ISO-week> [--format json|markdown]` | Produce consolidated report for governance review (success/failure counts, penalties, outstanding repairs). | JSON `PorWeeklyReportV1` or Markdown summary. |
| `sorafs_cli por export --range <start_epoch>..<end_epoch> --out por_export.parquet` | Export challenges, proofs, and verdicts for downstream analytics. | Parquet + manifest describing schema version. |

All commands accept `--torii <URL>` and `--auth <path>` (Norito token or key file) for authenticated access.

### Norito Payloads
New structs (to live in `sorafs_manifest::por` alongside SF-9a schemas):

```norito
struct PorChallengeStatusV1 {
    challenge_id: Uuid,
    manifest_digest: Digest32,
    provider_id: ProviderId,
    epoch_id: U64,
    status: PorChallengeOutcome,      // pending|verified|failed|repaired|forced
    failure_reason: Option<String>,
    issued_at: Timestamp,
    responded_at: Option<Timestamp>,
    repair_task_id: Option<Uuid>,
    proof_digest: Option<Digest32>,
}

struct PorWeeklyReportV1 {
    week: IsoWeek,
    generated_at: Timestamp,
    challenges_total: U32,
    challenges_verified: U32,
    challenges_failed: U32,
    forced_challenges: U32,
    repairs_enqueued: U32,
    repairs_completed: U32,
    slashing_events: Vec<PorSlashingEventV1>,
    providers_missing_vrf: Vec<ProviderId>,
    top_offenders: Vec<PorProviderSummaryV1>,
    notes: Option<String>,
}

struct PorSlashingEventV1 {
    provider_id: ProviderId,
    manifest_digest: Digest32,
    penalty_xor: Decimal64,
    verdict_cid: String,
    decided_at: Timestamp,
}

struct PorProviderSummaryV1 {
    provider_id: ProviderId,
    failed_challenges: U32,
    repaired_challenges: U32,
    pending_repairs: U32,
    vrf_missing_epochs: U32,
}
```

The CLI serialises these types using Norito JSON (`norito::json`) and ensures report outputs embed metadata (schema version, hash, generation timestamp).

## Torii API Extensions
| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/sorafs/por/status` | Query `PorChallengeStatusV1` list filtered by manifest, provider, epoch, or status. Supports pagination (`page_token`). |
| `GET` | `/sorafs/por/challenge/{id}` | Fetch detailed challenge, including signed `PorChallengeV1`, `PorProofV1` (if available), verification summary, and associated repair task. |
| `POST` | `/sorafs/por/trigger` | Submit `SignedAuditorRequestV1<ManualPorChallengeV1>` to request an immediate challenge (auditor/governance only). |
| `GET` | `/sorafs/por/report/{iso_week}` | Retrieve `PorWeeklyReportV1` generated by the coordinator. |
| `GET` | `/sorafs/por/export` | Stream Parquet export for specified epoch range (requires governance-level auth). |

`ManualPorChallengeV1` includes `{manifest_digest, provider_id, requested_samples, reason}` and reuses the seed derivation logic from SF-9a while marking the challenge as `manual`.

## Offline Verification Pipeline
- `sorafs_cli por verify` loads the proof bundle, replays verification using the same `ProofVerifier` as the coordinator, and emits:
  - `overall_result`: verified | failed | indeterminate.
  - Per-sample results with digest comparison and Merkle path validation.
  - Optional `--write-fixture` to materialise reproducible fixtures under `fixtures/sorafs_manifest/por/replay/`.
- Deterministic seeds ensure replay matches coordinator results; mismatches flag potential coordinator bugs or tampering.

## Reporting & Dashboards
- Weekly report generation occurs Sundays 00:00 UTC after the final epoch. Coordinator stores the signed report under `governance/sorafs/por/reports/<ISO-week>.json`.
- Markdown output mirrors governance meeting structure:
  ```
  # PoR Weekly Report (2026-W08)
  - Total challenges: 1344 (verified 1310, failed 22, forced 12)
  - Repairs enqueued: 18 (completed 15, outstanding 3)
  - Slashing events: 1 (provider sorafs:prov:abc, penalty 120 XOR, verdict CID ipfs://...)
  - Providers missing VRF: sorafs:prov:def (3 epochs), sorafs:prov:xyz (1 epoch)
  - Notes: ...
  ```
- Grafana datasource `por_weekly` ingests `PorWeeklyReportV1` via the reporting API. Dashboards show trend lines for failure rates, forced challenges, VRF misses, and repair correlations.
- Alerting:
  - `SORAfsPorWeeklyFailures` triggers if `challenges_failed > 0.05 * challenges_total`.
  - `SORAfsPorWeeklyVRFMiss` triggers if any provider misses VRF > 3 epochs.

## Governance & Audit Workflow
- Reports and manual challenge requests must be signed with governance-approved keys (`SignedGovernanceDocumentV1` envelope).
- Export files include `manifest.json` describing schema version, row counts, hash, and generation timestamp; exports are pinned in SoraFS with `por_export_<range>.car`.
- Governance meetings reference `PorWeeklyReportV1` to decide on penalties, certify reparations, and update public transparency logs.

## Implementation Checklist
- [x] Define CLI command set with consistent output formats and filters.
- [x] Specify Norito payloads for status, reports, slashing events, and provider summaries.
- [x] Document Torii endpoints for status queries, manual triggers, weekly reports, and exports.
- [x] Describe offline verification/replay pipeline and fixture regeneration.
- [x] Outline reporting cadence, dashboard integrations, and alert thresholds.
- [x] Enumerate governance hooks, signatures, and export retention requirements.

With this specification, the PoR validator tooling can be implemented alongside the coordinator (SF-9a), completing the SF-9 design phase.
