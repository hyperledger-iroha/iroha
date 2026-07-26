---
lang: ru
direction: ltr
source: docs/source/runbooks/torii_norito_rpc_faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 969f24fd53af5e5714eb4b257dc086b3990550a5085c9464805f689c7d8ee324
source_last_modified: "2025-12-14T09:53:36.247266+00:00"
translation_last_reviewed: 2025-12-28
---

# FAQ для операторов Norito-RPC

Этот FAQ объединяет тумблеры rollout/rollback, телеметрию и артефакты доказательств
из пунктов **NRPC-2** и **NRPC-4**, чтобы у операторов была единая точка
доступа во время canary, brownout или инцидентных репетиций. Используйте его
как входную страницу для on‑call handoff; подробные процедуры остаются в
`docs/source/torii/norito_rpc_rollout_plan.md` и
`docs/source/runbooks/torii_norito_rpc_canary.md`.

## 1. Конфигурационные переключатели

| Путь | Назначение | Допустимые значения / примечания |
|------|---------|------------------------|
| `torii.transport.norito_rpc.enabled` | Жёсткий on/off переключатель Norito‑транспорта. | `true` оставляет HTTP‑handlers включёнными; `false` выключает их вне зависимости от stage. |
| `torii.transport.norito_rpc.require_mtls` | Обязательный mTLS для Norito‑эндпоинтов. | По умолчанию `true`. Отключайте только в изолированных staging‑пулах. |
| `torii.transport.norito_rpc.allowed_clients` | Белый список сервисных аккаунтов / API‑токенов для Norito. | Задайте CIDR‑блоки, хэши токенов или OIDC client IDs согласно окружению. |
| `torii.transport.norito_rpc.stage` | Этап rollout, объявляемый SDK. | `disabled` (отклоняет Norito, принудительно JSON), `canary` (только allowlist, усиленная телеметрия), `ga` (по умолчанию для всех аутентифицированных клиентов). |
| `torii.preauth_scheme_limits.norito_rpc` | Лимиты конкуренции и burst по схеме. | Используйте те же ключи, что и для HTTP/WS throttles (например, `max_in_flight`, `rate_per_sec`). Повышение лимита без обновления Alertmanager ломает guardrail rollout. |
| `transport.norito_rpc.*` в `docs/source/config/client_api.md` | Клиентские overrides (CLI / SDK discovery). | Используйте `cargo xtask client-api-config diff`, чтобы просмотреть изменения перед отправкой в Torii. |

**Рекомендуемый brownout‑поток**

1. Установите `torii.transport.norito_rpc.stage=disabled`.
2. Оставьте `enabled=true`, чтобы probes/тесты алертов продолжали дергать handlers.
3. Переведите `torii.preauth_scheme_limits.norito_rpc.max_in_flight` в ноль при
   необходимости мгновенной остановки (например, пока ждёте применения конфига).
4. Обновите операторский лог и приложите digest новой конфигурации к stage‑отчёту.

## 2. Операционные чек‑листы

- **Canary / staging** — следуйте `docs/source/runbooks/torii_norito_rpc_canary.md`.
  Там перечислены те же ключи конфигурации и артефакты, которые собирают
  `scripts/run_norito_rpc_smoke.sh` + `scripts/telemetry/test_torii_norito_rpc_alerts.sh`.
- **Продакшн‑промо** — выполните шаблон stage‑отчёта из
  `docs/source/torii/norito_rpc_stage_reports.md`. Зафиксируйте hash конфига,
  hash allowlist, digest smoke‑bundle, hash Grafana‑export и ID alert‑drill.
- **Rollback** — переключите `stage` обратно в `disabled`, оставьте allowlist и
  зафиксируйте переключение в stage‑отчёте и инцидент‑логе. После исправления
  причины повторите canary‑чек‑лист перед возвратом `stage=ga`.

## 3. Телеметрия и алерты

| Актив | Локация | Примечания |
|-------|----------|-------|
| Dashboard | `dashboards/grafana/torii_norito_rpc_observability.json` | Отслеживает rate запросов, коды ошибок, размеры payload, ошибки декодирования и % внедрения. |
| Alerts | `dashboards/alerts/torii_norito_rpc_rules.yml` | Gates `NoritoRpcErrorBudget`, `NoritoRpcDecodeFailures`, `NoritoRpcFallbackSpike`. |
| Chaos‑скрипт | `scripts/telemetry/test_torii_norito_rpc_alerts.sh` | Ломает CI при дрейфе выражений алертов. Запускайте после каждого изменения конфига. |
| Smoke‑тесты | `python/iroha_python/scripts/run_norito_rpc_smoke.sh`, `cargo xtask norito-rpc-verify` | Включайте их логи в evidence‑bundle при каждом продвижении. |

Дашборды нужно экспортировать и прикладывать к release‑тикету (`make
docs-portal-dashboards` в CI), чтобы on‑call могли повторить метрики без доступа
к production Grafana.

## 4. Частые вопросы

**Как разрешить новый SDK в canary?**  
Добавьте сервисный аккаунт/токен в `torii.transport.norito_rpc.allowed_clients`,
перезагрузите Torii и зафиксируйте изменение в `docs/source/torii/norito_rpc_tracker.md`
под NRPC-2R. Владелец SDK должен также снять fixture‑прогон через
`scripts/run_norito_rpc_fixtures.sh --sdk <label>`.

**Что делать, если декодирование Norito падает в середине rollout?**  
Оставьте `stage=canary`, держите `enabled=true` и разберите ошибки через
`torii_norito_decode_failures_total`. Владельцы SDK могут откатиться на JSON,
исключив `Accept: application/x-norito`; Torii продолжит отдавать JSON, пока
stage не вернётся к `ga`.

**Как доказать, что gateway отдаёт правильный манифест?**  
Запустите `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario norito-rpc --host
<gateway-host>`, чтобы проба записала заголовки `Sora-Proof` вместе с digest
конфигурации Norito. Приложите JSON‑вывод к stage‑отчёту.

**Где фиксировать overrides редактирования?**  
Документируйте каждый временный override в колонке `Notes` stage‑отчёта и
регистрируйте патч конфигурации Norito в change control. Overrides автоматически
истекают в конфиге; эта запись напоминает on‑call о необходимости cleanup после
инцидентов.

По вопросам вне этого FAQ используйте каналы из canary‑ранбука
(`docs/source/runbooks/torii_norito_rpc_canary.md`).

## 5. Шаблон release‑note (follow‑up OPS-NRPC)

Пункт **OPS-NRPC** требует готового фрагмента release‑note, чтобы операторы
единообразно объявляли rollout Norito‑RPC. Скопируйте блок ниже в следующий
release‑пост (замените поля в скобках) и приложите evidence‑bundle, описанный
ниже.

> **Транспорт Torii Norito-RPC** — Norito‑конверты теперь обслуживаются вместе с
> JSON API. Флаг `torii.transport.norito_rpc.stage` поставляется со значением
> **[stage: disabled/canary/ga]** и следует чек‑листу поэтапного rollout в
> `docs/source/torii/norito_rpc_rollout_plan.md`. Операторы могут временно выйти
> из режима, выставив `torii.transport.norito_rpc.stage=disabled` при сохранении
> `torii.transport.norito_rpc.enabled=true`; SDK автоматически вернутся к JSON.
> Дашборды телеметрии (`dashboards/grafana/torii_norito_rpc_observability.json`) и
> алерт‑drill (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) остаются
> обязательными перед повышением stage, а canary/smoke‑артефакты из
> `python/iroha_python/scripts/run_norito_rpc_smoke.sh` нужно приложить к
> релизному тикету.

Перед публикацией:

1. Замените **[stage: …]** на stage, объявленный в Torii.
2. Свяжите релизный тикет с последним stage‑отчётом в
   `docs/source/torii/norito_rpc_stage_reports.md`.
3. Загрузите упомянутые Grafana/Alertmanager exports вместе с hashes smoke‑bundle
   из `scripts/run_norito_rpc_smoke.sh`.

Этот фрагмент закрывает требование OPS-NRPC без необходимости каждый раз
переформулировать статус rollout во время инцидентов.
