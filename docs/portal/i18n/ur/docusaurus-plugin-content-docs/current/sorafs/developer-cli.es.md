---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/developer-cli.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ڈویلپر-سی ایل آئی
عنوان: SoraFS CLI کک بوک
سائڈبار_لیبل: سی ایل آئی کک بوک
تفصیل: `sorafs_cli` کی مستحکم سطح کا ٹاسک پر مبنی ٹور۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/developer/cli.md` کی عکاسی کرتا ہے۔ دونوں کاپیاں مطابقت پذیری میں رکھیں۔
:::

`sorafs_cli` کنسولیڈیٹیڈ سطح (`sorafs_car` کریٹ کے ذریعہ `cli` فعال کے ساتھ فراہم کردہ) SoraFS نمونے تیار کرنے کے لئے ضروری ہر قدم کو بے نقاب کرتا ہے۔ عام بہاؤ میں براہ راست کودنے کے لئے اس کک بوک کا استعمال کریں۔ آپریشنل سیاق و سباق کے ل it اس کو مینی فیسٹ پائپ لائن اور آرکسٹریٹر رن بوکس کے ساتھ جوڑیں۔

## پیکیج پے لوڈ

عین مطابق کار فائلوں اور حصہ کے منصوبوں کو تیار کرنے کے لئے `car pack` استعمال کرتا ہے۔ کمانڈ خود بخود SF-1 چنکر کا انتخاب کرتا ہے جب تک کہ ہینڈل فراہم نہ کیا جائے۔

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- پہلے سے طے شدہ چنکر ہینڈل: `sorafs.sf1@1.0.0`۔
- ڈائریکٹری اندراجات کو لغت کے مطابق ترتیب دیا جاتا ہے تاکہ پلیٹ فارم کے مابین چیکسی مستحکم رہیں۔
- JSON ڈائجسٹ میں پے لوڈ ڈائجسٹس ، میٹا ڈیٹا فی حصہ ، اور رجسٹری اور آرکسٹریٹر کے ذریعہ تسلیم شدہ جڑ سی آئی ڈی شامل ہے۔

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
- `--chunk-plan` استعمال کریں جب آپ چاہتے ہیں کہ CLI بھیجنے سے پہلے SHA3 چنک ڈائجسٹ کو دوبارہ گنتی کرے۔ بصورت دیگر یہ خلاصہ میں سرایت شدہ ہضم کو دوبارہ استعمال کرتا ہے۔
- JSON آؤٹ پٹ ترمیم کے دوران سادہ فرق کے لئے پے لوڈ Norito کی عکاسی کرتا ہے۔

## طویل المیعاد چابیاں کے بغیر ظاہر ہوتا ہے

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- ان لائن ٹوکن ، ماحولیاتی متغیرات یا فائل پر مبنی ذرائع کو قبول کرتا ہے۔
- `--include-token=true` کے علاوہ خام JWT کو برقرار رکھے بغیر پروویژن میٹا ڈیٹا (`token_source` ، `token_hash_hex` ، CUNK DISCT) شامل کریں۔
- CI میں اچھی طرح سے کام کرتا ہے: `--identity-token-provider=github-actions` ترتیب دے کر گٹ ہب ایکشن سے OIDC کے ساتھ جوڑیں۔

## Torii پر ظاہر کریں

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority ih58... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Norito کو عرفی ثبوتوں کے لئے ضابطہ کشائی کرتا ہے اور تصدیق کرتا ہے کہ وہ Torii پر پوسٹ کرنے سے پہلے مینی فیسٹ کے ڈائجسٹ سے ملتے ہیں۔
- مماثل حملوں کو روکنے کے لئے منصوبے سے SHA3 CHUNK ڈائجسٹ کو دوبارہ گنتی کریں۔
- جوابی خلاصہ بعد میں آڈٹ کے لئے HTTP کی حیثیت ، ہیڈر ، اور لاگ پے لوڈ پر قبضہ کرتا ہے۔

## کار اور ثبوت کے مندرجات کی تصدیق کریں

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- پور کے درخت کو دوبارہ تعمیر کریں اور پے لوڈ ڈائجسٹ کو مینی فیسٹ سمری کے ساتھ موازنہ کریں۔
- جب گورننس میں نقل کے ثبوت بھیجتے ہو تو گرفتاری گنتی اور شناخت کنندگان کی ضرورت ہوتی ہے۔

## پروف پروف ٹیلی میٹری کو منتقل کریں

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- منتقل کردہ ہر ثبوت کے لئے NDJSON عناصر کو خارج کریں (`--emit-events=false` کے ساتھ ری پلے کو غیر فعال کریں)۔
- کامیابی/ناکامی کی گنتی ، لیٹینسی ہسٹگرامس ، اور نمونے کی ناکامیوں کو JSON سمری میں شامل کریں تاکہ ڈیش بورڈز لاگ ان کے بغیر نتائج کو گراف کرسکیں۔
- یہ صفر کے علاوہ کسی دوسرے کوڈ کے ساتھ باہر نکلتا ہے جب گیٹ وے کی ناکامیوں یا مقامی پور کی توثیق (`--por-root-hex` کے ذریعے) ثبوتوں کو مسترد کرتا ہے۔ ٹیسٹ رنز کے ل I `--max-failures` اور `--max-verification-failures` کے ساتھ دہلیز طے کریں۔
- آج پور کی حمایت کرتا ہے ؛ جب SF-13/SF-14 آنے پر PDP اور POTR وہی پیکیجنگ کا دوبارہ استعمال کریں۔
- `--governance-evidence-dir` مہیا کی گئی سمری ، میٹا ڈیٹا (ٹائم اسٹیمپ ، سی ایل آئی ورژن ، گیٹ وے یو آر ایل ، مینی فیسٹ ڈائجسٹ) اور سپلائی ڈائریکٹری میں ظاہر ہونے والی ایک کاپی لکھتا ہے تاکہ گورننس پیکجز پر عمل درآمد کو دہرانے کے بغیر پروف پروف پروف پروف ثبوت آرکائیو کریں۔

## اضافی حوالہ جات

- `docs/source/sorafs_cli.md` - مکمل جھنڈوں کی دستاویزات۔
- `docs/source/sorafs_proof_streaming.md` - پروف ٹیلی میٹری اسکیما اور ڈیش بورڈ ٹیمپلیٹ Grafana۔
- `docs/source/sorafs/manifest_pipeline.md` - chunking ، ظاہر مرکب اور کار کے انتظام میں گہرا ہونا۔