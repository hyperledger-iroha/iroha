---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: mecanismo de negociação
título: Motor de acuerdos de SoraFS
sidebar_label: Motor de referência
descrição: Resumo do motor de corrente SF-8, integração com Torii e superfícies de telemetria.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/deal_engine.md`. Mantenha ambas as localizações alinhadas enquanto a documentação herdada segue ativa.
:::

# Motor de referência de SoraFS

A linha de rota SF-8 apresenta o motor de acuerdos de SoraFS, que aporta
contabilidade determinista para acordos de armazenamento e recuperação entre
clientes e fornecedores. Os detalhes são descritos com as cargas úteis Norito
definido em `crates/sorafs_manifest/src/deal.rs`, que contém os termos do acordo,
bloqueio de bônus, micropagamentos probabilísticos e registros de liquidação.

O trabalhador incorporado de SoraFS (`sorafs_node::NodeHandle`) agora é uma instância
`DealEngine` para cada processo de nó. O motor:

- valide e registre os dados usando `DealTermsV1`;
- acumula cargas denominadas em XOR quando se reporta o uso de replicação;
- avaliar janelas de micropagamento probabilístico usando o mapa determinista
  baseado em BLAKE3; sim
- produzir instantâneos de razão e cargas úteis de liquidação aptos para publicação
  de governança.

As testes unitários incluem validação, seleção de micropagamentos e fluxos de
liquidação para que os operadores possam executar APIs com confiança.
As liquidações agora emitem cargas úteis de governo `DealSettlementV1`,
conectando-se diretamente ao pipeline de publicação SF-12, e atualizando a série
OpenTelemetria `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) para painéis de Torii e
aplicação de SLOs. Os próximos passos são focados na automação de corte
iniciado por auditores e coordenando a semântica de cancelamento com a política de governo.

A telemetria de uso também alimenta o conjunto de métricas `sorafs.node.micropayment_*`:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, e os contadores de tickets
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). Esses totais expõem o fluxo de loteria
probabilística para que os operadores possam correlacionar as vitórias de micropagos e
a transferência de crédito com os resultados da liquidação.

## Integração com Torii

Torii expõe endpoints dedicados para que os provedores reportem o uso e conduzam o ciclo
de vida do acordo sem fiação personalizada:- `POST /v2/sorafs/deal/usage` aceita telemetria `DealUsageReport` e retorna
  resultados deterministas de contabilidade (`UsageOutcome`).
- `POST /v2/sorafs/deal/settle` finaliza a janela atual, transmitindo o
  `DealSettlementRecord` resultante junto com um `DealSettlementV1` em base64
  lista para publicação no DAG de governança.
- O feed `/v2/events/sse` de Torii agora registra transmissões `SorafsGatewayEvent::DealUsage`
  que resume cada envio de uso (época, medições GiB-hora, contadores de tickets,
  cargas deterministas), registros `SorafsGatewayEvent::DealSettlement`
  que inclui o snapshot canônico do razão de liquidação além do digest/tamaño/base64
  BLAKE3 do artefato de governança em disco, e alertas `SorafsGatewayEvent::ProofHealth`
  cada vez que se excedem os limites do PDP/PoTR (provedor, ventana, estado de strike/cooldown,
  monte de penalização). Os consumidores podem filtrar pelo fornecedor para reagir a
  novas telemetria, liquidações ou alertas de saúde de testes sem fazer pesquisas.

Ambos os endpoints participam da estrutura de cotas de SoraFS através da nova janela
`torii.sorafs.quota.deal_telemetry`, permitindo que as operadoras ajustem a bandeja de envio
permitido por despliegue.