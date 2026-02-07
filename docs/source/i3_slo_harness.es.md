---
lang: es
direction: ltr
source: docs/source/i3_slo_harness.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: df3e3ac15baf47a6c53001acabcac7987a2386c2b772b1d8625eb60598f95a60
source_last_modified: "2026-01-03T18:08:01.691568+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Iroha 3 Arnés SLO

La línea de lanzamiento Iroha 3 incluye SLO explícitos para las rutas críticas Nexus:

- duración del intervalo de finalidad (cadencia NX‑18)
- verificación de pruebas (certificados de confirmación, certificaciones JDG, pruebas puente)
- Manejo de punto final de prueba (proxy de ruta de Axum a través de latencia de verificación)
- rutas de tarifas y apuestas (pagador/patrocinador y flujos de bonos/barra)

## Presupuestos

Los presupuestos se encuentran en `benchmarks/i3/slo_budgets.json` y se asignan directamente al banco
escenarios en la suite I3. Los objetivos son metas p99 por llamada:

- Tarifa/apuesta: 50 ms por llamada (`fee_payer`, `fee_sponsor`, `staking_bond`, `staking_slash`)
- Confirmar certificado / JDG / verificación de puente: 80 ms (`commit_cert_verify`, `jdg_attestation_verify`,
  `bridge_proof_verify`)
- Conjunto de certificado de confirmación: 80 ms (`commit_cert_assembly`)
- Programador de acceso: 50ms (`access_scheduler`)
- Proxy de punto final de prueba: 120 ms (`torii_proof_endpoint`)

Las sugerencias de velocidad de grabación (`burn_rate_fast`/`burn_rate_slow`) codifican la versión 14.4/6.0
Proporciones de ventanas múltiples para paginación versus alertas de tickets.

## Arnés

Ejecute el arnés a través de `cargo xtask i3-slo-harness`:

```bash
cargo xtask i3-slo-harness \
  --iterations 64 \
  --sample-count 5 \
  --out-dir artifacts/i3_slo/latest
```

Salidas:

- `bench_report.json|csv|md` — resultados sin procesar del banco de pruebas I3 (git hash + escenarios)
- `slo_report.json|md` — Evaluación de SLO con índice de aprobado/reprobado/presupuesto por objetivo

El arnés consume el archivo de presupuestos y aplica `benchmarks/i3/slo_thresholds.json`
durante la carrera de banco para fallar rápidamente cuando un objetivo retrocede.

## Telemetría y paneles de control

- Finalidad: `histogram_quantile(0.99, rate(iroha_slot_duration_ms_bucket[5m]))`
- Verificación de prueba: `histogram_quantile(0.99, sum by (le) (rate(zk_verify_latency_ms_bucket{status="Verified"}[5m])))`

Los paneles de inicio Grafana viven en `dashboards/grafana/i3_slo.json`. Prometheus
Las alertas de velocidad de grabación se proporcionan en `dashboards/alerts/i3_slo_burn.yml` con el
presupuestos anteriores integrados (finalidad 2, verificación de prueba 80 ms, proxy de punto final de prueba)
120 ms).

## Notas operativas

- Correr el arnés en pijamas; publicar `artifacts/i3_slo/<stamp>/slo_report.md`
  junto con los artefactos de banco para evidencia de gobernanza.
- Si un presupuesto falla, utilice la rebaja comparativa para identificar el escenario y luego profundice
  en el panel/alerta Grafana correspondiente para correlacionarlo con métricas en vivo.
- Los SLO de punto final de prueba utilizan la latencia de verificación como proxy para evitar rutas por ruta
  explosión de cardinalidad; el objetivo de referencia (120 ms) coincide con la retención/DoS
  barandillas en la API de prueba.