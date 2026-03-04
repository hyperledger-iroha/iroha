---
lang: es
direction: ltr
source: docs/source/sorafs/deal_engine.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e38b6c30d653b3c56a70a0575c7e527a29cb54111b528bd24f1c6a7f06f4fe86
source_last_modified: "2025-11-15T07:12:32.751859+00:00"
translation_last_reviewed: "2026-01-30"
---

# Motor de acuerdos SoraFS

El track SF-8 del roadmap introduce el motor de acuerdos SoraFS, proporcionando
contabilidad determinista para acuerdos de almacenamiento y retrieval entre
clientes y providers. Los acuerdos se describen con payloads Norito definidos
en `crates/sorafs_manifest/src/deal.rs`, cubriendo terminos del deal, lock de
bonds, micropagos probabilisticos y registros de settlement.

El worker embebido SoraFS (`sorafs_node::NodeHandle`) ahora instancia una
instancia `DealEngine` para cada proceso de nodo. El engine:

- valida y registra acuerdos usando `DealTermsV1`;
- acumula cargos denominados en XOR cuando se reporta uso de replicacion;
- evalua ventanas de micropagos probabilisticos usando muestreo determinista
  basado en Blake3; y
- produce snapshots de ledger y payloads de settlement adecuados para
  publicacion en governance.

Los tests unitarios cubren validacion, seleccion de micropagos y flujos de
settlement para que los operadores ejerciten las APIs con confianza. Los
settlements ahora emiten payloads de governance `DealSettlementV1`, conectando
directamente con el pipeline de publicacion SF-12, y actualizan la serie
OpenTelemetry `sorafs.node.deal_*` (`deal_settlements_total`,
`deal_expected_charge_nano`, `deal_client_debit_nano`, `deal_outstanding_nano`,
`deal_bond_slash_nano`, `deal_publish_total`) para dashboards Torii y
aplicacion de SLOs. Los items siguientes se enfocan en automatizacion de
slashing iniciada por auditores y en coordinar semanticas de cancelacion con la
politica de governance.

La telemetria de uso ahora tambien alimenta el set de metricas
`sorafs.node.micropayment_*`: `micropayment_charge_nano`,
`micropayment_credit_generated_nano`, `micropayment_credit_applied_nano`,
`micropayment_credit_carry_nano`, `micropayment_outstanding_nano`, y los
contadores de tickets (`micropayment_tickets_processed_total`,
`micropayment_tickets_won_total`, `micropayment_tickets_duplicate_total`).
Estos totales exponen el flujo de loteria probabilistica para que operadores
correlacionen ganadas de micropagos y carry-over de creditos con resultados de
settlement.

## Integracion con Torii

Torii expone endpoints dedicados para que los providers reporten uso y dirijan
el ciclo de vida del acuerdo sin wiring ad hoc:

- `POST /v1/sorafs/deal/usage` acepta telemetria `DealUsageReport` y retorna
  resultados deterministas de contabilidad (`UsageOutcome`).
- `POST /v1/sorafs/deal/settle` finaliza la ventana actual, streameando el
  `DealSettlementRecord` resultante junto a un `DealSettlementV1` codificado en
  base64 listo para publicarse en Governance DAG.
- El feed `/v1/events/sse` de Torii ahora difunde registros
  `SorafsGatewayEvent::DealUsage` que resumen cada submit de uso (epoch,
  GiB-hours medidos, contadores de tickets, cargos deterministas), registros
  `SorafsGatewayEvent::DealSettlement` que incluyen el snapshot canonico del
  ledger de settlement mas el digest/size/base64 BLAKE3 del artefacto de
  governance en disco, y alertas `SorafsGatewayEvent::ProofHealth` cuando se
  exceden thresholds PDP/PoTR (provider, ventana, estado strike/cooldown,
  monto de penalidad). Los consumidores pueden filtrar por provider para
  reaccionar a nueva telemetria, settlements o alertas de proof-health sin
  polling.

Ambos endpoints participan en el framework de cuotas SoraFS via la nueva
ventana `torii.sorafs.quota.deal_telemetry`, permitiendo a los operadores
ajustar la tasa de submissions permitida por despliegue.
