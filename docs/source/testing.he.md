<!-- Hebrew translation of docs/source/testing.md -->

---
lang: he
direction: rtl
source: docs/source/testing.md
status: complete
translator: manual
---

<div dir="rtl">

# מדריך בדיקות ופתרון תקלות

המדריך מפרט כיצד לשחזר תרחישי אינטגרציה, אילו תשתיות חייבות לפעול וכיצד לאסוף לוגים שימושיים. לפני שמתחילים, עיינו ב[דוח הסטטוס](../../status.md) כדי לדעת אילו רכיבים בירוק.

## צעדי שחזור

### בדיקות אינטגרציה (crate בשם `integration_tests`)

1. בנו את ה-workspace: `cargo build --workspace`
2. הריצו את הסוויטה עם לוגים מלאים: `cargo test -p integration_tests -- --nocapture`
3. להרצת תרחיש ייעודי: `cargo test -p integration_tests settlement::happy_path -- --nocapture`
4. צרו fixtures סריאליים בעזרת Norito כדי ליישר קלטים בין צמתים:
   ```rust
   use norito::json;

   let genesis_payload = json::to_string_pretty(&json::json!({
       "chain" : "testnet",
       "peers" : ["127.0.0.1:1337"],
       "accounts" : [{
           "id" : "ih58...",
           "public_key" : "ed0120..."
       }]
   }))?;
   ```
   שמרו את ה-Norito JSON שנוצר יחד עם ארטיפקטי הבדיקה כך שפירים אחרים יוכלו לשחזר את אותו מצב.

### בדיקות Python (`pytests`)

1. התקינו תלויות: `pip install -r pytests/requirements.txt` (בסביבה וירטואלית).
2. הפיצו את ה-fixtures התואמים ל-Norito דרך נתיב משותף או משתנה סביבה.
3. הריצו את הסוויטה עם פירוט: `pytest -vv pytests`
4. לניפוי ממוקד: `pytest -k "Query" pytests/tests/test_queries.py --log-cli-level=INFO`

## שירותים ופורטים נדרשים

ודאו שהשירותים הבאים זמינים לפני הרצת הסוויטות:

- **Torii HTTP API**: ברירת מחדל `127.0.0.1:1337`. הגדירו `torii.address` בקובץ התצורה (ראו `docs/source/references/peer.template.toml`).
- **התראות WebSocket של Torii**: ברירת מחדל `127.0.0.1:8080` – נדרש עבור `pytests`.
- **יצואן טלמטריה**: ברירת מחדל `127.0.0.1:8180`. בדיקות אינטגרציה מצפות שהמדדים יזרמו אליו.
- **PostgreSQL** (אם מופעל): ברירת מחדל `127.0.0.1:5432`. ודאו שהקרדיטשיאלס זהים לפרופיל בקובץ [`defaults/docker-compose.local.yml`](../../defaults/docker-compose.local.yml).

אם שירות אינו זמין, ראו את [מדריך הטלמטריה](telemetry.md).

### יציבות peers משובצים

`NetworkBuilder::start()` מיישם חלון live של חמש שניות אחרי genesis לכל peer משובץ. אם תהליך נופל בזמן זה, הבילדר נעצר עם שגיאה מפורטת שמצביעה על לוגים שנשמרו. בסביבות דלות משאבים ניתן להאריך את החלון (ב-milliseconds) בעזרת `IROHA_TEST_POST_GENESIS_LIVENESS_MS`; ערך `0` מבטל את ההגנה. ודאו שיש די CPU בשניות הראשונות של כל ריצה כדי שהפירים יגיעו לבלוק 1.

## איסוף וניתוח לוגים

התחילו מספרייה נקייה כדי שארטיפקטים ישנים לא יסתירו בעיות חדשות. הסקריפטים הבאים מייצרים פורמטים שמתאימים לכלי Norito.

- לאחר הבדיקות, הריצו [`scripts/analyze_telemetry.sh`](../../scripts/analyze_telemetry.sh) כדי לרכז מדדי צמתים ל-Norito JSON ממוספר בזמן.
- לתקלות רשת, הריצו [`scripts/run_iroha_monitor_demo.py`](../../scripts/run_iroha_monitor_demo.py) להזרים אירועי Torii אל `monitor_output.norito.json`.
- לוגי אינטגרציה שמורים ב-`integration_tests/target/`; דחסו אותם עם [`scripts/profile_build.sh`](../../scripts/profile_build.sh) לשיתוף.
- לוגי Python נשמרים תחת `pytests/.pytest_cache`. אספו אותם יחד עם הטלמטריה:
  ```bash
  ./scripts/report_red_team_failures.py --tests pytests --artifacts out/logs
  ```

איספו צרור מלא (אינטגרציה, Python, טלמטריה) לפני פתיחת issue כדי שהמתחזקים יוכלו לשחזר את עקבות ה-Norito.

## צעדים הבאים

לרשימות בדיקה ייעודיות לגרסה, ראו [pipeline](pipeline.md). אם נמצאה רגרסיה או כשל, תעדו זאת ב-[מעקב הסטטוס](../../status.md) והוסיפו קישורים רלוונטיים ל-[sumeragi troubleshooting](sumeragi.md).

</div>
