---
lang: ja
direction: ltr
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/devportal/reviewer-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 72707241614e2e9cae0651e964677e31e567e4493725de8fbf5b7407a9d8d7cd
source_last_modified: "2025-11-14T04:43:20.137568+00:00"
translation_last_reviewed: 2026-01-30
---

# پریویو ریویور آن بورڈنگ

## جائزہ

DOCS-SORA ڈویلپر پورٹل کے مرحلہ وار لانچ کو ٹریک کرتا ہے۔ checksum-gated builds
(`npm run serve`) اور مضبوط Try it فلو اگلا سنگ میل کھولتے ہیں: پبلک پریویو کے وسیع پیمانے پر
کھلنے سے پہلے ویری فائیڈ ریویورز کی آن بورڈنگ۔ یہ گائیڈ بیان کرتا ہے کہ درخواستیں کیسے جمع کی جائیں،
اہلیت کیسے چیک کی جائے، رسائی کیسے دی جائے، اور شرکاء کو محفوظ طریقے سے آف بورڈ کیسے کیا جائے۔
کوهورٹ پلاننگ، دعوتی cadence، اور ٹیلی میٹری exports کے لئے
[preview invite flow](./preview-invite-flow.md) دیکھیں؛ نیچے کے مراحل اس پر فوکس کرتے ہیں
کہ ریویور منتخب ہونے کے بعد کون سی کارروائیاں کرنی ہیں۔

- **اسکوپ:** وہ ریویورز جنہیں GA سے پہلے docs preview (`docs-preview.sora`, GitHub Pages builds، یا SoraFS bundles) تک رسائی درکار ہے۔
- **آؤٹ آف اسکوپ:** Torii یا SoraFS آپریٹرز (اپنے onboarding kits میں کور) اور پروڈکشن پورٹل deployments (دیکھیں [`devportal/deploy-guide`](./deploy-guide.md))۔

## رولز اور پری ریکویزٹس

| رول | عام اہداف | مطلوبہ artifacts | نوٹس |
| --- | --- | --- | --- |
| Core maintainer | نئی گائیڈز ویری فائی کرنا، smoke tests چلانا۔ | GitHub handle، Matrix contact، signed CLA فائل پر۔ | عموما پہلے سے `docs-preview` GitHub ٹیم میں ہوتا ہے؛ پھر بھی درخواست درج کریں تاکہ رسائی auditable رہے۔ |
| Partner reviewer | پبلک ریلیز سے پہلے SDK snippets یا governance content ویری فائی کرنا۔ | Corporate email، legal POC، signed preview terms۔ | ٹیلی میٹری + data handling requirements کی منظوری لازم ہے۔ |
| Community volunteer | گائیڈز پر usability feedback دینا۔ | GitHub handle، preferred contact، timezone، CoC قبولیت۔ | کوہورٹس چھوٹے رکھیں؛ contributor agreement سائن کرنے والے ریویورز کو ترجیح دیں۔ |

تمام ریویور اقسام کو لازمی ہے کہ:

1. preview artifacts کے لئے acceptable-use پالیسی تسلیم کریں۔
2. security/observability appendices پڑھیں
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. لوکل snapshot serve کرنے سے پہلے `docs/portal/scripts/preview_verify.sh` چلانے پر رضامند ہوں۔

## Intake ورک فلو

1. درخواست دینے والے سے
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   فارم بھرنے کو کہیں (یا issue میں copy/paste کریں)۔ کم از کم یہ ریکارڈ کریں: شناخت، رابطے کا طریقہ،
   GitHub handle، مجوزہ review dates، اور security docs پڑھنے کی تصدیق۔
2. درخواست کو `docs-preview` tracker (GitHub issue یا governance ticket) میں درج کریں اور approver assign کریں۔
3. پری ریکویزٹس چیک کریں:
   - CLA / contributor agreement فائل پر موجود ہو (یا partner contract reference)۔
   - acceptable-use acknowledgement درخواست میں محفوظ ہو۔
   - risk assessment مکمل ہو (مثال: partner reviewers کو Legal نے approve کیا ہو)۔
4. Approver درخواست میں sign-off کرے اور tracking issue کو کسی بھی change-management entry سے لنک کرے
   (مثال: `DOCS-SORA-Preview-####`)۔

## Provisioning اور ٹولنگ

1. **Artifacts شیئر کریں** — CI workflow یا SoraFS pin سے تازہ ترین preview descriptor + archive فراہم کریں
   (`docs-portal-preview` artifact)۔ ریویورز کو یاد دلائیں کہ چلائیں:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Checksum enforcement کے ساتھ serve کریں** — ریویورز کو checksum-gated کمانڈ کی طرف ریفر کریں:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   یہ `scripts/serve-verified-preview.mjs` کو reuse کرتا ہے تاکہ کوئی unverified build غلطی سے لانچ نہ ہو۔

3. **GitHub رسائی دیں (اختیاری)** — اگر ریویورز کو unpublished branches چاہییں تو انہیں review مدت کے لئے
   `docs-preview` GitHub ٹیم میں شامل کریں اور membership تبدیلی درخواست میں درج کریں۔

4. **Support channels شیئر کریں** — on-call contact (Matrix/Slack) اور incident procedure
   [`incident-runbooks`](./incident-runbooks.md) سے شیئر کریں۔

5. **Telemetry + feedback** — ریویورز کو یاد دلائیں کہ anonymized analytics جمع کی جاتی ہے
   (دیکھیں [`observability`](./observability.md))۔ دعوت میں دیے گئے feedback form یا issue template فراہم کریں
   اور event کو [`preview-feedback-log`](./preview-feedback-log) helper سے لاگ کریں تاکہ wave summary اپ ٹو ڈیٹ رہے۔

## ریویور چیک لسٹ

پریویو تک رسائی سے پہلے ریویورز کو یہ مکمل کرنا ہوگا:

1. downloaded artifacts verify کریں (`preview_verify.sh`)۔
2. `npm run serve` (یا `serve:verified`) سے پورٹل لانچ کریں تاکہ checksum guard فعال ہو۔
3. اوپر لنک کردہ security اور observability نوٹس پڑھیں۔
4. OAuth/Try it console کو device-code login کے ذریعے ٹیسٹ کریں (اگر applicable ہو) اور production tokens دوبارہ استعمال نہ کریں۔
5. agreed tracker میں findings درج کریں (issue، shared doc یا فارم) اور انہیں preview release tag سے tag کریں۔

## Maintainer ذمہ داریاں اور offboarding

| مرحلہ | اقدامات |
| --- | --- |
| Kickoff | یقینی بنائیں کہ intake checklist درخواست کے ساتھ منسلک ہے، artifacts + ہدایات شیئر کریں، [`preview-feedback-log`](./preview-feedback-log) کے ذریعے `invite-sent` انٹری شامل کریں، اور اگر review ایک ہفتے سے زیادہ ہو تو midpoint sync شیڈول کریں۔ |
| Monitoring | preview telemetry مانیٹر کریں (غیر معمولی Try it ٹریفک، probe failures) اور اگر کوئی مشکوک چیز ہو تو incident runbook فالو کریں۔ findings آنے پر `feedback-submitted`/`issue-opened` events لاگ کریں تاکہ wave metrics درست رہیں۔ |
| Offboarding | عارضی GitHub یا SoraFS رسائی واپس لیں، `access-revoked` درج کریں، درخواست archive کریں (feedback summary + outstanding actions شامل کریں)، اور reviewer registry اپ ڈیٹ کریں۔ ریویور سے مقامی builds صاف کرنے کو کہیں اور [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md) سے بنا digest منسلک کریں۔ |

ریویورز کو waves کے درمیان rotate کرتے وقت بھی یہی عمل استعمال کریں۔ repo میں paper trail
(issue + templates) برقرار رکھنے سے DOCS-SORA auditable رہتا ہے اور governance کو تصدیق میں مدد ملتی ہے
کہ preview access documented controls کے مطابق تھا۔

## Invite templates اور tracking

- ہر outreach کی شروعات
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  فائل سے کریں۔ یہ کم از کم قانونی زبان، preview checksum ہدایات، اور اس توقع کو شامل کرتی ہے کہ
  ریویورز acceptable-use پالیسی تسلیم کریں۔
- template ایڈٹ کرتے وقت `<preview_tag>`, `<request_ticket>` اور contact channels placeholders بدلیں۔
  final message کی ایک کاپی intake ticket میں محفوظ کریں تاکہ ریویورز، approvers اور auditors
  بھیجے گئے الفاظ کا حوالہ دے سکیں۔
- دعوت بھیجنے کے بعد tracking spreadsheet یا issue کو `invite_sent_at` timestamp اور متوقع end date کے ساتھ
  اپ ڈیٹ کریں تاکہ [preview invite flow](./preview-invite-flow.md) رپورٹ خودکار طور پر cohort اٹھا سکے۔
