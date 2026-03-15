---
lang: ru
direction: ltr
source: docs/source/sumeragi_pacemaker.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b0e7a0ca09fb294fed62bae221a6fd82ce07d9cf0802d90d960afaa5bcec40e9
source_last_modified: "2025-12-09T14:07:26.178015+00:00"
translation_last_reviewed: 2026-01-01
---

# Sumeragi Pacemaker - Таймеры, backoff и jitter

Эта заметка описывает политику pacemaker (timer) для Sumeragi и дает рекомендации операторам и примеры. Таймеры определяют, когда лидеры делают propose и когда валидаторы предлагают/входят в новый view после бездействия.

Статус: реализованы базовое окно на основе EMA, порог RTT и настраиваемые пределы jitter/backoff. Интервал предложений pacemaker теперь зажат между целевым block-time и propose timeout с RTT floor, ограничен `sumeragi.advanced.pacemaker.max_backoff_ms`. (`effective_block_time_ms` = `block_time_ms` scaled by `pacing_factor_bps`). Jitter применяется детерминированно для каждого узла и каждой пары (height, view).

## Концепции
- Базовое окно: экспоненциальная скользящая средняя наблюдаемых фаз консенсуса
  (propose, collect_da, collect_prevote, collect_precommit, commit). EMA
  инициализируется из `sumeragi.advanced.npos.timeouts.*_ms`; пока не набрано достаточно
  выборок, она фактически совпадает с настроенными значениями по умолчанию.
  EMA `collect_aggregator` экспортируется для наблюдаемости, но не включается в
  окно pacemaker. Сглаженные значения доступны через
  `sumeragi_phase_latency_ema_ms{phase=...}`.
- Множитель backoff: `sumeragi.advanced.pacemaker.backoff_multiplier` (по умолчанию 1). Каждый timeout добавляет `base * multiplier` к текущему окну.
- RTT floor: `avg_rtt_ms * sumeragi.advanced.pacemaker.rtt_floor_multiplier` (по умолчанию 2). Предотвращает слишком агрессивные timeouts на линках с высокой задержкой.
- Cap: `sumeragi.advanced.pacemaker.max_backoff_ms` (по умолчанию 60_000 ms). Жесткий потолок окна.
- Seed интервала предложений: `max(effective_block_time_ms, propose_timeout_ms * rtt_floor_multiplier)` и никогда не выше `sumeragi.advanced.pacemaker.max_backoff_ms`. (`effective_block_time_ms` = `block_time_ms` scaled by `pacing_factor_bps`). Это интервал в устойчивом состоянии даже без backoff.

Обновление эффективного окна при timeout:
- `window = min(cap, max(window + base * backoff_mul, avg_rtt * rtt_floor_mul))`
- При отсутствии RTT выборок RTT floor равен 0.

Экспортируемая телеметрия (см. telemetry.md):
- Runtime: `sumeragi_pacemaker_backoff_ms`, `sumeragi_pacemaker_rtt_floor_ms`, `sumeragi_phase_latency_ema_ms{phase=...}`
- REST snapshot: `/v2/sumeragi/phases` теперь включает `ema_ms` вместе с последними
  перефазными задержками, чтобы дашборды могли строить тренд EMA без прямого опроса
  Prometheus.
- Config: `sumeragi.advanced.pacemaker.backoff_multiplier`, `sumeragi.advanced.pacemaker.rtt_floor_multiplier`, `sumeragi.advanced.pacemaker.max_backoff_ms`

## Политика jitter
Чтобы избежать эффекта стада (синхронизированных timeout), pacemaker поддерживает небольшой jitter на узел вокруг эффективного окна.

Детерминированный jitter на узел:
- Источник: `blake2(chain_id || peer_id || height || view)` -> 64-bit, масштабируется до [-J, +J].
- Рекомендуемая полоса jitter: +/-10% от вычисленного `window`.
- Применять один раз на окно (height, view); не изменять внутри одной view.

Псевдокод:
```
base = ema_total_ms(view, height)  // seeded by sumeragi.advanced.npos.timeouts.*_ms
window = min(cap, max(prev + base * backoff_mul, avg_rtt * rtt_floor_mul))
seed = blake2(chain_id || peer_id || height || view)
u = (seed % 10_000) as f64 / 10_000.0  // [0, 1)
jfrac = 0.10 // 10% jitter band
jitter = (u * 2.0 - 1.0) * jfrac * window.as_millis() as f64
window_jittered_ms = (window.as_millis() as f64 + jitter).clamp(0.0, cap_ms as f64)
```

Примечания:
- Jitter детерминирован для узла и (height, view) и не требует внешней случайности.
- Держите полосу небольшой (<= 10%), чтобы сохранять отзывчивость и снижать stampede.
- Jitter влияет на liveness, но не на safety.

Telemetry:
- `sumeragi_pacemaker_jitter_ms` — абсолютная величина jitter (ms).
- `sumeragi_pacemaker_jitter_frac_permille` — заданная полоса jitter (permille).

## Рекомендации операторам

Низколатентные LAN/PoA
- backoff_multiplier: 1-2
- rtt_floor_multiplier: 2
- max_backoff_ms: 5_000-10_000
- Обоснование: держать предложения частыми; короткое восстановление при stall.

Геораспределенный WAN
- backoff_multiplier: 2-3
- rtt_floor_multiplier: 3-5
- max_backoff_ms: 30_000-60_000
- Обоснование: избегать агрессивного churn на линках с высоким RTT; терпеть bursts.

Сети с высокой вариативностью или мобильные
- backoff_multiplier: 3-4
- rtt_floor_multiplier: 4-6
- max_backoff_ms: 60_000
- Обоснование: минимизировать синхронные смены view; рассмотрите включение jitter, когда развертывание допускает более медленные commits.

## Примеры

1) LAN с avg RTT ~= 2 ms, base = 2000 ms, cap = 10_000 ms
- backoff_mul = 1, rtt_floor_mul = 2
- Первый timeout: window = max(0 + 2000, 2*2) = 2000 ms
- Второй timeout: window = min(10_000, 2000 + 2000) = 4000 ms

2) WAN с avg RTT ~= 80 ms, base = 4000 ms, cap = 60_000 ms
- backoff_mul = 2, rtt_floor_mul = 3
- Первый timeout: window = max(0 + 8000, 80*3) = 8000 ms
- Второй timeout: window = min(60_000, 8000 + 8000) = 16_000 ms

3) С полосой jitter 10% (иллюстративно, не реализовано)
- window = 16_000 ms, jitter in [-1_600, +1_600] ms -> `window' in [14_400, 17_600]` ms на узел

## Мониторинг
- Trend backoff: `avg_over_time(sumeragi_pacemaker_backoff_ms[5m])`
- Inspect RTT floor: `avg_over_time(sumeragi_pacemaker_rtt_floor_ms[5m])`
- Сравнить EMA с histogram: `sumeragi_phase_latency_ema_ms{phase=...}` рядом с соответствующими перцентилями `sumeragi_phase_latency_ms{phase=...}`
- Проверить config: `max(sumeragi.advanced.pacemaker.backoff_multiplier)`, `max(sumeragi.advanced.pacemaker.rtt_floor_multiplier)`, `max(sumeragi.advanced.pacemaker.max_backoff_ms)`

## Детерминизм и безопасность
- Таймеры/backoff/jitter влияют только на то, когда узлы запускают предложения/смены view; они не влияют на валидность подписей или правила commit certificate.
- Любая случайность должна быть детерминированной для узла и (height, view). Избегайте time-of-day или RNG ОС в критических путях консенсуса.
