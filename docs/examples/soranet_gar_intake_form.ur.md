---
lang: ur
direction: rtl
source: docs/examples/soranet_gar_intake_form.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6cd4da7e590d581719ed2607994d7d9eb16d153fbd06f85655d0da37c727853a
source_last_modified: "2025-11-21T15:16:55.013762+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/soranet_gar_intake_form.md کا اردو ترجمہ -->

# SoraNet GAR انٹیک ٹیمپلیٹ

GAR کارروائی (purge, ttl override, rate ceiling, moderation directive, geofence، یا legal hold) کی درخواست کرتے وقت یہ انٹیک فارم استعمال کریں۔
جمع شدہ فارم کو `gar_controller` کے outputs کے ساتھ pin کیا جانا چاہئے تاکہ audit logs اور receipts ایک ہی evidence URI کا حوالہ دیں۔

| فیلڈ | ویلیو | نوٹس |
|------|-------|------|
| درخواست ID |  | guardian/ops ٹکٹ ID. |
| درخواست گزار |  | اکاؤنٹ + رابطہ. |
| تاریخ/وقت (UTC) |  | کارروائی کب شروع ہونی چاہئے. |
| GAR نام |  | مثال: `docs.sora`. |
| کینونیکل ہوسٹ |  | مثال: `docs.gw.sora.net`. |
| کارروائی |  | `ttl_override` / `rate_limit_override` / `purge_static_zone` / `geo_fence` / `legal_hold` / `moderation`. |
| TTL override (seconds) |  | صرف `ttl_override` کے لئے ضروری. |
| Rate ceiling (RPS) |  | صرف `rate_limit_override` کے لئے ضروری. |
| اجازت یافتہ ریجنز |  | `geo_fence` کی درخواست پر ISO ریجن لسٹ. |
| ممنوع ریجنز |  | `geo_fence` کی درخواست پر ISO ریجن لسٹ. |
| Moderation slugs |  | GAR moderation ہدایات کے مطابق. |
| Purge tags |  | ایسے tags جنہیں serving سے پہلے purge کرنا ہو. |
| لیبلز |  | مشین labels (incident id, drill name, pop scope). |
| Evidence URIs |  | درخواست کی حمایت میں logs/dashboards/specs. |
| Audit URI |  | اگر defaults سے مختلف ہو تو per-pop audit URI. |
| درخواست کردہ ایکسپائری |  | Unix timestamp یا RFC3339; default کے لئے خالی چھوڑیں. |
| وجہ |  | صارف کے لئے وضاحت; receipts اور dashboards میں ظاہر ہوگی. |
| منظور کنندہ |  | درخواست کے لئے guardian/committee approver. |

### جمع کرانے کے مراحل

1. جدول بھریں اور اسے governance ٹکٹ کے ساتھ منسلک کریں.
2. GAR controller کی config (`policies`/`pops`) کو matching `labels`/`evidence_uris`/`expires_at_unix` کے ساتھ اپڈیٹ کریں.
3. events/receipts جاری کرنے کے لئے `cargo xtask soranet-gar-controller ...` چلائیں.
4. `gar_controller_summary.json`, `gar_reconciliation_report.json`, `gar_metrics.prom`, اور `gar_audit_log.jsonl` کو اسی ٹکٹ میں رکھیں۔ منظور کنندہ اس بات کی تصدیق کرتا ہے کہ رسیدوں کی تعداد PoP لسٹ سے میل کھاتی ہے، پھر ڈسپیچ ہوتا ہے.

</div>
