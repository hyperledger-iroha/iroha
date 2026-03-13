---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: motor de acuerdos
título: Motor de acordes da SoraFS
sidebar_label: Motor de acordes
descripción: Visao general do motor de acordos SF-8, integracao com Torii y superficies de telemetría.
---

:::nota Fuente canónica
Esta página espelha `docs/source/sorafs/deal_engine.md`. Mantenga ambos lugares alineados mientras la documentación alternativa permanezca activa.
:::

# Motor de acuerdo con SoraFS

La pista de la hoja de ruta SF-8 introduce el motor de acuerdo con SoraFS, necesario
contabilidade deterministica para acuerdos de armazenamento e recuperacao entre
clientes y proveedores. Los acordes sao descritos con las cargas útiles Norito
definidos en `crates/sorafs_manifest/src/deal.rs`, cobrindo termos do acordo,
bloqueo de bonos, micropagamentos probabilísticos y registros de liquidacao.

O trabajador embutido da SoraFS (`sorafs_node::NodeHandle`) ahora instancia um
`DealEngine` para cada proceso de nodo. O motor:

- valida e registra acordes usando `DealTermsV1`;
- acumula cobrancas denominadas em XOR quando o uso de replicacao e reportado;
- avalia janelas de micropagamento probabilistico usando amostragem deterministica
  basada en BLAKE3; mi
- Produz snapshots de ledger e payloads de liquidacao adecuados para publicacao
  de gobernancia.Testes unitarios cobrem validacao, selección de micropagamentos e flujos de liquidacao para
que los operadores pueden ejercer como API con confianza. Liquidacoes agora emitem
payloads degobernanza `DealSettlementV1`, conectando directamente ao pipeline de
publicacao SF-12, y actualiza la serie OpenTelemetry `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) para paneles de control Torii e
aplicaciones de SLO. Los siguientes elementos se enfocan en la automatización de corte iniciada por
auditores e na coordenacao de semánticas de cancelación con la política de gobierno.

La telemetría de uso también alimenta el conjunto de métricas `sorafs.node.micropayment_*`:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, y los contadores de tickets
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). Esses totais expoem o fluxo de loteria
probabilistica para que los operadores puedan correlacionar ganhos de micropagamento e
prórroga de crédito con resultados de liquidación.

## Integracao com Torii

Torii expoe endpoints dedicados para que proveedores informen el uso y conduzam o
ciclo de vida do acordo sin cableado sob medida:- `POST /v2/sorafs/deal/usage` aceita telemetria `DealUsageReport` y retorna
  resultados deterministas de contabilidade (`UsageOutcome`).
- `POST /v2/sorafs/deal/settle` finaliza a janela atual, transmitiendo o
  `DealSettlementRecord` resultante junto con un `DealSettlementV1` en base64
  pronto para publicacao no DAG de gobernanza.
- O feed `/v2/events/sse` do Torii ahora transmite registros `SorafsGatewayEvent::DealUsage`
  resumindo cada envio de uso (época, GiB-horas medidos, contadores de tickets,
  cobrancas deterministas), registros `SorafsGatewayEvent::DealSettlement`
  que incluye la instantánea canónica del libro mayor de liquidación más el resumen/tamanho/base64
  BLAKE3 hace artefato de gobierno en discoteca, y alertas `SorafsGatewayEvent::ProofHealth`
  sempre que limiares PDP/PoTR sao excedidos (provedor, janela, estado de strike/cooldown,
  valor de la penalidad). Consumidores podem filtrar por provedor para reagir a nova
  telemetria, liquidacoes ou alertas de saude de pruebas sem polling.

Ambos puntos finales participan en el framework de cotas de SoraFS a través de una nueva janela
`torii.sorafs.quota.deal_telemetry`, permitiendo que los operadores ajusten los taxones de envío
permitida por despliegue.