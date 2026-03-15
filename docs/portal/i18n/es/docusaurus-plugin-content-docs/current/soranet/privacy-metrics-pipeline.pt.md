---
lang: es
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: canalización-de-métricas-de-privacidad
título: Tubería de métricas de privacidad de SoraNet (SNNet-8)
sidebar_label: Tubería de métricas de privacidade
descripción: Coleta de telemetría que preserva la privacidad de retransmisiones y orquestadores de SoraNet.
---

:::nota Fuente canónica
Reflejo `docs/source/soranet/privacy_metrics_pipeline.md`. Mantenha ambas como copias sincronizadas.
:::

# Pipeline de métricas de privacidad de SoraNet

SNNet-8 introduce una superficie de telemetría consciente de privacidad para el tiempo de ejecución del relé. El relé ahora agrega eventos de apretón de manos y circuitos en cubos de un minuto y exporta apenas contadores Prometheus grossos, manteniendo circuitos individuales desvinculados en cuanto por necesidad de visibilidad activa de los operadores.

## Visao general del agregador- La implementación del tiempo de ejecución es `tools/soranet-relay/src/privacy.rs` como `PrivacyAggregator`.
- Buckets sao chaveados por minuto de reloj (`bucket_secs`, predeterminado 60 segundos) y armados en un anillo limitado (`max_completed_buckets`, predeterminado 120). Las acciones de coleccionista mantienen su propia cartera de pedidos limitada (`max_share_lag_buckets`, por defecto 12) para que janelas Prio antigas sejam esvaziadas como cubos suprimidos en vez de vazar memoria o mascarar presos de coleccionistas.
- `RelayConfig::privacy` mapa directo para `PrivacyConfig`, perillas de ajuste expondo (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`). El tiempo de ejecución de producción mantiene los valores predeterminados mientras SNNet-8a introduce umbrales de agregación segura.
- Los módulos de tiempo de ejecución registran eventos a través de ayudantes tipados: `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`, `record_verified_bytes`, e `record_gar_category`.

## El administrador del punto final realiza la retransmisiónLos operadores pueden consultar al administrador del oyente para transmitir observaciones brutas a través de `GET /privacy/events`. El endpoint retorna JSON delimitado por nuevas líneas (`application/x-ndjson`) contiene cargas útiles `SoranetPrivacyEventV1` distribuidas en `PrivacyEventBuffer` interno. O buffer guarda os eventos mais novos ate `privacy.event_buffer_capacity` entradas (predeterminado 4096) y e drenado na leitura, entao scrapers devem sondar con frecuencia suficiente para evitar lagunas. Los eventos se combinan con los protocolos de enlace, aceleración, ancho de banda verificado, circuito activo y GAR que alimentan los contadores Prometheus, lo que permite a los recopiladores almacenar migas de pan de forma segura para privacidad o alimentar flujos de trabajo de forma segura.

## Configuración del relé

Los operadores ajustan la cadencia de telemetría de privacidad en el archivo de configuración del relé vía secao `privacy`:

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

Los valores predeterminados de los dos campos corresponden a la especificación SNNet-8 y están validados sin registro:| Campo | Descripción | Padrón |
|-------|-----------|--------|
| `bucket_secs` | Largura de cada janela de agregacao (segundos). | `60` |
| `min_handshakes` | Numero minimo de contribuyentes antes de un cubo poder emitir contadores. | `12` |
| `flush_delay_buckets` | Buckets completos a esperar antes de tentar un enjuague. | `1` |
| `force_flush_buckets` | Idad máxima antes de emitir un cubo suprimido. | `6` |
| `max_completed_buckets` | Backlog de buckets retidos (impide la memoria sem limite). | `120` |
| `max_share_lag_buckets` | Janela de retencao para coleccionistas de acciones antes de suprimir. | `12` |
| `expected_shares` | Prio coleccionista comparte exigidos antes de combinar. | `2` |
| `event_buffer_capacity` | Backlog de eventos NDJSON para el administrador de transmisiones. | `4096` |

Definir `force_flush_buckets` menor que `flush_delay_buckets`, cerrar los umbrales o desactivar la guardia de retención ahora falta en la validación para evitar implementaciones que evaden la telemetría por retransmisión.

O limite `event_buffer_capacity` también limita `/admin/privacy/events`, garantizando que los raspadores no pueden quedar atrapados indefinidamente.

## Acciones de coleccionista de PrioSNNet-8a implanta colectores dobles que emiten cubos Prio con compartimiento secreto. El orquestador ahora analiza el stream NDJSON `/privacy/events` para entradas `SoranetPrivacyEventV1` e share `SoranetPrivacyPrioShareV1`, encaminando-as para `SoranetSecureAggregator::ingest_prio_share`. Los cubos emiten cuando chegam `PrivacyBucketConfig::expected_shares` contribuyen, espelhando o comportamento do relé. As share sao validadas para alinhamento de bucket e forma do histograma antes de serem combinados em `SoranetPrivacyBucketMetricsV1`. Se a contagem combinado de handshakes ficar abaixo de `min_contributors`, o bucket y exportado como `suppressed`, espelhando o comportamento do agregador no Relay. Janelas suprimidas ahora emite una etiqueta `suppression_reason` para que los operadores puedan distinguir entre `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed`, e `forced_flush_window_elapsed` para diagnosticar lagunas de telemetría. O motivo `collector_window_elapsed` también dispara cuando Prio comparte ficam alem de `max_share_lag_buckets`, tornando coleccionistas presos visiveis sem deixar acumuladores antiguos en memoria.

## Puntos finales de ingesta de Torii

Torii ahora exponen dos puntos finales HTTP con puerta de telemetría para que los relés y recolectores puedan encaminar observaciones sin realizar un transporte a medida:- `POST /v2/soranet/privacy/event` contiene una carga útil `RecordSoranetPrivacyEventDto`. El cuerpo incluye un `SoranetPrivacyEventV1` más una etiqueta `source` opcional. Torii valida la solicitud contra el perfil de telemetría activa, registra o evento, y responde con HTTP `202 Accepted` junto con un sobre Norito JSON conteniendo a janela calculada (`bucket_start_unix`, `bucket_duration_secs`) y el modo de retransmisión.
- `POST /v2/soranet/privacy/share` contiene una carga útil `RecordSoranetPrivacyShareDto`. O corpo carrega um `SoranetPrivacyPrioShareV1` e uma dica `forwarded_by` opcional para que los operadores puedan auditar flujos de coleccionistas. Submissoes bem-sucedidas retornam HTTP `202 Accepted` con un sobre Norito JSON resumindo o colector, a janela de bucket y a dica de supressao; Falhas de validacao mapeiam para una respuesta de telemetría `Conversion` para preservar el tratamiento determinístico de errores entre coleccionistas. El bucle de eventos del orquestador ahora emite esas acciones para hacer el sondeo de dos relés, manteniendo o acumulador Prio do Torii sincronizado con los cubos sin relé.

Ambos puntos finales respetan el perfil de telemetría: emiten `503 Service Unavailable` cuando las métricas están desativadas. Los clientes pueden enviar corpos Norito binario (`application/x.norito`) o Norito JSON (`application/x.norito+json`); El servidor negocia automáticamente el formato mediante extractores padrao do Torii.

## Métricas PrometheusCada cubeta exportado carrega etiquetas `mode` (`entry`, `middle`, `exit`) e `bucket_start`. Como siguientes familias de métricas sao emitidas:| Métrica | Descripción |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | Taxonomía de apretón de manos con `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | Contadores de acelerador con `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Duración del tiempo de reutilización agregada por apretones de manos acelerados. |
| `soranet_privacy_verified_bytes_total` | Ancho de banda verificado de pruebas de medicamento cega. |
| `soranet_privacy_active_circuits_{avg,max}` | Medios y pico de circuitos activos por cubo. |
| `soranet_privacy_rtt_millis{percentile}` | Estimaciones de percentil RTT (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Contadores de Governance Action Report com hash por digest de categoria. |
| `soranet_privacy_bucket_suppressed` | Buckets retidos porque o limiar de contribuyentes nao foi atingido. |
| `soranet_privacy_pending_collectors{mode}` | Acumuladores de acciones de coleccionista colgantes de combinacao, agrupados por modo de relevo. |
| `soranet_privacy_suppression_total{reason}` | Contadores de cubos suprimidos con `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` para que los paneles de control atribuam lagunas de privacidad. |
| `soranet_privacy_snapshot_suppression_ratio` | Razao suprimida/drenada do ultimo drenaje (0-1), util para presupuestos de alerta. |
| `soranet_privacy_last_poll_unixtime` | Marca de tiempo UNIX de la última encuesta realizada (alimenta o alerta coleccionista-idle). |
| `soranet_privacy_collector_enabled` | Gauge que vira `0` cuando el colector de privacidad está desactivado o falla al iniciar (alimenta o alerta colector-disabled). || `soranet_privacy_poll_errors_total{provider}` | Faltas de sondeo agrupadas por alias de retransmisión (incrementos en errores de decodificación, faltas HTTP o códigos de estado inesperados). |

Buckets sem observacoes permanecem silenciosos, mantendo tableros de instrumentos limpios sem fabrican janelas zeradas.

## Orientación operativa1. **Paneles**: seguimiento de métricas acima agrupadas por `mode` e `window_start`. Destaque janelas ausentes para revelar problemas de coleccionista o relé. Utilice `soranet_privacy_suppression_total{reason}` para distinguir falta de contribuyentes de supresores orientados por coleccionistas ao triar lagunas. El activo Grafana ahora envía un dolor dedicado **"Suppression Reasons (5m)"** alimentado por esos contadores más una estadística **"Suppressed Bucket %"** que calcula `sum(soranet_privacy_bucket_suppressed) / count(...)` por selección para que los operadores vean violacoes de presupuesto rápidamente. La serie **"Collector Share Backlog"** (`soranet_privacy_pending_collectors`) y la estadística **"Snapshot Suppression Ratio"** destacan los coleccionistas presos y desvio de presupuesto durante ejecuciones automatizadas.
2. **Alertas** - Conduza alarmas a partir de contadores seguros de privacidade: picos de rechazo de PoW, frecuencia de enfriamiento, deriva de RTT y rechazos de capacidad. Como os contadores son monótonos dentro de cada cubeta, registros simples basados ​​en taxa funcionan bien.
3. **Respuesta al incidente** - confie primeiro nos dados agregados. Cuando sea necesaria una depuración más profunda, solicite que los relés reproduzcan instantáneas de cubos o inspeccionen pruebas de medicamentos obtenidas en vez de coletar registros de tráfico bruto.4. **Retención** - raspado de fachada con frecuencia suficiente para evitar exceder `max_completed_buckets`. Los exportadores deben tratar a Saida Prometheus como fonte canonica e descartar buckets locais depois de encaminhados.

## Análisis de supresión y ejecución automática

La aceitacao de SNNet-8 depende de demostrar que los recolectores automatizados permanecen seguros y que a supressao fica dentro de dos limites da politica (<=10% dos cubos por retransmisión em qualquer janela de 30 minutos). Las herramientas necesarias para instalar esa puerta ahora con el repositorio; Los operadores deben integrar esto en sus rituales semanales. Los nuevos dolores de supresión de Grafana reflejan los trechos PromQL abajo, dando como equipos de visibilidad de planta ao vivo antes de que precisamente recorrer a consultas manuales.

### Recibe PromQL para revisar la supresión

Los operadores deben mantener los siguientes ayudantes de PromQL a mao; ambos sao referenciados en el tablero Grafana compartido (`dashboards/grafana/soranet_privacy_metrics.json`) y en las registros de Alertmanager:

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

Utilice una dicha proporción para confirmar que la estadística **"Suppressed Bucket %"** permanece abajo del presupuesto de política; Conecte el detector de picos a Alertmanager para obtener retroalimentación rápida cuando un contagio de contribuyentes cair inesperadamente.

### CLI de relatorio de bucket sin conexiónEl espacio de trabajo muestra `cargo xtask soranet-privacy-report` para capturar NDJSON pontuais. Aponte para um ou mais exports admin de retransmisión:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

El asistente pasa la captura del pelo `SoranetSecureAggregator`, imprime un resumen de supresión en la salida estándar y, opcionalmente, graba un archivo JSON estructurado a través de `--json-out <path|->`. Ele honra los mesmos mandos del coleccionista ao vivo (`--bucket-secs`, `--min-contributors`, `--expected-shares`, etc.), permitiendo que los operadores reproduzcan capturas históricas de umbrales de sollozo diferentes ao triar um incidente. Anexe el JSON junto con las capturas de Grafana para que la puerta de análisis de supresión SNNet-8 permanezca auditada.

### Lista de verificación de la primera ejecución automática

Agobernanza ainda exige provar que a primeira execucao automatizada atendeu ao presupuesto de supressao. O helper agora aceita `--max-suppression-ratio <0-1>` para que CI o los operadores falhem rápidamente cuando los cubos suprimidos exceden a janela permitida (predeterminado 10%) o cuando aún no hay cubos de carga. Flujo recomendado:

1. Exporte NDJSON dos administradores de puntos finales para retransmitir más o transmitir `/v2/soranet/privacy/event|share` al orquestador para `artifacts/sorafs_privacy/<relay>.ndjson`.
2. Rode o helper com o presupuesto de politica:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```El comando imprime a razao observada e sai com código nao zero quando o presupuesto e excedido **ou** quando ainda nao ha buckets prontos, sinalizando que a telemetria ainda nao foi produzida para a execucao. Como métricas en vivo deben mostrar `soranet_privacy_pending_collectors` drenando para cero e `soranet_privacy_snapshot_suppression_ratio` ficando abaixo do mesmo presupuesto en cuanto a ejecución ocorre.
3. Archive dicho JSON y el registro de la CLI con el paquete de evidencias SNNet-8 antes de trocar o por defecto el transporte para que los revisores puedan reproducir los artefatos exatos.

## Próximos pasos (SNNet-8a)

- Integrar los colectores duales Prio, conectando la ingesta de acciones al tiempo de ejecución para que los relés y los colectores emitan cargas útiles `SoranetPrivacyBucketMetricsV1` consistentes. *(Concluido - veja `ingest_privacy_payload` en `crates/sorafs_orchestrator/src/lib.rs` y os testes associados.)*
- Publicar o Dashboard Prometheus compartilhado e regras de alerta cobrindo lagunas de supressao, saude dos coleccionistas y quedas de anonimato. *(Concluido - veja `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` y accesorios de validación.)*
- Produzir os artefactos de calibración de privacidade diferenciales descritos en `privacy_metrics_dp.md`, incluidos cuadernos reproducidos y resúmenes de gobierno. *(Concluido - notebook y artefactos generados por `scripts/telemetry/run_privacy_dp.py`; wrapper CI `scripts/telemetry/run_privacy_dp_notebook.sh` ejecuta el notebook vía el flujo de trabajo `.github/workflows/release-pipeline.yml`; resumen de gobierno archivado en `docs/source/status/soranet_privacy_dp_digest.md`.)*Un lanzamiento actual entrega a base de SNNet-8: telemetría determinística y segura para privacidade que se encaixa directamente en los raspadores y paneles de control Prometheus existentes. Los artefatos de calibración de privacidade diferenciale estao no lugar, o flujo de trabajo de liberación de canalización mantem os salidas do notebook actualizados, y el trabajo restante se centra en el monitoreo de la primera ejecución automatizada y en la extensión de los análisis de alerta de supresión.