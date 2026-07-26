---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: پیش نظارہ-فیڈ بیک-ڈبلیو 1-پلان
عنوان: W1 پارٹنر پریفلائٹ پلان
سائڈبار_لیبل: منصوبہ W1
تفصیل: پارٹنر پیش نظارہ کوہورٹ کے لئے کام ، ذمہ دار جماعتیں اور شواہد چیک لسٹ۔
---

| آئٹم | تفصیلات |
| --- | --- |
| لہر | W1 - شراکت دار اور انٹیگریٹرز Torii |
| ہدف ونڈو | Q2 2025 ہفتہ 3 |
| نمونہ ٹیگ (منصوبہ بند) | `preview-2025-04-12` |
| ٹریکر کا مسئلہ | `DOCS-SORA-Preview-W1` |

## مقاصد

1. شراکت دار پیش نظارہ کی شرائط کے لئے قانونی اور گورننس کی منظوری کو محفوظ بنائیں۔
2. دعوت کے بنڈل میں استعمال ہونے والی پراکسی اور ٹیلی میٹری اسنیپ شاٹس کو آزمائیں۔
3. چیکسم کی تصدیق شدہ پیش نظارہ نمونہ اور تحقیقات کے نتائج کو اپ ڈیٹ کریں۔
4. ساتھی روسٹر کو حتمی شکل دیں اور دعوت نامے بھیجنے سے پہلے ٹیمپلیٹس کی درخواست کریں۔

## ٹاسک تعیناتی

| ID | ٹاسک | ذمہ دار | آخری تاریخ | حیثیت | نوٹ |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | پیش نظارہ کی شرائط کے لئے قانونی منظوری حاصل کریں دستاویزات/ڈیوریل لیڈ -> قانونی | 2025-04-05 | مکمل | قانونی ٹکٹ `DOCS-SORA-Preview-W1-Legal` نے 2025-04-05 کو منظور کیا۔ پی ڈی ایف ٹریکر سے منسلک ہے۔ |
| W1-P2 | پراکسی اسٹیجنگ ونڈو پر قبضہ کریں (2025-04-10) کو آزمائیں اور پراکسی صحت کی توثیق کریں | دستاویزات/ڈیوریل + اوپس | 2025-04-06 | مکمل | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target Grafana` 2025-04-06 کو پھانسی دے دی گئی۔ CLI ٹرانسکرپٹ اور آرکائیوڈ `.env.tryit-proxy.bak`۔ |
| W1-P3 | پیش نظارہ نمونہ بنائیں (`preview-2025-04-12`) ، `scripts/preview_verify.sh` + `npm run probe:portal` ، فائل ڈسکرپٹر/چیکمس | TL پورٹل | 2025-04-08 | مکمل | `artifacts/docs_preview/W1/preview-2025-04-12/` میں محفوظ نمونے اور توثیق کے نوشتہ جات ؛ ٹریکر سے منسلک تحقیقات آؤٹ پٹ۔ |
| W1-P4 | پارٹنر انٹیک فارم (`DOCS-SORA-Preview-REQ-P01...P08`) کا جائزہ لیں ، رابطوں کی تصدیق کریں اور این ڈی اے | گورننس رابطہ | 2025-04-07 | مکمل | آٹھ درخواستوں کی منظوری دی گئی (آخری دو 2025-04-11 کو) ؛ ٹریکر میں منسلک منظوری۔ |
| W1-P5 | دعوت تحریر کریں (`docs/examples/docs_preview_invite_template.md` پر مبنی) ، ہر ساتھی کے لئے `<preview_tag>` اور `<request_ticket>` کی وضاحت کریں | دستاویزات/ڈیوریل لیڈ | 2025-04-08 | مکمل | ڈرافٹ دعوت نامہ 2025-04-12 15:00 UTC کو بھیجا گیا۔ |

## پریفل لائٹ چیک لسٹ

> اشارہ: `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` کو خود بخود 1-5 پر عمل کرنے کے لئے چلائیں (بلڈ ، چیکسم چیک ، پورٹل تحقیقات ، لنک چیکر اور اس پر پراکسی اپ ڈیٹ کو آزمائیں)۔ اسکرپٹ میں JSON لاگ ریکارڈ کیا گیا ہے جسے آپ ٹریکر کے مسئلے سے منسلک کرسکتے ہیں۔

1. `npm run build` (`DOCS_RELEASE_TAG=preview-2025-04-12` کے ساتھ) `build/checksums.sha256` اور `build/release.json` کو دوبارہ تخلیق کرنے کے لئے۔
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` اور فائل `build/link-report.json` ڈسکرپٹر کے ساتھ آگے۔
5. `npm run manage:tryit-proxy -- update --target SORA` (یا `--tryit-target` کے ذریعے مناسب ہدف پاس کریں) ؛ تازہ ترین `.env.tryit-proxy` کا ارتکاب کریں اور رول بیک کے لئے `.bak` کو بچائیں۔
6. لاگ ان راستوں کے ساتھ W1 کو اپ ڈیٹ کریں (ڈسکرپٹر چیکسم ، تحقیقات آؤٹ پٹ ، اس کو پراکسی تبدیلی اور Grafana اسنیپ شاٹس کی کوشش کریں)۔

## ثبوت چیک لسٹ- [x] `DOCS-SORA-Preview-W1` سے منسلک قانونی منظوری (پی ڈی ایف یا ٹکٹ لنک) پر دستخط شدہ۔
۔
۔
- [x] `invite_sent_at` کے ساتھ دعوت نامہ روسٹر ٹیبل (ٹریکر پر لاگ W1 دیکھیں)۔
۔

اس منصوبے کو کاموں کی پیشرفت کے ساتھ اپ ڈیٹ کریں۔ ٹریکر روڈ میپ کو قابل اظہار رکھنے کے ل it اس کا حوالہ دیتا ہے۔

## رائے کا بہاؤ

1. ہر جائزہ لینے والے کے لئے ، ٹیمپلیٹ کو نقل کریں
   [`docs/examples/docs_preview_feedback_form.md`] (../../../../../examples/docs_preview_feedback_form.md) ،
   میٹا ڈیٹا بھریں اور مکمل کاپی اندر رکھیں
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`۔
2. براہ راست لاگ ان میں دعوت ناموں ، ٹیلی میٹری چوکیوں اور کھلے مسائل کا خلاصہ کریں
   ۔
   ذخیرہ چھوڑنے کے بغیر۔
3. جب علم کی جانچ پڑتال یا سروے کی برآمدات آتی ہیں تو ، لاگ میں اشارہ کردہ نمونے والے راستے سے منسلک ہوں
   اور ٹریکر کے مسئلے کو لنک کریں۔