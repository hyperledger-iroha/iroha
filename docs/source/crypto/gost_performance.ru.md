---
lang: ru
direction: ltr
source: docs/source/crypto/gost_performance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fab384ae80e1993b1e54d6addc82fd3dc652fb6e3958bea6a04e057a1805b57
source_last_modified: "2026-01-03T18:07:57.084090+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Рабочий процесс производительности ГОСТ

В этом примечании описывается, как мы отслеживаем и обеспечиваем соблюдение границ производительности для
Серверная часть подписи ГОСТ TC26.

## Запуск локально

```bash
make gost-bench                     # run benches + tolerance check
make gost-bench GOST_BENCH_ARGS="--tolerance 0.30"  # override guard
make gost-dudect                    # run the constant-time timing guard
./scripts/update_gost_baseline.sh   # bench + rebaseline helper
```

За кулисами обе цели вызывают `scripts/gost_bench.sh`, что:

1. Выполняется `cargo bench -p iroha_crypto --bench gost_sign --features gost -- --noplot`.
2. Запускает `gost_perf_check` для `target/criterion`, сверяя медианы с
   проверенный базовый уровень (`crates/iroha_crypto/benches/gost_perf_baseline.json`).
3. Вставляет сводку Markdown в `$GITHUB_STEP_SUMMARY`, если она доступна.

Чтобы обновить базовый уровень после утверждения регресса/улучшения, запустите:

```bash
make gost-bench-update
```

или напрямую:

```bash
./scripts/gost_bench.sh --write-baseline \
  --baseline crates/iroha_crypto/benches/gost_perf_baseline.json
```

`scripts/update_gost_baseline.sh` запускает стенд + проверку, перезаписывает базовый JSON и печатает
новые медианы. Всегда фиксируйте обновленный JSON вместе с записью решения в
`crates/iroha_crypto/docs/gost_backend.md`.

### Текущие эталонные медианы

| Алгоритм | Медиана (мкс) |
|------|-------------|
| ed25519 | 69,67 |
| gost256_paramset_a | 1136,96 |
| gost256_paramset_b | 1129,05 |
| gost256_paramset_c | 1133,25 |
| gost512_paramset_a | 8944,39 |
| gost512_paramset_b | 8963,60 |
| секп256к1 | 160,53 |

## КИ

`.github/workflows/gost-perf.yml` использует тот же сценарий, а также запускает защиту синхронизации.
CI дает сбой, когда измеренное медианное значение превышает базовый уровень более чем на настроенный допуск.
(по умолчанию 20%) или когда защита синхронизации обнаруживает утечку, поэтому регрессии фиксируются автоматически.

## Итоговый вывод

`gost_perf_check` печатает таблицу сравнения локально и добавляет то же содержимое в
`$GITHUB_STEP_SUMMARY`, поэтому журналы заданий CI и сводки выполнения имеют одни и те же номера.