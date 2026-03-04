---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: carta-registro-fragmento
título: ميثاق سجل fragmentador لـ SoraFS
sidebar_label: ميثاق سجل fragmentador
descripción: ميثاق الحوكمة لتقديم ملفات chunker واعتمادها.
---

:::nota المصدر المعتمد
Utilice el botón `docs/source/sorafs/chunker_registry_charter.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة توثيق Sphinx القديمة.
:::

# ميثاق حوكمة سجل fragmentador في SoraFS

> **مُصادَق عليه:** 2025-10-29 من قبل Panel de Infraestructura del Parlamento de Sora (انظر
> `docs/source/sorafs/council_minutes_2025-10-29.md`). أي تعديلات تتطلب تصويت حوكمة رسمي؛
> ويجب على فرق التنفيذ اعتبار هذه الوثيقة معيارية حتى يعتمد ميثاق بديل.

Utilice el procesador de datos SoraFS.
وهو يكمل [دليل تأليف ملفات fragmentador](./chunker-profile-authoring.md) عبر وصف كيفية اقتراح الملفات الجديدة ومراجعتها
وتصديقها ثم إيقافها لاحقاً.

## النطاق

ينطبق الميثاق على كل إدخال في `sorafs_manifest::chunker_registry` وعلى
Hay herramientas disponibles (CLI de manifiesto, CLI de anuncio de proveedor y SDK). ويُطبّق ثوابت alias y manejar التي
Nombre del usuario `chunker_registry::ensure_charter_compliance()`:

- معرفات الملفات أعداد صحيحة موجبة تزيد بشكل رتيب.
- يجب أن يظهر المقبض المعتمد `namespace.name@semver` كأول إدخال في `profile_aliases`.
  تليه البدائل القديمة.
- سلاسل البدائل يتم قصها وتكون فريدة ولا تتعارض مع المقابض المعتمدة في إدخالات أخرى.

## الأدوار- **Autor(es)** – يُعدّون المقترح، ويعيدون توليد accesorios, ويجمعون أدلة الحتمية.
- **Grupo de Trabajo sobre Herramientas (TWG)** – يتحقق من المقترح باستخدام قوائم التحقق المنشورة ويضمن ثبات قواعد السجل.
- **Consejo de Gobernanza (GC)** – يراجع تقرير TWG, ويوقع ظرف المقترح، ويوافق على جداول النشر/الإيقاف.
- **Equipo de almacenamiento** – يدير تنفيذ السجل وينشر تحديثات التوثيق.

## سير العمل عبر دورة الحياة

1. **تقديم المقترح**
   - ينفذ المؤلف قائمة التحقق من دليل التأليف ويُنشئ JSON from نوع `ChunkerProfileProposalV1` ضمن
     `docs/source/sorafs/proposals/`.
   - Haga clic en CLI para:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - قدّم PR يحتوي على accesorios والمقترح وتقرير الحتمية وتحديثات السجل.

2. **herramientas de montaje (TWG)**
   - أعد تشغيل قائمة التحقق (accesorios, fuzz, manifiesto/PoR).
   - شغّل `cargo test -p sorafs_car --chunker-registry` Y تأكد منجاح
     `ensure_charter_compliance()` مع الإدخال الجديد.
   - Aplicación CLI (`--list-profiles`, `--promote-profile`, transmisión
     `--json-out=-`) يعكس البدائل والمقابض المحدثة.
   - قدّم تقريراً قصيراً يلخص النتائج وحالة النجاح/الفشل.

3. **موافقة المجلس (GC)**
   - راجع تقرير TWG y بيانات المقترح.
   - وقّع resumen المقترح (`blake3("sorafs-chunker-profile-v1" || bytes)`) وأضف التواقيع إلى
     ظرف المجلس المرفق مع accesorios.
   - سجّل نتيجة التصويت في محاضر الحوكمة.

4. **النشر**
   - ادمج PR مع تحديث:
     - `sorafs_manifest::chunker_registry_data`.
     - التوثيق (`chunker_registry.md` وأدلة التأليف/المطابقة).
     - accesorios وتقارير الحتمية.
   - أخطر المشغلين وفرق SDK بالملف الجديد وخطة الإطلاق.5. **الإيقاف / الإزالة التدريجية**
   - يجب أن تتضمن المقترحات التي تستبدل ملفاً قائماً نافذة نشر مزدوجة (فترات سماح) وخطة ترقية.

6. **تغييرات طارئة**
   - تتطلب الإزالة أو الإصلاحات العاجلة تصويتاً بالأغلبية من المجلس.
   - يجب على TWG توثيق خطوات الحد من المخاطر وتحديث سجل الحوادث.

## herramientas de توقعات

- `sorafs_manifest_chunk_store` e `sorafs_manifest_stub` siguientes:
  - `--list-profiles` لفحص السجل.
  - `--promote-profile=<handle>` لتوليد كتلة البيانات المعتمدة المستخدمة عند ترقية ملف.
  - `--json-out=-` لبث التقارير إلى stdout, مما يتيح سجلات مراجعة قابلة لإعادة الإنتاج.
- يتم استدعاء `ensure_charter_compliance()` عند تشغيل الثنائيات ذات الصلة
  (`manifest_chunk_store`, `provider_advert_stub`). يجب أن تفشل اختبارات CI إذا كانت
  الإدخالات الجديدة تنتهك الميثاق.

## حفظ السجلات

- خزّن كل تقارير الحتمية في `docs/source/sorafs/reports/`.
- محاضر المجلس التي تشير إلى قرارات chunker محفوظة ضمن
  `docs/source/sorafs/migration_ledger.md`.
- حدّث `roadmap.md` e `status.md` بعد كل تغيير كبير في السجل.

## المراجع

- دليل التأليف: [دليل تأليف ملفات fragmentador](./chunker-profile-authoring.md)
- Número de modelo: `docs/source/sorafs/chunker_conformance.md`
- مرجع السجل: [سجل ملفات fragmentador](./chunker-registry.md)