---
lang: ur
direction: rtl
source: docs/source/sdk/python/support_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 76dab1f354c25d39577f521421e721afa288a3d19d302efed9ad3880183aec6c
source_last_modified: "2026-01-04T10:50:53.648952+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Python SDK Support Playbook (PY6-P3)

Roadmap item **PY6-P3 — CI/type-check automation & support policy** requires a
documented support model before the Python SDK can ship reproducible releases.
This playbook captures ownership, SLAs, escalation paths, and release evidence
so Release Engineering, Support, and governance reviewers share a single source
of truth when triaging incidents or approving PyPI drops.

## 1. Purpose & Scope

- Define who owns the SDK during pilots, GA, and LTS windows (Q1–Q2 2026).
- Describe the severity matrix, response timers, and required evidence bundles.
- Tie the CI/lint/type/test gates (`make python-checks`) and release smoke
  workflow (`make python-release-smoke`) to partner-facing commitments.
- Enumerate the artefacts that must accompany pilot, GA, hotfix, and LTS
  releases so audit/compliance reviews can proceed without ad hoc requests.

## 2. Ownership & Communications

| Role | Responsibilities | Notes |
|------|------------------|-------|
| Python SDK Maintainer | Approves roadmap gates, curates this playbook, records decisions in `status.md`. | Coordinates with Android/JS parity maintainers on fixture cadence. |
| Support Engineering Lead | Runs the support rota, triages Sev 1/2 incidents, maintains partner distro lists. | Publishes digest entries via the shared SDK status exporter. |
| Release Engineering | Operates CI (`.github/workflows/python-checks.yml`), provenance, and PyPI promotion. | Executes `python/iroha_python/scripts/release_smoke.sh`, archives manifests. |
| Observability/SRE | Maintains telemetry dashboards (`dashboards/mobile_parity.python.json` once live) and alert routing. | Tracks `python_checks_success_total`, fixture rotation SLA, queue depth telemetry. |
| Docs & Enablement | Keeps README/status entries fresh, publishes partner notices, localises critical updates. | Syncs translations on the monthly Docs/DevRel rotation. |

### 2.1 On-call Signals

1. Primary PagerDuty service: `python-sdk-primary` (mirrored to
   `python-sdk-secondary` for redundancy).
2. Announce Sev 1 in `#sdk-parity` within **15 minutes**; Sev 2 within **1 hour**.
3. Invite Release, Observability, and Compliance to Sev 1 bridges. Sev 2 bridges
   always include Release; add Compliance as needed.
4. File the after-action report using the shared SDK incident template within
   **5 business days**.

## 3. Support Phases & SLAs

| Phase | Window | Support Scope | Exit Criteria |
|-------|--------|---------------|---------------|
| Pilot | Q1 2026 partners | 9×5 coverage, manual telemetry reviews, direct Slack bridges | Pilot validation report + parity dashboard snapshot filed in `status.md`. |
| General Availability | Target Q2 2026 | 24×5 SLA, PagerDuty rotation, automated dashboards + exporter snapshots | GA packet (support playbook, release manifest, parity digest) archived under `artifacts/python_release/<tag>/`. |
| Hotfix | Any time | Sev 1 regression/security fix, dedicated bridge | Hotfix advisory in README/status, provenance + smoke evidence refreshed. |

### 3.1 Severity Matrix

| Severity | Examples | Detection | Acknowledge | Updates |
|----------|----------|-----------|-------------|---------|
| **Sev 1** | Cannot submit transactions, Norito parity failure, reproducibility gap, telemetry override stuck | `make python-checks`, dashboards, partner tickets | ≤15 min | Every 30 min until mitigation, incident logged in `status.md`. |
| **Sev 2** | Telemetry drift, doc/SLA mistakes, degraded Connect retries | Dashboards, `scripts/check_python_fixtures.py`, support queue | ≤1 hour | Hourly updates, summary posted within 24 h. |
| **Sev 3** | Documentation clarifications, sample bugs | Support queue | ≤1 business day | Track in backlog, mention in weekly digest. |

Timers start when alerts fire or partner communication lands. Every incident
must include fixture age (`scripts/check_python_fixtures.py --json-out`), parity
results, and the latest release manifest if applicable.

## 4. Release Gating & Evidence

Before distributing artefacts outside engineering:

1. **Lint/type/tests:** Run `make python-checks` (wraps `ruff`, `mypy`, `pytest`,
   and fixture parity). Archive the console output for the release bundle.
2. **Fixture parity:** `scripts/check_python_fixtures.py --json-out artifacts/python/fixture_status.json`.
   Keep the rotation metadata (`scripts/python_fixture_regen.sh`) alongside the JSON.
3. **Release smoke:** Execute `make python-release-smoke`, which delegates to
   `python/iroha_python/scripts/release_smoke.sh`. Preserve
   `dist/release_artifacts.json`, `SHA256SUMS`, Sigstore bundles, and the changelog preview.
4. **Provenance:** Attach Sigstore/OIDC evidence when available; for short-lived
   keys, note the `ephemeral_signature` flag in the manifest.
5. **Docs & status:** Update `status.md` with the release summary, fixture age,
   and support window; refresh README/localised quickstarts when instructions change.

CI (`.github/workflows/python-checks.yml`) blocks merges when any of these gates
fail. Do not re-run until the root cause is addressed.

## 5. Incident Workflow

1. **Triage:** Gather logs (`python/iroha_python/tests`, `scripts/check_python_fixtures.py`),
   dashboards, and partner reports. Capture failing payloads as deterministic fixtures.
2. **Mitigation:** Apply configuration updates, regenerate fixtures, or roll
   back releases as required. Record the Git commits and scripts used.
3. **Evidence:** Store logs, fixture diffs, release manifests, and console
   output under `docs/source/sdk/python/readiness/incidents/<YYYY-MM-DD>/`.
4. **Communication:** Post status in partner Slack/email including impact,
   workaround, and next update ETA. Reference runbook sections for context.
5. **Closure:** Update `status.md` and this playbook (if procedures changed),
   archive Grafana screenshots, and link the AAR to the incident log.

## 6. Partner Communication & Localization

- Share release calendars ≥60 days in advance, highlighting blackout windows
  and cutover dependencies.
- Translate critical notices (JP and HE) within two weeks using the Docs/DevRel
  rota; note completion in the localization tracker.
- Attach this playbook, the release automation guide, and current SLAs to the
  partner enablement packet referenced in roadmap AND8.

## 7. References

- `.github/workflows/python-checks.yml` — CI gate for lint/type/test + smoke.
- `python/iroha_python/scripts/run_checks.sh` — local helper invoked by
  `make python-checks`.
- `python/iroha_python/scripts/release_smoke.sh` — reproducible release harness.
- `docs/source/sdk/python/release_automation.md` — detailed release checklist.
- `docs/source/sdk/python/connect_end_to_end.md` — Connect walkthrough cited in
  partner enablement.
- `status.md` — weekly updates referencing this playbook and the PY6 milestones.

Keep this document in sync with roadmap PY6 updates and the evidence bundles
stored under `artifacts/python_release/<tag>/`. Any change to SLAs or ownership
must be captured here and mirrored in `status.md` within one business day.
