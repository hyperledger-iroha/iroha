---
lang: es
direction: ltr
source: docs/source/fastpq_rollout_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3a0c22a213e04a6a8fef94ded6ec0017531737ffd4b9418ec94286bb6759ff8a
source_last_modified: "2026-01-08T09:26:20.579700+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Guía de implementación de FASTPQ (Etapa 7-3)

Este manual implementa el requisito de la hoja de ruta Stage7-3: cada actualización de la flota
que permite la ejecución de FASTPQ GPU debe adjuntar un manifiesto de referencia reproducible,
evidencia Grafana emparejada y un simulacro de reversión documentado. Se complementa
`docs/source/fastpq_plan.md` (objetivos/arquitectura) y
`docs/source/fastpq_migration_guide.md` (pasos de actualización a nivel de nodo) centrándose
en la lista de verificación de implementación de cara al operador.

## Alcance y funciones

- **Ingeniería de lanzamiento / SRE:** capturas de referencia propias, firma de manifiesto y
  exportaciones del panel antes de la aprobación de la implementación.
- **Ops Guild:** realiza implementaciones por etapas, graba ensayos de reversión y almacena
  el paquete de artefactos en `artifacts/fastpq_rollouts/<timestamp>/`.
- **Gobernanza/Cumplimiento:** verifica que la evidencia acompañe cada cambio
  solicitud antes de que se cambie el valor predeterminado de FASTPQ para una flota.

## Requisitos del paquete de pruebas

Cada envío de implementación debe contener los siguientes artefactos. Adjuntar todos los archivos
al ticket de lanzamiento/actualización y mantenga el paquete en
`artifacts/fastpq_rollouts/<YYYYMMDD>/<fleet>/<lane>/`.| Artefacto | Propósito | Cómo producir |
|----------|---------|----------------|
| `fastpq_bench_manifest.json` | Demuestra que la carga de trabajo canónica de 20000 filas se mantiene por debajo del límite LDE `<1 s` y registra hashes para cada punto de referencia ajustado.| Capture ejecuciones de Metal/CUDA, envuélvalas y luego ejecute:`cargo xtask fastpq-bench-manifest \``  --bench metal=artifacts/fastpq_benchmarks/<metal>.json \``  --bench cuda=artifacts/fastpq_benchmarks/<cuda>.json \``  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \``  --signing-key secrets/fastpq_bench.ed25519 \``  --out artifacts/fastpq_rollouts/<stamp>/fastpq_bench_manifest.json` |
| Puntos de referencia envueltos (`fastpq_metal_bench_*.json`, `fastpq_cuda_bench_*.json`) | Capture metadatos del host, evidencia de uso de filas, puntos de acceso de relleno cero, resúmenes de microbench de Poseidon y estadísticas del kernel utilizadas por paneles/alertas.| Ejecute `fastpq_metal_bench` / `fastpq_cuda_bench`, luego ajuste el JSON sin formato:`python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 \``  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \``  --poseidon-metrics artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom \``  fastpq_metal_bench.json artifacts/fastpq_benchmarks/<metal>.json --sign-output`Repita para capturas CUDA (punto `--row-usage` e `--poseidon-metrics` en los archivos de testigos/extracciones pertinentes). El asistente incorpora las muestras filtradas `fastpq_poseidon_pipeline_total`/`fastpq_execution_mode_total` para que la evidencia de WP2-E.6 sea idéntica en Metal y CUDA. Utilice `scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>` cuando necesite un resumen de microbanco Poseidon independiente (se admiten entradas empaquetadas o sin procesar). |
|  |  | **Requisito de etiqueta de etapa 7:** `wrap_benchmark.py` ahora falla a menos que la sección `metadata.labels` resultante contenga tanto `device_class` como `gpu_kind`. Cuando la detección automática no puede inferirlos (por ejemplo, al ajustar en un nodo de CI desconectado), pase anulaciones explícitas como `--label device_class=xeon-rtx-sm80 --label gpu_kind=discrete`. |
|  |  | **Telemetría de aceleración:** el contenedor también captura `cargo xtask acceleration-state --format json` de forma predeterminada, escribiendo `<bundle>.accel.json` e `<bundle>.accel.prom` junto al punto de referencia empaquetado (anular con indicadores `--accel-*` o `--skip-acceleration-state`). La matriz de captura utiliza estos archivos para crear `acceleration_matrix.{json,md}` para paneles de flota. |
| Exportación Grafana | Demuestra la adopción de telemetría y anotaciones de alerta para la ventana de implementación.| Exporte el panel `fastpq-acceleration`:`curl -s -H "Authorization: Bearer $GRAFANA_TOKEN" \``  "$GRAFANA_URL/api/dashboards/uid/fastpq-acceleration" \``  | jq '.dashboard' \``  > artifacts/fastpq_rollouts/<stamp>/grafana_fastpq_acceleration.json`Anote el tablero con las horas de inicio/detención del lanzamiento antes de exportar. La canalización de lanzamiento puede hacer esto automáticamente a través de `scripts/run_release_pipeline.py --export-fastpq-grafana --grafana-url <URL>` (token suministrado a través de `GRAFANA_TOKEN`). |
| Instantánea de alerta | Captura las reglas de alerta que protegieron el lanzamiento.| Copie `dashboards/alerts/fastpq_acceleration_rules.yml` (y el dispositivo `tests/`) en el paquete para que los revisores puedan volver a ejecutar `promtool test rules …`. |
| Registro de perforación de retroceso | Demuestra que los operadores ensayaron el respaldo forzado de la CPU y los acuses de recibo de telemetría.| Utilice el procedimiento en [Rollback Drills](#rollback-drills) y almacene los registros de la consola (`rollback_drill.log`) más el raspado Prometheus resultante (`metrics_rollback.prom`). || `row_usage/fastpq_row_usage_<date>.json` | Registra la asignación de filas de ExecWitness FASTPQ que TF-5 rastrea en CI y paneles.| Descargue un testigo nuevo de Torii, decodifíquelo a través de `iroha_cli audit witness --decode exec.witness` (opcionalmente agregue `--fastpq-parameter fastpq-lane-balanced` para afirmar el conjunto de parámetros esperado; los lotes FASTPQ se emiten de forma predeterminada) y copie el JSON `row_usage` en `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/row_usage/`. Mantenga los nombres de los archivos con marca de tiempo para que los revisores puedan correlacionarlos con el ticket de implementación y ejecute `python3 scripts/fastpq/validate_row_usage_snapshot.py row_usage/*.json` (o `make check-fastpq-rollout`) para que la puerta Stage7-3 verifique que cada lote anuncie los recuentos del selector y la invariante `transfer_ratio = transfer_rows / total_rows` antes de adjuntar la evidencia. |

> **Consejo:** `artifacts/fastpq_rollouts/README.md` documenta el nombre preferido
> esquema (`<stamp>/<fleet>/<lane>`) y los archivos de evidencia requeridos. el
> La carpeta `<stamp>` debe codificar `YYYYMMDDThhmmZ` para que los artefactos se puedan ordenar
> sin consultar tickets.

## Lista de verificación para la generación de evidencia1. **Capture puntos de referencia de GPU.**
   - Ejecute la carga de trabajo canónica (20000 filas lógicas, 32768 filas rellenas) mediante
     `cargo run -p fastpq_prover --bin fastpq_metal_bench -- --rows 20000 --pretty`.
   - Envuelva el resultado con `scripts/fastpq/wrap_benchmark.py` usando `--row-usage <decoded witness>` para que el paquete incluya la evidencia del dispositivo junto con la telemetría de la GPU. Pase `--require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --sign-output` para que el contenedor falle rápidamente si cualquiera de los aceleradores excede el objetivo o si falta la telemetría de cola/perfil de Poseidon, y para generar la firma separada.
   - Repita en el host CUDA para que el manifiesto contenga ambas familias de GPU.
   - **No** pele el `benchmarks.metal_dispatch_queue` o
     Bloques `benchmarks.zero_fill_hotspots` del JSON empaquetado. La puerta CI
     (`ci/check_fastpq_rollout.sh`) ahora lee esos campos y falla cuando se pone en cola
     el espacio libre cae por debajo de una ranura o cuando cualquier punto de acceso LDE informa `mean_ms >
     0,40 ms`, aplicando automáticamente la protección de telemetría Stage7.
2. **Genere el manifiesto.** Utilice `cargo xtask fastpq-bench-manifest …` como
   mostrado en la tabla. Guarde `fastpq_bench_manifest.json` en el paquete de implementación.
3. **Exportar Grafana.**
   - Anotar el tablero `FASTPQ Acceleration Overview` con la ventana desplegable,
     vinculando a los ID de panel Grafana relevantes.
   - Exporte el JSON del panel a través de la API Grafana (comando arriba) e incluya
     la sección `annotations` para que los revisores puedan hacer coincidir las curvas de adopción con las
     lanzamiento por etapas.
4. **Alertas de instantáneas.** Copie las reglas de alerta exactas (`dashboards/alerts/…`) utilizadas
   por el despliegue en el paquete. Si se anularon las reglas Prometheus, incluya
   la diferencia de anulación.
5. **Prometheus/OTEL scrape.** Capture `fastpq_execution_mode_total{device_class="<matrix>"}` de cada
   anfitrión (antes y después del escenario) más el mostrador del OTEL
   `fastpq.execution_mode_resolutions_total` y el emparejado
   Entradas de registro `telemetry::fastpq.execution_mode`. Estos artefactos prueban que
   La adopción de GPU es estable y las reservas forzadas de CPU aún emiten telemetría.
6. **Archivar telemetría de uso de filas.** Después de decodificar la ejecución de ExecWitness para el
   implementación, coloque el JSON resultante en `row_usage/` en el paquete. El CI
   El ayudante (`ci/check_fastpq_row_usage.sh`) compara estas instantáneas con las
   líneas de base canónicas, y `ci/check_fastpq_rollout.sh` ahora requiere cada
   paquete para enviar al menos un archivo `row_usage` para mantener adjunta la evidencia TF-5
   al boleto de liberación.

## Flujo de implementación por etapas

Utilice tres fases deterministas para cada flota. Avanzar sólo después de la salida.
Los criterios de cada fase se satisfacen y documentan en el paquete de evidencia.| Fase | Alcance | Criterios de salida | Adjuntos |
|-------|-------|-----------------------|-------------|
| Piloto (P1) | 1 plano de control + 1 nodo de plano de datos por región | `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` ≥90 % durante 48 h, cero incidentes de Alertmanager y un simulacro de reversión de paso. | Paquete de ambos hosts (JSON de banco, exportación Grafana con anotación piloto, registros de reversión). |
| Rampa (P2) | ≥50% de validadores más al menos una vía de archivo por grupo | La ejecución de GPU se mantuvo durante 5 días, no más de 1 pico de degradación >10 minutos y los contadores Prometheus demuestran alertas de retroceso dentro de los 60 segundos. | Exportación Grafana actualizada que muestra la anotación de rampa, diferencias de raspado Prometheus, captura de pantalla/registro de Alertmanager. |
| Predeterminado (P3) | Nodos restantes; FASTPQ marcado como predeterminado en `iroha_config` | Manifiesto de banco firmado + exportación Grafana que hace referencia a la curva de adopción final y un ejercicio de reversión documentado que demuestra el cambio de configuración. | Manifiesto final, Grafana JSON, registro de reversión, referencia del ticket para revisión de cambios de configuración. |

Documente cada paso de la promoción en el ticket de lanzamiento y vincúlelo directamente al
Anotaciones `grafana_fastpq_acceleration.json` para que los revisores puedan correlacionar las
línea de tiempo con la evidencia.

## Ejercicios de retroceso

Cada etapa de implementación debe incluir un ensayo de reversión:

1. Elija un nodo por clúster y registre las métricas actuales:
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
2. Fuerce el modo CPU durante 10 minutos usando la perilla de configuración
   (`zk.fastpq.execution_mode = "cpu"`) o la anulación del entorno:
   ```bash
   FASTPQ_GPU=cpu irohad --config <path> --genesis-manifest-json <path>
   ```
3. Confirme el registro de degradación
   (`telemetry::fastpq.execution_mode resolved="cpu" requested="gpu"`) y raspar
   el punto final Prometheus nuevamente para mostrar los incrementos del contador.
4. Restaure el modo GPU, verifique que `telemetry::fastpq.execution_mode` ahora informe
   `resolved="metal"` (o `resolved="cuda"/"opencl"` para carriles que no sean metálicos),
   Confirme que el raspado Prometheus contiene muestras de CPU y GPU en
   `fastpq_execution_mode_total{backend=…}` y registre el tiempo transcurrido en
   detección/limpieza.
5. Almacene transcripciones de shell, métricas y reconocimientos de operadores como
   `rollback_drill.log` e `metrics_rollback.prom` en el paquete de implementación. Estos
   Los archivos deben ilustrar el ciclo completo de degradación + restauración porque
   `ci/check_fastpq_rollout.sh` ahora falla cuando el registro carece de GPU
   La línea de recuperación o la instantánea de métricas omiten los contadores de CPU o GPU.

Estos registros demuestran que cada clúster puede degradarse sin problemas y que los equipos de SRE
Sepa cómo retroceder de manera determinista si los controladores o núcleos de GPU retroceden.

## Evidencia alternativa en modo mixto (WP2-E.6)

Siempre que un host necesite GPU FFT/LDE pero CPU Poseidon hash (según Stage7 <900 ms
requisito), agrupa los siguientes artefactos junto con los registros de reversión estándar:1. **Config diff.** Registre (o adjunte) la anulación local del host que establece
   `zk.fastpq.poseidon_mode = "cpu"` (`FASTPQ_POSEIDON_MODE=cpu`) al salir
   `zk.fastpq.execution_mode` intacto. Nombra el parche
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/poseidon_fallback.patch`.
2. **Raspado del contador de Poseidón.**
   ```bash
   curl -s http://<host>:8180/metrics \
     | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"' \
     > artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom
   ```
   La captura debe mostrar `path="cpu_forced"` incrementando al mismo tiempo que el
   Contador GPU FFT/LDE para esa clase de dispositivo. Haz un segundo raspado después de revertir
   Regrese al modo GPU para que los revisores puedan ver el resumen de la fila `path="gpu"`.

   Pase el archivo resultante a `wrap_benchmark.py --poseidon-metrics …` para que el punto de referencia empaquetado registre los mismos contadores dentro de su sección `poseidon_metrics`; esto mantiene las implementaciones de Metal y CUDA en el mismo flujo de trabajo y hace que la evidencia alternativa sea auditable sin abrir archivos scrape separados.
3. **Extracto del registro.** Copie las entradas `telemetry::fastpq.poseidon` que prueban la
   solucionador volteado a CPU (`cpu_forced`) en
   `poseidon_fallback.log`, manteniendo marcas de tiempo para que las líneas de tiempo de Alertmanager puedan ser
   correlacionado con el cambio de configuración.

CI aplica hoy las comprobaciones de cola/llenado cero; una vez que aterriza la puerta de modo mixto,
`ci/check_fastpq_rollout.sh` también insistirá en que cualquier paquete que contenga
`poseidon_fallback.patch` envía la instantánea `metrics_poseidon.prom` correspondiente.
Seguir este flujo de trabajo mantiene la política alternativa de WP2-E.6 auditable y vinculada a
los mismos recolectores de evidencia utilizados durante la implementación predeterminada.

## Informes y automatización

- Adjunte todo el directorio `artifacts/fastpq_rollouts/<stamp>/` al
  libere el ticket y haga referencia a él desde `status.md` una vez que se cierre el lanzamiento.
- Ejecute `dashboards/alerts/tests/fastpq_acceleration_rules.test.yml` (a través de
  `promtool`) dentro de CI para garantizar que los paquetes de alertas incluidos con la implementación aún
  compilar.
- Validar el paquete con `ci/check_fastpq_rollout.sh` (o
  `make check-fastpq-rollout`) y pasa `FASTPQ_ROLLOUT_BUNDLE=<path>` cuando
  desea apuntar a un solo lanzamiento. CI invoca el mismo script a través de
  `.github/workflows/fastpq-rollout.yml`, por lo que los artefactos faltantes fallan rápidamente antes de
  el ticket de liberación puede cerrarse. La canalización de lanzamiento puede archivar paquetes validados.
  junto a los manifiestos firmados pasando
  `--fastpq-rollout-bundle artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>` a
  `scripts/run_release_pipeline.py`; el ayudante se repite
  `ci/check_fastpq_rollout.sh` (a menos que esté configurado `--skip-fastpq-rollout-check`) y
  copia el árbol de directorios en `artifacts/releases/<version>/fastpq_rollouts/…`.
  Como parte de esta puerta, el script impone la profundidad de la cola de Stage7 y el llenado cero.
  presupuestos leyendo `benchmarks.metal_dispatch_queue` y
  `benchmarks.zero_fill_hotspots` de cada banco JSON `metal`.

Siguiendo este manual podemos demostrar una adopción determinista, proporcionar una
paquete de evidencia único por implementación y mantener simulacros de reversión auditados junto con
los manifiestos de referencia firmados.