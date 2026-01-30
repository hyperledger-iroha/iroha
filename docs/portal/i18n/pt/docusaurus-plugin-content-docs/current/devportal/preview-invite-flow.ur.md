---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# پریویو دعوتی فلو

## مقصد

روڈ میپ آئٹم **DOCS-SORA** ریویور آن بورڈنگ اور پبلک پریویو دعوتی پروگرام کو وہ آخری رکاوٹیں قرار دیتا ہے جن کے بعد پورٹل بیٹا سے باہر جا سکتا ہے۔ یہ صفحہ بیان کرتا ہے کہ ہر دعوتی ویو کیسے کھولی جائے، کون سے artifacts دعوتیں بھیجنے سے پہلے لازمی ہیں، اور فلو کی auditability کیسے ثابت کی جائے۔ اسے ساتھ استعمال کریں:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) ہر ریویور کی ہینڈلنگ کے لئے۔
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) checksum ضمانتوں کے لئے۔
- [`devportal/observability`](./observability.md) ٹیلی میٹری exports اور alerting hooks کے لئے۔

## ویو پلان

| ویو | سامعین | انٹری معیار | ایگزٹ معیار | نوٹس |
| --- | --- | --- | --- | --- |
| **W0 - Core maintainers** | Docs/SDK maintainers جو day-one مواد validate کرتے ہیں۔ | GitHub ٹیم `docs-portal-preview` آباد ہو، `npm run serve` checksum gate سبز ہو، Alertmanager 7 دن خاموش رہے۔ | تمام P0 docs ریویو، backlog ٹیگ شدہ، کوئی blocking incident نہ ہو۔ | فلو validate کرنے کے لئے؛ دعوتی ای میل نہیں، صرف preview artifacts شیئر کریں۔ |
| **W1 - Partners** | SoraFS آپریٹرز، Torii integrators، اور NDA کے تحت governance reviewers۔ | W0 ختم، قانونی شرائط منظور، Try-it proxy staging پر۔ | پارٹنر sign-off جمع (issue یا signed form)، ٹیلی میٹری میں <=10 concurrent reviewers، 14 دن تک کوئی security regression نہیں۔ | invitation template + request tickets لازم۔ |
| **W2 - Community** | کمیونٹی ویٹ لسٹ سے منتخب contributors۔ | W1 ختم، incident drills rehearsed، public FAQ اپ ڈیٹ۔ | فیڈبیک ہضم، >=2 documentation releases preview pipeline سے بغیر rollback گزر چکی ہوں۔ | concurrent invites محدود (<=25) اور ہفتہ وار بیچ۔ |

`status.md` اور preview request tracker میں فعال ویو درج کریں تاکہ governance فوری طور پر پروگرام کی حالت دیکھ سکے۔

## Preflight checklist

ان اقدامات کو **دعوتیں شیڈول کرنے سے پہلے** مکمل کریں:

1. **CI artifacts دستیاب**
   - تازہ ترین `docs-portal-preview` + descriptor `.github/workflows/docs-portal-preview.yml` کے ذریعے اپ لوڈ ہو۔
   - SoraFS pin `docs/portal/docs/devportal/deploy-guide.md` میں نوٹ ہو (cutover descriptor موجود ہو).
2. **Checksum enforcement**
   - `docs/portal/scripts/serve-verified-preview.mjs` `npm run serve` کے ذریعے invoke ہو۔
   - `scripts/preview_verify.sh` ہدایات macOS + Linux پر ٹیسٹ ہوں۔
3. **Telemetry baseline**
   - `dashboards/grafana/docs_portal.json` صحت مند Try it ٹریفک دکھائے اور `docs.preview.integrity` الرٹ سبز ہو۔
   - `docs/portal/docs/devportal/observability.md` کا تازہ appendix Grafana لنکس کے ساتھ اپ ڈیٹ ہو۔
4. **Governance artifacts**
   - invite tracker issue تیار ہو (ہر ویو کے لئے ایک issue). 
   - reviewer registry template کاپی ہو (دیکھیں [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - قانونی اور SRE approvals issue کے ساتھ منسلک ہوں۔

دعوت بھیجنے سے پہلے invite tracker میں preflight مکمل ہونے کا اندراج کریں۔

## فلو کے مراحل

1. **امیدوار منتخب کریں**
   - ویٹ لسٹ شیٹ یا پارٹنر کیو سے نکالیں۔
   - ہر امیدوار کے پاس مکمل request template ہونا یقینی بنائیں۔
2. **رسائی کی منظوری**
   - invite tracker issue پر approver اسائن کریں۔
   - prerequisites چیک کریں (CLA/contract, acceptable use, security brief).
3. **دعوتیں ارسال کریں**
   - [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) کے placeholders (`<preview_tag>`, `<request_ticket>`, contacts) بھریں۔
   - descriptor + archive hash، Try it staging URL، اور support channels منسلک کریں۔
   - فائنل ای میل (یا Matrix/Slack transcript) issue میں محفوظ کریں۔
4. **Onboarding ٹریک کریں**
   - invite tracker کو `invite_sent_at`, `expected_exit_at`, اور status (`pending`, `active`, `complete`, `revoked`) کے ساتھ اپ ڈیٹ کریں۔
   - auditability کے لئے reviewer intake request کو لنک کریں۔
5. **Telemetry مانیٹر کریں**
   - `docs.preview.session_active` اور `TryItProxyErrors` alerts پر نظر رکھیں۔
   - اگر ٹیلی میٹری baseline سے ہٹے تو incident کھولیں اور نتیجہ invitation entry کے ساتھ نوٹ کریں۔
6. **فیڈبیک جمع کریں اور خارج ہوں**
   - فیڈبیک آنے پر یا `expected_exit_at` گزرنے پر دعوتیں بند کریں۔
   - اگلی cohort پر جانے سے پہلے ویو issue میں مختصر خلاصہ (findings, incidents, next actions) اپ ڈیٹ کریں۔

## Evidence & reporting

| Artifact | کہاں محفوظ کریں | اپ ڈیٹ cadence |
| --- | --- | --- |
| invite tracker issue | GitHub پروجیکٹ `docs-portal-preview` | ہر دعوت کے بعد اپ ڈیٹ کریں۔ |
| reviewer roster export | `docs/portal/docs/devportal/reviewer-onboarding.md` میں linked registry | ہفتہ وار۔ |
| telemetry snapshots | `docs/source/sdk/android/readiness/dashboards/<date>/` (telemetry bundle reuse کریں) | ہر ویو + incidents کے بعد۔ |
| feedback digest | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (ہر ویو کیلئے فولڈر بنائیں) | ویو exit کے 5 دن کے اندر۔ |
| governance meeting note | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | ہر DOCS-SORA governance sync سے پہلے بھریں۔ |

ہر بیچ کے بعد `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
چلائیں تاکہ مشین ریڈایبل digest بنے۔ رینڈر شدہ JSON کو ویو issue کے ساتھ منسلک کریں تاکہ governance reviewers پوری لاگ دوبارہ چلائے بغیر دعوتی تعداد کی تصدیق کر سکیں۔

ہر ویو ختم ہونے پر evidence کی فہرست `status.md` کے ساتھ منسلک کریں تاکہ روڈ میپ انٹری جلدی اپ ڈیٹ ہو سکے۔

## Rollback اور pause معیار

جب درج ذیل میں سے کوئی ہو تو دعوتی فلو روک دیں (اور governance کو مطلع کریں):

- Try it proxy incident جس میں rollback کرنا پڑا (`npm run manage:tryit-proxy`).
- Alert fatigue: 7 دن کے اندر preview-only endpoints کے لئے >3 alert pages.
- Compliance gap: دعوت بغیر signed terms یا request template لاگ کئے بھیجی گئی۔
- Integrity risk: `scripts/preview_verify.sh` سے checksum mismatch پکڑا گیا۔

invite tracker میں remediation دستاویز کرنے اور کم از کم 48 گھنٹے تک ٹیلی میٹری ڈیش بورڈ مستحکم ہونے کی تصدیق کے بعد ہی دوبارہ شروع کریں۔
