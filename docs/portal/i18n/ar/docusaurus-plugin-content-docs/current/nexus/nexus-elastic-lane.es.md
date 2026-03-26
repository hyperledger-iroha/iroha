---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-elastic-lane.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: حارة العلاقة المرنة
العنوان: توفير الممرات المرنة (NX-7)
Sidebar_label: توفير المسار المرن
الوصف: تدفق التمهيد لإنشاء بيانات المسار Nexus وإدخال الكتالوج وأدلة الطرح.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/nexus_elastic_lane.md`. احتفظ بنسخ متجددة حتى يصل حاجز التوطين إلى البوابة.
:::

# مجموعة توفير الممرات المرنة (NX-7)

> **عنصر خريطة الطريق:** NX-7 - أدوات توفير المسار المرن  
> **الحالة:** الأدوات كاملة - أنواع البيانات، مقتطفات الكتالوج، الحمولات Norito، الاختبارات البشرية،
> والمساعد في اختبار التحميل المجمع الآن يجمع بين زمن الوصول للفتحة + أدلة الأدلة التي تؤكد أن ممرات التحقق من الشحنات
> إذا تم نشر الخطيئة في البرمجة النصية على الوسائط.

يساعد هذا الدليل مشغلي `scripts/nexus_lane_bootstrap.sh` الجديد على أتمتة إنشاء بيان المسار ومقتطفات كتالوج المسار/مساحة البيانات وأدلة الطرح. الهدف هو تسهيل الممرات الجديدة العالية Nexus (العامة أو الخاصة) دون تحرير أرشيفات متعددة يدويًا أو استعادة هندسة الكتالوج يدويًا.

## 1. المتطلبات الأساسية

1. الموافقة على إدارة الأسماء المستعارة للمسار ومساحة البيانات ومجموعة المصادقين والتسامح مع السقوط (`f`) وسياسة التسوية.
2. قائمة نهائية للمدققين (معرفات الحساب) وقائمة مساحات الأسماء المحمية.
3. قم بالوصول إلى مستودع تكوين العقدة لتتمكن من إضافة المقتطفات التي تم إنشاؤها.
4. قواعد تسجيل بيانات المسار (الإصدار `nexus.registry.manifest_directory` و`cache_directory`).
5. جهات الاتصال الخاصة بالقياس عن بعد/مقابض PagerDuty للمسار، بحيث يتم توصيل التنبيهات أثناء اتصال المسار بالإنترنت.

## 2. أنواع الممرات الأثرية

قم بتشغيل المساعد من مصدر المستودع:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <katakana-i105-account-id> \
  --validator <katakana-i105-account-id> \
  --validator <katakana-i105-account-id> \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

الأعلام العصا:

- يجب أن يتزامن `--lane-id` مع فهرس الإدخال الجديد في `nexus.lane_catalog`.
- `--dataspace-alias` y `--dataspace-id/hash` يتحكم في إدخال كتالوج مساحة البيانات (عند استخدام معرف المسار عند حذفه).
- `--validator` يمكنك التكرار أو القراءة من `--validators-file`.
- `--route-instruction` / `--route-account` يصدر قوائم التشغيل الخاصة بالتثبيت.
- `--metadata key=value` (o `--telemetry-contact/channel/runbook`) يلتقط جهات الاتصال في دليل التشغيل حتى تعرض لوحات المعلومات المالكين الصحيحين.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` يدعم ربط ترقية وقت التشغيل بالبيان عندما يتطلب المسار ضوابط موسعة للمشغلين.
- `--encode-space-directory` يتم استدعاء `cargo xtask space-directory encode` تلقائيًا. قم بالدمج مع `--space-directory-out` عندما تريد أن يكون الملف `.to` مشفرًا في مكان منفصل افتراضي.

يُنتج البرنامج النصي ثلاثة مصنوعات داخل `--output-dir` (بسبب خلل في الدليل الفعلي)، ولكن هناك ربع اختياري عندما يكون الترميز قادرًا على:

1.`<slug>.manifest.json` - بيان المسار الذي يحتوي على نصاب المصادقين ومساحات الأسماء المحمية وبيانات التعريف الاختيارية لخطاف ترقية وقت التشغيل.
2. `<slug>.catalog.toml` - مقتطف TOML مع `[[nexus.lane_catalog]]` و`[[nexus.dataspace_catalog]]` وأي نظام إدخال مطلوب. تأكد من أن `fault_tolerance` تم تكوينه في مدخل مساحة البيانات لأبعاد وحدة ترحيل المسار (`3f+1`).
3.`<slug>.summary.json` - استئناف الاستماع الذي يصف الهندسة (الحلقة والقطاعات والبيانات الوصفية) بالإضافة إلى خطوات الطرح المطلوبة والأمر الدقيق لـ `cargo xtask space-directory encode` (أسفل `space_directory_encode.command`). هذا هو JSON الملحق بتذكرة الإعداد كأدلة.
4.`<slug>.manifest.to` - يتم إصداره عندما يكون `--encode-space-directory` نشطًا؛ قائمة التدفق `iroha app space-directory manifest publish` de Torii.

استخدم `--dry-run` لمعاينة JSON/المقتطفات بدون كتابة أرشيفات، و`--force` لنسخ العناصر الموجودة.

## 3. تطبيق التغييرات

1. انسخ بيان JSON في `nexus.registry.manifest_directory` الذي تم تكوينه (في ذاكرة التخزين المؤقت للدليل إذا كان السجل يعكس الحزم عن بعد). قم بإنشاء الملف إذا تم إصدار البيانات في مستودع التكوين الخاص بك.
2. قم بتوصيل مقتطف الكتالوج إلى `config/config.toml` (أو إلى `config.d/*.toml` المقابل). تأكد من أن `nexus.lane_count` هو أقل من `lane_id + 1` وقم بتحديث أي `nexus.routing_policy.rules` الذي سيتعين عليك فتح المسار الجديد.
3. الكوديفيكا (إذا حذفت `--encode-space-directory`) ونشر البيان في دليل الفضاء باستخدام الأمر الملتقط في الملخص (`space_directory_encode.command`). قم بإنتاج الحمولة `.manifest.to` التي تتوقعها Torii وتسجيل الأدلة للمراجعين؛ إنفيا كون `iroha app space-directory manifest publish`.
4. قم بتشغيل `irohad --sora --config path/to/config.toml --trace-config` وحفظ خروج التتبع في تذكرة التشغيل. من المحتمل أن تتزامن الهندسة الجديدة مع سبيكة/أجزاء كورا التي تم إنشاؤها.
5. قم باستعادة المصدقين المعينين في نفس الوقت عندما تكون تغييرات البيان/الكتالوج غير صالحة. احفظ ملخص JSON في التذكرة للاستماع إلى المستقبل.

## 4. إنشاء حزمة توزيع السجل

قم بتضمين البيان الذي تم إنشاؤه والتراكب حتى يتمكن المشغلون من توزيع البيانات على إدارة الممرات دون تحرير التكوينات في كل مضيف. يظهر مساعد التجميع في التخطيط الكنسي، وينتج تراكبًا اختياريًا لكتالوج الإدارة لـ `nexus.registry.cache_directory`، ويمكنه إصدار كرة قطرانية لعمليات النقل دون اتصال:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

ساليداس:

1.`manifests/<slug>.manifest.json` - نسخ هذه الأرشيفات إلى تكوين `nexus.registry.manifest_directory`.
2.`cache/governance_catalog.json` - هذا هو `nexus.registry.cache_directory`. يتم تحويل كل إدخال `--module` إلى تعريف للوحدة القابلة للتشغيل، مما يسمح بمبادلة وحدة الإدارة (NX-2) من خلال تحديث تراكب ذاكرة التخزين المؤقت بدلاً من تحرير `config.toml`.
3.`summary.json` - يتضمن التجزئات وبيانات تعريف التراكب وتعليمات التشغيل.
4. اختياري `registry_bundle.tar.*` - قائمة SCP أو S3 أو متتبعات المصنوعات اليدوية.

قم بمزامنة كل الدليل (أو الأرشيف) مع كل أداة التحقق، والمضيفين الإضافيين، وانسخ البيانات + تراكب ذاكرة التخزين المؤقت في مسارات السجل الخاصة بك قبل إعادة تشغيل Torii.

## 5. اختبار الإنسان للمصادقات

بعد إعادة تشغيل Torii، قم بتشغيل مساعد الدخان الجديد للتحقق من أن الخط يبلغ عن `manifest_ready=true`، مما يؤدي إلى ظهور المقاييس على محتوى الممرات المحترق، كما أن مقياس الغلق مغلق. يجب أن توضح الممرات التي تتطلب البيانات `manifest_path` بدون فراغ؛ سيفشل المساعد الآن في الوساطة عندما يفشل المسار حتى يتم تضمين كل ما يتم نشره من NX-7 دليلاً على البيان الثابت:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Agrega `--insecure` عندما يتم اختباره بشهادات موقعة ذاتيًا. ينتهي البرنامج النصي بكود غير مشفر إذا كان المسار خاطئًا، أو أنه مغلق أو يتم إزالة المقاييس/القياس عن بعد من القيم المتبددة. تستخدم المقابض `--min-block-height` و`--max-finality-lag` و`--max-settlement-backlog` و`--max-headroom-events` للحفاظ على القياس عن بعد للمسار (ارتفاع الكتلة/النهاية/التراكم/الإرتفاع) داخل حدود عملياتك، ومجموعات مع `--max-slot-p95` / `--max-slot-p99` (mas `--min-slot-samples`) لاستدعاء أهداف متانة الفتحة NX-18 بدون إخراج المساعد.

للتحقق من صحة الهواء (o CI) يمكن إعادة إنتاج استجابة Torii التي تم التقاطها بعيدًا عن نقطة النهاية الحية:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

تم التقاط التركيبات تحت `fixtures/nexus/lanes/` لتعكس المصنوعات التي تم إنتاجها بواسطة مساعد التمهيد بحيث يمكن للبيانات الجديدة أن تكون سهلة الاستخدام في البرمجة النصية. يقوم CI بتنفيذ التدفق نفسه عبر `ci/check_nexus_lane_smoke.sh` و`ci/check_nexus_lane_registry_bundle.sh` (الاسم المستعار: `make check-nexus-lanes`) لإظهار أن مساعد الدخان NX-7 يعمل بشكل منفصل مع تنسيق الحمولة المنشور ولضمان الحفاظ على الحزم/التراكبات القابلة للتكرار.