---
lang: ar
direction: rtl
source: docs/examples/soranet_gar_intake_form.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6cd4da7e590d581719ed2607994d7d9eb16d153fbd06f85655d0da37c727853a
source_last_modified: "2025-11-21T15:16:55.013762+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/soranet_gar_intake_form.md -->

# نموذج استقبال GAR من SoraNet

استخدم نموذج الاستقبال هذا عند طلب اجراء GAR (purge, ttl override, rate ceiling, moderation directive, geofence، او legal hold).
يجب تثبيت النموذج المرسل بجوار مخرجات `gar_controller` حتى تشير سجلات التدقيق والايصالات الى نفس عناوين URI للادلة.

| الحقل | القيمة | الملاحظات |
|-------|--------|-----------|
| معرف الطلب |  | معرف تذكرة guardian/ops. |
| مقدم الطلب |  | الحساب + جهة الاتصال. |
| التاريخ/الوقت (UTC) |  | وقت بدء الاجراء. |
| اسم GAR |  | مثال: `docs.sora`. |
| المضيف القياسي |  | مثال: `docs.gw.sora.net`. |
| الاجراء |  | `ttl_override` / `rate_limit_override` / `purge_static_zone` / `geo_fence` / `legal_hold` / `moderation`. |
| تجاوز TTL (ثوان) |  | مطلوب فقط لـ `ttl_override`. |
| سقف المعدل (RPS) |  | مطلوب فقط لـ `rate_limit_override`. |
| المناطق المسموح بها |  | قائمة مناطق ISO عند طلب `geo_fence`. |
| المناطق المحظورة |  | قائمة مناطق ISO عند طلب `geo_fence`. |
| slugs الموديريشن |  | يجب ان تطابق توجيهات moderation لـ GAR. |
| وسوم purge |  | وسوم يجب purge قبل الخدمة. |
| الوسوم |  | وسوم الالة (incident id, drill name, pop scope). |
| عناوين URI للادلة |  | سجلات/لوحات/مواصفات تدعم الطلب. |
| URI التدقيق |  | URI تدقيق لكل pop اذا اختلف عن الافتراضيات. |
| انتهاء الصلاحية المطلوب |  | Unix timestamp او RFC3339; اتركه فارغا للقيمة الافتراضية. |
| السبب |  | شرح موجه للمستخدم؛ يظهر في الايصالات واللوحات. |
| المعتمد |  | معتمد guardian/committee للطلب. |

### خطوات التقديم

1. املأ الجدول وارفقه بتذكرة الحوكمة.
2. حدّث اعدادات GAR controller (`policies`/`pops`) بقيم `labels`/`evidence_uris`/`expires_at_unix` المطابقة.
3. شغّل `cargo xtask soranet-gar-controller ...` لاصدار الاحداث/الايصالات.
4. ضع `gar_controller_summary.json`, `gar_reconciliation_report.json`, `gar_metrics.prom`, و `gar_audit_log.jsonl` في نفس التذكرة. يؤكد المعتمد ان عدد الايصالات يطابق قائمة PoP قبل الارسال.

</div>
