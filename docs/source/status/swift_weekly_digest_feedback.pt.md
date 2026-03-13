---
lang: pt
direction: ltr
source: docs/source/status/swift_weekly_digest_feedback.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d1bdb70f33d8d32f780640e8b8046fcea3980a83d8b4789bae011e8b9e3daf40
source_last_modified: "2026-01-03T18:07:58.614711+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Swift Weekly Digest Feedback Log
summary: Tracks stakeholder feedback for the Swift SDK weekly status digest.
---

# Swift Weekly Digest Feedback Log

## Purpose

Collect feedback from stakeholders on the Swift SDK weekly digest so future
reports capture the right metrics, context, and follow-up items. Notes here feed
into roadmap/status updates and exporter improvements.

## Stakeholder Checkpoints

| Date (UTC) | Audience | Owner | Notes |
|------------|----------|-------|-------|
| 2026-01-15 | Governance council | Swift Program PM | Present the 2026-01-12 digest, capture requests for additional metrics or format changes. |
| 2026-01-16 | SDK parity working group | Swift Lead | Review parity diff expectations and confirm whether outstanding fixtures require deeper commentary. |
| 2026-01-17 | Build Infra / CI on-call | Swift QA Lead | Validate CI lane coverage, capture noise/flake issues raised by the digest. |
| 2026-04-26 | Governance council (weekly) | Swift Program PM | Review the first automated Apr digest drop, align `/v2/pipeline` messaging, and confirm when the vote/readiness review should be scheduled. |
| 2026-04-27 | Observability / SRE on-call | SDK Program Lead | Validate that hashed-authority telemetry + queue-depth gaps are surfaced in the digest until OTLP exporters land. |

## Feedback Log

| Date | Source | Feedback | Action | Status |
|------|--------|----------|--------|--------|
| 2026-01-13 | Swift Program PM (self-review) | Capture parity diff context in highlights, keep risk callouts focused on SLA watchers. | Folded into 2026-01-12 digest; no further action. | Closed |
| 2026-01-15 | Governance council | Requested explicit `/v2/pipeline` adoption status, governance vote timing, and risk owners so the council can tie digest updates to rollout gates. | Added a “Governance Watchers” block to `docs/source/status/swift_weekly_digest_template.md` (and mirrored it in the dated digest) so owners + status/risk notes are recorded next to the metrics snapshot. | Closed |
| 2026-01-16 | SDK parity working group | Asked for a per-SDK fixture drift table plus a pointer to the latest Buildkite parity job logs so reviewers can chase stale fixtures quickly. | `scripts/swift_status_export.py` now renders a **Fixture Drift Summary** table (instruction, age, owner) and surfaces the Buildkite metadata artefact path; the digest template/sample inherit the generated block automatically. | Closed |
| 2026-01-17 | Build Infra / CI on-call | Needed CI noise summaries (`ci/xcode-swift-parity` success %) and a snapshot of the `swift_parity_success_total` metric until dashboards go live. | `scripts/swift_status_export.py` now renders a CI Signals section that embeds the parity counters and spotlights the parity lane automatically, so the exporter no longer relies on manual snapshots. | Closed |
| 2026-04-26 | Governance council | Asked the weekly digest to highlight `/v2/pipeline` adoption status plus the upcoming governance vote date so release gate owners can see blockers at a glance. | Added the “Governance Watchers” table to `docs/source/status/swift_weekly_digest.md`, tying `/v2/pipeline` and vote readiness rows to named owners with the CR-2 dependency called out. | Closed |
| 2026-04-27 | Observability TL / SRE on-call | Requested that the digest continue to note hashed-authority rotation age and the queue-depth telemetry gap until OTLP exporters land. | Expanded the Governance Watchers notes to flag the CR-2 queue telemetry dependency and kept the telemetry snippet’s `salt_epoch`/`rotation_age` values highlighted so SRE has the weekly evidence path. | Closed |
| 2026-05-03 | Observability TL | Weekly digest still printed `swift_parity_success_total: metrics state unavailable` when CI ran unattended because metrics exports were opt-in. | `ci/swift_status_export.sh` now emits Prometheus textfile metrics and a JSON state file under `artifacts/swift/` by default (with `SWIFT_STATUS_DISABLE_METRICS=1` as the opt-out), and the digest guide documents the override knobs. | Closed |
| 2026-05-05 | Torii Platform TL | Asked the digest to surface the Torii spec review schedule and link the agenda so `/v2/pipeline` freeze work stays aligned with IOS2 risks. | Added a “Torii spec review (IOS2)” row to the Governance Watchers block in `docs/source/status/swift_weekly_digest.md` pointing to `docs/source/sdk/swift/torii_spec_review.md` and the locked 2026-11-21 session. | Closed |

## Owner Rotation

- **Primary:** Swift Program PM (`@swift-program-pm`)
- **Secondary:** Swift Lead (`@swift-lead`)
- Ensure the digest feedback log is updated within 24 h of each checkpoint.

## Process Checklist

1. Distribute the latest digest to stakeholders (council, SDK parity WG, Build
   Infra) no later than Tuesday 17:00 UTC.
2. Record feedback, requested metrics, and any blockers in the table above.
3. Reflect accepted feedback in the next digest and, if tooling updates are
   required, open follow-up tasks or PRs referencing `scripts/swift_status_export.py`.
