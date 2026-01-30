---
lang: ru
direction: ltr
source: docs/portal/docs/soranet/constant-rate-profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e41e5ed0d7b74fe8ea1ac5bda290088ebb54572db240ae9c2546d719e0c6815f
source_last_modified: "2025-11-19T13:44:41.615366+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: constant-rate-profiles
title: Профили постоянной скорости SoraNet
sidebar_label: Профили постоянной скорости
description: Каталог пресетов SNNet-17B1 для production relays core/home и нулевого dogfood профиля SNNet-17A2, с математикой tick->bandwidth, CLI helpers и MTU guardrails.
---

:::note Канонический источник
Эта страница зеркалирует `docs/source/soranet/constant_rate_profiles.md`. Держите обе копии синхронизированными, пока устаревший набор документации не будет выведен.
:::

SNNet-17B вводит transport lanes с фиксированной скоростью, чтобы relays передавали трафик в ячейках 1,024 B независимо от размера payload. Операторы выбирают один из трех пресетов:

- **core** - relays в дата-центрах или на профессиональном хостинге, которые могут выделить >=30 Mbps под трафик.
- **home** - домашние или с низким uplink операторы, которым нужны анонимные fetches для приватных цепочек.
- **null** - dogfood пресет SNNet-17A2. Он сохраняет те же TLVs/envelope, но растягивает tick и ceiling для низкополосного staging.

## Сводка пресетов

| Профиль | Tick (ms) | Ячейка (B) | Cap lanes | Dummy floor | Payload на lane (Mb/s) | Payload ceiling (Mb/s) | Ceiling % от uplink | Рекомендованный uplink (Mb/s) | Cap neighbors | Триггер авто-отключения (%) |
|---------|-----------|----------|----------|-------------|-------------------------|------------------------|---------------------|----------------------------|--------------|--------------------------|
| core    | 5.0       | 1024     | 12       | 4           | 1.64                    | 19.50                  | 65                  | 30.0                       | 8            | 85                       |
| home    | 10.0      | 1024     | 4        | 2           | 0.82                    | 4.00                   | 40                  | 10.0                       | 2            | 70                       |
| null    | 20.0      | 1024     | 2        | 1           | 0.41                    | 0.75                   | 15                  | 5.0                        | 1            | 55                       |

- **Cap lanes** - максимальное число одновременных соседей с фиксированной скоростью. Relay отклоняет дополнительные цепочки при достижении лимита и увеличивает `soranet_handshake_capacity_reject_total`.
- **Dummy floor** - минимальное число lanes, которые остаются активными с dummy трафиком даже при меньшем спросе.
- **Payload ceiling** - бюджет uplink, выделенный под lanes с фиксированной скоростью после применения доли ceiling. Операторы не должны превышать этот бюджет, даже если есть дополнительная полоса.
- **Триггер авто-отключения** - устойчивый процент насыщения (усредненный по preset), который заставляет runtime опуститься до dummy floor. Емкость восстанавливается после recovery порога (75% для `core`, 60% для `home`, 45% для `null`).

**Важно:** preset `null` предназначен только для staging и dogfooding; он не обеспечивает приватность, требуемую для production цепочек.

## Таблица tick -> bandwidth

Каждая payload ячейка несет 1,024 B, поэтому столбец KiB/sec равен числу ячеек в секунду. Используйте helper, чтобы расширить таблицу пользовательскими ticks.

| Tick (ms) | Ячеек/сек | Payload KiB/sec | Payload Mb/s |
|-----------|-----------|-----------------|--------------|
| 5.0       | 200.00    | 200.00          | 1.64         |
| 7.5       | 133.33    | 133.33          | 1.09         |
| 10.0      | 100.00    | 100.00          | 0.82         |
| 15.0      | 66.67     | 66.67           | 0.55         |
| 20.0      | 50.00     | 50.00           | 0.41         |

Формула:

```
payload_mbps = (cell_bytes x 8 / 1_000_000) x (1000 / tick_ms)
```

CLI helper:

```bash
# Markdown table output for all presets plus default tick table
cargo xtask soranet-constant-rate-profile --tick-table --format markdown --json-out artifacts/soranet/constant_rate/report.json

# Restrict to a preset and emit JSON
cargo xtask soranet-constant-rate-profile --profile core --format json

# Custom tick series
cargo xtask soranet-constant-rate-profile --tick-table --tick-values 5,7.5,12,18 --format markdown
```

`--format markdown` выводит GitHub-стиль таблиц для сводки пресетов и опциональной tick таблицы, чтобы вы могли вставить детерминированный вывод в портал. Используйте вместе с `--json-out`, чтобы архивировать отрендеренные данные для доказательств управления.

## Конфигурация и overrides

`tools/soranet-relay` предоставляет пресеты как в конфигурационных файлах, так и в runtime overrides:

```bash
# Persisted in relay.json
"constant_rate_profile": "home"

# One-off override during rollout or maintenance
soranet-relay --config relay.json --constant-rate-profile core
```

Ключ конфигурации принимает `core`, `home` или `null` (default `core`). CLI overrides полезны для staging drills или запросов SOC, которые временно уменьшают duty cycle без переписывания конфигов.

## MTU guardrails

- Payload ячейки используют 1,024 B плюс ~96 B Norito+Noise framing и минимальные QUIC/UDP headers, сохраняя каждый датаграмм ниже минимального IPv6 MTU 1,280 B.
- Когда туннели (WireGuard/IPsec) добавляют дополнительную инкапсуляцию, вы **должны** уменьшить `padding.cell_size`, чтобы `cell_size + framing <= 1,280 B`. Валидатор relay применяет `padding.cell_size <= 1,136 B` (1,280 B - 48 B UDP/IPv6 overhead - 96 B framing).
- Профили `core` должны закреплять >=4 neighbors даже в idle, чтобы dummy lanes всегда покрывали часть PQ guards. Профили `home` могут ограничивать constant-rate цепочки для wallets/aggregators, но должны включать back-pressure, когда насыщение превышает 70% в течение трех окон telemetry.

## Telemetry и alerts

Relays экспортируют следующие метрики по каждому preset:

- `soranet_constant_rate_active_neighbors`
- `soranet_constant_rate_queue_depth`
- `soranet_constant_rate_saturation_percent`
- `soranet_constant_rate_dummy_lanes` / `soranet_constant_rate_dummy_ratio`
- `soranet_constant_rate_slot_rate_hz`
- `soranet_constant_rate_ceiling_hits_total`
- `soranet_constant_rate_degraded`

Alert при:

1. Dummy ratio ниже floor пресета (`core >= 4/8`, `home >= 2/2`, `null >= 1/1`) дольше двух окон.
2. `soranet_constant_rate_ceiling_hits_total` растет быстрее одного hit за пять минут.
3. `soranet_constant_rate_degraded` переключается на `1` вне планового drill.

Фиксируйте label пресета и список neighbors в отчетах об инцидентах, чтобы аудиторы могли подтвердить соответствие политик постоянной скорости требованиям дорожной карты.
