---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pipeline de métricas de privacidade
título: SoraNet پرائیویسی میٹرکس پائپ لائن (SNNet-8)
sidebar_label: پرائیویسی میٹرکس پائپ لائن
description: SoraNet کے relés e orquestradores کے لئے پرائیویسی محفوظ telemetria جمع کرنا۔
---

:::nota مستند ماخذ
`docs/source/soranet/privacy_metrics_pipeline.md` کی عکاسی کرتا ہے۔ پرانے docs کے ختم ہونے تک دونوں نقول ہم وقت رکھیں۔
:::

# SoraNet پرائیویسی میٹرکس پائپ لائن

Tempo de execução do relé SNNet-8 کے لئے پرائیویسی کو مدنظر رکھنے والی telemetria relé اب handshake اور circuito واقعات کو ایک منٹ کے baldes میں جمع کرتا ہے اور صرف موٹے Contadores Prometheus برآمد کرتا ہے، جس سے انفرادی circuitos não vinculáveis

## Agregador کا خلاصہ

- tempo de execução کی implementação `tools/soranet-relay/src/privacy.rs` میں `PrivacyAggregator` کے طور پر موجود ہے۔
- baldes کو relógio de parede منٹ کے حساب سے chave کیا جاتا ہے (`bucket_secs`, padrão 60 segundos) اور انہیں ایک محدود anel (`max_completed_buckets`, padrão 120) میں رکھا جاتا ہے۔ compartilhamentos de coletor اپنا محدود backlog رکھتے ہیں (`max_share_lag_buckets`, padrão 12) تاکہ پرانے Prio windows suprimido buckets کے طور پر flush ہوں اور vazamento de memória یا coletores presos چھپ نہ جائیں۔
- `RelayConfig::privacy` براہ راست `PrivacyConfig` میں map ہوتا ہے اور botões de afinação (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`) ظاہر کرتا ہے۔ padrões de tempo de execução de produção Limites de agregação segura SNNet-8a متعارف کراتا ہے۔
- tempo de execução ماڈیولز ajudantes digitados کے ذریعے eventos ریکارڈ کرتے ہیں: `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`, `record_verified_bytes`, ou `record_gar_category`۔

## Endpoint de administração de retransmissão

آپریٹرز `GET /privacy/events` کے ذریعے relé کے admin listener کو poll کر کے observações brutas لے سکتے ہیں۔ یہ JSON delimitado por nova linha de ponto de extremidade (`application/x-ndjson`) واپس کرتا ہے جس میں Cargas úteis `SoranetPrivacyEventV1` ہوتے ہیں جو اندرونی `PrivacyEventBuffer` سے espelho ہوتے ہیں۔ buffer تازہ ترین eventos کو `privacy.event_buffer_capacity` entradas تک رکھتا ہے (padrão 4096) اور ler پر drenar ہو جاتا ہے, اس لئے raspadores کو lacunas سے بچنے کیلئے کافی بار enquete کرنا چاہئے۔ eventos وہی handshake, acelerador, largura de banda verificada, circuito ativo, اور sinais GAR کور کرتے ہیں جو contadores Prometheus کو طاقت دیتے ہیں, تاکہ coletores downstream پرائیویسی محفوظ breadcrumbs محفوظ کر سکیں یا fluxos de trabalho de agregação seguros کو feed کریں۔

## Configuração do relé

Configuração do relé آپریٹرز فائل کے `privacy` سیکشن کے ذریعے پرائیویسی cadência de telemetria ایڈجسٹ کرتے ہیں:

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

Padrões de campo Especificação SNNet-8 کے مطابق ہیں اور carregar کے وقت validar ہوتے ہیں:| Campo | Descrição | Padrão |
|-------|-------------|---------|
| `bucket_secs` | ہر janela de agregação کی چوڑائی (segundos)۔ | `60` |
| `min_handshakes` | کم از کم contagem de contribuidores جس کے بعد contadores de balde emitem کر سکتا ہے۔ | `12` |
| `flush_delay_buckets` | flush کی کوشش سے پہلے مکمل baldes کی تعداد۔ | `1` |
| `force_flush_buckets` | emissão de balde suprimido کرنے سے پہلے زیادہ سے زیادہ عمر۔ | `6` |
| `max_completed_buckets` | محفوظ شدہ bucket backlog (unbounded memory سے بچاتا ہے)۔ | `120` |
| `max_share_lag_buckets` | supressão سے پہلے compartilhamentos de coletor کی janela de retenção۔ | `12` |
| `expected_shares` | combinar کرنے سے پہلے درکار Ações de colecionador Prio۔ | `2` |
| `event_buffer_capacity` | stream de administrador کیلئے backlog de eventos NDJSON۔ | `4096` |

`force_flush_buckets` کو `flush_delay_buckets` سے کم کرنا, limites کو صفر کرنا, یا guarda de retenção کو بند کرنا اب falha de validação کرتا ہے تاکہ ایسی implantações سے بچا جائے جو vazamento de telemetria por relé کریں۔

`event_buffer_capacity` کی حد `/admin/privacy/events` کو بھی vinculado کرتی ہے, تاکہ raspadores غیر معینہ مدت تک پیچھے نہ رہ جائیں۔

## Ações de colecionador Prio

Coletores duplos SNNet-8a تعینات کرتا ہے جو baldes Prio compartilhados em segredo emitem کرتے ہیں۔ orquestrador اب `/privacy/events` fluxo NDJSON کو entradas `SoranetPrivacyEventV1` اور `SoranetPrivacyPrioShareV1` compartilhamentos دونوں کیلئے analisar کرتا ہے اور انہیں `SoranetSecureAggregator::ingest_prio_share` میں forward کرتا ہے۔ baldes اس وقت emitir ہوتے ہیں جب `PrivacyBucketConfig::expected_shares` contribuições آ جائیں، جو comportamento de retransmissão کی عکاسی ہے۔ ações کو alinhamento do balde اور forma do histograma کیلئے validar کیا جاتا ہے, پھر انہیں `SoranetPrivacyBucketMetricsV1` میں combinar کیا جاتا ہے۔ Contagem de handshake combinada `min_contributors` سے نیچے چلا جائے تو bucket کو `suppressed` کے طور پر exportar کیا جاتا ہے، جو comportamento do agregador in-relay جیسا ہے۔ janelas suprimidas اب `suppression_reason` rótulo emit کرتے ہیں تاکہ آپریٹرز `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed`, اور `forced_flush_window_elapsed` کے درمیان فرق کر سکیں جب lacunas de telemetria کی تشخیص کر رہے ہوں۔ `collector_window_elapsed` کی وجہ تب بھی fogo ہوتی ہے جب Prio compartilha `max_share_lag_buckets` سے زیادہ دیر رہیں, جس سے colecionadores presos نظر آتے ہیں اور memória de acumuladores obsoletos میں نہیں رہتے۔

## Pontos de extremidade de ingestão Torii

Torii tem pontos de extremidade HTTP controlados por telemetria فراہم کرتا ہے تاکہ relés e coletores کسی transporte sob medida کے بغیر observações futuras کر سکیں:- `POST /v2/soranet/privacy/event` ایک `RecordSoranetPrivacyEventDto` carga útil قبول کرتا ہے۔ corpo میں `SoranetPrivacyEventV1` کے ساتھ ایک rótulo `source` opcional شامل ہوتا ہے۔ Solicitação Torii کو فعال perfil de telemetria کے مطابق validar کرتا ہے, evento ریکارڈ کرتا ہے, e HTTP `202 Accepted` کے ساتھ Envelope JSON Norito e janela de bucket computada (`bucket_start_unix`, `bucket_duration_secs`) e modo de retransmissão.
- `POST /v2/soranet/privacy/share` ایک `RecordSoranetPrivacyShareDto` carga útil قبول کرتا ہے۔ corpo میں `SoranetPrivacyPrioShareV1` اور opcional `forwarded_by` dica ہوتا ہے تاکہ آپریٹرز fluxos de coletor کا auditoria کر سکیں۔ Submissões HTTP `202 Accepted` کے ساتھ Norito Envelope JSON واپس کرتی ہیں جو coletor, janela de balde, اور dica de supressão کو resumir کرتا ہے؛ falhas de validação telemetria `Conversion` resposta سے mapa ہوتے ہیں تاکہ coletores کے درمیان tratamento determinístico de erros برقرار رہے۔ orquestrador کا loop de eventos اب polling de retransmissão کے دوران یہ compartilhamentos emitem کرتا ہے، جس سے Torii کا Buckets on-relay do acumulador Prio کے ساتھ sincronização رہتا ہے۔

دونوں perfil de telemetria de endpoints کی پابندی کرتے ہیں: métricas desativadas clientes Norito binário (`application/x.norito`) یا Norito JSON (`application/x.norito+json`) corpos بھیج سکتے ہیں؛ extratores Torii padrão do servidor کے ذریعے formato خود بخود negociar کرتا ہے۔

## Métricas Prometheus

ہر balde exportado میں `mode` (`entry`, `middle`, `exit`) اور `bucket_start` etiquetas ہوتے ہیں۔ As famílias métricas emitem o seguinte:

| Métrica | Descrição |
|--------|------------|
| `soranet_privacy_circuit_events_total{kind}` | taxonomia de aperto de mão `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}` شامل ہے۔ |
| `soranet_privacy_throttles_total{scope}` | contadores de acelerador جن میں `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}` ہے۔ |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | apertos de mão acelerados سے آنے والی cooldown مدتوں کا مجموعہ۔ |
| `soranet_privacy_verified_bytes_total` | provas de medição cegas سے largura de banda verificada۔ |
| `soranet_privacy_active_circuits_{avg,max}` | ہر balde میں circuitos ativos کا اوسط اور زیادہ سے زیادہ۔ |
| `soranet_privacy_rtt_millis{percentile}` | Estimativas de percentil RTT (`p50`, `p90`, `p99`)۔ |
| `soranet_privacy_gar_reports_total{category_hash}` | contadores hash do Relatório de Ação de Governança جو resumo da categoria سے chave ہوتے ہیں۔ |
| `soranet_privacy_bucket_suppressed` | وہ buckets جو limite de contribuição پورا نہ ہونے کی وجہ سے روکے گئے۔ |
| `soranet_privacy_pending_collectors{mode}` | acumuladores de compartilhamento de coletor جو combinar ہونے کے منتظر ہیں، modo de retransmissão کے حساب سے گروپ کیے گئے۔ |
| `soranet_privacy_suppression_total{reason}` | contadores de bucket suprimidos جن میں `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` شامل ہیں تاکہ lacunas de privacidade dos painéis کو نسبت دے سکیں۔ |
| `soranet_privacy_snapshot_suppression_ratio` | آخری drenagem کا proporção suprimida/drenada (0-1), orçamentos de alerta کیلئے مفید۔ |
| `soranet_privacy_last_poll_unixtime` | آخری کامیاب poll کا carimbo de data/hora UNIX (alerta de coletor ocioso کو چلانے کیلئے)۔ |
| `soranet_privacy_collector_enabled` | medidor جو اس وقت `0` ہو جاتا ہے جب coletor de privacidade بند ہو یا start ہونے میں ناکام ہو (alerta de coletor desativado کیلئے)۔ |
| `soranet_privacy_poll_errors_total{provider}` | falhas de pesquisa جو alias de retransmissão کے حساب سے گروپ ہوتے ہیں (erros de decodificação, falhas de HTTP, یا غیر متوقع códigos de status پر بڑھتے ہیں)۔ |جن baldes میں observações نہیں ہوتیں وہ خاموش رہتے ہیں, جس سے painéis صاف رہتے ہیں اور janelas preenchidas com zero نہیں بنتیں۔

## Orientação operacional

1. **Dashboards** - اوپر والی métricas کو `mode` اور `window_start` کے حساب سے گروپ کر کے چارٹ کریں۔ janelas faltando کو destaque کریں تاکہ coletor یا relé مسائل سامنے آئیں۔ `soranet_privacy_suppression_total{reason}` استعمال کریں تاکہ lacunas کی triagem میں déficit de contribuidor اور supressão orientada por coletor کے درمیان فرق ہو سکے۔ Grafana ativo اب ایک dedicado **"Motivos de supressão (5m)"** painel فراہم کرتا ہے جو ان contadores سے چلتا ہے, ساتھ ہی ایک **"Balde suprimido %"** stat جو `sum(soranet_privacy_bucket_suppressed) / count(...)` فی seleção حساب کرتا ہے تاکہ آپریٹرز violações de orçamento ایک نظر میں دیکھ سکیں۔ **"Collector Share Backlog"** série (`soranet_privacy_pending_collectors`) اور **"Snapshot Suppression Ratio"** coletores travados em estatísticas اور execuções automatizadas کے دوران desvio de orçamento کو نمایاں کرتے ہیں۔
2. **Alertas** - contadores seguros para privacidade e alarmes چلائیں: picos de rejeição de PoW, frequência de resfriamento, desvio de RTT, e rejeições de capacidade۔ چونکہ ہر balde کے اندر contadores monotônicos ہوتے ہیں, سادہ regras baseadas em taxa اچھا کام کرتے ہیں۔
3. **Resposta a incidentes** - Dados agregados پر انحصار کریں۔ جب گہرے depuração کی ضرورت ہو تو relés سے repetição de snapshots de bucket کروائیں یا provas de medição cegas دیکھیں, logs de tráfego brutos جمع نہ کریں۔
4. **Retenção** - `max_completed_buckets` سے تجاوز سے بچنے کیلئے کافی بار raspar کریں۔ exportadores کو saída Prometheus کو fonte canônica سمجھنا چاہئے اور forward کرنے کے بعد buckets locais حذف کر دینے چاہئیں۔

## Análise de supressão e execuções automatizadas

SNNet-8 کی قبولیت اس بات پر منحصر ہے کہ coletores automatizados صحت مند رہیں e supressão پالیسی حدود کے اندر رہے (ہر relé کیلئے کسی بھی 30 منٹ کی janela میں ≤10% buckets)۔ Portão کو پورا کرنے کیلئے درکار ferramentas اب repo کے ساتھ آتا ہے؛ آپریٹرز کو اسے اپنی ہفتہ وار rituais میں شامل کرنا ہوگا۔ نئے Grafana painéis de supressão نیچے دیے گئے PromQL snippets کو منعکس کرتے ہیں، جس سے on-call ٹیموں کو visibilidade ao vivo ملتی ہے اس سے پہلے کہ انہیں consultas manuais کی ضرورت پڑے۔

### Revisão de supressão de receitas PromQL

آپریٹرز کو درج ذیل Ajudantes PromQL ساتھ رکھنے چاہئیں؛ O painel Grafana compartilhado (`dashboards/grafana/soranet_privacy_metrics.json`) e as regras do Alertmanager são referenciados ہیں:

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

saída de proporção استعمال کریں تاکہ **"Balde Suprimido %"** stat پالیسی orçamento سے نیچے رہے؛ detector de pico کو Alertmanager سے جوڑیں تاکہ contribuidores کی تعداد غیر متوقع طور پر کم ہونے پر فوری فیڈبیک ملے۔

### CLI de relatório de intervalo off-line

espaço de trabalho میں `cargo xtask soranet-privacy-report` ایک وقتی NDJSON captura کیلئے دستیاب ہے۔ اسے ایک یا زیادہ relé admin exporta پر ponto کریں:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```یہ captura auxiliar کو `SoranetSecureAggregator` کے ذریعے stream کرتا ہے, stdout پر resumo de supressão پرنٹ کرتا ہے, اور اختیاری طور پر `--json-out <path|->` é um relatório JSON estruturado. یہ colecionador ao vivo e botões (`--bucket-secs`, `--min-contributors`, `--expected-shares`, وغیرہ) کو honor کرتا ہے, جس سے آپریٹرز questões کی triagem میں مختلف limites کے ساتھ repetição de capturas históricas کر سکتے ہیں۔ Portão de análise de supressão SNNet-8 کو auditoria کے قابل رکھنے کیلئے JSON کو Grafana capturas de tela کے ساتھ anexar کریں۔

### Lista de verificação de execução automatizada

Governança اب بھی ثبوت چاہتا ہے کہ پہلی automação executar supressão orçamento میں رہی۔ helper اب `--max-suppression-ratio <0-1>` قبول کرتا ہے تاکہ CI یا آپریٹرز اس وقت fail fast کر سکیں جب buckets suprimidos permitidos janela سے بڑھ جائیں (padrão 10%) یا جب ابھی کوئی baldes موجود نہ ہوں۔ Fluxo de fluxo:

1. endpoint(s) de administração de retransmissão por orquestrador کے fluxo `/v2/soranet/privacy/event|share` سے exportação NDJSON کر کے `artifacts/sorafs_privacy/<relay>.ndjson` میں محفوظ کریں۔
2. política orçamentária کے ساتھ ajudante چلائیں:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   کمانڈ proporção observada پرنٹ کرتی ہے اور saída diferente de zero کرتی ہے جب orçamento excede ہو **یا** جب baldes ابھی تیار نہ ہوں، جس سے اشارہ ملتا ہے کہ telemetria ابھی executar کیلئے پیدا نہیں ہوئی۔ métricas ao vivo رہتا ہے جب executar چل رہا ہو۔
3. padrão de transporte بدلنے سے پہلے Saída JSON اور CLI log کو Pacote de evidências SNNet-8 کے ساتھ arquivo کریں تاکہ revisores وہی artefatos دوبارہ چلا سکیں۔

## Próximas etapas (SNNet-8a)

- coletores Prio duplos کو integrar کریں اور ان کے compartilhar ingestão کو tempo de execução سے جوڑیں تاکہ relés اور coletores consistentes cargas úteis `SoranetPrivacyBucketMetricsV1` emitem کریں۔ *(Concluído - `crates/sorafs_orchestrator/src/lib.rs` میں `ingest_privacy_payload` اور متعلقہ testes دیکھیں۔)*
- JSON do painel Prometheus compartilhado e regras de alerta شائع کریں جو lacunas de supressão, saúde do coletor, اور quedas de anonimato کو cobertura کریں۔ *(Concluído - `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` e acessórios de validação دیکھیں۔)*
- `privacy_metrics_dp.md` میں بیان کردہ artefatos de calibração de privacidade diferencial تیار کریں, جن میں notebooks reproduzíveis اور resumos de governança شامل ہوں۔ *(Concluído - notebook اور artefatos `scripts/telemetry/run_privacy_dp.py` سے بنتے ہیں؛ CI wrapper `scripts/telemetry/run_privacy_dp_notebook.sh` notebook کو `.github/workflows/release-pipeline.yml` fluxo de trabalho کے ذریعے چلاتا ہے؛ resumo de governança `docs/source/status/soranet_privacy_dp_digest.md` میں فائل کیا گیا ہے۔)*

موجودہ release SNNet-8 کی بنیاد فراہم کرتی ہے: determinístico, telemetria com privacidade segura جو براہ راست موجودہ Prometheus scrapers اور dashboards میں فٹ ہوتی ہے۔ artefatos de calibração de privacidade diferencial موجود ہیں، liberar saídas de notebook de fluxo de trabalho de pipeline کو تازہ رکھتا ہے، اور باقی کام پہلی execução automatizada کی monitoramento اور análise de alerta de supressão کو مزید بڑھانے پر مرکوز ہے۔