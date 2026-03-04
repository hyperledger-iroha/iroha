---
lang: es
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/economic_analysis.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1b453559c05401edc11894e585c8d5ca4b678d4667c1cef0415582e1f7de8246
source_last_modified: "2025-11-05T17:23:10.790688+00:00"
translation_last_reviewed: 2026-01-01
---

# Analisis economico - Shadow Run 2025-10 -> 2025-11

Artefacto fuente: `docs/examples/soranet_incentive_shadow_run.json` (firma + clave publica en el mismo directorio). La simulacion reprodujo 60 epocas por relay con el motor de recompensas fijado a `RewardConfig` registrado en `reward_config.json`.

## Resumen de distribucion

- **Pagos totales:** 5,160 XOR en 360 epocas recompensadas.
- **Envolvente de fairness:** coeficiente Gini 0.121; participacion del relay superior 23.26%
  (muy por debajo del guardrail de governance 30%).
- **Disponibilidad:** promedio de flota 96.97%, todos los relays se mantuvieron arriba de 94%.
- **Ancho de banda:** promedio de flota 91.20%, con el peor desempeno en 87.23%
  durante mantenimiento planificado; penalizaciones aplicadas automaticamente.
- **Ruido de compliance:** se observaron 9 epocas de warning y 3 suspensiones y
  se tradujeron en reducciones de payout; ningun relay excedio el cap de 12 warnings.
- **Higiene operativa:** no se omitieron snapshots de metricas por falta de config,
  bonds o duplicados; no se emitieron errores del calculador.

## Observaciones

- Las suspensiones corresponden a epocas donde los relays entraron en modo mantenimiento. El
  motor de payout emitio pagos cero para esas epocas mientras preservaba el rastro
  de auditoria en el JSON del shadow-run.
- Las penalidades de warning recortaron 2% de los pagos afectados; la distribucion
  resultante sigue convergiendo gracias a los pesos de uptime/bandwidth (650/350 per mille).
- La variacion de bandwidth sigue el heatmap anonimo de guard. El peor desempeno
  (`6666...6666`) mantuvo 620 XOR en la ventana, por encima del piso 0.6x.
- Las alertas sensibles a latencia (`SoranetRelayLatencySpike`) se mantuvieron por debajo
  de los umbrales de warning durante toda la ventana; los dashboards relacionados se
  capturan en `dashboards/grafana/soranet_incentives.json`.

## Acciones recomendadas antes de GA

1. Sigue ejecutando replays shadow mensuales y actualiza el set de artefactos y este
   analisis si cambia la composicion de la flota.
2. Bloquea los pagos automaticos con la suite de alertas Grafana referenciada en la roadmap
   (`dashboards/alerts/soranet_incentives_rules.yml`); copia screenshots en las
   minutas de governance al solicitar renovacion.
3. Re-ejecuta la prueba de estres economico si el reward base, los pesos de uptime/bandwidth o
   la penalidad de compliance cambia en >=10%.
