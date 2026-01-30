---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
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
- Исполнение блока агрегирует receipts по lane/dataspace и публикует их через `lane_settlement_commitments` в `/v1/sumeragi/status`. Итоги раскрывают `total_local_micro`, `total_xor_due_micro` и `total_xor_after_haircut_micro`, суммированные по блоку для ночных выгрузок согласования.
- Новый счетчик `total_xor_variance_micro` отслеживает, сколько запаса безопасности было израсходовано (разница между начисленным XOR и ожиданием после haircut), а `swap_metadata` документирует детерминированные параметры конверсии (TWAP, epsilon, liquidity profile и volatility_class), чтобы аудиторы могли проверить входные параметры котировки независимо от конфигурации рантайма.

Потребители могут отслеживать `lane_settlement_commitments` вместе с существующими снимками commitments по lane и dataspace, чтобы убедиться, что буферы комиссий, уровни haircut и исполнение swap соответствуют настроенной модели комиссий Nexus.
