---
lang: ba
direction: ltr
source: docs/source/i3_slo_harness.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: df3e3ac15baf47a6c53001acabcac7987a2386c2b772b1d8625eb60598f95a60
source_last_modified: "2025-12-29T18:16:35.966039+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Iroha 3 СЛО Харнесс

Iroha 3 релиз линияһы асыҡ SLOs өсөн тәнҡитле Nexus юлдары:

- финал слот оҙайлылығы (NX‐18 каденция)
- иҫбатлаусы тикшерелеү (сертелдәргә, JDG аттестациялары, күпер иҫбатлауҙары)
- иҫбатлаусы ос нөктәһе менән эш итеү (Аксум юл прокси аша тикшерелгән латентлыҡ)
- түләү һәм ставкалар юлдары (түләүсе/бағыусы һәм облигациялар/слеш ағымы)

## Бюджеттар

Бюджеттар йәшәй `benchmarks/i3/slo_budgets.json` һәм карта туранан-тура эскәмйәгә .
сценарийҙары I3 люкс. Маҡсаттар p99 маҡсаттары буйынса шылтырата:

- Түләү/стажировка: 50мс шылтыратыу өсөн (`fee_payer`, `fee_sponsor`, `staking_bond`, `staking_slash`)
- Серҙе / JDG / күпер тикшерергә: 80мс (`commit_cert_verify`, `jdg_attestation_verify`,
  `bridge_proof_verify`)
- Серт йыйыуҙы йөкмәтә: 80мс (`commit_cert_assembly`)
- 50 мс (`access_scheduler`) инеүҙе планлаштырыусы.
- Дәлил ос нөктәһе прокси: 120мс (`torii_proof_endpoint`)

14.4/6.0
күп тәҙрә нисбәте өсөн пейджинг ҡаршы билет иҫкәртмәләр.

## Ханес

Йүгерергә йүгән аша `cargo xtask i3-slo-harness`:

```bash
cargo xtask i3-slo-harness \
  --iterations 64 \
  --sample-count 5 \
  --out-dir artifacts/i3_slo/latest
```

Сығыштар:

- `bench_report.json|csv|md` — сеймал I3 эскәмйә люкс һөҙөмтәләре (аллы хеш + сценарийҙар)
- `slo_report.json|md` — SLO баһалау менән үткәреү/уңышһыҙлыҡ/бюджет-никаза

Йүгән бюджеттар файлын ҡуллана һәм `benchmarks/i3/slo_thresholds.json` .
эскәмйә ваҡытында тиҙ уңышһыҙлыҡҡа осрай, ҡасан маҡсатлы регрессия.

## Телеметрия һәм приборҙар таҡтаһы

- Финал: `histogram_quantile(0.99, rate(iroha_slot_duration_ms_bucket[5m]))`
- Дәлил тикшерергә: `histogram_quantile(0.99, sum by (le) (rate(zk_verify_latency_ms_bucket{status="Verified"}[5m])))`

Grafana стартер панелдәре йәшәй `dashboards/grafana/i3_slo.json`. Prometheus
яндырыу тиҙлеге тураһында иҫкәртмәләр `dashboards/alerts/i3_slo_burn.yml` менән тәьмин ителә.
бюджеттар өҫтөндә бешерелгән (финал 2s, иҫбатлау раҫлау 80мс, дәлилдәр осло прокси
120мс).

## Оператив иҫкәрмәләр

- Төндә йүгәнде йүгертегеҙ; `fee_payer` баҫтырыу
  идара итеү дәлилдәре өсөн эскәмйә артефакттары менән бер рәттән.
- Әгәр бюджет уңышһыҙлыҡҡа осраһа, сценарийҙы билдәләү өсөн эскәмйә маркировкаһын ҡулланығыҙ, тимәк, быраулау
  тура килгән Grafana панелендә/иҫкәртмә тере метрика менән корреляция.
- Дәлил ос нөктәһе SLOs тикшерелгән латентлыҡты ҡулланыу өсөн прокси ҡотолоу өсөн пер-ауытыу .
  кардиналитет шартлатыу; ориентир маҡсаты (120мс) тап килә тотоу/DoS
  ҡоршауҙар API иҫбатлау.