---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-charter.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: ميثاق التسجيل مقسم
العنوان: Carta del registro de Chunker de SoraFS
Sidebar_label: بطاقة التسجيل الخاصة بالقطعة
الوصف: بطاقة إدارة العروض التقديمية وتوصيات الملفات الشخصية.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/chunker_registry_charter.md`. حافظ على النسخ المتزامنة حتى يتم سحب مجموعة وثائق Sphinx المتوارثة.
:::

# بطاقة إدارة السجل الخاص بـ SoraFS

> **تم التصديق:** 29-10-2025 بتاريخ 29/10/2025 لجنة البنية التحتية ببرلمان سوريا (الإصدار
> `docs/source/sorafs/council_minutes_2025-10-29.md`). أي إجابة تتطلب ذلك
> تصويت رسمي للحكومة؛ ينبغي لمعدات التنفيذ أن تعمل على هذه الوثيقة بنفس الطريقة
> يجب أن يتم المعياري الحصول على بطاقة بديلة.

تحدد هذه الوثيقة العملية والأدوار لتطوير سجل القطع في SoraFS.
أكمل [دليل إنشاء ملفات التعريف](./chunker-profile-authoring.md) بالوصف الجديد
يتم اقتراح الملفات الشخصية ومراجعتها والتصديق عليها وفي النهاية سيتم تخفيضها.

## الكانس

يتم تطبيق البطاقة على كل مدخل في `sorafs_manifest::chunker_registry` y
أي أداة تستخدم السجل (CLI للبيان، CLI لإعلان الموفر،
أدوات تطوير البرمجيات). قم بتفعيل ثوابت الأسماء المستعارة والتعامل مع عمليات التحقق من ذلك
`chunker_registry::ensure_charter_compliance()`:- تعتبر معرفات الملفات الشخصية إيجابية مما يزيد من رتابة الشكل.
- المقبض الكنسي `namespace.name@semver` **debe** يظهر كأول مرة
  أدخل إلى `profile_aliases`. Siguen los alias Heredados.
- يتم إعادة تسجيل سلاسل الأسماء المستعارة، وهي فريدة من نوعها ولا يتم تجميعها مع المقابض التقليدية
  من المدخلات الأخرى.

## الأدوار

- **المؤلف (المؤلفون)** – إعداد العرض وتجديد التركيبات وإعادة تجميعها
  أدلة الحتمية.
- **مجموعة عمل الأدوات (TWG)** – التحقق من صحة استخدام قوائم المراجعة
  المنشورات والتأكد من أن ثوابت السجل ستكتمل.
- **مجلس الإدارة (GC)** – مراجعة تقرير TWG، تأكيد فكرة المشروع
  واحصل على ساحات النشر/الإهمال.
- **فريق التخزين** – يحافظ على تنفيذ السجل والنشر
  تحديثات التوثيق.

## تدفق حلقة الحياة

1. **عرض تقديمي**
   - يقوم المؤلف بتنفيذ قائمة التحقق من صحة دليل التأليف والإنشاء
     un JSON `ChunkerProfileProposalV1` ar
     `docs/source/sorafs/proposals/`.
   - تضمين خروج CLI من:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - إرسال علاقة عامة تحتوي على التركيبات والعروض وتقرير الحتمية
     تحديثات السجل.2. **مراجعة الأدوات (TWG)**
   - إعادة إنتاج قائمة التحقق من الصحة (التركيبات، الزغب، خط أنابيب البيان/PoR).
   - قم بتشغيل `cargo test -p sorafs_car --chunker-registry` ثم تأكد من ذلك
     `ensure_charter_compliance()` قم بالدخول إلى المدخل الجديد.
   - التحقق من ملاءمة CLI (`--list-profiles`، `--promote-profile`، البث
     `--json-out=-`) يعكس الأسماء المستعارة ويعالج التحديثات.
   - أنتج تقريرًا موجزًا ​​يستأنف البهجة وحالة الاستحسان/الرضا.

3. **موافقة النصيحة (GC)**
   - مراجعة تقرير TWG والبيانات الوصفية الخاصة بالعرض.
   - تثبيت هضم العرض (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     وقم بإضافة الشركات حول النصائح التي يتم الحفاظ عليها جنبًا إلى جنب مع التركيبات.
   - سجل نتيجة التصويت في أعمال الإدارة.

4. **النشر**
   - دمج العلاقات العامة، التحديث:
     -`sorafs_manifest::chunker_registry_data`.
     - Documentación (`chunker_registry.md`، أدلة autoría/conformidad).
     - المباريات والتقارير الحتمية.
   - إخطار المشغلين ومعدات SDK بالملف الجديد والطرح المخطط له.

5. **الإهمال / التقاعد**
   - يجب أن تشتمل المقترحات التي تهدف إلى استبدال ملف شخصي على نافذة نشر
     مزدوج (فترات منحة) وخطة للتحديث.
     في السجل وتحديث دفتر الهجرة.6. ** عمليات الطوارئ **
   - تتطلب عمليات الإزالة أو الإصلاحات العاجلة تصويتًا بالمشورة مع الموافقة على الأغلب.
   - يجب على TWG توثيق إجراءات التخفيف من المخاطر وتحديث سجل الأحداث.

## توقعات الأدوات

- `sorafs_manifest_chunk_store` و `sorafs_manifest_stub` الموضح:
  - `--list-profiles` لفحص السجل.
  - `--promote-profile=<handle>` لإنشاء كتلة البيانات التعريفية المستخدمة
    al promover un perfil.
  - `--json-out=-` لإرسال التقارير إلى الوضع القياسي وتأهيل سجلات المراجعة
    القابلة للتكرار.
- `ensure_charter_compliance()` يتم استدعاؤه في البداية في الثنائيات ذات الصلة
  (`manifest_chunk_store`، `provider_advert_stub`). Las pruebas CI deben Fallar si
  nuevas entradas violan la carta.

## التسجيل

- حماية جميع تقارير التحديد في `docs/source/sorafs/reports/`.
- نصائح العمل التي تشير إلى قرارات التقطيع الحية
  `docs/source/sorafs/migration_ledger.md`.
- تم تحديث `roadmap.md` و`status.md` بعد كل تغيير كبير في السجل.

## المراجع

- دليل السلطة: [دليل سلطة ملفات التعريف](./chunker-profile-authoring.md)
- قائمة التحقق من المطابقة: `docs/source/sorafs/chunker_conformance.md`
- مرجع السجل: [سجل ملفات التعريف](./chunker-registry.md)