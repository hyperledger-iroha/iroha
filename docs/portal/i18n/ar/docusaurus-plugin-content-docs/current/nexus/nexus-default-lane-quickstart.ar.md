---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: nexus-default-lane-quickstart
العنوان: الريادة لـ Lane افتراضية (NX-5)
Sidebar_label: المراقب السريع لـ الحارة الافتراضية
description: تم ضبط وتحقق من التراجع لـlane الافتراضي في Nexus من أجل البدء في Torii وSDKs من حذف Lane_id في الحارات العامة.
---

:::ملاحظة المصدر الرسمي
احترام هذه الصفحة `docs/source/quickstart/default_lane.md`. حافظ على نسختين متطابقتين حتى يصل تنقية التوطين إلى البوابة.
:::

#المراقبة للقناة الافتراضية (NX-5)

> **سياق خارطة الطريق:** NX-5 - تكامل المسار العام الافتراضي. بيئة التشغيل تم عرضها الآن احتياطي `nexus.routing_policy.default_lane` كي بسيطة عندما يبدأ العمل إلى الحارة العامة canonical. وجه هذا الدليل إلى إعداد الكتالوج، والتحقق من الإجراء الاحتياطي في `/status`، ومراقبة العميل من البداية للنهاية.

##المتطلبات المسبقة

- نسخة Sora/Nexus من `irohad` (شغّل `irohad --sora --config ...`).
- الوصول إلى مستودع الإعدادات حتى الآن من الأقسام `nexus.*`.
- `iroha_cli` مهيأ للتحدث مع هدف الهدف.
- `curl`/`jq` (أو ما توميه) للفحص الحمولة `/status` في Torii.

## 1. وصف كتالوج حارة و مساحة بيانات

أعلنت عن الممرات ومساحات البيانات التي يجب أن توجد على الشبكة. المقتطف أدناه (مقتطع من `defaults/nexus/config.toml`) سجل ثلاث حارات عامة إضافة إلى الأسماء المستعارة لـ dataspace المطابقة:

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

يجب أن يكون كل `index` فريدا ومتاليا. معرفات مساحة البيانات هي قيم 64-بت؛ وبالتالي يتم أخذ التقديرات الرقمية بنفسه كفهارس حارة للوضوح.

## 2. ضبطات التوجيه والتجاوزات الاختيارية

قسم `nexus.routing_policy` يتحكم في المسار العادي لرسوم المرور برسومات محددة أو مبادئ ألمانية. إذا لم تطابق أي قاعدة، فقد وجه المجدول المثالي إلى `default_lane` و `default_dataspace`. جهاز التوجيه المنطقي موجود في `crates/iroha_core/src/queue/router.rs` ويطبق السياسة بشكل شفاف على واجهات Torii REST/gRPC.

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

عند إضافة الممرات جديدة لاحقاً، تصحيح الكتالوج أولا ثم وسّع قواعد التوجيه. يجب أن يغيب الخط الاحتياطي إلى المسار العام الذي تعمل عليه حركة المستخدمين حتى تبقى SDKs القديمة المرغوبة.

##3.إقلاع عقدة مع تطبيق السياسة

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

لتعلم تعلم المشتقة أثناء الإقلاع. التحقق من أي أخطاء (فهارس مفقودة، الأسماء المستعارة مكررة، معرفات مساحة البيانات غير صالحة) قبل بدء القيل والقال.

## 4.تؤكد صحة المسار

بمجرد أن تصبح على الإنترنت، استخدم أداة CLI المتاحة من أن تكون حارة افتراضية مختوم (بيان متشدد) وجاهزة للحركة. تعرض النظرة الملخصة صفا لكل حارة:

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

إذا كان الحارة الأصلية تعرض `sealed`، فاتبع runbook حارة قبل أن تشجع على الحركة. علم `--fail-on-sealed` مفيد لـ CI.

## 5. فحص وفحص الحالة Torii

عضو `/status` تم توجيهه لوجهه ولقطة جدولة لكل حارة. استخدم `curl`/`jq` لتأكيد الافتراضات المضبوطة والتحقق من أن الخط الاحتياطي يأتي القياس عن بعد:

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

لفحص عدادات جدولة الجائزة للـ حارة `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

هذا يعني أن لقطة TEU وبيانات مستعارة ورايات واضحة تتطابق مع الإعداد. نفس الحمولة تستخدمها لوحات Grafana لاستقبال الممرات.

## 6. تم اختبار افتراضات العميل

- **Rust/CLI.** `iroha_cli` و crate عميل Rust يحذفان بحث `lane_id` عندما لا تمرر `--lane-id` / `LaneSelector`. لذلك يرجع جهاز توجيه قائمة الانتظار إلى `default_lane`. استخدم الأعلام الصريحة `--lane-id`/`--dataspace-id` فقط عند استهداف حارة غير افتراضية.
- **JS/Swift/Android.** أحدث إصدار SDK تعامل `laneId`/`lane_id` كاختيارية وتعود إلى العناصر البائعة في `/status`. حافظ على تعليمات متزامنة بين التدريج والإنتاج حتى لا تحتاج إلى تطبيقات الهاتف لإعادة ظروف طارئة.
- **اختبارات الأنابيب/SSE.** مرشحات المعاملات الجامعية `tx_lane_id == <u32>` (انظر `docs/source/pipeline.md`). مرحبا في `/v2/pipeline/events/transactions` هذا هو الشرطي البلجيكي لإثبات أن الكتابات المرسلة بدون حارة صريح تصل تحت معرف الحارة الاحتياطية.

## 7.الهجوم وروابط التورم

- `/status` ينشر أيضًا `nexus_lane_governance_sealed_total` و `nexus_lane_governance_sealed_aliases` انتبه جيدًا إلى Alertmanager من التحذير عند فشل بيان المسار. ابق هذه التنبيهات مفعلة حتى في devnets.
- خريطة القياس للـ جدولة ولوحة الممرات (`dashboards/grafana/nexus_lanes.json`) تتوقع تكفي الاسم المستعار/السبيكة من الكتالوج. إذا لم يتم تحديد الاسم المستعار، لم يتم تحديد موعد محدد لكورا، كي يراقب المدققون على مسارات حتمية (المتابعة تحت NX-1).
- حسناات التربية لـ الممرات المستخدمة يجب ان تتضمن خطة التراجع. سجل تجزئة الـ البيان وأدلة الـ توم إلى هذا الدليل في دليل التشغيل لتشغيله حتى لا يتطلب الاستعداد لإتقان الحالة المطلوبة.