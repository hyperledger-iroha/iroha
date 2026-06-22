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
summary: SFM-4b4 implementation status for commit-reveal data foundations and remaining SoraFS juror voting service gates.
---

# Commit-Reveal Voting Service

## Current Status

SFM-4b4 has reusable commit/reveal and sortition data foundations in the
ministry policy-jury module. The repository does not yet ship the SoraFS
moderation voting contract, ballot orchestrator, juror CLI, challenge monitor,
or runtime service needed to run appeal-panel ballots end to end.

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
- Example policy-jury fixtures live in `docs/examples/ministry/`.

These types currently model policy-jury choices (`approve`, `reject`,
`abstain`). A SoraFS moderation ballot still needs a domain wrapper for
moderation outcomes such as `uphold`, `overturn`, `modify`, and `escalate`.

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

- Add SoraFS moderation ballot payloads that bind case ids, evidence bundle
  digests, appeal finance config versions, panel roster hashes, and moderation
  vote choices.
- Implement the ballot lifecycle store and orchestrator for announcements,
  commit windows, challenge buffers, reveal windows, tallying, retries, and
  contested outcomes.
- Implement the on-chain contract or ledger workflow that records commitments,
  reveals, challenges, outcomes, and juror penalties.
- Provide juror-facing CLI or portal commands for listing ballots, committing,
  revealing, challenging, and exporting audit evidence.
- Publish commit, reveal, challenge, and outcome events to the Governance DAG.
- Add end-to-end simulations with no-shows, duplicate commits, mismatched
  reveals, missed quorum, contested challenges, and successful decisions.

## Validation

The current foundation is covered by policy-jury data-model tests:

```sh
cargo test -p iroha_data_model policy_jury
```

Add a dedicated `sorafs` moderation voting test suite when the runtime service
lands. Until then, do not document `sorafs-juror` or SoraFS ballot service
commands as shipped.
