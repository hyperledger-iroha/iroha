---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo de taxa de nexo
título: Atualizações do modelo de taxas do Nexus
descrição: Espelho de `docs/source/nexus_fee_model.md`, documentando recibos de liquidação de vias e superfícies de conciliação.
---

:::nota Fonte canônica
Esta página reflete `docs/source/nexus_fee_model.md`. Mantenha as duas cópias homologadas enquanto as traduções japonesas, hebraicas, espanholas, portuguesas, francesas, russas, árabes e urdu migram.
:::

# Atualizações do modelo de taxas do Nexus

O roteador de liquidação unificado agora captura recibos deterministas por via para que os operadores possam conciliar débitos de gás com o modelo de taxas do Nexus.

- Para a arquitetura completa do roteador, política de buffer, matriz de telemetria e sequenciamento de rollout, veja `docs/settlement-router.md`. Esse guia explica como os parâmetros documentados aqui se conectam ao entregavel do roadmap NX-3 e como os SREs devem monitorar o roteador em produção.
- A configuração do ativo de gás (`pipeline.gas.units_per_gas`) inclui um decimal `twap_local_per_xor`, um `liquidity_profile` (`tier1`, `tier2`, ou `tier3`), e uma `volatility_class` (`stable`, `elevated`, `dislocated`). Esses sinalizadores alimentam o roteador de liquidação para que a cotacao XOR resulte em correspondência ao TWAP canônico e ao nível de corte de cabelo da pista.
- Cada transação que paga gás registra um `LaneSettlementReceipt`. Cada recibo armazenado o identificador de origem fornecido pelo chamador, o micro-valor local, o XOR imediatamente devido, o XOR esperado após o corte de cabelo, a variação realizada (`xor_variance_micro`) e o timestamp do bloco em milissegundos.
- A execução de blocos agregados recibos por lane/dataspace e os públicos via `lane_settlement_commitments` em `/v2/sumeragi/status`. Os totais mostram `total_local_micro`, `total_xor_due_micro` e `total_xor_after_haircut_micro` somados no bloco para exportações noturnas de conciliação.
- Um novo contador `total_xor_variance_micro` rastreia quanto de margem de segurança foi consumida (diferença entre o XOR devido e o esperado pós-haircut), e `swap_metadata` documenta os parâmetros de conversação determinística (TWAP, epsilon, perfil de liquidez e volatilidade_class) para que os auditores possam verificar as entradas da cotação independentemente da configuração em tempo de execução.

Os consumidores podem acompanhar o `lane_settlement_commitments` junto com os snapshots existentes de compromissos de faixa e espaço de dados para verificar se os buffers de taxas, os níveis de corte de cabelo e a execução do swap incluem ao modelo de taxas do Nexus configurado.