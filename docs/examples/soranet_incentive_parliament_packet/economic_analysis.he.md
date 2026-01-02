---
lang: he
direction: rtl
source: docs/examples/soranet_incentive_parliament_packet/economic_analysis.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1b453559c05401edc11894e585c8d5ca4b678d4667c1cef0415582e1f7de8246
source_last_modified: "2025-11-05T17:23:10.790688+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/soranet_incentive_parliament_packet/economic_analysis.md -->

# ניתוח כלכלי - Shadow Run 2025-10 -> 2025-11

ארטיפקט מקור: `docs/examples/soranet_incentive_shadow_run.json` (חתימה + מפתח ציבורי באותה ספריה). הסימולציה הריצה 60 epochs לכל relay עם מנוע תגמולים נעול ל-`RewardConfig` שמתועד ב-`reward_config.json`.

## סיכום חלוקה

- **סה"כ תשלומים:** 5,160 XOR על פני 360 epochs מתוגמלים.
- **מעטפת הוגנות:** מקדם ג'יני 0.121; נתח ה-relay העליון 23.26%
  (הרבה מתחת ל-guardrail של ממשל 30%).
- **זמינות:** ממוצע צי 96.97%, כל ה-relays נשארו מעל 94%.
- **רוחב פס:** ממוצע צי 91.20%, עם מבצע נמוך ב-87.23%
  בזמן תחזוקה מתוכננת; עונשים הוחלו אוטומטית.
- **רעש תאימות:** נצפו 9 epochs של warning ו-3 השעיות
  והומרו להפחתות תשלום; אף relay לא חרג ממגבלת 12 warnings.
- **היגיינה תפעולית:** לא דולגו snapshots של מדדים עקב חוסר
  config, bonds או כפילויות; לא נרשמו שגיאות מחשבון.

## תצפיות

- ההשעיות תואמות epochs שבהם relays עברו למצב תחזוקה. מנוע התשלומים
  הפיק תשלומים אפסיים עבור epochs אלו תוך שמירה על מסלול הביקורת ב-JSON של shadow-run.
- קנסות warning קיזזו 2% מהתשלומים שהושפעו; החלוקה עדיין מתכנסת בזכות
  משקלי uptime/bandwidth (650/350 per mille).
- סטיית רוחב הפס עוקבת אחר heatmap אנונימי של guard. המבצע הנמוך ביותר
  (`6666...6666`) שמר על 620 XOR לאורך החלון, מעל רצפת 0.6x.
- התראות רגישות להשהיה (`SoranetRelayLatencySpike`) נשארו מתחת
  לספי warning לאורך החלון; הדשבורדים הקשורים נשמרים תחת
  `dashboards/grafana/soranet_incentives.json`.

## פעולות מומלצות לפני GA

1. המשיכו להריץ replays חודשיים של shadow ועדכנו את סט הארטיפקטים ואת
   הניתוח הזה אם הרכב הצי משתנה.
2. חסמו תשלומים אוטומטיים על בסיס חבילת התראות Grafana המוזכרת ב-roadmap
   (`dashboards/alerts/soranet_incentives_rules.yml`); צרפו צילומי מסך לפרוטוקולי
   הממשל בעת בקשת חידוש.
3. הריצו מחדש את מבחן ה-stress הכלכלי אם התשלום הבסיסי, משקלי uptime/bandwidth או
   קנס ה-compliance משתנים ב- >=10%.

</div>
