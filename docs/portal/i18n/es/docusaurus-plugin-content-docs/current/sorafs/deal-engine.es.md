---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: motor de acuerdos
título: Motor de acuerdos de SoraFS
sidebar_label: Motor de acuerdos
descripción: Resumen del motor de acuerdos SF-8, integración con Torii y superficies de telemetría.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/deal_engine.md`. Mantenga ambas ubicaciones alineadas mientras la documentación heredada siga activa.
:::

# Motor de acuerdos de SoraFS

La línea de ruta SF-8 introduce el motor de acuerdos de SoraFS, que aporta
contabilidad determinista para acuerdos de almacenamiento y recuperación entre
clientes y proveedores. Los acuerdos se describen con los payloads Norito
definidos en `crates/sorafs_manifest/src/deal.rs`, que cubren términos del acuerdo,
bloqueo de bonos, micropagos probabilísticos y registros de liquidación.

El trabajador embebido de SoraFS (`sorafs_node::NodeHandle`) ahora instancia un
`DealEngine` para cada proceso de nodo. El motor:

- valida y registra acuerdos usando `DealTermsV1`;
- acumula cargos denominados en XOR cuando se reporta el uso de replicación;
- ventanas evalúan de micropago probabilístico usando muestreo determinista
  basado en BLAKE3; y
- producir instantáneas de libro mayor y cargas útiles de liquidación aptos para publicación
  de gobernanza.Las pruebas unitarias cubren validación, selección de micropagos y flujos de
liquidación para que los operadores puedan ejercitar las APIs con confianza.
Las liquidaciones ahora emiten payloads de gobernanza `DealSettlementV1`,
conectándose directamente al tubo de publicación SF-12, y actualizando la serie
OpenTelemetría `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) para tableros de Torii y
aplicación de SLO. Los siguientes pasos se enfocan en la automatización de slashing
iniciada por auditores y en coordinar la semántica de cancelación con la política de gobernanza.

La telemetría de uso también alimenta el conjunto de métricas `sorafs.node.micropayment_*`:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, y los contadores de tickets
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). Estos totales exponen el flujo de lotería
probabilística para que los operadores puedan correlacionar las victorias de micropagos y
el arrastre de crédito con los resultados de liquidación.

## Integración con Torii

Torii exponen endpoints dedicados para que los proveedores reporten uso y conduzcan el ciclo
de vida del acuerdo sin cableado personalizado:- `POST /v1/sorafs/deal/usage` acepta telemetría `DealUsageReport` y retorna
  resultados deterministas de contabilidad (`UsageOutcome`).
- `POST /v1/sorafs/deal/settle` finaliza la ventana actual, transmitiendo el
  `DealSettlementRecord` resultante junto con un `DealSettlementV1` en base64
  listo para publicación en el DAG de gobernanza.
- El feed `/v1/events/sse` de Torii ahora transmite registros `SorafsGatewayEvent::DealUsage`
  que resumen cada envío de uso (época, GiB-hora medidos, contadores de tickets,
  cargos deterministas), registros `SorafsGatewayEvent::DealSettlement`
  que incluyen el snapshot canónico del ledger de liquidación más el digest/tamaño/base64
  BLAKE3 del artefacto de gobernanza en disco, y alertas `SorafsGatewayEvent::ProofHealth`
  cada vez que se superan umbrales PDP/PoTR (proveedor, ventana, estado de strike/cooldown,
  monto de penalización). Los consumidores pueden filtrar por proveedor para reaccionar a
  nueva telemetría, liquidaciones o alertas de salud de pruebas sin hacer polling.

Ambos endpoints participan en el marco de cuotas de SoraFS a través de la nueva ventana
`torii.sorafs.quota.deal_telemetry`, permitiendo a los operadores ajustar la tasa de envío
permitida por despliegue.