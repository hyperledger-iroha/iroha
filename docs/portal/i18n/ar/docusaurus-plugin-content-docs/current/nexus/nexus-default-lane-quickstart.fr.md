---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: nexus-default-lane-quickstart
العنوان: دليل المسار السريع الافتراضي (NX-5)
Sidebar_label: دليل المسار السريع افتراضيًا
الوصف: قم بتكوين المسار الاحتياطي والتحقق من المسار الافتراضي لـ Nexus حتى يتمكن Torii وSDK من فتح ممر_id على الممرات العامة.
---

:::ملاحظة المصدر الكنسي
صفحة Cette تتكرر `docs/source/quickstart/default_lane.md`. احرص على محاذاة نسختين حتى يصل balayage de localization إلى البوابة.
:::

# توجيه المسار السريع افتراضيًا (NX-5)

> **خريطة الطريق السياقية:** NX-5 - تكامل المسار العام افتراضيًا. يكشف وقت التشغيل عن خلل احتياطي `nexus.routing_policy.default_lane` حتى تتمكن نقاط النهاية REST/gRPC من Torii وكل SDK من التمكن من تأمين `lane_id` تمامًا عندما تظهر حركة المرور على المسار العام الكنسي. يرافق هذا الدليل المشغلين لتكوين الكتالوج والتحقق من الإجراء الاحتياطي في `/status` واختبار سلوك العميل في كل مرة.

## المتطلبات الأساسية

- بناء Sora/Nexus من `irohad` (lancer `irohad --sora --config ...`).
- الوصول إلى مستودع التكوين لتعديل الأقسام `nexus.*`.
- `iroha_cli` قم بتكوين محادثة الكتلة cible.
- `curl`/`jq` (أو ما يعادله) للمفتش الحمولة `/status` من Torii.

## 1. قم بتحديد كتالوج الممرات ومساحات البيانات

قم بتعريف الممرات ومساحات البيانات التي توجد على الشبكة. L'extrait ci-dessous (إطار `defaults/nexus/config.toml`) يسجل ثلاث ممرات منشورة بالإضافة إلى الاسم المستعار لمساحة البيانات المقابلة:

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

Chaque `index` doit etre Unique et contigu. معرفات مساحة البيانات هي ذات قيم 64 بت؛ تستخدم الأمثلة الموضحة القيم الرقمية التي يزيد مؤشر الممر من وضوحها.

## 2. تحديد القيم الافتراضية للتوجيه والرسوم الإضافية الاختيارية

يتحكم القسم `nexus.routing_policy` في المسار الاحتياطي ويسمح بزيادة التوجيه من أجل التعليمات المحددة أو بادئات الحساب. إذا لم يكن هناك أي تنظيم يتوافق، فسيتم تكوين جدولة المعاملة عبر `default_lane` و`default_dataspace`. يوجد منطق جهاز التوجيه في `crates/iroha_core/src/queue/router.rs` ويطبق أسلوب سياسة شفاف على الأسطح REST/gRPC de Torii.

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

Lorsque vous ajoutez plus plus d'une nouvellesways, mettez d'abord a jour le catalogue, puis etendez les regles de Routage. يجب أن يستمر المسار الاحتياطي في توجيه المؤشر نحو المسار العام الذي يحمل معظم مستخدم حركة المرور حتى تستمر SDK في العمل.

## 3. ابدأ تجربة جديدة باستخدام السياسة المطبقة

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

إن الصحافة الجديدة لسياسة المسار مستمدة من البدء. يتم إصلاح كل خطأ في التحقق (الفهرس المفقود، الأسماء المستعارة المزدوجة، معرفات مساحة البيانات غير الصالحة) قبل ظهور القيل والقال.

## 4. تأكيد حالة إدارة الممر

مرة واحدة على الإنترنت مرة أخرى، استخدم مساعد CLI للتحقق من أن المسار الافتراضي هو ثابت (رسوم واضحة) ومناسب لحركة المرور. La vue recap affiche une ligne par lane :

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

إذا تم عرض المسار الافتراضي `sealed`، فاتبع دليل إدارة الممرات قبل ترخيص حركة المرور الخارجية. العلم `--fail-on-sealed` مفيد لـ CI.

## 5. فحص حمولات الحالة Torii

يعرض الرد `/status` سياسة التوجيه بالإضافة إلى لحظة جدولة المسار. استخدم `curl`/`jq` لتأكيد القيم حسب التكوينات الافتراضية والتحقق من المسار الاحتياطي الناتج عن القياس عن بعد:

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

من أجل مفتش الحسابات مباشرة من جدولة المسار `0` :

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

يؤكد ذلك أن TEU الفورية والبيانات الاسمية ومؤشرات البيان تتماشى مع التكوين. يتم استخدام الحمولة الصافية بواسطة اللوحات Grafana لاستيعاب الممرات على لوحة القيادة.

## 6. اختبار القيم الافتراضية للعملاء

- **Rust/CLI.** `iroha_cli` وصندوق العميل الصدأ يحذف البطل `lane_id` عندما لا تمر `--lane-id` / `LaneSelector`. يتم إعادة توجيه جهاز التوجيه إلى `default_lane`. استخدم العلامات الواضحة `--lane-id`/`--dataspace-id` الفريدة عند تشغيل مسار غير افتراضي.
- **JS/Swift/Android.** تم الإعلان عن الإصدارات الأحدث من SDK `laneId`/`lane_id` كخيارات وقيمة مُعلنة على أساس `/status`. حافظ على سياسة التوجيه متزامنة بين التدريج والإنتاج حتى لا تحتاج تطبيقات الهاتف المحمول إلى إعادة التكوين العاجل.
- **اختبارات الأنابيب/SSE.** تقبل مرشحات أحداث المعاملات المسندات `tx_lane_id == <u32>` (تظهر `docs/source/pipeline.md`). أدخل `/v1/pipeline/events/transactions` مع هذا الفلتر للتأكد من أن الكتب المرسلة بدون مسار صريح تصل إلى معرف المسار الاحتياطي.

## 7. إمكانية المراقبة ونقاط الوصول إلى الحكم

- `/status` نشر أيضًا `nexus_lane_governance_sealed_total` و`nexus_lane_governance_sealed_aliases` من أجل تنبيه مدير التنبيهات الذي يمكن تجنبه عندما يختفي المسار. Gardez ces التنبيهات النشطة meme sur les devnets.
- تظهر بطاقة القياس عن بعد للجدولة ولوحة التحكم في الممرات (`dashboards/grafana/nexus_lanes.json`) تحت الأسماء المستعارة/رابط الكتالوج. إذا قمت بتسمية اسم مستعار، فأعد ضبط ذخيرة Kura المقابلة حتى يحتفظ المدققون بمحددات الكيميائيات (التابعة لـ NX-1).
- تتضمن الموافقات الحوارية للممرات الافتراضية خطة للتراجع. قم بتسجيل تجزئة البيان وإجراءات الحوكمة في هذا الوضع السريع في مشغل دفتر التشغيل الخاص بك حتى لا تتمكن الدورات المستقبلية من تحديد الحالة المطلوبة.

بعد انتهاء عمليات التحقق هذه، يمكنك تعيين `nexus.routing_policy.default_lane` كمصدر حقيقي لتكوين SDK والبدء في إلغاء تنشيط سلاسل التعليمات البرمجية ذات المسار الأحادي على الشبكة.