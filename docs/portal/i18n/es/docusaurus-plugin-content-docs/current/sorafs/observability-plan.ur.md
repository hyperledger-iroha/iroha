---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan de observabilidad
título: SoraFS Observabilidad por SLO پلان
sidebar_label: Observabilidad y SLO
descripción: Puertas de enlace SoraFS, nodos, orquestador de múltiples fuentes, esquemas de telemetría, paneles y política de presupuesto de errores.
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sorafs_observability_plan.md` میں برقرار رکھے گئے منصوبے کی عکاسی کرتا ہے۔ جب تک پرانا Sphinx سیٹ مکمل طور پر منتقل نہ ہو جائے دونوں نقول کو ہم آہنگ رکھیں۔
:::

## Objetivos
- puertas de enlace, nodos, orquestador de múltiples fuentes, métricas, eventos estructurados, etc.
- Paneles de control Grafana, umbrales de alerta y ganchos de validación.
- presupuesto de errores y políticas de simulacro de caos کے ساتھ Objetivos de SLO قائم کریں۔

## Catálogo de métricas

### Superficies de puerta de enlace| Métrica | Tipo | Etiquetas | Notas |
|--------|------|--------|-------|
| `sorafs_gateway_active` | Medidor (ContadorArribaAbajo) | `endpoint`, `method`, `variant`, `chunker`, `profile` | `SorafsGatewayOtel` کے ذریعے emite ہوتا ہے؛ ہر punto final/método کمبینیشن کے لیے operaciones HTTP en vuelo ٹریک کرتا ہے۔ |
| `sorafs_gateway_responses_total` | Mostrador | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | ہر مکمل solicitud de puerta de enlace ایک بار incremento ہوتی ہے؛ `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histograma | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | respuestas de la puerta de enlace کے لیے latencia de tiempo hasta el primer byte؛ Prometheus `_bucket/_sum/_count` کے طور پر export۔ |
| `sorafs_gateway_proof_verifications_total` | Mostrador | `profile_version`, `result`, `error_code` | tiempo de solicitud پر resultados de verificación de prueba captura کیے جاتے ہیں (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histograma | `profile_version`, `result`, `error_code` | Recibos de PoR کے لیے distribución de latencia de verificación۔ || `telemetry::sorafs.gateway.request` | Evento estructurado | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | ہر finalización de solicitud پر emisión de registro estructurado ہوتا ہے تاکہ Correlación Loki/Tempo ہو سکے۔ |

`telemetry::sorafs.gateway.request` eventos Contadores OTEL کو cargas útiles estructuradas کے ساتھ espejo کرتے ہیں، Correlación Loki/Tempo کے لیے `endpoint`, `method`, `variant`, `status`, `error_code` y `duration_ms` Paneles de control de seguimiento SLO de la serie OTLP استعمال کرتے ہیں۔

### Telemetría de prueba de salud| Métrica | Tipo | Etiquetas | Notas |
|--------|------|--------|-------|
| `torii_sorafs_proof_health_alerts_total` | Mostrador | `provider_id`, `trigger`, `penalty` | جب بھی `RecordCapacityTelemetry` ایک `SorafsProofHealthAlert` emite کرے تو incremento ہوتا ہے۔ `trigger` PDP/PoTR/Ambas fallas فرق کرتا ہے، جبکہ `penalty` دکھاتا ہے کہ colateral واقعی slash ہوا یا cooldown نے suprimir کیا۔ |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Calibre | `provider_id` | ventana de telemetría infractora میں رپورٹ ہونے والی تازہ PDP/PoTR cuenta تاکہ ٹیمیں جان سکیں کہ proveedores نے política سے کتنا تجاوز کیا۔ |
| `torii_sorafs_proof_health_penalty_nano` | Calibre | `provider_id` | آخری alerta پر slash ہونے والی Nano-XOR مقدار (enfriamiento y aplicación de la supresión کیا تو صفر). |
| `torii_sorafs_proof_health_cooldown` | Calibre | `provider_id` | Indicador booleano (`1` = alerta de enfriamiento نے suprimir کیا) تاکہ alertas de seguimiento وقتی طور پر silenciar ہونے پر دکھایا جا سکے۔ |
| `torii_sorafs_proof_health_window_end_epoch` | Calibre | `provider_id` | alerta سے منسلک ventana de telemetría کا época تاکہ operadores Norito artefactos سے correlación کر سکیں۔ |

یہ feeds اب Panel de visualización de Taikai کی fila de prueba de estado کو چلاتے ہیں
(`dashboards/grafana/taikai_viewer.json`), operadores CDN, volúmenes de alerta, combinación de activadores PDP/PoTR, penalizaciones, estado de enfriamiento, proveedor, visibilidad en vivo.یہی métricas del visor Taikai کے دو reglas de alerta کو سپورٹ کرتے ہیں:
`SorafsProofHealthPenalty` اس وقت fuego ہوتا ہے جب
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` Mیں
گزشتہ 15 منٹ میں اضافہ ہو، جبکہ `SorafsProofHealthCooldown` advertencia دیتا ہے اگر
proveedor پانچ منٹ تک cooldown میں رہے۔ alertas de دونوں
`dashboards/alerts/taikai_viewer_rules.yml` Configuración de configuración SRes y PoR/PoTR
aplicación de la ley بڑھنے پر فوری contexto ملے۔

### Superficies del orquestador| Métrica/Evento | Tipo | Etiquetas | Productor | Notas |
|----------------|------|--------|----------|-------|
| `sorafs_orchestrator_active_fetches` | Calibre | `manifest_id`, `region` | `FetchMetricsCtx` | موجودہ sesiones a bordo۔ |
| `sorafs_orchestrator_fetch_duration_ms` | Histograma | `manifest_id`, `region` | `FetchMetricsCtx` | histograma de duración (milisegundos)؛ 1 ms سے 30 s cubos ۔ |
| `sorafs_orchestrator_fetch_failures_total` | Mostrador | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Motivos: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Mostrador | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | el reintento causa فرق کرتا ہے (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Mostrador | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Captura de recuentos de fallos/inhabilitación a nivel de sesión کرتا ہے۔ |
| `sorafs_orchestrator_chunk_latency_ms` | Histograma | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Rendimiento de distribución de latencia de recuperación por fragmento (ms)/análisis de SLO کے لیے۔ |
| `sorafs_orchestrator_bytes_total` | Mostrador | `manifest_id`, `provider_id` | `FetchMetricsCtx` | manifiesto/proveedor کے حساب سے bytes entregados؛ PromQL میں `rate()` کے ذریعے rendimiento نکالیں۔ |
| `sorafs_orchestrator_stalls_total` | Mostrador | `manifest_id`, `provider_id` | `FetchMetricsCtx` | `ScoreboardConfig::latency_cap_ms` سے تجاوز کرنے والے trozos گنتا ہے۔ || `telemetry::sorafs.fetch.lifecycle` | Evento estructurado | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | ciclo de vida del trabajo (inicio/completo) کو Norito Carga útil JSON کے ساتھ mirror کرتا ہے۔ |
| `telemetry::sorafs.fetch.retry` | Evento estructurado | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Racha de reintentos del proveedor کے لیے emit ہوتا ہے؛ `attempts` reintentos incrementales شمار کرتا ہے (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | Evento estructurado | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | جب umbral de falla del proveedor cruzó کرے تو ظاہر ہوتا ہے۔ |
| `telemetry::sorafs.fetch.error` | Evento estructurado | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | registro de fallas del terminal, ingestión de Loki/Splunk کے لیے مناسب۔ |
| `telemetry::sorafs.fetch.stall` | Evento estructurado | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | límite de latencia configurado سے بڑھنے پر emite ہوتا ہے (contadores de puesto کو espejo کرتا ہے). |

### Superficies de nodo/replicación| Métrica | Tipo | Etiquetas | Notas |
|--------|------|--------|-------|
| `sorafs_node_capacity_utilisation_pct` | Histograma | `provider_id` | porcentaje de utilización del almacenamiento en el histograma de OTEL (`_bucket/_sum/_count` کے طور پر export). |
| `sorafs_node_por_success_total` | Mostrador | `provider_id` | instantáneas del programador سے muestras PoR exitosas derivadas کا contador monótono ۔ |
| `sorafs_node_por_failure_total` | Mostrador | `provider_id` | muestras de PoR fallidas کا contador monótono ۔ |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Calibre | `provider` | bytes utilizados, profundidad de la cola y recuentos de PoR en vuelo, así como medidores Prometheus. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Calibre | `provider` | datos de éxito de capacidad/tiempo de actividad del proveedor, panel de capacidad میں دکھایا جاتا ہے۔ |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Calibre | `provider`, `manifest` | profundidad del trabajo pendiente اور contadores de fallas acumulativas جو ہر `/v2/sorafs/por/ingestion/{manifest}` encuesta پر export ہوتے ہیں، "PoR Stalls" panel/alerta کو feed کرتے ہیں۔ |

### Prueba de recuperación oportuna (PoTR) y SLA de fragmentos| Métrica | Tipo | Etiquetas | Productor | Notas |
|--------|------|--------|----------|-------|
| `sorafs_potr_deadline_ms` | Histograma | `tier`, `provider` | Coordinador del PoTR | fecha límite de retraso milisegundos میں (positivo = cumplido). |
| `sorafs_potr_failures_total` | Mostrador | `tier`, `provider`, `reason` | Coordinador del PoTR | Motivos: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Mostrador | `provider`, `manifest_id`, `reason` | Monitor de SLA | جب entrega de fragmentos SLO miss کرے تو fire ہوتا ہے (latencia, tasa de éxito). |
| `sorafs_chunk_sla_violation_active` | Calibre | `provider`, `manifest_id` | Monitor de SLA | Indicador booleano (0/1) جو ventana de infracción activa میں alternar ہوتا ہے۔ |

## Objetivos de SLO

- Disponibilidad de puerta de enlace sin confianza: **99,9%** (respuestas HTTP 2xx/304).
- Trustless TTFB P95: nivel activo ≤ 120 ms, nivel activo ≤ 300 ms.
- Tasa de éxito de la prueba: ≥ 99,5% por día.
- Éxito del orquestador (finalización del fragmento): ≥ 99%.

## Paneles y alertas1. **Observabilidad de la puerta de enlace** (`dashboards/grafana/sorafs_gateway_observability.json`): disponibilidad sin confianza, TTFB P95, desglose de rechazos, fallas de PoR/PoTR y métricas de OTEL.
2. **Orchestrator Health** (`dashboards/grafana/sorafs_fetch_observability.json`): carga de múltiples fuentes, reintentos, fallas del proveedor y ráfagas de bloqueo کو کور کرتا ہے۔
3. **Métricas de privacidad de SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`): depósitos de retransmisión anonimizados, ventanas de supresión y estado del recopilador en `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` y `soranet_privacy_poll_errors_total{provider}` en el mercado. کرتا ہے۔

Paquetes de alerta:

- `dashboards/alerts/sorafs_gateway_rules.yml` — disponibilidad de puerta de enlace, TTFB, picos de falla a prueba de agua ۔
- `dashboards/alerts/sorafs_fetch_rules.yml` — fallas/reintentos/bloqueos del orquestador؛ `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml`, y `dashboards/alerts/tests/soranet_policy_rules.test.yml` کے ذریعے validar۔
- `dashboards/alerts/soranet_privacy_rules.yml`: picos de degradación de la privacidad, alarmas de supresión, detección de recopiladores inactivos y alertas de recopiladores deshabilitados (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml`: alarmas de caída de tensión anónimas, `sorafs_orchestrator_brownouts_total`, cableadas, ہیں۔
- `dashboards/alerts/taikai_viewer_rules.yml`: alarmas de deriva/ingesta/retraso CEK del visor Taikai کے ساتھ ساتھ نئی SoraFS alertas de penalización/enfriamiento de estado de prueba, جو `torii_sorafs_proof_health_*` سے alimentado ہیں۔

## Estrategia de seguimiento- OpenTelemetry کو de extremo a extremo:
  - Los tramos OTLP de las puertas de enlace (HTTP) emiten ID de solicitud, resúmenes de manifiesto y hashes de tokens.
  - orquestador `tracing` + `opentelemetry` استعمال کر کے intentos de recuperación کے abarca exportación کرتا ہے۔
  - Los nodos PoR integrados SoraFS desafían las operaciones de almacenamiento y abarcan la exportación. Componentes de تمام `x-sorafs-trace` کے ذریعے propagar ہونے والا ID de seguimiento común compartir کرتے ہیں۔
- Métricas del orquestador `SorafsFetchOtel` Histogramas OTLP Puente Puentes `telemetry::sorafs.fetch.*` backends centrados en registros de eventos Cargas útiles JSON ligeras
- Coleccionistas: Coleccionistas de OTEL کو Prometheus/Loki/Tempo کے ساتھ چلائیں (Se prefiere Tempo). Exportadores de API de Jaeger اختیاری رہتے ہیں۔
- Operaciones de alta cardinalidad کو muestra کریں (rutas de éxito کے لیے 10%, fallas کے لیے 100%).

## Coordinación de telemetría TLS (SF-5b)- Alineación métrica:
  - Automatización TLS `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}`, y `sorafs_gateway_tls_ech_enabled` بھیجتی ہے۔
  - ان medidores کو Panel de descripción general de Gateway میں Panel TLS/Certificados کے تحت شامل کریں۔
- Vinculación de alerta:
  - Se activan alertas de caducidad de TLS (quedan ≤ 14 días) y SLO de disponibilidad sin confianza y correlacionan کریں۔
  - Desactivación de ECH ایک alerta secundaria emitir کرتا ہے جو TLS اور disponibilidad دونوں paneles کو referencia کرتا ہے۔
- Pipeline: trabajo de automatización TLS اسی Prometheus stack پر export کرتا ہے جس پر gateway metrics ہیں؛ SF-5b کے ساتھ instrumentación deduplicada de coordinación یقینی بناتی ہے۔

## Convenciones de etiquetas y nombres de métricas- Nombres de métricas موجودہ `torii_sorafs_*` یا `sorafs_*` prefijos کو seguir کرتے ہیں جو Torii اور gateway استعمال کرتے ہیں۔
- Conjuntos de etiquetas estandarizados ہیں:
  - `result` → Resultado HTTP (`success`, `refused`, `failed`).
  - `reason` → código de rechazo/error (`unsupported_chunker`, `timeout`, etc.).
  - `provider` → identificador de proveedor codificado en hexadecimal۔
  - `manifest` → resumen manifiesto canónico (recorte میں de alta cardinalidad کیا جاتا ہے). 
  - `tier` → etiquetas de niveles declarativos (`hot`, `warm`, `archive`).
- Puntos de emisión de telemetría:
  - Métricas de puerta de enlace `torii_sorafs_*` کے تحت رہتے ہیں اور `crates/iroha_core/src/telemetry.rs` کی reutilización de convenciones کرتے ہیں۔
  - Las métricas `sorafs_orchestrator_*` del orquestador y los eventos `telemetry::sorafs.fetch.*` (ciclo de vida, reintento, falla del proveedor, error, bloqueo) emiten کرتا ہے جن پر manifiesto digest, ID de trabajo, región اور identificadores de proveedor etiquetas ہوتے ہیں۔
  - Nodos `torii_sorafs_storage_*`, `torii_sorafs_capacity_*`, y `torii_sorafs_por_*` دکھاتے ہیں۔
- Observabilidad کے ساتھ coordenadas کریں تاکہ catálogo de métricas کو compartido Prometheus documento de nomenclatura میں registro کیا جائے، جس میں etiqueta cardinalidad expectativas (proveedor/manifiestos límites superiores) شامل ہوں۔

## Canalización de datos- Componente de recopiladores, implementación, OTLP, Prometheus (métricas), Loki/Tempo (registros/rastros), exportación
- Puertas de enlace/nodos eBPF (Tetragon) opcionales کے لیے rastreo de bajo nivel کو enriquecer کرتا ہے۔
- `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` y Torii son nodos integrados que funcionan correctamente orquestador `install_sorafs_fetch_otlp_exporter` کو کال کرتا رہتا ہے۔

## Ganchos de validación

- CI کے دوران `scripts/telemetry/test_sorafs_fetch_alerts.sh` چلائیں تاکہ Prometheus reglas de alerta métricas de bloqueo اور comprobaciones de supresión de privacidad کے ساتھ lockstep رہیں۔
- Paneles Grafana Control de versiones (`dashboards/grafana/`) Configuración de paneles Configuración de capturas de pantalla/enlaces Configuración de controles
- Ejercicios de caos کے نتائج `scripts/telemetry/log_sorafs_drill.sh` کے ذریعے log ہوتے ہیں؛ validación `scripts/telemetry/validate_drill_log.sh` استعمال کرتی ہے (دیکھیے [Manual de operaciones](operations-playbook.md)).