---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/privacy-metrics-pipeline.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خط أنابيب مقاييس الخصوصية
العنوان: SoraNet شبكة الإنترنت (SNNet-8)
Sidebar_label: اللعبة الشهيرة عبر الإنترنت
الوصف: SoraNet عبارة عن مرحلات ومنسقين لـ پرائیویسی محفوظ القياس عن بعد جمع کرنا۔
---

:::ملاحظة مستند ماخذ
`docs/source/soranet/privacy_metrics_pipeline.md` هذا هو العنوان. ختم المستندات بختم دونوں نقول في الوقت المحدد.
:::

# SoraNet شبكة الإنترنت الرائدة

SNNet-8 Relay runtime هو أحدث التقنيات في مجال القياس عن بعد. تتابع المصافحة وحقيقة الدائرة التي تحتوي على عدد كبير جدًا من الدلاء المجمعة وعدادات Prometheus التي تم تطويرها من قبل الأفراد ودوائر غير قابلة للربط وقابلة للربط عمل بصيرت متعدد الاستخدامات.

## المجمع کا خلاصہ- وقت التشغيل الذي تم تنفيذه `tools/soranet-relay/src/privacy.rs` هو `PrivacyAggregator` متاح الآن.
- تحتوي دلاء ساعة الحائط على مفتاح جاتا (`bucket_secs`، الافتراضي 60 ثانية) وحلقة محدودة (`max_completed_buckets`، الافتراضي 120). أسهم المجمع تشمل نطاق الأعمال المتراكمة (`max_share_lag_buckets`، الافتراضي 12) كما لا يتم تشغيل دلاء Windows Prio المكبوتة أثناء التدفق وتسرب الذاكرة أو المجمعات العالقة.
- `RelayConfig::privacy` للخريطة `PrivacyConfig` ومقابض الضبط (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`، `max_share_lag_buckets`، `expected_shares`) ظہر کرتا ہے. الإعدادات الافتراضية لوقت تشغيل الإنتاج عتبات التجميع الآمنة SNNet-8a عتبات التجميع الآمنة
- مساعدات ماڈيولز المكتوبة في وقت التشغيل والتي تتضمن تسجيل الأحداث: `record_circuit_accepted`، `record_circuit_rejected`، `record_throttle`، `record_throttle_cooldown`، `record_capacity_reject`، `record_active_sample`، `record_verified_bytes`، و`record_gar_category`.

## نقطة نهاية مسؤول الترحيليقوم المستمعون `GET /privacy/events` بترحيل المستمع الإداري لاستطلاع الملاحظات الأولية. هناك نقطة نهاية محددة بسطر جديد JSON (`application/x-ndjson`) وابس كرتا وهي تحتوي على حمولات `SoranetPrivacyEventV1` وهي اندرون `PrivacyEventBuffer` مرآة. المخزن المؤقت هو أحدث الأحداث في إدخالات `privacy.event_buffer_capacity` (افتراضي 4096) وقراءة ما بعد التصريف، وهو عبارة عن كاشطات تحتوي على فجوات كافية لاستطلاع الرأي. الأحداث والمصافحة، والخانق، وعرض النطاق الترددي الذي تم التحقق منه، والدائرة النشطة، وإشارات GAR من خلال عدادات Prometheus وعدادات الطاقة المتدفقة، ومجمعات المصب، ومحفوظات فتات الخبز، ومحفوظات التخزين أو التجميع الآمن سير العمل وتغذية.

## تكوين التتابع

تكوين تتابع الآبر يبرز `privacy` دائرة إيقاع القياس عن بعد السابقة:

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

تتوافق الإعدادات الافتراضية للحقول مع مواصفات SNNet-8 ويتم تحميلها عند التحقق من صحتها:| المجال | الوصف | الافتراضي |
|-------|------------|---------|
| `bucket_secs` | نافذة التجميع هي ثواني). | `60` |
| `min_handshakes` | مع انتهاء عدد المساهمين، تنبعث عدادات الجرافة من عدادات الجرافة. | `12` |
| `flush_delay_buckets` | قم بمسح الدلاء بالكامل بعدد كبير. | `1` |
| `force_flush_buckets` | ينبعث الدلو المكبوت من إشعاعات كبيرة. | `6` |
| `max_completed_buckets` | محفوظات شدہ دلو تراكم (ذاكرة غير محدودة سے بچاتا ہے)۔ | `120` |
| `max_share_lag_buckets` | قمع أسهم المجمع هو نافذة الاحتفاظ. | `12` |
| `expected_shares` | الجمع بين أسهم جامع کرنے پہلے درکار Prio. | `2` |
| `event_buffer_capacity` | تيار المشرف کیلئے NDJSON تراكم الأحداث ۔ | `4096` |

`force_flush_buckets` و`flush_delay_buckets` هو عتبات الصفر أو حارس الاستبقاء الذي يمنع التحقق من الصحة فشل في التحقق من الصحة، كما أن عمليات النشر هذه تسبب تسربًا للقياس عن بعد لكل تتابع.

`event_buffer_capacity` أو `/admin/privacy/events` على حد سواء تم ربطه أيضًا بكرات، ولم يتم استخدام أي كاشطات أخرى بعد ذلك.

## أسهم جامع بريوتحدد المجمعات المزدوجة SNNet-8a البيانات وتنبعث من دلاء Prio المشتركة سرًا. المنسق أب `/privacy/events` تيار NDJSON مع إدخالات `SoranetPrivacyEventV1` ومشاركات `SoranetPrivacyPrioShareV1` دون تحليل كرتا و `SoranetSecureAggregator::ingest_prio_share` إلى الأمام كرتا. تنبعث الدلاء من مساهمات `PrivacyBucketConfig::expected_shares`، وهي تتابع سلوكياتها. يتم التحقق من صحة المشاركات ومحاذاة الجرافة وشكل الرسم البياني من خلال التحقق من صحة البيانات، حيث يتم دمج `SoranetPrivacyBucketMetricsV1` في هذه البيانات. إذا كان عدد المصافحة المجمعة `min_contributors` هو ما يجب أن تفعله دلو `suppressed` للتصدير، فهذا هو سلوك مجمع التتابع. النوافذ المحظورة ينبعث منها تسمية `suppression_reason` وبطاقات `insufficient_contributors` و`collector_suppressed` و`collector_window_elapsed` و`forced_flush_window_elapsed` تعتبر فجوات القياس عن بعد من أهم الاختلافات. لقد تم إطلاق النار على `collector_window_elapsed` وأسهمت في أسهم Prio `max_share_lag_buckets` مما أدى إلى ظهور عدد كبير من جامعي التحف العالقين وذاكرة المراكم التي لا معنى لها.

## Torii نقاط نهاية الابتلاع

Torii بعد اثنين من نقاط نهاية HTTP ذات بوابات القياس عن بعد، قم بتبديل المرحلات وجامعي وسائل النقل المخصصة لإرسال الملاحظات إلى الأمام:- `POST /v1/soranet/privacy/event` ایک `RecordSoranetPrivacyEventDto` الحمولة قبول کرتا ہے۔ يحتوي الجسم على `SoranetPrivacyEventV1` وهو عبارة عن ملصق اختياري `source` يشتمل على أوتا. طلب Torii لتنشيط ملف تعريف القياس عن بعد بما يتوافق مع التحقق من الصحة وتسجيل الحدث وHTTP `202 Accepted` الذي يستمر Norito JSON مغلف WAPS نافذة الجرافة المحسوبة (`bucket_start_unix`، `bucket_duration_secs`) ووضع الترحيل ہوتا ہے۔
- `POST /v1/soranet/privacy/share` ایک `RecordSoranetPrivacyShareDto` الحمولة قبول کرتا ہے۔ تلميح الجسم `SoranetPrivacyPrioShareV1` وتلميح `forwarded_by` الاختياري هو بمثابة تدقيق لتدفقات المجمع. عمليات الإرسال HTTP `202 Accepted` والتي تم إنشاؤها بواسطة Norito بطاقة مغلف JSON عبارة عن أداة تجميع جو ونافذة دلو وتلميح منع لتلخيص الكرتا؛ فشل التحقق من الصحة القياس عن بعد استجابة `Conversion` خريطة مجموعة أدوات التجميع معالجة الأخطاء الحتمية معالجة الأخطاء. يقوم المُنسق بحلقة حدث تتابع استقصاء التتابع من خلال مزامنة مزامنة مستمرة لـ 18NT00000017X.

قم بتنزيل ملف تعريف القياس عن بعد لنقاط النهاية: تم تعطيل المقاييس مرة أخرى في `503 Service Unavailable`. العملاء Norito ثنائي (I18NI000000102X) أو Norito JSON (`application/x.norito+json`) الهيئات الأخرى؛ معيار الخادم Torii extractors تنسيق ذریعے الخاص بك التفاوض بشأن کرتا ہے۔

## مقاييس Prometheusتم تحديد الجرافة المصدرة `mode` (`entry`، `middle`، `exit`) والتسميات `bucket_start`. تنبعث من عائلات الدرجة المترية بوابات:| متري | الوصف |
|--------|------------|
| `soranet_privacy_circuit_events_total{kind}` | تصنيف المصافحة يشمل `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | عدادات الخانق هي `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | المصافحات المخنوقة هي بمثابة تهدئة مدتوں في المجموع. |
| `soranet_privacy_verified_bytes_total` | براهين القياس العمياء سے عرض النطاق الترددي المعتمد۔ |
| `soranet_privacy_active_circuits_{avg,max}` | هناك عدد كبير جدًا من الدوائر النشطة. |
| `soranet_privacy_rtt_millis{percentile}` | تقديرات النسب المئوية لـ RTT (`p50`، `p90`، `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | عدادات تقرير إجراءات الحوكمة المجزأة ملخص فئة جو مفتاح سےوتے ہیں۔ |
| `soranet_privacy_bucket_suppressed` | لم تعد عتبة عتبة المساهم والدلاء كافية. |
| `soranet_privacy_pending_collectors{mode}` | مجمعات أسهم المجمع تجمع بين الانتظار لفترة طويلة، ووضع التتابع في حساب المجموعة. |
| `soranet_privacy_suppression_total{reason}` | تتضمن عدادات الجرافة المكبوتة `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` معالجة فجوات الخصوصية في لوحات المعلومات بشكل متناسب. |
| `soranet_privacy_snapshot_suppression_ratio` | أخيرًا نسبة الاستنزاف/الاستنزاف (0-1)، ميزانيات التنبيه مفید۔ |
| `soranet_privacy_last_poll_unixtime` | آخر استطلاع للرأي هو الطابع الزمني لـ UNIX (تنبيه الخمول للمجمع عند انتهاء الصلاحية). |
| `soranet_privacy_collector_enabled` | قم بقياس الوقت الحالي `0` وابدأ عند تجميع أداة تجميع الخصوصية أو بدء تشغيلها مرة أخرى (تنبيه تعطيل أداة التجميع). || `soranet_privacy_poll_errors_total{provider}` | فشل الاستقصاء ترحيل الاسم المستعار کے حساب سے گروپ ہوتے ہیں (أخطاء فك التشفير، فشل HTTP، أو توقعات رموز الحالة غير المتوقعة). |

تحتوي الدلاء على ملاحظات ومقاطع خام، كما تحتوي على لوحات معلومات ونوافذ خالية من الأخطاء.

## التوجيه التشغيلي1. **لوحات المعلومات** - المقاييس الأولية `mode` و`window_start` هي حساب مجموعة اللوحة. النوافذ المفقودة تسلط الضوء على مشكلات جامع الطاقة أو التتابع. `soranet_privacy_suppression_total{reason}` يستخدم فجوات معالجة الفرز لنقص المساهم والقمع الذي يحركه المجمع مما يؤدي إلى فرق كبير. Grafana أصل اب ایك مخصص **"أسباب المنع (5 م)"** لوحة فراہم كرتا ہے جو ان عدادات سے چلتا ہے، ساتھ ہی ایك **"Suppressed Bucket %"** stat جو `sum(soranet_privacy_bucket_suppressed) / count(...)` في تحديد حساب كرتا ہے تاك خرق ميزانية آبيرز هو أمر بالغ الأهمية. سلسلة **"Collector Share Backlog"** (`soranet_privacy_pending_collectors`) و **"Snapshot Ruppression Ratio"** جامعي الإحصائيات المتوقفين وعمليات التشغيل الآلية أثناء انحراف الميزانية.
2. **التنبيه** - عدادات آمنة للخصوصية وأجهزة إنذار: رفض إثبات العمل للارتفاعات، وتردد التبريد، وانجراف RTT، ورفض السعة. هناك دلو من العدادات الرتيبة، وهو عبارة عن قواعد بسيطة تعتمد على المعدل.
3. **الاستجابة للحوادث** - جمع البيانات المجمعة للحصار. عند الحاجة إلى تصحيح الأخطاء، يجب عليك ترحيل مجموعة من اللقطات لإعادة تشغيل البراهين أو أدلة القياس المعماة، ولا يتم جمع سجلات حركة المرور الأولية.
4. **الاحتفاظ** - `max_completed_buckets` تجاوز الحد الأقصى لحجم الكافية. يقوم المصدرون بإخراج Prometheus من المصدر المتعارف عليه ويتم إعادة توجيهه بعد حذف الدلاء المحلية.## تحليلات القمع والتشغيل الآلي

تم قبول SNNet-8 بشكل حصري من قبل جامعي الآليين الصحيين والقمع بحدود اندر رہے (تتابع كل 30 دقيقة من النافذة ≥10٪ دلاء). بوابة هذه هي عبارة عن أدوات دركار من خلال الريبو؛ قد تشمل طقوس الحرب التي يقوم بها الآباء منذ فترة طويلة. لوحات القمع Grafana الجديدة تحتوي على مقتطفات جديدة من PromQL تمنع حجب البطاقة، وتوفر خدمة On-call رؤية حية متعددة بالإضافة إلى الاستعلامات اليدوية الضرورية پڑے۔

### مراجعة القمع کیلئے وصفات PromQL

يتم إنشاء درج من مساعدي PromQL؛ تمت الإشارة إلى لوحة معلومات Grafana المشتركة (`dashboards/grafana/soranet_privacy_metrics.json`) وقواعد Alertmanager:

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

تم استخدام نسبة الإخراج **"Suppressed Bucket %"** إحصائيات الميزانية الحالية؛ يقوم كاشف الارتفاع وAlertmanager بتحريك العدادات غير المتوقعة بشكل مفاجئ في الأشهر المقبلة من العام المقبل.

### تقرير واجهة سطر الأوامر (CLI) للدلو غير المتصل بالإنترنت

مساحة العمل `cargo xtask soranet-privacy-report` تلتقط NDJSON كل الأجهزة. هناك واحد أو أكثر من صادرات مسؤول الترحيل لكل نقطة:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```إنه مساعد في التقاط `SoranetSecureAggregator` لتدفق تدفق البيانات، وملخص القمع القياسي لملفات `SoranetSecureAggregator`، واختتام تقرير JSON المنظم لتقرير JSON المنسق لـ `SoranetSecureAggregator`. جامع مباشر ومقابض (`--bucket-secs`, `--min-contributors`, `--expected-shares`, إلخ) التي تكرم كارتا، وتصدر مشكلات الفرز عتبات مختلفة حتى يتم إعادة تشغيل اللقطات التاريخية سكتے ہيں. بوابة تحليلات قمع SNNet-8 تدقيق کے قابل رکھنے کیلئے JSON کو Grafana لقطات شاشة کے ساتھ إرفاق کریں.

### قائمة مرجعية للتشغيل الآلي

الحوكمة هي أيضًا أداة جديدة تكمل ميزانية قمع تشغيل الأتمتة. helper ab `--max-suppression-ratio <0-1>` يقبل كرتا و CI أو بريتس هذا الوقت يفشل بسرعة، كما لم يتم السماح للجرافات المكبوتة بالنافذة (افتراضي 10%) أو حتى الدلاء الموجودة غير موجودة. تجوّل التدفق:

1. قم بترحيل نقطة (نقاط) نهاية المشرف والمنسق للدفق `/v1/soranet/privacy/event|share` وتصدير NDJSON إلى `artifacts/sorafs_privacy/<relay>.ndjson`.
2. سياسة الموازنة هي شيء مساعد:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```التحكم في النسبة المرصودة للكرتون والخروج غير الصفري عندما تتجاوز الميزانية 4 ** أو** في الجرافات التي لا تزال قيد الاستخدام، ولا يتم تشغيل القياس عن بعد على الإطلاق ہوئی۔ المقاييس الحية تشير إلى `soranet_privacy_pending_collectors` صفر استنزاف رأس المال و`soranet_privacy_snapshot_suppression_ratio` الميزانية لا تحتاج إلى تشغيلها.
3. النقل الافتراضي يدعم إخراج JSON وسجل CLI لحزمة أدلة SNNet-8 والأرشفة الثابتة لكل المراجعين والعناصر الإضافية.

## الخطوات التالية (SNNet-8a)

- جامعات Prio المزدوجة التي تدمج كريات ومشاركة الابتلاع في وقت التشغيل مع مرحلات وجامعات متسقة `SoranetPrivacyBucketMetricsV1` تنبعث منها كريات. *(تم — `crates/sorafs_orchestrator/src/lib.rs` في `ingest_privacy_payload` واختبارات الموضوعات دیکھیں.)*
- لوحة معلومات Prometheus مشتركة JSON وقواعد التنبيه شائعة کریں جو قمع الفجوات، صحة المجمع، وعدم الكشف عن هويته کو تغطية کریں. *(تم — `dashboards/grafana/soranet_privacy_metrics.json`، `dashboards/alerts/soranet_privacy_rules.yml`، `dashboards/alerts/soranet_policy_rules.yml` وتركيبات التحقق من الصحة.)*
- `privacy_metrics_dp.md` يحتوي على أدوات معايرة الخصوصية التفاضلية التي تم إنشاؤها، وهي تحتوي على دفاتر ملاحظات قابلة للتكرار وملخصات للحوكمة شاملة. *(تم - دفتر الملاحظات والمصنوعات `scripts/telemetry/run_privacy_dp.py` بنتے ہیں؛ غلاف CI `scripts/telemetry/run_privacy_dp_notebook.sh` دفتر الملاحظات `.github/workflows/release-pipeline.yml` سير العمل الذي تم تنفيذه؛ ملخص الإدارة `docs/source/status/soranet_privacy_dp_digest.md` هذا هو..)*تم إصدار الإصدار SNNet-8 الموجود وهو عبارة عن أداة قياس عن بعد حتمية وآمنة للخصوصية لكاشطات ولوحات المعلومات Prometheus الموجودة. تتوفر أدوات معايرة الخصوصية التفاضلية، وتطلق مخرجات دفتر ملاحظات سير عمل خط الأنابيب بشكل سريع، بالإضافة إلى التشغيل الآلي الكامل لتحليلات تنبيه المراقبة والقمع التي تزيد من مركزها.