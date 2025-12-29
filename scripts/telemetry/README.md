# Telemetry Utility Scripts

Utility scripts for Android telemetry readiness and SoraFS chaos drill tracking.

- `run_schema_diff.sh`: produces Android vs Rust schema diff JSON artefacts used by
  the AND7 documentation. Supports both commit-based (`run_schema_diff.sh <android> <rust>`)
  and config-based (`--android-config <file> --rust-config <file>`) workflows,
  writes `artifacts/android/telemetry/schema_diff.prom` automatically, and accepts
  `--textfile-dir /var/lib/node_exporter/textfile_collector` (or the
  `ANDROID_SCHEMA_DIFF_TEXTFILE_DIR` env var) to mirror the metrics into the
  node_exporter textfile collector for dashboards/alerts. Requires the
  `telemetry_schema_diff` binary in the workspace.
- `compare_dashboards.py`: normalises Grafana dashboard exports (shared `metric_id`,
  units, and PromQL expressions) and emits a diff summary so Android vs Rust
  telemetry panels stay in lockstep. Used by `ci/check_android_dashboard_parity.sh`.
- `check_redaction_status.py`: summarises Android telemetry redaction health by reading the local cache (`artifacts/android/telemetry/status.json`) or a staging endpoint. Emits the runbook-friendly `STATUS`/`ALERT` lines, supports salt verification via `--expected-salt-epoch` / `--expected-salt-rotation` (or the matching env vars `ANDROID_TELEMETRY_EXPECTED_SALT_EPOCH` / `ANDROID_TELEMETRY_EXPECTED_SALT_ROTATION`), and can dump JSON for incident artefacts via `--json` (stdout only, suppresses text) or `--json-out <path>` (writes to a file while keeping the summary on stdout).
- `log_sorafs_drill.sh`: appends a row to `ops/drill-log.md` (or a custom `--log`
  target) so chaos drills and incident rehearsals remain auditable. Supports
  statuses `pass`, `fail`, `follow-up`, and `scheduled`.
- `inject_redaction_failure.sh`: injects or clears redaction mismatches for chaos
  rehearsals. By default it mutates the local status cache used by
  `check_redaction_status.py`; with `--status-url` it POSTs to a live admin
  endpoint.
- `generate_android_load.py` / `generate_android_load.sh`: synthesize Android
  telemetry payloads against `android-telemetry-stg`, with dry-run support,
  rate/duration knobs, and deterministic logging so runbook drills can seed
  traffic before queue replay tests.
- `run_sorafs_gateway_probe.sh`: wraps `cargo xtask sorafs-gateway-probe`,
  forces `--report-json` into `artifacts/sorafs_gateway_probe/`, captures the
  stdout/stderr log, parses the JSON summary, appends drill-log entries, and
  (optionally) triggers PagerDuty (`--pagerduty-dry-run` supported) plus
  rollback hooks whenever the probe fails so TLS/ECH drills have deterministic
  evidence.
- `reserve_ledger_digest.py`: converts `sorafs reserve ledger` JSON into
  dashboard-friendly summaries. The helper now accepts multiple `--ledger`
  paths + optional `--label` overrides, emits Markdown/JSON/NDJSON batches, and
  writes Prometheus textfiles (`--out-prom`) so economics dashboards ingest the
  same transfer feed used in governance evidence. Use it inside rent automation
  or to keep Alertmanager in sync with treasury executions.
- `capacity_reconcile.py`: compares the capacity fee ledger emitted by
  `/v1/sorafs/capacity/state` against executed XOR transfers (JSON or NDJSON)
  and reports missing/overpaid settlements or penalties. It writes JSON
  summaries and Prometheus textfiles so Alertmanager can fire on reconciliation
  gaps during nightly treasury runs.
- `validate_drill_log.sh [path]`: verifies that the drill log header matches the
  expected schema and that every row uses an allowed status.
- `test_sorafs_fetch_alerts.sh`: runs `promtool test rules` against the SoraFS
  fetch (`dashboards/alerts/tests/sorafs_fetch_rules.test.yml`) and SoraNet
  privacy (`dashboards/alerts/tests/soranet_privacy_rules.test.yml`) alert suites
  so both rule sets stay in sync with their dashboards.
- `run_privacy_dp.py`: generates deterministic differential-privacy calibration
  artefacts (`artifacts/soranet_privacy_dp/`) and the accompanying notebook
  under `notebooks/`. CI can execute this script to keep the published ε/δ
  summary and suppression matrix up to date.
- `run_privacy_dp_notebook.sh`: wraps `run_privacy_dp.py` and executes
  `notebooks/soranet_privacy_dp.ipynb` via `papermill` (or `jupyter nbconvert`
  as a fallback) so CI/governance pipelines ship the rendered notebook alongside
  refreshed artefacts.
- `swift_samples_metrics.py`: converts `scripts/check_swift_samples.sh`
  summaries into a Prometheus textfile (`swift_samples_*` gauges) so
  Buildkite can publish `swift_samples.prom` via `SWIFT_SAMPLES_METRICS_OUT`
  / `SWIFT_SAMPLES_METRICS_CLUSTER` for IOS5 dashboard ingestion.
- `validate_nexus_telemetry_pack.py`: validates Nexus rehearsal telemetry packs
  by ensuring the expected artefacts exist (`prometheus.tgz`, `otlp.ndjson`,
  `torii_structured_logs.jsonl`, rollback logs, etc.), computing SHA-256
  digests, and writing a signed manifest (`telemetry_manifest.json` +
  `.sha256`). Pass `--slot-range <start-end>` / `--workload-seed <value>` (or
  `--require-slot-range` / `--require-workload-seed`) to enforce that governance
  metadata is captured alongside the digests; the script normalises the
  boundaries and records both the canonical string and numeric bounds in the
  manifest. Use it after every Phase B4 rehearsal before uploading evidence to
  the tracker bucket.
- `package_and7_enablement.py`: walks enablement assets (deck, lab reports, quiz
  summaries, screenshot/recording manifests), computes SHA-256 digests, and
  writes JSON/Markdown manifests with optional asset copies under
  `<out>/assets/`. Use it to ship AND7 governance packets with deterministic
  evidence bundles.
- `check_slot_duration.py`: parses a Prometheus metrics export, aggregates
  `iroha_slot_duration_ms` histogram buckets, prints p50/p95/p99 slot latency,
  and enforces the NX-18 acceptance thresholds (p95 ≤ 1000 ms, p99 ≤ 1100 ms by
  default). Pass `--json-out <path>` to capture the summary in JSON so RC
  workflows can archive both the text output and a machine-readable record
  alongside the metrics snapshot used for gating.
- `bundle_slot_artifacts.py`: copies the Prometheus metrics file + JSON summary
  into a target directory (default `artifacts/nx18`) and writes
  `slot_bundle_manifest.json` with SHA-256 digests so NX-18 evidence bundles ship
  reproducible artefacts.
- `check_nexus_audit_outcome.py`: inspects a telemetry JSON log for
  `nexus.audit.outcome` events, enforces that the routed-trace audit completed
  successfully within the configured window, and archives the payload under
  `docs/examples/nexus_audit_outcomes/`. Use it to gate TRACE rehearsals and
  automate the “no event within 30 minutes” alert requirement.
- `run_soradns_transparency_tail.sh`: wraps the SoraDNS transparency tailer,
  exporting JSONL/TSV artefacts and Prometheus metrics for resolver events, and
  optionally pushing the metrics to a Pushgateway. Designed for the managed
  telemetry cron job backing DG-5a.
- `schedule_soradns_ir_drill.sh`: computes the next quarterly SoraDNS
  transparency IR drill slot and appends a scheduled entry to `ops/drill-log.md`
  so incident response rehearsals stay on cadence.
- `test_torii_norito_rpc_alerts.sh`: runs `promtool test rules` against the Norito
  RPC alert suite (`dashboards/alerts/tests/torii_norito_rpc_rules.test.yml`) so
  the Torii Norito dashboards and alerts remain in sync during rollout.
- `ci/run_android_telemetry_chaos_prep.sh`: convenience wrapper that runs the
  load generator, captures `check_redaction_status` output (text + JSON), copies
  pending queue dumps, computes SHA-256 manifests, and invokes
  `PendingQueueInspector` to emit `<label>.json` summaries whenever
  `ANDROID_PENDING_QUEUE_EXPORTS` is set (e.g.,
  `pixel8=/tmp/pending.queue,emulator=/tmp/emulator.queue`). The helper compiles
  the inspector classes on demand and can skip JSON generation via
  `ANDROID_PENDING_QUEUE_INSPECTOR=false`.
