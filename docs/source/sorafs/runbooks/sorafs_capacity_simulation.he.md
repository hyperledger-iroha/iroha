---
lang: he
direction: rtl
source: docs/source/sorafs/runbooks/sorafs_capacity_simulation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a74e1cb5abc86822ff9d24b9ce42a6567d964cbc01ca4c619b49ca6d239101da
source_last_modified: "2025-11-05T18:02:08.787799+00:00"
translation_last_reviewed: 2026-01-30
---

# ראנבוק סימולציית קיבולת של SoraFS

מדריך זה מסביר כיצד להפעיל את ערכת הסימולציה של שוק הקיבולת SF-2c וכיצד להמחיש את המדדים המתקבלים. המטרה היא לאמת משא ומתן על מכסות, טיפול ב‑failover ותיקון slashing מקצה לקצה באמצעות ה‑fixtures הניתנים לשחזור תחת `docs/examples/sorafs_capacity_simulation/`. ה‑payloads של הקיבולת עדיין משתמשים ב‑`sorafs_manifest_stub capacity`; השתמשו ב‑`iroha app sorafs toolkit pack` לזרימות אריזה של manifest/CAR.

## 1. הפקת ארטיפקטים של CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

הסקריפט קורא ל‑`sorafs_manifest_stub capacity` כדי להפיק payloads של Norito דטרמיניסטיים, קידודי base64, גופי בקשה ל‑Torii וסיכומי JSON עבור:

- שלוש הצהרות ספקים המשתתפות בתרחיש משא ומתן על מכסות.
- הוראת שכפול שמקצה את המניפסט המוכן בין הספקים.
- snapshots של טלמטריה הלוכדים את קו הבסיס לפני התקלה, חלון התקלה ושחזור ה‑failover.
- payload מחלוקת המבקש slashing לאחר התקלה המדומה.

הארטיפקטים נכתבים אל `./artifacts` (או אל הנתיב שסופק כארגומנט הראשון). בדקו את קובצי `_summary.json` למצב קריא.

## 2. צבירת תוצאות והפקת מדדים

```bash
./analyze.py --artifacts ./artifacts
```

סקריפט הניתוח מפיק:

- `capacity_simulation_report.json` — הקצאות מצטברות, דלתות failover ומטא‑דטה של מחלוקת.
- `capacity_simulation.prom` — מדדי textfile של Prometheus (`sorafs_simulation_*`) המתאימים לייבוא דרך textfile collector של node-exporter או scrape job עצמאי של Prometheus.

דוגמה לקונפיגורציית scrape של `prometheus.yml`:

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

כוונו את ה‑textfile collector אל קובץ ה‑`.prom` שנוצר (ב‑node-exporter, העתיקו אותו אל `--collector.textfile.directory` שהוגדר).

## 3. ייבוא דשבורד Grafana

1. ב‑Grafana, ייבאו את `dashboards/grafana/sorafs_capacity_simulation.json`.
2. קשרו את קלט ה‑datasource `Prometheus` להגדרת ה‑scrape שלמעלה.
3. ודאו את הפאנלים:
   - **Quota Allocation (GiB)** מציג את האיזון המחויב/מוקצה לכל ספק.
   - **Failover Trigger** עובר ל‑*Failover Active* כאשר מדדי התקלה נטענים.
   - **Uptime Drop During Outage** מציג את הירידה באחוזים של הספק `alpha`.
   - **Requested Slash Percentage** ממחיש את יחס התיקון שמופק מה‑fixture של המחלוקת.

## 4. בדיקות צפויות

- `sorafs_simulation_quota_total_gib{scope="assigned"}` שווה ל‑600 כל עוד הסך המחויב נשאר ≥600.
- `sorafs_simulation_failover_triggered` מדווח `1` ומדד הספק המחליף מדגיש את `beta`.
- `sorafs_simulation_slash_requested` מדווח `0.15` (15% slash) עבור מזהה הספק `alpha`.

הריצו `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` כדי לוודא שה‑fixtures ממשיכים להיבדק מול סכמת ה‑CLI.
