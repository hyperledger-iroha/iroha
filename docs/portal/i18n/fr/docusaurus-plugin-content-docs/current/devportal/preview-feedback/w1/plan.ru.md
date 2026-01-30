---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: preview-feedback-w1-plan
title: План preflight для партнеров W1
sidebar_label: План W1
description: Задачи, владельцы и чек-лист доказательств для партнерской preview-когорты.
---

| Пункт | Детали |
| --- | --- |
| Волна | W1 - партнеры и интеграторы Torii |
| Целевое окно | Q2 2025 неделя 3 |
| Тег артефакта (план) | `preview-2025-04-12` |
| Трекер | `DOCS-SORA-Preview-W1` |

## Цели

1. Получить юридические и governance-одобрения условий партнерского preview.
2. Подготовить Try it proxy и телеметрические снимки для пакета приглашений.
3. Обновить checksum-верифицированный preview-артефакт и результаты probe.
4. Финализировать список партнеров и шаблоны запросов до отправки приглашений.

## Разбивка задач

| ID | Задача | Владелец | Срок | Статус | Примечания |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Получить юридическое одобрение на дополнение условий preview | Docs/DevRel lead -> Legal | 2025-04-05 | ✅ Завершено | Юридический тикет `DOCS-SORA-Preview-W1-Legal` одобрен 2025-04-05; PDF приложен к трекеру. |
| W1-P2 | Зафиксировать staging-окно Try it proxy (2025-04-10) и проверить здоровье прокси | Docs/DevRel + Ops | 2025-04-06 | ✅ Завершено | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` выполнено 2025-04-06; транскрипт CLI и `.env.tryit-proxy.bak` архивированы. |
| W1-P3 | Собрать preview-артефакт (`preview-2025-04-12`), запустить `scripts/preview_verify.sh` + `npm run probe:portal`, архивировать descriptor/checksums | Portal TL | 2025-04-08 | ✅ Завершено | Артефакт и логи проверки сохранены в `artifacts/docs_preview/W1/preview-2025-04-12/`; вывод probe приложен к трекеру. |
| W1-P4 | Проверить intake-формы партнеров (`DOCS-SORA-Preview-REQ-P01...P08`), подтвердить контакты и NDA | Governance liaison | 2025-04-07 | ✅ Завершено | Все восемь запросов одобрены (последние два 2025-04-11); ссылки на одобрения в трекере. |
| W1-P5 | Подготовить текст приглашения (на базе `docs/examples/docs_preview_invite_template.md`), задать `<preview_tag>` и `<request_ticket>` для каждого партнера | Docs/DevRel lead | 2025-04-08 | ✅ Завершено | Черновик приглашения отправлен 2025-04-12 15:00 UTC вместе с ссылками на артефакт. |

## Preflight чек-лист

> Совет: запустите `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json`, чтобы автоматически выполнить шаги 1-5 (build, проверка checksum, portal probe, link checker и обновление Try it proxy). Скрипт записывает JSON-лог, который можно приложить к issue трекера.

1. `npm run build` (с `DOCS_RELEASE_TAG=preview-2025-04-12`) для пересоздания `build/checksums.sha256` и `build/release.json`.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` и архивировать `build/link-report.json` рядом с descriptor.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (или указать нужный target через `--tryit-target`); закоммитить обновленный `.env.tryit-proxy` и сохранить `.bak` для rollback.
6. Обновить issue W1 путями логов (checksum descriptor, вывод probe, изменение Try it proxy и Grafana snapshots).

## Чек-лист доказательств

- [x] Подписанное юридическое одобрение (PDF или ссылка на тикет) приложено к `DOCS-SORA-Preview-W1`.
- [x] Grafana скриншоты для `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] Descriptor и checksum-лог `preview-2025-04-12` сохранены в `artifacts/docs_preview/W1/`.
- [x] Таблица roster приглашений с заполненными `invite_sent_at` (см. W1 log в трекере).
- [x] Артефакты обратной связи отражены в [`preview-feedback/w1/log.md`](./log.md) с одной строкой на партнера (обновлено 2025-04-26 данными roster/telemetria/issues).

Обновляйте этот план по мере продвижения; трекер ссылается на него, чтобы сохранить auditability roadmap.

## Процесс обратной связи

1. Для каждого reviewer дублировать шаблон
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   заполнить метаданные и сохранить готовую копию в
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Сводить приглашения, телеметрические checkpoints и открытые issues в живом логе
   [`preview-feedback/w1/log.md`](./log.md), чтобы governance reviewers могли воспроизвести волну
   полностью, не покидая репозиторий.
3. Когда приходят экспорты knowledge-check или опросов, прикреплять их по пути артефакта, указанному в логе,
   и связывать с issue трекера.
