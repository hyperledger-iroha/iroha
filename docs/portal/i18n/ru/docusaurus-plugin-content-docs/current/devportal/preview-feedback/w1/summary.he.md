---
lang: he
direction: rtl
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/devportal/preview-feedback/w1/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5df22c97344663a9336423e59b80c095c322df1b845ca017e7abaa41d69bc870
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: preview-feedback-w1-summary
lang: ru
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

| Пункт | Детали |
| --- | --- |
| Волна | W1 - партнеры и интеграторы Torii |
| Окно приглашений | 2025-04-12 -> 2025-04-26 |
| Тег артефакта | `preview-2025-04-12` |
| Трекер | `DOCS-SORA-Preview-W1` |
| Участники | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## Основные моменты

1. **Workflow checksum** - Все reviewers проверили descriptor/archive через `scripts/preview_verify.sh`; логи сохранены рядом с подтверждениями приглашения.
2. **Телеметрия** - Дашборды `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` оставались зелеными на протяжении всей волны; инцидентов или alert pages не было.
3. **Feedback по docs (`docs-preview/w1`)** - Зафиксированы две небольшие правки:
   - `docs-preview/w1 #1`: уточнить навигационную формулировку в разделе Try it (закрыто).
   - `docs-preview/w1 #2`: обновить скриншот Try it (закрыто).
4. **Паритет runbook** - Операторы SoraFS подтвердили, что новые cross-links между `orchestrator-ops` и `multi-source-rollout` закрыли их замечания W0.

## Пункты действий

| ID | Описание | Владелец | Статус |
| --- | --- | --- | --- |
| W1-A1 | Обновить навигационную формулировку Try it по `docs-preview/w1 #1`. | Docs-core-02 | ✅ Завершено (2025-04-18). |
| W1-A2 | Обновить скриншот Try it по `docs-preview/w1 #2`. | Docs-core-03 | ✅ Завершено (2025-04-19). |
| W1-A3 | Свести выводы партнеров и телеметрию в roadmap/status. | Docs/DevRel lead | ✅ Завершено (см. tracker + status.md). |

## Итоговое резюме (2025-04-26)

- Все восемь reviewers подтвердили завершение во время финальных office hours, очистили локальные артефакты и получили отзыв доступа.
- Телеметрия оставалась зеленой до выхода; финальные snapshots приложены к `DOCS-SORA-Preview-W1`.
- Лог приглашений обновлен подтверждениями выхода; tracker отметил W1 как 🈴 и добавил checkpoints.
- Bundle доказательств (descriptor, checksum log, probe output, Try it proxy transcript, telemetry screenshots, feedback digest) архивирован в `artifacts/docs_preview/W1/`.

## Следующие шаги

- Подготовить план community intake W2 (governance approval + правки request template).
- Обновить preview artefact tag для волны W2 и перезапустить preflight скрипт после финализации дат.
- Перенести применимые выводы W1 в roadmap/status, чтобы community wave получила актуальные рекомендации.
