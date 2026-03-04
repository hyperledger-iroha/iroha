---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: nexus-settlement-faq
Название: الأسئلة الشائعة للتسوية
описание: Дэниел Уилсон, Дэниел Уилсон, XOR, Нэнил Бёрн. وأدلة التدقيق.
---

Установите флажок "Настроить блок управления двигателем" (`docs/source/nexus_settlement_faq.md`) Вы можете получить доступ к новому репозиторию в монорепозитории. Маршрутизатор поселений, который находится в Нижнем Новгороде, в Нью-Йорке. Для SDK используется Norito.

## أبرز النقاط

1. **Переулок** — область данных в пространстве `settlement_handle` (`xor_global` أو `xor_lane_weighted` أو `xor_hosted_custody` أو `xor_dual_fund`). راجع أحدث كتالوج Lane تحت `docs/source/project_tracker/nexus_config_deltas/`.
2. **Отключить соединение** — маршрутизатор выполняет функцию XOR, используя функцию XOR. В действительности. تقومlanes الخاصة بتمويل مخازن XOR مسبقا؛ Если вам нужны стрижки, вы можете сделать это прямо сейчас.
3. **القياس عن بعد** — راقب `nexus_settlement_latency_seconds` стрижка в стиле Navigator. Установите флажок для `dashboards/grafana/nexus_settlement.json` и установите для `dashboards/alerts/nexus_audit_rules.yml`.
4. **Установка** — подключение к маршрутизатору и подключение к нему маршрутизатора. Сделайте это.
5. **Внедрение SDK** – использование SDK в режиме онлайн-просмотра в Лейн-Федерации. Установите Norito для подключения к маршрутизатору.

## أمثلة على التدفقات

| Нюрн переулок | الأدلة المطلوبة | ماذا يثبت |
|-----------|--------------------|----------------|
| خاصة `xor_hosted_custody` | Маршрутизатор + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Он был создан для CBDC и XOR, а также для стрижки волос в ресторане CBDC. |
| عامة `xor_global` | Подключение маршрутизатора + поддержка DEX/TWAP + подключение к сети/телефонии | В 2017 году в журнале «Старый мир» и в TWAP появилась новая стрижка. |
| Код `xor_dual_fund` | Интернет-маршрутизатор общедоступный экранированный + экранированный маршрутизатор | يثبت أن المزج BTON, защищенный/публичный احترم احترم نسب الحوكمة وسجل, стрижка المطبق على كل جزء. |

## هل تحتاج مزيدا من التفاصيل؟

- Часто задаваемые вопросы: `docs/source/nexus_settlement_faq.md`.
- Маршрутизатор расчетов: `docs/source/settlement_router.md`.
- Доступ к CBDC: `docs/source/cbdc_lane_playbook.md`
- Код запроса: [عمليات Nexus](./nexus-operations)