---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: motor de acuerdos
título: Motor de acuerdos SoraFS
sidebar_label: Motor de acuerdos
descripción: Vista del conjunto del motor de acuerdo SF-8, de la integración Torii y de las superficies de televisión.
---

:::nota Fuente canónica
:::

# Motor de acuerdos SoraFS

La pista de ruta SF-8 introduce el motor de acuerdo SoraFS, Fournissant
una compatibilidad determinada para los acuerdos de almacenamiento y recuperación entre
clientes y proveedores. Los acuerdos son décrits a través de las cargas útiles Norito
definido en `crates/sorafs_manifest/src/deal.rs`, cumpliendo los términos del acuerdo,
le verrouillage de bonds, les micropaiements probabilistes et les enregistrements de règlement.

El trabajador SoraFS embarqué (`sorafs_node::NodeHandle`) instancia desormada en
`DealEngine` para cada proceso de nuevo. El motor:

- valide y registre los acuerdos a través de `DealTermsV1`;
- acumulación de cargos libellées en XOR cuando el uso de réplica esté informado;
- Evalue les fenêtres de micropaiement probabiliste via un échantillonnage deterministe
  basado en BLAKE3; y
- Producto de instantáneas de libro mayor y cargas útiles adaptadas a la publicación.
  de gobernanza.Las pruebas unitarias incluyen la validación, la selección de micropagos y el flujo de
Reglamenta que los operadores puedan ejercer las API en confianza. Los reglamentos
émettent desormais des payloads de gouvernance `DealSettlementV1`, s'intégrant directement
en proceso de publicación SF-12, y agregado al día de la serie OpenTelemetry `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) para los paneles Torii y
La aplicación de SLO. Los trabajos siguientes pueden automatizar la tala iniciada por
les auditeurs et la coordinacion des sémantiques d'annulation avec la politique de gouvernance.

La télémétrie d'usage alimente aussi el conjunto de métriques `sorafs.node.micropayment_*` :
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, y los ordenadores de tickets
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). Ces totaux exponennt le flux de lotería
probabilidad de que los operadores puedan corregir las ganancias de micropagos y
le transfer-over de crédit avec les résultats de règlement.

## Integración Torii

Torii expone los puntos finales deseados para que los proveedores señalen el uso y controlen
El ciclo de vida de los acuerdos sin cableado específico:- `POST /v1/sorafs/deal/usage` acepta la televisión `DealUsageReport` y envía
  des résultats de comptabilité déterministes (`UsageOutcome`).
- `POST /v1/sorafs/deal/settle` finalize la fenêtre courante, en streamant le
  `DealSettlementRecord` resultante con un `DealSettlementV1` codificado en base64
  prêt pour publication dans le DAG de gouvernance.
- Le feed `/v1/events/sse` de Torii difuso desormais des enregistrements
  `SorafsGatewayEvent::DealUsage` résumant chaque soumission d'usage (época, GiB-heures mesurés,
  compteurs de tickets, charge déterministes), des enregistrements
  `SorafsGatewayEvent::DealSettlement` que incluye la instantánea canónica del libro mayor de reglas
  Además de le digest/taille/base64 BLAKE3 de l'artefact de gouvernance sur disque, et des alertas
  `SorafsGatewayEvent::ProofHealth` dès que les seuils PDP/PoTR sont dépassés (fournisseur, fenêtre,
  estado de strike/cooldown, montante de pena). Los consumidores pueden filtrar por el proveedor.
  Para reaccionar a las nuevas télémétries, des règlements ou des alertas de salud de las pruebas sin sondeo.

Los dos puntos finales participan en el marco de cuotas SoraFS a través de la nueva ventana
`torii.sorafs.quota.deal_telemetry`, que permite a los operadores ajustar el taux de soumission
autorizado por el despliegue.