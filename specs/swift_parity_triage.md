<!--
Swift Norito fixture triage runbook.
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
  `IrohaSwift/Fixtures` against the canonical `fixtures/norito_rpc/` corpus.
- Governance (council) notifies SDK owners about upcoming Norito discriminator/ABI
  changes that require fixture refresh.

## Cadence decision & fallback plan

### Primary cadence (governance approved on 2026‑01‑15)

- **Window.** Scheduled regeneration happens every Wednesday at 17:00 UTC with
  the Android Foundations TL owning odd weeks and the Swift Lead owning even
  weeks. The operator renders one complete owner publication into a new,
  create-only absolute external `--output-root` and records the run in the
  parity handoff.
- **SLA.** Any governance-approved discriminator/ABI change must be mirrored
  into Swift fixtures within 48 hours. `ci/check_swift_fixtures.sh` and the
  parity dashboards alert whenever the SLA is breached.
- **Reporting.** After each scheduled run, the owner posts the sealed
  publication identity in `#sdk-parity`, runs `make swift-fixtures-check`, and
  records the entry in `status.md` plus the weekly digest.

### Manual fallback cadence

Trigger the fallback whenever **either** of the following holds:

1. The scheduled slot slips by more than six hours without a published regen.
2. `swift_parity_regen_hours_since_success` stays above 72 hours or
   `swift_parity_status` remains `0` for six consecutive hours on the parity
   dashboard.

Fallback procedure:

1. Record the fallback reason and covering owner in the parity handoff.
2. Run the canonical owner into two independent absent absolute external
   directories,
   require identical exact path sets, entry types, modes, completion manifests,
   and every file byte, apply the reviewed identity-relative patch from either
   sealed tree, then run the parity check:
   ```bash
   cargo run --locked -p xtask --features dev-tools --bin xtask -- \
     norito-rpc-fixtures --output-root /path/to/first-new-norito-rpc-publication
   cargo run --locked -p xtask --features dev-tools --bin xtask -- \
     norito-rpc-fixtures --output-root /path/to/second-new-norito-rpc-publication
   cargo run --locked -p xtask --features dev-tools --bin xtask -- norito-rpc-verify
   make swift-fixtures-check
   ```
3. Build/export the same-revision Offline Cash authority, then execute the full
   Swift suite (or `make swift-ci` as well if `/v1/pipeline` helpers changed):
   ```bash
   export IROHA_KOTLIN_OFFLINE_CASH_FIXTURE_BIN="$(
     bash ci/build_offline_cash_swift_fixture.sh --locked
   )"
   swift test --package-path IrohaSwift
   ```
   This keeps the cross-language fixtures and builders in sync without a
   capability skip.
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
decision) and the next scheduled owner publication is executed on time, clear
the fallback reason in the handoff. Update the weekly digest with the exit date
and close any incident tickets tied to the fallback window.

## Preparation

1. Ensure the Rust toolchain and Android fixtures are up to date:
   ```bash
   git pull --rebase
   cargo run --locked -p xtask --features dev-tools --bin xtask -- \
     norito-rpc-fixtures --output-root /path/to/first-new-norito-rpc-publication
   cargo run --locked -p xtask --features dev-tools --bin xtask -- \
     norito-rpc-fixtures --output-root /path/to/second-new-norito-rpc-publication
   cargo run --locked -p xtask --features dev-tools --bin xtask -- norito-rpc-verify
   make android-fixtures-check
   ```
   Before any tracked update, compare the exact path sets, entry types, modes,
   completion manifests, and every file byte, then apply the reviewed
   identity-relative patch from either sealed root.
2. Confirm you can run Swift package tests locally:
   ```bash
   export IROHA_KOTLIN_OFFLINE_CASH_FIXTURE_BIN="$(
     bash ci/build_offline_cash_swift_fixture.sh --locked
   )"
   swift test --package-path IrohaSwift
   ```
3. Confirm the Swift descriptor mirror in the sealed owner publication exactly
   matches its canonical descriptor pair. Android is a generated consumer, not
   an alternate fixture source.

## Triage workflow

1. **Inspect the dashboard entry**
   - Run `make swift-dashboards` to render the local summary using the same JSON
     feeds as CI. Note the instruction name, owner, and diff age.
   - If the diff originates from an intentional Rust change, confirm the culprit
     commit. Android, Python, and Swift mirrors must arrive together from one
     canonical publication; a partial mirror update is invalid.
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
   - Regeneration is all-or-nothing; subset and alternate-source modes are not
     supported.
3. **Update fixtures**
   - Render two independent create-only owner publications at absent absolute
     external roots and require identical exact path sets, entry types, modes,
     completion manifests, and every file byte before applying the reviewed
     identity-relative tracked patch.
   - Verify the Git diff only contains the complete owned `.norito`, descriptor,
     manifest, schema, vector, and SDK mirror updates. Commit messages should
     mention the originating Rust change or governance decision.
   - Confirm `artifacts/swift_fixture_regen_state.json` reflects the current
     rotation owner and timestamp; CI fails when the age exceeds the 48 h SLA
     (`SWIFT_FIXTURE_MAX_AGE_HOURS`, default 48). When CI needs to tolerate more
     than one cadence window (e.g., scheduled + fallback), set
     `SWIFT_FIXTURE_EXPECTED_CADENCE=weekly-wed-1700utc,fallback-mon-thu-utc`
     before running `ci/check_swift_fixtures.sh`.
4. **Re-run Swift tests**
   - Build/export the path from `ci/build_offline_cash_swift_fixture.sh --locked`
     as `IROHA_KOTLIN_OFFLINE_CASH_FIXTURE_BIN`, then execute
     `swift test --package-path IrohaSwift` to ensure the new fixtures still
     pass the fallback encoders.
   - When `/v1/pipeline` changes are involved, also run `make swift-ci` to confirm the
     regen SLA block turns green and telemetry metadata stays intact.
5. **Communicate**
- Update `status.md` (Latest Updates + iOS section) with a short note linking
  to the fixture refresh commit.
- Run `python3 scripts/swift_status_export.py --format markdown` (override
  `--parity`/`--ci` with the latest feeds or `--parity-url`/`--ci-url` for remote
  sources) to capture the metrics snippet for the
  weekly digest (`specs/status/swift_weekly_digest.md`) and paste the
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
# Render the complete owner publication into two absent absolute external directories
cargo run --locked -p xtask --features dev-tools --bin xtask -- \
  norito-rpc-fixtures --output-root /path/to/first-new-norito-rpc-publication
cargo run --locked -p xtask --features dev-tools --bin xtask -- \
  norito-rpc-fixtures --output-root /path/to/second-new-norito-rpc-publication

# Compare exact paths/types/modes/manifests/bytes, apply the reviewed
# identity-relative tracked patch, then validate (CI uses the same check).
cargo run --locked -p xtask --features dev-tools --bin xtask -- norito-rpc-verify
make swift-fixtures-check

# Render dashboards locally
make swift-dashboards

# End-to-end Swift tests
export IROHA_KOTLIN_OFFLINE_CASH_FIXTURE_BIN="$(
  bash ci/build_offline_cash_swift_fixture.sh --locked
)"
swift test --package-path IrohaSwift
```

For additional background see:

- `dashboards/mobile_parity.swift`
- `dashboards/mobile_ci.swift`
- `specs/references/ios_metrics.md`
- `scripts/check_swift_fixtures.py`
- `scripts/render_swift_dashboards.sh`
- `specs/references/ci_operations.md`
- `specs/swift_fixture_cadence_pre_read.md`
- `specs/sdk/swift/telemetry_chaos_checklist.md`
