---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: chunker-conformance
title: دليل مطابقة chunker في SoraFS
sidebar_label: مطابقة chunker
description: متطلبات وتدفقات عمل للحفاظ على ملف chunker الحتمي SF1 عبر fixtures و SDKs.
---

:::note المصدر المعتمد
تعكس هذه الصفحة `docs/source/sorafs/chunker_conformance.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف الوثائق القديمة.
:::

يوثق هذا الدليل المتطلبات التي يجب على كل تطبيق اتباعها للبقاء متوافقاً مع ملف chunker الحتمي في SoraFS (SF1).
كما يوثق سير إعادة التوليد، وسياسة التوقيع، وخطوات التحقق كي يبقى مستهلكو fixtures عبر SDKs متزامنين.

## الملف المعتمد

- مقبض الملف: `sorafs.sf1@1.0.0` (البديل القديم `sorafs.sf1@1.0.0`)
- بذرة الإدخال (hex): `0000000000dec0ded`
- الحجم المستهدف: 262144 bytes (256 KiB)
- الحجم الأدنى: 65536 bytes (64 KiB)
- الحجم الأقصى: 524288 bytes (512 KiB)
- متعدد الحدود المتدحرج: `0x3DA3358B4DC173`
- بذرة جدول gear: `sorafs-v1-gear`
- قناع القطع: `0x0000FFFF`

التطبيق المرجعي: `sorafs_chunker::chunk_bytes_with_digests_profile`.
يجب أن ينتج أي تسريع SIMD نفس الحدود والـ digests.

## حزمة fixtures

`cargo run --locked -p sorafs_chunker --bin export_vectors` يعيد توليد
fixtures ويصدر الملفات التالية ضمن `fixtures/sorafs_chunker/`:

- `sf1_profile_v1.{json,rs,ts,go}` — حدود chunk المعتمدة لمستهلكي Rust و TypeScript و Go.
  يعلن كل ملف المقبض المعتمد كأول إدخال في `profile_aliases`، يتبعه أي بدائل قديمة (مثل
  `sorafs.sf1@1.0.0` ثم `sorafs.sf1@1.0.0`). يتم فرض الترتيب بواسطة
  `ensure_charter_compliance` ولا يجب تغييره.
- `manifest_blake3.json` — manifest تم التحقق منه عبر BLAKE3 ويغطي كل ملفات fixtures.
- `manifest_signatures.json` — توقيعات المجلس (Ed25519) على digest الخاص بالـ manifest.
- `sf1_profile_v1_backpressure.json` والـ corpora الخام داخل `fuzz/` —
  سيناريوهات بث حتمية تُستخدم في اختبارات back-pressure للـ chunker.

### سياسة التوقيع

يجب أن تشمل إعادة توليد fixtures توقيعاً صالحاً من المجلس. يرفض المولد
الإخراج غير الموقّع ما لم يتم تمرير `--allow-unsigned` صراحة (مخصص
للتجارب المحلية فقط). أظرف التوقيع append-only ويتم إزالة التكرارات حسب الموقّع.

لإضافة توقيع من المجلس:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## التحقق

يعيد مساعد CI `ci/check_sorafs_fixtures.sh` تشغيل المولد مع
`--locked`. إذا انحرفت fixtures أو غابت التواقيع، تفشل المهمة. استخدم
هذا السكربت في workflows الليلية وقبل إرسال تغييرات fixtures.

خطوات التحقق اليدوية:

1. شغّل `cargo test -p sorafs_chunker`.
2. نفّذ `ci/check_sorafs_fixtures.sh` محلياً.
3. تأكد أن `git status -- fixtures/sorafs_chunker` نظيف.

## دليل الترقية

عند اقتراح ملف chunker جديد أو تحديث SF1:

انظر أيضاً: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) لمتطلبات
البيانات الوصفية وقوالب المقترح وقوائم التحقق.

1. صِغ `ChunkProfileUpgradeProposalV1` (انظر RFC SF-1) بمعلمات جديدة.
2. أعد توليد fixtures عبر `export_vectors` وسجل digest الجديد للـ manifest.
3. وقّع الـ manifest بحصة المجلس المطلوبة. يجب إلحاق كل التواقيع بـ `manifest_signatures.json`.
4. حدّث fixtures الخاصة بـ SDKs المتأثرة (Rust/Go/TS) وتأكد من التكافؤ عبر بيئات التشغيل.
5. أعد توليد corpora fuzz إذا تغيرت المعلمات.
6. حدّث هذا الدليل بالمقبض الجديد للملف والبذور وdigest.
7. قدّم التغيير مع الاختبارات المحدثة وتحديثات roadmap.

التغييرات التي تؤثر على حدود الـ chunk أو الـ digests دون اتباع هذه العملية
غير صالحة ولا يجب دمجها.
