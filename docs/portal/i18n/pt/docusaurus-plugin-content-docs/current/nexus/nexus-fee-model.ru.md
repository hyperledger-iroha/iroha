---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo de taxa de nexo
título: Modelo padrão Nexus
description: Зеркало `docs/source/nexus_fee_model.md`, документирующее квитанции расчетов по lane и поверхности согласования.
---

:::nota História Canônica
Esta página contém `docs/source/nexus_fee_model.md`. Держите обе копии синхронизированными, пока мигрируют переводы на японский, iврит, espanhol, português, francês, russo, árabe e урду.
:::

# Modelo de versão padrão Nexus

Единый роутер расчетов теперь фиксирует детерминированные квитанции по каждой lane, чтобы операторы могли сверять списания газа com modelo comercial Nexus.

- Para um roteador de arquiteto politizado, buffers políticos, distribuição de matrizes e implementação см. `docs/settlement-router.md`. Este é o caso, como parâmetros, descrição do plano, como definir o roteiro do NX-3 e como monitorar o monitor SRE роутер в продакшене.
- Конфигурация газового актива (`pipeline.gas.units_per_gas`) включает десятичное значение `twap_local_per_xor`, `liquidity_profile` (`tier1`, `tier2` ou `tier3`) e `volatility_class` (`stable`, `elevated`, `dislocated`). Essa bandeira pode ser usada no roteador de liquidação, isso é o mesmo que o XOR oferece canonizado TWAP e seu corte de cabelo para a pista.
- Caixa de transferência, оплачивающая газ, записывает `LaneSettlementReceipt`. Каждый recibo хранит предоставленный вызывающим идентификатор источника, локальную микро-сумму, XOR к немедленной оплате, ожидаемый XOR после corte de cabelo, фактическую вариацию (`xor_variance_micro`) e метку времени блока в миллисекундах.
- Organize um bloco de recibos para pista/espaço de dados e publique-o no `lane_settlement_commitments` em `/v2/sumeragi/status`. Итоги раскрывают `total_local_micro`, `total_xor_due_micro` e `total_xor_after_haircut_micro`, суммированные по блоку para ночных выгрузок согласования.
- A nova chave `total_xor_variance_micro` foi aberta, e a chave de segurança está disponível para uso начисленным XOR e ожиданием после corte de cabelo), um documento `swap_metadata` документирует детерминированные parâmetros de conversão (TWAP, épsilon, perfil de liquidez e volatilidade_class), Esses auditores podem fornecer seus parâmetros de configuração não configurados.

Você pode usar `lane_settlement_commitments` para obter compromissos claros na pista e no espaço de dados, что буферы комиссий, уровни corte de cabelo e исполнение swap соответствуют настроенной модели комиссий Nexus.