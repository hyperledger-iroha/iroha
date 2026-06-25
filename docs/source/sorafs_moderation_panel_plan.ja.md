---
lang: ja
direction: ltr
source: docs/source/sorafs_moderation_panel_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b6f5f6a4fad906e35d140daa391deba56cece043d34cc762d3f24df7f3b7d7bc
source_last_modified: 2026-06-24T08:22:33.535741Z
translation_last_reviewed: 2026-01-30
---

# Moderation Appeals & Sortition Panels

## Current Status

SFM-4b is partially implemented. The repository contains appeal finance helpers,
moderation reproducibility tooling, honey-audit gateway probes, and reusable
policy-jury sortition / commit-reveal data structures plus SoraFS-specific
moderation ballot wrappers and a local `sorafs_node` ballot lifecycle runtime
exposed through Torii JSON endpoints and the local Governance DAG publisher.
The local ballot list readback now keeps the full local total visible while
bounding the returned `ballots` array through a `limit` query parameter
(default 50, max 500), matching the production dashboard-readiness pattern used
by adjacent SoraFS event/readback APIs.
It does not yet ship the full moderation appeal service, SoraFS juror panel
engine, secure evidence viewer, durable voting orchestrator, or portal workflow
described in the original plan.

## Shipped Foundations

- `crates/sorafs_orchestrator/src/appeals.rs` implements deterministic appeal
  quote, settlement, and disbursement calculations.
- `sorafs_cli appeal quote`, `sorafs_cli appeal settle`, and
  `sorafs_cli appeal disburse` expose those calculations for operators and
  treasury evidence bundles.
- `iroha_data_model::ministry::jury` provides
  `PolicyJurySortitionV1`, `PolicyJuryBallotCommitV1`, and
  `PolicyJuryBallotRevealV1` for deterministic policy-jury draws and sealed
  commit/reveal payload validation.
- `iroha_data_model::sorafs::moderation` provides
  `SoraFsModerationBallotContextV1`,
  `SoraFsModerationBallotCommitV1`,
  `SoraFsModerationBallotRevealV1`, and `SoraFsModerationVoteChoice` so SoraFS
  cases can bind case ids, evidence bundle digests, appeal finance versions,
  panel roster hashes, policy references, and `uphold`/`overturn`/`modify`/
  `escalate` choices before a runtime service exists.
- `sorafs_node::ModerationBallotRuntime` is wired through `NodeHandle` as a
  deterministic local lifecycle store for ballot announcement, eligible-juror
  commit acceptance, challenge-buffered reveal acceptance, quorum tallying,
  contested tie detection, and replayable/broadcast local events.
- Torii exposes local moderation ballot announcement, list/get, commit, reveal,
  tally, and event backlog endpoints under
  `/v1/sorafs/moderation/ballots*`. The list endpoint reports full local
  ballot totals while bounding returned ballot records with `limit` (default
  50, max 500). Mutating requests require canonical app authentication, and
  commit/reveal requests bind the signer to the canonical juror id.
  Deposit-backed announcements persist the confirmed native asset-lock
  fingerprint, including the evidence hashes used to derive the escrow id, and
  Torii's configured-signer moderation settlement worker replays and subscribes
  to tallied local ballot events to queue retry-aware native settlement steps
  from that fingerprint.
- `sorafs_manifest::SoraFsModerationBallotGovernanceEventV1` and
  `sorafs_node::FilesystemGovernancePublisher` publish local moderation ballot
  announcement, commit-accepted, reveal-accepted, and tally evidence into the
  local Governance DAG `publish-index.json`, CAR queue, and optional signed
  runtime DAG.
- `docs/examples/ministry/policy_jury_roster_example.json` and
  `docs/examples/ministry/policy_jury_sortition_example.json` provide example
  inputs for the reusable policy-jury data path.
- `sorafs_cli moderation validate-repro`,
  `sorafs_cli moderation validate-corpus`, and
  `sorafs_cli moderation honey-audit` support moderation reproducibility and
  gateway enforcement evidence.

## Intended Panel Lifecycle

The production service still targets this lifecycle:

1. Appeal intake validates the appellant, related moderation proof tokens,
   evidence references, deposit quote, and policy reference.
2. Sortition selects a moderation panel from a proof-of-personhood snapshot and
   records the roster hash plus failover plan.
3. Jurors review evidence through an attested viewer with per-session access
   logging.
4. Jurors submit sealed ballot commitments, reveal votes during the reveal
   window, and satisfy quorum rules.
5. The decision updates the moderation cache, appeal finance settlement,
   transparency ledger, and any provider reputation penalties.

Only steps that can be represented by the shipped helper crates, CLI commands,
local `sorafs_node` runtime, and local Torii moderation ballot API are
available today.

## Data Boundaries

The shipped `PolicyJury*` types are reusable governance data structures, not a
complete SoraFS moderation-panel runtime. SoraFS-specific ballot wrappers now
bind:

- appeal case identifiers;
- moderation policy references;
- proof-token and denylist evidence references;
- panel roster hashes;
- settlement manifest version;
- moderation vote choices;

The local runtime and Torii API now bind panel-size/quorum policy,
commit/reveal windows, and eligible jurors for deterministic node-local tests.
The production service still needs durable state that binds:

- evidence access attestation;
- decision publication and appeal cache updates.

Do not document `sorafs moderation jury-accept`,
`sorafs moderation open-case`, or similar portal commands as shipped until the
corresponding service and CLI handlers exist.

## Remaining Production Gates

- Implement the moderation appeal intake API and persisted case lifecycle state.
- Adapt policy-jury sortition to SoraFS moderation cases, PoP snapshots, juror
  eligibility, no-show failover, and roster privacy requirements.
- Ship the secure evidence viewer and audit logger described by
  `docs/source/sorafs_evidence_viewer_plan.md`.
- Ship the SoraFS commit-reveal voting service described by
  `docs/source/sorafs_commit_reveal_plan.md`.
- Connect panel outcomes to gateway compliance caches, transparency
  publication, settlement reconciliation, and reputation scoring.
- Promote local Governance DAG moderation event publication into the durable
  contract-backed and public IPFS/IPNS decision trail.
- Add end-to-end tests for appeal submission, juror selection, evidence access,
  commit/reveal voting, decision publication, and settlement.

## Validation

Focused checks for the currently shipped foundations are:

```sh
cargo test -p sorafs_orchestrator appeal
cargo test -p iroha_data_model policy_jury
cargo test -p iroha_data_model sorafs_moderation_ballot
cargo test -p sorafs_node moderation_ballot
cargo test -p sorafs_manifest moderation_ballot_event
cargo test -p iroha_torii moderation_ballot --features app_api
cargo test -p iroha_torii moderation_ballot_list_limit --features app_api
cargo test -p iroha_torii generated_spec_includes_documented_paths --features app_api
```

Run the broader SoraFS moderation and gateway suites when the panel service
starts mutating runtime state.
