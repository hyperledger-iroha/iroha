---
lang: ru
direction: ltr
source: docs/examples/soranet_gar_intake_form.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6cd4da7e590d581719ed2607994d7d9eb16d153fbd06f85655d0da37c727853a
source_last_modified: "2025-11-21T15:16:55.013762+00:00"
translation_last_reviewed: 2026-01-01
---

# Шаблон запроса GAR для SoraNet

Используйте эту форму при запросе действия GAR (purge, ttl override, rate ceiling, moderation
directive, geofence, или legal hold). Отправленную форму следует прикрепить рядом с
результатами `gar_controller`, чтобы журналы аудита и квитанции ссылались на одни и те же
URI доказательств.

| Поле | Значение | Примечания |
|------|----------|------------|
| ID запроса |  | ID тикета guardian/ops. |
| Запрошено |  | Аккаунт + контакт. |
| Дата/время (UTC) |  | Когда должно начаться действие. |
| Имя GAR |  | например, `docs.sora`. |
| Канонический хост |  | например, `docs.gw.sora.net`. |
| Действие |  | `ttl_override` / `rate_limit_override` / `purge_static_zone` / `geo_fence` / `legal_hold` / `moderation`. |
| Переопределение TTL (секунды) |  | Требуется только для `ttl_override`. |
| Потолок скорости (RPS) |  | Требуется только для `rate_limit_override`. |
| Разрешенные регионы |  | Список регионов ISO при запросе `geo_fence`. |
| Запрещенные регионы |  | Список регионов ISO при запросе `geo_fence`. |
| Слаги модерации |  | Должно соответствовать директивам модерации GAR. |
| Теги purge |  | Теги, которые нужно очистить перед выдачей. |
| Метки |  | Машинные метки (incident id, drill name, pop scope). |
| URI доказательств |  | Логи/дашборды/спеки, подтверждающие запрос. |
| URI аудита |  | Audit URI по pop, если отличается от defaults. |
| Запрошенный срок |  | Unix timestamp или RFC3339; оставить пустым для default. |
| Причина |  | Пояснение для пользователей; отображается в квитанциях и дашбордах. |
| Утверждающий |  | Утверждающий guardian/committee для запроса. |

### Шаги подачи

1. Заполните таблицу и прикрепите ее к тикету governance.
2. Обновите конфиг GAR controller (`policies`/`pops`) с совпадающими `labels`/`evidence_uris`/`expires_at_unix`.
3. Запустите `cargo xtask soranet-gar-controller ...`, чтобы выпустить события/квитанции.
4. Добавьте `gar_controller_summary.json`, `gar_reconciliation_report.json`, `gar_metrics.prom` и `gar_audit_log.jsonl` в тот же тикет. Утверждающий подтверждает, что количество квитанций совпадает со списком PoP перед отправкой.
