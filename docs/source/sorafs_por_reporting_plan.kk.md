---
lang: kk
direction: ltr
source: docs/source/sorafs_por_reporting_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3ba32c6d0263b875066f132dc4b790583054ca7cbd638fbc1115206da0b5c4e6
source_last_modified: "2026-06-25T16:58:37+00:00"
translation_last_reviewed: 2026-06-25
title: SoraFS PoR Validator CLI & Reporting
summary: Reference for the SF-9b validator tooling and reporting surfaces.
---

# SoraFS PoR Validator CLI & Reporting

## CLI Enhancements

- `sorafs_cli por status --manifest <digest>` — display the latest challenges and outcomes with optional filters.
- `sorafs_cli por export --out <path>` — export GovernanceLog PoR verdicts for offline audits.
- `sorafs_cli por report --week <iso-week>` — render weekly health reports as Markdown or Norito JSON.

## Reporting

- Generate weekly proof health report (JSON + Markdown) via `por report`.
- Include metrics: total challenges, success rate, failures by provider.
- Publish report to governance dashboards / Git repo.

## GovernanceLog Integration

- Fetch/write verdicts into GovernanceLog (Norito payloads).
- `por export` streams `PorSlashingEventV1` and associated audit verdicts so downstream systems reconcile with the transparency DAG.

## Report Templates & Delivery

Weekly proof-health reports are published in two artefacts: machine-consumable JSON and
human-friendly Markdown. Both are generated from the same dataset and stored under
`reports/por/<year>/<week>/`.

### JSON schema (`PorWeeklyReportV1`)

```json
{
  "cycle_id": "2025-W12",
  "generated_at": "2025-03-24T09:30:00Z",
  "manifest_sample": 512,
  "providers": [
    {
      "provider_id": "prov_8d3a…",
      "manifest_count": 14,
      "challenges": 96,
      "success_rate_bps": 9890,
      "failures": 1,
      "first_failure_at": "2025-03-22T11:05:12Z",
      "last_success_latency_ms_p95": 1800,
      "repair_dispatched": true,
      "ticket_id": "REP-342"
    }
  ],
  "aggregate": {
    "total_challenges": 1536,
    "total_failures": 12,
    "success_rate_bps": 9920,
    "mean_latency_ms": 720,
    "p95_latency_ms": 2100
  },
  "anomalies": [
    {
      "provider_id": "prov_1a2b…",
      "reason": "missed_repair_sla",
      "details": "Two consecutive failures without repair completion within 4h threshold."
    }
  ]
}
```

- `cycle_id` matches the transparency ledger cycle identifier.
- `success_rate_bps` is an integer in `0..=10_000`; latency fields are unsigned
  integer milliseconds. Floating-point metrics are not part of the v1 report
  wire format.
- `providers[]` entries align with governance IDs and include repair linkage (`repair_dispatched`,
  ticket references) so operations can correlate with the repair plan.
- `anomalies` array captures reasons defined in `sorafs_repair_plan.md` (e.g., `missed_repair_sla`,
  `repeated_failure`, `latency_regression`).

### Markdown summary (`por-weekly-2025-W12.md`)

````markdown
# PoR Weekly Health — 2025-W12

Generated: 2025-03-24 09:30:00 UTC

| Provider | Manifests | Challenges | Success Rate | Failures | Repair Ticket | Notes |
|----------|-----------|------------|--------------|----------|---------------|-------|
| prov_8d3a | 14 | 96 | 98.9% | 1 | REP-342 | Failure attributed to disk saturation; repair completed within SLA. |
| prov_1a2b | 21 | 144 | 94.1% | 8 | REP-351 (pending) | Alert escalated — awaiting repair agent acknowledgement. |

## Aggregate Metrics
- Total challenges: **1 536**
- Success rate: **99.2%**
- Mean latency: **720 ms**
- P95 latency: **2 100 ms**

## Outstanding Actions
- prov_1a2b — repair ticket REP-351 overdue by 2h (see `sorafs_repair_plan.md` §Incident Escalation).
- prov_4c5d — observe latency regression (p95 2.9s vs target 2.0s); schedule follow-up sampling.

## Attachments
- [JSON report](./por-weekly-2025-W12.json)
- Governance DAG snapshot: `ipfs://sorafs-transparency/2025-W12/governance.car`
````

Markdown reports link to the JSON artefact and the corresponding CAR snapshot from the transparency plan.

## Scheduler Challenge Authority

Torii exposes no client-driven challenge ingress. The verified coordinator
scheduler derives challenges only from authenticated drand and provider-VRF
inputs under the active governance policy, then records the resulting
`PorChallengeV1` in the governance DAG. The CLI is read/report oriented: it can
inspect, export, and report scheduler outcomes but cannot issue challenges.

Automation must fail closed when the external randomness or VRF evidence cannot
be verified. CI validates deterministic scheduler fixtures without introducing a
public mutation command.

## Repair Automation Hooks

PoR reporting feeds directly into the repair pipeline (`docs/source/sorafs_repair_plan.md`):

- When a provider’s failure count exceeds the threshold defined in the repair plan, the
  report generator creates or updates a repair ticket via the `sorafs repair schedule`
  API. The ticket ID is embedded in both JSON and Markdown reports.
- Future CLI work will expose `sorafs_cli por repair-status --provider <id>` which cross-references the
  latest PoR results with repair progress (queued, in-progress, completed). It consumes the
  repair service’s Norito API, ensuring reporting and remediation stay in sync.
- Weekly reports include a section summarising repair SLAs: number of repairs completed
  within 4h, over-SLA repairs, and outstanding incidents.
- Alerting ties into the repair automation metrics (`sorafs_repair_plan.md` §Telemetry), so
  any provider with repeated PoR failures triggers both a repair schedule and a governance
  alert.
- Rollout evidence for reporting/archive handoff must carry the same
  `seed_replay_digest_hex` as the randomness artifact that generated the
  challenge set, so report archives cannot be mixed with replay evidence from a
  different drand/VRF seed.

Completion criteria:

1. JSON/Markdown templates render successfully using staging data and pass schema validation.
2. Scheduler challenge derivation requires authenticated drand and provider-VRF evidence and fails closed when either cannot be verified.
3. Repair pipeline consumes the generated reports and automatically aligns tickets/status without
   manual intervention.
4. Reporting/archive rollout evidence is payload-free and bound to the
   randomness seed replay digest accepted by the SF-9 rollout gate.
