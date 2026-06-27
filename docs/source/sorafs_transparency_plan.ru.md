---
lang: ru
direction: ltr
source: docs/source/sorafs_transparency_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7180a3e8fda0ce3cbf42bca6393fcd52dbe7ef100a6e6fe23b520010fdb5776a
source_last_modified: "2026-06-25T17:05:30+00:00"
translation_last_reviewed: 2026-06-25
---

# Transparency Dashboards & Enforcement Receipts

## Current Status

SFM-4c is partially implemented. The repository contains GAR enforcement receipt
types, moderation validation evidence, honey-audit reports, moderation and
privacy dashboards, SoraNet privacy metrics, canonical V1 transparency ledger
data-model payloads for entries, cycle headers, inclusion proofs, publication
bundles, privacy-safe aggregate payloads with explicit
epsilon/delta/suppression metadata, a node-side privacy aggregate cycle
publication bridge, a local aggregate source-event worker with deterministic
suppression/noise computation, a config-backed due-cycle scheduler API for
aggregate publication, a local transparency ledger source-entry worker for
privacy-safe GAR/moderation/appeal/legal-hold/redaction/evidence-access style
entries, concrete local adapters for GAR receipts, moderation ballot governance
events, appeal finance reports, and appeal finance settlement receipts, a
canonical-authenticated local Torii source-entry ingest route for those concrete
payloads, a canonical-authenticated local Torii privacy aggregate source-event
ingest route, a canonical-authenticated local Torii configured privacy aggregate
publish-due trigger, client/CLI producer and scheduler trigger tooling for
those aggregate routes, payload-free source-entry producer canary evidence
tooling, payload-free privacy aggregate canary evidence tooling for
producer/scheduler rollout archives, a canonical-authenticated local Torii
proof-token issuance feed, client/CLI proof-token issuance producer tooling and
payload-free canary evidence tooling, and a local SoraFS node publication path
into the Governance DAG filesystem/CAR queue, optional signed runtime DAG
external payloads, and Torii readback endpoints for locally published cycles,
entry inclusion proofs, bounded proof-token issuance indexes, a bounded local
explorer snapshot, a local browser transparency explorer UI, and gateway
proof-token verification, plus payload-free publication readback canary
tooling.
The local node can also derive and publish
proof-token issuance records from signed `SFGT` frames after verifying the
Ed25519 signer key. `iroha::client` and `iroha sorafs transparency
cycles|explorer|explorer-canary|publication-canary|tokens|token-issuance|source-entry|privacy-aggregate`
now wrap the local readback, rollout-evidence canaries, signed proof-token
issuance and source-entry ingest, and privacy aggregate source-event/publish-due
surfaces for operator automation. `scripts/check_sorafs_transparency_rollout_evidence.py`
now validates the collected source-entry, publication, privacy aggregate,
proof-token issuance, and explorer canary artifacts before a rollout can claim
ready status. The gate now also rejects mixed transparency bundles: publication
evidence must bind to a valid source-entry `source_batch_digest_hex`, while
privacy aggregate, proof-token issuance, and explorer evidence must bind to a
source-bound publication `cycle_digest_hex` from the same rollout bundle.
The local appeal-finance
report, weekly rollup, and
settlement receipt dashboards also bound their returned Governance DAG source
entry arrays via `limit` while keeping full aggregate totals visible, and cycle
detail readback bounds returned publication proofs without weakening full-cycle
verification. It does
not yet ship
deployed GAR/moderation/appeal producers that call the source-entry route,
captured deployed aggregate producer/scheduler rollout evidence, proof service
hardening, deployed public receipt explorer rollout evidence, deployed
proof-token issuance producers/explorer-linking rollout evidence, or deployed
moderation ledger publication service described by the original plan.

## Shipped Foundations

- `GarEnforcementReceiptV1` records deterministic GAR enforcement evidence:
  receipt id, GAR name, canonical host, action, timestamps, policy version,
  policy digest, operator, reason, notes, evidence URIs, and labels.
- `GarPolicyPayloadV1` carries licensing, CDN policy, moderation directives,
  metrics policy, telemetry labels, and optional replication-proof-token digest
  data.
- Torii gateway policy emits structured GAR violation events for denylist,
  moderation, legal hold, TTL, rate-limit, and region-policy failures.
- `sorafs_cli moderation honey-audit` emits JSON/Markdown gateway enforcement
  evidence for denied digests and optional moderation proof validation.
- `dashboards/grafana/ministry_moderation_overview.json` and
  `dashboards/alerts/ministry_moderation_rules.yml` provide the moderation
  manifest, drift, ingest, and latency monitoring story.
- `dashboards/grafana/soranet_privacy_metrics.json` and
  `dashboards/alerts/soranet_privacy_rules.yml` cover SoraNet privacy
  aggregation and suppression telemetry; these are adjacent privacy metrics, not
  a SoraFS moderation transparency ledger.
- `iroha_data_model::sorafs::transparency` defines
  `ModerationLedgerEntryV1`, `ModerationLedgerBlockV1`,
  `ModerationLedgerProofV1`, `ModerationLedgerCyclePublicationV1`, and
  deterministic BLAKE3 Merkle proof helpers with Norito/JSON roundtrip and
  tamper/ordering validation coverage.
- `ModerationPrivacyAggregateV1` and `ModerationPrivacyParametersV1` define a
  canonical privacy-safe aggregate payload with explicit
  differential-privacy and suppression parameters, sorted public metrics and
  metadata, a domain-separated aggregate hash, and conversion into a
  `PrivacyAggregate` transparency ledger entry.
- `sorafs_node::NodeHandle::publish_privacy_aggregate_cycle(...)` validates
  privacy aggregate payloads, requires them to fit within the requested cycle
  window, sorts them deterministically, derives stable transparency entry ids,
  builds a `ModerationLedgerCyclePublicationV1`, and publishes it through the
  configured Governance DAG publisher.
- `sorafs_node::NodeHandle::record_privacy_aggregate_source_event(...)` and
  `publish_privacy_aggregate_cycle_from_source_events(...)` provide the local
  source-event worker foundation: duplicate source-event rejection, cycle-window
  filtering, suppression-threshold enforcement, deterministic bounded noising
  from runtime seed material, source-payload digest binding, and handoff to the
  existing aggregate cycle publisher.
- `PrivacyAggregateScheduleConfig` and
  `NodeHandle::publish_due_privacy_aggregate_cycle_from_source_events(...)`
  provide due-cycle scheduling for the aggregate worker. The method derives
  deterministic cycle ids from due windows, catches up the oldest due
  unpublished window with retained source events before considering the latest
  due window empty, skips not-due/empty/already published/fully suppressed
  windows explicitly, and publishes each due cycle at most once per node
  runtime.
- `iroha_config` exposes the dormant-by-default
  `[sorafs.storage.privacy_aggregates]` scheduler knobs for enablement, cycle
  width, and publish delay. `sorafs_node::StorageConfig` projects enabled config
  into `PrivacyAggregateScheduleConfig`, and
  `NodeHandle::publish_due_configured_privacy_aggregate_cycle_from_source_events(...)`
  uses that cadence while keeping privacy policy and noise seed material as
  explicit runtime-only inputs.
- Torii exposes
  `/v1/sorafs/transparency/privacy-aggregates/source-events` for
  canonical-authenticated local aggregate source-event ingestion. The handler
  records one source event in the duplicate-checked aggregate worker for later
  configured cycle publication and returns only event ids, digests, and counts
  rather than raw metric values.
- Torii exposes
  `/v1/sorafs/transparency/privacy-aggregates/publish-due` for
  canonical-authenticated local configured aggregate publication. The handler
  evaluates the configured schedule, catches up stale due event-backed windows,
  accepts privacy policy, optional noise seed, policy digest, previous block
  hash, and public aggregate metadata as runtime-only request material, and
  returns structured
  published/skipped/already-published outcomes with cycle hashes when a cycle is
  published.
- `iroha::Client` and
  `iroha sorafs transparency privacy-aggregate source-event|publish-due
  --payload PATH` wrap the signed source-event and publish-due routes for
  deployed producer and scheduler automation. Payload files are loaded as
  canonical JSON and are rejected when empty before signing.
- `iroha sorafs transparency privacy-aggregate canary --source-event PATH
  [--source-event PATH...] [--publish-due PATH...] [--out PATH]` submits
  operator-supplied canary source-event and publish-due payloads through those
  signed routes, records request/response sizes, status, and BLAKE3 hashes, and
  emits `sorafs.transparency.privacy_aggregate.canary.v1` evidence without
  archiving raw metric arrays, metric names, or response bodies.
- `sorafs_node` can publish a validated
  `ModerationLedgerCyclePublicationV1` bundle through the configured local
  Governance DAG filesystem sink. The publisher writes `.to` and `.json`
  sidecars, publish-index labels, digest sidecars, and CAR queue segments under
  the `transparency_ledger_publication` payload kind.
- `ProofTokenIssuanceV1` records privacy-safe summaries of issued `SFGT`
  proof-token frames: token id, issued/expiry timestamps, action code, signer
  verifying key, token/blinded digests, bound public entry ids, optional
  evidence/policy digests, and sorted metadata. It exposes a canonical issuance
  hash and conversion into a `ProofTokenIssuance` transparency ledger entry.
- `sorafs_node::NodeHandle::publish_proof_token_issuance(...)` publishes those
  validated issuance records through the local Governance DAG filesystem sink.
  The publisher writes canonical `.to` payloads, JSON sidecars, digest
  sidecars, publish-index labels, and CAR queue segments under the
  `proof_token_issuance` payload kind.
- `sorafs_node::proof_token_issuance_from_frame(...)`,
  `proof_token_issuance_from_base64(...)`, and
  `NodeHandle::publish_proof_token_base64_issuance(...)` provide the local
  signed-frame ingest adapter for issued `SFGT` tokens. The adapter decodes the
  frame, verifies the Ed25519 signature against caller-supplied public signer
  bytes, derives the public token digest/blinded digest/action/entry metadata,
  and publishes the resulting `ProofTokenIssuanceV1` without accepting or
  persisting blinded-digest keys.
- `sorafs_node::TransparencyLedgerSourceEntry`,
  `NodeHandle::record_transparency_ledger_source_entry(...)`, and
  `publish_transparency_ledger_cycle_from_source_entries(...)` provide the
  local generic transparency source-entry worker. The worker admits
  privacy-safe GAR/moderation/appeal/legal-hold/redaction/evidence-access style
  entries with duplicate-id rejection, filters a requested cycle window, sorts
  entries deterministically, derives stable ledger entry ids, builds a
  `ModerationLedgerCyclePublicationV1`, and publishes it through the existing
  Governance DAG sink.
- `gar_enforcement_receipt_source_entry(...)`,
  `moderation_ballot_governance_event_source_entry(...)`,
  `appeal_finance_report_source_entry(...)`, and
  `appeal_finance_settlement_receipt_source_entry(...)` adapt existing typed
  SoraFS payloads into privacy-safe transparency source entries using canonical
  Norito payload digests and sorted public metadata. `NodeHandle` exposes
  explicit record helpers for those sources, and local moderation/appeal
  publication paths now record best-effort source entries for governance ballot
  events plus derived appeal finance reports.
- Torii exposes
  `/v1/sorafs/transparency/source-entries/{source_kind}` for
  canonical-authenticated local source-feed ingestion. The handler decodes one
  typed JSON payload selected by `source_kind`, derives the same privacy-safe
  public source entry through either a concrete adapter or the public notice
  summary DTO, records it in the local duplicate-checked source-entry worker,
  and returns only the derived public summary. Supported source kinds are
  `gar-enforcement-receipt`,
  `moderation-ballot-governance-event`, `appeal-finance-report`, and
  `appeal-finance-settlement-receipt`, plus `legal-hold-notice`,
  `redaction-notice`, and `evidence-access-summary`.
- `sorafs_manifest` defines `GovernanceExternalPayloadV1`, and `sorafs_node`
  uses it to sign the canonical transparency publication bytes into the local
  runtime Governance DAG when a runtime signer is configured.
- Torii exposes local public readback for
  `/v1/sorafs/transparency/cycles`,
  `/v1/sorafs/transparency/cycles/{cycle_id_hex}`, and
  `/v1/sorafs/transparency/cycles/{cycle_id_hex}/entries/{entry_id_hex}`. The
  handlers read `transparency_ledger_publication` entries from the Governance DAG
  publish-index, reject escaping artifact paths, verify canonical `.to` length
  and BLAKE3 digest, decode the Norito publication bundle, and re-check cycle
  hashes and inclusion proofs before returning cycle or entry proof JSON. The
  cycle list reports total and returned counts and bounds its summary array via
  `limit` with a default of 50 and a max of 500. Cycle detail readback uses the
  same `limit` contract for the returned `publication.proofs` array while
  keeping verification counts over the full decoded publication.
- `iroha sorafs transparency publication-canary [--cycle-id HEX...]
  [--limit N] [--torii-url URL] [--out PATH]` probes the deployed/public cycle
  list and optional cycle-detail routes, requires publisher identity fields
  unless explicitly waived, checks anchor metadata and verification flags, and
  emits `sorafs.transparency.publication_canary.v1` evidence with status,
  sizes, and BLAKE3 response hashes without archiving publication bodies,
  source entries, or private payload material.
- Torii exposes `/v1/sorafs/transparency/explorer` as a local read-only
  explorer snapshot. The endpoint composes the Governance DAG publish-index into
  cycle summaries, proof-token issuance summaries, payload-kind counts, source
  paths, index digests, and cache validators for public UI integration without
  exposing private proof-token digest keys. The cycle and proof-token issuance
  arrays are bounded by `limit` with a default of 50 and a max of 500 while
  total counts remain visible.
- Torii exposes `/v1/sorafs/transparency/tokens` for local proof-token issuance
  index readback. The endpoint summarizes `proof_token_issuance` publish-index
  entries by action code, distinct token ids, distinct signer keys, bound entry
  totals, expiry presence, and evidence-digest presence, and returns source
  entries for explorer linking. Aggregates are computed over the full local
  index, while the returned `entries` array is bounded by `limit` with a default
  of 50 and a max of 500.
- Torii exposes `/v1/sorafs/transparency/tokens/issuances` for
  canonical-authenticated local proof-token issuance feed ingestion. The handler
  accepts one URL-safe base64 `SFGT` frame, the Ed25519 signer public key,
  optional evidence/policy digests, and sorted public metadata, then verifies
  the signed frame through the node ingest helper, derives a
  `ProofTokenIssuanceV1`, and publishes it through the local Governance DAG
  publisher when configured. Blinded-digest keys are not accepted by this feed.
- `iroha::Client::post_sorafs_transparency_token_issuance_json(...)` and
  `iroha sorafs transparency token-issuance submit --payload PATH` wrap the
  signed proof-token issuance feed for deployed producer automation. Payload
  files are loaded as canonical JSON and are rejected when empty before
  signing.
- `iroha sorafs transparency token-issuance canary --issuance PATH
  [--issuance PATH...] [--out PATH]` submits operator-supplied proof-token
  issuance canary payloads through the signed
  `/v1/sorafs/transparency/tokens/issuances` route, records request/response
  sizes, status, and BLAKE3 hashes, and emits
  `sorafs.transparency.proof_token_issuance.canary.v1` evidence without
  archiving proof-token frames, private digest-key material, or response
  bodies.
- Torii exposes `/v1/sorafs/transparency/tokens/verify` for local verification
  of `SFGT` gateway proof-token frames. Callers supply the token and Ed25519
  verifying key, plus optional runtime-only blinded-digest key/evidence digest
  material when they need private evidence binding checks. The response reports
  signature, digest, expiry, and not-before status without persisting digest-key
  material. The verifier honors configured Torii API-token enforcement and uses
  the shared proof API limiter, returning `429` plus `Retry-After` when a caller
  exceeds the configured proof request budget.
- `iroha::client` exposes SoraFS transparency readback helpers for cycles,
  cycle detail, entry proofs, explorer snapshots, and proof-token issuance
  indexes, signed proof-token issuance submission for
  `/v1/sorafs/transparency/tokens/issuances`, and signed source-entry JSON
  submission for `/v1/sorafs/transparency/source-entries/{source_kind}`.
  `iroha sorafs transparency cycles list|get|entry`, `iroha sorafs
  transparency explorer`, `iroha sorafs transparency tokens`, `iroha sorafs
  transparency token-issuance submit`, and `iroha sorafs transparency
  source-entry submit` provide the matching operator bridge.
- `iroha sorafs transparency source-entry canary --source-entry KIND=PATH
  [--source-entry KIND=PATH...] [--out PATH]` submits operator-supplied
  source-entry producer canary payloads through the signed
  `/v1/sorafs/transparency/source-entries/{source_kind}` route, records
  request/response sizes, status, and BLAKE3 hashes, and emits
  `sorafs.transparency.source_entry.canary.v1` evidence without archiving
  source payload fields, private payload material, or response bodies.
- `scripts/check_sorafs_transparency_rollout_evidence.py --evidence-dir DIR
  [--summary-out PATH]` validates collected SFM-4c rollout artifacts and emits
  `sorafs.transparency.rollout_evidence_gate.v1` summary JSON. The gate
  requires source-entry, publication, privacy aggregate, proof-token issuance,
  and explorer canary schemas, requires every included canary to pass, verifies
  all supported source-entry producer kinds, requires publication list and
  cycle-detail probes, verifies publication anchor/publisher/verification
  signals, requires both aggregate source-event and publish-due probes, checks
  the explorer snapshot/UI/proof-token index routes, and recursively rejects raw
  payload, request/response body, bearer-token, signed-transaction,
  proof-token frame, private-key, and private digest-key fields. Publication
  evidence must match a valid source-entry `source_batch_digest_hex`, and
  privacy aggregate, proof-token issuance, and explorer evidence must match a
  source-bound publication `cycle_digest_hex`; publication cycles that fail
  source-entry binding do not anchor downstream rollout evidence. The checker
  supports shell-style `@ARGFILE` inputs for direct replay of reviewed artifact
  directories.
- `scripts/run_sorafs_transparency_rollout_evidence.py --torii-url URL
  --out-dir DIR ...` is the operator harness for collecting the required
  source-entry, privacy aggregate, proof-token issuance, publication, and
  explorer canary artifacts and then running the rollout evidence verifier. It
  fails before live submission when required source-entry kinds, privacy
  source-event/publish-due payloads, proof-token issuance payloads, or
  publication cycle-detail ids are missing, accepts repeated `--iroha-arg ARG`
  values for runtime-only client config/signing options that must be passed
  before `sorafs`, accepts shell-style `@ARGFILE` response files for reviewed
  operator inputs, and `--dry-run` emits the command plan without contacting
  live services. `scripts/examples/sorafs_transparency_rollout_evidence.args.example`
  documents the required source-entry kinds, aggregate probes, proof-token
  issuance probe, cycle id, Torii URL, and runtime-only client-config path
  without storing signing material.

## Target Ledger Model

The canonical V1 data model now covers privacy-safe entries, cycle headers,
entry roots, block hashes, inclusion proofs, publication bundles, and
privacy-safe aggregate payloads that can be converted into ledger entries. The
local SoraFS node can materialize those bundles into the Governance DAG
filesystem/CAR pipeline and optional signed runtime DAG blocks, and Torii can
verify and serve the locally published bundles. The production transparency
service still needs a live runtime ledger layer that ingests and publishes:

- moderation action summaries;
- appeal outcomes and deposit disposition summaries;
- GAR enforcement receipts and proof-token issuance logs;
- evidence access summaries once the evidence viewer exists;
- privacy-preserving aggregate metrics with parameter metadata;
- cycle headers, entry roots, inclusion proofs, and publisher signatures.

Ledger entries must continue to be sorted and hashed deterministically, encoded
with Norito, and anchored to the Governance DAG. Any public API or explorer must
verify the same canonical payloads used for publication.

## Target Runtime Services

| Component | Responsibility | Local state |
|-----------|----------------|-------------|
| Event ingestor | Consumes moderation, GAR, appeal, proof-token issuance, legal-hold/redaction notice, and evidence-viewer events. | Local generic source-entry intake, concrete GAR/moderation/appeal source adapters, canonical-authenticated Torii source-entry ingestion for those concrete payloads plus public legal-hold/redaction/evidence-access summaries, canonical-authenticated Torii proof-token issuance feed ingestion, automatic local moderation/appeal source recording, signed `SFGT` proof-token issuance frame ingest, client/CLI source-entry and proof-token issuance submission tooling, and payload-free source-entry/proof-token issuance producer canary evidence tooling are shipped; deployed GAR/moderation/appeal/legal-hold/redaction/evidence-viewer/proof-token issuance service producers and captured rollout evidence remain open. |
| Ledger builder | Builds cycle headers, entry roots, proofs, and publisher signatures. | V1 payload/proof/publication helpers shipped in the data model; local source-entry cycle builder, local node publication to filesystem/CAR, signed runtime DAG external payloads, payload-free publication readback canary tooling, and the rollout evidence verifier are shipped; deployed anchoring and captured service rollout evidence remain open. |
| Proof API | Serves cycle metadata, entries, inclusion proofs, proof-token issuance indexes, explorer snapshots, and token verification. | Local Torii readback for published cycles, entry proofs, proof-token issuance indexes, explorer snapshots, and proof-token verification is shipped with bounded list/explorer arrays, local verifier throttling, local browser UI route, client/CLI readback helpers, payload-free explorer canary tooling, and rollout evidence summary validation; deployed service hardening and captured public rollout evidence are not shipped. |
| Receipt explorer | Public UI for browsing cycles and verifying entries. | Local explorer snapshot API, static Torii browser UI, CLI readback bridge, `iroha sorafs transparency explorer-canary` rollout-evidence tooling, and summary-gate validation are shipped; captured deployed public rollout evidence is not shipped. |
| DP aggregator | Publishes SFM-4c privacy-safe moderation aggregates. | Canonical aggregate payloads, ledger-entry conversion, node-side cycle publication bridge, local source-event suppression/noise worker, config-backed due-cycle scheduler API, canonical-authenticated Torii source-event ingestion, a canonical-authenticated local publish-due trigger, client/CLI producer plus scheduler trigger tooling, privacy aggregate canary evidence tooling, and rollout evidence summary validation are shipped; captured deployed source-event producer and scheduler rollout evidence remains open. |

Document only the local `/v1/sorafs/transparency/*` readback,
canonical-authenticated source-entry ingest, privacy aggregate source-event
ingest, configured privacy aggregate publish-due trigger, proof-token
issuance feed, proof-token issuance-index, explorer snapshot, local explorer
UI, and proof-token verification endpoints as shipped. Do not document generic
`/v1/transparency/*` endpoints or deployed public receipt explorer rollout as
shipped until the live builder, deployment, and explorer paths exist.

## Remaining Production Gates

- Wire deployed GAR receipt, moderation validator evidence, appeal outcome,
  legal-hold/redaction notice, and future evidence-viewer audit producers into
  the shipped canonical-authenticated source-entry route, concrete adapters, and
  public notice summary intake, then capture payload-free rollout evidence with
  the shipped `iroha sorafs transparency source-entry canary --source-entry
  KIND=PATH` tooling.
- Attach deployed publisher identities, anchoring, and service rollout evidence
  around the shipped deterministic source-entry cycle builder, then capture
  payload-free readback evidence with the shipped `iroha sorafs transparency
  publication-canary` tooling.
- Wire deployed source-event producers and scheduler jobs, then capture
  operational rollout evidence with the shipped privacy aggregate canary around
  the shipped `/v1/sorafs/transparency/privacy-aggregates/source-events` route,
  `/v1/sorafs/transparency/privacy-aggregates/publish-due` trigger,
  client/CLI producer and scheduler trigger tooling,
  `NodeHandle::record_privacy_aggregate_source_event(...)`, and
  `publish_due_configured_privacy_aggregate_cycle_from_source_events(...)`.
- Finish deployed proof API hardening and capture public receipt explorer
  rollout evidence by running the shipped `iroha sorafs transparency
  explorer-canary` tooling beyond the local token-verifier throttle, bounded
  readback arrays, explorer snapshot API, and static browser UI.
- Wire deployed proof-token issuance producers and public explorer linking
  around the shipped signed-frame ingest adapter, canonical-authenticated feed,
  client/CLI submission wrapper, payload-free proof-token issuance canary, and
  readback index.
- After collecting deployed source-entry, publication, privacy aggregate,
  proof-token issuance, and explorer canary artifacts, either run
  `scripts/run_sorafs_transparency_rollout_evidence.py --torii-url URL
  --out-dir DIR ...` end to end or run
  `scripts/check_sorafs_transparency_rollout_evidence.py --evidence-dir DIR
  --summary-out PATH` over an equivalent artifact directory. Keep production
  promotion blocked unless the summary status is `ready`.
- For reviewed operator runs, copy the shape from
  `scripts/examples/sorafs_transparency_rollout_evidence.args.example` into a
  runtime-only argument file, replace every payload and cycle placeholder, and
  run the harness with `@ARGFILE --dry-run` before live submission. Do not store
  signing keys, bearer tokens, private digest keys, or raw canary response
  bodies in the argument file or rollout artifact directory.
- Add end-to-end tests for live ingest, deployed ledger publication,
  live proof-token issuance/explorer linking, deployed redaction/legal-hold feed
  wiring, and deployed scheduler stale-cycle rollout evidence.

## Validation

Current validation for the shipped foundations and V1 data-model payloads:

```sh
cargo test -p iroha_data_model gar
cargo test -p sorafs_orchestrator moderation
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-transparency-ledger cargo test -j 1 -p iroha_data_model transparency_ --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-privacy-aggregate cargo test -j 1 -p iroha_data_model privacy_aggregate --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-privacy-cycle cargo test -j 1 -p sorafs_node publish_privacy_aggregate_cycle --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-privacy-cycle cargo test -j 1 -p sorafs_node publish_privacy_aggregate_cycle_from_source_events --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-privacy-cycle cargo test -j 1 -p sorafs_node privacy_aggregate_source_event --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-privacy-stale-cycle cargo test -j 1 -p sorafs_node publish_due_privacy_aggregate_cycle_from_source_events --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-privacy-config cargo test -j 1 -p iroha_config sorafs_storage_privacy_aggregate_schedule_parses_and_clamps_cycle --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-privacy-config cargo test -j 1 -p iroha_config --test fixtures minimal_config_snapshot -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-privacy-config cargo test -j 1 -p sorafs_node publish_due_configured_privacy_aggregate_cycle --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-privacy-config cargo test -j 1 -p sorafs_node conversion_from_actual_preserves_fields --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-privacy-config cargo test -j 1 -p sorafs_node privacy_aggregate_schedule --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-transparency-ledger cargo test -j 1 -p sorafs_manifest external_payload --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-transparency-ledger cargo test -j 1 -p sorafs_node transparency_ledger --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-transparency-ledger cargo test -j 1 -p sorafs_node filesystem_publisher_appends_signed_runtime_dag_for_supported_payloads --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-transparency-ingest cargo test -j 1 -p sorafs_node transparency_ledger_source --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-transparency-adapters cargo test -j 1 -p sorafs_node concrete_source_entry --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-transparency-adapters cargo test -j 1 -p sorafs_node record_concrete_transparency_source_entries --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-transparency-adapters cargo test -j 1 -p sorafs_node node_handle_moderation_tally_publishes_appeal_finance_report_for_confirmed_deposit --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-transparency-source-api cargo test -j 1 -p iroha_torii transparency_source_entry_endpoint --lib --features app_api -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-privacy-source-api cargo test -j 1 -p iroha_torii privacy_aggregate_source_event_endpoint --lib --features app_api -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-privacy-source-api cargo test -j 1 -p iroha_torii privacy_aggregate_publish_due_endpoint --lib --features app_api -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-operator-service cargo test -j 1 -p iroha_cli transparency_publication_canary -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-client cargo test -j 1 -p iroha sorafs_transparency_privacy_aggregate --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-operator-service cargo test -j 1 -p iroha_cli transparency_privacy_aggregate -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-proof-token-feed-api cargo test -j 1 -p iroha_torii transparency_proof_token_issuance_endpoint --lib --features app_api -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-client cargo test -j 1 -p iroha sorafs_transparency_token_issuance --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-operator-service cargo test -j 1 -p iroha_cli transparency_token_issuance -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-transparency-explorer-api cargo test -j 1 -p iroha_torii transparency_explorer_snapshot --lib --features app_api -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-transparency-readback-limit cargo test -j 1 -p iroha_torii transparency_readback --lib --features app_api -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-transparency-source-api cargo test -j 1 -p iroha_torii generated_spec_includes_documented_paths --lib --features app_api -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-transparency-source-api cargo test -j 1 -p iroha_torii path_group_builders_expose_expected_routes --lib --features app_api -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-proof-token-index cargo test -j 1 -p iroha_data_model proof_token_issuance --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-proof-token-index cargo test -j 1 -p sorafs_node proof_token_issuance --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-proof-token-ingest cargo test -j 1 -p iroha_crypto proof_token --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-proof-token-ingest cargo test -j 1 -p sorafs_node proof_token --lib -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-transparency-api cargo test -j 1 -p iroha_torii transparency_ --lib --features app_api -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-proof-token-index cargo test -j 1 -p iroha_torii transparency_proof_token_issuance --lib --features app_api -- --nocapture
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-transparency-api cargo test -j 1 -p iroha_torii path_group_builders_expose_expected_routes --lib --features app_api -- --nocapture
python3 -m pytest scripts/tests/check_sorafs_transparency_rollout_evidence_test.py scripts/tests/run_sorafs_transparency_rollout_evidence_test.py
```

Add dedicated SFM-4c live ingest, deployed publication, live proof-token feed,
and explorer tests when those services land.
