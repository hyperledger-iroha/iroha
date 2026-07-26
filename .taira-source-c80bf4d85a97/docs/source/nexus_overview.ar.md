<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: ar
direction: rtl
source: docs/source/nexus_overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bda1352ff13cc866cd02a08f9db6be962798b547e905f2fccf236cd803eb0eda
source_last_modified: "2025-11-08T16:26:32.878050+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/nexus_overview.md -->

# نظرة عامة على Nexus وسياق التشغيل

**رابط خارطة الطريق:** NX-14 — توثيق Nexus ودلائل تشغيل المشغلين  
**الحالة:** مسودة 2026-03-24 (مرتبطة بـ `docs/source/nexus_operations.md`)  
**الجمهور المستهدف:** مديرو البرامج، مهندسو العمليات، وفرق الشركاء الذين يحتاجون إلى
ملخص من صفحة واحدة لبنية Sora Nexus (Iroha 3) قبل التعمق في المواصفات التفصيلية
(`docs/source/nexus.md`, `docs/source/nexus_lanes.md`, `docs/source/nexus_transition_notes.md`).

## 1. مسارات الاصدار والادوات المشتركة

- **Iroha 2** ما زال المسار المستضاف ذاتيا لنشر شبكات الكونسورتيوم.
- **Iroha 3 / Sora Nexus** يقدم تنفيذا متعدد المسارات، ومساحات بيانات، وحوكمة مشتركة.
  المستودع نفسه وسلسلة الادوات وخطوط CI تبني خطي الاصدار، لذا تنعكس اصلاحات IVM
  او مترجم Kotodama او SDKs على Nexus تلقائيا.
- **القطع (Artifacts):** حزم `iroha3-<version>-<os>.tar.zst` وصور OCI تحتوي على
  الثنائيات واعدادات نموذجية وبيانات تعريف ملف Nexus. يرجع المشغلون الى
  `docs/source/sora_nexus_operator_onboarding.md` لسير عمل التحقق من القطع
  من البداية الى النهاية.
- **سطح SDK المشترك:** حزم SDK ل Rust وPython وJS/TS وSwift وAndroid تستخدم
  نفس مخططات Norito وfixtures العناوين (`fixtures/account/address_vectors.json`)
  لكي تتمكن المحافظ وعمليات التمتة من التنقل بين شبكات Iroha 2 وNexus بدون
  تفرعات في التنسيق.

## 2. لبنات البناء المعمارية

| المكون | الوصف | المراجع الرئيسية |
|--------|-------|------------------|
| **Data Space (DS)** | مجال تنفيذ مضبوط بالحوكمة يحدد عضوية المدققين، فئة الخصوصية، سياسة الرسوم، وملف توافر البيانات. كل DS يمتلك مسارا او اكثر من lanes. | `docs/source/nexus.md`, `docs/source/nexus_transition_notes.md` |
| **Lane** | شريحة حتمية للتنفيذ والحالة. تصرح مانييفستات الـlane بمجموعات المدققين، وخطافات التسوية، وبيانات القياس، واذونات التوجيه. حلقة الاجماع العالمية ترتب التزامات الـlane. | `docs/source/nexus_lanes.md` |
| **Space Directory** | عقد سجل (ومساعدات CLI) يخزن مانييفستات DS وتدوير المدققين ومنح القدرات. يحتفظ بمانييفستات تاريخية موقعة ليتمكن المدققون من اعادة بناء الحالة. | `docs/source/nexus.md#space-directory` |
| **Lane Catalog** | قسم اعدادات (`[nexus]` في `config.toml`) يربط معرفات lane بالاسماء المستعارة وسياسات التوجيه ومعلمات الاحتفاظ. يمكن للمشغلين تفقد الكتالوج الفعلي عبر `irohad --sora --config ... --trace-config`. | `docs/source/sora_nexus_operator_onboarding.md` |
| **Settlement Router** | يوجه حركات XOR بين المسارات (مثلا مسارات CBDC الخاصة <-> مسارات السيولة العامة). السياسات الافتراضية في `docs/source/cbdc_lane_playbook.md`. | `docs/source/cbdc_lane_playbook.md` |
| **Telemetry & SLOs** | لوحات وقواعد تنبيه تحت `dashboards/grafana/nexus_*.json` تلتقط ارتفاع المسارات، تراكم DA، كمون التسوية، وعمق طوابير الحوكمة. خطة المعالجة في `docs/source/nexus_telemetry_remediation_plan.md`. | `dashboards/grafana/nexus_lanes.json`, `dashboards/alerts/nexus_audit_rules.yml` |

### فئات lane ومساحات البيانات

- `default_public` lanes تثبت احمالا عامة بالكامل تحت برلمان Sora.
- `public_custom` lanes تسمح باقتصاديات خاصة بالبرامج مع بقاء الشفافية.
- `private_permissioned` lanes تخدم CBDCs او تطبيقات الكونسورتيوم؛ وتصدر الالتزامات والاثباتات فقط.
- `hybrid_confidential` lanes تمزج اثباتات المعرفة الصفرية مع خطافات افصاح انتقائية.

كل lane يعلن:

1. **مانيفست lane:** بيانات وصفية معتمدة بالحوكمة ويتتبعها Space Directory.
2. **سياسة توافر البيانات:** معلمات الترميز بالمحو، خطافات الاستعادة، ومتطلبات التدقيق.
3. **ملف القياس:** لوحات وادلة تشغيل للنداء (on-call) يجب تحديثها كلما غيرت الحوكمة مسارا.

## 3. لقطة لجدول الاطلاق

| المرحلة | التركيز | معايير الخروج |
|---------|---------|---------------|
| **N0 - Closed beta** | مسجل مدار من المجلس، نطاق `.sora` فقط، تهيئة المشغلين يدويا. | مانييفستات DS موقعة، كتالوج المسارات ثابت، تم تسجيل تدريبات الحوكمة. |
| **N1 - Public launch** | يضيف لاحقات `.nexus` والمزادات ومسجل خدمة ذاتية. التسويات موصولة بخزانة XOR. | اختبارات مزامنة resolver/gateway خضراء، لوحات مصالحة الفوترة تعمل، تمرين نزاعات مكتمل. |
| **N2 - Expansion** | يمكن `.dao` وواجهات reseller وتحليلات وبوابة نزاعات وبطاقات قياس للمشرفين. | اثار الامتثال بترقيم النسخ، ادوات policy-jury تعمل، تقارير شفافية الخزانة منشورة. |
| **NX-12/13/14 gate** | محرك الامتثال ولوحات القياس والوثائق يجب ان تصل معا قبل فتح تجربة الشركاء. | `docs/source/nexus_overview.md` + `docs/source/nexus_operations.md` منشورة، لوحات مربوطة بتنبيهات، محرك السياسة موصول بالحوكمة. |

## 4. مسؤوليات المشغل

| المسؤولية | الوصف | الدليل |
|-----------|-------|--------|
| نظافة الاعدادات | الحفاظ على `config/config.toml` متزامنا مع كتالوج المسارات ومساحات البيانات المنشور؛ وتسجيل الفروقات في التذاكر. | حفظ خرج `irohad --sora --config ... --trace-config` مع ارشيف الاصدار. |
| متابعة المانيفستات | مراقبة تحديثات Space Directory وتحديث الكاش/قوائم السماح المحلية. | حزمة مانييفست موقعة محفوظة مع تذكرة الانضمام. |
| تغطية القياس | ضمان الوصول الى اللوحات المذكورة في القسم 2، وربط التنبيهات بـ PagerDuty، وتوثيق المراجعات الفصلية. | محاضر مراجعة المناوبة + تصدير Alertmanager. |
| تقرير الحوادث | اتباع مصفوفة الشدة في `docs/source/nexus_operations.md` وتقديم تقارير ما بعد الحادث خلال خمسة ايام عمل. | قالب ما بعد الحادث محفوظ حسب معرف الحادث. |
| جاهزية الحوكمة | المشاركة في تصويت مجلس Nexus عند تغير سياسات المسارات التي تؤثر على النشر؛ وتمرين تعليمات التراجع كل ربع سنة. | حضور المجلس + قائمة تدريب تحت `docs/source/project_tracker/nexus_config_deltas/`. |

## 5. خريطة الوثائق ذات الصلة

- **المواصفات التفصيلية:** `docs/source/nexus.md`
- **هندسة المسارات وتخزينها:** `docs/source/nexus_lanes.md`
- **خطة الانتقال والتوجيه المؤقت:** `docs/source/nexus_transition_notes.md`
- **مسار انضمام المشغلين:** `docs/source/sora_nexus_operator_onboarding.md`
- **سياسة مسارات CBDC وخطة التسوية:** `docs/source/cbdc_lane_playbook.md`
- **خطة معالجة القياس وخريطة اللوحات:** `docs/source/nexus_telemetry_remediation_plan.md`
- **دليل التشغيل / مسار الحوادث:** `docs/source/nexus_operations.md`

حافظ على هذا الملخص متزامنا مع بند خارطة الطريق NX-14 كلما حدثت تغييرات جوهرية
في الوثائق المرتبطة او عند ادخال فئات مسارات جديدة او تدفقات حوكمة جديدة.

</div>
