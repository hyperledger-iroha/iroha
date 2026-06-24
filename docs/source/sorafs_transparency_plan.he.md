---
lang: he
direction: rtl
source: docs/source/sorafs_transparency_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 953476f6124532a96eefc3117a92ab5757c3045acfbc5cab3dbb82d86fdae6dd
source_last_modified: 2026-06-24T07:58:54.966860Z
translation_last_reviewed: 2026-01-30
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
publish-due trigger, a canonical-authenticated local Torii proof-token issuance
feed, and a local SoraFS node publication path into the Governance DAG
filesystem/CAR queue, optional signed runtime DAG external payloads, and Torii
readback endpoints for locally published cycles, entry inclusion proofs,
bounded proof-token issuance indexes, a bounded local explorer snapshot, and gateway
proof-token verification. The local node can also derive and publish
proof-token issuance records from signed `SFGT` frames after verifying the
Ed25519 signer key. The local appeal-finance report, weekly rollup, and
settlement receipt dashboards also bound their returned Governance DAG source
entry arrays via `limit` while keeping full aggregate totals visible. It does
not yet ship
deployed GAR/moderation/appeal producers that call the source-entry route,
deployed aggregate source-event producers and rollout evidence, proof service
hardening, public receipt explorer UI, deployed proof-token issuance
producers/explorer linking, or deployed moderation ledger publication service
described by the original plan.

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
  provide due-cycle scheduling for the aggregate worker. The method derives a
  deterministic cycle id from the due window, skips not-due/empty/already
  published/fully suppressed windows explicitly, and publishes each due cycle at
  most once per node runtime.
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
  evaluates the configured due window, accepts privacy policy, optional noise
  seed, policy digest, previous block hash, and public aggregate metadata as
  runtime-only request material, and returns structured
  published/skipped/already-published outcomes with cycle hashes when a cycle is
  published.
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
  `limit` with a default of 50 and a max of 500.
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
- Torii exposes `/v1/sorafs/transparency/tokens/verify` for local verification
  of `SFGT` gateway proof-token frames. Callers supply the token and Ed25519
  verifying key, plus optional runtime-only blinded-digest key/evidence digest
  material when they need private evidence binding checks. The response reports
  signature, digest, expiry, and not-before status without persisting digest-key
  material. The verifier honors configured Torii API-token enforcement and uses
  the shared proof API limiter, returning `429` plus `Retry-After` when a caller
  exceeds the configured proof request budget.

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
| Event ingestor | Consumes moderation, GAR, appeal, proof-token issuance, legal-hold/redaction notice, and evidence-viewer events. | Local generic source-entry intake, concrete GAR/moderation/appeal source adapters, canonical-authenticated Torii source-entry ingestion for those concrete payloads plus public legal-hold/redaction/evidence-access summaries, canonical-authenticated Torii proof-token issuance feed ingestion, automatic local moderation/appeal source recording, and signed `SFGT` proof-token issuance frame ingest are shipped; deployed GAR/moderation/appeal/legal-hold/redaction/evidence-viewer/proof-token issuance service producers and rollout evidence are not shipped. |
| Ledger builder | Builds cycle headers, entry roots, proofs, and publisher signatures. | V1 payload/proof/publication helpers shipped in the data model; local source-entry cycle builder, local node publication to filesystem/CAR, and signed runtime DAG external payloads are shipped; deployed anchoring is not shipped. |
| Proof API | Serves cycle metadata, entries, inclusion proofs, proof-token issuance indexes, explorer snapshots, and token verification. | Local Torii readback for published cycles, entry proofs, proof-token issuance indexes, explorer snapshots, and proof-token verification is shipped with bounded list/explorer arrays and local verifier throttling; deployed service hardening and public UI integration are not shipped. |
| Receipt explorer | Public UI for browsing cycles and verifying entries. | Local explorer snapshot API is shipped; public UI and deployed rollout evidence are not shipped. |
| DP aggregator | Publishes SFM-4c privacy-safe moderation aggregates. | Canonical aggregate payloads, ledger-entry conversion, node-side cycle publication bridge, local source-event suppression/noise worker, config-backed due-cycle scheduler API, canonical-authenticated Torii source-event ingestion, and a canonical-authenticated local publish-due trigger are shipped; deployed source-event producers, deployed scheduler jobs, and rollout evidence are not shipped. |

Document only the local `/v1/sorafs/transparency/*` readback,
canonical-authenticated source-entry ingest, privacy aggregate source-event
ingest, configured privacy aggregate publish-due trigger, proof-token
issuance feed, proof-token issuance-index, explorer snapshot, and proof-token
verification endpoints as shipped. Do not document generic
`/v1/transparency/*` endpoints or a public receipt explorer UI as shipped until
the live builder, deployment, and explorer paths exist.

## Remaining Production Gates

- Wire deployed GAR receipt, moderation validator evidence, appeal outcome,
  legal-hold/redaction notice, and future evidence-viewer audit producers into
  the shipped canonical-authenticated source-entry route, concrete adapters, and
  public notice summary intake.
- Attach deployed publisher identities, anchoring, and service rollout evidence
  around the shipped deterministic source-entry cycle builder.
- Wire deployed source-event producers and operational rollout evidence around
  the shipped `/v1/sorafs/transparency/privacy-aggregates/source-events` route,
  `/v1/sorafs/transparency/privacy-aggregates/publish-due` trigger,
  `NodeHandle::record_privacy_aggregate_source_event(...)`, and
  `publish_due_configured_privacy_aggregate_cycle_from_source_events(...)`.
- Finish deployed proof API hardening and public receipt explorer UI integration
  beyond the shipped local token-verifier throttle, bounded readback arrays, and
  explorer snapshot API.
- Wire deployed proof-token issuance producers and public explorer linking
  around the shipped signed-frame ingest adapter, canonical-authenticated feed,
  and readback index.
- Add end-to-end tests for live ingest, deployed ledger publication,
  live proof-token issuance/explorer linking, deployed redaction/legal-hold feed
  wiring, and stale-cycle handling.

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
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-privacy-cycle cargo test -j 1 -p sorafs_node publish_due_privacy_aggregate_cycle_from_source_events --lib -- --nocapture
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
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-proof-token-feed-api cargo test -j 1 -p iroha_torii transparency_proof_token_issuance_endpoint --lib --features app_api -- --nocapture
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
```

Add dedicated SFM-4c live ingest, deployed publication, live proof-token feed,
and explorer tests when those services land.
