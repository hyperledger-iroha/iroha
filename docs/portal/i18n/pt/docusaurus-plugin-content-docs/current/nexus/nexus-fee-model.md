---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-fee-model
title: Atualizacoes do modelo de taxas do Nexus
description: Espelho de `docs/source/nexus_fee_model.md`, documentando recibos de liquidacao de lanes e superficies de conciliacao.
---

:::note Fonte canonica
Esta pagina reflete `docs/source/nexus_fee_model.md`. Mantenha as duas copias alinhadas enquanto as traducoes japonesas, hebraicas, espanholas, portuguesas, francesas, russas, arabes e urdu migram.
:::

# Atualizacoes do modelo de taxas do Nexus

O roteador de liquidacao unificado agora captura recibos deterministas por lane para que operadores possam conciliar debitos de gas com o modelo de taxas do Nexus.

- Para a arquitetura completa do roteador, politica de buffer, matriz de telemetria e sequenciamento de rollout, veja `docs/settlement-router.md`. Esse guia explica como os parametros documentados aqui se conectam ao entregavel do roadmap NX-3 e como os SREs devem monitorar o roteador em producao.
- A configuracao do ativo de gas (`pipeline.gas.units_per_gas`) inclui um decimal `twap_local_per_xor`, um `liquidity_profile` (`tier1`, `tier2`, ou `tier3`), e uma `volatility_class` (`stable`, `elevated`, `dislocated`). Esses sinalizadores alimentam o roteador de liquidacao para que a cotacao XOR resultante corresponda ao TWAP canonico e ao tier de haircut da lane.
- Cada transacao que paga gas registra um `LaneSettlementReceipt`. Cada recibo armazena o identificador de origem fornecido pelo chamador, o micro-valor local, o XOR devido imediatamente, o XOR esperado apos o haircut, a variancia realizada (`xor_variance_micro`) e o timestamp do bloco em milissegundos.
- A execucao de blocos agrega recibos por lane/dataspace e os publica via `lane_settlement_commitments` em `/v1/sumeragi/status`. Os totais mostram `total_local_micro`, `total_xor_due_micro` e `total_xor_after_haircut_micro` somados no bloco para exportacoes noturnas de conciliacao.
- Um novo contador `total_xor_variance_micro` rastreia quanto de margem de seguranca foi consumida (diferenca entre o XOR devido e o esperado pos-haircut), e `swap_metadata` documenta os parametros de conversao deterministica (TWAP, epsilon, liquidity profile e volatility_class) para que auditores possam verificar as entradas da cotacao independentemente da configuracao em runtime.

Os consumidores podem acompanhar `lane_settlement_commitments` junto com os snapshots existentes de commitments de lane e dataspace para verificar que os buffers de taxas, os tiers de haircut e a execucao do swap correspondem ao modelo de taxas do Nexus configurado.
