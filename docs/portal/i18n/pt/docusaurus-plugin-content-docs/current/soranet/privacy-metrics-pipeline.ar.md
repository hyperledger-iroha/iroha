---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pipeline de métricas de privacidade
título: مسار مقاييس الخصوصية no SoraNet (SNNet-8)
sidebar_label: مسار مقاييس الخصوصية
description: جمع telemetria مع الحفاظ على الخصوصية لمرحلـات SoraNet e orquestradores.
---

:::note المصدر القياسي
Modelo `docs/source/soranet/privacy_metrics_pipeline.md`. Não se preocupe, você pode fazer isso sem problemas.
:::

# مسار مقاييس الخصوصية no SoraNet

O SNNet-8 usa telemetria e relé de transmissão. O relé é o handshake e o circuito dos buckets, o que significa que o handshake e o circuito são os buckets Prometheus. محافظا على عدم ربط الدوائر الفردية بينما يمنح المشغلين رؤية قابلة للعمل.

## نظرة عامة على المجمع

- Verifique o valor do produto em `tools/soranet-relay/src/privacy.rs` ou `PrivacyAggregator`.
- يتم فهرسة baldes بدقيقة وقت الجدار (`bucket_secs`, 60 ثانية) وتخزينها في حلقة Método (`max_completed_buckets`, número 120). تحتفظ ações الخاصة بالـ colecionadores بمتأخرات محدودة (`max_share_lag_buckets`, الافتراضي 12) بحيث يتم تفريغ نوافذ Prio Os كبuckets suprimidos são de grande importância para os coletores e para os coletores.
- `RelayConfig::privacy` يطابق مباشرة `PrivacyConfig` e ويعرض مفاتيح الضبط (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`). بيئة التشغيل الانتاجية تبقي القيم الافتراضية بينما يقدم SNNet-8a عتبات تجميع امن.
- Ajudantes de tempo de execução como: `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`, `record_verified_bytes` e `record_gar_category`.

## نقطة admin para relé

يمكن للمشغلين استطلاع مستمع admin para relé من اجل ملاحظات خام عبر `GET /privacy/events`. O JSON do arquivo JSON (`application/x-ndjson`) é definido como `SoranetPrivacyEventV1` do `PrivacyEventBuffer` الداخلي. Verifique o valor do código `privacy.event_buffer_capacity` (4096) e instale-o. Todos os raspadores devem ser usados com cuidado. تغطي الاحداث نفس اشارات handshake e acelerador e largura de banda verificada e circuito ativo e GAR التي تغذي عدادات Prometheus, مما يسمح para coletores no site Migalhas de pão são usadas no pão ralado e na farinha de rosca.

## Relé de transmissão

Você pode usar o relé de telemetria no relé `privacy`:

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

O código de rede do SNNet-8 é o seguinte:

| الحقل | الوصف | الافتراضي |
|-------|-------|-----------|
| `bucket_secs` | عرض كل نافذة تجميع (ثوان). | `60` |
| `min_handshakes` | O balde está cheio de água. | `12` |
| `flush_delay_buckets` | Você pode usar baldes no lugar certo para usá-los. | `1` |
| `force_flush_buckets` | O balde está suprimido. | `6` |
| `max_completed_buckets` | Os baldes são usados ​​​​para armazenar baldes (تمنع ذاكرة غير محدودة). | `120` |
| `max_share_lag_buckets` | As ações do colecionador são suprimidas. | `12` |
| `expected_shares` | عدد ações de colecionador Prio المطلوبة قبل الدمج. | `2` |
| `event_buffer_capacity` | متأخرات احداث NDJSON para admin. | `4096` |Use `force_flush_buckets` para `flush_delay_buckets`, e defina o código de barras e instale-o. لتجنب عمليات نشر قد تسرب telemetria para relé.

O `event_buffer_capacity` é o `/admin/privacy/events`, mas você pode usar raspadores para fora do lugar.

## Ações de colecionador Prio

Coletores SNNet-8a مزدوجين يصدرون baldes Prio ذات مشاركة سرية. O orquestrador é responsável por `/privacy/events` NDJSON por `SoranetPrivacyEventV1` e compartilha `SoranetPrivacyPrioShareV1`, e por `SoranetSecureAggregator::ingest_prio_share`. Os baldes principais são usados ​​​​e `PrivacyBucketConfig::expected_shares` para serem usados ​​​​no relé do relé. As ações do bucket e do histograma estão no `SoranetPrivacyBucketMetricsV1`. Para fazer o handshake de `min_contributors`, você precisa usar o balde para `suppressed` para não usar o balde. relé. A função suprimida do `suppression_reason` é suprimida pelo sistema `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed`, e `forced_flush_window_elapsed` são recursos de telemetria. سبب `collector_window_elapsed` يطلق ايضا عندما تبقى Prio share لما بعد `max_share_lag_buckets`, مما يجعل coletores العالقين مرئيين دون Você pode fazer isso em qualquer lugar.

## Torii

يعرض Torii آن نقطتي HTTP محميتين بالـ telemetria بحيث يمكن للـ relés e coletores تمرير الملاحظات بدون تضمين نقل Exemplo:

- `POST /v2/soranet/privacy/event` é igual a `RecordSoranetPrivacyEventDto`. O código `SoranetPrivacyEventV1` é compatível com `source`. O Torii é um recurso de telemetria, o código de acesso é o HTTP `202 Accepted` do tipo HTTP `202 Accepted` Norito JSON é definido como um relé (`bucket_start_unix`, `bucket_duration_secs`) e relé.
- `POST /v2/soranet/privacy/share` é o mesmo que `RecordSoranetPrivacyShareDto`. Os coletores `SoranetPrivacyPrioShareV1` e `forwarded_by` são usados ​​para coletar colecionadores. Identificando HTTP `202 Accepted` com Norito JSON, coletor, bucket e supressão; Você pode usar a telemetria do `Conversion` para obter informações sobre telemetria Colecionadores de حتمية عبر. تقوم حلقة احداث orquestrador الآن باصدار هذه ações عند استطلاع relés, لتحافظ على تزامن مجمع Prio في Torii tem baldes ou relé.

تحترم النقطتان ملف telemetry: تصدران `503 Service Unavailable` عندما تكون المقاييس معطلة. Você pode usar o Norito como (`application/x.norito`) e Norito JSON (`application/x.norito+json`), ويتفاوض Certifique-se de que o produto esteja conectado ao Torii.

## Nome Prometheus

Este balde contém `mode` (`entry`, `middle`, `exit`) e `bucket_start`. يتم اصدار عائلات المقاييس التالية:| Métrica | Descrição |
|--------|------------|
| `soranet_privacy_circuit_events_total{kind}` | Aperto de mão simples com `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | Acelerador de emergência com `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Isso significa cooldown e apertos de mão estrangulados. |
| `soranet_privacy_verified_bytes_total` | largura de banda محققة من اثباتات قياس معماة. |
| `soranet_privacy_active_circuits_{avg,max}` | Coloque um balde no balde. |
| `soranet_privacy_rtt_millis{percentile}` | Verifique o RTT (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | عدادات Relatório de Ação de Governança المجزأة حسب digest الفئة. |
| `soranet_privacy_bucket_suppressed` | baldes não podem ser usados. |
| `soranet_privacy_pending_collectors{mode}` | مجمعات ações de colecionador المعلقة قبل الدمج, مجمعة حسب وضع relé. |
| `soranet_privacy_suppression_total{reason}` | Os buckets suprimidos em `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` são removidos do sistema. |
| `soranet_privacy_snapshot_suppression_ratio` | نسبة suprimido/المصفاة لآخر dreno (0-1), مفيدة لميزانيات التنبيه. |
| `soranet_privacy_last_poll_unixtime` | O UNIX não é compatível com o coletor (coletor ocioso). |
| `soranet_privacy_collector_enabled` | medidor يتحول الى `0` عندما يتعطل coletor الخصوصية او يفشل بالبدء (يغذي تنبيه coletor-desativado). |
| `soranet_privacy_poll_errors_total{provider}` | Você pode usar o relé de alias (sem usar o HTTP e o HTTP). حالة غير متوقعة). |

Os baldes de água fria podem ser usados ​​para armazenar baldes ou baldes de água.

## ارشادات التشغيل

1. **لوحات المعلومات** - ارسم المقاييس اعلاه مجمعة حسب `mode` e `window_start`. Verifique o coletor e o relé. Use `soranet_privacy_suppression_total{reason}` para coletar dados de supressão de coletores e colecionadores. يشحن اصل Grafana الآن لوحة **"Motivos de supressão (5m)"** مخصصة تغذيها تلك العدادات, بالاضافة stat **"Balde suprimido %"** O código `sum(soranet_privacy_bucket_suppressed) / count(...)` pode ser usado para alterar o valor do arquivo. سلسلة **"Collector Share Backlog"** (`soranet_privacy_pending_collectors`) e stat **"Snapshot Suppression Ratio"** تبرز coletores العالقين وانجراف الميزانية اثناء التشغيل الآلي.
2. **التنبيه** - قم بقيادة الانذارات من عدادات آمنة للخصوصية: ارتفاعات رفض PoW, تواتر cooldown, انجراف RTT, ورفض السعة. Não há necessidade de usar o balde, para que você possa usar o balde.
3. **استجابة الحوادث** - اعتمد على البيانات المجمعة اولا. Você pode usar os relés para usar os baldes e os baldes e os relés Isso é tudo que você precisa fazer.
4. **الاحتفاظ** - اسحب البيانات بما يكفي لتجنب تجاوز `max_completed_buckets`. Todos os exportadores devem usar baldes Prometheus para baldes e baldes.

## تحليلات supressão e والتشغيل الآلي

يعتمد قبول SNNet-8 على اثبات ان coletores الآليين يبقون بحالة جيدة وان supressão تبقى ضمن حدود السياسة (≤10% Os baldes para o relé são cerca de 30 minutos). O que você precisa saber é que você pode fazer isso sozinho. Não há nenhum problema com isso. Obtenha o recurso de supressão em Grafana para usar o PromQL, mas não o problema قبل اللجوء الى استعلامات يدوية.

### Como usar o PromQL para suprimir a supressãoVocê pode usar o PromQL para obter mais informações sobre o PromQL. كلاهما مذكور في لوحة Grafana código (`dashboards/grafana/soranet_privacy_metrics.json`) e Alertmanager:

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

استخدم مخرجات النسبة للتاكد من ان stat **"Suppressed Bucket %"** يبقى تحت ميزانية السياسة؛ اربط كاشف الارتفاعات بـ Alertmanager للحصول على اشعار سريع عند انخفاض عدد المساهمين بشكل غير Obrigado.

### اداة تقرير baldes خارجية

O espaço de trabalho é `cargo xtask soranet-privacy-report` para definir NDJSON como padrão. وجهه الى واحد او اكثر من exports admin para relé:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

يمرر المساعد الالتقاط عبر `SoranetSecureAggregator`, ويطبع ملخص supressão stdout, ويكتب اختياريا تقرير JSON منظم عبر `--json-out <path|->`. Coletor de coleta de dados (`--bucket-secs`, `--min-contributors`, `--expected-shares`, الخ), مما يسمح للمشغلين باعادة Verifique se o produto está funcionando corretamente. O JSON do Grafana pode ser usado para suprimir a supressão do SNNet-8.

### قائمة تدقيق التشغيل الآلي الاول

Não há necessidade de supressão de supressão. A configuração do `--max-suppression-ratio <0-1>` permite que os buckets sejam suprimidos. O valor é (10%) e não há baldes. A resposta é:

1. Use NDJSON como admin para o relé, usando o `/v2/soranet/privacy/event|share` para o orquestrador `artifacts/sorafs_privacy/<relay>.ndjson`.
2. شغل المساعد مع ميزانية السياسة:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   يطبع الامر النسبة المرصودة ويخرج برمز غير صفري عند تجاوز الميزانية **او** عندما لا تكون baldes جاهزة Você pode usar a telemetria para obter mais informações. Você pode usar o `soranet_privacy_pending_collectors` para instalar o `soranet_privacy_snapshot_suppression_ratio` ou `soranet_privacy_snapshot_suppression_ratio`. Não use mais.
3. Configure JSON e CLI para usar o SNNet-8 para configurar o site do site. Não use nada.

## Rede de computadores (SNNet-8a)

- دمج Prio coletores المزدوجين وربط ادخال ações em tempo de execução حتى تصدر relés e coletores حمولات `SoranetPrivacyBucketMetricsV1` متسقة. *(تم — راجع `ingest_privacy_payload` em `crates/sorafs_orchestrator/src/lib.rs` والاختبارات المصاحبة.)*
- نشر لوحة Prometheus المشتركة وقواعد التنبيه التي تغطي فجوات supressão وصحة coletores وتراجع اخفاء الهوية. *(تم — راجع `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` e ملفات التحقق.)*
- انتاج مواد معايرة الخصوصية التفاضلية الموضحة في `privacy_metrics_dp.md` بما في ذلك دفاتر قابلة لاعادة الانتاج وملخصات الحوكمة. *(تم — تم توليد الدفتر والمواد بواسطة `scripts/telemetry/run_privacy_dp.py`; يقوم غلاف CI `scripts/telemetry/run_privacy_dp_notebook.sh` بتشغيل الدفتر عبر مسار العمل `.github/workflows/release-pipeline.yml`;

يقدم الاصدار الحالي اساس SNNet-8: telemetria حتمية وآمنة للخصوصية تتصل مباشرة بـ raspadores e painéis Prometheus الحالية. مواد معايرة الخصوصية التفاضلية في مكانها, ومسار عمل release يحافظ على مخرجات الدفتر حديثة, والعمل المتبقي يركز على مراقبة اول تشغيل آلي وتوسيع تحليلات تنبيهات supressão.