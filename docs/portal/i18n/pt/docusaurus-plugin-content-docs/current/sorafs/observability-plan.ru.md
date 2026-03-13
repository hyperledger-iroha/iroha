---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de observabilidade
título: Plano de instalação e SLO para SoraFS
sidebar_label: Nome e SLO
description: Conjunto de telefone, painel de controle e conjunto de política para gateways SoraFS, узлов e мульти-istoчникового orquestrador.
---

:::nota História Canônica
Esta página está executando o plano, disponível em `docs/source/sorafs_observability_plan.md`. Faça uma cópia sincronizada, mas a estrela do Sphinx não migrou.
:::

##Céli
- Definindo métricas e estruturas para gateways, usuários e múltiplos operadores.
- Verifique o Grafana, usando alertas e ganchos de validação.
- Зафиксировать цели SLO вместе с политиками бюджета ошибок и хаос-дриллами.

## Catálogo de métricas

### Gateway de segurança

| Métrica | Tipo | Metica | Nomeação |
|--------|-----|-------|------------|
| `sorafs_gateway_active` | Medidor (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | Emitiry через `SorafsGatewayOtel`; exclua a operação HTTP na combinação de endpoint/método. |
| `sorafs_gateway_responses_total` | Contador | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Каждая завершенная gateway-запросом операция увеличивает счетчик один раз; `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histograma | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Tempo lento para o primeiro byte para gateway aberto; экспортируется как Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Contador | `profile_version`, `result`, `error_code` | As configurações fornecem uma imagem gráfica no momento da compra (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histograma | `profile_version`, `result`, `error_code` | Распределение латентности проверки para PoR-реплик. |
| `telemetry::sorafs.gateway.request` | Estruturação de projetos | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Структурированный лог, эмитируемый при завершении каждого запроса для корреляции Loki/Tempo. |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Contador | Aço inoxidável | Métrica Prometheus, projetada para a história dos EUA; эмитируются вместе com a nova série OTLP. |

События `telemetry::sorafs.gateway.request` отражают OTEL-счетчики со структурированными cargas úteis, por exemplo `endpoint`, `method`, `variant`, `status`, `error_code` e `duration_ms` para conexão em Loki/Tempo, tanto quanto possível. série OTLP para aprovação de SLO.

### Телеметрия здоровья доказательств| Métrica | Tipo | Metica | Nomeação |
|--------|-----|-------|------------|
| `torii_sorafs_proof_health_alerts_total` | Contador | `provider_id`, `trigger`, `penalty` | Инкрементируется каждый раз, когда `RecordCapacityTelemetry` эмитирует `SorafsProofHealthAlert`. `trigger` различает сбои PDP/PoTR/Both, e `penalty` показывает, был ли коллатерал реально списан ou подавлен cooldown. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Medidor | `provider_id` | Posicionar o PDP/PoTR em um problema de telefonia, fazer com que os comandos possam ser configurados, sem problemas превысили политику. |
| `torii_sorafs_proof_health_penalty_nano` | Medidor | `provider_id` | Summa Nano-XOR, especificado em alerta adicional (não, o tempo de espera não é permitido). |
| `torii_sorafs_proof_health_cooldown` | Medidor | `provider_id` | Medidor grande (`1` = cooldown de alerta), чтобы показать, когда последующие алерты временно приглушены. |
| `torii_sorafs_proof_health_window_end_epoch` | Medidor | `provider_id` | A tela de teste é compatível com o alerta, os operadores podem conectar o artefato Norito. |

Эти фиды теперь питают строку prova-saúde em дашборде Taikai viewer
(`dashboards/grafana/taikai_viewer.json`), seu operador CDN está executando o vídeo
ativando alertas, микса триггеров PDP/PoTR, penalidades e tempo de espera para
prova.

Esta é uma métrica que pode fornecer alertas de Taikai:
`SorafsProofHealthPenalty` срабатывает, когда
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` testado
em 15 minutos, um aviso de aviso `SorafsProofHealthCooldown`, exceto
провайдер остается в cooldown por um minuto. Оба алерта живут в
`dashboards/alerts/taikai_viewer_rules.yml`, чтобы SREs por contato
немедленно при эскалации принуждения PoR/PoTR.

### Orquestrador Operacional| Métrica / Событие | Tipo | Metica | Proibido | Nomeação |
|-------------------|-----|-------|---------------|------------|
| `sorafs_orchestrator_active_fetches` | Medidor | `manifest_id`, `region` | `FetchMetricsCtx` | Sessão, coloque-a no lugar. |
| `sorafs_orchestrator_fetch_duration_ms` | Histograma | `manifest_id`, `region` | `FetchMetricsCtx` | Гистограмма длительности в миллисекундах; baldes de 1 ms a 30 s. |
| `sorafs_orchestrator_fetch_failures_total` | Contador | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Exemplos: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Contador | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Selecione os itens retornados (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Contador | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Фиксирует отключения и счетчики отказов на уровне сессий. |
| `sorafs_orchestrator_chunk_latency_ms` | Histograma | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Use a busca de dados latente por pedaço (ms) para analisar a taxa de transferência/SLO. |
| `sorafs_orchestrator_bytes_total` | Contador | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Байты, enviado para manifesto/provedor; taxa de transferência é igual a `rate()` no PromQL. |
| `sorafs_orchestrator_stalls_total` | Contador | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Separe os pedaços, verificando `ScoreboardConfig::latency_cap_ms`. |
| `telemetry::sorafs.fetch.lifecycle` | Estruturação de projetos | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Execute o ciclo de trabalho (iniciar/concluir) com carga útil JSON Norito. |
| `telemetry::sorafs.fetch.retry` | Estruturação de projetos | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Эмитируется на каждую серию ретраев провайдера; `attempts` retorna o índice (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | Estruturação de projetos | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Публикуется, когда провайдер пересекает порог отказов. |
| `telemetry::sorafs.fetch.error` | Estruturação de projetos | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | A última opção é obtida para ser ingerida no Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | Estruturação de projetos | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Выдается, когда латентность chunk превышает настроенный лимит (отражает stall-счетчики). |

### Поверхности узлов / репликации| Métrica | Tipo | Metica | Nomeação |
|--------|-----|-------|------------|
| `sorafs_node_capacity_utilisation_pct` | Histograma | `provider_id` | O OTEL-Gистограмма fornece serviços de entrega (exportador como `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Contador | `provider_id` | Монотонный счетчик успешных PoR-выборок, полученных из agendador de snapshot. |
| `sorafs_node_por_failure_total` | Contador | `provider_id` | Монотонный счетчик неуспешных PoR-выборок. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Medidor | `provider` | Medidor Prometheus compatível para uso em baterias, fones de ouvido, PoR em voo. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Medidor | `provider` | É necessário usar um provedor de capacidade/tempo de atividade, instalado no mercado doméstico. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Medidor | `provider`, `manifest` | Глубина backlog mais a lista de pendências do servidor, transferência para o local após `/v2/sorafs/por/ingestion/{manifest}`, питают painel/алерт "PoR Stalls". |

### Prova de recuperação oportuna (PoTR) e SLA por pedaços

| Métrica | Tipo | Metica | Proibido | Nomeação |
|--------|-----|-------|---------------|------------|
| `sorafs_potr_deadline_ms` | Histograma | `tier`, `provider` | Coordenador PoTR | Vá para o número de milissegundos (положительный = выполнен). |
| `sorafs_potr_failures_total` | Contador | `tier`, `provider`, `reason` | Coordenador PoTR | Exemplos: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Contador | `provider`, `manifest_id`, `reason` | Monitor de SLA | Claro, o envio de pedaços não é usado no SLO (latеntность, taxa de sucesso). |
| `sorafs_chunk_sla_violation_active` | Medidor | `provider`, `manifest_id` | Monitor de SLA | Medidor grande (0/1), ajustado no momento da ativação. |

## Цели SLO

- Gateway de entrega confiável: **99,9%** (HTTP 2xx/304 ответы).
- Trustless TTFB P95: camada quente ≤ 120 ms, camada quente ≤ 300 ms.
- Taxa de transferência: ≥ 99,5% em dezembro.
- Успех оркестратора (pedaços de expansão): ≥ 99%.

## Дашборды e алерты

1. **Gateway Gateway** (`dashboards/grafana/sorafs_gateway_observability.json`) — отслеживает trustless-доступность, TTFB P95, разбиение отказов e сбои PoR/PoTR через метрики OTEL.
2. **Здоровье оркестратора** (`dashboards/grafana/sorafs_fetch_observability.json`) — покрывает нагрузку multi-fonte, ретраи, отказы провайдеров e всплески barracas.
3. **Метрики приватности SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — графики анонимизированных relé-baldes, окон подавления e здоровья coletor через `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` e `soranet_privacy_poll_errors_total{provider}`.

Alertas de pacotes:- `dashboards/alerts/sorafs_gateway_rules.yml` — gateway de entrega, TTFB, всплески отказов доказательств.
- `dashboards/alerts/sorafs_fetch_rules.yml` — отказы/ретраи/stalls оркестратора; Verifique os valores `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` e `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` — всплески деградации приватности, alarmes подавления, детекция coletor de inatividade e alertas de coletor aberto (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — alarmes de quedas de energia anônimas, привязанные к `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` — alarmes дрейфа/ingestão/CEK lag Visualizador Taikai плюс новые алерты penalidade/recarga здоровья доказательств SoraFS, основанные на `torii_sorafs_proof_health_*`.

## Estratégia de Transmissão

- Usando OpenTelemetry de ponta a ponta:
  - Gateways usam spans OTLP (HTTP) com IDs de solicitação anotados, resumos de manifestos e hashes de token.
  - O operador usa `tracing` + `opentelemetry` para o transporte abrange a busca.
  - Встроенные SoraFS-узлы экспортируют spans para PoR-челленджей e armazenamento-операций. Todos os componentes devem obter o ID de rastreamento, executando o `x-sorafs-trace`.
- `SorafsFetchOtel` contém métricas de orquestração em OTLP-гистограммы, e a solução `telemetry::sorafs.fetch.*` fornece cargas úteis JSON legíveis para back-ends de log-ориентированных.
-Coletores: Coletores OTEL запускайте рядом с Prometheus/Loki/Tempo (Tempo предпочтителен). Экспортеры Jaeger-совместимые остаются опциональными.
- Операции с высокой кардинальностью следует сэмплировать (10% para успешных путей, 100% para отказов).

## Coordenação de telefonia TLS (SF-5b)

- Métrica de verificação:
  - Automação TLS publicada `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` e `sorafs_gateway_tls_ech_enabled`.
  - Exibir medidores no painel Visão geral do gateway no painel TLS/Certificados.
- Seus alertas:
  - Когда срабатывают алерты истечения TLS (≤ 14 дней осталось), коррелируйте с SLO de disponibilidade confiável.
  - Отключение ECH эмитирует вторичный алерт, ссылающийся e TLS, e no painel de disponibilidade.
- Pipeline: trabalho de automação TLS transferido para a pilha Prometheus, este e gateway métrico; A coordenação do SF-5b garante a duplicação da instalação.

## Convenções de inovação e método- A unidade métrica fornece o perfil `torii_sorafs_*` ou `sorafs_*`, Torii e gateway.
- Наборы меток стандартизированы:
  - `result` → Resultado HTTP (`success`, `refused`, `failed`).
  - `reason` → código de saída/ошибки (`unsupported_chunker`, `timeout`, etc.).
  - `provider` → testador de identificador codificado em hexadecimal.
  - `manifest` → manifesto de resumo canônico (обрезается при высокой кардинальности).
  - `tier` → camada de configuração de configuração (`hot`, `warm`, `archive`).
- Точки эмиссии телеметрии:
  - O gateway métrico funciona em `torii_sorafs_*` e conecta-se a `crates/iroha_core/src/telemetry.rs`.
  - Оркестратор эмитирует метрики `sorafs_orchestrator_*` e события `telemetry::sorafs.fetch.*` (ciclo de vida, nova tentativa, falha do provedor, erro, paralisação), resumo do manifesto de resumo, ID do trabalho, região e provador de identificação.
  - Use o `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` e `torii_sorafs_por_*`.
- Координируйтесь с Observability, чтобы зарегистрировать каталог метрик общем документе по naming Prometheus, включая ожидания по кардинальности меток (provedor de granizo/manifestos).

## Pipeline de dados

- Os colecionadores usam o conjunto de componentes, exportam OTLP em Prometheus (метрики) e Loki/Tempo (логи/трассы).
- Опциональный eBPF (Tetragon) обогащает низкоуровневую трассировку para gateways/узлов.
- Use `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` para Torii e outros dispositivos; O operador do aparelho é `install_sorafs_fetch_otlp_exporter`.

## Ganchos de validação

- Запускайте `scripts/telemetry/test_sorafs_fetch_alerts.sh` no CI, este alerta de alerta Prometheus fornece sincronização com travas métricas e supressão de teste privado.
- Selecione o Grafana para a versão de controle (`dashboards/grafana/`) e abra a tela / tela de exibição painel de visualização.
- Exercícios de caos логируют результаты через `scripts/telemetry/log_sorafs_drill.sh`; validação de uso `scripts/telemetry/validate_drill_log.sh` (veja [Manual de operação](operations-playbook.md)).