---
lang: he
direction: rtl
source: docs/source/confidential_assets_audit_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8691a94d23e589f46d8e8cf2359d6d9a31f7c38c5b7bf0def69c88d2dd081765
source_last_modified: "2026-01-22T15:38:30.658489+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! ספר הביקורת והתפעול של נכסים סודיים עם הפניה של `roadmap.md:M4`.

# פנקס ביקורת ותפעול של נכסים סודיים

מדריך זה מגבש את משטחי הראיות עליהם מסתמכים המבקרים והמפעילים
בעת אימות תזרימי נכסים סודיים. זה משלים את ספר המשחקים של הסיבוב
(`docs/source/confidential_assets_rotation.md`) ופנקס הכיול
(`docs/source/confidential_assets_calibration.md`).

## 1. עדכוני גילוי ואירועים סלקטיביים

- כל הוראה סודית פולטת מטען מובנה `ConfidentialEvent`
  (`Shielded`, `Transferred`, `Unshielded`) שנלכדו ב
  `crates/iroha_data_model/src/events/data/events.rs:198` ובסדרה על ידי ה
  מבצעים (`crates/iroha_core/src/smartcontracts/isi/world.rs:3699`–`4021`).
  חבילת הרגרסיה מפעילה את עומסי הבטון כך שהאודיטורים יכולים לסמוך עליהם
  פריסות JSON דטרמיניסטיות (`crates/iroha_core/tests/zk_confidential_events.rs:19`–`299`).
- Torii חושף אירועים אלה דרך צינור SSE/WebSocket הסטנדרטי; רואי חשבון
  הירשם באמצעות `ConfidentialEventFilter` (`crates/iroha_data_model/src/events/data/filters.rs:82`),
  אופציונלי היקף להגדרת נכס יחיד. דוגמה CLI:

  ```bash
  iroha ledger events data watch --filter '{ "confidential": { "asset_definition_id": "rose#wonderland" } }'
  ```

- מטא נתונים של מדיניות ומעברים ממתינים זמינים דרך
  `GET /v2/confidential/assets/{definition_id}/transitions`
  (`crates/iroha_torii/src/routing.rs:15205`), שיקוף על ידי Swift SDK
  (`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:3245`) ומתועד ב
  גם מדריכי עיצוב הנכסים הסודיים וגם מדריכי ה-SDK
  (`docs/source/confidential_assets.md:70`, `docs/source/sdk/swift/index.md:334`).

## 2. טלמטריה, לוחות מחוונים וראיות כיול

- מדדי זמן ריצה עומק עץ פני השטח, היסטוריית מחויבות/גבולות, פינוי שורש
  מונים, ויחסי היט של מאמת-מטמון
  (`crates/iroha_telemetry/src/metrics.rs:5760`–`5815`). לוחות מחוונים Grafana ב
  `dashboards/grafana/confidential_assets.json` שולחים את הלוחות המשויכים ו
  התראות, עם זרימת העבודה מתועדת ב-`docs/source/confidential_assets.md:401`.
- ריצות כיול (NS/op, gas/op, ns/gas) עם יומנים חתומים
  `docs/source/confidential_assets_calibration.md`. Apple Silicon העדכני ביותר
  ריצת NEON מאוחסנת בארכיון ב
  `docs/source/confidential_assets_calibration_neon_20260428.log`, ואותו הדבר
  ספר חשבונות מתעד את הוויתור הזמני עבור פרופילים נייטרליים SIMD ו-AVX2 עד
  המארחים של x86 מגיעים לאינטרנט.

## 3. תגובה לאירועים ומשימות מפעיל

- נוהלי סיבוב/שדרוג נמצאים ב
  `docs/source/confidential_assets_rotation.md`, מכסה כיצד לשלב חדש
  חבילות פרמטרים, תזמון שדרוגי מדיניות ויידוע ארנקים/מבקרים. ה
  רשימות גשש (`docs/source/project_tracker/confidential_assets_phase_c.md`).
  בעלי רונבוק וציפיות לחזרות.
- עבור חזרות הפקה או חלונות חירום, מפעילים מצרפים ראיות ל
  ערכי `status.md` (לדוגמה, יומן החזרות הרב-נתיביות) וכוללים:
  `curl` הוכחה למעברי מדיניות, תמונות מצב Grafana והאירוע הרלוונטי
  מעכלים כדי שהאודיטורים יוכלו לשחזר מנטה → העברה → לחשוף קווי זמן.

## 4. קצב סקירה חיצוני

- היקף סקירת אבטחה: מעגלים חסויים, רישומי פרמטרים, מדיניות
  מעברים וטלמטריה. מסמך זה בתוספת טפסי ספר הכיול
  חבילת הראיות שנשלחה לספקים; מעקב אחר תזמון הביקורות מתבצע באמצעות
  M4 ב-`docs/source/project_tracker/confidential_assets_phase_c.md`.
- המפעילים חייבים לעדכן את `status.md` בכל ממצאי ספק או מעקב
  פריטי פעולה. עד לסיום הסקירה החיצונית, ספר ההפעלה הזה משמש כ-
  מבקרי בסיס מבצעיים יכולים לבדוק מול.