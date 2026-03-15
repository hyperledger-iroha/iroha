---
lang: mn
direction: ltr
source: docs/source/sorafs/reports/capacity_marketplace_validation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: abd7e123052c30e77c9dabb8236c67e4aac9b09f980a7e57c6f7d17b1455a370
source_last_modified: "2025-12-29T18:16:36.124540+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Capacity Marketplace Validation
tags: [SF-2c, acceptance, checklist]
summary: Acceptance checklist covering provider onboarding, dispute workflows, and treasury reconciliation gating the SoraFS capacity marketplace general availability.
---

# SoraFS Capacity Marketplace Validation Checklist

**Review window:** 2026-03-18 → 2026-03-24  
**Programme owners:** Storage Team (`@storage-wg`), Governance Council (`@council`), Treasury Guild (`@treasury`)  
**Scope:** Provider onboarding pipelines, dispute adjudication flows, and treasury reconciliation processes required for SF-2c GA.

The checklist below must be reviewed before enabling the marketplace for external operators. Each row links to deterministic evidence (tests, fixtures, or documentation) that auditors can replay.

## Acceptance Checklist

### Provider Onboarding

| Check | Validation | Evidence |
|-------|------------|----------|
| Registry accepts canonical capacity declarations | Integration test exercises `/v1/sorafs/capacity/declare` via the app API, verifying signature handling, metadata capture, and hand-off to the node registry. | `crates/iroha_torii/src/routing.rs:7654` |
| Smart contract rejects mismatched payloads | Unit test ensures provider IDs and committed GiB fields match the signed declaration before persisting. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI emits canonical onboarding artefacts | CLI harness writes deterministic Norito/JSON/Base64 outputs and validates round-trips so operators can stage declarations offline. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Operator guide captures admission workflow and governance guardrails | Documentation enumerates declaration schema, policy defaults, the onboarding/exit runbook, and review steps for the council. | `docs/source/sorafs/storage_capacity_marketplace.md:1`,`docs/source/sorafs/capacity_onboarding_runbook.md:1` |

### Dispute Resolution

| Check | Validation | Evidence |
|-------|------------|----------|
| Dispute records persist with canonical payload digest | Unit test registers a dispute, decodes the stored payload, and asserts pending status to guarantee ledger determinism. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| CLI dispute generator matches canonical schema | CLI test covers Base64/Norito outputs and JSON summaries for `CapacityDisputeV1`, ensuring evidence bundles hash deterministically. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Replay test proves dispute/penalty determinism | Proof-failure telemetry replayed twice produces identical ledger, credit, and dispute snapshots so slashes are deterministic across peers. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Runbook documents escalation and revocation flow | Operations guide captures council workflow, evidence requirements, and rollback procedures. | `docs/source/sorafs/dispute_revocation_runbook.md:1` |

### Treasury Reconciliation

| Check | Validation | Evidence |
|-------|------------|----------|
| Ledger accrual matches 30-day soak projection | Soak test spans five providers across 30 settlement windows, diffing ledger entries against the expected payout reference. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| Ledger export reconciliation recorded nightly | `capacity_reconcile.py` compares fee ledger expectations with executed XOR transfer exports, emits Prometheus metrics, and gates treasury approval via Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Billing dashboards surface penalties and accrual telemetry | Grafana import plots GiB·hour accrual, strike counters, and bonded collateral for on-call visibility. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Published report archives soak methodology and replay commands | Report details soak scope, execution commands, and observability hooks for auditors. | `docs/source/sorafs/reports/sf2c_capacity_soak.md:1` |

## Execution Notes

Re-run the validation suite before sign-off:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Operators should regenerate onboarding/dispute request payloads with `sorafs_manifest_stub capacity {declaration,dispute}` and archive the resulting JSON/Norito bytes alongside the governance ticket.

## Sign-off Artefacts

| Artefact | Path | blake2b-256 |
|----------|------|-------------|
| Provider onboarding approval packet | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Dispute resolution approval packet | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Treasury reconciliation approval packet | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Store the signed copies of these artefacts with the release bundle and link them in the governance change record.

## Approvals

- Storage Team Lead — @storage-tl (2026-03-24)  
- Governance Council Secretary — @council-sec (2026-03-24)  
- Treasury Operations Lead — @treasury-ops (2026-03-24)
