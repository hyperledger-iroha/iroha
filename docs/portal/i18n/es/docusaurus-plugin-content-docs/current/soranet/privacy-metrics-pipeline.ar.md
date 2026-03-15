---
lang: es
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: canalización-de-métricas-de-privacidad
título: مسار مقاييس الخصوصية في SoraNet (SNNet-8)
sidebar_label: مسار مقاييس الخصوصية
descripción: جمع telemetría مع الحفاظ على الخصوصية لمرحلـات SoraNet y orquestadores.
---

:::nota المصدر القياسي
Nombre `docs/source/soranet/privacy_metrics_pipeline.md`. حافظ على النسختين متطابقتين حتى يتم تقاعد مجموعة الوثائق القديمة.
:::

# مسار مقاييس الخصوصية في SoraNet

يقدم SNNet-8 سطح telemetría y بالخصوصية لبيئة تشغيل relé. يقوم relé الآن بتجميع احداث apretón de manos y circuito في cubos بحجم دقيقة ويصدر فقط عدادات Prometheus الخشنة، محافظا على عدم ربط الدوائر الفردية بينما يمنح المشغلين رؤية قابلة للعمل.

## نظرة عامة على المجمع- Haga clic en el botón `tools/soranet-relay/src/privacy.rs` y en `PrivacyAggregator`.
- يتم فهرسة cucharones بدقيقة وقت الجدار (`bucket_secs`، الافتراضي 60 ثانية) وتخزينها في حلقة محدودة (`max_completed_buckets`, الافتراضي 120). تحتفظ acciones الخاصة بالـ coleccionistas بمتأخرات محدودة (`max_share_lag_buckets`، الافتراضي 12) بحيث يتم تفريغ نوافذ Prio القديمة كبuckets suprimido بدلا من تسريب الذاكرة او اخفاء coleccionistas العالقين.
- `RelayConfig::privacy` يطابق مباشرة `PrivacyConfig` ويعرض مفاتيح الضبط (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`). La configuración del sistema operativo SNNet-8a se realiza mediante el software SNNet-8a.
- Tiempo de ejecución y ayudantes de tiempo de ejecución: `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`, `record_verified_bytes` y `record_gar_category`.

## نقطة admin للـ relevoيمكن للمشغلين استطلاع مستمع admin للـ Relay من اجل ملاحظات خام عبر `GET /privacy/events`. Utilice el archivo JSON (`application/x-ndjson`) y el archivo `SoranetPrivacyEventV1` desde el archivo `PrivacyEventBuffer`. يحتفظ المخزن بأحدث الاحداث حتى `privacy.event_buffer_capacity` إدخالا (الافتراضي 4096) Y يتم تفريغه عند القراءة, لذا يجب على raspadores الاستطلاع بما يكفي لتفادي الفجوات. تغطي الاحداث نفس اشارات apretón de manos, acelerador, ancho de banda verificado, circuito activo y GAR التي تغذي عدادات Prometheus, مما يسمح للـ colectores في المصب بأرشفة Breadcrumbs امنة للخصوصية او تغذية سير عمل التجميع الامن.

## relevo اعدادات

Información sobre telemetría y relé de relé `privacy`:

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

La configuración de SNNet-8 y la configuración de SNNet-8 son:| الحقل | الوصف | الافتراضي |
|-------|-------|-----------|
| `bucket_secs` | عرض كل نافذة تجميع (ثوان). | `60` |
| `min_handshakes` | الحد الادنى لعدد المساهمين قبل ان يطلق cubo عدادات. | `12` |
| `flush_delay_buckets` | عدد cubos المكتملة التي ننتظرها قبل محاولة التفريغ. | `1` |
| `force_flush_buckets` | العمر الاقصى قبل اصدار cucharón suprimido. | `6` |
| `max_completed_buckets` | متأخرات cubos المحتفظ بها (تمنع ذاكرة غير محدودة). | `120` |
| `max_share_lag_buckets` | نافذة الاحتفاظ لـ acciones de coleccionista قبل supresión. | `12` |
| `expected_shares` | عدد Acciones de coleccionista de Prio المطلوبة قبل الدمج. | `2` |
| `event_buffer_capacity` | متأخرات احداث NDJSON لتدفق admin. | `4096` |

ضبط `force_flush_buckets` اقل من `flush_delay_buckets`, او تصفير العتبات، او تعطيل حارس الاحتفاظ يفشل التحقق الآن لتجنب عمليات نشر قد تسرب telemetría لكل relé.

حد `event_buffer_capacity` يقيد ايضا `/admin/privacy/events`, مما يضمن عدم تمكن raspadores من التاخر بلا نهاية.

## Acciones de coleccionista de Prioينشر SNNet-8a colectores مزدوجين يصدرون cubos Prio ذات مشاركة سرية. يقوم orquestador الآن بتحليل تدفق `/privacy/events` NDJSON لادخالات `SoranetPrivacyEventV1` y acciones `SoranetPrivacyPrioShareV1`, ويمررها الى `SoranetSecureAggregator::ingest_prio_share`. تصدر cubos بمجرد وصول `PrivacyBucketConfig::expected_shares` مساهمات، بما يعكس سلوك relé. يتم التحقق من comparte el cubo y el histograma قبل دمجها في `SoranetPrivacyBucketMetricsV1`. اذا انخفض عدد apretón de manos المدمج عن `min_contributors`, يتم تصدير cubo كـ `suppressed` بما يعكس سلوك المجمع داخل relé. تصدر النوافذ suprimido الآن تسمية `suppression_reason` حتى يتمكن المشغلون من التمييز بين `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed` y `forced_flush_window_elapsed` son aplicaciones de telemetría. سبب `collector_window_elapsed` يطلق ايضا عندما تبقى Acciones de Prio لما بعد `max_share_lag_buckets`, مما يجعل coleccionistas العالقين مرئيين دون ترك مجمعات قديمة في الذاكرة.

## نقاط ادخال Torii

Aquí Torii utiliza HTTP para telemetría y relés y colectores que incluyen:- `POST /v2/soranet/privacy/event` يقبل حمولة `RecordSoranetPrivacyEventDto`. Aquí está el `SoranetPrivacyEventV1` y el `source`. Utilice Torii para conectarse a telemetría y conectarse a HTTP `202 Accepted` para conectarse a Norito JSON. يحتوي على نافذة الحساب (`bucket_start_unix`, `bucket_duration_secs`) y relé.
- `POST /v2/soranet/privacy/share` يقبل حمولة `RecordSoranetPrivacyShareDto`. يحمل الجسم `SoranetPrivacyPrioShareV1` y `forwarded_by` اختياري حتى يتمكن المشغلون من تدقيق تدفقات coleccionistas. Utilización de HTTP `202 Accepted` y Norito JSON, colector, depósito y supresión Hay dispositivos de telemetría como `Conversion` para coleccionistas. تقوم حلقة احداث orquestador الآن باصدار هذه acciones عند استطلاع relés, لتحافظ على تزامن مجمع Prio في Torii مع cubos على relevo.

تحترم النقطتان ملف telemetría: تصدران `503 Service Unavailable` عندما تكون المقاييس معطلة. Utilice el software Norito (`application/x.norito`) y Norito JSON (`application/x.norito+json`) y Norito. الخادم تلقائيا على الصيغة عبر مستخلصات Torii القياسية.

## مقاييس Prometheus

Este cubo está equipado con `mode` (`entry`, `middle`, `exit`) y `bucket_start`. يتم اصدار عائلات المقاييس التالية:| Métrica | Descripción |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | Hay un apretón de manos en `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | Utilice el acelerador en `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Se ha acelerado el tiempo de reutilización de los apretones de manos. |
| `soranet_privacy_verified_bytes_total` | ancho de banda محققة من اثباتات قياس معماة. |
| `soranet_privacy_active_circuits_{avg,max}` | المتوسط ​​والذروة للدوائر النشطة لكل cucharón. |
| `soranet_privacy_rtt_millis{percentile}` | Actualizaciones de RTT (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | عدادات Informe de acción de gobernanza المجزأة حسب digest الفئة. |
| `soranet_privacy_bucket_suppressed` | cubos المحجوبة لان عتبة المساهمين لم تتحقق. |
| `soranet_privacy_pending_collectors{mode}` | مجمعات acciones de coleccionista المعلقة قبل الدمج، مجمعة حسب وضع relé. |
| `soranet_privacy_suppression_total{reason}` | عدادات cubos suprimidos مع `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` حتى تنسب اللوحات فجوات الخصوصية. |
| `soranet_privacy_snapshot_suppression_ratio` | نسبة suprimido/المصفاة لآخر drenaje (0-1), مفيدة لميزانيات التنبيه. |
| `soranet_privacy_last_poll_unixtime` | طابع UNIX لآخر استطلاع ناجح (يغذي تنبيه colector inactivo). |
| `soranet_privacy_collector_enabled` | calibre يتحول الى `0` عندما يتعطل colector الخصوصية او يفشل بالبدء (يغذي تنبيه colector deshabilitado). |
| `soranet_privacy_poll_errors_total{provider}` | اخفاقات الاستطلاع مجمعة حسب alias de retransmisión (تزداد عند اخطاء فك الترميز، او اخفاقات HTTP، او اكواد حالة غير متوقعة). |

تبقى cubos بدون ملاحظات صامتة، مما يبقي اللوحات مرتبة دون توليد نوافذ صفرية مصطنعة.

## ارشادات التشغيل1. **لوحات المعلومات** - ارسم المقاييس اعلاه مجمعة حسب `mode` e `window_start`. ابرز النوافذ المفقودة لاظهار مشاكل colector y relé. استخدم `soranet_privacy_suppression_total{reason}` لتمييز نقص المساهمين عن supresión المدفوعة بالـ coleccionistas عند فرز الفجوات. يشحن اصل Grafana الآن لوحة **"Suppression Reasons (5m)"** مخصصة تغذيها تلك العدادات، بالاضافة الى stat **"Suppressed Bucket %"** الذي يحسب `sum(soranet_privacy_bucket_suppressed) / count(...)` Para obtener más información, consulte el manual de instrucciones. سلسلة **"Collector Share Backlog"** (`soranet_privacy_pending_collectors`) y stat **"Snapshot Suppression Ratio"** تبرز Collectors العالقين وانجراف الميزانية اثناء التشغيل الآلي.
2. **التنبيه** - قم بقيادة الانذارات من عدادات آمنة للخصوصية: ارتفاعات رفض PoW, تواتر cooldown, انجراف RTT, y رفض السعة. بما ان العدادات احادية الاتجاه داخل كل cubeta, تعمل قواعد المعدلات البسيطة بشكل جيد.
3. **استجابة الحوادث** - اعتمد على البيانات المجمعة اولا. Ver más حركة خام.
4. **الاحتفاظ** - اسحب البيانات بما يكفي لتجنب تجاوز `max_completed_buckets`. ينبغي على exportadores اعتبار مخرجات Prometheus المصدر القياسي وحذف cubos المحلية بعد تمريرها.

## supresión de تحليلات والتشغيل الآلييعتمد قبول SNNet-8 على اثبات ان colectores الآليين يبقون بحالة جيدة y supresión تبقى ضمن حدود السياسة (≤10% de los cubos لكل relé عبر اي 30 días). الادوات اللازمة لتلبية هذه البوابة تشحن الآن مع الشجرة؛ يجب على المشغلين ربطها بطقوسهم الاسبوعية. تعكس لوحات supresión الجديدة في Grafana مقتطفات PromQL ادناه، مما يمنح فرق المناوبة رؤية مباشرة قبل اللجوء الى استعلامات يدوية.

### وصفات PromQL لمراجعة supresión

يجب على المشغلين ابقاء مساعدات PromQL التالية قريبة؛ كلاهما مذكور في لوحة Grafana المشتركة (`dashboards/grafana/soranet_privacy_metrics.json`) y Alertmanager:

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

### اداة تقرير cubos خارجية

يوفر espacio de trabajo الامر `cargo xtask soranet-privacy-report` لالتقاطات NDJSON لمرة y احدة. Aquí está el administrador de exportaciones del relé:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

يمرر المساعد الالتقاط عبر `SoranetSecureAggregator`, y ملخص supresión de la salida estándar, y ويكتب اختياريا تقرير JSON عبر `--json-out <path|->`. يحترم نفس مفاتيح colector المباشر (`--bucket-secs`, `--min-contributors`, `--expected-shares`, الخ), مما يسمح للمشغلين باعادة تشغيل الالتقاطات التاريخية بعتبات مختلفة عند فرز حادثة. El formato JSON del Grafana es una supresión de datos de SNNet-8.

### قائمة تدقيق التشغيل الآلي الاوللا تزال الحوكمة تتطلب اثبات ان اول تشغيل آلي لبى ميزانية supresión. تقبل الاداة الآن `--max-suppression-ratio <0-1>` بحيث يمكن لـ CI او المشغلين الفشل بسرعة عندما تتجاوز buckets suprimidos النافذة المسموح بها (الافتراضي 10%) او عندما لا توجد cubos بعد. التدفق الموصى به:

1. صدّر NDJSON من نقاط admin للـ Relay بالاضافة الى تدفق `/v2/soranet/privacy/event|share` للـ Orchestrator الى `artifacts/sorafs_privacy/<relay>.ndjson`.
2. شغل المساعد مع ميزانية السياسة:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   يطبع الامر النسبة المرصودة ويخرج برمز غير صفري عند تجاوز الميزانية **او** عندما لا تكون cubos جاهزة بعد، مشيرا الى ان telemetría لم تُنتج بعد لهذا التشغيل. Utilice el conector `soranet_privacy_pending_collectors` y el `soranet_privacy_snapshot_suppression_ratio` para conectar el conector del cable. التشغيل.
3. Utilice JSON y CLI para acceder a SNNet-8 y conecte el dispositivo al servidor. نفس القطع.

## الخطوات التالية (SNNet-8a)- Los coleccionistas Prio comparten y comparten tiempo de ejecución con relés y coleccionistas `SoranetPrivacyBucketMetricsV1`. *(تم — راجع `ingest_privacy_payload` في `crates/sorafs_orchestrator/src/lib.rs` والاختبارات المصاحبة.)*
- نشر لوحة Prometheus المشتركة وقواعد التنبيه التي تغطي فجوات supresión وصحة وصحة وتراجع اخفاء الهوية. *(تم — راجع `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` y ملفات التحقق.)*
- انتاج مواد معايرة الخصوصية التفاضلية الموضحة في `privacy_metrics_dp.md` بما في ذلك دفاتر قابلة لاعادة الانتاج وملخصات الحوكمة. *(تم — تم توليد الدفتر والمواد بواسطة `scripts/telemetry/run_privacy_dp.py`; يقوم غلاف CI `scripts/telemetry/run_privacy_dp_notebook.sh` بتشغيل الدفتر عبر مسار العمل `.github/workflows/release-pipeline.yml`;

SNNet-8: telemetría y paneles de control Prometheus. مواد معايرة الخصوصية التفاضلية في مكانها، ومسار عمل liberación يحافظ على مخرجات الدفتر حديثة، والعمل المتبقي يركز على مراقبة اول تشغيل آلي وتوسيع تحليلات تنبيهات supresión.