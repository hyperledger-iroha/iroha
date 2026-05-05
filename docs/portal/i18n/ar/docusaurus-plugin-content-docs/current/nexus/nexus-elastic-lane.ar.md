---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-elastic-lane.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: حارة العلاقة المرنة
العنوان: تجهيزات حارة المرن (NX-7)
Sidebar_label: حارة المرن
description: سير عمل bootstrap لإنشاء بيانات لـ Nexus Lane ومدخلات الكتالوج وتكافؤ الطرح.
---

:::ملاحظة المصدر الرسمي
احترام هذه الصفحة `docs/source/nexus_elastic_lane.md`. حافظ على النسختين متطابقتين حتى يصل إلى الترجمة إلى البوابة.
:::

# مجموعة ادوات تجهيزات الحارة المرن (NX-7)

> **عنصر خارطة الطريق:** NX-7 - ادوات حارة المرن  
> **الحالة:** الادوات مكتملة - تولد البيانات، مقتطفات الكتالوج، حمولات Norito، الدخان،
> ومساعدات حزمة المتسابقات الحمل يجمع الان بوابة توقيت لكل فتحة + يظهر التكافؤ كي يتابعن حمل المرقمين
> دون سكربتات مخصصة.

هذا الدليل يوجه التشغيلين عبر المساعد الجديد `scripts/nexus_lane_bootstrap.sh` الذي يقوم بتصميم توليد البيان لـ حارة ومقتطفات كتالوج حارة/مساحة بيانات وبديلة طرح. الهدف هو تسهيل إنشاء ممرات جديدة في Nexus (عامة او خاصة) دون تحرير عدة ملفات يدويةيا إعادة إعادة اشتقاق هندسة الكتالوجيليا.

## 1.المتطلبات المسبقة

1. موافقة الحكام على الاسم المستعار لـ الحارة ومساحة البيانات NF المرقمين وتتسع الاجازة (`f`) وتسوية السياسة.
2. قائمة نهائية بالم 80 (معرفات ذكية) وقائمة مساحات الأسماء المحمية.
3. الوصول الى مستودع تهيئة يبدأ كي جديد من إضافة المقتطفات المولدة.
4. مسارات لتسجيل البيانات الخاصة بـ الحارة (انظر `nexus.registry.manifest_directory` و `cache_directory`).
5. جهات الاتصال للقياس عن بعد/مفاتيح PagerDuty الخاصة بالدخول إلى المسار حتى يمكن تشغيل التنبيهات بمجرد وصول المسار إلى الخدمة.

## 2. توليد التحف للـ حارة

شغّل المساعد من جذر المستودع:

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

اهم الاعلام:

- `--lane-id` يجب ان يطابق الفهرس الادخال الجديد في `nexus.lane_catalog`.
- `--dataspace-alias` و `--dataspace-id/hash` يتحكمان في ادخال كتالوج dataspace (افتراضيا يستخدم id الخاص بالlane عند الحذف).
- `--validator` يمكن تعديله او قراءته من `--validators-file`.
- `--route-instruction` / `--route-account` معايير توجيهي لـ لصق.
- `--metadata key=value` (او `--telemetry-contact/channel/runbook`) التقاط جهات اتصال الـ runbook لتسليط الضوء على اللوحات اصحابها الصحيحين.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` تحميل هوك runtime-upgrade الى البيان عندما يحتاج حارة الى ضوابط تشغيل متدرجة.
- `--encode-space-directory` يستدعي `cargo xtask space-directory encode` خارجي. استخدمه مع `--space-directory-out` اذا اردت ان عميق ملف `.to` المشفر في مسار غير افتراضي.

السكريبت ثلاثة قطع أثرية داخل `--output-dir` (الافتراضي هو المجلد الحالي)، مع اختياري الجنوبي عند تفعيل الترميز:

1. `<slug>.manifest.json` - البيان للـ حارة يحتوي على النصاب القانوني المتابعين ومساحات الأسماء المحمية وبيانات اختيارية لـ Hook runtime-upgrade.
2. `<slug>.catalog.toml` - مقتطف TOML يحتوي على `[[nexus.lane_catalog]]` و `[[nexus.dataspace_catalog]]` واي متطلبات توجيه مطلوبة. تأكد من تعيين `fault_tolerance` في إدخال dataspace للوحة التحكم في لوحة المفاتيح (`3f+1`).
3. `<slug>.summary.json` - ملخص تدقيق صف الأنواع (slug والقطاعات والبيانات) وخطوات الطرح المطلوبة والامر الأبيض `cargo xtask space-directory encode` (ضمن `space_directory_encode.command`). ارفق هذا JSON بتذكرة onboarding كدليل.
4.`<slug>.manifest.to` - يصدر عند تفعيل `--encode-space-directory`؛ جاهز لتدفق `iroha app space-directory manifest publish` في Torii.

استخدم `--dry-run` لمعاينة ملفات JSON/المقتطفات دون كتابة الملفات، و `--force` لاعادة الكتابة بالمصنوعات اليدوية الموجودة.

## 3. تطبيق التغييرات

1. انسخ المانيفست JSON الى `nexus.registry.manifest_directory` المهيأ (وإلى دليل ذاكرة التخزين المؤقت اذا كان التسجيل يطابق حزم التحكم عن بعد). التزم بالملف اذا كانت تُظهر تُدار بالنسخ في مستودع التهيئة.
2. الحق مقتطف الكتالوج في `config/config.toml` (او `config.d/*.toml` المناسب). تأكد من ان `nexus.lane_count` يساوي يساوي `lane_id + 1` وتصحيح اي `nexus.routing_policy.rules` يجب ان تشير الى المسار الجديد.
3.شفّر (اذا تجاوزت `--encode-space-directory`) ونشر البيان في الدليل الفضائي باستخدام الأمر المختار في التلخيص (`space_directory_encode.command`). وهذا ينتج الحمولة `.manifest.to` الذي ارتكبه Torii ويسجل العادل للمراجعين؛ ارسله عبر `iroha app space-directory manifest publish`.
4. شغّل `irohad --sora --config path/to/config.toml --trace-config` واشف مخرجات أثر في تذكرة الطرح. هذا ثابت ان الشكل الجديد تطابق البزاقة/قطاعات كورا المولدة.
5. تشغيل المتبرعين المحتملين للمسار بعد نشر البيان/الكتالوج. احتفظ بملف ملخص JSON في التذكرة للدقيقة المستقبلية.

## 4. بناء حزمة لتوزيع السجل

قم بتجميع المولدات والتراكبات لتتمكن من توزيع البيانات وتوسيع الممرات دون تعديل التكوينات على كل مضيف. مساعد المجمع ينسخ البيانات إلى التخطيط، وينتج تراكب اختياري لكاتالوج الـ to `nexus.registry.cache_directory`، ويمكنه اخراج tarball ونقل البيانات دون اتصال:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

المخرجات:

1. `manifests/<slug>.manifest.json` - انسخها الى `nexus.registry.manifest_directory` المهيأ.
2.`cache/governance_catalog.json` - ضعها في `nexus.registry.cache_directory`. كل ادخال `--module` يصبح تعريفنا لوحدة قابلة للتبديل، ما يتيح تغيير وحدات الالتبديل (NX-2) عبر تحديث overlay الخاص بالكاش بدلا من تعديل `config.toml`.
3.`summary.json` - يشمل التجزئات وبيانات التراكب باتجاهات للمشغلين.
4. اختياري `registry_bundle.tar.*` - جاهز لـ SCP او S3 او تتبعات artefacts.

زامن المجلد بالكامل (او الارشيف) لكل متر، وفكّه على المضيفين معزولة، وانسخ البيانات + تراكب الكاش الى مسارات السجل قبل إعادة Torii.

## 5. صلاحية الدخان للمقررين

بعد إعادة تشغيل Torii، شغّل مساعد الدخان الجديد. يجب ان يتم كشف الممرات التي تتطلب بيانات القيمة `manifest_path` غير فارغة؛ ويفشل المساعد مباشرة عند غياب المسار حتى يشمل كل نشر بيان دليل NX-7:

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

اضف `--insecure` عند اختبار بيئات التوقيع الذاتي. يخرج السكربت برمز غير صفري اذا كانت الحارة مفقودة او مختومة او اذا انحرفت المقاييس/القياس عن القيم. تم استخدام `--min-block-height` و `--max-finality-lag` و `--max-settlement-backlog` و `--max-headroom-events` للقياس لكل حارة (ارتفاع الخلية/النهائية/تراكم/الإرتفاع) ضمن حدود التشغيل، وربطها مع `--max-slot-p95` / `--max-slot-p99` (مع `--min-slot-samples`) لفرض ا محدد مدة الفتحة في NX-18 دون خدمة المساعدة.

معتمدة بشهادة air-gapped (او CI) يمكنك إعادة تشغيل تسجيل Torii اختيارية بدل من الوصول إلى نقطة النهاية الحية:

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

الـ تركيبات أصلية تحت `fixtures/nexus/lanes/` الثقة في المصنوعات اليدوية التي تصلها مساعد bootstrap حتى يمكن لينت مانفيسس الجديدة دون سكربتات مخصصة. ما دام CI صالحاً عبر `ci/check_nexus_lane_smoke.sh` و `ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`) لاثبات ان مساعد الدخان الخاص بـ NX-7 يظل متا مع الحمولة التالية ولتأكد من ان هضم/تراكبات خاصة بالحزمة لاعادة الإنتاج.