---
title: SoraFS Chaos Drill Log
summary: Registry of executed chaos drills and incident rehearsals.
---

| Date | Scenario | Status | Incident Commander | Scribe | Start (UTC) | End (UTC) | Notes | Follow-up / Incident Link |
|------|----------|--------|--------------------|--------|-------------|-----------|-------|---------------------------|
| 2026-03-28 | SoraNet salt rotation tabletop (SNNet-1b harness) | pass | Salt Council Ops Lead | Relay Ops | 16:00Z | 17:30Z | Ran harness-assisted rotation and recovery drill, captured Norito transcripts + telemetry snapshots, and verified client/relay recovery timings within SLA. | Evidence archived in `artifacts/soranet_drill/2026-03-28-salt-rotation.json` |
| 2026-03-21 | SoraNet salt rotation tabletop drill | pass | Crypto WG | Ops Bot | 16:00Z | 17:05Z | Ran `soranet-handshake-harness simulate` (artifacts/soranet_drill/2026-03-21-tabletop.json) and reviewed telemetry + frames with relay/client operators. | docs/source/soranet_salt_plan.md#review--approval |
| 2025-04-03 | SoraDNS transparency IR drill (Q2 2025) | scheduled | Transparency Oncall | Ops Bot | 14:00Z | - | Quarterly transparency rehearsal (scheduled automatically). | - |
| 2025-03-04 | Gateway outage chaos drill (automation dry-run) | pass | Automation Harness | Ops Bot | 10:00Z | 10:30Z | Simulated run to verify logging automation | - |
| 2025-03-04 | Proof failure surge drill (automation dry-run) | pass | Automation Harness | Ops Bot | 11:00Z | 11:45Z | Injected synthetic proof errors in harness | - |
| 2025-03-04 | Replication lag drill (automation dry-run) | follow-up | Automation Harness | Ops Bot | 12:30Z | 13:10Z | Queued follow-up to tune backlog thresholds | - |
| 2025-11-08 | tls-renewal | fail | Automation Harness | Ops Bot | 05:40Z | 05:40Z | Probe log: artifacts/sorafs_gateway_probe/demo/probe_20251108T054055Z.log; report: artifacts/sorafs_gateway_probe/demo/probe_20251108T054055Z.json; rollback: docs/source/sorafs_gateway_tls_automation.md#emergency-certificate-rotation; summary: failures: HTTP status, Sora-Proof decode; Local synthetic drill | - |
| 2025-11-08 | tls-renewal | fail | Automation Harness | Ops Bot | 10:43Z | 10:43Z | Probe log: artifacts/sorafs_gateway_probe/demo/probe_20251108T104355Z.log; report: artifacts/sorafs_gateway_probe/demo/probe_20251108T104355Z.json; rollback: docs/source/sorafs_gateway_tls_automation.md#emergency-certificate-rotation; Local synthetic drill (pager live) | - |
| 2025-11-08 | tls-renewal | fail | Automation Harness | Ops Bot | 10:48Z | 10:48Z | Probe log: artifacts/sorafs_gateway_probe/demo/probe_20251108T104812Z.log; report: artifacts/sorafs_gateway_probe/demo/probe_20251108T104812Z.json; rollback: docs/source/sorafs_gateway_tls_automation.md#emergency-certificate-rotation; Local synthetic drill (pager live) | - |
| 2025-11-08 | tls-renewal | fail | Automation Harness | Ops Bot | 10:49Z | 10:49Z | Probe log: artifacts/sorafs_gateway_probe/demo/probe_20251108T104901Z.log; report: artifacts/sorafs_gateway_probe/demo/probe_20251108T104901Z.json; rollback: docs/source/sorafs_gateway_tls_automation.md#emergency-certificate-rotation; summary: failures: HTTP status, Sora-Proof decode; Local synthetic drill (pager live) | - |
