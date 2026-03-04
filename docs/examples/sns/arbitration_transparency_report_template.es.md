---
lang: es
direction: ltr
source: docs/examples/sns/arbitration_transparency_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 305a3f3b253a013825d4dd798d2282e111913ec777fe0fbf5b02a92c7172b92a
source_last_modified: "2025-11-15T07:34:14.070551+00:00"
translation_last_reviewed: 2026-01-01
---

# Reporte de transparencia de arbitraje SNS - <Mes YYYY>

- **Sufijo:** `<.sora / .nexus / .dao>`
- **Ventana de reporte:** `<ISO start>` -> `<ISO end>`
- **Preparado por:** `<Council liaison>`
- **Artefactos de origen:** `cases.ndjson` SHA256 `<hash>`, export de dashboard `<filename>.json`

## 1. Resumen ejecutivo

- Casos nuevos totales: `<count>`
- Casos cerrados en el periodo: `<count>`
- Cumplimiento de SLA: `<ack %>` acuse / `<resolution %>` decision
- Overrides del guardian emitidos: `<count>`
- Transferencias/reembolsos ejecutados: `<count>`

## 2. Mezcla de casos

| Tipo de disputa | Casos nuevos | Casos cerrados | Resolucion mediana (dias) |
|-----------------|-------------|---------------|---------------------------|
| Propiedad | 0 | 0 | 0 |
| Violacion de politica | 0 | 0 | 0 |
| Abuso | 0 | 0 | 0 |
| Facturacion | 0 | 0 | 0 |
| Otro | 0 | 0 | 0 |

## 3. Desempeno de SLA

| Prioridad | SLA de acuse | Cumplido | SLA de resolucion | Cumplido | Incumplimientos |
|-----------|--------------|----------|-------------------|----------|-----------------|
| Urgente | <= 2 h | 0% | <= 72 h | 0% | 0 |
| Alta | <= 8 h | 0% | <= 10 d | 0% | 0 |
| Estandar | <= 24 h | 0% | <= 21 d | 0% | 0 |
| Info | <= 3 d | 0% | <= 30 d | 0% | 0 |

Describe causas raiz de cualquier incumplimiento y enlaza tickets de remediacion.

## 4. Registro de casos

| ID de caso | Selector | Prioridad | Estado | Resultado | Notas |
|------------|----------|-----------|--------|-----------|-------|
| SNS-YYYY-NNNNN | `label.suffix` | Estandar | Cerrado | Ratificado | `<summary>` |

Provee notas de una linea que referencien hechos anonimizados o enlaces a votos publicos.
Sella donde sea necesario y menciona las redacciones aplicadas.

## 5. Acciones y remedios

- **Congelaciones / liberaciones:** `<counts + case ids>`
- **Transferencias:** `<counts + assets moved>`
- **Ajustes de facturacion:** `<credits/debits>`
- **Seguimientos de politica:** `<tickets or RFCs opened>`

## 6. Apelaciones y overrides del guardian

Resume cualquier apelacion escalada al guardian board, incluyendo timestamps y decisiones
(approve/deny). Enlaza registros de `sns governance appeal` o votos del council.

## 7. Pendientes

- `<Action item>` - Owner `<name>`, ETA `<date>`
- `<Action item>` - Owner `<name>`, ETA `<date>`

Adjunta NDJSON, exportes de Grafana y logs de CLI referenciados en este reporte.
