---
lang: ja
direction: ltr
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/sorafs/reports/capacity-marketplace-validation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e551d2a854c25d7939a00f6461e01e748fbcd0c79abdb03d0fea9f2d97107c0c
source_last_modified: "2026-01-03T18:08:00+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# SoraFS Capacity Marketplace Validation Checklist

**Review window:** 2026-03-18 -> 2026-03-24  
**Programme owners:** Storage Team (`@storage-wg`), Governance Council (`@council`), Treasury Guild (`@treasury`)  
**Scope:** Provider onboarding pipelines، dispute adjudication flows، اور treasury reconciliation processes جو SF-2c GA کے لئے درکار ہیں۔

نیچے دی گئی checklist کو بیرونی operators کے لئے marketplace enable کرنے سے پہلے review کرنا لازم ہے۔ ہر row deterministic evidence (tests, fixtures, یا documentation) سے لنک کرتی ہے جسے auditors replay کر سکتے ہیں۔

## Acceptance Checklist

### Provider Onboarding

| Check | Validation | Evidence |
|-------|------------|----------|
| Registry canonical capacity declarations قبول کرتا ہے | Integration test app API کے ذریعے `/v1/sorafs/capacity/declare` چلایا جاتا ہے، signatures handling، metadata capture، اور node registry تک hand-off کو verify کرتا ہے۔ | `crates/iroha_torii/src/routing.rs:7654` |
| Smart contract mismatched payloads کو reject کرتا ہے | Unit test یقینی بناتا ہے کہ provider IDs اور committed GiB fields signed declaration کے مطابق ہوں قبل از persistence۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI canonical onboarding artefacts emit کرتا ہے | CLI harness deterministic Norito/JSON/Base64 outputs لکھتا ہے اور round-trips validate کرتا ہے تاکہ operators offline declarations تیار کر سکیں۔ | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Operator guide admission workflow اور governance guardrails کو cover کرتا ہے | Documentation declaration schema، policy defaults، اور council review steps کو enumerate کرتی ہے۔ | `../storage-capacity-marketplace.md` |

### Dispute Resolution

| Check | Validation | Evidence |
|-------|------------|----------|
| Dispute records canonical payload digest کے ساتھ persist ہوتے ہیں | Unit test dispute register کرتا ہے، stored payload decode کرتا ہے، اور pending status assert کرتا ہے تاکہ ledger determinism یقینی ہو۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| CLI dispute generator canonical schema سے match کرتا ہے | CLI test `CapacityDisputeV1` کے لئے Base64/Norito outputs اور JSON summaries cover کرتا ہے، جس سے evidence bundles deterministic hash ہوتے ہیں۔ | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Replay test dispute/penalty determinism کو ثابت کرتا ہے | Proof-failure telemetry کو دو بار replay کرنے سے ledger، credit اور dispute snapshots یکساں بنتے ہیں تاکہ slashes peers کے درمیان deterministic رہیں۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Runbook escalation اور revocation flow کو document کرتا ہے | Operations guide council workflow، evidence requirements اور rollback procedures کو capture کرتا ہے۔ | `../dispute-revocation-runbook.md` |

### Treasury Reconciliation

| Check | Validation | Evidence |
|-------|------------|----------|
| Ledger accrual 30-day soak projection سے match کرتا ہے | Soak test پانچ providers کو 30 settlement windows میں چلاتا ہے، ledger entries کو expected payout reference کے ساتھ diff کرتا ہے۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| Ledger export reconciliation ہر رات record ہوتا ہے | `capacity_reconcile.py` fee ledger expectations کو executed XOR transfer exports کے ساتھ compare کرتا ہے، Prometheus metrics emit کرتا ہے، اور Alertmanager کے ذریعے treasury approval gate کرتا ہے۔ | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Billing dashboards penalties اور accrual telemetry surface کرتے ہیں | Grafana import GiB-hour accrual، strike counters، اور bonded collateral plot کرتا ہے تاکہ on-call visibility ملے۔ | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Published report soak methodology اور replay commands archive کرتا ہے | Report soak scope، execution commands اور observability hooks کو auditors کے لئے detail کرتا ہے۔ | `./sf2c-capacity-soak.md` |

## Execution Notes

Sign-off سے پہلے validation suite دوبارہ چلائیں:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Operators کو `sorafs_manifest_stub capacity {declaration,dispute}` کے ساتھ onboarding/dispute request payloads دوبارہ generate کرنے چاہئیں اور بنے ہوئے JSON/Norito bytes کو governance ticket کے ساتھ archive کرنا چاہیے۔

## Sign-off Artefacts

| Artefact | Path | blake2b-256 |
|----------|------|-------------|
| Provider onboarding approval packet | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Dispute resolution approval packet | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Treasury reconciliation approval packet | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

ان artefacts کی signed copies کو release bundle کے ساتھ محفوظ کریں اور governance change record میں لنک کریں۔

## Approvals

- Storage Team Lead — @storage-tl (2026-03-24)  
- Governance Council Secretary — @council-sec (2026-03-24)  
- Treasury Operations Lead — @treasury-ops (2026-03-24)
