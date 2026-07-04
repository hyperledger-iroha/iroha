---
lang: pt
direction: ltr
source: docs/source/sorafs_moderation_panel_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9c2a51e7f8bbb2aad60248af4013e91bc9f935e35d37580567dcc5c22a371fbe
source_last_modified: "2026-07-04T15:05:52.499061+00:00"
translation_last_reviewed: 2026-07-04
source_mtime: "2026-07-04T06:07:00.822592+00:00"
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
`scripts/check_sorafs_moderation_panel_rollout_evidence.py` now provides the
fail-closed SFM-4b rollout evidence gate for deployed moderation-panel
promotion packets, and
`scripts/run_sorafs_moderation_panel_rollout_evidence.py` provides the matching
reviewed evidence collection planner/runner. The checker exports its
required top-level payload fields as `EVIDENCE_REQUIRED_FIELDS`, and the runner
dry-run emits the checker-backed `evidence_contract` map listing each selected
evidence kind's schema and required payload fields.
Every recognized rollout artifact must also carry reviewed `deployment_id` and
`environment` context, so staging or production evidence cannot be satisfied by
local, mock, dev, or otherwise unreviewed deployment packets. The gate also
blocks mixed reviewed deployment contexts across the same rollout bundle.
The gate also rejects mixed promotion packets: sortition, viewer, operator,
notification, voting, publication, settlement, transparency/reputation,
metrics, end-to-end, and governance artifacts must bind back to the appeal
intake `case_digest_hex`; viewer/operator/notification/voting and downstream
artifacts must bind back to a case-bound sortition `roster_hash_hex`; and
publication, settlement, transparency/reputation, metrics, end-to-end, and
governance artifacts must bind back to a roster-bound commit/reveal
`tally_digest_hex`.
End-to-end panel artifacts also publish `policy_digest_hex`; governance
approval artifacts must bind `policy_digest_hex` to that valid end-to-end
policy digest, and the checker emits those valid panel policy digests as
`valid_policy_digests`.
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
- Torii exposes local moderation ballot announcement, list/get, no-show plan
  readback, commit, challenge submission/resolution, reveal, tally, and event
  backlog endpoints under `/v1/sorafs/moderation/ballots*`. The list endpoint
  reports full local
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
commit/reveal windows, eligible jurors, and the payload-free no-show penalty
plan readback route for deterministic node-local tests.
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
- Capture reviewed, payload-free deployed evidence for appeal intake, sortition
  roster, evidence viewer, operator workflow, juror notification, commit/reveal,
  decision publication, settlement integration, transparency/reputation handoff,
  panel metrics, end-to-end panel simulation, and governance approval that
  passes the SFM-4b rollout evidence gate.

## Rollout Evidence Gate

Use the rollout gate after the deployed moderation appeal service, panel
sortition, evidence viewer, operator workflow, juror notification transport,
commit/reveal voting path, decision publication, settlement handoff,
transparency/reputation handoff, metrics, end-to-end panel simulation, and
governance packet have produced reviewed, payload-free JSON evidence:

```sh
python3 scripts/check_sorafs_moderation_panel_rollout_evidence.py \
  @scripts/examples/sorafs_moderation_panel_rollout_evidence.args.example
```

For staged collections with reviewed evidence paths, prefer the planner so the
verifier command and summary path are reproducible:

```sh
python3 scripts/run_sorafs_moderation_panel_rollout_evidence.py \
  @scripts/examples/sorafs_moderation_panel_rollout_collection.args.example \
  --dry-run
```

When operators have reviewed the deployed production facts, use
`scripts/build_sorafs_moderation_panel_canary.py` as the payload-free SFM-4b moderation panel canary builder
for the individual evidence artifacts consumed by the gate. The builder covers
every current parent-gate evidence kind, requires explicit `--verified-claim`
input for positive safety claims, complete intake/operator/ballot/decision
route, viewer role/security/event/export, commit-reveal scenario, publication
target, outcome, and metric coverage where applicable, shared
case/roster/tally digest bindings, explicit `--route-body-blake3-hex` evidence
for route responses, and threshold-bounded route, event-lag, viewer-URL,
panel-size, peer-count, validator-count, reviewed peer/validator labels. It rejects duplicate or unknown `--verified-claim`, route,
viewer-role, viewer-security-control, viewer-event-kind, viewer-export-target,
scenario, outcome, publication-target, and metric inputs before any canary JSON
is written. It also requires reviewed appeal-intake `--case` labels whose unique
inventory matches `--case-count`, reviewed sortition-roster `--roster-juror`
labels whose unique inventory matches `--panel-size`, reviewed evidence-viewer
reviewed `moderation-viewer-session-*` `--viewer-session` labels whose unique
inventory matches `--session-count` and rejects non-production markers,
reviewed juror-notification
`--notification` and `--juror` labels whose unique inventories match the
notification and juror counts, reviewed commit/reveal `--commit` and `--reveal`
labels whose unique inventories match the commit and reveal counts, and
reviewed settlement-integration `--settlement` labels whose unique inventory
matches `--settlement-count`, reviewed end-to-end `--panel-case` labels whose
unique inventory matches `--case-count`, and reviewed end-to-end/governance
policy-digest facts. It also derives the reviewed evidence-viewer role, security-control,
access-event-kind, and export-target count fields, commit/reveal scenario
count, decision outcome count, and transparency publication-target count from the
corresponding reviewed inventories before checker prevalidation, and it carries
integer route-latency and event-lag threshold facts. It forces raw evidence,
commit/reveal payload, decision, transaction, ledger, response body, signed URL,
session token, private juror data, and watermark secret inclusion flags to
`false`. Moderation-panel payload-safety artifacts must explicitly set
`payloads_included`, `response_bodies_included`,
`juror_private_data_included`, `raw_evidence_included`,
`session_tokens_included`, `signed_urls_included`,
`watermark_secrets_included`, `message_bodies_included`,
`commit_payloads_included`, `reveal_payloads_included`,
`raw_decision_included`, `signed_transaction_included`,
`raw_ledger_included`, and `critical_alerts_firing` to `false` before
promotion can report ready. It prevalidates the generated artifact with
`check_sorafs_moderation_panel_rollout_evidence.py`, and writes the JSON
atomically without following output symlinks. Example argfiles are checked in
for the appeal-intake anchor, commit/reveal tally, and end-to-end panel
evidence:

```sh
python3 scripts/build_sorafs_moderation_panel_canary.py \
  @scripts/examples/sorafs_moderation_panel_appeal_intake_canary.args.example

python3 scripts/build_sorafs_moderation_panel_canary.py \
  @scripts/examples/sorafs_moderation_panel_commit_reveal_canary.args.example

python3 scripts/build_sorafs_moderation_panel_canary.py \
  @scripts/examples/sorafs_moderation_panel_e2e_canary.args.example
```

The checker recognizes `sorafs.moderation_panel.*` SFM-4b rollout schemas for
appeal intake, sortition roster, evidence viewer, operator workflow, juror
notifications, commit/reveal, decision publication, settlement integration,
transparency/reputation handoff, end-to-end panel runs, metrics/alerts, and
governance approval. It reports `ready` only when every required kind is
present, every recognized artifact is valid, raw evidence payloads, private
commit/reveal payloads, message bodies, response bodies, signed transactions,
secrets, and raw ledgers are absent, route latency and event lag are
integer-unit evidence under configured thresholds, panel size and end-to-end
peer count meet the configured minimums, governance is bound to `iroha_config`,
every case-bound artifact
shares the valid appeal-intake `case_digest_hex`, every roster-bound artifact
shares a valid case-bound sortition `case_digest_hex`/`roster_hash_hex` pair,
and every tally-bound artifact shares a valid roster-bound commit/reveal
`case_digest_hex`/`roster_hash_hex`/`tally_digest_hex` tuple. Sortition rosters
that fail appeal-intake binding and commit/reveal runs that fail roster binding
do not anchor downstream rollout evidence.
Appeal-intake artifacts also bind `case_count` to the unique canonical
`cases[].name` inventory, require `accepted_case_count` to match the
`cases[].accepted` partition, require reviewed lowercase
`moderation-appeal-case-*` labels without non-production markers, and reject
duplicate case entries before promotion can report ready.
Sortition-roster artifacts also bind `panel_size` to the unique canonical
`jurors[].name` inventory, require `panel_size` to match the
`jurors[].eligible` partition, require reviewed lowercase
`moderation-roster-juror-*` labels without non-production markers, and reject
duplicate roster juror entries before promotion can report ready.
Appeal-intake, operator-workflow, commit/reveal, and decision-publication
artifacts also bind `route_count` to the unique canonical `routes[].name`
inventory and reject duplicate or unknown route entries before promotion can
report ready, and require every route response to carry a lowercase
`body_blake3_hex` digest.
Evidence-viewer artifacts also bind `session_count` to the unique canonical
`sessions[].name` inventory, require `attested_session_count` and
`logged_session_count` to match the `sessions[].attested` and
`sessions[].logged` partitions, require reviewed lowercase
`moderation-viewer-session-*` labels without non-production markers, and reject
duplicate session entries before promotion can report ready.
Evidence-viewer artifacts also bind `role_count`, `security_control_count`,
`access_event_kind_count`, and `export_target_count` to the unique canonical
`roles_tested`, `viewer_security_controls`, `access_event_kinds`, and
`export_targets` inventories and reject missing, inflated, duplicate, or
unknown scalar coverage before promotion can report ready.
Juror-notification artifacts also bind `notification_count` and `juror_count`
to the unique canonical `notifications[].name` and `jurors[].name`
inventories, require `delivered_notification_count` to match the
`notifications[].delivered` partition, require reviewed lowercase
`moderation-notification-*` notification labels and `moderation-juror-*` juror
labels without non-production markers, and reject duplicate notification or
juror entries before promotion can report ready.
Commit/reveal artifacts also bind `commit_count` and `reveal_count` to the
unique canonical `commits[].name` and `reveals[].name` inventories, reject
duplicate commit or reveal entries, require reviewed lowercase
`moderation-commit-*` and `moderation-reveal-*` labels without non-production
markers, and reject reveal totals above the reviewed commit total before
promotion can report ready.
Commit/reveal artifacts also bind `scenario_count` to the unique canonical
`scenarios_exercised` inventory and reject missing, inflated, duplicate, or
unknown scenario coverage before promotion can report ready.
Settlement-integration artifacts also bind `settlement_count` to the unique
canonical `settlements[].name` inventory, require reviewed lowercase
`moderation-settlement-*` labels without non-production markers, and reject
duplicate settlement entries before promotion can report ready.
Decision-publication artifacts also bind `outcome_count` to the unique
canonical `outcomes` inventory and reject missing, inflated, duplicate, or
unknown outcome coverage before promotion can report ready.
Transparency/reputation artifacts also bind `publication_target_count` to the
unique canonical `publication_targets` inventory and reject missing, inflated,
duplicate, or unknown publication-target coverage before promotion can report
ready.
Metrics/alert artifacts also bind `metric_count` to the unique canonical
`metrics` inventory and reject duplicate or unknown metric entries before
promotion can report ready.
The summary exports the sorted reviewed `metrics` inventory plus
`metric_count_values`, and the aggregate production-readiness gate requires
those fields to match the metrics/alert artifact fingerprint before final
promotion can report ready.
End-to-end panel artifacts also bind `peer_count` and `validator_count` to the
unique canonical `peers[].name` and `validators[].name` inventories and reject
duplicate peer or validator entries before promotion can report ready, and
require reviewed lowercase
`moderation-peer-*` and `moderation-validator-*` labels without non-production
markers.
End-to-end panel artifacts also bind `case_count` to the unique canonical
`cases[].name` inventory, require `case_count` to match the `cases[].passed`
partition, require reviewed lowercase `moderation-case-*` labels without
non-production markers, and reject duplicate end-to-end case entries before
promotion can report ready.
The gate also requires end-to-end panel evidence to carry `policy_digest_hex`,
publishes those values as `valid_policy_digests`, and requires governance
approval `policy_digest_hex` to match one of those valid panel policy digests.
The evidence-viewer canary
also requires role-scoped manifests, short-lived segment URLs, attested and
logged sessions, strict CSP and disabled offline mode, watermark overlay and
metadata hashing, append-only access logging, anomaly events, legal-hold
binding, Governance DAG and transparency-ledger export coverage, daily digest
publication, session-manifest, watermark-metadata, access-log,
legal-hold-receipt, transparency-report, and audit digest hashes, and full
coverage for view/seek/pause, screenshot-attempt, download-attempt, and
annotation event classes without including signed URLs, session tokens,
watermark secrets, raw evidence, raw access logs, legal-hold receipt payloads,
transparency report payloads, or response bodies. The commit/reveal canary also
requires commit digest
recomputation, duplicate-commit rejection, mismatched-reveal rejection, late
commit/reveal rejection, missed-quorum detection, no-show failover, juror
penalty planning, deterministic tally replay, contested challenge coverage,
governance event digest binding, event-lag bounds, and absence of raw
commit/reveal payloads.

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
cargo test -p iroha_torii moderation_ballot_no_show --features app_api -- --nocapture
cargo test -p iroha_torii generated_spec_includes_documented_paths --features app_api
```

Run the broader SoraFS moderation and gateway suites when the panel service
starts mutating runtime state.

The rollout evidence scripts have focused Python coverage in:

- `scripts/tests/check_sorafs_moderation_panel_rollout_evidence_test.py`
- `scripts/tests/run_sorafs_moderation_panel_rollout_evidence_test.py`

The runner validates the schema-closed collection-plan envelope before printing dry-run JSON or executing the verifier.
The shared runner plan guard also rejects non-canonical nested required-kind,
threshold, external-evidence, evidence-contract, and command-step shapes before
dry-run output or verifier execution.
