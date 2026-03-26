<!-- Hebrew translation of docs/source/query_json.md -->

---
lang: he
direction: rtl
source: docs/source/query_json.md
status: complete
translator: manual
---

<div dir="rtl">

# מעטפת JSON לשאילתות

Iroha מספקת נקודת קצה מבוססת Norito בכתובת `/query` לקבלת מסגרות חתומות. לכלי CLI וסקריפטים נוח לנסח את הבקשה ב-JSON ולאפשר לכלי להמיר אותה ל-`SignedQuery` חתום. המודול `iroha_data_model::query::json` מגדיר את המעטפת הקנונית בשימוש `iroha_cli ledger query stdin` וכלי עזר נוספים.

## מבנה המעטפת

המסמך ברמה העליונה הוא אובייקט שמכיל מקטע `singular` או `iterable` (בדיוק אחד מהם):

```json
{"singular": { /* singular query */ }}
{"iterable": { /* iterable query */ }}
```

בקשות שמכילות את שני המקטעים או שאינן מכילות אף אחד נדחות.

## שאילתות סינגולריות

שאילתה סינגולרית מזהה את השאילתה לפי שם ויכולה לצרף מטען אופציונלי:

```json
{
  "singular": {
    "type": "FindContractManifestByCodeHash",
    "payload": {
      "code_hash": "0x00112233…"
    }
  }
}
```

השאילתות הסינגולריות הנתמכות:

- `FindAbiVersion`
- `FindExecutorDataModel`
- `FindParameters`
- `FindContractManifestByCodeHash` (דורש מחרוזת הקס באורך 32 בתים בשדה `code_hash`)

## שאילתות איטרביליות

שאילתות איטרביליות מזהות את השאילתה ויכולות לכלול פרמטרי הרצה ופרדיקט:

```json
{
  "iterable": {
    "type": "FindDomains",
    "params": {
      "limit": 25,
      "offset": 10,
      "fetch_size": 50,
      "sort_by_metadata_key": "ui.order",
      "order": "Desc",
      "ids_projection": false,
      "lane_id": null,   // שמור למעבר עתידי על מסלולי Cursor
      "dsid": null       // שמור למזהי shards עתידיים
    },
    "predicate": {
      "equals": [
        {"field": "authority", "value": "<katakana-i105-account-id>"}
      ],
      "in": [
        {"field": "metadata.tier", "values": [1, 2, 3]}
      ],
      "exists": ["metadata.display_name"]
    }
  }
}
```

### פרמטרים

אובייקט `params` האופציונלי מגדיר עימוד ומיון:

- `limit` (`u64`, אופציונלי) — מספר פריטים מרבי לחילוץ.
- `offset` (`u64`, ברירת מחדל `0`) — מספר פריטים לדילוג.
- `fetch_size` (`u64`, אופציונלי) — גודל אצווה לזרימת cursor.
- `sort_by_metadata_key` (`string`, אופציונלי) — מפתח מטא־דאטה למיון יציב.
- `order` (`"Asc"` | `"Desc"`, אופציונלי) — סדר המיון; מתקבל רק כאשר הוגדר מפתח מטא־דאטה.
- `ids_projection` (`bool`, אופציונלי) — מבקש תגובה המורכבת ממזהים בלבד כאשר הצומת מקומפל עם הפיצ'ר הניסיוני `ids_projection`.
- `lane_id`, `dsid` (אופציונלי) — שדות שמורים למסלולי cursor וניתוב shards עתידיים. הפרסר מקבל אותם אך מתעלם (TBD).

כל המגבלות המספריות חייבות להיות שונה מאפס כאשר הן מסופקות. מפתח המיון נבדק לפי כללי [`Name`](../../crates/iroha_data_model/src/name.rs).

### DSL מינימלי לפרדיקטים

הפרדיקט מיוצג כאובייקט עם שלושה מערכים אופציונליים:

- `equals`: רשימת אובייקטים `{ "field": <path>, "value": <json value> }`.
- `in`: רשימת אובייקטים `{ "field": <path>, "values": [<json value>, …] }` עם רשימות ערכים לא ריקות.
- `exists`: רשימת מסלולי שדות שחייבים להיות קיימים (לא null).

מסלולי שדות משתמשים ברשימה מופרדת בנקודות (`metadata.display_name`, ‏`authority` וכדומה). המקודד מנרמל את הפרדיקט על ידי מיון הסעיפים לפי שם שדה כדי להבטיח חתימות דטרמיניסטיות.

## שימוש ב-CLI

ה-CLI קורא את המעטפת מ-stdin, חותם באמצעות החשבון המוגדר ושולח ל-`/query`:

```shell
$ cargo run -p iroha_cli -- query stdin <<'JSON'
{
  "iterable": {
    "type": "FindDomains",
    "params": {"limit": 5, "sort_by_metadata_key": "ui.order", "order": "Asc"},
    "predicate": {"exists": ["metadata.display_name"]}
  }
}
JSON
```

התגובה מודפסת בהתאם לפורמט הפלט שנבחר. ניתן להמיר את אותה מעטפת למסגרת חתומה תכניתית בעזרת `QueryEnvelopeJson::into_signed_request`. לעיון מפורט בממשק ה-Rust, ראו [`iroha_data_model::query::json`](../../crates/iroha_data_model/src/query/json).

</div>
