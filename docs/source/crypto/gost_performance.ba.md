---
lang: ba
direction: ltr
source: docs/source/crypto/gost_performance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fab384ae80e1993b1e54d6addc82fd3dc652fb6e3958bea6a04e057a1805b57
source_last_modified: "2025-12-29T18:16:35.939573+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ГОСТ етештереүсәнлеге эш ағымы

Был иҫкәрмә документында, нисек беҙ күҙәтеп һәм үтәү өсөн конверт өсөн етештереү
TC26 GOST ҡул ҡуйыу бекэнд.

## Урындағы йүгереүҙе

```bash
make gost-bench                     # run benches + tolerance check
make gost-bench GOST_BENCH_ARGS="--tolerance 0.30"  # override guard
make gost-dudect                    # run the constant-time timing guard
./scripts/update_gost_baseline.sh   # bench + rebaseline helper
```

Күренештәр артында ике маҡсатлы шылтыратыу `scripts/gost_bench.sh`, улар:

1. `cargo bench -p iroha_crypto --bench gost_sign --features gost -- --noplot` башҡарҙары.
.
   тикшерелгән-база һыҙығы (`crates/iroha_crypto/benches/gost_perf_baseline.json`).
.

Регрессия/яҡшыртыуҙы раҫлауҙан һуң база һыҙығын яңыртыу өсөн, йүгерергә:

```bash
make gost-bench-update
```

йәки туранан-тура:

```bash
./scripts/gost_bench.sh --write-baseline \
  --baseline crates/iroha_crypto/benches/gost_perf_baseline.json
```

`scripts/update_gost_baseline.sh` эскәмйә + шашка эшләй, JSON база линияһы өҫтөнә яҙа, һәм баҫмалар
яңы медианалар. Һәр ваҡыт яңыртылған JSON 2012 йылда ҡарар ҡабул итеү яҙмаһы менән бер рәттән үтә.
`crates/iroha_crypto/docs/gost_backend.md`.

### Ағымдағы белешмә медианалары

| Алгоритм | Медиан (μs) |
|--------------------|-------------|
| ed25519 | 69.67 |
| gost256_paramset_a | 1136.96 |
| gost256_paramset_b | 1129.05 |
| gost256_paramset_c | 1133.25 |
| gost512_paramet_a | 8944.39 |
| gost512_paramset_b | 8963.60 |
| secp256к1 | 160.53 |

## CI

`.github/workflows/gost-perf.yml` шул уҡ сценарийҙы ҡуллана һәм шулай уҡ дудект ваҡыт һаҡсыһы эшләй.
CI уңышһыҙлыҡҡа осрай, ҡасан үлсәү медианаһы база линияһынан артып, күберәк конфигурацияланған толерантлыҡ .
(20% ғәҙәттәгесә) йәки ваҡыт һаҡсыһы һыу ағыуын асыҡлағанда, шуға күрә регрессиялар автоматик рәүештә тотола.

## Йәмғеһе сығыш

`gost_perf_check` сағыштырыу таблицаһын локаль рәүештә баҫтыра һәм шул уҡ йөкмәткене 2012 йылға
`$GITHUB_STEP_SUMMARY`, шуға күрә CI эш урындары журналдары һәм резюме йүгерә бер үк һандар менән уртаҡлаша.