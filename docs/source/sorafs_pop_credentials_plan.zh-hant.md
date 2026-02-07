---
lang: zh-hant
direction: ltr
source: docs/source/sorafs_pop_credentials_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 36bf8486013b9ea169ceda85dca31c82b2e10ea345a508fd32247d276fa73e94
source_last_modified: "2025-12-29T18:16:36.173252+00:00"
translation_last_reviewed: 2026-02-07
title: Proof-of-Personhood Credential Pipeline
summary: Final specification for SFM-4b1 covering enrollment, credential format, ZK proofs, rotation, APIs, and security.
---

# Proof-of-Personhood Credential Pipeline

## Goals & Scope
- Issue, rotate, and revoke proof-of-personhood (PoP) credentials for juror participation in moderation panels and other governance workflows.
- Provide selective-disclosure credentials with zero-knowledge membership proofs to protect juror privacy while ensuring eligibility.
- Expose APIs, CLI tooling, and audit trails for enrollments, renewals, and revocations.

This specification completes **SFM-4b1 — POP credential pipeline**.

## Architecture Overview
| Component | Responsibility | Notes |
|-----------|----------------|-------|
| Enrollment portal (`pop_enroll`) | Guides candidates through identity verification and DID issuance. | Integrates with identity provider (KYC/attestation). |
| Credential issuer (`pop_issuer`) | Signs credentials, updates commitment tree, publishes rollups to governance. | Multi-sig service operated by Identity WG + Governance. |
| Credential registry | Stores Merkle commitment tree roots, revocation lists, event log. | Backed by Postgres + Governance DAG for public audit. |
| Juror client tools (`sorafs pop`) | CLI/SDK for downloading credentials, generating proofs, syncing revocations. | Works offline except for revocation sync. |
| Verification service (`pop_verifier`) | Validates proofs attached to sortition/vote payloads. | Embedded in sortition/voting services. |

## Credential Format
- Credential derived from BBS+ signature for selective disclosure (or CL signatures if preferred). Canonical Norito struct:
  ```norito
  struct PopCredentialV1 {
      version: u8,
      holder_did: String,
      juror_weight: u32,
      expiry_epoch: u32,
      issuance_epoch: u32,
      revocation_nonce: [u8; 16],
      attributes: Metadata,          // optional region, language, expertise tags
      issuer_signature: Signature,   // BBS+
  }
  ```
- Commitment tree record stored as:
  ```norito
  struct PopCredentialCommitmentV1 {
      version: u8,
      merkle_root: Digest32,
      valid_after_epoch: u32,
      valid_until_epoch: u32,
      revoked_nonces: Vec<[u8;16]>,
      issuer_signature: Signature,
  }
  ```

## Enrollment & Renewal Workflow
1. Candidate completes identity checks via portal (leveraging Sora identity service; supports KYC or community attestation).
2. Identity WG issues DID document referencing holder keys.
3. Candidate signs enrollment request `PopEnrollmentRequestV1` and submits deposit (if required).
4. `pop_issuer` generates credential, updates Merkle tree, publishes `PopCredentialCommitmentV1` to Governance DAG + Torii.
5. Credential delivered encrypted (sealed box using holder public key). CLI `sorafs pop receive --case <id>` stores credential securely (local encrypted store).
6. Renewal triggered 30 days prior to expiry; requires holder acknowledgement (`PopRenewalAckV1`) and optional revalidation. Revocation nonce rotated.
7. Revocations handled by `PopCredentialRevokeV1` referencing nonce and reason; recipients sync via revocation feed.

## Zero-Knowledge Proofs
- Jurors generate `JurorProofV1`:
  ```norito
  struct JurorProofV1 {
      case_id: Uuid,
      credential_root: Digest32,
      juror_commitment: Digest32, // hash of DID + nonce
      merkle_proof: Vec<Digest32>,
      zk_proof: Vec<u8>,          // Groth16 proof
      expiry_epoch: u32,
      signature: Signature,       // optional binding to juror key
  }
  ```
- Proof circuit ensures:
  - Credential included in root.
  - Not revoked / not expired.
  - Optional attribute constraints (region).
  - Juror DID bound to proof.
- Verification integrated into sortition service (SFM-4b) and voting backend.
- CLI `sorafs pop prove --case <id>` generates proof offline using stored credential + latest commitment root.

## APIs
- `POST /v1/pop/enroll` — submit enrollment request (with DID + attestation).
- `GET /v1/pop/manifest` — fetch latest commitment root and revocation list (versioned).
- `GET /v1/pop/revocations?since=` — incremental updates.
- `POST /v1/pop/renew` — ack renewal.
- `POST /v1/pop/revoke` — governance/issuer endpoint to revoke credential.
- `GET /v1/pop/status/{did}` — view credential metadata (authorized roles only).
- All endpoints require mTLS + JWT scopes (`pop.enroll`, `pop.manage`, etc.).

## CLI & SDK
- `sorafs pop sync` — download latest commitment root + revocations.
- `sorafs pop status` — show credential status, expiry, attributes.
- `sorafs pop prove --case <uuid>` — generate proof file.
- `sorafs pop revoke --nonce <hex>` (issuer).
- SDK functions: `PopClient::fetch_manifest`, `::verify_proof`, `::generate_proof`.

## Storage & Audit
- Credential store on holder device encrypted with passphrase + optional hardware token.
- Issuer logs events in `pop_events` table (enrollment, renewal, revoke). Daily hash of events recorded in Governance DAG.
- Commitment rollups pinned to IPFS for reproducibility.
- Revocation list includes reason codes (pseudonymised).

## Observability & Alerts
- Metrics:
  - `sorafs_pop_enrollments_total{status}`
  - `sorafs_pop_credentials_active`
  - `sorafs_pop_revocations_total{reason}`
  - `sorafs_pop_manifest_lag_seconds`
  - `sorafs_pop_proof_verification_total{result}`
- Alerts:
  - Manifest update lag > 24h.
  - High revocation spike.
  - Proof verification failure rate >1%.
- Logs: structured `pop_event`, `pop_revocation`, `pop_proof_request`.

## Security & Privacy
- Credentials never stored server-side; only commitments and hashed data.
- Revocation list hashed; public output does not reveal DID.
- Enrollment portal stores minimal PII; compliance with GDPR (erasure process).
- Credential packages encrypted per holder; CLI enforces passphrase strength.
- Issuer key managed via HSM with multi-signature approvals.

## Testing & Rollout
- Unit tests for credential serialization, revocation, proof verification.
- Integration tests using mock DID registry; ensure sortition accepts valid proofs and rejects revoked ones.
- Fuzz tests on revocation list parsing.
- Security testing: attempt replay of old proof, tampering with revocation data, forging credentials.
- Rollout:
  1. Implement issuer + registry; run staging with synthetic DIDs.
  2. Release CLI/SDK; test offline proof generation.
  3. Governance approves initial commitment root; publish to DAG.
  4. Onboard pilot jurors; monitor metrics.
  5. Full rollout before moderation panel launch.

## Implementation Checklist
- [x] Define credential and commitment schemas.
- [x] Specify enrollment, renewal, revocation workflows.
- [x] Document ZK proof generation/verification and API endpoints.
- [x] Capture storage, audit, observability, and security requirements.
- [x] Outline testing strategy and rollout plan.

With this specification, the Identity WG and governance platform can deploy the PoP credential pipeline that underpins juror sortition and other citizen-driven governance processes.
