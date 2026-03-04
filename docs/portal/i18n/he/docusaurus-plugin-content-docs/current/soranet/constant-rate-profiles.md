---
id: constant-rate-profiles
lang: he
direction: rtl
source: docs/portal/docs/soranet/constant-rate-profiles.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note מקור קנוני
דף זה משקף את `docs/source/soranet/constant_rate_profiles.md`. שמרו על שתי הגרסאות מסונכרנות עד שהסט הישן של התיעוד יופסק.
:::

SNNet-17B מציג נתיבי תעבורה בקצב קבוע כך ש-relays מעבירים תעבורה בתאי 1,024 B ללא קשר לגודל ה-payload. המפעילים בוחרים משלושה presets:

- **core** - relays במרכזי נתונים או באירוח מקצועי שיכולים להקדיש >=30 Mbps לכיסוי תעבורה.
- **home** - מפעילים ביתיים או בעלי uplink נמוך שעדיין צריכים fetches אנונימיים עבור מעגלים רגישי פרטיות.
- **null** - preset dogfood של SNNet-17A2. הוא משאיר את אותם TLVs/envelope אבל מותח את ה-tick ואת ה-ceiling עבור staging ברוחב פס נמוך.

## סיכום presets

| פרופיל | Tick (ms) | תא (B) | תקרת lanes | רצפת dummy | Payload לכל lane (Mb/s) | Payload תקרה (Mb/s) | % תקרה מה-uplink | Uplink מומלץ (Mb/s) | תקרת neighbors | טריגר נטרול אוטומטי (%) |
|---------|-----------|----------|----------|-------------|-------------------------|------------------------|---------------------|----------------------------|--------------|--------------------------|
| core    | 5.0       | 1024     | 12       | 4           | 1.64                    | 19.50                  | 65                  | 30.0                       | 8            | 85                       |
| home    | 10.0      | 1024     | 4        | 2           | 0.82                    | 4.00                   | 40                  | 10.0                       | 2            | 70                       |
| null    | 20.0      | 1024     | 2        | 1           | 0.41                    | 0.75                   | 15                  | 5.0                        | 1            | 55                       |

- **Lane cap** - מספר ה-neighbors המרבי בו-זמנית בקצב קבוע. ה-relay דוחה מעגלים נוספים לאחר שהתקרה הושגה ומגדיל את `soranet_handshake_capacity_reject_total`.
- **Dummy floor** - מספר ה-lanes המינימלי שנשאר חי עם תעבורת dummy גם כשהביקוש בפועל נמוך יותר.
- **Ceiling payload** - תקציב ה-uplink המוקצה ל-lanes בקצב קבוע לאחר החלת מקדם ה-ceiling. המפעילים לא צריכים לעבור תקציב זה גם אם זמינה רוחב פס נוסף.
- **Auto-disable trigger** - אחוז רוויה מתמשך (ממוצע לפי preset) שגורם ל-runtime לרדת לרצפת ה-dummy. הקיבולת מוחזרת אחרי סף ההתאוששות (75% עבור `core`, 60% עבור `home`, 45% עבור `null`).

**חשוב:** preset `null` מיועד ל-staging ול-dogfooding של יכולות בלבד; הוא לא עומד בהבטחות הפרטיות הנדרשות למעגלי פרודקשן.

## טבלת tick -> bandwidth

כל תא payload נושא 1,024 B, לכן עמודת KiB/sec שווה למספר התאים שנשלחים בשנייה. השתמשו ב-helper כדי להרחיב את הטבלה עם ticks מותאמים.

| Tick (ms) | תאים/שנייה | Payload KiB/sec | Payload Mb/s |
|-----------|-----------|-----------------|--------------|
| 5.0       | 200.00    | 200.00          | 1.64         |
| 7.5       | 133.33    | 133.33          | 1.09         |
| 10.0      | 100.00    | 100.00          | 0.82         |
| 15.0      | 66.67     | 66.67           | 0.55         |
| 20.0      | 50.00     | 50.00           | 0.41         |

נוסחה:

```
payload_mbps = (cell_bytes x 8 / 1_000_000) x (1000 / tick_ms)
```

עוזר CLI:

```bash
# Markdown table output for all presets plus default tick table
cargo xtask soranet-constant-rate-profile --tick-table --format markdown --json-out artifacts/soranet/constant_rate/report.json

# Restrict to a preset and emit JSON
cargo xtask soranet-constant-rate-profile --profile core --format json

# Custom tick series
cargo xtask soranet-constant-rate-profile --tick-table --tick-values 5,7.5,12,18 --format markdown
```

הדגל `--format markdown` מפיק טבלאות בסגנון GitHub גם לסיכום ה-presets וגם לדף העזר של ticks, כך שתוכלו להדביק פלט דטרמיניסטי לפורטל. חברו אותו ל-`--json-out` כדי לארכב את הנתונים המרונדרים כראיות ממשל.

## תצורה ו-overrides

`tools/soranet-relay` מציג את ה-presets גם בקבצי תצורה וגם ב-overrides בזמן ריצה:

```bash
# Persisted in relay.json
"constant_rate_profile": "home"

# One-off override during rollout or maintenance
soranet-relay --config relay.json --constant-rate-profile core
```

מפתח התצורה מקבל `core`, `home`, או `null` (ברירת מחדל `core`). Overrides דרך CLI שימושיים לתרגילי staging או לבקשות SOC שמפחיתות זמנית את ה-duty cycle בלי לשכתב configs.

## הגנות MTU

- תאי payload משתמשים ב-1,024 B ועוד כ-96 B של framing מסוג Norito+Noise וכותרות QUIC/UDP מינימליות, כך שכל datagram נשאר מתחת ל-MTU המינימלי של IPv6 בגודל 1,280 B.
- כאשר מנהרות (WireGuard/IPsec) מוסיפות עטיפה נוספת חייבים **בהכרח** להקטין את `padding.cell_size` כך ש-`cell_size + framing <= 1,280 B`. מאמת ה-relay אוכף `padding.cell_size <= 1,136 B` (1,280 B - 48 B overhead של UDP/IPv6 - 96 B framing).
- פרופילי `core` צריכים להצמיד >=4 neighbors גם במצב idle כדי ש-lanes של dummy תמיד יכסו תת-קבוצה של guards PQ. פרופילי `home` יכולים להגביל מעגלים בקצב קבוע ל-wallets/aggregators, אבל חייבים להפעיל back-pressure כאשר הרוויה עולה על 70% במשך שלושה חלונות טלמטריה.

## טלמטריה והתראות

Relays מייצאים את המדדים הבאים לכל preset:

- `soranet_constant_rate_active_neighbors`
- `soranet_constant_rate_queue_depth`
- `soranet_constant_rate_saturation_percent`
- `soranet_constant_rate_dummy_lanes` / `soranet_constant_rate_dummy_ratio`
- `soranet_constant_rate_slot_rate_hz`
- `soranet_constant_rate_ceiling_hits_total`
- `soranet_constant_rate_degraded`

התריעו כאשר:

1. יחס ה-dummy נשאר מתחת לרצפת ה-preset (`core >= 4/8`, `home >= 2/2`, `null >= 1/1`) ליותר משני חלונות.
2. `soranet_constant_rate_ceiling_hits_total` גדל מהר יותר מפגיעה אחת לכל חמש דקות.
3. `soranet_constant_rate_degraded` מתהפך ל-`1` מחוץ לתרגיל מתוכנן.

רשמו את תווית ה-preset ואת רשימת ה-neighbors בדוחות אירוע כדי שהמבקרים יוכלו להוכיח שמדיניות הקצב הקבוע תאמה את דרישות מפת הדרכים.
