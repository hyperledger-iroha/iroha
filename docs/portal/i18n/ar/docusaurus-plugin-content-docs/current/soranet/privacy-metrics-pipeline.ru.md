---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/privacy-metrics-pipeline.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خط أنابيب مقاييس الخصوصية
العنوان: جهاز قياس الخصوصية SoraNet (SNNet-8)
Sidebar_label: مقياس الخصوصية للناقل
الوصف: جميع أجهزة القياس عن بعد مع خصوصية التتابع والمنسق SoraNet.
---

:::note Канонический источник
أخرج `docs/source/soranet/privacy_metrics_pipeline.md`. قم بالنسخ المتزامن حتى لا يتم استخلاصها من الاستثناءات من الوثائق القديمة.
:::

# مقياس ناقل الخصوصية SoraNet

يوفر SNNet-8 توجيهًا عاليًا للخصوصية لأجهزة القياس عن بعد لترحيل وقت التشغيل. قم بترحيل مجموعة من المصافحة والدوائر في دقائق وتصدير الكثير من الرقائق Prometheus، إلغاء الدوائر غير مرغوب فيه ويجعل المشغل عمليًا.

## مطلوب مجمّع- يتم تحقيق وقت التشغيل من `tools/soranet-relay/src/privacy.rs` إلى `PrivacyAggregator`.
- يتم تفكيك الدلاء في غضون دقيقة واحدة (`bucket_secs`، لمدة 60 ثانية) ويتم تحريكها في عمود إضافي (`max_completed_buckets`، من خلال 120). تتضمن أسهم المجمع تراكمًا متراكمًا ذاتيًا (`max_share_lag_buckets`، في الجزء 12)، مما يؤدي إلى الاستفادة القصوى من Prio مثل الدلاء المكبوتة، وليس تم استخدامه في الحمام أو في أغطية لهواة جمع التحف.
- `RelayConfig::privacy` يتوافق تمامًا مع `PrivacyConfig`، ويفتح الإعدادات (`bucket_secs`، `min_handshakes`، `flush_delay_buckets`، `force_flush_buckets`، `max_completed_buckets`، `max_share_lag_buckets`، `expected_shares`). في وقت تشغيل الإنتاج، يتم تخزينه في الهواء الطلق، حيث يقوم SNNet-8a بإعادة تجميع غير آمن.
- تقوم وحدات وقت التشغيل بتسجيل الاشتراك من خلال المساعدين المسجلين: `record_circuit_accepted`، `record_circuit_rejected`، `record_throttle`، `record_throttle_cooldown`، `record_capacity_reject`، `record_active_sample`، `record_verified_bytes`، و`record_gar_category`.

## تتابع نقطة النهاية الإداريةيمكن للمشغلين إيقاف ترحيل مستمع المشرف للمراقبين عبر `GET /privacy/events`. ترسل نقطة النهاية JSON بضغط متفاوت (`application/x-ndjson`)، الحمولات الصافية المصاحبة `SoranetPrivacyEventV1`، خارج الشبكة `PrivacyEventBuffer`. يحتوي العازل على أحدث المكونات الجديدة حتى `privacy.event_buffer_capacity` مكتوبة (في الإلغاء 4096) وتتمثل في المخلفات، بعد المكشطة نحن بحاجة إلى التحدث إلى حد ما لإخراج المنتج. يتم إظهارها أيضًا من خلال المصافحة والخانق وعرض النطاق الترددي الذي تم التحقق منه والدائرة النشطة وGAR، والتي تحتوي على لوحات تحكم Prometheus، مما يسمح بأرشيف جامعي المصب فتات التنقل الآمنة الخاصة أو تدعم التجميع الآمن لسير العمل.

## تتابع التكوين

يقوم المشغلون بتعيين الإيقاع لأجهزة القياس عن بعد الخاصة في تتابع ملف التكوين من خلال القسم `privacy`:

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

بادئ ذي بدء ، مواصفات SNNet-8 والتحقق منها عند التحميل:| بول | الوصف | По умолчанию |
|------|----------|--------------|
| `bucket_secs` | شيرينا كل دقيقة تجميع (ثواني). | `60` |
| `min_handshakes` | الحد الأدنى من المياه قبل تطهير دلو. | `12` |
| `flush_delay_buckets` | مجموعة كبيرة من الدلاء قبل تدفق المياه. | `1` |
| `force_flush_buckets` | أقصى قدر من النفاذية قبل دلو مكبوت. | `6` |
| `max_completed_buckets` | الجرافات المتراكمة (لا يمكنك رميها بدون الجرانيت). | `120` |
| `max_share_lag_buckets` | حسنًا ، يتم تعزيز أسهم جامع الإدارة. | `12` |
| `expected_shares` | أسهم جامع شيسلو بريو قبل الالتزام بها. | `2` |
| `event_buffer_capacity` | Backlog NDJSON событий для admin-потока. | `4096` |

يؤدي التثبيت `force_flush_buckets` إلى `flush_delay_buckets` إلى التحقق من صحة عملية الإلغاء أو إلغاء حماية الحماية اختر إعادة الإرسال التي سيتم توصيلها بالقياس عن بعد إلى مرحل البيئة.

يتم تحديد الحد `event_buffer_capacity` أيضًا بواسطة `/admin/privacy/events`، مما يضمن عدم السماح للأجهزة الزاحفة بالخروج بسهولة.

## أسهم جامع بريوتعمل SNNet-8a على إنشاء مجمعات مزدوجة يتم ضخها بشكل سري من خلال دلاء Prio. يقوم المنسق ببارسي NDJSON بالضغط على `/privacy/events` لكتابة `SoranetPrivacyEventV1` ومشاركة `SoranetPrivacyPrioShareV1`، ويضبطه في `SoranetSecureAggregator::ingest_prio_share`. يتم سحب الدلاء عندما يتم توصيل `PrivacyBucketConfig::expected_shares`، مما يؤدي إلى تتابع التتابع. يتم التحقق من صحة المشاركات من خلال مستودعات البيانات ونماذج التسجيلات قبل الالتزام بها في `SoranetPrivacyBucketMetricsV1`. إذا كانت المصافحة قد تمت بواسطة `min_contributors`، يتم تصدير الجرافة مثل `suppressed`، مجمع التجميع العكسي تتابع الهواء. قم بتمرير الضغط مرة أخرى إلى `suppression_reason` بحيث يمكن للمشغلين الاستفادة من `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed` و `forced_flush_window_elapsed` عند فحص أجهزة القياس عن بعد. تم أيضًا إنشاء `collector_window_elapsed` عندما يتم بيع أسهم Prio بشكل كبير `max_share_lag_buckets`، مما أدى إلى ظهور هواة الجمع المتميزين بدون بطاريات فخمة في الهواء الطلق.

## نقاط النهاية Torii

Torii هناك نقطتان من نقاط اتصال HTTP العامة التي يمكن للمرحلات والمجمعات الاتصال بها بدون مراقبة النقل الذاتي:- `POST /v2/soranet/privacy/event` يوضح الحمولة `RecordSoranetPrivacyEventDto`. لقد تم قبول `SoranetPrivacyEventV1` بالإضافة إلى `source`. Torii يتحقق من خلال الملف الشخصي النشط عن بعد، ويسجل البيانات ويسجل HTTP `202 Accepted` الآن Norito JSON-convertom، содеращим очисленное окно (`bucket_start_unix`، `bucket_duration_secs`) وتتابع التتابع.
- `POST /v2/soranet/privacy/share` يوضح الحمولة `RecordSoranetPrivacyShareDto`. هناك حاجة إلى `SoranetPrivacyPrioShareV1` ورسالة جديدة غير متوقعة إلى `forwarded_by` التي يمكن للمشغلين من خلالها مراجعة جامعي النقاط. يتم إجراء عمليات الفحص الناجحة عبر HTTP `202 Accepted` مع Norito JSON-Convertom، وجامع مملوء، ودلو أوكنو، ودعم الإيصال؛ يتم التحقق من صحة البيانات باستخدام جهاز قياس عن بعد `Conversion` من أجل تسهيل عملية تحديد أجهزة الكمبيوتر بين جامعي. بعد ذلك، يقوم المنسق الخاص بالمشاركة في معالجة المرحلات، والمزامنة المصاحبة لبطارية Prio Torii مع دلاء التتابع.

تقوم نقطة البداية هذه بقراءة ملف تعريف أجهزة القياس عن بعد: يتم توصيلها بـ `503 Service Unavailable`، عندما يتم استبعاد المقاييس. يمكن للعملاء إجراء اتصال Norito ثنائي (`application/x.norito`) أو Norito JSON (`application/x.norito+json`)؛ يتوافق الخادم التلقائي مع التنسيق من خلال مستخرجات Torii القياسية.

## المقاييس Prometheusتحتوي كل دلو للتصدير على `mode` (`entry`، `middle`، `exit`) و`bucket_start`. اختر مقياس المجتمع التالي:| متري | الوصف |
|--------|------------|
| `soranet_privacy_circuit_events_total{kind}` | المصافحة مع `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | دواسة الوقود مع `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | تسارع التهدئة، المصافحات المخنوقة غير الضرورية. |
| `soranet_privacy_verified_bytes_total` | تم التحقق من صحة الاحتمالية من التصريحات الخادعة. |
| `soranet_privacy_active_circuits_{avg,max}` | اجمع واختر الدوائر النشطة في الجرافة. |
| `soranet_privacy_rtt_millis{percentile}` | النسب المئوية لـ RTT (`p50`، `p90`، `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | تقرير إجراءات الحوكمة، فئة الملخص. |
| `soranet_privacy_bucket_suppressed` | تم تعليق الدلاء، مما أدى إلى عدم كفاية المياه. |
| `soranet_privacy_pending_collectors{mode}` | أسهم جامع البطاريات في الالتزام بالصيانة، المجمعة من خلال نظام التتابع. |
| `soranet_privacy_suppression_total{reason}` | الدلاء المكبوتة مع `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}`، والتي يمكن أن تمنح لوحات الاتصال الخصوصية. |
| `soranet_privacy_snapshot_suppression_ratio` | قم بقمع/قطع الصرف التالي (0-1)، مفيد لتنبيهات الميزانية. |
| `soranet_privacy_last_poll_unixtime` | UNIX-في وقت قصير للغاية، مشكلة جيدة (تنبيه صغير لجامع الخمول). |
| `soranet_privacy_collector_enabled` | المقياس الذي يصل إلى `0`، عندما يكون مُجمِّع الخصوصية محظورًا أو غير مُغلق (مجمع تنبيهات صغير معطل). || `soranet_privacy_poll_errors_total{provider}` | مشكلة أخرى عن طريق الاسم المستعار Relay (يتم التحقق منها عند فك تشفير الملفات أو اتصالات HTTP أو رموز الحالة غير المرغوب فيها). |

تحتوي الدلاء بدون مراقبة على حشوات معدنية مصنوعة من مواد غير قابلة للتصنيع.

## توصيات التشغيل1. **لوحة البيانات** - قم بتركيب المقاييس الأخرى، المجمعة على `mode` و`window_start`. قم بالإشارة إلى النقطة المناسبة لحل مشاكل المجمعات أو المرحلات. استخدم `soranet_privacy_suppression_total{reason}` لأنواع مختلفة من المخلفات والقمع، وهواة التجميع المزعجين، أثناء فرز الاختبارات. في Grafana توجد هذه اللوحة **"أسباب المنع (5 م)"** على أساس هذه العدادات والإحصائيات **"دلو مكبوت %"**، `sum(soranet_privacy_bucket_suppressed) / count(...)` التالي اختر خيارًا ليتمكن المشغلون من رؤية الميزانية المرتفعة من خلال العرض الأول. سلسلة **"Collector Share Backlog"** (`soranet_privacy_pending_collectors`) وإحصائيات **"Snapshot Suppression Ratio"** تحدد أفضل جامعي وميزانية صغيرة خلال الأتمتة التلقائية بروغونوف.
2. **التنبيه** - قم بإيقاف تشغيل الاتصالات من أجهزة الكمبيوتر الشخصية الآمنة: رفض PoW الشامل، وتهدئة مؤقتة، وRTT قصيرة، ورفض السعة. بعد أن تم استخدام الدلو الرتيب في كل مرة، كان من الجيد فقط العمل بشكل جيد.
3. **إفصاح عن الحدث** - ابدأ بالاستفادة من البيانات المجمعة. عندما تحتاج إلى المزيد من المحادثات التليفزيونية، قم بمتابعة دلاء اللقطات الملتقطة أو التحقق من المزامنة الجيدة في جميع أنحاء العالم حركة الشعار.
4. **المراقبة** - قم بقص الجزء الكامل حتى لا تتمكن من إعادة `max_completed_buckets`. يجب على المصدرين قراءة المصدر Prometheus الأساسي وتشغيل الدلاء المحلية بعد الانتهاء.## قمع التحليلات والتحسينات التلقائية

تتعرف شركة SNNet-8 على العرض التوضيحي الذي يوضح أن المجمعات الآلية تفتقد إلى الأمان، وهو ما أدى إلى القمع في السياسات السابقة (<10% دلاء التتابع لمدة 30 دقيقة تقريبًا). يتم وضع الأدوات الجديدة في مكانها مع المستودع؛ يجب على المشغلين أن يقوموا بإنشائها وفقًا للطقوس الدورية. تعمل لوحة القمع الجديدة في Grafana على إعادة ضبط PromQL، مما يجعل الأمر الأخير حيًا إلى حد ما، مثل ذلك يرجى تقديم طلب جيد.

### تعليمات PromQL للقمع

يجب على المشغلين الاتصال بمساعدي PromQL التاليين؛ التسميات في لوحة التحكم الرئيسية Grafana (`dashboards/grafana/soranet_privacy_metrics.json`) ومدير التنبيه المناسب:

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

استخدم دعمًا قويًا للتأكد من أن الإحصائيات **"Suppressed Bucket %"** لا توفر أي ميزانية محدودة؛ قم بضم الكاشف إلى Alertmanager لإلغاء القفل بشكل أفضل عندما يتم إغلاق جهاز تشيسلو بشكل غير طبيعي.

### CLI للجرافات غير المتصلة بالإنترنت

تم توفير مساحة العمل `cargo xtask soranet-privacy-report` لـ NDJSON-Vыгruuzok. قم بترحيل واحد أو عدد قليل من تتابعات تصدير المسؤول:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```تم تطوير المساعدة من خلال `SoranetSecureAggregator`، وقم بإلغاء قمع التيار في stdout، ومن خلال كتابة الرسالة الهيكلية JSON التي تم إنشاؤها من خلاله `--json-out <path|->`. من خلال تعلم هذه الأجهزة والمجمع المباشر (`--bucket-secs`، `--min-contributors`، `--expected-shares` وما إلى ذلك)، يدعم المشغل إثارة الرغبات التاريخية من خلال تجارب أخرى في حادثة ما. استخدم JSON باستخدام الشاشة Grafana لمتابعة SNNet-8 من خلال قمع التحليلات.

### قائمة الاختيار الجلسة التلقائية الأولى

يجب أن يؤكد الحكام الذين يتمتعون بإدارة أفضل أن أتمتة جلساتهم كانت في السابق من خلال قمع الميزانية. يبدأ المساعد في استخدام `--max-suppression-ratio <0-1>`، بحيث يمكن لـ CI أو المشغلين أن يغلقوا الشبكة عندما يتم تثبيت الدلاء المكبوتة مسبقًا انقر فوق (أكثر من 10%) أو عندما يتم إخراج الدلاء. الطريقة الموصى بها:

1. قم بتصدير NDJSON من تتابع نقاط النهاية الإدارية ومنسق السرعة `/v2/soranet/privacy/event|share` إلى `artifacts/sorafs_privacy/<relay>.ndjson`.
2. تقديم المساعدة بسياسة الميزانية:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```الأمر بإلقاء نظرة على الهدوء وإنهاء العمل باستخدام كود صغير عندما تكون الميزانية السابقة ** أو ** عندما لا تكون هناك دلاء هذه إشارة إلى أن أجهزة القياس عن بعد لم يتم إنتاجها للمضي قدمًا. توضح المقاييس المباشرة أن `soranet_privacy_pending_collectors` يتدفق إلى الصفر، وأن `soranet_privacy_snapshot_suppression_ratio` لا يزال يحتفظ بهذه الميزانية أو الميزانية في الوقت المناسب تقدم.
3. قم بأرشفة إصدار JSON وسجل CLI مباشرة باستخدام حزمة SNNet-8 الخاصة بوسائل النقل المحظورة. يمكن للمسجلين اكتشاف قطع أثرية رائعة.

## الخطوات التالية (SNNet-8a)

- دمج جامعي Prio المزدوجين، بما في ذلك المشاركات الأولية في وقت التشغيل، من أجل المرحلات والمجمعات التي يتم شحنها من الحمولات النافعة `SoranetPrivacyBucketMetricsV1`. *(جوتوفو - سم.`ingest_privacy_payload` في `crates/sorafs_orchestrator/src/lib.rs` واختبارات الطاقة.)*
- نشر لوحة معلومات Prometheus العامة JSON والتنبيهات الصحيحة وسد الفجوات وهواة جمع البيانات وإخفاء الهوية. *(جوتوفو — سم.`dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` والتحقق.)*
- إضافة عيارات مصنوعة من عيار الخصوصية التفاضلية، الموضحة في `privacy_metrics_dp.md`، بما في ذلك كتل الطاقة والحوكمة هضم. *(Gotovo - يتم إنشاء الكمبيوتر الدفتري والمصنوعات اليدوية `scripts/telemetry/run_privacy_dp.py`؛ مجمّع CI `scripts/telemetry/run_privacy_dp_notebook.sh` يستخدم دفتر الملاحظات من خلال سير العمل `.github/workflows/release-pipeline.yml`؛ الملخص محفوظ في `docs/source/status/soranet_privacy_dp_digest.md`.)*يتم توفير الإصدار الحالي من SNNet-8 الأساسي: تحديد أجهزة قياس عن بعد خاصة وآمنة يتم تضمينها بشكل أساسي إلى جهاز الكمبيوتر ولوحة القيادة Prometheus. عيارات يدوية ذات خصوصية تفاضلية للمكان، وسير عمل موثوق به يدعم دفتر الملاحظات الفعلي، ويتخلص منه تم إنشاء عملية المراقبة أولاً بواسطة الأتمتة والتحليلات الشاملة للتنبيهات.