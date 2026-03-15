---
lang: he
direction: rtl
source: docs/portal/docs/soranet/privacy-metrics-pipeline.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: privacy-metrics-pipeline
כותרת: Pipeline de metricas de privacidade da SoraNet (SNNet-8)
sidebar_label: Pipeline de metricas de privacidade
תיאור: Coleta de telemetria que preserva privacidade para relays e orchestrators da SoraNet.
---

:::שים לב Fonte canonica
Reflete `docs/source/soranet/privacy_metrics_pipeline.md`. Mantenha ambas as copias sincronizadas.
:::

# Pipeline de metricas de privacidade da SoraNet

SNNet-8 מציג את השטחיות של טלמטריה פרטיות עבור ממסר זמן ריצה. או ממסר אגורה אגרי אירועי לחיצת יד ומעגלים עם דליים של um minuto e exporta apenas contadores Prometheus ברוטו, מעגלים מנטנדיים אינדיבידואליים ניתנים להצגה של אקונובל או מפעילים.

## Visao geral do agregador

- A implementacao do runtime fica em `tools/soranet-relay/src/privacy.rs` como `PrivacyAggregator`.
- Buckets sao chaveados por minuto de relogio (`bucket_secs`, ברירת מחדל 60 סגנונות) ו-armazenados em um ring limitado (`max_completed_buckets`, ברירת מחדל 120). Collector shares mantem seu proprio backlog limitado (`max_share_lag_buckets`, ברירת מחדל 12) para que janelas Prio antigas sejam esvaziadas como buckets supprimidos em vez de vazar memoria ou mascarar collectors presos.
- `RelayConfig::privacy` mapeia direto para `PrivacyConfig`, כפתורי expondo de ajuste (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, Prometheus0, I0100, I0100, I0100, I010 `max_share_lag_buckets`, `expected_shares`). O ברירת המחדל של זמן ריצה של producao mantem os enquanto SNNet-8a introduz thresholds de agregacao segura.
- מערכת הפעלה של רישום אירועי זמן ריצה באמצעות טיפים עוזרים: `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, I00000040, I00000004X, `record_capacity_reject`, I018NI40X `record_verified_bytes`, e `record_gar_category`.

## מנהל נקודת קצה עושה ממסר

מפעילי הפודם יועץ או מנהל מאזינים לעשות ממסר עבור צופים ברוטאס דרך `GET /privacy/events`. O נקודת הקצה נקודת הקצה JSON delimitado por novas linhas (`application/x-ndjson`) contendo loads `SoranetPrivacyEventV1` espelhados do `PrivacyEventBuffer` interno. O buffer guarda os eventos mais novos ate `privacy.event_buffer_capacity` entradas (ברירת מחדל 4096) e e drenado na leitura, entao scrapers devem sondar com frequencia suficiente para evitar lacunas. לחיצת יד, מצערת, רוחב פס מאומת, מעגל אקטיבי ו- GAR המשמשים ל-Prometheus, מאפשרים לאספנים במורד הזרם את פירורי הלחם לפרטיות או לזרימת עבודה פרטית.

## Configuracao do relay

מערכת ההפעלה של Ajustam ו-Cadencia de Telemetria de Privacidade ללא מידע על הגדרות ממסר דרך Secao `privacy`:

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

ברירת המחדל של OS dos campos correspondem a especificacao SNNet-8 e sao validados no carregamento:| קמפו | תיאור | פדראו |
|-------|--------|--------|
| `bucket_secs` | Largura de cada janela de agregacao (סגונדוס). | `60` |
| `min_handshakes` | Numero minimo de contribuidores ante de um bucket poder emitir contadores. | `12` |
| `flush_delay_buckets` | דליים משלימים את ההדחה. | `1` |
| `force_flush_buckets` | Idade maxima antes de emitir um bucket supprimido. | `6` |
| `max_completed_buckets` | Backlog de buckets retidos (מעכב מזכרות sem limite). | `120` |
| `max_share_lag_buckets` | Janela de retencao para אספן מניות antes de supprimir. | `12` |
| `expected_shares` | Prio אספן מניות exigidos antes de combinar. | `2` |
| `event_buffer_capacity` | צבר אירועי NDJSON לניהול זרם. | `4096` |

Definir `force_flush_buckets` menor que `flush_delay_buckets`, אפס ספי OS, ou desativar o guard de retencao agora falha na validacao para evitar deployments que vazariam telemetria por relay.

O limite `event_buffer_capacity` tambem limita `/admin/privacy/events`, garantindo que scrapers nao possam ficar atrasados ​​indefinidamente.

## מניות אספן פריו

SNNet-8a implanta אספנים דופלו que emitem דליים Prio com compartilhamento secreto. O מתזמר agora analisa o stream NDJSON `/privacy/events` para entradas `SoranetPrivacyEventV1` e shares `SoranetPrivacyPrioShareV1`, encaminhando-as para `SoranetSecureAggregator::ingest_prio_share`. דליים emitem quando chegam `PrivacyBucketConfig::expected_shares` תרומות, espelhando או comportamento do relay. As shares sao validadas para alinhamento de bucket e forma do histograma antes de serem combinadas em `SoranetPrivacyBucketMetricsV1`. ראה שילוב של לחיצות ידיים עם אביקסו דה `min_contributors`, או דלי ו-exportado como `suppressed`, espelhando או comportamento do agregador no relay. Janelas suprimidas agora emitem um label `suppression_reason` para que operadores possam distinguir entre `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed`, e Prometheus telemetria de diagnose. O motivo `collector_window_elapsed` tambem dispara quando Prio מניות ficam alem de `max_share_lag_buckets`, אספני טורננדו presos visiveis sem deixar acumuladores antigos na memoria.

## נקודות קצה de ingestao do Torii

Torii agora expone dois נקודות קצה HTTP com gating de telemetria para que relays e collectors possam encaminhar observacoes sem embutir um transporte bespoke:- `POST /v1/soranet/privacy/event` aceita um מטען `RecordSoranetPrivacyEventDto`. O corpo envolve um `SoranetPrivacyEventV1` ועם תווית `source` אופציונלי. Torii valida a requisicao contra o perfil de telemetria ativo, registra o evento, e response com HTTP `202 Accepted` junto com um envelope Norito JSON contendo a janela NI30I0ada ( `bucket_duration_secs`) e o modo do ממסר.
- `POST /v1/soranet/privacy/share` aceita um מטען `RecordSoranetPrivacyShareDto`. O corpo carrega um `SoranetPrivacyPrioShareV1` e uma dica `forwarded_by` אופציונלי עבור מפעילי possam auditar fluxos de collectors. Submissoes bem-sucedidas retornam HTTP `202 Accepted` com um envelope Norito JSON resumindo o אספן, a janela de bucket e a dica de supressao; falhas de validacao mapeiam para uma resposta de telemetria `Conversion` para preservar tratamento deterministico de erros entre collectors. O loop de eventos do orchestrator agora emite essas shares ao fazer polling dos relays, mantendo o acumulador Prio do Torii sincronizado com os buckets no relay.

נקודות הקצה של Ambos OS חוזרות על פרופיל טלמטריה: emitem `503 Service Unavailable` quando as metricas estao desativadas. לקוחות פודם enviar corpos Norito בינארי (`application/x.norito`) או Norito JSON (`application/x.norito+json`); o servidor negocia automaticamente o formato via extractors padrao do Torii.

## Metricas Prometheus

Cada דלי exportado carrega תוויות `mode` (`entry`, `middle`, `exit`) ו `bucket_start`. בתור families familias de metricas sao emitidas:

| מדד | תיאור |
|--------|----------------|
| `soranet_privacy_circuit_events_total{kind}` | Taxonomia de handshake com `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | Contadores de throttle com `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Duracoes de cooldown agregadas por לחיצות ידיים מצערות. |
| `soranet_privacy_verified_bytes_total` | אימות רוחב הפס של בדיקות רפואיות. |
| `soranet_privacy_active_circuits_{avg,max}` | Media e pico de circuits ativos por bucket. |
| `soranet_privacy_rtt_millis{percentile}` | אומדן אחוזי RTT (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Contadores de Governance Action Report com hash por digest de categoria. |
| `soranet_privacy_bucket_suppressed` | דליים retidos porque o limiar de contribuidores nao foi atingido. |
| `soranet_privacy_pending_collectors{mode}` | Acumuladores de collector shares pendentes de combinacao, agrupados por modo de relay. |
| `soranet_privacy_suppression_total{reason}` | Contadores de buckets supprimidos com `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` para que לוחות מחוונים atribuam lacunas de privacy. |
| `soranet_privacy_snapshot_suppression_ratio` | Razao suprimida/drenada do ultimo drain (0-1), util para budgets de alerta. |
| `soranet_privacy_last_poll_unixtime` | חותמת זמן UNIX לעשות סקר אולטימטיבי bem-sucedido (alimenta o alerta collector-iddle). |
| `soranet_privacy_collector_enabled` | מד que vira `0` quando o collector de privacidade esta desativado ou falha ao iniciar (alimenta o alerta collector-disabled). |
| `soranet_privacy_poll_errors_total{provider}` | Falhas de polling agrupadas por alias de relay (incrementa emros decode decode, falhas HTTP, ou status codes inesperados). |דליים כמו observacoes permanecem silenciosos, mantendo לוחות מחוונים limpos sem fabricar Janelas Zeradas.

## אוריינטקאו מבצעית

1. **לוחות מחוונים** - עקבות כמו metricas acima agrupadas por `mode` e `window_start`. Destaque Janelas Ausentes לבעיות חושפניות של אספן או ממסר. השתמש ב-`soranet_privacy_suppression_total{reason}` להבדיל בין תרומות אוריינטציות לאספנים או לאספנים. O asset Grafana agora envia um painel dedicado **"Suppression Reasons (5m)"** alimentado por esses contadores mais um stat **"Suppressed Bucket %"** que calcula `sum(soranet_privacy_bucket_suppressed) / count(...)` por selecoao de budget opera rapide. סדרה **"צבר שיתוף אספנים"** (`soranet_privacy_pending_collectors`) e o stat **"יחס דיכוי תמונת מצב"** destacam collectors presos e desvio desvio de budget durante executes automatizadas.
2. **התראה** - הודעה מעוררת אזעקה לסירוגין פרטיות: picos de PoW reject, frequencia de cooldown, drift de RTT ו-rejects. Como os contadores sao monotonos dentro de cada bucket, regras simples baseadas em taxa funcionam bem.
3. **תגובת האירוע** - confie primeiro nos dados agregados. אם יש צורך לבצע ניפוי באגים, בקש ממסרים לשחזר צילומי מצב של דליים או בדיקות רפואיות שונות.
4. **שימור** - faca scrape com frequencia suficiente para evitar exceder `max_completed_buckets`. יצואנים devem tratar a saida Prometheus como fonte canonica e descartar buckets locais depois de encaminhados.

## Analise de supressao e execucoes automatizadas

A aceitacao de SNNet-8 depende de demonstrar que collectors automatizados permanecem saudaveis e que a supressao fica dentro dos limites da politica (<=10% דליים ל-relay em qualquer janela de 30 דקות). O tooling necessario para cumprir esse esse gate agora vem com o repositorio; מפעילים מפתחים אינטגרר איסו אאוס סוס פולחן סמנאי. מערכת ההפעלה החדשה של ה-Grafana חוזרת אל ה-PromQL, ותוסף את המראה החיצוני של הצמחים לפני זמן קצר.

### Receitas PromQL para revisao de supressao

Operadores devem manter os seguintes helpers PromQL a mao; ambos sao referenciados ללא לוח מחוונים Grafana שילוב (`dashboards/grafana/soranet_privacy_metrics.json`) ו-Regraras do Alertmanager:

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

השתמש ב-Saida do ratio para confirmar que o stat **"Suppressed Bucket %"** permanece abaixo do budget de politica; מחובר לגלאי דוקרנים ובין היתר Alertmanager עבור משוב מהיר ואפשר להשפיע על התרומות.

### CLI de relatorio de bucket במצב לא מקוון

O תצוגת סביבת עבודה `cargo xtask soranet-privacy-report` עבור תפיסות NDJSON pontuais. Aponte para um ou mais admin admin de relay:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```O helper pass a captura pelo `SoranetSecureAggregator`, imprime um resumo de supressao no stdout e, optionalmente, grava um relatorio JSON estruturado via `--json-out <path|->`. ידיות ה-Mesmos של Ele honra os dos collector ao vivo (`--bucket-secs`, `--min-contributors`, `--expected-shares`, וכו'), מאפשרים למבצעים חוזרים לתפוס את ההיסטוריות להתייפח בין תקריות משולשות שונות. Anexe o JSON junto com capturas do Grafana para que o gate de analise de supressao SNNet-8 permanece auditavel.

### רשימת בדיקה ראשונית לביצוע אוטומטי

גוברננקה איינדה אקסיג'י פרובר que a primeira execucao automatizada atendeu ao budget de supressao. O helper agora aceita `--max-suppression-ratio <0-1>` para que CI ou operadores falhem rapidamente quando buckets suprimidos excederem a janela permitida (ברירת מחדל 10%) או quando ainda nao houver buckets. Fluxo recomendado:

1. ייצא את נקודות הקצה של NDJSON dos admin לעשות ממסר או זרם `/v1/soranet/privacy/event|share` לעשות מתזמר עבור `artifacts/sorafs_privacy/<relay>.ndjson`.
2. נסע או עוזר com o budget de politica:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   O comando imprime a razao observada e sai com codigo nao zero quando o budget e excedido **ou** quando ainda nao ha buckets prontos, sinalizando que a telemetria ainda nao foi produzida para a execucao. כמטרות אאו ויוו התפתחו רובר `soranet_privacy_pending_collectors` drenando para zero e `soranet_privacy_snapshot_suppression_ratio` ficando abaixo do mesmo budget enquanto a execucao ocorre.
3. ארכיון אסידה JSON e o log da CLI com o pacote de evidencias SNNet-8 antes de trocar או ברירת מחדל לעשות transporte para que revisores possam reproduzir os artefatos exatos.

## Proximos passos (SNNet-8a)

- שילוב של אספני Prio כפולים, חיבור לשיתוף מניות וזמן ריצה עבור ממסרים e emitam emitamloads `SoranetPrivacyBucketMetricsV1` עקביים. *(Concluido - veja `ingest_privacy_payload` em `crates/sorafs_orchestrator/src/lib.rs` e os testes associados.)*
- פרסום לוח המחוונים Prometheus שומר על התראה קודמת, אספנים עשירים ואנונימאטים. *(Concluido - veja `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` e fixtures de validacao.)*
- Produzir os artefatos de calibracao de privacidade diferencial decritos em `privacy_metrics_dp.md`, incluindo notebooks reproduziveis e digests de governanca. *(Concluido - notebook e artefatos gerados por `scripts/telemetry/run_privacy_dp.py`; עטיפה CI `scripts/telemetry/run_privacy_dp_notebook.sh` ביצוע או מחברת דרך o workflow `.github/workflows/release-pipeline.yml`; digest de governanca arquivado em Prometheus.)

שחרור בסיס ל-SNNet-8: טלמטריה ודטרמיניסטית וסיגוריה פרטית que se encaixa diretamente nos scrapers e Dashboards Prometheus existents. Os artefatos de calibracao de privacidade diferencial estao no lugar, o זרימת עבודה לעשות שחרור צינורות יציאות או יציאות למחשב נייד, e o trabalho restante foca no monitoramento da primeira execucao automatizada e na extensao das analises de supressao.