---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/preview-host-exposure.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پیش نظارہ میزبان نمائش گائیڈ

DOCS-SORA روڈ میپ کا تقاضا ہے کہ ہر عوامی جائزہ اسی چیکس پیکیج پر انحصار کرے جس کا جائزہ لینے والے مقامی طور پر ٹیسٹ کرتے ہیں۔ اس گائیڈ کا استعمال کرتے ہوئے جائزہ لینے والے پر سوار ہونے کے بعد (اور دعوت نامہ کی منظوری کا ٹکٹ) ٹیسٹ کے پیش نظارہ میزبان کو نیٹ ورک پر ڈالنے کے لئے مکمل ہے۔

## شرائط

- آڈیٹرز کی اہلیت کی لہر کو منظور کرنا اور اسے معائنہ ٹریکر میں ریکارڈ کرنا۔
- آخری گیٹ وے بلڈ `docs/portal/build/` کے تحت ہے اور چیکسم کی تصدیق کی گئی ہے (`build/checksums.sha256`)۔
- SoraFS پیش نظارہ اسناد (Torii ایڈریس ، مجاز ہستی ، نجی کلید ، بھیجنے والا عہد) یا تو ماحولیاتی متغیرات میں یا JSON فائل میں [`docs/examples/sorafs_preview_publish.json`] (../../../examples/sorafs_preview_publish.json) کے طور پر محفوظ کیا جاتا ہے۔
- متبادل رابطوں کے علاوہ مطلوبہ نام (`docs-preview.sora.link` ، `docs.iroha.tech` ، ...) کے ساتھ DNS تبدیلی کا ٹکٹ کھولیں۔

## مرحلہ 1 - پیکیج کی تعمیر اور تصدیق کریں

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

تصدیق کا اسکرپٹ جاری رکھنے سے انکار کرتا ہے جب چیکسم کا بیان غائب ہے یا اس میں چھیڑ چھاڑ کی گئی ہے ، جس سے ہر معائنہ کا ٹریس آڈٹ ہوجاتا ہے۔

## مرحلہ 2 - پیک کے نشانات SoraFS

ایک جامد سائٹ کو ایک عین مطابق کار/مینی فیسٹ جوڑی میں تبدیل کریں۔ `ARTIFACT_DIR` بذریعہ ڈیفالٹ `docs/portal/artifacts/` ہے۔

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

`portal.car` ، `portal.manifest.*` ، ڈسکرپٹر ، اور چیکم بیان کو معائنہ لہر کے ٹکٹ سے منسلک کریں۔

## مرحلہ 3 - عرف پیش نظارہ شائع کریں

جب آپ میزبان کو بے نقاب کرنے کے لئے تیار ہوں تو ** `--skip-submit` کے بغیر پن ہیلپر کو دوبارہ شروع کریں۔ JSON فائل یا واضح CLI جھنڈے فراہم کریں:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

کمانڈ `portal.pin.report.json` ، `portal.manifest.submit.summary.json` ، اور `portal.submit.response.json` لکھیں ، جس میں دعوت نامے کے ساتھ ہونا چاہئے۔

## مرحلہ 4 - ڈی این ایس ری ڈائریکشن پلان تیار کریں

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

نتیجے میں JSON فائل کو او پی ایس ٹیم کے ساتھ شیئر کریں تاکہ DNS تبدیلی منشور کے عین مطابق ڈائجسٹ کی طرف اشارہ کرے۔ جب کسی رول بیک ماخذ کے طور پر کسی سابقہ ​​ڈسکرپٹر کو دوبارہ استعمال کرتے ہو تو ، `--previous-dns-plan path/to/previous.json` شامل کریں۔

## مرحلہ 5 - شائع شدہ میزبان کو چیک کریں

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

اسکین خدمت شدہ ورژن ٹیگ ، سی ایس پی ہیڈر ، اور دستخطی ڈیٹا کی تصدیق کرتا ہے۔ کمانڈ دو علاقوں سے واپس کریں (یا کرل آؤٹ پٹ منسلک کریں) تاکہ آڈیٹر دیکھ سکیں کہ کیشے کا کنارے گرم ہے۔

## ثبوت پیکیج

اپنے پیش نظارہ لہر کے ٹکٹ پر درج ذیل آئٹمز شامل کریں اور اپنی دعوت میل میں ان کی نشاندہی کریں:

| اثر | مقصد |
| ------- | ------- |
| `build/checksums.sha256` | ثابت کرتا ہے کہ پیکیج CI کی تعمیر سے مماثل ہے۔ |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | معیاری پے لوڈ SoraFS + مینی فیسٹ۔ |
| `portal.pin.report.json` ، `portal.manifest.submit.summary.json` ، `portal.submit.response.json` | یہ ظاہر کرنے اور عرف کو جوڑنے کی کامیابی کو ثابت کرتا ہے۔ |
| `artifacts/sorafs/portal.dns-cutover.json` | ڈی این ایس ڈیٹا (ٹکٹ ، ونڈو ، رابطے) ، روٹ اپ گریڈ سمری (`Sora-Route-Binding`) ، `route_plan` پوائنٹر (JSON پلان + ہیڈر ٹیمپلیٹس) ، کیشے صاف کریں اور او پی ایس ٹیم کے لئے رول بیک ہدایات۔ |
| `artifacts/sorafs/preview-descriptor.json` | ایک مقام کا بیان کرنے والا جو آرکائیو+چیکسم کو جوڑتا ہے۔ |
| `probe` آؤٹ پٹ | تصدیق کرتا ہے کہ براہ راست میزبان متوقع ریلیز ٹیگ کا اعلان کرتا ہے۔ |