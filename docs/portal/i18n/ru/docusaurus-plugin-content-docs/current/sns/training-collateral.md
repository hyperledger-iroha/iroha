---
lang: ru
direction: ltr
source: docs/portal/docs/sns/training-collateral.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: training-collateral
title: Учебные материалы SNS
description: Учебная программа, процесс локализации и фиксация доказательств приложений, требуемых SN-8.
---

> Отражает `docs/source/sns/training_collateral.md`. Используйте эту страницу при брифинге команд регистратора, DNS, guardian и финансов перед каждым запуском суффикса.

## 1. Срез программы

| Трек | Цели | Предварительное чтение |
|-------|------------|-----------|
| Операции регистратора | Отправлять манифесты, мониторить KPI дашборды, эскалировать ошибки. | `sns/onboarding-kit`, `sns/kpi-dashboard`. |
| DNS и gateway | Применять скелеты резолвера, репетировать freeze/rollback. | `sorafs/gateway-dns-runbook`, direct-mode policy samples. |
| Guardians и совет | Выполнять споры, обновлять аддендумы управления, логировать приложения. | `sns/governance-playbook`, steward scorecards. |
| Финансы и аналитика | Собирать метрики ARPU/bulk, публиковать пакеты приложений. | `finance/settlement-iso-mapping`, KPI dashboard JSON. |

### Поток модулей

1. **M1 — Ориентация по KPI (30 мин):** пройтись по фильтрам суффиксов, экспортам и счетчикам freeze. Артефакт: PDF/CSV снимки с digest SHA-256.
2. **M2 — Жизненный цикл манифеста (45 мин):** собрать и валидировать манифесты регистратора, генерировать скелеты резолвера через `scripts/sns_zonefile_skeleton.py`. Артефакт: git diff со скелетом + доказательство GAR.
3. **M3 — Тренировки по спорам (40 мин):** симулировать freeze + апелляцию guardian, сохранить CLI логи в `artifacts/sns/training/<suffix>/<cycle>/logs/`.
4. **M4 — Сбор приложений (25 мин):** экспортировать JSON дашборда и выполнить:

   ```bash
   cargo xtask sns-annex      --suffix <suffix>      --cycle <cycle>      --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --output docs/source/sns/reports/<suffix>/<cycle>.md      --regulatory-entry docs/source/sns/regulatory/<memo>.md      --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```

   Артефакт: обновленный Markdown приложения + регуляторный мемо + блоки портала.

## 2. Процесс локализации

- Языки: `ar`, `es`, `fr`, `ja`, `pt`, `ru`, `ur`.
- Каждый перевод находится рядом с исходным файлом (`docs/source/sns/training_collateral.<lang>.md`). Обновляйте `status` + `translation_last_reviewed` после актуализации.
- Материалы по языкам размещаются в `artifacts/sns/training/<suffix>/<lang>/<cycle>/` (slides/, workbooks/, recordings/, logs/).
- Запускайте `python3 scripts/sync_docs_i18n.py --lang <code>` после редактирования английского источника, чтобы переводчики увидели новый hash.

### Чеклист поставки

1. Обновите stub перевода (`status: complete`) после локализации.
2. Экспортируйте слайды в PDF и загрузите в каталог `slides/` по языкам.
3. Запишите walkthrough KPI ≤10 мин; добавьте ссылку в stub языка.
4. Создайте governance‑тикет с тегом `sns-training`, включающий digest слайдов/workbook, ссылки на запись и доказательства приложений.

## 3. Учебные активы

- Шаблон слайдов: `docs/examples/sns_training_template.md`.
- Шаблон workbook: `docs/examples/sns_training_workbook.md` (по одному на участника).
- Приглашения + напоминания: `docs/examples/sns_training_invite_email.md`.
- Форма оценки: `docs/examples/sns_training_eval_template.md` (ответы сохраняются в `artifacts/sns/training/<suffix>/<cycle>/feedback/`).

## 4. Расписание и метрики

| Цикл | Окно | Метрики | Примечания |
|-------|--------|---------|-------|
| 2026‑03 | После обзора KPI | Посещаемость %, digest приложения записан | `.sora` + `.nexus` cohorts |
| 2026‑06 | Перед GA `.dao` | Финансовая готовность ≥90 % | Включить обновление политики |
| 2026‑09 | Экспансия | Дрилаб по спору <20 мин, SLA приложения ≤2 дня | Согласовать с стимулями SN-7 |

Собирайте анонимные отзывы в `docs/source/sns/reports/sns_training_feedback.md`, чтобы следующие когорты улучшали локализацию и лабораторные упражнения.

