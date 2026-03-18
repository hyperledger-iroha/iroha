---
lang: am
direction: ltr
source: docs/source/sorafs_moderation_panel_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c09b8303ff3c6f45be06dfa07bed74cb535e09d5962fbb19f434cf73be75aa5f
source_last_modified: "2025-12-29T18:16:36.168535+00:00"
translation_last_reviewed: 2026-02-07
title: Moderation Appeals & Sortition Panels
summary: Final specification for SFM-4b covering appeal staking, juror selection, evidence handling, voting, transparency, and rollout.
---

# Moderation Appeals & Sortition Panels

## Goals & Scope
- Provide a governance-backed moderation system that allows users/operators to appeal compliance actions, with citizen panels selected via verifiable sortition.
- Implement dynamic appeal staking, secure evidence handling, commit-reveal voting, and immutable receipts.
- Integrate with gateway compliance (SFM-4) and transparency dashboards while preserving privacy and legal requirements.

This specification completes **SFM-4b — Moderation appeal & sortition panel tooling**.

## Architecture Overview
| Component | Responsibility | Notes |
|-----------|----------------|-------|
| Appeal service (`moderation_appeals`) | Accepts appeals, calculates staking deposits, tracks case lifecycle. | REST/gRPC API, Norito payloads for cases. |
| Sortition engine (`jury_sorter`) | Builds juror pools using proof-of-personhood credentials and ZK membership proofs; selects panels deterministically. | Runs atop governance identity service. |
| Evidence viewer (`evidence_portal`) | Secure, watermarking-enabled viewer for jurors; enforces attestation. | Browser app + streaming backend. |
| Voting backend (`jury_vote`) | Commit-reveal voting with sealed ballots and quorum enforcement; stores votes in governance ledger. | Uses Norito receipts. |
| Transparency pipeline | Generates weekly ledgers showing appeals, outcomes, deposits, and overrides. | Publishes to Governance DAG + dashboards. |

### Case Lifecycle
1. **Appeal submission**: Appellant submits `ModerationAppealV1` with stake deposit; gateway compliance attaches proof tokens/evidence references.
2. **Validation**: Appeal service verifies eligibility, deposit, and policy references; assigns `case_id`.
3. **Panel selection**: Sortition engine selects jurors based on `ModerationCaseProfileV1` (governance-defined). Jurors notified via Torii and email.
4. **Evidence review**: Jurors access evidence through secure viewer; interactions logged.
5. **Deliberation & voting**: Jurors submit sealed commits, later reveal votes; quorum checked.
6. **Outcome publication**: Decision recorded as `ModerationDecisionV1`; refunds/penalties processed; transparency ledger updated.
7. **Override/Escalation**: Council/executive may override with `ModerationOverrideV1`; logged with justification.

## Data Model
- **Appeal submission**:
  ```norito
  struct ModerationAppealV1 {
      version: u8,
      case_id: Uuid,
      appellant: AccountId,
      related_tokens: Vec<String>,     // proof token IDs
      policy_reference: String,        // URN
      case_class: ModerationCaseClass, // takedown | access_suspension | compliance_flag | other
      description: String,
      stake_amount_xor: Decimal64,
      submitted_at: Timestamp,
      attachments: Vec<EvidenceReferenceV1>, // pointer to evidence storage
      signature: Signature,
  }
  ```
- **Case profile** (governance-defined):
  ```norito
  struct ModerationCaseProfileV1 {
      case_class: ModerationCaseClass,
      min_panel_size: u8,
      max_panel_size: u8,
      quorum: u8,
      stake_formula: StakeFormulaV1,
      eligibility_pool: String,  // reference to PoP group
      decision_deadline_hours: u16,
      escalation_policy: EscalationPolicyV1,
  }
  ```
- **Juror credentials**:
  - Proof-of-personhood credential `PopCredentialV1` (issued via SFM-4b1).
  - ZK membership proof `JurorProofV1` verifying credential without revealing identity.
- **Vote process**:
  - `VoteCommitV1 { case_id, juror_id_hash, commitment_hash, salt, signature }`
  - `VoteRevealV1 { case_id, juror_id_hash, vote: ModerationVote, salt, justification }`
  - Votes tallied after reveal phase; failure to reveal triggers slashing of juror bond.
- **Decision**:
  ```norito
  struct ModerationDecisionV1 {
      case_id: Uuid,
      outcome: ModerationOutcome, // uphold | overturn | modify | escalate
      votes_for: u8,
      votes_against: u8,
      abstained: u8,
      decided_at: Timestamp,
      panel_members: Vec<JurorSummaryV1>,
      notes: Option<String>,
      signature: Signature,
  }
  ```
- **Receipts & transparency**:
  - `ModerationReceiptV1` published per case summarizing decision, stake disposition, and Merkle proofs.
  - Weekly `ModerationLedgerV1` listing cases, outcomes, appeals pending, overrides.

## Appeal Staking & Economics
- Stake deposit formula:
  ```
  deposit = base_rate[class] * congestion_factor(backlog) * size_multiplier(content_size)
  ```
  - `base_rate` defined per class (e.g., takedown 100 XOR, access suspension 200 XOR).
  - `congestion_factor = 1 + 0.05 * max(0, backlog - target_backlog)` (clamped).
  - `size_multiplier` scales with evidence size (1.0–2.0).
- Deposits locked in escrow; refunded if appeal upheld / partially upheld.
- Penalties:
  - Frivolous appeals (score < 0.2) forfeit 50% deposit.
  - No-show jurors lose juror bond (see below).
- Governance config stored in `moderation_economics.toml`.

## Sortition & Juror Management
- Juror pool derived from PoP credentials with optional weighting (stake, past reliability).
- Sortition algorithm:
  - Uses VRF seeded with case_id + governance randomness to select jurors.
  - Generates `SortitionProofV1` containing VRF output, membership proof, and panel roster hash.
- Juror obligations:
  - Accept/decline participation within 12h. Decline allowed twice per quarter before bond penalty.
  - Juror bond (e.g., 50 XOR) locked during case; forfeited for no-show or misconduct.
- Transparency:
  - Panel roster hashed (pseudonymized) and published; actual identities remain confidential.
- Tools:
  - CLI `sorafs moderation jury-accept --case <id>` etc.
  - Portal displays schedule, evidence, vote deadlines.

## Evidence Handling
- Evidence stored via secure viewer:
  - Encrypted at rest, streamed with DRM/watermark overlay containing juror pseudonym, timestamp.
  - Attestation via WebAuthn; session tokens expire after 15 minutes.
  - Downloads disabled; copy events logged.
- Evidence references include `EvidenceReferenceV1 { storage_uri, hash, content_type, size_bytes }`.
- Access logs appended as `EvidenceAuditEventV1`.
- Erasure workflow ensures evidence removed when mandated, with signed receipt.

## Voting Workflow
- Phases:
  1. `Review` – jurors access evidence.
  2. `Commit` (12h window) – jurors submit hashed vote.
  3. `Reveal` (12h window) – reveal actual vote.
  4. `Tally` – system counts votes, checks quorum.
- If quorum not met, case escalates to next-tier panel or governance council.
- Vote options: `uphold`, `overturn`, `modify`, `abstain`.
- Votes and decisions recorded in Governance DAG for audit.
- Decisions auto-notify stakeholders (appellant, operator, compliance team).

## Integration Points
- Gateway compliance: appeals link to `Sora-Moderation-Token` proof; decision updates compliance cache to unblock content or confirm block.
- Transparency dashboard: weekly ledger consumes decisions, deposit stats, juror metrics.
- Reputation engine: providers with repeated upheld appeals (blocking reversed) may incur compliance penalties.
- AI pre-screen: escalated items feed more training data for models (with privacy guard).

## Observability & Alerts
- Metrics:
  - `sorafs_moderation_appeals_total{status}`
  - `sorafs_moderation_backlog_count{class}`
  - `sorafs_moderation_sortition_duration_seconds`
  - `sorafs_moderation_vote_participation_ratio`
  - `sorafs_moderation_stake_locked_xor`
  - `sorafs_moderation_evidence_access_total{action}`
- Logs: `moderation_case_event`, `juror_action`, `vote_commit`, `vote_reveal`.
- Alerts:
  - Appeal backlog exceeds threshold.
  - Sortition failure or missing VRF proofs.
  - Juror participation < quorum risk.
  - Evidence viewer attestation failures.
  - Override occurrences (flagged for council review).

## Security & Privacy
- Appeal submissions signed; anti-spam captcha / rate limit per account.
- Juror identities pseudonymised; real identity accessible to governance under sealed procedure.
- Evidence viewer uses short-lived URLs, watermarks, and tamper detection.
- Vote secrecy ensured by commit-reveal; double voting prevented by case-specific salts.
- Audit logs hashed daily; storage encryption uses per-case keys.
- Compliance with GDPR: retention policies, erasure receipts, pseudonymisation.

## Testing & Rollout
- Unit tests for staking formula, sortition proofs, vote tallying.
- Integration tests: simulate end-to-end case (submission, sortition, vote, decision).
- Security tests: attempt replay attacks on votes, tamper with evidence tokens, ensure detection.
- UX tests with juror sample group; confirm accessibility.
- Rollout plan:
  1. Implement services and run closed alpha with test cases.
  2. Governance approves case profiles and economics config.
  3. Staging launch with limited class (e.g., content takedown).
  4. Collect feedback, adjust thresholds.
  5. Production launch for core classes; expand to other classes after 2 successful cycles.

## Documentation & Tooling
- Operator guides (`docs/source/sorafs/moderation_operator.md`).
- Juror handbook (portal page) explaining responsibilities, UI walkthrough.
- Transparency report integration updated to include appeal outcomes.
- CLI docs (`docs/source/sorafs_cli.md`) updated with moderation commands.

## Implementation Checklist
- [x] Define appeal schemas, staking formulas, and governance config.
- [x] Specify sortition process with PoP credentials and ZK proofs.
- [x] Document evidence handling safeguards and audit trails.
- [x] Detail commit-reveal voting, quorum rules, and decision outputs.
- [x] Capture integration points, observability, security, and privacy requirements.
- [x] Outline testing plan and staged rollout.

With this specification, moderation, governance, and operations teams have a clear blueprint to implement citizen-panel appeals that balance transparency, reproducibility, and privacy.
