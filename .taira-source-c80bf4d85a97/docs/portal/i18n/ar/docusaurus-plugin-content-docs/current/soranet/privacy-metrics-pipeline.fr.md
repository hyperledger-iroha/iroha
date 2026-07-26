---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/privacy-metrics-pipeline.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خط أنابيب مقاييس الخصوصية
العنوان: خط أنابيب قياس السرية SoraNet (SNNet-8)
Sidebar_label: خط أنابيب مقاييس السرية
الوصف: تحافظ مجموعة القياس عن بعد على سرية المرحلات والمنسقين SoraNet.
---

:::ملاحظة المصدر الكنسي
ريفليت `docs/source/soranet/privacy_metrics_pipeline.md`. قم بمزامنة النسختين حتى يتم سحب مجموعة الوثائق القديمة.
:::

# خط أنابيب قياس السرية SoraNet

يقدم SNNet-8 سطحًا للقياس عن بعد يتميز بالسرية من أجل وقت تشغيل التتابع. يزيد التتابع من أحداث المصافحة والدائرة في دلاء من دقيقة واحدة ولا يُصدّر ما يزيد حجم أجهزة الكمبيوتر Prometheus، مع مراعاة الدوائر الفردية غير القابلة للربط على الإطلاق من خلال توفير رؤية قابلة للاستغلال من قبل المشغلين.

##فتحة المزرعة- وقت تشغيل التنفيذ في `tools/soranet-relay/src/privacy.rs` sous `PrivacyAggregator`.
- الدلاء مُفهرسة حسب دقيقة الساعة (`bucket_secs`، افتراضيًا 60 ثانية) ويتم تخزينها في سنة ميلادية (`max_completed_buckets`، افتراضيًا 120). تحافظ مشاركات المجمعات على الأعمال المتراكمة الخاصة بها (`max_share_lag_buckets`، افتراضيًا 12) بحيث يتم إغلاق النوافذ ذات الجودة العالية في مجموعات محذوفة حتى يتم إضافة الذاكرة أو إخفاء المجموعات المجمعة.
- `RelayConfig::privacy` يتم تعيينه مباشرة على `PrivacyConfig`، ويعرض الإعدادات (`bucket_secs`، `min_handshakes`، `flush_delay_buckets`، `force_flush_buckets`، `max_completed_buckets`، `max_share_lag_buckets`، `expected_shares`). يحافظ وقت تشغيل الإنتاج على القيم الافتراضية بينما يقدم SNNet-8a ضمانات إضافية آمنة.
- تقوم وحدات وقت التشغيل بتسجيل الأحداث عبر أنواع المساعدة التالية: `record_circuit_accepted`، `record_circuit_rejected`، `record_throttle`، `record_throttle_cooldown`، `record_capacity_reject`، `record_active_sample`، `record_verified_bytes` و`record_gar_category`.

## مسؤول نقطة النهاية du Relayيمكن للمشغلين استجواب مسؤول التتابع من أجل الملاحظات المباشرة عبر `GET /privacy/events`. تم تجديد نقطة النهاية لـ JSON المحددة بواسطة خطوط جديدة (`application/x-ndjson`) تحتوي على الحمولات `SoranetPrivacyEventV1` المعادة من `PrivacyEventBuffer` الداخلي. يحفظ المخزن المؤقت الأحداث الأكثر حداثة حتى `privacy.event_buffer_capacity` المدخلة (افتراضيًا 4096) ويتم عرضه على الشاشة، دون أن تكون الكاشطات ضرورية بما يكفي لتجنب المتاعب. تشمل الأحداث نفس إشارات المصافحة والخانق وعرض النطاق الترددي الذي تم التحقق منه والدائرة النشطة وGAR التي تغذي حاسبات Prometheus، وتسمح بجامعي البيانات من خلال أرشفة فتات الخبز الآمنة من أجل السرية أو توفير مسارات العمل المجمعة آمن.

## تكوين دو ريلاي

يقوم المشغلون بضبط إيقاع السرية عن بعد في ملف تكوين الترحيل عبر القسم `privacy`:

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

تتوافق القيم الافتراضية للأبطال مع مواصفات SNNet-8 ويتم التحقق منها بالشحن:| بطل | الوصف | افتراضي |
|-------|------------|--------|
| `bucket_secs` | كبر حجم نافذة التجميع (ثواني). | `60` |
| `min_handshakes` | عدد الحد الأدنى للمساهمات قبل أن تتمكن من زيادة عدد الحسابات. | `12` |
| `flush_delay_buckets` | تم إكمال عدد من الدلاء قبل إجراء عملية التدفق. | `1` |
| `force_flush_buckets` | الحد الأقصى للعمر قبل أن يتم التخلص من دلو مملوء. | `6` |
| `max_completed_buckets` | تراكم الدلاء المحفوظة (évite une mémoire not Bornée). | `120` |
| `max_share_lag_buckets` | نافذة الاحتفاظ بالأسهم المجمعة قبل القمع. | `12` |
| `expected_shares` | تتطلب الأسهم Prio de Collecteur دمجًا متقدمًا. | `2` |
| `event_buffer_capacity` | Backlog d'événements NDJSON لتدفق المشرف. | `4096` |

حدد `force_flush_buckets` plus بدلاً من `flush_delay_buckets`، أو قم بإلغاء تنشيط بطاقة الحفظ مما يؤدي إلى تعطيل التحقق لتجنب عمليات النشر التي تؤدي إلى ترحيل التتابع عن بعد.

الحد `event_buffer_capacity` يحمل أيضًا `/admin/privacy/events`، مما يضمن أن الكاشطات لا يمكنها أن تسبب تأخيرًا غير محدد.

## أسهم جامعي بريوSNNet-8a ينشر المجمعات المزدوجة التي توفر دلاء من أجل مشاركة الأسرار. يقوم المُنسق بتحليل التدفق NDJSON `/privacy/events` للمدخلات `SoranetPrivacyEventV1` والأسهم `SoranetPrivacyPrioShareV1`، المرسل إلى `SoranetSecureAggregator::ingest_prio_share`. ستؤدي الجرافات إلى وصول مساهمات `PrivacyBucketConfig::expected_shares`، مما يعكس سلوك التتابع. تم التحقق من المشاركات من أجل محاذاة الدلاء وشكل الرسم البياني قبل دمجها في `SoranetPrivacyBucketMetricsV1`. إذا كان اسم المصافحة المجمعة مثل `min_contributors`، فسيتم تصدير الجرافة مثل `suppressed`، مما يعكس سلوك مرحل التتابع. تم تعطيل النوافذ المحذوفة بملصق `suppression_reason` حتى يتمكن المشغلون من تمييز `insufficient_contributors` و`collector_suppressed` و`collector_window_elapsed` و`forced_flush_window_elapsed` أثناء تشخيص المشاكل القياس عن بعد. السبب `collector_window_elapsed` يتم تقليصه عندما يتم تدريب الأسهم الأولية على `max_share_lag_buckets`، مما يجعل المجمعات المرئية مرئية بدون ترك المراكم المؤقتة في الذاكرة.

## نقاط النهاية للابتلاع Torii

يكشف Torii عن خلل في نقطتي نهاية HTTP المحميتين من خلال وحدة القياس عن بعد حتى تتمكن المرحلات والمجمعات من إرسال الملاحظات دون منع النقل بالقياس:- `POST /v1/soranet/privacy/event` يقبل الحمولة `RecordSoranetPrivacyEventDto`. يغلف الجسم `SoranetPrivacyEventV1` بالإضافة إلى علامة `source` الاختيارية. Torii التحقق من صحة الطلب ضد ملف تعريف الاتصال عن بعد النشط، وتسجيل الحدث والرد باستخدام HTTP `202 Accepted` المصاحب لمغلف Norito JSON المحتوي على النافذة الحسابية (`bucket_start_unix`, `bucket_duration_secs`) ووضع التتابع.
- `POST /v1/soranet/privacy/share` يقبل الحمولة `RecordSoranetPrivacyShareDto`. يقوم فريق النقل بخيار `SoranetPrivacyPrioShareV1` ومؤشر `forwarded_by` حتى يتمكن المشغلون من مراقبة تدفق المجمعين. تتطلب عمليات الإرسال HTTP `202 Accepted` مع مغلف Norito JSON إعادة تشغيل المجمع ونافذة الجرافة ومؤشر القمع؛ تتوافق نتائج التحقق من الصحة مع استجابة القياس عن بعد `Conversion` لتتمكن من الحفاظ على خطأ محدد بين المجمعين. يتم اختلال سلسلة أحداث المُنسق من خلال هذه المشاركات عند استجواب المرحلات، مما يضمن مزامنة المجمع Prio de Torii مع دلاء التتابع.النقطتان الطرفيتان تتعلقان بملف القياس عن بعد: `503 Service Unavailable` عندما تكون المقاييس معطلة. يمكن للعملاء إرسال مجموعة Norito ثنائية (`application/x.norito`) أو Norito JSON (`application/x.norito+json`) ؛ يقوم الخادم بالتنسيق تلقائيًا عبر مستخرجات Torii القياسية.

## المقاييس Prometheus

تم تصدير دلو Chaque إلى الملصقات `mode` (`entry`، `middle`، `exit`) و`bucket_start`. تنبعث أسر المقاييس التالية:| متري | الوصف |
|--------|------------|
| `soranet_privacy_circuit_events_total{kind}` | تصنيف المصافحات مع `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | عدادات الخانق مع `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | يتم تحديد فترات التهدئة حسب المصافحة الخانقة. |
| `soranet_privacy_verified_bytes_total` | Bande passante vérfiée issues de preuves de mesure aveugles. |
| `soranet_privacy_active_circuits_{avg,max}` | Moyenne et pic de Circuits actifs par Bucket. |
| `soranet_privacy_rtt_millis{percentile}` | تقديرات النسب المئوية RTT (`p50`، `p90`، `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | يحتوي تقرير محاسبي إجراءات الحوكمة على فهرسة ضمن ملخص الفئة. |
| `soranet_privacy_bucket_suppressed` | تحتفظ الدلاء لأن بقية المساهمين لم تصل بعد. |
| `soranet_privacy_pending_collectors{mode}` | مجمعات الأسهم المجمعة في انتظار التجميع، مجمعة حسب وضع التتابع. |
| `soranet_privacy_suppression_total{reason}` | تساعد عدادات البيانات الإضافية مع `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` في أن لوحات المعلومات قد تزيد من السرية. |
| `soranet_privacy_snapshot_suppression_ratio` | نسبة الحذف/فيديو التصريف الأخير (0-1)، مفيدة للميزانيات التنبيهية. |
| `soranet_privacy_last_poll_unixtime` | Horodatage UNIX du dernier poll réussi (يوفر تنبيه المجمع الخامل). |
| `soranet_privacy_collector_enabled` | المقياس الذي ينتقل إلى `0` عندما يتم تعطيل مجمع السرية أو عدم البدء (يتم تعطيل مجمع التنبيهات). || `soranet_privacy_poll_errors_total{provider}` | فحوصات الاستقصاء الجماعية بالاسم المستعار للترحيل (زيادة أخطاء فك التشفير أو فحوصات HTTP أو رموز الحالة غير المقصودة). |

تبقى الدلاء بدون ملاحظات صامتة، مع الحفاظ على لوحات المعلومات الخاصة بها بدون تصنيع نوافذ مليئة بالصفر.

## الإرشاد التشغيلي1. **Dashboards** - تتبع المقاييس المجمعة حسب `mode` و`window_start`. قم بقياس النوافذ القديمة لإصلاح مشاكل التجميع أو التتابع. استخدم `soranet_privacy_suppression_total{reason}` لتمييز عدم كفاية مساهمات القمع التجريبية بين المجمعين أثناء فرز البضائع. يتضمن الأصل Grafana خللًا في اللوحة الأخيرة **"أسباب الإيقاف (5 م)"** التي يتم شحنها بواسطة هذه الحسابات، بالإضافة إلى إحصائيات **"الجرافة المثبطة٪"** التي تحسب `sum(soranet_privacy_bucket_suppressed) / count(...)` من خلال التحديد الذي يقوم المشغلون بإعادة تشغيله اختلالات الميزانية في انقلاب. السلسلة **"Collector Share Backlog"** (`soranet_privacy_pending_collectors`) والإحصائيات **"Snapshot Suppression Ratio"** تثبت المجمعات المحظورة واشتقاق الميزانية من خلال عمليات التنفيذ التلقائية.
2. **Alerting** - قم بتوجيه التنبيهات من خلال أجهزة الكمبيوتر الخاصة بالسرية: صور إعادة تنشيط PoW، وتكرار فترات التبريد، واشتقاق RTT، ورفض السعة. بينما تكون الحواسيب رتيبة في كل دلو داخلي، فإن القواعد البسيطة تعمل بشكل جيد.
3. **الاستجابة للحوادث** - اضغط على البيانات المعتمدة. عندما يكون التصحيح العميق ضروريًا، اطلب إعادة تشغيل لقطات الدلاء أو فحص قياسات القياس بدلاً من إعادة تسجيل يوميات حركة المرور العنيفة.4. **الاحتفاظ** - مكشطة كافية لمنع تجاوز `max_completed_buckets`. يجب على المصدرين إجراء عملية النقل Prometheus كمصدر Canonique وحذف الدلاء الموجودة في مكان واحد للإرسال.

## تحليل القمع والتنفيذ التلقائي

يعتمد قبول SNNet-8 على توضيح أن المجمعات الآلية تبقى بلا حدود وأن القمع يظل ضمن حدود السياسة (أقل من 10% من الدلاء على مدار نافذة لمدة 30 دقيقة). الأدوات اللازمة لتلبية هذه المتطلبات تظل متاحة دائمًا مع المستودع؛ يجب على المشغلين دمج طقوسهم اليومية. تعكس لوحات القمع الجديدة Grafana إضافات PromQL التي لا تشوبها شائبة، مما يوفر رؤية واضحة للمعدات مباشرة قبل طلب الطلبات اليدوية.

### Recettes PromQL لمجلة القمع

يجب على المشغلين أن يحافظوا على مساعدات PromQL التالية للبوابة الرئيسية؛ تم الرجوع إلى الثنائي في لوحة المعلومات Grafana المشاركة (`dashboards/grafana/soranet_privacy_metrics.json`) وقواعد مدير التنبيه:

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

استخدم النسبة لتأكيد أن الإحصائيات **"Suppressed Bucket %"** تقع تحت ميزانية السياسة ; قم بتمرير كاشف الصور إلى Alertmanager من أجل العودة السريعة عندما يكون عدد المساهمين غير المراقبين.### CLI de Rapport de Bucket hors ligne

تعرض مساحة العمل `cargo xtask soranet-privacy-report` لالتقاط لقطات NDJSON. Pointez-le sur un ou plusieurs Exports admin de Relay :

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

تقوم الأداة بتمرير الالتقاط باستخدام `SoranetSecureAggregator`، وطباعة سيرة ذاتية للقمع على stdout، واختياريًا، إنشاء علاقة JSON structuré عبر `--json-out <path|->`. فيما يتعلق بالضوابط التي تم ضبطها على المجمع المباشر (`--bucket-secs`، `--min-contributors`، `--expected-shares`، وما إلى ذلك)، تسمح لمشغلي تجديد اللقطات التاريخية من متابعة مختلفة أثناء فرز حادث ما. قم بتوصيل JSON مع اللقطات Grafana حتى يكون باب تحليل القمع SNNet-8 قابلاً للتدقيق.

### قائمة التحقق من التنفيذ التلقائي الأول

تتطلب الحوكمة دائمًا التأكد من أن التنفيذ الأول يتم تلقائيًا مع احترام ميزانية القمع. يقبل الجهاز الخلل `--max-suppression-ratio <0-1>` حتى يتمكن CI أو المشغلون من الاستجابة بسرعة عند إزالة الحاويات من النافذة المصرح بها (10% افتراضيًا) أو عند عدم ظهور الحاوية مرة أخرى. يوصى بالتدفق:

1. مُصدِّر NDJSON من إدارة نقاط النهاية للتتابع بالإضافة إلى التدفق `/v1/soranet/privacy/event|share` من المُنسق مقابل `artifacts/sorafs_privacy/<relay>.ndjson`.
2. تنفيذ الأداة بالميزانية السياسية :

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```Laأمر يطبع النسبة المرصودة وينتهي برمز غير فارغ عندما تكون الميزانية منخفضة **أو** عندما لا تكون جاهزة، مما يشير إلى أن القياس عن بعد لم يعد يتم إنتاجه للتنفيذ. يجب أن يتم ضبط المقاييس الحية `soranet_privacy_pending_collectors` على الصفر وتبقى `soranet_privacy_snapshot_suppression_ratio` على نفس الميزانية أثناء التنفيذ.
3. أرشفة عملية JSON ومجلة CLI باستخدام ملف Preuve SNNet-8 قبل نقلها بشكل افتراضي حتى يتمكن المراجعون من تجديد العناصر الدقيقة.

## Prochaines étapes (SNNet-8a)- قم بدمج المجمعات Prio doubles، من خلال توصيل إدخال المشاركات في وقت التشغيل حتى تتمكن المرحلات والمجمعات من تحميل الحمولات `SoranetPrivacyBucketMetricsV1` بشكل متماسك. *(فعل — انظر `ingest_privacy_payload` في `crates/sorafs_orchestrator/src/lib.rs` والاختبارات المرتبطة.)*
- قم بنشر لوحة المعلومات Prometheus المشاركة وتغطي قواعد التنبيه جهود القمع وصحة المجمعين وقواعد عدم الكشف عن هويتهم. *(فعل — عرض `dashboards/grafana/soranet_privacy_metrics.json`، `dashboards/alerts/soranet_privacy_rules.yml`، `dashboards/alerts/soranet_policy_rules.yml` وتركيبات التحقق من الصحة.)*
- إنتاج منتجات معايرة السرية المختلفة الموضحة في `privacy_metrics_dp.md`، وتتضمن دفاتر ملاحظات قابلة لإعادة الإنتاج وخلاصات الإدارة. *(الأمر — دفتر ملاحظات + عناصر تم إنشاؤها وفقًا لـ `scripts/telemetry/run_privacy_dp.py`؛ غلاف CI `scripts/telemetry/run_privacy_dp_notebook.sh` ينفذ دفتر الملاحظات عبر سير العمل `.github/workflows/release-pipeline.yml`؛ ملخص الإدارة المودع في `docs/source/status/soranet_privacy_dp_digest.md`.)*

الإصدار الحالي من مؤسسة SNNet-8: جهاز قياس عن بعد محدد وآمن للسرية التي يتم توجيهها بشكل كامل إلى الكاشطات Prometheus ولوحات المعلومات. يتم وضع عناصر معايرة السرية التفاضلية في مكانها الصحيح، ويحافظ سير عمل خط الأنابيب على طلعات الكمبيوتر المحمول يوميًا، ويركز العمل المتبقي على مراقبة التنفيذ الأول تلقائيًا بالإضافة إلى تمديد تحليلات تنبيه القمع.