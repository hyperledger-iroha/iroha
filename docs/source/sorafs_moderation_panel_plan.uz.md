---
lang: uz
direction: ltr
source: docs/source/sorafs_moderation_panel_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c09b8303ff3c6f45be06dfa07bed74cb535e09d5962fbb19f434cf73be75aa5f
source_last_modified: "2025-12-29T18:16:36.168535+00:00"
translation_last_reviewed: 2026-02-07
title: Moderation Appeals & Sortition Panels
summary: SFM-4b implementation status for appeal finance, policy-jury data foundations, and remaining moderation panel services.
---

---
title: Moderation Appeals & Sortition Panels
summary: SFM-4b implementation status for appeal finance, policy-jury data foundations, and remaining moderation panel services.
---

# Moderation Appeals & Sortition Panels

## Current Status

SFM-4b is partially implemented. The repository contains appeal finance helpers,
moderation reproducibility tooling, honey-audit gateway probes, and reusable
policy-jury sortition / commit-reveal data structures. It does not yet ship the
full moderation appeal service, SoraFS juror panel engine, secure evidence
viewer, voting orchestrator, or portal workflow described in the original plan.

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

Only steps that can be represented by the shipped helper crates and CLI commands
are available today.

## Data Boundaries

The shipped `PolicyJury*` types are reusable governance data structures, not a
complete SoraFS moderation-panel runtime. Before they can be used for moderation
appeals, the runtime needs SoraFS-specific wrappers that bind:

- appeal case identifiers;
- moderation policy references;
- proof-token and denylist evidence references;
- panel-size and quorum policy;
- settlement manifest version;
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
- Connect panel outcomes to gateway compliance caches, appeal finance
  settlement, transparency publication, and reputation scoring.
- Add end-to-end tests for appeal submission, juror selection, evidence access,
  commit/reveal voting, decision publication, and settlement.

## Validation

Focused checks for the currently shipped foundations are:

```sh
cargo test -p sorafs_orchestrator appeal
cargo test -p iroha_data_model policy_jury
```

Run the broader SoraFS moderation and gateway suites when the panel service
starts mutating runtime state.
