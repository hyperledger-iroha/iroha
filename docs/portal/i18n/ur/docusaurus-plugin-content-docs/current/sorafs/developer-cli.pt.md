---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/developer-cli.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ڈویلپر-سی ایل آئی
عنوان: SoraFS CLI ترکیبیں
سائڈبار_لیبل: سی ایل آئی ترکیبیں
تفصیل: `sorafs_cli` کی مستحکم سطح کے لئے ٹاسک پر مبنی گائیڈ۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/developer/cli.md` کا آئینہ دار ہے۔ دونوں کاپیاں ہم آہنگ رکھیں۔
:::

`sorafs_cli` کنسولیڈیٹیڈ سطح (`sorafs_car` کریٹ کے ذریعہ `cli` فعال کے ساتھ فراہم کردہ) SoraFS نمونے تیار کرنے کے لئے ضروری ہر قدم کو بے نقاب کرتا ہے۔ عام ورک فلوز میں سیدھے کودنے کے لئے اس کک بوک کا استعمال کریں۔ آپریشنل سیاق و سباق کے لئے آرکسٹریٹر مینی فیسٹ پائپ لائن اور رن بکس کے ساتھ یکجا کریں۔

## پیک پے لوڈز

عین مطابق کار فائلوں اور حصہ منصوبوں کو تیار کرنے کے لئے `car pack` کا استعمال کریں۔ کمانڈ خود بخود SF-1 چنکر کا انتخاب کرتا ہے جب تک کہ ہینڈل فراہم نہ کیا جائے۔

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- پہلے سے طے شدہ چنکر ہینڈل: `sorafs.sf1@1.0.0`۔
- ڈائریکٹری اندراجات کو لغت کے مطابق ترتیب دیا جاتا ہے تاکہ پلیٹ فارمز میں چیکسم مستحکم ہوں۔
- JSON ڈائجسٹ میں پے لوڈ ڈائجسٹ ، فی چنک میٹا ڈیٹا ، اور روٹ سی آئی ڈی شامل ہے جو رجسٹری اور آرکسٹریٹر کے ذریعہ تسلیم شدہ ہے۔

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

- `--pin-*` اختیارات `sorafs_manifest::ManifestBuilder` میں براہ راست `PinPolicy` فیلڈز کا نقشہ۔
- جب آپ چاہتے ہیں کہ CLI بھیجنے سے پہلے CLI کے SHA3 ڈائجسٹ کو دوبارہ گنتی کرنا چاہتے ہو تو `--chunk-plan` فراہم کریں۔ بصورت دیگر یہ خلاصہ میں سرایت شدہ ہضم کو دوبارہ استعمال کرتا ہے۔
- JSON آؤٹ پٹ جائزوں کے دوران سادہ فرق کے لئے Norito پے لوڈ کا آئینہ دار ہے۔

## طویل المیعاد چابیاں کے بغیر ظاہر ہوتا ہے

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- ان لائن ٹوکن ، ماحولیاتی متغیرات یا فائل پر مبنی فونٹ قبول کرتا ہے۔
- خام JWT کو برقرار رکھے بغیر ، جب تک `--include-token=true` کو برقرار رکھے بغیر پروویژن میٹا ڈیٹا (`token_source` ، `token_hash_hex` ، CUNK DISCT) شامل کرتا ہے۔
- CI میں اچھی طرح سے کام کرتا ہے: `--identity-token-provider=github-actions` ترتیب دے کر گٹ ہب ایکشن سے OIDC کے ساتھ یکجا کریں۔

## Torii پر ظاہر کریں

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

- Norito کو Torii پر پوسٹ کے ذریعے بھیجنے سے پہلے عرفی ثبوتوں اور چیکوں کے لئے Norito۔
- موڑ کے حملوں سے بچنے کے لئے ہوائی جہاز سے حصہ کے SHA3 ڈائجسٹ کو دوبارہ گنتی کرتا ہے۔
- جوابی خلاصہ بعد میں آڈٹ کے ل H HTTP کی حیثیت ، ہیڈر ، اور لاگ پے لوڈ پر قبضہ کرتا ہے۔

## کار کے مشمولات اور ثبوت چیک کریں

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- پور کے درخت کی تشکیل نو اور پے لوڈ ہضم کو مینی فیسٹ سمری کے ساتھ موازنہ کرتا ہے۔
- گورننس میں نقل کے ثبوت بھیجتے وقت مطلوبہ گنتی اور شناخت کنندگان کی گرفتاری۔

## پروف پروف ٹیلی میٹری کو منتقل کریں

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v2/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- منتقل کردہ ہر ثبوت کے لئے NDJSON آئٹمز کا اخراج (`--emit-events=false` کے ساتھ ری پلے کو غیر فعال کریں)۔
- کامیابی/ناکامی کی گنتی ، لیٹینسی ہسٹگرامس ، اور نمونے کی ناکامیوں کو JSON سمری میں جمع کرتا ہے تاکہ ڈیش بورڈز لاگ ان پڑھے بغیر نتائج پلاٹ کرسکیں۔
- غیر صفر کوڈ کے ساتھ باہر نکلتا ہے جب گیٹ وے کی ناکامی کی اطلاع ہوتی ہے یا جب مقامی پور چیک (`--por-root-hex` کے ذریعے) ثبوتوں کو مسترد کرتا ہے۔ ٹیسٹ رنز کے ل I `--max-failures` اور `--max-verification-failures` کے ساتھ تھریشولڈز کو ایڈجسٹ کریں۔
- آج پور کی حمایت کرتا ہے ؛ جب SF-13/SF-14 آنے پر PDP اور POTR اسی لفافے کو دوبارہ استعمال کریں۔
- `--governance-evidence-dir` مہیا کردہ سمری ، میٹا ڈیٹا (ٹائم اسٹیمپ ، سی ایل آئی ورژن ، گیٹ وے یو آر ایل ، منشور ڈائجسٹ) اور فراہم کردہ ڈائرکٹری میں ظاہر ہونے والی ایک کاپی لکھتا ہے تاکہ گورننس پیکجوں نے پھانسی کو دہرائے بغیر پروف اسٹریم شواہد کو محفوظ کیا۔

## اضافی حوالہ جات

- `docs/source/sorafs_cli.md` - جھنڈوں کی مکمل دستاویزات۔
- `docs/source/sorafs_proof_streaming.md` - پروف ٹیلی میٹری اسکیم اور ڈیش بورڈ ٹیمپلیٹ Grafana۔
- `docs/source/sorafs/manifest_pipeline.md` - chunking ، ظاہر مرکب اور کار کے انتظام میں گہری غوطہ۔