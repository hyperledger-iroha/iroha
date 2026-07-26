---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: nexus-default-lane-quickstart
العنوان: المسار الافتراضي کوئیک استارتٹ (NX-5)
Sidebar_label: المسار الافتراضي کوئیک استايرٹ
الوصف: Nexus هو المسار الاحتياطي الافتراضي لتكوين المسار الاحتياطي Torii وSDK والممرات العامة لحذف معرف المسار.
---

:::ملاحظة المصدر الكنسي
هذه هي الصفحة `docs/source/quickstart/default_lane.md`. عندما لا تكتمل عملية اكتساح التوطين، لا يتم محاذاة الفيديو بالكامل.
:::

# المسار الافتراضي کوئیک استارتٹ (NX-5)

> **سياق خريطة الطريق:** NX-5 - تكامل المسار العام الافتراضي۔ وقت التشغيل هو `nexus.routing_policy.default_lane` الاحتياطي وحدد نقاط النهاية Torii REST/gRPC وSDK `lane_id` التي تم حذفها من خلال المسار العام الأساسي سے تعلق رکھتا ہو۔ يقوم مشغلو هذه الأجهزة بتكوين السجل، `/status`، والتحقق الاحتياطي من السجل، وممارسة سلوك العميل الشامل.

## المتطلبات الأساسية

- `irohad` کا Sora/Nexus build ( `irohad --sora --config ...` چلایں ).
- مستودع التكوين يقوم بتحرير الأقسام `nexus.*` إلى أي مكان.
- `iroha_cli` تم تكوين المجموعة المستهدفة جو.
- Torii `/status` فحص الحمولة الصافية `curl`/`jq` (أو ما يعادلها).

## 1. كتالوج الممرات ومساحة البيانات

يتم الإعلان عن شبكة متاحة عبر الممرات ومساحات البيانات. نیچے ولا مقتطف (`defaults/nexus/config.toml` سے) تسجيل هذه الممرات العامة والأسماء المستعارة لمساحة البيانات المطابقة:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

ہر `index` منفردة ومتجاورة ہونا چاہیے. معرفات مساحة البيانات قيم 64 بت؛ استخدم المثال السابق ووضح فهارس الممرات لاستخدام القيم الرقمية.

## 2. إعدادات التوجيه الافتراضية والتجاوزات الاختيارية

`nexus.routing_policy` المسار الاحتياطي الذي يتحكم في الكرتا وتعليمات خاصة أو بادئات الحساب لتجاوز التوجيه. إذا كانت مطابقة القاعدة لن تتمكن من جدولة الفرانزية وتكوين `default_lane` و`default_dataspace` على المسار الصحيح. يتم تطبيق منطق جهاز التوجيه `crates/iroha_core/src/queue/router.rs` وTorii REST/gRPC على أسطح شفافة شفافة.

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```


## 3. تشغيل العقدة الثابتة

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

بدء تشغيل العقدة ے دوران سياسة التوجيه المشتقة لاگ کرتا ہے۔ هناك أيضًا أخطاء في التحقق من الصحة (الفهارس المفقودة، الأسماء المستعارة المكررة، معرفات مساحة البيانات غير الصالحة) بدء القيل والقال منذ فترة طويلة.

## 4. حالة حوكمة الحارة کنفرم کریں

العقدة المتصلة بالإنترنت بعد ذلك، يستخدم مساعد CLI الممر الافتراضي مغلقًا (تم تحميل البيان) وحركة المرور جاهزة. عرض ملخص للحارة کے لئے صف پرنٹ کرتا ہے:

```bash
iroha_cli app nexus lane-report --summary
```

مثال الإخراج:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

إذا كان المسار الافتراضي `sealed` فإن حركة المرور الخارجية تسمح لك بإدارة دليل إدارة المسار فالو کریں. `--fail-on-sealed` العلم CI کے لئے مفید ہے.

## 5. فحص حمولات الحالة Torii

`/status` سياسة توجيه الاستجابة ولقطة جدولة المسار دونوں فضح کرتا ہے۔ `curl`/`jq` يؤدي استخدام الإعدادات الافتراضية التي تم تكوينها لتتبع البيانات وقياس المسافة للمسار الاحتياطي إلى ما يلي:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

إخراج العينة:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

حارة `0` لعدادات الجدولة المباشرة التالية:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

إنه يسجل لقطة TEU وبيانات تعريف الاسم المستعار وتكوين أعلام البيان الذي يتم محاذاة بشكل ثابت. يمكن استخدام لوحات الحمولة النافعة Grafana في لوحة القيادة الخاصة بمدخل المسار.

## 6. تمرين افتراضيات العميل

- **Rust/CLI.** حقل `iroha_cli` وصندوق عميل Rust `lane_id` الذي يحذف البطاقة عند `--lane-id` / `LaneSelector`. هذا هو جهاز توجيه قائمة الانتظار `default_lane` للرجوع الاحتياطي. تحدد إشارات `--lane-id`/`--dataspace-id` الصريحة المسار غير الافتراضي للهدف عند استخدام الكرت.
- **JS/Swift/Android.** تُصدر SDK حاليًا `laneId`/`lane_id` وهي إدارة اختيارية و`/status` تُعلن عن القيمة الاحتياطية. تعمل سياسة التوجيه والتدريج والإنتاج على مزامنة تطبيقات الهاتف المحمول وعمليات إعادة التكوين في حالات الطوارئ.
- **اختبارات الأنابيب/SSE.** عوامل تصفية حدث المعاملة `tx_lane_id == <u32>` مسندات قبول کرتے ہیں (دیکھیں `docs/source/pipeline.md`). `/v1/pipeline/events/transactions` الذي يقوم بالتصفية للاشتراك في ميزة التسجيل أو وجود ممر واضح يتم كتابة معرف المسار الاحتياطي فيه تحت القائمة.

## 7. خطافات قابلية الملاحظة والحوكمة

- `/status` `nexus_lane_governance_sealed_total` و`nexus_lane_governance_sealed_aliases` ينشرون هذه الرسالة وتحذير مدير التنبيهات من خلال بيان المسار هذا. يمكن أيضًا تمكين التنبيهات الخاصة بشبكات التطوير.
- خريطة القياس عن بعد للمجدول وكتالوج لوحة تحكم إدارة المسار (`dashboards/grafana/nexus_lanes.json`) وحقول الاسم المستعار/الحلقة الثابتة المتوقعة. إذا قمت بإعادة تسمية الاسم المستعار إلى أدلة Kura وإعادة تسمية المسارات الحتمية لمدققي الحسابات (NX-1 تحت المسار أوتا ہے).
- الممرات الافتراضية التي تتطلب موافقات البرلمان تتضمن خطة التراجع التي تشمل ہونا چاہیے. دليل التجزئة والحوكمة الواضح الذي يعد بمثابة بداية سريعة لسجل التشغيل الخاص بالمشغل يسجل عمليات التناوب المستقبلية التي لا تتطلبها الحالة.