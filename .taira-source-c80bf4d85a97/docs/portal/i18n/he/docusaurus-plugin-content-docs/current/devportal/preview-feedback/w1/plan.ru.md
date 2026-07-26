---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w1-plan
כותרת: План preflight для партнеров W1
sidebar_label: План W1
תיאור: Задачи, владельцы и чек-лист доказательств для партнерской preview-когорты.
---

| Пункт | Детали |
| --- | --- |
| Волна | W1 - партнеры и интеграторы Torii |
| Целевое окно | Q2 2025 неделя 3 |
| Тег артефакта (план) | `preview-2025-04-12` |
| Трекер | `DOCS-SORA-Preview-W1` |

## Цели

1. Получить юридические и governance-одобрения условий партнерского תצוגה מקדימה.
2. בדוק נסה את זה proxy и телеметрические снимки для пакета приглашений.
3. אופטימיזציה של checksum-верифицированный preview-артефакт и результаты בדיקה.
4. Финализировать список партнеров и шаблоны запросов до отправки приглашений.

## Разбивка задач

| תעודת זהות | Задача | Владелец | Срок | Статус | Примечания |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | תצוגה מקדימה של Получить юридическое одобрение на дополнение условий | Docs/DevRel lead -> משפטי | 2025-04-05 | ✅ Завершено | Юридический тикет `DOCS-SORA-Preview-W1-Legal` одобрен 2025-04-05; PDF приложен к трекеру. |
| W1-P2 | Зафиксировать staging-окно Try it proxy (2025-04-10) и проверить здоровье прокси | Docs/DevRel + Ops | 2025-04-06 | ✅ Завершено | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` выполнено 2025-04-06; транскрипт CLI ו `.env.tryit-proxy.bak` архивированы. |
| W1-P3 | Собрать preview-артефакт (`preview-2025-04-12`), запустить `scripts/preview_verify.sh` + `npm run probe:portal`, архивировать תיאור/סיכומי בדיקה | פורטל TL | 2025-04-08 | ✅ Завершено | Артефакт и логи проверки сохранены в `artifacts/docs_preview/W1/preview-2025-04-12/`; вывод בדיקה приложен к трекеру. |
| W1-P4 | צור צריכת פורמים (`DOCS-SORA-Preview-REQ-P01...P08`), צור קשרים ו-NDA | קשר ממשל | 2025-04-07 | ✅ Завершено | Все восемь запросов одобрены (последние два 2025-04-11); ссылки на одобрения в трекере. |
| W1-P5 | Подготовить текст приглашения (באמצעות `docs/examples/docs_preview_invite_template.md`), задать `<preview_tag>` ו-`<request_ticket>` עבור караждогого | Docs/DevRel lead | 2025-04-08 | ✅ Завершено | Черновик приглашения отправлен 2025-04-12 15:00 UTC вместе с ссылками на артефакт. |

## טיסה מוקדמת чек-лист

> סיווג: запустите `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json`, чтобы автоматически выполнить шаги 1-5 (בנה, הצגת סכום בדיקה, בדיקה בפורטל, נסה את בודק פרוקסי ואפשרות). סקריפט записывает JSON-лог, который можно приложить к issue трекера.

1. `npm run build` (עם `DOCS_RELEASE_TAG=preview-2025-04-12`) עבור пересоздания `build/checksums.sha256` ו-`build/release.json`.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` и архивировать `build/link-report.json` рядом с descriptor.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (или указать нужный target через `--tryit-target`); закоммитить обновленный `.env.tryit-proxy` ו- сохранить `.bak` לחזרה לאחור.
6. בטל את הבעיה W1 путями логов (מתאר בדיקת סכום, בדיקה, בדיקה, נסה את פרוקסי ותמונות מצב של Grafana).

## Чек-лист доказательств- [x] Подписанное юридическое одобрение (PDF или ссылка на тикет) приложено к `DOCS-SORA-Preview-W1`.
- [x] Grafana скриншоты ל-`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] Descriptor и checksum-лог `preview-2025-04-12` сохранены в `artifacts/docs_preview/W1/`.
- [x] סגל Таблица приглашений с заполненными `invite_sent_at` (см. W1 log в трекере).
- [x] Артефакты обратной связи отражены в [`preview-feedback/w1/log.md`](./log.md) с одной строкой на партнера-обн2026лера (обн2026лера) סגל/טלמטריה/בעיות).

Обновляйте этот план по мере продвижения; трекер ссылается на него, чтобы сохранить מפת הדרכים לביקורת.

## Процесс обратной связи

1. Для каждого מבקר дублировать шаблон
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   заполнить метаданные и сохранить готовую копию в
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. בדוק את הפריטים, נקודות המחסום והטלפונים האפשריים
   [`preview-feedback/w1/log.md`](./log.md), чтобы מבקרי ממשל могли воспроизвести волну
   полностью, не покидая репозиторий.
3. בדוק את הידע של הספורט או בדוק את האפשרויות, הצג את הביקורות על הסוכנות,
   и связывать с issue трекера.