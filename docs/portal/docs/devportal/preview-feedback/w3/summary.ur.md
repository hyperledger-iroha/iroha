---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w3/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 76a4303fa2657476a3f983f1aa5597c9ddb478f670d233b0a7cf4e3791419a72
source_last_modified: "2025-11-20T12:45:46.606949+00:00"
translation_last_reviewed: 2026-01-01
---

---
id: preview-feedback-w3-summary
title: W3 بیٹا فيڈبیک اور اسٹیٹس خلاصہ
sidebar_label: W3 خلاصہ
description: 2026 beta preview wave (finance, observability, SDK, ecosystem) کے لئے live digest۔
---

| آئٹم | تفصیل |
| --- | --- |
| لہر | W3 - beta cohorts (finance + ops + SDK partner + ecosystem advocate) |
| دعوتی ونڈو | 2026-02-18 -> 2026-02-28 |
| آرٹیفیکٹ ٹیگ | `preview-20260218` |
| ٹریکر ایشو | `DOCS-SORA-Preview-W3` |
| شرکا | finance-beta-01, observability-ops-02, partner-sdk-03, ecosystem-advocate-04 |

## نمایاں نکات

1. **End-to-end evidence pipeline.** `npm run preview:wave -- --wave preview-20260218 --invite-start 2026-02-18 --invite-end 2026-02-28 --report-date 2026-03-01 --notes "Finance/observability beta wave"` فی ویو سمری (`artifacts/docs_portal_preview/preview-20260218-summary.json`)، digest (`preview-20260218-digest.md`) بناتا ہے اور `docs/portal/src/data/previewFeedbackSummary.json` اپ ڈیٹ کرتا ہے تاکہ governance reviewers ایک ہی کمانڈ پر انحصار کر سکیں۔
2. **Telemetry + governance coverage.** تمام چار reviewers نے checksum gated access کی تصدیق کی، feedback جمع کرایا، اور وقت پر revoke ہوئے؛ digest میں feedback issues (`docs-preview/20260218` set + `DOCS-SORA-Preview-20260218`) اور wave کے دوران جمع Grafana runs شامل ہیں۔
3. **Portal surfacing.** اپ ڈیٹ شدہ پورٹل ٹیبل اب W3 wave کو latency اور response-rate metrics کے ساتھ closed دکھاتی ہے، اور نیچے نئی log page ان auditors کے لئے timeline دکھاتی ہے جو raw JSON log نہیں کھینچتے۔

## ایکشن آئٹمز

| ID | وضاحت | مالک | اسٹیٹس |
| --- | --- | --- | --- |
| W3-A1 | preview digest محفوظ کر کے tracker کے ساتھ منسلک کرنا۔ | Docs/DevRel lead | ✅ مکمل (2026-02-28). |
| W3-A2 | invite/digest evidence کو portal + roadmap/status میں mirror کرنا۔ | Docs/DevRel lead | ✅ مکمل (2026-02-28). |

## اختتامی خلاصہ (2026-02-28)

- دعوتیں 2026-02-18 کو بھیجی گئیں، acknowledgements چند منٹ بعد لاگ ہوئے؛ final telemetry check کے بعد 2026-02-28 کو preview access revoke کیا گیا۔
- Digest + summary `artifacts/docs_portal_preview/` میں محفوظ ہوئے، raw log کو `artifacts/docs_portal_preview/feedback_log.json` سے anchor کیا گیا تاکہ replay ہو سکے۔
- Issue follow-ups `docs-preview/20260218` کے تحت governance tracker `DOCS-SORA-Preview-20260218` میں فائل ہوئے؛ CSP/Try it نوٹس observability/finance owners کو روٹ کیے گئے اور digest سے لنک کیے گئے۔
- Tracker row کو 🈴 Completed پر اپ ڈیٹ کیا گیا اور پورٹل feedback table نے closed wave دکھایا، جس سے DOCS-SORA کی باقی beta readiness task مکمل ہوئی۔
