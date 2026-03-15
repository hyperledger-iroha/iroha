---
lang: he
direction: rtl
source: docs/source/da/rent_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7cdc46bcd87af7924817a94900c8fad2c23570607f4065f19d8a42d259fe83f
source_last_modified: "2026-01-22T15:38:30.661606+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# מדיניות השכרה וזמינות נתונים (DA-7)

_סטטוס: שרטוט — בעלים: WG Economics / Treasury / Storage Team_

פריט מפת הדרכים **DA-7** מציג שכר דירה מפורש הנקוב ב-XOR עבור כל גוש
הוגש ל-`/v1/da/ingest`, בתוספת בונוסים המתגמלים ביצוע PDP/PoTR ו
יציאה מוגשת כדי להביא לקוחות. מסמך זה מגדיר את הפרמטרים הראשוניים,
ייצוג מודל הנתונים שלהם, וזרימת העבודה בחישוב המשמשת את Torii,
ערכות SDK ולוחות מחוונים של משרד האוצר.

## מבנה מדיניות

המדיניות מקודדת כ-[`DaRentPolicyV1`](/crates/iroha_data_model/src/da/types.rs)
בתוך מודל הנתונים. Torii וכלי ממשל ממשיכים את המדיניות
Norito מטענים כדי שניתן יהיה לחשב מחדש הצעות מחיר לשכירות ופנקסי תמריצים
באופן דטרמיניסטי. הסכימה חושפת חמישה כפתורים:

| שדה | תיאור | ברירת מחדל |
|-------|----------------|--------|
| `base_rate_per_gib_month` | XOR מחויב לכל GiB לחודש שמירה. | `250_000` מיקרו-XOR (0.25 XOR) |
| `protocol_reserve_bps` | חלק משכר הדירה מנותב לעתודה של הפרוטוקול (נקודות בסיס). | `2_000` (20%) |
| `pdp_bonus_bps` | אחוז בונוס לכל הערכת PDP מוצלחת. | `500` (5%) |
| `potr_bonus_bps` | אחוז בונוס לכל הערכת PoTR מוצלחת. | `250` (2.5%) |
| `egress_credit_per_gib` | אשראי ששולם כאשר ספק מגיש 1GiB של נתוני DA. | `1_500` micro-XOR |

כל ערכי נקודת הבסיס מאומתים מול `BASIS_POINTS_PER_UNIT` (10000).
עדכוני מדיניות חייבים לעבור דרך הממשל, וכל צומת Torii חושף את
מדיניות פעילה דרך סעיף התצורה `torii.da_ingest.rent_policy`
(`iroha_config`). מפעילים יכולים לעקוף את ברירות המחדל ב-`config.toml`:

```toml
[torii.da_ingest.rent_policy]
base_rate_per_gib_month_micro = 250000        # 0.25 XOR/GiB-month
protocol_reserve_bps = 2000                   # 20% protocol reserve
pdp_bonus_bps = 500                           # 5% PDP bonus
potr_bonus_bps = 250                          # 2.5% PoTR bonus
egress_credit_per_gib_micro = 1500            # 0.0015 XOR/GiB egress credit
```

כלי CLI (`iroha app da rent-quote`) מקבל את אותן כניסות מדיניות Norito/JSON
ופולט חפצי אמנות המשקפים את `DaRentPolicyV1` הפעיל מבלי להגיע
חזרה למצב Torii. ספק את תמונת המצב של המדיניות המשמשת לריצת בליעה כדי שה
הציטוט נשאר ניתן לשחזור.

### חפצי אמנות מתמשכים להשכרה

הפעל את `iroha app da rent-quote --gib <size> --months <months> --quote-out <path>` ל
פולט גם את הסיכום על המסך וגם חפץ JSON מודפס יפה. הקובץ
מתעד `policy_source`, תמונת המצב המוטבעת של `DaRentPolicyV1`, המחושב
`DaRentQuote`, ו-`ledger_projection` נגזר (בסדרה באמצעות
[`DaRentLedgerProjection`](/crates/iroha_data_model/src/da/types.rs)) מה שהופך אותו מתאים ללוחות המחוונים של האוצר וספר החשבונות ISI
צינורות. כאשר `--quote-out` מצביע על ספרייה מקוננת, ה-CLI יוצר כל
הורים חסרים, כך שהמפעילים יכולים לתקן מיקומים כגון
`artifacts/da/rent_quotes/<timestamp>.json` לצד חבילות ראיות אחרות של DA.
צרף את החפץ לאישורי השכרה או ריצוי התאמה כך שה-XOR
פירוט (שכר דירה בסיס, רזרבה, בונוסים של PDP/PoTR וזיכויים יציאה) הוא
ניתן לשחזור. עברו את `--policy-label "<text>"` כדי לעקוף את האוטומטית
תיאור נגזר `policy_source` (נתיבי קבצים, ברירת מחדל משובצת וכו') עם
תג קריא לאדם כגון כרטיס ניהול או חשיש מניפסט; גזרות ה-CLI
ערך זה ודוחה מחרוזות ריקות/לבן בלבד, כך שהראיות המוקלטות
נשאר בר ביקורת.

```json
{
  "policy_source": "policy JSON `configs/da/rent_policy.json`",
  "gib": 10,
  "months": 3,
  "policy": { "...": "DaRentPolicyV1 fields elided" },
  "quote": { "...": "DaRentQuote breakdown" },
  "ledger_projection": {
    "rent_due": { "micro": 7500000 },
    "protocol_reserve_due": { "micro": 1500000 },
    "provider_reward_due": { "micro": 6000000 },
    "pdp_bonus_pool": { "micro": 375000 },
    "potr_bonus_pool": { "micro": 187500 },
    "egress_credit_per_gib": { "micro": 1500 }
  }
}
```קטע הקרנת פנקס חשבונות מזין ישירות ל-DA שכר חשבונות ISIs: זה
מגדיר את ה-XOR deltas המיועדים לעתודה של הפרוטוקול, תשלומי ספקים ו
את מאגר הבונוס לכל הוכחה ללא צורך בקוד תזמור בהתאמה אישית.

### הפקת תוכניות פנקס שכר דירה

הפעל את `iroha app da rent-ledger --quote <path> --payer-account <id> --treasury-account <id> --protocol-reserve-account <id> --provider-account <id> --pdp-bonus-account <id> --potr-bonus-account <id> --asset-definition xor#sora`
להמיר הצעת מחיר שכר דירה מתמשכת להעברות פנקס חשבונות ניתנות לביצוע. הפקודה
מנתח את `ledger_projection` המוטבע, פולט הוראות Norito `Transfer`
שגובים את דמי השכירות הבסיסיים לאוצר, מנתבים את השמורה/ספק
מנות, ומממן מראש את מאגר הבונוס של PDP/PoTR ישירות מהמשלם. ה
פלט JSON משקף את המטא-נתונים של הציטוט כך ש-CI וכלי האוצר יכולים לנמק
על אותו חפץ:

```json
{
  "quote_path": "artifacts/da/rent_quotes/2025-12-07/rent.json",
  "rent_due_micro_xor": 7500000,
  "protocol_reserve_due_micro_xor": 1500000,
  "provider_reward_due_micro_xor": 6000000,
  "pdp_bonus_pool_micro_xor": 375000,
  "potr_bonus_pool_micro_xor": 187500,
  "egress_credit_per_gib_micro_xor": 1500,
  "instructions": [
    { "Transfer": { "...": "payer -> treasury base rent instruction elided" }},
    { "Transfer": { "...": "treasury -> reserve" }},
    { "Transfer": { "...": "treasury -> provider payout" }},
    { "Transfer": { "...": "payer -> PDP bonus escrow" }},
    { "Transfer": { "...": "payer -> PoTR bonus escrow" }}
  ]
}
```

השדה האחרון `egress_credit_per_gib_micro_xor` מאפשר לוחות מחוונים ותשלום
מתזמנים מיישרים החזרי יציאה עם מדיניות שכר הדירה שיצרה את
ציטוט מבלי לחשב מחדש את מתמטיקה של המדיניות בדבק סקריפטים.

## ציטוט לדוגמה

```rust
use iroha_data_model::da::types::DaRentPolicyV1;

// 10 GiB retained for 3 months.
let policy = DaRentPolicyV1::default();
let quote = policy.quote(10, 3).expect("policy validated");

assert_eq!(quote.base_rent.as_micro(), 7_500_000);      // 7.5 XOR total rent
assert_eq!(quote.protocol_reserve.as_micro(), 1_500_000); // 20% reserve
assert_eq!(quote.provider_reward.as_micro(), 6_000_000);  // Direct provider payout
assert_eq!(quote.pdp_bonus.as_micro(), 375_000);          // PDP success bonus
assert_eq!(quote.potr_bonus.as_micro(), 187_500);         // PoTR success bonus
assert_eq!(quote.egress_credit_per_gib.as_micro(), 1_500);
```

ניתן לשחזר את הציטוט על פני צמתי Torii, SDKs ודוחות משרד האוצר מכיוון
הוא משתמש במבני Norito דטרמיניסטיים במקום מתמטיקה אד-הוק. מפעילים יכולים
צרף את ה-JSON/CBOR המקודד `DaRentPolicyV1` להצעות ממשל או להשכרה
ביקורות כדי להוכיח אילו פרמטרים היו בתוקף עבור כל גוש נתון.

## בונוסים ורזרבות

- **עתודה של פרוטוקול:** `protocol_reserve_bps` מממנת את עתודה XOR שמגבה
  שכפול מחדש חירום וקיצוץ החזרים. האוצר עוקב אחר הדלי הזה
  בנפרד כדי להבטיח שייתרות פנקס חשבונות תואמות את התעריף המוגדר.
- **בונוסים של PDP/PoTR:** כל הערכת הוכחה מוצלחת מקבלת תוספת נוספת
  התשלום נגזר מ-`base_rent × bonus_bps`. כאשר מתזמן ה-DA פולט הוכחה
  קבלות זה כולל את תגי נקודת הבסיס כך שניתן להפעיל מחדש תמריצים.
- **זיכוי יציאה:** ספקים רושמים GiB שהוגשו לכל מניפסט, הכפל ב
  `egress_credit_per_gib`, ושלח את הקבלות דרך `iroha app da prove-availability`.
  מדיניות השכירות שומרת על הסכום ל-GiB מסונכרן עם הממשל.

## זרימה תפעולית

1. **הטמעה:** `/v1/da/ingest` טוען את `DaRentPolicyV1` הפעיל, מצטט שכר דירה
   מבוסס על גודל כתמים ושימור, ומטמיע את הציטוט ב-Norito
   מתגלה. המגיש חותם על הצהרה המתייחסת לחשב השכירות ו
   מזהה כרטיס האחסון.
2. **חשבונאות:** משרד האוצר אינסט סקריפטים מפענחים את המניפסט, התקשר
   `DaRentPolicyV1::quote`, ואכלס פנקסי שכר דירה (שכר דירה בסיס, מילואים,
   בונוסים וזיכויים צפויים ליציאה). כל אי התאמה בין שכר הדירה שנרשם
   והצעות מחיר מחושבות מחדש נכשלות ב-CI.
3. **תגמולי הוכחה:** כאשר מתזמני PDP/PoTR מסמנים הצלחה הם פולטים קבלה
   המכיל את תקציר המניפסט, סוג ההוכחה והבונוס XOR שנגזר ממנו
   את המדיניות. ממשל יכול לבקר את התשלומים על ידי חישוב מחדש של אותה הצעת מחיר.
4. **החזר יציאה:** מתזמני אחזור מגישים סיכומי יציאה חתומים.
   Torii מכפיל את ספירת GiB ב-`egress_credit_per_gib` ומנפיק תשלום
   הוראות נגד נאמנות שכר הדירה.

## טלמטריהצמתי Torii חושפים את השימוש בשכר דירה באמצעות מדדי Prometheus הבאים (תוויות:
`cluster`, `storage_class`):

- `torii_da_rent_gib_months_total` - חודשי GiB שצוטטו על ידי `/v1/da/ingest`.
- `torii_da_rent_base_micro_total` - שכר דירה בסיס (מיקרו XOR) שנצבר בעת הצריכה.
- `torii_da_protocol_reserve_micro_total` - תרומות עתודות פרוטוקול.
- `torii_da_provider_reward_micro_total` - תשלומי שכר דירה בצד הספק.
- `torii_da_pdp_bonus_micro_total` ו-`torii_da_potr_bonus_micro_total` —
  מאגרי בונוס של PDP/PoTR שמקורם בהצעת המחיר.

לוחות מחוונים לכלכלה מסתמכים על מונים אלה כדי להבטיח ISIs של ספרי חשבונות, ברזי מילואים,
לוחות הזמנים של בונוס PDP/PoTR תואמים כולם לפרמטרי המדיניות התקפים עבור כל אחד מהם
אשכול ומחלקת אחסון. לוח SoraFS Capacity Health Grafana
(`dashboards/grafana/sorafs_capacity_health.json`) מציג כעת לוחות ייעודיים
עבור חלוקת שכר דירה, צבירת בונוס PDP/PoTR ותפיסת חודשי GiB, מה שמאפשר
האוצר יסנן לפי אשכול Torii או מחלקת אחסון בעת סקירת הכנסה
נפח ותשלומים.

## השלבים הבאים

- ✅ קבלות `/v1/da/ingest` מוטמעות כעת `rent_quote` ומשטחי CLI/SDK מציגים את המצוטט
  שכר דירה בסיס, נתח מילואים ובונוסים של PDP/PoTR כך שהשולחים יוכלו לעיין בהתחייבויות XOR לפני כן
  ביצוע מטענים.
- שלב את ספר שכר הדירה עם עדכוני המוניטין/פנקסי ההזמנות הקרובים של DA
  כדי להוכיח שספקי זמינות גבוהה מקבלים את התשלומים הנכונים.