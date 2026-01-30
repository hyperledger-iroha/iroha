---
lang: es
direction: ltr
source: docs/source/sorafs_gateway_load_tests.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fbdb35ca448a6f30e15d2a7d258ce9e40614672089bc6b578021579f7ecc1b9f
source_last_modified: "2025-12-05T17:03:20.752641+00:00"
translation_last_reviewed: "2026-01-30"
---

# Plan de pruebas de carga para el gateway SoraFS

El harness de carga determinista ya se distribuye junto con la suite de replay. Ejecuta los
adaptadores de gateway basados en fixtures bajo ≥1.000 solicitudes concurrentes y registra
percentiles de latencia junto con desgloses de rechazos/errores. El harness vive en
`integration_tests/src/sorafs_gateway_conformance.rs` (`run_deterministic_load_test`) y se
habilita mediante la regresión `sorafs_gateway_deterministic_load_harness`. CI lo invoca con
`cargo test -p integration_tests sorafs_gateway_conformance`.

Los operadores pueden reutilizar el JSON emitido por `SuiteReport::to_json_value()` (ver
`ci/check_sorafs_gateway_conformance.sh`) para recolectar reportes de pruebas de carga firmados
para gobernanza.

Las secciones siguientes capturan mejoras pendientes y la matriz de escenarios usada por el harness.

## Objetivos

1. Sostener ≥1.000 streams de rango concurrentes mientras se validan pruebas y rechazos.
2. Medir latencia de cola (P95, P99) para escenarios de caché caliente y caché fría.
3. Inyectar corrupción controlada (alteración de chunks, mismatch de pruebas) y afirmar rechazo y logging determinista.
4. Producir reportes de pruebas de carga firmados consumibles por operadores y gobernanza.

## Desglose de trabajo (borrador)

| Tarea | Responsable(s) | Notas |
|------|-----------------|-------|
| Implementar pool determinista de workers | QA Guild / Tooling WG | Reutilizar ejecutor basado en Tokio; soportar RNG con semilla para orden de solicitudes reproducible. |
| Integrar fixtures de replay | QA Guild | Streamear fixtures CAR canónicos, pruebas PoR y casos negativos. |
| Recolectar métricas | Observabilidad | Capturar histogramas de latencia, conteos de rechazo, throughput, CPU/memoria. |
| Inyección de fallos | QA Guild | Voltear bits en payload, alterar nodos de prueba, omitir headers requeridos. |
| Reporting | Tooling WG | Emitir reportes JSON/CSV con estadísticas y artefactos de fallos. |

## Matriz de escenarios

| ID | Descripción | Carga | Resultado esperado |
|----|-------------|-------|-------------------|
| L1 | Streaming de rango con caché caliente | 1.000 concurrentes, corrida de 10 minutos | P95 < 120 ms, P99 < 250 ms, cero fallos de prueba |
| L2 | Bootstrap completo de CAR en caché fría | 250 concurrentes | P95 < 500 ms, validación de pruebas exitosa |
| L3 | Inyección de corrupción | 1% de solicitudes alteradas | Todas las respuestas corruptas rechazadas (422) |
| L4 | Desajuste de admisión | 5% solicitudes sin sobre de manifiesto | Rechazo 428 con telemetría registrada |
| L5 | Simulacro de rate-limit GAR | Burst más allá del límite configurado | 429 con motivo `rate_limited` |
| L6 | Intento de downgrade de headers | 2% solicitudes sin headers de trustless | Gateway devuelve 428 `required_headers_missing`, telemetría etiquetada |

## Métricas y telemetría

Los gateways deben exponer métricas Prometheus (vía `/metrics`) o logs JSON para:

- `sorafs_gateway_latency_ms_bucket{scenario}` — Histogramas de latencia por escenario.
- `sorafs_gateway_refusals_total{reason}` — Conteos de rechazo por motivo (unsupported_chunker, proof_verification_failed, etc.).
- `sorafs_gateway_bytes_total` — Total de bytes servidos por prueba.
- `sorafs_gateway_concurrency_active` — Gauge de concurrencia.

El harness de carga debe emitir:

- Throughput por segundo.
- Percentiles de latencia (P50, P95, P99, max).
- Resúmenes de fallos (motivo, conteo, primer timestamp).
- Contadores de verificación de pruebas (aceptadas vs rechazadas).
- Conteos de enforcement de admisión (sobre faltante, sobre expirado).

## Cobertura de inyección de fallos

- **Corrupción de pruebas:** Bit-flip en muestras dentro de pruebas PoR; garantizar 422 y log del fallo.
- **Downgrade de headers:** Omitir headers trustless requeridos (p. ej., `X-SoraFS-Nonce`), esperar 428.
- **Desajuste de admisión:** Omitir el sobre del manifiesto o enviar sobres expirados, esperar 428.
- **Límite GAR:** Exceder caps configurados de tasa/egreso para disparar 429 con etiquetado de telemetría.

## Decisiones pendientes

- Objetivo final de concurrencia para caché fría (requiere sizing de hardware).
- Si exigir cobertura HTTP/3 en la primera iteración.
- Integración con runners internos de CI (GitHub Actions vs rigs dedicados).
- Checks de salud post-run automatizados (p. ej., verificar que no queden warnings de expiración de admisión).

## Próximos pasos

1. Finalizar la lista de fixtures y rangos de chunks para runs de caché caliente/fría.
2. Extender el worker pool para cubrir gateways HTTP/3 cuando el transporte llegue.
3. Validar en gateway de staging interno y registrar métricas baseline.
4. Iterar sobre la estrategia de inyección de fallos antes de abrir a operadores.
