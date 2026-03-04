---
lang: es
direction: ltr
source: docs/source/project_tracker/nsc37a_zk_ticket_schema.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1544046c586f36acd0f16031fd1f99928ec7425f4fe65e23ab957e647cb6ef02
source_last_modified: "2026-01-04T10:50:53.638189+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Draft schema outline for NSC-37a zero-knowledge streaming tickets. -->

# NSC-37a — ZK Ticket Schema Refresh

## Scope
- Update Norito streaming ticket structures to embed ZK commitments, nullifiers, and proof metadata.
- Align schema with `iroha_zkp_halo2` verifier registry entries and host-side acceptance checks.
- Provide migration guidance for ticket issuers, relay validators, and SDKs.

## Proposed Schema Changes (Draft)
- `StreamingTicket` fields:
  - `commitment: Hash32` – binding viewer-specific token.
  - `nullifier: Hash32` – prevents double-use of tickets.
  - `proof_id: VerifierId` – references halo2 verifier registry entry.
  - `issued_at: Timestamp` / `expires_at: Timestamp` – explicit validity window.
  - `policy: TicketPolicy` – optional struct containing relay constraints (max relays, geographic hints).
  - `capabilities: TicketCapabilities` – bitflags describing allowed streaming profiles.
- `TicketRevocation` fields:
  - `nullifier: Hash32`.
  - `reason_code: u16` (enum to be mapped).
  - `revocation_signature: Signature`.
- Update `PrivacyRouteUpdate` to carry optional proof payload commitment.

## Validation
- Schema hash changes must be recorded in Norito golden vectors.
- Host validation flow:
  - Verify `proof_id` exists and approved.
  - Check nullifier uniqueness via WSV map.
  - Validate proof using `iroha_zkp_halo2`.

## Tasks
- [x] Draft Norito structs and update `norito_streaming.md` documentation.
- [x] Generate serialization tests (`crates/norito/tests/`) covering tickets and revocations.
- [x] Coordinate with Core Host to define nullifier storage and validation errors.
- [ ] Update SDKs (Python/JS/Swift/Java) to parse/emit tickets with commitments. (Python ✅, Java ✅; Swift/JS pending)
- [x] Produce migration playbook (operator & governance guidance). See `docs/source/project_tracker/nsc37a_zk_ticket_migration_playbook.md`.
- [x] Align with NSC-37b implementation plan. See [NSC-37b alignment](#nsc-37b-alignment-plan).
- [x] Document upgrade steps for ticket issuers (generate commitments, store nullifiers). See [Upgrade playbook for issuers](#upgrade-playbook-for-ticket-issuers).
- [ ] Update golden files (`integration_tests/fixtures/norito_streaming_*`) once schema finalises.

## Open Questions
- Should tickets include relay-specific capabilities or remain generic?
- Preferred nullifier storage (dedicated map vs embed per privacy route).
- Required telemetry for tracking ticket usage/failures.

## Upgrade Playbook for Ticket Issuers

This checklist applies to relay operators and streaming partners that mint privacy-preserving
tickets. Follow the steps in order; each item records the artefacts that must be produced or
updated before the next migration phase begins.

1. **Bootstrap the commitment toolchain.**  
   - Pull the updated `norito_streaming_ticket` CLI (`cargo install --path crates/norito_streaming_ticket`).  
   - Configure the CLI with the issuer signing key (`tickets.toml` → `[issuer.signing_key]`).  
   - Run `norito_streaming_ticket commitments self-test` to verify the Halo2 commitment circuit
     and ensure the deterministic transcript matches the published vectors in
     `crates/norito/tests/streaming_ticket_commitment.rs`.

2. **Generate ticket commitments + nullifiers.**  
   - For each active streaming offer, invoke  
     `norito_streaming_ticket commitments issue --ticket tickets/<id>.json --out commitments/<id>.json`.  
   - The output JSON contains the `(commitment, nullifier)` pair alongside the attestation hash.
     Archive it under the issuer’s change-management directory (e.g., `audit/zk-tickets/YYYYMMDD/`).
   - Record the nullifier in the issuer’s operational database; schema:  
     `ticket_id TEXT PRIMARY KEY`, `nullifier BLOB`, `issued_at TIMESTAMP`.

3. **Publish verifier references.**  
   - Update the issuer manifest to include `proof_id` (Halo2 verifier registry identifier) and the
     commitment version. Use the helper script  
     `scripts/zk_ticket_embed_proof_id.py commitments/<id>.json manifests/<id>.json`.  
   - Submit the manifest through the normal governance channel (SoraFS pin registry or Kaigi relay)
     and ensure the approvals reference NSC-37a.

4. **Prepare WSV nullifier storage.**  
   - Coordinate with the Core Host team to deploy the `StreamingTicketNullifierMap` migration.  
   - For each existing ticket, call the admin endpoint
     `POST /admin/streaming/nullifiers` with payload  
     `{ "ticket_id": "...", "nullifier": "base64" }`.  
   - Verify the WSV mirror by querying `GET /admin/streaming/nullifiers/<ticket_id>`; the response
     must match the locally archived nullifier.

5. **Update issuance pipelines.**  
     - Python: `iroha_streaming.ticket.issue_ticket()`  
     - JavaScript: `sdk.streaming.issueTicket()` (pending release 0.9.0)  
     - Java: `StreamingTicketBuilder` (shipped in `norito-java 0.6.0`)  
     - Swift: `StreamingTicketComposer` (beta artefact `swift-sdk 0.10.0-beta2`)  
   - Ensure each pipeline writes the nullifier to the issuer DB and publishes the commitment artefact
     to the audit bucket.

6. **Run end-to-end rehearsal.**  
   - Spin up the staging environment and execute `scripts/zk_ticket_smoketest.sh --env staging`.  
   - Confirm:  
     - Tickets are accepted by the relay (commitment/non-nullifier checks pass).  
     - Revocations reference the nullifier and propagate across relays within the SLA.  
     - Telemetry `streaming.ticket_nullifier_conflicts_total` remains at zero.

7. **Sign-off and documentation.**  
   - Attach the smoke-test report and manifest change log to the issuer’s migration ticket.  
   - Update the operational runbook (`ops/streaming_tickets.md`) with the new CLI commands and
   - Notify the governance channel that NSC-37a issuer onboarding is complete.

All issuers must complete steps 1–4 before the network toggles the “nullifier required” policy
(target **2026-03-18**). Steps 5–7 should be wrapped up before the schema freeze on
**2026-03-25**.

## Timeline
- Circulate schema draft by **2026-03-05**.
- Core Host/Streaming review: **2026-03-07**.
- SDK update plan ready: **2026-03-15**.
- Final schema freeze: **2026-03-25**.

## NSC-37b Alignment Plan

NSC-37b introduces streaming policy enforcement and dispute workflows on top of the ticket schema
defined here. To avoid double work the two initiatives share the same data-model derivations,
migration switch, and telemetry hooks. Alignment checklist:

1. **Shared schema module (`iroha_data_model::streaming::ticket`).**  
   - NSC-37a uses the module for issuance/parsing.  
   - NSC-37b contributes enforcement helpers (`TicketPolicy::allows_profile`) and dispute metadata.  
   - Ownership: Core Host team (`@core-host`), PR #13807 (in review). Action: reuse module ID, no
     duplicate types or serde shims.

2. **WSV extensions.**  
   - NSC-37a adds `StreamingTicketNullifierMap`.  
   - NSC-37b adds `StreamingTicketUsageLedger` keyed by `(ticket_id, capability)` for dispute trails.  
   - Migration script (`ci/migrations/2026-03-17-streaming.sql`) seeds both structures. Action:
     confirm nullifier map migration remains idempotent; add ledger seeding hook guarded by the same
     feature flag (`streaming_tickets_v1`).

3. **Telemetry + alerts.**  
   - NSC-37a: `soranet_salt_skew_gauge` and `streaming.ticket_nullifier_conflicts_total`.  
   - NSC-37b: `streaming.ticket_usage_total`, `streaming.dispute_open_total`.  
   - Alignment action: register all metrics in `docs/source/telemetry.md` under a single section
     (“Streaming Tickets NSC-37a/b”) and ensure the Prometheus namespace/labels match.

4. **SDK surface.**  
   - All SDKs add `ticket.issue()` (NSC-37a).  
   - NSC-37b adds `ticket.report_usage()` and `ticket.raise_dispute()`.  
   - Action: each SDK task board references the shared milestone `SDK-178`. Shipping the NSC-37a
     release includes a dormant `streaming_disputes` module (gated by a feature flag) that exposes
     the method signatures for `report_usage`/`raise_dispute` and documents the expected Norito
     payloads. Enabling the flag in NSC-37b activates the implementations without additional public
     API churn.

5. **Operator communication.**  
   - Combined runbook page (`ops/streaming_tickets.md`) now includes the NSC-37a issuance section
     (this document) and NSC-37b dispute handling instructions.  
   - Governance announcement scheduled for **2026-03-18** will mention both migration toggles and
     reference the consolidated playbooks.

Outstanding NSC-37b-specific work is tracked in `docs/source/project_tracker/nsc37b_streaming_enforcement.md`;
this plan ensures both tracks land coherently and prevents diverging schema updates.

## Appendix: Draft Norito Representation (pseudo-code)
```
struct StreamingTicket {
    ticket_id: TicketId,
    owner: AccountId,
    commitment: Hash32,
    nullifier: Hash32,
    proof_id: VerifierId,
    issued_at: Timestamp,
    expires_at: Timestamp,
    policy: Option<TicketPolicy>,
    capabilities: TicketCapabilities,
    signature: Signature,
}

struct TicketPolicy {
    max_relays: u16,
    allowed_regions: Vec<RegionCode>,
    max_bandwidth_kbps: Option<u32>,
}

bitflags TicketCapabilities {
    const LIVE = 0x01;
    const VOD = 0x02;
    const PREMIUM_PROFILE = 0x04;
    const HDR = 0x08;
    const SPATIAL_AUDIO = 0x10;
}
```
