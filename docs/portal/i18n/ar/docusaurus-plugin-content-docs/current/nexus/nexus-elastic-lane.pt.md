---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-elastic-lane.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: حارة العلاقة المرنة
العنوان: توفير المسار المرن (NX-7)
Sidebar_label: توفير المسار المرن
الوصف: تدفق التمهيد لإنشاء بيانات المسار Nexus وإدخال الكتالوج وأدلة الطرح.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة espelha `docs/source/nexus_elastic_lane.md`. احتفظ بالنسخ المضمنة التي قمت بها من خلال مسح الموقع على البوابة.
:::

# مجموعة توفير الممرات المرنة (NX-7)

> **عنصر خريطة الطريق:** NX-7 - أدوات توفير المسار المرن  
> **الحالة:** الأدوات كاملة - بيانات المجموعة ومقتطفات الكتالوج والحمولات Norito واختبارات الدخان،
> e o مساعد حزمة التحميل - اختبار Agora Costura Gating de Latency Por Slot + بيانات الأدلة التي تثبت أن شاحنات الشحن مدققة
> possam ser publicadas sem scripting sob medida.

تساعد هذه الأداة المشغلين الجدد `scripts/nexus_lane_bootstrap.sh` في أتمتة عملية إصدار بيانات المسار ومقتطفات كتالوج المسار/مساحة البيانات وأدلة التشغيل. الهدف وتسهيل إنشاء الممرات الجديدة Nexus (العامة أو الخاصة) دون تحرير العديد من الملفات يدويًا دون إعادة اشتقاق هندسة الكتالوج يدويًا.

## 1. المتطلبات الأساسية

1. الموافقة على الإدارة للاسم المستعار للمسار ومساحة البيانات ومجموعة المصادقين والتسامح مع الخطأ (`f`) وسياسة التسوية.
2. قائمة نهائية للمدققين (معرفات الحساب) وقائمة مساحات الأسماء المحمية.
3. قم بالوصول إلى مستودع التكوين من خلال العقدة لتتمكن من إضافة المقتطفات الجيدة.
4. تسجيل بيانات المسار (`nexus.registry.manifest_directory` و`cache_directory`).
5. جهات الاتصال/مقابض القياس عن بعد الخاصة بـ PagerDuty للمسار، حتى يتم توصيل التنبيهات أثناء وجود المسار عبر الإنترنت.

## 2. تغيير الممرات الفنية

تنفيذ أو مساعدة من مصدر المستودع:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator i105... \
  --validator i105... \
  --validator i105... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

أعلام شاف:

- `--lane-id` يجب أن يكون متوافقًا مع الفهرس الجديد المدخل إلى `nexus.lane_catalog`.
- `--dataspace-alias` و `--dataspace-id/hash` التحكم في إدخال كتالوج مساحة البيانات (من خلال لوحة الاستخدام أو معرف المسار عند حذفه).
- قد يكون `--validator` متكررًا أو محذوفًا من `--validators-file`.
- `--route-instruction` / `--route-account` يُصدر قواعد تدوير للعمود.
- `--metadata key=value` (ou `--telemetry-contact/channel/runbook`) يلتقط اتصالات دفتر التشغيل حتى تكون لوحات المعلومات هي المالك الصحيح.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` إضافة ربط ترقية وقت التشغيل إلى البيان عندما يتطلب المسار عناصر تحكم محددة من المشغل.
- `--encode-space-directory` chama `cargo xtask space-directory encode` تلقائيًا. قم بدمج com `--space-directory-out` عندما تريد ملف `.to` مشفرًا وموضعًا آخر في مكانه الافتراضي.

تم إنتاج ثلاثة نصوص برمجية في `--output-dir` (من خلال الدليل أو الدليل الحالي)، ولكن بربع اختياري عندما يكون التشفير مؤهلاً:

1.`<slug>.manifest.json` - بيان المسار التنافسي أو نصاب المصادقين ومساحات الأسماء المحمية والخيارات الوصفية لربط ترقية وقت التشغيل.
2. `<slug>.catalog.toml` - مقتطف TOML مع `[[nexus.lane_catalog]]`، `[[nexus.dataspace_catalog]]`، ثم قم بالتوقف عن طلبات التناوب. تأكد من أن `fault_tolerance` تم تعريفه على مدخل مساحة البيانات لأبعاد مجموعة مسار التتابع (`3f+1`).
3.`<slug>.summary.json` - ملخص الاستماع لتوضيح الشكل الهندسي (الحلقة والقطاعات والامتدادات) بالإضافة إلى خطوات الطرح المطلوبة والأمر النهائي لـ `cargo xtask space-directory encode` (في `space_directory_encode.command`). قم بإضافة JSON إلى تذكرة الإعداد كأدلة.
4.`<slug>.manifest.to` - عندما يكون `--encode-space-directory` مؤهلاً؛ Pronto para o Fluxo `iroha app space-directory manifest publish` do Torii.

استخدم `--dry-run` لعرض ملفات JSON/المقتطفات دون سحب الملفات و`--force` لكشف العناصر المصطنعة الموجودة.

## 3. أبليك كما مودانكاس

1. انسخ بيان JSON لتكوين `nexus.registry.manifest_directory` (لدليل ذاكرة التخزين المؤقت لتسجيل حزم عن بعد). يتم إنشاء ملف أو ملف من خلال الإصدارات الخاصة به في مخزن التكوين الخاص به.
2. قم بإرفاق مقتطف الكتالوج في `config/config.toml` (أو ليس `config.d/*.toml` مناسبًا). تأكد من أن `nexus.lane_count` سيكون أقل من `lane_id + 1` وقم بتنشيط `nexus.routing_policy.rules` الذي يجب عليك التوجه إلى المسار الجديد.
3. قم بالتشفير (se voce pulou `--encode-space-directory`) ونشر أو بيان دليل الفضاء باستخدام o comando capturado بدون ملخص (`space_directory_encode.command`). يتم إنتاج هذا المنتج من الحمولة `.manifest.to` التي تتوقعها من Torii وتسجيل الأدلة للمراجعين؛ تحسد عبر `iroha app space-directory manifest publish`.
4. قم بتنفيذ `irohad --sora --config path/to/config.toml --trace-config` ثم قم بتتبع أي تذكرة بدء التشغيل. هذا يثبت أن الهندسة الجديدة تتوافق مع سبيكة/أجزاء كورا المسطحة.
5. التحقق من المصدقين على المسار عندما يتم تعديل البيانات/الكتالوج المزروع. Mantenha o ملخص JSON لا تذكرة للاستماع إلى المستقبل.

## 4. قم بتوزيع حزمة التسجيل

قم بتضمين بيان إدارة وتراكب حتى يتمكن المشغلون من توزيع بيانات إدارة الممرات دون تحرير التكوينات في كل مضيف. مساعد تجميع نسخ البيانات للتخطيط الكنسي، إنتاج تراكب اختياري لكتالوج الإدارة لـ `nexus.registry.cache_directory` ويمكنه إصدار كرة تار لعمليات النقل دون اتصال:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

صيدا:

1.`manifests/<slug>.manifest.json` - انسخ هذه الملفات لتكوين `nexus.registry.manifest_directory`.
2.`cache/governance_catalog.json` - الكلمة في `nexus.registry.cache_directory`. كل ما تم إدخاله `--module` سيوفر تعريفًا لوحدة التوصيل، مما يسمح بمبادلة وحدة الإدارة (NX-2) من خلال تحديث أو تراكب ذاكرة التخزين المؤقت أثناء تحرير `config.toml`.
3.`summary.json` - يتضمن التجزئات والتراكبات التعريفية والتعليمات الخاصة بالمشغلين.
4. اختياري `registry_bundle.tar.*` - قريبًا لـ SCP أو S3 أو أجهزة تعقب التحف.

قم بمزامنة الدليل الداخلي (أو الملف) لكل أداة التحقق، بالإضافة إلى المضيفين air-gapped ونسخ البيانات + تراكب ذاكرة التخزين المؤقت لكاميرات التسجيل الخاصة بهم قبل إعادة التشغيل أو Torii.

## 5. اختبارات الدخان للمصادقة

بعد إعادة تشغيل Torii، قم بتنفيذ مساعد الدخان الجديد للتحقق من تقرير المسار `manifest_ready=true`، حيث تظهر المقاييس عدوى الممرات المتقطعة وقياس هذه الخطوط المغلقة. الممرات التي توضح ما هو مطلوب هي تصدير `manifest_path` ليس فقط; سيبدأ المساعد الآن على الفور عندما يفشل المسار حتى يتضمن كل نشر NX-7 أدلة واضحة:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v2/sumeragi/status \
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

Adicione `--insecure` ao testar ambientes ذاتي التوقيع. النص يقول مع كوديغو لا صفر إذا كان المسار مفتوحًا، مختومًا أو إذا كانت المقاييس/القياس عن بعد متباينة بين القيمتين المتوقعتين. استخدم مقابض نظام التشغيل `--min-block-height` و`--max-finality-lag` و`--max-settlement-backlog` و`--max-headroom-events` للتحكم في القياس عن بعد للمسار (ارتفاع الكتلة/النهاية/التراكم/الإرتفاع) في حدود عملياتك والجمع مع `--max-slot-p95` / `--max-slot-p99` (المزيد `--min-slot-samples`) للاستيراد كأداة متينة لفتحة NX-18 دون مساعدة.

يمكن لأعضاء التحقق من الهواء (أو CI) إعادة إنتاج استجابة Torii عند الوصول إلى نقطة النهاية مباشرة:

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

تعكس التركيبات المدرجة في `fixtures/nexus/lanes/` المنتجات المصنّعة من خلال مساعد التمهيد بحيث تكون البيانات الجديدة مخطوطة دون كتابة نصوص برمجية بشكل جيد. ينفذ CI التدفق نفسه عبر `ci/check_nexus_lane_smoke.sh` و`ci/check_nexus_lane_registry_bundle.sh` (الاسم المستعار: `make check-nexus-lanes`) لإثبات أن مساعد الدخان NX-7 يستمر في التوافق مع تنسيق الحمولة المنشورة وضمان أن يتم إنتاج الملخصات/التراكبات.