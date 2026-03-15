---
lang: ru
direction: ltr
source: docs/source/config/client_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa548ec31fe928decc5c23719472618ff97f4eb45b084f9f9084df82b96cfac6
source_last_modified: "2026-01-03T18:07:57.683798+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Справочник по настройке клиентского API

В этом документе описаны ручки настройки клиентской части Torii, которые
поверхности через `iroha_config::parameters::user::Torii`. Раздел ниже
основное внимание уделяется элементам управления транспортировкой Norito-RPC, представленным для NRPC-1; будущее
Настройки клиентского API должны расширять этот файл.

### `torii.transport.norito_rpc`

| Ключ | Тип | По умолчанию | Описание |
|-----|------|---------|-------------|
| `enabled` | `bool` | `true` | Главный переключатель, обеспечивающий декодирование двоичного кода Norito. Когда `false`, Torii отклоняет каждый запрос Norito-RPC с `403 norito_rpc_disabled`. |
| `stage` | `string` | `"disabled"` | Уровень развертывания: `disabled`, `canary` или `ga`. Этапы определяют решения о допуске и выходные данные `/rpc/capabilities`. |
| `require_mtls` | `bool` | `false` | Обеспечивает mTLS для транспорта Norito-RPC: когда `true`, Torii отклоняет запросы Norito-RPC, которые не несут заголовок маркера mTLS (например, `X-Forwarded-Client-Cert`). Флаг отображается через `/rpc/capabilities`, поэтому SDK могут предупреждать о неправильно настроенных средах. |
| `allowed_clients` | `array<string>` | `[]` | Канарский белый список. При `stage = "canary"` принимаются только запросы, содержащие заголовок `X-API-Token`, присутствующий в этом списке. |

Пример конфигурации:

```toml
[torii.transport.norito_rpc]
enabled = true
require_mtls = true
stage = "canary"
allowed_clients = ["alpha-canary-token", "beta-canary-token"]
```

Семантика сцены:

- **отключено** — Norito-RPC недоступен, даже если `enabled = true`. Клиенты
  получить `403 norito_rpc_disabled`.
- **canary** — запросы должны включать заголовок `X-API-Token`, соответствующий одному
  из `allowed_clients`. Все остальные запросы получают `403
  norito_rpc_canary_denied`.
- **ga** — Norito-RPC доступен каждому аутентифицированному вызывающему абоненту (в соответствии с
  обычная скорость и ограничения перед аутентификацией).

Операторы могут обновлять эти значения динамически через `/v2/config`. Каждое изменение
немедленно отражается в `/rpc/capabilities`, обеспечивая SDK и наблюдаемость.
информационные панели, чтобы показать состояние транспорта в реальном времени.