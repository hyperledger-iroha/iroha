---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pipeline de métricas de privacidade
título: Pipeline de métricas de confidencialidade SoraNet (SNNet-8)
sidebar_label: Pipeline de métricas de confidencialidade
descrição: Coleta de telefonia preserva a confidencialidade dos relés e orquestradores SoraNet.
---

:::nota Fonte canônica
Reflete `docs/source/soranet/privacy_metrics_pipeline.md`. Garanta que as duas cópias sejam sincronizadas até que o antigo conjunto de documentação seja retirado.
:::

# Pipeline de métricas de confidencialidade SoraNet

SNNet-8 introduz uma superfície de telemetria sensível à confidencialidade para o tempo de execução do relé. O relé concorda com os eventos de handshake e circuito em baldes de um minuto e não exporta que os computadores Prometheus sejam grosseiros, garantindo que os circuitos individuais não corrigíveis sejam dotados de uma visibilidade explorável para os operadores.

## Aperçu de l'agrégateur

- Implementação do tempo de execução em `tools/soranet-relay/src/privacy.rs` sob `PrivacyAggregator`.
- Os baldes são indexados por minuto de relógio (`bucket_secs`, por padrão 60 segundos) e armazenados em um ano nascido (`max_completed_buckets`, por padrão 120). As ações dos colecionadores conservam seu próprio backlog nascido (`max_share_lag_buckets`, por padrão 12) depois que as janelas do Prio anteriores eram vídeos em baldes, suprimindo a maior parte do fluxo de memória ou mascarando os colecionadores bloqueados.
- `RelayConfig::privacy` é mapeado diretamente para `PrivacyConfig`, expondo os regulamentos (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`). O tempo de execução de produção mantém os valores padrão enquanto o SNNet-8a introduz seus servidores de agregação segura.
- Os módulos de tempo de execução registram eventos por meio dos tipos de ajuda: `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`, `record_verified_bytes` e `record_gar_category`.

## Administrador do endpoint do relé

Os operadores podem interrogar o ouvinte administrador do relé para observações brutas via `GET /privacy/events`. O envio do endpoint do JSON é delimitado por novas linhas (`application/x-ndjson`) contendo cargas úteis `SoranetPrivacyEventV1` refletido a partir do `PrivacyEventBuffer` interno. O buffer conserva os eventos mais recentes até as entradas `privacy.event_buffer_capacity` (padrão 4096) e é visto na palestra, pois os raspadores devem ser suficientes para evitar problemas. Os eventos receberam os mesmos sinais de handshake, acelerador, largura de banda verificada, circuito ativo e GAR que alimentam os computadores Prometheus, permitindo que os coletores obtenham o arquivamento de migalhas de pão seguras para a confidencialidade ou para alimentar fluxos de trabalho de agregação segura.

## Configuração do relé

Os operadores ajustam a cadência de transmissão de confidencialidade no arquivo de configuração do relé por meio da seção `privacy`:

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

Os valores padrão dos campos correspondentes à especificação SNNet-8 e são válidos no custo:| Campeão | Descrição | Padrão |
|-------|------------|--------|
| `bucket_secs` | Largeur de chaque fenêtre d'agrégation (segundos). | `60` |
| `min_handshakes` | Nomeie o mínimo de contribuintes antes que um balde possa aumentar os acionistas. | `12` |
| `flush_delay_buckets` | Nome de baldes completos para atender antes de tentar uma descarga. | `1` |
| `force_flush_buckets` | Âge maximal avant d'émettre un bucket supprimé. | `6` |
| `max_completed_buckets` | Backlog de baldes conservados (evite uma memória não nascida). | `120` |
| `max_share_lag_buckets` | Fenêtre de retenção de ações cobradas antes da supressão. | `12` |
| `expected_shares` | Ações O preço de cobrança requer uma combinação antecipada. | `2` |
| `event_buffer_capacity` | Backlog de eventos NDJSON para o fluxo administrativo. | `4096` |

Defina `force_flush_buckets` mais como `flush_delay_buckets`, definindo seus valores para zero ou desativando o garde de retenção, ouve-se a validação para evitar implementações que foram direcionadas para o telefone por retransmissão.

O limite `event_buffer_capacity` suportado também por `/admin/privacy/events` garante que os raspadores não possam causar um atraso indefinido.

## Ações de colecionadores Prio

SNNet-8a implanta coletores duplos que enviam baldes antes de compartilhar segredos. O orquestrador analisa o fluxo NDJSON `/privacy/events` para as entradas `SoranetPrivacyEventV1` e as ações `SoranetPrivacyPrioShareV1`, transmettant para `SoranetSecureAggregator::ingest_prio_share`. Os baldes contêm uma vez que as contribuições `PrivacyBucketConfig::expected_shares` chegam, refletindo o comportamento do relé. As ações são válidas para o alinhamento dos baldes e a forma do histograma antes de serem combinadas em `SoranetPrivacyBucketMetricsV1`. Se o nome combinado de apertos de mão for `min_contributors`, o balde será exportado como `suppressed`, refletindo o comportamento do relé de controle do distribuidor. As janelas suprimidas foram desmontadas com um rótulo `suppression_reason` para que os operadores possam distinguir `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed` e `forced_flush_window_elapsed` durante o diagnóstico de problemas de telemétrie. A razão `collector_window_elapsed` também diminui quando as ações são treinadas a partir de `max_share_lag_buckets`, deixando os coletores bloqueados visíveis sem deixar acumuladores acumulados na memória.

## Endpoints de ingestão Torii

Torii expõe dois pontos de extremidade HTTP protegidos pela telemetria para que os relés e coletores possam transmitir observações sem planejar um transporte sob medida:- `POST /v2/soranet/privacy/event` aceita uma carga útil `RecordSoranetPrivacyEventDto`. O corpo envolve um `SoranetPrivacyEventV1` mais uma etiqueta `source` opcional. Torii valida a solicitação com o perfil de telemetria ativo, registra o evento e responde com HTTP `202 Accepted` acompanhado de um envelope Norito JSON contendo a janela calculada (`bucket_start_unix`, `bucket_duration_secs`) e o modo do relé.
- `POST /v2/soranet/privacy/share` aceita uma carga útil `RecordSoranetPrivacyShareDto`. O corpo transporta um `SoranetPrivacyPrioShareV1` e um índice `forwarded_by` opcional para que os operadores possam auditar o fluxo de coletores. As mensagens enviadas HTTP `202 Accepted` com um envelope Norito JSON retornam o coletor, a janela do balde e a indicação de supressão; as verificações de validação correspondentes a uma resposta de telefone `Conversion` para preservar um traço de erro determinado entre os colecionadores. A parte de eventos do orquestrador foi desordenada quando os relés foram interceptados, mantendo o acumulador Prio de Torii sincronizado com os baldes no relé.

Os dois pontos de extremidade respeitam o perfil de telefonia: o identificador `503 Service Unavailable` quando as métricas são desativadas. Os clientes podem enviar o corpo binário Norito (`application/x.norito`) ou Norito JSON (`application/x.norito+json`) ; o servidor negocia automaticamente o formato por meio dos extratores padrão Torii.

## Métricas Prometheus

Cada balde exportado porta as etiquetas `mode` (`entry`, `middle`, `exit`) e `bucket_start`. As famílias de métricas seguintes são emisidas:| Métrica | Descrição |
|--------|------------|
| `soranet_privacy_circuit_events_total{kind}` | Taxonomia de handshakes com `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | Competeurs de acelerador com `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Tempos de resfriamento aprovados fornecidos por apertos de mão acelerados. |
| `soranet_privacy_verified_bytes_total` | Bande passante vérifiée issue de preuves de mesure aveugles. |
| `soranet_privacy_active_circuits_{avg,max}` | Moyenne e uma foto de circuitos ativos por balde. |
| `soranet_privacy_rtt_millis{percentile}` | Estimativas de percentis RTT (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Relatórios de Ação de Governança foram indexados pelo resumo da categoria. |
| `soranet_privacy_bucket_suppressed` | Buckets reténus parce que seuil de contribuintes não foi considerado. |
| `soranet_privacy_pending_collectors{mode}` | Acumuladores de ações de colecionadores em tentativa de combinação, agrupados por modo de retransmissão. |
| `soranet_privacy_suppression_total{reason}` | Os controles de baldes foram excluídos com `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` para que os painéis possam atribuir a confidencialidade. |
| `soranet_privacy_snapshot_suppression_ratio` | Proporção suprimida/vidé do dreno superior (0-1), útil para orçamentos alertas. |
| `soranet_privacy_last_poll_unixtime` | Horodatage UNIX du dernier poll réussi (alimente o alerta de coletor-idle). |
| `soranet_privacy_collector_enabled` | Medidor que passa para `0` quando o coletor de confidencialidade é desativado ou não é iniciado (alimente o alerta de coletor desativado). |
| `soranet_privacy_poll_errors_total{provider}` | Verificações de pesquisa agrupadas por alias de retransmissão (aumentam em caso de erros de decodificação, verificações HTTP ou códigos de status inativos). |

Os baldes sem observações permanecem silenciosos, guardando os painéis próprios sem o fabricante de janelas respondendo a zeros.

## Orientação operacional1. **Dashboards** - rastreie as métricas dos dois grupos de `mode` e `window_start`. Mostre as janelas quebradas para reparar os problemas do coletor ou do relé. Use `soranet_privacy_suppression_total{reason}` para distinguir as insuficiências dos contribuintes das supressões pilotadas pelos coletores durante a triagem de trous. O ativo Grafana inclui um painel dédié **"Motivos de supressão (5m)"** alimentar por esses compradores, além de uma estatística **"Balde suprimido%"** que calcula `sum(soranet_privacy_bucket_suppressed) / count(...)` por seleção para que os operadores representem os gastos de orçamento em um golpe de estado. A série **"Collector Share Backlog"** (`soranet_privacy_pending_collectors`) e a estatística **"Snapshot Suppression Ratio"** evidenciaram os colecionadores bloqueados e o resultado do orçamento pendente das execuções automatizadas.
2. **Alerta** - pilote alertas de computadores seguros para confidencialidade: fotos de rejeição de PoW, frequência de resfriamento, derivação RTT e rejeição de capacidade. Como os computadores são monótonos no interior de cada balde, as regras de taux simples funcionam bem.
3. **Resposta a incidentes** - toque nos dados aprovados. Quando uma depuração mais profunda é necessária, exija relés de recuperação de instantâneos de baldes ou inspecione testes de medição aveugles em vez de coletar diários de tráfego bruto.
4. **Retenção** - raspador suficiente para evitar a passagem `max_completed_buckets`. Os exportadores devem trair a saída Prometheus como fonte canônica e suprimir os baldes localizados uma vez que foram transmitidos.

## Análise de supressão e execução automatizada

A aceitação do SNNet-8 depende da demonstração de que os coletores são automatizados e que a supressão permanece nos limites da política (≤10% dos baldes por retransmissão em toda a janela de 30 minutos). As ferramentas necessárias para satisfazer esta exigência são mantidas livres com o depósito; os operadores devem aderir aos seus rituais hebdomadários. Os novos painéis de supressão Grafana refletem os extratos PromQL ci-dessous, dando às equipes uma visibilidade direta antes de recorrer a solicitações manuais.

### Receitas PromQL para a revista de supressão

Os operadores devem usar os auxiliares PromQL seguintes à porta principal ; Os dois são referenciados no painel Grafana compartilhado (`dashboards/grafana/soranet_privacy_metrics.json`) e nas regras do Alertmanager:

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

Use a proporção para confirmar que o status **"Suppressed Bucket %"** permanece no orçamento da política; ramifica o detector de fotos no Alertmanager para um retorno rápido quando o nome dos colaboradores está mais desatendido.

### CLI de relatório de balde fora da linha

O espaço de trabalho expõe `cargo xtask soranet-privacy-report` para capturas pontuais NDJSON. Aponte para um ou mais exports admin de relay :

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```A ferramenta fez a captura por `SoranetSecureAggregator`, imprime um resumo de supressão em stdout e, opcionalmente, escreve um relacionamento JSON estruturado via `--json-out <path|->`. Ele respeita as mesmas regras do colecionador ao vivo (`--bucket-secs`, `--min-contributors`, `--expected-shares`, etc.), permitindo que os operadores regozijem capturas históricas em seus diferentes locais durante a triagem de um incidente. Execute o JSON com capturas Grafana para que a porta de análise de supressão SNNet-8 seja auditável.

### Checklist de primeira execução automatizada

O governo exige sempre que a primeira execução seja automatizada e respeite o orçamento de supressão. A ferramenta aceita `--max-suppression-ratio <0-1>` quando o CI ou os operadores podem tocar rapidamente quando os baldes são apagados da janela autorizada (10% por padrão) ou quando algum balde não está mais presente. Fluxo recomendado:

1. Exportador do NDJSON a partir dos endpoints admin do relé mais o fluxo `/v2/soranet/privacy/event|share` do orquestrador versão `artifacts/sorafs_privacy/<relay>.ndjson`.
2. Execute a ferramenta com o orçamento político:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   O comando imprime a proporção observada e termina com um código não nulo quando o orçamento é ultrapassado **ou** quando algum balde não está pronto, sinalizando que a telemetria não foi produzida novamente para a execução. As métricas ao vivo devem ser montadas `soranet_privacy_pending_collectors` se vidant vers zero et `soranet_privacy_snapshot_suppression_ratio` permanecem sob o mesmo orçamento pendente da execução.
3. Arquive a classificação JSON e o diário CLI com o dossiê de teste SNNet-8 antes de bascular o transporte por padrão para que os revisores possam recuperar os artefatos exatos.

## Prochaines étapes (SNNet-8a)

- Integre os coletores Prio doubles, conectando a ingestão de compartilhamentos em tempo de execução para que os relés e coletores emitam cargas úteis `SoranetPrivacyBucketMetricsV1` coerentes. *(Fait — veja `ingest_privacy_payload` em `crates/sorafs_orchestrator/src/lib.rs` e os testes associados.)*
- Publique o painel Prometheus compartilhado e as regras de alerta que cobrem os problemas de supressão, a saúde dos coletores e as bases de anonimato. *(Fait — veja `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` e os dispositivos de validação.)*
- Produza os artefatos de calibração de confidencialidade diferentes descritos em `privacy_metrics_dp.md`, incluindo notebooks reprodutíveis e resumos de governança. *(Fait — notebook + artefatos gerados por `scripts/telemetry/run_privacy_dp.py`; wrapper CI `scripts/telemetry/run_privacy_dp_notebook.sh` executa o notebook por meio do fluxo de trabalho `.github/workflows/release-pipeline.yml`; resumo de governo depositado em `docs/source/status/soranet_privacy_dp_digest.md`.)*A versão atual livre da fundação SNNet-8: um telefone determinado e seguro para a confidencialidade que é integral diretamente nos raspadores Prometheus e nos painéis. Os artefatos de calibração de confidencialidade diferentes estão no local, o fluxo de trabalho do pipeline de liberação mantém as saídas do notebook do dia e o trabalho restante se concentra na vigilância da primeira execução automatizada, além da extensão das análises de alerta de supressão.