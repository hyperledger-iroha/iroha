---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: پیش نظارہ-فیڈ بیک-ڈبلیو 1-پلان
عنوان: W1 شراکت داروں سے پہلے سے وابستہ منصوبہ
سائڈبار_لیبل: W1 منصوبہ
تفصیل: پارٹنر پیش نظارہ گروپ کے لئے کام ، مالکان اور ڈائریکٹری کی فہرست۔
---

| آئٹم | تفصیلات |
| --- | --- |
| لہر | W1 - شراکت دار اور انٹیگریٹرز Torii |
| ہدف ونڈو سیکنڈ کوارٹر 2025 ہفتہ 3 |
| اثر ٹیگ (ڈایاگرام) | `preview-2025-04-12` |
| ٹریکر ٹکٹ | `DOCS-SORA-Preview-W1` |

## مقاصد

1. شراکت داروں کے معائنے کے حالات کے لئے قانونی اور حکمرانی کی منظوری حاصل کرنا۔
2. دعوت پیکیج میں استعمال ہونے والے آئی ٹی ایجنٹ اور پیمائش کے اسنیپ شاٹس تیار کریں۔
3. چیکسم اور تحقیقات کے نتائج کے ذریعہ حاصل کردہ معائنہ کے سراغ کو اپ ڈیٹ کرنا۔
4. دعوت نامے بھیجنے سے پہلے شراکت داروں اور درخواست ٹیمپلیٹس کی فہرست کو حتمی شکل دیں۔

## کاموں کی تفصیل

| ID | ٹاسک | مالک | میرٹ | حیثیت | نوٹ |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | معائنہ کی شرائط و ضوابط کے لئے قانونی منظوری حاصل کرنا دستاویزات/ڈیوریل لیڈ -> قانونی | 2025-04-05 | ✅ مکمل | قانونی ٹکٹ `DOCS-SORA-Preview-W1-Legal` کو 2025-04-05 پر منظور کیا گیا تھا۔ پی ڈی ایف فائل ٹریکر سے منسلک ہے۔ |
| W1-P2 | کسی ایجنٹ کے لئے ایک اسٹیجنگ ونڈو محفوظ کریں (2025-04-10) کو آزمائیں اور ایجنٹ کی توثیق کریں دستاویزات/ڈیوریل + اوپس | 2025-04-06 | ✅ مکمل | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target Grafana` 2025-04-06 پر عمل درآمد ؛ CLI لاگ `.env.tryit-proxy.bak` آرکائو کیا گیا ہے۔ |
| W1-P3 | پیش نظارہ ٹریس (`preview-2025-04-12`) بنائیں ، `scripts/preview_verify.sh` + `npm run probe:portal` ، آرکائیو ڈسکرپٹر/چیکمس | پورٹل TL | 2025-04-08 | ✅ مکمل | ٹریس اور تصدیق کے نوشتہ جات `artifacts/docs_preview/W1/preview-2025-04-12/` کے تحت محفوظ کیے گئے ہیں۔ تحقیقات کے نتائج ٹریکر کے ساتھ منسلک ہوتے ہیں۔ |
| W1-P4 | شراکت داروں کے لئے انٹیک فارم کا جائزہ لیں (`DOCS-SORA-Preview-REQ-P01...P08`) ، رابطوں کی تصدیق کریں اور این ڈی اے | گورننس رابطہ | 2025-04-07 | ✅ مکمل | تمام آٹھ درخواستوں کی منظوری دی گئی (آخری دو اپریل 11 ، 2025 کو) ؛ لنکس ٹریکر میں ہیں۔ |
| W1-P5 | ایک دعوت نامہ (`docs/examples/docs_preview_invite_template.md` پر مبنی) ، ہر ساتھی کے لئے `<preview_tag>` اور `<request_ticket>` ترتیب دیں | دستاویزات/ڈیوریل لیڈ | 2025-04-08 | ✅ مکمل | ڈرافٹ دعوت نامہ 2025-04-12 15:00 UTC کو ٹریل لنکس کے ساتھ بھیجا گیا ہے۔ |

## پری لانچ چیک لسٹ

> اشارہ: خود بخود 1-5 (بلڈ ، چیکسم ، گیٹ وے کی تحقیقات ، لنک چیکر ، اپ ڈیٹ ایجنٹ اور آزمائیں) کو خود بخود انجام دینے کے لئے `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` چلائیں۔ اسکرپٹ میں JSON ریکارڈ ریکارڈ کیا گیا ہے جسے آپ ٹریکر ٹکٹ سے منسلک کرسکتے ہیں۔

1. `npm run build` (`DOCS_RELEASE_TAG=preview-2025-04-12` کے ساتھ) `build/checksums.sha256` اور `build/release.json` کو دوبارہ تخلیق کرنے کے لئے۔
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3.`PORTAL_BASE_URL=SORA DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`۔
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` اور آرکائیو `build/link-report.json` ڈسکرپٹر کے آگے۔
5. `npm run manage:tryit-proxy -- update --target SORA` (یا `--tryit-target` کے ذریعے مناسب ہدف پاس کریں) ؛ `.env.tryit-proxy` پر اپ ڈیٹ انسٹال کریں اور فال بیک کے لئے `.bak` رکھیں۔
6. لاگ راہوں کے ساتھ W1 ٹکٹ کو اپ ڈیٹ کریں (ڈسکرپٹر چیکسم ، تحقیقات کے نتائج ، ایجنٹ کو تبدیل کریں ، اور اسنیپ شاٹس Grafana)۔

## ثبوت کی فہرست

- [x] دستخط شدہ قانونی رضامندی (پی ڈی ایف یا ٹکٹ لنک) `DOCS-SORA-Preview-W1` سے منسلک ہے۔
- [x] Torii ، `TryItProxyErrors` ، `DocsPortal/GatewayRefusals` کے لئے Grafana اسنیپ شاٹس۔
- [x] `preview-2025-04-12` کے لئے ڈسکرپٹر اور چیکم لاگ کو `artifacts/docs_preview/W1/` کے تحت محفوظ کیا گیا ہے۔
- [x] فیلڈز کے ساتھ دعوت نامہ روسٹر ٹیبل `invite_sent_at` آبادی (ٹریکر میں W1 ریکارڈ دیکھیں)۔
۔

اس منصوبے کو کاموں کی پیشرفت کے ساتھ اپ ڈیٹ کریں۔ ٹریکر سے روڈ میپ کی آڈیٹیبلٹی کو برقرار رکھنے کے لئے ان سے مراد ہے۔## آراء ورک فلو

1. ہر جائزہ لینے والے کے لئے ، ٹیمپلیٹ کو کاپی کریں
   [`docs/examples/docs_preview_feedback_form.md`] (../../../../../examples/docs_preview_feedback_form.md) ،
   میٹا ڈیٹا کو پُر کریں ، اور ذیل میں مکمل ورژن کو محفوظ کریں
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`۔
2. براہ راست لاگ میں کالوں ، پیمائش کے نکات اور کھلے مسائل کا خلاصہ کریں
   [`preview-feedback/w1/log.md`] (./log.md) تاکہ گورننس کے جائزہ لینے والے پوری لہر کو دوبارہ شروع کرسکیں
   گودام چھوڑنے کے بغیر۔
3. جب علم کی برآمدات یا سوالنامے آتے ہیں تو ، انہیں ریکارڈ میں مذکور ٹریل سے منسلک کریں
   اور ٹریکر ٹکٹ کو لنک کریں۔