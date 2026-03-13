---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: mecanismo de negociação
título: mecanismo de negociação SoraFS
sidebar_label: mecanismo de negociação
descrição: mecanismo de negociação SF-8, integração Torii e superfícies de telemetria.
---

:::nota مستند ماخذ
:::

# Mecanismo de negociação SoraFS

SF-8 roadmap track SoraFS deal engine متعارف کراتا ہے، جو
clientes اور provedores کے درمیان armazenamento اور acordos de recuperação کے لیے
contabilidade determinística فراہم کرتا ہے۔ Contratos e cargas úteis Norito کے ساتھ
Você pode usar o `crates/sorafs_manifest/src/deal.rs` para obter mais informações
termos de negócio, bloqueio de títulos, micropagamentos probabilísticos e registros de liquidação کو کور کرتے ہیں۔

Trabalhador SoraFS incorporado (`sorafs_node::NodeHandle`) no processo do nó کے لیے
Instância `DealEngine` بناتا ہے۔ یہ motor:

- `DealTermsV1` کے ذریعے ofertas کو validar اور registrar کرتا ہے؛
- relatório de uso de replicação ہونے پر cobranças denominadas XOR acumulam کرتا ہے؛
- amostragem determinística baseada em BLAKE3 کے ذریعے avaliação probabilística de janelas de micropagamento کرتا ہے؛ Mais
- snapshots do razão e publicação de governança کے لیے موزوں cargas úteis de liquidação بناتا ہے۔

Validação de testes unitários, seleção de micropagamentos e fluxos de liquidação کو cobertura کرتے ہیں تاکہ
operadores اعتماد کے ساتھ Exercício de APIs کر سکیں۔ Assentamentos com governança `DealSettlementV1`
cargas úteis emitem کرتے ہیں، جو Pipeline de publicação SF-12 میں براہ راست fio ہوتے ہیں، اور
OpenTelemetry کی série `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) e painéis Torii
Aplicação de SLO کے لیے atualização کرتی ہے۔ Itens de acompanhamento, automação de corte iniciada pelo auditor
semântica de cancelamento کو política de governança کے ساتھ coordenada کرنے پر مرکوز ہیں۔

Telemetria de uso اب `sorafs.node.micropayment_*` conjunto de métricas کو بھی feed کرتی ہے:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, e balcões de ingressos
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). یہ Fluxo de loteria probabilística total کو ظاہر کرتے ہیں تاکہ
ganhos de micropagamento de operadores اور transferência de crédito کو resultados de liquidação کے ساتھ correlacionar کر سکیں۔

## Integração Torii

Endpoints dedicados Torii expõem o relatório de uso de provedores de کرتا ہے تاکہ
A fiação sob medida e a unidade de ciclo de vida do negócio são as seguintes:- `POST /v2/sorafs/deal/usage` `DealUsageReport` telemetria aceita کرتا ہے اور
  resultados contábeis determinísticos (`UsageOutcome`) retorno کرتا ہے۔
- `POST /v2/sorafs/deal/settle` finalização da janela atual کرتا ہے، اور
  نتیجے میں بننے والا `DealSettlementRecord` `DealSettlementV1` codificado em base64 کے ساتھ stream کرتا ہے
  جو governança publicação DAG کے لیے تیار ہوتا ہے۔
- Torii کا `/v2/events/sse` feed اب `SorafsGatewayEvent::DealUsage` registra transmissão کرتا ہے
  جو ہر envio de uso کا خلاصہ دیتے ہیں (época, GiB-horas medidas, balcões de ingressos,
  encargos determinísticos), registros `SorafsGatewayEvent::DealSettlement` e instantâneo do razão de liquidação canônica کے ساتھ
  artefato de governança em disco کا BLAKE3 digest/size/base64 شامل کرتے ہیں، اور
  Alertas `SorafsGatewayEvent::ProofHealth` جب Os limites de PDP/PoTR excedem ہوں (provedor, janela, estado de aviso/recarga, valor da penalidade)۔
  Provedor de consumidores کے لحاظ سے filtro کر کے نئی telemetria, assentamentos یا alertas de prova de saúde پر
  votação کے بغیر reagir کر سکتے ہیں۔

دونوں endpoints SoraFS estrutura de cota میں نئے janela `torii.sorafs.quota.deal_telemetry` کے ذریعے
شامل ہیں، جس سے operadores ہر implantação کے لیے ajuste de taxa de envio permitido کر سکتے ہیں۔