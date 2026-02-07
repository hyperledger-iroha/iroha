---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# فوری آغاز SoraFS

یہ ہینڈ آن گائیڈ SF-1 چنکر کے تعصب پسند پروفائل سے گزرتا ہے ،
متعدد فراہم کنندگان سے ظاہر اور نمونے لینے کے بہاؤ کے دستخط جو زیر اثر ہیں
اسٹوریج پائپ لائن SoraFS۔ اسے مکمل کریں
[منشور پائپ لائن کا تفصیلی تجزیہ] (manifest-pipeline.md)
ڈیزائن نوٹوں اور CLI جھنڈوں کی مدد کے لئے۔

## تقاضے

- مورچا ٹولچین (`rustup update`) ، ورک اسپیس مقامی طور پر کلون کیا گیا۔
- اختیاری: [اوپن ایس ایس ایل مطابقت پذیر ED25519 کلیدی جوڑی] (https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  منشور پر دستخط کرنے کے لئے۔
- اختیاری: اگر آپ پورٹل Docusaurus کا پیش نظارہ کرنے کا ارادہ رکھتے ہیں تو نوڈ ڈاٹ جے ایس ≥ 18۔

مفید وصول کرنے کے لئے تجربات کے دوران `export RUST_LOG=info` انسٹال کریں
سی ایل آئی پیغامات۔

## 1. تازہ ترین فکسچر کو اپ ڈیٹ کریں

کیننیکل SF-1 chunking ویکٹر تیار کریں۔ ٹیم نے بھی دستخط کیے
`--signing-key` کی وضاحت کرتے وقت ظاہر لفافے ؛ `--allow-unsigned` استعمال کریں
صرف مقامی ترقی میں۔

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

نتائج:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (اگر دستخط شدہ)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2۔ پے لوڈ کو تقسیم کریں اور اس منصوبے کا مطالعہ کریں

صوابدیدی فائل یا محفوظ شدہ دستاویزات کو تقسیم کرنے کے لئے `sorafs_chunker` استعمال کریں:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

کلیدی فیلڈز:

- `profile` / `break_mask` - `sorafs.sf1@1.0.0` کے پیرامیٹرز کی تصدیق کرتا ہے۔
- `chunks[]` - بلیک 3 ٹکڑوں کی آفسیٹس ، لمبائی اور ہضم کرنے کا حکم دیا گیا۔

بڑے فکسچر کے ل pre ، یقینی بنانے کے لئے پریپٹسٹ ریگریشن چلائیں
وہ ندی اور بیچ چنکنگ ہم آہنگ رہتا ہے:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. منشور کو جمع کریں اور اس پر دستخط کریں

حصہ منصوبہ ، عرفی ، اور دستخطوں کو ایک مینی فیسٹ میں جمع کریں
`sorafs-manifest-stub`۔ نیچے دیئے گئے کمانڈ میں ایک فائل کا پے بوجھ دکھایا گیا ہے۔ راستہ پاس کریں
درخت کو پیک کرنے کے لئے ایک ڈائریکٹری میں (سی ایل آئی اسے لغت کے مطابق ترتیب سے عبور کرتا ہے)۔

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

`/tmp/docs.report.json` کے لئے چیک کریں:

- `chunking.chunk_digest_sha3_256` - SHA3 آف آفسیٹس/لمبائی کا ڈائجسٹ ، فکسچر سے میل کھاتا ہے
  چنکر
- `manifest.manifest_blake3` - بلیک 3 ڈائجسٹ ، منشور لفافے میں دستخط شدہ۔
- `chunk_fetch_specs[]` - آرکیسٹریٹرز کے لئے بازیافت ہدایات کا آرڈر دیا گیا۔

جب آپ اصل دستخط فراہم کرنے کے لئے تیار ہوں تو ، دلائل `--signing-key` شامل کریں
اور `--signer`۔ لفافہ لکھنے سے پہلے ٹیم ہر ED25519 کے دستخط کی تصدیق کرتی ہے۔

## 4. متعدد فراہم کنندگان سے وصول کرنے کی نقالی

ایک یا کے ذریعے حصہ کے منصوبے کو کھیلنے کے لئے لانے کے لئے دیو-سی ایل آئی کا استعمال کریں
کئی فراہم کنندگان۔ یہ CI دھواں ٹیسٹ اور پروٹو ٹائپنگ کے لئے مثالی ہے
آرکیسٹریٹر۔

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

چیک:

- `payload_digest_hex` کو مینی فیسٹ رپورٹ سے ملنا چاہئے۔
- `provider_reports[]` ہر فراہم کنندہ کے لئے کامیابیوں/غلطیوں کی تعداد ظاہر کرتا ہے۔
-نان صفر `chunk_retry_total` بیک دباؤ کی ترتیبات کو نمایاں کرتا ہے۔
- اسٹارٹ اپ میں فراہم کرنے والوں کی تعداد کو محدود کرنے کے لئے `--max-peers=<n>` پاس کریں اور
  اعلی امیدواروں پر CI تخروپن فوکس کریں۔
- `--retry-budget=<n>` فی حصہ (3) تک دہرانے کی پہلے سے طے شدہ تعداد کو اوور رائڈس
  جب غلطیوں کو انجیکشن لگاتے ہو تو آرکسٹریٹر ریگریشنز کی تیزی سے شناخت کریں۔`--expect-payload-digest=<hex>` اور `--expect-payload-len=<bytes>` کو جلدی سے شامل کریں
جب بحال شدہ پے لوڈ منشور سے ہٹ جاتا ہے تو ناکام ہوجاتے ہیں۔

## 5. اگلے اقدامات

- ** مینجمنٹ انضمام ** - مینی فیسٹ ڈائجسٹ کو پاس کریں اور
  کونسل کے عمل میں `manifest_signatures.json` تاکہ پن رجسٹری اعلان کرسکے
  دستیابی
- ** رجسٹری کے ساتھ مذاکرات ** - دیکھیں [`sorafs/chunker_registry.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  نئے پروفائلز رجسٹر کرنے سے پہلے۔ آٹومیشن کو کیننیکل کو ترجیح دینی چاہئے
  عددی ID کے ساتھ ہینڈلز (`namespace.name@semver`)۔
- ** سی آئی آٹومیشن ** - ریلیز پائپ لائنوں میں مذکورہ بالا احکامات شامل کریں تاکہ دستاویزات ،
  فکسچر اور نمونے نے دستخط شدہ کے ساتھ ساتھ اختیاری منشور کو شائع کیا
  میٹا ڈیٹا۔