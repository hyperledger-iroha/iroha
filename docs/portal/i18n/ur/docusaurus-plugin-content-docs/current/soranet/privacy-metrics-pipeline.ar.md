---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/privacy-metrics-pipeline.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: privacy-metrics-pipeline
title: مسار مقاييس الخصوصية في SoraNet (SNNet-8)
sidebar_label: مسار مقاييس الخصوصية
description: جمع telemetry مع الحفاظ على الخصوصية لمرحلـات SoraNet و orchestrators.
---

:::note المصدر القياسي
تعكس `docs/source/soranet/privacy_metrics_pipeline.md`. حافظ على النسختين متطابقتين حتى يتم تقاعد مجموعة الوثائق القديمة.
:::

# مسار مقاييس الخصوصية في SoraNet

يقدم SNNet-8 سطح telemetry واعي بالخصوصية لبيئة تشغيل relay. يقوم relay الآن بتجميع احداث handshake و circuit في buckets بحجم دقيقة ويصدر فقط عدادات Prometheus الخشنة، محافظا على عدم ربط الدوائر الفردية بينما يمنح المشغلين رؤية قابلة للعمل.

## نظرة عامة على المجمع

- تنفيذ بيئة التشغيل موجود في `tools/soranet-relay/src/privacy.rs` تحت `PrivacyAggregator`.
- يتم فهرسة buckets بدقيقة وقت الجدار (`bucket_secs`، الافتراضي 60 ثانية) وتخزينها في حلقة محدودة (`max_completed_buckets`، الافتراضي 120). تحتفظ shares الخاصة بالـ collectors بمتأخرات محدودة (`max_share_lag_buckets`، الافتراضي 12) بحيث يتم تفريغ نوافذ Prio القديمة كبuckets suppressed بدلا من تسريب الذاكرة او اخفاء collectors العالقين.
- `RelayConfig::privacy` يطابق مباشرة `PrivacyConfig` ويعرض مفاتيح الضبط (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`). بيئة التشغيل الانتاجية تبقي القيم الافتراضية بينما يقدم SNNet-8a عتبات تجميع امن.
- تسجل وحدات runtime الاحداث عبر helpers مثل: `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`, `record_verified_bytes`, و `record_gar_category`.

## نقطة admin للـ relay

يمكن للمشغلين استطلاع مستمع admin للـ relay من اجل ملاحظات خام عبر `GET /privacy/events`. تعيد النقطة JSON محدد الاسطر (`application/x-ndjson`) يحتوي على حمولة `SoranetPrivacyEventV1` منعكسة من `PrivacyEventBuffer` الداخلي. يحتفظ المخزن بأحدث الاحداث حتى `privacy.event_buffer_capacity` إدخالا (الافتراضي 4096) ويتم تفريغه عند القراءة، لذا يجب على scrapers الاستطلاع بما يكفي لتفادي الفجوات. تغطي الاحداث نفس اشارات handshake و throttle و verified bandwidth و active circuit و GAR التي تغذي عدادات Prometheus، مما يسمح للـ collectors في المصب بأرشفة breadcrumbs امنة للخصوصية او تغذية سير عمل التجميع الامن.

## اعدادات relay

يضبط المشغلون إيقاعات telemetry الخصوصية في ملف اعدادات relay عبر قسم `privacy`:

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

القيم الافتراضية للحقول تطابق مواصفات SNNet-8 ويتم التحقق منها عند التحميل:

| الحقل | الوصف | الافتراضي |
|-------|-------|-----------|
| `bucket_secs` | عرض كل نافذة تجميع (ثوان). | `60` |
| `min_handshakes` | الحد الادنى لعدد المساهمين قبل ان يطلق bucket عدادات. | `12` |
| `flush_delay_buckets` | عدد buckets المكتملة التي ننتظرها قبل محاولة التفريغ. | `1` |
| `force_flush_buckets` | العمر الاقصى قبل اصدار bucket suppressed. | `6` |
| `max_completed_buckets` | متأخرات buckets المحتفظ بها (تمنع ذاكرة غير محدودة). | `120` |
| `max_share_lag_buckets` | نافذة الاحتفاظ لـ collector shares قبل suppression. | `12` |
| `expected_shares` | عدد Prio collector shares المطلوبة قبل الدمج. | `2` |
| `event_buffer_capacity` | متأخرات احداث NDJSON لتدفق admin. | `4096` |

ضبط `force_flush_buckets` اقل من `flush_delay_buckets`، او تصفير العتبات، او تعطيل حارس الاحتفاظ يفشل التحقق الآن لتجنب عمليات نشر قد تسرب telemetry لكل relay.

حد `event_buffer_capacity` يقيد ايضا `/admin/privacy/events`، مما يضمن عدم تمكن scrapers من التاخر بلا نهاية.

## Prio collector shares

ينشر SNNet-8a collectors مزدوجين يصدرون buckets Prio ذات مشاركة سرية. يقوم orchestrator الآن بتحليل تدفق `/privacy/events` NDJSON لادخالات `SoranetPrivacyEventV1` و shares `SoranetPrivacyPrioShareV1`، ويمررها الى `SoranetSecureAggregator::ingest_prio_share`. تصدر buckets بمجرد وصول `PrivacyBucketConfig::expected_shares` مساهمات، بما يعكس سلوك relay. يتم التحقق من shares لمواءمة bucket وشكل histogram قبل دمجها في `SoranetPrivacyBucketMetricsV1`. اذا انخفض عدد handshake المدمج عن `min_contributors`، يتم تصدير bucket كـ `suppressed` بما يعكس سلوك المجمع داخل relay. تصدر النوافذ suppressed الآن تسمية `suppression_reason` حتى يتمكن المشغلون من التمييز بين `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed`, و `forced_flush_window_elapsed` عند تشخيص فجوات telemetry. سبب `collector_window_elapsed` يطلق ايضا عندما تبقى Prio shares لما بعد `max_share_lag_buckets`، مما يجعل collectors العالقين مرئيين دون ترك مجمعات قديمة في الذاكرة.

## نقاط ادخال Torii

يعرض Torii الآن نقطتي HTTP محميتين بالـ telemetry بحيث يمكن للـ relays و collectors تمرير الملاحظات بدون تضمين نقل مخصص:

- `POST /v1/soranet/privacy/event` يقبل حمولة `RecordSoranetPrivacyEventDto`. يلف الجسم `SoranetPrivacyEventV1` مع تسمية `source` اختيارية. يتحقق Torii من الطلب مقابل ملف telemetry النشط، يسجل الحدث، ويرد بـ HTTP `202 Accepted` مع مغلف Norito JSON يحتوي على نافذة الحساب (`bucket_start_unix`, `bucket_duration_secs`) ووضع relay.
- `POST /v1/soranet/privacy/share` يقبل حمولة `RecordSoranetPrivacyShareDto`. يحمل الجسم `SoranetPrivacyPrioShareV1` وتلميح `forwarded_by` اختياري حتى يتمكن المشغلون من تدقيق تدفقات collectors. تعيد الطلبات الناجحة HTTP `202 Accepted` مع مغلف Norito JSON يلخص collector ونافذة bucket وتلميح suppression؛ بينما يتم ربط اخفاقات التحقق باستجابة telemetry من نوع `Conversion` للحفاظ على معالجة اخطاء حتمية عبر collectors. تقوم حلقة احداث orchestrator الآن باصدار هذه shares عند استطلاع relays، لتحافظ على تزامن مجمع Prio في Torii مع buckets على relay.

تحترم النقطتان ملف telemetry: تصدران `503 Service Unavailable` عندما تكون المقاييس معطلة. يمكن للعملاء ارسال اجسام Norito ثنائية (`application/x.norito`) او Norito JSON (`application/x.norito+json`)، ويتفاوض الخادم تلقائيا على الصيغة عبر مستخلصات Torii القياسية.

## مقاييس Prometheus

يحمل كل bucket مصدّر تسميات `mode` (`entry`, `middle`, `exit`) و `bucket_start`. يتم اصدار عائلات المقاييس التالية:

| Metric | Description |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | تصنيف handshake مع `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | عدادات throttle مع `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | مدد cooldown المجمعة المقدمة من handshakes throttled. |
| `soranet_privacy_verified_bytes_total` | bandwidth محققة من اثباتات قياس معماة. |
| `soranet_privacy_active_circuits_{avg,max}` | المتوسط والذروة للدوائر النشطة لكل bucket. |
| `soranet_privacy_rtt_millis{percentile}` | تقديرات المئينات لـ RTT (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | عدادات Governance Action Report المجزأة حسب digest الفئة. |
| `soranet_privacy_bucket_suppressed` | buckets المحجوبة لان عتبة المساهمين لم تتحقق. |
| `soranet_privacy_pending_collectors{mode}` | مجمعات collector shares المعلقة قبل الدمج، مجمعة حسب وضع relay. |
| `soranet_privacy_suppression_total{reason}` | عدادات buckets suppressed مع `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` حتى تنسب اللوحات فجوات الخصوصية. |
| `soranet_privacy_snapshot_suppression_ratio` | نسبة suppressed/المصفاة لآخر drain (0-1)، مفيدة لميزانيات التنبيه. |
| `soranet_privacy_last_poll_unixtime` | طابع UNIX لآخر استطلاع ناجح (يغذي تنبيه collector-idle). |
| `soranet_privacy_collector_enabled` | gauge يتحول الى `0` عندما يتعطل collector الخصوصية او يفشل بالبدء (يغذي تنبيه collector-disabled). |
| `soranet_privacy_poll_errors_total{provider}` | اخفاقات الاستطلاع مجمعة حسب alias relay (تزداد عند اخطاء فك الترميز، او اخفاقات HTTP، او اكواد حالة غير متوقعة). |

تبقى buckets بدون ملاحظات صامتة، مما يبقي اللوحات مرتبة دون توليد نوافذ صفرية مصطنعة.

## ارشادات التشغيل

1. **لوحات المعلومات** - ارسم المقاييس اعلاه مجمعة حسب `mode` و `window_start`. ابرز النوافذ المفقودة لاظهار مشاكل collector او relay. استخدم `soranet_privacy_suppression_total{reason}` لتمييز نقص المساهمين عن suppression المدفوعة بالـ collectors عند فرز الفجوات. يشحن اصل Grafana الآن لوحة **"Suppression Reasons (5m)"** مخصصة تغذيها تلك العدادات، بالاضافة الى stat **"Suppressed Bucket %"** الذي يحسب `sum(soranet_privacy_bucket_suppressed) / count(...)` لكل اختيار حتى يتمكن المشغلون من رصد تجاوزات الميزانية بسرعة. سلسلة **"Collector Share Backlog"** (`soranet_privacy_pending_collectors`) و stat **"Snapshot Suppression Ratio"** تبرز collectors العالقين وانجراف الميزانية اثناء التشغيل الآلي.
2. **التنبيه** - قم بقيادة الانذارات من عدادات آمنة للخصوصية: ارتفاعات رفض PoW، تواتر cooldown، انجراف RTT، ورفض السعة. بما ان العدادات احادية الاتجاه داخل كل bucket، تعمل قواعد المعدلات البسيطة بشكل جيد.
3. **استجابة الحوادث** - اعتمد على البيانات المجمعة اولا. عند الحاجة لتصحيح اعمق، اطلب من relays اعادة تشغيل لقطات buckets او فحص اثباتات القياس المعماة بدلا من جمع سجلات حركة خام.
4. **الاحتفاظ** - اسحب البيانات بما يكفي لتجنب تجاوز `max_completed_buckets`. ينبغي على exporters اعتبار مخرجات Prometheus المصدر القياسي وحذف buckets المحلية بعد تمريرها.

## تحليلات suppression والتشغيل الآلي

يعتمد قبول SNNet-8 على اثبات ان collectors الآليين يبقون بحالة جيدة وان suppression تبقى ضمن حدود السياسة (≤10% من buckets لكل relay عبر اي نافذة 30 دقيقة). الادوات اللازمة لتلبية هذه البوابة تشحن الآن مع الشجرة؛ يجب على المشغلين ربطها بطقوسهم الاسبوعية. تعكس لوحات suppression الجديدة في Grafana مقتطفات PromQL ادناه، مما يمنح فرق المناوبة رؤية مباشرة قبل اللجوء الى استعلامات يدوية.

### وصفات PromQL لمراجعة suppression

يجب على المشغلين ابقاء مساعدات PromQL التالية قريبة؛ كلاهما مذكور في لوحة Grafana المشتركة (`dashboards/grafana/soranet_privacy_metrics.json`) وقواعد Alertmanager:

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

استخدم مخرجات النسبة للتاكد من ان stat **"Suppressed Bucket %"** يبقى تحت ميزانية السياسة؛ اربط كاشف الارتفاعات بـ Alertmanager للحصول على اشعار سريع عند انخفاض عدد المساهمين بشكل غير متوقع.

### اداة تقرير buckets خارجية

يوفر workspace الامر `cargo xtask soranet-privacy-report` لالتقاطات NDJSON لمرة واحدة. وجهه الى واحد او اكثر من exports admin للـ relay:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

يمرر المساعد الالتقاط عبر `SoranetSecureAggregator`، ويطبع ملخص suppression الى stdout، ويكتب اختياريا تقرير JSON منظم عبر `--json-out <path|->`. يحترم نفس مفاتيح collector المباشر (`--bucket-secs`, `--min-contributors`, `--expected-shares`, الخ)، مما يسمح للمشغلين باعادة تشغيل الالتقاطات التاريخية بعتبات مختلفة عند فرز حادثة. ارفق JSON مع لقطات Grafana حتى يبقى حاجز تحليلات suppression لـ SNNet-8 قابلًا للتدقيق.

### قائمة تدقيق التشغيل الآلي الاول

لا تزال الحوكمة تتطلب اثبات ان اول تشغيل آلي لبى ميزانية suppression. تقبل الاداة الآن `--max-suppression-ratio <0-1>` بحيث يمكن لـ CI او المشغلين الفشل بسرعة عندما تتجاوز buckets suppressed النافذة المسموح بها (الافتراضي 10%) او عندما لا توجد buckets بعد. التدفق الموصى به:

1. صدّر NDJSON من نقاط admin للـ relay بالاضافة الى تدفق `/v1/soranet/privacy/event|share` للـ orchestrator الى `artifacts/sorafs_privacy/<relay>.ndjson`.
2. شغل المساعد مع ميزانية السياسة:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   يطبع الامر النسبة المرصودة ويخرج برمز غير صفري عند تجاوز الميزانية **او** عندما لا تكون buckets جاهزة بعد، مشيرا الى ان telemetry لم تُنتج بعد لهذا التشغيل. يجب ان تُظهر المقاييس المباشرة ان `soranet_privacy_pending_collectors` تتجه للصفر وان `soranet_privacy_snapshot_suppression_ratio` يبقى تحت نفس الميزانية اثناء التشغيل.
3. ارشف مخرجات JSON وسجل CLI مع حزمة ادلة SNNet-8 قبل تبديل النقل الافتراضي حتى يتمكن المراجعون من اعادة تشغيل نفس القطع.

## الخطوات التالية (SNNet-8a)

- دمج Prio collectors المزدوجين وربط ادخال shares في runtime حتى تصدر relays و collectors حمولات `SoranetPrivacyBucketMetricsV1` متسقة. *(تم — راجع `ingest_privacy_payload` في `crates/sorafs_orchestrator/src/lib.rs` والاختبارات المصاحبة.)*
- نشر لوحة Prometheus المشتركة وقواعد التنبيه التي تغطي فجوات suppression وصحة collectors وتراجع اخفاء الهوية. *(تم — راجع `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` وملفات التحقق.)*
- انتاج مواد معايرة الخصوصية التفاضلية الموضحة في `privacy_metrics_dp.md` بما في ذلك دفاتر قابلة لاعادة الانتاج وملخصات الحوكمة. *(تم — تم توليد الدفتر والمواد بواسطة `scripts/telemetry/run_privacy_dp.py`; يقوم غلاف CI `scripts/telemetry/run_privacy_dp_notebook.sh` بتشغيل الدفتر عبر مسار العمل `.github/workflows/release-pipeline.yml`; تم حفظ ملخص الحوكمة في `docs/source/status/soranet_privacy_dp_digest.md`.)*

يقدم الاصدار الحالي اساس SNNet-8: telemetry حتمية وآمنة للخصوصية تتصل مباشرة بـ scrapers و dashboards Prometheus الحالية. مواد معايرة الخصوصية التفاضلية في مكانها، ومسار عمل release يحافظ على مخرجات الدفتر حديثة، والعمل المتبقي يركز على مراقبة اول تشغيل آلي وتوسيع تحليلات تنبيهات suppression.
