---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-conformance.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: توافق القطعة
العنوان: دليل مطابقة القطعة SoraFS
Sidebar_label: مقطع مطابق
الوصف: المتطلبات وسير العمل للحفاظ على ملف التعريف الخاص بـ SF1 الذي يتم تحديده في التركيبات وحزم SDK.
---

:::ملاحظة المصدر الكنسي
:::

هذا الدليل يدون المتطلبات التي يجب أن يتبعها كل تنفيذ من أجل الباقي
تمت محاذاته مع ملف التعريف المحدد لـ SoraFS (SF1). الوثيقة أيضا
سير عمل التجديد وسياسة التوقيعات وخطوات التحقق التي
مستهلكو التركيبات في SDKs متزامنون.

## الملف الشخصي canonique

- بذرة المدخل (ست عشرية): `0000000000dec0ded`
- حجم الخط: 262144 بايت (256 كيلو بايت)
- الحد الأدنى للحجم: 65536 بايت (64 كيلو بايت)
- الحد الأقصى للحجم: 524288 بايت (512 كيلو بايت)
- متعدد حدود التدحرج : `0x3DA3358B4DC173`
- بذرة تروس الطاولة: `sorafs-v1-gear`
- قناع التمزق : `0x0000FFFF`

تنفيذ المرجع: `sorafs_chunker::chunk_bytes_with_digests_profile`.
يجب أن يؤدي تسريع SIMD إلى إنتاج حدود وخلاصات متطابقة.

## حزمة التركيبات

`cargo run --locked -p sorafs_chunker --bin export_vectors` قم بإعادة تشغيله
التركيبات وإخراج الملفات التالية `fixtures/sorafs_chunker/` :- `sf1_profile_v1.{json,rs,ts,go}` — حدود القطع الأساسية للملفات
  المستهلكون Rust وTypeScript وGo. كل ملف يعلن عن مقبض Canonique
  `sorafs.sf1@1.0.0`، ثم `sorafs.sf1@1.0.0`). L'ordre est imposé par
  تم تعديل `ensure_charter_compliance` وNE DOIT PAS.
- `manifest_blake3.json` - تم التحقق من البيان BLAKE3 الذي يغطي كل ملف من التركيبات.
- `manifest_signatures.json` - توقيعات المجلس (Ed25519) على ملخص البيان.
- `sf1_profile_v1_backpressure.json` والجسم في `fuzz/` —
  يتم استخدام سيناريوهات التدفق من خلال اختبارات الضغط الخلفي للمقطع.

### سياسة التوقيع

يتضمن تجديد التركيبات **فعل** توقيعًا صالحًا للمشورة. المولد
قم بإعادة المحاولة غير الموقعة إذا كان `--allow-unsigned` قد أصبح واضحًا (السابق)
فريدة من نوعها للتجربة المحلية). مغلفات التوقيع ملحقة فقط وآخرون
تم حذفها من قبل التوقيع.

لإضافة توقيع المجلس:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## التحقق

المساعد CI `ci/check_sorafs_fixtures.sh` يستمتع بالمولد مع
`--locked`. إذا كانت التركيبات متباينة أو إذا كانت التوقيعات متباعدة، فإن الوظيفة ستتردد. استخدم
هذا البرنامج النصي في سير العمل في الليل وقبل إجراء تغييرات على التركيبات.

خطوات التحقق اليدوية :

1. قم بتنفيذ `cargo test -p sorafs_chunker`.
2. موضع لانسيز `ci/check_sorafs_fixtures.sh`.
3. تأكد من أن `git status -- fixtures/sorafs_chunker` مناسب.

## Playbook de Mise à Niveauعندما تقترح مقطعًا جديدًا للملف الشخصي أو ما هو موجود حاليًا في SF1:

Voir aussi : [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) من أجل les
متطلبات البيانات ونماذج الاقتراحات وقوائم التحقق من الصحة.

1. قم بتثبيت `ChunkProfileUpgradeProposalV1` (البحث عن RFC SF-1) باستخدام الإعدادات الجديدة.
2. أعد التركيبات عبر `export_vectors` وأرسل ملخص البيان الجديد.
3. قم بالتوقيع على البيان مع النصاب القانوني المطلوب. Toutes les التوقيعات doivent être
   ملحق بـ `manifest_signatures.json`.
4. قم بتحديث تركيبات SDK المعنية (Rust/Go/TS) وضمان التكافؤ خلال وقت التشغيل.
5. قم بإعادة تشكيل الجسم إذا تغيرت الإعدادات.
6. استكمل هذا الدليل يوميًا بمقبض الملف الشخصي الجديد والبذور والهضم.
7. قم بإجراء التعديل من خلال الاختبارات والخطوات في خريطة الطريق.

التغييرات التي تؤثر على حدود القطعة أو الهضم دون متابعة هذه العملية
إنهم معاقون ولا ينبغي دمجهم.