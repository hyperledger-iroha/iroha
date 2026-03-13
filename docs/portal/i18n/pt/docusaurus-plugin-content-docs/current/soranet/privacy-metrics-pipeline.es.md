---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pipeline de métricas de privacidade
título: Pipeline de métricas de privacidade da SoraNet (SNNet-8)
sidebar_label: Pipeline de métricas de privacidade
descrição: Telemetria com preservação de privacidade para relés e orquestradores de SoraNet.
---

:::nota Fonte canônica
Reflexo `docs/source/soranet/privacy_metrics_pipeline.md`. Mantenha ambas as cópias sincronizadas até que os documentos herdados sejam retirados.
:::

# Pipeline de métricas de privacidade da SoraNet

SNNet-8 apresenta uma superfície de telemetria consciente da privacidade para
o tempo de execução do relé. Agora o relé agrega eventos de handshake e circuito em
baldes de um minuto e exportar solo contadores Prometheus gruesos, mantendo
Os circuitos individuais desvinculados enquanto oferecem visibilidade acionável
para os operadores.

## Resumo do agregador

- A implementação do tempo de execução vive em `tools/soranet-relay/src/privacy.rs` como
  `PrivacyAggregator`.
- Os baldes são indexados por minuto de relógio (`bucket_secs`, padrão 60 segundos) e
  é armazenado em um anel acotado (`max_completed_buckets`, padrão 120). As ações
  de coletores mantêm seu próprio backlog acotado (`max_share_lag_buckets`, padrão 12)
  para que las ventanas Prio stale se vacien como baldes suprimidos em vez de
  filtrar memória ou enmascarar coletores atascados.
- `RelayConfig::privacy` mapeia direto para `PrivacyConfig`, exponiendo botões de ajuste
  (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`,
  `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`). O tempo de execução
  de produção mantém os padrões enquanto SNNet-8a introduz limites de
  agregação segura.
- Os módulos de tempo de execução registram eventos com ajudantes tipados:
  `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`,
  `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`,
  `record_verified_bytes`, e `record_gar_category`.

## Administrador do endpoint do relé

Os operadores podem consultar o ouvinte admin do relé para observações
crudas via `GET /privacy/events`. O endpoint retorna JSON delimitado por
novas linhas (`application/x-ndjson`) com cargas úteis `SoranetPrivacyEventV1`
refletidos desde o `PrivacyEventBuffer` interno. O buffer retém os eventos
mas novos até `privacy.event_buffer_capacity` entradas (padrão 4096) e se
Vazio na palestra, porque os raspadores devem ser sondados o suficiente para
não dejar huecos. Los eventos cubren las mismas senales de aperto de mão, acelerador,
largura de banda verificada, circuito ativo e GAR que alimentam os contadores Prometheus,
permitindo que coletores downstream arquivem migalhas de pão seguros para a privacidade
o fluxo de trabalho alimentar de agregação segura.

## Configuração do relé

Os operadores ajustam a cadência de telemetria de privacidade no arquivo de
configuração do relé através da seção `privacy`:

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

Os padrões de campo coincidem com a especificação SNNet-8 e são válidos para
carregar:| Campo | Descrição | Padrão |
|-------|-------------|---------|
| `bucket_secs` | Ancho de cada janela de agregação (segundos). | `60` |
| `min_handshakes` | Mínimo de contribuintes antes de emitir um balde. | `12` |
| `flush_delay_buckets` | Buckets completados a esperar antes da descarga. | `1` |
| `force_flush_buckets` | Edad maxima antes de emitir um balde suprimido. | `6` |
| `max_completed_buckets` | Backlog de buckets retidos (evita memória ilimitada). | `120` |
| `max_share_lag_buckets` | Ventana de retenção para ações de colecionadores antes de suprimir. | `12` |
| `expected_shares` | Ações Pré-requisitos antes da combinação. | `2` |
| `event_buffer_capacity` | Backlog NDJSON de eventos para o stream de administração. | `4096` |

Configure `force_flush_buckets` por baixo de `flush_delay_buckets`, coloque
os limites em zero ou desativando a guarda de retenção agora falham
validação para evitar despliegues que filtram telemetria por relé.

O limite `event_buffer_capacity` também inclui `/admin/privacy/events`,
assegurando que os raspadores não se atrasem indefinidamente.

## Ações do colecionador Prio

SNNet-8a oferece coletores duplos que emitem buckets Prio com compartilhamento secreto.
Agora o orquestrador analisa o stream NDJSON `/privacy/events` tanto para
entradas `SoranetPrivacyEventV1` como para ações `SoranetPrivacyPrioShareV1`,
reenviando-os para `SoranetSecureAggregator::ingest_prio_share`. Los baldes se
emiten cuando llegan `PrivacyBucketConfig::expected_shares` contribuições,
refletindo o comportamento do agregador no relé. As ações são válidas
por alinhamento de balde e forma de histograma antes de combiná-lo
`SoranetPrivacyBucketMetricsV1`. Se o conteúdo combinado de apertos de mão cae por
abaixo de `min_contributors`, o balde é exportado como `suppressed`, refletindo
o comportamento do agregador no relé. Las ventanas suprimidas agora
emite uma etiqueta `suppression_reason` para que os operadores distingam entre
`insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed`
e `forced_flush_window_elapsed` para diagnosticar objetos de telemetria. A razão
`collector_window_elapsed` também dispara quando as ações Prio se quedan mas
alla de `max_share_lag_buckets`, fazendo visíveis os coletores atascados sem
manter acumuladores obsoletos na memória.

## Endpoints de ingestão Torii

Torii agora exponha os endpoints HTTP com telemetria gateada para relés e
colecionadores reenvien observações sem incrustar um transporte sob medida:- `POST /v2/soranet/privacy/event` aceita uma carga útil
  `RecordSoranetPrivacyEventDto`. O corpo envolve um `SoranetPrivacyEventV1`
  mas uma etiqueta opcional `source`. Torii valida a solicitação contra o
  perfil de telemetria ativo, registre o evento e responda com HTTP
  `202 Accepted` junto com um envelope Norito JSON que contém a janela
  cálculo do balde (`bucket_start_unix`, `bucket_duration_secs`) e el
  modo do relé.
- `POST /v2/soranet/privacy/share` aceita uma carga útil `RecordSoranetPrivacyShareDto`.
  O corpo contém um `SoranetPrivacyPrioShareV1` e uma dica opcional `forwarded_by`
  para que os operadores auditem fluxos de coletores. As entregas exitosas
  devolve HTTP `202 Accepted` com um envelope Norito JSON que retoma o
  coletor, a janela do balde e a dica de supressão; as falhas de
  validação mapeada para uma resposta de telemetria `Conversion` para preservação
  o manejo de erros é determinado pelos coletores. O loop de eventos do orquestrador
  agora emite estas ações enquanto sonde relés, mantendo o acumulador
  Prévio de Torii sincronizado com os buckets on-relay.

Ambos os endpoints respeitam o perfil de telemetria: emitem `503 Service
Indisponível` quando as métricas estão desativadas. Os clientes podem enviar
cuerpos Norito binário (`application/x.norito`) ou Norito JSON (`application/x.norito+json`);
o servidor negocia o formato automaticamente através dos extratores padrão de
Torii.

## Métricas Prometheus

Cada balde exportado leva etiquetas `mode` (`entry`, `middle`, `exit`) e
`bucket_start`. Veja as seguintes famílias de métricas:

| Métrica | Descrição |
|--------|------------|
| `soranet_privacy_circuit_events_total{kind}` | Taxonomia de handshake com `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | Contadores de acelerador com `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Durações agregadas de cooldown aportadas por handshakes estrangulados. |
| `soranet_privacy_verified_bytes_total` | Ancho de banda selecionada de provas de medicina cegada. |
| `soranet_privacy_active_circuits_{avg,max}` | Mídia e pico de circuitos ativos por balde. |
| `soranet_privacy_rtt_millis{percentile}` | Estimativas de percentual de RTT (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Contadores GAR hasheados por resumo de categoria. |
| `soranet_privacy_bucket_suppressed` | Baldes retidos porque não se cumpre o umbral de contribuintes. |
| `soranet_privacy_pending_collectors{mode}` | Acumuladores de ações pendentes de combinação, agrupados por modo de retransmissão. |
| `soranet_privacy_suppression_total{reason}` | Contadores de baldes suprimidos com `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` para cobrir lacunas de privacidade. |
| `soranet_privacy_snapshot_suppression_ratio` | Proporção suprimido/descargado do último dreno (0-1), útil para pressupostos de alerta. |
| `soranet_privacy_last_poll_unixtime` | Timestamp UNIX da última pesquisa exitosa (alimenta o alerta do coletor-idle). |
| `soranet_privacy_collector_enabled` | Gauge que cae a `0` quando o coletor de privacidade está desativado ou falha ao iniciar (alimenta o alerta coletor-desativado). |
| `soranet_privacy_poll_errors_total{provider}` | Falhas de votação agrupadas por alias de retransmissão (incrementos por erros de decodificação, falhas de HTTP ou códigos inesperados). |Los buckets sin observaciones permanecem silenciosos, mantendo painéis
limpios sin fabricar ventanas con ceros.

## Guia Operativo

1. **Dashboards** - gráfico das métricas anteriores agrupadas por `mode` e
   `window_start`. Destaca ventanas faltantes para revelar problemas em colecionadores
   ó relés. Usa `soranet_privacy_suppression_total{reason}` para distinguir
   carencias de contribuintes de supressão por coletor na triagem. O ativo de
   Grafana agora inclui um painel dedicado **"Suppression Reasons (5m)"**
   alimentado por esos contadores mas un stat **"Suppressed Bucket %"** que
   calcular `sum(soranet_privacy_bucket_suppressed) / count(...)` por seleção
   para que os operadores tenham brechas de presunção de vistazo. A série
   **Backlog de compartilhamento do coletor** (`soranet_privacy_pending_collectors`) e o estado
   **Taxa de supressão de instantâneos** coletores resaltam atascados e desviam do orçamento
   durante execuções automatizadas.
2. **Alerta** - dispara alarmes de contadores com privacidade segura: picos de rechazo
   PoW, frequência de resfriamento, desvio de RTT e rejeições de capacidade. Como eles
   contadores são monotônicos dentro de cada balde, regras baseadas em tasa
   funciona bem.
3. **Resposta a incidentes** - Confie primeiro nos dados agregados. Quando você precisar
   depuração profunda, solicita que os relés reproduzam instantâneos de baldes o
   inspecionar provas de medicina cegada em vez de coletar registros de tráfego
   crudos.
4. **Retenção** - raspa com frequência suficiente para não exceder
   `max_completed_buckets`. Os exportadores devem tratar a saída Prometheus como
   a fonte canônica e descartar buckets locais uma vez reenviados.

## Análise de supressão e ejeções automatizadas

A aceitação do SNNet-8 depende da demonstração de que os coletores automatizados são
mantenha a saúde e que a supressão permaneça dentro dos limites da política
(<=10% de baldes por revezamento em qualquer ventana de 30 minutos). El ferramental
necessário que você seja incluído no repositório; os operadores devem integrá-los em seu sus
rituais semanais. Os novos painéis de supressão em Grafana refletem os
snippets PromQL abaixo, dando visibilidade ao vivo aos equipamentos de plantão antes de
recorrer a consultas manuais.

### Recetas PromQL para revisar supressão

Os operadores devem ter à mão os seguintes helpers PromQL; ambos se
referenciado no painel compartilhado (`dashboards/grafana/soranet_privacy_metrics.json`)
e nas regras do Alertmanager:

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

Use a proporção para confirmar que o status **"Suppressed Bucket %"** é mantido
por baixo do pressuposto da política; conecte o detector de picos a
Alertmanager para feedback rápido quando o conteúdo de colaboradores abaixo de
forma inesperada.

### CLI de relatório off-line de buckets

A área de trabalho expõe `cargo xtask soranet-privacy-report` para capturas NDJSON
pontuais. Apontar para um ou mais exportar admin del relé:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```El helper processa a captura com `SoranetSecureAggregator`, imprime um resumo de
supressão em stdout e opcionalmente escrever um relatório JSON estruturado via
`--json-out <path|->`. Respeita os mesmos botões que o colecionador ao vivo
(`--bucket-secs`, `--min-contributors`, `--expected-shares`, etc.), permitindo
reproduzir capturas históricas em diferentes limites para investigar um problema.
Adicione o JSON junto com as capturas de tela de Grafana para que o portão de análise
SNNet-8 segue sendo auditável.

### Checklist da primeira execução automatizada

Governança exige que a primeira execução automatizada cumpra o
presupuesto de supressão. El helper agora aceita `--max-suppression-ratio <0-1>`
para que CI u operadores caiam rapidamente quando os baldes suprimidos excedem la
ventilação permitida (padrão 10%) ou quando não houver baldes de feno presentes. Flujo
Recomendado:

1. Exporte NDJSON do endpoint admin do relé e do stream
   `/v2/soranet/privacy/event|share` do orquestrador para trás
   `artifacts/sorafs_privacy/<relay>.ndjson`.
2. Execute o ajudante com o pressuposto da política:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   El comando imprime el ratio presente y sale con código distinto de cero
   quando se excede o presupuesto **o** quando não há baldes de feno listados, rebaixados
   que a telemetria não foi produzida para a execução. As métricas em
   vivo deben mostrar `soranet_privacy_pending_collectors` drenando para zero e
   `soranet_privacy_snapshot_suppression_ratio` caindo abaixo do mesmo
   presupuesto enquanto se executa a corrida.
3. Arquive o JSON de saída e o log da CLI com o pacote de evidências SNNet-8
   antes de mudar o padrão de transporte para que os revisores possam
   reproduzir os artefatos exatos.

## Próximos passos (SNNet-8a)

- Integrar os coletores Prio duales, conectando a ingestão de compartilhamentos ao tempo de execução
  para que relés e coletores emitam cargas úteis `SoranetPrivacyBucketMetricsV1`
  consistentes. *(Concluído - versão `ingest_privacy_payload` pt
  `crates/sorafs_orchestrator/src/lib.rs` e testes associados.)*
- Publicar o painel Prometheus compartilhado e as regras de alerta que cobrem
  lacunas de supressão, saúde do colecionador e quedas de anonimato. *(Concluído - ver
  `dashboards/grafana/soranet_privacy_metrics.json`,
  `dashboards/alerts/soranet_privacy_rules.yml`,
  `dashboards/alerts/soranet_policy_rules.yml`, e acessórios de validação.)*
- Produzir os artefatos de calibração de privacidade diferencial descritos em
  `privacy_metrics_dp.md`, incluindo notebooks reproduzíveis e resumos de
  governança. *(Concluído - caderno + artefatos gerados por
  `scripts/telemetry/run_privacy_dp.py`; Wrapper de CI
  `scripts/telemetry/run_privacy_dp_notebook.sh` executa o notebook através do
  fluxo de trabalho `.github/workflows/release-pipeline.yml`; resumo de governança arquivado em
  `docs/source/status/soranet_privacy_dp_digest.md`.)*

A versão atual entrega a base SNNet-8: telemetria determinista e segura
para a privacidade que se integra diretamente com raspadores e painéis Prometheus
existentes. Os artefatos de calibração de privacidade diferencial estão listados,
o fluxo de trabalho de liberação mantém frescas as saídas do notebook e o trabalho
O restante é focado no monitoramento da primeira execução automatizada e extensor
a análise de alertas de supressão.