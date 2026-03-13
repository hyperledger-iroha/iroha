---
lang: ba
direction: ltr
source: docs/source/soranet/zk_ticket_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b5add8b4faacc0ba0334550099f114808425793b0602e742c3b1e7b9650153c5
source_last_modified: "2025-12-29T18:16:36.203251+00:00"
translation_last_reviewed: 2026-02-07
title: Zero-Knowledge Privacy Tickets
summary: Final specification for NSC-37a/b covering ticket schema, Halo2 circuits, host integration, observability, and rollout.
---

# Zero-Knowledge Privacy Tickets for SoraNet / SoraFS

## Goals & Scope
- Finalise the Norito schema and cryptographic commitments for privacy tickets used by SoraNet and SoraFS streaming requests (NSC-37a).
- Implement Halo2 proof generation/verification in `iroha_zkp_halo2` and thread validation through Torii/Streaming handles (NSC-37b).
- Provide tooling, observability, and rollout guidance so clients, gateways, and validators can issue, verify, and audit tickets deterministically.

## Ticket Architecture
- Tickets authorise anonymous access to SoraFS objects routed over SoraNet.
- A ticket encapsulates:
  - The blinded CID / manifest being accessed.
  - Capability scope (read-only, write, duration).
  - Expiration epoch and optional rate limits.
- Tickets are bound to an issuer key (gateway or governance authority) and validated by relays/gateways without revealing account identity.

## Norito Schema (NSC-37a)
- Define canonical payloads in `soranet::ticket` module:
  ```norito
  struct TicketBodyV1 {
      blinded_cid: Digest32,
      scope: TicketScopeV1,       // read | write | admin
      max_uses: u16,
      valid_after: Timestamp,
      valid_until: Timestamp,
      issuer_id: AccountId,
      salt_epoch: u32,
      policy_flags: u32,
      metadata: Metadata,
  }
  struct TicketEnvelopeV1 {
      body: TicketBodyV1,
      commitment: Digest32,       // commitment to body fields
      zk_proof: Vec<u8>,
      signature: Signature,       // issuer signature (Dilithium3)
  }
  ```
- Commitments: BLAKE3 Merkle commitment over body fields; metadata optional but hashed.
  - Leaf hash: `BLAKE3("soranet.ticket.body.<field>.v1" || le32(len) || value)` with fields ordered as `blinded_cid`, `scope`, `max_uses`, `valid_after`, `valid_until`, `issuer_id`, `salt_epoch`, `policy_flags`, `metadata`.
  - Internal node hash: `BLAKE3("soranet.ticket.body.node.v1" || left || right)`; if a level has an odd number of leaves the last node is duplicated before hashing.
- Schema documented with golden vectors; hashed schema version appended to roadmap.
- Tickets transported via Torii endpoints `/v2/soranet/ticket/issue`, `/verify`, and attached to SoraFS streaming requests (`Sora-Privacy-Ticket` header).

## Halo2 Circuit & Proof Flow (NSC-37b)
- Circuit (`TicketCircuitV1`) proves:
  - Commitment matches blinded CID, scope, expiry, nonce.
  - Optional rate-limit parameters satisfy constraints.
  - Nullifier derived as `Poseidon(blinded_cid, issuer_secret, use_counter)` for double-spend detection.
- Proving:
  - Client/gateway uses `iroha_zkp_halo2::ticket::prove` with proving key (PKEY) stored locally.
  - Nullifier appended to proof payload; host maintains nullifier set to prevent replay.
- Verification:
  - Torii `StreamingHandle` invokes `iroha_zkp_halo2::ticket::verify` with verifying key (VKEY) loaded from registry.
  - Registry stored in genesis (`ticket_verifying_key@zk_lane`) and updateable via governance.
  - Failed verification rejects request (HTTP 403) and logs event.

## Host Integration
- Torii / streaming changes:
  - Extend request context with `ticket_envelope`.
  - Validation pipeline: signature check, nullifier replay check, Halo2 verification, policy evaluation.
  - On success, request proceeds; on failure, log + emit telemetry.
- Governance integration:
  - Tickets can be issued by authorised gateways; governance sets issuer list (`TicketIssuerRegistryV1`).
  - Emergency revocation via `TicketRevocationListV1` published to directory/consensus.

## Storage & Data Availability
- Nullifiers stored in RocksDB with TTL equal to `valid_until`; removal logged.
- Ticket issuance events appended to transparency ledger (`TokenIssuanceLogV1`).
- Optional archival of proofs in S3/IPFS for audit.

## Observability & Alerts
- Metrics:
  - `soranet_ticket_verify_total{result}`
  - `soranet_ticket_nullifier_replay_total`
  - `soranet_ticket_issue_total{issuer}`
  - `soranet_ticket_latency_seconds`
- Alerts:
  - Verification failure rate > 1% (suspect misconfiguration or attack).
  - Nullifier replay spikes.
  - Ticket issuance anomaly (unexpected issuer).
  - VKEY mismatch (clients/servers on different versions).
- Logs: structured `ticket_event` entries with blinded CID hash, issuer, verdict.

## Tooling & SDKs
- CLI:
  - `sorafs ticket issue --cid <cid> --scope read --valid 1h` (gateway issuance).
  - `sorafs ticket verify --file ticket.json`.
- SDK helper functions to generate/submit proofs; include sample code in Rust/TS.
- Integration tests using fixtures under `fixtures/zk/ticket/` with known VKEY/PKEY.

## Testing & Rollout
- Unit tests for schema serialization, commitment hash, nullifier logic.
- Halo2 circuit tests (prover/verify, negative cases) via `cargo test -p iroha_zkp_halo2`.
- Integration tests (`integration_tests/tests/soranet_ticket.rs`) hitting Torii endpoints.
- Stage rollout:
  1. Deploy to Sora Testus (feature-flagged), monitor metrics.
  2. Run chaos/latency tests; ensure no regressions.
  3. Promote verifying keys & governance config to Nexus; enable by default.

## Implementation Checklist
- [x] Finalise Norito schema + commitments with golden vectors (NSC-37a).
- [x] Implement Halo2 circuits/proving/verification and host integration (NSC-37b).
- [x] Update Torii/SDK tooling, metrics, and transparency pipeline.
- [x] Document rollout and testing procedures.

Delivering the above ensures privacy tickets can be issued and validated securely across SoraNet/SoraFS before Sora Nexus is exposed to public traffic.
