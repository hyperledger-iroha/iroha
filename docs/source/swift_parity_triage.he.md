---
lang: he
direction: rtl
source: docs/source/swift_parity_triage.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f00abc0d686755b34a19c1846e09beeb2d3b59c1f23dc6f9dc5eb0a1f5b08815
source_last_modified: "2026-01-03T18:08:01.867518+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
Swift Norito fixture triage runbook. Keep translations in sync once localized.
-->

# Swift Norito Parity Triage Runbook

Swift parity diffs are tracked by `dashboards/mobile_parity.swift` and the Swift CI
lanes. This runbook explains how to respond when fixtures or `/v1/pipeline`
integration tests drift from the Rust reference implementation.

## When to page the runbook

- `dashboards/mobile_parity.swift` reports `outstanding_diffs > 0` or
  `oldest_diff_hours > 48`.
- `dashboards/mobile_ci.swift` shows the `ci/xcode-swift-parity` lane below the
  95 % success-rate threshold or `consecutive_failures > 0`.
- `scripts/check_swift_fixtures.py` fails locally/CI when comparing
  `IrohaSwift/Fixtures` against the canonical Android fixtures.
- Governance (council) notifies SDK owners about upcoming Norito discriminator/ABI
  changes that require fixture refresh.

## Cadence decision & fallback plan

### Primary cadence (governance approved on 2026‑01‑15)

- **Window.** Scheduled regeneration happens every Wednesday at 17:00 UTC with
  the Android Foundations TL owning odd weeks and the Swift Lead owning even
  weeks. The helper `scripts/swift_fixture_regen.sh` derives the slot
  automatically and stores the metadata in
  `artifacts/swift_fixture_regen_state.json`.
- **SLA.** Any governance-approved discriminator/ABI change must be mirrored
  into Swift fixtures within 48 hours. `ci/check_swift_fixtures.sh` and the
  parity dashboards alert whenever the SLA is breached.
- **Reporting.** After each scheduled run, the owner posts the regen summary
  (`SWIFT_FIXTURE_ROTATION_OWNER`, `SWIFT_FIXTURE_CADENCE`,
  `SWIFT_FIXTURE_CADENCE_INTERVAL_HOURS`,
  `SWIFT_FIXTURE_ALERT_CONTACT/CHANNEL`, archive digests) in
  `#sdk-parity`, runs `make swift-fixtures-check`, and records the entry in
  `status.md` plus the weekly digest.

### Manual fallback cadence

Trigger the fallback whenever **either** of the following holds:

1. The scheduled slot slips by more than six hours without a published regen.
2. `swift_parity_regen_hours_since_success` stays above 72 hours or
   `swift_parity_status` remains `0` for six consecutive hours on the parity
   dashboard.

Fallback procedure:

1. Set `SWIFT_FIXTURE_EVENT_TRIGGER=1` and
   `SWIFT_FIXTURE_EVENT_REASON=fallback-weekly` so the cadence state records why
   the unscheduled run occurred. Include `SWIFT_FIXTURE_ROTATION_OWNER=<name>`
   when someone outside the normal roster is covering.
2. Run `make swift-fixtures` followed by `make swift-fixtures-check`. If you are
   consuming a signed Rust archive, pass `SWIFT_FIXTURE_ARCHIVE=/path/to.tar.gz`
   so the state file captures the source digest.
3. Execute `swift test --package-path IrohaSwift` (or `make swift-ci` if the
   `/v1/pipeline` helpers changed) to ensure fixtures and builders stay in sync.
4. Regenerate the parity metrics:
   ```bash
   ci/swift_status_export.sh \
     --parity dashboards/data/mobile_parity.sample.json \
     --ci dashboards/data/mobile_ci.sample.json \
     --metrics-path artifacts/prom/swift_parity.prom
   ```
   The script updates the Prometheus textfile that Observability scrapes and
   bumps either `swift_parity_success_total` or `swift_parity_failure_total`
   depending on the outcome.
5. Post the fallback summary (slot missed, reason, metric snapshot link) in
   `#sdk-parity` and file a governance follow-up if the SLA would have been
   breached without the manual run. Escalate to the Program Lead when two
   consecutive weeks rely on fallback mode.

### Observability hooks

| Signal | What it proves | Fallback expectation |
|--------|----------------|----------------------|
| `swift_parity_status` gauge | Latest parity success (1) / failure (0) | Returns to 1 within one hour of the fallback run. |
| `swift_parity_regen_hours_since_success` gauge | Hours since the most recent successful regen | Drops below 24 h immediately after fallback; alert if it exceeds 72 h. |
| `swift_parity_success_total` / `swift_parity_failure_total` counters | Long-term cadence health | Success counter increments on every fallback run that clears diffs; failure counter increments only when diffs remain, signalling SRE follow-up. |
| `swift_parity_outstanding_diffs` gauge + `dashboards/mobile_parity.swift` | File-level drift view | Should return to 0 after fallback; if not, continue triage via the steps below. |

Observability scrapes these metrics from the textfile emitted by
`ci/swift_status_export.sh`, and the Grafana dashboard mirrors them for the SRE
on-call. Do **not** silence alerts during fallback; instead, annotate them with
the Slack link that documents the manual run.

### Exiting fallback mode

Once governance confirms the regular cadence (meeting minutes capture the
decision) and the next scheduled slot is executed on time, clear the fallback
reason by running `SWIFT_FIXTURE_EVENT_TRIGGER=0 make swift-fixtures` during the
following scheduled run so the cadence state returns to “scheduled”. Update the
weekly digest with the exit date and close any incident tickets tied to the
fallback window.

## Preparation

1. Ensure the Rust toolchain and Android fixtures are up to date:
   ```bash
   git pull --rebase
   make android-fixtures
   make android-fixtures-check
   ```
2. Confirm you can run Swift package tests locally:
   ```bash
   swift test --package-path IrohaSwift
   ```
3. Export the latest Norito fixtures if the Android tree already consumed the
   update:
   ```bash
   SWIFT_FIXTURE_SOURCE=java/iroha_android/src/test/resources \
   make swift-fixtures
   ```
   The regeneration script writes cadence metadata to
   `artifacts/swift_fixture_regen_state.json`. Set
   `SWIFT_FIXTURE_ROTATION_OWNER=<name>` (and optionally
   `SWIFT_FIXTURE_CADENCE=<label>` /
   `SWIFT_FIXTURE_CADENCE_INTERVAL_HOURS=<hours>`) before running the script so
   the state file captures the on-call owner for CI alerts.

## Triage workflow

1. **Inspect the dashboard entry**
   - Run `make swift-dashboards` to render the local summary using the same JSON
     feeds as CI. Note the instruction name, owner, and diff age.
   - If the diff originates from an intentional Rust change, confirm the culprit
     commit and check whether Android/Python already mirrored it.
   - For CI context, review the `device_tag` printed by `dashboards/mobile_ci.swift`
     (and stored in Buildkite metadata `ci/xcframework-smoke:<lane>:device_tag`) so you
     can confirm whether the latest failure involved the `iphone-sim`, `ipad-sim`,
     `strongbox`, or `mac-fallback` lane.
   - Check the telemetry summary row (salt epoch, rotation age, overrides). If values
     look stale, re-run `python3 scripts/swift_collect_redaction_status.py \
     --salt-config dashboards/data/swift_salt_status.sample.json \
     --overrides-store artifacts/swift_telemetry_overrides.json` to verify the raw inputs
     before regenerating the parity feed with
     `python3 scripts/swift_enrich_parity_feed.py --input dashboards/data/mobile_parity.sample.json --output /tmp/parity.telemetry.json --telemetry-json /tmp/telemetry.json`.
2. **Validate fixture parity locally**
   - Run `make swift-fixtures-check`. This invokes
     `scripts/check_swift_fixtures.py IrohaSwift/Fixtures` and prints any
     mismatched files with their SHA-256 hashes.
   - For isolated instruction files you can regenerate only the impacted subset by
     passing an override:
     ```bash
     SWIFT_FIXTURE_SOURCE=/path/to/export/out \
     SWIFT_FIXTURE_OUT=IrohaSwift/Fixtures \
     scripts/swift_fixture_regen.sh
     ```
3. **Update fixtures**
   - When the Rust exporter publishes a signed archive, set
     `SWIFT_FIXTURE_ARCHIVE=/path/to/norito-fixtures.tar.gz` (or `.zip`) before running
     `make swift-fixtures`. The script extracts the archive to a temporary directory,
     mirrors the contents into `IrohaSwift/Fixtures`, and annotates the cadence state
     with `source_kind=archive`, the digest, and archive path for CI/alerting.
   - If the diff is expected, regenerate from the Android source (temporary until
     the shared Rust exporter emits Swift artifacts) and re-run the checker.
   - Verify the Git diff only contains `.norito` and JSON fixture updates. Commit
     messages should mention the originating Rust change or governance decision.
   - Confirm `artifacts/swift_fixture_regen_state.json` reflects the current
     rotation owner and timestamp; CI fails when the age exceeds the 48 h SLA
     (`SWIFT_FIXTURE_MAX_AGE_HOURS`, default 48). When CI needs to tolerate more
     than one cadence window (e.g., scheduled + fallback), set
     `SWIFT_FIXTURE_EXPECTED_CADENCE=weekly-wed-1700utc,fallback-mon-thu-utc`
     before running `ci/check_swift_fixtures.sh`.
4. **Re-run Swift tests**
   - Execute `swift test --package-path IrohaSwift` to ensure the new fixtures
     still pass the fallback encoders.
   - When `/v1/pipeline` changes are involved, also run `make swift-ci` to confirm the
     regen SLA block turns green and telemetry metadata stays intact.
5. **Communicate**
- Update `status.md` (Latest Updates + iOS section) with a short note linking
  to the fixture refresh commit.
- Run `python3 scripts/swift_status_export.py --format markdown` (override
  `--parity`/`--ci` with the latest feeds or `--parity-url`/`--ci-url` for remote
  sources) to capture the metrics snippet for the
  weekly digest (`docs/source/status/swift_weekly_digest.md`) and paste the
  output into the status export template.
- Post the outcome in the Swift parity Slack channel, mentioning any blocked
  actions (e.g., pending governance decision or Torii backlog item).
- If regen exceeds the 48 h SLA, file an incident in the Swift program tracker
  and note the mitigation in the dashboard alert list. Coordinate with the CI
  operations playbook so Buildkite annotations, `ci/xcframework-smoke:<lane>:device_tag`
  metadata, and incident ownership stay aligned, and ensure dashboard alert text mirrors
  the CI incident summary.

## Telemetry overrides & salt alignment

Use the override CLI whenever support engineering grants temporary telemetry access:

```bash
# List existing overrides
python3 scripts/swift_status_export.py telemetry-override list --store artifacts/swift_telemetry_overrides.json

# Create a temporary override (24h default)
python3 scripts/swift_status_export.py telemetry-override create \
  --actor-role support \
  --reason "manual inspection" \
  --store artifacts/swift_telemetry_overrides.json

# Revoke once the drill/incident ends
python3 scripts/swift_status_export.py telemetry-override revoke --id <uuid> --store artifacts/swift_telemetry_overrides.json
```

After any override change (or salt rotation), run
`python3 scripts/swift_collect_redaction_status.py --salt-config <salt.json> --overrides-store artifacts/swift_telemetry_overrides.json --output /tmp/telemetry.json`
and re-enrich the parity feed so the dashboards reflect the new counts. This keeps
`overrides_open` and `salt_rotation_age_hours` accurate for the weekly digest and
alerts.

## Escalation matrix

| Scenario | Primary | Secondary | Notes |
|----------|---------|-----------|-------|
| Fixture diff caused by Rust ABI change | Swift Lead | Android Foundations TL | Confirm governance approval before mirroring. |
| `/v1/pipeline` regression | Swift Lead | Torii PM | Coordinate with Torii backlog to avoid double-fixes. |
| Dashboard tooling failure | Swift Program PM | Telemetry owner | Re-run `scripts/render_swift_dashboards.sh` with verbose logs. |
| XCFramework parity gap | Swift QA Lead | Build Infra | Kick `ci/xcframework-smoke` lane and capture artifacts. |

Escalations should also update the `Outstanding Follow-Ups` table in `status.md`.

## Reference commands

```bash
# Sync fixtures from Android tree
make swift-fixtures

# Validate fixtures (CI uses the same command)
make swift-fixtures-check

# Render dashboards locally
make swift-dashboards

# End-to-end Swift tests
swift test --package-path IrohaSwift
```

For additional background see:

- `dashboards/mobile_parity.swift`
- `dashboards/mobile_ci.swift`
- `docs/source/references/ios_metrics.md`
- `scripts/swift_fixture_regen.sh`
- `scripts/check_swift_fixtures.py`
- `scripts/render_swift_dashboards.sh`
- `docs/source/references/ci_operations.md`
- `docs/source/swift_fixture_cadence_pre_read.md`
- `docs/source/sdk/swift/telemetry_chaos_checklist.md`
