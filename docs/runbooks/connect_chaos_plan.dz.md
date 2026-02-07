---
lang: dz
direction: ltr
source: docs/runbooks/connect_chaos_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b1d414173d2f43d403a6a1ba5cd59a645cb0b94f5765e69a00f7078b1e96b1cd
source_last_modified: "2025-12-29T18:16:35.913527+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Connect Chaos & Fault Rehearsal Plan (IOS3 / IOS7)

This playbook defines the repeatable chaos drills that satisfy the IOS3/IOS7
roadmap action _“plan joint chaos rehearsal”_ (`roadmap.md:1527`). Pair it with
the Connect preview runbook (`docs/runbooks/connect_session_preview_runbook.md`)
when staging cross-SDK demos.

## Goals & Success Criteria
- Exercise the shared Connect retry/back-off policy, offline queue limits, and
  telemetry exporters under controlled faults without mutating production code.
- Capture deterministic artefacts (`iroha connect queue inspect` output,
  `connect.*` metrics snapshots, Swift/Android/JS SDK logs) so governance can
  audit every drill.
- Prove that wallets and dApps honour config changes (manifest drifts, salt
  rotation, attestation failures) by surfacing the canonical `ConnectError`
  category and redaction-safe telemetry events.

## Prerequisites
1. **Environment bootstrap**
   - Start the demo Torii stack: `scripts/ios_demo/start.sh --telemetry-profile full`.
   - Launch at least one SDK sample (`examples/ios/NoritoDemoXcode/NoritoDemoXcode`,
     `examples/ios/NoritoDemo`, Android `demo-connect`, JS `examples/connect`).
2. **Instrumentation**
   - Enable SDK diagnostics (`ConnectQueueDiagnostics`, `ConnectQueueStateTracker`,
     `ConnectSessionDiagnostics` in Swift; `ConnectQueueJournal` + `ConnectQueueJournalTests`
     equivalents in Android/JS).
   - Ensure the CLI `iroha connect queue inspect --sid <sid> --metrics` resolves
     the queue path produced by the SDK (`~/.iroha/connect/<sid>/state.json` and
     `metrics.ndjson`).
   - Wire telemetry exporters so the following time series are visible in
     Grafana and via `scripts/swift_status_export.py telemetry`: `connect.queue_depth`,
     `connect.queue_dropped_total`, `connect.reconnects_total`,
     `connect.resume_latency_ms`, `swift.connect.frame_latency`,
     `android.telemetry.redaction.salt_version`.
3. **Evidence folders** – create `artifacts/connect-chaos/<date>/` and store:
   - raw logs (`*.log`), metrics snapshots (`*.json`), dashboard exports
     (`*.png`), CLI outputs, and PagerDuty IDs.

## Scenario Matrix

| ID | Fault | Injection steps | Expected signals | Evidence |
|----|-------|-----------------|------------------|----------|
| C1 | WebSocket outage & reconnect | Wrap `/v1/connect/ws` behind a proxy (e.g., `kubectl -n demo port-forward svc/torii 18080:8080` + `toxiproxy-cli toxic add ... timeout`) or temporarily block the service (`kubectl scale deploy/torii --replicas=0` for ≤60 s). Force the wallet to keep sending frames so offline queues fill. | `connect.reconnects_total` increments, `connect.resume_latency_ms` spikes but stays <1 s P95, queues enter `state=Draining` via `ConnectQueueStateTracker`. SDKs emit `ConnectError.Transport.reconnecting` once, then resume. | - `iroha connect queue inspect --sid <sid>` output showing non-zero `resume_attempts_total`.<br>- Dashboard annotation for outage window.<br>- Sample log excerpt with reconnect + drain messages. |
| C2 | Offline queue overflow / TTL expiry | Patch the sample to shrink queue limits (Swift: instantiate `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` inside `ConnectSessionDiagnostics`; Android/JS use corresponding constructors). Suspend the wallet for ≥2× `retentionInterval` while the dApp keeps enqueuing requests. | `connect.queue_dropped_total{reason="overflow"}` and `{reason="ttl"}` increment, `connect.queue_depth` plateaus at the new limit, SDKs surface `ConnectError.QueueOverflow(limit: 4)` (or `.QueueExpired`). `iroha connect queue inspect` shows `state=Overflow` with `warn/drop` watermarks at 100%. | - Screenshot of the metric counters.<br>- CLI JSON output capturing overflow.<br>- Swift/Android log snippet containing the `ConnectError` line. |
| C3 | Manifest drift / admission rejection | Tamper with the Connect manifest served to wallets (e.g., modify `docs/connect_swift_ios.md` sample manifest, or start Torii with `--connect-manifest-path` pointing at a copy where `chain_id` or `permissions` differ). Have the dApp request approval and ensure the wallet rejects via policy. | Torii returns `HTTP 409` for `/v1/connect/session` with `manifest_mismatch`, SDKs emit `ConnectError.Authorization.manifestMismatch(manifestVersion)`, telemetry raises `connect.manifest_mismatch_total`, and queues remain empty (`state=Idle`). | - Torii log excerpt showing mismatch detection.<br>- SDK screenshot of surfaced error.<br>- Metrics snapshot proving no queued frames during the test. |
| C4 | Key rotation / salt-version bump | Rotate the Connect salt or AEAD key mid-session. In dev stacks, restart Torii with `CONNECT_SALT_VERSION=$((old+1))` (mirrors the Android redaction salt test in `docs/source/sdk/android/telemetry_schema_diff.md`). Keep the wallet offline until salt rotation completes, then resume. | First resume attempt fails with `ConnectError.Authorization.invalidSalt`, queues flush (dApp drops cached frames with reason `salt_version_mismatch`), telemetry emits `android.telemetry.redaction.salt_version` (Android) and `swift.connect.session_event{event="salt_rotation"}`. Second session after SID refresh succeeds. | - Dashboard annotation with salt epoch before/after.<br>- Logs containing the invalid-salt error and subsequent success.<br>- `iroha connect queue inspect` output showing `state=Stalled` followed by fresh `state=Active`. |
| C5 | Attestation / StrongBox failure | On Android wallets, configure `ConnectApproval` to include `attachments[]` + StrongBox attestation. Use the attestation harness (`scripts/android_keystore_attestation.sh` with `--inject-failure strongbox-simulated`) or tamper with the attestation JSON before handing to the dApp. | DApp rejects the approval with `ConnectError.Authorization.invalidAttestation`, Torii logs the failure reason, exporters bump `connect.attestation_failed_total`, and the queue purges the offending entry. Swift/JS dApps log the error while keeping the session alive. | - Harness log with injected failure ID.<br>- SDK error log + telemetry counter capture.<br>- Evidence that the queue removed the bad frame (`recordsRemoved > 0`). |

## Scenario Details

### C1 — WebSocket outage & reconnect
1. Wrap Torii behind a proxy (toxiproxy, Envoy, or a `kubectl port-forward`) so
   you can toggle availability without killing the entire node.
2. Trigger a 45 s outage:
   ```bash
   toxiproxy-cli toxic add connect-ws --type timeout --toxicity 1.0 --attribute timeout=45000
   sleep 45 && toxiproxy-cli toxic remove connect-ws --toxic timeout
   ```
3. Observe telemetry dashboards and `scripts/swift_status_export.py telemetry
   --json-out artifacts/connect-chaos/<date>/c1_metrics.json`.
4. Dump queue state immediately after the outage:
   ```bash
   iroha connect queue inspect --sid "$SID" --metrics > artifacts/connect-chaos/<date>/c1_queue.txt
   ```
5. Success = a single reconnect attempt, bounded queue growth, and automatic
   drain after the proxy recovers.

### C2 — Offline queue overflow / TTL expiry
1. Shrink queue thresholds in local builds:
   - Swift: update the `ConnectQueueJournal` initializer inside your sample
     (e.g., `examples/ios/NoritoDemoXcode/NoritoDemoXcode/ContentView.swift`)
     to pass `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)`.
   - Android/JS: pass the equivalent config object when constructing
     `ConnectQueueJournal`.
2. Suspend the wallet (simulator background or device airplane mode) for ≥60 s
   while the dApp issues `ConnectClient.requestSignature(...)` calls.
3. Use `ConnectQueueDiagnostics`/`ConnectQueueStateTracker` (Swift) or the JS
   diagnostics helper to export the evidence bundle (`state.json`, `journal/*.to`,
   `metrics.ndjson`).
4. Success = overflow counters increment, SDK surfaces `ConnectError.QueueOverflow`
   once, and the queue recovers after the wallet resumes.

### C3 — Manifest drift / admission rejection
1. Make a copy of the admission manifest, e.g.:
   ```bash
   cp configs/connect_manifest.json /tmp/manifest_drift.json
   sed -i '' 's/"chain_id": ".*"/"chain_id": "bogus-chain"/' /tmp/manifest_drift.json
   ```
2. Launch Torii with `--connect-manifest-path /tmp/manifest_drift.json` (or
   update the docker compose/k8s config for the drill).
3. Attempt to start a session from the wallet; expect HTTP 409.
4. Capture Torii + SDK logs plus `connect.manifest_mismatch_total` from
   the telemetry dashboard.
5. Success = rejection without queue growth, plus the wallet displays the shared
   taxonomy error (`ConnectError.Authorization.manifestMismatch`).

### C4 — Key rotation / salt bump
1. Record the current salt version from telemetry:
   ```bash
   scripts/swift_status_export.py telemetry --json-out artifacts/connect-chaos/<date>/c4_before.json
   ```
2. Restart Torii with a new salt (`CONNECT_SALT_VERSION=$((OLD+1))` or update the
   config map). Keep the wallet offline until the restart completes.
3. Resume the wallet; the first resume should fail with an invalid-salt error
   and `connect.queue_dropped_total{reason="salt_version_mismatch"}` increments.
4. Force the app to drop cached frames by deleting the session directory
   (`rm -rf ~/.iroha/connect/<sid>` or the platform-specific cache clear), then
   restart the session with fresh tokens.
5. Success = telemetry shows the salt bump, the invalid resume event is logged
   once, and the next session succeeds without manual intervention.

### C5 — Attestation / StrongBox failure
1. Generate an attestation bundle using `scripts/android_keystore_attestation.sh`
   (set `--inject-failure strongbox-simulated` to flip the signature bit).
2. Have the wallet attach this bundle via its `ConnectApproval` API; the dApp
   should validate and reject the payload.
3. Verify telemetry (`connect.attestation_failed_total`, Swift/Android incident
   metrics) and ensure the queue dropped the poisoned entry.
4. Success = rejection is isolated to the bad approval, queues stay healthy,
   and the attestation log is stored with the drill evidence.

## Evidence Checklist
- `artifacts/connect-chaos/<date>/c*_metrics.json` exports from
  `scripts/swift_status_export.py telemetry`.
- CLI outputs (`c*_queue.txt`) from `iroha connect queue inspect`.
- SDK + Torii logs with timestamps and SID hashes.
- Dashboard screenshots with annotations for each scenario.
- PagerDuty / incident IDs if Sev 1/2 alerts fired.

Completing the full matrix once per quarter satisfies the roadmap gate and
shows that Swift/Android/JS Connect implementations respond deterministically
across the highest-risk failure modes.
