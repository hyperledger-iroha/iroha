---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: observability-plan
title: Plano de observabilidade e SLO da SoraFS
sidebar_label: Observabilidade e SLOs
description: Schema de telemetria, dashboards e politica de budget de erro para gateways SoraFS, nos e o orquestrador multi-source.
---

:::note Fonte canonica
Esta pagina espelha o plano mantido em `docs/source/sorafs_observability_plan.md`. Mantenha ambas as copias sincronizadas.
:::

## Objetivos
- Definir metricas e eventos estruturados para gateways, nos e o orquestrador multi-source.
- Fornecer dashboards Grafana, limiares de alerta e hooks de validacao.
- Estabelecer objetivos SLO junto com politicas de budget de erro e drills de caos.

## Catalogo de metricas

### Superficies do gateway

| Metrica | Tipo | Labels | Notas |
|---------|------|--------|-------|
| `sorafs_gateway_active` | Gauge (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | Emitido via `SorafsGatewayOtel`; rastreia operacoes HTTP em voo por combinacao de endpoint/metodo. |
| `sorafs_gateway_responses_total` | Counter | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Cada solicitacao completada do gateway incrementa uma vez; `result` in {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histogram | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Latencia de time-to-first-byte para respostas do gateway; exportada como Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Counter | `profile_version`, `result`, `error_code` | Resultados de verificacao de provas capturados no momento da solicitacao (`result` in {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histogram | `profile_version`, `result`, `error_code` | Distribuicao de latencia de verificacao para recibos PoR. |
| `telemetry::sorafs.gateway.request` | Evento estruturado | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Log estruturado emitido ao concluir cada solicitacao para correlacao em Loki/Tempo. |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Counter | Conjuntos de labels alternativos | Metricas Prometheus mantidas para dashboards historicos; emitidas junto com a nova serie OTLP. |

Eventos `telemetry::sorafs.gateway.request` espelham os contadores OTEL com payloads estruturados, expondo `endpoint`, `method`, `variant`, `status`, `error_code` e `duration_ms` para correlacao em Loki/Tempo, enquanto os dashboards consomem a serie OTLP para acompanhamento de SLO.

### Telemetria de saude das provas

| Metrica | Tipo | Labels | Notas |
|---------|------|--------|-------|
| `torii_sorafs_proof_health_alerts_total` | Counter | `provider_id`, `trigger`, `penalty` | Incrementa toda vez que `RecordCapacityTelemetry` emite um `SorafsProofHealthAlert`. `trigger` distingue falhas PDP/PoTR/Both, enquanto `penalty` captura se o colateral foi realmente cortado ou suprimido por cooldown. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Gauge | `provider_id` | Contagens mais recentes de PDP/PoTR relatadas dentro da janela de telemetria infratora para que as equipes quantifiquem o quanto os provedores ultrapassaram a politica. |
| `torii_sorafs_proof_health_penalty_nano` | Gauge | `provider_id` | Valor Nano-XOR cortado no ultimo alerta (zero quando o cooldown suprimiu a aplicacao). |
| `torii_sorafs_proof_health_cooldown` | Gauge | `provider_id` | Gauge booleano (`1` = alerta suprimido por cooldown) para mostrar quando alertas de acompanhamento estao temporariamente silenciados. |
| `torii_sorafs_proof_health_window_end_epoch` | Gauge | `provider_id` | Epoca registrada para a janela de telemetria ligada ao alerta para que os operadores correlacionem com artefatos Norito. |

Esses feeds agora alimentam a linha de saude das provas do dashboard Taikai viewer
(`dashboards/grafana/taikai_viewer.json`), dando aos operadores CDN visibilidade em tempo real
sobre volumes de alertas, mix de triggers PDP/PoTR, penalidades e estado de cooldown por
provedor.

As mesmas metricas agora sustentam duas regras de alerta do Taikai viewer:
`SorafsProofHealthPenalty` dispara quando
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` aumenta
nos ultimos 15 minutos, enquanto `SorafsProofHealthCooldown` emite um aviso se um
provedor permanecer em cooldown por cinco minutos. Ambos os alertas vivem em
`dashboards/alerts/taikai_viewer_rules.yml` para que os SREs recebam contexto imediato
quando a aplicacao PoR/PoTR se intensifica.

### Superficies do orquestrador

| Metrica / Evento | Tipo | Labels | Produtor | Notas |
|------------------|------|--------|----------|-------|
| `sorafs_orchestrator_active_fetches` | Gauge | `manifest_id`, `region` | `FetchMetricsCtx` | Sessoes atualmente em voo. |
| `sorafs_orchestrator_fetch_duration_ms` | Histogram | `manifest_id`, `region` | `FetchMetricsCtx` | Histograma de duracao em milissegundos; buckets de 1 ms a 30 s. |
| `sorafs_orchestrator_fetch_failures_total` | Counter | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Razoes: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Counter | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Distingue causas de retry (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Counter | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Captura desabilitacao e contagens de falhas no nivel de sessao. |
| `sorafs_orchestrator_chunk_latency_ms` | Histogram | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Distribuicao de latencia de fetch por chunk (ms) para analise de throughput/SLO. |
| `sorafs_orchestrator_bytes_total` | Counter | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Bytes entregues por manifest/provedor; derive o throughput via `rate()` em PromQL. |
| `sorafs_orchestrator_stalls_total` | Counter | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Conta chunks que excedem `ScoreboardConfig::latency_cap_ms`. |
| `telemetry::sorafs.fetch.lifecycle` | Evento estruturado | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Espelha o ciclo de vida do job (start/complete) com payload JSON Norito. |
| `telemetry::sorafs.fetch.retry` | Evento estruturado | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Emitido por streak de retries por provedor; `attempts` conta retries incrementais (>= 1). |
| `telemetry::sorafs.fetch.provider_failure` | Evento estruturado | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Publicado quando um provedor cruza o limite de falhas. |
| `telemetry::sorafs.fetch.error` | Evento estruturado | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Registro de falha terminal, amigavel para ingestao Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | Evento estruturado | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Emitido quando a latencia de chunk ultrapassa o limite configurado (espelha contadores de stall). |

### Superficies de no / replicacao

| Metrica | Tipo | Labels | Notas |
|---------|------|--------|-------|
| `sorafs_node_capacity_utilisation_pct` | Histogram | `provider_id` | Histograma OTEL da porcentagem de utilizacao de storage (exportado como `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Counter | `provider_id` | Contador monotono de amostras PoR bem-sucedidas, derivado de snapshots do scheduler. |
| `sorafs_node_por_failure_total` | Counter | `provider_id` | Contador monotono de amostras PoR com falha. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Gauge | `provider` | Gauges Prometheus existentes para bytes usados, profundidade de fila, contagens PoR em voo. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Gauge | `provider` | Dados de capacidade/uptime bem-sucedidos do provedor exibidos no dashboard de capacidade. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Gauge | `provider`, `manifest` | Profundidade do backlog mais contadores acumulados de falhas exportados sempre que `/v1/sorafs/por/ingestion/{manifest}` e consultado, alimentando o painel/alerta "PoR Stalls". |

### Proof of Timely Retrieval (PoTR) e SLA de chunks

| Metrica | Tipo | Labels | Produtor | Notas |
|---------|------|--------|----------|-------|
| `sorafs_potr_deadline_ms` | Histogram | `tier`, `provider` | Coordenador PoTR | Folga do deadline em milissegundos (positivo = atendido). |
| `sorafs_potr_failures_total` | Counter | `tier`, `provider`, `reason` | Coordenador PoTR | Razoes: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Counter | `provider`, `manifest_id`, `reason` | Monitor de SLA | Disparado quando a entrega de chunks falha no SLO (latencia, taxa de sucesso). |
| `sorafs_chunk_sla_violation_active` | Gauge | `provider`, `manifest_id` | Monitor de SLA | Gauge booleano (0/1) alternado durante a janela de violacao ativa. |

## Objetivos SLO

- Disponibilidade trustless do gateway: **99.9%** (respostas HTTP 2xx/304).
- TTFB P95 trustless: hot tier <= 120 ms, warm tier <= 300 ms.
- Taxa de sucesso de provas: >= 99.5% por dia.
- Sucesso do orquestrador (conclusao de chunks): >= 99%.

## Dashboards e alertas

1. **Observabilidade do gateway** (`dashboards/grafana/sorafs_gateway_observability.json`) - acompanha disponibilidade trustless, TTFB P95, detalhamento de recusas e falhas PoR/PoTR via metricas OTEL.
2. **Saude do orquestrador** (`dashboards/grafana/sorafs_fetch_observability.json`) - cobre carga multi-source, retries, falhas de provedores e rajadas de stalls.
3. **Metricas de privacidade SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) - graficos de buckets de relay anonimizados, janelas de supressao e saude do collector via `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` e `soranet_privacy_poll_errors_total{provider}`.

Pacotes de alertas:

- `dashboards/alerts/sorafs_gateway_rules.yml` - disponibilidade do gateway, TTFB, picos de falha de provas.
- `dashboards/alerts/sorafs_fetch_rules.yml` - falhas/retries/stalls do orquestrador; validado via `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` e `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` - picos de degradacao de privacidade, alarmes de supressao, deteccao de collector ocioso e alertas de collector desabilitado (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` - alarmes de brownout de anonimato ligados a `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` - alarmes de drift/ingest/CEK lag do Taikai viewer mais os novos alertas de penalidade/cooldown de saude das provas SoraFS alimentados por `torii_sorafs_proof_health_*`.

## Estrategia de tracing

- Adote OpenTelemetry de ponta a ponta:
  - Gateways emitem spans OTLP (HTTP) anotados com IDs de solicitacao, digests de manifest e hashes de token.
  - O orquestrador usa `tracing` + `opentelemetry` para exportar spans de tentativas de fetch.
  - Nos SoraFS embutidos exportam spans para desafios PoR e operacoes de storage. Todos os componentes compartilham um trace ID comum propagado via `x-sorafs-trace`.
- `SorafsFetchOtel` liga metricas do orquestrador a histogramas OTLP enquanto eventos `telemetry::sorafs.fetch.*` fornecem payloads JSON leves para backends centrados em logs.
- Collectors: execute collectors OTEL ao lado de Prometheus/Loki/Tempo (Tempo preferido). Exportadores compatveis com Jaeger permanecem opcionais.
- Operacoes de alta cardinalidade devem ser amostradas (10% para caminhos de sucesso, 100% para falhas).

## Coordenacao de telemetria TLS (SF-5b)

- Alinhamento de metricas:
  - A automacao TLS envia `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` e `sorafs_gateway_tls_ech_enabled`.
  - Inclua esses gauges no dashboard Gateway Overview sob o painel TLS/Certificates.
- Vinculo de alertas:
  - Quando alertas de expiracao TLS dispararem (<= 14 dias restantes), correlacione com o SLO de disponibilidade trustless.
  - A desativacao de ECH emite um alerta secundario referenciando tanto os paineis TLS quanto os de disponibilidade.
- Pipeline: o job de automacao TLS exporta para a mesma stack Prometheus das metricas do gateway; a coordenacao com SF-5b garante instrumentacao deduplicada.

## Convencoes de nomes e labels de metricas

- Nomes de metricas seguem os prefixos existentes `torii_sorafs_*` ou `sorafs_*` usados por Torii e o gateway.
- Conjuntos de labels sao padronizados:
  - `result` -> resultado HTTP (`success`, `refused`, `failed`).
  - `reason` -> codigo de recusa/erro (`unsupported_chunker`, `timeout`, etc.).
  - `provider` -> identificador de provedor codificado em hex.
  - `manifest` -> digest canonico do manifest (cortado quando a cardinalidade e alta).
  - `tier` -> labels declarativas de tier (`hot`, `warm`, `archive`).
- Pontos de emissao de telemetria:
  - Metricas do gateway vivem sob `torii_sorafs_*` e reutilizam convencoes de `crates/iroha_core/src/telemetry.rs`.
  - O orquestrador emite metricas `sorafs_orchestrator_*` e eventos `telemetry::sorafs.fetch.*` (lifecycle, retry, provider failure, error, stall) etiquetados com digest de manifest, job ID, regiao e identificadores de provedor.
  - Nos expoem `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` e `torii_sorafs_por_*`.
- Coordene com Observability para registrar o catalogo de metricas no documento compartilhado de nomes Prometheus, incluindo expectativas de cardinalidade de labels (limites superiores de provedor/manifests).

## Pipeline de dados

- Collectors sao implantados junto a cada componente, exportando OTLP para Prometheus (metricas) e Loki/Tempo (logs/traces).
- eBPF opcional (Tetragon) enriquece tracing de baixo nivel para gateways/nos.
- Use `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` para Torii e nos embutidos; o orquestrador continua chamando `install_sorafs_fetch_otlp_exporter`.

## Hooks de validacao

- Execute `scripts/telemetry/test_sorafs_fetch_alerts.sh` durante CI para garantir que regras de alerta Prometheus permanecam em sincronia com metricas de stall e checks de supressao de privacidade.
- Mantenha dashboards Grafana sob controle de versao (`dashboards/grafana/`) e atualize screenshots/links quando os paineis mudarem.
- Drills de caos registram resultados via `scripts/telemetry/log_sorafs_drill.sh`; a validacao usa `scripts/telemetry/validate_drill_log.sh` (veja o [Playbook de operacoes](operations-playbook.md)).
