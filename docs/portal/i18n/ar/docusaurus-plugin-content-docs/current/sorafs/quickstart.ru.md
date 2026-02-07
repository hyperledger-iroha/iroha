---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# البداية السريعة SoraFS

يتم تحقيق هذا الدليل العملي من خلال الملف التعريفي المحدد للشاحنة SF-1,
قم بإدراج البيانات واختيارات سريعة من عدد قليل من الموردين الذين يتم تسليمهم في الأساس
ناقل الحركة SoraFS. هذه هي الذات الكاملة
[بيان ناقل المعلومات الجيد](manifest-pipeline.md)
لسلامة التصميم والأخلاق تحت علم CLI.

## تريبوفانيا

- تولشين روست (`rustup update`)، نسخة من مساحة العمل محلية.
- اختياريًا: [للمفتاح Ed25519، المتوافق مع OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  لنشر البيانات.
- اختياريًا: Node.js أكبر من 18، إذا كنت تخطط مسبقًا لنشر البوابة Docusaurus.

قم بتثبيت `export RUST_LOG=info` أثناء التجربة للحصول على فائدة مفيدة
المشاركة CLI.

## 1. التعرف على تحديد التركيبات

تم تصميم المتجهات الكنسي من خلال تغيير SF-1. قم بتعبئة الأمر أيضًا
تحويلات البيان عند الإعلان عن `--signing-key`; استخدم `--allow-unsigned`
فقط في الصناعة المحلية.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

النتائج:

-`fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
-`fixtures/sorafs_chunker/manifest_blake3.json`
-`fixtures/sorafs_chunker/manifest_signatures.json` (إذا تم النشر)
-`fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. حدد الحمولة واستكمل الخطة

استخدم `sorafs_chunker` لمسح ملف الإنتاج أو الأرشيف:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

كلمات مفتاحية:

- `profile` / `break_mask` – التحقق من المعلمات `sorafs.sf1@1.0.0`.
- `chunks[]` - إعدادات وميزات وتفاصيل أفضل لـ BLAKE3.من أجل إنشاء مجموعة كبيرة من الصور، قم بإعادة التراجع إلى قاعدة Proptest لتتمكن من مشاهدتها،
ما هي المزامنة السريعة والحزم:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. بيان اليقظة والتضامن

تظهر خطة الالتزام والأسماء المستعارة وإدارة المشاركات مع العرض
`sorafs-manifest-stub`. قم بإظهار الأمر التالي لملف واحد للحمولة؛ اسبق الطريق
إلى الدلائل، لتغطيه العنوان (CLI تتعهد به في ليكسيكوغرافيتسكوم).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

تحقق من `/tmp/docs.report.json` على:

- `chunking.chunk_digest_sha3_256` – SHA3-ضبط الضبط/الخط، متصل بالتركيبات
  شكرا.
- `manifest.manifest_blake3` – BLAKE3-дайддест, подписанный в повите manifesta.
- `chunk_fetch_specs[]` – تعليمات مختارة للموسيقيين.

عندما تريد تقديم عرض حقيقي، قم بإضافة الحجج `--signing-key`
و `--signer`. قم بالتحقق من الأمر عندما تقوم بإدراج Ed25519 قبل كتابة التحويل.

## 4. قم بتزويدك بعدد قليل من مقدمي الخدمة

استخدم dev-CLI للاختيارات لتنزيل الخطة من خلال جزء واحد أو
عدد قليل من المقدمين. هذا مثالي لاختبار الدخان CI والنماذج الأولية
أوركيستراتورا.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

بروفيرك:- `payload_digest_hex` يتم الالتزام ببيان آخر.
- `provider_reports[]` يعرض كل النجاح/الجهاز في مقدم الخدمة.
- Ненулевой `chunk_retry_total` يدعم الضغط الخلفي.
- قم بالتقدم إلى `--max-peers=<n>` لتتمكن من تخطي موفر الشيسلو في النهاية و
  التركيز على محاكاة CI للمرشحين الأساسيين.
- `--retry-budget=<n>` تم إعادة تصميمه بشكل قياسي من خلال رأس السهم (3) ، لذلك
  يتم تشغيل أوركسترا الانحدار بشكل أفضل عند الحقن بشكل متكرر.

قم بإضافة `--expect-payload-digest=<hex>` و`--expect-payload-len=<bytes>` لتكون جاهزًا
تم الانتهاء من ذلك باستخدام oshibkoy عندما يتم إلغاء حجب الحمولة النافعة من البيان.

## 5. أحدث الصيحات

- **التكامل مع الإدارة** – قم بمتابعة بيان الصفحة و
  `manifest_signatures.json` في العملية التي يمكنك من خلالها الالتزام بـ Pin Registry
  مضمون.
- **المراسلات مع السجل** – تواصل مع [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  قبل التسجيل في الملف الشخصي الجديد. الأتمتة تتطلب جهدًا كبيرًا للكتابة الكنسية
  المعرف (`namespace.name@semver`) معرف جيد.
- **أتمتة CI** – قم بإضافة الأوامر التالية إلى الإصدار العادي للتوثيق،
  يتم نشر الصور والمصنوعات اليدوية من خلال بيانات تحديد المعالم
  ترجمة.