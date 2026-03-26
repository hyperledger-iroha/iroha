---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/developer-cli.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ڈویلپر-سی ایل آئی
عنوان: SoraFS کے لئے CLI نسخہ
سائڈبار_لیبل: سی ایل آئی ترکیبیں
تفصیل: `sorafs_cli` یونیفائیڈ سطح کی ٹاسک پر مبنی وضاحت۔
---

::: منظور شدہ ماخذ کو نوٹ کریں
یہ صفحہ `docs/source/sorafs/developer/cli.md` کی عکاسی کرتا ہے۔ اس بات کو یقینی بنائیں کہ اس وقت تک دونوں کاپیاں ہم آہنگی میں رکھیں جب تک کہ پرانا اسفنکس کلسٹر ریٹائر نہ ہوجائے۔
:::

یونیفائیڈ `sorafs_cli` ڈیک (`cli` کے ساتھ کریٹ `sorafs_car` سے فراہم کردہ) SoraFS نمونے ترتیب دینے کے لئے درکار ہر قدم دکھاتا ہے۔ عام کام کے بہاؤ میں کودنے کے لئے اس کتابچے کا استعمال کریں۔ آپریشنل سیاق و سباق حاصل کرنے کے ل it اس کو مینی فیسٹ پائپ لائن اور آرکسٹریٹر پلے بوکس کے ساتھ جوڑیں۔

## بوجھ کی پیکیجنگ

عین مطابق کار آرکائیوز اور ٹکڑوں کو تیار کرنے کے لئے `car pack` کا استعمال کریں۔ کمانڈ خود بخود چنکر SF-1 کا انتخاب کرتا ہے جب تک کہ کوئی ہینڈل فراہم نہ کیا جائے۔

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- پہلے سے طے شدہ چنکر ہینڈل: `sorafs.sf1@1.0.0`۔
- ڈائریکٹری اندراجات لغت کے ترتیب میں دکھائے جاتے ہیں تاکہ پلیٹ فارمز میں چیکسم مستقل رہیں۔
- JSON ڈائجسٹس میں پے لوڈ ، ہر ایک حصہ کے لئے میٹا ڈیٹا ، اور لاگ اور فارمیٹر کے ذریعہ روٹ سی آئی ڈی کو تسلیم کیا گیا ہے۔

## منشور کی تعمیر

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

-`--pin-*` اختیارات `sorafs_manifest::ManifestBuilder` کے اندر براہ راست `PinPolicy` فیلڈز سے لنک کریں۔
- جب آپ چاہتے ہو کہ CLI بھیجنے سے پہلے CLI ڈائجسٹ SHA3 کو دوبارہ گنتی کرنا چاہتا ہے تو `--chunk-plan` کو بچائیں۔ بصورت دیگر یہ ڈائجسٹ میں شامل ڈائجسٹ کو دوبارہ استعمال کرتا ہے۔
- JSON آؤٹ پٹ جائزوں کے دوران فرق کو آسان بنانے کے لئے Norito پے لوڈ کی عکاسی کرتا ہے۔

## طویل مدتی چابیاں کے بغیر منشور پر دستخط کرنا

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- ان لائن علامتوں ، ماحولیاتی متغیرات ، یا فائل پر مبنی ذرائع کو قبول کرتا ہے۔
- خام JWT کو بچانے کے بغیر اصل ڈیٹا (`token_source` ، `token_hash_hex` ، حص j ہ کو ہضم کریں) شامل کرتا ہے۔
- CI میں اچھی طرح سے کام کرتا ہے: `--identity-token-provider=github-actions` کو ترتیب دینے کے ذریعے گٹ ہب ایکشن میں OIDC کے ساتھ ضم کریں۔

## Torii پر مینی فیسٹ بھیجیں

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority <katakana-i105-account-id> \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Norito عرف ڈائریکٹریوں کی ضابطہ کشائی کو انجام دیتا ہے اور Torii پر پوسٹ سے پہلے مینی فیسٹ ڈائجسٹ کے ساتھ ان کے میچ کی جانچ پڑتال کرتا ہے۔
- مماثل حملوں کو روکنے کے لئے منصوبہ بندی کے حصے کے لئے ڈائجسٹ SHA3 کو دوبارہ گنتی کرتا ہے۔
- جوابی خلاصہ بعد میں آڈٹ کے ل H HTTP کی حیثیت ، ہیڈر ، اور لاگ پے لوڈ پر قبضہ کرتا ہے۔

## کار کا مواد اور ثبوت چیک کریں

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- پور کے درخت کی تشکیل نو اور پے لوڈ ہضموں کا موازنہ مینی فیسٹ سمری سے کرتا ہے۔
- گورننس میں نقل کے ثبوت بھیجتے وقت مطلوبہ کاؤنٹرز اور آئی ڈی کو اپنی گرفت میں لے لیتا ہے۔

## ثبوت کی ٹیلی میٹرک نشریات

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```

- ہر اسٹریمڈ ڈائرکٹری کے لئے NDJSON عناصر کا اخراج (ری پلے `--emit-events=false` کے ذریعے غیر فعال کیا جاسکتا ہے)۔
- گروپ پاس/فیل کاؤنٹرز ، لیٹینسی ہسٹگرامس ، اور نمونے کی ناکامیوں کو JSON سمری میں منتقل کریں تاکہ ڈیش بورڈز بغیر چھیلنے کے نتائج پلاٹ کرسکیں۔
- جب گیٹ وے کی ناکامی کی اطلاع دی جاتی ہے یا جب مقامی پور کی توثیق کے عمل (`--por-root-hex` کے ذریعے) شواہد کو مسترد کرتا ہے تو غیر صفر سے باہر نکلنے کے ساتھ ختم ہوجاتا ہے۔ تربیت کے تجربات کے لئے `--max-failures` اور `--max-verification-failures` کے ذریعے دہلیز کو ایڈجسٹ کریں۔
- POR فی الحال تائید ؛ جب SF-13/SF-14 آجائے تو PDP اور POTR ایک ہی سانچے کو دوبارہ استعمال کریں۔
-`--governance-evidence-dir` فارمیٹڈ سمری ، میٹا ڈیٹا (ٹائم اسٹیمپ ، سی ایل آئی ورژن ، پورٹل یو آر ایل ، ڈائجسٹ مینی فیسٹ) لکھتا ہے اور مخصوص ڈائرکٹری میں ظاہر ہوتا ہے تاکہ گورننس پیکجز پھانسی کو دوبارہ شروع کیے بغیر شواہد کے بہاؤ کی ڈائرکٹری کو محفوظ کرسکیں۔

## اضافی حوالہ جات- `docs/source/sorafs_cli.md` - جھنڈوں کی جامع دستاویزات۔
- `docs/source/sorafs_proof_streaming.md` - گائیڈ ٹیلی میٹری ڈایاگرام اور پلیٹ ٹیمپلیٹ Grafana۔
- `docs/source/sorafs/manifest_pipeline.md` - chunking ، ظاہر تعمیر ، اور کار پروسیسنگ میں گہری ڈوبکی۔