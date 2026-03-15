---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de observabilidade
título: Plano de observabilidade e SLO de SoraFS
sidebar_label: Observabilidade e SLO
descrição: esquema de telefonia, painéis e políticas de erro orçamentário para gateways SoraFS, noeuds e orquestrador multi-fonte.
---

:::nota Fonte canônica
Esta página reflete o plano de manutenção em `docs/source/sorafs_observability_plan.md`. Gardez as duas cópias sincronizadas junto com a migração completa do antigo conjunto Sphinx.
:::

## Objetivos
- Definir métricas e eventos estruturados para gateways, noeuds e orquestradores multifonte.
- Fornece painéis Grafana, seus alertas e ganchos de validação.
- Estabelecer objetivos de SLO com políticas de erro orçamentário e exercícios de caos.

## Catálogo de métricas

### Superfícies do gateway

| Métrica | Tipo | Etiquetas | Notas |
|--------|------|------------|-------|
| `sorafs_gateway_active` | Medidor (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | Émis via `SorafsGatewayOtel`; atende às operações HTTP em volume por combinação de endpoint/método. |
| `sorafs_gateway_responses_total` | Contador | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Todo o gateway necessário foi encerrado incrementalmente uma vez ; `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histograma | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Latência time-to-first-byte para respostas gateway ; exportado em Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Contador | `profile_version`, `result`, `error_code` | Resultados de verificação de tentativas capturadas no momento da solicitação (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histograma | `profile_version`, `result`, `error_code` | Distribuição da latência de verificação para os recursos PoR. |
| `telemetry::sorafs.gateway.request` | Evento estruturado | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Log estruturado é mis a chaque fin de requête para correlação Loki/Tempo. |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Contador | Jeux d'étiquettes herités | Métricas Prometheus preservadas para os painéis históricos; emissões paralelas da nova série OTLP. |

Os eventos `telemetry::sorafs.gateway.request` refletem os computadores OTEL com cargas úteis estruturadas, expostos `endpoint`, `method`, `variant`, `status`, `error_code` e `duration_ms` para a correlação Loki/Tempo, portanto os painéis consomem a série OTLP para seguir o SLO.

### Télémétrie de santé des preuves| Métrica | Tipo | Etiquetas | Notas |
|--------|------|------------|-------|
| `torii_sorafs_proof_health_alerts_total` | Contador | `provider_id`, `trigger`, `penalty` | Aumente cada vez que `RecordCapacityTelemetry` apresentou um `SorafsProofHealthAlert`. `trigger` distingue as verificações PDP/PoTR/Both, enquanto `penalty` captura se a garantia for real foi amputada ou suprimida por resfriamento. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Medidor | `provider_id` | Os últimos cálculos do PDP/PoTR reportados na janela de telefonia fizeram com que as equipes quantificassem o atraso político entre os fornecedores. |
| `torii_sorafs_proof_health_penalty_nano` | Medidor | `provider_id` | O Montant Nano-XOR foi amputado no último alerta (zero quando o resfriamento for desativado). |
| `torii_sorafs_proof_health_cooldown` | Medidor | `provider_id` | Medidor booleano (`1` = alerta suprimido por resfriamento) para sinalizar quando os alertas seguintes são silenciados temporariamente. |
| `torii_sorafs_proof_health_window_end_epoch` | Medidor | `provider_id` | Época registrada para a janela de telefonia, alertada para que os operadores possam se conectar com os artefatos Norito. |

Este fluxo de alimentos é desordenado na linha de prova-saúde do painel Taikai Viewer
(`dashboards/grafana/taikai_viewer.json`), oferece aos operadores CDN visibilidade direta
nos volumes de alertas, na combinação de gatilhos PDP/PoTR, nas penalidades e no estado de resfriamento por
Fournisseur.

As mesmas métricas são mantidas mantendo duas regras de alerta do visualizador de Taikai:
`SorafsProofHealthPenalty` é desativado quando
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` aumentar
nos últimos 15 minutos, então `SorafsProofHealthCooldown` emitiu um aviso se um
fournisseur reste e cooldown pendente cinco minutos. Les deux alertes vivent dans
`dashboards/alerts/taikai_viewer_rules.yml` para que os SREs estejam disponíveis em um contexto imediato
Quando a aplicação PoR/PoTR é intensificada.

### Superfícies do orquestrador| Métrica / Événement | Tipo | Etiquetas | Produtor | Notas |
|----------------------|------|------------|------------|-------|
| `sorafs_orchestrator_active_fetches` | Medidor | `manifest_id`, `region` | `FetchMetricsCtx` | Sessões atuais no vol. |
| `sorafs_orchestrator_fetch_duration_ms` | Histograma | `manifest_id`, `region` | `FetchMetricsCtx` | Histograma de duração em milissegundos; baldes 1 ms a 30 s. |
| `sorafs_orchestrator_fetch_failures_total` | Contador | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Razões: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Contador | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Distinga as causas de nova tentativa (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Contador | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Capture as desativações e contas de verificação no nível da sessão. |
| `sorafs_orchestrator_chunk_latency_ms` | Histograma | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Distribuição de busca de latência por pedaço (ms) para análise de taxa de transferência/SLO. |
| `sorafs_orchestrator_bytes_total` | Contador | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Octetos livres por manifesto/fornecedor; reduza a taxa de transferência via `rate()` no PromQL. |
| `sorafs_orchestrator_stalls_total` | Contador | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Compte les chunks qui dépassent `ScoreboardConfig::latency_cap_ms`. |
| `telemetry::sorafs.fetch.lifecycle` | Evento estruturado | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Reflita o ciclo de vida do trabalho (iniciar/concluir) com carga útil JSON Norito. |
| `telemetry::sorafs.fetch.retry` | Evento estruturado | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Émis por sequência de nova tentativa por fornecedor; `attempts` calcula as tentativas incrementais (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | Evento estruturado | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Publié lorsqu'un fornecedor franqueado le seuil d'échecs. |
| `telemetry::sorafs.fetch.error` | Evento estruturado | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Registro de falha no terminal, adaptado à ingestão do Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | Evento estruturado | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Isso acontece quando o pedaço de latência ultrapassa o limite configurado (reflete os computadores de parada). |

### Nœud de superfícies / replicação| Métrica | Tipo | Etiquetas | Notas |
|--------|------|------------|-------|
| `sorafs_node_capacity_utilisation_pct` | Histograma | `provider_id` | Histograma OTEL da porcentagem de utilização do estoque (exportado em `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Contador | `provider_id` | Compteur monótono de échantillons PoR réussis, derivado de snapshots du agendador. |
| `sorafs_node_por_failure_total` | Contador | `provider_id` | Compteur monotone des échantillons PoR échoués. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Medidor | `provider` | Medidores Prometheus existentes para octetos utilizados, profundor de arquivo, conta PoR e vol. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Medidor | `provider` | Dados de capacidade/tempo de atividade relatados pelo fornecedor expostos no painel de capacidade. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Medidor | `provider`, `manifest` | Profondeur du backlog mais contas acumuladas de cheques exportados para cada interrogação de `/v2/sorafs/por/ingestion/{manifest}`, alimenta o painel/alerta "PoR Stalls". |

### Prevenção de recuperação em tempo útil (PoTR) e SLA de pedaços

| Métrica | Tipo | Etiquetas | Produtor | Notas |
|---------|------|------------|------------|-------|
| `sorafs_potr_deadline_ms` | Histograma | `tier`, `provider` | Coordenador PoTR | Margem de prazo em milissegundos (positivo = respeitado). |
| `sorafs_potr_failures_total` | Contador | `tier`, `provider`, `reason` | Coordenador PoTR | Razões: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Contador | `provider`, `manifest_id`, `reason` | Monitor SLA | Diminuído quando a entrega de pedaços avalia o SLO (latência, taxa de sucesso). |
| `sorafs_chunk_sla_violation_active` | Medidor | `provider`, `manifest_id` | Monitor SLA | Medidor booleano (0/1) ativado durante a janela de violação ativa. |

## Objetivos SLO

- Disponibilidade confiável do gateway: **99,9%** (respostas HTTP 2xx/304).
- TTFB P95 sem confiança: camada quente ≤ 120 ms, camada quente ≤ 300 ms.
- Taxa de sucesso das tentativas: ≥ 99,5% por dia.
- Succès de l'orchestrateur (finalização de pedaços): ≥ 99%.

## Painéis e alertas

1. **Gateway de observabilidade** (`dashboards/grafana/sorafs_gateway_observability.json`) — compatível com a disponibilidade trustless, TTFB P95, a partição de recusa e as verificações PoR/PoTR por meio das métricas OTEL.
2. **Santé de l'orchestrateur** (`dashboards/grafana/sorafs_fetch_observability.json`) — cobre a carga multi-fonte, as tentativas, os échecs fournisseurs e os rafales de stalls.
3. **Métricas de confidencialidade SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — rastreie os baldes de relações anônimas, as janelas de supressão e a saúde dos coletores via `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` e `soranet_privacy_poll_errors_total{provider}`.

Pacotes de alertas:- `dashboards/alerts/sorafs_gateway_rules.yml` — gateway de disponibilidade, TTFB, fotos de verificações prévias.
- `dashboards/alerts/sorafs_fetch_rules.yml` — échecs/retries/stalls do orquestrador ; validado via `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` e `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` — fotos de degradação de confidencialidade, alarmes de supressão, detecção de coletor inativo e alertas de coletor desativado (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — alarmes de queda de energia de cabos anônimos em `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` — alarmes de desvio/ingestão/CEK lag Taikai viewer mais os novos alertas de penalização/recarga de sanidade de pré-prevenções SoraFS alimentados por `torii_sorafs_proof_health_*`.

## Estratégia de rastreamento

- Adotador OpenTelemetry de combate em combate:
  - Os gateways enviam spans OTLP (HTTP) anotados com IDs de solicitação, resumos de manifesto e hashes de token.
  - O orquestrador utiliza `tracing` + `opentelemetry` para exportar os trechos de tentativas de busca.
  - Os noeuds SoraFS embarquem os vãos para os défis PoR e as operações de armazenamento. Todos os componentes compartilham um ID de rastreamento comum propagado via `x-sorafs-trace`.
- `SorafsFetchOtel` depende de métricas orquestradas em histogramas OTLP enquanto os eventos `telemetry::sorafs.fetch.*` fornecem cargas úteis JSON legíveis para back-ends centrados em logs.
- Colecionadores: execute os coletores OTEL no côté de Prometheus/Loki/Tempo (Tempo préféré). As opções restantes da API Jaeger dos exportadores.
- As operações de alta cardinalidade devem ser echantillonnées (10% para os caminhos de sucesso, 100% para os échecs).

## Coordenação da telefonia TLS (SF-5b)

- Alinhamento de métricas:
  - Automatização TLS pública `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` e `sorafs_gateway_tls_ech_enabled`.
  - Inclui esses medidores no painel Visão geral do gateway no painel TLS/Certificados.
- Ligação de alertas:
  - Quando os alertas de expiração TLS forem desativados (≤ 14 dias restantes), corrija-os com o SLO de disponibilidade confiável.
  - A desativação do ECH emitiu um alerta secundário referente aos painéis TLS e à disponibilidade.
- Pipeline: o trabalho de automação TLS exporta para a mesma pilha Prometheus que as métricas gateway; a coordenação com SF-5b garante uma instrumentação duplicada.

## Convenções de nommage e etiqueta de métricas- Os nomes de métricas seguem os prefixos existentes `torii_sorafs_*` ou `sorafs_*` usados ​​por Torii e pelo gateway.
- Os conjuntos de etiquetas são padronizados:
  - `result` → resultado HTTP (`success`, `refused`, `failed`).
  - `reason` → código de recusa/erro (`unsupported_chunker`, `timeout`, etc.).
  - `provider` → fornecedor identificador codificado em hexadecimal.
  - `manifest` → resumo canônico do manifesto (tronqué quand la cardinalité est elevado).
  - `tier` → etiquetas de nível declarativo (`hot`, `warm`, `archive`).
- Pontos de emissão de télémétrie:
  - As métricas do gateway vivem sob `torii_sorafs_*` e utilizam as convenções de `crates/iroha_core/src/telemetry.rs`.
  - O orquestrador contém as métricas `sorafs_orchestrator_*` e os eventos `telemetry::sorafs.fetch.*` (ciclo de vida, nova tentativa, falha do provedor, erro, parada) marcados com resumo do manifesto, ID do trabalho, região e fornecedor de identificadores.
  - Os números expostos `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` e `torii_sorafs_por_*`.
- Coordenado com Observabilidade para registrar o catálogo de métricas no documento de nome Prometheus enviado, e inclui as atentas de cardinalidade de rótulos (carregados superiores de fornecedores/manifestos).

## Pipeline de données

- Os coletores são implantados na região de cada composto, exportando OTLP para Prometheus (métricas) e Loki/Tempo (logs/traces).
- Opção eBPF (Tetragon) enriquece o rastreamento no nível básico para gateways/nœuds.
- Utilize `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` para Torii e os novos embarques; l'orchestrateur continue d'appeler `install_sorafs_fetch_otlp_exporter`.

## Ganchos de validação

- Execute `scripts/telemetry/test_sorafs_fetch_alerts.sh` em CI para garantir que as regras de alerta Prometheus permaneçam alinhadas com as métricas de bloqueio e as verificações de supressão de confidencialidade.
- Mantenha os painéis Grafana sob controle de versão (`dashboards/grafana/`) e mantenha as capturas/links atualizados quando os painéis forem alterados.
- Os exercícios de caos publicam os resultados via `scripts/telemetry/log_sorafs_drill.sh`; a validação é aplicada em `scripts/telemetry/validate_drill_log.sh` (veja o [Playbook d'exploitation](operations-playbook.md)).