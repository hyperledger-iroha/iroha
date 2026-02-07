---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: "SoraFS Provider Advert Rollout Plan"
---

> Adapted from [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# SoraFS Provider Advert Rollout Plan

This plan coordinates the cut-over from permissive provider advertisements to
the fully-governed `ProviderAdvertV1` surface required for multi-source chunk
retrieval. It focuses on three deliverables:

- **Operator guide.** Step-by-step actions storage providers must complete
  before each gate flips.
- **Telemetry coverage.** Dashboards and alerts that Observability and Ops use
  to confirm the network only accepts compliant adverts.
The rollout aligns with SF-2b/2c milestones in the [SoraFS migration
roadmap](./migration-roadmap) and assumes the admission policy in the
[provider admission policy](./provider-admission-policy) is already in
effect.

## Current Requirements

SoraFS accepts only governance-enveloped `ProviderAdvertV1` payloads. The
following requirements are enforced at admission:

- `profile_id=sorafs.sf1@1.0.0` with canonical `profile_aliases` present.
- `chunk_range_fetch` capability payloads must be included for multi-source
  retrieval.
- `signature_strict=true` with council signatures attached to the advert
  envelope.
- `allow_unknown_capabilities` is only permitted during explicit GREASE drills
  and must be logged.

## Operator Checklist

1. **Inventory adverts.** List every published advert and record:
   - Governing envelope path (`defaults/nexus/sorafs_admission/...` or production equivalent).
   - Advert `profile_id` and `profile_aliases`.
   - Capability list (expect at least `torii_gateway` and `chunk_range_fetch`).
   - `allow_unknown_capabilities` flag (required when vendor-reserved TLVs are present).
2. **Regenerate with provider tooling.**
   - Rebuild the payload with your provider advert publisher, ensuring:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` with a defined `max_span`
     - `allow_unknown_capabilities=<true|false>` when GREASE TLVs are present
   - Validate via `/v1/sorafs/providers` and `sorafs_fetch`; warnings about unknown
     capabilities must be triaged.
3. **Validate multi-source readiness.**
   - Execute `sorafs_fetch` with `--provider-advert=<path>`; the CLI now fails
     when `chunk_range_fetch` is missing and prints warnings for ignored unknown
     capabilities. Capture the JSON report and archive it with operations logs.
4. **Stage renewals.**
   - Submit `ProviderAdmissionRenewalV1` envelopes at least 30 days before
     expiration. Renewals must retain the canonical handle and capability set;
     only stake, endpoints, or metadata should change.
5. **Communicate with dependent teams.**
   - SDK owners must release versions that surface warnings to operators when
     adverts are rejected.
   - DevRel announces each phase transition; include dashboard links and the
     threshold logic below.
6. **Install dashboards & alerts.**
   - Import the Grafana export and place it under **SoraFS / Provider
     Rollout** with dashboard UID `sorafs-provider-admission`.
   - Ensure the alert rules point to the shared `sorafs-advert-rollout`
     notification channel in staging and production.

## Telemetry & Dashboards

The following metrics are already exposed via `iroha_telemetry`:

- `torii_sorafs_admission_total{result,reason}` — counts accepted, rejected,
  and warning outcomes. Reasons include `missing_envelope`, `unknown_capability`,
  `stale`, and `policy_violation`.

Grafana export: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Import the file into the shared dashboards repository (`observability/dashboards`)
and update only the datasource UID before publishing.

The board publishes under the Grafana folder **SoraFS / Provider Rollout** with
the stable UID `sorafs-provider-admission`. Alert rules
`sorafs-admission-warn` (warning) and `sorafs-admission-reject` (critical) are
pre-configured to use the `sorafs-advert-rollout` notification policy; adjust
that contact point if the destination list changes rather than editing the
dashboard JSON.

Recommended Grafana panels:

| Panel | Query | Notes |
|-------|-------|-------|
| **Admission outcome rate** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Stack chart to visualise accept vs warn vs reject. Alert when warn > 0.05 * total (warning) or reject > 0 (critical). |
| **Warning ratio** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Single-line timeseries that feeds the pager threshold (5% warning rate rolling 15 minutes). |
| **Rejection reasons** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Drives runbook triage; attach links to mitigation steps. |
| **Refresh debt** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Indicates providers missing the refresh deadline; cross-reference with discovery cache logs. |

CLI artefacts for manual dashboards:

- `sorafs_fetch --provider-metrics-out` writes `failures`, `successes`, and
  `disabled` counters per provider. Import into ad-hoc dashboards to monitor
  orchestrator dry-runs before switching production providers.
- The JSON report’s `chunk_retry_rate` and `provider_failure_rate` fields
  highlight throttling or stale payload symptoms that often precede admission
  rejections.

### Grafana dashboard layout

Observability publishes a dedicated board — **SoraFS Provider Admission
Rollout** (`sorafs-provider-admission`) — under **SoraFS / Provider Rollout**
with the following canonical panel IDs:

- Panel 1 — *Admission outcome rate* (stacked area, unit “ops/min”).
- Panel 2 — *Warning ratio* (single series), emitting the expression
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- Panel 3 — *Rejection reasons* (time series grouped by `reason`), sorted by
  `rate(...[5m])`.
- Panel 4 — *Refresh debt* (stat), mirroring the query in the table above and
  annotated with the advert refresh deadlines pulled from the migration ledger.

Copy (or create) the JSON skeleton in the infrastructure dashboards repo at
`observability/dashboards/sorafs_provider_admission.json`, then update only the
data source UID; the panel IDs and alert rules are referenced by the runbooks
below, so avoid renumbering them without revising this documentation.

For convenience the repository now ships a reference dashboard definition at
`docs/source/grafana_sorafs_admission.json`; copy it into your Grafana folder if
you need a starting point for local testing.

### Prometheus alert rules

Add the following rule group to `observability/prometheus/sorafs_admission.rules.yml`
(create the file if this is the first SoraFS rule group) and include it from
your Prometheus configuration. Replace `<pagerduty>` with the actual routing
label for your on-call rotation.

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

Run `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
before pushing changes to ensure the syntax passes `promtool check rules`.

## Admission Outcomes

- Missing `chunk_range_fetch` capability → reject with `reason="missing_capability"`.
- Unknown capability TLVs without `allow_unknown_capabilities=true` → reject with
  `reason="unknown_capability"`.
- `signature_strict=false` → reject (reserved for isolated diagnostics).
- Expired `refresh_deadline` → reject.

## Communication & Incident Handling

- **Weekly status mailer.** DevRel circulates a brief summary of admission
  metrics, outstanding warnings, and upcoming deadlines.
- **Incident response.** If `reject` alerts fire, on-call engineers:
  1. Fetch the offending advert via Torii discovery (`/v1/sorafs/providers`).
  2. Re-run advert validation in the provider pipeline and compare with
     `/v1/sorafs/providers` to reproduce the error.
  3. Coordinate with the provider to rotate the advert before the next refresh
     deadline.
- **Change freezes.** No capability schema changes land during R1/R2 unless
  the rollout committee signs off; GREASE trials must be scheduled during the
  weekly maintenance window and logged in the migration ledger.

## References

- [SoraFS Node/Client Protocol](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Provider Admission Policy](./provider-admission-policy)
- [Migration Roadmap](./migration-roadmap)
- [Provider Advert Multi-Source Extensions](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)
