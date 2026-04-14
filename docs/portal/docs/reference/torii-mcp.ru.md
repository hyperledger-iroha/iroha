<!-- Auto-generated stub for Russian (ru) translation. Replace this content with the full translation. -->

---
lang: ru
direction: ltr
source: docs/portal/docs/reference/torii-mcp.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 316a408473f53a9763a18f40d49cfd766b5b93a3611e277e5a761e366e85c082
source_last_modified: "2026-03-15T11:38:44.302824+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
id: torii-mcp
title: Torii API MCP
description: Справочное руководство по использованию встроенного моста протокола контекста модели Torii.
---

Torii предоставляет собственный мост протокола контекста модели (MCP) по адресу `/v1/mcp`.
Эта конечная точка позволяет агентам обнаруживать инструменты и вызывать маршруты Torii/Connect через JSON-RPC.

## Форма конечной точки

- `GET /v1/mcp` возвращает метаданные возможностей (не упакованные в JSON-RPC).
- `POST /v1/mcp` принимает запросы JSON-RPC 2.0.
- Если `torii.mcp.enabled = false`, ни один маршрут не отображается.
- Если `torii.require_api_token` включен, отсутствующий/недействительный токен отклоняется перед отправкой JSON-RPC.

## Конфигурация

Включите MCP под `torii.mcp`:

```json
{
  "torii": {
    "mcp": {
      "enabled": true,
      "max_request_bytes": 1048576,
      "max_tools_per_list": 500,
      "profile": "read_only",
      "expose_operator_routes": false,
      "allow_tool_prefixes": [],
      "deny_tool_prefixes": [],
      "rate_per_minute": 240,
      "burst": 120,
      "async_job_ttl_secs": 300,
      "async_job_max_entries": 2000
    }
  }
}
```

Ключевое поведение:

- `profile` управляет видимостью инструмента (`read_only`, `writer`, `operator`).
- `allow_tool_prefixes`/`deny_tool_prefixes` применяет дополнительную политику на основе имени.
- `rate_per_minute`/`burst` применяет ограничение сегмента токенов для запросов MCP.
— Состояние асинхронного задания из `tools/call_async` сохраняется в памяти с помощью `async_job_ttl_secs` и `async_job_max_entries`.

## Рекомендуемый поток клиентов

1. Позвоните по номеру `initialize`.
2. Вызовите `tools/list` и кэшируйте `toolsetVersion`.
3. Используйте `tools/call` для обычных операций.
4. Используйте `tools/call_async` + `tools/jobs/get` для более длительных операций.
5. Повторно запустите `tools/list`, если `listChanged` равен `true`.

Не записывайте в код полный каталог инструментов. Откройте для себя во время выполнения.

## Методы и семантика

Поддерживаемые методы JSON-RPC:

- `initialize`
- `tools/list`
- `tools/call`
- `tools/call_batch`
- `tools/call_async`
- `tools/jobs/get`

Примечания:- `tools/list` принимает как `toolset_version`, так и `toolsetVersion`.
- `tools/jobs/get` принимает как `job_id`, так и `jobId`.
- `tools/list.cursor` — смещение числовой строки; недопустимые значения возвращаются к `0`.
- `tools/call_batch` — это максимально возможный вариант для каждого элемента (один неудачный вызов не приводит к сбою одноуровневых вызовов).
- `tools/call_async` немедленно проверяет только форму конверта; ошибки выполнения появляются позже в состоянии задания.
- `jsonrpc` должно быть `"2.0"`; опущено. `jsonrpc` принимается для совместимости.

## Авторизация и переадресация

Отправка MCP не обходит авторизацию Torii. Вызовы выполняют обычные обработчики маршрутов и проверки подлинности.

Torii пересылает входящие заголовки, связанные с аутентификацией, для отправки инструмента:

- `Authorization`
- `x-api-token`
- `x-iroha-account`
- `x-iroha-signature`
- `x-iroha-api-version`

Клиенты также могут предоставлять дополнительные заголовки для каждого вызова через `arguments.headers`.
`content-length`, `host` и `connection` из `arguments.headers` игнорируются.

## Модель ошибки

HTTP-уровень:

- `400` неверный JSON
- Токен API `403` отклонен до обработки JSON-RPC.
- Полезная нагрузка `413` превышает `max_request_bytes`
- `429` с ограничением скорости
- `200` для ответов JSON-RPC (включая ошибки JSON-RPC)

Уровень JSON-RPC:- Верхний уровень `error.data.error_code` стабилен (например, `invalid_request`, `invalid_params`, `tool_not_found`, `tool_not_allowed`, `job_not_found`, `rate_limited`).
- Отказы инструмента проявляются в результате использования инструмента MCP с `isError = true` и структурированными деталями.
- Сбои инструмента, отправленного по маршруту, сопоставляют статус HTTP с `structuredContent.error_code` (например, `forbidden`, `not_found`, `server_error`).

## Именование инструмента

Инструменты, производные от OpenAPI, используют стабильные имена на основе маршрутов:

- `torii.<method>_<path...>`
- Пример: `torii.get_v1_accounts`

Кураторские псевдонимы также доступны в разделах `iroha.*` и `connect.*`.

## Каноническая спецификация

Полный контракт на проводном уровне поддерживается в:

- `crates/iroha_torii/docs/mcp_api.md`

Когда поведение меняется в `crates/iroha_torii/src/mcp.rs` или `crates/iroha_torii/src/lib.rs`,
обновите эту спецификацию с тем же изменением, а затем отразите руководство по использованию ключа здесь.