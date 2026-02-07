---
lang: ba
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

Юл картаһы әйбер **SF-2c** мандаттар, тип ҡаҙна иҫбатлай ҡәҙерле гонорар леджер
һәр төндә башҡарылған XOR күсермәләренә тап килә. Ҡулланыу
I18NI000000003X ярҙамсыһы сағыштырыу өсөн
I18NI000000004X снимок ҡаршы башҡарылған күсермә партияһы һәм
сығарыу I18NT000000000X текст файлы метрикаһы өсөн Alertmanager.

## Алдан шарттар
- Ҡыйыу дәүләт снимоктары (`fee_ledger` яҙмалары) I18NT000000001X-тан экспортҡа сыға.
- Шул уҡ тәҙрә өсөн Леджер экспорты (JSON йәки NDJSON менән `provider_id_hex`,
  `kind` = иҫәп-хисап/штраф, һәм `amount_nano`).
- node_exporter тексты йыйыусыға юл, әгәр һеҙ иҫкәртмәләр теләйһегеҙ икән.

## Ранбук
```bash
python3 scripts/telemetry/capacity_reconcile.py \
  --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
  --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
  --label nightly-capacity \
  --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
  --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
```

- Сығыу кодтары: I18NI000000009X таҙа матчта, I18NI000000010X ҡасан тораҡ пункттары/транспорт юҡ
  йәки артыҡ түләнгән, `2` дөрөҫ булмаған индереүҙәр буйынса.
- JSON резюмеһы + хештар ҡаҙна пакетына 2018 йылда беркетегеҙ.
  `docs/examples/sorafs_capacity_marketplace_validation/`.
- Ҡасан I18NI000000013X файл ерҙәре текст файл йыйыусы, иҫкәртмә
  `SoraFSCapacityReconciliationMismatch` (ҡара:
  `dashboards/alerts/sorafs_capacity_rules.yml`) юғалған һайын ут,
  артыҡ түләнгән, йәки көтөлмәгән провайдер күсермәләр асыҡлана.

## Сығыштар
- Пер-провайдер статустары менән диффтар өсөн тораҡ пункттары һәм штрафтар.
- Дөйөм алғанда, датчиктар тип экспортҡа сығарыла:
  - `sorafs_capacity_reconciliation_missing_total{kind}`
  - `sorafs_capacity_reconciliation_overpaid_total{kind}`
  - I18NI000000018X
  - I18NI000000019X
  - I18NI000000020X

## Көтөлгән диапазондар һәм толерантлыҡ
- Яраштырыу теүәл: көтөлгән vs фактик иҫәп-хисап/штраф нано нуль толерантлыҡ менән тап килергә тейеш. Нуль булмаған дифф теләһә ниндәй бит операторҙары тейеш.
- CI булавкалар 30 көнлөк һыу һибеү өсөн һыйҙырышлылыҡ өсөн һыйҙырышлылыҡ өсөн түләү (һынау `capacity_fee_ledger_30_day_soak_deterministic`) `71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1`. Яңыртыу дигести ҡасан ғына хаҡтар йәки һыуытыу семантикаһы үҙгәрә.
- Һыуытҡыс профилендә (`penalty_bond_bps=0`, I18NI000000024X) штрафтар нулдә ҡала; етештереү тик штрафтар сығарырға тейеш, ҡасан утилизация/өҫтөндә/PoR иҙәндәре боҙола һәм хөрмәт конфигурацияланған һыуытыу алдынан эҙмә-эҙлекле ҡырҡылған.