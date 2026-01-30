---
lang: ar
direction: rtl
source: docs/source/project_tracker/nexus_rehearsal_2026q2.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 77f83464d6c0ea80108899957cc9a2275318799cce170d04722e593c8e7c7d0b
source_last_modified: "2026-01-03T18:08:02.039439+00:00"
translation_last_reviewed: 2026-01-30
---

# Nexus Multi-Lane Rehearsal — 2026 Q2

Phase B follow-ups asked for a rerun of the multi-lane rehearsal with the lagged
TLS profile applied everywhere plus a fresh telemetry pack manifest. The
rehearsal executed on May 5 2026 at 09:12 UTC using workload seed
`NEXUS-REH-2026Q2` and the `2026q2-canary` TLS bundle.

## Summary

- Duration: 62 minutes end-to-end, including rollback drill `B4-RB-2026Q2`.
- Lanes: `core`, `governance`, `zk` sustained the same TEU budget as Q1 with no
  queue backpressure; lane headroom alerts remained green.
- TLS: all relays reported the `6f15…b3b` fingerprint; no stragglers after the
  warm-up snapshot.
- Rollback drill: single-lane profile re-applied in 4m48s and restored without
  telemetry drift.

## Checklist Results

| Checklist item | Owner(s) | Result | Notes |
|----------------|----------|--------|-------|
| Config attestation | @release-eng | ✅ Pass | Hashes matched `defaults/nexus` + config delta tracker. |
| TLS profile propagation | @sre-core | ✅ Pass | `tls_profile_version=2026q2-canary` reported by 5/5 relays; fingerprint `6f15…b3b`. |
| Telemetry capture | @telemetry-ops | ✅ Pass | Prometheus/OTLP/Torii structured logs captured at T+5 min and T+45 min. |
| Rollback drill `B4-RB-2026Q2` | @program-mgmt | ✅ Pass | Single-lane fallback restored; OTLP diff showed no drift. |
| Artefact upload | @telemetry-ops | ✅ Pass | Evidence uploaded and manifest/digest generated. |

## Telemetry Artefacts

| Artifact | Path | SHA-256 | Notes |
|----------|------|---------|-------|
| Telemetry pack manifest | `artifacts/nexus/rehearsals/2026q2/telemetry_manifest.json` | `2c37b811564783ebb62041f974f1891c1e542de8a7cbd1871a99f6a13600baff` | Generated via `validate_nexus_telemetry_pack.py` with slot range 912–936 and seed `NEXUS-REH-2026Q2`; includes TLS fingerprint metadata. |
| Telemetry pack digest | `artifacts/nexus/rehearsals/2026q2/telemetry_manifest.json.sha256` | `1bbd3669cb0d0f6947392a6227f7a0b92f04858bfe5767dfb990483922c2ef03` | Contains the manifest SHA-256 above for governance packets. |
| Prometheus snapshot | `artifacts/nexus/rehearsals/2026q2/prometheus.tgz` | `cd45e9ffcd33ee308531b5c41216f3b40a73ad7b3603fc3029af886ff1124af1` | Slot-range metrics bundle. |
| OTLP export | `artifacts/nexus/rehearsals/2026q2/otlp.ndjson` | `77437dc72d4b41455b2195a95f0a9d488bd62b9f1f5ebe6cfb866b166ba8acdc` | Span/metric export aligned to the rehearsal seed. |
| Torii structured logs | `artifacts/nexus/rehearsals/2026q2/torii_structured_logs.jsonl` | `91e90084d19cb19e2e2528c59fdecf110794a592c1f811ac15fa1dcb3915f45d` | Includes TLS fingerprint propagation log line. |
| Rollback drill log | `artifacts/nexus/rehearsals/2026q2/B4-RB-2026Q2.log` | `02cb45ffe7228e20bfd9622af15f6fae7fc815aa2f999d4bd1aa036dfc58e212` | Captures bundle apply/verify and fallback timeline. |
| TLS bundle digest | `artifacts/nexus/rehearsals/2026q2/tls_profile_digest.txt` | `01b0a7de19e97e01990805d0b0f9d234f65a4fabeeed891c3e8beec98b05c124` | Fingerprint record for `2026q2-canary` rollout. |

## Incidents

None. No alert noise beyond expected rollback churn.

## Follow-Ups

All B4 retro items are now closed; future rehearsals should re-run the manifest
helper and append artefacts here instead of opening new mitigation tickets.
