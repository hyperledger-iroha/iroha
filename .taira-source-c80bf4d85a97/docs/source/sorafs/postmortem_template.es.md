---
lang: es
direction: ltr
source: docs/source/sorafs/postmortem_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7ed57867c09440d713694d5603ada2b20202d823025ba5cbe8e606b82575dcc5
source_last_modified: "2025-11-02T18:41:43.018956+00:00"
translation_last_reviewed: "2026-01-30"
---

# Plantilla de postmortem de incidentes SoraFS

> Duplica este archivo y reemplaza los placeholders. Adjunta dashboards,
> fragmentos de telemetria y decisiones de governance como apendices.

## Resumen

- **ID de incidente / drill:** `YYYY-MM-DD-<short-code>`
- **Fecha / ventana (UTC):** `<start> - <end>`
- **Clasificacion:** `<P1|P2|Drill>`
- **Impacto primario:** `<gateway availability | proof integrity | replication backlog | other>`
- **Detectado por:** `<alert name / manual report>`
- **Resuelto por:** `<team / individual>`

## Evaluacion de impacto

- **Sintomas de cara al cliente:** `<API errors, latency, backlog>`
- **Duracion sobre SLO:** `<minutes>`
- **Providers / manifests afectados:** `<list>`
- **Consecuencias downstream:** `<governance actions, replay required?>`

## Timeline

| Timestamp (UTC) | Actor | Evento |
|-----------------|-------|--------|
| `00:00` | `<system/team>` | Deteccion |
| `00:05` | | Mitigacion inicial |
| `...` | | |

## Analisis de causa raiz

- **Causa primaria:** `<component + failure mode>`
- **Factores contribuyentes:** `<config drift, telemetry gap, coordination>`
- **Que funciono:** `<items that reduced impact>`
- **Que fallo:** `<runbook gaps, tooling issues>`

## Telemetria y evidencia

- Adjuntar metricas relevantes (p. ej., `sorafs_fetch.error`, `sorafs_gateway_latency_ms_bucket`).
- Enlazar snapshots de paneles Grafana o logs con timestamps.

## Acciones correctivas

| Accion | Owner | Prioridad | Fecha objetivo |
|--------|-------|----------|----------------|
| `<Fix replication backlog alert thresholds>` | `<SRE>` | High | `<YYYY-MM-DD>` |

## Verificacion de seguimiento

- **Drill de caos programado:** `<yes/no> - planned date>`
- **Updates de runbook requeridos:** `<docs updated or pending>`
- **Notificacion a governance / stakeholders:** `<completed / outstanding>`

## Lecciones aprendidas

- `<Key observation>`
- `<Process improvement>`

## Apendice

- Links a issues, commits y dashboards relacionados.
- Adjuntar artefactos (snapshots de manifiesto, bundles de proof) si aplica.
