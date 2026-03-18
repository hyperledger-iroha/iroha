---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/capacity-simulation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7074151fb4e8b1bd54e65c07523e27f7aa4bf2b9c83f6c17a375b261816a0734
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: capacity-simulation
lang: he
direction: rtl
source: docs/portal/docs/sorafs/capacity-simulation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: ראנבוק סימולציית קיבולת של SoraFS
sidebar_label: ראנבוק סימולציית קיבולת
description: הפעלת ערכת הסימולציה של שוק הקיבולת SF-2c עם fixtures ניתנים לשחזור, ייצואי Prometheus ולוחות Grafana.
---

:::note מקור קנוני
דף זה משקף את `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. שמרו על שתי העתקות מסונכרנות עד שמערכת התיעוד הישנה של Sphinx תועבר במלואה.
:::

מדריך זה מסביר כיצד להריץ את ערכת הסימולציה של שוק הקיבולת SF-2c וכיצד להמחיש את המדדים המתקבלים. הוא מאמת משא ומתן על מכסות, טיפול ב‑failover ותיקון slashing מקצה לקצה באמצעות ה‑fixtures הדטרמיניסטיים תחת `docs/examples/sorafs_capacity_simulation/`. ה‑payloads של הקיבולת עדיין משתמשים ב‑`sorafs_manifest_stub capacity`; השתמשו ב‑`iroha app sorafs toolkit pack` לזרימות אריזה של manifest/CAR.

## 1. הפקת ארטיפקטים של CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` עוטף את `sorafs_manifest_stub capacity` כדי להפיק payloads של Norito, blobs של base64, גופי בקשה ל‑Torii וסיכומי JSON עבור:

- שלוש הצהרות ספקים המשתתפות בתרחיש משא ומתן על מכסות.
- הוראת שכפול שמקצה את המניפסט המוכן בין הספקים הללו.
- snapshots של טלמטריה עבור קו הבסיס לפני התקלה, חלון התקלה ושחזור ה‑failover.
- payload מחלוקת המבקש slashing לאחר התקלה המדומה.

כל הארטיפקטים נכתבים תחת `./artifacts` (אפשר לעקוף על‑ידי העברת תיקייה אחרת כארגומנט הראשון). בדקו את קובצי `_summary.json` להקשר קריא.

## 2. צבירת תוצאות והפקת מדדים

```bash
./analyze.py --artifacts ./artifacts
```

המנתח מפיק:

- `capacity_simulation_report.json` - הקצאות מצטברות, דלתות failover ומטא‑דטה של מחלוקת.
- `capacity_simulation.prom` - מדדי textfile של Prometheus (`sorafs_simulation_*`) המתאימים ל‑textfile collector של node-exporter או ל‑scrape job עצמאי.

דוגמה לקונפיגורציית scrape של Prometheus:

```yaml
scrape_configs:
  - job_name: sorafs-capacity-sim
    scrape_interval: 15s
    static_configs:
      - targets: ["localhost:9100"]
        labels:
          scenario: "capacity-sim"
    metrics_path: /metrics
    params:
      format: ["prometheus"]
```

כוונו את ה‑textfile collector אל `capacity_simulation.prom` (בעת שימוש ב‑node-exporter, העתיקו אותו לתיקייה שסופקה דרך `--collector.textfile.directory`).

## 3. ייבוא דשבורד Grafana

1. ב‑Grafana, ייבאו את `dashboards/grafana/sorafs_capacity_simulation.json`.
2. קשרו את משתנה ה‑datasource `Prometheus` ליעד ה‑scrape שהוגדר למעלה.
3. ודאו את הפאנלים:
   - **Quota Allocation (GiB)** מציג את האיזון המחויב/מוקצה לכל ספק.
   - **Failover Trigger** עובר ל‑*Failover Active* כאשר מדדי התקלה מגיעים.
   - **Uptime Drop During Outage** מציג את הירידה באחוזים עבור הספק `alpha`.
   - **Requested Slash Percentage** ממחיש את יחס התיקון שמופק מה‑fixture של המחלוקת.

## 4. בדיקות צפויות

- `sorafs_simulation_quota_total_gib{scope="assigned"}` שווה ל‑`600` כל עוד הסך המחויב נשאר >=600.
- `sorafs_simulation_failover_triggered` מדווח `1` ומדד הספק המחליף מדגיש את `beta`.
- `sorafs_simulation_slash_requested` מדווח `0.15` (15% slash) עבור מזהה הספק `alpha`.

הריצו `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` כדי לוודא שה‑fixtures עדיין מתקבלים בסכמת ה‑CLI.
