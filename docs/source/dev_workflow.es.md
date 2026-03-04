---
lang: es
direction: ltr
source: docs/source/dev_workflow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3be11bea2cb39520bc3c09f4614555d4ac97760e3590609e2f8df27bf28d1a1a
source_last_modified: "2026-01-18T17:14:31.034360+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Flujo de trabajo de desarrollo de AGENTES

Este runbook consolida las barreras de seguridad para contribuyentes de la hoja de ruta de AGENTES para que
Los nuevos parches siguen las mismas puertas predeterminadas.

## Objetivos de inicio rápido

- Ejecute `make dev-workflow` (envoltorio alrededor de `scripts/dev_workflow.sh`) para ejecutar:
  1. `cargo fmt --all`
  2. `cargo clippy --workspace --all-targets --locked -- -D warnings`
  3. `cargo build --workspace --locked`
  4. `cargo test --workspace --locked`
  5. `swift test` de `IrohaSwift/`
- `cargo test --workspace` dura mucho tiempo (a menudo horas). Para iteraciones rápidas,
  use `scripts/dev_workflow.sh --skip-tests` o `--skip-swift`, luego ejecute el completo
  secuencia antes del envío.
- Si `cargo test --workspace` se detiene al bloquear el directorio de compilación, vuelva a ejecutarlo con
  `scripts/dev_workflow.sh --target-dir target/codex-tests` (o configurar
  `CARGO_TARGET_DIR` a una ruta aislada) para evitar conflictos.
- Todos los pasos de carga utilizan `--locked` para respetar la política del repositorio de mantener
  `Cargo.lock` intacto. Prefiere ampliar las cajas existentes en lugar de agregar
  nuevos miembros del espacio de trabajo; Busque aprobación antes de introducir una nueva caja.

## Barandillas- `make check-agents-guardrails` (o `ci/check_agents_guardrails.sh`) falla si
  La rama modifica `Cargo.lock`, introduce nuevos miembros del espacio de trabajo o agrega nuevos
  dependencias. El script compara el árbol de trabajo y `HEAD` con
  `origin/main` por defecto; configure `AGENTS_BASE_REF=<ref>` para anular la base.
- `make check-dependency-discipline` (o `ci/check_dependency_discipline.sh`)
  diferencia las dependencias `Cargo.toml` con respecto a la base y falla en cajas nuevas; conjunto
  `DEPENDENCY_DISCIPLINE_ALLOW=<dep1,dep2>` para reconocer intencional
  adiciones.
- Bloques `make check-missing-docs` (o `ci/check_missing_docs_guard.sh`) nuevos
  Entradas `#[allow(missing_docs)]`, banderas tocaron cajas (`Cargo.toml` más cercana)
  cuyo `src/lib.rs`/`src/main.rs` carece de documentos `//!` a nivel de caja y rechaza nuevos
  elementos públicos sin documentos `///` relativos a la referencia base; conjunto
  `MISSING_DOCS_GUARD_ALLOW=1` solo con la aprobación del revisor. El guardia también
  verifica que `docs/source/agents/missing_docs_inventory.{json,md}` sean nuevos;
  regenerar con `python3 scripts/inventory_missing_docs.py`.
- `make check-tests-guard` (o `ci/check_tests_guard.sh`) señala cajas cuyo
  Las funciones modificadas de Rust carecen de evidencia de pruebas unitarias. Los mapas de guardia cambiaron de línea.
  a las funciones, pasa si las pruebas de caja cambiaron en la diferencia y, de lo contrario, escanea
  archivos de prueba existentes para hacer coincidir llamadas a funciones, por lo que la cobertura preexistente
  cuenta; las cajas sin ninguna prueba coincidente fallarán. Conjunto `TEST_GUARD_ALLOW=1`
  sólo cuando los cambios sean verdaderamente neutrales y el revisor esté de acuerdo.
- `make check-docs-tests-metrics` (o `ci/check_docs_tests_metrics_guard.sh`)
  hace cumplir la política de hoja de ruta de que los hitos avanzan junto con la documentación,
  pruebas y métricas/paneles de control. Cuando `roadmap.md` cambia en relación con
  `AGENTS_BASE_REF`, el guardia espera al menos un cambio de documento, un cambio de prueba,
  y un cambio de métricas/telemetría/panel. Conjunto `DOC_TEST_METRIC_GUARD_ALLOW=1`
  sólo con la aprobación del revisor.
- `make check-todo-guard` (o `ci/check_todo_guard.sh`) falla cuando los marcadores TODO
  desaparecer sin los cambios de documentos/pruebas que lo acompañen. Agregar o actualizar cobertura
  al resolver un TODO, o configurar `TODO_GUARD_ALLOW=1` para eliminaciones intencionales.
- Bloques `make check-std-only` (o `ci/check_std_only.sh`) `no_std`/`wasm32`
  cfgs para que el espacio de trabajo siga siendo solo `std`. Configure `STD_ONLY_GUARD_ALLOW=1` solo para
  experimentos de CI autorizados.
- `make check-status-sync` (o `ci/check_status_sync.sh`) mantiene abierta la hoja de ruta
  sección libre de elementos completados y requiere `roadmap.md`/`status.md` para
  cambiar juntos para que el plan y el estado permanezcan alineados; conjunto
  `STATUS_SYNC_ALLOW_UNPAIRED=1` solo para correcciones de errores tipográficos poco comunes después de
  fijación `AGENTS_BASE_REF`.
- `make check-proc-macro-ui` (o `ci/check_proc_macro_ui.sh`) ejecuta trybuild
  Suites de interfaz de usuario para cajas de derivación/proc-macro. Ejecútelo cuando toque proc-macros para
  mantenga estables los diagnósticos de `.stderr` y detecte regresiones de IU en pánico; conjunto
  `PROC_MACRO_UI_CRATES="crate1 crate2"` para centrarse en cajas específicas.
- Reconstrucciones `make check-env-config-surface` (o `ci/check_env_config_surface.sh`)
  el inventario de alternancia de entorno (`docs/source/agents/env_var_inventory.{json,md}`),
  falla si está obsoleto, **y** falla cuando aparecen nuevas correcciones ambientales de producción
  relativo a `AGENTS_BASE_REF` (detectado automáticamente; configurado explícitamente cuando sea necesario).
  Actualice el rastreador después de agregar/eliminar búsquedas de entorno a través de
  `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`;
  use `ENV_CONFIG_GUARD_ALLOW=1` solo después de documentar las perillas ambientales intencionalesen el rastreador de migración.
- `make check-serde-guard` (o `ci/check_serde_guard.sh`) regenera el serde
  inventario de uso (`docs/source/norito_json_inventory.{json,md}`) en un archivo temporal
  ubicación, falla si el inventario comprometido está obsoleto y rechaza cualquier nuevo
  la producción `serde`/`serde_json` acierta en relación con `AGENTS_BASE_REF`. conjunto
  `SERDE_GUARD_ALLOW=1` solo para experimentos de CI después de presentar un plan de migración.
- `make guards` aplica la política de serialización Norito: niega nuevas
  Uso de `serde`/`serde_json`, ayudantes AoS ad-hoc y dependencias SCALE externas
  los bancos Norito (`scripts/deny_serde_json.sh`,
  `scripts/check_no_direct_serde.sh`, `scripts/deny_handrolled_aos.sh`,
  `scripts/check_no_scale.sh`).
- **Política de interfaz de usuario de proc-macro:** cada caja de proc-macro debe enviarse con un `trybuild`
  arnés (`tests/ui.rs` con puntos de aprobación/falla) detrás del `trybuild-tests`
  característica. Coloque muestras de camino feliz en `tests/ui/pass`, casos de rechazo en
  `tests/ui/fail` con salidas `.stderr` comprometidas y mantener diagnósticos
  sin pánico y estable. Actualizar accesorios con
  `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` (opcionalmente con
  `CARGO_TARGET_DIR=target-codex` para evitar dañar las compilaciones existentes) y
  evite depender de compilaciones de cobertura (se esperan guardias `cfg(not(coverage))`).
  Para macros que no emiten un punto de entrada binario, prefiera
  `// compile-flags: --crate-type lib` en los aparatos para mantener los errores enfocados. Añadir
  nuevos casos negativos cada vez que cambian los diagnósticos.
- CI ejecuta los scripts de guardarrail a través de `.github/workflows/agents-guardrails.yml`
  por lo que las solicitudes de extracción fallan rápidamente cuando se violan las políticas.
- El git hook de muestra (`hooks/pre-commit.sample`) ejecuta guardrail, dependencia,
  scripts de documentos faltantes, solo estándar, env-config y status-sync para que los contribuyentes
  detectar violaciones de políticas antes que CI. Mantenga rutas de navegación TODO para cualquier
  seguimientos en lugar de posponer grandes cambios en silencio.