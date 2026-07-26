---
lang: ru
direction: ltr
source: docs/examples/soranet_gateway_billing/reconciliation_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5c10cd7eda24260bfd1319c7b8ac23dba2a1c8a1cb39ea49f0f1a64427ca15db
source_last_modified: "2025-11-21T12:24:49.353535+00:00"
translation_last_reviewed: 2026-01-01
---

# Сверка биллинга шлюза SoraGlobal

- **Окно:** `<from>/<to>`
- **Tenant:** `<tenant-id>`
- **Версия каталога:** `<catalog-version>`
- **Снимок использования:** `<path or hash>`
- **Guardrails:** soft cap `<soft-cap-xor> XOR`, hard cap `<hard-cap-xor> XOR`, порог алерта `<alert-threshold>%`
- **Плательщик -> Казначейство:** `<payer>` -> `<treasury>` в `<asset-definition>`
- **Итого к оплате:** `<total-xor> XOR` (`<total-micros>` micro-XOR)

## Проверки строк начисления
- [ ] Записи использования содержат только meter id из каталога и валидные регионы биллинга
- [ ] Единицы количества соответствуют определениям каталога (requests, GiB, ms, etc.)
- [ ] Региональные множители и уровни скидок применены согласно каталогу
- [ ] Экспорты CSV/Parquet совпадают с line items счета JSON

## Оценка guardrails
- [ ] Достигнут порог алерта soft cap? `<yes/no>` (если yes, приложить доказательства алерта)
- [ ] Превышен hard cap? `<yes/no>` (если yes, приложить approval override)
- [ ] Минимальный порог счета соблюден

## Проекция на ledger
- [ ] Сумма batch перевода равна `total_micros` в счете
- [ ] Определение asset соответствует валюте биллинга
- [ ] Аккаунты плательщика и казначейства соответствуют tenant и зарегистрированному оператору
- [ ] Артефакты Norito/JSON приложены для audit replay

## Примечания по спору/корректировке
- Наблюдаемая разница: `<variance detail>`
- Предлагаемая корректировка: `<delta and rationale>`
- Подтверждающие доказательства: `<logs/dashboards/alerts>`

## Одобрения
- Billing analyst: `<name + signature>`
- Treasury reviewer: `<name + signature>`
- Governance packet hash: `<hash/reference>`
