---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan de observabilidad
título: Plano de observabilidade e SLO da SoraFS
sidebar_label: Observabilidade y SLO
descripción: Esquema de telemetría, paneles de control y política de presupuesto de error para gateways SoraFS, nos e o orquestador multi-source.
---

:::nota Fuente canónica
Esta página espelha o plano mantido en `docs/source/sorafs_observability_plan.md`. Mantenha ambas como copias sincronizadas.
:::

## Objetivos
- Definir métricas y eventos estructurados para gateways, nos y o orquestador multi-source.
- Fornecer tableros Grafana, limiares de alerta y ganchos de validación.
- Estabelecer objetivos SLO junto com políticas de presupuesto de error e ejercicios de caos.

## Catálogo de métricas

### Superficies de la puerta de enlace| Métrica | Tipo | Etiquetas | Notas |
|---------|------|--------|-------|
| `sorafs_gateway_active` | Medidor (ContadorArribaAbajo) | `endpoint`, `method`, `variant`, `chunker`, `profile` | Emitido vía `SorafsGatewayOtel`; Rastrea las operaciones HTTP en una combinación de punto final/método. |
| `sorafs_gateway_responses_total` | Mostrador | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Cada solicitud completada do gateway incrementa una vez; `result` en {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histograma | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Latencia de tiempo hasta el primer byte para respuestas a la puerta de enlace; exportada como Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Mostrador | `profile_version`, `result`, `error_code` | Resultados de verificacao de provas capturadas no momento da solicitacao (`result` in {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histograma | `profile_version`, `result`, `error_code` | Distribución de latencia de verificación para recibos PoR. || `telemetry::sorafs.gateway.request` | Evento estructurado | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Log estructurado emitido para concluir cada solicitud para correlacionar en Loki/Tempo. |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Mostrador | Conjuntos de etiquetas alternativas | Métricas Prometheus mantidas para tableros históricos; emitidas junto con una nueva serie OTLP. |

Eventos `telemetry::sorafs.gateway.request` espelham os contadores OTEL com payloads estruturados, expondo `endpoint`, `method`, `variant`, `status`, `error_code` e `duration_ms` para correlacionar en Loki/Tempo, mientras los paneles contienen una serie OTLP para acompañamiento de SLO.

### Telemetría de saude das provas| Métrica | Tipo | Etiquetas | Notas |
|---------|------|--------|-------|
| `torii_sorafs_proof_health_alerts_total` | Mostrador | `provider_id`, `trigger`, `penalty` | Incrementa hoy que `RecordCapacityTelemetry` emite un `SorafsProofHealthAlert`. `trigger` distingue falhas PDP/PoTR/Both, mientras que `penalty` captura se o colateral foi realmente cortado o suprimido por cooldown. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Calibre | `provider_id` | Contagens mais recientes de PDP/PoTR relacionados dentro de la janela de telemetría infratora para que as equipes quantifiquem o quanto os provedores ultrapassaram a politica. |
| `torii_sorafs_proof_health_penalty_nano` | Calibre | `provider_id` | Valor Nano-XOR cortado en la última alerta (cero cuando el tiempo de reutilización suprime la aplicación). |
| `torii_sorafs_proof_health_cooldown` | Calibre | `provider_id` | Gauge booleano (`1` = alerta suprimido por cooldown) para mostrar cuando alertas de acompañamiento están temporalmente silenciados. |
| `torii_sorafs_proof_health_window_end_epoch` | Calibre | `provider_id` | Época registrada para a janela de telemetria ligada ao alerta para que os operadores correlacionem com artefatos Norito. |

Esses feeds agora alimentam a linha de saude das provas do Dashboard Taikai Viewer
(`dashboards/grafana/taikai_viewer.json`), dando a los operadores CDN visibilidad en tiempo real
sobre volúmenes de alertas, mezcla de activadores PDP/PoTR, penalidades y estado de enfriamiento por
proveedor.Como mesmas métricas ahora sostienen dos registros de alerta del visor Taikai:
`SorafsProofHealthPenalty` dispara cuando
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` aumenta
nos ultimos 15 minutos, mientras `SorafsProofHealthCooldown` emite un aviso se um
El proveedor permanecerá en tiempo de reutilización durante cinco minutos. Ambos os alertas viven en
`dashboards/alerts/taikai_viewer_rules.yml` para que los SRE reciban el contexto inmediatamente
Cuando la aplicación PoR/PoTR se intensifica.

### Superficies del orquestador| Métrica / Evento | Tipo | Etiquetas | Productor | Notas |
|------------------|------|--------|----------|-------|
| `sorafs_orchestrator_active_fetches` | Calibre | `manifest_id`, `region` | `FetchMetricsCtx` | Sessoes atualmente em voo. |
| `sorafs_orchestrator_fetch_duration_ms` | Histograma | `manifest_id`, `region` | `FetchMetricsCtx` | Histograma de duracao em milissegundos; cubos de 1 ms a 30 s. |
| `sorafs_orchestrator_fetch_failures_total` | Mostrador | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Razoes: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Mostrador | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Distinga causas de reintento (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Mostrador | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Captura desabilitacao e contagens de falhas no nivel de sesión. |
| `sorafs_orchestrator_chunk_latency_ms` | Histograma | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Distribución de latencia de recuperación por fragmento (ms) para analizar el rendimiento/SLO. |
| `sorafs_orchestrator_bytes_total` | Mostrador | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Bytes entregues por manifiesto/proveedor; derivar el rendimiento a través de `rate()` en PromQL. |
| `sorafs_orchestrator_stalls_total` | Mostrador | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Conta trozos que exceden `ScoreboardConfig::latency_cap_ms`. || `telemetry::sorafs.fetch.lifecycle` | Evento estructurado | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Espelha o ciclo de vida do job (inicio/completo) con carga útil JSON Norito. |
| `telemetry::sorafs.fetch.retry` | Evento estructurado | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Emitido por racha de reintentos por provedor; `attempts` el contador reintenta incrementalmente (>= 1). |
| `telemetry::sorafs.fetch.provider_failure` | Evento estructurado | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Publicado quando um provedor cruza o limite de falhas. |
| `telemetry::sorafs.fetch.error` | Evento estructurado | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Registro de falha terminal, amigavel para ingestao Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | Evento estructurado | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Emitido cuando a latencia de trozo ultrapassa o límite configurado (espelha contadores de parada). |

### Superficies de no / replicacao| Métrica | Tipo | Etiquetas | Notas |
|---------|------|--------|-------|
| `sorafs_node_capacity_utilisation_pct` | Histograma | `provider_id` | Histograma OTEL da porcentaje de utilización de almacenamiento (exportado como `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Mostrador | `provider_id` | Contador monótono de demostraciones PoR bem-sucedidas, derivado de instantáneas del planificador. |
| `sorafs_node_por_failure_total` | Mostrador | `provider_id` | Contador monotono de amostras PoR com falha. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Calibre | `provider` | Gauges Prometheus existentes para bytes usados, profundidad de fila, contagens PoR em voo. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Calibre | `provider` | Dados de capacidad/uptime bem-sucedidos do provedor exibidos no panel de capacidade. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Calibre | `provider`, `manifest` | Profundidade do backlog mais contadores acumulados de falhas exportados siempre que `/v2/sorafs/por/ingestion/{manifest}` e consultado, alimentando o Painel/alerta "PoR Stalls". |

### Prueba de recuperación oportuna (PoTR) y SLA de fragmentos| Métrica | Tipo | Etiquetas | Productor | Notas |
|---------|------|--------|----------|-------|
| `sorafs_potr_deadline_ms` | Histograma | `tier`, `provider` | Coordinador PoTR | Folga do date date em milissegundos (positivo = atendido). |
| `sorafs_potr_failures_total` | Mostrador | `tier`, `provider`, `reason` | Coordinador PoTR | Razoes: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Mostrador | `provider`, `manifest_id`, `reason` | Monitor de SLA | Disparado quando a entrega de chunks falha no SLO (latencia, taxa de sucesso). |
| `sorafs_chunk_sla_violation_active` | Calibre | `provider`, `manifest_id` | Monitor de SLA | Calibre booleano (0/1) alternado durante una janela de violacao activa. |

## Objetivos SLO

- Disponibilidade trustless del gateway: **99.9%** (respostas HTTP 2xx/304).
- TTFB P95 sin confianza: nivel activo = 99,5% por día.
- Sucesso do orquestador (conclusao de chunks): >= 99%.

## Paneles y alertas1. **Observabilidade do gateway** (`dashboards/grafana/sorafs_gateway_observability.json`) - acompanha disponibilidade trustless, TTFB P95, detalle de recusas y falhas PoR/PoTR vía métricas OTEL.
2. **Saude do orquestrador** (`dashboards/grafana/sorafs_fetch_observability.json`) - carga de cobre multifuente, reintentos, falhas de provedores e rajadas de puestos.
3. **Metricas de privacidade SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) - gráficos de cubos de relé anonimizados, janelas de supressao e saude do colector vía `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` e `soranet_privacy_poll_errors_total{provider}`.

Paquetes de alertas:

- `dashboards/alerts/sorafs_gateway_rules.yml` - disponibilidad de gateway, TTFB, picos de falha de pruebas.
- `dashboards/alerts/sorafs_fetch_rules.yml` - falhas/retrys/stalls do orquestrador; validado a través de `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` e `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` - picos de degradación de privacidad, alarmas de supresión, detección de colector ocioso y alertas de colector desactivado (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` - alarmas de apagón de anonimato ligados a `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` - alarmas de deriva/ingesta/CEK lag del visor Taikai más nuevas alertas de penalidad/enfriamiento de saude das pruebas SoraFS alimentadas por `torii_sorafs_proof_health_*`.

## Estrategia de rastreo- Adote OpenTelemetry de ponta a ponta:
  - Las puertas de enlace emiten tramos OTLP (HTTP) anotados con ID de solicitud, resúmenes de manifiesto y hashes de token.
  - El orquestador usa `tracing` + `opentelemetry` para exportar spans de tentativas de fetch.
  - Nos SoraFS embutidos exportam spans para desafios PoR e operacoes de Storage. Todos los componentes comparten un ID de seguimiento propagado a través de `x-sorafs-trace`.
- `SorafsFetchOtel` liga las métricas del orquestador a histogramas OTLP en cuanto eventos `telemetry::sorafs.fetch.*` necesita cargas útiles niveles JSON para backends centrados en registros.
- Coleccionistas: ejecutar coleccionistas OTEL al lado de Prometheus/Loki/Tempo (Tempo preferido). Exportadores compatibles con Jaeger permanecen opcionalmente.
- Operacoes de alta cardinalidade devem ser amostradas (10% para caminhos de sucesso, 100% para falhas).

## Coordinación de telemetría TLS (SF-5b)- Ajuste de métricas:
  - Un TLS automático envia `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` e `sorafs_gateway_tls_ech_enabled`.
  - Incluye medidores sin tablero, descripción general de la puerta de enlace o TLS/certificados.
- Vínculo de alertas:
  - Quando alertas de expiracao TLS dispararem (<= 14 días restantes), correlacione com o SLO de disponibilidade trustless.
  - Una desactivación de ECH emite una alerta secundaria referenciando tanto los dolores de cabeza de TLS como de disponibilidad.
- Pipeline: el trabajo de exportación automática TLS para una mesma stack Prometheus de las métricas del gateway; a coordenacao com SF-5b garante instrumentacao deduplicada.

## Convenios de nombres y etiquetas de métricas- Nombres de métricas según los prefijos existentes `torii_sorafs_*` o `sorafs_*` usados ​​por Torii y la puerta de enlace.
- Conjuntos de etiquetas sao padronizados:
  - `result` -> resultado HTTP (`success`, `refused`, `failed`).
  - `reason` -> código de recusa/error (`unsupported_chunker`, `timeout`, etc.).
  - `provider` -> identificador de proveedor codificado en hexadecimal.
  - `manifest` -> digest canonico do manifest (cortado quando a cardinalidade e alta).
  - `tier` -> etiquetas declarativas de nivel (`hot`, `warm`, `archive`).
- Puntos de emisión de telemetría:
  - Las métricas de la puerta de enlace viven con `torii_sorafs_*` y reutilizan las convenciones de `crates/iroha_core/src/telemetry.rs`.
  - El orquestador emite métricas `sorafs_orchestrator_*` y eventos `telemetry::sorafs.fetch.*` (ciclo de vida, reintento, falla del proveedor, error, parada) etiquetas con resumen de manifiesto, ID de trabajo, región e identificadores de proveedor.
  - Nos exponen `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` e `torii_sorafs_por_*`.
- Coordene com Observabilidad para registrar o catalogo de métricas no documento compartilhado de nomes Prometheus, incluyendo expectativas de cardinalidade de etiquetas (limites superiores de provedor/manifests).

## Tubería de dados- Collectors sao implantados junto a cada componente, exportando OTLP para Prometheus (metricas) y Loki/Tempo (logs/traces).
- eBPF opcional (Tetragon) enriquece el rastreo de bajo nivel para gateways/nos.
- Use `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` para Torii e nos embutidos; o orquestador continuo chamando `install_sorafs_fetch_otlp_exporter`.

## Ganchos de validación

- Ejecute `scripts/telemetry/test_sorafs_fetch_alerts.sh` durante CI para garantizar que reggas de alerta Prometheus permanecam em sincronia con métricas de bloqueo y comprobaciones de supresión de privacidad.
- Mantiene los paneles de control Grafana, controla el versao (`dashboards/grafana/`) y actualiza capturas de pantalla/enlaces cuando los cambios cambian.
- Drills de caos registram resultados vía `scripts/telemetry/log_sorafs_drill.sh`; a validacao usa `scripts/telemetry/validate_drill_log.sh` (veja o [Playbook de operacoes](operations-playbook.md)).