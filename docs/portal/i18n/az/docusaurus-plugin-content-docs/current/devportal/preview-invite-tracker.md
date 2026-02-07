---
id: preview-invite-tracker
lang: az
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Preview invite tracker
sidebar_label: Preview tracker
description: Wave-by-wave status log for the checksum-gated docs portal preview program.
---

This tracker records every docs portal preview wave so DOCS-SORA owners and
governance reviewers can see which cohort is active, who approved the invites,
and which artefacts still need attention. Update it whenever invites are sent,
revoked, or deferred so the audit trail stays inside the repository.

## Wave status

| Wave | Cohort | Tracker issue | Approver(s) | Status | Target window | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 – Core maintainers** | Docs + SDK maintainers validating checksum flow | `DOCS-SORA-Preview-W0` (GitHub/ops tracker) | Docs/DevRel lead + Portal TL | 🈴 Completed | Q2 2025 weeks 1–2 | Invites sent 2025‑03‑25, telemetry stayed green, exit summary published 2025‑04‑08. |
| **W1 – Partners** | SoraFS operators, Torii integrators under NDA | `DOCS-SORA-Preview-W1` | Docs/DevRel lead + Governance liaison | 🈴 Completed | Q2 2025 week 3 | Invites ran 2025‑04‑12 → 2025‑04‑26 with all eight partners acked; evidence captured in [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) and the exit digest in [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 – Community** | Curated community waitlist (≤25 at a time) | `DOCS-SORA-Preview-W2` | Docs/DevRel lead + Community manager | 🈴 Completed | Q3 2025 week 1 (tentative) | Invites ran 2025‑06‑15 → 2025‑06‑29 with telemetry green throughout; evidence + findings captured in [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). |
| **W3 – Beta cohorts** | Finance/observability beta + SDK partner + ecosystem advocate | `DOCS-SORA-Preview-W3` | Docs/DevRel lead + Governance liaison | 🈴 Completed | Q1 2026 week 8 | Invites ran 2026‑02‑18 → 2026‑02‑28; digest + portal data generated via `preview-20260218` wave (see [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> Note: link each tracker issue to the corresponding preview request tickets and
> archive them under the `docs-portal-preview` project so approvals remain
> discoverable.

## Active tasks (W0)

- ✅ Preflight artefacts refreshed (GitHub Actions `docs-portal-preview` run 2025‑03‑24, descriptor verified via `scripts/preview_verify.sh` using tag `preview-2025-03-24`).
- ✅ Telemetry baselines captured (`docs.preview.integrity`, `TryItProxyErrors` dashboards snapshot saved to the W0 tracker issue).
- ✅ Outreach copy locked using [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) with preview tag `preview-2025-03-24`.
- ✅ Intake requests logged for the first five maintainers (tickets `DOCS-SORA-Preview-REQ-01` … `-05`).
- ✅ First five invites sent 2025‑03‑25 10:00–10:20 UTC after seven consecutive green telemetry days; acknowledgements stored in `DOCS-SORA-Preview-W0`.
- ✅ Monitor telemetry + host office hours (daily check-ins through 2025‑03‑31; checkpoint log below).
- ✅ Collect midpoint feedback / issues and tag them `docs-preview/w0` (see [W0 digest](./preview-feedback/w0/summary.md)).
- ✅ Publish wave summary + invite exit confirmations (exit bundle dated 2025‑04‑08; see [W0 digest](./preview-feedback/w0/summary.md)).
- ✅ W3 beta wave tracked; future waves scheduled as needed after governance review.

## W1 partner wave summary

- ✅ **Legal & governance approvals.** Partner addendum signed 2025‑04‑05; approvals uploaded to `DOCS-SORA-Preview-W1`.
- ✅ **Telemetry + Try it staging.** Change ticket `OPS-TRYIT-147` executed 2025‑04‑06 with Grafana snapshots for `docs.preview.integrity`, `TryItProxyErrors`, and `DocsPortal/GatewayRefusals` archived.
- ✅ **Artefact + checksum prep.** `preview-2025-04-12` bundle verified; descriptor/checksum/probe logs stored under `artifacts/docs_preview/W1/preview-2025-04-12/`.
- ✅ **Invite roster + dispatch.** All eight partner requests (`DOCS-SORA-Preview-REQ-P01…P08`) approved; invites sent 2025‑04‑12 15:00–15:21 UTC with acknowledgements logged per reviewer.
- ✅ **Feedback instrumentation.** Daily office hours + telemetry checkpoints recorded; see [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) for the digest.
- ✅ **Final roster/exit log.** [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) now records invite/ack timestamps, telemetry evidence, quiz exports, and artefact pointers as of 2025‑04‑26 so governance can replay the wave.

## Invite log — W0 core maintainers

| Reviewer ID | Role | Request ticket | Invite sent (UTC) | Expected exit (UTC) | Status | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Portal maintainer | `DOCS-SORA-Preview-REQ-01` | 2025‑03‑25 10:05 | 2025‑04‑08 10:00 | Active | Acked checksum verification; focusing on nav/sidebar review. |
| sdk-rust-01 | Rust SDK lead | `DOCS-SORA-Preview-REQ-02` | 2025‑03‑25 10:08 | 2025‑04‑08 10:00 | Active | Testing SDK recipes + Norito quickstarts. |
| sdk-js-01 | JS SDK maintainer | `DOCS-SORA-Preview-REQ-03` | 2025‑03‑25 10:12 | 2025‑04‑08 10:00 | Active | Validating Try it console + ISO flows. |
| sorafs-ops-01 | SoraFS operator liaison | `DOCS-SORA-Preview-REQ-04` | 2025‑03‑25 10:15 | 2025‑04‑08 10:00 | Active | Auditing SoraFS runbooks + orchestration docs. |
| observability-01 | Observability TL | `DOCS-SORA-Preview-REQ-05` | 2025‑03‑25 10:18 | 2025‑04‑08 10:00 | Active | Reviewing telemetry/incident appendices; owns Alertmanager coverage. |

All invites reference the same `docs-portal-preview` artefact (run 2025‑03‑24,
tag `preview-2025-03-24`) and the verification transcript captured in
`DOCS-SORA-Preview-W0`. Any additions/pauses must be logged in both the table
above and the tracker issue before proceeding to the next wave.

## Checkpoint log — W0

| Date (UTC) | Activity | Notes |
| --- | --- | --- |
| 2025‑03‑26 | Telemetry baseline review + office hours | `docs.preview.integrity` + `TryItProxyErrors` remained green; office hours confirmed all reviewers completed checksum verification. |
| 2025‑03‑27 | Midpoint feedback digest posted | Summary captured in [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md); two minor nav issues logged as `docs-preview/w0` labels, no incidents reported. |
| 2025‑03‑31 | Final week telemetry spot check | Last pre-exit office hours; reviewers confirmed remaining docs tasks on track, no alerts fired. |
| 2025‑04‑08 | Exit summary + invite closures | Acknowledged completed reviews, revoked temporary access, archived findings in [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08); tracker updated before prepping W1. |

## Invite log — W1 partners

| Reviewer ID | Role | Request ticket | Invite sent (UTC) | Expected exit (UTC) | Status | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | SoraFS operator (EU) | `DOCS-SORA-Preview-REQ-P01` | 2025‑04‑12 15:00 | 2025‑04‑26 15:00 | Completed | Delivered orchestrator ops feedback 2025‑04‑20; exit ack 15:05 UTC. |
| sorafs-op-02 | SoraFS operator (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025‑04‑12 15:03 | 2025‑04‑26 15:00 | Completed | Logged rollout guidance comments in `docs-preview/w1`; exit ack 15:10 UTC. |
| sorafs-op-03 | SoraFS operator (US) | `DOCS-SORA-Preview-REQ-P03` | 2025‑04‑12 15:06 | 2025‑04‑26 15:00 | Completed | Dispute/blacklist edits filed; exit ack 15:12 UTC. |
| torii-int-01 | Torii integrator | `DOCS-SORA-Preview-REQ-P04` | 2025‑04‑12 15:09 | 2025‑04‑26 15:00 | Completed | Try it auth walkthrough accepted; exit ack 15:14 UTC. |
| torii-int-02 | Torii integrator | `DOCS-SORA-Preview-REQ-P05` | 2025‑04‑12 15:12 | 2025‑04‑26 15:00 | Completed | RPC/OAuth doc comments logged; exit ack 15:16 UTC. |
| sdk-partner-01 | SDK partner (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025‑04‑12 15:15 | 2025‑04‑26 15:00 | Completed | Preview integrity feedback merged; exit ack 15:18 UTC. |
| sdk-partner-02 | SDK partner (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025‑04‑12 15:18 | 2025‑04‑26 15:00 | Completed | Telemetry/redaction review done; exit ack 15:22 UTC. |
| gateway-ops-01 | Gateway operator | `DOCS-SORA-Preview-REQ-P08` | 2025‑04‑12 15:21 | 2025‑04‑26 15:00 | Completed | Gateway DNS runbook comments filed; exit ack 15:24 UTC. |

## Checkpoint log — W1

| Date (UTC) | Activity | Notes |
| --- | --- | --- |
| 2025‑04‑12 | Invite dispatch + artefact verification | All eight partners emailed with `preview-2025-04-12` descriptor/archive; acknowledgements stored in tracker. |
| 2025‑04‑13 | Telemetry baseline review | `docs.preview.integrity`, `TryItProxyErrors`, and `DocsPortal/GatewayRefusals` dashboards reviewed — green across the board; office hours confirmed checksum verification completed. |
| 2025‑04‑18 | Mid-wave office hours | `docs.preview.integrity` remained green; two doc nits logged under `docs-preview/w1` (nav wording + Try it screenshot). |
| 2025‑04‑22 | Final telemetry spot check | Proxy + dashboards still healthy; no new issues raised, noted in tracker ahead of exit. |
| 2025‑04‑26 | Exit summary + invite closures | All partners confirmed review completion, invites revoked, evidence archived in [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## W3 beta cohort recap

- ✅ Invites sent 2026‑02‑18 with checksum verification + acknowledgements logged the same day.
- ✅ Feedback collected under `docs-preview/20260218` with governance issue `DOCS-SORA-Preview-20260218`; digest + summary generated via `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- ✅ Access revoked 2026‑02‑28 after the final telemetry check; tracker + portal tables updated to show W3 as completed.

## Invite log — W2 community

| Reviewer ID | Role | Request ticket | Invite sent (UTC) | Expected exit (UTC) | Status | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | Community reviewer (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025‑06‑15 16:00 | 2025‑06‑29 16:00 | Completed | Ack 16:06 UTC; focusing on SDK quickstarts; exit confirmed 2025‑06‑29. |
| comm-vol-02 | Community reviewer (Governance) | `REQ-C02` | 2025‑06‑15 16:03 | 2025‑06‑29 16:00 | Completed | Governance/SNS review done; exit confirmed 2025‑06‑29. |
| comm-vol-03 | Community reviewer (Norito) | `REQ-C03` | 2025‑06‑15 16:06 | 2025‑06‑29 16:00 | Completed | Norito walkthrough feedback logged; exit ack 2025‑06‑29. |
| comm-vol-04 | Community reviewer (SoraFS) | `REQ-C04` | 2025‑06‑15 16:09 | 2025‑06‑29 16:00 | Completed | SoraFS runbook review done; exit ack 2025‑06‑29. |
| comm-vol-05 | Community reviewer (Accessibility) | `REQ-C05` | 2025‑06‑15 16:12 | 2025‑06‑29 16:00 | Completed | Accessibility/UX notes shared; exit ack 2025‑06‑29. |
| comm-vol-06 | Community reviewer (Localization) | `REQ-C06` | 2025‑06‑15 16:15 | 2025‑06‑29 16:00 | Completed | Localization feedback logged; exit ack 2025‑06‑29. |
| comm-vol-07 | Community reviewer (Mobile) | `REQ-C07` | 2025‑06‑15 16:18 | 2025‑06‑29 16:00 | Completed | Mobile SDK doc checks delivered; exit ack 2025‑06‑29. |
| comm-vol-08 | Community reviewer (Observability) | `REQ-C08` | 2025‑06‑15 16:21 | 2025‑06‑29 16:00 | Completed | Observability appendix review done; exit ack 2025‑06‑29. |

## Checkpoint log — W2

| Date (UTC) | Activity | Notes |
| --- | --- | --- |
| 2025‑06‑15 | Invite dispatch + artefact verification | `preview-2025-06-15` descriptor/archive shared with 8 community reviewers; acknowledgements stored in tracker. |
| 2025‑06‑16 | Telemetry baseline review | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` dashboards green; Try it proxy logs show community tokens active. |
| 2025‑06‑18 | Office hours & issue triage | Collected two suggestions (`docs-preview/w2 #1` tooltip wording, `#2` localization sidebar) — both routed to Docs. |
| 2025‑06‑21 | Telemetry check + doc fixes | Docs addressed `docs-preview/w2 #1/#2`; dashboards still green, no incidents. |
| 2025‑06‑24 | Final week office hours | Reviewers confirmed remaining feedback submissions; no alert fire. |
| 2025‑06‑29 | Exit summary + invite closures | Acks recorded, preview access revoked, telemetry snapshots + artefacts archived (see [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025‑04‑15 | Office hours & issue triage | Two documentation suggestions logged under `docs-preview/w1`; no incidents or alerts triggered. |

## Reporting hooks

- Each Wednesday, update the tracker table above plus the active invite issue
  with a short status note (invites sent, active reviewers, incidents).
- When a wave closes, append the feedback summary path (for example,
  `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) and link it from
  `status.md`.
- If any pause criteria from the [preview invite flow](./preview-invite-flow.md)
  trigger, add the remediation steps here before resuming invites.
