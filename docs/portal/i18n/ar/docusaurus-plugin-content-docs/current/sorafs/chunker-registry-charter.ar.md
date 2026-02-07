---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-charter.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: ميثاق التسجيل مقسم
العنوان: ميثاق سجل Chunker لـ SoraFS
Sidebar_label: ميثاق السجل Chunker
الوصف: ميثاق ال تور لتصبح ملفات Chunker واعتمادها.
---

:::ملحوظة المصدر مؤهل
احترام هذه الصفحة `docs/source/sorafs/chunker_registry_charter.md`. احرص على جميع النسختين متزامنتين إلى أن يتم إيقاف مجموعة Sphinx القديمة.
:::

# ميثاق وتسجل Chunker في SoraFS

> **مُصدَّق عليه:** 2025-10-29 من قبل لجنة البنية التحتية لبرلمان سورا (انظر
> `docs/source/sorafs/council_minutes_2025-10-29.md`). أي تعديلات تصويت لصلاحية؛
> ويجب على فرق تنفيذ اعتماد هذه الكلية القياسية حتى يعتمد ميثاق بديل.

يحدد هذا الميثاق الحراري والدوار وسجل chunker في SoraFS.
وهو يكمل [دليل تأليف ملفات Chunker](./chunker-profile-authoring.md) عبر وصف كيفية تركيب الملفات الجديدة ومراجعتها
وصديقها ثم إيقافها لاحقاً.

## النطاق

يستخدم الميثاق على كل خطوط في `sorafs_manifest::chunker_registry` وعلى
أي أداة يستهلكها السجل (بيان واجهة سطر الأوامر (CLI) وإعلان مقدم الخدمة (CLI) ومجموعات SDK). ويطبّق ثوابت الاسم المستعار والتعامل معها
تتحقق منها `chunker_registry::ensure_charter_compliance()`:

- تسجيل البيانات الشخصية بشكل صحيح.
- يجب أن يظهر المقبض المؤهل `namespace.name@semver` كأول التدفق في `profile_aliases`.
  تليه البدائل القديمة.
- يتم تحديد الخطوط البدائلة بدقة محددة ولا تتعارض مع المحدودات المعتمدة بالخطوط الأخرى.

##التعديل- **المؤلف (المؤلفون)** – عذرون المقترح، ويعيدون توليد التركيبات، ويجمعون بسبب الحتمية.
- **Tooling Working Group (TWG)** – ويوافق من يقترح اعتماد وثيقة الاعتماد المنشورة ويضمن سجل قواعد الثبات.
- **مجلس الإدارة (GC)** – يراجع تقرير TWG، ويوقع ظرف المقترح، ويوافق على جداول النشر/الإيقاف.
- **Storage Team** – إدارة السجل ونشر تحديثات التوثيق.

## سير العمل عبر دورة الحياة

1. **التقديم المقترح**
   - ينفذ قائمة المؤلفات والتحقق من دليل التأليف والمنشئ JSON من النوع `ChunkerProfileProposalV1` ضمنًا
     `docs/source/sorafs/proposals/`.
   -أخرج مخرجات CLI من:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - ومعلومات العلاقات العامة تحتوي على التركيبات والمقترحات وتقرير الحتمية وتحديثات السجل.

2. **أدوات المراجعة (TWG)**
   - إعداد قائمة التحقق (التركيبات، الزغب، خط البيان/PoR).
   - شغّل `cargo test -p sorafs_car --chunker-registry` وتأكد من النجاح
     `ensure_charter_compliance()` مع الحقن الجديد.
   - تحقق من أن تسيطر على CLI (`--list-profiles`, `--promote-profile`, التدفق
     `--json-out=-`) يعكس البدائل والمقابض المحدثة.
   - بلاغ رسمي عاجلاً يلخص النتائج وحالة النجاح/الفشل.

3. **موافقة المجلس (GC)**
   - تقرير راجع TWG وبيانات المقترحة.
   - ملخص الاتفاق المقترح (`blake3("sorafs-chunker-profile-v1" || bytes)`) وأضف التواقيع إلى
     ظرف المجلس المرفق مع التركيبات.
   - سجّل نتيجة التصويت في محاضر ال تور.

4. **النشر**
   - ادمج العلاقات العامة مع تحديث:
     -`sorafs_manifest::chunker_registry_data`.
     - التوثيق (`chunker_registry.md` وأدلة التأليف/المطابقة).
     - المباريات و التقارير الحتمية .
   - أخطر إطلاقين وفرق SDK بالملف الجديد وثمرة الإطلاق.5. **الإيقاف / الإزالة**
   - يجب أن تتضمن المقترحات التي تستبدل ملفاً قائماً في نافذة نشر مزدوجة (فترات سماح) وترقية.

6. **تغييرات طارئة**
   - طلبات الإزالة أو الإصلاحات العاجلة تصويتا بالغالبية من المجلس.
   - يجب على TWG العديد من التقارير المتعددة والتحديثات اللاحقة.

## أدوات التوقعات

- `sorafs_manifest_chunk_store` و `sorafs_manifest_stub` يوفّران:
  - `--list-profiles` لفحص السجل.
  - `--promote-profile=<handle>` كتلة البيانات المعتمدة المستعملة عند ترقية ملف.
  - `--json-out=-` لبث التقارير إلى stdout، مما يتيح مراجعة السجلات القابلة لإعادة الإنتاج.
- يتم الاتصال بـ `ensure_charter_compliance()` عند تشغيل الثنائيات ذات الصلة
  (`manifest_chunk_store`، `provider_advert_stub`). يجب أن تفشل السيولة CI إذا كانت
  الإدخالات الجديدة تتهك الميثاق.

## حفظ تسجيل

- خزّن كل التقارير الحتمية في `docs/source/sorafs/reports/`.
- محاضر المجلس الذي يشير إلى chunker محفوظة ضمنا
  `docs/source/sorafs/migration_ledger.md`.
- تحديث `roadmap.md` و `status.md` بعد كل تغيير كبير في السجل.

## المراجع

- دليل التأليف: [دليل تأليف ملفات Chunker](./chunker-profile-authoring.md)
- قائمة المهام: `docs/source/sorafs/chunker_conformance.md`
- سجل مرجعي: [سجل ملفات Chunker](./chunker-registry.md)