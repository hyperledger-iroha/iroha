---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/preview-host-exposure.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پیش نظارہ میزبان نمائش گائیڈ

DOCS-SORA روڈ میپ کا تقاضا ہے کہ ہر عوامی پیش نظارہ وہی چیکسم کی تصدیق شدہ بنڈل استعمال کریں جس کا جائزہ لینے والے مقامی طور پر ورزش کرتے ہیں۔ اس رن بک کو استعمال کرنے والے کے بعد جائزہ لینے والے کے بعد (اور دعوت نامہ کی منظوری کا ٹکٹ) بیٹا ہوسٹ آن لائن لانے کے لئے مکمل ہے۔

## شرائط

- جائزہ لینے والے آن بورڈنگ ویو کی منظوری اور پیش نظارہ ٹریکر میں رجسٹرڈ۔
- `docs/portal/build/` اور چیکسم کی تصدیق شدہ (`build/checksums.sha256`) پر موجود پورٹل کی تازہ ترین تعمیر۔
- SoraFS پیش نظارہ اسناد (URL Torii ، اتھارٹی ، نجی کلید ، بھیجے گئے) ماحولیاتی متغیرات میں یا JSON کنفیگ جیسے [`docs/examples/sorafs_preview_publish.json`] (../../../examples/sorafs_preview_publish.json) میں محفوظ کیا گیا ہے۔
- اوپن ڈی این ایس مطلوبہ میزبان نام (`docs-preview.sora.link` ، `docs.iroha.tech` ، وغیرہ) کے ساتھ ٹکٹ تبدیل کریں۔

## مرحلہ 1 - بنڈل کی تعمیر اور تصدیق کریں

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

توثیق کا اسکرپٹ جاری رکھنے سے انکار کرتا ہے جب ہر پیش نظارہ نمونے کے آڈٹ کو برقرار رکھتے ہوئے ، جب چیکم مینی فیسٹ غائب ہو یا اس میں چھیڑ چھاڑ ہو۔

## مرحلہ 2 - نمونے SoraFS پیک کریں

جامد سائٹ کو ایک عین مطابق کار/مینی فیسٹ جوڑی میں تبدیل کریں۔ `ARTIFACT_DIR` پہلے سے طے شدہ اور `docs/portal/artifacts/` ہے۔

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

`portal.car` ، `portal.manifest.*` ، ڈسکرپٹر ، اور چیکسم پیش نظارہ لہر کے ٹکٹ سے ظاہر ہوتا ہے۔

## مرحلہ 3 - پیش نظارہ عرف شائع کریں

جب آپ میزبان کو بے نقاب کرنے کے لئے تیار ہوں تو ** `--skip-submit` کے بغیر پن ہیلپر کو دوبارہ بنائیں۔ واضح JSON تشکیل یا CLI جھنڈے فراہم کریں:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

کمانڈ `portal.pin.report.json` ، `portal.manifest.submit.summary.json` اور `portal.submit.response.json` لکھتا ہے ، جس میں دعوت نامہ کے بنڈل کے ساتھ ہونا ضروری ہے۔

## مرحلہ 4 - DNS کاٹنے کا منصوبہ تیار کریں

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

نتیجے میں JSON کو اوپس کے ساتھ بانٹیں تاکہ DNS تبدیلی عین مطابق ظاہر ہضم کا حوالہ دے۔ جب کسی رول بیک ماخذ کے طور پر کسی سابقہ ​​ڈسکرپٹر کو دوبارہ استعمال کرتے ہو تو ، `--previous-dns-plan path/to/previous.json` شامل کریں۔

## مرحلہ 5 - تعینات میزبان کی جانچ کریں

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

تحقیقات پیش کردہ ریلیز ٹیگ ، سی ایس پی ہیڈر ، اور میٹا ڈیٹا پر دستخط کرنے کی تصدیق کرتی ہے۔ دو خطوں (یا کرل آؤٹ پٹ کو شامل کریں) سے کمانڈ دہرائیں تاکہ آڈیٹرز دیکھیں کہ ایج کیشے گرم ہے۔

## ثبوت بنڈل

پیش نظارہ لہر کے ٹکٹ میں درج ذیل نمونے شامل کریں اور دعوت نامے کے ای میل میں ان کا حوالہ دیں:

| نمونہ | مقصد |
| ---------- | ----------- |
| `build/checksums.sha256` | ثابت کرتا ہے کہ بنڈل CI کی تعمیر سے مماثل ہے۔ |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | کیننیکل پے لوڈ SoraFS + مینی فیسٹ۔ |
| `portal.pin.report.json` ، `portal.manifest.submit.summary.json` ، `portal.submit.response.json` | ظاہر کرتا ہے کہ ظاہر جمع کرانے + عرف بائنڈنگ مکمل ہوچکی ہے۔ |
| `artifacts/sorafs/portal.dns-cutover.json` | ڈی این ایس میٹا ڈیٹا (ٹکٹ ، ونڈو ، رابطے) ، روٹ پروموشن سمری (`Sora-Route-Binding`) ، `route_plan` پوائنٹر (JSON پلان + ہیڈر ٹیمپلیٹس) ، کیشے پرج انفارمیشن اور او پی ایس کے لئے رول بیک ہدایات۔ |
| `artifacts/sorafs/preview-descriptor.json` | دستخط شدہ ڈسریکٹر جو آرکائیو + چیکسم کو باندھتا ہے۔ |
| `probe` کی آؤٹ پٹ | تصدیق کرتا ہے کہ براہ راست میزبان متوقع ریلیز ٹیگ کا اعلان کرتا ہے۔ |