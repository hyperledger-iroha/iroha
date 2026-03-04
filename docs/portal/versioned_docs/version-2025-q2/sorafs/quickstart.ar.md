---
lang: ar
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 79a048e6061f7054e14a471004cf7da0dddd3f9bf627d9f1d20ff63803cb0979
source_last_modified: "2026-01-04T17:06:14.405886+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS التشغيل السريع

يتجول هذا الدليل العملي عبر ملف تعريف القطع الحتمي SF-1،
توقيع البيان وتدفق الجلب متعدد الموفرين الذي يدعم SoraFS
خط أنابيب التخزين. قم بإقرانه مع [الغوص العميق لخط أنابيب البيان](manifest-pipeline.md)
للحصول على ملاحظات التصميم والمواد المرجعية لعلامة CLI.

## المتطلبات الأساسية

- سلسلة أدوات الصدأ (`rustup update`)، تم استنساخ مساحة العمل محليًا.
- اختياري: [زوج المفاتيح Ed25519 الذي تم إنشاؤه بواسطة OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  لتوقيع البيانات.
- اختياري: Node.js ≥ 18 إذا كنت تخطط لمعاينة بوابة Docusaurus.

قم بتعيين `export RUST_LOG=info` أثناء تجربة عرض رسائل CLI المفيدة.

## 1. قم بتحديث التركيبات الحتمية

إعادة إنشاء ناقلات تقطيع SF-1 الكنسي. الأمر ينبعث أيضا موقعة
مظاريف البيان عند توفير `--signing-key`؛ استخدم `--allow-unsigned`
خلال التنمية المحلية فقط.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

النواتج:

-`fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
-`fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (إذا تم التوقيع)
-`fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. قم بتقطيع الحمولة وفحص الخطة

استخدم `sorafs_chunker` لتقطيع ملف أو أرشيف عشوائي:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

الحقول الرئيسية:

- `profile` / `break_mask` - يؤكد معلمات `sorafs.sf1@1.0.0`.
- `chunks[]` – الإزاحات المطلوبة والأطوال وملخصات BLAKE3.

بالنسبة للتركيبات الأكبر حجمًا، قم بتشغيل الانحدار المدعوم من Proptest لضمان التدفق و
يظل تقطيع الدُفعات متزامنًا:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. قم بإنشاء بيان وتوقيعه

قم بتغليف خطة القطعة والأسماء المستعارة وتوقيعات الإدارة في بيان باستخدام
`sorafs-manifest-stub`. يعرض الأمر أدناه حمولة ملف واحد؛ تمرير
مسار دليل لحزم شجرة (تقوم واجهة سطر الأوامر (CLI) بتوجيهها بشكل معجمي).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

مراجعة `/tmp/docs.report.json` من أجل:

- `chunking.chunk_digest_sha3_256` - ملخص SHA3 للإزاحات/الأطوال، يطابق
  تركيبات مقسمة.
- `manifest.manifest_blake3` - ملخص BLAKE3 موقع في مظروف البيان.
- `chunk_fetch_specs[]` - تعليمات الجلب المطلوبة للمنسقين.

عندما تصبح جاهزًا لتوفير التوقيعات الحقيقية، قم بإضافة `--signing-key` و`--signer`
الحجج. يتحقق الأمر من كل توقيع Ed25519 قبل كتابة ملف
المغلف.

## 4. محاكاة الاسترجاع متعدد الموفرين

استخدم المطور لجلب CLI لإعادة تشغيل خطة القطعة مقابل واحد أو أكثر
مقدمي الخدمات. يعد هذا مثاليًا لاختبارات دخان CI والنماذج الأولية للمنسق.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

التأكيدات:

- يجب أن يتطابق `payload_digest_hex` مع تقرير البيان.
- يظهر `provider_reports[]` عدد مرات النجاح/الفشل لكل موفر.
- يسلط `chunk_retry_total` غير الصفر الضوء على تعديلات الضغط الخلفي.
- قم بتمرير `--max-peers=<n>` لتحديد عدد مقدمي الخدمة المجدولين للتشغيل
  والحفاظ على تركيز محاكاة CI على المرشحين الأساسيين.
- يتجاوز `--retry-budget=<n>` عدد مرات إعادة المحاولة الافتراضي لكل مجموعة (3) حتى تتمكن من
  يمكن أن يظهر انحدارات الأوركسترا بشكل أسرع عند فشل الحقن.

إضافة `--expect-payload-digest=<hex>` و`--expect-payload-len=<bytes>` للفشل
بسرعة عندما تنحرف الحمولة المعاد بناؤها عن البيان.

## 5. الخطوات التالية- **تكامل الحوكمة** - قم بتوصيل ملخص البيان و
  `manifest_signatures.json` في سير عمل المجلس حتى يتمكن سجل Pin من ذلك
  الإعلان عن التوفر.
- **التفاوض بشأن التسجيل** – راجع [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  قبل تسجيل الملفات الشخصية الجديدة. يجب أن تفضل الأتمتة المقابض الأساسية
  (`namespace.name@semver`) عبر المعرفات الرقمية.
- **أتمتة CI** - قم بإضافة الأوامر أعلاه لتحرير خطوط الأنابيب بحيث تكون المستندات،
  تنشر التركيبات والمصنوعات اليدوية بيانات حتمية بجانب الموقعة
  البيانات الوصفية.