---
lang: ar
direction: rtl
source: docs/source/android_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: da7119ab99121dbcfc268f5406f43b16ac9149cef6500a45c6717ad16c02ab80
source_last_modified: "2026-01-28T17:01:56.615899+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# دليل عمليات Android SDK

يدعم دليل التشغيل هذا المشغلين ومهندسي الدعم الذين يديرون Android SDK
عمليات النشر لـ AND7 وما بعده. قم بالاقتران مع دليل التشغيل لدعم Android لاتفاقية مستوى الخدمة (SLA).
التعريفات ومسارات التصعيد.

> **ملاحظة:** عند تحديث إجراءات الحادث، قم أيضًا بتحديث ما تمت مشاركته
> مصفوفة استكشاف الأخطاء وإصلاحها (`docs/source/sdk/android/troubleshooting.md`) لذلك
> يظل جدول السيناريو واتفاقيات مستوى الخدمة ومراجع القياس عن بعد متوافقة مع دليل التشغيل هذا.

## 0. التشغيل السريع (عند تشغيل أجهزة النداء)

احتفظ بهذا التسلسل في متناول يديك لتنبيهات Sev1/Sev2 قبل الغوص في التفاصيل
الأقسام أدناه:

1. **تأكيد التكوين النشط:** التقط المجموع الاختباري لبيان `ClientConfig`
   المنبعثة عند بدء تشغيل التطبيق ومقارنتها بالبيان المثبت في
   `configs/android_client_manifest.json`. إذا تباينت التجزئة، أوقف الإصدارات و
   قم بتقديم تذكرة انجراف التكوين قبل لمس القياس عن بعد/التجاوزات (انظر §1).
2. ** قم بتشغيل بوابة فرق المخطط: ** قم بتنفيذ `telemetry-schema-diff` CLI ضد
   اللقطة المقبولة
   (`docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`).
   تعامل مع أي مخرجات `policy_violations` على أنها Sev2 وقم بحظر الصادرات حتى
   التناقض مفهوم (انظر الفقرة 2.6).
3. **التحقق من لوحات المعلومات + حالة واجهة سطر الأوامر:** افتح Android Telemetry Redaction و
   المجالس الصحية مصدر، ثم تشغيل
   `scripts/telemetry/check_redaction_status.py --status-url <collector>`. إذا
   السلطات تحت الأرض أو خطأ في التصدير، والتقاط لقطات الشاشة و
   إخراج CLI لمستند الحادث (انظر §2.4-§2.5).
4. **اتخذ قرار بشأن التجاوزات:** فقط بعد الخطوات المذكورة أعلاه ومع الحادث/المالك
   المسجلة، قم بإصدار تجاوز محدود عبر `scripts/android_override_tool.sh`
   وقم بتسجيل الدخول إلى `telemetry_override_log.md` (انظر الفقرة 3). انتهاء الصلاحية الافتراضي: <24 ساعة.
5. **التصعيد حسب قائمة جهات الاتصال:** صفحة Android عند الطلب وقابلية المراقبة TL
   (جهات الاتصال في §8)، ثم اتبع شجرة التصعيد في §4.1. إذا شهادة أو
   إشارات StrongBox متضمنة، اسحب الحزمة الأحدث وقم بتشغيل الحزام
   الشيكات من §7 قبل إعادة تمكين الصادرات.

## 1. التكوين والنشر

- **مصادر ClientConfig:** تأكد من قيام عملاء Android بتحميل Torii لنقطة النهاية، TLS
  السياسات، وأعد محاولة المقابض من البيانات المشتقة من `iroha_config`. التحقق من صحة
  القيم أثناء بدء تشغيل التطبيق والمجموع الاختباري للسجل للبيان النشط.
  مرجع التنفيذ: `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`
  المواضيع `TelemetryOptions` من `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/TelemetryOptions.java`
  (بالإضافة إلى `TelemetryObserver`) لذلك يتم إصدار السلطات المجزأة تلقائيًا.
- **التحديث السريع:** استخدم مراقب التكوين لالتقاط `iroha_config`
  التحديثات دون إعادة تشغيل التطبيق. يجب أن تؤدي عمليات إعادة التحميل الفاشلة إلى إصدار ملف
  حدث `android.telemetry.config.reload` وتشغيل إعادة المحاولة باستخدام الأسي
  التراجع (بحد أقصى 5 محاولات).
- **السلوك الاحتياطي:** عندما يكون التكوين مفقودًا أو غير صالح، ارجع إلى
  الإعدادات الافتراضية الآمنة (وضع القراءة فقط، عدم وجود إرسال في قائمة الانتظار المعلقة) وإظهار المستخدم
  موجه. سجل الحادثة للمتابعة.

### 1.1 تشخيص إعادة تحميل التكوين- يقوم مراقب التكوين بإصدار إشارات `android.telemetry.config.reload` مع
  `source`، و`result`، و`duration_ms`، والحقول الاختيارية `digest`/`error` (راجع
  `configs/android_telemetry.json` و
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ConfigWatcher.java`).
  توقع حدوث حدث `result:"success"` واحد لكل بيان مطبق؛ متكرر
  تشير سجلات `result:"error"` إلى أن المراقب استنفد محاولات التراجع الخمس
  بدءًا من 50 مللي ثانية.
- أثناء وقوع حادث، التقط أحدث إشارة إعادة تحميل من المجمع
  (متجر OTLP/span أو نقطة نهاية حالة التنقيح) وقم بتسجيل `digest` +
  `source` في مستند الحادث. قارن الملخص ب
  `configs/android_client_manifest.json` وبيان الإصدار الموزع على
  مشغلي.
- إذا استمر المراقب في إصدار الأخطاء، قم بتشغيل الحزام المستهدف لإعادة الإنتاج
  فشل التحليل مع بيان المشتبه به:
  `ci/run_android_tests.sh org.hyperledger.iroha.android.client.ConfigWatcherTests`.
  قم بإرفاق مخرجات الاختبار وبيان الفشل بحزمة الحادث حتى SRE
  يمكن أن يختلف عن مخطط التكوين المخبوز.
- عندما يكون القياس عن بعد لإعادة التحميل مفقودًا، تأكد من أن `ClientConfig` النشط يحمل علامة
  حوض القياس عن بعد وأن جامع OTLP لا يزال يقبل
  معرف `android.telemetry.config.reload`؛ وإلا تعامل معه على أنه قياس عن بعد Sev2
  الانحدار (نفس المسار كما في §2.4) والإصدارات وقفة حتى تعود الإشارة.

### 1.2 حزم التصدير الرئيسية الحتمية
- تُصدر صادرات البرامج الآن حزم v3 مع الملح لكل تصدير + nonce، و`kdf_kind`، و`kdf_work_factor`.
  يفضل المصدر Argon2id (64 MiB، 3 تكرارات، التوازي = 2) ويعود إلى
  PBKDF2-HMAC-SHA256 بأرضية تكرار تبلغ 350 كيلو عند عدم توفر Argon2id على الجهاز. حزمة
  لا يزال AAD يرتبط بالاسم المستعار؛ يجب أن تتكون عبارة المرور من 12 حرفًا على الأقل لصادرات الإصدار 3 و
  يرفض المستورد البذور الخالية من الملح/النونس.
  `KeyExportBundle.decode(Base64|bytes)`، قم بالاستيراد باستخدام عبارة المرور الأصلية، ثم أعد التصدير إلى الإصدار v3 إلى
  الانتقال إلى تنسيق الذاكرة الثابتة. يرفض المستورد أزواج الملح/النونس التي تحتوي على صفر كامل أو المعاد استخدامها؛ دائما
  قم بتدوير الحزم بدلاً من إعادة استخدام الصادرات القديمة بين الأجهزة.
- اختبارات المسار السلبي في `ci/run_android_tests.sh --tests org.hyperledger.iroha.android.crypto.export.DeterministicKeyExporterTests`
  الرفض. امسح مصفوفات أحرف عبارة المرور بعد الاستخدام والتقط كلاً من إصدار الحزمة و`kdf_kind`
  في ملاحظات الحوادث عند فشل عملية الاسترداد.

## 2. القياس عن بعد والتنقيح

> مرجع سريع: انظر
> [`telemetry_redaction_quick_reference.md`](sdk/android/telemetry_redaction_quick_reference.md)
> لقائمة مراجعة الأمر/العتبة المختصرة المستخدمة أثناء التمكين
> الجلسات والجسور الحادثة.- **جرد الإشارات:** راجع `docs/source/sdk/android/telemetry_redaction.md`
  للحصول على القائمة الكاملة للامتدادات/المقاييس/الأحداث المنبعثة و
  `docs/source/sdk/android/readiness/signal_inventory_worksheet.md`
  للحصول على تفاصيل المالك/التحقق والفجوات المعلقة.
- **فرق المخطط الأساسي:** لقطة AND7 المعتمدة هي
  `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`.
  يجب مقارنة كل تشغيل CLI جديد مع هذا المنتج حتى يتمكن المراجعون من رؤيته
  أن `intentional_differences` و`android_only_signals` المقبولين لا يزالان
  تطابق جداول السياسة الموثقة في
  `docs/source/sdk/android/telemetry_schema_diff.md` §3. يضيف CLI الآن
  `policy_violations` عند فقدان أي اختلاف متعمد أ
  `status:"accepted"`/`"policy_allowlisted"` (أو عندما تفقد سجلات Android فقط
  حالتها المقبولة)، لذا تعامل مع الانتهاكات غير الفارغة على أنها Sev2 وتوقف
  الصادرات. تبقى مقتطفات `jq` أدناه بمثابة فحص يدوي للسلامة في المؤرشفة
  المصنوعات اليدوية:
  ```bash
  jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted")' "$OUT"
  jq '.android_only_signals[] | select(.status != "accepted")' "$OUT"
  jq '.field_mismatches[] | {signal, field, android, rust}' "$OUT"
  ```
  تعامل مع أي مخرجات من هذه الأوامر على أنها انحدار مخطط يحتاج إلى
  خطأ في جاهزية AND7 قبل استمرار تصدير أجهزة القياس عن بعد؛ `field_mismatches`
  يجب أن يبقى فارغًا وفقًا لـ `telemetry_schema_diff.md` §5. المساعد يكتب الآن
  `artifacts/android/telemetry/schema_diff.prom` تلقائيًا؛ تمرير
  `--textfile-dir /var/lib/node_exporter/textfile_collector` (أو مجموعة
  `ANDROID_SCHEMA_DIFF_TEXTFILE_DIR`) عند التشغيل على مضيفات التدريج/الإنتاج
  لذلك ينقلب المقياس `telemetry_schema_diff_run_status` إلى `policy_violation`
  تلقائيًا إذا اكتشف CLI الانجراف.
- **مساعد CLI:** يقوم `scripts/telemetry/check_redaction_status.py` بالفحص
  `artifacts/android/telemetry/status.json` بشكل افتراضي؛ قم بتمرير `--status-url` إلى
  التدريج للاستعلام و`--write-cache` لتحديث النسخة المحلية في وضع عدم الاتصال
  التدريبات. استخدم `--min-hashed 214` (أو قم بتعيين
  `ANDROID_TELEMETRY_MIN_HASHED_AUTHORITIES=214`) لفرض الإدارة
  الكلمة على السلطات المجزأة خلال كل استطلاع للحالة.
- **تجزئة السلطة:** تتم تجزئة جميع السلطات باستخدام Blake2b-256 مع
  يتم تخزين ملح التدوير ربع السنوي في قبو الأسرار الآمن. تحدث التناوبات على
  أول يوم اثنين من كل ربع سنة الساعة 00:00 بالتوقيت العالمي. التحقق من أن المصدر يلتقط
  الملح الجديد عن طريق التحقق من مقياس `android.telemetry.redaction.salt_version`.
- **مجموعات ملفات تعريف الجهاز:** فقط `emulator` و`consumer` و`enterprise`
  يتم تصدير الطبقات (إلى جانب إصدار SDK الرئيسي). تقارن لوحات المعلومات هذه
  التهم ضد خطوط الأساس الصدأ. التباين > 10% يثير التنبيهات.
- **البيانات التعريفية للشبكة:** يصدر Android علامتي `network_type` و`roaming` فقط.
  لا يتم إطلاق أسماء شركات النقل أبدًا؛ لا ينبغي للمشغلين طلب المشترك
  المعلومات في سجلات الحوادث. يتم إصدار اللقطة المعقمة على هيئة
  حدث `android.telemetry.network_context`، لذا تأكد من تسجيل التطبيقات أ
  `NetworkContextProvider` (إما عبر
  `ClientConfig.Builder.setNetworkContextProvider(...)` أو الراحة
  `enableAndroidNetworkContext(...)` helper) قبل إصدار مكالمات Torii.
- **مؤشر Grafana:** لوحة المعلومات `Android Telemetry Redaction` هي
  التحقق البصري الأساسي لمخرج CLI أعلاه - قم بتأكيد
  تتوافق لوحة `android.telemetry.redaction.salt_version` مع عصر الملح الحالي
  ويظل عنصر واجهة المستخدم `android_telemetry_override_tokens_active` عند الصفر
  عندما لا يتم إجراء أي تدريبات أو حوادث. قم بالتصعيد إذا انجرفت أي من اللوحتين
  قبل أن تبلغ البرامج النصية لـ CLI عن الانحدار.

### 2.1 سير عمل خط أنابيب التصدير1. **توزيع التكوين.** يتم ربط `ClientConfig.telemetry.redaction` من
   `iroha_config` وتم إعادة تحميله بسرعة بواسطة `ConfigWatcher`. كل عملية إعادة تحميل تسجل
   ملخص واضح بالإضافة إلى عصر الملح - التقط هذا الخط في الحوادث وأثناءها
   التدريبات.
2. **الأجهزة.** تصدر مكونات SDK امتدادات/مقاييس/أحداث في ملف
   `TelemetryBuffer`. يقوم المخزن المؤقت بوضع علامات على كل حمولة مع ملف تعريف الجهاز و
   عصر الملح الحالي حتى يتمكن المصدر من التحقق من مدخلات التجزئة بشكل حتمي.
3. **مرشح التنقيح.** `RedactionFilter` تجزئات `authority`، و`alias`، و
   معرفات الجهاز قبل أن يغادروا الجهاز. تنبعث منها الفشل
   `android.telemetry.redaction.failure` وحظر محاولة التصدير.
4. **المصدر + المجمع.** يتم شحن الحمولات المعقمة عبر نظام Android
   مصدر OpenTelemetry إلى نشر `android-otel-collector`. ال
   مراوح التجميع تخرج إلى آثار (Tempo)، ومقاييس (Prometheus)، وNorito
   المصارف السجل.
5. **خطافات إمكانية الملاحظة.** يقرأ `scripts/telemetry/check_redaction_status.py`
   عدادات المجمع (`android.telemetry.export.status`،
   `android.telemetry.redaction.salt_version`) وينتج حزمة الحالة
   المشار إليها في جميع أنحاء هذا runbook.

### 2.2 بوابات التحقق

- **فرق المخطط:** تشغيل
  `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json`
  كلما ظهر التغيير. بعد كل تشغيل، قم بتأكيد كل
  تم ختم الإدخال `intentional_differences[*]` و`android_only_signals[*]`
  `status:"accepted"` (أو `status:"policy_allowlisted"` للمجزأة/الموحدة
  الحقول) على النحو الموصى به في `telemetry_schema_diff.md` §3 قبل إرفاق ملف
  قطعة أثرية للحوادث وتقارير المختبر الفوضى. استخدم اللقطة المعتمدة
  (`android_vs_rust-20260305.json`) كحاجز حماية ووبر المنبعث حديثًا
  JSON قبل تقديمه:
  ```bash
  LATEST=docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json
  jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted") | {signal, field, status}' "$LATEST"
  jq '.android_only_signals[] | select(.status != "accepted") | {signal, status}' "$LATEST"
  ```
  قارن `$LATEST` ضد
  `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`
  لإثبات أن القائمة المسموح بها ظلت دون تغيير. `status` مفقود أو فارغ
  الإدخالات (على سبيل المثال على `android.telemetry.redaction.failure` أو
  `android.telemetry.redaction.salt_version`) يتم التعامل معها الآن على أنها تراجعات و
  يجب حلها قبل إغلاق المراجعة؛ تظهر واجهة سطر الأوامر (CLI) على أنها المقبولة
  الدولة مباشرة، لذلك لا ينطبق المرجع الترافقي اليدوي §3.4 إلا عندما
  توضيح سبب ظهور الحالة غير `accepted`.

  ** إشارات AND7 الأساسية (لقطة 05/03/2026)**| إشارة | قناة | الحالة | مذكرة الحوكمة | ربط التحقق من الصحة |
  |--------|---------|------------------|---|-----------------|-----------------|
  | `android.telemetry.redaction.override` | الحدث | `accepted` | تتجاوز المرايا البيانات ويجب أن تتطابق مع إدخالات `telemetry_override_log.md`. | شاهد `android_telemetry_override_tokens_active` وبيانات الأرشيف وفقًا للفقرة 3. |
  | `android.telemetry.network_context` | الحدث | `accepted` | يقوم Android بتنقيح أسماء شركات الاتصالات عمدًا؛ يتم تصدير `network_type` و`roaming` فقط. | تأكد من تسجيل التطبيقات لـ `NetworkContextProvider` وتأكد من أن حجم الحدث يتبع حركة مرور Torii على `Android Telemetry Overview`. |
  | `android.telemetry.redaction.failure` | عداد | `accepted` | تنبعث عند فشل التجزئة؛ تتطلب الإدارة الآن بيانات تعريفية واضحة للحالة في عناصر المخطط المختلفة. | يجب أن تظل لوحة المعلومات `Redaction Compliance` وإخراج واجهة سطر الأوامر (CLI) من `check_redaction_status.py` عند الصفر باستثناء أثناء التدريبات. |
  | `android.telemetry.redaction.salt_version` | مقياس | `accepted` | إثبات أن المصدر يستخدم عصر الملح الفصلي الحالي. | قارن عنصر واجهة مستخدم الملح الخاص بـ Grafana مع عصر قبو الأسرار وتأكد من احتفاظ عمليات تشغيل المخطط المختلفة بالتعليق التوضيحي `status:"accepted"`. |

  إذا أسقط أي إدخال في الجدول أعلاه `status`، فيجب أن يكون الفرق الاصطناعي
  تم تجديد **و** `telemetry_schema_diff.md` قبل AND7
  يتم تعميم حزمة الحكم. قم بتضمين JSON المحدث في
  `docs/source/sdk/android/readiness/schema_diffs/` وربطه من
  الحادث أو معمل الفوضى أو تقرير التمكين الذي أدى إلى إعادة التشغيل.
- **تغطية CI/الوحدة:** يجب أن يمر `ci/run_android_tests.sh` من قبل
  بنيات النشر؛ يفرض الجناح سلوك التجزئة/التجاوز من خلال التمرين
  مصدري القياس عن بعد مع حمولات العينة.
- **فحوصات سلامة الحاقن:** الاستخدام
  `scripts/telemetry/inject_redaction_failure.sh --dry-run` قبل التدريبات
  للتأكد من فشل أعمال الحقن وذلك للتنبيه بالحريق عند تجزئة الحراس
  تعثرت. قم دائمًا بمسح الحاقن باستخدام `--clear` بمجرد التحقق من الصحة
  يكمل.

### 2.3 الهاتف المحمول ↔ قائمة التحقق من تكافؤ قياس الصدأ عن بعد

حافظ على محاذاة مصدري Android وخدمات عقدة Rust مع احترام
تم توثيق متطلبات التنقيح المختلفة في
`docs/source/sdk/android/telemetry_redaction.md`. الجدول أدناه بمثابة
القائمة المسموح بها المزدوجة المشار إليها في إدخال خريطة الطريق AND7 - قم بتحديثها كلما
يقدم schema diff الحقول أو يزيلها.| الفئة | المصدرين الروبوت | خدمات الصدأ | ربط التحقق من الصحة |
|----------|-------------------|--------------|-----------------|
| سياق السلطة / المسار | قم بتجزئة `authority`/`alias` عبر Blake2b-256 وقم بإسقاط أسماء مضيفات Torii الأولية قبل التصدير؛ تنبعث منها `android.telemetry.redaction.salt_version` لإثبات دوران الملح. | قم بإصدار أسماء المضيفين Torii الكاملة ومعرفات النظير للارتباط. | قارن بين إدخالات `android.torii.http.request` و`torii.http.request` في أحدث فرق المخطط ضمن `readiness/schema_diffs/`، ثم تأكد من تطابق `android.telemetry.redaction.salt_version` مع ملح الكتلة عن طريق تشغيل `scripts/telemetry/check_redaction_status.py`. |
| هوية الجهاز والموقّع | الحاوية `hardware_tier`/`device_profile`، والأسماء المستعارة لوحدة تحكم التجزئة، وعدم تصدير الأرقام التسلسلية مطلقًا. | لا توجد بيانات تعريف الجهاز؛ تنبعث العقد من أداة التحقق `peer_id` ووحدة التحكم `public_key` حرفيًا. | قم بعكس التعيينات في `docs/source/sdk/mobile_device_profile_alignment.md`، وقم بمراجعة مخرجات `PendingQueueInspector` أثناء التمارين المعملية، وتأكد من بقاء اختبارات تجزئة الاسم المستعار داخل `ci/run_android_tests.sh` باللون الأخضر. |
| البيانات التعريفية للشبكة | تصدير القيم المنطقية `network_type` + `roaming` فقط؛ تم إسقاط `carrier_name`. | يحتفظ Rust بأسماء المضيفين النظيرة بالإضافة إلى البيانات التعريفية الكاملة لنقطة نهاية TLS. | قم بتخزين أحدث JSON المختلف في `readiness/schema_diffs/` وتأكد من أن جانب Android لا يزال يحذف `carrier_name`. تنبيه إذا كانت أداة "سياق الشبكة" الخاصة بـ Grafana تعرض أي سلاسل حاملة. |
| تجاوز / دليل الفوضى | قم بإصدار أحداث `android.telemetry.redaction.override` و`android.telemetry.chaos.scenario` بأدوار ممثل مقنعة. | تصدر خدمات الصدأ موافقات التجاوز دون إخفاء الأدوار ولا توجد امتدادات خاصة بالفوضى. | تحقق من `docs/source/sdk/android/readiness/and7_operator_enablement.md` بعد كل تمرين لضمان أرشفة رموز التجاوز ومصنوعات الفوضى جنبًا إلى جنب مع أحداث Rust غير المقنعة. |

سير عمل التكافؤ:

1. بعد كل تغيير في البيان أو المصدر، قم بالتشغيل
   `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json --textfile-dir /var/lib/node_exporter/textfile_collector`
   لذا فإن قطعة JSON والمقاييس المتطابقة تقع في حزمة الأدلة
   (لا يزال المساعد يكتب `artifacts/android/telemetry/schema_diff.prom` افتراضيًا).
2. قم بمراجعة الفرق مقابل الجدول أعلاه؛ إذا كان Android يصدر الآن حقلاً
   مسموح به فقط على Rust (أو العكس)، قم بتقديم خطأ جاهزية AND7 وتحديثه
   خطة التنقيح.
3. أثناء الفحوصات الأسبوعية، قم بالجري
   `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`
   لتأكيد تطابق العصور الملحية مع عنصر واجهة المستخدم Grafana ولاحظ العصر في
   مجلة تحت الطلب.
4. سجل أي دلتا في
   `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` هكذا
   ويمكن للحوكمة مراجعة قرارات التكافؤ.

### 2.4 لوحات تحكم إمكانية المراقبة وحدود التنبيه

حافظ على توافق لوحات المعلومات والتنبيهات مع موافقات فرق مخطط AND7 متى
مراجعة إخراج `scripts/telemetry/check_redaction_status.py`:

- `Android Telemetry Redaction` — أداة عصر الملح، تجاوز مقياس الرمز المميز.
- `Redaction Compliance` — العداد `android.telemetry.redaction.failure` و
  لوحات الاتجاه حاقن.
- `Exporter Health` — تفاصيل الأسعار `android.telemetry.export.status`.
- `Android Telemetry Overview` - مجموعات ملفات تعريف الجهاز وحجم سياق الشبكة.

تعكس الحدود التالية البطاقة المرجعية السريعة ويجب تنفيذها
أثناء الاستجابة للحوادث والتدريبات:| متري / لوحة | العتبة | العمل |
|----------------|----------|--------|
| `android.telemetry.redaction.failure` (لوحة `Redaction Compliance`) | >0 خلال نافذة متجددة مدتها 15 دقيقة | التحقق من الإشارة الفاشلة، وتشغيل الحاقن بشكل واضح، وتسجيل إخراج CLI + لقطة شاشة Grafana. |
| `android.telemetry.redaction.salt_version` (لوحة `Android Telemetry Redaction`) | يختلف عن عصر الملح في أسرار قبو الملح | أوقف الإصدارات، وقم بالتنسيق مع تناوب الأسرار، وملف مذكرة AND7. |
| `android.telemetry.export.status{status="error"}` (لوحة `Exporter Health`) | >1% من الصادرات | افحص صحة المجمع، والتقط تشخيصات CLI، وقم بالتصعيد إلى SRE. |
| `android.telemetry.device_profile{tier="enterprise"}` مقابل تكافؤ الصدأ (`Android Telemetry Overview`) | التباين > 10% من خط الأساس لصدأ | متابعة إدارة الملف، والتحقق من مجموعات التركيبات، وإضافة تعليقات توضيحية لفرق المخطط. |
| حجم `android.telemetry.network_context` (`Android Telemetry Overview`) | ينخفض ​​إلى الصفر أثناء وجود حركة مرور Torii | قم بتأكيد تسجيل `NetworkContextProvider`، وأعد تشغيل مخطط الاختلاف لضمان عدم تغيير الحقول. |
| `android.telemetry.redaction.override` / `android_telemetry_override_tokens_active` (`Android Telemetry Redaction`) | نافذة تجاوز/حفر خارجية غير صفرية معتمدة | ربط الرمز المميز بحادث ما، وإعادة إنشاء الملخص، والإلغاء عبر سير العمل في الفقرة 3. |

### 2.5 مسار جاهزية المشغل وتمكينه

يستدعي عنصر خارطة الطريق AND7 منهجًا مخصصًا للمشغلين، لذا يدعم SRE و
يفهم أصحاب المصلحة في الإصدار جداول التكافؤ أعلاه قبل بدء دليل التشغيل
جا. استخدم المخطط التفصيلي في
`docs/source/sdk/android/telemetry_readiness_outline.md` للخدمات اللوجستية الأساسية
(جدول الأعمال، مقدمو العرض، الجدول الزمني) و`docs/source/sdk/android/readiness/and7_operator_enablement.md`
للحصول على قائمة المراجعة التفصيلية وروابط الأدلة وسجل الإجراءات. احتفظ بما يلي
تتم مزامنة المراحل كلما تغيرت خطة القياس عن بعد:| المرحلة | الوصف | حزمة الأدلة | المالك الأساسي |
|-------|------------|-----------------|---------------|
| توزيع ما قبل القراءة | أرسل السياسة المقروءة مسبقًا، `telemetry_redaction.md`، وبطاقة المرجع السريع قبل خمسة أيام عمل على الأقل من الإحاطة الإعلامية. تتبع الإقرارات في سجل اتصالات المخطط التفصيلي. | `docs/source/sdk/android/telemetry_readiness_outline.md` (لوجستيات الجلسة + سجل الاتصالات) والبريد الإلكتروني المؤرشف في `docs/source/sdk/android/readiness/archive/<YYYY-MM>/`. | المستندات/مدير الدعم |
| جلسة الاستعداد المباشرة | يمكنك تقديم التدريب لمدة 60 دقيقة (التعمق في السياسة، والإرشادات التفصيلية لدليل التشغيل، ولوحات المعلومات، والعرض التوضيحي لمختبر الفوضى) والحفاظ على تشغيل التسجيل للمشاهدين غير المتزامنين. | التسجيل + الشرائح المخزنة تحت `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` مع المراجع الملتقطة في الفقرة 2 من المخطط التفصيلي. | LLM (القائم بأعمال مالك AND7) |
| تنفيذ مختبر الفوضى | قم بتشغيل C2 (تجاوز) + C6 (إعادة تشغيل قائمة الانتظار) على الأقل من `docs/source/sdk/android/readiness/labs/telemetry_lab_01.md` مباشرة بعد الجلسة المباشرة وقم بإرفاق السجلات/لقطات الشاشة بمجموعة التمكين. | تقارير السيناريو ولقطات الشاشة داخل `docs/source/sdk/android/readiness/labs/reports/<YYYY-MM>/` و`/screenshots/<YYYY-MM>/`. | إمكانية ملاحظة Android TL + SRE عند الطلب |
| فحص المعرفة والحضور | اجمع إرسالات الاختبار، وصحح أي شخص سجل أقل من 90%، وسجل إحصائيات الحضور/الاختبار. حافظ على توافق الأسئلة المرجعية السريعة مع قائمة التحقق من التكافؤ. | يتم تصدير الاختبار في `docs/source/sdk/android/readiness/forms/responses/`، وملخص Markdown/JSON الذي يتم إنتاجه عبر `scripts/telemetry/generate_and7_quiz_summary.py`، وجدول الحضور داخل `and7_operator_enablement.md`. | هندسة الدعم |
| الأرشيف والمتابعات | قم بتحديث سجل إجراءات مجموعة التمكين، وتحميل العناصر إلى الأرشيف، ولاحظ الإكمال في `status.md`. يجب نسخ أي رموز مميزة للإصلاح أو التجاوز تم إصدارها أثناء الجلسة إلى `telemetry_override_log.md`. | `docs/source/sdk/android/readiness/and7_operator_enablement.md` §6 (سجل الإجراءات)، `.../archive/<YYYY-MM>/checklist.md`، وسجل التجاوز المشار إليه في §3. | LLM (القائم بأعمال مالك AND7) |

عند إعادة تشغيل المنهج (ربع سنوي أو قبل تغييرات المخطط الرئيسية)، قم بالتحديث
المخطط التفصيلي بتاريخ الجلسة الجديد، وحافظ على تحديث قائمة الحضور، و
قم بإعادة إنشاء ملخص الاختبار لمصنوعات JSON/Markdown حتى تتمكن حزم الإدارة من ذلك
مرجع أدلة متسقة. يجب أن يرتبط الإدخال `status.md` لـ AND7 بـ
أحدث مجلد أرشيف بمجرد إغلاق كل دورة تمكين.

### 2.6 القوائم المسموح بها لفرق المخططات وعمليات التحقق من السياسة

تدعو خارطة الطريق صراحةً إلى اتباع سياسة القائمة المزدوجة المسموح بها (تنقيحات الأجهزة المحمولة مقابل تنقيحات الأجهزة المحمولة).
الاحتفاظ بالصدأ) الذي يتم فرضه بواسطة `telemetry-schema-diff` CLI الموجود تحت
`tools/telemetry-schema-diff`. كل قطعة أثرية مختلفة مسجلة في
يجب أن يقوم `docs/source/sdk/android/readiness/schema_diffs/` بتوثيق الحقول الموجودة
مجزأة/مخزنة على Android، وما هي الحقول التي تظل غير مجزأة على Rust، وما إذا كان
انزلقت أي إشارة غير مدرجة في القائمة المسموح بها إلى البنية. التقاط تلك القرارات
مباشرة في JSON عن طريق تشغيل:

```bash
cargo run -p telemetry-schema-diff -- \
  --android-config configs/android_telemetry.json \
  --rust-config configs/rust_telemetry.json \
  --format json \
  > "$LATEST"

if jq -e '.policy_violations | length > 0' "$LATEST" >/dev/null; then
  jq '.policy_violations[]' "$LATEST"
  exit 1
fi
```يتم تقييم `jq` النهائي إلى عدم التشغيل عندما يكون التقرير نظيفًا. علاج أي الإخراج
من هذا الأمر كخطأ في جاهزية Sev2: `policy_violations` مأهول
المصفوفة تعني أن واجهة سطر الأوامر (CLI) اكتشفت إشارة غير موجودة في قائمة Android فقط
ولا في قائمة استثناءات الصدأ فقط الموثقة في
`docs/source/sdk/android/telemetry_schema_diff.md`. عندما يحدث هذا، توقف
قم بالتصدير، وقم بتقديم تذكرة AND7، وأعد تشغيل الفرق فقط بعد وحدة السياسة
وتم تصحيح اللقطات الواضحة. قم بتخزين JSON الناتج في
`docs/source/sdk/android/readiness/schema_diffs/` مع لاحقة التاريخ والملاحظة
المسار داخل تقرير الحادث أو المختبر حتى تتمكن الإدارة من إعادة تشغيل الفحوصات.

** مصفوفة التجزئة والاحتفاظ **

| سيجنال.فيلد | التعامل مع الروبوت | معالجة الصدأ | علامة القائمة المسموح بها |
|--------------|-----------------|---------------|---------------|
| `torii.http.request.authority` | Blake2b-256 مجزأة (`representation: "blake2b_256"`) | مخزنة حرفيا للتتبع | `policy_allowlisted` (تجزئة الجوال) |
| `attestation.result.alias` | Blake2b-256 مجزأة | الاسم المستعار للنص العادي (أرشيف الشهادات) | `policy_allowlisted` |
| `attestation.result.device_tier` | دلو (`representation: "bucketed"`) | سلسلة الطبقة العادية | `policy_allowlisted` |
| `hardware.profile.hardware_tier` | غائب - يقوم مصدرو Android بإسقاط الحقل بالكامل | تقديم بدون تنقيح | `rust_only` (موثق في القسم 3 من `telemetry_schema_diff.md`) |
| `android.telemetry.redaction.override.*` | إشارة Android فقط مع أدوار الممثل المقنع | لا توجد إشارة مكافئة منبعثة | `android_only` (يجب أن يبقى `status:"accepted"`) |

عندما تظهر إشارات جديدة، قم بإضافتها إلى وحدة سياسة فرق المخطط **و**
الجدول أعلاه بحيث يعكس دليل التشغيل منطق التنفيذ الذي تم تضمينه في واجهة سطر الأوامر (CLI).
يفشل تشغيل المخطط الآن إذا حذفت أي إشارة Android فقط الرقم `status` الصريح أو إذا
المصفوفة `policy_violations` ليست فارغة، لذا احتفظ بقائمة التحقق هذه متزامنة مع
`telemetry_schema_diff.md` §3 وأحدث لقطات JSON المشار إليها في
`telemetry_redaction_minutes_*.md`.

## 3. تجاوز سير العمل

التجاوزات هي خيار "كسر الزجاج" عند تجزئة الانحدارات أو الخصوصية
تنبيهات كتلة العملاء. قم بتطبيقها فقط بعد تسجيل المسار الكامل للقرار
في وثيقة الحادث.1. **تأكيد الانحراف والنطاق.** انتظر تنبيه PagerDuty أو فرق المخطط
   بوابة لاطلاق النار، ثم تشغيل
   `scripts/telemetry/check_redaction_status.py --status-url <collector>` إلى
   إثبات السلطات غير متطابقة. قم بإرفاق مخرجات CLI ولقطات الشاشة Grafana
   إلى محضر الحادثة.
2. **قم بإعداد طلب موقع.** قم بالتعبئة
   `docs/examples/android_override_request.json` مع معرف التذكرة، الطالب،
   انتهاء الصلاحية، والتبرير. قم بتخزين الملف بجانب الحادثة الأثرية
   الامتثال يمكن تدقيق المدخلات.
3. **أصدر التجاوز.** استدعاء
   ```bash
   scripts/android_override_tool.sh apply \
     --request docs/examples/android_override_request.json \
     --log docs/source/sdk/android/telemetry_override_log.md \
     --out artifacts/android/telemetry/override-$(date -u +%Y%m%dT%H%M%SZ).json \
     --event-log docs/source/sdk/android/readiness/override_logs/override_events.ndjson \
     --actor-role <support|sre|docs|compliance|program|other>
   ```
   يقوم المساعد بطباعة رمز التجاوز، ويكتب البيان، ويلحق صفًا
   إلى سجل تدقيق Markdown. لا تنشر الرمز المميز أبدًا في الدردشة؛ تسليمها مباشرة
   إلى مشغلي Torii الذين يطبقون التجاوز.
4. **مراقبة التأثير.** في غضون خمس دقائق، تحقق من واحدة
   تم إطلاق الحدث `android.telemetry.redaction.override`، المجمع
   تعرض نقطة نهاية الحالة `override_active=true`، ويسرد مستند الحادث ملف
   انتهاء الصلاحية. شاهد "تجاوز الرموز المميزة" في لوحة معلومات Android Telemetry Overview
   نشط" (`android_telemetry_override_tokens_active`) لنفسه
   قم بحساب الرموز المميزة واستمر في تشغيل سطر الأوامر للحالة كل 10 دقائق حتى
   استقرار التجزئة.
5. **الإلغاء والأرشفة.** بمجرد وصول عملية التخفيف، قم بالتشغيل
  `scripts/android_override_tool.sh revoke --token <token>` لذلك سجل التدقيق
  يلتقط وقت الإلغاء، ثم ينفذ
  `scripts/android_override_tool.sh digest --out docs/source/sdk/android/readiness/override_logs/override_digest_$(date -u +%Y%m%dT%H%M%SZ).json`
  لتحديث اللقطة المعقمة التي تتوقعها الإدارة. إرفاق
  البيان، ملخص JSON، ونصوص CLI، ولقطات Grafana، وسجل NDJSON
  تم إنتاجه عبر `--event-log` إلى
  `docs/source/sdk/android/readiness/screenshots/<date>/` والارتباط المتقاطع لـ
  الإدخال من `docs/source/sdk/android/telemetry_override_log.md`.

تتطلب التجاوزات التي تتجاوز 24 ساعة موافقة مدير SRE وقسم الامتثال
يجب تسليط الضوء عليه في مراجعة AND7 الأسبوعية القادمة.

### 3.1 تجاوز مصفوفة التصعيد

| الوضع | المدة القصوى | الموافقون | الإخطارات المطلوبة |
|-----------|--------------|----------|------------------------|
| تحقيق مستأجر واحد (عدم تطابق السلطة المجزأة، العميل Sev2) | 4 ساعات | مهندس دعم + SRE عند الطلب | تذكرة `SUP-OVR-<id>`، حدث `android.telemetry.redaction.override`، سجل الحوادث |
| انقطاع القياس عن بعد على مستوى الأسطول أو إعادة إنتاج طلب SRE | 24 ساعة | SRE عند الطلب + قائد البرنامج | ملاحظة PagerDuty، تجاوز إدخال السجل، التحديث في `status.md` |
| طلب الامتثال/الطب الشرعي أو أي حالة تتجاوز 24 ساعة | حتى يتم إبطالها صراحة | مدير SRE + قائد الامتثال | القائمة البريدية للحوكمة، وسجل التجاوز، والحالة الأسبوعية AND7 |

#### مسؤوليات الدور| الدور | المسؤوليات | اتفاقية مستوى الخدمة / ملاحظات |
|------|------------------|------------|
| القياس عن بعد لنظام Android عند الطلب (قائد الحادث) | اكتشاف القيادة، وتنفيذ أدوات التجاوز، وتسجيل الموافقات في مستند الحادث، والتأكد من حدوث الإلغاء قبل انتهاء الصلاحية. | قم بالإقرار بـ PagerDuty خلال 5 دقائق وقم بتسجيل التقدم كل 15 دقيقة. |
| إمكانية ملاحظة Android TL (هاروكا ياماموتو) | التحقق من صحة إشارة الانجراف، وتأكيد حالة المصدر/المجمع، والتوقيع على بيان التجاوز قبل تسليمه إلى المشغلين. | انضم إلى الجسر في غضون 10 دقائق؛ تفويض إلى مالك المجموعة المرحلية إذا لم يكن متاحًا. |
| اتصال SRE (ليام أوكونور) | قم بتطبيق البيان على المجمعين ومراقبة الأعمال المتراكمة والتنسيق مع Release Engineering لعمليات التخفيف من جانب Torii. | قم بتسجيل كل إجراء `kubectl` في طلب التغيير ولصق نصوص الأوامر في مستند الحادث. |
| الامتثال (صوفيا مارتينز / دانيال بارك) | الموافقة على التجاوزات لمدة أطول من 30 دقيقة، والتحقق من صف سجل التدقيق، وتقديم المشورة بشأن مراسلة المنظم/العميل. | إقرار النشر في `#compliance-alerts`؛ بالنسبة لأحداث الإنتاج، قم بتقديم مذكرة امتثال قبل إصدار التجاوز. |
| مدير المستندات/الدعم (بريا ديشباندي) | أرشفة البيانات/مخرجات واجهة سطر الأوامر (CLI) ضمن `docs/source/sdk/android/readiness/…`، واحتفظ بسجل التجاوز مرتبًا، وقم بجدولة مختبرات المتابعة إذا ظهرت فجوات. | يؤكد الاحتفاظ بالأدلة (13 شهرًا) وملفات المتابعات AND7 قبل إغلاق الحادث. |

قم بالتصعيد على الفور إذا اقترب أي رمز تجاوز من انتهاء صلاحيته دون
خطة الإلغاء الموثقة

## 4. الاستجابة للحوادث

- **التنبيهات:** خدمة PagerDuty `android-telemetry-primary` تغطي التنقيح
  الأعطال، وانقطاع التيار الكهربائي عن المصدرين، وانجراف الدلو. الإقرار ضمن نوافذ SLA
  (راجع دليل قواعد اللعبة للدعم).
- **التشخيص:** قم بتشغيل `scripts/telemetry/check_redaction_status.py` للتجميع
  صحة المصدر الحالية، والتنبيهات الأخيرة، ومقاييس السلطة المجزأة. تضمين
  الإخراج في المخطط الزمني للحادث (`incident/YYYY-MM-DD-android-telemetry.md`).
- **لوحات المعلومات:** مراقبة تنقيح القياس عن بعد لنظام Android، والقياس عن بعد لنظام Android
  نظرة عامة، وامتثال التنقيح، ولوحات معلومات صحة المصدر. التقاط
  لقطات شاشة لسجلات الحوادث والتعليق على أي إصدار أو تجاوز
  الانحرافات الرمزية قبل إغلاق الحادث.
- ** التنسيق: ** إشراك هندسة الإصدار لقضايا المصدرين والامتثال
  لأسئلة التجاوز/معلومات تحديد الهوية الشخصية (PII)، ورئيس البرنامج لحوادث Sev 1.

### 4.1 تدفق التصعيد

يتم فرز حوادث Android باستخدام نفس مستويات الخطورة مثل Android
دعم قواعد اللعبة (§2.1). يلخص الجدول أدناه الأشخاص الذين يجب ترحيلهم وكيفية ذلك
ومن المتوقع بسرعة أن ينضم كل مستجيب إلى الجسر.| شدة | التأثير | المستجيب الأساسي (≥5 دقيقة) | التصعيد الثانوي (أقل من 10 دقائق) | إشعارات إضافية | ملاحظات |
|----------|--------|------------------------------------------|--------------------------------|----------|-------|
| سيف1 | الانقطاع الذي يواجهه العميل أو انتهاك الخصوصية أو تسرب البيانات | القياس عن بعد لنظام Android عند الطلب (`android-telemetry-primary`) | Torii عند الطلب + قائد البرنامج | الامتثال + حوكمة SRE (`#sre-governance`)، مالكي المجموعات المرحلية (`#android-staging`) | ابدأ تشغيل غرفة الحرب على الفور وافتح مستندًا مشتركًا لتسجيل الأوامر. |
| سيف2 | تدهور الأسطول أو تجاوز سوء الاستخدام أو تراكم إعادة التشغيل لفترة طويلة | القياس عن بعد لنظام Android عند الطلب | أسس Android TL + مدير المستندات/الدعم | قائد البرنامج، الاتصال الهندسي للإصدار | قم بالتصعيد إلى الامتثال إذا تجاوزت التجاوزات 24 ساعة. |
| سيف3 | مشكلة تتعلق بمستأجر واحد، أو بروفة معملية، أو تنبيه استشاري | مهندس دعم | Android عند الطلب (اختياري) | وثائق/دعم التوعية | قم بالتحويل إلى Sev2 إذا تم توسيع النطاق أو تأثر العديد من المستأجرين. |

| نافذة | العمل | المالك (المالكون) | الأدلة / الملاحظات |
|--------|--------|---------|----------------|
| 0–5 دقائق | أقر بـ PagerDuty، وقم بتعيين قائد الحادث (IC)، وقم بإنشاء `incident/YYYY-MM-DD-android-telemetry.md`. قم بإسقاط الارتباط بالإضافة إلى حالة السطر الواحد في `#android-sdk-support`. | عند الطلب SRE / مهندس الدعم | لقطة شاشة لـ PagerDuty ack + كعب الحادث الذي تم ارتكابه بجانب سجلات الحوادث الأخرى. |
| 5-15 دقيقة | قم بتشغيل `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status` والصق الملخص في مستند الحادث. Ping Android Observability TL (Haruka Yamamoto) وقائد الدعم (Priya Deshpande) للتسليم في الوقت الفعلي. | IC + Android إمكانية المراقبة TL | قم بإرفاق إخراج CLI بتنسيق JSON، ولاحظ فتح عناوين URL للوحة المعلومات، وحدد من يملك التشخيصات. |
| 15-25 دقيقة | قم بإشراك مالكي مجموعات التدريج (Haruka Yamamoto لقابلية المراقبة، وLiam O'Connor لـ SRE) لإعادة الإنتاج على `android-telemetry-stg`. قم بتحميل البذور باستخدام `scripts/telemetry/generate_android_load.sh --cluster android-telemetry-stg` وتفريغ قائمة انتظار الالتقاط من محاكي Pixel + لتأكيد تكافؤ الأعراض. | التدريج أصحاب الكتلة | قم بتحميل مخرجات `pending.queue` + `PendingQueueInspector` المعقمة إلى مجلد الحادث. |
| 25-40 دقيقة | حدد التجاوزات، أو اختناق Torii، أو خيار StrongBox الاحتياطي. في حالة الاشتباه في تعرض معلومات تحديد الهوية الشخصية (PII) أو التجزئة غير الحتمية، قم بصفحة الامتثال (Sofia Martins, Daniel Park) عبر `#compliance-alerts` وقم بإخطار قائد البرنامج في نفس سلسلة رسائل الحادث. | IC + الامتثال + قائد البرنامج | رموز تجاوز الارتباط وبيانات Norito وتعليقات الموافقة. |
| ≥40 دقيقة | توفير تحديثات الحالة لمدة 30 دقيقة (ملاحظات PagerDuty + `#android-sdk-support`). قم بجدولة جسر غرفة الحرب إذا لم يكن نشطًا بالفعل، وقم بتوثيق الوقت المتوقع للوصول، وتأكد من أن هندسة الإصدار (Alexei Morozov) في وضع الاستعداد لتجميع عناصر المجمع/SDK. | آي سي | التحديثات ذات الطابع الزمني بالإضافة إلى سجلات القرارات المخزنة في ملف الحادث والملخصة في `status.md` أثناء التحديث الأسبوعي التالي. |- يجب أن تظل جميع عمليات التصعيد معكوسة في مستند الحادث باستخدام جدول "المالك / وقت التحديث التالي" من دليل دعم Android.
- إذا كانت هناك حادثة أخرى مفتوحة بالفعل، فانضم إلى غرفة الحرب الحالية وألحق سياق Android بدلاً من إنشاء حادثة جديدة.
- عندما يمس الحادث فجوات دليل التشغيل، قم بإنشاء مهام متابعة في ملحمة AND7 JIRA ووضع علامة `telemetry-runbook`.

## 5. تمارين الفوضى والاستعداد

- تنفيذ السيناريوهات المفصلة في
  `docs/source/sdk/android/telemetry_chaos_checklist.md` ربع سنوي وقبل
  الإصدارات الرئيسية. سجل النتائج باستخدام قالب تقرير المختبر.
- تخزين الأدلة (لقطات الشاشة والسجلات) تحت
  `docs/source/sdk/android/readiness/screenshots/`.
- تتبع تذاكر العلاج في ملحمة AND7 ذات التصنيف `telemetry-lab`.
- خريطة السيناريو: C1 (خطأ التنقيح)، C2 (تجاوز)، C3 (انقطاع التصدير عن المصدر)، C4
  (بوابة فرق المخطط باستخدام `run_schema_diff.sh` مع التكوين المنجرف)، C5
  (تم تصنيف انحراف ملف تعريف الجهاز عبر `generate_android_load.sh`)، C6 (مهلة Torii)
  + إعادة تشغيل قائمة الانتظار)، C7 (رفض التصديق). حافظ على هذا الترقيم متوافقًا مع
  `telemetry_lab_01.md` وقائمة الفوضى عند إضافة التدريبات.

### 5.1 مثقاب الانجراف والتجاوز (C1/C2)

1. حقن فشل التجزئة عبر
   `scripts/telemetry/inject_redaction_failure.sh` وانتظر PagerDuty
   التنبيه (`android.telemetry.redaction.failure`). التقاط إخراج CLI من
   `scripts/telemetry/check_redaction_status.py --status-url <collector>` ل
   سجل الحادثة.
2. قم بمسح الفشل باستخدام `--clear` وتأكد من حل التنبيه في الداخل
   10 دقائق؛ قم بإرفاق لقطات شاشة Grafana للوحات الملح/السلطة.
3. قم بإنشاء طلب تجاوز موقع باستخدام
   `docs/examples/android_override_request.json`، قم بتطبيقه
   `scripts/android_override_tool.sh apply`، وتحقق من العينة غير المجزأة بواسطة
   فحص حمولة المصدر في التدريج (ابحث عن
   `android.telemetry.redaction.override`).
4. قم بإلغاء التجاوز باستخدام `scripts/android_override_tool.sh revoke --token <token>`،
   إلحاق تجزئة رمز التجاوز بالإضافة إلى إشارة التذكرة إلى
   `docs/source/sdk/android/telemetry_override_log.md`، وخلاصة JSON
   تحت `docs/source/sdk/android/readiness/override_logs/`. هذا يغلق
   سيناريو C2 في قائمة مراجعة الفوضى ويحافظ على أدلة الحوكمة متجددة.

### 5.2 تدريبات انقطاع التيار الكهربائي وإعادة تشغيل قائمة الانتظار (C3/C6)1. قم بتحجيم أداة التجميع المرحلية لأسفل (مقياس kubectl
   Publish/android-otel-collector --replicas=0`) لمحاكاة المصدر
   انقطاع التيار الكهربائي. تتبع مقاييس المخزن المؤقت عبر واجهة سطر الأوامر (CLI) للحالة وتأكد من إطلاق التنبيهات عندها
   علامة 15 دقيقة.
2. قم باستعادة المجمع، وتأكيد استنزاف الأعمال المتراكمة، وأرشفة سجل المجمع
   مقتطف يوضح اكتمال إعادة التشغيل.
3. على كل من Pixel المرحلي والمحاكي، اتبع السيناريو C6: التثبيت
   `examples/android/operator-console`، قم بتبديل وضع الطائرة، ثم أرسل العرض التوضيحي
   عمليات النقل، ثم قم بتعطيل وضع الطائرة وشاهد مقاييس عمق قائمة الانتظار.
4. اسحب كل قائمة انتظار معلقة (`adb shell run-as  cat files/pending.queue >
   /tmp/.queue`), compile the inspector (`gradle -p java/iroha_android
   :الأساسية:الفئات>/dev/null`), and run `java -cp build/classes
   org.hyperledger.iroha.android.tools.PendingQueueInspector --ملف
   /tmp/.queue --json > queue-replay-.json`. إرفاق فك التشفير
   المغلفات بالإضافة إلى إعادة التجزئة إلى سجل المختبر.
5. قم بتحديث تقرير الفوضى مع مدة انقطاع المصدر، وعمق قائمة الانتظار قبل / بعد،
   والتأكيد على أن `android_sdk_offline_replay_errors` بقي 0.

### 5.3 البرنامج النصي للفوضى العنقودية المرحلية (android-telemetry-stg)

مالكا المجموعة المرحلية هاروكا ياماموتو (Android Observability TL) وليام أوكونور
(SRE) اتبع هذا البرنامج النصي كلما تمت جدولة تشغيل البروفة. يستمر التسلسل
يتوافق المشاركون مع القائمة المرجعية لفوضى القياس عن بعد مع ضمان ذلك
يتم الاستيلاء على المصنوعات اليدوية للحكم.

**المشاركين**

| الدور | المسؤوليات | الاتصال |
|------|------------------|---------|
| أندرويد عند الطلب IC | يقود التدريب، وينسق ملاحظات PagerDuty، ويمتلك سجل الأوامر | بيجر ديوتي `android-telemetry-primary`، `#android-sdk-support` |
| أصحاب المجموعات المرحلية (هاروكا، ليام) | نوافذ تغيير البوابة، تشغيل إجراءات `kubectl`، القياس عن بعد لمجموعة اللقطات | `#android-staging` |
| مدير المستندات/الدعم (بريا) | تسجيل الأدلة، وتتبع قائمة مراجعة المختبرات، ونشر تذاكر المتابعة | `#docs-support` |

**التنسيق قبل الرحلة**

- قبل 48 ساعة من التدريب، قم بتقديم طلب تغيير يسرد ما هو مخطط له
  السيناريوهات (C1 – C7) والصق الارتباط في `#android-staging` حتى يتمكن مالكو المجموعة من
  يمكن أن يمنع عمليات النشر المتضاربة.
- اجمع أحدث تجزئة `ClientConfig` و`kubectl --context staging واحصل على البودات
  -n android-telemetry-stg` الإخراج لتحديد الحالة الأساسية ثم تخزينها
  كلاهما تحت `docs/source/sdk/android/readiness/labs/reports/<date>/`.
- التأكد من تغطية الجهاز (بيكسل + المحاكي) والتأكد من ذلك
  قام `ci/run_android_tests.sh` بتجميع الأدوات المستخدمة أثناء المختبر
  (`PendingQueueInspector`، حاقنات القياس عن بعد).

**نقاط التفتيش التنفيذ**

- أعلن عن "بدء الفوضى" في `#android-sdk-support`، وابدأ تسجيل الجسر،
  واحتفظ بـ `docs/source/sdk/android/telemetry_chaos_checklist.md` مرئيًا
  وكل أمر يرويه للكاتب.
- اطلب من مالك التدريج أن يعكس كل إجراء للمحقن (`kubectl scale`، المصدر
  إعادة التشغيل، وتحميل المولدات) لذلك يؤكد كل من Observability وSRE الخطوة.
- التقط الإخراج من scripts/telemetry/check_redaction_status.py
  --status-url https://android-telemetry-stg/api/redaction/status` بعد كل منها
  السيناريو والصقه في وثيقة الحادث.

** التعافي **- لا تترك الجسر حتى يتم مسح جميع المحاقن (`inject_redaction_failure.sh --clear`،
  تعرض لوحات المعلومات `kubectl scale ... --replicas=1`) وGrafana الحالة الخضراء.
- عمليات تفريغ قائمة انتظار أرشيفات المستندات/الدعم، وسجلات واجهة سطر الأوامر (CLI)، ولقطات الشاشة أسفل
  `docs/source/sdk/android/readiness/screenshots/<date>/` ووضع علامة على الأرشيف
  قائمة التحقق قبل إغلاق طلب التغيير.
- قم بتسجيل تذاكر المتابعة باستخدام الملصق `telemetry-chaos` لأي سيناريو
  فشل أو تم إنتاج مقاييس غير متوقعة، وقم بالإشارة إليها في `status.md`
  خلال المراجعة الأسبوعية القادمة.

| الوقت | العمل | المالك (المالكون) | قطعة أثرية |
|------|--------|----------|----------|
| T − 30 دقيقة | تحقق من صحة `android-telemetry-stg`: `kubectl --context staging get pods -n android-telemetry-stg`، وتأكد من عدم وجود ترقيات معلقة، ولاحظ إصدارات المجمع. | هاروكا | `docs/source/sdk/android/readiness/screenshots/<date>/cluster-health.png` |
| تي −20 دقيقة | تحميل خط الأساس للبذور (`scripts/telemetry/generate_android_load.sh --cluster android-telemetry-stg --duration 20m`) والتقاط stdout. | ليام | `readiness/labs/reports/<date>/load-generator.log` |
| تي − 15 دقيقة | انسخ `docs/source/sdk/android/readiness/incident/telemetry_chaos_template.md` إلى `docs/source/sdk/android/readiness/incident/<date>-telemetry-chaos.md`، وقم بإدراج السيناريوهات المطلوب تشغيلها (C1–C7)، وقم بتعيين الكتبة. | بريا ديشباندي (الدعم) | تم تنفيذ عملية تخفيض السعر قبل بدء التدريب. |
| تي −10 دقيقة | تأكد من محاكي Pixel + عبر الإنترنت، وتم تثبيت أحدث SDK، وقام `ci/run_android_tests.sh` بتجميع `PendingQueueInspector`. | هاروكا، ليام | `readiness/screenshots/<date>/device-checklist.png` |
| تي −5 دقيقة | ابدأ تشغيل Zoom Bridge، وابدأ تسجيل الشاشة، وأعلن "بدء الفوضى" في `#android-sdk-support`. | IC / المستندات / الدعم | تم حفظ التسجيل تحت `readiness/archive/<month>/`. |
| +0 دقيقة | قم بتنفيذ السيناريو المحدد من `docs/source/sdk/android/readiness/labs/telemetry_lab_01.md` (عادةً C2 + C6). اجعل دليل المختبر مرئيًا واستدعاء استدعاءات الأوامر فور حدوثها. | هاروكا يقود، ليام يعكس النتائج | السجلات المرفقة بملف الحادث في الوقت الحقيقي. |
| +15 دقيقة | توقف مؤقتًا لجمع المقاييس (`scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`) والتقاط لقطات شاشة Grafana. | هاروكا | `readiness/screenshots/<date>/status-<scenario>.png` |
| +25 دقيقة | استعادة أي حالات فشل تم إدخالها (`inject_redaction_failure.sh --clear`، `kubectl scale ... --replicas=1`)، وإعادة تشغيل قوائم الانتظار، وتأكيد إغلاق التنبيهات. | ليام | `readiness/labs/reports/<date>/recovery.log` |
| +35 دقيقة | استخلاص المعلومات: تحديث مستند الحادث بالنجاح/الفشل لكل سيناريو، وسرد المتابعات، ودفع العناصر إلى git. قم بإعلام Docs/Support بإمكانية إكمال قائمة التحقق من الأرشيف. | آي سي | تم تحديث مستند الحادث، وتم تحديد `readiness/archive/<month>/checklist.md`. |

- إبقاء أصحاب التدريج على الجسر حتى يصبح المصدرون بصحة جيدة ويتم مسح جميع التنبيهات.
- قم بتخزين عمليات تفريغ قائمة الانتظار الأولية في `docs/source/sdk/android/readiness/labs/reports/<date>/queues/` والإشارة إلى تجزئاتها في سجل الحوادث.
- في حالة فشل السيناريو، قم على الفور بإنشاء تذكرة JIRA تحمل الاسم `telemetry-chaos` وقم بربطها من `status.md`.
- مساعد الأتمتة: يقوم `ci/run_android_telemetry_chaos_prep.sh` بتغليف مولد الأحمال ولقطات الحالة وأنابيب تصدير قائمة الانتظار. قم بتعيين `ANDROID_TELEMETRY_DRY_RUN=false` عندما يكون الوصول المرحلي متاحًا و`ANDROID_PENDING_QUEUE_EXPORTS=pixel8=/tmp/pixel.queue,emulator=/tmp/emulator.queue` (إلخ.) بحيث يقوم البرنامج النصي بنسخ كل ملف قائمة انتظار، ويصدر `<label>.sha256`، ويقوم بتشغيل `PendingQueueInspector` لإنتاج `<label>.json`. استخدم `ANDROID_PENDING_QUEUE_INSPECTOR=false` فقط عندما يجب تخطي إصدار JSON (على سبيل المثال، لا يتوفر JDK). **قم دائمًا بتصدير معرفات الملح المتوقعة قبل تشغيل المساعد** عن طريق تعيين `ANDROID_TELEMETRY_EXPECTED_SALT_EPOCH=<YYYYQ#>` و`ANDROID_TELEMETRY_EXPECTED_SALT_ROTATION=<id>` بحيث تفشل مكالمات `check_redaction_status.py` المضمنة بسرعة إذا انحرف القياس عن بعد الذي تم التقاطه عن خط الأساس لـ Rust.

## 6. التوثيق والتمكين- **مجموعة تمكين المشغل:** `docs/source/sdk/android/readiness/and7_operator_enablement.md`
  يربط دليل التشغيل وسياسة القياس عن بعد ودليل المختبر وقائمة مراجعة الأرشيف والمعرفة
  يتحقق في حزمة واحدة جاهزة لـ AND7. الرجوع إليها عند إعداد SRE
  القراءة المسبقة للحوكمة أو جدولة التحديث ربع السنوي.
- **جلسات التمكين:** يتم تشغيل تسجيل التمكين لمدة 60 دقيقة بتاريخ 18-02-2026
  مع تحديثات ربع سنوية. المواد تعيش تحت
  `docs/source/sdk/android/readiness/`.
- **التحقق من المعرفة:** يجب أن يحصل الموظفون على درجة ≥90% من خلال نموذج الاستعداد. متجر
  النتائج في `docs/source/sdk/android/readiness/forms/responses/`.
- **التحديثات:** كلما كانت مخططات القياس عن بعد أو لوحات المعلومات أو سياسات التجاوز
  قم بتغيير وتحديث دليل التشغيل هذا وقواعد اللعب الخاصة بالدعم و`status.md` في نفس الوقت
  العلاقات العامة.
- **المراجعة الأسبوعية:** بعد كل إصدار مرشح لإصدار Rust (أو على الأقل أسبوعيًا)، تحقق
  `java/iroha_android/README.md` ودليل التشغيل هذا لا يزال يعكس الأتمتة الحالية،
  إجراءات تناوب التركيبات وتوقعات الحوكمة. التقاط المراجعة في
  `status.md` حتى يتمكن تدقيق المعالم الأساسية للأساسات من تتبع حداثة الوثائق.

## 7. أداة التصديق على StrongBox- **الغرض:** التحقق من صحة حزم التصديق المدعومة بالأجهزة قبل الترويج للأجهزة في
  تجمع StrongBox (AND2/AND6). يستهلك الحزام سلاسل الشهادات الملتقطة ويتحقق منها
  ضد الجذور الموثوقة باستخدام نفس السياسة التي ينفذها كود الإنتاج.
- **المرجع:** راجع `docs/source/sdk/android/strongbox_attestation_harness_plan.md` للاطلاع على النص الكامل
  التقاط واجهة برمجة التطبيقات (API)، ودورة حياة الاسم المستعار، وأسلاك CI/Buildkite، ومصفوفة الملكية. تعامل مع هذه الخطة على أنها
  مصدر الحقيقة عند تأهيل فنيي مختبر جدد أو تحديث عناصر التمويل/الامتثال.
- **سير العمل:**
  1. اجمع حزمة التصديق على الجهاز (الاسم المستعار، `challenge.hex`، و`chain.pem` مع
     leaf → ترتيب الجذر) وانسخه إلى محطة العمل.
  2. قم بتشغيل scripts/android_keystore_attestation.sh --bundle-dir  --trust-root 
     [--trust-root-dir ] --require-strongbox --output ` باستخدام الأمر المناسب
     Google/Samsung root (تسمح لك الدلائل بتحميل حزم البائعين بأكملها).
  3. أرشفة ملخص JSON جنبًا إلى جنب مع مواد التصديق الأولية في
     `artifacts/android/attestation/<device-tag>/`.
- **تنسيق الحزمة:** اتبع `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  لتخطيط الملف المطلوب (`chain.pem`، `challenge.hex`، `alias.txt`، `result.json`).
- **الجذور الموثوقة:** احصل على PEMs التي يوفرها البائع من متجر أسرار معمل الجهاز؛ تمرير متعددة
  وسيطات `--trust-root` أو أشر `--trust-root-dir` إلى الدليل الذي يحتفظ بنقاط الارتساء عندما
  تنتهي السلسلة بمرساة غير تابعة لـ Google.
- **أداة CI:** استخدم `scripts/android_strongbox_attestation_ci.sh` للتحقق من الحزم المؤرشفة دفعة واحدة
  على أجهزة المختبر أو العدائين CI. يقوم البرنامج النصي بمسح `artifacts/android/attestation/**` ويستدعي ملف
  تسخير لكل دليل يحتوي على الملفات الموثقة، وكتابة `result.json` المحدثة
  ملخصات في مكانها.
- **مسار CI:** بعد مزامنة الحزم الجديدة، قم بتشغيل خطوة Buildkite المحددة في
  `.buildkite/android-strongbox-attestation.yml` (`buildkite-agent pipeline upload --pipeline .buildkite/android-strongbox-attestation.yml`).
  يتم تنفيذ المهمة `scripts/android_strongbox_attestation_ci.sh`، وإنشاء ملخص باستخدام
  `scripts/android_strongbox_attestation_report.py`، يقوم بتحميل التقرير إلى `artifacts/android_strongbox_attestation_report.txt`،
  ويعلق على البنية كـ `android-strongbox/report`. التحقيق في أي فشل على الفور و
  ربط عنوان URL للبناء من مصفوفة الجهاز.
- **التقارير:** قم بإرفاق مخرجات JSON بمراجعات الإدارة وتحديث إدخال مصفوفة الجهاز
  `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` مع تاريخ التصديق.
- **بروفة وهمية:** عند عدم توفر الأجهزة، قم بتشغيل `scripts/android_generate_mock_attestation_bundles.sh`
  (الذي يستخدم `scripts/android_mock_attestation_der.py`) لسك حزم الاختبار الحتمية بالإضافة إلى جذر وهمي مشترك حتى يتمكن CI وdocs من ممارسة الحزام من البداية إلى النهاية.
- **حواجز الحماية المضمنة في الكود:** `ci/run_android_tests.sh --tests
  org.hyperledger.iroha.android.crypto.keystore.KeystoreKeyProviderTests` يغطي فارغة مقابل تحدى
  تجديد الشهادة (بيانات تعريف StrongBox/TEE) وإصدار `android.keystore.attestation.failure`
  عند عدم تطابق التحدي، يتم اكتشاف تراجعات ذاكرة التخزين المؤقت/القياس عن بعد قبل شحن الحزم الجديدة.

## 8. جهات الاتصال

- **دعم الهندسة عند الطلب:** `#android-sdk-support`
- **حوكمة SRE:** `#sre-governance`
- **المستندات/الدعم:** `#docs-support`
- **شجرة التصعيد:** راجع دليل التشغيل لدعم Android §2.1

## 9. سيناريوهات استكشاف الأخطاء وإصلاحهايستدعي عنصر خريطة الطريق AND7-P2 ثلاث فئات من الحوادث التي تقوم بصفحة الملف بشكل متكرر
Android عند الطلب: مهلة Torii/الشبكة، وفشل التصديق على StrongBox، و
`iroha_config` الانجراف الواضح. العمل من خلال القائمة المرجعية ذات الصلة قبل التقديم
متابعة Sev1/2 وأرشفة الأدلة في `incident/<date>-android-*.md`.

### 9.1 Torii ومهلة الشبكة

**الإشارات**

- تنبيهات على `android_sdk_submission_latency`، `android_sdk_pending_queue_depth`،
  `android_sdk_offline_replay_errors`، ومعدل الخطأ Torii `/v2/pipeline`.
- عناصر واجهة مستخدم `operator-console` (أمثلة/Android) تعرض استنزاف قائمة الانتظار المتوقفة أو
  إعادة المحاولة عالقة في التراجع الأسي.

**الرد الفوري**

1. أقر بـ PagerDuty (`android-networking`) وابدأ في تسجيل الحوادث.
2. التقط لقطات Grafana (زمن انتقال الإرسال + عمق قائمة الانتظار) التي تغطي
   آخر 30 دقيقة.
3. قم بتسجيل تجزئة `ClientConfig` النشطة من سجلات الجهاز (`ConfigWatcher`
   يطبع ملخص البيان عندما تنجح عملية إعادة التحميل أو تفشل).

** التشخيص **

- **سلامة قائمة الانتظار:** اسحب ملف قائمة الانتظار الذي تم تكوينه من جهاز التدريج أو الملف
  المحاكي (`adb shell run-as  cat files/pending.queue >
  /tmp/pending.queue`). فك رموز المظاريف مع
  `OfflineSigningEnvelopeCodec` كما هو موضح في
  `docs/source/sdk/android/offline_signing.md#4-queueing--replay` لتأكيد
  يتطابق التراكم مع توقعات المشغل. قم بإرفاق التجزئة التي تم فك تشفيرها إلى
  حادثة.
- **مستودع التجزئة:** بعد تنزيل ملف قائمة الانتظار، قم بتشغيل مساعد المفتش
  لالتقاط التجزئات/الأسماء المستعارة الأساسية للعناصر الحادثة:

  ```bash
  gradle -p java/iroha_android :core:classes >/dev/null  # compiles classes if needed
  java -cp build/classes org.hyperledger.iroha.android.tools.PendingQueueInspector \
    --file /tmp/pending.queue --json > queue-inspector.json
  ```

  قم بإرفاق `queue-inspector.json` والنموذج المطبوع بشكل جميل بالحادث
  وربطه من تقرير مختبر AND7 للسيناريو د.
- **اتصال Torii:** قم بتشغيل أداة نقل HTTP محليًا لاستبعاد SDK
  الانحدارات: تمارين `ci/run_android_tests.sh`
  `HttpClientTransportTests`، و`HttpClientTransportHarnessTests`، و
  `ToriiMockServerTests`. تشير حالات الفشل هنا إلى خطأ في العميل وليس إلى خطأ
  انقطاع Torii.
- **تجربة حقن الخطأ:** على البكسل المرحلي (StrongBox) وAOSP
  المحاكي، قم بتبديل الاتصال لإعادة إنتاج نمو قائمة الانتظار المعلقة:
  `adb shell cmd connectivity airplane-mode enable` → أرسل عرضين توضيحيين
  المعاملات عبر وحدة تحكم المشغل → وضع الطائرة للاتصال بـ adb shell cmd
  Disable` → verify the queue drains and `android_sdk_offline_replay_errors`
  يبقى 0. سجل تجزئات المعاملات المعاد تشغيلها.
- **تكافؤ التنبيه:** عند ضبط الحدود أو بعد تغيير Torii، قم بالتنفيذ
  `scripts/telemetry/test_torii_norito_rpc_alerts.sh` لذا تبقى قواعد Prometheus
  تتماشى مع لوحات المعلومات.

** التعافي **

1. إذا تدهور مستوى Torii، قم بتشغيل Torii عند الطلب واستمر في إعادة تشغيل
   قائمة الانتظار بمجرد أن يقبل `/v2/pipeline` حركة المرور.
2. قم بإعادة تكوين العملاء المتأثرين فقط عبر بيانات `iroha_config` الموقعة. ال
   يجب أن يقوم مراقب التحميل السريع `ClientConfig` بإصدار سجل نجاح قبل وقوع الحادث
   يمكن أن تغلق.
3. قم بتحديث الحادثة بحجم قائمة الانتظار قبل/بعد إعادة التشغيل بالإضافة إلى تجزئات
   أي المعاملات المسقطة.

### 9.2 فشل الصندوق القوي والتصديق

**الإشارات**- تنبيهات على `android_sdk_strongbox_success_rate` أو
  `android.keystore.attestation.failure`.
- يقوم القياس عن بعد `android.keystore.keygen` الآن بتسجيل المطلوب
  `KeySecurityPreference` والمسار المستخدم (`strongbox`، `hardware`،
  `software`) مع علامة `fallback=true` عندما يصل تفضيل StrongBox إلى
  تي إي/البرمجيات. طلبات STRONGBOX_REQUIRED تفشل الآن بسرعة بدلاً من أن تكون بصمت
  إرجاع مفاتيح TEE.
- تذاكر الدعم التي تشير إلى أجهزة `KeySecurityPreference.STRONGBOX_ONLY`
  العودة إلى مفاتيح البرامج.

**الرد الفوري**

1. قم بالتعرف على PagerDuty (`android-crypto`) والتقاط تسمية الاسم المستعار المتأثرة
   (تجزئة مملحة) بالإضافة إلى دلو ملف تعريف الجهاز.
2. التحقق من إدخال مصفوفة التصديق الخاصة بالجهاز
   `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` و
   سجل آخر تاريخ تم التحقق منه.

** التشخيص **

- **التحقق من الحزمة:** تشغيل
  `scripts/android_keystore_attestation.sh --bundle-dir <bundle> --trust-root <root.pem>`
  على الشهادة المؤرشفة لتأكيد ما إذا كان الفشل بسبب الجهاز
  التكوين الخاطئ أو تغيير السياسة. قم بإرفاق `result.json` الذي تم إنشاؤه.
- **متجدد التحدي:** لا يتم تخزين التحديات مؤقتًا. كل طلب التحدي يجدد جديدة
  التصديق وذاكرة التخزين المؤقت بواسطة `(alias, challenge)`؛ تعمل المكالمات بدون تحدي على إعادة استخدام ذاكرة التخزين المؤقت. غير مدعوم
- **مسح CI:** قم بتنفيذ `scripts/android_strongbox_attestation_ci.sh` لذلك كل
  يتم إعادة التحقق من صحة الحزمة المخزنة؛ هذا حراس ضد القضايا النظامية المقدمة
  بواسطة مراسي الثقة الجديدة.
- **التمرين على الجهاز:** على الأجهزة التي لا تحتوي على StrongBox (أو عن طريق تشغيل المحاكي)،
  قم بتعيين SDK ليتطلب StrongBox فقط، ثم أرسل معاملة تجريبية، ثم قم بالتأكيد
  يُصدر مصدر القياس عن بعد الحدث `android.keystore.attestation.failure`
  مع السبب المتوقع. كرر ذلك على جهاز Pixel قادر على استخدام StrongBox لضمان
  الطريق السعيد يبقى أخضر.
- **فحص انحدار SDK:** قم بتشغيل `ci/run_android_tests.sh` وادفع
  الاهتمام بالأجنحة التي تركز على التصديق (`AndroidKeystoreBackendDetectionTests`،
  `AttestationVerifierTests`، `IrohaKeyManagerDeterministicExportTests`،
  `KeystoreKeyProviderTests` لفصل ذاكرة التخزين المؤقت/التحدي). الفشل هنا
  تشير إلى الانحدار من جانب العميل.

** التعافي **

1. قم بإعادة إنشاء حزم التصديق إذا قام البائع بتدوير الشهادات أو إذا كان
   تلقى الجهاز مؤخرًا OTA رئيسيًا.
2. قم بتحميل الحزمة المحدثة إلى `artifacts/android/attestation/<device>/` و
   تحديث إدخال المصفوفة بالتاريخ الجديد.
3. إذا لم يكن StrongBox متوفرًا في الإنتاج، فاتبع سير عمل التجاوز في
   القسم 3 وتوثيق المدة الاحتياطية؛ يتطلب التخفيف على المدى الطويل
   استبدال الجهاز أو إصلاح البائع.

### 9.2أ استرداد الصادرات الحتمية

- **التنسيقات:** الصادرات الحالية هي v3 (ملح/نونس لكل تصدير + Argon2id، مسجلة كـ
- **سياسة عبارة المرور:** يفرض الإصدار 3 عبارة مرور مكونة من ≥12 حرفًا. إذا كان المستخدمون يوفرون أقصر
  عبارات المرور، واطلب منهم إعادة التصدير باستخدام عبارة مرور متوافقة؛ واردات v0/v1 هي
  معفى ولكن يجب إعادة تغليفه كـ v3 مباشرة بعد الاستيراد.
- **حماية العبث/إعادة الاستخدام:** ترفض أجهزة فك التشفير الأطوال الصفرية/الملحية القصيرة أو الأطوال غير المتساوية وتتكرر
  تظهر أزواج الملح/النونس كأخطاء `salt/nonce reuse`. أعد إنشاء التصدير لمسحه
  الحارس؛ لا تحاول فرض إعادة الاستخدام.
  `SoftwareKeyProvider.importDeterministic(...)` لإعادة ترطيب المفتاح، إذن
  `exportDeterministic(...)` لإصدار حزمة v3 حتى تقوم أدوات سطح المكتب بتسجيل KDF الجديد
  المعلمات.### 9.3 عدم تطابق البيان والتكوين

**الإشارات**

- فشل إعادة تحميل `ClientConfig`، أو أسماء مضيف Torii غير متطابقة، أو القياس عن بعد
  تم وضع علامة على اختلافات المخطط بواسطة أداة الفرق AND7.
- يقوم المشغلون بالإبلاغ عن مقابض إعادة المحاولة/التراجع المختلفة عبر الأجهزة في نفس الوقت
  أسطول.

**الرد الفوري**

1. التقط ملخص `ClientConfig` المطبوع في سجلات Android و
   الملخص المتوقع من بيان الإصدار.
2. قم بتفريغ تكوين العقدة قيد التشغيل للمقارنة:
   `iroha_cli config show --actual > /tmp/iroha_config.actual.json`.

** التشخيص **

- **Schema diff:** تشغيل scripts/telemetry/run_schema_diff.sh --android-config
   --rust-config  --textfile-dir /var/lib/node_exporter/textfile_collector`
  لإنشاء تقرير فرق Norito، قم بتحديث الملف النصي Prometheus، وأرفق
  قطعة JSON بالإضافة إلى أدلة المقاييس للحادث وسجل جاهزية القياس عن بعد AND7.
- **التحقق من صحة البيان:** استخدم `iroha_cli runtime capabilities` (أو وقت التشغيل
  أمر التدقيق) لاسترداد تجزئة التشفير/ABI المُعلن عنها للعقدة والتأكد من ذلك
  أنها تتطابق مع بيان المحمول. يؤكد عدم التطابق أن العقدة قد تم إرجاعها مرة أخرى
  دون إعادة إصدار بيان Android.
- **فحص انحدار SDK:** أغطية `ci/run_android_tests.sh`
  `ClientConfigNoritoRpcTests`، و`ClientConfig.ValidationTests`، و
  `HttpClientTransportStatusTests`. تشير الأعطال إلى أن حزمة SDK التي تم شحنها لا يمكنها ذلك
  تحليل تنسيق البيان المنشور حاليًا.

** التعافي **

1. قم بإعادة إنشاء البيان عبر المسار المعتمد (عادةً
   `iroha_cli runtime Capabilities` → بيان Norito الموقّع → حزمة التكوين) و
   إعادة نشره من خلال قناة المشغل. لا تقم أبدًا بتحرير `ClientConfig`
   يتجاوز على الجهاز.
2. بمجرد وصول البيان المصحح، انتبه إلى `ConfigWatcher` "reload ok"
   رسالة على كل مستوى من مستويات الأسطول ولا يتم إغلاق الحادث إلا بعد القياس عن بعد
   مخطط فرق تقارير التكافؤ.
3. سجل تجزئة البيان ومسار الاختلاف في المخطط ورابط الحادث
   `status.md` ضمن قسم Android لإمكانية التدقيق.

## 10. منهج تمكين المشغلين

يتطلب عنصر خريطة الطريق **AND7** حزمة تدريب قابلة للتكرار حتى يتمكن المشغلون،
يمكن لمهندسي الدعم وSRE اعتماد تحديثات القياس عن بعد/التنقيح بدون
التخمين. إقران هذا القسم مع
`docs/source/sdk/android/readiness/and7_operator_enablement.md`، والذي يحتوي على
قائمة المراجعة التفصيلية وروابط القطع الأثرية.

### 10.1 وحدات الجلسة (إحاطة مدتها 60 دقيقة)

1. ** هندسة القياس عن بعد (15 دقيقة). ** قم بالتجول عبر المخزن المؤقت للمصدر،
   مرشح التنقيح وأدوات فرق المخطط. تجريبي
   `scripts/telemetry/run_schema_diff.sh --textfile-dir /var/lib/node_exporter/textfile_collector` زائد
   `scripts/telemetry/check_redaction_status.py` حتى يرى الحاضرون مدى التكافؤ
   القسري.
2. **دليل التدريب + مختبرات الفوضى (20 دقيقة).** قم بتمييز الأقسام من 2 إلى 9 من هذا الدليل،
   تدرب على سيناريو واحد من `readiness/labs/telemetry_lab_01.md`، واشرح كيفية القيام بذلك
   لأرشفة القطع الأثرية تحت `readiness/labs/reports/<stamp>/`.
3. **التجاوز + سير عمل الامتثال (10 دقائق).** مراجعة تجاوزات القسم 3،
   إثبات `scripts/android_override_tool.sh` (تطبيق/إلغاء/ملخص)، و
   التحديث `docs/source/sdk/android/telemetry_override_log.md` بالإضافة إلى الأحدث
   هضم JSON.
4. **الأسئلة والأجوبة / التحقق من المعرفة (15 دقيقة).** استخدم البطاقة المرجعية السريعة
   `readiness/cards/telemetry_redaction_qrc.md` لترسيخ الأسئلة، إذن
   التقاط المتابعات في `readiness/and7_operator_enablement.md`.### 10.2 إيقاع الأصول وأصحابها

| الأصول | الإيقاع | المالك (المالكون) | موقع الأرشيف |
|-------|---------|---------|------------------|
| الإرشادات المسجلة (تكبير / فرق) | ربع سنوية أو قبل كل دورة ملح | إمكانية ملاحظة Android TL + مدير المستندات/الدعم | `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` (التسجيل + قائمة المراجعة) |
| سطح الشريحة وبطاقة مرجعية سريعة | قم بالتحديث كلما تغيرت السياسة/دليل التشغيل | المستندات/مدير الدعم | `docs/source/sdk/android/readiness/deck/` و`/cards/` (تصدير PDF + تخفيض السعر) |
| فحص المعرفة + ورقة الحضور | بعد كل جلسة مباشرة | هندسة الدعم | كتلة الحضور `docs/source/sdk/android/readiness/forms/responses/` و`and7_operator_enablement.md` |
| الأسئلة والأجوبة المتراكمة / سجل الإجراءات | المتداول. يتم تحديثه بعد كل جلسة | ماجستير في القانون (بالنيابة DRI) | `docs/source/sdk/android/readiness/and7_operator_enablement.md` §6 |

### 10.3 حلقة الأدلة والملاحظات

- تخزين عناصر الجلسة (لقطات الشاشة، وتدريبات الأحداث، وصادرات الاختبارات) في
  نفس الدليل المؤرخ المستخدم في تدريبات الفوضى حتى تتمكن الإدارة من تدقيق كليهما
  مسارات الاستعداد معًا.
- عند اكتمال الجلسة، قم بتحديث `status.md` (قسم Android) بروابط إلى
  دليل الأرشيف ولاحظ أي متابعات مفتوحة.
- يجب تحويل الأسئلة المعلقة من الأسئلة والأجوبة المباشرة إلى قضايا أو مستند
  سحب الطلبات خلال أسبوع واحد؛ قم بالإشارة إلى ملاحم خارطة الطريق (AND7/AND8) في
  وصف التذكرة حتى يبقى المالكون على توافق.
- تقوم مزامنة SRE بمراجعة قائمة مراجعة الأرشيف بالإضافة إلى عناصر فرق المخطط المدرجة في
  القسم 2.3 قبل الإعلان عن إغلاق المنهج لهذا الربع.