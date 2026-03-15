---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/soranet/privacy-metrics-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b86f9abaf85d418ffdbdd6fc4d62f4b736d0bdedb5bb2a0783c86a67380da14e
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: privacy-metrics-pipeline
lang: pt
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fonte canonica
Reflete `docs/source/soranet/privacy_metrics_pipeline.md`. Mantenha ambas as copias sincronizadas.
:::

# Pipeline de metricas de privacidade da SoraNet

SNNet-8 introduz uma superficie de telemetria consciente de privacidade para o runtime do relay. O relay agora agrega eventos de handshake e circuit em buckets de um minuto e exporta apenas contadores Prometheus grossos, mantendo circuits individuais desvinculados enquanto fornece visibilidade acionavel aos operadores.

## Visao geral do agregador

- A implementacao do runtime fica em `tools/soranet-relay/src/privacy.rs` como `PrivacyAggregator`.
- Buckets sao chaveados por minuto de relogio (`bucket_secs`, default 60 segundos) e armazenados em um ring limitado (`max_completed_buckets`, default 120). Collector shares mantem seu proprio backlog limitado (`max_share_lag_buckets`, default 12) para que janelas Prio antigas sejam esvaziadas como buckets suprimidos em vez de vazar memoria ou mascarar collectors presos.
- `RelayConfig::privacy` mapeia direto para `PrivacyConfig`, expondo knobs de ajuste (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`). O runtime de producao mantem os defaults enquanto SNNet-8a introduz thresholds de agregacao segura.
- Os modulos de runtime registram eventos via helpers tipados: `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`, `record_verified_bytes`, e `record_gar_category`.

## Endpoint admin do relay

Os operadores podem consultar o listener admin do relay para observacoes brutas via `GET /privacy/events`. O endpoint retorna JSON delimitado por novas linhas (`application/x-ndjson`) contendo payloads `SoranetPrivacyEventV1` espelhados do `PrivacyEventBuffer` interno. O buffer guarda os eventos mais novos ate `privacy.event_buffer_capacity` entradas (default 4096) e e drenado na leitura, entao scrapers devem sondar com frequencia suficiente para evitar lacunas. Os eventos cobrem os mesmos sinais de handshake, throttle, verified bandwidth, active circuit e GAR que alimentam os contadores Prometheus, permitindo a collectors downstream arquivar breadcrumbs seguros para privacidade ou alimentar workflows de agregacao segura.

## Configuracao do relay

Os operadores ajustam a cadencia de telemetria de privacidade no arquivo de configuracao do relay via a secao `privacy`:

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

Os defaults dos campos correspondem a especificacao SNNet-8 e sao validados no carregamento:

| Campo | Descricao | Padrao |
|-------|-----------|--------|
| `bucket_secs` | Largura de cada janela de agregacao (segundos). | `60` |
| `min_handshakes` | Numero minimo de contribuidores antes de um bucket poder emitir contadores. | `12` |
| `flush_delay_buckets` | Buckets completos a esperar antes de tentar um flush. | `1` |
| `force_flush_buckets` | Idade maxima antes de emitir um bucket suprimido. | `6` |
| `max_completed_buckets` | Backlog de buckets retidos (impede memoria sem limite). | `120` |
| `max_share_lag_buckets` | Janela de retencao para collector shares antes de suprimir. | `12` |
| `expected_shares` | Prio collector shares exigidos antes de combinar. | `2` |
| `event_buffer_capacity` | Backlog de eventos NDJSON para o stream admin. | `4096` |

Definir `force_flush_buckets` menor que `flush_delay_buckets`, zerar os thresholds, ou desativar o guard de retencao agora falha na validacao para evitar deployments que vazariam telemetria por relay.

O limite `event_buffer_capacity` tambem limita `/admin/privacy/events`, garantindo que scrapers nao possam ficar atrasados indefinidamente.

## Prio collector shares

SNNet-8a implanta collectors duplos que emitem buckets Prio com compartilhamento secreto. O orchestrator agora analisa o stream NDJSON `/privacy/events` para entradas `SoranetPrivacyEventV1` e shares `SoranetPrivacyPrioShareV1`, encaminhando-as para `SoranetSecureAggregator::ingest_prio_share`. Buckets emitem quando chegam `PrivacyBucketConfig::expected_shares` contribuicoes, espelhando o comportamento do relay. As shares sao validadas para alinhamento de bucket e forma do histograma antes de serem combinadas em `SoranetPrivacyBucketMetricsV1`. Se a contagem combinada de handshakes ficar abaixo de `min_contributors`, o bucket e exportado como `suppressed`, espelhando o comportamento do agregador no relay. Janelas suprimidas agora emitem um label `suppression_reason` para que operadores possam distinguir entre `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed`, e `forced_flush_window_elapsed` ao diagnosticar lacunas de telemetria. O motivo `collector_window_elapsed` tambem dispara quando Prio shares ficam alem de `max_share_lag_buckets`, tornando collectors presos visiveis sem deixar acumuladores antigos na memoria.

## Endpoints de ingestao do Torii

Torii agora expone dois endpoints HTTP com gating de telemetria para que relays e collectors possam encaminhar observacoes sem embutir um transporte bespoke:

- `POST /v2/soranet/privacy/event` aceita um payload `RecordSoranetPrivacyEventDto`. O corpo envolve um `SoranetPrivacyEventV1` mais um label `source` opcional. Torii valida a requisicao contra o perfil de telemetria ativo, registra o evento, e responde com HTTP `202 Accepted` junto com um envelope Norito JSON contendo a janela calculada (`bucket_start_unix`, `bucket_duration_secs`) e o modo do relay.
- `POST /v2/soranet/privacy/share` aceita um payload `RecordSoranetPrivacyShareDto`. O corpo carrega um `SoranetPrivacyPrioShareV1` e uma dica `forwarded_by` opcional para que operadores possam auditar fluxos de collectors. Submissoes bem-sucedidas retornam HTTP `202 Accepted` com um envelope Norito JSON resumindo o collector, a janela de bucket e a dica de supressao; falhas de validacao mapeiam para uma resposta de telemetria `Conversion` para preservar tratamento deterministico de erros entre collectors. O loop de eventos do orchestrator agora emite essas shares ao fazer polling dos relays, mantendo o acumulador Prio do Torii sincronizado com os buckets no relay.

Ambos os endpoints respeitam o perfil de telemetria: emitem `503 Service Unavailable` quando as metricas estao desativadas. Clientes podem enviar corpos Norito binary (`application/x.norito`) ou Norito JSON (`application/x.norito+json`); o servidor negocia automaticamente o formato via extractors padrao do Torii.

## Metricas Prometheus

Cada bucket exportado carrega labels `mode` (`entry`, `middle`, `exit`) e `bucket_start`. As seguintes familias de metricas sao emitidas:

| Metric | Description |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | Taxonomia de handshake com `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | Contadores de throttle com `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Duracoes de cooldown agregadas por handshakes throttled. |
| `soranet_privacy_verified_bytes_total` | Bandwidth verificada de provas de medicao cega. |
| `soranet_privacy_active_circuits_{avg,max}` | Media e pico de circuits ativos por bucket. |
| `soranet_privacy_rtt_millis{percentile}` | Estimativas de percentil RTT (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Contadores de Governance Action Report com hash por digest de categoria. |
| `soranet_privacy_bucket_suppressed` | Buckets retidos porque o limiar de contribuidores nao foi atingido. |
| `soranet_privacy_pending_collectors{mode}` | Acumuladores de collector shares pendentes de combinacao, agrupados por modo de relay. |
| `soranet_privacy_suppression_total{reason}` | Contadores de buckets suprimidos com `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` para que dashboards atribuam lacunas de privacidade. |
| `soranet_privacy_snapshot_suppression_ratio` | Razao suprimida/drenada do ultimo drain (0-1), util para budgets de alerta. |
| `soranet_privacy_last_poll_unixtime` | Timestamp UNIX do ultimo poll bem-sucedido (alimenta o alerta collector-idle). |
| `soranet_privacy_collector_enabled` | Gauge que vira `0` quando o collector de privacidade esta desativado ou falha ao iniciar (alimenta o alerta collector-disabled). |
| `soranet_privacy_poll_errors_total{provider}` | Falhas de polling agrupadas por alias de relay (incrementa em erros de decode, falhas HTTP, ou status codes inesperados). |

Buckets sem observacoes permanecem silenciosos, mantendo dashboards limpos sem fabricar janelas zeradas.

## Orientacao operacional

1. **Dashboards** - trace as metricas acima agrupadas por `mode` e `window_start`. Destaque janelas ausentes para revelar problemas de collector ou relay. Use `soranet_privacy_suppression_total{reason}` para distinguir falta de contribuidores de supressoes orientadas por collectors ao triar lacunas. O asset Grafana agora envia um painel dedicado **"Suppression Reasons (5m)"** alimentado por esses contadores mais um stat **"Suppressed Bucket %"** que calcula `sum(soranet_privacy_bucket_suppressed) / count(...)` por selecao para que operadores vejam violacoes de budget rapidamente. A serie **"Collector Share Backlog"** (`soranet_privacy_pending_collectors`) e o stat **"Snapshot Suppression Ratio"** destacam collectors presos e desvio de budget durante execucoes automatizadas.
2. **Alerting** - conduza alarmes a partir de contadores seguros de privacidade: picos de PoW reject, frequencia de cooldown, drift de RTT e capacity rejects. Como os contadores sao monotonos dentro de cada bucket, regras simples baseadas em taxa funcionam bem.
3. **Incident response** - confie primeiro nos dados agregados. Quando for necessario debug mais profundo, solicite que relays reproduzam snapshots de buckets ou inspecionem provas de medicao cega em vez de coletar logs de trafego bruto.
4. **Retention** - faca scrape com frequencia suficiente para evitar exceder `max_completed_buckets`. Exporters devem tratar a saida Prometheus como fonte canonica e descartar buckets locais depois de encaminhados.

## Analise de supressao e execucoes automatizadas

A aceitacao de SNNet-8 depende de demonstrar que collectors automatizados permanecem saudaveis e que a supressao fica dentro dos limites da politica (<=10% dos buckets por relay em qualquer janela de 30 minutos). O tooling necessario para cumprir esse gate agora vem com o repositorio; operadores devem integrar isso aos seus rituais semanais. Os novos paineis de supressao do Grafana refletem os trechos PromQL abaixo, dando as equipes de plantao visibilidade ao vivo antes que precisem recorrer a consultas manuais.

### Receitas PromQL para revisao de supressao

Operadores devem manter os seguintes helpers PromQL a mao; ambos sao referenciados no dashboard Grafana compartilhado (`dashboards/grafana/soranet_privacy_metrics.json`) e nas regras do Alertmanager:

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

Use a saida do ratio para confirmar que o stat **"Suppressed Bucket %"** permanece abaixo do budget de politica; conecte o detector de spikes ao Alertmanager para feedback rapido quando a contagem de contribuidores cair inesperadamente.

### CLI de relatorio de bucket offline

O workspace expoe `cargo xtask soranet-privacy-report` para capturas NDJSON pontuais. Aponte para um ou mais exports admin de relay:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

O helper passa a captura pelo `SoranetSecureAggregator`, imprime um resumo de supressao no stdout e, opcionalmente, grava um relatorio JSON estruturado via `--json-out <path|->`. Ele honra os mesmos knobs do collector ao vivo (`--bucket-secs`, `--min-contributors`, `--expected-shares`, etc.), permitindo que operadores reproduzam capturas historicas sob thresholds diferentes ao triar um incidente. Anexe o JSON junto com capturas do Grafana para que o gate de analise de supressao SNNet-8 permanece auditavel.

### Checklist da primeira execucao automatizada

A governanca ainda exige provar que a primeira execucao automatizada atendeu ao budget de supressao. O helper agora aceita `--max-suppression-ratio <0-1>` para que CI ou operadores falhem rapidamente quando buckets suprimidos excederem a janela permitida (default 10%) ou quando ainda nao houver buckets. Fluxo recomendado:

1. Exporte NDJSON dos endpoints admin do relay mais o stream `/v2/soranet/privacy/event|share` do orchestrator para `artifacts/sorafs_privacy/<relay>.ndjson`.
2. Rode o helper com o budget de politica:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   O comando imprime a razao observada e sai com codigo nao zero quando o budget e excedido **ou** quando ainda nao ha buckets prontos, sinalizando que a telemetria ainda nao foi produzida para a execucao. As metricas ao vivo devem mostrar `soranet_privacy_pending_collectors` drenando para zero e `soranet_privacy_snapshot_suppression_ratio` ficando abaixo do mesmo budget enquanto a execucao ocorre.
3. Arquive a saida JSON e o log da CLI com o pacote de evidencias SNNet-8 antes de trocar o default do transporte para que revisores possam reproduzir os artefatos exatos.

## Proximos passos (SNNet-8a)

- Integrar os dual Prio collectors, conectando a ingestao de shares ao runtime para que relays e collectors emitam payloads `SoranetPrivacyBucketMetricsV1` consistentes. *(Concluido - veja `ingest_privacy_payload` em `crates/sorafs_orchestrator/src/lib.rs` e os testes associados.)*
- Publicar o dashboard Prometheus compartilhado e regras de alerta cobrindo lacunas de supressao, saude dos collectors e quedas de anonimato. *(Concluido - veja `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` e fixtures de validacao.)*
- Produzir os artefatos de calibracao de privacidade diferencial descritos em `privacy_metrics_dp.md`, incluindo notebooks reproduziveis e digests de governanca. *(Concluido - notebook e artefatos gerados por `scripts/telemetry/run_privacy_dp.py`; wrapper CI `scripts/telemetry/run_privacy_dp_notebook.sh` executa o notebook via o workflow `.github/workflows/release-pipeline.yml`; digest de governanca arquivado em `docs/source/status/soranet_privacy_dp_digest.md`.)*

A release atual entrega a base do SNNet-8: telemetria deterministica e segura para privacidade que se encaixa diretamente nos scrapers e dashboards Prometheus existentes. Os artefatos de calibracao de privacidade diferencial estao no lugar, o workflow do release pipeline mantem os outputs do notebook atualizados, e o trabalho restante foca no monitoramento da primeira execucao automatizada e na extensao das analises de alerta de supressao.
