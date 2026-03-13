---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: nexus-default-lane-quickstart
العنوان: جويا رابيدا دو لين بادراو (NX-5)
Sidebar_label: Guia Rapida do Lane Padrao
الوصف: تكوين والتحقق من المسار الاحتياطي لـ Nexus حتى Torii ويمكن لـ SDK حذف معرف المسار في الممرات العامة.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة espelha `docs/source/quickstart/default_lane.md`. قم بحفظ النسخ من خلال مراجعة الموقع من خلال البوابة.
:::

# غيا رابيدا دو لاين بادراو (NX-5)

> **سياق خريطة الطريق:** NX-5 - المسار العام المتكامل. يعرض وقت التشغيل الآن خيارًا احتياطيًا `nexus.routing_policy.default_lane` لنقاط النهاية REST/gRPC لـ Torii ويمكن لكل SDK حذفه بأمان `lane_id` عندما يتعلق الأمر بحركة المرور في المسار العام الكنسي. يستخدم هذا الدليل مشغلي تكوين الكتالوج والتحقق من الإجراء الاحتياطي في `/status` وممارسة سلوك العميل من نقطة إلى نقطة.

## المتطلبات الأساسية

- قم ببناء Sora/Nexus من `irohad` (نفذ `irohad --sora --config ...`).
- الوصول إلى مستودع التكوين لتحرير `nexus.*` ثانية.
- تم تكوين `iroha_cli` للتشغيل مع مجموعة أخرى.
- `curl`/`jq` (أو ما يعادله) لاختبار الحمولة النافعة `/status` إلى Torii.

## 1. قم بفتح كتالوج الممرات ومساحات البيانات

قم بتعريف الممرات ومساحات البيانات التي يجب أن تكون موجودة بالفعل. بعد ذلك (تم تسجيله بواسطة `defaults/nexus/config.toml`) قم بتسجيل ثلاث ممرات عامة باسم مراسلي مساحة البيانات المستعار:

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

يجب أن يكون كل `index` فريدًا ومتواصلًا. معرفات مساحة البيانات لها قيم 64 بت؛ هذه الأمثلة تستخدم نفس القيم الرقمية التي توضحها مؤشرات الممرات بشكل أكبر.

## 2. تحديد أسس التناوب والخيارات الاختيارية

يتيح لك `nexus.routing_policy` التحكم في المسار الاحتياطي والسماح بالانتقال إلى تعليمات محددة أو بادئات الحساب. إذا لم يتم إعادة المراسلة، أو تقوم جدولة التحويل بالتحويل لتكوينات `default_lane` و`default_dataspace`. يعمل منطق جهاز التوجيه على `crates/iroha_core/src/queue/router.rs` ويطبق سياسة شكلية شفافة مثل أسطح REST/gRPC التي تعمل على Torii.

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

عند إضافة مسارات جديدة في المستقبل، قم بتحديث الكتالوج الأول ثم الانتهاء من التناوب. يجب أن يستمر المسار الاحتياطي في الاستمرار في المسار العام الذي يركز على الجزء الأكبر من حركة مرور المستخدمين حتى تكون بدائل SDK متوافقة بشكل دائم.

## 3. قم بإنشاء عقدة مع تطبيق سياسي

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

عقدة التسجيل السياسي المشتقة أثناء بدء التشغيل. تظهر أخطاء التحقق (المؤشرات الأصلية، الأسماء المستعارة المكررة، معرفات مساحة البيانات غير الصالحة) قبل بدء القيل والقال.

## 4. تأكيد حالة إدارة المسار

طالما أن العقدة موجودة عبر الإنترنت، استخدم مساعد CLI للتحقق من المسار الذي تم نقله (بيان الشحن) وسرعة حركة المرور. تأشيرة السيرة الذاتية تطبع خطًا من خلال المسار:

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

إذا ظهرت لوحة المسار `sealed`، فاتبع دليل التحكم في الممرات قبل السماح بحركة المرور الخارجية. علامة `--fail-on-sealed` وتستخدم لـ CI.

## 5. فحص حالة الحمولات Torii

الرد `/status` يعرض سياسة التدوير من حيث جدولة اللقطة أو المسار. استخدم `curl`/`jq` لتأكيد الإعدادات التي تم تكوينها والتحقق من المسار الاحتياطي الذي يتم إنتاجه عن بعد:

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

لاستكشاف جهات الاتصال الحية في جدولة المسار `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

يؤكد هذا أن لقطة TEU، والأسماء المستعارة، وعلامات البيان موجودة في التكوين. يتم استخدام نفس الحمولة باستخدام Grafana للوحة القيادة الخاصة بإدخال المسار.

## 6. قم بتمرين نظام العميل

- **Rust/CLI.** `iroha_cli` وحذف عميل الصندوق Rust من النطاق `lane_id` عندما لا يمرر `--lane-id` / `LaneSelector`. قم بتوجيه جهاز التوجيه إلى `default_lane`. استخدم العلامات الصريحة `--lane-id`/`--dataspace-id` للرجوع إلى المسار في الممر.
- **JS/Swift/Android.** كما تم إصدار أحدث إصدارات SDK لـ `laneId`/`lane_id` كخيارات وخيارات احتياطية للقيمة المعلنة لـ `/status`. الحفاظ على سياسة التناوب المتزامنة بين التدريج والإنتاج حتى لا تتحرك التطبيقات بشكل دقيق لإعادة تكوين حالات الطوارئ.
- **اختبارات خطوط الأنابيب/SSE.** مرشحات أحداث المعاملات هذه هي `tx_lane_id == <u32>` (حتى `docs/source/pipeline.md`). Assine `/v2/pipeline/events/transactions` مع هذا المرشح لإثبات أنك تكتب دون تحديد المسار بوضوح أو معرف المسار الاحتياطي.

## 7. إمكانية المراقبة والحوكمة

- `/status` كما تم نشر `nexus_lane_governance_sealed_total` و`nexus_lane_governance_sealed_aliases` لكي يقوم Alertmanager بإبلاغك عند فقدان المسار الخاص بك. احتفظ بهذه التنبيهات المؤهلة مع شبكات التطوير.
- خريطة القياس عن بعد للجدولة ولوحة التحكم في الممرات (`dashboards/grafana/nexus_lanes.json`) لرصد الاسم المستعار/الحلقة الثابتة للكتالوج. إذا قمت بإعادة تسمية اسم مستعار، فأعد توجيه مديري Kura المراسلين ليحتفظ المدققون بمحددات الكاميرة (rastreado sob NX-1).
- تتضمن المحادثات الحوارية للممرات خطة التراجع. قم بتسجيل التجزئة وأدلة الحوكمة جنبًا إلى جنب مع هذه البداية السريعة في دليل المشغل الخاص بك حتى لا يتم تحديد الحالة المطلوبة في المستقبل.

بعد اجتياز عمليات التحقق هذه، يمكنك استخدام `nexus.routing_policy.default_lane` كمصدر حقيقي لتكوين حزم SDK والبدء في إلغاء تمكين طرق التشفير البديلة للمسار الفريد على الإنترنت.