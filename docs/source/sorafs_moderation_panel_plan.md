---
title: Moderation Appeals & Sortition Panels
summary: SFM-4b implementation status for appeal finance, policy-jury data foundations, and remaining moderation panel services.
---

# Moderation Appeals & Sortition Panels

## Current Status

SFM-4b includes appeal-finance helpers, moderation reproducibility tooling,
honey-audit probes, reusable policy-jury data structures, SoraFS ballot
envelopes, and an authoritative native moderation ledger. Appellant-bound
intake, PoP-gated eligibility, deterministic primary/waitlist sortition,
assignment acceptance and failover, commit/reveal, challenges, terminal
outcomes, and classified no-shows are consensus-owned transitions.

The process-local sorafs_node ballot runtime, checkpoint, event stream, and
caller-fed projection have been deleted. Torii transports exact signed native
instructions and serves only reconciled finalized-chain projections. The
durable orchestrator is a transaction submitter and committed-state consumer,
not a competing authority.

The fail-closed SFM-4b evidence checker and collection runner bind every
recognized artifact to one reviewed deployment, case, roster, tally, and active
policy context. Moderation GETs now read only a fresh worker-owned finalized
projection; reconciliation runs outside request threads under supervised
deadlines, non-overlap fencing, monotonic cursor/freshness/liveness checks, and
dead-letter readiness. The evidence viewer's signed receipt checkpoint and
exact predecessor-bound projection are its sole audit authority, and missing
runtime dependencies fail through typed payload-free startup errors. The
retained checkpoint anchor signs its canonical digest, receipt count, exact
chain head, and governed signer identity; audit pages require that digest and
an explicit digest-bound limit and return `409` on change. A deployment-owned
monotonic transparency or ledger head is still required for first-contact
freshness.

The durable orchestrator now runs payload-free panel notifications through the
same supervised maintenance owner. It checkpoints a lease before leaving local
state, derives a stable worker identity from the chain and governed provider
binding, sends one canonical Norito notification to an independently qualified
idempotent boundary, and checkpoints the exact receipt or bounded
retry/dead-letter result. Enabling the orchestrator requires
`panel_notification_handle`, `panel_notification_revision`, and
`panel_notification_policy_digest_hex`; these values are public bindings, not
credentials. Standard `irohad` forwards the deployment-owned boundary into
Torii, and startup or an in-flight call fails closed when the provider is
missing, substituted, stale, unavailable, or test-marked.

The strict transaction ingress is the one deliberate local boundary: it admits
an already signed transaction through Torii's canonical durable queue and owns
no signing authority. It no longer self-attests from operator-supplied values.
The implementation reports the fixed handle
`torii.sorafs.moderation-strict-ingress.v1`, revision `1`, and the BLAKE3 policy
digest
`cc0ceac18b93fa9705c0ef86f657a9ed94c5dd6531578496a2d64e8ec5216d2e`.
The three configured `strict_ingress_*` values are an independent expected
binding; Torii startup fails if any of them differs from that implementation
identity.

Remaining production blockers under
`V1-BLOCK-MODERATION-VIEWER-RUNTIME-01` are deployment construction of the real
messaging, settlement/publication, HSM/KMS/WebAuthn, linearizable sealed-CAS
checkpoint-store, and signed receipt-to-transparency providers; a monotonic
public-head adapter; semantic operation-ID fencing; equivalent
predecessor-bound CAS/single-writer ownership for the moderation orchestrator;
and signed replay-safe terminal and receipt compaction/archive. The evidence
viewer core and standard launcher now require the qualified external
checkpoint authority and treat its local file as a verified cache, but the
repository still lacks a real store adapter and reviewed reference-deployment
evidence for multi-replica CAS, the complete moderation service, evidence
viewer, juror notification/portal workflow, downstream
settlement/publication, and four-validator recovery scenarios.

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
  `escalate` choices.
- `iroha_data_model::sorafs::moderation_ledger`, first-class moderation ISIs,
  and typed `FindSorafsModeration*` queries provide the consensus-owned ballot
  source of truth. Appellant-bound intake pins the active moderation policy and
  native PoP root/revocation/audit snapshot, deduplicates case, proof-token,
  and deposit-lock digests, and fixes bounded registration, acceptance,
  commit/challenge/reveal windows. Private Halo2 membership proofs are verified
  against that exact historical snapshot while only their digest, appeal
  nullifier, eligibility class, expiry, and account binding are retained.
  Ordinary later issuer-policy, commitment-root, revocation-list, and audit-head
  advancement cannot rewrite or strand an admitted appeal: every historical
  publication and audit link is revalidated from consensus state. An emergency
  registry pause intentionally freezes both new and pending moderation use until
  governance resumes it. Sortition
  freezes the latest committed parent hash only after registration closes,
  rejects the appellant even when that account also holds the management
  permission, and requires the independent operator instruction to name that
  exact anchor. It then uses a
  domain-separated, hardware-independent BLAKE3 score over the frozen seed and
  deterministic per-credential appeal nullifier, rejects duplicate people and
  accounts, recomputes the proposed roster, and persists the anchor plus an
  immutable waitlist. Primary assignment acceptance and deterministic waitlist failover
  activate the existing commit/reveal case atomically or record a terminal
  insufficient-pool/failover-exhausted state. Governance policy snapshots also
  bound panel/window/challenge resources and distinct
  missing-commit/unrevealed-commit penalties; block time controls every phase;
  juror transactions are authority-bound; payload-free challenges block unsafe
  progress; accepted challenges close without juror penalties; and challenges
  left unresolved through reveal close are atomically marked `expired` and
  force the same fail-safe challenged outcome rather than deadlocking the case
  or retroactively creating no-show penalties. Finalization atomically persists
  the appeal and ballot outcome plus any valid no-show records. These instructions and queries use Iroha's existing
  signed transaction and generic Torii query APIs.
- `iroha_data_model::sorafs::pop_registry`, its permissioned issuer ISIs, and
  public typed queries now provide the consensus-owned active credential root,
  revocation root/list version, payload-free credential/revocation commitments,
  and audit head consumed by the moderation snapshot. Pending appeals read and
  verify those immutable historical anchors even after the active registry
  advances; missing, mutated, or non-ancestral pinned state still fails closed. Raw
  credential, witness, nonce, and proof payloads are never persisted in the
  moderation ledger.
- sorafs_node no longer owns a moderation ballot, appeal, scheduler,
  checkpoint, or event-stream authority. The only authoritative lifecycle is
  the native moderation ledger; node-side screening, quarantine, evidence
  viewing, and orchestration are non-authoritative consumers.
- Torii's version-one moderation routes accept exact signed native ISIs for
  intake, eligibility, sortition, assignment acceptance, activation, commit,
  challenge, reveal, and finalization. List, detail, no-show, and event
  responses come from a reconciled finalized-chain snapshot with bounded
  arrays and cursors; there is no local ballot fallback.
- The durable orchestrator submits transactions and reconciles committed state.
  Exact-once settlement/publication handoff adapters and the supervised
  payload-free panel-notification worker consume finalized outcomes rather than
  a process-local tally stream. Notification claims are durable before the
  provider call; replay uses the same notification identity and canonical
  bytes; qualification drift becomes an ambiguous retry; and exact receipts or
  dead letters are checkpointed. Deployment-owned implementations of those
  external boundaries are still required.
- `sorafs_manifest::SoraFsModerationBallotGovernanceEventV1` and the embedded
  node's signed Governance DAG outbox retain the canonical payload-free
  publication format, but production producers must derive it from committed
  native events.
- `docs/examples/ministry/policy_jury_roster_example.json` and
  `docs/examples/ministry/policy_jury_sortition_example.json` provide example
  inputs for the reusable policy-jury data path.
- `sorafs_cli moderation validate-repro`,
  `sorafs_cli moderation validate-corpus`, and
  `sorafs_cli moderation honey-audit` support moderation reproducibility and
  gateway enforcement evidence.

## Intended Panel Lifecycle

The production service wraps this lifecycle; steps 1, 2, and the
consensus-owned portion of step 4 are now native ledger transitions:

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

The deployed evidence-viewer, notification/orchestration, downstream outcome,
and settlement integrations around those native transitions remain open.

## Data Boundaries

The reusable `PolicyJury*` types remain governance data structures, while the
native moderation intake/sortition lifecycle supplies the SoraFS-specific
runtime binding. Together with the ballot wrappers it binds:

- appeal case identifiers;
- moderation policy references;
- proof-token and governed compliance catalog/denial evidence references;
- panel roster hashes;
- settlement manifest version;
- moderation vote choices;

The authoritative ledger and finalized-chain Torii projection bind
panel-size/quorum policy, intake/deposit digests, exact active PoP snapshots,
proof-nullifier replay protection, deterministic primary/waitlist selection,
assignment failover, commit/reveal windows, eligible jurors, and payload-free
no-show readback. The production service still needs deployed integrations that
bind:

- evidence access attestation;
- decision publication and appeal cache updates.

Do not document `sorafs moderation jury-accept`,
`sorafs moderation open-case`, or similar portal commands as shipped until the
corresponding service and CLI handlers exist. Native assignment acceptance and
case activation are submitted as typed ISIs; no direct-open ISI exists.

## Remaining Production Gates

- Construct and inject the reference deployment's real messaging,
  settlement/publication, HSM/KMS/WebAuthn, and authenticated downstream
  providers through the shipped qualified boundaries. Add the signed
  receipt-to-transparency producer, cross-replica semantic operation-ID
  fencing, predecessor-bound checkpoint CAS/single-writer ownership, and signed
  replay-safe terminal/receipt compaction and archive.
- Deploy the existing appeal/panel transaction outbox, finalized-chain
  orchestrator, retry/reconciliation worker, supervised panel-notification
  pass, and challenge/no-show maintenance with the remaining juror portal and
  scheduled settlement provider integrations around the authoritative intake,
  sortition, and commit/reveal ledger described by
  `docs/source/sorafs_commit_reveal_plan.md`.
- Connect panel outcomes to gateway compliance caches, transparency
  publication, settlement reconciliation, and reputation scoring.
- Connect committed native moderation events to the durable Governance DAG and
  public IPFS/IPNS decision trail.
- Add reviewed four-peer deployed end-to-end tests for appeal submission,
  private juror enrollment, evidence access, commit/reveal voting, decision
  publication, restart reconciliation, and settlement. Native unit/integration
  coverage already exercises malformed/noncanonical proof rejection, wrong and
  rotated roots, credential-nullifier replay, biased/duplicate rosters,
  deadline and authority violations, insufficient pools, no-show failover and
  exhaustion, replay, and transaction atomicity.
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
`moderation-viewer-session-*` `--viewer-session` labels whose unique
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

The checker exports its required top-level payload fields as
`EVIDENCE_REQUIRED_FIELDS`, and the runner dry-run emits the checker-backed
`evidence_contract` map listing each selected evidence kind's schema and
required payload fields. Every recognized rollout artifact must also carry
reviewed `deployment_id` and `environment` context, and the gate blocks mixed
reviewed deployment contexts across the same rollout bundle.

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
The aggregate production-readiness gate also requires `valid_roster_bindings`,
`valid_tally_bindings`, `valid_e2e_runs`, and
`valid_evidence_viewer_digest_sets` to preserve those case, roster, tally, and
viewer-observed gateway catalog relationships before final promotion can
report ready. Evidence-viewer fingerprints and digest sets export the canonical
nested `gateway_compliance_denial_enforced.catalog_digest_hex` value. When the
moderation-panel and gateway-compliance lanes are promoted together, the
viewer-observed catalog digest set must equal gateway compliance
`valid_catalog_digests`. The aggregate gate also rechecks the lane-proven bound
artifacts before final promotion: case-bound artifact fingerprints must match
`valid_case_digests`, roster-bound artifact fingerprints must match
`valid_roster_bindings`, tally-bound artifact fingerprints must match
`valid_tally_bindings`, and policy-bound governance approval fingerprints must
match `valid_policy_digests`. The moderation-panel gate fail-closes when more
than one valid case, roster, tally, or policy anchor appears, and clears the
mixed `valid_case_digests`, `valid_roster_bindings`,
`valid_tally_bindings`, or `valid_policy_digests` set before aggregate
promotion can report ready.
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
transparency report payloads, or response bodies. Evidence-viewer canaries also
require explicit audit-log tamper rejection and watermark metadata mismatch
rejection before promotion can report ready. The commit/reveal canary also
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
