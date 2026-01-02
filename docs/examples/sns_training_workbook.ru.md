---
lang: ru
direction: ltr
source: docs/examples/sns_training_workbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d6965998c392217380a1722e49098f831438e2f4499b9e3258398a66f905a35
source_last_modified: "2025-11-15T09:17:29.843566+00:00"
translation_last_reviewed: 2026-01-01
---

# Шаблон учебной тетради SNS

Используйте эту тетрадь как канонический раздаточный материал для каждой учебной когорты. Замените плейсхолдеры (`<...>`) перед раздачей участникам.

## Данные сессии
- Суффикс: `<.sora | .nexus | .dao>`
- Цикл: `<YYYY-MM>`
- Язык: `<ar/es/fr/ja/pt/ru/ur>`
- Фасилитатор: `<name>`

## Лаб 1 — Экспорт KPI
1. Откройте KPI-дашборд портала (`docs/portal/docs/sns/kpi-dashboard.md`).
2. Отфильтруйте по суффиксу `<suffix>` и временному окну `<window>`.
3. Экспортируйте снимки PDF + CSV.
4. Запишите SHA-256 экспортированного JSON/PDF здесь: `______________________`.

## Лаб 2 — Drill по manifest
1. Заберите пример manifest из `artifacts/sns/training/<suffix>/<cycle>/manifests/<lang>.json`.
2. Проверьте через `cargo run --bin sns_manifest_check -- --input <file>`.
3. Сгенерируйте скелет resolver через `scripts/sns_zonefile_skeleton.py`.
4. Вставьте резюме diff:
   ```
   <git diff output>
   ```

## Лаб 3 — Симуляция спора
1. Используйте CLI guardian для запуска freeze (case id `<case-id>`).
2. Запишите хеш спора: `______________________`.
3. Загрузите лог доказательств в `artifacts/sns/training/<suffix>/<cycle>/logs/`.

## Лаб 4 — Автоматизация annex
1. Экспортируйте JSON дашборда Grafana и скопируйте его в `artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json`.
2. Запустите:
   ```bash
   cargo xtask sns-annex      --suffix <suffix>      --cycle <cycle>      --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --output docs/source/sns/reports/<suffix>/<cycle>.md      --regulatory-entry docs/source/sns/regulatory/<memo>.md      --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```
3. Вставьте путь к annex + вывод SHA-256: `________________________________`.

## Заметки по обратной связи
- Что было неясно?
- Какие лабы вышли за время?
- Какие баги в tooling заметили?

Верните заполненные тетради фасилитатору; они должны храниться в
`artifacts/sns/training/<suffix>/<cycle>/workbooks/`.
