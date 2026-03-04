---
id: preview-feedback-w0-summary
lang: ur
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w0/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

| آئٹم | تفصیل |
| --- | --- |
| لہر | W0 - core maintainers |
| خلاصہ تاریخ | 2025-03-27 |
| ریویو ونڈو | 2025-03-25 -> 2025-04-08 |
| شرکا | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observability-01 |
| آرٹیفیکٹ ٹیگ | `preview-2025-03-24` |

## نمایاں نکات

1. **Checksum ورک فلو** - تمام reviewers نے تصدیق کی کہ `scripts/preview_verify.sh`
   مشترکہ descriptor/archive جوڑی کے خلاف کامیاب رہا۔ کسی دستی override کی
   ضرورت نہیں ہوئی۔
2. **نیویگیشن فيڈبیک** - sidebar کی ترتیب میں دو معمولی مسائل رپورٹ ہوئے
   (`docs-preview/w0 #1-#2`). دونوں Docs/DevRel کو دیے گئے اور لہر کو بلاک
   نہیں کرتے۔
3. **SoraFS runbook برابری** - sorafs-ops-01 نے `sorafs/orchestrator-ops` اور
   `sorafs/multi-source-rollout` کے درمیان زیادہ واضح cross-links کی درخواست کی۔
   follow-up issue بنایا گیا؛ W1 سے پہلے حل کرنا ہے۔
4. **ٹیلیمیٹری ریویو** - observability-01 نے تصدیق کی کہ `docs.preview.integrity`,
   `TryItProxyErrors` اور Try-it proxy logs سب green رہے؛ کوئی alert فائر نہیں ہوا۔

## ایکشن آئٹمز

| ID | وضاحت | مالک | اسٹیٹس |
| --- | --- | --- | --- |
| W0-A1 | devportal sidebar entries کو دوبارہ ترتیب دینا تاکہ reviewers والے docs نمایاں ہوں (`preview-invite-*` کو ایک ساتھ رکھیں). | Docs-core-01 | مکمل - sidebar اب reviewers docs کو مسلسل دکھاتا ہے (`docs/portal/sidebars.js`). |
| W0-A2 | `sorafs/orchestrator-ops` اور `sorafs/multi-source-rollout` کے درمیان واضح cross-link شامل کرنا۔ | Sorafs-ops-01 | مکمل - ہر runbook اب دوسرے کی طرف لنک کرتا ہے تاکہ rollout کے دوران دونوں گائیڈ نظر آئیں۔ |
| W0-A3 | governance tracker کے ساتھ telemetry snapshots + query bundle شیئر کرنا۔ | Observability-01 | مکمل - bundle `DOCS-SORA-Preview-W0` کے ساتھ منسلک ہے۔ |

## اختتامی خلاصہ (2025-04-08)

- پانچوں reviewers نے تکمیل کی تصدیق کی، لوکل builds صاف کیے، اور preview ونڈو سے
  باہر نکل گئے؛ access revocations `DOCS-SORA-Preview-W0` میں ریکارڈ ہیں۔
- لہر کے دوران کوئی incidents یا alerts نہیں ہوئے؛ telemetry dashboards پوری مدت
  green رہے۔
- نیویگیشن + cross-link اقدامات (W0-A1/A2) نافذ ہو چکے ہیں اور اوپر کے docs میں
  دکھائی دیتے ہیں؛ telemetry evidence (W0-A3) tracker کے ساتھ منسلک ہے۔
- evidence bundle محفوظ کر دیا گیا: telemetry screenshots، دعوت کی acknowledgements،
  اور یہ خلاصہ tracker issue سے لنک ہیں۔

## اگلے اقدامات

- W1 کھولنے سے پہلے W0 action items نافذ کریں۔
- قانونی منظوری اور proxy staging slot حاصل کریں، پھر [preview invite flow](../../preview-invite-flow.md) میں
  بیان کردہ partner-wave preflight اقدامات پر عمل کریں۔

_یہ خلاصہ [preview invite tracker](../../preview-invite-tracker.md) سے منسلک ہے تاکہ
DOCS-SORA roadmap قابلِ سراغ رہے۔_
