---
lang: ru
direction: ltr
source: docs/source/nexus_fee_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 532c57a0dae54224af0d30640edf8a3cbc8ac9a1df7d73b563bd16c3a635aec1
source_last_modified: "2026-01-08T19:45:50.411145+00:00"
translation_last_reviewed: 2026-01-08
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
- Транзакции IVM должны содержать метаданные `gas_limit` (`u64`, > 0), чтобы ограничить риск по комиссиям.
  Эндпоинт `/v2/contracts/call` требует `gas_limit` явно, а некорректные значения отклоняются.
- Когда транзакция задает метаданные `fee_sponsor`, спонсор должен выдать вызывающему
  `CanUseFeeSponsor { sponsor }`. Попытки спонсирования без разрешения отклоняются и фиксируются.
- Каждая транзакция, оплачивающая gas, записывает `LaneSettlementReceipt`. Каждый receipt хранит
  source identifier, переданный вызывающей стороной, локальную micro-amount, XOR к немедленной оплате,
  XOR после haircut, реализованный safety margin (`xor_variance_micro`) и timestamp блока в
  миллисекундах.
- Исполнение блока агрегирует receipts по lane/dataspace и публикует их через
  `lane_settlement_commitments` в `/v2/sumeragi/status`. Итоги выставляют `total_local_micro`,
  `total_xor_due_micro` и `total_xor_after_haircut_micro`, суммированные по блоку для ночных выгрузок
  reconciliation.
- Новый счетчик `total_xor_variance_micro` отслеживает, сколько safety margin было израсходовано
  (разница между due XOR и post-haircut ожиданием), а `swap_metadata` документирует детерминированные
  параметры конверсии (TWAP, epsilon, liquidity profile, volatility_class), чтобы аудиторы могли
  проверять входные данные котировки независимо от runtime конфигурации.

Потребители могут отслеживать `lane_settlement_commitments` вместе с существующими snapshots
commitments для lane и dataspace, чтобы убедиться, что fee buffers, haircut tiers и исполнение swap
соответствуют настроенной модели комиссий Nexus.
