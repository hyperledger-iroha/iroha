---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de observabilidade
título: SoraFS Observabilidade no SLO
sidebar_label: Observabilidade em SLOs
descrição: Gateways SoraFS, nós e orquestrador de múltiplas fontes, esquema de telemetria, painéis e política de orçamento de erros.
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sorafs_observability_plan.md` میں برقرار رکھے گئے منصوبے کی عکاسی کرتا ہے۔ جب تک پرانا Esfinge سیٹ مکمل طور پر منتقل نہ ہو جائے دونوں نقول کو ہم آہنگ رکھیں۔
:::

## Objetivos
- gateways, nós e orquestrador multi-fonte کے لیے métricas اور eventos estruturados کی تعریف کریں۔
- Painéis Grafana, limites de alerta e ganchos de validação فراہم کریں۔
- orçamento de erros e políticas de simulação de caos کے ساتھ metas de SLO قائم کریں۔

## Catálogo de Métricas

### Superfícies de gateway

| Métrica | Tipo | Etiquetas | Notas |
|--------|------|--------|-------|
| `sorafs_gateway_active` | Medidor (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | `SorafsGatewayOtel` کے ذریعے emitir ہوتا ہے؛ ہر endpoint/método کمبینیشن کے لیے operações HTTP em voo ٹریک کرتا ہے۔ |
| `sorafs_gateway_responses_total` | Contador | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | ہر مکمل solicitação de gateway ایک بار increment ہوتی ہے؛ `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histograma | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | respostas do gateway کے لیے latência de tempo até o primeiro byte; Prometheus `_bucket/_sum/_count` کے طور پر export۔ |
| `sorafs_gateway_proof_verifications_total` | Contador | `profile_version`, `result`, `error_code` | tempo de solicitação پر captura de resultados de verificação de prova کیے جاتے ہیں (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histograma | `profile_version`, `result`, `error_code` | Recibos PoR کے لیے distribuição de latência de verificação۔ |
| `telemetry::sorafs.gateway.request` | Evento estruturado | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | ہر conclusão da solicitação پر emissão de log estruturado ہوتا ہے تاکہ Correlação Loki/Tempo ہو سکے۔ |

Eventos `telemetry::sorafs.gateway.request` Contadores OTEL کو cargas úteis estruturadas کے ساتھ espelho کرتے ہیں, Correlação Loki/Tempo کے لیے `endpoint`, `method`, `variant`, `status`, `error_code` e `duration_ms` دکھاتے ہیں جبکہ dashboards Rastreamento SLO کے لیے Série OTLP استعمال کرتے ہیں۔

### Telemetria de comprovação de saúde| Métrica | Tipo | Etiquetas | Notas |
|--------|------|--------|-------|
| `torii_sorafs_proof_health_alerts_total` | Contador | `provider_id`, `trigger`, `penalty` | جب بھی `RecordCapacityTelemetry` ایک `SorafsProofHealthAlert` emitir کرے ou incrementar ہوتا ہے۔ `trigger` PDP/PoTR/ambas as falhas کیا۔ |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Medidor | `provider_id` | janela de telemetria ofensiva. Contagens de PDP/PoTR. کیا۔ |
| `torii_sorafs_proof_health_penalty_nano` | Medidor | `provider_id` | Alerta آخری پر barra ہونے والی Nano-XOR مقدار (cooldown نے supressão de aplicação کیا تو صفر). |
| `torii_sorafs_proof_health_cooldown` | Medidor | `provider_id` | Medidor booleano (`1` = resfriamento de alerta نے suprimir کیا) تاکہ alertas de acompanhamento وقتی طور پر mute ہونے پر دکھایا جا سکے۔ |
| `torii_sorafs_proof_health_window_end_epoch` | Medidor | `provider_id` | alerta سے منسلک janela de telemetria کا época تاکہ operadores Norito artefatos سے correlação کر سکیں۔ |

یہ feeds اب Painel do visualizador Taikai کی linha de prova de saúde کو چلاتے ہیں
(`dashboards/grafana/taikai_viewer.json`), جس سے Operadores CDN کو volumes de alerta, combinação de gatilhos PDP/PoTR, penalidades e estado de resfriamento فی provedor کی visibilidade ao vivo ملتی ہے۔

یہی métricas اب visualizador Taikai کے دو regras de alerta کو سپورٹ کرتے ہیں:
`SorafsProofHealthPenalty` é fogo e fogo
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` Cartão
گزشتہ 15 منٹ میں اضافہ ہو، جبکہ `SorafsProofHealthCooldown` aviso دیتا ہے اگر
provedor پانچ منٹ تک cooldown میں رہے۔ Alertas de segurança
`dashboards/alerts/taikai_viewer_rules.yml` میں موجود ہیں تاکہ SREs e PoR/PoTR
aplicação بڑھنے پر فوری contexto ملے۔

### Superfícies do orquestrador| Métrica/Evento | Tipo | Etiquetas | Produtor | Notas |
|----------------|------|--------|----------|-------|
| `sorafs_orchestrator_active_fetches` | Medidor | `manifest_id`, `region` | `FetchMetricsCtx` | موجودہ sessões a bordo۔ |
| `sorafs_orchestrator_fetch_duration_ms` | Histograma | `manifest_id`, `region` | `FetchMetricsCtx` | histograma de duração (milissegundos)؛ 1 ms ou 30 s baldes۔ |
| `sorafs_orchestrator_fetch_failures_total` | Contador | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Razões: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Contador | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | tentar novamente causa فرق کرتا ہے (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Contador | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | captura de registros de falha/desativação em nível de sessão کرتا ہے۔ |
| `sorafs_orchestrator_chunk_latency_ms` | Histograma | `manifest_id`, `provider_id` | `FetchMetricsCtx` | distribuição de latência de busca por bloco (ms), taxa de transferência/análise de SLO |
| `sorafs_orchestrator_bytes_total` | Contador | `manifest_id`, `provider_id` | `FetchMetricsCtx` | manifesto/provedor کے حساب سے bytes entregues؛ PromQL میں `rate()` کے ذریعے taxa de transferência نکالیں۔ |
| `sorafs_orchestrator_stalls_total` | Contador | `manifest_id`, `provider_id` | `FetchMetricsCtx` | `ScoreboardConfig::latency_cap_ms` é um pedaço de madeira e pedaços de ouro |
| `telemetry::sorafs.fetch.lifecycle` | Evento estruturado | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | ciclo de vida do trabalho (iniciar/concluir) کو Carga útil JSON Norito کے ساتھ espelho کرتا ہے۔ |
| `telemetry::sorafs.fetch.retry` | Evento estruturado | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | sequência de novas tentativas do provedor کے لیے emitir ہوتا ہے؛ `attempts` tentativas incrementais شمار کرتا ہے (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | Evento estruturado | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Limite de falha do provedor cruzado کرے تو ظاہر ہوتا ہے۔ |
| `telemetry::sorafs.fetch.error` | Evento estruturado | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | registro de falha de terminal, ingestão de Loki/Splunk کے لیے مناسب۔ |
| `telemetry::sorafs.fetch.stall` | Evento estruturado | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | limite de latência de pedaço configurado سے بڑھنے پر emit ہوتا ہے (contadores de estol کو espelho کرتا ہے). |

### Nó/superfícies de replicação| Métrica | Tipo | Etiquetas | Notas |
|--------|------|--------|-------|
| `sorafs_node_capacity_utilisation_pct` | Histograma | `provider_id` | porcentagem de utilização de armazenamento no histograma OTEL ( `_bucket/_sum/_count` کے طور پر export ). |
| `sorafs_node_por_success_total` | Contador | `provider_id` | instantâneos do agendador سے amostras PoR derivadas de sucesso کا contador monotônico۔ |
| `sorafs_node_por_failure_total` | Contador | `provider_id` | amostras PoR com falha کا contador monotônico۔ |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Medidor | `provider` | bytes usados, profundidade da fila e contagens de voo PoR کے لیے موجودہ medidores Prometheus۔ |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Medidor | `provider` | dados de sucesso de capacidade/tempo de atividade do provedor ou painel de capacidade میں دکھایا جاتا ہے۔ |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Medidor | `provider`, `manifest` | profundidade do backlog اور contadores de falhas cumulativas جو ہر `/v1/sorafs/por/ingestion/{manifest}` poll پر exportação ہوتے ہیں, painel/alerta "PoR Stalls" کو feed کرتے ہیں۔ |

### Prova de recuperação oportuna (PoTR) e pedaço de SLA

| Métrica | Tipo | Etiquetas | Produtor | Notas |
|----|------|--------|----------|-------|
| `sorafs_potr_deadline_ms` | Histograma | `tier`, `provider` | Coordenador PoTR | folga de prazo milissegundos میں (positivo = cumprido). |
| `sorafs_potr_failures_total` | Contador | `tier`, `provider`, `reason` | Coordenador PoTR | Razões: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Contador | `provider`, `manifest_id`, `reason` | Monitor de SLA | جب entrega de pedaços SLO miss کرے تو fire ہوتا ہے (latência, taxa de sucesso). |
| `sorafs_chunk_sla_violation_active` | Medidor | `provider`, `manifest_id` | Monitor de SLA | Medidor booleano (0/1) جو janela de violação ativa میں alternar ہوتا ہے۔ |

## Metas de SLO

- Disponibilidade sem confiança do gateway: **99,9%** (respostas HTTP 2xx/304).
- Trustless TTFB P95: camada quente ≤ 120 ms, camada quente ≤ 300 ms.
- Taxa de sucesso da prova: ≥ 99,5% ao dia.
- Sucesso do orquestrador (conclusão do bloco): ≥ 99%.

## Painéis e alertas

1. **Observabilidade do gateway** (`dashboards/grafana/sorafs_gateway_observability.json`) — disponibilidade confiável, TTFB P95, quebra de recusa e falhas PoR/PoTR کو métricas OTEL کے ذریعے ٹریک کرتا ہے۔
2. **Orchestrator Health** (`dashboards/grafana/sorafs_fetch_observability.json`) — carregamento de múltiplas fontes, novas tentativas, falhas de provedor e bursts de paralisação کو کور کرتا ہے۔
3. **SoraNet Privacy Metrics** (`dashboards/grafana/soranet_privacy_metrics.json`) — buckets de retransmissão anonimizados, janelas de supressão e integridade do coletor کو `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` e `soranet_privacy_poll_errors_total{provider}` کے ذریعے چارٹ کرتا ہے۔

Pacotes de alerta:- `dashboards/alerts/sorafs_gateway_rules.yml` — disponibilidade de gateway, TTFB, picos de falha de prova۔
- `dashboards/alerts/sorafs_fetch_rules.yml` — falhas/novas tentativas/paralisações do orquestrador; `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml`, e `dashboards/alerts/tests/soranet_policy_rules.test.yml` são validados
- `dashboards/alerts/soranet_privacy_rules.yml` — picos de downgrade de privacidade, alarmes de supressão, detecção de coletor ocioso e alertas de coletor desativado (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — alarmes de queda de energia anonimato جو `sorafs_orchestrator_brownouts_total` سے com fio ہیں۔
- `dashboards/alerts/taikai_viewer_rules.yml` - Alarmes de desvio / ingestão / atraso CEK do visualizador Taikai

## Estratégia de rastreamento

- OpenTelemetry é uma ferramenta de ponta a ponta:
  - Gateways OTLP spans (HTTP) emitem کرتے ہیں جن پر IDs de solicitação, resumos de manifesto e hashes de token ہوتے ہیں۔
  - orquestrador `tracing` + `opentelemetry` استعمال کر کے tentativas de busca کے spans exportação کرتا ہے۔
  - Nós SoraFS incorporados PoR desafia اور operações de armazenamento کے abrange exportação کرتے ہیں۔ Os componentes `x-sorafs-trace` são propagados e compartilhados com ID de rastreamento comum.
- Métricas do orquestrador `SorafsFetchOtel` کو Histogramas OTLP میں bridge کرتا ہے جبکہ `telemetry::sorafs.fetch.*` backends centrados em log de eventos کے لیے cargas úteis JSON leves فراہم کرتے ہیں۔
- Colecionadores: Colecionadores OTEL کو Prometheus/Loki/Tempo کے ساتھ چلائیں (Tempo preferido). Exportadores de API Jaeger اختیاری رہتے ہیں۔
- Operações de alta cardinalidade کو amostra کریں (caminhos de sucesso کے لیے 10%, falhas کے لیے 100%).

## Coordenação de Telemetria TLS (SF-5b)

- Alinhamento métrico:
  - Automação TLS `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}`, e `sorafs_gateway_tls_ech_enabled` بھیجتی ہے۔
  - ان medidores کو Painel de visão geral do gateway میں Painel TLS/Certificados کے تحت شامل کریں۔
- Ligação de alerta:
  - جب Alertas de expiração TLS disparam ہوں (≤ 14 dias restantes) تو disponibilidade confiável SLO کے ساتھ correlacionar کریں۔
  - Desativação ECH ایک emissão de alerta secundário کرتا ہے جو TLS اور disponibilidade دونوں painéis کو referência کرتا ہے۔
- Pipeline: trabalho de automação TLS اسی pilha Prometheus پر exportação کرتا ہے جس پر métricas de gateway ہیں؛ SF-5b کے ساتھ instrumentação desduplicada de coordenação یقینی بناتی ہے۔

## Nomenclatura de métricas e convenções de rótulos- Nomes de métricas موجودہ `torii_sorafs_*` یا `sorafs_*` prefixos کو follow کرتے ہیں جو Torii اور gateway استعمال کرتے ہیں۔
- Conjuntos de etiquetas padronizados ہیں:
  - `result` → Resultado HTTP (`success`, `refused`, `failed`).
  - `reason` → código de recusa/erro (`unsupported_chunker`, `timeout`, etc.).
  - `provider` → identificador de provedor com codificação hexadecimal۔
  - `manifest` → resumo do manifesto canônico (alta cardinalidade میں trim کیا جاتا ہے). 
  - `tier` → rótulos de camada declarativa (`hot`, `warm`, `archive`).
- Pontos de emissão de telemetria:
  - Métricas de gateway `torii_sorafs_*` کے تحت رہتے ہیں اور `crates/iroha_core/src/telemetry.rs` کی reutilização de convenções کرتے ہیں۔
  - métricas do orquestrador `sorafs_orchestrator_*` e eventos `telemetry::sorafs.fetch.*` (ciclo de vida, nova tentativa, falha do provedor, erro, parada) emitem کرتا ہے جن پر manifest digest, ID do trabalho, região e tags de identificadores do provedor ہوتے ہیں۔
  - Nós `torii_sorafs_storage_*`, `torii_sorafs_capacity_*`, e `torii_sorafs_por_*` دکھاتے ہیں۔
- Observabilidade کے ساتھ coordenada کریں تاکہ catálogo métrico کو documento de nomenclatura Prometheus compartilhado میں registro کیا جائے, جس میں expectativas de cardinalidade do rótulo (limites superiores do provedor/manifesto) شامل ہوں۔

## Pipeline de dados

- Coletores ہر componente کے ساتھ implantar ہوتے ہیں، OTLP کو Prometheus (métricas) اور Loki/Tempo (logs/traces) پر exportar کرتے ہیں۔
- Gateways/nós opcionais eBPF (Tetragon) کے لیے rastreamento de baixo nível کو enriquecer کرتا ہے۔
- `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` کو Torii اور nós incorporados کے لیے استعمال کریں؛ orquestrador `install_sorafs_fetch_otlp_exporter` کو کال کرتا رہتا ہے۔

## Ganchos de validação

- CI کے دوران `scripts/telemetry/test_sorafs_fetch_alerts.sh` چلائیں تاکہ Prometheus regras de alerta paralisam métricas اور verificações de supressão de privacidade کے ساتھ lockstep رہیں۔
- Painéis Grafana e controle de versão (`dashboards/grafana/`) کے تحت رکھیں اور painéis میں تبدیلی پر capturas de tela/links اپڈیٹ کریں۔
- Exercícios de caos کے نتائج `scripts/telemetry/log_sorafs_drill.sh` کے ذریعے log ہوتے ہیں؛ validação `scripts/telemetry/validate_drill_log.sh` استعمال کرتی ہے (دیکھیے [Manual de Operações](operations-playbook.md)).