<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/source/nexus_cross_dataspace_localnet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2324cfc7b086ceb96317eb2260abe41101f17e5c0749d0a1d28ffbf4cb5e8e45
source_last_modified: "2026-02-19T18:33:20.275472+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Nexus Prueba de red local entre espacios de datos

Este runbook ejecuta la prueba de integración Nexus de que:

- inicia una red local de 4 pares con dos espacios de datos privados restringidos (`ds1`, `ds2`),
- enruta el tráfico de la cuenta a cada espacio de datos,
- crea un activo en cada espacio de datos,
- ejecuta liquidación de swap atómico en espacios de datos en ambas direcciones,
- demuestra la semántica de reversión al presentar un tramo sin fondos suficientes y verificar que los saldos permanezcan sin cambios.

La prueba canónica es:
`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`.

## Ejecución rápida

Utilice el script contenedor desde la raíz del repositorio:

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh
```

Comportamiento predeterminado:

- ejecuta solo la prueba de prueba entre espacios de datos,
- establece `NORITO_SKIP_BINDINGS_SYNC=1`,
- establece `IROHA_TEST_SKIP_BUILD=1`,
- utiliza `--test-threads=1`,
- pasa `--nocapture`.

## Opciones útiles

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh --keep-dirs
scripts/run_nexus_cross_dataspace_atomic_swap.sh --no-skip-build
scripts/run_nexus_cross_dataspace_atomic_swap.sh --release
scripts/run_nexus_cross_dataspace_atomic_swap.sh --all-nexus
```

- `--keep-dirs` mantiene directorios de pares temporales (`IROHA_TEST_NETWORK_KEEP_DIRS=1`) para análisis forense.
- `--all-nexus` ejecuta `mod nexus::` (subconjunto de integración completo Nexus), no solo la prueba de prueba.

## Puerta CI

Ayudante de CI:

```bash
ci/check_nexus_cross_dataspace_localnet.sh
```

Hacer objetivo:

```bash
make check-nexus-cross-dataspace
```

Esta puerta ejecuta el contenedor de prueba determinista y falla el trabajo si el atómico entre espacios de datos
el escenario de intercambio retrocede.

## Comandos equivalentes manuales

Prueba de prueba dirigida:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod \
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing \
  -- --nocapture --test-threads=1
```

Subconjunto completo Nexus:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod nexus:: -- --nocapture --test-threads=1
```

## Señales de prueba esperadas- La prueba pasa.
- Aparece una advertencia esperada para el tramo de liquidación con fondos insuficientes que falló intencionalmente:
  `settlement leg requires 10000 but only ... is available`.
- Las afirmaciones del saldo final tienen éxito después de:
  - intercambio a plazo exitoso,
  - intercambio inverso exitoso,
  - swap fallido y con financiación insuficiente (revertir saldos sin cambios).

## Instantánea de validación actual

A partir del **19 de febrero de 2026**, este flujo de trabajo pasó con:

- prueba dirigida: `1 passed; 0 failed`,
- subconjunto Nexus completo: `24 passed; 0 failed`.