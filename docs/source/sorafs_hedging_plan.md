---
title: SoraFS XOR Hedging & Billing
summary: SFM-5 target architecture and current gap status for XOR price feeds, hedging, billing statements, risk controls, and rollout.
---

# SoraFS XOR Hedging & Billing

## Status
SFM-5 now has a local deterministic SoraFS hedging/billing payload foundation in
`crates/sorafs_manifest::hedging`. The workspace ships canonical Norito/JSON
schemas for normalized XOR/USD feed samples, reference-price decisions, billing
line items, and billing statements. Pure helpers replay weighted fixed-point
reference-price aggregation, stale/rejected-feed refusal, divergence degradation
flags, exact XOR-to-USD multiplication, billing
totals, and BLAKE3 digest ids for statements and line items.
The first-release payload validators now also enforce bounded canonical text,
at most 64 uniquely identified and uniquely sourced feeds, an exact 10,000 bps
weight budget, sorted feed/reason inventories, and checked weighted arithmetic.
Statements cap account and line cardinality, sort line IDs canonically, reject
duplicate source events, bind category to debit/credit direction and metered
quantity, require the reference decision to be effective exactly at the period
end, and expose an exact predecessor/account/contiguous-period transition
validator. Digest preimages include their canonical Norito byte length. The
generated positive and adversarial fixture set was regenerated against these
rules and passes the full fixture validator.
The reference validator also accepts those payloads through
`validate_hedging_payload_bytes`, and `sorafs-validate hedging`/`billing` can
validate feed, decision, line-item, and statement files with deterministic
operator outcomes.
The source bridge surface also exposes the same reference validator through
`sorafs_reference_validate_hedging_json`, Connect C/JNI
`connect_norito_sorafs_reference_validate_hedging_json`, and Kotlin/JVM, Java
Android, and Swift SDK wrappers. The bridge source and checked C header both
declare the sole first-release ABI, version 21; packaged
native artifacts still need to be regenerated before SDK release consumption.
`scripts/check_sorafs_hedging_rollout_evidence.py` now provides the SFM-5
promotion gate for staged rollout evidence. It requires feed-collector,
reference-price, billing-cycle, statement-publication, reconciliation,
metrics/alert, native-bridge-release, and governance-approval artifacts, rejects
payload-bearing evidence including common camel-case or hyphenated secret-key
spellings, and requires at least two distinct successful staged billing cycles
before reporting `ready`. Feed-collector and reference-price artifacts also
bind `feed_count` to the unique canonical `feeds[].name` inventory, require
`accepted_feed_count` to equal `feed_count`, require coverage for the reviewed
`feed-primary`, `feed-secondary`, and `feed-tertiary` price feeds, and reject
duplicate or unknown feed entries before promotion can report ready.
Feed-collector `feed_lag_seconds` and reference-price `divergence_bps` and
`decision_lag_seconds` must be non-negative integer-unit evidence before they
can satisfy rollout ceilings; `divergence_bps` and the operator
`--max-divergence-bps` threshold are also capped at `10000`, so impossible
basis-point rates cannot satisfy promotion. Billing-cycle evidence must bind each cycle
to a valid reference-price decision id from the same rollout bundle and carry only
payload-free line-item roots, statement-bundle digests,
reconciliation digests, and per-statement digest arrays whose length matches
the signed statement count, plus `policy_digest_hex` for the billing policy
that priced the staged cycle. Billing-cycle artifacts also require `cycle_id`
to match a reviewed lowercase `billing-cycle-*` label without non-production markers,
bind `statement_count` to the unique canonical `statements[].name` inventory
using reviewed `billing-statement-*` labels without non-production markers,
and bind `line_item_count` to the unique canonical `line_items[].name`
inventory using reviewed `billing-line-item-*` labels without non-production
markers, rejecting duplicate statement or line-item entries before promotion
can report ready.
Statement-publication, reconciliation,
metrics/alert, and governance-approval evidence must also carry the same
`statement_bundle_digest_hex`/`reconciliation_digest_hex` tuple as a valid
staged billing cycle in the same rollout bundle, and governance approval
`policy_digest_hex` must match a valid billing-cycle policy digest. The
aggregate production-readiness gate now derives cycle tuple and policy digest
sets from `valid_billing_cycles`: `valid_cycle_bindings` must match those cycle
tuples, `valid_policy_digests` must match those cycle policies, and every
billing-cycle `reference_decision_id_hex` must appear in
`valid_reference_decision_ids` before final promotion can report ready. It also
rechecks the lane-proven bound artifacts before final promotion:
cycle-bound artifact fingerprints must match `valid_cycle_bindings`, and
policy-bound artifact fingerprints must match `valid_policy_digests`. The
hedging gate fail-closes when more than one valid cycle tuple or policy anchor
appears, and clears the mixed `valid_cycle_bindings` or
`valid_policy_digests` set before aggregate promotion can report ready.
Statement-publication artifacts also bind `route_count` to the unique canonical
`routes[].name` inventory and reject duplicate or unknown route entries before promotion
can report ready, require every route response to carry a lowercase
`body_blake3_hex` digest, and bind `acknowledgement_probe_count` to the
unique canonical `acknowledgement_probes` inventory using reviewed
`billing-ack-probe-*` labels without non-production markers. Reconciliation
artifacts also bind `source_count` to the
    unique canonical `sources[].name` inventory and `line_item_count` to the
    unique canonical `line_items[].name` inventory using reviewed
    `billing-line-item-*` labels without non-production markers, rejecting
    duplicate or unknown source entries and duplicate line-item entries before
    promotion can report ready. Native-bridge release
    artifacts also bind
`artifact_count` to the unique canonical `artifacts[].id` inventory, require
reviewed `hedging-native-artifact-*` labels without non-production markers, and
reject duplicate artifact entries before promotion can report ready. They also
require artifact IDs to start with reviewed native bridge family prefixes and
at least one Swift-family plus one JNI-family artifact before promotion can
report ready, matching the Swift XCFramework plus JNI bridge release evidence
shape used by the checked-in canary fixtures. Metrics/alert
artifacts also bind `metric_count` to the unique canonical `metrics` inventory
and reject duplicate or unknown metric entries before promotion can report ready.
The summary exports the sorted reviewed `metrics` inventory plus
`metric_count_values`, and the aggregate production-readiness gate requires
those fields to match the metrics/alert artifact fingerprint before final
promotion can report ready. This
prevents promotion packets from mixing statement publication, reconciliation,
dashboard, approval, or policy artifacts from different billing runs.
Reference-price, cycle-tuple, and policy-digest binding failures are recorded
on the offending artifact before required-kind validity is computed, so the
JSON summary matches the fail-closed rollout decision. The
checker supports shell-style
`@ARGFILE` inputs
so reviewed operator evidence paths can be replayed without embedding secrets
or raw billing payloads. The checker exports its required top-level payload
fields as `EVIDENCE_REQUIRED_FIELDS`, and the collection planner dry-run JSON
includes the checker-backed `evidence_contract` map for selected required
kinds.
`scripts/run_sorafs_hedging_rollout_evidence.py` now provides the matching
collection planner/runner for reviewed staged evidence. It accepts explicit
payload-free canary artifacts, supports shell-style `@ARGFILE` inputs, forwards
the gate thresholds, preflights the verifier script and output targets before
printing dry-run plans, and invokes the checker with a reproducible summary
path. The planner does not replace the missing live collector, hedging, billing,
publication, or governance services; operators must still capture those service
canary artifacts before promotion can pass.
`scripts/build_sorafs_hedging_canary.py` is a payload-free SFM-5 hedging/billing canary builder
for feed collector, reference price, billing
cycle, statement publication, reconciliation, metrics/alerts, native-bridge
release, and governance approval evidence. It takes reviewed deployment facts,
requires every positive proof claim and required
feed/line-item/route/source/metric coverage explicitly, rejects duplicate or
unknown `--verified-claim`, feed, route, source, and metric closed-set inputs
before writing, forces raw feed, statement, financial-record, response-body, and
debug-artifact inclusion flags to `false`, rejects malformed,
non-production, or duplicate `--artifact` ids for native-bridge release
canaries, rejects native-bridge release canaries with fewer than two distinct
artifact ids, requires statement-publication canaries to take explicit
`--route-body-blake3-hex` response digest evidence, emits `metric_count` matching the canonical
`metrics` inventory, rejects ungoverned hedge-execution enablement, validates
each generated artifact through the hedging/billing rollout gate, and writes
atomically without following output symlinks. Billing-cycle canaries require
reviewed `--cycle-id` labels to match the same `billing-cycle-*` production shape
enforced by the gate. Billing-cycle and governance-approval canaries require
reviewed `--policy-digest-hex` input, and billing-cycle canaries require
reviewed `--statement` labels in the `billing-statement-*` family whose unique
inventory matches `--statement-digest-hex`, and billing-cycle plus
reconciliation canaries require reviewed `--line-item` labels in the
`billing-line-item-*` family whose unique inventory matches `--line-item-count`
before locally generated evidence exercises the same policy-bound promotion
path. The builder is an evidence
packaging aid; it does not replace the missing collector service, daemonized
pricing/exposure engine, billing aggregator, statement publisher, complete
runtime service API, or native bridge release process.

This is not yet a complete production hedging and billing stack. There is still no
shipped `hedgingd`, price-feed collector service, `billingd`, statement
publisher, complete SoraFS hedging/billing service API, service-management CLI, released
native bridge artifacts, or captured staged rollout evidence that
passes the SFM-5 gate. The checked-in fixture generator, fixture manifest,
README, and generated `.to`/`.json` byte suite now define the canonical SFM-5
feed, reference-price, line-item, statement, and negative fixture set. A
checked-in Grafana dashboard plus Prometheus alert/test fixtures and telemetry
helper methods now define the hedging/billing observability contract that
deployed services must satisfy.
`HedgingPriceFeedV1.evidence_digest` remains an intrinsic evidence binding only;
the raw payload by itself does not authenticate an external feed signer. The
manifest crate now ships `SignedHedgingPriceFeedV1` and an external
`HedgingFeedTrustPolicyV1` that binds strong Ed25519 keys to exact
`(feed_id, source)` pairs, validity/freshness/skew limits, and explicit
revocations. `GovernedHedgingReferencePriceDecisionV1` retains and replays all
signed inputs, while `GovernedBillingStatementV1` prevents a statement from
downgrading its reference price to unauthenticated raw feeds. Their canonical
decoders are byte/allocation bounded and reject trailing or compressed
noncanonical Norito. `SignedHedgingFeedLedgerV1` now supplies the deterministic
collector-side high-water state: it keeps one latest signed sample per feed,
binds the external policy digest, rejects global admission-clock rollback,
per-feed observation rollback, exact replay, same-time equivocation, reused
evidence digests, malformed ordering, and tampered restart checkpoints. Updates
replace the prior per-feed high-water mark, so the checkpoint stays bounded for
an indefinitely running collector; every governed decision still retains its
complete signed input set for immutable audit history. Deployment admission
must still remain disabled until the
collector, hedging daemon, and statement publisher load the external policy and
accept only these governed wrappers at their runtime boundaries.

The embedded `sorafs_node` runtime now supplies that local governed boundary.
When `hedging_feed_trust_policy_path` names a secure canonical policy file, the
node accepts bounded exact-canonical `SignedHedgingPriceFeedV1` envelopes,
persists one replay-validated latest high-water mark per feed in
`economics/signed-hedging-feeds.to`, restores it on restart, and derives
governed reference-price decisions only from those retained signed samples.
Pre-commit persistence failures roll memory back; uncertain post-rename
durability disables further durable mutation. Successful admission and decision
derivation populate the existing feed-lag, reference-price, and divergence
metric families. This is an in-process trusted boundary, not a collector or
collector service. Torii now exposes its minimum authenticated operator surface:
`POST /v1/sorafs/economics/hedging/feeds`,
`GET /v1/sorafs/economics/status`, and
`GET /v1/sorafs/economics/hedging/reference`. Every route requires exact
X-Iroha canonical request authentication plus the
`sorafs_economics_operator` role and returns `Cache-Control: private, no-store`.
The reference route accepts optional `effective_at_unix`,
`max_feed_age_secs`, and `max_divergence_bps` parameters, rejects duplicate or
unknown parameters, defaults the age ceiling to the configured trust-policy
maximum, and derives only from the durable latest signed high-water marks.
These routes do not fetch feeds, manage secrets, execute hedges, aggregate
billing events, or publish statements.

Implemented adjacent foundations include SoraFS reserve quote/ledger tooling,
DA rent/bonus telemetry, reserve ledger digest dashboards, generic subscription
billing code, and generic oracle/feed tests. Those pieces do not yet constitute
the SoraFS hedging or statement service pipeline described here. The sections
below distinguish the shipped local payload/math foundation from the remaining
runtime implementation.

## Goals & Scope
- Maintain a resilient XOR/USD reference price for SoraFS billing and economic reporting.
- Generate dual-quoted billing statements in XOR and USD for providers and buyers.
- Provide APIs, CLI tooling, and alerts for escrow runway, exposure, invoice reconciliation, and statement acknowledgement.
- Keep decisions auditable through signed Norito payloads, deterministic logs, and governance evidence.

## Target Architecture
| Component | Responsibility | Current workspace status |
|-----------|----------------|--------------------------|
| Hedging engine | Aggregate price feeds, derive the reference XOR/USD rate, track exposure, and optionally execute hedges. | Local pure reference-price helpers plus the durable `sorafs_node` governed signed-feed high-water runtime, restart checkpoint, metric emission, and authenticated Torii admission/status/reference routes are shipped; daemon, exposure tracking, collector automation, and hedge execution are not shipped. |
| Price feed collectors | Fetch primary/secondary/tertiary feeds and normalize them into signed price payloads. | Not shipped for SoraFS hedging. |
| Billing aggregator | Consume settlement, rent, egress, fee, and penalty events and produce account accruals. | Local line-item and statement builders are shipped; event ingestion and accrual service are not shipped. |
| Statement publisher | Store, sign, publish, notify, and track acknowledgements for statements. | Not shipped. |
| Alerting service | Monitor feed divergence, escrow runway, exposure limits, and statement failures. | Checked-in Grafana/Prometheus fixtures and telemetry helpers are shipped; the embedded node emits feed lag/reference/divergence when its governed APIs are invoked, while always-on collector emission and service management are not shipped. |

## Target Price Feeds And Decisions
- Primary feed: governance-approved on-chain XOR/USD or XOR/stablecoin TWAP.
- Secondary feed: independent market feed or synthetic pair.
- Tertiary feed: internal market/orderbook-implied sanity check once SFM-2 exists.
- Implemented locally: reject stale/rejected feeds before decision construction
  and mark the decision degraded on feed divergence or degraded collector status.
- Implemented locally: use deterministic integer weighted-average math for
  aggregation and replay.
- Implemented locally: record every decision as a versioned Norito payload with
  feed inputs, weights, status, evidence digests, degradation reasons, and a
  BLAKE3 decision id.

Automated hedge execution must remain off until governance approves venues,
keys, limits, failover rules, and reconciliation evidence.

## Target Billing Pipeline
- Event sources: orderbook settlement receipts, reserve/rent ledgers, egress accounting, orchestrator fees, provider incentives, and governance penalties.
- Hourly accruals update account-level usage and projected escrow runway.
- Implemented locally: weekly/ad-hoc statements finalize line items,
  adjustments, XOR totals, USD equivalents, reference-price binding, and due
  dates.
- Implemented locally: statement payloads are Norito-first; PDF/email delivery
  remains a presentation layer over the signed payload.
- Statement hashes should be publishable into governance evidence once the governance DAG pipeline is available.

## Target APIs And CLI
The minimum authenticated local economics operator routes described above are
shipped. No complete hedging or billing service routes under
`/v1/sorafs/hedging` or `/v1/sorafs/billing` are currently shipped. The
intended full service API surface is:

- Latest XOR/USD reference price and feed status.
- Hedging status, inventory, and exposure.
- Billing statement list, fetch, acknowledgement, and accrual queries.
- Escrow balance/runway queries.
- Billing and hedging configuration inspection.

Implemented local validator CLI: `sorafs-validate hedging` validates Norito
feed, reference-price decision, billing-line, and statement payloads. Target
runtime CLI helpers should still mirror the future service routes for
price/status inspection, statement download, escrow inspection, and
acknowledgement. They should reuse the existing SoraFS CLI conventions and
signed client configuration.

Implemented source SDK/FFI bridge: Rust C FFI, Connect C/JNI, Kotlin/JVM, Java
Android, and Swift callers can pass the same four hedging/billing payload
selectors and receive the canonical reference-validator JSON outcome.

## Target Observability
The checked-in SFM-5 observability pack includes
`dashboards/grafana/sorafs_hedging_billing.json`,
`dashboards/alerts/sorafs_hedging_billing_rules.yml`, and
`dashboards/alerts/tests/sorafs_hedging_billing_rules.test.yml`. The telemetry
registry exposes helper methods for the dashboard/alert metric families below,
but runtime services still need to call them from the feed, hedging, and billing
jobs:

- XOR/USD reference price and feed lag.
- Feed divergence and exposure drift.
- Statement generation count, failures, and overdue acknowledgements.
- Escrow runway by account type.

Additional service-level target metrics, including decision result counters,
statement latency, hedge inventory, and billing line items by category, remain
runtime implementation work.

The alert pack covers feed divergence, primary feed staleness, exposure drift,
statement generation failure, acknowledgement backlog, and escrow runway below
warning/critical thresholds. The Prometheus rule test pins both firing and
non-firing cases for those conditions.

The rollout evidence gate expects the deployed dashboard/alert canary to prove
the presence of the XOR/USD reference price, feed lag, statement generation,
statement failure, and escrow runway metric families while reporting no firing
critical alerts.

## Security & Governance Requirements
- Price-feed and statement payloads must be signed and replayable.
- Hedge trades require governance-approved keys, venues, inventory limits, and manual override policy.
- Financial data storage needs encryption, retention, and audit-log policy before production use.
- API scopes should distinguish read-only hedging, billing read, billing management, and treasury operations.
- Production behavior must be configured through `iroha_config`, not environment variables.

## Testing Strategy
Required before rollout:
- Implemented locally: unit tests for feed validation, fixed-point aggregation,
  divergence degradation, billing line items, statement totals, digest replay,
  Norito/JSON roundtrips, reference-validator outcomes, and validator CLI
  parser aliases.
- Integration tests with stale/divergent feed simulations and synthetic settlement/rent inputs.
- End-to-end tests from usage events through statement finalization and acknowledgement.
- Serialization and fixture tests for all statement, feed, decision, and adjustment payloads.
- Implemented locally as a generated fixture suite:
  `cargo run -p sorafs_manifest --bin generate_hedging_fixtures` emits the
  target positive and negative SFM-5 `.to`/`.json` fixture set documented in
  `fixtures/sorafs_manifest/hedging/README.md`. The target file inventory and
  validator commands are pinned in
  `fixtures/sorafs_manifest/hedging/fixture_manifest.json`, and
  `scripts/check_sorafs_hedging_fixture_manifest.py` validates the manifest in
  pre-generation mode, including accepted/rejected path and reviewed
  `negative_case` contracts, or fails closed on missing/mismatched generated
  bytes in full mode. Full mode also runs the pinned `sorafs-validate hedging`
  command contract without shell execution and compares each generated payload
  against the manifest's accepted or rejected outcome, verifies the
  kind-specific top-level and nested JSON sidecar field set, checks V1
  versioning, duplicate nested ids, account-id hex binding, and statement
  timestamp ordering, enforces even-length lowercase hex payload mirrors,
  positive prices and timestamps, canonical signed-512 exact-decimal amount
  strings (at most 28 fractional digits generally and nine for XOR), canonical
  unsigned `u128` metered-quantity strings, and bounded basis-point fields, and rejects extra
  generated `.to` or `.json` files that are not pinned by the manifest. The
  generated positive and negative fixture byte suite is checked in under
  `fixtures/sorafs_manifest/hedging/`, and the rollout contract pins those
  files to the manifest inventory so future deletions or unmanifested fixture
  drift fail closed.
- Reconciliation tests comparing generated statements with underlying ledger and settlement sources.
- Rollout evidence collection with
  `scripts/run_sorafs_hedging_rollout_evidence.py
  @scripts/examples/sorafs_hedging_rollout_collection.args.example`, followed
  by validation with
  `scripts/check_sorafs_hedging_rollout_evidence.py
  @scripts/examples/sorafs_hedging_rollout_evidence.args.example`. Production
  promotion remains blocked unless the summary status is `ready`, including at
  least two distinct staged billing cycles whose reference-decision ids match a
  valid reference-price artifact in the same evidence bundle. The same
  promotion contract now requires staged billing-cycle `policy_digest_hex`
  values to anchor governance approval and binds `statement_count` to reviewed
  `statements[].name` inventory using `billing-statement-*` labels without
  non-production markers and `line_item_count` to reviewed `line_items[].name`
  inventory using `billing-line-item-*` labels without non-production markers.
  Feed canaries also bind `feed_count` to
  reviewed `feeds[].name` inventory, require `accepted_feed_count` to match,
  and require the reviewed `feed-primary`, `feed-secondary`, and `feed-tertiary`
  price feeds while rejecting unknown feed names.
  Hedging/billing payload-safety artifacts must explicitly set
  `payload_bytes_included`, `response_bodies_included`, `degraded`,
  `statement_bodies_included`, `raw_financial_records_included`,
  `critical_alerts_firing`, and `debug_artifacts` to `false` before promotion
  can report ready.
  Statement-publication canaries bind `route_count` to reviewed
  `routes[].name` inventory, reject unknown route names, and require each
  generated route record to carry the reviewed `--route-body-blake3-hex`
  response digest.
  Statement-publication canaries bind `acknowledgement_probe_count` to reviewed
  `billing-ack-probe-*` `acknowledgement_probes` inventory and reject duplicate
  or non-production acknowledgement probes.
  Reconciliation canaries bind both `source_count` to reviewed `sources[].name`
  inventory and `line_item_count` to reviewed `line_items[].name` inventory
  using `billing-line-item-*` labels without non-production markers, rejecting
  unknown source names. Metrics canaries reject unknown metric names outside the
  reviewed alert inventory.
  The collection
  planner's dry-run JSON includes the checker-backed `evidence_contract` map so operators can
  inspect the exact required fields for each requested evidence kind before
  collecting or submitting live billing artifacts. For reviewed local canary
  packaging, generate payload-free artifacts with
  `scripts/build_sorafs_hedging_canary.py
  @scripts/examples/sorafs_hedging_reference_price_canary.args.example` and
  `scripts/build_sorafs_hedging_canary.py
  @scripts/examples/sorafs_billing_cycle_canary.args.example` and
  `scripts/build_sorafs_hedging_canary.py
  @scripts/examples/sorafs_statement_publication_canary.args.example` before
  passing the generated evidence files to the rollout gate; production promotion still
  requires at least two distinct billing-cycle artifacts.

## Rollout Status
- Done: target requirements are documented; adjacent reserve, DA rent telemetry,
  and generic billing/oracle foundations exist; local SoraFS hedging/feed,
  reference-price decision, billing-line, and billing-statement Norito payloads
  plus deterministic math/tests and reference-validator/CLI coverage are
  shipped; source-level Rust C FFI, Connect C/JNI, Kotlin/JVM, Java Android,
  and Swift bridge wrappers are shipped; the SFM-5 rollout evidence gate and
  collection planner with dry-run evidence-contract export plus operator
  argument-file examples are shipped; payload-free canary builder support for
  all SFM-5 evidence kinds is shipped; the
  deterministic hedging/billing fixture generator, fixture manifest, and
  fixture README are shipped; the fixture manifest checker and focused tests are
  shipped, including accepted/rejected validator outcome enforcement for full
  generated-byte checks, nested JSON sidecar shape/value validation, and exact
  generated-file inventory rejection; the generated fixture byte files are
  checked in and pinned by the rollout contract; the
  checked-in Grafana dashboard and Prometheus alert/test fixtures define the
  runtime observability contract, and
  `iroha_telemetry::Metrics` exposes helper methods for those metric families.
  The gate
  rejects staged billing cycles that omit digest roots, carry a statement
  digest count that does not match the signed statement count, or reference a
  missing/invalid reference-price decision; it also publishes valid
  billing-cycle policy digests as `valid_policy_digests` and rejects governance
  approval artifacts whose `policy_digest_hex` is not anchored to one of them.
  The aggregate production-readiness gate now derives cycle tuple and policy
  digest sets from `valid_billing_cycles`: `valid_cycle_bindings` must match
  those cycle tuples, `valid_policy_digests` must match those cycle policies,
  and every billing-cycle `reference_decision_id_hex` must appear in
  `valid_reference_decision_ids` before final promotion can report ready. It
  also rechecks cycle-bound and policy-bound artifact fingerprints against
  `valid_cycle_bindings` and `valid_policy_digests`. The hedging gate
  fail-closes when more than one valid cycle tuple or policy anchor appears,
  and clears the mixed `valid_cycle_bindings` or `valid_policy_digests` set
  before aggregate promotion can report ready.
- Remaining: implement collector service, daemonized pricing/exposure engine,
  billing aggregator, statement publisher, signed exposure/billing/statement
  APIs beyond the local economics operator boundary, runtime CLI helpers,
  always-on service emission of the checked-in metric families, released
  native bridge artifacts, reconciliation tests, governance approval flow, and
  at least two successful staged billing cycles whose evidence passes the SFM-5
  gate.

The runner validates the schema-closed collection-plan envelope before printing dry-run JSON or executing the verifier.
The shared runner plan guard also rejects non-canonical nested required-kind,
threshold, external-evidence, evidence-contract, and command-step shapes before
dry-run output or verifier execution.
