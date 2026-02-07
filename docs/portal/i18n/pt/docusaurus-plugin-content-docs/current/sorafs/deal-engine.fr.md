---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: mecanismo de negociação
título: Motor de acordos SoraFS
sidebar_label: Motor de acordos
descrição: Vista do conjunto do motor de acordo SF-8, da integração Torii e das superfícies de telefonia.
---

:::nota Fonte canônica
:::

# Motor de acordo SoraFS

A pista do roteiro SF-8 apresenta o motor de acordo SoraFS, fornecido
uma comptabilidade determinada para os acordos de armazenamento e recuperação entre
clientes e fornecedores. Os acordos são registrados por meio das cargas úteis Norito
definido em `crates/sorafs_manifest/src/deal.rs`, cobrindo os termos do acordo,
o verrouillage de títulos, os micropagamentos probabilísticos e os registros de regulamento.

Le trabalhador SoraFS embarque (`sorafs_node::NodeHandle`) instância desormida mais um
`DealEngine` para cada processo de noeud. O motor:

- valide e registre os acordos via `DealTermsV1`;
- cumule des chargees libellées en XOR quando o uso da replicação for relatado;
- avaliar as janelas de micropagamento probabilístico por meio de uma mudança determinada
  baseado em BLAKE3 ; et
- produzir instantâneos de razão e cargas úteis de regulamento adaptados à publicação
  de governança.

Os testes unitários cobrem a validação, a seleção de micropaimentos e o fluxo de
É regra que os operadores possam exercer as APIs com confiança. Os regulamentos
é desormado de cargas úteis de governo `DealSettlementV1`, diretamente integrado
no pipeline de publicação SF-12, e recentemente lançou a série OpenTelemetry `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) para os painéis Torii e
o aplicativo SLO. Os trabalhos seguintes permitem a automação do corte iniciado por
os auditores e a coordenação da semântica da anulação com a política de governo.

A transmissão de uso também do conjunto de métricas `sorafs.node.micropayment_*` :
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, e os compradores de ingressos
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). Esses todos expõem o fluxo da loteria
é provável que os operadores possam corrigir os ganhos de micropagamentos e
a transferência de crédito com os resultados do regulamento.

## Integração Torii

Torii expõe os endpoints dedicados para que os fornecedores sinalizem o uso e o piloto
o ciclo de vida dos acordos sem fiação específica:- `POST /v1/sorafs/deal/usage` aceita a televisão `DealUsageReport` e o envio
  resultados de comptabilidade determinada (`UsageOutcome`).
- `POST /v1/sorafs/deal/settle` finaliza a janela atual, transmitindo
  `DealSettlementRecord` resultante com um `DealSettlementV1` codificado em base64
  pronto para publicação no DAG de governo.
- Le feed `/v1/events/sse` de Torii difuso désormais des registros
  `SorafsGatewayEvent::DealUsage` resumo de cada uso de uso (época, GiB-heures mesurés,
  compteurs de tickets, charge déterministes), des enregistrements
  `SorafsGatewayEvent::DealSettlement` que inclui o instantâneo canônico do livro-razão
  assim como o digest/taille/base64 BLAKE3 do artefato de governança em disco e os alertas
  `SorafsGatewayEvent::ProofHealth` faz com que seus PDP/PoTR sejam ultrapassados (fornecedor, janela,
  estado de strike/cooldown, montant de penalité). Os consumidores podem filtrar pelo fornecedor
  para reagir a novas telemetrias, regulamentos ou alertas de saúde, provas sem votação.

Os dois endpoints participam da estrutura de cotas SoraFS por meio da nova janela
`torii.sorafs.quota.deal_telemetry`, permite que os operadores ajustem o taux de soumission
autorizado por implantação.