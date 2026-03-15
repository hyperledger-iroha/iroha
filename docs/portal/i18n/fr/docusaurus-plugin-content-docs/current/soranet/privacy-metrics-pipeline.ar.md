---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pipeline de mesures de confidentialité
titre : مسار مقاييس الخصوصية في SoraNet (SNNet-8)
sidebar_label : مسار مقاييس الخصوصية
description : La télémétrie est également utilisée pour SoraNet et les orchestrateurs.
---

:::note المصدر القياسي
Voir `docs/source/soranet/privacy_metrics_pipeline.md`. حافظ على النسختين متطابقتين حتى يتم تقاعد مجموعة الوثائق القديمة.
:::

# مسار مقاييس الخصوصية في SoraNet

Le SNNet-8 est un relais de télémétrie et un relais. Relais pour poignée de main et circuit pour seaux pour relais Prometheus عدم ربط الدوائر الفردية بينما يمنح المشغلين رؤية قابلة للعمل.

## نظرة عامة على المجمع

- Utilisez la fonction `tools/soranet-relay/src/privacy.rs` pour `PrivacyAggregator`.
- يتم فهرسة buckets بدقيقة وقت الجدار (`bucket_secs`, 60 ثانية) وتخزينها في حلقة محدودة (`max_completed_buckets`, article 120). Actions pour les collectionneurs (`max_share_lag_buckets`, 12) pour les collectionneurs Prio القديمة كبuckets supprimés par les collectionneurs et les collectionneurs.
- `RelayConfig::privacy` يطابق مباشرة `PrivacyConfig` ويعرض مفاتيح الضبط (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`). La fonction SNNet-8a est également compatible avec SNNet-8a.
- Les helpers du runtime sont les suivants : `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`, `record_verified_bytes` et `record_gar_category`.

## نقطة admin للـ relais

يمكن للمشغلين استطلاع مستمع admin للـ relay من اجل ملاحظات خام عبر `GET /privacy/events`. JSON محدد الاسطر (`application/x-ndjson`) est également compatible avec `SoranetPrivacyEventV1` منعكسة من `PrivacyEventBuffer` الداخلي. يحتفظ المخزن بأحدث الاحداث حتى `privacy.event_buffer_capacity` إدخالا (الافتراضي 4096) et تفريغه عند القراءة، لذا يجب على grattoirs الاستطلاع بما يكفي لتفادي الفجوات. Il s'agit d'une poignée de main, d'un papillon, d'une bande passante vérifiée et d'un circuit actif et du GAR pour les collecteurs Prometheus. بأرشفة le fil d'Ariane امنة للخصوصية او تغذية سير عمل التجميع الامن.

## اعدادات relais

La télémétrie est compatible avec les relais de relais `privacy` :

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

La fonction SNNet-8 est également compatible avec les éléments suivants :

| الحقل | الوصف | الافتراضي |
|-------|-------|---------------|
| `bucket_secs` | عرض كل نافذة تجميع (ثوان). | `60` |
| `min_handshakes` | Il s'agit d'un seau à eau. | `12` |
| `flush_delay_buckets` | عدد buckets المكتملة التي ننتظرها قبل محاولة التفريغ. | `1` |
| `force_flush_buckets` | العمر الاقصى قبل اصدار bucket supprimé. | `6` |
| `max_completed_buckets` | متأخرات buckets المحتفظ بها (تمنع ذاكرة غير محدودة). | `120` |
| `max_share_lag_buckets` | نافذة الاحتفاظ لـ actions de collectionneur قبل suppression. | `12` |
| `expected_shares` | عدد Actions de collection Prio المطلوبة قبل الدمج. | `2` |
| `event_buffer_capacity` | متأخرات احداث NDJSON لتدفق admin. | `4096` |ضبط `force_flush_buckets` اقل من `flush_delay_buckets`, او تصفير العتبات، او تعطيل حارس الاحتفاظ يفشل التحقق الآن لتجنب عمليات نشر قد تسرب télémétrie لكل relais.

حد `event_buffer_capacity` يقيد ايضا `/admin/privacy/events`, مما يضمن عدم تمكن scrapers من التاخر بلا نهاية.

## Actions de collection Prio

Les collecteurs SNNet-8a sont dotés de seaux Prio. L'orchestrateur est basé sur `/privacy/events` NDJSON pour `SoranetPrivacyEventV1` et partage `SoranetPrivacyPrioShareV1`, et `SoranetSecureAggregator::ingest_prio_share`. تصدر buckets byمجرد وصول `PrivacyBucketConfig::expected_shares` مساهمات، بما يعكس سلوك relay. Il s'agit d'un compartiment de partages et d'un histogramme pour `SoranetPrivacyBucketMetricsV1`. Il s'agit de la poignée de main `min_contributors` et du seau `suppressed` pour le relais. تصدر النوافذ supprimé الآن تسمية `suppression_reason`, `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed` et `forced_flush_window_elapsed` sont des outils de télémétrie. `collector_window_elapsed` pour les collectionneurs Prio share pour `max_share_lag_buckets` pour les collectionneurs مجمعات قديمة في الذاكرة.

## نقاط ادخال Torii

Utilisez Torii pour le protocole HTTP pour la télémétrie pour les relais et les collecteurs comme suit :

- `POST /v2/soranet/privacy/event` correspond à `RecordSoranetPrivacyEventDto`. يلف الجسم `SoranetPrivacyEventV1` مع تسمية `source` اختيارية. Torii pour la télémétrie et la télémétrie via HTTP `202 Accepted` pour Norito JSON يحتوي على نافذة الحساب (`bucket_start_unix`, `bucket_duration_secs`) et relais.
- `POST /v2/soranet/privacy/share` correspond à `RecordSoranetPrivacyShareDto`. يحمل الجسم `SoranetPrivacyPrioShareV1` et `forwarded_by` sont des collectionneurs. Utilisez le protocole HTTP `202 Accepted` avec Norito JSON pour le collecteur et le bucket et la suppression. بينما يتم ربط اخفاقات التحقق باستجابة telemetry من نوع `Conversion` للحفاظ على معالجة اخطاء حتمية عبر collectionneurs. Il s'agit d'un orchestrateur pour les partages et d'un relais pour Torii. seaux على relais.

Utilisez la télémétrie : utilisez `503 Service Unavailable` pour la télémétrie. JSON Norito (`application/x.norito`) et Norito JSON (`application/x.norito+json`) sont également disponibles. خادم تلقائيا على الصيغة عبر مستخلصات Torii القياسية.

## مقاييس Prometheus

Le seau contient les éléments `mode` (`entry`, `middle`, `exit`) et `bucket_start`. يتم اصدار عائلات المقاييس التالية:| Métrique | Descriptif |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | Prise de contact avec `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | Utilisez l'accélérateur pour `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Le temps de recharge est limité aux poignées de main. |
| `soranet_privacy_verified_bytes_total` | bande passante محققة من اثباتات قياس معماة. |
| `soranet_privacy_active_circuits_{avg,max}` | Il s'agit d'un seau. |
| `soranet_privacy_rtt_millis{percentile}` | Prises en charge pour RTT (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | عدادات Rapport d'action sur la gouvernance المجزأة حسب digest الفئة. |
| `soranet_privacy_bucket_suppressed` | buckets المحجوبة لان عتبة المساهمين لم تتحقق. |
| `soranet_privacy_pending_collectors{mode}` | مجمعات collecteur d'actions المعلقة قبل الدمج، مجمعة حسب وضع relais. |
| `soranet_privacy_suppression_total{reason}` | Les buckets ont été supprimés selon `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}`. |
| `soranet_privacy_snapshot_suppression_ratio` | نسبة supprimé/المصفاة لآخر drain (0-1)، مفيدة لميزانيات التنبيه. |
| `soranet_privacy_last_poll_unixtime` | Sous UNIX, il s'agit d'un collecteur inactif. |
| `soranet_privacy_collector_enabled` | jauge `0` pour collecteur désactivé et collecteur désactivé. |
| `soranet_privacy_poll_errors_total{provider}` | La fonction de relais alias est compatible avec le relais alias (il s'agit d'un relais HTTP et d'une connexion Internet). متوقعة). |

Les seaux sont également destinés aux seaux et aux seaux.

## ارشادات التشغيل

1. **لوحات المعلومات** - ارسم المقاييس اعلاه مجمعة حسب `mode` et `window_start`. ابرز النوافذ المفقودة لاظهار مشاكل collecteur et relais. L'`soranet_privacy_suppression_total{reason}` est un outil de suppression des collecteurs pour les collecteurs. يشحن اصل Grafana الآن لوحة **"Suppression Reasons (5m)"** مخصصة تغذيها تلك العدادات، بالاضافة الى stat **"Suppressed Bucket %"** الذي يحسب `sum(soranet_privacy_bucket_suppressed) / count(...)` لكل اختيار حتى يتمكن المشغلون من رصد تجاوزات الميزانية بسرعة. سلسلة **"Collector Share Backlog"** (`soranet_privacy_pending_collectors`) et stat **"Snapshot Suppression Ratio"** pour les collecteurs et les collecteurs.
2. **التنبيه** - قم بقيادة الانذارات من عدادات آمنة للخصوصية: ارتفاعات رفض PoW, تواتر cooldown, انجراف RTT, ورفض السعة. بما ان العدادات احادية الاتجاه داخل كل bucket, تعمل قواعد المعدلات البسيطة بشكل جيد.
3. **استجابة الحوادث** - اعتمد على البيانات المجمعة اولا. عند الحاجة لتصحيح اعمق، اطلب من relays اعادة تشغيل لقطات buckets او فحص اثباتات القياس المعماة بدلا من جمع سجلات حركة خام.
4. **احتفاظ** - اسحب البيانات بما يكفي لتجنب تجاوز `max_completed_buckets`. ينبغي على exportateurs اعتبار مخرجات Prometheus المصدر القياسي وحذف buckets المحلية بعد تمريرها.

## تحليلات suppression والتشغيل الآلي

يعتمد قبول SNNet-8 على اثبات ان collectors الآليين يبقون بحالة جيدة وان suppression تبقى ضمن حدود السياسة (≤10% for buckets لكل relay عبر اي نافذة 30 دقيقة). الادوات اللازمة لتلبية هذه البوابة تشحن الآن مع الشجرة؛ يجب على المشغلين ربطها بطقوسهم الاسبوعية. Suppression des suppressions dans Grafana Mise à jour PromQL pour la suppression des données الى استعلامات يدوية.

### Suppression de PromQLيجب على المشغلين ابقاء مساعدات PromQL التالية قريبة؛ كلاهما مذكور في لوحة Grafana المشتركة (`dashboards/grafana/soranet_privacy_metrics.json`) et Alertmanager :

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

استخدم مخرجات النسبة للتاكد من ان stat **"Suppressed Bucket %"** يبقى تحت ميزانية السياسة؛ اربط كاشف الارتفاعات by Alertmanager للحصول على اشعار سريع عند انخفاض عدد المساهمين بشكل غير متوقع.

### اداة تقرير buckets خارجية

L'espace de travail est utilisé pour `cargo xtask soranet-privacy-report` pour NDJSON. وجهه الى واحد او اكثر من exports admin for relay:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

La suppression de la sortie standard est effectuée avec `SoranetSecureAggregator` et la suppression de la sortie standard est effectuée en utilisant JSON avec `--json-out <path|->`. يحترم نفس مفاتيح collector المباشر (`--bucket-secs`, `--min-contributors`, `--expected-shares`, الخ)، مما يسمح للمشغلين باعادة تشغيل التقاطات التاريخية بعتبات مختلفة عند فرز حادثة. JSON est également compatible avec la suppression Grafana pour la suppression de SNNet-8.

### قائمة تدقيق التشغيل الآلي الاول

لا تزال الحوكمة تتطلب اثبات ان اول تشغيل آلي لبى ميزانية suppression. Mise à jour `--max-suppression-ratio <0-1>` pour la suppression des compartiments CI et mise à jour de la suppression des seaux (افتراضي 10%) et او عندما لا توجد seaux بعد. التدفق الموصى به:

1. Utilisez NDJSON comme administrateur pour le relais pour créer un `/v2/soranet/privacy/event|share` pour un orchestrateur `artifacts/sorafs_privacy/<relay>.ndjson`.
2. شغل المساعد مع ميزانية السياسة:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   يطبع الامر النسبة المرصودة ويخرج غير صفري عند تجاوز الميزانية **او** عندما لا تكون buckets جاهزة بعد، مشيرا الى ان télémétrie لم تُنتج بعد لهذا التشغيل. يجب ان تُظهر المقاييس المباشرة ان `soranet_privacy_pending_collectors` تتجه للصفر وان `soranet_privacy_snapshot_suppression_ratio` يبقى تحت نفس الميزانية اثناء التشغيل.
3. Utilisez JSON et CLI avec SNNet-8 pour créer un lien vers la version ultérieure. تشغيل نفس القطع.

## الخطوات التالية (SNNet-8a)

- Les collecteurs Prio sont des partages pour le runtime, des relais et des collecteurs sont `SoranetPrivacyBucketMetricsV1`. *(تم — راجع `ingest_privacy_payload` في `crates/sorafs_orchestrator/src/lib.rs` والاختبارات المصاحبة.)*
- نشر لوحة Prometheus المشتركة وقواعد اخفاء الهوية. *(تم — راجع `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` et `dashboards/alerts/soranet_policy_rules.yml`.)*
- انتاج مواد معايرة الخصوصية التفاضلية الموضحة في `privacy_metrics_dp.md` بما في ذلك دفاتر قابلة لاعادة الانتاج وملخصات الحوكمة. *(تم — تم توليد الدفتر والمواد بواسطة `scripts/telemetry/run_privacy_dp.py`; يقوم غلاف CI `scripts/telemetry/run_privacy_dp_notebook.sh` بتشغيل الدفتر عبر مسار العمل `.github/workflows/release-pipeline.yml` ; تم حفظ ملخص الحوكمة في `docs/source/status/soranet_privacy_dp_digest.md`.)*

يقدم الاصدار الحالي اساس SNNet-8: telemetry حتمية وآمنة للخصوصية تتصل مباشرة by scrapers et tableaux de bord Prometheus الحالية. مواد معايرة الخصوصية التفاضلية في مكانها، ومسار عمل release يحافظ على مخرجات الدفتر حديثة، والعمل المتبقي يركز على مراقبة اول تشغيل آلي وتوسيع تحليلات تنبيهات suppression.