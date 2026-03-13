---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de observabilidade
título: Plano de observabilidade e SLO da SoraFS
sidebar_label: Observabilidade e SLOs
description: Esquema de telemetria, dashboards e política de orçamento de erro para gateways SoraFS, nos e o orquestrador multi-source.
---

:::nota Fonte canônica
Esta página reflete o plano mantido em `docs/source/sorafs_observability_plan.md`. Mantenha ambas as cópias sincronizadas.
:::

## Objetivos
- Definir métricas e eventos estruturados para gateways, nos e o orquestrador multi-source.
- Fornecer dashboards Grafana, limites de alerta e ganchos de validação.
- Estabelecer objetivos SLO junto com políticas de erro orçamentário e exercícios de caos.

## Catálogo de métricas

### Superfícies do gateway

| Métrica | Tipo | Etiquetas | Notas |
|--------|------|--------|-------|
| `sorafs_gateway_active` | Medidor (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | Emitido via `SorafsGatewayOtel`; rastreia operações HTTP em voo por combinação de endpoint/método. |
| `sorafs_gateway_responses_total` | Contador | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Cada solicitação completada do gateway é incrementada uma vez; `result` em {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histograma | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Latência de tempo até o primeiro byte para respostas do gateway; exportado como Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Contador | `profile_version`, `result`, `error_code` | Resultados de verificação de provas capturadas no momento da solicitação (`result` em {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histograma | `profile_version`, `result`, `error_code` | Distribuição de latência de verificação para recibos PoR. |
| `telemetry::sorafs.gateway.request` | Evento estruturado | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Log estruturado emitido ao concluir cada solicitação para correlação em Loki/Tempo. |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Contador | Conjuntos de rótulos alternativos | Métricas Prometheus mantidas para dashboards históricos; emissões junto com a nova série OTLP. |

Eventos `telemetry::sorafs.gateway.request` espelham os contadores OTEL com payloads estruturados, expondo `endpoint`, `method`, `variant`, `status`, `error_code` e `duration_ms` para correlação em Loki/Tempo, enquanto os dashboards consomem uma série OTLP para acompanhamento de SLO.

### Telemetria de saúde das provas| Métrica | Tipo | Etiquetas | Notas |
|--------|------|--------|-------|
| `torii_sorafs_proof_health_alerts_total` | Contador | `provider_id`, `trigger`, `penalty` | Aumente cada vez que `RecordCapacityTelemetry` emite um `SorafsProofHealthAlert`. `trigger` distingue falhas PDP/PoTR/Both, enquanto `penalty` captura se o colateral foi realmente cortado ou suprimido por cooldown. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Medidor | `provider_id` | Contagens mais recentes de PDP/PoTR relacionadas dentro da janela de telemetria infratora para que as equipes quantifiquem o quanto os provedores ultrapassaram a política. |
| `torii_sorafs_proof_health_penalty_nano` | Medidor | `provider_id` | Valor Nano-XOR cortado no último alerta (zero quando o cooldown suprimiu a aplicação). |
| `torii_sorafs_proof_health_cooldown` | Medidor | `provider_id` | Gauge booleano (`1` = alerta suprimido por cooldown) para mostrar quando alertas de acompanhamento estão temporariamente silenciados. |
| `torii_sorafs_proof_health_window_end_epoch` | Medidor | `provider_id` | Época registrada para a janela de telemetria ligada ao alerta para que os operadores se correlacionem com os artistas Norito. |

Esses feeds agora alimentam a linha de saúde das provas do dashboard Taikai viewer
(`dashboards/grafana/taikai_viewer.json`), dando aos operadores CDN visibilidade em tempo real
sobre volumes de alertas, combinação de gatilhos PDP/PoTR, deliberações e estado de resfriamento por
provedor.

As mesmas métricas agora sustentam duas regras de alerta do Taikai Viewer:
`SorafsProofHealthPenalty` dispara quando
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` aumenta
nos últimos 15 minutos, enquanto `SorafsProofHealthCooldown` emite um aviso se um
provedor de permanência em cooldown por cinco minutos. Ambos os alertas vivem em
`dashboards/alerts/taikai_viewer_rules.yml` para que os SREs recebam contexto imediatamente
quando a aplicação PoR/PoTR se intensifica.

### Superficies do orquestrador| Métrica / Evento | Tipo | Etiquetas | Produtor | Notas |
|------------------|------|--------|----------|-------|
| `sorafs_orchestrator_active_fetches` | Medidor | `manifest_id`, `region` | `FetchMetricsCtx` | Sessões atualmente em voo. |
| `sorafs_orchestrator_fetch_duration_ms` | Histograma | `manifest_id`, `region` | `FetchMetricsCtx` | Histograma de duração em milissegundos; baldes de 1 ms a 30 s. |
| `sorafs_orchestrator_fetch_failures_total` | Contador | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Razões: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Contador | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Distinguir causas de nova tentativa (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Contador | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Captura desabilitações e contágios de falhas no nível de sessão. |
| `sorafs_orchestrator_chunk_latency_ms` | Histograma | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Distribuição de latência de busca por chunk (ms) para análise de throughput/SLO. |
| `sorafs_orchestrator_bytes_total` | Contador | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Bytes entregues por manifesto/provedor; derivar o rendimento via `rate()` em PromQL. |
| `sorafs_orchestrator_stalls_total` | Contador | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Conta pedaços que excedem `ScoreboardConfig::latency_cap_ms`. |
| `telemetry::sorafs.fetch.lifecycle` | Evento estruturado | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Espelha o ciclo de vida do job (start/complete) com payload JSON Norito. |
| `telemetry::sorafs.fetch.retry` | Evento estruturado | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Emitido por sequência de tentativas pelo provedor; `attempts` conta retentativas incrementais (>= 1). |
| `telemetry::sorafs.fetch.provider_failure` | Evento estruturado | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Publicado quando um provedor cruza o limite de falhas. |
| `telemetry::sorafs.fetch.error` | Evento estruturado | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Registro de falha no terminal, amigável para ingestão de Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | Evento estruturado | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Emitido quando a latência de chunk ultrapassa o limite configurado (espelha contadores de stall). |

### Superfícies de não / replicacao| Métrica | Tipo | Etiquetas | Notas |
|--------|------|--------|-------|
| `sorafs_node_capacity_utilisation_pct` | Histograma | `provider_id` | Histograma OTEL da porcentagem de utilização de armazenamento (exportado como `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Contador | `provider_id` | Contador monotono de amostras PoR bem-sucedidas, derivado de snapshots do agendador. |
| `sorafs_node_por_failure_total` | Contador | `provider_id` | Contador monótono de amostras PoR com falha. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Medidor | `provider` | Gauges Prometheus existentes para bytes usados, profundidade de fila, contagens PoR em voo. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Medidor | `provider` | Dados de capacidade/tempo de atividade bem sucedidos do provedor fornecidos no painel de capacidade. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Medidor | `provider`, `manifest` | Profundidade do backlog mais contadores acumulados de falhas exportados sempre que `/v2/sorafs/por/ingestion/{manifest}` e consultado, alimentando o painel/alerta "PoR Stalls". |

### Prova de recuperação oportuna (PoTR) e SLA de pedaços

| Métrica | Tipo | Etiquetas | Produtor | Notas |
|--------|------|--------|----------|-------|
| `sorafs_potr_deadline_ms` | Histograma | `tier`, `provider` | Coordenador PoTR | Folga do prazo em milissegundos (positivo = atendido). |
| `sorafs_potr_failures_total` | Contador | `tier`, `provider`, `reason` | Coordenador PoTR | Razões: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Contador | `provider`, `manifest_id`, `reason` | Monitorar SLA | Disparado quando a entrega de pedaços falha no SLO (latência, taxa de sucesso). |
| `sorafs_chunk_sla_violation_active` | Medidor | `provider`, `manifest_id` | Monitorar SLA | Gauge booleano (0/1) alternado durante uma janela de violação ativa. |

## Objetivos SLO

- Disponibilidade trustless do gateway: **99,9%** (respostas HTTP 2xx/304).
- TTFB P95 sem confiança: camada quente = 99,5% por dia.
- Sucesso do orquestrador (conclusão de chunks): >= 99%.

## Painéis e alertas

1. **Observabilidade do gateway** (`dashboards/grafana/sorafs_gateway_observability.json`) - acompanha disponibilidade trustless, TTFB P95, detalhamento de recusas e falhas PoR/PoTR via métricas OTEL.
2. **Saúde do orquestrador** (`dashboards/grafana/sorafs_fetch_observability.json`) - cobre carga multi-source, retries, falhas de provedores e rajadas de travamentos.
3. **Métricas de privacidade SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) - gráficos de buckets de relé anonimizados, janelas de supressão e saúde do coletor via `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` e `soranet_privacy_poll_errors_total{provider}`.

Pacotes de alertas:- `dashboards/alerts/sorafs_gateway_rules.yml` - disponibilidade do gateway, TTFB, picos de falhas de provas.
- `dashboards/alerts/sorafs_fetch_rules.yml` - falhas/repetições/paradas do orquestrador; validado via `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` e `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` - picos de degradação de privacidade, alarmes de supressão, detecção de coletor ocioso e alertas de coletor desabilitado (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` - alarmes de brownout de anonimato ligados a `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` - alarmes de drift/ingest/CEK lag do Taikai viewer mais os novos alertas de surra/cooldown de saúde das provas SoraFS alimentados por `torii_sorafs_proof_health_*`.

## Estratégia de rastreamento

- Adote OpenTelemetry de ponta a ponta:
  - Gateways emitem spans OTLP (HTTP) anotados com IDs de solicitação, resumos de manifesto e hashes de token.
  - O orquestrador usa `tracing` + `opentelemetry` para exportar spans de tentativas de fetch.
  - Nos SoraFS embutidos exportam vãos para desafios PoR e operações de armazenamento. Todos os componentes compartilham um ID de rastreamento comum propagado via `x-sorafs-trace`.
- `SorafsFetchOtel` liga métricas do orquestrador a histogramas OTLP enquanto eventos `telemetry::sorafs.fetch.*` fornece payloads JSON leves para backends centrados em logs.
- Colecionadores: execute coletores OTEL ao lado de Prometheus/Loki/Tempo (Tempo preferido). Exportadores compatíveis com Jaeger permanecem preservados.
- Operações de alta cardinalidade devem ser amostradas (10% para caminhos de sucesso, 100% para falhas).

## Coordenação de telemetria TLS (SF-5b)

- Alinhamento de métricas:
  - A automação TLS envia `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` e `sorafs_gateway_tls_ech_enabled`.
  - Inclui esses medidores no painel Visão geral do gateway sob o painel TLS/Certificados.
- Vinculo de alertas:
  - Quando alertas de expiração TLS dispararem (<= 14 dias restantes), correlacione com o SLO de disponibilidade trustless.
  - A desativação do ECH emite um alerta secundário referenciando tanto os problemas do TLS quanto os de disponibilidade.
- Pipeline: o trabalho de exportação automática de TLS para a mesma pilha Prometheus das métricas do gateway; a coordenação com SF-5b garante instrumentação deduplicada.

## Convenções de nomes e rótulos de métricas- Nomes de métricas Seguem os prefixos existentes `torii_sorafs_*` ou `sorafs_*` usados ​​por Torii e o gateway.
- Conjuntos de etiquetas são padronizados:
  - `result` -> resultado HTTP (`success`, `refused`, `failed`).
  - `reason` -> código de recusa/erro (`unsupported_chunker`, `timeout`, etc.).
  - `provider` -> identificador de provedor codificado em hex.
  - `manifest` -> resumo canônico do manifesto (cortado quando a cardinalidade e alta).
  - `tier` -> rótulos declarativos de nível (`hot`, `warm`, `archive`).
- Pontos de emissão de telemetria:
  - Métricas do gateway vivem sob `torii_sorafs_*` e reutilizam convenções de `crates/iroha_core/src/telemetry.rs`.
  - O orquestrador emite métricas `sorafs_orchestrator_*` e eventos `telemetry::sorafs.fetch.*` (ciclo de vida, nova tentativa, falha do provedor, erro, parada) marcados com resumo de manifesto, ID de trabalho, região e identificadores de provedor.
  - Nos expoem `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` e `torii_sorafs_por_*`.
- Coordene com Observabilidade para registrar o catálogo de métricas no documento compartilhado de nomes Prometheus, incluindo expectativas de cardinalidade de rótulos (limites superiores de provedor/manifestos).

## Pipeline de dados

- Coletores são implantados junto a cada componente, exportando OTLP para Prometheus (métricas) e Loki/Tempo (logs/traces).
- eBPF opcional (Tetragon) rastreamento de riqueza de baixo nível para gateways/nos.
- Utilize `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` para Torii e nos embutidos; o orquestrador continua tocando `install_sorafs_fetch_otlp_exporter`.

## Ganchos de validação

- Execute `scripts/telemetry/test_sorafs_fetch_alerts.sh` durante CI para garantir que as regras de alerta Prometheus permaneçam em sincronia com métricas de bloqueio e verificações de supressão de privacidade.
- Manter dashboards Grafana sob controle de versão (`dashboards/grafana/`) e atualizar screenshots/links quando os paineis mudarem.
- Drills de caos registram resultados via `scripts/telemetry/log_sorafs_drill.sh`; a validacao usa `scripts/telemetry/validate_drill_log.sh` (veja o [Playbook de operacoes](operations-playbook.md)).