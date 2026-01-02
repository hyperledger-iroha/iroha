---
lang: ru
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/06-verification-report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf489427d0eba2beebfdefc44092730c3963cbd77e83669853f4e9681ac9fd2d
source_last_modified: "2025-11-21T14:25:28.923348+00:00"
translation_last_reviewed: 2026-01-01
---

## Отчет об проверке оператора (фаза T0)

- Имя оператора: ______________________
- ID descriptor relay: ______________________
- Дата отправки (UTC): ___________________
- Контактный email / matrix: ___________________

### Сводка чеклиста

| Пункт | Выполнено (Y/N) | Примечания |
|------|-----------------|------------|
| Аппаратное обеспечение и сеть проверены | | |
| Блок compliance применен | | |
| Envelope допуска проверен | | |
| Smoke тест guard rotation | | |
| Телеметрия собирается и дашборды активны | | |
| Brownout drill выполнен | | |
| Успех PoW tickets в пределах цели | | |

### Снимок метрик

- PQ ratio (`sorafs_orchestrator_pq_ratio`): ________
- Количество downgrade за последние 24ч: ________
- Средний RTT circuit (p95): ________ ms
- Медианное время решения PoW: ________ ms

### Вложения

Пожалуйста, приложите:

1. Hash support bundle relay (`sha256`): __________________________
2. Скриншоты дашбордов (PQ ratio, circuit success, PoW histogram).
3. Подписанный drill bundle (`drills-signed.json` + публичный ключ подписанта в hex и вложения).
4. Отчет по метрикам SNNet-10 (`cargo xtask soranet-testnet-metrics --input <snapshot> --out metrics-report.json`).

### Подпись оператора

Я подтверждаю, что указанная информация точна и все необходимые шаги выполнены.

Подпись: _________________________  Дата: ___________________
