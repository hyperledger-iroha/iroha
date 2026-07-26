---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-conformance.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: توافق القطعة
العنوان: دليل المادة في SoraFS
Sidebar_label: شريط المقالات
الوصف: متطلبات و تراكيب العمل على ملف Chunker الحتمي SF1 عبر التركيبات و SDKs.
---

:::ملحوظة المصدر مؤهل
احترام هذه الصفحة `docs/source/sorafs/chunker_conformance.md`. احرص على التأكد من النسختين متزامنتين إلى أن يتم إيقاف الوثائق القديمة.
:::

ويؤكد هذا الدليل المتطلبات التي يجب على كل تطبيق يتبعها ومتوافقاً مع ملف Chunker الحتمي في SoraFS (SF1).
كما توثق سير إعادة التوليد، وسياسة التوقيع، وخطوات التحقق كي يبقى مستهلكو التركيبات عبر SDKs متزامنين.

##المؤهل

- ملف الملف: `sorafs.sf1@1.0.0` (البديل القديم `sorafs.sf1@1.0.0`)
- بذرة الإدخال (عرافة): `0000000000dec0ded`
- الحجم المستهدف: 262144 بايت (256 كيلو بايت)
-الحجم: 65536 بايت (64 كيلو بايت)
- الحجم الأقصى: 524288 بايت (512 كيلو بايت)
-الخطوط المتعددة التدحرج: `0x3DA3358B4DC173`
- بذرة جدول العتاد: `sorafs-v1-gear`
- قناع القطع : `0x0000FFFF`

التطبيق المرجعي: `sorafs_chunker::chunk_bytes_with_digests_profile`.
يجب أن ينتج أي تسريع SIMD نفس الحدود والـ الهضم.

## حزمة التركيبات

`cargo run --locked -p sorafs_chunker --bin export_vectors` إعادة توليد
التجهيزات ويصدر الملفات التالية ضمن `fixtures/sorafs_chunker/`:- `sf1_profile_v1.{json,rs,ts,go}` — النطاق المعتمد لمستهلكي Rust و TypeScript و Go.
  أعلن كل ما هو مؤهل مؤهل كأول التوقيع في `profile_aliases`، يريده أي خيار بديل (مثل
  `sorafs.sf1@1.0.0` ثم `sorafs.sf1@1.0.0`). يتم فرض الترتيب بواسطة
  `ensure_charter_compliance` ولا يجب تغييره.
- `manifest_blake3.json` — تم التحقق من البيان منه عبر BLAKE3 ويغطي كل ملفات التركيبات.
- `manifest_signatures.json` — توقيعات المجلس (Ed25519) على الملخص الخاص بالـ Manifest.
- `sf1_profile_v1_backpressure.json` والـ corpora داخل الجسم `fuzz/` —
  سيناريوهات تركت حتمية لاستغلال السيولة في الضغط الخلفي للـchunker.

### لنقل التوقيع

يجب أن تشمل إعادة توليد التركيبات توقيعاً صالحاً من المجلس. يرفض المولد
النتيجة غير الموثقة ما يتم تشخيصه `--allow-unsigned` صراحة (مخصص
للتجاره المحليه فقط). أظرف إلحاق التوقيع فقط لإزالة التكرارات حسب المشهود.

متابعة توقيع من المجلس:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## التحقق

أعاد مساعد CI `ci/check_sorafs_fixtures.sh` تشغيل المولد مع
`--locked`. إذا انحرفت التركيبات أو غابت التواقيع، فستفشل المهمة. استخدم
هذا السكربت في سير العمل نقص التغييرات الجديدة في التركيبات.

خطوات التحقق المصنوعة يدوياً:

1. شغّل `cargo test -p sorafs_chunker`.
2.نفّذ `ci/check_sorafs_fixtures.sh` محلياً.
3. تأكد من أن `git status -- fixtures/sorafs_chunker` نظيف.

## دليل الترقية

عند تركيب ملف Chunker جديد أو تحديث SF1:

أنظر: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) لمتطلبات
البيانات الوصفية والقوالب المقترحة وقوائم التحقق.1. صِغ `ChunkProfileUpgradeProposalV1` (انظر RFC SF-1) بمعلمات جديدة.
2. إنشاء تركيبات عبر `export_vectors` وسجل الملخص الجديد للبيان.
3. وقّع الـ المانيفست بتخصص المجلس المطلوبة. لا بد من كل التواقيع بـ `manifest_signatures.json`.
4. تحديث التركيبات الخاصة بـ SDKs المتأثرة (Rust/Go/TS) والتأكد من التوافق عبر بيئات التشغيل.
5. إعادة إنشاء الزغب الجسدي إذا تغيرت المعلمات.
6. وجه هذا الدليل بالمقبض الجديد للملف والبذور والملخص.
7. التغيير الجديد مع خارطة الطريق المحدثة وتحديثات.

التغييرات التي قررت على حدود الـ Chunk أو الـ Digest دون اتباع هذه التصرفات
غير صالحة ولا يجب دمجها.