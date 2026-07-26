---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-conformance.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: توافق القطعة
العنوان: Guía de Conformidad del Chunker de SoraFS
Sidebar_label: مطابقة المقطع
الوصف: المتطلبات والتدفقات للحفاظ على الملف المحدد لـ Chunker SF1 والتركيبات وSDKs.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/chunker_conformance.md`. حافظ على الإصدارات المتزامنة حتى يتم سحب المستندات المتوارثة.
:::

هذا الدليل مقنن للمتطلبات التي يجب اتباعها للتنفيذ جميعًا
تم تصميمه باستخدام ملف تعريف القطع SoraFS (SF1). تامبيان
توثيق تدفق التجديد وسياسة الشركات وإجراءات التحقق من ذلك
يمكن لمستهلكي التركيبات في حزم SDK المزامنة دائمًا.

## بيرفيل كانونيكو

- مقبض الملف الشخصي: `sorafs.sf1@1.0.0` (الاسم المستعار المنشأ `sorafs.sf1@1.0.0`)
- بذرة المدخل (ست عشرية): `0000000000dec0ded`
- حجم الهدف: 262144 بايت (256 كيلو بايت)
- الحجم الأدنى: 65536 بايت (64 كيلو بايت)
- الحجم الأقصى: 524288 بايت (512 كيلو بايت)
- بولينوميو دي المتداول: `0x3DA3358B4DC173`
- بذرة الطبلة والعتاد: `sorafs-v1-gear`
- قناع الاستراحة: `0x0000FFFF`

تنفيذ المرجع: `sorafs_chunker::chunk_bytes_with_digests_profile`.
أي تسريع SIMD يجب أن ينتج حدودًا ويلخص المتطابقات.

## حزمة التركيبات

`cargo run --locked -p sorafs_chunker --bin export_vectors` تجديد لوس
التركيبات وإصدار الملفات التالية `fixtures/sorafs_chunker/`:- `sf1_profile_v1.{json,rs,ts,go}` — حدود القطع القانونية للمستهلكين
  الصدأ وTypeScript والانطلاق. يعلن كل أرشيف عن المقبض القانوني باعتباره الأول
  تم إدخاله في `profile_aliases`، يتبعه أي اسم مستعار موجود (ص. على سبيل المثال،
  `sorafs.sf1@1.0.0`، لويغو `sorafs.sf1@1.0.0`). الطلب بحد ذاته impone por
  `ensure_charter_compliance` ولا يوجد تغيير في DEBE.
- `manifest_blake3.json` - تم التحقق من البيان باستخدام BLAKE3 الذي يغطي كل أرشيف التركيبات.
- `manifest_signatures.json` — شركة المشورة (Ed25519) حول ملخص البيان.
- `sf1_profile_v1_backpressure.json` وجسم كامل داخل `fuzz/` —
  سيناريوهات تحديد التدفق المستخدمة من خلال اختبارات الضغط الخلفي للمقطع.

### سياسة الشركات

يجب أن يشتمل تجديد التركيبات **يجب** على نصيحة ثابتة. الجينيرادور
قم بإعادة إخراج الخرج دون تثبيته إلى الحد الأدنى الذي يتم تمريره بشكل واضح `--allow-unsigned` (فكرة
منفردًا للتجربة المحلية). Los sobres de Firma هي ملحقة فقط بحد ذاتها
deduplican porfirmante.

من أجل الانضمام إلى شركة المشورة:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## التحقق

يقوم مساعد CI `ci/check_sorafs_fixtures.sh` بإعادة تشغيل المولد مع
`--locked`. إذا تباينت التركيبات أو الشركات الكبيرة، فستفشل المهمة. الولايات المتحدة الأمريكية
هذا البرنامج النصي عبارة عن سير عمل ليلي وقبل إرسال تغييرات التركيبات.

دليل خطوات التحقق:

1. إخراج `cargo test -p sorafs_chunker`.
2. استدعاء `ci/check_sorafs_fixtures.sh` محليًا.
3. تأكد من أن `git status -- fixtures/sorafs_chunker` غير واضح.

## دليل التحديثعند اقتراح ملف جديد للتقطيع أو SF1 الفعلي:

الإصدار أيضًا: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) لـ
متطلبات البيانات الوصفية ومجموعات النشر وقوائم التحقق من الصحة.

1. قم بتحرير `ChunkProfileUpgradeProposalV1` (إصدار RFC SF-1) بمعلمات جديدة.
2. قم بتجديد التركيبات عبر `export_vectors` وقم بتسجيل الملخص الجديد للبيان.
3. قم بتأكيد البيان وفقًا للنصيحة المطلوبة. كل شيء يجب أن يكون ثابتًا
   قم بفحص `manifest_signatures.json`.
4. تحديث تركيبات SDK المؤثرة (Rust/Go/TS) وضمان تطابقها خلال وقت التشغيل.
5. قم بتجديد زغب الجسم إذا قمت بتغيير المعلمات.
6. قم بتنشيط هذا الدليل باستخدام المقبض الجديد للملفات والبذور والهضم.
7. قم بالتغيير جنبًا إلى جنب مع تجارب التحديث وتحديثات خريطة الطريق.

التغييرات التي تؤثر على حدود القطعة أو الملخصات دون متابعة هذه العملية
ابن غير صالح ولا ينبغي اندماجه.