---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-charter.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: ميثاق التسجيل مقسم
العنوان: هارتيا ريسترا تشانكر SoraFS
Sidebar_label: Хаartия еестра chunker
الوصف: إدارة صارمة للمبيعات وتقطيع الملف الشخصي.
---

:::note Канонический источник
هذا الجزء يعرض `docs/source/sorafs/chunker_registry_charter.md`. قم بالنسخ المتزامن، حيث لن يتم استبعاد أبو الهول من المجموعة القديمة.
:::

# التحكم في القرص المقسم SoraFS

> **الموافقة:** 2025-10-29 لجنة البنية التحتية لبرلمان سورا (см.
> `docs/source/sorafs/council_minutes_2025-10-29.md`). نريد أن نحب الصواب
> الحوكمة الرسمية; يجب أن تقوم أوامر الاتصال بقراءة هذا المستند
> معياري، حتى لا يتم إصدار بطاقة جديدة.

تحدد هذه البطاقة العملية والدور الذي تلعبه في تطور جهاز القطع SoraFS.
نحن نكمل [التكوين من خلال تأليف الملف الشخصيchunker](./chunker-profile-authoring.md)، الوصف، كما هو جديد
تقترح الملفات الشخصية، وتتنوع، وتصدق، وتتجدد من خلال الخضوع.

## Область

يتم تنفيذ الرسم البياني من خلال كتابة رقم في `sorafs_manifest::chunker_registry` و
إلى أي الأدوات التي يمكن استخدامها للمسجل (CLI الواضح، CLI للإعلان عن الموفر،
أدوات تطوير البرمجيات). قم بإصلاح الأسماء المستعارة والمقبض الثابت، التحقق منها
`chunker_registry::ensure_charter_compliance()`:

- الملف التعريفي - لوحة كاملة كاملة، رتيبة.
- المقبض الكنسي `namespace.name@semver` **الطول** يجب أن تكتب أولاً
- السكتات الدماغية الاسم المستعار متفوقة وفريدة من نوعها ولا تتعارض مع المقبض الكنسي المخطوطة الأخرى.## رولي

- **الكاتب(المنتجات)** – العروض العامة، تجديد التركيبات والاشتراك
  تأكيد الحتمية.
- **مجموعة عمل الأدوات (TWG)** – التحقق من صحة الاقتراحات المنشورة
  الاختيار والمتعة هو أن المتابعين المتغيرين ملتزمون.
- **مجلس الإدارة (GC)** – التواصل مع TWG وتقديم تحويل الاقتراحات
  وأكمل عدد من المنشورات/الإهمالات.
- **فريق التخزين** – استكمال تحقيق السجل والإعلان عنه
  وثائق الموافقة.

## دورة الحياة

1. **اقتراحات الهدية**
   - يقوم المؤلف بإنهاء عمليات التحقق من صحة التفويض والإنشاء
     JSON `ChunkerProfileProposalV1` в
     `docs/source/sorafs/proposals/`.
   - قم بإدراج خيار CLI من:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - تنفيذ العلاقات العامة، والتركيبات المشتركة، والاقتراحات، وتجاهل التصميم و
     إعادة التسجيل.

2. ** أدوات Ревью (TWG) **
   - التحقق من صحة الاختيار (التركيبات، الزغب، بيان خط الأنابيب / PoR).
   - اضغط على `cargo test -p sorafs_car --chunker-registry` وأتمنى لك ذلك
     يتم إرسال `ensure_charter_compliance()` برسالة جديدة.
   - التحقق من تحديث CLI (`--list-profiles`, `--promote-profile`, التدفق
     `--json-out=-`) يعرض الاسم المستعار والمقبض.
   - قم بالتسجيل مع المرشحين وحالة النجاح/الفشل.3. ** شركة التحسين (GC) **
   - إعادة صياغة TWG والاقتراحات التحويلية.
   - قم بنشر ملخص المقترحات (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     وقم بإضافة العناصر في تحويل الألواح التي تتناسب مع التركيبات.
   - تثبيت نتيجة الشمولية في بروتوكول الحكم.

4. **النشر**
   - ميرجيت للعلاقات العامة، ملاحظة:
     -`sorafs_manifest::chunker_registry_data`.
     - التوثيق (`chunker_registry.md`, уководства по AUTORINGU/SOOOTVETTSTIU).
     - تركيبات وأشياء من التحديد.
   - مراقبة المشغلين والأوامر SDK لملف تعريف جديد وطرح الخطة.

5. ** الإعفاء / المكافأة **
   - اقتراحات، حفظ الملفات الشخصية، بما في ذلك الضغط على منشورين مزدوجين
     (فترات زمنية) وخطة الترقية.
     قم بالتسجيل والاطلاع على دفتر أستاذ الهجرة.

6. **التغيير الخارجي**
   - تحديث أو إصلاح عاجل يحتاج إلى تغيير جذري.
   - تقوم TWG بتوثيق بعض المخاطر وتتبع أحداث المجلة.

## صيانة الأدوات

- `sorafs_manifest_chunk_store` و `sorafs_manifest_stub` يقترحان:
  - `--list-profiles` للفحص.
  - `--promote-profile=<handle>` للأجيال القادمة من الكتلة الأساسية،
    يتم استخدامه عند إنتاج الملف الشخصي.
  - `--json-out=-` لقص المنافذ في stdout، توصيل الطاقة
    سجل السجل.
- يتم تسجيل `ensure_charter_compliance()` عند الانتهاء من الندوات ذات الصلة
  (`manifest_chunk_store`، `provider_advert_stub`). يجب إجراء اختبارات CI، إذا
  كتابة جديدة للبطاقة.## التوثيق

- جميع نقاط التحديد في `docs/source/sorafs/reports/`.
- بروتوكولات الحلول المتعلقة بـ Chunker تأتي إلى
  `docs/source/sorafs/migration_ledger.md`.
- قم بإعادة إنشاء `roadmap.md` و`status.md` بعد كل عملية تسجيل مسجلة.

## مرحبا

- تفويض التأليف: [التفويض تأليف الملف الشخصي مقسم](./chunker-profile-authoring.md)
- رمز الاختيار: `docs/source/sorafs/chunker_conformance.md`
- المسجل الصحيح: [قطاعة الملف الشخصي للمسجل](./chunker-registry.md)