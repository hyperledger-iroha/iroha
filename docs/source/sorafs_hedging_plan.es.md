---
lang: es
direction: ltr
source: docs/source/sorafs_hedging_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4d41b7132642d1025e0566398ea92aae654f2cc0aac5a1c19e9febae0e07f43e
source_last_modified: "2026-07-02T08:57:47.791466+00:00"
translation_last_reviewed: 2026-07-02
source_mtime: 2026-07-02T08:57:47.791466+00:00
---
# SoraFS XOR Hedging & Billing

## Status
SFM-5 now has a local deterministic SoraFS hedging/billing payload foundation in
`crates/sorafs_manifest::hedging`. The workspace ships canonical Norito/JSON
schemas for normalized XOR/USD feed samples, reference-price decisions, billing
line items, and billing statements. Pure helpers replay weighted fixed-point
reference-price aggregation, stale/rejected-feed refusal, divergence degradation
flags, micro-XOR to USD-micro conversion with deterministic ceiling, billing
totals, and BLAKE3 digest ids for statements and line items.
The reference validator also accepts those payloads through
`validate_hedging_payload_bytes`, and `sorafs-validate hedging`/`billing` can
validate feed, decision, line-item, and statement files with deterministic
operator outcomes.
The source bridge surface also exposes the same reference validator through
`sorafs_reference_validate_hedging_json`, Connect C/JNI
`connect_norito_sorafs_reference_validate_hedging_json`, and Kotlin/JVM, Java
Android, and Swift SDK wrappers. The bridge source ABI is now 12; packaged
native artifacts still need to be regenerated before SDK release consumption.
`scripts/check_sorafs_hedging_rollout_evidence.py` now provides the SFM-5
promotion gate for staged rollout evidence. It requires feed-collector,
reference-price, billing-cycle, statement-publication, reconciliation,
metrics/alert, native-bridge-release, and governance-approval artifacts, rejects
payload-bearing evidence including common camel-case or hyphenated secret-key
spellings, and requires at least two distinct successful staged billing cycles
before reporting `ready`. Feed-collector and reference-price artifacts also
bind `feed_count` to the unique canonical `feeds[].name` inventory, require
`accepted_feed_count` to equal `feed_count`, and reject duplicate feed entries
before promotion can report ready. Billing-cycle evidence must bind each cycle
to a valid reference-price decision id from the same rollout bundle and carry only
payload-free line-item roots, statement-bundle digests,
reconciliation digests, and per-statement digest arrays whose length matches
the signed statement count, plus `policy_digest_hex` for the billing policy
    that priced the staged cycle. Billing-cycle artifacts also bind
    `statement_count` to the unique canonical `statements[].name` inventory and
    `line_item_count` to the unique canonical `line_items[].name` inventory,
    rejecting duplicate statement or line-item entries before promotion can report
    ready.
Statement-publication, reconciliation,
metrics/alert, and governance-approval evidence must also carry the same
`statement_bundle_digest_hex`/`reconciliation_digest_hex` tuple as a valid
staged billing cycle in the same rollout bundle, and governance approval
`policy_digest_hex` must match a valid billing-cycle policy digest.
Statement-publication artifacts also bind `route_count` to the unique canonical
`routes[].name` inventory and reject duplicate route entries before promotion
    can report ready. Reconciliation artifacts also bind `source_count` to the
    unique canonical `sources[].name` inventory and `line_item_count` to the
    unique canonical `line_items[].name` inventory, rejecting duplicate source or
    line-item entries before promotion can report ready. Native-bridge release
    artifacts also bind
`artifact_count` to the unique canonical `artifacts[].id` inventory and reject
duplicate artifact entries before promotion can report ready. This
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
for feed collector, reference price, billing cycle, statement publication,
reconciliation, metrics/alerts, native-bridge release, and governance approval
evidence. It takes reviewed deployment facts, requires every positive proof
    claim and required feed/line-item/route/source/metric coverage explicitly, forces raw
feed, statement, financial-record, response-body, and debug-artifact inclusion
flags to `false`, rejects duplicate `--artifact` ids for native-bridge release
canaries, rejects ungoverned hedge-execution enablement, validates each
generated artifact through the hedging/billing rollout gate, and writes
atomically without following output symlinks. Billing-cycle and
governance-approval canaries require reviewed `--policy-digest-hex` input, and
    billing-cycle canaries require reviewed `--statement` labels whose unique
    inventory matches `--statement-digest-hex`, and billing-cycle plus
    reconciliation canaries require reviewed `--line-item` labels whose unique
    inventory matches `--line-item-count` before locally generated evidence
    exercises the same policy-bound promotion path. The
builder is an evidence
packaging aid; it does not replace the missing collector service, daemonized
pricing/exposure engine, billing aggregator, statement publisher, runtime API,
or native bridge release process.

This is not yet a production hedging and billing stack. There is still no
shipped `hedgingd`, price-feed collector service, `billingd`, statement
publisher, SoraFS hedging/billing REST API, service-management CLI, released
native bridge artifacts, runtime service wiring that populates the
hedging/billing metric families, or captured staged rollout evidence that
passes the SFM-5 gate. The checked-in fixture generator, fixture manifest,
README, and generated `.to`/`.json` byte suite now define the canonical SFM-5
feed, reference-price, line-item, statement, and negative fixture set. A
checked-in Grafana dashboard plus Prometheus alert/test fixtures and telemetry
helper methods now define the hedging/billing observability contract that
deployed services must satisfy.

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
| Hedging engine | Aggregate price feeds, derive the reference XOR/USD rate, track exposure, and optionally execute hedges. | Local pure reference-price decision helper and reference validator are shipped; daemon, exposure tracking, and hedge execution are not shipped. |
| Price feed collectors | Fetch primary/secondary/tertiary feeds and normalize them into signed price payloads. | Not shipped for SoraFS hedging. |
| Billing aggregator | Consume settlement, rent, egress, fee, and penalty events and produce account accruals. | Local line-item and statement builders are shipped; event ingestion and accrual service are not shipped. |
| Statement publisher | Store, sign, publish, notify, and track acknowledgements for statements. | Not shipped. |
| Alerting service | Monitor feed divergence, escrow runway, exposure limits, and statement failures. | Checked-in Grafana/Prometheus fixtures and telemetry helper methods are shipped; runtime service emission and service management are not shipped. |

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
No hedging or billing routes are currently shipped. The intended API surface is:

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
  positive prices, timestamps, canonical unsigned `u128` billing
  amount/quantity strings, and bounded basis-point fields, and rejects extra
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
  `statements[].name` inventory and `line_item_count` to reviewed
  `line_items[].name` inventory. Feed canaries also bind `feed_count` to
  reviewed `feeds[].name` inventory and require `accepted_feed_count` to match.
  Reconciliation canaries bind both `source_count` to reviewed `sources[].name`
  inventory and `line_item_count` to reviewed `line_items[].name` inventory.
  The collection
  planner's dry-run JSON includes the checker-backed `evidence_contract` map so operators can
  inspect the exact required fields for each requested evidence kind before
  collecting or submitting live billing artifacts. For reviewed local canary
  packaging, generate payload-free artifacts with
  `scripts/build_sorafs_hedging_canary.py
  @scripts/examples/sorafs_hedging_reference_price_canary.args.example` and
  `scripts/build_sorafs_hedging_canary.py
  @scripts/examples/sorafs_billing_cycle_canary.args.example` before passing the
  generated evidence files to the rollout gate; production promotion still
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
- Remaining: implement collector service, daemonized pricing/exposure engine,
  billing aggregator, statement publisher, signed APIs, runtime CLI helpers,
  runtime service emission of the checked-in metric families, released native
  bridge artifacts, reconciliation tests, governance approval flow, and at least
  two successful staged billing cycles whose evidence passes the SFM-5 gate.

The runner validates the schema-closed collection-plan envelope before printing dry-run JSON or executing the verifier.
The shared runner plan guard also rejects non-canonical nested required-kind,
threshold, external-evidence, evidence-contract, and command-step shapes before
dry-run output or verifier execution.
