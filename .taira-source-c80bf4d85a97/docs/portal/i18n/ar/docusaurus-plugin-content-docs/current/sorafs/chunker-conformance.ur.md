---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-conformance.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: توافق القطعة
العنوان: دليل مطابقة القطعة SoraFS
Sidebar_label: توافق المقطع
الوصف: تتميز التركيبات ومجموعات SDK بملف تعريف قطعي SF1 محدد لتحقق المتطلبات وسير العمل.
---

:::ملاحظة مستند ماخذ
:::

يعمل سير عمل التجديد وسياسة التوقيع وخطوات التحقق أيضًا على توثيق مجموعة أدوات SDK ومزامنة المستهلكين الثابتين.

## الملف الشخصي الكنسي

- بذرة الإدخال (ست عشري): `0000000000dec0ded`
- حجم الهدف: 262144 بايت (256 كيلو بايت)
- الحد الأدنى للحجم: 65536 بايت (64 كيلو بايت)
- الحد الأقصى للحجم: 524288 بايت (512 كيلو بايت)
- كثير الحدود المتداول: `0x3DA3358B4DC173`
- بذور طاولة التروس: `sorafs-v1-gear`
- قناع الاستراحة: `0x0000FFFF`

التنفيذ المرجعي: `sorafs_chunker::chunk_bytes_with_digests_profile`.
كما يعمل أيضًا على تسريع SIMD الذي يتجاوز الحدود ويهضم ما هو جديد.

##حزمة التركيبات

يتم تجديد التركيبات `cargo run --locked -p sorafs_chunker --bin export_vectors`
البطاقة ودرج الطبقة `fixtures/sorafs_chunker/` هي:- `sf1_profile_v1.{json,rs,ts,go}` — مستهلكو Rust وTypeScript وGo لحدود القطع الأساسية. ہر فائل
  هذا (مثلاً `sorafs.sf1@1.0.0`، أو `sorafs.sf1@1.0.0`). أو يأمر
  `ensure_charter_compliance` الذي يفرض العقاب ولا يتم تغييره.
- `manifest_blake3.json` — بيان BLAKE3 الذي تم التحقق منه
- `manifest_signatures.json` — ملخص البيان پر توقيعات المجلس (Ed25519)۔
- `sf1_profile_v1_backpressure.json` و`fuzz/` اندرر المواد الأولية —
  يتم استخدام سيناريوهات التدفق الحتمية التي يتم من خلالها استخدام اختبارات الضغط الخلفي.

### سياسة التوقيع

تجديد التركيبات **لازم** تمام لتوقيع المجلس الصحيح يشمل کرے۔ خرج المولد غير الموقع ويرفض الكرتا ويجب أن يكون `--allow-unsigned` واضحًا تمامًا (كما هو الحال في التجارب السابقة). يتم إلحاق مظاريف التوقيع فقط، ويقوم المُوقع بإلغاء التكرار.

توقيع المجلس يشمل کرنے کے لیے:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## التحقق

مساعد CI `ci/check_sorafs_fixtures.sh` مولد `--locked` قام بإعادة إنتاجه.
إذا انجرفت التركيبات أو فقدت التوقيعات، فستفشل مهمتك. هذا هو البرنامج النصي
يتم إرسال سير العمل الليلي وتغييرات التركيبات إلى قائمة الاستخدام.

خطوات التحقق اليدوي:

1. `cargo test -p sorafs_chunker` .
2.`ci/check_sorafs_fixtures.sh` لوكل چلايں.
3. التحقق من صحة `git status -- fixtures/sorafs_chunker`.

## ترقية قواعد اللعبة التي تمارسها

يقترح ملف تعريف Chunker هذا الوقت أو وقت SF1:وهذا أيضًا: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) أيضًا
متطلبات البيانات الوصفية، وقوالب المقترحات، وقوائم التحقق من الصحة.

1. تم تسجيل المعلمات الجديدة `ChunkProfileUpgradeProposalV1` (RFC SF-1).
2. `export_vectors` تعمل التركيبات على تجديد الذاكرة ولا تحتوي على ملخص واضح للسجلات.
3. يشترط نصاب المجلس ليكون علامة واضحة. تمام التوقيعات
   `manifest_signatures.json` إلحاق ہونی چاہیں.
4. يتم تشغيل تركيبات SDK (Rust/Go/TS) وتكافؤ وقت التشغيل المتبادل.
5. إذا كانت المعلمات غير قابلة للتجديد الجماعي.
6. يحتوي على مقبض الملف الشخصي والبذور وهضم الطعام.
7. قم بإجراء الاختبارات البديلة وتحديثات خارطة الطريق التي يتم إرسالها تلقائيًا.

إذا تم إجراء عملية قطع الحدود أو الملخصات، فسيتم إلغاء صلاحيتها ودمجها مرة أخرى.