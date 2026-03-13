<!-- Hebrew translation of docs/source/torii/router.md -->

---
lang: he
direction: rtl
source: docs/source/torii/router.md
status: complete
translator: manual
---

<div dir="rtl">

# הרכבת ה-Router של Torii

Torii חושפת את ממשק ה-HTTP שלה באמצעות Builder מדורג שמאגד קבוצות של מסלולים ומחבר מצב משותף פעם אחת בלבד. המחלקה `RouterBuilder`, שממוקמת ב-`iroha_torii::router::builder`, עוטפת את `axum::Router<SharedAppState>`. כל `Torii::add_*_routes` מקבלת כעת `&mut RouterBuilder` ומבצעת רישום דרך `RouterBuilder::apply`.

## דפוס ה-Builder

ה-API החדש מחליף את הפטרן הישן של “החזר Router חדש”. במקום להשיב אובייקט Router בכל קריאה, מזמנים את העוזרים ברצף:

```rust
let mut builder = RouterBuilder::new(app_state.clone());

// קבוצות ניטרליות לפיצ'רים
torii.add_sumeragi_routes(&mut builder);
torii.add_telemetry_routes(&mut builder);
torii.add_core_info_routes(&mut builder);

// קבוצות אופציונליות
#[cfg(feature = "connect")]
torii.add_connect_routes(&mut builder);
#[cfg(feature = "app_api")]
torii.add_app_api_routes(&mut builder);

let router = builder.finish();
```

מתכונת זו מבטיחה כי:

- `AppState` משותף משוכפל רק בעת הצורך (המתודה `apply_with_state` עדיין זמינה למעבדים הדורשים זאת).
- עוזרים הנתונים לתכונות מותנות-קומפילציה מתחברים באופן דטרמיניסטי ללא קשר להגדרות קומפילציה.
- מבחנים יכולים להריץ שילובים שונים בלי “ללהטט” ב-Router ביניים.

## בדיקות רגרסיה

בדיקת האינטגרציה `crates/iroha_torii/tests/router_feature_matrix.rs` בונה את Torii עם סטאק בזיכרון ומוודאת שה-router נבנה תחת סט הפיצ'רים הפעיל. כאשר נקודת הקצה של Schema/OpenAPI קומפלה, הבדיקה יכולה להשוות את המסמך שנוצר מול Snapshot:

- קבעו `IROHA_TORII_OPENAPI_EXPECTED=/path/to/openapi.json` כדי לאמת שהפלט תואם לסנאפשוט.
- ניתן להגדיר `IROHA_TORII_OPENAPI_ACTUAL=/tmp/iroha-openapi.json` כדי לכתוב את הפלט הנוכחי לצורך בדיקה ידנית או כלי diff.

אם נקודת הקצה של OpenAPI אינה זמינה, ההשוואה תדלג באופן אוטומטי. הבדיקה מריצה גם קריאות Smoke מותנות-פיצ'ר (למשל `/v2/domains` עם `app_api`, ‏`/v2/connect/status` עם `connect`) כדי להבטיח שה-builder המדורג מכסה את כל קבוצות המסלולים.

## הערות הגירה

פרויקטים המש嵌Torii צריכים לעבור מכל מניפולציה ישירה ב-Router למתודות `RouterBuilder`. התבנית ההיסטורית של החזרת Router מעודכן מתוך `add_*_routes` הוצאה משימוש ואינה נתמכת עוד בתוך הקרייט.

## קריאה נוספת

- `crates/iroha_torii/docs/mcp_api.md` — חוזה ה־MCP JSON-RPC הקנוני של Torii עבור אינטגרציות agent/tool (`/v2/mcp`).

</div>
