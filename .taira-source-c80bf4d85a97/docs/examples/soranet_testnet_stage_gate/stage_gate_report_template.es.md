---
lang: es
direction: ltr
source: docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 06dcd662ffb2b22a13d9cd5418f5d2e8e64a4cdadb71a054488ce75b1eb96188
source_last_modified: "2025-11-07T11:07:58.852037+00:00"
translation_last_reviewed: 2026-01-01
---

# Reporte de stage-gate SNNet-10 (T?_ -> T?_)

> Reemplaza cada placeholder (items entre < >) antes del envio. Conserva los encabezados de seccion para que la automatizacion de governance pueda parsear el archivo.

## 1. Metadatos

| Campo | Valor |
|-------|-------|
| Promocion | `<T0->T1 o T1->T2>` |
| Ventana de reporte | `<YYYY-MM-DD -> YYYY-MM-DD>` |
| Relays en alcance | `<count + IDs o "ver apendice A">` |
| Contacto principal | `<name/email/Matrix handle>` |
| Archivo de envio | `<snnet10-stage-gate-YYYYMMDD.tar.zst>` |
| Archivo SHA-256 | `<sha256:...>` |

## 2. Resumen de metricas

| Metrica | Observado | Umbral | Pasa? | Fuente |
|--------|----------|--------|------|--------|
| Ratio de exito de circuitos | `<0.000>` | >=0.95 | N / Y | `reports/metrics-report.json` |
| Ratio de brownout de fetch | `<0.000>` | <=0.01 | N / Y | `reports/metrics-report.json` |
| Varianza de mix GAR | `<+0.0%>` | <=+/-10% | N / Y | `reports/metrics-report.json` |
| PoW p95 en segundos | `<0.0 s>` | <=3 s | N / Y | `telemetry/pow_window.json` |
| Latencia p95 | `<0 ms>` | <200 ms | N / Y | `telemetry/latency_window.json` |
| Ratio PQ (promedio) | `<0.00>` | >= target | N / Y | `telemetry/pq_summary.json` |

**Narrativa:** `<summaries of anomalies, mitigations, overrides>`

## 3. Registro de drill e incidentes

| Timestamp (UTC) | Region | Tipo | Alert ID | Resumen de mitigacion |
|-----------------|--------|------|----------|-----------------------|
| `<YYYY-MM-DD HH:MM>` | `<region>` | `Brownout drill` | `<alert://...>` | `<restored anon-guard-pq in 3m12s>` |

## 4. Adjuntos y hashes

| Artefacto | Ruta | SHA-256 |
|----------|------|---------|
| Snapshot de metricas | `reports/metrics-window.json` | `<sha256>` |
| Reporte de metricas | `reports/metrics-report.json` | `<sha256>` |
| Transcripciones de guard rotation | `evidence/guard_rotation/*.log` | `<sha256>` |
| Manifiestos de exit bonding | `evidence/exit_bonds/*.to` | `<sha256>` |
| Logs de drill | `evidence/drills/*.md` | `<sha256>` |
| MASQUE readiness (T1->T2) | `reports/masque-readiness.md` | `<sha256 or n/a>` |
| Plan de rollback (T1->T2) | `reports/downgrade_plan.md` | `<sha256 or n/a>` |

## 5. Aprobaciones

| Rol | Nombre | Firmado (S/N) | Notas |
|-----|--------|--------------|-------|
| Networking TL | `<name>` | N / Y | `<comments>` |
| Governance rep | `<name>` | N / Y | `<comments>` |
| SRE delegate | `<name>` | N / Y | `<comments>` |

## Apendice A - Lista de relays

```
- relay-id-001 (AS64496, region=SJC)
- relay-id-002 (AS64497, region=NRT)
...
```

## Apendice B - Resumenes de incidentes

```
<Detailed context for any incidents or overrides referenced above.>
```
