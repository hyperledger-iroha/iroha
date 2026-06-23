---
lang: zh-hant
direction: ltr
source: docs/source/sorafs_transparency_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 72c55f773aae184a828641c347fa9ff1863f9568ecb48078c6dde767e2a036f7
source_last_modified: "2025-12-29T18:16:36.181496+00:00"
translation_last_reviewed: 2026-02-07
title: Transparency Dashboards & Enforcement Receipts
summary: SFM-4c implementation status for GAR receipts, moderation dashboards, privacy metrics, and remaining transparency ledger services.
---

---
title: Transparency Dashboards & Enforcement Receipts
summary: SFM-4c implementation status for GAR receipts, moderation dashboards, privacy metrics, and remaining transparency ledger services.
---

# Transparency Dashboards & Enforcement Receipts

## Current Status

SFM-4c is partially implemented. The repository contains GAR enforcement receipt
types, moderation validation evidence, honey-audit reports, moderation and
privacy dashboards, and SoraNet privacy metrics. It does not yet ship the SoraFS
transparency ledger builder, public proof API, receipt explorer, or moderation
ledger publication service described by the original plan.

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

## Target Ledger Model

The production transparency service still needs a canonical ledger layer that
publishes:

- moderation action summaries;
- appeal outcomes and deposit disposition summaries;
- GAR enforcement receipts and proof-token issuance logs;
- evidence access summaries once the evidence viewer exists;
- privacy-preserving aggregate metrics with parameter metadata;
- cycle headers, entry roots, inclusion proofs, and publisher signatures.

Ledger entries must be sorted and hashed deterministically, encoded with Norito,
and anchored to the Governance DAG. Any public API or explorer must verify the
same canonical payloads used for publication.

## Target Runtime Services

| Component | Responsibility | Local state |
|-----------|----------------|-------------|
| Event ingestor | Consumes moderation, GAR, appeal, and evidence-viewer events. | Not shipped as SFM-4c service. |
| Ledger builder | Builds cycle headers, entry roots, proofs, and publisher signatures. | Not shipped. |
| Proof API | Serves cycle metadata, entries, inclusion proofs, and token verification. | Not shipped. |
| Receipt explorer | Public UI for browsing cycles and verifying entries. | Not shipped. |
| DP aggregator | Publishes SFM-4c privacy-safe moderation aggregates. | Adjacent SoraNet privacy metrics exist; SFM-4c aggregator not shipped. |

Do not document `/v1/transparency/*` endpoints or a public receipt explorer as
shipped until the builder, API, and publication path exist.

## Remaining Production Gates

- Define `ModerationLedgerBlockV1`, `ModerationLedgerEntryV1`, and
  `ModerationLedgerProofV1` payloads in the data model with Norito roundtrip and
  canonical-hash tests.
- Implement an ingest path for GAR receipts, moderation validator evidence,
  appeal outcomes, and future evidence-viewer audit summaries.
- Implement cycle building, deterministic entry ordering, Merkle inclusion
  proofs, publisher signatures, and Governance DAG anchoring.
- Implement privacy-safe moderation aggregate publication with explicit
  epsilon/delta or suppression parameters.
- Build the public proof API and receipt explorer with rate limits and replay
  verification.
- Add end-to-end tests for ingest, ledger publication, proof verification,
  token verification, redaction/legal-hold entries, and stale-cycle handling.

## Validation

Current validation is limited to the shipped foundations:

```sh
cargo test -p iroha_data_model gar
cargo test -p sorafs_orchestrator moderation
```

Add dedicated SFM-4c ledger and API tests when the transparency builder and
proof service land.
