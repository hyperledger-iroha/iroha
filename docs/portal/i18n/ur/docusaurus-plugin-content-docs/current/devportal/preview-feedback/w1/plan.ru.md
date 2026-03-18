---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: پیش نظارہ-فیڈ بیک-ڈبلیو 1-پلان
عنوان: W1 شراکت داروں کے لئے پری لائٹ پلان
سائڈبار_لیبل: منصوبہ W1
تفصیل: پارٹنر پیش نظارہ کوہورٹ کے لئے کام ، مالکان اور شواہد کی چیک لسٹ۔
---

| آئٹم | تفصیلات |
| --- | --- |
| لہر | W1 - شراکت دار اور انٹیگریٹرز Torii |
| ہدف ونڈو | Q2 2025 ہفتہ 3 |
| نمونہ ٹیگ (منصوبہ) | `preview-2025-04-12` |
| ٹریکر | `DOCS-SORA-Preview-W1` |

## اہداف

1. پارٹنر پیش نظارہ کی شرائط کی قانونی اور حکمرانی کی منظوری حاصل کریں۔
2. دعوت پیکیج کے ل it اسے پراکسی اور ٹیلی میٹری کی تصاویر کی تیاری کریں۔
3. چیکسم کی تصدیق شدہ پیش نظارہ نمونہ اور تحقیقات کے نتائج کو اپ ڈیٹ کریں۔
4. دعوت نامے بھیجنے سے پہلے شراکت داروں کی فہرست کو حتمی شکل دیں اور ٹیمپلیٹس کی درخواست کریں۔

## ٹاسک خرابی

| ID | مسئلہ | مالک | آخری تاریخ | حیثیت | نوٹ |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | پیش نظارہ کی شرائط کو شامل کرنے کے لئے قانونی منظوری حاصل کریں | دستاویزات/ڈیوریل لیڈ -> قانونی | 2025-04-05 | ✅ مکمل | قانونی ٹکٹ `DOCS-SORA-Preview-W1-Legal` نے 2025-04-05 کی منظوری دی۔ پی ڈی ایف ٹریکر کے ساتھ منسلک ہے۔ |
| W1-P2 | اسٹیجنگ ونڈو کو منجمد کریں اسے پراکسی (2025-04-10) آزمائیں اور پراکسی کی صحت چیک کریں | دستاویزات/ڈیوریل + اوپس | 2025-04-06 | ✅ مکمل | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target Grafana` مکمل 2025-04-06 ؛ ٹرانسکرپٹ CLI اور `.env.tryit-proxy.bak` آرکائیوڈ۔ |
| W1-P3 | پیش نظارہ نمونہ جمع کریں (`preview-2025-04-12`) ، `scripts/preview_verify.sh` + `npm run probe:portal` ، آرکائیو ڈسکرپٹر/چیکسم | پورٹل TL | 2025-04-08 | ✅ مکمل | نمونے اور توثیق کے نوشتہ جات `artifacts/docs_preview/W1/preview-2025-04-12/` میں محفوظ کیے گئے ہیں۔ تحقیقات کا پن ٹریکر کے ساتھ منسلک ہے۔ |
| W1-P4 | پارٹنر انٹیک فارم (`DOCS-SORA-Preview-REQ-P01...P08`) چیک کریں ، رابطوں کی تصدیق کریں اور این ڈی اے | گورننس رابطہ | 2025-04-07 | ✅ مکمل | تمام آٹھ درخواستوں کی منظوری دی گئی (آخری دو 2025-04-11) ؛ ٹریکر میں منظوری کے لنکس۔ |
| W1-P5 | دعوت نامہ تیار کریں (`docs/examples/docs_preview_invite_template.md` پر مبنی) ، `<preview_tag>` اور `<request_ticket>` کو ہر ساتھی کے لئے سیٹ کریں | دستاویزات/ڈیوریل لیڈ | 2025-04-08 | ✅ مکمل | مسودہ دعوت نامہ 2025-04-12 15:00 UTC کے ساتھ ساتھ نمونے کے لنکس بھیجا۔ |

## پریفل لائٹ چیک لسٹ

> اشارہ: `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` کو خود بخود 1-5 انجام دینے کے لئے چلائیں (بلڈ ، چیکسم ، پورٹل تحقیقات ، لنک چیکر اور اس پر پراکسی اپ ڈیٹ کو آزمائیں)۔ اسکرپٹ ایک JSON لاگ لکھتا ہے جو ایشو ٹریکر کے ساتھ منسلک ہوسکتا ہے۔

1. `npm run build` (`DOCS_RELEASE_TAG=preview-2025-04-12` سے) `build/checksums.sha256` اور `build/release.json` کو دوبارہ تیار کرنے کے لئے۔
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` اور آرکائیو `build/link-report.json` ڈسکرپٹر کے آگے۔
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (یا `--tryit-target` کے ذریعے مطلوبہ ہدف کی وضاحت کریں) ؛ تازہ ترین `.env.tryit-proxy` کا ارتکاب کریں اور رول بیک کے لئے `.bak` کو بچائیں۔
6. لاگ ان راستوں کے ساتھ ڈبلیو 1 کو اپ ڈیٹ کریں (چیکم ڈسکرپٹر ، تحقیقات آؤٹ پٹ ، تبدیلی کو پراکسی اور Grafana ایکس اسنیپ شاٹس کو تبدیل کریں)۔

## ثبوت چیک لسٹ- [x] `DOCS-SORA-Preview-W1` سے منسلک قانونی منظوری (پی ڈی ایف یا ٹکٹ لنک) پر دستخط شدہ۔
- [x] Grafana اسکرین شاٹس برائے `docs.preview.integrity` ، `TryItProxyErrors` ، `DocsPortal/GatewayRefusals`۔
- [x] وضاحتی اور چیکم لاگ `preview-2025-04-12` `artifacts/docs_preview/W1/` میں محفوظ کیا گیا ہے۔
- [x] مکمل `invite_sent_at` کے ساتھ دعوت نامہ روسٹر ٹیبل (ٹریکر میں W1 لاگ دیکھیں)۔
۔

جاتے وقت اس منصوبے کو اپ ڈیٹ کریں۔ ٹریکر آڈیٹیبلٹی روڈ میپ کو برقرار رکھنے کے لئے اس کا حوالہ دیتا ہے۔

## رائے کا عمل

1. ہر جائزہ لینے والے کے لئے ٹیمپلیٹ کو نقل کریں
   [`docs/examples/docs_preview_feedback_form.md`] (../../../../../examples/docs_preview_feedback_form.md) ،
   میٹا ڈیٹا کو پُر کریں اور تیار شدہ کاپی کو محفوظ کریں
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`۔
2. دعوت نامے ، ٹیلی میٹری چوکیوں کو یکجا کریں اور براہ راست لاگ میں مسائل کھولیں
   [`preview-feedback/w1/log.md`](./log.md) so that governance reviewers can reproduce the wave
   مکمل طور پر ، ذخیرہ چھوڑنے کے بغیر۔
3۔ جب علم کی جانچ پڑتال یا سروے کی برآمدات آتی ہیں تو ، انہیں لاگ میں متعین کردہ نمونے والے راستے سے منسلک کریں ،
   اور ایشو ٹریکر سے لنک کریں۔