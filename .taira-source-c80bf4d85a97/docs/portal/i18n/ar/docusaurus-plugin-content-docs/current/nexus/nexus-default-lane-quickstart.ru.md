---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: nexus-default-lane-quickstart
العنوان: المسار الافتراضي للبداية السريعة (NX-5)
Sidebar_label: المسار الافتراضي للبدء
الوصف: قم بإنشاء المسار الافتراضي الاحتياطي والتحقق منه في Nexus، حيث يمكن لـ Torii وSDK فتح Lane_id في الممرات العامة.
---

:::note Канонический источник
هذا الجزء يعرض `docs/source/quickstart/default_lane.md`. قم بالنسخ المتزامن حتى لا يتم نشر خطة الترجمة في البوابة.
:::

# المسار الافتراضي للبداية السريعة (NX-5)

> **خريطة طريق السياق:** NX-5 - تكامل المسار العام الافتراضي. يوفر النطاق الاحتياطي `nexus.routing_policy.default_lane`، بحيث يمكن الوصول بسهولة إلى نقاط REST/gRPC Torii وبعض SDK `lane_id`، عندما تنتقل حركة المرور إلى المسار العام القانوني. هذا هو المشغل المجهز من خلال الكتالوج، ويتحقق من الإجراء الاحتياطي في `/status` ويتحقق من وصول العميل إلى النهاية كونسا.

## الرغبة المسبقة

- سورا/Nexus لـ `irohad` (لـ `irohad --sora --config ...`).
- قم بتثبيت تكوين المستودع لتعديل القسم `nexus.*`.
- `iroha_cli`، مجموعة كبيرة من الخلايا.
- `curl`/`jq` (أو ما يعادله) لعرض الحمولة النافعة `/status` في Torii.

## 1. قم بسرد مسار الكتالوج ومساحة البيانات

قم بإختيار الممرات ومساحات البيانات التي تحتاجها الأجهزة في المجموعات. الجزء الجديد (معتمد من `defaults/nexus/config.toml`) يسجل ثلاثة ممرات عامة واسم مستعار مفضل لمساحة البيانات:

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

كل `index` مطلوب فريد وغير مبتكر. مساحات بيانات المعرف - هذا هو عمر 64 بت؛ في الأمثلة، يتم استخدام كل ما هو موضح في حارة الفهارس للرؤية.

## 2. إضافة التنظيم التنظيمي والاقتراح الاختياري

يتحكم القسم `nexus.routing_policy` في المسار الاحتياطي ويتيح إعادة اقتراح التخطيط للتعليمات الملموسة أو البادئات الحسابات. إذا لم يحدث ذلك بشكل صحيح، يقوم المجدول بإدارة المعاملات في `default_lane` و`default_dataspace`. يصل جهاز التوجيه المنطقي إلى `crates/iroha_core/src/queue/router.rs` ويعيد بشكل واضح السياسة إلى Torii REST/gRPC.

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


## 3. ابدأ حياتك بالسياسة الأولية

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

إنها بداية سياسة شاملة قبل البدء. التحقق من صحة صحة الأشياء (مؤشرات المراجعة، الاسم المستعار المضاف، مساحات بيانات المعرفات غير الصحيحة) ينضم إلى بداية القيل والقال.

## 4. تحسين حوكمة الممر

بعد الاتصال بالإنترنت، استخدم مساعد CLI لتتمكن من تحديد المسار الافتراضي المغلق (البيان المضمن) والدخول إلى حركة المرور. ينبعث الماء من مسافة واحدة إلى المسار:

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

إذا حددت الممرات الافتراضية `sealed`، فاتبع دليل التشغيل للتحكم في الممرات التي تسبقها، حتى تتمكن من استكشاف حركة المرور الأخرى. العلم `--fail-on-sealed` سهل لـ CI.

## 5. التحقق من حالة الحمولة Torii

قم بالرد على `/status` لتقسيم السياسات وجدولة الصور عبر الممرات. استخدم `curl`/`jq` للتحقق من أهمية التشجيع والتحقق من نشر المسار الاحتياطي القياس عن بعد:

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

كيفية استخدام برنامج جدولة البيانات الحية للمسار `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

وهذا يؤكد أن لقطة TEU والاسم المستعار التحويلي والعلامات الواضحة هي التكوينات المرغوبة. تستخدم هذه الحمولة لوحة Grafana لاستيعاب حارة لوحة القيادة.

## 6. التحقق من العميل التلقائي

- **Rust/CLI.** `iroha_cli` وصناديق العملاء الصدأ يملأ القطب `lane_id`، عندما لا تسبق `--lane-id` / `LaneSelector`. يتم توجيه جهاز توجيه قائمة الانتظار في هذه الحالة إلى `default_lane`. استخدم العلم الجديد `--lane-id`/`--dataspace-id` فقط عند العمل بمسار غير افتراضي.
- **JS/Swift/Android.** تشير الإصدارات الأحدث من SDK إلى `laneId`/`lane_id` الاختيارية والاحتياطية، وهي متاحة مجانًا `/status`. بسبب السياسة المتزامنة بين التدريج والإنتاج، لا يتطلب الأمر أن تكون تطبيقات الهاتف المحمول تغيير.
- **اختبارات خطوط الأنابيب/SSE.** مرشحات مبدأ المعاملات الخاصة `tx_lane_id == <u32>` (برمز `docs/source/pipeline.md`). قم بالإشارة إلى `/v1/pipeline/events/transactions` باستخدام هذا الفلتر للإشارة إلى أنه يتم التحكم فيه بدون حارة جديدة، ويتم الحصول على معرف المسار الاحتياطي.

## 7. خطافات إمكانية الملاحظة والحوكمة

- `/status` يتم أيضًا نشر `nexus_lane_governance_sealed_total` و`nexus_lane_governance_sealed_aliases`، حيث يمكن لـ Alertmanager توقع عندما يتم عرض البيان. اتبع هذه التنبيهات المضمنة حتى الآن على devnet.
- تقوم بطاقة جدولة القياس عن بعد وإدارة لوحة المعلومات للممرات (`dashboards/grafana/nexus_lanes.json`) بإظهار بعض الاسم المستعار/الحلقة الثابتة من الكتالوج. إذا قمت بتعديل الاسم المستعار، قم بإعادة توجيه وأدلة Kura الرائعة ليتمكن مدققو الحسابات من تحديد مساراتهم (يتبع NX-1).
- تشمل الموافقة البرلمانية للممرات الافتراضية التراجع عن الخطة. قم بتأكيد بيان التجزئة وحوكمة التضمين باستخدام هذا العنصر Quickstart في دليل التشغيل الخاص بمشغل شبكة الجوال، بحيث لا يلزم إجراء تدوير متكرر الحالة.

عندما يتم التحقق من ذلك، يمكنك قراءة `nexus.routing_policy.default_lane` الإعدادات الأصلية لتكوين SDK والبدء في إلغاء الحظر غير المتبع ممرات رمزية ذات مسار واحد في مجموعات.