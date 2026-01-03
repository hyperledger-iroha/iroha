---
lang: ru
direction: ltr
source: docs/source/nexus_fee_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e02872dbcb6d92d8be4d40fc2864f28fc6564391640a6ea67768a1f837b57e0f
source_last_modified: "2025-11-15T20:09:59.438546+00:00"
translation_last_reviewed: 2026-01-01
---

# Обновления модели комиссий Nexus

Единый settlement router теперь фиксирует детерминированные per-lane receipts, чтобы операторы могли
сверять списания gas с моделью комиссий Nexus.

- Полную архитектуру роутера, политику буферов, матрицу телеметрии и последовательность rollout см. в
  `docs/settlement-router.md`. Там же объясняется, как параметры, описанные здесь, связаны с
  deliverable NX-3 и как SRE должны мониторить роутер в продакшене.
- Конфигурация gas asset (`pipeline.gas.units_per_gas`) включает decimal `twap_local_per_xor`,
  `liquidity_profile` (`tier1`, `tier2`, или `tier3`) и `volatility_class` (`stable`, `elevated`,
  `dislocated`). Эти флаги подаются в settlement router, чтобы итоговая XOR котировка совпадала с
  каноническим TWAP и haircut tier для lane.
- Каждая транзакция, оплачивающая gas, записывает `LaneSettlementReceipt`. Каждый receipt хранит
  source identifier, переданный вызывающей стороной, локальную micro-amount, XOR к немедленной оплате,
  XOR после haircut, реализованный safety margin (`xor_variance_micro`) и timestamp блока в
  миллисекундах.
- Исполнение блока агрегирует receipts по lane/dataspace и публикует их через
  `lane_settlement_commitments` в `/v1/sumeragi/status`. Итоги выставляют `total_local_micro`,
  `total_xor_due_micro` и `total_xor_after_haircut_micro`, суммированные по блоку для ночных выгрузок
  reconciliation.
- Новый счетчик `total_xor_variance_micro` отслеживает, сколько safety margin было израсходовано
  (разница между due XOR и post-haircut ожиданием), а `swap_metadata` документирует детерминированные
  параметры конверсии (TWAP, epsilon, liquidity profile, volatility_class), чтобы аудиторы могли
  проверять входные данные котировки независимо от runtime конфигурации.

Потребители могут отслеживать `lane_settlement_commitments` вместе с существующими snapshots
commitments для lane и dataspace, чтобы убедиться, что fee buffers, haircut tiers и исполнение swap
соответствуют настроенной модели комиссий Nexus.
