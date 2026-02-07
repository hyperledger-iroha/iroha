---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/quickstart.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS فوری آغاز

عملية إنشاء SoraFS لطباعة الورق عبر الإنترنت
حتمية SF-1 جزيئات مضادة للفيروسات، وصناعة يدوية، ومتعددة الأشكال
جلب فلو پر لے جاتی ہے۔ يتم حفظ بعض الملاحظات وملف CLI
[بيان خط الأنابيب وصف وضاح](manifest-pipeline.md) يستخدم بشكل مستمر.

##ضروريات

- الصدأ ٹول چين (`rustup update`)، ومساحة العمل متاحة لكل نقرة.
- اختير: [OpenSSL يتوافق مع Ed25519 keypair](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  لقد صنعت صناعتي الفضائية.
- الأختيار: Node.js ≥ 18 إذا كنت تريد رؤية منفذ Docusaurus.

`export RUST_LOG=info` سيستمتع بالتجارب الممتعة لـ CLI في مسابقات سامن.

## 1. تركيبات حتمية

SF-1 هي ناقلات التقطيع الأساسية التي يتم تكرارها مرة أخرى. تم إنشاء J18NI00000014X
لقد قمت بإدارة مظاريف بيان موقعة من قبل النساء أيضًا؛ `--allow-unsigned` صرفت
يتم استخدام بعض من أهم العناصر.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

آؤٹپٹ:

-`fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
-`fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (أجر سيناء ہوا ہو)
-`fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. الحمولة الصافية للقطعة والخطة المميزة

`sorafs_chunker` هو أيضًا مقال أو مقال رائع:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

اسم الملف:

- `profile` / `break_mask` – `sorafs.sf1@1.0.0` يتم فحص العدادات.
- `chunks[]` – ترتیب إزاحات الحرب والأطوال وهضم BLAKE3 قطعة ۔

جدول المباريات، دعم الانحدار الانحداري، التدفق والدفعة
تقطيع الأجزاء:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```## 3. بيان الحمولة والبطاقة الائتمانية

خطة القطعة والأسماء المستعارة وتوقيعات الإدارة `sorafs-manifest-stub`
بيان واضح. لا يوجد أي شيء آخر يسمح بإدارة الحمولة النافعة ذات الملف الواحد؛ لعبة رائعة
يوجد مسار الدليل (CLI هو تصنيف معجمي لـ چلتا).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

`/tmp/docs.report.json` المميزات:

- `chunking.chunk_digest_sha3_256` – الإزاحات/الأطوال كما هو موضح في SHA3، وتركيبات القطع متوافقة.
- `manifest.manifest_blake3` – ملخص المغلف الفضائي عبر BLAKE3.
- `chunk_fetch_specs[]` – جلب المنسقين لموسيقى الحرب ہدايات۔

فيما يتعلق بالتواقيع الحقيقية التي تم إنشاؤها مؤخرًا، تتضمن وسيطات `--signing-key` و`--signer`
کریں۔ قم بإدارة المغلف للتوقيع على Ed25519 كبطاقة توثيق.

## 4. استرجاع متعدد الموفرين

يقوم المطور بإحضار خطة قطع CLI من أحد أو العديد من موفري الخدمة الذين يفشلون في إعادة التشغيل. أو سي آي
اختبارات الدخان والنماذج الأولية للمنسق هي الأفضل.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

التوثيقات:

- `payload_digest_hex` هو تقرير البيان.
- `provider_reports[]` موفر الخدمة يحسب النجاح/الفشل دکھاتا ہے.
- غير صفر `chunk_retry_total` الضغط الخلفي متجدد الهواء.
- `--max-peers=<n>` يقوم بتشغيل أحدث موفري الخدمات الذين يبلغ عددهم نطاقًا محدودًا وCI
  عمليات المحاكاة التي يتم تطويرها حسب المركز.
- `--retry-budget=<n>` عدد مرات إعادة المحاولة لكل قطعة (3) تجاوز عدد مرات الفشل في الحقن
  انحدارات الأوركسترا جلد ظاہر ہوں.`--expect-payload-digest=<hex>` و`--expect-payload-len=<bytes>` يتضمنان تقنية إعادة البناء
الحمولة إذا كان البيان سے ہٹے تفشل بشكل فوري ہو جئے.

## 5.اگلے اجراءات

- **تكامل الحوكمة** – ملخص البيان ومجلس `manifest_signatures.json`
  يساعد سير العمل أيضًا في تحديد مدى توفر السجل.
- **التفاوض بشأن التسجيل** – ملفات تعريف جديدة
  [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  دیکھیں۔ المعرفات الرقمية للمقابض الأساسية (`namespace.name@semver`)
  ترجيح ديني جہیے۔
- **أتمتة CI** – تتضمن هذه الخطوة الأولى التحكم في إصدار خطوط الأنابيب التي تتضمن إنشاء المستندات،
  التركيبات والمصنوعات الموقعة البيانات الوصفية کے ساتھ المظاهر الحتمية شاع کریں۔