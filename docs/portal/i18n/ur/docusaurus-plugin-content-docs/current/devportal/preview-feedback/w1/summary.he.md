---
lang: he
direction: rtl
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/devportal/preview-feedback/w1/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ed19d66bd20d3abdb76db2d6d29fc54ccdcbdb8aa1a6aa5f83e29654048a8ef
source_last_modified: "2025-11-14T04:43:19.909755+00:00"
translation_last_reviewed: 2026-01-30
---

| آئٹم | تفصیل |
| --- | --- |
| لہر | W1 - پارٹنرز اور Torii integrators |
| دعوتی ونڈو | 2025-04-12 -> 2025-04-26 |
| آرٹیفیکٹ ٹیگ | `preview-2025-04-12` |
| ٹریکر ایشو | `DOCS-SORA-Preview-W1` |
| شرکا | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## نمایاں نکات

1. **Checksum ورک فلو** - تمام reviewers نے `scripts/preview_verify.sh` کے ذریعے descriptor/archive کی تصدیق کی؛ logs کو دعوتی acknowledgements کے ساتھ محفوظ کیا گیا۔
2. **ٹیلیمیٹری** - `docs.preview.integrity`, `TryItProxyErrors`, اور `DocsPortal/GatewayRefusals` dashboards پوری لہر کے دوران green رہے؛ کوئی incidents یا alert pages نہیں ہوئیں۔
3. **Docs فيڈبیک (`docs-preview/w1`)** - دو معمولی نٹس ریکارڈ ہوئیں:
   - `docs-preview/w1 #1`: Try it سیکشن میں navigation wording واضح کرنا (حل ہو گیا)۔
   - `docs-preview/w1 #2`: Try it اسکرین شاٹ اپ ڈیٹ کرنا (حل ہو گیا)۔
4. **Runbook parity** - SoraFS operators نے تصدیق کی کہ `orchestrator-ops` اور `multi-source-rollout` کے درمیان نئے cross-links نے W0 کے خدشات حل کیے۔

## ایکشن آئٹمز

| ID | وضاحت | مالک | اسٹیٹس |
| --- | --- | --- | --- |
| W1-A1 | `docs-preview/w1 #1` کے مطابق Try it navigation wording اپ ڈیٹ کرنا۔ | Docs-core-02 | ✅ مکمل (2025-04-18). |
| W1-A2 | `docs-preview/w1 #2` کے مطابق Try it اسکرین شاٹ اپ ڈیٹ کرنا۔ | Docs-core-03 | ✅ مکمل (2025-04-19). |
| W1-A3 | پارٹنر findings اور telemetry evidence کو roadmap/status میں سمری کرنا۔ | Docs/DevRel lead | ✅ مکمل (tracker + status.md دیکھیں). |

## اختتامی خلاصہ (2025-04-26)

- تمام آٹھ reviewers نے آخری office hours میں تکمیل کی تصدیق کی، لوکل artefacts صاف کیے، اور ان کی رسائی واپس لی گئی۔
- ٹیلیمیٹری اختتام تک green رہی؛ آخری snapshots `DOCS-SORA-Preview-W1` کے ساتھ منسلک ہیں۔
- دعوتی log میں exit acknowledgements شامل کیے گئے؛ tracker نے W1 کو 🈴 پر سیٹ کیا اور checkpoints شامل کیے۔
- evidence bundle (descriptor, checksum log, probe output, Try it proxy transcript, telemetry screenshots, feedback digest) `artifacts/docs_preview/W1/` میں archive ہوا۔

## اگلے اقدامات

- W2 community intake plan تیار کریں (governance approval + request template tweaks).
- W2 wave کے لئے preview artefact tag ریفریش کریں اور تاریخیں فائنل ہونے پر preflight اسکرپٹ دوبارہ چلائیں۔
- W1 کے قابل اطلاق findings کو roadmap/status میں منتقل کریں تاکہ community wave کے پاس تازہ guidance ہو۔
