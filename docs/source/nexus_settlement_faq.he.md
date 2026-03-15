<!-- Hebrew translation for docs/source/nexus_settlement_faq.md -->

---
lang: he
direction: rtl
source: docs/source/nexus_settlement_faq.md
status: draft
translator: LLM (Codex)
---

# שאלות ותשובות – הסליקה של Nexus

**קישור לרודמפ:** NX-14 — תיעוד ורנבוקים למפעילי Nexus  
**סטטוס:** נוסח ראשון 24.03.2026 (מותאם למפרט רואטר הסליקה ולפלייבוק ה‑CBDC)  
**קהל יעד:** מפעילים, כותבי SDK וצוותי ממשל המכינים את השקת Nexus (Iroha 3).

מסמך זה מרכז את השאלות שעלו בביקורת NX-14 לגבי רואטר הסליקה, המרת XOR, טלמטריה ואסמכתאות. לספציפיקציה מלאה עיינו ב־`docs/source/settlement_router.md`; לפוליסות CBDC ייעודיות ראו `docs/source/cbdc_lane_playbook.md`.

> **בשתי מילים:** כל הזרימות נסגרות דרך Settlement Router, שמחייב באפן דטרמיניסטי את מאגרי ה‑XOR ומסבסד עמלות לפי ההגדרות ברמת הנתיבים. מפעילים חייבים לשמור על תיאום בין `config/config.toml`, לוחות המחוונים והיומנים החתומים.

## שאלות נפוצות

### אילו נתיבים מטפלים בסליקה ואיך יודעים איפה הדאטה־ספייס שלי?

- כל דאטה־ספייס מצהיר `settlement_handle` במניפסט. ברירות המחדל:
  - `xor_global` — הנתיבים הציבוריים הרגילים.
  - `xor_lane_weighted` — נתיבי ציבור עם נזילות חלופית.
  - `xor_hosted_custody` — נתיבי CBDC/פרטיים (מאגר XOR משומר).
  - `xor_dual_fund` — נתיבי היבריד/סודיים המשלבים מאגרים מוצפנים וציבוריים.
- ראו `docs/source/nexus_lanes.md` לסיווגי נתיבים ו-`docs/source/project_tracker/nexus_config_deltas/*.md` לאישורים העדכניים. הפקודה `irohad --sora --config … --trace-config` מדפיסה את הקטלוג הפעיל לביקורת.

### איך הרואטר מחשב שערי המרה?

- קיימת תעלה דטרמיניסטית אחת:
  - בנתיבים ציבוריים משתמשים בבריכת ה‑XOR הציבורית; כאשר הנזילות דלה, נופלים ל‑TWAP שאושר ע"י הממשל.
  - נתיבים פרטיים מממנים מראש מאגר XOR. בעת החיוב הרואטר מתעד `{lane_id, source_token, xor_amount, haircut}` ומיישם `haircut.rs` אם המאגרים חורגים.
- ההגדרות חיות תחת `[settlement]` ב־`config/config.toml`. אין לערוך ללא הנחיה. פירוט שדות נמצא ב־`docs/source/settlement_router.md`.

### איך מוחלים עמלות והחזרים?

- כל נתיב מגדיר במניפסט:
  - `base_fee_bps` — עמלה בסיסית לכל סליקה.
  - `liquidity_haircut_bps` — פיצוי לספקי הנזילות.
  - `rebate_policy` — אופציונלי (למשל מבצעי CBDC).
- הרואטר מפיק אירועי `SettlementApplied` בפורמט Norito עם פירוט העמלות כדי ש‑SDK ואודיטורים יוכלו ליישב מול הבלוקצ'יין.

### אילו טלמטריות מוכיחות שהסליקה בריאה?

- Prometheus:
  - `nexus_settlement_latency_seconds{lane_id}` — P99 < 900 ms לנתיבים ציבוריים / < 1200 ms לפרטיים.
  - `settlement_router_conversion_total{source_token}` — נפחי המרה.
  - `settlement_router_haircut_total{lane_id}` — התראה על ערכים חיוביים ללא הערת ממשל.
- השדה `lane_settlement_commitments[*].swap_metadata.volatility_class` מתעד אם הופעל מרווח יציב/מוגבר/מנותק; ערכים שאינם Stable חייבים קישור ליומן התקריות או להערת ממשל.
- לוחות מחוונים: `dashboards/grafana/nexus_settlement.json` ו־`nexus_lanes.json`. כללי התרעה ב־`dashboards/alerts/nexus_audit_rules.yml`.
- בהידרדרות, הפעילו את נוהל האירוע ב־`docs/source/nexus_operations.md`.

### מה מצפה הבקר?

1. **צילום תצורה** — `config/config.toml` (סעיף `[settlement]`) יחד עם קטלוג הנתיבים רלוונטי.
2. **יומני רואטר** — ארכיון יומי של `settlement_router.log` (מזהים, חיובי XOR, נקודות חיתוך).
3. **ייצוא טלמטריה** — צילום שבועי של המדדים לעיל.
4. **דוח התאמה** — מומלץ: `SettlementRecordV1` (ראה `docs/source/cbdc_lane_playbook.md`) מול יומן האוצר.

### האם ה‑SDK זקוקים לטיפול מיוחד?

- כן:
  - חשיפת עוזרים לקריאת `/v2/settlement/records` ופענוח `SettlementApplied`.
  - פרסום מזהי נתיב ו־handle בקונפיגורציה כך שמפעילים ינתבו נכון.
  - מימוש payload‑י Norito (`SettlementInstructionV1` וכו') ובדיקות E2E.
- עיינו ב־Nexus SDK Quickstart עבור דוגמאות פר־שפה.

### איך זה מתקשר לממשל ובלמי חירום?

- הממשל יכול להשעות handle דרך עדכון מניפסט; הרואטר יחזיר `ERR_SETTLEMENT_PAUSED`.
- "תקרות חיתוך" מגבילות חיובי XOR פר בלוק.
- נטרו `governance.settlement_pause_total` ופעלו לפי תבנית האירוע ב־`docs/source/nexus_operations.md`.

### איך מדווחים על באגים?

- פערי פיצ'רים → issue עם התג `NX-14` וקישור לרודמפ.
- אירועי סליקה דחופים → הזעקת Nexus primary עם היומן המצורף.
- תיקוני מסמכים → PR למסמך זה ולגרסת הפורטל (`docs/portal/docs/nexus/...`).

### דוגמאות לזרימות סליקה

להלן שלושה תרחישים. בכל מקרה יש ללכוד יומן רואטר, גיבוב טרנזקציה בטבלה והמדדים התואמים.

#### נתיב CBDC פרטי (`xor_hosted_custody`)

```text
2026-03-24T11:42:07Z settlement_router lane=3 dataspace=ds::cbdc::jp
    handle=xor_hosted_custody settlement_id=0x9c2f...a413
    source_token=JPYCBDC amount=125000.00
    xor_debited=312.500000 xor_rate=400.000000 haircut_bps=25 base_fee_bps=15
    fee_breakdown={base=0.046875, haircut=0.078125}
    ledger_tx=0x7ab1...ff11 telemetry_trace=nexus-settle-20260324T1142Z-lane3
```

```text
nexus_settlement_latency_seconds{lane_id="3"} 0.842
settlement_router_conversion_total{lane_id="3",source_token="JPYCBDC"} += 1
settlement_router_haircut_total{lane_id="3"} += 0.078125
```

שמרו את היומן, הטרנזקציה והמדדים באותו ארכיון. הדוגמאות הבאות מכסות נתיבים ציבוריים והיברידיים.

#### נתיב ציבורי (`xor_global`)

```text
2026-03-25T08:11:04Z settlement_router lane=0 dataspace=ds::public::creator
    handle=xor_global settlement_id=0x81cc...991c
    source_token=XOR amount=42.000000
    xor_debited=42.000000 xor_rate=1.000000 haircut_bps=0 base_fee_bps=10
    fee_breakdown={base=0.004200, liquidity=0.000000}
    dex_twap_id=twap-20260325T0810Z ledger_tx=0x319e...dd72 telemetry_trace=nexus-settle-20260325T0811Z-lane0
```

```text
nexus_settlement_latency_seconds{lane_id="0"} 0.224
settlement_router_conversion_total{lane_id="0",source_token="XOR"} += 1
settlement_router_haircut_total{lane_id="0"} += 0
```

צרפו גם את מזהה ה‑TWAP או את הערת הממשל אם נעשה שימוש בגיבוי.

#### נתיב היברידי/סודי (`xor_dual_fund`)

```text
2026-03-26T19:54:31Z settlement_router lane=9 dataspace=ds::hybrid::art
    handle=xor_dual_fund settlement_id=0x55d2...c0ab
    source_token=ARTCREDIT amount=9800.00
    xor_debited_public=12.450000 xor_debited_shielded=11.300000
    xor_rate_public=780.000000 xor_rate_shielded=820.000000
    haircut_bps=35 base_fee_bps=20 dual_fund_ratio=0.52
    fee_breakdown={base=0.239000, haircut=0.418750}
    ledger_tx=0xa924...1104 telemetry_trace=nexus-settle-20260326T1954Z-lane9
```

```text
nexus_settlement_latency_seconds{lane_id="9"} 0.973
settlement_router_conversion_total{lane_id="9",source_token="ARTCREDIT"} += 1
settlement_router_haircut_total{lane_id="9"} += 0.418750
```

השוו את היומן למדיניות ה־dual fund בקatalog הממשלתי, ייצאו `SettlementRecordV1` לנתיב ושמרו את מדדי Prometheus כדי להוכיח שהחלוקה בין המאגר הציבורי למאגר המוצפן עמדה במגבלות.

עדכנו את המסמך בכל שינוי ברואטר או במודל העמלות כדי לשמור על תאימות בין הרודמפ לתיעוד הציבורי.
