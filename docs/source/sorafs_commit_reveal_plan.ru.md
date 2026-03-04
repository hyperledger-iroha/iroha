---
lang: ru
direction: ltr
source: docs/source/sorafs_commit_reveal_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99453646976cda7047bcd6b3e84510751a5fd0af4cef2e59928f68661e9493c7
source_last_modified: "2026-01-03T18:08:01.383456+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Commit-Reveal Voting Service
summary: Final specification for SFM-4b4 covering ballots, juror clients, contract, monitoring, and rollout.
---

# Commit-Reveal Voting Service

## Goals & Scope
- Provide a sealed-ballot voting mechanism for moderation juror panels (and reuse for other governance flows) with deterministic timelines, verifiable commitments, and quorum enforcement.
- Supply juror client tooling, smart contract implementation, monitoring, and audit integration.

This specification completes **SFM-4b4 — Commit-reveal voting service**.

## Architecture Overview
| Component | Responsibility | Notes |
|-----------|----------------|-------|
| Voting contract (`CommitRevealContractV1`) | Stores commitments, enforces timelines, tallies weighted votes, triggers outcomes. | Deployed on governance chain (Iroha smart contract). |
| Voting orchestrator (`ballot_orchestrator`) | Manages ballot lifecycle, publishes announcements, closes windows, handles retries. | Reads config from governance. |
| Juror client (`sorafs-juror`) | CLI/SDK that fetches ballots, generates commits, reveals, and monitors deadlines. | Works offline until submission. |
| Challenge monitor (`challenge_daemon`) | Tracks commit integrity, open challenges, and auto-fail conditions. | Emits alerts. |
| Audit exporter | Writes ballot events to Governance DAG for replay and transparency. | Works with DAG publishing pipeline. |

## Ballot Lifecycle
1. **Announcement / Registration (default 12h)**  
   Governance publishes `BallotAnnouncementV1` (includes ballot ID, question, options, timeline, juror roster hash, quorum thresholds). Juror client fetches roster and validates PoP proof.
2. **Commit window (default 24h)**  
   Jurors submit `CommitEnvelopeV1`:
   ```norito
   struct CommitEnvelopeV1 {
       ballot_id: Uuid,
       juror_commitment_id: Digest32, // hashed DID + nonce
       commitment_hash: Digest32,     // H(choice || salt || ballot_id)
       weight: Decimal64,
       timestamp: Timestamp,
       signature: Signature,
   }
   ```
   Contract records commitment, ensuring latest commit per juror. Events logged.
3. **Challenge buffer (default 2h)**  
   No new commits/reveals. Jurors can submit `CommitChallengeV1` if they suspect duplicates or roster tampering. Unresolved challenges at buffer end → ballot auto-fails (`Outcome=Contested`).
4. **Reveal window (default 18h)**  
   Jurors submit `RevealEnvelopeV1`:
   ```norito
   struct RevealEnvelopeV1 {
       ballot_id: Uuid,
       juror_commitment_id: Digest32,
       choice: ModerationVoteV1,  // approve | overturn | modify | abstain
       salt: Digest32,
       justification: Option<String>,
       timestamp: Timestamp,
       signature: Signature,
   }
   ```
   Contract verifies commitment hash matches and updates tally.
5. **Tally & Outcome**  
   Once reveal window closes (or all jurors revealed), contract checks quorum and tallies outcomes. Emits `BallotOutcomeV1` with final vote distribution, participation, and flags.

Customizable durations via governance config: `registration_hours`, `commit_hours`, `challenge_hours`, `reveal_hours`, `quorum_participation`, `approval_threshold`.

## Quorum & Thresholds
- Participation quorum default 67% of stake-weighted jurors (commit + reveal). Configurable.
- Approval threshold default 60% for `approve` votes among reveals.
- Abstain counts toward participation but not approval.
- Auto-fail conditions: unresolved challenges, insufficient participation, governance epoch rollover, contract errors.
- Juror slashing: failing to reveal after committing triggers penalty (slashing bond, log event).

## Smart Contract Details
- State:
  - `Ballot { ballot_id, status, config, announcements, roster_hash, commits, reveals, challenges }`
  - `CommitRecord { commitment_hash, weight, timestamp }`
  - `RevealRecord { choice, justification, weight, timestamp }`
  - `ChallengeRecord { juror_commitment_id, reason, status }`
- Functions:
  - `announce_ballot`, `submit_commit`, `submit_challenge`, `submit_reveal`, `finalize_ballot`, `retry_ballot`.
- Access control: Only governance orchestrator can announce/finalize; jurors identified via PoP proof.
- Events: `CommitAccepted`, `CommitChallenged`, `RevealAccepted`, `BallotOutcome`, `BallotFailed`.

## Juror Client
- CLI commands:
  - `sorafs-juror ballots` – list active ballots.
  - `sorafs-juror commit --ballot <id> --choice <opt>` generates salt, commitment, submits.
  - `sorafs-juror reveal --ballot <id>` reveals stored choice/salt.
  - `sorafs-juror status --ballot <id>` monitor deadlines, participation.
  - `sorafs-juror challenge --ballot <id> --reason <text>`.
- Client stores commits locally encrypted; supports offline commit generation (submit later).
- Provides reminders/notifications (e.g., `notify-send`) before windows close.
- Integration with PoP proofs using `sorafs pop prove`.

## Monitoring & Transparency
- Metrics:
  - `sorafs_commit_commits_total{ballot_id}`
  - `sorafs_commit_reveals_total{ballot_id}`
  - `sorafs_commit_participation_ratio{ballot_id}`
  - `sorafs_commit_challenges_total`
  - `sorafs_commit_late_reveals_total`
- Alerts:
  - Participation < 50% halfway through reveal window.
  - Challenge pending at buffer end.
  - Contract event gap > 2 min.
- DAG publishing:
  - Commit/reveal/challenge/outcome events appended to Governance DAG with pseudonymised juror IDs.
  - CLI `sorafs-juror export --ballot <id>` fetches DAG entries for auditing.

## Security
- Commits hashed with BLAKE3; salts 256-bit random.
- Juror commitments pseudonymised (hash of DID + nonce) to preserve privacy; governance can map via secure service.
- Challenge submissions require signed justification; false challenge rate monitored.
- Contract guarded by governance: ability to pause/resume ballots.

## Testing & Rollout
- Unit tests for contract functions and vote tallying.
- Property tests ensuring commitments cannot be tampered with, reveals match.
- Integration tests running full ballot simulation with CLI in CI.
- Chaos tests: drop network events, ensure orchestrator retries/reschedules.
- Rollout plan:
  1. Implement contract + CLI; deploy on testnet.
  2. Conduct dry-run ballots with pilot jurors.
  3. Governance approves configuration; publish to DAG.
  4. Production launch in parallel with moderation panel rollout.
  5. Monitor first ballots, adjust thresholds if needed.

## Implementation Checklist
- [x] Define contract state, functions, and events.
- [x] Specify ballot timeline, quorum rules, and auto-fail conditions.
- [x] Document juror client commands and integration with PoP proofs.
- [x] Capture monitoring, alerts, and DAG publication.
- [x] Outline security considerations, testing, and rollout.

The commit-reveal specification now equips governance and moderation teams with the blueprint to implement sealed juror ballots reliably and auditable within the SoraFS ecosystem.
