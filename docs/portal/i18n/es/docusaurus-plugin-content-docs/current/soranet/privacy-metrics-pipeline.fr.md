---
lang: es
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: canalización-de-métricas-de-privacidad
título: Tubería de mediciones de confidencialidad SoraNet (SNNet-8)
sidebar_label: Tubería de mediciones de confidencialidad
descripción: Colección de télémétrie préservant la confidencialité pour les retransmisiones y orquestadores SoraNet.
---

:::nota Fuente canónica
Refléte `docs/source/soranet/privacy_metrics_pipeline.md`. Guarde las dos copias sincronizadas justo con el antiguo conjunto de documentación retirado.
:::

# Pipeline des métriques de confidencialité SoraNet

SNNet-8 introduce una superficie de televisión sensible a la confidencialidad para el tiempo de ejecución del relé. El relé aumenta los eventos de apretón de manos y de circuito en cubos de un minuto y no exporta los ordenadores Prometheus más gruesos, manteniendo los circuitos individuales no correlables todos y donnant aux operadores una visibilidad explotable.

## Descripción del agregador- El tiempo de ejecución de implementación se realiza en `tools/soranet-relay/src/privacy.rs` en `PrivacyAggregator`.
- Los cubos están indexados por minuto del reloj (`bucket_secs`, por defecto 60 segundos) y almacenados en un anillo nacido (`max_completed_buckets`, por defecto 120). Las acciones de los coleccionistas conservan el propio backlog borné (`max_share_lag_buckets`, por defecto 12) a fin de que las ventanas Prio périmées soient vidées en cubos supprimés plutôt que de fuiter la mémoire ou de enmascarar des Collecteurs bloqués.
- `RelayConfig::privacy` se asigna directamente a `PrivacyConfig`, exponiendo los ajustes (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`). El tiempo de ejecución de producción conserva los valores predeterminados desde que SNNet-8a introduce las secuencias de agregación segura.
- El tiempo de ejecución de los módulos registra los eventos a través de los tipos de ayuda: `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`, `record_verified_bytes` y `record_gar_category`.

## Administrador de punto final de retransmisiónLos operadores pueden interrogar al administrador del oyente del relé para las observaciones brutas a través de `GET /privacy/events`. El envío de JSON al endpoint está delimitado por nuevas líneas (`application/x-ndjson`) y contiene las cargas útiles `SoranetPrivacyEventV1` reflejadas después del `PrivacyEventBuffer` interno. El buffer conserva los eventos más recientes justo en los platos principales `privacy.event_buffer_capacity` (por defecto 4096) y está en video durante la lectura, donde los raspadores deben ser suficientes para evitar los trous. Los eventos incluyen las mismas señales de protocolo de enlace, aceleración, ancho de banda verificado, circuito activo y GAR que alimentan los ordenadores Prometheus, lo que permite que los recopiladores avalen el archivador de migas de pan para la confidencialidad o alimentación de los flujos de trabajo de agregación segura.

## Configuración del relé

Los operadores ajustan la cadencia de télémétrie de confidencialité en el archivo de configuración del relé a través de la sección `privacy`:

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

Los valores predeterminados de los campos corresponden a la especificación SNNet-8 y son válidos con cargo:| Campeón | Descripción | predeterminado |
|-------|-------------|--------|
| `bucket_secs` | Largeur de chaque fenêtre d'agrégation (segundos). | `60` |
| `min_handshakes` | Nombre mínimo de contribuyentes antes de que un cubo pueda emitir contadores. | `12` |
| `flush_delay_buckets` | Nombre de cubos completos para asistir antes de ensayar un lavado. | `1` |
| `force_flush_buckets` | Âge maximal avant d'émettre un cubeta supprimé. | `6` |
| `max_completed_buckets` | Backlog de cubetas conservadas (évite une mémoire non bornée). | `120` |
| `max_share_lag_buckets` | Fenêtre de rétention des accionarios cobradores antes de la supresión. | `12` |
| `expected_shares` | Acciones Prio de Collecteur requiere avant combinaison. | `2` |
| `event_buffer_capacity` | Backlog d'événements NDJSON para el administrador de flujo. | `4096` |

Definir `force_flush_buckets` además de `flush_delay_buckets`, poner los segundos en cero o desactivar el dispositivo de retención y desactivar la validación para evitar implementaciones que eliminen la télémétrie por relé.

El límite `event_buffer_capacity` corre a cargo del `/admin/privacy/events`, garantizando que los raspadores no pueden provocar un retardo indefinido.

## Acciones de coleccionistas PrioSNNet-8a implementa recolectores dobles que liberan cubos Prio à partage de secrets. El orquestador analiza el flujo NDJSON `/privacy/events` para los platos principales `SoranetPrivacyEventV1` y las acciones `SoranetPrivacyPrioShareV1`, y los transmite a `SoranetSecureAggregator::ingest_prio_share`. Les cubos émettent una fois que `PrivacyBucketConfig::expected_shares` llegan contribuciones, reflejando el comportamiento del relé. Las acciones son válidas para la alineación de los cubos y la forma del histograma antes de ser combinados en `SoranetPrivacyBucketMetricsV1`. Si el nombre combinado de apretones de manos se encuentra en `min_contributors`, el cubo se exporta como `suppressed`, lo que refleja el comportamiento del agregador de confianza. Las ventanas suprimidas activan una etiqueta `suppression_reason` para que los operadores puedan distinguir `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed` y `forced_flush_window_elapsed` durante el diagnóstico de los problemas de télémétrie. La razón `collector_window_elapsed` se debilita también cuando las acciones Prio traînent au-delà de `max_share_lag_buckets`, rendant les Collecteurs bloqués visibles sans laisser d'accumulateurs périmés en mémoire.

## Puntos finales de ingestión Torii

Torii expone dos puntos finales HTTP protegidos por la televisión porque los relés y recopiladores pueden transmitir observaciones sin embarcar un transporte sobre medida:- `POST /v1/soranet/privacy/event` acepta la carga útil `RecordSoranetPrivacyEventDto`. El cuerpo envuelve un `SoranetPrivacyEventV1` más una etiqueta `source` opcional. Torii valide la solicitud con el perfil de televisión activo, registre el evento y responda con HTTP `202 Accepted` acompañado de un sobre Norito Contenido JSON de la ventana calculada (`bucket_start_unix`, `bucket_duration_secs`) y el modo de relé.
- `POST /v1/soranet/privacy/share` acepta la carga útil `RecordSoranetPrivacyShareDto`. El cuerpo de transporte es un `SoranetPrivacyPrioShareV1` y un índice `forwarded_by` opcional para que los operadores puedan auditar el flujo de los coleccionistas. Las fuentes de datos solicitadas HTTP `202 Accepted` con un sobre Norito JSON resumen el recopilador, la ventana del cubo y la indicación de supresión; Las pruebas de validación corresponden a una respuesta de televisión `Conversion` para preservar un error determinado entre los coleccionistas. El conjunto de eventos del orquestador se desormará en estas acciones cuando interrogue los relés, según el acumulador Prio de Torii sincronizado con los cubos de relé.Los dos puntos finales respetan el perfil de la televisión: se activa `503 Service Unavailable` cuando las mediciones se desactivan. Los clientes pueden enviar archivos binarios Norito (`application/x.norito`) o Norito JSON (`application/x.norito+json`); El servidor cambia automáticamente el formato mediante los extractores estándar Torii.

## Métricas Prometheus

Cada cubo exportó porte les etiquetas `mode` (`entry`, `middle`, `exit`) et `bucket_start`. Les familles de métriques suivantes sont émises:| Métrica | Descripción |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | Taxonomía de apretones de manos con `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | Compteurs de throttle con `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Duración del tiempo de reutilización agregada al acelerar los apretones de manos. |
| `soranet_privacy_verified_bytes_total` | Bande passante vérifiée issues de preuves de mesure aveugles. |
| `soranet_privacy_active_circuits_{avg,max}` | Moyenne et pic de circuitos activos por cubo. |
| `soranet_privacy_rtt_millis{percentile}` | Estimaciones de percentiles RTT (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | El Informe de acción de los Compteurs de Governance hachés indexés par digest de catégorie. |
| `soranet_privacy_bucket_suppressed` | Buckets retenus parce que le seuil de contribuidours n'a pas été atteint. |
| `soranet_privacy_pending_collectors{mode}` | Acumuladores de acciones de coleccionistas en atención de combinación, agrupados por modo de retransmisión. |
| `soranet_privacy_suppression_total{reason}` | Los ordenadores de cubos actualizados con `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` permiten que los paneles de control atribuyan los recursos confidenciales. |
| `soranet_privacy_snapshot_suppression_ratio` | Ratio superior/vídeo del último drenaje (0-1), útil para los presupuestos de alerta. |
| `soranet_privacy_last_poll_unixtime` | Horodatage UNIX du dernier poll réussi (alimente l'alerte coleccionista-inactivo). |
| `soranet_privacy_collector_enabled` | Calibre que pasa a `0` cuando el recopilador de confidencialidad está desactivado o no comienza (alimente el alertador está desactivado). || `soranet_privacy_poll_errors_total{provider}` | Controles de sondeo agrupados por alias de retransmisión (incrementan los errores de descodificación, controles HTTP o códigos de estado inatendidos). |

Los cubos sin observaciones permanecen silenciosos, vigilando los tableros propios sin fabricantes de ventanas que responden a cero.

## Guía operativa1. **Paneles** - trace les métriques ci-dessus groupées par `mode` et `window_start`. Mettre en évidence les fenêtres manquantes pour faire remonter les problèmes de Collecteur ou de Relay. Utilice `soranet_privacy_suppression_total{reason}` para distinguir las insuficiencias de los contribuyentes de supresiones pilotadas por los recopiladores durante la clasificación de los trous. El activo Grafana incluye un panel predeterminado **"Suppression Reasons (5m)"** alimentado por estos computadores, además de una estadística **"Suppressed Bucket %"** que calcula `sum(soranet_privacy_bucket_suppressed) / count(...)` por selección para que los operadores representen los cambios de presupuesto en un golpe de efecto. La serie **"Collector Share Backlog"** (`soranet_privacy_pending_collectors`) y la estadística **"Snapshot Suppression Ratio"** indican que los recopiladores bloqueados y la derivación del presupuesto durante las ejecuciones automáticas.
2. **Alertas**: activa las alertas desde los ordenadores de seguridad para la confidencialidad: imágenes de rechazo de PoW, frecuencia de enfriamiento, derivación de RTT y rechazos de capacidad. Como los ordenadores son monótonos en el interior de cada cubo, las reglas de tareas simples funcionan bien.
3. **Respuesta al incidente** - s'appuyer d'abord sur les données agrégées. Para realizar un débogage plus profundo es necesario, demande aux relés de rejouer des snapshots de cubes o d'inspecter des preuves de mesure aveugles au lieu de récolter des journaux de trafic bruts.4. **Retención** - raspador suficiente para evitar el paso `max_completed_buckets`. Los exportadores no traen la salida Prometheus como fuente canónica y eliminan los cubos ubicados en una hoja de transmisión.

## Analizar la supresión y ejecución automática

La aceptación de SNNet-8 depende de la demostración de que los coleccionistas automatizados restent sains y de que la supresión reste dentro de los límites de la política (≤10 % de los cubos por relevo en todas las ventanas de 30 minutos). Las herramientas necesarias para satisfacer esta exigencia se mantienen en libros con el depósito; Los operadores deben integrar los rituales de los hebdomadaires. Los nuevos paneles de supresión Grafana reflejan los extractos PromQL ci-dessous, donnant aux équipes d'astreinte una visibilité en direct avant de recurr à des requêtes manuelles.

### Recettes PromQL para la revista de supresión

Los operadores deben guardar los ayudantes de PromQL siguientes al puerto principal; Las dos referencias se encuentran en el tablero Grafana compartido (`dashboards/grafana/soranet_privacy_metrics.json`) y las reglas Alertmanager:

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

Utilice el ratio para confirmar la estadística **"Suppressed Bucket %"** restante en el presupuesto de la política; Rama el detector de imágenes en Alertmanager para un retorno rápido cuando el nombre de los contribuyentes se realiza de manera inatendida.### CLI de relación de cubo fuera de línea

El espacio de trabajo expone `cargo xtask soranet-privacy-report` para las capturas ponctuelles de NDJSON. Pointez-le sur un ou plusieurs exports admin de retransmisión :

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

Para hacer pasar la captura por `SoranetSecureAggregator`, imprima un currículum de supresión en salida estándar y, opcionalmente, escriba una relación estructurada JSON a través de `--json-out <path|->`. Il respecte les mêmes réglages que le Collecteur live (`--bucket-secs`, `--min-contributors`, `--expected-shares`, etc.), permettant aux opérateurs de rejouer des captures historiques sous des seuils différents lors du triage d'un incident. Ingrese el JSON con las capturas Grafana para que la puerta de análisis de supresión SNNet-8 sea auditable.

### Lista de verificación de ejecución automática de estreno

La gobernanza exige siempre asegurar que la primera ejecución sea automática respetando el presupuesto de supresión. La herramienta acepta desormais `--max-suppression-ratio <0-1>` después de que el CI o los operadores puedan activarse rápidamente cuando los cubos superen la ventana autorizada (10 % por defecto) o cuando el cubo no esté presente otra vez. Flujo recomendado:

1. Exportador de NDJSON desde los puntos finales del administrador del relé más el flujo `/v1/soranet/privacy/event|share` del orquestador versus `artifacts/sorafs_privacy/<relay>.ndjson`.
2. Ejecutar la herramienta con el presupuesto político :

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```El comando imprime la relación observada y se termina con un código no nulo cuando el presupuesto se ha superado **o** cuando el cubo no está listo, indicando que la televisión no se ha vuelto a producir para la ejecución. Les métriques live doivent montrer `soranet_privacy_pending_collectors` se vidant vers cero et `soranet_privacy_snapshot_suppression_ratio` restant sous le même presupuesto colgante de ejecución.
3. Archive la salida JSON y la revista CLI con el expediente de preuve SNNet-8 antes de bascular el transporte por defecto para que los revisores puedan recargar los artefactos exactos.

## Prochaines étapes (SNNet-8a)- Integrar los recolectores Prio doubles, conectando la ingestión de acciones en tiempo de ejecución a fin de que los relés y recolectores émettent des payloads `SoranetPrivacyBucketMetricsV1` coherentes. *(Fait — voir `ingest_privacy_payload` dans `crates/sorafs_orchestrator/src/lib.rs` et les tests associés.)*
- Publier le Dashboard Prometheus partagé et les règles d'alerte couvrant les trous de supression, la santé des Collecteurs et les baisses d'anonymat. *(Fait — voir `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` y los accesorios de validación.)*
- Produire les artefactos de calibración de confidencialité différentielle décrits dans `privacy_metrics_dp.md`, y comprende des notebooks reproductibles et des digests de gouvernance. *(Fait — notebook + artefactos generados por `scripts/telemetry/run_privacy_dp.py`; el contenedor CI `scripts/telemetry/run_privacy_dp_notebook.sh` ejecuta el notebook a través del flujo de trabajo `.github/workflows/release-pipeline.yml`; resumen de gobierno depositado en `docs/source/status/soranet_privacy_dp_digest.md`.)*

La versión actual del libro SNNet-8: un televisor determinado y seguro para la confidencialidad que está integrado directamente en los raspadores Prometheus y en los paneles auxiliares. Los artefactos de calibración de confidencialidad diferentes están en su lugar, el flujo de trabajo de la tubería de liberación mantiene las salidas del cuaderno del día y el trabajo restante se concentra en la vigilancia de la ejecución de estreno automatizada más la extensión de los análisis de alerta de supresión.