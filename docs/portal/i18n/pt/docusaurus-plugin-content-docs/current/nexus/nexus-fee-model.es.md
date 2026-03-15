---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo de taxa de nexo
título: Atualizações do modelo de tarifas de Nexus
descrição: Espejo de `docs/source/nexus_fee_model.md`, que documenta os recibos de liquidação de vias e as superfícies de conciliação.
---

:::nota Fonte canônica
Esta página reflete `docs/source/nexus_fee_model.md`. Mantenha ambas as cópias alinhadas enquanto as traduções para o japonês, hebraico, espanhol, português, francês, russo, árabe e urdu migran.
:::

# Atualizações do modelo de tarifas de Nexus

O roteador de liquidação unificado agora captura recibos deterministas por via para que os operadores possam conciliar os débitos de gás com o modelo de tarifas de Nexus.

- Para a arquitetura completa do roteador, a política de buffer, a matriz de telemetria e a sequência de despliegue, e `docs/settlement-router.md`. Este guia explica como os parâmetros documentados aqui são vinculados à entrega do roadmap NX-3 e como os SREs devem monitorar o roteador em produção.
- A configuração do ativo de gás (`pipeline.gas.units_per_gas`) inclui um decimal `twap_local_per_xor`, um `liquidity_profile` (`tier1`, `tier2`, ou `tier3`), e um `volatility_class` (`stable`, `elevated`, `dislocated`). Essas bandas alimentam o roteador de liquidação para que a cotização XOR resultante coincida com o TWAP canônico e o nível de corte de cabelo para a pista.
- Cada transação que paga gás registra um `LaneSettlementReceipt`. Cada recibo armazena o identificador de origem fornecido pelo chamador, o micro-monte local, o XOR debitado imediatamente, o XOR esperado após o corte de cabelo, a variação realizada (`xor_variance_micro`), e a marca de tempo do bloqueio em milissegundos.
- A execução de blocos agrega recibos por lane/dataspace e os publica via `lane_settlement_commitments` e `/v1/sumeragi/status`. Os totais expostos `total_local_micro`, `total_xor_due_micro`, e `total_xor_after_haircut_micro` somados sobre o bloco para as exportações noturnas de conciliação.
- Um novo contador `total_xor_variance_micro` rastreia quanto margem de segurança é consumida (diferença entre o XOR devido e a expectativa pós-haircut), e `swap_metadata` documenta os parâmetros de conversão deterministas (TWAP, epsilon, perfil de liquidez, e volatilidade_class) para que os auditores possam verificar as entradas de la inicialização independente da configuração em tempo de execução.

Os consumidores podem observar `lane_settlement_commitments` junto com os snapshots existentes de compromissos de faixa e espaço de dados para verificar se os buffers de tarifas, os níveis de corte de cabelo e a execução de swap coincidem com o modelo de tarifas de Nexus configurado.