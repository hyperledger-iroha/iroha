---
lang: ka
direction: ltr
source: docs/source/sorafs_commit_reveal_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99453646976cda7047bcd6b3e84510751a5fd0af4cef2e59928f68661e9493c7
source_last_modified: "2025-12-29T18:16:36.137578+00:00"
translation_last_reviewed: 2026-02-07
title: Commit-Reveal Voting Service
summary: SFM-4b4 implementation status for commit-reveal data foundations and remaining SoraFS juror voting service gates.
---

# Commit-Reveal Voting Service

## Current Status

SFM-4b4 has reusable commit/reveal and sortition data foundations in the
ministry policy-jury module plus SoraFS-specific moderation ballot
commit/reveal payloads in the SoraFS data model. The repository does not yet
ship the SoraFS moderation voting contract, ballot orchestrator, juror CLI,
challenge monitor, or runtime service needed to run appeal-panel ballots end to
end.

## Shipped Foundations

- `iroha_data_model::ministry::jury::PolicyJuryBallotCommitV1` records a sealed
  juror commitment for a proposal and round.
- `PolicyJuryBallotRevealV1` computes the canonical commitment digest from
  round id, proposal id, juror id, vote choice, and nonce.
- `PolicyJuryBallotCommitV1::verify_reveal` validates version, proposal, round,
  juror, nonce length, ballot mode, and commitment digest before accepting a
  reveal.
- `PolicyJurySortitionV1` validates committee size, duplicate slots, duplicate
  jurors, ordered waitlists, and failover rank references.
- `iroha_data_model::sorafs::moderation::SoraFsModerationBallotContextV1`
  binds case id, evidence bundle digest, appeal finance config version, panel
  roster hash, policy reference, and optional evidence URI for SoraFS cases.
- `SoraFsModerationBallotCommitV1` and `SoraFsModerationBallotRevealV1` bind
  the context to `uphold`, `overturn`, `modify`, and `escalate` vote choices,
  reject blank case/policy/finance fields, reject zero evidence/roster hashes,
  and verify the reveal nonce and commitment digest.
- `sorafs_manifest::SoraFsModerationBallotGovernanceEventV1` and
  `sorafs_node::FilesystemGovernancePublisher` publish local announcement,
  commit-accepted, reveal-accepted, and tally events into the local
  `publish-index.json`, CAR queue, and optional signed runtime DAG when a
  governance publisher is configured.
- Example policy-jury fixtures live in `docs/examples/ministry/`.

These types currently model policy-jury choices (`approve`, `reject`,
`abstain`). SoraFS moderation cases should use the SoraFS wrappers when binding
appeal evidence, panel rosters, and finance config versions to moderation
outcomes.

## Target Ballot Lifecycle

The production service still targets this flow:

1. The moderation panel service announces a ballot with case id, policy
   reference, panel roster hash, quorum rules, and commit/reveal deadlines.
2. Jurors submit signed commitment envelopes during the sealed phase.
3. A challenge buffer allows roster or duplicate-commitment disputes before
   reveals open.
4. Jurors reveal choices and salts during the reveal phase.
5. The service verifies each reveal against the stored commitment, tallies the
   outcome, applies quorum rules, and emits a signed decision event.
6. The decision feeds compliance caches, appeal settlement, transparency
   publication, and any reputation penalties.

## Remaining Production Gates

- Implement the ballot lifecycle store and orchestrator for announcements,
  commit windows, challenge buffers, reveal windows, tallying, retries, and
  contested outcomes.
- Implement the on-chain contract or ledger workflow that records commitments,
  reveals, challenges, outcomes, and juror penalties.
- Provide juror-facing CLI or portal commands for listing ballots, committing,
  revealing, challenging, and exporting audit evidence.
- Extend Governance DAG publication beyond local lifecycle events to durable
  challenge/dispute records, contract-backed decisions, and public IPFS/IPNS
  rollout evidence.
- Add end-to-end simulations with no-shows, duplicate commits, mismatched
  reveals, missed quorum, contested challenges, and successful decisions.

## Validation

The current foundation is covered by policy-jury data-model tests:

```sh
cargo test -p iroha_data_model policy_jury
cargo test -p iroha_data_model sorafs_moderation_ballot
```

Add a dedicated `sorafs` moderation voting test suite when the runtime service
lands. Until then, do not document `sorafs-juror` or SoraFS ballot service
commands as shipped.
