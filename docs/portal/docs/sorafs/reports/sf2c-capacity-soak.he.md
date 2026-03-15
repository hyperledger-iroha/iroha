---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09567fc0280e726bdd1f2f1289dc98547ac70db9b19324ef5e413c2cff34de80
source_last_modified: "2025-11-10T16:20:18.769519+00:00"
translation_last_reviewed: 2026-01-30
---

# דוח soak לצבירת קיבולת SF-2c

תאריך: 2026-03-21

## היקף

דוח זה מתעד את מבחני ה-soak הדטרמיניסטיים לצבירת קיבולת ותשלומים של SoraFS שנדרשו במסלול המפת דרכים SF-2c.

- **Soak רב-ספקים ל-30 יום:** מופעל על ידי
  `capacity_fee_ledger_30_day_soak_deterministic` ב-
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  ה-harness יוצר חמישה providers, מכסה 30 חלונות settlement ומוודא שסכומי
  ledger תואמים תחזית ייחוס מחושבת בנפרד. המבחן מפיק Blake3 digest
  (`capacity_soak_digest=...`) כדי ש-CI תוכל ללכוד ולהשוות את ה-snapshot הקנוני.
- **קנסות על אספקת חסר:** נאכפים על ידי
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (אותו קובץ). המבחן מאשר שספי strikes, cooldowns, slashes של collateral ומוני
  ledger נשארים דטרמיניסטיים.

## הרצה

הרץ את בדיקות ה-soak מקומית עם:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

הבדיקות מסתיימות בתוך פחות משנייה על לפטופ סטנדרטי ואינן דורשות fixtures חיצוניים.

## תצפיתיות

Torii כעת חושף snapshots של credit providers לצד fee ledgers כדי ש-dashboards יוכלו להתריע על יתרות נמוכות
ועל penalty strikes:

- REST: `GET /v2/sorafs/capacity/state` מחזיר רשומות `credit_ledger[*]` שמחזירות
  את שדות ledger שאומתו במבחן ה-soak. ראו
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Grafana import: `dashboards/grafana/sorafs_capacity_penalties.json` מציג את
  מוני strikes המיוצאים, סך הקנסות וה-collateral המשועבד כדי שהצוות on-call יוכל
  להשוות baselines של soak עם סביבות חיות.

## המשך

- לתזמן ריצות gate שבועיות ב-CI כדי להריץ מחדש את מבחן ה-soak (smoke-tier).
- להרחיב את לוח Grafana עם יעדי scrape של Torii לאחר שהיצוא telemetry יפעל בפרודקשן.
