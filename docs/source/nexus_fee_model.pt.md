---
lang: pt
direction: ltr
source: docs/source/nexus_fee_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 532c57a0dae54224af0d30640edf8a3cbc8ac9a1df7d73b563bd16c3a635aec1
source_last_modified: "2026-01-08T19:45:50.411145+00:00"
translation_last_reviewed: 2026-01-08
---

# Atualizacoes do modelo de taxas Nexus

O roteador de settlement unificado agora captura recibos deterministas por lane para que operadores
reconciliem os debitos de gas com o modelo de taxas Nexus.

- Para a arquitetura completa do roteador, politica de buffer, matriz de telemetria e sequenciamento
  de rollout, veja `docs/settlement-router.md`. Esse guia explica como os parametros documentados aqui
  se conectam ao entregavel de roadmap NX-3 e como os SREs devem monitorar o roteador em producao.
- A configuracao do asset de gas (`pipeline.gas.units_per_gas`) inclui um decimal `twap_local_per_xor`,
  um `liquidity_profile` (`tier1`, `tier2`, ou `tier3`), e um `volatility_class` (`stable`,
  `elevated`, `dislocated`). Esses flags alimentam o settlement router para que a cotacao XOR resultante
  coincida com o TWAP canonico e o tier de haircut da lane.
- As transacoes IVM devem incluir o metadado `gas_limit` (`u64`, > 0) para limitar a exposicao a taxas.
  O endpoint `/v2/contracts/call` exige `gas_limit` explicitamente e valores invalidos sao rejeitados.
- Quando uma transacao define o metadado `fee_sponsor`, o sponsor deve conceder
  `CanUseFeeSponsor { sponsor }` ao chamador. Tentativas de sponsorship nao autorizadas sao rejeitadas
  e registradas.
- Cada transacao que paga gas registra um `LaneSettlementReceipt`. Cada recibo armazena o
  identificador de origem fornecido pelo chamador, o micro-montante local, o XOR devido
  imediatamente, o XOR esperado apos o haircut, a margem de seguranca realizada
  (`xor_variance_micro`), e o timestamp do bloco em milissegundos.
- A execucao do bloco agrega recibos por lane/dataspace e os publica via `lane_settlement_commitments`
  em `/v2/sumeragi/status`. Os totais exibem `total_local_micro`, `total_xor_due_micro`, e
  `total_xor_after_haircut_micro` somados sobre o bloco para exportes noturnos de reconciliacao.
- Um novo contador `total_xor_variance_micro` acompanha quanto de margem de seguranca foi consumida
  (diferenca entre o XOR devido e a expectativa pos-haircut), e `swap_metadata` documenta os
  parametros deterministas de conversao (TWAP, epsilon, liquidity profile, e volatility_class) para
  que auditores possam verificar os insumos da cotacao independentemente da configuracao de runtime.

Consumidores podem observar `lane_settlement_commitments` junto com os snapshots existentes de
commitments de lane e dataspace para verificar se buffers de taxas, tiers de haircut e a execucao de
swap correspondem ao modelo de taxas Nexus configurado.
