---
lang: kk
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

Жол картасының **SF-2c** тармағы қазынашылық сыйақыны төлеу кітабын растауды талап етеді
әр түнде орындалатын XOR тасымалдауларына сәйкес келеді. пайдаланыңыз
салыстыру үшін `scripts/telemetry/capacity_reconcile.py` көмекшісі
Орындалған тасымалдау бумасына қарсы `/v1/sorafs/capacity/state` суреті және
Alertmanager үшін Prometheus мәтіндік файл көрсеткіштерін шығарыңыз.

## Алғышарттар
- Torii ішінен экспортталған сыйымдылық күйінің суреті (`fee_ledger` жазбалары).
- Бір терезеге арналған кітап экспорты (`provider_id_hex` бар JSON немесе NDJSON,
  `kind` = есеп айырысу/айыппұл, және `amount_nano`).
- Ескертулер қажет болса, node_exporter мәтіндік файл жинағышына жол.

## Runbook
```bash
python3 scripts/telemetry/capacity_reconcile.py \
  --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
  --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
  --label nightly-capacity \
  --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
  --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
```

- Шығу кодтары: таза сәйкестікте `0`, есеп айырысулар/айыппұлдар болмаған кезде `1`
  немесе артық төленген, жарамсыз кірістерде `2`.
- JSON қорытындысын + хэштерді қазыналық пакетке тіркеңіз
  `docs/examples/sorafs_capacity_marketplace_validation/`.
- `.prom` файлы мәтіндік файл коллекторына түскенде, ескерту
  `SoraFSCapacityReconciliationMismatch` (қараңыз
  `dashboards/alerts/sorafs_capacity_rules.yml`) жоғалған кезде өрт шығады,
  артық төленген немесе күтпеген провайдер аударымдары анықталды.

## Шығыстар
- Есеп айырысулар мен айыппұлдар бойынша айырмашылықтары бар провайдерлердің мәртебесі.
- Өлшемдер ретінде экспортталған жиынтықтар:
  - `sorafs_capacity_reconciliation_missing_total{kind}`
  - `sorafs_capacity_reconciliation_overpaid_total{kind}`
  - `sorafs_capacity_reconciliation_unexpected_transfers_total`
  - `sorafs_capacity_reconciliation_expected_nano{kind}`
  - `sorafs_capacity_reconciliation_actual_nano{kind}`

## Күтілетін ауқымдар мен төзімділіктер
- Сәйкестік дәл: күтілетін және нақты есеп айырысу/айыппұл наностары нөлдік төзімділікке сәйкес келуі керек. Кез келген нөлдік емес айырмашылық операторларды беттеу керек.
- CI сыйымдылық алымы кітабына (`capacity_fee_ledger_30_day_soak_deterministic` сынағы) `71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1` дейін 30 күндік сіңіру дайджестін бекітеді. Баға немесе салқындату семантикасы өзгерген кезде ғана дайджестті жаңартыңыз.
- Жібіту профилінде (`penalty_bond_bps=0`, `strike_threshold=u32::MAX`) айыппұлдар нөлде қалады; Өндіріс пайдалану/жұмыс уақыты/PoR қабаттары бұзылған кезде ғана айыппұлдар шығаруы және кезекті қиғаш сызықтар алдында конфигурацияланған салқындату мерзімін сақтауы керек.