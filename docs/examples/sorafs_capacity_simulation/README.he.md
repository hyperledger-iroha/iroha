---
lang: he
direction: rtl
source: docs/examples/sorafs_capacity_simulation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 727a648141405b0c8f12a131ff903d3e7ce5b74a7f899dd99fe9aa6490b55ef2
source_last_modified: "2025-11-05T17:59:15.481814+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/sorafs_capacity_simulation/README.md -->

# ערכת סימולציית קיבולת SoraFS

ספריה זו מספקת את הארטיפקטים הניתנים לשחזור עבור סימולציית שוק הקיבולת SF-2c. הערכה מפעילה משא ומתן על מכסות, טיפול ב-failover ושיקום slashing באמצעות כלי CLI של הייצור וסקריפט ניתוח קל.

## דרישות מקדימות

- Toolchain של Rust המסוגל להריץ `cargo run` לחברי ה-workspace.
- Python 3.10+ (ספריית סטנדרט בלבד).

## התחלה מהירה

```bash
# 1. יצירת artefacts CLI קנוניים
./run_cli.sh ./artifacts

# 2. איסוף התוצאות והפקת מדדי Prometheus
./analyze.py --artifacts ./artifacts
```

הסקריפט `run_cli.sh` מפעיל `sorafs_manifest_stub capacity` כדי לבנות:

- הצהרות ספקים דטרמיניסטיות עבור סט ה-fixtures של משא ומתן מכסות.
- סדר שכפול שתואם את תרחיש המשא ומתן.
- snapshots טלמטריה לחלון ה-failover.
- payload של מחלוקת הכולל בקשת slashing.

הסקריפט כותב Norito bytes (`*.to`), payloads base64 (`*.b64`), bodies של בקשות Torii
וסיכומים קריאים (`*_summary.json`) תחת תיקיית הארטיפקטים הנבחרת.

`analyze.py` צורך את הסיכומים שנוצרו, מפיק דוח מצטבר
(`capacity_simulation_report.json`), ומייצר textfile של Prometheus
(`capacity_simulation.prom`) הכולל:

- מדדי `sorafs_simulation_quota_*` שמתארים קיבולת שנוהלה והקצאה לכל ספק.
- מדדי `sorafs_simulation_failover_*` שמדגישים שינויי downtime והספק הנבחר.
- `sorafs_simulation_slash_requested` שמקליט את אחוז השיקום מה-payload של המחלוקת.

ייבאו את חבילת Grafana ב-`dashboards/grafana/sorafs_capacity_simulation.json`
והגדירו מקור Prometheus שסורק את ה-textfile שנוצר (למשל באמצעות textfile collector של
node-exporter). ה-runbook ב-
`docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` מפרט את הזרימה המלאה,
כולל טיפים לתצורת Prometheus.

## Fixtures

- `scenarios/quota_negotiation/` — מפרטי הצהרות ספק וסדר שכפול.
- `scenarios/failover/` — חלונות טלמטריה להפסקה הראשית ולעליית failover.
- `scenarios/slashing/` — מפרט מחלוקת שמפנה לאותו סדר שכפול.

fixtures אלו מאומתים ב-`crates/sorafs_car/tests/capacity_simulation_toolkit.rs`
כדי להבטיח שהם נשארים מסונכרנים עם סכמת ה-CLI.

</div>
