---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/deal-engine.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: deal-engine
title: Motor de acordos da SoraFS
sidebar_label: Motor de acordos
description: Visao geral do motor de acordos SF-8, integracao com Torii e superficies de telemetria.
---

:::note Fonte canonica
Esta pagina espelha `docs/source/sorafs/deal_engine.md`. Mantenha ambos os locais alinhados enquanto a documentacao alternativa permanecer ativa.
:::

# Motor de acordos da SoraFS

O track do roadmap SF-8 introduz o motor de acordos da SoraFS, fornecendo
contabilidade deterministica para acordos de armazenamento e recuperacao entre
clientes e provedores. Os acordos sao descritos com os payloads Norito
definidos em `crates/sorafs_manifest/src/deal.rs`, cobrindo termos do acordo,
bloqueio de bonds, micropagamentos probabilisticos e registros de liquidacao.

O worker embutido da SoraFS (`sorafs_node::NodeHandle`) agora instancia um
`DealEngine` para cada processo de nodo. O motor:

- valida e registra acordos usando `DealTermsV1`;
- acumula cobrancas denominadas em XOR quando o uso de replicacao e reportado;
- avalia janelas de micropagamento probabilistico usando amostragem deterministica
  baseada em BLAKE3; e
- produz snapshots de ledger e payloads de liquidacao adequados para publicacao
  de governanca.

Testes unitarios cobrem validacao, selecao de micropagamentos e fluxos de liquidacao para
que operadores possam exercitar as APIs com confianca. Liquidacoes agora emitem
payloads de governanca `DealSettlementV1`, conectando diretamente ao pipeline de
publicacao SF-12, e atualizam a serie OpenTelemetry `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) para dashboards do Torii e
aplicacao de SLOs. Os itens seguintes focam na automatizacao de slashing iniciada por
auditores e na coordenacao de semanticas de cancelamento com a politica de governanca.

A telemetria de uso agora tambem alimenta o conjunto de metricas `sorafs.node.micropayment_*`:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, e os contadores de tickets
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). Esses totais expoem o fluxo de loteria
probabilistica para que operadores possam correlacionar ganhos de micropagamento e
carry-over de credito com resultados de liquidacao.

## Integracao com Torii

Torii expoe endpoints dedicados para que provedores reportem uso e conduzam o
ciclo de vida do acordo sem wiring sob medida:

- `POST /v1/sorafs/deal/usage` aceita telemetria `DealUsageReport` e retorna
  resultados deterministas de contabilidade (`UsageOutcome`).
- `POST /v1/sorafs/deal/settle` finaliza a janela atual, transmitindo o
  `DealSettlementRecord` resultante junto com um `DealSettlementV1` em base64
  pronto para publicacao no DAG de governanca.
- O feed `/v1/events/sse` do Torii agora transmite registros `SorafsGatewayEvent::DealUsage`
  resumindo cada envio de uso (epoch, GiB-hours medidos, contadores de tickets,
  cobrancas deterministas), registros `SorafsGatewayEvent::DealSettlement`
  que incluem o snapshot canonico do ledger de liquidacao mais o digest/tamanho/base64
  BLAKE3 do artefato de governanca em disco, e alertas `SorafsGatewayEvent::ProofHealth`
  sempre que limiares PDP/PoTR sao excedidos (provedor, janela, estado de strike/cooldown,
  valor da penalidade). Consumidores podem filtrar por provedor para reagir a nova
  telemetria, liquidacoes ou alertas de saude de proofs sem polling.

Ambos os endpoints participam do framework de cotas da SoraFS via a nova janela
`torii.sorafs.quota.deal_telemetry`, permitindo que operadores ajustem a taxa de envio
permitida por deploy.
