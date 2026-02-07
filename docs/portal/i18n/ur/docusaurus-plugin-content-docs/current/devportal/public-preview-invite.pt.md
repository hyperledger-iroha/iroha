---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/public-preview-invite.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# عوامی پیش نظارہ دعوت نامہ پلے بک

## پروگرام کے مقاصد

یہ پلے بوک وضاحت کرتا ہے کہ جیسے ہی عوامی پیش نظارہ کا اعلان اور چلائیں
جائزہ لینے والا کام کرنے والا ورک فلو فعال ہے۔ یہ DOCS-SORA روڈ میپ کو ایماندار رکھتا ہے
اس بات کو یقینی بنائیں کہ ہر دعوت نامہ قابل تصدیق نمونے ، سیکیورٹی رہنمائی ، اور a
واضح آراء کا راستہ۔

- ** سامعین: ** کمیونٹی کے ممبروں ، شراکت داروں اور دیکھ بھال کرنے والوں کی فہرست کی فہرست جنہوں نے اس پر دستخط کیے
  پیش نظارہ قابل استعمال پالیسی۔
- ** حدود: ** پہلے سے طے شدہ لہر کا سائز <= 25 جائزہ لینے والے ، 14 دن تک رسائی ونڈو ، جواب
  24 گھنٹوں کے اندر واقعات میں۔

## لانچ گیٹ چیک لسٹ

کسی بھی دعوت نامے بھیجنے سے پہلے ان کاموں کو مکمل کریں:

1. تازہ ترین پیش نظارہ سی آئی کو بھیجا گیا (`docs-portal-preview` ،
   چیکسم مینی فیسٹ ، ڈسریکٹر ، بنڈل SoraFS)۔
2. `npm run --prefix docs/portal serve` (چیکسم کے ذریعہ گیٹڈ) اسی ٹیگ پر ٹیسٹ کیا گیا۔
3. منظور شدہ جائزہ لینے والوں کے لئے جہاز پر چلنے والے ٹکٹ اور دعوت ناموں کی لہر سے منسلک۔
4. توثیق شدہ سیکیورٹی ، مشاہدہ اور واقعہ کے دستاویزات
   ([`security-hardening`] (./security-hardening.md) ،
   [`observability`] (./observability.md) ،
   [`incident-runbooks`] (./incident-runbooks.md))۔
5. تیار کردہ آراء فارم یا ٹیمپلیٹ جاری کریں (شدت والے فیلڈز ، شامل کریں ،
   پنروتپادن اقدامات ، اسکرین شاٹس اور ماحولیات کی معلومات)۔
6. اشتہار کاپی کا جائزہ دستاویزات/ڈیوریل + گورننس کے ذریعہ کیا گیا ہے۔

## دعوت پیکیج

ہر دعوت نامے میں شامل ہونا ضروری ہے:

1. ** چیک شدہ نمونے ** - ظاہر/منصوبہ SoraFS یا نمونے کے ل links لنک فراہم کریں
   گٹ ہب ، نیز چیکسم مینی فیسٹ اور ڈسکرپٹر۔ واضح طور پر کمانڈ کا حوالہ دیں
   چیک کریں تاکہ جائزہ لینے والے سائٹ کو اپ لوڈ کرنے سے پہلے چلاسکیں۔
2.

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3.
   مشترکہ معلومات اور واقعات کی فوری اطلاع دی جانی چاہئے۔
4. ** فیڈ بیک چینل ** - ٹیمپلیٹ/فارم کو لنک کریں اور جوابی وقت کی توقعات کو واضح کریں۔
5. ** پروگرام کی تاریخیں ** - اسٹارٹ/اختتامی تاریخیں ، دفتر کے اوقات یا مطابقت پذیری درج کریں ، اور اگلا درج کریں
   ریفریش ونڈو۔

مثال کے طور پر ای میل
[`docs/examples/docs_preview_invite_template.md`] (../../../examples/docs_preview_invite_template.md)
ان ضروریات کا احاطہ کرتا ہے۔ پلیس ہولڈرز کو اپ ڈیٹ کریں (تاریخیں ، یو آر ایل ، رابطے)
بھیجنے سے پہلے

## پیش نظارہ میزبان کو بے نقاب کریں

جب بورڈنگ مکمل ہو اور تبدیلی کا ٹکٹ ہوتا ہے تو صرف پیش نظارہ میزبان کو ہی فروغ دیں
منظور شدہ اقدامات کے لئے [پیش نظارہ میزبان ایکسپوزر گائیڈ] (./preview-host-exposure.md) دیکھیں
اس سیکشن میں استعمال ہونے والے اختتام سے آخر تک بلڈ/شائع/تصدیق کریں۔

1. ** تعمیر اور پیکیجنگ: ** ریلیز ٹیگ کو ٹیگ کریں اور اختیاری نمونے تیار کریں۔

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
   اور `portal.dns-cutover.json` `artifacts/sorafs/` میں۔ ان فائلوں کو دعوت کی لہر سے منسلک کریں
   تاکہ ہر جائزہ لینے والا ایک ہی بٹس کو چیک کرسکے۔2. ** پیش نظارہ عرف شائع کریں: ** `--skip-submit` کے بغیر کمانڈ چلائیں
   (provides `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, and the alias proof issued
   گورننس کے ذریعہ)۔ اسکرپٹ `docs-preview.sora` اور جاری کرنے کے لئے ظاہر ہوگا
   `portal.manifest.submit.summary.json` پلس `portal.pin.report.json` ثبوت کے بنڈل کے لئے۔

3. ** تعیناتی کی جانچ کریں: ** تصدیق کریں کہ عرف حل ہوجاتا ہے اور یہ کہ چیکسم ٹیگ سے مماثل ہے
   دعوت نامے بھیجنے سے پہلے۔

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   `npm run serve` (`scripts/serve-verified-preview.mjs`) کو ایک فال بیک کے طور پر ہاتھ پر رکھیں
   لہذا جائزہ لینے والے ایک مقامی کاپی اپ لوڈ کرسکتے ہیں اگر پیش نظارہ ایج ناکام ہوجاتا ہے۔

## مواصلات کی ٹائم لائن

| دن | ایکشن | مالک |
| --- | --- | --- |
| D-3 | دعوت نامہ کاپی کو حتمی شکل دیں ، نمونے کو اپ ڈیٹ کریں ، توثیق ڈرائی رن | دستاویزات/ڈیوریل |
| D-2 | گورننس سائن آف + تبدیلی ٹکٹ | دستاویزات/ڈیوریل + گورننس |
| D-1 | ٹیمپلیٹ کا استعمال کرتے ہوئے دعوت نامے بھیجیں ، وصول کنندگان کی فہرست کے ساتھ ٹریکر کو اپ ڈیٹ کریں | دستاویزات/ڈیوریل |
| d | کک آف کال / آفس اوقات ، ٹیلی میٹری ڈیش بورڈز کی نگرانی کریں دستاویزات/ڈیوریل + آن کال |
| D+7 | وسط لہر کی رائے ڈائجسٹ ، ٹریج مسدود کرنے کے مسائل | دستاویزات/ڈیوریل |
| D+14 | لہر بند کریں ، عارضی رسائی کو کالعدم کریں ، `status.md` میں خلاصہ شائع کریں دستاویزات/ڈیوریل |

## رسائی سے باخبر رہنے اور ٹیلی میٹری

1. ہر وصول کنندہ ، دعوت نامہ ٹائم اسٹیمپ اور منسوخ کی تاریخ کو ریکارڈ کریں
   پیش نظارہ رائے لاگ (دیکھیں
   [`preview-feedback-log`] (./preview-feedback-log)) تاکہ ہر لہر ایک ہی ہو
   ثبوت کی پگڈنڈی:

   ```bash
   # Adicione um novo evento de convite a artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   معاون واقعات `invite-sent` ، `acknowledged` ، ہیں ،
   `feedback-submitted` ، `issue-opened` ، اور `access-revoked`۔ لاگ ان ہے
   `artifacts/docs_portal_preview/feedback_log.json` بذریعہ ڈیفالٹ ؛ ٹکٹ سے منسلک
   رضامندی کے فارموں کے ساتھ دعوت ناموں کی لہر۔ مددگار استعمال کریں
   اختتامی نوٹ سے پہلے آڈٹیبل رول اپ تیار کرنے کا خلاصہ:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   JSON کا خلاصہ فی لہر ، کھلی وصول کنندگان ، ای میل گنتی ، دعوت دیتا ہے ،
   رائے اور حالیہ واقعہ کا ٹائم اسٹیمپ۔ مددگار کی حمایت کی جاتی ہے
   [`scripts/preview-feedback-log.mjs`] (../../scripts/preview-feedback-log.mjs) ،
   لہذا وہی ورک فلو مقامی طور پر یا CI میں چلا سکتا ہے۔ ڈائجسٹ ٹیمپلیٹ میں استعمال کریں
   [`docs/examples/docs_preview_feedback_digest.md`] (../../../examples/docs_preview_feedback_digest.md)
   جب لہر کی بازیافت کی اشاعت کریں۔
2. ٹیلی میٹری ڈیش بورڈز کو `DOCS_RELEASE_TAG` کے ساتھ ٹیگ کریں تاکہ لہر میں استعمال کیا جاسکے
   چوٹیوں کو دعوت نامے کے ساتھ منسلک کیا جاسکتا ہے۔
3. اس کی تصدیق کے لئے تعینات کے بعد `npm run probe:portal -- --expect-release=<tag>` چلائیں
   پیش نظارہ ماحول صحیح رہائی میٹا ڈیٹا کا اعلان کرتا ہے۔
4. رن بک ٹیمپلیٹ میں کسی بھی واقعات کو ریکارڈ کریں اور کوہورٹ کو لنک کریں۔

## رائے اور بندش1 مشترکہ ڈی او سی یا ایشو بورڈ میں مجموعی آراء۔ آئٹمز کے ساتھ نشان زد کریں
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
   ``` آسانی سے تلاش کرنے کے لئے روڈ میپ مالکان کے لئے۔
2. لہر کی رپورٹ کو پُر کرنے کے لئے پیش نظارہ لاگر کے خلاصہ آؤٹ پٹ کا استعمال کریں ، پھر
   `status.md` (شرکاء ، کلیدی نتائج ، منصوبہ بند اصلاحات) اور میں شریک کا خلاصہ کریں
   `roadmap.md` کو اپ ڈیٹ کریں اگر سنگ میل DOCS-SORA میں تبدیلی آئی ہے۔
3. آف بورڈنگ مراحل پر عمل کریں
   [`reviewer-onboarding`](./reviewer-onboarding.md): revoke access, archive requests and
   شرکا کا شکریہ۔
4. نمونے کو اپ ڈیٹ کرکے اگلی لہر تیار کریں ، چیکسم کے گیٹس کو دوبارہ حاصل کریں ، اور
   نئی تاریخوں کے ساتھ دعوت نامے کے سانچے کو اپ ڈیٹ کرنا۔

اس پلے بوک کا اطلاق مستقل طور پر پیش نظارہ پروگرام کے قابل اظہار اور برقرار رکھتا ہے
پورٹل کے طور پر دعوت ناموں کو بڑھانے کے لئے دستاویزات/dewrel کو ایک تکرار کرنے والا راستہ دیتا ہے
جی اے کے قریب