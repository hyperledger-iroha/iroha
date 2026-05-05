---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-elastic-lane.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: حارة العلاقة المرنة
العنوان: لچکدار حارة پروویژنگ (NX-7)
Sidebar_label: لچکدار حارة پروویژنگ
الوصف: بيانات المسار Nexus، وإدخالات الكتالوج، وأدلة التشغيل.
---

:::ملاحظة المصدر الكنسي
هذه هي الصفحة `docs/source/nexus_elastic_lane.md`. عندما تقوم بترجمة المنفذ، فلا داعي للقلق بشأن محاذاة الفيديو.
:::

# لچکدار حارة پرویژنگ ٹول كٹ (NX-7)

> **عنصر خريطة الطريق:** NX-7 - أدوات لچکدار حارة پروویژنگ  
> **الحالة:** الأدوات مکمل - البيانات، مقتطفات الكتالوج، حمولات Norito، اختبارات الدخان بناتا ہے،
> ومساعد حزمة اختبار التحميل، وبوابة زمن الوصول للفتحة، وبيانات الأدلة التي تساعد في التحقق من صحة عمليات التحميل
> بغداد مخصوص البرمجة النصية کے شائعة کیے جا سکیں۔

يقوم مشغلو هذه الأجهزة بتوفير مساعد `scripts/nexus_lane_bootstrap.sh` للبوابات وإنشاء بيان الممرات، ومقتطفات كتالوج المسار/مساحة البيانات، وأدلة الطرح التي نحتاجها. تهدف هذه الفكرة إلى ممرات Nexus الجديدة (العامة أو الخاصة) إلى إنشاء أقسام متعددة لتحرير الصفحة وهندسة الكتالوج بشكل متكرر مما يؤدي إلى استخلاص إرشادات سهلة الاستخدام.

## 1. المتطلبات الأساسية

1. الاسم المستعار للمسار، مساحة البيانات، مجموعة المدقق، التسامح مع الخطأ (`f`)، وسياسة التسوية موافقة الإدارة.
2. يتم التحقق من صحة المدققين (معرفات الحساب) ومساحات الأسماء المحمية.
3. يحتوي مستودع تكوين العقدة على المقتطفات التي تم إنشاؤها والتي تتضمن كل شيء.
4. سجل بيان المسار للمسارات (`nexus.registry.manifest_directory` و`cache_directory`).
5. ممر لجهات اتصال القياس عن بعد/مقابض PagerDuty لحارة التنبيهات عبر الإنترنت وسلكية.

## 2. حارة التحف بنائیں

جذر المستودع المساعد:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

العلامات الرئيسية:

- `--lane-id` إلى `nexus.lane_catalog` لا يوجد إدخال جديد يتطابق مع الفهرس .
- `--dataspace-alias` و`--dataspace-id/hash` إدخال كتالوج مساحة البيانات والتحكم في البطاقة (حذف ہونے پر لاستخدام معرف المسار الافتراضي ہوتا ہے).
- `--validator` قم بتكرار ما تم إجراؤه أو `--validators-file` تم إجراؤه مرة أخرى.
- `--route-instruction` / `--route-account` قواعد التوجيه الجاهزة لللصق تنبعث منها خراطيش.
- `--metadata key=value` (أو `--telemetry-contact/channel/runbook`) تلتقط جهات اتصال دليل التشغيل البطاقات واللوحات المعلوماتية الصحيحة لأصحابها.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` خطاف ترقية وقت التشغيل والبيان الذي يتضمن بطاقة وشريط حارة وعناصر تحكم مشغل ممتدة.
- `--encode-space-directory` يعمل بشكل طبيعي على `cargo xtask space-directory encode` چلاتا. `--space-directory-out` يتم استخدام البطاقة من `.to` بشكل افتراضي بالإضافة إلى ذلك ويتم تشغيله.

هذا هو السكربت `--output-dir` اندر هذه المصنوعات بنتے ہیں (الدليل الافتراضي الموجود ہے)، وترميز فعال من خلال چوتھا بنتا ہے:

1. `<slug>.manifest.json` - بيان المسار الذي يتضمن نصاب التحقق من الصحة ومساحات الأسماء المحمية وبيانات تعريف ربط ترقية وقت التشغيل.
2. `<slug>.catalog.toml` - مقتطف TOML هو `[[nexus.lane_catalog]]`، `[[nexus.dataspace_catalog]]`، وقواعد التوجيه المطلوبة. إدخال مساحة البيانات `fault_tolerance` يجب أن يتم تعيينه من قبل لجنة ترحيل المسار (`3f+1`) وهو مناسب حقًا.
3. `<slug>.summary.json` - ملخص التدقيق للهندسة (الحلقة والقطاعات وبيانات التعريف) وهو ما يتطلب خطوات الطرح المطلوبة و`cargo xtask space-directory encode` هو الأمر الدقيق ( `space_directory_encode.command` تحت) من خلال نقرة واحدة. هذه تذكرة الصعود هي الدليل الذي تم إرفاقه مسبقًا.
4. `<slug>.manifest.to` - `--encode-space-directory` فعال ہونے پر بنتا ہے؛ Torii إلى `iroha app space-directory manifest publish` التدفق جاهز.

`--dry-run` عبارة عن ملفات JSON/snippets التي تسمح بمعاينة الكتابة، و`--force` هي عبارة عن عناصر موجودة يتم الكتابة فوقها.

## 3.تبديل لعبة لاغو

1. ملف JSON الذي تم تكوينه `nexus.registry.manifest_directory` يحتوي على ملف تعريف (ويمكن أيضًا استخدام دليل ذاكرة التخزين المؤقت في حالة عكس حزم التسجيل البعيدة). إذا تم إصدار بيان الريبو التكويني، فاحرص على الالتزام به.
2. مقتطف الكتالوج `config/config.toml` (أو `config.d/*.toml` مناسب) هو إلحاق كریں. `nexus.lane_count` من `lane_id + 1` يوجد طريق ومسار جديد إلى `nexus.routing_policy.rules`.
3. قم بتشفير الكري (إذا كان `--encode-space-directory` هو هذا) ويمكن لدليل الفضاء نشر الكري. ملخص استخدام الأمر الموجود (`space_directory_encode.command`) کریں۔ تم تقديم الحمولة الصافية بنتا ومراجعي الحسابات إلى `.manifest.to`؛ `iroha app space-directory manifest publish` قم بإرسال الرسالة.
4. `irohad --sora --config path/to/config.toml --trace-config` يتم حفظ إخراج التتبع وتذكرة التشغيل في الأرشيف. إنها كرات ثابتة وهي هندسة جديدة متولدة من قطاعات سبيكة/كورا تتوافق مع ذلك.
5. يتم نشر البيان/الكتالوج مرة أخرى بعد المسار الذي تم تعيينه للمدققين لإعادة تشغيل البطاقة. تتضمن عمليات التدقيق المستقبلية ملخصًا لتذكرة JSON.

## 4. حزمة توزيع التسجيل

البيان والتراكب الذي تم إنشاؤه لحزمة مشغلي العمليات هم المضيف من خلال التكوينات، وتحرير بيانات إدارة المسار، وتوزيعها. يظهر مساعد المجمّع في التخطيط المتعارف عليه من قبل `nexus.registry.cache_directory` في تراكب كتالوج الإدارة، وعمليات النقل دون اتصال بالإنترنت في tarball نکال سکتا:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

النواتج:

1.`manifests/<slug>.manifest.json` - تم تكوين `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` - `nexus.registry.cache_directory` محدث. إدخال `--module` هو تعريف الوحدة النمطية القابلة للتوصيل، ويمكن إجراء عمليات مبادلة وحدة الإدارة (NX-2) وتفريغ تراكب ذاكرة التخزين المؤقت فقط، `config.toml` لا يوجد مصطلح.
3.`summary.json` - التجزئة، وتراكب البيانات التعريفية، وتعليمات المشغل.
4. اختياري `registry_bundle.tar.*` - SCP، S3، أو أجهزة تعقب القطع الأثرية جاهزة.

مكمل الدليل (أو الأرشيف) الذي يقوم بمزامنة البيانات من خلال أداة التحقق من الصحة، والمضيفين الذين تم فصلهم عن الهواء لاستخراج الملفات، وإعادة تشغيل Torii، وإظهار البيانات + تراكب ذاكرة التخزين المؤقت، كما يتم حفظ مسارات التسجيل.

## 5. اختبارات الدخان المدققة

إعادة تشغيل Torii بعد إعادة تشغيل مساعد الدخان الجديد، وتسجيل المسار `manifest_ready=true`، والمقاييس المتوقعة لعدد الممرات، ومقياس مختوم صافٍ. الممرات التي تظهر في داخلها غير فارغة `manifest_path` ظاہر کرنا چاہيے؛ helper Ab path غائب أونے پر فوراً فشل ہوتا ہے تاك ہر NX-7 نشر دليل واضح موقع يشمل:

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

البيئات الموقعة ذاتيًا موجودة في `--insecure` وتتضمن القراءة. إذا كانت الحارة غائبة، مختومة، أو القيم المتوقعة للمقاييس/القياس عن بعد، فسوف تنجرف إلى مخرج غير صفري من كرتا. يتم استخدام `--min-block-height` و`--max-finality-lag` و`--max-settlement-backlog` و`--max-headroom-events` لقياس ارتفاع الكتلة على مستوى المسار/النهائية/المتراكمة/قياس المساحة العلوية عن بعد للمغلف التشغيلي، و `--max-slot-p95` / `--max-slot-p99` (السبت `--min-slot-samples`) وهو عبارة عن أهداف ذات مدة زمنية محددة للفتحة NX-18 تساعد الاندرويد على فرض الكرات.

عمليات التحقق من صحة الهواء (أو CI) التي تم التقاطها لنقطة النهاية المباشرة من خلال إعادة تشغيل استجابة Torii:

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

`fixtures/nexus/lanes/` تظهر التركيبات المسجلة لمساعد bootstrap في المصنوعات اليدوية للبطاقة الجديدة والتي تعد بمثابة خصوصية للبرمجة النصية من حيث الوبر. تدفق CI هو `ci/check_nexus_lane_smoke.sh` و`ci/check_nexus_lane_registry_bundle.sh` (الاسم المستعار: `make check-nexus-lanes`) وهو عبارة عن ذرات شيلاتا ثابتة ومساعد الدخان NX-7 شائع تنسيق الحمولة النافعة بما يتوافق مع رهانا وملخصات/تراكبات الحزمة قابلة للتكرار.