---
title: Commit-Reveal Voting Service
summary: SFM-4b4 implementation status for commit-reveal data foundations and remaining SoraFS juror voting service gates.
---

# Commit-Reveal Voting Service

## Current Status

SFM-4b4 has reusable commit/reveal and sortition data foundations in the
ministry policy-jury module plus SoraFS-specific moderation ballot
commit/reveal payloads in the SoraFS data model and a local `sorafs_node`
ballot lifecycle runtime exposed through local Torii JSON endpoints. Accepted
local ballot lifecycle events can also be materialized into the SoraFS
Governance DAG filesystem publisher and optional signed runtime DAG. The
repository does not yet ship the SoraFS moderation voting contract, durable
ballot orchestrator, juror CLI, challenge monitor, or production service needed
to run appeal-panel ballots end to end. The shared SFM-4b moderation-panel
rollout evidence gate now validates a dedicated
`sorafs.moderation_panel.commit_reveal_canary.v1` artifact for this boundary,
including authenticated commit/reveal routes, digest recomputation, duplicate
commit rejection, mismatched reveal rejection, late commit/reveal rejection,
missed-quorum detection, no-show failover, juror penalty planning,
deterministic tally replay, contested challenge coverage, governance event
digest binding, event-lag limits, and absence of raw commit/reveal payloads.
That gate blocks deployed promotion evidence; it does not replace the missing
durable service or contract-backed workflow.

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
- `sorafs_node::ModerationBallotRuntime`, exposed through `NodeHandle`, accepts
  ballot announcements, juror commitments, challenge-buffered reveals, and
  deterministic quorum tallies with contested tie detection plus replayable
  local events.
- Torii exposes local moderation ballot endpoints for announcement, list/get,
  commit, reveal, tally, and event backlog under
  `/v1/sorafs/moderation/ballots*`. Mutating requests require canonical app
  authentication, announcement requests require a `deposit_confirmation` object
  that Torii confirms against the runtime native asset-lock ledger before local
  ballot admission, and commit/reveal requests require the authenticated account
  to match the canonical juror id in the payload. Ballot list/detail readbacks
  bound embedded commit and reveal arrays with `limit` (default 50, max 500)
  while preserving full counts and truncation metadata.
- `sorafs_manifest::SoraFsModerationBallotGovernanceEventV1` and
  `sorafs_node::FilesystemGovernancePublisher` publish local announcement,
  commit-accepted, reveal-accepted, and tally events into the local
  `publish-index.json`, CAR queue, and optional signed runtime DAG when a
  governance publisher is configured.
- `scripts/check_sorafs_moderation_panel_rollout_evidence.py` validates
  payload-free commit/reveal rollout evidence and rejects canaries that omit
  duplicate-commit, mismatched-reveal, late-submission, missed-quorum,
  no-show-failover, contested-challenge, deterministic-replay, or governance
  digest-binding coverage.
- Example policy-jury fixtures live in `docs/examples/ministry/`.

These types currently model policy-jury choices (`approve`, `reject`,
`abstain`). SoraFS moderation cases should use the SoraFS wrappers when binding
appeal evidence, panel rosters, and finance config versions to moderation
outcomes.

## Target Ballot Lifecycle

The production service still targets this flow:

1. The moderation panel service announces a ballot with case id, confirmed
   appeal-deposit custody, policy reference, panel roster hash, quorum rules,
   and commit/reveal deadlines.
2. Jurors submit signed commitment envelopes during the sealed phase.
3. A challenge buffer allows roster or duplicate-commitment disputes before
   reveals open.
4. Jurors reveal choices and salts during the reveal phase.
5. The service verifies each reveal against the stored commitment, tallies the
   outcome, applies quorum rules, and emits a signed decision event.
6. The decision feeds compliance caches, appeal settlement, transparency
   publication, and any reputation penalties.

## Remaining Production Gates

- Persist or contract-back the local ballot lifecycle store and add the
  production orchestrator for retries, no-show handling, challenge disputes, and
  durable contested-outcome workflows.
- Implement the on-chain contract or ledger workflow that records commitments,
  reveals, challenges, outcomes, and juror penalties.
- Provide juror-facing CLI or portal commands for listing ballots, committing,
  revealing, challenging, and exporting audit evidence through the Torii API.
- Extend Governance DAG publication beyond local lifecycle events to durable
  challenge/dispute records, contract-backed decisions, and public IPFS/IPNS
  rollout evidence.
- Add end-to-end simulations with no-shows, duplicate commits, mismatched
  reveals, missed quorum, contested challenges, and successful decisions.
- Collect a passing payload-free `commit_reveal` canary through the SFM-4b
  rollout evidence gate after the durable service exists.

## Validation

The current foundation is covered by policy-jury data-model tests:

```sh
python3 scripts/check_sorafs_moderation_panel_rollout_evidence.py \
  @scripts/examples/sorafs_moderation_panel_rollout_evidence.args.example \
  --require-kind commit_reveal
cargo test -p iroha_data_model policy_jury
cargo test -p iroha_data_model sorafs_moderation_ballot
cargo test -p sorafs_node moderation_ballot
cargo test -p sorafs_manifest moderation_ballot_event
cargo test -p iroha_torii moderation_ballot --features app_api
cargo test -p iroha_torii generated_spec_includes_documented_paths --features app_api
```

Add CLI and end-to-end `sorafs` moderation voting suites when the production
service lands. Until then, do not document `sorafs-juror` or SoraFS ballot
service commands as shipped.
