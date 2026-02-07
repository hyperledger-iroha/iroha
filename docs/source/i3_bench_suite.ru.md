---
lang: ru
direction: ltr
source: docs/source/i3_bench_suite.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a3158cd70a42104bacaafc520fdcc10e20e3bc347d895be448fcb10da4f668bd
source_last_modified: "2026-01-03T18:08:01.692664+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha Набор из 3 скамеек

Пакет Iroha 3 умножает количество горячих путей, на которые мы полагаемся во время ставок, комиссию
взимание платы, проверка подтверждения, планирование и конечные точки подтверждения. Он работает как
Команда `xtask` с детерминированными приспособлениями (фиксированные начальные значения, фиксированный материал ключа,
и стабильные полезные данные запросов), поэтому результаты воспроизводятся на разных хостах.

## Запуск пакета

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

Флаги:

- `--iterations` управляет итерациями для каждого примера сценария (по умолчанию: 64).
- `--sample-count` повторяет каждый сценарий для вычисления медианы (по умолчанию: 5).
- `--json-out|--csv-out|--markdown-out` выбирает выходные артефакты (все необязательно).
- `--threshold` сравнивает медианы с базовыми границами (установите `--no-threshold`
  пропустить).
- `--flamegraph-hint` аннотирует отчет Markdown с помощью `cargo flamegraph`.
  команда для профилирования сценария.

CI-клей находится в `ci/i3_bench_suite.sh` и по умолчанию использует указанные выше пути; набор
`I3_BENCH_ITERATIONS`/`I3_BENCH_SAMPLES` для настройки времени выполнения в ночных играх.

## Сценарии

- `fee_payer` / `fee_sponsor` / `fee_insufficient` — дебет плательщика против спонсора
  и отказ от дефицита.
- `staking_bond` / `staking_slash` — связывание/отвязывание очереди с и без
  рубящий.
- `commit_cert_verify` / `jdg_attestation_verify` / `bridge_proof_verify` —
  проверка подписи по сертификатам фиксации, аттестациям JDG и мосту
  доказательства полезной нагрузки.
- `commit_cert_assembly` — сборка дайджеста для сертификатов фиксации.
- `access_scheduler` — планирование набора доступа с учетом конфликтов.
- `torii_proof_endpoint` — анализ конечной точки с доказательством Axum + проверка туда и обратно.

В каждом сценарии фиксируются медианные наносекунды на итерацию, пропускную способность и
детерминированный счетчик распределения для быстрой регрессии. Пороги живут в
`benchmarks/i3/thresholds.json`; границы там, когда оборудование меняется и
зафиксируйте новый артефакт вместе с отчетом.

## Устранение неполадок

- Зафиксируйте частоту/регулятор процессора при сборе данных, чтобы избежать шумных регрессий.
- Используйте `--no-threshold` для исследовательских запусков, затем снова включите, как только базовый уровень будет достигнут.
  освеженный.
- Чтобы профилировать один сценарий, установите `--iterations 1` и повторите запуск под
  `cargo flamegraph -p xtask -- i3-bench-suite --iterations 128 --sample-count 1 --no-threshold --flamegraph-hint`.