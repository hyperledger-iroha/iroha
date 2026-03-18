---
lang: my
direction: ltr
source: docs/source/project_tracker/nsc37a_zk_ticket_migration_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e7083506e01d76648e7520eebe62e7332a0d4cfa787640b509fca5a22d4390b7
source_last_modified: "2026-01-05T09:28:12.036749+00:00"
translation_last_reviewed: 2026-02-07
title: "NSC-37a Zero-Knowledge Ticket Migration Playbook"
summary: Operational plan for rolling out the refreshed Norito streaming ticket schema.
---

# NSC-37a Zero-Knowledge Ticket Migration Playbook

This playbook describes how streaming operators, governance facilitators, and SDK teams should migrate to the refreshed zero-knowledge ticket schema defined in NSC-37a. It assumes the Norito structs in `norito_streaming.md` and the serialization tests under `crates/norito/tests/` have already landed in `main`.

## 1. Roles & Responsibilities

| Role | Responsibilities |
|------|------------------|
| **Schema Owners (ZK WG)** | Finalise the schema hash, publish golden serialization vectors, and coordinate halo2 verifier registry entries. |
| **Core Host Team** | Implement nullifier storage, host-side validation errors, and WSV migration tooling. |
| **Streaming Runtime Team** | Update relay validators and auditors to work with commitments/nullifiers. |
| **Governance Ops** | Approve the verifier registry entries, publish upgrade notices, and coordinate ticket issuer cut-overs. |

## 2. Prerequisites

1. `StreamingTicket` / `TicketRevocation` schema changes merged to `main`.
2. `iroha_zkp_halo2` exposes verifier IDs for the production proving key.
3. `StreamingTelemetry` extended to report `tickets_issued`, `tickets_revoked`, `nullifier_hits`.
4. Golden vectors (`fixtures/norito_streaming/tickets_v2/`) published and signed.
5. Core Host nullifier map migration tool (`cargo xtask streaming-nullifier-migrate`) available.

## 3. Migration Phases

### Phase A — Validation & Staging (Week 0)
- Enable the new schema behind feature flag `streaming.zk_tickets_v2` on staging networks.
- Run SDK integration tests (`integration_tests/tests/streaming_zk_ticket_roundtrip.rs`) across all language bindings.
- Perform load tests with auditor tools to ensure nullifier lookups stay within latency SLO (< 15 ms P95).

### Phase B — Governance Sign-off (Week 1)
- Governance Ops publishes the verifier registry entry and updated issuer policy.
- Announce the cut-over window to issuers and relay operators (minimum 5 business days notice).
- Issue a dry-run batch of tickets in staging and confirm revocation flows.

### Phase C — Production Soft-Launch (Week 2)
- Relay validators log both commitment and nullifier statuses and raise alerts on duplicate nullifiers.
- Operators monitor telemetry dashboards (`dashboards/streaming/zk_ticket_adoption.json`) for anomalies.

## 4. Testing Checklist

| Test | Owner | Description |
|------|-------|-------------|
| SDK serialization parity | SDK teams | Ensure new tickets round-trip with golden vectors and reject malformed payloads. |
| Host validation | Core Host | Confirm nullifier map rejects duplicates, proof IDs resolve, and error codes propagate to clients. |
| Relay auditor | Streaming Runtime | Validate cover traffic, commitment audit logs, and nullifier telemetry. |
| Governance replay | Governance Ops | Re-run acceptance scripts using updated verifier ID and confirm ledger entries. |

## 5. Rollback Plan

1. Toggle feature flag `streaming.zk_tickets_v2` off in issuers and relay validators.
3. Issue rollback notice via governance channels and retrigger audit scripts.
4. If a verifier mismatch caused the incident, publish corrected registry entries before attempting re-enablement.

## 6. Communication Templates

- **Issuer Notification:** Outline the new fields, cut-over timeline, and testing expectations.
- **Relay Operator Bulletin:** Provide instructions for enabling telemetry alerts and verifying nullifier map health.
- **Status Update:** Daily during Phase C to report adoption metrics and any incident IDs.

## 7. Post-Migration Tasks

- Close outstanding SDK tickets (Python `NT-451`, JS `NT-452`, Swift `NT-453`, Java `NT-454`).
- File an incident summary if rollback plan was invoked (link to `ops/incidents/`).

## 8. References

- Schema draft: `docs/source/project_tracker/nsc37a_zk_ticket_schema.md`
- Telemetry plan: `docs/source/project_tracker/nsc28b_av_sync_telemetry.md`
- Verifier registry guide: `docs/source/zk/halo2_verifier_registry.md`

