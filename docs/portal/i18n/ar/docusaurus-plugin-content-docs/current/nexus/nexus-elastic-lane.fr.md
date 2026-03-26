---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-elastic-lane.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: حارة العلاقة المرنة
العنوان: توفير المسار المرن (NX-7)
Sidebar_label: توفير المسار المرن
الوصف: سير عمل التمهيد لإنشاء بيانات المسار Nexus وإدخالات الكتالوج وإجراءات الطرح.
---

:::ملاحظة المصدر الكنسي
صفحة Cette تتكرر `docs/source/nexus_elastic_lane.md`. قم بحفظ نسختين من النسخ حتى يصل غموض الترجمة إلى البوابة.
:::

# مجموعة توفير الممرات المرنة (NX-7)

> **عنصر خريطة الطريق:** NX-7 - أدوات توفير الممرات المرنة  
> **الحالة:** الأدوات كاملة - نوع البيانات، مقتطفات الكتالوج، الحمولات الصافية Norito، اختبارات الدخان،
> ويتم تجميع مساعد حزمة اختبار التحميل مع الحفاظ على بوابة زمن الوصول حسب الفتحة + بيانات الفحص حتى يتم تشغيل مدققي الشحن
> ينشر puissent etre بدون برمجة نصية بقدر ما.

يرافق هذا الدليل المشغلين عبر المساعد الجديد `scripts/nexus_lane_bootstrap.sh` الذي يقوم بأتمتة إنشاء بيانات المسار ومقتطفات من كتالوج المسار/مساحة البيانات ومقدمات الطرح. الهدف هو تسهيل إنشاء الممرات الجديدة Nexus (العامة أو الخاصة) بدون تحرير الملفات الإضافية الرئيسية وإعادة اشتقاق هندسة الكتالوج الرئيسية.

## 1. المتطلبات الأساسية

1. الموافقة على إدارة الاسم المستعار للممر ومساحة البيانات ومجموعة المدققين والتسامح مع اللوحات (`f`) وسياسة التسوية.
2. قائمة نهائية للمدققين (معرفات الحساب) وقائمة مساحات الأسماء المحمية.
3. قم بالوصول إلى مستودع تكوينات الأخبار حتى تتمكن من إضافة المقتطفات العامة.
4. لوائح تسجيل بيانات المسار (تظهر `nexus.registry.manifest_directory` و`cache_directory`).
5. جهات الاتصال/القياس عن بعد/مقابض PagerDuty للخط الذي يسمح بالتنبيهات المتصلة بأن الخط متصل بالإنترنت.

## 2. إنشاء قطع أثرية للممر

اسحب المساعد من سباق المستودع:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator soraカタカナ... \
  --validator soraカタカナ... \
  --validator soraカタカナ... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

أعلام كلي :

- `--lane-id` يتوافق مع فهرس الإدخال الجديد في `nexus.lane_catalog`.
- `--dataspace-alias` و`--dataspace-id/hash` يتحكمان في مدخل كتالوج مساحة البيانات (افتراضيًا، معرف المسار عند حذفه).
- `--validator` يمكن تكراره أو تكراره بعد `--validators-file`.
- `--route-instruction` / `--route-account` يصدر قواعد التوجيه المسبقة.
- `--metadata key=value` (ou `--telemetry-contact/channel/runbook`) يلتقط جهات الاتصال في دفتر التشغيل حتى تعرض لوحات المعلومات أصحابها الجيدين.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` قم بربط ترقية وقت التشغيل بالبيان عندما يتطلب المسار ضوابط التشغيل المستمرة.
- `--encode-space-directory` استدعاء تلقائي `cargo xtask space-directory encode`. ادمج مع `--space-directory-out` عندما تريد أن يقوم الملف `.to` بتشفير جميع الخطوات التي يقوم بها المسار افتراضيًا.

تم إنتاج البرنامج النصي بثلاثة عناصر في `--output-dir` (افتراضيًا للمرجع الرئيسي)، بالإضافة إلى خيار رباعي عندما يكون التشفير نشطًا:

1.`<slug>.manifest.json` - بيان المسار الذي يحتوي على نصاب المدققين ومساحات الأسماء المحمية وخيارات التعريف الخاصة بترقية وقت التشغيل.
2. `<slug>.catalog.toml` - مقتطف TOML مع `[[nexus.lane_catalog]]`، `[[nexus.dataspace_catalog]]` وكل تنظيم التوجيه المطلوب. تأكد من أن `fault_tolerance` محدد على مساحة البيانات المدخلة لأبعاد مرحل الخط (`3f+1`).
3.`<slug>.summary.json` - استئناف التدقيق في الأشكال الهندسية (الحلقة، والقطاعات، والفوقيات) بالإضافة إلى خطوات الطرح المطلوبة والأمر الدقيق `cargo xtask space-directory encode` (Sous `space_directory_encode.command`). انضم إلى تذكرة الصعود إلى JSON كأولوية مسبقة.
4.`<slug>.manifest.to` - الانبعاث عندما يكون `--encode-space-directory` نشطًا؛ من أجل التدفق `iroha app space-directory manifest publish` من Torii.

استخدم `--dry-run` لمعاينة JSON/snippets بدون كتابة ملفات، و`--force` لمسح العناصر الموجودة.

## 3. تطبيق التغييرات

1. انسخ بيان JSON في تكوين `nexus.registry.manifest_directory` (وذلك في دليل ذاكرة التخزين المؤقت في حالة تسجيل الحزم البعيدة). قم بتثبيت الملف إذا كانت البيانات عبارة عن إصدارات في مستودع التكوين الخاص بك.
2. أضف مقتطف الكتالوج إلى `config/config.toml` (أو إلى `config.d/*.toml` بشكل مناسب). تأكد من أن `nexus.lane_count` هو `lane_id + 1` فقط، واضبط كل يوم على `nexus.routing_policy.rules` الذي يحرك المؤشر إلى المسار الجديد.
3. قم بتشفير (إذا كنت تريد أن تقول `--encode-space-directory`) ونشر البيان في دليل الفضاء عبر أمر الالتقاط في الملخص (`space_directory_encode.command`). هذا المنتج هو الحمولة `.manifest.to` يحضر على قدم المساواة Torii ويسجل الإجراء المسبق لعمليات التدقيق؛ سوميتيز عبر `iroha app space-directory manifest publish`.
4. قم بمسح `irohad --sora --config path/to/config.toml --trace-config` وأرشفة أثر العملية في تذكرة التشغيل. هذا يثبت أن الهندسة الجديدة تتوافق مع الأجزاء الموجودة في هذا النوع من البزاقة.
5. قم بإعادة تعيين المدققين على المسار مرة واحدة بعد نشر بيان التغييرات/الكتالوج. احتفظ بملخص JSON في التذكرة لإجراء عمليات التدقيق المستقبلية.

## 4. إنشاء حزمة توزيع التسجيل

قم بتعبئة البيان العام والتراكب حتى يتمكن المشغلون من توزيع بيانات إدارة الممرات دون تحرير التكوينات في كل مرة. تساعد أداة التجميع على نسخ البيانات في التخطيط القياسي، وإنتاج خيار تراكب كتالوج الإدارة لـ `nexus.registry.cache_directory`، ويمكن إنشاء كرة قطرانية لعمليات النقل دون اتصال:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

الطلعات الجوية :

1. `manifests/<slug>.manifest.json` - تكوين النسخ في `nexus.registry.manifest_directory`.
2.`cache/governance_catalog.json` - إيداع في `nexus.registry.cache_directory`. كل إدخال `--module` يخرج تعريفًا للوحدة القابلة للتفرع، مما يسمح بمبادلة وحدة الإدارة (NX-2) بالإضافة إلى تراكب ذاكرة التخزين المؤقت في محرر `config.toml`.
3.`summary.json` - بما في ذلك التجزئات وبيانات التراكب وتعليمات التشغيل.
4. الخيار `registry_bundle.tar.*` - مخصص لـ SCP أو S3 أو أجهزة تعقب القطع الأثرية.

قم بمزامنة الذخيرة بأكملها (أو الأرشيف) مع كل أداة التحقق من الصحة، وقم بنسخ البيانات + تراكب ذاكرة التخزين المؤقت في سلاسل التسجيل قبل استرداد Torii.

## 5. اختبارات الدخان للمدققين

بعد إعادة تعيين Torii، قم بتشغيل مساعد الدخان الجديد للتحقق من أن الخط يشير إلى `manifest_ready=true`، وأن المقاييس تكشف عن عدد الممرات الموجودة، وأن العداد مغلق. يجب أن تعرض الممرات التي تتطلب البيانات `manifest_path` غير فيديو؛ يوضح المساعد ما الذي يمنعك من اتباع كل طريقة من عمليات النشر NX-7 بما في ذلك علامة البيان:

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

Ajoutez `--insecure` عندما تقوم باختبار البيئات الموقعة ذاتيًا. يتم فرز النص برمز غير صفر إذا تم إغلاق المسار، أو إذا كانت المقاييس/القياس عن بعد تشتق من القيم الحالية. استخدم المقابض `--min-block-height` و`--max-finality-lag` و`--max-settlement-backlog` و`--max-headroom-events` للحفاظ على القياس عن بعد على طول المسار (ارتفاع الكتلة/النهائي/تراكم/مساحة الإرتفاع) في مغلفاتك التشغيلية، والاقتران معها `--max-slot-p95` / `--max-slot-p99` (بالإضافة إلى `--min-slot-samples`) لفرض كائنات مدة الفتحة NX-18 دون ترك المساعد.

من أجل عمليات التحقق من الصحة air-gapped (ou CI)، يمكنك الحصول على رد Torii يتم التقاطه بدلاً من الاستجواب في نقطة نهاية مباشرة:

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

تعكس التركيبات التي تم تسجيلها في `fixtures/nexus/lanes/` المصنوعات اليدوية من خلال مساعد التمهيد حتى تظهر العناصر الجديدة بشكل فعال وبدون كتابة نصوص برمجية على قياس. يقوم CI بتنفيذ التدفق عبر `ci/check_nexus_lane_smoke.sh` و`ci/check_nexus_lane_registry_bundle.sh` (الاسم المستعار: `make check-nexus-lanes`) للتأكد من أن مساعد الدخان NX-7 يتوافق مع تنسيق الحمولة المنشورة ولضمان أن تكون خلاصات/تراكبات الحزمة قابلة لإعادة الإنتاج.