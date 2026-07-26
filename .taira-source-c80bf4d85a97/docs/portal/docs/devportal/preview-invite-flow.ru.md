---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1e6e4dda03f047326084118775695b76067c683eaf382388147a87518e45691e
source_last_modified: "2025-12-19T22:35:07.713078+00:00"
translation_last_reviewed: 2026-01-01
---

# Поток приглашений preview

## Назначение

Пункт дорожной карты **DOCS-SORA** указывает onboarding ревьюеров и программу приглашений public preview как последние блокеры перед выходом портала из beta. Эта страница описывает, как открывать каждую волну приглашений, какие артефакты должны быть отправлены перед рассылкой и как доказать, что поток аудируем. Используйте вместе с:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) для работы с каждым ревьюером.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) для гарантий checksum.
- [`devportal/observability`](./observability.md) для экспорта телеметрии и hooks оповещений.

## План волн

| Волна | Аудитория | Критерии входа | Критерии выхода | Примечания |
| --- | --- | --- | --- | --- |
| **W0 - Core maintainers** | Maintainers Docs/SDK, валидирующие контент дня один. | Команда GitHub `docs-portal-preview` заполнена, gate checksum в `npm run serve` зеленый, Alertmanager тихий 7 дней. | Все P0 доки просмотрены, backlog промаркирован, блокирующих инцидентов нет. | Используется для проверки потока; email-инвайтов нет, только обмен preview артефактами. |
| **W1 - Partners** | Операторы SoraFS, интеграторы Torii, ревьюеры governance под NDA. | W0 завершена, юридические условия утверждены, Try-it proxy в staging. | Собран sign-off партнеров (issue или подписанная форма), телеметрия показывает <=10 одновременных ревьюеров, нет регрессий безопасности 14 дней. | Обязать шаблон приглашения + request tickets. |
| **W2 - Community** | Избранные участники из community waitlist. | W1 завершена, инцидентные учения проведены, публичный FAQ обновлен. | Фидбек обработан, >=2 документационных релиза прошли через preview pipeline без rollback. | Ограничить одновременные приглашения (<=25) и батчить еженедельно. |

Документируйте активную волну в `status.md` и в preview request tracker, чтобы governance видел статус с первого взгляда.

## Preflight чеклист

Выполните эти действия **перед** планированием приглашений для волны:

1. **Артефакты CI доступны**
   - Последний `docs-portal-preview` + descriptor загружен `.github/workflows/docs-portal-preview.yml`.
   - SoraFS pin отмечен в `docs/portal/docs/devportal/deploy-guide.md` (descriptor cutover присутствует).
2. **Принудительный checksum**
   - `docs/portal/scripts/serve-verified-preview.mjs` вызывается через `npm run serve`.
   - Инструкции `scripts/preview_verify.sh` протестированы на macOS + Linux.
3. **Базовая телеметрия**
   - `dashboards/grafana/docs_portal.json` показывает здоровый трафик Try it, и алерт `docs.preview.integrity` зеленый.
   - Последнее приложение в `docs/portal/docs/devportal/observability.md` обновлено ссылками Grafana.
4. **Артефакты governance**
   - Issue invite tracker готов (одна issue на волну).
   - Шаблон реестра ревьюеров скопирован (см. [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Юридические и SRE approvals приложены к issue.

Зафиксируйте завершение preflight в invite tracker перед отправкой писем.

## Шаги потока

1. **Выбор кандидатов**
   - Взять из waitlist spreadsheet или partner queue.
   - Убедиться, что у каждого кандидата заполнен request template.
2. **Одобрение доступа**
   - Назначить approver на issue invite tracker.
   - Проверить требования (CLA/контракт, acceptable use, security brief).
3. **Отправка приглашений**
   - Заполнить плейсхолдеры в [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, контакты).
   - Приложить descriptor + hash архива, Try it staging URL и каналы поддержки.
   - Сохранить финальное письмо (или Matrix/Slack transcript) в issue.
4. **Отслеживание onboarding**
   - Обновить invite tracker полями `invite_sent_at`, `expected_exit_at`, и статусом (`pending`, `active`, `complete`, `revoked`).
   - Привязать intake request ревьюера для аудитабельности.
5. **Мониторинг телеметрии**
   - Следить за `docs.preview.session_active` и алертами `TryItProxyErrors`.
   - Завести инцидент при отклонении телеметрии от baseline и записать результат рядом с записью приглашения.
6. **Сбор фидбека и выход**
   - Закрыть приглашения после получения фидбека или наступления `expected_exit_at`.
   - Обновить issue волны кратким резюме (находки, инциденты, следующие шаги) перед переходом к следующей когорте.

## Evidence и отчетность

| Артефакт | Где хранить | Частота обновления |
| --- | --- | --- |
| Issue invite tracker | GitHub проект `docs-portal-preview` | Обновлять после каждого приглашения. |
| Экспорт roster ревьюеров | Реестр, связанный в `docs/portal/docs/devportal/reviewer-onboarding.md` | Еженедельно. |
| Снимки телеметрии | `docs/source/sdk/android/readiness/dashboards/<date>/` (переиспользовать telemetry bundle) | На каждую волну + после инцидентов. |
| Feedback digest | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (создать папку на волну) | В течение 5 дней после выхода из волны. |
| Governance meeting note | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Заполнять до каждого DOCS-SORA governance sync. |

Запускайте `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
после каждого батча, чтобы получить машинно-читабельный digest. Прикрепите рендеренный JSON к issue волны, чтобы ревьюеры governance могли подтвердить количество приглашений без воспроизведения всего лога.

Прикрепляйте список evidence к `status.md` при завершении каждой волны, чтобы дорожную карту можно было быстро обновить.

## Критерии rollback и паузы

Приостановите поток приглашений (и уведомите governance), если происходит что-либо из следующего:

- Инцидент Try it proxy, потребовавший rollback (`npm run manage:tryit-proxy`).
- Усталость от алертов: >3 alert pages для preview-only endpoints в течение 7 дней.
- Пробел комплаенса: приглашение отправлено без подписанных условий или без записи request template.
- Риск целостности: checksum mismatch, обнаруженный `scripts/preview_verify.sh`.

Возобновляйте только после документирования remediation в invite tracker и подтверждения стабильности телеметрии минимум 48 часов.
