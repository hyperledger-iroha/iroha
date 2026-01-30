---
id: chunker-registry-charter
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-charter.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note المصدر المعتمد
تعكس هذه الصفحة `docs/source/sorafs/chunker_registry_charter.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة توثيق Sphinx القديمة.
:::

# ميثاق حوكمة سجل chunker في SoraFS

> **مُصادَق عليه:** 2025-10-29 من قبل Sora Parliament Infrastructure Panel (انظر
> `docs/source/sorafs/council_minutes_2025-10-29.md`). أي تعديلات تتطلب تصويت حوكمة رسمي؛
> ويجب على فرق التنفيذ اعتبار هذه الوثيقة معيارية حتى يعتمد ميثاق بديل.

يحدد هذا الميثاق العملية والأدوار لتطوير سجل chunker في SoraFS.
وهو يكمل [دليل تأليف ملفات chunker](./chunker-profile-authoring.md) عبر وصف كيفية اقتراح الملفات الجديدة ومراجعتها
وتصديقها ثم إيقافها لاحقاً.

## النطاق

ينطبق الميثاق على كل إدخال في `sorafs_manifest::chunker_registry` وعلى
أي tooling يستهلك السجل (manifest CLI و provider-advert CLI و SDKs). ويُطبّق ثوابت alias و handle التي
تتحقق منها `chunker_registry::ensure_charter_compliance()`:

- معرفات الملفات أعداد صحيحة موجبة تزيد بشكل رتيب.
- يجب أن يظهر المقبض المعتمد `namespace.name@semver` كأول إدخال في `profile_aliases`.
  تليه البدائل القديمة.
- سلاسل البدائل يتم قصها وتكون فريدة ولا تتعارض مع المقابض المعتمدة في إدخالات أخرى.

## الأدوار

- **Author(s)** – يُعدّون المقترح، ويعيدون توليد fixtures، ويجمعون أدلة الحتمية.
- **Tooling Working Group (TWG)** – يتحقق من المقترح باستخدام قوائم التحقق المنشورة ويضمن ثبات قواعد السجل.
- **Governance Council (GC)** – يراجع تقرير TWG، ويوقع ظرف المقترح، ويوافق على جداول النشر/الإيقاف.
- **Storage Team** – يدير تنفيذ السجل وينشر تحديثات التوثيق.

## سير العمل عبر دورة الحياة

1. **تقديم المقترح**
   - ينفذ المؤلف قائمة التحقق من دليل التأليف ويُنشئ JSON من نوع `ChunkerProfileProposalV1` ضمن
     `docs/source/sorafs/proposals/`.
   - أدرج مخرجات CLI من:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - قدّم PR يحتوي على fixtures والمقترح وتقرير الحتمية وتحديثات السجل.

2. **مراجعة tooling (TWG)**
   - أعد تشغيل قائمة التحقق (fixtures، fuzz، خط manifest/PoR).
   - شغّل `cargo test -p sorafs_car --chunker-registry` وتأكد من نجاح
     `ensure_charter_compliance()` مع الإدخال الجديد.
   - تحقق من أن سلوك CLI (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`) يعكس البدائل والمقابض المحدثة.
   - قدّم تقريراً قصيراً يلخص النتائج وحالة النجاح/الفشل.

3. **موافقة المجلس (GC)**
   - راجع تقرير TWG وبيانات المقترح.
   - وقّع digest المقترح (`blake3("sorafs-chunker-profile-v1" || bytes)`) وأضف التواقيع إلى
     ظرف المجلس المرفق مع fixtures.
   - سجّل نتيجة التصويت في محاضر الحوكمة.

4. **النشر**
   - ادمج PR مع تحديث:
     - `sorafs_manifest::chunker_registry_data`.
     - التوثيق (`chunker_registry.md` وأدلة التأليف/المطابقة).
     - fixtures وتقارير الحتمية.
   - أخطر المشغلين وفرق SDK بالملف الجديد وخطة الإطلاق.

5. **الإيقاف / الإزالة التدريجية**
   - يجب أن تتضمن المقترحات التي تستبدل ملفاً قائماً نافذة نشر مزدوجة (فترات سماح) وخطة ترقية.

6. **تغييرات طارئة**
   - تتطلب الإزالة أو الإصلاحات العاجلة تصويتاً بالأغلبية من المجلس.
   - يجب على TWG توثيق خطوات الحد من المخاطر وتحديث سجل الحوادث.

## توقعات tooling

- `sorafs_manifest_chunk_store` و `sorafs_manifest_stub` يوفّران:
  - `--list-profiles` لفحص السجل.
  - `--promote-profile=<handle>` لتوليد كتلة البيانات المعتمدة المستخدمة عند ترقية ملف.
  - `--json-out=-` لبث التقارير إلى stdout، مما يتيح سجلات مراجعة قابلة لإعادة الإنتاج.
- يتم استدعاء `ensure_charter_compliance()` عند تشغيل الثنائيات ذات الصلة
  (`manifest_chunk_store`, `provider_advert_stub`). يجب أن تفشل اختبارات CI إذا كانت
  الإدخالات الجديدة تنتهك الميثاق.

## حفظ السجلات

- خزّن كل تقارير الحتمية في `docs/source/sorafs/reports/`.
- محاضر المجلس التي تشير إلى قرارات chunker محفوظة ضمن
  `docs/source/sorafs/migration_ledger.md`.
- حدّث `roadmap.md` و `status.md` بعد كل تغيير كبير في السجل.

## المراجع

- دليل التأليف: [دليل تأليف ملفات chunker](./chunker-profile-authoring.md)
- قائمة مطابقة: `docs/source/sorafs/chunker_conformance.md`
- مرجع السجل: [سجل ملفات chunker](./chunker-registry.md)
