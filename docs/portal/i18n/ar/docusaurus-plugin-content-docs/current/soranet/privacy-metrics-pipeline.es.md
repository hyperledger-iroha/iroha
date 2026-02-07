---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/privacy-metrics-pipeline.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خط أنابيب مقاييس الخصوصية
العنوان: خط أنابيب قياسات الخصوصية في SoraNet (SNNet-8)
Sidebar_label: مسار قياسات الخصوصية
الوصف: القياس عن بعد مع الحفاظ على خصوصية المرحلات ومنسقي SoraNet.
---

:::ملاحظة فوينتي كانونيكا
ريفليجا `docs/source/soranet/privacy_metrics_pipeline.md`. قم بحفظ النسخ المتزامنة حتى يتم سحب المستندات المتوارثة.
:::

# خط أنابيب قياسات الخصوصية في SoraNet

تقدم SNNet-8 سطحًا للقياس عن بعد مع مراعاة الخصوصية
وقت تشغيل التتابع. الآن سيجمع التتابع أحداث المصافحة والدائرة أون
دلاء من دقيقة واحدة وتصدير قواطع فردية Prometheus كبيرة، مع الاستمرار
توفر الدوائر الفردية desvinculados mientras رؤية واضحة
للمشغلين.

## استئناف المجمع- تنفيذ وقت التشغيل المباشر في `tools/soranet-relay/src/privacy.rs`
  `PrivacyAggregator`.
- يتم فهرسة الدلاء خلال دقيقة من الساعة (`bucket_secs`، الافتراضي 60 ثانية) ذ
  يتم تخزينه في حلقة واحدة (`max_completed_buckets`، الافتراضي 120). أسهم لوس
  يقوم المجمعون بصيانة الأعمال المتراكمة الخاصة بهم (`max_share_lag_buckets`، الافتراضي 12)
  لكي تصبح النوافذ القديمة فارغة مثل الدلاء العلوية في مكانها
  قم بتصفية الذاكرة أو إخفاء جامعي البيانات.
- `RelayConfig::privacy` يوجه مباشرة إلى `PrivacyConfig`، يعرض مقابض الضبط
  (`bucket_secs`، `min_handshakes`، `flush_delay_buckets`، `force_flush_buckets`،
  `max_completed_buckets`، `max_share_lag_buckets`، `expected_shares`). وقت التشغيل
  يحافظ الإنتاج على الإعدادات الافتراضية بينما يقدم SNNet-8a عتبات
  agregasion segura.
- تقوم وحدات وقت التشغيل بتسجيل الأحداث مع أنواع مساعدة:
  `record_circuit_accepted`، `record_circuit_rejected`، `record_throttle`،
  `record_throttle_cooldown`، `record_capacity_reject`، `record_active_sample`،
  `record_verified_bytes`، و`record_gar_category`.

## مسؤول نقطة النهاية ديل ريلاييمكن للمشغلين استشارة المستمع المسؤول عن التتابع للملاحظات
Crudas عبر `GET /privacy/events`. يتم تقسيم نقطة النهاية إلى JSON المحددة بواسطة
خطوط جديدة (`application/x-ndjson`) مع حمولات `SoranetPrivacyEventV1`
تم مراجعة `PrivacyEventBuffer` الداخلي. يحتفظ المخزن المؤقت بالأحداث
mas nuevos hasta `privacy.event_buffer_capacity` intradas (افتراضي 4096) و se
فراغ في المحاضرة، لأن أدوات الكشط يجب أن تكون كافية ل
لا ديجار هويكو. الأحداث تحتوى على سماء المصافحة، الخانق،
عرض النطاق الترددي الذي تم التحقق منه والدائرة النشطة وGAR الذي يدعم أجهزة التحكم Prometheus،
السماح لهواة جمع البيانات بأرشفة فتات الخبز بأمان من أجل الخصوصية
o سير العمل الغذائي في التجميع الآمن.

## تكوين التتابع

يضبط المشغلون وتيرة قياس الخصوصية عن بعد في الملف
تكوين التتابع عبر القسم `privacy`:

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

تتزامن الإعدادات الافتراضية للنطاق مع مواصفات SNNet-8 وهي صالحة تمامًا
كارجار:| كامبو | الوصف | الافتراضي |
|-------|------------|---------|
| `bucket_secs` | قم بإغلاق كل نافذة من فتحات التجميع (ثانية). | `60` |
| `min_handshakes` | الحد الأدنى من المساهمين قبل إطلاق دلو. | `12` |
| `flush_delay_buckets` | تم إكمال الدلاء للانتظار قبل التدفق. | `1` |
| `force_flush_buckets` | أقصى حد قبل إطلاق دلو فوق الحد. | `6` |
| `max_completed_buckets` | تراكم الجرافات المحفوظة (Evita Memory ilimitada). | `120` |
| `max_share_lag_buckets` | نافذة احتجاز لأسهم المجمعات قبل الإزالة. | `12` |
| `expected_shares` | قيمة الأسهم مطلوبة قبل الجمع. | `2` |
| `event_buffer_capacity` | Backlog NDJSON de events para el adminstream. | `4096` |

تكوين `force_flush_buckets` من خلال `flush_delay_buckets`، المشغل
العتبات متجمدة أو غير قابلة للتفكيك بعد ذلك
التحقق من صحة لتجنب الفشل في تصفية القياس عن بعد للتتابع.

الحد `event_buffer_capacity` تامبيان أكوتا `/admin/privacy/events`,
تأكد من أن الكاشطات لن تتأخر إلى أجل غير مسمى.

## أسهم ديل جامع بريويقوم SNNet-8a بجمع الثنائيات التي تصدر دلاء من Prio مع المشاركة السرية.
أهورا إل أوركسترا بارسيا إل ستريم NDJSON `/privacy/events` تانتو بارا
الإدخالات `SoranetPrivacyEventV1` للأسهم `SoranetPrivacyPrioShareV1`،
reenviandolos إلى `SoranetSecureAggregator::ingest_prio_share`. الدلاء بحد ذاتها
قم بإصدار مساهمات `PrivacyBucketConfig::expected_shares`,
يعكس سلوك المجمع في التتابع. الأسهم صالحة
من أجل تقسيم الجرافة وشكل الرسم البياني قبل الدمج
`SoranetPrivacyBucketMetricsV1`. إذا تم الجمع بين الحساب والمصافحة من قبل
debajo de `min_contributors`، الدلو يتم تصديره مثل `suppressed`، يُرجع
El compportamiento del aggregador en el Relay. النوافذ العلوية الآن
قم بإصدار شارة `suppression_reason` لتمييز المشغلين بين بعضهم البعض
`insufficient_contributors`، `collector_suppressed`، `collector_window_elapsed`
y `forced_flush_window_elapsed` هو جهاز القياس عن بعد. لا رازون
`collector_window_elapsed` يتم تغييرها أيضًا عندما تكون الأسهم مدفوعة الأجر
alla de `max_share_lag_buckets`، لقد تم رؤية هواة الجمع بلا خطيئة
صيانة المراكم القديمة في الذاكرة.

## نقاط النهاية للتناول Torii

Torii الآن يعرض نقاط نهاية HTTP مع أجهزة القياس عن بعد التي يتم إرسالها
يقوم هواة الجمع بإعادة تقديم الملاحظات بدون إضافة وسيلة نقل مخصصة:- `POST /v1/soranet/privacy/event` يقبل الحمولة
  `RecordSoranetPrivacyEventDto`. الجسم يغلف un `SoranetPrivacyEventV1`
  ولكن هناك آداب اختيارية `source`. Torii التحقق من صحة الطلب
  سجل بيانات القياس عن بعد النشط، وقم بتسجيل الحدث والرد عبر HTTP
  `202 Accepted` مع مظروف Norito JSON الذي يحتوي على النافذة
  حساب الجرافة (`bucket_start_unix`, `bucket_duration_secs`) وال
  طريقة التتابع.
- `POST /v1/soranet/privacy/share` يقبل الحمولة `RecordSoranetPrivacyShareDto`.
  الجسم lleva un `SoranetPrivacyPrioShareV1` وتلميح اختياري `forwarded_by`
  حتى يتمكن المشغلون من رؤية تدفقات المجمعات. لاس انتريجاس اكسيتوساس
  تطوير HTTP `202 Accepted` مع مغلف Norito JSON لاستئناف El
  جامع، نافذة الدلو وتلميح القمع؛ لاس فالاس دي
  تشير عملية التحقق من الصحة إلى استجابة القياس عن بعد `Conversion` للحفظ
  طريقة تحديد الأخطاء في المجمعات. حلقة الحدث من الأوركسترا
  الآن قم بإصدار هذه الأسهم من خلال مرحلات المستشعرات، والحفاظ على التراكم
  يتم مزامنة Prio de Torii مع الجرافات الموجودة على التتابع.

توفر نقاط النهاية الشاملة ملف القياس عن بعد: يصدر خدمة 503
غير متاح` عندما يتم تعطيل المقاييس. يمكن للعملاء الحسد
cuerpos Norito ثنائي (`application/x.norito`) أو Norito JSON (`application/x.norito+json`)؛
يتعامل الخادم مع التنسيق تلقائيًا عبر المستخرجات القياسية
Torii.## متريكاس Prometheus

كل دلو تم تصديره يحتوي على العلامات `mode` (`entry`, `middle`, `exit`) y
`bucket_start`. قم بإصدار المجموعات التالية من المقاييس:| متريكا | الوصف |
|--------|------------|
| `soranet_privacy_circuit_events_total{kind}` | تصنيف المصافحة مع `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | صمامات الخانق مع `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | يتم تقليل فترات التهدئة المجمعة عن طريق المصافحة. |
| `soranet_privacy_verified_bytes_total` | Ancho de banda تم التحقق من إثباتات الدواء. |
| `soranet_privacy_active_circuits_{avg,max}` | الوسائط والدوائر النشطة لكل دلو. |
| `soranet_privacy_rtt_millis{percentile}` | تقديرات النسبة المئوية لـ RTT (`p50`، `p90`، `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | مواجهات GAR hasheados من خلال ملخص الفئة. |
| `soranet_privacy_bucket_suppressed` | يتم الاحتفاظ بالدلاء حتى لا تغطي غطاء المساهمين. |
| `soranet_privacy_pending_collectors{mode}` | مجمعات الأسهم المعلقة المجمعة، المجمعة بطريقة التتابع. |
| `soranet_privacy_suppression_total{reason}` | قواطع الدلاء العلوية مع `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` لتخصيص فجوات الخصوصية. |
| `soranet_privacy_snapshot_suppression_ratio` | نسبة الحذف/التفريغ الأخيرة (0-1)، تستخدم للتنبيه المسبق. |
| `soranet_privacy_last_poll_unixtime` | الطابع الزمني لنظام UNIX لآخر استطلاع للخروج (تنبيه جامع الخمول). |
| `soranet_privacy_collector_enabled` | المقياس الذي يوجد به `0` عندما يتم تعطيل أداة تجميع الخصوصية أو إيقاف تشغيلها (تم تعطيل أداة تجميع التنبيهات). |
| `soranet_privacy_poll_errors_total{provider}` | فشل في تجميع الاستقصاءات بواسطة الأسماء المستعارة للترحيل (زيادة بسبب أخطاء فك التشفير، أو فشل HTTP، أو الرموز غير القابلة للتنفيذ). |يتم الحفاظ على الدلاء بدون ملاحظات صامتة، والحفاظ على لوحات المعلومات
Limpios لا تصنع نوافذ مع سيروس.

## جويا أوبيراتيفا1. **لوحات المعلومات** - رسم المقاييس السابقة المجمعة وفقًا لـ `mode` و
   `window_start`. Destaca ventanas faltantes للكشف عن مشاكل جامعي
   س المرحلات. الولايات المتحدة الأمريكية `soranet_privacy_suppression_total{reason}` للتميز
   رعاية المساهمين من خلال جمع الفرز. الأصول دي
   Grafana الآن يتضمن لوحة مخصصة **"أسباب المنع (5 دقائق)"**
   يتم تغذية هذه المحولات مع إحصائيات **"Suppressed Bucket %"** que
   احسب `sum(soranet_privacy_bucket_suppressed) / count(...)` حسب التحديد
   لكي يتمكن المشغلون من كسر افتراضات الرؤية. لا سيري
   **تراكم مشاركة المجمع** (`soranet_privacy_pending_collectors`) والإحصائيات
   **نسبة قمع اللقطة** تعيد هواة الجمع إلى التراكم والانجراف في الميزانية
   durante ejacuciones automatizadas.
2. **التنبيه** - إلغاء الإنذارات من جهات الاتصال الآمنة للخصوصية: صور الاسترداد
   PoW، تردد التهدئة، انجراف RTT والقدرة المرفوضة. كومو لوس
   المكثفات رتيبة في كل دلو، وتنظم الأسس في الوقت المحدد
   وظيفة جيدة.
3. **الاستجابة للحوادث** - الثقة الأولى بالبيانات المجمعة. عندما يتطلب الأمر
   إزالة الشوائب العميقة، وتطلب إعادة إنتاج لقطات من الدلاء o
   إثباتات فحص الأدوية الموجودة في مكان استعادة سجلات المرور
   com.crudos.
4. **الاحتفاظ** - يتم الحذف بتكرار كافٍ بحيث لا يتجاوز ذلك
   `max_completed_buckets`. يجب أن يقوم المصدرون بإخراج Prometheus comoقم بإلغاء تنشيط Canonica وقم بإزالة المجموعات المحلية مرة واحدة.

## تحليل القمع والتنفيذ التلقائي

يعتمد قبول SNNet-8 على إثبات أن المجمعات تتم آليًا
الحفاظ على الصحة وبقاء القمع داخل حدود السياسة
(<=10% من الدلاء لكل نافذة لمدة 30 دقيقة). الأدوات
من الضروري تضمينه في الريبو; يجب أن يكون المشغلون متكاملين معهم
طقوس الخاتمة. تم عرض لوحات القمع الجديدة في Grafana
مقتطفات من PromQL تظهر، وتتمكن من رؤية المعدات عند الطلب مسبقًا
كرر استشارة الأدلة.

### Recetas PromQL لمراجعة القمع

يجب أن يكون لدى المشغلين يد المساعدة التالية PromQL؛ أمبوس حد ذاتها
مرجعي في لوحة القيادة المقسمة (`dashboards/grafana/soranet_privacy_metrics.json`)
وقواعد مدير التنبيهات:

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

يتم استخدام النسبة لتأكيد الحفاظ على الإحصائيات **"Suppressed Bucket %"**
من أجل فرض سياسة مسبقة؛ قم بتوصيل كاشف المسامير أ
مدير تنبيه للتعليقات السريعة عندما يكون محتوى المساهمة أقل
شكل غير متوقع.

### CLI لتقارير المستودعات غير المتصلة بالإنترنت

تعرض مساحة العمل `cargo xtask soranet-privacy-report` لالتقاط NDJSON
بونتوال. Apuntalo a uno o mas Exports admin del Relay:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```المساعدة في معالجة الالتقاط باستخدام `SoranetSecureAggregator`، إنشاء استئناف
Supresion en stdout ويمكنك اختياريًا كتابة تقرير JSON estructurado عبر
`--json-out <path|->`. احترام نفس المقابض التي جامع في الجسم الحي
(`--bucket-secs`، `--min-contributors`، `--expected-shares`، وما إلى ذلك)، السماح
قم بإعادة إنتاج اللقطات التاريخية بحدود مختلفة عند التحقيق في مشكلة ما.
قم بإضافة JSON مع لقطات شاشة Grafana لبوابة التحليل
SNNet-8 سيجا سييندو قابلة للتدقيق.

### قائمة التحقق من التنفيذ التلقائي الأول

الحوكمة من شأنها أن تجعل التنفيذ التلقائي الأول يكتمل
يفترض القمع. المساعد الآن يقبل `--max-suppression-ratio <0-1>`
لكي يتعطل مشغلوك بسرعة عندما تتجاوز الدلاء الزائدة
يُسمح بفتح النافذة (افتراضي 10%) أو عندما لا يتم تقديم دلاء قش. فلوجو
موصى به:

1. قم بتصدير NDJSON من مسؤول نقطة النهاية للتتابع والبث
   `/v1/soranet/privacy/event|share` منسق الموسيقى
   `artifacts/sorafs_privacy/<relay>.ndjson`.
2. تنفيذ المساعد بافتراض السياسة:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```الأمر يطبع النسبة المرصودة والبيع برمز مميز من الذهب
   عندما يتجاوز الافتراض **o** عندما لا يكون هناك قوائم دلو من القش، فهذا يعني
   لم يتم إنتاج جهاز القياس عن بعد للقذف. لاس المقاييس أون
   فيفو ديبن موسترار `soranet_privacy_pending_collectors` دريناندو هاسيا سيرو واي
   `soranet_privacy_snapshot_suppression_ratio` يخفي نفس الشيء
   يفترض أن يتم تنفيذ التمرين.
3. أرشيف JSON الناتج وسجل CLI باستخدام حزمة الأدلة SNNet-8
   قبل تغيير النقل الافتراضي حتى تتمكن من إجراء التحديثات
   إعادة إنتاج القطع الأثرية الدقيقة.

## بروكسيموس باسوس (SNNet-8a)- دمج المجمعات الثنائية المميزة، وربط مشاركة المشاركة في وقت التشغيل
  لكي تقوم المرحلات والجامعات بإصدار حمولات `SoranetPrivacyBucketMetricsV1`
  يتسق. *(تم - الإصدار `ingest_privacy_payload` en
  `crates/sorafs_orchestrator/src/lib.rs` واختبارات الشركاء.)*
- نشر لوحة المعلومات Prometheus المقسمة وأنظمة التنبيه الخاصة بالشاشة
  فجوات القمع، وصحة التجميع، وعدم الكشف عن هويته. *(تم - الإصدار
  `dashboards/grafana/soranet_privacy_metrics.json`,
  `dashboards/alerts/soranet_privacy_rules.yml`,
  `dashboards/alerts/soranet_policy_rules.yml`، وتركيبات التحقق من الصحة.)*
- إنتاج المصنوعات اليدوية لمعايرة الخصوصية التفاضلية الموصوفة باللغة الإنجليزية
  `privacy_metrics_dp.md`، بما في ذلك دفاتر الملاحظات القابلة للنسخ والخلاصات
  الحكم. *(تم - دفتر + المصنوعات اليدوية التي تم إنشاؤها بواسطة
  `scripts/telemetry/run_privacy_dp.py`; المجمع CI
  `scripts/telemetry/run_privacy_dp_notebook.sh` قم بتشغيل الكمبيوتر المحمول عبر el
  سير العمل `.github/workflows/release-pipeline.yml`; خلاصة الحوكمة أرشيفية ar
  `docs/source/status/soranet_privacy_dp_digest.md`.)*

الإصدار الفعلي يدخل في قاعدة SNNet-8: تحديد القياس عن بعد وآمن
للخصوصية التي تتكامل مباشرة مع الكاشطات ولوحات المعلومات Prometheus
موجود. توجد قوائم مصطنعة لمعايرة الخصوصية التفاضلية،
يقوم سير العمل بتحرير اللوحات الجدارية من مخلفات دفتر الملاحظات والعمل
يتم التركيز على مراقبة التشغيل التلقائي الأول والموسع
تنبيهات تحليلية للقمع.