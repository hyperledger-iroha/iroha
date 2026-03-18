---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-charter.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: ميثاق التسجيل مقسم
العنوان: Carta do Registro de Chunker da SoraFS
Sidebar_label: بطاقة تسجيل القطع
الوصف: بطاقة التحكم للإرسال والموافقة على أداء القطع.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة تحمل عنوان `docs/source/sorafs/chunker_registry_charter.md`. Mantenha ambas as copias sincronzadas.
:::

# البطاقة الحاكمة لسجل القطع من SoraFS

> **تصديق:** 2025-10-29 بيلو سورا لجنة البنية التحتية البرلمانية (فيجا)
> `docs/source/sorafs/council_minutes_2025-10-29.md`). كل ما يتطلبه الأمر هو ذلك
> التصويت الرسمي للحكم؛ يجب على معدات التنفيذ أن تقوم بهذا المستند
> المعيار هو أن المذكرة البديلة ستتم الموافقة عليها.

تحدد هذه البطاقة العملية والورقة لتطوير سجل القطع في SoraFS.
هذا تكملة لـ [دليل أدوات القطع المثالي](./chunker-profile-authoring.md) لكشفها كجديدة
بيرفيس ساو بروبوستوس، المنقحة، المصدقة، وفي نهاية المطاف توقف.

##اسكوبو

يتم تطبيق البطاقة على كل ما تم إدخاله في `sorafs_manifest::chunker_registry` e
أي أدوات تستهلك أو تسجل (CLI الواضح، CLI لإعلان الموفر،
أدوات تطوير البرمجيات). إنها تؤثر على الأسماء المستعارة الثابتة وتتعامل مع التحقق منها
`chunker_registry::ensure_charter_compliance()`:- معرفات الملف الشخصي هي إيجابية للغاية مما يزيد من شكل رتيب.
- مقبض Canonico `namespace.name@semver` **deve** يظهر كأول مرة
  أدخل في `profile_aliases`. الأسماء المستعارة البديلة هي التالية.
- كسلاسل من الأسماء المستعارة sao aparadas وunicas e nao colidem com تتعامل مع Canonicos
  من المدخلات الأخرى.

## بابيس

- **المؤلف (المؤلفون)** - إعداد العرض وإعادة إنشاء التركيبات والتركيبات أ
  أدلة الحتمية.
- **مجموعة عمل الأدوات (TWG)** - التحقق من صحة الاقتراح المستخدم كقوائم مرجعية
  المنشورات والتأكد من أن الثوابت ستسجل حضورها.
- **مجلس الإدارة (GC)** - مراجعة أو علاقة TWG، إضافة إلى مظروف الاقتراح
  والموافقة على أسباب النشر/الإيقاف.
- **فريق التخزين** - يتولى تنفيذ عملية التسجيل والنشر
  تحديث المستندات.

## فلوكسو دو سيكلو دي فيدا

1. **إرسال الاقتراح**
   - يقوم المؤلف بتنفيذ قائمة التحقق من صحة دليل القيادة والتنفيذ
     أم JSON `ChunkerProfileProposalV1` تنهد
     `docs/source/sorafs/proposals/`.
   - تضمين مساعدة في CLI de:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - قم بإرسال تركيبات وعروض وعلاقات تحديد العلاقات العامة المتنافسة
     تم تحديث التسجيل.2. **مراجعة الأدوات (TWG)**
   - قم بإعادة إنشاء قائمة التحقق من الصحة (التركيبات، الزغب، خط أنابيب البيان/PoR).
   - نفذ `cargo test -p sorafs_car --chunker-registry` وتأكد من ذلك
     `ensure_charter_compliance()` يمر عبر المدخل الجديد.
   - التحقق من سلوك CLI (`--list-profiles`، `--promote-profile`، البث
     `--json-out=-`) يعيد فتح الأسماء المستعارة والمقابض التي تم تحديثها.
   - إنتاج علاقة قصيرة لاستئناف الأحداث وحالة الموافقة/الإصلاح.

3. ** أبوفاكاو دو كونسيلهو (GC) **
   - مراجعة علاقة TWG واقتراحات الاقتراح.
   - Assine o ملخص العرض (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     يتم إرفاقه كملحقات في المغلف من خلال تقديم المشورة بشأن التركيبات.
   - سجل نتيجة التصويت على وظائف الحوكمة.

4. ** بوبليكاكاو **
   - دمج واجهة العلاقات العامة، وتحديثها:
     -`sorafs_manifest::chunker_registry_data`.
     - Documentacao (`chunker_registry.md`، دليل التشغيل/المطابقة).
     - تركيبات وعلاقات الحتمية.
   - إخطار المشغلين ومعدات SDK حول الملف الشخصي الجديد ومستوى الطرح.

5. ** إهمال / إنسيرامنتو **
   - المقترحات التي يجب أن تتضمن استبدال ملف شخصي موجود هي صورة عامة
     مزدوج (فترة رعاية) وخطة ترقية.
     لا يوجد تسجيل أو تحديث دفتر حسابات الهجرة.6. **تعديلات الطوارئ**
   - عمليات إزالة أو إصلاحات عاجلة تطلب منك تقديم المشورة بشأن الموافقة على معظمها.
   - يتعين على TWG توثيق خطوات تخفيف المخاطر وتحديث سجل الأحداث.

## توقعات الأدوات

- `sorafs_manifest_chunk_store` و`sorafs_manifest_stub` المعرض:
  - `--list-profiles` لفحص التسجيل.
  - `--promote-profile=<handle>` لإنشاء كتلة التعريفات الكنسي المستخدمة
    ao promover um perfil.
  - `--json-out=-` لإرسال علاقات القياس وتأهيل سجلات المراجعة
    reproduziveis.
- `ensure_charter_compliance()` واستدعاء تهيئة الثنائيات ذات الصلة
  (`manifest_chunk_store`، `provider_advert_stub`). الخصيتين في CI يجب أن تفشلا
  novas entradas violarem a carta.

## التسجيل

- أرمازين جميع علاقات التحديد في `docs/source/sorafs/reports/`.
- كما ننصحك بالرجوع إلى قرارات القطع التي اتخذتها
  `docs/source/sorafs/migration_ledger.md`.
- قم بتنشيط `roadmap.md` و`status.md` من كل مودانكا مايور بدون تسجيل.

## المراجع

- دليل autoria: [دليل autoria de perfis dechunker](./chunker-profile-authoring.md)
- قائمة التحقق من المطابقة: `docs/source/sorafs/chunker_conformance.md`
- مرجع السجل: [سجل بيانات القطع](./chunker-registry.md)