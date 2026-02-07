---
lang: kk
direction: ltr
source: docs/fraud_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4ac4c98cc4aa6ab0c34e58e6428d0ee33eb9a0c3fdad9e6958bdc75f2a48dc66
source_last_modified: "2026-01-22T16:26:46.488648+00:00"
translation_last_reviewed: 2026-02-07
---

# Fraud Governance Playbook

This document summarizes the scaffolding required for the PSP fraud stack while
full microservices and SDKs are under active development. It captures
expectations for analytics, auditor workflows, and fallback procedures so that
upcoming implementations can plug into the ledger safely.

## Services Overview

1. **API Gateway** – receives synchronous `RiskQuery` payloads, forwards them to
   feature aggregation, and relays `FraudAssessment` responses back to ledger
   flows. High availability (active-active) is required; use regional pairs with
   deterministic hashing to avoid request skew.
2. **Feature Aggregation** – composes feature vectors for scoring. Emit
   `FeatureInput` hashes only; sensitive payloads stay off-chain. Observability
   must publish latency histograms, queue depth gauges, and replay counters per
   tenant.
3. **Risk Engine** – evaluates rules/models and produces deterministic
   `FraudAssessment` outputs. Ensure rule execution order is stable and capture
   audit logs per assessment ID.

## Analytics & Model Promotion

- **Anomaly Detection**: maintain a streaming job that flags deviations in
  decision rates per tenant. Feed alerts into the governance dashboard and store
  summaries for quarterly reviews.
- **Graph Analysis**: run nightly graph traversals on relational exports to
  identify collusion clusters. Export findings into the governance portal via
  `GovernanceExport` with references to supporting evidence.
- **Feedback Ingestion**: curate manual review outcomes and chargeback reports.
  Convert them into feature deltas and incorporate them into training datasets.
  Publish ingestion status metrics so the risk team can spot stalled feeds.
- **Model Promotion Pipeline**: automate candidate evaluation (offline metrics,
  canary scoring, rollback readiness). Promotions should emit a signed
  `FraudAssessment` sample set and update the `model_version` field in
  `GovernanceExport`.

## Auditor Workflow

1. Snapshot the latest `GovernanceExport` and verify the `policy_digest` matches
   the manifest provided by the risk team.
2. Validate that rule aggregates reconcile with ledger-side decision totals for
   the sampled window.
3. Review the anomaly detection and graph analysis reports for outstanding
   issues. Document escalations and expected remediation owners.
4. Sign and archive the review checklist. Store the Norito-encoded artifacts in
   the governance portal for reproducibility.

## Fallback Playbooks

- **Engine Outage**: if the risk engine is unavailable for more than 60 seconds,
  the gateway should flip into review-only mode, issuing `AssessmentDecision::Review`
  for all requests and alerting operators.
- **Telemetry Gap**: when metrics or traces fall behind (missing for 5 minutes),
  halt automatic model promotions and notify the on-call engineer.
- **Model Regression**: if post-deployment feedback indicates elevated fraud
  losses, roll back to the previous signed model bundle and update the roadmap
  with corrective actions.

## Data-Sharing Agreements

- Maintain jurisdiction-specific appendices covering retention, encryption, and
  breach notification SLAs. Partners must sign the appendix before receiving
  `FraudAssessment` exports.
- Document data minimization practices for each integration (e.g., hashing
  account identifiers, truncating card numbers).
- Refresh agreements annually or whenever regulatory requirements change.

## API Schemas

The gateway now exposes concrete JSON envelopes that map one-to-one to the
Norito types implemented in `crates/iroha_data_model::fraud`:

- **Risk intake** – `POST /v1/fraud/query` accepts the `RiskQuery` schema:
  - `query_id` (`[u8; 32]`, hex encoded)
  - `subject` (`AccountId`, canonical IH58 literal; optional `@<domain>` hint or alias)
  - `operation` (tagged enum matching `RiskOperation`; the JSON `type`
    discriminator mirrors the enum variant)
  - `related_asset` (`AssetId`, optional)
  - `features` (array of `{ key: String, value_hash: hex32 }` mapped from
    `FeatureInput`)
  - `issued_at_ms` (`u64`)
  - `context` (`RiskContext`; carries `tenant_id`, optional `session_id`,
    optional `reason`)
- **Risk decision** – `POST /v1/fraud/assessment` consumes the
  `FraudAssessment` payload (also mirrored in the governance exports):
  - `query_id`, `engine_id`, `risk_score_bps`, `confidence_bps`,
    `decision` (`AssessmentDecision` enum), `rule_outcomes`
    (array of `{ rule_id, score_delta_bps, rationale? }`)
  - `generated_at_ms`
  - `signature` (optional base64 wrapping the Norito-encoded assessment)
- **Governance export** – `GET /v1/fraud/governance/export` returns the
  `GovernanceExport` structure when the `governance` feature is enabled, bundling
  active parameters, the latest enactment, model version, policy digest, and the
  `DecisionAggregate` histogram.

Round-trip tests in `crates/iroha_data_model/src/fraud/types.rs` ensure these
schemas remain binary-conformant with the Norito codec, and
`integration_tests/tests/fraud_monitoring_requires_assessment_bands.rs` exercises
the full intake/decision pipeline end-to-end.

## PSP SDK References

The following language stubs track the PSP-facing integration examples:

- **Rust** – `integration_tests/tests/fraud_monitoring_requires_assessment_bands.rs`
  uses the workspace `iroha` client to craft `RiskQuery` metadata and validate
  admission failures/successes.
- **TypeScript** – `docs/source/governance_api.md` documents the REST surface
  consumed by the lightweight Torii gateway used in the PSP demo dashboard; the
  scripted client lives in `scripts/ci/schedule_fraud_scoring.sh` for smoke
  drills.
- **Swift & Kotlin** – the existing SDKs (`IrohaSwift` and
  `crates/iroha_cli/docs/multisig.md` references) expose the Torii metadata
  hooks needed to attach `fraud_assessment_*` fields. PSP-specific helpers are
  tracked under the “Fraud & Telemetry Governance Loop” milestone in
  `status.md` and reuse those SDKs’ transaction builders.

These references will be kept in sync with the microservice gateway so PSP
implementers always have an up-to-date schema and sample code path for each
supported language.
