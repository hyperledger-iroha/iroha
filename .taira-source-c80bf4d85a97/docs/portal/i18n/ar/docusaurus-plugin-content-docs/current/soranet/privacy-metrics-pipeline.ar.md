---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/privacy-metrics-pipeline.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خط أنابيب مقاييس الخصوصية
العنوان: مسار معايير الخصوصية في SoraNet (SNNet-8)
Sidebar_label: مسار معايير الخصوصية
الوصف: جمع القياس عن بعد مع ضمان الخصوصية لرحلـات SoraNet و Orchestrators.
---

:::ملاحظة المصدر القياسي
احترام `docs/source/soranet/privacy_metrics_pipeline.md`. حافظ على النسختين متطابقتين حتى يتم التقاعد من مجموعة الوثائق القديمة.
:::

# مسار معايير الخصوصية في SoraNet

يقدم SNNet-8 سطح القياس عن بعد واعي بالخصوصية لبيئة تشغيل التتابع. يقوم بالتتابع الآن بتجميع احداث المصافحة والدائرة في الدلاء بحجم دقيقة ويصدر فقط عدادات Prometheus الخشنة، خصوصية على عدم ربط النقاط بينما تتيح إمكانية التشغيل لرؤية العمل.

## نظرة عامة على المجمع- إطلاق بيئة التشغيل الموجودة في `tools/soranet-relay/src/privacy.rs` تحت `PrivacyAggregator`.
- يتم تشغيل دلاء فهرسة بدقيقة وقت الجدار (`bucket_secs`، الافتراضي 60 ثانية) وتخزينها في حلقة محدودة (`max_completed_buckets`، الافتراضي 120). لأن الأسهم الخاصة بالـ Collectors بمتأخرات محدودة (`max_share_lag_buckets`، افتراضي 12) بحيث يتم تفريغ نافذة Prio القديمة كباكيتس مقمعة بدائل من الممكن الذاكرة او اخفاء جامعي العقين.
- `RelayConfig::privacy` يتوافق مباشرة مع `PrivacyConfig` ويعرض مفاتيح الضبط (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`، `expected_shares`). بيئة التشغيل الإنتاجية لا مؤقتة بينما يدخل SNNet-8a اعتبات التجميع الآمن.
- مكونات runtime الاحداث عبر helpers مثل: `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`, `record_verified_bytes`, و `record_gar_category`.

## نقطة admin للـ Relayيمكن للمشغلين استطلاع رأي المشرف للـ تتابع من اجل تصفح الصور عبر `GET /privacy/events`. لا يوجد قضية JSON محددة الاسطر (`application/x-ndjson`) تحتوي على حمولة `SoranetPrivacyEventV1` منعكسة من `PrivacyEventBuffer` الداخلي. مخزنة أحدث الاحداث حتى `privacy.event_buffer_capacity` التدفقا (الافتراضي 4096) وهي تفريغه عند القراءة، لذا يجب على الكاشطات بما يكفي لتفادي الفجوات. تغطي الاحداث نفس اشارات المصافحة و الخانق و النطاق الترددي الذي تم التحقق منه و الدائرة النشطة و GAR التي تغذي عدادات Prometheus، مما يسمح لـ جامعي في المصب بأرشفة breadcrumbs أمنية للخصوصية او تغذية سير العمل الرسمي.

##إعدادات التتابع

قم بضبط إيقاعات القياس عن بعد الخصوصية في ملف إعدادات التتابع عبر قسم `privacy`:

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

القيم المخصصة للعبقول تطابق مواصفات SNNet-8 والتحقق منها عند التحميل:| الحقل | الوصف | افتراضي |
|-------|-------|-----------|
| `bucket_secs` | عرض كل نافذة تجميع (ثوان). | `60` |
| `min_handshakes` | الحد من الادنى لعدد من قبل ان يطلق دلو عدد من. | `12` |
| `flush_delay_buckets` | عدد الدلاء المكتملة التي تنتظرها قبل محاولة التفريغ. | `1` |
| `force_flush_buckets` | العمر الاقصى قبل اصدار الجرافة المكبوتة. | `6` |
| `max_completed_buckets` | الدلاء المتأخرة المحتفظ بها (تمنع ذاكرة غير محدودة). | `120` |
| `max_share_lag_buckets` | نافذة الإصلاح لـ أسهم المجمع قبل القمع. | `12` |
| `expected_shares` | عدد مشاركات جامعي Prio المطلوبة قبل الدمج. | `2` |
| `event_buffer_capacity` | متأخرات احداث NDJSON لتدفق المشرف. | `4096` |

ضبط `force_flush_buckets` أقل من `flush_delay_buckets`، أو تصفير العتبات، او جونسون إصلاح يفشل في التحقق الآن من عمليات نشر قد تسرب القياس عن بعد لكل تتابع.

حد `event_buffer_capacity` يقيد أيضًا `/admin/privacy/events`، مما يضمن عدم قدرة الكاشطات من التاخر بلا نهاية.

## أسهم جامع بريوينشر جامعي SNNet-8a مزدوجين يصدرون دلاء Prio ذات مشاركة سرية. يقوم الآن بتحليل تدفق `/privacy/events` NDJSON لا إدخالات `SoranetPrivacyEventV1` و share `SoranetPrivacyPrioShareV1`، ويمرها إلى `SoranetSecureAggregator::ingest_prio_share`. السيطرة على الدلاء بمجرد الوصول `PrivacyBucketConfig::expected_shares` مساهمات، بما في ذلك التحكم في التتابع. يتم التحقق من أسهم لمواءمة دلو وشكل الرسم البياني لدمجها من قبل في `SoranetPrivacyBucketMetricsV1`. اذا أمكن عدد المصافحة المدمجة عن `min_contributors`، يتم تصدير الجرافة كـ `suppressed` بما في ذلك يعكس التحكم في المجمع داخل التتابع. ونتيجة لذلك تم قمع التسجيل `suppression_reason` حتى يبدأون في التدريب بين `insufficient_contributors` و`collector_suppressed` و`collector_window_elapsed` و`forced_flush_window_elapsed` عند تشخيص فجوات القياس عن بعد. سبب `collector_window_elapsed` يطلق أيضا عندما تستمر أسهم Prio لما بعد `max_share_lag_buckets`، مما يجعل جامعي العالقين واضحين دون ترك مجمعات قديمة في الذاكرة.

## نقاط إدخال Torii

يعرض Torii الآن نقطتي HTTP محميتين بالـ القياس عن بعد بحيث يمكن للمرحلات والمجمعات تسارع إلى ملاحظة دون مشاركة نقل مخصص:- `POST /v1/soranet/privacy/event` يقبل بصمة `RecordSoranetPrivacyEventDto`. يلف الجسم `SoranetPrivacyEventV1` مع تسمية `source` اختيارية. صحيح أن Torii من طلب مقابل ملف القياس عن بعد لتحقيق الحدث، ويرد بـ HTTP `202 Accepted` مع مغلف Norito JSON يحتوي على حساب النافذة (`bucket_start_unix`, `bucket_duration_secs`) Relay.
- `POST /v1/soranet/privacy/share` يقبل بصمة `RecordSoranetPrivacyShareDto`. عصر الجسم `SoranetPrivacyPrioShareV1` وتلميح `forwarded_by` اختياري حتى يتمكن من البدء من تدقيق تدفقات المجمعات. الطلبات الناجحة HTTP `202 Accepted` مع المغلف Norito JSON يلخص جامع ونافذة الجرافة وتلميح القمع؛ بينما يتم ربط اخفاقات التحقق من الاستجابة للقياس عن بعد من النوع `Conversion` ضد اخطاء حتمية عبر المجمعات. تقوم الآن بإصدار هذه المشاركات عند استطلاع Relays، لتحافظ على تزامن مجمع Prio في Torii مع دلاء على التتابع.

تحترم النقطتان ملف القياس عن بعد: `503 Service Unavailable` عندما تكون المقاييس معطلة. يمكن إرسال اجسام Norito إلى (`application/x.norito`) أو Norito JSON (`application/x.norito+json`)، ويتعين علينا التفاوض بشأن التعليمات البرمجية لتوجيهات Torii القياسية.

## معايير Prometheus

عصر كل دلو مصدّر تسميات `mode` (`entry`, `middle`, `exit`) و `bucket_start`. يتم إصدار عائلات المعايير التالية:| متري | الوصف |
|--------|------------|
| `soranet_privacy_circuit_events_total{kind}` | تصنيف المصافحة مع `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | عدادات الخانق مع `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | مدد Cooldown المجمعة الفرنسية من المصافحة خنق. |
| `soranet_privacy_verified_bytes_total` | عرض النطاق الترددي محققة من اثباتات قياس معماة. |
| `soranet_privacy_active_circuits_{avg,max}` | التصنيف والذروة للدوائر العضوية لكل دلو. |
| `soranet_privacy_rtt_millis{percentile}` | تصنيفات المئينات لـ RTT (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | أعدادات تقرير إجراءات الحوكمة المجزأة حسب الملخص الفئة. |
| `soranet_privacy_bucket_suppressed` | الدلاء المحجوبة لتنبيه لتتغير. |
| `soranet_privacy_pending_collectors{mode}` | مجمعات جامع المشاركات المعلقة قبل المدمجة، مجمعة حسب وضع التتابع. |
| `soranet_privacy_suppression_total{reason}` | عدادات مقموعة مع `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` حتى تنسب اللوحات فجوات الخصوصية. |
| `soranet_privacy_snapshot_suppression_ratio` | نسبة المكبوتة/المصفاة المتأخرة (0-1)، مفيدة لميزانيات التنبيه. |
| `soranet_privacy_last_poll_unixtime` | طابع UNIX النهائي للبحث الناجح (يغذي تنبيه Collector-idle). |
| `soranet_privacy_collector_enabled` | يتحول المقياس إلى `0` عندما يعطل المجمع الخصوصية أو يفشل في البدء (يغذي تنبيه المجمع معطل). |
| `soranet_privacy_poll_errors_total{provider}` | اخفاقات الشبكة مجمعة حسب الاسم المستعار Relay (تزداد عند اخطاء فك الترميز، او اخفاقات HTTP، او اكوادحالة غير المتوقعة). |

لا تزال دلاء بدون ملاحظات مكتوبة، مما تبقى من اللوحات المميزة دون توليد نوافذ صفرية مصطنعة.

## اتجاهات التشغيل1. **لوحات المعلومات** - رسم المقاييس اعلاه مجمعة حسب `mode` و `window_start`. برز نقرة ثانية لا الظاهرة مشاكل جامع او تتابع. استخدم `soranet_privacy_suppression_total{reason}` لتمييز سلبيات عن القمع بسبب الـcollectors عند فرز الفجوات. يشحن اصل Grafana الآن لوحة **"Suppressed Bucket %"** الذي يحسب `sum(soranet_privacy_bucket_suppressed) / count(...)` لكل اختيار حتى يبدأون من ضبط تجاوزات الميزانية بسرعة. سلسلة **"Collector Share Backlog"** (`soranet_privacy_pending_collectors`) و stat **"Snapshot Suppression Ratio"** تبرز جامعي العالقين وانجراف أثناء التشغيل التشغيلي.
2. **التنبيه** - قم بتوجيه الانذارات من إعدادات آمنة للخصوصية: ارتفاعات الرفض PoW، تواتر Cooldown، انجراف RTT، ورفض السعة. بما ان العدادات أحادية الأخطاء داخل كل دلو، تعمل بقواعد المعدلات الصغيرة بشكل جيد.
3. **الاستجابة** - المكان على البيانات المجمعة اولا. عند الحاجة إلى تصحيح عمق، اطلب من المرحلات إعادة لقطات تشغيل الدلاء أو فحص إثباتات القياس المماة بدلات من جمع سجلات حركة الخام.
4. **الاحتفاظ** - سحب البيانات بما في ذلك باستثناء تجاوز `max_completed_buckets`. يجب على المصدرين تصنيف مخرجات Prometheus المصدر القياسي وحذف الدلاء محليًا بعد TBTB.

## تحليلات القمع والتشغيليعتمد اعتماد SNNet-8 على اثبات ان المجمعين قويين يبقون بحالة جيدة وان القمع موجود ضمن حدود السياسة (≥10% من الدلاء لكل تتابع عبر اي نافذة 30 دقيقة). الادوات الضرورية لحرب هذه البوابة تشحن الآن بالشجرة؛ يجب على المشغلين ربطها بطقوسهم الأسبوعية. احترام لوحات القمع الجديدة في Grafana مقتطفات PromQL ادناه، مما يسمح بفرق المناوبة رؤية مباشرة قبل مؤثرة الى استعلامات مميزة.

### وصفات PromQL لمراجعةقمع

يجب على تشغيل مساعدي بقاء PromQL التالي؛ مشهورة في لوحة Grafana المشتركة (`dashboards/grafana/soranet_privacy_metrics.json`) وقواعد Alertmanager:

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

استخدم اختلافات نسبة للتأكيد من ان stat **"Suppressed Bucket %"** يبقى تحت حسب الضرورة؛ تتبع كاشف الارتفاعات بـ Alertmanager للحصول على شعار سريع عند إجراء تخفيض عدد بشكل غير متوقع.

### اداة تقرير جرافات خارجية

توفر مساحة العمل الأمر `cargo xtask soranet-privacy-report` NDJSON لمرة واحدة. وجهه الى واحد او اكثر من الصادرات المشرف للـ Relay:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

يمرر المساعد الالتقاط عبر `SoranetSecureAggregator`، ويطبع ملخص القمع الى stdout، ويكتب اختياريا تقرير JSON منظم عبر `--json-out <path|->`. يحترم تشغيل مفاتيح المجمع مباشرة (`--bucket-secs`, `--min-contributors`, `--expected-shares`, الخ)، مما يسمح للمشغلين باعادة الالتقاطات التاريخية بعتبات مختلفة عند فرز فرزة. ارفق JSON مع لقطات Grafana حتى يبقى حاجز تحليلات قمع لـ SNNet-8 قابلًا للتدقيق.

### قائمة تدقيق التشغيل الاوللا يزال لا يتطلب الأمر اثبات ان أولا آلي لى القمع. تقبل الااداة الآن `--max-suppression-ratio <0-1>` بحيث يمكن لـ CI او يبدأون بسرعة عندما تتجاوز الدلاء المقموعة النافذة المخصصة لها (الافتراضي 10%) او عندما لا توجد دلاء بعد. راكب به:

1. صدّر NDJSON من نقاط المشرف للـ Relay بالإضافة إلى التدفق `/v1/soranet/privacy/event|share` للـ Orchestrator إلى `artifacts/sorafs_privacy/<relay>.ndjson`.
2. مهمة المساعدة في التكاليف السياسية:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   يطبع الأمر نسبة المرصودة ويخرج برمز غير صفري عند تجاوز الميزانية **او** عندما لا تكون دلاء جاهزة بعد، ودعا الى ان القياس عن بعد لا تنتج بعد التشغيل. يجب ان تحدد المقاييس ان `soranet_privacy_pending_collectors` تتجه للصفر وان `soranet_privacy_snapshot_suppression_ratio` يبقى تحت نفس ميزانية التشغيل.
3. تم اكتشاف اختراعات JSON CLI مع حزمة SNNet-8 المعادلة قبل محرك النقل الافتراضي حتى ابتكرت مراجع لإعادة تشغيل نفس القطع.

## الخطوات التالية (SNNet-8a)- دمج مُجمعات Prio الثنائية وربط ادخال المشاركات في وقت التشغيل حتى تأثر المرحلات والمجمعات هامولات `SoranetPrivacyBucketMetricsV1` متسقة. *(تم — راجع `ingest_privacy_payload` في `crates/sorafs_orchestrator/src/lib.rs` والاختبارات المصاحبة.)*
- نشر لوحة Prometheus المشتركة وقواعد التنبيه التي تغطي فجوات قمع صحفة المجمعات و تراجع اخفاء الهوية. *(تم — راجع `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` للتحقق)*
- إنتاج محتويات قابلة لإعادة التعبئة التفاضلية الموضحة في `privacy_metrics_dp.md` بما في ذلك دفاتر لاعادة الانتاج وملخصات التلاعب. *(تم — تم توليد الدفتر شارة بواسطة `scripts/telemetry/run_privacy_dp.py`؛ ويقوم غلاف CI `scripts/telemetry/run_privacy_dp_notebook.sh` ويسمح الدفتر عبر مسار العمل `.github/workflows/release-pipeline.yml`؛ تم حفظ ملخص التكامل في `docs/source/status/soranet_privacy_dp_digest.md`.)*

يقدم الاصدار الحالي الأساسي SNNet-8: القياس عن بعد حتمية وآمن للخصوصية ويتصل مباشرة بـ الكاشطات ولوحات المعلومات Prometheus الحالية. محتوى تجديد الخصوصية التفاضلية في مكانها، ومسار عمل الإصدار يظل محافظًا على مخرجات الدفتر الجديد، ويقلل من اختيار أول تشغيل آلي لتحليلات تنبيهات القمع.