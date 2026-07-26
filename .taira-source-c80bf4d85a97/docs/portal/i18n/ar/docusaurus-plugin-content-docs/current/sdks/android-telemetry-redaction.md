---
slug: /sdks/android-telemetry
lang: ar
direction: rtl
source: docs/portal/docs/sdks/android-telemetry-redaction.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: خطة تنقيح قياس عن بُعد Android
sidebar_label: قياس عن بُعد Android
slug: /sdks/android-telemetry
---

:::note مصدر مرجعي
:::

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# خطة تنقيح قياس عن بُعد Android (AND7)

## النطاق

يوثّق هذا المستند سياسة تنقيح قياس عن بُعد المقترحة وأدوات التمكين الخاصة بـ SDK أندرويد كما يتطلب بند خارطة الطريق **AND7**. وهو ينسّق القياس على الأجهزة المحمولة مع خط الأساس لعُقد Rust مع مراعاة ضمانات الخصوصية الخاصة بالجهاز. تُستخدم المخرجات كقراءة مسبقة لمراجعة حوكمة SRE في فبراير 2026.

الأهداف:

- حصر كل إشارة يصدرها Android وتصل إلى منصات الملاحظة المشتركة (تتبعات OpenTelemetry، سجلات Norito المرمّزة، وتصدير المقاييس).
- تصنيف الحقول التي تختلف عن خط الأساس في Rust وتوثيق ضوابط التنقيح أو الاحتفاظ.
- وصف أعمال التمكين والاختبار حتى تستجيب فرق الدعم بشكل حتمي لتنبيهات التنقيح.

## جرد الإشارات (مسودة)

التجهيزات المخططة مجمعة حسب القناة. جميع أسماء الحقول تتبع مخطط قياس عن بُعد لـ SDK Android (`org.hyperledger.iroha.android.telemetry.*`). الحقول الاختيارية مميزة بـ `?`.

| معرف الإشارة | القناة | الحقول الأساسية | تصنيف PII/PHI | التنقيح / الاحتفاظ | الملاحظات |
|-----------|---------|------------|------------------------|-----------------------|-------|
| `android.torii.http.request` | Span تتبع | `authority_hash`, `route`, `status_code`, `latency_ms` | السلطة عامة؛ المسار لا يحتوي أسراراً | إصدار السلطة مُجزأة (`blake2b_256`) قبل التصدير؛ الاحتفاظ 7 أيام | يعكس `torii.http.request` في Rust؛ التجزئة تحفظ خصوصية alias المحمول. |
| `android.torii.http.retry` | حدث | `route`, `retry_count`, `error_code`, `backoff_ms` | لا شيء | بلا تنقيح؛ الاحتفاظ 30 يوماً | يستخدم لتدقيق إعادة المحاولة الحتمي؛ الحقول مطابقة لـ Rust. |
| `android.pending_queue.depth` | مقياس Gauge | `queue_type`, `depth` | لا شيء | بلا تنقيح؛ الاحتفاظ 90 يوماً | يطابق `pipeline.pending_queue_depth` في Rust. |
| `android.keystore.attestation.result` | حدث | `alias_label`, `security_level`, `attestation_digest`, `device_brand_bucket` | Alias (مشتق)، بيانات الجهاز | استبدال alias بوسم حتمي، وتنقيح العلامة إلى bucket enum | مطلوب لجهوزية AND2؛ عُقد Rust لا تُصدر بيانات الجهاز. |
| `android.keystore.attestation.failure` | عدّاد | `alias_label`, `failure_reason` | لا شيء بعد تنقيح alias | بلا تنقيح؛ الاحتفاظ 90 يوماً | يدعم تدريبات chaos؛ `alias_label` مشتق من alias المُجزأ. |
| `android.telemetry.redaction.override` | حدث | `override_id`, `actor_role_masked`, `reason`, `expires_at` | دور الفاعل يعد PII تشغيلياً | تصدير فئة الدور المقنّعة؛ الاحتفاظ 365 يوماً مع سجل تدقيق | غير موجود في Rust؛ يجب تمرير overrides عبر الدعم. |
| `android.telemetry.export.status` | عدّاد | `backend`, `status` | لا شيء | بلا تنقيح؛ الاحتفاظ 30 يوماً | تماثل مع عدّادات حالة المُصدِّر في Rust. |
| `android.telemetry.redaction.failure` | عدّاد | `signal_id`, `reason` | لا شيء | بلا تنقيح؛ الاحتفاظ 30 يوماً | مطلوب لمحاكاة `streaming_privacy_redaction_fail_total` في Rust. |
| `android.telemetry.device_profile` | مقياس Gauge | `profile_id`, `sdk_level`, `hardware_tier` | بيانات الجهاز | إصدار buckets تقريبية (SDK major, hardware tier)؛ الاحتفاظ 30 يوماً | يتيح لوحات تماثل دون كشف تفاصيل OEM. |
| `android.telemetry.network_context` | حدث | `network_type`, `roaming` | شركة الاتصالات قد تكون PII | حذف `carrier_name` بالكامل؛ الاحتفاظ بباقي الحقول 7 أيام | `ClientConfig.networkContextProvider` يوفر لقطة مُنقّحة لتسمح للتطبيقات بإصدار نوع الشبكة + التجوال دون كشف بيانات المشترك؛ لوحات التماثل تعتبر الإشارة النظير المحمول لـ `peer_host` في Rust. |
| `android.telemetry.config.reload` | حدث | `source`, `result`, `duration_ms` | لا شيء | بلا تنقيح؛ الاحتفاظ 30 يوماً | يعكس spans إعادة تحميل الإعدادات في Rust. |
| `android.telemetry.chaos.scenario` | حدث | `scenario_id`, `outcome`, `duration_ms`, `device_profile` | ملف الجهاز ضمن buckets | مثل `device_profile`؛ الاحتفاظ 30 يوماً | يُسجل أثناء تدريبات chaos المطلوبة لجهوزية AND7. |
| `android.telemetry.redaction.salt_version` | مقياس Gauge | `salt_epoch`, `rotation_id` | لا شيء | بلا تنقيح؛ الاحتفاظ 365 يوماً | يتتبع دوران Blake2b salt؛ تنبيه تماثل عند اختلاف epoch في Android عن Rust. |
| `android.crash.report.capture` | حدث | `crash_id`, `signal`, `process_state`, `has_native_trace`, `anr_watchdog_bucket` | بصمة crash + بيانات العملية | تجزئة `crash_id` مع salt التنقيح المشتركة، وتجميع حالة watchdog في buckets، وحذف stack frames قبل التصدير؛ الاحتفاظ 30 يوماً | يُفعَّل تلقائياً عند استدعاء `ClientConfig.Builder.enableCrashTelemetryHandler()`؛ يغذي لوحات التماثل دون كشف آثار تعريفية. |
| `android.crash.report.upload` | عدّاد | `crash_id`, `backend`, `status`, `retry_count` | بصمة crash | إعادة استخدام `crash_id` المُجزأ، وإخراج الحالة فقط؛ الاحتفاظ 30 يوماً | الإرسال عبر `ClientConfig.crashTelemetryReporter()` أو `CrashTelemetryHandler.recordUpload` لضمان نفس ضمانات Sigstore/OLTP لباقي القياس. |

### نقاط التكامل

- يقوم `ClientConfig` الآن بتمرير بيانات القياس المشتقة من manifest عبر `setTelemetryOptions(...)`/`setTelemetrySink(...)` مع تسجيل `TelemetryObserver` تلقائياً حتى تتدفق السلطات المُجزأة ومقاييس salt بدون observers مخصصة. راجع `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java` والفئات تحت `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/`.
- يمكن للتطبيقات استدعاء `ClientConfig.Builder.enableAndroidNetworkContext(android.content.Context)` لتسجيل `AndroidNetworkContextProvider` القائم على الانعكاس، والذي يستدعي `ConnectivityManager` وقت التشغيل ويُصدر حدث `android.telemetry.network_context` دون إضافة تبعيات Android وقت البناء.
- اختبارات الوحدة `TelemetryOptionsTests` و`TelemetryObserverTests` (`java/iroha_android/src/test/java/org/hyperledger/iroha/android/telemetry/`) تحمي أدوات التجزئة وخطاف دمج ClientConfig حتى تظهر ارتدادات manifest فوراً.
- يشير kit/labs للتمكين الآن إلى واجهات API حقيقية بدلاً من الشيفرة الصورية، ما يبقي هذا المستند والـ runbook متوافقين مع SDK المُشحن.

> **ملاحظة تشغيلية:** ورقة المالك/الحالة موجودة في `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` ويجب تحديثها مع هذا الجدول في كل نقطة AND7.

## قوائم السماح للتماثل وتدفق فرق المخطط

تتطلب الحوكمة قائمة سماح مزدوجة حتى لا تُسرب صادرات Android المعرفات التي تُظهرها خدمات Rust عن قصد. يعكس هذا القسم إدخال الـ runbook (`docs/source/android_runbook.md` §2.3) مع إبقاء خطة AND7 مكتفية بذاتها.

| الفئة | مُصدّرات Android | خدمات Rust | نقطة التحقق |
|----------|-------------------|---------------|-----------------|
| سياق السلطة/المسار | تجزئة `authority`/`alias` عبر Blake2b-256 وحذف أسماء Torii الخام قبل التصدير؛ إصدار `android.telemetry.redaction.salt_version` لإثبات دوران salt. | إصدار أسماء Torii كاملة ومعرفات peer للربط. | مقارنة `android.torii.http.request` بـ `torii.http.request` في أحدث diff تحت `docs/source/sdk/android/readiness/schema_diffs/` ثم تشغيل `scripts/telemetry/check_redaction_status.py` لتأكيد epochs للـ salt. |
| هوية الجهاز/الموقّع | تجميع `hardware_tier`/`device_profile` في buckets، وتجزيء aliases للمتحكم، وعدم تصدير الأرقام التسلسلية. | إصدار `peer_id` للمحقق، و`public_key` للمتحكم، وhashes للصف دون تغيير. | المواءمة مع `docs/source/sdk/mobile_device_profile_alignment.md`، وتشغيل اختبارات تجزئة alias ضمن `java/iroha_android/run_tests.sh`، وأرشفة مخرجات queue inspector أثناء labs. |
| بيانات الشبكة | إصدار `network_type` + `roaming` فقط؛ حذف `carrier_name`. | الاحتفاظ ببيانات hostname/TLS للأقران. | تخزين كل diff في `readiness/schema_diffs/` والتنبيه إذا أظهر عنصر “Network Context” في Grafana أسماء شركات الاتصالات. |
| أدلة override/chaos | إصدار `android.telemetry.redaction.override`/`android.telemetry.chaos.scenario` مع أدوار مقنّعة. | إصدار موافقات override بدون قناع؛ دون spans خاصة بالـ chaos. | مراجعة `docs/source/sdk/android/readiness/and7_operator_enablement.md` بعد التدريبات لضمان أن رموز override وأدلة chaos تأتي بجانب أحداث Rust غير المقنّعة. |

سير العمل:

1. بعد كل تغيير في manifest أو المُصدِّر، نفّذ `scripts/telemetry/run_schema_diff.sh --android-config <android.json> --rust-config <rust.json>` وضع JSON في `docs/source/sdk/android/readiness/schema_diffs/`.
2. راجع diff مقابل الجدول أعلاه. إذا أصدر Android حقلاً خاصاً بـ Rust (أو العكس)، سجّل خطأ AND7 وحدّث هذه الخطة والـ runbook.
3. خلال مراجعات ops الأسبوعية، نفّذ `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status` وسجّل epoch للـ salt وطابع diff في ورقة الجهوزية.
4. سجّل أي انحرافات في `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` حتى تلتقط حزم الحوكمة قرارات التماثل.

> **مرجع المخطط:** معرفات الحقول القياسية مصدرها `android_telemetry_redaction.proto` (يتم توليدها أثناء بناء Android SDK مع واصفات Norito). يكشف المخطط حقول `authority_hash` و`alias_label` و`attestation_digest` و`device_brand_bucket` و`actor_role_masked` المستخدمة الآن عبر SDK ومُصدّرات القياس.

`authority_hash` هو digest ثابت بطول 32 بايت لقيمة Torii authority المسجلة في proto. يلتقط `attestation_digest` بصمة بيان attestation القياسي، بينما يربط `device_brand_bucket` سلسلة العلامة الخام في Android بالـ enum المعتمد (`generic`, `oem`, `enterprise`). يحمل `actor_role_masked` فئة الدور (`support`, `sre`, `audit`) بدلاً من معرف المستخدم الخام.

### مواءمة تصدير قياس الأعطال

تشارك قياسات الأعطال الآن نفس مُصدّرات OpenTelemetry ونفس سلسلة provenance التي تستخدمها إشارات Torii، ما يغلق متابعة الحوكمة حول المُصدّرات المكررة. يغذي crash handler حدث `android.crash.report.capture` بمعرّف `crash_id` مُجزأ (Blake2b-256 باستخدام salt التنقيح المتابع عبر `android.telemetry.redaction.salt_version`)، وبـ buckets لحالة العملية وبيانات ANR watchdog مُنقّحة. تبقى stack traces على الجهاز وتُختصر فقط في `has_native_trace` و`anr_watchdog_bucket` قبل التصدير، لذلك لا تغادر أي PII أو سلاسل OEM الجهاز.

إن رفع crash يُنشئ عدّاد `android.crash.report.upload`، ما يمكّن SRE من تدقيق موثوقية backend دون معرفة المستخدم أو الأثر. وبما أن الإشارتين تستخدمان مُصدّر Torii، فهما ترثان توقيع Sigstore وسياسات الاحتفاظ وأدوات التنبيه المحددة لـ AND7. يمكن لـ runbooks الربط بين معرف crash المُجزأ عبر حزم أدلة Android وRust دون خط أنابيب خاص.

فعِّل المعالج عبر `ClientConfig.Builder.enableCrashTelemetryHandler()` بعد ضبط خيارات القياس والمصارف؛ ويمكن لجسور الرفع إعادة استخدام `ClientConfig.crashTelemetryReporter()` (أو `CrashTelemetryHandler.recordUpload`) لإصدار نتائج backend ضمن نفس القناة الموقعة.

## فروقات السياسة مقابل خط الأساس في Rust

الفروقات بين سياسات قياس Android وRust مع خطوات التخفيف.

| الفئة | خط الأساس Rust | سياسة Android | التخفيف / التحقق |
|----------|---------------|----------------|-------------------------|
| معرّفات السلطة/الند | سلاسل authority مكشوفة | `authority_hash` (Blake2b-256 مع تدوير salt) | نشر salt المشترك عبر `iroha_config.telemetry.redaction_salt`؛ اختبار تماثل يضمن إمكانية الربط للدعم. |
| بيانات المضيف/الشبكة | تصدير hostnames/IPs للعُقد | نوع الشبكة + التجوال فقط | تحديث لوحات صحة الشبكة لاستخدام فئات التوفر بدل hostnames. |
| خصائص الجهاز | غير مطبق (سيرفر) | ملف bucketized (SDK 21/23/29+، tier `emulator`/`consumer`/`enterprise`) | تدريبات chaos تتحقق من mapping؛ runbook يوضح مسار التصعيد عند الحاجة لتفاصيل أدق. |
| Overrides للتنقيح | غير مدعوم | token override يدوي محفوظ في Norito ledger (`actor_role_masked`, `reason`) | يتطلب override طلباً موقعاً؛ سجل التدقيق يحتفظ سنة واحدة. |
| آثار attestation | attestation على الخادم عبر SRE فقط | SDK يصدر ملخص attestation مُنقّح | مطابقة hashes مع مدقق Rust؛ تجزئة alias تمنع التسريب. |

قائمة التحقق:

- اختبارات وحدة للتنقيح لكل إشارة تتحقق من الحقول المُجزأة/المقنّعة قبل الإرسال.
- أداة diff للمخطط (مشتركة مع Rust) تُشغّل ليلاً لتأكيد تماثل الحقول.
- سكربت تدريبات chaos يمارس مسار override ويؤكد سجل التدقيق.

## مهام التنفيذ (قبل حوكمة SRE)

1. **تأكيد الجرد** — مطابقة الجدول أعلاه مع hooks الفعلية في Android SDK وتعريفات Norito schema. الملاك: Android Observability TL، LLM.
2. **Telemetry Schema Diff** — تشغيل أداة diff المشتركة مقابل مقاييس Rust لإنتاج أدلة تماثل لمراجعة SRE. المالك: SRE privacy lead.
3. **مسودة الـ runbook (أُنجزت 2026-02-03)** — يشرح `docs/source/android_runbook.md` مسار override بالكامل (القسم 3) ومصفوفة التصعيد والمسؤوليات الموسعة (القسم 3.1)، ويربط أدوات CLI وأدلة الحوادث وسكربتات chaos بسياسة الحوكمة. الملاك: LLM مع تحرير Docs/Support.
4. **محتوى التمكين** — تجهيز شرائح الإحاطة وتعليمات المختبر وأسئلة التحقق للّقاء في فبراير 2026. الملاك: Docs/Support Manager وفريق تمكين SRE.

## سير عمل التمكين وربط الـ runbook

### 1. تغطية smoke محلية + CI

- `scripts/android_sample_env.sh --telemetry --telemetry-duration=5m --telemetry-cluster=<host>` يشغّل sandbox Torii ويعيد تشغيل fixture SoraFS متعدد المصادر (بالتفويض إلى `ci/check_sorafs_orchestrator_adoption.sh`) ويغرس قياس Android اصطناعيًا.
  - توليد الحركة بواسطة `scripts/telemetry/generate_android_load.py` الذي يسجل transcript request/response في `artifacts/android/telemetry/load-generator.log` ويأخذ headers وتجاوزات المسار أو وضع dry‑run في الاعتبار.
  - ينسخ المساعد scoreboard وملخصات SoraFS إلى `${WORKDIR}/sorafs/` حتى تُثبت تدريبات AND7 تماثل المصادر المتعددة قبل استخدام العملاء على الأجهزة المحمولة.
- تعيد CI استخدام الأدوات نفسها: `ci/check_android_dashboard_parity.sh` يشغّل `scripts/telemetry/compare_dashboards.py` مقابل `dashboards/grafana/android_telemetry_overview.json` ولوحة Rust المرجعية وملف السماح `dashboards/data/android_rust_dashboard_allowances.json`، ويصدر snapshot موقع `docs/source/sdk/android/readiness/dashboard_parity/android_vs_rust-latest.json`.
- تتبع تدريبات chaos ملف `docs/source/sdk/android/telemetry_chaos_checklist.md`؛ ويشكّل script sample‑env مع فحص تماثل اللوحات حزمة أدلة “ready” التي تغذي تدقيق burn‑in لـ AND7.

### 2. إصدار override ومسار التدقيق

- `scripts/android_override_tool.py` هي CLI الأساسية لإصدار وإلغاء overrides. تقوم `apply` بابتلاع طلب موقع، وإصدار manifest bundle (`telemetry_redaction_override.to` افتراضياً)، وإضافة صف token مُجزأ إلى `docs/source/sdk/android/telemetry_override_log.md`. وتضيف `revoke` طابع الإلغاء إلى الصف نفسه، بينما تكتب `digest` لقطة JSON مُنقّحة مطلوبة للحوكمة.
- ترفض CLI تعديل سجل التدقيق إذا لم يكن رأس جدول Markdown موجوداً، امتثالاً لمتطلبات `docs/source/android_support_playbook.md`. تغطي `scripts/tests/test_android_override_tool_cli.py` محلل الجدول ومرسلات manifest ومعالجة الأخطاء.
- يرفق المشغلون manifest المُولّد ومقتطف السجل المُحدّث **و** digest JSON في `docs/source/sdk/android/readiness/override_logs/` عند كل override؛ يحتفظ السجل بتاريخ 365 يوماً حسب قرار الحوكمة.

### 3. جمع الأدلة والاحتفاظ

- كل تدريب أو حادث يُنتج حزمة منظمة تحت `artifacts/android/telemetry/` تتضمن:
  - transcript مولّد الحركة والعدادات المجمعة من `generate_android_load.py`.
  - فرق اللوحات (`android_vs_rust-<stamp>.json`) وhash السماح الصادر من `ci/check_android_dashboard_parity.sh`.
  - فرق سجل override (إن وُجد)، وmanifest الموافق، وdigest JSON المُحدّث.
- يشير تقرير burn‑in لـ SRE إلى هذه الأدلة وإلى scoreboard SoraFS المنسوخ بواسطة `android_sample_env.sh`، ما يوفّر سلسلة حتمية من hashes قياس عن بُعد → لوحات → حالة overrides.

## مواءمة ملفات الأجهزة عبر SDKs

تترجم اللوحات `hardware_tier` الخاص بـ Android إلى `mobile_profile_class` القياسي المحدد في `docs/source/sdk/mobile_device_profile_alignment.md` كي تقارن AND7 وIOS7 نفس الشرائح:

- `lab` — يُصدر كـ `hardware_tier = emulator`، ويطابق `device_profile_bucket = simulator` في Swift.
- `consumer` — يُصدر كـ `hardware_tier = consumer` (مع لاحقة SDK major) ويُجمع مع buckets `iphone_small`/`iphone_large`/`ipad` في Swift.
- `enterprise` — يُصدر كـ `hardware_tier = enterprise` ويتماشى مع bucket `mac_catalyst` في Swift ومع runtimes سطح مكتب iOS المُدارة مستقبلاً.

يجب إضافة أي tier جديد إلى مستند المواءمة وأدلة diff قبل أن تستهلكه اللوحات.

## الحوكمة والتوزيع

- **حزمة pre‑read** — سيُوزَّع هذا المستند مع ملحقاته (schema diff، runbook diff، مخطط readiness deck) على قائمة بريد حوكمة SRE في موعد أقصاه **2026-02-05**.
- **حلقة التغذية الراجعة** — ستغذي التعليقات التي تُجمع أثناء الحوكمة ملحمة JIRA `AND7`؛ تُذكر العوائق في `status.md` وملاحظات الاجتماع الأسبوعي Android.
- **النشر** — بعد الاعتماد، سيُربط ملخص السياسة من `docs/source/android_support_playbook.md` ويشار إليه في FAQ القياس المشتركة في `docs/source/telemetry.md`.

## ملاحظات التدقيق والامتثال

- تلتزم السياسة بمتطلبات GDPR/CCPA بإزالة بيانات المشترك المحمول قبل التصدير؛ يدور salt لسلطة التجزئة كل ربع سنة ويُخزن في مخزن الأسرار المشترك.
- تُسجل أدوات التمكين وتحديثات runbook في سجل الامتثال.
- تؤكد المراجعات الفصلية أن overrides بقيت ضمن حلقة مغلقة (دون وصول قديم).

## نتيجة الحوكمة (2026-02-12)

اعتمدت جلسة حوكمة SRE في **2026-02-12** سياسة تنقيح Android دون تعديلات. القرارات الرئيسية (انظر `docs/source/sdk/android/telemetry_redaction_minutes_20260212.md`):

- **قبول السياسة.** أُقرت سلطة التجزئة، وتجميع ملف الجهاز، وحذف أسماء carriers. يصبح تتبع دوران salt عبر `android.telemetry.redaction.salt_version` بند تدقيق فصلي.
- **خطة التحقق.** تم اعتماد تغطية unit/integration وتشغيل فرق المخطط ليلاً وتدريبات chaos ربع سنوية. الإجراء: نشر تقرير تماثل اللوحات بعد كل تدريب.
- **حوكمة overrides.** تمت الموافقة على tokens المسجلة في Norito مع نافذة احتفاظ 365 يوماً. تتولى Support engineering مراجعة digest سجل override في تزامنات التشغيل الشهرية.

## حالة المتابعة

1. **مواءمة ملف الجهاز (الاستحقاق 2026-03-01).** ✅ مكتمل — يحدد المخطط المشترك في `docs/source/sdk/mobile_device_profile_alignment.md` كيفية ربط `hardware_tier` من Android بـ `mobile_profile_class` القياسي المستخدم في لوحات التماثل وأداة diff.

## الإحاطة القادمة لحوكمة SRE (الربع الثاني 2026)

يتطلب بند **AND7** أن تتلقى جلسة الحوكمة القادمة قراءة مسبقة موجزة عن تنقيح قياس Android. استخدم هذا القسم كإحاطة حيّة وحدثه قبل كل اجتماع مجلس.

### قائمة التحقق للتحضير

1. **حزمة الأدلة** — صدّر أحدث schema diff ولقطات اللوحات وdigest سجل overrides (انظر المصفوفة أدناه) وضعها في مجلد بتاريخ (مثلاً `docs/source/sdk/android/readiness/and7_sre_brief/2026-02-07/`) قبل إرسال الدعوة.
2. **ملخص التدريب** — أرفق أحدث سجل تدريب chaos ولقطة مقياس `android.telemetry.redaction.failure`؛ تأكد من أن تعليقات Alertmanager تشير إلى الطابع الزمني نفسه.
3. **تدقيق overrides** — تحقق من تسجيل جميع overrides النشطة في سجل Norito وتلخيصها في deck الاجتماع. أضف تواريخ الانتهاء ومعرّفات الحوادث المرتبطة.
4. **ملاحظة جدول الأعمال** — أخبر رئيس SRE قبل الاجتماع بـ 48 ساعة مع رابط الإحاطة، مع إبراز القرارات المطلوبة (إشارات جديدة، تغييرات الاحتفاظ، أو تحديثات سياسة overrides).

### مصفوفة الأدلة

| الأثر | الموقع | المالك | الملاحظات |
|----------|----------|-------|-------|
| فرق المخطط مقابل Rust | `docs/source/sdk/android/readiness/schema_diffs/<latest>.json` | Telemetry tooling DRI | يجب توليده خلال <72 ساعة قبل الاجتماع. |
| لقطات فرق اللوحات | `docs/source/sdk/android/readiness/dashboards/<date>/` | Observability TL | تضمين `sorafs.fetch.*`, `android.telemetry.*` ولقطات Alertmanager. |
| digest للـ overrides | `docs/source/sdk/android/readiness/override_logs/<date>.json` | Support engineering | نفّذ `scripts/android_override_tool.sh digest` (انظر README بالمجلد) على `telemetry_override_log.md` الأحدث؛ تبقى الرموز مُجزأة. |
| سجل تدريب chaos | `artifacts/android/telemetry/chaos/<date>/log.ndjson` | QA automation | أرفق ملخص KPI (عدد التوقفات، نسبة إعادة المحاولة، استخدام overrides). |

### أسئلة مفتوحة للمجلس

- هل نحتاج تقصير نافذة احتفاظ overrides من 365 يوماً الآن بعد أتمتة digest؟
- هل ينبغي لـ `android.telemetry.device_profile` اعتماد تسميات `mobile_profile_class` المشتركة في الإصدار القادم، أم الانتظار حتى يشحن Swift/JS SDK التغيير نفسه؟
- هل نحتاج إرشادات إضافية حول إقامة البيانات إقليمياً بعد وصول أحداث Torii Norito-RPC إلى Android (متابعة NRPC-3)؟

### إجراءات Telemetry Schema Diff

شغّل أداة فرق المخطط مرة واحدة على الأقل لكل مرشح إصدار (وعند أي تغيير في instrumentation Android) حتى يتلقى مجلس SRE أدلة تماثل حديثة مع فرق اللوحات:

1. صدّر مخططات القياس في Android وRust للمقارنة. إعدادات CI موجودة في `configs/android_telemetry.json` و`configs/rust_telemetry.json`.
2. نفّذ `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/<date>-android_vs_rust.json`.
   - بدلاً من ذلك، مرّر commits (`scripts/telemetry/run_schema_diff.sh android-main rust-main`) لجلب الإعدادات مباشرة من git؛ يقوم السكربت بتثبيت hashes داخل الأثر.
3. أرفق JSON الناتج بحزمة readiness واربطه من `status.md` + `docs/source/telemetry.md`. يسلط الفرق الضوء على الحقول المضافة/المزالة وفروقات الاحتفاظ ليتمكن المدققون من التحقق دون إعادة تشغيل الأداة.
4. عندما يكشف الفرق عن تباين مسموح (مثل إشارات override خاصة بـ Android)، حدّث ملف allowlist المشار إليه بواسطة `ci/check_android_dashboard_parity.sh` ودوّن المبرر في README الخاص بمجلد diff.

> **قواعد الأرشفة:** احتفظ بخمسة diffات الأخيرة تحت `docs/source/sdk/android/readiness/schema_diffs/` وانقل اللقطات الأقدم إلى `artifacts/android/telemetry/schema_diffs/` حتى يرى مراجِعو الحوكمة أحدث البيانات دائماً.
