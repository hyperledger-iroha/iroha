---
lang: ru
direction: ltr
source: docs/connect_config.md
status: complete
translator: manual
source_hash: 15799a5698133ba1d6c5510d71d17fd60934519df890f83cb53b49d56980dc5b
source_last_modified: "2025-11-05T17:16:36.700630+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Русский перевод docs/connect_config.md (Torii Connect Configuration) -->

## Конфигурация Torii Connect

Iroha Torii может предоставлять опциональные WebSocket‑эндпоинты в стиле
WalletConnect и минимальный встроенный relay, когда feature Cargo `connect`
включён (по умолчанию). Поведение в runtime настраивается через конфиг:

- Установите `connect.enabled=false`, чтобы отключить все маршруты Connect
  (`/v2/connect/*`).
- Оставьте `true` (по умолчанию), чтобы включить WS‑эндпоинты сессий и
  `/v2/connect/status`.

Переменные окружения (user‑config → actual‑config):

- `CONNECT_ENABLED` (bool; по умолчанию `true`)
- `CONNECT_WS_MAX_SESSIONS` (`usize`; по умолчанию `10000`)
- `CONNECT_WS_PER_IP_MAX_SESSIONS` (`usize`; по умолчанию `10`)
- `CONNECT_WS_RATE_PER_IP_PER_MIN` (`u32`; по умолчанию `120`)
- `CONNECT_FRAME_MAX_BYTES` (`usize`; по умолчанию `64000`)
- `CONNECT_SESSION_BUFFER_MAX_BYTES` (`usize`; по умолчанию `262144`)
- `CONNECT_PING_INTERVAL_MS` (duration; по умолчанию `30000`)
- `CONNECT_PING_MISS_TOLERANCE` (`u32`; по умолчанию `3`)
- `CONNECT_PING_MIN_INTERVAL_MS` (duration; по умолчанию `15000`)
- `CONNECT_DEDUPE_CAP` (`usize`; по умолчанию `8192`)
- `CONNECT_RELAY_ENABLED` (bool; по умолчанию `true`)
- `CONNECT_RELAY_STRATEGY` (string; по умолчанию `"broadcast"`)
- `CONNECT_P2P_TTL_HOPS` (`u8`; по умолчанию `0`)

Примечания:

- `CONNECT_SESSION_TTL_MS` и `CONNECT_DEDUPE_TTL_MS` задаются как duration‑литералы
  в пользовательском конфиге и маппятся на реальные поля `session_ttl` и
  `dedupe_ttl`.
- Механизм heartbeat ограничивает настроенный интервал минимальным
  значением, совместимым с браузерами (`ping_min_interval_ms`); сервер
  терпит `ping_miss_tolerance` подряд пропущенных pong‑ответов, после чего
  закрывает WebSocket и увеличивает метрику `connect.ping_miss_total`.
- При отключении в runtime (`connect.enabled=false`) маршруты Connect WS и
  статус‑эндпоинты не регистрируются; запросы к `/v2/connect/ws` и
  `/v2/connect/status` возвращают 404.
- Сервер требует клиентский `sid` в `/v2/connect/session` (base64url или hex,
  32 байта). Генерация fallback‑`sid` больше не выполняется.

См. также:
`crates/iroha_config/src/parameters/{user,actual}.rs` и значения по умолчанию
в `crates/iroha_config/src/parameters/defaults.rs` (модуль `connect`).

