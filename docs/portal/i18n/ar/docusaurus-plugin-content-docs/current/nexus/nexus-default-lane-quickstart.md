---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-default-lane-quickstart
title: البدء السريع لـ lane الافتراضي (NX-5)
sidebar_label: البدء السريع لـ lane الافتراضي
description: اضبط وتحقق من fallback لـ lane الافتراضي في Nexus لكي تتمكن Torii و SDKs من حذف lane_id في lanes العامة.
---

:::note المصدر الرسمي
تعكس هذه الصفحة `docs/source/quickstart/default_lane.md`. حافظ على النسختين متطابقتين حتى يصل مسح التوطين إلى البوابة.
:::

# البدء السريع لـ lane الافتراضي (NX-5)

> **سياق خارطة الطريق:** NX-5 - تكامل lane العام الافتراضي. بيئة التشغيل تعرض الآن fallback `nexus.routing_policy.default_lane` كي تتمكن نقاط النهاية REST/gRPC في Torii وكل SDK من حذف `lane_id` بأمان عندما تنتمي الحركة إلى lane العام canonical. يوجه هذا الدليل المشغلين لإعداد الكتالوج، والتحقق من fallback في `/status`، واختبار سلوك العميل من البداية للنهاية.

## المتطلبات المسبقة

- نسخة Sora/Nexus من `irohad` (شغّل `irohad --sora --config ...`).
- وصول إلى مستودع الإعدادات حتى تتمكن من تعديل أقسام `nexus.*`.
- `iroha_cli` مهيأ للتحدث مع عنقود الهدف.
- `curl`/`jq` (أو ما يعادله) لفحص حمولة `/status` في Torii.

## 1. وصف كتالوج lane و dataspace

أعلن عن lanes و dataspaces التي يجب أن توجد على الشبكة. المقتطف أدناه (مقتطع من `defaults/nexus/config.toml`) يسجل ثلاث lanes عامة إضافة إلى aliases للـ dataspace المطابقة:

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

يجب أن يكون كل `index` فريدا ومتتاليا. معرفات dataspace هي قيم 64-بت؛ وتستخدم الأمثلة أعلاه القيم الرقمية نفسها كفهارس lane للوضوح.

## 2. ضبط افتراضات التوجيه والتجاوزات الاختيارية

قسم `nexus.routing_policy` يتحكم في lane الاحتياطي ويسمح بتجاوز التوجيه لتعليمات محددة أو بادئات الحسابات. إذا لم تطابق أي قاعدة، يوجه scheduler المعاملة إلى `default_lane` و `default_dataspace` المحددين. منطق router موجود في `crates/iroha_core/src/queue/router.rs` ويطبق السياسة بشكل شفاف على واجهات Torii REST/gRPC.

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

عند إضافة lanes جديدة لاحقا، حدّث الكتالوج أولا ثم وسّع قواعد التوجيه. يجب أن يظل lane الاحتياطي يشير إلى lane العام الذي يحمل غالبية حركة المستخدمين حتى تبقى SDKs القديمة متوافقة.

## 3. إقلاع عقدة مع تطبيق السياسة

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

تسجل العقدة سياسة التوجيه المشتقة أثناء الإقلاع. تظهر أي أخطاء تحقق (فهارس مفقودة، aliases مكررة، معرفات dataspace غير صالحة) قبل بدء gossip.

## 4. تأكيد حالة حوكمة lane

بمجرد أن تصبح العقدة online، استخدم أداة CLI للتحقق من أن lane الافتراضي مختوم (manifest محمّل) وجاهز للحركة. تعرض النظرة الملخصة صفا لكل lane:

```bash
iroha_cli nexus lane-report --summary
```

Example output:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

إذا كان lane الافتراضي يعرض `sealed`، اتبع runbook حوكمة lane قبل السماح بحركة خارجية. علم `--fail-on-sealed` مفيد لـ CI.

## 5. فحص حمولة حالة Torii

استجابة `/status` تعرض سياسة التوجيه ولقطة scheduler لكل lane. استخدم `curl`/`jq` لتأكيد الافتراضات المضبوطة والتحقق من أن lane الاحتياطي ينتج القياس عن بعد:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Sample output:

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

لفحص عدادات scheduler الحية للـ lane `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

هذا يؤكد أن لقطة TEU وبيانات alias ورايات manifest تتطابق مع الإعداد. نفس الحمولة تستخدمها لوحات Grafana لعرض lane-ingest.

## 6. اختبار افتراضات العميل

- **Rust/CLI.** `iroha_cli` و crate عميل Rust يحذفان حقل `lane_id` عندما لا تمرر `--lane-id` / `LaneSelector`. لذلك يرجع queue router إلى `default_lane`. استخدم الأعلام الصريحة `--lane-id`/`--dataspace-id` فقط عند استهداف lane غير افتراضي.
- **JS/Swift/Android.** أحدث إصدارات SDK تعامل `laneId`/`lane_id` كاختيارية وتعود إلى القيمة المعلنة في `/status`. حافظ على سياسة التوجيه متزامنة بين staging و production حتى لا تحتاج تطبيقات الهاتف لإعادة تهيئة طارئة.
- **Pipeline/SSE tests.** مرشحات أحداث المعاملات تقبل الشرط `tx_lane_id == <u32>` (انظر `docs/source/pipeline.md`). اشترك في `/v1/pipeline/events/transactions` بهذا الشرط لإثبات أن الكتابات المرسلة بدون lane صريح تصل تحت معرف lane الاحتياطي.

## 7. المراقبة وروابط الحوكمة

- `/status` ينشر ايضا `nexus_lane_governance_sealed_total` و `nexus_lane_governance_sealed_aliases` كي يتمكن Alertmanager من التحذير عندما تفقد lane manifest. ابق هذه التنبيهات مفعلة حتى في devnets.
- خريطة القياس للـ scheduler ولوحة حوكمة lanes (`dashboards/grafana/nexus_lanes.json`) تتوقع حقول alias/slug من الكتالوج. إذا اعدت تسمية alias، اعد تسمية دلائل Kura المقابلة كي يحافظ المدققون على مسارات حتمية (متابعة تحت NX-1).
- موافقات البرلمان لـ lanes الافتراضية يجب ان تتضمن خطة rollback. سجّل hash الـ manifest وأدلة الحوكمة بجانب هذا الدليل في runbook المشغل حتى لا تضطر الدورات المستقبلية لتخمين الحالة المطلوبة.

