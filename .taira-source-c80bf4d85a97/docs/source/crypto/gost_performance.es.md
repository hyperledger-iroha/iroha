---
lang: es
direction: ltr
source: docs/source/crypto/gost_performance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fab384ae80e1993b1e54d6addc82fd3dc652fb6e3958bea6a04e057a1805b57
source_last_modified: "2026-01-03T18:07:57.084090+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Flujo de trabajo de rendimiento GOST

Esta nota documenta cómo rastreamos y aplicamos el sobre de desempeño para el
TC26 backend de firma GOST.

## Ejecutando localmente

```bash
make gost-bench                     # run benches + tolerance check
make gost-bench GOST_BENCH_ARGS="--tolerance 0.30"  # override guard
make gost-dudect                    # run the constant-time timing guard
./scripts/update_gost_baseline.sh   # bench + rebaseline helper
```

Detrás de escena, ambos objetivos llaman a `scripts/gost_bench.sh`, que:

1. Ejecuta `cargo bench -p iroha_crypto --bench gost_sign --features gost -- --noplot`.
2. Ejecuta `gost_perf_check` contra `target/criterion`, verificando las medianas contra las
   línea base registrada (`crates/iroha_crypto/benches/gost_perf_baseline.json`).
3. Inyecta el resumen de Markdown en `$GITHUB_STEP_SUMMARY` cuando esté disponible.

Para actualizar la línea base después de aprobar una regresión/mejora, ejecute:

```bash
make gost-bench-update
```

o directamente:

```bash
./scripts/gost_bench.sh --write-baseline \
  --baseline crates/iroha_crypto/benches/gost_perf_baseline.json
```

`scripts/update_gost_baseline.sh` ejecuta el banco + verificador, sobrescribe el JSON de referencia e imprime
las nuevas medianas. Confirme siempre el JSON actualizado junto con el registro de decisión en
`crates/iroha_crypto/docs/gost_backend.md`.

### Medianas de referencia actuales

| Algoritmo | Mediana (μs) |
|----------------------|-------------|
| ed25519 | 69,67 |
| gost256_paramset_a | 1136,96 |
| gost256_paramset_b | 1129.05 |
| gost256_paramset_c | 1133,25 |
| gost512_paramset_a | 8944,39 |
| gost512_paramset_b | 8963,60 |
| secp256k1 | 160,53 |

## CI

`.github/workflows/gost-perf.yml` usa el mismo script y también ejecuta el protector de sincronización dudect.
CI falla cuando la mediana medida excede la línea base por más que la tolerancia configurada
(20% por defecto) o cuando el protector de sincronización detecta una fuga, por lo que las regresiones se detectan automáticamente.

## Salida resumida

`gost_perf_check` imprime la tabla de comparación localmente y agrega el mismo contenido a
`$GITHUB_STEP_SUMMARY`, por lo que los registros de trabajos de CI y los resúmenes de ejecución comparten los mismos números.