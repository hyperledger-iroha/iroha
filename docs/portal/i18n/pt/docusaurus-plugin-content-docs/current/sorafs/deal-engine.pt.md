---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: mecanismo de negociação
título: Motor de acordos da SoraFS
sidebar_label: Motor de acordos
descrição: Visão geral do motor de acordos SF-8, integração com Torii e superfícies de telemetria.
---

:::nota Fonte canônica
Esta página espelha `docs/source/sorafs/deal_engine.md`. Mantenha ambos os locais alinhados enquanto a documentação alternativa permanece ativa.
:::

# Motor de acordos da SoraFS

O track do roadmap SF-8 apresenta o motor de acordos da SoraFS, fornecendo
contabilidade determinística para acordos de armazenamento e recuperação entre
clientes e fornecedores. Os acordos são descritos com as cargas úteis Norito
definidos em `crates/sorafs_manifest/src/deal.rs`, cobrindo os termos do acordo,
bloqueio de títulos, micropagamentos probabilísticos e registros de liquidação.

O trabalhador embutido da SoraFS (`sorafs_node::NodeHandle`) agora uma instância
`DealEngine` para cada processo de nó. Ó motor:

- validar e registrar acordos usando `DealTermsV1`;
- acumula cobrancas designadas em XOR quando o uso de replicação e reportado;
- avaliar janelas de micropagamento probabilístico usando amostragem determinística
  baseado em BLAKE3; e
- produz snapshots de ledger e payloads de liquidação adequados para publicação
  de governança.

Testes unitários de cobrem validação, seleção de micropagamentos e fluxos de liquidação para
que os operadores podem exercitar as APIs com confiança. Liquidações agora emitem
payloads de governança `DealSettlementV1`, conectando diretamente ao pipeline de
publicação SF-12, e atualizam a série OpenTelemetry `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) para painéis do Torii e
aplicação de SLOs. Os itens seguintes focam na automatização de slashing iniciada por
auditores e na coordenação de semântica de cancelamento com a política de governança.

A telemetria de uso agora também alimenta o conjunto de métricas `sorafs.node.micropayment_*`:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, e os contadores de tickets
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). Esses totais expõem o fluxo de loteria
probabilística para que os operadores possam correlacionar ganhos de micropagamento e
transferência de crédito com resultados de liquidação.

## Integração com Torii

Torii expoe endpoints dedicados para que provedores reportem uso e conduzam o
ciclo de vida do acordo sem fiação sob medida:- `POST /v2/sorafs/deal/usage` aceita telemetria `DealUsageReport` e retorna
  resultados deterministas de contabilidade (`UsageOutcome`).
- `POST /v2/sorafs/deal/settle` finaliza a janela atual, transmitindo o
  `DealSettlementRecord` resultante junto com um `DealSettlementV1` em base64
  pronto para publicação no DAG de governança.
- O feed `/v2/events/sse` do Torii agora transmite registros `SorafsGatewayEvent::DealUsage`
  resumindo cada envio de uso (época, GiB-horas medidas, contadores de tickets,
  cobrancas deterministas), registros `SorafsGatewayEvent::DealSettlement`
  que inclui o snapshot canônico do ledger de liquidação mais o digest/tamanho/base64
  BLAKE3 dos artistas de governança em disco, e alertas `SorafsGatewayEvent::ProofHealth`
  sempre que os limites PDP/PoTR são excedidos (provedor, janela, estado de strike/cooldown,
  valor da negociação). Consumidores podem filtrar por provedor para reagir a nova
  telemetria, liquidações ou alertas de saúde de provas sem polling.

Ambos os endpoints participam do framework de cotas da SoraFS através de uma nova janela
`torii.sorafs.quota.deal_telemetry`, permitindo que os operadores ajustem a taxa de envio
permitido por implantação.