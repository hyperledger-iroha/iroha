---
lang: ru
direction: ltr
source: docs/examples/sns_training_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd9da5045f5f40dbc31837145ad13bf79b4d751b0803c0b6d69bab49885ed1b4
source_last_modified: "2025-11-15T09:17:21.371048+00:00"
translation_last_reviewed: 2026-01-01
---

# Шаблон слайдов обучения SNS

Этот Markdown-скелет повторяет слайды, которые фасилитаторы должны адаптировать для языковых когорт. Скопируйте эти разделы в Keynote/PowerPoint/Google Slides и локализуйте пункты, скриншоты и диаграммы по мере необходимости.

## Титульный слайд
- Программа: «Sora Name Service onboarding»
- Подзаголовок: укажите суффикс + цикл (например, `.sora - 2026-03`)
- Докладчики + организации

## Ориентация по KPI
- Скриншот или embed `docs/portal/docs/sns/kpi-dashboard.md`
- Список пунктов с пояснениями по фильтрам суффиксов, таблице ARPU и трекеру freeze
- Врезки про экспорт PDF/CSV

## Жизненный цикл manifest
- Диаграмма: registrar -> Torii -> governance -> DNS/gateway
- Шаги со ссылкой на `docs/source/sns/registry_schema.md`
- Пример фрагмента manifest с аннотациями

## Тренировки по спорам и freeze
- Диаграмма потока для вмешательства guardian
- Чеклист со ссылкой на `docs/source/sns/governance_playbook.md`
- Пример таймлайна freeze-тикета

## Сбор annex
- Фрагмент команды с `cargo xtask sns-annex ... --portal-entry ...`
- Напоминание архивировать Grafana JSON в `artifacts/sns/regulatory/<suffix>/<cycle>/`
- Ссылка на `docs/source/sns/reports/.<suffix>/<cycle>.md`

## Следующие шаги
- Ссылка на обратную связь (см. `docs/examples/sns_training_eval_template.md`)
- Контакты каналов Slack/Matrix
- Даты ближайших вех
