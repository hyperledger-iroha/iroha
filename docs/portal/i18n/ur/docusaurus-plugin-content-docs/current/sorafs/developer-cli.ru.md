---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/developer-cli.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ڈویلپر-سی ایل آئی
عنوان: نسخہ کتاب CLI SoraFS
سائڈبار_لیبل: سی ایل آئی ہدایت کی کتاب
تفصیل: مستحکم سطح `sorafs_cli` کے لئے کاموں کا عملی تجزیہ۔
---

::: نوٹ کینونیکل ماخذ
:::

`sorafs_cli` کنسولیڈیٹیڈ سطح (کریٹ `sorafs_car` کے ذریعہ فراہم کردہ `cli` فعال ہے) SoraFS نمونے تیار کرنے کے لئے درکار ہر قدم کا احاطہ کرتا ہے۔ سیدھے عام ورک فلوز تک جانے کے لئے اس کک بوک کا استعمال کریں۔ آپریشنل سیاق و سباق کے ل it اس کو مینی فیسٹ پائپ لائن اور آرکسٹریٹر رن بوکس کے ساتھ جوڑیں۔

## پیکیجنگ پے لوڈز

ڈٹرمینسٹک کار آرکائیوز اور حصہ منصوبوں کو حاصل کرنے کے لئے `car pack` کا استعمال کریں۔ کمانڈ خود بخود چنکر SF-1 کا انتخاب کرتا ہے اگر ہینڈل کی وضاحت نہیں کی گئی ہے۔

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- معیاری ہینڈل چنکر: `sorafs.sf1@1.0.0`۔
- ان پٹ ڈائریکٹریوں کو لغت کے مطابق ترتیب دیا جاتا ہے تاکہ پلیٹ فارمز میں چیکسمس کو مستحکم رکھا جاسکے۔
- JSON کا خلاصہ رجسٹری اور آرکسٹریٹر کے ذریعہ پہچانے جانے والے پے لوڈ ڈائجسٹس ، کنٹ میٹا ڈیٹا اور روٹ سی آئی ڈی شامل ہیں۔

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

- `--pin-*` اختیارات Norito میں `PinPolicy` فیلڈز کا براہ راست نقشہ۔
- اگر آپ چاہتے ہیں کہ CLI بھیجنے سے پہلے CLI کو SHA3 ڈائجسٹ کو دوبارہ گنتی کرنا چاہتا ہے تو `--chunk-plan` کی وضاحت کریں۔ بصورت دیگر یہ خلاصہ سے ڈائجسٹ کا استعمال کرتا ہے۔
- JSON آؤٹ پٹ جائزہ کے دوران آسان فرق کے لئے Norito پے لوڈ کی عکاسی کرتا ہے۔

## طویل المیعاد چابیاں کے بغیر ظاہر ہونے پر دستخط کرنا

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- ان لائن ٹوکن ، ماحولیاتی متغیرات یا فائل کے ذرائع کو قبول کرتا ہے۔
- اگر Torii کی وضاحت نہیں کی گئی ہے ، بغیر کسی کو بچائے بغیر ، پرووننس میٹا ڈیٹا (`token_source` ، `token_hash_hex` ، ہضم حصہ) شامل کرتا ہے۔
- CI دوستانہ: `--identity-token-provider=github-actions` ترتیب دے کر گٹ ہب ایکشن OIDC کے ساتھ ضم کریں۔

## Torii پر ظاہر کرنا

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority <i105-account-id> \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Norito عرفی ثبوتوں کو ضابطہ کشائی کرنے اور چیک کرتا ہے کہ Torii پر پوسٹ سے پہلے مینی فیسٹ ڈائجسٹ میچ کرتا ہے۔
- مماثل حملوں کو روکنے کے لئے منصوبے سے SHA3 ڈائجسٹ حصہ کو دوبارہ گنتی کرتا ہے۔
- جوابی خلاصہ بعد میں آڈٹ کے ل the HTTP کی حیثیت ، ہیڈر اور رجسٹری کے پے لوڈ کو حاصل کرتا ہے۔

## کاروں اور ثبوتوں کے مندرجات کی جانچ پڑتال

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- پور کے درخت کو دوبارہ تعمیر کرتا ہے اور پے لوڈ ہضموں کا مینی فیسٹ سمری کے ساتھ موازنہ کرتا ہے۔
- گورننس میں نقل کے ثبوت بھیجتے وقت مقدار اور شناخت کاروں کو طے کرتا ہے۔

## ٹیلی میٹری کے ثبوتوں کو اسٹریم کرنا

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- منظور شدہ ہر پروف کے لئے NDJSON عناصر تیار کرتا ہے (`--emit-events=false` کے ذریعے ری پلے کو غیر فعال کریں)۔
- کامیابی/غلطی کی گنتی ، لیٹینسی ہسٹگرامس اور انتخابی ناکامیوں کو خلاصہ JSON میں جمع کرتا ہے تاکہ ڈیش بورڈز بغیر پڑھے بغیر گراف تیار کرسکیں۔
- غیر صفر کوڈ کے ساتھ ناکام ہوجاتا ہے جب گیٹ وے غلطیوں یا مقامی پور چیک (`--por-root-hex` کے ذریعے) ثبوتوں کو مسترد کرتا ہے۔ ریہرسلوں کے لئے `--max-failures` اور `--max-verification-failures` کے ذریعے دہلیز طے کریں۔
- آج پور کی حمایت کی گئی ہے۔ SF-13/SF-14 کی رہائی کے بعد PDP اور POTR اسی لفافے کو دوبارہ استعمال کریں۔
-`--governance-evidence-dir` پیش کردہ خلاصہ ، میٹا ڈیٹا (ٹائم اسٹیمپ ، سی ایل آئی ورژن ، گیٹ وے یو آر ایل ، منشور ڈائجسٹ) اور مخصوص ڈائرکٹری میں ظاہر ہونے والی ایک کاپی لکھتا ہے تاکہ گورننس پیکجوں کے بغیر دوبارہ چلائے بغیر پروف پروف شواہد کو محفوظ کیا جاسکے۔

## اضافی لنکس

- `docs/source/sorafs_cli.md` - جھنڈوں پر جامع دستاویزات۔
- `docs/source/sorafs_proof_streaming.md` - ثبوت ٹیلی میٹری ڈایاگرام اور Grafana ڈیش بورڈ ٹیمپلیٹ۔
- `docs/source/sorafs/manifest_pipeline.md` - chunking ، ظاہر اسمبلی اور کار پروسیسنگ کا تفصیلی تجزیہ۔