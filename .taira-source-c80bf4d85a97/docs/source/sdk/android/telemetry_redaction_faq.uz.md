---
lang: uz
direction: ltr
source: docs/source/sdk/android/telemetry_redaction_faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c3a71cfba151e1e7c4b7689901405b29b333c1e8fe5b952491b85b361113a69f
source_last_modified: "2025-12-29T18:16:36.054371+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Telemetry Redaction FAQ (AND7)

This backlog captures the recurring questions that surfaced during the AND7
telemetry enablement sessions, governance reviews, and chaos rehearsals. It is
the companion to the policy (`telemetry_redaction.md`), quick-reference card
(`telemetry_redaction_quick_reference.md`), pre-read (`telemetry_redaction_pre_read.md`),
and meeting minutes (`telemetry_redaction_minutes_20260212.md`). Update this FAQ
after each workshop or incident so on-call staff have a single canonical thread
for escalation guidance.

## When should I request a redaction override?

Overrides are the **last resort** when a pipeline fault leaves data stuck in the
redaction queue and threatens SLAs. File an override only after:

1. Running the chaos checklist (`telemetry_chaos_checklist.md`) steps that apply
   to the failing signal.
2. Capturing CLI evidence from `scripts/telemetry/inject_redaction_failure.sh`
   and `scripts/telemetry/check_redaction_status.py`.
3. Confirming with the Android Observability TL that the override is scoped to a
   single incident window.

Once approved, record the override in the audit log
(`telemetry_override_log.md`) and include the token hash plus expiry timestamp.
The quick-reference card lists the exact CLI invocation for both the approval
and cleanup commands.

## What evidence is required before closing an incident?

Collect the following artefacts and attach them to the incident ticket before
marking an override resolved:

- Latest status bundle JSON (`artifacts/android/telemetry/status-*.json`).
- Grafana screenshots highlighting `android.telemetry.redaction.failure` and
  `android.telemetry.export.status`.
- CLI output from the injector/status helpers and any queue export logs.
- The override log entry plus a link to the relevant section in
  `android_runbook.md` and the chaos rehearsal transcript.

Incidents without the artefacts above remain **action items** during the next
SRE governance review.

## How do salt rotations work on Android?

- Salt epochs are governed by the policy in `telemetry_redaction.md`.
- During the pre-flight checks, compare the reported
  `android.telemetry.redaction.salt_version` against the quarterly secret stored
  in the vault (`scripts/telemetry/check_redaction_status.py --status-url …`).
- When rotating, run the same command with `--write-cache` to capture the before
  and after timestamps, then store the diff under
  `docs/source/sdk/android/readiness/screenshots/<YYYY-MM-DD>/salts/`.
- Rotations must be announced in `#android-program` and referenced in the
  override audit log if an incident overlaps the change.

## How do we verify parity with the Rust telemetry schema?

Use `scripts/telemetry/run_schema_diff.sh` with the Android and Rust schema
snapshots (`configs/android/telemetry_schema.json` and
`configs/rust/telemetry_schema.json`). The output feeds both
`telemetry_schema_diff.md` and the FAQ entry for the relevant release. Any new
field requires:

1. Update to the policy doc (`telemetry_redaction.md`, signal inventory table).
2. An alert or dashboard note describing the retention/redaction plan.
3. Inclusion in the enablement packet and knowledge check questions.

## What is the chaos rehearsal workflow?

The rehearsal plan lives in `telemetry_chaos_checklist.md`. Each scenario must:

- Execute the checklist commands (injector, exporter, queue snapshot, recovery).
- Capture logs via `ci/run_android_telemetry_chaos_prep.sh --status-only`.
- Archive screenshots and CLI transcripts under
  `docs/source/sdk/android/readiness/screenshots/<scenario>/`.
- Produce an FAQ update summarising lessons learned and any new mitigation
  steps. Link back to the checklist item so future sessions stay deterministic.

## Where do training assets and knowledge checks live?

- **Pre-read:** `telemetry_redaction_pre_read.md`
- **Workshop minutes:** `telemetry_redaction_minutes_20260212.md`
- **Quick reference:** `telemetry_redaction_quick_reference.md`
- **Chaos lab artefacts:** `docs/source/sdk/android/readiness/`
- **Knowledge check + FAQ updates:** This file

After each enablement session, append the new questions/answers here and update
the knowledge-check forms referenced in `telemetry_redaction_minutes_20260212.md`.

## How do I propose a new FAQ entry?

1. Draft the question/answer, including links to supporting evidence (runbooks,
   scripts, screenshots).
2. Update this file and add the corresponding reference to `status.md` if the
   change modifies readiness evidence.
3. Post a short summary in `#and7-telemetry` so the Android Observability TL can
   track the backlog.

Future sessions can tag unresolved topics with “🈸 Drafting” to keep parity with
the roadmap status legend.
