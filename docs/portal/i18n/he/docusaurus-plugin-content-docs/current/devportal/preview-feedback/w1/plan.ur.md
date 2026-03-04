---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w1-plan
title: W1 شراکت داروں کے لئے پری فلائٹ پلان
sidebar_label: W1 پلان
description: پارٹنر preview کوہوٹ کے لئے ٹاسکس، مالکان، اور ثبوت چیک لسٹ۔
---

| آئٹم | تفصیل |
| --- | --- |
| לרי | W1 - پارٹنرز اور Torii integrators |
| ہدف ونڈو | Q2 2025 ہفتہ 3 |
| آرٹیفیکٹ ٹیگ (منصوبہ) | `preview-2025-04-12` |
| ٹریکر ایشو | `DOCS-SORA-Preview-W1` |

## מידע

1. پارٹنر preview شرائط کے لئے قانونی اور گورننس منظوری حاصل کرنا۔
2. دعوتی بنڈل میں استعمال ہونے والے Try it proxy اور telemetry snapshots تیار کرنا۔
3. checksum سے verify شدہ preview artefact اور probe نتائج تازہ کرنا۔
4. دعوتیں بھیجنے سے پہلے پارٹنر roster اور request templates حتمی کرنا۔

## שגיאה

| תעודת זהות | ٹاسک | مالک | مقررہ تاریخ | اسٹیٹس | نوٹس |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | preview terms addendum کے لئے قانونی منظوری حاصل کرنا | Docs/DevRel lead -> משפטי | 2025-04-05 | ✅ מקל | قانونی ٹکٹ `DOCS-SORA-Preview-W1-Legal` 2025-04-05 کو منظور ہوا؛ PDF ٹریکر کے ساتھ منسلک ہے۔ |
| W1-P2 | Try it proxy کا staging ونڈو (2025-04-10) محفوظ کرنا اور proxy health کی تصدیق | Docs/DevRel + Ops | 2025-04-06 | ✅ מקל | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` 2025-04-06 CLI transcript اور `.env.tryit-proxy.bak` محفوظ کر دیے گئے۔ |
| W1-P3 | preview artefact (`preview-2025-04-12`) بنانا، `scripts/preview_verify.sh` + `npm run probe:portal` چلانا، descriptor/checksums محفوظ کرنا | פורטל TL | 2025-04-08 | ✅ מקל | artefact اور verification logs `artifacts/docs_preview/W1/preview-2025-04-12/` میں محفوظ ہیں؛ probe output ٹریکر کے ساتھ منسلک ہے۔ |
| W1-P4 | پارٹنر intake forms (`DOCS-SORA-Preview-REQ-P01...P08`) کا جائزہ، contacts اور NDAs کی تصدیق | קשר ממשל | 2025-04-07 | ✅ מקל | تمام آٹھ درخواستیں منظور ہوئیں (آخری دو 2025-04-11 کو منظور ہوئیں)؛ approvals ٹریکر میں لنک ہیں۔ |
| W1-P5 | دعوتی متن تیار کرنا (`docs/examples/docs_preview_invite_template.md` پر مبنی)، ہر پارٹنر کے لئے `<preview_tag>` اور `<request_ticket>` سیٹ קרנה | Docs/DevRel lead | 2025-04-08 | ✅ מקל | دعوت کا مسودہ 2025-04-12 15:00 UTC کو artefact لنکس کے ساتھ بھیجا گیا۔ |

## טיסה מוקדמת

> דף: `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` פרק 1-5 פרק 1-5 פרק קוד (build, checksum אימות, checker it proxy). اس اسکرپٹ میں JSON لاگ بنتا ہے جسے ٹریکر ایشو کے ساتھ منسلک کیا جا سکتا ہے۔

1. `npm run build` (`DOCS_RELEASE_TAG=preview-2025-04-12` کے ساتھ) تاکہ `build/checksums.sha256` اور `build/release.json` دوبارہ بنیں۔
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` اور `build/link-report.json` کو descriptor کے ساتھ archive کریں۔
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (یا مناسب target `--tryit-target` سے دیں)؛ اپ ڈیٹ شدہ `.env.tryit-proxy` کو commit کریں اور rollback کے لئے `.bak` رکھیں۔
6. W1 ٹریکر ایشو کو log paths کے ساتھ اپ ڈیٹ کریں (descriptor checksum، probe output، Try it proxy تبدیلی، اور Grafana snapshots)۔

## ثبوت چیک لسٹ- [x] دستخط شدہ قانونی منظوری (PDF یا ٹکٹ لنک) `DOCS-SORA-Preview-W1` کے ساتھ منسلک ہے۔
- [x] `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` کے Grafana screenshots۔
- [x] `preview-2025-04-12` descriptor اور checksum log `artifacts/docs_preview/W1/` کے تحت محفوظ ہیں۔
- [x] دعوت roster table میں `invite_sent_at` timestamps مکمل ہیں (ٹریکر W1 log دیکھیں)۔
- [x] feedback artefacts [`preview-feedback/w1/log.md`](./log.md) میں نظر آتے ہیں، ہر پارٹنر کے لئے ایک row (2025-04-26 کو roster/telemetria/issues ڈیٹا کے ساتھ اپ ڈیٹ)۔

جوں جوں کام آگے بڑھے یہ پلان اپ ڈیٹ کریں؛ ٹریکر اسے roadmap کی auditability برقرار رکھنے کے لئے حوالہ دیتا ہے۔

## فیڈبیک ورک فلو

1. ہر reviewer کے لئے
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md) کا template کاپی کریں،
   میٹا ڈیٹا بھریں اور مکمل کاپی
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/` میں رکھیں۔
2. הזמנות, מחסומי טלמטריה, או בעיות פתוחות
   [`preview-feedback/w1/log.md`](./log.md) کے live log میں خلاصہ کریں تاکہ governance reviewers پوری لہر کو
   repository کے اندر ہی replay کر سکیں۔
.3
   اور tracker issue سے link کریں۔