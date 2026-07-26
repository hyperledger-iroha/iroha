<!-- Hebrew translation of docs/source/zk/prover_runbook.md -->

---
lang: he
direction: rtl
source: docs/source/zk/prover_runbook.md
status: complete
translator: manual
---

<div dir="rtl">

# רנבוק Torii ZK: קבצים מצורפים ומוכיח

רנבוק זה מסייע למפעילים לנטר, להתריע ולטפל בשירות הקבצים המצורפים של Torii ובתהליך המוכיח הרץ ברקע. ההנחיות מניחות שפרופיל הטלמטריה מאפשר חשיפה של מדדי Prometheus (`telemetry_profile = "extended"` או `"full"`) ושדשבורדים תפעוליים אוספים את נקודת הקצה `/metrics`.

## רכיבים

- **ממשק הקבצים המצורפים** – `POST /v1/zk/attachments` שומר מטענים אטומים (הוכחות, טרנסקריפטים). כאשר המוכיח מופעל, הרשומות נסרקות ברקע. ניתן לרשום, להוריד ולמחוק מצורפים דרך ה-API או `iroha_cli app zk attachments *`.
- **מוכיח ברקע** – נשלט על ידי `torii.zk_prover_enabled=true`. התהליך מרוקן את תור המצורפים, מאמת `ProofAttachment`, ומפיק דוחות JSON (`/v1/zk/prover/reports`). תקציבי משאבים: `torii.zk_prover_max_inflight`, ‏`torii.zk_prover_max_scan_bytes`, ‏`torii.zk_prover_max_scan_millis`. הסקופ נשלט על ידי `torii.zk_prover_allowed_backends`/`torii.zk_prover_allowed_circuits`, ובמקרה שאין VK אינליין נטענים בייטים מ-`torii.zk_prover_keys_dir` בשם `<backend>__<name>.vk`.
- **משטח טלמטריה** – מדדים רשומים ב-`crates/iroha_telemetry::metrics` ומפורסמים תחת `/metrics`.

## רשימת בדיקה מהירה

| תרחיש | פעולה |
| --- | --- |
|  `torii_zk_prover_budget_exhausted_total` גדל | לבדוק את סיבת התקציב (`bytes` או `time`). להעלות את הגבולות בקונפיגורציה או להפחית את נפח המצורפים. |
| `torii_zk_prover_pending` נשאר מעל 0 במשך יותר מעשר דקות | תור פקוק: לבדוק מדדי סריקה, לשקול הרחבת מגבלת ה-inflight או לצמצם מצורפים. |
| מצורפים אינם נסרקים | לוודא שהעובד מופעל, לבדוק לוגים של `zk_prover` ולוודא שהתקציבים אינם אפס. |
| העלאות דרך CLI/SDK נכשלות | לבדוק בלוגי Torii שגיאות HTTP ולוודא שיש מקום בדיסק. |
| דוחות חסרים | לאשר את ה-TTL (`torii.zk_prover_reports_ttl_secs`) ולחפש הודעות GC בלוגים. |

## מדדים וקטעי PromQL

| מדד | תיאור | PromQL / רמז להתרעה |
| --- | --- | --- |
| `torii_zk_prover_inflight` | מספר המצורפים שנמצאים בעיבוד | להתריע אם הערך נדבק למקסימום במשך יותר מעשר דקות. |
| `torii_zk_prover_pending` | אורך התור הממתין לרשות עיבוד | `avg_over_time(torii_zk_prover_pending[5m]) > 0` מצביע על פיגור. |
| `torii_zk_prover_last_scan_bytes` / `torii_zk_prover_last_scan_ms` | גודל או זמן של מחזור הסריקה האחרון | לעקוב באמצעות `increase` לאורך זמן כדי לזהות רגרסיות. |
| `torii_zk_prover_budget_exhausted_total{reason}` | מספר הפעמים שהתקציב חוסל (לפי סיבה) | `rate(torii_zk_prover_budget_exhausted_total[5m]) > 0` צריך להניע בדיקה. |
| `torii_zk_prover_attachment_bytes_bucket` | התפלגות גדלי המצורפים לפי `content_type` | `histogram_quantile(0.95, sum(rate(torii_zk_prover_attachment_bytes_bucket[5m])) by (le, content_type))` להערכת נפח ההוכחות. |
| `torii_zk_prover_latency_ms_bucket` | היסטוגרמת השהיית העיבוד לכל מצורף | להתריע כאשר ה-p95 עובר את `torii.zk_prover_max_scan_millis`. |
| `zk_verify_proof_bytes_bucket` / `zk_verify_latency_ms_bucket` | מדדי אימות מקצה לקצה (נתיב ביצוע) | מסייעים לקשר בין עומסי אימות בלדג'ר לבין קצב העיבוד של הפרובר. |
| `torii_zk_prover_gc_total` | מספר המצורפים שנמחקו עקב TTL | זינוק חד עשוי להעיד על TTL שאינו מוגדר נכון. |

## דשבורד Grafana לדוגמה

ניתן לייבא את הדשבורד המוכן בקובץ [`grafana_zk_prover.json`](../grafana_zk_prover.json). הפאנלים הכלולים בו:

1. **עומק תור** – `torii_zk_prover_pending` ו-`torii_zk_prover_inflight`.
2. **תפוקת מצורפים** – קוונטילים של היסטוגרמת גדלי המצורפים.
3. **השהיית המוכיח** – השוואת `torii_zk_prover_latency_ms` לגבולות התקציב המוגדרים.
4. **קצב גמר התקציב** – `increase(torii_zk_prover_budget_exhausted_total[15m])` לפי סיבה.
5. **חפיפה עם השהיית אימות** – השוואה בין `zk_verify_latency_ms` לבין השהיית הפרובר כדי לזהות עומסים בלדג'ר.

ייבאו את ה-JSON דרך *Dashboards → Import* ב-Grafana. עדכנו את מזהה מקור הנתונים של Prometheus לפי הסביבה.

## לוגים וניטור

- ממשק הקבצים המצורפים רושם לוגים מבניים עם היעד `torii::zk_attachments`. כישלונות מחזירים סטטוס HTTP מפורש שמופיע בלוגים של Torii.
- המוכיח ברקע רושם תחת `torii::zk_prover`. בכל סריקה מופיעים `scheduled`, ‏`processed_bytes`, ‏`elapsed_ms` ומצב התקציב.
- לצורך תחקור מעמיק ניתן להפעיל זמנית `RUST_LOG=torii::zk_prover=debug`. אין להשאיר רמת debug בפרודקשן.
- תהליך הטלמטריה רושם פגיעות תקציב ואיפוסי תורים תחת `torii::zk_prover::metrics`.

## תרחישי תגובה נפוצים

### 1. התפרצות של גמר תקציב

1. לאמת במדדים: `torii_zk_prover_budget_exhausted_total{reason}` במגמת עלייה.
2. לבדוק בלוגים אם הסיבה היא גודל (`bytes`) או זמן (`time`).
3. לבדוק תצורה:
   - `torii.zk_prover_max_scan_bytes`
   - `torii.zk_prover_max_scan_millis`
   - `torii.zk_prover_max_inflight`
4. אם המטענים גדולים מהצפוי, לוודא שלקוחות אינם מעלים ארכיונים דחוסים או צרורות הוכחות. ניתן להעלות את תקציב הבייטים או לפצל את המצורפים במעלה הזרם.
5. אחרי התאמות, לנטר את `torii_zk_prover_pending` כדי לוודא שהתור מתנקז ולתעד את הספים החדשים.

### 2. תור מצורפים גדל

1. לבדוק את `torii_zk_prover_pending` ואת `torii_zk_prover_inflight`.
2. לבדוק האם `torii_zk_prover_last_scan_ms` מתקרב למגבלה; ייתכן שהעיבוד מוגבל על ידי `torii.zk_prover_max_scan_millis`.
3. לוודא שהעובד פעיל: בלוגים אמורים להציג שורות `scan_completed` בכל `torii.zk_prover_scan_period_secs`.
4. לשקול העלאת `torii.zk_prover_max_inflight` (מקביליות) או קיצור מרווחי הסריקה.
5. אם מצורפים ישנים אינם דרושים, ניתן למחוק אותם ידנית באמצעות `DELETE /v1/zk/attachments/:id` או הפקודה המקבילה ב-CLI.

### 3. דוחות חסרים או מתעכבים

1. לבדוק את `torii.zk_prover_reports_ttl_secs` ולהבטיח שמדיניות השמירה תואמת לציפיות.
2. לעקוב אחרי `torii_zk_prover_gc_total` כדי לראות אם דוחות נמחקים מוקדם מהמתוכנן.
3. לוודא שלקוחות מושכים את `/v1/zk/prover/reports` לפני שפג ה-TTL.
4. לצורך חקירה דחופה, ניתן למשוך את המצורפים הגולמיים ולעבד אותם מחדש במצב לא מקוון.

## אינטגרציה עם CLI ו-SDK

- `iroha_cli app zk attachments` עוטף את נקודות הקצה של Torii; לבנות את הכלי מאותו הקומיט של הצומת כדי לשמור על סנכרון DTO.
- כלי העזר של Swift ו-Python תחת `ToriiClient` מספקים `upload_attachment`, ‏`list_attachments`, ‏`get_attachment` ו-`delete_attachment`. יש להשתמש במדדים מהרנבוק כדי לאמת את הזרימות שמונעות על ידי SDK.

## ניהול שינויים

- לתעד שינויים בקונפיגורציה במחברת התפעול (פורמט מומלץ: חותמת זמן, מפעיל, ערך ישן, ערך חדש, סיבה).
- בעת שינוי תקציבים או תדירות סריקה, לעדכן את הדשבורדים של Grafana ואת ספי ההתרעות בהתאם.
- להריץ `cargo test -p iroha_torii zk_prover` לפני הפעלת גרסאות מוכיח חדשות כדי לזהות רגרסיות המכוסות בבדיקות היחידה.

## מקורות

- [ZK App API](../zk_app_api.md)
- [ZK lifecycle (VK/proof flows)](lifecycle.md)
- [Torii CLI smoke tests](../../../crates/iroha_cli/tests/cli_smoke.rs)
- [Telemetry overview](../telemetry.md)
- [Roadmap – Milestone 4](../../../roadmap.md)

</div>
