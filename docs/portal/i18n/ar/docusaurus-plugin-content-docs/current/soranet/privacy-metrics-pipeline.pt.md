---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/privacy-metrics-pipeline.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خط أنابيب مقاييس الخصوصية
العنوان: خط أنابيب قياسات الخصوصية في SoraNet (SNNet-8)
Sidebar_label: خط أنابيب لمقاييس الخصوصية
الوصف: مجموعة القياس عن بعد التي تحافظ على خصوصية المرحلات والمنسقين في SoraNet.
---

:::ملاحظة فونتي كانونيكا
ريفليت `docs/source/soranet/privacy_metrics_pipeline.md`. Mantenha ambas as copias sincronzadas.
:::

# خط أنابيب مقاييس الخصوصية من SoraNet

قدم SNNet-8 سطحًا للقياس عن بعد يتميز بالخصوصية لوقت تشغيل التتابع. قم بالترحيل الآن من خلال جمع أحداث المصافحة والدائرة في مجموعات لمدة دقيقة واحدة وتصدير عدد قليل من المقاتلين Prometheus الإجمالي، مع الحفاظ على دوائر فردية غير مكتملة حتى تتمكن من رؤية العمل من قبل المشغلين.

## Visao geral do aregador- يتم تنفيذ أمر وقت التشغيل على `tools/soranet-relay/src/privacy.rs` مثل `PrivacyAggregator`.
- يتم غسل الدلاء في دقيقة واحدة من الساعة (`bucket_secs`، الافتراضي 60 ثانية) ويتم تخزينها في حلقة محدودة (`max_completed_buckets`، الافتراضي 120). يقوم المُجمِّع بإدارة أعماله المتراكمة المحدودة (`max_share_lag_buckets`، الافتراضي 12) حتى يتم إنشاء مجموعات Prio antigas مثل دلاء إضافية في وقت تخزين الذاكرة أو إخفاء أسعار المجمعات.
- `RelayConfig::privacy` خريطة مباشرة لـ `PrivacyConfig`، مقابض الضبط (`bucket_secs`، `min_handshakes`، `flush_delay_buckets`، `force_flush_buckets`، `max_completed_buckets`، `max_share_lag_buckets`، `expected_shares`). يحافظ وقت تشغيل الإنتاج على الإعدادات الافتراضية أثناء تقديم SNNet-8a عتبات التجميع الآمن.
- تسجل وحدات وقت التشغيل الأحداث عبر أنواع المساعدة: `record_circuit_accepted`، `record_circuit_rejected`، `record_throttle`، `record_throttle_cooldown`، `record_capacity_reject`، `record_active_sample`، `record_verified_bytes`، و`record_gar_category`.

## مسؤول نقطة النهاية يقوم بالترحيليمكن للمشغلين استشارة مسؤول المستمع أو ترحيل الملاحظات الشاملة عبر `GET /privacy/events`. تعيد نقطة النهاية JSON المحددة بواسطة خطوط جديدة (`application/x-ndjson`) إلى الحمولات النافعة `SoranetPrivacyEventV1` المتماثلة إلى `PrivacyEventBuffer` الداخلية. يقوم المخزن المؤقت بحماية الأحداث الجديدة التي تم إدخالها `privacy.event_buffer_capacity` (افتراضي 4096) ويتم مسحه على الشاشة، بما في ذلك الكاشطات التي يتم استشعارها بتردد كافٍ لتجنب الثغرات. تشتمل الأحداث على المصافحة والخانق وعرض النطاق الترددي الذي تم التحقق منه والدائرة النشطة وGAR التي تغذي وحدات التحكم Prometheus، مما يسمح لهواة التجميع بالحصول على فتات الخبز الآمنة للخصوصية أو توفير سير العمل من أجل التجميع الآمن.

## تكوين التتابع

يقوم المشغلون بضبط إيقاع قياس الخصوصية عن بعد من خلال ملف التكوين الذي يتم ترحيله عبر المفتاح `privacy`:

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

تتوافق الإعدادات الافتراضية ذات النطاقين مع SNNet-8 المحدد ولا يتم تحميلها بشكل صحيح:| كامبو | وصف | بادراو |
|-------|-----------|--------|
| `bucket_secs` | Largura de cada janela de agregacao (ثانية). | `60` |
| `min_handshakes` | الحد الأدنى من عدد المساهمين قبل دلو يمكن أن يطلق المتنافسين. | `12` |
| `flush_delay_buckets` | الدلاء مكتملة للانتظار قبل البدء بالتدفق. | `1` |
| `force_flush_buckets` | الحد الأقصى من الحد الأقصى قبل إطلاق دلو فوق. | `6` |
| `max_completed_buckets` | تراكم المستودعات المتبقية (يعوق الذاكرة دون حدود). | `120` |
| `max_share_lag_buckets` | Janela de retencao لهواة جمع الأسهم قبل الحذف. | `12` |
| `expected_shares` | يشارك جامع Prio exigidos قبل التجميع. | `2` |
| `event_buffer_capacity` | تراكم أحداث NDJSON لمشرف البث. | `4096` |

حدد `force_flush_buckets` على الأقل `flush_delay_buckets`، أو إيقاف العتبات، أو عدم الرضا أو حماية الاحتفاظ الآن قبل التحقق من الصحة لتجنب عمليات النشر التي تستخدم القياس عن بعد للترحيل.

الحد `event_buffer_capacity` هو أيضًا الحد `/admin/privacy/events`، مما يضمن أن الكاشطات لا يمكنها أن تفشل إلى أجل غير مسمى.

## أسهم جامع بريوSNNet-8a مُجمعات مزروعة مزدوجة تُصدر دلاءً من الدرجة الأولى مع مشاركة سرية. قام المنسق الآن بتحليل الدفق NDJSON `/privacy/events` للإدخال `SoranetPrivacyEventV1` ومشاركة `SoranetPrivacyPrioShareV1`، وذلك من أجل `SoranetSecureAggregator::ingest_prio_share`. تُصدر الدلاء عندما تقوم بمساهمة `PrivacyBucketConfig::expected_shares`، أو ترحيلها أو طريقة ترحيلها. كما أن المشاركات تتمتع بصلاحيات لتعزيز الدلو وشكل الرسم البياني قبل أن تكون مجموعات في `SoranetPrivacyBucketMetricsV1`. إذا تم الجمع بين المصافحة عبر `min_contributors`، أو الحاوية والتصدير مثل `suppressed`، فلا يتم ترحيل حالة التجميع. أصدرت Janelas أعلاه مؤخرًا علامة `suppression_reason` حتى يتمكن المشغلون من التمييز بين `insufficient_contributors` و`collector_suppressed` و`collector_window_elapsed` و`forced_flush_window_elapsed` لتشخيص ثغرات القياس عن بعد. يتم أيضًا تغيير الدافع `collector_window_elapsed` عندما يتم مشاركة Prio على أي حال من `max_share_lag_buckets`، فإن جامعي الإعصار يضغطون على أنفسهم دون إزالة المراكم القديمة في الذاكرة.

## نقاط نهاية الإدخال do Torii

Torii الآن يعرض نقطتي النهاية HTTP مع بوابة القياس عن بعد حتى تتمكن المرحلات والمجمعات من جمع الملاحظات دون إرسال وسيلة نقل مخصصة:- `POST /v1/soranet/privacy/event` يستخدم الحمولة `RecordSoranetPrivacyEventDto`. يشتمل الجسم على `SoranetPrivacyEventV1` مع تسمية `source` اختيارية. Torii التحقق من صحة الطلب مقابل ملف القياس عن بعد أو التسجيل أو الحدث والرد عبر HTTP `202 Accepted` جنبًا إلى جنب مع المغلف Norito JSON يتنافس على حساب السجل (`bucket_start_unix`، `bucket_duration_secs`) هـ أو طريقة التتابع.
- `POST /v1/soranet/privacy/share` يستخدم الحمولة `RecordSoranetPrivacyShareDto`. يحمل الجسم `SoranetPrivacyPrioShareV1` ويقال `forwarded_by` اختياريًا حتى يتمكن المشغلون من مراقبة تدفقات المجمعات. يتم إرساله بنجاح مرة أخرى إلى HTTP `202 Accepted` مع مغلف Norito JSON يستعيد أو يُجمّع، ويملأ الجرافة ويحذف القمع؛ تم التحقق من صحة الخريطة لرد القياس عن بعد `Conversion` للحفاظ على تصحيح الأخطاء بين المجمعين. ستصدر حلقة أحداث المنسق هذه المشاركات من خلال إجراء استطلاع للمرحلات، مع الحفاظ على التراكم الأولي لـ Torii المتزامن مع الدلاء بدون مرحل.

من خلال نقاط النهاية المؤقتة أو ملف القياس عن بعد: أرسل `503 Service Unavailable` عندما تكون المقاييس غير صالحة. يمكن للعملاء إرسال المجموعة Norito الثنائية (`application/x.norito`) أو Norito JSON (`application/x.norito+json`)؛ يتم تشغيل الخادم تلقائيًا أو تنسيق عبر المستخرجين بواسطة Torii.

## متريكاس Prometheusملصقات كل دلو تصدير كاريجا `mode` (`entry`، `middle`، `exit`) و`bucket_start`. كما يلي عائلات القياسات الخاصة بها:| متري | الوصف |
|--------|------------|
| `soranet_privacy_circuit_events_total{kind}` | تصنيف المصافحة com `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | صمامات الخانق مع `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | تم ضبط فترات التهدئة المتراكمة عن طريق المصافحة. |
| `soranet_privacy_verified_bytes_total` | يتم التحقق من عرض النطاق الترددي من خلال التجارب الطبية. |
| `soranet_privacy_active_circuits_{avg,max}` | الوسائط والدوائر متعددة الوظائف لكل دلو. |
| `soranet_privacy_rtt_millis{percentile}` | تقديرات RTT المئوية (`p50`، `p90`، `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | تقرير إجراءات الحوكمة الخاص بـ Contadores de Governance Action Report هو عبارة عن تجزئة لملخص الفئة. |
| `soranet_privacy_bucket_suppressed` | الدلاء المتبقية بسبب أو الحد من المساهمين لم يتم فعلها. |
| `soranet_privacy_pending_collectors{mode}` | مجمعات الأسهم المجمعة المعلقة، المجمعة بطريقة التتابع. |
| `soranet_privacy_suppression_total{reason}` | قواطع الدلاء العلوية مع `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` حتى توفر لوحات المعلومات ثغرات في الخصوصية. |
| `soranet_privacy_snapshot_suppression_ratio` | رازاو سوبريميدا/درنادا في نهاية المطاف (0-1)، باستخدام ميزانيات التنبيه. |
| `soranet_privacy_last_poll_unixtime` | يقوم Timestamp UNIX بإجراء استطلاع نهائي للنجاح (التغذية أو تنبيه المجمع الخامل). |
| `soranet_privacy_collector_enabled` | قم بقياس `0` عندما يكون مجمع الخصوصية معطلاً أو معطلاً. || `soranet_privacy_poll_errors_total{provider}` | أخطاء الاقتراع المجمعة بواسطة الاسم المستعار للترحيل (زيادة أخطاء فك التشفير، أو خطأ HTTP، أو رموز الحالة غير القابلة للانفجار). |

يتم ملاحظة الدلاء بشكل صامت دائمًا، والحفاظ على لوحات العدادات نظيفة حتى يتم تصنيعها مرة أخرى.

## أورينتكاو التشغيلي1. **لوحات المعلومات** - تتبع حسب المقاييس المستخدمة في `mode` و`window_start`. قم بإزالة المخلفات من أجل الكشف عن مشاكل التجميع أو التتابع. استخدم `soranet_privacy_suppression_total{reason}` للتمييز بين المساهمين الخاطئين من المثبطين الموجهين لجامعي الثغرات في سد الثغرات. أرسل الأصل Grafana الآن لوحة مخصصة **"Suppression Reasons (5m)"** تم تغذيتها بواسطة قواطع إضافية لإحصائيات **"Suppressed Bucket %"** التي تحسب `sum(soranet_privacy_bucket_suppressed) / count(...)` لتحديد المشغلين الذين يقومون بانتهاكات الميزانية بسرعة. سلسلة **"Collector Share Backlog"** (`soranet_privacy_pending_collectors`) وإحصائيات **"Snapshot Suppression Ratio"** تقوم بجمع أدوات التجميع من الضغط ونقص الميزانية أثناء التنفيذ التلقائي.
2. **التنبيه** - إرسال إنذارات من عناصر التحكم الآمنة للخصوصية: رفض صور PoW، وتردد التبريد، وانجراف RTT، ورفض السعة. كما أن المقاييس رتيبة في كل دلو، فإنها تصلح بشكل بسيط بناءً على وظيفتها.
3. **الاستجابة للحوادث** - ثق بوالديك المتفق عليهما أولاً. عند الحاجة إلى تصحيح أكثر عمقًا، اطلب إعادة إنتاج لقطات من الجرافات أو فحص تجريبي طبي أثناء جمع سجلات المرور الشاملة.4. **الاحتفاظ** - إزالة التردد الكافي لتجنب تجاوز `max_completed_buckets`. يجب على المصدرين أن يجروا Prometheus مثل خط Canonica ويزيلوا الدلاء من مكان التنقيب.

## تحليل القمع والتنفيذ التلقائي

يعتمد استخدام SNNet-8 على إظهار أن المجمعات الآلية تدوم طويلاً وأن يتم قمعها داخل حدود سياسية (<= 10% من البيانات لكل 30 دقيقة). الأدوات اللازمة لشراء هذه البوابة موجودة في المستودع؛ يجب أن يتكامل المشغلون مع طقوسهم الخاصة طوال الأسبوع. تعكس آلام القمع الجديدة التي يسببها Grafana ثلاثة أضعاف PromQL حتى تتمكن معدات النباتات من رؤية الحياة قبل إعادة تصحيح الاستشارات اليدوية.

### إيصالات PromQL لمراجعة الحذف

يقوم المشغلون بتوفير المساعدة التالية لـ PromQL إلى ماو؛ جميع المراجع الخاصة بلوحة المعلومات Grafana تشترك (`dashboards/grafana/soranet_privacy_metrics.json`) وتتبع مدير التنبيه:

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

استخدم النسبة المئوية لتأكيد الإحصائيات **"Suppressed Bucket %"** الدائمة بعد الميزانية السياسية؛ قم بتوصيل كاشف المسامير إلى مدير التنبيه للتعليقات السريعة عند إرسال المساهمين بشكل مستمر.

### سطر الأوامر الخاص بالدلو غير المتصل بالإنترنتمساحة العمل تعرض `cargo xtask soranet-privacy-report` لالتقاط صور NDJSON. يقوم Aponte para um ou mais بتصدير admin de Relay:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

للمساعدة في تمرير الالتقاط إلى `SoranetSecureAggregator`، قم بطباعة استئناف الحذف بدون إعدادات قياسية، واختياريًا، قم برسم علاقة JSON التي تم إنشاؤها عبر `--json-out <path|->`. هذه هي المقابض المجمعة في الجسم الحي (`--bucket-secs`، `--min-contributors`، `--expected-shares`، وما إلى ذلك)، مما يسمح للمشغلين بإعادة التقاط صور تاريخية لعتبات مختلفة عند تسجيل حادثة. قم بإضافة JSON جنبًا إلى جنب مع التقاطات Grafana حتى تتمكن بوابة تحليل SNNet-8 من الاستمرار في التدقيق.

### قائمة التحقق من التنفيذ التلقائي الأول

هناك حاجة إلى إثبات أن التنفيذ التلقائي الأول يتم من خلال قمع الميزانية. هذا المساعد هو `--max-suppression-ratio <0-1>` لكي يفشل CI أو المشغلون بسرعة عندما تتجاوز الدلاء الحد الأقصى المسموح به (افتراضي 10%) أو عندما لا تتجاوز الدلاء. الموصى بها فلوكسو:

1. قم بتصدير NDJSON إلى مسؤول نقاط النهاية ليقوم بالترحيل أو الدفق `/v1/soranet/privacy/event|share` إلى المنسق لـ `artifacts/sorafs_privacy/<relay>.ndjson`.
2. ركب أو ساعد في ميزانية السياسة:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```O comando imprime a razao observada e sai com codigo nao Zero quando o Budget e excedido **ou** quando ainda nao ha Bucks prontos, sinalizando que a telemetry ainda nao foi produzida para a execucao. كما يجب أن تظهر مقاييس الجسم الحي `soranet_privacy_pending_collectors` من الضغط إلى الصفر و`soranet_privacy_snapshot_suppression_ratio` حتى يتم تنفيذ نفس الميزانية.
3. احفظ ملف JSON وسجل CLI باستخدام حزمة الأدلة SNNet-8 قبل التروكر أو النقل الافتراضي حتى تتمكن المراجع من إعادة إنتاج المصنوعات الإضافية.

## بروكسيموس باسوس (SNNet-8a)

- دمج مجمعات Prio المزدوجة، وقم بتوصيل المشاركات في وقت التشغيل حتى تتمكن المرحلات والمجمعات من إصدار حمولات `SoranetPrivacyBucketMetricsV1` بشكل متسق. *(الخلاصة - تم الاطلاع على `ingest_privacy_payload` في `crates/sorafs_orchestrator/src/lib.rs` والخصيتين المرتبطتين.)*
- نشر لوحة المعلومات Prometheus ومشاركتها ولوائح التنبيه التي تغطي ثغرات القمع، بالإضافة إلى المجمعين والأربعاء المجهولين. *(الخلاصة - التحقق من `dashboards/grafana/soranet_privacy_metrics.json`، `dashboards/alerts/soranet_privacy_rules.yml`، `dashboards/alerts/soranet_policy_rules.yml` وتركيبات التحقق.)*
- قم بإنتاج أدوات معايرة الخصوصية ذات الوصف التفاضلي في `privacy_metrics_dp.md`، بما في ذلك دفاتر الملاحظات المنتجة وملخصات الإدارة. *(الخلاصة - دفتر الملاحظات والأشياء الجيدة لـ `scripts/telemetry/run_privacy_dp.py`؛ غلاف CI `scripts/telemetry/run_privacy_dp_notebook.sh` تنفيذ دفتر الملاحظات عبر سير العمل `.github/workflows/release-pipeline.yml`؛ خلاصة الإدارة المحفوظة في `docs/source/status/soranet_privacy_dp_digest.md`.)*إصدار فعلي يدخل إلى قاعدة SNNet-8: القياس عن بعد الحتمية والأمن للخصوصية التي تتضمن مباشرة كاشطاتنا ولوحات المعلومات Prometheus الموجودة. أدوات معايرة الخصوصية التفاضلية ليست في مكانها، أو يقوم سير العمل بتحرير خط الأنابيب لحماية مخرجات الكمبيوتر الدفتري المحدثة، أو يركز العمل على مراقبة التنفيذ التلقائي الأول وتوسيع نطاق تحليلات تنبيه القمع.