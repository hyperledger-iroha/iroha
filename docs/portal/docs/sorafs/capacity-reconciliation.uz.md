---
lang: uz
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

“Yo‘l xaritasi” bandi **SF-2c** g‘aznachilik sig‘im to‘lovi daftarini tasdiqlashni talab qiladi.
har kecha bajariladigan XOR transferlariga mos keladi. dan foydalaning
solishtirish uchun `scripts/telemetry/capacity_reconcile.py` yordamchisi
`/v2/sorafs/capacity/state` bajarilgan transfer to'plamiga qarshi surat va
Alertmanager uchun Prometheus matn fayli ko'rsatkichlarini chiqaradi.

## Old shartlar
- Imkoniyat holatining surati (`fee_ledger` yozuvlari) Torii dan eksport qilindi.
- Xuddi shu oyna uchun daftar eksporti (`provider_id_hex` bilan JSON yoki NDJSON,
  `kind` = hisob-kitob/jarima va `amount_nano`).
- Agar ogohlantirishlarni istasangiz, node_exporter matn fayli kollektoriga yo'l.

## Runbook
```bash
python3 scripts/telemetry/capacity_reconcile.py \
  --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
  --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
  --label nightly-capacity \
  --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
  --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
```

- Chiqish kodlari: toza o'yinda `0`, hisob-kitoblar/jarimalar etishmayotganda `1`
  yoki ortiqcha to'langan, noto'g'ri kirishlar bo'yicha `2`.
- JSON xulosasini + xeshlarni xazina paketiga biriktiring
  `docs/examples/sorafs_capacity_marketplace_validation/`.
- `.prom` fayli matnli fayl kollektoriga tushganda, ogohlantirish
  `SoraFSCapacityReconciliationMismatch` (qarang
  `dashboards/alerts/sorafs_capacity_rules.yml`) yo'qolganda yonadi,
  ortiqcha to'langan yoki kutilmagan provayder o'tkazmalari aniqlangan.

## Chiqishlar
- Hisob-kitoblar va jarimalar bo'yicha farqli provayderlarning holati.
- o'lchovlar sifatida eksport qilingan jami:
  - `sorafs_capacity_reconciliation_missing_total{kind}`
  - `sorafs_capacity_reconciliation_overpaid_total{kind}`
  - `sorafs_capacity_reconciliation_unexpected_transfers_total`
  - `sorafs_capacity_reconciliation_expected_nano{kind}`
  - `sorafs_capacity_reconciliation_actual_nano{kind}`

## Kutilayotgan diapazonlar va bardoshlik
- Kelishuv aniq: kutilgan va haqiqiy hisob-kitob/jarima nanoslari nol bardoshlik bilan mos kelishi kerak. Har qanday nolga teng bo'lmagan farq sahifa operatorlari bo'lishi kerak.
- CI sig'im to'lovi daftariga (test `capacity_fee_ledger_30_day_soak_deterministic`) `71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1` gacha bo'lgan 30 kunlik singdirish dayjestini o'rnatadi. Dijestni faqat narxlash yoki sovutish semantikasi o'zgarganda yangilang.
- ho'llash profilida (`penalty_bond_bps=0`, `strike_threshold=u32::MAX`) jarimalar nol darajasida qoladi; ishlab chiqarish faqat foydalanish/ish vaqti/PoR qavatlari buzilganda jarimalar chiqarishi va ketma-ket slashlardan oldin sozlangan sovutish vaqtiga rioya qilishi kerak.