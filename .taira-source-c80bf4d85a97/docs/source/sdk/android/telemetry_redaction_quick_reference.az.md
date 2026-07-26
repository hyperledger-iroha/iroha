---
lang: az
direction: ltr
source: docs/source/sdk/android/telemetry_redaction_quick_reference.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2f3224185b9031dc3bbdccb87818a8c0f4fc78ddef43f22422b7d8cb5dc70bf3
source_last_modified: "2025-12-29T18:16:36.055341+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Telemetry Redaction Quick Reference (AND7)

This card condenses the telemetry redaction runbook for on-call engineers and
training sessions referenced by roadmap item AND7. It mirrors the details in the
full policy (`telemetry_redaction.md`) and operations runbook
(`../../android_runbook.md`) while keeping the core checks on a single page for
incident bridges and enablement labs.

## 1. Pre-Flight Checklist

Complete these steps at the start of every shift or chaos rehearsal:

1. **Pull the latest status bundle**
   ```bash
   scripts/telemetry/check_redaction_status.py \
     --status-url https://android-telemetry-stg/api/redaction/status \
     --min-hashed 214 \
     --json-out artifacts/android/telemetry/status-latest.json
   ```
   - Confirm `STATUS: healthy` summary lines.
   - Attach the JSON artefact to your shift log.
   - Use `--min-hashed` (or `ANDROID_TELEMETRY_MIN_HASHED_AUTHORITIES=214`)
     to ensure the hashed-authority floor noted in the governance packet is
     enforced automatically during the pre-flight poll.
2. **Verify salt rotation**
   - Compare `android.telemetry.redaction.salt_version` from the status bundle
     against the quarterly salt epoch from the secrets vault.
   - Mismatches require escalation to the Android Observability TL.
3. **Validate schema parity**
   ```bash
   OUT=docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json
   scripts/telemetry/run_schema_diff.sh \
     --android-config configs/android_telemetry.json \
     --rust-config configs/rust_telemetry.json \
     --out "$OUT"
   jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted") | {signal, field, status}' "$OUT"
   jq '.android_only_signals[] | select(.status != "accepted") | {signal, status}' "$OUT"
   jq '.field_mismatches[] | {signal, field, android, rust}' "$OUT"
   ```
   - Compare `$OUT` with the approved snapshot
     `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`
     and attach the new artefact to the incident/PR. The CLI must now emit
     `status:"accepted"` for the Android-only signals (including
     `android.telemetry.redaction.failure` and
     `android.telemetry.redaction.salt_version`); treat missing statuses as a
     schema regression per `telemetry_schema_diff.md` §3.4. `field_mismatches`
     must remain empty (the command above should not print anything) per the
     governance checklist in §5.
   - Cross-check the `intentional_differences` table for the hashed Torii
     authority fields and the `android.telemetry.device_profile` buckets so the
     allowlists captured in §3 remain in lock-step with the CLI output.
4. **Spot-check exporter health**
   - Open the `Android Telemetry Redaction`, `Redaction Compliance`, `Exporter
     Health`, and `Android Telemetry Overview` Grafana boards.
   - `Android Telemetry Redaction`: `android.telemetry.redaction.salt_version`
     matches the current salt epoch and the override token panel reads zero when
     no drills are running.
   - `Redaction Compliance`: no spikes on
     `android.telemetry.redaction.failure` in the last 24 h; injector trend
     stays flat outside rehearsals.
   - `Exporter Health`: `android.telemetry.export.status{status="error"}`
     remains below 1 % while the `status="ok"` line trends upward.
   - `Android Telemetry Overview`: `android.telemetry.network_context` event
     counts move whenever Torii traffic is present and the enterprise bucket
     stays within ±10 % of the Rust baseline.
5. **Review override queue**
   - Run `scripts/android_override_tool.sh digest --out artifacts/android/telemetry/override_digest-$(date -u +%Y%m%d).json`.
   - Confirm `active_overrides` is zero and that the digest references the latest
     row in `docs/source/sdk/android/telemetry_override_log.md`; if not, tie the
     active token to an incident and schedule cleanup.

## 2. Command Cheat Sheet

| Task | Command / Source | Expected Output |
|------|------------------|-----------------|
| Generate status bundle | `scripts/telemetry/check_redaction_status.py --status-url <collector>` | `STATUS`/`ALERT` lines plus JSON bundle under `artifacts/android/telemetry/`. |
| Inject failure drill | `scripts/telemetry/inject_redaction_failure.sh --dry-run` | CLI notes failing signal; alerts fire within 5 min. Clear with `--clear`. |
| Export pending queue evidence | `ci/run_android_telemetry_chaos_prep.sh` with `ANDROID_PENDING_QUEUE_EXPORTS=<serial>=/tmp/queue.bin` | Copies queue snapshots, emits `<label>.sha256/.json` for incident bundles. |
| Capture chaos rehearsal log | `ci/run_android_telemetry_chaos_prep.sh --status-only` | Archives script logs + status bundle for knowledge checks. |
| Record override approval | `scripts/android_override_tool.sh --event-log docs/source/sdk/android/readiness/override_logs/override_events.ndjson --actor-role <bucket> apply --request … --log docs/source/sdk/android/telemetry_override_log.md --out artifacts/android/telemetry/override-$(date -u +%Y%m%dT%H%M%SZ).json` (see `telemetry_redaction_faq.md#when-should-i-request-a-redaction-override`) | NDJSON telemetry event + Markdown log referencing incident ticket plus emitted manifest. |
| Inspect override digest | `scripts/android_override_tool.sh digest --log docs/source/sdk/android/telemetry_override_log.md --out artifacts/android/telemetry/override_digest-$(date -u +%Y%m%d).json` | JSON summary with `active_overrides` count; attach alongside the status bundle. |

## 3. Metric Thresholds

| Metric | Threshold | Action |
|--------|-----------|--------|
| `android.telemetry.redaction.failure` counter (`streaming_privacy.redaction_fail_total` / `android_telemetry_redaction_failure_total`) | >0 over 15 min window | Investigate failing signal, run injector clear, attach CLI output to incident. |
| `android.telemetry.redaction.salt_version` | Deviates from cluster salt epoch | Halt releases; coordinate with secrets rotation to restore parity. |
| `android.telemetry.export.status{status="error"}` | >1% of exports | Check collector health; capture CLI diagnostics and escalate to SRE. |
| `android.telemetry.device_profile{tier="enterprise"}` vs Rust parity | Variance >10% | File follow-up in governance notes and verify device pools. |
| `android.telemetry.network_context` volume | Drops to zero during active Torii traffic | Confirm apps registered a `NetworkContextProvider`/`enableAndroidNetworkContext(...)`; re-run schema diff if fields changed. |
| `android.telemetry.redaction.override` event / `android_telemetry_override_tokens_active` gauge | Non-zero outside approved drill | Confirm override memo + incident ID, regenerate the digest, then revoke via the support workflow. |

## 4. Escalation & Evidence

1. **Escalate quickly:** page the Android Observability TL and Support lead when
   any metric crosses the thresholds above.
2. **Evidence bundle:** collect:
   - Latest status JSON (`artifacts/android/telemetry/status-*.json`).
   - Grafana screenshot with timestamp.
   - CLI output from injector/status commands.
   - Pending queue exports if replay paths are impacted.
   - Latest override digest (`artifacts/android/telemetry/override_digest-*.json`)
     when tokens are issued or revoked.
3. **Runbook references:** include links to `android_runbook.md` section 2 and the
   chaos checklist (`telemetry_chaos_checklist.md`) in the incident doc.
4. **Handoffs:** update the operations changelog with override details, the
   remediation window, and references to any Alertmanager silences used.

## 5. Chaos Rehearsal Flow (Module 3)

Use this 10-minute drill during enablement sessions (Module 3) or quarterly
chaos rehearsals. It mirrors `telemetry_lab_01.md` and keeps artefacts aligned
with AND7 acceptance criteria.

1. **Warm-up snapshot**
   ```bash
   ci/run_android_telemetry_chaos_prep.sh --status-only
   ```
   - Archives the latest status bundle + script logs into
     `artifacts/android/telemetry/chaos/`.
   - Attach the bundle to the lab report before inducing faults.
2. **Inject a redaction failure**
   ```bash
   scripts/telemetry/inject_redaction_failure.sh \
     --note chaos-drill-$(date -u +%Y%m%dT%H%M%SZ)
   ```
   - Alerts should fire within 5 minutes (Redaction Compliance dashboard).
   - Pass `--status-url https://android-telemetry-stg/admin/redaction/failure`
     when targeting the staging endpoint instead of the local cache.
   - Let the failure persist for at least two status polls, then clear it with
     `--clear` and re-run the status script to confirm recovery.
3. **Offline queue replay (optional)**
   ```bash
   ANDROID_PENDING_QUEUE_EXPORTS="pixel8=/tmp/pixel8.queue,emu=/tmp/emu.queue" \
     ci/run_android_telemetry_chaos_prep.sh
   ```
   - Collect queue dumps from the staging Pixel + emulator (see Scenario D in
     `readiness/labs/telemetry_lab_01.md`).
   - The helper emits `<label>.sha256` and `<label>.json` evidence plus the
     decoded envelopes for review.
4. **Evidence & reporting**
   - File an override entry in `telemetry_override_log.md` if any overrides were
     used.
   - Append drill notes/screenshots to
     `readiness/labs/reports/<YYYY-MM>/telemetry_lab_01_report.md`.
   - Record Grafana timestamps + CLI output in the enablement tracker so the
     AND7 governance review has reproducible artefacts.

Keeping this quick-reference card with the enablement packet closes the AND7
training deliverable for Feb 2026 governance reviews and gives on-call staff a
single page to drive telemetry redaction responses.

## 6. Canonical Signals Snapshot (2026-03-05)

Use this table as the “known good” reference when auditing schema diff exports
or answering governance questions. Each signal listed below **must** retain
`status:"accepted"` (or the tooling must be regenerated) before enablement packs
are redistributed.

| Signal | Channel | Status | Why it matters | Spot check |
|--------|---------|--------|----------------|------------|
| `android.telemetry.redaction.override` | Event | `accepted` | Tracks break-glass flows; audit log rows for overrides must mirror this event stream. | Ensure `android_telemetry_override_tokens_active` matches the override digest and capture NDJSON logs per `android_runbook.md` §3. |
| `android.telemetry.network_context` | Event | `accepted` | Android drops carrier names and exports only `network_type`/`roaming`, satisfying the privacy allowlist in `telemetry_schema_diff.md` §3. | Confirm apps enable `NetworkContextProvider` and that the event volume follows Torii traffic on `Android Telemetry Overview`. |
| `android.telemetry.redaction.failure` | Counter | `accepted` | Emits whenever hashing fails; governance requires explicit status metadata in the schema diff artefact. | Watch the Redaction Compliance dashboard and CLI bundle for non-zero counts outside drills. |
| `android.telemetry.redaction.salt_version` | Gauge | `accepted` | Proves the exporter knows the current salt epoch, closing the schema diff action item for rotation observability. | Compare Grafana’s salt widget with the secrets-vault epoch and flag any schema diff missing the status entry. |

If a schema diff run drops one of these status flags, rerun
`scripts/telemetry/run_schema_diff.sh`, re-export the AND7 artefact under
`readiness/schema_diffs/`, and update both this quick-reference card and
`android_runbook.md` so governance packets continue to reflect the canonical
snapshot described in roadmap item AND7.
