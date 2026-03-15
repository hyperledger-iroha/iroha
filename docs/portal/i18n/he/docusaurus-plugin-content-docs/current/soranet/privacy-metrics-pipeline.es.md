---
lang: he
direction: rtl
source: docs/portal/docs/soranet/privacy-metrics-pipeline.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: privacy-metrics-pipeline
כותרת: Pipeline de metricas de privacidad de SoraNet (SNNet-8)
sidebar_label: Pipeline de metricas de privacidad
תיאור: Telemetria con preservacion de privacidad para relays y orchestrators de SoraNet.
---

:::שימו לב פואנטה קנוניקה
Refleja `docs/source/soranet/privacy_metrics_pipeline.md`. Mantengan ambas copias sincronizadas hasta que los docs heredados se retiren.
:::

# Pipeline de metricas de privacidad de SoraNet

SNNet-8 מציגה una superficie de telemetria consciente de la privacidad para
זמן ריצה של ממסר. Ahora el relay agrega eventos de handshake y circuit en
דליים de un minuto y exporta solo contadores Prometheus gruesos, manteniendo
los circuits individuales desvinculados mientras ofrece visibilidad acciónable
פאר לוס מפעילים.

## קורות חיים של אגרגדור

- La implementacion del runtime vive ב-`tools/soranet-relay/src/privacy.rs` como
  `PrivacyAggregator`.
- Los buckets se indexan por minuto de reloj (`bucket_secs`, ברירת מחדל 60 סגנונות) y
  se almacenan en un ring acotado (`max_completed_buckets`, ברירת מחדל 120). מניות לוס
  de collectors mantienen su propio backlog acotado (`max_share_lag_buckets`, ברירת מחדל 12)
  para que las ventanas Prio stale se vacien como buckets suprimidos en lugar de
  מזכרות מסננים או אספנים של מסננים.
- `RelayConfig::privacy` מפה ישירות ל-`PrivacyConfig`, כפתורי הפעלה
  (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`,
  `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`). זמן ריצה
  de produccion mantiene los ברירת מחדל mientras SNNet-8a להציג ספים de
  agregacion segura.
- Los modulos de runtime registran eventos with helpers tipados:
  `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`,
  `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`,
  `record_verified_bytes`, y `record_gar_category`.

## מנהל נקודת קצה ממסר

מפעילי המחלקה מייעצים למאזין מנהל ממסר עבור תצפית
crudas דרך `GET /privacy/events`. ה-Endpoint devuelve JSON מגביל
nuevas lineas (`application/x-ndjson`) עם מטענים `SoranetPrivacyEventV1`
reflejados desde el `PrivacyEventBuffer` interno. El buffer retiene los eventos
mas nuevos hasta `privacy.event_buffer_capacity` entradas (ברירת מחדל 4096) y se
vacía en la lectura, por lo que los scrapers deben sondear lo suficiente para
ללא dejar huecos. Los eventos cubren las mismas senales de לחיצת יד, מצערת,
רוחב פס מאומת, מעגל פעיל ו-GAR que alimentan los contadores Prometheus,
permitiendo a collectors downstream archivar breadcrumbs seguros para la privacidad
o זרימות עבודה של תזונה.

## תצורת ממסר

Los operadores ajustan la cadencia de telemetria de privacidad en el archivo de
תצורת ממסר דרך la seccion `privacy`:

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

ברירת המחדל של קמפו coinciden con la especificacion SNNet-8 y se validan al
מטען:| קמפו | תיאור | ברירת מחדל |
|-------|----------------|--------|
| `bucket_secs` | Ancho de cada ventana de agregacion (סגונדוס). | `60` |
| `min_handshakes` | Minimo de contribuyentes antes de emitir un bucket. | `12` |
| `flush_delay_buckets` | דליים משלימים אספראר ante del flush. | `1` |
| `force_flush_buckets` | Edad maxima antes de emitir un bucket supprimido. | `6` |
| `max_completed_buckets` | Backlog de buckets retenidos (evita memoria ilimitada). | `120` |
| `max_share_lag_buckets` | Ventana de retencion para shares de collectors antes de supprimir. | `12` |
| `expected_shares` | מניות Prio requeridos antes de combinar. | `2` |
| `event_buffer_capacity` | Backlog NDJSON של אירועים עבור זרם אדמין. | `4096` |

Configurar `force_flush_buckets` עבור debajo de `flush_delay_buckets`, פונר
los thresholds en cero o deshabilitar el guard de retencion ahora falla la
validacion para evitar despliegues que filtren telemetria por relay.

El limite `event_buffer_capacity` tambien acota `/admin/privacy/events`,
asegurando que los scrapers no queden atras indefinidamente.

## מניות של אספן פריו

SNNet-8a despliega אספנים דואלים que emiten דליים Prio con סוד שיתוף.
Ahora el orchestrator parsea el stream NDJSON `/privacy/events` tanto para
entradas `SoranetPrivacyEventV1` como para shares `SoranetPrivacyPrioShareV1`,
reenviandolos a `SoranetSecureAggregator::ingest_prio_share`. Los buckets se
emiten cuando lgan `PrivacyBucketConfig::expected_shares` תרומות,
reflejando el comportamiento del agregador en el relay. מניות לוס תקפות
por alineacion de bucket y forma del histograma antes de combinarse en
`SoranetPrivacyBucketMetricsV1`. Si el conteo combinado de לחיצות ידיים cae por
debajo de `min_contributors`, el bucket se exporta como `suppressed`, reflejando
el comportamiento del agregador en el relay. Las ventanas suprimidas ahora
emiten una etiqueta `suppression_reason` למען המבצעים של המבצע
`insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed`
y `forced_flush_window_elapsed` al diagnosticar huecos de telemetria. לה ראזון
`collector_window_elapsed` tambien dispara cuando las shares Prio se quedan mas
alla de `max_share_lag_buckets`, Haciendo visibles los collectors atascados sin
mantener acumuladores stale en memoria.

## נקודות קצה de ingesta Torii

Torii ahora expone dos endpoints HTTP con telemetria gateada para que relays y
אספנים reenvien observaciones sin incrustar un הובלה בהתאמה אישית:- `POST /v1/soranet/privacy/event` מקבל מטען
  `RecordSoranetPrivacyEventDto`. El body envuelve un `SoranetPrivacyEventV1`
  יש אופציונלי כללי התנהגות `source`. Torii valida la solicitud contra el
  פרופיל טלמטריה פעיל, רישום אירועים ותגובה עם HTTP
  `202 Accepted` junto con un envelope Norito JSON que contiene la ventana
  computada del bucket (`bucket_start_unix`, `bucket_duration_secs`) y el
  מוד דל ממסר.
- `POST /v1/soranet/privacy/share` מקבל מטען `RecordSoranetPrivacyShareDto`.
  El body lleva un `SoranetPrivacyPrioShareV1` y un רמז אופציונלי `forwarded_by`
  para que los operadores Auditen flujos de collectors. Las entregas exitosas
  devuelven HTTP `202 Accepted` עם מעטפה Norito JSON עם קורות חיים
  אספן, la ventana del bucket y el רמז דיכוי; לאס פאלאס דה
  validacion mapean ותשובה לטלמטריה `Conversion` לשמירה
  el manejo de errores determinista en collectors. לופ לאירוע של מתזמר
  ahora emite estas חולקת ממסרי מינטרס זונדה, manteniendo el acumulador
  Prio de Torii sincronizado con los buckets on-relay.

נקודות הקצה של Ambos חוזרות על פרופיל הטלמטריה: emiten `503 Service
לא זמין` cuando las metricas estan deshabilitadas. Los clients pueden enviar
cuerpos Norito בינארי (`application/x.norito`) או Norito JSON (`application/x.norito+json`);
el servidor negocia el formato automaticamente via los extractores standard de
Torii.

## Metricas Prometheus

Cada bucket exportado lleva etiquetas `mode` (`entry`, `middle`, `exit`) y
`bucket_start`. ראה את המדריכים המשפחתיים הבאים:

| מטריקה | תיאור |
|--------|----------------|
| `soranet_privacy_circuit_events_total{kind}` | טקסונומיה דה לחיצת יד קון `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | Contadores de Throttle con `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Duraciones agregadas de cooldown aportadas por לחיצות ידיים מצערות. |
| `soranet_privacy_verified_bytes_total` | Ancho de banda verificado de proofs de medicion cegada. |
| `soranet_privacy_active_circuits_{avg,max}` | Media y pico de circuits activos por bucket. |
| `soranet_privacy_rtt_millis{percentile}` | Estimaciones de percentil de RTT (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Contadores GAR hasheados por digest de categoria. |
| `soranet_privacy_bucket_suppressed` | דליים retenidos porque no se cumplio el umbral de contribuyentes. |
| `soranet_privacy_pending_collectors{mode}` | Acumuladores de shares pendientes de combinar, agrupados por modo de relay. |
| `soranet_privacy_suppression_total{reason}` | Contadores de buckets supprimidos con `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` para atribuir gaps de privacidad. |
| `soranet_privacy_snapshot_suppression_ratio` | Ratio supprimido/descargado del ultimo drain (0-1), util para presupuestos de alerta. |
| `soranet_privacy_last_poll_unixtime` | חותמת זמן UNIX del ultimo poll exitoso (alimenta la alerta collector-בטלה). |
| `soranet_privacy_collector_enabled` | מד que cae a `0` cuando el collector privacy esta deshabilitado o falla al iniciar (alimenta la alerta collector-disabled). |
| `soranet_privacy_poll_errors_total{provider}` | תקלות של סקר אגרופי לכינוי ממסר (מספר שגיאות פענוח, כשלים ב-HTTP או קודים קודמים). |Los buckets sin observaciones permanecen silenciosos, manteniendo לוחות מחוונים
limpios sin fabricar ventanas con ceros.

## Guia operativa

1. **לוחות מחוונים** - grafica las metricas anteriores agrupadas por `mode` y
   `window_start`. Destaca ventanas faltantes para revelar problemas in collectors
   o ממסרים. USa `soranet_privacy_suppression_total{reason}` עבור הבדל
   carencias de contribuyentes de supresion por collector al triage. El asset de
   Grafana אוורה כולל פאנל ייעודי **"סיבות דיכוי (5 מ')"**
   alimentado por esos contadores mas un stat **"Suppressed Bucket %"** que
   calcula `sum(soranet_privacy_bucket_suppressed) / count(...)` לבחירה
   para que los operadores vean brechas de presupuesto de un vistazo. לה סדרה
   **צבר שיתוף אספנים** (`soranet_privacy_pending_collectors`) y el stat
   **יחס דיכוי תמונת מצב** אספני רזלטן אטסקאדוס ודריסת תקציב
   durante ejecuciones automatizadas.
2. **התראה** - dispara alarmas desde contadores בטוח לפרטיות: picos de rechazo
   PoW, frecuencia de cooldown, drift de RTT ו-Rejects. קומו לוס
   Contadores בן מונוטוני dentro de cada דלי, reglas basadas en tasa
   funcionan bien.
3. **תגובה לאירוע** - confia primero en datos agregados. Cuando se requiera
   depuracion profunda, solicita a relays reproducer צילומי מצב של buckets o
   בדיקת הוכחות לרפואה
   crudos.
4. **שימור** - scrapea con suficiente frecuencia para no exceder
   `max_completed_buckets`. Los יצואנים deben tratar la salida Prometheus como
   la fuente canonica y descartar buckets locales una vez reenviados.

## Analitica de supresion y ejecuciones automatizadas

La aceptacion SNNet-8 תלוי בהצגה של אספנים אוטומטיות
mantienen sanos y que la supresion permanece dentro de los limites de policy
(<=10% של דליים לפי ממסר ב-30 דקות). אל כלי עבודה
necesario ya se incluye en el repo; los operadores deben integrarlo en sus
טקסים סמאנלים. Los nuevos paneles de supresion en Grafana reflejan los
קטעי PromQL abajo, dando visibilidad en vivo a los equipos on-call antes de
חוזרים על ייעוץ במדריך.

### Recetas PromQL para revisar supresion

Los operadores deben tener a mano los suientes helpers PromQL; ambos se
referencian en el לוח המחוונים compartido (`dashboards/grafana/soranet_privacy_metrics.json`)
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
פורמה אינספרדה.

### CLI de reporte offline de buckets

הצגת סביבת עבודה `cargo xtask soranet-privacy-report` עבור NDJSON
puntuales. Apuntalo a uno o mas ייצוא מנהל ממסר:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```El helper processa la captura con `SoranetSecureAggregator`, imprime un resumen de
supresion en stdout y optionalmente escribe un reporte JSON estructurado via
`--json-out <path|->`. Respeta los mismos knobs que el collector en vivo
(`--bucket-secs`, `--min-contributors`, `--expected-shares` וכו'), permitiendo
משחזרים ספים היסטוריים של ספים שונים לחקור את הנושא.
Adjunta el JSON junto con צילומי מסך של Grafana para que el gate de analitica
SNNet-8 siga siendo ניתן לביקורת.

### רשימת בדיקה אוטומטית של שחרור ראשוני

ממשל aun exige probar que la primera ejecucion automatizada cumplio el
presupuesto de supresion. El helper ahora acepta `--max-suppression-ratio <0-1>`
para que CI u operadores fallen rapido cuando los buckets suprimidos exceden la
ventana permitida (ברירת מחדל 10%) o cuando aun no hay buckets presentes. פלוג'ו
מומלץ:

1. ייצא את NDJSON למנהל נקודת הקצה של הממסר והזרם
   `/v1/soranet/privacy/event|share` del מתזמר hacia
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
   אנטה דה קמביאר אל טרנספורט ברירת מחדל para que los revisores puedan
   reproducir los artefactos exactos.

## Proximos pasos (SNNet-8a)

- Integrar los collectors Prio duales, conectando la ingesta de shares all runtime
  para que ממסרים y אספנים emitan מטענים `SoranetPrivacyBucketMetricsV1`
  עקבית. *(בוצע - ver `ingest_privacy_payload` en
  `crates/sorafs_orchestrator/src/lib.rs` y בדיקות associados.)*
- לוח המחוונים הציבורי Prometheus שיתוף פעולה ותקשורת
  פערים דה דיכוי, שלום אספן ואנונימיות. *(בוצע - ver
  `dashboards/grafana/soranet_privacy_metrics.json`,
  `dashboards/alerts/soranet_privacy_rules.yml`,
  `dashboards/alerts/soranet_policy_rules.yml`, y fixtures de validacion.)*
- Producir los artefactos de calibracion de differential privacy descritos en
  `privacy_metrics_dp.md`, כולל מחברות ניתנות לשחזור ותקצירים
  ממשל. *(בוצע - מחברת + artefactos generados por
  `scripts/telemetry/run_privacy_dp.py`; עטיפת CI
  `scripts/telemetry/run_privacy_dp_notebook.sh` מחברת ejecuta el via el
  זרימת עבודה `.github/workflows/release-pipeline.yml`; governance digest archivado en
  `docs/source/status/soranet_privacy_dp_digest.md`.)*

La גרסה בפועל entrega la base SNNet-8: telemetria determinista y segura
para la privacidad que se integra directo con scrapers y לוחות מחוונים Prometheus
existentes. Los artefactos de calibracion de differential privacy estan listos,
el workflow de release mantiene frescas las salidas del notebook, y el trabajo
restante se enfoca en monitorear la primera ejecucion automatizada y extender
la analitica de alertas de supresion.