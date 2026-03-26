---
id: suffix-catalog
lang: he
direction: rtl
source: docs/portal/docs/sns/suffix-catalog.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# קטלוג סיומות Sora Name Service

ה-roadmap של SNS עוקב אחר כל סיומת מאושרת (SN-1/SN-2). עמוד זה משקף את קטלוג
מקור האמת כך שמפעילים שמריצים registrars, DNS gateways או tooling של ארנקים
יכולים לטעון את אותם פרמטרים בלי לגרד את מסמכי הסטטוס.

- **Snapshot:** [`docs/examples/sns/suffix_catalog_v1.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/examples/sns/suffix_catalog_v1.json)
- **Consumers:** `iroha sns policy`, ערכות onboarding ל-SNS, דשבורדים של KPI, וסקריפטי
  release של DNS/Gateway קוראים את אותו bundle JSON.
- **Statuses:** `active` (הרשמות מותרות), `paused` (מוגבל זמנית), `revoked` (הוכרז אך
  לא זמין כעת).

## סכמת הקטלוג

| שדה | סוג | תיאור |
|-----|-----|-------|
| `suffix` | string | סיומת קריאה לבני אדם עם נקודה מובילה. |
| `suffix_id` | `u16` | מזהה המאוחסן ב-ledger בתוך `SuffixPolicyV1::suffix_id`. |
| `status` | enum | `active`, `paused` או `revoked` המתארים מוכנות להשקה. |
| `steward_account` | string | חשבון אחראי על stewardship (תואם ל-hooks של מדיניות registrar). |
| `fund_splitter_account` | string | חשבון שמקבל תשלומים לפני הניתוב לפי `fee_split`. |
| `payment_asset_id` | string | נכס שמשמש ל-settlement (`61CtjvNd9T3THAR65GsMVHr82Bjc` למחזור הראשוני). |
| `min_term_years` / `max_term_years` | integer | גבולות תקופת רכישה מהמדיניות. |
| `grace_period_days` / `redemption_period_days` | integer | חלונות בטיחות לחידוש שנאכפים על ידי Torii. |
| `referral_cap_bps` | integer | תקרת referral carve-out מותרת על ידי ממשל (basis points). |
| `reserved_labels` | array | אובייקטי תווית מוגנים בממשל `{label, assigned_to, release_at_ms, note}`. |
| `pricing` | array | אובייקטי tier עם `label_regex`, `base_price`, `auction_kind`, וגבולות משך. |
| `fee_split` | object | `{treasury_bps, steward_bps, referral_max_bps, escrow_bps}` חלוקה ב-basis points. |
| `policy_version` | integer | מונה מונוטוני שמוגדל כשממשל עורך את המדיניות. |

## הקטלוג הנוכחי

| סיומת | מזהה (`hex`) | Steward | Fund splitter | סטטוס | נכס תשלום | תקרת referral (bps) | תקופה (min - max שנים) | Grace / Redemption (ימים) | דרגות מחיר (regex -> מחיר בסיס / מכרז) | תוויות שמורות | חלוקת fees (T/S/R/E bps) | גרסת מדיניות |
|-------|-------------|---------|---------------|--------|------------|---------------------|-------------------------|----------------------------|----------------------------------------|---------------|---------------------------|-------------|
| `.sora` | `0x0001` | `soraカタカナ...` | `soraカタカナ...` | פעיל | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 500 | 1-5 | 30 / 60 | `T0: ^[a-z0-9]{3,}$ -> 120 XOR (Vickrey)` | `treasury -> soraカタカナ...` | `7000 / 3000 / 1000 / 0` | 1 |
| `.nexus` | `0x0002` | `soraカタカナ...` | `soraカタカナ...` | מושהה | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 300 | 1-3 | 15 / 30 | `T0: ^[a-z0-9]{4,}$ -> 480 XOR (Vickrey)`<br>`T1: ^[a-z]{2}$ -> 4000 XOR (Dutch floor 500)` | `treasury -> soraカタカナ...`, `guardian -> soraカタカナ...` | `6500 / 2500 / 800 / 200` | 2 |
| `.dao` | `0x0003` | `soraカタカナ...` | `soraカタカナ...` | מבוטל | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 0 | 1-2 | 30 / 30 | `T0: ^[a-z0-9]{3,}$ -> 60 XOR (Vickrey)` | `dao (held for future release)` | `9000 / 1000 / 0 / 0` | 0 |

## קטע JSON

```json
{
  "version": 1,
  "generated_at": "2026-05-01T00:00:00Z",
  "suffixes": [
    {
      "suffix": ".sora",
      "suffix_id": 1,
      "status": "active",
      "fund_splitter_account": "soraカタカナ...",
      "payment_asset_id": "61CtjvNd9T3THAR65GsMVHr82Bjc",
      "referral_cap_bps": 500,
      "pricing": [
        {
          "tier_id": 0,
          "label_regex": "^[a-z0-9]{3,}$",
          "base_price": {"asset_id": "61CtjvNd9T3THAR65GsMVHr82Bjc", "amount": 120},
          "auction_kind": "vickrey_commit_reveal",
          "min_duration_years": 1,
          "max_duration_years": 5
        }
      ],
      "...": "see docs/examples/sns/suffix_catalog_v1.json for the full record"
    }
  ]
}
```

## הערות אוטומציה

1. טענו את snapshot ה-JSON ובצעו hash/חתימה לפני הפצה למפעילים.
2. כלי registrar צריכים להציג `suffix_id`, מגבלות תקופה ותמחור מהקטלוג בכל
   פגיעה של בקשה ב-`/v1/sns/*`.
3. עזרי DNS/Gateway קוראים את מטא-דאטה התוויות השמורות בעת יצירת תבניות GAR כדי
   שתשובות DNS ישארו מיושרות עם בקרות הממשל.
4. משימות KPI annex מתייגות exports של דשבורדים במטא-דאטה של סיומת כדי שההתראות
   יתאימו למצב ההשקה שנרשם כאן.
