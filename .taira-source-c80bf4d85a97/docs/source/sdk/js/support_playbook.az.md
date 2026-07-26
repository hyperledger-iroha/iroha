---
lang: az
direction: ltr
source: docs/source/sdk/js/support_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 57178653cc3f3505f7ea3106c1321a7d81cdcf7c031b0cc9a2ef7a0090a2ea0a
source_last_modified: "2026-01-05T09:28:12.059662+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# JavaScript SDK Support Playbook (JS5)

Roadmap item **JS5 — publishing automation & provenance** calls for a documented
support model before the JavaScript SDK moves beyond the rolling-preview
window. This playbook aligns the JS5 guardrails with governance expectations so
maintainers, Release Engineering, and SRE share a single source of truth when
handling incidents or rehearsing rollback drills.

## 1. Purpose & Scope

- Define ownership, on-call signals, and communication channels for the SDK.
- Capture the severity matrix, acknowledgement timers, and evidence bundles
  required for pilot, GA, hotfix, and LTS releases.
- Tie the CI/publishing automation (`npm run release:provenance`,
  `npm run release:matrix`, `npm run release:verify`,
  `scripts/js_sbom_provenance.sh`, `scripts/js_signed_staging.sh`) to partner
  SLAs.
- Describe the rollback workflow (`scripts/js_release_rollback.sh`) so every
  drill and real incident yields reproducible artefacts.

## 2. Ownership & Communications

| Role | Responsibilities | Notes |
|------|------------------|-------|
| JavaScript SDK Maintainer | Owns roadmap gates, curates this playbook, records decisions in `status.md`. | Coordinates fixture cadence with Android/Python leads. |
| Release Engineering | Operates `.github/workflows/javascript-sdk*.yml`, provenance capture, npm promotion. | Archives artefacts under `artifacts/js-sdk-provenance/` and `artifacts/js-sdk-release-matrix/`. |
| Support Engineering Lead | Runs the PagerDuty rota, triages Sev 1/2 incidents, keeps partner distro lists. | Publishes digest entries through the shared SDK status exporter (`status.md`). |
| Observability/SRE | Maintains dashboards/alerts for JS parity, npm telemetry, and CI success signals. | Tracks `javascript_sdk_publish_success_total`, verification SLA, and queue depth telemetry. |
| Docs & Enablement | Keeps README/portal entries fresh, mirrors governance notices, localises urgent updates. | Coordinates translations during the Docs/DevRel monthly sync. |

### 2.1 On-call Signals

1. Primary PagerDuty service: `javascript-sdk-primary` (mirrored to
   `javascript-sdk-secondary`).
2. Announce Sev 1 incidents in `#sdk-parity` within **15 minutes**; Sev 2 within
   **1 hour**. Mention incident/bug IDs and link to collected evidence.
3. Invite Release, Observability, and Compliance to Sev 1 bridges; invite
   Release and Observability for Sev 2 unless governance requires more roles.
4. File the after-action report with the shared SDK incident template within
   **5 business days**.

## 3. Support Phases & SLAs

| Phase | Window | Support Scope | Exit Criteria |
|-------|--------|---------------|---------------|
| Pilot | Q4 FY2025 preview customers | 9×5 coverage, manual telemetry reviews, ad-hoc bridges | Pilot validation packet + parity digest filed in `status.md`. |
| General Availability | Target Q2 2026 | 24×5 SLA, PagerDuty rotation, automated dashboards + exporter snapshots | GA bundle (support playbook, release manifest, verification evidence) archived under `artifacts/js-release/<tag>/`. |
| Hotfix | Any time | Sev 1 regression/security fix, dedicated bridge | Hotfix advisory in README/status, provenance + verification evidence refreshed. |

### 3.1 Severity Matrix

| Severity | Examples | Detection | Acknowledge | Updates |
|----------|----------|-----------|-------------|---------|
| **Sev 1** | Transactions fail via JS helpers, Norito builders diverge from Rust, provenance missing | CI red, parity dashboards, partner tickets | ≤15 min | Every 30 min until mitigation; incident logged in `status.md`. |
| **Sev 2** | Retry telemetry drift, Connect preview failures, docs blocking adoption | Dashboards, `npm run release:verify`, support queue | ≤1 hour | Hourly until resolved; summary posted within 24 h. |
| **Sev 3** | Sample code fixes, doc clarifications | Support queue | ≤1 business day | Track in backlog; mention in weekly digest. |

Timers start when alerts fire or partner communication lands. Attach fixture age
(`docs/source/sdk/js/fixture_cadence.md`), verification outputs, and the latest
release manifest to every incident ticket.

## 4. Release Gating & Evidence

Before distributing artefacts outside engineering:

1. **Build + tests:** Run `npm run build:native`, `npm test`,
   `npm run check:changelog`, and the integration suite (`npm run test:integration`)
   against the dev Torii cluster.
2. **Provenance bundle:** `npm run release:provenance` records the
   `npm pack` tarball, metadata, SHA-256 sums, and console logs under
   `artifacts/js-sdk-provenance/v<version>_<timestamp>/`.
3. **Release-candidate matrix:** `npm run release:matrix` (wrapper around
   `javascript/iroha_js/scripts/release-matrix.mjs`) captures per-OS logs,
   tarballs, status JSON, and a Prometheus textfile under
   `artifacts/js-sdk-release-matrix/`. Use `--textfile-dir`/`JS_RELEASE_MATRIX_TEXTFILE_DIR`
   when the gauges must be mirrored into the shared node_exporter textfile
   collector.
4. **Signed staging run:** `scripts/js_signed_staging.sh <version>` runs the
   publish workflow against the staging registry and stores logs plus checksums
   under `artifacts/js/npm_staging/<version>/`.
5. **SBOM + verification:** `scripts/js_sbom_provenance.sh` emits a CycloneDX
   report that is uploaded with the release bundle, and
   `npm run release:verify -- --version <version>` captures the published
   tarball hashes inside `artifacts/js/verification/v<version>_<timestamp>/`.
6. **Docs & status:** Update `docs/source/sdk/js/publishing.md`,
   `docs/portal/docs/sdks/javascript.md`, and `status.md` with release notes,
   fixture cadence, and support window reminders.

CI workflows (`javascript-sdk.yml`, `javascript-sdk-rc-matrix.yml`,
`javascript-sdk-publish.yml`) must succeed before notifying partners.

## 5. Rollback & Incident Workflow

1. **Freeze distribution:** Repoint `latest`/custom dist-tags to the last known
   good version and deprecate the bad tarball. `scripts/js_release_rollback.sh`
   wraps the dist-tag/deprecate commands and stores logs under
   `artifacts/js/incidents/<incident>/`.
2. **Verify:** Run `npm run release:verify` for the suspect and replacement
   versions (plus any staged artefacts) so the incident folder includes hashes,
   console logs, and comparison metadata.
3. **Staging rehearse:** When a rebuild is required,
   `scripts/js_signed_staging.sh <replacement>` captures the signed dry-run to
   prove the replacement tarball matches CI inputs before promotion.
4. **Recover:** Promote the replacement tarball, run
   `scripts/js_sbom_provenance.sh` to update the SBOM, and re-run the
   verification helper to confirm npm now serves the fixed artefact.
5. **Communicate:** Post the incident summary (impact, workaround, ETA,
   evidence path) in `#sdk-parity`, notify affected partners, and add a line to
   `status.md`. Archive Grafana screenshots + logs alongside the incident folder
   so governance reviewers can replay the evidence.

## 6. Partner Communication & Localization

- Share release calendars ≥60 days in advance, flagging blackout windows and
  dependency on Torii/Connect milestones.
- Translate critical notices (JP + HE) within two weeks using the Docs/DevRel
  rota; track completion in the localization log referenced from
  `docs/source/sdk/js/index.md`.
- Attach this playbook, the publishing guide, fixture cadence charter, and the
  current SLA summary to partner enablement packets.

## 7. References

- `docs/source/sdk/js/publishing.md` — release automation + rollback guide.
- `docs/source/sdk/js/release_playbook.md` — GitHub workflow driven release flow.
- `docs/source/sdk/js/fixture_cadence.md` — governance-approved regeneration schedule.
- `javascript/iroha_js/scripts/record-release-provenance.mjs`,
  `scripts/js_release_rollback.sh`, `scripts/js_sbom_provenance.sh`,
  `scripts/js_signed_staging.sh`.
- `status.md` — weekly JS SDK updates referencing this playbook and JS5 gates.
