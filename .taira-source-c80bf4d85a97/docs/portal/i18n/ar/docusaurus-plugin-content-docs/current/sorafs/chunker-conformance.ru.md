---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-conformance.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: توافق القطعة
العنوان: Руководство по соответствию Chunker SoraFS
Sidebar_label: Соответствие Chunker
الوصف: البحث وسير العمل من أجل تحديد ملف تعريف Chunker SF1 في التركيبات ومجموعات SDK.
---

:::note Канонический источник
:::

يجب أن يكون هذا المستند ضروريًا للتنفيذ بعد ذلك
نلتزم بالمواصفات المحددة لملف التعريف الخاص بـchunker SoraFS (SF1). مرة أخرى
توثيق تجديدات سير العمل والملاحظات السياسية والتحقق منها
يقوم مشتري التركيبات في SDKs بمزامنة المزامنة.

## الملف الشخصي الكنسي

- البذور Водной (ست عشرية): `0000000000dec0ded`
- الحجم الخلوي: 262144 بايت (256 كيلو بايت)
- الحجم الأدنى: 65536 بايت (64 كيلو بايت)
- الحجم الأقصى: 524288 بايت (512 كيلو بايت)
- كثير الحدود المتداول: `0x3DA3358B4DC173`
- معدات أقراص البذور: `sorafs-v1-gear`
- قناع الاستراحة: `0x0000FFFF`

تحقيق إيطالي: `sorafs_chunker::chunk_bytes_with_digests_profile`.
يجب على أي شخص يرغب في الحصول على SIMD التحقق من حبيبات وهضمات متطابقة.

## تركيبات نابوري

تم تجديد `cargo run --locked -p sorafs_chunker --bin export_vectors`
التركيبات وتعبئة الملفات التالية في `fixtures/sorafs_chunker/`:- `sf1_profile_v1.{json,rs,ts,go}` — الجرانيت الكنسي للعلبة
  يمكنك شراء Rust وTypeScript وGo. يتم تسليم كل ملف إلى المقبض الأساسي كيف
  `sorafs.sf1@1.0.0`، ثم `sorafs.sf1@1.0.0`). Порадок fixiruется
  `ensure_charter_compliance` ولا يتم حذفها.
- `manifest_blake3.json` — بيان BLAKE3 الذي تم التحقق منه، تركيبات الملف المثبت.
- `manifest_signatures.json` — راجع الرسالة (Ed25519) في أعلى ملخص البيان.
- `sf1_profile_v1_backpressure.json` والمجموعات الكبيرة في `fuzz/` —
  يتم استخدام تحديد سيناريوهات البث في اختبار الضغط الخلفي.

### مقالة سياسية

تشتمل تركيبات التجديد **الجهد** على مكونات صالحة. مولد كهربائي
قم بإيقاف تشغيل الاتصال غير المصرح به، إذا لم يسبق له مثيل `--allow-unsigned` (مسبق
فقط للتجارب المحلية). تحويلات подписей إلحاق فقط و
إلغاء التكرار من خلال التسليم.

تريد إضافة هذه النصائح:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## التحقق

مساعد CI `ci/check_sorafs_fixtures.sh` مولد الغلق الخلفي مع
`--locked`. إذا تم إرسال التركيبات أو تقديم طلب، فستكون الوظيفة مناسبة. استخدم
هذا البرنامج النصي في سير العمل ليلاً ويسبق التركيبات المتغيرة.

بعض الأسئلة البسيطة:

1. أدخل `cargo test -p sorafs_chunker`.
2. استخدم `ci/check_sorafs_fixtures.sh` محليًا.
3. يرجى ملاحظة أن `git status -- fixtures/sorafs_chunker` موجود.

## اشتراك بلاي بوك

قبل الملف الشخصي المقترح الجديد Chunker أو SF1:سم. أيضا: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
الحاجة إلى التحقق من صحة العناصر المقترحة والاختيارات.

1. قم بتوصيل `ChunkProfileUpgradeProposalV1` (باسم RFC SF-1) بالمعلمات الجديدة.
2. قم بإعادة تجديد التركيبات من خلال `export_vectors` وقم بطباعة بيان الملخص الجديد.
3. قم بإدراج البيان المطلوب بشأن رغبتك. كل ما تحتاج إليه هو تقديم
   تم الإضافة إلى `manifest_signatures.json`.
4. قم بشراء تركيبات SDK المجانية (Rust/Go/TS) وحافظ على التكافؤ بين وقت التشغيل.
5. قم بإعادة تنظيم المجموعة الزغبية من خلال تغيير المعلمات.
6. تعرف على هذا المقبض الجديد المتطور والبذور والهضم.
7. قم بالتغيير على الفور باستخدام التوصيات وخريطة الطريق.

التحسين الذي يغذي الحبوب أو الهضم دون الانضمام إلى هذه العملية,
يتم دمج الأشياء غير الضرورية وغير المجهدة.