---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/capacity-reconciliation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e26cc8232dd7d3b392d56646fdfbf809952f017532a37aafbfde3c8cc704ae0e
source_last_modified: "2025-12-07T08:57:10.640650+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: capacity-reconciliation
title: התאמת קיבולת SoraFS
description: זרימה לילית להתאמת ספרי עמלות קיבולת לייצואי העברות XOR.
---

פריט מפת הדרכים **SF-2c** מחייב את צוות האוצר להוכיח שפנקס עמלות הקיבולת תואם להעברות XOR המבוצעות מדי לילה. השתמשו בכלי `scripts/telemetry/capacity_reconcile.py` כדי להשוות את snapshot של `/v2/sorafs/capacity/state` מול אצוות ההעברות שבוצעו ולהפיק מטריקות טקסט של Prometheus ל-Alertmanager.

## דרישות מקדימות
- Snapshot של מצב הקיבולת (רשומות `fee_ledger`) מיוצא מ-Torii.
- ייצוא ledger לאותה חלון (JSON או NDJSON עם `provider_id_hex`,
  `kind` = settlement/penalty, ו-`amount_nano`).
- נתיב ל-textfile collector של node_exporter אם רוצים התראות.

## Runbook
```bash
python3 scripts/telemetry/capacity_reconcile.py \
  --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
  --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
  --label nightly-capacity \
  --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
  --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
```

- קודי יציאה: `0` כאשר יש התאמה, `1` כאשר חסרים settlements/penalties או שיש תשלום יתר, `2` בקלטים לא תקינים.
- צרפו את תקציר ה-JSON + hashes לחבילת האוצר תחת
  `docs/examples/sorafs_capacity_marketplace_validation/`.
- כאשר קובץ `.prom` מגיע ל-textfile collector, ההתראה
  `SoraFSCapacityReconciliationMismatch` (ראו
  `dashboards/alerts/sorafs_capacity_rules.yml`) תופעל כאשר מזוהים העברות חסרות, עודפות או בלתי צפויות.

## פלטים
- סטטוסים לפי ספק עם diffs עבור settlements ו-penalties.
- סיכומים שמיוצאים כ-gauges:
  - `sorafs_capacity_reconciliation_missing_total{kind}`
  - `sorafs_capacity_reconciliation_overpaid_total{kind}`
  - `sorafs_capacity_reconciliation_unexpected_transfers_total`
  - `sorafs_capacity_reconciliation_expected_nano{kind}`
  - `sorafs_capacity_reconciliation_actual_nano{kind}`

## טווחים צפויים וסבילות
- ההתאמה היא מדויקת: nanos מצופים מול בפועל עבור settlement/penalty חייבים להתאים עם אפס סבילות. כל פער שאינו אפס אמור לגרום לפייג'ינג של האופרטורים.
- CI מקבע digest soak של 30 יום לפנקס עמלות הקיבולת (test `capacity_fee_ledger_30_day_soak_deterministic`) ל-`71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1`. עדכנו את ה-digest רק כאשר משתנים סמנטיקות תמחור או cooldown.
- בפרופיל ה-soak (`penalty_bond_bps=0`, `strike_threshold=u32::MAX`) penalties נשארים אפס; בפרודקשן יש להפיק penalties רק כאשר נחצים ספי utilisation/uptime/PoR ולכבד את ה-cooldown המוגדר לפני slashes עוקבים.
