---
lang: es
direction: ltr
source: docs/examples/soranet_gateway_billing/reconciliation_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5c10cd7eda24260bfd1319c7b8ac23dba2a1c8a1cb39ea49f0f1a64427ca15db
source_last_modified: "2025-11-21T12:24:49.353535+00:00"
translation_last_reviewed: 2026-01-01
---

# Reconciliacion de facturacion de gateway SoraGlobal

- **Ventana:** `<from>/<to>`
- **Tenant:** `<tenant-id>`
- **Version de catalogo:** `<catalog-version>`
- **Snapshot de uso:** `<path or hash>`
- **Guardrails:** soft cap `<soft-cap-xor> XOR`, hard cap `<hard-cap-xor> XOR`, umbral de alerta `<alert-threshold>%`
- **Pagador -> Tesoreria:** `<payer>` -> `<treasury>` en `<asset-definition>`
- **Total adeudado:** `<total-xor> XOR` (`<total-micros>` micro-XOR)

## Chequeos de line items
- [ ] Entradas de uso cubren solo ids de medidor del catalogo y regiones de facturacion validas
- [ ] Unidades de cantidad coinciden con definiciones del catalogo (requests, GiB, ms, etc.)
- [ ] Multiplicadores por region y tiers de descuento aplicados segun catalogo
- [ ] Exportaciones CSV/Parquet coinciden con los line items de la factura JSON

## Evaluacion de guardrails
- [ ] Se alcanzo el umbral de alerta de soft cap? `<yes/no>` (adjunta evidencia de alerta si es yes)
- [ ] Hard cap excedido? `<yes/no>` (si yes, adjunta aprobacion de override)
- [ ] Se cumple el piso minimo de factura

## Proyeccion de ledger
- [ ] Total del lote de transferencias igual a `total_micros` en la factura
- [ ] Definicion de asset coincide con la moneda de facturacion
- [ ] Cuentas de pagador y tesoreria coinciden con tenant y operador registrado
- [ ] Artefactos Norito/JSON adjuntos para replay de auditoria

## Notas de disputa/ajuste
- Variacion observada: `<variance detail>`
- Ajuste propuesto: `<delta and rationale>`
- Evidencia de soporte: `<logs/dashboards/alerts>`

## Aprobaciones
- Analista de facturacion: `<name + signature>`
- Revisor de tesoreria: `<name + signature>`
- Hash de paquete de governance: `<hash/reference>`
