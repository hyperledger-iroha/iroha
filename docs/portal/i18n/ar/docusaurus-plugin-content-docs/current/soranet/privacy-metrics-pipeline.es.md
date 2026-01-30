---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/privacy-metrics-pipeline.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: privacy-metrics-pipeline
title: Pipeline de metricas de privacidad de SoraNet (SNNet-8)
sidebar_label: Pipeline de metricas de privacidad
description: Telemetria con preservacion de privacidad para relays y orchestrators de SoraNet.
---

:::note Fuente canonica
Refleja `docs/source/soranet/privacy_metrics_pipeline.md`. Mantengan ambas copias sincronizadas hasta que los docs heredados se retiren.
:::

# Pipeline de metricas de privacidad de SoraNet

SNNet-8 introduce una superficie de telemetria consciente de la privacidad para
el runtime del relay. Ahora el relay agrega eventos de handshake y circuit en
buckets de un minuto y exporta solo contadores Prometheus gruesos, manteniendo
los circuits individuales desvinculados mientras ofrece visibilidad accionable
para los operadores.

## Resumen del agregador

- La implementacion del runtime vive en `tools/soranet-relay/src/privacy.rs` como
  `PrivacyAggregator`.
- Los buckets se indexan por minuto de reloj (`bucket_secs`, default 60 segundos) y
  se almacenan en un ring acotado (`max_completed_buckets`, default 120). Los shares
  de collectors mantienen su propio backlog acotado (`max_share_lag_buckets`, default 12)
  para que las ventanas Prio stale se vacien como buckets suprimidos en lugar de
  filtrar memoria o enmascarar collectors atascados.
- `RelayConfig::privacy` mapea directo a `PrivacyConfig`, exponiendo knobs de ajuste
  (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`,
  `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`). El runtime
  de produccion mantiene los defaults mientras SNNet-8a introduce thresholds de
  agregacion segura.
- Los modulos de runtime registran eventos con helpers tipados:
  `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`,
  `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`,
  `record_verified_bytes`, y `record_gar_category`.

## Endpoint admin del relay

Los operadores pueden consultar el listener admin del relay para observaciones
crudas via `GET /privacy/events`. El endpoint devuelve JSON delimitado por
nuevas lineas (`application/x-ndjson`) con payloads `SoranetPrivacyEventV1`
reflejados desde el `PrivacyEventBuffer` interno. El buffer retiene los eventos
mas nuevos hasta `privacy.event_buffer_capacity` entradas (default 4096) y se
vacía en la lectura, por lo que los scrapers deben sondear lo suficiente para
no dejar huecos. Los eventos cubren las mismas senales de handshake, throttle,
verified bandwidth, active circuit y GAR que alimentan los contadores Prometheus,
permitiendo a collectors downstream archivar breadcrumbs seguros para la privacidad
o alimentar workflows de agregacion segura.

## Configuracion del relay

Los operadores ajustan la cadencia de telemetria de privacidad en el archivo de
configuracion del relay via la seccion `privacy`:

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

Los defaults de campo coinciden con la especificacion SNNet-8 y se validan al
cargar:

| Campo | Descripcion | Default |
|-------|-------------|---------|
| `bucket_secs` | Ancho de cada ventana de agregacion (segundos). | `60` |
| `min_handshakes` | Minimo de contribuyentes antes de emitir un bucket. | `12` |
| `flush_delay_buckets` | Buckets completados a esperar antes del flush. | `1` |
| `force_flush_buckets` | Edad maxima antes de emitir un bucket suprimido. | `6` |
| `max_completed_buckets` | Backlog de buckets retenidos (evita memoria ilimitada). | `120` |
| `max_share_lag_buckets` | Ventana de retencion para shares de collectors antes de suprimir. | `12` |
| `expected_shares` | Shares Prio requeridos antes de combinar. | `2` |
| `event_buffer_capacity` | Backlog NDJSON de eventos para el admin stream. | `4096` |

Configurar `force_flush_buckets` por debajo de `flush_delay_buckets`, poner
los thresholds en cero o deshabilitar el guard de retencion ahora falla la
validacion para evitar despliegues que filtren telemetria por relay.

El limite `event_buffer_capacity` tambien acota `/admin/privacy/events`,
asegurando que los scrapers no queden atras indefinidamente.

## Shares del collector Prio

SNNet-8a despliega collectors duales que emiten buckets Prio con secret sharing.
Ahora el orchestrator parsea el stream NDJSON `/privacy/events` tanto para
entradas `SoranetPrivacyEventV1` como para shares `SoranetPrivacyPrioShareV1`,
reenviandolos a `SoranetSecureAggregator::ingest_prio_share`. Los buckets se
emiten cuando llegan `PrivacyBucketConfig::expected_shares` contribuciones,
reflejando el comportamiento del agregador en el relay. Los shares se validan
por alineacion de bucket y forma del histograma antes de combinarse en
`SoranetPrivacyBucketMetricsV1`. Si el conteo combinado de handshakes cae por
debajo de `min_contributors`, el bucket se exporta como `suppressed`, reflejando
el comportamiento del agregador en el relay. Las ventanas suprimidas ahora
emiten una etiqueta `suppression_reason` para que los operadores distingan entre
`insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed`
y `forced_flush_window_elapsed` al diagnosticar huecos de telemetria. La razon
`collector_window_elapsed` tambien dispara cuando las shares Prio se quedan mas
alla de `max_share_lag_buckets`, haciendo visibles los collectors atascados sin
mantener acumuladores stale en memoria.

## Endpoints de ingesta Torii

Torii ahora expone dos endpoints HTTP con telemetria gateada para que relays y
collectors reenvien observaciones sin incrustar un transporte bespoke:

- `POST /v1/soranet/privacy/event` acepta un payload
  `RecordSoranetPrivacyEventDto`. El body envuelve un `SoranetPrivacyEventV1`
  mas una etiqueta opcional `source`. Torii valida la solicitud contra el
  perfil de telemetria activo, registra el evento y responde con HTTP
  `202 Accepted` junto con un envelope Norito JSON que contiene la ventana
  computada del bucket (`bucket_start_unix`, `bucket_duration_secs`) y el
  modo del relay.
- `POST /v1/soranet/privacy/share` acepta un payload `RecordSoranetPrivacyShareDto`.
  El body lleva un `SoranetPrivacyPrioShareV1` y un hint opcional `forwarded_by`
  para que los operadores auditen flujos de collectors. Las entregas exitosas
  devuelven HTTP `202 Accepted` con un envelope Norito JSON que resume el
  collector, la ventana del bucket y el suppression hint; las fallas de
  validacion mapean a una respuesta de telemetria `Conversion` para preservar
  el manejo de errores determinista en collectors. El event loop del orchestrator
  ahora emite estas shares mientras sondea relays, manteniendo el acumulador
  Prio de Torii sincronizado con los buckets on-relay.

Ambos endpoints respetan el perfil de telemetria: emiten `503 Service
Unavailable` cuando las metricas estan deshabilitadas. Los clients pueden enviar
cuerpos Norito binary (`application/x.norito`) o Norito JSON (`application/x.norito+json`);
el servidor negocia el formato automaticamente via los extractores standard de
Torii.

## Metricas Prometheus

Cada bucket exportado lleva etiquetas `mode` (`entry`, `middle`, `exit`) y
`bucket_start`. Se emiten las siguientes familias de metricas:

| Metrica | Descripcion |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | Taxonomia de handshake con `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | Contadores de throttle con `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Duraciones agregadas de cooldown aportadas por handshakes throttled. |
| `soranet_privacy_verified_bytes_total` | Ancho de banda verificado de proofs de medicion cegada. |
| `soranet_privacy_active_circuits_{avg,max}` | Media y pico de circuits activos por bucket. |
| `soranet_privacy_rtt_millis{percentile}` | Estimaciones de percentil de RTT (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Contadores GAR hasheados por digest de categoria. |
| `soranet_privacy_bucket_suppressed` | Buckets retenidos porque no se cumplio el umbral de contribuyentes. |
| `soranet_privacy_pending_collectors{mode}` | Acumuladores de shares pendientes de combinar, agrupados por modo de relay. |
| `soranet_privacy_suppression_total{reason}` | Contadores de buckets suprimidos con `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` para atribuir gaps de privacidad. |
| `soranet_privacy_snapshot_suppression_ratio` | Ratio suprimido/descargado del ultimo drain (0-1), util para presupuestos de alerta. |
| `soranet_privacy_last_poll_unixtime` | Timestamp UNIX del ultimo poll exitoso (alimenta la alerta collector-idle). |
| `soranet_privacy_collector_enabled` | Gauge que cae a `0` cuando el privacy collector esta deshabilitado o falla al iniciar (alimenta la alerta collector-disabled). |
| `soranet_privacy_poll_errors_total{provider}` | Fallas de polling agrupadas por alias de relay (incrementa por decode errors, HTTP failures o codigos inesperados). |

Los buckets sin observaciones permanecen silenciosos, manteniendo dashboards
limpios sin fabricar ventanas con ceros.

## Guia operativa

1. **Dashboards** - grafica las metricas anteriores agrupadas por `mode` y
   `window_start`. Destaca ventanas faltantes para revelar problemas en collectors
   o relays. Usa `soranet_privacy_suppression_total{reason}` para distinguir
   carencias de contribuyentes de supresion por collector al triage. El asset de
   Grafana ahora incluye un panel dedicado **"Suppression Reasons (5m)"**
   alimentado por esos contadores mas un stat **"Suppressed Bucket %"** que
   calcula `sum(soranet_privacy_bucket_suppressed) / count(...)` por seleccion
   para que los operadores vean brechas de presupuesto de un vistazo. La serie
   **Collector Share Backlog** (`soranet_privacy_pending_collectors`) y el stat
   **Snapshot Suppression Ratio** resaltan collectors atascados y drift de budget
   durante ejecuciones automatizadas.
2. **Alerting** - dispara alarmas desde contadores privacy-safe: picos de rechazo
   PoW, frecuencia de cooldown, drift de RTT y capacity rejects. Como los
   contadores son monotonic dentro de cada bucket, reglas basadas en tasa
   funcionan bien.
3. **Incident response** - confia primero en datos agregados. Cuando se requiera
   depuracion profunda, solicita a relays reproducir snapshots de buckets o
   inspeccionar proofs de medicion cegada en lugar de recolectar logs de trafico
   crudos.
4. **Retention** - scrapea con suficiente frecuencia para no exceder
   `max_completed_buckets`. Los exporters deben tratar la salida Prometheus como
   la fuente canonica y descartar buckets locales una vez reenviados.

## Analitica de supresion y ejecuciones automatizadas

La aceptacion SNNet-8 depende de demostrar que los collectors automatizados se
mantienen sanos y que la supresion permanece dentro de los limites de policy
(<=10% de buckets por relay en cualquier ventana de 30 minutos). El tooling
necesario ya se incluye en el repo; los operadores deben integrarlo en sus
rituales semanales. Los nuevos paneles de supresion en Grafana reflejan los
snippets PromQL abajo, dando visibilidad en vivo a los equipos on-call antes de
recurrir a consultas manuales.

### Recetas PromQL para revisar supresion

Los operadores deben tener a mano los siguientes helpers PromQL; ambos se
referencian en el dashboard compartido (`dashboards/grafana/soranet_privacy_metrics.json`)
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

Usa el ratio para confirmar que el stat **"Suppressed Bucket %"** se mantiene
por debajo del presupuesto de policy; conecta el detector de spikes a
Alertmanager para feedback rapido cuando el conteo de contribuyentes baje de
forma inesperada.

### CLI de reporte offline de buckets

El workspace expone `cargo xtask soranet-privacy-report` para capturas NDJSON
puntuales. Apuntalo a uno o mas exports admin del relay:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

El helper procesa la captura con `SoranetSecureAggregator`, imprime un resumen de
supresion en stdout y opcionalmente escribe un reporte JSON estructurado via
`--json-out <path|->`. Respeta los mismos knobs que el collector en vivo
(`--bucket-secs`, `--min-contributors`, `--expected-shares`, etc.), permitiendo
reproducir capturas historicas bajo distintos thresholds al investigar un issue.
Adjunta el JSON junto con screenshots de Grafana para que el gate de analitica
SNNet-8 siga siendo auditable.

### Checklist de primera ejecucion automatizada

Governance aun exige probar que la primera ejecucion automatizada cumplio el
presupuesto de supresion. El helper ahora acepta `--max-suppression-ratio <0-1>`
para que CI u operadores fallen rapido cuando los buckets suprimidos exceden la
ventana permitida (default 10%) o cuando aun no hay buckets presentes. Flujo
recomendado:

1. Exporta NDJSON desde el endpoint admin del relay y el stream
   `/v1/soranet/privacy/event|share` del orchestrator hacia
   `artifacts/sorafs_privacy/<relay>.ndjson`.
2. Ejecuta el helper con el presupuesto de policy:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   El comando imprime el ratio observado y sale con codigo distinto de cero
   cuando se excede el presupuesto **o** cuando no hay buckets listos, indicando
   que la telemetria aun no se ha producido para la ejecucion. Las metricas en
   vivo deben mostrar `soranet_privacy_pending_collectors` drenando hacia cero y
   `soranet_privacy_snapshot_suppression_ratio` quedandose bajo el mismo
   presupuesto mientras se ejecuta la corrida.
3. Archiva el JSON de salida y el log del CLI con el bundle de evidencia SNNet-8
   antes de cambiar el transporte default para que los revisores puedan
   reproducir los artefactos exactos.

## Proximos pasos (SNNet-8a)

- Integrar los collectors Prio duales, conectando la ingesta de shares al runtime
  para que relays y collectors emitan payloads `SoranetPrivacyBucketMetricsV1`
  consistentes. *(Done - ver `ingest_privacy_payload` en
  `crates/sorafs_orchestrator/src/lib.rs` y tests asociados.)*
- Publicar el dashboard Prometheus compartido y reglas de alerta que cubran
  gaps de supresion, salud de collector y anonymity brownouts. *(Done - ver
  `dashboards/grafana/soranet_privacy_metrics.json`,
  `dashboards/alerts/soranet_privacy_rules.yml`,
  `dashboards/alerts/soranet_policy_rules.yml`, y fixtures de validacion.)*
- Producir los artefactos de calibracion de differential privacy descritos en
  `privacy_metrics_dp.md`, incluyendo notebooks reproducibles y digests de
  governance. *(Done - notebook + artefactos generados por
  `scripts/telemetry/run_privacy_dp.py`; CI wrapper
  `scripts/telemetry/run_privacy_dp_notebook.sh` ejecuta el notebook via el
  workflow `.github/workflows/release-pipeline.yml`; governance digest archivado en
  `docs/source/status/soranet_privacy_dp_digest.md`.)*

La version actual entrega la base SNNet-8: telemetria determinista y segura
para la privacidad que se integra directo con scrapers y dashboards Prometheus
existentes. Los artefactos de calibracion de differential privacy estan listos,
el workflow de release mantiene frescas las salidas del notebook, y el trabajo
restante se enfoca en monitorear la primera ejecucion automatizada y extender
la analitica de alertas de supresion.
