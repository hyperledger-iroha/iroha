---
lang: es
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: canalización-de-métricas-de-privacidad
título: Convertidor métrico de privacidad SoraNet (SNNet-8)
sidebar_label: Convertidor métrico privado
descripción: Сбор телеметрии с сохранением приватности для retransmisión y orquestador SoraNet.
---

:::nota Канонический источник
Utilice `docs/source/soranet/privacy_metrics_pipeline.md`. Deje copias sincronizadas, ya que no necesita guardar documentos exclusivos.
:::

# Конвейер метрик приватности SoraNet

SNNet-8 está orientado a la privacidad de los televisores para el relé de tiempo de ejecución. El relé de temperatura agrega el apretón de manos y el circuito en cubos pequeños y exporta a los circuitos externos Prometheus. несвязываемыми и давая операторам практическую видимость.

## Agregador de objetos- Runtime-rеализация находится в `tools/soranet-relay/src/privacy.rs` как `PrivacyAggregator`.
- Buckets индексируются по minутам настенного времени (`bucket_secs`, по умолчанию 60 секунд) и хранятся в ограниченном кольце (`max_completed_buckets`, en el número 120). Las acciones de coleccionista incluyen una acumulación de trabajos pendientes (`max_share_lag_buckets`, según el número 12), algunos usuarios de Prio que se encuentran en cubos suprimidos y otros. утекали в память или маскировали зависшие coleccionistas.
- `RelayConfig::privacy` compatible con `PrivacyConfig`, compatible con (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`). En el tiempo de ejecución de producción de la mayoría de los usuarios, SNNet-8a incluye algunas agregaciones básicas.
- Las configuraciones del tiempo de ejecución del módulo están compuestas por ayudantes típicos: `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`, `record_verified_bytes` y `record_gar_category`.

## Relevo del administradorLos operadores pueden operar el relé de escucha de administrador para su dispositivo desde `GET /privacy/events`. Este es un archivo JSON con una base de datos (`application/x-ndjson`), cargas útiles integradas `SoranetPrivacyEventV1`, otras funciones `PrivacyEventBuffer`. El módulo de memoria intermedia tiene una nueva conexión con el archivo `privacy.event_buffer_capacity` (con el número 4096) y otras fuentes de alimentación. скрейперам нужно опрашивать достаточно часто, чтобы избежать пропусков. События покрывают те же сигналы handshake, throttle, verificado ancho de banda, circuito activo y GAR, которые питают счетчики Prometheus, позволяя downstream colectores архивировать Migas de pan individuales y agregaciones gratuitas de flujos de trabajo.

## Relé de configuración

Los operadores configuran los televisores privados de cadencia en el archivo de configuración del relé según las secciones `privacy`:

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

Значения по умолчанию соответствуют спецификации SNNet-8 y проверяются при загрузке:| Polo | Descripción | По умолчанию |
|------|----------|--------------|
| `bucket_secs` | Ширина каждого окна агрегации (секунды). | `60` |
| `min_handshakes` | Minimalnoe число участников перед выпуском счетчиков из cubeta. | `12` |
| `flush_delay_buckets` | Количество завершенных cubos перед попыткой al ras. | `1` |
| `force_flush_buckets` | Максимальный возраст перед выпуском cubo suprimido. | `6` |
| `max_completed_buckets` | Хранимый depósitos de trabajos pendientes (не дает памяти расти без границ). | `120` |
| `max_share_lag_buckets` | Окно удержания acciones de coleccionista до подавления. | `12` |
| `expected_shares` | Las acciones de coleccionista de Число Prio перед объединением. | `2` |
| `event_buffer_capacity` | Backlog NDJSON disponible para el administrador. | `4096` |

Установка `force_flush_buckets` ниже `flush_delay_buckets`, обнуление порогов или отключение guard удержания теперь проваливает валидацию, чтобы Asegúrese de que el televisor esté conectado al relé del aparato.

El límite `event_buffer_capacity` y el `/admin/privacy/events` garantizan que estos scripts no se pueden configurar de forma segura.

## Acciones de coleccionista de PrioSNNet-8a разворачивает двойные colectores, которые выпускают секретно-разделенные cubos Prio. El orquestador parte el archivo NDJSON `/privacy/events` para el archivo `SoranetPrivacyEventV1` y comparte `SoranetPrivacyPrioShareV1`, que se encuentra en `SoranetSecureAggregator::ingest_prio_share`. Cucharones выпускаются, когда приходит `PrivacyBucketConfig::expected_shares` вкладов, отражая поведение relé. Las acciones se validan según los depósitos y los formularios de los gráficos antes de ser observados en `SoranetPrivacyBucketMetricsV1`. Este es un apretón de manos que no es `min_contributors`, un cubo exportado como `suppressed`, un agregador de potencia внутри relé. Cuando el operador utiliza el método `suppression_reason`, sus operadores pueden seleccionar `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed` e `forced_flush_window_elapsed` antes del diagnóstico de los televisores. Причина `collector_window_elapsed` также срабатывает, когда Prio Shares задерживаются дольше `max_share_lag_buckets`, делая зависшие coleccionistas видимыми без хранения устаревших аккумуляторов в памяти.

## Puntos principales Torii

Torii el tipo de publicación de terminales HTTP, los relés y los recopiladores pueden permitir la conexión sin configuración transporte seguro:- `POST /v2/soranet/privacy/event` carga útil del modelo `RecordSoranetPrivacyEventDto`. Este equipo está equipado con `SoranetPrivacyEventV1` y no necesita el método `source`. Torii proporciona información sobre los perfiles de actividad de los televisores, la configuración de la conexión y la activación de HTTP `202 Accepted` вместе с Norito Convertidor JSON, содержащим вычисленное окно (`bucket_start_unix`, `bucket_duration_secs`) y el relé.
- `POST /v2/soranet/privacy/share` carga útil del modelo `RecordSoranetPrivacyShareDto`. Si no hay `SoranetPrivacyPrioShareV1` y no hay soportes `forwarded_by`, estos operadores pueden auditar a los coleccionistas de fotos. Las aplicaciones que utilizan HTTP `202 Accepted` con Norito son un convertidor JSON, un recopilador de resumen, un depósito de datos y una fuente de alimentación; ошибки валидации сопоставляются с телеметрическим ответом `Conversion`, чтобы сохранить детерминированную обработку ошибок между coleccionistas. El ciclo del orquestador se realiza mediante relés opuestos, sincronización sincronizada con el acumulador Prio Torii con cubos en el relé.

Оба эндпоинта учитывают профиль телеметрии: они возвращают `503 Service Unavailable`, когда метрики отключены. Los clientes pueden utilizar el formato binario Norito (`application/x.norito`) o Norito JSON (`application/x.norito+json`); Servidor automático de formato para extractores estándar Torii.

## Métricas PrometheusEl cucharón deportivo portátil necesita metales `mode` (`entry`, `middle`, `exit`) y `bucket_start`. Выпускаются следующие семейства метрик:| Métrica | Descripción |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | Таксономия apretones de manos con `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | Счетчики acelerador с `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Tiempo de reutilización más rápido y apretones de manos acelerados. |
| `soranet_privacy_verified_bytes_total` | Проверенная пропускная способность от слепых доказательств измерений. |
| `soranet_privacy_active_circuits_{avg,max}` | Среднее и пик активных circuitos en el cubo. |
| `soranet_privacy_rtt_millis{percentile}` | Percentiles RTT adicionales (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Хешированные счетчики Informe de acción de gobernanza, ключом является resumen de categorías. |
| `soranet_privacy_bucket_suppressed` | Los cubos удержаны, потому что порог участников не был достигнут. |
| `soranet_privacy_pending_collectors{mode}` | Аккумуляторы acciones de coleccionista в ожидании объединения, сгруппированные по режиму relevo. |
| `soranet_privacy_suppression_total{reason}` | Los cubos suprimidos счетчики с `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}`, чтобы дашборды могли атрибутировать провалы приватности. |
| `soranet_privacy_snapshot_suppression_ratio` | Доля suprimido/слитых на последнем drenaje (0-1), полезно для бюджетов алертов. |
| `soranet_privacy_last_poll_unixtime` | UNIX-время самого свежего успешного опроса (colector de alertas inactivo). |
| `soranet_privacy_collector_enabled` | Gauge, который падает до `0`, когда Privacy Collector отключен или не запустился (питает алерт падает-disabled). || `soranet_privacy_poll_errors_total{provider}` | Ошибки опроса по алиасу retransmisión (увеличивается при ошибках декодирования, HTTP-сбоях или неожиданных статус-кодах). |

Los cubos без наблюдений остаются молчаливыми, сохраняя дашборды чистыми без создания нулевых окон.

## Recomendaciones operativas1. **Дашборды** - строьте метрики выше, группируя по `mode` and `window_start`. Puede que haya problemas con los colectores o los relés. Utilice `soranet_privacy_suppression_total{reason}` para eliminar la necesidad de supresión y colectores, colectores defectuosos y tres sondas. En Grafana está el panel **"Suppression Reasons (5m)"** en la base de datos y estadísticas **"Suppressed Bucket %"**, junto con `sum(soranet_privacy_bucket_suppressed) / count(...)` выбранному диапазону, чтобы операторы видели нарушения бюджета с первого взгляда. Serie **"Collector Share Backlog"** (`soranet_privacy_pending_collectors`) y estadística **"Snapshot Suppression Ratio"** выделяют зависшие coleccionistas y дрейф бюджета во время автоматизированных прогонов.
2. **Alerta** - запускайте тревоги от приватно-безопасных счетчиков: всплески PoW rechazo, частоту cooldown, дрейф RTT y capacidad rechazada. Si coloca monotones en el cubo anterior, deberá colocarlos en la parte superior del cubo.
3. **Incidente** - сначала полагайтесь на агрегированные данные. Когда нужен более глубокий дебаг, просите relays воспроизвести snapshots buckets или проверить слепые доказательства измерений вместо сбора сырых логов трафика.
4. **Удержание** - escriba una cuenta de fábrica, no debe consultar `max_completed_buckets`. Los exportadores deben colocar en el interior el Prometheus, un sistema de almacenamiento portátil y cubos locales ubicados después de su uso.## Supresión analítica y programas automáticos

La configuración SNNet-8 elimina las demostraciones de este tipo, cómo se eliminan los colectores automáticos y cómo se suprimen los dispositivos anteriores. политики (≤10% cubos en el relé durante 30 minutos). Todos los instrumentos deben colocarse todos en el repositorio; Los operadores deben realizar ajustes en diferentes rituales. Nuevos paneles de supresión en Grafana que no están activados por PromQL, ya que el comando de usuario está aquí para ver esto. прибегнут к ручным запросам.

### PromQL-рецепты для обзора supresión

Los operadores deben utilizar el software PromQL-хелперы; оба упомянуты в общем дашборде Grafana (`dashboards/grafana/soranet_privacy_metrics.json`) y el administrador de alertas:

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

Utilice la información adicional para indicar que la estadística **"% de depósito suprimido"** no está disponible por separado; подключите детектор всплесков к Alertmanager для быстрого отклика, когда число участников неожиданно падает.

### CLI para cubos de офлайн-отчета

En el espacio de trabajo se encuentra `cargo xtask soranet-privacy-report` para la configuración NDJSON. Укажите один или несколько admin-экспортов relé:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```El ayudante que realiza el trabajo con `SoranetSecureAggregator`, conecta la supresión de datos en la salida estándar y la estructura de la estructura JSON. `--json-out <path|->`. Он учитывает те же настройки, что и live coleccionista (`--bucket-secs`, `--min-contributors`, `--expected-shares` y т. д.), позволяя операторам воспроизводить исторические захваты с другими порогами при разборе инцидента. Utilice archivos JSON en las pantallas Grafana, que son compatibles con SNNet-8 y la supresión analítica del audio.

### Чеклист первой автоматизированной сессии

Los gobernantes pueden activar el sistema de supresión de mensajes automáticamente. El asistente de mantenimiento utiliza `--max-suppression-ratio <0-1>`, los CI y los operadores que pueden proteger los cubos suprimidos de forma segura. допустимое окно (по умолчанию 10%) или когда cubos еще отсутствуют. Fotos recomendadas:

1. Exporte NDJSON con retransmisión de puntos finales de administrador y orquestador de archivos `/v2/soranet/privacy/event|share` en `artifacts/sorafs_privacy/<relay>.ndjson`.
2. Запустить хелпер с бюджетом политики:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```Команда печатает наблюдаемое соотношение and завершает работу с ненулевым кодом, когда бюджет превышен **или** когда cubos еще не готовы, сигнализируя, что телеметрия еще не была произведена для progona. Los parámetros en vivo se pueden conectar a `soranet_privacy_pending_collectors` y no se conectan a `soranet_privacy_snapshot_suppression_ratio` y no se conectan en tiempo real. прогона.
3. Conecte el archivo JSON y el registro CLI con el paquete de archivos SNNet-8 para el transporte de mercancías por partes. ревьюеры могли воспроизвести точные артефакты.

## Следующие шаги (SNNet-8a)

- Integración de los recopiladores Prio, las acciones principales en tiempo de ejecución, los relés y los recopiladores de cargas útiles `SoranetPrivacyBucketMetricsV1`. *(Готово — см. `ingest_privacy_payload` в `crates/sorafs_orchestrator/src/lib.rs` и сопутствующие тесты.)*
- Publicar el JSON del tablero Prometheus y alertas detalladas, brechas de supresión, recopiladores de información y anonimato. *(Готово — см. `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` y проверки.)*
- Подготовить артефакты калибровки диференциальной приватности, описанные в `privacy_metrics_dp.md`, включая воспроизводимые resumen de bloques y gobernanza. *(Готово — notebook and артефакты генерируются `scripts/telemetry/run_privacy_dp.py`; CI wrapper `scripts/telemetry/run_privacy_dp_notebook.sh` implementa un notebook con el flujo de trabajo `.github/workflows/release-pipeline.yml`; digest сохранен в `docs/source/status/soranet_privacy_dp_digest.md`.)*La solución de instalación del software SNNet-8: детерминированную, приватно-безопасную телеметрию, которая напрямую подключается к существующим скрейперам и дашбордам Prometheus. Артефакты калибровки диференциальной приватности на месте, flujo de trabajo релизного пайплайна поддерживает актуальность вывода notebook, а оставшаяся работа сосредоточена на мониторинге первого автоматизированного progona and расширении аналитики-spression-alertov.