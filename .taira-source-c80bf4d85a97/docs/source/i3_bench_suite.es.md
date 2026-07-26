---
lang: es
direction: ltr
source: docs/source/i3_bench_suite.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a3158cd70a42104bacaafc520fdcc10e20e3bc347d895be448fcb10da4f668bd
source_last_modified: "2026-01-03T18:08:01.692664+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha Conjunto de 3 bancos

El conjunto de banco Iroha 3 mide los caminos activos en los que confiamos durante el replanteo, tarifa
cobro, verificación de prueba, programación y puntos finales de prueba. Funciona como un
Comando `xtask` con accesorios deterministas (semillas fijas, material de clave fijo,
y cargas útiles de solicitudes estables) para que los resultados sean reproducibles en todos los hosts.

## Ejecutando la suite

```bash
cargo xtask i3-bench-suite \
  --iterations 64 \
  --sample-count 5 \
  --json-out benchmarks/i3/latest.json \
  --csv-out benchmarks/i3/latest.csv \
  --markdown-out benchmarks/i3/latest.md \
  --threshold benchmarks/i3/thresholds.json \
  --allow-overwrite
```

Banderas:

- `--iterations` controla las iteraciones por muestra de escenario (predeterminado: 64).
- `--sample-count` repite cada escenario para calcular la mediana (predeterminado: 5).
- `--json-out|--csv-out|--markdown-out` elige artefactos de salida (todos opcionales).
- `--threshold` compara las medianas con los límites de referencia (establecido `--no-threshold`
  saltar).
- `--flamegraph-hint` anota el informe Markdown con `cargo flamegraph`
  comando para perfilar un escenario.

El pegamento CI reside en `ci/i3_bench_suite.sh` y utiliza de forma predeterminada las rutas anteriores; conjunto
`I3_BENCH_ITERATIONS`/`I3_BENCH_SAMPLES` para ajustar el tiempo de ejecución en las noches.

## Escenarios

- `fee_payer` / `fee_sponsor` / `fee_insufficient` — débito del pagador versus patrocinador
  y rechazo del déficit.
- `staking_bond` / `staking_slash` — cola de enlace/desvinculación con y sin
  cortando.
- `commit_cert_verify` / `jdg_attestation_verify` / `bridge_proof_verify` —
  verificación de firma sobre certificados de confirmación, certificaciones JDG y puente
  cargas útiles de prueba.
- `commit_cert_assembly`: ensamblaje de resumen para certificados de confirmación.
- `access_scheduler`: programación de conjuntos de acceso consciente de conflictos.
- `torii_proof_endpoint`: análisis de punto final de prueba de Axum + viaje de ida y vuelta de verificación.

Cada escenario registra una mediana de nanosegundos por iteración, rendimiento y una
contador de asignación determinista para regresiones rápidas. Los umbrales viven en
`benchmarks/i3/thresholds.json`; toparse con límites allí cuando el hardware cambia y
confirmar el nuevo artefacto junto con un informe.

## Solución de problemas

- Fije la frecuencia/gobernador de la CPU al recopilar evidencia para evitar regresiones ruidosas.
- Utilice `--no-threshold` para ejecuciones exploratorias y luego vuelva a habilitarlo una vez que se alcance la línea base.
  renovado.
- Para perfilar un único escenario, configure `--iterations 1` y vuelva a ejecutarlo en
  `cargo flamegraph -p xtask -- i3-bench-suite --iterations 128 --sample-count 1 --no-threshold --flamegraph-hint`.