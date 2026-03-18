---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/developer-cli.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ڈویلپر-سی ایل آئی
عنوان: CLI ترکیبیں SoraFS
سائڈبار_لیبل: سی ایل آئی ترکیبیں
تفصیل: مستحکم سطح `sorafs_cli` کا ٹاسک پر مبنی راستہ۔
---

::: نوٹ کینونیکل ماخذ
:::

مستحکم سطح `sorafs_cli` (کریٹ `sorafs_car` کے ذریعہ فراہم کردہ خصوصیت `cli` فعال ہے) میں نمونے SoraFS تیار کرنے کے لئے ضروری ہر قدم کی خاکہ پیش کیا گیا ہے۔ عام ورک فلوز میں سیدھے کودنے کے لئے اس کک بوک کا استعمال کریں۔ آپریشنل سیاق و سباق کے ل it اس کو مینی فیسٹ پائپ لائن اور آرکسٹریٹر رن بوکس کے ساتھ منسلک کریں۔

## پیکیج پے لوڈ

عین مطابق کار آرکائیوز اور حصہ منصوبوں کو تیار کرنے کے لئے `car pack` استعمال کریں۔ کمانڈ خود بخود SF-1 چنکر کا انتخاب کرتا ہے جب تک کہ ہینڈل فراہم نہ کیا جائے۔

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- پہلے سے طے شدہ چنکر ہینڈل: `sorafs.sf1@1.0.0`۔
- ڈائریکٹری اندراجات کو لغت کے ترتیب میں تلاش کیا جاتا ہے تاکہ پلیٹ فارم کے مابین چیکسی مستحکم رہیں۔
- JSON کا خلاصہ رجسٹری اور آرکسٹریٹر کے ذریعہ پہچانے جانے والے پے لوڈ ڈائجسٹس ، فی چنک میٹا ڈیٹا اور روٹ سی آئی ڈی شامل ہیں۔

## منشور تعمیر کریں

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- اختیارات `--pin-*` کا نقشہ براہ راست `PinPolicy` `sorafs_manifest::ManifestBuilder` میں۔
- جب آپ چاہتے ہیں کہ CLI جمع کرنے سے پہلے CLI کے SHA3 ڈائجسٹ کو دوبارہ گنتی کرے تو `--chunk-plan` فراہم کریں۔ بصورت دیگر یہ خلاصہ میں مربوط ہضم کو دوبارہ استعمال کرتا ہے۔
- JSON آؤٹ پٹ جائزوں کے دوران سادہ فرق کے ل pay پے لوڈ Norito کی عکاسی کرتا ہے۔

## طویل مدتی چابیاں کے بغیر ظاہر ہوتا ہے

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- ان لائن ٹوکن ، ماحولیاتی متغیرات یا فائل پر مبنی ذرائع کو قبول کرتا ہے۔
- خام JWT کو برقرار رکھے بغیر ، جب تک `--include-token=true` کو برقرار رکھے بغیر پروویژن میٹا ڈیٹا (`token_source` ، `token_hash_hex` ، CUNK DISCT) شامل کریں۔
- CI میں اچھی طرح سے کام کرتا ہے: `--identity-token-provider=github-actions` ترتیب دے کر Github ایکشن سے OIDC کے ساتھ یکجا کریں۔

## Torii پر ظاہر جمع کروائیں

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority i105... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Norito کو عرفی ثبوتوں اور چیکوں کی ضابطہ کشائی انجام دیتا ہے جو وہ پوسٹ سے پہلے Torii پر مینی فیسٹ ڈائجسٹ سے ملتے ہیں۔
- مماثل حملوں سے بچنے کے لئے منصوبے سے حصہ SHA3 ڈائجسٹ کو دوبارہ گنتی کرتا ہے۔
- جوابی خلاصہ بعد میں آڈیٹنگ کے لئے رجسٹری سے HTTP کی حیثیت ، ہیڈر اور پے لوڈ پر قبضہ کرتا ہے۔

## کار کا مواد اور ثبوت چیک کریں

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- پور کے درخت کی تشکیل نو اور پے لوڈ ہضموں کو مینی فیسٹ سمری کے ساتھ موازنہ کیا۔
- گورننس میں نقل کے ثبوت پیش کرتے وقت مطلوبہ گنتی اور شناخت کنندگان کو اپنی گرفت میں لے لیتا ہے۔

## براڈکاسٹ پروف ٹیلی میٹری

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- ہر اسٹریمڈ پروف کے لئے NDJSON عناصر کا اخراج (`--emit-events=false` کے ساتھ ری پلے کو غیر فعال کریں)۔
- مجموعی طور پر کامیابی/ناکامی کی گنتی ، لیٹینسی ہسٹگرامس ، اور JSON سمری میں ناکامیوں کو مجموعی طور پر تاکہ ڈیش بورڈز لاگ ان کی جانچ پڑتال کیے بغیر نتائج کو پلاٹ کرسکیں۔
- غیر صفر کوڈ کے ساتھ باہر نکلتا ہے جب گیٹ وے کی ناکامیوں یا مقامی پور کی توثیق (`--por-root-hex` کے ذریعے) ثبوتوں کو مسترد کرتا ہے۔ دہرائیں رنز کے لئے `--max-failures` اور `--max-verification-failures` کے ساتھ تھریشولڈز کو ایڈجسٹ کریں۔
- آج پور کی حمایت کریں ؛ PDP اور POTR ایک بار SF-13/SF-14 کی جگہ پر ایک ہی لفافے کو دوبارہ استعمال کریں۔
- `--governance-evidence-dir` مہیا کردہ سمری ، میٹا ڈیٹا (ٹائم اسٹیمپ ، سی ایل آئی ورژن ، گیٹ وے یو آر ایل ، منشور ڈائجسٹ) اور فراہم کردہ ڈائرکٹری میں ظاہر ہونے والی ایک کاپی لکھتا ہے تاکہ گورننس پیکجز پر عملدرآمد کو دوبارہ چلائے بغیر پروف اسٹریم آرکائیو کرسکیں۔

## اضافی حوالہ جات

- `docs/source/sorafs_cli.md` - جھنڈوں کی مکمل دستاویزات۔
- `docs/source/sorafs_proof_streaming.md` - پروف ٹیلی میٹری ڈایاگرام اور ڈیش بورڈ ٹیمپلیٹ Grafana۔
- `docs/source/sorafs/manifest_pipeline.md` - chunking ، ظاہر مرکب اور کار کے انتظام میں گہرا غوطہ۔