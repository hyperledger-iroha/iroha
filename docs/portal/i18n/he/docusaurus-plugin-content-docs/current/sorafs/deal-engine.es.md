---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/deal-engine.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: מנוע עסקה
כותרת: Motor de acuerdos de SoraFS
sidebar_label: Motor de acuerdos
תיאור: קורות חיים של מנוע אקורדוס SF-8, אינטגרציה עם Torii y superficies de telemetria.
---

:::הערה Fuente canónica
Esta página refleja `docs/source/sorafs/deal_engine.md`. Mantén ambas ubicaciones alineadas mientras la documentación heredada siga active.
:::

# Motor de acuerdos de SoraFS

La línea de ruta SF-8 להציג את מנוע ה-acuerdos de SoraFS, que aporta
contabilidad determinista para acuerdos de almacenamiento y recuperación entre
לקוחות y proveedores. Los acuerdos תואר עם מטענים Norito
definidos en `crates/sorafs_manifest/src/deal.rs`, que cubren términos del acuerdo,
בלוקו דה בונו, מיקרו-פגוס הסתברות ורישום ליקוי.

El worker embebido de SoraFS (`sorafs_node::NodeHandle`) ahora instancia un
`DealEngine` עבור תהליך תהליך נודו. מנוע אל:

- valida y registra acuerdos usando `DealTermsV1`;
- acumula cargos denominados en XOR cuando se reporta el uso de replicación;
- evalúa ventanas de micropago probabilístico usando muestreo determinista
  basado en BLAKE3; y
- הפקת תצלומי מצב של ספר חשבונות ומטענים מטעמים עבור פרסום
  דה גוברננסה.

Las pruebas unitarias cubren validación, selección de micropagos y flujos de
liquidación para que los operadores puedan ejercitar las APIs confianza.
Las liquidaciones ahora emiten payloads de gobernanza `DealSettlementV1`,
קונקטנדוזה ישיר אל צינור הפרסום SF-12, וממשיכים לסדרה
OpenTelemetry `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) עבור לוחות מחוונים de Torii y
אפליקציית SLOs. Los suientes pasos se enfocan en la automatización de slashing
iniciada por auditores y en coordinar la semántica de cancelación con la política de gobernanza.

La telemetria de uso también alimenta el conjunto de métricas `sorafs.node.micropayment_*`:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, y los contadores de tickets
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). Estos totals exponen el flujo de lotería
probabilística para que los operadores puedan correlacionar las victorias de micropagos y
העברת כספים על בסיס אשראי.

## אינטגרציה עם Torii

Torii הצגת נקודות קצה dedicados para que los proveedores reporten uso y conduzcan el ciclo
התאמה אישית של חיווט de vida del acuerdo:- `POST /v1/sorafs/deal/usage` acepta telemetría `DealUsageReport` y retorna
  resultados deterministas de contabilidad (`UsageOutcome`).
- `POST /v1/sorafs/deal/settle` finaliza la ventana actual, transmitiendo el
  `DealSettlementRecord` resultante junto con un `DealSettlementV1` en base64
  listo para publicación en el DAG de gobernanza.
- El feed `/v1/events/sse` de Torii ahora transmite registros `SorafsGatewayEvent::DealUsage`
  que resumen cada envío de uso (תקופה, GiB-hora medidos, contadores de tickets,
  cargos deterministas), registros `SorafsGatewayEvent::DealSettlement`
  que incluyen el snapshot canónico del book de liquidación more el digest/tamaño/base64
  BLAKE3 del artefacto de gobernanza en disco, y alertas `SorafsGatewayEvent::ProofHealth`
  cada vez que se exceden umbrales PDP/PoTR (proveedor, ventana, estado de strike/cooldown,
  monto de penalisación). Los consumidores pueden filtrar por proveedor para reaccionar a
  nueva telemetria, liquidaciones o alertas de salud de pruebas sin hacer סקרים.

משתתף נקודות קצה של Ambos en el framework de cuotas de SoraFS a través de la nueva ventana
`torii.sorafs.quota.deal_telemetry`, permitiendo a los operadores ajustar la tasa de envío
permitida por despliegue.