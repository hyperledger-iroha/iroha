---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e45957522f3ac3ab0d003af79dc75bee1a2bf3c16d3aa8b6926f4c2b50a524a1
source_last_modified: "2025-11-15T20:10:15.053186+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: nexus-fee-model
title: Обновления модели комиссий Nexus
description: Зеркало `docs/source/nexus_fee_model.md`, документирующее квитанции расчетов по lane и поверхности согласования.
---

:::note Канонический источник
Эта страница отражает `docs/source/nexus_fee_model.md`. Держите обе копии синхронизированными, пока мигрируют переводы на японский, иврит, испанский, португальский, французский, русский, арабский и урду.
:::

# Обновления модели комиссий Nexus

Единый роутер расчетов теперь фиксирует детерминированные квитанции по каждой lane, чтобы операторы могли сверять списания газа с моделью комиссий Nexus.

- За полной архитектурой роутера, политикой буферов, матрицей телеметрии и последовательностью rollout см. `docs/settlement-router.md`. Это руководство объясняет, как параметры, описанные здесь, связаны с поставкой roadmap NX-3 и как SRE должны мониторить роутер в продакшене.
- Конфигурация газового актива (`pipeline.gas.units_per_gas`) включает десятичное значение `twap_local_per_xor`, `liquidity_profile` (`tier1`, `tier2` или `tier3`) и `volatility_class` (`stable`, `elevated`, `dislocated`). Эти флаги подаются в settlement router, чтобы итоговая котировка XOR соответствовала каноническому TWAP и уровню haircut для lane.
- Каждая транзакция, оплачивающая газ, записывает `LaneSettlementReceipt`. Каждый receipt хранит предоставленный вызывающим идентификатор источника, локальную микро-сумму, XOR к немедленной оплате, ожидаемый XOR после haircut, фактическую вариацию (`xor_variance_micro`) и метку времени блока в миллисекундах.
- Исполнение блока агрегирует receipts по lane/dataspace и публикует их через `lane_settlement_commitments` в `/v2/sumeragi/status`. Итоги раскрывают `total_local_micro`, `total_xor_due_micro` и `total_xor_after_haircut_micro`, суммированные по блоку для ночных выгрузок согласования.
- Новый счетчик `total_xor_variance_micro` отслеживает, сколько запаса безопасности было израсходовано (разница между начисленным XOR и ожиданием после haircut), а `swap_metadata` документирует детерминированные параметры конверсии (TWAP, epsilon, liquidity profile и volatility_class), чтобы аудиторы могли проверить входные параметры котировки независимо от конфигурации рантайма.

Потребители могут отслеживать `lane_settlement_commitments` вместе с существующими снимками commitments по lane и dataspace, чтобы убедиться, что буферы комиссий, уровни haircut и исполнение swap соответствуют настроенной модели комиссий Nexus.
