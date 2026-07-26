---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/public-preview-invite.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# عوامی پیش نظارہ دعوت نامہ پلے بک

## پروگرام کے مقاصد

یہ پلے بوک وضاحت کرتا ہے کہ ایک بار عوامی پیش نظارہ کا اعلان اور چلائیں
جائزہ لینے والا کام کرنے والا ورک فلو فعال ہے۔ یہ DOCS-SORA روڈ میپ کو ایماندار رکھتا ہے
اس بات کو یقینی بنانا کہ ہر دعوت نامہ قابل تصدیق نمونے ، حفاظتی ہدایات کے ساتھ روانہ ہوجائے
اور آراء کے لئے ایک واضح راستہ۔

- ** سامعین: ** کمیونٹی کے ممبروں ، شراکت داروں اور دیکھ بھال کرنے والوں کی تیار کردہ فہرست جو ہیں
  پیش نظارہ قابل قبول استعمال کی پالیسی پر دستخط کرتا ہے۔
- ** کیپس: ** پہلے سے طے شدہ لہر کا سائز <= 25 جائزہ لینے والے ، 14 دن تک رسائی ونڈو ،
  24 گھنٹوں کے اندر واقعات کا جواب۔

## لانچ گیٹ چیک لسٹ

دعوت نامہ بھیجنے سے پہلے ان کاموں کو مکمل کریں:

1. تازہ ترین پیش نظارہ سی آئی (`docs-portal-preview` میں بھری ہوئی ،
   چیکسم مینی فیسٹ ، ڈسریکٹر ، بنڈل SoraFS)۔
2. `npm run --prefix docs/portal serve` (گیٹ بائی چیکسم) اسی ٹیگ پر ٹیسٹ کیا گیا۔
3. دعوت نامہ کی لہر سے منسلک منظور شدہ جائزہ لینے والوں کے لئے جہاز پر چلنے والے ٹکٹ۔
4. سیکیورٹی ، مشاہدہ اور واقعہ کے درست دستاویزات
   ([`security-hardening`] (./security-hardening.md) ،
   [`observability`] (./observability.md) ،
   [`incident-runbooks`] (./incident-runbooks.md))۔
5. تاثرات کا فارم یا جاری کردہ ٹیمپلیٹ تیار (شدت والے فیلڈز ، شامل کریں ،
   پنروتپادن اقدامات ، اسکرین شاٹس اور ماحولیات کی معلومات)۔
6. اعلان کاپی دستاویزات/ڈیوریل + گورننس کے ذریعہ جائزہ لیا گیا۔

## دعوت پیکیج

ہر دعوت نامے میں شامل ہونا ضروری ہے:

1. ** تصدیق شدہ نمونے ** - ظاہر/منصوبہ SoraFS یا نمونے کے ل links لنک فراہم کریں
   گٹ ہب ، پلس چیکسم منشور اور ڈسریکٹر۔ واضح طور پر کمانڈ کا حوالہ دیں
   توثیق تاکہ جائزہ لینے والے سائٹ کو لانچ کرنے سے پہلے اسے چلا سکیں۔
2. ** ہدایات کی خدمت کریں ** - چیکسم کے ذریعہ پیش نظارہ کمانڈ شامل کریں:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3.
   شیئر نہیں کیا جانا چاہئے ، اور واقعات کی فوری اطلاع دی جانی چاہئے۔
4. ** فیڈ بیک چینل ** - ٹیمپلیٹ/فارم کو لنک کریں اور جوابی وقت کی توقعات کو واضح کریں۔
5. ** پروگرام کی تاریخیں ** - اسٹارٹ/اختتامی تاریخیں ، دفتر کے اوقات یا مطابقت پذیری اور اگلا فراہم کریں
   ریفریش ونڈو۔

ای میل کی مثال میں
[`docs/examples/docs_preview_invite_template.md`] (../../../examples/docs_preview_invite_template.md)
ان ضروریات کا احاطہ کرتا ہے۔ پلیس ہولڈرز کو اپ ڈیٹ کریں (تاریخیں ، یو آر ایل ، رابطے)
بھیجنے سے پہلے

## میزبان پیش نظارہ کو بے نقاب کریں

آن بورڈنگ مکمل ہونے کے بعد ہی میزبان پیش نظارہ کو ہی فروغ دیں اور تبدیلی کے ٹکٹ کی منظوری دی گئی ہے۔
اختتام سے آخر میں مرحلے کے لئے [میزبان پیش نظارہ گائیڈ] (./preview-host-exposure.md) دیکھیں
اس حصے میں استعمال ہونے والے بلڈ/شائع/تصدیق کی۔

1. ** تعمیر اور پیکیجنگ: ** ریلیز ٹیگ کو نشان زد کریں اور اختیاری نمونے تیار کریں۔

   ```bash
   cd docs/portal
   export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
   npm ci
   npm run build
   ./scripts/sorafs-pin-release.sh \
     --alias docs-preview.sora \
     --alias-namespace docs \
     --alias-name preview \
     --pin-label docs-preview \
     --skip-submit
   node scripts/generate-preview-descriptor.mjs \
     --manifest artifacts/checksums.sha256 \
     --archive artifacts/sorafs/portal.tar.gz \
     --out artifacts/sorafs/preview-descriptor.json
   ```

   پن اسکرپٹ `portal.car` ، `portal.manifest.*` ، `portal.pin.proposal.json` لکھتا ہے ،
   اور `portal.dns-cutover.json` `artifacts/sorafs/` کے تحت۔ ان فائلوں کو لہر سے جوڑیں
   دعوت نامہ تاکہ ہر جائزہ لینے والا ایک ہی بٹس کو چیک کرسکے۔2
   (provide `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, and proof of alias issued
   گورننس کے ذریعے)۔ اسکرپٹ `docs-preview.sora` اور اخراج سے ظاہر ہوتا ہے
   `portal.manifest.submit.summary.json` پلس `portal.pin.report.json` ثبوت کے بنڈل کے لئے۔

3. ** تعی .ن کی تحقیقات: ** تصدیق کریں کہ عرف حل ہوجاتا ہے اور یہ کہ چیکسم ٹیگ سے مماثل ہے
   دعوت نامے بھیجنے سے پہلے۔

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   `npm run serve` (`scripts/serve-verified-preview.mjs`) کو ایک فال بیک کے طور پر ہاتھ پر رکھیں
   تاکہ جائزہ لینے والے مقامی کاپی لانچ کرسکیں اگر ایج کا پیش نظارہ خراب ہوجاتا ہے۔

## مواصلات کی ٹائم لائن

| دن | ایکشن | مالک |
| --- | --- | --- |
| D-3 | دعوت نامے کو حتمی شکل دیں ، نمونے کو تازہ کریں ، خشک رن کی توثیق | دستاویزات/ڈیوریل |
| D-2 | سائن آف گورننس + تبدیلی کا ٹکٹ | دستاویزات/ڈیوریل + گورننس |
| D-1 | ٹیمپلیٹ کے ذریعے دعوت نامے بھیجیں ، وصول کنندگان کی فہرست کے ساتھ ٹریکر کو اپ ڈیٹ کریں | دستاویزات/ڈیوریل |
| d | کک آف کال / آفس اوقات ، ٹیلی میٹری ڈیش بورڈز کی نگرانی کریں دستاویزات/ڈیوریل + آن کال |
| D+7 | مڈ ویو فیڈ بیک ڈائجسٹ ، مسدود مسائل کی چھانٹ رہا ہے دستاویزات/ڈیوریل |
| D+14 | بند لہر ، عارضی رسائی کو کالعدم کریں ، `status.md` میں خلاصہ شائع کریں دستاویزات/ڈیوریل |

## رسائی سے باخبر رہنے اور ٹیلی میٹری

1. ہر وصول کنندہ ، دعوت نامہ ٹائم اسٹیمپ اور منسوخ کی تاریخ کو ریکارڈ کریں
   پیش نظارہ رائے لاگ (دیکھیں
   [`preview-feedback-log`] (./preview-feedback-log)) تاکہ ہر لہر ایک ہی ہو
   ثبوت کا سراغ لگانا:

   ```bash
   # Ajouter un nouvel evenement d'invitation a artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   معاون واقعات `invite-sent` ، `acknowledged` ، ہیں ،
   `feedback-submitted` ، `issue-opened` ، اور `access-revoked`۔ لاگ واقع ہے
   `artifacts/docs_portal_preview/feedback_log.json` بذریعہ ڈیفالٹ ؛ اسے رسید سے منسلک کریں
   رضامندی کے فارموں کے ساتھ دعوت کی لہر۔ خلاصہ مددگار استعمال کریں
   اختتامی نوٹ سے پہلے آڈٹیبل رول اپ تیار کرنے کے لئے:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   JSON کا خلاصہ لہر کے ذریعہ دعوت ناموں کی فہرست دیتا ہے ، کھلے وصول کنندگان ،
   تاثرات کاؤنٹرز اور آخری ایونٹ کا ٹائم اسٹیمپ۔ مددگار پر مبنی ہے
   [`scripts/preview-feedback-log.mjs`] (../../scripts/preview-feedback-log.mjs) ،
   لہذا وہی ورک فلو مقامی طور پر یا CI میں چلا سکتا ہے۔ ڈائجسٹ ٹیمپلیٹ کا استعمال کریں
   [`docs/examples/docs_preview_feedback_digest.md`] (../../../examples/docs_preview_feedback_digest.md) میں
   جب لہر کی بازیافت کی اشاعت کریں۔
2. ٹیلی میٹری ڈیش بورڈز کو `DOCS_RELEASE_TAG` کے ساتھ ٹیگ کریں تاکہ لہر کے لئے استعمال کیا جاسکے
   چوٹیوں کو دعوت نامے سے منسلک کیا جاسکتا ہے۔
3. اس کی تصدیق کے لئے تعیناتی کے بعد `npm run probe:portal -- --expect-release=<tag>` چلائیں
   پیش نظارہ ماحول صحیح رہائی میٹا ڈیٹا کا اعلان کرتا ہے۔
4. رن بک ٹیمپلیٹ میں کسی بھی واقعات کو ریکارڈ کریں اور انہیں کوہورٹ سے جوڑیں۔

## رائے اور بندش1. آراء کو شیئرنگ دستاویز یا ایشو بورڈ میں جمع کریں۔ ٹیگ آئٹمز کے ساتھ
   `docs-preview/<wave>` تاکہ روڈ میپ مالکان انہیں آسانی سے تلاش کرسکیں۔
2. لہر کی رپورٹ کو آباد کرنے کے لئے پیش نظارہ لاگر کے خلاصہ آؤٹ پٹ کا استعمال کریں ، پھر
   `status.md` (شرکاء ، اہم نتائج ، منصوبہ بند اصلاحات) اور میں شریک کا خلاصہ کریں
   `roadmap.md` کو اپ ڈیٹ کریں اگر سنگ میل دستاویزات-SORA میں بدلا ہوا ہے۔
3. آف بورڈنگ مراحل پر عمل کریں
   [`reviewer-onboarding`](./reviewer-onboarding.md): revoke access, archive requests and
   شرکا کا شکریہ۔
4. نمونے کو تازہ دم کرکے اور چیکسم کے دروازوں کو دوبارہ شروع کرکے اگلی لہر کی تیاری کریں
   اور نئی تاریخوں کے ساتھ دعوت نامے کے سانچے کو اپ ڈیٹ کرنا۔

اس پلے بوک کا اطلاق مستقل طور پر پیش نظارہ پروگرام کو قابل اظہار اور دیتا ہے
DOCS/Dewrel دعوت ناموں کو بڑھانے کا ایک قابل تکرار طریقہ جب پورٹل GA کے قریب آتا ہے۔