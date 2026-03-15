---
lang: es
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: canalización-de-métricas-de-privacidad
título: Pipeline de métricas de privacidad de SoraNet (SNNet-8)
sidebar_label: Tubería de métricas de privacidad
descripción: Telemetria con preservacion de privacidad para retransmisiones y orquestadores de SoraNet.
---

:::nota Fuente canónica
Refleja `docs/source/soranet/privacy_metrics_pipeline.md`. Mantengan ambas copias sincronizadas hasta que los documentos heredados se retiren.
:::

# Pipeline de métricas de privacidad de SoraNet

SNNet-8 introduce una superficie de telemetría consciente de la privacidad para
el tiempo de ejecución del relé. Ahora el relevo agrega eventos de handshake y circuito en
buckets de un minuto y exporta solo contadores Prometheus horribles, manteniendo
los circuitos individuales desvinculados mientras ofrece visibilidad accionable
para los operadores.

## Resumen del agregador- La implementación del runtime vive en `tools/soranet-relay/src/privacy.rs` como
  `PrivacyAggregator`.
- Los cubos se indexan por minuto de reloj (`bucket_secs`, predeterminado 60 segundos) y
  se almacenan en un anillo acotado (`max_completed_buckets`, predeterminado 120). las acciones
  de coleccionistas mantiene su propio backlog acotado (`max_share_lag_buckets`, default 12)
  para que las ventanas Prio stale se vacien como cubos suprimidos en lugar de
  Filtrar memoria o enmascarar coleccionistas atascados.
- `RelayConfig::privacy` mapea directo a `PrivacyConfig`, exponiendo perillas de ajuste
  (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`,
  `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`). El tiempo de ejecución
  de producción mantiene los valores predeterminados mientras SNNet-8a introduce umbrales de
  agregación segura.
- Los módulos de tiempo de ejecución registran eventos con ayudantes tipados:
  `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`,
  `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`,
  `record_verified_bytes`, y `record_gar_category`.

## Administrador del punto final del reléLos operadores pueden consultar el oyente admin del relevo para observaciones
crudas vía `GET /privacy/events`. El endpoint devuelve JSON delimitado por
nuevas lineas (`application/x-ndjson`) con payloads `SoranetPrivacyEventV1`
reflejados desde el `PrivacyEventBuffer` interno. El buffer retiene los eventos
mas nuevos hasta `privacy.event_buffer_capacity` entradas (predeterminado 4096) y se
vacío en la lectura, por lo que los scrapers deben sondear lo suficiente para
no dejar huecos. Los eventos cubren las mismas señales de handshake, throttle,
ancho de banda verificado, circuito activo y GAR que alimentan los contadores Prometheus,
permitiendo a los recopiladores posteriores archivar migas de pan seguros para la privacidad
o alimentar flujos de trabajo de agregacion segura.

## Configuración del relé

Los operadores ajustan la cadencia de telemetría de privacidad en el archivo de
configuración del relé vía la sección `privacy`:

```json
{
  "mode": "Entry",
  "listen": "0.0.0.0:443",
  "privacy": {
    "bucket_secs": 60,
    "min_handshakes": 12,
    "flush_delay_buckets": 1,
    "force_flush_buckets": 6,
    "max_completed_buckets": 120,
    "max_share_lag_buckets": 12,
    "expected_shares": 2
  }
}
```

Los defaults de campo coinciden con la especificación SNNet-8 y se validan al
cargar:| Campo | Descripción | Predeterminado |
|-------|-------------|---------|
| `bucket_secs` | Ancho de cada ventana de agregación (segundos). | `60` |
| `min_handshakes` | Mínimo de contribuyentes antes de emitir un cubo. | `12` |
| `flush_delay_buckets` | Buckets completados a esperar antes del enjuague. | `1` |
| `force_flush_buckets` | Edad máxima antes de emitir un cubo suprimido. | `6` |
| `max_completed_buckets` | Backlog de cubetas retenidos (evita memoria ilimitada). | `120` |
| `max_share_lag_buckets` | Ventana de retención para acciones de coleccionistas antes de suprimir. | `12` |
| `expected_shares` | Acciones Prio requeridos antes de combinar. | `2` |
| `event_buffer_capacity` | Backlog NDJSON de eventos para el admin stream. | `4096` |

Configurar `force_flush_buckets` por debajo de `flush_delay_buckets`, poner
los umbrales en cero o deshabilitar el guardia de retención ahora falla la
validación para evitar despliegues que filtran telemetría por relé.

El limite `event_buffer_capacity` tambien acota `/admin/privacy/events`,
asegurando que los scrapers no queden atras indefinidamente.

## Acciones del coleccionista PrioSNNet-8a despliega colectores duales que emiten cubos Prio con intercambio secreto.
Ahora el orquestador parsea el stream NDJSON `/privacy/events` tanto para
entradas `SoranetPrivacyEventV1` como para acciones `SoranetPrivacyPrioShareV1`,
reenviandolos a `SoranetSecureAggregator::ingest_prio_share`. Los cubos se
emiten cuando llegan `PrivacyBucketConfig::expected_shares` contribuciones,
reflejando el comportamiento del agregador en el relé. Las acciones se validan
por alineacion de bucket y forma del histograma antes de combinarse en
`SoranetPrivacyBucketMetricsV1`. Si el conteo combinado de apretones de manos cae por
Debajo de `min_contributors`, el cubo se exporta como `suppressed`, reflejando
el comportamiento del agregador en el relé. Las ventanas suprimidas ahora
emiten una etiqueta `suppression_reason` para que los operadores distingan entre
`insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed`
y `forced_flush_window_elapsed` al diagnosticar huecos de telemetria. La razón
`collector_window_elapsed` tambien dispara cuando las acciones Prio se quedan mas
alla de `max_share_lag_buckets`, haciendo visibles los coleccionistas atascados sin
mantener acumuladores obsoletos en memoria.

## Puntos finales de ingesta Torii

Torii ahora exponen dos endpoints HTTP con telemetría gateada para que retransmisiones y
coleccionistas reenvien observaciones sin incrustar un transporte a medida:- `POST /v1/soranet/privacy/event` acepta una carga útil
  `RecordSoranetPrivacyEventDto`. El cuerpo envuelve un `SoranetPrivacyEventV1`
  mas una etiqueta opcional `source`. Torii valida la solicitud contra el
  perfil de telemetria activo, registra el evento y responde con HTTP
  `202 Accepted` junto con un sobre Norito JSON que contiene la ventana
  computada del cubo (`bucket_start_unix`, `bucket_duration_secs`) y el
  modo del relevo.
- `POST /v1/soranet/privacy/share` acepta una carga útil `RecordSoranetPrivacyShareDto`.
  El cuerpo lleva un `SoranetPrivacyPrioShareV1` y una pista opcional `forwarded_by`
  para que los operadores auditen flujos de recolectores. Las entregas exitosas
  devuelven HTTP `202 Accepted` con un sobre Norito JSON que resume el
  coleccionista, la ventana del cubo y el supresión indirecta; las fallas de
  validacion mapean a una respuesta de telemetria `Conversion` para preservar
  el manejo de errores determinista en coleccionistas. El event loop del orquestador
  ahora emite estas acciones mientras sondea retransmisiones, manteniendo el acumulador
  Prio de Torii sincronizado con los cubos en relé.

Ambos endpoints respetan el perfil de telemetría: emiten `503 Service
No disponible` cuando las métricas están deshabilitadas. Los clientes pueden enviar
cuerpos Norito binario (`application/x.norito`) o Norito JSON (`application/x.norito+json`);
el servidor negocia el formato automáticamente a través de los extractores estándar de
Torii.## Métricas Prometheus

Cada cubeta exportado lleva etiquetas `mode` (`entry`, `middle`, `exit`) y
`bucket_start`. Se emiten las siguientes familias de métricas:| Métrica | Descripción |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | Taxonomía de apretón de manos con `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | Contadores de acelerador con `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Duraciones agregadas de tiempo de reutilización aportadas por apretones de manos acelerados. |
| `soranet_privacy_verified_bytes_total` | Ancho de banda verificado de pruebas de medicamento cegada. |
| `soranet_privacy_active_circuits_{avg,max}` | Media y pico de circuitos activos por cubo. |
| `soranet_privacy_rtt_millis{percentile}` | Estimaciones de percentil de RTT (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Contadores GAR hasheados por resumen de categoría. |
| `soranet_privacy_bucket_suppressed` | Buckets retenidos porque no se cumplio el umbral de contribuyentes. |
| `soranet_privacy_pending_collectors{mode}` | Acumuladores de acciones pendientes de combinar, agrupados por modo de relevo. |
| `soranet_privacy_suppression_total{reason}` | Contadores de cubos suprimidos con `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` para atribuir lagunas de privacidad. |
| `soranet_privacy_snapshot_suppression_ratio` | Ratio suprimido/descargado del último drenaje (0-1), util para presupuestos de alerta. |
| `soranet_privacy_last_poll_unixtime` | Timestamp UNIX del ultimo poll exitoso (alimenta la alerta coleccionista-idle). |
| `soranet_privacy_collector_enabled` | Gauge que cae a `0` cuando el Privacy Collector está deshabilitado o falla al iniciar (alimenta la alerta Collector-Disabled). |
| `soranet_privacy_poll_errors_total{provider}` | Fallas de polling agrupadas por alias de retransmisión (incrementa por errores de decodificación, fallos HTTP o códigos inesperados). |Los cubos sin observaciones permanecen silenciosos, manteniendo tableros de instrumentos
limpios sin fabricar ventanas con ceros.

## Guía operativa1. **Dashboards** - gráfica de las métricas anteriores agrupadas por `mode` y
   `window_start`. Destaca ventanas faltantes para revelar problemas en coleccionistas.
   o relés. Usa `soranet_privacy_suppression_total{reason}` para distinguir
   carencias de contribuyentes de supresión por coleccionista al triaje. El activo de
   Grafana ahora incluye un panel dedicado **"Razones de supresión (5m)"**
   alimentado por esos contadores mas una estadística **"Suppressed Bucket %"** que
   calcular `sum(soranet_privacy_bucket_suppressed) / count(...)` por selección
   para que los operadores vean brechas de presupuesto de un vistazo. la serie
   **Recolector de acciones pendientes** (`soranet_privacy_pending_collectors`) y la estadística
   **Snapshot Suppression Ratio** resaltan coleccionistas atascados y deriva de presupuesto
   durante ejecuciones automatizadas.
2. **Alertas** - dispara alarmas desde contadores privacidad-segura: picos de rechazo
   PoW, frecuencia de enfriamiento, deriva de RTT y rechazos de capacidad. Como los
   contadores son monotonic dentro de cada cubo, reglas basadas en tasa
   funcionan bien.
3. **Respuesta al incidente** - confia primero en datos agregados. Cuando se requiera
   depuración profunda, solicita a relés reproducir instantáneas de cubos o
   pruebas de medicamento cegada en lugar de inspeccionar registros de tráfico
   crudos.
4. **Retención** - scrapea con suficiente frecuencia para no exceder
   `max_completed_buckets`. Los exportadores deben tratar la salida Prometheus comola fuente canónica y descartar buckets locales una vez reenviados.

## Analitica de supresion y ejecuciones automatizadas

La aceptación SNNet-8 depende de demostrar que los recolectores automatizados se
mantengan sanos y que la supresión permanezca dentro de los límites de política
(<=10% de cubos por relevo en cualquier ventana de 30 minutos). el herramientas
necesario ya se incluye en el repo; los operadores deben integrarlo en sus
rituales semanales. Los nuevos paneles de supresión en Grafana reflejan los
snippets PromQL abajo, dando visibilidad en vivo a los equipos de guardia antes de
Recurrir a consultas manuales.

### Recetas PromQL para revisar la supresión

Los operadores deben tener a mano los siguientes ayudantes PromQL; ambos se
referencia en el tablero compartido (`dashboards/grafana/soranet_privacy_metrics.json`)
y en las reglas de Alertmanager:

```promql
/* Suppression ratio per relay mode (30 minute window) */
(
  increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_suppressed|collector_window_elapsed|forced_flush_window_elapsed"}[30m])
) /
clamp_min(
  increase(soranet_privacy_circuit_events_total{kind="accepted"}[30m]) +
  increase(soranet_privacy_suppression_total[30m]),
1
)
```

```promql
/* Detect new suppression spikes above the permitted minute budget */
increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_window_elapsed|collector_suppressed"}[5m])
/
clamp_min(
  sum(increase(soranet_privacy_circuit_events_total{kind="accepted"}[5m])),
1
)
```

Usa el ratio para confirmar que la estadística **"Suppressed Bucket %"** se mantiene
por debajo del presupuesto de póliza; conecta el detector de picos a
Alertmanager para feedback rapido cuando el conteo de contribuyentes baja de
forma inesperada.

### CLI de informes fuera de línea de depósitos

El espacio de trabajo exponen `cargo xtask soranet-privacy-report` para capturas NDJSON
puntuales. Apuntalo a uno o mas exports admin del relevo:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```El ayudante procesa la captura con `SoranetSecureAggregator`, imprime un resumen de
supresión en stdout y opcionalmente escribe un informe JSON estructurado vía
`--json-out <path|->`. Respeta los mismos mandos que el coleccionista en vivo.
(`--bucket-secs`, `--min-contributors`, `--expected-shares`, etc.), permitiendo
reproducir capturas históricas bajo distintos umbrales al investigar un problema.
Adjunta el JSON junto con capturas de pantalla de Grafana para que el gate de analitica
SNNet-8 sigue siendo auditable.

### Checklist de primera ejecucion automatizada

Governance aun exige probar que la primera ejecucion automatizada cumplio el
presupuesto de supresión. El ayudante ahora acepta `--max-suppression-ratio <0-1>`
para que CI u operadores falló rapido cuando los cubos suprimidos exceden la
ventana permitida (predeterminado 10%) o cuando aún no hay cubos presentes. Flujo
recomendado:

1. Exporta NDJSON desde el endpoint admin del relevo y el stream
   `/v1/soranet/privacy/event|share` del orquestador hacia
   `artifacts/sorafs_privacy/<relay>.ndjson`.
2. Ejecuta el helper con el presupuesto de póliza:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```El comando imprime el ratio observado y sale con código distinto de cero
   cuando se excede el presupuesto **o** cuando no hay cubos listos, indicando
   que la telemetria aun no se ha producido para la ejecucion. Las métricas en
   vivo deben mostrar `soranet_privacy_pending_collectors` renando hacia cero y
   `soranet_privacy_snapshot_suppression_ratio` quedandose bajo el mismo
   presupuesto mientras se ejecuta la corrida.
3. Archiva el JSON de salida y el log del CLI con el paquete de evidencia SNNet-8
   antes de cambiar el transporte predeterminado para que los revisores puedan
   reproducir los artefactos exactos.

## Próximos pasos (SNNet-8a)- Integrar los coleccionistas Prio duales, conectando la ingesta de acciones al tiempo de ejecución
  para que relés y colectores emitan cargas útiles `SoranetPrivacyBucketMetricsV1`
  consistentes. *(Listo - ver `ingest_privacy_payload` es
  `crates/sorafs_orchestrator/src/lib.rs` y tests asociados.)*
- Publicar el tablero Prometheus compartido y reglas de alerta que cubran
  gaps de supresion, salud de coleccionista y apagones de anonimato. *(Hecho - ver
  `dashboards/grafana/soranet_privacy_metrics.json`,
  `dashboards/alerts/soranet_privacy_rules.yml`,
  `dashboards/alerts/soranet_policy_rules.yml`, y accesorios de validación.)*
- Producir los artefactos de calibración de privacidad diferencial descritos en
  `privacy_metrics_dp.md`, incluyendo cuadernos reproducibles y resúmenes de
  gobernanza. *(Listo - notebook + artefactos generados por
  `scripts/telemetry/run_privacy_dp.py`; contenedor de CI
  `scripts/telemetry/run_privacy_dp_notebook.sh` ejecuta el cuaderno a través del
  flujo de trabajo `.github/workflows/release-pipeline.yml`; resumen de gobernanza archivado en
  `docs/source/status/soranet_privacy_dp_digest.md`.)*

La versión actual entrega la base SNNet-8: telemetría determinista y segura
para la privacidad que se integra directo con scrapers y tableros Prometheus
existentes. Los artefactos de calibracion de privacidad diferencial estan listos,
el flujo de trabajo de lanzamiento mantiene frescas las salidas del cuaderno, y el trabajo
restante se enfoca en monitorear la primera ejecucion automatizada y extensor
la analítica de alertas de supresión.