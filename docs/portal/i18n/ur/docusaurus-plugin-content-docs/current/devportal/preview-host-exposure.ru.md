---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/preview-host-exposure.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پیش نظارہ میزبان نمائش گائیڈ

DOCS-SORA روڈ میپ کا تقاضا ہے کہ ہر عوامی پیش نظارہ وہی چیکسم کی تصدیق شدہ بنڈل استعمال کریں جس کا جائزہ لینے والے مقامی طور پر چیک کریں۔ بیٹا پیش نظارہ ہوسٹ آن لائن لانے کے لئے جائزہ لینے والے پر سوار بورڈنگ (اور دعوت ناموں کی منظوری کے بعد) اس رن بک کا استعمال کریں۔

## شرائط

- جائزہ لینے والے پر سوار ہونے کی لہر کو پیش نظارہ ٹریکر میں منظور اور ریکارڈ کیا گیا ہے۔
- تازہ ترین پورٹل بلڈ `docs/portal/build/` میں واقع ہے اور چیکسم کی جانچ پڑتال کی گئی ہے (`build/checksums.sha256`)۔
- SoraFS پیش نظارہ اسناد (Torii URL ، اتھارٹی ، نجی کلید ، جو EPOCH کے ذریعہ بھیجا گیا ہے) ماحولیاتی متغیرات یا JSON CONFIG میں محفوظ کیا جاتا ہے ، مثال کے طور پر [`docs/examples/sorafs_preview_publish.json`] (../../../examples/sorafs_preview_publish.json)۔
- مطلوبہ میزبان نام (`docs-preview.sora.link` ، `docs.iroha.tech` ، وغیرہ) اور آن کال رابطوں کے ساتھ DNS کو تبدیل کرنے کے لئے ایک ٹکٹ کھولا گیا ہے۔

## مرحلہ 1 - بنڈل کی تعمیر اور جانچ کریں

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

چیک اسکرپٹ جاری رکھنے سے انکار کردے گا اگر چیکسم مینی فیسٹ غائب ہے یا اس میں چھیڑ چھاڑ ہے ، جو تمام پیش نظارہ نمونے کے آڈٹ کو محفوظ رکھتی ہے۔

## مرحلہ 2 - پیک نمونے SoraFS

ایک جامد سائٹ کو ایک تعی .ن کار/مینی فیسٹ جوڑی میں تبدیل کریں۔ `ARTIFACT_DIR` بذریعہ ڈیفالٹ `docs/portal/artifacts/`۔

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

`portal.car` ، `portal.manifest.*` ، وضاحتی اور چیکسم پیش نظارہ لہر کے ٹکٹ سے ظاہر ہوتا ہے۔

## مرحلہ 3 - پیش نظارہ عرف شائع کریں

جب آپ میزبان کھولنے کے لئے تیار ہوں تو ** `--skip-submit` کے بغیر پن ہیلپر ** چلائیں۔ JSON تشکیل یا واضح CLI جھنڈوں کو پاس کریں:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

کمانڈ `portal.pin.report.json` ، `portal.manifest.submit.summary.json` اور `portal.submit.response.json` لکھتا ہے ، جسے دعوت ناموں کے ثبوت کے بنڈل میں شامل کیا جانا چاہئے۔

## مرحلہ 4 - ڈی این ایس کٹ اوور پلان تیار کریں

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

نتیجے میں JSON کو او پی ایس کے ساتھ شیئر کریں تاکہ DNS سوئچنگ کا حوالہ عین ڈائجسٹ ظاہر ہوجائے۔ جب کسی رول بیک ماخذ کے طور پر کسی سابقہ ​​ڈسکرپٹر کو دوبارہ استعمال کرتے ہو تو ، `--previous-dns-plan path/to/previous.json` شامل کریں۔

## مرحلہ 5 - تعینات میزبان کو چیک کریں

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

تحقیقات دیئے گئے ریلیز ٹیگ ، سی ایس پی ہیڈر اور دستخطی میٹا ڈیٹا کی تصدیق کرتی ہے۔ دو خطوں سے کمانڈ دہرائیں (یا curl آؤٹ پٹ منسلک کریں) تاکہ آڈیٹر یہ دیکھ سکیں کہ کنارے کیشے کو گرم کیا گیا ہے۔

## ثبوت بنڈل

پیش نظارہ لہر کے ٹکٹ میں درج ذیل نمونے شامل کریں اور دعوت نامے میں ان کی نشاندہی کریں:

| نمونہ | منزل |
| ---------- | ----------- |
| `build/checksums.sha256` | ثابت کرتا ہے کہ بنڈل CI کی تعمیر سے مماثل ہے۔ |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | کیننیکل SoraFS پے لوڈ + مینی فیسٹ۔ |
| `portal.pin.report.json` ، `portal.manifest.submit.summary.json` ، `portal.submit.response.json` | اشارہ کرتا ہے کہ ظاہر بھیجنے اور عرف بائنڈنگ کامیاب رہی۔ |
| `artifacts/sorafs/portal.dns-cutover.json` | ڈی این ایس میٹا ڈیٹا (ٹکٹ ، ونڈو ، رابطے) ، روٹ پروگریس سمری (`Sora-Route-Binding`) ، پوائنٹر `route_plan` (JSON پلان + ہیڈر ٹیمپلیٹس) ، کیشے کو صاف کریں اور او پی ایس کے لئے رول بیک ہدایات۔ |
| `artifacts/sorafs/preview-descriptor.json` | آرکائیو + چیکسم سے منسلک وضاحتی ڈسکرپٹر پر دستخط شدہ۔ |
| پن `probe` | تصدیق کرتا ہے کہ براہ راست میزبان متوقع ریلیز ٹیگ شائع کررہا ہے۔ |