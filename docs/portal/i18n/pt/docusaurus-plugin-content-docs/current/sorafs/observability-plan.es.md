---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de observabilidade
título: Plano de observação e SLO de SoraFS
sidebar_label: Observabilidade e SLOs
description: Esquema de telemetria, dashboards e política de presunção de erro para gateways SoraFS, nós e o orquestrador multifuente.
---

:::nota Fonte canônica
Esta página reflete o plano mantido em `docs/source/sorafs_observability_plan.md`. Mantenha ambas as cópias sincronizadas até que o conjunto de documentação Sphinx migre por completo.
:::

## Objetivos
- Definir detalhes e eventos estruturados para gateways, nós e o orquestrador multifuente.
- Experimente painéis de controle Grafana, guarda-chuvas de alerta e ganchos de validação.
- Estabelecer objetivos de SLO junto com políticas de presunção de erro e exercícios de caos.

## Catálogo de estatísticas

### Superfícies do gateway

| Métrica | Tipo | Etiquetas | Notas |
|--------|------|-----------|-------|
| `sorafs_gateway_active` | Medidor (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | Emitido via `SorafsGatewayOtel`; rastreie operações HTTP em vôo por combinação de endpoint/método. |
| `sorafs_gateway_responses_total` | Contador | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Cada solicitação concluída do gateway é incrementada uma vez; `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histograma | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Latência de tempo até o primeiro byte para respostas do gateway; exportado como Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Contador | `profile_version`, `result`, `error_code` | Resultados de verificação de testes capturados no momento da solicitação (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histograma | `profile_version`, `result`, `error_code` | Distribuição de latência de verificação para recibos PoR. |
| `telemetry::sorafs.gateway.request` | Evento estruturado | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Log estruturado emitido para completar cada solicitação de correlação em Loki/Tempo. |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Contador | Conjuntos de etiquetas heredadas | Métricas Prometheus retenidas para dashboards históricos; emissões junto com a nova série OTLP. |

Os eventos `telemetry::sorafs.gateway.request` refletem os contadores OTEL com cargas úteis estruturadas, expondo `endpoint`, `method`, `variant`, `status`, `error_code` e `duration_ms` para correlação em Loki/Tempo, enquanto os painéis consomem a série OTLP para o acompanhamento de SLO.

### Telemetria de saúde de testes| Métrica | Tipo | Etiquetas | Notas |
|--------|------|-----------|-------|
| `torii_sorafs_proof_health_alerts_total` | Contador | `provider_id`, `trigger`, `penalty` | Se incrementa cada vez que `RecordCapacityTelemetry` emite um `SorafsProofHealthAlert`. `trigger` distingue falhas PDP/PoTR/Ambos, enquanto `penalty` é capturado e a garantia é realmente recuperada ou suprimida por resfriamento. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Medidor | `provider_id` | Os relatos mais recentes de PDP/PoTR relatados dentro da janela de telemetria infratora para que os equipamentos cuantifiquem quanto se excediam os provedores da política. |
| `torii_sorafs_proof_health_penalty_nano` | Medidor | `provider_id` | Monto Nano-XOR gravado no último alerta (cero quando o resfriamento suprimiu o aplicativo). |
| `torii_sorafs_proof_health_cooldown` | Medidor | `provider_id` | Gauge booleano (`1` = alerta suprimida por cooldown) para mostrar quando os alertas de acompanhamento estão temporariamente silenciados. |
| `torii_sorafs_proof_health_window_end_epoch` | Medidor | `provider_id` | Época registrada para a janela de telemetria vinculada ao alerta para que os operadores se correlacionem com os artefatos Norito. |

Esses feeds agora alimentam a fila de saúde dos testes do painel Taikai Viewer
(`dashboards/grafana/taikai_viewer.json`), dando aos operadores CDN visibilidade em tempo real
de volumes de alertas, mistura de disparadores PDP/PoTR, penalizações e estado de resfriamento por
provedor.

As mesmas métricas agora respaldam as regras de alerta do visualizador do Taikai:
`SorafsProofHealthPenalty` se dispara quando
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` aumenta em
nos últimos 15 minutos, enquanto `SorafsProofHealthCooldown` lança uma advertência se uma
O provedor permanece em espera por cinco minutos. Ambas alertas viven en
`dashboards/alerts/taikai_viewer_rules.yml` para que os SREs recebam contexto imediato
quando a aplicação PoR/PoTR se intensifica.

### Superfícies do orquestrador| Métrica / Evento | Tipo | Etiquetas | Produtor | Notas |
|-----------------|------|-----------|-----------|-------|
| `sorafs_orchestrator_active_fetches` | Medidor | `manifest_id`, `region` | `FetchMetricsCtx` | Sessões atualmente em voo. |
| `sorafs_orchestrator_fetch_duration_ms` | Histograma | `manifest_id`, `region` | `FetchMetricsCtx` | Histograma de duração em milissegundos; baldes de 1 ms a 30 s. |
| `sorafs_orchestrator_fetch_failures_total` | Contador | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Razões: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Contador | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Distinguir causas de reintenção (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Contador | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Captura debilitação e conteúdo de falhas no nível da sessão. |
| `sorafs_orchestrator_chunk_latency_ms` | Histograma | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Distribuição de latência de busca por pedaço (ms) para análise de taxa de transferência/SLO. |
| `sorafs_orchestrator_bytes_total` | Contador | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Bytes entregues por manifesto/provedor; deriva o rendimento via `rate()` no PromQL. |
| `sorafs_orchestrator_stalls_total` | Contador | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Quantos pedaços excedem `ScoreboardConfig::latency_cap_ms`. |
| `telemetry::sorafs.fetch.lifecycle` | Evento estruturado | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Reflita sobre o ciclo de vida do trabalho (inicial/concluído) com payload JSON Norito. |
| `telemetry::sorafs.fetch.retry` | Evento estruturado | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Emitido por racha de reintentos por provedor; `attempts` conta reintentos incrementais (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | Evento estruturado | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Se publica quando um provedor cruza o umbral de fallos. |
| `telemetry::sorafs.fetch.error` | Evento estruturado | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Registro de terminal falho, amigável para ingestão em Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | Evento estruturado | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | É emitido quando a latência do pedaço ultrapassa o limite configurado (reflete contadores de travamento). |

### Superfícies de nó/replicação| Métrica | Tipo | Etiquetas | Notas |
|--------|------|-----------|-------|
| `sorafs_node_capacity_utilisation_pct` | Histograma | `provider_id` | Histograma OTEL da porcentagem de utilização de armazenamento (exportado como `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Contador | `provider_id` | Contador monotônico de exibições PoR exitosas, derivado de snapshots do agendador. |
| `sorafs_node_por_failure_total` | Contador | `provider_id` | Contador monotônico de mostras PoR falidas. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Medidor | `provider` | Medidores Prometheus existentes para bytes usados, profundidade de cola, conteúdos PoR em voo. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Medidor | `provider` | Dados de capacidade/tempo de atividade existentes no provedor exposto no painel de capacidade. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Medidor | `provider`, `manifest` | A profundidade do backlog é maior que os contadores acumulados de falhas exportados cada vez que você consulta `/v2/sorafs/por/ingestion/{manifest}`, alimentando o painel/alerta "PoR Stalls". |

### Teste de recuperação rápida (PoTR) e SLA de chunks

| Métrica | Tipo | Etiquetas | Produtor | Notas |
|--------|------|-----------|-----------|-------|
| `sorafs_potr_deadline_ms` | Histograma | `tier`, `provider` | Coordenador PoTR | Holgura do prazo em milissegundos (positivo = cumplido). |
| `sorafs_potr_failures_total` | Contador | `tier`, `provider`, `reason` | Coordenador PoTR | Razões: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Contador | `provider`, `manifest_id`, `reason` | Monitorar SLA | Se dispara quando a entrega de pedaços incumpri o SLO (latência, taxa de saída). |
| `sorafs_chunk_sla_violation_active` | Medidor | `provider`, `manifest_id` | Monitorar SLA | Medidor booleano (0/1) alternado durante a janela de incumprimento ativada. |

## Objetivos SLO

- Disponibilidade confiável do gateway: **99,9%** (respostas HTTP 2xx/304).
- TTFB P95 sem confiança: camada quente ≤ 120 ms, camada quente ≤ 300 ms.
- Taxa de sucesso de testes: ≥ 99,5% por dia.
- Éxito do orquestrador (finalização de pedaços): ≥ 99%.

## Painéis e alertas

1. **Observabilidade do gateway** (`dashboards/grafana/sorafs_gateway_observability.json`) — rastreia disponibilidade trustless, TTFB P95, desglose de rechazos e falhas PoR/PoTR através das métricas OTEL.
2. **Salud del orquestrador** (`dashboards/grafana/sorafs_fetch_observability.json`) — cubre carga multifuente, reintentos, fallos de proveedores y ráfagas de stalls.
3. **Métricas de privacidade de SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — gráficos de baldes de relé anonimizados, janelas de supressão e saúde de coletores via `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` e `soranet_privacy_poll_errors_total{provider}`.

Pacotes de alertas:- `dashboards/alerts/sorafs_gateway_rules.yml` — disponibilidade do gateway, TTFB, picos de falhas de teste.
- `dashboards/alerts/sorafs_fetch_rules.yml` — fallos/reintentos/stalls do orquestrador; validado através de `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` e `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` — picos de degradação de privacidade, alarmes de supressão, detecção de coletor inativo e alertas de coletor desativado (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — alarmes de brownout de anonimato conectados a `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` — alarmes de deriva/ingestão/CEK lag do visualizador de Taikai além dos novos alertas de penalização/resfriamento de saúde de testes SoraFS impulsionados por `torii_sorafs_proof_health_*`.

## Estratégia de trazas

- Adote OpenTelemetry de extremo a extremo:
  - Os gateways emitem spans OTLP (HTTP) anotados com IDs de solicitação, resumos de manifesto e hashes de token.
  - O orquestrador usa `tracing` + `opentelemetry` para exportar extensões de intenções de busca.
  - Os nodos SoraFS embebidos exportam spans para desafios PoR e operações de armazenamento. Todos os componentes compartilham um ID de rastreamento comum propagado via `x-sorafs-trace`.
- `SorafsFetchOtel` conecta as estatísticas do orquestrador a histogramas OTLP enquanto os eventos `telemetry::sorafs.fetch.*` fornece cargas úteis JSON leves para back-ends centralizados em logs.
- Colecionadores: colecionadores ejecuta OTEL junto com Prometheus/Loki/Tempo (Tempo preferido). Os exportadores API Jaeger ainda são opcionais.
- As operações de alta cardinalidade devem ser muestrearse (10% para rotas de sucesso, 100% para falhas).

## Coordenação de telemetria TLS (SF-5b)

- Alinhamento de métricas:
  - A automação TLS envia `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` e `sorafs_gateway_tls_ech_enabled`.
  - Inclui esses medidores no painel Visão geral do gateway no painel TLS/Certificados.
- Vinculação de alertas:
  - Quando houver alertas de expiração TLS (≤ 14 dias restantes) correlacionados com o SLO de disponibilidade confiável.
  - A desativação do ECH emite um alerta secundário que refere tanto os painéis TLS como de disponibilidade.
- Pipeline: o trabalho de automatização TLS exporta para a mesma pilha Prometheus que as métricas do gateway; a coordenação com SF-5b garante instrumentação desduplicada.

## Convenções de nomes e etiquetas de métricas- Os nomes das estatísticas seguem os prefixos existentes `torii_sorafs_*` ou `sorafs_*` usados ​​por Torii e o gateway.
- Os conjuntos de etiquetas estão padronizados:
  - `result` → resultado HTTP (`success`, `refused`, `failed`).
  - `reason` → código de rechazo/erro (`unsupported_chunker`, `timeout`, etc.).
  - `provider` → identificador de provedor codificado em hexadecimal.
  - `manifest` → resumo canônico de manifesto (recortado quando hay alta cardinalidad).
  - `tier` → etiquetas declarativas de nível (`hot`, `warm`, `archive`).
- Pontos de emissão de telemetria:
  - As métricas do gateway vivem abaixo de `torii_sorafs_*` e reutilizam as convenções de `crates/iroha_core/src/telemetry.rs`.
  - O organizador emite análises `sorafs_orchestrator_*` e eventos `telemetry::sorafs.fetch.*` (ciclo de vida, nova tentativa, falha do provedor, erro, parada) marcados com resumo de manifesto, ID de trabalho, região e identificadores de fornecedor.
  - Os nodos expõem `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` e `torii_sorafs_por_*`.
- Coordenação com Observabilidade para registrar o catálogo de métricas no documento compartido de nomes Prometheus, incluindo expectativas de cardinalidade de etiquetas (límites superiores de provedor/manifestos).

## Pipeline de dados

- Os coletores são despliegan junto com cada componente, exportando OTLP para Prometheus (métricas) e Loki/Tempo (logs/trazas).
- eBPF opcional (Tetragon) enriquecendo o trazido de baixo nível para gateways/nodos.
- Usa `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` para Torii e nodos embebidos; o orquestrador continua chamando `install_sorafs_fetch_otlp_exporter`.

## Ganchos de validação

- Execute `scripts/telemetry/test_sorafs_fetch_alerts.sh` durante CI para garantir que as regras de alerta de Prometheus permaneçam em sintonia com avaliações de barracas e verificações de supressão de privacidade.
- Mantenha os painéis de Grafana sob controle de versões (`dashboards/grafana/`) e atualize capturas/links quando os painéis forem alterados.
- Los drills de caos registran resultados via `scripts/telemetry/log_sorafs_drill.sh`; a validação usa `scripts/telemetry/validate_drill_log.sh` (consulte o [Playbook de operações](operations-playbook.md)).