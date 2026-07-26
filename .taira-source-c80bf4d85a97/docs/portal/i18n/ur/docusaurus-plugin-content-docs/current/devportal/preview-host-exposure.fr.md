---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/preview-host-exposure.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پیش نظارہ میزبان نمائش گائیڈ

DOCS-SORA روڈ میپ کا تقاضا ہے کہ ہر عوامی پیش نظارہ اسی چیکسم کی تصدیق شدہ بنڈل پر تیار کرے جس کا جائزہ لینے والے مقامی طور پر جانچتے ہیں۔ بیٹا ہوسٹ آن لائن رکھنے کے لئے جائزہ لینے والے پر بورڈنگ (اور دعوت نامہ کی منظوری کے ٹکٹ) کے بعد اس رن بک کا استعمال کریں۔

## شرائط

- جائزہ لینے والے جہاز پر چلنے والی لہر کو پیش نظارہ ٹریکر میں منظور اور ریکارڈ کیا گیا۔
- `docs/portal/build/` اور چیکسم کی تصدیق شدہ (`build/checksums.sha256`) کے تحت موجود پورٹل کی تازہ ترین تعمیر۔
- SoraFS پیش نظارہ شناخت کار (URL Torii ، اتھارٹی ، نجی کلید ، پیش کیا گیا) ماحولیاتی متغیرات میں ذخیرہ شدہ یا JSON تشکیل جیسے [`docs/examples/sorafs_preview_publish.json`] (../../../examples/sorafs_preview_publish.json)۔
- مطلوبہ میزبان (`docs-preview.sora.link` ، `docs.iroha.tech` ، وغیرہ) کے ساتھ کھولا گیا ٹکٹ آن کال رابطوں کے ساتھ کھولا گیا۔

## مرحلہ 1 - بنڈل کی تعمیر اور تصدیق کریں

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

تصدیق کا اسکرپٹ جاری رکھنے سے انکار کرتا ہے جب چیکسم ظاہر ہوتا ہے یا خراب ہوتا ہے ، جو ہر پیش نظارہ نمونے کا آڈٹ رکھتا ہے۔

## مرحلہ 2 - نمونے SoraFS کو پیکج کریں

جامد سائٹ کو ایک عین مطابق کار/مینی فیسٹ جوڑی میں تبدیل کریں۔ `ARTIFACT_DIR` `docs/portal/artifacts/` سے پہلے سے طے شدہ ہے۔

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

`portal.car` ، `portal.manifest.*` ، وضاحتی اور چیکسم پیش نظارہ لہر کے ٹکٹ سے ظاہر ہوتا ہے۔

## مرحلہ 3 - عرف پیش نظارہ شائع کریں

جب آپ میزبان کو بے نقاب کرنے کے لئے تیار ہوں تو ** `--skip-submit` کے بغیر پن ہیلپر کو دوبارہ بنائیں۔ یا تو JSON تشکیل یا واضح CLI جھنڈے فراہم کریں:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

کمانڈ `portal.pin.report.json` ، `portal.manifest.submit.summary.json` اور `portal.submit.response.json` لکھتا ہے ، جس میں دعوت نامہ کے بنڈل کے ساتھ ہونا ضروری ہے۔

## مرحلہ 4 - DNS فیل اوور پلان تیار کریں

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

نتیجے میں JSON کو اوپس کے ساتھ شیئر کریں تاکہ DNS سوئچ عین مطابق ظاہر ہضم کا حوالہ دے۔ جب پچھلے ڈسکرپٹر کو رول بیک ماخذ کے طور پر دوبارہ استعمال کیا جاتا ہے تو ، `--previous-dns-plan path/to/previous.json` شامل کریں۔

## مرحلہ 5 - تعینات میزبان کا سروے کریں

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

تحقیقات پیش کردہ ریلیز ٹیگ ، سی ایس پی ہیڈر اور دستخطی میٹا ڈیٹا کی تصدیق کرتی ہے۔ دو خطوں سے کمانڈ دوبارہ جاری کریں (یا کرل آؤٹ پٹ منسلک کریں) تاکہ سامعین دیکھیں کہ ایج کیشے گرم ہے۔

## ثبوت بنڈل

پیش نظارہ لہر کے ٹکٹ میں درج ذیل نمونے شامل کریں اور دعوت نامے کے ای میل میں ان کا حوالہ دیں:| نمونہ | مقصد |
| --------- | --------- |
| `build/checksums.sha256` | ثابت کرتا ہے کہ بنڈل CI کی تعمیر سے مماثل ہے۔ |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | پے لوڈ SoraFS کینونیکل + مینی فیسٹ۔ |
| `portal.pin.report.json` ، `portal.manifest.submit.summary.json` ، `portal.submit.response.json` | ظاہر کرتا ہے کہ ظاہر جمع کرانے + عرف بائنڈنگ کامیاب رہی۔ |
| `artifacts/sorafs/portal.dns-cutover.json` | ڈی این ایس میٹا ڈیٹا (ٹکٹ ، ونڈو ، رابطے) ، روٹ پروموشن سمری (`Sora-Route-Binding`) ، `route_plan` پوائنٹر (JSON پلان + ہیڈر ٹیمپلیٹس) ، کیشے پرج انفارمیشن اور او پی ایس کے لئے رول بیک ہدایات۔ |
| `artifacts/sorafs/preview-descriptor.json` | ڈسریکٹر پر دستخط کریں جو آرکائیو + چیکسم کو جوڑتا ہے۔ |
| آؤٹ پٹ `probe` | تصدیق کرتا ہے کہ آن لائن میزبان متوقع ریلیز ٹیگ کا اعلان کرتا ہے۔ |