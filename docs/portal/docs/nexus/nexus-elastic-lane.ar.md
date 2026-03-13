---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-elastic-lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 879f71958cffe6083348ed2032649db5698acb97932489594299bb713fe49817
source_last_modified: "2025-11-21T18:07:39.995809+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: nexus-elastic-lane
title: تجهيز lane المرن (NX-7)
sidebar_label: تجهيز lane المرن
description: سير عمل bootstrap لانشاء manifests لـ Nexus lane ومدخلات الكتالوج وادلة rollout.
---

:::note المصدر الرسمي
تعكس هذه الصفحة `docs/source/nexus_elastic_lane.md`. حافظ على النسختين متطابقتين حتى يصل مسح الترجمة الى البوابة.
:::

# مجموعة ادوات تجهيز lane المرن (NX-7)

> **عنصر خارطة الطريق:** NX-7 - ادوات تجهيز lane المرن  
> **الحالة:** الادوات مكتملة - تولد manifests، مقتطفات الكتالوج، حمولات Norito، اختبارات smoke،
> ومساعد bundle لاختبارات الحمل يجمع الان بوابة زمن الاستجابة لكل slot + manifests الادلة كي تنشر اختبارات حمل المدققين
> دون سكربتات مخصصة.

هذا الدليل يوجه المشغلين عبر المساعد الجديد `scripts/nexus_lane_bootstrap.sh` الذي يقوم بأتمتة توليد manifest لـ lane ومقتطفات كتالوج lane/dataspace وادلة rollout. الهدف هو تسهيل انشاء lanes جديدة في Nexus (عامة او خاصة) دون تحرير عدة ملفات يدويا ودون اعادة اشتقاق هندسة الكتالوج يدويا.

## 1. المتطلبات المسبقة

1. موافقة الحوكمة على alias لـ lane و dataspace ومجموعة المدققين وتحمّل الاعطال (`f`) وسياسة settlement.
2. قائمة نهائية بالمدققين (معرفات الحسابات) وقائمة namespaces المحمية.
3. وصول الى مستودع تهيئة العقد كي تتمكن من اضافة المقتطفات المولدة.
4. مسارات لسجل manifests الخاص بـ lane (انظر `nexus.registry.manifest_directory` و `cache_directory`).
5. جهات اتصال للقياس عن بعد/مفاتيح PagerDuty الخاصة بالlane حتى يمكن ربط التنبيهات بمجرد دخول lane الى الخدمة.

## 2. توليد artefacts للـ lane

شغّل المساعد من جذر المستودع:

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

اهم الاعلام:

- `--lane-id` يجب ان يطابق index الادخال الجديد في `nexus.lane_catalog`.
- `--dataspace-alias` و `--dataspace-id/hash` يتحكمان في ادخال كتالوج dataspace (افتراضيا يستخدم id الخاص بالlane عند الحذف).
- `--validator` يمكن تكراره او قراءته من `--validators-file`.
- `--route-instruction` / `--route-account` تصدر قواعد توجيه جاهزة للصق.
- `--metadata key=value` (او `--telemetry-contact/channel/runbook`) تلتقط جهات اتصال الـ runbook لكي تعرض اللوحات اصحابها الصحيحين.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` تضيف hook runtime-upgrade الى manifest عندما يحتاج lane الى ضوابط تشغيل ممتدة.
- `--encode-space-directory` يستدعي `cargo xtask space-directory encode` تلقائيا. استخدمه مع `--space-directory-out` اذا اردت ان يوضع ملف `.to` المشفر في مسار غير الافتراضي.

ينتج السكربت ثلاثة artefacts داخل `--output-dir` (الافتراضي هو المجلد الحالي)، مع رابع اختياري عند تفعيل encoding:

1. `<slug>.manifest.json` - manifest للـ lane يحتوي quorum المدققين و namespaces المحمية وبيانات اختيارية لـ hook runtime-upgrade.
2. `<slug>.catalog.toml` - مقتطف TOML يحتوي `[[nexus.lane_catalog]]` و `[[nexus.dataspace_catalog]]` واي قواعد توجيه مطلوبة. تاكد من تعيين `fault_tolerance` في ادخال dataspace لتحديد لجنة lane-relay (`3f+1`).
3. `<slug>.summary.json` - ملخص تدقيق يصف الهندسة (slug والقطاعات والبيانات) وخطوات rollout المطلوبة والامر الدقيق `cargo xtask space-directory encode` (ضمن `space_directory_encode.command`). ارفق هذا JSON بتذكرة onboarding كدليل.
4. `<slug>.manifest.to` - يصدر عند تفعيل `--encode-space-directory`؛ جاهز لتدفق `iroha app space-directory manifest publish` في Torii.

استخدم `--dry-run` لمعاينة JSON/المقتطفات دون كتابة ملفات، و `--force` لاعادة الكتابة فوق artefacts الموجودة.

## 3. تطبيق التغييرات

1. انسخ manifest JSON الى `nexus.registry.manifest_directory` المهيأ (والى cache directory اذا كان registry يطابق حزم remote). التزم بالملف اذا كانت manifests تُدار بالنسخ في مستودع التهيئة.
2. الحق مقتطف الكتالوج في `config/config.toml` (او `config.d/*.toml` المناسب). تاكد من ان `nexus.lane_count` يساوي على الاقل `lane_id + 1` وحدّث اي `nexus.routing_policy.rules` يجب ان تشير الى lane الجديد.
3. شفّر (اذا تجاوزت `--encode-space-directory`) وانشر manifest في Space Directory باستخدام الامر الملتقط في summary (`space_directory_encode.command`). هذا ينتج payload `.manifest.to` الذي يتوقعه Torii ويسجل الادلة للمراجعين؛ ارسله عبر `iroha app space-directory manifest publish`.
4. شغّل `irohad --sora --config path/to/config.toml --trace-config` وارشف مخرجات trace في تذكرة rollout. هذا يثبت ان الهندسة الجديدة تطابق slug/قطاعات Kura المولدة.
5. اعد تشغيل المدققين المخصصين للlane بعد نشر تغييرات manifest/الكتالوج. احتفظ بملف summary JSON في التذكرة للتدقيقات المستقبلية.

## 4. بناء bundle لتوزيع السجل

قم بتجميع manifest المولد و overlay لكي يتمكن المشغلون من توزيع بيانات حوكمة lanes دون تعديل configs على كل مضيف. مساعد bundler ينسخ manifests الى التخطيط القانوني، وينتج overlay اختياري لكاتالوج الحوكمة لـ `nexus.registry.cache_directory`، ويمكنه اخراج tarball لنقل offline:

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
2. `cache/governance_catalog.json` - ضعها في `nexus.registry.cache_directory`. كل ادخال `--module` يصبح تعريفا لوحدة قابلة للتبديل، ما يتيح تبديل وحدات الحوكمة (NX-2) عبر تحديث overlay الخاص بالكاش بدلا من تعديل `config.toml`.
3. `summary.json` - يتضمن hashes وبيانات overlay وتعليمات للمشغلين.
4. اختياري `registry_bundle.tar.*` - جاهز لـ SCP او S3 او متتبعات artefacts.

زامن المجلد بالكامل (او الارشيف) لكل مدقق، وفكّه على hosts معزولة، وانسخ manifests + overlay الكاش الى مسارات السجل قبل اعادة تشغيل Torii.

## 5. اختبارات smoke للمدققين

بعد اعادة تشغيل Torii، شغّل مساعد smoke الجديد للتحقق من ان lane يبلّغ `manifest_ready=true`، وان المقاييس تعرض عدد lanes المتوقع، وان مقياس sealed خال. يجب ان تعرض lanes التي تتطلب manifests قيمة `manifest_path` غير فارغة؛ ويفشل المساعد مباشرة عند غياب المسار حتى يتضمن كل نشر NX-7 دليل manifest الموقع:

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

اضف `--insecure` عند اختبار بيئات self-signed. يخرج السكربت برمز غير صفري اذا كانت lane مفقودة او sealed او اذا انحرفت المقاييس/القياس عن القيم المتوقعة. استخدم `--min-block-height` و `--max-finality-lag` و `--max-settlement-backlog` و `--max-headroom-events` للحفاظ على القياس لكل lane (ارتفاع الكتلة/النهائية/backlog/headroom) ضمن حدود التشغيل، واربطها مع `--max-slot-p95` / `--max-slot-p99` (مع `--min-slot-samples`) لفرض اهداف مدة slot في NX-18 دون مغادرة المساعد.

لعمليات التحقق air-gapped (او CI) يمكنك اعادة تشغيل استجابة Torii ملتقطة بدلا من الوصول الى endpoint حي:

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

الـ fixtures المسجلة تحت `fixtures/nexus/lanes/` تعكس artefacts التي ينتجها مساعد bootstrap حتى يمكن lint manifests الجديدة دون سكربتات مخصصة. تنفذ CI نفس التدفق عبر `ci/check_nexus_lane_smoke.sh` و `ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`) لاثبات ان مساعد smoke الخاص بـ NX-7 يبقى متوافقا مع صيغة payload المنشورة وللتأكد من ان digests/overlays الخاصة بالbundle قابلة لاعادة الانتاج.
