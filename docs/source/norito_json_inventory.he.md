<!-- Hebrew translation of docs/source/norito_json_inventory.md -->

---
lang: he
direction: rtl
source: docs/source/norito_json_inventory.md
status: complete
translator: manual
---

<div dir="rtl">

# אינבנטרי Norito JSON (שימוש ב-Serde)

_עודכן לאחרונה: 9 בנובמבר 2025 באמצעות `python3 scripts/inventory_serde_usage.py --json`_

הריצה האחרונה של האינבנטרי מציגה **אפס שימוש ב-Serde בקרייטי הפרודקשן**. כל מודולי הריצה מסתמכים על נגזרות, visitors ועזרי Norito. ההתאמות היחידות שנותרו בדוח ה-JSON הגולמי נובעות מ:

- מסמכי מדיניות (`AGENTS.md`, ‏`CONTRIBUTING.md`, ‏`status.md`) שמתארים את מגנוני המגן למניעת Serde בקוד פרודקשן.
- סקריפטים (`scripts/check_no_direct_serde.sh`, ‏`scripts/deny_serde_json.sh`, ‏`scripts/list_serde_json_candidates.sh`) שמיישמים או מתעדים את מנגנוני המגן.
- נגזרות Norito כגון `#[derive(JsonSerialize, JsonDeserialize)]`; הביטוי `Serialize` עדיין מזוהה ע״י מנוע האיתור אך מדובר במימוש Norito, לא Serde.
- רשימות היתרים לבדיקות שמשוות פלטים היסטוריים של Serde מול snapshot-ים של Norito.

## סיכום

| קטגוריה | פגיעות Serde | פרשנות |
| --- | --- | --- |
| קרייטי פרודקשן (`core`, ‏`torii`, ‏`ivm`, ‏`config`, ‏`norito`, ‏`fastpq_prover`, ‏`cli`) | 0 | אושר נקי; רק נגזרות Norito קיימות בעץ. |
| כלי עזר ומסמכי מדיניות (`scripts`, ‏`docs`, ‏`AGENTS.md`, ‏`misc`) | טקסט מגננות | השורות מופנות לתיעוד או לסקריפטים של deny-list. |
| דוגמאות ובדיקות (`data_model_samples`, ‏`integration`, ‏`pytests`) | Fixtures מכוונים | snapshot-ים היסטוריים נשמרים לרגרסיה. |

הדוח הממוכשר ב-`docs/source/norito_json_inventory.json` מפרט אילו התאמות הותרו ולמה. ריצות עתידיות צריכות להמשיך להחזיר אפס פגיעות בפרודקשן; כל כניסה שלא נמצא ברשימת ההיתרים מצביעה על רגרסיה במאמצי הסרת Serde.

</div>
