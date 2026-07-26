---
lang: es
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/04-telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a947c289c13c15b09dfbbf28c23ae1539fd3e29ca3943fa8522c3eca32c28bf5
source_last_modified: "2025-11-04T17:24:14.014911+00:00"
translation_last_reviewed: 2026-01-01
---

# Requisitos de telemetria

## Targets de Prometheus

Recolecta el relay y el orchestrator con las siguientes labels:

```yaml
- job_name: "soranet-relay"
  static_configs:
    - targets: ["relay-host:9898"]
      labels:
        region: "testnet-t0"
        role: "relay"
- job_name: "sorafs-orchestrator"
  static_configs:
    - targets: ["orchestrator-host:9797"]
      labels:
        region: "testnet-t0"
        role: "orchestrator"
```

## Dashboards requeridos

1. `dashboards/grafana/soranet_testnet_overview.json` *(por publicar)* - carga el JSON e importa las variables `region` y `relay_id`.
2. `dashboards/grafana/soranet_privacy_metrics.json` *(activo SNNet-8)* - asegura que los paneles de privacy bucket se rendericen sin huecos.

## Reglas de alertas

Los umbrales deben coincidir con lo esperado en el playbook:

- `soranet_privacy_circuit_events_total{kind="downgrade"}` increase > 0 en 10 minutos dispara `critical`.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}` > 5 por 30 minutos dispara `warning`.
- `up{job="soranet-relay"}` == 0 por 2 minutos dispara `critical`.

Carga tus reglas en Alertmanager con el receiver `testnet-t0`; valida con `amtool check-config`.

## Evaluacion de metricas

Agrega un snapshot de 14 dias y alimentalo al validador SNNet-10:

```
cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json --out metrics-report.json
```

- Reemplaza el archivo sample con tu snapshot exportado cuando ejecutes con datos reales.
- Un resultado `status = fail` bloquea la promocion; resuelve los checks resaltados antes de reintentar.

## Reporte

Cada semana sube:

- Capturas de queries (`.png` o `.pdf`) que muestren el ratio PQ, la tasa de exito de circuitos y el histograma de resolucion PoW.
- Salida de la regla de recording de Prometheus para `soranet_privacy_throttles_per_minute`.
- Un breve relato describiendo las alertas que se dispararon y los pasos de mitigacion (incluye timestamps).
