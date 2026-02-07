---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: motor de acuerdos
título: motor de ofertas SoraFS
sidebar_label: motor de ofertas
descripción: motor de oferta SF-8, integración Torii y superficies de telemetría کا جائزہ۔
---

:::nota مستند ماخذ
:::

# SoraFS motor de ofertas

Seguimiento de la hoja de ruta del SF-8 SoraFS Motor de acuerdos متعارف کراتا ہے، جو
clientes اور proveedores کے درمیان almacenamiento اور acuerdos de recuperación کے لیے
contabilidad determinista فراہم کرتا ہے۔ Acuerdos کو Norito cargas útiles کے ساتھ
بیان کیا جاتا ہے جو `crates/sorafs_manifest/src/deal.rs` میں تعریف شدہ ہیں، اور
términos del acuerdo, bloqueo de bonos, micropagos probabilísticos y registros de liquidación

Trabajador SoraFS integrado (`sorafs_node::NodeHandle`) en un proceso de nodo کے لیے
Instancia `DealEngine` بناتا ہے۔ یہ motor:

- `DealTermsV1` کے ذریعے ofertas کو validar اور registrarse کرتا ہے؛
- informe de uso de replicación ہونے پر Los cargos denominados en XOR se acumulan کرتا ہے؛
- muestreo determinista basado en BLAKE3 کے ذریعے ventanas probabilísticas de micropagos evalúan کرتا ہے؛ اور
- instantáneas del libro mayor اور publicación de gobernanza کے لیے موزوں cargas útiles de liquidación بناتا ہے۔Validación de pruebas unitarias, selección de micropagos y flujos de liquidación y cobertura de کرتے ہیں تاکہ
operadores اعتماد کے ساتھ Ejercicio de API کر سکیں۔ Asentamientos اب `DealSettlementV1` gobernanza
las cargas útiles emiten کرتے ہیں، جو SF-12 pipeline de publicación میں براہ راست wire ہوتے ہیں، اور
OpenTelemetry serie `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) y paneles de control Torii
Aplicación de SLO کے لیے actualización کرتی ہے۔ Automatización de corte iniciada por el auditor de elementos de seguimiento اور
semántica de cancelación کو política de gobernanza کے ساتھ coordinar کرنے پر مرکوز ہیں۔

Telemetría de uso اب `sorafs.node.micropayment_*` conjunto de métricas کو بھی feed کرتی ہے:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, mostradores de boletos
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). یہ totales de flujo de lotería probabilística کو ظاہر کرتے ہیں تاکہ
operadores de micropagos ganan اور prórroga de crédito کو resultados de liquidación کے ساتھ correlacionan کر سکیں۔

## Integración Torii

Los puntos finales dedicados Torii exponen el informe de uso de proveedores de کرتا ہے تاکہ کر سکیں اور
بدون cableado a medida کے unidad de ciclo de vida del acuerdo کر سکیں:- La telemetría `POST /v1/sorafs/deal/usage` `DealUsageReport` acepta کرتا ہے اور
  resultados contables deterministas (`UsageOutcome`) retorno کرتا ہے۔
- `POST /v1/sorafs/deal/settle` ventana actual finalizar کرتا ہے، اور
  نتیجے میں بننے والا `DealSettlementRecord` codificado en base64 `DealSettlementV1` کے ساتھ stream کرتا ہے
  جو publicación del DAG de gobernanza کے لیے تیار ہوتا ہے۔
- Torii کا `/v1/events/sse` feed اب `SorafsGatewayEvent::DealUsage` registros transmitidos کرتا ہے
  جو ہر envío de uso کا خلاصہ دیتے ہیں (época, horas GiB medidas, mostradores de boletos,
  cargos deterministas), registros `SorafsGatewayEvent::DealSettlement`, instantánea del libro mayor de liquidación canónica کے ساتھ
  artefacto de gobernanza en disco کا BLAKE3 digest/size/base64 شامل کرتے ہیں، اور
  Alertas `SorafsGatewayEvent::ProofHealth`: los umbrales de PDP/PoTR exceden ہوں (proveedor, ventana, estado de huelga/enfriamiento, monto de la penalización) ۔
  Proveedor de consumidores کے لحاظ سے filtro کر کے نئی telemetría, acuerdos یا alertas de salud de prueba پر
  sondeo کے بغیر reaccionar کر سکتے ہیں۔

دونوں puntos finales SoraFS marco de cuotas میں نئے `torii.sorafs.quota.deal_telemetry` ventana کے ذریعے
شامل ہیں، جس سے operadores ہر implementación کے لیے ajuste de tasa de envío permitido کر سکتے ہیں۔