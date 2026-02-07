---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/quickstart.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# البداية السريعة لـ SoraFS

هذا هو الدليل العملي لإعادة تحديد ملف القطع SF-1،
شركة البيانات وتدفق الاسترداد متعدد المزودين الذي يدعم
خط أنابيب التخزين SoraFS. أكمل مع
[تحليل عميق لخط أنابيب البيانات](manifest-pipeline.md)
لملاحظات التصميم والإشارة إلى علامات CLI.

## المتطلبات السابقة

- Toolchain de Rust (`rustup update`)، نسخة محلية من مساحة العمل.
- اختياري: [من خلال المفاتيح Ed25519 التي تم إنشاؤها مع OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  للبيانات الثابتة.
- اختياري: Node.js ≥ 18 إذا قمت بمعاينة بوابة Docusaurus.

حدد `export RUST_LOG=info` أثناء التجارب لعرض رسائل CLI المساعدة.

## 1. تحديث تحديدات التركيبات

قم بتجديد ناقلات القطع الأساسية SF-1. يصدر الأمر أيضًا
أسماء الشركات المؤكدة عندما تقترح `--signing-key`؛ الولايات المتحدة الأمريكية
`--allow-unsigned` فقط أثناء التطوير المحلي.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

ساليداس:

-`fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
-`fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (إذا كان ثابتًا)
-`fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. تجزئة الحمولة وفحص الخطة

استخدام `sorafs_chunker` لتجزئة ملف أو ملف مضغوط عشوائي:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

عصا كامبوس:

- `profile` / `break_mask` – تأكيد معلمات `sorafs.sf1@1.0.0`.
- `chunks[]` - إزاحة الخطوط وخطوط الطول وهضم BLAKE3 للقطع.بالنسبة للتركيبات الأكبر حجمًا، قم بتنفيذ التراجع المناسب من أجل ذلك
نضمن أن عملية التقطيع والبث ستتم مزامنتها كثيرًا:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. قم ببناء بيان ثابت

قم بتضمين خطة القطع والأسماء المستعارة وشركات الإدارة في بيان مستخدم
`sorafs-manifest-stub`. يعرض أمر التفريغ حمولة من ملف واحد فقط؛ باسا
مسار الدليل لتجميع شجرة (CLI يقوم بالتسجيل في ترتيب المعجم).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

مراجعة `/tmp/docs.report.json` للفقرة:

- `chunking.chunk_digest_sha3_256` – ملخص SHA3 للإزاحات/خطوط الطول، المتزامنة مع لوس
  تركيبات تشانكر.
- `manifest.manifest_blake3` - ملخص BLAKE3 الثابت حول البيان.
- `chunk_fetch_specs[]` – تعليمات الاسترداد المخصصة للمصممين.

عندما يتم إعداد هذه القائمة لطرح الشركات الحقيقية، بالإضافة إلى الحجج `--signing-key` و
`--signer`. الأمر الذي تم التحقق منه كل شركة Ed25519 قبل كتابة الموضوع.

## 4. محاكاة الاسترداد متعدد المعالجات

استخدام CLI لجلب التصميم لإعادة إنتاج خطة القطع مقابل واحدة أو أكثر
الموردون. إنه مثالي لاختبارات الدخان في CI والنماذج الأولية للسائق.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

التوافقات:- يجب أن يتوافق `payload_digest_hex` مع معلومات البيان.
- `provider_reports[]` يرسل معلومات ناجحة/تسقط من قبل المورّد.
- Un `chunk_retry_total` منفصل لضبط الضغط الخلفي.
- Pasa `--max-peers=<n>` للحد من عدد الموردين المبرمجين للتنفيذ
  والحفاظ على محاكاة CI معززة للمرشحين الرئيسيين.
- `--retry-budget=<n>` يقوم بكتابة التسجيل من خلال خلل في إعادة الإرسال من خلال القطعة (3) للفقرة
  اكتشف أسرع تراجعات الأوركيستادور بعد سقوطه.

قم بإضافة `--expect-payload-digest=<hex>` و`--expect-payload-len=<bytes>` للسقوط السريع
عندما يتم إعادة بناء الشحنة يتم عرض البيان.

## 5. الخطوة التالية

- **تكامل الإدارة** – مشاهدة ملخص البيان
  `manifest_signatures.json` وتدفق النصائح حتى يتمكن Pin Registry
  الإعلان عن التوفر.
- **مفاوضات التسجيل** – استشارة [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  قبل تسجيل الملفات الشخصية الجديدة. يجب أن تفضل الأتمتة المعالجين القانونيين
  (`namespace.name@semver`) للمعرفات الرقمية.
- **أتمتة CI** – إضافة الأوامر السابقة إلى خطوط الإصدار الخاصة بها
  تظهر الوثائق والتركيبات والمصنوعات اليدوية تحديدات واضحة جنبًا إلى جنب مع
  البيانات التعريفية الثابتة.