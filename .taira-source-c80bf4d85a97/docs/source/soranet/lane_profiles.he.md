---
lang: he
direction: rtl
source: docs/source/soranet/lane_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c54af03ac031f706287033ae7158c6d58c76ef275cf88d4127ed0a3f296ec5a
source_last_modified: "2026-01-03T18:08:02.001573+00:00"
translation_last_reviewed: 2026-01-22
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/soranet/lane_profiles.md -->

# פרופילי נתיב של SoraNet

`network.lane_profile` שולט בתקרות החיבור כברירת מחדל ובתקציבי uplink בעדיפות נמוכה
שמוחלים על שכבת ה‑P2P. הפרופיל **core** משקף את ההתנהגות הקודמת (תקרות לא מוגדרות
אלא אם הוגדרו במפורש) ומיועד לפריסות דאטה־סנטר/מאמתים. הפרופיל **home** מחיל
מגבלות שמרניות לקישורים מוגבלים ומפיק תקציבים לכל עמית באופן אוטומטי.

| פרופיל | Tick (ms) | רמז MTU (bytes) | יעד uplink (Mbps) | שכנים בקצב קבוע | מקס' נכנסים | מקס' כולל | תקציב עדיפות נמוכה לעמית | קצב עדיפות נמוכה לעמית |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| core | 5 | 1500 | הורשה מהקונפיגורציה | הורשה מהקונפיגורציה | הורשה מהקונפיגורציה | הורשה מהקונפיגורציה | הורשה מהקונפיגורציה | הורשה מהקונפיגורציה |
| home | 10 | 1400 | 120 | 12 | 12 | 32 | 1,250,000 B/s (~10 Mbps) | 947 msgs/s (בהנחה של payloads בגודל 1,320 B) |

### התנהגות

- הפרופיל שנבחר נרשם במשטח הקונפיגורציה (`network.lane_profile`) כדי ש‑CLI/SDK יוכלו לדווח איזה preset shaping פעיל.
- Home ממלא אוטומטית את `max_incoming`, `max_total_connections`,
  `low_priority_bytes_per_sec`, ו‑`low_priority_rate_per_sec` כאשר שדות אלו אינם מוגדרים.
  ערכים מפורשים עדיין גוברים אם מספקים אותם.
- שינוי הפרופיל דרך API עדכון הקונפיגורציה מחיל מחדש את התקרות הנגזרות כך שקישורים
  מוגבלים מנטרלים נתיבים נוספים באופן אוטומטי ללא כוונון ידני.
- רמזי MTU הם מייעצים; אם אתם עוקפים תקרות מסגרות לפי נושא, שמרו עליהן בתוך מעטפת
  ה‑payload המוצהרת כדי להימנע מפירוק חבילות בקישורי home/consumer.

### דוגמה

```toml
[network]
lane_profile = "home"               # apply home shaping defaults
# Optional overrides if you need stricter or looser caps:
# max_total_connections = 40
# low_priority_bytes_per_sec = 900000
```

</div>
