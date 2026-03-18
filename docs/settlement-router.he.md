---
lang: he
direction: rtl
source: docs/settlement-router.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 782429c90ac5df034fd7c8ff2c3acf4f9f11348f14f15fcd321f343b22b154b8
source_last_modified: "2025-11-22T12:03:21.814470+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/settlement-router.md -->

# נתב סליקה דטרמיניסטי (NX-3)

**סטטוס:** הושלם (NX-3)  
**בעלים:** Economics WG / Core Ledger WG / Treasury / SRE  
**טווח:** נתיב סליקה קנוני של XOR לכל ה-lanes/dataspaces. נשלח עם crate של הנתב,
קבלות ברמת lane, guard rails לבאפרים, טלמטריה ומשטחי ראיות לאופרטורים.

## מטרות
- לאחד המרות XOR ויצירת קבלות בין בניות של lane יחיד לבין Nexus.
- להחיל haircuts דטרמיניסטיים ושוליים של תנודתיות עם באפרים מוגדרים כך שאופרטורים
  יוכלו לתזמן סליקה בבטחה.
- לחשוף קבלות, טלמטריה ודשבורדים שמבקרים יכולים לשחזר ללא כלים ייעודיים.

## ארכיטקטורה
| רכיב | מיקום | אחריות |
|-----------|----------|----------------|
| פרימיטיבים של הנתב | `crates/settlement_router/` | מחשבון shadow-price, מדרגות haircut, עזרי מדיניות באפר, טיפוס קבלת סליקה.【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】【crates/settlement_router/src/policy.rs:1】 |
| חזית ריצה | `crates/iroha_core/src/settlement/mod.rs:1` | עוטף את תצורת הנתב ל-`SettlementEngine`, חושף `quote` ו-accumulator שמשמשים בביצוע בלוק. |
| אינטגרציית בלוק | `crates/iroha_core/src/block.rs:120` | מרוקן רשומות `PendingSettlement`, מאגד `LaneSettlementCommitment` לכל lane/dataspace, מפענח מטא-דטה של באפר lane ופולט טלמטריה. |
| טלמטריה ודשבורדים | `crates/iroha_telemetry/src/metrics.rs:4847`, `dashboards/grafana/settlement_router_overview.json:1` | מדדי Prometheus/OTLP לבאפרים, שונות, haircuts, מוני המרות; לוח Grafana ל-SRE. |
| סכמת ייחוס | `docs/source/nexus_fee_model.md:1` | מתעדת את שדות קבלת הסליקה הנשמרים בתוך `LaneBlockCommitment`. |

## תצורה
פרמטרי הנתב חיים תחת `[settlement.router]` (מאומתים על ידי `iroha_config`):

```toml
[settlement.router]
twap_window_seconds = 60      # TWAP window used to derive local→XOR conversions
epsilon_bps = 25              # Base margin added to every quote (basis points)
buffer_alert_pct = 75         # Remaining-buffer % that opens an alert
buffer_throttle_pct = 25      # Remaining-buffer % where throttling begins
buffer_xor_only_pct = 10      # Remaining-buffer % where XOR-only mode is enforced
buffer_halt_pct = 2           # Remaining-buffer % where settlement halts
buffer_horizon_hours = 72     # Horizon (hours) represented by the XOR buffer
```

מטא-דטה של lane מחברת את חשבון הבאפר לכל dataspace:
- `settlement.buffer_account` — חשבון המחזיק את הרזרבה (למשל `buffer::cbdc_treasury`).
- `settlement.buffer_asset` — הגדרת נכס שנגרעת לצורך מרווח (בדרך כלל `xor#sora`).
- `settlement.buffer_capacity_micro` — קיבולת מוגדרת במיקרו-XOR (מחרוזת דצימלית).

מטא-דטה חסרה מבטלת snapshot של באפר עבור אותו lane (הטלמטריה נופלת לקיבולת/סטטוס מאופסים).

## צינור ההמרה
1. **Quote:** `SettlementEngine::quote` מחיל את ה-epsilon ואת שוליים של תנודתיות
   ומדרגת ה-haircut על ציטוטי TWAP, ומחזיר `SettlementReceipt` עם `xor_due` ו-
   `xor_after_haircut` יחד עם חותמת זמן ו-`source_id` שסופקו על ידי הקורא.【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】
2. **Accumulate:** במהלך ביצוע הבלוק ה-executor רושם רשומות `PendingSettlement`
   (סכום מקומי, TWAP, epsilon, bucket של תנודתיות, פרופיל נזילות, חותמת זמן אורקל).
   `LaneSettlementBuilder` מאגד סכומים ומטא-דטה של swap לפי `(lane, dataspace)`
   לפני חתימת הבלוק.【crates/iroha_core/src/settlement/mod.rs:34】【crates/iroha_core/src/block.rs:3460】
3. **Buffer snapshot:** אם מטא-דטה של lane מצהירה על באפר, הבונה לוכד
   `SettlementBufferSnapshot` (מרווח שנותר, קיבולת, סטטוס) תוך שימוש בספי
   `BufferPolicy` מהתצורה.【crates/iroha_core/src/block.rs:203】
4. **Commit + telemetry:** קבלות והוכחות swap נשמרות בתוך `LaneBlockCommitment`
   ומועתקות לתמונות סטטוס. טלמטריה מתעדת מדדי באפר, שונות (`iroha_settlement_pnl_xor`),
   שוליים שהוחלו (`iroha_settlement_haircut_bp`), שימוש אופציונלי ב-swapline,
   ומוני המרה/הנחה לכל נכס כדי שהדשבורדים וההתראות יהיו מסונכרנים עם תוכן הבלוק.
   【crates/iroha_core/src/block.rs:298】【crates/iroha_core/src/telemetry.rs:844】
5. **משטחי ראיות:** `status::set_lane_settlement_commitments` מפרסם התחייבויות
   עבור ממסרים/צרכני DA, דשבורדי Grafana קוראים את מדדי Prometheus, ואופרטורים
   משתמשים ב-`ops/runbooks/settlement-buffers.md` לצד
   `dashboards/grafana/settlement_router_overview.json` כדי לעקוב אחר אירועי
   refill/throttle.

## טלמטריה וראיות
- `iroha_settlement_buffer_xor`, `iroha_settlement_buffer_capacity_xor`, `iroha_settlement_buffer_status` — snapshot של באפר לכל lane/dataspace (מיקרו-XOR + מצב מקודד).【crates/iroha_telemetry/src/metrics.rs:6212】
- `iroha_settlement_pnl_xor` — שונות ממומשת בין XOR שצריך לשלם לבין XOR אחרי haircut עבור אצוות הבלוק.【crates/iroha_telemetry/src/metrics.rs:6236】
- `iroha_settlement_haircut_bp` — בסיס נקודות ה-epsilon/haircut האפקטיבי שהוחל על האצווה.【crates/iroha_telemetry/src/metrics.rs:6244】
- `iroha_settlement_swapline_utilisation` — שימוש אופציונלי לפי פרופיל נזילות כאשר קיימת הוכחת swap.【crates/iroha_telemetry/src/metrics.rs:6252】
- `settlement_router_conversion_total` / `settlement_router_haircut_total` — מונים לכל lane/dataspace עבור המרות סליקה וסך haircuts (יחידות XOR).【crates/iroha_telemetry/src/metrics.rs:6260】【crates/iroha_core/src/block.rs:304】
- לוח Grafana: `dashboards/grafana/settlement_router_overview.json` (מרווח באפר, שונות, haircuts) יחד עם כללי Alertmanager המשובצים ב-pack ההתראות של Nexus lane.
- מדריך אופרטורים: `ops/runbooks/settlement-buffers.md` (זרימת refill/alert) ו-FAQ ב-`docs/source/nexus_settlement_faq.md`.

## רשימת בדיקות למפתחים ול-SRE
- הגדירו ערכי `[settlement.router]` ב-`config/config.json5` (או TOML) ואמתו ביומני `irohad --version`; ודאו שהספים מקיימים `alert > throttle > xor_only > halt`.
- מלאו מטא-דטה של lane עם חשבון/נכס/קיבולת הבאפר כדי שמדדי הבאפר ייצגו רזרבות חיות; השמיטו את השדות עבור lanes שלא אמורים לעקוב אחרי באפרים.
- נטרו את מדדי `settlement_router_*` ו-`iroha_settlement_*` דרך `dashboards/grafana/settlement_router_overview.json`; התריעו על מצבי throttle/XOR-only/halt.
- הריצו `cargo test -p settlement_router` לכיסוי מחירים/מדיניות ואת בדיקות האגרגציה ברמת בלוק ב-`crates/iroha_core/src/block.rs`.
- תעדו אישורי ממשל לשינויי קונפיגורציה ב-`docs/source/nexus_fee_model.md` ועדכנו את `status.md` כאשר ספים או משטחי טלמטריה משתנים.

## תמונת מצב של תוכנית ההשקה
- הנתב והטלמטריה נשלחים בכל build; אין feature gates. מטא-דטה של lane שולטת אם snapshot של באפר מתפרסם.
- תצורת ברירת המחדל תואמת לערכי ה-roadmap (TWAP של 60 שניות, epsilon בסיסי 25 נק', אופק באפר של 72 שעות); כוונו דרך הקונפיג והפעילו מחדש את `irohad` כדי להחיל.
- חבילת ראיות = התחייבויות סליקה של lane + סקרייפ Prometheus לסדרות `settlement_router_*`/`iroha_settlement_*` + צילום מסך/ייצוא JSON של Grafana עבור חלון הזמן הרלוונטי.

## ראיות והפניות
- הערות קבלה של NX-3: `status.md` (סעיף NX-3).
- משטחי אופרטור: `dashboards/grafana/settlement_router_overview.json`, `ops/runbooks/settlement-buffers.md`.
- סכמת קבלות ומשטחי API: `docs/source/nexus_fee_model.md`, `/v1/sumeragi/status` -> `lane_settlement_commitments`.

</div>
