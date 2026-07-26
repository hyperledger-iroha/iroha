---
lang: pt
direction: ltr
source: docs/source/sorafs_commit_reveal_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 164ce0902aaa17a31659adb0bff4360e1abb8604cad5bed81c802c09aa0c8cec
source_last_modified: "2026-07-04T15:39:59.807879+00:00"
translation_last_reviewed: 2026-07-04
---

# Commit-Reveal Voting Service

## Current Status

SFM-4b4 has reusable commit/reveal and sortition data foundations,
SoraFS-specific ballot envelopes, and an authoritative native moderation
ledger. The native ledger persists policy snapshots, appeals, eligibility,
sortition, assignments, cases, commitments, reveals, payload-free challenges,
terminal outcomes, and classified no-show records through first-class ISIs,
typed events, and typed queries.

The obsolete process-local sorafs_node ballot runtime, checkpoint, mutation
API, event backlog, and caller-fed finalized projection have been deleted.
Torii's version-one moderation routes now accept exact signed native
instructions and return reconciled finalized-chain projections. The durable
orchestrator submits transactions and rebuilds its operational state from
committed events; it does not become a second source of truth.

The SFM-4b rollout evidence gate validates payload-free commit/reveal coverage,
digest bindings, negative scenarios, event lag, and absence of private ballot
payloads. That gate blocks promotion evidence; it does not replace the still
required deployed service, juror workflow, four-validator tests, security
review, and genuine production evidence.

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
- sorafs_node no longer owns ballot, appeal, scheduler, checkpoint, or event-stream authority.
  All case, assignment, commit, reveal, challenge, outcome, and no-show state is
  native consensus state; the node retains only screening, quarantine,
  evidence-viewer, and finalized-chain orchestration support.
- Torii retains the version-one moderation route family as an authenticated
  transport over the native ledger. Every mutation route accepts one exact
  signed native moderation instruction and submits it through strict durable
  transaction ingress. Every list, detail, no-show, and event response is
  rebuilt from a reconciled finalized-chain snapshot. No moderation transition
  or readback falls back to a process-local ballot database.
- iroha client and CLI moderation ballot commands construct or submit the same
  native signed transactions and consume finalized-chain readbacks; they do not
  mutate a node-owned ballot store.
- sorafs_manifest::SoraFsModerationBallotGovernanceEventV1 remains the canonical
  payload-free governance publication envelope. Producers must derive it from
  committed native events before sending it to the governance DAG and
  transparency adapters.
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

- Deploy and supervise the durable finalized-chain orchestrator, including
  transaction retries, reconciliation, scheduled no-show settlement handoff,
  notification delivery, challenge monitoring, and contested-outcome workflows.
- Audit the native CLI/client and executor workflows for juror-facing
  deployment, challenge evidence export, portal UX, and public runbooks.
- Connect committed moderation events and outcomes to public Governance DAG
  publication, gateway compliance, settlement, transparency, and reputation.
- Add reviewed four-peer deployed simulations with intake and PoP enrollment,
  restarts, operator retry/reconciliation, evidence access, commit/reveal,
  decision publication, and settlement. Native tests already cover no-shows,
  failover/exhaustion, wrong or rotated roots, nullifier replay,
  biased/duplicate rosters, duplicate commits, mismatched reveals, missed
  quorum, contested challenges, successful activation/decisions, and atomic
  rollback.
- Collect a passing payload-free commit_reveal canary through the SFM-4b rollout
  evidence gate after the deployed service has produced genuine evidence.

## Validation

The code foundation is covered by data-model, native execution, executor,
orchestrator, manifest, and Torii finalized-projection tests. Run:

    python3 scripts/check_sorafs_moderation_panel_rollout_evidence.py       @scripts/examples/sorafs_moderation_panel_rollout_evidence.args.example       --require-kind commit_reveal
    cargo test -p iroha_data_model policy_jury
    cargo test -p iroha_data_model sorafs_moderation_ballot
    cargo test -p iroha_data_model moderation_ledger
    cargo test -p iroha_data_model sorafs_decode_from_slice_roundtrips
    cargo test -p iroha_core sorafs_moderation --lib
    cargo test -p iroha_executor sorafs_permission_tests --lib
    cargo test -p sorafs_node moderation_orchestrator
    cargo test -p sorafs_manifest moderation_ballot_event
    cargo test -p iroha_torii moderation_ballot --features app_api
    cargo test -p iroha_torii moderation_ballot_no_show --features app_api -- --nocapture
    cargo test -p iroha_torii generated_spec_includes_documented_paths --features app_api

Keep native CLI and executor coverage in cargo test -p iroha_cli
moderation_ballots; add deployed service and end-to-end suites with genuine
runtime evidence. Until then, do not document sorafs-juror, portal-only
commands, or deployed ballot services as shipped.
