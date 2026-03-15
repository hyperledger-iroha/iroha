---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pipeline de métricas de privacidade
título: Pipeline de métricas de privacidade da SoraNet (SNNet-8)
sidebar_label: Pipeline de métricas de privacidade
descrição: Coleta de telemetria que preserva privacidade para retransmissores e orquestradores da SoraNet.
---

:::nota Fonte canônica
Reflete `docs/source/soranet/privacy_metrics_pipeline.md`. Mantenha ambas as cópias sincronizadas.
:::

# Pipeline de métricas de privacidade da SoraNet

SNNet-8 introduz uma superfície de telemetria consciente de privacidade para o tempo de execução do relé. O relé agora agrega eventos de handshake e circuito em baldes de um minuto e exporta apenas contadores Prometheus grossos, mantendo circuitos individuais desvinculados enquanto fornece visibilidade acionavel aos operadores.

## Visão geral do agregador

- A implementação do runtime fica em `tools/soranet-relay/src/privacy.rs` como `PrivacyAggregator`.
- Buckets são chaveados por minuto de relógio (`bucket_secs`, padrão 60 segundos) e armazenados em um anel limitado (`max_completed_buckets`, padrão 120). Collector share mantem seu proprio backlog limitado (`max_share_lag_buckets`, default 12) para que janelas Prio antigos sejam esvaziados como buckets suprimidos em vez de vazar memória ou mascarar coletores presos.
- `RelayConfig::privacy` mapa direto para `PrivacyConfig`, expondo botões de ajuste (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`). O runtime de produção mantém os padrões enquanto o SNNet-8a introduz limites de agregação segura.
- Os módulos de runtime registram eventos via helpers tipados: `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`, `record_verified_bytes`, e `record_gar_category`.

## Administrador do endpoint faz retransmissão

Os operadores consultam o listener admin do relay para observações brutas podem via `GET /privacy/events`. O endpoint retorna JSON delimitado por novas linhas (`application/x-ndjson`) contendo payloads `SoranetPrivacyEventV1` espelhados do `PrivacyEventBuffer` interno. O buffer guarda os eventos mais novos com entradas `privacy.event_buffer_capacity` (padrão 4096) e é drenado na leitura, então raspadores devem sondar com frequência suficiente para evitar lacunas. Os eventos cobrem os mesmos sinais de handshake, acelerador, largura de banda verificada, circuito ativo e GAR que alimentam os contadores Prometheus, permitindo aos coletores downstream arquivar breadcrumbs seguros para privacidade ou fluxos de trabalho alimentares de agregação segura.

## Configuração do relé

Os operadores ajustam a cadência de telemetria de privacidade no arquivo de configuração do relé através da seção `privacy`:

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

Os padrões dos campos dependem da especificação SNNet-8 e são validados no carregamento:| Campo | Descrição | Padrão |
|-------|-----------|--------|
| `bucket_secs` | Largura de cada janela de agregação (segundos). | `60` |
| `min_handshakes` | Número mínimo de contribuidores antes de um balde de poder emitir contadores. | `12` |
| `flush_delay_buckets` | Baldes completos a esperar antes de tentar uma descarga. | `1` |
| `force_flush_buckets` | Idade máxima antes de emitir um balde suprimido. | `6` |
| `max_completed_buckets` | Backlog de buckets retidos (impedir memória sem limite). | `120` |
| `max_share_lag_buckets` | Janela de retenção para ações de colecionador antes de suprimir. | `12` |
| `expected_shares` | Ações de colecionador prévias exigidas antes de combinar. | `2` |
| `event_buffer_capacity` | Backlog de eventos NDJSON para o administrador do stream. | `4096` |

Definir `force_flush_buckets` menor que `flush_delay_buckets`, zerar os limites, ou desativar o guarda de retenção agora falha na validação para evitar implantações que vazariam telemetria por relé.

O limite `event_buffer_capacity` também limita `/admin/privacy/events`, garantindo que os raspadores não possam ficar atrasados ​​indefinidamente.

## Ações de colecionador Prio

SNNet-8a implanta coletores duplos que emitem buckets Prio com compartilhamento secreto. O orquestrador agora analisa o stream NDJSON `/privacy/events` para entradas `SoranetPrivacyEventV1` e ações `SoranetPrivacyPrioShareV1`, encaminhando-as para `SoranetSecureAggregator::ingest_prio_share`. Buckets emitem quando chegam `PrivacyBucketConfig::expected_shares` contribuicoes, espelhando o comportamento do revezamento. As ações são validadas para alinhamento de bucket e forma do histograma antes de serem combinadas em `SoranetPrivacyBucketMetricsV1`. Se a combinação combinada de handshakes ficar abaixo de `min_contributors`, o bucket e exportado como `suppressed`, espelhando o comportamento do agregador no relé. Janelas suprimidas agora emitem um rótulo `suppression_reason` para que os operadores possam distinguir entre `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed`, e `forced_flush_window_elapsed` ao diagnosticar lacunas de telemetria. O motivo `collector_window_elapsed` também é disparatado quando as ações da Prio ficam além de `max_share_lag_buckets`, tornando os colecionadores presos visiveis sem deixar acumuladores antigos na memória.

## Endpoints de ingestão do Torii

Torii agora expõe dois endpoints HTTP com gating de telemetria para que relés e coletores possam encaminhar observações sem embutir um transporte sob medida:- `POST /v1/soranet/privacy/event` aceita uma carga útil `RecordSoranetPrivacyEventDto`. O corpo envolve um `SoranetPrivacyEventV1` mais uma etiqueta `source` opcional. Torii valida a requisição contra o perfil de telemetria ativo, registra o evento, e responde com HTTP `202 Accepted` junto com um envelope Norito JSON contendo a janela calculada (`bucket_start_unix`, `bucket_duration_secs`) e o modo do relé.
- `POST /v1/soranet/privacy/share` aceita uma carga útil `RecordSoranetPrivacyShareDto`. O corpo carrega um `SoranetPrivacyPrioShareV1` e uma dica `forwarded_by` opcional para que os operadores possam auditar fluxos de coletores. Envios bem-sucedidos retornam HTTP `202 Accepted` com um envelope Norito JSON resumindo o coletor, uma janela de bucket e uma dica de supressão; falhas de validação mapeiam para uma resposta de telemetria `Conversion` para preservar tratamento determinístico de erros entre coletores. O loop de eventos do orquestrador agora emite essas ações ao fazer polling dos relés, mantendo o acumulador Prio do Torii sincronizado com os buckets no relé.

Ambos os endpoints respeitam o perfil de telemetria: emitem `503 Service Unavailable` quando as métricas estão desativadas. Os clientes podem enviar corpos Norito binário (`application/x.norito`) ou Norito JSON (`application/x.norito+json`); o servidor negocia automaticamente o formato via extratores padrão do Torii.

## Métricas Prometheus

Cada balde exportado carrega etiquetas `mode` (`entry`, `middle`, `exit`) e `bucket_start`. As seguintes famílias de métricas são emitidas:

| Métrica | Descrição |
|--------|------------|
| `soranet_privacy_circuit_events_total{kind}` | Taxonomia de handshake com `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | Contadores de acelerador com `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Durações de cooldown agregadas por handshakes estrangulados. |
| `soranet_privacy_verified_bytes_total` | Largura de banda verificada de provas de medicação cega. |
| `soranet_privacy_active_circuits_{avg,max}` | Mídia e pico de circuitos ativos por balde. |
| `soranet_privacy_rtt_millis{percentile}` | Estimativas de percentil RTT (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Contadores de Governance Action Report com hash por resumo de categoria. |
| `soranet_privacy_bucket_suppressed` | Baldes retidos porque os limites de contribuições não foram atingidos. |
| `soranet_privacy_pending_collectors{mode}` | Acumuladores de ações de colecionador pendentes de combinação, agrupados por modo de retransmissão. |
| `soranet_privacy_suppression_total{reason}` | Contadores de baldes suprimidos com `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` para que os dashboards atribuam lacunas de privacidade. |
| `soranet_privacy_snapshot_suppression_ratio` | Razão suprimida/drenada do último dreno (0-1), util para orçamentos de alerta. |
| `soranet_privacy_last_poll_unixtime` | Timestamp UNIX da última pesquisa bem-sucedida (alimenta o alerta coletor-idle). |
| `soranet_privacy_collector_enabled` | Medidor que vira `0` quando o coletor de privacidade está desativado ou falha ao iniciar (alimentação ou alerta coletor-desativado). |
| `soranet_privacy_poll_errors_total{provider}` | Falhas de polling agrupadas por alias de relay (incrementa em erros de decodificação, falhas HTTP, ou códigos de status inesperados). |Baldes sem observações permanecem silenciosos, mantendo os painéis limpos sem fabricar janelas zeradas.

## Orientação operacional

1. **Dashboards** - rastreie as métricas acima agrupadas por `mode` e `window_start`. Destaque janelas ausentes para revelar problemas de coletor ou relé. Use `soranet_privacy_suppression_total{reason}` para distinguir falta de contribuidores de supressões orientadas por coletores ao triar lacunas. O ativo Grafana agora envia um painel dedicado **"Suppression Reasons (5m)"** alimentado por esses contadores mais um stat **"Suppressed Bucket %"** que calcula `sum(soranet_privacy_bucket_suppressed) / count(...)` por seleção para que os operadores vejam violações de orçamento rapidamente. A série **"Collector Share Backlog"** (`soranet_privacy_pending_collectors`) e o stat **"Snapshot Suppression Ratio"** destacam coletores presos e desvio de orçamento durante execuções automáticas.
2. **Alerta** - conduz alarmes a partir de contadores seguros de privacidade: picos de rejeição de PoW, frequência de resfriamento, desvio de RTT e rejeições de capacidade. Como os contadores são monótonos dentro de cada balde, regras simples baseadas em taxas funcionam bem.
3. **Resposta ao incidente** - confie primeiro nos dados agregados. Quando for necessário depurar mais profundamente, solicite que os relés reproduzam snapshots de buckets ou inspecionem provas de medicamento às cegas em vez de coletar logs de tráfego bruto.
4. **Retenção** - faca raspada com frequência suficiente para evitar ultrapassar `max_completed_buckets`. Os exportadores devem tratar a saida Prometheus como fonte canônica e descartar baldes locais depois de encaminhados.

## Análise de supressão e execução automatizada

A aceitação do SNNet-8 depende de demonstrar que os coletores automáticos permanecem saudáveis e que a supressão fica dentro dos limites da política (<=10% dos baldes por relé em qualquer janela de 30 minutos). O ferramental necessário para cumprir esse portão agora vem com o repositório; Os operadores devem integrar isso aos seus rituais semanais. As novas dores de supressão do Grafana refletem os trechos PromQL abaixo, dando às equipes de plantação visibilidade ao vivo antes que precisem recorrer a consultas manuais.

### Receitas PromQL para revisão de supressão

Os operadores devem manter os seguintes auxiliares PromQL a mao; ambos são referenciados no dashboard Grafana compartilhado (`dashboards/grafana/soranet_privacy_metrics.json`) e nas regras do Alertmanager:

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

Use a saida do ratio para confirmar que o stat **"Suppressed Bucket %"** permanece abaixo do orçamento de política; conecte o detector de picos ao Alertmanager para feedback rápido quando a contagem de contribuidores cair inesperadamente.

### CLI de relato de bucket offline

O workspace expoe `cargo xtask soranet-privacy-report` para capturas NDJSON pontuais. Aponte para um ou mais exports admin de relay:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```O helper passa a captura pelo `SoranetSecureAggregator`, imprime um resumo de supressão no stdout e, opcionalmente, grava um relato JSON estruturado via `--json-out <path|->`. Ele honra os mesmos botões do coletor ao vivo (`--bucket-secs`, `--min-contributors`, `--expected-shares`, etc.), permitindo que os operadores reproduzam capturas históricas sob limiares diferentes ao triar um incidente. Anexe o JSON junto com capturas do Grafana para que o portão de análise de supressão SNNet-8 permaneça auditável.

### Checklist da primeira execução automatizada

A governança ainda exige provar que a primeira execução automatizada atendeu ao orçamento de supressão. O ajudante agora aceita `--max-suppression-ratio <0-1>` para que CI ou operadores falhem rapidamente quando buckets suprimidos excedem a janela permitida (padrão 10%) ou quando ainda não há buckets. Fluxo recomendado:

1. Exporte NDJSON dos endpoints admin do relay mais o stream `/v1/soranet/privacy/event|share` do orquestrador para `artifacts/sorafs_privacy/<relay>.ndjson`.
2. Rodou o ajudante com o orçamento de política:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   O comando imprime a razão observada e sai com código não zero quando o orçamento é excedido **ou** quando ainda não há baldes prontos, sinalizando que a telemetria ainda não foi produzida para a execução. As métricas ao vivo devem mostrar `soranet_privacy_pending_collectors` drenando para zero e `soranet_privacy_snapshot_suppression_ratio` ficando abaixo do mesmo orçamento enquanto a execução ocorre.
3. Arquive o JSON dito e o log da CLI com o pacote de evidências SNNet-8 antes de trocar o padrão do transporte para que os revisores possam reproduzir os artefatos exatos.

## Próximos passos (SNNet-8a)

- Integrar os coletores Prio duplos, conectando a ingestão de compartilhamentos ao runtime para que relés e coletores emitam payloads `SoranetPrivacyBucketMetricsV1` consistentes. *(Concluído - veja `ingest_privacy_payload` em `crates/sorafs_orchestrator/src/lib.rs` e os testes associados.)*
- Publicar o dashboard Prometheus compartilhado e regras de alerta lacunas de supressão, saúde dos colecionadores e quedas de anonimato. *(Concluído - veja `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` e fixtures de validação.)*
- Produzir os desenhos de calibração de privacidade específicos descritos em `privacy_metrics_dp.md`, incluindo cadernos reproduzíveis e resumos de governança. *(Concluido - notebook e artistas gerados por `scripts/telemetry/run_privacy_dp.py`; wrapper CI `scripts/telemetry/run_privacy_dp_notebook.sh` executa o notebook via o workflow `.github/workflows/release-pipeline.yml`; resumo de governança arquivado em `docs/source/status/soranet_privacy_dp_digest.md`.)*

A release atual entrega a base do SNNet-8: telemetria determinística e segura para privacidade que se encaixa diretamente nos raspadores e dashboards Prometheus existentes. Os artefatos de calibração de privacidade diferenciais estão no lugar, o fluxo de trabalho do pipeline de liberação mantém as saídas do notebook atualizados, e o trabalho restante foca no monitoramento da primeira execução automatizada e na extensão das análises de alerta de supressão.