---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: پیش نظارہ-فیڈ بیک-ڈبلیو 1-پلان
عنوان: W1 پارٹنر پریفلائٹ پلان
سائڈبار_لیبل: منصوبہ W1
تفصیل: پارٹنر پیش نظارہ کوہورٹ کے لئے کام ، دیکھ بھال کرنے والے اور پروف چیک لسٹ۔
---

| عنصر | تفصیلات |
| --- | --- |
| لہر | W1 - شراکت دار اور انٹیگریٹرز Torii |
| ہدف ونڈو | Q2 2025 ہفتہ 3 |
| نمونہ ٹیگ (منصوبے) | `preview-2025-04-12` |
| ٹریکر جاری کریں | `DOCS-SORA-Preview-W1` |

## مقاصد

1. پارٹنر پیش نظارہ کی شرائط کے لئے قانونی اور گورننس کی منظوری حاصل کریں۔
2. دعوت کے بنڈل میں استعمال ہونے والی پراکسی اور ٹیلی میٹری اسنیپ شاٹس کو آزمائیں۔
3. چیکسم اور تحقیقات کے نتائج کے ذریعہ تصدیق شدہ پیش نظارہ نمونہ کو تازہ کریں۔
4. ساتھی روسٹر کو حتمی شکل دیں اور دعوت نامے بھیجنے سے پہلے ٹیمپلیٹس کی درخواست کریں۔

## دھبوں کو کاٹنا

| ID | داغ | ذمہ دار | آخری تاریخ | حیثیت | نوٹ |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | پیش نظارہ کی شرائط کے لئے قانونی منظوری حاصل کریں دستاویزات/ڈیوریل لیڈ -> قانونی | 2025-04-05 | مکمل | قانونی ٹکٹ `DOCS-SORA-Preview-W1-Legal` 2025-04-05 پر درست ؛ پی ڈی ایف ٹریکر سے منسلک ہوتا ہے۔ |
| W1-P2 | پراکسی اسٹیجنگ ونڈو (2025-04-10) کی کوشش کریں اور پراکسی کی صحت کی توثیق کریں | دستاویزات/ڈیوریل + اوپس | 2025-04-06 | مکمل | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target Grafana` 2025-04-06 کو پھانسی دے دی گئی۔ CLI ٹرانسکرپشن + `.env.tryit-proxy.bak` آرکائیو۔ |
| W1-P3 | پیش نظارہ نمونہ بنائیں (`preview-2025-04-12`) ، `scripts/preview_verify.sh` + `npm run probe:portal` ، آرکائو ڈسکرپٹر/چیکمس | TL پورٹل | 2025-04-08 | مکمل | آرٹیکٹیکٹ + توثیق لاگز `artifacts/docs_preview/W1/preview-2025-04-12/` کے تحت محفوظ ہیں۔ ٹریکر سے منسلک تحقیقات آؤٹ پٹ۔ |
| W1-P4 | جائزہ پارٹنر انٹیک فارم (`DOCS-SORA-Preview-REQ-P01...P08`) ، رابطوں کی تصدیق + NDAS | رابطہ گورننس | 2025-04-07 | مکمل | آٹھ درخواستوں کی منظوری دی گئی (آخری دو 2025-04-11 کو) ؛ ٹریکر میں منسلک منظوری۔ |
| W1-P5 | دعوت نامہ لکھیں (`docs/examples/docs_preview_invite_template.md` پر مبنی) ، ہر ساتھی کے لئے `<preview_tag>` اور `<request_ticket>` کی وضاحت کریں | دستاویزات/ڈیوریل لیڈ | 2025-04-08 | مکمل | ڈرافٹ دعوت نامہ 2025-04-12 15:00 UTC کو نمونہ لنکس کے ساتھ بھیجا گیا۔ |

## پریفل لائٹ چیک لسٹ

> اشارہ: `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` کو خود بخود 1-5 پر عمل کرنے کے لئے چلائیں (بلڈ ، توثیق چیکسم ، پورٹل تحقیقات ، لنک چیکر ، اور اس کو پراکسی اپ ڈیٹ کرنے کی کوشش کریں)۔ اسکرپٹ میں JSON لاگ کو ٹریکر سے منسلک کرنے کے لئے ریکارڈ کیا گیا ہے۔

1. `npm run build` (`DOCS_RELEASE_TAG=preview-2025-04-12` کے ساتھ) `build/checksums.sha256` اور `build/release.json` کو دوبارہ تخلیق کرنے کے لئے۔
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` اور آرکائیو `build/link-report.json` ڈسکرپٹر کے آگے۔
5. `npm run manage:tryit-proxy -- update --target SORA` (یا `--tryit-target` کے ذریعے مناسب ہدف فراہم کریں) ؛ تازہ ترین `.env.tryit-proxy` کا ارتکاب کریں اور `.bak` کو رول بیک کے لئے رکھیں۔
6. لاگ ان راستوں کے ساتھ W1 کے مسئلے کو اپ ڈیٹ کریں (وضاحتی چیکسم ، تحقیقات آؤٹ پٹ ، اس کو پراکسی تبدیلی اور Grafana اسنیپ شاٹس) کی کوشش کریں)۔

## پروف چیک لسٹ- [x] `DOCS-SORA-Preview-W1` سے منسلک قانونی منظوری (پی ڈی ایف یا ٹکٹ لنک) پر دستخط شدہ۔
۔
۔
- [x] `invite_sent_at` معلومات کے ساتھ دعوت نامہ روسٹر ٹیبل (ٹریکر لاگ W1 دیکھیں)۔
۔

پیشرفت کے ساتھ ساتھ اس منصوبے کو اپ ڈیٹ کریں۔ ٹریکر اس سے مراد روڈ میپ کو قابل اظہار رکھنے کے ل. ہے۔

## رائے کا بہاؤ

1. ہر جائزہ لینے والے کے لئے ، ٹیمپلیٹ کو نقل کریں
   [`docs/examples/docs_preview_feedback_form.md`] (../../../../../examples/docs_preview_feedback_form.md) ،
   میٹا ڈیٹا کو پُر کریں اور مکمل کاپی کو بطور اسٹور کریں
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`۔
2. دعوت نامے ، ٹیلی میٹری چوکیوں اور زندہ لاگ میں کھلے مسائل کا خلاصہ کریں
   [`preview-feedback/w1/log.md`](./log.md) so that governance reviewers can replay the wave
   ڈپو چھوڑنے کے بغیر۔
3. جب علم کی جانچ پڑتال یا سروے کی برآمدات آتی ہیں تو ، انہیں لاگ میں آرٹیکٹیکٹ پاتھ نوٹ کے ساتھ منسلک کریں
   اور ٹریکر کے نتائج کو لنک کریں۔