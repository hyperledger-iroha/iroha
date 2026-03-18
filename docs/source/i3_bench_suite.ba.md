---
lang: ba
direction: ltr
source: docs/source/i3_bench_suite.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a3158cd70a42104bacaafc520fdcc10e20e3bc347d895be448fcb10da4f668bd
source_last_modified: "2025-12-29T18:16:35.965528+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha 3 эскәмйә люкс

Iroha 3 эскәмйә люкс тапҡыр эҫе юлдарҙан беҙ ставка ваҡытында таянып, түләү
зарядка, иҫбатлау тикшерелгән, планлаштырыу, һәм иҫбатлау ос нөктәләре. Ул йүгерә
`xtask` командаһы менән детерминистик ҡоролмалары (фиксированный орлоҡтар, нығытылған төп материал,
һәм тотороҡло запрос файҙалы йөкләмәләр) шуға күрә һөҙөмтәләр хужалар буйынса ҡабатлана.

## Йүгереп люкс

```bash
cargo xtask i3-bench-suite \
  --iterations 64 \
  --sample-count 5 \
  --json-out benchmarks/i3/latest.json \
  --csv-out benchmarks/i3/latest.csv \
  --markdown-out benchmarks/i3/latest.md \
  --threshold benchmarks/i3/thresholds.json \
  --allow-overwrite
```

Флагтар:

- `--iterations` бер сценарий өлгөһөнә итерацион идара итеү (по умолчанию: 64).
- `--sample-count` һәр сценарийҙы ҡабатлай, медиананы иҫәпләү өсөн (по умолчанию: 5).
- `--json-out|--csv-out|--markdown-out` сығыш артефакттарын һайлай (бөтәһе лә факультатив).
- `--threshold` медианаларҙы башланғыс сиктәр менән сағыштыра (`--no-threshold` тип аталған.
  һикерергә).
- `--flamegraph-hint` аннотациялау Markdown отчет менән `cargo flamegraph`
  команда профиле сценарийы.

CI елем йәшәй `ci/i3_bench_suite.sh` һәм өҫтәге юлдарға тиклем ғәҙәттәгесә; йыйылма
`I3_BENCH_ITERATIONS`/`I3_BENCH_SAMPLES` төнгөлөктә эшләү ваҡытын көйләү өсөн.

## Сценарий

- `fee_payer` / `fee_sponsor` / `fee_insufficient` — түләүсе vs бағыусылыҡ дебет
  һәм етешһеҙлек кире ҡағыу.
- `staking_bond` / `staking_slash` — облигациялар/бишһеҙ сират менән һәм 2000 й.
  ҡырҡып.
- `commit_cert_verify` X / `jdg_attestation_verify` / `bridge_proof_verify` — .
  ҡултамға тикшерелеүе тураһында таныҡлыҡтар, JDG аттестациялары, һәм күпер
  иҫбатлау файҙалы йөктәр.
- `commit_cert_assembly` — таныҡлыҡтар өсөн һеңдерелгән йыйылма.
- `access_scheduler` — конфликт-аңлы рөхсәт ителгән график.
- `torii_proof_endpoint` — Axum иҫбатлау осло осло анализлау + тикшерелгән түңәрәк сәйәхәт.

Һәр сценарийҙа итерацион, үткәреүсәнлек һәм а медиана наносекундтары теркәлә.
тиҙ регрессиялар өсөн детерминистик бүленеш иҫәпләүсеһе. Сиктәр 1990 йылда йәшәй.
`benchmarks/i3/thresholds.json`; ҡабарып сиктәре унда ҡасан аппарат үҙгәрә һәм
яңы артефактты отчет менән бер рәттән ҡылған.

## Төҙөкләндереүҙең

- Пенный процессор йышлығы/губернаторы йыя, дәлилдәр йыйыу өсөн, шау-шыулы регрессияларҙан ҡотолоу өсөн.
- Ҡулланыу `--no-threshold` өсөн эҙләнеүҙең йүгерә, һуңынан яңынан мөмкинлек бирә, бер тапҡыр база линияһы булып тора
  яңыртылған.
- Бер сценарийҙы профилактикалау өсөн `--iterations 1`-ны билдәләгеҙ һәм 1990 йылдарҙа яңынан эшләтеп ебәрегеҙ.
  `cargo flamegraph -p xtask -- i3-bench-suite --iterations 128 --sample-count 1 --no-threshold --flamegraph-hint`.