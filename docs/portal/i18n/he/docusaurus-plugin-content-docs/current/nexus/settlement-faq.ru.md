---
lang: he
direction: rtl
source: docs/portal/docs/nexus/settlement-faq.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-settlement-faq
כותרת: שאלות נפוצות בהתנחלות
תיאור: Ответы для операторов о маршрутизации התיישבות, конвертации в XOR, телеметрии ו аудиторских доказательствах.
---

Эта страница зеркалирует внутренний שאלות נפוצות על הסדר (`docs/source/nexus_settlement_faq.md`), чтобы читатели портала могли изучать тец поиска в mono-repo. Здесь объясняется, как Settlement Router обрабатывает выплаты, какие метрики отслеживать и как SDK должнипин нагрузки Norito.

## Основные моменты

1. **Sopоставление lane** — каждый dataspace объявляет `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` `xor_hosted_custody`). Смотрите актуальный каталог ליין ב-`docs/source/project_tracker/nexus_config_deltas/`.
2. **Детерминированная конвертация** — роутер переводит все settlement в XOR через источники ликвидности, утвержеменные. Приватные lane заранее пополняют XOR-буферы; תספורות применяются только когда буферы выходят за пределы политики.
3. **Телеметрия** — отслеживайте `nexus_settlement_latency_seconds`, счетчики конвертации датчики תספורת. Дашборды находятся в `dashboards/grafana/nexus_settlement.json`, а алерты в `dashboards/alerts/nexus_audit_rules.yml`.
4. ** Доказательства** — архивируйте конфиги, логи роутера, эксPORT телеметрии отчеты по сверке для аудки.
5. **Обязанности SDK** — каждый SDK должен предоставлять помощники התנחלות, נתיב תעודות זהות ומטעני מטען Norito, паритет с роутером.

## Примеры потоков

| Тип ליין | Какие доказательства собрать | Что это подтверждает |
|-----------|------------------------|----------------|
| Приватная `xor_hosted_custody` | Лог роутера + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | CBDC-буферы списывают детерминированный XOR, а תספורות остаются в пределах политики. |
| Публичная `xor_global` | מכשירי חשמל + ציוד ל-DEX/TWAP + מטריות לתקשורת/קונבורה | Общий путь ликвидности оценил перевод по опубликованному תספורת TWAP עם нулевым. |
| Гибридная `xor_dual_fund` | Лог роутера, показывающий разделение public vs shielded + счетчики телеметрии | Смесь shielded/public соблюдала коэффициенты управления и зафиксировала תספורת для каждой части. |

## אין עוד כרטיס?

- שאלות נפוצות: `docs/source/nexus_settlement_faq.md`
- נתב התיישבות Спецификация: `docs/source/settlement_router.md`
- פוליבוק CBDC: `docs/source/cbdc_lane_playbook.md`
- הפעלת ספר הפעלה: [Операции Nexus](./nexus-operations)