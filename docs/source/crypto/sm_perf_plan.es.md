---
lang: es
direction: ltr
source: docs/source/crypto/sm_perf_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 493c3c0f6a991b2a5d04f33f97b7e97bff372271c5c57751ff41f5e86d43cbc7
source_last_modified: "2026-01-03T18:07:57.107521+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Captura de rendimiento de SM y plan de referencia

Estado: Redactado — 2025-05-18  
Propietarios: Performance WG (líder), Infra Ops (programación de laboratorio), QA Guild (control de CI)  
Tareas de hoja de ruta relacionadas: SM-4c.1a/b, SM-5a.3b, captura entre dispositivos FASTPQ Stage 7

### 1. Objetivos
1. Registre las medianas de Neoverse en `sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json`. Las líneas de base actuales se exportan desde la captura `neoverse-proxy-macos` en `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/` (etiqueta de CPU `neoverse-proxy-macos`) con la tolerancia de comparación SM3 ampliada a 0,70 para aarch64 macOS/Linux. Cuando se abra el tiempo básico, vuelva a ejecutar `scripts/sm_perf_capture_helper.sh --matrix --cpu-label neoverse-n2-b01 --output artifacts/sm_perf/<date>/neoverse-n2-b01` en el host Neoverse y promueva las medianas agregadas a las líneas de base.  
2. Reúna las medianas x86_64 coincidentes para que `ci/check_sm_perf.sh` pueda proteger ambas clases de host.  
3. Publicar un procedimiento de captura repetible (comandos, diseño de artefactos, revisores) para que futuras puertas de rendimiento no dependan del conocimiento tribal.

### 2. Disponibilidad de hardware
Solo se puede acceder a los hosts Apple Silicon (macOS arm64) en el espacio de trabajo actual. La captura `neoverse-proxy-macos` se exporta como línea base provisional de Linux, pero la captura de medianas Neoverse o x86_64 sin sistema operativo aún requiere que el hardware de laboratorio compartido rastreado en `INFRA-2751`, lo ejecute Performance WG una vez que se abra la ventana del laboratorio. Las ventanas de captura restantes ahora se reservan y se rastrean en el árbol de artefactos:

- Neoverse N2 bare-metal (rack B de Tokio) reservado para el 12 de marzo de 2026. Los operadores reutilizarán los comandos de la Sección 3 y almacenarán los artefactos en `artifacts/sm_perf/2026-03-lab/neoverse-b01/`.
- x86_64 Xeon (rack D de Zurich) reservado para el 19 de marzo de 2026 con SMT desactivado para reducir el ruido; Los artefactos aterrizarán bajo `artifacts/sm_perf/2026-03-lab/xeon-d01/`.
- Después de que ambas ejecuciones lleguen, promueva las medianas a los archivos JSON de referencia y habilite la puerta CI en `ci/check_sm_perf.sh` (fecha de cambio objetivo: 2026-03-25).

Hasta esas fechas, solo las líneas base de macOS arm64 se pueden actualizar localmente.### 3. Procedimiento de captura
1. **Sincronizar cadenas de herramientas**  
   ```bash
   rustup override set $(cat rust-toolchain.toml)
   cargo fetch
   ```
2. **Generar matriz de captura** (por host)  
   ```bash
   scripts/sm_perf_capture_helper.sh --matrix \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}
   ```
   El asistente ahora escribe `capture_commands.sh` e `capture_plan.json` en el directorio de destino. El script configura rutas de captura `raw/*.json` por modo para que los técnicos de laboratorio puedan realizar lotes de ejecuciones de forma determinista.
3. **Ejecutar capturas**  
   Ejecute cada comando desde `capture_commands.sh` (o ejecute el equivalente manualmente), asegurándose de que cada modo emita un blob JSON estructurado a través de `--capture-json`. Proporcione siempre una etiqueta de host a través de `--cpu-label "<model/bin>"` (o `SM_PERF_CPU_LABEL=<label>`) para que los metadatos de captura y las líneas de base posteriores registren el hardware exacto que produjo las medianas. El ayudante ya indica el camino adecuado; para ejecuciones manuales el patrón es:
   ```bash
   SM_PERF_CAPTURE_LABEL=auto \
   scripts/sm_perf.sh --mode auto \
     --cpu-label "neoverse-n2-lab-b01" \
     --capture-json artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/auto.json
   ```
4. **Validar resultados**  
   ```bash
   scripts/sm_perf_check \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json
   ```
   Asegúrese de que la variación se mantenga dentro del ±3 % entre ejecuciones. De lo contrario, vuelva a ejecutar el modo afectado y anote el reintento en el registro.
5. **Promover las medianas**  
   Utilice `scripts/sm_perf_aggregate.py` para calcular las medianas y copiarlas en los archivos JSON de referencia:
   ```bash
   scripts/sm_perf_aggregate.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json
   ```
   Los grupos auxiliares capturados por `metadata.mode`, validan que cada conjunto comparte el
   mismo triple `{target_arch, target_os}` y emite un resumen JSON con una entrada
   por modo. Las medianas que deberían ubicarse en los archivos de referencia se encuentran bajo
   `modes.<mode>.benchmarks`, mientras que los registros de bloque `statistics` que lo acompañan
   la lista completa de muestras, mín/máx, media y desviación estándar de la población para revisores y CI.
   Una vez que existe el archivo agregado, puede escribir automáticamente los JSON de referencia (con
   el mapa de tolerancia estándar) a través de:
   ```bash
   scripts/sm_perf_promote_baseline.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json \
     --out-dir crates/iroha_crypto/benches \
     --target-os unknown_linux_gnu \
     --overwrite
   ```
   Anule `--mode` para restringir a un subconjunto o `--cpu-label` para fijar el
   nombre de CPU registrado cuando la fuente agregada lo omite.
   Una vez que finalicen ambos hosts por arquitectura, actualice:
   - `sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json`
   - `sm_perf_baseline_x86_64_unknown_linux_gnu_{scalar,auto}.json` (nuevo)

   Los archivos `aarch64_unknown_linux_gnu_*` ahora reflejan el `m3-pro-native`
   captura (etiqueta de la CPU y notas de metadatos conservadas) para que `scripts/sm_perf.sh` pueda
   Detección automática de hosts aarch64-unknown-linux-gnu sin indicadores manuales. cuando el
   Se completa la ejecución del laboratorio completo, vuelva a ejecutar `scripts/sm_perf.sh --mode 
   --write-baseline crates/iroha_crypto/benches/sm_perf_baseline_aarch64_unknown_linux_gnu_.json`
   con las nuevas capturas para sobrescribir las medianas provisionales y sellar las reales
   etiqueta de host.

   > Referencia: la captura de Apple Silicon de julio de 2025 (etiqueta de CPU `m3-pro-local`) es
   > archivado en `artifacts/sm_perf/2025-07-lab/takemiyacStudio.lan/{raw,aggregated.json}`.
   > Refleje ese diseño cuando publique los artefactos de Neoverse/x86 para que los revisores
   > puede diferenciar los resultados brutos/agregados de manera consistente.

### 4. Diseño y aprobación de artefactos
```
artifacts/sm_perf/
  2025-07-lab/
    neoverse-b01/
      raw/
      aggregated.json
      run-log.md
    neoverse-b02/
      …
    xeon-d01/
    xeon-d02/
```
- `run-log.md` registra el hash del comando, la revisión de git, el operador y cualquier anomalía.
- Los archivos JSON agregados se incorporan directamente a las actualizaciones de referencia y se adjuntan a la revisión de rendimiento en `docs/source/crypto/sm_perf_baseline_comparison.md`.
- QA Guild revisa los artefactos antes de que cambien las líneas de base y aprueba en `status.md` en la sección Rendimiento.### 5. Cronología de activación de CI
| Fecha | Hito | Acción |
|------|-----------|--------|
| 2025-07-12 | Capturas de Neoverse completas | Actualice los archivos JSON `sm_perf_baseline_aarch64_*`, ejecute `ci/check_sm_perf.sh` localmente, abra PR con los artefactos adjuntos. |
| 2025-07-24 | capturas x86_64 completas | Agregue nuevos archivos de referencia + activación en `ci/check_sm_perf.sh`; asegúrese de que los carriles CI que cruzan el arco los consuman. |
| 2025-07-27 | Aplicación de CI | Habilite el flujo de trabajo `sm-perf-gate` para que se ejecute en ambas clases de host; las fusiones fallan si las regresiones exceden las tolerancias configuradas. |

### 6. Dependencias y comunicación
- Coordinar cambios de acceso al laboratorio vía `infra-ops@iroha.tech`.  
- Performance WG publica actualizaciones diarias en el canal `#perf-lab` mientras se ejecutan las capturas.  
- QA Guild prepara la comparación (`scripts/sm_perf_compare.py`) para que los revisores puedan visualizar deltas.  
- Una vez que se fusionen las líneas de base, actualice `roadmap.md` (SM-4c.1a/b, SM-5a.3b) e `status.md` con notas de finalización de captura.

Con este plan, el trabajo de aceleración de SM obtiene medianas reproducibles, activación de CI y un rastro de evidencia rastreable, satisfaciendo el elemento de acción "reservar ventanas de laboratorio y capturar medianas".

### 7. Puerta CI y humo local

- `ci/check_sm_perf.sh` es el punto de entrada de CI canónico. Se desembolsa en `scripts/sm_perf.sh` para cada modo en `SM_PERF_MODES` (el valor predeterminado es `scalar auto neon-force`) y configura `CARGO_NET_OFFLINE=true` para que los bancos se ejecuten de manera determinista en las imágenes de CI.  
- `.github/workflows/sm-neon-check.yml` ahora llama a la puerta en el ejecutor arm64 de macOS para que cada solicitud de extracción ejercite el trío escalar/automático/neon-force a través del mismo asistente usado localmente; el carril complementario de Linux/Neoverse se conectará una vez que x86_64 capture la tierra y las líneas de base del proxy de Neoverse se actualicen con la ejecución básica.  
- Los operadores pueden anular la lista de modos localmente: `SM_PERF_MODES="scalar" bash ci/check_sm_perf.sh` recorta la ejecución a una sola pasada para una prueba de humo rápida, mientras que los argumentos adicionales (por ejemplo, `--tolerance 0.20`) se reenvían directamente a `scripts/sm_perf.sh`.  
- `make check-sm-perf` ahora envuelve la puerta para comodidad del desarrollador; Los trabajos de CI pueden invocar el script directamente mientras los desarrolladores de macOS aprovechan el destino de creación.  
- Una vez que lleguen las líneas base de Neoverse/x86_64, el mismo script seleccionará el JSON apropiado a través de la lógica de detección automática de host ya presente en `scripts/sm_perf.sh`, por lo que no se necesita cableado adicional en los flujos de trabajo más allá de configurar la lista de modos deseados por grupo de hosts.

### 8. Ayudante de actualización trimestral- Ejecute `scripts/sm_perf_quarterly.sh --owner "<name>" --cpu-label "<label>" [--quarter YYYY-QN] [--output-root artifacts/sm_perf]` para crear un directorio con un cuarto de sello, como `artifacts/sm_perf/2026-Q1/<label>/`. El asistente envuelve `scripts/sm_perf_capture_helper.sh --matrix` y emite `capture_commands.sh`, `capture_plan.json` e `quarterly_plan.json` (propietario + metadatos trimestrales) para que los operadores del laboratorio puedan programar ejecuciones sin planes escritos a mano.
- Ejecute el `capture_commands.sh` generado en el host de destino, agregue las salidas sin procesar con `scripts/sm_perf_aggregate.py --output <dir>/aggregated.json` y promueva las medianas a los JSON de referencia a través de `scripts/sm_perf_promote_baseline.py --out-dir crates/iroha_crypto/benches --overwrite`. Vuelva a ejecutar `ci/check_sm_perf.sh` para confirmar que las tolerancias permanezcan en verde.
- Cuando cambien el hardware o las cadenas de herramientas, actualice las tolerancias/notas de comparación en `docs/source/crypto/sm_perf_baseline_comparison.md`, ajuste las tolerancias `ci/check_sm_perf.sh` si las nuevas medianas se estabilizan y alinee los umbrales de alerta/panel con las nuevas líneas de base para que las alarmas de operaciones sigan siendo significativas.
- Confirmar `quarterly_plan.json`, `capture_plan.json`, `capture_commands.sh` y el JSON agregado junto con las actualizaciones de referencia; adjunte los mismos artefactos a las actualizaciones de estado/hoja de ruta para la trazabilidad.