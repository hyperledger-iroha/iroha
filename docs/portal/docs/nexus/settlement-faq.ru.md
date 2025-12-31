---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e992cea8d0c835b30bd9e91860f6b6f87bed79a2c25bd6d0544639685834f80c
source_last_modified: "2025-11-10T17:37:35.921927+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: nexus-settlement-faq
title: FAQ по settlement
description: Ответы для операторов о маршрутизации settlement, конвертации в XOR, телеметрии и аудиторских доказательствах.
---

Эта страница зеркалирует внутренний FAQ по settlement (`docs/source/nexus_settlement_faq.md`), чтобы читатели портала могли изучать те же рекомендации без поиска в mono-repo. Здесь объясняется, как Settlement Router обрабатывает выплаты, какие метрики отслеживать и как SDK должны интегрировать полезные нагрузки Norito.

## Основные моменты

1. **Сопоставление lane** — каждый dataspace объявляет `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` или `xor_dual_fund`). Смотрите актуальный каталог lane в `docs/source/project_tracker/nexus_config_deltas/`.
2. **Детерминированная конвертация** — роутер переводит все settlement в XOR через источники ликвидности, утвержденные управлением. Приватные lane заранее пополняют XOR-буферы; haircuts применяются только когда буферы выходят за пределы политики.
3. **Телеметрия** — отслеживайте `nexus_settlement_latency_seconds`, счетчики конвертации и датчики haircut. Дашборды находятся в `dashboards/grafana/nexus_settlement.json`, а алерты в `dashboards/alerts/nexus_audit_rules.yml`.
4. **Доказательства** — архивируйте конфиги, логи роутера, экспорт телеметрии и отчеты по сверке для аудитов.
5. **Обязанности SDK** — каждый SDK должен предоставлять помощники settlement, IDs lane и кодировщики payloads Norito, чтобы сохранить паритет с роутером.

## Примеры потоков

| Тип lane | Какие доказательства собрать | Что это подтверждает |
|-----------|--------------------|----------------|
| Приватная `xor_hosted_custody` | Лог роутера + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | CBDC-буферы списывают детерминированный XOR, а haircuts остаются в пределах политики. |
| Публичная `xor_global` | Лог роутера + ссылка на DEX/TWAP + метрики латентности/конвертации | Общий путь ликвидности оценил перевод по опубликованному TWAP с нулевым haircut. |
| Гибридная `xor_dual_fund` | Лог роутера, показывающий разделение public vs shielded + счетчики телеметрии | Смесь shielded/public соблюдала коэффициенты управления и зафиксировала haircut для каждой части. |

## Нужно больше деталей?

- Полный FAQ: `docs/source/nexus_settlement_faq.md`
- Спецификация settlement router: `docs/source/settlement_router.md`
- Плейбук политики CBDC: `docs/source/cbdc_lane_playbook.md`
- Runbook операций: [Операции Nexus](./nexus-operations)
