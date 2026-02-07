---
lang: ur
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 79a048e6061f7054e14a471004cf7da0dddd3f9bf627d9f1d20ff63803cb0979
source_last_modified: "2026-01-04T17:06:14.405886+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS کوئیک اسٹارٹ

یہ گائیڈ ڈٹرمینسٹک SF-1 چنکر پروفائل کے ذریعے چلتا ہے ،
منشور پر دستخط ، اور ملٹی فراہم کرنے والے بہاؤ جو SoraFS کو کم کرتے ہیں
اسٹوریج پائپ لائن۔ [منشور پائپ لائن گہری غوطہ خور] (manifest-pipeline.md) کے ساتھ جوڑا
ڈیزائن نوٹ اور سی ایل آئی پرچم حوالہ مواد کے ل .۔

## شرائط

- مورچا ٹولچین (`rustup update`) ، ورک اسپیس مقامی طور پر کلون کیا گیا۔
- اختیاری: [اوپن ایس ایل سے تیار کردہ ED25519 کیپیئر] (https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  منشور پر دستخط کرنے کے لئے۔
- اختیاری: اگر آپ Docusaurus پورٹل کا پیش نظارہ کرنے کا ارادہ رکھتے ہیں تو نوڈ ڈاٹ جے ایس ≥ 18۔

مددگار CLI پیغامات کی سطح پر تجربہ کرتے وقت `export RUST_LOG=info` سیٹ کریں۔

## 1. تعصب پسند فکسچر کو تازہ کریں

کیننیکل SF-1 chunking ویکٹرز کو دوبارہ تخلیق کریں۔ کمانڈ پر دستخط بھی خارج ہوتا ہے
جب `--signing-key` فراہم کیا جاتا ہے تو منشور لفافے ؛ `--allow-unsigned` استعمال کریں
صرف مقامی ترقی کے دوران۔

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

نتائج:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (اگر دستخط شدہ)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2۔ ایک پے لوڈ کو چنک کریں اور منصوبے کا معائنہ کریں

کسی صوابدیدی فائل یا محفوظ شدہ دستاویزات کے لئے `sorafs_chunker` استعمال کریں:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

کلیدی فیلڈز:

- `profile` / `break_mask` - `sorafs.sf1@1.0.0` پیرامیٹرز کی تصدیق کرتا ہے۔
- `chunks[]` - آرڈر شدہ آفسیٹس ، لمبائی اور بلیک بلیک 3 ڈائجسٹس۔

بڑے فکسچر کے ل ، ، اسٹریمنگ کو یقینی بنانے کے لئے پریپٹیسٹ بیک بیک ریگریشن چلائیں اور
بیچ چنکنگ میں مطابقت پذیری میں قیام:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. ایک منشور کی تعمیر اور دستخط کریں

حصہ ، عرفیت ، اور گورننس کے دستخطوں کو استعمال کرتے ہوئے ایک مینی فیسٹ میں لپیٹیں
`sorafs-manifest-stub`۔ نیچے دیئے گئے کمانڈ میں ایک فائل پے لوڈ کی نمائش کی گئی ہے۔ پاس
ایک درخت کو پیکیج کرنے کا ایک ڈائرکٹری راستہ (سی ایل آئی اس کو لغت کے لحاظ سے چلاتا ہے)۔

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

`/tmp/docs.report.json` کا جائزہ لیں:

- `chunking.chunk_digest_sha3_256` - SHA3 آفسیٹس/لمبائی کا ڈائجسٹ ، میچ کرتا ہے
  چنکر فکسچر۔
- `manifest.manifest_blake3` - بلیک 3 ڈائجسٹ نے مینی فیسٹ لفافے میں دستخط کیے۔
- `chunk_fetch_specs[]` - آرکیسٹریٹرز کے لئے بازیافت ہدایات کا آرڈر دیا گیا۔

جب حقیقی دستخطوں کی فراہمی کے لئے تیار ہوں تو ، `--signing-key` اور `--signer` شامل کریں
دلائل کمانڈ لکھنے سے پہلے ہر ED25519 دستخط کی تصدیق کرتا ہے
لفافہ

## 4. ملٹی فراہم کرنے والے بازیافت کو نقل کریں

ایک یا زیادہ کے خلاف حصہ پلان کو دوبارہ چلانے کے لئے ڈویلپر بازیافت سی ایل آئی کا استعمال کریں
فراہم کرنے والے یہ CI دھواں ٹیسٹ اور آرکسٹریٹر پروٹو ٹائپنگ کے لئے مثالی ہے۔

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

دعوے:

- `payload_digest_hex` کو مینی فیسٹ رپورٹ سے ملنا چاہئے۔
- `provider_reports[]` کامیابی/ناکامی کی گنتی فی فراہم کنندہ کی سطح پر ہے۔
-نان صفر `chunk_retry_total` بیک پریشر ایڈجسٹمنٹ کو نمایاں کرتا ہے۔
- رن کے لئے شیڈول فراہم کرنے والوں کی تعداد کو محدود کرنے کے لئے `--max-peers=<n>` پاس کریں
  اور بنیادی امیدواروں پر مرکوز سی آئی کے نقوش کو مرکوز رکھیں۔
- `--retry-budget=<n>` فی سے طے شدہ فی چنٹ ریٹری گنتی (3) کو اوور رائڈ کرتی ہے
  جب ناکامیوں کو انجیکشن لگاتے ہو تو آرکیسٹریٹر ریگریشنز کو تیزی سے سطح پر لے جاسکتے ہیں۔

ناکام ہونے کے لئے `--expect-payload-digest=<hex>` اور `--expect-payload-len=<bytes>` شامل کریں
تیز رفتار جب تعمیر نو پے لوڈ منشور سے انحراف کرتا ہے۔

## 5. اگلے اقدامات- ** گورننس انضمام ** - پائپ منشور ڈائجسٹ اور
  `manifest_signatures.json` کونسل کے ورک فلو میں تاکہ پن رجسٹری کر سکے
  دستیابی کی تشہیر کریں۔
- ** رجسٹری مذاکرات ** - مشورہ کریں [`sorafs/chunker_registry.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  نئے پروفائلز رجسٹر کرنے سے پہلے۔ آٹومیشن کو کیننیکل ہینڈلز کو ترجیح دینی چاہئے
  (`namespace.name@semver`) عددی IDs سے زیادہ۔
- ** سی آئی آٹومیشن ** - پائپ لائنوں کو جاری کرنے کے لئے مذکورہ بالا احکامات شامل کریں تو دستاویزات ،
  فکسچر ، اور نمونے دستخط شدہ کے ساتھ ساتھ عین مطابق ظاہر ہوتے ہیں
  میٹا ڈیٹا۔