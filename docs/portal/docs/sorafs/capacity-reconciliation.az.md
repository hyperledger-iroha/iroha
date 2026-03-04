---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/capacity-reconciliation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e26cc8232dd7d3b392d56646fdfbf809952f017532a37aafbfde3c8cc704ae0e
source_last_modified: "2025-12-29T18:16:35.177959+00:00"
translation_last_reviewed: 2026-02-07
id: capacity-reconciliation
title: SoraFS Capacity Reconciliation
description: Nightly workflow for matching capacity fee ledgers to XOR transfer exports.
translator: machine-google-reviewed
---

Yol xəritəsi bəndi **SF-2c** xəzinədarlığın tutum haqqı kitabçasını sübut etməsini tələb edir
hər gecə həyata keçirilən XOR köçürmələrinə uyğun gəlir. istifadə edin
`scripts/telemetry/capacity_reconcile.py` ilə müqayisə etmək üçün köməkçi
`/v1/sorafs/capacity/state` icra edilmiş köçürmə dəstəsinə qarşı snapshot və
Alertmanager üçün Prometheus mətn faylı ölçülərini yayır.

## İlkin şərtlər
- Torii-dən ixrac edilmiş tutum vəziyyətinin görüntüsü (`fee_ledger` girişləri).
- Eyni pəncərə üçün kitab ixracı (`provider_id_hex` ilə JSON və ya NDJSON,
  `kind` = hesablaşma/cərimə və `amount_nano`).
- Xəbərdarlıq etmək istəyirsinizsə, node_exporter mətn faylı kollektoruna gedən yol.

## Runbook
```bash
python3 scripts/telemetry/capacity_reconcile.py \
  --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
  --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
  --label nightly-capacity \
  --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
  --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
```

- Çıxış kodları: təmiz matçda `0`, hesablaşmalar/cəzalar olmadıqda `1`
  və ya artıq ödənilmiş, etibarsız girişlərdə `2`.
- JSON xülasəsini + hashləri xəzinə paketinə əlavə edin
  `docs/examples/sorafs_capacity_marketplace_validation/`.
- `.prom` faylı mətn faylı kollektoruna düşəndə xəbərdarlıq
  `SoraFSCapacityReconciliationMismatch` (bax
  `dashboards/alerts/sorafs_capacity_rules.yml`) itkin düşəndə yanğınlar,
  həddindən artıq ödənişli və ya gözlənilməz provayder köçürmələri aşkar edilir.

## Çıxışlar
- Hesablaşmalar və cərimələr üçün fərqləri olan hər provayder statusları.
- Ölçülər kimi ixrac edilən cəmi:
  - `sorafs_capacity_reconciliation_missing_total{kind}`
  - `sorafs_capacity_reconciliation_overpaid_total{kind}`
  - `sorafs_capacity_reconciliation_unexpected_transfers_total`
  - `sorafs_capacity_reconciliation_expected_nano{kind}`
  - `sorafs_capacity_reconciliation_actual_nano{kind}`

## Gözlənilən Aralıqlar və Tolerantlıqlar
- Uzlaşma dəqiqdir: gözlənilən və faktiki hesablaşma/cəza nanosları sıfır dözümlülüklə uyğun olmalıdır. Sıfır olmayan istənilən fərq operatorları səhifələməlidir.
- CI tutum haqqı dəftəri (test `capacity_fee_ledger_30_day_soak_deterministic`) üçün 30 günlük islatmaq həzmini `71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1`-ə bağlayır. Yalnız qiymət və ya soyutma semantikası dəyişdikdə həzmi yeniləyin.
- Islatma profilində (`penalty_bond_bps=0`, `strike_threshold=u32::MAX`) cərimələr sıfırda qalır; istehsal yalnız istifadə/iş vaxtı/PoR mərtəbələri pozulduqda cərimələr buraxmalı və ardıcıl kəsiklərdən əvvəl konfiqurasiya edilmiş soyumağa riayət etməlidir.