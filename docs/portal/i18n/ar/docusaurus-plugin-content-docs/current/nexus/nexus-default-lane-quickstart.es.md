---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: nexus-default-lane-quickstart
العنوان: الطريق السريع المحدد مسبقًا (NX-5)
Sidebar_label: دليل المسار السريع المحدد مسبقًا
الوصف: قم بتكوين المسار الاحتياطي Nexus والتحقق منه مسبقًا حتى يتمكن Torii وSDK من حذف معرف المسار في الممرات العامة.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/quickstart/default_lane.md`. احتفظ بنسخ متجددة حتى يصل حاجز التوطين إلى البوابة.
:::

# دليل المسار السريع المحدد مسبقًا (NX-5)

> **سياق خريطة الطريق:** NX-5 - تكامل المسار العام المحدد مسبقًا. يعرض وقت التشغيل الآن احتياطيًا `nexus.routing_policy.default_lane` حتى تتمكن نقاط النهاية REST/gRPC الخاصة بـ Torii وكل SDK من حذف `lane_id` بأمان عندما تكون حركة المرور على المسار العام الكنسي. تساعد هذه الأداة المشغلين على تكوين الكتالوج والتحقق من الإجراء الاحتياطي في `/status` وتفعيل معاملة العميل من أقصى الحدود.

## المتطلبات الأساسية

- إنشاء Sora/Nexus من `irohad` (إخراج `irohad --sora --config ...`).
- قم بالوصول إلى مستودع التكوين حتى تتمكن من تحرير الأقسام `nexus.*`.
- تم تكوين `iroha_cli` للتحدث مع أهداف المجموعة.
- `curl`/`jq` (أو ما يعادله) لفحص الحمولة النافعة `/status` من Torii.

## 1. وصف كتالوج الممرات ومساحات البيانات

أعلن عن الممرات ومساحات البيانات التي يجب أن تكون موجودة باللون الأحمر. الجزء التالي (المسجل في `defaults/nexus/config.toml`) يسجل ثلاث ممرات منشورة بالإضافة إلى الاسم المستعار لمساحة البيانات المقابلة:

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

كل `index` يجب أن يكون فريدًا ومتواصلًا. تبلغ قيمة معرفات مساحة البيانات 64 بت؛ تستخدم الأمثلة السابقة نفس القيم الرقمية التي تجعل مؤشرات الممرات أكثر وضوحًا.

## 2. تكوين قيم الإعداد المحددة مسبقًا والقواعد الاختيارية

يتحكم القسم `nexus.routing_policy` في المسار الاحتياطي ويسمح لك بالاشتراك في التعليمات لتعليمات محددة أو بادئات الحساب. إذا لم يتم ضبطه في نفس الوقت، يقوم المجدول بإعادة المعاملة إلى `default_lane` و`default_dataspace`. منطق جهاز التوجيه الحي في `crates/iroha_core/src/queue/router.rs` وتطبيق السياسة الشكلية الشفافة على أسطح REST/gRPC في Torii.

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

عندما يتم إضافة مسارات جديدة، قم بتحديث الكتالوج الأول وتوسيع نطاق أنظمة التشغيل. يجب أن يستمر المسار الاحتياطي عبر المسار العام الذي يركز على الجزء الأكبر من حركة مرور المستخدمين حتى تعمل حزم SDK المتوارثة.

## 3. عقد عقدة مع تطبيق السياسة

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

العقدة تسجل سياسة التوجيه المستمدة أثناء التشغيل. يظهر أي خطأ في التحقق (المؤشرات الخاطئة، والأسماء المستعارة المكررة، ومعرفات مساحة البيانات غير الصالحة) قبل بدء النميمة.

## 4. تأكيد حالة إدارة الممر

بمجرد أن تكون العقدة على الخط، استخدم مساعد CLI للتحقق من أن المسار المحدد مسبقًا تم بيعه (بيان الشحنة) وقائمة المرور. مشهد السيرة الذاتية ينشئ فيلا في الممر:

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

إذا كان المسار المحدد مسبقًا `sealed`، فتابع دليل إدارة الممرات قبل السماح بحركة المرور الخارجية. العلم `--fail-on-sealed` يستخدم لـ CI.

## 5. فحص حمولات الحالة Torii

توضح الإجابة `/status` سياسة التشغيل مثل عملية الجدولة الفورية. Usa `curl`/`jq` لتأكيد القيم المحددة مسبقًا والتحقق من أن المسار الاحتياطي ينتج القياس عن بعد:

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

لفحص المحولات الحية للجدولة للمسار `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

يؤكد هذا أن لحظية TEU والبيانات الاسمية المستعارة وعلامات البيان غير متوافقة مع التكوين. نفس الحمولة هي التي تستخدم لوحات Grafana للوحة القيادة الخاصة بإدخال المسار.

## 6. اختبر قيم العميل المحددة مسبقًا

- **Rust/CLI.** `iroha_cli` ويحذف صندوق الصدأ العميل المجال `lane_id` عندما لا يمر `--lane-id` / `LaneSelector`. يتكرر جهاز توجيه الكولا لهذا الغرض `default_lane`. الأعلام الصريحة هي `--lane-id`/`--dataspace-id` فقط عند فتح مسار غير محدد مسبقًا.
- **JS/Swift/Android.** الإصدارات الأخيرة من SDK تستخدم `laneId`/`lane_id` كخيارات اختيارية وتم الإعلان عنها بالقيمة الاحتياطية لـ `/status`. حافظ على مزامنة سياسة الإعداد بين التدريج والإنتاج حتى لا تحتاج التطبيقات المتحركة إلى إعادة تكوين في حالات الطوارئ.
- **اختبارات الأنابيب/SSE.** تقبل مرشحات أحداث المعاملات `tx_lane_id == <u32>` (الإصدار `docs/source/pipeline.md`). قم بالاشتراك في `/v2/pipeline/events/transactions` باستخدام هذا الفلتر لإظهار أن الكتب المرسلة بدون مسار واضح يمكن الرجوع إليها أسفل معرف المسار الاحتياطي.

## 7. إمكانية المراقبة وفعاليات الحكومة

- `/status` يتم أيضًا نشر `nexus_lane_governance_sealed_total` و`nexus_lane_governance_sealed_aliases` حتى يتمكن Alertmanager من رؤيته عند فتح مسار آخر لبيانه. احتفظ بهذه التنبيهات المؤهلة بما في ذلك شبكات التطوير.
- خريطة القياس عن بعد للجدولة ولوحة التحكم في الممرات (`dashboards/grafana/nexus_lanes.json`) تعرض الاسم المستعار/رابط الكتالوج. إذا قمت بإعادة تسمية اسم مستعار، فقم بكتابة أدلة Kura المقابلة حتى يتمكن المدققون من الحفاظ على محدداتهم (يتبع NX-1).
- يجب أن تتضمن الموافقات البرلمانية للممرات المحددة مسبقًا خطة للتراجع. قم بتسجيل تجزئة البيان وأدلة الإدارة جنبًا إلى جنب مع هذا البدء السريع في دليل المشغل الخاص بك حتى لا تتمكن الدورات المستقبلية من تحقيق الحالة المطلوبة.

بمجرد أن تمر هذه الاختبارات، يمكنك استخدام `nexus.routing_policy.default_lane` كقوة حقيقية لتكوين SDK والتمكن من إلغاء تحديد مسارات التعليمات البرمجية المتوارثة في المسار الفريد باللون الأحمر.