---
lang: he
direction: rtl
source: docs/examples/soranet_gar_intake_form.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6cd4da7e590d581719ed2607994d7d9eb16d153fbd06f85655d0da37c727853a
source_last_modified: "2025-11-21T15:16:55.013762+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/soranet_gar_intake_form.md -->

# טופס קליטה GAR של SoraNet

השתמשו בטופס קליטה זה בעת בקשת פעולת GAR (purge, ttl override, rate ceiling, moderation directive, geofence או legal hold).
הטופס שנשלח צריך להיות מוצמד לצד הפלטים של `gar_controller` כדי שלוגי הביקורת והקבלות יצטטו את אותם URI של ראיות.

| שדה | ערך | הערות |
|-----|-----|-------|
| מזהה בקשה |  | מזהה כרטיס guardian/ops. |
| מבוקש על ידי |  | חשבון + איש קשר. |
| תאריך/שעה (UTC) |  | מועד תחילת הפעולה. |
| שם GAR |  | לדוגמה, `docs.sora`. |
| מארח קנוני |  | לדוגמה, `docs.gw.sora.net`. |
| פעולה |  | `ttl_override` / `rate_limit_override` / `purge_static_zone` / `geo_fence` / `legal_hold` / `moderation`. |
| החלפת TTL (שניות) |  | נדרש רק עבור `ttl_override`. |
| תקרת קצב (RPS) |  | נדרש רק עבור `rate_limit_override`. |
| אזורים מותרים |  | רשימת אזורי ISO בעת בקשת `geo_fence`. |
| אזורים חסומים |  | רשימת אזורי ISO בעת בקשת `geo_fence`. |
| סלאגים של moderation |  | להתאים להנחיות המודרציה של GAR. |
| תגי purge |  | תגים שיש לבצע להם purge לפני הפצה. |
| תוויות |  | תוויות מכונה (incident id, drill name, pop scope). |
| URI ראיות |  | לוגים/דשבורדים/מפרטים התומכים בבקשה. |
| URI ביקורת |  | URI ביקורת לכל pop אם שונה מהברירות מחדל. |
| תוקף מבוקש |  | Unix timestamp או RFC3339; השאירו ריק לברירת המחדל. |
| סיבה |  | הסבר למשתמש; מופיע בקבלות ובדשבורדים. |
| מאשר |  | מאשר guardian/committee לבקשה. |

### שלבי הגשה

1. מלאו את הטבלה וצרפו אותה לכרטיס governance.
2. עדכנו את תצורת GAR controller (`policies`/`pops`) עם `labels`/`evidence_uris`/`expires_at_unix` תואמים.
3. הריצו `cargo xtask soranet-gar-controller ...` כדי להפיק אירועים/קבלות.
4. הוסיפו `gar_controller_summary.json`, `gar_reconciliation_report.json`, `gar_metrics.prom`, ו-`gar_audit_log.jsonl` לאותו כרטיס. המאשר מאשר שספירת הקבלות תואמת את רשימת ה-PoP לפני שיגור.

</div>
