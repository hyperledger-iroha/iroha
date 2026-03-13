---
lang: mn
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

Замын зургийн **SF-2c** зүйл нь төрийн сан хүчин чадлын төлбөрийн дэвтэрийг нотлохыг үүрэг болгосон.
шөнө бүр хийгддэг XOR шилжүүлэгтэй таарч байна. -г ашиглана уу
харьцуулах `scripts/telemetry/capacity_reconcile.py` туслах
Гүйцэтгэсэн шилжүүлгийн багцын эсрэг `/v2/sorafs/capacity/state` агшин зуурын зураг болон
Alertmanager-д зориулсан Prometheus текст файлын хэмжигдэхүүнийг ялгаруулна.

## Урьдчилсан нөхцөл
- Torii-с экспортлогдсон чадавхийн төлөвийн агшин зуурын зураг (`fee_ledger` оруулгууд).
- Нэг цонхонд зориулсан дэвтэр экспорт хийх (`provider_id_hex`-тай JSON эсвэл NDJSON,
  `kind` = төлбөр тооцоо/торгууль, мөн `amount_nano`).
- Хэрэв танд анхааруулга өгөхийг хүсвэл node_exporter текст файл цуглуулагч руу очих зам.

## Runbook
```bash
python3 scripts/telemetry/capacity_reconcile.py \
  --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
  --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
  --label nightly-capacity \
  --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
  --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
```

- Гарах кодууд: цэвэр тоглолт дээр `0`, тооцоо/торгууль дутуу үед `1`
  эсвэл илүү төлсөн, хүчингүй оролт дээр `2`.
- JSON хураангуй + хэшийг төрийн сангийн багцад хавсаргана уу
  `docs/examples/sorafs_capacity_marketplace_validation/`.
- `.prom` файл текст файл цуглуулагч руу буухад дохиолол гарч ирнэ.
  `SoraFSCapacityReconciliationMismatch` (харна уу
  `dashboards/alerts/sorafs_capacity_rules.yml`) алга болсон үед гал асаах,
  илүү төлсөн, эсвэл гэнэтийн үйлчилгээ үзүүлэгч шилжүүлэг илэрсэн.

## Гаралт
- Төлбөр тооцоо, торгуулийн зөрүүтэй үйлчилгээ үзүүлэгчийн статус.
- хэмжигчээр экспортолсон нийт дүн:
  - `sorafs_capacity_reconciliation_missing_total{kind}`
  - `sorafs_capacity_reconciliation_overpaid_total{kind}`
  - `sorafs_capacity_reconciliation_unexpected_transfers_total`
  - `sorafs_capacity_reconciliation_expected_nano{kind}`
  - `sorafs_capacity_reconciliation_actual_nano{kind}`

## Хүлээгдэж буй хүрээ ба хүлцэл
- Эвлэрүүлэн зуучлах нь яг тодорхой байна: хүлээгдэж буй болон бодит тооцоо/торгуулийн нано хоёр нь тэвчихгүй байх ёстой. Ямар ч тэгээс ялгаатай нь хуудасны операторууд байх ёстой.
- CI хүчин чадлын хураамжийн дэвтэрт зориулсан 30 хоногийн дэвтэх дижестийг (туршилт `capacity_fee_ledger_30_day_soak_deterministic`) `71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1` хүртэл тогтооно. Үнийн болон хөргөлтийн семантик өөрчлөгдөх үед л тоймыг сэргээнэ үү.
- Норгосны профайлд (`penalty_bond_bps=0`, `strike_threshold=u32::MAX`) торгууль 0 хэвээр байна; Үйлдвэрлэл нь зөвхөн ашиглалт/ашиглалтын хугацаа/PoR-ийн давхрагыг зөрчсөн тохиолдолд торгууль ногдуулах ёстой бөгөөд дараалсан зүсэлт хийхээс өмнө тохируулсан хөргөлтийн хугацааг хүндэтгэх ёстой.