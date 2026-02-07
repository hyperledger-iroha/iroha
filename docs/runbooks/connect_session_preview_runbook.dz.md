---
lang: dz
direction: ltr
source: docs/runbooks/connect_session_preview_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d04af2ad3ae5cc6a9254236f5627850aab7c6517308e9f3a09650cbc1490168
source_last_modified: "2026-01-05T18:22:23.401292+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Connect Session Preview Runbook (IOS7 / JS4)

This runbook documents the end-to-end procedure for staging, validating, and
tearing down Connect preview sessions as required by roadmap milestones **IOS7**
and **JS4** (`roadmap.md:1340`, `roadmap.md:1656`). Follow these steps whenever
you demo the Connect strawman (`docs/source/connect_architecture_strawman.md`),
exercise the queue/telemetry hooks promised in the SDK roadmaps, or collect
evidence for `status.md`.

## 1. Preflight Checklist

| Item | Details | References |
|------|---------|------------|
| Torii endpoint + Connect policy | Confirm the Torii base URL, `chain_id`, and Connect policy (`ToriiClient.getConnectStatus()` / `getConnectAppPolicy()`). Capture the JSON snapshot in the runbook ticket. | `javascript/iroha_js/src/toriiClient.js`, `docs/source/sdk/js/quickstart.md#connect-sessions--queueing` |
| Fixture + bridge versions | Note the Norito fixture hash and bridge build you will use (Swift requires `NoritoBridge.xcframework`, JS requires `@iroha/iroha-js` ≥ the version that shipped `bootstrapConnectPreviewSession`). | `docs/source/sdk/swift/reproducibility_checklist.md`, `javascript/iroha_js/CHANGELOG.md` |
| Telemetry dashboards | Ensure the dashboards that chart `connect.queue_depth`, `connect.queue_overflow_total`, `connect.resume_latency_ms`, `swift.connect.session_event`, etc., are reachable (Grafana `Android/Swift Connect` board + exported Prometheus snapshots). | `docs/source/connect_architecture_strawman.md`, `docs/source/sdk/swift/telemetry_redaction.md`, `docs/source/sdk/js/quickstart.md` |
| Evidence folders | Pick a destination such as `docs/source/status/swift_weekly_digest.md` (weekly digest) and `docs/source/sdk/swift/connect_risk_tracker.md` (risk tracker). Store logs, metrics screenshots, and acknowledgements under `docs/source/sdk/swift/readiness/archive/<date>/connect/`. | `docs/source/status/swift_weekly_digest.md`, `docs/source/sdk/swift/connect_risk_tracker.md` |

## 2. Bootstrap the Preview Session

1. **Validate policy + quotas.** Call:
   ```js
   const status = await torii.getConnectStatus();
   console.log(status.policy.queue_max, status.policy.offline_timeout_ms);
   ```
   Fail the run if `queue_max` or TTL differs from the config you planned to
   test.
2. **Generate deterministic SID/URIs.** The `@iroha/iroha-js`
   `bootstrapConnectPreviewSession` helper ties SID/URI generation to Torii
   session registration; use it even when Swift will drive the WebSocket layer.
   ```js
   import {
     ToriiClient,
     bootstrapConnectPreviewSession,
   } from "@iroha/iroha-js";

   const client = new ToriiClient(process.env.TORII_BASE_URL, { chainId: "sora-mainnet" });
   const { preview, session, tokens } = await bootstrapConnectPreviewSession(client, {
     chainId: "sora-mainnet",
     appBundle: "dev.sora.example.dapp",
     walletBundle: "dev.sora.example.wallet",
     register: true,
   });
   console.log("sid", preview.sidBase64Url, "ws url", preview.webSocketUrl);
   ```
   - Set `register: false` to dry-run QR/deep-link scenarios.
   - Persist the returned `sidBase64Url`, deeplink URLs, and `tokens` blob in the
     evidence folder; the governance review expects these artefacts.
3. **Distribute secrets.** Share the deeplink URI with the wallet operator
   (swift dApp sample, Android wallet, or QA harness). Never paste raw tokens
   into chat; use the encrypted vault documented in the enablement packet.

## 3. Drive the Session

1. **Open the WebSocket.** Swift clients typically use:
   ```swift
   let connectURL = URL(string: preview.webSocketUrl)!
   let client = ConnectClient(url: connectURL)
   let sid: Data = /* decode preview.sidBase64Url into raw bytes using your harness helper */
   let session = ConnectSession(sessionID: sid, client: client)
   let recorder = ConnectReplayRecorder(sessionID: sid)
   session.addObserver(ConnectEventObserver(queue: .main) { event in
       logger.info("connect event", metadata: ["kind": "\(event.kind)"])
   })
   try client.open()
   ```
   Reference `docs/connect_swift_integration.md` for additional setup (bridge
   imports, concurrency adapters).
2. **Approve + sign flows.** DApps call `ConnectSession.requestSignature(...)`,
   while wallets respond via `approveSession` / `reject`. Each approval must log
   the hashed alias + permissions to match the Connect governance charter.
3. **Exercise queue + resume paths.** Toggle network connectivity or suspend the
   wallet to ensure the bounded queue and replay hooks log entries. JS/Android
   SDKs emit `ConnectQueueError.overflow(limit)` /
   `.expired(ttlMs)` when they drop frames; Swift should observe the same once
   IOS7 queue scaffolding lands (`docs/source/connect_architecture_strawman.md`).
   After you record at least one reconnect, run
   ```bash
   iroha connect queue inspect --sid "$SID" --root ~/.iroha/connect --metrics
   ```
   (or pass the export directory returned by `ConnectSessionDiagnostics`) and
   attach the rendered table/JSON to the runbook ticket. The CLI reads the same
   `state.json` / `metrics.ndjson` pair that `ConnectQueueStateTracker` produces,
   so governance reviewers can trace drill evidence without bespoke tooling.

## 4. Telemetry & Observability

- **Metrics to capture:**
  - `connect.queue_depth{direction}` gauge (should stay below policy cap).
  - `connect.queue_dropped_total{reason="overflow|ttl"}` counter (non-zero only
    during fault-injection).
  - `connect.resume_latency_ms` histogram (record the p95 after forcing a
    reconnect).
  - `connect.replay_success_total` / `connect.replay_error_total`.
  - Swift-specific `swift.connect.session_event` and
    `swift.connect.frame_latency` exports (`docs/source/sdk/swift/telemetry_redaction.md`).
- **Dashboards:** Update the Connect board bookmarks with annotation markers.
  Attach screenshots (or JSON exports) to the evidence folder alongside the raw
  OTLP/Prometheus snapshots pulled via the telemetry exporter CLI.
- **Alerting:** If any Sev 1/2 thresholds trigger (per `docs/source/android_support_playbook.md` §5),
  page the SDK Program Lead and document the PagerDuty incident ID in the runbook
  ticket before continuing.

## 5. Cleanup & Rollback

1. **Delete staged sessions.** Always delete preview sessions so queue depth
   alarms remain meaningful:
   ```js
   await client.deleteConnectSession(preview.sidBase64Url);
   ```
   For Swift-only test runs, call the same endpoint through the Rust/CLI helper.
2. **Purge journals.** Remove any persisted queue journals
   (`ApplicationSupport/ConnectQueue/<sid>.to`, IndexedDB stores, etc.) so the
   next run starts clean. Record the file hash before deletion if you need to
   debug a replay issue.
3. **File incident notes.** Summarise the run in:
   - `docs/source/status/swift_weekly_digest.md` (deltas block),
   - `docs/source/sdk/swift/connect_risk_tracker.md` (clear or downgrade CR-2
     once telemetry is in place),
   - the JS SDK changelog or recipe if new behaviour was validated.
4. **Escalate failures:**
   - Queue overflow without injected faults ⇒ file a bug against the SDK whose
     policy diverged from Torii.
   - Resume errors ⇒ attach `connect.queue_depth` + `connect.resume_latency_ms`
     snapshots to the incident report.
   - Governance mismatches (tokens reused, TTL exceeded) ⇒ raise with the SDK
     Program Lead and annotate `roadmap.md` during the next revision.

## 6. Evidence Checklist

| Artefact | Location |
|----------|----------|
| SID/deeplink/tokens JSON | `docs/source/sdk/swift/readiness/archive/<date>/connect/session.json` |
| Dashboard exports (`connect.queue_depth`, etc.) | `.../metrics/` subfolder |
| PagerDuty / incident IDs | `.../notes.md` |
| Cleanup confirmation (Torii delete, journal wipe) | `.../cleanup.log` |

Completing this checklist satisfies the “docs/runbooks updated” exit criterion
for IOS7/JS4 and gives governance reviewers a deterministic trail for every
Connect preview session.
