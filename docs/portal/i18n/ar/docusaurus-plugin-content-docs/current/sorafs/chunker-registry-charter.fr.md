---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-charter.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: ميثاق التسجيل مقسم
العنوان: مخطط التسجيل Chunker SoraFS
Sidebar_label: مقطع تسجيل المخطط
الوصف: ميثاق حوكمة لموافقات وموافقات الملفات الشخصية.
---

:::ملاحظة المصدر الكنسي
هذه الصفحة تعكس `docs/source/sorafs/chunker_registry_charter.md`. قم بمزامنة النسختين حتى تكتمل مجموعة أبو الهول الموروثة.
:::

# ميثاق إدارة السجل chunker SoraFS

> **المصدقة:** 2025-10-29 من قبل لجنة البنية التحتية البرلمانية في سورا (تصويت)
> `docs/source/sorafs/council_minutes_2025-10-29.md`). تعديل كامل exige un
> التصويت النموذجي للحوكمة؛ تعتمد معدات التنفيذ على هذه الوثيقة
> المعيار هو الموافقة على ميثاق الاستبدال.

يحدد هذا المخطط العملية والأدوار التي ستؤدي إلى تطوير سجل القطع SoraFS.
أكمل [دليل إنشاء ملفات التعريف مقسم](./chunker-profile-authoring.md) في تعليق جديد
الملفات الشخصية هي مقترحات، ومراجعة، وتم التصديق عليها، ونهائية منخفضة القيمة.

## بورتيه

يتم تطبيق المخطط على كل إدخال من `sorafs_manifest::chunker_registry` et
إلى جميع الأدوات التي تستهلك التسجيل (CLI الواضح، CLI لإعلان الموفر،
أدوات تطوير البرمجيات). Elle فرض الثوابت الاسمية والتعامل مع التحقق من قدم المساواة
`chunker_registry::ensure_charter_compliance()` :- معرف الملف الشخصي عبارة عن مجموعة من الإيجابيات التي تزيد من رتابة الصورة.
- يظهر المقبض canonique `namespace.name@semver` **doit** لأول مرة
- السلاسل الاسمية مشذبة وفريدة ولا تتصادم مع المقابض
  canoniques d'autres entrées.

## الأدوار

- **المؤلف (المؤلفون)** – يتولى إعداد الاقتراح، وينظم التركيبات ويجمعها
  preuves de déterminisme.
- **مجموعة عمل الأدوات (TWG)** – التحقق من صحة الاقتراح بمساعدة قوائم المراجعة
  تم النشر والتأكد من احترام ثوابت السجل.
- **مجلس الإدارة (GC)** – دراسة تقرير TWG، توقيع مغلف الاقتراح
  والموافقة على رزنامة النشر/الإهلاك.
- **فريق التخزين** – يحافظ على تنفيذ التسجيل والنشر
  les Misses à jour de documentaires.

## تدفق دورة الحياة

1. **موافقة الاقتراح**
   - يقوم الكاتب بتنفيذ قائمة التحقق من صحة دليل المؤلف والإنشاء
     un JSON `ChunkerProfileProposalV1` sous
     `docs/source/sorafs/proposals/`.
   - قم بتضمين عملية CLI الخاصة بـ:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - عرض تركيبات محتوى العلاقات العامة، والاقتراح، وتقرير التحديد، وما إلى ذلك
     ما زلت في يوم التسجيل.2. ** أدوات المراجعة (TWG) **
   - قم بتجديد قائمة التحقق من الصحة (التركيبات، الزغب، بيان خط الأنابيب/PoR).
   - قم بتنفيذ `cargo test -p sorafs_car --chunker-registry` والتحقق من ذلك
     `ensure_charter_compliance()` يمر بالمدخل الجديد.
   - التحقق من سلوك CLI (`--list-profiles`، `--promote-profile`، البث
     `--json-out=-`) يعكس الأسماء المستعارة والمقابض في الوقت الحالي.
   - قم بإعداد تقرير محكمة يلخص الإحصائيات وحالة النجاح/الفشل.

3. **موافقة المجلس (GC)**
   - Examiner le Rapport TWG et les métadonnées de la proposition.
   - التوقيع على خلاصة الاقتراح (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     وإضافة التوقيعات إلى مغلف المجلس مع الاستمرار في التركيبات.
   - إرسال نتيجة التصويت في محضر الحوكمة.

4. **النشر**
   - دمج العلاقات العامة في يومنا هذا :
     -`sorafs_manifest::chunker_registry_data`.
     - التوثيق (`chunker_registry.md`، أدلة التأليف/المطابقة).
     - تركيبات وتقارير التحديد.
   - قم بإخطار المشغلين ومعدات SDK بالملف التعريفي الجديد والطرح المسبق.

5. **الاستهلاك / التراجع**
   - المقترحات التي تحل محل الملف الشخصي الموجود يجب أن تتضمن نافذة للنشر
     مزدوج (فترات نعمة) وخطة ترقية.
   - بعد انتهاء صلاحية نافذة النعمة، قم بتغيير الملف الشخصي باعتباره مستهلكًا
     في السجل وقياس دفتر الأستاذ للترحيل.6. ** تغييرات عاجلة **
   - تتطلب عمليات القمع أو الإصلاحات العاجلة تصويتًا للأغلبية.
   - تقوم TWG بتوثيق خطوات تخفيف المخاطر ومتابعة يومية الحادث.

## أدوات الحضور

- `sorafs_manifest_chunk_store` و`sorafs_manifest_stub` المكشوف :
  - `--list-profiles` لفحص السجل.
  - `--promote-profile=<handle>` لإنشاء كتلة métadonnées canonique utilisé
    أثناء الترويج للملف الشخصي.
  - `--json-out=-` لدفق التقارير عبر الوضع القياسي، مما يسمح بسجلات العرض
    الاستنساخ.
- `ensure_charter_compliance()` تم استدعاؤه للبدء في الثنائيات المعنية
  (`manifest_chunk_store`، `provider_advert_stub`). اختبارات CI تفعل ذلك
  دي نوفيلس تدخل عنيفة على الرسم البياني.

## التسجيل

- Stocker tous les Rapports de déterminisme dans `docs/source/sorafs/reports/`.
- دقائق الاستشارة المرجعية للقرارات تنبض بالحياة
  `docs/source/sorafs/migration_ledger.md`.
- Mettre à jour `roadmap.md` et `status.md` بعد كل تغيير قاهرة في السجل.

## المراجع

- دليل الإنشاء : [دليل إنشاء ملفات التعريف مقسم](./chunker-profile-authoring.md)
- قائمة التحقق من المطابقة: `docs/source/sorafs/chunker_conformance.md`
- مرجع السجل: [تسجيل ملفات التعريف مقسم](./chunker-registry.md)